r9 fixes the five failures reported from the Acer Chromebook. The desktop, the
session work from r8 and every component that already worked are unchanged.

## 1. Wi-Fi: `802-11-wireless-security.key-mgmt: property is missing`

The shell used to let NetworkManager infer the security method from its scan
cache. When that lookup came up empty, NetworkManager wrote a profile with no
`key-mgmt` at all and activation died with exactly that message.

Now the scan result decides, and the method is written into the profile:

| Scan says | `key-mgmt` |
|---|---|
| `--`, empty | (none — open network) |
| `WPA2`, `WPA1 WPA2`, `WPA2 WPA3` | `wpa-psk` |
| `WPA3` on its own | `sae` |
| `OWE` | `owe` |
| `WEP` | `none` + WEP key |
| `802.1X` | refused with a readable message (needs a login, not a password) |

The profile is created over NetworkManager's own interface with
`autoconnect=true`, and activated with `passwd-file`, so the passphrase never
appears in a process argument list, a log line or the diagnostic archive. The
temporary file is deleted at once. A profile that fails to activate is removed
again; one the user saved is left alone. A saved network is tried before asking
for a password, which is what makes it reconnect by itself after a reboot.

Also fixed: the panel's status query used an `nmcli` form that always failed, so
it said **"Not connected"** on a working link. It now reads the active connection
and prefers the station interface.

## 2. Maximize and full-screen

r8 asked labwc for a `Clearlooks` theme that is not installed in the image, so
the decoration the desktop configured was never the one on screen. r9 ships a
`refurb` theme (dark titlebar, teal hover), spells the layout out as
`icon:iconify,max,close`, and adds:

* double-click on the titlebar toggles maximize;
* `Super+F` and `Super+Up` toggle maximize, `Super+Down` restores;
* `F11` and `Super+F11` toggle true full-screen (no titlebar, covers everything).

A maximized window stops above the bar, which keeps its exclusive zone.

## 3. One launcher

The brand mark is the only launcher and opens the full application menu; the
redundant "Applications" entry is gone. Nothing else moved.

## 4. Brightness and 5. volume keys

`refurb-control brightness` now asks **logind** first
(`Session.SetBrightness`), which is the path the firmware's own keys use and
which works for the session's own user even when the backlight node is root-only;
sysfs and `brightnessctl` remain as fallbacks and every path verifies that the
value actually moved. `Super+F6`/`F7` (brightness) and `Super+F8`/`F9`/`F10`
(mute, volume down, volume up) run the same commands, so the machine stays
controllable whatever the top row sends. `refurb-keys` shows the keycodes the
kernel delivers, and `refurb-diag` gained the labwc keyboard environment, the
backlight devices and a key-probe section.

## Verified in QEMU on this image

* a real WPA2 access point running inside the VM (second `mac80211_hwsim` radio)
  joined **through the panel**: network listed, password field, Connect, link up,
  and the profile carries `802-11-wireless-security.key-mgmt: wpa-psk` — the
  reported error does not occur;
* the passphrase left no file behind and is not in any argument list;
* a profile saved by a previous attempt reconnected without asking again;
* the panel's status then named the joined network;
* the theme's titlebar shows minimize/maximize/close, the maximize button fills
  the working area above the bar and shows the restore glyph when maximized;
* the bar has one launcher;
* 20 unit tests (security mapping, profile arguments, secret handling, timeout
  handling, nmcli parsing) and a clean release build.

## Not verified — needs the Chromebook

* the machine's own Wi-Fi chipset: association, a wrong password, a WPA3-only
  network, and reconnection after a reboot;
* whether its top row sends `XF86MonBrightnessUp`/`XF86Audio*` or F6–F10 (run
  `refurb-keys`, or `refurb-diag` and send keyboard sections 51, 55, 57, 59);
* which backlight device it exposes and whether logind can drive it
  (`refurb-diag` section 58);
* Bluetooth pairing/audio, and the media keys of an external keyboard.

Running `refurb-keys` and `refurb-diag` on the machine and sending the results is
the fastest way to close the remaining gaps.

**SHA256** (`refurbished-linux-0.3.0-r9-amd64.iso`, 929,038,336 bytes):

```
4c7548668ee635d0e60f8722ff879507a3b7445a0f7468f92e7f470344b7387e
```
