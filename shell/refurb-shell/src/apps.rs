//! Application catalogue.
//!
//! The shell does not own an application database: it reads freedesktop
//! `.desktop` entries, which is what the software manager, the file manager
//! and every installed package already agree on. Scanning happens once at
//! session start and when a window closes; there is no indexer and no watcher.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Stdio;

#[derive(Clone, Debug, Default)]
pub struct App {
    /// Desktop file stem, e.g. `brave-browser`.
    pub id: String,
    pub name: String,
    pub comment: String,
    pub exec: String,
    pub icon: String,
    pub terminal: bool,
    /// `X-Refurb-Web`: a web application opened in Brave (Microsoft 365, etc).
    pub web: Option<String>,
    /// `X-Refurb-Pin`: 1..=9 pins the app to the launcher grid.
    pub pin: Option<u8>,
    /// `X-Refurb-Taskbar`: 1 puts the app on the taskbar even when it is not
    /// running. Only the two daily applications carry it.
    pub taskbar: bool,
    /// `X-Refurb-Group`: `web`, `system` or empty.
    pub group: String,
}

fn dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
        out.push(PathBuf::from(data_home).join("applications"));
    } else if let Some(home) = std::env::var_os("HOME") {
        out.push(PathBuf::from(home).join(".local/share/applications"));
    }
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    for d in data_dirs.split(':') {
        if !d.is_empty() {
            out.push(PathBuf::from(d).join("applications"));
        }
    }
    out
}

/// Read every visible application, first definition wins (user dirs first).
pub fn scan() -> Vec<App> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut apps = Vec::new();
    for dir in dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
            if !seen.insert(id.clone()) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some(app) = parse_desktop(&id, &text) {
                apps.push(app);
            }
        }
    }
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

/// Parse the `[Desktop Entry]` group of a desktop file.
pub fn parse_desktop(id: &str, text: &str) -> Option<App> {
    let mut app = App {
        id: id.to_string(),
        ..Default::default()
    };
    let mut in_entry = false;
    let mut saw_entry = false;
    let mut no_display = false;
    let mut hidden = false;
    let mut kind = String::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            saw_entry |= in_entry;
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        // Ignore localised duplicates such as Name[de]=...; the catalogue uses
        // the untranslated baseline and Brave/localisation handles the rest.
        if key.contains('[') {
            continue;
        }
        let value = value.trim();
        match key {
            "Name" => app.name = value.to_string(),
            "Comment" => app.comment = value.to_string(),
            "Exec" => app.exec = value.to_string(),
            "Icon" => app.icon = value.to_string(),
            "NoDisplay" => no_display = value.eq_ignore_ascii_case("true"),
            "Hidden" => hidden = value.eq_ignore_ascii_case("true"),
            "Terminal" => app.terminal = value.eq_ignore_ascii_case("true"),
            "Type" => kind = value.to_string(),
            "X-Refurb-Web" => app.web = Some(value.to_string()),
            "X-Refurb-Group" => app.group = value.to_string(),
            "X-Refurb-Pin" => app.pin = value.parse().ok(),
            "X-Refurb-Taskbar" => app.taskbar = value.eq_ignore_ascii_case("1") || value.eq_ignore_ascii_case("true"),
            _ => {}
        }
    }

    if !saw_entry || no_display || hidden || app.name.is_empty() {
        return None;
    }
    if !kind.is_empty() && kind != "Application" {
        return None;
    }
    if app.web.is_none() && app.exec.is_empty() {
        return None;
    }
    Some(app)
}

/// Split an `Exec=` line into argv, honouring quotes and dropping field codes.
pub fn split_exec(exec: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut has_token = false;
    for ch in exec.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                has_token = true;
            }
            ' ' | '\t' if !quoted => {
                if has_token {
                    args.push(std::mem::take(&mut cur));
                    has_token = false;
                }
            }
            '\\' if !quoted => {}
            c => {
                cur.push(c);
                has_token = true;
            }
        }
    }
    if has_token {
        args.push(cur);
    }
    args.into_iter()
        .filter(|a| !is_field_code(a))
        .collect()
}

fn is_field_code(arg: &str) -> bool {
    arg.len() == 2 && arg.starts_with('%') && "fFuUdDnNickvm".contains(&arg[1..])
}

impl App {
    pub fn is_web(&self) -> bool {
        self.web.is_some()
    }

    /// Start the application. Web apps open in Brave, terminal apps get a
    /// terminal, everything else is exec'd directly.
    pub fn launch(&self) -> std::io::Result<()> {
        if let Some(url) = &self.web {
            return spawn_detached("brave-browser", &["--new-window", url])
                .or_else(|_| spawn_detached("brave", &["--new-window", url]))
                .or_else(|_| spawn_detached("xdg-open", &[url.as_str()]));
        }
        let argv = split_exec(&self.exec);
        let Some((prog, rest)) = argv.split_first() else {
            return Err(std::io::Error::other("empty Exec"));
        };
        if self.terminal {
            let mut args = vec!["-e".to_string(), prog.clone()];
            args.extend(rest.iter().cloned());
            let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            return spawn_detached("foot", &refs);
        }
        spawn_detached(prog, &rest.iter().map(|s| s.as_str()).collect::<Vec<_>>())
    }

    /// The command a "copy the command" dialog shows.
    pub fn command_line(&self) -> String {
        self.web
            .as_ref()
            .map(|u| format!("brave-browser --new-window {u}"))
            .unwrap_or_else(|| self.exec.clone())
    }
}

fn spawn_detached(prog: &str, args: &[&str]) -> std::io::Result<()> {
    // Every launched application ends up as a child of the shell. Without a
    // wait they pile up as zombies when they close, so the reaper collects them.
    crate::reaper::spawn_detached(
        prog,
        args,
        Stdio::null(),
        Stdio::null(),
        Stdio::null(),
    )
}

/// Relevance of `app` for query `q`; `None` means "no match".
pub fn score(app: &App, q: &str) -> Option<i32> {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return Some(0);
    }
    let name = app.name.to_lowercase();
    let comment = app.comment.to_lowercase();
    if name == q {
        return Some(1000);
    }
    if name.starts_with(&q) {
        return Some(900 - name.len() as i32);
    }
    if name.split(|c: char| c == ' ' || c == '-' || c == '_').any(|w| w.starts_with(&q)) {
        return Some(800 - name.len() as i32);
    }
    if name.contains(&q) {
        return Some(700 - name.len() as i32);
    }
    if app.id.to_lowercase().contains(&q) {
        return Some(500);
    }
    if comment.contains(&q) {
        return Some(300);
    }
    None
}

/// Search results, best first.
pub fn search(apps: &[App], q: &str) -> Vec<usize> {
    let mut hits: Vec<(i32, usize)> = apps
        .iter()
        .enumerate()
        .filter_map(|(i, a)| score(a, q).map(|s| (s, i)))
        .collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| apps[a.1].name.cmp(&apps[b.1].name)));
    hits.into_iter().map(|(_, i)| i).collect()
}

/// The application shortcut file for `id`, wherever it was installed.
pub fn desktop_file(id: &str) -> Option<PathBuf> {
    dirs()
        .into_iter()
        .map(|dir| dir.join(format!("{id}.desktop")))
        .find(|path| path.is_file())
}

/// Whether a window belongs to `app`. The compositor reports the application
/// id the program set, which is the desktop file's stem for every program this
/// image ships (`brave-browser`, `pcmanfm`, ...), or its exec name.
pub fn matches_toplevel(app: &App, app_id: &str) -> bool {
    let id = app_id.trim().to_lowercase();
    if id.is_empty() {
        return false;
    }
    let stem = id.strip_suffix(".desktop").unwrap_or(&id);
    if app.id.to_lowercase() == stem {
        return true;
    }
    split_exec(&app.exec).first().is_some_and(|program| {
        let program = program.to_lowercase();
        let base = PathBuf::from(&program)
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
            .unwrap_or(program);
        base == stem
    })
}

/// Applications pinned to the taskbar, in pin order.
pub fn taskbar_pins(apps: &[App]) -> Vec<usize> {
    let mut pins: Vec<(u8, usize)> = apps
        .iter()
        .enumerate()
        .filter(|(_, app)| app.taskbar)
        .map(|(index, app)| (app.pin.unwrap_or(u8::MAX), index))
        .collect();
    pins.sort();
    pins.into_iter().map(|(_, index)| index).collect()
}

/// Pinned apps in pin order.
pub fn pinned(apps: &[App]) -> Vec<usize> {
    let mut pins: Vec<(u8, usize)> = apps
        .iter()
        .enumerate()
        .filter_map(|(i, a)| a.pin.map(|p| (p, i)))
        .collect();
    pins.sort();
    pins.into_iter().map(|(_, i)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BRAVE: &str = "[Desktop Entry]\nType=Application\nName=Brave\nComment=Private web browser\nExec=brave-browser %U\nIcon=brave-browser\nX-Refurb-Pin=1\nX-Refurb-Taskbar=1\n";

    const M365: &str = "[Desktop Entry]\nType=Application\nName=Microsoft 365\nComment=Word, Excel and PowerPoint on the web\nExec=brave-browser --new-window https://www.office.com\nX-Refurb-Web=https://www.office.com\nX-Refurb-Pin=7\nX-Refurb-Group=web\n";

    const HIDDEN: &str = "[Desktop Entry]\nType=Application\nName=Hide me\nExec=foo\nNoDisplay=true\n";

    #[test]
    fn parses_a_normal_entry() {
        let a = parse_desktop("brave-browser", BRAVE).unwrap();
        assert_eq!(a.name, "Brave");
        assert_eq!(a.pin, Some(1));
        assert!(a.taskbar);
        assert!(!a.is_web());
        assert_eq!(split_exec(&a.exec), vec!["brave-browser"]);
    }

    #[test]
    fn only_taskbar_entries_are_pinned_to_the_bar() {
        let brave = parse_desktop("brave-browser", BRAVE).unwrap();
        let other = parse_desktop("terminal", "[Desktop Entry]\nType=Application\nName=Terminal\nExec=foot\nX-Refurb-Pin=6\n").unwrap();
        let apps = vec![other, brave];
        let pins = taskbar_pins(&apps);
        assert_eq!(pins.len(), 1);
        assert_eq!(apps[pins[0]].name, "Brave");
    }

    #[test]
    fn windows_are_matched_to_their_desktop_entry() {
        let brave = parse_desktop("brave-browser", BRAVE).unwrap();
        assert!(matches_toplevel(&brave, "brave-browser"));
        assert!(matches_toplevel(&brave, "brave-browser.desktop"));
        assert!(!matches_toplevel(&brave, "pcmanfm"));
        assert!(!matches_toplevel(&brave, ""));
        let files = parse_desktop(
            "refurb-files",
            "[Desktop Entry]\nType=Application\nName=Files\nExec=pcmanfm\n",
        )
        .unwrap();
        assert!(matches_toplevel(&files, "pcmanfm"));
    }

    #[test]
    fn parses_a_web_entry_and_drops_no_longer_needed_argv() {
        let a = parse_desktop("m365", M365).unwrap();
        assert!(a.is_web());
        assert_eq!(a.group, "web");
        assert_eq!(a.web.as_deref(), Some("https://www.office.com"));
        let argv = split_exec("sh -c \"echo hi\" %U");
        assert_eq!(argv, vec!["sh", "-c", "echo hi"]);
    }

    #[test]
    fn hides_no_display_entries() {
        assert!(parse_desktop("x", HIDDEN).is_none());
    }

    #[test]
    fn ranks_prefix_above_comment_matches() {
        let mk = |name: &str, comment: &str| App {
            name: name.into(),
            comment: comment.into(),
            ..Default::default()
        };
        let apps = vec![
            mk("Files", "Browse your documents"),
            mk("Software", "Install and remove applications"),
            mk("Web browser helper", "Brave integration"),
        ];
        let hits = search(&apps, "so");
        assert_eq!(apps[hits[0]].name, "Software");
        assert!(score(&mk("Brave", ""), "brave").unwrap() > score(&mk("Brave", ""), "rav").unwrap());
    }
}
