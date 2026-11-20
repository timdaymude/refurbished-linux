# Performance in r11

What this round changed that affects speed and memory, and what the test image
actually measured. Every number here comes from **system emulation (QEMU, no
KVM)** — the slowest possible way to run this system, and one with no GPU at
all. They are useful for catching regressions between rounds and for knowing
which process eats memory; they are not the Chromebook's numbers.

## How it was measured

    qemu-system-x86_64 -m 2048 -smp 2 -machine pc -cpu qemu64 \
        -cdrom reawakening-os-0.3.0-r11-amd64.iso -boot d -vga std \
        -device virtio-tablet-pci -netdev user,id=n0,hostfwd=tcp::9090-:9090 \
        -device virtio-net-pci,netdev=n0,romfile=

2 emulated cores, 1973 MB of usable RAM, Debian's `llvmpipe` software renderer
(`WLR_RENDERER_ALLOW_SOFTWARE=1`), CD-ROM boot. The guest was driven over QMP
and the serial console; figures come from `systemd-analyze`, `free -m`, `top`
(two samples, four seconds apart) and `ps`.

## Boot

| Stage | Time |
|---|---|
| kernel | 31.9 s |
| userspace to `graphical.target` | 57.2 s |
| total | 1 min 29 s |

Almost all of that is emulation: the kernel time is dominated by the emulated
firmware and PCI bus, and userspace is dominated by `llvmpipe` painting the
first wallpaper. On the Chromebook the same image reaches the desktop in a few
seconds; r10's hardware notes have the comparable figure.

## Idle desktop (no applications open)

| Measurement | Value |
|---|---|
| Memory used | 531 MB of 1973 MB (available 1442 MB) |
| Processes | 108 |
| Zombies | **0** |
| CPU busy | ~28 % of the two emulated cores (71.7 % idle) |
| `labwc` RSS | 179 MB |
| `refurb-shell` RSS | 52 MB |

The 28 % is the software renderer: `labwc` keeps two full-screen ARGB buffers
per layer surface and `llvmpipe` is doing the compositing on an emulated CPU.
On a machine with a working GPU this is the compositor's job and costs
essentially nothing.

## Jellyfin Media Player at its login screen

| Measurement | Value |
|---|---|
| Memory used | 780 MB, plus 113 MB of swap |
| CPU | 94 % user — the Qt WebEngine and libmpv stack on `llvmpipe` |
| `jellyfinmediaplayer` RSS | 426 MB |
| Three `QtWebEngineProcess` children | 61–100 MB each |
| `labwc` RSS | 200 MB |

Jellyfin is the heaviest thing in the image by a wide margin, and it is the one
component whose numbers will differ most on hardware: with a GPU it decodes in
hardware and the CPU figure collapses. It also pushed the machine into swap on
a 2 GB emulated box with a browser also resident, which is the honest warning
about running a Jellyfin client on a 2 GB machine: it works, but it wants the
RAM.

## Behaviour worth knowing

- **The zombie fix is a thread, not a timer.** `shell/refurb-shell/src/reaper.rs`
  waits on child processes and reaps them as they exit, so applications started
  from the bar or the launcher cannot accumulate `Z` entries. Both measurements
  above report 0 zombies after a full session of launching and closing windows.
- **Closing an application removes its taskbar entry immediately** (the shell
  reacts to the compositor's `closed` event instead of waiting for the next
  redraw tick). Measured: Brave closed from its own close button, twelve
  processes gone, entry gone on the next frame.
- **The quick panel and the power dialog are full-screen layer surfaces.** That
  is what lets them dismiss on a click outside themselves and hold the keyboard,
  and it costs one 1280×800 ARGB buffer pair (about 8 MB) while they are open —
  released when they close, which is why the idle figure above is with both
  closed.
- **The quick panel no longer resizes the surface when a section expands.** Only
  the card moves inside a surface that is already full-screen, so the work is a
  redraw of one card's worth of pixels instead of a compositor round trip.

## What these numbers do not tell you

- Whether Brave's video playback is smooth on the Chromebook, with or without
  `VaapiVideoDecodeLinuxGL`. The emulator has no GPU to decode with.
- Whether the 160 % audio ceiling is the right one, or whether the speakers are
  loud enough — untestable without the machine.
- Whether the top row's brightness keys change a real backlight.
- Idle power draw, which is the number that matters most on a laptop and which
  nothing in an emulator can measure.

Those are on the hardware list for the next session with the Chromebook.
