# Established components, 0.3.0-r7

The desktop stays Reawakening OS: the same bar, launcher, quick settings,
Wi-Fi dialog, sliders, settings window, lock screen and multi-monitor drawing,
on the same labwc compositor. What changed is underneath. Where a full
NetworkManager, BlueZ or PipeWire front end already exists and is maintained,
the shell now opens it instead of reimplementing it, and the parts that only
the shell can do stay in the shell.

Every claim below was checked on a booted image in QEMU (OVMF UEFI, 2 cores,
2 GB, software rendering); hardware-only results are marked **unverified**.

## What was reused, unchanged

| Function | Component | State |
| --- | --- | --- |
| Wi-Fi association | NetworkManager + wpa_supplicant | already shipped, untouched |
| Bluetooth stack | BlueZ 5.82 | already shipped, untouched |
| Audio server | PipeWire + WirePlumber | already shipped, untouched |
| Battery / power supply | UPower + logind | already shipped, untouched |
| Removable media | udisks2 | already shipped, untouched |
| Compositor, terminal, files | labwc, foot, PCManFM | already shipped, untouched |
| Brightness device | brightnessctl + sysfs | already shipped, untouched |
| Resolution, screenshots, OSD | wlr-randr, grim, wob | already shipped, untouched |

The shell keeps every control it had: the bar, the launcher, the quick
settings panel, the Wi-Fi password dialog (with the keyboard-focus fix from
r6), the volume and brightness sliders, the settings window, the lock screen
and the drawing on every output.

## What was added

Four programs, each an ordinary application that talks to the same daemons:

| Program | Covers | Cost |
| --- | --- | --- |
| `nm-connection-editor` | NetworkManager's own editor: scanning, WPA/WPA2/WPA3 security, saved networks, autoconnect, hidden networks | no daemon; only resident while its window is open |
| `blueman` | BlueZ manager: discovery, pairing agents, trusted devices, audio profiles | no daemon beyond BlueZ; only resident while its window is open |
| `pavucontrol` | PipeWire mixer: per-application streams, input devices, headset profiles | only resident while its window is open |
| `mako-notifier` | `org.freedesktop.Notifications` | ~8 MB, the one new resident process |

They are reached from the places the user already is:

* quick settings → Wi-Fi → **Wi-Fi settings**
* quick settings → Bluetooth → **Manage…**
* Settings → Sound → **Outputs, inputs and apps…**

Each is launched as a child process, so a crash in one of them cannot take the
desktop down with it. If a program is missing, the shell falls back
(`nmtui` in a terminal for Wi-Fi, `bluetoothctl` for Bluetooth) and says so in
the session log instead of doing nothing.

## What was rejected, and why

**iwgtk** was the first choice for Wi-Fi and is the reason this section exists.
Debian builds it against **iwd only**; on an image that runs NetworkManager it
opens with *"iwd is down"*. Worse, installing it pulled `iwd` itself, a second
daemon that manages the same radio as NetworkManager. Both were removed
(`apt purge iwgtk`, `autoremove` → `iwd`, `libell0`).

**xfce4-power-manager** and **Thunar** were not added. UPower and logind
already provide battery state and lid handling, and PCManFM is the file
manager the image ships and the desktop's files button opens. Adding them
would have meant a second background service and a second file manager for no
functional gain.

## Root causes found this round

1. **polkit covered only part of the desktop.** The r6 rule allowed
   `org.freedesktop.NetworkManager.*` and `org.bluez.*`. Blueman's own actions
   (`rfkill.setstate`, its network/DHCP helpers) and udisks2 mounting a disk
   are `auth_admin_keep`, and a session started from a system unit is not an
   *active* logind session, so they were refused outright - a Bluetooth power
   toggle in the manager or a USB stick in PCManFM would simply fail. The rule
   now also allows `org.blueman.*` and the udisks2 mount / unmount / eject /
   power-off actions for the `netdev` group. Formatting and partitioning are
   deliberately **not** allowed.
2. **The notification service started before the compositor.** `mako` is a
   Wayland client; starting it in `session-launch` before `exec labwc` made it
   exit at once with *"cannot connect to a Wayland display"*, so every failure
   message from those programs was dropped. It now waits for labwc's socket in
   the background and then takes over `org.freedesktop.Notifications`.
3. **`iwgtk` was the wrong package** (see above): an iwd-only front end on a
   NetworkManager image, plus a conflicting daemon.

## Measurements

Idle desktop, no application open, 2 GB / 2 cores, software rendering.

| Process | r6 (before) | r7 (after) |
| --- | --- | --- |
| `refurb-shell` | 45.0 MB | 45.2 MB |
| `pipewire` | 10.7 MB | 10.6 MB |
| `wireplumber` | 14.0 MB | 13.9 MB |
| `mako` | - | 7.9 MB |

The custom shell and the audio stack are unchanged within a megabyte. The only
new resident cost is the notification daemon, ~8 MB. `nm-connection-editor`,
`blueman-manager` and `pavucontrol` are not started at login: they appear only
when opened, and their GTK dependencies cost disk, not idle memory.

## Verification in this round

On the shipped r7 ISO, by driving the real desktop in QEMU (pointer and
keyboard events, framebuffer screenshots):

* boots to the desktop, bar drawn, no panic and no shell restart;
* `mako` is running as the session user (pid 8096, 8.1 MB RSS);
* the volume slider still works on this image: a click moved the sink from
  100% to 48% and `wpctl get-volume` agreed;
* `nm-connection-editor` starts under labwc and draws its **Network
  Connections** window (the a11y warning about `org.a11y.Bus` is expected -
  AT-SPI is not installed);
* `pavucontrol` starts under labwc and shows the PipeWire output device; on
  the previous build of the same shell code, the Settings → Sound button
  started it as a child process (the identical action path - `Action::OpenTool`
  - is what the r7 build uses);
* no `iwd` remains in the image (`iwd.service` and `/sbin/iwd` are gone).

**Not verified in this round:**

* a real WPA2 association with a real radio, and reconnection after reboot -
  the emulated test machine has no radio, so the editor was verified as a
  program, not as a connection;
* Bluetooth discovery, pairing and audio profiles - no adapter in QEMU;
* the quick-settings **Wi-Fi settings** button click itself: the quick panel's
  expand-on-click is unreliable under synthetic QEMU input (it took several
  attempts on r6 too), so the button was seen drawn but its click was not
  captured. The program it launches was verified running, and the same action
  path was verified with the Settings → Sound button;
* the `blueman-manager` window: the command started it, but the typed command
  went into another window's search field before a screenshot was taken.

One observation needs repeating on hardware: about a minute after setting the
volume to 48%, the panel showed 100% again. It was not established whether the
sink itself changed or only the display did. `refurb-diag` records
`wpctl get-volume` at capture time, so a real machine will settle it.

## Trying it on the laptop

The r6 checklist in `docs/hardware-0.6.md` still applies, with these additions:

1. Quick settings → Wi-Fi → **Wi-Fi settings** opens *Network Connections*.
   Use **+ → Wi-Fi** to scan, pick the network, enter the password; tick
   *Connect automatically* (it is on by default) for reconnection after
   reboot.
2. Quick settings → Bluetooth → **Manage…** opens Blueman: search, pair,
   trust, connect, and choose the audio profile of a headset.
3. Settings → Sound → **Outputs, inputs and apps…** opens Volume Control.
4. Deliberately fail something (wrong Wi-Fi password) and confirm a
   notification appears - that is `mako` doing its job.
