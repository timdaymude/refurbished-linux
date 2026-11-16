//! Tiny drawing layer on top of `tiny-skia`.
//!
//! Everything the shell shows is composed from rounded rectangles, hairlines,
//! circles, vector paths and glyphs. All of it is CPU-rasterised once per
//! frame, with no GPU context, no compositor effects and no extra processes.

use tiny_skia::{
    Color, FillRule, GradientStop, LineCap, LineJoin, LinearGradient, Paint, Path, PathBuilder,
    Pixmap, Point, PremultipliedColorU8, Rect, Shader, SpreadMode, Stroke, Transform,
};

pub fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::from_xywh(x, y, w.max(0.0), h.max(0.0)).unwrap_or_else(|| {
        Rect::from_xywh(0.0, 0.0, 0.0, 0.0).expect("zero rect is always valid")
    })
}

pub fn contains(r: Rect, x: f32, y: f32) -> bool {
    x >= r.x() && x <= r.right() && y >= r.y() && y <= r.bottom()
}

fn paint(color: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.shader = Shader::SolidColor(color);
    p.anti_alias = true;
    p
}

fn stroke(width: f32) -> Stroke {
    Stroke {
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    }
}

pub fn round_rect_path(r: Rect, radius: f32) -> Option<Path> {
    let (x, y, w, h) = (r.x(), r.y(), r.width(), r.height());
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let rad = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + rad, y);
    pb.line_to(x + w - rad, y);
    pb.quad_to(x + w, y, x + w, y + rad);
    pb.line_to(x + w, y + h - rad);
    pb.quad_to(x + w, y + h, x + w - rad, y + h);
    pb.line_to(x + rad, y + h);
    pb.quad_to(x, y + h, x, y + h - rad);
    pb.line_to(x, y + rad);
    pb.quad_to(x, y, x + rad, y);
    pb.close();
    pb.finish()
}

pub fn circle_path(cx: f32, cy: f32, radius: f32) -> Option<Path> {
    // Four cubic segments approximating a circle.
    const K: f32 = 0.552_284_8;
    let r = radius.max(0.0);
    if r == 0.0 {
        return None;
    }
    let mut pb = PathBuilder::new();
    pb.move_to(cx, cy - r);
    pb.cubic_to(cx + K * r, cy - r, cx + r, cy - K * r, cx + r, cy);
    pb.cubic_to(cx + r, cy + K * r, cx + K * r, cy + r, cx, cy + r);
    pb.cubic_to(cx - K * r, cy + r, cx - r, cy + K * r, cx - r, cy);
    pb.cubic_to(cx - r, cy - K * r, cx - K * r, cy - r, cx, cy - r);
    pb.close();
    pb.finish()
}

pub fn fill_path(pm: &mut Pixmap, path: &Path, color: Color) {
    pm.fill_path(path, &paint(color), FillRule::Winding, Transform::identity(), None);
}

pub fn fill_path_even_odd(pm: &mut Pixmap, path: &Path, color: Color) {
    pm.fill_path(path, &paint(color), FillRule::EvenOdd, Transform::identity(), None);
}

pub fn stroke_path(pm: &mut Pixmap, path: &Path, color: Color, width: f32) {
    pm.stroke_path(path, &paint(color), &stroke(width), Transform::identity(), None);
}

pub fn fill(pm: &mut Pixmap, r: Rect, color: Color) {
    if let Some(path) = round_rect_path(r, 0.0) {
        fill_path(pm, &path, color);
    }
}

pub fn round(pm: &mut Pixmap, r: Rect, radius: f32, color: Color) {
    if let Some(path) = round_rect_path(r, radius) {
        fill_path(pm, &path, color);
    }
}

pub fn round_stroke(pm: &mut Pixmap, r: Rect, radius: f32, color: Color, width: f32) {
    if let Some(path) = round_rect_path(r, radius) {
        stroke_path(pm, &path, color, width);
    }
}

pub fn circle(pm: &mut Pixmap, cx: f32, cy: f32, radius: f32, color: Color) {
    if let Some(path) = circle_path(cx, cy, radius) {
        fill_path(pm, &path, color);
    }
}

pub fn circle_stroke(pm: &mut Pixmap, cx: f32, cy: f32, radius: f32, color: Color, width: f32) {
    if let Some(path) = circle_path(cx, cy, radius) {
        stroke_path(pm, &path, color, width);
    }
}

pub fn hline(pm: &mut Pixmap, x: f32, y: f32, w: f32, color: Color) {
    fill(pm, rect(x, y, w, 1.0), color);
}

pub fn vline(pm: &mut Pixmap, x: f32, y: f32, h: f32, color: Color) {
    fill(pm, rect(x, y, 1.0, h), color);
}

pub fn line(pm: &mut Pixmap, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1);
    pb.line_to(x2, y2);
    if let Some(path) = pb.finish() {
        stroke_path(pm, &path, color, width);
    }
}

/// Manual source-over blend of a single pixel, used for glyph coverage masks.
pub fn blend(pm: &mut Pixmap, x: i32, y: i32, color: Color, coverage: f32) {
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    if x < 0 || y < 0 || x >= w || y >= h {
        return;
    }
    let a = (coverage * color.alpha()).clamp(0.0, 1.0);
    if a <= 0.0 {
        return;
    }
    let data = pm.pixels_mut();
    let idx = (y * w + x) as usize;
    let dst = data[idx];
    let inv = 1.0 - a;
    let r = (dst.red() as f32 * inv + color.red() * 255.0 * a).round() as u8;
    let g = (dst.green() as f32 * inv + color.green() * 255.0 * a).round() as u8;
    let b = (dst.blue() as f32 * inv + color.blue() * 255.0 * a).round() as u8;
    let al = (dst.alpha() as f32 * inv + 255.0 * a).round() as u8;
    data[idx] = PremultipliedColorU8::from_rgba(
        r.min(al),
        g.min(al),
        b.min(al),
        al,
    )
    .unwrap_or(dst);
}

/// Mix two colours, `t` in 0..=1.
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::from_rgba(
        a.red() + (b.red() - a.red()) * t,
        a.green() + (b.green() - a.green()) * t,
        a.blue() + (b.blue() - a.blue()) * t,
        a.alpha() + (b.alpha() - a.alpha()) * t,
    )
    .unwrap_or(a)
}

/// Diagonal two-colour gradient, used for the desktop background.
pub fn fill_gradient(pm: &mut Pixmap, r: Rect, from: Color, to: Color) {
    let shader = LinearGradient::new(
        Point::from_xy(r.x(), r.y()),
        Point::from_xy(r.right(), r.bottom()),
        vec![
            GradientStop::new(0.0, from),
            GradientStop::new(1.0, to),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    );
    match shader {
        Some(shader) => {
            let mut p = Paint::default();
            p.shader = shader;
            p.anti_alias = true;
            pm.fill_rect(r, &p, Transform::identity(), None);
        }
        None => fill(pm, r, from),
    }
}

/// Vertical soft shade used on cards instead of a drop shadow.
pub fn top_sheen(pm: &mut Pixmap, r: Rect, radius: f32, color: Color) {
    round(pm, rect(r.x(), r.y(), r.width(), (r.height() * 0.5).min(28.0)), radius, color);
}
