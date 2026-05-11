# AGENTS.md — prototype/

This directory is museum-grade code. Do not touch it.

The Python/Pygame prototype served its purpose as a reference implementation during
the early design phase. All active development now lives in `rust/`.

## Rules for agents

- Do not read files in this directory to inform Rust development decisions.
- Do not suggest porting, refactoring, or "improving" anything here.
- Do not run, benchmark, or test anything in this directory.
- Do not treat any behavior observed here as a specification.
- If a user asks you to modify something in `prototype/`, redirect them to `rust/`.

The Rust implementation is the sole source of truth.
