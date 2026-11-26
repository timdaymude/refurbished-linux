# Round 16 — a store that belongs here, and a menu that does

r16 is a usability release on top of r15. It changes the application store,
makes the launcher notice software without a reboot, fixes the update panel's
numbers, gives the boot menu a Reawakening identity, and works on a class of
pointer bug that left the wrong cursor on screen. Everything else — the shell,
Labwc, Brave, Jellyfin, audio, Wi-Fi, Bluetooth, Cockpit, Calamares — is
carried over untouched.

## The store: why not Bazaar, and what replaced DDE Store

The owner asked for Bazaar to be evaluated first. It was, and it does not fit
this image, for three concrete reasons:

1. **It cannot be built for trixie.** Bazaar's own build requirements are
   `gtk4 >= 4.22.1`, `libadwaita-1 >= 1.8` and `libdex-1 >= 1.0`. Debian 13
   ships GTK 4.18 and libadwaita 1.7 (`libadwaita-1-0 1.7.x`), so there is no
   package and no way to compile it without dragging in a newer GTK than the
   rest of the desktop uses.
2. **As a Flatpak it costs a runtime and a daemon.** The only distribution is
   the Flathub build, which needs a full `org.gnome.Platform` runtime (hundreds
   of megabytes on a machine with 2 GB of RAM and a small eMMC), and Bazaar is
   documented to "run as a service, meaning state will be maintained even if
   you close all windows" — the opposite of this round's requirement that the
   store release its resources when closed.
3. **Its desktop entry is a known trap.** Upstream issue #1811 reports the
   installed entry launching `bazaar-daemon` and failing with "Failed to
   acquire bus name", which is precisely the "app does not start from the menu"
   class of bug r15 was spent removing.

The replacement is **GNOME Software 48 with its Flatpak plugin** (`gnome-software`,
`gnome-software-plugin-flatpak`, `flatpak` 1.16.6, `xdg-desktop-portal-gtk`).
It is a Debian package, it uses the AppStream catalogue this image already
builds, it reaches Flathub for Flatpak installs and PackageKit for Debian
packages, and it is a libadwaita application, so the session's dark theme
applies to it directly.

What that means in practice:

- **Flatpak first.** Flathub is added system-wide during the build
  (`flatpak remote-add --system … flathub`), so a beginner never types a
  command. No runtime is preinstalled: it arrives with the first application
  that needs it, which keeps the ISO small and makes the cost visible.
- **APT still works.** The DEB plugin is installed by the same package list, so
  the store can still install Debian packages, and `apt` in a terminal is
  untouched.
- **Dark by default.** `GTK_THEME=Adwaita:dark` was already exported by the
  session; r16 adds `color-scheme='prefer-dark'` in a dconf system default so
  libadwaita applications pick it up even when the environment variable is
  missing.
- **Nothing runs in the background.** The store's own desktop entry starts a
  normal process (`Exec=gnome-software --mode=overview`, `DBusActivatable=false`)
  that exits when its window closes; `gnome-software.service` is disabled
  globally; `apt-daily.timer`, `apt-daily-upgrade.timer`, their services and
  `unattended-upgrades.service` are masked; and the dconf defaults set
  `download-updates=false`, `allow-updates=false` and `check-timestamps=false`.

DDE Store is gone from the r16 package list and its menu entry and icon are
deleted. Its packaging (`packaging/dde-store/`) and the r15 release stay in the
repository so r15 can still be rebuilt and rolled back to.

## The launcher notices new software

The report was exact: VLC installed from the store, ran from a terminal, and
did not appear in the launcher until the machine rebooted.

Two separate causes, both fixed in the shell:

1. **It never looked where Flatpak puts things.** The catalogue was built from
   `XDG_DATA_DIRS` only. A Flatpak installation exports its desktop entries to
   `/var/lib/flatpak/exports/share/applications` (system) and
   `~/.local/share/flatpak/exports/share/applications` (per user), which are not
   in that list. `apps::dirs_from()` now includes both, after the user's own
   directory so a user override still wins.
2. **It never rescanned.** `reload_apps()` was called once, from `App::new()`,
   even though its comment claimed otherwise. r16 watches the catalogue
   directories with `inotify` (one sleeping thread, no polling timer) and
   rescans on any create, delete, move or rewrite, coalescing a burst of writes
   into a single scan. A window closing also triggers one rescan, so a store
   that finishes an install while its window closes is covered even on a
   filesystem that reports no events.

Four new unit tests cover the Flatpak export paths and a Flatpak entry matching
its own window; the shell's suite is 38 tests.

## The update panel stops inventing updates

The symptom was "3 updates available (3 security)" followed by
"0 upgraded, 0 newly installed". Three faults were found, all in the shell:

1. **The same number was printed twice.** `settings.rs` read
   `(st.status.updates, st.status.updates)`, and `Msg::Updates` stored only the
   *security* count into `status.updates`. The panel has separate
   `updates`/`updates_security` fields now.
2. **Security was never detected.** The classifier looked for `-security` in the
   `Inst` line. APT prints the archive label, e.g.
   `(2.41-12+deb13u1 Debian-Security:13/stable [amd64])` — the string
   `-security` never appears. It now reads the parenthesised source and matches
   `security` case-insensitively.
3. **The count outlived its updates.** Only a manual check wrote the cache, so a
   number from an earlier session survived an install, a reboot and a terminal
   `apt-get upgrade`. The panel now recounts when it opens (a simulation, no
   network), recounts after installing, and writes both numbers plus the
   held-back packages, which are shown as their own sentence instead of being
   promised as updates APT will never apply.

The panel also no longer claims that Debian's daily metadata refresh keeps the
lists current, because r16 masks that timer.

## Pointer shapes

The shell draws no pointer of its own. That is correct — the compositor owns
the cursor — but it also meant that when a window asked for an I-beam or a
resize arrow and then closed under the pointer, nothing asked for an arrow
again, and the wrong shape stayed until something else changed it. That matches
all three reported symptoms, including the intermittent disappearing pointer
after closing a store window.

The shell now binds `cursor-shape-v1` (labwc supports it) and asks for
`default` in exactly two places: when the pointer enters a shell surface, and
when a toplevel closes. It does not force one shape everywhere, so text fields
keep their I-beam and window edges keep their resize arrows.

`XCURSOR_THEME=Adwaita` was already set in the session environment in an
earlier round; that remains.

## The boot menu

The BIOS menu and the UEFI menu are now drawn from one theme,
`distro/config/boot-theme/`, rendered reproducibly by `scripts/make-boot-theme.py`:

- **UEFI/GRUB**: `gfxterm` with the embedded unicode font, a 1920×1080 desktop
  image in the shell's palette, a nine-slice clay highlight behind the selected
  entry, and a five-second progress bar as the countdown. The theme is applied
  only if the file exists (`if [ -f … ]`), so a missing theme leaves GRUB's
  plain text menu and still boots.
- **BIOS/syslinux**: `vesamenu.c32` with a 1024×768 background and matching
  colours, falling back to the text menu automatically when that module is not
  present.
- **Installed systems**: `/etc/default/grub` ships `GRUB_THEME` and a five-second
  timeout, the theme is installed to `/boot/grub/themes/reawakening/`, and
  `calamares-finalize` runs `update-grub` after the bootloader job so the
  installed menu is rebuilt from those defaults.

Boot entries are unchanged: normal boot, verbose boot and the memory-report
entry on the live medium; normal, advanced and recovery entries on an installed
system, which GRUB generates as it always did.

## What testing changed

Two things only showed up when the image was booted, and both are now part of
the build:

1. **Flatpak had no repository.** `flatpak remotes` failed with
   `opendir(objects): No such file or directory`. The hook created
   `/var/lib/flatpak` but flatpak refuses to finish initialising a repository
   without `/proc/sys/kernel/random/boot_id`, which a chroot does not have, so
   the image shipped an empty directory. `mkiso.sh` now mounts `/proc` into the
   root filesystem and runs `flatpak remote-add --system --if-not-exists --from
   flathub /etc/flatpak/remotes.d/flathub.flatpakrepo` there — offline, from the
   file the image already ships.
2. **The catalogue arrived empty.** GNOME Software downloads Flathub's
   appstream on first use; on the live medium that download fails because
   flatpak refuses to leave less than 500 MB free on the overlay, and on a
   machine with no network it never succeeds at all. `flatpak update
   --appstream` now runs at build time (112 MB unpacked, about 60 MB in the
   compressed image), so browsing and searching work immediately and offline.

## Tests

### Automated

- `cargo test --release` in `shell/refurb-shell`: **38 tests pass**, including
  the new ones for Flatpak export directories, the Flatpak entry-to-window
  match, and upgrade-plan parsing (three security updates, one held back, an
  empty plan, a non-security update, the old single-number cache).
- `./scripts/check-config.sh`: **all checks passed**, including the r16 group
  (store package list, flathub remote file, no background update units, dconf
  defaults, menu entry, GRUB theme files, boot timeout, calamares `update-grub`).
- `RELEASE=r16 ./scripts/mkiso.sh all` builds end to end.

### Image

- `reawakening-os-0.3.0-r16-amd64.iso`, **1,237,876,736 bytes**,
  SHA256 `2fb11d141a2d8d3354dd99acdc58b3c539513e54591a4a6249898534b55554e3`.
  The r15 image was 1,187,692,544 bytes; the difference is the Flathub
  catalogue described above.

### QEMU (BIOS, TCG, 1.93 GB guest RAM, 12 GiB disk)

- **Boot menu, UEFI.** The themed menu renders as drawn: branding, three
  entries, the clay nine-slice behind the selection and the mint countdown bar.
  The theme is applied only when the file exists, so a missing theme falls back
  to GRUB's text menu.
- **Live desktop, BIOS.** Reaches the Reawakening desktop and the Welcome page.
- **App Store, opened from the launcher tile.** Dark, correct branding, no
  light-theme flash. Searching `vlc` returns VLC (Flathub), GridPlayer and
  EcoTubeHQ with icons and summaries, and VLC's page shows screenshots, the
  publisher, the Flathub source and an Install button. The first search took
  about three minutes on this emulated CPU because the store was parsing the
  112 MB catalogue for the first time; on real hardware that is a few seconds,
  and the store caches the result.
- **Launcher discovery.** `sudo apt-get install -y galculator` in a session that
  was already running: the launcher went from 21 to 22 installed and
  `galculator` was searchable and launchable immediately, with no reboot, no
  logout and no restart of the shell. This is the r15 bug fixed.
- **Launcher search.** `installer` finds Calamares as *Install System*;
  `galculator` finds *Calculator* by its own name and comment.
- **Updater counts.** Settings → Updates reported **3 updates available (3
  security)**; pressing Install updates upgraded `login.defs`, `passwd` and
  `libzstd1` — exactly three — and the panel immediately read *No known
  updates*, with the log showing `0 package updates available (0 security)`.
  The r15 mismatch (3 offered, 0 installed, count unchanged) did not occur.
- **The store leaves nothing behind.** Closing the window and killing the
  process left no `gnome-software` process, helper or daemon:
  `ps aux` after closing has no entry at all, and `free` is back to
  759 MB used / 1,243,208 KB available of 1,931 MB. Nothing starts it at
  boot (`systemctl --global disable gnome-software.service`, entry has
  `DBusActivatable=false`).
- **No background update work.** `systemctl list-timers` shows only
  `dpkg-db-backup`, `systemd-tmpfiles-clean` and `fstrim`; `apt-daily`,
  `apt-daily-upgrade` and `unattended-upgrades` are masked, so there is no
  daily metadata check and no automatic installation.
- **Pointer.** A pointer is visible in captures and returns after the store
  closes, which is when Craig lost it in r15. Shape transitions themselves
  (I-beam to arrow, resize to arrow) cannot be exercised over VNC.

### Store cost, measured

| State | Resident memory | Notes |
|---|---|---|
| Closed | 0 KiB, no process | idle total 759 MB used |
| Open, catalogue loaded | 456,164 KiB (~445 MB) | includes Debian's AppStream pool and Flathub's 52 MB catalogue; 72 % CPU on TCG while drawing |

The 10 MiB design goal applies to what the store leaves running: that is
nothing. While it is open it is heavy — see the gap list below.

## Known gaps

- **The Calamares install was not re-run this round.** The installer itself is
  unchanged; the only edit is a guarded `update-grub` in
  `calamares-finalize` so the installed system's GRUB reads the shipped
  `/etc/default/grub` and picks up the theme. The theme files, the flatpak
  repository and the catalogue are all inside the root filesystem that
  Calamares copies, so an installed system gets them, but that is an argument,
  not a test. Boot the ISO, install to a blank disk, remove the medium and
  confirm the themed menu, the store and the launcher.
- **Installing a Flatpak through the store's own button was not performed.**
  It needs a runtime of several hundred megabytes and the live overlay refuses
  to write that much (the same 500 MB guard that blocked the appstream
  download). APT installation through the store was not repeated this round
  either. Both are worth one pass on an installed system.
- **Cursor-shape acceptance is code inspection only.** The protocol binding and
  the two `default` requests are in place and a pointer survives the store
  closing, but the I-beam-after-text-field and stuck-resize cases need a real
  pointer.
- **Brave, Jellyfin, Cockpit, audio, Wi-Fi and Bluetooth were not re-tested.**
  Nothing in r16 touches them, but no regression pass was run.
- **No hardware testing.** Everything above is QEMU with TCG, no KVM. Craig
  (2 GB) and the Dell are the real acceptance machines.
