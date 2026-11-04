//! The wheel's two-level marking-menu gesture (1.7.1-dev18, the author's
//! spec section 11.1, held to design/marking-menu-reference.md): the
//! cursor path selects, the drawing follows. Pure; the wheel feeds it the
//! cursor each frame and asks what a release fires. No time enters a
//! selection: every threshold is a distance or an angle.
//!
//! - **Level 1**, around the wheel's centre: the hub ([`HUB`]) selects
//!   nothing; past it the stroke is on a half, right ([`RIGHT`], personal)
//!   or left ([`LEFT`], team), split by the vertical axis (x = 0 counts as
//!   right). Once the stroke crosses the ring's outer edge ([`RIM`]) the
//!   half is the one where the segment between two frames crossed it, so a
//!   fast flick keeps the half it went through.
//! - **The corner.** From the rim crossing, the fan's centre follows the
//!   cursor while the stroke keeps going outward: each motion of at least
//!   [`MOTION`] from the centre within [`STRAIGHT`] of the half's outward
//!   axis (+x right, 180° left) moves the centre to the cursor. The first
//!   motion outside that band fixes the centre where it is: the corner, the
//!   last point before the turn (a stop and a restart in a new direction is
//!   the same, the stop point being the centre). Maya's corner rule (the
//!   patent takes half the wedge spacing; ours is half the 36° to the
//!   nearest slot).
//! - **Level 2**, from the corner: five equal parts of 72° measured from
//!   the outward axis, the left half mirrored. Four are slots, centred at
//!   −36° (up-and-out), +36° (down-and-out), −108° (up) and +108° (down)
//!   ([`ANGLES`] in screen angles); once the cursor is [`SUB_DEAD`] from the
//!   corner the slot is the part its direction falls in. The fifth part,
//!   toward the wheel, chooses nothing, and the fan stays open: moving
//!   back out into a slot's part chooses it again (1.7.1-dev19, spec
//!   section 12.3; dev18 closed the fan there). Boundaries go to the
//!   earlier slot in [`ANGLES`] order, so straight out from the corner is
//!   up-and-out.
//! - A release while the centre still follows (no corner) or in the back
//!   part chooses nothing.
//! - Coming back inside the rim closes the fan and the half is free again.
//! - A stroke that starts outside the rim (a press near a screen edge, the
//!   wheel moved inward) crosses nothing until it has been inside the rim
//!   once.
//! - The stroke's path is kept for its ink trail ([`Gesture::trail`]).
//!
//! Drawing is apart (`wheel_draw::Hover`): the fan is drawn only once the
//! cursor holds still, never because of a corner.
//!
//! Superseded 2026-11-04 (dev18): dev17's fan opened at [`RIM`] + 20 px
//! (`LOCK`) on the path, its slots at −90°, ∓30° and +90° with the nearest
//! within 60°; dev16's four fixed slots by direction and its sub-menu of
//! variants; dev15's sub-ring.

/// The hub's radius and the ring (its inner and outer edge), screen pixels
/// (the author's: hub 26, ring 34..100).
pub const HUB: f32 = 26.;
pub const RING_IN: f32 = 34.;
pub const RIM: f32 = 100.;
/// The band around the half's outward axis within which a motion keeps
/// the fan's centre following (degrees): half the 36° from the axis to the
/// nearest slot.
pub const STRAIGHT: f32 = 18.;
/// A motion shorter than this from the centre has no direction yet (screen
/// pixels): a twitch is not a turn (1.7.1-dev19, spec section 12.3; dev18
/// 5).
pub const MOTION: f32 = 12.;
/// Within this many pixels of the corner, no slot is chosen.
pub const SUB_DEAD: f32 = 24.;
/// The fan's slots: centred this far from the corner, squares this size.
pub const FAN_AT: f32 = 96.;
pub const SLOT: f32 = 72.;
/// One part of the fan's circle (degrees): five equal parts.
pub const PART: f32 = 72.;
/// Each half's slot angles (degrees, screen: y down, clockwise positive):
/// up, up-and-out, down-and-out, down. The right half's are −108, −36, 36,
/// 108 from its outward axis (0°); the left half's the same mirrored about
/// the vertical axis.
pub const ANGLES: [[f32; 4]; 2] = [[-108., -36., 36., 108.], [-72., -144., 144., 72.]];
/// Back inside the rim by this much closes the fan.
const UNLOCK: f32 = 6.;
/// The trail keeps this many points (a stroke is short).
const TRAIL: usize = 64;

/// The halves by index.
pub const RIGHT: usize = 0;
pub const LEFT: usize = 1;

/// What the stroke selects now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    /// The hub: a release cancels.
    Centre,
    /// A half with no slot chosen: a release cancels too.
    Half(usize),
    /// A half's slot (0..4 in [`ANGLES`] order).
    Slot(usize, usize),
}
impl Selection {
    pub fn half(self) -> Option<usize> {
        match self {
            Selection::Centre => None,
            Selection::Half(h) | Selection::Slot(h, _) => Some(h),
        }
    }
    pub fn slot(self) -> Option<(usize, usize)> {
        match self {
            Selection::Slot(h, n) => Some((h, n)),
            _ => None,
        }
    }
}

/// The fan: its half, its centre on screen, and whether the centre is the
/// corner (fixed) or still follows the cursor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fan {
    pub half: usize,
    pub center: (f32, f32),
    pub corner: bool,
}
impl Fan {
    /// Slot `n`'s centre on screen.
    pub fn slot(&self, n: usize) -> (f32, f32) {
        let a = ANGLES[self.half][n].to_radians();
        (
            self.center.0 + FAN_AT * a.cos(),
            self.center.1 + FAN_AT * a.sin(),
        )
    }
}

/// Where the fan stands on a crossed half.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Level2 {
    /// The centre follows the cursor outward.
    Following((f32, f32)),
    /// Fixed at the corner.
    Corner((f32, f32)),
}

#[derive(Clone, Copy, Debug)]
pub struct Gesture {
    center: (f32, f32),
    last: (f32, f32),
    /// The half whose rim the stroke crossed, and its fan.
    crossed: Option<(usize, Level2)>,
    /// The stroke has been inside the rim.
    inside: bool,
    selection: Selection,
    trail: [(f32, f32); TRAIL],
    trail_len: usize,
}

/// The half of a point relative to the centre: x = 0 is the right.
pub fn half((x, _): (f32, f32)) -> usize {
    if x >= 0. {
        RIGHT
    } else {
        LEFT
    }
}
/// A direction's angle from half `h`'s outward axis, mirrored on the left
/// (degrees, −180..180, clockwise positive on screen; up is negative on
/// both halves).
fn from_outward(h: usize, (x, y): (f32, f32)) -> f32 {
    let a = y.atan2(x).to_degrees();
    let a = if h == RIGHT { a } else { 180. - a };
    (a + 540.) % 360. - 180.
}
/// The part of the fan a direction from the corner falls in on half `h`:
/// a slot (0..4 in [`ANGLES`] order), or None for the part toward the
/// wheel. Boundaries go to the earlier slot.
pub fn slot(h: usize, d: (f32, f32)) -> Option<usize> {
    let a = from_outward(h, d);
    if (-2. * PART..=-PART).contains(&a) {
        Some(0)
    } else if a > -PART && a <= 0. {
        Some(1)
    } else if a > 0. && a <= PART {
        Some(2)
    } else if a > PART && a <= 2. * PART {
        Some(3)
    } else {
        None
    }
}
/// Where the segment from `a` to `b` (relative to the centre) first
/// crosses the circle of radius `r`, when `a` is inside it.
fn crossing(a: (f32, f32), b: (f32, f32), r: f32) -> Option<(f32, f32)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let qa = dx * dx + dy * dy;
    let qb = 2. * (a.0 * dx + a.1 * dy);
    let qc = a.0 * a.0 + a.1 * a.1 - r * r;
    if qa == 0. || qc > 0. {
        return None;
    }
    let t = (-qb + (qb * qb - 4. * qa * qc).max(0.).sqrt()) / (2. * qa);
    (0. ..=1.)
        .contains(&t)
        .then_some((a.0 + t * dx, a.1 + t * dy))
}
/// The fan on half `h` after the cursor moved to `p` (screen points).
fn advance(h: usize, level: Level2, p: (f32, f32)) -> Level2 {
    match level {
        Level2::Following(c) => {
            let d = (p.0 - c.0, p.1 - c.1);
            if d.0.hypot(d.1) < MOTION {
                level
            } else if from_outward(h, d).abs() <= STRAIGHT {
                Level2::Following(p)
            } else {
                // The turn: the corner, judged at once from this motion.
                advance(h, Level2::Corner(c), p)
            }
        }
        Level2::Corner(_) => level,
    }
}

impl Gesture {
    /// A stroke that pressed at `press`, around the drawn centre `center`.
    pub fn new(press: (f32, f32), center: (f32, f32)) -> Self {
        let mut trail = [(0., 0.); TRAIL];
        trail[0] = press;
        let inside = (press.0 - center.0).hypot(press.1 - center.1) < RIM;
        Self {
            center,
            last: press,
            crossed: None,
            inside,
            selection: Selection::Centre,
            trail,
            trail_len: 1,
        }
    }
    pub fn center(&self) -> (f32, f32) {
        self.center
    }
    pub fn selection(&self) -> Selection {
        self.selection
    }
    /// The fan from the rim crossing until the cursor is back inside the
    /// ring: its centre follows the cursor, then stands at the corner.
    pub fn fan(&self) -> Option<Fan> {
        let (half, level) = self.crossed?;
        let (center, corner) = match level {
            Level2::Following(c) => (c, false),
            Level2::Corner(c) => (c, true),
        };
        Some(Fan {
            half,
            center,
            corner,
        })
    }
    /// The stroke's path from the press point (for its ink trail).
    pub fn trail(&self) -> &[(f32, f32)] {
        &self.trail[..self.trail_len]
    }
    /// The cursor at `p`.
    pub fn update(&mut self, p: (f32, f32)) {
        let c = self.center;
        let rel = |q: (f32, f32)| (q.0 - c.0, q.1 - c.1);
        let (here, before) = (rel(p), rel(self.last));
        self.last = p;
        let tail = self.trail[self.trail_len - 1];
        if (p.0 - tail.0).hypot(p.1 - tail.1) >= 4. {
            if self.trail_len == TRAIL {
                self.trail.copy_within(1.., 0);
                self.trail_len -= 1;
            }
            self.trail[self.trail_len] = p;
            self.trail_len += 1;
        }
        let r = here.0.hypot(here.1);
        if self.crossed.is_some() && r < RIM - UNLOCK {
            self.crossed = None;
        }
        self.crossed = match self.crossed {
            Some((h, level)) => Some((h, advance(h, level, p))),
            None if r >= RIM && self.inside => {
                // Following from where the segment crossed the rim, then
                // the rest of the segment as a motion.
                let at = crossing(before, here, RIM).unwrap_or(here);
                let h = half(at);
                let start = Level2::Following((c.0 + at.0, c.1 + at.1));
                Some((h, advance(h, start, p)))
            }
            None => None,
        };
        self.inside |= r < RIM;
        self.selection = match self.crossed {
            Some((h, Level2::Corner(k))) => {
                let d = (p.0 - k.0, p.1 - k.1);
                match (d.0.hypot(d.1) > SUB_DEAD).then(|| slot(h, d)).flatten() {
                    Some(n) => Selection::Slot(h, n),
                    None => Selection::Half(h),
                }
            }
            Some((h, _)) => Selection::Half(h),
            None if r <= HUB => Selection::Centre,
            None => Selection::Half(half(here)),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const C: (f32, f32) = (500., 500.);
    fn run(points: &[(f32, f32)]) -> Gesture {
        let mut g = Gesture::new(C, C);
        for p in points {
            g.update((C.0 + p.0, C.1 + p.1));
        }
        g
    }
    fn stroke(points: &[(f32, f32)]) -> Selection {
        run(points).selection()
    }
    #[test]
    fn the_hub_the_halves_and_no_slot_before_a_corner() {
        assert_eq!(stroke(&[(10., 10.)]), Selection::Centre);
        assert_eq!(stroke(&[(60., -20.)]), Selection::Half(RIGHT));
        assert_eq!(stroke(&[(-60., 20.)]), Selection::Half(LEFT));
        assert_eq!(
            stroke(&[(0., -60.)]),
            Selection::Half(RIGHT),
            "x = 0 is right"
        );
        // A straight drag never selects, however far: the centre follows.
        for path in [
            &[(60., 0.), (140., 0.), (260., 4.), (400., -8.)][..],
            &[(400., 0.)][..],
            &[(-60., 0.), (-300., 30.)][..],
        ] {
            let g = run(path);
            let fan = g.fan().unwrap();
            assert!(!fan.corner, "{path:?}");
            assert_eq!(
                fan.center,
                (C.0 + path.last().unwrap().0, C.1 + path.last().unwrap().1)
            );
            assert!(g.selection().slot().is_none(), "{path:?}");
        }
    }
    #[test]
    fn the_turn_fixes_the_corner_and_the_parts_choose_from_it() {
        // Right then up, one frame each: up (Assist me's place).
        let g = run(&[(140., 0.), (140., -70.)]);
        let fan = g.fan().unwrap();
        assert!(fan.corner && fan.center == (640., 500.));
        assert_eq!(g.selection(), Selection::Slot(RIGHT, 0));
        // From a corner at x 140: up-and-out, down-and-out, down.
        let at = |d: (f32, f32)| stroke(&[(140., 0.), (140. + d.0, d.1)]);
        assert_eq!(at((40., -40.)), Selection::Slot(RIGHT, 1));
        assert_eq!(at((40., 40.)), Selection::Slot(RIGHT, 2));
        assert_eq!(at((0., 50.)), Selection::Slot(RIGHT, 3));
        assert_eq!(at((-20., -60.)), Selection::Slot(RIGHT, 0), "up leans back");
        // Within the dead zone from the corner: no slot yet.
        assert_eq!(at((0., -20.)), Selection::Half(RIGHT));
        // Back toward the wheel past the dead zone: no slot, the fan still
        // open; then up from the corner: the up slot; back inside the ring:
        // closed (spec section 12.3; superseded 2026-11-04: dev18 closed
        // the fan in the back part).
        let g = run(&[(160., 0.), (120., 0.)]);
        assert_eq!(g.selection(), Selection::Half(RIGHT));
        assert!(g.fan().is_some_and(|f| f.corner), "still open");
        let g = run(&[(160., 0.), (120., 0.), (160., -60.)]);
        assert_eq!(g.selection(), Selection::Slot(RIGHT, 0));
        let g = run(&[(160., 0.), (120., 0.), (60., 0.)]);
        assert!(g.fan().is_none(), "inside the ring: closed");
        // The left half mirrors it.
        let left = |d: (f32, f32)| stroke(&[(-140., 0.), (-140. + d.0, d.1)]);
        assert_eq!(left((0., -60.)), Selection::Slot(LEFT, 0));
        assert_eq!(left((-40., -40.)), Selection::Slot(LEFT, 1));
        assert_eq!(left((-40., 40.)), Selection::Slot(LEFT, 2));
        assert_eq!(left((0., 60.)), Selection::Slot(LEFT, 3));
        assert_eq!(left((40., 0.)), Selection::Half(LEFT), "back");
    }
    #[test]
    fn a_stop_then_a_move_picks_from_the_stop_point() {
        // Out to x 180 in small steps, a stop (jitter under MOTION), then
        // down: the corner is the stop point, not the rim.
        let mut path: Vec<(f32, f32)> = (1..=18).map(|i| (10. * i as f32, 0.)).collect();
        path.extend([(181., 1.), (182., -1.), (180., 0.), (180., 60.)]);
        let g = run(&path);
        let fan = g.fan().unwrap();
        assert!(fan.corner && (fan.center.0 - 680.).abs() < 0.01);
        assert_eq!(g.selection(), Selection::Slot(RIGHT, 3));
        // An 8 px sideways twitch after the crossing fixes no corner.
        let g = run(&[(120., 0.), (120., 8.), (200., 8.)]);
        assert!(!g.fan().unwrap().corner);
        // A slight wobble outward is no turn.
        let g = run(&[(120., 0.), (160., 10.), (200., 0.)]);
        assert!(!g.fan().unwrap().corner);
        // Straight out from a corner is the earlier slot, up-and-out.
        assert_eq!(slot(RIGHT, (1., 0.)), Some(1));
        assert_eq!(slot(LEFT, (-1., 0.)), Some(1));
        assert_eq!(slot(RIGHT, (-1., 0.)), None);
    }
    #[test]
    fn a_fast_flick_keeps_its_half_and_back_inside_frees_it() {
        // Two frames: right through the rim, then up and out.
        assert_eq!(
            stroke(&[(200., 0.), (240., -60.)]),
            Selection::Slot(RIGHT, 1)
        );
        // Crossed on the left even though the cursor ends right of centre:
        // the crossing is the corner (the segment is not outward).
        let g = run(&[(-60., -90.), (10., -300.)]);
        assert_eq!(g.fan().map(|f| f.half), Some(LEFT));
        // Started outside the rim: nothing until it has been inside.
        let mut g = Gesture::new((300., 500.), C);
        g.update((290., 500.));
        assert!(g.fan().is_none() && g.selection() == Selection::Half(LEFT));
        g.update((420., 500.));
        g.update((660., 500.));
        assert_eq!(g.fan().map(|f| f.half), Some(RIGHT));
        // Back inside the rim: the fan closes, the half follows the cursor.
        let g = run(&[(140., 0.), (-60., 0.)]);
        assert!(g.fan().is_none());
        assert_eq!(g.selection(), Selection::Half(LEFT));
        assert!(crossing((0., 0.), (0., -200.), RIM).is_some_and(|p| (p.1 + RIM).abs() < 0.01));
        // The trail keeps the stroke from the press.
        let g = run(&[(0., -50.), (0., -100.), (0., -101.)]);
        assert_eq!(g.trail().len(), 3);
        assert_eq!(g.trail()[0], C);
        // The slots sit 96 px from the corner at their angles.
        let fan = Fan {
            half: RIGHT,
            center: (0., 0.),
            corner: true,
        };
        let up = fan.slot(0);
        assert!((up.0 - 96. * (-108f32).to_radians().cos()).abs() < 0.01 && up.1 < -90.);
    }
}
