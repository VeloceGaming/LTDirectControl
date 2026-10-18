//! Ground rings: the Unit highlight that needs no game hook (Settings,
//! Unit highlight: Ground ring, and the stand-in while another mod holds the
//! unit renderer). Drawn into the match world ("Game" map) at z -1, which
//! sits above the ground and below every unit, so a unit's sprite covers
//! the part of its ring behind it (found 2026-10-19 with a test tool,
//! 1.3.20-1.3.22: z <= -100 is under the ground, z 0 over the units).
//!
//! Design (the author's proposal page, 2026-10-19): a soft ground glow, a
//! crisp rim with a halo in the team colour; the attack target adds a
//! hairline ring with four inward chevrons turning slowly; an attack-click
//! flies the chevrons in from wide while the rim flashes and settles.
use crate::{camera::CameraFrame, combat::Unit};
use mod_api_stable::StableClient;
use std::time::Instant;

/// The match world's map is 2048 units; after the frame its centre is the
/// camera centre and one map unit is one world unit.
const MAP: f32 = 2048.;
/// Above the ground (z <= -100) and below the units (z 0).
const Z: i32 = -1;
const ENEMY: u32 = 0xff4d5e;
const ALLY: u32 = 0x4fa9ff;
const FLASH: u32 = 0xfff4f4;
const FADE_IN: f32 = 0.09;
const FADE_OUT: f32 = 0.12;
const LOCK_FLY: f32 = 0.20;
const LOCK_FLASH: f32 = 0.26;
/// Chevron turn rate, radians per second (14 degrees).
const TURN: f32 = 14. * std::f32::consts::PI / 180.;
const RIM_SEGMENTS: usize = 40;
const GLOW_SEGMENTS: usize = 32;
const GLOW_BANDS: usize = 6;

/// One stroke on the map: from, to, width (map units), 0xRRGGBBAA.
pub type Line = ((f32, f32), (f32, f32), f32, u32);

/// How one unit's ring looks this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub rgb: u32,
    pub alpha: f32,
    /// Size factor while fading in (0.92 -> 1).
    pub scale: f32,
    pub target: bool,
    /// Extra halo while the target is also hovered.
    pub halo: f32,
    /// Chevron distance factor during the lock-on fly-in (1.9 -> 1).
    pub spread: f32,
    /// Rim toward white during the lock-on (0..1).
    pub flash: f32,
    pub turn: f32,
}

/// Fades, the lock-on and the turning chevrons across frames.
pub struct GroundRings {
    started: Instant,
    last: Option<Instant>,
    /// Hovered units fading in or out: (unit, fade 0..1, hovered now).
    fades: Vec<(Unit, f32, bool)>,
    /// The attack-click being animated: (unit id, click time).
    lock: Option<(usize, Instant)>,
    /// The rings of the last running frame, kept while paused.
    shown: Vec<(Unit, Look)>,
}
impl Default for GroundRings {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            last: None,
            fades: Vec::new(),
            lock: None,
            shown: Vec::new(),
        }
    }
}
impl GroundRings {
    /// While paused: the rings of the last running frame, unchanged (the
    /// game stops hover and target updates during a pause).
    pub fn paused(&mut self) -> Vec<(Unit, Look)> {
        self.last = None;
        self.shown.clone()
    }
    /// Out of the match: nothing kept for a later pause.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    /// Advance by the time since the last call and return each ring to draw:
    /// the attack target (hostile only) and hovered units fading in or out.
    pub fn update(
        &mut self,
        hover: Option<Unit>,
        attack: Option<Unit>,
        click: Option<(usize, Instant)>,
    ) -> Vec<(Unit, Look)> {
        let now = Instant::now();
        let dt = self
            .last
            .map_or(0., |t| now.duration_since(t).as_secs_f32().min(0.1));
        self.last = Some(now);
        let target = attack.filter(|u| !u.friendly);
        if let Some((id, at)) = click.filter(|(id, _)| target.is_some_and(|t| t.id == *id)) {
            if self.lock.is_none_or(|(_, seen)| seen != at) {
                self.lock = Some((id, at));
            }
        }
        let lock_age = self
            .lock
            .filter(|(id, _)| target.is_some_and(|t| t.id == *id))
            .map(|(_, at)| now.saturating_duration_since(at).as_secs_f32());
        for f in &mut self.fades {
            f.2 = hover.is_some_and(|h| h.id == f.0.id);
            if let Some(h) = hover.filter(|h| h.id == f.0.id) {
                f.0 = h;
            }
        }
        if let Some(h) = hover.filter(|h| !self.fades.iter().any(|f| f.0.id == h.id)) {
            self.fades.push((h, 0., true));
        }
        for f in &mut self.fades {
            f.1 = if f.2 {
                (f.1 + dt / FADE_IN).min(1.)
            } else {
                f.1 - dt / FADE_OUT
            };
        }
        self.fades.retain(|f| f.1 > 0. || f.2);
        let turn = self.started.elapsed().as_secs_f32() * TURN;
        let mut rings = Vec::new();
        for (unit, fade, _) in &self.fades {
            if target.is_none_or(|t| t.id != unit.id) {
                rings.push((*unit, hover_look(*unit, *fade, turn)));
            }
        }
        if let Some(t) = target {
            let hovered = hover.is_some_and(|h| h.id == t.id);
            rings.push((t, target_look(hovered, lock_age, turn)));
        }
        self.shown = rings.clone();
        rings
    }
}
fn hover_look(unit: Unit, fade: f32, turn: f32) -> Look {
    Look {
        rgb: if unit.friendly { ALLY } else { ENEMY },
        alpha: fade.clamp(0., 1.),
        scale: 0.92 + 0.08 * fade.clamp(0., 1.),
        target: false,
        halo: 1.,
        spread: 1.,
        flash: 0.,
        turn,
    }
}
fn target_look(hovered: bool, lock_age: Option<f32>, turn: f32) -> Look {
    let fly = lock_age.map_or(1., |a| (a / LOCK_FLY).min(1.));
    let settle = lock_age.map_or(1., |a| (a / LOCK_FLASH).min(1.));
    Look {
        rgb: ENEMY,
        alpha: 1.,
        scale: 1.,
        target: true,
        halo: if hovered { 1.25 } else { 1.15 },
        spread: 1. + 0.9 * (1. - ease_out(fly)),
        flash: (1. - settle).powi(2) * 0.85,
        turn,
    }
}
fn ease_out(t: f32) -> f32 {
    1. - (1. - t).powi(3)
}
/// The game's own oval shadow under every champion, in world units: half
/// width, half height, and its centre below the champion's position. The
/// same for every champion (estimated 2026-10-19 from the author's 1.3.25
/// screenshots of a small and a large champion, which agree once converted
/// to world units; a guess until seen). The author: "the oval shape shadow
/// would be ideal, both position and size".
pub const CHAMPION_SHADOW: (f32, f32, f32) = (13.5, 3., 9.5);
/// The ring's half axes in world units. Champions: the game's shadow.
/// Others: a share of the measured body width, flattened like the units'
/// ground shadows: minions 55 % (80 % was "too wide for minion",
/// 2026-10-19), towers and the nexus 45 %, about their base's width (option
/// A, the author, 2026-10-19; 65 % circled the painted shadow), monsters 50 %.
pub fn radii(unit: &Unit) -> (f32, f32) {
    if unit.is_champion {
        return (CHAMPION_SHADOW.0, CHAMPION_SHADOW.1);
    }
    let width = unit
        .body
        .map_or((unit.radius as f32 / 1000.) * 2., |b| b.width);
    let share = if unit.is_minion {
        0.55
    } else if unit.is_tower {
        0.45
    } else {
        0.5
    };
    let rx = (width * share).clamp(6., 40.);
    (rx, rx * 0.38)
}
/// How far below the unit's position its feet are, in world units, by
/// kind of unit (seen in game 2026-10-19 with 1.3.23 and 1.3.24, measured
/// against the game's art the same day):
/// - champions: the centre of the game's shadow, `CHAMPION_SHADOW` (the
///   position itself put 1.3.25's rings at the hips, 11.5 put 1.3.24's under
///   the health bar);
/// - minions: the position is mid-body, the feet about 5 below (the value
///   the selection boxes use; the sheet's larger Morgard kinds made the
///   lowest art point too low);
/// - towers and the nexus: 6 below, just above the base's bottom edge so
///   the base hides the ring's back half (option A, 2026-10-19; 12 sat on
///   the painted shadow below the base, 29.5 at its bottom);
/// - jungle monsters: the lowest point of the measured body art, which is
///   their feet (bodies are stored around the frame centre, one sprite pixel
///   per world unit); without loaded art, half the body height.
pub fn feet(unit: &Unit) -> f32 {
    if unit.is_champion {
        return CHAMPION_SHADOW.2;
    }
    if unit.is_minion {
        return 5.;
    }
    if unit.is_tower {
        return 6.;
    }
    let measured = unit
        .body
        .and_then(|b| b.art)
        .and_then(crate::sprite_art::set)
        .and_then(|set| {
            set.iter()
                .flat_map(|art| art.bodies.iter().map(|b| b[3]))
                .reduce(f32::max)
        });
    measured
        .or_else(|| unit.body.map(|b| b.height / 2.))
        .unwrap_or(unit.radius as f32 / 1000.)
        .clamp(0., 80.)
}
/// World to map units.
pub fn to_map(frame: CameraFrame, world: (f32, f32)) -> (f32, f32) {
    (
        MAP / 2. + world.0 - frame.center.0,
        MAP / 2. + world.1 - frame.center.1,
    )
}
fn rgba(rgb: u32, alpha: f32) -> u32 {
    (rgb << 8) | (alpha.clamp(0., 1.) * 255.).round() as u32
}
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |v: u32, s: u32| ((v >> s) & 0xff) as f32;
    let m = |s| (ch(a, s) + (ch(b, s) - ch(a, s)) * t).round() as u32;
    (m(16) << 16) | (m(8) << 8) | m(0)
}
/// The strokes of one ring around `center` (map units). `ppu` is screen
/// pixels per world unit (2048 / camera extent); stroke widths are
/// designed in pixels at the default zoom and never thinner than 1.5 px.
pub fn lines(center: (f32, f32), radii: (f32, f32), look: Look, ppu: f32) -> Vec<Line> {
    let (rx, ry) = (radii.0 * look.scale, radii.1 * look.scale);
    let k = ry / rx;
    let zoom = (ppu / 2.).clamp(0.75, 1.25);
    let px = |design: f32| (design * zoom).max(1.5) / ppu;
    let rgb = mix(look.rgb, FLASH, look.flash);
    let a = look.alpha;
    let point = |angle: f32, r: f32| (center.0 + angle.cos() * r, center.1 + angle.sin() * r * k);
    let ellipse = |out: &mut Vec<Line>, r: f32, segments: usize, width: f32, color: u32| {
        for i in 0..segments {
            let a0 = i as f32 * std::f32::consts::TAU / segments as f32;
            let a1 = (i + 1) as f32 * std::f32::consts::TAU / segments as f32;
            out.push((point(a0, r), point(a1, r), width, color));
        }
    };
    let mut out = Vec::new();
    // Ground glow: concentric bands fading outward.
    let glow = if look.target { 0.26 } else { 0.18 } * a * (1. + look.flash * 0.6);
    let band = rx * 1.1 / GLOW_BANDS as f32;
    for i in 0..GLOW_BANDS {
        let t = i as f32 / GLOW_BANDS as f32;
        let strength = glow * (1. - t.powf(1.5)) * 0.55;
        ellipse(
            &mut out,
            band * (i as f32 + 0.5),
            GLOW_SEGMENTS,
            band,
            rgba(rgb, strength),
        );
    }
    // Rim: wide faint halo, narrower halo, crisp core.
    let core = px(if look.target { 2.5 } else { 2. }) + look.flash * 1.5 / ppu;
    for (extra, alpha) in [(5., 0.2), (2., 0.28)] {
        let width = core + px(extra) * look.halo;
        ellipse(&mut out, rx, RIM_SEGMENTS, width, rgba(rgb, alpha * a));
    }
    ellipse(&mut out, rx, RIM_SEGMENTS, core, rgba(rgb, a));
    if look.target {
        // Hairline ring and four inward chevrons on it, turning slowly.
        let outer = rx + 3.;
        ellipse(&mut out, outer, RIM_SEGMENTS, px(1.), rgba(rgb, 0.55 * a));
        let depth = 3.5;
        let wing = 2.2;
        for n in 0..4 {
            let angle =
                look.turn + n as f32 * std::f32::consts::FRAC_PI_2 + std::f32::consts::FRAC_PI_4;
            let r0 = (outer + 1.) * look.spread;
            let tip = point(angle, r0);
            let base = point(angle, r0 + depth);
            // Tangent of the ellipse at the base, for the chevron's wings.
            let (tx, ty) = (-angle.sin(), angle.cos() * k);
            let len = (tx * tx + ty * ty).sqrt().max(1e-3);
            let (tx, ty) = (tx / len * wing, ty / len * wing);
            let width = px(2.);
            out.push((tip, (base.0 + tx, base.1 + ty), width, rgba(rgb, a)));
            out.push((tip, (base.0 - tx, base.1 - ty), width, rgba(rgb, a)));
        }
    }
    out
}
/// Draws `rings` into the match world, after the frame.
pub fn draw(ctx: &mut StableClient<'_>, frame: CameraFrame, rings: &[(Unit, Look)]) {
    if !frame.valid() {
        return;
    }
    let ppu = MAP / frame.extent.0;
    for (unit, look) in rings {
        let world = (
            unit.position.0 as f32 / 1000.,
            unit.position.1 as f32 / 1000. + feet(unit),
        );
        for (a, b, width, color) in lines(to_map(frame, world), radii(unit), *look, ppu) {
            ctx.draw_line("Game", a.0, a.1, b.0, b.1, width, Z, color);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn unit(id: usize, friendly: bool) -> Unit {
        Unit {
            id,
            position: (500_000, 400_000),
            radius: 10_000,
            is_champion: true,
            is_minion: false,
            friendly,
            in_cc: false,
            is_tower: false,
            body: Some(crate::sprite_picking::Body {
                width: 24.,
                height: 40.,
                art: None,
            }),
        }
    }
    #[test]
    fn rings_are_compact_flat_and_sized_by_body_and_class() {
        let champion = unit(1, false);
        // Champions: the game's shadow, whatever their body.
        assert_eq!(radii(&champion), (13.5, 3.));
        let minion = Unit {
            is_minion: true,
            is_champion: false,
            ..champion
        };
        assert!((radii(&minion).0 - 13.2).abs() < 0.01);
        let tower = Unit {
            is_tower: true,
            is_champion: false,
            ..champion
        };
        assert!((radii(&tower).0 - 10.8).abs() < 0.01);
        let tiny = Unit {
            body: None,
            radius: 1_000,
            is_champion: false,
            ..champion
        };
        assert_eq!(radii(&tiny).0, 6.);
    }
    #[test]
    fn rings_sit_at_each_kind_of_units_feet() {
        let champion = unit(1, false);
        assert_eq!(feet(&champion), 9.5);
        let minion = Unit {
            is_champion: false,
            is_minion: true,
            ..champion
        };
        assert_eq!(feet(&minion), 5.);
        let tower = Unit {
            is_champion: false,
            is_tower: true,
            ..champion
        };
        assert_eq!(feet(&tower), 6.);
        // A monster without loaded art: half its body height, else its radius.
        let monster = Unit {
            is_champion: false,
            ..champion
        };
        assert_eq!(feet(&monster), 20.);
        let bare = Unit {
            body: None,
            ..monster
        };
        assert_eq!(feet(&bare), 10.);
    }
    #[test]
    fn a_pause_keeps_the_last_rings_and_leaving_the_match_clears_them() {
        let mut rings = GroundRings::default();
        let enemy = unit(7, false);
        let key = |r: Vec<(Unit, Look)>| r.into_iter().map(|(u, l)| (u.id, l)).collect::<Vec<_>>();
        let running = key(rings.update(None, Some(enemy), None));
        assert_eq!(running.len(), 1);
        assert_eq!(key(rings.paused()), running);
        assert_eq!(key(rings.paused()), running);
        rings.clear();
        assert!(rings.paused().is_empty());
    }
    #[test]
    fn the_map_is_centred_on_the_camera_one_unit_per_world_unit() {
        let frame = CameraFrame {
            viewport: crate::camera::Rect {
                x: 0.,
                y: 0.,
                w: 1920.,
                h: 1080.,
            },
            center: (500., 400.),
            extent: (1024., 1024.),
            zoom: 1.,
            minimap: crate::camera::Rect {
                x: 0.,
                y: 0.,
                w: 0.,
                h: 0.,
            },
        };
        assert_eq!(to_map(frame, (500., 400.)), (1024., 1024.));
        assert_eq!(to_map(frame, (510., 390.)), (1034., 1014.));
    }
    #[test]
    fn hover_fades_in_and_out_and_the_target_takes_over_its_ring() {
        let mut rings = GroundRings::default();
        let enemy = unit(7, false);
        let first = rings.update(Some(enemy), None, None);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].1.alpha, 0.); // Starts from nothing.
        rings.last = Some(Instant::now() - Duration::from_millis(50));
        let mid = rings.update(Some(enemy), None, None)[0].1;
        assert!(mid.alpha > 0.4 && mid.alpha < 0.7 && mid.scale < 1.);
        // Hover and target on one unit: only the target ring, with more halo.
        let both = rings.update(Some(enemy), Some(enemy), None);
        assert_eq!(both.len(), 1);
        assert!(both[0].1.target && both[0].1.halo > 1.15);
        // Hover leaves: the target stays, the hover ring is gone.
        let ally = unit(8, true);
        rings.last = Some(Instant::now() - Duration::from_millis(100));
        let moved = rings.update(Some(ally), Some(enemy), None);
        assert!(moved.iter().any(|(u, l)| u.id == 7 && l.target));
        assert!(moved.iter().any(|(u, l)| u.id == 8 && l.rgb == ALLY));
        // A friendly attack target gets no target ring.
        assert!(rings
            .update(None, Some(ally), None)
            .iter()
            .all(|(_, l)| !l.target));
    }
    #[test]
    fn an_attack_click_flies_the_chevrons_in_and_flashes_then_settles() {
        let mut rings = GroundRings::default();
        let enemy = unit(7, false);
        let at = Instant::now();
        let start = rings.update(None, Some(enemy), Some((7, at)))[0].1;
        assert!(start.spread > 1.8 && start.flash > 0.8);
        rings.lock = Some((7, at - Duration::from_millis(300)));
        let settled = rings.update(
            None,
            Some(enemy),
            Some((7, at - Duration::from_millis(300))),
        )[0]
        .1;
        assert_eq!((settled.spread, settled.flash), (1., 0.));
        // No click: steady target.
        let steady = target_look(false, None, 0.);
        assert_eq!((steady.spread, steady.flash), (1., 0.));
    }
    #[test]
    fn strokes_keep_a_readable_width_and_the_target_adds_chevrons() {
        let look = hover_look(unit(1, true), 1., 0.);
        let hover = lines((1024., 1024.), (14.4, 5.5), look, 2.);
        assert_eq!(hover.len(), GLOW_BANDS * GLOW_SEGMENTS + 3 * RIM_SEGMENTS);
        // Zoomed far out (0.5 px per unit) the core stays 1.5 px wide.
        let far = lines((1024., 1024.), (14.4, 5.5), look, 0.5);
        let core = far[GLOW_BANDS * GLOW_SEGMENTS + 2 * RIM_SEGMENTS].2;
        assert!((core * 0.5 - 1.5).abs() < 0.001);
        let target = lines(
            (1024., 1024.),
            (14.4, 5.5),
            target_look(false, None, 0.),
            2.,
        );
        assert_eq!(target.len(), hover.len() + RIM_SEGMENTS + 8);
        // Every stroke stays near the unit: within the glow and chevrons.
        assert!(target.iter().all(|(a, b, _, _)| [a, b]
            .iter()
            .all(|p| (p.0 - 1024.).abs() < 25. && (p.1 - 1024.).abs() < 12.)));
    }
}
