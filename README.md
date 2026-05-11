# MatriSaver

![MatriSaver — digital rain banner](assets/branding/hero-rain.jpg)

<p align="center">
  <img src="assets/branding/logo.png" width="160" alt="MatriSaver logo" />
</p>

MatriSaver is a Matrix-inspired digital rain screensaver project in active migration from
Python/Pygame to a native Rust multi-platform architecture.

## Current Status

- Runtime prototype: Python/Pygame (`prototype/src/`)
- Production migration: Rust workspace (`rust/`)
- Native Windows host lifecycle (`/s` and `/p`) now runs through a real Win32 + `wgpu` path on Windows builds; Linux/macOS hosts remain benchmark-oriented stubs.
- Target packaging:
  - Windows: `MSI`
  - macOS: draggable app/bundle
  - Linux: `.deb`

## Quick Start (Current Python Runtime)

Requirements:

- Python 3.12+
- Pygame
- Font with katakana support (bundled: `assets/fonts/NotoSansCJK-Regular.ttc`)

Install:

```bash
sudo apt update
sudo apt install -y python3 python3-pip fonts-noto-cjk
python3 -m pip install --user -r prototype/requirements.txt
```

Run:

```bash
./prototype/bin/run.sh
```

Variant examples:

```bash
./prototype/bin/run.sh --variant original
./prototype/bin/run.sh --variant reloaded
./prototype/bin/run.sh --variant revolutions
./prototype/bin/run.sh --variant resurrections
```

Useful runtime flags:

- `--single-display`
- `--enable-overlay`
- `--font /path/to/font.ttc`

## Windows Launchers

- One-time setup: `prototype/bin/setup_windows.ps1`
- WSL launcher: `prototype/bin/run_wsl_windows.ps1`
- Native Windows multi-monitor launcher: `prototype/bin/run_windows_multi.ps1`

## Documentation

Formal documentation tree: [`docs/README.md`](docs/README.md)

High-priority docs:

- Architecture: [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.md)
- Migration roadmap: [`docs/planning/ROADMAP.md`](docs/planning/ROADMAP.md)
- Performance research cycle: [`docs/research/PERFORMANCE_RESEARCH_CYCLE_01.md`](docs/research/PERFORMANCE_RESEARCH_CYCLE_01.md)
- Rust workspace/development: [`docs/development/RUST_WORKSPACE.md`](docs/development/RUST_WORKSPACE.md)
