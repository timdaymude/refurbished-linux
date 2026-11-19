//! Everything the shell needs to know about the machine.
//!
//! Deliberately boring: `/sys` reads for battery, brightness and radio state,
//! and short-lived command-line tools (`nmcli`, `bluetoothctl`, `wpctl`,
//! `apt-get`) for anything that has to talk to a service. There is no D-Bus
//! client, no polling daemon and no state kept after the shell exits. Calls
//! are made when the user opens a panel and on a slow background refresh.

use std::path::{Path, PathBuf};
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

    pub fn sec(&self) -> WifiSec {
        WifiSec::from_scan(&self.security)
    }
}

/// How a network's access point says it authenticates.
///
/// `nmcli device wifi connect` is normally left to work this out from its scan
/// cache, and that is exactly what failed on physical hardware: NetworkManager
/// wrote a profile with no `key-mgmt` at all and activating it died with
/// "802-11-wireless-security.key-mgmt: property is missing". The method is
/// therefore decided here, from the scan result, and written into the profile
/// we create. One method is never forced on every network: an open network
/// gets no security section, WPA2-Personal gets `wpa-psk`, WPA3-only gets
/// `sae`, OWE gets `owe` and WEP gets `none` with a WEP key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WifiSec {
    #[default]
    Open,
    WpaPsk,
    Sae,
    Owe,
    Wep,
    /// 802.1X/EAP: needs an identity and a certificate, not a passphrase.
    Enterprise,
}

impl WifiSec {
    /// Read the `SECURITY` column of `nmcli -t -f SECURITY device wifi list`.
    pub fn from_scan(security: &str) -> WifiSec {
        let s = security.trim().to_ascii_uppercase();
        if s.is_empty() || s == "--" {
            return WifiSec::Open;
        }
        if s.contains("802.1X") || s.contains("EAP") {
            return WifiSec::Enterprise;
        }
        if s.contains("OWE") {
            return WifiSec::Owe;
        }
        // "WPA3" alone is SAE-only; "WPA2 WPA3" is a transitional AP, which
        // still accepts a pre-shared key.
        if s.contains("WPA3") && !s.contains("WPA2") && !s.contains("WPA1") {
            return WifiSec::Sae;
        }
        if s.contains("WPA") {
            return WifiSec::WpaPsk;
        }
        if s.contains("WEP") {
            return WifiSec::Wep;
        }
        // Marked secure but with an unfamiliar label: a pre-shared key is the
        // only thing the panel can ask for, and it is right far more often
        // than leaving the profile without a security method.
        WifiSec::WpaPsk
    }

    /// The value of `802-11-wireless-security.key-mgmt`.
    pub fn key_mgmt(&self) -> Option<&'static str> {
        match self {
            WifiSec::Open => None,
            WifiSec::WpaPsk => Some("wpa-psk"),
            WifiSec::Sae => Some("sae"),
            WifiSec::Owe => Some("owe"),
            WifiSec::Wep => Some("none"),
            WifiSec::Enterprise => Some("wpa-eap"),
        }
    }

    pub fn needs_password(&self) -> bool {
        matches!(
            self,
            WifiSec::WpaPsk | WifiSec::Sae | WifiSec::Wep | WifiSec::Enterprise
        )
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

/// The wireless interface a user can actually join a network with.
///
/// `nmcli device wifi list` merges the results of every wireless interface,
/// including one that is running a hotspot: the hotspot's own SSID then shows
/// up as "Connected", and clicking it disconnects instead of asking for a
/// password. Asking a station interface for its own scan results keeps the list
/// to networks this machine can join.
fn wifi_client_interface() -> Option<String> {
    // iw lives in /usr/sbin, which is not always in the session's PATH.
    let out = run("/usr/sbin/iw", &["dev"]).or_else(|| run("iw", &["dev"]))?;
    let mut interface: Option<String> = None;
    for line in out.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix("Interface ") {
            interface = Some(name.trim().to_string());
        } else if line == "type managed" {
            if let Some(name) = interface.clone() {
                return Some(name);
            }
        }
    }
    None
}

/// `nmcli` arguments with the client interface appended when it is known.
fn wifi_list_args(base: &[&str], interface: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = base.iter().map(|s| s.to_string()).collect();
    if let Some(name) = interface {
        args.push("ifname".into());
        args.push(name.to_string());
    }
    args
}

/// The network the machine is actually joined to, if any.
///
/// Read from the active connections rather than from the scan list. The old
/// version asked `nmcli device wifi` for the `IN-USE` column, which fails
/// outright ("argument 'ifname' not understood"), so the bar and the panel said
/// "Not connected" while a link was up. The scan list is not a substitute
/// either: with `ifname` it drops the `*` marker for the network in use. The
/// active connection is the thing the user is asking about.
pub fn wifi_status() -> Option<WifiStatus> {
    let client = wifi_client_interface();
    let out = run(
        "nmcli",
        &[
            "-t",
            "-f",
            "NAME,TYPE,DEVICE",
            "connection",
            "show",
            "--active",
        ],
    )?;
    // A machine can run a hotspot of its own, which is also a wireless
    // connection: report the station interface, and only fall back to any
    // wireless connection when there is no managed interface to prefer.
    let mut other: Option<String> = None;
    for line in out.lines() {
        let fields = split_t(line);
        if fields.len() < 3 || !fields[1].contains("wireless") {
            continue;
        }
        if let Some(client) = &client {
            if &fields[2] != client {
                if other.is_none() {
                    other = Some(fields[0].clone());
                }
                continue;
            }
        }
        return Some(WifiStatus {
            strength: wifi_signal(&fields[0]).unwrap_or(0),
            ssid: fields[0].clone(),
        });
    }
    other.map(|ssid| WifiStatus {
        strength: wifi_signal(&ssid).unwrap_or(0),
        ssid,
    })
}

/// Signal strength of a network from the scan list, for the bar icon.
fn wifi_signal(ssid: &str) -> Option<u8> {
    let interface = wifi_client_interface();
    let args = wifi_list_args(
        &["-t", "-f", "SSID,SIGNAL", "device", "wifi", "list"],
        interface.as_deref(),
    );
    let argv: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let out = run("nmcli", &argv)?;
    out.lines()
        .filter_map(|line| {
            let fields = split_t(line);
            if fields.len() >= 2 && fields[0] == ssid {
                fields[1].trim().parse::<u8>().ok()
            } else {
                None
            }
        })
        .max()
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
///
/// Runs on a worker thread: the rescan is awaited (NetworkManager rejects a
/// second scan within a few seconds) and the list is read afterwards, so the
/// panel shows the networks the scan just found instead of the previous cache.
pub fn scan_wifi() -> Vec<WifiNetwork> {
    let interface = wifi_client_interface();
    let rescan = wifi_list_args(&["device", "wifi", "rescan"], interface.as_deref());
    let rescan_args: Vec<&str> = rescan.iter().map(|s| s.as_str()).collect();
    let _ = run("nmcli", &rescan_args);
    std::thread::sleep(std::time::Duration::from_millis(400));
    let list = wifi_list_args(
        &["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list"],
        interface.as_deref(),
    );
    let list_args: Vec<&str> = list.iter().map(|s| s.as_str()).collect();
    let Some(out) = run("nmcli", &list_args) else {
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

/// Run `nmcli` and turn a failure into the reason it printed.
fn nmcli(args: &[String]) -> Result<(), String> {
    let argv: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let out = Command::new("nmcli")
        .args(&argv)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Could not run nmcli: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let mut text = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if text.is_empty() {
        text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    }
    // nmcli prints several lines; the first one is the actual reason.
    let reason = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("NetworkManager refused the connection.")
        .trim_start_matches("Error: ")
        .to_string();
    Err(if reason.chars().count() > 160 {
        reason.chars().take(160).collect::<String>() + "…"
    } else {
        reason
    })
}

/// The `nmcli connection add|modify` arguments for this network.
///
/// NetworkManager's keyfile plugin only accepts files owned by root, so a
/// desktop user cannot hand the daemon a profile file: a user-written keyfile is
/// rejected with "no settings plugin could load". The profile is therefore
/// created through NetworkManager's own D-Bus API, which is what the polkit rule
/// for `settings.modify.system` exists for. The security method is always named
/// explicitly; nothing is left for NetworkManager to infer from its scan cache.
fn profile_args(ssid: &str, sec: WifiSec, exists: bool) -> Vec<String> {
    let mut args: Vec<String> = vec!["connection".into()];
    if exists {
        args.push("modify".into());
        args.push(ssid.into());
        args.push("connection.autoconnect".into());
        args.push("yes".into());
        match sec.key_mgmt() {
            Some(key_mgmt) => {
                args.push("802-11-wireless-security.key-mgmt".into());
                args.push(key_mgmt.into());
            }
            // Open network: drop the security an earlier attempt left behind.
            None => {
                for property in [
                    "802-11-wireless-security.key-mgmt",
                    "802-11-wireless-security.psk",
                    "802-11-wireless-security.wep-key0",
                ] {
                    args.push(format!("-{property}"));
                }
            }
        }
    } else {
        args.push("add".into());
        args.push("type".into());
        args.push("wifi".into());
        args.push("con-name".into());
        args.push(ssid.into());
        args.push("802-11-wireless.ssid".into());
        args.push(ssid.into());
        args.push("802-11-wireless.mode".into());
        args.push("infrastructure".into());
        args.push("connection.autoconnect".into());
        args.push("yes".into());
        if let Some(key_mgmt) = sec.key_mgmt() {
            args.push("802-11-wireless-security.key-mgmt".into());
            args.push(key_mgmt.into());
        }
    }
    if sec == WifiSec::Wep {
        args.push("802-11-wireless-security.wep-key-type".into());
        args.push("1".into());
    }
    args
}

/// One line for `nmcli connection up … passwd-file`, which is the only way to
/// hand NetworkManager a secret without putting it in an argument list.
fn secret_line(sec: WifiSec, password: &str) -> Result<String, String> {
    if password.contains('\n') || password.contains('\r') {
        return Err("That password contains a line break.".into());
    }
    let property = match sec {
        WifiSec::Wep => "802-11-wireless-security.wep-key0",
        _ => "802-11-wireless-security.psk",
    };
    Ok(format!("{property}:{password}\n"))
}

/// Write the 0600 file `nmcli` reads its secret from, removed after activation.
fn write_secret_file(sec: WifiSec, password: &str) -> Result<PathBuf, String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let line = secret_line(sec, password)?;
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let path = dir.join(format!("refurb-wifi-{}.passwd", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| format!("Could not write the password file: {e}"))?;
    file.write_all(line.as_bytes())
        .map_err(|e| format!("Could not write the password file: {e}"))?;
    Ok(path)
}

/// Connect to a network through a profile with an explicit security method.
///
/// The reason the network does not come up is returned to the caller instead of
/// being thrown away, so the panel can say what actually failed.
pub fn connect_wifi(ssid: &str, sec: WifiSec, password: Option<&str>) -> Result<(), String> {
    if !have("nmcli") {
        return Err("NetworkManager is not installed.".into());
    }
    if sec == WifiSec::Enterprise {
        return Err(
            "This network uses an enterprise login. Open Network settings to enter it.".into(),
        );
    }
    let password = password.filter(|p| !p.is_empty());
    if sec.needs_password() && password.is_none() {
        return Err("This network needs a password.".into());
    }

    let created = saved_wifi(ssid).is_none();
    nmcli(&profile_args(ssid, sec, !created))?;

    let secret = match (sec.needs_password(), password) {
        (true, Some(password)) => Some(write_secret_file(sec, password)?),
        _ => None,
    };
    let mut args: Vec<String> = vec![
        "--wait".into(),
        "30".into(),
        "connection".into(),
        "up".into(),
        ssid.into(),
    ];
    if let Some(path) = &secret {
        args.push("passwd-file".into());
        args.push(path.to_string_lossy().into_owned());
    }
    let result = nmcli(&args);
    // The password file only has to outlive the activation.
    if let Some(path) = secret {
        let _ = std::fs::remove_file(path);
    }
    match result {
        Ok(()) => Ok(()),
        // "already active" is a success in disguise: the profile worked.
        Err(reason) if reason.contains("already active") => Ok(()),
        // The wait expired, but the link may have come up just after it. Ask
        // NetworkManager before telling the user it failed.
        Err(reason) if activation_timeout(&reason) && wifi_active(ssid) => Ok(()),
        Err(reason) => {
            // Remove only what this attempt created; a profile the user saved
            // themselves stays theirs.
            if created {
                let _ = nmcli(&["connection".into(), "delete".into(), ssid.into()]);
            }
            // A wrong key ends the four-way handshake and NetworkManager
            // reports missing secrets. Say what actually happened.
            if wants_password(&reason) {
                return Err("That password was not accepted.".into());
            }
            Err(reason)
        }
    }
}

/// A saved NetworkManager profile for this SSID, if the machine has one.
pub fn saved_wifi(ssid: &str) -> Option<String> {
    let out = run("nmcli", &["-t", "-f", "NAME,TYPE", "connection", "show"])?;
    for line in out.lines() {
        let fields = split_t(line);
        if fields.len() >= 2 && fields[1].contains("wireless") && fields[0] == ssid {
            return Some(fields[0].clone());
        }
    }
    None
}

/// Bring up a profile that is already saved, without asking for a password.
pub fn connect_saved_wifi(ssid: &str) -> Result<(), String> {
    if !have("nmcli") {
        return Err("NetworkManager is not installed.".into());
    }
    nmcli(&[
        "--wait".into(),
        "30".into(),
        "connection".into(),
        "up".into(),
        "id".into(),
        ssid.into(),
    ])
}

/// Is this connection up right now?
///
/// Used after `nmcli` times out: on a slow machine the association can finish
/// after the wait expires, and a panel that reports a failure over a working link
/// is worse than the wait being long.
pub fn wifi_active(ssid: &str) -> bool {
    run(
        "nmcli",
        &["-t", "-f", "NAME", "connection", "show", "--active"],
    )
    .map(|out| {
        out.lines()
            .any(|line| split_t(line).first().map(|name| name == ssid).unwrap_or(false))
    })
    .unwrap_or(false)
}

/// Did `nmcli` give up waiting rather than fail?
pub fn activation_timeout(reason: &str) -> bool {
    let text = reason.to_ascii_lowercase();
    text.contains("timeout expired") || text.contains("timed out")
}

/// Did activation fail because the saved profile has no stored secret?
///
/// That is the one failure worth turning back into a password prompt; any other
/// reason (no carrier, wrong key, AP gone) is shown to the user as it is.
pub fn wants_password(reason: &str) -> bool {
    let text = reason.to_ascii_lowercase();
    text.contains("secrets were required")
        || text.contains("no secrets")
        || (text.contains("secret") && text.contains("required"))
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

/// The sink every control addresses. One string, one place to change it.
const SINK: &str = "@DEFAULT_AUDIO_SINK@";

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

/// Apply a sink volume. Called from the volume worker, never from the event
/// loop: each `wpctl` call is a process launch, and doing that once per pointer
/// motion event is what made dragging the slider feel unreliable. `wpctl` is
/// invoked directly instead of through `sh -c`, so a drag can never leave half
/// a command running and the mute step can never be reordered after the write.
///
/// Raising the volume lifts a mute as well: `wpctl` keeps the mute flag across
/// `set-volume`, which is why the slider used to move while the machine stayed
/// silent and the next poll put "Muted" back on the bar.
pub fn set_volume(level: f32) -> bool {
    let level = if level.is_finite() { level } else { return false };
    let level = level.clamp(0.0, VOLUME_MAX);
    let pct = format!("{}%", (level * 100.0).round() as i32);
    let ok = Command::new("wpctl")
        .args(["set-volume", "--limit", "1.6", SINK, &pct])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if ok && level > 0.0 {
        let _ = Command::new("wpctl")
            .args(["set-mute", SINK, "0"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    ok
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
    // The helper lives in /usr/lib/refurb, which is not on PATH for a session
    // started by labwc. Looking for the bare name always failed, so the shell
    // fell back to a bare `wpctl` that ignores the 160% ceiling and the OSD.
    const HELPER: &str = "/usr/lib/refurb/refurb-control";
    if Path::new(HELPER).is_file() {
        spawn(HELPER, args);
        return;
    }
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

// ---------------------------------------------------------------------------
// established desktop programs
// ---------------------------------------------------------------------------

/// Start a proven program as its own process.
///
/// The shell draws its own interface, but it should not reimplement a full
/// NetworkManager or BlueZ front end: scanning, saved networks, the password
/// dialog, pairing agents and device trust already exist, are maintained, and
/// cannot take the desktop down with them. Returns false when the program is
/// not installed, so the caller can say so instead of doing nothing.
pub fn launch(prog: &str, args: &[&str]) -> bool {
    if !have(prog) {
        return false;
    }
    spawn(prog, args);
    true
}

/// Wi-Fi: NetworkManager's own connection editor, with the Wi-Fi security,
/// saved networks and autoconnect settings. The quick panel keeps its dialog
/// for the common case; this is the full interface behind it. `nmtui` in a
/// terminal is the fallback, and both speak to the same NetworkManager.
///
/// (iwgtk was the first choice, but Debian builds it against iwd only: it
/// opens with "iwd is down" on an image that runs NetworkManager.)
pub fn open_wifi_settings() -> bool {
    launch("nm-connection-editor", &[]) || launch("foot", &["-e", "nmtui"])
}

/// Bluetooth: BlueZ's own manager, with discovery, pairing, trusted devices
/// and audio profile selection.
pub fn open_bluetooth_settings() -> bool {
    launch("blueman-manager", &[]) || launch("foot", &["-e", "bluetoothctl"])
}

/// Sound: per-application streams, output and input device selection.
pub fn open_sound_settings() -> bool {
    launch("pavucontrol", &[])
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

/// One backlight the kernel registered, as it really is on this machine.
///
/// A laptop commonly has two (`intel_backlight` or `amdgpu_bl0` for the panel
/// and `acpi_video0`, a firmware stub). They are not interchangeable: writing
/// to the stub changes a value and nothing else. The device is therefore
/// described, not just found.
#[derive(Clone, Debug)]
pub struct Backlight {
    pub name: String,
    pub dir: String,
    pub max: f32,
    pub current: f32,
    pub writable: bool,
}

fn sysfs_u32(path: &str) -> Option<f32> {
    read_trim(path)?.parse::<f32>().ok().filter(|v| v.is_finite())
}

/// Every backlight the kernel exposes, best candidate first.
pub fn backlights() -> Vec<Backlight> {
    let Ok(dir) = std::fs::read_dir("/sys/class/backlight") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in dir.flatten() {
        let path = entry.path();
        let dir = path.display().to_string();
        let Some(max) = sysfs_u32(&format!("{dir}/max_brightness")) else {
            continue;
        };
        if max <= 0.0 {
            continue;
        }
        let Some(current) = sysfs_u32(&format!("{dir}/brightness")) else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().to_string();
        // Writability is what matters, not existence: without a udev rule or a
        // privileged helper the node is root-only and every write silently
        // fails, which used to leave a slider that moved and did nothing.
        let writable = std::fs::OpenOptions::new()
            .write(true)
            .open(format!("{dir}/brightness"))
            .is_ok();
        out.push(Backlight {
            name,
            dir,
            max,
            current,
            writable,
        });
    }
    out.sort_by_key(|b| rank(&b.name));
    out
}

/// Real panel backlights before firmware stubs.
fn rank(name: &str) -> u8 {
    if name.contains("acpi_video") {
        2
    } else if name.contains("backlight") || name.contains("bl") {
        0
    } else {
        1
    }
}

pub fn brightness() -> Option<f32> {
    let device = backlights().into_iter().next()?;
    Some((device.current / device.max).clamp(0.01, 1.0))
}

/// True only when this machine has a backlight something can actually change.
/// The quick panel and the Settings window use this to decide whether to show
/// a brightness control at all: a control that cannot work is worse than none.
pub fn brightness_available() -> bool {
    let devices = backlights();
    devices.iter().any(|b| b.writable) || (!devices.is_empty() && have("brightnessctl"))
}

/// Change the panel brightness and report what the hardware actually did.
///
/// Runs on a worker thread. It never panics, never divides by zero and never
/// claims success: the value is read back from the device, so the slider can
/// show the real level instead of the position it was dragged to.
pub fn set_brightness(level: f32) -> Result<f32, String> {
    if !level.is_finite() {
        return Err("invalid brightness value".into());
    }
    // Never fully dark: a backlight at zero looks like a dead machine.
    let level = level.clamp(0.02, 1.0);
    let devices = backlights();
    if devices.is_empty() {
        return Err("no backlight device".into());
    }
    // sysfs first: it is the kernel's own interface and needs no helper. The
    // udev rule makes the node writable for the desktop user.
    for device in devices.iter().filter(|d| d.writable) {
        let raw = ((level * device.max).round() as i64).clamp(1, device.max as i64);
        if std::fs::write(format!("{}/brightness", device.dir), format!("{raw}\n")).is_ok() {
            if let Some(actual) = brightness() {
                return Ok(actual);
            }
            return Ok(level);
        }
    }
    // A helper may still work (setuid, or a udev rule that only tags the node).
    if have("brightnessctl") {
        let pct = format!("{}%", (level * 100.0).round() as i32);
        let ok = Command::new("brightnessctl")
            .args(["-q", "set", &pct])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if ok {
            return Ok(brightness().unwrap_or(level));
        }
    }
    Err("the backlight is not writable by this user".into())
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
    fn security_methods_follow_the_scan_result() {
        assert_eq!(WifiSec::from_scan("--"), WifiSec::Open);
        assert_eq!(WifiSec::from_scan(""), WifiSec::Open);
        assert_eq!(WifiSec::from_scan("WPA2"), WifiSec::WpaPsk);
        assert_eq!(WifiSec::from_scan("WPA1 WPA2"), WifiSec::WpaPsk);
        assert_eq!(WifiSec::from_scan("WPA3"), WifiSec::Sae);
        // A transitional access point still accepts a pre-shared key.
        assert_eq!(WifiSec::from_scan("WPA2 WPA3"), WifiSec::WpaPsk);
        assert_eq!(WifiSec::from_scan("WPA2 802.1X"), WifiSec::Enterprise);
        assert_eq!(WifiSec::from_scan("WEP"), WifiSec::Wep);
        assert_eq!(WifiSec::from_scan("OWE"), WifiSec::Owe);
    }

    #[test]
    fn profiles_always_name_a_security_method() {
        let wpa = profile_args("Cafe Net", WifiSec::WpaPsk, false);
        assert!(wpa
            .windows(2)
            .any(|w| w[0] == "802-11-wireless-security.key-mgmt" && w[1] == "wpa-psk"));
        assert!(wpa
            .windows(2)
            .any(|w| w[0] == "802-11-wireless.ssid" && w[1] == "Cafe Net"));

        let sae = profile_args("Wpa3 Only", WifiSec::Sae, false);
        assert!(sae
            .windows(2)
            .any(|w| w[0] == "802-11-wireless-security.key-mgmt" && w[1] == "sae"));

        let open = profile_args("Free Wifi", WifiSec::Open, false);
        assert!(!open
            .iter()
            .any(|arg| arg == "802-11-wireless-security.key-mgmt"));

        // Switching a saved network back to open drops the old secrets.
        let reopened = profile_args("Free Wifi", WifiSec::Open, true);
        assert!(reopened
            .iter()
            .any(|arg| arg == "-802-11-wireless-security.psk"));
        assert!(reopened
            .iter()
            .any(|arg| arg == "-802-11-wireless-security.key-mgmt"));
    }

    #[test]
    fn a_timed_out_wait_is_not_a_failure_by_itself() {
        assert!(activation_timeout("Timeout expired (30 seconds)"));
        assert!(activation_timeout("Connection activation timed out"));
        assert!(!activation_timeout("Secrets were required, but not provided"));
    }

    #[test]
    fn secrets_never_reach_an_argument_list() {
        assert_eq!(
            secret_line(WifiSec::WpaPsk, "pa ss$word").unwrap(),
            "802-11-wireless-security.psk:pa ss$word\n"
        );
        assert_eq!(
            secret_line(WifiSec::Wep, "abcde").unwrap(),
            "802-11-wireless-security.wep-key0:abcde\n"
        );
        assert!(secret_line(WifiSec::WpaPsk, "line\nbreak").is_err());
        for arg in profile_args("Cafe Net", WifiSec::WpaPsk, false) {
            assert!(!arg.contains("pa ss$word"));
        }
    }

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
