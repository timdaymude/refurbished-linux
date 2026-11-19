#!/bin/sh
# Boot the release ISO in QEMU exactly as the target machine would see it
# (2 cores, 2 GB, no KVM) and collect evidence: a serial log, a screenshot of
# the real framebuffer through the QEMU monitor, and whichever boot entry we ask
# for. Nothing here inherits the caller's stdio; the VM is daemonised and killed
# by recorded pid.
#   ./scripts/qemu-desktop-test.sh [iso] [entry]
#     entry: default | diag
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
TOOLBOX=${TOOLBOX:-$ROOT/.toolbox}
export PATH="$TOOLBOX/usr/bin:$PATH"
export LD_LIBRARY_PATH="$TOOLBOX/usr/lib/x86_64-linux-gnu:$TOOLBOX/lib/x86_64-linux-gnu"
export QEMU_MODULE_DIR="$TOOLBOX/usr/lib/x86_64-linux-gnu/qemu"
ISO=${1:-$(ls -1t "$ROOT"/reawakening-os-*-amd64.iso 2>/dev/null | head -1)}
ENTRY=${2:-default}
SER="$ROOT/.build/ser-03.sock"; LOG="$ROOT/.build/serial-03.log"; MON="$ROOT/.build/mon-03.sock"; PID="$ROOT/.build/qemu-03.pid"
QEMU_SHARE="$TOOLBOX/usr/share/qemu"
mkdir -p "$QEMU_SHARE" "$ROOT/.build"
for f in "$TOOLBOX"/usr/share/seabios/* "$TOOLBOX"/usr/share/vgabios/*; do
    [ -f "$f" ] && cp -n "$f" "$QEMU_SHARE/" 2>/dev/null || true
done
rm -f "$SER" "$MON" "$PID" "$LOG"
qemu-system-x86_64 -L "$QEMU_SHARE" \
    -m 2048 -smp 2 -machine pc -cpu qemu64 \
    -cdrom "$ISO" -boot d -display none -vga std -nic none -no-reboot \
    -serial "unix:$SER,server,nowait" -monitor "unix:$MON,server,nowait" \
    -daemonize -pidfile "$PID"
echo "qemu pid $(cat "$PID")"

if [ "$ENTRY" = diag ]; then
    # The isolinux menu shows for 3 s; pick the third entry before it expires.
    # Keys arriving later are harmless (they go to the console).
    sleep 13
    for i in 1 2 3 4 5 6 7 8; do
        python3 "$ROOT/scripts/qmon.py" "$MON" sendkey down sendkey down sendkey ret >/dev/null 2>&1 || true
        sleep 1
    done
    echo "selected the memory-report entry"
fi
