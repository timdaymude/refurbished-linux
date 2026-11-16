//! Software: the graphical front end to Debian's package manager.
//!
//! There is no PackageKit, no package daemon and no catalogue refresher. The
//! window asks `apt-cache` when it opens and when the query changes, and asks
//! `apt-get` to install or remove when the user presses the button. Closing the
//! window costs nothing because nothing is left running.

use tiny_skia::Rect;

use crate::text::{MEDIUM, REGULAR, SEMIBOLD};
use crate::{icons, render, theme};

use super::widgets::{self, Ctx};
use super::{Action, Focus, WindowKind};

pub fn draw(cx: &mut Ctx, w: f32, h: f32) {
    let st = cx.st;
    let drag = widgets::titlebar(cx, w, "Software", icons::Glyph::Download);
    cx.hit(drag, Action::Nothing);
    let close = render::rect(w - 38.0, 8.0, 26.0, 26.0);
    widgets::icon_button(cx, close, icons::Glyph::Close, Action::WindowClose(WindowKind::Software), false);
    let title_w = cx.measure("Software", theme::T_TITLE, SEMIBOLD);
    cx.t(
        44.0 + title_w + 12.0,
        0.0,
        42.0,
        &format!("{} applications installed", st.apps.len()),
        theme::T_SMALL,
        REGULAR,
        theme::ink_faint(),
    );

    // ---- search ----------------------------------------------------------
    let field = render::rect(20.0, 54.0, w - 40.0, 36.0);
    widgets::search_field(
        cx,
        field,
        &st.software.query,
        "Search for software — try “browser”, “office”, “photo”",
        st.focus == Focus::Software,
    );
    cx.hit(field, Action::Nothing);

    let log_h = 92.0;
    let body = render::rect(20.0, field.bottom() + 14.0, w - 40.0, h - field.bottom() - log_h - 34.0);
    let detail_w = 264.0f32.min(body.width() * 0.38);
    let list = render::rect(body.x(), body.y(), body.width() - detail_w - 12.0, body.height());
    let detail = render::rect(list.right() + 12.0, body.y(), detail_w, body.height());

    result_list(cx, list);
    detail_pane(cx, detail);

    // ---- activity log ----------------------------------------------------
    let log = render::rect(20.0, h - log_h - 18.0, w - 40.0, log_h);
    render::round(cx.pm, log, theme::RADIUS_SM, theme::well());
    render::round_stroke(cx.pm, log, theme::RADIUS_SM, theme::line(), 1.0);
    let title = if st.software.busy {
        "Working… please wait"
    } else {
        "Activity"
    };
    cx.t(
        log.x() + 10.0,
        log.y() + 6.0,
        18.0,
        title,
        theme::T_SMALL - 0.5,
        SEMIBOLD,
        if st.software.busy { theme::accent_deep() } else { theme::ink_faint() },
    );
    let visible = ((log.height() - 28.0) / 15.0).floor().max(1.0) as usize;
    let start = st.software.log.len().saturating_sub(visible);
    for (i, line) in st.software.log[start..].iter().enumerate() {
        let text = cx.ellip(line, theme::T_SMALL - 1.0, REGULAR, log.width() - 20.0);
        cx.t(
            log.x() + 10.0,
            log.y() + 26.0 + i as f32 * 15.0,
            15.0,
            &text,
            theme::T_SMALL - 1.0,
            REGULAR,
            theme::ink_soft(),
        );
    }
    if st.software.log.is_empty() {
        cx.t(
            log.x() + 10.0,
            log.y() + 26.0,
            16.0,
            "Nothing to do yet. Installs and removals are logged here.",
            theme::T_SMALL - 0.5,
            REGULAR,
            theme::ink_faint(),
        );
    }
}

fn result_list(cx: &mut Ctx, area: Rect) {
    let st = cx.st;
    render::round(cx.pm, area, theme::RADIUS_SM, theme::well());
    render::round_stroke(cx.pm, area, theme::RADIUS_SM, theme::line(), 1.0);

    if st.software.results.is_empty() {
        let (title, body) = if st.software.searched {
            ("No software found", "Try a different word, for example “editor”.")
        } else {
            (
                "Search Debian's catalogue",
                "Applications are installed only when you ask for them.",
            )
        };
        widgets::empty_state(cx, area, title, body);
        // Suggested searches double as the "browse" affordance.
        let chips = ["Internet", "Office", "Graphics", "Education", "Utilities"];
        let mut x = area.x() + 20.0;
        for chip in chips {
            let cw = cx.measure(chip, theme::T_SMALL, MEDIUM) + 22.0;
            if x + cw > area.right() - 20.0 {
                break;
            }
            let r = render::rect(x, area.bottom() - 54.0, cw, 28.0);
            let hot = cx.hot(r);
            render::round(
                cx.pm,
                r,
                theme::RADIUS_SM,
                if hot { theme::accent_soft() } else { theme::card() },
            );
            render::round_stroke(cx.pm, r, theme::RADIUS_SM, theme::line(), 1.0);
            cx.tc(x, cw, r.y(), r.height(), chip, theme::T_SMALL, MEDIUM, theme::accent_deep());
            cx.hit(r, Action::SoftwareSuggest(chip.to_string()));
            x += cw + 8.0;
        }
        return;
    }

    let row_h = 48.0;
    let visible = ((area.height() - 8.0) / row_h).floor().max(1.0) as usize;
    let start = st.software.scroll.min(st.software.results.len().saturating_sub(1));
    for (i, pkg) in st
        .software
        .results
        .iter()
        .skip(start)
        .take(visible)
        .enumerate()
    {
        let idx = start + i;
        let r = render::rect(area.x() + 6.0, area.y() + 6.0 + i as f32 * row_h, area.width() - 12.0, row_h - 2.0);
        let selected = st.software.selected == Some(idx);
        widgets::list_row(
            cx,
            r,
            if pkg.installed { icons::Glyph::Check } else { icons::Glyph::Download },
            &pkg.name,
            &pkg.summary,
            selected,
            Action::SoftwarePick(idx),
        );
        if pkg.installed {
            widgets::badge_right(
                cx,
                r.right() - 10.0,
                r.y() + (r.height() - 17.0) / 2.0,
                "Installed",
                theme::accent_deep(),
                theme::accent_soft(),
            );
        }
    }
    if st.software.results.len() > visible {
        let more = format!(
            "{} more — scroll for more results",
            st.software.results.len() - visible
        );
        cx.tr(
            area.right() - 8.0,
            area.bottom() - 16.0,
            14.0,
            &more,
            theme::T_SMALL - 1.0,
            REGULAR,
            theme::ink_faint(),
        );
    }
}

fn detail_pane(cx: &mut Ctx, area: Rect) {
    let st = cx.st;
    render::round(cx.pm, area, theme::RADIUS_SM, theme::card());
    render::round_stroke(cx.pm, area, theme::RADIUS_SM, theme::line(), 1.0);

    let Some(idx) = st.software.selected else {
        cx.tc(
            area.x(),
            area.width(),
            area.y() + 30.0,
            20.0,
            "Select an application",
            theme::T_BODY,
            MEDIUM,
            theme::ink_faint(),
        );
        cx.tc(
            area.x() + 16.0,
            area.width() - 32.0,
            area.y() + 54.0,
            40.0,
            "Choose something from the list to read about it, then install or remove it.",
            theme::T_SMALL,
            REGULAR,
            theme::ink_faint(),
        );
        return;
    };
    let Some(pkg) = st.software.results.get(idx) else {
        return;
    };

    let chip = render::rect(area.x() + 16.0, area.y() + 18.0, 44.0, 44.0);
    render::round(cx.pm, chip, 12.0, if pkg.installed { theme::accent_soft() } else { theme::well() });
    icons::draw(
        cx.pm,
        if pkg.installed { icons::Glyph::Check } else { icons::Glyph::Download },
        render::rect(chip.x() + 12.0, chip.y() + 12.0, 20.0, 20.0),
        if pkg.installed { theme::accent_deep() } else { theme::ink_soft() },
        2.0,
    );
    let name = cx.ellip(&pkg.name, theme::T_TITLE, SEMIBOLD, area.width() - 32.0);
    cx.t(area.x() + 16.0, chip.bottom() + 10.0, 24.0, &name, theme::T_TITLE, SEMIBOLD, theme::ink());

    let state = if pkg.installed { "Installed" } else { "Available" };
    widgets::badge(
        cx,
        area.x() + 16.0,
        chip.bottom() + 40.0,
        state,
        if pkg.installed { theme::accent_deep() } else { theme::ink_soft() },
        if pkg.installed { theme::accent_soft() } else { theme::well() },
    );

    let mut y = chip.bottom() + 70.0;
    let words = &pkg.summary;
    let lines = wrap(cx, words, area.width() - 32.0, theme::T_BODY);
    for line in lines.iter().take(6) {
        cx.t(area.x() + 16.0, y, 18.0, line, theme::T_BODY, REGULAR, theme::ink_soft());
        y += 19.0;
    }

    let btn = render::rect(area.x() + 16.0, area.bottom() - 52.0, area.width() - 32.0, 34.0);
    if st.software.busy {
        widgets::button(cx, btn, "Working…", widgets::Style::Ghost, Action::Nothing);
    } else if pkg.installed {
        widgets::button(cx, btn, "Remove", widgets::Style::Danger, Action::SoftwareAct);
    } else {
        widgets::button(cx, btn, "Install", widgets::Style::Primary, Action::SoftwareAct);
    }
    cx.tr(
        area.right() - 16.0,
        area.bottom() - 18.0,
        14.0,
        &format!("apt: {}", pkg.name),
        theme::T_SMALL - 1.0,
        REGULAR,
        theme::ink_faint(),
    );
}

fn wrap(cx: &mut Ctx, text: &str, max_w: f32, px: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        if cx.measure(&candidate, px, REGULAR) > max_w && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current = word.to_string();
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}
