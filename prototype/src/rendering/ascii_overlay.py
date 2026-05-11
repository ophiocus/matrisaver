"""ASCII overlay for images."""

from __future__ import annotations

import random
import time
from pathlib import Path
from typing import Dict, List, Set, Tuple

import pygame

try:
    from PIL import Image as PilImage, ImageEnhance, ImageFilter, ImageOps
    _PIL_AVAILABLE = True
except ImportError:
    _PIL_AVAILABLE = False

from config import ASCII_GRADIENT, Color


class OverlayCell:
    __slots__ = ("glyph", "opacity", "reveal_time", "saved_glyph", "saved_brightness")

    def __init__(self, glyph: str, opacity: float, reveal_time: float) -> None:
        self.glyph = glyph
        self.opacity = opacity
        self.reveal_time = reveal_time
        self.saved_glyph = ""
        self.saved_brightness = 0.0


class AsciiOverlay:
    INTRO_DURATION = 3.0
    OUTRO_DURATION = 3.0
    HOLD_DURATION = 8.0

    IMAGE_EXTENSIONS = {".png", ".jpg", ".jpeg", ".bmp", ".gif", ".tga", ".tiff", ".webp"}

    # Preprocessing constants applied when writing images into use/.
    _USE_CONTRAST_FACTOR = 1.5
    _USE_UNSHARP_RADIUS = 2.0
    _USE_UNSHARP_PERCENT = 150
    _USE_UNSHARP_THRESHOLD = 3

    def __init__(self, overlay_dir: Path) -> None:
        self.overlay_dir = overlay_dir
        self._use_dir = overlay_dir / "use"
        self.grid: Dict[Tuple[int, int], Tuple[str, float]] = {}
        self.phase = "idle"
        self._phase_start = 0.0
        self._active_cells: Dict[Tuple[int, int], OverlayCell] = {}
        self._release_times: Dict[Tuple[int, int], float] = {}
        self._last_cols = 0
        self._last_rows = 0
        self._last_char_size = 0
        self._queue: List[Path] = []

    # ------------------------------------------------------------------
    # use/ sync
    # ------------------------------------------------------------------

    def _origin_images(self) -> Dict[str, Path]:
        """Return {filename: path} for all images directly in overlay_dir."""
        if not self.overlay_dir.is_dir():
            return {}
        return {
            p.name: p
            for p in self.overlay_dir.iterdir()
            if p.is_file() and p.suffix.lower() in self.IMAGE_EXTENSIONS
        }

    def _use_images(self) -> Dict[str, Path]:
        """Return {filename: path} for all images in use/."""
        if not self._use_dir.is_dir():
            return {}
        return {
            p.name: p
            for p in self._use_dir.iterdir()
            if p.is_file() and p.suffix.lower() in self.IMAGE_EXTENSIONS
        }

    def _sync_use_folder(self) -> None:
        """Mirror origin → use/ on startup.

        - Images in origin but not in use/: preprocess and copy into use/.
        - Images in use/ but not in origin: delete from use/.
        - Images present in both: left as-is (not re-processed).
        """
        self._use_dir.mkdir(parents=True, exist_ok=True)
        origin = self._origin_images()
        use = self._use_images()

        # Remove stale entries from use/.
        for name in list(use):
            if name not in origin:
                try:
                    use[name].unlink()
                except OSError:
                    pass

        # Process and copy new arrivals from origin.
        for name, src in origin.items():
            if name not in use:
                dest = self._use_dir / name
                self._preprocess_and_save(src, dest)

    def _preprocess_and_save(self, src: Path, dest: Path) -> None:
        """Apply contrast/sharpening to src and save the result to dest.

        The source file is never modified.  Falls back to a plain copy if
        PIL is unavailable.
        """
        if not _PIL_AVAILABLE:
            import shutil
            try:
                shutil.copy2(src, dest)
            except OSError:
                pass
            return
        try:
            pil_img = PilImage.open(src).convert("RGBA")
            rgb = pil_img.convert("RGB")
            rgb = ImageOps.autocontrast(rgb, cutoff=1)
            rgb = ImageEnhance.Contrast(rgb).enhance(self._USE_CONTRAST_FACTOR)
            rgb = rgb.filter(
                ImageFilter.UnsharpMask(
                    radius=self._USE_UNSHARP_RADIUS,
                    percent=self._USE_UNSHARP_PERCENT,
                    threshold=self._USE_UNSHARP_THRESHOLD,
                )
            )
            # Re-merge processed RGB with original alpha so transparency is preserved.
            r, g, b = rgb.split()
            _, _, _, a = pil_img.split()
            processed = PilImage.merge("RGBA", (r, g, b, a))
            processed.save(dest, format="PNG")
        except Exception:
            import shutil
            try:
                shutil.copy2(src, dest)
            except OSError:
                pass

    # ------------------------------------------------------------------
    # Scanning and queue
    # ------------------------------------------------------------------

    def _scan_images(self) -> List[Path]:
        """Return sorted list of ready-to-use images from use/."""
        if not self._use_dir.is_dir():
            return []
        return sorted(
            p for p in self._use_dir.iterdir()
            if p.is_file() and p.suffix.lower() in self.IMAGE_EXTENSIONS
        )

    def _next_image(self) -> Path | None:
        if not self._queue:
            self._queue = self._scan_images()
        if not self._queue:
            return None
        return self._queue.pop(0)

    # ------------------------------------------------------------------
    # Lifecycle
    # ------------------------------------------------------------------

    def build(self, total_cols: int, total_rows: int, char_size: int) -> None:
        self._sync_use_folder()
        self.grid = {}
        self._active_cells = {}
        self._release_times = {}
        self.phase = "idle"
        self._last_cols = total_cols
        self._last_rows = total_rows
        self._last_char_size = char_size

    def _build_from_image(self, image_path: Path) -> None:
        self.grid = {}
        total_cols = self._last_cols
        total_rows = self._last_rows
        char_size = self._last_char_size
        if not image_path.exists():
            return
        try:
            image = pygame.image.load(str(image_path)).convert_alpha()
        except pygame.error:
            return

        image_width, image_height = image.get_size()
        max_width = total_cols * char_size
        max_height = total_rows * char_size
        scale = min(max_width / image_width, max_height / image_height)
        scaled_w = max(1, int(image_width * scale))
        scaled_h = max(1, int(image_height * scale))
        image = pygame.transform.smoothscale(image, (scaled_w, scaled_h))

        img_cols = max(1, scaled_w // char_size)
        img_rows = max(1, scaled_h // char_size)
        col_offset = (total_cols - img_cols) // 2
        row_offset = (total_rows - img_rows) // 2

        for row in range(img_rows):
            for col in range(img_cols):
                sample_x = min(scaled_w - 1, col * char_size + char_size // 2)
                sample_y = min(scaled_h - 1, row * char_size + char_size // 2)
                pixel = image.get_at((sample_x, sample_y))
                r, g, b = pixel[0], pixel[1], pixel[2]
                a = pixel[3] if len(pixel) > 3 else 255
                luminance = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0
                if a < 20 or luminance < 0.05:
                    continue
                idx = min(len(ASCII_GRADIENT) - 1, int(luminance * (len(ASCII_GRADIENT) - 1)))
                glyph = ASCII_GRADIENT[idx]
                if glyph == " ":
                    continue
                opacity = max(0.2, min(1.0, luminance * (a / 255.0)))
                grid_col = col_offset + col
                grid_row = row_offset + row
                if 0 <= grid_col < total_cols and 0 <= grid_row < total_rows:
                    self.grid[(grid_col, grid_row)] = (glyph, opacity)

    def trigger(self) -> None:
        if self.phase != "idle":
            return
        image = self._next_image()
        if image is None:
            return
        self._build_from_image(image)
        if not self.grid:
            return
        now = time.time()
        self.phase = "intro"
        self._phase_start = now
        keys = list(self.grid.keys())
        random.shuffle(keys)
        self._active_cells = {}
        for i, key in enumerate(keys):
            glyph, opacity = self.grid[key]
            t = now + (i / max(1, len(keys))) * self.INTRO_DURATION
            self._active_cells[key] = OverlayCell(glyph, opacity, t)
        self._release_times = {}

    def update(self) -> None:
        if self.phase == "idle":
            return
        now = time.time()
        elapsed = now - self._phase_start
        if self.phase == "intro":
            if elapsed >= self.INTRO_DURATION:
                self.phase = "hold"
                self._phase_start = now
        elif self.phase == "hold":
            if elapsed >= self.HOLD_DURATION:
                self._begin_outro(now)
        elif self.phase == "outro":
            if elapsed >= self.OUTRO_DURATION:
                self.phase = "idle"
                self._active_cells = {}
                self._release_times = {}

    def _begin_outro(self, now: float) -> None:
        self.phase = "outro"
        self._phase_start = now
        keys = list(self._active_cells.keys())
        random.shuffle(keys)
        self._release_times = {}
        for i, key in enumerate(keys):
            self._release_times[key] = now + (i / max(1, len(keys))) * self.OUTRO_DURATION

    def is_overlay_cell(self, col_index: int, row_index: int) -> bool:
        if self.phase == "idle":
            return False
        key = (col_index, row_index)
        cell = self._active_cells.get(key)
        if cell is None:
            return False
        now = time.time()
        if self.phase == "intro":
            return now >= cell.reveal_time
        if self.phase == "hold":
            return True
        if self.phase == "outro":
            release = self._release_times.get(key, 0.0)
            return now < release
        return False

    def get_overlay_glyph(self, col_index: int, row_index: int) -> Tuple[str, float] | None:
        key = (col_index, row_index)
        cell = self._active_cells.get(key)
        if cell is None:
            return None
        return (cell.glyph, cell.opacity)

    def save_undertext(self, col_index: int, row_index: int, glyph: str, brightness: float) -> None:
        key = (col_index, row_index)
        cell = self._active_cells.get(key)
        if cell is not None and not cell.saved_glyph:
            cell.saved_glyph = glyph
            cell.saved_brightness = brightness

    def get_saved_undertext(self, col_index: int, row_index: int) -> Tuple[str, float]:
        key = (col_index, row_index)
        cell = self._active_cells.get(key)
        if cell is not None:
            return (cell.saved_glyph, cell.saved_brightness)
        return ("", 0.0)

    def fade_factor(self, col_index: int, row_index: int) -> float:
        key = (col_index, row_index)
        cell = self._active_cells.get(key)
        if cell is None:
            return 0.0
        now = time.time()
        if self.phase == "intro":
            elapsed = now - cell.reveal_time
            if elapsed < 0.0:
                return 0.0
            return min(1.0, elapsed / 0.5)
        if self.phase == "hold":
            return 1.0
        if self.phase == "outro":
            release = self._release_times.get(key, 0.0)
            remaining = release - now
            if remaining <= 0.0:
                return 0.0
            return min(1.0, remaining / 0.5)
        return 0.0
