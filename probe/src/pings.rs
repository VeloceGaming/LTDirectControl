//! Pings (stage 1 since 1.7.1-dev13): the ping key enters ping mode and the mouse tells the
//! teammates' AI something about what is under the cursor. The game's own
//! levers carry it (docs/investigation-ai-signals.md sections 10 to 12,
//! docs/investigation-team-ai.md sections 12 to 26): the fight call, the
//! objective setter, the fog ping's vision write, the driven windows (the
//! plan's own flags, 1.7.1-dev17) and the team strategy. The spec is
//! design/pings-spec-stage1.md.
//!
//! The parts:
//! - `pings/wheel.rs`: the input on the client thread (the key enters ping
//!   mode; a left click pings that unit, a left drag opens the wheel and
//!   its release fires), the place or unit, and the window.
//! - `pings/gesture.rs`: the two-level marking-menu gesture.
//! - `pings/intent.rs`: "here" (the wheel's place) and "that" (the
//!   clicked unit), the eight slots and their orders.
//! - `plan_call.rs` (crate root): Poke and All-in, the team strategy for a
//!   window through the official mod interface.
//! - `pings/places.rs`: where the lanes, pits and bases are.
//! - `pings/wheel_draw.rs`: the wheel; `pings/mark.rs`: the pin and its
//!   preview; `pings/stack.rs`: the answer stack.
//! - the native half: `native_adapter::ping_orders` (the orders, written at
//!   each teammate's own AI step) and `native_adapter::signal_call` (the
//!   fight call and the mapping this file's roster feeds).
//! - `pings/battle_help.rs` (research builds): the watch and the lever
//!   keys' choice marks.
//!
//! Superseded 2026-11-04 (1.7.1-dev20, the author's spec section 13.1):
//! the research dev feedback panel (`pings/dev_panel.rs`, 1.7.1-dev12) is
//! removed; its lines' sources stay in `probe.log`.
//!
//! Superseded 2026-11-04 (1.7.1-dev13): stage 1's four calls (help,
//! regroup, push, back off, 1.7.1-dev1) and the live ping's card and ground
//! rings are gone; the wheel's slots are the spec's intents.
#[cfg(feature = "research")]
pub mod battle_help;
mod gesture;
mod intent;
mod mark;
pub(crate) mod places;
mod stack;
mod subject;
pub(crate) mod walk;
mod wheel;
mod wheel_draw;

pub use intent::PlanCall;
pub use wheel::{armed_cursor, left_reserved, wheel_open, Pings, MOVE_PX, PATH};

use crate::native_adapter::signal_call::{self as call, RosterPlayer};
use crate::Logger;
use mod_api_stable::StableSim;

/// One second of simulation.
pub const SECOND: usize = 60;
/// The team's sight is read this often (ticks).
const SIGHT_EVERY: usize = 6;
/// What the controlled athlete's team sees, and when that was read
/// (1.7.1-dev15: a target ping needs the team's sight, whatever the
/// player's own vision choice shows; 1.7.1-dev16: the whole list, the
/// pings' targets also while the controlled champion is dead and the
/// hover list is empty).
static TEAM_UNITS: std::sync::Mutex<Option<(std::time::Instant, Vec<crate::combat::Unit>)>> =
    std::sync::Mutex::new(None);
/// The units the team sees (read within the last second; else none).
pub fn team_units() -> Vec<crate::combat::Unit> {
    TEAM_UNITS
        .lock()
        .ok()
        .and_then(|s| {
            s.as_ref()
                .filter(|(at, _)| {
                    crate::clock::now().saturating_duration_since(*at)
                        <= std::time::Duration::from_secs(1)
                })
                .map(|(_, units)| units.clone())
        })
        .unwrap_or_default()
}
/// The team sees unit `id`.
pub fn team_sees(id: usize) -> bool {
    team_units().iter().any(|u| u.id == id)
}

/// From the viewed match's simulation step (any living actor's since
/// 1.7.1-dev16), once a tick: every 6 ticks the team's sight; once a
/// second, the roster for the native mapping (every athlete's SDK player
/// id, side, lane and champion entity id; logged when it changes) and the
/// pits' positions (`places::learn`). Read only.
pub fn observe(sim: &StableSim<'_>, tick: usize, controlled: usize, logger: &Logger) {
    // Every living actor's step calls this (1.7.1-dev16): once a tick.
    static LAST: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(usize::MAX);
    if LAST.swap(tick, std::sync::atomic::Ordering::Relaxed) == tick {
        return;
    }
    if tick.is_multiple_of(SIGHT_EVERY) {
        let side = (0..sim.player_count())
            .filter_map(|i| sim.player_at(i))
            .find(|p| p.id() == controlled)
            .map(|p| p.team());
        if let Some(side) = side {
            // Each teammate's hp percent, for the walk's low rule
            // (1.7.1-dev23, spec section 15.6).
            for p in (0..sim.player_count()).filter_map(|i| sim.player_at(i)) {
                let alive = p.champion().filter(|c| c.is_alive());
                if let (true, Some(c)) = (p.team() == side, alive) {
                    let (hp, max) = c.hp();
                    if max > 0 {
                        walk::record(p.id(), tick, hp as f32 * 100. / max as f32);
                    }
                }
            }
            let units = crate::simulation::team_units(sim, side);
            if let Ok(mut seen) = TEAM_UNITS.lock() {
                *seen = Some((crate::clock::now(), units));
            }
        }
    }
    if !tick.is_multiple_of(SECOND) {
        return;
    }
    let roster: Vec<RosterPlayer> = (0..sim.player_count())
        .filter_map(|i| {
            let p = sim.player_at(i)?;
            Some(RosterPlayer {
                player: p.id(),
                side: p.team(),
                lane: p.lane().map(|l| l.code()),
                entity: p.champion()?.id(),
                controlled: p.id() == controlled,
            })
        })
        .collect();
    if !roster.is_empty() && call::publish_roster(roster.clone()) {
        for p in &roster {
            logger.write(&format!(
                "PING map roster player={} side={} lane={} entity={} controlled={}",
                p.player,
                p.side,
                p.lane.map_or("none".to_owned(), |l| l.to_string()),
                p.entity,
                if p.controlled { "yes" } else { "no" }
            ));
        }
    }
    for index in 0..sim.entity_count() {
        let Some(unit) = sim.entity_at(index) else {
            continue;
        };
        if unit.is_champion() || unit.is_minion() || unit.is_tower() {
            continue;
        }
        let Some(tag) = unit.name().as_deref().and_then(places::pit_tag) else {
            continue;
        };
        let at = unit.pos();
        if places::learn(tag, at, unit.id(), unit.is_alive()) {
            logger.write(&format!(
                "PING pit learned {} at={},{}",
                if tag == 0 { "Morgard" } else { "Serpen" },
                at.0,
                at.1
            ));
        }
    }
}
