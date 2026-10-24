//! Linux host entry point.
//!
//! Three jobs, split by mode:
//!
//! * **Playback** (`--run`, `--window`) — real windows, real surfaces.
//!   Lives in [`linux_host`], compiled only on Linux because that is
//!   where the windowing dependencies apply; the crate still builds on
//!   the other CI legs, matching how `matrisaver-host-windows` gates its
//!   Win32 module.
//! * **Headless benchmark** (`--benchmark-frames N`) — the behaviour this
//!   binary had before Milestone 3, unchanged and still portable. It is
//!   the only mode that works with no compositor, which makes it the
//!   fallback the no-session message points at.
//! * **Diagnostics** (`--list-adapters`, `--list-monitors`, `--help`).

// Off Linux this binary is a stub: `linux_host` is compiled out, so
// everything that exists only to serve playback — the scene layout, the
// session probe, the playback-only CLI fields, the playback exit codes —
// is legitimately unread, and `cargo clippy -D warnings` would reject
// the macOS and Windows CI legs for it. Scoped to not(linux) so the leg
// that actually builds playback still enforces dead-code warnings.
//
// layout and session stay ungated rather than being cfg'd away with
// linux_host: they are pure arithmetic and environment parsing with
// tests worth running on every leg.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

mod cli;
mod layout;
mod session;

#[cfg(target_os = "linux")]
mod linux_host;

use cli::{exit_code, glow_quality_key, Mode};
use matrisaver_core::CoreRuntime;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let parsed = match cli::parse(&args) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(exit_code::USAGE);
        }
    };

    let code = match parsed.mode {
        Mode::Help => {
            println!("{}", cli::USAGE);
            exit_code::OK
        }
        Mode::Benchmark { frames } => run_benchmark(&parsed, &args, frames),
        other => dispatch_platform(parsed, other),
    };

    std::process::exit(code);
}

#[cfg(target_os = "linux")]
fn dispatch_platform(parsed: cli::Cli, _mode: Mode) -> i32 {
    linux_host::run(parsed)
}

/// The crate is a workspace member on every platform (CI builds all four
/// crates on all three runners), so the non-Linux build needs an answer
/// for the playback modes. Adapter enumeration is portable, so it still
/// works; windows and monitors do not.
#[cfg(not(target_os = "linux"))]
fn dispatch_platform(parsed: cli::Cli, mode: Mode) -> i32 {
    match mode {
        Mode::ListAdapters => {
            let mut runtime = CoreRuntime::new(parsed.settings.clone());
            runtime.set_gpu_selection(parsed.gpu_selection.clone());
            for adapter in runtime.adapter_snapshots() {
                println!(
                    "ADAPTER host=linux name={} backend={} device_type={}",
                    adapter.name, adapter.backend, adapter.device_type
                );
            }
            exit_code::OK
        }
        _ => {
            eprintln!(
                "This binary only presents on Linux. Use matrisaver-host-windows or \
                 matrisaver-host-macos on this platform, or --benchmark-frames N for the \
                 portable headless path."
            );
            exit_code::RUNTIME
        }
    }
}

/// Headless path. Identical output contract to the pre-Milestone-3 stub:
/// one `ADAPTER` line per adapter when asked, then one `PERF` line.
fn run_benchmark(parsed: &cli::Cli, args: &[String], frames: u32) -> i32 {
    let mut runtime = CoreRuntime::new(parsed.settings.clone());
    runtime.set_gpu_selection(parsed.gpu_selection.clone());
    runtime.set_surface_size(parsed.width, parsed.height);

    if cli::has_flag(args, "--gpu-scaffold") {
        if let Err(error) = runtime.enable_gpu_scaffold() {
            eprintln!("Failed to enable GPU scaffold: {error}");
        }
    }

    for _ in 0..frames {
        runtime.tick_profiled(1.0 / 60.0);
    }

    print_perf_line(&runtime, parsed.width, parsed.height);
    exit_code::OK
}

fn print_perf_line(runtime: &CoreRuntime, width: u32, height: u32) {
    let Some(summary) = runtime.performance_summary() else {
        return;
    };
    let adapter = runtime.selected_adapter_snapshot();
    let backend = adapter.map_or("none", |value| value.backend.as_str());
    let adapter_name = adapter.map_or("none", |value| value.name.as_str());
    let device_type = adapter.map_or("none", |value| value.device_type.as_str());
    println!(
        "PERF host=linux mode=screensaver glow_quality={} selected_backend={} selected_adapter={} selected_device_type={} width={} height={} frames={} avg_total_ms={:.4} p95_total_ms={:.4} avg_update_ms={:.4} avg_draw_ms={:.4} avg_post_ms={:.4} avg_fps={:.2}",
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

#[cfg(test)]
mod tests {
    use super::perf_token;

    #[test]
    fn perf_tokens_are_sanitized() {
        assert_eq!(perf_token("NVIDIA GeForce RTX"), "NVIDIA_GeForce_RTX");
        assert_eq!(perf_token("llvmpipe (LLVM 19)"), "llvmpipe__LLVM_19_");
        assert_eq!(perf_token("gl-1.0_x.y"), "gl-1.0_x.y");
    }
}
