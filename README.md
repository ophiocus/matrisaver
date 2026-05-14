# MatriSaver

![MatriSaver — digital rain banner](assets/branding/hero-rain.jpg)

<p align="center">
  <img src="assets/branding/logo.png" width="160" alt="MatriSaver logo" />
</p>

MatriSaver is a Matrix-inspired digital rain screensaver written in
native Rust + `wgpu`. Windows is the primary target (ships as an MSI
that installs `matrisaver.scr` into `System32`); Linux and macOS host
stubs build cleanly but aren't packaged yet.

## Status

- Windows host: full Win32 + `wgpu` lifecycle for `/s`, `/p`, and `/c` (egui settings dialog).
- Linux / macOS hosts: build, benchmark, and run headless; native screensaver integration TBD.
- Update flow: GitHub Releases API; in-app one-click install for the Windows MSI.

## Install on Windows

Download the latest MSI from the [releases page](https://github.com/ophiocus/matrisaver/releases/latest)
and run it. The installer:

- Drops `matrisaver.scr` into `%SystemRoot%\System32\` so Windows
  picks it up in Display Properties → Screen Saver Settings.
- Optionally creates a Start Menu shortcut "MatriSaver Settings" that
  opens the Screen Saver settings page directly.
- Offers a "launch now" checkbox on the final wizard page.

## Build from source

Requirements: Rust 1.95+, MSVC build tools on Windows.

```bash
cd rust
cargo build --release
```

Binary lands at `rust/target/release/matrisaver-host-windows.exe`. On
Windows, run `matrisaver-host-windows.exe /s` for a fullscreen test or
copy/rename to `%SystemRoot%\System32\matrisaver.scr` for a real
screensaver install.

A convenience launcher is provided at the repo root:

```
H:\matrisaver\run-screensaver.cmd
```

Double-click runs the local release build in screensaver mode.

## After installing the MSI — how to open Settings

The Windows-blessed path to a screensaver's configuration is **Display
Properties → Screen Saver Settings**, not a direct invocation of the
`.scr`. Two equivalent ways to get there:

1. **Start Menu → MatriSaver → "MatriSaver Settings"** (created by the
   MSI; opt-out checkbox on the install Customize page if you don't
   want it).
2. **Win+R → `control desk.cpl,,@screensaver`** → pick MatriSaver from
   the dropdown → **Settings…**

Either one opens the in-app egui settings dialog with variant pickers,
glow quality, glyph size, multi-monitor toggle, Import/Export of
`settings.json`, and the one-click "Install update" button when a
newer release is available.

Direct `.scr` invocation from PowerShell will hit Windows'
`scrfile`-association handler, which always routes to screensaver
mode (`%1 /S`) — `/c` gets dropped. There's no clean shell incantation
around that; use one of the two methods above.

## Documentation

Formal documentation tree: [`docs/README.md`](docs/README.md)

High-priority docs:

- Architecture: [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.md)
- Migration roadmap: [`docs/planning/ROADMAP.md`](docs/planning/ROADMAP.md)
- Performance research cycle: [`docs/research/PERFORMANCE_RESEARCH_CYCLE_01.md`](docs/research/PERFORMANCE_RESEARCH_CYCLE_01.md)
- Rust workspace/development: [`docs/development/RUST_WORKSPACE.md`](docs/development/RUST_WORKSPACE.md)
