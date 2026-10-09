//! The mod's own text in the game's language. English source text is the
//! key: `tr("Cancel")` returns the current language's wording, or the
//! English when a translation is missing. `trf` fills named `{values}`.
//! Translations live in `probe/lang/<code>.json` (English text -> wording),
//! one file per game language; a test checks that every `tr`/`trf` text in
//! the source is translated in all of them, with the same `{values}`.
//!
//! The language follows Settings > Interface > Mod language: Auto (the
//! game's language, detected from the game's own word for "Close") or a
//! fixed choice. Log lines, setting keys and asset paths are never
//! translated. Game words (items, champions, skills) already come from the
//! game in its language.
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        OnceLock,
    },
};

/// The game's languages: code (as in the game's text files) and own name.
/// Index + 1 is the stored "Mod language" value (0 = Auto): never reorder.
pub const LANGS: [(&str, &str); 17] = [
    ("en", "English"),
    ("ko", "한국어"),
    ("ja", "日本語"),
    ("zh-hans", "简体中文"),
    ("zh-hant", "繁體中文"),
    ("de", "Deutsch"),
    ("fr", "Français"),
    ("es-ES", "Español"),
    ("pt-BR", "Português (Brasil)"),
    ("it", "Italiano"),
    ("nl", "Nederlands"),
    ("pl", "Polski"),
    ("ru", "Русский"),
    ("tr", "Türkçe"),
    ("vi", "Tiếng Việt"),
    ("th", "ไทย"),
    ("haw", "ʻŌlelo Hawaiʻi"),
];
/// The "Mod language" choices: Auto, then LANGS in order.
pub const CHOICES: [&str; 18] = [
    "Auto (game language)",
    LANGS[0].1,
    LANGS[1].1,
    LANGS[2].1,
    LANGS[3].1,
    LANGS[4].1,
    LANGS[5].1,
    LANGS[6].1,
    LANGS[7].1,
    LANGS[8].1,
    LANGS[9].1,
    LANGS[10].1,
    LANGS[11].1,
    LANGS[12].1,
    LANGS[13].1,
    LANGS[14].1,
    LANGS[15].1,
    LANGS[16].1,
];
/// The game's own text for "Close" (`asset/base/text/ui`, `common.close`)
/// in each language, in LANGS order; all seventeen differ.
pub const GAME_CLOSE: [&str; 17] = [
    "Close",
    "닫기",
    "閉じる",
    "关闭",
    "關閉",
    "Schließen",
    "Fermer",
    "Cerrar",
    "Fechar",
    "Chiudi",
    "Sluiten",
    "Zamknij",
    "Закрыть",
    "Kapat",
    "Đóng",
    "ปิด",
    "Pani",
];
/// The game text whose wording identifies the language.
pub const PROBE_TEXT: &str = "#asset/base/text/ui?common.close";

const FILES: [&str; 17] = [
    "{}",
    include_str!("../lang/ko.json"),
    include_str!("../lang/ja.json"),
    include_str!("../lang/zh-hans.json"),
    include_str!("../lang/zh-hant.json"),
    include_str!("../lang/de.json"),
    include_str!("../lang/fr.json"),
    include_str!("../lang/es-ES.json"),
    include_str!("../lang/pt-BR.json"),
    include_str!("../lang/it.json"),
    include_str!("../lang/nl.json"),
    include_str!("../lang/pl.json"),
    include_str!("../lang/ru.json"),
    include_str!("../lang/tr.json"),
    include_str!("../lang/vi.json"),
    include_str!("../lang/th.json"),
    include_str!("../lang/haw.json"),
];

/// Current language (LANGS index).
static CURRENT: AtomicUsize = AtomicUsize::new(0);
/// The game's language as last detected (LANGS index), usize::MAX if not yet.
static DETECTED: AtomicUsize = AtomicUsize::new(usize::MAX);

fn table(index: usize) -> &'static HashMap<String, String> {
    static TABLES: [OnceLock<HashMap<String, String>>; 17] = [const { OnceLock::new() }; 17];
    TABLES[index].get_or_init(|| serde_json::from_str(FILES[index]).unwrap_or_default())
}
/// `english` in the current language.
pub fn tr(english: &'static str) -> &'static str {
    match CURRENT.load(Ordering::Relaxed) {
        0 => english,
        i => table(i).get(english).map_or(english, String::as_str),
    }
}
/// `english` in the current language with each `{name}` replaced.
#[allow(dead_code)] // Used as the mod's text moves into the tables.
pub fn trf(english: &'static str, values: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut text = tr(english).to_owned();
    for (name, value) in values {
        text = text.replace(&format!("{{{name}}}"), &value.to_string());
    }
    text
}
/// Current language code, e.g. "zh-hant".
pub fn code() -> &'static str {
    LANGS[CURRENT.load(Ordering::Relaxed)].0
}
/// A number that changes with the language (windows rebuild on change).
pub fn generation() -> usize {
    CURRENT.load(Ordering::Relaxed)
}
/// The LANGS index of the game's word for "Close", if it is one of them.
pub fn identify(displayed: &str) -> Option<usize> {
    GAME_CLOSE.iter().position(|w| *w == displayed.trim())
}
/// Record the detected game language (client, from the probe label).
pub fn detected(index: Option<usize>) {
    DETECTED.store(index.unwrap_or(usize::MAX), Ordering::Relaxed);
}
/// Once per frame: apply the "Mod language" setting (0 = Auto). Returns the
/// language index when it changed.
pub fn sync() -> Option<usize> {
    let choice = crate::settings::current().number("mod_language") as usize;
    let want = if (1..=LANGS.len()).contains(&choice) {
        choice - 1
    } else {
        Some(DETECTED.load(Ordering::Relaxed))
            .filter(|i| *i < LANGS.len())
            .unwrap_or(0)
    };
    (CURRENT.swap(want, Ordering::Relaxed) != want).then_some(want)
}

/// Client: the game's own word for "Close" (through the SDK's text lookup,
/// as the tooltips use) identifies its language. Checked once a second, so
/// a change in the game's options is followed.
#[derive(Default)]
pub struct Probe {
    checked: Option<std::time::Instant>,
    logged: Option<String>,
}
impl Probe {
    pub fn update(&mut self, ctx: &mut mod_api_stable::StableClient<'_>, log: &crate::Logger) {
        if self
            .checked
            .is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(1))
        {
            return;
        }
        self.checked = Some(std::time::Instant::now());
        let word = crate::tooltips::translated(ctx, PROBE_TEXT).unwrap_or_default();
        let found = identify(&word);
        detected(found);
        if self.logged.as_deref() != Some(word.as_str()) {
            log.write(&format!(
                "LANGUAGE game word for Close={word:?} detected={:?}",
                found.map(|i| LANGS[i].0)
            ));
            self.logged = Some(word);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `tr("…")` / `trf("…"` literal in the source.
    fn source_texts() -> Vec<String> {
        let mut out = Vec::new();
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut stack = vec![src];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("rs")
                    || path.ends_with("lang.rs")
                {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap();
                for marker in ["tr(", "trf("] {
                    let mut rest = text.as_str();
                    while let Some(at) = rest.find(marker) {
                        let start = at + marker.len();
                        // Whole calls only (not e.g. `from_str(`), whose
                        // first argument is a literal (possibly on the next line).
                        let before = rest[..at].chars().next_back();
                        let after = rest[start..].trim_start();
                        if before.is_some_and(|c| c.is_alphanumeric() || c == '_')
                            || !after.starts_with('"')
                        {
                            rest = &rest[start..];
                            continue;
                        }
                        // Rust string literal up to the closing quote.
                        let body = &after[1..];
                        let mut end = 0;
                        let bytes = body.as_bytes();
                        while end < bytes.len() && bytes[end] != b'"' {
                            end += if bytes[end] == b'\\' { 2 } else { 1 };
                        }
                        let literal = body[..end]
                            .replace("\\n", "\n")
                            .replace("\\\"", "\"")
                            .replace("\\\\", "\\");
                        out.push(literal);
                        rest = &body[end..];
                    }
                }
            }
        }
        // Texts translated where they are drawn rather than at a literal.
        for (title, hint, _) in crate::settings_ui::PAGES {
            out.extend([title.to_owned(), hint.to_owned()]);
        }
        out.extend(crate::settings_ui::ADVANCED.map(str::to_owned));
        out.extend(crate::emote_library::SLOT_NAMES.map(str::to_owned));
        out.extend(["Off", "On"].map(str::to_owned));
        out.extend(crate::shop_ui::STAT_FILTERS.map(|f| f.0.to_owned()));
        out.extend(
            [
                crate::settings_ui::HINT_IDLE,
                crate::settings_ui::HINT_LISTEN,
            ]
            .map(str::to_owned),
        );
        for def in crate::settings::OPTIONS {
            out.extend([def.section, def.label, def.hint].map(str::to_owned));
            if let crate::settings::Control::Choice(choices) = def.control {
                // Language names stay in their own language.
                let own = def.key == "mod_language";
                out.extend(
                    choices
                        .iter()
                        .take(if own { 1 } else { choices.len() })
                        .map(|c| c.to_string()),
                );
            }
        }
        for def in crate::settings::BINDINGS {
            out.extend([def.group, def.label].map(str::to_owned));
        }
        out.retain(|t| !t.is_empty());
        out.sort();
        out.dedup();
        out
    }
    /// Developer aid: `cargo test --release --offline lang::tests::list_missing
    /// -- --ignored` writes the texts not yet in zh-hant.json (every file
    /// gets the same set) to target/lang-missing.json.
    #[test]
    #[ignore]
    fn list_missing() {
        let t: HashMap<String, String> = serde_json::from_str(FILES[4]).unwrap();
        let missing: Vec<String> = source_texts()
            .into_iter()
            .filter(|text| !t.contains_key(text))
            .collect();
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/lang-missing.json");
        std::fs::write(&path, serde_json::to_string_pretty(&missing).unwrap()).unwrap();
        eprintln!("{} missing texts -> {}", missing.len(), path.display());
    }
    fn placeholders(text: &str) -> Vec<String> {
        let mut out: Vec<String> = text
            .split('{')
            .skip(1)
            .filter_map(|s| s.split_once('}').map(|(name, _)| name.to_owned()))
            .collect();
        out.sort();
        out
    }
    #[test]
    fn every_text_is_translated_in_every_language_with_the_same_values() {
        let texts = source_texts();
        assert!(!texts.is_empty());
        for (i, (code, _)) in LANGS.iter().enumerate().skip(1) {
            let t: HashMap<String, String> =
                serde_json::from_str(FILES[i]).unwrap_or_else(|e| panic!("{code}: {e}"));
            for text in &texts {
                let wording = t
                    .get(text)
                    .unwrap_or_else(|| panic!("{code}: missing {text:?}"));
                assert!(!wording.trim().is_empty(), "{code}: empty {text:?}");
                assert_eq!(
                    placeholders(text),
                    placeholders(wording),
                    "{code}: {text:?}"
                );
            }
            for key in t.keys() {
                assert!(texts.contains(key), "{code}: stale {key:?}");
            }
        }
    }
    #[test]
    fn the_games_close_word_identifies_each_language() {
        for (i, word) in GAME_CLOSE.iter().enumerate() {
            assert_eq!(identify(word), Some(i));
        }
        assert_eq!(identify("#asset/base/text/ui?common.close"), None);
        assert_eq!(CHOICES[5], "繁體中文");
        assert_eq!(trf("Steps: {n}", &[("n", &5)]), "Steps: 5",);
    }
}
