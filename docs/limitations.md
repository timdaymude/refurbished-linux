# Limitations and known gaps

This is the list you should read before believing a claim about Reawakening
OS. It is ordered by how much it could hurt.

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

## Round 11 additions

- **The power key itself is a hardware test.** QEMU delivers no `KEY_POWER` to
  the guest (the ACPI power button is not wired to the emulated keyboard), so
  what was verified is the two halves separately: `systemd-inhibit --list` shows
  the session holding exactly one `handle-power-key` lock, and
  `refurb-shell --send power` - the command labwc's `XF86PowerOff` binding runs -
  raises the dialog. That the physical key reaches labwc once logind is inhibited
  can only be seen on the Chromebook.
- **Server-side decorations for Brave are not in this build.** The window rule
  was removed while chasing a crash loop that turned out to be QEMU's `-cpu max`
  (see `docs/round-11.md` section 14), and it was not re-tested afterwards.
  Brave therefore keeps its own titlebar; maximize is `Alt+F10`, `Super+F`,
  `Super+Up` or a double-click on its header, and `F11` is full-screen.
- **The quick panel swallows the first click outside itself.** That is what a
  popup does, and it is the price of the panel being able to see that click at
  all. The panel also holds the keyboard while it is open, so typing goes to it
  until it is dismissed.
- **Jellyfin playback against a real server is untested.** What was verified is
  that Jellyfin Media Player 1.13 starts, draws its server-address screen and
  gets a taskbar entry. It is also the heaviest component in the image: about
  426 MB resident plus three Qt WebEngine children, which pushed a 2 GB emulated
  machine into swap with a browser open.
- **Cockpit's login from another computer is untested**, because the live
  account has no password: `refurb-cockpit status` says so, and `refurb-cockpit
  password` is the fix. Verified instead: the socket is active, serves
  `https://127.0.0.1:9090` locally, and the host-forwarded port is refused, which
  is the loopback binding doing its job.
- **The 160 % audio ceiling and the stuck cursor** were not reproduced. r11
  installs the diagnostics that will show what happens on the machine
  (`refurb-diag` collects the sink, the hardware mixer, `vainfo` and `/dev/dri`)
  and names the cursor theme, but neither is a proven fix.
- **The Chromebook top row is still a hardware test.** The hwdb entry matches
  only Google's internal atkbd, and `refurb-keys` reports what a key press
  produced, so the first session with the machine will say whether the mapping
  fires.
- **`iproute2` is not installed**, so `refurb-diag` cannot print `ss` output; use
  `lsof`/`/proc/net/tcp` or install it, if that is ever needed.

## Round 12 additions

- **Desktop icons belong to the primary screen.** The wallpaper is painted on
  every monitor, but files are shown only on the first one: a second monitor
  would otherwise present a second, independent arrangement of the same files.
- **Deleting from the desktop does not ask.** It moves the item to the
  freedesktop trash (restorable in PCManFM), but there is no confirmation
  dialog. Rename is committed with Enter and cancelled with Escape.
- **A desktop shortcut is only run when it is trusted**: owned by the user, not
  writable by group or others, executable, and not a symlink. A `.desktop` file
  copied by hand from `/usr/share/applications` works; one dropped by a browser
  normally will not, and the desktop says why instead of running it.
- **Shortcuts can only be added for installed applications listed in the pinned
  set.** Dragging a `.desktop` file into `~/Desktop` with PCManFM works too; a
  full application picker is not part of this round.
- **The icon grid is five rows of fixed cells.** Positions snap to the grid
  rather than free pixel placement, which is what makes them survive a change of
  resolution. A very small screen shows fewer columns, not smaller icons.
- **Alt+Z dictation is English-only and needs the 68 MB Vosk model**, which is
  downloaded while the image is built. It runs offline, but the first Alt+Z of a
  session loads the model (a second or two; emulated QEMU is slower).
- **Dictation has no punctuation or capitalisation**, because the small Vosk
  model does not produce it. Text is typed as each phrase completes and at the
  end, into whichever window has the focus at that moment; changing focus
  mid-dictation sends the text to the new window.
- **Dictation accuracy on the Chromebook's microphone is unverified.** The
  engine, model and typing path are tested; microphone response, gain and
  accuracy are hardware tests.
- **The Jellyfin audio-delay fix is unverified on the real codec.** The
  WirePlumber rule is verified to load and to leave `node.suspend-on-idle =
  false` on the analog sink, but the emulated QEMU codec still reports the node
  as `suspended` (QEMU has no working host audio backend here), so neither the
  keep-awake behaviour nor "sound starts with the picture" could be observed.
  Both need the Chromebook's codec and speakers.
- **Keeping the analog sink awake costs idle power.** That is the trade for the
  audio fix; HDMI and USB outputs still suspend when idle.
- **No SMD Linux source was available**, so r12's dictation is a new
  implementation rather than a port. See `docs/round-12.md` section 3.
- **The desktop rescans `~/Desktop` every two seconds at most**, comparing a
  signature rather than using inotify. A file written and deleted within one
  interval is not noticed; ordinary use is unaffected.
- **A click on the wallpaper can be lost after a window closes under the
  pointer.** The shell learns which surface the pointer is on from the
  compositor's enter/leave events; in QEMU a right-click on the empty desktop was
  ignored until the pointer crossed the bar and came back. Moving the pointer
  restores it, and clicks on icons or windows are unaffected.
- **A microphone that produces no audio for six seconds is treated as a
  failure.** Dictation then says "no microphone is available" and exits instead
  of showing "Listening" forever. On hardware with a working microphone the
  stream always carries data, so this only surfaces for a missing, muted or
  permission-blocked device.
