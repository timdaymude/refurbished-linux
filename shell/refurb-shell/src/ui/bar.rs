//! The top bar: brand, application menu, taskbar, status and clock.

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
    cx.hit(render::rect(6.0, 0.0, 27.0, h), Action::LauncherOpen);

    // ---- application menu button -----------------------------------------
    let btn = render::rect(chip.right() + 7.0, 3.0, 112.0, h - 6.0);
    let hot = cx.hot(btn);
    if hot {
        render::round(cx.pm, btn, theme::RADIUS_SM, theme::accent_soft());
    }
    let icon = render::rect(btn.x() + 9.0, btn.y() + (btn.height() - 15.0) / 2.0, 15.0, 15.0);
    cx.glyph(
        icons::Glyph::Grid,
        icon,
        if hot { theme::accent_deep() } else { theme::ink_soft() },
        2.0,
    );
    cx.t(
        icon.right() + 7.0,
        btn.y(),
        btn.height(),
        "Applications",
        theme::T_BAR,
        MEDIUM,
        if hot { theme::accent_deep() } else { theme::ink() },
    );
    cx.hit(btn, Action::LauncherOpen);

    let mut left = btn.right() + 10.0;

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
        let chip_w = 150.0f32.min(space / cx.st.toplevels.len().max(1) as f32 - 4.0);
        for (i, t) in cx.st.toplevels.iter().enumerate() {
            if chip_w < 44.0 {
                break;
            }
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
