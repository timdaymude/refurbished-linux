# Controls round: login password, volume, quick settings, brightness (0.3.0-r5)

Investigation of the reported usability failures, done against the running
system in QEMU (OVMF UEFI, 2 GB, software rendering). Every claim below is
either backed by something measured in the VM or explicitly marked unverified.
There is still no physical laptop to test on, and that limits several items.

## 1. The login password field

**What the system actually has.** There is no graphical login screen and no
display manager: `refurb-session.service` starts the desktop directly as the
account with uid ≥ 1000 (`session-start` resolves it), and on the live image
that account has an empty password. So the machine never asks for a password at
boot. The only password prompts are:

- the lock screen, `Super+L` → `swaylock`, which does have a password field;
- the text consoles `tty2`–`tty6`, which use the normal `login` prompt.

**What was wrong with the lock screen.** Two real defects:

1. **Locks stacked.** `lock_screen()` spawned `swaylock` every time the lock
   action ran and never checked whether one was already running. Two lockers
   both grab the keyboard, so every keystroke is delivered twice and the typed
   password can never match — the field appears to ignore or corrupt input.
   Observed directly: `pgrep -a swaylock` returned two live instances after
   pressing the shortcut more than once.
2. **The field was invisible.** `swaylock` hides its indicator until the first
   key press and the invocation passed no colours, so the screen was a plain
   dark rectangle with no visible place to type and no feedback that keys were
   arriving — matching "difficulty entering my password" and "does not lose
   focus" (there was nothing on screen to focus).

**Fixes.** `lock_screen()` now refuses to start a second locker
(`pgrep -x swaylock` guard) and starts it with `--indicator-idle-visible` plus
readable ring/text colours, so the field is on screen before the first keystroke.

**Verified in the VM:** `Super+L` locks; the lock screen is drawn; the indicator
is visible after the change where it was absent before; PAM receives the typed
password (`pam_unix(swaylock:auth): authentication failure` in the journal after
a deliberately wrong entry), so keystrokes do reach the field; pressing the
shortcut twice no longer stacks lockers.

**Not verified:** completing a full unlock with a mixed-character password. One
run returned to the desktop while typing and another did not, and the VM's
`/run/user/1000` is replaced by any console login (the known tmpfs problem),
which makes lock-screen observations unreliable here. This needs a real machine.

## 2. Volume sliders

**Root cause found.** `widgets::slider` registered its click target as the
*track only* — a 6-pixel-tall line inside a 30-pixel row. Clicks anywhere else
in the row (the obvious place to click, and the whole area visually part of the
control) landed on nothing, which is exactly "does not respond consistently".
Brightness used the same helper, so it had the same defect.

**Second root cause.** `set_volume()` never lifted a mute. `wpctl` keeps the
mute flag across `set-volume`, so dragging the slider on a muted sink changed
the number while the machine stayed silent, and the next 30 s poll put "Muted"
back — the "returns to mute" report.

**Fixes.**
- The whole row is the hit area; the session still converts the click's x
  position within the track into a value, so the slider maths is unchanged.
- `set_volume()` runs `set-volume` and `set-mute … 0` in one shell when the
  target level is above zero, so raising the volume always produces sound.
- 160 % ceiling unchanged (`VOLUME_MAX = 1.6`, `wpctl --limit 1.6`).

**Verified in the VM:** with the panel open, a click at the *top edge* of the
volume row (surface y = 126, row spans 122–152) now resolves to
`VolumeSet(...)` and dispatches it; under the old geometry that point was
outside the 6-pixel track and did nothing. The quick panel opens from the bar
and shows the volume row, the Wi-Fi/Bluetooth toggles and the footer buttons.

**Not verified here:** the audible result of that click. This boot had no audio
hardware attached, so PipeWire exposed only "Dummy Output", and a console login
created a second PipeWire/WirePlumber stack — the two together make `wpctl`
numbers meaningless in that state. Real volume changes, the 160 % ceiling and
audible output were verified in the previous round with an emulated HDA codec.

## 3. Quick settings panel

Each control was checked against what it claims to do: the volume slider (fixed
above), the Wi-Fi and Bluetooth rows (they open lists and dispatch scans), the
Settings / Software / Updates buttons (they open the corresponding windows) and
Lock / Restart / Shut down (they dispatch the power actions). The panel opens
from both the speaker icon and the gear in the bar.

**Brightness row behaviour is correct and was verified by its absence:** the row
is only added when a backlight device exists (`status.brightness.is_some()`), so
in this VM — which has no backlight — the panel shows volume and nothing else
rather than a control that cannot work.

## 4. Brightness

- The slider and the keys share the same ceiling and the same guard against a
  fully dark screen (2 % minimum).
- `set_brightness()` used to do nothing at all when `brightnessctl` was missing.
  It now falls back to writing `/sys/class/backlight/*/brightness` directly,
  which the shipped udev rule makes writable for the desktop user.
- `refurb-control brightness up|down` got the same fallback, so the keys keep
  working on an image without the helper.

**Unverified:** brightness on real hardware. QEMU has no backlight device, so
there is nothing to dim; the only thing that could be verified here is that the
control is hidden when there is no device, which it is.

## 5. Function keys

Volume up/down/mute go through `refurb-control`, which uses `wpctl` with the
same 160 % ceiling as the slider; brightness up/down use `brightnessctl` or the
sysfs fallback. Keyboard and panel therefore drive the same backend and cannot
disagree about the ceiling.

**Verified:** the volume keys change the sink (0.50 → 0.65 → 0.55 in the
previous round) and the helper unmutes on volume-up.
**Unverified:** brightness keys on real hardware; the helper exits non-zero
(rather than pretending) when no backlight exists.

## 6. What is still open

- A password-based unlock has not been demonstrated end to end in the VM.
- No real backlight, no real audio device, no physical keyboard.
- The lock screen is the only graphical password field; if a login prompt is
  expected at boot, that is a feature the system does not have yet — worth
  deciding on deliberately rather than discovering by accident.
