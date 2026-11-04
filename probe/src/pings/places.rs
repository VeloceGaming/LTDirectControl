//! Where the map's lanes, pits and bases are, for the ping's place
//! (1.7.1-dev13). Pure functions and one small store; version-free (the
//! map is the game's, not an executable's).
//!
//! **Lanes (guess, from the dev12 log, 2026-11-03).** Each laner's position
//! while his big plan was PassiveLine with its lane field read (one match,
//! side 0, 397 samples) gives three centre lines on the 960-unit map: top
//! along x ≈ 73 then y ≈ 49, bottom along y ≈ 911 then x ≈ 887, mid on
//! x + y ≈ 960 (the map turned half round onto itself, as the samples on
//! the side-0 half suggest). A point within [`LANE_BAND`] of a centre line
//! is on that lane; further away it is jungle or river (empty ground).
//!
//! **Pits.** No position is assumed: the controlled athlete's simulation
//! step learns where the Serpen and Morgard monsters stand from the units
//! named `serpen` and `epic` (the art sheets `ingame/serpen`, `ingame/epic`;
//! guess: the units carry those names), once a second. Since 1.7.1-dev15 a
//! pit is a pit only while its monster is alive (the author's spec, section
//! 9): seen alive at the latest look, and that look under [`PIT_STALE`]
//! old. A dead or unseen monster's pit is a lane or the ground.
//! Superseded 2026-11-04 (dev13, dev14): a pit stayed a pit for the
//! session once its monster was seen.
//!
//! **Pits by the game's clock (1.7.1-dev22, the author's spec section
//! 15.3).** A pit is also a place, whatever the vision, while the game's
//! own clock says its monster is up or spawns within [`PIT_GATE_SECS`]
//! (the game's "not up" threshold, reasons 0x21 and 0x24). The step hook
//! publishes that clock once a second ([`publish`]; the kind-0 object's
//! alive word and spawn ticks, read from the code) with the side's rally
//! point for the walk (`pings/walk.rs`). Until a pit's monster has been
//! seen its place is the map's fixed pit ([`SEEDED`], seen in the dev17
//! log).
//!
//! **Bases (guess, 1.7.1-dev15).** The lanes' centre lines meet in two
//! corners, (0, 960) and (960, 0); the dev12 samples put side 0's laners
//! near (0, 960) early in the match, so side 0's base is taken to be that
//! corner and side 1's the other. A point within [`BASE_RADIUS`] of its
//! own team's corner is the base; the check comes before the lanes, whose
//! lines run into both bases.
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// World units (1000 per map unit).
type Point = (u64, u64);

/// A point within this many map units of a lane's centre line is on the
/// lane (a guess to tune: lanes look about 60 units wide in the samples).
pub const LANE_BAND: f32 = 70.;
/// A point within this many map units of a pit is the pit (the brief's
/// value).
pub const PIT_RADIUS: f32 = 250.;
/// A point within this many map units of its own team's corner is the
/// base (a guess to tune).
pub const BASE_RADIUS: f32 = 130.;
/// A pit monster last seen longer ago than this is taken as gone (the
/// simulation step looks once a second).
pub const PIT_STALE: Duration = Duration::from_millis(2500);
/// The map's fixed pits in world units, by tag (Morgard 0, Serpen 1): the
/// positions the dev17 log showed (seen in game), used until a monster has
/// been seen.
pub const SEEDED: [Point; 2] = [(288_000, 288_000), (672_000, 672_000)];
/// A pit counts while its monster spawns within this many seconds (the
/// game's own gate: the respawn rule refuses an objective earlier).
pub const PIT_GATE_SECS: u64 = 15;
/// Each side's base corner in map units (guess, see above).
const BASES: [(f32, f32); 2] = [(0., 960.), (960., 0.)];

/// The three lanes' centre lines, in map units, by the game's lane number
/// (0 Top, 1 Mid, 2 Bottom).
const LANES: [&[(f32, f32)]; 3] = [
    &[(73., 960.), (73., 49.), (960., 49.)],
    &[(0., 960.), (960., 0.)],
    &[(0., 911.), (887., 911.), (887., 0.)],
];

fn segment_gap(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = dx * dx + dy * dy;
    let t = if len == 0. {
        0.
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len).clamp(0., 1.)
    };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}
fn map_units(p: Point) -> (f32, f32) {
    (p.0 as f32 / 1000., p.1 as f32 / 1000.)
}

/// How far a world point is from lane `lane`'s centre line, in map units.
pub fn lane_gap(p: Point, lane: u8) -> f32 {
    let p = map_units(p);
    LANES.get(usize::from(lane)).map_or(f32::INFINITY, |line| {
        line.windows(2)
            .map(|w| segment_gap(p, w[0], w[1]))
            .fold(f32::INFINITY, f32::min)
    })
}
/// The lane nearest to a world point, at any distance (the base's go, a
/// tower off the band).
pub fn nearest_lane(p: Point) -> u8 {
    (0..3u8)
        .min_by(|a, b| lane_gap(p, *a).total_cmp(&lane_gap(p, *b)))
        .unwrap_or(1)
}
/// The lane under a world point: the nearest centre line within the band.
pub fn lane_at(p: Point) -> Option<u8> {
    let lane = nearest_lane(p);
    (lane_gap(p, lane) <= LANE_BAND).then_some(lane)
}
/// The point is side `side`'s own base.
pub fn in_base(p: Point, side: u8) -> bool {
    let Some(corner) = BASES.get(usize::from(side)) else {
        return false;
    };
    let p = map_units(p);
    (p.0 - corner.0).hypot(p.1 - corner.1) <= BASE_RADIUS
}

/// A pit: its monster's objective tag (Morgard 0, Serpen 1) and position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pit {
    pub tag: u8,
    pub at: Point,
}
/// A pit monster as the simulation step last saw it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Seen {
    at: Point,
    entity: usize,
    alive: bool,
    when: Instant,
}
impl Seen {
    fn live(&self, now: Instant) -> bool {
        self.alive && now.saturating_duration_since(self.when) <= PIT_STALE
    }
}
/// The pits seen this session, by tag (Morgard 0, Serpen 1).
static PITS: Mutex<[Option<Seen>; 2]> = Mutex::new([None; 2]);
/// One monster as the game's clock reads it (the step hook, once a
/// second): up now, the ticks until it spawns (0 once up), and the
/// controlled side's rally point for it when read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clock {
    pub up: bool,
    pub spawn_in: u64,
    pub rally: Option<Point>,
}
/// The clock by tag, the match's ticks per second, and when it was read.
type Clocks = ([Option<Clock>; 2], u64, Instant);
static CLOCKS: Mutex<Option<Clocks>> = Mutex::new(None);
/// The step hook: the game's clock for both monsters (by tag), with the
/// match's ticks per second.
pub fn publish(clocks: [Option<Clock>; 2], tps: u64) {
    if let Ok(mut c) = CLOCKS.lock() {
        *c = Some((clocks, tps, crate::clock::now()));
    }
}
/// A match ended or another began: the clock is forgotten.
pub fn forget_clock() {
    if let Ok(mut c) = CLOCKS.lock() {
        *c = None;
    }
}
/// The clock of pit `tag`, while fresh.
pub fn clock(tag: u8) -> Option<(Clock, u64)> {
    let (clocks, tps, at) = (*CLOCKS.lock().ok()?)?;
    if crate::clock::since(at) > PIT_STALE {
        return None;
    }
    Some(((*clocks.get(usize::from(tag))?)?, tps))
}
/// The clock makes the pit a place: up, or spawning within the gate.
pub fn placeable(c: Clock, tps: u64) -> bool {
    c.up || c.spawn_in <= PIT_GATE_SECS * tps.max(1)
}
/// Where pit `tag` is: where its monster was seen, else the map's fixed
/// pit.
fn position(pits: &[Option<Seen>; 2], tag: usize) -> Point {
    pits.get(tag)
        .copied()
        .flatten()
        .map_or(SEEDED[tag.min(1)], |s| s.at)
}
/// The walk's target for pit `tag` (spec section 15.2): the side's rally
/// point as the clock read it, else the pit's position.
pub fn walk_target(tag: u8) -> Point {
    let rally = clock(tag).and_then(|(c, _)| c.rally);
    rally.unwrap_or_else(|| {
        let pits = PITS.lock().map(|p| *p).unwrap_or([None; 2]);
        position(&pits, usize::from(tag))
    })
}

/// The pit's tag for a unit's name, when it is a pit monster.
pub fn pit_tag(name: &str) -> Option<u8> {
    match crate::sprite_picking::sheet_name(name) {
        "epic" => Some(0),
        "serpen" => Some(1),
        _ => None,
    }
}
/// The simulation step saw the pit monster `tag` (unit `entity`) at `at`,
/// alive or not. True the first time this session (the caller logs it).
pub fn learn(tag: u8, at: Point, entity: usize, alive: bool) -> bool {
    let Ok(mut pits) = PITS.lock() else {
        return false;
    };
    let Some(slot) = pits.get_mut(usize::from(tag)) else {
        return false;
    };
    let seen = Seen {
        at,
        entity,
        alive,
        when: crate::clock::now(),
    };
    slot.replace(seen).is_none()
}
/// The pit within [`PIT_RADIUS`] of a world point, nearest first: one
/// whose monster was seen alive lately, or whose clock makes it a place.
pub fn pit_at(p: Point) -> Option<Pit> {
    let pits = *PITS.lock().ok()?;
    let clocks: [Option<(Clock, u64)>; 2] = std::array::from_fn(|t| clock(t as u8));
    nearest_pit(&places(&pits, &clocks, crate::clock::now()), p)
}
/// Pit `tag` is a place now (seen alive lately, or by the clock): the
/// walk's "monster" hand-back (`walk.rs`).
pub fn is_place(tag: u8) -> bool {
    let Ok(pits) = PITS.lock().map(|p| *p) else {
        return false;
    };
    let clocks: [Option<(Clock, u64)>; 2] = std::array::from_fn(|t| clock(t as u8));
    places(&pits, &clocks, crate::clock::now())
        .get(usize::from(tag))
        .is_some_and(Option::is_some)
}
/// The pits that are places now, by tag.
fn places(
    pits: &[Option<Seen>; 2],
    clocks: &[Option<(Clock, u64)>; 2],
    now: Instant,
) -> [Option<Point>; 2] {
    let seen = live(pits, now);
    std::array::from_fn(|t| {
        seen[t].or_else(|| {
            clocks[t]
                .filter(|(c, tps)| placeable(*c, *tps))
                .map(|_| position(pits, t))
        })
    })
}
/// The live pit whose monster is unit `entity` (a click on the monster).
pub fn pit_of(entity: usize) -> Option<Pit> {
    let pits = *PITS.lock().ok()?;
    let now = crate::clock::now();
    pits.iter().enumerate().find_map(|(tag, s)| {
        let s = s.filter(|s| s.entity == entity && s.live(now))?;
        Some(Pit {
            tag: tag as u8,
            at: s.at,
        })
    })
}
fn live(pits: &[Option<Seen>; 2], now: Instant) -> [Option<Point>; 2] {
    pits.map(|s| s.filter(|s| s.live(now)).map(|s| s.at))
}
fn nearest_pit(pits: &[Option<Point>; 2], p: Point) -> Option<Pit> {
    let p = map_units(p);
    pits.iter()
        .enumerate()
        .filter_map(|(tag, at)| {
            let at = (*at)?;
            let q = map_units(at);
            let gap = (p.0 - q.0).hypot(p.1 - q.1);
            (gap <= PIT_RADIUS).then_some((gap, Pit { tag: tag as u8, at }))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, pit)| pit)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn w(x: u64, y: u64) -> Point {
        (x * 1000, y * 1000)
    }
    #[test]
    fn the_samples_lie_on_their_lanes_and_the_jungle_on_none() {
        // dev12 log samples: top laner at (16, 844), mid at (77, 875) and
        // (550, 403), bottom at (109, 944) and (876, 600).
        assert_eq!(lane_at(w(16, 844)), Some(0));
        assert_eq!(lane_at(w(150, 60)), Some(0));
        assert_eq!(lane_at(w(550, 403)), Some(1));
        assert_eq!(lane_at(w(480, 480)), Some(1));
        assert_eq!(lane_at(w(400, 911)), Some(2));
        assert_eq!(lane_at(w(876, 600)), Some(2));
        // The jungle between top and mid, and between mid and bottom.
        assert_eq!(lane_at(w(250, 300)), None);
        assert_eq!(lane_at(w(700, 700)), None);
    }
    #[test]
    fn a_pit_is_known_once_seen_and_covers_its_radius() {
        let pits = [None, Some(w(610, 610))];
        assert_eq!(nearest_pit(&pits, w(700, 700)).map(|p| p.tag), Some(1));
        assert_eq!(nearest_pit(&pits, w(900, 900)), None, "424 units away");
        assert_eq!(nearest_pit(&[None, None], w(610, 610)), None);
        let both = [Some(w(350, 350)), Some(w(610, 610))];
        assert_eq!(nearest_pit(&both, w(450, 450)).map(|p| p.tag), Some(0));
        assert_eq!(pit_tag("serpen"), Some(1));
        assert_eq!(pit_tag("epic_monster"), Some(0));
        assert_eq!(pit_tag("tower"), None);
    }
    #[test]
    fn a_pit_counts_only_while_its_monster_lives_and_was_seen_lately() {
        let t = Instant::now();
        let seen = |alive, when| Seen {
            at: w(610, 610),
            entity: 40,
            alive,
            when,
        };
        assert_eq!(live(&[None, Some(seen(true, t))], t)[1], Some(w(610, 610)));
        assert_eq!(live(&[None, Some(seen(false, t))], t)[1], None, "dead");
        let late = t + PIT_STALE + Duration::from_millis(1);
        assert_eq!(live(&[None, Some(seen(true, t))], late)[1], None, "stale");
    }
    #[test]
    fn a_pit_in_fog_is_a_place_by_the_games_clock() {
        let t = Instant::now();
        let tps = 60;
        let soon = Clock {
            up: false,
            spawn_in: 10 * tps,
            rally: None,
        };
        let later = Clock {
            spawn_in: 20 * tps,
            ..soon
        };
        // Never seen: the fixed pit, 10 s away counts, 20 s away does not.
        let p = places(&[None, None], &[None, Some((soon, tps))], t);
        assert_eq!(p[1], Some(SEEDED[1]));
        assert_eq!(nearest_pit(&p, w(700, 700)).map(|p| p.tag), Some(1));
        let p = places(&[None, None], &[None, Some((later, tps))], t);
        assert_eq!(p[1], None, "20 s: ground or lane as before");
        // Up, unseen (fog): a place; seen earlier, at its seen position.
        let up = Clock {
            up: true,
            spawn_in: 0,
            rally: None,
        };
        let seen = Seen {
            at: w(300, 300),
            entity: 9,
            alive: false,
            when: t,
        };
        assert_eq!(
            places(&[Some(seen), None], &[Some((up, tps)), None], t)[0],
            Some(w(300, 300))
        );
        assert!(placeable(soon, tps) && !placeable(later, tps) && placeable(up, tps));
    }
    #[test]
    fn the_base_is_the_own_corner_and_the_nearest_lane_is_any_distance() {
        assert!(in_base(w(40, 920), 0));
        assert!(!in_base(w(40, 920), 1), "the other team's base");
        assert!(in_base(w(920, 40), 1));
        assert!(!in_base(w(480, 480), 0));
        assert!(!in_base(w(40, 920), 7));
        assert_eq!(nearest_lane(w(250, 300)), 0, "nearer top than mid");
        assert!(lane_gap(w(73, 500), 0).abs() < 0.01);
        assert_eq!(lane_at(w(900, 500)), Some(2));
    }
}
