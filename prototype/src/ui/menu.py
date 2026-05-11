"""Main menu UI."""

from __future__ import annotations

from typing import Tuple

import pygame

from config import VARIANTS


class Menu:
    def __init__(self, screen: pygame.Surface, font: pygame.font.Font) -> None:
        self.screen = screen
        self.font = font
        self.width, self.height = screen.get_size()
        self.options = list(VARIANTS.keys())
        self.selected = 0

    def resize(self, size: Tuple[int, int]) -> None:
        self.width, self.height = size

    def draw(self) -> None:
        self.screen.fill((0, 0, 0))
        title = self.font.render("Matrix Screensaver", True, (0, 255, 120))
        self.screen.blit(title, ((self.width - title.get_width()) // 2, 80))
        for index, key in enumerate(self.options):
            variant = VARIANTS[key]
            color = (0, 255, 120) if index == self.selected else (0, 140, 70)
            label = self.font.render(f"{index + 1}. {variant.name}", True, color)
            self.screen.blit(
                label,
                (self.width // 2 - label.get_width() // 2, 180 + index * 40),
            )
        hint = self.font.render("Enter to start, Esc to quit", True, (0, 120, 60))
        self.screen.blit(hint, ((self.width - hint.get_width()) // 2, self.height - 80))

    def handle_event(self, event: pygame.event.Event) -> str | None:
        if event.type == pygame.KEYDOWN:
            if event.key in (pygame.K_DOWN, pygame.K_s):
                self.selected = (self.selected + 1) % len(self.options)
            elif event.key in (pygame.K_UP, pygame.K_w):
                self.selected = (self.selected - 1) % len(self.options)
            elif event.key in (pygame.K_RETURN, pygame.K_SPACE):
                return self.options[self.selected]
        return None
