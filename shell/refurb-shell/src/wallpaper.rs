//! Wallpapers: the pictures that ship with Reawakening OS, and the ones a user
//! brings themselves.
//!
//! One system paints both. A built-in picture is read from
//! `/usr/share/backgrounds/reawakening`; a personal one is a PNG that
//! `refurb-wallpaper` (or Settings → Appearance) copied into
//! `~/.local/share/reawakening/wallpapers` at a sensible size. The choice is one
//! line in `~/.config/reawakening/wallpaper`, so it survives a reboot without a
//! daemon, and the shell notices a change to that file while it is running.
//!
//! Decoding is pure Rust (`image` with the PNG, JPEG and WebP readers), so the
//! image carries no ImageMagick, no thumbnail service and no wallpaper daemon.
//! The active picture is cached once — a few megabytes — and thumbnails are
//! cached at 340 px wide (about a quarter of a megabyte each), because the
//! settings panel must not decode six 1920×1080 photographs every time it is
//! repainted on a 2 GB machine.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use tiny_skia::{Pixmap, Rect};

/// Where the built-in images are installed, relative to the image root.
pub const DIR: &str = "/usr/share/backgrounds/reawakening";

/// Set this to the repository's `assets/wallpapers` to render previews on a
/// machine that has no installed image.
pub const DIR_OVERRIDE: &str = "REAWAKENING_WALLPAPERS";

/// Set this to keep personal wallpapers somewhere other than the home directory
/// (used by the tests).
pub const HOME_OVERRIDE: &str = "REAWAKENING_WALLPAPER_HOME";

/// Personal pictures are copied no wider than this. A 12-megapixel photograph
/// is 4000 px across and would cost 60 MB as RGBA on every repaint; 2560 keeps
/// a 1080p or 1440p screen sharp and costs at most ~16 MB once.
pub const MAX_WIDTH: u32 = 2560;
pub const MAX_HEIGHT: u32 = 1600;

pub struct Builtin {
    pub id: &'static str,
    pub file: &'static str,
    pub label: &'static str,
    pub blurb: &'static str,
}

/// The pictures in the image. Stable ids, because they are what the
/// configuration file stores and what a user's existing choice is matched
/// against after an update.
pub const BUILTIN: [Builtin; 6] = [
    Builtin {
        id: "default",
        file: "reawakening-default.png",
        label: "Stained glass",
        blurb: "Tux in a blue stained-glass window. Ships as the default.",
    },
    Builtin {
        id: "moonlight",
        file: "reawakening-moonlight.png",
        label: "Moonlight",
        blurb: "Tux on a moonlit beach, looking out over the ocean.",
    },
    Builtin {
        id: "summit",
        file: "reawakening-summit.jpg",
        label: "Summit",
        blurb: "A mountain lake at first light.",
    },
    Builtin {
        id: "orbit",
        file: "reawakening-orbit.jpg",
        label: "Orbit",
        blurb: "A ringed world above a quiet nebula.",
    },
    Builtin {
        id: "clockwork",
        file: "reawakening-clockwork.jpg",
        label: "Clockwork",
        blurb: "Brass gears and an hourglass, for the time-traveller.",
    },
    Builtin {
        id: "workshop",
        file: "reawakening-workshop.jpg",
        label: "Workshop",
        blurb: "Tux at a workbench, tinkering after hours.",
    },
];

/// The default picture, used whenever nothing else can be read.
pub fn default_wallpaper() -> Wallpaper {
    Wallpaper::Builtin(BUILTIN[0].id)
}

fn builtin(id: &str) -> Option<&'static Builtin> {
    BUILTIN.iter().find(|b| b.id == id)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Wallpaper {
    Builtin(&'static str),
    /// A copied, managed picture. The original file is never moved, so a
    /// photograph on a USB drive stays where it is and the desktop keeps
    /// working after the drive is pulled out.
    Personal(PathBuf),
}

impl Default for Wallpaper {
    fn default() -> Self {
        default_wallpaper()
    }
}

impl Wallpaper {
    pub fn label(&self) -> String {
        match self {
            Wallpaper::Builtin(id) => builtin(id).map(|b| b.label.to_string()).unwrap_or_default(),
            Wallpaper::Personal(path) => path
                .file_stem()
                .map(|s| s.to_string_lossy().replace('-', " "))
                .unwrap_or_else(|| "Your picture".to_string()),
        }
    }

    pub fn blurb(&self) -> String {
        match self {
            Wallpaper::Builtin(id) => builtin(id).map(|b| b.blurb.to_string()).unwrap_or_default(),
            Wallpaper::Personal(path) => format!(
                "Your own picture, kept at {} so it survives the original being moved.",
                path.display()
            ),
        }
    }

    pub fn path(&self) -> PathBuf {
        match self {
            Wallpaper::Builtin(id) => directory().join(
                builtin(id)
                    .map(|b| b.file)
                    .unwrap_or(BUILTIN[0].file),
            ),
            Wallpaper::Personal(path) => path.clone(),
        }
    }

    /// What is written to the configuration file: a built-in id, or a path
    /// prefixed with `file:` so a file called `moonlight` cannot be mistaken
    /// for the built-in picture of the same name.
    pub fn id(&self) -> String {
        match self {
            Wallpaper::Builtin(id) => (*id).to_string(),
            Wallpaper::Personal(path) => format!("file:{}", path.display()),
        }
    }

    pub fn from_id(text: &str) -> Wallpaper {
        let text = text.trim();
        if let Some(path) = text.strip_prefix("file:") {
            let path = PathBuf::from(path);
            if !path.as_os_str().is_empty() {
                return Wallpaper::Personal(path);
            }
        }
        builtin(text)
            .map(|b| Wallpaper::Builtin(b.id))
            .unwrap_or_else(default_wallpaper)
    }

    /// True when the picture can actually be read right now. Settings uses it
    /// to say "this picture is missing" instead of painting a blank screen.
    pub fn exists(&self) -> bool {
        self.path().is_file()
    }

    pub fn is_builtin(&self) -> bool {
        matches!(self, Wallpaper::Builtin(_))
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub wallpaper: Wallpaper,
    pub label: String,
    pub blurb: String,
}

/// Every picture the user can pick: the built-ins, then the personal ones,
/// newest first.
pub fn catalog() -> Vec<Entry> {
    let mut entries: Vec<Entry> = BUILTIN
        .iter()
        .map(|b| Entry {
            wallpaper: Wallpaper::Builtin(b.id),
            label: b.label.to_string(),
            blurb: b.blurb.to_string(),
        })
        .collect();
    let mut personal: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    if let Some(dir) = personal_dir() {
        if let Ok(read) = std::fs::read_dir(&dir) {
            for entry in read.flatten() {
                let path = entry.path();
                let image = path
                    .extension()
                    .map(|e| e.eq_ignore_ascii_case("png") || e.eq_ignore_ascii_case("jpg"))
                    .unwrap_or(false);
                if !image {
                    continue;
                }
                let modified = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                personal.push((modified, path));
            }
        }
    }
    personal.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, path) in personal {
        let wall = Wallpaper::Personal(path);
        let label = wall.label();
        let blurb = wall.blurb();
        entries.push(Entry {
            wallpaper: wall,
            label,
            blurb,
        });
    }
    entries
}

fn directory() -> PathBuf {
    match std::env::var_os(DIR_OVERRIDE) {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(DIR),
    }
}

pub fn personal_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(HOME_OVERRIDE) {
        return Some(PathBuf::from(dir));
    }
    let base = match std::env::var_os("XDG_DATA_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => PathBuf::from(std::env::var_os("HOME")?).join(".local/share"),
    };
    Some(base.join("reawakening/wallpapers"))
}

/// The configuration file. Exposed so the session can watch it for changes made
/// by another process (the file manager's "Set as Desktop Wallpaper" action).
pub fn config_file() -> Option<PathBuf> {
    config_path().map(|_| {
        let base = match std::env::var_os("XDG_CONFIG_HOME") {
            Some(dir) if !dir.is_empty() => PathBuf::from(dir),
            _ => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"),
        };
        base.join("reawakening/wallpaper")
    })
}

fn config_path() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => PathBuf::from(std::env::var_os("HOME")?).join(".config"),
    };
    Some(base.join("reawakening/wallpaper"))
}

// -- the choice -------------------------------------------------------------

fn active_cell() -> &'static Mutex<Wallpaper> {
    static ACTIVE: OnceLock<Mutex<Wallpaper>> = OnceLock::new();
    ACTIVE.get_or_init(|| Mutex::new(default_wallpaper()))
}

pub fn active() -> Wallpaper {
    active_cell()
        .lock()
        .map(|w| w.clone())
        .unwrap_or_else(|_| default_wallpaper())
}

/// Read the saved choice and make it the active one. A missing personal file is
/// remembered rather than silently forgotten, so Settings can explain it.
pub fn restore() -> Wallpaper {
    let choice = config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|text| Wallpaper::from_id(&text))
        .unwrap_or_default();
    if let Ok(mut active) = active_cell().lock() {
        *active = choice.clone();
    }
    choice
}

/// Make `choice` active and remember it for the next session.
pub fn activate(choice: &Wallpaper) -> std::io::Result<()> {
    if let Ok(mut active) = active_cell().lock() {
        *active = choice.clone();
    }
    let Some(path) = config_path() else {
        return Ok(());
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Write-and-rename: the shell watches this file to repaint when another
    // process (the file manager's action) changes the wallpaper, and a
    // half-written line would be read as "default".
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, format!("{}\n", choice.id()))?;
    std::fs::rename(tmp, path)
}

// -- decoding ---------------------------------------------------------------

pub struct Image {
    pub width: u32,
    pub height: u32,
    /// Straight (non-premultiplied) RGBA8.
    pub rgba: Vec<u8>,
}

const THUMB_WIDTH: u32 = 340;

fn full_cache() -> &'static Mutex<Option<(String, Arc<Image>)>> {
    static CACHE: OnceLock<Mutex<Option<(String, Arc<Image>)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn thumb_cache() -> &'static Mutex<Vec<(String, Arc<Image>)>> {
    static CACHE: OnceLock<Mutex<Vec<(String, Arc<Image>)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Vec::new()))
}

fn key(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn load_full(wall: &Wallpaper) -> Option<Arc<Image>> {
    let path = wall.path();
    let key = key(&path);
    if let Ok(cache) = full_cache().lock() {
        if let Some((k, img)) = cache.as_ref() {
            if *k == key {
                return Some(img.clone());
            }
        }
    }
    let img = Arc::new(decode(&path)?);
    if let Ok(mut cache) = full_cache().lock() {
        *cache = Some((key, img.clone()));
    }
    Some(img)
}

/// Small copy, decoded once. Six photographs at 1920×1080 would otherwise be
/// 48 MB of pixels and would be thrown away after every repaint.
fn load_thumb(wall: &Wallpaper) -> Option<Arc<Image>> {
    let path = wall.path();
    let key = key(&path);
    if let Ok(cache) = thumb_cache().lock() {
        if let Some((_, img)) = cache.iter().find(|(k, _)| *k == key) {
            return Some(img.clone());
        }
    }
    let full = load_full(wall)?;
    let width = THUMB_WIDTH.min(full.width.max(1));
    let height = ((full.height as f32 * width as f32 / full.width.max(1) as f32).round() as u32).max(1);
    let small = Arc::new(scale_bilinear(&full, width, height));
    if let Ok(mut cache) = thumb_cache().lock() {
        cache.retain(|(k, _)| *k != key);
        cache.push((key, small.clone()));
        // A handful is enough for a settings page; anything beyond it is from a
        // picture the user has since replaced.
        if cache.len() > 16 {
            cache.remove(0);
        }
    }
    Some(small)
}

fn decode(path: &Path) -> Option<Image> {
    let reader = image::ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    let img = reader.decode().ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    if width == 0 || height == 0 {
        return None;
    }
    Some(Image {
        width,
        height,
        rgba: img.into_raw(),
    })
}

fn scale_bilinear(img: &Image, width: u32, height: u32) -> Image {
    let (sw, sh) = (img.width.max(1), img.height.max(1));
    let mut rgba = vec![0u8; width as usize * height as usize * 4];
    for y in 0..height {
        let sy = ((y as f32 + 0.5) * sh as f32 / height as f32 - 0.5).clamp(0.0, sh as f32 - 1.0);
        let y0 = sy.floor() as u32;
        let y1 = (y0 + 1).min(sh - 1);
        let fy = sy - y0 as f32;
        for x in 0..width {
            let sx = ((x as f32 + 0.5) * sw as f32 / width as f32 - 0.5).clamp(0.0, sw as f32 - 1.0);
            let x0 = sx.floor() as u32;
            let x1 = (x0 + 1).min(sw - 1);
            let fx = sx - x0 as f32;
            let at = |ix: u32, iy: u32| {
                let o = (iy as usize * sw as usize + ix as usize) * 4;
                [
                    img.rgba[o] as f32,
                    img.rgba[o + 1] as f32,
                    img.rgba[o + 2] as f32,
                    img.rgba[o + 3] as f32,
                ]
            };
            let (a, b) = (at(x0, y0), at(x1, y0));
            let (c, d) = (at(x0, y1), at(x1, y1));
            let mut out = [0u8; 4];
            for ch in 0..4 {
                let top = a[ch] + (b[ch] - a[ch]) * fx;
                let bottom = c[ch] + (d[ch] - c[ch]) * fx;
                out[ch] = (top + (bottom - top) * fy).round().clamp(0.0, 255.0) as u8;
            }
            let o = (y as usize * width as usize + x as usize) * 4;
            rgba[o..o + 4].copy_from_slice(&out);
        }
    }
    Image {
        width,
        height,
        rgba,
    }
}

// -- installing a personal picture -----------------------------------------

/// Fit an image inside the managed size without distorting it.
fn fit(width: u32, height: u32) -> (u32, u32) {
    if width == 0 || height == 0 {
        return (1, 1);
    }
    let scale = (MAX_WIDTH as f32 / width as f32)
        .min(MAX_HEIGHT as f32 / height as f32)
        .min(1.0);
    (
        (width as f32 * scale).round().max(1.0) as u32,
        (height as f32 * scale).round().max(1.0) as u32,
    )
}

fn short_hash(text: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")[..10].to_string()
}

fn sanitize(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars().take(40) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "picture".to_string()
    } else {
        out
    }
}

/// Copy `source` into the managed directory as a PNG no larger than
/// [`MAX_WIDTH`]×[`MAX_HEIGHT`], and return the managed path. The original is
/// only read; nothing about it is changed or moved.
pub fn install_to(source: &Path, dir: &Path) -> Result<PathBuf, String> {
    let reader = image::ImageReader::open(source)
        .map_err(|e| format!("cannot open {}: {e}", source.display()))?
        .with_guessed_format()
        .map_err(|e| format!("cannot read {}: {e}", source.display()))?;
    let (width, height) = reader
        .into_dimensions()
        .map_err(|e| format!("{} is not an image this system can read: {e}", source.display()))?;
    if width as u64 * height as u64 > 80_000_000 {
        return Err(format!(
            "{} is {}×{}, too large to use as a wallpaper",
            source.display(),
            width,
            height
        ));
    }
    let decoded = image::ImageReader::open(source)
        .map_err(|e| format!("cannot open {}: {e}", source.display()))?
        .with_guessed_format()
        .map_err(|e| format!("cannot read {}: {e}", source.display()))?
        .decode()
        .map_err(|e| format!("{} could not be decoded: {e}", source.display()))?
        .into_rgba8();
    let (tw, th) = fit(decoded.width(), decoded.height());
    let scaled = if (tw, th) == decoded.dimensions() {
        decoded
    } else {
        image::imageops::resize(&decoded, tw, th, image::imageops::FilterType::Triangle)
    };
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let stem = source
        .file_stem()
        .map(|s| sanitize(&s.to_string_lossy()))
        .unwrap_or_else(|| "picture".to_string());
    let stamp = std::fs::metadata(source)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let name = format!("personal-{stem}-{}.png", short_hash(&format!("{}{stamp}{tw}x{th}", source.display())));
    let dest = dir.join(name);
    scaled
        .save_with_format(&dest, image::ImageFormat::Png)
        .map_err(|e| format!("cannot write {}: {e}", dest.display()))?;
    // Keep the file world-readable (0644): the desktop reads it as the same
    // user, and a backup run as another account should not be locked out.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o644));
    }
    Ok(dest)
}

/// Install a picture into the user's managed directory.
pub fn install(source: &Path) -> Result<Wallpaper, String> {
    let dir = personal_dir().ok_or_else(|| "no home directory is set".to_string())?;
    Ok(Wallpaper::Personal(install_to(source, &dir)?))
}

// -- painting ---------------------------------------------------------------

/// Paint the active wallpaper over the whole pixmap. Returns false when no
/// image can be read, so the caller can fall back to the drawn background.
pub fn paint(pm: &mut Pixmap) -> bool {
    let wall = active();
    let img = load_full(&wall).or_else(|| load_full(&default_wallpaper()));
    let Some(img) = img else {
        return false;
    };
    let area = Rect::from_xywh(0.0, 0.0, pm.width() as f32, pm.height() as f32)
        .unwrap_or_else(|| Rect::from_xywh(0.0, 0.0, 1.0, 1.0).expect("unit rect"));
    draw_cover(pm, &img, area);
    true
}

/// Draw one wallpaper into `area`, cropped to fill it. Settings uses the small
/// cached copy so a page of thumbnails does not cost tens of megabytes.
pub fn draw_into(pm: &mut Pixmap, wall: &Wallpaper, area: Rect) -> bool {
    let Some(img) = load_thumb(wall) else {
        return false;
    };
    draw_cover(pm, &img, area);
    true
}

/// Bilinear, cover-fit. The image is scaled by the larger of the two ratios and
/// centred, so a wide monitor crops top and bottom rather than distorting the
/// picture; a narrow one crops the sides.
fn draw_cover(pm: &mut Pixmap, img: &Image, area: Rect) {
    let (pw, ph) = (pm.width() as i32, pm.height() as i32);
    let x0 = area.x().floor().max(0.0) as i32;
    let y0 = area.y().floor().max(0.0) as i32;
    let x1 = area.right().ceil().min(pw as f32) as i32;
    let y1 = area.bottom().ceil().min(ph as f32) as i32;
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let (aw, ah) = (area.width().max(1.0), area.height().max(1.0));
    let scale = (aw / img.width as f32).max(ah / img.height as f32);
    let off_x = (img.width as f32 - aw / scale) / 2.0;
    let off_y = (img.height as f32 - ah / scale) / 2.0;
    let (iw, ih) = (img.width as i32, img.height as i32);
    let stride = pw as usize * 4;
    let data = pm.data_mut();

    for y in y0..y1 {
        let sy = off_y + ((y as f32 + 0.5) - area.y()) / scale - 0.5;
        let sy = sy.clamp(0.0, ih as f32 - 1.0);
        let iy0 = sy.floor() as i32;
        let iy1 = (iy0 + 1).min(ih - 1);
        let fy = sy - iy0 as f32;
        for x in x0..x1 {
            let sx = off_x + ((x as f32 + 0.5) - area.x()) / scale - 0.5;
            let sx = sx.clamp(0.0, iw as f32 - 1.0);
            let ix0 = sx.floor() as i32;
            let ix1 = (ix0 + 1).min(iw - 1);
            let fx = sx - ix0 as f32;

            let at = |ix: i32, iy: i32| {
                let o = (iy as usize * img.width as usize + ix as usize) * 4;
                [
                    img.rgba[o] as f32,
                    img.rgba[o + 1] as f32,
                    img.rgba[o + 2] as f32,
                    img.rgba[o + 3] as f32,
                ]
            };
            let (a, b) = (at(ix0, iy0), at(ix1, iy0));
            let (c, d) = (at(ix0, iy1), at(ix1, iy1));
            let mut out = [0u8; 4];
            for ch in 0..4 {
                let top = a[ch] + (b[ch] - a[ch]) * fx;
                let bottom = c[ch] + (d[ch] - c[ch]) * fx;
                out[ch] = (top + (bottom - top) * fy).round().clamp(0.0, 255.0) as u8;
            }
            let o = y as usize * stride + x as usize * 4;
            data[o] = out[0];
            data[o + 1] = out[1];
            data[o + 2] = out[2];
            data[o + 3] = out[3];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_wallpapers() {
        std::env::set_var(DIR_OVERRIDE, "../../assets/wallpapers");
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("refurb-wallpaper-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn ids_round_trip_and_unknown_ones_fall_back() {
        assert_eq!(Wallpaper::from_id("moonlight"), Wallpaper::Builtin("moonlight"));
        assert_eq!(
            Wallpaper::from_id("file:/home/x/pic.png"),
            Wallpaper::Personal(PathBuf::from("/home/x/pic.png"))
        );
        assert_eq!(Wallpaper::from_id("nonsense"), default_wallpaper());
        assert_eq!(
            Wallpaper::Builtin("orbit").id(),
            "orbit",
            "the configuration must store the stable id, not the file name"
        );
    }

    #[test]
    fn every_built_in_has_a_distinct_id_and_file() {
        let mut ids: Vec<&str> = BUILTIN.iter().map(|b| b.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), BUILTIN.len());
        let mut files: Vec<&str> = BUILTIN.iter().map(|b| b.file).collect();
        files.sort_unstable();
        files.dedup();
        assert_eq!(files.len(), BUILTIN.len());
    }

    #[test]
    fn paints_the_shipped_wallpaper() {
        repo_wallpapers();
        let mut pm = Pixmap::new(320, 200).expect("pixmap");
        assert!(paint(&mut pm), "the shipped default wallpaper did not decode");
        let blue = pm
            .pixels()
            .iter()
            .filter(|p| p.blue() > 80 && p.blue() > p.red())
            .count();
        assert!(blue > 1000, "wallpaper painted {blue} blue pixels");
    }

    #[test]
    fn decodes_every_built_in_including_the_jpegs() {
        repo_wallpapers();
        for b in BUILTIN.iter() {
            let path = PathBuf::from("../../assets/wallpapers").join(b.file);
            assert!(
                path.is_file(),
                "{} is listed in the catalogue but not in assets/wallpapers",
                b.file
            );
            let img = decode(&path).unwrap_or_else(|| panic!("{} did not decode", b.file));
            assert!(img.width > 640 && img.height > 360, "{} is too small", b.file);
        }
    }

    #[test]
    fn thumbnails_fill_their_rect_without_touching_the_rest() {
        repo_wallpapers();
        let mut pm = Pixmap::new(400, 200).expect("pixmap");
        let area = Rect::from_xywh(50.0, 20.0, 120.0, 80.0).expect("rect");
        assert!(draw_into(&mut pm, &Wallpaper::Builtin("moonlight"), area));
        let inside = pm.pixel(60, 30).expect("inside").alpha();
        let outside = pm.pixel(5, 5).expect("outside").alpha();
        assert_eq!(inside, 255, "thumbnail did not paint");
        assert_eq!(outside, 0, "thumbnail painted outside its rect");
    }

    #[test]
    fn a_missing_personal_picture_is_reported_but_still_paints() {
        repo_wallpapers();
        let wall = Wallpaper::Personal(PathBuf::from("/nonexistent/photo.png"));
        assert!(!wall.exists());
        assert!(activate(&wall).is_ok());
        let mut pm = Pixmap::new(320, 200).expect("pixmap");
        assert!(paint(&mut pm), "a missing picture must fall back to the default");
        assert!(activate(&default_wallpaper()).is_ok());
    }

    #[test]
    fn a_personal_picture_is_copied_at_a_sensible_size_and_kept() {
        let dir = temp_dir("install");
        let source = dir.join("holiday.png");
        let mut img = image::RgbaImage::new(4000, 3000);
        for (x, y, px) in img.enumerate_pixels_mut() {
            *px = image::Rgba([(x % 255) as u8, (y % 255) as u8, 90, 255]);
        }
        img.save_with_format(&source, image::ImageFormat::Png).expect("write source");
        let installed = install_to(&source, &dir.join("managed")).expect("install");
        assert!(installed.is_file());
        assert!(source.is_file(), "the original must not be moved");
        let saved = decode(&installed).expect("installed decodes");
        // 4000×3000 is capped by the 1600 px height, not by the width.
        assert_eq!((saved.width, saved.height), (2133, 1600));
        assert!(installed.to_string_lossy().contains("holiday"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_tall_picture_is_fitted_without_stretching() {
        assert_eq!(fit(4000, 3000), (2133, 1600));
        assert_eq!(fit(1200, 4000), (480, 1600));
        assert_eq!(fit(800, 600), (800, 600), "small pictures are not enlarged");
    }
}
