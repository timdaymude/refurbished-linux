r8 rebuilds the desktop on a real systemd-logind session and finishes the polish round.

## Session

- The desktop starts as a **PAM login on tty1** (`agetty --autologin` → `refurb-session`), not as a root-run system service. `loginctl` reports uid 1000 on `seat0`, tty1, type `user`; the user manager runs; `XDG_SESSION_ID` and the session bus are real.
- pipewire, pipewire-pulse and wireplumber are enabled as packaged **systemd user units** instead of being hand-started inside `dbus-run-session`.
- Polkit exceptions drop from five wildcard rules to **two exact actions** (NetworkManager system settings, Blueman rfkill) — the stock policy files already grant network control, mounts, eject and power-off to active sessions.
- A second login on another tty no longer disturbs the desktop's runtime directory. `xdg-desktop-portal`, `-document-portal` and `-permission-store` are masked because the live image has no FUSE and no backend.
- The live CD's own tty1 autologin no longer fights ours (`live-config.nottyautologin`).

## Desktop

- The bar is at the **bottom of every screen** by default (`REFURB_BAR_POSITION=top` still flips it).
- Quick-settings clicks act on the current panel: the hit map is rebuilt when it opens and after every click.
- Monitors are picked up from the registry when they appear or disappear, and the screen list is reconciled against it once a second as well.
- Volume changes read the sink value back instead of assuming success; `mako` gets a config.
- The compositor keymap loads instead of silently falling back to US.

## Diagnostics

`refurb-diag` gained session/logind, user units, runtime dir, a `pkcheck` probe, Bluetooth, wayland-info, key events, shell debug journal and notification sections, and redacts credentials before writing the tarball.

## Verified in QEMU on this image

logind session (`seat0`, tty1, `user`), user manager `running`, no failed user units, `pkcheck` = 0 for network-control, settings.modify.system, udisks2.filesystem-mount, eject-media, blueman.rfkill.setstate and login1.power-off; bar at the bottom; quick panel and launcher open; `nm-connection-editor`, `blueman-manager` and `pavucontrol` run; the shell restarts after a kill; the diag archive holds 58 files with no `psk=` or `password=` values. Memory: shell 46 MB, labwc 158 MB under software rendering.

## Not verified

Real Wi-Fi association and reconnect, Bluetooth pairing and audio, a real backlight, media keys (QEMU's PS/2 keyboard never sends `XF86Audio*`), volume against a real sink (the VM only has PipeWire's *Dummy Output*, which ignores writes), a non-US layout, and two simultaneously active screens (QEMU's second GPU has no mode under software rendering). No hardware claims are made for these — see the checklist in `docs/session-0.8.md`.

**SHA256** (ISO, 929,038,336 bytes):

```
38f428fa5497126e8f436a53c069dbf2cfb3e87e125b5cfe92d2af381bf04a10
```
