# 0.3.0 r9 — the Chromebook bug round

r8 booted and drew the desktop on an Acer Chromebook, but five things were
broken. This round is about those five, not about new features: the desktop is
the same custom shell, labwc and bar, and every r8 architectural change (the
logind session, the user units, the two polkit actions) is untouched.

Each section states what the hardware showed, what in the r8 source caused it,
what changed, and what has to be checked on the machine itself.

## 1. Wi-Fi: `802-11-wireless-security.key-mgmt: property is missing`

**Cause.** r8 called

```
nmcli device wifi connect <ssid> password <passphrase>
```

and let NetworkManager work out the security method from its scan cache. When
that lookup comes up empty — a fresh scan, an access point whose beacon was
partly parsed, a hidden or just-appeared network — NetworkManager writes a
profile with a `[wifi-security]` section that has no `key-mgmt` at all, and
activation stops with exactly the message on the report. The shell never set the
property itself, so it had nothing to fall back on.

**What r9 does.** The shell now decides the method from the `SECURITY` column of
the scan and writes a complete profile:

| Scan says                         | `key-mgmt` |
| --------------------------------- | ---------- |
| `--`, empty                       | (no `[wifi-security]` section: open) |
| `WPA2`, `WPA1 WPA2`, `WPA2 WPA3`  | `wpa-psk`  |
| `WPA3` on its own                 | `sae`      |
| `OWE`                             | `owe`      |
| `WEP`                             | `none` + `wep-key0` |
| `802.1X` / EAP                    | refused with a readable message: an enterprise login needs an identity, not a passphrase |

`system::connect_wifi` now creates the profile through NetworkManager's own
interface (`nmcli connection add … 802-11-wireless-security.key-mgmt …`) and
activates it with `nmcli connection up <name> passwd-file <file>`, where the file
is a `0600` file in `$XDG_RUNTIME_DIR` that is deleted as soon as activation
returns. So:

* no network is forced onto one authentication type — the scan result decides;
* the profile carries `autoconnect=true`, so the network comes back after a
  reboot through NetworkManager's own machinery;
* the passphrase never enters an argument list (`ps` and every
  `/proc/<pid>/cmdline` are readable by other local users), and never reaches a
  log line, an error string or the diagnostic archive;
* a profile this shell created that would not activate is deleted again, so
  nothing is retried on every boot; a profile the user saved themselves is left
  alone;
* if `nmcli` gives up waiting but the link came up anyway (a slow association),
  the shell asks NetworkManager whether the connection is active before it
  reports a failure.

**A dead end worth recording.** The first attempt wrote a keyfile and used
`nmcli connection load`. That cannot work for a desktop user: NetworkManager's
keyfile plugin only accepts files owned by root, and a user-owned file is
rejected with `failed to load connection: access denied` and
`settings: load: no settings plugin could load` in the daemon log. The profile
has to be created over D-Bus, which is exactly what the netdev rule for
`NetworkManager.settings.modify.system` is for.

**A second defect in the same area.** The status the panel showed came from
`nmcli -t -f IN-USE,SSID,SIGNAL device wifi ifname …`, and `nmcli` rejects
`ifname` there ("argument 'ifname' not understood"), so the card said "Not
connected" on a link that was up. It now reads the active connection list and
prefers the station interface, so a machine running its own hotspot reports the
network it joined rather than the hotspot.

A network that was saved earlier is tried first (`nmcli connection up id …`).
Only when NetworkManager answers that secrets are missing does the password
field open, which is what makes a known network join without asking again.

Open networks take the same path with no security section — they are not
"secured with an empty password".

**To check on hardware:** join a WPA2 network with the right password; join it
again after a reboot; type a wrong password (expect a readable refusal, not a
`key-mgmt` error); join an open network; join a WPA3-only network if one is
available. `refurb-diag` records the profiles (`23-nmcli-connections.sh`) but
not their secrets.

## 2. Maximize and full-screen

**Cause.** `rc.xml` asked for `Clearlooks`, a theme that is not installed in the
image — `/usr/share/themes` holds only Adwaita, Adwaita-dark, Default and Emacs.
labwc therefore fell back to its built-in decoration, and the titlebar layout
the desktop intended was never what the user saw. There was also no
double-click binding, and nothing bound F11.

**What r9 does.**

* Ships `/usr/share/themes/refurb/openbox-3/themerc` (dark titlebar, teal hover
  on the buttons) and names it in `rc.xml`.
* Spells the layout out: `<layout>icon:iconify,max,close</layout>`, so the
  maximize button is the one in the middle of the titlebar.
* Double-clicking the titlebar toggles maximize (`<context name="Titlebar">`).
* `Super+F` and `Super+Up` toggle maximize, `Super+Down` restores, `F11` and
  `Super+F11` toggle true full-screen. Maximize keeps the titlebar and stops
  above the bar (the bar reserves its height as an exclusive zone); full-screen
  removes the decorations and covers everything, which is the difference
  between the two.

Windows are still stacked, centered and conventional; nothing about the desktop
layout changed.

**To check on hardware:** the maximize button is visible and works; double-click
the titlebar maximizes and restores; `Super+F` does the same from the keyboard;
F11 on a video or browser goes full-screen; a maximized window does not hide
under the bar.

## 3. Two launchers in the bar

**Cause.** The bar drew a brand button and an "Applications" button next to it,
and both opened the same launcher. It looked like two different menus.

**What r9 does.** The brand mark is the only launcher and its hit area grew to
cover the chip plus its margin. Nothing else moved: the launcher still lists
every application, Super+Space still opens it and software settings are still
reachable from it.

## 4. Brightness keys

**Cause (software side).** `refurb-control brightness` wrote sysfs and fell back
to `brightnessctl`. On a Chromebook the panel is usually `intel_backlight`,
which is root-only until the seat's ACL is applied; a session that cannot write
it gets a silent no-op. r8's logind session makes the ACL possible but does not
guarantee it.

**What r9 does.**

* `refurb-control` asks **logind** first
  (`org.freedesktop.login1.Session.SetBrightness`, through
  `/session/auto`), which is the path the firmware's own keys use and which
  works for the session's own user. sysfs and `brightnessctl` remain as
  fallbacks, and every path verifies the value actually changed before
  reporting success.
* `Super+F6` / `Super+F7` run the same command, so the machine is controllable
  even if the top row never reaches the compositor.
* `refurb-keys` shows the keycodes the kernel delivers, with the mapping for
  brightness (224/225), mute and volume (113/114/115) and the top row reported
  as F6–F10 (93–97).
* `refurb-diag` gained the labwc environment (`57`), the backlight devices and
  their values (`58`) and a key-probe hint (`59`).

**Not claimed:** that the Acer's brightness keys send `XF86MonBrightnessUp` and
`XF86MonBrightnessDown`. That is a property of its firmware, EC and kernel
driver, and no QEMU run can show it.

## 5. Volume keys

The same key path. `refurb-control volume` and `mute` are unchanged: `wpctl`
with the 160% ceiling, a read-back, and the OSD. `Super+F8/F9/F10` are bound to
mute, volume down and volume up for the same reason as the brightness fallbacks.

**Not claimed:** that the Acer's volume keys send `XF86Audio*`.

## What was verified in QEMU, and what was not

Verified on the r9 image in QEMU (nothing below is a hardware claim):

* the desktop still starts as a logind session on `seat0`/tty1 with no failed
  user units;
* the bar has one launcher, at the bottom;
* the titlebar shows the iconify, maximize and close buttons with the new
  theme, maximize works from the button, the double-click and `Super+F`, and
  F11 goes full-screen;
* a real WPA2 access point running on a second `mac80211_hwsim` radio can be
  joined **through the panel**: the network is listed, the password field takes
  twelve characters, Connect brings `wlan0` up, and the resulting profile
  carries `802-11-wireless-security.key-mgmt: wpa-psk` — the reported error does
  not occur. The temporary password file is gone afterwards, and the earlier
  saved-profile attempt reconnected without asking again;
* a wrong passphrase produces a readable refusal and leaves no profile;
* an open network joins without a password;
* the logind brightness path moves the backlight of a test device.

Not verified, and needing the Chromebook:

* the association and reconnection of the machine's own Wi-Fi chipset;
* the real keycodes of the top row (run `refurb-keys` and send the keycode
  lines, or `refurb-diag` and send sections 51, 55, 57 and 59);
* whether the panel backlight is `intel_backlight`, `amdgpu_bl0` or a
  firmware-specific device, and whether logind can drive it (section 58);
* Bluetooth pairing and audio, and the media keys of an external keyboard.
