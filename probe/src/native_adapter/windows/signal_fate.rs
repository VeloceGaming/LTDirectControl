//! The delivery hooks: in research builds since 1.7.1-dev6 the delivery
//! probe, five hooks inside the game's delivery of AI calls (this version's `signals.rs`, "Delivery"):
//! the late drop, the gate, the two calls of the receive handler and the
//! scene opener. Each replaces one `call rel32` with a call to a function of
//! the same signature (stack arguments pass through unchanged), calls the
//! original first and returns its result untouched. Then, inside
//! `catch_unwind` and only on the viewed match's worker, it reads a few
//! fields with null and alignment checks and writes one line
//! (`signal_fate` names them). Nothing the game passes in is kept: the
//! signal is a copy on the delivery's stack, still valid after the call.
//!
//! The gate calls the handler, which calls the scene opener: their lines
//! are put in that order (each takes its place before calling the
//! original) and written when the outermost hook returns, within the
//! probe's line budget; a line marked `ours=yes` always.
//!
//! All are checked before any is written; if one does not read back, the
//! ones already written are put back, so they are in or out together.
//!
//! Public builds (1.7.1-dev13, the ping system) put in only the two heard
//! hooks, for the mod's own scene call below, and write no delivery lines;
//! the scene opener is called, never hooked, once its opening bytes match.
//!
//! The mod's own scene (1.7.1-dev7): in an AI environment of kind 0 the
//! receive handler opens no scene for a call (read from the code), so the
//! mod's BattleHelp is heard and nothing follows. When the heard hook sees
//! the mod's own call (its sender, kind and target, to a teammate the mod
//! wrote it to) in kind 0, it calls the scene opener itself, once per
//! teammate and request, with the arguments the handler passes in the
//! other kinds (`open_scene`). This runs a game function on a path the
//! game does not take in kind 0: `catch_unwind` does not stop a crash
//! inside it.
//!
//! Since 1.7.1-dev8 the mod's own call is the one carrying its mark
//! (`SIGNAL_MARK` in the signal's word at +4); the same call without it is
//! the athlete's own AI planner's (`ours=unmarked`). Around the mod's
//! scene call the probe reads the brain's scene state, its call-join
//! counters and list C's length, and the teammate's unit, before and after
//! (`SIGNAL scene_state`, `SIGNAL answer`), and leaves the answer for the
//! HUD (`signal_call::answered`). Read only; nothing is kept.
use super::super::signals::*;
use super::signal_hook::{check_call, receiver_reads, restore_call, word, write_call, MEMORY};
use super::*;
use crate::native_adapter::signal_call as call;
#[cfg(feature = "research")]
use crate::native_adapter::signal_fate::AnswerFacts;
use crate::native_adapter::signal_fate::{
    self as fate, Mine, SceneState, Signal, REASON_BATTLE_HELP, SIGNAL_BYTES,
};
use crate::native_adapter::signal_probe as probe;
use std::cell::{Cell, RefCell};

#[cfg(feature = "research")]
pub(super) use fate::join_line;
#[cfg(feature = "research")]
pub(super) use fate::JOIN_NAMES;
pub(super) use fate::{env_kind, env_line, ENV_WRAPPER};

#[cfg(feature = "research")]
type LateFn = unsafe extern "system" fn(usize, usize, usize, usize) -> u64;
type HeardFn =
    unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, usize, usize) -> u64;
/// The gate's and the scene opener's: nine arguments.
type NineFn =
    unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, usize, usize, usize) -> u64;

#[cfg(feature = "research")]
static LATE: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "research")]
static GATE: AtomicUsize = AtomicUsize::new(0);
static HEARD: AtomicUsize = AtomicUsize::new(0);
static SCENE: AtomicUsize = AtomicUsize::new(0);
/// (site RVA, patch bytes) of each hook in.
static FATE_PATCH: OnceLock<Vec<(usize, [u8; 5])>> = OnceLock::new();

struct Hook {
    site: usize,
    bytes: [u8; 5],
    original: usize,
    prologue: &'static [u8],
    hook: usize,
    slot: &'static AtomicUsize,
}

/// Patches the hooks in, after the step hook (research builds: all five;
/// public builds: the two heard sites). Logs the outcome.
pub(super) unsafe fn install_fate_probe(base: usize, logger: &Logger) {
    #[cfg(feature = "research")]
    let hooks = [
        Hook {
            site: SIGNAL_LATE_SITE,
            bytes: SIGNAL_LATE_BYTES,
            original: SIGNAL_LATE_ORIGINAL,
            prologue: &SIGNAL_LATE_PROLOGUE,
            hook: late_hook as *const () as usize,
            slot: &LATE,
        },
        Hook {
            site: SIGNAL_GATE_SITE,
            bytes: SIGNAL_GATE_BYTES,
            original: SIGNAL_GATE_ORIGINAL,
            prologue: &SIGNAL_GATE_PROLOGUE,
            hook: gate_hook as *const () as usize,
            slot: &GATE,
        },
        Hook {
            site: SIGNAL_HEARD_SITES[0],
            bytes: SIGNAL_HEARD_BYTES[0],
            original: SIGNAL_HEARD_ORIGINAL,
            prologue: &SIGNAL_HEARD_PROLOGUE,
            hook: heard_hook_1 as *const () as usize,
            slot: &HEARD,
        },
        Hook {
            site: SIGNAL_HEARD_SITES[1],
            bytes: SIGNAL_HEARD_BYTES[1],
            original: SIGNAL_HEARD_ORIGINAL,
            prologue: &SIGNAL_HEARD_PROLOGUE,
            hook: heard_hook_2 as *const () as usize,
            slot: &HEARD,
        },
        Hook {
            site: SIGNAL_SCENE_SITE,
            bytes: SIGNAL_SCENE_BYTES,
            original: SIGNAL_SCENE_ORIGINAL,
            prologue: &SIGNAL_SCENE_PROLOGUE,
            hook: scene_hook as *const () as usize,
            slot: &SCENE,
        },
    ];
    #[cfg(not(feature = "research"))]
    let hooks = [
        Hook {
            site: SIGNAL_HEARD_SITES[0],
            bytes: SIGNAL_HEARD_BYTES[0],
            original: SIGNAL_HEARD_ORIGINAL,
            prologue: &SIGNAL_HEARD_PROLOGUE,
            hook: heard_hook_1 as *const () as usize,
            slot: &HEARD,
        },
        Hook {
            site: SIGNAL_HEARD_SITES[1],
            bytes: SIGNAL_HEARD_BYTES[1],
            original: SIGNAL_HEARD_ORIGINAL,
            prologue: &SIGNAL_HEARD_PROLOGUE,
            hook: heard_hook_2 as *const () as usize,
            slot: &HEARD,
        },
    ];
    // Public builds call the scene opener without hooking it: its opening
    // bytes first.
    #[cfg(not(feature = "research"))]
    {
        let entry = std::slice::from_raw_parts(
            base.wrapping_add(SIGNAL_SCENE_ORIGINAL) as *const u8,
            SIGNAL_SCENE_PROLOGUE.len(),
        );
        if entry != SIGNAL_SCENE_PROLOGUE {
            logger.write(&format!(
                "PING scene opener {SIGNAL_SCENE_ORIGINAL:#x} differs: the fight call is written but opens no scene"
            ));
            return;
        }
    }
    logger.write(&match patch_all(base, &hooks) {
        Ok(patched) => {
            let sites: Vec<String> = patched.iter().map(|(s, _)| format!("{s:#x}")).collect();
            let _ = FATE_PATCH.set(patched);
            #[cfg(not(feature = "research"))]
            SCENE.store(base + SIGNAL_SCENE_ORIGINAL, Ordering::Release);
            format!(
                "SIGNAL delivery hooks installed sites=[{}] ({})",
                sites.join(","),
                if cfg!(feature = "research") {
                    "research build; read-only"
                } else {
                    "pings: the mod's own fight call"
                }
            )
        }
        Err(e) => format!(
            "SIGNAL delivery hooks unavailable: {e}; the step probe and other hooks unaffected"
        ),
    });
}
unsafe fn patch_all(base: usize, hooks: &[Hook]) -> Result<Vec<(usize, [u8; 5])>, String> {
    for h in hooks {
        check_call(base, h.site, h.bytes, h.original, h.prologue)?;
    }
    for h in hooks {
        h.slot.store(base + h.original, Ordering::Release);
    }
    let mut patched: Vec<(usize, [u8; 5])> = Vec::new();
    for h in hooks {
        match write_call(base + h.site, h.bytes, h.hook) {
            Ok(bytes) => patched.push((h.site, bytes)),
            Err(e) => {
                for (site, _) in &patched {
                    if let Some(h) = hooks.iter().find(|h| h.site == *site) {
                        restore_call(base + h.site, h.bytes);
                    }
                }
                return Err(format!("{e} (at {:#x}; the others put back)", h.site));
            }
        }
    }
    Ok(patched)
}
/// True when every delivery hook is still in; None when none were put in.
pub(super) fn fate_intact(base: usize) -> Option<bool> {
    let patched = FATE_PATCH.get()?;
    Some(
        patched
            .iter()
            .all(|(site, bytes)| unsafe { read_call(base + site) } == *bytes),
    )
}

// ---- Line order and writing ----
// Public builds keep the depth count (so `leave` always runs) and drop
// the lines: `write` logs nothing there.

#[derive(Default)]
struct Pending {
    depth: u32,
    /// (line, forced: about the mod's own call).
    lines: Vec<(String, bool)>,
}
thread_local! {
    static PENDING: RefCell<Pending> = RefCell::new(Pending::default());
    /// Whose the call the receive handler is handling is, for the scene
    /// opener it calls (which is not passed the signal).
    static HEARD_MINE: Cell<Mine> = const { Cell::new(Mine::Unknown) };
}
/// Before the original: this hook's place among the lines (None off the
/// viewed match's worker, or when the buffer is busy).
fn enter() -> Option<usize> {
    if !crate::perf::on_worker_thread() {
        return None;
    }
    PENDING.with(|p| {
        p.try_borrow_mut().ok().map(|mut p| {
            p.depth += 1;
            p.lines.len()
        })
    })
}
/// After the original: the hook's lines (if any) at its place; written
/// out when the outermost hook returns.
fn leave(mark: usize, lines: Vec<(String, bool)>) {
    let lines = PENDING.with(|p| {
        let mut p = p.try_borrow_mut().ok()?;
        let at = mark.min(p.lines.len());
        p.lines.splice(at..at, lines);
        p.depth = p.depth.saturating_sub(1);
        (p.depth == 0).then(|| std::mem::take(&mut p.lines))
    });
    if let Some(lines) = lines.filter(|l| !l.is_empty()) {
        let _ = catch_unwind(AssertUnwindSafe(|| write(lines)));
    }
}
fn write(lines: Vec<(String, bool)>) {
    if !cfg!(feature = "research") {
        // Public builds: only the mod's own call's answer, always.
        if let Some(shared) = SHARED.get() {
            for (line, forced) in lines {
                if forced {
                    shared.logger.write(&line);
                }
            }
        }
        return;
    }
    let Some(shared) = SHARED.get() else {
        return;
    };
    for (line, forced) in lines {
        let spent = MEMORY.with(|m| {
            m.try_borrow_mut()
                .map(|mut m| {
                    m.spend_or_force(1, forced)
                        .map_err(|e| e.map(str::to_owned))
                })
                .unwrap_or(Err(None))
        });
        match spent {
            Ok(()) => shared.logger.write(&line),
            Err(Some(last)) => shared.logger.write(&last),
            Err(None) => {}
        }
    }
}
/// Runs `line` (bounded reads, one line) inside `catch_unwind`, then
/// `leave`, so the depth always comes back.
fn finish(mark: Option<usize>, line: impl FnOnce() -> Option<(String, bool)>) {
    if let Some(mark) = mark {
        let line = catch_unwind(AssertUnwindSafe(line)).ok().flatten();
        leave(mark, line.into_iter().collect());
    }
}

// ---- Reads ----

/// The signal copy the delivery passes (aligned: the game moves it with
/// aligned stores).
unsafe fn signal(at: usize) -> Option<Signal> {
    (at != 0 && at.is_multiple_of(8)).then(|| {
        let raw: [u8; SIGNAL_BYTES] = std::ptr::read_unaligned(at as *const [u8; SIGNAL_BYTES]);
        Signal::read(&raw)
    })
}
/// (side, index) of a player state, when both are in range.
unsafe fn player_of(state: usize) -> Option<(u8, u8)> {
    let side = word(state, SIGNAL_PLAYER_SIDE)?;
    let index = std::ptr::read_unaligned((state + SIGNAL_PLAYER_INDEX) as *const u32);
    probe::slot(side, index).map(|_| (side as u8, index as u8))
}
/// (now, mode, delay) through the delivery's pair {sim, receiver}.
#[cfg_attr(not(feature = "research"), allow(dead_code))]
unsafe fn pair_reads(pair: usize) -> (Option<u64>, Option<u8>, Option<u64>) {
    let now = word(pair, 0).and_then(|sim| word(sim as usize, SIGNAL_WORLD_TICK));
    let (mode, delay) = word(pair, 8).map_or((None, None), |r| receiver_reads(r as usize));
    (now, mode, delay)
}
fn ours(now: Option<u64>, to: Option<(u8, u8)>, sender: u32, s: &Signal) -> Mine {
    let tick = now.unwrap_or_else(|| call::TICK.load(Ordering::Relaxed));
    call::open_call(tick, SIGNAL_MARK)
        .map_or(Mine::No, |o| o.mine(to, sender, s.tag, s.target, s.word))
}

// ---- Hooks ----

#[cfg(feature = "research")]
unsafe extern "system" fn late_hook(brain: usize, due: usize, sender: usize, at: usize) -> u64 {
    let original: LateFn = std::mem::transmute(LATE.load(Ordering::Acquire));
    let mark = enter();
    let result = original(brain, due, sender, at);
    finish(mark, || {
        let s = signal(at)?;
        let sender = sender as u32;
        // Only the brain is passed here, and no brain address is kept
        // (1.7.1-dev7: dev6's brain field went with the scan), so the
        // receiver and its delay are `?`; `now` is the latest tick any
        // step saw.
        let now = Some(call::TICK.load(Ordering::Relaxed));
        let mine = ours(now, None, sender, &s);
        let line = fate::late_line(None, sender, &s, due as u64, now, None, mine);
        Some((line, mine.forced()))
    });
    result
}

#[cfg(feature = "research")]
#[allow(clippy::too_many_arguments)]
unsafe extern "system" fn gate_hook(
    brain: usize,
    context: usize,
    unknown: usize,
    player: usize,
    pair: usize,
    sender: usize,
    at: usize,
    flag: usize,
    extra: usize,
) -> u64 {
    let original: NineFn = std::mem::transmute(GATE.load(Ordering::Acquire));
    let mark = enter();
    let result = original(
        brain, context, unknown, player, pair, sender, at, flag, extra,
    );
    finish(mark, || {
        let s = signal(at)?;
        let sender = sender as u32;
        let to = player_of(player);
        let (now, mode, delay) = pair_reads(pair);
        let accepted = mode.and_then(|m| fate::mask_ok(&SIGNAL_GATE_RULES, sender, m));
        let mine = ours(now, to, sender, &s);
        let line = fate::gate_line(to, sender, &s, now, mode, delay, accepted, mine);
        Some((line, mine.forced()))
    });
    result
}

#[allow(clippy::too_many_arguments)]
unsafe fn heard(
    site: u8,
    brain: usize,
    context: usize,
    unknown: usize,
    player: usize,
    pair: usize,
    sender: usize,
    at: usize,
    extra: usize,
) -> u64 {
    let original: HeardFn = std::mem::transmute(HEARD.load(Ordering::Acquire));
    let mark = enter();
    // Whose the call is, before the handler runs: the scene hook nested in
    // it reads it.
    let mine = mark
        .and_then(|_| {
            catch_unwind(AssertUnwindSafe(|| {
                let s = signal(at)?;
                let now = pair_reads(pair).0;
                Some(ours(now, player_of(player), sender as u32, &s))
            }))
            .ok()
            .flatten()
        })
        .unwrap_or(Mine::Unknown);
    let outer = HEARD_MINE.with(|m| m.replace(mine));
    let result = original(brain, context, unknown, player, pair, sender, at, extra);
    HEARD_MINE.with(|m| m.set(outer));
    let act = mark.and_then(|_| {
        catch_unwind(AssertUnwindSafe(|| plan_act(player, pair, sender, at)))
            .ok()
            .flatten()
    });
    finish(mark, || {
        // The heard line: research builds only.
        if !cfg!(feature = "research") {
            return None;
        }
        let s = signal(at)?;
        let sender = sender as u32;
        let to = player_of(player);
        let (now, mode, _) = pair_reads(pair);
        Some((
            fate::heard_line(to, sender, &s, now, mode, site, mine),
            mine.forced(),
        ))
    });
    if let Some(act) = act {
        open_scene(&act, [brain, context, unknown, player, pair, sender, extra]);
    }
    result
}

/// The mod's own call, heard in an environment of kind 0, to act on.
struct Act {
    serial: u64,
    to: (u8, u8),
    target: u64,
    now: u64,
}
/// Whether the heard call is the mod's own and wants its scene now (see the
/// module notes). Marks it acted on (once per teammate and request) and
/// owes that brain a join line. No lock or borrow is kept.
unsafe fn plan_act(player: usize, pair: usize, sender: usize, at: usize) -> Option<Act> {
    if FATE_PATCH.get().is_none() || SCENE.load(Ordering::Acquire) == 0 {
        return None;
    }
    let kind0 = MEMORY.with(|m| m.try_borrow().ok().and_then(|m| m.env)) == Some(Some(0));
    let s = signal(at)?;
    if !kind0 || s.tag != call::BATTLE_HELP {
        return None;
    }
    let to = player_of(player)?;
    let now = pair_reads(pair).0?;
    let heard = call::Heard {
        sender: sender as u32,
        tag: s.tag,
        target: s.target,
        word: s.word,
    };
    let serial = call::act_on(now, to, heard, SIGNAL_MARK)?;
    let slot = probe::slot(u64::from(to.0), u32::from(to.1))?;
    MEMORY.with(|m| {
        if let Ok(mut m) = m.try_borrow_mut() {
            m.mod_scene(slot, now);
        }
    });
    Some(Act {
        serial,
        to,
        target: s.target,
        now,
    })
}
/// Calls the scene opener as the receive handler does for BattleHelp in
/// the other kinds (0xe67566..0xe67580 on 0.7.0): the handler's own brain,
/// context, third argument, player state and pair, the sender as a dword,
/// reason 2, the signal's target and the handler's last argument. The
/// hooked scene site does not see this call, so its line is written here.
///
/// Since 1.7.1-dev8 the brain and the teammate's unit are read before and
/// after the call (`before`, `after_lines`), through the same guarded reads.
unsafe fn open_scene(act: &Act, args: [usize; 7]) {
    let [brain, context, unknown, player, pair, sender, extra] = args;
    let original: NineFn = std::mem::transmute(SCENE.load(Ordering::Acquire));
    let mark = enter();
    let before = mark.and_then(|_| {
        catch_unwind(AssertUnwindSafe(|| {
            (snapshot(brain), teammate(pair, act.to))
        }))
        .ok()
    });
    original(
        brain,
        context,
        unknown,
        player,
        pair,
        sender & 0xffff_ffff,
        usize::from(REASON_BATTLE_HELP),
        act.target as usize,
        extra,
    );
    if let Some(mark) = mark {
        let lines = catch_unwind(AssertUnwindSafe(|| {
            after_lines(act, brain, sender as u32, before)
        }))
        .unwrap_or_default();
        leave(mark, lines);
    }
}
/// Busy means fighting someone else (1.7.1-dev15): a teammate the game
/// answered "in battle" whose Battle now targets the pinged enemy (the
/// game re-pointed him) is on it, accepted. The answer and its log word
/// (`on_it` for that case).
unsafe fn repointed(
    brain: usize,
    target: u64,
    answer: fate::Answer,
) -> (fate::Answer, &'static str) {
    if answer == fate::Answer::InBattle && super::signal_send::battle_target(brain) == Some(target)
    {
        (fate::Answer::Join, "on_it")
    } else if answer == fate::Answer::InBattle {
        (answer, "busy")
    } else {
        (answer, answer.text())
    }
}
/// The lines after the mod's scene call: the scene (by=mod), the scene
/// state before and after, the game's answer, and `PING acted`. The answer
/// is also left for the HUD.
#[cfg(not(feature = "research"))]
unsafe fn after_lines(
    act: &Act,
    brain: usize,
    _sender: u32,
    before: Option<(Snapshot, Teammate)>,
) -> Vec<(String, bool)> {
    let (side, index) = act.to;
    let after = snapshot(brain);
    let answer = match before.and_then(|b| b.0.counters).zip(after.counters) {
        Some((b, a)) => fate::answer(&b, &a).0,
        None => fate::Answer::None,
    };
    let (answer, word) = repointed(brain, act.target, answer);
    let stored = call::answered(act.to, answer, act.now);
    vec![(
        format!(
            "PING answer team={side} to={index} target={} answer={word} request={} ({stored})",
            act.target, act.serial
        ),
        true,
    )]
}
#[cfg(feature = "research")]
unsafe fn after_lines(
    act: &Act,
    brain: usize,
    sender: u32,
    before: Option<(Snapshot, Teammate)>,
) -> Vec<(String, bool)> {
    let (side, index) = act.to;
    let mut lines = vec![(
        fate::scene_line(
            Some(act.to),
            sender,
            REASON_BATTLE_HELP,
            act.target,
            Some(act.now),
            Mine::Yes,
            true,
        ),
        true,
    )];
    let after = snapshot(brain);
    if let Some((before, unit)) = before {
        lines.push((
            fate::scene_state_line(act.to, act.serial, &before.state, &after.state),
            true,
        ));
        let (answer, delta) = match (before.counters, after.counters) {
            (Some(b), Some(a)) => fate::answer(&b, &a),
            _ => (fate::Answer::None, "?".to_owned()),
        };
        let (answer, word) = repointed(brain, act.target, answer);
        let delta = format!("{delta} answer={word}");
        let stored = (before.counters.is_some() && after.counters.is_some())
            .then(|| call::answered(act.to, answer, act.now));
        let facts = AnswerFacts {
            reply: reply_pushed(brain, before.list_c_len),
            hp: unit.hp,
            speed: unit.speed,
            pos: unit.pos,
        };
        lines.push((
            fate::answer_line(act.to, act.serial, answer, &delta, &facts),
            true,
        ));
        lines.extend(stored.map(|line| (line, true)));
    }
    lines.push((
        format!(
            "PING acted team={side} to={index} target={} request={}",
            act.target, act.serial
        ),
        true,
    ));
    lines
}

/// What the probe reads of a brain around the mod's scene call (public
/// builds use the counters only).
#[cfg_attr(not(feature = "research"), allow(dead_code))]
struct Snapshot {
    state: SceneState,
    counters: Option<[u64; 8]>,
    list_c_len: Option<usize>,
}
unsafe fn snapshot(brain: usize) -> Snapshot {
    let aligned = brain != 0 && brain.is_multiple_of(8);
    let state = SceneState {
        kind: word(brain, SIGNAL_SCENE_KIND),
        arg: word(brain, SIGNAL_SCENE_ARG),
        active: word(brain, SIGNAL_SCENE_BLOCK),
        tick: word(brain, SIGNAL_SCENE_TICK),
        pos: word(brain, SIGNAL_SCENE_POS).zip(word(brain, SIGNAL_SCENE_POS + 8)),
        dist: word(brain, SIGNAL_SCENE_DIST),
        flag: aligned.then(|| std::ptr::read_unaligned((brain + SIGNAL_SCENE_FLAG) as *const u8)),
    };
    let counters = aligned.then(|| {
        let c = std::slice::from_raw_parts(
            (brain + SIGNAL_JOIN_STATS) as *const u64,
            fate::JOIN_NAMES.len(),
        );
        std::array::from_fn(|i| c[i])
    });
    Snapshot {
        state,
        counters,
        list_c_len: word(brain.wrapping_add(SIGNAL_TEAM_LIST_C), 16).map(|l| l as usize),
    }
}
/// The tag of the reply the call pushed onto list C: None when the list
/// does not read as one, `Some(None)` when it did not grow. The pointer is
/// read after the call (pushing may have moved the list).
#[cfg(feature = "research")]
unsafe fn reply_pushed(brain: usize, before: Option<usize>) -> Option<Option<u8>> {
    let before = before?;
    // Added at run time: 0.6.5's placeholder offset would overflow a
    // constant sum (that version never runs this).
    let list = brain.wrapping_add(SIGNAL_TEAM_LIST_C);
    let capacity = word(list, 0)? as usize;
    let pointer = word(list, 8)? as usize;
    let len = word(list, 16)? as usize;
    if !probe::in_bounds(capacity, pointer, len) {
        return None;
    }
    if len <= before {
        return Some(None);
    }
    Some(Some(std::ptr::read_unaligned(
        (pointer + before * probe::LIST_ENTRY) as *const u8,
    )))
}
/// The teammate's unit as the scene opener judges it, read before the
/// call: hp percent, move speed and position.
#[derive(Default)]
#[cfg_attr(not(feature = "research"), allow(dead_code))]
struct Teammate {
    hp: Option<u64>,
    speed: Option<u64>,
    pos: Option<(u64, u64)>,
}
/// Through the simulation's units table, as the receive handler reads it
/// (`[[pair] + 0x238 + (side * 5 + index) * 8]` on 0.7.0). No unit (dead):
/// nothing.
unsafe fn teammate(pair: usize, (side, index): (u8, u8)) -> Teammate {
    let unit = word(pair, 0)
        .and_then(|sim| {
            word(
                sim as usize,
                SIGNAL_SIM_UNITS + (usize::from(side) * 5 + usize::from(index)) * 8,
            )
        })
        .map(|u| u as usize);
    let Some(unit) = unit.filter(|u| *u != 0) else {
        return Teammate::default();
    };
    let hp = word(unit, SIGNAL_UNIT_HP)
        .zip(word(unit, SIGNAL_UNIT_MAX_HP))
        .and_then(|(hp, max)| (max != 0).then(|| hp.saturating_mul(100) / max));
    Teammate {
        hp,
        speed: word(unit, SIGNAL_UNIT_SPEED),
        pos: word(unit, SIGNAL_UNIT_POS).zip(word(unit, SIGNAL_UNIT_POS + 8)),
    }
}
#[allow(clippy::too_many_arguments)]
unsafe extern "system" fn heard_hook_1(
    brain: usize,
    context: usize,
    unknown: usize,
    player: usize,
    pair: usize,
    sender: usize,
    at: usize,
    extra: usize,
) -> u64 {
    heard(1, brain, context, unknown, player, pair, sender, at, extra)
}
#[allow(clippy::too_many_arguments)]
unsafe extern "system" fn heard_hook_2(
    brain: usize,
    context: usize,
    unknown: usize,
    player: usize,
    pair: usize,
    sender: usize,
    at: usize,
    extra: usize,
) -> u64 {
    heard(2, brain, context, unknown, player, pair, sender, at, extra)
}

#[cfg(feature = "research")]
#[allow(clippy::too_many_arguments)]
unsafe extern "system" fn scene_hook(
    brain: usize,
    context: usize,
    unknown: usize,
    player: usize,
    pair: usize,
    sender: usize,
    reason: usize,
    target: usize,
    extra: usize,
) -> u64 {
    let original: NineFn = std::mem::transmute(SCENE.load(Ordering::Acquire));
    let mark = enter();
    let result = original(
        brain, context, unknown, player, pair, sender, reason, target, extra,
    );
    finish(mark, || {
        let sender = sender as u32;
        let reason = reason as u8;
        let to = player_of(player);
        let (now, _, _) = pair_reads(pair);
        // Reason 2 is BattleHelp's, the mod's own kind; whose it is comes
        // from the handler's call this one is nested in.
        let mine = if reason == REASON_BATTLE_HELP {
            HEARD_MINE.with(Cell::get)
        } else {
            Mine::No
        };
        Some((
            fate::scene_line(to, sender, reason, target as u64, now, mine, false),
            mine.forced(),
        ))
    });
    result
}
