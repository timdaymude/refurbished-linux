//! Small user preferences that belong to the desktop itself: how the clock is
//! written, and nothing else yet.
//!
//! They live in one line each under `~/.config/reawakening/`, so they survive a
//! reboot without a daemon, a GSettings schema or a cache, and they can be read
//! by hand.

use std::io;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Clock {
    /// 24-hour, the Debian default. `17:05`.
    #[default]
    H24,
    /// 12-hour with AM/PM, for people who read a wall clock that way.
    H12,
}

impl Clock {
    pub const ALL: [Clock; 2] = [Clock::H24, Clock::H12];

    pub fn id(self) -> &'static str {
        match self {
            Clock::H24 => "24",
            Clock::H12 => "12",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Clock::H24 => "24-hour (17:05)",
            Clock::H12 => "12-hour (5:05 PM)",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Clock::H24 => "24-hour",
            Clock::H12 => "12-hour",
        }
    }

    pub fn from_text(text: &str) -> Clock {
        match text.trim() {
            "12" | "12h" | "12-hour" => Clock::H12,
            _ => Clock::H24,
        }
    }
}

fn config_dir() -> Option<PathBuf> {
    match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir)),
        _ => Some(PathBuf::from(std::env::var_os("HOME")?).join(".config")),
    }
}

fn clock_path() -> Option<PathBuf> {
    Some(config_dir()?.join("reawakening/clock"))
}

pub fn clock() -> Clock {
    clock_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|text| Clock::from_text(&text))
        .unwrap_or_default()
}

pub fn set_clock(clock: Clock) -> io::Result<()> {
    let Some(path) = clock_path() else {
        return Ok(());
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, format!("{}\n", clock.id()))
}

/// Format a wall-clock time in the chosen style. Kept separate from the system
/// call so the two formats can be tested without changing the machine's clock.
pub fn format_time(hour: u32, minute: u32, clock: Clock) -> String {
    match clock {
        Clock::H24 => format!("{:02}:{:02}", hour % 24, minute % 60),
        Clock::H12 => {
            let suffix = if hour % 24 < 12 { "AM" } else { "PM" };
            let mut h = hour % 12;
            if h == 0 {
                h = 12;
            }
            format!("{}:{:02} {}", h, minute % 60, suffix)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_clock_styles_read_correctly() {
        assert_eq!(format_time(0, 5, Clock::H24), "00:05");
        assert_eq!(format_time(17, 5, Clock::H24), "17:05");
        assert_eq!(format_time(0, 5, Clock::H12), "12:05 AM");
        assert_eq!(format_time(9, 30, Clock::H12), "9:30 AM");
        assert_eq!(format_time(12, 0, Clock::H12), "12:00 PM");
        assert_eq!(format_time(23, 59, Clock::H12), "11:59 PM");
    }

    #[test]
    fn the_saved_choice_is_read_back() {
        assert_eq!(Clock::from_text("12\n"), Clock::H12);
        assert_eq!(Clock::from_text("24"), Clock::H24);
        assert_eq!(Clock::from_text("nonsense"), Clock::H24);
        assert_eq!(Clock::default(), Clock::H24);
    }
}
