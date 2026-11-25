#!/bin/sh
# Build Reawakening OS's dde-store package from Debian's 1.2.5-2 source plus
# 002-homepage-short-category.patch, inside a throw-away Debian trixie chroot.
#
#   ./packaging/dde-store/build.sh
#   -> distro/config/packages.chroot/dde-store_1.2.5-2.2~refurb1_amd64.deb
#
# Why a local package at all: Debian trixie's dde-store 1.2.5-2 segfaults on
# its home page when an AppStream category has fewer than ten applications
# (Debian #1117833). 1.2.5-2.1 fixes it, but that release is a Qt 6 port and
# this image is Qt 5, so the one-line bound is backported instead of upgrading
# Qt, DTK or anything else. Nothing from unstable is added to the image.
#
# The build host has no root: every privileged step runs inside `unshare -r`,
# with the same chown shim the ISO build uses, because only one uid is mapped
# in that namespace. `Rules-Requires-Root: no` in debian/control means dpkg-deb
# normalises ownership itself, so no fakeroot is needed.
set -eu

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
WORK=${WORK:-$ROOT/.build}
TOOLBOX=${TOOLBOX:-$ROOT/.toolbox}
SRC="$WORK/dde-store-build"
ROOTFS="$WORK/dderoot"
OUT="$ROOT/distro/config/packages.chroot"
VERSION="1.2.5-2.2~refurb2"
BASE="https://deb.debian.org/debian/pool/main/d/dde-store"
MIRROR=${MIRROR:-http://deb.debian.org/debian}
SUITE=${SUITE:-trixie}

ORIG_SHA=13314d7b4711754ea7a7fb7cd4c7c5b760fcfdec89ef62ab751496589170334e
DEBIAN_SHA=c649ebc4c7626d565581e80dceea19b805aebe938e6efbae5357cc11594dc17f

PATH="$HOME/.cargo/bin:$TOOLBOX/usr/bin:$TOOLBOX/usr/sbin:$TOOLBOX/sbin:$PATH"
export PATH
LD_LIBRARY_PATH="$TOOLBOX/usr/lib/x86_64-linux-gnu:$TOOLBOX/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export LD_LIBRARY_PATH

as_root() { unshare -r --map-root-user --mount --pid --fork --mount-proc -- "$@"; }
say() { printf '\n=== %s\n' "$*"; }

# ---------------------------------------------------------------- sources ---
say "sources into $SRC"
mkdir -p "$SRC"
fetch() {
    [ -s "$SRC/$1" ] || curl -fsSL "$BASE/$1" -o "$SRC/$1"
}
fetch dde-store_1.2.5.orig.tar.gz
fetch dde-store_1.2.5-2.debian.tar.xz
echo "$ORIG_SHA  $SRC/dde-store_1.2.5.orig.tar.gz" | sha256sum -c - >/dev/null
echo "$DEBIAN_SHA  $SRC/dde-store_1.2.5-2.debian.tar.xz" | sha256sum -c - >/dev/null

rm -rf "$SRC/tree"
mkdir -p "$SRC/tree"
tar xzf "$SRC/dde-store_1.2.5.orig.tar.gz" -C "$SRC/tree"
mv "$SRC/tree/dde-store-1.2.5" "$SRC/tree/dde-store"
# The upstream tarball carries its own packaging (debian/compat 10, its own
# rules). Debian's debian.tar.xz replaces it; leaving both on disk makes
# debhelper refuse to build because the compat level is declared twice.
rm -rf "$SRC/tree/dde-store/debian"
tar xJf "$SRC/dde-store_1.2.5-2.debian.tar.xz" -C "$SRC/tree/dde-store"
for patch in "$ROOT"/packaging/dde-store/*.patch; do
    name=$(basename "$patch")
    cp "$patch" "$SRC/tree/dde-store/debian/patches/"
    grep -qx "$name" "$SRC/tree/dde-store/debian/patches/series" ||
        printf '%s\n' "$name" >> "$SRC/tree/dde-store/debian/patches/series"
done

python3 - "$SRC/tree/dde-store/debian/changelog" "$VERSION" <<'PY'
import sys
path, version = sys.argv[1], sys.argv[2]
old = open(path, encoding="utf-8").read()
entry = f"""dde-store ({version}) unstable; urgency=medium

  * Rebuild for Reawakening OS with the home-page crash fix from 1.2.5-2.1
    (Debian #1117833): bound the category loop to the number of applications
    AppStream actually returned. The store no longer segfaults when a category
    contains fewer than ten entries. Still Qt 5; no unrelated components move.
  * Never wait for the third-party star-ratings server before showing the
    catalogue: a four-second timeout shows local AppStream data without
    ratings when the network cannot reach odrs.gnome.org.

 -- Reawakening OS <timdaymude@users.noreply.github.com>  Wed, 25 Nov 2026 01:00:00 +0000

"""
open(path, "w", encoding="utf-8").write(entry + old)
PY

# ---------------------------------------------------------------- chroot ----
if [ ! -x "$ROOTFS/bin/sh" ]; then
    say "debootstrap $SUITE into $ROOTFS"
    [ -f "$WORK/libchownshim.so" ] || \
        rustc --crate-type cdylib -O -o "$WORK/libchownshim.so" "$ROOT/scripts/chown-shim.rs"
    FUNCS="$TOOLBOX/usr/share/debootstrap/functions"
    if [ -f "$FUNCS" ] && ! grep -q 'EXTRACT_DEB_TAR_OPTIONS="--no-same-owner"' "$FUNCS"; then
        sed -i 's/^EXTRACT_DEB_TAR_OPTIONS=$/EXTRACT_DEB_TAR_OPTIONS="--no-same-owner"/' "$FUNCS"
    fi
    as_root env DEBOOTSTRAP_DIR="$TOOLBOX/usr/share/debootstrap" \
        EXTRACT_DEB_TAR_OPTIONS="--no-same-owner" TAR_OPTIONS="--no-same-owner" \
        "$TOOLBOX/usr/sbin/debootstrap" --foreign --variant=minbase --arch=amd64 \
        --no-check-gpg --components=main "$SUITE" "$ROOTFS" "$MIRROR"
    install -m 644 "$WORK/libchownshim.so" "$ROOTFS/lib/libchownshim.so"
    as_root env LD_PRELOAD=/lib/libchownshim.so TAR_OPTIONS="--no-same-owner" \
        DEBIAN_FRONTEND=noninteractive \
        chroot "$ROOTFS" /debootstrap/debootstrap --second-stage
fi

cp /etc/resolv.conf "$ROOTFS/etc/resolv.conf"
printf 'deb %s %s main\n' "$MIRROR" "$SUITE" > "$ROOTFS/etc/apt/sources.list"
printf '#!/bin/sh\nexit 101\n' > "$ROOTFS/usr/sbin/policy-rc.d"
chmod 755 "$ROOTFS/usr/sbin/policy-rc.d"
printf 'DPkg::Options { "--force-not-root"; };\n' > "$ROOTFS/etc/apt/apt.conf.d/99-build-chroot"

say "build dependencies"
as_root env LD_PRELOAD=/lib/libchownshim.so TAR_OPTIONS="--no-same-owner" \
    chroot "$ROOTFS" /bin/bash -c '
    set -e
    export DEBIAN_FRONTEND=noninteractive LC_ALL=C
    apt-get -o APT::Sandbox::User=root update >/dev/null
    apt-get -y --no-install-recommends -o APT::Sandbox::User=root install \
        build-essential cmake debhelper pkg-config \
        qtbase5-dev qttools5-dev qttools5-dev-tools \
        libdtkcore-dev libdtkgui-dev libdtkwidget-dev \
        libpackagekitqt5-dev libappstreamqt5-dev >/dev/null
'

say "build $VERSION"
# A previous run leaves its .deb in the chroot's /tmp; a glob would then copy
# that old file under the new name. Start from a clean slate.
rm -f "$ROOTFS"/tmp/dde-store_*_amd64.deb
rm -rf "$ROOTFS/tmp/dde-store"
cp -a "$SRC/tree/dde-store" "$ROOTFS/tmp/dde-store"
as_root env LD_PRELOAD=/lib/libchownshim.so TAR_OPTIONS="--no-same-owner" \
    chroot "$ROOTFS" /bin/bash -c '
    set -e
    export LC_ALL=C DEB_BUILD_OPTIONS=nocheck
    cd /tmp/dde-store
    dpkg-buildpackage -us -uc -b
'

DEB="$ROOTFS/tmp/dde-store_${VERSION}_amd64.deb"
[ -f "$DEB" ] || { echo "no $VERSION .deb was produced" >&2; exit 1; }
install -d "$OUT"
install -m 644 "$DEB" "$OUT/dde-store_${VERSION}_amd64.deb"
say "$OUT/dde-store_${VERSION}_amd64.deb"
dpkg-deb -I "$OUT/dde-store_${VERSION}_amd64.deb" | sed -n '1,12p'
