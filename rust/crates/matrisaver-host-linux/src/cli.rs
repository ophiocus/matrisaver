//! Command line for the Linux host.
//!
//! Flag names and semantics track `matrisaver-host-windows` wherever the
//! two overlap (`--variant`, `--pipeline`, `--glow-quality`,
//! `--overlay` / `--no-overlay`, `--multi-monitor` / `--single-monitor`,
//! `--char-size`, the GPU selection flags and the benchmark/lifecycle
//! flags) so a tuning recipe carries across hosts unchanged. Linux adds
//! what a `.scr` gets from Windows instead of from argv: the run mode.

use matrisaver_core::config::{GlowQuality, Pipeline, Settings};
use matrisaver_core::gpu::GpuSelectionOptions;
use matrisaver_core::storage;

/// Process exit codes. Documented because a systemd unit, an
/// xscreensaver hook and a support report all read them.
pub mod exit_code {
    /// Clean exit, including a screensaver dismissed by the user.
    pub const OK: i32 = 0;
    /// Runtime failure after startup (surface lost, device error).
    pub const RUNTIME: i32 = 1;
    /// Bad usage: unknown flag or unparseable value.
    pub const USAGE: i32 = 2;
    /// No usable GPU adapter on any backend.
    pub const NO_GPU: i32 = 3;
    /// Playback requested with no display session reachable.
    pub const NO_SESSION: i32 = 4;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Fullscreen playback on every participating output.
    Run,
    /// A single ordinary window. Development and screenshot aid; also
    /// the only way to watch the renderer while reading a terminal.
    Window,
    /// Headless: tick the runtime N frames and print a `PERF` line. The
    /// pre-Milestone-3 behaviour of this binary, unchanged, and the only
    /// mode that works with no compositor.
    Benchmark { frames: u32 },
    /// Print adapters and exit.
    ListAdapters,
    /// Print outputs and exit.
    ListMonitors,
    /// Print usage and exit.
    Help,
}

#[derive(Debug, Clone)]
pub struct Cli {
    pub mode: Mode,
    pub settings: Settings,
    pub gpu_selection: GpuSelectionOptions,
    /// Window size for `Window` mode and scene size for `Benchmark`.
    pub width: u32,
    pub height: u32,
    pub lifecycle_frames: Option<u32>,
    pub lifecycle_trace: Option<String>,
    /// Keep the cursor visible in `Run` mode. Handy when debugging a
    /// compositor that reports pointer events oddly.
    pub show_cursor: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError(pub String);

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

pub const USAGE: &str = "\
matrisaver — Matrix-inspired digital rain screensaver

USAGE:
    matrisaver [MODE] [OPTIONS]

MODES:
    --run                 Fullscreen playback on every output (default).
    --window              Run in a single ordinary window.
    --benchmark-frames N  Headless: tick N frames, print a PERF line, exit.
                          The only mode that needs no compositor.
    --list-adapters       List the GPU adapters wgpu can see, then exit.
    --list-monitors       List the outputs the compositor reports, then exit.
    -h, --help            Print this help.

PLAYBACK OPTIONS:
    --variant KEY         original | reloaded | revolutions | resurrections | bane
    --pipeline KEY        Override the variant's pipeline.
    --glow-quality Q      low | balanced | high
    --overlay             Force the ASCII overlay on.
    --no-overlay          Force the ASCII overlay off.
    --performance         Force performance mode on.
    --no-performance      Force performance mode off.
    --multi-monitor       Span every output with one continuous scene (default).
    --single-monitor      Use the primary output only.
    --char-size N         Glyph cell size in pixels.
    --show-cursor         Do not hide the pointer during playback.
    --settings PATH       Load settings.json from PATH instead of the
                          XDG config location.

GPU SELECTION:
    --wgpu-backend LIST   Comma-separated: vulkan, gl, ... (alias: --backend)
    --wgpu-adapter-name S Substring match on the adapter name
                          (alias: --adapter-name)

DIAGNOSTICS:
    --width N             Window/scene width  (window + benchmark modes).
    --height N            Window/scene height (window + benchmark modes).
    --lifecycle-frames N  Exit after N rendered frames (soak tests).
    --lifecycle-trace P   Append LIFECYCLE trace lines to file P.

ENVIRONMENT:
    MATRISAVER_FORCE_SESSION   wayland | x11 | headless — override detection.
    MATRISAVER_OVERLAY_DIR     Overlay image directory override.

EXIT CODES:
    0 ok   1 runtime error   2 usage   3 no usable GPU   4 no display session";

/// Flags that take a following value, so a bare `--foo bar` does not
/// leave `bar` looking like an unknown positional argument.
const VALUE_FLAGS: &[&str] = &[
    "--variant",
    "--pipeline",
    "--glow-quality",
    "--char-size",
    "--settings",
    "--wgpu-backend",
    "--backend",
    "--wgpu-adapter-name",
    "--adapter-name",
    "--width",
    "--height",
    "--benchmark-frames",
    "--lifecycle-frames",
    "--lifecycle-trace",
];

const BOOL_FLAGS: &[&str] = &[
    "--run",
    "--window",
    "--list-adapters",
    "--list-monitors",
    "--help",
    "-h",
    "--overlay",
    "--no-overlay",
    "--performance",
    "--no-performance",
    "--multi-monitor",
    "--single-monitor",
    "--show-cursor",
    "--gpu-scaffold",
];

/// Parse argv (including argv\[0\]).
pub fn parse(args: &[String]) -> Result<Cli, CliError> {
    reject_unknown_flags(args)?;

    if has_flag(args, "--help") || has_flag(args, "-h") {
        return Ok(Cli {
            mode: Mode::Help,
            settings: Settings::default(),
            gpu_selection: GpuSelectionOptions::default(),
            width: 1920,
            height: 1080,
            lifecycle_frames: None,
            lifecycle_trace: None,
            show_cursor: false,
        });
    }

    let settings_path = option_value(args, "--settings").map(std::path::PathBuf::from);
    let mut settings = storage::load_settings_or_default(settings_path.as_deref());
    apply_settings_overrides(&mut settings, args)?;
    let settings = settings.sanitize();

    let benchmark_frames = match option_value(args, "--benchmark-frames") {
        Some(raw) => Some(parse_u32_value("--benchmark-frames", raw)?.max(1)),
        None => None,
    };

    // Mode precedence: diagnostics, then the explicit headless path,
    // then the explicit window modes, then the default. Listing modes
    // win so `--list-monitors --run` answers the question instead of
    // taking over the screen.
    let mode = if has_flag(args, "--list-adapters") {
        Mode::ListAdapters
    } else if has_flag(args, "--list-monitors") {
        Mode::ListMonitors
    } else if let Some(frames) = benchmark_frames {
        Mode::Benchmark { frames }
    } else if has_flag(args, "--window") {
        Mode::Window
    } else {
        Mode::Run
    };

    let width = match option_value(args, "--width") {
        Some(raw) => parse_u32_value("--width", raw)?.max(1),
        None => 1920,
    };
    let height = match option_value(args, "--height") {
        Some(raw) => parse_u32_value("--height", raw)?.max(1),
        None => 1080,
    };
    let lifecycle_frames = match option_value(args, "--lifecycle-frames") {
        Some(raw) => Some(parse_u32_value("--lifecycle-frames", raw)?),
        None => None,
    };

    Ok(Cli {
        mode,
        settings,
        gpu_selection: parse_gpu_selection(args),
        width,
        height,
        lifecycle_frames,
        lifecycle_trace: option_value(args, "--lifecycle-trace").map(str::to_owned),
        show_cursor: has_flag(args, "--show-cursor"),
    })
}

fn reject_unknown_flags(args: &[String]) -> Result<(), CliError> {
    let mut index = 1;
    while index < args.len() {
        let arg = args[index].as_str();
        if let Some((key, _)) = arg.split_once('=') {
            if VALUE_FLAGS.contains(&key) {
                index += 1;
                continue;
            }
        }
        if BOOL_FLAGS.contains(&arg) {
            index += 1;
            continue;
        }
        if VALUE_FLAGS.contains(&arg) {
            // Skip the flag and its value.
            index += 2;
            continue;
        }
        return Err(CliError(format!(
            "unknown argument '{arg}'\n\nRun `matrisaver --help` for usage."
        )));
    }
    Ok(())
}

fn apply_settings_overrides(settings: &mut Settings, args: &[String]) -> Result<(), CliError> {
    if let Some(variant) = option_value(args, "--variant") {
        if matrisaver_core::config::variant_by_key(variant).is_none() {
            let keys: Vec<&str> = matrisaver_core::config::VARIANTS
                .iter()
                .map(|candidate| candidate.key)
                .collect();
            return Err(CliError(format!(
                "unknown --variant '{variant}'. Available: {}",
                keys.join(", ")
            )));
        }
        settings.variant = variant.to_owned();
    }
    if let Some(raw) = option_value(args, "--pipeline") {
        settings.pipeline =
            parse_pipeline(raw).ok_or_else(|| CliError(format!("unknown --pipeline '{raw}'")))?;
    }
    if let Some(raw) = option_value(args, "--glow-quality") {
        settings.glow_quality = parse_glow_quality(raw).ok_or_else(|| {
            CliError(format!(
                "unknown --glow-quality '{raw}'. Available: low, balanced, high"
            ))
        })?;
    }
    if let Some(raw) = option_value(args, "--char-size") {
        settings.char_size = raw
            .parse::<u16>()
            .map_err(|_| CliError(format!("--char-size expects a number, got '{raw}'")))?;
    }
    if has_flag(args, "--overlay") {
        settings.overlay_enabled = true;
    }
    if has_flag(args, "--no-overlay") {
        settings.overlay_enabled = false;
    }
    if has_flag(args, "--performance") {
        settings.performance_mode = true;
    }
    if has_flag(args, "--no-performance") {
        settings.performance_mode = false;
    }
    if has_flag(args, "--multi-monitor") {
        settings.multi_monitor = true;
    }
    if has_flag(args, "--single-monitor") {
        settings.multi_monitor = false;
    }
    Ok(())
}

fn parse_u32_value(flag: &str, raw: &str) -> Result<u32, CliError> {
    raw.parse::<u32>()
        .map_err(|_| CliError(format!("{flag} expects a number, got '{raw}'")))
}

pub fn parse_pipeline(raw: &str) -> Option<Pipeline> {
    [
        Pipeline::OpenGl,
        Pipeline::Cpu,
        Pipeline::CpuGlow,
        Pipeline::CodeStrips3D,
    ]
    .into_iter()
    .find(|candidate| candidate.key() == raw)
}

pub fn parse_glow_quality(raw: &str) -> Option<GlowQuality> {
    match raw {
        "low" => Some(GlowQuality::Low),
        "balanced" => Some(GlowQuality::Balanced),
        "high" => Some(GlowQuality::High),
        _ => None,
    }
}

pub fn glow_quality_key(value: GlowQuality) -> &'static str {
    match value {
        GlowQuality::Low => "low",
        GlowQuality::Balanced => "balanced",
        GlowQuality::High => "high",
    }
}

fn parse_gpu_selection(args: &[String]) -> GpuSelectionOptions {
    let explicit_backend = option_value(args, "--wgpu-backend")
        .or_else(|| option_value(args, "--backend"))
        .map(str::to_owned);
    let explicit_adapter = option_value(args, "--wgpu-adapter-name")
        .or_else(|| option_value(args, "--adapter-name"))
        .map(str::to_owned);
    GpuSelectionOptions::from_env().with_overrides(explicit_backend, explicit_adapter)
}

/// `--key value` or `--key=value`.
pub fn option_value<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    for (index, arg) in args.iter().enumerate() {
        if let Some(value) = arg.strip_prefix(key) {
            if let Some(value) = value.strip_prefix('=') {
                return Some(value);
            }
        }
        if arg == key {
            return args.get(index + 1).map(String::as_str);
        }
    }
    None
}

pub fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

#[cfg(test)]
mod tests {
    use super::{exit_code, has_flag, option_value, parse, parse_glow_quality, Mode};
    use matrisaver_core::config::GlowQuality;

    fn argv(values: &[&str]) -> Vec<String> {
        std::iter::once("matrisaver".to_owned())
            .chain(values.iter().map(|value| (*value).to_owned()))
            .collect()
    }

    #[test]
    fn defaults_to_fullscreen_run_mode() {
        let cli = parse(&argv(&[])).expect("parse");
        assert_eq!(cli.mode, Mode::Run);
        assert!(!cli.show_cursor);
    }

    #[test]
    fn benchmark_flag_selects_the_headless_path() {
        let cli = parse(&argv(&["--benchmark-frames", "240"])).expect("parse");
        assert_eq!(cli.mode, Mode::Benchmark { frames: 240 });

        let cli = parse(&argv(&["--benchmark-frames=120"])).expect("parse");
        assert_eq!(cli.mode, Mode::Benchmark { frames: 120 });
    }

    #[test]
    fn benchmark_frames_floor_is_one() {
        let cli = parse(&argv(&["--benchmark-frames=0"])).expect("parse");
        assert_eq!(cli.mode, Mode::Benchmark { frames: 1 });
    }

    #[test]
    fn listing_modes_win_over_playback() {
        let cli = parse(&argv(&["--run", "--list-monitors"])).expect("parse");
        assert_eq!(cli.mode, Mode::ListMonitors);

        let cli = parse(&argv(&["--list-adapters", "--window"])).expect("parse");
        assert_eq!(cli.mode, Mode::ListAdapters);
    }

    #[test]
    fn window_mode_is_explicit() {
        let cli = parse(&argv(&["--window"])).expect("parse");
        assert_eq!(cli.mode, Mode::Window);
    }

    #[test]
    fn help_short_and_long() {
        assert_eq!(parse(&argv(&["-h"])).expect("parse").mode, Mode::Help);
        assert_eq!(parse(&argv(&["--help"])).expect("parse").mode, Mode::Help);
    }

    #[test]
    fn monitor_scope_flags_drive_settings() {
        let cli = parse(&argv(&["--single-monitor"])).expect("parse");
        assert!(!cli.settings.multi_monitor);

        let cli = parse(&argv(&["--multi-monitor"])).expect("parse");
        assert!(cli.settings.multi_monitor);
    }

    #[test]
    fn overlay_flags_drive_settings() {
        let cli = parse(&argv(&["--no-overlay"])).expect("parse");
        assert!(!cli.settings.overlay_enabled);

        let cli = parse(&argv(&["--overlay"])).expect("parse");
        assert!(cli.settings.overlay_enabled);
    }

    #[test]
    fn known_variant_is_accepted() {
        let cli = parse(&argv(&["--variant", "reloaded"])).expect("parse");
        assert_eq!(cli.settings.variant, "reloaded");
    }

    #[test]
    fn unknown_variant_is_rejected_with_the_list() {
        let error = parse(&argv(&["--variant", "nebuchadnezzar"])).expect_err("should reject");
        assert!(error.0.contains("nebuchadnezzar"), "{}", error.0);
        assert!(error.0.contains("reloaded"), "{}", error.0);
    }

    #[test]
    fn unknown_glow_quality_is_rejected() {
        let error = parse(&argv(&["--glow-quality", "ultra"])).expect_err("should reject");
        assert!(error.0.contains("low, balanced, high"), "{}", error.0);
    }

    #[test]
    fn glow_quality_values_parse() {
        assert_eq!(parse_glow_quality("low"), Some(GlowQuality::Low));
        assert_eq!(parse_glow_quality("balanced"), Some(GlowQuality::Balanced));
        assert_eq!(parse_glow_quality("high"), Some(GlowQuality::High));
        assert_eq!(parse_glow_quality("none"), None);
    }

    #[test]
    fn non_numeric_dimension_is_rejected() {
        let error = parse(&argv(&["--width", "wide"])).expect_err("should reject");
        assert!(error.0.contains("--width"), "{}", error.0);
    }

    #[test]
    fn unknown_flag_is_rejected_not_ignored() {
        let error = parse(&argv(&["--turbo"])).expect_err("should reject");
        assert!(error.0.contains("--turbo"), "{}", error.0);
        assert!(error.0.contains("--help"), "{}", error.0);
    }

    #[test]
    fn value_of_a_value_flag_is_not_mistaken_for_a_flag() {
        parse(&argv(&["--lifecycle-trace", "/tmp/trace.log"])).expect("parse");
        parse(&argv(&["--adapter-name", "llvmpipe"])).expect("parse");
    }

    #[test]
    fn gpu_selection_flags_and_aliases() {
        let cli =
            parse(&argv(&["--wgpu-backend=gl", "--adapter-name", "llvmpipe"])).expect("parse");
        assert_eq!(cli.gpu_selection.backend.as_deref(), Some("gl"));
        assert_eq!(cli.gpu_selection.adapter_name.as_deref(), Some("llvmpipe"));
    }

    #[test]
    fn lifecycle_options_round_trip() {
        let cli = parse(&argv(&[
            "--lifecycle-frames",
            "600",
            "--lifecycle-trace",
            "/tmp/lc.log",
        ]))
        .expect("parse");
        assert_eq!(cli.lifecycle_frames, Some(600));
        assert_eq!(cli.lifecycle_trace.as_deref(), Some("/tmp/lc.log"));
    }

    #[test]
    fn option_value_supports_both_spellings() {
        let args = argv(&["--variant=bane"]);
        assert_eq!(option_value(&args, "--variant"), Some("bane"));

        let args = argv(&["--variant", "bane"]);
        assert_eq!(option_value(&args, "--variant"), Some("bane"));
        assert!(has_flag(&args, "--variant"));
    }

    #[test]
    fn exit_codes_are_distinct() {
        let codes = [
            exit_code::OK,
            exit_code::RUNTIME,
            exit_code::USAGE,
            exit_code::NO_GPU,
            exit_code::NO_SESSION,
        ];
        let mut sorted = codes.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), codes.len());
    }
}
