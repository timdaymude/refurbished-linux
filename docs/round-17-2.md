# Round 17.2 — the USB boot fix

On Craig's Acer Chromebook the r17.1 boot menu appeared, and selecting Linux
printed:

```
error: file '/live/vmlinuz' not found.
error: you need to load the kernel first.
```

Both messages come from the UEFI boot path, not from the kernel, the desktop or
the medium: GRUB was asking for the live files on a device that does not have
them. Fixing that exposed a second fault behind it — once the medium was found,
the kernel still did not receive its initrd. Both are fixed in this image; no
desktop, installer or application code changed.

| | |
|---|---|
| Image | `reawakening-os-0.3.0-r17.2-amd64.iso` |
| Size | 1,272,578,048 bytes |
| SHA256 | `44f8bdf7e5f90349f5ff47a55c1a0b8c8e72243a408e65ae27bc0694c803159e` |
| Rollback | r17.1 and r16 stay published |
| Branch | `refurb/0.3` |
| Release | <https://github.com/timdaymude/refurbished-linux/releases/tag/v0.3.0-r17.2> |

## 1. What GRUB was reading

`grub-mkstandalone` builds a single EFI program whose own config and theme live
in a RAM disk, and GRUB starts with `$root` pointing at that RAM disk
(`(memdisk)`). The r17.1 config tried once to find the medium — one `search
--no-floppy --set=root --file /live/vmlinuz` — and then booted regardless of
whether it had worked. When that search returns nothing, the menu entry runs
`linux /live/vmlinuz` against `(memdisk)`, which is exactly the pair of errors
above.

The search can return nothing even when the stick is perfectly readable:

- `--no-floppy` makes GRUB skip any device its firmware classifies as a floppy.
  Some UEFI firmware presents a USB stick that way.
- A stick written by an imaging tool can have a partition table GRUB cannot
  parse (or none GRUB recognises), so `search` never reaches the ISO volume even
  though the volume itself reads fine by name.
- The firmware may hand GRUB the ISO's small El Torito EFI partition rather
  than the volume that holds `/live`.

The same fault was reproduced locally: booting r17.1 as a plain disk under OVMF,
GRUB's `ls` listed only `(memdisk) (ahci2)` — the medium by name, but no
partition and nothing `search` would return.

## 2. The fix

The menu now looks for the medium in four independent ways, and only accepts a
device after checking that `/live/vmlinuz` is really on it:

1. `search --file /live/vmlinuz`, **without** `--no-floppy`, so floppy-class USB
   devices are not skipped.
2. `search --file /live/filesystem.squashfs` (the largest, most distinctive
   file).
3. `search --label REAWAKENING_OS` (the volume label the build writes).
4. Walking the names UEFI firmware gives block devices — `(hd0)`…`(hd5)`,
   `(cd0)`…`(cd2)`, `(usb0)`…`(usb2)` and their `gpt`/`msdos` partition
   suffixes — and testing `/live/vmlinuz` on each. This is what saves a stick
   whose partition table did not survive the write.
5. Finally, if a writing tool put the ISO on the stick as a *file*, loopback-mount
   `reawakening-os-0.3.0-r17.2-amd64.iso` from the common device names and use
   the volume inside it.

`$root` is set from the device that answered — never inherited from `(memdisk)`.
If no device holds the live files, the entry prints a diagnostic instead of a
bare GRUB error: the list of devices GRUB can see, and what to check (write the
image in dd/raw mode, write it completely, keep the ISO's file name if it was
copied as a file).

The menu itself is untouched: same theme, artwork, wordmark, three entries,
selection highlight, five-second countdown, and the same plain-text fallback if
the graphics mode cannot be set.

## 3. The second fault: the initrd never arrived

With the medium found, the boot got further and failed again:

```
EFI stub: ERROR: Failed to load initrd: 0x8000000000000001
EFI stub: ERROR: efi_stub_entry() failed!
error: start_image() returned 0x8000000000000001
```

The cause was in how the EFI image was built. It carried GRUB's own BIOS-class
storage drivers (`ahci ata pata scsi usb usbms uhci ohci ehci`). Under UEFI those
drivers take the medium away from the firmware's block I/O: GRUB reads the
kernel through its own PCI/USB driver, which works, but the initrd is not read at
that point — it is read lazily, from inside the kernel's
`EFI_LOAD_FILE2_PROTOCOL` callback, after Linux has taken over the controller.
That read fails with a device error, so the kernel never gets its initrd.

The EFI image now contains none of those drivers. GRUB reads the medium through
firmware block I/O (the same approach Debian's own UEFI image uses), which stays
valid across the handover.

Measured as an A/B with the same GRUB, kernel and initrd: with the BIOS storage
modules the kernel fails at `start_image()` with `0x8000000000000001`; without
them the same image boots to userspace.

## Verified

- The ISO contains `/live/vmlinuz`, `/live/initrd.img`,
  `/live/filesystem.squashfs` and `boot/grub/efi.img`.
- **UEFI optical boot of this image**: branded menu, Linux starts, live desktop
  with the Welcome window (`screenshots/r17-2-optical-desktop.png`).
- **UEFI USB boot of this image** (emulated EHCI stick under OVMF — the closest
  local analogue to Craig's USB stick): menu, kernel 6.12.118 loaded, initrd
  accepted, automatic login on tty1, Cockpit listening
  (`screenshots/r17-2-usb-boot.png`).
- `scripts/check-config.sh`: all checks passed, including a new r17.2 group that
  fails the build check if a BIOS storage driver returns to the EFI image or if
  `--no-floppy` returns to the search.
- `cargo test` was not re-run: no shell code changed. The desktop, installer,
  applications, wallpaper, clock and monitor work are r17.1's, unchanged.

## What this does not prove

- **Craig's firmware.** Everything here is OVMF and an emulated stick. The fix
  covers the known ways firmware can hide a USB volume from GRUB, but only the
  Chromebook can confirm it. A second failure there would be a different fault.
- **The ISO-file path.** Loopback-mounting an ISO stored as a file on the stick
  is implemented and syntactically exercised, but no stick was written in
  ISO-image mode to test it end to end.
- **BIOS.** The syslinux path did not change in this round and was not re-run;
  it was verified for r17.1.
- **Everything else.** No desktop, installer, store or hardware test was repeated,
  because no code behind them changed.
