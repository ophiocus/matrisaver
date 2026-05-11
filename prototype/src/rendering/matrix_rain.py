"""Matrix rain renderer."""

from __future__ import annotations

import math
import os
import random
import time
from pathlib import Path
from typing import List, Tuple

import pygame

from config import Column, Ghost, PIPELINES, RuntimeConfig
from rendering.ascii_overlay import AsciiOverlay


Color = Tuple[int, int, int]


class GlyphCache:
    def __init__(self, font: pygame.font.Font) -> None:
        self.font = font
        self.cache: dict[Tuple[str, Color], pygame.Surface] = {}

    def get(self, glyph: str, color: Color) -> pygame.Surface:
        key = (glyph, color)
        cached = self.cache.get(key)
        if cached is not None:
            return cached
        rendered = self.font.render(glyph, True, color).convert_alpha()
        self.cache[key] = rendered
        return rendered


class MatrixRain:
    def __init__(
        self,
        screen: pygame.Surface,
        font_path: str | None,
        config: RuntimeConfig,
        overlay_display_index: int,
        window_origin: Tuple[int, int],
        overlay_enabled: bool,
        performance_mode: bool,
    ) -> None:
        self.screen = screen
        self.font_path = font_path
        self.config = config
        self.overlay_display_index = max(0, overlay_display_index)
        self.window_origin = window_origin
        self.overlay_enabled = overlay_enabled
        self.performance_mode = performance_mode
        self.pipeline = config.pipeline
        self.width, self.height = screen.get_size()
        self.rain_font = self._load_font(self.config.char_size)
        self.glyph_cache = GlyphCache(self.rain_font)
        self.overlay = AsciiOverlay(self._overlay_dir()) if overlay_enabled else None
        self.columns: List[Column] = []
        self._last_char_size = self.config.char_size
        self._last_density = self.config.density
        self._last_speed_range = self.config.speed_range
        self._last_trail_length_multiplier = self.config.trail_length_multiplier
        self._super_volatile_next_change = time.time() + random.uniform(2.0, 7.0)
        self._super_volatile_pulse_time: float | None = None
        self._overlay_triggered = False
        self._overlay_trigger_time = time.time() + random.uniform(10.0, 20.0)
        self._make_columns()
        self._variant_key = "original" if self.config.density == 1.0 else ""
        if self.overlay is not None:
            self._build_overlay_grid()

    def _make_columns(self) -> None:
        self.columns.clear()
        column_count = max(1, self.width // self.config.char_size)
        for column_index in range(column_count):
            if random.random() > self.config.density:
                continue
            x = column_index * self.config.char_size
            chain_length = random.randint(12, 32)
            y_positions = [random.uniform(-self.height, 0) for _ in range(chain_length)]
            speeds = [random.uniform(*self.config.speed_range) for _ in range(chain_length)]
            current_speeds = speeds[:]
            glyphs = [random.choice(self.config.symbols) for _ in range(chain_length)]
            next_glyph_swap = time.time() + random.uniform(0.33, 5.0)
            self.columns.append(
                Column(
                    x,
                    y_positions,
                    speeds,
                    current_speeds,
                    glyphs,
                    next_glyph_swap,
                    [],
                    [],
                    [],
                    [],
                    [],
                    [],
                    [],
                    [],
                    [],
                    [],
                    [],
                    random.uniform(-self.height, 0),
                    random.uniform(2.5, 6.0),
                    1.0,
                    self.height * (self.config.trail_length_multiplier + random.uniform(0.0, 0.2)),
                    -1,
                    random.choice(self.config.symbols),
                    0,
                    0.0,
                    0.0,
                    0.0,
                    "",
                    -1,
                    False,
                )
            )
            self._reset_eraser(self.columns[-1])

    def resize(self, size: Tuple[int, int]) -> None:
        self.width, self.height = size
        self._make_columns()
        if self.overlay is not None:
            self._build_overlay_grid()

    def _build_overlay_grid(self) -> None:
        total_cols = max(1, self.width // self.config.char_size)
        total_rows = max(1, self.height // self.config.char_size)
        self.overlay.build(total_cols, total_rows, self.config.char_size)
        self._overlay_triggered = False
        self._overlay_trigger_time = time.time() + random.uniform(10.0, 20.0)

    def _col_index(self, column: Column) -> int:
        return column.x // self.config.char_size

    def update(self) -> None:
        self._sync_settings()
        now = time.time()
        if self.overlay is not None:
            if self.overlay.phase == "idle" and now >= self._overlay_trigger_time:
                self.overlay.trigger()
            self.overlay.update()
            if self.overlay.phase == "idle" and self._overlay_triggered:
                self._overlay_triggered = False
                self._overlay_trigger_time = now + random.uniform(15.0, 30.0)
            elif self.overlay.phase != "idle":
                self._overlay_triggered = True
        if self._variant_key == "original" and now >= self._super_volatile_next_change:
            self._super_volatile_pulse_time = now
            self._super_volatile_next_change = now + random.uniform(2.0, 7.0)
        else:
            self._super_volatile_pulse_time = None
        for column in self.columns:
            if self._variant_key == "original":
                self._update_original_column(column, now)
                continue
            if now >= column.next_glyph_swap:
                self._spawn_ghosts(column)
                column.glyphs = [
                    random.choice(self.config.symbols) for _ in range(len(column.glyphs))
                ]
                column.next_glyph_swap = now + random.uniform(0.33, 5.0)
            for index, y_value in enumerate(column.y_positions):
                speed = column.speeds[index]
                if random.random() < self.config.pause_chance:
                    speed *= 0.2
                if random.random() < self.config.jitter_chance:
                    speed *= random.uniform(0.4, 1.6)
                column.current_speeds[index] = speed
                column.y_positions[index] = y_value + speed
            if column.y_positions[0] > self.height + self.config.char_size * 4:
                self._reset_column(column)
            self._update_ghosts(column, now)

    def _reset_column(self, column: Column) -> None:
        chain_length = random.randint(12, 32)
        column.y_positions = [random.uniform(-self.height, 0) for _ in range(chain_length)]
        column.speeds = [random.uniform(*self.config.speed_range) for _ in range(chain_length)]
        column.current_speeds = column.speeds[:]
        column.glyphs = [random.choice(self.config.symbols) for _ in range(chain_length)]
        column.next_glyph_swap = time.time() + random.uniform(0.33, 5.0)
        column.ghosts = []
        column.original_symbols = []
        column.original_brightness = []
        column.original_volatile = []
        column.original_volatile_next = []
        column.original_volatile_last = []
        column.original_super_volatile = []
        column.original_super_opacity = []
        column.original_gamma = []
        column.original_bloom = []
        column.volatile_cells = []
        column.head_y = random.uniform(-self.height, 0)
        column.head_speed = random.uniform(2.5, 6.0)
        column.head_opacity = 1.0
        column.delete_gap = (
            self.height * (self.config.trail_length_multiplier + random.uniform(0.0, 0.2))
        )
        column.last_head_row = -1
        column.head_row_step = 0
        column.head_glyph = random.choice(self.config.symbols)
        self._reset_eraser(column)

    def _sync_settings(self) -> None:
        if self.config.char_size != self._last_char_size or self.config.density != self._last_density:
            self._last_char_size = self.config.char_size
            self._last_density = self.config.density
            self.rain_font = self._load_font(self.config.char_size)
            self.glyph_cache = GlyphCache(self.rain_font)
            self._make_columns()
            if self.overlay is not None:
                self._build_overlay_grid()
        if self.config.trail_length_multiplier < 0.5:
            self.config.trail_length_multiplier = 0.5
        if self.config.pipeline not in PIPELINES:
            self.config.pipeline = "cpu"
        if self.config.trail_length_multiplier != self._last_trail_length_multiplier:
            self._last_trail_length_multiplier = self.config.trail_length_multiplier
            for column in self.columns:
                column.delete_gap = (
                    self.height
                    * (self.config.trail_length_multiplier + random.uniform(0.0, 0.2))
                )
        if self.config.speed_range != self._last_speed_range:
            self._last_speed_range = self.config.speed_range
            for column in self.columns:
                column.speeds = [
                    random.uniform(*self.config.speed_range) for _ in range(len(column.speeds))
                ]
                column.current_speeds = column.speeds[:]
        if self.config.gamma_range[0] > self.config.gamma_range[1]:
            self.config.gamma_range = (self.config.gamma_range[1], self.config.gamma_range[0])
        if self.config.bloom_range[0] > self.config.bloom_range[1]:
            self.config.bloom_range = (self.config.bloom_range[1], self.config.bloom_range[0])

    def draw(self) -> None:
        now = time.time() if self._variant_key == "original" else 0.0
        for column in self.columns:
            if self._variant_key == "original":
                self._draw_original_column(column, now)
                continue
            self._draw_ghosts(column)
            for index, y_value in enumerate(column.y_positions):
                if 0 <= y_value < self.height:
                    glyph = column.glyphs[index]
                    speed_factor = max(
                        0.25,
                        column.current_speeds[index] / max(1.0, float(self.config.speed_range[1])),
                    )
                    core = tuple(
                        min(255, int(channel * speed_factor)) for channel in self.config.color
                    )
                    rendered = self.glyph_cache.get(glyph, core)
                    x_pos = column.x
                    y_pos = int(y_value)
                    self.screen.blit(rendered, (x_pos, y_pos))


    def _overlay_dir(self) -> Path:
        base = Path(__file__).resolve()
        candidates = [
            base.parents[4] / "assets" / "overlays",  # repo-root layout
            base.parents[3] / "assets" / "overlays",  # legacy layout fallback
        ]
        for candidate in candidates:
            if candidate.is_dir():
                return candidate
        return candidates[0]

    def _load_font(self, size: int) -> pygame.font.Font:
        if self.font_path:
            try:
                return pygame.font.Font(self.font_path, size)
            except (FileNotFoundError, TypeError):
                pass
        return pygame.font.SysFont("DejaVu Sans", size)

    def _is_overlay_locked(self, col_idx: int, row_idx: int) -> bool:
        if self.overlay is None:
            return False
        return self.overlay.is_overlay_cell(col_idx, row_idx)

    def _update_original_column(self, column: Column, now: float) -> None:
        total_rows = max(1, self.height // self.config.char_size)
        col_idx = self._col_index(column)
        if len(column.original_symbols) != total_rows:
            column.original_symbols = ["" for _ in range(total_rows)]
            column.original_brightness = [0.0 for _ in range(total_rows)]
            column.original_volatile = [False for _ in range(total_rows)]
            column.original_volatile_next = [0.0 for _ in range(total_rows)]
            column.original_volatile_last = [0.0 for _ in range(total_rows)]
            column.original_super_volatile = [False for _ in range(total_rows)]
            column.original_super_opacity = [0.0 for _ in range(total_rows)]
            column.original_gamma = [1.0 for _ in range(total_rows)]
            column.original_bloom = [0.0 for _ in range(total_rows)]
            column.volatile_cells = []

        if self.overlay is not None and self.overlay.phase == "intro":
            for row_idx in range(total_rows):
                if self.overlay.is_overlay_cell(col_idx, row_idx):
                    self.overlay.save_undertext(
                        col_idx, row_idx,
                        column.original_symbols[row_idx],
                        column.original_brightness[row_idx],
                    )

        column.head_y += column.head_speed
        column.eraser_y += column.eraser_speed
        if column.eraser_y <= column.head_y:
            if column.eraser_y >= column.head_y - self.config.char_size:
                self._reset_head(column)
                self._reset_eraser(column)
        else:
            if column.head_y >= column.eraser_y - self.config.char_size:
                self._reset_eraser(column)
        if column.eraser_y > self.height + self.config.char_size * 2:
            self._reset_eraser(column)
        if column.head_y > self.height + self.config.char_size * 2:
            self._reset_head(column)
        head_row = int(column.head_y // self.config.char_size)
        if 0 <= head_row < total_rows and not self._is_overlay_locked(col_idx, head_row):
            if head_row != column.last_head_row:
                column.last_head_row = head_row
                column.head_row_step = 1
                column.head_glyph = self._next_head_glyph(column.head_glyph)
                column.original_symbols[head_row] = column.head_glyph
                column.original_brightness[head_row] = self._base_opacity()
                self._assign_original_effects(column, head_row)
            else:
                if column.head_row_step < 2:
                    column.head_row_step += 1
                    column.head_glyph = self._next_head_glyph(column.head_glyph)
                    column.original_symbols[head_row] = column.head_glyph
                    column.original_brightness[head_row] = self._base_opacity()
                    self._assign_original_effects(column, head_row)

        self._update_volatile_jumps(column, now, total_rows)

        delete_row = int((column.head_y - column.delete_gap) // self.config.char_size)
        if 0 <= delete_row < total_rows and not self._is_overlay_locked(col_idx, delete_row):
            self._erase_row(column, delete_row)

        eraser_row = int(column.eraser_y // self.config.char_size)
        if 0 <= eraser_row < total_rows and not self._is_overlay_locked(col_idx, eraser_row):
            if eraser_row != column.eraser_last_row:
                column.eraser_last_row = eraser_row
                column.eraser_glyph = random.choice(self.config.symbols)
            self._erase_row(column, eraser_row)

        if self.overlay is not None and self.overlay.phase == "outro":
            for row_idx in range(total_rows):
                if not self.overlay.is_overlay_cell(col_idx, row_idx):
                    saved = self.overlay.get_saved_undertext(col_idx, row_idx)
                    if saved[0]:
                        column.original_symbols[row_idx] = saved[0]
                        column.original_brightness[row_idx] = saved[1]

    def _draw_original_column(self, column: Column, now: float) -> None:
        total_rows = len(column.original_symbols)
        head_row = int(column.head_y // self.config.char_size)
        delete_row = int((column.head_y - column.delete_gap) // self.config.char_size)
        col_idx = self._col_index(column)
        for row_index in range(total_rows):
            overlay_data = None
            fade = 0.0
            if self.overlay is not None and self.overlay.is_overlay_cell(col_idx, row_index):
                overlay_data = self.overlay.get_overlay_glyph(col_idx, row_index)
                fade = self.overlay.fade_factor(col_idx, row_index)

            if overlay_data is not None and fade > 0.0:
                ov_glyph, ov_opacity = overlay_data
                ov_brightness = ov_opacity * fade
                color = self._gamma_color(
                    (0, min(255, int(255 * ov_brightness)), 0), 1.0
                )
                rendered = self.glyph_cache.get(ov_glyph, color)
                x_pos = column.x
                y_pos = row_index * self.config.char_size
                self.screen.blit(rendered, (x_pos, y_pos))
                if fade < 1.0:
                    rain_glyph = column.original_symbols[row_index]
                    rain_brightness = column.original_brightness[row_index]
                    if rain_glyph and rain_brightness > 0.0:
                        rain_color = self._gamma_color(
                            (0, min(255, int(255 * rain_brightness * (1.0 - fade))), 0), 1.0
                        )
                        rain_rendered = self.glyph_cache.get(rain_glyph, rain_color)
                        self.screen.blit(rain_rendered, (x_pos, y_pos))
                continue

            glyph = column.original_symbols[row_index]
            if not glyph:
                continue
            brightness = column.original_brightness[row_index]
            if brightness <= 0.0:
                continue
            brightness = self._volatile_opacity(column, row_index, now, brightness)
            gamma = self._volatile_gamma(column, row_index, now, column.original_gamma[row_index])
            if row_index == head_row:
                color = self._gamma_color((120, 220, 255), gamma)
            else:
                color = self._gamma_color((0, min(255, int(255 * brightness)), 0), gamma)
            rendered = self.glyph_cache.get(glyph, color)
            x_pos = column.x
            y_pos = row_index * self.config.char_size
            if row_index == head_row:
                self.screen.blit(rendered, (x_pos, y_pos))
            else:
                self.screen.blit(rendered, (x_pos, y_pos))
                if column.original_super_volatile[row_index]:
                    bold_color = self._gamma_color((0, 255, 120), gamma)
                    bold_rendered = self.glyph_cache.get(glyph, bold_color).copy()
                    bold_rendered.set_alpha(int(column.original_super_opacity[row_index] * 255))
                    self.screen.blit(bold_rendered, (x_pos + 1, y_pos))
                    self.screen.blit(bold_rendered, (x_pos, y_pos + 1))

        eraser_row = int(column.eraser_y // self.config.char_size)
        if 0 <= eraser_row < total_rows and not self._is_overlay_locked(col_idx, eraser_row):
            gamma = self._volatile_gamma(column, eraser_row, now, column.original_gamma[eraser_row])
            color = self._gamma_color((160, 255, 200), gamma)
            rendered = self.glyph_cache.get(column.eraser_glyph, color)
            self.screen.blit(rendered, (column.x, eraser_row * self.config.char_size))

        if 0 <= delete_row < total_rows and not self._is_overlay_locked(col_idx, delete_row):
            erase_rect = pygame.Rect(
                column.x,
                delete_row * self.config.char_size,
                self.config.char_size,
                self.config.char_size,
            )
            self.screen.fill((0, 0, 0), erase_rect)


    def _assign_original_effects(self, column: Column, row_index: int) -> None:
        if random.random() < self.config.volatile_chance:
            column.original_volatile[row_index] = True
            column.original_volatile_next[row_index] = 0.0
            column.original_volatile_last[row_index] = 0.0
            column.original_super_volatile[row_index] = random.random() < 0.3
            column.original_super_opacity[row_index] = random.uniform(0.0, 1.0)
            cell = (column.x, row_index)
            if cell not in column.volatile_cells:
                column.volatile_cells.append(cell)
        else:
            cell = (column.x, row_index)
            if cell in column.volatile_cells:
                column.volatile_cells.remove(cell)
            column.original_volatile[row_index] = False
            column.original_volatile_next[row_index] = 0.0
            column.original_volatile_last[row_index] = 0.0
            column.original_super_volatile[row_index] = False
            column.original_super_opacity[row_index] = 0.0
        column.original_gamma[row_index] = self._base_gamma()
        column.original_bloom[row_index] = random.uniform(*self.config.bloom_range)

    def _erase_row(self, column: Column, row_index: int) -> None:
        column.original_symbols[row_index] = ""
        column.original_brightness[row_index] = 0.0
        if column.original_volatile[row_index]:
            cell = (column.x, row_index)
            if cell in column.volatile_cells:
                column.volatile_cells.remove(cell)
        column.original_volatile[row_index] = False
        column.original_volatile_next[row_index] = 0.0
        column.original_volatile_last[row_index] = 0.0
        column.original_super_volatile[row_index] = False
        column.original_super_opacity[row_index] = 0.0
        column.original_gamma[row_index] = 1.0
        column.original_bloom[row_index] = 0.0

    def _update_volatile_jumps(self, column: Column, now: float, total_rows: int) -> None:
        if self._super_volatile_pulse_time is not None:
            for row_index in range(total_rows):
                if column.original_super_volatile[row_index]:
                    column.original_symbols[row_index] = self._shift_glyph(
                        column.original_symbols[row_index]
                    )
                    column.original_volatile_last[row_index] = self._super_volatile_pulse_time
        for row_index in range(total_rows):
            if not column.original_volatile[row_index]:
                continue
            glyph = column.original_symbols[row_index]
            if not glyph:
                continue
            if column.original_volatile_next[row_index] == 0.0:
                column.original_volatile_next[row_index] = now + random.uniform(0.5, 15.0)
                continue
            if now < column.original_volatile_next[row_index]:
                continue
            column.original_symbols[row_index] = self._shift_glyph(glyph)
            column.original_volatile_last[row_index] = now
            column.original_volatile_next[row_index] = now + random.uniform(0.5, 15.0)

    def _volatile_gamma(
        self,
        column: Column,
        row_index: int,
        now: float,
        base_gamma: float,
    ) -> float:
        if not column.original_volatile[row_index]:
            return base_gamma
        next_change = column.original_volatile_next[row_index]
        if next_change <= 0.0:
            return base_gamma
        max_gamma = max(base_gamma, self.config.gamma_range[1])
        remaining = next_change - now
        if 0.0 < remaining <= 1.0:
            progress = 1.0 - remaining
            pulse = 0.5 + 0.5 * math.sin(now * 18.0)
            return base_gamma + (max_gamma - base_gamma) * (0.7 * progress + 0.3 * pulse)
        last_change = column.original_volatile_last[row_index]
        if last_change > 0.0:
            elapsed = now - last_change
            if 0.0 <= elapsed <= 1.0:
                return max_gamma + (base_gamma - max_gamma) * elapsed
        return base_gamma

    def _volatile_opacity(
        self,
        column: Column,
        row_index: int,
        now: float,
        base_opacity: float,
    ) -> float:
        if not column.original_volatile[row_index]:
            return base_opacity
        next_change = column.original_volatile_next[row_index]
        if next_change <= 0.0:
            return base_opacity
        max_opacity = 1.0
        remaining = next_change - now
        if 0.0 < remaining <= 1.0:
            progress = 1.0 - remaining
            pulse = 0.5 + 0.5 * math.sin(now * 18.0)
            return base_opacity + (max_opacity - base_opacity) * (0.7 * progress + 0.3 * pulse)
        last_change = column.original_volatile_last[row_index]
        if last_change > 0.0:
            elapsed = now - last_change
            if 0.0 <= elapsed <= 1.0:
                return max_opacity + (base_opacity - max_opacity) * elapsed
        return base_opacity

    def _base_opacity(self) -> float:
        return random.uniform(0.4, 0.8)

    def _base_gamma(self) -> float:
        low, high = self.config.gamma_range
        span = max(0.0, high - low)
        return low + span * random.uniform(0.4, 0.8)

    def _next_head_glyph(self, current: str) -> str:
        symbols = self.config.symbols
        if not symbols:
            return current
        if current not in symbols and len(symbols) == 1:
            return symbols[0]
        next_glyph = current
        while next_glyph == current:
            next_glyph = random.choice(symbols)
        return next_glyph

    def _shift_glyph(self, glyph: str) -> str:
        symbols = self.config.symbols
        if glyph in symbols:
            offset = random.randint(5, 50)
            direction = 1 if random.random() < 0.5 else -1
            current_index = symbols.index(glyph)
            new_index = (current_index + direction * offset) % len(symbols)
            return symbols[new_index]
        return random.choice(symbols)

    def _reset_eraser(self, column: Column) -> None:
        speed_factor = random.uniform(0.6, 1.4)
        column.eraser_speed = max(0.4, column.head_speed * speed_factor)
        column.eraser_offset = random.uniform(
            self.config.char_size * 6,
            self.height * 0.35,
        )
        column.eraser_head_above = random.random() < 0.25
        if column.eraser_head_above:
            column.eraser_y = column.head_y + column.eraser_offset
        else:
            column.eraser_y = column.head_y - column.eraser_offset
        column.eraser_last_row = -1
        column.eraser_glyph = random.choice(self.config.symbols)

    def _reset_head(self, column: Column) -> None:
        column.head_y = random.uniform(-self.height, 0)
        column.head_speed = random.uniform(2.5, 6.0)
        column.head_opacity = 1.0
        column.delete_gap = (
            self.height * self.config.trail_length_multiplier + random.uniform(0.0, 0.2)
        )
        column.last_head_row = -1
        column.head_row_step = 0
        column.head_glyph = random.choice(self.config.symbols)

    @staticmethod
    def _gamma_color(color: Color, gamma: float) -> Color:
        if gamma <= 0:
            return color
        return tuple(
            min(255, max(0, int((channel / 255.0) ** gamma * 255))) for channel in color
        )

    def _spawn_ghosts(self, column: Column) -> None:
        chance = self.config.ghost_chance
        now = time.time()
        for index, y_value in enumerate(column.y_positions):
            if 0 <= y_value < self.height and random.random() < chance:
                column.ghosts.append(
                    Ghost(
                        x=column.x,
                        y=y_value,
                        glyph=column.glyphs[index],
                        next_swap=now + random.uniform(0.33, 5.0) * self.config.ghost_swap_multiplier,
                    )
                )

    def _update_ghosts(self, column: Column, now: float) -> None:
        head_y = column.y_positions[0]
        cutoff = self.config.char_size * 0.9
        updated: List[Ghost] = []
        for ghost in column.ghosts:
            if abs(ghost.y - head_y) <= cutoff:
                continue
            if ghost.y < -self.config.char_size or ghost.y > self.height + self.config.char_size:
                continue
            if now >= ghost.next_swap:
                ghost.glyph = random.choice(self.config.symbols)
                ghost.next_swap = (
                    now + random.uniform(0.33, 5.0) * self.config.ghost_swap_multiplier
                )
            updated.append(ghost)
        column.ghosts = updated

    def _draw_ghosts(self, column: Column) -> None:
        ghost_color = tuple(int(channel * 0.5) for channel in self.config.color)
        for ghost in column.ghosts:
            if 0 <= ghost.y < self.height:
                rendered = self.rain_font.render(ghost.glyph, True, ghost_color)
                self.screen.blit(rendered, (ghost.x, int(ghost.y)))
