# MatriSaver Migration Roadmap

Related docs:

- Docs index: [`../README.md`](../README.md)
- Architecture: [`../architecture/ARCHITECTURE.md`](../architecture/ARCHITECTURE.md)
- Performance research: [`../research/PERFORMANCE_RESEARCH_CYCLE_01.md`](../research/PERFORMANCE_RESEARCH_CYCLE_01.md)

This roadmap reflects your constraints:

- Windows-first
- Complete feature parity with current build
- Direct production architecture (no throwaway prototype branch)
- Unsigned builds initially

## Milestone 0: Foundation (Current)

Status: Complete

Acceptance criteria:

- Rust workspace exists with shared core crate and three host crates.
- Core lifecycle and config schema compile.
- Local-only stubs exist for telemetry and error reporting.
- Architecture, signing, and packaging docs are in-repo.

## Milestone 1: Core Runtime Parity

Status: In Progress

Acceptance criteria:

- Shared runtime implements all current effects and variant behaviors.
- Runtime settings map to current feature set, including developer tuning controls.
- Multi-monitor layout abstraction exists in core-facing host API.
- Deterministic smoke tests validate effect state updates and settings serialization.

## Milestone 2: Windows Formal Screensaver

Acceptance criteria:

- Host supports `.scr` invocation modes: `/s`, `/c`, `/p <HWND>`.
- Full-screen run mode supports multi-monitor.
- Preview mode renders correctly in Windows screensaver preview window.
- Exit behavior always defers to OS lock/session behavior.
- MSI packaging installs, upgrades, and uninstalls cleanly.

## Milestone 3: Linux Packaging and Host Integration

Acceptance criteria:

- Linux host supports multi-monitor full run mode.
- `.deb` package installs runtime and integration helpers.
- Idle/lock integration documented and validated for target desktop(s).
- Fallback behavior for unsupported compositor/GPU paths is explicit.

## Milestone 4: macOS Host and Draggable Distribution

Acceptance criteria:

- macOS host bridge launches core runtime in screensaver-compatible lifecycle.
- Multi-monitor behavior validated.
- Unsigned draggable distribution artifact is buildable and documented.
- Security model defers completely to OS lock behavior.

## Milestone 5: Stabilization and Release Hygiene

Acceptance criteria:

- Crash/diagnostic local reports capture actionable state.
- Long-run soak tests complete without regressions.
- Backward compatibility matrix documented for GPU tiers and OS versions.
- Signing plan can be executed later without architectural changes.

## Minimum Test Matrix

- Single monitor and multi-monitor.
- Mixed-DPI setups where supported by OS.
- Integrated graphics and discrete graphics.
- Suspend/resume cycle.
- Lock/unlock cycle.
