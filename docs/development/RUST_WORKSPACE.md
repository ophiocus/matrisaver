# Rust Migration Workspace

Related docs:

- Docs index: [`../README.md`](../README.md)
- WSL tooling setup: [`WSL_RUST_TOOLING.md`](WSL_RUST_TOOLING.md)
- Architecture: [`../architecture/ARCHITECTURE.md`](../architecture/ARCHITECTURE.md)

This workspace is the production migration target for MatriSaver.

## Crates

- `matrisaver-core`: shared runtime lifecycle, settings model, and local-only diagnostics stubs.
- `matrisaver-host-windows`: Windows `.scr` host with real Win32 lifecycle path for `/s` and `/p`, plus `/c` settings persistence.
- `matrisaver-host-macos`: macOS host bridge scaffold.
- `matrisaver-host-linux`: Linux host integration scaffold.

## Local Build

```bash
./check_rust.sh
```

This script delegates to `rust/check.sh`, which sources `rust/env.sh` first so it works even
when non-interactive shells do not already have Cargo on `PATH`.

## Windows Host Config Mode

You can exercise `/c` settings persistence behavior from the Windows host crate now:

```bash
source "$HOME/.cargo/env"
cargo run -p matrisaver-host-windows -- /c --variant reloaded --pipeline cpu_glow --glow-quality high --overlay --char-size 28
```

Settings are saved via `matrisaver-core::storage` to:

- `$MATRISAVER_SETTINGS_PATH` (if set), otherwise
- `$XDG_CONFIG_HOME/matrisaver/settings.json`, otherwise
- `$HOME/.config/matrisaver/settings.json`

## Windows Preview Mode Contract

- `/p <HWND>` and `/p:<HWND>` are parsed and validated.
- Decimal and hex HWND values are accepted (example: `4242` or `0x1092`).
- Preview mode without a valid HWND exits with code `2`.
- On Windows builds (target OS = Windows), `/s` and `/p` now enter a real Win32 message-loop
  lifecycle path instead of benchmark-only stubs.
- The Windows lifecycle path now creates a real `wgpu` surface from the runtime `HWND`,
  initializes `matrisaver-core` GPU scaffold rendering on that same device/queue, and presents
  scaffold output each timer tick (`WM_TIMER`) for both `/s` (top-level fullscreen) and `/p`
  (embedded child window).
- `WM_SIZE` now drives runtime surface-size updates plus `wgpu` surface reconfiguration.
- `/s` still exits on keyboard or mouse input; `/p` still preserves embedded preview semantics.
- `--lifecycle-frames <n>` can be used in lifecycle mode to auto-exit after `n` timer ticks
  (useful for non-interactive validation in CI/scripting).
- `--lifecycle-trace-file <path>` writes per-frame lifecycle counters to a host-local file,
  creates parent directories when needed, and appends across runs.
- `--gpu-scaffold` remains benchmark-mode-only. Lifecycle mode now uses shared-device scaffold
  presentation by default to stay aligned with runtime parity work.
- On non-Windows builds, use `--benchmark-frames` when invoking this host crate.

## Overlay Follow-ups

- Keep overlay intro style configurable in persisted settings (future):
  `overlay_intro_mode = all_at_once | wave_left_to_right`.
- Current runtime behavior injects image-driven ASCII overlays from `assets/overlays/`, with
  animated per-column headers and `all_at_once` startup as the active default.
- The alternate `wave_left_to_right` intro path is implemented in runtime internals but is not yet
  exposed through persisted settings/UI plumbing.

## Windows Runtime Validation (PowerShell)

From `H:\matrisaver\rust` on the Win11 host:

```powershell
cargo test -p matrisaver-host-windows
cargo build -p matrisaver-host-windows --release

# Settings mode still persists values.
cargo run -p matrisaver-host-windows -- /c --variant reloaded --pipeline cpu_glow --glow-quality high --overlay --char-size 28

# Benchmark/PERF path remains separate from real lifecycle.
cargo run -q -p matrisaver-host-windows -- /s --gpu-scaffold --wgpu-backend gl --wgpu-adapter-name nvidia --glow-quality balanced --benchmark-frames 240 --width 1920 --height 1080

# Real on-window lifecycle path (no --benchmark-frames).
.\target\release\matrisaver-host-windows.exe /s
.\target\release\matrisaver-host-windows.exe /p 4242

# Non-interactive lifecycle smoke run (auto-exits after frame budget).
.\target\release\matrisaver-host-windows.exe /s --lifecycle-frames 120

# Lifecycle trace capture.
.\target\release\matrisaver-host-windows.exe /s --lifecycle-trace-file H:\matrisaver\artifacts\lifecycle_trace.log
```

Note: `/p` requires a real parent preview window handle from the shell host; the numeric sample
above is a placeholder invocation format.

## Runtime Benchmark Stub Path

Use benchmark sampling to validate timing plumbing and log format:

```bash
source "$HOME/.cargo/env"
cargo run -q -p matrisaver-host-linux -- --gpu-scaffold --wgpu-backend gl --wgpu-adapter-name nvidia --glow-quality balanced --benchmark-frames 240 --width 2560 --height 1440
cargo run -q -p matrisaver-host-macos -- --gpu-scaffold --wgpu-backend gl --wgpu-adapter-name nvidia --glow-quality balanced --benchmark-frames 240 --width 2560 --height 1440
cargo run -q -p matrisaver-host-windows -- /s --gpu-scaffold --wgpu-backend gl --wgpu-adapter-name nvidia --glow-quality balanced --benchmark-frames 240 --width 2560 --height 1440
```

Each host prints a normalized line starting with `PERF ...`.
The line includes `glow_quality`, selected adapter metadata (`selected_backend`,
`selected_adapter`, `selected_device_type`), `width`, and `height` for matrix-style benchmark
capture.

Optional GPU/adapter probe flags:

- `--list-adapters`: prints discovered adapters and backend/device type.
- `--gpu-scaffold`: enables offscreen atlas-textured instanced draw with reduced-res glow passes.
- `--glow-quality low|balanced|high`: selects downsample quality tier for benchmark runs.
- `--wgpu-backend <list>`: backend constraint (`gl`, `vulkan`, `dx12`, etc.).
- `--wgpu-adapter-name <substring>`: adapter-name contains match (case-insensitive).

If host flags are not provided, core still reads `WGPU_BACKEND` and `WGPU_ADAPTER_NAME`
environment hints.

Benchmark baseline capture is documented in:

- [`../research/PERFORMANCE_RESEARCH_CYCLE_02_BASELINE.md`](../research/PERFORMANCE_RESEARCH_CYCLE_02_BASELINE.md)
