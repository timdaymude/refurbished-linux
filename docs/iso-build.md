# Building the ISO

Two ways to build. Both produce the same image; the second needs no root at all.

## On a normal Debian/Ubuntu machine (recommended, fastest)

    sudo apt-get install live-build debootstrap xorriso mtools squashfs-tools \
                         grub-pc-bin grub-efi-amd64-bin isolinux syslinux-common \
                         dosfstools zstd
    git clone <this repository> && cd refurbished-linux
    ./scripts/build-shell.sh          # release build + .deb for the image
    sudo ./scripts/build-iso.sh       # -> distro/live-image-amd64.hybrid.iso

## With no root (what this workspace uses)

`scripts/mkiso.sh` builds the same image without root:

    ./scripts/mkiso.sh toolbox     # fetch the toolchain (.toolbox/)
    ./scripts/mkiso.sh bootstrap   # debootstrap trixie, extract phase
    ./scripts/mkiso.sh bootstrap2  # debootstrap second stage
    ./scripts/mkiso.sh packages    # install the package lists + Brave   (long)
    ./scripts/mkiso.sh configure   # shell package, shipped files, hooks
    ./scripts/mkiso.sh squashfs    # filesystem.squashfs
    ./scripts/mkiso.sh iso         # bootable hybrid ISO

or simply `./scripts/mkiso.sh all`.

Privileges come from `unshare -r`: the build user becomes uid 0 in its own
user namespace, so `chroot`, `debootstrap` and `dpkg` work while the host stays
untouched. Nothing is installed on the host itself.

### Three things a rootless build has to work around

1. **Only one uid can be mapped.** `chown file:shadow` therefore fails with
   EINVAL and takes maintainer scripts down with it. The build sets
   `TAR_OPTIONS=--no-same-owner`, runs dpkg with `--force-not-root`, and
   preloads `scripts/chown-shim.rs` (a 40-line cdylib that makes `chown`
   succeed). Real ownership is normalised afterwards by
   `mksquashfs -all-root`, which is exactly what the live system needs.
2. **apt's privilege drop cannot work.** `apt-get -o APT::Sandbox::User=root`
   keeps the download in the same namespace.
3. **HTTPS needs a trust store inside the chroot**, so `ca-certificates` is
   installed before Brave's archive is read.

## Requirements

| Item | Why |
|---|---|
| ~6 GB free disk | root filesystem, squashfs and the ISO |
| ~2 GB RAM | `mksquashfs` and `xorriso` |
| Debian 13 or Ubuntu 24.04 host, amd64 | the bootstrap targets trixie/amd64 |
| Network access | ~700 MB of Debian packages, ~120 MB of Brave, ~40 MB toolchain |
| External downloads | `deb.debian.org` (trixie), `archive.ubuntu.com` (build tools when rootless), `brave-browser-apt-release.s3.brave.com` |

No credentials or accounts are needed anywhere.

## What ends up on the ISO

    /isolinux/isolinux.bin          BIOS boot loader (El Torito, no emulation)
    /boot/grub/efi.img              UEFI boot loader (grub, standalone image)
    /live/vmlinuz                   kernel 6.12 (Debian trixie)
    /live/initrd.img                live-boot initramfs
    /live/filesystem.squashfs       the whole system, zstd, all files owned by root

The image is isohybrid with a GPT, so the same file boots from a CD/DVD, from a
USB stick written with `dd`, and on UEFI machines (`boot/grub/efi.img`), plus
legacy BIOS machines (`isolinux.bin`).

Boot entries: *Start Refurbished Linux* (quiet splash), and a verbose variant.
Both append `console=ttyS0,115200`, which is harmless on machines without a
serial port and makes a broken boot easy to diagnose on the ones that have one.

## Booting it without hardware

`scripts/qemu-boot.sh` runs the ISO under QEMU (software emulation, because
KVM is not available in this workspace), with the serial port on a socket:

    ./scripts/qemu-boot.sh
    python3 scripts/serial-tee.py .build/serial-full.log 400 &
    python3 scripts/shot.py .build/shot.png          # screendump -> PNG

With software emulation a boot to the login prompt takes roughly three minutes
on two emulated cores.
