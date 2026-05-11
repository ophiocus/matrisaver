# MatriSaver Native Architecture

Related docs:

- Docs index: [`../README.md`](../README.md)
- Roadmap: [`../planning/ROADMAP.md`](../planning/ROADMAP.md)
- Performance research: [`../research/PERFORMANCE_RESEARCH_CYCLE_01.md`](../research/PERFORMANCE_RESEARCH_CYCLE_01.md)
- Thin adapter boundary: [`thin-adapters/THIN_ADAPTERS.md`](thin-adapters/THIN_ADAPTERS.md)
- Overlay subsystem: [`OVERLAY_SUBSYSTEM.md`](OVERLAY_SUBSYSTEM.md)

This document defines the production architecture for migrating MatriSaver from Python/Pygame
to a formal, installable screensaver application.

## Goals

- Preserve complete visual and behavioral feature parity with the current Windows build.
- Deliver native screensaver integration per OS while sharing as much rendering/runtime logic as possible.
- Keep unsigned developer builds first, with a documented signing path for later.
- Support multi-monitor as a hard requirement.
- Always defer to OS lock-screen and session security behavior.

## Stack Decision

- Core language: Rust
- Shared rendering/runtime: `matrisaver-core` crate
- Platform host adapters:
  - `matrisaver-host-windows` (native `.scr` semantics)
  - `matrisaver-host-macos` (ScreenSaver host bridge)
  - `matrisaver-host-linux` (desktop-specific host bridge, starting with xscreensaver-style flow)

Why this split:

- It isolates OS lifecycle contracts from effect/render logic.
- It enables independent platform packaging while sharing one core engine.
- It keeps legal/security-sensitive host behavior explicit and testable.

## Repository Layout

```text
rust/
  Cargo.toml                 # Workspace manifest
  crates/
    matrisaver-core/         # Shared runtime and rendering abstraction
    matrisaver-host-windows/ # Windows .scr host entry
    matrisaver-host-macos/   # macOS host entry
    matrisaver-host-linux/   # Linux host entry
```

## Component Boundaries

### `matrisaver-core`

- Owns portable state and effect model.
- Exposes a host-agnostic `CoreRuntime` lifecycle:
  - `new(config)`
  - `tick(delta_time)`
  - `request_exit(reason)`
- Defines settings, telemetry/error-reporting stubs, and pipeline enums.
- No OS-specific APIs.

### Host Crates

- Parse host-specific launch modes.
- Control window creation and monitor enumeration.
- Translate input/session events into `CoreRuntime` lifecycle calls.
- Enforce lock-screen-safe exit behavior.
- Provide a config entry surface that maps to the same persisted settings schema.

## Packaging Targets

- Windows: MSI that installs screensaver host artifact and settings integration.
- macOS: draggable app/bundle distribution for unsigned dev flow.
- Linux: `.deb` package with post-install integration steps.

## Security and Policy

- Input exit does not bypass authentication or unlock session.
- Hosts must hand control back to OS session manager immediately.
- Telemetry and error reporting remain local-only stubs for now.

## Compatibility Strategy

- Prefer broad GPU/API compatibility in host defaults.
- Include runtime fallback policy in core config:
  - preferred GPU pipeline
  - conservative fallback pipeline
- Multi-monitor support is first-class in all hosts.
