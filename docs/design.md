# Design: the Focus desktop

## The decision

Three concepts were on the table.

**Concept A — browser-first, existing pieces.** A minimal panel, a launcher,
Brave in the middle, no desktop icons. Cheap to build. Rejected because it is
exactly what every lightweight distribution already does: a panel and a menu on
top of a conventional stack. Nothing about it would feel new, and the panel
would still drag in GTK or Qt.

**Concept B — a custom shell on small components.** `labwc` for compositing and
window management, one custom program for everything the user sees. This is what
was built.

**Concept C — almost browser-only.** Brave full screen, with a hidden strip for
files, settings and power. Excellent for a kiosk or a school lab; hostile for
someone who also wants their photos, a USB stick and a terminal. Rejected as the
default, but preserved as a *behaviour*: the desktop has no icons and no
widgets, so the browser is the natural centre of the screen.

The result is B with C's temperament: a quiet desktop, a browser as the obvious
first move, and everything else one keystroke or one click away.

## What the shell draws itself

`refurb-shell` is a single Rust binary. It owns:

- the 34 px top bar: identity mark, clock, network, Bluetooth, volume, battery,
  quick-settings button;
- the launcher: search over installed `.desktop` entries, pinned favourites,
  category rows, keyboard-first, no animation;
- quick settings: Wi-Fi list with inline password entry, Bluetooth device list,
  volume, brightness, lock and power;
- the Software window: search-as-you-type over the Debian catalogue, installed
  list, install/remove with a live log;
- the Settings window: about, display, sound, power, updates, keyboard;
- the lock screen (through `swaylock`, whose colours and layout are configured
  to match).

Everything is painted with `tiny-skia` into a CPU bitmap and handed to the
compositor through `wl_shm`, and text is rasterised by `fontdue` from bundled
Inter TTFs. There is no toolkit underneath: no theme engine to load, no icon
theme to parse, no GTK CSS provider, no font configuration daemon dependency
beyond what the session already needs.

Consequences that matter on 2 GB:

- The shell's baseline cost is one process, a few buffers and a glyph cache.
- Drawing is CPU work, which is fine: a bar repaint is a few hundred
  microseconds, and low-end integrated GPUs are often *slower* than the CPU at
  compositing small damage regions.
- Nothing needs to be recompiled per theme; the palette is 12 constants in
  `theme.rs`.

## Interaction model for a Windows user

| Windows | Refurbished Linux |
|---|---|
| Start menu | `Super`, or click the mark on the left of the bar |
| Search | type immediately after `Super` |
| Alt+Tab | `Alt+Tab` (labwc) |
| Taskbar | the window strip on the right of the bar (foreign-toplevel protocol) |
| Win+L | `Super+L` |
| Win+E | `Super+E` (file manager) |
| Task manager → Services | Cockpit |
| Settings | `Super+,` or the gear in the bar |
| Microsoft Store | Software (`Super+S`) |

The bar is deliberately at the top, the clock is on the right, the power menu is
behind the gear on the right. Nothing important is hidden behind a right-click.

## Visual identity

Warm paper background (`#f4f1e9`), deep green accent (`#0e7a69`), dark ink
(`#1b1a18`), 1 px hairline borders, 8–12 px radii, generous spacing, no shadows
and no transparency. Type is Inter at three weights — the same family the
interface ships, so there is no font fallback lottery on a fresh install. Icons
are drawn as 24-unit vector paths in `icons.rs`: one stroke width, one grid,
consistent corners. The mark is a single rounded square with an “R” cut from it.

No wallpaper image: the background is a two-stop gradient computed at the
output's size. That is 0 bytes of disk and no daemon.

## Why not GTK, Qt, or an existing panel?

Each of them costs more than it gives here: a toolkit pulls a theme engine, an
icon theme, a settings daemon and a CSS parser into the session, and none of
those make a 34 px bar faster. A hand-drawn bar is a few thousand lines and it
starts in milliseconds.

## Deliberate omissions

No notifications daemon (the shell reports failures in its own surfaces), no
tray, no clipboard manager, no screen blanking daemon, no global menu, no
window animations, no scale effects, no compositor blur. Each one is a process
or a per-frame cost, and none is needed to browse the web or plug in a USB
stick.

## Session model

There is no display manager. `refurb-session.service` logs uid 1000 in on tty1
with `PAMName=login` (so `logind` grants the DRM and input devices), sets the
environment, opens the first-boot page once, and `exec`s `labwc`. Labwc's
`autostart` starts `refurb-shell`. Locking is explicit (`swaylock`); the machine
does not silently lock itself in the middle of a download.

Keybindings live in `distro/config/includes.chroot/etc/xdg/labwc/rc.xml` and
call `refurb-shell --send <command>`, which writes one line into
`$XDG_RUNTIME_DIR/refurb-shell.sock` (0600, same user only). There is no D-Bus
service to activate and no IPC library to load.
