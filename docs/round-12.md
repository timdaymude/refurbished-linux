# Round 12 — Desktop usability, voice dictation and Jellyfin audio

r12 starts from r11, which is complete and published (`v0.3.0-r11`,
commit `a29eef0`). Nothing from r11 was rebuilt: the power confirmation, the
quick-panel dismissal, Alt+F10, the zombie reaper, Jellyfin Media Player and
Cockpit are unchanged. This round adds four things and investigates a fifth:

1. Brave and Files stay on the taskbar, closed or not.
2. `~/Desktop` is the desktop: icons, shortcuts, arrangement, menus, rename and
   delete.
3. Alt+Z voice dictation, offline, with no resident speech process.
4. The Jellyfin audio start delay: investigation and a targeted audio fix.
5. Measurements, documentation and an honest account of what only the
   Chromebook can verify.

The custom desktop is untouched: labwc, the shell, the dark theme, both
wallpapers, the bottom bar, launcher, quick settings, multi-monitor support and
Windows-style snapping are all as they were. No desktop environment was
installed and no new background service was added.

## 1. Taskbar pins

The bar used to show only *running* windows, so Brave and Files disappeared from
it the moment they were closed. A Windows user reaches for those two icons
first, and an empty taskbar reads as "nothing is installed".

`X-Refurb-Taskbar=1` in a `.desktop` file now pins it to the bar. Exactly two
entries carry it — `brave-browser.desktop` and `refurb-files.desktop` — and the
configuration check fails the build if that set changes.

| Action | Behaviour |
|---|---|
| Application closed | Icon is visible, in ink, with no running state |
| Single click | Starts the application |
| Click while running | Focuses its window (`zwlr_foreign_toplevel.activate`) |
| Window closes | Accent background and pip go away; the icon stays |
| Second Brave window | No second icon: the pinned icon is the Brave icon |
| Logout / reboot | Pinned entries come from the installed desktop files, so they are always there |

The pinned chips are laid out before the window chips (left to right) so a
window can never push them off the edge of a narrow screen. Window chips for
non-pinned applications are unchanged, including their titles and active state.

Resolution uses the compositor's application id. `apps::matches_toplevel` accepts
the desktop file's stem (`brave-browser`, `pcmanfm`) and the first word of
`Exec=`, which covers every program this image ships. Unit tests cover both
matches and a non-match.

## 2. Desktop icons and shortcuts

`~/Desktop` is now the desktop, without a file manager or a desktop environment:

- Files and folders appear as icons, folders first, then by name.
- `.desktop` files appear as application shortcuts (a copy of a system entry is
  enough; see "Adding shortcuts" below).
- Double-click opens: folders in PCManFM, files through `xdg-open`, shortcuts
  through their own `Exec=`.
- Icons can be dragged to another grid cell and stay there.
- Right-click on an icon: Open, Rename…, Move to Trash.
- Right-click on the wallpaper: New Folder, Add Application Shortcut, Refresh.
- Rename happens in place: the label becomes a text field, Enter commits,
  Escape cancels, and the position is kept.
- Delete moves the item to the freedesktop trash (`~/.local/share/Trash`) with a
  `.trashinfo` file, so it can be restored from PCManFM's Trash. Nothing is
  unlinked outright.
- Positions live in `~/.config/refurb/desktop-layout` (name, column, row).
- The directory is rescanned when its signature (names, sizes, modification
  times) changes, checked at most every two seconds, so new files appear without
  a watcher or an indexer.

### Where the icons are drawn

They are painted onto the existing wallpaper layer surface
(`zwlr_layer_shell_v1` background layer), which is already below every window
and outside the bar's exclusive zone. That gives three properties for free:

- a click on a window never reaches a desktop icon;
- window snapping, maximize and the taskbar are unaffected;
- a second monitor gets its own wallpaper and no second set of files, because
  icons are drawn only on the primary screen.

The wallpaper was already re-rendered whenever the layer was damaged; adding a
handful of icons costs one glyph and one label per icon in that same frame.

### Trust: shortcuts are not auto-run

A `.desktop` file is a command. Double-clicking an arbitrary one that arrived
in a download would be remote code execution with extra steps. A shortcut is
launched only when all of these hold:

- it is a regular file, not a symlink;
- it is owned by the logged-in user;
- it is not writable by group or others;
- it has the owner execute bit.

Anything else produces a message on the desktop explaining why it was refused.
The test `an_untrusted_shortcut_is_not_trusted` covers the world-writable,
non-executable and executable cases.

### Adding shortcuts

"Add Application Shortcut" in the desktop menu lists the pinned applications
(Brave, Files, Software, Settings, Cockpit, Terminal). Choosing one copies its
installed `.desktop` file into `~/Desktop` with mode 0755. The trust therefore
comes from the system directory it was copied out of, not from the fact that it
now sits on the desktop. Copies made by hand from `/usr/share/applications`
work the same way; anything dropped by a browser does not.

## 3. Alt+Z voice dictation

### What SMD Linux had

The handoff asked for the SMD implementation to be reviewed first. There is no
public SMD Linux source: the only repository on the `timdaymude` GitHub account
is an empty 2015 `chromium` fork, and a web search finds no SMD Linux
repository, package or documentation. Nothing could be reused or checked, so
this round did not pretend otherwise; the design below is new and its engine
choice is explained rather than inherited.

### Design

| Part | Decision |
|---|---|
| Activation | `Alt+Z` toggles. The binding is in `rc.xml`; `Ctrl+Z` is untouched and still reaches the application as Undo. |
| Capture | `pw-record` at 16 kHz mono, streamed over a pipe. Nothing is written to disk. |
| Engine | Vosk small English model (about 68 MB), running in-process in Python. Offline: no network access at runtime, no account, no upload. |
| Text insertion | `wtype`, the Wayland virtual-keyboard client. No X11 tool, no `ydotool`, no root, no uinput device. |
| Lifetime | The helper loads the model, records, transcribes and exits. There is no resident daemon and no idle microphone stream. |
| Indicator | The bar shows a microphone pill: "Starting…" while the model loads, "Listening" while recording, and the error for six seconds when something fails. |
| Privacy | Recording starts only on the key press and stops on the next one; the state file says what is happening; audio is discarded as it is transcribed. |

Speech is typed as each utterance completes (when Vosk detects a pause), and any
remainder is typed when dictation stops, so text appears while speaking without
the complexity of rewriting partial text with backspaces.

### Failure handling

- No speech engine or model → "the speech engine is not installed" / "the speech
  model is missing" in the bar, and the helper exits.
- No recorder → "the audio recorder is not installed".
- No default microphone → `pw-record` ends immediately; the bar says "no
  microphone is available" for six seconds and the helper exits non-zero.
- A microphone that opens but delivers nothing (suspended device, permission
  problem) → after six seconds with no audio the helper reports "no microphone
  is available" and exits, rather than leaving a pill that says "Listening".
- Stopping dictation ends the recorder at once. The signal handler terminates
  `pw-record` directly instead of waiting for the read loop to notice, which is
  what keeps a silent or suspended microphone from staying open after Alt+Z.
- `wtype` missing or the window refuses the text → an explicit error, not silent
  loss.
- A stale PID file (the helper crashed) is detected by checking
  `/proc/<pid>/cmdline`, so the next Alt+Z starts fresh.

### Cost

The engine is not resident. While dictating, the Python process and the model
are the large part of the cost; measurements are in
`docs/performance-r12.md`. Between uses the cost is one `Alt+Z` binding and a
700-byte state file read about once a second by the shell.

## 4. Jellyfin audio start delay

### What was observed

On the Chromebook, Jellyfin video through Brave starts immediately and the sound
arrives several seconds later. A similar symptom appeared during SMD Linux
development, but no SMD source or notes are available to confirm the cause was
the same, so nothing was assumed.

### Investigation

The candidates in the handoff were PipeWire, WirePlumber, ALSA initialisation,
buffering, browser audio initialisation, output-device suspension and service
configuration. The one that fits the symptom is **output-device suspension**:

- WirePlumber suspends an audio sink that has been idle (the default in
  WirePlumber 0.5, `node.suspend-on-idle`).
- Chromium — and therefore Brave — creates its audio output stream only when
  playback starts, not when the page loads.
- Resuming the ALSA device on this class of hardware takes seconds: the codec is
  powered up, the UCM profile is applied and the first samples are dropped while
  the device settles. Video is already on screen because it does not wait for
  audio.

This explains "audio late, then normal", and why a second video starts with sound
immediately. It also explains why the same image behaves differently on a
desktop with a permanently running sink (HDMI output, for example).

### Fix

`/etc/wireplumber/wireplumber.conf.d/51-refurb-audio.conf` sets
`node.suspend-on-idle = false` for the built-in **analog** output only
(`node.name = "~alsa_output.*analog.*"`). HDMI and USB outputs keep the default,
so they still power down.

This is a deliberate trade: an idle analog codec stays powered, costing a small
amount of battery, in exchange for sound starting with the picture. The
alternative — a helper that wakes the sink when Brave starts — would be a
background process, which this project does not add for something PipeWire can
configure.

No new audio service was added: PipeWire, pipewire-pulse and WirePlumber are the
ones r8 already enabled.

### What is verified, and what is not

The configuration is verified to load: the running analog sink carries
`node.suspend-on-idle = "false"`, while the rule's match pattern leaves HDMI and
USB sinks alone. What could **not** be verified in QEMU is the consequence: this
QEMU has no usable host audio backend (ALSA fails to open), so its emulated HDA
sink still reports `suspended` whether or not the policy applies. The actual
"sound begins promptly" acceptance criterion needs the Chromebook's codec and
speakers, and is listed as the first hardware check for r12.

## 5. Testing

### Automated

- `cargo test --release`: 35 tests, including the new desktop layout, drag
  collision, name validation, trash encoding, shortcut trust, taskbar pin
  selection, window matching and a desktop render that registers clickable
  icons.
- `scripts/check-config.sh`: all groups pass, including new checks for the two
  taskbar pins, the Alt+Z binding and helper, the dictation dependencies and the
  WirePlumber rule.
- `refurb-dictate transcribe test.wav`, run against the actual shipped script
  and model, returns the expected sentence, so the engine and model path are
  proven before the ISO is built.

### QEMU acceptance

The image was booted from the ISO in QEMU (software emulation, no KVM, no GPU)
and driven over the serial console and QMP. Results are recorded in
`docs/performance-r12.md`; the short version is in the release notes.

### Hardware checks that remain

- Audio start delay on the real codec and speakers.
- Microphone capture and dictation accuracy with the Chromebook's microphone.
- Alt+Z while a Chromebook top-row key is held (keyboard quirks).
- Top-row brightness/volume/mute, real backlight, stuck cursor (r11 items, still
  unverified on hardware).
- Jellyfin playback against a real server.
- Cockpit login from another computer (the live account has no password).

None of these are claimed as verified in this round.

## 6. Files changed

| Area | Files |
|---|---|
| Taskbar | `shell/refurb-shell/src/ui/bar.rs`, `src/apps.rs`, the two `.desktop` files |
| Desktop icons | `shell/refurb-shell/src/desktop.rs` (new), `src/ui/deskicons.rs` (new), `src/ui/mod.rs`, `src/session/mod.rs`, `src/main.rs` |
| Dictation | `distro/.../usr/lib/refurb/refurb-dictate` (new), `usr/bin/refurb-dictate` (symlink), `hooks/normal/0800-dictation.hook.chroot` (new), `etc/xdg/labwc/rc.xml`, `package-lists/refurb-desktop.list.chroot`, `refurb-diag` |
| Jellyfin audio | `etc/wireplumber/wireplumber.conf.d/51-refurb-audio.conf` (new) |
| Checks and docs | `scripts/check-config.sh`, `docs/round-12.md`, `docs/performance-r12.md`, `docs/limitations.md`, `README.md`, `docs/releases.md` |

## 7. Deliberately not done

- No installer work: r11 deferred it, and this round's brief is desktop
  usability. The design is still in `docs/installer-design.md`.
- No desktop-environment components, icon themes, file watchers, tray or
  notification daemon.
- No live typing of partial text: text is committed at each pause and at stop.
- No attempt to make the desktop icons multi-monitor: they belong to the primary
  screen, like the files they show.
