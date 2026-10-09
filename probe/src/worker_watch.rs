//! Worker stall report. The match worker marks each step of the mod's
//! per-tick work; when a running match publishes no frame for 3 s,
//! crate::native_timing logs `WORKER STALL` with the last step entered (and
//! `WORKER RESUMED` if it continues). Both are always written
//! (crate::logging::SAFETY). A step of `Outside` means the worker was not in
//! the mod's per-tick code: the game, or a native hook.
use std::sync::atomic::{AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::time::Duration;

/// No published frame for this long while running is a stall.
pub const STALL: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum Step {
    Outside,
    SessionGate,
    Bind,
    Roster,
    Team,
    Purchases,
    Shop,
    StatsPanel,
    Hud,
    Controlled,
    Units,
    AttackRange,
    Combat,
    Items,
    Abilities,
    Dispatch,
    Publication,
}
const STEPS: [Step; 17] = {
    use Step::*;
    [
        Outside,
        SessionGate,
        Bind,
        Roster,
        Team,
        Purchases,
        Shop,
        StatsPanel,
        Hud,
        Controlled,
        Units,
        AttackRange,
        Combat,
        Items,
        Abilities,
        Dispatch,
        Publication,
    ]
};
static STEP: AtomicU8 = AtomicU8::new(0);
static PLAYER: AtomicUsize = AtomicUsize::new(usize::MAX);
/// The viewed match's worker thread; other matches' workers are not marked.
static WORKER: AtomicU64 = AtomicU64::new(0);

/// The session's worker thread (0 when unbound).
pub fn bind(thread: u64) {
    WORKER.store(thread, Ordering::Relaxed);
}
/// Worker: entering `step` for `player` (usize::MAX when not per player).
pub fn enter(step: Step, player: usize) {
    if WORKER.load(Ordering::Relaxed) != crate::platform_input::thread_id() {
        return;
    }
    PLAYER.store(player, Ordering::Relaxed);
    STEP.store(step as u8, Ordering::Relaxed);
}
/// Worker: the start of a player's per-tick work (waiting for the session
/// lock); the returned guard marks `Outside` when that work returns.
pub fn begin(player: usize) -> Outside {
    enter(Step::SessionGate, player);
    Outside
}
pub struct Outside;
impl Drop for Outside {
    fn drop(&mut self) {
        enter(Step::Outside, usize::MAX);
    }
}
/// The last step entered, and its player.
pub fn last() -> (Step, Option<usize>) {
    let step = STEPS
        .get(STEP.load(Ordering::Relaxed) as usize)
        .copied()
        .unwrap_or(Step::Outside);
    let player = Some(PLAYER.load(Ordering::Relaxed)).filter(|p| *p != usize::MAX);
    (step, player)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_step_reads_back_as_itself() {
        for (i, step) in STEPS.iter().enumerate() {
            assert_eq!(*step as usize, i);
        }
    }
}
