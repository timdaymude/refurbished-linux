# Round 14 — Calamares and the Software Center removal

Status: implementation and QEMU verification in progress for r14.

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

    welcome → locale → keyboard → partition → users → summary
      ↓  (second confirmation)
    refurb-safety → partition → mount → unpackfs → machineid → fstab
      → locale → keyboard → localecfg → users → networkcfg → hwclock
      → packages → bootloader → refurb-finalize → removeuser → umount

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

It does **not** filter a writable live USB made by a third-party tool, and it
does not know about eMMC `boot0`/`boot1`/`rpmb` block devices. Reawakening adds:

1. **`refurb-installer` wrapper** — runs Calamares as root with the session's
   display variables, marks the live medium and every eMMC boot/RPMB device
   read-only for the duration, and restores the previous flags afterwards.
2. **`refurb-safety` Calamares job** — runs before the partition job and
   refuses the live disk, any partition on it, and eMMC boot areas even if the
   UI still offered them. It fails closed if the target is unknown.
3. **`removeuser` + finalize** — the live `refurb` account and its
   `/etc/sudoers.d/50-refurb-live` rule cannot survive into the installed
   system.

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

<!-- FILLED IN AFTER THE QEMU RUNS -->
