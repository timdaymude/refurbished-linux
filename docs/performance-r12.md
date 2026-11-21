# Performance and acceptance in r12

What this round costs, and what the test image actually showed. Every number
comes from **system emulation (QEMU, no KVM, no GPU)** — the slowest way to run
this system. They are useful for catching regressions between rounds; they are
not the Chromebook's numbers.

## How it was measured

    qemu-system-x86_64 -m 2048 -smp 2 -machine pc -cpu qemu64 \
        -cdrom reawakening-os-0.3.0-r12-amd64.iso -boot d -vga std \
        -device intel-hda -device hda-duplex \
        -device virtio-tablet-pci -netdev user,id=n0,hostfwd=tcp::9090-:9090 \
        -device virtio-net-pci,netdev=n0,romfile=

2 emulated cores, 1973 MB of usable RAM, Debian's `llvmpipe` software renderer
(`WLR_RENDERER_ALLOW_SOFTWARE=1`), CD-ROM boot. The guest was driven over QMP
(pointer, keys, screenshots) and the serial console; figures come from
`systemd-analyze`, `free -m`, `top` (two samples, three seconds apart) and `ps`.

QEMU's host audio backend could not open ALSA on this build machine ("Could not
initialize DAC/ADC"), so the emulated HDA codec exists but has no working host
device behind it. That matters for the audio section below.

## Boot

| Stage | r12 | r11 |
|---|---|---|
| kernel | 35.5 s | 31.9 s |
| userspace to `graphical.target` | 1 min 4.2 s | 57.2 s |
| total | 1 min 39.7 s | 1 min 29 s |

The difference is emulation variance (the guest is CPU-bound on `llvmpipe`
painting the first wallpaper), not new work at boot: r12 adds no service, no
daemon and no autostart entry. Two boots measured 1 min 37.7 s and 1 min 39.7 s.

## Idle desktop (no applications open)

| Measurement | r12 | r11 |
|---|---|---|
| Memory used | **537 MB** of 1973 MB (available 1435 MB) | 531 MB |
| Processes | 115 | 108 |
| Zombies | **0** | 0 |
| CPU idle (two samples) | 81.2 %, then **97.5 %** | 71.7 % |
| `labwc` RSS | 148 MB | 179 MB |
| `refurb-shell` RSS | **52 MB** | 52 MB |

The extra processes are the ones a desktop session starts on demand and that
r12 now exercises (GVfs metadata/volume monitors, started by PCManFM, which the
desktop opens on a double click). The shell itself did not grow: desktop icons,
pins and the dictation pill are drawn into the wallpaper buffer the shell
already owned, and the desktop scan is two directory reads a second at most.
The idle CPU figure settles at about 2.5 % of two emulated cores once the first
frames are painted.

## Dictation

| Measurement | Value |
|---|---|
| Resident before use | **nothing** (no helper, no model, no microphone stream) |
| Helper RSS while listening | 181 MB (Vosk small model loaded in memory) |
| `pw-record` RSS while listening | 8.8 MB |
| Model on disk | 68 MB (`/usr/share/refurb/vosk/model`) |
| Time from Alt+Z to "Listening" in QEMU | ~20 s (mostly the model load on an emulated CPU); a few seconds on hardware |
| After Alt+Z to stop | helper, recorder, PID file and state file all gone (verified) |

181 MB is the whole cost of dictation, and only while the user is dictating on a
2 GB machine that is otherwise idle at 537 MB. A resident engine would pay that
all the time; this design pays it for the length of an utterance.

Stopping was the one behaviour that needed a fix during acceptance: the first
build's helper noticed the stop only when the next block of audio arrived, which
left the recorder running if the microphone was silent. The shipped helper
terminates `pw-record` directly from its signal handler and treats six seconds
with no audio as "no microphone is available". The fixed path was verified twice:
once on the build host with a stub recorder that streams silence, and once on
the shipped ISO (state file, PID file and both processes gone immediately after
the second Alt+Z).

## Jellyfin audio

| Check | Result |
|---|---|
| Rule installed and loaded | yes (`/etc/wireplumber/wireplumber.conf.d/51-refurb-audio.conf`) |
| `node.suspend-on-idle` on the analog sink | **`"false"`** |
| HDMI/USB sinks | not matched by the rule, keep the default |
| Sink state when idle in QEMU | still `suspended` |

The last line is why this round does not claim the audio fix is verified: this
emulated codec has no host backend, so ALSA cannot be brought up and PipeWire
suspends the node regardless of the policy. The property is set on the node, and
the rule matches only the built-in analog output; whether that keeps the
Chromebook's codec awake, and whether sound then starts with the picture, needs
the machine.

## QEMU acceptance results

Driven from the shipped ISO over QMP and the serial console.

| Area | Result |
|---|---|
| Taskbar pins | Brave and Files chips visible with both applications closed and after reboot |
| Single click on a pin | starts the application (Brave, Files) and shows its running state on the pin |
| Click on a running pin | raises/focuses the existing window; a second click never creates a duplicate chip |
| Closing the application | window closes, running state disappears, pin stays |
| Desktop icons | files, folders and a `.desktop` shortcut from `~/Desktop` appear automatically |
| Double click | opens the folder in PCManFM and runs a trusted shortcut |
| Drag | icon snaps to the grid; `~/.config/refurb/desktop-layout` records the new cell |
| Right click | Open / Rename… / Move to Trash on an icon; New Folder / Add Application Shortcut / Refresh on the wallpaper |
| Rename | inline field commits with Enter, the file on disk is renamed, layout follows the name |
| Delete | item moves to `~/.local/share/Trash` with a `.trashinfo` entry and leaves the desktop |
| Alt+Z | bar pill shows Starting… then Listening; second Alt+Z removes the pill, the helper and the recorder |
| Text insertion | `refurb-dictate type "hello from dictation"` appeared in a focused `foot` window (wtype path) |
| Audio policy | `node.suspend-on-idle = "false"` on the analog sink (see the caveat above) |
| Zombies | 0 after launching and closing Brave, PCManFM and foot |

One QEMU-only artefact is recorded in `docs/limitations.md`: after a window
closes under the pointer, a click on the empty wallpaper can be ignored until
the pointer crosses the bar and back.

## What these numbers do not tell you

- Whether sound starts with the picture on the real codec — the reason the
  WirePlumber rule exists.
- Dictation accuracy, gain and permission behaviour with the Chromebook's own
  microphone.
- Whether Alt+Z coexists with the Chromebook top row (keyboard quirks).
- Brightness/volume/mute on real hardware, idle power draw, Jellyfin playback
  against a real server and Cockpit login from another computer — unchanged
  hardware items from r11.
