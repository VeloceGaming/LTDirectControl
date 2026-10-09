//! A bounded, static edge tint, drawn below HUD and excluded from the minimap.
use crate::{
    camera::{CameraControl, Rect},
    player_hud::Snapshot,
    Logger,
};
use mod_api_stable::StableClient;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
const PATH: &str = "ingame.lt_screen_effect";
const BANDS: usize = 196;
#[derive(Default)]
pub struct Effect {
    strength: f32,
    dead: bool,
    cache: HashMap<String, String>,
    attempted: Option<Instant>,
    reported: Option<u8>,
    failed: bool,
}
impl Effect {
    pub fn update(&mut self, enabled: bool, active: bool, snapshot: Option<&Snapshot>, dt: u64) {
        self.dead = active && snapshot.is_some_and(|s| !s.alive);
        let target = if enabled && active && snapshot.is_some_and(|s| s.alive) {
            snapshot
                .filter(|s| s.alive)
                .and_then(|s| s.hp)
                .filter(|(_, max)| *max > 0)
                .map_or(0., |(hp, max)| severity(hp, max))
        } else {
            self.strength = 0.;
            0.
        };
        // No oscillation or flashing; render rate does not change the transition.
        self.strength +=
            (target - self.strength) * (1. - (-(dt.min(100_000) as f32) / 180_000.).exp());
        if self.strength < 0.001 {
            self.strength = 0.;
        }
    }
    pub fn apply(&mut self, ctx: &mut StableClient<'_>, camera: &CameraControl, log: &Logger) {
        let mode = if self.dead {
            2
        } else if self.strength > 0. {
            1
        } else {
            0
        };
        let frame = camera.frame();
        if mode == 0 || frame.is_none() {
            if ctx.ui_exists(PATH) {
                ctx.ui_set_visible(PATH, false);
            }
            if self.reported != Some(0) {
                log.write("SCREEN EFFECT inactive");
                self.reported = Some(0);
            }
            return;
        }
        if !ctx.ui_exists(PATH) {
            if self
                .attempted
                .is_some_and(|at| at.elapsed() < Duration::from_secs(2))
            {
                return;
            }
            self.attempted = Some(Instant::now());
            let mut source = String::from(
                "lt_screen_effect:empty { width: 100%; height: 100%; anchor_x: 0; anchor_y: 0; pivot_x: 0; pivot_y: 0; z: 990; ignore_event: true;\n",
            );
            for i in 0..BANDS {
                source.push_str(&format!(
                    "#band{i}:color {{ z: 990; visible: false; ignore_event: true; }}\n"
                ));
            }
            source.push('}');
            let ok = ctx.ui_spawn_source("ingame", &crate::ui_theme::themed(&source));
            log.write(&format!(
                "SCREEN EFFECT native UI spawn={ok} exists={}",
                ctx.ui_exists(PATH)
            ));
            if !ok || !ctx.ui_exists(PATH) {
                return;
            }
            self.cache.clear();
        }
        ctx.ui_set_visible(PATH, true);
        let frame = frame.unwrap();
        let v = frame.viewport;
        let m = frame.minimap;
        let excluded = Rect {
            x: m.x - 4.,
            y: m.y - 4.,
            w: m.w + 8.,
            h: m.h + 8.,
        };
        let mut pieces = Vec::new();
        if self.dead {
            pieces.extend(
                subtract(v, excluded)
                    .into_iter()
                    .map(|r| (r, 0x00000066u32)),
            );
        } else {
            let depth = 128f32.min(v.w / 4.).min(v.h / 4.);
            for i in 0..12 {
                let d = depth * i as f32 / 12.;
                let step = depth / 12.;
                let alpha = (112. * self.strength * (1. - i as f32 / 12.).powi(2)).round() as u32;
                if alpha == 0 {
                    continue;
                }
                for band in [
                    Rect {
                        x: v.x + d,
                        y: v.y + d,
                        w: v.w - 2. * d,
                        h: step,
                    },
                    Rect {
                        x: v.x + d,
                        y: v.y + v.h - d - step,
                        w: v.w - 2. * d,
                        h: step,
                    },
                    Rect {
                        x: v.x + d,
                        y: v.y + d + step,
                        w: step,
                        h: v.h - 2. * (d + step),
                    },
                    Rect {
                        x: v.x + v.w - d - step,
                        y: v.y + d + step,
                        w: step,
                        h: v.h - 2. * (d + step),
                    },
                ] {
                    pieces.extend(
                        subtract(band, excluded)
                            .into_iter()
                            .map(|r| (r, 0x9e253600u32 | alpha)),
                    );
                }
            }
        }
        let (ox, oy, _, _) = ctx.ui_node_rect("ingame").unwrap_or((0., 0., 1920., 1080.));
        for i in 0..BANDS {
            let props = pieces.get(i).map_or_else(|| "visible: false;".into(), |(r, color)| {
                format!("visible: true; x: {:.2}px; y: {:.2}px; width: {:.2}px; height: {:.2}px; color: #{color:08x};", r.x-ox, r.y-oy, r.w, r.h)
            });
            if !crate::hud_motion::properties(
                ctx,
                &mut self.cache,
                &format!("{PATH}.band{i}"),
                &props,
            ) && !self.failed
            {
                log.write(&format!("SCREEN EFFECT property failed band={i}"));
                self.failed = true;
            }
        }
        if self.reported != Some(mode) {
            log.write(&format!(
                "SCREEN EFFECT native UI mode={mode} pieces={} strength={:.3} viewport={v:?}",
                pieces.len(),
                self.strength
            ));
            self.reported = Some(mode);
        }
    }
}
fn severity(hp: usize, max: usize) -> f32 {
    if max == 0 {
        return 0.;
    }
    let ratio = hp as f32 / max as f32;
    if ratio >= 0.25 {
        0.
    } else {
        0.35 + 0.65 * ((0.25 - ratio) / 0.25).clamp(0., 1.)
    }
}
fn subtract(r: Rect, cut: Rect) -> Vec<Rect> {
    let x = r.x.max(cut.x);
    let y = r.y.max(cut.y);
    let right = (r.x + r.w).min(cut.x + cut.w);
    let bottom = (r.y + r.h).min(cut.y + cut.h);
    if x >= right || y >= bottom {
        return vec![r];
    }
    [
        Rect {
            x: r.x,
            y: r.y,
            w: r.w,
            h: y - r.y,
        },
        Rect {
            x: r.x,
            y: bottom,
            w: r.w,
            h: r.y + r.h - bottom,
        },
        Rect {
            x: r.x,
            y,
            w: x - r.x,
            h: bottom - y,
        },
        Rect {
            x: right,
            y,
            w: r.x + r.w - right,
            h: bottom - y,
        },
    ]
    .into_iter()
    .filter(|r| r.w > 0. && r.h > 0.)
    .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn warning_is_visible_below_threshold_without_flashing_at_healthy_hp() {
        assert_eq!(severity(250, 1000), 0.);
        assert_eq!(severity(1000, 1000), 0.);
        assert!((112. * severity(200, 1000)).round() >= 50.);
        assert_eq!(severity(0, 1000), 1.);
        assert_eq!(severity(0, 0), 0.);
    }
    #[test]
    fn map_exclusion_removes_exact_intersection_without_covering_other_pixels() {
        let r = Rect {
            x: 0.,
            y: 0.,
            w: 1920.,
            h: 1080.,
        };
        let cut = Rect {
            x: 1560.,
            y: 720.,
            w: 360.,
            h: 360.,
        };
        let pieces = subtract(r, cut);
        assert_eq!(
            pieces.iter().map(|r| r.w * r.h).sum::<f32>(),
            1920. * 1080. - 360. * 360.
        );
        for p in [(1561., 721.), (1919., 1079.)] {
            assert!(!pieces.iter().any(|r| r.contains(p)));
        }
        assert!(pieces.iter().any(|r| r.contains((1559., 1079.))));
    }
    #[test]
    fn disabled_or_released_effect_clears_without_leaking_into_other_scenes() {
        let mut effect = Effect {
            strength: 1.,
            dead: false,
            ..Default::default()
        };
        effect.update(false, true, None, 16_667);
        assert_eq!(effect.strength, 0.);
        effect.strength = 1.;
        effect.update(true, false, None, 16_667);
        assert_eq!(effect.strength, 0.);
        effect.strength = 1.;
        effect.update(true, true, None, 16_667);
        assert_eq!(effect.strength, 0.);
    }
}
