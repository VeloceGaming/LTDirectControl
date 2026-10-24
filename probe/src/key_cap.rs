//! Key caps and tooltips that name the player's own bindings (1.6.0-dev4).
//!
//! A window keeps one [`KeyText`] per cap in its view and calls
//! [`KeyText::set`] every frame with the chord bound now
//! (`Values::shown_chord`), so a rebind in Settings shows at once; the text
//! is rebuilt only when the chord changes. The short names are
//! `Chord::short` in `settings.rs`. A cap widens to fit its text with the
//! padding it had around its default key ([`width`]); an unbound action
//! shows no cap, and its tooltip drops its " · key" part ([`tip`]).
use crate::settings::{Chord, Values, BINDINGS};

/// The text one key cap shows and the chord it was made from.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyText {
    chord: Option<Chord>,
    text: String,
}
impl KeyText {
    /// The cap of binding `key` at its default chord.
    pub fn new(key: &str) -> Self {
        let chord = BINDINGS
            .iter()
            .find(|d| d.key == key)
            .and_then(|d| d.defaults.into_iter().flatten().next());
        let mut k = Self {
            chord: None,
            text: String::new(),
        };
        k.rebuild(chord);
        k
    }
    /// Follow the chord bound now; rebuilds the text only on a change.
    pub fn set(&mut self, chord: Option<Chord>) {
        if chord != self.chord {
            self.rebuild(chord);
        }
    }
    /// [`Self::set`] from binding `key` in `values`.
    pub fn follow(&mut self, values: &Values, key: &str) {
        self.set(values.shown_chord(key));
    }
    fn rebuild(&mut self, chord: Option<Chord>) {
        self.chord = chord;
        self.text.clear();
        if let Some(c) = chord {
            c.short(&mut self.text);
        }
    }
    /// The short key name; empty while the action is unbound.
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn chord(&self) -> Option<Chord> {
        self.chord
    }
}
/// A cap's width for `text`: `base` (its width today) while the text is no
/// wider than `widest`, the widest default text the cap was drawn for;
/// otherwise wider by the difference, rounded up to a whole pixel, so the
/// padding either side stays what `widest` had. Measured with the packaged
/// font advances (`hud_style::width`: the medium weight; the caps use the
/// numeric weight, about 2 % wider, so a grown cap is a fraction of a pixel
/// tighter than the default one).
pub fn width(text: &str, base: f32, size: f32, widest: &str) -> f32 {
    let grow = crate::hud_style::width(text, size) - crate::hud_style::width(widest, size);
    if grow <= 0. {
        base
    } else {
        (base + grow).ceil()
    }
}
/// A tooltip naming a key: `translated` is the current language's wording
/// of a text ending in " · {key}" (`lang::tr("Start · {key}")`, written at
/// the call so the translation tests find it). `{key}` becomes the short
/// name; an unbound action loses the " · {key}" part.
pub fn tip(translated: &str, key: &KeyText) -> String {
    if key.text().is_empty() {
        translated
            .replace("{key}", "")
            .trim_end_matches(|c: char| c.is_whitespace() || c == '·')
            .to_owned()
    } else {
        translated.replace("{key}", key.text())
    }
}
/// A tooltip naming several keys, one line per key (1.6.0-dev5: the
/// camera's "Lock camera · {key}" and "Hold {follow} to follow"): each
/// `{name}` becomes its key's short name. For an unbound key, a line that
/// ends with it loses its " · {name}" part as in [`tip`]; any other line
/// naming it is dropped.
pub fn tip_lines(translated: &str, keys: &[(&str, &KeyText)]) -> String {
    let mut out = String::new();
    'line: for line in translated.split('\n') {
        let mut line = line.to_owned();
        for (name, key) in keys {
            let slot = format!("{{{name}}}");
            if !line.contains(&slot) {
                continue;
            }
            if !key.text().is_empty() {
                line = line.replace(&slot, key.text());
            } else if line.trim_end().ends_with(&slot) {
                line = line
                    .replace(&slot, "")
                    .trim_end_matches(|c: char| c.is_whitespace() || c == '·')
                    .to_owned();
            } else {
                continue 'line;
            }
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The texts that name a key in a tooltip, as the windows look them up.
    const TIPS: [&str; 6] = [
        "Start · {key}",
        "Pause · {key}",
        "Resume · {key}",
        "Take control · {key}",
        "Return control to AI · {key}",
        "Team details · {key}",
    ];
    #[test]
    fn a_cap_rebuilds_its_text_only_when_the_chord_changes() {
        let mut k = KeyText::new("shop");
        assert_eq!(k.text(), "P");
        let capacity = k.text.capacity();
        for _ in 0..100 {
            k.set(Some(Chord {
                code: 0x50,
                mods: 0,
            }));
        }
        assert_eq!((k.text(), k.text.capacity()), ("P", capacity));
        k.set(Some(Chord {
            code: 0x42,
            mods: 1,
        }));
        assert_eq!(k.text(), "S+B");
        k.set(None);
        assert_eq!(k.text(), "");
        assert_eq!(KeyText::new("start").text(), "F11");
        assert_eq!(KeyText::new("tab").text(), "Tab");
    }
    #[test]
    fn caps_keep_todays_width_at_their_default_keys_and_grow_past_them() {
        // P and B: 18 px at 12 px; skills: 22 px at 14 px, W the widest.
        for key in ["P", "B"] {
            assert_eq!(width(key, 18., 12., "B"), 18.);
        }
        for key in ["Q", "W", "R", "1", "E"] {
            assert_eq!(width(key, 22., 14., "W"), 22.);
        }
        assert_eq!(width("Tab", 30., 13., "Tab"), 30.);
        assert_eq!(width("F11", 36., 14., "F11"), 36.);
        // Wider keys keep the padding the default had.
        let sb = width("S+B", 18., 12., "B");
        let measured = crate::hud_style::width("S+B", 12.) - crate::hud_style::width("B", 12.);
        assert_eq!(sb, (18. + measured).ceil());
        assert!(sb > 18. && sb < 40.);
        assert!(width("M4", 22., 14., "W") > 22.);
        assert!(width("S+F12", 30., 13., "Tab") > 30.);
        assert!(width("S+F11", 36., 14., "F11") > 36.);
        assert!(width("C+S+A+F12", 22., 14., "W") > width("S+F12", 22., 14., "W"));
    }
    #[test]
    fn tooltips_name_the_bound_key_or_drop_it_when_unbound() {
        let mut k = KeyText::new("start");
        assert_eq!(tip("Start · {key}", &k), "Start · F11");
        k.set(Some(Chord {
            code: 0x7a,
            mods: 1,
        }));
        assert_eq!(tip("開始 · {key}", &k), "開始 · S+F11");
        k.set(None);
        assert_eq!(tip("Start · {key}", &k), "Start");
        assert_eq!(tip("一時停止 · {key}", &k), "一時停止");
    }
    #[test]
    fn the_camera_tooltip_names_both_keys_or_drops_an_unbound_ones_part() {
        let mut lock = KeyText::new("camera_toggle");
        let mut follow = KeyText::new("follow");
        let text = "Lock camera · {key}\nHold {follow} to follow";
        let tip = |lock: &KeyText, follow: &KeyText| {
            tip_lines(text, &[("key", lock), ("follow", follow)])
        };
        assert_eq!(tip(&lock, &follow), "Lock camera · Y\nHold Spc to follow");
        follow.set(Some(Chord { code: 5, mods: 0 }));
        lock.set(Some(Chord {
            code: 0x59,
            mods: 1,
        }));
        assert_eq!(tip(&lock, &follow), "Lock camera · S+Y\nHold M4 to follow");
        follow.set(None);
        assert_eq!(tip(&lock, &follow), "Lock camera · S+Y");
        lock.set(None);
        assert_eq!(tip(&lock, &follow), "Lock camera");
        follow.set(Some(Chord {
            code: 0x20,
            mods: 0,
        }));
        assert_eq!(tip(&lock, &follow), "Lock camera\nHold Spc to follow");
        // A wording that names an unbound key mid-line (Japanese puts the
        // follow key first) drops that line.
        let mut none = KeyText::new("follow");
        none.set(None);
        let ja = "カメラ固定 · {key}\n{follow}長押しで追従";
        let y = KeyText::new("camera_toggle");
        assert_eq!(
            tip_lines(ja, &[("key", &y), ("follow", &none)]),
            "カメラ固定 · Y"
        );
        assert_eq!(
            tip_lines(ja, &[("key", &none), ("follow", &follow)]),
            "カメラ固定\nSpc長押しで追従"
        );
    }
    #[test]
    fn every_language_keeps_each_key_tooltip_with_one_key_at_its_end() {
        for (i, (code, _)) in crate::lang::LANGS.iter().enumerate().skip(1) {
            let table: std::collections::HashMap<String, String> =
                serde_json::from_str(crate::lang::FILES[i]).unwrap();
            for english in TIPS {
                let wording = table
                    .get(english)
                    .unwrap_or_else(|| panic!("{code}: missing {english:?}"));
                assert_eq!(wording.matches("{key}").count(), 1, "{code}: {wording:?}");
                assert!(wording.ends_with(" · {key}"), "{code}: {wording:?}");
                let unbound = tip(
                    wording,
                    &KeyText {
                        chord: None,
                        text: String::new(),
                    },
                );
                assert!(!unbound.is_empty() && !unbound.ends_with('·'), "{code}");
            }
            // The camera tooltip: its lock key at the first line's end,
            // its follow key once on the second line.
            let camera = table
                .get("Lock camera · {key}\nHold {follow} to follow")
                .unwrap_or_else(|| panic!("{code}: missing the camera tooltip"));
            let lines: Vec<&str> = camera.split('\n').collect();
            assert_eq!(lines.len(), 2, "{code}: {camera:?}");
            assert!(lines[0].ends_with(" · {key}"), "{code}: {camera:?}");
            assert_eq!(lines[0].matches("{key}").count(), 1, "{code}");
            assert_eq!(lines[1].matches("{follow}").count(), 1, "{code}");
            assert!(!lines[1].contains("{key}"), "{code}");
            for old in [
                "Lock camera · Y\nHold Space to follow",
                "Start · F11",
                "Team details · Tab",
                "Return control to AI · F12",
            ] {
                assert!(!table.contains_key(old), "{code}: stale {old:?}");
            }
        }
    }
}
