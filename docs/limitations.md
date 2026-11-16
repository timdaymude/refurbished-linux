# Limitations and known gaps

This is the list you should read before believing a claim about Refurbished
Linux. It is ordered by how much it could hurt.

## Built and booted, but not finished

- **The ISO exists and boots** (`refurbished-linux-0.2.0-amd64.iso`, 773 MiB,
  BIOS + UEFI). It reaches a login prompt on the console with Cockpit listening.
- **No installer.** The image is live-only: there is no “install to disk” step
  yet, so requirement “install and reboot into the installed system” is not met.
- **The graphical session is unverified.** In QEMU the guest screen never left
  the boot loader frame, and the session unit's journal could not be read back
  because the live guest's console login requires a password. The Wayland client
  code has still never drawn a surface on a real compositor.
- **No hardware testing.** Every performance and hardware claim in these docs is
  a target or a budget, not a measurement.
- **Brave, YouTube, Microsoft 365 and Cockpit are untested** in this system.
- **The two Brave extensions are configured, not verified**: `normal_installed`
  + pinned in `/etc/brave/policies/managed/extensions.json`. Whether the Web
  Store IDs resolve and whether h264ify/ResoWarden behave needs a network and a
  real browser.
- **Dark theme coverage is complete for everything the shell draws**, plus GTK
  apps, the terminal and Brave's UI via `GTK_THEME`. Cockpit follows
  `prefers-color-scheme` and is not forced; some third-party GTK apps may still
  look light if they hard-code colours.

## Deliberate, and permanent unless someone changes their mind

- **One user, uid 1000, no display manager.** `refurb-session.service` logs in
  that specific account. Multiple accounts, a login screen and fast user
  switching are not supported. This is the cost of "turn it on and it is ready".
- **No screen blanking or auto-lock.** There is no idle daemon; the screen stays
  on until you lock it (`Super+L`) or suspend. Adding `swayidle` would cost a
  resident process; it was left out on purpose.
- **No notifications daemon.** Applications that post desktop notifications get
  no bubbles. The shell shows its own errors in its own windows.
- **No system tray.** Applications that only live in a tray (some chat clients,
  some VPN tools) will be awkward; their windows still work.
- **No printing.** CUPS is masked. Printing is not a core use case for this
  machine class, and `cupsd` plus `cups-browsed` is not free.
- **No Wine or Windows applications.** See `windows-migration.md`.
- **Hardware video decoding needs the right VA-API driver for the GPU
  generation.** `mesa-va-drivers` and `intel-media-va-driver` are installed; very
  old GPUs may still fall back to software decoding, and 1080p may stutter.
- **Suspend/resume depends on the firmware.** `logind` is configured to suspend
  on lid close, but old BIOSes and some Wi-Fi chipsets misbehave on resume. This
  is exactly what the hardware test is for.
- **Bluetooth audio needs a working PipeWire Bluetooth module.** It is installed;
  pairing is done through `bluetoothctl` inside the shell's quick settings, so
  unusual pairing flows (PIN entry on a keyboard, for example) may need the
  terminal.

## Rough edges you should expect

- **Keyboard handling** goes through `libxkbcommon` loaded at runtime (it is in
  the image). If it cannot be loaded, the shell falls back to a US layout, which
  makes non-US keyboards awkward. This fallback path is untested.
- **Launcher search** is substring matching over `Name` and `Comment` from
  `.desktop` files, with pins first. It does not know about applications that
  ship no desktop entry.
- **Software search** asks `apt-cache search` on a 320 ms debounce and shows the
  first results; obscure packages may need the terminal. There is no category
  browser, no screenshots and no ratings — those need an online catalogue, and
  an online catalogue needs a background service.
- **Updates are whole-system upgrades** through `apt-get upgrade`. There is no
  per-package selection in the GUI, and no automatic reboot handling beyond the
  hint that appears when a kernel update lands.
- **No Calamares installer.** The ISO uses Debian's installer in live mode: it is
  small and reliable, but it looks like Debian's installer, not like this
  desktop. A themed installer is the highest-value next piece of work.
- **`sudo` rule without a password** for the two package helpers. On a machine
  that boots straight into a session, a password prompt would be theatre; the
  helpers validate their arguments and cannot be used to run anything else.
- **Fonts**: Inter and DejaVu are installed. CJK and emoji fonts are not, so some
  web pages will show boxes for those glyphs unless Brave downloads its own
  fallback.
