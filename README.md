# Refurbished Linux

A small, quick, browser-first operating system for computers that modern systems
have left behind: dual-core, 2 GB of RAM, an old integrated GPU, a spinning disk.
Version **0.3.0** (release r8) — dark by default, with a bootable ISO.

Built on Debian 13 “trixie”, but not a Debian with a different wallpaper. The
desktop is a new shell written from scratch — its own drawing code, its own
icons, its own font, its own interaction model — running on top of the tiny
`labwc` compositor.

    +--------------------------------------------------------------+
    |  R   Brave        Files  Software  Settings  Cockpit  ...  [] |
    |                                                              |
    |                                                              |
    |            Super + Space  ->  launcher                       |
    |            Super + S      ->  software                       |
    |            Super + ,      ->  settings                       |
    +--------------------------------------------------------------+

## What is in here

| Path | What it is |
|---|---|
| `shell/refurb-shell/` | The desktop shell: a Rust program that draws the bar, launcher, quick settings, software manager and settings windows itself, and takes over the session after `labwc`. |
| `shell/refurb-shell/preview/` | Screenshots rendered by the same drawing code that runs on screen (`--render-preview`). |
| `distro/` | The live-build tree that turns Debian + the shell into an ISO. |
| `scripts/` | Build and measurement scripts. |
| `docs/` | ISO build guide, release reports, hardware findings (0.4, 0.5, 0.6), component integration (0.7), the session rewrite (0.8), design, memory budget, test protocol, limitations, Windows-migration notes. |

## Status: read this before trusting anything

**Verified**

- `refurbished-linux-0.3.0-r8-amd64.iso` builds end to end and is
  **929,038,336 bytes** with a published SHA256
  (`38f428fa5497126e8f436a53c069dbf2cfb3e87e125b5cfe92d2af381bf04a10`). It
  carries both a BIOS (`isolinux.bin`) and a UEFI (`boot/grub/efi.img`) boot
  entry on an isohybrid MBR+GPT layout. The r7 ISO stays published as
  `refurbished-linux-0.3.0-r7-amd64.iso`.
- **The desktop is now a real login session** (r8). It starts from the account's
  own login on tty1, so `pam_systemd` registers a logind session with a seat, an
  `XDG_SESSION_ID`, a systemd user instance and a user D-Bus. Verified on the
  shipped ISO: `loginctl` shows session 1 as `uid 1000, seat0, tty1, user`; the
  pipewire, pipewire-pulse and wireplumber **user units** are running;
  `pkcheck` authorizes `network-control`, `settings.modify.system`,
  `udisks2.filesystem-mount`, `udisks2.eject-media`, `blueman.rfkill.setstate`
  and `login1.power-off` for the shell's own process; a second login on ttyS0 no
  longer disturbs the desktop's runtime directory (that r7 limitation is gone).
- **The taskbar is at the bottom of the screen** by default (r8), and monitors
  plugged in or unplugged while the desktop runs are picked up: registry events
  rebuild the per-screen wallpaper and bar, and the screen list is also
  reconciled against the registry once a second, so an output that appears late
  cannot be missed. Two *active* screens at once could not be produced in QEMU —
  its second GPU stays inactive under software rendering — so that path is
  code-reviewed rather than observed; see below.
- **The compositor keymap loads** (r8): the keyboard layout now comes from the
  compositor instead of a silent US fallback.
- The ISO **boots in QEMU/OVMF to the desktop from the shipped image**: the bar
  draws, the session log contains no panic, no restart and no keymap warning.
- **The Wi-Fi path works up to the password.** Clicking a network gives the panel
  the keyboard, 11 typed characters produce 11 dots, and NetworkManager stores
  exactly what was typed (`wpa-psk` / the typed password). `polkitd` and a
  `netdev` rule ship in the image, so `pkcheck` now authorizes
  `network-control` for the desktop user instead of the previous
  `Not authorized to control networking.`
- **Volume and brightness behave.** A click on the volume row shows 155% and
  `wpctl get-volume` reports `1.55`; `refurb-control volume up|down|mute` moves
  the sink and returns 0. Against a fake `/sys/class/backlight` the brightness
  row appears, writes, hides itself when the device turns read-only, and the
  shell keeps running; `refurb-control brightness` exits non-zero with a reason
  instead of claiming success.
- `refurb-diag` runs on the live image and writes its archive.
- **Established programs cover the parts the shell should not reimplement:**
  `nm-connection-editor` (Wi-Fi), `blueman` (Bluetooth) and `pavucontrol`
  (sound) open from the quick panel and Settings as child processes, and
  `mako-notifier` provides notifications. All four were verified running under
  labwc on the r7 image; a real Wi-Fi association, Bluetooth pairing and audio
  routing are still hardware tests.
- `refurb-shell` compiles (including the Wayland session backend) and 16 unit
  tests pass: desktop-entry parsing, launcher ranking, `nmcli` parsing,
  battery/memory, clock, text measure/draw, and every surface rendering
  non-empty.
- The keymap handed over by the compositor is used as-is (the NUL-termination
  bug that forced a US layout on every machine is fixed).
- The dark interface is rendered by the shipping drawing code into
  `shell/refurb-shell/preview/` and was inspected screen by screen.
- Full distro build works **without root** (`scripts/mkiso.sh all`, user
  namespace + chroot), or with `live-build` on a normal Debian host.
- `scripts/check-config.sh` validates the distribution tree (shell scripts,
  XML, JSON, pins, sudoers, masked daemons).

**Not verified — do not assume these work**

- **A real radio associating with an access point.** In the emulated test the
  client reached the DHCP stage with the correct password, but the access point
  and the client share one machine and NetworkManager's own scan tears the AP
  down, so a complete lease was not reproduced. The password path, the stored
  PSK and the authorization are verified; the radio is not.
- **A real backlight panel, and the reported brightness crash.** No backlight
  exists in the VM. The row was exercised against a fake
  `/sys/class/backlight`; the crash is addressed structurally (unwind, panic
  hook, `catch_unwind`, worker thread), not reproduced.
- **The media keys and `Super` combinations themselves.** QEMU's PS/2 keyboard
  never delivers `XF86Audio*`/`XF86MonBrightness*` keycodes. What is verified is
  the command each binding runs.
- **Audio through real speakers**, **physical keyboard layouts**, **Bluetooth
  devices**, and **Cockpit's UI**.
- **Installation to disk.** The ISO is live-only; there is no installer.
- **A second, simultaneously active screen.** QEMU gives a second GPU no usable
  mode (`Display output is not active.`), so the per-screen bar and wallpaper
  path could not be observed with two live outputs; the single-screen layout and
  the reconciliation code are what was verified.
- **A real Wi-Fi association, Bluetooth pairing/audio and a real backlight**
  remain hardware tests; see the checklist at the end of
  [docs/session-0.8.md](docs/session-0.8.md) and
  [docs/hardware-0.6.md](docs/hardware-0.6.md).
- **Volume against real hardware.** In this VM the only sink is PipeWire's
  *Dummy Output*, which ignores volume writes, so r8 could not repeat r7's
  end-to-end 100%→48% sink test; the shell's path (`wpctl set-volume` with the
  160% ceiling, then a read-back) is unchanged and was verified in r6/r7.
- **A non-US keyboard layout.** The keymap now loads, but this image is built
  with a US layout, so a German/French map was not exercised.
- **The Brave extensions.** They are configured (`normal_installed`, pinned,
  removable) but no running Brave was available to confirm they install, that
  h264ify switches YouTube to H.264, or that ResoWarden holds quality.
- **Idle memory.** `docs/memory.md` is a budget, not a measurement;
  `scripts/measure-memory.sh` has never been run on a booted system.

## Building

### The ISO (this is the product)

    ./scripts/mkiso.sh all          # no root needed, ~40 min, -> refurbished-linux-0.3.0-amd64.iso
    ./scripts/mkiso.sh squashfs && ./scripts/mkiso.sh iso   # rebuild from a staged .build/rootfs

On a Debian/Ubuntu host with root, the same image comes from
`./scripts/build-shell.sh && sudo ./scripts/build-iso.sh` (live-build).

Boot it without hardware:

    ./scripts/qemu-boot.sh                          # QEMU, software emulation
    python3 scripts/serial-tee.py .build/serial.log &
    python3 scripts/shot.py .build/shot.png

See [docs/iso-build.md](docs/iso-build.md) for the pipeline, the rootless
workarounds, requirements and external downloads.

### The shell alone

    ./scripts/build-shell.sh        # release build + .deb
    cd shell/refurb-shell && cargo run --release -- --render-preview preview

### Measure idle memory

    ./scripts/measure-memory.sh     # on a booted system, nothing open

## How it is put together

- **Compositor**: `labwc` (wlroots). No animations, no shadows, stacked windows,
  `Alt+Tab`, `Super+Space`. Everything a Windows user already knows.
- **Shell**: one Rust process, one `poll(2)` loop, a CPU rasteriser
  (`tiny-skia`) and `fontdue` for text. No GTK, no Qt, no icon theme, no
  wallpaper daemon, no notification daemon, no tray.
- **Privilege**: two shell scripts (`refurb-apt`, `refurb-update`) are the only
  things the session user may run as root, and they validate their arguments.
  The software manager is not a daemon: it is a window that runs `apt-cache`
  when you type.
- **Updates**: no checker. A once-a-day, short-lived metadata refresh keeps
  “check for updates” instant; installing is always a deliberate action.
- **Cockpit**: installed, socket-activated, bound to `127.0.0.1:9090` only.

## Philosophy

1. Idle memory first, everything else second.
2. Reliability is not negotiable for a lower number.
3. If it is not needed to browse, connect, or recover, it is not installed.
4. Background work is a bug unless the user asked for it.

See [docs/components-0.7.md](docs/components-0.7.md) for this round's component
work, [docs/hardware-0.6.md](docs/hardware-0.6.md) for the hardware round and its
test report, [docs/release-0.3.md](docs/release-0.3.md) for the desktop round,
[docs/design.md](docs/design.md) for the desktop rationale,
[docs/limitations.md](docs/limitations.md) for the honest gaps, and
[docs/testing.md](docs/testing.md) for the acceptance protocol.

## Licence

The shell and distribution configuration are GPL-3.0. Bundled Inter fonts are
under the SIL Open Font License (see `shell/refurb-shell/assets/fonts/`).
Debian packages keep their own licences.
