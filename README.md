# Refurbished Linux

A small, quick, browser-first operating system for computers that modern systems
have left behind: dual-core, 2 GB of RAM, an old integrated GPU, a spinning disk.
Version **0.2.0 “Focus”** — dark by default, with a bootable ISO.

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
| `docs/` | ISO build guide, 0.2 test report, design, memory budget, test protocol, limitations, Windows-migration notes. |

## Status: read this before trusting anything

**Verified**

- `refurbished-linux-0.2.0-amd64.iso` builds end to end and is **810,549,248
  bytes** with a published SHA256. It carries both a BIOS (`isolinux.bin`) and a
  UEFI (`boot/grub/efi.img`) boot entry on an isohybrid MBR+GPT layout.
- The ISO **boots in QEMU**: boot menu, kernel, live-boot initramfs, squashfs
  mounted, systemd multi-user, our `/etc/issue` branding on the console,
  Cockpit's socket listening, login prompt offered.
- `refurb-shell` compiles (including the Wayland session backend) and 16 unit
  tests pass: desktop-entry parsing, launcher ranking, `nmcli` parsing,
  battery/memory, clock, text measure/draw, and every surface rendering
  non-empty.
- The dark interface is rendered by the shipping drawing code into
  `shell/refurb-shell/preview/` and was inspected screen by screen.
- Full distro build works **without root** (`scripts/mkiso.sh all`, user
  namespace + chroot), or with `live-build` on a normal Debian host.
- `scripts/check-config.sh` validates the distribution tree (shell scripts,
  XML, JSON, pins, sudoers, masked daemons).

**Not verified — do not assume these work**

- **The graphical session on screen.** The guest's screen stayed on the boot
  loader frame and the session unit's journal could not be read back (the live
  guest's console login needs a password this build environment does not have).
  Unknown, not “working”.
- **Installation to disk.** The ISO is live-only for now: no installer is
  included. This is the biggest gap in the milestone.
- **The Brave extensions.** They are configured (`normal_installed`, pinned,
  removable) but no running Brave was available to confirm they install, that
  h264ify switches YouTube to H.264, or that ResoWarden holds quality.
- **Wi-Fi, Bluetooth, Bravo/YouTube, Cockpit's UI, suspend** — no hardware and
  no network in the guest.
- **Idle memory.** `docs/memory.md` is a budget, not a measurement;
  `scripts/measure-memory.sh` has never been run on a booted system.

## Building

### The ISO (this is the product)

    ./scripts/mkiso.sh all          # no root needed, ~40 min, -> refurbished-linux-0.2.0-amd64.iso

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

See [docs/release-0.2.md](docs/release-0.2.md) for this round's test report,
[docs/design.md](docs/design.md) for the desktop rationale,
[docs/limitations.md](docs/limitations.md) for the honest gaps, and
[docs/testing.md](docs/testing.md) for the acceptance protocol.

## Licence

The shell and distribution configuration are GPL-3.0. Bundled Inter fonts are
under the SIL Open Font License (see `shell/refurb-shell/assets/fonts/`).
Debian packages keep their own licences.
