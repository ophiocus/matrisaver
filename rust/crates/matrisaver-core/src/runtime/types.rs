// Private runtime types, constants, and their impl blocks shared across all included files.

const OVERLAY_IMAGE_EXTENSIONS: [&str; 8] =
    ["png", "jpg", "jpeg", "bmp", "gif", "tga", "tiff", "webp"];
const OVERLAY_INITIAL_TRIGGER_SECONDS: f32 = 8.0;
const OVERLAY_TRIGGER_MIN_SECONDS: f32 = 15.0;
const OVERLAY_TRIGGER_RANGE_SECONDS: f32 = 15.0;
// Showcase/demo pacing (MATRISAVER_OVERLAY_FAST) — rip through a whole
// queue quickly instead of the leisurely default cadence. The active-
// hold "fast hold" constant was removed when the pre-show phase was
// dropped in v0.3.x (three-stage enforcement); the trigger pacing
// is what remains.
const OVERLAY_FAST_INITIAL_TRIGGER_SECONDS: f32 = 1.0;
const OVERLAY_FAST_TRIGGER_MIN_SECONDS: f32 = 1.5;
const OVERLAY_FAST_TRIGGER_RANGE_SECONDS: f32 = 1.5;
// Frames to wait after the painting heads finish before grabbing the
// render-to-PNG sidecar, so the bloom/persistence has settled to full
// bloom. ~0.3s at 60fps.
const OVERLAY_CAPTURE_SETTLE_FRAMES: u64 = 18;

/// Append a diagnostic overlay trace line to the path in
/// `MATRISAVER_OVERLAY_LOG`. Silent no-op when the env var is unset,
/// so the production binary pays zero cost (no file is opened).
/// Stderr-based logging (`eprintln!`) doesn't survive once the
/// screensaver window comes up — the `.scr` is GUI-subsystem and its
/// console handles detach — so traces go straight to a file the caller
/// owns. Open-write-close per call: tracing fires on lifecycle
/// transitions (a few per second at most), so the syscall overhead is
/// irrelevant and we avoid any global state / locking ceremony.
fn overlay_log(msg: &str) {
    use std::io::Write;
    let Some(path) = std::env::var_os("MATRISAVER_OVERLAY_LOG") else {
        return;
    };
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{msg}");
    }
}

// Three-stage overlay timing, expressed as multipliers on the variant's
// max rain head_speed (speed_range.1):
//
//   Stage 1 — fast INTRO: overlay painting headers sweep in to freeze
//     the silhouette. Multiplied at inject time onto speed_range.1 so
//     the silhouette assembles visibly snappier than rain.
//   Stage 2 — STAY: post-reveal dwell (`overlay_persist_seconds`,
//     handled by `overlay_dissolve_at`). Speed doesn't apply — locked.
//   Stage 3 — slow OUTRO: dissolve heads spawned at the silhouette top
//     by `dissolve_overlay_into_rain`. The lifecycle's `head_y`
//     advancement consults each column's `outro_speed_override` (set
//     at dissolve time to `head_speed * OUTRO_MULTIPLIER`, auto-cleared
//     once `head_y` crosses `outro_release_y`), so the wash drags
//     slower than rain and the silhouette ablates gracefully instead
//     of snapping out.
const OVERLAY_INTRO_SPEED_MULTIPLIER: f32 = 3.0;
const OVERLAY_OUTRO_SPEED_MULTIPLIER: f32 = 0.4;

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
    /// Sampled overlay colour (rgb), set when a painting head freezes
    /// this cell into a silhouette. Only consulted by the renderer for
    /// frozen cells under a sample-colour variant; otherwise unused.
    overlay_color: [f32; 3],
}

#[derive(Debug, Clone)]
struct OverlayTargetCell {
    row_index: usize,
    glyph_index: u32,
    brightness: f32,
    /// Colour sampled from the source image at this cell, carried so the
    /// frozen silhouette can render in the scene's own hues.
    color: [f32; 3],
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
    /// Colour sampled from the source image at this cell.
    color: [f32; 3],
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
    /// Stage 3 (slow OUTRO) override: when `Some`, the lifecycle
    /// advances `head_y` at this speed instead of `head_speed`. Set
    /// by `dissolve_overlay_into_rain` when a head is yanked to the
    /// silhouette top so the dissolve sweep moves slower than rain.
    /// Auto-cleared once `head_y` crosses `outro_release_y`.
    outro_speed_override: Option<f32>,
    /// `y` pixel threshold (inclusive) at which `outro_speed_override`
    /// auto-clears. Set by the dissolve to (silhouette-bottom + buffer)
    /// so the slow wash covers the whole figure before rain resumes.
    outro_release_y: f32,
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

/// Per-injection sampling spec for overlay cell sampling. Bundles the
/// invariant args that hold steady across every cell of one inject
/// call — the grid dimensions, the RGB→Y weights, the cover-cropped
/// window of the source image (in source-pixel coords), and whether
/// the per-pixel silhouette synthesis filter runs on each subsample —
/// so the per-cell sampler call site stays at a sane arg count.
#[derive(Debug, Clone, Copy)]
struct OverlaySamplePlan {
    grid: CellGrid,
    luma_weights: (f32, f32, f32),
    /// `(x0, y0, x1, y1)` window of the source image that maps onto the
    /// grid. For COVER, the shorter image axis fills the grid; the
    /// longer is cropped symmetrically (pad/2 each side).
    visible_rect: (f32, f32, f32, f32),
    /// When true, each subsample's (alpha, luma) is run through
    /// `synthesise_silhouette` (the in-runtime port of bane_mask.py)
    /// before being averaged into the cell value. RGB is unaffected.
    /// Set true when the source has no companion `<name>.mask.png` so
    /// user-dropped images derive their own silhouette without
    /// requiring an external pre-bake step. False when a hand-crafted
    /// mask is being read directly (its values stand on their own).
    synthesize_silhouette: bool,
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
