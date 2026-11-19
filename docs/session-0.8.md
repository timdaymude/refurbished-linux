# A real login session, 0.3.0-r8

Everything in r7 worked around one architectural mistake, and r8 removes the
mistake instead of working around it.

## What was wrong

r7 started the desktop from a **system** unit:

```
graphical.target -> refurb-session.service -> session-start
                 -> runuser -u refurb -- refurb-session
                 -> dbus-run-session -- session-launch (pipewire, mako, labwc)
```

That produced a graphical session with **no logind session behind it**. There is
no PAM login, so there is no `pam_systemd`, so there is no session: no seat, no
`XDG_SESSION_ID`, no `systemd --user`, no user bus. Consequences, all of which
r7 then patched around:

| Symptom | Root cause | r7 workaround (now removed) |
| --- | --- | --- |
| polkit refused NetworkManager / Blueman / udisks2 writes: "not authorized" | the subject was an *inactive* session | broad wildcard polkit rule for five action families |
| `systemctl --user` did not exist | no user manager without a session | PipeWire, pipewire-pulse and WirePlumber spawned by hand in `session-launch`, with no restart supervision |
| audio sockets and the session bus belonged to one private process tree | `dbus-run-session` wrapped the compositor | every program had to inherit the environment from labwc |
| notification daemon died at login | it was started before the compositor existed | a background socket-wait subshell in `session-launch` |
| `loginctl`, suspend inhibition, session locking were unavailable | no session to ask | not attempted |

## What r8 does instead

The desktop now starts the way every Linux desktop starts: as **the user's own
login**.

```
getty@tty1 (ExecStart = tty1-autologin)
  -> agetty --autologin <desktop account>
    -> login -> PAM (pam_systemd) -> logind session: seat0, Type=tty->wayland, Active=yes
      -> systemd --user (dbus.socket, pipewire.socket, wireplumber)
        -> /etc/profile.d/zz-refurb-desktop.sh (only on tty1, never as root)
          -> refurb-session -> labwc
```

The boot experience is unchanged: the machine still goes straight to the
desktop. There is no greeter and no display manager, because the requirement is
an automatic desktop start, not a login screen.

### The pieces

| File | Role |
| --- | --- |
| `/etc/systemd/system/getty@tty1.service.d/10-refurb-autologin.conf` | replaces the packaged `ExecStart` with the wrapper below |
| `/usr/lib/refurb/tty1-autologin` | resolves the desktop account (any uid >= 1000) and runs `agetty --autologin`; falls back to a normal login prompt if there is none |
| `/etc/profile.d/zz-refurb-desktop.sh` | turns that login into the desktop: tty1 only, uid >= 1000 only, never inside an existing Wayland/X session, then `exec refurb-session` |
| `/usr/lib/systemd/system/refurb-session-prepare.service` | the one thing root still has to do before the login: make the desktop account own its home directory (the image is built without root, so it can otherwise belong to the build account) |
| `/usr/bin/refurb-session` | session environment, first-run welcome, then `session-launch`; no `dbus-run-session` any more |
| `/usr/lib/refurb/session-launch` | starts the packaged **user** units for audio and waits for the graph, then `exec labwc` |
| `/etc/xdg/labwc/autostart` | shell supervisor, `mako`, the OSD helper - all after the compositor socket exists |

`refurb-session.service` is gone. An image upgraded from r7 has it disabled and
its `graphical.target.wants` link removed by the build hook.

### Why `agetty --autologin` and not greetd

A greeter would add a second program, a PAM service and a login screen to
preserve an automatic start that autologin already provides. `agetty` is part of
util-linux, is already on the image, and produces exactly the same session a
typed login would: `pam_systemd` registers it, the seat is assigned, the user
manager starts. The account is resolved at boot, so the live medium and an
installed system behave the same whatever uid the account got.

### Services now belong to the session

PipeWire, pipewire-pulse and WirePlumber are the **packaged systemd user units**:

```
/etc/systemd/user/sockets.target.wants/pipewire.socket
/etc/systemd/user/sockets.target.wants/pipewire-pulse.socket
/etc/systemd/user/pipewire.service.wants/wireplumber.service
```

Those links are created by the build hook because `systemctl --user preset`
cannot run in a build chroot - which is also why r7 found the daemons not running
at all and started them by hand. They now restart on failure, inside the session
that owns their sockets and their bus.

`mako` moved from `session-launch` to labwc's autostart, where a Wayland socket
exists by definition. The socket-wait subshell is gone.

## Polkit: from five wildcards to two named actions

The r7 rule allowed every `org.freedesktop.NetworkManager.*`, `org.bluez.*`,
`org.blueman.*` and four udisks2 actions. With a real active session, the
shipped Debian policy already authorizes almost all of that. The values below
are read from the policy files in this image, not assumed:

| Action | `allow_active` | Needed by | r8 rule |
| --- | --- | --- | --- |
| `NetworkManager.network-control` | `yes` | connect/disconnect Wi-Fi | not needed |
| `NetworkManager.enable-disable-wifi` | `yes` | radio toggle | not needed |
| `udisks2.filesystem-mount` | `yes` | USB stick in the file manager | not needed |
| `udisks2.eject-media` | `yes` | eject | not needed |
| `udisks2.power-off-drive` | `yes` | power off a drive | not needed |
| `NetworkManager.settings.modify.system` | `auth_admin_keep` | saving a Wi-Fi profile (writes `/etc`) | **yes** |
| `blueman.rfkill.setstate` | `auth_admin_keep` | Blueman's radio switch | **yes** |

The two remaining actions are the ones no amount of correct session management
can satisfy, because the session has no authentication agent: Debian sets them
to `auth_admin_keep` even for an active local user. Both are granted to the
`netdev` group, which the image already puts the desktop account in.
Partitioning, formatting, mounting another seat's or another user's disk, and
every administrator action stay refused.

## Security posture

* The desktop still starts automatically; nobody has to type a password to reach
  it (unchanged from every previous release, and the machine is a single-user
  laptop).
* The desktop never runs as root: the profile hook refuses any uid below 1000.
* `tty2`-`tty6` remain ordinary recovery consoles with a password login, and
  `tty1` belongs to the desktop, so an unrelated text login cannot take the
  desktop's seat or start a second compositor.
* The polkit exception list shrank from seven entries (five of them wildcards) to
  two exact action ids.
* `sudo` is still limited to the two validated helpers in
  `/etc/sudoers.d/refurb-helpers`.

## Desktop fixes in the same round

* **The bar is at the bottom of every screen by default.** It used to depend on
  `REFURB_BAR_POSITION=bottom`, which nothing set, so every image drew it at the
  top. `REFURB_BAR_POSITION=top` is the escape hatch.
* **Monitors can be connected and disconnected while the desktop runs.** The
  shell bound `wl_output` once at startup and ignored the registry afterwards, so
  a second monitor attached later stayed black and had no bar. It now tracks
  output globals and rebuilds the per-screen wallpaper and bar on add and remove.
* **Quick panel clicks expand immediately.** The panel changes height when a
  section opens, and the click map was only rebuilt after the compositor had
  configured the new size. A click that arrived in between was matched against
  the old layout and silently did nothing - the "quick settings ignores clicks"
  report. The hit map is now rebuilt as soon as the size changes, and the effect
  of a click is applied before the next event in the same batch.
* **The volume percentage shows what the sink did.** After each write the worker
  reads the sink back, so a value clamped by WirePlumber (150% of its limit)
  appears as the real level instead of the dragged position.
* **Notifications got a dark default look** (`/etc/xdg/mako/config`).

## What booting the new image caught

The architecture change was verified by booting the ISO, not by reading it. Four
defects showed up that would have shipped otherwise:

1. **live-config was still owning tty1.** Its systemd generator
   (`live-config-getty-generator`) writes `/run/systemd/generator/getty@tty1.service.d/`,
   which is merged *after* `/etc/systemd/system`, so its
   `agetty --autologin user` replaced the autologin this image needs. There is no
   `user` account, so the first r8 boot ended on `Authentication failure` and a
   login prompt. Fixed with the generator's own supported switch,
   `live-config.nottyautologin` on the kernel command line: it stops the
   generator from writing console autologins, leaves tty2-tty6 as real login
   prompts, and lets the image's own tty1 drop-in apply.
2. **The shell supervisor could not restart the shell without a display name.**
   A crash makes the supervisor start `refurb-shell` again; if `WAYLAND_DISPLAY`
   is absent from that environment the restart dies with *"Could not find
   wayland compositor"* and the desktop stays bar-less. The supervisor now
   discovers the compositor's socket in `XDG_RUNTIME_DIR` when the variable is
   missing, which also makes a hand-started supervisor work.
3. **The compositor keymap never loaded.** The event's file descriptor was read
   with `size` as the limit; on this compositor that produced an empty string,
   libxkbcommon failed to parse it, and the shell fell back to its built-in US
   layout - silently correct on an English image and wrong on any other. The
   descriptor is now read to its end and the failure, if it ever happens again,
   is logged with the byte count the shell saw versus the count it was promised.
4. **flatpak's document portal failed on every boot** (D-Bus activated by the
   browser, needs FUSE, which a live image does not have), leaving
   `systemctl --user` reporting *degraded*. It is masked now; nothing on this
   image uses portals.

## Diagnostics

`refurb-diag` is on `PATH` (`/usr/local/bin/refurb-diag`) and collects, in
addition to r7's inventory:
session and seat state (`loginctl`), the user manager and its units, the runtime
directory (bus and sockets), a `pkcheck` result for the eight authorizations the
desktop actually uses, Bluetooth adapters/devices/service log/modules, three
seconds of key events with key codes, the Wayland renderer, and the shell's own
journal. Credentials are stripped from every collected file before the archive is
written, and `nmcli` is never asked for secrets.

## Monitors appearing after the desktop starts

The shell builds the wallpaper and bar of each screen from the output list it
binds at startup. With a second GPU added to the VM, the compositor's second
head stayed inactive (`Display output is not active.`, no usable mode under
software rendering), so a genuinely late output could not be exercised. Two
safeguards are in place anyway:

- `wl_registry` `Global`/`GlobalRemove` events for `wl_output` bind the new
  output and call `rebuild_screens()`, which closes and re-opens the desktop and
  bar for every screen at the new geometry.
- `Session::reconcile_outputs()` runs on the once-a-second pump tick and compares
  the registry snapshot with the bound outputs, binding or dropping as needed.
  It returns immediately when nothing changed, so it costs a comparison of a
  handful of globals per second, and it does not depend on event delivery
  working.

What was verified in QEMU: one active output, bar at the bottom, wallpaper
painted, shell stable across a supervisor restart. Not verified: two active
screens at the same time.
