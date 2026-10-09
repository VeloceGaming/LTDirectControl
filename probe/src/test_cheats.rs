//! Testing aid: Home+End toggles "no cooldowns" for your own champion in the
//! viewed match. Off at every start and never saved. While on, a named
//! permanent buff is kept on the champion by the match hook below; turning
//! it off removes it. It changes the real match, so a tag is drawn on screen
//! and each toggle is logged.
//!
//! The game's own items reduce cooldowns with positive values (+10, +15);
//! 0.74.1's -100 froze the cooldown timer and the cast. The formula is
//! haste-style, cooldown x 100 / (100 + value) (0.74.2 test: +99 halved
//! them), so +900 leaves a tenth. Each cast's remaining cooldown is logged.
use crate::{movement::Movement, Logger};
use mod_api_stable::{BuffV1, StableMatchHook, StableSim};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

const BUFF: &str = "lt_test_no_cooldowns";
/// Cooldown reduction value (game convention: positive is shorter).
const REDUCTION: i32 = 900;
static ON: AtomicBool = AtomicBool::new(false);
static HELD: AtomicBool = AtomicBool::new(false);

pub fn active() -> bool {
    ON.load(Ordering::Relaxed)
}
/// Client thread: a fresh Home+End press toggles the option.
pub fn update(home_end: bool, log: &Logger) {
    if home_end && !HELD.swap(true, Ordering::Relaxed) {
        let on = !ON.fetch_xor(true, Ordering::Relaxed);
        log.write(&format!(
            "TEST no cooldowns {} (Home+End)",
            if on { "ON" } else { "OFF" }
        ));
    } else if !home_end {
        HELD.store(false, Ordering::Relaxed);
    }
}

/// Keeps the buff on the viewed match's controlled champion while the option
/// is on, and removes it once when it is turned off. Other matches (background
/// simulations) are never touched.
pub struct CooldownHook {
    pub movement: Arc<Movement>,
    pub logger: Arc<Logger>,
    /// (match, champion entity) currently carrying the buff.
    pub applied: Mutex<Option<(crate::native_timing::MatchKey, usize)>>,
    /// Last seen remaining cooldowns (Q, W, R) of the champion, in ticks.
    pub cooldowns: Mutex<[usize; 3]>,
}
impl StableMatchHook for CooldownHook {
    fn on_match_tick(&self, sim: &mut StableSim<'_>, _rng_seed: u64) {
        let on = active();
        let Ok(mut applied) = self.applied.lock() else {
            return;
        };
        if !on && applied.is_none() {
            return;
        }
        let Some(origin) = sim.sim_origin().filter(|o| o.kind == 2) else {
            return;
        };
        let key = (sim.seed(), origin.match_id, origin.set_index);
        if !on {
            if let Some((k, entity)) = *applied {
                if k == key {
                    sim.entity_remove_buff(entity, BUFF);
                    *applied = None;
                }
            }
            return;
        }
        let Some((viewed, player)) = self.movement.hud_identity() else {
            return;
        };
        if viewed != key {
            return;
        }
        let Some((entity, cooldowns)) = sim.get_player(player).and_then(|p| {
            let entity = p.champion()?.id();
            Some((entity, p.cooldowns()))
        }) else {
            return;
        };
        // A cast restarts a cooldown: log the new remaining ticks.
        if let (Some((_, q, w, r)), Ok(mut last)) = (cooldowns, self.cooldowns.lock()) {
            for (i, (now, slot)) in [(q, "Q"), (w, "W"), (r, "R")].into_iter().enumerate() {
                if now > last[i] {
                    self.logger.write(&format!(
                        "TEST no cooldowns cast slot={slot} remaining_ticks={now} reduction={REDUCTION}"
                    ));
                }
                last[i] = now;
            }
        }
        let mut buff = BuffV1 {
            skill_cooldown_mult: REDUCTION,
            ult_cooldown_mult: REDUCTION,
            ..BuffV1::default()
        };
        buff.set_name(BUFF);
        // At most one copy: this re-applies after respawns or removals.
        if sim.entity_stack_buff(entity, &buff, 1, false) > 0 && *applied != Some((key, entity)) {
            if let Some((k, old)) = applied.replace((key, entity)) {
                if k == key && old != entity {
                    sim.entity_remove_buff(old, BUFF);
                }
            }
            self.logger
                .write(&format!("TEST no cooldowns buff applied entity={entity}"));
        }
    }
}
