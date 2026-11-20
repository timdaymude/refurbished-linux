//! The power confirmation.
//!
//! Pressing "Shut down" or the machine's power key used to call
//! `systemctl poweroff` immediately: one click and the session, with every open
//! document, was gone. This is the modal that stands between the request and the
//! power action. It is a full-screen overlay so that a click outside the card
//! lands on the backdrop and does nothing instead of reaching the button behind
//! it, and it holds the keyboard while it is open: Escape cancels, Tab or the
//! arrow keys move between the two buttons and Enter activates the focused one.
//! Cancel has the focus when the dialog opens, so a stray Enter cancels.

use tiny_skia::Rect;

use crate::system::PowerAction;
use crate::text::{MEDIUM, REGULAR, SEMIBOLD};
use crate::{icons, render, theme};

use super::widgets::{self, Ctx};
use super::Action;

pub const CARD_W: f32 = 428.0;
pub const CARD_H: f32 = 212.0;

fn words(target: PowerAction) -> (&'static str, &'static str, &'static str) {
    match target {
        PowerAction::ShutDown => (
            "Shut down Reawakening OS?",
            "Open applications will be closed and the computer will turn off.",
            "Shut Down",
        ),
        PowerAction::Restart => (
            "Restart Reawakening OS?",
            "Open applications will be closed and the computer will restart.",
            "Restart",
        ),
        PowerAction::Suspend => (
            "Suspend Reawakening OS?",
            "The computer will sleep until you wake it.",
            "Suspend",
        ),
    }
}

pub fn draw(cx: &mut Ctx, w: f32, h: f32) {
    // The backdrop swallows every click. Registering it first keeps it below the
    // card in the hit map, because later regions take priority.
    render::fill(cx.pm, render::rect(0.0, 0.0, w, h), theme::scrim());
    cx.hit(render::rect(0.0, 0.0, w, h), Action::Nothing);

    let card = render::rect(
        ((w - CARD_W) / 2.0).max(12.0),
        ((h - CARD_H) / 2.0).max(12.0),
        CARD_W.min(w - 24.0),
        CARD_H.min(h - 24.0),
    );
    widgets::panel(cx, card);
    cx.hit(card, Action::Nothing);

    let target = cx.st.confirm;
    let (title, body, action) = match target {
        Some(target) => words(target),
        None => ("Power", "Nothing to confirm.", "OK"),
    };
    let danger = cx.st.confirm_danger;

    // Icon chip and title.
    let chip = render::rect(card.x() + 20.0, card.y() + 22.0, 34.0, 34.0);
    render::round(cx.pm, chip, theme::RADIUS_SM, theme::clay_soft());
    icons::draw(
        cx.pm,
        icons::Glyph::Power,
        render::rect(chip.x() + 8.0, chip.y() + 8.0, 18.0, 18.0),
        theme::clay(),
        2.0,
    );
    cx.t(
        chip.right() + 12.0,
        card.y() + 18.0,
        26.0,
        title,
        theme::T_TITLE,
        SEMIBOLD,
        theme::ink(),
    );
    cx.t(
        chip.right() + 12.0,
        card.y() + 40.0,
        18.0,
        "This is your only chance to cancel.",
        theme::T_SMALL,
        REGULAR,
        theme::ink_faint(),
    );

    // Body.
    let mut y = chip.bottom() + 16.0;
    let max_w = card.width() - 40.0;
    let mut line = String::new();
    for word in body.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if cx.measure(&candidate, theme::T_BODY, REGULAR) > max_w && !line.is_empty() {
            cx.t(card.x() + 20.0, y, 18.0, &line, theme::T_BODY, REGULAR, theme::ink_soft());
            y += 19.0;
            line = word.to_string();
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        cx.t(card.x() + 20.0, y, 18.0, &line, theme::T_BODY, REGULAR, theme::ink_soft());
    }

    // Buttons: Cancel on the left, the destructive action on the right. Focus is
    // drawn as a ring, not as a hover colour, so a pointer that happens to rest
    // on a button never looks like the keyboard selection.
    let bw = (card.width() - 40.0 - 10.0) / 2.0;
    let by = card.bottom() - 64.0;
    let cancel = render::rect(card.x() + 20.0, by, bw, 34.0);
    let confirm = render::rect(cancel.right() + 10.0, by, bw, 34.0);
    if !danger {
        focus_ring(cx, cancel);
    }
    widgets::button(cx, cancel, "Cancel", widgets::Style::Secondary, Action::DialogCancel);
    if danger {
        focus_ring(cx, confirm);
    }
    widgets::button(cx, confirm, action, widgets::Style::Danger, Action::DialogConfirm);

    cx.tc(
        card.x() + 20.0,
        card.width() - 40.0,
        card.bottom() - 22.0,
        14.0,
        "Escape cancels  ·  Tab chooses  ·  Enter confirms",
        theme::T_SMALL - 1.5,
        REGULAR,
        theme::ink_faint(),
    );
}

fn focus_ring(cx: &mut Ctx, r: Rect) {
    render::round_stroke(
        cx.pm,
        render::rect(r.x() - 2.0, r.y() - 2.0, r.width() + 4.0, r.height() + 4.0),
        theme::RADIUS_SM + 2.0,
        theme::accent(),
        1.5,
    );
}
