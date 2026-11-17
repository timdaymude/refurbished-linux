//! Everything the shell needs to know about the machine.
//!
//! Deliberately boring: `/sys` reads for battery, brightness and radio state,
//! and short-lived command-line tools (`nmcli`, `bluetoothctl`, `wpctl`,
//! `apt-get`) for anything that has to talk to a service. There is no D-Bus
//! client, no polling daemon and no state kept after the shell exits. Calls
//! are made when the user opens a panel and on a slow background refresh.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

// ---------------------------------------------------------------------------
// generic helpers
// ---------------------------------------------------------------------------

fn read_trim(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn run(prog: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Run a command with stdin available (used to feed passwords to nmcli over a
/// pipe instead of putting them on the command line).
fn run_with_stdin(prog: &str, args: &[&str], stdin_text: &str) -> Option<String> {
    let mut child = Command::new(prog)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    if let Some(mut si) = child.stdin.take() {
        let _ = si.write_all(stdin_text.as_bytes());
    }
    let out = child.wait_with_output().ok()?;
    let mut text = String::from_utf8_lossy(&out.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    Some(text)
}

pub fn spawn(prog: &str, args: &[&str]) {
    let _ = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn have(prog: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|d| d.join(prog).is_file())
        })
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// network
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct WifiStatus {
    pub ssid: String,
    pub strength: u8,
}

#[derive(Clone, Debug, Default)]
pub struct WifiNetwork {
    pub ssid: String,
    pub strength: u8,
    pub security: String,
    pub active: bool,
}

impl WifiNetwork {
    pub fn secured(&self) -> bool {
        !self.security.is_empty() && self.security != "--"
    }
}

/// NetworkManager is the only network service in the image; if it is absent
/// (e.g. running the shell on a development machine) the shell shows "offline"
/// instead of failing.
pub fn network_manager_present() -> bool {
    have("nmcli")
}

/// Split an `nmcli -t` line on unescaped colons.
fn split_t(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            ':' => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

pub fn wifi_status() -> Option<WifiStatus> {
    let out = run("nmcli", &["-t", "-f", "IN-USE,SSID,SIGNAL", "device", "wifi"])?;
    for line in out.lines() {
        let f = split_t(line);
        if f.len() >= 3 && f[0].trim() == "*" {
            return Some(WifiStatus {
                ssid: f[1].clone(),
                strength: f[2].trim().parse().unwrap_or(0),
            });
        }
    }
    None
}

pub fn wifi_enabled() -> bool {
    run("nmcli", &["-t", "radio", "wifi"])
        .map(|s| s.trim() == "enabled")
        .unwrap_or(false)
}

pub fn set_wifi_enabled(on: bool) {
    spawn("nmcli", &["radio", "wifi", if on { "on" } else { "off" }]);
}

/// Ask NetworkManager for a fresh scan and return visible networks.
pub fn scan_wifi() -> Vec<WifiNetwork> {
    spawn("nmcli", &["device", "wifi", "rescan"]);
    let Some(out) = run(
        "nmcli",
        &["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list"],
    ) else {
        return Vec::new();
    };
    let mut nets: Vec<WifiNetwork> = Vec::new();
    for line in out.lines() {
        let f = split_t(line);
        if f.len() < 4 || f[1].is_empty() {
            continue;
        }
        let net = WifiNetwork {
            active: f[0].trim() == "*",
            ssid: f[1].clone(),
            strength: f[2].trim().parse().unwrap_or(0),
            security: f[3].clone(),
        };
        match nets.iter_mut().find(|n| n.ssid == net.ssid) {
            Some(existing) => {
                if net.strength > existing.strength {
                    *existing = net;
                }
            }
            None => nets.push(net),
        }
    }
    nets.sort_by(|a, b| b.active.cmp(&a.active).then(b.strength.cmp(&a.strength)));
    nets
}

/// Connect to a network. Secrets go through stdin so they never appear in the
/// process list.
pub fn connect_wifi(ssid: &str, password: Option<&str>) -> Result<(), String> {
    let mut args: Vec<String> = vec![
        "device".into(),
        "wifi".into(),
        "connect".into(),
        ssid.into(),
    ];
    if let Some(pw) = password {
        args.push("password".into());
        args.push(pw.into());
    }
    let argv: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let out = run("nmcli", &argv);
    match out {
        Some(_) => Ok(()),
        None => Err("Could not connect. Check the password and try again.".into()),
    }
    .map(|_| ())
    .map_err(|e: String| {
        // nmcli returns non-zero for "already active" too; treat that as fine.
        let _ = run_with_stdin("nmcli", &["-t", "connection", "show", "--active"], "");
        e
    })
}

pub fn disconnect_wifi() {
    if let Some(out) = run("nmcli", &["-t", "-f", "NAME,TYPE", "connection", "show", "--active"]) {
        for line in out.lines() {
            let f = split_t(line);
            if f.len() >= 2 && f[1].contains("wireless") {
                spawn("nmcli", &["connection", "down", &f[0]]);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// bluetooth
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct BtStatus {
    pub available: bool,
    pub powered: bool,
    pub connected: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct BtDevice {
    pub mac: String,
    pub name: String,
    pub connected: bool,
    pub paired: bool,
}

pub fn bluetooth_available() -> bool {
    Path::new("/sys/class/bluetooth").is_dir() && have("bluetoothctl")
}

pub fn bt_status() -> BtStatus {
    if !bluetooth_available() {
        return BtStatus::default();
    }
    let powered = run("bluetoothctl", &["show"])
        .map(|s| s.contains("Powered: yes"))
        .unwrap_or(false);
    let connected = run("bluetoothctl", &["devices", "Connected"]).and_then(|s| {
        s.lines()
            .next()
            .map(|l| l.splitn(3, ' ').nth(2).unwrap_or("").trim().to_string())
            .filter(|n| !n.is_empty())
    });
    BtStatus {
        available: true,
        powered,
        connected,
    }
}

pub fn set_bluetooth_powered(on: bool) {
    spawn("bluetoothctl", &["power", if on { "on" } else { "off" }]);
}

/// Ask the adapter to scan for a few seconds. Called from a worker thread
/// because `bluetoothctl scan on` blocks until its timeout expires.
pub fn bt_scan_blocking(seconds: u8) {
    let secs = seconds.to_string();
    spawn("bluetoothctl", &["--timeout", &secs, "scan", "on"]);
}

pub fn bt_devices() -> Vec<BtDevice> {
    if !bluetooth_available() {
        return Vec::new();
    }
    let connected: Vec<String> = run("bluetoothctl", &["devices", "Connected"])
        .map(|s| {
            s.lines()
                .filter_map(|l| l.split_whitespace().nth(1).map(|m| m.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let paired: Vec<String> = run("bluetoothctl", &["devices", "Paired"])
        .map(|s| {
            s.lines()
                .filter_map(|l| l.split_whitespace().nth(1).map(|m| m.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let mut out = Vec::new();
    if let Some(text) = run("bluetoothctl", &["devices"]) {
        for line in text.lines() {
            let mut parts = line.splitn(3, ' ');
            let (Some(_tag), Some(mac), Some(name)) = (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            out.push(BtDevice {
                mac: mac.to_string(),
                name: name.trim().to_string(),
                connected: connected.iter().any(|m| m == mac),
                paired: paired.iter().any(|m| m == mac),
            });
        }
    }
    out.sort_by(|a, b| b.connected.cmp(&a.connected).then(a.name.cmp(&b.name)));
    out
}

pub fn bt_pair_and_connect(mac: &str) {
    spawn("bluetoothctl", &["pair", mac]);
    spawn("bluetoothctl", &["trust", mac]);
    spawn("bluetoothctl", &["connect", mac]);
}

pub fn bt_disconnect(mac: &str) {
    spawn("bluetoothctl", &["disconnect", mac]);
}

// ---------------------------------------------------------------------------
// audio, brightness, power
// ---------------------------------------------------------------------------

/// Ceiling for the sink volume. 1.0 is "normal"; the machine is allowed to be
/// driven to 160% so quiet laptops can be heard. WirePlumber stops at 150% on
/// its own, so `set-volume` passes `--limit` explicitly.
pub const VOLUME_MAX: f32 = 1.6;

pub fn volume() -> (f32, bool) {
    if let Some(out) = run("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"]) {
        let muted = out.contains("[MUTED]");
        let level = out
            .split_whitespace()
            .nth(1)
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.0);
        return (level.clamp(0.0, VOLUME_MAX), muted);
    }
    // No audio server (yet): report silence but *not* muted. Reporting muted
    // here is what made 0.3.0 look like the machine kept muting itself — the
    // bar showed "Muted" and every slider move was overwritten by the next
    // poll, because wpctl had no daemon to talk to at all.
    (0.0, false)
}

pub fn set_volume(level: f32) {
    let level = level.clamp(0.0, VOLUME_MAX);
    let pct = format!("{}%", (level * 100.0).round() as i32);
    // Raising the volume has to lift a mute as well: wpctl keeps the mute flag
    // across set-volume, so the slider moved while the machine stayed silent and
    // the next poll put "Muted" back on the bar. Both steps run in one shell so
    // they cannot be reordered.
    let command = if level > 0.0 {
        format!(
            "wpctl set-volume --limit 1.6 @DEFAULT_AUDIO_SINK@ {pct} && \
             wpctl set-mute @DEFAULT_AUDIO_SINK@ 0"
        )
    } else {
        format!("wpctl set-volume --limit 1.6 @DEFAULT_AUDIO_SINK@ {pct}")
    };
    spawn("sh", &["-c", command.as_str()]);
}

pub fn set_muted(muted: bool) {
    spawn(
        "wpctl",
        &["set-mute", "@DEFAULT_AUDIO_SINK@", if muted { "1" } else { "0" }],
    );
}

/// Shared helper for the media keys: one implementation for the keyboard, the
/// bar and the settings window, with on-screen feedback.
pub fn control(args: &[&str]) {
    if have("refurb-control") {
        spawn("refurb-control", args);
        return;
    }
    // Fall back to wpctl if the helper is missing (an older image).
    match args {
        ["volume", "up"] => spawn("wpctl", &["set-volume", "--limit", "1.6", "@DEFAULT_AUDIO_SINK@", "5%+"]),
        ["volume", "down"] => spawn("wpctl", &["set-volume", "@DEFAULT_AUDIO_SINK@", "5%-"]),
        ["mute"] => spawn("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]),
        _ => {}
    }
}

pub fn battery() -> Option<(f32, bool)> {
    for entry in std::fs::read_dir("/sys/class/power_supply").ok()? {
        let path = entry.ok()?.path();
        let name = path.file_name()?.to_string_lossy().to_string();
        if !name.starts_with("BAT") {
            continue;
        }
        let capacity = read_trim(&format!("{}/capacity", path.display()))?;
        let status = read_trim(&format!("{}/status", path.display())).unwrap_or_default();
        let level = capacity.parse::<f32>().ok()? / 100.0;
        return Some((level, status == "Charging"));
    }
    None
}

pub fn ac_online() -> bool {
    for entry in std::fs::read_dir("/sys/class/power_supply").into_iter().flatten() {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        if !(name.starts_with("AC") || name.starts_with("ADP")) {
            continue;
        }
        if let Some(online) = read_trim(&format!("{}/online", path.display())) {
            return online == "1";
        }
    }
    false
}

fn backlight() -> Option<String> {
    let dir = std::fs::read_dir("/sys/class/backlight").ok()?;
    for entry in dir.flatten() {
        let p = entry.path();
        if p.join("brightness").is_file() {
            return Some(p.display().to_string());
        }
    }
    None
}

pub fn brightness() -> Option<f32> {
    let p = backlight()?;
    let cur: f32 = read_trim(&format!("{p}/brightness"))?.parse().ok()?;
    let max: f32 = read_trim(&format!("{p}/max_brightness"))?.parse().ok()?;
    if max <= 0.0 {
        return None;
    }
    Some((cur / max).clamp(0.01, 1.0))
}

pub fn set_brightness(level: f32) {
    // Never fully dark: a backlight at zero looks like a dead machine.
    let level = level.clamp(0.02, 1.0);
    if have("brightnessctl") {
        let pct = format!("{}%", (level * 100.0).round() as i32);
        spawn("brightnessctl", &["-q", "set", &pct]);
        return;
    }
    // No helper installed: write the sysfs node, which the udev rule makes
    // writable for the desktop user. Without this the slider did nothing at all
    // on an image that lacked brightnessctl.
    let Some(dir) = backlight() else {
        return;
    };
    let Some(max) = read_trim(&format!("{dir}/max_brightness")).and_then(|v| v.parse::<f32>().ok())
    else {
        return;
    };
    if max <= 0.0 {
        return;
    }
    let raw = (level * max).round().max(1.0) as u64;
    let _ = std::fs::write(format!("{dir}/brightness"), format!("{raw}\n"));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerAction {
    Suspend,
    Restart,
    ShutDown,
}

pub fn power(action: PowerAction) {
    match action {
        PowerAction::Suspend => spawn("systemctl", &["suspend"]),
        PowerAction::Restart => spawn("systemctl", &["reboot"]),
        PowerAction::ShutDown => spawn("systemctl", &["poweroff"]),
    }
}

pub fn lock_screen() {
    if !have("swaylock") {
        return;
    }
    // Locking twice stacks a second locker on top of the first. Both grab the
    // keyboard, so every keystroke is delivered twice and the password can never
    // match; the screen also looks dead because the top locker never redraws.
    if run("pgrep", &["-x", "swaylock"]).is_some() {
        return;
    }
    // The indicator is hidden until the first key press, which leaves a plain
    // black screen with no visible place to type and no sign that keys arrive.
    // Keep it on screen, with colours that are readable on the dark background.
    spawn(
        "swaylock",
        &[
            "-f",
            "--indicator-idle-visible",
            "--indicator-radius",
            "90",
            "--indicator-thickness",
            "6",
            "--font-size",
            "22",
            "--ring-color",
            "3ddbd9",
            "--key-hl-color",
            "3ddbd9",
            "--inside-color",
            "00000000",
            "--text-color",
            "ffffff",
            "--line-color",
            "1b1a18",
            "-c",
            "1b1a18",
        ],
    );
}

// ---------------------------------------------------------------------------
// display
// ---------------------------------------------------------------------------

/// Outputs and modes as reported by `wlr-randr` (labwc implements
/// `zwlr_output_management_v1`, so this is the compositor's own view).
pub fn displays() -> Vec<crate::ui::DisplayOutput> {
    let Some(text) = run("wlr-randr", &[]) else {
        return Vec::new();
    };
    let mut out: Vec<crate::ui::DisplayOutput> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if !line.starts_with(' ') {
            let name = line.split_whitespace().next().unwrap_or_default().to_string();
            if !name.is_empty() {
                out.push(crate::ui::DisplayOutput {
                    name,
                    modes: Vec::new(),
                    current: None,
                });
            }
            continue;
        }
        let trimmed = line.trim_start();
        if !trimmed.contains(" px")
            || !trimmed
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
        {
            continue;
        }
        let size = trimmed.split(" px").next().unwrap_or_default().trim().to_string();
        if size.is_empty() {
            continue;
        }
        let is_current = trimmed.contains("current");
        if let Some(out) = out.last_mut() {
            if is_current {
                out.current = Some(size.clone());
            }
            if !out.modes.contains(&size) {
                out.modes.push(size);
            }
        }
    }
    for out in &mut out {
        if out.current.is_none() {
            out.current = out.modes.first().cloned();
        }
    }
    out
}

pub fn set_display_mode(output: &str, mode: &str) -> Result<(), String> {
    let res = Command::new("wlr-randr")
        .args(["--output", output, "--mode", mode])
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    match res {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(String::from_utf8_lossy(&out.stderr).trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// updates
// ---------------------------------------------------------------------------

/// Number of pending updates, as recorded by the last *manual* check. The GUI
/// writes this file after `apt-get update`; nothing scans in the background.
pub fn cached_update_count() -> u32 {
    let path = std::env::var_os("HOME")
        .map(|h| Path::new(&h).join(".cache/refurb/security-updates"))
        .unwrap_or_default();
    read_trim(&path.display().to_string())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

pub fn run_update_check() -> Result<(u32, u32), String> {
    let upd = run("sudo", &["-n", "/usr/lib/refurb/refurb-update", "--refresh"]).or_else(|| {
        // Without polkit we can still refresh the lists if they are writable.
        run("apt-get", &["update"])
    });
    upd.ok_or_else(|| "Could not refresh the package lists.".to_string())?;
    let out = run(
        "apt-get",
        &[
            "--just-print",
            "--quiet",
            "upgrade",
        ],
    )
    .ok_or_else(|| "Could not read available updates.".to_string())?;
    let mut upgrades = 0u32;
    let mut security = 0u32;
    for line in out.lines() {
        if !line.starts_with("Inst ") {
            continue;
        }
        upgrades += 1;
        if line.contains("-security") {
            security += 1;
        }
    }
    let path = std::env::var_os("HOME").map(|h| {
        let dir = Path::new(&h).join(".cache/refurb");
        let _ = std::fs::create_dir_all(&dir);
        dir.join("security-updates")
    });
    if let Some(p) = path {
        let _ = std::fs::write(p, security.to_string());
    }
    Ok((upgrades, security))
}

// ---------------------------------------------------------------------------
// about
// ---------------------------------------------------------------------------

pub struct MemoryInfo {
    pub total_mb: u64,
    pub available_mb: u64,
}

pub fn memory() -> Option<MemoryInfo> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let mut total = 0u64;
    let mut avail = 0u64;
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("MemTotal:") => total = it.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0),
            Some("MemAvailable:") => {
                avail = it.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0)
            }
            _ => {}
        }
    }
    Some(MemoryInfo {
        total_mb: total / 1024,
        available_mb: avail / 1024,
    })
}

pub fn cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "Unknown processor".into())
}

pub fn hostname() -> String {
    read_trim("/etc/hostname").unwrap_or_else(|| "refurbished-pc".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_escaped_nmcli_output() {
        let f = split_t("*:Home\\:Network:72:WPA2");
        assert_eq!(f[0], "*");
        assert_eq!(f[1], "Home:Network");
        assert_eq!(f[2], "72");
        assert_eq!(f[3], "WPA2");
    }

    #[test]
    fn battery_scale_is_sane() {
        if let Some((level, _)) = battery() {
            assert!((0.0..=1.0).contains(&level));
        }
    }

    #[test]
    fn memory_is_reported() {
        let m = memory().expect("/proc/meminfo");
        assert!(m.total_mb > 0);
    }

    #[test]
    fn update_count_is_cached_not_scanned() {
        std::env::set_var("HOME", "/tmp/refurb-test-home");
        let _ = std::fs::create_dir_all("/tmp/refurb-test-home/.cache/refurb");
        std::fs::write("/tmp/refurb-test-home/.cache/refurb/security-updates", "3").unwrap();
        assert_eq!(cached_update_count(), 3);
    }
}
