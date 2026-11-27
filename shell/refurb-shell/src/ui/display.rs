//! Settings → Display.
//!
//! Every connected screen is a row; the selected one gets its resolution and
//! refresh rate; and the screens themselves are draggable rectangles, so
//! "television on the right" is arranged by putting it on the right. Nothing is
//! applied until Apply is pressed, and then a fifteen-second countdown asks
//! whether the result is actually usable — a resolution a monitor cannot show
//! reverts by itself.
//!
//! The panel assumes nothing about the machine. A desktop with one monitor, a
//! laptop with a built-in panel, a laptop and a television, three screens in a
//! row or one above another are all the same code path: a list of rectangles
//! with positions.

use std::time::Instant;

use tiny_skia::Rect;

use crate::displays;
use crate::text::{MEDIUM, REGULAR};
use crate::{icons, render, theme};

use super::widgets::{self, Ctx};
use super::{Action, DisplayUi};

pub fn draw(cx: &mut Ctx, pane: Rect) {
    let st = cx.st;
    let mut y = super::settings::heading(
        cx,
        pane,
        "Display",
        "Arrange your screens. Nothing changes until you press Apply.",
    );

    if st.display.outputs.is_empty() {
        super::settings::paragraph(
            cx,
            pane,
            &mut y,
            "No displays have been reported yet. This panel asks the compositor \
             for its output list when it opens; on a machine with no Wayland \
             session it stays empty.",
        );
        widgets::button(
            cx,
            render::rect(pane.x() + 20.0, y + 10.0, 132.0, 32.0),
            "Scan again",
            widgets::Style::Secondary,
            Action::DisplayRefresh,
        );
        if let Some(note) = st.display.note.last() {
            cx.t(
                pane.x() + 20.0,
                y + 52.0,
                18.0,
                note,
                theme::T_SMALL,
                REGULAR,
                theme::clay(),
            );
        }
        return;
    }

    // ---- the screens, as selectable chips ---------------------------------
    let avail = pane.width() - 40.0;
    let cols = (((avail + 8.0) / 186.0).floor() as usize).clamp(1, 4);
    let chip_w = (avail - 8.0 * (cols as f32 - 1.0)) / cols as f32;
    let chip_h = 52.0;
    let shown = st.display.outputs.len().min(cols * 2);
    for i in 0..shown {
        let out = &st.display.outputs[i];
        let col = i % cols;
        let row = i / cols;
        let r = render::rect(
            pane.x() + 20.0 + col as f32 * (chip_w + 8.0),
            y + row as f32 * (chip_h + 6.0),
            chip_w,
            chip_h,
        );
        chip(cx, r, i, out, st.display.selected == i);
    }
    y += (shown.saturating_sub(1) / cols + 1) as f32 * (chip_h + 6.0) + 4.0;

    let selected = st.display.selected.min(st.display.outputs.len() - 1);
    let out = st.display.outputs[selected].clone();

    // ---- arrangement diagram, and the selected screen's controls ----------
    let canvas_w = ((pane.width() - 40.0) * 0.53).max(240.0);
    let canvas_h = (pane.bottom() - y - 112.0).clamp(120.0, 190.0);
    let canvas = render::rect(pane.x() + 20.0, y, canvas_w, canvas_h);
    diagram(cx, canvas, selected);

    let side = render::rect(
        canvas.right() + 12.0,
        y,
        pane.right() - 20.0 - canvas.right() - 12.0,
        canvas_h,
    );
    widgets::section_label(cx, side.x(), side.y(), "This screen");
    let name = cx.ellip(&out.title(), theme::T_BODY, MEDIUM, side.width());
    cx.t(side.x(), side.y() + 16.0, 20.0, &name, theme::T_BODY, MEDIUM, theme::ink());
    cx.t(
        side.x(),
        side.y() + 34.0,
        16.0,
        &format!("{} · {}", out.name, out.resolution_label()),
        theme::T_SMALL,
        REGULAR,
        theme::ink_faint(),
    );

    let mut sy = side.y() + 54.0;
    let half = (side.width() - 8.0) / 2.0;
    let is_main = st.display.primary == selected;
    widgets::button(
        cx,
        render::rect(side.x(), sy, half, 28.0),
        if is_main { "Main display" } else { "Make main" },
        if is_main { widgets::Style::Primary } else { widgets::Style::Secondary },
        if is_main { Action::Nothing } else { Action::DisplayMain(selected) },
    );
    let mirrored = st.display.mirrored == Some(selected);
    widgets::button(
        cx,
        render::rect(side.x() + half + 8.0, sy, half, 28.0),
        if mirrored { "Mirrored" } else { "Mirror" },
        if mirrored { widgets::Style::Primary } else { widgets::Style::Secondary },
        if selected == st.display.primary {
            Action::Nothing
        } else {
            Action::DisplayMirror
        },
    );
    sy += 34.0;

    widgets::section_label(cx, side.x(), sy, "Resolution and refresh rate");
    sy += 16.0;
    let mode_w = (side.width() - 6.0) / 2.0;
    let modes: Vec<usize> = out
        .modes
        .iter()
        .enumerate()
        .filter(|(_, m)| m.height >= 480)
        .map(|(i, _)| i)
        .take(6)
        .collect();
    for (slot, index) in modes.iter().enumerate() {
        let mode = &out.modes[*index];
        let col = slot % 2;
        let row = slot / 2;
        let r = render::rect(
            side.x() + col as f32 * (mode_w + 6.0),
            sy + row as f32 * 30.0,
            mode_w,
            26.0,
        );
        if r.bottom() > side.bottom() {
            break;
        }
        let active = mode.current;
        widgets::button(
            cx,
            r,
            &mode.label(),
            if active { widgets::Style::Primary } else { widgets::Style::Secondary },
            Action::DisplayMode(*index),
        );
    }
    if modes.is_empty() {
        cx.t(side.x(), sy, 18.0, "No modes reported", theme::T_SMALL, REGULAR, theme::ink_faint());
    }

    // ---- actions ----------------------------------------------------------
    y = canvas.bottom() + 10.0;
    widgets::button(
        cx,
        render::rect(pane.x() + 20.0, y, 112.0, 32.0),
        "Identify",
        widgets::Style::Secondary,
        Action::DisplayIdentify,
    );
    widgets::button(
        cx,
        render::rect(pane.x() + 140.0, y, 96.0, 32.0),
        "Apply",
        widgets::Style::Primary,
        Action::DisplayApply,
    );
    widgets::button(
        cx,
        render::rect(pane.x() + 244.0, y, 92.0, 32.0),
        "Scan again",
        widgets::Style::Ghost,
        Action::DisplayRefresh,
    );
    if st.display.pending.is_some() {
        widgets::button(
            cx,
            render::rect(pane.x() + 344.0, y, 92.0, 32.0),
            "Revert",
            widgets::Style::Danger,
            Action::DisplayRevert,
        );
    }
    y += 38.0;

    // ---- what just happened, or what is about to --------------------------
    if let Some(pending) = &st.display.pending {
        let remaining = pending
            .deadline
            .saturating_duration_since(Instant::now())
            .as_secs()
            + 1;
        let banner = render::rect(
            pane.x() + 20.0,
            pane.bottom() - 62.0,
            pane.width() - 40.0,
            42.0,
        );
        render::round(cx.pm, banner, theme::RADIUS_SM, theme::accent_soft());
        cx.t(
            banner.x() + 12.0,
            banner.y() + 2.0,
            18.0,
            &format!("Keep this arrangement? Reverting in {remaining}s."),
            theme::T_BODY,
            MEDIUM,
            theme::accent_deep(),
        );
        cx.t(
            banner.x() + 12.0,
            banner.y() + 20.0,
            16.0,
            &pending.reason,
            theme::T_SMALL,
            REGULAR,
            theme::ink_soft(),
        );
        widgets::button(
            cx,
            render::rect(banner.right() - 82.0, banner.y() + 9.0, 70.0, 24.0),
            "Keep",
            widgets::Style::Primary,
            Action::DisplayKeep,
        );
        return;
    }

    let mut note_y = y;
    for line in st.display.note.iter().rev().take(2) {
        cx.t(
            pane.x() + 20.0,
            note_y,
            16.0,
            line,
            theme::T_SMALL,
            REGULAR,
            theme::ink_soft(),
        );
        note_y += 16.0;
    }
    if st.display.note.is_empty() {
        let hint = arrangement_sentence(&st.display);
        cx.t(
            pane.x() + 20.0,
            note_y,
            16.0,
            &hint,
            theme::T_SMALL - 0.5,
            REGULAR,
            theme::ink_faint(),
        );
    }
}

fn chip(cx: &mut Ctx, r: Rect, index: usize, out: &displays::Output, selected: bool) {
    let hot = cx.hot(r) || selected;
    render::round(
        cx.pm,
        r,
        theme::RADIUS_SM,
        if selected {
            theme::accent_soft()
        } else if hot {
            theme::line_soft()
        } else {
            theme::well()
        },
    );
    render::round_stroke(
        cx.pm,
        r,
        theme::RADIUS_SM,
        if selected { theme::accent() } else { theme::line() },
        1.0,
    );
    let title = cx.ellip(&out.title(), theme::T_BODY, MEDIUM, r.width() - 20.0);
    cx.t(
        r.x() + 10.0,
        r.y() + 6.0,
        20.0,
        &title,
        theme::T_BODY,
        MEDIUM,
        theme::ink(),
    );
    // "Main" and "Mirror" belong to the same line as the resolution: a badge
    // in the corner sat on top of the size and left a stray "768" behind.
    let mut detail = out.resolution_label();
    let main = cx.st.display.primary == index;
    let mirrored = cx.st.display.mirrored == Some(index);
    if main {
        detail.push_str(" · Main");
    } else if mirrored {
        detail.push_str(" · Mirror");
    }
    let detail = cx.ellip(&detail, theme::T_SMALL, REGULAR, r.width() - 20.0);
    cx.t(
        r.x() + 10.0,
        r.y() + 26.0,
        16.0,
        &detail,
        theme::T_SMALL,
        if main { MEDIUM } else { REGULAR },
        if main { theme::accent_deep() } else { theme::ink_faint() },
    );
    // One switch per screen, so a display can be darkened without leaving this
    // page. It is staged like everything else; Apply makes it real.
    widgets::button(
        cx,
        render::rect(r.right() - 52.0, r.y() + 8.0, 42.0, 20.0),
        if out.enabled { "On" } else { "Off" },
        if out.enabled { widgets::Style::Secondary } else { widgets::Style::Ghost },
        Action::DisplayToggle(index),
    );
    cx.hit(r, Action::DisplaySelect(index));
}

/// The draggable diagram. Rectangles keep their relative sizes, scaled to fit
/// the box; positions are the real positions in the compositor's coordinate
/// space, so "left of", "right of" and "above" are exactly what the pointer
/// will do.
fn diagram(cx: &mut Ctx, area: Rect, selected: usize) {
    let outputs = &cx.st.display.outputs;
    render::round(cx.pm, area, theme::RADIUS_SM, theme::well());
    render::round_stroke(cx.pm, area, theme::RADIUS_SM, theme::line(), 1.0);
    widgets::section_label(cx, area.x() + 8.0, area.y() + 6.0, "Drag to arrange");
    cx.hit(area, Action::Nothing);

    let rects: Vec<(i32, i32, u32, u32)> = outputs
        .iter()
        .filter(|o| o.enabled)
        .map(|o| o.rect())
        .collect();
    if rects.is_empty() {
        cx.tc(
            area.x(),
            area.width(),
            area.y() + area.height() / 2.0 - 10.0,
            20.0,
            "Every screen is switched off",
            theme::T_SMALL,
            REGULAR,
            theme::ink_faint(),
        );
        return;
    }
    let gap = 10.0;
    let top = area.y() + 22.0;
    let (scale, origin) = displays::diagram(&rects, area.width(), area.height() - 24.0, gap);
    for (index, out) in outputs.iter().enumerate() {
        if !out.enabled {
            continue;
        }
        let (x, y, w, h) = out.rect();
        let r = render::rect(
            area.x() + gap + (x - origin.0) as f32 * scale,
            top + (y - origin.1) as f32 * scale,
            (w as f32 * scale).max(24.0),
            (h as f32 * scale).max(16.0),
        );
        let is_selected = index == selected;
        render::round(
            cx.pm,
            r,
            4.0,
            if is_selected {
                theme::accent_soft()
            } else {
                theme::card()
            },
        );
        render::round_stroke(
            cx.pm,
            r,
            4.0,
            if is_selected { theme::accent() } else { theme::line() },
            if is_selected { 2.0 } else { 1.0 },
        );
        // The connector name is what a person finds on the cable, so it is what
        // the diagram shows; the number matches the Identify overlay.
        let label = format!("{} {}", index + 1, out.name);
        let shown = cx.ellip(&label, theme::T_SMALL - 1.0, MEDIUM, r.width() - 10.0);
        if r.width() > 46.0 && r.height() > 22.0 {
            cx.tc(
                r.x(),
                r.width(),
                r.y() + r.height() / 2.0 - 8.0,
                16.0,
                &shown,
                theme::T_SMALL - 1.0,
                MEDIUM,
                if is_selected { theme::accent_deep() } else { theme::ink() },
            );
            let size = &out.size();
            let s = format!("{}×{}", size.0, size.1);
            cx.tc(
                r.x(),
                r.width(),
                r.y() + r.height() / 2.0 + 8.0,
                14.0,
                &s,
                theme::T_SMALL - 2.0,
                REGULAR,
                theme::ink_faint(),
            );
        }
        if cx.st.display.mirrored == Some(index) {
            icons::draw(
                cx.pm,
                icons::Glyph::Display,
                render::rect(r.x() + 4.0, r.y() + 4.0, 12.0, 12.0),
                theme::accent_deep(),
                1.6,
            );
        }
        cx.hit(r, Action::DisplayGrab(index));
    }
}

/// One sentence that says how the screens sit relative to each other, so the
/// arrangement can be understood without reading coordinates.
pub fn arrangement_sentence(st: &DisplayUi) -> String {
    let Some(main) = st.outputs.iter().find(|o| o.name == st.main_name) else {
        return "Choose a display and drag its rectangle to arrange the screens.".to_string();
    };
    let (mx, my, mw, mh) = main.rect();
    let (mright, mbottom) = (mx + mw as i32, my + mh as i32);
    let mut parts: Vec<String> = Vec::new();
    for out in st.outputs.iter().filter(|o| o.enabled && o.name != main.name) {
        let (x, y, w, h) = out.rect();
        let (right, bottom) = (x + w as i32, y + h as i32);
        let beside = x >= mright || right <= mx;
        let stacked = y >= mbottom || bottom <= my;
        // "Beside" wins only when the two rectangles are actually clear of each
        // other on that axis. A screen above and to the left of the main one is
        // both, and the nearer edge is the useful description.
        let side = if beside && stacked {
            let horizontal = (x - mright).abs().min((right - mx).abs());
            let vertical = (y - mbottom).abs().min((bottom - my).abs());
            if vertical <= horizontal {
                if y >= mbottom { "below" } else { "above" }
            } else if x >= mright {
                "to the right of"
            } else {
                "to the left of"
            }
        } else if beside {
            if x >= mright { "to the right of" } else { "to the left of" }
        } else if stacked {
            if y >= mbottom { "below" } else { "above" }
        } else {
            "over"
        };
        parts.push(format!("{} is {side} {}", out.name, main.name));
    }
    if parts.is_empty() {
        format!("{} is the only active screen.", main.name)
    } else {
        format!("{}.", parts.join(", and "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::displays::{Mode, Output};

    fn output(name: &str, x: i32, y: i32, w: u32, h: u32) -> Output {
        Output {
            name: name.into(),
            enabled: true,
            position: (x, y),
            scale: 1.0,
            modes: vec![Mode {
                width: w,
                height: h,
                refresh: 60.0,
                preferred: true,
                current: true,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn the_sentence_describes_left_right_and_above() {
        let mut st = DisplayUi::default();
        st.main_name = "eDP-1".into();
        st.outputs = vec![
            output("eDP-1", 0, 0, 1920, 1080),
            output("HDMI-A-1", 1920, 0, 1920, 1080),
            output("DP-1", 0, -1080, 1280, 1024),
            output("DP-2", 2000, -1024, 1280, 1024),
        ];
        let text = arrangement_sentence(&st);
        assert!(text.contains("HDMI-A-1 is to the right of eDP-1"), "{text}");
        assert!(text.contains("DP-1 is above eDP-1"), "{text}");
        assert!(text.contains("DP-2 is above eDP-1"), "{text}");
    }

    #[test]
    fn a_single_screen_needs_no_relationship() {
        let mut st = DisplayUi::default();
        st.main_name = "eDP-1".into();
        st.outputs = vec![output("eDP-1", 0, 0, 1920, 1080)];
        assert_eq!(arrangement_sentence(&st), "eDP-1 is the only active screen.");
    }
}
