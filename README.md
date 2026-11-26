# Reawakening OS

**Give Your Computer a Second Life.**

A small, quick, browser-first operating system for computers that modern systems
have left behind: dual-core, 2 GB of RAM, an old integrated GPU, a spinning disk.
Version **0.3.0** (release r15, test image) — dark by default, with a bootable ISO that
installs with the Calamares graphical installer and ships the Reawakening
wallpapers.

Formerly named *Refurbished Linux*: the images up to r9 were published under that
name, and the repository, internal program names (`refurb-shell`,
`refurb-control`, `refurb-diag`, `refurb-keys`, `refurb-session`) and package
paths still use it. Only what the system shows to its user was renamed.

Built on Debian 13 “trixie”, but not a Debian with a different wallpaper. The
desktop is a new shell written from scratch — its own drawing code, its own
icons, its own font, its own interaction model — running on top of the tiny
`labwc` compositor.

    +--------------------------------------------------------------+
    |  ◠   Brave      Files  Settings  Cockpit  Terminal ...  [] |
    |                                                              |
    |                                                              |
    |            Super + Space  ->  launcher                       |
    |                                                              |
    |            Super + ,      ->  settings                       |
    +--------------------------------------------------------------+

## What is in here

| Path | What it is |
|---|---|
| `shell/refurb-shell/` | The desktop shell: a Rust program that draws the bar, launcher, quick settings and settings windows itself, and takes over the session after `labwc`. |
| `shell/refurb-shell/preview/` | Screenshots rendered by the same drawing code that runs on screen (`--render-preview`). |
| `assets/wallpapers/` | The two shipped wallpapers: `reawakening-default.png` (Tux in a blue stained-glass window, the default) and `reawakening-moonlight.png` (Tux on a moonlit beach). Installed to `/usr/share/backgrounds/reawakening/` and chosen in Settings → Appearance. |
| `distro/` | The live-build tree that turns Debian + the shell into an ISO. |
| `scripts/` | Build and measurement scripts. `scripts/clean.sh` reclaims the build output once an image is verified. |
| `docs/` | ISO build guide, release reports, hardware findings (0.4, 0.5, 0.6), component integration (0.7), the session rewrite (0.8), design, memory budget, test protocol, limitations, Windows-migration notes, and `releases.md` — every image built, its SHA256 and where a copy lives. |

## Status: read this before trusting anything

**Verified**

- `reawakening-os-0.3.0-r16-amd64.iso` builds end to end and is
  **1,237,876,736 bytes** with SHA256
  (`2fb11d141a2d8d3354dd99acdc58b3c539513e54591a4a6249898534b55554e3`). r16
  replaces DDE Store with **GNOME Software 48.3** and its Flatpak plugin, with
  the Flathub remote and the whole Flathub catalogue shipped in the image: dark,
  Flatpak-first, APT still underneath, and nothing left running when the window
  closes. The launcher now notices software installed while the session is
  running — APT or Flatpak, through an inotify watch on the desktop-entry
  directories including Flatpak's exports — so VLC appears without a reboot.
  The boot menus are a branded dark GRUB and syslinux theme with a five-second
  countdown and a text fallback, the shell asks for the default pointer when it
  gains a surface and when a window closes, and Settings → Updates reports real
  totals, real security counts and held-back packages, recounting after every
  install. Verified from the shipped ISO in QEMU: the themed UEFI menu renders;
  the store opens from the launcher tile and searches Flathub; `galculator`
  installed mid-session appears in the launcher immediately; 3 security updates
  were reported, installed and then shown as 0; closing the store leaves no
  process and idle memory returns to 759 MB of 1,931 MB; no apt-daily timer
  exists. Not yet verified: a full Calamares install, a Flatpak installed
  through the store's button, cursor shapes, and anything on real hardware —
  see `docs/round-16.md`.

- `reawakening-os-0.3.0-r15-amd64.iso` (previous release) builds end to end and is
  **1,187,692,544 bytes** with SHA256
  (`21c7477d05f6c4e7d9f8fb5832809fd49d379d9377ed8b63d26b73909745e9f3`). r15 adds
  **DDE Store** as the graphical application store, and repairs Cockpit's remote
  access. Three store faults are fixed with a locally built
  `dde-store 1.2.5-2.2~refurb2` (the Qt Wayland plugin, the home-page crash
  Debian fixed in 1.2.5-2.1, and a star-ratings request that could keep the
  catalogue empty forever), the AppStream catalogue now ships in
  `/usr/share/swcatalog/yaml` instead of dangling symlinks into the deleted apt
  lists, and PackageKit is D-Bus activated instead of masked. Verified from the
  shipped ISO in QEMU: the App Store is in the launcher and opens; every
  category lists real applications; `vlc` and `gimp` searches return the right
  results; and on a system installed from this ISO, installing `galculator`
  through the store asked for the account password, installed it, and it
  survived a reboot. Cockpit is active and enabled at boot, serves its login
  page over HTTPS on loopback and through a forwarded port, and becomes
  reachable from another machine once Settings → Remote is switched on. The
  published asset was downloaded again and re-tested from a fresh boot: its
  hash matches, the store's own Uninstall button removed `galculator` after the
  password prompt, and Brave logged in to Cockpit and ran commands in its web
  terminal. Not yet verified: an empty catalogue and a locked package database,
  Cockpit's login from a second computer's browser, and anything on real
  hardware — see `docs/round-15.md`.
- `reawakening-os-0.3.0-r14-amd64.iso` (previous release) builds end to end and
  is **1,205,600,256 bytes** with SHA256
  (`37abc90b59923e42da88a0030be3a627b5b892f89b6358cfdca234b3ea905bb6`). r14
  replaces the terminal installer with **Calamares 3.3.14** and retires the
  custom Software Center. The desktop entry launches Calamares, which walks
  through Welcome, Location, Keyboard, Partitions, Users, Summary, progress and
  Finish with the Reawakening branding; the disk is never preselected as a
  destructive target ("Erase disk" starts unchecked and Next stays disabled),
  and `prompt-install` asks again before the first write. The live medium and
  eMMC boot areas are marked read-only by the `refurb-installer` wrapper, a
  bystander disk is left alone, and the live account and its passwordless sudo
  rule are removed from the target. The Software window, its `Super+S`
  binding, its pin and the `refurb-apt` helper are gone; APT and Settings →
  Updates are unchanged. Verified in QEMU from the shipped ISO: a BIOS install,
  a UEFI install (booting with an empty variable store), an install to an
  emulated USB disk whose bystander internal disk stayed byte-identical and
  which then booted alone, cancellation leaving the disk untouched, and on the
  installed systems login, sudo, group membership, `apt-get update`, file
  persistence, Cockpit, the removed live account and the absence of any
  Software entry. `scripts/check-config.sh` passes. The progress slideshow's
  white panel is a known cosmetic bug. Physical-hardware installation is still
  to be tested; see `docs/round-14.md` section 5.7.
- `reawakening-os-0.3.0-r13-amd64.iso` builds end to end and is
  **1,176,059,904 bytes** with SHA256
  (`40ed8f124f29d3f5c7f3f926b78617b3324df982df96ca833d41015e3c9130b9`). r13 is
  the first installable image. `refurb-install` runs from the live desktop or a
  terminal, lists every disk with its size, model and whether it is removable,
  refuses the disk the live medium is running from, and writes nothing until
  the user types `erase <disk>`. It installs a UEFI (ESP + ext4) or BIOS (BIOS
  boot + ext4) system, creates the chosen account with sudo, optionally sets a
  root password and otherwise leaves root locked, carries the live session's
  desktop and saved Wi-Fi settings into the new home, installs GRUB on the
  target only (and always writes the `/EFI/BOOT` fallback, so a disk boots even
  with an empty firmware variable store or when a USB install is moved), removes
  itself and the live passwordless-sudo rule from the installed system, and can
  reboot into it. The live medium also
  gained usable administration (its already-passwordless autologin account can
  now sudo), a Cockpit remote switch that takes effect (the loopback drop-in
  used to sort after the remote one and always win), HTTPS URLs and
  `refurb-cockpit ip`, and real amplification above 100%: WirePlumber now uses
  the software mixer for ALSA cards (`api.alsa.soft-mixer = true`) while the
  codec stays at 0 dB, and `refurb-audio init` normalises the speaker controls
  once per session. Verified in QEMU: a BIOS install to a blank disk, boot of
  that disk with no ISO attached, sudo for the created account, root locked,
  Cockpit HTTPS login from the host through the forwarded port, 160% software
  gain with the hardware mixer unchanged at 0 dB, mute on/off, and file
  persistence across a reboot. Also run: a full UEFI install and boot under
  OVMF (including a boot with an empty variable store), an install to an
  emulated USB disk whose marked internal disk stayed byte-identical and which
  then booted alone, and the live `refurb-cockpit on/off` switch from the
  host's side of the forwarded port. See `docs/round-13.md`.
- `reawakening-os-0.3.0-r12-amd64.iso` builds end to end and is
  **1,158,348,800 bytes** with SHA256
  (`9291a33cf1e7d9db7f329795ff478729f28550e4fa186c94496b5b813cd685c6`). r12 makes
  the desktop usable without a full desktop environment: Brave and Files are
  pinned to the bar (the chips stay when the application is closed, one click
  starts or focuses it, closing clears only the running state, and no duplicate
  chip appears); `~/Desktop` is drawn as icons on the wallpaper (double click
  opens, dragging snaps to a grid that persists in
  `~/.config/refurb/desktop-layout`, right-click menus rename and move to the
  freedesktop trash, new files appear within two seconds); Alt+Z toggles
  offline voice dictation (Vosk small model, `pw-record`, text typed with
  `wtype`, helper and microphone gone on stop); and a WirePlumber rule sets
  `node.suspend-on-idle = false` on the built-in analog output so Jellyfin audio
  is not waiting for a suspended codec. Verified in QEMU from the shipped ISO:
  pins launch/focus/close with no duplicates, desktop files appear and open,
  rename and trash work, dragging persists, the right-click menus open, Alt+Z
  shows Starting…/Listening and the second press removes the pill, helper and
  recorder, `refurb-dictate type` inserts text into a focused window, the
  analog sink carries `node.suspend-on-idle = "false"`, 0 zombies, 537 MB idle,
  `cargo test --release` 35/35, `scripts/check-config.sh` clean. The audio fix
  itself could not be observed because this QEMU has no working host audio
  backend. See `docs/round-12.md` and `docs/performance-r12.md`.
- `reawakening-os-0.3.0-r11-amd64.iso` builds end to end and is
  **1,107,296,256 bytes** with SHA256
  (`7de10aa35bdd678f3053cfcca518822acaf086ae02f839d97af91ebcd641ed59`). r11 is
  the image that asks before it shuts down: the power key, the quick panel's
  buttons and the settings panel all raise the same modal dialog, Cancel is the
  default, Escape cancels and a click outside does nothing. The quick panel now
  closes on a click anywhere outside it (including on an application window) and
  on Escape, because it covers the output instead of being a small surface. This
  image also reaps zombie processes, ships Jellyfin Media Player, adds Cockpit
  with a loopback-only remote switch, names the cursor theme, maps the Chromebook
  top row through hwdb and `refurb-keys`, and honours Alt+F10. Verified in QEMU
  from the shipped ISO: the dialog appears and cancels, the panel closes both
  ways, Alt+F10 maximizes and keeps the bar, closing Brave removes its taskbar
  entry, Jellyfin starts to its server screen, Cockpit answers on
  `https://127.0.0.1:9090` and is refused through the forwarded port, 0 zombies,
  `cargo test --release` 24/24, `scripts/check-config.sh` clean. See
  `docs/round-11.md` and `docs/performance-r11.md`. The installer is deliberately
  not in this release; `docs/round-11.md` section 13 explains why.
- `refurbished-linux-0.3.0-r9-amd64.iso` builds end to end and is
  **929,038,336 bytes** with a published SHA256
  (`4c7548668ee635d0e60f8722ff879507a3b7445a0f7468f92e7f470344b7387e`). It
  carries both a BIOS (`isolinux.bin`) and a UEFI (`boot/grub/efi.img`) boot
  entry on an isohybrid MBR+GPT layout. The r8 and r7 ISOs stay published.
- **Wi-Fi joins instead of failing on `key-mgmt`** (r9). The panel no longer
  asks NetworkManager to guess the security method from its scan cache: the
  scan result decides (`wpa-psk` for WPA2 and transitional APs, `sae` for
  WPA3-only, `owe`, `none` for open networks, WEP with a WEP key) and the profile
  is created with exactly that method. Verified in QEMU against a WPA2 access
  point running in the VM: joining from the panel produced a profile with
  `802-11-wireless-security.key-mgmt: wpa-psk` instead of the reported
  `property is missing`, and the passphrase never entered an argument list.
- **The Wi-Fi status is real** (r9): it is read from the active connection. The
  old query used an `nmcli` form that failed every time, so the panel said "Not
  connected" on a working link.
- **One launcher and usable window controls** (r9): the brand mark is the only
  launcher, and the decoration is a theme that ships in the image (r8 named a
  theme that was not installed) with iconify/maximize/close, titlebar
  double-click, `Super+F`/`Super+Up` to maximize and F11 for full-screen; a
  maximized window stops above the bar.
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
- **The Acer Chromebook's own top-row keys.** Whether brightness and volume
  reach the compositor as `XF86MonBrightnessUp`/`XF86Audio*` — and whether they
  arrive as F6–F10 instead — depends on the machine's firmware and EC driver and
  cannot be reproduced here. `refurb-keys` shows the keycodes the kernel
  delivers, `refurb-diag` collects the keyboard sections, and `Super+F6`…`F10`
  run the same brightness/mute/volume commands if the top row does not.
- **The media keys and `Super` combinations themselves.** QEMU's PS/2 keyboard
  never delivers `XF86Audio*`/`XF86MonBrightness*` keycodes. What is verified is
  the command each binding runs.
- **Audio through real speakers or headphones**, and therefore the Chromebook's
  actual loudness. What was verified in the emulated machine is the mechanism:
  the software mixer is enabled on the ALSA card, the codec's own controls sit
  at 0 dB, 160% is applied above that, and mute works. Whether that is loud
  enough on the Acer's codec, and whether 160% distorts, needs the hardware.
- **The browser interface of Cockpit and its web terminal.** The HTTPS endpoint,
  the login page, PAM authentication with the installed account and the
  forwarded port were verified with `curl` from the host; no browser was driven
  through the UI.
- **Installation on physical hardware.** The installer was exercised against
  virtual disks (BIOS, UEFI and an external USB target). Real firmware
  partitioning, real USB enumeration and an existing Windows installation are
  not reproduced here.
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

    RELEASE=r14 ./scripts/mkiso.sh all   # no root needed, -> reawakening-os-0.3.0-r14-amd64.iso
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
  wallpaper daemon (the shell decodes the shipped PNGs itself, with the pure-Rust
  `png` crate, and paints every screen cover-fit at its own size), no
  notification daemon, no tray.
- **Privilege**: an installed system administers through `sudo` with the
  account's own password; Updates goes through its validating helper
  (`refurb-update`). The live medium's account is passwordless by design (it
  autologins), so the live image grants it passwordless sudo to make the
  installer and Cockpit usable; Calamares deletes that rule from the target.
  r14 removed the Software Center window together with `refurb-apt`: packages
  are installed with APT or a terminal, and Settings → Updates remains.
- **Updates**: no checker. A once-a-day, short-lived metadata refresh keeps
  “check for updates” instant; installing is always a deliberate action.
- **Cockpit**: installed and socket-activated, and loopback-only — on the live
  medium and on an installed system alike — until Settings → Remote is switched
  on (or `refurb-cockpit on` runs). HTTPS with a self-signed certificate, PAM
  login with the account password, plus `cockpit-networkmanager` and
  `cockpit-storaged` for the Network and Disks pages.

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
