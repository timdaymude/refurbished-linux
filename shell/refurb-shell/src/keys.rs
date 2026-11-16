//! Keyboard input.
//!
//! The compositor sends an XKB keymap; if `libxkbcommon` is present it is
//! loaded at runtime so that every keyboard layout works. On machines without
//! that library (and in unit tests) a built-in US layout is used, so the shell
//! can always be typed into and never hard-depends on a C library at build
//! time.

use std::os::raw::{c_char, c_int, c_void};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Backspace,
    Tab,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Modifiers {
    pub depressed: u32,
    pub latched: u32,
    pub locked: u32,
    pub group: u32,
}

impl Modifiers {
    pub fn shift(&self) -> bool {
        self.depressed & 1 != 0 || self.locked & 1 != 0
    }
}

pub struct Keymap {
    api: Option<Xkb>,
}

struct Xkb {
    _lib: libloading::Library,
    state: *mut c_void,
    update_mask: unsafe extern "C" fn(*mut c_void, u32, u32, u32, u32, u32, u32) -> u32,
    key_get_one_sym: unsafe extern "C" fn(*mut c_void, u32) -> u32,
    key_get_utf32: unsafe extern "C" fn(*mut c_void, u32) -> u32,
}

// The XKB state is only touched from the session's single thread.
unsafe impl Send for Xkb {}

impl Keymap {
    /// A keymap that understands the US layout only.
    pub fn fallback() -> Self {
        Self { api: None }
    }

    /// Build from the keymap string delivered by `wl_keyboard.keymap`.
    pub fn from_keymap_string(text: &str) -> Self {
        Self {
            api: load_xkb(text),
        }
    }

    pub fn uses_xkb(&self) -> bool {
        self.api.is_some()
    }

    pub fn update_modifiers(&mut self, m: &Modifiers) {
        if let Some(api) = &self.api {
            unsafe {
                (api.update_mask)(
                    api.state,
                    m.depressed,
                    m.latched,
                    m.locked,
                    m.group,
                    0,
                    0,
                );
            }
        }
    }

    /// Translate an evdev key code (the code the Wayland `key` event carries).
    pub fn translate(&self, evdev: u32, shift: bool) -> Option<Key> {
        if let Some(api) = &self.api {
            let code = evdev + 8; // evdev -> X11 keycode offset
            let sym = unsafe { (api.key_get_one_sym)(api.state, code) };
            if let Some(k) = named_key(sym) {
                return Some(k);
            }
            let utf = unsafe { (api.key_get_utf32)(api.state, code) };
            if utf >= 0x20 {
                return char::from_u32(utf).map(Key::Char);
            }
            return None;
        }
        let code = evdev;
        if let Some(k) = fallback_named(code) {
            return Some(k);
        }
        fallback_char(code, shift).map(Key::Char)
    }
}

fn load_xkb(text: &str) -> Option<Xkb> {
    use libloading::Symbol;
    type CtxNew = unsafe extern "C" fn(u32) -> *mut c_void;
    type KeymapNew = unsafe extern "C" fn(*mut c_void, *const c_char, c_int, u32) -> *mut c_void;
    type StateNew = unsafe extern "C" fn(*mut c_void) -> *mut c_void;

    let text = std::ffi::CString::new(text).ok()?;
    unsafe {
        let lib = libloading::Library::new("libxkbcommon.so.0").ok()?;
        let ctx_new: Symbol<CtxNew> = lib.get(b"xkb_context_new\0").ok()?;
        let km_new: Symbol<KeymapNew> = lib.get(b"xkb_keymap_new_from_string\0").ok()?;
        let st_new: Symbol<StateNew> = lib.get(b"xkb_state_new\0").ok()?;
        let upd: Symbol<unsafe extern "C" fn(*mut c_void, u32, u32, u32, u32, u32, u32) -> u32> =
            lib.get(b"xkb_state_update_mask\0").ok()?;
        let sym: Symbol<unsafe extern "C" fn(*mut c_void, u32) -> u32> =
            lib.get(b"xkb_state_key_get_one_sym\0").ok()?;
        let utf: Symbol<unsafe extern "C" fn(*mut c_void, u32) -> u32> =
            lib.get(b"xkb_state_key_get_utf32\0").ok()?;

        let ctx = ctx_new(0);
        if ctx.is_null() {
            return None;
        }
        let keymap = km_new(ctx, text.as_ptr(), 1 /* TEXT_V1 */, 0);
        if keymap.is_null() {
            return None;
        }
        let state = st_new(keymap);
        if state.is_null() {
            return None;
        }
        Some(Xkb {
            state,
            update_mask: *upd,
            key_get_one_sym: *sym,
            key_get_utf32: *utf,
            _lib: lib,
        })
    }
}

fn named_key(sym: u32) -> Option<Key> {
    const ESCAPE: u32 = 0xff1b;
    const RETURN: u32 = 0xff0d;
    const KP_ENTER: u32 = 0xff8d;
    const BACKSPACE: u32 = 0xff08;
    const TAB: u32 = 0xff09;
    const DELETE: u32 = 0xffff;
    const LEFT: u32 = 0xff51;
    const UP: u32 = 0xff52;
    const RIGHT: u32 = 0xff53;
    const DOWN: u32 = 0xff54;
    const HOME: u32 = 0xff50;
    const END: u32 = 0xff57;
    match sym {
        ESCAPE => Some(Key::Escape),
        RETURN | KP_ENTER => Some(Key::Enter),
        BACKSPACE => Some(Key::Backspace),
        TAB => Some(Key::Tab),
        DELETE => Some(Key::Delete),
        LEFT => Some(Key::Left),
        UP => Some(Key::Up),
        RIGHT => Some(Key::Right),
        DOWN => Some(Key::Down),
        HOME => Some(Key::Home),
        END => Some(Key::End),
        _ => None,
    }
}

fn fallback_named(code: u32) -> Option<Key> {
    match code {
        1 => Some(Key::Escape),
        14 => Some(Key::Backspace),
        15 => Some(Key::Tab),
        28 | 96 => Some(Key::Enter),
        103 => Some(Key::Up),
        105 => Some(Key::Left),
        106 => Some(Key::Right),
        108 => Some(Key::Down),
        111 => Some(Key::Delete),
        102 => Some(Key::Home),
        107 => Some(Key::End),
        _ => None,
    }
}

/// US layout fallback: evdev code to character.
fn fallback_char(code: u32, shift: bool) -> Option<char> {
    let base = match code {
        2 => '1',
        3 => '2',
        4 => '3',
        5 => '4',
        6 => '5',
        7 => '6',
        8 => '7',
        9 => '8',
        10 => '9',
        11 => '0',
        12 => '-',
        13 => '=',
        16 => 'q',
        17 => 'w',
        18 => 'e',
        19 => 'r',
        20 => 't',
        21 => 'y',
        22 => 'u',
        23 => 'i',
        24 => 'o',
        25 => 'p',
        26 => '[',
        27 => ']',
        30 => 'a',
        31 => 's',
        32 => 'd',
        33 => 'f',
        34 => 'g',
        35 => 'h',
        36 => 'j',
        37 => 'k',
        38 => 'l',
        39 => ';',
        40 => '\'',
        41 => '`',
        43 => '\\',
        44 => 'z',
        45 => 'x',
        46 => 'c',
        47 => 'v',
        48 => 'b',
        49 => 'n',
        50 => 'm',
        51 => ',',
        52 => '.',
        53 => '/',
        57 => ' ',
        _ => return None,
    };
    if !shift {
        return Some(base);
    }
    Some(match base {
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '0' => ')',
        '-' => '_',
        '=' => '+',
        '[' => '{',
        ']' => '}',
        ';' => ':',
        '\'' => '"',
        '`' => '~',
        '\\' => '|',
        ',' => '<',
        '.' => '>',
        '/' => '?',
        c => c.to_ascii_uppercase(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_handles_letters_digits_and_shift() {
        let k = Keymap::fallback();
        assert!(!k.uses_xkb());
        assert_eq!(k.translate(30, false), Some(Key::Char('a')));
        assert_eq!(k.translate(30, true), Some(Key::Char('A')));
        assert_eq!(k.translate(2, true), Some(Key::Char('!')));
        assert_eq!(k.translate(57, false), Some(Key::Char(' ')));
        assert_eq!(k.translate(28, false), Some(Key::Enter));
        assert_eq!(k.translate(1, false), Some(Key::Escape));
        assert_eq!(k.translate(103, false), Some(Key::Up));
        assert_eq!(k.translate(200, false), None);
    }

    #[test]
    fn modifiers_report_shift() {
        let m = Modifiers {
            depressed: 1,
            ..Default::default()
        };
        assert!(m.shift());
        assert!(!Modifiers::default().shift());
    }
}
