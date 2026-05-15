#!/usr/bin/env bash
# Pre-flight gate runner — mirrors .github/workflows/ci.yml exactly.
#
# Run this BEFORE pushing master, BEFORE tagging a release. If any
# gate fails locally, CI would have failed too. Cheaper to find out
# here.
#
# The gate list (and `--release` flag) must stay aligned with ci.yml
# and release.yml. The whole point of this script is to take the
# human "did I remember to run all the gates" question out of the
# loop.
#
# Usage (from repo root or anywhere):
#   bash scripts/preflight.sh
#
# Exit code 0 = all green. Non-zero = at least one gate failed.

set -euo pipefail

# Resolve repo root from script location, then jump into rust/.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT/rust"

step() {
  local name="$1"
  shift
  printf '\n\e[36m═══ %s ═══\e[0m\n' "$name"
  if ! "$@"; then
    printf '\e[31mFAILED: %s\e[0m\n' "$name"
    exit 1
  fi
}

# Mirror of .github/workflows/ci.yml gate list. KEEP IN SYNC.
step 'cargo fmt --check' \
  cargo fmt --check

step 'cargo clippy --workspace --all-targets --release -- -D warnings' \
  cargo clippy --workspace --all-targets --release -- -D warnings

step 'cargo test --workspace --release' \
  cargo test --workspace --release

# Release builds are run by release.yml per-platform; the local
# pre-flight only builds the workspace once in release mode as a
# cheap "does the release profile compile" sanity check.
step 'cargo build --release (workspace sanity)' \
  cargo build --release

printf '\n\e[32m═══ all gates green ═══\e[0m\n'
echo 'Safe to push master. Wait for ci.yml on master to pass before pushing the tag.'
