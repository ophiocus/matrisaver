use matrisaver_core::config::{GlowQuality, Settings};
use matrisaver_core::gpu::GpuSelectionOptions;
use matrisaver_core::CoreRuntime;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let benchmark_frames = parse_benchmark_frames(&args).max(1);
    let list_adapters = has_flag(&args, "--list-adapters");
    let enable_gpu_scaffold = has_flag(&args, "--gpu-scaffold");
    let gpu_selection = parse_gpu_selection(&args);
    let mut settings = Settings::default();
    if let Some(glow_quality) =
        parse_option_value(&args, "--glow-quality").and_then(parse_glow_quality)
    {
        settings.glow_quality = glow_quality;
    }
    let mut runtime = CoreRuntime::new(settings);
    runtime.set_gpu_selection(gpu_selection);
    let width = parse_dimension(&args, "--width", 1920);
    let height = parse_dimension(&args, "--height", 1080);
    runtime.set_surface_size(width, height);
    if list_adapters {
        for adapter in runtime.adapter_snapshots() {
            println!(
                "ADAPTER host=linux name={} backend={} device_type={}",
                adapter.name, adapter.backend, adapter.device_type
            );
        }
    }
    if enable_gpu_scaffold {
        if let Err(error) = runtime.enable_gpu_scaffold() {
            eprintln!("Failed to enable GPU scaffold: {error}");
        }
    }
    run_runtime_benchmark("linux", &mut runtime, benchmark_frames, width, height);

    println!(
        "Linux host stub ready. Next step: wire idle/lock integration and .deb packaging glue."
    );
}

fn parse_benchmark_frames(args: &[String]) -> u32 {
    for (index, arg) in args.iter().enumerate() {
        if let Some(raw) = arg.strip_prefix("--benchmark-frames=") {
            if let Ok(value) = raw.parse::<u32>() {
                return value;
            }
        }
        if arg == "--benchmark-frames" {
            if let Some(raw) = args.get(index + 1) {
                if let Ok(value) = raw.parse::<u32>() {
                    return value;
                }
            }
        }
    }
    1
}

fn run_runtime_benchmark(
    host: &str,
    runtime: &mut CoreRuntime,
    frames: u32,
    width: u32,
    height: u32,
) {
    for _ in 0..frames {
        runtime.tick_profiled(1.0 / 60.0);
    }

    if let Some(summary) = runtime.performance_summary() {
        let adapter = runtime.selected_adapter_snapshot();
        let backend = adapter.map_or("none", |value| value.backend.as_str());
        let adapter_name = adapter.map_or("none", |value| value.name.as_str());
        let device_type = adapter.map_or("none", |value| value.device_type.as_str());
        println!(
            "PERF host={} mode=screensaver glow_quality={} selected_backend={} selected_adapter={} selected_device_type={} width={} height={} frames={} avg_total_ms={:.4} p95_total_ms={:.4} avg_update_ms={:.4} avg_draw_ms={:.4} avg_post_ms={:.4} avg_fps={:.2}",
            host,
            glow_quality_key(runtime.settings().glow_quality),
            perf_token(backend),
            perf_token(adapter_name),
            perf_token(device_type),
            width,
            height,
            summary.frame_count,
            summary.avg_total_ms,
            summary.p95_total_ms,
            summary.avg_update_ms,
            summary.avg_draw_ms,
            summary.avg_post_process_ms,
            summary.avg_fps
        );
    }
}

fn parse_option_value<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    for (index, arg) in args.iter().enumerate() {
        if let Some(value) = arg.strip_prefix(&format!("{key}=")) {
            return Some(value);
        }
        if arg == key {
            return args.get(index + 1).map(String::as_str);
        }
    }
    None
}

fn parse_glow_quality(raw: &str) -> Option<GlowQuality> {
    match raw {
        "low" => Some(GlowQuality::Low),
        "balanced" => Some(GlowQuality::Balanced),
        "high" => Some(GlowQuality::High),
        _ => None,
    }
}

fn parse_gpu_selection(args: &[String]) -> GpuSelectionOptions {
    let explicit_backend = parse_option_value(args, "--wgpu-backend")
        .or_else(|| parse_option_value(args, "--backend"))
        .map(str::to_owned);
    let explicit_adapter = parse_option_value(args, "--wgpu-adapter-name")
        .or_else(|| parse_option_value(args, "--adapter-name"))
        .map(str::to_owned);
    GpuSelectionOptions::from_env().with_overrides(explicit_backend, explicit_adapter)
}

fn perf_token(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn glow_quality_key(value: GlowQuality) -> &'static str {
    match value {
        GlowQuality::Low => "low",
        GlowQuality::Balanced => "balanced",
        GlowQuality::High => "high",
    }
}

fn parse_dimension(args: &[String], key: &str, fallback: u32) -> u32 {
    for (index, arg) in args.iter().enumerate() {
        if let Some(raw) = arg.strip_prefix(&format!("{key}=")) {
            if let Ok(value) = raw.parse::<u32>() {
                return value.max(1);
            }
        }
        if arg == key {
            if let Some(raw) = args.get(index + 1) {
                if let Ok(value) = raw.parse::<u32>() {
                    return value.max(1);
                }
            }
        }
    }
    fallback
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

#[cfg(test)]
mod tests {
    use super::{parse_benchmark_frames, parse_glow_quality, parse_gpu_selection};
    use matrisaver_core::config::GlowQuality;

    #[test]
    fn parses_benchmark_frames_inline_or_separate() {
        let args = vec!["app".to_string(), "--benchmark-frames=120".to_string()];
        assert_eq!(parse_benchmark_frames(&args), 120);

        let args = vec![
            "app".to_string(),
            "--benchmark-frames".to_string(),
            "240".to_string(),
        ];
        assert_eq!(parse_benchmark_frames(&args), 240);
    }

    #[test]
    fn parses_glow_quality_values() {
        assert_eq!(parse_glow_quality("low"), Some(GlowQuality::Low));
        assert_eq!(parse_glow_quality("balanced"), Some(GlowQuality::Balanced));
        assert_eq!(parse_glow_quality("high"), Some(GlowQuality::High));
        assert_eq!(parse_glow_quality("none"), None);
    }

    #[test]
    fn parses_explicit_gpu_selection_flags() {
        let args = vec![
            "app".to_string(),
            "--wgpu-backend=gl".to_string(),
            "--wgpu-adapter-name".to_string(),
            "nvidia".to_string(),
        ];
        let selection = parse_gpu_selection(&args);
        assert_eq!(selection.backend.as_deref(), Some("gl"));
        assert_eq!(selection.adapter_name.as_deref(), Some("nvidia"));
    }
}
