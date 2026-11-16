//! Type: Inter (bundled, OFL) rasterised through `fontdue` into a glyph cache.
//!
//! The shell ships its own fonts so that the bar and launcher look identical
//! on every installation and never depend on a font package being present.

use std::collections::HashMap;

use fontdue::{Font, FontSettings, Metrics};
use tiny_skia::{Color, Pixmap};

use crate::render::blend;

pub const REGULAR: u8 = 0;
pub const MEDIUM: u8 = 1;
pub const SEMIBOLD: u8 = 2;

static REGULAR_BYTES: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
static MEDIUM_BYTES: &[u8] = include_bytes!("../assets/fonts/Inter-Medium.ttf");
static SEMIBOLD_BYTES: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");

pub struct TextEngine {
    fonts: Vec<Font>,
    cache: HashMap<(u8, u16, char), (Metrics, Vec<u8>)>,
}

impl TextEngine {
    pub fn new() -> Self {
        let load = |bytes: &'static [u8]| {
            Font::from_bytes(bytes, FontSettings::default()).expect("bundled Inter font is valid")
        };
        Self {
            fonts: vec![
                load(REGULAR_BYTES),
                load(MEDIUM_BYTES),
                load(SEMIBOLD_BYTES),
            ],
            cache: HashMap::with_capacity(1024),
        }
    }

    fn px_key(px: f32) -> u16 {
        (px * 4.0).round().clamp(4.0, 4096.0) as u16
    }

    pub fn ascent(&self, px: f32, weight: u8) -> f32 {
        self.fonts[weight as usize]
            .horizontal_line_metrics(px)
            .map(|m| m.ascent)
            .unwrap_or(px * 0.75)
    }

    pub fn descent(&self, px: f32, weight: u8) -> f32 {
        self.fonts[weight as usize]
            .horizontal_line_metrics(px)
            .map(|m| m.descent)
            .unwrap_or(px * 0.25)
    }

    /// Baseline that vertically centres a single line of text in `band`.
    pub fn baseline_in(&self, band_top: f32, band_h: f32, px: f32, weight: u8) -> f32 {
        let asc = self.ascent(px, weight);
        let desc = self.descent(px, weight);
        band_top + (band_h - (asc + desc)) / 2.0 + asc
    }

    fn with_glyph<F: FnOnce(&Metrics, &[u8])>(
        &mut self,
        weight: u8,
        key: u16,
        ch: char,
        px: f32,
        f: F,
    ) -> Metrics {
        let k = (weight, key, ch);
        if !self.cache.contains_key(&k) {
            let font = &self.fonts[weight as usize % self.fonts.len()];
            let (m, bitmap) = font.rasterize(ch, px);
            self.cache.insert(k, (m, bitmap));
        }
        let (m, bitmap) = self.cache.get(&k).expect("glyph cached above");
        f(m, bitmap);
        m.clone()
    }

    pub fn width(&mut self, s: &str, px: f32, weight: u8) -> f32 {
        let key = Self::px_key(px);
        let real = key as f32 / 4.0;
        let mut w = 0.0;
        for ch in s.chars() {
            if ch == '\t' {
                w += real * 2.0;
                continue;
            }
            w += self.with_glyph(weight, key, ch, real, |_, _| {}).advance_width;
        }
        w
    }

    /// Draw `s` with its baseline at `baseline`; returns the advance width.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        pm: &mut Pixmap,
        x: f32,
        baseline: f32,
        s: &str,
        px: f32,
        weight: u8,
        color: Color,
    ) -> f32 {
        let key = Self::px_key(px);
        let real = key as f32 / 4.0;
        let mut pen = x;
        for ch in s.chars() {
            if ch == '\t' {
                pen += real * 2.0;
                continue;
            }
            let m = self.with_glyph(weight, key, ch, real, |m, bitmap| {
                if ch == ' ' || m.width == 0 || m.height == 0 {
                    return;
                }
                let gx = (pen + m.xmin as f32).round() as i32;
                let gy = (baseline - (m.height as f32 + m.ymin as f32)).round() as i32;
                for row in 0..m.height {
                    for col in 0..m.width {
                        let a = bitmap[row * m.width + col];
                        if a == 0 {
                            continue;
                        }
                        blend(pm, gx + col as i32, gy + row as i32, color, a as f32 / 255.0);
                    }
                }
            });
            pen += m.advance_width;
        }
        pen - x
    }

    /// Left-aligned text, vertically centred in a band.
    #[allow(clippy::too_many_arguments)]
    pub fn text_left(
        &mut self,
        pm: &mut Pixmap,
        x: f32,
        top: f32,
        h: f32,
        s: &str,
        px: f32,
        weight: u8,
        color: Color,
    ) -> f32 {
        let b = self.baseline_in(top, h, px, weight);
        self.draw(pm, x, b, s, px, weight, color)
    }

    /// Centre text horizontally inside `[x, x + w]`.
    #[allow(clippy::too_many_arguments)]
    pub fn text_center(
        &mut self,
        pm: &mut Pixmap,
        x: f32,
        w: f32,
        top: f32,
        h: f32,
        s: &str,
        px: f32,
        weight: u8,
        color: Color,
    ) {
        let tw = self.width(s, px, weight);
        let b = self.baseline_in(top, h, px, weight);
        self.draw(pm, x + (w - tw) / 2.0, b, s, px, weight, color);
    }

    /// Right-aligned text ending at `right`.
    #[allow(clippy::too_many_arguments)]
    pub fn text_right(
        &mut self,
        pm: &mut Pixmap,
        right: f32,
        top: f32,
        h: f32,
        s: &str,
        px: f32,
        weight: u8,
        color: Color,
    ) {
        let tw = self.width(s, px, weight);
        let b = self.baseline_in(top, h, px, weight);
        self.draw(pm, right - tw, b, s, px, weight, color);
    }

    /// Shorten with an ellipsis so the result fits in `max_w`.
    pub fn ellipsize(&mut self, s: &str, px: f32, weight: u8, max_w: f32) -> String {
        if max_w <= 0.0 {
            return String::new();
        }
        if self.width(s, px, weight) <= max_w {
            return s.to_string();
        }
        let chars: Vec<char> = s.chars().collect();
        let mut lo = 0usize;
        let mut hi = chars.len();
        while lo < hi {
            let mid = (lo + hi + 1) / 2;
            let candidate: String = chars[..mid].iter().collect::<String>() + "…";
            if self.width(&candidate, px, weight) <= max_w {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        let mut out: String = chars[..lo].iter().collect();
        out.push('…');
        out
    }
}

impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_and_ellipsizes() {
        let mut t = TextEngine::new();
        let w = t.width("Applications", 13.0, REGULAR);
        assert!(w > 40.0 && w < 200.0, "unexpected width {w}");
        let short = t.ellipsize("A very long application title indeed", 13.0, REGULAR, 60.0);
        assert!(t.width(&short, 13.0, REGULAR) <= 60.5);
    }

    #[test]
    fn draws_ink_into_an_empty_pixmap() {
        let mut t = TextEngine::new();
        let mut pm = Pixmap::new(120, 40).unwrap();
        t.text_left(&mut pm, 4.0, 0.0, 40.0, "Brave", 16.0, MEDIUM, crate::theme::ink());
        let painted = pm.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert!(painted > 50, "expected glyph coverage, got {painted}");
    }
}
