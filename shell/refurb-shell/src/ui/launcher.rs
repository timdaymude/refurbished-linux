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
        if st.picking {
            "Search every installed application"
        } else {
            "Search applications, or type a web address"
        },
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

    if st.query.is_empty() && !st.picking {
        pinned_grid(cx, body);
    } else {
        // The picker lists every installed application (the empty query is a
        // match for all of them), so an application installed a minute ago is
        // there without restarting anything.
        results_list(cx, body, st.picking);
    }

    // ---- footer ----------------------------------------------------------
    render::hline(
        cx.pm,
        card.x() + 18.0,
        card.bottom() - 34.0,
        card.width() - 36.0,
        theme::line_soft(),
    );
    let hints = if st.picking {
        "Enter  add     type to filter     Esc  close"
    } else {
        "Enter  open     Esc  close     Super  launcher"
    };
    cx.t(
        card.x() + 18.0,
        card.bottom() - 32.0,
        28.0,
        hints,
        theme::T_SMALL - 0.5,
        REGULAR,
        theme::ink_faint(),
    );
    let count = if st.picking {
        "Add to desktop".to_string()
    } else {
        format!("{} installed", st.apps.len())
    };
    cx.tr(
        card.right() - 18.0,
        card.bottom() - 32.0,
        28.0,
        &count,
        theme::T_SMALL - 0.5,
        REGULAR,
        theme::ink_faint(),
    );

    if cx.st.launcher_menu.is_some() {
        draw_menu(cx, w, h);
    }
}

fn pinned_grid(cx: &mut Ctx, area: Rect) {
    let st = cx.st;
    let pins = st.pinned.clone();
    if pins.is_empty() {
        widgets::empty_state(
            cx,
            area,
            "No pinned applications",
            "Use the launcher to start an application.",
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

fn results_list(cx: &mut Ctx, area: Rect, picking: bool) {
    let st = cx.st;
    let results = apps::search(&st.apps, &st.query);
    if results.is_empty() {
        widgets::empty_state(
            cx,
            area,
            &format!("Nothing installed matches “{}”", st.query),
            "Try a different name, or search the web.",
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
        let action = if picking {
            Action::LauncherAdd(*idx)
        } else {
            Action::Launch(*idx)
        };
        widgets::list_row(
            cx,
            r,
            super::glyph_for(app),
            &app.name,
            &app.comment,
            false,
            action,
        );
    }

    if picking {
        return;
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

/// The launcher's right-click menu. Right-clicking an application offers the
/// one action that does not belong in the launcher's normal click path: put a
/// shortcut on the desktop. The shortcut is the application's own desktop
/// entry, so a Flatpak launches through `flatpak run`, not a guessed binary.
fn draw_menu(cx: &mut Ctx, w: f32, h: f32) {
    let Some(menu) = cx.st.launcher_menu else {
        return;
    };
    let Some(app) = cx.st.apps.get(menu.app) else {
        return;
    };
    // A press anywhere outside the menu closes it; the menu's own rows are
    // registered afterwards and win over this region.
    cx.hit(render::rect(0.0, 0.0, w, h), Action::LauncherMenuClose);

    let items = [
        ("Open".to_string(), icons::Glyph::ChevronRight, Action::Launch(menu.app)),
        (
            "Add to Desktop".to_string(),
            icons::Glyph::Plus,
            Action::LauncherAdd(menu.app),
        ),
    ];
    let row_h = 32.0;
    let pad = 6.0;
    let header = 22.0;
    let mut width = 200.0f32;
    for (label, _, _) in &items {
        width = width.max(cx.measure(label, theme::T_SMALL, MEDIUM) + 62.0);
    }
    width = width.max(cx.measure(&app.name, theme::T_SMALL, MEDIUM) + 24.0);
    let width = width.min(w - 16.0);
    let height = pad * 2.0 + header + row_h * items.len() as f32;
    let x = menu.x.min(w - width - 8.0).max(8.0);
    let y = menu.y.min(h - height - 8.0).max(8.0);
    let card = render::rect(x, y, width, height);
    widgets::panel(cx, card);
    cx.hit(card, Action::Nothing);
    let title = cx.ellip(&app.name, theme::T_SMALL, MEDIUM, card.width() - 20.0);
    cx.t(
        card.x() + 10.0,
        card.y() + pad,
        header,
        &title,
        theme::T_SMALL,
        MEDIUM,
        theme::ink_faint(),
    );

    for (i, (label, glyph, action)) in items.iter().enumerate() {
        let row = render::rect(
            card.x() + pad,
            card.y() + pad + header + i as f32 * row_h,
            card.width() - pad * 2.0,
            row_h - 2.0,
        );
        let hot = cx.hot(row);
        if hot {
            render::round(cx.pm, row, theme::RADIUS_SM, theme::accent_soft());
        }
        icons::draw(
            cx.pm,
            *glyph,
            render::rect(row.x() + 9.0, row.y() + 8.0, 15.0, 15.0),
            if hot { theme::accent_deep() } else { theme::ink_soft() },
            1.9,
        );
        let full = format!("{label}");
        let shown = cx.ellip(&full, theme::T_SMALL, MEDIUM, row.width() - 42.0);
        cx.t(
            row.x() + 32.0,
            row.y(),
            row.height(),
            &shown,
            theme::T_SMALL,
            MEDIUM,
            theme::ink(),
        );
        cx.hit(row, action.clone());
    }
}

/// Human-readable application count used by the footer of other panels.
pub fn describe(app: &DesktopApp) -> String {
    if let Some(url) = &app.web {
        format!("Web application · {url}")
    } else {
        format!("{} · {}", app.name, app.command_line())
    }
}
