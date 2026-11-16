# Installer design (0.4 target)

Status: **design only. No disk operations are implemented yet.** Nothing in
this document has been run against a real disk; the implementation plan below
ends with a virtual-machine test before any hardware is touched.

## Why not Calamares

Calamares is a Qt application with its own module system, branding engine and
~40 dependencies; it would add roughly 150 MB of packages and a Qt stack to an
image whose idle desktop currently sits at 190 MB PSS. That contradicts the
whole point of Refurbished Linux.

**Decision: a small native installer, `refurb-install`, written in Rust with
the same drawing code `refurb-shell` already uses** (`tiny-skia` + `fontdue`,
no toolkit). It is one binary, reuses the shell's icons and palette, and adds
no daemon: it runs as a normal window from the launcher, only while the user is
installing.

Fallback if that proves impractical in one round: a scripted
`refurb-install --auto` in a terminal plus `live-installer`'s shell UI. Never
Calamares.

## Target layout

UEFI (default, recommended):

| Partition | Size | Type | Contents |
|---|---|---|---|
| 1 | 512 MB | EFI System (`ef00`) | FAT32, `/boot/efi`, GRUB + kernel |
| 2 | remaining | Linux (`8300`) | ext4, `/` (no separate `/home`) |

Legacy BIOS: single ext4 `/` partition with GRUB in the MBR. No swap partition
— zram provides compressed swap in RAM, as on the live image.

Whole-disk GPT is used in both cases. **No LVM, no LUKS in 0.4** (LUKS is a
death sentence for a 2 GB / dual-core machine's interactivity, and it doubles
install complexity). Both can come later as an explicit opt-in.

## Install sequence

1. **Welcome** — what this does, that the machine will restart, and that it is
   a live session until then.
2. **Language and keyboard** — locale and keymap for the installed system.
3. **Disk selection** — list `/sys/block/*` devices with model, size, bus and
   whether they are removable; refuse USB sticks and the live medium itself.
4. **Erase warning** — the chosen disk's model, size and existing partitions
   (including Windows partitions found by filesystem type) listed back to the
   user. A checkbox plus typing the word `erase` is required before the button
   becomes active. **The installer never formats anything without this step.**
5. **Account** — full name, username, password (with strength hint), hostname.
6. **Summary** — partitions to be created, packages, bootloader target.
7. **Install** — progress with the current step, cancellable *before*
   partitioning only. Steps: partition → format → unsquash the live filesystem
   → chroot: fstab, locale, keymap, user, hostname, zram, initramfs → GRUB.
8. **Finish** — "Restart into Refurbished Linux", plus a log file the user can
   read if anything failed.

## Implementation notes

- Partitioning via `sfdisk` from a generated script (no `parted`, no Python).
- Formatting via `mkfs.vfat` / `mkfs.ext4`.
- Filesystem copy: `unsquashfs` the live `filesystem.squashfs` straight onto
  the new root — the same artifact already verified bootable.
- The chroot stage reuses the existing hooks: the live account `refurb` is
  **removed** and the user's own account created in its place; `session-start`
  already resolves whichever uid ≥ 1000 exists, so no code changes are needed
  there.
- GRUB: `grub-install --target=x86_64-efi --efi-directory=/boot/efi` or
  `--target=i386-pc /dev/sdX`, then `update-grub` with
  `GRUB_DISABLE_OS_PROBER=false` so an existing Windows install is offered.
- Brave and its two extension policies come from the squashfs unchanged.
- Nothing is added to the installed system: no installer service, no daemon,
  no telemetry. `refurb-install` ships in the live image only and is removed
  from the installed system by the chroot step.

## Safety rules the code must enforce

- Refuse to run unless booted from the live medium (`/run/live` exists).
- Refuse block devices that are removable, or that back `/` or `/run/live`.
- Never start partitioning before the typed confirmation.
- Write the partition table only after formatting tools are verified present.
- On any error after partitioning: stop, leave the disk as-is, print the log
  path; never attempt a second destructive pass automatically.

## Test plan (in this order)

1. QEMU, blank 8 GB virtio disk, UEFI: full install, then boot the installed
   system and confirm the dark desktop and Brave.
2. QEMU, blank disk, BIOS: same.
3. QEMU, disk with a fake Windows ESP + NTFS partition: confirm the warning
   names them and that declining leaves the disk untouched (`sfdisk -d` diff).
4. Installed-system memory measurement: idle PSS, MemAvailable, zram, then
   Brave with YouTube when network is available.
5. Physical hardware last.
