//! Shared controls. Every control is drawn by the shell and registers its own
//! hit region, so input handling never has to know about layout.

use tiny_skia::{Color, Pixmap, Rect};

use crate::text::{MEDIUM, REGULAR, SEMIBOLD};
use crate::{icons, render, theme};

use super::{Action, State};

pub struct Ctx<'a> {
    pub st: &'a State,
    pub text: &'a mut crate::text::TextEngine,
    pub pm: &'a mut Pixmap,
    pub hits: &'a mut Vec<(Rect, Action)>,
    pub hover: Option<(f32, f32)>,
}

impl<'a> Ctx<'a> {
    pub fn hot(&self, r: Rect) -> bool {
        self.hover
            .map(|(x, y)| render::contains(r, x, y))
            .unwrap_or(false)
    }

    /// Register a clickable region. `Action::Nothing` swallows the click,
    /// which is how a card stops a background "click outside to close" region
    /// from firing. Later regions take priority over earlier ones.
    pub fn hit(&mut self, r: Rect, a: Action) {
        self.hits.push((r, a));
    }

    /// Resolve a click against the hit regions, last registered first.
    pub fn resolve(&self, x: f32, y: f32) -> Option<Action> {
        self.hits
            .iter()
            .rev()
            .find(|(r, _)| render::contains(*r, x, y))
            .map(|(_, a)| a.clone())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn t(
        &mut self,
        x: f32,
        top: f32,
        h: f32,
        s: &str,
        px: f32,
        weight: u8,
        color: Color,
    ) -> f32 {
        self.text.text_left(self.pm, x, top, h, s, px, weight, color)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn tc(
        &mut self,
        x: f32,
        w: f32,
        top: f32,
        h: f32,
        s: &str,
        px: f32,
        weight: u8,
        color: Color,
    ) {
        self.text
            .text_center(self.pm, x, w, top, h, s, px, weight, color)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn tr(
        &mut self,
        right: f32,
        top: f32,
        h: f32,
        s: &str,
        px: f32,
        weight: u8,
        color: Color,
    ) {
        self.text
            .text_right(self.pm, right, top, h, s, px, weight, color)
    }

    pub fn measure(&mut self, s: &str, px: f32, weight: u8) -> f32 {
        self.text.width(s, px, weight)
    }

    pub fn ellip(&mut self, s: &str, px: f32, weight: u8, max_w: f32) -> String {
        self.text.ellipsize(s, px, weight, max_w)
    }

    pub fn glyph(&mut self, g: icons::Glyph, r: Rect, color: Color, stroke: f32) {
        icons::draw(self.pm, g, r, color, stroke);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

pub fn panel(cx: &mut Ctx, r: Rect) {
    render::round(cx.pm, r, theme::RADIUS_LG, theme::card());
    render::round_stroke(cx.pm, r, theme::RADIUS_LG, theme::line(), 1.0);
}

pub fn button(cx: &mut Ctx, r: Rect, label: &str, style: Style, action: Action) {
    let hot = cx.hot(r);
    let (bg, fg) = match style {
        Style::Primary => (
            if hot { theme::accent_deep() } else { theme::accent() },
            theme::card(),
        ),
        Style::Secondary => (
            if hot { theme::accent_soft() } else { theme::well() },
            theme::accent_deep(),
        ),
        Style::Ghost => (
            if hot { theme::line_soft() } else { Color::TRANSPARENT },
            theme::ink_soft(),
        ),
        Style::Danger => (
            if hot { theme::clay() } else { theme::clay_soft() },
            if hot { theme::card() } else { theme::clay() },
        ),
    };
    if bg != Color::TRANSPARENT {
        render::round(cx.pm, r, theme::RADIUS_SM, bg);
    }
    if style == Style::Secondary {
        render::round_stroke(cx.pm, r, theme::RADIUS_SM, theme::line(), 1.0);
    }
    cx.tc(
        r.x(),
        r.width(),
        r.y(),
        r.height(),
        label,
        theme::T_SMALL,
        MEDIUM,
        fg,
    );
    cx.hit(r, action);
}

pub fn icon_button(cx: &mut Ctx, r: Rect, glyph: icons::Glyph, action: Action, on: bool) {
    let hot = cx.hot(r);
    if hot {
        render::round(cx.pm, r, theme::RADIUS_SM, theme::line_soft());
    }
    let color = if on {
        theme::accent_deep()
    } else if hot {
        theme::ink()
    } else {
        theme::ink_soft()
    };
    let inset = (r.width().min(r.height()) * 0.22).min(6.0);
    let inner = render::rect(
        r.x() + inset,
        r.y() + inset,
        r.width() - inset * 2.0,
        r.height() - inset * 2.0,
    );
    cx.glyph(glyph, inner, color, 1.9);
    cx.hit(r, action);
}

/// Search / text input. `caret` draws the blinking insertion point.
pub fn search_field(cx: &mut Ctx, r: Rect, value: &str, placeholder: &str, focused: bool) {
    let bg = if focused { theme::card() } else { theme::well() };
    render::round(cx.pm, r, theme::RADIUS_SM, bg);
    render::round_stroke(
        cx.pm,
        r,
        theme::RADIUS_SM,
        if focused { theme::accent() } else { theme::line() },
        1.0,
    );
    let icon_box = render::rect(r.x() + 8.0, r.y() + (r.height() - 16.0) / 2.0, 16.0, 16.0);
    cx.glyph(icons::Glyph::Search, icon_box, theme::ink_faint(), 2.0);
    let text_x = icon_box.right() + 8.0;
    let avail = r.right() - text_x - 10.0;
    if value.is_empty() {
        cx.t(
            text_x,
            r.y(),
            r.height(),
            placeholder,
            theme::T_BODY,
            REGULAR,
            theme::ink_faint(),
        );
    } else {
        let shown = cx.ellip(value, theme::T_BODY, REGULAR, avail);
        let w = cx.t(
            text_x,
            r.y(),
            r.height(),
            &shown,
            theme::T_BODY,
            REGULAR,
            theme::ink(),
        );
        if focused {
            render::fill(
                cx.pm,
                render::rect(text_x + w + 1.0, r.y() + r.height() * 0.28, 1.5, r.height() * 0.44),
                theme::accent(),
            );
        }
    }
}

/// A large square toggle (Wi-Fi, Bluetooth).
pub fn toggle_tile(
    cx: &mut Ctx,
    r: Rect,
    glyph: icons::Glyph,
    label: &str,
    sub: &str,
    on: bool,
    action: Action,
) {
    let hot = cx.hot(r);
    let bg = if on {
        if hot { theme::accent_soft() } else { theme::mix(theme::accent_soft(), theme::card(), 0.35) }
    } else if hot {
        theme::line_soft()
    } else {
        theme::well()
    };
    render::round(cx.pm, r, theme::RADIUS, bg);
    render::round_stroke(
        cx.pm,
        r,
        theme::RADIUS,
        if on { theme::mix(theme::accent(), theme::accent_soft(), 0.55) } else { theme::line() },
        1.0,
    );
    let icon_box = render::rect(r.x() + 12.0, r.y() + 12.0, 20.0, 20.0);
    cx.glyph(
        glyph,
        icon_box,
        if on { theme::accent_deep() } else { theme::ink_soft() },
        1.9,
    );
    cx.t(
        r.x() + 40.0,
        r.y() + 8.0,
        20.0,
        label,
        theme::T_BODY,
        MEDIUM,
        if on { theme::accent_deep() } else { theme::ink() },
    );
    cx.t(
        r.x() + 12.0,
        r.y() + 34.0,
        18.0,
        sub,
        theme::T_SMALL,
        REGULAR,
        theme::ink_soft(),
    );
    // State pip, top right.
    render::circle(
        cx.pm,
        r.right() - 16.0,
        r.y() + 16.0,
        3.4,
        if on { theme::accent() } else { theme::ink_faint() },
    );
    cx.hit(r, action);
}

/// Horizontal slider; returns the track rect so the session can support drag.
#[allow(clippy::too_many_arguments)]
pub fn slider(
    cx: &mut Ctx,
    r: Rect,
    glyph: icons::Glyph,
    value: f32,
    _kind: super::SliderKind,
    action_for: impl Fn(f32) -> Action,
) -> Rect {
    let icon_box = render::rect(r.x(), r.y() + (r.height() - 18.0) / 2.0, 18.0, 18.0);
    cx.glyph(glyph, icon_box, theme::ink_soft(), 1.9);
    let track = render::rect(r.x() + 30.0, r.y() + r.height() / 2.0 - 3.0, r.width() - 30.0, 6.0);
    render::round(cx.pm, track, 3.0, theme::well());
    let filled = (track.width() * value.clamp(0.0, 1.0)).max(0.0);
    render::round(
        cx.pm,
        render::rect(track.x(), track.y(), filled, track.height()),
        3.0,
        theme::accent(),
    );
    let knob_x = track.x() + filled;
    render::circle(cx.pm, knob_x, track.y() + 3.0, 7.5, theme::card());
    render::circle_stroke(cx.pm, knob_x, track.y() + 3.0, 7.5, theme::accent(), 1.6);
    // Whole row is clickable; the session converts x within `track` to a value.
    cx.hit(track, action_for(value));
    track
}

/// One row in a list. Returns nothing; callers own selection state.
#[allow(clippy::too_many_arguments)]
pub fn list_row(
    cx: &mut Ctx,
    r: Rect,
    glyph: icons::Glyph,
    title: &str,
    sub: &str,
    selected: bool,
    action: Action,
) {
    let hot = cx.hot(r) || selected;
    if hot {
        render::round(cx.pm, r, theme::RADIUS_SM, if selected { theme::accent_soft() } else { theme::line_soft() });
    }
    let chip = render::rect(r.x() + 8.0, r.y() + (r.height() - 26.0) / 2.0, 26.0, 26.0);
    render::round(
        cx.pm,
        chip,
        theme::RADIUS_SM,
        if selected { theme::card() } else { theme::well() },
    );
    cx.glyph(
        glyph,
        render::rect(chip.x() + 5.0, chip.y() + 5.0, 16.0, 16.0),
        if selected { theme::accent_deep() } else { theme::ink_soft() },
        1.9,
    );
    let text_x = chip.right() + 10.0;
    let title_w = r.width() - (text_x - r.x()) - 16.0;
    let t = cx.ellip(title, theme::T_BODY, MEDIUM, title_w);
    cx.t(
        text_x,
        r.y() + if sub.is_empty() { 0.0 } else { 3.0 },
        if sub.is_empty() { r.height() } else { r.height() * 0.55 },
        &t,
        theme::T_BODY,
        MEDIUM,
        theme::ink(),
    );
    if !sub.is_empty() {
        let s = cx.ellip(sub, theme::T_SMALL, REGULAR, title_w);
        cx.t(
            text_x,
            r.y() + r.height() * 0.46,
            r.height() * 0.5,
            &s,
            theme::T_SMALL,
            REGULAR,
            theme::ink_faint(),
        );
    }
    cx.hit(r, action);
}

/// Small rounded label. Returns its width.
pub fn badge(cx: &mut Ctx, x: f32, y: f32, text: &str, fg: Color, bg: Color) -> f32 {
    let tw = cx.measure(text, theme::T_SMALL - 1.0, MEDIUM);
    let w = tw + 14.0;
    let h = 17.0;
    render::round(cx.pm, render::rect(x, y, w, h), h / 2.0, bg);
    cx.tc(x, w, y, h, text, theme::T_SMALL - 1.0, MEDIUM, fg);
    w
}

/// Badge anchored to its right edge, so it never runs past the panel it sits in.
pub fn badge_right(cx: &mut Ctx, right: f32, y: f32, text: &str, fg: Color, bg: Color) -> f32 {
    let w = cx.measure(text, theme::T_SMALL - 1.0, MEDIUM) + 14.0;
    badge(cx, right - w, y, text, fg, bg);
    w
}

pub fn divider(cx: &mut Ctx, x: f32, y: f32, w: f32) {
    render::hline(cx.pm, x, y, w, theme::line_soft());
}

pub fn section_label(cx: &mut Ctx, x: f32, y: f32, text: &str) {
    cx.t(
        x,
        y,
        16.0,
        &text.to_uppercase(),
        theme::T_SMALL - 1.0,
        SEMIBOLD,
        theme::ink_faint(),
    );
}

/// Window title bar. Returns the draggable area.
pub fn titlebar(cx: &mut Ctx, w: f32, title: &str, kind_glyph: icons::Glyph) -> Rect {
    let h = 42.0;
    render::round(cx.pm, render::rect(0.0, 0.0, w, h), theme::RADIUS_LG, theme::panel());
    render::fill(cx.pm, render::rect(0.0, h - 1.0, w, 1.0), theme::line_soft());
    let chip = render::rect(14.0, 11.0, 20.0, 20.0);
    render::round(cx.pm, chip, 6.0, theme::accent());
    cx.glyph(
        kind_glyph,
        render::rect(chip.x() + 3.0, chip.y() + 3.0, 14.0, 14.0),
        theme::card(),
        2.1,
    );
    cx.t(44.0, 0.0, h, title, theme::T_TITLE, SEMIBOLD, theme::ink());
    // Close button is drawn (and hit-registered) by the caller on top of this
    // area; hit regions are resolved last-to-first so it wins over the drag bar.
    let close = render::rect(w - 38.0, (h - 26.0) / 2.0, 26.0, 26.0);
    icon_button(cx, close, icons::Glyph::Close, Action::Nothing, false);
    render::rect(0.0, 0.0, w - 46.0, h)
}

pub fn empty_state(cx: &mut Ctx, r: Rect, title: &str, body: &str) {
    let g = (r.width().min(r.height()) * 0.16).clamp(28.0, 54.0);
    let area = render::rect(r.x() + (r.width() - g) / 2.0, r.y() + r.height() * 0.22, g, g);
    cx.glyph(icons::Glyph::Search, area, theme::line(), 1.6);
    cx.tc(
        r.x(),
        r.width(),
        area.bottom() + 12.0,
        24.0,
        title,
        theme::T_LABEL,
        MEDIUM,
        theme::ink_soft(),
    );
    cx.tc(
        r.x() + 20.0,
        r.width() - 40.0,
        area.bottom() + 38.0,
        20.0,
        body,
        theme::T_SMALL,
        REGULAR,
        theme::ink_faint(),
    );
}
