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

    pub const VARIANTS: [VariantConfig; 4] = [
        VariantConfig {
            key: "original",
            name: "The Matrix (1999)",
            color: (0, 255, 70),
            speed_range: (4, 10),
            density: 1.0,
            symbol_set: SymbolSet::KatakanaSymbols,
            glow_color: (180, 255, 180),
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
    ];

    pub fn variant_by_key(key: &str) -> Option<&'static VariantConfig> {
        VARIANTS.iter().find(|variant| variant.key == key)
    }

    /// Persisted runtime settings shared across host integrations.
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
    }

    impl Default for Settings {
        fn default() -> Self {
            Self {
                variant: "original".to_owned(),
                pipeline: Pipeline::OpenGl,
                glow_quality: GlowQuality::Balanced,
                overlay_enabled: false,
                performance_mode: false,
                multi_monitor: true,
                char_size: 22,
            }
        }
    }

    fn default_glow_quality() -> GlowQuality {
        GlowQuality::Balanced
    }

    impl Settings {
        pub fn sanitize(mut self) -> Self {
            if variant_by_key(&self.variant).is_none() {
                self.variant = "original".to_owned();
            }
            self.char_size = self.char_size.clamp(8, 96);
            self
        }
    }
}

pub mod renderer {
    use crate::config::GlowQuality;
    use bytemuck::{Pod, Zeroable};

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct AtlasGlyph {
        pub glyph: char,
        pub u0: f32,
        pub v0: f32,
        pub u1: f32,
        pub v1: f32,
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

            let mut glyphs = Vec::with_capacity(unique.len());
            for (index, glyph) in unique.iter().enumerate() {
                let idx = index as u16;
                let col = idx % cells_per_row;
                let row = idx / cells_per_row;
                let px = (col * glyph_size) as f32;
                let py = (row * glyph_size) as f32;
                let tw = texture_width as f32;
                let th = texture_height as f32;
                glyphs.push(AtlasGlyph {
                    glyph: *glyph,
                    u0: px / tw,
                    v0: py / th,
                    u1: (px + glyph_size as f32) / tw,
                    v1: (py + glyph_size as f32) / th,
                });
            }

            Self {
                glyph_size,
                texture_size: (texture_width, texture_height),
                glyphs,
            }
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
    overlay_locked_cells: Vec<(usize, usize)>,
    overlay_image_cursor: usize,
    overlay_injected_count: u32,
    overlay_image_name: String,
    overlay_reference_rect: Option<(u32, u32, u32, u32)>,
    overlay_headers: Vec<OverlayHeader>,
    overlay_intro_glyphs: Vec<OverlayIntroGlyph>,
    overlay_intro_mode: OverlayIntroMode,
    overlay_tuning: OverlayTuning,
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
            overlay_next_trigger: OVERLAY_INITIAL_TRIGGER_SECONDS,
            overlay_locked_cells: Vec::new(),
            overlay_image_cursor: 0,
            overlay_injected_count: 0,
            overlay_image_name: "none".to_owned(),
            overlay_reference_rect: None,
            overlay_headers: Vec::new(),
            overlay_intro_glyphs: Vec::new(),
            overlay_intro_mode: OverlayIntroMode::AllAtOnce,
            overlay_tuning: OverlayTuning::default(),
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
        if let Some(gpu) = &mut self.gpu_scaffold {
            let color = self.runtime_config.color;
            let glyph_tint = [
                color.0 as f32 / 255.0,
                color.1 as f32 / 255.0,
                color.2 as f32 / 255.0,
            ];
            gpu.draw_instanced_pass(
                &instances,
                frame_plan.downsample_factor,
                glyph_tint,
                style_params,
            );
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
            vec!["original", "reloaded", "revolutions", "resurrections"]
        );
    }

    #[test]
    fn variant_runtime_conversion_preserves_key_fields() {
        let variant = config::variant_by_key("original").expect("original variant is missing");
        let runtime = variant.to_runtime(22);
        assert_eq!(runtime.char_size, 22);
        assert_eq!(runtime.color, (0, 255, 70));
        assert_eq!(runtime.pipeline, config::Pipeline::OpenGl);
        assert!((runtime.density - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn core_runtime_uses_default_variant_when_key_is_unknown() {
        let settings = config::Settings {
            variant: "missing-key".to_owned(),
            ..config::Settings::default()
        };
        let runtime = CoreRuntime::new(settings);
        assert_eq!(runtime.runtime_config().color, (0, 255, 70));
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
    fn overlay_ascii_fallback_picks_available_glyph() {
        let lookup = vec![('.', 0), ('*', 1), ('+', 2)];
        assert_eq!(
            CoreRuntime::overlay_glyph_index_for_luminance(0.0, &lookup),
            Some(0)
        );
        assert_eq!(
            CoreRuntime::overlay_glyph_index_for_luminance(1.0, &lookup),
            Some(1)
        );
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
