#!/bin/bash
# Runs inside the image rootfs. Started by scripts/headless-desktop.sh.
# Every process this starts is killed before the script exits, and nothing
# inherits the caller's stdio, so no descriptors leak back to the build host.
set -u
mode=${1:-session}
mount -t proc proc /proc 2>/dev/null || true
export XDG_RUNTIME_DIR=/tmp/xdg HOME=/root WAYLAND_DISPLAY=wayland-0
mkdir -p "$XDG_RUNTIME_DIR"; chmod 700 "$XDG_RUNTIME_DIR"
export WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1
rm -f /tmp/out.png

comp=""
cleanup() {
    if [ -n "$comp" ]; then
        kill -TERM -- "-$comp" 2>/dev/null
        sleep 1
        kill -KILL -- "-$comp" 2>/dev/null
    fi
    rm -f "$XDG_RUNTIME_DIR"/wayland-0 "$XDG_RUNTIME_DIR"/wayland-0.lock 2>/dev/null
}
trap cleanup EXIT INT TERM

shell_pid() { ps -eo pid,comm | awk '$2=="refurb-shell"{print $1; exit}'; }

setsid labwc </dev/null >/tmp/labwc.log 2>&1 &
comp=$!
sleep 6

case "$mode" in
session)
    echo "compositor=$comp shell=$(shell_pid)"
    grim /tmp/out.png 2>/tmp/grim.err && echo "capture ok" || { echo "capture failed"; cat /tmp/grim.err; }
    ;;
crash)
    first=$(shell_pid)
    echo "shell before: $first"
    kill -9 "$first" 2>/dev/null
    sleep 5
    second=$(shell_pid)
    echo "shell after crash: ${second:-none}"
    [ -n "$second" ] && [ "$second" != "$first" ] && echo "RESTARTED" || echo "NOT RESTARTED"
    grim /tmp/out.png 2>/dev/null && echo "capture ok"
    echo "--- session log"; tail -6 /root/.local/state/refurb/session.log 2>/dev/null
    ;;
esac
echo "--- labwc log"; tail -3 /tmp/labwc.log 2>/dev/null
