//! Shared runtime abstractions for all MatriSaver platform hosts.

pub mod config {
    use serde::{Deserialize, Serialize};

    pub type Color = (u8, u8, u8);

    pub const KATAKANA: &str = "ﾊﾐﾋｰｳｼﾅﾓﾆｻﾜﾂｵﾘｱﾎﾃﾏｹﾒｴｶｷﾑﾕﾗｾﾈｽﾀﾇﾍﾏﾋﾗｳﾄｻﾝ";
    pub const NUMERALS: &str = "0123456789";
    pub const SYMBOLS: &str = ":・.*=+-<>¦｜/\\";
    pub const LATIN: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    pub const ASCII_GRADIENT: &str = " .:-=+*#%@";

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SymbolSet {
        KatakanaSymbols,
        KatakanaSymbolsLatin,
    }

    impl SymbolSet {
        pub fn materialize(self) -> String {
            match self {
                Self::KatakanaSymbols => [KATAKANA, SYMBOLS].join(""),
                Self::KatakanaSymbolsLatin => [KATAKANA, SYMBOLS, LATIN].join(""),
            }
        }
    }

    /// Rendering pipeline selection.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum Pipeline {
        #[serde(rename = "opengl")]
        OpenGl,
        #[serde(rename = "cpu")]
        Cpu,
        #[serde(rename = "cpu_glow")]
        CpuGlow,
    }

    impl Pipeline {
        pub fn key(self) -> &'static str {
            match self {
                Self::OpenGl => "opengl",
                Self::Cpu => "cpu",
                Self::CpuGlow => "cpu_glow",
            }
        }

        pub fn label(self) -> &'static str {
            match self {
                Self::OpenGl => "OpenGL Shader",
                Self::Cpu => "CPU",
                Self::CpuGlow => "CPU Glow",
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum GlowQuality {
        #[serde(rename = "low")]
        Low,
        #[serde(rename = "balanced")]
        Balanced,
        #[serde(rename = "high")]
        High,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct VariantConfig {
        pub key: &'static str,
        pub name: &'static str,
        pub color: Color,
        /// Optional second tint for overlay-painted glyphs (silhouette
        /// intro, painting headers, frozen locked cells). `None` means
        /// "use the field colour" — overlay glyphs render the same as
        /// the rain, matching pre-v0.3.4 behaviour. `Some(rgb)` recolours
        /// just the overlay (the `bane` variant uses crimson so the
        /// painted silhouette reads red over the dim green field).
        pub overlay_tint: Option<Color>,
        /// Optional variant-pinned overlay subdirectory (relative to the
        /// overlays root). `Some("bane")` makes the variant draw ONLY
        /// from `assets/overlays/bane/` (or the ProgramData equivalent),
        /// ignoring the default overlay pack and any user-configured
        /// directories — so the film-specific silhouette is the only
        /// thing painted. `None` uses the normal overlay resolution chain.
        pub overlay_subdir: Option<&'static str>,
        /// Paint the overlay silhouette at full `char_size` column pitch
        /// instead of the default packed pitch. The overlay normally
        /// fills every rain column-slot (`column_span = 1/COLUMN_PITCH_SCALE`),
        /// so silhouette glyphs sit at half-char pitch and overlap into a
        /// dense fine mat — fine for the soft default image pack, but it
        /// makes a bold silhouette like `bane` read as tiny next to the
        /// sparse full-size rain. `true` drops the span to 1 so each
        /// painted glyph stands alone at the same visual weight as the
        /// rain. `false` keeps the dense default.
        pub overlay_full_pitch: bool,
        /// Skip the "dim pre-show": the post-injection active-hold window
        /// that displays a dim full-silhouette preview (the intro ghost
        /// layer) before the painting heads sweep in. `true` jumps
        /// straight to painting, so the silhouette is revealed only as
        /// the heads paint and freeze it — the lifecycle becomes
        /// rain → paint-in → freeze/hold → release → rain washes back.
        /// `false` keeps the preview the default image pack was tuned for.
        pub overlay_skip_preview: bool,
        /// Colour overlay glyphs by sampling the source image's hue per
        /// cell, instead of a single flat `overlay_tint`. `true` makes the
        /// painted code carry the scene's own colours (the film variants
        /// want this for their iconic-scene overlays). `false` uses the
        /// flat `overlay_tint` — bane keeps its fixed crimson.
        pub overlay_sample_color: bool,
        pub speed_range: (u8, u8),
        pub density: f32,
        pub symbol_set: SymbolSet,
        pub glow_color: Color,
        pub pause_chance: f32,
        pub jitter_chance: f32,
        pub ghost_chance: f32,
        pub ghost_swap_multiplier: f32,
        pub trail_length_multiplier: f32,
        pub volatile_chance: f32,
        pub gamma_range: (f32, f32),
        pub bloom_range: (f32, f32),
        pub head_bloom: f32,
        pub font_strength: f32,
        pub pipeline: Pipeline,
        pub vfx_glow_strength: f32,
        pub vfx_glow_radius: f32,
        pub vfx_glow_threshold: f32,
        pub vfx_gamma: f32,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct RuntimeConfig {
        pub color: Color,
        /// Resolved overlay tint (always concrete — `VariantConfig`'s
        /// `None` collapses to `color` here, so the renderer can read it
        /// unconditionally). Equal to `color` for every variant except
        /// `bane`, which carries crimson.
        pub overlay_tint: Color,
        /// Variant-pinned overlay subdirectory, propagated from
        /// `VariantConfig`. `Some("bane")` = use only that overlay dir.
        pub overlay_subdir: Option<&'static str>,
        /// Paint the silhouette at full char-size pitch (no column-slot
        /// packing), propagated from `VariantConfig`. `true` for `bane`.
        pub overlay_full_pitch: bool,
        /// Skip the dim pre-show active-hold window, propagated from
        /// `VariantConfig`. `true` for `bane`.
        pub overlay_skip_preview: bool,
        /// Sample overlay glyph colour from the source image per cell,
        /// propagated from `VariantConfig`. `true` for the film variants.
        pub overlay_sample_color: bool,
        pub speed_range: (u8, u8),
        pub density: f32,
        pub symbols: String,
        pub glow_color: Color,
        pub pause_chance: f32,
        pub jitter_chance: f32,
        pub ghost_chance: f32,
        pub ghost_swap_multiplier: f32,
        pub trail_length_multiplier: f32,
        pub volatile_chance: f32,
        pub gamma_range: (f32, f32),
        pub bloom_range: (f32, f32),
        pub head_bloom: f32,
        pub font_strength: f32,
        pub pipeline: Pipeline,
        pub vfx_glow_strength: f32,
        pub vfx_glow_radius: f32,
        pub vfx_glow_threshold: f32,
        pub vfx_gamma: f32,
        pub char_size: u16,
    }

    impl VariantConfig {
        pub fn to_runtime(self, char_size: u16) -> RuntimeConfig {
            RuntimeConfig {
                color: self.color,
                overlay_tint: self.overlay_tint.unwrap_or(self.color),
                overlay_subdir: self.overlay_subdir,
                overlay_full_pitch: self.overlay_full_pitch,
                overlay_skip_preview: self.overlay_skip_preview,
                overlay_sample_color: self.overlay_sample_color,
                speed_range: self.speed_range,
                density: self.density,
                symbols: self.symbol_set.materialize(),
                glow_color: self.glow_color,
                pause_chance: self.pause_chance,
                jitter_chance: self.jitter_chance,
                ghost_chance: self.ghost_chance,
                ghost_swap_multiplier: self.ghost_swap_multiplier,
                trail_length_multiplier: self.trail_length_multiplier,
                volatile_chance: self.volatile_chance,
                gamma_range: self.gamma_range,
                bloom_range: self.bloom_range,
                head_bloom: self.head_bloom,
                font_strength: self.font_strength,
                pipeline: self.pipeline,
                vfx_glow_strength: self.vfx_glow_strength,
                vfx_glow_radius: self.vfx_glow_radius,
                vfx_glow_threshold: self.vfx_glow_threshold,
                vfx_gamma: self.vfx_gamma,
                char_size,
            }
        }
    }

    impl RuntimeConfig {
        pub fn sanitize(&mut self) {
            self.density = self.density.clamp(0.3, 1.0);
            if self.speed_range.0 > self.speed_range.1 {
                self.speed_range = (self.speed_range.1, self.speed_range.0);
            }
            self.trail_length_multiplier = self.trail_length_multiplier.max(0.5);
            if self.gamma_range.0 > self.gamma_range.1 {
                self.gamma_range = (self.gamma_range.1, self.gamma_range.0);
            }
            if self.bloom_range.0 > self.bloom_range.1 {
                self.bloom_range = (self.bloom_range.1, self.bloom_range.0);
            }
            self.char_size = self.char_size.clamp(8, 96);
        }
    }

    pub const VARIANTS: [VariantConfig; 5] = [
        VariantConfig {
            key: "original",
            // 1999 green calibration: a small red lift warms the pure
            // (0,255,70) emerald toward the "institutional fluorescent /
            // sickly digital" green Bill Pope graded the in-Matrix world
            // to — and the blue drop kills the spring-green tint that was
            // leaning too modern. Conservative on purpose; the heavily-
            // pushed DVD "puke green" is the failure mode to avoid.
            name: "The Matrix (1999)",
            color: (35, 235, 65),
            overlay_tint: None,
            // Each variant leads its overlay queue with its own film's
            // iconic-scene set under assets/overlays/<key>/. Empty/absent
            // dirs fall back to the shared pack, so this is a no-op until
            // the per-film art is dropped in.
            overlay_subdir: Some("original"),
            overlay_full_pitch: false,
            overlay_skip_preview: false,
            overlay_sample_color: true,
            speed_range: (4, 10),
            density: 1.0,
            symbol_set: SymbolSet::KatakanaSymbols,
            // Softer, greener glow — less blown-white than the old
            // (180,255,180), so the bloom reads as phosphor halo
            // rather than a white wash.
            glow_color: (120, 235, 140),
            pause_chance: 0.02,
            jitter_chance: 0.02,
            ghost_chance: 0.12,
            ghost_swap_multiplier: 10.0,
            trail_length_multiplier: 3.0,
            volatile_chance: 0.4,
            gamma_range: (0.9, 1.1),
            bloom_range: (0.05, 0.35),
            head_bloom: 1.4,
            font_strength: 1.2,
            pipeline: Pipeline::OpenGl,
            vfx_glow_strength: 1.1,
            vfx_glow_radius: 1.5,
            vfx_glow_threshold: 0.6,
            vfx_gamma: 1.1,
        },
        VariantConfig {
            key: "reloaded",
            name: "The Matrix Reloaded (2003)",
            color: (0, 255, 90),
            overlay_tint: None,
            overlay_subdir: Some("reloaded"),
            overlay_full_pitch: false,
            overlay_skip_preview: false,
            overlay_sample_color: true,
            speed_range: (6, 14),
            density: 0.9,
            symbol_set: SymbolSet::KatakanaSymbolsLatin,
            glow_color: (200, 255, 200),
            pause_chance: 0.015,
            jitter_chance: 0.04,
            ghost_chance: 0.15,
            ghost_swap_multiplier: 10.0,
            trail_length_multiplier: 1.5,
            volatile_chance: 0.4,
            gamma_range: (0.7, 1.3),
            bloom_range: (0.2, 0.9),
            head_bloom: 2.2,
            font_strength: 1.0,
            pipeline: Pipeline::OpenGl,
            vfx_glow_strength: 1.2,
            vfx_glow_radius: 1.8,
            vfx_glow_threshold: 0.55,
            vfx_gamma: 1.1,
        },
        VariantConfig {
            key: "revolutions",
            name: "The Matrix Revolutions (2003)",
            color: (0, 230, 70),
            overlay_tint: None,
            overlay_subdir: Some("revolutions"),
            overlay_full_pitch: false,
            overlay_skip_preview: false,
            overlay_sample_color: true,
            speed_range: (3, 16),
            density: 0.75,
            symbol_set: SymbolSet::KatakanaSymbols,
            glow_color: (220, 255, 220),
            pause_chance: 0.05,
            jitter_chance: 0.1,
            ghost_chance: 0.2,
            ghost_swap_multiplier: 12.0,
            trail_length_multiplier: 1.5,
            volatile_chance: 0.4,
            gamma_range: (0.7, 1.3),
            bloom_range: (0.2, 0.9),
            head_bloom: 2.2,
            font_strength: 1.0,
            pipeline: Pipeline::OpenGl,
            vfx_glow_strength: 1.2,
            vfx_glow_radius: 1.8,
            vfx_glow_threshold: 0.55,
            vfx_gamma: 1.1,
        },
        VariantConfig {
            key: "resurrections",
            name: "The Matrix Resurrections (2021)",
            color: (0, 220, 150),
            overlay_tint: None,
            overlay_subdir: Some("resurrections"),
            overlay_full_pitch: false,
            overlay_skip_preview: false,
            overlay_sample_color: true,
            speed_range: (5, 12),
            density: 0.85,
            symbol_set: SymbolSet::KatakanaSymbolsLatin,
            glow_color: (140, 255, 255),
            pause_chance: 0.06,
            jitter_chance: 0.08,
            ghost_chance: 0.18,
            ghost_swap_multiplier: 10.0,
            trail_length_multiplier: 1.5,
            volatile_chance: 0.4,
            gamma_range: (0.7, 1.3),
            bloom_range: (0.2, 0.9),
            head_bloom: 2.2,
            font_strength: 1.0,
            pipeline: Pipeline::OpenGl,
            vfx_glow_strength: 1.2,
            vfx_glow_radius: 1.8,
            vfx_glow_threshold: 0.55,
            vfx_gamma: 1.1,
        },
        VariantConfig {
            // Revolutions Bane-defeat (registry performance #6). This
            // entry is ONLY the background field: a faint, sparse, dim
            // green rain. The crimson Bane silhouette is layered on top
            // at runtime via the overlay path + a second red-tinted
            // draw pass (approach A) — it is NOT encoded in this table.
            // Reference frames are pure black with the figure as the
            // sole luminous element; the dim field + low density keep
            // the green from competing with the red hero.
            key: "bane",
            name: "Revolutions — Bane (2003)",
            // Dim emerald: well below the 1999 (35,235,65) and
            // Revolutions (0,230,70) head greens so the field reads as
            // a quiet substrate, not a feature.
            color: (12, 130, 45),
            // Crimson overlay — the painted Bane silhouette (intro
            // glyphs, headers, frozen locked cells) renders red over the
            // dim green field. Saturated red with a hair of green so it
            // doesn't clip to pure (255,0,0); the head-HDR boost lifts the
            // hot cores well above 1.0 so the mip-chain bloom blooms it.
            overlay_tint: Some((255, 18, 14)),
            // Draw only the Bane silhouette mask, not the default pack.
            overlay_subdir: Some("bane"),
            // Paint the silhouette at full char-size pitch so the code
            // forming Bane reads at the same scale as the rain, instead
            // of a dense half-pitch mat.
            overlay_full_pitch: true,
            // No dim preview — the silhouette appears only as the heads
            // paint it in over the rain.
            overlay_skip_preview: true,
            // Bane keeps its fixed crimson tint, not image-sampled colour.
            overlay_sample_color: false,
            speed_range: (3, 11),
            // Sparse field. 0.5 sits clearly below every other variant
            // and above the 0.3 sanitize floor.
            density: 0.5,
            symbol_set: SymbolSet::KatakanaSymbols,
            // Dim glow to match — the bright halo budget is reserved
            // for the red overlay glyphs, not the green field.
            glow_color: (90, 200, 110),
            pause_chance: 0.05,
            jitter_chance: 0.08,
            ghost_chance: 0.18,
            ghost_swap_multiplier: 11.0,
            trail_length_multiplier: 1.5,
            volatile_chance: 0.4,
            gamma_range: (0.7, 1.3),
            bloom_range: (0.2, 0.9),
            head_bloom: 2.0,
            font_strength: 1.0,
            pipeline: Pipeline::OpenGl,
            vfx_glow_strength: 1.2,
            vfx_glow_radius: 1.8,
            vfx_glow_threshold: 0.55,
            vfx_gamma: 1.1,
        },
    ];

    pub fn variant_by_key(key: &str) -> Option<&'static VariantConfig> {
        VARIANTS.iter().find(|variant| variant.key == key)
    }

    /// Persisted runtime settings shared across host integrations.
    /// One user-supplied or install-shipped directory of overlay
    /// images. Multiple of these compose the lookup chain; the engine
    /// walks them in declaration order and dedupes by filename.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct OverlaySource {
        pub path: std::path::PathBuf,
        #[serde(default = "default_true")]
        pub enabled: bool,
        /// When true and the directory is writable, the engine writes
        /// the per-image ASCII glyph grid alongside each source image
        /// as `<image>.<extension>.ascii.txt` after each injection.
        /// Idempotent permission probe per session — silently skipped
        /// on read-only directories.
        #[serde(default)]
        pub write_ascii_alongside: bool,
    }

    fn default_true() -> bool {
        true
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Settings {
        pub variant: String,
        pub pipeline: Pipeline,
        #[serde(default = "default_glow_quality")]
        pub glow_quality: GlowQuality,
        pub overlay_enabled: bool,
        pub performance_mode: bool,
        pub multi_monitor: bool,
        pub char_size: u16,
        /// Ordered list of overlay-image directories. Earlier entries
        /// take priority over later ones when filenames collide. An
        /// empty list falls back to the legacy resolution chain
        /// (`MATRISAVER_OVERLAY_DIR` env var, ancestor walks of the
        /// exe / cwd, then `CARGO_MANIFEST_DIR` at compile time).
        #[serde(default)]
        pub overlay_directories: Vec<OverlaySource>,
        /// Opt-in contrast-stretch before ASCII glyph mapping.
        /// Defensible for low-contrast input photos; off by default
        /// (matrisaver V2 defaults to passthrough per canonical
        /// ASCII-conversion practice).
        #[serde(default)]
        pub overlay_auto_levels: bool,
        /// User toggle for sampling overlay glyph colour from the source
        /// image's natural hues. ANDed with each variant's
        /// `overlay_sample_color` capability: when off, colour-capable
        /// variants (the films) fall back to their flat field tint;
        /// fixed-tint variants (bane crimson) are unaffected either way.
        /// Defaults on so the chromatic overlays show out of the box.
        #[serde(default = "default_overlay_natural_color")]
        pub overlay_natural_color: bool,
        // ── Visual-effects knobs (v0.3.3) ────────────────────────────
        //
        // Exposed in the admin panel after v0.3.0/v0.3.1/v0.3.2 made
        // the HDR + mip-chain-bloom + overlay-dwell defaults visible.
        // Each #[serde(default = "...")] uses a named default fn so
        // older settings.json files that predate these fields keep
        // working and pick up the new defaults silently.
        /// Head-glyph HDR brightness multiplier. The glyph shader
        /// emits `(1.0 + head_mix * vfx_head_hdr_scale)` × base color
        /// for head glyphs — heads at full head_mix end up
        /// `1.0 + this` times brighter than trails. Bigger values
        /// crank the bloom halo around heads; too big crushes the
        /// midtones via the ACES tone-map shoulder.
        ///
        /// Default: 1.5. v0.3.0 shipped at 3.0 (heads at 4.0×) and
        /// produced the "no midtones / looks thresholded" artefact
        /// in overlay silhouettes. v0.3.2 dialled back to 1.5.
        #[serde(default = "default_vfx_head_hdr_scale")]
        pub vfx_head_hdr_scale: f32,
        /// Bloom prefilter threshold (HDR linear). HDR pixels above
        /// this value contribute to the bloom; the shader applies a
        /// soft knee at half the threshold so the cutoff fades.
        /// Lower = more glow, fuzzier; higher = sharper, head-only.
        ///
        /// Default: 0.7. v0.3.0 shipped at 1.0 (head-only).
        #[serde(default = "default_vfx_bloom_threshold")]
        pub vfx_bloom_threshold: f32,
        /// Per-upsample additive intensity in the mip-chain bloom.
        /// Each upsample pass writes `tent_filter(src) * intensity`
        /// additively onto the larger mip below. Bigger = stronger
        /// glow at every level (compounds across 4 upsamples).
        ///
        /// Default: 0.85.
        #[serde(default = "default_vfx_bloom_intensity")]
        pub vfx_bloom_intensity: f32,
        /// Post-reveal dwell time, in seconds. After the overlay
        /// painting heads finish, the silhouette stays frozen for
        /// this long before normal rain washes back over it. v0.3.2
        /// introduced this; without it, the silhouette dissolved
        /// within one frame of completing.
        ///
        /// Default: 15.0.
        #[serde(default = "default_overlay_persist_seconds")]
        pub overlay_persist_seconds: f32,
    }

    impl Default for Settings {
        fn default() -> Self {
            Self {
                variant: "original".to_owned(),
                pipeline: Pipeline::OpenGl,
                glow_quality: GlowQuality::Balanced,
                // v0.2.1: overlays default on. The MSI ships the
                // overlay pack into %ProgramData%\matrisaver\overlays\
                // and the runtime treats that as an always-on
                // baseline source, so a fresh install shows the
                // ASCII-overlay effect without the user touching the
                // dialog. Existing users keep whatever their saved
                // settings.json says — only fresh settings get `true`.
                overlay_enabled: true,
                performance_mode: false,
                multi_monitor: true,
                char_size: 22,
                overlay_directories: Vec::new(),
                overlay_auto_levels: false,
                overlay_natural_color: default_overlay_natural_color(),
                vfx_head_hdr_scale: default_vfx_head_hdr_scale(),
                vfx_bloom_threshold: default_vfx_bloom_threshold(),
                vfx_bloom_intensity: default_vfx_bloom_intensity(),
                overlay_persist_seconds: default_overlay_persist_seconds(),
            }
        }
    }

    fn default_glow_quality() -> GlowQuality {
        GlowQuality::Balanced
    }

    fn default_overlay_natural_color() -> bool {
        true
    }

    // v0.3.3 VFX/overlay knob defaults — named functions so #[serde(default = "...")]
    // can adopt them, which lets pre-v0.3.3 settings.json files load without
    // explicit values for these fields.
    fn default_vfx_head_hdr_scale() -> f32 {
        1.5
    }
    fn default_vfx_bloom_threshold() -> f32 {
        0.7
    }
    fn default_vfx_bloom_intensity() -> f32 {
        0.85
    }
    fn default_overlay_persist_seconds() -> f32 {
        15.0
    }

    impl Settings {
        pub fn sanitize(mut self) -> Self {
            if variant_by_key(&self.variant).is_none() {
                self.variant = "original".to_owned();
            }
            self.char_size = self.char_size.clamp(8, 96);
            // Drop empty / dotfile / clearly bogus paths; keep order.
            self.overlay_directories.retain(|src| {
                !src.path.as_os_str().is_empty()
                    && src.path.file_name().is_some_and(|n| !n.is_empty())
            });
            // VFX knobs — clamp to sane envelopes. Values outside these
            // ranges produce visual artefacts (uniform overflow, bloom
            // saturation, NaN propagation from negative intensities)
            // rather than improved visuals.
            if !self.vfx_head_hdr_scale.is_finite() {
                self.vfx_head_hdr_scale = default_vfx_head_hdr_scale();
            }
            self.vfx_head_hdr_scale = self.vfx_head_hdr_scale.clamp(0.0, 4.0);
            if !self.vfx_bloom_threshold.is_finite() {
                self.vfx_bloom_threshold = default_vfx_bloom_threshold();
            }
            self.vfx_bloom_threshold = self.vfx_bloom_threshold.clamp(0.0, 3.0);
            if !self.vfx_bloom_intensity.is_finite() {
                self.vfx_bloom_intensity = default_vfx_bloom_intensity();
            }
            self.vfx_bloom_intensity = self.vfx_bloom_intensity.clamp(0.0, 2.0);
            if !self.overlay_persist_seconds.is_finite() {
                self.overlay_persist_seconds = default_overlay_persist_seconds();
            }
            self.overlay_persist_seconds = self.overlay_persist_seconds.clamp(0.0, 120.0);
            self
        }
    }
}

pub mod renderer {
    use crate::config::GlowQuality;
    use ab_glyph::{point, Font, FontArc, FontRef, Glyph};
    use bytemuck::{Pod, Zeroable};

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct AtlasGlyph {
        pub glyph: char,
        pub u0: f32,
        pub v0: f32,
        pub u1: f32,
        pub v1: f32,
        /// Fraction of the glyph cell the rasterised outline inks, in
        /// `0.0..~1.0` (anti-aliased coverage summed over the cell and
        /// divided by the cell area). Measured once at atlas build with
        /// the same embedded font the GPU rasterises. This is what lets
        /// the overlay image→glyph mapper choose glyphs by *actual* ink
        /// density (proper tonal ramp) instead of a hardcoded ASCII
        /// punctuation ramp. Blank/whitespace glyphs are ~0.0.
        pub coverage: f32,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct GlyphAtlas {
        pub glyph_size: u16,
        pub texture_size: (u16, u16),
        pub glyphs: Vec<AtlasGlyph>,
    }

    impl GlyphAtlas {
        pub fn from_symbols(symbols: &str, glyph_size: u16, max_texture_size: u16) -> Self {
            let unique: Vec<char> = symbols.chars().collect();
            let count = unique.len().max(1) as u16;
            let cells_per_row = ((count as f32).sqrt().ceil() as u16).max(1);
            let rows = count.div_ceil(cells_per_row);
            let texture_width = (cells_per_row * glyph_size)
                .min(max_texture_size)
                .max(glyph_size);
            let texture_height = (rows * glyph_size).min(max_texture_size).max(glyph_size);

            // Load the same embedded font the GPU rasterises so the
            // measured coverage matches what actually gets drawn. If it
            // fails to load we fall back to a neutral 0.5 coverage for
            // every glyph (the GPU would be drawing placeholders anyway).
            let font = Self::embedded_font();

            let mut glyphs = Vec::with_capacity(unique.len());
            for (index, glyph) in unique.iter().enumerate() {
                let idx = index as u16;
                let col = idx % cells_per_row;
                let row = idx / cells_per_row;
                let px = (col * glyph_size) as f32;
                let py = (row * glyph_size) as f32;
                let tw = texture_width as f32;
                let th = texture_height as f32;
                let coverage = font
                    .as_ref()
                    .map(|font| Self::glyph_coverage(font, *glyph, glyph_size as f32))
                    .unwrap_or(0.5);
                glyphs.push(AtlasGlyph {
                    glyph: *glyph,
                    u0: px / tw,
                    v0: py / th,
                    u1: (px + glyph_size as f32) / tw,
                    v1: (py + glyph_size as f32) / th,
                    coverage,
                });
            }

            Self {
                glyph_size,
                texture_size: (texture_width, texture_height),
                glyphs,
            }
        }

        fn embedded_font() -> Option<FontArc> {
            const CJK_FONT_BYTES: &[u8] =
                include_bytes!("../../../../assets/fonts/NotoSansCJK-Regular.ttc");
            FontArc::try_from_slice(CJK_FONT_BYTES).ok().or_else(|| {
                FontRef::try_from_slice_and_index(CJK_FONT_BYTES, 0)
                    .ok()
                    .map(FontArc::new)
            })
        }

        /// Anti-aliased ink coverage of a glyph in its cell, in
        /// `0.0..~1.0`. Sums per-pixel coverage from the rasterised
        /// outline and divides by the cell area. Mirrors the GPU
        /// `draw_font_cell` scale (`glyph_size * 0.98`) so the measured
        /// density tracks what's drawn. Whitespace / outline-less glyphs
        /// return 0.0.
        fn glyph_coverage(font: &FontArc, glyph_char: char, glyph_size: f32) -> f32 {
            let scale_value = (glyph_size * 0.98).max(4.0);
            let scale = ab_glyph::PxScale {
                x: scale_value,
                y: scale_value,
            };
            let glyph = Glyph {
                id: font.glyph_id(glyph_char),
                scale,
                position: point(0.0, 0.0),
            };
            let Some(outline) = font.outline_glyph(glyph) else {
                return 0.0;
            };
            let mut sum = 0.0f32;
            outline.draw(|_x, _y, coverage| sum += coverage);
            let cell_area = (glyph_size * glyph_size).max(1.0);
            (sum / cell_area).clamp(0.0, 1.0)
        }

        /// Atlas glyph indices sorted by ascending ink coverage, with
        /// near-blank glyphs (whitespace) dropped. This is the tonal
        /// ramp the overlay mapper indexes into: low luminance picks
        /// sparse glyphs, high luminance picks dense ones. Computed from
        /// real measured coverage rather than a hardcoded ASCII string.
        pub fn coverage_ramp(&self) -> Vec<(f32, u32)> {
            let mut ramp: Vec<(f32, u32)> = self
                .glyphs
                .iter()
                .enumerate()
                .filter(|(_, glyph)| glyph.coverage > 0.02)
                .map(|(index, glyph)| (glyph.coverage, index as u32))
                .collect();
            ramp.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            ramp
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct FramePlan {
        pub instance_count: u32,
        pub downsample_factor: u8,
        pub glow_quality: GlowQuality,
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
    pub struct GlyphInstance {
        pub position_size: [f32; 4],
        pub uv_rect: [f32; 4],
        pub params: [f32; 4],
        /// Per-instance overlay colour (rgb in .xyz, .w unused). Only
        /// consulted by the glyph shader for overlay-flagged glyphs when
        /// the sample-colour flag is set (film variants); rain, ghosts,
        /// and fixed-tint overlays (bane) leave it zeroed and unused.
        pub color: [f32; 4],
    }

    pub fn plan_frame(
        width: u32,
        height: u32,
        char_size: u16,
        density: f32,
        glow_quality: GlowQuality,
    ) -> FramePlan {
        let cols = (width / char_size.max(1) as u32).max(1);
        let rows = (height / char_size.max(1) as u32).max(1);
        let effective_density = density.clamp(0.3, 1.0);
        let trail_len = ((rows as f32) * 0.35).max(1.0);
        let instance_count = ((cols as f32) * effective_density * trail_len) as u32;
        let downsample_factor = match glow_quality {
            GlowQuality::Low => 4,
            GlowQuality::Balanced => 2,
            GlowQuality::High => 1,
        };
        FramePlan {
            instance_count: instance_count.max(1),
            downsample_factor,
            glow_quality,
        }
    }

    pub fn build_instances(
        frame_plan: FramePlan,
        atlas: &GlyphAtlas,
        width: u32,
        height: u32,
        char_size: u16,
        animation_seconds: f32,
    ) -> Vec<GlyphInstance> {
        let instance_count = frame_plan.instance_count.max(1) as usize;
        let cols = (width / char_size.max(1) as u32).max(1);
        let rows = (height / char_size.max(1) as u32).max(1);
        let cell_w = width as f32 / cols as f32;
        let cell_h = height as f32 / rows as f32;

        let mut instances = Vec::with_capacity(instance_count);
        let fallback_glyph = AtlasGlyph {
            glyph: ' ',
            u0: 0.0,
            v0: 0.0,
            u1: 1.0,
            v1: 1.0,
            coverage: 0.0,
        };
        let glyphs = if atlas.glyphs.is_empty() {
            std::slice::from_ref(&fallback_glyph)
        } else {
            atlas.glyphs.as_slice()
        };

        for index in 0..instance_count {
            let idx_u32 = index as u32;
            let col = idx_u32 % cols;
            let row = (idx_u32 / cols) % rows;
            let glyph = glyphs[index % glyphs.len()];
            let noise =
                ((idx_u32.wrapping_mul(1_103_515_245).wrapping_add(12_345)) & 1023) as f32 / 1023.0;
            let column_seed = ((col.wrapping_mul(747_796_405).wrapping_add(2_891_336_453)) & 1023)
                as f32
                / 1023.0;
            let speed = 0.25 + column_seed * 1.4;
            let rows_f = rows as f32;
            let scroll_rows = (animation_seconds * speed * rows_f).rem_euclid(rows_f);
            let y_row = (row as f32 + scroll_rows).rem_euclid(rows_f);
            let head_row =
                (animation_seconds * speed * rows_f + column_seed * rows_f).rem_euclid(rows_f);
            let trail_len = (rows_f * 0.33).max(1.0);
            let distance = (head_row - y_row).rem_euclid(rows_f);
            let trail = (1.0 - (distance / trail_len)).clamp(0.0, 1.0);
            let head_boost = (1.0 - (distance / 2.25)).clamp(0.0, 1.0).powf(1.8);
            let size_scale = 0.8 + (noise * 0.35);
            let x = (col as f32 + 0.5) * cell_w;
            let y = (y_row + 0.5) * cell_h;
            let size = char_size as f32 * size_scale;
            let brightness = (0.08 + trail * 0.78) * (0.8 + noise * 0.2) + head_boost * 0.45;

            instances.push(GlyphInstance {
                position_size: [x, y, size, size],
                uv_rect: [glyph.u0, glyph.v0, glyph.u1, glyph.v1],
                params: [brightness.min(1.0), head_boost, noise, 0.0],
                color: [0.0; 4],
            });
        }

        instances
    }
}

pub mod gpu;

pub mod storage {
    use crate::config::Settings;
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};

    pub fn default_settings_path() -> PathBuf {
        // Override at the top of the chain so power-users and tests
        // can pin the file location explicitly.
        if let Some(path) = std::env::var_os("MATRISAVER_SETTINGS_PATH") {
            return PathBuf::from(path);
        }
        // OS-native config directory:
        //   Windows : %APPDATA% (Roaming)
        //   Linux   : $XDG_CONFIG_HOME or ~/.config
        //   macOS   : ~/Library/Application Support
        // The previous implementation was hand-rolled XDG-only and on
        // Windows fell through to a bare relative "settings.json"
        // when launched without $HOME (e.g. Display Properties →
        // winlogon parent), which tried to write into
        // C:\Windows\System32\ and silently failed.
        if let Some(base) = dirs::config_dir() {
            return base.join("matrisaver").join("settings.json");
        }
        PathBuf::from("settings.json")
    }

    pub fn load_settings(path: Option<&Path>) -> io::Result<Settings> {
        let effective_path = path.map_or_else(default_settings_path, Path::to_path_buf);
        let raw = fs::read_to_string(effective_path)?;
        let parsed: Settings = serde_json::from_str(&raw)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        Ok(parsed.sanitize())
    }

    pub fn load_settings_or_default(path: Option<&Path>) -> Settings {
        load_settings(path).unwrap_or_default().sanitize()
    }

    pub fn save_settings(settings: &Settings, path: Option<&Path>) -> io::Result<()> {
        let effective_path = path.map_or_else(default_settings_path, Path::to_path_buf);
        if let Some(parent) = effective_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let sanitized = settings.clone().sanitize();
        let serialized = serde_json::to_string_pretty(&sanitized)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        fs::write(effective_path, format!("{serialized}\n"))
    }
}

pub mod perf {
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct FrameTimings {
        pub update_ms: f64,
        pub draw_ms: f64,
        pub post_process_ms: f64,
        pub total_ms: f64,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct PerfSummary {
        pub frame_count: u64,
        pub avg_update_ms: f64,
        pub avg_draw_ms: f64,
        pub avg_post_process_ms: f64,
        pub avg_total_ms: f64,
        pub p95_total_ms: f64,
        pub avg_fps: f64,
    }

    #[derive(Debug, Default, Clone)]
    pub struct FrameProfiler {
        frame_count: u64,
        total_update_ms: f64,
        total_draw_ms: f64,
        total_post_process_ms: f64,
        total_frame_ms: f64,
        frame_samples_ms: Vec<f64>,
    }

    impl FrameProfiler {
        pub fn record(&mut self, frame: FrameTimings) {
            self.frame_count += 1;
            self.total_update_ms += frame.update_ms;
            self.total_draw_ms += frame.draw_ms;
            self.total_post_process_ms += frame.post_process_ms;
            self.total_frame_ms += frame.total_ms;
            self.frame_samples_ms.push(frame.total_ms);
        }

        pub fn summary(&self) -> Option<PerfSummary> {
            if self.frame_count == 0 {
                return None;
            }
            let count = self.frame_count as f64;
            let avg_total_ms = self.total_frame_ms / count;
            let mut samples = self.frame_samples_ms.clone();
            samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let p95_index = ((samples.len() - 1) as f64 * 0.95).round() as usize;
            let p95_total_ms = samples[p95_index];
            Some(PerfSummary {
                frame_count: self.frame_count,
                avg_update_ms: self.total_update_ms / count,
                avg_draw_ms: self.total_draw_ms / count,
                avg_post_process_ms: self.total_post_process_ms / count,
                avg_total_ms,
                p95_total_ms,
                avg_fps: if avg_total_ms > f64::EPSILON {
                    1000.0 / avg_total_ms
                } else {
                    f64::INFINITY
                },
            })
        }
    }
}

pub mod update;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    UserInput,
    SessionTransition,
    HostRequest,
}

/// Host-agnostic runtime lifecycle shell.
pub struct CoreRuntime {
    settings: config::Settings,
    runtime_config: config::RuntimeConfig,
    atlas: renderer::GlyphAtlas,
    surface_size: (u32, u32),
    gpu_selection: gpu::GpuSelectionOptions,
    gpu_scaffold: Option<gpu::GpuRendererScaffold>,
    exit_reason: Option<ExitReason>,
    profiler: perf::FrameProfiler,
    animation_seconds: f32,
    frame_index: u64,
    rain_columns: Vec<RainColumn>,
    rain_layout: (u32, u32, u16),
    super_volatile_next_change: f32,
    super_volatile_pulse_time: Option<f32>,
    overlay_active_until: Option<f32>,
    overlay_next_trigger: f32,
    /// Post-reveal hold timestamp. Set when the last overlay painting
    /// head completes its targets; the silhouette stays locked (frozen
    /// cells preserved) until `now >= overlay_dissolve_at`, at which
    /// point `clear_overlay_locks()` fires and normal rain resumes.
    /// Without this, the painting heads completing immediately tore
    /// down the silhouette — the user never got to dwell on the
    /// fully-revealed image. The dwell duration is the admin-panel-
    /// tunable `Settings.overlay_persist_seconds`.
    overlay_dissolve_at: Option<f32>,
    overlay_locked_cells: Vec<(usize, usize)>,
    overlay_image_cursor: usize,
    overlay_injected_count: u32,
    overlay_image_name: String,
    /// Source path of the current overlay image when it came from a
    /// custom folder that opted into sidecar output (`write_ascii`).
    /// `Some` arms the render-to-PNG capture; the rendered "matrix-code
    /// version" is written next to this file at full bloom. `None` for
    /// variant-pinned / shipped-pack overlays (no sidecars).
    overlay_capture_source: Option<std::path::PathBuf>,
    /// Frame index at which to grab the full-bloom render-to-PNG sidecar
    /// (set a short settle after the painting heads finish). One-shot.
    overlay_capture_at_frame: Option<u64>,
    overlay_reference_rect: Option<(u32, u32, u32, u32)>,
    overlay_headers: Vec<OverlayHeader>,
    overlay_intro_glyphs: Vec<OverlayIntroGlyph>,
    overlay_intro_mode: OverlayIntroMode,
    overlay_tuning: OverlayTuning,
    /// Idempotent per-session cache of overlay-source writability.
    /// `true` = the writer probe succeeded once; `false` = probe
    /// failed, skip future writes silently. Per the v0.2.0 contract:
    /// no error surfacing, no retries.
    overlay_dir_writable: std::collections::HashMap<std::path::PathBuf, bool>,
    /// Showcase/demo pacing: when `MATRISAVER_OVERLAY_FAST` is set, the
    /// overlay trigger gap, initial delay, and active-hold collapse to a
    /// few seconds so every queued image cycles quickly (e.g. to preview
    /// a whole folder live). Off = the normal 8s / 15-30s cadence.
    overlay_fast: bool,
}

impl CoreRuntime {
    pub fn new(settings: config::Settings) -> Self {
        let settings = settings.sanitize();
        let char_size = settings.char_size;
        let variant = config::variant_by_key(&settings.variant).unwrap_or(&config::VARIANTS[0]);
        let mut runtime_config = variant.to_runtime(settings.char_size);
        runtime_config.pipeline = settings.pipeline;
        runtime_config.sanitize();
        let atlas =
            renderer::GlyphAtlas::from_symbols(&runtime_config.symbols, settings.char_size, 4096);
        let overlay_fast = std::env::var("MATRISAVER_OVERLAY_FAST")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        Self {
            runtime_config,
            settings,
            atlas,
            surface_size: (1920, 1080),
            gpu_selection: gpu::GpuSelectionOptions::from_env(),
            gpu_scaffold: None,
            exit_reason: None,
            profiler: perf::FrameProfiler::default(),
            animation_seconds: 0.0,
            frame_index: 0,
            rain_columns: Vec::new(),
            rain_layout: (0, 0, char_size),
            super_volatile_next_change: 2.0,
            super_volatile_pulse_time: None,
            overlay_active_until: None,
            overlay_next_trigger: if overlay_fast {
                OVERLAY_FAST_INITIAL_TRIGGER_SECONDS
            } else {
                OVERLAY_INITIAL_TRIGGER_SECONDS
            },
            overlay_dissolve_at: None,
            overlay_locked_cells: Vec::new(),
            overlay_image_cursor: 0,
            overlay_injected_count: 0,
            overlay_image_name: "none".to_owned(),
            overlay_capture_source: None,
            overlay_capture_at_frame: None,
            overlay_reference_rect: None,
            overlay_headers: Vec::new(),
            overlay_intro_glyphs: Vec::new(),
            overlay_intro_mode: OverlayIntroMode::AllAtOnce,
            overlay_tuning: OverlayTuning::default(),
            overlay_dir_writable: std::collections::HashMap::new(),
            overlay_fast,
        }
    }

    /// Active-hold ("dim pre-show") duration — collapsed in fast mode.
    fn overlay_hold_seconds(&self) -> f32 {
        if self.overlay_fast {
            OVERLAY_FAST_HOLD_SECONDS
        } else {
            OVERLAY_HOLD_SECONDS
        }
    }

    /// (min, range) seconds for the gap before the next overlay —
    /// collapsed in fast mode so a whole queue cycles quickly.
    fn overlay_trigger_gap(&self) -> (f32, f32) {
        if self.overlay_fast {
            (
                OVERLAY_FAST_TRIGGER_MIN_SECONDS,
                OVERLAY_FAST_TRIGGER_RANGE_SECONDS,
            )
        } else {
            (OVERLAY_TRIGGER_MIN_SECONDS, OVERLAY_TRIGGER_RANGE_SECONDS)
        }
    }

    pub fn tick(&mut self, delta_seconds: f32) {
        let _ = self.tick_profiled(delta_seconds);
    }

    pub fn tick_profiled(&mut self, delta_seconds: f32) -> perf::FrameTimings {
        let frame_started = std::time::Instant::now();
        let update_started = std::time::Instant::now();
        // Rendering/effect updates will be implemented here as parity work progresses.
        self.animation_seconds =
            (self.animation_seconds + delta_seconds.max(0.0)).rem_euclid(4096.0);
        self.frame_index = self.frame_index.wrapping_add(1);
        if self.animation_seconds >= self.super_volatile_next_change {
            self.super_volatile_pulse_time = Some(self.animation_seconds);
            self.super_volatile_next_change =
                self.animation_seconds + 2.0 + hash01(self.frame_index as u32, 0x5151_AA77) * 5.0;
        } else {
            self.super_volatile_pulse_time = None;
        }
        let update_ms = update_started.elapsed().as_secs_f64() * 1000.0;

        let draw_started = std::time::Instant::now();
        let frame_plan = renderer::plan_frame(
            self.surface_size.0,
            self.surface_size.1,
            self.settings.char_size,
            self.runtime_config.density,
            self.settings.glow_quality,
        );
        let instances = self.build_stream_instances(delta_seconds.max(0.0));

        // Keep a tiny deterministic CPU checksum to avoid dead-code paths in no-GPU mode.
        let mut checksum: u32 = 0;
        for (idx, instance) in instances.iter().enumerate() {
            checksum = checksum
                .wrapping_add(idx as u32)
                .wrapping_add((instance.params[0] * 255.0) as u32)
                .wrapping_add(frame_plan.downsample_factor as u32);
        }
        if checksum == u32::MAX {
            self.request_exit(ExitReason::HostRequest);
        }
        let style_params = self.variant_style_params();
        // Resolve (and consume) any due full-bloom render-to-PNG sidecar
        // request before borrowing the GPU scaffold below.
        let overlay_capture_path = self.take_due_overlay_capture();
        if let Some(gpu) = &mut self.gpu_scaffold {
            let color = self.runtime_config.color;
            let glyph_tint = [
                color.0 as f32 / 255.0,
                color.1 as f32 / 255.0,
                color.2 as f32 / 255.0,
            ];
            let overlay = self.runtime_config.overlay_tint;
            let overlay_tint = [
                overlay.0 as f32 / 255.0,
                overlay.1 as f32 / 255.0,
                overlay.2 as f32 / 255.0,
            ];
            gpu.draw_instanced_pass(
                &instances,
                frame_plan.downsample_factor,
                gpu::GlyphTints {
                    field: glyph_tint,
                    overlay: overlay_tint,
                    // Variant capability AND the user's natural-colour
                    // toggle: colour-capable variants sample image hues
                    // only when the user leaves it on; fixed-tint variants
                    // (bane) ignore it.
                    sample_overlay_color: self.runtime_config.overlay_sample_color
                        && self.settings.overlay_natural_color,
                },
                style_params,
                self.animation_seconds,
                gpu::VfxRenderParams {
                    head_hdr_scale: self.settings.vfx_head_hdr_scale,
                    bloom_threshold: self.settings.vfx_bloom_threshold,
                    bloom_intensity: self.settings.vfx_bloom_intensity,
                },
            );
            // Full bloom — grab the rendered "matrix-code version" of the
            // overlay and write it next to the source image. Best-effort.
            if let Some(path) = &overlay_capture_path {
                if let Err(error) = gpu.read_output_to_png(path) {
                    eprintln!("overlay render-to-PNG sidecar failed: {error}");
                }
            }
        }
        let draw_ms = draw_started.elapsed().as_secs_f64() * 1000.0;

        let post_started = std::time::Instant::now();
        let _post_scale = 1.0 / f64::from(frame_plan.downsample_factor.max(1));
        let post_process_ms = post_started.elapsed().as_secs_f64() * 1000.0;
        let total_ms = frame_started.elapsed().as_secs_f64() * 1000.0;

        let timings = perf::FrameTimings {
            update_ms,
            draw_ms,
            post_process_ms,
            total_ms,
        };
        self.profiler.record(timings);
        timings
    }

    pub fn request_exit(&mut self, reason: ExitReason) {
        self.exit_reason = Some(reason);
    }

    pub fn settings(&self) -> &config::Settings {
        &self.settings
    }

    pub fn runtime_config(&self) -> &config::RuntimeConfig {
        &self.runtime_config
    }

    pub fn apply_settings(&mut self, settings: config::Settings) {
        *self = Self::new(settings);
    }

    pub fn set_surface_size(&mut self, width: u32, height: u32) {
        self.surface_size = (width.max(1), height.max(1));
        if let Some((x, y, w, h)) = self.overlay_reference_rect {
            let max_x = self.surface_size.0.saturating_sub(1);
            let max_y = self.surface_size.1.saturating_sub(1);
            let x = x.min(max_x);
            let y = y.min(max_y);
            let w = w.min(self.surface_size.0.saturating_sub(x)).max(1);
            let h = h.min(self.surface_size.1.saturating_sub(y)).max(1);
            self.overlay_reference_rect = Some((x, y, w, h));
        }
        if let Some(gpu) = &mut self.gpu_scaffold {
            gpu.set_surface_size(self.surface_size.0, self.surface_size.1);
        }
    }

    pub fn set_overlay_reference_rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        let max_x = self.surface_size.0.saturating_sub(1);
        let max_y = self.surface_size.1.saturating_sub(1);
        let x = x.min(max_x);
        let y = y.min(max_y);
        let w = width.min(self.surface_size.0.saturating_sub(x)).max(1);
        let h = height.min(self.surface_size.1.saturating_sub(y)).max(1);
        self.overlay_reference_rect = Some((x, y, w, h));
    }

    pub fn clear_overlay_reference_rect(&mut self) {
        self.overlay_reference_rect = None;
    }

    pub fn adapter_snapshots(&self) -> Vec<gpu::AdapterSnapshot> {
        gpu::enumerate_adapters()
    }

    pub fn set_gpu_selection(&mut self, selection: gpu::GpuSelectionOptions) {
        self.gpu_selection = selection;
    }

    pub fn selected_adapter_snapshot(&self) -> Option<&gpu::AdapterSnapshot> {
        self.gpu_scaffold
            .as_ref()
            .map(gpu::GpuRendererScaffold::selected_adapter)
    }

    pub fn enable_gpu_scaffold(&mut self) -> Result<(), String> {
        let scaffold = gpu::GpuRendererScaffold::initialize(
            self.surface_size.0,
            self.surface_size.1,
            &self.atlas,
            &self.gpu_selection,
        )?;
        self.gpu_scaffold = Some(scaffold);
        Ok(())
    }

    pub fn enable_gpu_scaffold_with_shared_device(
        &mut self,
        selected_adapter: gpu::AdapterSnapshot,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Result<(), String> {
        let scaffold = gpu::GpuRendererScaffold::initialize_with_shared_device(
            self.surface_size.0,
            self.surface_size.1,
            &self.atlas,
            selected_adapter,
            device,
            queue,
        )?;
        self.gpu_scaffold = Some(scaffold);
        Ok(())
    }

    pub fn gpu_scaffold_output_view(&self) -> Option<&wgpu::TextureView> {
        self.gpu_scaffold
            .as_ref()
            .map(gpu::GpuRendererScaffold::output_view)
    }

    pub fn performance_summary(&self) -> Option<perf::PerfSummary> {
        self.profiler.summary()
    }
}

include!("runtime/types.rs");
include!("runtime/trace.rs");
include!("runtime/overlay/state.rs");
include!("runtime/overlay/inject.rs");
include!("runtime/overlay/emit.rs");
include!("runtime/overlay/io.rs");
include!("runtime/overlay/image.rs");
include!("runtime/lifecycle/mutators.rs");
include!("runtime/lifecycle/frame.rs");
include!("runtime/lifecycle/column.rs");
include!("runtime/lifecycle/cells.rs");
include!("runtime/lifecycle/reset.rs");

fn hash01(a: u32, b: u32) -> f32 {
    let mut value = a
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(b.wrapping_mul(0x85EB_CA6B))
        .wrapping_add(0xC2B2_AE35);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^= value >> 16;
    (value as f32) / (u32::MAX as f32)
}

#[cfg(test)]
mod tests {
    use super::config;
    use super::perf;
    use super::renderer;
    use super::storage;
    use super::CoreRuntime;
    use super::OverlayTuning;
    use super::OverlayTuningConfig;
    use super::RainColumn;
    use super::RowCell;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn has_all_expected_variants() {
        let keys: Vec<&str> = config::VARIANTS.iter().map(|variant| variant.key).collect();
        assert_eq!(
            keys,
            vec![
                "original",
                "reloaded",
                "revolutions",
                "resurrections",
                "bane"
            ]
        );
    }

    #[test]
    fn variant_runtime_conversion_preserves_key_fields() {
        let variant = config::variant_by_key("original").expect("original variant is missing");
        let runtime = variant.to_runtime(22);
        assert_eq!(runtime.char_size, 22);
        // v0.3.0 1999 green calibration: (0,255,70) → (35,235,65).
        assert_eq!(runtime.color, (35, 235, 65));
        assert_eq!(runtime.pipeline, config::Pipeline::OpenGl);
        assert!((runtime.density - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn bane_variant_resolves_crimson_overlay_and_subdir() {
        let variant = config::variant_by_key("bane").expect("bane variant is missing");
        let runtime = variant.to_runtime(22);
        // Dim green field, crimson overlay — two distinct colours so the
        // painted silhouette reads red over the rain.
        assert_eq!(runtime.color, (12, 130, 45));
        assert_eq!(runtime.overlay_tint, (255, 18, 14));
        assert_ne!(runtime.color, runtime.overlay_tint);
        // Variant pins its own overlay directory, paints at full pitch,
        // and skips the dim pre-show preview.
        assert_eq!(runtime.overlay_subdir, Some("bane"));
        assert!(runtime.overlay_full_pitch);
        assert!(runtime.overlay_skip_preview);
        // Bane uses its fixed crimson tint, not image-sampled colour.
        assert!(!runtime.overlay_sample_color);
        // Films keep the field colour, dense half-pitch packing, and the
        // dim preview (no rendering overrides), but each now leads its
        // overlay queue with its own per-film subdir and samples colour.
        let original = config::variant_by_key("original").unwrap().to_runtime(22);
        assert_eq!(original.overlay_tint, original.color);
        assert_eq!(original.overlay_subdir, Some("original"));
        assert!(!original.overlay_full_pitch);
        assert!(!original.overlay_skip_preview);
        assert!(original.overlay_sample_color);
    }

    #[test]
    fn overlay_queue_interleaves_variant_iconic_first() {
        let variant = vec![
            (PathBuf::from("/v/scene1.png"), false),
            (PathBuf::from("/v/scene2.png"), false),
        ];
        let folder = vec![
            (PathBuf::from("/f/a.png"), true),
            (PathBuf::from("/f/b.png"), true),
            (PathBuf::from("/f/c.png"), true),
        ];
        let out = CoreRuntime::interleave_overlay_queues(variant, folder);
        let names: Vec<String> = out
            .iter()
            .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        // Variant iconic leads, then folder, alternating; the longer
        // queue's tail follows once the shorter is exhausted.
        assert_eq!(
            names,
            vec!["scene1.png", "a.png", "scene2.png", "b.png", "c.png"]
        );
        // write_ascii flag rides through (folder entries true).
        assert!(out[1].1);
    }

    #[test]
    fn overlay_queue_dedupes_by_filename_variant_wins() {
        let variant = vec![(PathBuf::from("/v/dup.png"), false)];
        let folder = vec![(PathBuf::from("/f/dup.png"), true)];
        let out = CoreRuntime::interleave_overlay_queues(variant, folder);
        assert_eq!(out.len(), 1);
        // The variant copy appears first, so it wins the dedup.
        assert_eq!(out[0].0, PathBuf::from("/v/dup.png"));
        assert!(!out[0].1);
    }

    #[test]
    fn overlay_queue_empty_variant_yields_folder_only() {
        let folder = vec![(PathBuf::from("/f/a.png"), false)];
        let out = CoreRuntime::interleave_overlay_queues(Vec::new(), folder);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, PathBuf::from("/f/a.png"));
    }

    #[test]
    fn bane_overlay_injects_silhouette_when_mask_present() {
        // The Bane mask is a DEV-only, gitignored asset. In CI (and any
        // fresh clone) it's absent, so skip cleanly rather than fail.
        let mask = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("assets")
            .join("overlays")
            .join("bane")
            .join("bane-hold.png");
        if !mask.exists() {
            eprintln!("skipping: bane mask absent at {}", mask.display());
            return;
        }
        let settings = config::Settings {
            variant: "bane".to_owned(),
            ..config::Settings::default()
        };
        let mut runtime = CoreRuntime::new(settings);
        runtime.set_surface_size(1920, 1080);
        // Drive ~22s of frames: past the 8s initial trigger, past the 8s
        // active-hold, and into the painting phase where headers freeze
        // silhouette cells (which is what bumps overlay_injected_count).
        for _ in 0..(60 * 22) {
            runtime.tick(1.0 / 60.0);
        }
        assert!(
            runtime.overlay_injected_count > 0,
            "bane overlay never painted any cells (count=0); image={}",
            runtime.overlay_image_name
        );
        assert!(
            runtime.overlay_image_name.contains("bane"),
            "overlay injected from unexpected image: {}",
            runtime.overlay_image_name
        );
    }

    #[test]
    fn core_runtime_uses_default_variant_when_key_is_unknown() {
        let settings = config::Settings {
            variant: "missing-key".to_owned(),
            ..config::Settings::default()
        };
        let runtime = CoreRuntime::new(settings);
        // v0.3.0 1999 green calibration: (0,255,70) → (35,235,65).
        assert_eq!(runtime.runtime_config().color, (35, 235, 65));
        assert_eq!(runtime.runtime_config().char_size, 22);
    }

    #[test]
    fn core_runtime_prefers_pipeline_from_settings() {
        let settings = config::Settings {
            pipeline: config::Pipeline::Cpu,
            ..config::Settings::default()
        };
        let runtime = CoreRuntime::new(settings);
        assert_eq!(runtime.runtime_config().pipeline, config::Pipeline::Cpu);
    }

    #[test]
    fn settings_round_trip_persistence() {
        let mut path = std::env::temp_dir();
        path.push(unique_test_file_name("matrisaver-settings-roundtrip"));

        let settings = config::Settings {
            variant: "reloaded".to_owned(),
            pipeline: config::Pipeline::CpuGlow,
            glow_quality: config::GlowQuality::High,
            overlay_enabled: true,
            performance_mode: true,
            multi_monitor: false,
            char_size: 31,
            overlay_directories: vec![config::OverlaySource {
                path: std::path::PathBuf::from("/tmp/my-overlays"),
                enabled: true,
                write_ascii_alongside: true,
            }],
            overlay_auto_levels: true,
            overlay_natural_color: false,
            // v0.3.3 VFX/overlay knobs — exercise non-default values so
            // serde round-trip is verified for the new fields too.
            vfx_head_hdr_scale: 2.2,
            vfx_bloom_threshold: 0.55,
            vfx_bloom_intensity: 1.1,
            overlay_persist_seconds: 22.5,
        };
        storage::save_settings(&settings, Some(&path)).expect("save settings failed");

        let loaded = storage::load_settings(Some(&path)).expect("load settings failed");
        assert_eq!(loaded, settings);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn sanitizes_invalid_char_size_on_load() {
        let mut path = std::env::temp_dir();
        path.push(unique_test_file_name("matrisaver-settings-sanitize"));
        std::fs::write(
            &path,
            "{\n  \"variant\": \"unknown\",\n  \"pipeline\": \"opengl\",\n  \"overlay_enabled\": false,\n  \"performance_mode\": false,\n  \"multi_monitor\": true,\n  \"char_size\": 1\n}\n",
        )
        .expect("failed to write test payload");

        let loaded = storage::load_settings(Some(&path)).expect("load settings failed");
        assert_eq!(loaded.variant, "original");
        assert_eq!(loaded.char_size, 8);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn profiler_reports_average_values() {
        let mut profiler = perf::FrameProfiler::default();
        profiler.record(perf::FrameTimings {
            update_ms: 1.0,
            draw_ms: 2.0,
            post_process_ms: 3.0,
            total_ms: 6.0,
        });
        profiler.record(perf::FrameTimings {
            update_ms: 3.0,
            draw_ms: 4.0,
            post_process_ms: 5.0,
            total_ms: 12.0,
        });

        let summary = profiler.summary().expect("summary should exist");
        assert_eq!(summary.frame_count, 2);
        assert!((summary.avg_update_ms - 2.0).abs() < f64::EPSILON);
        assert!((summary.avg_draw_ms - 3.0).abs() < f64::EPSILON);
        assert!((summary.avg_post_process_ms - 4.0).abs() < f64::EPSILON);
        assert!((summary.avg_total_ms - 9.0).abs() < f64::EPSILON);
        assert!((summary.p95_total_ms - 12.0).abs() < f64::EPSILON);
    }

    #[test]
    fn runtime_collects_profiled_ticks() {
        let mut runtime = CoreRuntime::new(config::Settings::default());
        runtime.tick_profiled(1.0 / 60.0);
        runtime.tick_profiled(1.0 / 60.0);
        let summary = runtime
            .performance_summary()
            .expect("runtime summary should exist");
        assert_eq!(summary.frame_count, 2);
    }

    #[test]
    fn atlas_contains_symbol_entries() {
        let atlas = renderer::GlyphAtlas::from_symbols("ABC", 16, 256);
        assert_eq!(atlas.glyphs.len(), 3);
        assert!(atlas.texture_size.0 >= 16);
    }

    #[test]
    fn frame_planning_reflects_glow_quality() {
        let low = renderer::plan_frame(1920, 1080, 22, 1.0, config::GlowQuality::Low);
        let high = renderer::plan_frame(1920, 1080, 22, 1.0, config::GlowQuality::High);
        assert_eq!(low.downsample_factor, 4);
        assert_eq!(high.downsample_factor, 1);
        assert!(low.instance_count > 0);
    }

    #[test]
    fn instance_generation_matches_frame_plan() {
        let atlas = renderer::GlyphAtlas::from_symbols("ABCD", 16, 256);
        let frame = renderer::plan_frame(1280, 720, 16, 0.8, config::GlowQuality::Balanced);
        let instances = renderer::build_instances(frame, &atlas, 1280, 720, 16, 0.0);
        assert_eq!(instances.len(), frame.instance_count as usize);
        assert!(instances.iter().all(|instance| {
            instance.uv_rect[0] >= 0.0
                && instance.uv_rect[1] >= 0.0
                && instance.uv_rect[2] <= 1.0
                && instance.uv_rect[3] <= 1.0
        }));
    }

    #[test]
    fn frozen_cells_block_head_write_and_erase() {
        let mut column = RainColumn {
            column_slot: 0,
            y_positions: Vec::new(),
            speeds: Vec::new(),
            current_speeds: Vec::new(),
            glyph_indices: Vec::new(),
            next_glyph_swap_at: 0.0,
            head_y: 0.0,
            head_speed: 0.0,
            glyph_cursor: 7,
            head_glyph_index: 9,
            delete_gap: 0.0,
            last_head_row: -1,
            head_row_step: 0,
            eraser_y: 0.0,
            eraser_speed: 0.0,
            eraser_offset: 0.0,
            eraser_last_row: -1,
            head_reset_count: 0,
            eraser_reset_count: 0,
            head_write_count: 0,
            chain_reset_count: 0,
            glyph_swap_count: 0,
            row_cells: vec![RowCell {
                glyph_index: Some(42),
                brightness: 0.8,
                volatile: true,
                volatile_next: 1.0,
                volatile_last: 1.0,
                super_volatile: true,
                frozen: true,
                overlay_color: [0.0; 3],
            }],
            ghosts: Vec::new(),
        };

        let wrote = CoreRuntime::write_head_row(&mut column, 0, 1.0, 1.0, 99);
        assert!(!wrote);
        assert_eq!(column.head_write_count, 0);
        assert_eq!(column.row_cells[0].glyph_index, Some(42));

        CoreRuntime::erase_row(&mut column, 0);
        assert_eq!(column.row_cells[0].glyph_index, Some(42));
        assert!(column.row_cells[0].volatile);
    }

    #[test]
    fn overlay_coverage_ramp_maps_tone_to_density() {
        // Ramp ascending by coverage. Dark tone -> sparse glyph, bright
        // tone -> dense glyph. The per-cell variety jitter stays within a
        // small window, so assert tonal *bands* rather than exact glyphs.
        let ramp = vec![(0.05f32, 7u32), (0.30, 3), (0.55, 9), (0.85, 1)];
        let cov = |idx: u32| ramp.iter().find(|(_, i)| *i == idx).unwrap().0;
        for seed in 0..32u32 {
            let dark = CoreRuntime::overlay_glyph_index_by_coverage(0.0, &ramp, seed).unwrap();
            assert!(cov(dark) <= 0.30, "dark tone must map to a sparse glyph");
            let bright = CoreRuntime::overlay_glyph_index_by_coverage(1.0, &ramp, seed).unwrap();
            assert!(cov(bright) >= 0.55, "bright tone must map to a dense glyph");
        }
        // Empty ramp yields nothing; single-entry ramp yields that entry.
        assert_eq!(
            CoreRuntime::overlay_glyph_index_by_coverage(0.5, &[], 0),
            None
        );
        assert_eq!(
            CoreRuntime::overlay_glyph_index_by_coverage(0.5, &[(0.4, 42)], 0),
            Some(42)
        );
    }

    #[test]
    fn coverage_ramp_is_sorted_and_drops_blanks() {
        // A real atlas: coverage measured, ramp ascending, blanks gone.
        let atlas = renderer::GlyphAtlas::from_symbols("ABCﾊﾐ 0", 24, 1024);
        let ramp = atlas.coverage_ramp();
        assert!(!ramp.is_empty(), "ramp should have inked glyphs");
        for pair in ramp.windows(2) {
            assert!(pair[0].0 <= pair[1].0, "ramp must be ascending by coverage");
        }
        // The space character inks ~nothing and must be excluded.
        for &(coverage, index) in &ramp {
            assert!(coverage > 0.02);
            assert_ne!(atlas.glyphs[index as usize].glyph, ' ');
        }
    }

    #[test]
    fn overlay_tuning_defaults_to_passthrough() {
        // V2: filter fields are gone, auto_levels defaults off.
        let tuning = OverlayTuning::default();
        assert!(!tuning.auto_levels_enabled);
        assert!((tuning.alpha_cutoff - 0.03).abs() < f32::EPSILON);
        assert!((tuning.intro_glyph_scale - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn overlay_tuning_parses_typography_fields() {
        // Filter-related JSON fields (denoise_mode, clahe_*, unsharp_*,
        // gamma, contrast) parse-skip silently — serde defaults to
        // ignore unknown fields. Confirm the surviving typography
        // knobs still flow through.
        let config = serde_json::from_str::<OverlayTuningConfig>(
            r#"{
                "auto_levels_enabled": true,
                "levels_low_percentile": 0.1,
                "levels_high_percentile": 0.9,
                "intro_density_multiplier_x": 3.0,
                "intro_glyph_scale": 0.4,
                "intro_layer_brightness_scale": 1.35,
                "denoise_mode": "median",
                "clahe_enabled": true
            }"#,
        )
        .expect("overlay config should parse");
        let tuning = OverlayTuning::default().with_overrides(config);
        assert!(tuning.auto_levels_enabled);
        assert!((tuning.levels_low_percentile - 0.1).abs() < f32::EPSILON);
        assert!((tuning.levels_high_percentile - 0.9).abs() < f32::EPSILON);
        assert!((tuning.intro_density_multiplier_x - 3.0).abs() < f32::EPSILON);
        assert!((tuning.intro_glyph_scale - 0.4).abs() < f32::EPSILON);
        assert!((tuning.intro_layer_brightness_scale - 1.35).abs() < f32::EPSILON);
    }

    fn unique_test_file_name(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock went backwards")
            .as_nanos();
        PathBuf::from(format!("{prefix}-{nanos}.json"))
    }
}
