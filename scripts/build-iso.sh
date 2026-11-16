#!/bin/sh
# Build the Refurbished Linux ISO.
#
#   sudo ./scripts/build-iso.sh
#
# Needs: live-build, debootstrap, xorriso, mtools, squashfs-tools, root.
# The host must be Debian or Ubuntu with the same architecture (amd64).
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
DISTRO="$ROOT/distro"

if [ "$(id -u)" -ne 0 ]; then
    echo "This script needs root (debootstrap, chroot, loop mounts)." >&2
    echo "Re-run with: sudo $0" >&2
    exit 1
fi

for tool in lb debootstrap; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "missing: $tool (apt-get install live-build debootstrap)" >&2
        exit 1
    }
done

# The desktop shell is shipped as a package, never built inside the chroot.
if ! ls "$DISTRO"/config/packages.chroot/refurb-shell_*.deb >/dev/null 2>&1; then
    echo "==> no refurb-shell package yet; building it"
    "$ROOT/scripts/build-shell.sh"
fi

"$ROOT/scripts/check-config.sh"

cd "$DISTRO"
echo "==> lb config"
lb config
echo "==> lb build (this takes a while and needs network access)"
lb build

ISO=$(ls -1t "$DISTRO"/*.iso 2>/dev/null | head -1 || true)
if [ -n "$ISO" ]; then
    echo "==> ISO: $ISO"
    echo "    write it with: sudo dd if=$ISO of=/dev/sdX bs=8M status=progress oflag=direct"
else
    echo "no ISO produced; see binary.log in $DISTRO" >&2
    exit 1
fi
