#!/bin/sh
# Test-only launcher for the r7 image (not shipped).
set -eu
ROOT=/workspace/chats/f9ad42685f3caa2547dfd38e
T="$ROOT/.toolbox"
export PATH="$T/usr/bin:$PATH"
export LD_LIBRARY_PATH="$T/usr/lib/x86_64-linux-gnu:$T/lib/x86_64-linux-gnu"
export QEMU_MODULE_DIR="$T/usr/lib/x86_64-linux-gnu/qemu"
cp -f "$T/usr/share/OVMF/OVMF_VARS_4M.fd" "$ROOT/.build/vars7.fd"
rm -f /tmp/q.sock
qemu-system-x86_64 -L "$T/usr/share/qemu" \
  -machine pc -cpu qemu64 -m 2048 -smp 2 \
  -drive if=pflash,format=raw,readonly=on,file="$T/usr/share/OVMF/OVMF_CODE_4M.fd" \
  -drive if=pflash,format=raw,file="$ROOT/.build/vars7.fd" \
  -cdrom "${1:-$ROOT/reawakening-os-0.3.0-r10-amd64.iso}" -boot d \
  -display none -vga std -audio none \
  -device qemu-xhci -device usb-tablet \
  -qmp unix:/tmp/q.sock,server,nowait \
  -serial unix:"$ROOT/.build/ser7.sock",server,nowait \
  -nic none -no-reboot &
echo $! > "$ROOT/.build/qemu7.pid"
sleep 2
echo "qemu pid $(cat "$ROOT/.build/qemu7.pid")"
