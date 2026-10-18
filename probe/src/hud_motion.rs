//! Presentation-only interpolation. Native hit rectangles stay at rest.
use std::{collections::HashMap, time::Instant};

#[derive(Clone, Copy)]
struct Tween {
    from: f32,
    target: f32,
    at: Instant,
    seconds: f32,
    tonal: bool,
}
impl Tween {
    fn value(self, now: Instant) -> f32 {
        let t = (now.duration_since(self.at).as_secs_f32() / self.seconds).clamp(0., 1.);
        let eased = easing(t, self.tonal);
        self.from + (self.target - self.from) * eased
    }
}
/// Solve the source's two cubic-bezier curves rather than treating time as t.
pub fn easing(time: f32, tonal: bool) -> f32 {
    let (x1, y1, x2, y2) = if tonal {
        (0.4, 0.1, 0.5, 1.)
    } else {
        (0.15, 0.8, 0.3, 1.)
    };
    let cubic = |t: f32, a: f32, b: f32| {
        3. * (1. - t).powi(2) * t * a + 3. * (1. - t) * t.powi(2) * b + t.powi(3)
    };
    let time = time.clamp(0., 1.);
    if time == 0. || time == 1. {
        return time;
    }
    let (mut lo, mut hi) = (0., 1.);
    for _ in 0..14 {
        let t = (lo + hi) / 2.;
        if cubic(t, x1, x2) < time {
            lo = t;
        } else {
            hi = t;
        }
    }
    cubic((lo + hi) / 2., y1, y2)
}
#[derive(Default)]
pub struct Motion(HashMap<String, Tween>);
impl Motion {
    pub fn value(&mut self, key: &str, target: f32, seconds: f32) -> f32 {
        self.at(key, target, seconds, Instant::now())
    }
    pub fn tonal(&mut self, key: &str, target: f32, seconds: f32) -> f32 {
        self.curve_at(key, target, seconds, true, Instant::now())
    }
    fn at(&mut self, key: &str, target: f32, seconds: f32, now: Instant) -> f32 {
        self.curve_at(key, target, seconds, false, now)
    }
    fn curve_at(&mut self, key: &str, target: f32, seconds: f32, tonal: bool, now: Instant) -> f32 {
        let tween = self.0.entry(key.into()).or_insert(Tween {
            from: 0.,
            target: 0.,
            at: now,
            seconds,
            tonal,
        });
        if tween.target != target {
            *tween = Tween {
                from: tween.value(now),
                target,
                at: now,
                seconds,
                tonal,
            };
        }
        tween.value(now)
    }
    // motion: pass
    /// Start `key` at rest on `value` (a new key otherwise tweens from 0).
    /// Does nothing once the key exists.
    pub fn seed(&mut self, key: &str, value: f32) {
        let now = Instant::now();
        self.0.entry(key.into()).or_insert(Tween {
            from: value,
            target: value,
            at: now,
            seconds: 1.,
            tonal: false,
        });
    }
    // motion: pass
    /// Forget one key: it starts afresh (or from its next seed) next time.
    pub fn remove(&mut self, key: &str) {
        self.0.remove(key);
    }
    pub fn reset(&mut self) {
        self.0.clear();
    }
}
pub fn color(from: u32, to: u32, t: f32) -> String {
    let t = t.clamp(0., 1.);
    (0..4)
        .rev()
        .map(|n| {
            let a = ((from >> (n * 8)) & 255) as f32;
            let b = ((to >> (n * 8)) & 255) as f32;
            format!("{:02x}", (a + (b - a) * t).round() as u8)
        })
        .collect()
}
// Compare top-level properties separately: changing geometry must not force
// unchanged visibility/source setters to run again every frame.
fn fields(source: &str) -> Vec<(&str, &str)> {
    let mut result = Vec::new();
    let (mut start, mut depth, mut quoted, mut escape) = (0, 0, false, false);
    for (i, c) in source.char_indices() {
        if quoted {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                quoted = false;
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            '{' => depth += 1,
            '}' => depth -= 1,
            ';' if depth == 0 => {
                if let Some((key, value)) = source[start..i].split_once(':') {
                    result.push((key.trim(), value.trim()));
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    if let Some((key, value)) = source[start..].split_once(':') {
        result.push((key.trim(), value.trim()));
    }
    result
}
// Fast path: most frames send a node the very text it got last frame. Two
// whole-text slots per path answer that with one lookup, before any parsing:
// `whole:` holds the text last written, `whole2:` a text the field check
// found already applied since that write (several nodes get a visibility
// text and a colour text each frame). Each holds the source as given, before
// theming, behind the background it was themed with ("rrggbb"), so a
// background change falls through to the field check. Both keys contain the
// path, so `cache.clear()` and path-scoped `retain` drop them with the
// fields.
const WHOLE: &str = "whole:";
const WHOLE2: &str = "whole2:";
thread_local! {
    static KEY: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}
fn slot_holds(cache: &HashMap<String, String>, tag: &str, path: &str, source: &str) -> bool {
    let stored = KEY.with(|key| {
        let mut key = key.borrow_mut();
        key.clear();
        key.push_str(tag);
        key.push_str(path);
        cache.get(key.as_str())
    });
    stored.is_some_and(|stored| {
        stored.len() == 6 + source.len()
            && stored.get(6..) == Some(source)
            && stored
                .get(..6)
                .and_then(|bg| u32::from_str_radix(bg, 16).ok())
                == Some(crate::ui_theme::background())
    })
}
fn slot_value(source: &str) -> String {
    format!("{:06x}{source}", crate::ui_theme::background())
}
fn whole_unchanged(cache: &HashMap<String, String>, path: &str, source: &str) -> bool {
    slot_holds(cache, WHOLE, path, source) || slot_holds(cache, WHOLE2, path, source)
}
fn fields_unchanged(cache: &HashMap<String, String>, path: &str, source: &str) -> bool {
    fields(source).iter().all(|(key, value)| {
        cache
            .get(&format!("field:{path}:{key}"))
            .is_some_and(|old| old == value)
    })
}
pub fn unchanged(cache: &HashMap<String, String>, path: &str, source: &str) -> bool {
    whole_unchanged(cache, path, source) || fields_unchanged(cache, path, source)
}
pub fn properties(
    ctx: &mut mod_api_stable::StableClient<'_>,
    cache: &mut HashMap<String, String>,
    path: &str,
    source: &str,
) -> bool {
    write_properties(cache, path, source, |value| {
        ctx.ui_set_properties(path, value)
    })
}
fn write_properties(
    cache: &mut HashMap<String, String>,
    path: &str,
    source: &str,
    mut write: impl FnMut(&str) -> bool,
) -> bool {
    // The same text as before needs neither theming nor parsing.
    if whole_unchanged(cache, path, source) {
        return true;
    }
    // Design greys (`#~rrggbbaa`) follow the background colour.
    let themed = crate::ui_theme::themed(source);
    // Compare top-level fields: a geometry change must not resend the
    // unchanged visibility or source setters.
    if fields_unchanged(cache, path, &themed) {
        cache.insert(format!("{WHOLE2}{path}"), slot_value(source));
        return true;
    }
    if !write(&themed) {
        return false;
    }
    for (key, value) in fields(&themed) {
        cache.insert(format!("field:{path}:{key}"), value.into());
    }
    cache.insert(format!("{WHOLE}{path}"), slot_value(source));
    cache.remove(&format!("{WHOLE2}{path}"));
    true
}
pub fn fade_rich(text: &str, t: f32) -> String {
    let mut s = text.to_owned();
    let mut at = 0;
    while let Some(relative) = s[at..].find("<#") {
        let start = at + relative;
        if s.as_bytes().get(start + 10) == Some(&b'>')
            && s.as_bytes()
                .get(start + 2..start + 10)
                .is_some_and(|b| b.iter().all(u8::is_ascii_hexdigit))
        {
            if let Ok(alpha) = u8::from_str_radix(&s[start + 8..start + 10], 16) {
                s.replace_range(
                    start + 8..start + 10,
                    &format!("{:02x}", (alpha as f32 * t.clamp(0., 1.)).round() as u8),
                );
            }
        }
        at = start + 2;
    }
    s
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_panel_close_reopen_updates_host_and_cache_together() {
        let mut cache = HashMap::new();
        let mut host_visible = false;
        let mut writes = 0;
        for active in [true, true, false, false, true, false, true] {
            let source = format!("visible: {active};");
            assert!(write_properties(&mut cache, "panel", &source, |value| {
                host_visible = value.contains("visible: true;");
                writes += 1;
                true
            }));
            assert_eq!(host_visible, active);
        }
        assert_eq!(writes, 5);
        assert!(!write_properties(
            &mut cache,
            "panel",
            "visible: false;",
            |_| false
        ));
        assert!(host_visible);
        assert!(!unchanged(&cache, "panel", "visible: false;"));
    }
    #[test]
    fn equal_text_returns_before_parsing_and_writes_nothing() {
        let mut cache = HashMap::new();
        let mut writes = 0;
        let source = "visible: true; x: 4px;";
        assert!(write_properties(&mut cache, "p", source, |_| {
            writes += 1;
            true
        }));
        // Without the field keys only the whole-text slot can answer.
        cache.retain(|k, _| !k.starts_with("field:"));
        assert!(write_properties(&mut cache, "p", source, |_| {
            writes += 1;
            true
        }));
        assert!(unchanged(&cache, "p", source));
        assert_eq!(writes, 1);
        // A different text is written once, then is the equal text.
        for _ in 0..3 {
            assert!(write_properties(&mut cache, "p", "visible: false;", |_| {
                writes += 1;
                true
            }));
        }
        assert_eq!(writes, 2);
    }
    #[test]
    fn alternating_texts_hit_and_geometry_does_not_resend_visibility() {
        let mut cache = HashMap::new();
        let mut sent = Vec::new();
        for x in [1, 1, 1, 2, 2, 2] {
            for source in ["visible: true;".to_owned(), format!("x: {x}px;")] {
                assert!(write_properties(&mut cache, "p", &source, |value| {
                    sent.push(value.to_owned());
                    true
                }));
            }
        }
        assert_eq!(sent, ["visible: true;", "x: 1px;", "x: 2px;"]);
        // Fields changed together with geometry: only the text that differs.
        let mut sent = Vec::new();
        for source in ["visible: true; x: 2px;", "visible: true; x: 3px;"] {
            write_properties(&mut cache, "p", source, |value| {
                sent.push(value.to_owned());
                true
            });
        }
        assert_eq!(sent, ["visible: true; x: 3px;"]);
    }
    #[test]
    fn failed_write_retries_and_another_background_misses_the_slot() {
        let mut cache = HashMap::new();
        let mut writes = 0;
        for ok in [false, false, true, true] {
            write_properties(&mut cache, "p", "visible: true;", |_| {
                writes += 1;
                ok
            });
        }
        assert_eq!(writes, 3);
        // A slot written under another background is no answer: the field
        // check decides (and here finds the text applied).
        cache.clear();
        cache.insert("whole:p".into(), "000000visible: true;".into());
        assert!(!whole_unchanged(&cache, "p", "visible: true;"));
        assert!(!unchanged(&cache, "p", "visible: true;"));
        cache.insert("whole:p".into(), slot_value("visible: true;"));
        assert!(whole_unchanged(&cache, "p", "visible: true;"));
        assert!(!whole_unchanged(&cache, "p", "visible: true; "));
    }
    #[test]
    fn interrupted_hover_is_continuous_and_settles_without_drift() {
        let now = Instant::now();
        let mut m = Motion::default();
        m.at("hover", 1., 0.1, now);
        let half = now + std::time::Duration::from_millis(50);
        let v = m.at("hover", 1., 0.1, half);
        assert!(v > 0.5 && v < 1.);
        assert_eq!(m.at("hover", 0., 0.1, half), v);
        assert_eq!(
            m.at("hover", 0., 0.1, half + std::time::Duration::from_secs(1)),
            0.
        );
    }
    #[test]
    fn cache_distinguishes_geometry_from_sources_and_fade_keeps_rich_stat_types() {
        assert_eq!(
            fields("source: \"asset/a;b\"; btn: { color: #fff; stroke: 1; }"),
            vec![
                ("source", "\"asset/a;b\""),
                ("btn", "{ color: #fff; stroke: 1; }")
            ]
        );
        assert_eq!(
            fade_rich("<#ff9028ff>100%<> 中 <#a974ffff>60%<>", 0.5),
            "<#ff902880>100%<> 中 <#a974ff80>60%<>"
        );
        assert_eq!(fade_rich("<#中文測試>text", 0.5), "<#中文測試>text");
    }
}
