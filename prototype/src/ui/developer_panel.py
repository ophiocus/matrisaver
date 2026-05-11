"""Developer options panel."""

from __future__ import annotations

from dataclasses import dataclass
from typing import List, Tuple

import pygame

from config import PIPELINES, RuntimeConfig, VARIANTS


@dataclass
class ControlField:
    key: str
    label: str
    min_value: float
    max_value: float
    step: float
    formatter: str
    choices: List[str] | None = None


class DeveloperPanel:
    def __init__(
        self,
        screen: pygame.Surface,
        font: pygame.font.Font,
        config: RuntimeConfig,
        on_close: callable,
        variant_key: str,
        on_variant_change: callable,
        on_pipeline_change: callable,
    ) -> None:
        self.screen = screen
        self.font = font
        self.config = config
        self.on_close = on_close
        self.variant_key = variant_key
        self.on_variant_change = on_variant_change
        self.on_pipeline_change = on_pipeline_change
        self.width, self.height = screen.get_size()
        self.fields = [
            ControlField("variant", "Variant", 0, 0, 1, "{}", list(VARIANTS.keys())),
            ControlField("char_size", "Char Size", 12, 36, 2, "{:.0f}"),
            ControlField("density", "Density", 0.3, 1.0, 0.05, "{:.2f}"),
            ControlField("speed_min", "Speed Min", 1, 20, 1, "{:.0f}"),
            ControlField("speed_max", "Speed Max", 2, 28, 1, "{:.0f}"),
            ControlField("pause", "Pause Chance", 0.0, 0.2, 0.01, "{:.2f}"),
            ControlField("jitter", "Jitter Chance", 0.0, 0.2, 0.01, "{:.2f}"),
            ControlField("ghost_chance", "Ghost Chance", 0.0, 0.4, 0.02, "{:.2f}"),
            ControlField("ghost_swap", "Ghost Swap Mult", 2.0, 20.0, 1.0, "{:.0f}"),
            ControlField("pipeline", "Pipeline", 0, 0, 1, "{}", list(PIPELINES.keys())),
            ControlField("vfx_glow_strength", "VFX Glow", 0.0, 3.0, 0.1, "{:.1f}"),
            ControlField("vfx_glow_radius", "VFX Radius", 0.0, 6.0, 0.2, "{:.1f}"),
            ControlField("vfx_glow_threshold", "VFX Threshold", 0.0, 1.0, 0.05, "{:.2f}"),
            ControlField("vfx_gamma", "VFX Gamma", 0.6, 2.2, 0.1, "{:.1f}"),
            ControlField("trail_multiplier", "Trail Length", 0.5, 50.0, 0.5, "{:.1f}"),
            ControlField("volatile_chance", "Volatile Chance", 0.0, 1.0, 0.05, "{:.2f}"),
            ControlField("gamma_min", "Gamma Min", 0.4, 2.0, 0.05, "{:.2f}"),
            ControlField("gamma_max", "Gamma Max", 0.4, 2.0, 0.05, "{:.2f}"),
            ControlField("bloom_min", "Bloom Min", 0.0, 2.0, 0.05, "{:.2f}"),
            ControlField("bloom_max", "Bloom Max", 0.0, 2.0, 0.05, "{:.2f}"),
            ControlField("head_bloom", "Head Bloom", 0.8, 3.0, 0.1, "{:.1f}"),
            ControlField("font_strength", "Font Strength", 1.0, 4.0, 0.5, "{:.1f}"),
        ]
        self.fields_per_page = 10
        self.page_index = 0
        self.selected = 0
        self.panel_rect = pygame.Rect(0, 0, 0, 0)
        self.line_height = 40
        self._update_layout()

    def resize(self, size: Tuple[int, int]) -> None:
        self.width, self.height = size
        self._update_layout()

    def _update_layout(self) -> None:
        margin_x = max(40, int(self.width * 0.08))
        margin_y = max(40, int(self.height * 0.08))
        panel_width = max(320, self.width - margin_x * 2)
        panel_height = max(320, self.height - margin_y * 2)
        panel_width = min(panel_width, self.width - 20)
        panel_height = min(panel_height, self.height - 20)
        self.panel_rect = pygame.Rect(
            (self.width - panel_width) // 2,
            (self.height - panel_height) // 2,
            panel_width,
            panel_height,
        )
        visible_count = min(self.fields_per_page, len(self.fields))
        self.line_height = max(28, int(panel_height / (visible_count + 4)))

    def handle_event(self, event: pygame.event.Event) -> None:
        if event.type == pygame.KEYDOWN:
            if event.key == pygame.K_ESCAPE:
                self.on_close()
                return
            if event.key in (pygame.K_TAB, pygame.K_DOWN):
                self._move_selection(1)
            elif event.key == pygame.K_UP:
                self._move_selection(-1)
            elif event.key == pygame.K_PAGEUP:
                self._change_page(-1)
            elif event.key == pygame.K_PAGEDOWN:
                self._change_page(1)
            elif event.key in (pygame.K_LEFT, pygame.K_a):
                self._adjust_selected(-1)
            elif event.key in (pygame.K_RIGHT, pygame.K_d):
                self._adjust_selected(1)
        elif event.type == pygame.MOUSEBUTTONDOWN:
            if not self.panel_rect.collidepoint(event.pos):
                self.on_close()
                return
            self._select_from_mouse(event.pos)

    def _select_from_mouse(self, position: Tuple[int, int]) -> None:
        start_y = self.panel_rect.top + self.line_height * 2
        for row_offset, (index, _field) in enumerate(self._visible_fields()):
            row_rect = pygame.Rect(
                self.panel_rect.left + 40,
                start_y + row_offset * self.line_height - 10,
                self.panel_rect.width - 80,
                self.line_height,
            )
            if row_rect.collidepoint(position):
                self._set_selected(index)
                return

    def _page_count(self) -> int:
        return max(1, (len(self.fields) + self.fields_per_page - 1) // self.fields_per_page)

    def _page_bounds(self) -> Tuple[int, int]:
        start = self.page_index * self.fields_per_page
        end = min(len(self.fields), start + self.fields_per_page)
        return start, end

    def _visible_fields(self) -> List[Tuple[int, ControlField]]:
        start, end = self._page_bounds()
        return list(enumerate(self.fields[start:end], start))

    def _set_selected(self, index: int) -> None:
        if not self.fields:
            return
        self.selected = index % len(self.fields)
        self.page_index = self.selected // self.fields_per_page

    def _move_selection(self, direction: int) -> None:
        if not self.fields:
            return
        self._set_selected(self.selected + direction)

    def _change_page(self, direction: int) -> None:
        page_count = self._page_count()
        if page_count <= 1:
            return
        self.page_index = (self.page_index + direction) % page_count
        start, end = self._page_bounds()
        if self.selected < start or self.selected >= end:
            self.selected = start

    def _adjust_selected(self, direction: int) -> None:
        field = self.fields[self.selected]
        if field.choices:
            if field.key == "variant":
                current_index = field.choices.index(self.variant_key)
                next_index = (current_index + direction) % len(field.choices)
                self.variant_key = field.choices[next_index]
                self.on_variant_change(self.variant_key)
            elif field.key == "pipeline":
                current_index = field.choices.index(self.config.pipeline)
                next_index = (current_index + direction) % len(field.choices)
                self.config.pipeline = field.choices[next_index]
                self.on_pipeline_change(self.config.pipeline)
            return
        value = self._get_value(field.key)
        value += field.step * direction
        value = max(field.min_value, min(field.max_value, value))
        self._set_value(field.key, value)

    def _get_value(self, key: str) -> float:
        if key == "variant":
            return 0.0
        if key == "char_size":
            return float(self.config.char_size)
        if key == "density":
            return float(self.config.density)
        if key == "speed_min":
            return float(self.config.speed_range[0])
        if key == "speed_max":
            return float(self.config.speed_range[1])
        if key == "pause":
            return float(self.config.pause_chance)
        if key == "jitter":
            return float(self.config.jitter_chance)
        if key == "ghost_chance":
            return float(self.config.ghost_chance)
        if key == "ghost_swap":
            return float(self.config.ghost_swap_multiplier)
        if key == "pipeline":
            return 0.0
        if key == "vfx_glow_strength":
            return float(self.config.vfx_glow_strength)
        if key == "vfx_glow_radius":
            return float(self.config.vfx_glow_radius)
        if key == "vfx_glow_threshold":
            return float(self.config.vfx_glow_threshold)
        if key == "vfx_gamma":
            return float(self.config.vfx_gamma)
        if key == "trail_multiplier":
            return float(self.config.trail_length_multiplier)
        if key == "volatile_chance":
            return float(self.config.volatile_chance)
        if key == "gamma_min":
            return float(self.config.gamma_range[0])
        if key == "gamma_max":
            return float(self.config.gamma_range[1])
        if key == "bloom_min":
            return float(self.config.bloom_range[0])
        if key == "bloom_max":
            return float(self.config.bloom_range[1])
        if key == "head_bloom":
            return float(self.config.head_bloom)
        if key == "font_strength":
            return float(self.config.font_strength)
        return 0.0

    def _set_value(self, key: str, value: float) -> None:
        if key == "char_size":
            self.config.char_size = int(value)
        elif key == "density":
            self.config.density = value
        elif key == "speed_min":
            self.config.speed_range = (int(value), self.config.speed_range[1])
        elif key == "speed_max":
            self.config.speed_range = (self.config.speed_range[0], int(value))
        elif key == "pause":
            self.config.pause_chance = value
        elif key == "jitter":
            self.config.jitter_chance = value
        elif key == "ghost_chance":
            self.config.ghost_chance = value
        elif key == "ghost_swap":
            self.config.ghost_swap_multiplier = value
        elif key == "pipeline":
            return
        elif key == "vfx_glow_strength":
            self.config.vfx_glow_strength = value
        elif key == "vfx_glow_radius":
            self.config.vfx_glow_radius = value
        elif key == "vfx_glow_threshold":
            self.config.vfx_glow_threshold = value
        elif key == "vfx_gamma":
            self.config.vfx_gamma = value
        elif key == "trail_multiplier":
            self.config.trail_length_multiplier = value
        elif key == "volatile_chance":
            self.config.volatile_chance = value
        elif key == "gamma_min":
            self.config.gamma_range = (value, max(value, self.config.gamma_range[1]))
        elif key == "gamma_max":
            self.config.gamma_range = (min(value, self.config.gamma_range[0]), value)
        elif key == "bloom_min":
            self.config.bloom_range = (value, max(value, self.config.bloom_range[1]))
        elif key == "bloom_max":
            self.config.bloom_range = (min(value, self.config.bloom_range[0]), value)
        elif key == "head_bloom":
            self.config.head_bloom = value
        elif key == "font_strength":
            self.config.font_strength = value

    def draw(self) -> None:
        overlay = pygame.Surface((self.width, self.height))
        overlay.set_alpha(200)
        overlay.fill((0, 0, 0))
        self.screen.blit(overlay, (0, 0))
        pygame.draw.rect(self.screen, (0, 40, 20), self.panel_rect)
        pygame.draw.rect(self.screen, (0, 180, 90), self.panel_rect, 2)

        title = self.font.render("Developer Controls", True, (0, 255, 140))
        self.screen.blit(
            title,
            (self.panel_rect.centerx - title.get_width() // 2, self.panel_rect.top + 24),
        )

        start_y = self.panel_rect.top + self.line_height * 2
        for row_offset, (index, field) in enumerate(self._visible_fields()):
            if field.key == "variant":
                label = f"{field.label}: {VARIANTS[self.variant_key].name}"
            elif field.key == "pipeline":
                label = f"{field.label}: {PIPELINES.get(self.config.pipeline, 'CPU')}"
            else:
                value = self._get_value(field.key)
                value_text = field.formatter.format(value)
                label = f"{field.label}: {value_text}"
            color = (0, 255, 140) if index == self.selected else (0, 160, 80)
            rendered = self.font.render(label, True, color)
            self.screen.blit(
                rendered,
                (self.panel_rect.left + 40, start_y + row_offset * self.line_height),
            )

        self._draw_page_dots()

        help_text = self.font.render(
            "Arrows adjust, PgUp/PgDn switch pages, Esc closes", True, (0, 120, 60)
        )
        self.screen.blit(
            help_text,
            (
                self.panel_rect.centerx - help_text.get_width() // 2,
                self.panel_rect.bottom - 40,
            ),
        )

    def _draw_page_dots(self) -> None:
        page_count = self._page_count()
        if page_count <= 0:
            return
        dot_spacing = 18
        active_radius = 7
        inactive_radius = 4
        total_width = (page_count - 1) * dot_spacing + active_radius * 2
        start_x = self.panel_rect.centerx - total_width / 2
        y_pos = self.panel_rect.bottom - 68
        for index in range(page_count):
            radius = active_radius if index == self.page_index else inactive_radius
            color = (0, 255, 140) if index == self.page_index else (0, 120, 60)
            x_pos = int(start_x + index * dot_spacing)
            pygame.draw.circle(self.screen, color, (x_pos, int(y_pos)), radius)
