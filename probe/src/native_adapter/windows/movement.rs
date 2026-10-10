//! Movement: the native move and steering hooks, stop and recall.
use super::*;

pub(crate) static ORIGINAL_STEER: AtomicUsize = AtomicUsize::new(0);
pub(crate) static ORIGINAL_DIRECT_STEP: AtomicUsize = AtomicUsize::new(0);
pub(crate) static ORIGINAL_MOVE: AtomicUsize = AtomicUsize::new(0);
pub(crate) static NOTIFY_STOP: AtomicUsize = AtomicUsize::new(0);
pub(crate) static NOTIFY_CANCEL_RECALL: AtomicUsize = AtomicUsize::new(0);
pub(crate) type MoveFn = unsafe extern "system" fn(usize, u64, u64, usize);
pub(crate) type StopEventFn = unsafe extern "system" fn(usize, usize);
pub(crate) type SteerFn =
    unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, usize, u64, u64, usize);
pub(crate) type DirectStepFn =
    unsafe extern "system" fn(usize, usize, usize, usize, usize, u64, u64, usize);
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe extern "system" fn steer_hook(
    world: usize,
    navigation: usize,
    bounds: usize,
    actor: usize,
    x: usize,
    y: usize,
    speed: usize,
    goal_x: u64,
    goal_y: u64,
    events: usize,
) {
    let _watch = crate::worker_watch::hook(crate::worker_watch::Step::Steer);
    STEER_ENTRIES.fetch_add(1, Ordering::Relaxed);
    crate::perf::hook(crate::perf::Hook::Steer);
    let original: SteerFn = std::mem::transmute(ORIGINAL_STEER.load(Ordering::Acquire));
    let direct = catch_unwind(AssertUnwindSafe(|| {
        let shared = SHARED.get()?;
        PATCHES.get()?;
        let key = owned_steering(shared, actor, x, y)?;
        let _profile = crate::perf::work(crate::perf::Work::OwnedSteering);
        OWNED_STEER_ENTRIES.fetch_add(1, Ordering::Relaxed);
        // These fields belong to the current borrowed native entity. No
        // pointer from an earlier input call or published frame is reused.
        let from = (
            std::ptr::read_unaligned(x as *const u64),
            std::ptr::read_unaligned(y as *const u64),
        );
        shared
            .movement
            .direct_segment(key, actor, from, (goal_x, goal_y), &shared.logger)
            .then_some(())
    }))
    .unwrap_or(None)
    .is_some();
    if direct {
        DIRECT_STEPS.fetch_add(1, Ordering::Relaxed);
        let step: DirectStepFn = std::mem::transmute(ORIGINAL_DIRECT_STEP.load(Ordering::Acquire));
        step(world, actor, x, y, speed, goal_x, goal_y, events);
    } else {
        original(
            world, navigation, bounds, actor, x, y, speed, goal_x, goal_y, events,
        );
    }
}

pub(crate) unsafe fn owned_steering(
    shared: &Shared,
    actor: usize,
    x: usize,
    y: usize,
) -> Option<crate::native_timing::MatchKey> {
    // The fingerprinted caller establishes x/y as adjacent EntityData
    // position fields. Recover only this call's current borrow.
    let key = owned_key(shared, actor)?;
    let entity = x.checked_sub(0x658)?;
    if y != x.checked_add(8)? || owned_entity(shared, entity) != Some((key, actor)) {
        let failures = STEER_POINTER_REJECTIONS.fetch_add(1, Ordering::Relaxed);
        if failures < 8 {
            shared.logger.write(&format!("MOVEMENT STEERING_REJECT actor={actor} reason=current-position-fields-mismatch; native navigation retained"));
        }
        return None;
    }
    Some(key)
}
pub(crate) fn take_stop_ticket(
    key: crate::native_timing::MatchKey,
    actor: usize,
) -> Option<StopTicket> {
    STOP_TICKET.with(|slot| {
        slot.get()
            .filter(|t| t.actor == actor && t.key == key)
            .inspect(|_| slot.set(None))
    })
}
pub(crate) unsafe fn can_stop_movement(entity: usize) -> bool {
    can_stop_action(entity, 2)
}
pub(crate) unsafe fn can_stop_action(entity: usize, action: usize) -> bool {
    if std::ptr::read_unaligned((entity + 0x68) as *const u32) != 15
        || std::ptr::read_unaligned((entity + 0x70) as *const usize) != action
    {
        return false;
    }
    let count = std::ptr::read_unaligned((entity + 0x2c8) as *const usize);
    if count > 512 {
        return false;
    }
    let effects = std::ptr::read_unaligned((entity + 0x2c0) as *const usize);
    if count != 0 && effects == 0 {
        return false;
    }
    // Preserve the native effect gate, including forced movement and CC.
    (0..count).all(|i| {
        let kind = std::ptr::read_unaligned((effects + i * 0x28) as *const u32);
        kind < 32 && (0x3b8u32 & (1u32 << kind)) != 0
    })
}
pub(crate) unsafe fn repeated_move(entity: usize, x: u64, y: u64, hold: bool) -> bool {
    !hold
        && can_stop_movement(entity)
        && std::ptr::read_unaligned((entity + 0x78) as *const u64) == x
        && std::ptr::read_unaligned((entity + 0x80) as *const u64) == y
}
pub(crate) unsafe fn stop_movement(
    entity: usize,
    actor: usize,
    events: usize,
    notify: StopEventFn,
) -> bool {
    if std::ptr::read_unaligned((entity + 0x70) as *const usize) == 0 {
        return true;
    }
    if !can_stop_movement(entity) {
        return false;
    }
    notify(events, actor);
    std::ptr::write_unaligned((entity + 0x70) as *mut usize, 0);
    true
}
pub(crate) unsafe fn stop_recall(
    entity: usize,
    actor: usize,
    events: usize,
    notify: StopEventFn,
) -> bool {
    if std::ptr::read_unaligned((entity + 0x68) as *const u32) != 15
        || std::ptr::read_unaligned((entity + 0x70) as *const usize) != 1
    {
        return false;
    }
    notify(events, actor);
    std::ptr::write_unaligned((entity + 0x70) as *mut usize, 0);
    true
}
pub(crate) unsafe fn cancel_recall(shared: &Shared, entity: usize, actor: usize, events: usize) {
    let notify: StopEventFn = std::mem::transmute(NOTIFY_CANCEL_RECALL.load(Ordering::Acquire));
    if stop_recall(entity, actor, events, notify) {
        shared.movement.observe_action(0, &shared.logger);
        shared.logger.write(&format!("RECALL NATIVE_CANCEL actor={actor} action=1->0; native return cancellation event emitted"));
    }
}
pub(crate) unsafe extern "system" fn move_hook(entity: usize, x: u64, y: u64, events: usize) {
    let _watch = crate::worker_watch::hook(crate::worker_watch::Step::Move);
    let _profile = crate::perf::work(crate::perf::Work::MoveInclusive);
    crate::perf::hook(crate::perf::Hook::Move);
    let original: MoveFn = std::mem::transmute(ORIGINAL_MOVE.load(Ordering::Acquire));
    let Some(shared) = SHARED.get() else {
        original(entity, x, y, events);
        return;
    };
    let handled = catch_unwind(AssertUnwindSafe(|| {
        // A skill held during windup reaches this hook as a current-position
        // Move. An empty effect queue plus observed attack timing proves the
        // accepted attack is no longer pending; only its animation remains.
        let Some((key, actor)) = owned_entity(shared, entity) else {
            return false;
        };
        observe_owned_abilities(shared, entity, key, actor);
        if finish_attack_backswing(shared, key, actor, entity, events) {
            observe_owned_abilities(shared, entity, key, actor);
        }
        let Some(ticket) = take_stop_ticket(key, actor) else {
            return false;
        };
        if ticket.cancel_recall {
            cancel_recall(shared, entity, ticket.actor, events);
        }
        let action = std::ptr::read_unaligned((entity + 0x70) as *const usize);
        let goal = (
            std::ptr::read_unaligned((entity + 0x78) as *const u64),
            std::ptr::read_unaligned((entity + 0x80) as *const u64),
        );
        let position = (
            std::ptr::read_unaligned((entity + 0x658) as *const u64),
            std::ptr::read_unaligned((entity + 0x660) as *const u64),
        );
        let repeated = repeated_move(entity, x, y, ticket.hold);
        shared.movement.trace_native_move(
            ticket.actor,
            position,
            action,
            (x, y),
            goal,
            repeated,
            &shared.logger,
        );
        if !ticket.hold {
            return repeated;
        }
        let actor = ticket.actor;
        let action = std::ptr::read_unaligned((entity + 0x70) as *const usize);
        let notify: StopEventFn = std::mem::transmute(NOTIFY_STOP.load(Ordering::Acquire));
        let stopped = stop_movement(entity, actor, events, notify);
        if stopped && action == 2 {
            shared.logger.write(&format!(
                "MANUAL NATIVE_STOP actor={actor} action=2->0; native stop-animation event emitted"
            ));
        }
        stopped
    }))
    .unwrap_or_else(|_| {
        shared
            .timing
            .cancel("Movement stop adapter panic", &shared.logger);
        false
    });
    if !handled {
        original(entity, x, y, events);
    }
}
