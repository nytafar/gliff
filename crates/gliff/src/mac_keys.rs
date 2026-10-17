//! macOS key codes to evdev. GDK on Linux reports a key's evdev code plus 8
//! as its hardware keycode; on macOS it reports the native virtual key code
//! (`kVK_*` in Carbon's `HIToolbox/Events.h`), which the server cannot use.
//! Keys map by position: each sends the code of the key in the same place
//! on a PC keyboard, and the remote's layout decides what it types. Command
//! is the remote's Super and Option its Alt.

/// Caps Lock's evdev code. macOS reports Caps Lock as a press when the lock
/// engages and a release when it lets go, each for a whole tap.
pub const CAPS_LOCK: u32 = 58;

/// The evdev code of a macOS key code, or `None` for fn (Globe), which
/// macOS keeps to itself, and for codes no key sends.
pub fn evdev(code: u32) -> Option<u32> {
    Some(match code {
        0x00 => 30,        // KEY_A
        0x01 => 31,        // KEY_S
        0x02 => 32,        // KEY_D
        0x03 => 33,        // KEY_F
        0x04 => 35,        // KEY_H
        0x05 => 34,        // KEY_G
        0x06 => 44,        // KEY_Z
        0x07 => 45,        // KEY_X
        0x08 => 46,        // KEY_C
        0x09 => 47,        // KEY_V
        0x0a => 86,        // ISO section: KEY_102ND
        0x0b => 48,        // KEY_B
        0x0c => 16,        // KEY_Q
        0x0d => 17,        // KEY_W
        0x0e => 18,        // KEY_E
        0x0f => 19,        // KEY_R
        0x10 => 21,        // KEY_Y
        0x11 => 20,        // KEY_T
        0x12 => 2,         // KEY_1
        0x13 => 3,         // KEY_2
        0x14 => 4,         // KEY_3
        0x15 => 5,         // KEY_4
        0x16 => 7,         // KEY_6
        0x17 => 6,         // KEY_5
        0x18 => 13,        // KEY_EQUAL
        0x19 => 10,        // KEY_9
        0x1a => 8,         // KEY_7
        0x1b => 12,        // KEY_MINUS
        0x1c => 9,         // KEY_8
        0x1d => 11,        // KEY_0
        0x1e => 27,        // KEY_RIGHTBRACE
        0x1f => 24,        // KEY_O
        0x20 => 22,        // KEY_U
        0x21 => 26,        // KEY_LEFTBRACE
        0x22 => 23,        // KEY_I
        0x23 => 25,        // KEY_P
        0x24 => 28,        // Return: KEY_ENTER
        0x25 => 38,        // KEY_L
        0x26 => 36,        // KEY_J
        0x27 => 40,        // KEY_APOSTROPHE
        0x28 => 37,        // KEY_K
        0x29 => 39,        // KEY_SEMICOLON
        0x2a => 43,        // KEY_BACKSLASH
        0x2b => 51,        // KEY_COMMA
        0x2c => 53,        // KEY_SLASH
        0x2d => 49,        // KEY_N
        0x2e => 50,        // KEY_M
        0x2f => 52,        // KEY_DOT
        0x30 => 15,        // KEY_TAB
        0x31 => 57,        // KEY_SPACE
        0x32 => 41,        // KEY_GRAVE
        0x33 => 14,        // Delete: KEY_BACKSPACE
        0x35 => 1,         // KEY_ESC
        0x36 => 126,       // right Command: KEY_RIGHTMETA
        0x37 => 125,       // Command: KEY_LEFTMETA
        0x38 => 42,        // KEY_LEFTSHIFT
        0x39 => CAPS_LOCK, // KEY_CAPSLOCK
        0x3a => 56,        // Option: KEY_LEFTALT
        0x3b => 29,        // KEY_LEFTCTRL
        0x3c => 54,        // KEY_RIGHTSHIFT
        0x3d => 100,       // right Option: KEY_RIGHTALT
        0x3e => 97,        // KEY_RIGHTCTRL
        0x40 => 187,       // KEY_F17
        0x41 => 83,        // KEY_KPDOT
        0x43 => 55,        // KEY_KPASTERISK
        0x45 => 78,        // KEY_KPPLUS
        0x47 => 69,        // keypad Clear: KEY_NUMLOCK
        0x48 => 115,       // KEY_VOLUMEUP
        0x49 => 114,       // KEY_VOLUMEDOWN
        0x4a => 113,       // KEY_MUTE
        0x4b => 98,        // KEY_KPSLASH
        0x4c => 96,        // KEY_KPENTER
        0x4e => 74,        // KEY_KPMINUS
        0x4f => 188,       // KEY_F18
        0x50 => 189,       // KEY_F19
        0x51 => 117,       // KEY_KPEQUAL
        0x52 => 82,        // KEY_KP0
        0x53 => 79,        // KEY_KP1
        0x54 => 80,        // KEY_KP2
        0x55 => 81,        // KEY_KP3
        0x56 => 75,        // KEY_KP4
        0x57 => 76,        // KEY_KP5
        0x58 => 77,        // KEY_KP6
        0x59 => 71,        // KEY_KP7
        0x5a => 190,       // KEY_F20
        0x5b => 72,        // KEY_KP8
        0x5c => 73,        // KEY_KP9
        0x5d => 124,       // JIS Yen: KEY_YEN
        0x5e => 89,        // JIS underscore: KEY_RO
        0x5f => 95,        // JIS keypad comma: KEY_KPJPCOMMA
        0x60 => 63,        // KEY_F5
        0x61 => 64,        // KEY_F6
        0x62 => 65,        // KEY_F7
        0x63 => 61,        // KEY_F3
        0x64 => 66,        // KEY_F8
        0x65 => 67,        // KEY_F9
        0x66 => 123,       // JIS Eisu: KEY_HANJA
        0x67 => 87,        // KEY_F11
        0x68 => 122,       // JIS Kana: KEY_HANGEUL
        0x69 => 183,       // F13, a PC keyboard's Print Screen: KEY_F13
        0x6a => 186,       // KEY_F16
        0x6b => 184,       // F14, a PC keyboard's Scroll Lock: KEY_F14
        0x6d => 68,        // KEY_F10
        0x6e => 127,       // context menu: KEY_COMPOSE
        0x6f => 88,        // KEY_F12
        0x71 => 185,       // F15, a PC keyboard's Pause: KEY_F15
        0x72 => 110,       // Help, a PC keyboard's Insert: KEY_INSERT
        0x73 => 102,       // KEY_HOME
        0x74 => 104,       // KEY_PAGEUP
        0x75 => 111,       // forward Delete: KEY_DELETE
        0x76 => 62,        // KEY_F4
        0x77 => 107,       // KEY_END
        0x78 => 60,        // KEY_F2
        0x79 => 109,       // KEY_PAGEDOWN
        0x7a => 59,        // KEY_F1
        0x7b => 105,       // KEY_LEFT
        0x7c => 106,       // KEY_RIGHT
        0x7d => 108,       // KEY_DOWN
        0x7e => 103,       // KEY_UP
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_keys_share_a_code() {
        let mut seen = std::collections::BTreeMap::new();
        for code in 0..=0xff {
            if let Some(ev) = evdev(code) {
                if let Some(other) = seen.insert(ev, code) {
                    panic!("{code:#x} and {other:#x} both map to {ev}");
                }
            }
        }
        assert_eq!(seen.len(), 119);
    }

    #[test]
    fn rows_follow_the_pc_layout() {
        // kVK codes for QWERTYUIOP, ASDFGHJKL, ZXCVBNM: each row runs over
        // consecutive evdev codes.
        let rows: [&[u32]; 3] = [
            &[0x0c, 0x0d, 0x0e, 0x0f, 0x11, 0x10, 0x20, 0x22, 0x1f, 0x23],
            &[0x00, 0x01, 0x02, 0x03, 0x05, 0x04, 0x26, 0x28, 0x25],
            &[0x06, 0x07, 0x08, 0x09, 0x0b, 0x2d, 0x2e],
        ];
        for (row, first) in rows.iter().zip([16, 30, 44]) {
            let codes: Vec<u32> = row.iter().map(|&k| evdev(k).unwrap()).collect();
            let expected: Vec<u32> = (first..first + row.len() as u32).collect();
            assert_eq!(codes, expected);
        }
    }

    #[test]
    fn modifiers_are_the_remote_ones() {
        assert_eq!(evdev(0x37), Some(125)); // Command: Super
        assert_eq!(evdev(0x3a), Some(56)); // Option: Alt
        assert_eq!(evdev(0x3b), Some(29)); // Control
        assert_eq!(evdev(0x38), Some(42)); // Shift
        assert_eq!(evdev(0x39), Some(CAPS_LOCK));
    }

    #[test]
    fn fn_and_unused_codes_have_no_key() {
        for code in [0x34, 0x3f, 0x42, 0x44, 0x46, 0x4d, 0x6c, 0x70, 0x7f, 0x80] {
            assert_eq!(evdev(code), None, "{code:#x}");
        }
    }
}
