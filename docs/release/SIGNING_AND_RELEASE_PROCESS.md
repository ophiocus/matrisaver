# Signing and Release Process (Deferred)

Related docs:

- Docs index: [`../README.md`](../README.md)
- Roadmap: [`../planning/ROADMAP.md`](../planning/ROADMAP.md)

Current policy: build and distribute unsigned artifacts during active development.

This document records the future signing requirements so no architectural rework is needed
when signing is introduced.

## Common Requirements

- Deterministic build inputs (pinned toolchain and dependency lockfiles).
- CI-generated artifacts with immutable build metadata.
- Release manifest that maps source commit to binary hashes.

## Windows (MSI + Screensaver Binary)

Future requirements:

- Authenticode certificate (OV minimum, EV preferred for reputation).
- Sign both executable/screensaver binary and MSI installer.
- Timestamp signatures using trusted timestamp authority.

Future process:

1. Build release artifacts in CI.
2. Sign `.scr`/`.exe` and `.msi`.
3. Verify signatures and chain trust.
4. Publish signed checksums and release notes.

## macOS (Draggable Distribution)

Future requirements:

- Apple Developer ID Application certificate.
- Apple Developer ID Installer certificate (if installer package is later introduced).
- Notarization credentials and stapling process.

Future process:

1. Build app/bundle artifact.
2. Sign app with hardened runtime settings as required.
3. Submit for notarization.
4. Staple notarization ticket.
5. Validate with `spctl` and `codesign` checks.

## Linux (.deb)

Future requirements:

- GPG signing key for package repository metadata and package artifacts.

Future process:

1. Build `.deb` package.
2. Sign package/repo metadata.
3. Publish key distribution instructions.
4. Validate install flow on clean systems.

## Secrets and Key Management

- Keep private keys outside repo and out of long-lived CI runners.
- Prefer hardware-backed or managed key service storage.
- Require audited, least-privilege access for signing operations.
- Rotate certificates/keys before expiry and maintain overlap periods.

## Release Gates Before Signing Is Enabled

- Artifact integrity checks (hashes) generated for every release.
- Reproducible build notes retained with each artifact.
- Local verification script confirms expected binary metadata.
