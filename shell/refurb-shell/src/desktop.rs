//! Desktop icons.
//!
//! Everything a Windows user expects to be able to put on the desktop lives in
//! `~/Desktop`, the directory the file manager and every save dialog already
//! use. This module reads that directory, remembers where each icon was left,
//! and performs the file operations behind the icon menu. It draws nothing:
//! that is `ui::deskicons`.
//!
//! There is no watcher and no indexer. The session asks `refresh` once every two
//! seconds; it compares a cheap signature (name, size, modification time) and
//! rescans only when something actually changed, which is what makes a USB
//! stick's files appear without polling the disk in a loop.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use crate::apps::{self, App as DesktopApp};

/// Grid geometry. The cell is wide enough for a label at 12 px and tall enough
/// for the icon plus two short lines; five rows fit a 600-pixel-tall screen
/// above the taskbar, and wider screens simply leave more empty space beside
/// the column instead of hiding icons off the bottom.
pub const CELL_W: f32 = 90.0;
pub const CELL_H: f32 = 88.0;
pub const ORIGIN_X: f32 = 20.0;
pub const ORIGIN_Y: f32 = 16.0;
pub const ROWS: i32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Folder,
    /// A `.desktop` file: an application shortcut.
    App,
    File,
}

#[derive(Clone, Debug)]
pub struct Icon {
    /// What the label says. For a shortcut this is the application's name.
    pub name: String,
    /// The file's own name, which is what rename and delete operate on.
    pub file: String,
    pub path: PathBuf,
    pub kind: Kind,
    /// Parsed shortcut, if this icon is one.
    pub app: Option<DesktopApp>,
    pub col: i32,
    pub row: i32,
}

#[derive(Clone, Debug)]
pub struct Menu {
    /// The icon the menu was opened on; `None` for the desktop itself.
    pub icon: Option<usize>,
    /// True while the "Add shortcut" list of applications is showing.
    pub shortcuts: bool,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug)]
pub struct Press {
    pub icon: usize,
    pub at: (f64, f64),
}

#[derive(Clone, Debug)]
pub struct State {
    pub icons: Vec<Icon>,
    pub selected: Option<usize>,
    pub menu: Option<Menu>,
    /// `(icon, text)` while a name is being typed.
    pub renaming: Option<(usize, String)>,
    /// A short message shown on the desktop ("Moved to Trash", an error).
    pub notice: Option<(String, Instant)>,
    pub press: Option<Press>,
    pub signature: u64,
    pub last_scan: Instant,
    /// Last click, for double-click detection.
    pub last_click: Option<(usize, Instant)>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            icons: Vec::new(),
            selected: None,
            menu: None,
            renaming: None,
            notice: None,
            press: None,
            signature: 0,
            last_scan: Instant::now() - Duration::from_secs(10),
            last_click: None,
        }
    }
}

impl State {
    pub fn say(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), Instant::now()));
    }

    /// Forget a message whose five seconds are up. Returns true if the desktop
    /// has to be repainted.
    pub fn expire_notice(&mut self) -> bool {
        match &self.notice {
            Some((_, at)) if at.elapsed() > Duration::from_secs(5) => {
                self.notice = None;
                true
            }
            _ => false,
        }
    }
}

pub fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// The desktop directory. `~/Desktop` is deliberately hard-coded rather than
/// read from `user-dirs.dirs`: this image creates it, documents it and shows it
/// in the file manager under that name, so the two can never disagree.
pub fn dir() -> PathBuf {
    home()
        .map(|home| home.join("Desktop"))
        .unwrap_or_else(|| PathBuf::from("Desktop"))
}

pub fn ensure_dir() {
    let _ = std::fs::create_dir_all(dir());
}

fn layout_path() -> PathBuf {
    home()
        .map(|home| home.join(".config/refurb/desktop-layout"))
        .unwrap_or_else(|| PathBuf::from("desktop-layout"))
}

// ---------------------------------------------------------------------------
// arrangement
// ---------------------------------------------------------------------------

pub fn cell_origin(col: i32, row: i32) -> (f32, f32) {
    (
        ORIGIN_X + col as f32 * CELL_W,
        ORIGIN_Y + row as f32 * CELL_H,
    )
}

pub fn cell_at(x: f32, y: f32) -> (i32, i32) {
    (
        ((x - ORIGIN_X) / CELL_W).floor() as i32,
        ((y - ORIGIN_Y) / CELL_H).floor() as i32,
    )
}

/// The cell nearest `(col,row)` that no other icon occupies, clamped into the
/// visible rows so an icon can never be dropped off the bottom of the screen.
pub fn free_cell(icons: &[Icon], col: i32, row: i32, skip: usize) -> Option<(i32, i32)> {
    let mut c = col.max(0);
    let mut r = row.clamp(0, ROWS - 1);
    for _ in 0..500 {
        let taken = icons
            .iter()
            .enumerate()
            .any(|(i, icon)| i != skip && icon.col == c && icon.row == r);
        if !taken {
            return Some((c, r));
        }
        r += 1;
        if r >= ROWS {
            r = 0;
            c += 1;
        }
    }
    None
}

fn first_free(used: &HashSet<(i32, i32)>) -> (i32, i32) {
    for col in 0..200 {
        for row in 0..ROWS {
            if !used.contains(&(col, row)) {
                return (col, row);
            }
        }
    }
    (0, 0)
}

pub fn parse_layout(text: &str) -> HashMap<String, (i32, i32)> {
    let mut map = HashMap::new();
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split('\t');
        let (Some(name), Some(col), Some(row)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        if let (Ok(col), Ok(row)) = (col.parse::<i32>(), row.parse::<i32>()) {
            if col >= 0 && row >= 0 && row < ROWS {
                map.insert(name.to_string(), (col, row));
            }
        }
    }
    map
}

pub fn layout_text(icons: &[Icon]) -> String {
    let mut out = String::from("# Reawakening OS desktop layout: name<TAB>column<TAB>row\n");
    for icon in icons {
        out.push_str(&format!("{}\t{}\t{}\n", icon.file, icon.col, icon.row));
    }
    out
}

pub fn save_layout(icons: &[Icon]) {
    let path = layout_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, layout_text(icons));
}

/// Move a saved arrangement from an old file name to a new one, so renaming an
/// icon does not also move it to the first free cell.
fn move_layout(old: &str, new: &str) {
    let path = layout_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let mut map = parse_layout(&text);
    if let Some(cell) = map.remove(old) {
        map.insert(new.to_string(), cell);
    }
    let mut lines: Vec<(String, (i32, i32))> = map.into_iter().collect();
    lines.sort_by(|a, b| a.1.cmp(&b.1));
    let mut out = String::from("# Reawakening OS desktop layout: name<TAB>column<TAB>row\n");
    for (name, (col, row)) in lines {
        out.push_str(&format!("{name}\t{col}\t{row}\n"));
    }
    let _ = std::fs::write(path, out);
}

// ---------------------------------------------------------------------------
// scanning
// ---------------------------------------------------------------------------

/// Hash of everything that would change what the desktop shows. A directory
/// read with a few `stat` calls, twice a second at most; no inotify dependency
/// and no index.
pub fn signature() -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut rows: Vec<(String, u64, u64)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            rows.push((name, meta.len(), mtime));
        }
    }
    rows.sort();
    let mut hasher = DefaultHasher::new();
    rows.hash(&mut hasher);
    hasher.finish()
}

pub fn scan() -> Vec<Icon> {
    let saved = std::fs::read_to_string(layout_path())
        .map(|text| parse_layout(&text))
        .unwrap_or_default();
    let mut icons: Vec<Icon> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir()) {
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().to_string();
            if file.starts_with('.') {
                continue;
            }
            let path = entry.path();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let app = if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
                std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|text| {
                        let stem = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or(&file);
                        apps::parse_desktop(stem, &text)
                    })
            } else {
                None
            };
            let kind = if meta.is_dir() {
                Kind::Folder
            } else if app.is_some() {
                Kind::App
            } else {
                Kind::File
            };
            let name = app
                .as_ref()
                .map(|app| app.name.clone())
                .unwrap_or_else(|| file.clone());
            icons.push(Icon {
                name,
                file,
                path,
                kind,
                app,
                col: 0,
                row: 0,
            });
        }
    }
    // Folders first, then by name, which is how every file manager presents a
    // mixed list and what makes the new icon appear where a person looks.
    icons.sort_by(|a, b| {
        let folder_a = (a.kind != Kind::Folder) as u8;
        let folder_b = (b.kind != Kind::Folder) as u8;
        folder_a
            .cmp(&folder_b)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    let mut used: HashSet<(i32, i32)> = HashSet::new();
    let mut chosen: Vec<Option<(i32, i32)>> = vec![None; icons.len()];
    for (i, icon) in icons.iter().enumerate() {
        if let Some(&(col, row)) = saved.get(&icon.file) {
            if used.insert((col, row)) {
                chosen[i] = Some((col, row));
            }
        }
    }
    for cell in chosen.iter_mut() {
        if cell.is_none() {
            let at = first_free(&used);
            used.insert(at);
            *cell = Some(at);
        }
    }
    for (icon, cell) in icons.iter_mut().zip(chosen) {
        if let Some((col, row)) = cell {
            icon.col = col;
            icon.row = row;
        }
    }
    icons
}

/// Rescan when the directory changed. Returns true when the icons were
/// replaced, so the caller knows to repaint.
pub fn refresh(st: &mut State, force: bool) -> bool {
    let now = Instant::now();
    if !force && now.duration_since(st.last_scan) < Duration::from_millis(2000) {
        return false;
    }
    st.last_scan = now;
    let signature = signature();
    if !force && signature == st.signature {
        return false;
    }
    st.signature = signature;
    let selected = st.selected.and_then(|i| st.icons.get(i)).map(|i| i.file.clone());
    st.icons = scan();
    st.selected = selected.and_then(|file| st.icons.iter().position(|icon| icon.file == file));
    // Indices in an open menu or rename box point into the old list.
    st.menu = None;
    st.renaming = None;
    st.press = None;
    true
}

// ---------------------------------------------------------------------------
// operations
// ---------------------------------------------------------------------------

fn program(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).is_file()))
        .unwrap_or(false)
}

pub fn open(icon: &Icon) -> Result<(), String> {
    match icon.kind {
        Kind::App => {
            if !trusted_shortcut(&icon.path) {
                return Err(format!(
                    "“{}” is not a trusted shortcut. A desktop shortcut must belong to you, be writable only by you, and be marked executable.",
                    icon.name
                ));
            }
            let app = icon
                .app
                .clone()
                .ok_or_else(|| "This shortcut has no launch command.".to_string())?;
            app.launch()
                .map_err(|e| format!("Could not start {}: {e}", app.name))
        }
        Kind::Folder => {
            let path = icon.path.to_string_lossy().to_string();
            crate::system::spawn("pcmanfm", &[&path]);
            Ok(())
        }
        Kind::File => {
            if !program("xdg-open") {
                return Err("No program is installed to open this file.".into());
            }
            let path = icon.path.to_string_lossy().to_string();
            crate::system::spawn("xdg-open", &[&path]);
            Ok(())
        }
    }
}

extern "C" {
    fn getuid() -> u32;
}

/// A desktop shortcut runs a command, so it is only launched when it is the
/// user's own file: owned by the account, not writable by anybody else, marked
/// executable, and a regular file rather than a symlink that could point into a
/// temporary download directory. This is the difference between "double-click
/// opens Brave" and "double-click runs whatever a web page dropped here".
pub fn trusted_shortcut(path: &Path) -> bool {
    let Ok(link) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if link.file_type().is_symlink() {
        return false;
    }
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    use std::os::unix::fs::MetadataExt;
    meta.uid() == unsafe { getuid() } && meta.mode() & 0o022 == 0 && meta.mode() & 0o100 != 0
}

fn valid_name(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.starts_with('.')
        && !name.contains('/')
        && !name.chars().any(char::is_control)
}

pub fn rename(icon: &Icon, new_name: &str) -> Result<(), String> {
    let new_name = new_name.trim();
    if !valid_name(new_name) {
        return Err("A name cannot be empty, start with a dot, or contain “/”.".into());
    }
    if new_name == icon.file {
        return Ok(());
    }
    let Some(parent) = icon.path.parent() else {
        return Err("This item has no folder.".into());
    };
    let target = parent.join(new_name);
    if target.exists() {
        return Err(format!("“{new_name}” already exists here."));
    }
    std::fs::rename(&icon.path, &target).map_err(|e| format!("Could not rename: {e}"))?;
    move_layout(&icon.file, new_name);
    Ok(())
}

fn timestamp() -> String {
    std::process::Command::new("date")
        .arg("+%Y-%m-%dT%H:%M:%S")
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Percent-encoding for a trash `Path=` entry: everything except unreserved
/// characters and the path separator.
pub fn encode_path(path: &str) -> String {
    let mut out = String::new();
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

fn unique_name(dir: &Path, name: &str) -> String {
    if !dir.join(name).exists() {
        return name.to_string();
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| name.to_string());
    let extension = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    for n in 2..1000 {
        let candidate = format!("{stem} {n}{extension}");
        if !dir.join(&candidate).exists() {
            return candidate;
        }
    }
    format!("{stem}-{}", std::process::id())
}

/// Move an item to the freedesktop trash. A delete that cannot be undone from
/// the file manager's Trash is a delete nobody should be able to trigger by
/// accident from a right-click menu.
pub fn trash(icon: &Icon) -> Result<(), String> {
    let home = home().ok_or_else(|| "There is no home directory.".to_string())?;
    let files = home.join(".local/share/Trash/files");
    let info = home.join(".local/share/Trash/info");
    std::fs::create_dir_all(&files).map_err(|e| format!("Could not open the Trash: {e}"))?;
    std::fs::create_dir_all(&info).map_err(|e| format!("Could not open the Trash: {e}"))?;
    let name = unique_name(&files, &icon.file);
    let target = files.join(&name);
    std::fs::rename(&icon.path, &target).map_err(|e| format!("Could not move to Trash: {e}"))?;
    let absolute = icon
        .path
        .canonicalize()
        .unwrap_or_else(|_| icon.path.clone());
    let body = format!(
        "[Trash Info]\nPath={}\nDeletionDate={}\n",
        encode_path(&absolute.to_string_lossy()),
        timestamp()
    );
    let _ = std::fs::write(info.join(format!("{name}.trashinfo")), body);
    Ok(())
}

pub fn new_folder() -> Result<String, String> {
    ensure_dir();
    let dir = dir();
    for n in 1..200 {
        let name = if n == 1 {
            "New Folder".to_string()
        } else {
            format!("New Folder {n}")
        };
        let path = dir.join(&name);
        if !path.exists() {
            std::fs::create_dir(&path).map_err(|e| format!("Could not create the folder: {e}"))?;
            return Ok(name);
        }
    }
    Err("Could not find a free name for a new folder.".into())
}

/// Put a copy of an installed application's desktop file on the desktop. The
/// copy is the user's own file, mode 0755, which is exactly what
/// `trusted_shortcut` asks for: the trust comes from the system directory the
/// file was copied out of, not from the fact that it sits on the desktop.
pub fn install_shortcut(app: &DesktopApp) -> Result<String, String> {
    let source = apps::desktop_file(&app.id)
        .ok_or_else(|| format!("No shortcut file is installed for {}.", app.name))?;
    ensure_dir();
    let dir = dir();
    let base: String = app
        .name
        .chars()
        .map(|c| if c == '/' || c.is_control() { '-' } else { c })
        .collect();
    let base = if base.trim().is_empty() {
        app.id.clone()
    } else {
        base.trim().to_string()
    };
    let mut name = format!("{base}.desktop");
    if dir.join(&name).exists() {
        for n in 2..200 {
            let candidate = format!("{base} {n}.desktop");
            if !dir.join(&candidate).exists() {
                name = candidate;
                break;
            }
        }
    }
    let target = dir.join(&name);
    std::fs::copy(&source, &target).map_err(|e| format!("Could not copy the shortcut: {e}"))?;
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755));
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_round_trips() {
        let icons = vec![
            Icon {
                name: "Files".into(),
                file: "Files.desktop".into(),
                path: PathBuf::from("/home/x/Desktop/Files.desktop"),
                kind: Kind::App,
                app: None,
                col: 0,
                row: 2,
            },
            Icon {
                name: "notes.txt".into(),
                file: "notes.txt".into(),
                path: PathBuf::from("/home/x/Desktop/notes.txt"),
                kind: Kind::File,
                app: None,
                col: 1,
                row: 0,
            },
        ];
        let parsed = parse_layout(&layout_text(&icons));
        assert_eq!(parsed.get("Files.desktop"), Some(&(0, 2)));
        assert_eq!(parsed.get("notes.txt"), Some(&(1, 0)));
    }

    #[test]
    fn layout_ignores_junk_and_out_of_range_rows() {
        let parsed = parse_layout("keep\t2\t3\nrow\t2\t9\nno-cell\n\n");
        assert_eq!(parsed.get("keep"), Some(&(2, 3)));
        assert_eq!(parsed.get("row"), None);
    }

    #[test]
    fn cells_do_not_overlap() {
        let icons = vec![Icon {
            name: "a".into(),
            file: "a".into(),
            path: PathBuf::new(),
            kind: Kind::File,
            app: None,
            col: 0,
            row: 0,
        }];
        // The dragged icon itself may stay where it is; anything else has to
        // move to the next free cell.
        assert_eq!(free_cell(&icons, 0, 0, 0), Some((0, 0)));
        assert_eq!(free_cell(&icons, 0, 0, 1), Some((0, 1)));
    }

    #[test]
    fn cell_at_is_the_inverse_of_cell_origin() {
        let (x, y) = cell_origin(3, 2);
        assert_eq!(cell_at(x + 4.0, y + 4.0), (3, 2));
        assert_eq!(cell_at(0.0, 0.0), (-1, -1));
    }

    #[test]
    fn names_are_validated() {
        assert!(valid_name("Holiday photos"));
        assert!(!valid_name(""));
        assert!(!valid_name(" .. "));
        assert!(!valid_name(".hidden"));
        assert!(!valid_name("a/b"));
    }

    #[test]
    fn trash_paths_are_encoded() {
        assert_eq!(
            encode_path("/home/refurb/Desktop/a b.txt"),
            "/home/refurb/Desktop/a%20b.txt"
        );
        assert_eq!(encode_path("/home/u/x#1"), "/home/u/x%231");
    }

    #[test]
    fn unique_names_do_not_collide() {
        let dir = std::env::temp_dir().join(format!("refurb-desktop-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(unique_name(&dir, "a.txt"), "a.txt");
        std::fs::write(dir.join("a.txt"), b"x").unwrap();
        assert_eq!(unique_name(&dir, "a.txt"), "a 2.txt");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_untrusted_shortcut_is_not_trusted() {
        let dir = std::env::temp_dir().join(format!("refurb-trust-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("evil.desktop");
        std::fs::write(&path, b"[Desktop Entry]\nType=Application\nName=x\nExec=x\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        // World-writable: exactly the case a downloaded shortcut would be in.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(!trusted_shortcut(&path));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(trusted_shortcut(&path));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!trusted_shortcut(&path), "not executable");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
