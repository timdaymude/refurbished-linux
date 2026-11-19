#!/bin/sh
# Boot the ISO in QEMU (software emulation: no KVM here) and record evidence.
#
#   ./scripts/qemu-boot.sh [iso]
#
# Writes .build/serial.log (guest kernel + systemd output), .build/shot-N.ppm
# screenshots through the QEMU monitor, and leaves the VM running.
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
TOOLBOX=${TOOLBOX:-$ROOT/.toolbox}
export PATH="$TOOLBOX/usr/bin:$PATH"
export LD_LIBRARY_PATH="$TOOLBOX/usr/lib/x86_64-linux-gnu:$TOOLBOX/lib/x86_64-linux-gnu"
# Modern qemu builds ship the accelerators and devices as loadable modules, and
# the module directory is compiled into the binary.
export QEMU_MODULE_DIR="$TOOLBOX/usr/lib/x86_64-linux-gnu/qemu"
ISO=${1:-$(ls -1t "$ROOT"/reawakening-os-*-amd64.iso "$ROOT"/refurbished-linux-*-amd64.iso 2>/dev/null | head -1)}
mkdir -p "$ROOT/.build"

# qemu looks for firmware in a compiled-in prefix; point it at the toolbox.
QEMU_SHARE="$TOOLBOX/usr/share/qemu"
mkdir -p "$QEMU_SHARE"
for f in "$TOOLBOX"/usr/share/seabios/*; do [ -f "$f" ] && cp -n "$f" "$QEMU_SHARE/" 2>/dev/null || true; done
for f in "$TOOLBOX"/usr/share/vgabios/*; do [ -f "$f" ] && cp -n "$f" "$QEMU_SHARE/" 2>/dev/null || true; done

rm -f "$ROOT/.build/mon.sock" "$ROOT/.build/serial.log" "$ROOT/.build/ser.sock" "$ROOT/.build/serial-full.log"
qemu-system-x86_64 \
    -L "$QEMU_SHARE" \
    -m 2048 -smp 2 \
    -machine pc -cpu max \
    -cdrom "$ISO" -boot d \
    -display none -vga std \
    -serial "unix:$ROOT/.build/ser.sock,server,nowait" \
 \
    -monitor "unix:$ROOT/.build/mon.sock,server,nowait" \
    -nic none \
    -no-reboot &
echo "$!" > "$ROOT/.build/qemu.pid"
echo "qemu pid $(cat "$ROOT/.build/qemu.pid"), serial log .build/serial.log"
