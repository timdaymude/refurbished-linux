//! Quick settings: network, Bluetooth, sound, brightness and power.
//!
//! This is the panel a Windows user reaches for first — but Wi-Fi and Bluetooth
//! are handled here directly (through NetworkManager and BlueZ) instead of
//! launching a separate heavyweight control panel.

use tiny_skia::{Color, Rect};

use crate::system::PowerAction;
use crate::text::{MEDIUM, REGULAR, SEMIBOLD};
use crate::{icons, render, theme};

use super::widgets::{self, Ctx};
use super::{Action, Section, SliderKind, WindowKind};

const PAD: f32 = 14.0;
const GAP: f32 = 8.0;
const HEADER_H: f32 = 26.0;
const TOGGLE_H: f32 = 62.0;
const SLIDER_H: f32 = 30.0;
const ROW_H: f32 = 36.0;
const BTN_H: f32 = 30.0;

pub const CARD_W: f32 = 372.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Block {
    Header,
    Toggles,
    Volume,
    Brightness,
    Wifi(u8, bool),
    Bt(u8),
    Footer,
}

impl Block {
    fn height(self) -> f32 {
        match self {
            Block::Header => HEADER_H,
            Block::Toggles => TOGGLE_H,
            Block::Volume | Block::Brightness => SLIDER_H,
            Block::Wifi(rows, pending) => {
                18.0 + rows as f32 * ROW_H + if pending { 78.0 } else { 0.0 } + BTN_H + 8.0
            }
            Block::Bt(rows) => 18.0 + rows as f32 * ROW_H + BTN_H + 8.0,
            // divider start offset + label row + two button rows
            Block::Footer => 8.0 + BTN_H + GAP + BTN_H,
        }
    }
}

fn plan(st: &super::State) -> Vec<Block> {
    let mut out = vec![Block::Header, Block::Toggles, Block::Volume];
    if st.status.brightness.is_some() {
        out.push(Block::Brightness);
    }
    if st.wifi.open {
        let rows = st.wifi.networks.len().min(5) as u8;
        out.push(Block::Wifi(rows, st.wifi.pending.is_some()));
    }
    if st.bt.open {
        let rows = st.bt.devices.len().min(4) as u8;
        out.push(Block::Bt(rows));
    }
    out.push(Block::Footer);
    out
}

/// Surface height for the current state (the session resizes the layer
/// surface when a section is expanded).
pub fn height(st: &super::State) -> u32 {
    let blocks = plan(st);
    let sum: f32 = blocks.iter().map(|b| b.height()).sum();
    (PAD * 2.0 + sum + GAP * (blocks.len() as f32 - 1.0)).ceil() as u32
}

pub fn draw(cx: &mut Ctx, w: f32, h: f32) {
    let st = cx.st;
    let card = render::rect(4.0, 4.0, (w - 8.0).min(CARD_W), h - 8.0);
    widgets::panel(cx, card);
    cx.hit(card, Action::Nothing);

    let mut y = card.y() + PAD;
    for block in plan(st) {
        let x = card.x() + PAD;
        let width = card.width() - PAD * 2.0;
        match block {
            Block::Header => {
                cx.t(x, y, HEADER_H, "Quick settings", theme::T_TITLE, SEMIBOLD, theme::ink());
                let host = cx.ellip(&st.status.hostname, theme::T_SMALL, REGULAR, width * 0.5);
                cx.tr(
                    x + width,
                    y,
                    HEADER_H,
                    &host,
                    theme::T_SMALL,
                    REGULAR,
                    theme::ink_faint(),
                );
            }
            Block::Toggles => {
                let tile_w = (width - GAP) / 2.0;
                let wifi_on = st.status.wifi_enabled;
                let wifi_sub = st
                    .status
                    .wifi
                    .as_ref()
                    .map(|w| w.ssid.clone())
                    .unwrap_or_else(|| {
                        if wifi_on {
                            "Not connected".into()
                        } else {
                            "Off".into()
                        }
                    });
                widgets::toggle_tile(
                    cx,
                    render::rect(x, y, tile_w, TOGGLE_H),
                    icons::Glyph::Wifi,
                    "Wi-Fi",
                    &wifi_sub,
                    wifi_on,
                    Action::WifiList,
                );
                let bt_sub = if !st.status.bt.available {
                    "Not available".to_string()
                } else if !st.status.bt.powered {
                    "Off".to_string()
                } else {
                    st.status
                        .bt
                        .connected
                        .clone()
                        .unwrap_or_else(|| "On".into())
                };
                widgets::toggle_tile(
                    cx,
                    render::rect(x + tile_w + GAP, y, tile_w, TOGGLE_H),
                    icons::Glyph::Bluetooth,
                    "Bluetooth",
                    &bt_sub,
                    st.status.bt.available && st.status.bt.powered,
                    Action::BtList,
                );
            }
            Block::Volume => {
                let r = render::rect(x, y, width, SLIDER_H);
                let pct = (st.status.volume * 100.0).round() as i32;
                let label = if st.status.muted {
                    "Muted".to_string()
                } else {
                    format!("{pct}%")
                };
                // The track spans the whole 0..160% range (see
                // system::VOLUME_MAX); the fill is a fraction of the track.
                let value = if st.status.muted {
                    0.0
                } else {
                    st.status.volume / crate::system::VOLUME_MAX
                };
                let track = widgets::slider(
                    cx,
                    render::rect(r.x(), r.y(), r.width() - 46.0, r.height()),
                    if st.status.muted {
                        icons::Glyph::VolumeMute
                    } else {
                        icons::Glyph::Volume
                    },
                    value,
                    SliderKind::Volume,
                    Action::VolumeSet,
                );
                cx.tr(
                    r.right(),
                    r.y(),
                    r.height(),
                    &label,
                    theme::T_SMALL,
                    REGULAR,
                    theme::ink_soft(),
                );
                // Only the speaker icon toggles mute. Clicking the track used to
                // do both (set the volume and silently mute), which is why the
                // slider appeared to snap back to muted in 0.3.0.
                cx.hit(
                    render::rect(track.x() - 30.0, r.y(), 30.0, r.height()),
                    Action::MuteToggle,
                );
            }
            Block::Brightness => {
                let r = render::rect(x, y, width, SLIDER_H);
                let level = st.status.brightness.unwrap_or(0.6);
                widgets::slider(
                    cx,
                    render::rect(r.x(), r.y(), r.width() - 46.0, r.height()),
                    icons::Glyph::Display,
                    level,
                    SliderKind::Brightness,
                    Action::BrightnessSet,
                );
                cx.tr(
                    r.right(),
                    r.y(),
                    r.height(),
                    &format!("{}%", (level * 100.0).round() as i32),
                    theme::T_SMALL,
                    REGULAR,
                    theme::ink_soft(),
                );
            }
            Block::Wifi(rows, pending) => {
                let inner = render::rect(x, y, width, block.height());
                widgets::divider(cx, inner.x(), inner.y(), inner.width());
                widgets::section_label(cx, inner.x(), inner.y() + 5.0, "Wi-Fi networks");
                let mut ry = inner.y() + 20.0;
                if st.wifi.scanning {
                    cx.t(
                        inner.x(),
                        ry,
                        18.0,
                        "Scanning for networks…",
                        theme::T_SMALL,
                        REGULAR,
                        theme::ink_faint(),
                    );
                } else if st.wifi.networks.is_empty() {
                    cx.t(
                        inner.x(),
                        ry,
                        18.0,
                        "No networks found",
                        theme::T_SMALL,
                        REGULAR,
                        theme::ink_faint(),
                    );
                }
                for (i, net) in st.wifi.networks.iter().take(rows as usize).enumerate() {
                    let row = render::rect(inner.x(), ry + i as f32 * ROW_H, inner.width(), ROW_H - 2.0);
                    let hot = cx.hot(row);
                    if hot {
                        render::round(cx.pm, row, theme::RADIUS_SM, theme::line_soft());
                    }
                    let strong = net.strength.min(100) as f32 / 100.0;
                    signal_bars(
                        cx.pm,
                        render::rect(row.x() + 2.0, row.y() + 12.0, 14.0, 12.0),
                        strong,
                        if net.active { theme::accent() } else { theme::ink_soft() },
                    );
                    let name = cx.ellip(&net.ssid, theme::T_BODY, MEDIUM, row.width() - 92.0);
                    cx.t(
                        row.x() + 24.0,
                        row.y(),
                        row.height(),
                        &name,
                        theme::T_BODY,
                        MEDIUM,
                        theme::ink(),
                    );
                    if net.secured() {
                        icons::draw(
                            cx.pm,
                            icons::Glyph::Lock,
                            render::rect(row.right() - 74.0, row.y() + 10.0, 14.0, 14.0),
                            theme::ink_faint(),
                            1.9,
                        );
                    }
                    if net.active {
                        widgets::badge_right(
                            cx,
                            row.right() - 10.0,
                            row.y() + (row.height() - 17.0) / 2.0,
                            "Connected",
                            theme::accent_deep(),
                            theme::accent_soft(),
                        );
                    }
                    cx.hit(row, Action::WifiPick(i));
                }
                if pending {
                    let py = ry + rows as f32 * ROW_H + 4.0;
                    let target = cx.st.wifi.pending.clone().unwrap_or_default();
                    cx.t(
                        inner.x(),
                        py,
                        18.0,
                        &format!("Password for “{target}”"),
                        theme::T_SMALL,
                        MEDIUM,
                        theme::ink_soft(),
                    );
                    let field = render::rect(inner.x(), py + 20.0, inner.width() - 84.0, 28.0);
                    render::round(cx.pm, field, theme::RADIUS_SM, theme::well());
                    render::round_stroke(cx.pm, field, theme::RADIUS_SM, theme::accent(), 1.0);
                    let dots = "•".repeat(st.wifi.password.chars().count());
                    if dots.is_empty() {
                        cx.t(
                            field.x() + 9.0,
                            field.y(),
                            field.height(),
                            "Network password",
                            theme::T_BODY,
                            REGULAR,
                            theme::ink_faint(),
                        );
                    } else {
                        cx.t(
                            field.x() + 9.0,
                            field.y(),
                            field.height(),
                            &dots,
                            theme::T_BODY,
                            MEDIUM,
                            theme::ink(),
                        );
                    }
                    widgets::button(
                        cx,
                        render::rect(field.right() + 6.0, py + 20.0, 78.0, 28.0),
                        "Connect",
                        widgets::Style::Primary,
                        Action::WifiPasswordSubmit,
                    );
                    if let Some(err) = &st.wifi.error {
                        cx.t(
                            inner.x(),
                            py + 50.0,
                            18.0,
                            err,
                            theme::T_SMALL,
                            REGULAR,
                            theme::clay(),
                        );
                    }
                }
                let btn_y = ry + rows as f32 * ROW_H + if pending { 78.0 } else { 0.0 };
                widgets::button(
                    cx,
                    render::rect(inner.x(), btn_y, 108.0, BTN_H),
                    "Scan again",
                    widgets::Style::Secondary,
                    Action::WifiRescan,
                );
                widgets::button(
                    cx,
                    render::rect(inner.x() + 116.0, btn_y, 88.0, BTN_H),
                    "Turn off",
                    widgets::Style::Ghost,
                    Action::WifiPower,
                );
            }
            Block::Bt(rows) => {
                let inner = render::rect(x, y, width, block.height());
                widgets::divider(cx, inner.x(), inner.y(), inner.width());
                widgets::section_label(cx, inner.x(), inner.y() + 5.0, "Bluetooth devices");
                let mut ry = inner.y() + 20.0;
                if st.bt.scanning {
                    cx.t(
                        inner.x(),
                        ry,
                        18.0,
                        "Searching for devices nearby…",
                        theme::T_SMALL,
                        REGULAR,
                        theme::ink_faint(),
                    );
                } else if st.bt.devices.is_empty() {
                    cx.t(
                        inner.x(),
                        ry,
                        18.0,
                        "No devices paired yet",
                        theme::T_SMALL,
                        REGULAR,
                        theme::ink_faint(),
                    );
                }
                for (i, dev) in st.bt.devices.iter().take(rows as usize).enumerate() {
                    let row = render::rect(inner.x(), ry + i as f32 * ROW_H, inner.width(), ROW_H - 2.0);
                    let hot = cx.hot(row);
                    if hot {
                        render::round(cx.pm, row, theme::RADIUS_SM, theme::line_soft());
                    }
                    icons::draw(
                        cx.pm,
                        icons::Glyph::Bluetooth,
                        render::rect(row.x() + 2.0, row.y() + 10.0, 16.0, 16.0),
                        if dev.connected { theme::accent() } else { theme::ink_soft() },
                        1.9,
                    );
                    let name = cx.ellip(&dev.name, theme::T_BODY, MEDIUM, row.width() - 110.0);
                    cx.t(
                        row.x() + 26.0,
                        row.y(),
                        row.height(),
                        &name,
                        theme::T_BODY,
                        MEDIUM,
                        theme::ink(),
                    );
                    let action_label = if dev.connected {
                        "Disconnect"
                    } else if dev.paired {
                        "Connect"
                    } else {
                        "Pair"
                    };
                    cx.tr(
                        row.right() - 8.0,
                        row.y(),
                        row.height(),
                        action_label,
                        theme::T_SMALL,
                        MEDIUM,
                        theme::accent_deep(),
                    );
                    cx.hit(row, Action::BtToggleDevice(i));
                }
                let btn_y = ry + rows as f32 * ROW_H + 2.0;
                widgets::button(
                    cx,
                    render::rect(inner.x(), btn_y, 132.0, BTN_H),
                    "Search for devices",
                    widgets::Style::Secondary,
                    Action::BtRescan,
                );
                widgets::button(
                    cx,
                    render::rect(inner.x() + 140.0, btn_y, 88.0, BTN_H),
                    "Turn off",
                    widgets::Style::Ghost,
                    Action::BtPower,
                );
            }
            Block::Footer => {
                widgets::divider(cx, x, y, width);
                let by = y + 8.0;
                let bw = (width - GAP * 2.0) / 3.0;
                widgets::button(
                    cx,
                    render::rect(x, by, bw, BTN_H),
                    "Settings",
                    widgets::Style::Secondary,
                    Action::WindowOpen(WindowKind::Settings),
                );
                widgets::button(
                    cx,
                    render::rect(x + bw + GAP, by, bw, BTN_H),
                    "Software",
                    widgets::Style::Secondary,
                    Action::WindowOpen(WindowKind::Software),
                );
                let updates = if st.status.updates > 0 {
                    format!("Updates ({})", st.status.updates)
                } else {
                    "Updates".to_string()
                };
                widgets::button(
                    cx,
                    render::rect(x + (bw + GAP) * 2.0, by, bw, BTN_H),
                    &updates,
                    widgets::Style::Secondary,
                    Action::Section(Section::Updates),
                );
                let by2 = by + BTN_H + GAP;
                widgets::button(
                    cx,
                    render::rect(x, by2, bw, BTN_H),
                    "Lock",
                    widgets::Style::Ghost,
                    Action::Lock,
                );
                widgets::button(
                    cx,
                    render::rect(x + bw + GAP, by2, bw, BTN_H),
                    "Restart",
                    widgets::Style::Ghost,
                    Action::Power(PowerAction::Restart),
                );
                widgets::button(
                    cx,
                    render::rect(x + (bw + GAP) * 2.0, by2, bw, BTN_H),
                    "Shut down",
                    widgets::Style::Danger,
                    Action::Power(PowerAction::ShutDown),
                );
            }
        }
        y += block.height() + GAP;
    }
    bar_close_affordance(cx, card);
}

/// Small "×" in the header that closes the panel without changing anything.
fn bar_close_affordance(cx: &mut Ctx, card: Rect) {
    let r = render::rect(card.right() - 30.0, card.y() + 10.0, 20.0, 20.0);
    let hot = cx.hot(r);
    icons::draw(
        cx.pm,
        icons::Glyph::Close,
        render::rect(r.x() + 3.0, r.y() + 3.0, 14.0, 14.0),
        if hot { theme::ink() } else { theme::ink_faint() },
        2.0,
    );
    cx.hit(r, Action::QuickClose);
}

fn signal_bars(pm: &mut tiny_skia::Pixmap, r: Rect, strength: f32, color: Color) {
    let bars = 4.0;
    let lit = (strength * bars).ceil().max(1.0);
    for i in 0..4 {
        let hgt = 3.0 + i as f32 * 3.0;
        let bar = render::rect(
            r.x() + i as f32 * 3.6,
            r.bottom() - hgt,
            2.4,
            hgt,
        );
        render::round(
            pm,
            bar,
            1.0,
            if (i as f32) < lit { color } else { theme::line() },
        );
    }
}
