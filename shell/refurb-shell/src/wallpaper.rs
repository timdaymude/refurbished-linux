//! The wallpapers that ship with Reawakening OS.
//!
//! Two PNGs live in the image at `/usr/share/backgrounds/reawakening`. The
//! shell decodes them itself with the `png` crate — pure Rust, no image tools,
//! no wallpaper daemon — caches the decoded pixels, and paints each output
//! cover-fit at that output's own size. A second monitor therefore gets the
//! same picture cropped, never stretched.
//!
//! The choice is one line in `~/.config/reawakening/wallpaper`. Nothing else
//! reads or writes it, and no other process has to be running for it to work.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use tiny_skia::{Pixmap, Rect};

/// Where the images are installed, relative to the image root.
pub const DIR: &str = "/usr/share/backgrounds/reawakening";

/// Set this to the repository's `assets/wallpapers` to render previews on a
/// machine that has no installed image.
pub const DIR_OVERRIDE: &str = "REAWAKENING_WALLPAPERS";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Choice {
    /// Tux in a blue stained-glass window. The default.
    #[default]
    StainedGlass,
    /// Tux on a moonlit beach.
    Moonlight,
}

impl Choice {
    pub const ALL: [Choice; 2] = [Choice::StainedGlass, Choice::Moonlight];

    pub fn index(self) -> usize {
        match self {
            Choice::StainedGlass => 0,
            Choice::Moonlight => 1,
        }
    }

    pub fn from_index(index: usize) -> Choice {
        Choice::ALL.get(index).copied().unwrap_or_default()
    }

    /// Stable identifier written to the configuration file.
    pub fn id(self) -> &'static str {
        match self {
            Choice::StainedGlass => "default",
            Choice::Moonlight => "moonlight",
        }
    }

    pub fn file_name(self) -> &'static str {
        match self {
            Choice::StainedGlass => "reawakening-default.png",
            Choice::Moonlight => "reawakening-moonlight.png",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Choice::StainedGlass => "Stained glass",
            Choice::Moonlight => "Moonlight",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Choice::StainedGlass => "Tux in a blue stained-glass window. Ships as the default.",
            Choice::Moonlight => "Tux on a moonlit beach, looking out over the ocean.",
        }
    }

    pub fn from_id(text: &str) -> Choice {
        Choice::ALL
            .into_iter()
            .find(|c| c.id() == text.trim())
            .unwrap_or_default()
    }
}

fn directory() -> PathBuf {
    match std::env::var_os(DIR_OVERRIDE) {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(DIR),
    }
}

pub fn source(choice: Choice) -> PathBuf {
    directory().join(choice.file_name())
}

// -- the choice -------------------------------------------------------------

/// The wallpaper the desktop is painting right now. Kept in an atomic so the
/// drawing code does not have to touch the disk on every frame.
static ACTIVE: AtomicUsize = AtomicUsize::new(0);

pub fn active() -> Choice {
    Choice::from_index(ACTIVE.load(Ordering::Relaxed))
}

fn config_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(dir).join("reawakening/wallpaper"));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/reawakening/wallpaper"))
}

/// Read the saved choice and make it the active one. Falls back to the default
/// when the file is missing or names something that is not shipped.
pub fn restore() -> Choice {
    let choice = config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|text| Choice::from_id(&text))
        .unwrap_or_default();
    ACTIVE.store(choice.index(), Ordering::Relaxed);
    choice
}

/// Make `choice` active and remember it for the next session.
pub fn activate(choice: Choice) -> std::io::Result<()> {
    ACTIVE.store(choice.index(), Ordering::Relaxed);
    let Some(path) = config_path() else {
        return Ok(());
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, format!("{}\n", choice.id()))
}

// -- decoding ---------------------------------------------------------------

pub struct Image {
    pub width: u32,
    pub height: u32,
    /// Straight (non-premultiplied) RGBA8; the shipped wallpapers are opaque.
    pub rgba: Vec<u8>,
}

static CACHE: OnceLock<Mutex<Vec<(Choice, Arc<Image>)>>> = OnceLock::new();

fn image(choice: Choice) -> Option<Arc<Image>> {
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(guard) = cache.lock() {
        if let Some((_, img)) = guard.iter().find(|(c, _)| *c == choice) {
            return Some(img.clone());
        }
    }
    let img = Arc::new(decode(&source(choice))?);
    if let Ok(mut guard) = cache.lock() {
        guard.retain(|(c, _)| *c != choice);
        guard.push((choice, img.clone()));
    }
    Some(img)
}

/// True when the image for `choice` can be read (used by the settings panel to
/// say so instead of offering a picture that is not there).
pub fn available(choice: Choice) -> bool {
    source(choice).is_file()
}

fn decode(path: &std::path::Path) -> Option<Image> {
    let file = std::fs::File::open(path).ok()?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    // Expand palette/low-bit-depth files and drop 16-bit samples down to 8:
    // the wallpapers are 8-bit RGB, and nothing here needs more.
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let (width, height) = (info.width, info.height);
    if width == 0 || height == 0 {
        return None;
    }
    let src = &buf[..info.buffer_size()];
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    match info.color_type {
        png::ColorType::Rgb => {
            for px in src.chunks_exact(3) {
                rgba.extend_from_slice(&[px[0], px[1], px[2], 0xFF]);
            }
        }
        png::ColorType::Rgba => rgba.extend_from_slice(src),
        png::ColorType::Grayscale => {
            for v in src {
                rgba.extend_from_slice(&[*v, *v, *v, 0xFF]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for px in src.chunks_exact(2) {
                rgba.extend_from_slice(&[px[0], px[0], px[0], px[1]]);
            }
        }
        png::ColorType::Indexed => return None,
    }
    if rgba.len() < width as usize * height as usize * 4 {
        return None;
    }
    Some(Image {
        width,
        height,
        rgba,
    })
}

// -- painting ---------------------------------------------------------------

/// Paint the active wallpaper over the whole pixmap. Returns false when the
/// image is missing, so the caller can fall back to the drawn background.
pub fn paint(pm: &mut Pixmap) -> bool {
    let choice = active();
    let Some(img) = image(choice) else {
        return false;
    };
    let area = Rect::from_xywh(0.0, 0.0, pm.width() as f32, pm.height() as f32)
        .unwrap_or_else(|| Rect::from_xywh(0.0, 0.0, 1.0, 1.0).expect("unit rect"));
    draw_cover(pm, &img, area);
    true
}

/// Draw one wallpaper into `area`, cropped to fill it (the same rule the
/// desktop uses). Small rectangles are used by the settings thumbnails.
pub fn draw_into(pm: &mut Pixmap, choice: Choice, area: Rect) -> bool {
    let Some(img) = image(choice) else {
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

    fn repo_wallpapers(dir: &str) {
        std::env::set_var(DIR_OVERRIDE, dir);
    }

    #[test]
    fn ids_and_files_are_stable() {
        assert_eq!(Choice::StainedGlass.id(), "default");
        assert_eq!(Choice::Moonlight.file_name(), "reawakening-moonlight.png");
        assert_eq!(Choice::from_id("moonlight"), Choice::Moonlight);
        assert_eq!(Choice::from_id("nonsense"), Choice::StainedGlass);
        assert_eq!(Choice::from_index(9), Choice::StainedGlass);
    }

    #[test]
    fn paints_the_shipped_wallpaper() {
        repo_wallpapers("../../assets/wallpapers");
        let mut pm = Pixmap::new(320, 200).expect("pixmap");
        assert!(paint(&mut pm), "the shipped default wallpaper did not decode");
        // The stained glass is blue: something blue must have been painted.
        let blue = pm
            .pixels()
            .iter()
            .filter(|p| p.blue() > 80 && p.blue() > p.red())
            .count();
        assert!(blue > 1000, "wallpaper painted {blue} blue pixels");
    }

    #[test]
    fn thumbnails_fill_their_rect_without_touching_the_rest() {
        repo_wallpapers("../../assets/wallpapers");
        let mut pm = Pixmap::new(400, 200).expect("pixmap");
        let area = Rect::from_xywh(50.0, 20.0, 120.0, 80.0).expect("rect");
        assert!(draw_into(&mut pm, Choice::Moonlight, area));
        let inside = pm.pixel(60, 30).expect("inside").alpha();
        let outside = pm.pixel(5, 5).expect("outside").alpha();
        assert_eq!(inside, 255, "thumbnail did not paint");
        assert_eq!(outside, 0, "thumbnail painted outside its rect");
    }
}
