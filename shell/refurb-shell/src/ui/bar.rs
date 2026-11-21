//! The bar: brand (the launcher), taskbar, status and clock.

use crate::apps;
use crate::text::{MEDIUM, REGULAR, SEMIBOLD};
use crate::{icons, render, theme};

use super::widgets::{self, Ctx};
use super::Action;

pub fn draw(cx: &mut Ctx, w: f32, h: f32) {
    render::fill(cx.pm, render::rect(0.0, h - 1.0, w, 1.0), theme::line());

    // ---- brand ------------------------------------------------------------
    let chip = render::rect(9.0, (h - 21.0) / 2.0, 21.0, 21.0);
    render::round(cx.pm, chip, 6.5, theme::accent());
    cx.glyph(
        icons::Glyph::Brand,
        render::rect(chip.x() + 3.0, chip.y() + 3.0, 15.0, 15.0),
        theme::card(),
        2.2,
    );
    // The brand mark is the one application launcher. A second "Applications"
    // button sat next to it and opened the same menu, which read as two
    // launchers that did not obviously do the same thing.
    cx.hit(render::rect(4.0, 0.0, chip.width() + 12.0, h), Action::LauncherOpen);

    let mut left = chip.right() + 12.0;

    // ---- status cluster (laid out from the right) ------------------------
    let mut right = w - 6.0;

    let gear = render::rect(right - 28.0, 3.0, 28.0, h - 6.0);
    widgets::icon_button(cx, gear, icons::Glyph::Gear, Action::QuickToggle, cx.st.quick_open);
    right = gear.x() - 4.0;

    // clock
    let clock_w = cx.measure(&cx.st.clock, theme::T_BAR + 0.5, SEMIBOLD);
    cx.tr(
        right,
        0.0,
        h,
        &cx.st.clock,
        theme::T_BAR + 0.5,
        SEMIBOLD,
        theme::ink(),
    );
    right -= clock_w + 10.0;

    // Dictation is either running or it is not: show it in the bar, where the
    // user is already looking, instead of a separate floating window that could
    // steal the focus from the field the text is going into.
    if let Some(state) = &cx.st.dictation {
        let error = state.starts_with("error");
        let label = match state.as_str() {
            "starting" => "Starting…".to_string(),
            "listening" => "Listening".to_string(),
            other => other
                .trim_start_matches("error:")
                .trim_start_matches("error")
                .trim()
                .to_string(),
        };
        let label = if label.is_empty() {
            "Dictation".to_string()
        } else {
            label
        };
        let shown = cx.ellip(&label, theme::T_BAR, MEDIUM, 190.0);
        let tw = cx.measure(&shown, theme::T_BAR, MEDIUM);
        let pill_w = tw + 30.0;
        if right - pill_w > left {
            let pill = render::rect(right - pill_w, 5.0, pill_w, h - 10.0);
            render::round(
                cx.pm,
                pill,
                pill.height() / 2.0,
                if error { theme::clay_soft() } else { theme::accent_soft() },
            );
            cx.glyph(
                icons::Glyph::Mic,
                render::rect(pill.x() + 8.0, pill.y() + 4.0, 14.0, 14.0),
                if error { theme::clay() } else { theme::accent_deep() },
                2.0,
            );
            cx.t(
                pill.x() + 26.0,
                pill.y(),
                pill.height(),
                &shown,
                theme::T_BAR,
                MEDIUM,
                if error { theme::clay() } else { theme::accent_deep() },
            );
            right = pill.x() - 6.0;
        }
    }

    // battery
    if let Some((level, charging)) = cx.st.status.battery {
        let pct = format!("{}%", (level * 100.0).round() as i32);
        let pct_w = cx.measure(&pct, theme::T_BAR, REGULAR);
        cx.tr(right, 0.0, h, &pct, theme::T_BAR, REGULAR, theme::ink_soft());
        let icon = render::rect(
            right - pct_w - 26.0,
            3.0,
            24.0,
            h - 6.0,
        );
        icons::draw_battery(cx.pm, icon, theme::ink_soft(), level, charging);
        right = icon.x() - 4.0;
    }

    // volume
    let vol_btn = render::rect(right - 28.0, 3.0, 28.0, h - 6.0);
    widgets::icon_button(
        cx,
        vol_btn,
        if cx.st.status.muted || cx.st.status.volume <= 0.001 {
            icons::Glyph::VolumeMute
        } else {
            icons::Glyph::Volume
        },
        Action::QuickToggle,
        false,
    );
    right = vol_btn.x() - 2.0;

    // bluetooth
    if cx.st.status.bt.available {
        let bt_btn = render::rect(right - 28.0, 3.0, 28.0, h - 6.0);
        widgets::icon_button(
            cx,
            bt_btn,
            icons::Glyph::Bluetooth,
            Action::BtList,
            cx.st.status.bt.connected.is_some(),
        );
        right = bt_btn.x() - 2.0;
    }

    // wi-fi
    if cx.st.status.wifi_present {
        let wifi_btn = render::rect(right - 28.0, 3.0, 28.0, h - 6.0);
        widgets::icon_button(
            cx,
            wifi_btn,
            if cx.st.status.wifi_enabled {
                icons::Glyph::Wifi
            } else {
                icons::Glyph::WifiOff
            },
            Action::WifiList,
            cx.st.status.wifi.is_some(),
        );
        right = wifi_btn.x() - 2.0;
    }

    // pending updates
    if cx.st.status.updates > 0 {
        let up = render::rect(right - 28.0, 3.0, 28.0, h - 6.0);
        widgets::icon_button(cx, up, icons::Glyph::Shield, Action::Section(super::Section::Updates), true);
        render::circle(cx.pm, up.right() - 7.0, up.y() + 6.0, 3.2, theme::clay());
        right = up.x() - 2.0;
    }

    // ---- taskbar ---------------------------------------------------------
    let space = right - left - 6.0;
    if space > 60.0 {
        // Pinned applications come first, so a window can never push them off
        // the edge of the bar. They are drawn whether or not they are running;
        // the accent background and the pip are the running state.
        let pinned: Vec<(usize, bool)> = cx
            .st
            .taskbar
            .iter()
            .map(|&index| {
                let running = cx
                    .st
                    .apps
                    .get(index)
                    .map(|app| {
                        cx.st
                            .toplevels
                            .iter()
                            .any(|t| apps::matches_toplevel(app, &t.app_id))
                    })
                    .unwrap_or(false);
                (index, running)
            })
            .collect();
        for (index, running) in pinned {
            let r = render::rect(left, 4.0, 34.0, h - 8.0);
            if r.right() > right {
                break;
            }
            if running {
                render::round(cx.pm, r, theme::RADIUS_SM, theme::accent_soft());
            } else if cx.hot(r) {
                render::round(cx.pm, r, theme::RADIUS_SM, theme::line_soft());
            }
            if let Some(app) = cx.st.apps.get(index) {
                cx.glyph(
                    super::glyph_for(app),
                    render::rect(r.x() + 9.0, r.y() + 6.0, 16.0, 16.0),
                    if running { theme::accent_deep() } else { theme::ink() },
                    1.9,
                );
            }
            if running {
                render::circle(cx.pm, r.x() + r.width() / 2.0, r.bottom() - 3.0, 2.0, theme::accent());
            }
            cx.hit(r, Action::LaunchPinned(index));
            left = r.right() + 4.0;
        }

        // Window chips for everything that is not one of those pinned
        // applications. A second Brave window focuses the same single icon
        // instead of adding a duplicate, which is what was asked for.
        let windows: Vec<usize> = cx
            .st
            .toplevels
            .iter()
            .enumerate()
            .filter(|(_, toplevel)| {
                !cx.st.taskbar.iter().any(|&index| {
                    cx.st
                        .apps
                        .get(index)
                        .map(|app| apps::matches_toplevel(app, &toplevel.app_id))
                        .unwrap_or(false)
                })
            })
            .map(|(index, _)| index)
            .collect();
        let chip_w = 150.0f32.min(space / windows.len().max(1) as f32 - 4.0);
        for &i in &windows {
            if chip_w < 44.0 {
                break;
            }
            let t = &cx.st.toplevels[i];
            let r = render::rect(left, 4.0, chip_w, h - 8.0);
            if r.right() > right {
                break;
            }
            let hit_hot = cx.hot(r);
            if hit_hot || t.active {
                render::round(
                    cx.pm,
                    r,
                    theme::RADIUS_SM,
                    if t.active { theme::accent_soft() } else { theme::line_soft() },
                );
            }
            let dot = render::rect(r.x() + 8.0, r.y() + (r.height() - 12.0) / 2.0, 12.0, 12.0);
            cx.glyph(
                icons::Glyph::Dot,
                dot,
                if t.active { theme::accent() } else { theme::ink_faint() },
                1.9,
            );
            let title = cx.ellip(&t.title, theme::T_BAR, MEDIUM, chip_w - 34.0);
            cx.t(
                dot.right() + 5.0,
                r.y(),
                r.height(),
                &title,
                theme::T_BAR,
                MEDIUM,
                if t.active { theme::accent_deep() } else { theme::ink() },
            );
            cx.hit(r, Action::ActivateToplevel(i));
            left = r.right() + 4.0;
        }
    }

    // A hairline separates the taskbar from the status area when a window is
    // focused, which is the only "decoration" the bar uses.
    if !cx.st.toplevels.is_empty() && right - left > 8.0 {
        render::vline(cx.pm, right + 3.0, h * 0.3, h * 0.4, theme::line());
    }
}
