# Round 13: installable, administrable, audible (0.3.0-r13)

This round does four things the previous images did not: it installs to a disk,
it gives the live medium and the installed system a usable administrator
account, it makes Cockpit reachable from another computer, and it makes volume
above 100% real audio gain instead of a number that changes nothing.

Nothing from the desktop was replaced. labwc, the Rust shell, the wallpapers,
the taskbar pins, desktop icons, snapping, dictation and the dark theme are the
same code as r12. The ISO grew by about 18 MB, almost all of it the GRUB and
filesystem tools the installer needs on the target.

## 1. The installer: `refurb-install`

The r11 design (`docs/installer-design.md`) proposed a Rust GUI. This round ships
the documented fallback — a terminal program with the same steps and safety
rules — because it is one POSIX shell script that can be read end to end, runs in
`foot` from the launcher or over a serial console, and needs no toolkit.

Run it from the live desktop (launcher → **Install Reawakening OS**), from a
terminal as `sudo refurb-install`, or with a preselected disk:

    sudo refurb-install /dev/sda
    sudo refurb-install --dry-run
    sudo refurb-install --uefi /dev/sda

### What it does

1. Refuses to run unless `/run/live/*/filesystem.squashfs` exists (it installs
   the live medium's own squashfs, never a copy of the running tmpfs root).
2. Lists `/sys/block` disks with size, model and internal / removable, and
   refuses the disk that backs the live medium.
3. Shows the target's current partitions and filesystems, warns that everything
   on it will be erased, and requires the exact phrase `erase <disk>`. Anything
   else exits with the disk untouched.
4. Asks for username, full name, computer name and a password typed twice
   (hidden; minimum 8 characters). Optionally sets a separate root password;
   otherwise root stays locked.
5. Partitions GPT: UEFI = 512 MB FAT32 ESP + ext4 root; BIOS = 2 MB BIOS-boot +
   ext4 root. No swap partition (zram provides swap, as on the live image), no
   LVM, no LUKS.
6. Unpacks the live squashfs onto the new root, writes `/etc/fstab` by UUID,
   sets hostname/hosts, carries over keyboard/locale/timezone, removes the live
   account, creates the chosen account at uid 1000 in `sudo`, `audio`, `video`,
   `input`, `render` and `netdev`, sets its password through stdin (`chpasswd`,
   never an argument list), locks root unless a password was asked for, deletes
   the live passwordless-sudo rule and the installer itself from the target.
7. Copies the live session's `~/.config/refurb`, `~/.config/reawakening` and
   `~/.config/labwc` and every saved NetworkManager connection into the new
   system, so the wallpaper choice, desktop icon layout and Wi-Fi profiles
   survive the installation.
8. Removes live-boot/live-config where possible (best effort, offline-safe),
   deletes `/etc/live` and `/lib/live`, rebuilds the initramfs, installs GRUB
   (`x86_64-efi` or `i386-pc`) and runs `update-grub` with `os-prober` enabled.
9. Enables `cockpit.socket` (and network access unless declined), then offers a
   reboot.

### Why an external install cannot eat the internal disk

Only the selected disk is passed to `sfdisk`, `mkfs` and `grub-install`. A
removable target uses `grub-install --removable --no-nvram`: GRUB is written to
the target's own ESP as `/EFI/BOOT/BOOTX64.EFI` and no firmware boot entry is
added. A BIOS target only gets GRUB's MBR and core image on that disk. Nothing
reads or writes the internal ESP, the internal MBR or the firmware boot order.

### What the tests showed

On QEMU (2 GB, 2 cores, software emulation) a BIOS install to a blank 8 GB
virtio disk completed: partitioning, FAT/ext4 formatting, squashfs unpack,
account creation, initramfs, GRUB. The installed disk was then booted **with no
ISO attached** and reached the custom desktop (wallpaper, bar, pins).

- `testuser` existed at uid 1000 and was the only non-system account; the live
  `refurb` account was gone.
- `sudo id -u` with the account password returned `0`.
- `passwd -S root` reported `L` (locked) because no root password was chosen.
- `/etc/fstab` had one UUID root entry (BIOS layout), `/boot` had the kernel,
  initramfs and `grub/i386-pc`, and the hostname was `reawakening-test`.
- `/etc/sudoers.d` contained only `README` and the two validating helpers; the
  live passwordless rule was gone. `/usr/lib/refurb/refurb-install` was gone.
- A file written in the home directory survived a reboot, and the 160% volume
  setting was restored by WirePlumber.
- Brave, Jellyfin Media Player, foot and PCManFM were present in the installed
  system.

The UEFI path and the external-USB safety case are covered in section 5.

## 2. Administrator access

### Installed system

The installer creates a normal account in `sudo`. Debian's `%sudo ALL=(ALL:ALL)
ALL` rule is what grants it; the password is the one chosen during installation.
Root exists but is locked by default. The installer asks, once, whether to set a
root password:

- **No** (default): root stays locked; `sudo` is the administration path.
- **Yes**: `chpasswd` sets the password the user typed. Nothing is hardcoded and
  nothing is logged.

### Live medium

The live account `refurb` has no password by design (`passwd -d refurb`), because
tty1 autologins it without a password. That also meant ordinary `sudo` could
never authenticate, so the installer, Cockpit's network switch and the
diagnostics were unusable. r13 ships `/etc/sudoers.d/50-refurb-live`:

    refurb ALL=(ALL:ALL) NOPASSWD: ALL

This does not widen physical access — the account is already logged in at the
keyboard without a password — and it is deleted by the installer from every
installed system. Cockpit still requires a password for network login, and its
remote listener is off until `refurb-cockpit on`.

`visudo -c` validates the file during the image build. `refurb-cockpit password`
now uses `sudo passwd <user>` so it also works on the live medium, where the
account has no old password to type.

## 3. Cockpit from another computer

The reported symptom was that `https://CHROMEBOOK-IP:9090` did not answer even
though Cockpit worked locally. Two independent causes were found in the image,
not in the network:

1. **The loopback drop-in always won.** `refurb-cockpit on` wrote
   `/etc/systemd/system/cockpit.socket.d/50-refurb-remote.conf`, but the shipped
   loopback file was named `listen.conf`, which sorts *after* `50-…` and
   re-applied `ListenStream=127.0.0.1:9090`. Enabling remote access restarted
   the socket straight back onto loopback. The shipped file is now
   `10-refurb-loopback.conf`, so `50-refurb-remote.conf` is read last.
2. **With no password, no login.** The live account could not authenticate to
   Cockpit's PAM login. `refurb-cockpit password` now works (section 2) and is
   the documented step before enabling remote access.

Additional fixes:

- every printed URL is `https://` (Cockpit serves TLS with a self-signed
  certificate; the browser warns once);
- `refurb-cockpit ip` prints the current interface addresses and the URL to
  use, and `status` still shows them;
- `iproute2` is now installed, so those addresses are actually readable
  (`ip`/`ss` were missing from the image, and the fallback uses `hostname -I`
  on an older image);
- the installer enables `cockpit.socket` on the target and, unless the user
  declines, writes the same all-interfaces drop-in. The installed account has a
  real password, so remote login works after the first boot;
- Cockpit remains HTTPS-only, authenticated, and bound to the local network. It
  is never exposed to the internet by anything in the image; the script tells
  the user not to forward port 9090.

Verified from the host (a different machine from the guest's point of view)
through QEMU's forwarded port against the installed system: `GET /` returned
200, `/cockpit/login` returned 401 without credentials, and the same request
with HTTP Basic `testuser:<password>` returned 200 — a real PAM login, not just
a listening socket. Inside the guest, `systemctl is-active cockpit.socket`,
`is-enabled` and `https://127.0.0.1:9090/` (200) were also checked.

**Not verified:** clicking through Cockpit's browser UI and its web terminal.
The HTTP/TLS/PAM layer is verified; the JavaScript application was not driven.

## 4. Low volume and the 100–160% range

### Cause

The bar's percentage is the PipeWire sink volume. By default PipeWire hands
volume to the ALSA hardware mixer, whose range ends at the codec's maximum: once
it is at 100%, `wpctl set-volume --limit 1.6` still reports 1.60, but there is
nothing left for the codec to turn up. On the Chromebook that is audibly quiet,
and it looks like the amplification is broken because the number moves.

A second problem was in the r12 rule itself: it matched only
`alsa_output.*analog.*`, while many Chromebooks use the SOF/UCM name
`…HiFi__Speaker__sink`. On those machines the idle-suspend rule never matched
either.

### Fix

`/etc/wireplumber/wireplumber.conf.d/51-refurb-audio.conf` now:

- sets `api.alsa.soft-mixer = true` on ALSA cards, so WirePlumber leaves the
  hardware mixer at 0 dB and applies the whole sink volume — including 100–160%
  — in software;
- keeps `node.suspend-on-idle = false` for both the HDA analog sink and the
  SOF `HiFi__Speaker` sink, which is the Jellyfin start-delay fix.

`refurb-audio init` runs once from `session-launch` (no daemon, no resident
process) and sets `Master`, `PCM` and `Speaker` on each ALSA card to 100% and
unmuted, so the software gain is not compensating for a codec attenuated below
0 dB. `Headphone`/`Headset`/`Mic` are deliberately not touched; headphone
loudness is whatever the one sink volume says, so plugging headphones in cannot
jump to full scale. `refurb-audio status` prints the sink, its volume, the
soft-mixer property and the hardware controls for diagnostics.

`rtkit` was added: without it every audio operation logged RTKit errors and
PipeWire ran at normal priority, which is a dropout risk on the 2 GB target.
It is D-Bus activated and has no idle process.

### Verified in QEMU

The emulated HDA codec gives a real ALSA sink, so the mechanism (not the
loudness) can be checked:

- `pw-dump` shows `"api.alsa.soft-mixer": true` on the ALSA card;
- the sink is `alsa_output.pci-0000_00_04.0.analog-stereo` with
  `node.suspend-on-idle = "false"`;
- `amixer -c0` reports `Master` at `[100%] [0.00dB] [on]` after login;
- `wpctl set-volume --limit 1.6 @DEFAULT_AUDIO_SINK@ 160%` reports `1.60` while
  the hardware Master stays at 0 dB — the extra range is software gain;
- mute toggles to `1.60 [MUTED]` and back.

**Not verified:** how loud that is on the Acer's speakers, whether 160% distorts
there, headphone output, and Jellyfin playback against a real server. Those are
hardware/listening tests. The start-delay property is applied, but it cannot be
heard in a VM without a host audio backend.

## 5. UEFI and external-USB tests

### UEFI (OVMF)

A full install to a blank 8 GB virtio disk completed with the firmware in UEFI
mode. The target's ESP then contained both
`/EFI/BOOT/BOOTX64.EFI` (the removable fallback) and
`/EFI/reawakening/grubx64.efi`, the installed fstab had the root and ESP UUIDs,
and the disk booted under OVMF to the login prompt.

The first attempt failed and found a real defect: an internal UEFI install wrote
only the NVRAM boot entry, so a disk whose firmware variable was missing or
lost did not boot at all even though GRUB was on its ESP. The installer now
always writes the removable `/EFI/BOOT` path first and only then attempts the
NVRAM entry (internal disks; a USB target never adds one). The retest boots with
a **fresh, empty OVMF variable store** and still reaches the installed system
through the fallback, which is also what makes a USB install portable.

On the installed UEFI system: `uefitest` at uid 1000 in `sudo`, `sudo id -u` =
0, root locked, NVRAM/fallback present, `rtkit` active, `ip` reporting
`ens3 10.0.2.15/24`, `cockpit.socket` active, the HTTPS login from the host
returned 200, and the audio card carried `api.alsa.soft-mixer: true` with the
sink at 0 dB. (The first `wpctl` call immediately after login raced the default
node; a second call found the sink and the property.)

### External USB target

The internal disk was prepared as a stand-in for a machine that already boots
something: random data over the first 16 MB, a GPT with a 2 GB “windows”
vFAT partition, and `marker.txt` on it. A blank USB disk was attached through
`usb-storage` (so `/sys/block/sda/removable` is 1) and the installer was pointed
at `/dev/sda`.

The installer recognised it as removable and installed the BIOS layout there:
a 2 MB BIOS-boot partition, an ext4 root labelled `REAWAKENING_ROOT`, GRUB in
that disk's MBR/core image, the account, and a single-UUID fstab.

The internal disk was **byte-identical** afterwards: SHA256 of its first 64 MB
was `8c03d57d7911b0d87c178c759f8d1766d23bbf8232d916c3ebe12d647e320c94` before
and after the install; its `FAKEWIN` filesystem still mounted read-only and
`marker.txt` still read `internal-marker`; `lsblk` showed its partition table
unchanged. No bootloader or firmware entry was written outside `/dev/sda`.

Booting the USB disk **alone** (internal disk not attached) reached the login
prompt; `usbtest` logged in, `sudo id -u` returned 0, root was locked,
`cockpit.socket` was active, the host's HTTPS login returned 200, and the
desktop was drawn (screenshot taken through the QEMU monitor).

### Live Cockpit switch

On the live medium, `refurb-cockpit on` (through the passwordless live sudo)
wrote the remote drop-in, restarted the socket and `refurb-cockpit status`
reported remote access enabled, with `refurb-cockpit ip` printing
`ens3 https://10.0.2.15:9090`; the forwarded port answered from the host with
the login page (401 without credentials). `refurb-cockpit off` restored the
loopback binding and the forwarded port stopped answering. This is the exact
path that the old drop-in ordering made impossible. The test also caught that
`sudo refurb-cockpit on` (as opposed to `refurb-cockpit on`) fell through the
root branch and did nothing; the root branch now executes the same action.

## 6. Build, size and checks

- ISO: `reawakening-os-0.3.0-r13-amd64.iso`, **1,176,059,904 bytes**, SHA256
  `40ed8f124f29d3f5c7f3f926b78617b3324df982df96ca833d41015e3c9130b9`.
- `cargo` was not available in this build session (the shell sources are
  unchanged from r12); the distribution check was run instead.
- New packages: `squashfs-tools`, `dosfstools`, `e2fsprogs`, `fdisk`, `parted`,
  `grub-common`, `grub2-common`, `grub-pc-bin`, `grub-efi-amd64-bin`,
  `os-prober`, `iproute2`, `rtkit`. The GRUB *bin* packages are used instead of
  the meta packages because the latter's postinst asks for a device and cannot
  run in the rootless build.
- `scripts/check-config.sh` now checks the installer's safety strings, its
  package dependencies, removal of the live-only files from the target, the
  Cockpit drop-in order/HTTPS/IP command, and the audio software-mixer rule.
- `scripts/check-config.sh` passes. No shell source changed this round; `cargo`
  is not installed in the build session, so the 35 shell tests were not re-run
  here (their last run was on the identical r12 shell code).

## 7. What is still a physical-hardware test

- The installer on real firmware: real USB enumeration, an internal disk with
  an existing Windows ESP, and a UEFI boot menu with a USB stick. The virtual
  equivalents are in section 5.
- Loudness, distortion and headphones on the Acer Chromebook; which ALSA
  controls its codec exposes; whether the SOF `HiFi__Speaker` name is the one it
  uses.
- Jellyfin Media Player playing from a real server, including the first seconds
  of audio.
- Cockpit's browser UI and web terminal from a second physical computer.
- Wi-Fi settings carried through an install using the real radio (the copy of
  NetworkManager profiles is implemented and runs; there is no radio in QEMU).

## 8. Files

| Area | Files |
|---|---|
| Installer | `distro/.../usr/lib/refurb/refurb-install` (new), `usr/bin/refurb-install` (symlink), `usr/share/applications/refurb-install.desktop` (new) |
| Accounts | `etc/sudoers.d/50-refurb-live` (new), `hooks/normal/0500-session.hook.chroot` (chmod/visudo) |
| Cockpit | `usr/lib/refurb/refurb-cockpit`, `etc/systemd/system/cockpit.socket.d/10-refurb-loopback.conf` (renamed from `listen.conf`), `refurb-cockpit.desktop`, installer drop-in |
| Audio | `etc/wireplumber/wireplumber.conf.d/51-refurb-audio.conf`, `usr/lib/refurb/refurb-audio` (new), `usr/lib/refurb/session-launch`, package list (`rtkit`) |
| Packages | `distro/config/package-lists/refurb-base.list.chroot` |
| Checks/docs | `scripts/check-config.sh`, this file, `README.md`, `docs/limitations.md`, `docs/releases.md`, `releases/0.3.0-r13-*.sha256` |
