// Private runtime types, constants, and their impl blocks shared across all included files.

const OVERLAY_IMAGE_EXTENSIONS: [&str; 8] =
    ["png", "jpg", "jpeg", "bmp", "gif", "tga", "tiff", "webp"];
const OVERLAY_HOLD_SECONDS: f32 = 8.0;
const OVERLAY_INITIAL_TRIGGER_SECONDS: f32 = 8.0;
const OVERLAY_TRIGGER_MIN_SECONDS: f32 = 15.0;
const OVERLAY_TRIGGER_RANGE_SECONDS: f32 = 15.0;
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

#[derive(Debug, Clone, Copy)]
struct OverlayTuning {
    // CONFIG_UI: `overlay_alpha_cutoff`
    alpha_cutoff: f32,
    // CONFIG_UI: `overlay_luma_weights`
    luma_weights: (f32, f32, f32),
    // CONFIG_UI: `overlay_luma_gamma`
    gamma: f32,
    // CONFIG_UI: `overlay_contrast`
    contrast: f32,
    // CONFIG_UI: `overlay_auto_levels_low_percentile`
    levels_low_percentile: f32,
    // CONFIG_UI: `overlay_auto_levels_high_percentile`
    levels_high_percentile: f32,
    // CONFIG_UI: `overlay_brightness_floor`
    brightness_floor: f32,
    // CONFIG_UI: `overlay_brightness_scale`
    brightness_scale: f32,
    // CONFIG_UI: `overlay_header_brightness_scale`
    header_brightness_scale: f32,
    // CONFIG_UI: `overlay_intro_density_multiplier_x`
    intro_density_multiplier_x: f32,
    // CONFIG_UI: `overlay_intro_glyph_scale`
    intro_glyph_scale: f32,
    // CONFIG_UI: `overlay_intro_layer_brightness_scale`
    intro_layer_brightness_scale: f32,
    // CONFIG_UI: `overlay_denoise_mode`
    denoise_mode: OverlayDenoiseMode,
    // CONFIG_UI: `overlay_denoise_strength`
    denoise_strength: f32,
    // CONFIG_UI: `overlay_clahe_enabled`
    clahe_enabled: bool,
    // CONFIG_UI: `overlay_clahe_clip_limit`
    clahe_clip_limit: f32,
    // CONFIG_UI: `overlay_clahe_tile_grid`
    clahe_tile_grid: (u32, u32),
    // CONFIG_UI: `overlay_unsharp_enabled`
    unsharp_enabled: bool,
    // CONFIG_UI: `overlay_unsharp_amount`
    unsharp_amount: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
enum OverlayDenoiseMode {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "median")]
    Median,
    #[serde(rename = "bilateral")]
    Bilateral,
}

#[derive(Debug, Default, serde::Deserialize)]
struct OverlayTuningConfig {
    alpha_cutoff: Option<f32>,
    luma_weights: Option<[f32; 3]>,
    gamma: Option<f32>,
    contrast: Option<f32>,
    levels_low_percentile: Option<f32>,
    levels_high_percentile: Option<f32>,
    brightness_floor: Option<f32>,
    brightness_scale: Option<f32>,
    header_brightness_scale: Option<f32>,
    intro_density_multiplier_x: Option<f32>,
    intro_glyph_scale: Option<f32>,
    intro_layer_brightness_scale: Option<f32>,
    denoise_mode: Option<OverlayDenoiseMode>,
    denoise_strength: Option<f32>,
    clahe_enabled: Option<bool>,
    clahe_clip_limit: Option<f32>,
    clahe_tile_grid: Option<[u32; 2]>,
    unsharp_enabled: Option<bool>,
    unsharp_amount: Option<f32>,
}

impl OverlayTuning {
    fn with_overrides(mut self, config: OverlayTuningConfig) -> Self {
        if let Some(value) = config.alpha_cutoff {
            self.alpha_cutoff = value;
        }
        if let Some([r, g, b]) = config.luma_weights {
            self.luma_weights = (r, g, b);
        }
        if let Some(value) = config.gamma {
            self.gamma = value;
        }
        if let Some(value) = config.contrast {
            self.contrast = value;
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
        if let Some(value) = config.denoise_mode {
            self.denoise_mode = value;
        }
        if let Some(value) = config.denoise_strength {
            self.denoise_strength = value;
        }
        if let Some(value) = config.clahe_enabled {
            self.clahe_enabled = value;
        }
        if let Some(value) = config.clahe_clip_limit {
            self.clahe_clip_limit = value;
        }
        if let Some([x, y]) = config.clahe_tile_grid {
            self.clahe_tile_grid = (x, y);
        }
        if let Some(value) = config.unsharp_enabled {
            self.unsharp_enabled = value;
        }
        if let Some(value) = config.unsharp_amount {
            self.unsharp_amount = value;
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
        self.gamma = self.gamma.clamp(0.2, 3.0);
        self.contrast = self.contrast.clamp(0.2, 3.0);
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
        self.denoise_strength = self.denoise_strength.clamp(0.0, 1.0);
        self.clahe_clip_limit = self.clahe_clip_limit.clamp(0.5, 8.0);
        self.clahe_tile_grid = (
            self.clahe_tile_grid.0.clamp(1, 32),
            self.clahe_tile_grid.1.clamp(1, 32),
        );
        self.unsharp_amount = self.unsharp_amount.clamp(0.0, 2.0);
        self
    }
}

impl Default for OverlayTuning {
    fn default() -> Self {
        Self {
            alpha_cutoff: 0.03,
            luma_weights: (0.18, 0.74, 0.08),
            gamma: 1.0,
            contrast: 1.0,
            levels_low_percentile: 0.05,
            levels_high_percentile: 0.95,
            brightness_floor: 0.10,
            brightness_scale: 0.95,
            header_brightness_scale: 2.0,
            intro_density_multiplier_x: 2.0,
            intro_glyph_scale: 0.5,
            intro_layer_brightness_scale: 1.0,
            denoise_mode: OverlayDenoiseMode::None,
            denoise_strength: 0.25,
            clahe_enabled: false,
            clahe_clip_limit: 2.0,
            clahe_tile_grid: (8, 8),
            unsharp_enabled: false,
            unsharp_amount: 0.35,
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
