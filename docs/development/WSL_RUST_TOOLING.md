# WSL Rust Tooling Setup

Related docs:

- Docs index: [`../README.md`](../README.md)
- Rust workspace guide: [`RUST_WORKSPACE.md`](RUST_WORKSPACE.md)

This document tracks Rust-related tooling installed in this WSL host for MatriSaver development.

## Host

- OS: Ubuntu 24.04.1 LTS (WSL2)
- Arch: x86_64

## Installed Tooling

### User-Local Toolchain (rustup)

Installed with:

```bash
curl https://sh.rustup.rs -sSf | sh -s -- -y --profile default
source "$HOME/.cargo/env"
rustup default stable
```

Validated versions:

- `rustc 1.93.1`
- `cargo 1.93.1`
- `rustfmt 1.8.0-stable`
- `clippy 0.1.93`

### System Dependencies (apt)

Installed packages:

- `build-essential`
- `pkg-config`
- `libssl-dev`
- `cmake`

## Shell Notes

- Rustup configured `$HOME/.profile` and `$HOME/.bashrc` to source `$HOME/.cargo/env`.
- In non-interactive command runners, explicitly source cargo env if needed:

```bash
source "$HOME/.cargo/env"
```

- Repo helper scripts also enforce PATH bootstrap automatically:
  - `rust/env.sh`
  - `rust/check.sh`
  - `./check_rust.sh` (repo-root convenience wrapper)

## Verification Command

Workspace check:

```bash
./check_rust.sh
```

Last verified result: success for all workspace crates.
