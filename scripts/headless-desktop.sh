#!/bin/sh
# Verify the graphical session without a VM: the real image rootfs, the real
# labwc, the real refurb-shell, captured with grim through the headless wlroots
# backend. Safe for a shared build host: the run gets its own PID namespace,
# every daemon's stdio goes to a file, and the whole process group is killed
# before this script returns.
#   ./scripts/headless-desktop.sh [rootfs] [out.png] [session|crash]
set -eu
rootfs=${1:-.build/rootfs}
out=${2:-.build/desktop.png}
mode=${3:-session}
[ -x "$rootfs/usr/bin/labwc" ] || { echo "no labwc in $rootfs" >&2; exit 2; }
install -m 755 scripts/guest-session-test.sh "$rootfs/tmp/guest-session-test.sh"
timeout 90 unshare -r --map-root-user --mount --pid --fork --mount-proc \
    -- chroot "$PWD/$rootfs" /bin/bash /tmp/guest-session-test.sh "$mode"
[ -f "$rootfs/tmp/out.png" ] && cp "$rootfs/tmp/out.png" "$out" && echo "saved $out ($(stat -c %s "$out") bytes)" || echo "no screenshot produced"
