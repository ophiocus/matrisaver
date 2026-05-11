"""Trace overlay effect."""

from __future__ import annotations

import random
import time
from typing import Tuple

import pygame

from config import TraceSchedule, VariantConfig


class TraceEffect:
    def __init__(
        self,
        screen: pygame.Surface,
        font: pygame.font.Font,
        variant: VariantConfig,
    ) -> None:
        self.screen = screen
        self.font = font
        self.variant = variant
        self.width, self.height = screen.get_size()
        self.timer = self._schedule_next()

    def resize(self, size: Tuple[int, int]) -> None:
        self.width, self.height = size

    def _schedule_next(self) -> TraceSchedule:
        now = time.time()
        delay = random.uniform(60, 180)
        message = random.choice(
            [
                "TRACE INITIATED",
                "LOCATING EXIT",
                "SIGNAL TRACE",
                "LINE SEARCH",
                "ROUTING CALL",
            ]
        )
        return TraceSchedule(next_trigger=now + delay, active_until=0.0, message=message)

    def update(self) -> None:
        now = time.time()
        if self.timer.active_until and now > self.timer.active_until:
            self.timer = self._schedule_next()
        elif now > self.timer.next_trigger and not self.timer.active_until:
            self.timer.active_until = now + random.uniform(5, 9)

    def draw(self) -> None:
        if not self.timer.active_until:
            return
        overlay = pygame.Surface((self.width, self.height))
        overlay.set_alpha(190)
        overlay.fill((0, 0, 0))
        self.screen.blit(overlay, (0, 0))
        text = self.font.render(self.timer.message, True, self.variant.glow_color)
        x_pos = (self.width - text.get_width()) // 2
        y_pos = (self.height - text.get_height()) // 2
        self.screen.blit(text, (x_pos, y_pos))
        self._draw_trace_grid()

    def _draw_trace_grid(self) -> None:
        columns = 10
        rows = 6
        grid_width = self.width * 0.6
        grid_height = self.height * 0.25
        top_left_x = (self.width - grid_width) / 2
        top_left_y = (self.height - grid_height) / 2 + 60
        for column in range(columns + 1):
            x_pos = top_left_x + (grid_width / columns) * column
            pygame.draw.line(
                self.screen,
                self.variant.color,
                (x_pos, top_left_y),
                (x_pos, top_left_y + grid_height),
                1,
            )
        for row in range(rows + 1):
            y_pos = top_left_y + (grid_height / rows) * row
            pygame.draw.line(
                self.screen,
                self.variant.color,
                (top_left_x, y_pos),
                (top_left_x + grid_width, y_pos),
                1,
            )
