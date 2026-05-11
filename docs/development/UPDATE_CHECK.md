# Update Check

The Windows host checks for newer releases when invoked in `/c` (config)
mode. The screensaver and preview execution paths are never involved.

## How it works

1. `build.rs` derives the binary's version from the latest `git describe --tags --match v*`
   tag, falling back to `Cargo.toml`'s version. The result is exposed as
   `env!("APP_VERSION")`.
2. On `/c` invocation the host hits the **GitHub Releases API**:
   `https://api.github.com/repos/{APP_GH_REPO}/releases/latest`.
3. It parses the JSON response, compares `tag_name` against `APP_VERSION`
   using the SemVer logic in `matrisaver-core::update`, and finds the
   first `.msi` asset on the release.
4. It prints a structured `UPDATE …` line to stdout. Failures are
   non-fatal and produce `UPDATE status=failed reason=…`.

This matches the I:\ project family (Skeleton, MDReader, TinyBoothSoundStudio,
examhelper) — the GitHub release tag is the source of truth, no
hand-edited manifest file in the repo.

## TLS deviation from the family

The family uses `reqwest` with `rustls-tls`. matrisaver-host-windows uses
`reqwest` with `native-tls`, which on Windows backs to SChannel. This
keeps `rustls` and `ring` out of the dependency graph entirely. If you
update the family's update-check code, do not blindly copy the
`features = ["rustls-tls"]` line — keep `native-tls` for matrisaver.

## Repo target

Compiled in via `matrisaver_core::update::APP_GH_REPO`, set to
`ophiocus/matrisaver`.

Override at runtime (useful for staging channels, forks, or
CI tests):

```
set MATRISAVER_GH_REPO=someone-else/matrisaver-fork
matrisaver.scr /c
```

Or per-invocation (useful for staging-channel tests):

```
matrisaver.scr /c --update-check-repo ophiocus/matrisaver-staging
```

Skip entirely (for CI or offline environments):

```
matrisaver.scr /c --skip-update-check
```

## Release workflow

```
# 1. Bump version in rust/Cargo.toml
[workspace.package]
version = "0.2.0"

# 2. Tag and push
git tag v0.2.0
git push origin v0.2.0

# 3. CI (.github/workflows/release.yml) builds the .scr + MSI and
#    uploads them to a GitHub Release named after the tag.
```

That's it. No manifest file to update — the next `/c` invocation reads
the new release directly from the GitHub API.

## stdout format

```
UPDATE status=up-to-date current=0.1.0
UPDATE status=available current=0.1.0 latest=0.2.0 msi_url=https://… changelog_url=https://…
UPDATE status=failed reason=HTTP_404
```

Spaces in failure reasons are converted to `_` so the line stays
parseable as space-separated key=value pairs.
