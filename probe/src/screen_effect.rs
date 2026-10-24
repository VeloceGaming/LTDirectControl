//! A bounded, static edge tint, drawn below HUD and excluded from the minimap.
use crate::{
    camera::{CameraControl, Rect},
    player_hud::Snapshot,
    Logger,
};
use mod_api_stable::StableClient;
use std::time::{Duration, Instant};
const PATH: &str = "ingame.lt_screen_effect";
pub(crate) fn template() -> String {
    let mut source = String::from(
        "lt_screen_effect:empty { width: 100%; height: 100%; anchor_x: 0; anchor_y: 0; pivot_x: 0; pivot_y: 0; z: 990; ignore_event: true;\n",
    );
    for i in 0..BANDS {
        source.push_str(&format!(
            "#band{i}:color {{ z: 990; visible: false; ignore_event: true; }}\n"
        ));
    }
    source.push('}');
    source
}
const BANDS: usize = 196;
/// The battlefield veil while your athlete is dead, League-like without a
/// greyscale filter (the author, 2026-10-18; the greyscale composite hook
/// was removed in 1.3.16): a cool slate tint over the battlefield (18 %;
/// 15 % black before 1.3.16, 40 % before 1.3.5) so the fight stays
/// watchable, and a vignette darkening the edges, faded in over
/// `DEATH_FADE`. The strip's own death veil is the motion pass's B6.
const DEATH_TINT: u32 = 0x141a2400;
const DEATH_TINT_ALPHA: f32 = 46.;
const DEATH_EDGE: u32 = 0x05080e00;
/// Extra darkness at the outermost edge band (on top of the tint).
const DEATH_EDGE_ALPHA: f32 = 112.;
const DEATH_FADE: Duration = Duration::from_millis(350);
#[derive(Default)]
pub struct Effect {
    strength: f32,
    dead: bool,
    /// 0 to 1 while dead, over `DEATH_FADE`.
    death: f32,
    cache: crate::ui_writes::PropCache,
    /// Each band's node in `cache` (1.6.0).
    bands: Vec<crate::ui_writes::NodeId>,
    attempted: Option<Instant>,
    reported: Option<u8>,
    failed: bool,
    // motion: pass
    /// The HUD motion pass (hud_motion_pass.rs).
    pass: crate::hud_motion_pass::Pass,
}
impl Effect {
    pub fn update(&mut self, enabled: bool, active: bool, snapshot: Option<&Snapshot>, dt: u64) {
        self.dead = active && snapshot.is_some_and(|s| !s.alive);
        self.death = death_fade(self.death, self.dead, dt);
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
        // No flashing; render rate does not change the transition. (The HUD
        // motion pass, S5, may breathe the drawn strength; see apply.) motion: S5
        self.strength +=
            (target - self.strength) * (1. - (-(dt.min(100_000) as f32) / 180_000.).exp());
        if self.strength < 0.001 {
            self.strength = 0.;
        }
    }
    pub fn apply(&mut self, ctx: &mut StableClient<'_>, camera: &CameraControl, log: &Logger) {
        // motion: pass
        self.pass.frame();
        // motion: S5 (restore `self.strength` in the bands below)
        let strength =
            crate::hud_motion_pass::s5_low_health_heartbeat(&mut self.pass, self.strength);
        let mode = if self.dead {
            2
        } else if self.strength > 0. {
            1
        } else {
            0
        };
        let frame = camera.frame();
        if mode != 0 && frame.is_some() && crate::ui_root::hidden() {
            crate::ui_root::hide(ctx, PATH);
            return;
        }
        if mode == 0 || frame.is_none() {
            // Write only on a change (this runs every frame).
            crate::ui_root::hide(ctx, PATH);
            if self.reported != Some(0) {
                log.write("SCREEN EFFECT inactive");
                self.reported = Some(0);
            }
            return;
        }
        if !ctx.ui_exists(PATH) {
            if self
                .attempted
                .is_some_and(|at| crate::clock::since(at) < Duration::from_secs(2))
            {
                return;
            }
            self.attempted = Some(crate::clock::now());
            if !self.spawn(ctx, log) {
                return;
            }
        }
        crate::ui_root::show(ctx, PATH);
        if self.bands.is_empty() {
            let cache = &mut self.cache;
            self.bands = (0..BANDS)
                .map(|i| cache.node(&format!("{PATH}.band{i}")))
                .collect();
        }
        let frame = frame.unwrap();
        let v = frame.viewport;
        let m = frame.minimap;
        let excluded = Rect {
            x: m.x - 4.,
            y: m.y - 4.,
            w: m.w + 8.,
            h: m.h + 8.,
        };
        let pieces = if self.dead {
            death_pieces(v, excluded, self.death)
        } else {
            edge_bands(v, excluded, 128., |i| {
                let alpha = (112. * strength * (1. - i as f32 / 12.).powi(2)).round() as u32;
                (alpha > 0).then_some(0x9e253600 | alpha)
            })
        };
        let (ox, oy, _, _) = ctx.ui_node_rect("ingame").unwrap_or((0., 0., 1920., 1080.));
        let pair = |a: f32, b: f32| u64::from(a.to_bits()) << 32 | u64::from(b.to_bits());
        let unpair = |v: u64| (f32::from_bits((v >> 32) as u32), f32::from_bits(v as u32));
        for i in 0..BANDS {
            let key = pieces.get(i).map_or((false, 0, 0, 0), |(r, color)| {
                (true, pair(r.x - ox, r.y - oy), pair(r.w, r.h), *color)
            });
            let written = self.cache.props_keyed(ctx, self.bands[i], 0, key, |(shown, xy, wh, color)| {
                if !shown {
                    return "visible: false;".into();
                }
                let ((x, y), (w, h)) = (unpair(xy), unpair(wh));
                format!("visible: true; x: {x:.2}px; y: {y:.2}px; width: {w:.2}px; height: {h:.2}px; color: #{color:08x};")
            });
            if !written && !self.failed {
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
/// The death veil's fade after `dt` µs: rising to 1 over `DEATH_FADE`
/// while dead, back to 0 at once on revival.
fn death_fade(fade: f32, dead: bool, dt: u64) -> f32 {
    if dead {
        (fade + dt as f32 / DEATH_FADE.as_micros() as f32).min(1.)
    } else {
        0.
    }
}
/// The death veil at `fade` (0 to 1): the tint over the viewport, then
/// the vignette bands, all outside `excluded` (the minimap).
fn death_pieces(v: Rect, excluded: Rect, fade: f32) -> Vec<(Rect, u32)> {
    let tint = (DEATH_TINT_ALPHA * fade).round() as u32;
    let mut pieces: Vec<(Rect, u32)> = subtract(v, excluded)
        .into_iter()
        .map(|r| (r, DEATH_TINT | tint))
        .collect();
    pieces.extend(edge_bands(v, excluded, 240., |i| {
        let alpha = (DEATH_EDGE_ALPHA * fade * (1. - i as f32 / 12.).powi(2)).round() as u32;
        (alpha > 0).then_some(DEATH_EDGE | alpha)
    }));
    pieces
}
/// Twelve nested frames along the viewport's edges, `depth` px deep at
/// most (a quarter of the viewport), outermost first; `color(i)` is ring
/// i's colour, None to skip it.
fn edge_bands(
    v: Rect,
    excluded: Rect,
    depth: f32,
    color: impl Fn(usize) -> Option<u32>,
) -> Vec<(Rect, u32)> {
    let depth = depth.min(v.w / 4.).min(v.h / 4.);
    let step = depth / 12.;
    let mut pieces = Vec::new();
    for i in 0..12 {
        let Some(color) = color(i) else { continue };
        let d = depth * i as f32 / 12.;
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
            pieces.extend(subtract(band, excluded).into_iter().map(|r| (r, color)));
        }
    }
    pieces
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
impl Effect {
    fn spawn(&mut self, ctx: &mut StableClient<'_>, log: &Logger) -> bool {
        let ok = ctx.ui_spawn_source("ingame", &crate::ui_theme::themed(&template()));
        log.write(&format!(
            "SCREEN EFFECT native UI spawn={ok} exists={}",
            ctx.ui_exists(PATH)
        ));
        if !ok || !ctx.ui_exists(PATH) {
            return false;
        }
        self.cache.clear();
        true
    }
}
impl crate::prewarm::Prespawn for Effect {
    fn built(&self, ctx: &StableClient<'_>) -> bool {
        ctx.ui_exists(PATH)
    }
    /// Its bands are hidden; the root is hidden too, as when inactive.
    fn prespawn(&mut self, ctx: &mut StableClient<'_>, log: &Logger) -> bool {
        let ok = self.spawn(ctx, log);
        if ok {
            ctx.ui_set_visible(PATH, false);
        }
        ok
    }
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
    fn the_death_veil_tints_darkens_the_edges_fades_in_and_spares_the_minimap() {
        let v = Rect {
            x: 0.,
            y: 0.,
            w: 1920.,
            h: 1080.,
        };
        let cut = Rect {
            x: 1556.,
            y: 716.,
            w: 368.,
            h: 368.,
        };
        let pieces = death_pieces(v, cut, 1.);
        assert!(pieces.len() <= BANDS);
        let alpha_at = |p: (f32, f32)| -> u32 {
            pieces
                .iter()
                .filter(|(r, _)| r.contains(p))
                .map(|(_, c)| c & 0xff)
                .sum()
        };
        // The centre gets the tint only; the outermost edge the most.
        assert_eq!(alpha_at((960., 540.)), 46);
        assert_eq!(alpha_at((2., 540.)), 46 + 112);
        assert!(alpha_at((100., 540.)) < alpha_at((2., 540.)));
        assert_eq!(alpha_at((1700., 900.)), 0);
        // Fades in from nothing over DEATH_FADE, and resets on revival.
        assert!((death_fade(0., true, 175_000) - 0.5).abs() < 0.01);
        assert_eq!(death_fade(0.5, true, 400_000), 1.);
        assert_eq!(death_fade(1., false, 16_667), 0.);
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
