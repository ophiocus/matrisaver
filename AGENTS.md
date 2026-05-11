# AGENTS.md

Project: Matrix-inspired digital rain screensaver in Rust (production) with a legacy Python/Pygame prototype.

Guidelines:
- Rust workspace lives in `rust/`; this is the primary codebase.
- Python prototype lives entirely in `prototype/` (code, venv, scripts). No Python files outside that directory.
- Store fonts or other binary assets in `assets/` and reference them by relative path.
- If you add tests, place them in `tests/` and run from the Rust workspace.
- In non-interactive shells, source Cargo env before Rust commands: `source "$HOME/.cargo/env"`.
- Prefer `./check_rust.sh` for workspace Rust validation from repo root.
- Platform workflow: Windows 11 is the base host OS, and this agent runs in WSL on that machine.
- Do all development and repository operations from this WSL host; run Windows-runtime validation on Win11 via PowerShell.
- Keep environment/setup tracking up to date in `docs/development/WSL_RUST_TOOLING.md` when tooling changes.
