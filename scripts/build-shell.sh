#!/bin/sh
# Build refurb-shell and package it as a .deb for the ISO.
#
#   ./scripts/build-shell.sh
#
# Output: distro/config/packages.chroot/refurb-shell_<version>_amd64.deb
# live-build installs every .deb in that directory into the image, so the
# desktop shell needs no compilation step inside the chroot.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
SRC="$ROOT/shell/refurb-shell"
OUT="$ROOT/distro/config/packages.chroot"

PATH="$HOME/.cargo/bin:$PATH"
export PATH

command -v cargo >/dev/null 2>&1 || {
    echo "cargo not found. Install Rust (https://rustup.rs) and try again." >&2
    exit 1
}

VERSION=$(sed -n 's/^version *= *"\(.*\)"/\1/p' "$SRC/Cargo.toml" | head -1)
[ -n "$VERSION" ] || { echo "cannot read version from Cargo.toml" >&2; exit 1; }

echo "==> building refurb-shell $VERSION (release)"
(cd "$SRC" && cargo build --release)

BIN="$SRC/target/release/refurb-shell"
[ -x "$BIN" ] || { echo "release binary missing" >&2; exit 1; }

PKG=$(mktemp -d)
trap 'rm -rf "$PKG"' EXIT
mkdir -p "$PKG/DEBIAN" "$PKG/usr/bin" "$PKG/usr/share/doc/refurb-shell"

install -m 0755 "$BIN" "$PKG/usr/bin/refurb-shell"
install -m 0644 "$SRC/assets/fonts/Inter-LICENSE.txt" \
    "$PKG/usr/share/doc/refurb-shell/copyright"

cat > "$PKG/DEBIAN/control" <<CONTROL
Package: refurb-shell
Version: $VERSION
Section: x11
Priority: optional
Architecture: amd64
Depends: libc6
Recommends: labwc, fonts-inter
Installed-Size: $(du -sk "$PKG/usr" | cut -f1)
Maintainer: Reawakening OS <timdaymude@users.noreply.github.com>
Description: the Reawakening OS desktop shell
 The bar, launcher, quick settings, software manager and settings windows of
 Reawakening OS. It draws its own interface with a CPU rasteriser: no GTK,
 no Qt, no toolkit, no icon theme. Keyboard handling uses libxkbcommon when it
 is present and falls back to a US layout when it is not.
CONTROL

mkdir -p "$OUT"
DEB="$OUT/refurb-shell_${VERSION}_amd64.deb"
dpkg-deb --build --root-owner-group "$PKG" "$DEB" >/dev/null 2>&1 || \
    dpkg-deb -b "$PKG" "$DEB" >/dev/null
echo "==> $DEB"
dpkg-deb -c "$DEB" | sed 's/^/    /'
