//! Diagnostic-only basic-attack timing records. Never changes native state.
//! Static review shows two exclusive outcomes of the native attack consumer:
//! a delayed effect queued without an action lock, or action 3 with no queue
//! entry. These records classify each accepted attack and follow its native
//! action/queue transitions at the hook points that already observe the actor.
use crate::native_timing::MatchKey;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const ATTACKS_PER_MATCH: usize = 400;
const CHANGES_PER_ATTACK: usize = 12;
const TRACE_LIFETIME: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub action: usize,
    /// Action payload words +78/+80. 0.6.2's action-3 handler advanced +78;
    /// 0.6.3's tick code compares the scaled start timing with +80. Both are
    /// recorded so the test shows which one counts toward the hit.
    pub elapsed: usize,
    pub counter: usize,
    pub queue: usize,
    pub cooldown: usize,
}
impl Snapshot {
    fn locked(&self) -> Option<(usize, usize)> {
        (self.action == 3).then_some((self.elapsed, self.counter))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Start {
    pub before: Snapshot,
    pub after: Snapshot,
    /// Raw attack-effect fields: kind (+4b4), start timing (+4a8, present
    /// unless the +4b8 niche is -1), attack-speed bonus (+3f4).
    pub kind: u32,
    pub start_timing: Option<u64>,
    pub speed: i32,
    pub queued_delay: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Branch {
    /// Delayed effect queued; the action is not locked.
    Queued,
    /// Action 3 started; no queue entry from this call.
    Locked,
    Unclassified,
}

impl Start {
    pub fn branch(&self) -> Branch {
        let (b, a) = (self.before, self.after);
        if a.action == 3 && a.elapsed == 0 && a.queue == b.queue {
            Branch::Locked
        } else if a.action <= 2 && a.queue == b.queue + 1 {
            Branch::Queued
        } else {
            Branch::Unclassified
        }
    }
    /// The native queue path's delay formula, from the declared start timing:
    /// min(start * 100 / max(speed + 100, 1), cooldown - 1). For locked
    /// attacks this is a candidate hit tick only; it is not proven native.
    pub fn expected_hit(&self) -> Option<u64> {
        let start = self.start_timing?;
        let divisor = (self.speed as i64 + 100).max(1) as u64;
        let delay = start.checked_mul(100)? / divisor;
        Some(delay.min((self.after.cooldown as u64).saturating_sub(1)))
    }
}

struct Active {
    id: usize,
    branch: Branch,
    at: Instant,
    baseline_queue: usize,
    last: Snapshot,
    last_locked: Option<(usize, usize)>,
    changes: usize,
}

#[derive(Default)]
struct State {
    key: Option<MatchKey>,
    actor: Option<usize>,
    started: usize,
    active: Option<Active>,
}

pub struct AttackTrace(Mutex<State>);

impl AttackTrace {
    pub const fn new() -> Self {
        Self(Mutex::new(State {
            key: None,
            actor: None,
            started: 0,
            active: None,
        }))
    }

    fn rebind(s: &mut State, key: MatchKey, actor: usize) {
        if s.key != Some(key) || s.actor != Some(actor) {
            *s = State {
                key: Some(key),
                actor: Some(actor),
                ..State::default()
            };
        }
    }

    /// Returns log lines for an accepted attack (cooldown was spent).
    pub fn start(&self, key: MatchKey, actor: usize, source: &str, start: Start) -> Vec<String> {
        self.start_at(key, actor, source, start, Instant::now())
    }

    fn start_at(
        &self,
        key: MatchKey,
        actor: usize,
        source: &str,
        start: Start,
        now: Instant,
    ) -> Vec<String> {
        let Ok(mut s) = self.0.lock() else {
            return Vec::new();
        };
        Self::rebind(&mut s, key, actor);
        let mut lines = Vec::new();
        if let Some(previous) = s.active.take() {
            lines.push(end_line(actor, &previous, "next-attack", now));
        }
        if s.started >= ATTACKS_PER_MATCH {
            return lines;
        }
        s.started += 1;
        let id = s.started;
        let branch = start.branch();
        let (b, a) = (start.before, start.after);
        lines.push(format!(
            "ATTACK TRACE_START id={id} actor={actor} source={source} branch={branch:?} action={}->{} locked_payload={:?} queue={}->{} cooldown={}->{} kind={} start_timing={:?} speed_bonus={} expected_hit_tick={:?} queued_delay={:?}; diagnostic only",
            b.action, a.action, a.locked(), b.queue, a.queue, b.cooldown, a.cooldown,
            start.kind, start.start_timing, start.speed, start.expected_hit(), start.queued_delay,
        ));
        if s.started == ATTACKS_PER_MATCH {
            lines.push(format!("ATTACK TRACE_LIMIT actor={actor} attacks={ATTACKS_PER_MATCH}; later attacks in this match are not traced"));
        }
        s.active = Some(Active {
            id,
            branch,
            at: now,
            baseline_queue: b.queue,
            last: a,
            last_locked: a.locked(),
            changes: 0,
        });
        lines
    }

    /// Follow the active attack at any existing observation point.
    pub fn sample(&self, key: MatchKey, actor: usize, now_state: Snapshot) -> Vec<String> {
        self.sample_at(key, actor, now_state, Instant::now())
    }

    fn sample_at(&self, key: MatchKey, actor: usize, snap: Snapshot, now: Instant) -> Vec<String> {
        let Ok(mut s) = self.0.lock() else {
            return Vec::new();
        };
        if s.key != Some(key) || s.actor != Some(actor) {
            return Vec::new();
        }
        let Some(active) = s.active.as_mut() else {
            return Vec::new();
        };
        let mut lines = Vec::new();
        if let Some(locked) = snap.locked() {
            active.last_locked = Some(locked);
        }
        let changed = snap.action != active.last.action || snap.queue != active.last.queue;
        if changed && active.changes < CHANGES_PER_ATTACK {
            active.changes += 1;
            lines.push(format!(
                "ATTACK TRACE_CHANGE id={} actor={actor} ms={} action={}->{} queue={}->{} locked_payload={:?} last_locked_payload={:?} cooldown={}",
                active.id,
                now.duration_since(active.at).as_millis(),
                active.last.action, snap.action, active.last.queue, snap.queue,
                snap.locked(), active.last_locked, snap.cooldown,
            ));
        }
        active.last = snap;
        let settled = snap.action != 3 && snap.queue <= active.baseline_queue;
        let expired = now.duration_since(active.at) >= TRACE_LIFETIME;
        if settled || expired {
            let reason = if settled { "settled" } else { "expired" };
            let done = s.active.take().unwrap();
            lines.push(end_line(actor, &done, reason, now));
        }
        lines
    }
}

fn end_line(actor: usize, a: &Active, reason: &str, now: Instant) -> String {
    format!(
        "ATTACK TRACE_END id={} actor={actor} branch={:?} reason={reason} ms={} last_locked_payload={:?} final_action={} final_queue={}",
        a.id,
        a.branch,
        now.duration_since(a.at).as_millis(),
        a.last_locked,
        a.last.action,
        a.last.queue,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: MatchKey = (1, 2, 3);
    fn snap(action: usize, elapsed: usize, queue: usize, cooldown: usize) -> Snapshot {
        Snapshot {
            action,
            elapsed,
            counter: elapsed + 100,
            queue,
            cooldown,
        }
    }
    fn start(before: Snapshot, after: Snapshot) -> Start {
        Start {
            before,
            after,
            kind: 0,
            start_timing: Some(13),
            speed: 0,
            queued_delay: None,
        }
    }

    #[test]
    fn classifies_exclusive_native_outcomes() {
        assert_eq!(
            start(snap(0, 0, 0, 0), snap(3, 0, 0, 40)).branch(),
            Branch::Locked
        );
        assert_eq!(
            start(snap(2, 9, 1, 0), snap(2, 9, 2, 40)).branch(),
            Branch::Queued
        );
        // Both together never occurs natively; do not claim either.
        assert_eq!(
            start(snap(0, 0, 0, 0), snap(3, 0, 1, 40)).branch(),
            Branch::Unclassified
        );
    }

    #[test]
    fn expected_hit_uses_native_speed_scaling_and_cooldown_cap() {
        let mut s = start(snap(0, 0, 0, 0), snap(3, 0, 0, 40));
        assert_eq!(s.expected_hit(), Some(13));
        s.speed = 30; // 13 * 100 / 130
        assert_eq!(s.expected_hit(), Some(10));
        s.after.cooldown = 5;
        s.speed = 0;
        assert_eq!(s.expected_hit(), Some(4));
        s.start_timing = None;
        assert_eq!(s.expected_hit(), None);
    }

    #[test]
    fn follows_locked_attack_until_action_clears() {
        let t = AttackTrace::new();
        let at = Instant::now();
        let lines = t.start_at(
            KEY,
            7,
            "input",
            start(snap(0, 0, 0, 0), snap(3, 0, 0, 40)),
            at,
        );
        assert!(lines[0].contains("branch=Locked"));
        assert!(t.sample_at(KEY, 7, snap(3, 5, 0, 35), at).is_empty());
        let lines = t.sample_at(KEY, 7, snap(0, 0, 0, 20), at + Duration::from_millis(330));
        assert!(
            lines[0].contains("action=3->0")
                && lines[0].contains("last_locked_payload=Some((5, 105))")
        );
        assert!(lines[1].contains("TRACE_END") && lines[1].contains("reason=settled"));
        assert!(t.sample_at(KEY, 7, snap(0, 0, 0, 0), at).is_empty());
    }

    #[test]
    fn follows_queued_attack_until_effect_leaves_queue() {
        let t = AttackTrace::new();
        let at = Instant::now();
        t.start_at(
            KEY,
            7,
            "native-auto",
            start(snap(2, 0, 0, 0), snap(2, 0, 1, 40)),
            at,
        );
        let lines = t.sample_at(KEY, 7, snap(2, 0, 0, 30), at + Duration::from_millis(160));
        assert!(lines[0].contains("queue=1->0"));
        assert!(lines[1].contains("reason=settled"));
    }

    #[test]
    fn ignores_other_actors_and_resets_per_match() {
        let t = AttackTrace::new();
        let at = Instant::now();
        t.start_at(
            KEY,
            7,
            "input",
            start(snap(0, 0, 0, 0), snap(3, 0, 0, 40)),
            at,
        );
        assert!(t.sample_at(KEY, 8, snap(0, 0, 0, 0), at).is_empty());
        let lines = t.start_at(
            (9, 9, 9),
            7,
            "input",
            start(snap(0, 0, 0, 0), snap(3, 0, 0, 40)),
            at,
        );
        assert!(
            lines[0].contains("id=1"),
            "new match restarts numbering: {lines:?}"
        );
    }

    #[test]
    fn bounds_records_and_expires() {
        let t = AttackTrace::new();
        let at = Instant::now();
        for i in 0..ATTACKS_PER_MATCH + 5 {
            t.start_at(
                KEY,
                7,
                "input",
                start(snap(0, 0, 0, 0), snap(3, 0, 0, 40)),
                at + Duration::from_millis(i as u64),
            );
        }
        assert_eq!(t.0.lock().unwrap().started, ATTACKS_PER_MATCH);
        let t = AttackTrace::new();
        t.start_at(
            KEY,
            7,
            "input",
            start(snap(0, 0, 0, 0), snap(3, 0, 0, 40)),
            at,
        );
        let lines = t.sample_at(KEY, 7, snap(3, 9, 0, 0), at + TRACE_LIFETIME);
        assert!(lines.iter().any(|l| l.contains("reason=expired")));
    }
}
