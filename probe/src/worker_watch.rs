//! Worker stall report. The match worker marks each step of the mod's
//! per-tick work; when a running match publishes no frame for 3 s,
//! crate::native_timing logs `WORKER STALL` with the last step entered (and
//! `WORKER RESUMED` if it continues). Both are always written
//! (crate::logging::SAFETY). A step of `Outside` means the worker was in
//! none of the mod's code. A native hook's step (`Send` to `Shop`) covers the
//! hook and the game function it forwards to. The stall line also carries
//! the worker's call stack (crate::native_adapter::thread_stack).
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
    // Native hooks on the worker (crate::native_adapter).
    Send,
    Steer,
    Move,
    Attack,
    Skill,
    Aim,
    ShopHook,
}
const STEPS: [Step; 24] = {
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
        Send,
        Steer,
        Move,
        Attack,
        Skill,
        Aim,
        ShopHook,
    ]
};
static STEP: AtomicU8 = AtomicU8::new(0);
static PLAYER: AtomicUsize = AtomicUsize::new(usize::MAX);
/// Players whose per-tick work began since the last published frame, and
/// the last of them.
static BEGUN: AtomicUsize = AtomicUsize::new(0);
static LAST_PLAYER: AtomicUsize = AtomicUsize::new(usize::MAX);
/// The viewed match's worker thread; other matches' workers are not marked.
static WORKER: AtomicU64 = AtomicU64::new(0);

/// The session's worker thread (0 when unbound).
pub fn bind(thread: u64) {
    WORKER.store(thread, Ordering::Relaxed);
}
fn on_worker() -> bool {
    WORKER.load(Ordering::Relaxed) == crate::platform_input::thread_id()
}
/// Worker: entering `step` for `player` (usize::MAX when not per player).
pub fn enter(step: Step, player: usize) {
    if !on_worker() {
        return;
    }
    PLAYER.store(player, Ordering::Relaxed);
    STEP.store(step as u8, Ordering::Relaxed);
}
/// Worker: the start of a player's per-tick work (waiting for the session
/// lock); the returned guard marks `Outside` when that work returns.
pub fn begin(player: usize) -> Outside {
    if on_worker() {
        BEGUN.fetch_add(1, Ordering::Relaxed);
        LAST_PLAYER.store(player, Ordering::Relaxed);
    }
    enter(Step::SessionGate, player);
    Outside
}
/// Worker: a frame was published; the next tick's count starts over.
pub fn published() {
    if on_worker() {
        BEGUN.store(0, Ordering::Relaxed);
    }
    enter(Step::Publication, usize::MAX);
}
/// Worker: inside a native hook (and the game function it forwards to); the
/// returned guard puts back the step that was current.
pub fn hook(step: Step) -> Restore {
    if !on_worker() {
        return Restore(None);
    }
    let previous = (STEP.load(Ordering::Relaxed), PLAYER.load(Ordering::Relaxed));
    STEP.store(step as u8, Ordering::Relaxed);
    Restore(Some(previous))
}
pub struct Restore(Option<(u8, usize)>);
impl Drop for Restore {
    fn drop(&mut self) {
        if let Some((step, player)) = self.0 {
            PLAYER.store(player, Ordering::Relaxed);
            STEP.store(step, Ordering::Relaxed);
        }
    }
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
/// Players begun since the last published frame, and the last one begun.
pub fn progress() -> (usize, Option<usize>) {
    let last = Some(LAST_PLAYER.load(Ordering::Relaxed)).filter(|p| *p != usize::MAX);
    (BEGUN.load(Ordering::Relaxed), last)
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
