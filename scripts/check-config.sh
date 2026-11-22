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
         "$INC/usr/bin/refurb-session" "$INC"/usr/lib/refurb/*; do
    [ -f "$f" ] || continue
    # refurb-dictate is Python, not shell; it is syntax-checked below with the
    # interpreter that will run it.
    case "$f" in *refurb-dictate) continue ;; esac
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
         archives/jellyfin.list.chroot archives/jellyfin.key.chroot \
         package-lists/refurb-apps.list.chroot \
         hooks/normal/0700-input.hook.chroot \
         includes.chroot/etc/udev/hwdb.d/70-refurb-chromebook-keys.hwdb \
         includes.chroot/usr/lib/refurb/refurb-cockpit \
         includes.chroot/usr/lib/refurb/refurb-install \
         includes.chroot/usr/lib/refurb/refurb-audio \
         includes.chroot/usr/share/applications/refurb-install.desktop \
         includes.chroot/etc/sudoers.d/50-refurb-live \
         includes.chroot/etc/systemd/system/cockpit.socket.d/10-refurb-loopback.conf \
         includes.chroot/usr/lib/refurb/refurb-dictate \
         includes.chroot/etc/wireplumber/wireplumber.conf.d/51-refurb-audio.conf \
         hooks/normal/0800-dictation.hook.chroot \
         includes.chroot/usr/share/applications/refurb-cockpit-remote.desktop \
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

echo "== the taskbar carries Brave and Files when they are closed"
python3 - "$INC" <<'PY'
import pathlib, sys
entries = []
for p in pathlib.Path(sys.argv[1]).rglob("*.desktop"):
    for line in p.read_text().splitlines():
        if line.strip() == "X-Refurb-Taskbar=1":
            entries.append(p.name)
want = ["brave-browser.desktop", "refurb-files.desktop"]
print(f"  {'ok  ' if sorted(entries) == want else 'FAIL'} taskbar pins {sorted(entries)}")
PY

echo "== the graphical installer is Calamares with Reawakening settings"
if grep -q '^calamares$' "$DISTRO/config/package-lists/refurb-base.list.chroot" && \
   grep -q '^branding: reawakening$' "$INC/etc/calamares/settings.conf" && \
   grep -q '^initialPartitioningChoice: none$' "$INC/etc/calamares/modules/partition.conf" && \
   [ -s "$INC/etc/calamares/branding/reawakening/branding.desc" ] && \
   [ -x "$INC/usr/lib/refurb/refurb-installer" ] && \
   grep -q 'blockdev --setro' "$INC/usr/lib/refurb/refurb-installer" && \
   grep -q 'installEFIFallback: true' "$INC/etc/calamares/modules/bootloader.conf"; then
    ok "Calamares has branding, an explicit erase choice, live-disk protection and the UEFI fallback"
else
    err "the Calamares configuration or its safety pieces are missing"
fi
if [ -x "$INC/usr/lib/refurb/calamares-finalize" ] && \
   grep -q '50-refurb-live' "$INC/usr/lib/refurb/calamares-finalize" && \
   grep -q 'cockpit.socket' "$INC/usr/lib/refurb/calamares-finalize" && \
   grep -q '^username: refurb$' "$INC/etc/calamares/modules/removeuser.conf"; then
    ok "the installed system loses the live account, its sudo rule and the installer"
else
    err "the Calamares finalization does not remove the live-only state"
fi
if sh -n "$INC/usr/lib/refurb/calamares-finalize"; then
    ok "the post-install finalization script parses"
else
    err "the post-install finalization script is not valid shell"
fi

echo "== the withdrawn Software window is gone"
if [ ! -e "$INC/usr/share/applications/refurb-software.desktop" ] && \
   [ ! -e "$INC/usr/lib/refurb/refurb-apt" ] && \
   ! grep -Rqs 'refurb-software\|WindowKind::Software' "$ROOT/shell/refurb-shell/src" && \
   ! grep -q 'refurb-apt' "$INC/etc/sudoers.d/refurb-helpers"; then
    ok "no Software launcher, window, helper or passwordless rule remains"
else
    err "a Software Center entry point or privilege remains"
fi

echo "== Alt+Z dictation is wired to a working pipeline"
if [ -x "$INC/usr/lib/refurb/refurb-dictate" ] && \
   python3 -c 'import ast,sys; ast.parse(open(sys.argv[1]).read())' \
       "$INC/usr/lib/refurb/refurb-dictate" && \
   grep -q 'refurb-dictate toggle' "$INC/etc/xdg/labwc/rc.xml" && \
   grep -q 'key="A-z"' "$INC/etc/xdg/labwc/rc.xml" && \
   grep -q 'wtype' "$INC/usr/lib/refurb/refurb-dictate"; then
    ok "Alt+Z runs the offline dictation helper"
else
    err "Alt+Z is not connected to a dictation helper"
fi
if grep -q '^wtype$' "$DISTRO/config/package-lists/refurb-desktop.list.chroot" && \
   grep -q 'import vosk' "$DISTRO/config/hooks/normal/0800-dictation.hook.chroot"; then
    ok "wtype and the speech engine are installed at build time"
else
    err "the dictation dependencies are missing from the image"
fi

echo "== the analog sink is kept awake (the Jellyfin audio delay)"
if grep -q 'suspend-on-idle = false' "$INC/etc/wireplumber/wireplumber.conf.d/51-refurb-audio.conf" && \
   grep -q 'alsa_output.*analog' "$INC/etc/wireplumber/wireplumber.conf.d/51-refurb-audio.conf" && \
   grep -q 'HiFi__Speaker' "$INC/etc/wireplumber/wireplumber.conf.d/51-refurb-audio.conf"; then
    ok "the built-in output (HDA analog and SOF speaker) opts out of idle suspension"
else
    err "the built-in output still suspends on idle"
fi

echo "== amplification above 100% is real software gain"
if grep -q 'api.alsa.soft-mixer = true' "$INC/etc/wireplumber/wireplumber.conf.d/51-refurb-audio.conf" && \
   grep -q 'refurb-audio init' "$INC/usr/lib/refurb/session-launch" && \
   [ -x "$INC/usr/lib/refurb/refurb-audio" ]; then
    ok "software mixer enabled and the codec levels are normalised once per session"
else
    err "the mixer setup that makes 100-160% audible is missing"
fi

echo "== the installer is present and keeps its safety rules"
if [ -x "$INC/usr/lib/refurb/refurb-install" ] && \
   grep -q 'erase \$name' "$INC/usr/lib/refurb/refurb-install" && \
   grep -q 'is the disk this live medium is running from' "$INC/usr/lib/refurb/refurb-install" && \
   grep -q -- '--removable' "$INC/usr/lib/refurb/refurb-install"; then
    ok "refurb-install requires a typed confirmation and never writes the live or internal disk"
else
    err "the installer or its safety rules are missing"
fi
for p in squashfs-tools dosfstools e2fsprogs fdisk grub-common grub2-common \
         grub-pc-bin grub-efi-amd64-bin; do
    grep -q "^$p\$" "$DISTRO/config/package-lists/refurb-base.list.chroot" || \
        err "$p is missing, so the installer cannot finish"
done
if grep -q 'rm -f "$MNT/usr/lib/refurb/refurb-install"' "$INC/usr/lib/refurb/refurb-install" && \
   grep -q '50-refurb-live' "$INC/usr/lib/refurb/refurb-install"; then
    ok "installed systems do not keep the installer or the live passwordless sudo"
else
    err "the installer does not remove its live-only pieces from the target"
fi

echo "== Cockpit can be reached from another computer"
if [ -f "$INC/etc/systemd/system/cockpit.socket.d/10-refurb-loopback.conf" ] && \
   [ "$(ls "$INC/etc/systemd/system/cockpit.socket.d/" | wc -l)" = 1 ] && \
   grep -q 'ListenStream=127.0.0.1:9090' "$INC/etc/systemd/system/cockpit.socket.d/10-refurb-loopback.conf" && \
   grep -q '50-refurb-remote.conf' "$INC/usr/lib/refurb/refurb-cockpit" && \
   grep -q 'https://' "$INC/usr/lib/refurb/refurb-cockpit" && \
   grep -q 'ip | address' "$INC/usr/lib/refurb/refurb-cockpit"; then
    ok "loopback is the shipped default and the remote drop-in sorts after it"
else
    err "the Cockpit drop-in order or URLs are wrong"
fi
if grep -q '^cockpit$' "$DISTRO/config/package-lists/refurb-cockpit.list.chroot" && \
   grep -q 'cockpit.socket' "$DISTRO/config/hooks/normal/0400-cockpit.hook.chroot"; then
    ok "Cockpit is installed and socket activated"
else
    err "Cockpit is not installed or not enabled"
fi

echo "== sudoers rule is limited to the two helpers"
if [ "$(grep -c 'NOPASSWD' "$INC/etc/sudoers.d/refurb-helpers")" = 1 ] && \
   grep -q 'refurb-update' "$INC/etc/sudoers.d/refurb-helpers"; then
    ok "one NOPASSWD rule, the update helper only"
else
    err "unexpected sudoers content"
fi

echo "== the power key is handled by the desktop, not by logind alone"
# The desktop takes a low-level inhibitor; logind keeps its normal action for
# the case where no desktop is running at all.
if [ -x "$INC/usr/lib/refurb/session-inhibit" ] && \
   grep -q 'systemd-inhibit' "$INC/usr/lib/refurb/session-inhibit" && \
   grep -q 'handle-power-key' "$INC/usr/lib/refurb/session-inhibit" && \
   grep -q 'session-inhibit' "$INC/etc/xdg/labwc/autostart"; then
    ok "the session inhibits logind's power-key handling"
else
    err "nothing takes the power key away from logind"
fi
if grep -q '^HandlePowerKey=poweroff' "$INC/etc/systemd/logind.conf.d/99-refurb.conf"; then
    ok "without a desktop the power key still switches off"
else
    err "logind no longer has a power-key action for a machine with no session"
fi
if grep -q 'XF86PowerOff' "$INC/etc/xdg/labwc/rc.xml" && \
   grep -q 'refurb-shell --send power' "$INC/etc/xdg/labwc/rc.xml"; then
    ok "labwc sends the power key to the confirmation dialog"
else
    err "the power key is not bound to the confirmation dialog"
fi

# Every top-row control the Chromebook remap promises. A missing line here means
# a key that silently does nothing on the hardware it was written for.
missing=""
for code in 40 41 42 43 44; do
    grep -q "KEYBOARD_KEY_$code=" "$INC/etc/udev/hwdb.d/70-refurb-chromebook-keys.hwdb" || missing="$missing $code"
done
[ -z "$missing" ] && ok "hwdb maps the whole Chromebook top row" || err "hwdb is missing KEYBOARD_KEY_$missing"

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
