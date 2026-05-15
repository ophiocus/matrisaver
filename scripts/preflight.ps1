# Pre-flight gate runner — mirrors .github/workflows/ci.yml exactly.
#
# Run this BEFORE pushing master, BEFORE tagging a release. If any gate
# fails locally, CI would have failed too. Cheaper to find out here.
#
# The gate list (and `--release` flag) must stay aligned with ci.yml and
# release.yml. The whole point of this script is to take the human
# "did I remember to run all the gates" question out of the loop.
#
# Usage (from repo root or anywhere):
#   pwsh scripts/preflight.ps1
#
# Exit code 0 = all green. Non-zero = at least one gate failed.

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location (Join-Path $repoRoot 'rust')

function Step([string]$name, [scriptblock]$body) {
    Write-Host ""
    Write-Host "═══ $name ═══" -ForegroundColor Cyan
    & $body
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAILED: $name" -ForegroundColor Red
        exit $LASTEXITCODE
    }
}

# Mirror of .github/workflows/ci.yml gate list. KEEP IN SYNC.
Step 'cargo fmt --check' {
    cargo fmt --check
}

Step 'cargo clippy --workspace --all-targets --release -- -D warnings' {
    cargo clippy --workspace --all-targets --release -- -D warnings
}

Step 'cargo test --workspace --release' {
    cargo test --workspace --release
}

# Release builds are run by release.yml per-platform; the local pre-flight
# only builds the workspace once in release mode as a cheap "does the
# release profile compile" sanity check. release.yml will still rebuild
# per host crate.
Step 'cargo build --release (workspace sanity)' {
    cargo build --release
}

Write-Host ""
Write-Host "═══ all gates green ═══" -ForegroundColor Green
Write-Host "Safe to push master. Wait for ci.yml on master to pass before pushing the tag."
