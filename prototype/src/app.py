"""Application entry logic."""

from __future__ import annotations

import os
import sys
import traceback
from pathlib import Path
from typing import List, Sequence, Tuple

import pygame

from config import KATAKANA, LATIN, NUMERALS, SYMBOLS, RuntimeConfig, VARIANTS
from effects.trace_effect import TraceEffect
from rendering.matrix_rain import MatrixRain
from rendering.opengl_pipeline import OpenGLPipeline
from ui.developer_panel import DeveloperPanel
from ui.menu import Menu


class App:
    def __init__(
        self,
        variant_key: str | None,
        font_path: str | None,
        span_all_displays: bool,
        overlay_enabled: bool,
        performance_mode: bool,
    ) -> None:
        pygame.init()
        pygame.display.set_caption("Matrix Screensaver")
        screen_size, flags = self._window_settings(span_all_displays)
        self.screen = pygame.display.set_mode(screen_size, flags)
        self.render_surface = self.screen
        self.clock = pygame.time.Clock()
        self.font = self._load_font(font_path, 22)
        self.font_path = font_path
        self.variant_key = variant_key
        self.overlay_enabled = overlay_enabled
        self.performance_mode = performance_mode
        self.span_all_displays = span_all_displays
        self.log_path = os.environ.get("MATRISAVER_CHILD_LOG")
        self.matrix: MatrixRain | None = None
        self.trace_effect: TraceEffect | None = None
        self.runtime_config: RuntimeConfig | None = None
        self.dev_panel: DeveloperPanel | None = None
        self.dev_panel_visible = False
        self.opengl_pipeline: OpenGLPipeline | None = None
        self.overlay_display_index = self._primary_display_index()
        self.window_origin = self._window_origin()
        self.menu = Menu(self.screen, self.font)
        self._log("app_init", f"screen={screen_size} flags={flags} origin={self.window_origin}")

    def _load_font(self, font_path: str | None, size: int) -> pygame.font.Font:
        for candidate in self._font_candidates(font_path):
            if candidate is None:
                return pygame.font.SysFont("DejaVu Sans", size)
            try:
                return pygame.font.Font(candidate, size)
            except FileNotFoundError:
                continue
        return pygame.font.SysFont("DejaVu Sans", size)

    def _font_candidates(self, font_path: str | None) -> List[str | None]:
        candidates: List[str | None] = []
        if font_path:
            candidates.append(font_path)
        candidates.extend(
            [
                "assets/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                None,
            ]
        )
        return candidates

    def _window_settings(self, span_all_displays: bool) -> Tuple[Tuple[int, int], int]:
        if span_all_displays:
            override = os.environ.get("MATRISAVER_VIRTUAL_BOUNDS")
            if override:
                rect = self._parse_rect(override)
                if rect:
                    os.environ["SDL_VIDEO_WINDOW_POS"] = f"{rect.x},{rect.y}"
                    return (rect.width, rect.height), pygame.NOFRAME
            desktop_sizes = pygame.display.get_desktop_sizes()
            if len(desktop_sizes) > 1:
                total_width = sum(size[0] for size in desktop_sizes)
                max_height = max(size[1] for size in desktop_sizes)
                window_pos = os.environ.get("MATRISAVER_WINDOW_POS", "0,0")
                os.environ["SDL_VIDEO_WINDOW_POS"] = window_pos
                return (total_width, max_height), pygame.NOFRAME
        return (0, 0), pygame.FULLSCREEN

    def _parse_rect(self, value: str) -> pygame.Rect | None:
        parts = [part.strip() for part in value.split(",")]
        if len(parts) != 4:
            return None
        try:
            x, y, width, height = (int(part) for part in parts)
        except ValueError:
            return None
        return pygame.Rect(x, y, width, height)

    def _window_origin(self) -> Tuple[int, int]:
        try:
            return pygame.display.get_window_position()
        except AttributeError:
            return (0, 0)

    def _primary_display_index(self) -> int:
        try:
            primary = pygame.display.get_primary_display()
            return max(0, primary)
        except AttributeError:
            return 0

    def run(self) -> None:
        variant_key = self.variant_key or self.menu.options[self.menu.selected]
        self._start_variant(variant_key)
        self._loop()

    def _start_variant(self, key: str) -> None:
        variant = VARIANTS[key]
        self.runtime_config = variant.to_runtime(22)
        if not self._font_supports_katakana(self.font):
            self.runtime_config.symbols = NUMERALS + SYMBOLS + LATIN
        self._configure_pipeline()
        self.matrix = MatrixRain(
            self.render_surface,
            self._resolve_font_path(self.font_path),
            self.runtime_config,
            self.overlay_display_index,
            self.window_origin,
            self.overlay_enabled,
            self.performance_mode,
        )
        self.matrix._variant_key = key
        self.trace_effect = TraceEffect(self.render_surface, self.font, variant)
        self.dev_panel = DeveloperPanel(
            self.render_surface,
            self.font,
            self.runtime_config,
            on_close=self._quit,
            variant_key=key,
            on_variant_change=self._apply_variant,
            on_pipeline_change=self._apply_pipeline,
        )

    def _apply_variant(self, key: str) -> None:
        variant = VARIANTS[key]
        if self.runtime_config is None:
            self.runtime_config = variant.to_runtime(22)
        else:
            self.runtime_config.color = variant.color
            self.runtime_config.speed_range = variant.speed_range
            self.runtime_config.density = variant.density
            self.runtime_config.symbols = variant.symbols
            self.runtime_config.glow_color = variant.glow_color
            self.runtime_config.pause_chance = variant.pause_chance
            self.runtime_config.jitter_chance = variant.jitter_chance
            self.runtime_config.ghost_chance = variant.ghost_chance
            self.runtime_config.ghost_swap_multiplier = variant.ghost_swap_multiplier
            self.runtime_config.trail_length_multiplier = variant.trail_length_multiplier
            self.runtime_config.volatile_chance = variant.volatile_chance
            self.runtime_config.gamma_range = variant.gamma_range
            self.runtime_config.bloom_range = variant.bloom_range
            self.runtime_config.head_bloom = variant.head_bloom
            self.runtime_config.font_strength = variant.font_strength
            self.runtime_config.pipeline = variant.pipeline
            self.runtime_config.vfx_glow_strength = variant.vfx_glow_strength
            self.runtime_config.vfx_glow_radius = variant.vfx_glow_radius
            self.runtime_config.vfx_glow_threshold = variant.vfx_glow_threshold
            self.runtime_config.vfx_gamma = variant.vfx_gamma
        if not self._font_supports_katakana(self.font):
            self.runtime_config.symbols = NUMERALS + SYMBOLS + LATIN
        if self.matrix is not None:
            self.matrix.config = self.runtime_config
            self.matrix._variant_key = key
            self.matrix.resize(self.render_surface.get_size())
        if self.trace_effect is not None:
            self.trace_effect.variant = variant

    def _apply_pipeline(self, pipeline: str) -> None:
        if self.runtime_config is None:
            return
        self.runtime_config.pipeline = pipeline
        if self.matrix is not None:
            self.matrix.pipeline = pipeline
        self._configure_pipeline()

    def _configure_pipeline(self) -> None:
        if not self.runtime_config:
            return
        screen_size, flags = self._window_settings(self.span_all_displays)
        target_surface: pygame.Surface
        if self.runtime_config.pipeline == "opengl":
            try:
                self.render_surface = pygame.Surface(screen_size, pygame.SRCALPHA)
                flags |= pygame.OPENGL | pygame.DOUBLEBUF
                self.screen = pygame.display.set_mode(screen_size, flags)
                if self.opengl_pipeline is None:
                    self.opengl_pipeline = OpenGLPipeline(screen_size)
                else:
                    self.opengl_pipeline.resize(screen_size)
                target_surface = self.render_surface
            except Exception as error:
                self._log("pipeline_fallback", f"opengl failed, switching to cpu: {error}")
                self.runtime_config.pipeline = "cpu"
                flags &= ~pygame.OPENGL
                self.screen = pygame.display.set_mode(screen_size, flags)
                self.render_surface = self.screen
                self.opengl_pipeline = None
                target_surface = self.screen
        else:
            flags &= ~pygame.OPENGL
            self.screen = pygame.display.set_mode(screen_size, flags)
            self.render_surface = self.screen
            self.opengl_pipeline = None
            target_surface = self.screen
        self.menu.screen = target_surface
        if self.matrix is not None:
            self.matrix.screen = target_surface
            self.matrix.resize(self.render_surface.get_size())
        if self.trace_effect is not None:
            self.trace_effect.screen = target_surface
            self.trace_effect.resize(self.render_surface.get_size())
        if self.dev_panel is not None:
            self.dev_panel.screen = target_surface
            self.dev_panel.resize(self.render_surface.get_size())

    def _loop(self) -> None:
        assert self.matrix is not None
        assert self.trace_effect is not None
        assert self.dev_panel is not None
        running = True
        while running:
            for event in pygame.event.get():
                if event.type == pygame.QUIT:
                    self._log("quit_event", "pygame.QUIT")
                    running = False
                if event.type == pygame.KEYDOWN and event.key == pygame.K_ESCAPE:
                    self._log("quit_event", "ESC")
                    running = False
                if event.type == pygame.KEYDOWN and event.key == pygame.K_o:
                    self.dev_panel_visible = not self.dev_panel_visible
                if self.dev_panel is not None and self.dev_panel_visible:
                    self.dev_panel.handle_event(event)
                elif event.type == pygame.VIDEORESIZE:
                    self.matrix.resize(event.size)
                    self.trace_effect.resize(event.size)
                    self.menu.resize(event.size)
                    self.dev_panel.resize(event.size)
            if self.performance_mode:
                self.render_surface.fill((0, 0, 0))
            else:
                fade = pygame.Surface(self.render_surface.get_size())
                fade.set_alpha(40)
                fade.fill((0, 0, 0))
                self.render_surface.blit(fade, (0, 0))
            self.matrix.update()
            self.matrix.draw()
            self.trace_effect.update()
            self.trace_effect.draw()
            if self.dev_panel_visible:
                self.dev_panel.draw()
            if self.runtime_config and self.runtime_config.pipeline == "opengl":
                if self.opengl_pipeline is not None:
                    self.opengl_pipeline.present(self.render_surface, self.runtime_config)
            pygame.display.flip()
            self.clock.tick(30)
        self._quit()

    def _font_supports_katakana(self, font: pygame.font.Font) -> bool:
        metrics = font.metrics(KATAKANA[:1])
        if not metrics:
            return False
        return metrics[0] is not None

    def _resolve_font_path(self, font_path: str | None) -> str | None:
        if font_path and os.path.exists(font_path):
            return font_path
        candidate = os.path.join(os.getcwd(), "assets", "NotoSansCJK-Regular.ttc")
        if os.path.exists(candidate):
            return candidate
        return None

    def _quit(self) -> None:
        self._log("quit", "pygame.quit")
        pygame.quit()
        sys.exit()

    def _log(self, label: str, message: str) -> None:
        if not self.log_path:
            return
        try:
            with open(self.log_path, "a", encoding="utf-8") as handle:
                handle.write(f"{label}: {message}\n")
        except OSError:
            pass


def install_crash_logger() -> None:
    log_path = os.environ.get("MATRISAVER_CHILD_LOG")
    if not log_path:
        return

    def _handle_exception(exc_type, exc_value, exc_tb) -> None:
        try:
            with open(log_path, "w", encoding="utf-8") as handle:
                handle.write("".join(traceback.format_exception(exc_type, exc_value, exc_tb)))
        finally:
            sys.__excepthook__(exc_type, exc_value, exc_tb)

    sys.excepthook = _handle_exception
