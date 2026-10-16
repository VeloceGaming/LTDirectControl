//! Practice tool (testing aid, not in the settings window: switched on by
//! `"practice_mode": 1` in controls.json, which is read at game start; it
//! changes the real match). While it is on, in the match on screen:
//! - every athlete but the controlled one holds still: its input becomes
//!   "move to where you are", so it neither walks, attacks nor casts;
//! - every champion's health is refilled each tick, and the no-cooldown
//!   cheat (`test_cheats`) is on for the controlled champion;
//! - Home+Insert places the enemy champions in a row at the cursor and
//!   puts them back there every tick;
//! - the draft picks the champions named in `practice_lineup` (a comma-
//!   separated list of champion names in controls.json) for whichever team
//!   the game's AI drafts for, and never bans them;
//! - a readout lists the controlled athlete's last shots: the skill, the
//!   projectile kind or action state, and which hit test it reached
//!   (`crate::athlete_rules`; it needs the stat rules installed).
use crate::athlete_rules::{HitTest, ShotEvent};
use crate::native_timing::MatchKey;
use mod_api_stable::{InputV1, StableAiContext, StableDraftContext, StableDraftHook, StableSim};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

static ON: AtomicBool = AtomicBool::new(false);
/// A requested row position for the enemies, `x << 32 | y` in internal
/// units, or 0.
static PULL: AtomicU64 = AtomicU64::new(0);
static PULL_HELD: AtomicBool = AtomicBool::new(false);
static LINEUP: Mutex<(String, Vec<String>)> = Mutex::new((String::new(), Vec::new()));
/// Distance between two enemies placed in a row, in internal units.
const ROW_SPACING: u64 = 12_500;
/// Ticks after a pull at which the pinned enemies are reported (about one
/// second).
const REPORT_TICKS: usize = 60;

/// Where the last pull placed each enemy, in the match it was made in.
#[derive(Default)]
struct Pins {
    key: Option<MatchKey>,
    spots: Vec<Pin>,
    report_at: Option<usize>,
}
struct Pin {
    entity: usize,
    name: String,
    spot: (u64, u64),
    /// The furthest it was found from its spot in one tick, internal units.
    max_drift: u64,
}
static PINS: Mutex<Pins> = Mutex::new(Pins {
    key: None,
    spots: Vec::new(),
    report_at: None,
});

pub fn active() -> bool {
    ON.load(Ordering::Relaxed)
}

/// Client frame: copies the setting and the lineup, logs a change, and
/// turns a fresh Home+Insert at a ground point into a pull request.
pub fn configure(on: bool, lineup: Option<String>, pull: Option<(u64, u64)>, log: &crate::Logger) {
    if ON.swap(on, Ordering::Relaxed) != on {
        log.write(&format!("PRACTICE {}", if on { "ON" } else { "OFF" }));
        // Off releases the pulled enemies; on again does not pull them back.
        if let Ok(mut pins) = PINS.lock() {
            *pins = Pins::default();
        }
    }
    let lineup = lineup.unwrap_or_default();
    if let Ok(mut current) = LINEUP.lock() {
        if current.0 != lineup {
            current.1 = parse_lineup(&lineup);
            log.write(&format!("PRACTICE lineup {:?}", current.1));
            current.0 = lineup;
        }
    }
    match pull.filter(|_| on) {
        Some((x, y)) if !PULL_HELD.swap(true, Ordering::Relaxed) => {
            if x < u64::from(u32::MAX) && y < u64::from(u32::MAX) {
                PULL.store((x << 32) | y.max(1), Ordering::Relaxed);
            }
        }
        Some(_) => {}
        None => PULL_HELD.store(false, Ordering::Relaxed),
    }
}

/// Champion names, lower case, in order, without blanks or repeats.
pub fn parse_lineup(text: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for name in text.split(',').map(|n| n.trim().to_lowercase()) {
        if !name.is_empty() && !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

fn lineup() -> Vec<String> {
    LINEUP.lock().map(|l| l.1.clone()).unwrap_or_default()
}

/// The input every athlete of the match on screen but the controlled one
/// gets while practising: move to where it stands. None for the controlled
/// athlete and outside that match.
pub fn hold(
    ctx: &mut StableAiContext<'_>,
    movement: &crate::movement::Movement,
) -> Option<InputV1> {
    if !active() {
        return None;
    }
    let player = ctx.player_id();
    let sim = ctx.sim()?;
    let origin = sim.sim_origin().filter(|o| o.kind == 2)?;
    let key = (sim.seed(), origin.match_id, origin.set_index);
    let (viewed, controlled) = movement.hud_identity()?;
    if viewed != key || controlled == player {
        return None;
    }
    let champion = sim.get_player(player)?.champion()?;
    let (x, y) = champion.pos();
    Some(InputV1::move_to(x, y))
}

/// Match tick of the match on screen (`key`): refill every champion's
/// health, place the enemies when a pull was requested, and put the pulled
/// enemies back on their spots. `controlled` is the player the mod controls
/// in it.
pub fn tick(sim: &mut StableSim<'_>, controlled: usize, key: MatchKey, log: &crate::Logger) {
    let Some(team) = sim.get_player(controlled).map(|p| p.team()) else {
        return;
    };
    let mut champions = Vec::new();
    for index in 0..sim.player_count() {
        if let Some(p) = sim.player_at(index) {
            if let Some(c) = p.champion() {
                let name = c.name().unwrap_or_else(|| "?".to_owned());
                champions.push((c.id(), c.hp().1, p.team() != team, name));
            }
        }
    }
    for (entity, max, _, _) in &champions {
        sim.entity_set_hp(*entity, *max);
    }
    let Ok(mut pins) = PINS.lock() else {
        return;
    };
    if pins.key != Some(key) {
        // Another match: its entities are not the pulled ones.
        *pins = Pins {
            key: Some(key),
            ..Pins::default()
        };
    }
    let now = sim.tick();
    let pull = PULL.swap(0, Ordering::Relaxed);
    if pull != 0 {
        let (x, y) = (pull >> 32, pull & 0xffff_ffff);
        let enemies: Vec<_> = champions.iter().filter(|c| c.2).collect();
        let count = enemies.len() as u64;
        // A row along x, centred on the cursor.
        let start = x.saturating_sub(count.saturating_sub(1) * ROW_SPACING / 2);
        let mut refused = Vec::new();
        pins.spots.clear();
        for (i, (entity, _, _, name)) in enemies.into_iter().enumerate() {
            let spot = (start + i as u64 * ROW_SPACING, y);
            if !sim.entity_set_pos(*entity, spot.0, spot.1) {
                refused.push(name.clone());
            }
            pins.spots.push(Pin {
                entity: *entity,
                name: name.clone(),
                spot,
                max_drift: 0,
            });
        }
        pins.report_at = Some(now + REPORT_TICKS);
        log.write(&format!(
            "PRACTICE pulled {count} enemies to ({}, {}); teleport refused for {refused:?}",
            x / 1000,
            y / 1000
        ));
        return;
    }
    // Back on their spots, and how far they had got since the last tick.
    for pin in &mut pins.spots {
        let Some(pos) = sim.get_entity(pin.entity).map(|e| e.pos()) else {
            continue;
        };
        let drift = distance(pos, pin.spot);
        if drift > 0 {
            pin.max_drift = pin.max_drift.max(drift);
            sim.entity_set_pos(pin.entity, pin.spot.0, pin.spot.1);
        }
    }
    if pins.report_at.is_some_and(|at| now >= at) {
        pins.report_at = None;
        let report: Vec<String> = pins
            .spots
            .iter()
            .map(|p| {
                format!(
                    "{} at ({}, {}) moved up to {} units in a tick",
                    p.name,
                    p.spot.0 / 1000,
                    p.spot.1 / 1000,
                    p.max_drift / 1000
                )
            })
            .collect();
        log.write(&format!("PRACTICE after 1 s: {}", report.join("; ")));
    }
}

fn distance(a: (u64, u64), b: (u64, u64)) -> u64 {
    let dx = a.0.abs_diff(b.0) as f64;
    let dy = a.1.abs_diff(b.1) as f64;
    dx.hypot(dy) as u64
}

/// Picks the lineup's champions and never bans them, while practising.
pub struct LineupDraft;
impl StableDraftHook for LineupDraft {
    fn id(&self) -> String {
        format!("{}:practice_lineup", crate::MOD_ID)
    }
    fn decide_pick(&self, ctx: &StableDraftContext<'_>) -> Option<usize> {
        let names = lineup();
        if !active() || names.is_empty() {
            return None;
        }
        let taken: Vec<usize> = ctx
            .ally_picks()
            .iter()
            .chain(ctx.enemy_picks())
            .copied()
            .collect();
        names.iter().find_map(|name| {
            ctx.available_champions().iter().copied().find(|id| {
                !taken.contains(id)
                    && ctx
                        .champion_name(*id)
                        .is_some_and(|n| n.eq_ignore_ascii_case(name))
            })
        })
    }
    fn decide_ban(&self, ctx: &StableDraftContext<'_>) -> Option<usize> {
        let names = lineup();
        if !active() || names.is_empty() {
            return None;
        }
        ctx.available_champions().iter().copied().find(|id| {
            !ctx.champion_name(*id)
                .is_some_and(|n| names.iter().any(|w| n.eq_ignore_ascii_case(w)))
        })
    }
}

// ---- The shot readout -------------------------------------------------------

/// One shot as the readout shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Shot {
    /// The skill whose cooldown restarted most recently, and how many ticks
    /// before the shot; None before the first cast.
    pub skill: Option<(&'static str, u64)>,
    pub producer: usize,
    /// True for an entity's action state, false for a projectile's kind.
    pub entity: bool,
    pub word: u64,
    /// The first hit test (shape A or B) that checked a target against it.
    pub hit_test: Option<HitTest>,
    pub last_tick: usize,
    pub seen: Instant,
}
const SHOTS_SHOWN: usize = 8;
/// Producer events of one shot keep coming while it flies; events of the
/// same producer and word this close together are the same shot.
const SAME_SHOT_TICKS: usize = 20;
static SHOTS: Mutex<VecDeque<Shot>> = Mutex::new(VecDeque::new());

/// Controlled player's tick: folds the hooks' events into the readout.
pub fn record(events: &[ShotEvent], tick: usize, skill: Option<(&'static str, u64)>) {
    if events.is_empty() {
        return;
    }
    let Ok(mut shots) = SHOTS.lock() else {
        return;
    };
    for event in events {
        let same = shots.iter_mut().find(|s| {
            s.producer == event.producer
                && s.word == event.word
                && tick.saturating_sub(s.last_tick) <= SAME_SHOT_TICKS
        });
        match same {
            Some(shot) => {
                shot.last_tick = tick;
                shot.hit_test = shot.hit_test.or(event.hit_test);
            }
            None => {
                shots.push_front(Shot {
                    skill,
                    producer: event.producer,
                    entity: event.entity,
                    word: event.word,
                    hit_test: event.hit_test,
                    last_tick: tick,
                    seen: Instant::now(),
                });
                shots.truncate(SHOTS_SHOWN);
            }
        }
    }
}

/// The readout's lines, newest first.
pub fn readout() -> Vec<String> {
    let Ok(shots) = SHOTS.lock() else {
        return Vec::new();
    };
    shots.iter().map(line).collect()
}

fn line(s: &Shot) -> String {
    let skill = s.skill.map_or("?".to_owned(), |(slot, ago)| {
        format!("{slot} ({ago} ticks before)")
    });
    let word = if s.entity {
        format!("state {}", s.word)
    } else {
        format!("kind {}", s.word)
    };
    let shape = match s.hit_test {
        Some(HitTest { shape, rva }) => format!("shape {shape} ({rva:#x})"),
        None if s.seen.elapsed().as_millis() < 700 => "...".to_owned(),
        None => "no hit test".to_owned(),
    };
    format!("{skill}  {word}  {shape}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_is_straight_line_in_internal_units() {
        assert_eq!(distance((0, 0), (3_000, 4_000)), 5_000);
        assert_eq!(distance((10, 10), (10, 10)), 0);
        assert_eq!(distance((5_000, 0), (2_000, 4_000)), 5_000);
    }

    #[test]
    fn the_lineup_is_lower_case_trimmed_and_without_repeats() {
        assert_eq!(
            parse_lineup(" Crossbowman, pyromancer ,,HARPY, crossbowman"),
            ["crossbowman", "pyromancer", "harpy"]
        );
        assert!(parse_lineup("").is_empty());
    }

    #[test]
    fn events_of_one_shot_merge_and_a_later_shot_is_new() {
        SHOTS.lock().unwrap().clear();
        let fired = ShotEvent {
            producer: 0x10,
            entity: false,
            word: 2,
            hit_test: None,
        };
        record(&[fired], 100, Some(("Q", 3)));
        record(&[fired], 104, Some(("Q", 7)));
        record(
            &[ShotEvent {
                hit_test: Some(HitTest {
                    shape: 'A',
                    rva: 0x99,
                }),
                ..fired
            }],
            105,
            Some(("Q", 8)),
        );
        {
            let shots = SHOTS.lock().unwrap();
            assert_eq!(shots.len(), 1);
            assert_eq!(
                shots[0].hit_test.map(|h| (h.shape, h.rva)),
                Some(('A', 0x99))
            );
            assert_eq!(shots[0].skill, Some(("Q", 3)));
        }
        record(&[fired], 200, Some(("W", 1)));
        assert_eq!(SHOTS.lock().unwrap().len(), 2);
        assert!(readout()[0].starts_with("W (1 ticks before)  kind 2"));
        SHOTS.lock().unwrap().clear();
    }
}
