//! Screens: what the compositor says about them, how to arrange them, and how
//! to put an arrangement back when the same monitor is plugged in again.
//!
//! labwc implements `zwlr_output_management_v1`, so `wlr-randr` is the
//! compositor's own view of every output: connector, maker, model, serial,
//! modes, position, scale and whether it is switched on. This module reads that
//! text, turns it into something the interface can drag around, and writes a
//! change back with the same tool. Nothing here runs in the background: a
//! profile is applied when an output arrives (a registry event) and when the
//! user presses Apply, and at no other time.
//!
//! Profiles are keyed by EDID identity rather than by connector name, because
//! the same television may come back on `HDMI-A-1` on one machine and
//! `HDMI-A-2` on another, and a laptop dock renumbers connectors between boots.
//! Identical monitors (same make, model and serial) are told apart by connector
//! name, which is the only information left.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    /// Refresh rate in Hz. 0.0 means `wlr-randr` did not print one.
    pub refresh: f32,
    pub preferred: bool,
    pub current: bool,
}

impl Mode {
    pub fn size_label(&self) -> String {
        format!("{}×{}", self.width, self.height)
    }

    pub fn label(&self) -> String {
        if self.refresh > 0.0 {
            format!("{}×{} · {:.0} Hz", self.width, self.height, self.refresh)
        } else {
            self.size_label()
        }
    }

    /// The string `wlr-randr --mode` accepts for this mode. A refresh is only
    /// included when it is known, so a compositor that prints no rates still
    /// gets a usable size.
    pub fn spec(&self) -> String {
        if self.refresh > 0.0 {
            format!("{}x{}@{:.3}Hz", self.width, self.height, self.refresh)
        } else {
            format!("{}x{}", self.width, self.height)
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Output {
    pub name: String,
    /// `Make:` from wlr-randr, which comes from the monitor's EDID.
    pub make: String,
    pub model: String,
    pub serial: String,
    pub enabled: bool,
    pub position: (i32, i32),
    pub transform: String,
    pub scale: f32,
    /// Physical size in millimetres, used to keep the arrangement diagram in
    /// proportion without pretending every screen is the same shape.
    pub physical: (u32, u32),
    pub modes: Vec<Mode>,
}

impl Output {
    pub fn current_mode(&self) -> Option<&Mode> {
        self.modes.iter().find(|m| m.current)
    }

    /// The size the output currently has, or the preferred size when it is
    /// switched off. Arrangement maths needs a rectangle even for a dark
    /// output, because the user is about to switch it on.
    pub fn size(&self) -> (u32, u32) {
        if let Some(mode) = self.current_mode() {
            return (mode.width, mode.height);
        }
        self.modes
            .iter()
            .find(|m| m.preferred)
            .or_else(|| self.modes.first())
            .map(|m| (m.width, m.height))
            .unwrap_or((1920, 1080))
    }

    pub fn rect(&self) -> (i32, i32, u32, u32) {
        let (w, h) = self.size();
        (self.position.0, self.position.1, w, h)
    }

    /// A stable key for a monitor. EDID's make/model/serial survive a connector
    /// renumbering; where the monitor reports no usable EDID (cheap adapters do
    /// this) the connector name is the fallback, which still recognises the same
    /// screen on the same port.
    pub fn identity(&self) -> String {
        let clean = |s: &str| {
            let s = s.trim();
            !s.is_empty() && !s.eq_ignore_ascii_case("(null)") && !s.eq_ignore_ascii_case("unknown")
        };
        if clean(&self.model) || clean(&self.serial) {
            let make = if clean(&self.make) { self.make.trim() } else { "" };
            let model = if clean(&self.model) { self.model.trim() } else { "" };
            let serial = if clean(&self.serial) { self.serial.trim() } else { "" };
            format!("edid:{make}|{model}|{serial}")
        } else {
            format!("port:{}", self.name)
        }
    }

    /// What a person calls this screen: the model when it is known, otherwise
    /// the connector. Never both when they are the same string.
    pub fn title(&self) -> String {
        let model = self.model.trim();
        if model.is_empty() || model.eq_ignore_ascii_case("(null)") {
            self.name.clone()
        } else if self.make.trim().is_empty()
            || self.make.eq_ignore_ascii_case("(null)")
            || model.to_lowercase().contains(&self.make.trim().to_lowercase())
        {
            format!("{model} ({})", self.name)
        } else {
            format!("{} {model} ({})", self.make.trim(), self.name)
        }
    }

    pub fn resolution_label(&self) -> String {
        match self.current_mode() {
            Some(mode) if self.enabled => mode.size_label(),
            Some(mode) => format!("{} · off", mode.size_label()),
            None => "no mode".to_string(),
        }
    }
}

/// Parse `wlr-randr` output. Tolerant on purpose: fields the compositor does
/// not support are simply absent, and an output is still usable without them.
pub fn parse(text: &str) -> Vec<Output> {
    let mut outputs: Vec<Output> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if !line.starts_with(' ') {
            // "HDMI-A-1 "Goldstar Company Ltd LG TV 0x00000000" (HDMI-A-1)"
            let name = line
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim_matches('"')
                .to_string();
            if name.is_empty() {
                continue;
            }
            outputs.push(Output {
                name,
                enabled: true,
                scale: 1.0,
                transform: "normal".to_string(),
                ..Default::default()
            });
            continue;
        }
        let Some(out) = outputs.last_mut() else {
            continue;
        };
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Make:") {
            out.make = rest.trim().to_string();
        } else if let Some(rest) = trimmed.strip_prefix("Model:") {
            out.model = rest.trim().to_string();
        } else if let Some(rest) = trimmed.strip_prefix("Serial:") {
            out.serial = rest.trim().to_string();
        } else if let Some(rest) = trimmed.strip_prefix("Physical size:") {
            // "340x190 mm"
            let size = rest.trim().trim_end_matches("mm").trim();
            if let Some((w, h)) = size.split_once('x') {
                out.physical = (
                    w.trim().parse().unwrap_or(0),
                    h.trim().parse().unwrap_or(0),
                );
            }
        } else if let Some(rest) = trimmed.strip_prefix("Enabled:") {
            out.enabled = rest.trim().eq_ignore_ascii_case("yes");
        } else if let Some(rest) = trimmed.strip_prefix("Position:") {
            if let Some((x, y)) = split_pair(rest.trim()) {
                out.position = (x, y);
            }
        } else if let Some(rest) = trimmed.strip_prefix("Transform:") {
            out.transform = rest.trim().to_string();
        } else if let Some(rest) = trimmed.strip_prefix("Scale:") {
            if let Ok(scale) = rest.trim().parse::<f32>() {
                out.scale = scale;
            }
        } else if let Some(mode) = parse_mode(trimmed) {
            // The same size can be listed at several refresh rates; keep each,
            // because 60 Hz and 50 Hz are a real choice on a television.
            if !out
                .modes
                .iter()
                .any(|m| m.width == mode.width && m.height == mode.height && (m.refresh - mode.refresh).abs() < 0.01)
            {
                out.modes.push(mode);
            }
        }
    }
    for out in &mut outputs {
        if !out.modes.iter().any(|m| m.current) {
            if let Some(first) = out.modes.first_mut() {
                first.current = out.enabled;
            }
        }
    }
    outputs
}

fn split_pair(text: &str) -> Option<(i32, i32)> {
    let (a, b) = text.split_once(',')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

fn parse_mode(line: &str) -> Option<Mode> {
    // "1920x1080 px, 60.000000 Hz (preferred, current)"
    // A headless or virtual output prints "1280x720 px (current)": no rate, no
    // comma. Both are real modes and both must reach the panel.
    if !line.contains(" px") {
        return None;
    }
    let (size, rest) = line.split_once(" px")?;
    let (w, h) = size.trim().split_once('x')?;
    let width = w.trim().parse().ok()?;
    let height = h.trim().parse().ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    // After " px" the text starts with a comma: ", 60.000000 Hz (current)".
    let rate = rest
        .trim_start_matches([',', ' '])
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_end_matches("Hz");
    let refresh = if rate.starts_with(|c: char| c.is_ascii_digit()) {
        rate.parse::<f32>().unwrap_or(0.0)
    } else {
        0.0
    };
    Some(Mode {
        width,
        height,
        refresh,
        preferred: line.contains("preferred"),
        current: line.contains("current"),
    })
}

// -- applying a layout ------------------------------------------------------

/// One output's desired state. `None` means "leave this field alone", which is
/// what a profile that only remembers position should do to a resolution the
/// user has since chosen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Setup {
    pub name: String,
    pub enabled: bool,
    pub mode: Option<Mode>,
    pub position: Option<(i32, i32)>,
    pub scale: Option<f32>,
    pub transform: Option<String>,
    /// Mirror this output onto another: `wlr-randr --same-as`.
    pub same_as: Option<String>,
}

impl Setup {
    pub fn args(&self) -> Vec<String> {
        let mut args = vec!["--output".to_string(), self.name.clone()];
        if let Some(other) = &self.same_as {
            // Mirroring implies enabled and takes its mode from the source.
            args.push("--same-as".to_string());
            args.push(other.clone());
            return args;
        }
        if self.enabled {
            args.push("--on".to_string());
            if let Some(mode) = &self.mode {
                args.push("--mode".to_string());
                args.push(mode.spec());
            }
            if let Some((x, y)) = self.position {
                args.push("--pos".to_string());
                args.push(format!("{x},{y}"));
            }
            if let Some(scale) = self.scale {
                args.push("--scale".to_string());
                args.push(format!("{scale:.3}"));
            }
            if let Some(transform) = &self.transform {
                args.push("--transform".to_string());
                args.push(transform.clone());
            }
        } else {
            args.push("--off".to_string());
        }
        args
    }
}

/// Apply each setup in order. The caller puts the source of a mirror first.
/// Returns the first error with the output it belongs to, because "could not
/// set mode" without a name is useless in a settings panel.
pub fn apply(setups: &[Setup]) -> Result<(), String> {
    for setup in setups {
        if let Err(problem) = run(setup) {
            // A resolution the monitor has stopped offering must not cost the
            // user the arrangement as well. Retry the same output without its
            // mode, and only report the failure if that also fails.
            if setup.mode.is_some() {
                let plain = Setup {
                    mode: None,
                    ..setup.clone()
                };
                if run(&plain).is_ok() {
                    continue;
                }
            }
            return Err(problem);
        }
    }
    Ok(())
}

fn run(setup: &Setup) -> Result<(), String> {
    let args = setup.args();
    let out = std::process::Command::new("wlr-randr")
        .args(&args)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("{}: {e}", setup.name))?;
    if out.status.success() {
        Ok(())
    } else {
        let message = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if message.is_empty() {
            format!("{}: wlr-randr refused the change", setup.name)
        } else {
            format!("{}: {message}", setup.name)
        })
    }
}

/// Build what Apply sends: the main display first, because a mirror needs its
/// source to exist before the copy. Disabled screens are still included, with
/// `--off`, or switching one off would only last until the next refresh.
pub fn setups(outputs: &[Output], main_name: &str, mirrored: Option<usize>) -> Vec<Setup> {
    let mut order: Vec<usize> = (0..outputs.len()).collect();
    order.sort_by_key(|i| outputs[*i].name != main_name);
    order
        .into_iter()
        .map(|i| {
            let out = &outputs[i];
            let mirror = mirrored == Some(i) && out.name != main_name;
            Setup {
                name: out.name.clone(),
                enabled: out.enabled || mirror,
                mode: out.current_mode().cloned().or_else(|| {
                    out.modes
                        .iter()
                        .find(|m| m.preferred)
                        .cloned()
                        .or_else(|| out.modes.first().cloned())
                }),
                position: Some(out.position),
                scale: Some(if out.scale > 0.0 { out.scale } else { 1.0 }),
                transform: Some(if out.transform.is_empty() {
                    "normal".to_string()
                } else {
                    out.transform.clone()
                }),
                same_as: if mirror {
                    Some(main_name.to_string())
                } else {
                    None
                },
            }
        })
        .collect()
}

// -- arrangements -----------------------------------------------------------

/// Where a newly connected screen goes when there is no saved arrangement: to
/// the right of everything that is already there, aligned at the top. This is
/// the layout people expect from a projector or a second monitor, and it needs
/// no question about it.
pub fn auto_position(others: &[(i32, i32, u32, u32)], _size: (u32, u32)) -> (i32, i32) {
    let right = others
        .iter()
        .map(|(x, _y, w, _)| (*x).max(0) + *w as i32)
        .max();
    match right {
        Some(right) => {
            let top = others.iter().map(|(_, y, _, _)| *y).min().unwrap_or(0);
            (right, top)
        }
        None => (0, 0),
    }
}

/// Snap a dragged rectangle to its neighbours' edges. `threshold` is in the
/// same units as the rectangles (device pixels), so the feel does not change
/// with the zoom of the diagram. A rectangle that overlaps another is still
/// accepted: a mirror pair legitimately has identical coordinates.
pub fn snap_position(
    proposed: (i32, i32),
    size: (u32, u32),
    others: &[(i32, i32, u32, u32)],
    threshold: i32,
) -> (i32, i32) {
    let (w, h) = (size.0 as i32, size.1 as i32);
    let (mut x, mut y) = proposed;
    for (ox, oy, ow, oh) in others {
        let (ox, oy, ow, oh) = (*ox, *oy, *ow as i32, *oh as i32);
        // Left/right edges, then the same edges aligned the other way.
        for candidate in [ox - w, ox + ow, ox, ox + ow - w] {
            if (x - candidate).abs() <= threshold {
                x = candidate;
            }
        }
        for candidate in [oy - h, oy + oh, oy, oy + oh - h] {
            if (y - candidate).abs() <= threshold {
                y = candidate;
            }
        }
    }
    (x, y)
}

// -- saved arrangements -----------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    pub identity: String,
    pub name: String,
    pub mode: Option<Mode>,
    pub position: (i32, i32),
    pub enabled: bool,
    pub scale: f32,
    pub transform: String,
    pub same_as: Option<String>,
}

impl Profile {
    pub fn from_output(out: &Output) -> Profile {
        Profile {
            identity: out.identity(),
            name: out.name.clone(),
            // Only the size and rate are remembered; `preferred`/`current`
            // describe the compositor's list, not a saved choice.
            mode: out.current_mode().map(|m| Mode {
                preferred: false,
                current: false,
                ..m.clone()
            }),
            position: out.position,
            enabled: out.enabled,
            scale: if out.scale > 0.0 { out.scale } else { 1.0 },
            transform: if out.transform.is_empty() {
                "normal".to_string()
            } else {
                out.transform.clone()
            },
            same_as: None,
        }
    }

    /// Turn a profile into a `wlr-randr` setup for an output that is present.
    /// When a mirrored source is not connected, the display is simply switched
    /// on in its own right instead of staying dark. A saved mode the monitor no
    /// longer offers is dropped rather than keeping the whole arrangement from
    /// being applied: a television that changed its idea of 1080p should still
    /// come back in the right place.
    pub fn setup(&self, out: &Output, source_present: bool) -> Setup {
        let mode = self.mode.as_ref().and_then(|saved| {
            out.modes
                .iter()
                .find(|available| {
                    available.width == saved.width
                        && available.height == saved.height
                        && (saved.refresh <= 0.0
                            || (available.refresh - saved.refresh).abs() < 0.5)
                })
                .cloned()
        });
        Setup {
            name: out.name.clone(),
            enabled: if self.same_as.is_some() { source_present } else { self.enabled },
            mode,
            position: Some(self.position),
            scale: Some(self.scale),
            transform: Some(self.transform.clone()),
            same_as: if source_present { self.same_as.clone() } else { None },
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profiles {
    /// Identity of the display that carries the desktop icons.
    pub main: Option<String>,
    pub items: Vec<Profile>,
}

impl Profiles {
    pub fn get(&self, identity: &str) -> Option<&Profile> {
        self.items.iter().find(|p| p.identity == identity)
    }

    pub fn put(&mut self, profile: Profile) {
        match self.items.iter_mut().find(|p| p.identity == profile.identity) {
            Some(existing) => *existing = profile,
            None => self.items.push(profile),
        }
    }

    pub fn remove(&mut self, identity: &str) {
        self.items.retain(|p| p.identity != identity);
        if self.main.as_deref() == Some(identity) {
            self.main = None;
        }
    }

    /// Parse the small tab-separated file this module writes. Anything
    /// unreadable is ignored: a damaged profile must never keep a screen dark
    /// or stop the session from starting.
    pub fn parse(text: &str) -> Profiles {
        let mut profiles = Profiles::default();
        for line in text.lines() {
            let line = line.trim_end();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() >= 2 && fields[0] == "main" {
                profiles.main = Some(fields[1].to_string());
                continue;
            }
            if fields.len() < 8 {
                continue;
            }
            let mode = parse_mode_spec(fields[2]);
            let position = split_pair(fields[3]).unwrap_or((0, 0));
            profiles.items.push(Profile {
                identity: fields[0].to_string(),
                name: fields[1].to_string(),
                mode,
                position,
                enabled: fields[4] != "0",
                scale: fields[5].parse().unwrap_or(1.0),
                transform: if fields[6] == "-" {
                    "normal".to_string()
                } else {
                    fields[6].to_string()
                },
                same_as: if fields[7] == "-" {
                    None
                } else {
                    Some(fields[7].to_string())
                },
            });
        }
        profiles
    }

    pub fn render(&self) -> String {
        let mut text = String::from(
            "# Reawakening OS display arrangements.\n\
             # identity\tconnector\tmode\tposition\tenabled\tscale\ttransform\tmirror-of\n",
        );
        if let Some(main) = &self.main {
            text.push_str(&format!("main\t{main}\n"));
        }
        for p in &self.items {
            text.push_str(&format!(
                "{}\t{}\t{}\t{},{}\t{}\t{:.3}\t{}\t{}\n",
                p.identity,
                p.name,
                p.mode.as_ref().map(Mode::spec).unwrap_or_else(|| "-".to_string()),
                p.position.0,
                p.position.1,
                if p.enabled { 1 } else { 0 },
                p.scale,
                if p.transform.is_empty() { "-" } else { &p.transform },
                p.same_as.as_deref().unwrap_or("-"),
            ));
        }
        text
    }

    pub fn load(path: &Path) -> Profiles {
        std::fs::read_to_string(path)
            .map(|text| Profiles::parse(&text))
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // Write then rename: a session that dies mid-write must not leave a
        // half-written arrangement that the next login tries to apply.
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, self.render())?;
        std::fs::rename(tmp, path)
    }
}

fn parse_mode_spec(text: &str) -> Option<Mode> {
    if text == "-" || text.is_empty() {
        return None;
    }
    let (size, refresh) = match text.split_once('@') {
        Some((size, rate)) => (
            size,
            rate.trim_end_matches("Hz")
                .trim()
                .parse::<f32>()
                .unwrap_or(0.0),
        ),
        None => (text, 0.0),
    };
    let (w, h) = size.split_once('x')?;
    Some(Mode {
        width: w.trim().parse().ok()?,
        height: h.trim().parse().ok()?,
        refresh,
        preferred: false,
        current: false,
    })
}

pub fn profiles_path() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => PathBuf::from(std::env::var_os("HOME")?).join(".config"),
    };
    Some(base.join("reawakening/displays"))
}

/// The smallest scale that still keeps every rectangle on the diagram, and the
/// offset that puts the top-left of the layout at the diagram's top-left.
pub fn diagram(
    rects: &[(i32, i32, u32, u32)],
    width: f32,
    height: f32,
    gap: f32,
) -> (f32, (i32, i32)) {
    if rects.is_empty() {
        return (0.0, (0, 0));
    }
    let left = rects.iter().map(|(x, _, _, _)| *x).min().unwrap_or(0);
    let top = rects.iter().map(|(_, y, _, _)| *y).min().unwrap_or(0);
    let right = rects
        .iter()
        .map(|(x, _, w, _)| x + *w as i32)
        .max()
        .unwrap_or(1);
    let bottom = rects
        .iter()
        .map(|(_, y, _, h)| y + *h as i32)
        .max()
        .unwrap_or(1);
    let scale = ((width - gap * 2.0) / (right - left).max(1) as f32)
        .min((height - gap * 2.0) / (bottom - top).max(1) as f32)
        .max(0.01);
    (scale, (left, top))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
eDP-1 \"BOE 0x095F (eDP-1)\"
  Make: BOE
  Model: 0x095F
  Serial: (null)
  Physical size: 340x190 mm
  Enabled: yes
  Modes:
    1920x1080 px, 60.000000 Hz (preferred, current)
    1680x1050 px, 60.000000 Hz
  Position: 0,0
  Transform: normal
  Scale: 1.000000
HDMI-A-1 \"Goldstar Company Ltd LG TV 0x01010101 (HDMI-A-1)\"
  Make: Goldstar Company Ltd
  Model: LG TV
  Serial: 0x01010101
  Physical size: 1600x900 mm
  Enabled: no
  Modes:
    1920x1080 px, 60.000000 Hz (preferred)
    1920x1080 px, 50.000000 Hz
  Position: 0,0
  Transform: normal
  Scale: 1.000000
";

    #[test]
    fn parses_two_outputs_with_modes_and_state() {
        let outs = parse(SAMPLE);
        assert_eq!(outs.len(), 2);
        assert_eq!(outs[0].name, "eDP-1");
        assert_eq!(outs[0].current_mode().unwrap().width, 1920);
        assert!(outs[0].modes[0].preferred);
        assert_eq!(outs[0].physical, (340, 190));
        assert!(!outs[1].enabled);
        assert_eq!(outs[1].modes.len(), 2, "both refresh rates kept");
        assert_eq!(outs[1].title(), "Goldstar Company Ltd LG TV (HDMI-A-1)");
    }

    #[test]
    fn identity_prefers_edid_and_falls_back_to_the_connector() {
        let outs = parse(SAMPLE);
        assert_eq!(outs[1].identity(), "edid:Goldstar Company Ltd|LG TV|0x01010101");
        let anonymous = parse("HDMI-A-2\n  Enabled: yes\n");
        assert_eq!(anonymous[0].identity(), "port:HDMI-A-2");
    }

    #[test]
    fn apply_puts_the_mirror_source_first_and_switches_off_the_dark_one() {
        let mut outputs = parse(SAMPLE);
        outputs[1].enabled = true;
        let made = setups(&outputs, "eDP-1", Some(1));
        assert_eq!(made[0].name, "eDP-1", "the main display goes first");
        assert_eq!(made[1].same_as.as_deref(), Some("eDP-1"));
        assert!(made[1].enabled);
        let mut dark = outputs.clone();
        dark[1].enabled = false;
        let with_off = setups(&dark, "eDP-1", None);
        assert!(with_off
            .iter()
            .any(|s| s.name == "HDMI-A-1" && s.args().contains(&"--off".to_string())));
    }

    #[test]
    fn a_mode_without_a_refresh_rate_is_still_a_mode() {
        let outs = parse(
            "HEADLESS-1 \"Headless output 1\"\n  Enabled: yes\n  Modes:\n    1280x720 px (current)\n  Position: 0,0\n",
        );
        assert_eq!(outs.len(), 1);
        assert_eq!(outs[0].modes.len(), 1);
        assert_eq!(outs[0].modes[0].size_label(), "1280×720");
        assert_eq!(outs[0].modes[0].refresh, 0.0);
        assert_eq!(outs[0].resolution_label(), "1280×720");
    }

    #[test]
    fn a_saved_mode_the_monitor_no_longer_offers_is_dropped_not_fatal() {
        let out = parse(SAMPLE)[1].clone();
        let profile = Profile {
            identity: out.identity(),
            name: out.name.clone(),
            mode: Some(Mode {
                width: 1024,
                height: 768,
                refresh: 60.0,
                preferred: false,
                current: false,
            }),
            position: (1920, 0),
            enabled: true,
            scale: 1.0,
            transform: "normal".into(),
            same_as: None,
        };
        let setup = profile.setup(&out, false);
        assert!(setup.mode.is_none(), "a stale mode must not be sent");
        assert_eq!(setup.position, Some((1920, 0)));
        let good = Profile {
            mode: Some(out.modes[1].clone()),
            ..profile
        };
        assert!(good.setup(&out, false).mode.is_some());
    }

    #[test]
    fn a_new_screen_goes_to_the_right_and_aligned_at_the_top() {
        let existing = [(0, 0, 1920, 1080)];
        assert_eq!(auto_position(&existing, (1280, 720)), (1920, 0));
        assert_eq!(auto_position(&[], (1280, 720)), (0, 0));
    }

    #[test]
    fn dragging_snaps_to_the_neighbours_edge() {
        let other = [(0, 0, 1920, 1080)];
        // Dropped four pixels short of "to the right of the laptop".
        assert_eq!(snap_position((1917, 9), (1280, 720), &other, 12), (1920, 0));
        // Dropped above it.
        assert_eq!(
            snap_position((5, -714), (1280, 720), &other, 12),
            (0, -720)
        );
        // Far away: left exactly where the pointer put it.
        assert_eq!(snap_position((4000, 3000), (1280, 720), &other, 12), (4000, 3000));
    }

    #[test]
    fn mode_specs_round_trip_through_the_profile_file() {
        // Profiles store the size and rate only; the compositor decides which
        // mode is preferred or current on the next connection.
        let mode = Mode {
            width: 1920,
            height: 1080,
            refresh: 59.94,
            preferred: false,
            current: false,
        };
        let setup = Setup {
            name: "HDMI-A-1".into(),
            enabled: true,
            mode: Some(mode.clone()),
            position: Some((1920, 0)),
            scale: Some(1.0),
            transform: Some("normal".into()),
            same_as: None,
        };
        let args = setup.args();
        assert!(args.contains(&"--mode".to_string()));
        assert!(args.contains(&"1920x1080@59.940Hz".to_string()));
        assert!(args.contains(&"1920,0".to_string()));

        let mut profiles = Profiles::default();
        profiles.put(Profile {
            identity: "edid:BOE|0x095F|".into(),
            name: "eDP-1".into(),
            mode: Some(mode),
            position: (0, 0),
            enabled: true,
            scale: 1.0,
            transform: "normal".into(),
            same_as: None,
        });
        profiles.main = Some("edid:BOE|0x095F|".into());
        let text = profiles.render();
        let back = Profiles::parse(&text);
        assert_eq!(back, profiles);
        assert_eq!(back.get("edid:BOE|0x095F|").unwrap().name, "eDP-1");
        assert_eq!(
            back.get("edid:BOE|0x095F|").unwrap().mode.as_ref().unwrap().refresh,
            59.94
        );
    }

    #[test]
    fn an_unreadable_profile_file_does_not_lose_the_display() {
        let parsed = Profiles::parse("this is not a profile\n\nmain\tedid:x|\n");
        assert!(parsed.items.is_empty());
        assert_eq!(parsed.main.as_deref(), Some("edid:x|"));
    }

    #[test]
    fn diagram_fits_three_screens_into_its_box() {
        let rects = [
            (0, 0, 1920, 1080),
            (1920, 0, 1920, 1080),
            (0, -1080, 1920, 1080),
        ];
        let (scale, origin) = diagram(&rects, 300.0, 200.0, 8.0);
        assert!(scale > 0.0);
        assert_eq!(origin, (0, -1080));
        // The layout is 3840 wide and 2160 tall; both must fit in the box.
        assert!(3840.0 * scale <= 300.0);
        assert!(2160.0 * scale <= 200.0);
    }
}
