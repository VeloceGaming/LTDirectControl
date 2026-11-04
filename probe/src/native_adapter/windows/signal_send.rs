//! The fight call the mod sends (research builds since 1.7.1-dev5, every
//! build since 1.7.1-dev13), from the step hook (`signal_hook.rs`), on the
//! viewed match's worker. `append` also writes the ping orders' calls
//! (`ping_step.rs`; until 1.7.1-dev21 also the research lever keys'
//! HideLine).
//!
//! `send`: right after the game's own broadcast ran for a teammate's brain,
//! one BattleHelp entry is appended to that brain's queue A when there is
//! room (length below capacity: nothing is allocated, the capacity is never
//! touched). The game's own code reads and grows this vector on this same
//! thread, between steps, so nothing else is in it now. Every other byte of
//! the game stays as it was.
//!
//! Since 1.7.1-dev8 the entry carries the mod's mark at +0x14
//! (`SIGNAL_MARK`), so the delivery probe can tell it from the same call
//! sent by the athlete's own AI.
//!
//! The controlled athlete's brain comes from `signal_call::Owners`, fed by
//! every brain's step (1.7.1-dev7; dev5's one-time scan is superseded).
//! The shared state's lock is held only for bookkeeping, never while game
//! memory is read or written.
use super::super::signals::*;
use super::signal_hook::word;
use crate::native_adapter::signal_call::{self as call, Decision, Mapping};
use crate::native_adapter::signal_probe as probe;

/// The target of the brain's big plan when it is Battle (raw 1, the
/// target entity at the plan's first payload word), read only.
pub(super) unsafe fn battle_target(brain: usize) -> Option<u64> {
    (word(brain, SIGNAL_BIG_PLAN)? == 1)
        .then(|| word(brain, SIGNAL_BIG_PLAN_ARG))
        .flatten()
}

/// The pending call for this brain's step: written into its queue A, left
/// for a later step (queue full), on it already, or ended (expired).
pub(super) unsafe fn send(
    brain: usize,
    side: u8,
    index: u8,
    tick: u64,
    out: &mut Vec<String>,
) -> bool {
    let battle = battle_target(brain);
    let Some((decision, map, serial, target)) = call::with(|s| {
        let request = s.request?;
        if request.ended {
            return None;
        }
        let Some(map) = s.owners.mapping else {
            if s.unresolved_serial != request.serial {
                s.unresolved_serial = request.serial;
                out.push(format!(
                    "PING map unresolved: nothing sent (request {})",
                    request.serial
                ));
            }
            return None;
        };
        let r = s.request.as_mut()?;
        Some((
            r.decide(&map, side, index, tick, battle),
            map,
            r.serial,
            r.target,
        ))
    })
    .flatten() else {
        return false;
    };
    match decision {
        Decision::Nothing => false,
        Decision::Expired => {
            let done = call::with(|s| s.request.map(|r| r.done_text())).flatten();
            out.push(format!(
                "PING expired request={serial} sent_to={}",
                done.unwrap_or_default()
            ));
            false
        }
        Decision::Write => write(brain, side, index, tick, &map, serial, target, out),
        Decision::OnIt => {
            let stored = call::answered(
                (side, index),
                crate::native_adapter::signal_fate::Answer::Join,
                tick,
            );
            out.push(format!(
                "PING answer team={side} to={index} target={target} answer=on_it request={serial} (nothing written; {stored})"
            ));
            false
        }
        Decision::Throttled => false,
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn write(
    brain: usize,
    side: u8,
    index: u8,
    tick: u64,
    map: &Mapping,
    serial: u64,
    target: usize,
    out: &mut Vec<String>,
) -> bool {
    // Due now (1.7.1-dev13; until dev12 tick + 15): heard at the
    // teammate's next delivery.
    let due = tick;
    let bytes = call::entry(
        due,
        u32::from(map.index),
        call::BATTLE_HELP,
        0,
        target as u64,
        SIGNAL_MARK,
    );
    let Ok((len, capacity)) = append(brain, &bytes) else {
        let first = call::with(|s| {
            s.request
                .as_mut()
                .filter(|r| r.serial == serial)
                .is_some_and(|r| r.first_drop(index))
        })
        .unwrap_or(false);
        if first {
            let (len, capacity) = queue(brain);
            out.push(format!(
                "PING dropped: queue full team={side} to={index} len={len}/{capacity} (kept for its next step)"
            ));
        }
        return false;
    };
    let _ = call::with(|s| {
        if let Some(r) = s.request.as_mut().filter(|r| r.serial == serial) {
            r.written(index, tick);
        }
    });
    out.push(format!(
        "PING sent team={side} to={index} from={} kind=BattleHelp target={target} due={due} mark={SIGNAL_MARK:#x} len={len}/{capacity} request={serial}",
        map.index,
    ));
    true
}

/// Queue A's length and capacity, for the log.
pub(super) unsafe fn queue(brain: usize) -> (usize, usize) {
    let header = brain + SIGNAL_TEAM_QUEUE_A;
    (
        std::ptr::read_unaligned((header + 16) as *const usize),
        std::ptr::read_unaligned(header as *const usize),
    )
}
/// Appends one entry to the brain's queue A when it has two free entries
/// (1.7.1-dev15, so the AI's own calls are not starved; until dev14 one):
/// nothing is allocated, the capacity is never touched. The new length and
/// the capacity; Err when there is no room (the caller tries again at a
/// later step).
pub(super) unsafe fn append(brain: usize, bytes: &[u8; 40]) -> Result<(usize, usize), ()> {
    let header = brain + SIGNAL_TEAM_QUEUE_A;
    let capacity = std::ptr::read_unaligned(header as *const usize);
    let pointer = std::ptr::read_unaligned((header + 8) as *const usize);
    let len = std::ptr::read_unaligned((header + 16) as *const usize);
    let room = probe::in_bounds(capacity, pointer, len)
        && len.saturating_add(2) <= capacity
        && pointer != 0
        && pointer.is_multiple_of(8);
    if !room {
        return Err(());
    }
    let at = pointer + len * probe::QUEUE_ENTRY;
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), at as *mut u8, bytes.len());
    std::ptr::write_unaligned((header + 16) as *mut usize, len + 1);
    Ok((len + 1, capacity))
}
