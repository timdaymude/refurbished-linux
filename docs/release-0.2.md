# Refurbished Linux 0.2 — development round 2 report

**Milestone:** first bootable ISO.
**Date:** November 2026.
**Base:** Debian 13 “trixie”, kernel 6.12, amd64.

## A. The ISO

| | |
|---|---|
| File | `refurbished-linux-0.2.0-amd64.iso` |
| Version | 0.2.0 (codename “Focus”) |
| Size | 810,549,248 bytes (773 MiB) |
| SHA256 | see `refurbished-linux-0.2.0-amd64.iso.sha256` next to the image |
| Firmware | BIOS (`isolinux.bin`) **and** UEFI (`boot/grub/efi.img`) |
| Media | isohybrid + GPT: CD/DVD, `dd` to USB, UEFI and legacy BIOS |
| Contents | kernel 6.12, live-boot initramfs, 709 MB zstd squashfs |

Verified on the image itself (not on a build log):

    El Torito boot img : 1  BIOS  y  /isolinux/isolinux.bin
    El Torito boot img : 2  UEFI  y  /boot/grub/efi.img
    Boot record        : El Torito , MBR isohybrid cyl-align-on GPT
    Volume id          : 'REFURBISHED_LINUX'
    Application id     : Refurbished Linux 0.2.0

## B. What was actually tested, and how

The build machine for this round had no root, no container runtime and no KVM,
so the distribution was bootstrapped and packaged inside an unprivileged user
namespace (`scripts/mkiso.sh`), and booted under QEMU in software emulation.

| # | Test | Result |
|---|---|---|
| 1 | `mkiso.sh all` builds the tree end to end | **pass** — no root, ~40 min |
| 2 | ISO structure: BIOS + UEFI El Torito, isohybrid MBR/GPT, volume id | **pass** (`xorriso -report_el_torito`) |
| 3 | ISO boots in QEMU (BIOS, 2 GB, 2 cores, software emulation) | **pass** — boot menu, kernel, initramfs, squashfs mounted |
| 4 | Live system reaches multi-user with our branding | **pass** — console shows `Refurbished Linux 1.0 (Focus) refurbished ttyS0` *(the 0.2 branding fix landed after this boot; see “known issues”)* |
| 5 | Cockpit socket activation | **pass** — boot prints `Web console: https://localhost:9090/` |
| 6 | Graphical desktop (labwc + refurb-shell) on the virtual display | **inconclusive** — the session unit's result could not be read back: the guest console login requires a password this environment does not have, and the guest screen never switched away from the boot loader |
| 7 | Installation to internal storage | **not tested** — the image includes no disk installer yet (see below) |
| 8 | Installed system booting | **not tested** (depends on 7) |
| 9 | Wi-Fi, Bluetooth, Brave, YouTube, extensions | **not tested** — needs real hardware and a network |
| 10 | Idle memory | **not measured** — `scripts/measure-memory.sh` needs a running session; see `docs/memory.md` for the budget it will be compared against |

### What the boot proves

The ISO is a working live system: firmware finds it, the kernel and initramfs
mount the squashfs, systemd reaches its default target, our `/etc/issue`
branding is printed, Cockpit's socket is listening, and a login prompt is
offered on the console. Everything that happens *before* the graphical session
is therefore verified on the built artifact.

### Why the graphical session is not claimed

`refurb-session.service` starts `labwc` on tty1 as uid 1000. In QEMU the guest
screen kept showing the boot loader frame, which means nothing ever drew on the
framebuffer — consistent with either (a) the unit failing to start, or (b) a
failing DRM/seat handover in the emulated VGA device. Telling those apart needs
the unit's journal, which needs a login on the guest console; Debian live sets
passwords this environment cannot supply. **This is the first thing to fix in
round 3**, and the honest statement today is: unknown, not working, unverified.

## C. Dark theme

Dark is now the default everywhere and costs no extra memory — it is the same
drawing code with a different palette (`shell/refurb-shell/src/theme.rs`):

| Surface | How it is dark |
|---|---|
| Desktop, bar, launcher, quick settings, Software, Settings, lock screen | shell palette (`#14161b` paper, `#1b1e24` panel, `#20242b` cards, `#35c2a3` accent) |
| File manager and other GTK apps | `/etc/gtk-3.0/settings.ini` + `GTK_THEME=Adwaita:dark` |
| Terminal | `/etc/xdg/foot/foot.ini` palette matched to the shell |
| Brave | `GTK_THEME` (drives Chromium's UI on Linux) + `BraveDarkMode: 2` policy |
| Cockpit | follows the browser/OS `prefers-color-scheme`; not forced |
| Welcome page | dark CSS |

## D. Brave extensions

Both requested extensions are configured as *default but removable* installs via
`/etc/brave/policies/managed/extensions.json`:

| Extension | ID | Mode |
|---|---|---|
| h264ify | `aleakchihdccplidncghkekgioiakgal` | `normal_installed`, pinned |
| ResoWarden | `bbifegkegpabeapkbngmieongpmppdin` | `normal_installed`, pinned |

`normal_installed` means Brave installs them on first launch and the user can
still disable or remove them — deliberately not `ExtensionInstallForcelist`,
which cannot be undone. They update through the normal Web Store mechanism.

**Not verified:** whether the IDs resolve, whether h264ify actually switches
YouTube to H.264, whether ResoWarden keeps the chosen quality or handles Shorts,
and their memory cost. All of that needs a running Brave with a network. Also
note the extensions are fetched from the Web Store on first launch, so a machine
that never gets online will not have them.

## E. Known issues carried into round 3

1. **No disk installer.** The ISO boots live but cannot install. `live-installer`
   / Debian's `d-i` components were not included in this build; the fastest
   correct path is a `live-installer` hook plus a first-boot “Install” entry, or
   Calamares on top.
2. **Graphical session unverified** (see above).
3. **Serial console login has no usable account** on the live system, which also
   blocks scripted post-boot testing. A dedicated debug account or
   `live-config.noroot`/password override would fix it.
4. **Branding on the tested ISO said 1.0**; the 0.2 os-release fix was applied
   after that boot and is included in the rebuilt image.
5. **Memory reset in the wrong place** — `measure-memory.sh` still needs a
   running session.
6. The rootless build depends on three environment workarounds documented in
   `docs/iso-build.md`; a normal Debian host with root does not need any of them.

## F. Rebuild it

    ./scripts/mkiso.sh all                  # no root needed
    # or, on a Debian/Ubuntu host with root:
    sudo apt-get install live-build debootstrap xorriso mtools squashfs-tools \
                         grub-pc-bin grub-efi-amd64-bin isolinux syslinux-common
    ./scripts/build-shell.sh && sudo ./scripts/build-iso.sh

Boot it without hardware:

    ./scripts/qemu-boot.sh                  # QEMU, software emulation
    python3 scripts/shot.py .build/shot.png # screenshot of the VM

## G. Screenshots

`shell/refurb-shell/preview/*.png` are rendered by the shipping drawing code and
show the dark interface: desktop, launcher, launcher search, quick settings,
Wi-Fi password entry, Software, Settings (updates and about), and the 32-glyph
icon set. They are the interface, not a mock-up, but they are not photographs of
the ISO running.
