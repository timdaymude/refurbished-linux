# distro/ — the live-build tree

    config/common                       live-build options (trixie, amd64, iso-hybrid)
    config/package-lists/*.list.chroot  what gets installed, by purpose
    config/archives/brave.*             Brave's apt repository and signing key
    config/hooks/normal/*.hook.chroot   identity, masking, browser defaults, session
    config/includes.chroot/             every file that lands in the image
    config/packages.chroot/             .debs installed into the image (refurb-shell)

Build it with `sudo ../scripts/build-iso.sh`, which builds the shell package,
runs `../scripts/check-config.sh`, and then `lb config && lb build` here.

## The files that matter

| File in `includes.chroot` | Role |
|---|---|
| `usr/lib/systemd/system/refurb-session.service` | logs uid 1000 in on tty1 and starts the desktop; no display manager |
| `usr/bin/refurb-session` | session environment, first-boot welcome page, `exec labwc` |
| `etc/xdg/labwc/rc.xml` | keybindings, window rules (no server-side decorations on shell windows) |
| `etc/xdg/labwc/autostart` | starts `refurb-shell` — and nothing else |
| `usr/lib/refurb/refurb-apt` | the only way the Software window can install or remove packages |
| `usr/lib/refurb/refurb-update` | metadata refresh and upgrade, on request only |
| `etc/systemd/system/cockpit.socket.d/listen.conf` | Cockpit on `127.0.0.1:9090` only |
| `etc/brave/policies/managed/refurb.json` | no background mode, no metrics, hardware acceleration on |
| `etc/systemd/zram-generator.conf` | compressed swap, `min(ram/2, 2048)` |
| `usr/share/refurb/welcome/index.html` | the first-boot page |

## Changing what is installed

Edit the list, not a hook. `refurb-base` is the system, `refurb-desktop` is the
desktop, `refurb-browser` is Brave, `refurb-cockpit` is Cockpit,
`refurb-firmware` is firmware. Anything else belongs in the Software window, not
in the image.
