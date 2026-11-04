//! The ping system's orders, the hook side (1.7.1-dev13; the pure half is
//! `native_adapter/ping_orders.rs`).
//!
//! Called from the step hook (`signal_hook.rs`) at a brain's first step in
//! a tick: after the game's broadcast, before that step's handler and
//! cascade, on the viewed match's worker, inside the hook's
//! `catch_unwind`. It acts only on the controlled athlete's teammates
//! (never his own brain, never an enemy's), and only on the brain whose
//! step it is. Nothing the game passes in is kept.
//!
//! What it changes in the game, each only for an order the player gave:
//! - the objective, through the game's own setter, only while that brain's
//!   tracing is off (with tracing off the setter calls nothing and
//!   allocates nothing, read from the code);
//! - for a fog ping (1.7.1-dev15), one enemy's entry in the vision table:
//!   the point, seen and checked now, the squared distance from the
//!   teammate's unit (never the Mia stamps or the hp ratios; until
//!   1.7.1-dev21 the research key F9 added that enemy's Mia stamp);
//! - for a driven window (1.7.1-dev17, `ping_orders::Drive`), in AI kind 0
//!   only: one or two bytes of the PassiveLine plan (`hold_wave`, `waiting`,
//!   `regroup_now`) or the fight plan's run-away latch (two words), each
//!   written only while the big plan read at this step is the variant that
//!   owns the field. No game function is called for them and nothing is
//!   allocated.
//!
//! - for a take order at a pit (1.7.1-dev18, the author's spec section
//!   11.9), at most one plan rebuild per teammate and order through the
//!   game's own replace helper with ForcePassive, only while the plan is
//!   not a Battle and that brain's tracing is off (the helper then drops
//!   the old plan and copies the buffer, allocating nothing; read from the
//!   code). The team's play is read first ([`play`]): who publishes it,
//!   whether the team is on it, whether the teammate hunts and whether the
//!   game's own walk rule would let him walk.
//!
//! Superseded 2026-11-04 (1.7.1-dev17): the inbox calls (stop, Mia, the
//! hide-lane call) are no longer written.
//!
//! The setter's and the helper's opening bytes are checked once, at
//! install ([`check_setter`]).
//!
//! Research builds log each of the controlled side's brains' play fields
//! once a second and at every order write (`OBJ play`, spec section 11.9).

use super::super::signals::*;
use super::signal_hook::{word, STEP_IN};
use super::*;
use crate::native_adapter::ping_orders::{Facts, Flag, Objective, PlanKind, Play, Think, Write};
use crate::native_adapter::signal_call as call;
use crate::native_adapter::PingReach;

type SetterFn = unsafe extern "system" fn(usize, u32, usize);
type ReplaceFn = unsafe extern "system" fn(usize, usize, usize, usize);
/// The setter's opening bytes matched this version's (checked once, at
/// install).
static SETTER_OK: AtomicBool = AtomicBool::new(false);
/// The plan replace helper's opening bytes matched (1.7.1-dev18).
static REPLACE_OK: AtomicBool = AtomicBool::new(false);
/// The Location the setter takes: read only when tracing is on, when it is
/// never called. The mod's own, zeroed.
static LOCATION: [u64; 3] = [0; 3];

/// The game's value for an enemy whose unit is missing (0xecc09e).
pub(super) const FAR: u64 = 1_000_000_000_000;

/// The unit of player (side, index): `[sim + units + (side * 5 + index) *
/// 8]`, when there is one.
pub(super) unsafe fn unit(simulation: usize, side: u8, index: u8) -> Option<usize> {
    let slot = (usize::from(side) * 5 + usize::from(index)) * 8;
    word(simulation, SIGNAL_SIM_UNITS.wrapping_add(slot))
        .map(|u| u as usize)
        .filter(|u| *u != 0 && u.is_multiple_of(8))
}
pub(super) unsafe fn position(simulation: usize, side: u8, index: u8) -> Option<(u64, u64)> {
    let u = unit(simulation, side, index)?;
    word(u, SIGNAL_UNIT_POS).zip(word(u, SIGNAL_UNIT_POS.wrapping_add(8)))
}
/// The brain's last-seen tick of each enemy (0 when unreadable).
unsafe fn seen(brain: usize) -> [u64; 5] {
    std::array::from_fn(|k| word(brain, SIGNAL_VISION_SEEN.wrapping_add(k * 8)).unwrap_or(0))
}
/// The fog ping: enemy `k`'s vision entry on this brain.
unsafe fn believe(
    brain: usize,
    simulation: usize,
    (side, index): (u8, u8),
    k: u8,
    at: (u64, u64),
    tick: u64,
) {
    use crate::native_adapter::ping_orders::{squared_distance, vision_writes};
    let dist = position(simulation, side, index).map_or(FAR, |own| squared_distance(own, at));
    let [pos, seen, checked, distance] = vision_writes(
        k,
        [
            SIGNAL_VISION_POS,
            SIGNAL_VISION_SEEN,
            SIGNAL_VISION_CHECKED,
            SIGNAL_VISION_DISTANCE,
        ],
    )
    .map(|(offset, _)| offset);
    let put = |offset: usize, value: u64| {
        std::ptr::write_unaligned(brain.wrapping_add(offset) as *mut u64, value);
    };
    put(pos, at.0);
    put(pos.wrapping_add(8), at.1);
    put(seen, tick);
    put(checked, tick);
    put(distance, dist);
}

/// Checks the setter's and the plan replace helper's opening bytes; the
/// log line.
pub(super) unsafe fn check_setter(base: usize) -> String {
    let matches = |at: usize, prologue: &[u8]| {
        std::slice::from_raw_parts(base.wrapping_add(at) as *const u8, prologue.len()) == prologue
    };
    let replace = SIGNAL_PLAN_SIZE == PLAN_BUFFER * 8
        && matches(SIGNAL_PLAN_REPLACE, &SIGNAL_PLAN_REPLACE_PROLOGUE);
    REPLACE_OK.store(replace, Ordering::Release);
    let rebuild = if replace {
        format!("; plan rebuild ready at {SIGNAL_PLAN_REPLACE:#x}")
    } else {
        format!("; plan helper {SIGNAL_PLAN_REPLACE:#x} differs: no rebuilds")
    };
    if matches(SIGNAL_OBJECTIVE_SETTER, &SIGNAL_OBJECTIVE_PROLOGUE) {
        SETTER_OK.store(true, Ordering::Release);
        format!("PING objective setter ready at {SIGNAL_OBJECTIVE_SETTER:#x}{rebuild}")
    } else {
        format!(
            "PING objective setter {SIGNAL_OBJECTIVE_SETTER:#x} differs: objectives are not set{rebuild}"
        )
    }
}
/// The plan buffer the helper copies, in u64 words (0x400 bytes).
const PLAN_BUFFER: usize = 128;
/// One plan rebuild through the game's helper: ForcePassive, which the
/// handler rebuilds from the objective after the cascade. Never on a
/// Battle plan, never while tracing is on. The plan's raw tag before.
unsafe fn rebuild(brain: usize) -> Result<u64, &'static str> {
    let Some(base) = verified_base().filter(|_| REPLACE_OK.load(Ordering::Acquire)) else {
        return Err("helper not checked");
    };
    if std::ptr::read_unaligned(brain.wrapping_add(SIGNAL_TRACE_ON) as *const u8) != 0 {
        return Err("tracing on");
    }
    let raw = word(brain, SIGNAL_BIG_PLAN).ok_or("no plan")?;
    if plan_kind(raw) == PlanKind::Battle {
        return Err("fighting");
    }
    let mut buffer = [0u64; PLAN_BUFFER];
    buffer[0] = SIGNAL_FORCE_PASSIVE;
    let replace: ReplaceFn = std::mem::transmute(base.wrapping_add(SIGNAL_PLAN_REPLACE));
    replace(
        brain.wrapping_add(SIGNAL_TEAM_PLAN),
        brain.wrapping_add(SIGNAL_BIG_PLAN),
        buffer.as_ptr() as usize,
        LOCATION.as_ptr() as usize,
    );
    Ok(raw)
}
/// The AI environment's kind-0 object (the monsters' spawn ticks), when
/// the environment is kind 0 (directly or inside the wrapper).
unsafe fn env_object(simulation: usize) -> Option<usize> {
    use crate::native_adapter::signal_fate::{env_kind, ENV_WRAPPER};
    let base = verified_base()?;
    let rva = |v: u64| (v as usize).checked_sub(base);
    let mut data = word(simulation, 0)? as usize;
    let mut kind = env_kind(SIGNAL_ENV_VTABLES, rva(word(simulation, 8)?)?)?;
    if kind == ENV_WRAPPER {
        kind = env_kind(SIGNAL_ENV_VTABLES, rva(word(data, 8)?)?)?;
        data = word(data, 0)? as usize;
    }
    (kind == 0 && data != 0 && data.is_multiple_of(8)).then(|| data + SIGNAL_ENV_OBJECT)
}
/// What the hook has in hand at a step besides the brain.
#[derive(Clone, Copy, Debug)]
pub(super) struct Site {
    pub simulation: usize,
    /// The step's context (the shared blocks at + 0x10).
    pub context: usize,
    /// The receiver state (the world at + 0x20).
    pub receiver: usize,
}
/// The team's shared block for `side`, when the context is the one whose
/// first word is this simulation (as the publisher pick reads it).
unsafe fn shared_block(site: Site, side: u8) -> Option<usize> {
    if side > 1 || word(site.context, 0)? as usize != site.simulation {
        return None;
    }
    let blocks = word(site.context, SIGNAL_CONTEXT_SHARED)? as usize;
    (blocks != 0 && blocks.is_multiple_of(8))
        .then(|| blocks + usize::from(side) * SIGNAL_SHARED_STRIDE)
}
/// The team's play as this teammate's step shows it (spec section 11.9).
unsafe fn play(brain: usize, site: Site, (side, index): (u8, u8), tick: u64, env: Env) -> Play {
    let follower = Some(byte(brain, SIGNAL_SYNC_FOLLOWER) != 0);
    let raw = word(brain, SIGNAL_BIG_PLAN);
    let sync = shared_block(site, side).filter(|b| word(*b, SIGNAL_SHARED_SYNC) != Some(u64::MAX));
    let team_on: [bool; 2] = std::array::from_fn(|k| {
        sync.is_some_and(|b| byte(b, SIGNAL_SHARED_RECORDS[k]) != SIGNAL_RECORD_NONE)
    });
    let hunting: [bool; 2] = std::array::from_fn(|k| raw == Some(SIGNAL_PLAN_PLAY[k]));
    let walk = walk_rule(site, (side, index), tick, env);
    Play {
        follower,
        team_on,
        hunting,
        walk,
    }
}
/// Side `side`'s rally point for monster `k` (Serpen 0, Morgard 1) in the
/// world object, as the builder reads it.
unsafe fn rally_at(world: usize, side: u8, k: usize) -> Option<(u64, u64)> {
    let at = world + SIGNAL_RALLY[k] + usize::from(side) * SIGNAL_RALLY_STRIDE;
    word(at, 0).zip(word(at, 8))
}
/// The world object at `[receiver + 0x20]`, when the builder would take its
/// usual branch there (its dword + 0x7168 is 0).
unsafe fn world_of(receiver: usize) -> Option<usize> {
    let world = word(receiver, SIGNAL_RECEIVER_WORLD)? as usize;
    (world != 0
        && world.is_multiple_of(8)
        && std::ptr::read_unaligned((world + SIGNAL_WORLD_OTHER_RALLY) as *const u32) == 0)
        .then_some(world)
}
/// The game's clock for both pits, once a second, for the client
/// (1.7.1-dev22, spec section 15.3): each monster up (Serpen's alive word,
/// or its spawn tick reached), the ticks until it spawns, and side
/// `side`'s rally point for it (the walk's target, section 15.2). Kind 0
/// only; nothing when the object is unread.
unsafe fn publish_pits(site: Site, side: u8, tick: u64, env: Env) {
    use std::sync::atomic::AtomicU64;
    static LAST: AtomicU64 = AtomicU64::new(u64::MAX);
    let Some(tps) = env.tps.filter(|t| (1..=1000).contains(t) && env.kind0) else {
        return;
    };
    let last = LAST.load(Ordering::Relaxed);
    if last != u64::MAX && tick >= last && tick < last + tps {
        return;
    }
    LAST.store(tick, Ordering::Relaxed);
    let Some(object) = env_object(site.simulation) else {
        return;
    };
    let world = world_of(site.receiver);
    // By the client's pit tag: Morgard 0, Serpen 1 (`places`).
    let clock = |k: usize| {
        let spawn = word(object, SIGNAL_SPAWN_TICK[k])?;
        let alive = k == 0 && word(object, SIGNAL_SERPEN_ALIVE).is_some_and(|w| w != 0);
        let spawn_in = spawn.saturating_sub(tick);
        Some(crate::pings::places::Clock {
            up: alive || spawn_in == 0,
            spawn_in: if alive { 0 } else { spawn_in },
            rally: world.and_then(|w| rally_at(w, side, k)),
        })
    };
    crate::pings::places::publish([clock(1), clock(0)], tps);
}
/// The builder's walk rule per monster (Serpen, Morgard): the monster up
/// (Serpen), or its spawn within this unit's travel to the side's rally
/// point plus 2 s. False whenever something is unread or the builder would
/// take its other branch.
unsafe fn walk_rule(site: Site, (side, index): (u8, u8), tick: u64, env: Env) -> [bool; 2] {
    let tps = env.tps.filter(|t| (1..=1000).contains(t));
    let (Some(tps), Some(object), Some(unit)) = (
        tps.filter(|_| env.kind0),
        env_object(site.simulation),
        unit(site.simulation, side, index),
    ) else {
        return [false; 2];
    };
    let Some(world) = world_of(site.receiver) else {
        return [false; 2];
    };
    let (Some(x), Some(y)) = (
        word(unit, SIGNAL_UNIT_POS),
        word(unit, SIGNAL_UNIT_POS.wrapping_add(8)),
    ) else {
        return [false; 2];
    };
    let speed = word(unit, SIGNAL_UNIT_SPEED).unwrap_or(0).max(1);
    std::array::from_fn(|k| {
        let (Some((rx, ry)), Some(spawn)) =
            (rally_at(world, side, k), word(object, SIGNAL_SPAWN_TICK[k]))
        else {
            return false;
        };
        let alive = k == 0 && word(object, SIGNAL_SERPEN_ALIVE).is_some_and(|w| w != 0);
        let (dx, dy) = (u128::from(x.abs_diff(rx)), u128::from(y.abs_diff(ry)));
        let distance = ((dx * dx + dy * dy) as f64).sqrt() as u64;
        let travel = distance / speed;
        alive || spawn.saturating_sub(tick) <= travel + SIGNAL_WALK_SLACK * tps
    })
}
/// What the orders can reach on this executable (before the mapping).
pub(in crate::native_adapter) fn ping_reach() -> PingReach {
    let calls = installed(family::SIGNALS) && STEP_IN.load(Ordering::Acquire);
    PingReach {
        calls,
        objectives: calls && SETTER_OK.load(Ordering::Acquire),
    }
}

/// The objective (3 bytes at brain + objective).
pub(super) unsafe fn objective(brain: usize) -> Objective {
    Objective(std::ptr::read_unaligned(
        brain.wrapping_add(SIGNAL_OBJECTIVE) as *const [u8; 3],
    ))
}
/// The objective handlers' drop reason byte and the tick it was written.
unsafe fn reason(brain: usize) -> (u8, u64) {
    let byte = std::ptr::read_unaligned(brain.wrapping_add(SIGNAL_OBJECTIVE_REASON) as *const u8);
    (byte, word(brain, SIGNAL_OBJECTIVE_REASON_TICK).unwrap_or(0))
}
/// The objective through the game's setter, unless tracing is on or the
/// setter was not checked. The objective as it reads after the call.
pub(super) unsafe fn set_objective(brain: usize, o: Objective) -> Result<Objective, &'static str> {
    let Some(base) = verified_base().filter(|_| SETTER_OK.load(Ordering::Acquire)) else {
        return Err("setter not checked");
    };
    let tracing = std::ptr::read_unaligned(brain.wrapping_add(SIGNAL_TRACE_ON) as *const u8);
    if tracing != 0 {
        return Err("tracing on");
    }
    let setter: SetterFn = std::mem::transmute(base.wrapping_add(SIGNAL_OBJECTIVE_SETTER));
    setter(
        brain.wrapping_add(SIGNAL_TEAM_PLAN),
        o.u24(),
        LOCATION.as_ptr() as usize,
    );
    Ok(objective(brain))
}

/// The AI environment and clock at a step, from the hook: the AI kind is
/// 0, the receiver's mode byte, and the match's ticks per second (the word
/// at `[[receiver + 8] + SIGNAL_STATE_DELAY]`, which the game multiplies
/// for its own 5 s and 30 s).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Env {
    pub kind0: bool,
    pub mode: Option<u8>,
    pub tps: Option<u64>,
}

/// One brain's step: the orders owed to it. Lines go to `out`.
pub(super) unsafe fn step(
    brain: usize,
    site: Site,
    (side, index): (u8, u8),
    tick: u64,
    env: Env,
    out: &mut Vec<String>,
) {
    let simulation = site.simulation;
    // Only the controlled athlete's side, and orders only for teammates;
    // checked before any read.
    let map = call::with(|s| s.owners.mapping).flatten();
    if !map.is_some_and(|m| m.side == side) {
        return;
    }
    research_play(brain, site, (side, index), tick, env, false, out);
    publish_pits(site, side, tick, env);
    if map.is_some_and(|m| m.index == index) {
        return;
    }
    let facts = Facts {
        tick,
        objective: objective(brain),
        reason: reason(brain),
        seen: seen(brain),
        battle: word(brain, SIGNAL_BIG_PLAN).is_none_or(|raw| plan_kind(raw) == PlanKind::Battle),
        // The play's fields only while he is owed a pit order.
        play: if call::with(|s| s.pings.owes_pit(index)).unwrap_or(false) {
            play(brain, site, (side, index), tick, env)
        } else {
            Play::default()
        },
    };
    let Some(plan) = call::with(|s| {
        s.pings.set_fighting(index, facts.battle);
        s.pings.step(index, facts)
    }) else {
        return;
    };
    out.extend(plan.lines);
    if let Some((k, at)) = plan.vision {
        believe(brain, simulation, (side, index), k, at, tick);
    }
    if let Some(o) = plan.set {
        match set_objective(brain, o) {
            Ok(now) => {
                let after = reason(brain);
                let _ = call::with(|s| s.pings.written(index, after));
                out.push(format!(
                    "PING set n={} team={side} to={index} objective={} reads={} was={} reason={:#04x} t={tick}",
                    plan.serial,
                    o.text(),
                    now.text(),
                    facts.objective.text(),
                    after.0
                ));
            }
            Err(why) => {
                let _ = call::with(|s| s.pings.not_written(index));
                out.push(format!(
                    "PING set skipped n={} team={side} to={index}: {why}",
                    plan.serial
                ));
            }
        }
    }
    if plan.rebuild {
        out.push(match rebuild(brain) {
            Ok(before) => format!(
                "PING rebuild n={} team={side} to={index} plan={before} now={} objective={} t={tick}",
                plan.serial,
                word(brain, SIGNAL_BIG_PLAN).map_or("?".to_owned(), |p| p.to_string()),
                objective(brain).text()
            ),
            Err(why) => format!(
                "PING rebuild skipped n={} team={side} to={index}: {why}",
                plan.serial
            ),
        });
    }
    if plan.set.is_some() || plan.rebuild {
        research_play(brain, site, (side, index), tick, env, true, out);
    }
    if plan.rebuild {
        // The builder decides later in this think: his next think shows it.
        research_next(index);
    }
    drive(brain, simulation, index, tick, env, out);
}

/// The big plan's kind by this version's tags.
fn plan_kind(raw: u64) -> PlanKind {
    if SIGNAL_PLAN_BATTLE.contains(&raw) {
        PlanKind::Battle
    } else if raw == SIGNAL_PLAN_PASSIVE_LINE {
        PlanKind::PassiveLine
    } else {
        PlanKind::Other
    }
}
unsafe fn byte(brain: usize, offset: usize) -> u8 {
    std::ptr::read_unaligned(brain.wrapping_add(offset) as *const u8)
}
/// PassiveLine's `line` offset (its entry of `SIGNAL_PLAN_LANES`).
fn line_offset() -> Option<usize> {
    SIGNAL_PLAN_LANES
        .iter()
        .find(|(tag, _)| *tag == SIGNAL_PLAN_PASSIVE_LINE)
        .map(|(_, at)| *at)
}
/// A flag's brain offset.
fn flag_offset(flag: Flag) -> usize {
    match flag {
        Flag::HoldWave => SIGNAL_PLAN_HOLD_WAVE,
        Flag::Waiting => SIGNAL_PLAN_WAITING,
        Flag::RegroupNow => SIGNAL_PLAN_REGROUP_NOW,
    }
}
/// The game's bottom-support rule decides `hold_wave` itself at this think
/// (the handler's kind-0 gates, `SIGNAL_SUPPORT_*`). Its unit gate is not
/// mirrored, so this says yes a little more often than the game does.
unsafe fn support_rule(
    brain: usize,
    simulation: usize,
    index: u8,
    env: Env,
    plan: PlanKind,
    line: u8,
) -> bool {
    let mode = env
        .mode
        .is_some_and(|m| m <= 8 && SIGNAL_SUPPORT_MODES >> m & 1 == 1);
    let sim = SIGNAL_SUPPORT_SIM_WORDS
        .iter()
        .any(|at| word(simulation, *at) == Some(0));
    let o = objective(brain);
    let own_gank = matches!(o.tag(), Objective::GANK | Objective::DIVE) && o.0[1] == line;
    index == SIGNAL_SUPPORT_SLOT
        && env.kind0
        && mode
        && sim
        && plan == PlanKind::PassiveLine
        && byte(brain, SIGNAL_PLAN_SPLIT_ROLE) == 0
        && !own_gank
}
/// The driven windows at this think: read the plan and its fields, ask the
/// pure half, write what it returns while the plan is still the owner.
unsafe fn drive(
    brain: usize,
    simulation: usize,
    index: u8,
    tick: u64,
    env: Env,
    out: &mut Vec<String>,
) {
    let (Some(raw), Some(line_at)) = (word(brain, SIGNAL_BIG_PLAN), line_offset()) else {
        return;
    };
    let plan = plan_kind(raw);
    let line = byte(brain, line_at);
    let think = Think {
        tick,
        plan,
        raw_plan: raw,
        hold_wave: byte(brain, SIGNAL_PLAN_HOLD_WAVE),
        waiting: byte(brain, SIGNAL_PLAN_WAITING),
        regroup_now: byte(brain, SIGNAL_PLAN_REGROUP_NOW),
        line,
        push_basis: byte(brain, SIGNAL_PLAN_PUSH_BASIS),
        latch: (
            word(brain, SIGNAL_BATTLE_RUNAWAY).unwrap_or(0),
            word(brain, SIGNAL_BATTLE_RUNAWAY_TICK).unwrap_or(0),
        ),
        support_rule: support_rule(brain, simulation, index, env, plan, line),
        tps: env.tps.filter(|t| (1..=1000).contains(t)).unwrap_or(0),
        kind0: env.kind0,
    };
    let Some((writes, lines)) = call::with(|s| s.pings.drive(index, think)) else {
        return;
    };
    for w in writes {
        // Only while the plan read at this step owns the field: read again
        // before each write (the pure half returns no other write).
        match word(brain, SIGNAL_BIG_PLAN) {
            Some(raw) if plan_kind(raw) == w.owner() => {}
            _ => continue,
        }
        let put_word = |offset: usize, value: u64| {
            std::ptr::write_unaligned(brain.wrapping_add(offset) as *mut u64, value);
        };
        match w {
            Write::Flag(flag, value) => {
                std::ptr::write_unaligned(brain.wrapping_add(flag_offset(flag)) as *mut u8, value)
            }
            Write::Latch(until) => {
                put_word(SIGNAL_BATTLE_RUNAWAY_TICK, until);
                put_word(SIGNAL_BATTLE_RUNAWAY, 1);
            }
            Write::Unlatch => put_word(SIGNAL_BATTLE_RUNAWAY, 0),
        }
    }
    out.extend(lines);
}

/// Research builds: each brain's last `OBJ play` tick by roster index
/// (`u64::MAX`: log at his next think).
#[cfg(feature = "research")]
static LAST: [std::sync::atomic::AtomicU64; 5] =
    [const { std::sync::atomic::AtomicU64::new(0) }; 5];
/// Research builds: teammate `index`'s next think logs his play line.
#[cfg_attr(not(feature = "research"), allow(unused_variables))]
fn research_next(index: u8) {
    #[cfg(feature = "research")]
    if let Some(last) = LAST.get(usize::from(index)) {
        last.store(u64::MAX, Ordering::Relaxed);
    }
}
/// Research builds (spec section 11.9, item 4): one brain's play fields,
/// once a second per brain and at every order write (`write`):
/// `sync_follower`, the shared sync tag and its two record None bytes, the
/// brain's own None bytes, the objective, the reason byte and its tick, the
/// plan tag and the walk rule.
#[cfg_attr(not(feature = "research"), allow(unused_variables, clippy::ptr_arg))]
unsafe fn research_play(
    brain: usize,
    site: Site,
    (side, index): (u8, u8),
    tick: u64,
    env: Env,
    write: bool,
    out: &mut Vec<String>,
) {
    #[cfg(feature = "research")]
    {
        let Some(last) = LAST.get(usize::from(index)) else {
            return;
        };
        let second = env.tps.filter(|t| (1..=1000).contains(t)).unwrap_or(60);
        let before = last.load(Ordering::Relaxed);
        if !write && tick >= before && tick < before.saturating_add(second) {
            return;
        }
        last.store(tick, Ordering::Relaxed);
        let block = shared_block(site, side);
        let sync = block.and_then(|b| word(b, SIGNAL_SHARED_SYNC));
        let shared: Vec<String> = SIGNAL_SHARED_RECORDS
            .iter()
            .map(|at| block.map_or("?".to_owned(), |b| byte(b, *at).to_string()))
            .collect();
        let (why, at) = reason(brain);
        let p = play(brain, site, (side, index), tick, env);
        out.push(format!(
            "OBJ play t={tick} team={side} index={index}{} follower={} sync={} shared=[{}] own=[{},{}] objective={} reason={why:#04x}@{at} plan={} walk=[{},{}]",
            if write { " write" } else { "" },
            byte(brain, SIGNAL_SYNC_FOLLOWER),
            sync.map_or("?".to_owned(), |w| (w as i64).to_string()),
            shared.join(","),
            byte(brain, SIGNAL_BRAIN_RECORDS[0]),
            byte(brain, SIGNAL_BRAIN_RECORDS[1]),
            objective(brain).text(),
            word(brain, SIGNAL_BIG_PLAN).map_or("?".to_owned(), |p| p.to_string()),
            u8::from(p.walk[0]),
            u8::from(p.walk[1]),
        ));
    }
}
