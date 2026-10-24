#!/usr/bin/env bash
#
# Build matrisaver_<version>_<arch>.deb.
#
# dpkg-deb + a staged tree, deliberately: no cargo-deb (one less build
# dependency on every machine and CI runner, and the layout below is the
# thing being reviewed), and no fakeroot either — `dpkg-deb
# --root-owner-group` stamps root:root ownership without privileges, so
# this runs as an ordinary user. It needs dpkg-deb, which every Debian
# derivative has in the base system.
#
# Usage:
#   scripts/package-deb.sh [--binary PATH] [--out DIR] [--arch ARCH] [--skip-build]
#
# Default: build the release binary with cargo, stage it, emit the .deb
# into artifacts/.

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGING="$REPO_ROOT/packaging/deb"

BINARY=""
OUT_DIR="$REPO_ROOT/artifacts"
ARCH=""
SKIP_BUILD=0

while [ $# -gt 0 ]; do
    case "$1" in
        --binary) BINARY="$2"; shift 2 ;;
        --out) OUT_DIR="$2"; shift 2 ;;
        --arch) ARCH="$2"; shift 2 ;;
        --skip-build) SKIP_BUILD=1; shift ;;
        -h|--help) sed -n '3,20p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "package-deb.sh: unknown argument '$1'" >&2; exit 2 ;;
    esac
done

need() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "package-deb.sh: '$1' is required but not installed." >&2
        exit 1
    }
}
need dpkg-deb

if [ -z "$ARCH" ]; then
    if command -v dpkg-architecture >/dev/null 2>&1; then
        ARCH="$(dpkg-architecture -qDEB_HOST_ARCH)"
    else
        case "$(uname -m)" in
            x86_64) ARCH=amd64 ;;
            aarch64) ARCH=arm64 ;;
            *) echo "package-deb.sh: cannot map $(uname -m) to a Debian arch; pass --arch." >&2
               exit 1 ;;
        esac
    fi
fi

# Version comes from the workspace manifest, which is also what
# release.yml checks the tag against.
VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$REPO_ROOT/rust/Cargo.toml" | head -1)"
if [ -z "$VERSION" ]; then
    echo "package-deb.sh: could not read version from rust/Cargo.toml" >&2
    exit 1
fi

if [ -z "$BINARY" ]; then
    BINARY="${CARGO_TARGET_DIR:-$REPO_ROOT/rust/target}/release/matrisaver-host-linux"
    if [ "$SKIP_BUILD" -eq 0 ]; then
        echo "==> cargo build --release -p matrisaver-host-linux"
        ( cd "$REPO_ROOT/rust" && cargo build --release -p matrisaver-host-linux )
    fi
fi
if [ ! -f "$BINARY" ]; then
    echo "package-deb.sh: binary not found at $BINARY" >&2
    exit 1
fi

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
# mktemp -d gives 0700, which would ship the package root as drwx------
# and make the installed tree unreadable to anyone but root.
chmod 0755 "$STAGE"

install -d -m 0755 \
    "$STAGE/DEBIAN" \
    "$STAGE/usr/bin" \
    "$STAGE/usr/libexec/matrisaver" \
    "$STAGE/usr/share/applications" \
    "$STAGE/usr/share/doc/matrisaver" \
    "$STAGE/usr/share/man/man1" \
    "$STAGE/usr/share/matrisaver/integration" \
    "$STAGE/usr/share/matrisaver/overlays" \
    "$STAGE/usr/share/pixmaps" \
    "$STAGE/usr/lib/systemd/user"

# ── runtime ──────────────────────────────────────────────────────────
# Installed as `matrisaver`, not `matrisaver-host-linux`: the crate name
# is a build detail, and the man page, .desktop entry and every
# integration example call it matrisaver.
install -m 0755 "$BINARY" "$STAGE/usr/bin/matrisaver"
if command -v strip >/dev/null 2>&1; then
    strip --strip-unneeded "$STAGE/usr/bin/matrisaver" 2>/dev/null || true
fi

install -m 0755 "$PACKAGING/integration/matrisaver-idle" \
    "$STAGE/usr/libexec/matrisaver/matrisaver-idle"

# ── overlay pack ─────────────────────────────────────────────────────
# Only the default overlay ships. The other images in assets/overlays
# are a personal photo and a stock-agency comp; neither is ours to
# redistribute in a public archive. core resolves this directory via
# shipped_overlay_pack_dirs(), and a per-user pack under
# $XDG_DATA_HOME/matrisaver/overlays takes precedence over it.
install -m 0644 "$REPO_ROOT/assets/overlays/matrisaver-default.png" \
    "$STAGE/usr/share/matrisaver/overlays/matrisaver-default.png"
install -m 0644 "$REPO_ROOT/assets/overlays/README.md" \
    "$STAGE/usr/share/matrisaver/overlays/README.md"

# ── integration helpers ──────────────────────────────────────────────
for example in "$PACKAGING"/integration/*.example; do
    install -m 0644 "$example" "$STAGE/usr/share/matrisaver/integration/"
done
install -m 0644 "$PACKAGING/matrisaver-idle.service" \
    "$STAGE/usr/lib/systemd/user/matrisaver-idle.service"

# ── desktop integration ──────────────────────────────────────────────
install -m 0644 "$PACKAGING/matrisaver.desktop" \
    "$STAGE/usr/share/applications/matrisaver.desktop"
install -m 0644 "$REPO_ROOT/assets/branding/logo.png" \
    "$STAGE/usr/share/pixmaps/matrisaver.png"

# ── documentation ────────────────────────────────────────────────────
sed "s/@VERSION@/$VERSION/g" "$PACKAGING/matrisaver.1" \
    > "$STAGE/usr/share/man/man1/matrisaver.1"
gzip -9n "$STAGE/usr/share/man/man1/matrisaver.1"

install -m 0644 "$PACKAGING/copyright" "$STAGE/usr/share/doc/matrisaver/copyright"
install -m 0644 "$PACKAGING/README.Debian" "$STAGE/usr/share/doc/matrisaver/README.Debian"

# Two changelogs, deliberately. dpkg and lintian parse
# doc/<pkg>/changelog.gz as a *Debian* changelog, so handing them the
# upstream Keep-a-Changelog markdown produced one syntax warning per
# heading (~50 of them). So changelog.gz carries a Debian-format entry
# and the upstream file keeps its own name and extension, which is also
# how a reader can tell the two apart.
#
# changelog.gz rather than changelog.Debian.gz because the version
# carries no Debian revision (0.3.7, not 0.3.7-1), which makes this a
# *native* package, and native packages name it changelog.gz.
#
# SOURCE_DATE_EPOCH is honoured so a release build is reproducible; gzip
# -n drops the timestamp from the archive member for the same reason.
if [ -n "${SOURCE_DATE_EPOCH:-}" ]; then
    CHANGELOG_DATE="$(date -R -u -d "@$SOURCE_DATE_EPOCH")"
else
    CHANGELOG_DATE="$(date -R)"
fi
MAINTAINER="$(sed -n 's/^Maintainer: //p' "$PACKAGING/control.in" | head -1)"
sed -e "s/@VERSION@/$VERSION/g" \
    -e "s|@MAINTAINER@|$MAINTAINER|g" \
    -e "s|@DATE@|$CHANGELOG_DATE|g" \
    "$PACKAGING/changelog.Debian.in" \
    > "$STAGE/usr/share/doc/matrisaver/changelog"
gzip -9n "$STAGE/usr/share/doc/matrisaver/changelog"

install -m 0644 "$REPO_ROOT/CHANGELOG.md" \
    "$STAGE/usr/share/doc/matrisaver/CHANGELOG.md"
gzip -9n "$STAGE/usr/share/doc/matrisaver/CHANGELOG.md"

# ── control metadata ─────────────────────────────────────────────────
INSTALLED_SIZE="$(du -ks "$STAGE/usr" | cut -f1)"
sed -e "s/@VERSION@/$VERSION/g" \
    -e "s/@ARCH@/$ARCH/g" \
    -e "s/@INSTALLED_SIZE@/$INSTALLED_SIZE/g" \
    "$PACKAGING/control.in" > "$STAGE/DEBIAN/control"

install -m 0755 "$PACKAGING/postinst" "$STAGE/DEBIAN/postinst"
install -m 0755 "$PACKAGING/prerm" "$STAGE/DEBIAN/prerm"

# md5sums over every installed file, paths relative to the package root.
( cd "$STAGE" && find usr -type f -print0 | sort -z | xargs -0 md5sum > DEBIAN/md5sums )
chmod 0644 "$STAGE/DEBIAN/md5sums"

install -d -m 0755 "$OUT_DIR"
DEB="$OUT_DIR/matrisaver_${VERSION}_${ARCH}.deb"
# -Zxz, not dpkg-deb's default: Ubuntu 24.04 defaults to zstd, which
# dpkg older than 1.21.x refuses to unpack ("unsupported compression"),
# so a zstd package silently excludes Debian 11 and Ubuntu 20.04 hosts.
# xz is understood everywhere and the size difference here is noise next
# to the embedded CJK font.
dpkg-deb -Zxz --root-owner-group --build "$STAGE" "$DEB" >/dev/null

echo "==> $DEB"
dpkg-deb --info "$DEB" | sed 's/^/    /'
echo "    ---- contents ----"
dpkg-deb --contents "$DEB" | awk '{ printf "    %s %s\n", $1, $6 }'
