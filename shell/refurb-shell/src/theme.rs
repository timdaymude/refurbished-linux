//! Refurbished Linux visual identity.
//!
//! Dark is the default, and it is the only place the palette is written: every
//! surface in the shell reads these constants, so the whole desktop changes
//! together. A light palette is a future option, not a second code path.
//! Nothing here relies on an icon theme, a GTK theme or a compositor
//! effect: every pixel is produced by the shell itself.

use tiny_skia::Color;

pub use crate::render::mix;

pub const BAR_H: f32 = 34.0;
pub const RADIUS: f32 = 10.0;
pub const RADIUS_SM: f32 = 7.0;
pub const RADIUS_LG: f32 = 14.0;

/// Outer padding used by cards and the bar.
pub const PAD: f32 = 10.0;

// ---- surfaces -------------------------------------------------------------

/// Desktop background (also the colour of the shell's "wallpaper").
pub fn paper() -> Color {
    Color::from_rgba8(0x14, 0x16, 0x1B, 0xFF)
}

/// Bar / launcher surface. Slightly lighter than the paper so the bar reads
/// as a separate plane without shadows.
pub fn panel() -> Color {
    Color::from_rgba8(0x1B, 0x1E, 0x24, 0xFF)
}

/// Cards and window bodies.
pub fn card() -> Color {
    Color::from_rgba8(0x20, 0x24, 0x2B, 0xFF)
}

/// Recessed areas: search fields, log views, list rows.
pub fn well() -> Color {
    Color::from_rgba8(0x17, 0x1A, 0x20, 0xFF)
}

pub fn ink() -> Color {
    Color::from_rgba8(0xEC, 0xEE, 0xF2, 0xFF)
}

pub fn ink_soft() -> Color {
    Color::from_rgba8(0xA7, 0xAE, 0xBB, 0xFF)
}

pub fn ink_faint() -> Color {
    Color::from_rgba8(0x73, 0x7B, 0x8A, 0xFF)
}

pub fn line() -> Color {
    Color::from_rgba8(0x2E, 0x34, 0x40, 0xFF)
}

pub fn line_soft() -> Color {
    Color::from_rgba8(0x26, 0x2B, 0x34, 0xFF)
}

// ---- accent ---------------------------------------------------------------

pub fn accent() -> Color {
    Color::from_rgba8(0x35, 0xC2, 0xA3, 0xFF)
}

pub fn accent_deep() -> Color {
    Color::from_rgba8(0x8F, 0xE9, 0xD2, 0xFF)
}

pub fn accent_soft() -> Color {
    Color::from_rgba8(0x17, 0x33, 0x2C, 0xFF)
}

/// Secondary accent, used for "attention" states such as a security update
/// badge. Warm clay rather than red so it does not read as an error.
pub fn clay() -> Color {
    Color::from_rgba8(0xE8, 0x83, 0x5C, 0xFF)
}

pub fn clay_soft() -> Color {
    Color::from_rgba8(0x3A, 0x23, 0x20, 0xFF)
}

/// Dimming layer behind the launcher.
pub fn scrim() -> Color {
    Color::from_rgba8(0x08, 0x09, 0x0C, 0x9E)
}

// ---- type scale -----------------------------------------------------------

pub const T_BAR: f32 = 12.0;
pub const T_SMALL: f32 = 12.0;
pub const T_BODY: f32 = 13.0;
pub const T_LABEL: f32 = 14.0;
pub const T_TITLE: f32 = 16.0;
pub const T_HEAD: f32 = 19.0;
pub const T_DISPLAY: f32 = 26.0;

// -- shared drawing helpers -------------------------------------------------

/// A soft inner highlight used on tiles and buttons instead of a shadow.
pub fn tile_bg(hot: bool) -> Color {
    if hot {
        accent_soft()
    } else {
        well()
    }
}

/// Muted background of the brand chip.
pub fn brand_bg() -> Color {
    accent()
}

/// Text colour that sits on `accent()`. On the dark palette the accent is light
/// enough that ink would be unreadable, so chips use the near-black ink.
pub fn on_accent() -> Color {
    Color::from_rgba8(0x08, 0x14, 0x11, 0xFF)
}
