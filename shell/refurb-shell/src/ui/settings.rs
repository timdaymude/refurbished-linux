//! Settings: a single window with a section rail, in the shape Windows users
//! expect. Everything here is either a `/sys` read or an explicit user action.

use tiny_skia::Rect;

use crate::prefs;
use crate::system::PowerAction;
use crate::text::{MEDIUM, REGULAR, SEMIBOLD};
use crate::wallpaper;
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
        Section::Remote => remote(cx, pane),
        Section::Display => display(cx, pane),
        Section::Appearance => appearance(cx, pane),
        Section::Sound => sound(cx, pane),
        Section::Power => power(cx, pane),
        Section::About => about(cx, pane),
    }
}

pub(crate) fn heading(cx: &mut Ctx, pane: Rect, title: &str, subtitle: &str) -> f32 {
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

pub(crate) fn paragraph(cx: &mut Ctx, pane: Rect, y: &mut f32, text: &str) {
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
         package catalogue service. Debian's daily package-list refresh is \
         switched off in this image, so nothing reaches the network unless you \
         press Check, and nothing is installed unless you press Install.",
    );
    y += 8.0;

    let (upgrades, security) = (st.status.updates, st.status.updates_security);
    if upgrades > 0 {
        let text = format!(
            "{upgrades} update{} available ({security} security)",
            if upgrades == 1 { "" } else { "s" }
        );
        cx.t(pane.x() + 20.0, y, 20.0, &text, theme::T_BODY, MEDIUM, theme::clay());
        y += 22.0;
        if st.status.updates_kept > 0 {
            let text = format!(
                "{} more need{} a new package and cannot be installed here.",
                st.status.updates_kept,
                if st.status.updates_kept == 1 { "s" } else { "" }
            );
            cx.t(
                pane.x() + 20.0,
                y,
                16.0,
                &text,
                theme::T_SMALL,
                REGULAR,
                theme::ink_soft(),
            );
            y += 18.0;
        }
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

/// Cockpit without a terminal: what the service is doing, the address another
/// computer uses, and one switch that asks for the account password. The URL
/// is shown as text and can be copied, because typing an IP address on a phone
/// is exactly the step where remote administration used to fail.
fn remote(cx: &mut Ctx, pane: Rect) {
    let st = cx.st;
    let mut y = heading(
        cx,
        pane,
        "Remote administration",
        "Cockpit: administration and a terminal in a browser, here or on another computer.",
    );

    let card = render::rect(pane.x() + 20.0, y, pane.width() - 40.0, 94.0);
    render::round(cx.pm, card, theme::RADIUS_SM, theme::well());
    let label_x = card.x() + 14.0;
    let value_x = card.x() + 186.0;
    let mut row = card.y() + 12.0;
    cx.t(label_x, row, 20.0, "Cockpit service", theme::T_SMALL, REGULAR, theme::ink_faint());
    let (service, service_tone) = if st.remote.socket_active {
        ("Available", theme::ink())
    } else {
        ("Not running", theme::clay())
    };
    cx.t(value_x, row, 20.0, service, theme::T_BODY, MEDIUM, service_tone);
    row += 28.0;
    cx.t(label_x, row, 20.0, "Network access", theme::T_SMALL, REGULAR, theme::ink_faint());
    let (access, access_tone) = if st.remote.enabled {
        ("Allowed on this network", theme::accent_deep())
    } else {
        ("This computer only", theme::ink_soft())
    };
    cx.t(value_x, row, 20.0, access, theme::T_BODY, MEDIUM, access_tone);
    row += 28.0;
    cx.t(label_x, row, 20.0, "Sign in as", theme::T_SMALL, REGULAR, theme::ink_faint());
    let user = if st.remote.user.is_empty() {
        "your account".to_string()
    } else {
        st.remote.user.clone()
    };
    cx.t(value_x, row, 20.0, &user, theme::T_BODY, MEDIUM, theme::ink());
    y = card.bottom() + 16.0;

    if st.remote.enabled {
        if st.remote.urls.is_empty() {
            cx.t(
                pane.x() + 20.0,
                y,
                20.0,
                "No network address yet: connect Wi-Fi or Ethernet first.",
                theme::T_SMALL,
                REGULAR,
                theme::ink_faint(),
            );
            y += 26.0;
        } else {
            widgets::section_label(cx, pane.x() + 20.0, y, "Open from another computer");
            y += 18.0;
            for (i, url) in st.remote.urls.iter().take(3).enumerate() {
                cx.t(pane.x() + 20.0, y, 24.0, url, theme::T_BODY, MEDIUM, theme::ink());
                widgets::button(
                    cx,
                    render::rect(pane.right() - 100.0, y - 3.0, 80.0, 26.0),
                    "Copy",
                    widgets::Style::Ghost,
                    Action::RemoteCopy(i),
                );
                y += 28.0;
            }
        }
        y += 6.0;
    }

    let (label, style) = if st.remote.enabled {
        ("Turn off network access", widgets::Style::Secondary)
    } else {
        ("Allow access from this network", widgets::Style::Primary)
    };
    widgets::button(
        cx,
        render::rect(pane.x() + 20.0, y, 236.0, 34.0),
        label,
        style,
        Action::RemoteToggle,
    );
    widgets::button(
        cx,
        render::rect(pane.x() + 268.0, y, 146.0, 34.0),
        "Open Cockpit",
        widgets::Style::Ghost,
        Action::RemoteOpen,
    );
    widgets::button(
        cx,
        render::rect(pane.x() + 426.0, y, 92.0, 34.0),
        "Refresh",
        widgets::Style::Ghost,
        Action::RemoteRefresh,
    );
    y += 44.0;

    if st.remote.busy {
        cx.t(
            pane.x() + 20.0,
            y,
            16.0,
            "Asking for your password…",
            theme::T_SMALL,
            REGULAR,
            theme::ink_soft(),
        );
        y += 18.0;
    }
    for line in st.remote.note.iter().rev().take(2) {
        cx.t(
            pane.x() + 20.0,
            y,
            16.0,
            line,
            theme::T_SMALL,
            REGULAR,
            theme::clay(),
        );
        y += 18.0;
    }
    y += 6.0;

    paragraph(
        cx,
        pane,
        &mut y,
        "Cockpit logs in with this computer's own account and password, and never with root \
         or a separate web password. The switch needs that password because exposing a login \
         page on the network is an administrator's decision. Do not forward port 9090 from a \
         router: this is for your own network only. The certificate is generated on this \
         machine, so the browser warns once.",
    );
}

fn display(cx: &mut Ctx, pane: Rect) {
    super::display::draw(cx, pane);
}

/// Appearance: every picture the desktop can paint, as thumbnails, plus the
/// file manager route for anything not listed and the clock style.
///
/// Both wallpaper routes — this page and "Set as Desktop Wallpaper" in Files —
/// end in the same place: a managed copy under `~/.local/share/reawakening` and
/// one line in `~/.config/reawakening/wallpaper`.
fn appearance(cx: &mut Ctx, pane: Rect) {
    let current = cx.st.wallpaper.clone();
    let mut y = heading(
        cx,
        pane,
        "Appearance",
        "Wallpapers are drawn by the session at every screen's own resolution.",
    );
    let entries = &cx.st.wallpapers;
    let gap = 8.0;
    let cols = 4usize;
    let card_w = (pane.width() - 40.0 - gap * (cols as f32 - 1.0)) / cols as f32;
    let thumb_h = (card_w * 0.54).clamp(58.0, 104.0);
    let card_h = thumb_h + 30.0;
    let mut hovered: Option<String> = None;
    for (i, entry) in entries.iter().take(cols * 2).enumerate() {
        let col = i % cols;
        let row = i / cols;
        let card = render::rect(
            pane.x() + 20.0 + col as f32 * (card_w + gap),
            y + row as f32 * (card_h + gap),
            card_w,
            card_h,
        );
        let active = entry.wallpaper == current;
        render::round(cx.pm, card, theme::RADIUS_SM, theme::well());
        let thumb = render::rect(card.x() + 5.0, card.y() + 5.0, card.width() - 10.0, thumb_h);
        if !wallpaper::draw_into(cx.pm, &entry.wallpaper, thumb) {
            render::round(cx.pm, thumb, 4.0, theme::line_soft());
            cx.t(
                thumb.x() + 8.0,
                thumb.y(),
                thumb.height(),
                "not installed",
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
        let label_w = if active { card.width() - 68.0 } else { card.width() - 14.0 };
        let label = cx.ellip(&entry.label, theme::T_SMALL, MEDIUM, label_w);
        cx.t(
            card.x() + 7.0,
            thumb.bottom() + 3.0,
            18.0,
            &label,
            theme::T_SMALL,
            MEDIUM,
            theme::ink(),
        );
        if active {
            widgets::badge_right(
                cx,
                card.right() - 5.0,
                thumb.bottom() + 4.0,
                "In use",
                theme::on_accent(),
                theme::accent(),
            );
        }
        if cx.hot(card) {
            hovered = Some(entry.blurb.clone());
        }
        cx.hit(card, Action::Wallpaper(i));
    }
    let rows = entries.len().min(cols * 2).div_ceil(cols).max(1);
    y += rows as f32 * (card_h + gap) + 2.0;

    if !current.is_builtin() && !current.exists() {
        cx.t(
            pane.x() + 20.0,
            y,
            18.0,
            "The picture you chose is no longer there. Pick another one.",
            theme::T_SMALL,
            MEDIUM,
            theme::clay(),
        );
        y += 20.0;
    } else if entries.len() > cols * 2 {
        cx.t(
            pane.x() + 20.0,
            y,
            18.0,
            &format!(
                "{} more of your own pictures are in Files.",
                entries.len() - cols * 2
            ),
            theme::T_SMALL,
            REGULAR,
            theme::ink_faint(),
        );
        y += 20.0;
    }

    // ---- your own picture -------------------------------------------------
    widgets::button(
        cx,
        render::rect(pane.x() + 20.0, y, 168.0, 30.0),
        "Choose a picture…",
        widgets::Style::Secondary,
        Action::WallpaperBrowse,
    );
    cx.t(
        pane.x() + 198.0,
        y,
        30.0,
        "or right-click a picture in Files → Open With → Set as Desktop Wallpaper",
        theme::T_SMALL - 0.5,
        REGULAR,
        theme::ink_faint(),
    );
    y += 40.0;

    // ---- the clock --------------------------------------------------------
    widgets::divider(cx, pane.x() + 20.0, y, pane.width() - 40.0);
    y += 10.0;
    widgets::section_label(cx, pane.x() + 20.0, y, "Clock");
    y += 17.0;
    // The hovered picture explains itself in one line, in the space the clock
    // row leaves. That is more useful than a fixed caption and costs no room.
    if let Some(blurb) = hovered {
        let line = cx.ellip(&blurb, theme::T_SMALL - 0.5, REGULAR, pane.width() - 40.0);
        cx.t(
            pane.x() + 20.0,
            y - 4.0,
            16.0,
            &line,
            theme::T_SMALL - 0.5,
            REGULAR,
            theme::ink_faint(),
        );
    }
    let mut bx = pane.x() + 20.0;
    for style in prefs::Clock::ALL {
        let w = if style == prefs::Clock::H12 { 138.0 } else { 132.0 };
        widgets::button(
            cx,
            render::rect(bx, y, w, 28.0),
            style.label(),
            if style == cx.st.clock_format {
                widgets::Style::Primary
            } else {
                widgets::Style::Secondary
            },
            Action::ClockFormat(style),
        );
        bx += w + 8.0;
    }
    let sample = prefs::format_time(17, 5, cx.st.clock_format);
    cx.t(
        bx + 4.0,
        y,
        28.0,
        &format!("Example: {sample}"),
        theme::T_SMALL,
        REGULAR,
        theme::ink_faint(),
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
