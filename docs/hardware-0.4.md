# Hardware work: Wi-Fi, Bluetooth, audio, media keys (0.3.0-r3)

Round 6 of Refurbished Linux. The image shipped as `v0.3.0-r3`. This file
records what was actually wrong, what changed, and — just as important — what
could not be verified without physical hardware.

Everything below was measured in QEMU (2 cores, 2 GB, TCG, OVMF UEFI). No
physical laptop was available, so "verified" means "verified against virtual
hardware", and anything requiring a real radio, codec or backlight is listed as
unverified rather than claimed.

## What was broken (root causes, with evidence)

| Symptom | Cause found in the image |
|---|---|
| Wi-Fi found no networks | `wpasupplicant` was **not installed**. NetworkManager had no association backend, so scans returned nothing and connections were impossible. |
| Bluetooth never scanned | `bluetooth.service` was present but **never enabled**. `systemctl enable` inside the build chroot silently fell back to the SysV helper and created no unit symlink. |
| Sound dead, volume always "Muted" | Nothing ever started **PipeWire or WirePlumber**. The desktop is a plain `runuser` unit, not a logind session, so no user services existed. `systemd.rs` then reported `(0.0, muted=true)` when `wpctl` failed, so the bar said "Muted" and every slider move was overwritten by the next poll. |
| Volume slider snapped back | The quick-settings volume row had a hidden `MuteToggle` hit area covering the whole slider track, so dragging also muted. |
| Volume capped at 100% | `set_volume` clamped to 1.0 and WirePlumber's own ceiling is 150%. |
| Media keys did nothing | The keybinds called `wpctl`/`brightnessctl` with no OSD and inherited a PATH that does not exist outside a login shell. |
| `sudo`/`su` unusable | **No setuid binaries existed in any released image.** The rootless build cannot apply the S bit; dpkg recorded 4755 but the files were 0755. |
| Non-root accounts could not run anything | The source tree's directories are 0700 in this checkout (git does not track directory modes) and `cp -a` copied that onto `/`, `/etc`, `/usr`. Login failed with `no shell: Permission denied`. |
| Microsoft apps preinstalled | Four `.desktop` launchers (Microsoft 365, Outlook, OneDrive, Teams) plus welcome-page links and a first-run comment advertising them. |

## What changed

**Wi-Fi** — `wpasupplicant`, `iw` added to the base package list. NetworkManager
is explicitly enabled. Firmware packages were already present
(`firmware-iwlwifi`, `-realtek`, `-atheros`, `-brcm80211`, `-mediatek`).

**Bluetooth** — `bluetooth.service` is enabled by creating the unit symlink
directly (not relying on `systemctl enable` in a chroot), plus
`/etc/modules-load.d/refurb-bluetooth.conf` so `/sys/class/bluetooth` exists and
the unit's `ConditionPathIsDirectory` can pass. An `ExecStartPre` unblocks
rfkill. `bluez-firmware` added for Broadcom/Intel combo cards.

**Audio** — new `/usr/lib/refurb/session-launch` starts `pipewire`,
`pipewire-pulse` and `wireplumber` inside the session bus and waits for the
daemon before the compositor draws. `pipewire-alsa`, `alsa-ucm-conf`,
`alsa-utils`, `firmware-sof-signed` and `firmware-intel-sound` added: without
UCM profiles many laptop codecs come up muted or with the wrong output, and
modern Intel laptops need the SOF blobs to have any codec at all.
`volume()` now returns "not muted" when no daemon answers instead of claiming
the user muted the machine, and the volume slider no longer carries a mute
hit area.

**Amplification** — `system::VOLUME_MAX = 1.6`; `wpctl set-volume --limit 1.6`
lifts WirePlumber's 150% ceiling; the slider's whole track maps to 0–160%, so
100% sits at 62.5% of the travel.

**Media keys** — all five keys now call one helper,
`/usr/lib/refurb/refurb-control`, which applies the 160% ceiling, unmutes on
volume-up, reports brightness from sysfs, and feeds the OSD. Keybinds use the
absolute path, `refurb-session` exports a real PATH, and the helper sets its own.

**Microsoft software** — the four launchers are deleted from the tree and also
removed at build time (an image built from an older tree still contains them),
the welcome page no longer links to or advertises them, and its copy says they
are not installed. Users can still open those services in Brave; nothing is
pinned, bundled or running.

**Build correctness** — `scripts/mkiso.sh` now normalises directory modes before
copying the includes tree and restores setuid/setgid bits after package
installation. Both bugs shipped in every earlier image.

## Verification performed

Booted under OVMF UEFI with a virtual HDA codec (audio captured to a WAV file)
and, for Wi-Fi, the kernel's `mac80211_hwsim` simulated radio.

| Check | Result |
|---|---|
| Image boots to the desktop | **yes** — bar, wallpaper, Brave welcome page (screenshot: `docs/screenshots/final-desktop.png`) |
| Bluetooth service | **active**, `ConditionResult=yes` |
| NetworkManager + `wpa_supplicant` | **active / active** |
| Wi-Fi scanning | **works** — `nmcli device wifi list` returned the test AP on channel 6 with signal 100 |
| Wi-Fi association | **not verified** — the virtual AP could not hand out DHCP in this VM; association was not observed end-to-end |
| Audio playback | **works** — 2 s of 1 kHz sine captured from the emulated codec: 264,598 non-zero samples, peak 23,844 at 90% volume |
| Volume up/down keys | **work** — three injected `XF86AudioRaiseVolume` presses raised the sink 0.50 → 0.65; two `XF86AudioLowerVolume` presses took it 0.65 → 0.55 |
| 160% amplification | **works** — `wpctl get-volume` reports `1.60` after setting 160% |
| Mute does not come back | **works** — muting then raising the volume leaves the sink `1.60` and unmuted |
| Mute key | **binding verified, key injection unavailable** — QEMU's `sendkey` has no mute key; the same helper path is what the key runs |
| Brightness keys | **unverified** — QEMU has no backlight device, so `brightnessctl` has nothing to act on |
| OSD feedback | **unverified** — `wob` starts, but no overlay was observed. See the caveat below. |
| Microsoft launchers | **gone** — no `.desktop` entries, no links, verified inside the built image |
| Microsoft packages/services | **none exist** — the system never contained any Microsoft binaries or daemons |
| sudo setuid | **restored** — `-rwsr-xr-x` in the built image, `sudo -n true` succeeds for the live account when a permissive sudoers is present (test build) |

## Known problem found while testing

The desktop is not a logind session (by design: no display manager). When a
user logs in on a text console (`Ctrl+Alt+F2`), logind mounts a fresh tmpfs over
`/run/user/1000`. The running compositor keeps working through its already-open
file descriptors, but the Wayland socket and the OSD FIFO become unreachable by
path, so newly launched graphical programs cannot connect.

This affects the shipped image on real hardware too, and it is the reason the
OSD could not be exercised in this VM: any serial login used to run tests
performs exactly that mount. It needs a proper fix (registering the desktop as a
logind session, or making `/run/user/<uid>` a persistent mount), and is the next
thing to address.

## Not verified at all

Physical Wi-Fi radios and firmware, Bluetooth discovery and pairing with real
devices, real codec output through speakers and headphones, brightness keys on
real backlight hardware, suspend/resume, and any laptop model. Those need a
machine in front of someone.
