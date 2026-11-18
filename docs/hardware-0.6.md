# Hardware and input, 0.3.0-r6

This round started from the failures reported on the target laptop, not from a
test plan: the Wi-Fi password could not be typed, changing the brightness
appeared to crash the desktop, the volume slider was unusable, and the volume
keys did nothing. Everything below was found by reading the shipped image and
by running it, not by assuming the previous round had fixed it.

The QEMU image cannot prove that a physical radio, panel or keyboard works.
Where a result depends on real hardware it is marked **unverified**, and the
sections say exactly what was and was not measured.

## What was actually wrong

### 1. The Wi-Fi password field was never able to receive a keystroke

The quick-settings panel is a `wlr-layer-shell` surface. It was created with

    set_keyboard_interactivity(KeyboardInteractivity::None)

and that was never changed when the password dialog opened. A layer surface
with `None` is not given the keyboard by the compositor, so no key event ever
reached the shell while the panel was on screen. The shell had all the logic to
edit the password (`WifiPasswordChar`, `Backspace`, `Enter`) and a `Focus`
state, but nothing could trigger it. Typing went to whatever window happened to
be focused, or nowhere.

There was a second, independent bug behind "the characters come out wrong":

`wl_keyboard.keymap` delivers a NUL-terminated keymap string and counts the
terminator in its size. The shell read `size` bytes into a `String` and passed
that to `CString::new`, which fails on an interior NUL. The failure was silent,
so `load_xkb` returned `None` and the shell fell back to its **built-in US
layout on every machine**, whatever keyboard the user actually has. On any
non-US layout every shifted symbol, every AltGr character and every punctuation
key in a Wi-Fi password was wrong. The session log proves it:

    refurb-shell: libxkbcommon is not available; falling back to a US keymap

even though `libxkbcommon0` and `xkb-data` are installed in the image.

Fixes:

* the panel requests `KeyboardInteractivity::Exclusive` while a password is
  being entered and drops back to `None` the moment the dialog closes, so it
  never swallows typing meant for an application;
* the password field registers its own click region, shows a focus ring and a
  caret, and the placeholder says what clicking it does;
* a failed connection keeps the dialog, the focus and the typed text;
* the keymap is cut at the NUL before it is handed to libxkbcommon, so the
  compositor's real layout is used;
* the shell's error text now names the reason `nmcli` gave instead of a
  generic "check the password".

### 2. The brightness control could not be trusted, and could not be tested

Findings:

* `set_brightness` ran synchronously on the event loop and *always* reported
  success to the interface, whatever the hardware did. The next 30-second poll
  then put the real value back, so the slider appeared to reset itself.
* It preferred `brightnessctl`, which on a machine with no logind session and
  no setuid helper can fail silently, and only then tried sysfs.
* Device choice was "first directory in `/sys/class/backlight`". A laptop with
  `acpi_video0` next to `intel_backlight` can pick the firmware stub, which
  accepts writes and changes nothing on the panel.
* The release binary was built with `panic = "abort"`, so **any** panic in the
  shell - a drawing or hardware path included - killed the process. The
  supervisor then restarted it two seconds later, which is exactly the reported
  "the desktop crashed and eventually recovered".
* Every hardware write was a new process, one per pointer-motion event, from the
  same thread that draws the screen.

No backlight exists in the test VM, so **the reported crash could not be
reproduced here**. The reasons it cannot take the desktop down any more are
structural, not cosmetic:

* `panic = "unwind"`, a panic hook that writes the message and a backtrace to
  the session log, and `catch_unwind` around every action and every brightness
  write;
* brightness writes run on their own worker thread, are coalesced, and validate
  the value (`NaN`/infinity are rejected, 0 is floored to 2%);
* sysfs is tried first, on **writable** devices only, in a ranked order with
  firmware stubs last; `brightnessctl` is a fallback whose result is checked;
* the level is read back from the device, so the slider shows what the panel
  did, not what was requested;
* the control is only drawn when a writable backlight exists, and it is removed
  again if a write ever fails.

### 3. The volume slider was fighting itself

* Every pointer-motion event during a drag spawned `sh -c "wpctl set-volume
  ... && wpctl set-mute ..."`. Hundreds of processes per drag, racing, so the
  sink could end on an *older* value than the one under the pointer.
* Every event repainted **all** surfaces, including the full-screen wallpaper,
  through a CPU rasteriser.
* The 30-second status poll read `nmcli`, `bluetoothctl` twice and `wpctl`
  **on the event-loop thread**, freezing the whole interface for hundreds of
  milliseconds, and then overwrote the volume the user had just set.
* `WifiPower` and `BtPower` slept 300 ms and 250 ms on the same thread.
* The mute hotspot covered only the speaker icon, not the percentage label.

Fixes: a coalescing volume worker that keeps only the newest value and unmutes
when raising; frames are coalesced to at most one per 14 ms and only the
surfaces that changed are painted (the wallpaper never repaints on hover);
status is gathered on a worker thread and cannot overwrite a control touched in
the last 1.5 s; no `sleep` on the event loop; the whole slider row is clickable
with a hover highlight, a larger knob, and the real percentage next to it. The
0-160% range is unchanged: the track still spans the full amplification range.

### 4. Keyboard shortcuts

The labwc configuration binds `XF86AudioRaiseVolume`, `XF86AudioLowerVolume`,
`XF86AudioMute`, `XF86MonBrightnessUp` and `XF86MonBrightnessDown` to
`/usr/lib/refurb/refurb-control`. `refurb-control` now checks what happened and
exits non-zero with a reason instead of silently doing nothing, prefers sysfs
for brightness, and is also reachable as `refurb-control` on `PATH`.

The bindings themselves were checked from the shipped `rc.xml`: `Alt+F4`
closes the focused window, `Alt+Return` and `Ctrl+Return` open a terminal and
`F10` runs its command, so the file is parsed and executed. What could **not**
be checked in QEMU is the keys that carry the media functions or `Super`:
QEMU's emulated PS/2 keyboard does not deliver `XF86Audio*`/`XF86MonBrightness*`
keycodes at all, and `Super` is unreliable there. Those five keys and
`Super+Return`, `Super+Space`, `Super+L`, `Super+E` are **unverified on
hardware**; what is verified is the command each of them runs (see below).

The shell's own shortcut helper also used to look for `refurb-control` on
`PATH`, which never contains `/usr/lib/refurb`; it now uses the absolute path,
so the bar, the keys and the settings window all apply the same 160% ceiling.

### 5. Not one network request could ever succeed: polkit was not installed

This is the reason the Wi-Fi was unusable, and it was independent of the
password field. The image contained only polkit's *client* libraries
(`libpolkit-gobject-1-0`, `libpolkit-agent-1-0`); the daemon itself (`polkitd`)
was never installed. NetworkManager asks polkit before every write, and with no
daemon to answer, polkit refuses:

    NetworkManager[936]: audit: op="connection-add-activate" pid=3415 uid=1000
      result="fail" reason="Not authorized to control networking."

So "connect to this network" was refused after the password had been typed,
whether the password was right or not, and there was no dialog or agent to ask
for anything. The same refusal made `udisks2` unable to mount a drive and the
Bluetooth power toggle fail.

Packages are not enough on their own: Debian's stock policy asks for an
administrator password, and this image deliberately has nobody to type one.
The fix is both parts:

* `polkitd` is now in `config/package-lists/refurb-base.list.chroot` (with
  `polkit-agent-helper-1` setuid, its systemd unit enabled and the `polkitd`
  account created);
* `config/includes.chroot/etc/polkit-1/rules.d/50-refurb-desktop.rules` lets
  members of `netdev` (the desktop account already is one) use
  `org.freedesktop.NetworkManager.*` and `org.bluez.*` without a prompt. It is
  the same person who can already read the Wi-Fi state, on a single-user
  appliance with no administrator present.

With those two, `pkcheck --action-id org.freedesktop.NetworkManager.network-control
--process $$` returns 0 for the desktop user instead of an authorization error.

A second, smaller defect in the same area: `nmcli device wifi list` merges the
results of *every* wireless interface, so once the machine runs a hotspot its
own SSID appears in the list marked "Connected", and clicking it disconnects
instead of asking for a password. The shell now asks the interface that can
actually join a network (`iw dev` → the first `type managed` interface) for its
own scan results.

### 6. Why applications felt slow

Measured on the running image (idle desktop, 2 GB, 1280x800, software
rendering):

| process | CPU | RSS |
| --- | --- | --- |
| labwc | 12% | ~160 MB |
| brave (10 processes) | 15% + children | ~280 MB+ |
| refurb-shell | 4.9% | ~45 MB |
| pipewire/wireplumber | ~2% | ~35 MB |

Three separate causes, all real:

* the shell's own redraw and process-spawn pattern above;
* the first-run welcome page opens Brave automatically on the first boot, and
  Brave on 2 GB is the single largest consumer;
* software rendering: with no usable GPU wlroots rasterises the whole screen on
  the CPU, so every unnecessary repaint costs the applications their time.

The shell changes remove the unnecessary repaints and the synchronous probes.
Brave at first boot remains a cost; it is a browser the user asked for, not a
daemon, and nothing else is started.

A second PipeWire/WirePlumber stack appears if someone logs in on a text
console while the desktop is running: logind mounts a fresh `/run/user/1000`
over the session's runtime directory, hiding the Wayland socket and the audio
sockets. That is a real defect of starting the session outside a logind
session, it is reproducible, and it is **not fixed** in this round - use the
desktop's own terminal, not a TTY, while the desktop is running.

## Diagnostics for the target machine

    refurb-diag            # or Super+D from the desktop
    sudo refurb-diag       # for the sections that need root

writes `~/refurb-diag-<host>-<time>.tar.gz` with: kernel and OS, full PCI/USB
inventory with drivers, CPU/memory/storage, rfkill/NetworkManager/`iw` state and
firmware messages, the audio stack and its log, DRM outputs and GPU messages,
every backlight device with its permissions and raw values, input devices and
the keymap, the desktop session log (including shell panics), the session
journal, the labwc configuration and its checksum, resource pressure, the last
errors of the boot, OOM and coredump evidence, package versions and a live
`refurb-control`/`wpctl` check. Nothing is uploaded.

## Verification in this round

All of it ran in QEMU/OVMF from the built ISO, against the shipped image, by
driving the real desktop (pointer and keyboard events, screenshots), not by
calling the code directly.

Checked and working:

* the image boots to the desktop, the bar draws, the session log contains only
  `desktop session starting` - no keymap warning, no panic, no restart;
* typing into the Wi-Fi password field works: the field takes the keyboard as
  soon as a network is clicked (`KeyboardInteractivity::Exclusive`), 11 typed
  characters produce 11 dots, and Enter/Connect submits them. The profile that
  NetworkManager then holds contains exactly what was typed:
  `key-mgmt: wpa-psk`, `psk: Testpass123`;
* the polkit refusal is gone: the audit line above no longer appears, and
  `pkcheck` authorizes `network-control` for the desktop user;
* a network this machine runs as a hotspot is listed as joinable, not as
  "Connected" (`iw dev` → `ifname` scan);
* the volume slider matches the sink exactly: a click at the right place shows
  155% and `wpctl get-volume` reports `1.55`; the 160% ceiling holds;
* `refurb-control volume up` moves the sink 0.40 → 0.45, `refurb-control mute`
  mutes and unmutes, and both return 0 - this is the command the media keys run;
* brightness: with a fake `/sys/class/backlight` device at 50/100 the row
  appears within one poll and reads 50%; dragging it to 30% writes 30 to the
  device and the panel shows 30%; making the device read-only removes the row
  after the failed write and the shell keeps running;
* `refurb-control brightness up` with an unwritable device now prints the
  reason and exits non-zero instead of claiming success;
* the terminal uses a monospace font and no longer prints
  `.bashrc: Permission denied`;
* `refurb-diag` runs as the desktop user and writes its archive.

Two build defects were found by running the result, not by reading it:

* a build tree whose directories had lost their mode (an archive restore set
  them to 0700) produced an image where `/etc` and `/usr` were unreadable and
  login answered `refurb: no shell: Permission denied`. `mkiso.sh` now
  normalises directory modes and the mode of the session helpers before
  squashing;
* `sudo` had shipped without its setuid bit when the image was rebuilt from an
extracted squashfs; the same stage restores it.

## Trying it on the laptop

Boot the ISO (BIOS or UEFI; the image is isohybrid, so `dd` and Rufus both
work), then check exactly the things QEMU could not answer:

1. **Wi-Fi.** Open the quick panel, click a network, type the password. The
   panel takes the keyboard as the dialog opens, and a failure keeps the dialog
   and the reason under the field.

       nmcli -f NAME,TYPE,DEVICE connection show
       journalctl -b -u NetworkManager --no-pager | grep -i 'audit\|authorized'

   No `Not authorized` line means the polkit path is clean.
2. **Brightness.** Drag the row and watch the device move:

       cat /sys/class/backlight/*/brightness

   If the row is missing there is no writable backlight device; `refurb-diag`
   records every device with its permissions.
3. **Media and brightness keys.** Press them, then compare with the commands
   the bindings run: `refurb-control brightness up|down`, `refurb-control
   volume up|down|mute` - they print a reason and exit 1 when they cannot.
4. **Super shortcuts:** `Super+Space`, `Super+Return`, `Super+D` (diagnostics),
   `Super+L`, `Super+E`, `Super+S`, `Super+,`.
5. **Audio:** `wpctl status`, then play something and drag the slider.
6. **If the desktop ever restarts:**

       journalctl -b -u refurb-session --no-pager | tail -50

   A shell panic is logged with its message and backtrace (`refurb-shell: panic`).
7. **Send back** `~/refurb-diag-<host>-<time>.tar.gz`; it has the hardware,
   network, audio, backlight, input and session evidence for all of the above.

**Unverified** (there is no such hardware here): a real radio associating with
an access point - in the emulated test the client reached the DHCP stage with the
correct password and associated once, but the emulated AP and client share one
machine and the AP is torn down by NetworkManager's own scan, so a full lease
was not reproduced reliably; real backlight writes on a panel; the brightness
keys and the volume keys themselves (QEMU cannot deliver those keycodes);
physical keyboard layouts; and the brightness crash itself, which is only
addressed structurally. Those need the same checks on the target laptop, with
the `refurb-diag` archive and the session log.
