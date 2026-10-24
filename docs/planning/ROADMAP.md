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

Status: Mostly complete — see the per-criterion notes. Reference docs:
[`../development/LINUX_HOST.md`](../development/LINUX_HOST.md) and
[`../release/LINUX_PACKAGING.md`](../release/LINUX_PACKAGING.md).

Acceptance criteria:

- Linux host supports multi-monitor full run mode. **Implemented**: one
  scene rendered at the union bounding box of every output, with one
  borderless-fullscreen window per output presenting its own UV
  sub-rectangle, so the cascade stays continuous across a bezel. Covered
  by unit tests for side-by-side, stacked, negative-origin, mixed-
  resolution, no-primary and oversized-span layouts; **not yet run
  against two physical monitors** (the WSLg test host reports one
  output).
- `.deb` package installs runtime and integration helpers. **Done**:
  `scripts/package-deb.sh`, lintian-clean, built on Ubuntu 24.04 and
  install/remove verified on Debian 13.
- Idle/lock integration documented and validated for target desktop(s).
  **Documented, not validated**: helpers and a systemd user unit ship
  for swayidle, xautolock and xss-lock, with GNOME/KDE directed to their
  own idle management and xscreensaver `programs:` marked unsupported
  (no `-window-id` protocol). Validation needs a real desktop session;
  WSLg is not one.
- Fallback behavior for unsupported compositor/GPU paths is explicit.
  **Done**: session detection with a named exit code (4) and actionable
  guidance, an ordered Vulkan-then-GL backend ladder with a `FALLBACK`
  line per demotion and exit code 3 on total failure, and layout
  demotion to primary-only when a span exceeds the texture limit. One
  open question: the GL rung segfaults during teardown under WSLg after
  rendering every frame correctly; cause not settled (see the Linux host
  doc).

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
