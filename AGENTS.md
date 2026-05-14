# AGENTS.md

Project: Matrix-inspired digital rain screensaver in Rust. Ships as a
Windows `.scr` via MSI; Linux and macOS host stubs build but aren't
yet packaged.

Guidelines:
- Rust workspace lives in `rust/`; it is the entire codebase.
- Store fonts or other binary assets in `assets/` and reference them by relative path.
- If you add tests, place them in `tests/` (Rust integration) or alongside source as `#[cfg(test)] mod tests`.
- In non-interactive shells, source Cargo env before Rust commands: `source "$HOME/.cargo/env"`.
- Prefer `./check_rust.sh` for workspace Rust validation from repo root.
- Platform workflow: Windows 11 is the base host OS, and this agent runs in WSL on that machine.
- Do all development and repository operations from this WSL host; run Windows-runtime validation on Win11 via PowerShell.
- Keep environment/setup tracking up to date in `docs/development/WSL_RUST_TOOLING.md` when tooling changes.
