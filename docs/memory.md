# Memory: the budget, and how to check it

## The rule

Idle memory means: freshly booted, desktop settled, **no applications open**,
measured as the sum of the resident set size of every process.

Sum-of-RSS is deliberately pessimistic — shared libraries count once per process
— so a number that passes this test will pass any honest measurement.

    ./scripts/measure-memory.sh      # waits 30 s, prints the table, states PASS/FAIL

**No measurement has been taken yet.** The table below is the design budget, not
a result. It exists so that surprises are visible during the first hardware test.

## Budget for a 2 GB machine at idle

| Component | Budget | Why |
|---|---:|---|
| Kernel + drivers + firmware | ~60 MB | not negotiable |
| `systemd`, `journald`, `logind`, `udevd` | ~35 MB | journald is capped, no persistent huge logs |
| `dbus-daemon` (session + system) | ~8 MB | needed by NetworkManager and portals |
| `polkitd` | ~9 MB | needed for NetworkManager authorisation |
| `NetworkManager` + `wpa_supplicant` | ~28 MB | Wi-Fi is non-negotiable |
| `bluetoothd` | ~7 MB | Bluetooth is non-negotiable |
| `pipewire` + `wireplumber` | ~22 MB | audio; `pulseaudio` would be more |
| `udisks2` + `upower` | ~16 MB | USB drives, battery, suspend |
| `labwc` | ~45 MB | small compositor, includes Mesa |
| `refurb-shell` | ~40 MB | includes glyph cache and four shm buffers |
| `Cockpit` | 0 MB | socket-activated: nothing runs until you open it |
| `PackageKit`, indexers, updaters, cloud sync | 0 MB | not installed, or masked |
| **Total** | **~270 MB** | target is < 500 MB, ideal 300–400 MB |

The headroom between ~270 MB and 500 MB is intentional: firmware blobs, GPU
drivers and file caches vary wildly on refurbished hardware.

## What buys the memory back

- **No PackageKit.** The software manager is a window; it shells out to
  `apt-cache` and exits. PackageKit would be a permanent daemon plus a
  catalogue cache.
- **No update checker.** `apt-daily.timer` runs once a day for a few seconds and
  only downloads *metadata* to a cache; `apt-daily-upgrade`,
  `unattended-upgrades`, `fwupd` and PackageKit's offline updater are masked.
- **No indexer.** Nothing scans the home directory, ever. Search in the file
  manager is on demand.
- **Socket-activated Cockpit**, bound to loopback.
- **Brave managed policy disables background mode**, so closing the browser
  really releases its memory instead of leaving a resident process.
- **zram swap** (`min(ram/2, 2048)` with zstd) so a 2 GB machine can hold a
  browser plus an office tab without hitting the disk.
- **`vm.swappiness=150`, `vm.page-cluster=0`**: with zram, swapping cold pages
  early is cheaper than dropping file cache.
- **No notification, tray, clipboard, wallpaper or screen-locker daemon.**
- **`Acquire::Languages "none"`** and no recommended packages: fewer files, less
  disk churn on a spinning disk during updates.

## What it costs, honestly

- Browsing is not free. Brave with a handful of tabs typically sits between
  250 MB and 600 MB on its own; the operating system cannot change that, it can
  only leave room. `docs/testing.md` describes how to test YouTube on real
  hardware and what to expect when hardware decoding is unavailable.
- Tabs of Microsoft 365 are heavier than the desktop itself. The mitigation is
  zram, not pretending.
