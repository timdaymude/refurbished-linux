# Round 17 — more walls to hang, and monitors you can arrange

r17 is a refinement release on the published r16. Nothing was rebuilt from
scratch: the shell, Labwc, Brave, Jellyfin, audio, Wi-Fi, Bluetooth, Cockpit,
Calamares, Flatpak, APT and the updater are carried over, and every change
below is additive. The release answers five owner requests — more wallpapers,
"Set as Desktop Wallpaper" on a right-click, real multi-monitor management, a
12/24-hour clock, and a boot menu that survives being scaled — and fixes one
regression it found in the process.

| | |
|---|---|
| Image | `reawakening-os-0.3.0-r17-amd64.iso` |
| Size | 1,263,861,760 bytes (r16: 1,237,876,736) |
| SHA256 | `b6941bf5b57b5f9952d4df469fef8f31af720c9f6f05570d0a2c58737459395a` |
| Rollback | r16 stays published: 1,237,876,736 bytes, `2fb11d14…5554e3` |
| Branch | `refurb/0.3` |

## Wallpapers

The built-in set is now six images at 1920×1080 (two PNG, four JPEG): stained
glass, summit, orbit, tide, dusk and paper. They ship in
`/usr/share/backgrounds/reawakening/` and are decoded with the `image` crate,
which replaces the r16 `png`-only dependency and adds JPEG and WebP.

Settings → Appearance shows all six as a grid with a preview of the current
one. Choosing a tile repaints the desktop immediately, without restarting the
session, and the choice is written to `~/.config/reawakening/wallpaper` so it
survives a reboot. A personal picture is copied (and downscaled if it is
larger than the screen) into `~/.local/share/reawakening/wallpapers/` under a
name-hash pair, so unplugging the USB stick or deleting the original does not
break the desktop; the config stores that managed copy, never the original
path.

### "Set as Desktop Wallpaper" in the file manager

Right-clicking a picture in Files now has a **Set as Desktop Wallpaper** entry.
It is a normal desktop entry (`/usr/share/applications/set-wallpaper.desktop`,
`MimeType=image/jpeg;image/png;image/webp;…`) whose `Exec` is
`/usr/lib/refurb/refurb-wallpaper %f`, so it goes through exactly the same code
path as the Settings grid: validate, install the managed copy, write the
config, repaint every screen, and post a libnotify toast ("Wallpaper changed —
<name> is now the desktop background").

r16 tried a PCManFM *custom action* (a DES-EMA file in
`/usr/share/file-manager/actions/`). PCManFM 1.4.0 in trixie does not load
those — the file is present in both the system and the user directory and the
entry never appears. The desktop entry above is the supported route and is what
ships; it also means the same item shows up in any file manager that reads
`MimeType` handlers, not only PCManFM. Running the wrapper with no argument
(that is, choosing it from an application menu) opens Settings → Appearance on
the wallpaper grid, via the new `appearance` IPC command.

## Displays

`displays.rs` reads `wlr-randr` once and parses every output: connector name,
make/model, current mode, every mode the monitor offers, whether it is enabled,
its position and its scale. Settings → Display lists them all as chips
(number, model, connector, "1280×800 · Main | On"), and the canvas draws one
draggable rectangle per screen, positioned to scale, with the arrangement
sentence underneath ("Main is 1920×1080, left of the Dell U2412M").

- **Identify** overlays a large number on every physical screen for four
  seconds, so the rectangle you are dragging can be matched to the panel in
  front of you.
- **Arrange** by dragging: the drop picks the nearest edge (left, right, above,
  below) and writes the position in pixels, so three or more screens work the
  same way two do.
- **Resolution, refresh, primary, mirror, extend, enable and disable** are all
  in the same panel. Mirror uses `wlr-randr --same-as`; extend re-enables the
  output at its saved mode.
- **Confirm or revert**: an applied change shows a 15-second banner with
  **Keep**. Doing nothing (or losing the pointer, or a black screen) puts the
  previous arrangement back and says so: "No confirmation: the previous display
  arrangement is back." That is the recovery route for a mode the monitor
  cannot display.
- **Remembered per monitor**: each output's arrangement is saved under an EDID
  identity (falling back to a port name when the monitor reports no EDID) in
  `~/.config/reawakening/displays`, and restored the first time that monitor
  appears again — including after it was unplugged and replugged. Nothing polls:
  the restore runs on the compositor's hotplug events and once at startup.
- **New monitors turn on by themselves**, at their preferred mode, with no
  dialog. Every screen gets its own wallpaper surface and its own bar; the
  screen that holds the desktop icons is the "main" one, and `Desktop`/`Bar`
  bind to it while the others use `DesktopOn(i)`/`BarOn(i)`.

## Clock

Settings → Appearance has a 12-hour and a 24-hour button. Either takes effect
on the next repaint (under a second), is stored in the same preferences file as
the wallpaper, and never touches the timezone or the system clock. There is no
daemon: the bar text is recomputed on the session's existing tick.

## Boot menu

The BIOS menu keeps the r16 artwork and gains a layout that holds up at every
supported resolution. The wordmark is now drawn into the bottom-right of the
1024×768 backdrop instead of the top-left, because `vesamenu` always prints its
entries from the top-left and the two used to overlap; the three entries,
hotkeys, countdown bar and hint line are unchanged. `gfxmode` is
`auto,1920x1080,1600x900,1366x768,1024x768,800x600`, so the same art is chosen
at 1080p, at 900p, at 768p and at the 800×600 floor, and the menu still works
if the theme cannot be loaded at all.

The UEFI image gained the storage drivers it was missing (`ahci`, `ata`,
`pata`, `scsi`, `usb`, `usbms`, `uhci`, `ohci`, `ehci`): without them the
standalone GRUB binary could not see the optical drive or a USB stick, so
`search --file /live/vmlinuz` never found the medium. With them, the image
finds the ISO on SATA and on USB and lists all three entries. UEFI uses GRUB's
plain text menu on purpose — see the gap below.

## The crash this round found, and the fix

The first r17 builds booted to a black screen with a working pointer: labwc was
running, the shell was not. The guest's own session log had the answer, two
lines per restart:

```
refurb-shell exited with status 1 (signal 0); restarting in 2s
Protocol error 2 on object zwlr_layer_surface_v1@16: layer_surface has never been configured
```

The shell was committing a buffer to a layer surface the compositor had never
configured, which is a protocol error, so labwc disconnected it. The cause was
a race in the shell's own startup: the once-a-second output check ran while the
wallpaper and bar were still waiting for their first configure, decided the
screen set had changed, destroyed and recreated both surfaces, and then the
configure for the *old* surface arrived and marked the *new* one as configured.

`rebuild_screens` is now idempotent: it returns immediately when the set of
screens and the main display are unchanged (and is re-armed when a screen
surface is closed). The startup churn disappeared, the protocol error with it,
and the desktop has stayed up across every boot since. The fix is worth noting
because it also explains the intermittent black desktop seen in a few r16
sessions on this emulator: same race, much smaller window.

## Measured on the shipped ISO (QEMU, BIOS, `-cpu qemu64`, 2 GB)

- Desktop at ~150–200 s from power-on under TCG, bar and wallpaper painted.
- Settings → Appearance: six wallpapers; Orbit applied and repainted a running
  desktop in well under a second.
- Clock: 12-hour setting showed `3:14 AM`; the 24-hour path is covered by the
  unit tests below.
- Settings → Display: `Red Hat, Inc. QEMU Monitor / 1280×800 · Main | On`,
  connector `Virtual-1`, six modes; 1920×1080 applied, **Keep** kept it; a
  second attempt without Keep reverted at 15 s and printed the "No
  confirmation…" note.
- Files → right-click a wallpaper → **Set as Desktop Wallpaper** → background
  changed to Orbit immediately, with the toast, and the session stayed up.
- Idle, 1280×800, terminal open: about 460–480 MB in use (MemAvailable 1.5 GB),
  shell RSS 54 MB, labwc 159 MB, PCManFM 45 MB, load average 0.6. These are
  emulator numbers; they are not a hardware acceptance.

## Automated checks

`cargo test` in `shell/refurb-shell`: **57 passed**, including display parsing
(mode-less modes, three-screen `setups()` ordering), the arrangement sentence,
the wallpaper catalogue and personal-install naming, and clock formatting.

## What this round did not verify

- **Installing to disk and booting the installed system.** Calamares is carried
  over unchanged, but no install run was made from this ISO.
- **Two monitors on real hardware.** QEMU here cannot present two simultaneously
  active outputs, so the drag/arrange/Identify flow was exercised only against
  the three-output headless compositor test. Craig's and the Dell's real
  EDIDs and cable arrangements still need a human.
- **The UEFI themed menu.** Details below.
- **Google Drive.** CURRENT STATUS and the master index were not updated from
  this environment.
