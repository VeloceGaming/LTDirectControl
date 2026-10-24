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
        self.at(key, target, seconds, crate::clock::now())
    }
    pub fn tonal(&mut self, key: &str, target: f32, seconds: f32) -> f32 {
        self.curve_at(key, target, seconds, true, crate::clock::now())
    }
    fn at(&mut self, key: &str, target: f32, seconds: f32, now: Instant) -> f32 {
        self.curve_at(key, target, seconds, false, now)
    }
    fn curve_at(&mut self, key: &str, target: f32, seconds: f32, tonal: bool, now: Instant) -> f32 {
        // A new key only when the tween is new (not one per call).
        if !self.0.contains_key(key) {
            self.0.insert(
                key.into(),
                Tween {
                    from: 0.,
                    target: 0.,
                    at: now,
                    seconds,
                    tonal,
                },
            );
        }
        let tween = self.0.get_mut(key).expect("inserted above");
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
        if self.0.contains_key(key) {
            return;
        }
        let now = crate::clock::now();
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
    format!("{:08x}", color_rgba(from, to, t))
}
/// [`color`] as 0xRRGGBBAA (the same channels, before formatting).
pub fn color_rgba(from: u32, to: u32, t: f32) -> u32 {
    let t = t.clamp(0., 1.);
    (0..4).rev().fold(0, |out, n| {
        let a = ((from >> (n * 8)) & 255) as f32;
        let b = ((to >> (n * 8)) & 255) as f32;
        out << 8 | u32::from((a + (b - a) * t).round() as u8)
    })
}
/// One property write through a window's cache (`ui_writes.rs` has the
/// rules): true once the node holds `source`, written now or before.
pub fn properties(
    ctx: &mut mod_api_stable::StableClient<'_>,
    cache: &mut crate::ui_writes::PropCache,
    path: &str,
    source: &str,
) -> bool {
    cache.props_at(ctx, path, source)
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
    fn colour_mix_formats_as_1_5_3_did() {
        let old = |from: u32, to: u32, t: f32| -> String {
            let t = t.clamp(0., 1.);
            (0..4)
                .rev()
                .map(|n| {
                    let a = ((from >> (n * 8)) & 255) as f32;
                    let b = ((to >> (n * 8)) & 255) as f32;
                    format!("{:02x}", (a + (b - a) * t).round() as u8)
                })
                .collect()
        };
        for (from, to) in [
            (0x3a383700, 0x3a3837ff),
            (0xffffffff, 0xfdee00ff),
            (0, 0xffffffff),
        ] {
            for k in -2..=22 {
                let t = k as f32 / 20.;
                assert_eq!(color(from, to, t), old(from, to, t));
            }
        }
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
        let mut fields = Vec::new();
        crate::ui_writes::for_each_field(
            "source: \"asset/a;b\"; btn: { color: #fff; stroke: 1; }",
            |k, v| fields.push((k, v)),
        );
        assert_eq!(
            fields,
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
