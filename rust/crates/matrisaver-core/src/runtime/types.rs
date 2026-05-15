// Private runtime types, constants, and their impl blocks shared across all included files.

const OVERLAY_IMAGE_EXTENSIONS: [&str; 8] =
    ["png", "jpg", "jpeg", "bmp", "gif", "tga", "tiff", "webp"];
const OVERLAY_HOLD_SECONDS: f32 = 8.0;
const OVERLAY_INITIAL_TRIGGER_SECONDS: f32 = 8.0;
const OVERLAY_TRIGGER_MIN_SECONDS: f32 = 15.0;
const OVERLAY_TRIGGER_RANGE_SECONDS: f32 = 15.0;

// Post-reveal hold: how long the painted silhouette dwells after the
// last painting head finishes its targets, before clear_overlay_locks
// fires and normal rain resumes. Without it, v0.3.x cleared locks the
// same frame the last head completed and the fully-revealed silhouette
// was visible for ~one frame only.
//
// v0.3.3 made this an admin-panel slider — the runtime reads from
// `Settings.overlay_persist_seconds` (default 15.0, range 0..120).
// No const here anymore; the named-default fn in `lib.rs::config`
// (`default_overlay_persist_seconds`) is the single source of truth
// for the default value.
const COLUMN_PITCH_SCALE: f32 = 0.5;
const OVERLAY_DENSITY_GLYPHS: &str = ".:-=+*<>¦｜/\\";

#[derive(Debug, Clone)]
struct RowCell {
    glyph_index: Option<u32>,
    brightness: f32,
    volatile: bool,
    volatile_next: f32,
    volatile_last: f32,
    super_volatile: bool,
    frozen: bool,
}

#[derive(Debug, Clone)]
struct OverlayTargetCell {
    row_index: usize,
    glyph_index: u32,
    brightness: f32,
}

#[derive(Debug, Clone)]
struct OverlayHeader {
    column_slot: u32,
    y: f32,
    speed: f32,
    glyph_index: u32,
    brightness: f32,
    next_target_index: usize,
    targets: Vec<OverlayTargetCell>,
}

#[derive(Debug, Clone)]
struct OverlayIntroGlyph {
    column_slot: u32,
    row_index: usize,
    x_offset: f32,
    glyph_index: u32,
    brightness: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum OverlayIntroMode {
    AllAtOnce,
    WaveLeftToRight,
}

/// Overlay tuning — V2.
///
/// Image-filtering fields (`denoise_*`, `clahe_*`, `unsharp_*`,
/// `gamma`, `contrast`) were dropped in v0.2.0 after research showed
/// every canonical ASCII-conversion tool (jp2a, libcaca, Paul
/// Bourke's reference) defaults to passthrough and exposes
/// adjustments only as user-controlled options. The seven-stage
/// pre-ASCII pipeline matrisaver had been running by default was
/// the outlier — it flattened silhouettes via denoise and amplified
/// rain-grid noise via unsharp.
///
/// What survives:
///
///   * **Sampling** — `alpha_cutoff` for silhouette boundary,
///     `luma_weights` for RGB→Y.
///   * **Auto-levels** — opt-in via `auto_levels_enabled`. Defensible
///     for low-contrast / clustered-histogram inputs (canonically
///     called out as appropriate for "images with clustered
///     intensity values"); harmful for already-high-contrast
///     overlays. Default off so the engine is passthrough.
///   * **Glyph emission** — `brightness_floor`, `brightness_scale`,
///     `header_brightness_scale` control how bright the emitted
///     overlay glyphs render against the rain. Typography, not
///     image processing.
///   * **Intro layer** — `intro_density_multiplier_x`,
///     `intro_glyph_scale`, `intro_layer_brightness_scale` shape the
///     sub-column ghost-glyph layer.
#[derive(Debug, Clone, Copy)]
struct OverlayTuning {
    // Sampling
    alpha_cutoff: f32,
    luma_weights: (f32, f32, f32),

    // Auto-levels (opt-in)
    auto_levels_enabled: bool,
    levels_low_percentile: f32,
    levels_high_percentile: f32,

    // Glyph emission
    brightness_floor: f32,
    brightness_scale: f32,
    header_brightness_scale: f32,

    // Intro layer typography
    intro_density_multiplier_x: f32,
    intro_glyph_scale: f32,
    intro_layer_brightness_scale: f32,
}

#[derive(Debug, Default, serde::Deserialize)]
struct OverlayTuningConfig {
    alpha_cutoff: Option<f32>,
    luma_weights: Option<[f32; 3]>,
    auto_levels_enabled: Option<bool>,
    levels_low_percentile: Option<f32>,
    levels_high_percentile: Option<f32>,
    brightness_floor: Option<f32>,
    brightness_scale: Option<f32>,
    header_brightness_scale: Option<f32>,
    intro_density_multiplier_x: Option<f32>,
    intro_glyph_scale: Option<f32>,
    intro_layer_brightness_scale: Option<f32>,
}

impl OverlayTuning {
    fn with_overrides(mut self, config: OverlayTuningConfig) -> Self {
        if let Some(value) = config.alpha_cutoff {
            self.alpha_cutoff = value;
        }
        if let Some([r, g, b]) = config.luma_weights {
            self.luma_weights = (r, g, b);
        }
        if let Some(value) = config.auto_levels_enabled {
            self.auto_levels_enabled = value;
        }
        if let Some(value) = config.levels_low_percentile {
            self.levels_low_percentile = value;
        }
        if let Some(value) = config.levels_high_percentile {
            self.levels_high_percentile = value;
        }
        if let Some(value) = config.brightness_floor {
            self.brightness_floor = value;
        }
        if let Some(value) = config.brightness_scale {
            self.brightness_scale = value;
        }
        if let Some(value) = config.header_brightness_scale {
            self.header_brightness_scale = value;
        }
        if let Some(value) = config.intro_density_multiplier_x {
            self.intro_density_multiplier_x = value;
        }
        if let Some(value) = config.intro_glyph_scale {
            self.intro_glyph_scale = value;
        }
        if let Some(value) = config.intro_layer_brightness_scale {
            self.intro_layer_brightness_scale = value;
        }

        self.sanitize()
    }

    fn sanitize(mut self) -> Self {
        self.alpha_cutoff = self.alpha_cutoff.clamp(0.0, 1.0);
        let (r, g, b) = self.luma_weights;
        let sum = r + g + b;
        if !sum.is_finite() || sum <= f32::EPSILON {
            self.luma_weights = (0.2126, 0.7152, 0.0722);
        }
        self.levels_low_percentile = self.levels_low_percentile.clamp(0.0, 1.0);
        self.levels_high_percentile = self.levels_high_percentile.clamp(0.0, 1.0);
        if self.levels_low_percentile >= self.levels_high_percentile {
            self.levels_low_percentile = 0.05;
            self.levels_high_percentile = 0.95;
        }
        self.brightness_floor = self.brightness_floor.clamp(0.0, 1.0);
        self.brightness_scale = self.brightness_scale.clamp(0.0, 2.0);
        self.header_brightness_scale = self.header_brightness_scale.clamp(0.0, 4.0);
        self.intro_density_multiplier_x = self.intro_density_multiplier_x.clamp(1.0, 4.0);
        self.intro_glyph_scale = self.intro_glyph_scale.clamp(0.25, 1.0);
        self.intro_layer_brightness_scale = self.intro_layer_brightness_scale.clamp(0.0, 2.0);
        self
    }
}

impl Default for OverlayTuning {
    fn default() -> Self {
        Self {
            alpha_cutoff: 0.03,
            luma_weights: (0.18, 0.74, 0.08),
            auto_levels_enabled: false,
            levels_low_percentile: 0.05,
            levels_high_percentile: 0.95,
            brightness_floor: 0.10,
            brightness_scale: 0.95,
            header_brightness_scale: 2.0,
            intro_density_multiplier_x: 2.0,
            intro_glyph_scale: 0.5,
            intro_layer_brightness_scale: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
struct GhostGlyph {
    row: f32,
    glyph_index: u32,
    next_swap_at: f32,
}

#[derive(Debug, Clone)]
struct RainColumn {
    column_slot: u32,
    y_positions: Vec<f32>,
    speeds: Vec<f32>,
    current_speeds: Vec<f32>,
    glyph_indices: Vec<u32>,
    next_glyph_swap_at: f32,
    head_y: f32,
    head_speed: f32,
    glyph_cursor: u32,
    head_glyph_index: u32,
    delete_gap: f32,
    last_head_row: i32,
    head_row_step: u8,
    eraser_y: f32,
    eraser_speed: f32,
    eraser_offset: f32,
    eraser_last_row: i32,
    head_reset_count: u64,
    eraser_reset_count: u64,
    head_write_count: u64,
    chain_reset_count: u64,
    glyph_swap_count: u64,
    row_cells: Vec<RowCell>,
    ghosts: Vec<GhostGlyph>,
}

#[derive(Debug, Clone, Copy)]
struct OriginalLifecycleMutators {
    head_speed_scale: f32,
    eraser_speed_scale: f32,
    eraser_offset_scale: f32,
    delete_gap_scale: f32,
    volatile_chance_bias: f32,
    ghost_chance_bias: f32,
    extra_row_write_chance: f32,
    volatile_interval_scale: f32,
    super_volatile_bonus: f32,
}

impl Default for OriginalLifecycleMutators {
    fn default() -> Self {
        Self {
            head_speed_scale: 1.0,
            eraser_speed_scale: 1.0,
            eraser_offset_scale: 1.0,
            delete_gap_scale: 1.0,
            volatile_chance_bias: 0.0,
            ghost_chance_bias: 0.0,
            extra_row_write_chance: 0.0,
            volatile_interval_scale: 1.0,
            super_volatile_bonus: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct CellGrid {
    cols: u32,
    rows: u32,
}

#[derive(Debug, Clone, Copy)]
struct LifecycleTickContext {
    now: f32,
    frame_dt: f32,
    rows: u32,
    char_size: u32,
    volatile_chance: f32,
    ghost_chance: f32,
    ghost_swap_multiplier: f32,
    trail_length_multiplier: f32,
    super_volatile_pulse_time: Option<f32>,
}
