//! The application launcher.
//!
//! A centred card over a dimmed desktop: search field, pinned application
//! grid, and live results. Web applications (Microsoft 365, Outlook, OneDrive,
//! Teams) live in the same grid as local ones, because for the people this
//! system is for they are simply "apps".

use tiny_skia::Rect;

use crate::apps::{self, App as DesktopApp};
use crate::text::{MEDIUM, REGULAR};
use crate::{icons, render, theme};

use super::widgets::{self, Ctx};
use super::{Action, Focus};

pub fn draw(cx: &mut Ctx, w: f32, h: f32) {
    let st = cx.st;
    render::fill(cx.pm, render::rect(0.0, 0.0, w, h), theme::scrim());
    // Clicking the dimmed area (or the card's own background) closes the
    // launcher; child hit regions registered later win over this one.
    cx.hit(render::rect(0.0, 0.0, w, h), Action::LauncherClose);

    let cw = (w - 128.0).clamp(420.0, 800.0);
    let ch = (h - 110.0).clamp(330.0, 560.0);
    let card = render::rect(
        (w - cw) / 2.0,
        ((h - ch) / 2.0) - 16.0,
        cw,
        ch,
    );
    widgets::panel(cx, card);
    cx.hit(card, Action::Nothing);

    // ---- search ----------------------------------------------------------
    let field = render::rect(card.x() + 18.0, card.y() + 16.0, card.width() - 36.0, 40.0);
    widgets::search_field(
        cx,
        field,
        &st.query,
        "Search applications, or type a web address",
        st.focus == Focus::Launcher,
    );
    cx.hit(field, Action::Nothing);

    // ---- body ------------------------------------------------------------
    let body = render::rect(
        card.x() + 18.0,
        field.bottom() + 16.0,
        card.width() - 36.0,
        card.bottom() - field.bottom() - 58.0,
    );

    if st.query.is_empty() {
        pinned_grid(cx, body);
    } else {
        results_list(cx, body);
    }

    // ---- footer ----------------------------------------------------------
    render::hline(
        cx.pm,
        card.x() + 18.0,
        card.bottom() - 34.0,
        card.width() - 36.0,
        theme::line_soft(),
    );
    let hints = "Enter  open     Esc  close     Super  launcher";
    cx.t(
        card.x() + 18.0,
        card.bottom() - 32.0,
        28.0,
        hints,
        theme::T_SMALL - 0.5,
        REGULAR,
        theme::ink_faint(),
    );
    let count = format!("{} installed", st.apps.len());
    cx.tr(
        card.right() - 18.0,
        card.bottom() - 32.0,
        28.0,
        &count,
        theme::T_SMALL - 0.5,
        REGULAR,
        theme::ink_faint(),
    );
}

fn pinned_grid(cx: &mut Ctx, area: Rect) {
    let st = cx.st;
    let pins = st.pinned.clone();
    if pins.is_empty() {
        widgets::empty_state(
            cx,
            area,
            "No pinned applications",
            "Open the software manager to install something.",
        );
        return;
    }
    let cols = 5usize;
    let gap = 10.0;
    let tile_w = (area.width() - gap * (cols as f32 - 1.0)) / cols as f32;
    let tile_h = 92.0;
    let rows = ((area.height() + gap) / (tile_h + gap)).floor().max(1.0) as usize;

    for (i, idx) in pins.iter().enumerate() {
        let row = i / cols;
        let col = i % cols;
        if row >= rows {
            break;
        }
        let r = render::rect(
            area.x() + col as f32 * (tile_w + gap),
            area.y() + row as f32 * (tile_h + gap),
            tile_w,
            tile_h,
        );
        let Some(app) = st.apps.get(*idx) else {
            continue;
        };
        tile(cx, r, app, *idx);
    }
}

fn tile(cx: &mut Ctx, r: Rect, app: &DesktopApp, idx: usize) {
    let hot = cx.hot(r);
    let is_hero = app.name.eq_ignore_ascii_case("brave");
    let base = if app.is_web() { theme::accent_soft() } else { theme::well() };
    render::round(
        cx.pm,
        r,
        theme::RADIUS,
        if hot { theme::mix(base, theme::accent(), 0.12) } else { base },
    );
    if hot {
        render::round_stroke(cx.pm, r, theme::RADIUS, theme::accent(), 1.0);
    }
    let side = 42.0;
    let icon_sq = render::rect(
        r.x() + (r.width() - side) / 2.0,
        r.y() + 12.0,
        side,
        side,
    );
    let (bg, fg) = if is_hero {
        (theme::accent(), theme::card())
    } else if app.is_web() {
        (theme::card(), theme::accent_deep())
    } else {
        (theme::mix(theme::well(), theme::ink_faint(), 0.12), theme::ink())
    };
    render::round(cx.pm, icon_sq, 11.0, bg);
    let g = super::glyph_for(app);
    icons::draw(
        cx.pm,
        g,
        render::rect(icon_sq.x() + 10.0, icon_sq.y() + 10.0, side - 20.0, side - 20.0),
        fg,
        1.9,
    );
    let label = cx.ellip(&app.name, theme::T_SMALL, MEDIUM, r.width() - 12.0);
    cx.tc(
        r.x(),
        r.width(),
        icon_sq.bottom() + 6.0,
        18.0,
        &label,
        theme::T_SMALL,
        MEDIUM,
        theme::ink(),
    );
    cx.hit(r, Action::Launch(idx));
}

fn results_list(cx: &mut Ctx, area: Rect) {
    let st = cx.st;
    let results = apps::search(&st.apps, &st.query);
    if results.is_empty() {
        widgets::empty_state(
            cx,
            area,
            &format!("Nothing installed matches “{}”", st.query),
            "You can still look for it on the web, or install it from Software.",
        );
        let btn = render::rect(
            area.x() + area.width() / 2.0 - 90.0,
            area.y() + area.height() * 0.68,
            180.0,
            32.0,
        );
        widgets::button(cx, btn, "Search the web", widgets::Style::Primary, Action::SearchWeb);
        return;
    }

    let row_h = 42.0;
    let max_rows = ((area.height() - 42.0) / row_h).floor().max(1.0) as usize;
    for (i, idx) in results.iter().take(max_rows).enumerate() {
        let Some(app) = st.apps.get(*idx) else {
            continue;
        };
        let r = render::rect(area.x(), area.y() + i as f32 * row_h, area.width(), row_h - 2.0);
        widgets::list_row(
            cx,
            r,
            super::glyph_for(app),
            &app.name,
            &app.comment,
            false,
            Action::Launch(*idx),
        );
    }

    // Always offer the web as the last resort, Windows-style.
    let y = area.bottom() - 40.0;
    let r = render::rect(area.x(), y, area.width(), 38.0);
    let hot = cx.hot(r);
    if hot {
        render::round(cx.pm, r, theme::RADIUS_SM, theme::line_soft());
    }
    let q = cx.ellip(&st.query, theme::T_BODY, REGULAR, area.width() - 200.0);
    let label = format!("Search the web for “{q}”");
    cx.t(r.x() + 34.0, r.y(), r.height(), &label, theme::T_BODY, MEDIUM, theme::accent_deep());
    icons::draw(
        cx.pm,
        icons::Glyph::Globe,
        render::rect(r.x() + 9.0, r.y() + 11.0, 16.0, 16.0),
        theme::accent_deep(),
        1.9,
    );
    cx.hit(r, Action::SearchWeb);
}

/// Human-readable application count used by the footer of other panels.
pub fn describe(app: &DesktopApp) -> String {
    if let Some(url) = &app.web {
        format!("Web application · {url}")
    } else {
        format!("{} · {}", app.name, app.command_line())
    }
}
