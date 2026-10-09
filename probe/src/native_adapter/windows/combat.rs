//! Basic attacks, Q/W/R skills and skill aim: native hooks that observe
//! and steer the controlled athlete, plus ability state reads.
use super::*;

pub(crate) static ORIGINAL_AIM: AtomicUsize = AtomicUsize::new(0);
pub(crate) static ORIGINAL_ATTACK: AtomicUsize = AtomicUsize::new(0);
pub(crate) static ATTACK_TRACE: crate::attack_trace::AttackTrace =
    crate::attack_trace::AttackTrace::new();
pub(crate) static ORIGINAL_SKILLS: [AtomicUsize; 3] = [const { AtomicUsize::new(0) }; 3];
pub(crate) type AttackFn = unsafe extern "system" fn(usize, usize, usize);
pub(crate) type AimFn = unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, usize);
pub(crate) static PRESERVED_AIMS: AtomicUsize = AtomicUsize::new(0);
/// Read from the current SDK simulation borrow, not the previous consumer hook.
/// SDK entity_pos (2e16650) uses this shared-borrow +208 getter and these fields.
pub(crate) unsafe fn attack_ranges(
    state: usize,
    table: usize,
    actor: usize,
) -> Option<(u64, Option<u64>)> {
    let shared = SHARED.get()?;
    let base = verified_base()?;
    owned_key(shared, actor)?;
    if state == 0
        || table == 0
        || std::ptr::read_unaligned(table as *const usize) < 0x50
        || std::ptr::read_unaligned((table + 0x48) as *const usize) != base + 0x2e16650
    {
        return None;
    }
    let object = std::ptr::read_unaligned(state as *const usize);
    let vtable = std::ptr::read_unaligned((state + 8) as *const usize);
    if object == 0 || vtable == 0 {
        return None;
    }
    let get: unsafe extern "system" fn(usize, usize) -> usize =
        std::mem::transmute(std::ptr::read_unaligned((vtable + 0x208) as *const usize));
    let entity = get(object, actor);
    if entity == 0 || owned_entity(shared, entity).map(|(_, id)| id) != Some(actor) {
        return None;
    }
    let current = read_effect_metadata(entity, 0x488)?.range;
    let maximum = read_attack_maximum(entity, crate::acquisition::cap());
    Some((current, maximum))
}
/// The same fingerprinted AA descriptor used above: no new pointer or hook.
pub(crate) unsafe fn read_attack_maximum(entity: usize, cap: Option<u64>) -> Option<u64> {
    crate::acquisition::maximum(
        std::ptr::read_unaligned((entity + 0x498) as *const u64),
        std::ptr::read_unaligned((entity + 0x4a0) as *const u64),
        cap,
    )
}
pub(crate) unsafe fn manual_aim_words(
    shared: &Shared,
    actor: usize,
    target: usize,
) -> Option<(crate::native_timing::MatchKey, [usize; 3])> {
    let key = owned_key(shared, actor)?;
    if target == 0 || !target.is_multiple_of(8) {
        return None;
    }
    let words = std::ptr::read_unaligned(target as *const [usize; 3]);
    // Low dword is the tag; upper dword is enum padding.
    matches!(words[0] as u32, 1 | 2).then_some((key, words))
}
/// The native routine borrows the POD target only for this call. Run its
/// stat/RNG evaluation normally, but give it a copy for manual ground/direction
/// casts. The effect below the call still receives the original cursor aim.
pub(crate) unsafe fn evaluate_aim(
    original: AimFn,
    mut args: [usize; 7],
    preserve: bool,
) -> Option<[usize; 3]> {
    if !preserve {
        original(
            args[0], args[1], args[2], args[3], args[4], args[5], args[6],
        );
        return None;
    }
    let mut copy = std::ptr::read_unaligned(args[6] as *const [usize; 3]);
    args[6] = copy.as_mut_ptr() as usize;
    original(
        args[0], args[1], args[2], args[3], args[4], args[5], args[6],
    );
    Some(copy)
}
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe extern "system" fn aim_hook(
    rng: usize,
    sim: usize,
    table: usize,
    navigation: usize,
    actor: usize,
    effect: usize,
    target: usize,
) {
    crate::perf::hook(crate::perf::Hook::Aim);
    let original: AimFn = std::mem::transmute(ORIGINAL_AIM.load(Ordering::Acquire));
    let manual = catch_unwind(AssertUnwindSafe(|| {
        let shared = SHARED.get()?;
        PATCHES.get()?;
        manual_aim_words(shared, actor, target)
    }))
    .unwrap_or(None);
    let corrected = evaluate_aim(
        original,
        [rng, sim, table, navigation, actor, effect, target],
        manual.is_some(),
    );
    if let (Some((key, before)), Some(corrected)) = (manual, corrected) {
        let count = PRESERVED_AIMS.fetch_add(1, Ordering::Relaxed) + 1;
        if (before[1..] != corrected[1..] || count <= 3) && SHARED.get().is_some() {
            let _ = catch_unwind(AssertUnwindSafe(|| {
                SHARED.get().unwrap().logger.write(&format!(
                    "AIM PRESERVED actor={actor} key={key:?} kind={} cursor=({},{}) native_suggestion=({},{}) count={count}",
                    before[0] as u32, before[1] as i64, before[2] as i64,
                    corrected[1] as i64, corrected[2] as i64,
                ));
            }));
        }
    }
}
/// Copy scalar state from the live worker only; never retain native objects.
pub(crate) unsafe fn observe_owned_abilities(
    shared: &Shared,
    entity: usize,
    key: crate::native_timing::MatchKey,
    actor: usize,
) -> bool {
    if owned_entity(shared, entity) != Some((key, actor)) {
        return false;
    }
    let _profile = crate::perf::work(crate::perf::Work::OwnedAbilities);
    if crate::logging::verbose() {
        for line in ATTACK_TRACE.sample(key, actor, attack_snapshot(entity)) {
            shared.logger.write(&line);
        }
    }
    // Use the same borrowed state for readiness and the native action/count
    // snapshot, including immediately after a consumer spends cooldown.
    shared.abilities.observe_actor(
        key,
        Some(actor),
        Some((
            std::ptr::read_unaligned((entity + 0x658) as *const u64),
            std::ptr::read_unaligned((entity + 0x660) as *const u64),
        )),
        [0xb8, 0xc0, 0xc8].map(|o| std::ptr::read_unaligned((entity + o) as *const usize)),
    );
    shared.abilities.observe_level(
        key,
        actor,
        std::ptr::read_unaligned((entity + 0x5c0) as *const usize),
    );
    let metadata = read_ability_metadata(entity);
    shared
        .abilities
        .observe_metadata(key, actor, metadata, &shared.logger);
    if let Some(base) = verified_base().filter(|_| shared.abilities.geometry_due(key, actor)) {
        let offsets = [0x4c0, 0x4f8, 0x530];
        let geometry = std::array::from_fn(|slot| {
            if metadata[slot].is_some() {
                crate::native_preview::read(base, entity, offsets[slot])
            } else {
                crate::skill_preview::Geometry::default()
            }
        });
        shared
            .abilities
            .observe_geometry(key, actor, geometry, &shared.logger);
    }
    let charges = [0x578, 0x588, 0x598].map(|offset| read_charges(entity, offset));
    if shared.abilities.observe_native(
        key,
        actor,
        std::ptr::read_unaligned((entity + 0x70) as *const usize),
        charges,
    ) {
        shared.logger.write(&format!("ABILITY CHARGES actor={actor} Q/W/R={charges:?}; declared use count, not inferred capacity/cost"));
    }
    shared.movement.observe_action(
        std::ptr::read_unaligned((entity + 0x70) as *const usize),
        &shared.logger,
    );
    shared.movement.observe_attack_range(
        key,
        read_effect_metadata(entity, 0x488)
            .map(|d| d.range)
            .filter(|r| *r > 0),
        &shared.logger,
    );
    true
}
/// The Q/W/R consumers call these shared-borrow Action methods with
/// (action userdata, EntityData/Stat). Current 0.6.3 consumers establish
/// slots +90 (cooltime), +a8 (use count), CDR +3f8 (+464 for R),
/// minimum Q=3 ticks, W/R=1. No calls for unlearned actions.
/// Neither the object nor its vtable is retained beyond this hook.
pub(crate) unsafe fn read_charges(
    entity: usize,
    offset: usize,
) -> Option<crate::abilities::Charges> {
    type Read = unsafe extern "system" fn(usize, usize) -> usize;
    let level = std::ptr::read_unaligned((entity + 0x5c0) as *const usize);
    let unlock = match offset {
        0x578 => 1,
        0x588 => 3,
        0x598 => 5,
        _ => return None,
    };
    if level < unlock {
        return None;
    }
    let object = std::ptr::read_unaligned((entity + offset) as *const usize);
    let table = std::ptr::read_unaligned((entity + offset + 8) as *const usize);
    if object == 0 || table == 0 {
        return None;
    }
    let cool: Read = std::mem::transmute(std::ptr::read_unaligned((table + 0x90) as *const usize));
    let count: Read = std::mem::transmute(std::ptr::read_unaligned((table + 0xa8) as *const usize));
    let raw = cool(object, entity);
    let uses = count(object, entity);
    let mut cdr = std::ptr::read_unaligned((entity + 0x3f8) as *const i32);
    if offset == 0x598 {
        cdr = cdr.checked_add(std::ptr::read_unaligned((entity + 0x464) as *const i32))?;
    }
    let divisor = cdr.checked_add(100)?.max(1) as usize;
    let capacity = (raw.checked_mul(100)? / divisor).max(if offset == 0x578 { 3 } else { 1 });
    if !(1..=64).contains(&uses) || capacity > 3_600_000 {
        return None;
    }
    let cost = capacity / uses;
    (cost > 0).then_some(crate::abilities::Charges {
        capacity,
        cost,
        uses: if raw == 0 { 1 } else { uses },
    })
}
pub(crate) unsafe fn read_ability_metadata(
    entity: usize,
) -> [Option<crate::abilities::Descriptor>; 3] {
    [0x4c0, 0x4f8, 0x530].map(|offset| read_effect_metadata(entity, offset))
}
pub(crate) unsafe fn read_effect_metadata(
    entity: usize,
    offset: usize,
) -> Option<crate::abilities::Descriptor> {
    let level = std::ptr::read_unaligned((entity + 0x5c0) as *const u64);
    let bonus = std::ptr::read_unaligned((entity + 0x430) as *const u64);
    let data = entity + offset;
    let casting = std::ptr::read_unaligned((data + 0x30) as *const u32);
    if casting > 3 {
        return None;
    }
    crate::abilities::Descriptor::from_fields(
        casting,
        std::ptr::read_unaligned((data + 0x28) as *const u32),
        std::ptr::read_unaligned((data + 0x10) as *const u64),
        std::ptr::read_unaligned((data + 0x18) as *const u64),
        level,
        bonus,
    )
}
pub(crate) unsafe extern "system" fn attack_hook(entity: usize, input: usize, events: usize) {
    attack_hook_from("input", entity, input, events);
}
pub(crate) unsafe extern "system" fn auto_attack_hook(entity: usize, input: usize, events: usize) {
    attack_hook_from("native-auto", entity, input, events);
}
/// Native InputTarget::Target is a POD tag (low dword) and entity ID.
/// Stop only a matching, in-range manual order. The caller always forwards
/// the attack afterwards, so native readiness, windup and damage still decide.
pub(crate) unsafe fn stop_for_manual_attack(
    entity: usize,
    input: usize,
    ticket: StopTicket,
    events: usize,
    notify: StopEventFn,
) -> bool {
    let Some(target) = ticket.attack_target else {
        return false;
    };
    if input == 0
        || std::ptr::read_unaligned(input as *const u32) != 0
        || std::ptr::read_unaligned((input + 8) as *const usize) != target
    {
        return false;
    }
    stop_movement(entity, ticket.actor, events, notify)
}
pub(crate) unsafe fn attack_hook_from(source: &str, entity: usize, input: usize, events: usize) {
    let _profile = crate::perf::work(crate::perf::Work::AttackInclusive);
    crate::perf::hook(crate::perf::Hook::Attack);
    if let Some(shared) = SHARED.get() {
        let _ = catch_unwind(AssertUnwindSafe(|| inventory(shared, entity)));
    }
    let original: AttackFn = std::mem::transmute(ORIGINAL_ATTACK.load(Ordering::Acquire));
    let selected = SHARED.get().and_then(|shared| {
        catch_unwind(AssertUnwindSafe(|| {
            let (key, actor) = owned_entity(shared, entity)?;
            observe_owned_abilities(shared, entity, key, actor);
            if let Some(ticket) = (source == "input")
                .then(|| take_stop_ticket(key, actor))
                .flatten()
            {
                if ticket.cancel_recall {
                    cancel_recall(shared, entity, actor, events);
                }
                let action = std::ptr::read_unaligned((entity + 0x70) as *const usize);
                let notify: StopEventFn =
                    std::mem::transmute(NOTIFY_STOP.load(Ordering::Acquire));
                if stop_for_manual_attack(entity, input, ticket, events, notify) && action == 2 {
                    let cooldown = std::ptr::read_unaligned((entity + 0xb0) as *const usize);
                    shared.logger.write(&format!("ATTACK WAIT_STOP actor={actor} target={:?} in_range=true action=2->0 cooldown={cooldown}; native attack attempt follows", ticket.attack_target));
                }
            }
            Some((
                shared,
                key,
                actor,
                std::ptr::read_unaligned((entity + 0x2b0) as *const usize),
                std::ptr::read_unaligned((entity + 0xb0) as *const usize),
                attack_snapshot(entity),
            ))
        }))
        .unwrap_or_else(|_| {
            shared
                .timing
                .cancel("Ability observation adapter panic", &shared.logger);
            None
        })
    });
    original(entity, input, events);
    if let Some((shared, key, actor, previous_len, before_cooldown, before)) = selected {
        let after = attack_snapshot(entity);
        if after.cooldown > before_cooldown && crate::logging::verbose() {
            let start = crate::attack_trace::Start {
                before,
                after,
                kind: std::ptr::read_unaligned((entity + 0x4b4) as *const u32),
                start_timing: (std::ptr::read_unaligned((entity + 0x4b8) as *const i32) != -1)
                    .then(|| std::ptr::read_unaligned((entity + 0x4a8) as *const u64)),
                speed: std::ptr::read_unaligned((entity + 0x3f4) as *const i32),
                queued_delay: queued_attack_delay(entity, previous_len),
            };
            for line in ATTACK_TRACE.start(key, actor, source, start) {
                shared.logger.write(&line);
            }
        }
        if let Some(hit) = locked_attack_hit_tick(entity, previous_len) {
            let cooldown = std::ptr::read_unaligned((entity + 0xb0) as *const usize);
            shared
                .abilities
                .observe_attack_started(key, actor, hit, cooldown);
            shared.logger.write(&format!("ATTACK START source={source} actor={actor} branch=locked hit_tick={hit} cooldown={cooldown}; release allowed only after the hit tick"));
        } else if std::ptr::read_unaligned((entity + 0xb0) as *const usize) > before_cooldown {
            shared.logger.write(&format!("ATTACK OBSERVED source={source} actor={actor} pending-effect-unconfirmed; preserving native animation"));
        }
        observe_owned_abilities(shared, entity, key, actor);
    }
}
/// Hit tick of a BaseAttack that locked action 3 in this call. The native
/// consumer either locks action 3 without a queue entry or queues a delayed
/// effect without locking (0.50 trace: Ninja locked, Gunner queued). The
/// locked hit is the declared start timing (+4a8, present unless +4b8 is
/// -1) scaled by the action's attack-speed factor (+80 = 100 + bonus).
/// Release later requires elapsed (+78) strictly beyond this tick.
pub(crate) unsafe fn locked_attack_hit_tick(entity: usize, previous_len: usize) -> Option<usize> {
    if std::ptr::read_unaligned((entity + 0x70) as *const usize) != 3
        || std::ptr::read_unaligned((entity + 0x78) as *const usize) != 0
        || std::ptr::read_unaligned((entity + 0x2b0) as *const usize) != previous_len
        || std::ptr::read_unaligned((entity + 0x4b4) as *const u32) != 0
        || std::ptr::read_unaligned((entity + 0x4b8) as *const i32) == -1
    {
        return None;
    }
    let start = std::ptr::read_unaligned((entity + 0x4a8) as *const usize);
    let factor = std::ptr::read_unaligned((entity + 0x80) as *const usize);
    if !(1..=10_000).contains(&factor) || start > 3600 {
        return None;
    }
    let hit = start * 100 / factor;
    (hit < 3600).then_some(hit)
}
/// Delay of the entry appended by this call, if exactly one was appended.
pub(crate) unsafe fn queued_attack_delay(entity: usize, previous_len: usize) -> Option<usize> {
    let ptr = std::ptr::read_unaligned((entity + 0x2a8) as *const usize);
    let cap = std::ptr::read_unaligned((entity + 0x2a0) as *const usize);
    let len = std::ptr::read_unaligned((entity + 0x2b0) as *const usize);
    if ptr == 0
        || !ptr.is_multiple_of(8)
        || cap > 4096
        || len > cap
        || len != previous_len.checked_add(1)?
    {
        return None;
    }
    let delay = std::ptr::read_unaligned((ptr + previous_len * 0x38 + 0x10) as *const usize);
    (delay < 3600).then_some(delay)
}
/// Scalar copies only: action tag, its two payload words, pending-effect
/// queue length and attack cooldown (0.6.3 EntityData layout).
pub(crate) unsafe fn attack_snapshot(entity: usize) -> crate::attack_trace::Snapshot {
    crate::attack_trace::Snapshot {
        action: std::ptr::read_unaligned((entity + 0x70) as *const usize),
        elapsed: std::ptr::read_unaligned((entity + 0x78) as *const usize),
        counter: std::ptr::read_unaligned((entity + 0x80) as *const usize),
        queue: std::ptr::read_unaligned((entity + 0x2b0) as *const usize),
        cooldown: std::ptr::read_unaligned((entity + 0xb0) as *const usize),
    }
}

pub(crate) unsafe fn skill_hook(slot: usize, entity: usize, input: usize, events: usize) {
    let _profile = crate::perf::work(crate::perf::Work::SkillInclusive);
    let original: AttackFn = std::mem::transmute(ORIGINAL_SKILLS[slot].load(Ordering::Acquire));
    let selected = catch_unwind(AssertUnwindSafe(|| {
        SHARED.get().and_then(|shared| {
            let (key, actor) = owned_entity(shared, entity)?;
            observe_owned_abilities(shared, entity, key, actor);
            Some((shared, key, actor))
        })
    }))
    .unwrap_or(None);
    let offset = [0xb8, 0xc0, 0xc8][slot];
    let trace = selected
        .and_then(|(shared, _, _)| {
            shared
                .abilities
                .trace_point_stamp(slot)
                .filter(|_| shared.timing.permit_native_trace())
        })
        .map(|stamp| {
            let words = std::array::from_fn::<_, 3, _>(|i| {
                std::ptr::read_unaligned((input + i * 8) as *const u64)
            });
            let length = std::ptr::read_unaligned((entity + 0x2b0) as *const usize);
            (stamp.id, words, length)
        });
    let before = selected.map(|_| std::ptr::read_unaligned((entity + offset) as *const usize));
    original(entity, input, events);
    if let (Some((shared, key, actor)), Some(before)) = (selected, before) {
        if catch_unwind(AssertUnwindSafe(|| {
            if let Some((trace_id, words, previous_len)) = trace {
                let ptr = std::ptr::read_unaligned((entity + 0x2a8) as *const usize);
                let cap = std::ptr::read_unaligned((entity + 0x2a0) as *const usize);
                let len = std::ptr::read_unaligned((entity + 0x2b0) as *const usize);
                let queued = (ptr != 0 && ptr.is_multiple_of(8) && cap <= 4096 && len <= cap && len == previous_len + 1)
                    .then(|| std::array::from_fn::<_, 3, _>(|i| std::ptr::read_unaligned((ptr + previous_len * 0x38 + 0x20 + i * 8) as *const u64)));
                shared.logger.write(&format!("ABILITY NATIVE_AIM trace_id={trace_id} slot={slot} actor={actor} input_words={words:?} queued_words={queued:?} queue_before={previous_len} queue_after={len}; queued intent, not projectile trajectory"));
            }
            let after = std::ptr::read_unaligned((entity + offset) as *const usize);
            let action = std::ptr::read_unaligned((entity + 0x70) as *const usize);
            shared.abilities.acknowledge_native(
                key,
                actor,
                slot,
                after > before,
                action,
                &shared.logger,
            );
            observe_owned_abilities(shared, entity, key, actor);
        }))
        .is_err()
        {
            shared
                .timing
                .cancel("Native skill acknowledgement panic", &shared.logger);
        }
    }
}
pub(crate) unsafe extern "system" fn skill_q_hook(entity: usize, input: usize, events: usize) {
    crate::perf::hook(crate::perf::Hook::Skill);
    skill_hook(0, entity, input, events);
}
pub(crate) unsafe extern "system" fn skill_w_hook(entity: usize, input: usize, events: usize) {
    crate::perf::hook(crate::perf::Hook::Skill);
    skill_hook(1, entity, input, events);
}
pub(crate) unsafe extern "system" fn skill_r_hook(entity: usize, input: usize, events: usize) {
    crate::perf::hook(crate::perf::Hook::Skill);
    skill_hook(2, entity, input, events);
}

pub(crate) unsafe fn finish_attack_backswing(
    shared: &Shared,
    key: crate::native_timing::MatchKey,
    actor: usize,
    entity: usize,
    events: usize,
) -> bool {
    if !can_stop_action(entity, 3)
        || std::ptr::read_unaligned((entity + 0x4b4) as *const u32) != 0 // BaseAttack only
        || std::ptr::read_unaligned((entity + 0x2b0) as *const usize) != 0
    {
        return false;
    }
    // Player preference (Settings > Combat & casting > Attacks).
    if crate::settings::option("attack_cancel") != 1. {
        return false;
    }
    let elapsed = std::ptr::read_unaligned((entity + 0x78) as *const usize);
    let cooldown = std::ptr::read_unaligned((entity + 0xb0) as *const usize);
    let reason = if shared
        .abilities
        .attack_interrupt(key, actor, elapsed, cooldown)
    {
        "buffered-skill"
    } else if shared
        .abilities
        .committed_attack_start(key, actor, elapsed, cooldown)
        .is_some_and(|start| shared.movement.manual_release_after(start))
    {
        "manual-move-or-stop"
    } else {
        return false;
    };
    let notify: StopEventFn = std::mem::transmute(NOTIFY_STOP.load(Ordering::Acquire));
    notify(events, actor);
    std::ptr::write_unaligned((entity + 0x70) as *mut usize, 0);
    shared.logger.write(&format!("ATTACK COMMITTED_INTERRUPT actor={actor} reason={reason} elapsed={elapsed} cooldown={cooldown}; hit tick passed, empty pending queue, animation released, cooldown retained"));
    true
}

// 0.70 diagnostic: an inventory of skill effect trees, so the remaining
// effect types can be decoded. The viewed match's champions have their own
// budget (every champion, once per match: the roster names them); background
// matches add new tree shapes only.
const VIEWED_LIMIT: usize = 240;
const BACKGROUND_LIMIT: usize = 200;
const BACKGROUND_PER_SHAPE: usize = 2;
static VIEWED_LINES: AtomicUsize = AtomicUsize::new(0);
static BACKGROUND_LINES: AtomicUsize = AtomicUsize::new(0);
static INVENTORY_SHAPES: std::sync::Mutex<Vec<(u64, usize)>> = std::sync::Mutex::new(Vec::new());
thread_local! {
    static INVENTORIED: std::cell::RefCell<std::collections::HashSet<usize>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
}
unsafe fn inventory(shared: &Shared, entity: usize) {
    let viewed = crate::perf::on_worker_thread();
    let (lines, limit) = if viewed {
        (&VIEWED_LINES, VIEWED_LIMIT)
    } else {
        (&BACKGROUND_LINES, BACKGROUND_LIMIT)
    };
    if lines.load(Ordering::Relaxed) >= limit {
        return;
    }
    let fresh = INVENTORIED.with(|seen| {
        let mut seen = seen.borrow_mut();
        if seen.len() >= 4096 {
            seen.clear();
        }
        seen.insert(entity)
    });
    let Some(base) = verified_base().filter(|_| fresh) else {
        return;
    };
    let actor = std::ptr::read_unaligned((entity + 0x5b8) as *const usize);
    for (slot, offset) in [("Q", 0x4c0), ("W", 0x4f8), ("R", 0x530)] {
        let Some(tree) = crate::native_preview::dump(base, entity, offset, &|a, n| {
            super::tooltips::accessible(a, n, false)
        }) else {
            continue;
        };
        if !viewed && !new_background_shape(&tree) {
            continue;
        }
        if lines.fetch_add(1, Ordering::Relaxed) < limit {
            shared.logger.write(&format!(
                "PREVIEW TREE viewed={viewed} actor={actor} slot={slot} {tree}"
            ));
        }
    }
}
/// Background trees: each shape (the tree without payload words) twice.
fn new_background_shape(tree: &str) -> bool {
    let mut shape = String::new();
    let mut inside = false;
    for c in tree.chars() {
        match c {
            '[' => inside = true,
            ']' => inside = false,
            _ if !inside => shape.push(c),
            _ => {}
        }
    }
    let hash = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        shape.hash(&mut h);
        h.finish()
    };
    INVENTORY_SHAPES.lock().is_ok_and(|mut shapes| {
        match shapes.iter_mut().find(|(h, _)| *h == hash) {
            Some((_, n)) if *n >= BACKGROUND_PER_SHAPE => false,
            Some((_, n)) => {
                *n += 1;
                true
            }
            None => {
                shapes.push((hash, 1));
                true
            }
        }
    })
}
