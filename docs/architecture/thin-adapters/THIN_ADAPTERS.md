# Thin Adapter Architecture

Related docs:

- Native architecture: [`../ARCHITECTURE.md`](../ARCHITECTURE.md)
- Rust workspace guide: [`../../development/RUST_WORKSPACE.md`](../../development/RUST_WORKSPACE.md)

This document defines the "thin adapter" boundary used by MatriSaver hosts.

## Rule Of Ownership

- `matrisaver-core` owns runtime behavior and visual logic.
- Host crates own OS lifecycle wiring, native window/surface contracts, and process/session rules.
- Hosts should pass events to core and render core output, not re-implement effect logic.

## Shared Runtime Domains (Write Once)

- Overlay: image sampling, ASCII glyph mapping, lock/freeze behavior, intro timing, and injected cell bookkeeping.
- Lifecycle simulation: rain-column update cadence, mutators, ghost/volatile rules, and reset behavior.
- Trace state: normalized lifecycle counters and summary line formatting (`LIFECYCLE ...`).

These domains are implemented once in `matrisaver-core` and reused by all hosts.

## Host Adapter Responsibilities

- Parse host invocation mode (`/s`, `/p`, `/c`, or platform equivalent).
- Create and manage native windows and GPU surfaces.
- Translate OS/input/session messages into runtime tick/resize/exit requests.
- Persist or route trace output destinations (stdout/file), while keeping trace content format in core.

## Current File Layout

Core runtime logic is split from `lib.rs` into focused files under:

- `rust/crates/matrisaver-core/src/runtime/trace.rs`
- `rust/crates/matrisaver-core/src/runtime/overlay.rs`
- `rust/crates/matrisaver-core/src/runtime/lifecycle.rs`

`lib.rs` remains the crate root and composition surface, while runtime behavior is grouped by domain.

## Adapter Quality Gates

- No host-specific APIs in overlay/lifecycle/trace logic.
- No duplicated variant/overlay behavior across host crates.
- Host changes must not alter shared `LIFECYCLE` trace schema without updating core and docs together.
