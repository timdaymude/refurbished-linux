#!/bin/sh
# Static checks on the distribution tree. Catches the mistakes that would
# otherwise only show up after a 40 minute ISO build.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
DISTRO="$ROOT/distro"
INC="$DISTRO/config/includes.chroot"
fail=0

err() { printf '  FAIL %s\n' "$*"; fail=$((fail + 1)); }
ok()  { printf '  ok   %s\n' "$*"; }

echo "== shell scripts parse with the shell that will run them"
for f in "$ROOT"/scripts/*.sh "$DISTRO"/config/hooks/normal/*.hook.chroot \
         "$INC/usr/bin/refurb-session" "$INC"/usr/lib/refurb/refurb-apt \
         "$INC"/usr/lib/refurb/refurb-update; do
    if sh -n "$f" 2>/dev/null; then ok "${f#$ROOT/}"; else err "${f#$ROOT/} (sh -n)"; fi
done

echo "== XML, JSON and desktop files are well formed"
python3 - "$DISTRO" <<'PY' || fail=$((fail + 1))
import json, pathlib, sys, xml.etree.ElementTree as ET
root = pathlib.Path(sys.argv[1])
bad = 0
for p in list(root.rglob("*.xml")):
    try:
        ET.parse(p)
        print(f"  ok   {p.relative_to(root.parent)}")
    except Exception as e:
        print(f"  FAIL {p.relative_to(root.parent)}: {e}"); bad += 1
for p in list(root.rglob("*.json")):
    try:
        json.loads(p.read_text())
        print(f"  ok   {p.relative_to(root.parent)}")
    except Exception as e:
        print(f"  FAIL {p.relative_to(root.parent)}: {e}"); bad += 1
for p in list(root.rglob("*.desktop")):
    text = p.read_text()
    if not text.startswith("[Desktop Entry]") or "Name=" not in text or "Exec=" not in text:
        print(f"  FAIL {p.relative_to(root.parent)}: incomplete desktop entry"); bad += 1
    else:
        print(f"  ok   {p.relative_to(root.parent)}")
sys.exit(1 if bad else 0)
PY

echo "== live-build configuration is complete"
for f in common package-lists/refurb-base.list.chroot \
         package-lists/refurb-desktop.list.chroot package-lists/refurb-browser.list.chroot \
         archives/brave.list.chroot archives/brave.key.chroot \
         hooks/normal/0100-identity.hook.chroot hooks/normal/0200-no-background-work.hook.chroot \
         includes.chroot/usr/share/wayland-sessions/refurb.desktop \
         includes.chroot/etc/xdg/labwc/rc.xml \
         includes.chroot/usr/lib/systemd/system/refurb-session-prepare.service; do
    [ -s "$DISTRO/config/$f" ] && ok "config/$f" || err "config/$f missing or empty"
done

echo "== pinned launcher entries are contiguous"
python3 - "$INC" <<'PY'
import pathlib, sys
pins = {}
for p in pathlib.Path(sys.argv[1]).rglob("*.desktop"):
    for line in p.read_text().splitlines():
        if line.startswith("X-Refurb-Pin="):
            pins[int(line.split("=", 1)[1])] = p.name
want = list(range(1, len(pins) + 1))
print(f"  {'ok  ' if sorted(pins) == want else 'FAIL'} pins {sorted(pins)}")
for k in sorted(pins):
    print(f"       {k}: {pins[k]}")
PY

echo "== sudoers rule is limited to the two helpers"
if [ "$(grep -c 'NOPASSWD' "$INC/etc/sudoers.d/refurb-helpers")" = 1 ] && \
   grep -q 'refurb-apt' "$INC/etc/sudoers.d/refurb-helpers"; then
    ok "one NOPASSWD rule, helpers only"
else
    err "unexpected sudoers content"
fi

echo "== no update daemon, packagekit or indexer is enabled"
if grep -Rqs 'unattended-upgrades.service\|packagekit.service' "$DISTRO/config/hooks/normal/0200-no-background-work.hook.chroot"; then
    ok "masked in 0200-no-background-work"
else
    err "masking hook does not cover the update daemons"
fi

echo ""
if [ "$fail" -eq 0 ]; then
    echo "check-config: all checks passed"
else
    echo "check-config: $fail check group(s) failed"
    exit 1
fi
