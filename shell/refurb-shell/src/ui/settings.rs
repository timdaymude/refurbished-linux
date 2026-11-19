//! Settings: a single window with a section rail, in the shape Windows users
//! expect. Everything here is either a `/sys` read or an explicit user action.

use tiny_skia::Rect;

use crate::system::PowerAction;
use crate::text::{MEDIUM, REGULAR, SEMIBOLD};
use crate::wallpaper::{self, Choice as Wallpaper};
use crate::{icons, render, theme};

use super::widgets::{self, Ctx};
use super::{Action, Section, Tool, WindowKind};

pub fn draw(cx: &mut Ctx, w: f32, h: f32) {
    let drag = widgets::titlebar(cx, w, "Settings", icons::Glyph::Gear);
    cx.hit(drag, Action::Nothing);
    let close = render::rect(w - 38.0, 8.0, 26.0, 26.0);
    widgets::icon_button(cx, close, icons::Glyph::Close, Action::WindowClose(WindowKind::Settings), false);

    let rail = render::rect(20.0, 58.0, 168.0, h - 78.0);
    let pane = render::rect(rail.right() + 14.0, 58.0, w - rail.right() - 34.0, h - 78.0);
    render::round(cx.pm, rail, theme::RADIUS_SM, theme::well());
    render::round_stroke(cx.pm, rail, theme::RADIUS_SM, theme::line(), 1.0);
    render::round(cx.pm, pane, theme::RADIUS_SM, theme::card());
    render::round_stroke(cx.pm, pane, theme::RADIUS_SM, theme::line(), 1.0);

    let mut y = rail.y() + 8.0;
    for section in Section::ALL {
        let r = render::rect(rail.x() + 6.0, y, rail.width() - 12.0, 34.0);
        let active = cx.st.settings.section == section;
        let hot = cx.hot(r) || active;
        if hot {
            render::round(
                cx.pm,
                r,
                theme::RADIUS_SM,
                if active { theme::card() } else { theme::line_soft() },
            );
        }
        icons::draw(
            cx.pm,
            section.glyph(),
            render::rect(r.x() + 9.0, r.y() + 9.0, 16.0, 16.0),
            if active { theme::accent_deep() } else { theme::ink_soft() },
            1.9,
        );
        cx.t(
            r.x() + 32.0,
            r.y(),
            r.height(),
            section.label(),
            theme::T_BODY,
            if active { MEDIUM } else { REGULAR },
            if active { theme::ink() } else { theme::ink_soft() },
        );
        cx.hit(r, Action::Section(section));
        y += 36.0;
    }
    cx.t(
        rail.x() + 12.0,
        rail.bottom() - 30.0,
        20.0,
        "Reawakening OS",
        theme::T_SMALL - 1.0,
        REGULAR,
        theme::ink_faint(),
    );
    cx.t(
        rail.x() + 12.0,
        rail.bottom() - 16.0,
        14.0,
        &format!("shell {}", cx.st.version),
        theme::T_SMALL - 2.0,
        REGULAR,
        theme::ink_faint(),
    );

    match cx.st.settings.section {
        Section::Updates => updates(cx, pane),
        Section::Display => display(cx, pane),
        Section::Appearance => appearance(cx, pane),
        Section::Sound => sound(cx, pane),
        Section::Power => power(cx, pane),
        Section::About => about(cx, pane),
    }
}

fn heading(cx: &mut Ctx, pane: Rect, title: &str, subtitle: &str) -> f32 {
    cx.t(pane.x() + 20.0, pane.y() + 18.0, 26.0, title, theme::T_HEAD, SEMIBOLD, theme::ink());
    cx.t(
        pane.x() + 20.0,
        pane.y() + 44.0,
        theme::T_SMALL * 1.6,
        subtitle,
        theme::T_SMALL,
        REGULAR,
        theme::ink_soft(),
    );
    pane.y() + 74.0
}

fn paragraph(cx: &mut Ctx, pane: Rect, y: &mut f32, text: &str) {
    let max = pane.width() - 40.0;
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if cx.measure(&candidate, theme::T_BODY, REGULAR) > max && !line.is_empty() {
            cx.t(pane.x() + 20.0, *y, 18.0, &line, theme::T_BODY, REGULAR, theme::ink_soft());
            *y += 19.0;
            line = word.to_string();
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        cx.t(pane.x() + 20.0, *y, 18.0, &line, theme::T_BODY, REGULAR, theme::ink_soft());
        *y += 19.0;
    }
}

fn updates(cx: &mut Ctx, pane: Rect) {
    let st = cx.st;
    let mut y = heading(
        cx,
        pane,
        "Updates",
        "Updates are checked when you ask, never in the background.",
    );
    paragraph(
        cx,
        pane,
        &mut y,
        "Reawakening OS does not run an update checker, an update notifier or a \
         package catalogue service. Debian's own daily metadata refresh keeps the \
         package lists current for a few seconds a day and then exits.",
    );
    y += 8.0;

    let (upgrades, security) = (st.status.updates, st.status.updates);
    if upgrades > 0 {
        let text = format!(
            "{upgrades} update{} available ({security} security)",
            if upgrades == 1 { "" } else { "s" }
        );
        cx.t(pane.x() + 20.0, y, 20.0, &text, theme::T_BODY, MEDIUM, theme::clay());
    } else {
        cx.t(
            pane.x() + 20.0,
            y,
            20.0,
            "No known updates. Run a check to be certain.",
            theme::T_BODY,
            REGULAR,
            theme::ink_soft(),
        );
    }
    y += 30.0;

    widgets::button(
        cx,
        render::rect(pane.x() + 20.0, y, 172.0, 34.0),
        "Check for updates",
        widgets::Style::Secondary,
        Action::UpdateCheck,
    );
    widgets::button(
        cx,
        render::rect(pane.x() + 200.0, y, 172.0, 34.0),
        "Install updates",
        widgets::Style::Primary,
        Action::UpdateInstall,
    );
    y += 48.0;

    if !st.updates_note.is_empty() {
        let log = render::rect(pane.x() + 20.0, y, pane.width() - 40.0, pane.bottom() - y - 20.0);
        render::round(cx.pm, log, theme::RADIUS_SM, theme::well());
        let visible = ((log.height() - 16.0) / 15.0).floor().max(1.0) as usize;
        let start = st.updates_note.len().saturating_sub(visible);
        for (i, line) in st.updates_note[start..].iter().enumerate() {
            let text = cx.ellip(line, theme::T_SMALL - 1.0, REGULAR, log.width() - 20.0);
            cx.t(
                log.x() + 10.0,
                log.y() + 8.0 + i as f32 * 15.0,
                15.0,
                &text,
                theme::T_SMALL - 1.0,
                REGULAR,
                theme::ink_soft(),
            );
        }
    }
}

fn display(cx: &mut Ctx, pane: Rect) {
    let st = cx.st;
    let mut y = heading(cx, pane, "Display", "Resolution and rotation, applied immediately.");
    if st.display.outputs.is_empty() {
        paragraph(
            cx,
            pane,
            &mut y,
            "No display information available yet. This panel reads `wlr-randr` \
             when it opens; on a machine without a Wayland session it stays empty.",
        );
        widgets::button(
            cx,
            render::rect(pane.x() + 20.0, y + 10.0, 132.0, 32.0),
            "Refresh",
            widgets::Style::Secondary,
            Action::Section(Section::Display),
        );
        return;
    }
    let out = &st.display.outputs[st.display.selected.min(st.display.outputs.len() - 1)];
    cx.t(
        pane.x() + 20.0,
        y,
        22.0,
        &format!("Output: {}", out.name),
        theme::T_BODY,
        SEMIBOLD,
        theme::ink(),
    );
    y += 28.0;
    widgets::section_label(cx, pane.x() + 20.0, y, "Available modes");
    y += 20.0;
    let col_w = (pane.width() - 40.0) / 3.0;
    for (i, mode) in out.modes.iter().take(9).enumerate() {
        let col = i % 3;
        let row = i / 3;
        let r = render::rect(
            pane.x() + 20.0 + col as f32 * col_w,
            y + row as f32 * 38.0,
            col_w - 8.0,
            32.0,
        );
        let current = out.current.as_deref() == Some(mode.as_str());
        widgets::button(
            cx,
            r,
            mode,
            if current { widgets::Style::Primary } else { widgets::Style::Secondary },
            Action::DisplayMode(i),
        );
    }
    cx.t(
        pane.x() + 20.0,
        pane.bottom() - 34.0,
        18.0,
        "Scaling stays at 100% to keep the compositor cheap on old GPUs.",
        theme::T_SMALL,
        REGULAR,
        theme::ink_faint(),
    );
}

/// Wallpapers: the two pictures that ship in the image, shown as thumbnails so
/// the choice is made by looking at them rather than by reading file names.
fn appearance(cx: &mut Ctx, pane: Rect) {
    let current = cx.st.wallpaper;
    let mut y = heading(
        cx,
        pane,
        "Appearance",
        "Two wallpapers, drawn by the session at every screen's own resolution.",
    );
    let gap = 14.0;
    let card_w = (pane.width() - 40.0 - gap) / 2.0;
    let thumb_h = (card_w * 0.56).clamp(90.0, (pane.bottom() - y - 96.0).max(90.0));
    for (i, choice) in Wallpaper::ALL.iter().enumerate() {
        let card = render::rect(
            pane.x() + 20.0 + i as f32 * (card_w + gap),
            y,
            card_w,
            thumb_h + 48.0,
        );
        let active = *choice == current;
        render::round(cx.pm, card, theme::RADIUS_SM, theme::well());
        let thumb = render::rect(card.x() + 6.0, card.y() + 6.0, card.width() - 12.0, thumb_h);
        if !wallpaper::draw_into(cx.pm, *choice, thumb) {
            render::round(cx.pm, thumb, 4.0, theme::line_soft());
            cx.t(
                thumb.x() + 10.0,
                thumb.y(),
                thumb.height(),
                "image not installed",
                theme::T_SMALL,
                REGULAR,
                theme::ink_faint(),
            );
        }
        render::round_stroke(
            cx.pm,
            card,
            theme::RADIUS_SM,
            if active { theme::accent() } else { theme::line() },
            if active { 1.6 } else { 1.0 },
        );
        cx.t(
            card.x() + 10.0,
            thumb.bottom() + 6.0,
            20.0,
            choice.label(),
            theme::T_BODY,
            MEDIUM,
            theme::ink(),
        );
        if active {
            let badge = render::rect(card.right() - 62.0, thumb.bottom() + 7.0, 52.0, 18.0);
            render::round(cx.pm, badge, 6.0, theme::accent());
            cx.tc(
                badge.x(),
                badge.width(),
                badge.y() + 2.0,
                14.0,
                "In use",
                theme::T_SMALL - 1.5,
                MEDIUM,
                theme::on_accent(),
            );
        }
        cx.hit(card, Action::Wallpaper(i));
    }
    y += thumb_h + 62.0;
    paragraph(
        cx,
        pane,
        &mut y,
        &format!(
            "{} The choice is remembered for this account and applied to every connected screen.",
            current.blurb()
        ),
    );
}

fn sound(cx: &mut Ctx, pane: Rect) {
    let st = cx.st;
    let mut y = heading(cx, pane, "Sound", "PipeWire with WirePlumber: output, volume and mute.");
    let row = render::rect(pane.x() + 20.0, y, pane.width() - 40.0, 34.0);
    widgets::slider(
        cx,
        render::rect(row.x(), row.y(), row.width() - 56.0, 34.0),
        icons::Glyph::Volume,
        st.status.volume / crate::system::VOLUME_MAX,
        super::SliderKind::Volume,
        Action::VolumeSet,
    );
    cx.tr(
        row.right(),
        row.y(),
        34.0,
        &format!("{}%", (st.status.volume * 100.0).round() as i32),
        theme::T_BODY,
        MEDIUM,
        theme::ink(),
    );
    y += 48.0;
    widgets::button(
        cx,
        render::rect(pane.x() + 20.0, y, 120.0, 32.0),
        if st.status.muted { "Unmute" } else { "Mute" },
        widgets::Style::Secondary,
        Action::MuteToggle,
    );
    // PipeWire's own mixer for everything this row does not cover: per-app
    // streams, input devices, and Bluetooth headset profiles.
    widgets::button(
        cx,
        render::rect(pane.x() + 152.0, y, 214.0, 32.0),
        "Outputs, inputs and apps…",
        widgets::Style::Ghost,
        Action::OpenTool(Tool::SoundSettings),
    );
    y += 52.0;
    paragraph(
        cx,
        pane,
        &mut y,
        "Volume keys are handled by the shell, not by a daemon, and the default \
         output is whatever PipeWire selects. Bluetooth headsets appear as a \
         normal output device once paired.",
    );
}

fn power(cx: &mut Ctx, pane: Rect) {
    let st = cx.st;
    let mut y = heading(cx, pane, "Power", "Battery, suspend and shutdown.");
    if let Some((level, charging)) = st.status.battery {
        let icon = render::rect(pane.x() + 20.0, y + 2.0, 46.0, 20.0);
        icons::draw_battery(cx.pm, icon, theme::ink(), level, charging);
        let status = if charging { "Charging" } else { "On battery" };
        cx.t(
            pane.x() + 78.0,
            y,
            26.0,
            &format!("{}% · {status}", (level * 100.0).round() as i32),
            theme::T_BODY,
            MEDIUM,
            theme::ink(),
        );
        y += 36.0;
    } else {
        cx.t(
            pane.x() + 20.0,
            y,
            24.0,
            "Desktop computer · mains power",
            theme::T_BODY,
            MEDIUM,
            theme::ink(),
        );
        y += 36.0;
    }
    paragraph(
        cx,
        pane,
        &mut y,
        "Closing the lid suspends the machine. Lock the screen from quick \
         settings when you step away; the session stays exactly where you left it.",
    );
    y += 12.0;
    let bw = 132.0;
    widgets::button(
        cx,
        render::rect(pane.x() + 20.0, y, bw, 32.0),
        "Suspend",
        widgets::Style::Secondary,
        Action::Power(PowerAction::Suspend),
    );
    widgets::button(
        cx,
        render::rect(pane.x() + 20.0 + bw + 10.0, y, bw, 32.0),
        "Restart",
        widgets::Style::Secondary,
        Action::Power(PowerAction::Restart),
    );
    widgets::button(
        cx,
        render::rect(pane.x() + 20.0 + (bw + 10.0) * 2.0, y, bw, 32.0),
        "Shut down",
        widgets::Style::Danger,
        Action::Power(PowerAction::ShutDown),
    );
}

fn about(cx: &mut Ctx, pane: Rect) {
    let st = cx.st;
    let mut y = heading(cx, pane, "About", "Reawakening OS · built on Debian 13 (trixie)");
    let rows: [(&str, String); 5] = [
        ("Computer", st.status.hostname.clone()),
        ("Processor", st.status.cpu.clone()),
        (
            "Memory",
            format!(
                "{} MB total · {} MB available",
                st.status.mem_total_mb, st.status.mem_available_mb
            ),
        ),
        (
            "Desktop",
            format!("Reawakening shell {} · labwc (Wayland) · CPU rendering", st.version),
        ),
        ("Browser", "Brave (default)".to_string()),
    ];
    for (label, value) in rows {
        cx.t(
            pane.x() + 20.0,
            y,
            20.0,
            label,
            theme::T_SMALL,
            REGULAR,
            theme::ink_faint(),
        );
        cx.t(
            pane.x() + 120.0,
            y,
            20.0,
            &value,
            theme::T_BODY,
            MEDIUM,
            theme::ink(),
        );
        y += 26.0;
    }
    y += 10.0;
    paragraph(
        cx,
        pane,
        &mut y,
        "Reawakening OS is free software, licensed under the GPL-3.0. It keeps \
         the essentials only: no indexing, no telemetry, no cloud sync and no \
         background catalogue. Administration is available through Cockpit when \
         you open it, and through the terminal.",
    );
}
