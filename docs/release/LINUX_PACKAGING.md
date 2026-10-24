# Linux Packaging (`.deb`)

Related docs:

- Docs index: [`../README.md`](../README.md)
- Linux host reference: [`../development/LINUX_HOST.md`](../development/LINUX_HOST.md)
- Signing and release process: [`SIGNING_AND_RELEASE_PROCESS.md`](SIGNING_AND_RELEASE_PROCESS.md)
- Roadmap (Milestone 3): [`../planning/ROADMAP.md`](../planning/ROADMAP.md)

## Building

```bash
scripts/package-deb.sh                     # build + stage + emit into artifacts/
scripts/package-deb.sh --skip-build \
    --binary rust/target/release/matrisaver-host-linux
```

Output: `artifacts/matrisaver_<version>_<arch>.deb`. The version comes
from `rust/Cargo.toml`'s `[workspace.package]`, which is the same value
`release.yml` checks the tag against, so the package cannot disagree with
the release.

Requirements: `dpkg-deb` only, which is in the base system of every
Debian derivative. Deliberately **not** `cargo-deb` — one less build
dependency per machine and CI runner, and the staged tree under
`packaging/deb/` is the thing being reviewed rather than generated
metadata. No `fakeroot` and no privileges either: `dpkg-deb
--root-owner-group` stamps `root:root` ownership directly.

`SOURCE_DATE_EPOCH` is honoured for the Debian changelog date, and gzip
runs with `-n`, so a release build is reproducible. `release.yml` sets it
from the tagged commit's timestamp.

Compression is pinned to **xz** (`-Zxz`). Ubuntu 24.04's `dpkg-deb`
defaults to zstd, which dpkg older than 1.21 refuses to unpack — a zstd
package silently excludes Debian 11 and Ubuntu 20.04 hosts. The size
difference is noise next to the embedded CJK font.

## Layout

| Path | Contents |
| --- | --- |
| `/usr/bin/matrisaver` | The host binary. Installed under the short name because the crate name is a build detail, and the man page, `.desktop` entry and every integration example call it `matrisaver`. |
| `/usr/libexec/matrisaver/matrisaver-idle` | Idle-launch wrapper; picks the session's idle daemon. |
| `/usr/lib/systemd/user/matrisaver-idle.service` | **User** unit driving the wrapper. |
| `/usr/share/matrisaver/overlays/` | Shipped overlay pack. |
| `/usr/share/matrisaver/integration/` | Worked examples per idle daemon. |
| `/usr/share/applications/matrisaver.desktop` | Desktop entry. |
| `/usr/share/pixmaps/matrisaver.png` | Icon (size-agnostic path, so no hicolor theme bookkeeping). |
| `/usr/share/man/man1/matrisaver.1.gz` | Man page, including the exit codes. |
| `/usr/share/doc/matrisaver/` | `copyright`, `README.Debian`, `changelog.gz` (Debian format), `CHANGELOG.md.gz` (upstream). |

### Two changelogs

`dpkg` and `lintian` parse `doc/<pkg>/changelog.gz` as a *Debian*
changelog. Handing them the upstream Keep-a-Changelog markdown produced
one syntax warning per heading — about fifty of them. So `changelog.gz`
carries a short Debian-format entry and the upstream file keeps its own
name, `CHANGELOG.md.gz`, which also tells a reader which is which.

It is `changelog.gz` rather than `changelog.Debian.gz` because the
version carries no Debian revision (`0.3.7`, not `0.3.7-1`), making this
a *native* package, and native packages use the short name.

### Overlay assets: what ships and what does not

Only `matrisaver-default.png` and the overlay README ship. The other two
images in `assets/overlays/` are a personal photo and a file whose name
matches a stock-agency comp; neither is ours to redistribute in a public
archive.

**The MSI ships all three** and carries the same exposure. Not changed
here — it would alter the shipped Windows package — but it is worth a
decision of its own.

Core resolves the installed pack through
`shipped_overlay_pack_dirs()`, in precedence order:

1. `$MATRISAVER_OVERLAY_DIR`
2. `$XDG_DATA_HOME/matrisaver/overlays` (default `~/.local/share/...`)
3. `/usr/local/share/matrisaver/overlays`
4. `/usr/share/matrisaver/overlays`

Those Unix candidates were **added** for this milestone. Before them, the
only packaged root was `%ProgramData%\matrisaver\overlays` plus an
exe-relative `assets/overlays` ancestor walk; from `/usr/bin/matrisaver`
that walk probes `/usr/bin/assets/overlays`, `/usr/assets/overlays` and
`/assets/overlays`, so a `.deb` install would have resolved **zero**
overlays no matter what the package shipped. A per-user pack leads the
system one, so a user can shadow the shipped art without root.

## Dependencies

`Depends: libc6 (>= 2.34)` — everything else the renderer needs is
dlopened at runtime, which is also why the build host needs no `-dev`
packages. `Recommends: mesa-vulkan-drivers | libgl1-mesa-dri` covers the
two rungs of the GPU ladder; `Suggests: swayidle | xautolock | xss-lock`
covers idle integration.

## Idle and lock integration

MatriSaver does not watch for idleness, by design. Every desktop already
ships an idle daemon, and a second one competing with the session's own
inhibitors (a full-screen video, a presentation) is how a screensaver
earns a reputation for interrupting people. The wrapper picks the
daemon that matches the running session:

| Session | Daemon | Status |
| --- | --- | --- |
| sway / Hyprland / river (Wayland) | `swayidle -w` | Example shipped; **not run on a real session** |
| Plain X11 | `xautolock -time N -locker ... -detectsleep` | Example shipped; **not run on a real session** |
| X11 with an existing locker | `xss-lock --notifier` | Example shipped, with the caveat below |
| GNOME / KDE | their own idle management | No integration needed; point their settings at MatriSaver |
| xscreensaver `programs:` | — | **Unsupported.** MatriSaver does not implement the `-window-id` protocol, so it would draw *over* xscreensaver's window and the two would fight over the screen. |

The unit must stay a **user** unit. A system unit has no
`WAYLAND_DISPLAY` or `DISPLAY`, so MatriSaver would exit 4 every time:

```bash
systemctl --user enable --now matrisaver-idle.service
```

If the unit starts before the compositor publishes its environment, the
compositor autostart needs:

```bash
systemctl --user import-environment WAYLAND_DISPLAY DISPLAY XDG_RUNTIME_DIR
```

### Not a locker

MatriSaver draws; it does not authenticate, and any key or click
dismisses it. This matches the architecture rule that exit behaviour
always defers to OS lock/session behaviour. `xss-lock -- matrisaver`
therefore leaves the session **unsecured** while xss-lock believes a
locker is up; the shipped example uses `--notifier` with a real locker
instead. Chain a locker after the screensaver if you want both — do not
substitute one for the other.

## Verification performed

Built on Ubuntu 24.04 (WSL), installed on **Debian 13 trixie** (WSL) —
deliberately a different distro from the build host, so the dependency
declarations are actually exercised. 2026-10-23:

- `dpkg -i` clean, `postinst` guidance shown.
- `dpkg --verify matrisaver` clean (md5sums match).
- `dpkg -r` clean; `/usr/share/matrisaver` removed.
- `lintian`: **zero tags.**
- `desktop-file-validate`: clean.
- Man page renders with zero `groff -ww` warnings (131 lines).
- `systemd-analyze verify` on the user unit: clean.
- `matrisaver --help`, `--benchmark-frames 60`: exit 0 on the target
  distro (binary built against glibc 2.39, run on 2.41).
- Playback with no session: exit 4.
- Idle wrapper with no daemon installed: exit 1 plus install guidance.
- `sh -n` on every shipped shell script.

Not verified: installation on a real desktop session, and therefore the
idle-integration table's first three rows. See the "Verified, and not"
section of [`../development/LINUX_HOST.md`](../development/LINUX_HOST.md).
