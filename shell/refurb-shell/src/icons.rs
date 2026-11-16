//! The Refurbished Linux icon set.
//!
//! Drawn as vectors on a 24×24 unit grid with 1.9-unit round strokes, so the
//! whole set is resolution independent, needs no icon theme package and stays
//! sharp on the low-resolution panels that old laptops actually have.

use tiny_skia::{Color, FillRule, Paint, Path, PathBuilder, Pixmap, Shader, Transform};

use crate::render::{circle, circle_stroke, fill_path, rect, round, round_stroke, stroke_path};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Brand,
    Grid,
    Search,
    Wifi,
    WifiOff,
    Bluetooth,
    Volume,
    VolumeMute,
    Battery,
    Power,
    Globe,
    Envelope,
    Cloud,
    Chat,
    Doc,
    Folder,
    Terminal,
    Gear,
    Download,
    Check,
    Close,
    ChevronRight,
    Lock,
    Refresh,
    Shield,
    Display,
    Moon,
    Restart,
    Plus,
    Minus,
    Apps,
    Dot,
}

impl Glyph {
    pub const ALL: [Glyph; 32] = [
        Glyph::Brand,
        Glyph::Grid,
        Glyph::Search,
        Glyph::Wifi,
        Glyph::WifiOff,
        Glyph::Bluetooth,
        Glyph::Volume,
        Glyph::VolumeMute,
        Glyph::Battery,
        Glyph::Power,
        Glyph::Globe,
        Glyph::Folder,
        Glyph::Terminal,
        Glyph::Gear,
        Glyph::Download,
        Glyph::Check,
        Glyph::Close,
        Glyph::ChevronRight,
        Glyph::Lock,
        Glyph::Refresh,
        Glyph::Shield,
        Glyph::Display,
        Glyph::Moon,
        Glyph::Restart,
        Glyph::Plus,
        Glyph::Minus,
        Glyph::Apps,
        Glyph::Dot,
        Glyph::Envelope,
        Glyph::Cloud,
        Glyph::Chat,
        Glyph::Doc,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Glyph::Brand => "brand",
            Glyph::Grid => "grid",
            Glyph::Search => "search",
            Glyph::Wifi => "wifi",
            Glyph::WifiOff => "wifi-off",
            Glyph::Bluetooth => "bluetooth",
            Glyph::Volume => "volume",
            Glyph::VolumeMute => "volume-mute",
            Glyph::Battery => "battery",
            Glyph::Power => "power",
            Glyph::Globe => "globe",
            Glyph::Envelope => "envelope",
            Glyph::Cloud => "cloud",
            Glyph::Chat => "chat",
            Glyph::Doc => "doc",
            Glyph::Folder => "folder",
            Glyph::Terminal => "terminal",
            Glyph::Gear => "gear",
            Glyph::Download => "download",
            Glyph::Check => "check",
            Glyph::Close => "close",
            Glyph::ChevronRight => "chevron-right",
            Glyph::Lock => "lock",
            Glyph::Refresh => "refresh",
            Glyph::Shield => "shield",
            Glyph::Display => "display",
            Glyph::Moon => "moon",
            Glyph::Restart => "restart",
            Glyph::Plus => "plus",
            Glyph::Minus => "minus",
            Glyph::Apps => "apps",
            Glyph::Dot => "dot",
        }
    }
}

fn arc_path(cx: f32, cy: f32, r: f32, a0: f32, a1: f32) -> Option<Path> {
    if r <= 0.0 {
        return None;
    }
    let (a0, a1) = (a0.to_radians(), a1.to_radians());
    let total = a1 - a0;
    if total.abs() < 0.0001 {
        return None;
    }
    // Cubic segments of at most 90 degrees, the standard kappa approximation
    // (the same construction `circle_path` uses for full circles).
    let segs = (total.abs() / std::f32::consts::FRAC_PI_2).ceil().max(1.0) as usize;
    let step = total / segs as f32;
    let handle = 4.0 / 3.0 * (step / 4.0).tan() * r;
    let mut pb = PathBuilder::new();
    pb.move_to(cx + r * a0.cos(), cy + r * a0.sin());
    for i in 0..segs {
        let s = a0 + step * i as f32;
        let e = s + step;
        let (cs, sn) = (s.cos(), s.sin());
        let (ce, sn2) = (e.cos(), e.sin());
        pb.cubic_to(
            cx + r * cs - handle * sn,
            cy + r * sn + handle * cs,
            cx + r * ce + handle * sn2,
            cy + r * sn2 - handle * ce,
            cx + r * ce,
            cy + r * sn2,
        );
    }
    pb.finish()
}

fn poly(points: &[(f32, f32)], close: bool) -> Option<Path> {
    if points.len() < 2 {
        return None;
    }
    let mut pb = PathBuilder::new();
    pb.move_to(points[0].0, points[0].1);
    for p in &points[1..] {
        pb.line_to(p.0, p.1);
    }
    if close {
        pb.close();
    }
    pb.finish()
}

fn scissors(pm: &mut Pixmap, path: Option<Path>, color: Color, w: f32) {
    if let Some(p) = path {
        stroke_path(pm, &p, color, w);
    }
}

fn brush(pm: &mut Pixmap, path: Option<Path>, color: Color) {
    if let Some(p) = path {
        fill_path(pm, &p, color);
    }
}

/// Draw a glyph scaled into `r`. `stroke` is in 24-grid units (1.9 is the
/// house weight, 2.4 for emphasis).
pub fn draw(pm: &mut Pixmap, g: Glyph, r: tiny_skia::Rect, color: Color, stroke: f32) {
    let s = r.width().min(r.height()) / 24.0;
    if s <= 0.0 {
        return;
    }
    let ox = r.x() + (r.width() - 24.0 * s) / 2.0;
    let oy = r.y() + (r.height() - 24.0 * s) / 2.0;
    let x = |u: f32| ox + u * s;
    let y = |v: f32| oy + v * s;
    let sw = stroke * s;
    // A sub-rect of the icon grid, handy for multi-part glyphs.
    let sub = |a: f32, b: f32, w: f32, h: f32| rect(x(a), y(b), w * s, h * s);

    match g {
        Glyph::Brand => {
            // A capital R built from the same strokes as the rest of the set:
            // stem, bowl, leg. The earlier abstract mark read as a "C" at small
            // sizes; this one is legible from 16 px in the bar to 190 px on the
            // desktop, which is what the identity has to survive.
            let w = sw * 1.25;
            scissors(pm, poly(&[(x(7.6), y(4.4)), (x(7.6), y(19.6))], false), color, w);
            scissors(pm, poly(&[(x(7.6), y(4.4)), (x(12.4), y(4.4))], false), color, w);
            scissors(pm, arc_path(x(12.4), y(8.4), 4.0 * s, -90.0, 90.0), color, w);
            scissors(pm, poly(&[(x(12.4), y(12.4)), (x(7.6), y(12.4))], false), color, w);
            scissors(pm, poly(&[(x(12.2), y(12.4)), (x(17.0), y(19.6))], false), color, w);
        }
        Glyph::Grid | Glyph::Apps => {
            for (a, b) in [(3.5, 3.5), (13.5, 3.5), (3.5, 13.5), (13.5, 13.5)] {
                round(pm, sub(a, b, 7.0, 7.0), 2.0 * s, color);
            }
        }
        Glyph::Search => {
            circle_stroke(pm, x(10.5), y(10.5), 6.0 * s, color, sw);
            scissors(pm, poly(&[(x(15.0), y(15.0)), (x(20.0), y(20.0))], false), color, sw);
        }
        Glyph::Wifi => {
            scissors(pm, arc_path(x(12.0), y(19.0), 9.0 * s, 205.0, 335.0), color, sw);
            scissors(pm, arc_path(x(12.0), y(19.0), 5.6 * s, 205.0, 335.0), color, sw);
            brush(pm, arc_path(x(12.0), y(19.0), 2.2 * s, 0.0, 360.0), color);
        }
        Glyph::WifiOff => {
            scissors(pm, arc_path(x(12.0), y(19.0), 9.0 * s, 205.0, 335.0), color, sw);
            brush(pm, arc_path(x(12.0), y(19.0), 2.2 * s, 0.0, 360.0), color);
            scissors(pm, poly(&[(x(4.0), y(4.0)), (x(20.0), y(20.0))], false), color, sw);
        }
        Glyph::Bluetooth => {
            // The rune: a vertical spine plus two diagonals meeting it at the
            // top and bottom, drawn as three separate strokes.
            scissors(
                pm,
                poly(&[(x(12.0), y(2.8)), (x(12.0), y(21.2))], false),
                color,
                sw * 0.95,
            );
            scissors(
                pm,
                poly(&[(x(12.0), y(2.8)), (x(17.2), y(7.2)), (x(7.2), y(15.2))], false),
                color,
                sw * 0.95,
            );
            scissors(
                pm,
                poly(&[(x(12.0), y(21.2)), (x(17.2), y(16.8)), (x(7.2), y(8.8))], false),
                color,
                sw * 0.95,
            );
        }
        Glyph::Volume | Glyph::VolumeMute => {
            let mut pb = PathBuilder::new();
            pb.move_to(x(3.5), y(9.5));
            pb.line_to(x(7.0), y(9.5));
            pb.line_to(x(11.5), y(5.0));
            pb.line_to(x(11.5), y(19.0));
            pb.line_to(x(7.0), y(14.5));
            pb.line_to(x(3.5), y(14.5));
            pb.close();
            brush(pm, pb.finish(), color);
            if g == Glyph::Volume {
                scissors(pm, arc_path(x(13.0), y(12.0), 4.2 * s, -58.0, 58.0), color, sw * 0.9);
                scissors(pm, arc_path(x(13.0), y(12.0), 7.4 * s, -52.0, 52.0), color, sw * 0.9);
            } else {
                scissors(
                    pm,
                    poly(&[(x(15.0), y(9.5)), (x(20.5), y(15.0))], false),
                    color,
                    sw * 0.9,
                );
                scissors(
                    pm,
                    poly(&[(x(20.5), y(9.5)), (x(15.0), y(15.0))], false),
                    color,
                    sw * 0.9,
                );
            }
        }
        Glyph::Battery => {
            round_stroke(pm, sub(2.5, 8.0, 17.0, 8.5), 2.2 * s, color, sw * 0.85);
            round(pm, sub(20.6, 10.5, 1.9, 3.5), 0.8 * s, color);
        }
        Glyph::Power => {
            scissors(pm, arc_path(x(12.0), y(13.0), 7.6 * s, 300.0, 600.0), color, sw);
            scissors(pm, poly(&[(x(12.0), y(3.2)), (x(12.0), y(11.5))], false), color, sw);
        }
        Glyph::Globe => {
            circle_stroke(pm, x(12.0), y(12.0), 8.2 * s, color, sw * 0.9);
            scissors(pm, poly(&[(x(3.8), y(12.0)), (x(20.2), y(12.0))], false), color, sw * 0.8);
            scissors(pm, arc_path(x(12.0), y(12.0), 8.2 * s, -42.0, 42.0), color, sw * 0.5);
            scissors(pm, arc_path(x(12.0), y(12.0), 8.2 * s, 138.0, 222.0), color, sw * 0.5);
            // Upper meridian, the line that makes it read as a globe.
            scissors(pm, arc_path(x(12.0), y(12.0), 4.4 * s, 0.0, 360.0), color, sw * 0.5);
        }
        Glyph::Envelope => {
            round_stroke(pm, sub(2.8, 5.6, 18.4, 12.8), 2.2 * s, color, sw * 0.9);
            scissors(
                pm,
                poly(&[(x(3.6), y(7.4)), (x(12.0), y(13.4)), (x(20.4), y(7.4))], false),
                color,
                sw * 0.9,
            );
        }
        Glyph::Cloud => {
            scissors(pm, poly(&[(x(5.4), y(17.8)), (x(18.6), y(17.8))], false), color, sw * 0.9);
            scissors(pm, arc_path(x(9.6), y(13.4), 4.4 * s, 90.0, 270.0), color, sw * 0.9);
            scissors(pm, arc_path(x(13.4), y(10.6), 4.8 * s, 180.0, 360.0), color, sw * 0.9);
            scissors(pm, arc_path(x(16.0), y(13.4), 4.4 * s, 270.0, 450.0), color, sw * 0.9);
        }
        Glyph::Chat => {
            round_stroke(pm, sub(2.8, 4.6, 18.4, 12.6), 4.6 * s, color, sw * 0.9);
            scissors(
                pm,
                poly(&[(x(7.8), y(17.0)), (x(7.8), y(20.8)), (x(12.2), y(17.0))], false),
                color,
                sw * 0.9,
            );
        }
        Glyph::Doc => {
            round_stroke(pm, sub(5.2, 2.8, 13.6, 18.4), 2.4 * s, color, sw * 0.9);
            for (from, to) in [(8.4, 15.6), (12.0, 15.6), (15.6, 12.6)] {
                scissors(
                    pm,
                    poly(&[(x(8.6), y(from)), (x(to), y(from))], false),
                    color,
                    sw * 0.7,
                );
            }
        }
        Glyph::Folder => {
            let mut pb = PathBuilder::new();
            pb.move_to(x(3.0), y(19.0));
            pb.line_to(x(3.0), y(6.5));
            pb.quad_to(x(3.0), y(5.5), x(4.2), y(5.5));
            pb.line_to(x(9.2), y(5.5));
            pb.line_to(x(11.4), y(8.2));
            pb.line_to(x(19.8), y(8.2));
            pb.quad_to(x(21.0), y(8.2), x(21.0), y(9.4));
            pb.line_to(x(21.0), y(18.0));
            pb.quad_to(x(21.0), y(19.0), x(19.8), y(19.0));
            pb.close();
            scissors(pm, pb.finish(), color, sw * 0.9);
        }
        Glyph::Terminal => {
            round_stroke(pm, sub(2.5, 4.5, 19.0, 15.0), 2.6 * s, color, sw * 0.85);
            scissors(
                pm,
                poly(&[(x(6.5), y(9.5)), (x(9.8), y(12.3)), (x(6.5), y(15.0))], false),
                color,
                sw * 0.85,
            );
            scissors(pm, poly(&[(x(12.0), y(15.2)), (x(16.5), y(15.2))], false), color, sw * 0.85);
        }
        Glyph::Gear => {
            circle_stroke(pm, x(12.0), y(12.0), 7.4 * s, color, sw * 0.85);
            circle_stroke(pm, x(12.0), y(12.0), 3.2 * s, color, sw * 0.85);
            for i in 0..8 {
                let a = (i as f32) * 45.0_f32.to_radians();
                let (c, si) = (a.cos(), a.sin());
                scissors(
                    pm,
                    poly(
                        &[
                            (x(12.0 + c * 6.4), y(12.0 + si * 6.4)),
                            (x(12.0 + c * 9.4), y(12.0 + si * 9.4)),
                        ],
                        false,
                    ),
                    color,
                    sw * 0.85,
                );
            }
        }
        Glyph::Download => {
            scissors(pm, poly(&[(x(12.0), y(3.5)), (x(12.0), y(15.0))], false), color, sw);
            scissors(
                pm,
                poly(&[(x(6.5), y(10.0)), (x(12.0), y(15.5)), (x(17.5), y(10.0))], false),
                color,
                sw,
            );
            scissors(pm, poly(&[(x(4.5), y(20.0)), (x(19.5), y(20.0))], false), color, sw);
        }
        Glyph::Check => {
            scissors(
                pm,
                poly(&[(x(4.5), y(12.5)), (x(9.8), y(18.0)), (x(19.5), y(6.0))], false),
                color,
                sw * 1.1,
            );
        }
        Glyph::Close => {
            scissors(pm, poly(&[(x(6.0), y(6.0)), (x(18.0), y(18.0))], false), color, sw);
            scissors(pm, poly(&[(x(18.0), y(6.0)), (x(6.0), y(18.0))], false), color, sw);
        }
        Glyph::ChevronRight => {
            scissors(
                pm,
                poly(&[(x(9.0), y(4.5)), (x(16.5), y(12.0)), (x(9.0), y(19.5))], false),
                color,
                sw,
            );
        }
        Glyph::Lock => {
            round_stroke(pm, sub(4.8, 10.5, 14.4, 9.6), 2.2 * s, color, sw * 0.85);
            scissors(pm, arc_path(x(12.0), y(10.6), 4.3 * s, 180.0, 360.0), color, sw * 0.85);
        }
        Glyph::Refresh | Glyph::Restart => {
            scissors(pm, arc_path(x(12.0), y(12.0), 7.6 * s, -50.0, 215.0), color, sw);
            let mut pb = PathBuilder::new();
            pb.move_to(x(16.6), y(2.6));
            pb.line_to(x(20.4), y(7.6));
            pb.line_to(x(14.6), y(8.4));
            pb.close();
            brush(pm, pb.finish(), color);
        }
        Glyph::Shield => {
            let mut pb = PathBuilder::new();
            pb.move_to(x(12.0), y(2.6));
            pb.line_to(x(20.0), y(6.0));
            pb.line_to(x(20.0), y(12.0));
            pb.quad_to(x(20.0), y(18.4), x(12.0), y(21.6));
            pb.quad_to(x(4.0), y(18.4), x(4.0), y(12.0));
            pb.line_to(x(4.0), y(6.0));
            pb.close();
            scissors(pm, pb.finish(), color, sw * 0.85);
        }
        Glyph::Display => {
            round_stroke(pm, sub(2.5, 4.0, 19.0, 12.5), 2.2 * s, color, sw * 0.85);
            scissors(
                pm,
                poly(&[(x(9.0), y(20.0)), (x(15.0), y(20.0))], false),
                color,
                sw * 0.85,
            );
            scissors(
                pm,
                poly(&[(x(12.0), y(16.5)), (x(12.0), y(20.0))], false),
                color,
                sw * 0.85,
            );
        }
        Glyph::Moon => {
            // A crescent built from two quadratics: the outer edge bulges to
            // the left, the inner edge follows it less far.
            let mut pb = PathBuilder::new();
            pb.move_to(x(12.0), y(3.2));
            pb.quad_to(x(1.6), y(12.0), x(12.0), y(20.8));
            pb.quad_to(x(8.6), y(12.0), x(12.0), y(3.2));
            pb.close();
            brush(pm, pb.finish(), color);
        }
        Glyph::Plus => {
            scissors(pm, poly(&[(x(12.0), y(5.0)), (x(12.0), y(19.0))], false), color, sw);
            scissors(pm, poly(&[(x(5.0), y(12.0)), (x(19.0), y(12.0))], false), color, sw);
        }
        Glyph::Minus => {
            scissors(pm, poly(&[(x(5.0), y(12.0)), (x(19.0), y(12.0))], false), color, sw);
        }
        Glyph::Dot => {
            circle(pm, x(12.0), y(12.0), 3.4 * s, color);
        }
    }
}

/// Battery with a fill level (0..=1) and optional charging bolt.
pub fn draw_battery(pm: &mut Pixmap, r: tiny_skia::Rect, color: Color, level: f32, charging: bool) {
    let s = r.width().min(r.height()) / 24.0;
    if s <= 0.0 {
        return;
    }
    let ox = r.x() + (r.width() - 24.0 * s) / 2.0;
    let oy = r.y() + (r.height() - 24.0 * s) / 2.0;
    let body = rect(ox + 2.5 * s, oy + 8.0 * s, 17.0 * s, 8.5 * s);
    let fill_color = if level <= 0.15 { crate::theme::clay() } else { color };
    round_stroke(pm, body, 2.2 * s, color, 1.7 * s);
    round(pm, rect(ox + 20.6 * s, oy + 10.5 * s, 1.9 * s, 3.5 * s), 0.8 * s, color);
    let inner_w = (body.width() - 3.4 * s) * level.clamp(0.02, 1.0);
    round(
        pm,
        rect(body.x() + 1.7 * s, body.y() + 1.7 * s, inner_w, body.height() - 3.4 * s),
        1.0 * s,
        fill_color,
    );
    if charging {
        let mut pb = PathBuilder::new();
        pb.move_to(ox + 12.6 * s, oy + 9.0 * s);
        pb.line_to(ox + 8.6 * s, oy + 13.6 * s);
        pb.line_to(ox + 12.0 * s, oy + 13.6 * s);
        pb.line_to(ox + 10.6 * s, oy + 17.6 * s);
        pb.line_to(ox + 15.6 * s, oy + 12.2 * s);
        pb.line_to(ox + 12.2 * s, oy + 12.2 * s);
        pb.close();
        if let Some(p) = pb.finish() {
            let mut paint = Paint::default();
            paint.shader = Shader::SolidColor(crate::theme::card());
            paint.anti_alias = true;
            pm.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
        }
    }
}
