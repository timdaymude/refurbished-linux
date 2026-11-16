# Testing: acceptance protocol

The specification's first rule is that a successful installation is not enough.
This is the protocol to run on real hardware — the one thing that has not been
done yet.

## Test machines

Minimum two, ideally three:

| Machine | Why |
|---|---|
| Dual-core, 2 GB RAM, spinning disk, Intel HD 2000/3000-era graphics | the design target; the harshest case |
| Dual-core or better, 4 GB RAM, SSD, AMD APU | the comfortable case |
| Anything with a known-good Wi-Fi and Bluetooth adapter | isolates network problems from desktop problems |

Record for each: CPU model, RAM, GPU, disk type, Wi-Fi chipset (`lspci -nn`),
Bluetooth chipset (`lsusb`), BIOS year.

## Before you start

    sudo apt-get install btop            # or use the terminal you have
    ./scripts/measure-memory.sh          # baseline, nothing open

## The acceptance tests

| # | Test | Pass condition |
|---|---|---|
| 1 | Install on a dual-core 2 GB machine | installer completes; installed system boots to the desktop; no error dialogs |
| 2 | Idle RAM | `./scripts/measure-memory.sh` reports under 500 MB with the desktop settled and nothing open |
| 3 | Boot without background activity | after 5 minutes idle, `btop` shows no process above 1 % CPU average, and no `apt`/`packagekit`/indexer process exists |
| 4 | Wi-Fi through the GUI | click the bar → gear → Wi-Fi → choose network → type password → connected; reboot → reconnects automatically; wrong password shows an error, not a hang |
| 5 | Bluetooth | open quick settings → Bluetooth on → device appears in the list within 15 s → pair → connect → audio plays on headphones; disconnect and reconnect without unpairing |
| 6 | Brave | start from the pinned launcher entry; a page loads; `brave://gpu` shows a rendering backend |
| 7 | YouTube | see the matrix below |
| 8 | Microsoft 365 | sign in at office.com; open a Word document; edit and save; Outlook and OneDrive load |
| 9 | Software manager | open → search “gimp” → results within 1 s after typing stops → Install → progress visible → app appears in the launcher |
| 10 | Install/uninstall | install a small package and remove it again; no leftover broken dependencies (`sudo apt-get check`) |
| 11 | Updates | Settings → Updates → Check now → security count shown → Install → list of packages → completes → reboot is offered if needed |
| 12 | Cockpit | open Cockpit from the launcher; log in as the user; open the Terminal; `uptime` works; `ss -ltn` shows port 9090 bound to 127.0.0.1 only |
| 13 | Files and USB | file manager opens home; plug in a USB stick → it appears → copy a file → eject → no data loss |
| 14 | Power | shutdown, restart, suspend and resume; after resume, Wi-Fi reconnects and the screen is usable |

Extra checks worth recording: volume keys, brightness keys, `Super+L` lock and
unlock, `Alt+Tab`, printing (not supported — see limitations), and a second user
account (not supported by the current session service).

## YouTube matrix

YouTube is the most common complaint on old hardware, so test it explicitly.
For each machine, record the *dropped frame* count from YouTube's “Stats for
nerds” after 60 seconds of playback at each setting.

| Resolution | Hardware decoding | Expected |
|---|---|---|
| 480p | on | smooth everywhere |
| 720p | on | smooth on 3rd-gen Intel and newer, AMD GCN and newer |
| 720p | off (forced software) | may drop frames on dual-core; still watchable |
| 1080p | on | only on hardware that supports VP9 or H.264 decode |
| 1080p | off | expect drops; this is a hardware limit, not a bug |

How to force software decoding, to see the fallback:

    brave-browser --disable-accelerated-video-decode

How to confirm what is actually used: `brave://gpu` → “Video Decode”, and
`about:media-internals` during playback. If Mesa reports `VA-API` as
unavailable on older Intel, the reason is usually a missing
`intel-media-va-driver` codec for that generation; 720p H.264 should still work
through the i965 driver. Record what you see instead of assuming.

## Recording results

For each machine, keep a short report:

    Machine: ThinkPad X220, i5-2520M, 8 GB -> limited to 2 GB in firmware, SSD
    Test 1: pass. Installer: automatic partitioning, ext4, no swap partition
            (zram provides swap). Boot: 22 s to desktop.
    Test 2: 318 MB (see attached measure-memory output)
    Test 4: pass, Intel Centrino Advanced-N 6205, reconnect after reboot 4 s
    Test 7: 720p hw on: 0 dropped / 1800; 1080p hw on: 112 dropped (VP9)

Failures are more useful than passes: include `journalctl -b -p err` and the
output of the script.
