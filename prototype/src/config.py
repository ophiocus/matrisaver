"""Shared configuration and data models."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, List, Tuple

Color = Tuple[int, int, int]

KATAKANA = (
    "ﾊﾐﾋｰｳｼﾅﾓﾆｻﾜﾂｵﾘｱﾎﾃﾏｹﾒｴ"
    "ｶｷﾑﾕﾗｾﾈｽﾀﾇﾍﾏﾋﾗｳﾄｻﾝ"
)
NUMERALS = "0123456789"
SYMBOLS = ":・.*=+-<>¦｜/\\"
LATIN = "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
ASCII_GRADIENT = " .:-=+*#%@"

PIPELINES: Dict[str, str] = {
    "opengl": "OpenGL Shader",
    "cpu": "CPU",
    "cpu_glow": "CPU Glow",
}


@dataclass(frozen=True)
class VariantConfig:
    name: str
    color: Color
    speed_range: Tuple[int, int]
    density: float
    symbols: str
    glow_color: Color
    pause_chance: float
    jitter_chance: float
    ghost_chance: float
    ghost_swap_multiplier: float
    trail_length_multiplier: float
    volatile_chance: float
    gamma_range: Tuple[float, float]
    bloom_range: Tuple[float, float]
    head_bloom: float
    font_strength: float
    pipeline: str
    vfx_glow_strength: float
    vfx_glow_radius: float
    vfx_glow_threshold: float
    vfx_gamma: float

    def to_runtime(self, char_size: int) -> "RuntimeConfig":
        return RuntimeConfig(
            color=self.color,
            speed_range=self.speed_range,
            density=self.density,
            symbols=self.symbols,
            glow_color=self.glow_color,
            pause_chance=self.pause_chance,
            jitter_chance=self.jitter_chance,
            ghost_chance=self.ghost_chance,
            ghost_swap_multiplier=self.ghost_swap_multiplier,
            trail_length_multiplier=self.trail_length_multiplier,
            volatile_chance=self.volatile_chance,
            gamma_range=self.gamma_range,
            bloom_range=self.bloom_range,
            head_bloom=self.head_bloom,
            font_strength=self.font_strength,
            pipeline=self.pipeline,
            vfx_glow_strength=self.vfx_glow_strength,
            vfx_glow_radius=self.vfx_glow_radius,
            vfx_glow_threshold=self.vfx_glow_threshold,
            vfx_gamma=self.vfx_gamma,
            char_size=char_size,
        )


@dataclass
class RuntimeConfig:
    color: Color
    speed_range: Tuple[int, int]
    density: float
    symbols: str
    glow_color: Color
    pause_chance: float
    jitter_chance: float
    ghost_chance: float
    ghost_swap_multiplier: float
    trail_length_multiplier: float
    volatile_chance: float
    gamma_range: Tuple[float, float]
    bloom_range: Tuple[float, float]
    head_bloom: float
    font_strength: float
    pipeline: str
    vfx_glow_strength: float
    vfx_glow_radius: float
    vfx_glow_threshold: float
    vfx_gamma: float
    char_size: int


VARIANTS: Dict[str, VariantConfig] = {
    "original": VariantConfig(
        name="The Matrix (1999)",
        color=(0, 255, 70),
        speed_range=(4, 10),
        density=1.0,
        symbols=KATAKANA + SYMBOLS,
        glow_color=(180, 255, 180),
        pause_chance=0.02,
        jitter_chance=0.02,
        ghost_chance=0.12,
        ghost_swap_multiplier=10.0,
        trail_length_multiplier=3.0,
        volatile_chance=0.4,
        gamma_range=(0.9, 1.1),
        bloom_range=(0.05, 0.35),
        head_bloom=1.4,
        font_strength=1.2,
        pipeline="opengl",
        vfx_glow_strength=1.1,
        vfx_glow_radius=1.5,
        vfx_glow_threshold=0.6,
        vfx_gamma=1.1,
    ),
    "reloaded": VariantConfig(
        name="The Matrix Reloaded (2003)",
        color=(0, 255, 90),
        speed_range=(6, 14),
        density=0.9,
        symbols=KATAKANA + SYMBOLS + LATIN,
        glow_color=(200, 255, 200),
        pause_chance=0.015,
        jitter_chance=0.04,
        ghost_chance=0.15,
        ghost_swap_multiplier=10.0,
        trail_length_multiplier=1.5,
        volatile_chance=0.4,
        gamma_range=(0.7, 1.3),
        bloom_range=(0.2, 0.9),
        head_bloom=2.2,
        font_strength=1.0,
        pipeline="opengl",
        vfx_glow_strength=1.2,
        vfx_glow_radius=1.8,
        vfx_glow_threshold=0.55,
        vfx_gamma=1.1,
    ),
    "revolutions": VariantConfig(
        name="The Matrix Revolutions (2003)",
        color=(0, 230, 70),
        speed_range=(3, 16),
        density=0.75,
        symbols=KATAKANA + SYMBOLS,
        glow_color=(220, 255, 220),
        pause_chance=0.05,
        jitter_chance=0.1,
        ghost_chance=0.2,
        ghost_swap_multiplier=12.0,
        trail_length_multiplier=1.5,
        volatile_chance=0.4,
        gamma_range=(0.7, 1.3),
        bloom_range=(0.2, 0.9),
        head_bloom=2.2,
        font_strength=1.0,
        pipeline="opengl",
        vfx_glow_strength=1.2,
        vfx_glow_radius=1.8,
        vfx_glow_threshold=0.55,
        vfx_gamma=1.1,
    ),
    "resurrections": VariantConfig(
        name="The Matrix Resurrections (2021)",
        color=(0, 220, 150),
        speed_range=(5, 12),
        density=0.85,
        symbols=KATAKANA + SYMBOLS + LATIN,
        glow_color=(140, 255, 255),
        pause_chance=0.06,
        jitter_chance=0.08,
        ghost_chance=0.18,
        ghost_swap_multiplier=10.0,
        trail_length_multiplier=1.5,
        volatile_chance=0.4,
        gamma_range=(0.7, 1.3),
        bloom_range=(0.2, 0.9),
        head_bloom=2.2,
        font_strength=1.0,
        pipeline="opengl",
        vfx_glow_strength=1.2,
        vfx_glow_radius=1.8,
        vfx_glow_threshold=0.55,
        vfx_gamma=1.1,
    ),
}


@dataclass
class Column:
    x: int
    y_positions: List[float]
    speeds: List[float]
    current_speeds: List[float]
    glyphs: List[str]
    next_glyph_swap: float
    ghosts: List["Ghost"]
    original_symbols: List[str]
    original_brightness: List[float]
    original_volatile: List[bool]
    original_volatile_next: List[float]
    original_volatile_last: List[float]
    original_super_volatile: List[bool]
    original_super_opacity: List[float]
    original_gamma: List[float]
    original_bloom: List[float]
    volatile_cells: List[Tuple[int, int]]
    head_y: float
    head_speed: float
    head_opacity: float
    delete_gap: float
    last_head_row: int
    head_glyph: str
    head_row_step: int
    eraser_y: float
    eraser_speed: float
    eraser_offset: float
    eraser_glyph: str
    eraser_last_row: int
    eraser_head_above: bool


@dataclass
class Ghost:
    x: int
    y: float
    glyph: str
    next_swap: float


@dataclass
class TraceSchedule:
    next_trigger: float
    active_until: float
    message: str
