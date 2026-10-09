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
pub fn unchanged(cache: &HashMap<String, String>, path: &str, source: &str) -> bool {
    fields(source).iter().all(|(key, value)| {
        cache
            .get(&format!("field:{path}:{key}"))
            .is_some_and(|old| old == value)
    })
}
pub fn properties(
    ctx: &mut mod_api_stable::StableClient<'_>,
    cache: &mut HashMap<String, String>,
    path: &str,
    source: &str,
) -> bool {
    // Design greys (`#~rrggbbaa`) follow the background colour.
    let source = crate::ui_theme::themed(source);
    write_properties(cache, path, &source, |value| {
        ctx.ui_set_properties(path, value)
    })
}
fn write_properties(
    cache: &mut HashMap<String, String>,
    path: &str,
    source: &str,
    mut write: impl FnMut(&str) -> bool,
) -> bool {
    if unchanged(cache, path, source) {
        return true;
    }
    if !write(source) {
        return false;
    }
    for (key, value) in fields(source) {
        cache.insert(format!("field:{path}:{key}"), value.into());
    }
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
