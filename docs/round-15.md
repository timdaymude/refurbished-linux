# Round 15 — DDE Store and Cockpit

**Image:** `reawakening-os-0.3.0-r15-amd64.iso` (prerelease, for hardware testing)
**Size:** 1,187,692,544 bytes (1.11 GiB)
**SHA256:** `21c7477d05f6c4e7d9f8fb5832809fd49d379d9377ed8b63d26b73909745e9f3`
**Base:** r14 (unchanged desktop, Calamares, Brave, Jellyfin, dictation, wallpapers)
**Branch:** `refurb/0.3`

This round gives Reawakening OS a graphical application store: DDE Store,
patched so that it actually works on Debian 13 with the stock Qt 5 DTK, plus a
repaired and re-verified Cockpit. Every claim below was measured on the shipped
ISO, booted in QEMU, and — where marked — on a system installed from that ISO.

## Why DDE Store failed before

Three independent faults, all reproduced here before any change:

1. **Missing Qt Wayland platform plugin.** `libqt5waylandclient5` (Debian's
   `qtwayland5`) was not installed, so the store could not create a Wayland
   surface at all. Fixed by installing the package.
2. **Segmentation fault on the home page.** Debian's `dde-store` 1.2.5-2 loops
   `for (int i = 0; i < 10; i++)` over the applications AppStream returned for a
   category. Any category with fewer than ten entries walks off the list and
   crashes. This is the same defect Debian fixed in 1.2.5-2.1
   ([#1117833](https://bugs.debian.org/cgi-bin/bugreport.cgi?bug=1117833)).
   1.2.5-2.1 is a Qt 6 port and cannot be installed on trixie's Qt 5 DTK, so the
   one-line fix (`qMin(10, items.size())`) is backported in
   `packaging/dde-store/002-homepage-short-category.patch`.
3. **The catalogue never appeared, even without a crash.** The store's backend
   only emits category and search results after `RatingsHelper::fetched`, and
   the star-rating request to `https://odrs.gnome.org/1.0/reviews/api/ratings`
   never settled in the test VM (its DNS answer was IPv6-only and the guest had
   no IPv6 route). The window opened, the category rows stayed empty forever.
   `003-ratings-must-not-block-catalogue.patch` gives that request a four-second
   transfer timeout plus a four-second fallback timer that emits `fetched` once,
   so the catalogue appears with or without ratings.

Both patches are built into `dde-store_1.2.5-2.2~refurb2_amd64.deb` from the
unmodified Debian source, reproducibly, by `packaging/dde-store/build.sh`;
nothing else in the store's dependency set is upgraded.

## The catalogue was empty for a fourth reason, found here

The r15 image shipped `dde-store` 1.2.5-2.2~refurb1, opened without crashing,
listed its categories — and showed no applications in any of them. The cause
was not the store:

- Debian's AppStream apt hook fills `/var/lib/swcatalog/yaml` with **symlinks
  into `/var/lib/apt/lists`**.
- `0600-slim.hook.chroot` deletes those lists to save space, so every symlink
  dangles.
- libappstream therefore loads zero components; `appstreamcli status` reported
  "YAML: 4" (files) and 19 components (all of them local `.desktop` files).

The first version of the build guard did not catch this either: it grepped
`appstreamcli search vlc` for the word "vlc", which also matches the error line
`No component matching 'vlc' found`. Both the fix and the guard are now honest:

- `0450-store.hook.chroot` copies the DEP-11 YAML itself into
  **`/usr/share/swcatalog/yaml`** (a system catalogue directory libappstream
  reads and never prunes), removes the apt-owned `/var/lib/swcatalog/yaml`, and
  rebuilds the cache: 4 files, 7.8 MiB.
- The guard hides `/var/lib/apt/lists`, drops the cache, and then requires
  `appstreamcli get org.videolan.vlc` and `org.gimp.GIMP` to succeed. A missing
  catalogue fails the build instead of the user's first click.

That is what the r15 ISO in this round contains, and it is why the catalogue is
now full.

## What was verified on the shipped ISO (QEMU, 2 GB RAM, no KVM)

All of the following ran from a clean boot of
`reawakening-os-0.3.0-r15-amd64.iso`.

**Launch.** Super+Space, type "app store", Enter. The launcher shows *App
Store* with its bag icon and the description "Find, install and remove
applications". The store opens on the home page; no segmentation fault.

**Catalogue.** The home page lists real applications per category (Messaging:
Birdtray, Nicotine+, Dino, Pidgin; Internet: Geary, Wireshark, Wike, KRDC;
Games, Development, Office, Graphics, Video, Music follow), each category with
its "View More" button.

**Search.** `vlc` → FreetuxTV, Synclpay, VLC. `gimp` → BleachBit, GNU Image
Manipulation Program, Shutter, XSane, xzgv. `jellyfin` → **no results**
(see limitations: Jellyfin Media Player comes from a third-party repository and
is not in Debian's catalogue; it is a valid local AppStream component but the
store searches the catalogue, not installed packages).

**Install, with authentication.** On the installed system: search
`galculator`, open its item page, press Install, the polkit dialog
"Authentication is required to install software" appears, the account password
is accepted, PackageKit downloads and installs. The button then reads
*Uninstall* and `dpkg -l galculator` reports `2.1.4-2` installed. The package's
desktop entry is at `/usr/share/applications/galculator.desktop`, so it appears
in the launcher like any other application.

**Persistence.** After a full power cycle, `galculator 2.1.4-2` is still
installed and the store still opens with its catalogue.

**Removal.** The package was removed on the installed system and
`brave-browser`, `cockpit`, `dde-store` and `jellyfin-media-player` were all
still installed afterwards. The removal was performed with APT, not with the
store's Uninstall button — that one click is the remaining gap (see
limitations).

**Performance (measured, TCG emulation, 2 GB guest).**

| Measurement | Value |
|---|---|
| ISO size, r15 | 1,187,692,544 bytes |
| ISO size, r14 | 1,205,600,256 bytes |
| Delta | −17,907,712 bytes (smaller, despite the new store and catalogue) |
| Catalogue added | 7.8 MiB of DEP-11 YAML in the image |
| Store + Cockpit package set, installed size | ≈ 52.6 MiB |
| `dde-store` resident memory, open, catalogue loaded | 257,520 KiB (≈252 MiB) |
| `dde-store` CPU while loading the catalogue | 27.8 % of one core (TCG; a real CPU is far lower) |
| Processes after closing the store | `packagekitd` exits on its idle timeout; no store process remains |
| Idle RAM after reboot, store closed | 601 MiB used, 1,171 MiB available |
| Boot-time daemons added | none: `packagekit.service` is D-Bus activated and has no `[Install]` section |

The store is heavy once open — DTK plus the whole 35k-component AppStream pool.
On a 2 GB machine it fits (≈1.1 GiB still available) but it is not a 10 MiB
addition; the idle target is met by not running anything when it is closed,
which is the part the system controls.

## PackageKit

`0200-no-background-work.hook.chroot` used to mask `packagekit.service`, which
also disables D-Bus activation — the reason the store could never talk to it.
Round 15 unmasks the unit but leaves it disabled: nothing starts it at boot, the
store's first call activates it, and it exits when idle. The offline-update
service stays masked, no update checker is added, and Settings → Updates is
untouched.

## Cockpit

Checked on the installed system, which is where the user's report came from.

- `cockpit.socket` is active and enabled at boot; `cockpit 337-1~deb13u2` with
  `cockpit-networkmanager` and `cockpit-storaged` installed (the Network and
  Disks pages need them).
- The socket serves the full login page (`login-user-input`,
  `login-password-input`, `login-button`) over HTTPS with the self-signed
  certificate, both on loopback inside the guest and through a forwarded port
  from another machine.
- **LAN access is off by default.** `/etc/systemd/system/cockpit.socket.d/10-refurb-loopback.conf`
  restricts the socket to loopback until it is enabled; this is why a user
  cannot reach `https://CHROMEBOOK-IP:9090` on a fresh install.
- `refurb-cockpit state` → `socket=active`, `remote=off`, and after
  `refurb-cockpit on` the same URL answers **HTTP 200** from the other machine.
- Settings → Remote is the beginner-facing control: service availability, the
  current local address, the full URL with a copy button, an enable/disable
  switch that authenticates through `pkexec` and the
  `org.reawakening.cockpit.policy` action, and an Open button that launches the
  URL in Brave. The Cockpit tile is also in the launcher.

Not verified in this round: an interactive browser login followed by the web
terminal on the forwarded origin. The host-side headless browser available in
this environment cannot initialise NSS, and the guest browser could not be
driven reliably through VNC in the time available. That test is the first item
for hardware acceptance.

## Install and boot from the ISO

Calamares was walked through end to end on a 12 GiB virtio disk: Welcome (English
UI, America/New_York), Keyboard, Partitions ("Erase disk", `/dev/vda`,
ext4 + ESP as before), Users (`testuser`, host `reawakening-test`), Summary,
Install. The USB test stick had to be removed first, because Calamares
preselects the first enumerated disk and the stick was `/dev/sda`. The install
ran to "All done. Reawakening OS 0.3.0 has been installed." The VM was then
powered off and booted with **no ISO attached**: the installed system came up on
its own, autologged in, and both `packagekit` (on demand) and `cockpit.socket`
were present exactly as in the live session.

## Changes in the tree

- `packaging/dde-store/` — patch 002, patch 003, `build.sh`, `README.md`;
  built package `distro/config/packages.chroot/dde-store_1.2.5-2.2~refurb2_amd64.deb`.
- `distro/config/package-lists/refurb-store.list.chroot` — `dde-store`,
  `qtwayland5`, `appstream`, `packagekit`, `packagekit-tools`, `mate-polkit`,
  `pkexec`, `wl-clipboard`.
- `distro/config/package-lists/refurb-cockpit.list.chroot` — adds
  `cockpit-networkmanager`, `cockpit-storaged`.
- `distro/config/hooks/normal/0450-store.hook.chroot` — PackageKit unmasked but
  not enabled, offline updates masked, DEP-11 shipped in
  `/usr/share/swcatalog/yaml`, catalogue verified without apt lists, polkit
  agent and desktop entry checked.
- `distro/config/hooks/normal/0200-no-background-work.hook.chroot` — PackageKit
  no longer masked.
- `distro/config/includes.chroot/usr/lib/refurb/refurb-cockpit` — `state` mode,
  `PKEXEC_UID` user resolution, socket enablement.
- `distro/config/includes.chroot/usr/share/polkit-1/actions/org.reawakening.cockpit.policy`.
- `distro/config/includes.chroot/etc/xdg/labwc/autostart` — starts
  `mate-polkit`, without which every install answers "Not authorized".
- `distro/config/includes.chroot/usr/local/share/applications/dde-store.desktop`,
  `usr/share/icons/hicolor/scalable/apps/deepin-app-store.svg`.
- `shell/refurb-shell/` — `Section::Remote` settings panel, `Glyph::Store`,
  actions `RemoteToggle/RemoteRefresh/RemoteCopy/RemoteOpen`. `cargo test
  --release`: 33 tests pass.

## Next starting point

1. Boot the ISO on the Chromebook and repeat the acceptance script in
   `docs/hardware-0.4.md`: open the launcher, App Store, search and install a
   small package with the password, launch it, remove it, reboot and confirm.
2. In Settings → Remote enable LAN access, then log into
   `https://CHROMebook-IP:9090` from another machine and open the Terminal page.
3. Record idle RAM with the store closed and with it open.
4. Decide whether the store's light window and missing icons are worth a theme
   fix (see limitations) or acceptable.
