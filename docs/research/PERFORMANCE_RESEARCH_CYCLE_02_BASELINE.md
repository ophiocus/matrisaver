# Performance Research Cycle 02 Baseline

Related docs:

- Docs index: [`../README.md`](../README.md)
- Research cycle 01: [`PERFORMANCE_RESEARCH_CYCLE_01.md`](PERFORMANCE_RESEARCH_CYCLE_01.md)
- Rust workspace guide: [`../development/RUST_WORKSPACE.md`](../development/RUST_WORKSPACE.md)

## Objective

Establish a repeatable baseline measurement path for host stubs and core runtime timing while
bringing in atlas-textured instanced glyph rendering plus first reduced-resolution glow passes.

## Instrumentation Added

- `matrisaver-core::perf::FrameProfiler`
- `matrisaver-core::perf::FrameTimings`
- `matrisaver-core::perf::PerfSummary`
- `CoreRuntime::tick_profiled(...)`
- `CoreRuntime::performance_summary()`
- `renderer::build_instances(...)`
- `gpu::GpuRendererScaffold::draw_instanced_pass(...)`
- GPU atlas texture sampling in glyph fragment stage
- glow prefilter + separable blur + composite passes

All host stubs now support benchmark frame sampling via `--benchmark-frames` and print a
normalized `PERF ...` line for easy log scraping.

The benchmark line now includes p95 frame time (`p95_total_ms`) and supports explicit benchmark
resolution via `--width` and `--height` plus `glow_quality` metadata.

Optional probe controls are also available:

- `--list-adapters`
- `--gpu-scaffold` (offscreen `wgpu` atlas + instanced draw + glow path)
- `--glow-quality low|balanced|high`
- `--wgpu-backend <list>` (example: `gl`, `vulkan`, `dx12`, `vulkan,gl`)
- `--wgpu-adapter-name <substring>` (adapter-name contains match, case-insensitive)

Windows runtime note:

- Real Windows lifecycle runs (`/s` and `/p` without `--benchmark-frames`) now use on-window
  `wgpu` surface presentation with shared-device `matrisaver-core` scaffold output and
  `WM_SIZE`-driven surface reconfigure.
- Benchmark captures in this document remain sourced from the explicit benchmark path
  (`--benchmark-frames`), which is still the authoritative PERF/telemetry mode.

## Commands Used

```bash
source "$HOME/.cargo/env"
cargo run -q -p matrisaver-host-linux -- --gpu-scaffold --wgpu-backend gl --wgpu-adapter-name nvidia --glow-quality low --benchmark-frames 240 --width 1280 --height 720
cargo run -q -p matrisaver-host-linux -- --gpu-scaffold --wgpu-backend gl --wgpu-adapter-name nvidia --glow-quality low --benchmark-frames 240 --width 1920 --height 1080
cargo run -q -p matrisaver-host-linux -- --gpu-scaffold --wgpu-backend gl --wgpu-adapter-name nvidia --glow-quality low --benchmark-frames 240 --width 2560 --height 1440

cargo run -q -p matrisaver-host-linux -- --gpu-scaffold --wgpu-backend gl --wgpu-adapter-name nvidia --glow-quality balanced --benchmark-frames 240 --width 1280 --height 720
cargo run -q -p matrisaver-host-linux -- --gpu-scaffold --wgpu-backend gl --wgpu-adapter-name nvidia --glow-quality balanced --benchmark-frames 240 --width 1920 --height 1080
cargo run -q -p matrisaver-host-linux -- --gpu-scaffold --wgpu-backend gl --wgpu-adapter-name nvidia --glow-quality balanced --benchmark-frames 240 --width 2560 --height 1440

cargo run -q -p matrisaver-host-linux -- --gpu-scaffold --wgpu-backend gl --wgpu-adapter-name nvidia --glow-quality high --benchmark-frames 240 --width 1280 --height 720
cargo run -q -p matrisaver-host-linux -- --gpu-scaffold --wgpu-backend gl --wgpu-adapter-name nvidia --glow-quality high --benchmark-frames 240 --width 1920 --height 1080
cargo run -q -p matrisaver-host-linux -- --gpu-scaffold --wgpu-backend gl --wgpu-adapter-name nvidia --glow-quality high --benchmark-frames 240 --width 2560 --height 1440

cargo run -q -p matrisaver-host-macos -- --gpu-scaffold --wgpu-backend gl --wgpu-adapter-name nvidia --glow-quality balanced --benchmark-frames 240 --width 1920 --height 1080
cargo run -q -p matrisaver-host-windows -- /s --gpu-scaffold --wgpu-backend gl --wgpu-adapter-name nvidia --glow-quality balanced --benchmark-frames 240 --width 1920 --height 1080
```

## Captured Baseline (Atlas + Instanced Draw + Glow)

Linux matrix (`--gpu-scaffold`):

| Glow quality | Resolution | avg_total_ms | p95_total_ms | avg_draw_ms | avg_fps |
| --- | --- | ---: | ---: | ---: | ---: |
| low | 1280x720 | 5.6598 | 6.1092 | 5.6595 | 176.68 |
| low | 1920x1080 | 5.5112 | 5.8639 | 5.5109 | 181.45 |
| low | 2560x1440 | 5.5965 | 6.1576 | 5.5962 | 178.68 |
| balanced | 1280x720 | 5.4319 | 5.7518 | 5.4316 | 184.10 |
| balanced | 1920x1080 | 5.4796 | 6.0070 | 5.4793 | 182.49 |
| balanced | 2560x1440 | 5.6091 | 5.9811 | 5.6088 | 178.28 |
| high | 1280x720 | 5.1288 | 5.6913 | 5.1286 | 194.98 |
| high | 1920x1080 | 6.7037 | 5.8927 | 6.7034 | 149.17 |
| high | 2560x1440 | 5.6099 | 6.1373 | 5.6097 | 178.25 |

Cross-host spot check (balanced, 1920x1080, `--gpu-scaffold`, explicit adapter selection):

- Linux: `PERF host=linux mode=screensaver glow_quality=balanced selected_backend=Gl selected_adapter=D3D12__NVIDIA_GeForce_RTX_3070_Ti_ selected_device_type=Other width=1920 height=1080 frames=240 avg_total_ms=5.4796 p95_total_ms=6.0070 avg_update_ms=0.0000 avg_draw_ms=5.4793 avg_post_ms=0.0001 avg_fps=182.49`
- macOS host stub: `PERF host=macos mode=screensaver glow_quality=balanced selected_backend=Gl selected_adapter=D3D12__NVIDIA_GeForce_RTX_3070_Ti_ selected_device_type=Other width=1920 height=1080 frames=240 avg_total_ms=5.3252 p95_total_ms=5.5883 avg_update_ms=0.0000 avg_draw_ms=5.3250 avg_post_ms=0.0001 avg_fps=187.79`
- Windows host stub: `PERF host=windows mode=screensaver glow_quality=balanced selected_backend=Gl selected_adapter=D3D12__NVIDIA_GeForce_RTX_3070_Ti_ selected_device_type=Other width=1920 height=1080 frames=240 avg_total_ms=5.0465 p95_total_ms=5.5585 avg_update_ms=0.0000 avg_draw_ms=5.0463 avg_post_ms=0.0001 avg_fps=198.16`

## Interpretation

- These values include GPU-side per-frame instance-buffer upload, atlas-texture sampling, glow
  prefilter, separable blur, and composite submission.
- Deterministic adapter selection is now explicit in command lines and in `PERF` metadata via
  `selected_backend`, `selected_adapter`, and `selected_device_type`.
- On this WSL capture environment, `MESA: ZINK failed to choose pdev` and `libEGL ... failed to
  create dri2 screen` warnings still appear, but the selected adapter metadata confirms the run
  used the requested GL adapter path (`D3D12 (NVIDIA GeForce RTX 3070 Ti)`) rather than
  `llvmpipe` fallback.
- Frame-time quality/resolution scaling remains noisy under WSL translation and software stack
  variability, so this matrix should be interpreted as host-plumbing validation rather than final
  production performance characterization.

## Optional Probe Run (Linux Host)

Commands:

```bash
source "$HOME/.cargo/env"
cargo run -q -p matrisaver-host-linux -- --list-adapters --benchmark-frames 60 --width 1280 --height 720
cargo run -q -p matrisaver-host-linux -- --gpu-scaffold --wgpu-backend gl --wgpu-adapter-name nvidia --benchmark-frames 60 --width 1280 --height 720
cargo run -q -p matrisaver-host-linux -- --gpu-scaffold --wgpu-backend vulkan --wgpu-adapter-name llvmpipe --benchmark-frames 60 --width 1280 --height 720
```

Observed highlights:

- Adapter discovery included a CPU Vulkan adapter (`llvmpipe`) and a GL adapter path.
- Explicit `--wgpu-backend gl --wgpu-adapter-name nvidia` selection reports
  `selected_backend=Gl selected_adapter=D3D12__NVIDIA_GeForce_RTX_3070_Ti_` in `PERF` output.
- Explicit `--wgpu-backend vulkan --wgpu-adapter-name llvmpipe` selection reports
  `selected_backend=Vulkan selected_adapter=llvmpipe__... selected_device_type=Cpu`, validating
  deterministic routing in both directions.

## Next Benchmark Gate

For the next cycle, keep this same matrix and add backend and topology metadata from real target
machines (Windows-first priority):

1. backend in use
2. monitor topology
3. resolution
4. quality tier
5. mean plus p95 frame time
