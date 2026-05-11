"""Matrix-inspired digital rain screensaver using Pygame."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from typing import List, Sequence

import pygame

from app import App, install_crash_logger
from config import VARIANTS


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Matrix-inspired digital rain screensaver")
    parser.add_argument(
        "--variant",
        choices=sorted(VARIANTS.keys()),
        help="Select the film variant.",
    )
    parser.add_argument(
        "--font",
        default=None,
        help="Path to a TTF font for the glyphs.",
    )
    parser.add_argument(
        "--single-display",
        action="store_true",
        help="Force single-display fullscreen instead of spanning all monitors.",
    )
    parser.add_argument(
        "--enable-overlay",
        action="store_true",
        help="Enable the ASCII overlay image.",
    )
    parser.add_argument(
        "--performance",
        action="store_true",
        help="Reduce visual effects for better performance.",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str]) -> None:
    args = parse_args(argv)
    install_crash_logger()
    if _launch_multi_monitor_instances(argv):
        return
    app = App(
        args.variant,
        args.font,
        span_all_displays=not args.single_display,
        overlay_enabled=args.enable_overlay,
        performance_mode=args.performance,
    )
    app.run()


def _launch_multi_monitor_instances(argv: Sequence[str]) -> bool:
    if os.environ.get("MATRISAVER_CHILD"):
        return False
    monitors = os.environ.get("MATRISAVER_MONITORS")
    if not monitors:
        return False
    rects = _parse_monitor_rects(monitors)
    if len(rects) <= 1:
        return False
    python = sys.executable
    script = str(Path(__file__).resolve())
    for index, rect in enumerate(rects):
        child_env = os.environ.copy()
        child_env["MATRISAVER_CHILD"] = "1"
        child_env.pop("MATRISAVER_MONITORS", None)
        child_env["MATRISAVER_VIRTUAL_BOUNDS"] = f"{rect.x},{rect.y},{rect.width},{rect.height}"
        child_env["MATRISAVER_WINDOW_POS"] = f"{rect.x},{rect.y}"
        child_env["MATRISAVER_CHILD_LOG"] = os.path.join(
            tempfile.gettempdir(), f"matrisaver-child-{index}.log"
        )
        subprocess.Popen([python, script, *argv], env=child_env)
    return True


def _parse_monitor_rects(value: str) -> List[pygame.Rect]:
    rects: List[pygame.Rect] = []
    for entry in value.split(";"):
        parts = [part.strip() for part in entry.split(",")]
        if len(parts) != 4:
            continue
        try:
            x, y, width, height = (int(part) for part in parts)
        except ValueError:
            continue
        rects.append(pygame.Rect(x, y, width, height))
    return rects


if __name__ == "__main__":
    main(sys.argv[1:])
