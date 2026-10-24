//! The names of keys: full ones for the Settings window (`Chord::label`)
//! and short ones for key caps and tooltips (`Chord::short`), 1.6.0-dev5.
//!
//! The punctuation keys (Windows virtual-key codes 0xBA-0xC0, 0xDB-0xDF
//! and 0xE2) sit in different places on different keyboards, so they are
//! named by the character the key types on the player's keyboard layout
//! (`platform_input::layout_char`), with a US table as the fallback.
use std::fmt::Write;

/// Write the name of key `code` into `out`: the short one for a cap
/// (`short`), else the full one. A key with no name is `Key <code>` or
/// `K<code>`.
pub fn write(code: u8, short: bool, out: &mut String) {
    if let Some(c) = punctuation(code, crate::platform_input::layout_char(code)) {
        out.push(c);
        return;
    }
    match code {
        0x30..=0x39 | 0x41..=0x5a => out.push(code as char),
        0x60..=0x69 => {
            let _ = write!(out, "{}{}", if short { "N" } else { "Num " }, code - 0x60);
        }
        0x70..=0x87 => {
            let _ = write!(out, "F{}", code - 0x6f);
        }
        _ => match names(code) {
            Some((full, cap)) => out.push_str(if short { cap } else { full }),
            None if short => {
                let _ = write!(out, "K{code}");
            }
            None => {
                let _ = write!(out, "Key {code}");
            }
        },
    }
}
/// The full and the short name of a named key.
fn names(code: u8) -> Option<(&'static str, &'static str)> {
    Some(match code {
        1 => ("Mouse1", "M1"),
        2 => ("Mouse2", "M2"),
        4 => ("Mouse3", "M3"),
        5 => ("Mouse4", "M4"),
        6 => ("Mouse5", "M5"),
        8 => ("Backspace", "Bksp"),
        9 => ("Tab", "Tab"),
        0x0d => ("Enter", "Ent"),
        0x10 => ("Shift", "Shift"),
        0x11 => ("Ctrl", "Ctrl"),
        0x12 => ("Alt", "Alt"),
        0x13 => ("Pause", "Paus"),
        0x14 => ("Caps Lock", "Caps"),
        0x1b => ("Esc", "Esc"),
        0x20 => ("Space", "Spc"),
        0x21 => ("Page Up", "PgUp"),
        0x22 => ("Page Down", "PgDn"),
        0x23 => ("End", "End"),
        0x24 => ("Home", "Home"),
        0x25 => ("Left", "Lft"),
        0x26 => ("Up", "Up"),
        0x27 => ("Right", "Rgt"),
        0x28 => ("Down", "Dn"),
        0x2c => ("Print Screen", "PrtSc"),
        0x2d => ("Insert", "Ins"),
        0x2e => ("Delete", "Del"),
        // 0x6c, the numeric pad's separator, has no key on most keyboards.
        0x6a => ("Num *", "N*"),
        0x6b => ("Num +", "N+"),
        0x6d => ("Num -", "N-"),
        0x6e => ("Num .", "N."),
        0x6f => ("Num /", "N/"),
        0x90 => ("Num Lock", "NumLk"),
        0x91 => ("Scroll Lock", "ScrLk"),
        _ => return None,
    })
}
/// The character a punctuation key shows: `layout` is what the keyboard
/// layout answered for it (`MapVirtualKeyW` with `MAPVK_VK_TO_CHAR`; 0
/// when it knows none, and always 0 in tests). A dead key (the top bit
/// set) still names its character; a letter is shown upper case, as the
/// letter keys are. Without a usable answer the US character, if any.
/// None for every other key.
pub fn punctuation(code: u8, layout: u32) -> Option<char> {
    if !matches!(code, 0xba..=0xc0 | 0xdb..=0xdf | 0xe2) {
        return None;
    }
    let from_layout = char::from_u32(layout & 0x7fff_ffff)
        .filter(|c| !c.is_control() && !c.is_whitespace() && *c != '\0')
        .map(|c| {
            let mut upper = c.to_uppercase();
            match (upper.next(), upper.next()) {
                (Some(u), None) => u,
                _ => c,
            }
        });
    from_layout.or_else(|| us(code))
}
/// The character each punctuation key types on a US keyboard. 0xDF and
/// 0xE2 have none there.
fn us(code: u8) -> Option<char> {
    Some(match code {
        0xba => ';',
        0xbb => '=',
        0xbc => ',',
        0xbd => '-',
        0xbe => '.',
        0xbf => '/',
        0xc0 => '`',
        0xdb => '[',
        0xdc => '\\',
        0xdd => ']',
        0xde => '\'',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn name(code: u8, short: bool) -> String {
        let mut s = String::new();
        write(code, short, &mut s);
        s
    }
    #[test]
    fn punctuation_keys_fall_back_to_the_us_characters() {
        for (code, c) in [
            (0xba, ';'),
            (0xbb, '='),
            (0xbc, ','),
            (0xbd, '-'),
            (0xbe, '.'),
            (0xbf, '/'),
            (0xc0, '`'),
            (0xdb, '['),
            (0xdc, '\\'),
            (0xdd, ']'),
            (0xde, '\''),
        ] {
            // Tests never ask the layout, so both names are the US one.
            assert_eq!(name(code, true), c.to_string(), "{code:#x}");
            assert_eq!(name(code, false), c.to_string(), "{code:#x}");
            assert_eq!(punctuation(code, 0), Some(c));
        }
        // No US character: the code, as before.
        assert_eq!(name(0xdf, true), "K223");
        assert_eq!(name(0xe2, false), "Key 226");
        // Not a punctuation key: the layout's answer is not asked for.
        assert_eq!(punctuation(0x41, 'x' as u32), None);
        assert_eq!(punctuation(0xa0, ';' as u32), None);
    }
    #[test]
    fn punctuation_keys_follow_the_layouts_answer() {
        // German: 0xBA types ü, 0xC0 ö, 0xDE ä (upper case, as letters).
        assert_eq!(punctuation(0xba, 'ü' as u32), Some('Ü'));
        assert_eq!(punctuation(0xc0, 'Ö' as u32), Some('Ö'));
        assert_eq!(punctuation(0xbf, '#' as u32), Some('#'));
        // A dead key (French ^ on 0xDD) names its character.
        assert_eq!(punctuation(0xdd, 0x8000_0000 | '^' as u32), Some('^'));
        // 0xE2, the key beside the left Shift outside the US.
        assert_eq!(punctuation(0xe2, '<' as u32), Some('<'));
        // No usable answer: the US character.
        assert_eq!(punctuation(0xba, 0), Some(';'));
        assert_eq!(punctuation(0xba, 0x0d), Some(';'));
        assert_eq!(punctuation(0xba, 0x20), Some(';'));
        assert_eq!(punctuation(0xba, 0xd800), Some(';'));
        assert_eq!(punctuation(0xdf, 0), None);
    }
    #[test]
    fn the_keys_dev5_names_have_full_and_short_names() {
        for (code, full, short) in [
            (0x0d, "Enter", "Ent"),
            (0x1b, "Esc", "Esc"),
            (0x14, "Caps Lock", "Caps"),
            (0x13, "Pause", "Paus"),
            (0x2c, "Print Screen", "PrtSc"),
            (0x2d, "Insert", "Ins"),
            (0x2e, "Delete", "Del"),
            (0x21, "Page Up", "PgUp"),
            (0x22, "Page Down", "PgDn"),
            (0x90, "Num Lock", "NumLk"),
            (0x91, "Scroll Lock", "ScrLk"),
            (0x60, "Num 0", "N0"),
            (0x65, "Num 5", "N5"),
            (0x69, "Num 9", "N9"),
            (0x6a, "Num *", "N*"),
            (0x6b, "Num +", "N+"),
            (0x6d, "Num -", "N-"),
            (0x6e, "Num .", "N."),
            (0x6f, "Num /", "N/"),
            (0x6c, "Key 108", "K108"),
        ] {
            assert_eq!(name(code, false), full, "{code:#x}");
            assert_eq!(name(code, true), short, "{code:#x}");
        }
    }
}
