//! Research builds only (1.7.1-dev5): the simulation side of the one call
//! the mod sends (a BattleHelp tap; see `native_adapter::signal_call`).
//! Read-only, from the controlled athlete's own simulation step on the
//! viewed match's worker:
//!
//! - (until 1.7.1-dev12 the roster too; `pings::observe` since dev13);
//! - the watch: for 10 s after the first entry was written, once a second,
//!   each teammate's distance to the target and whether it shrank
//!   (`PING watch`). Nothing here moves anyone.
//!
//! Superseded 2026-11-04 (1.7.1-dev21, spec section 14.2): the lever
//! keys' choice marks (1.7.1-dev11), with the keys.
//!
//! Superseded 2026-11-04 (1.7.1-dev18, spec section 11.2): the fight
//! call's reply marks (dev8; larger and logged as `PING reply` since dev9)
//! retired into the answers over the champions every order shows, in both
//! builds (`stack::Stack::heads`, `mark::heads`): one mechanism, one look.
use crate::native_adapter::signal_call as call;
use crate::Logger;
use mod_api_stable::StableSim;
use std::sync::Mutex;

/// The watch's last line: (request, tick, each teammate's last distance by
/// SDK player id).
type Watched = (u64, u64, Vec<(usize, u64)>);
static WATCHED: Mutex<Option<Watched>> = Mutex::new(None);

/// One call per tick from the controlled athlete's step (`selected`), after
/// `pings::observe` (which publishes the roster since 1.7.1-dev13).
pub fn observe(sim: &StableSim<'_>, tick: usize, controlled: usize, logger: &Logger) {
    watch(sim, tick as u64, controlled, logger);
}

/// Distance between two world points, in world units.
fn distance(a: (u64, u64), b: (u64, u64)) -> u64 {
    let x = a.0.abs_diff(b.0) as f64;
    let y = a.1.abs_diff(b.1) as f64;
    x.hypot(y) as u64
}

fn watch(sim: &StableSim<'_>, tick: u64, controlled: usize, logger: &Logger) {
    let Some((serial, target, sent)) = call::watch(tick) else {
        return;
    };
    let Ok(mut watched) = WATCHED.lock() else {
        return;
    };
    if watched
        .as_ref()
        .is_some_and(|w| w.0 == serial && tick < w.1 + call::WATCH_EVERY)
    {
        return;
    }
    let previous = watched
        .take()
        .filter(|w| w.0 == serial)
        .map(|w| w.2)
        .unwrap_or_default();
    let target_pos = (0..sim.entity_count())
        .filter_map(|i| sim.entity_at(i))
        .find(|e| e.id() == target)
        .filter(|e| e.is_alive())
        .map(|e| e.pos());
    let Some(target_pos) = target_pos else {
        logger.write(&format!(
            "PING watch t={tick} target={target} not alive or not found (request {serial})"
        ));
        *watched = Some((serial, tick, previous));
        return;
    };
    let side = sim.get_player(controlled).map(|p| p.team());
    let mut now = Vec::new();
    for p in (0..sim.player_count()).filter_map(|i| sim.player_at(i)) {
        if Some(p.team()) != side || p.id() == controlled {
            continue;
        }
        let Some(champion) = p.champion().filter(|c| c.is_alive()) else {
            logger.write(&format!("PING watch t={tick} player={} dead", p.id()));
            continue;
        };
        let dist = distance(champion.pos(), target_pos);
        let closing = match previous.iter().find(|d| d.0 == p.id()) {
            Some(&(_, before)) if dist < before => "yes",
            Some(_) => "no",
            None => "first",
        };
        let index =
            call::brain_of(champion.id()).map_or("?".to_owned(), |(_, index)| index.to_string());
        logger.write(&format!(
            "PING watch t={tick} p{index} player={} dist={dist} closing={closing} since_send={}",
            p.id(),
            tick - sent
        ));
        now.push((p.id(), dist));
    }
    *watched = Some((serial, tick, now));
}
