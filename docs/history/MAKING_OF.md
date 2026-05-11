# MatriSaver: History of the Making

Related docs:

- Docs index: [`../README.md`](../README.md)
- Architecture: [`../architecture/ARCHITECTURE.md`](../architecture/ARCHITECTURE.md)
- Migration roadmap: [`../planning/ROADMAP.md`](../planning/ROADMAP.md)
- Rust workspace guide: [`../development/RUST_WORKSPACE.md`](../development/RUST_WORKSPACE.md)
- Performance cycle 01: [`../research/PERFORMANCE_RESEARCH_CYCLE_01.md`](../research/PERFORMANCE_RESEARCH_CYCLE_01.md)
- Performance cycle 02 baseline: [`../research/PERFORMANCE_RESEARCH_CYCLE_02_BASELINE.md`](../research/PERFORMANCE_RESEARCH_CYCLE_02_BASELINE.md)

This document captures how the project has been built so far and where it is headed, with
specific focus on the path to an installable Windows 11 screensaver.

## Why This Exists

MatriSaver started as a Python/Pygame implementation and is being migrated to a production Rust
architecture to support:

- native host integration on each OS
- multi-monitor as a must-have
- direct production implementation (no throwaway prototype branch)
- Windows-first delivery priorities
- unsigned builds initially, with a later signing path
- strict deferral to OS lock/session behavior

## Build History (Backfilled)

## Phase A: Baseline Product Shape (Python Era)

- Matrix-style digital rain variants and controls were established.
- Core visual language and behavior expectations became the parity target for migration.
- Product constraints were made explicit: lock behavior belongs to the OS, not app logic.

## Phase B: Rust Migration Foundation

- Rust workspace and crate boundaries were established:
  - `matrisaver-core`
  - `matrisaver-host-windows`
  - `matrisaver-host-macos`
  - `matrisaver-host-linux`
- Host contracts were scaffolded (`/s`, `/c`, `/p` semantics on Windows, equivalent stubs on
  macOS/Linux).
- Local-only telemetry and error stubs were retained to preserve privacy/safety defaults.

## Phase C: Instrumentation and Performance Research

- Frame profiling was added to core (`FrameProfiler`, `FrameTimings`, `PerfSummary`).
- Hosts gained normalized benchmark output (`PERF ...`) and benchmark controls.
- Research workflow moved to documentation-first gating:
  - cycle notes
  - reproducible commands
  - measured output artifacts

## Phase D: Rendering Milestone (Current Technical Step)

- GPU scaffold now includes:
  - atlas-textured instanced glyph rendering
  - reduced-resolution glow chain (prefilter, separable blur, composite)
- Deterministic GPU selection is now implemented in core and exposed through host flags:
  - env hints: `WGPU_BACKEND`, `WGPU_ADAPTER_NAME`
  - explicit host overrides: `--wgpu-backend`, `--wgpu-adapter-name`
- `PERF` output now includes selected adapter metadata:
  - `selected_backend`
  - `selected_adapter`
  - `selected_device_type`
- WSL captures still show zink/egl warnings, but routing is now auditable by metadata.

## Phase E: Focused Visual Parity Pass (Windows-First Rust)

This pass focused on closing visible behavioral gaps between the Python prototype renderer and
the Rust core runtime before investing in new UX surface area.

What changed in this pass:

- Original-variant rendering now preserves row-memory semantics with head/eraser lifecycle,
  volatile mutation, and ghost entities in Rust core.
- All runtime variants currently route through this same original lifecycle model, with
  per-variant mutators applied to speed/delete-gap/volatile/ghost cadence.
- Startup regressions were addressed so seeded visible rows appear immediately instead of waiting
  for first head-write cycles.
- Shader-side visual shaping now distinguishes normal, volatile, and ghost glyph channels using
  per-instance style tagging.
- Volatile glyphs now receive pulse/gamma-like emphasis, while ghosts are dimmed and color-shifted
  as a distinct layer.
- Variant mood is now more strongly expressed through runtime-driven style parameters
  (`vfx_gamma`, `head_bloom`, `ghost_chance`, `volatile_chance`, `vfx_glow_strength`) passed into
  GPU shading.
- Glyph atlas generation now rasterizes from bundled CJK font data (`assets/fonts/NotoSansCJK-Regular.ttc`)
  instead of placeholder cross/circle scaffold cells.
- Image-driven overlay injection is now wired into Rust runtime flow, including header animation,
  hold/release timing, and per-frame lifecycle trace counters.

What remains for full visual parity:

- Overlay polish is still pending for closer parity with Python undertext save/restore and fade
  blending behavior.
- Variant mutators are currently hardcoded in core; user-facing tuning/config surfacing is pending.

## Phase F: Windows Lifecycle and Overlay Integration (Recent)

- Windows host `/s` and `/p` now run through real Win32 message-loop lifecycle and on-window `wgpu`
  surface presentation (instead of benchmark-only behavior).
- `/p` parent handle parsing/validation is enforced for both decimal and hex formats.
- Lifecycle instrumentation gained frame-budget auto-exit (`--lifecycle-frames`) and appendable
  trace capture (`--lifecycle-trace-file`) for scripted smoke runs.
- Overlay flow was extended with primary-display anchoring updates and animated header intro
  defaults (`all_at_once`) to better match current visual intent.

## Recent Git Activity Snapshot (Feb 2026)

- `cad8d18`, `aa80729`, `41e37c1`: moved Windows host from stubs to real lifecycle presentation,
  then added frame-budget automation.
- `a5fa2bd`: unified core lifecycle path, variant mutators, and CJK atlas rasterization.
- `c38e64f`, `12271aa`, `0f8674a`: added overlay traceability, anchor alignment, and animated
  overlay-header defaults.
- `2b00da3`, `30edc7e`, `dad5d4e`: completed prototype launcher relocation and removed legacy root
  Python runtime tree.

## Where We Are Now

Current state is "partial production runtime with real Windows lifecycle plus benchmarkable cross-
host stubs," not yet "installed Win11 screensaver product."

What is complete:

- core render/profiling milestone
- benchmark command surface
- deterministic adapter/backend selection
- real Windows `/s` and `/p` lifecycle path with `wgpu` presentation
- image-driven overlay injection with lifecycle trace visibility
- docs and baseline benchmark backfill

What is not complete:

- real preview embedding polish and full multi-monitor production behavior
- device-lost/recreate hardening and long-run lifecycle soak confidence
- installer/registration path that appears in Windows Screen Saver Settings

## Roadmap: From Here to Installed Windows 11 Screensaver

## 1) Windows `.scr` Lifecycle Hardening

- harden `/s`, `/c`, `/p <HWND>` behavior for preview-host edge cases and lifecycle resilience
- complete preview embedding polish for the Screen Saver Settings preview pane
- validate device-lost/recreate and monitor-change behavior under long-running sessions

## 2) Production Rendering + Multi-Monitor Runtime

- move from scaffold-oriented flow into production presentation path
- harden multi-monitor topology handling (attach/detach/resolution/dpi changes)
- handle device-lost/recreate paths robustly

## 3) Settings UX and Persistence Integration

- provide practical `/c` configuration experience
- ensure one canonical persisted schema with migration safety
- validate host/runtime agreement on all settings fields

## 4) Packaging and Installation (Windows-First)

- produce Windows installer artifact (MSI target)
- install and register screensaver so it is discoverable in Win11 settings UX
- support upgrade and uninstall flows safely

## 5) Stabilization Gates

- real hardware matrix validation (GPU vendors, single/multi monitor, mixed DPI)
- soak/stability runs for idle/lock/unlock and long sessions
- performance gates captured with backend/adapter trace metadata

## 6) Release Hygiene (Applied)

- provide repeatable release checklist for unsigned distribution
- require install/preview/run/uninstall smoke validation per candidate build
- capture known caveats (for example WSL-only warnings) separately from production blockers

## 7) Operations and Support Readiness (Applied)

- document operator runbooks and troubleshooting paths for installer and runtime selection
- ensure docs stay synchronized with benchmark flags and output schema
- maintain a clear "what to collect" list for bug reports (host mode, adapter metadata, build id)

## Done Definition: Installed Win11 Screensaver

The migration reaches this milestone only when all points below are true:

1. Installer runs on Windows 11 and installs the screensaver artifacts correctly.
2. MatriSaver appears in Windows "Screen Saver Settings" and can be selected.
3. Preview mode (`/p`) renders correctly inside the native preview window.
4. Screensaver mode (`/s`) runs full-screen across target monitor setups and exits correctly on user input.
5. Uninstall removes binaries/integration cleanly without breaking system settings.
6. Release-smoke checklist passes (install -> select -> preview -> run -> uninstall) for each candidate build.
7. Support-readiness docs are current and include benchmark/adapter metadata capture guidance.
