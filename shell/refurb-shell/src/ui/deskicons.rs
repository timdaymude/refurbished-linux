//! Desktop icons, drawn straight onto the wallpaper layer surface.
//!
//! The wallpaper is a background layer surface, so these icons sit below every
//! window and never interfere with the taskbar, window snapping or the
//! compositor's own input handling. A click that lands on an icon is registered
//! as a hit region; a click anywhere else on the desktop reaches no hit region
//! and is handled (or ignored) by the session.

use crate::desktop as desk;
use crate::text::MEDIUM;
use crate::{icons, render, theme};

use super::widgets::{self, Ctx};
use super::Action;

pub fn draw(cx: &mut Ctx, w: f32, h: f32) {
    let icons = cx.st.desk.icons.clone();
    let selected = cx.st.desk.selected;
    let renaming = cx.st.desk.renaming.clone();
    for (i, icon) in icons.iter().enumerate() {
        let (x, y) = desk::cell_origin(icon.col, icon.row);
        let tile = render::rect(x, y, desk::CELL_W, desk::CELL_H - 6.0);
        if tile.right() > w || tile.bottom() > h - theme::BAR_H - 6.0 {
            continue;
        }
        let hot = cx.hot(tile);
        if selected == Some(i) || hot {
            render::round(
                cx.pm,
                render::rect(tile.x() + 1.0, tile.y() + 1.0, tile.width() - 2.0, tile.height() - 2.0),
                theme::RADIUS_SM,
                if selected == Some(i) {
                    theme::accent_soft()
                } else {
                    theme::line_soft()
                },
            );
        }
        let chip = render::rect(
            tile.x() + (tile.width() - 44.0) / 2.0,
            tile.y() + 5.0,
            44.0,
            44.0,
        );
        let is_shortcut = icon.kind == desk::Kind::App;
        render::round(
            cx.pm,
            chip,
            11.0,
            if is_shortcut {
                theme::accent_soft()
            } else {
                theme::well()
            },
        );
        let glyph = match icon.kind {
            desk::Kind::Folder => icons::Glyph::Folder,
            desk::Kind::App => icon
                .app
                .as_ref()
                .map(super::glyph_for)
                .unwrap_or(icons::Glyph::Apps),
            desk::Kind::File => icons::Glyph::Doc,
        };
        icons::draw(
            cx.pm,
            glyph,
            render::rect(chip.x() + 10.0, chip.y() + 10.0, 24.0, 24.0),
            if is_shortcut {
                theme::accent_deep()
            } else {
                theme::ink()
            },
            1.9,
        );

        if let Some((idx, text)) = &renaming {
            if *idx == i {
                let field = render::rect(tile.x() + 2.0, tile.y() + 52.0, tile.width() - 4.0, 25.0);
                render::round(cx.pm, field, theme::RADIUS_SM, theme::card());
                render::round_stroke(cx.pm, field, theme::RADIUS_SM, theme::accent(), 1.0);
                let shown = cx.ellip(text, theme::T_SMALL, MEDIUM, field.width() - 14.0);
                let tw = cx.t(
                    field.x() + 6.0,
                    field.y(),
                    field.height(),
                    &shown,
                    theme::T_SMALL,
                    MEDIUM,
                    theme::ink(),
                );
                render::fill(
                    cx.pm,
                    render::rect(field.x() + 7.0 + tw, field.y() + 6.0, 1.5, field.height() - 12.0),
                    theme::accent(),
                );
                cx.hit(field, Action::Nothing);
                continue;
            }
        }

        // Labels are one line where that fits. "Install Reawakening OS" does
        // not fit a 90-pixel cell at 12 px, and ellipsizing the name of the
        // live installer would hide what it is, so a long name is balanced
        // onto two lines instead (still the same small, centred label style).
        let lines = label_lines(cx, &icon.name, tile.width() - 6.0, theme::T_SMALL - 0.5);
        if lines.len() > 1 {
            let size = theme::T_SMALL - 0.5;
            for (n, line) in lines.iter().enumerate() {
                cx.tc(
                    tile.x(),
                    tile.width(),
                    chip.bottom() + 2.0 + n as f32 * 13.0,
                    13.0,
                    line,
                    size,
                    MEDIUM,
                    theme::ink(),
                );
            }
        } else {
            let label = lines.first().cloned().unwrap_or_default();
            cx.tc(
                tile.x(),
                tile.width(),
                chip.bottom() + 3.0,
                28.0,
                &label,
                theme::T_SMALL,
                MEDIUM,
                theme::ink(),
            );
        }
        cx.hit(tile, Action::DeskIcon(i));
    }

    if let Some(menu) = cx.st.desk.menu.clone() {
        draw_menu(cx, w, h, &menu);
    }

    if let Some((text, _)) = &cx.st.desk.notice {
        let tw = cx.measure(text, theme::T_SMALL, MEDIUM);
        let width = (tw + 26.0).min(w - 40.0);
        let pill = render::rect((w - width) / 2.0, h - theme::BAR_H - 46.0, width, 30.0);
        render::round(cx.pm, pill, 15.0, theme::card());
        render::round_stroke(cx.pm, pill, 15.0, theme::line(), 1.0);
        let shown = cx.ellip(text, theme::T_SMALL, MEDIUM, width - 18.0);
        cx.tc(
            pill.x(),
            pill.width(),
            pill.y(),
            pill.height(),
            &shown,
            theme::T_SMALL,
            MEDIUM,
            theme::ink(),
        );
        cx.hit(pill, Action::Nothing);
    }
}

/// Split a name into at most two lines that each fit `width`, balancing the
/// words rather than filling the first line and leaving one word on the second.
/// A single word that is too wide falls back to an ellipsized line.
fn label_lines(cx: &mut Ctx, name: &str, width: f32, size: f32) -> Vec<String> {
    if cx.measure(name, size, MEDIUM) <= width {
        return vec![name.to_string()];
    }
    let words: Vec<&str> = name.split_whitespace().collect();
    if words.len() < 2 {
        return vec![cx.ellip(name, size, MEDIUM, width)];
    }
    let mut best: Option<(f32, usize)> = None;
    for split in 1..words.len() {
        let first = words[..split].join(" ");
        let second = words[split..].join(" ");
        let widest = cx
            .measure(&first, size, MEDIUM)
            .max(cx.measure(&second, size, MEDIUM));
        if best.map_or(true, |(w, _)| widest < w) {
            best = Some((widest, split));
        }
    }
    match best {
        Some((widest, split)) if widest <= width => {
            vec![words[..split].join(" "), words[split..].join(" ")]
        }
        _ => vec![cx.ellip(name, size, MEDIUM, width)],
    }
}

fn draw_menu(cx: &mut Ctx, w: f32, h: f32, menu: &desk::Menu) {
    // Any press that misses a row closes the menu; rows are registered after
    // this region and win over it.
    cx.hit(render::rect(0.0, 0.0, w, h), Action::DeskMenuClose);

    let mut items: Vec<(String, icons::Glyph, Action)> = Vec::new();
    if menu.icon.is_some() {
        items.push(("Open".to_string(), icons::Glyph::ChevronRight, Action::DeskOpen));
        items.push(("Rename…".to_string(), icons::Glyph::Doc, Action::DeskRename));
        items.push(("Move to Trash".to_string(), icons::Glyph::Close, Action::DeskDelete));
    } else {
        items.push(("New Folder".to_string(), icons::Glyph::Folder, Action::DeskNewFolder));
        items.push((
            "Add Application Shortcut".to_string(),
            icons::Glyph::Plus,
            Action::DeskMenuShortcuts,
        ));
        items.push(("Refresh".to_string(), icons::Glyph::Refresh, Action::DeskRefresh));
    }

    let row_h = 32.0;
    let pad = 6.0;
    let mut width = 190.0f32;
    for (label, _, _) in &items {
        width = width.max(cx.measure(label, theme::T_SMALL, MEDIUM) + 62.0);
    }
    let width = width.min(w - 16.0);
    let height = pad * 2.0 + row_h * items.len() as f32;
    let x = menu.x.min(w - width - 8.0).max(8.0);
    let y = menu.y.min(h - height - theme::BAR_H - 8.0).max(8.0);
    let card = render::rect(x, y, width, height);
    widgets::panel(cx, card);
    cx.hit(card, Action::Nothing);

    for (i, (label, glyph, action)) in items.iter().enumerate() {
        let row = render::rect(
            card.x() + pad,
            card.y() + pad + i as f32 * row_h,
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
        let shown = cx.ellip(label, theme::T_SMALL, MEDIUM, row.width() - 42.0);
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
