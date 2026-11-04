//! The step hook (family `SIGNALS`): the ping system's native half since
//! 1.7.1-dev13, the signal probe in research builds since 1.7.1-dev4.
//!
//! In every build, at each brain's first step in a tick on the viewed
//! match's worker, it feeds the mapping (each brain's owner, so the
//! controlled athlete's brain is known), writes the fight call
//! (`signal_send.rs`) and the orders (`ping_step.rs`) into the teammates'
//! brains, and logs the AI environment's kind once a match. More hooks
//! follow a call into the delivery (`signal_fate.rs`); public builds put in
//! only the two that hear the mod's own call.
//!
//! Research builds also log each brain's three lists when they change, its
//! receiver mode and delay and its call-join counters. (The lever tests,
//! `team_order.rs`, ran here until 1.7.1-dev21.)
//!
//! The hook replaces the per-player AI step's call to the broadcast (this
//! version's `signals.rs`). It calls the broadcast first and returns its
//! result unchanged; then, inside `catch_unwind`, it turns away every
//! thread but the viewed match's worker (one compare) and acts at most
//! once per brain and tick. The lines go straight to the log after that;
//! the log is never held while the game's code runs. Nothing the game
//! passes in is kept: a brain is known by its player's side and index
//! (`signal_probe::Memory`).
//!
//! Installed on its own after the other families: a differing site turns
//! it off and leaves every other hook as it was.
use super::super::signals::*;
use super::ping_step;
use super::signal_fate;
use super::signal_send;
use super::*;
use crate::native_adapter::signal_call as call;
#[cfg(feature = "research")]
use crate::native_adapter::signal_probe::Queue;
use crate::native_adapter::signal_probe::{self as probe, Memory};
use std::cell::RefCell;

type BroadcastFn = unsafe extern "system" fn(usize, usize, usize, usize, usize) -> u64;
static ORIGINAL_BROADCAST: AtomicUsize = AtomicUsize::new(0);
static SIGNAL_TRIED: AtomicBool = AtomicBool::new(false);
static SIGNAL_PATCH: OnceLock<[u8; 5]> = OnceLock::new();
/// The step hook is in: orders can be written.
pub(super) static STEP_IN: AtomicBool = AtomicBool::new(false);
thread_local! {
    /// The probe's memory on the viewed match's worker, shared with the
    /// delivery hooks (same thread): brains seen, lines spent.
    pub(super) static MEMORY: RefCell<Memory> = RefCell::new(Memory::default());
}

/// Patches the step hook in, at most once a session. Called from `install`,
/// at the title screen, after the other families are in.
pub(super) unsafe fn install_signal_probe(base: usize, logger: &Logger) {
    if SIGNAL_TRIED.swap(true, Ordering::AcqRel) {
        return;
    }
    let patched = patch(base);
    logger.write(&match &patched {
        Ok(()) => format!(
            "SIGNAL step hook installed site={SIGNAL_SITE:#x} broadcast={SIGNAL_ORIGINAL:#x} (pings{})",
            if cfg!(feature = "research") { " and the research probe" } else { "" }
        ),
        Err(e) => {
            format!("SIGNAL step hook unavailable: {e}; pings send nothing, other hooks unaffected")
        }
    });
    if patched.is_err() {
        return;
    }
    STEP_IN.store(true, Ordering::Release);
    // The objective setter (1.7.1-dev10; the orders since 1.7.1-dev13):
    // checked once, used only from the step hook above.
    logger.write(&ping_step::check_setter(base));
    // The delivery hooks (1.7.1-dev6), on their own: a differing site
    // turns only them off.
    signal_fate::install_fate_probe(base, logger);
}
unsafe fn patch(base: usize) -> Result<(), String> {
    check_call(
        base,
        SIGNAL_SITE,
        SIGNAL_BYTES,
        SIGNAL_ORIGINAL,
        &SIGNAL_PROLOGUE,
    )?;
    ORIGINAL_BROADCAST.store(base + SIGNAL_ORIGINAL, Ordering::Release);
    let bytes = write_call(
        base + SIGNAL_SITE,
        SIGNAL_BYTES,
        broadcast_hook as *const () as usize,
    )?;
    let _ = SIGNAL_PATCH.set(bytes);
    Ok(())
}
/// The call at `base + site` holds `expected` and its original opens with
/// `prologue` (checked before anything is written).
pub(super) unsafe fn check_call(
    base: usize,
    site: usize,
    expected: [u8; 5],
    original: usize,
    prologue: &[u8],
) -> Result<(), String> {
    if read_call(base + site) != expected {
        return Err(format!(
            "call site {site:#x} differs (another mod, or a changed game)"
        ));
    }
    let entry = std::slice::from_raw_parts((base + original) as *const u8, prologue.len());
    if entry != prologue {
        return Err(format!("entry {original:#x} differs"));
    }
    Ok(())
}
/// Points the call at `site` (address) to `hook` through a relay; restores
/// `expected` when it does not read back. The patch's bytes.
pub(super) unsafe fn write_call(
    site: usize,
    expected: [u8; 5],
    hook: usize,
) -> Result<[u8; 5], String> {
    let target = relay(site, hook)?;
    let bytes = relative_call(site, target)?;
    let mut old = 0;
    if VirtualProtect(site as *mut c_void, bytes.len(), 0x40, &mut old) == 0 {
        return Err("cannot unprotect the call site".into());
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), site as *mut u8, bytes.len());
    let flushed = FlushInstructionCache(GetCurrentProcess(), site as *const c_void, 5) != 0;
    let readback = read_call(site) == bytes;
    if !(flushed && readback) {
        std::ptr::copy_nonoverlapping(expected.as_ptr(), site as *mut u8, 5);
        FlushInstructionCache(GetCurrentProcess(), site as *const c_void, 5);
    }
    let mut ignored = 0;
    VirtualProtect(site as *mut c_void, bytes.len(), old, &mut ignored);
    if !(flushed && readback) {
        return Err("the call site did not read back; original restored".into());
    }
    Ok(bytes)
}
/// Puts the original call back at `site` (address).
pub(super) unsafe fn restore_call(site: usize, expected: [u8; 5]) {
    let mut old = 0;
    if VirtualProtect(site as *mut c_void, 5, 0x40, &mut old) == 0 {
        return;
    }
    std::ptr::copy_nonoverlapping(expected.as_ptr(), site as *mut u8, 5);
    FlushInstructionCache(GetCurrentProcess(), site as *const c_void, 5);
    let mut ignored = 0;
    VirtualProtect(site as *mut c_void, 5, old, &mut ignored);
}
/// True when the probe's call sites (the step's and the delivery's) still
/// hold their patches.
pub(super) fn signal_probe_intact() -> Option<bool> {
    let bytes = SIGNAL_PATCH.get()?;
    let base = verified_base()?;
    let step = unsafe { read_call(base + SIGNAL_SITE) } == *bytes;
    Some(step && signal_fate::fate_intact(base).unwrap_or(true))
}

/// The fourth argument is the simulation, the fifth the receiver state
/// (superseded 2026-11-03: dev4 named it the simulation's vtable).
unsafe extern "system" fn broadcast_hook(
    brain: usize,
    context: usize,
    player: usize,
    simulation: usize,
    receiver: usize,
) -> u64 {
    let original: BroadcastFn = std::mem::transmute(ORIGINAL_BROADCAST.load(Ordering::Acquire));
    let result = original(brain, context, player, simulation, receiver);
    if crate::perf::on_worker_thread() {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            observe(brain, context, player, simulation, receiver)
        }));
    }
    result
}

/// A word at `base + offset`, when `base` looks like a game object.
pub(super) unsafe fn word(base: usize, offset: usize) -> Option<u64> {
    (base != 0 && base.is_multiple_of(8))
        .then(|| std::ptr::read_unaligned((base + offset) as *const u64))
}
/// The receiver state's mode byte and delay (`[[receiver + 8] + 0x12f8]`
/// on this version), each when its object looks like one.
pub(super) unsafe fn receiver_reads(receiver: usize) -> (Option<u8>, Option<u64>) {
    if receiver == 0 || !receiver.is_multiple_of(8) {
        return (None, None);
    }
    let mode = std::ptr::read_unaligned((receiver + SIGNAL_RECEIVER_MODE) as *const u8);
    let delay = word(receiver, SIGNAL_RECEIVER_STATE)
        .and_then(|state| word(state as usize, SIGNAL_STATE_DELAY));
    (Some(mode), delay)
}
/// One of the brain's lists, when its header is plausible.
#[cfg(feature = "research")]
unsafe fn list<'a>(brain: usize, offset: usize, entry: usize) -> Option<Queue<'a>> {
    let capacity = word(brain, offset)? as usize;
    let pointer = word(brain, offset + 8)? as usize;
    let len = word(brain, offset + 16)? as usize;
    if !probe::in_bounds(capacity, pointer, len) {
        return None;
    }
    let bytes = if len == 0 {
        &[][..]
    } else {
        std::slice::from_raw_parts(pointer as *const u8, len * entry)
    };
    Some(Queue { capacity, bytes })
}

unsafe fn observe(brain: usize, context: usize, player: usize, simulation: usize, receiver: usize) {
    let Some(shared) = SHARED.get() else {
        return;
    };
    if brain == 0 || !brain.is_multiple_of(8) {
        return;
    }
    let (Some(tick), Some(side)) = (
        word(simulation, SIGNAL_WORLD_TICK),
        word(player, SIGNAL_PLAYER_SIDE),
    ) else {
        return;
    };
    let (mode, delay) = receiver_reads(receiver);
    let index = std::ptr::read_unaligned((player + SIGNAL_PLAYER_INDEX) as *const u32);
    let Some(slot) = probe::slot(side, index) else {
        // Not expected (sides 0 and 1, indexes 0 to 4): said once a match,
        // so a quiet log is never mistaken for a hook that never fired.
        let first = MEMORY.with(|memory| {
            memory.try_borrow_mut().is_ok_and(|mut m| {
                if m.observe_match(tick) {
                    call::new_match();
                }
                !std::mem::replace(&mut m.range_logged, true)
            })
        });
        if first {
            shared.logger.write(&format!(
                "SIGNAL probe: brain out of range side={side} index={index} t={tick}; its steps are not logged"
            ));
        }
        return;
    };
    call::TICK.store(tick, Ordering::Relaxed);
    let step = Step {
        brain,
        player,
        tick,
        side: side as u8,
        index: index as u8,
        slot,
        mode,
        delay,
        simulation,
        context,
        receiver,
    };
    let lines = MEMORY.with(|memory| {
        memory
            .try_borrow_mut()
            .map(|mut memory| collect(&mut memory, &step))
            .unwrap_or_default()
    });
    for line in &lines {
        shared.logger.write(line);
    }
}
/// What one step of the viewed match shows the hook.
struct Step {
    brain: usize,
    player: usize,
    tick: u64,
    side: u8,
    index: u8,
    slot: usize,
    mode: Option<u8>,
    /// The clock word `[[receiver + 8] + SIGNAL_STATE_DELAY]`: the ticks
    /// per second the game multiplies for its own 5 s and 30 s (1.7.1-dev17,
    /// read from the code at 0xefcde7 and 0xe40af7; named the delivery's
    /// delay in 1.7.1-dev5).
    delay: Option<u64>,
    simulation: usize,
    /// The step's context and receiver state (1.7.1-dev18: the team's
    /// shared block and the world), read during this step only.
    context: usize,
    receiver: usize,
}
/// The AI environment's kind, from the vtable at `[sim + 8]` (for the
/// wrapper, the inner one at `[[sim] + 8]`), and its log line.
unsafe fn environment(simulation: usize) -> (Option<u8>, String) {
    let Some(base) = verified_base() else {
        return (None, "SIGNAL env unknown: no executable base".to_owned());
    };
    let rva = |vtable: Option<u64>| vtable.and_then(|v| (v as usize).checked_sub(base));
    let Some(outer) = rva(word(simulation, 8)) else {
        return (None, "SIGNAL env unknown: no vtable".to_owned());
    };
    let kind = signal_fate::env_kind(SIGNAL_ENV_VTABLES, outer);
    if kind != Some(signal_fate::ENV_WRAPPER) {
        return (kind, signal_fate::env_line(outer, kind, None));
    }
    let inner = rva(word(simulation, 0).and_then(|data| word(data as usize, 8)));
    let inner_kind = inner.and_then(|v| signal_fate::env_kind(SIGNAL_ENV_VTABLES, v));
    let line = signal_fate::env_line(outer, kind, Some((inner.unwrap_or(0), inner_kind)));
    (inner_kind.filter(|k| *k != signal_fate::ENV_WRAPPER), line)
}
/// The lines this step adds: its owner (the mapping), the calls and orders
/// the mod writes for it, and in research builds a new brain, its
/// counters and the brain's lists when they changed
/// (within the match's budget; `PING` lines and a dump holding the mod's
/// own entry always).
unsafe fn collect(memory: &mut Memory, s: &Step) -> Vec<String> {
    if memory.observe_match(s.tick) {
        call::new_match();
    }
    let (tick, side, index) = (s.tick, s.side, s.index);
    let mut pings = Vec::new();
    if memory.env.is_none() {
        let (kind, line) = environment(s.simulation);
        memory.env = Some(kind);
        pings.push(line);
    }
    #[cfg(feature = "research")]
    let mut lines = Vec::new();
    #[cfg(feature = "research")]
    if memory.first_sight(s.slot) {
        lines.push(format!("SIGNAL player t={tick} team={side} index={index}"));
    }
    if !memory.first_step(s.slot, tick) {
        return pings;
    }
    if let Some(entity) = word(s.player, SIGNAL_PLAYER_ENTITY) {
        pings.extend(call::step_owner(side, index, entity as usize));
    }
    #[cfg(feature = "research")]
    research_counters(memory, s, &mut lines, &mut pings);
    let wrote = signal_send::send(s.brain, side, index, tick, &mut pings);
    // The ping system's orders (1.7.1-dev13).
    let env = ping_step::Env {
        kind0: memory.env == Some(Some(0)),
        mode: s.mode,
        tps: s.delay,
    };
    let site = ping_step::Site {
        simulation: s.simulation,
        context: s.context,
        receiver: s.receiver,
    };
    ping_step::step(s.brain, site, (side, index), tick, env, &mut pings);
    #[cfg(feature = "research")]
    {
        research_lists(memory, s, wrote, &mut lines);
        if !lines.is_empty() {
            match memory.spend_or_force(lines.len(), wrote) {
                Ok(()) => {}
                Err(last) => lines = last.map(str::to_owned).into_iter().collect(),
            }
        }
        // The call's own lines first, then the dump that shows it.
        pings.extend(lines);
    }
    #[cfg(not(feature = "research"))]
    let _ = wrote;
    pings
}
/// Research builds: the brain's call-join counters when they changed.
#[cfg(feature = "research")]
unsafe fn research_counters(
    memory: &mut Memory,
    s: &Step,
    lines: &mut Vec<String>,
    pings: &mut Vec<String>,
) {
    let join = std::slice::from_raw_parts(
        (s.brain + SIGNAL_JOIN_STATS) as *const u64,
        signal_fate::JOIN_NAMES.len(),
    );
    let join: [u64; 8] = std::array::from_fn(|i| join[i]);
    if let Some(forced) = memory.join_line(s.slot, join, s.tick) {
        let line = signal_fate::join_line(s.side, s.index, &join);
        // After a mod scene, outside the budget.
        if forced {
            pings.push(line);
        } else {
            lines.push(line);
        }
    }
}
/// Research builds: the brain's three lists, when they changed or the mod
/// just wrote into them.
#[cfg(feature = "research")]
unsafe fn research_lists(memory: &mut Memory, s: &Step, wrote: bool, lines: &mut Vec<String>) {
    let lists = (
        list(s.brain, SIGNAL_TEAM_QUEUE_A, probe::QUEUE_ENTRY),
        list(s.brain, SIGNAL_TEAM_QUEUE_B, probe::QUEUE_ENTRY),
        list(s.brain, SIGNAL_TEAM_LIST_C, probe::LIST_ENTRY),
    );
    match lists {
        (Some(a), Some(b), Some(c)) => {
            if memory.changed(s.slot, probe::digest([&a, &b, &c])) || wrote {
                lines.extend(probe::dump(
                    s.tick,
                    usize::from(s.side),
                    u32::from(s.index),
                    (s.mode, s.delay),
                    [&a, &b, &c],
                ));
            }
        }
        _ if !std::mem::replace(&mut memory.bounds_logged, true) => {
            lines.push("SIGNAL probe: queue out of bounds".to_owned());
        }
        _ => {}
    }
}
