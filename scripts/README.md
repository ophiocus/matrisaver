# scripts/

Pre-flight gate runner and release helpers for matrisaver.

## `preflight.{ps1,sh}`

Runs the same four gates `.github/workflows/ci.yml` runs, in the same
order, with the same flags. **Run this before every push to master and
before every tag push.** If a gate fails locally, CI would have failed
too — cheaper to find out here than after a tag push.

```pwsh
# Windows / PowerShell
pwsh scripts/preflight.ps1
```

```sh
# Git Bash / WSL / Linux / macOS
bash scripts/preflight.sh
```

The four gates:

1. `cargo fmt --check`
2. `cargo clippy --workspace --all-targets --release -- -D warnings`
3. `cargo test --workspace --release`
4. `cargo build --release` (workspace sanity — release.yml will rebuild
   per host crate)

### Why `--release` matters

ci.yml uses `--release` on clippy and test. So does this script. Local
debug-mode runs can pass while release-mode CI fails: different
optimisation passes surface different lints (e.g. dead-code-elimination
warnings) and tests that depend on release-mode `cfg!(debug_assertions)`
behaviour will diverge. Always mirror CI's flags.

### Why this script exists

v0.3.0 was tag-pushed without a `cargo fmt --check`. Release CI fired
on the tag, the gate failed on all three platforms, and we burned the
recovery window on a tag reflog (the artifact-less first push) +
re-trigger. Running `pwsh scripts/preflight.ps1` before that push
would have caught it locally in under a minute.

## Two-phase push protocol (release procedure)

When shipping a new version:

1. **Local pre-flight** — `pwsh scripts/preflight.ps1`. All four
   gates must be green.
2. **Bump `rust/Cargo.toml`** to the new version.
3. **Re-run pre-flight** after the version bump (`cargo build`
   re-compiles, lockfile updates).
4. **Commit + push master** — *do not push the tag yet*.
5. **Watch ci.yml** on the master push:
   `gh run watch $(gh run list --workflow=ci.yml --branch=master --limit=1 --json databaseId --jq '.[0].databaseId')`
   — must conclude `success`.
6. **Now tag** — `git tag -a vX.Y.Z -m "..." && git push origin vX.Y.Z`.
7. **Watch release.yml** — should succeed on all three platforms and
   publish artifacts.

### The carve-out

If release.yml fails AND no artifacts published AND no newer tag
exists, you can reflog the tag onto a fix commit instead of bumping
the version. See `~/.claude/projects/H--matrisaver/memory/release_policy.md`
for the full rule and when it does/doesn't apply.
