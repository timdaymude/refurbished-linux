# Round 11 — usability, controls and the honest gaps

Release r11 keeps the r10 desktop and changes what a person actually touches:
the power key, the quick panel, the taskbar, the keyboard's top row, Brave's
window controls, the software window's wording, Cockpit's remote switch and the
Jellyfin player. Nothing in the shell's drawing model, the compositor choice or
the wallpaper handling was replaced.

The installer is **not** in this release. See the last section and
[installer-design.md](installer-design.md); that decision is deliberate.

## 1. Shutdown and restart ask first

`Action::Power` used to call `systemctl poweroff` in the same event that drew the
button. It now sets a pending action and opens a modal:

- **Dialog** — a full-screen layer-shell overlay with a centred card. It holds
  the keyboard, so Escape cancels even when the pointer is over an application,
  and the backdrop swallows clicks: clicking outside the card does nothing
  instead of hitting the button that was underneath.
- **Cancel is the default.** Enter cancels unless Tab or an arrow key moved the
  focus to the destructive button first, so a stray Enter can never power the
  machine off. The destructive button is the only one with the clay "danger"
  fill.
- **The same dialog for restart and suspend**, with its own wording.
- **The power key.** logind acts on a short press itself, before any program
  sees the key, and `HandlePowerKey=ignore` does not hand the key to the
  compositor either. The documented mechanism is a low-level
  `handle-power-key` inhibitor lock, so the session now takes one
  (`/usr/lib/refurb/session-inhibit`, started from labwc's autostart) and labwc
  binds `XF86PowerOff` to `refurb-shell --send power`. When the desktop is not
  running the lock is gone and logind's own action is back, so a recovery
  console still switches off normally. Holding the key for several seconds
  still cuts the power through the embedded controller, with no software
  involved.

The scroll of commands a confirmation dialog may run is deliberately short:
`systemctl poweroff|reboot|suspend`, through polkit, which the image already
authorizes for the desktop account (`login1.power-off`).

## 2. Quick settings close when you leave them

r10's panel was a small layer surface in the corner, and a Wayland surface only
receives the clicks that land on it: a press on the desktop or on a window went
to whatever was underneath, so the panel never heard about it and stayed open.
Escape had the same problem for a different reason - the panel only took the
keyboard while a Wi-Fi password was being typed, so the key went to the window
behind it.

The panel now covers the output with a transparent layer surface and draws its
card in the corner (`ui::quick::card_rect`). Everything outside the card is
therefore the panel's own:

- a press on the desktop, the bar or an application window closes it;
- Escape closes it, because the surface holds the keyboard while it is open;
- the gear, the Wi-Fi and Bluetooth tiles still toggle it, and the X in the
  header is unchanged;
- a press inside the card does nothing, and a drag on the volume or brightness
  slider never closes the panel, so an adjustment in progress is not cut short.

The one cost is that the first press outside is swallowed by the panel instead
of reaching the window behind it, which is what every popup does. While the
confirmation dialog is open the IPC shortcuts (`Super+Space`, `--send software`,
...) are ignored: a modal stays modal.

## 3. The taskbar follows the windows

A closed window is now removed from the bar immediately. `Closed` used to set a
flag and wait for a later `Done` event to rebuild the list; a client that sends
`Closed` and nothing else left its entry in the taskbar after the window was
gone. Our own Software and Settings windows go through the same path.

## 4. Maximizing Brave

Brave draws client-side decorations, so its maximize control sits inside its own
tab-strip area instead of in a labwc titlebar. The obvious fix - asking the
compositor for server decorations with
`<windowRule identifier="brave-browser" serverDecoration="yes"/>` - **is not in
this build**, and the reason is worth recording. The first r11 test image was
booted with QEMU's `-cpu max`, and there labwc was killed every few seconds and
the session restarted in a loop (section 14). The rule was taken out while
chasing that; the harness was then changed to `-cpu qemu64` and the image became
stable with the rule already gone. Nobody has since booted this round with the
rule in place on a stable harness, so it does not ship: a compositor that dies
is not an improvement over a button that is easy to miss, and an unverified
window rule is exactly what cost this round an afternoon.

What the release provides instead:

- `Alt+F10` maximize/restore, the convention on GNOME and KDE, added next to the
  existing `Super+F` and `Super+Up`.
- Double-clicking Brave's own header maximizes and restores, because Chromium
  handles that itself.
- Brave's own titlebar still has maximize/restore next to its tab strip.
- `F11` and `Super+F11` remain true full-screen, which covers the taskbar; a
  maximized window stops above it, as before.

## 5. Chicken-and-egg zombies

Every program the shell started was a `Child` that was dropped without a wait.
The shell lives for the whole session, so every closed application stayed in the
process table as a zombie - the `Z` processes visible in the hardware
screenshots. `src/reaper.rs` keeps a list of adopted children and calls
`try_wait` on all of them on a 500 ms tick, so a long-running browser never
blocks the cleanup of a short-lived helper. `system.rs` and `apps.rs` both go
through it, and a unit test adopts a 30-second `sleep` before three `true`
processes to prove the short ones are still collected.

## 6. Cursor

`XCURSOR_THEME=Adwaita` is now set for the session. Without a theme name,
wlroots looks for the bare `left_ptr` cursor and a machine whose theme does not
provide it can keep the last shape a client requested. The shell also drops its
drag state when the pointer leaves its surfaces, so a slider drag that ends
outside the panel cannot leave the shell fighting the next status poll.

This reduces the causes; it is **not** a reproduction of the reported stuck
text-cursor in Brave. See the limitations.

## 7. Chromebook top row

The top row reaches Linux in one of two ways: as the proper `XF86` media keysyms
(in which case the existing labwc binds already work), or as plain `F6`–`F10`,
which is what made `F7` open Brave's caret-browsing prompt instead of dimming the
screen.

`/etc/udev/hwdb.d/70-refurb-chromebook-keys.hwdb` rewrites the kernel key codes
for the internal keyboard of Google hardware - atkbd scancodes `0x40`–`0x44`,
which are F6–F10 - to `brightnessdown`, `brightnessup`, `mute`, `volumedown`,
`volumeup`. It matches only an atkbd device whose DMI vendor is Google, so
external keyboards and other machines are untouched, and it cannot fire twice on
a model that already sends XF86 keysyms.

`refurb-keys` was rewritten around the question it exists to answer: it lists
the keyboards and their modalias, says whether the remap is installed, runs the
same `refurb-control` commands the keys run and reports whether the hardware
actually changed, and `refurb-keys --capture` shows the raw key codes (the
desktop account is in the `input` group for that).

## 8. Software window: what it really is

The window was already a front end to `apt-cache` and `apt-get`, not a
seventeen-item list; what was wrong was the wording. The header now says how many
*desktop applications* are on the computer and that results come from Debian's
catalogue, and the empty state explains that Reawakening OS installs what Debian
packages instead of implying a broken search.

Searching now falls back from package names to descriptions when the name search
finds almost nothing, which is how people actually search ("video player",
"photo editor"). Install, remove, dependencies and updates are Debian's own:
`apt-get` through `refurb-apt`, and Settings → Updates through `refurb-update`.

## 9. Jellyfin Desktop

`jellyfin-media-player` (Jellyfin's official desktop client, Qt 6 + libmpv) is
installed from Jellyfin's own Debian archive, with the archive key shipped in the
repository and copied into the image by the build. It is deliberately the
*player*, not the Jellyfin server, and it uses the same libmpv/VA-API path as the
rest of the system rather than a second multimedia stack. `qt6-wayland` is
installed so it runs as a Wayland client like every other window.

Playback against a real server is not testable here; what is verified is listed
below.

## 10. Cockpit

Two separate problems, both fixed or explained:

- **The socket was loopback only** (`127.0.0.1:9090`), by design: a live USB
  whose desktop account has no password must not offer a web login to the
  network. `refurb-cockpit status` explains the state, `refurb-cockpit on`
  switches the socket to every interface through a systemd drop-in, and
  `refurb-cockpit off` puts it back. Enabling it needs the account's password,
  as any administrative action does, and the script says so.
- **The live account has no password**, so Cockpit's PAM login cannot succeed
  from another computer even with the socket listening. `refurb-cockpit
  password` sets one. An installed system has a real password already, which is
  why this is a live-medium problem.

The script never forwards ports, never installs a firewall exception and tells
the user not to expose port 9090 from a router.

## 11. Audio at 160%

160% is PipeWire's software gain on the sink (`wpctl set-volume --limit 1.6`),
not extra hardware amplification: above 100% the samples are scaled up in the
graph, so a quiet laptop whose ALSA mixer is at half volume stays quiet and
clipping becomes possible. The round did not change the ceiling or the default:
`refurb-control` reports what the sink did, and `refurb-diag` now collects
`wpctl status`, `wpctl inspect` on the default sink and `amixer contents` so the
hardware mixer can be compared with the software level on the machine that is
quiet.

No output level was raised blindly; see the limitations for what could not be
measured without the machine.

## 12. Performance

- Zombie processes: fixed (section 5).
- Hardware video decoding: the Brave launcher enables
  `VaapiVideoDecodeLinuxGL`, and `libva2`, `libva-drm2`, `vainfo` are installed
  so the GPU can be checked with `vainfo`. `refurb-diag` collects `/dev/dri` and
  `vainfo` output.
- Measurement method and numbers are in [performance-r11.md](performance-r11.md).
  They are QEMU numbers, not the Chromebook's.

## 13. The installer is not in this release

[installer-design.md](installer-design.md) describes what a Reawakening OS
installer has to be, and this round added a review of it rather than an
implementation. The live image still installs with Debian's own installer, which
is small, reliable and looks like Debian. A half-built installer that can lose a
disk is worse than a plain one that works, so the decision for r11 is explicit:
no installer work shipped, the design is reviewed and waiting, and it is the
first item on the next round.

## 14. How this round was tested, and what that is worth

Everything below ran on the published image in **QEMU system emulation** (2
emulated cores, 2 GB, `llvmpipe`, no GPU, no KVM), driven over QMP and the serial
console:

- boot to the desktop with the bottom bar and the wallpaper;
- serial login as `refurb` with an empty password;
- `systemd-inhibit --list` showing exactly one `handle-power-key` lock held by
  the session;
- `refurb-shell --send power` raising the confirmation, Escape cancelling it,
  and a click outside the card leaving the machine alone;
- the quick panel: the gear opens it, Escape closes it, a press on the Brave
  window closes it;
- Alt+F10 maximizing a window and leaving the bar in place;
- closing Brave from its own close button and its taskbar entry disappearing;
- Jellyfin Media Player starting to its server-address screen and appearing in
  the taskbar;
- Cockpit answering on `https://127.0.0.1:9090` (200, `cockpit.socket` active)
  while the forwarded host port is refused - the loopback binding working;
- 0 zombie processes after launching and closing applications;
- `cargo test --release` 24/24 and `scripts/check-config.sh` clean.

Two lessons about the harness itself, both of which cost real time:

- **`-cpu max` is not a safer choice.** With it, labwc died in a loop and the
  session restarted every few seconds, which looked exactly like a regression in
  this round's changes. The same image is stable with `-cpu qemu64`. Images are
  now booted with the model the r10 verification used.
- **A stale package hides a fix.** `mkiso.sh configure` only builds the shell
  when `distro/config/packages.chroot/refurb-shell_<version>.deb` is missing, so
  an earlier build's binary was silently reused and two UI fixes appeared not to
  work. Delete the `.deb` when the shell source changed.

None of this is hardware verification. The Chromebook's top row, backlight,
speakers, video decoding, cursor and power button are still untested, and the
limitations file lists them as such.
