//! The team strategy windows (1.7.1-dev17, the author's spec section 10.2;
//! the finishing rule since 1.7.1-dev18, section 11.5): the controlled
//! athlete's team strategy for a while, through the official mod interface
//! (`strategy_get_json`, `strategy_set_json`; no game address). Seen in
//! game in research builds since 1.7.1-dev9: a field set by its bare name
//! with a bare JSON string is accepted at once and kept; the game puts the
//! manager's strategy back when the match ends.
//!
//! - Poke: `object_battle` and `tower_press` Poking.
//! - All-in: `object_battle` Initiating and `object_buildup` Gather.
//! - Take here at a pit (and the click on a pit monster): `object_finish`
//!   KillPriority; Defend here at a pit: `object_finish` BattlePriority.
//!
//! Each call is a window of [`WINDOW`]. At its end the fields it changed
//! get back the values the viewed match started with (read at its first
//! tick). A later call replaces a running one that sets any of the same
//! fields (the earlier's other fields put back first); calls on different
//! fields run side by side (a Take at a pit leaves a running All-in alone).
//! A released control ends them all at the next tick.
//!
//! Threads: the client leaves the requests ([`request`], [`end`]); the
//! match hook, which holds the changeable simulation, performs them
//! ([`observe`]). The SDK honours changes there, never inside the AI
//! step. Short locks.
//!
//! Sent only where the pings send their other orders (0.7.0; on 0.6.5 the
//! slot marks the place, `intent::wheel`, and a pit order sets no rule,
//! `intent::finish`).
//!
//! The game was not seen to read `object_battle` directly
//! (docs/investigation-team-ai.md section 7: no direct byte read found), so
//! how much Poking and Initiating change is for play to show.
//! `object_finish` is read by the fight plan's "objective priority" exit
//! (section 17, exit 4, through `0x10b0e30`; read from the code).
use crate::native_adapter::ping_orders::Status;
use crate::native_timing::MatchKey;
use crate::pings::PlanCall;
use crate::{movement::Movement, Logger};
use mod_api_stable::StableSim;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// The window: 20 s, to tune in play (no game constant names a teamfight's
/// length; 20 s covers the approach and the fight at the pit).
pub const WINDOW: usize = 20 * crate::pings::SECOND;
/// The fields a call sets, with their values (the interface's spelling;
/// the finishing values as a logged strategy spells them,
/// docs/investigation-ai-signals.md).
pub fn fields(call: PlanCall) -> &'static [(&'static str, &'static str)] {
    match call {
        PlanCall::Poke => &[("object_battle", "Poking"), ("tower_press", "Poking")],
        PlanCall::AllIn => &[
            ("object_battle", "Initiating"),
            ("object_buildup", "Gather"),
        ],
        PlanCall::KillFirst => &[("object_finish", "KillPriority")],
        PlanCall::FightFirst => &[("object_finish", "BattlePriority")],
    }
}
/// Every field a call can change: saved at the match's first tick.
const SAVED: [&str; 4] = [
    "object_battle",
    "tower_press",
    "object_buildup",
    "object_finish",
];
/// Requests and answers kept at most (a few calls per second at most).
const KEPT: usize = 8;

/// What the client asks the match hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ask {
    Call(u64, PlanCall),
    End,
}
/// The requests not yet performed, oldest first.
static ASK: Mutex<Vec<Ask>> = Mutex::new(Vec::new());
static SERIAL: AtomicU64 = AtomicU64::new(0);
/// The latest calls' serials and whether their set was taken (None: not
/// yet).
static TAKEN: Mutex<Vec<(u64, Option<bool>)>> = Mutex::new(Vec::new());

fn push<T>(list: &Mutex<Vec<T>>, item: T) {
    if let Ok(mut l) = list.lock() {
        if l.len() >= KEPT {
            l.remove(0);
        }
        l.push(item);
    }
}
/// Client: a call; its serial.
pub fn request(call: PlanCall) -> u64 {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed) + 1;
    push(&ASK, Ask::Call(serial, call));
    push(&TAKEN, (serial, None));
    serial
}
/// Client: end every running window (control released).
pub fn end() {
    push(&ASK, Ask::End);
}
/// Client: call `serial`'s rows for the stack, one per teammate in `to`:
/// pending until set, then accepted (the team took it) or declined.
pub fn answers(serial: u64, to: [bool; 5]) -> [Option<Status>; 5] {
    let taken = TAKEN
        .lock()
        .ok()
        .and_then(|t| t.iter().find(|(n, _)| *n == serial).map(|(_, ok)| *ok));
    let status = match taken {
        Some(None) => Status::Pending,
        Some(Some(true)) => Status::Accepted,
        _ => Status::Declined,
    };
    std::array::from_fn(|i| to[i].then_some(status))
}

/// A running window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Running {
    serial: u64,
    call: PlanCall,
    until: usize,
}
/// Two calls share a field: the later replaces the earlier.
fn overlaps(a: PlanCall, b: PlanCall) -> bool {
    fields(a)
        .iter()
        .any(|(f, _)| fields(b).iter().any(|(g, _)| f == g))
}
/// What the hook does at one tick, from the requests and the running
/// windows: the windows to end (each with the fields a new call sets next,
/// so not put back), and the calls to set. Pure.
/// The windows a tick ends, each with the fields not to put back.
type Ended = Vec<(Running, Vec<&'static str>)>;
fn plan(running: &[Running], asks: &[Ask], tick: usize) -> (Ended, Vec<(u64, PlanCall)>) {
    let mut live: Vec<Running> = running.to_vec();
    let mut ended = Vec::new();
    let mut calls = Vec::new();
    for r in live.iter().filter(|r| tick >= r.until) {
        ended.push((*r, Vec::new()));
    }
    live.retain(|r| tick < r.until);
    // A window replaced before it was set (same tick) is only dropped.
    let mut end = |r: Running, keep: Vec<&'static str>, calls: &mut Vec<(u64, PlanCall)>| {
        if calls.iter().any(|(n, _)| *n == r.serial) {
            calls.retain(|(n, _)| *n != r.serial);
        } else {
            ended.push((r, keep));
        }
    };
    for ask in asks {
        match *ask {
            Ask::End => {
                for r in live.drain(..) {
                    end(r, Vec::new(), &mut calls);
                }
            }
            Ask::Call(serial, call) => {
                let keep: Vec<&'static str> = fields(call).iter().map(|(f, _)| *f).collect();
                let (gone, kept): (Vec<_>, Vec<_>) =
                    live.into_iter().partition(|r| overlaps(r.call, call));
                live = kept;
                for r in gone {
                    end(r, keep.clone(), &mut calls);
                }
                calls.push((serial, call));
                live.push(Running {
                    serial,
                    call,
                    until: tick + WINDOW,
                });
            }
        }
    }
    (ended, calls)
}

struct Match {
    key: MatchKey,
    team: usize,
    /// Each saved field's JSON at the match's first tick.
    saved: [Option<String>; 4],
    running: Vec<Running>,
}
static MATCH: Mutex<Option<Match>> = Mutex::new(None);

/// The match hook, every tick of every match: only the viewed match.
pub fn observe(sim: &mut StableSim<'_>, movement: &Movement, log: &Logger) {
    let Some(origin) = sim.sim_origin().filter(|o| o.kind == 2) else {
        return;
    };
    let key = (sim.seed(), origin.match_id, origin.set_index);
    let Some((viewed, player)) = movement.hud_identity() else {
        return;
    };
    if viewed != key {
        return;
    }
    let Ok(mut slot) = MATCH.lock() else {
        return;
    };
    if slot.as_ref().is_none_or(|m| m.key != key) {
        let Some(team) = sim.get_player(player).map(|p| p.team()) else {
            return;
        };
        // A new match: its own strategy; a request left before it is old.
        if let Ok(mut a) = ASK.lock() {
            a.clear();
        }
        let saved = SAVED.map(|f| sim.strategy_get_json(team, f));
        *slot = Some(Match {
            key,
            team,
            saved,
            running: Vec::new(),
        });
    }
    let Some(m) = slot.as_mut() else {
        return;
    };
    let tick = sim.tick();
    let asks = ASK
        .lock()
        .map(|mut a| std::mem::take(&mut *a))
        .unwrap_or_default();
    if asks.is_empty() && m.running.iter().all(|r| tick < r.until) {
        return;
    }
    let (ended, calls) = plan(&m.running, &asks, tick);
    for (r, keep) in &ended {
        restore(sim, m, r, keep, log);
    }
    m.running
        .retain(|r| !ended.iter().any(|(e, _)| e.serial == r.serial));
    for (serial, call) in calls {
        let mut ok = true;
        for (field, value) in fields(call) {
            let taken = sim.strategy_set_json(m.team, field, &format!("\"{value}\""));
            ok &= taken;
            log.write(&format!(
                "PING plan n={serial} team={} {field}={value} ok={taken} t={tick}",
                m.team
            ));
        }
        if let Ok(mut t) = TAKEN.lock() {
            if let Some(entry) = t.iter_mut().find(|(n, _)| *n == serial) {
                entry.1 = Some(ok);
            }
        }
        let r = Running {
            serial,
            call,
            until: tick + WINDOW,
        };
        if ok {
            m.running.push(r);
        } else {
            // A field refused: the others back at once.
            restore(sim, m, &r, &[], log);
        }
    }
}
/// Window `r`'s fields back to the match's start values, except those in
/// `keep` (a new call sets them next).
fn restore(sim: &mut StableSim<'_>, m: &Match, r: &Running, keep: &[&str], log: &Logger) {
    for (field, _) in fields(r.call) {
        if keep.contains(field) {
            continue;
        }
        let at = SAVED.iter().position(|f| f == field);
        let ok = match at.and_then(|i| m.saved[i].as_deref()) {
            Some(json) => sim.strategy_set_json(m.team, field, json),
            None => false,
        };
        log.write(&format!(
            "PING plan ended n={} team={} {field} restored={ok}",
            r.serial, m.team
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_call_sets_its_fields_and_every_field_is_saved() {
        for call in [
            PlanCall::Poke,
            PlanCall::AllIn,
            PlanCall::KillFirst,
            PlanCall::FightFirst,
        ] {
            for (field, _) in fields(call) {
                assert!(SAVED.contains(field), "{field}");
            }
        }
        assert_eq!(fields(PlanCall::Poke)[1], ("tower_press", "Poking"));
        assert_eq!(fields(PlanCall::AllIn)[0], ("object_battle", "Initiating"));
        assert_eq!(
            fields(PlanCall::KillFirst),
            &[("object_finish", "KillPriority")]
        );
        assert_eq!(
            fields(PlanCall::FightFirst),
            &[("object_finish", "BattlePriority")]
        );
        assert_eq!(WINDOW, 1200);
    }
    #[test]
    fn a_call_replaces_only_the_windows_on_its_own_fields() {
        let run = |serial, call, until| Running {
            serial,
            call,
            until,
        };
        // All-in running; Take at a pit: both run, nothing put back.
        let running = [run(1, PlanCall::AllIn, 1300)];
        let (ended, calls) = plan(&running, &[Ask::Call(2, PlanCall::KillFirst)], 200);
        assert!(ended.is_empty());
        assert_eq!(calls, [(2, PlanCall::KillFirst)]);
        // Poke over All-in: All-in ends, object_battle not put back (Poke
        // sets it next), object_buildup put back.
        let (ended, calls) = plan(&running, &[Ask::Call(3, PlanCall::Poke)], 200);
        assert_eq!(ended, [(running[0], vec!["object_battle", "tower_press"])]);
        assert_eq!(calls, [(3, PlanCall::Poke)]);
        // Defend over Take at the same tick: only Defend is set.
        let asks = [
            Ask::Call(4, PlanCall::KillFirst),
            Ask::Call(5, PlanCall::FightFirst),
        ];
        assert_eq!(plan(&[], &asks, 10).1, [(5, PlanCall::FightFirst)]);
        // The end of a window, and a released control.
        let both = [
            run(1, PlanCall::AllIn, 1300),
            run(2, PlanCall::KillFirst, 1400),
        ];
        let (ended, _) = plan(&both, &[], 1300);
        assert_eq!(ended, [(both[0], vec![])]);
        let (ended, calls) = plan(&both, &[Ask::End], 500);
        assert_eq!(ended.len(), 2);
        assert!(calls.is_empty());
    }
    #[test]
    fn the_rows_wait_then_show_whether_the_team_took_it() {
        let to = [false, true, true, false, false];
        let n = request(PlanCall::Poke);
        let other = request(PlanCall::KillFirst);
        assert_eq!(
            answers(n, to)[1],
            Some(Status::Pending),
            "a later call kept apart"
        );
        assert_eq!(answers(n, to)[0], None);
        if let Ok(mut t) = TAKEN.lock() {
            if let Some(e) = t.iter_mut().find(|(s, _)| *s == n) {
                e.1 = Some(true);
            }
        }
        assert_eq!(answers(n, to)[2], Some(Status::Accepted));
        assert_eq!(answers(other, to)[2], Some(Status::Pending));
        assert_eq!(
            answers(u64::MAX, to)[2],
            Some(Status::Declined),
            "not a call"
        );
    }
}
