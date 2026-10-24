# AGENTS.md

Project: Matrix-inspired digital rain screensaver in Rust. Ships as a
Windows `.scr` via MSI and a Linux `.deb`; the macOS host builds but
isn't packaged yet.

Guidelines:
- Rust workspace lives in `rust/`; it is the entire codebase.
- Store fonts or other binary assets in `assets/` and reference them by relative path.
- If you add tests, place them in `tests/` (Rust integration) or alongside source as `#[cfg(test)] mod tests`.
- In non-interactive shells, source Cargo env before Rust commands: `source "$HOME/.cargo/env"`.
- Prefer `./check_rust.sh` for workspace Rust validation from repo root.
- Platform workflow: Windows 11 is the base host OS, and this agent runs in WSL on that machine.
- Do all development and repository operations from this WSL host; run Windows-runtime validation on Win11 via PowerShell.
- Keep environment/setup tracking up to date in `docs/development/WSL_RUST_TOOLING.md` when tooling changes.
- Shared present passes (flat blit, 3D compositor) live in
  `matrisaver-core::present`, not in a host crate. A host owns windows,
  surfaces and session rules; it does not own how a scene reaches a
  surface. See `docs/architecture/thin-adapters/THIN_ADAPTERS.md`.
- Linux playback: `docs/development/LINUX_HOST.md`. Packaging:
  `scripts/package-deb.sh` and `docs/release/LINUX_PACKAGING.md`.
- Run the CI gates on **Linux** before claiming green: the three-OS
  matrix means a `#[cfg]` slip or a dead-code warning can pass on
  Windows and fail on ubuntu-latest.
