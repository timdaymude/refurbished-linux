# Moving from Windows

## What a Windows user will recognise

| They expect | Where it is |
|---|---|
| Start menu | click the mark at the left of the top bar, or press `Super` |
| Search programs | press `Super` and start typing |
| Taskbar with open windows | the window strip at the right of the top bar |
| Alt+Tab | `Alt+Tab` |
| Show desktop | `Super+D` |
| File Explorer | `Files` (PCManFM), or `Super+E` |
| Settings | `Super+,` or the gear at the right of the bar |
| Store | `Software` — Debian's real catalogue, no account needed |
| Task Manager / Services | `Cockpit` in the launcher, from any browser |
| Lock screen | `Super+L` |
| Notifications | none; nothing interrupts you, which is the point |

## Microsoft services

These are web applications, opened in Brave and pinned in the launcher:

| Service | Address |
|---|---|
| Microsoft 365 (Word, Excel, PowerPoint) | https://www.office.com |
| Outlook mail and calendar | https://outlook.office.com |
| OneDrive | https://onedrive.live.com |
| Teams | https://teams.microsoft.com |

Sign-in with a Microsoft account works normally in Brave. For Teams, the web
client is the right choice on this hardware; the desktop client is heavy.

Practical tips for a 2 GB machine:

- Install the Microsoft 365 web apps as Brave *apps* (menu → Cast, save and
  share → Install page as app). They get their own window and no browser chrome.
- Keep one document tab at a time on 2 GB. Two office tabs plus a video call is
  beyond the machine, not beyond the operating system.
- OneDrive files are opened from the browser. There is no sync client, so files
  live in the cloud unless you download them.

## What will not run

Windows programs (`.exe`) do not run natively, and nothing in the default
install pretends otherwise. If a specific program is unavoidable:

- Look for a Linux or web equivalent first; that is almost always the lightest
  answer.
- Wine is in Debian's catalogue and can be installed from `Software`, but it
  brings its own memory and CPU cost and works unevenly. On a 2 GB machine,
  expect it to be disappointing.
- A virtual machine with Windows is not realistic here: the host has nothing to
  spare.

## First hour checklist

1. Wi-Fi: bar gear → Wi-Fi → network → password.
2. Sound: bar speaker icon; check the output device, then play something.
3. Brave: pin nothing, sign in if you want sync; check `brave://gpu` once.
4. Files: plug in the USB stick you keep your documents on.
5. Software: install one thing you actually need and see the flow.
6. Cockpit: open it once so you know where the terminal is if the desktop ever
   misbehaves.
7. Suspend and resume once, before you rely on it.
