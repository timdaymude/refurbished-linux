# Round 14 — Calamares and the Software Center removal

Status: implemented and verified in QEMU (BIOS, UEFI, external USB, cancellation,
installed-system use) for r14. Physical-hardware installation is still an open
test — see section 5.7.

## 1. Why Calamares, and what it changes

r13 shipped `refurb-install`, a terminal program that asked for a device path
and a typed `erase /dev/sdX`. It was safe, but it was not an installer an
ordinary Windows user could complete. r14 replaces the user-facing installer
with **Calamares 3.3.14**, the Debian trixie package.

The r13 backend is not thrown away blindly: its tested rules are carried into
Calamares' own modules and a post-install script (live account removal,
passwordless sudo removal, Cockpit activation, initramfs rebuild). The terminal
program remains in the image as a rescue path only; the desktop entry and the
documented path now launch Calamares.

### Evaluated alternatives

| Option | Why it was not used |
|---|---|
| **Calamares** | Debian's packaged installer (3.3.14, 4.0 MiB package, 11 MiB installed; Qt6/KF6/KPMcore dependencies). Chosen: familiar wizard, maintained partition/account/bootloader modules. |
| Build a custom GTK wizard around `refurb-install` | Explicitly rejected by the r14 brief. It would also leave the distribution maintaining its own partition and bootloader UI. |
| Anaconda / YaST / Ubuntu's Flutter installer | Distribution-specific stacks; not available as a Debian live-installer integration and far larger than the Calamares dependency set. |
| Keep the terminal installer | Fails the usability requirement. |

The extra dependency weight is live-media only. `packages.conf` removes
`calamares` from the installed system; the desktop, audio, Wi-Fi and Cockpit
stacks are unchanged.

## 2. Configured Calamares sequence

    show:  welcome → locale → keyboard → partition → users → summary
      ↓  (prompt-install confirmation; Cancel still works until the first write)
    exec:  partition → mount → unpackfs → machineid → fstab → locale
           → keyboard → localecfg → users → networkcfg → hwclock
           → packages → bootloader → refurb-finalize → removeuser → umount
      ↓
    show:  finished

- **Partition:** explicit choice is required (`initialPartitioningChoice:
  none`); manual partitioning, LVM and LUKS are hidden; ext4 root, 512 MiB ESP,
  no swap partition (zram is the swap).
- **Account:** a sudo administrator with the desktop's audio/video/input/render
  and netdev groups; root password is optional on the same page.
- **Summary:** Calamares lists the exact disk, the partitions it will delete or
  format, and requires the `prompt-install` confirmation.
- **Filesystem source:** `/run/live/medium/live/filesystem.squashfs`, the same
  live filesystem the r13 installer unpacked.
- **Bootloader:** Calamares' GRUB module, with `installEFIFallback: true`. On
  UEFI the removable `/EFI/BOOT/BOOTX64.EFI` is written in addition to the
  normal entry, which is what makes a USB installation moveable.

## 3. Disk safety beyond Calamares' defaults

Calamares itself:

- refuses a disk that contains the running `/`;
- refuses an iso9660 disk (the normal dd-written live USB);
- only partitions the disk selected on its own partition page;
- requires the partition choice and the install confirmation.

It does **not** filter a writable live USB made by a third-party tool, it does
not know about eMMC `boot0`/`boot1`/`rpmb` block devices, and it preselects the
first disk it enumerates in the device list. Reawakening adds:

1. **`refurb-installer` wrapper** — runs Calamares as root with the session's
   display variables, marks the live medium and every eMMC boot/RPMB device
   read-only for the duration, and restores the previous flags afterwards. A
   device that cannot be made read-only stays in the list, so the summary and
   the confirmation remain the last gate before anything is written.
2. **No preselected destructive action** — `initialPartitioningChoice: none`, so
   "Erase disk" starts unchecked and Next stays disabled until the user chooses
   it.
3. **`prompt-install: true`** — Calamares asks once more at the point of no
   return, and Cancel keeps working until the first write (verified).
4. **`removeuser` + finalize** — the live `refurb` account and its
   `/etc/sudoers.d/50-refurb-live` rule cannot survive into the installed
   system (verified).

A pre-partition `refurb-safety` job was written first and then removed: Calamares
does not publish the selected device into global storage until the partition
module builds its jobs, so a job in front of it cannot know the target. The
read-only marking in the wrapper is what covers that gap.

## 4. Software Center removal

The r11–r13 "Software" window was drawn by `refurb-shell` and used the
`refurb-apt` root helper. It is removed completely:

- the desktop entry and its taskbar pin;
- the window and package-search code (`pkg.rs`, `ui/software.rs`);
- the quick-settings Software button;
- the `Super+S` keybinding and `--send software` command;
- the `refurb-apt` executable and its passwordless sudo rule;
- launcher wording that pointed at it.

Settings → Updates and the `refurb-update` helper remain: they are the normal
operating-system update path, not an application store. APT itself is
untouched.

## 5. Verification

Everything below was run from the shipped ISO
(`reawakening-os-0.3.0-r14-amd64.iso`, 1,205,600,256 bytes, SHA256
`37abc90b59923e42da88a0030be3a627b5b892f89b6358cfdca234b3ea905bb6`) in QEMU
under pure software emulation (no KVM), driven through a VNC console and the
QEMU monitor's `sendkey`.

| Test | Result |
|---|---|
| BIOS install to a blank 10 GiB disk | Passed |
| UEFI install to a blank 10 GiB disk (OVMF) | Passed |
| Boot of the UEFI disk with an empty firmware variable store | Passed |
| Install to an emulated USB disk with a bystander internal disk attached | Passed |
| Cancel on the partition page | Passed — nothing written |
| Installed system: login, sudo, groups, APT, persistence | Passed |
| Software Center absent from the installed system | Passed |

### 5.1 Wizard

Welcome → Location → Keyboard → Partitions → Users → Summary → Install →
progress → Finish, with the Reawakening branding (dark stylesheet, brand mark,
"Welcome to Reawakening OS"). Next stays disabled on the partition page until a
partitioning choice is made, and Install stays disabled until the summary
validates.

With two disks attached the storage-device combo defaulted to the first
enumerated device (`/dev/sda`, the USB disk) — Calamares' own behaviour, noted in
`docs/limitations.md`. "Erase disk" was still unchecked and Next stayed
disabled, so the destructive action remained opt-in, and the summary and the
confirmation dialog both named the disk before anything was written.

### 5.2 BIOS install

Blank `/dev/vda`, 10 GiB. Summary: "Erase disk `/dev/vda` and install
Reawakening 0.3.0", "Creating new MSDOS partition table on `/dev/vda`", "Create
new 10239MiB partition on `/dev/vda` with file system ext4", "Set flags on
10239MiB ext4 partition to boot", "Install boot loader on `/dev/vda`". The
dialog "Continue with Installation?" → Install Now, progress 0 → 100% (mount,
unpackfs, packages, bootloader, initramfs), ending in "All done. Reawakening OS
0.3.0 has been installed on your computer."

The disk then booted with no ISO attached. Logging in as the created account
with its password worked; `id` showed sudo, audio, video, plugdev, netdev,
bluetooth, render and input; `sudo` with that password returned uid 0; `id
refurb` reported no such user; `/etc/sudoers.d` held only `10-installer`,
`README` and `refurb-helpers` (the live `50-refurb-live` rule was gone);
`cockpit.socket` was enabled; `apt-get update` succeeded; a file written in the
home directory survived a cold reboot; and `~/.config/refurb` had been copied
from the live session.

### 5.3 UEFI install

Blank 10 GiB disk under OVMF. The summary listed a GPT table, a 512 MiB FAT32 ESP
labelled REAWAKENING at `/boot/efi` and a 9.50 GiB ext4 root. The install ended
with "All done.", and the disk booted with a fresh variable store — that is,
through the removable `/EFI/BOOT/BOOTX64.EFI` fallback alone — into the desktop.

### 5.4 External USB install

The live medium with a bystander internal disk as `/dev/vda`, plus a blank 10 GiB
image attached as a USB mass-storage device (xHCI) and enumerated as `/dev/sda`.
Calamares listed both devices; `/dev/sda` was selected and "Erase disk" chosen
explicitly. The summary named only `/dev/sda` (MSDOS table, 10239 MiB ext4, boot
flag, "Install boot loader on /dev/sda") and the install ended with "All done.".

The bystander disk hashed to `619d31cfbba60883ba7d8d8ce302f3c95045d031a396b0b37a1edc23e84624fa`
both before and after the install: byte-identical, so the internal disk was not
touched.

The USB image was then booted **alone**, still attached as a USB device and with
no ISO: it reached the desktop and the welcome page. On that system `lsblk`
showed `sda`/`sda1` as the only disk, mounted at `/` (9.8 GiB, 3.0 GiB used); a
console login as the created account with its password worked; `sudo` asked for
that password and returned root; `apt-get update` succeeded; a marker file in
`$HOME` survived a cold boot; and `/etc/sudoers.d` was again limited to
`10-installer`, `README` and `refurb-helpers`.

### 5.5 Cancellation

On the partition page with "Erase disk" selected, Cancel → "CANCEL
INSTALLATION? … Yes" quit the installer. Afterwards `sfdisk -l /dev/vda` showed
no partitions and `blkid /dev/vda` printed nothing: nothing had been written.

### 5.6 Software Center

The installed system has no `software` desktop entry, no
`refurb-software.desktop`, no `refurb-apt`, and no Software chip or `Super+S`
binding; the launcher lists Brave, Files, Settings, Cockpit and Terminal. APT,
dpkg and PackageKit are untouched and `apt-get update` works. The same was
checked inside the built squashfs before the ISO was booted.

### 5.7 Not verified

- **Physical hardware.** Every test above used virtual disks. Real firmware
  partitioning, real USB enumeration, an internal disk that already boots
  Windows, and the eMMC boot areas are Chromebook tests.
- **The progress slideshow renders white.** Its QML panel draws the default
  white background instead of the dark branding; the progress bar, percentage
  and status text are correct. Cosmetic, does not affect the install, and
  deliberately not "fixed" by rebuilding an already tested ISO.
- **Speed.** The unpack step ran under software emulation; the timings say
  nothing about a real machine.
- **Dual boot, LUKS and LVM** are not offered (hidden in the configuration), so
  they are not merely untested but absent.
