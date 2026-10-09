use super::*;
#[test]
fn outline_roles_merge_same_unit_and_keep_target_thinner_than_hover() {
    let enemy = crate::combat::Unit {
        id: 29,
        position: (200_000, 200_000),
        radius: 20_000,
        is_champion: true,
        is_minion: false,
        friendly: false,
        in_cc: false,
        is_tower: false,
        body: None,
    };
    let other = crate::combat::Unit { id: 30, ..enemy };
    let targets = OutlineTargets {
        hover: Some(enemy),
        attack: Some(other),
        click: None,
    };
    let now = std::time::Instant::now();
    assert_eq!(
        targets.for_unit(enemy.id, now).unwrap().1,
        OutlineRole::Hover
    );
    assert_eq!(
        targets.for_unit(other.id, now).unwrap().1,
        OutlineRole::Attack
    );
    assert!(targets.for_unit(31, now).is_none());
    let merged = OutlineTargets {
        hover: Some(enemy),
        attack: Some(enemy),
        click: None,
    };
    assert_eq!(
        merged.for_unit(enemy.id, now).unwrap().1,
        OutlineRole::Hover
    );
    let target_only = OutlineTargets {
        hover: None,
        attack: Some(enemy),
        click: None,
    };
    assert_eq!(
        target_only.for_unit(enemy.id, now).unwrap().1,
        OutlineRole::Attack
    );
    let ally = crate::combat::Unit {
        friendly: true,
        ..enemy
    };
    assert!(OutlineTargets {
        hover: None,
        attack: Some(ally),
        click: Some((ally.id, now)),
    }
    .for_unit(ally.id, now)
    .is_none());
    assert!(OutlineRole::Attack.offset() < OutlineRole::Hover.offset());
    assert_eq!(OutlineRole::Attack.offset(), 2. / 3.);
    assert_eq!(OutlineRole::Hover.offset(), 1.2);
    assert!(OutlineTargets::default().for_unit(enemy.id, now).is_none());
}
#[test]
fn click_pulse_wins_once_then_settles_to_hover_or_target() {
    use std::time::{Duration, Instant};
    let unit = crate::combat::Unit {
        id: 29,
        position: (200_000, 200_000),
        radius: 20_000,
        is_champion: true,
        is_minion: false,
        friendly: false,
        in_cc: false,
        is_tower: false,
        body: None,
    };
    let at = Instant::now();
    for hovered in [false, true] {
        let targets = OutlineTargets {
            hover: hovered.then_some(unit),
            attack: Some(unit),
            click: Some((unit.id, at)),
        };
        let first = targets.for_unit(unit.id, at).unwrap();
        assert_eq!((first.1, first.2), (OutlineRole::Click, 3.));
        let middle = targets
            .for_unit(unit.id, at + Duration::from_millis(60))
            .unwrap();
        assert_eq!(middle.1, OutlineRole::Click);
        let end = targets
            .for_unit(unit.id, at + crate::movement::ATTACK_CLICK_DURATION)
            .unwrap();
        assert_eq!(
            end.1,
            if hovered {
                OutlineRole::Hover
            } else {
                OutlineRole::Attack
            }
        );
        assert!(first.2 > middle.2 && middle.2 > end.2);
        // A repeated click restarts even on the same identity.
        let repeated = OutlineTargets {
            click: Some((unit.id, at + Duration::from_secs(1))),
            ..targets
        };
        assert_eq!(
            repeated
                .for_unit(unit.id, at + Duration::from_secs(1))
                .unwrap()
                .2,
            3.
        );
        // A stale click cannot pulse another attack, an ally, or
        // a hovered unit after its attack order was cancelled.
        let cancelled = OutlineTargets {
            attack: None,
            ..targets
        };
        let role = cancelled.for_unit(unit.id, at).map(|(_, r, _)| r);
        assert_eq!(role, hovered.then_some(OutlineRole::Hover));
        let replaced = OutlineTargets {
            click: Some((30, at)),
            ..targets
        };
        assert_eq!(replaced.for_unit(unit.id, at).unwrap().1, end.1);
    }
}
#[test]
fn spectator_layout_lease_switches_and_restores_both_native_flags() {
    for (original_wide, original_ui_wide) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
        let mut config = [0x55u8; 0x60];
        config[0x45] = original_wide;
        config[0x46] = 1;
        let original = config;
        let addr = config.as_mut_ptr() as usize;
        let mut ui = vec![0x33u8; 0x9250];
        let live = ui.as_mut_ptr() as usize;
        unsafe {
            std::ptr::write_unaligned((live + 0x9150) as *mut usize, addr - 0x18);
            ui[0x9234] = original_ui_wide;
            let original_ui = ui.clone();
            let lease = CameraLease::capture(101, addr, live);
            assert!(lease.full_width(101, addr, live));
            assert_eq!((config[0x45], ui[0x9234]), (1, 1));
            config[0x18..0x28].fill(2);
            config[0x4b] = 2;
            ui[0x9234] = 0;
            assert!(lease.full_width(101, addr, live));
            let overridden = config;
            let overridden_ui = ui.clone();
            let mut other = [0x33u8; 0x60];
            let other_before = other;
            let other_addr = other.as_mut_ptr() as usize;
            let mut other_ui = ui.clone();
            let other_live = other_ui.as_mut_ptr() as usize;
            for (v, c, u) in [
                (102, addr, live),
                (101, other_addr, live),
                (101, addr, other_live),
                (101, addr, 0),
            ] {
                assert!(!lease.full_width(v, c, u));
                assert!(!lease.restore(v, c, u));
            }
            assert_eq!(config, overridden);
            assert_eq!(ui, overridden_ui);
            assert_eq!(other, other_before);
            assert_eq!(other_ui, overridden_ui);
            // Even an identical UI address must still link to this config.
            std::ptr::write_unaligned((live + 0x9150) as *mut usize, other_addr - 0x18);
            assert!(!lease.full_width(101, addr, live));
            assert!(!lease.restore(101, addr, live));
            assert_eq!(config, overridden);
            assert_eq!(ui[0x9234], 1);
            std::ptr::write_unaligned((live + 0x9150) as *mut usize, addr - 0x18);
            assert!(lease.restore(101, addr, live));
            assert_eq!(config, original);
            assert_eq!(ui, original_ui);
            other[0x45] = 1 - original_wide;
            other_ui[0x9234] = 1 - original_ui_wide;
            std::ptr::write_unaligned((other_live + 0x9150) as *mut usize, other_addr - 0x18);
            let fresh_original = other;
            let fresh_ui = other_ui.clone();
            let fresh = CameraLease::capture(102, other_addr, other_live);
            assert!(fresh.full_width(102, other_addr, other_live));
            assert!(fresh.restore(102, other_addr, other_live));
            assert_eq!(other, fresh_original);
            assert_eq!(other_ui, fresh_ui);
        }
    }
}
unsafe fn fake_ui_any(data: usize) -> (usize, usize) {
    let pair = std::ptr::read(data as *const [usize; 2]);
    (pair[0], pair[1])
}
unsafe extern "system" fn fake_ui_type_id(out: *mut [u64; 2], _: usize) {
    std::ptr::write(out, [0x810a3f1ff5177337, 0xaf7a6919a6336e28]);
}
unsafe extern "system" fn fake_other_type_id(out: *mut [u64; 2], _: usize) {
    std::ptr::write(out, [0, 0]);
}
#[test]
fn live_ingame_downcast_checks_type_and_shared_config_before_layout_access() {
    let config = [0u8; 0x60];
    let addr = config.as_ptr() as usize;
    let mut ui = vec![0u8; 0x9250];
    let object = ui.as_mut_ptr() as usize;
    let mut any_table = [0usize; 4];
    any_table[3] = fake_ui_type_id as *const () as usize;
    let mut pair = [object, any_table.as_ptr() as usize];
    let mut table = [0usize; 11];
    table[10] = fake_ui_any as *const () as usize;
    let mut node = [0usize; 0x240 / 8];
    node[0x230 / 8] = pair.as_ptr() as usize;
    node[0x238 / 8] = table.as_ptr() as usize;
    let node_addr = node.as_ptr() as usize;
    unsafe {
        assert_eq!(ingame_ui_from_node(0, addr), None);
        assert_eq!(ingame_ui_from_node(node_addr, addr), None);
        std::ptr::write_unaligned((object + 0x9150) as *mut usize, addr - 0x18);
        assert_eq!(ingame_ui_from_node(node_addr, addr), Some(object));
        std::ptr::write(
            any_table.as_mut_ptr().add(3),
            fake_other_type_id as *const () as usize,
        );
        assert_eq!(ingame_ui_from_node(node_addr, addr), None);
        std::ptr::write(pair.as_mut_ptr(), 0);
        assert_eq!(ingame_ui_from_node(node_addr, addr), None);
        std::ptr::write(node.as_mut_ptr().add(0x238 / 8), 0);
        assert_eq!(ingame_ui_from_node(node_addr, addr), None);
    }
}
thread_local! {
    static DROPPED_COMMANDS: std::cell::RefCell<Vec<(u64, u8)>> = const { std::cell::RefCell::new(Vec::new()) };
}
unsafe extern "system" fn spy_drop_command(command: usize) {
    let pointer = command as *const u8;
    DROPPED_COMMANDS.with(|d| {
        d.borrow_mut()
            .push((command_tag(pointer), *pointer.add(0x90)))
    });
}
fn fake_commands<const N: usize>(tags: [u64; N]) -> [NativeCommand; N] {
    std::array::from_fn(|i| {
        let mut command = NativeCommand([0; COMMAND_SIZE]);
        // Text uses the String capacity niche, rather than an enum tag.
        let raw = if tags[i] == 7 {
            16
        } else {
            tags[i] | (1 << 63)
        };
        command.0[..8].copy_from_slice(&raw.to_le_bytes());
        command.0[0x90] = i as u8 + 10;
        command
    })
}
#[test]
fn mixed_sprite_circle_text_pass_moves_sprites_and_drops_auxiliaries_once() {
    let commands = fake_commands([3, 4, 7, 8, 3]);
    let vector = NativeCommandVec {
        capacity: commands.len(),
        pointer: commands.as_ptr() as usize,
        length: commands.len(),
    };
    let before = commands.each_ref().map(|c| c.0);
    let mut destination = fake_commands([0, 0]);
    DROPPED_COMMANDS.with(|d| d.borrow_mut().clear());
    unsafe {
        assert!(valid_command_vec(vector));
        assert!(supported_outline_commands(vector));
        assert_eq!(sprite_count(vector), 2);
        assert_eq!(
            take_outline_commands(
                vector,
                Some(destination.as_mut_ptr() as *mut u8),
                spy_drop_command
            ),
            2
        );
    }
    assert_eq!(destination[0].0, before[0]);
    assert_eq!(destination[1].0, before[4]);
    assert_eq!(commands.each_ref().map(|c| c.0), before);
    DROPPED_COMMANDS.with(|d| assert_eq!(*d.borrow(), [(4, 11), (7, 12), (8, 13)]));
}
#[test]
fn fallback_drops_every_known_command_once_without_moving_original() {
    let commands = fake_commands([3, 4, 7, 8, 3]);
    let vector = NativeCommandVec {
        capacity: commands.len(),
        pointer: commands.as_ptr() as usize,
        length: commands.len(),
    };
    let before = commands.each_ref().map(|c| c.0);
    DROPPED_COMMANDS.with(|d| d.borrow_mut().clear());
    unsafe {
        assert_eq!(take_outline_commands(vector, None, spy_drop_command), 0);
    }
    assert_eq!(commands.each_ref().map(|c| c.0), before);
    DROPPED_COMMANDS
        .with(|d| assert_eq!(*d.borrow(), [(3, 10), (4, 11), (7, 12), (8, 13), (3, 14)]));
}
#[test]
fn unknown_commands_are_rejected_and_never_given_to_native_destructor() {
    let commands = fake_commands([4, 19, 7]);
    let vector = NativeCommandVec {
        capacity: commands.len(),
        pointer: commands.as_ptr() as usize,
        length: commands.len(),
    };
    assert!(!unsafe { supported_outline_commands(vector) });
    DROPPED_COMMANDS.with(|d| d.borrow_mut().clear());
    unsafe {
        take_outline_commands(vector, None, spy_drop_command);
    }
    DROPPED_COMMANDS.with(|d| assert_eq!(*d.borrow(), [(4, 10), (7, 12)]));
}
#[test]
fn all_native_command_variants_are_accepted_with_bounded_pass_size() {
    let commands = fake_commands(std::array::from_fn::<_, 19, _>(|i| i as u64));
    let mut vector = NativeCommandVec {
        capacity: commands.len(),
        pointer: commands.as_ptr() as usize,
        length: commands.len(),
    };
    assert!(unsafe { supported_outline_commands(vector) });
    vector.capacity = MAX_BODY_COMMANDS + 1;
    vector.length = MAX_BODY_COMMANDS + 1;
    assert!(!unsafe { valid_command_vec(vector) });
}
#[test]
fn aim_copy_preserves_cursor_and_native_rng_and_seven_argument_abi() {
    unsafe extern "system" fn correct(
        a: usize,
        b: usize,
        c: usize,
        d: usize,
        e: usize,
        f: usize,
        target: usize,
    ) {
        let seen = &mut *(a as *mut Vec<usize>);
        seen.extend([b, c, d, e, f]);
        let input = &mut *(target as *mut [usize; 3]);
        seen.extend(*input);
        input[1] = 999;
        input[2] = 555;
    }
    let mut seen = Vec::new();
    let mut target = [1usize, (-47_291i64) as usize, (-125_996i64) as usize];
    let cursor = target;
    let args = [
        &mut seen as *mut Vec<usize> as usize,
        2,
        3,
        4,
        468,
        6,
        target.as_mut_ptr() as usize,
    ];
    unsafe {
        assert_eq!(evaluate_aim(correct, args, true), Some([1, 999, 555]));
    }
    assert_eq!(target, cursor);
    assert_eq!(seen, [2, 3, 4, 468, 6, cursor[0], cursor[1], cursor[2]]);
    seen.clear();
    // Same relay mechanism as the installed hook; exercises args 5-7
    // on the Windows stack, not just a direct Rust call.
    let address =
        unsafe { relay(correct as *const () as usize, correct as *const () as usize) }.unwrap();
    let relayed: AimFn = unsafe { std::mem::transmute(address) };
    unsafe {
        assert_eq!(evaluate_aim(relayed, args, false), None);
    }
    assert_eq!(target, [1, 999, 555]);
    assert_eq!(seen.len(), 8);
    let shared = owned_fixture("aim-worker-scope");
    assert!(owned_key(&shared, 7).is_some());
    assert!(owned_key(&shared, 8).is_none());
    for kind in 0usize..5 {
        let words = [kind | (0xaabbccddusize << 32), 30, 40];
        let result = unsafe { manual_aim_words(&shared, 7, words.as_ptr() as usize) };
        assert_eq!(result.is_some(), matches!(kind, 1 | 2));
        assert!(unsafe { manual_aim_words(&shared, 8, words.as_ptr() as usize) }.is_none());
    }
    assert!(unsafe { manual_aim_words(&shared, 7, 0) }.is_none());
    // Foreign workers must fail before touching even an aligned invalid pointer.
    let worker = std::thread::spawn(move || unsafe { manual_aim_words(&shared, 7, 8) });
    assert_eq!(worker.join().unwrap(), None);
}
fn owned_fixture(name: &str) -> Shared {
    let key = (1, 33, 1);
    let shared = Shared {
        timing: Arc::new(NativeTiming::running_test_worker(key)),
        logger: Arc::new(crate::test_support::logger(name)),
        movement: Arc::new(crate::movement::Movement::new(true)),
        camera: Arc::new(crate::camera::CameraControl::default()),
        abilities: Arc::new(crate::abilities::Abilities::default()),
    };
    shared
        .abilities
        .observe_actor(key, Some(7), Some((200_000, 200_000)), [0; 3]);
    shared.abilities.update(
        crate::platform_input::Keys {
            focused: true,
            ..Default::default()
        },
        true,
        &shared.camera,
        &shared.logger,
    );
    shared
}
fn owned_bytes() -> [u8; 0x6c0] {
    let mut bytes = [0u8; 0x6c0];
    bytes[0x68..0x6c].copy_from_slice(&15u32.to_le_bytes());
    bytes[0x5b8..0x5c0].copy_from_slice(&7usize.to_le_bytes());
    bytes[0x5c0..0x5c8].copy_from_slice(&1usize.to_le_bytes());
    bytes
}
#[test]
fn reused_actor_on_background_worker_cannot_replace_live_charge_state() {
    let shared = Arc::new(owned_fixture("owned-native-36"));
    let charges = Some(crate::abilities::Charges {
        capacity: 300,
        cost: 100,
        uses: 3,
    });
    shared
        .abilities
        .observe_native((1, 33, 1), 7, 2, [charges; 3]);
    let other = shared.clone();
    std::thread::spawn(move || {
        let mut bytes = owned_bytes();
        bytes[0x70..0x78].copy_from_slice(&5usize.to_le_bytes());
        let entity = bytes.as_ptr() as usize;
        assert!(unsafe { owned_entity(&other, entity) }.is_none());
        assert!(!unsafe { observe_owned_abilities(&other, entity, (1, 33, 1), 7) });
    })
    .join()
    .unwrap();
    assert_eq!(shared.abilities.hud_skills().uses, [Some(3); 3]);
    let bytes = owned_bytes();
    assert!(unsafe { observe_owned_abilities(&shared, bytes.as_ptr() as usize, (1, 33, 1), 7) });
    assert_eq!(shared.abilities.hud_skills().ready, [None; 3]);
    assert!(owned_key(&shared, 8).is_none());
    shared
        .abilities
        .observe_actor((1, 33, 2), Some(7), Some((200_000, 200_000)), [0; 3]);
    assert!(owned_key(&shared, 7).is_none());
}
#[test]
fn steering_uses_current_borrow_without_previous_command_or_saved_address() {
    let shared = owned_fixture("owned-steering-36");
    STOP_TICKET.set(None);
    let first = owned_bytes();
    let second = owned_bytes();
    for bytes in [&first, &second] {
        let entity = bytes.as_ptr() as usize;
        assert_eq!(
            unsafe { owned_steering(&shared, 7, entity + 0x658, entity + 0x660) },
            Some((1, 33, 1))
        );
        assert!(unsafe { owned_steering(&shared, 7, entity + 0x658, entity + 0x668) }.is_none());
        assert!(unsafe { owned_steering(&shared, 8, entity + 0x658, entity + 0x660) }.is_none());
    }
    let mut wrong = owned_bytes();
    wrong[0x68..0x6c].copy_from_slice(&12u32.to_le_bytes());
    let entity = wrong.as_ptr() as usize;
    assert!(unsafe { owned_steering(&shared, 7, entity + 0x658, entity + 0x660) }.is_none());
    assert!(unsafe { owned_steering(&shared, 7, 0, 8) }.is_none());
}
#[test]
fn identical_move_does_not_suppress_changed_goals_holds_busy_or_forced_actions() {
    let mut bytes = [0u8; 0x6c0];
    bytes[0x68..0x6c].copy_from_slice(&15u32.to_le_bytes());
    bytes[0x70..0x78].copy_from_slice(&2usize.to_le_bytes());
    bytes[0x78..0x80].copy_from_slice(&300_000u64.to_le_bytes());
    bytes[0x80..0x88].copy_from_slice(&400_000u64.to_le_bytes());
    let address = bytes.as_ptr() as usize;
    assert!(unsafe { repeated_move(address, 300_000, 400_000, false) });
    assert!(!unsafe { repeated_move(address, 300_001, 400_000, false) });
    assert!(!unsafe { repeated_move(address, 300_000, 400_000, true) });
    for action in [0usize, 1, 3, 4, 5, 6] {
        bytes[0x70..0x78].copy_from_slice(&action.to_le_bytes());
        assert!(!unsafe { repeated_move(address, 300_000, 400_000, false) });
    }
    bytes[0x70..0x78].copy_from_slice(&2usize.to_le_bytes());
    let mut effect = [0u8; 0x28];
    effect[..4].copy_from_slice(&10u32.to_le_bytes());
    bytes[0x2c0..0x2c8].copy_from_slice(&(effect.as_ptr() as usize).to_le_bytes());
    bytes[0x2c8..0x2d0].copy_from_slice(&1usize.to_le_bytes());
    assert!(!unsafe { repeated_move(address, 300_000, 400_000, false) });
}
static RECEIVED: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());
unsafe extern "system" fn fake_worker(a: usize, b: usize, c: usize) {
    *RECEIVED.lock().unwrap() = vec![a as u64, b as u64, c as u64];
    // Model send's sret output so forwarding verifies output memory too.
    std::ptr::write(a as *mut usize, usize::MAX);
}
unsafe extern "system" fn fake_move(a: usize, b: u64, c: u64, d: usize) {
    *RECEIVED.lock().unwrap() = vec![a as u64, b, c, d as u64];
}
#[allow(clippy::too_many_arguments)]
unsafe extern "system" fn fake_steer(
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    e: usize,
    f: usize,
    g: usize,
    h: u64,
    i: u64,
    j: usize,
) {
    *RECEIVED.lock().unwrap() = vec![
        a as u64, b as u64, c as u64, d as u64, e as u64, f as u64, g as u64, h, i, j as u64,
    ];
}
#[allow(clippy::too_many_arguments)]
unsafe extern "system" fn fake_step(
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    e: usize,
    f: u64,
    g: u64,
    h: usize,
) {
    *RECEIVED.lock().unwrap() = vec![
        a as u64, b as u64, c as u64, d as u64, e as u64, f, g, h as u64,
    ];
}
#[test]
fn attack_commit_observer_rejects_unaccepted_attacks_and_preserves_entity() {
    let mut bytes = [0u8; 0x6c0];
    let mut queue = [0usize; 7];
    queue[2] = 5;
    bytes[0x68..0x6c].copy_from_slice(&15u32.to_le_bytes());
    bytes[0x70..0x78].copy_from_slice(&3usize.to_le_bytes());
    bytes[0x2a0..0x2a8].copy_from_slice(&1usize.to_le_bytes());
    bytes[0x2a8..0x2b0].copy_from_slice(&(queue.as_ptr() as usize).to_le_bytes());
    bytes[0x2b0..0x2b8].copy_from_slice(&1usize.to_le_bytes());
    // Queued-branch entry: delay readable, but it is not a locked attack.
    let actor = bytes.as_ptr() as usize;
    assert_eq!(unsafe { queued_attack_delay(actor, 0) }, Some(5));
    assert_eq!(unsafe { locked_attack_hit_tick(actor, 0) }, None);
    // Locked branch as traced for Ninja: no new entry, start 13, factor 100.
    bytes[0x2b0..0x2b8].copy_from_slice(&0usize.to_le_bytes());
    bytes[0x80..0x88].copy_from_slice(&100usize.to_le_bytes());
    bytes[0x4a8..0x4b0].copy_from_slice(&13usize.to_le_bytes());
    let before = bytes;
    assert_eq!(unsafe { locked_attack_hit_tick(actor, 0) }, Some(13));
    bytes[0x80..0x88].copy_from_slice(&130usize.to_le_bytes());
    assert_eq!(unsafe { locked_attack_hit_tick(actor, 0) }, Some(10));
    bytes[0x80..0x88].copy_from_slice(&100usize.to_le_bytes());
    assert_eq!(bytes, before);
    assert!(unsafe { can_stop_action(actor, 3) });
    // Not BaseAttack, absent declared timing, zero factor, started
    // earlier (elapsed != 0) or a queue entry from this call: no proof.
    for (range, value) in [
        (0x4b4..0x4b8, 1u32.to_le_bytes().to_vec()),
        (0x4b8..0x4bc, (-1i32).to_le_bytes().to_vec()),
        (0x80..0x88, 0usize.to_le_bytes().to_vec()),
        (0x78..0x80, 1usize.to_le_bytes().to_vec()),
        (0x2b0..0x2b8, 1usize.to_le_bytes().to_vec()),
    ] {
        let mut changed = before;
        changed[range].copy_from_slice(&value);
        let entity = changed.as_ptr() as usize;
        assert_eq!(unsafe { locked_attack_hit_tick(entity, 0) }, None);
    }
    bytes[0x78..0x80].copy_from_slice(&1usize.to_le_bytes());
    let mut effect = [0u8; 0x28];
    effect[..4].copy_from_slice(&10u32.to_le_bytes());
    bytes[0x2c0..0x2c8].copy_from_slice(&(effect.as_ptr() as usize).to_le_bytes());
    bytes[0x2c8..0x2d0].copy_from_slice(&1usize.to_le_bytes());
    assert!(!unsafe { can_stop_action(actor, 3) });
}
unsafe extern "system" fn fake_attack(a: usize, b: usize, c: usize) {
    *RECEIVED.lock().unwrap() = vec![a as u64, b as u64, c as u64];
}
unsafe extern "system" fn fake_cooltime(_: usize, _: usize) -> usize {
    360
}
unsafe extern "system" fn fake_uses(_: usize, _: usize) -> usize {
    3
}
#[test]
fn native_observer_refreshes_spent_cooldown_before_charge_readiness() {
    let shared = owned_fixture("owned-cooldown-36");
    let mut bytes = owned_bytes();
    let mut table = [0usize; 24];
    table[0x90 / 8] = fake_cooltime as *const () as usize;
    table[0xa8 / 8] = fake_uses as *const () as usize;
    bytes[0x578..0x580].copy_from_slice(&1usize.to_le_bytes());
    bytes[0x580..0x588].copy_from_slice(&(table.as_ptr() as usize).to_le_bytes());
    let before = bytes;
    assert!(unsafe { observe_owned_abilities(&shared, bytes.as_ptr() as usize, (1, 33, 1), 7) });
    assert_eq!(shared.abilities.hud_skills().uses[0], Some(3));
    assert_eq!(bytes, before);
    bytes[0xb8..0xc0].copy_from_slice(&360usize.to_le_bytes());
    let after = bytes;
    assert!(unsafe { observe_owned_abilities(&shared, bytes.as_ptr() as usize, (1, 33, 1), 7) });
    assert_eq!(shared.abilities.hud_skills().uses[0], Some(0));
    assert_eq!(shared.abilities.hud_skills().ready[0], Some(false));
    assert_eq!(bytes, after);
}
#[test]
fn charge_reader_matches_native_cdr_recast_budget_and_never_writes_entity() {
    let mut bytes = [0u8; 0x6c0];
    let mut table = [0usize; 24];
    table[0x90 / 8] = fake_cooltime as *const () as usize;
    table[0xa8 / 8] = fake_uses as *const () as usize;
    for offset in [0x578, 0x588, 0x598] {
        bytes[offset..offset + 8].copy_from_slice(&1usize.to_le_bytes());
        bytes[offset + 8..offset + 16].copy_from_slice(&(table.as_ptr() as usize).to_le_bytes());
    }
    bytes[0x5c0..0x5c8].copy_from_slice(&5usize.to_le_bytes());
    bytes[0x3f8..0x3fc].copy_from_slice(&100i32.to_le_bytes());
    bytes[0x464..0x468].copy_from_slice(&100i32.to_le_bytes());
    let before = bytes;
    let entity = bytes.as_ptr() as usize;
    for offset in [0x578, 0x588] {
        assert_eq!(
            unsafe { read_charges(entity, offset) },
            Some(crate::abilities::Charges {
                capacity: 180,
                cost: 60,
                uses: 3
            })
        );
    }
    assert_eq!(
        unsafe { read_charges(entity, 0x598) },
        Some(crate::abilities::Charges {
            capacity: 120,
            cost: 40,
            uses: 3
        })
    );
    assert_eq!(bytes, before);
    bytes[0x5c0..0x5c8].copy_from_slice(&1usize.to_le_bytes());
    assert!(unsafe { read_charges(bytes.as_ptr() as usize, 0x588) }.is_none());
    assert!(unsafe { read_charges(bytes.as_ptr() as usize, 0x598) }.is_none());
}
#[test]
fn native_effect_reader_uses_three_skill_slots_and_rejects_absent_effects() {
    let mut bytes = [0xccu8; 0x6c0];
    // Set distinct metadata with deliberately meaningless Arc/vtable
    // bytes: the reader must never dereference or clone those fields.
    bytes[0x5c0..0x5c8].copy_from_slice(&3u64.to_le_bytes());
    bytes[0x430..0x438].copy_from_slice(&2_000u64.to_le_bytes());
    for (slot, offset) in [0x4c0, 0x4f8, 0x530].into_iter().enumerate() {
        bytes[offset + 0x10..offset + 0x18]
            .copy_from_slice(&(70_000u64 + slot as u64 * 10_000).to_le_bytes());
        bytes[offset + 0x18..offset + 0x20].copy_from_slice(&5_000u64.to_le_bytes());
        bytes[offset + 0x28..offset + 0x2c].copy_from_slice(&(6u32 + slot as u32).to_le_bytes());
        bytes[offset + 0x30..offset + 0x34].copy_from_slice(&(slot as u32).to_le_bytes());
    }
    let values = unsafe { read_ability_metadata(bytes.as_ptr() as usize) };
    for (slot, value) in values.into_iter().enumerate() {
        assert_eq!(
            value.unwrap(),
            crate::abilities::Descriptor {
                casting: slot as u32,
                target: 6 + slot as u32,
                range: 82_000 + slot as u64 * 10_000
            }
        );
    }
    bytes[0x4f0..0x4f4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(unsafe { read_ability_metadata(bytes.as_ptr() as usize) }[0].is_none());
}
#[test]
fn basic_attack_metadata_uses_current_effect_growth_and_bonus_without_pointer_calls() {
    let mut bytes = [0xccu8; 0x6c0];
    bytes[0x5c0..0x5c8].copy_from_slice(&3u64.to_le_bytes());
    bytes[0x430..0x438].copy_from_slice(&2_000u64.to_le_bytes());
    bytes[0x498..0x4a0].copy_from_slice(&60_000u64.to_le_bytes());
    bytes[0x4a0..0x4a8].copy_from_slice(&5_000u64.to_le_bytes());
    bytes[0x4b0..0x4b4].copy_from_slice(&6u32.to_le_bytes());
    bytes[0x4b8..0x4bc].copy_from_slice(&0u32.to_le_bytes());
    let entity = bytes.as_ptr() as usize;
    assert_eq!(
        unsafe { read_effect_metadata(entity, 0x488) }
            .unwrap()
            .range,
        72_000
    );
    bytes[0x498..0x4a0].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(unsafe { read_effect_metadata(bytes.as_ptr() as usize, 0x488) }.is_none());
    bytes[0x498..0x4a0].copy_from_slice(&60_000u64.to_le_bytes());
    bytes[0x4b8..0x4bc].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(unsafe { read_effect_metadata(bytes.as_ptr() as usize, 0x488) }.is_none());
}
#[allow(clippy::too_many_arguments)]
unsafe extern "system" fn fake_view(
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    e: usize,
    f: usize,
    g: usize,
    h: usize,
    i: f32,
) {
    *RECEIVED.lock().unwrap() = vec![
        a as u64,
        b as u64,
        c as u64,
        d as u64,
        e as u64,
        f as u64,
        g as u64,
        h as u64,
        i.to_bits() as u64,
    ];
}
unsafe extern "system" fn fake_input(
    a: usize,
    b: usize,
    c: usize,
    dt: f32,
    e: usize,
    f: usize,
    g: usize,
) {
    *RECEIVED.lock().unwrap() = vec![
        a as u64,
        b as u64,
        c as u64,
        dt.to_bits() as u64,
        e as u64,
        f as u64,
        g as u64,
    ];
}
#[test]
fn follow_payload_uses_team_and_lane_instead_of_sdk_player_id() {
    for side in 0..2 {
        for lane in 0..5 {
            let mut config = [0xccu8; 0x60];
            let addr = config.as_mut_ptr() as usize;
            unsafe {
                set_follow_config(addr, side, lane);
            }
            assert_eq!(
                u32::from_le_bytes(config[0x18..0x1c].try_into().unwrap()),
                2
            );
            assert_eq!(
                u32::from_le_bytes(config[0x1c..0x20].try_into().unwrap()),
                lane
            );
            assert_eq!(
                usize::from_le_bytes(config[0x20..0x28].try_into().unwrap()),
                side
            );
            assert!(config[..0x18]
                .iter()
                .chain(config[0x28..].iter())
                .all(|b| *b == 0xcc));
        }
    }
}
#[test]
fn spectator_filter_covers_press_release_and_preserves_mouse_zoom() {
    for tag in [0x8000000000000006, 0x8000000000000007] {
        assert!(spectator_key_is_owned(tag, 0x17)); // default S pause shortcut
        assert!(spectator_key_is_owned(tag, 4)); // default Tab info shortcut
        assert!(!spectator_key_is_owned(tag, 0x3d));
        assert!(!spectator_key_is_owned(tag, 0x3e));
        assert!(!spectator_key_is_owned(tag, 0x21));
    }
    for tag in [
        0,
        0x8000000000000001,
        0x8000000000000003,
        0x8000000000000004,
        0x8000000000000005,
        0x800000000000000d,
    ] {
        assert!(!spectator_key_is_owned(tag, 0x17));
    }
}
#[test]
fn windows_hash_rel32_and_forwarding_abi() {
    assert_eq!(
        relative_call(STEER_SITE, STEER_ORIGINAL).unwrap(),
        STEER_BYTES
    );
    ORIGINAL_STEER.store(fake_steer as *const () as usize, Ordering::Release);
    unsafe {
        steer_hook(1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
    }
    assert_eq!(
        *RECEIVED.lock().unwrap(),
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    );
    let step: DirectStepFn = fake_step;
    unsafe {
        step(1, 2, 3, 4, 5, 6, 7, 8);
    }
    assert_eq!(*RECEIVED.lock().unwrap(), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(
        sha256(b"abc").unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        relative_call(WORKER_SITE, WORKER_ORIGINAL).unwrap(),
        WORKER_BYTES
    );
    assert_eq!(relative_call(VIEW_SITE, VIEW_ORIGINAL).unwrap(), VIEW_BYTES);
    assert_eq!(relative_jump(MOVE_SITE, MOVE_ORIGINAL).unwrap(), MOVE_BYTES);
    assert_eq!(
        relative_call(INPUT_SITE, INPUT_ORIGINAL).unwrap(),
        INPUT_BYTES
    );
    assert_eq!(
        relative_call(ATTACK_SITE, ATTACK_ORIGINAL).unwrap(),
        ATTACK_BYTES
    );
    ORIGINAL_ATTACK.store(fake_attack as *const () as usize, Ordering::Release);
    unsafe {
        attack_hook(11, 22, 33);
    }
    assert_eq!(*RECEIVED.lock().unwrap(), vec![11, 22, 33]);
    assert_eq!(
        relative_call(AUTO_ATTACK_SITE, ATTACK_ORIGINAL).unwrap(),
        AUTO_ATTACK_BYTES
    );
    unsafe {
        auto_attack_hook(44, 55, 66);
    }
    assert_eq!(*RECEIVED.lock().unwrap(), vec![44, 55, 66]);
    for slot in 0..3 {
        assert_eq!(
            relative_call(SKILL_SITES[slot], SKILL_ORIGINALS[slot]).unwrap(),
            SKILL_BYTES[slot]
        );
        ORIGINAL_SKILLS[slot].store(fake_attack as *const () as usize, Ordering::Release);
        unsafe {
            skill_hook(slot, 11, 22, 33);
        }
        assert_eq!(*RECEIVED.lock().unwrap(), vec![11, 22, 33]);
    }
    let attack_relay = unsafe {
        relay(
            fake_attack as *const () as usize,
            fake_attack as *const () as usize,
        )
    }
    .unwrap();
    let relay_attack: AttackFn = unsafe { std::mem::transmute(attack_relay) };
    unsafe {
        relay_attack(44, 55, 66);
    }
    assert_eq!(*RECEIVED.lock().unwrap(), vec![44, 55, 66]);
    ORIGINAL_INPUT.store(fake_input as *const () as usize, Ordering::Release);
    unsafe {
        input_hook(1, 2, 3, 0.0125, 5, 6, 7);
    }
    assert_eq!(
        *RECEIVED.lock().unwrap(),
        vec![1, 2, 3, 0.0125f32.to_bits() as u64, 5, 6, 7]
    );
    let input_relay = unsafe {
        relay(
            fake_input as *const () as usize,
            fake_input as *const () as usize,
        )
    }
    .unwrap();
    let relay_input: NativeInputFn = unsafe { std::mem::transmute(input_relay) };
    unsafe {
        relay_input(7, 6, 5, 0.025, 3, 2, 1);
    }
    assert_eq!(
        *RECEIVED.lock().unwrap(),
        vec![7, 6, 5, 0.025f32.to_bits() as u64, 3, 2, 1]
    );
    assert!(relative_call(0, usize::MAX).is_err());
    ORIGINAL_WORKER.store(fake_worker as *const () as usize, Ordering::Release);
    ORIGINAL_VIEW.store(fake_view as *const () as usize, Ordering::Release);
    ORIGINAL_MOVE.store(fake_move as *const () as usize, Ordering::Release);
    unsafe {
        move_hook(11, 22, 33, 44);
    }
    assert_eq!(*RECEIVED.lock().unwrap(), vec![11, 22, 33, 44]);
    unsafe {
        let mut sent = 0usize;
        worker_hook(&mut sent as *mut usize as usize, 22, 33);
        assert_eq!(sent, usize::MAX);
        assert_eq!(
            *RECEIVED.lock().unwrap(),
            vec![&mut sent as *mut usize as u64, 22, 33]
        );
    }
    let worker_relay = unsafe {
        relay(
            fake_worker as *const () as usize,
            fake_worker as *const () as usize,
        )
    }
    .unwrap();
    let relay_worker: WorkerFn = unsafe { std::mem::transmute(worker_relay) };
    unsafe {
        let mut sent = 0usize;
        relay_worker(&mut sent as *mut usize as usize, 55, 66);
        assert_eq!(sent, usize::MAX);
        assert_eq!(
            *RECEIVED.lock().unwrap(),
            vec![&mut sent as *mut usize as u64, 55, 66]
        );
    }
    // SHARED is intentionally absent: passthrough preserves all nine
    // arguments, including the fifth pointer and ninth stack f32 slot.
    unsafe {
        view_hook(1, 2, 3, 4, 5, 6, 60, 8, 0.0125);
    }
    assert_eq!(
        *RECEIVED.lock().unwrap(),
        vec![1, 2, 3, 4, 5, 6, 60, 8, 0.0125f32.to_bits() as u64]
    );
    let view_relay = unsafe {
        relay(
            fake_view as *const () as usize,
            fake_view as *const () as usize,
        )
    }
    .unwrap();
    let relay_view: ViewFn = unsafe { std::mem::transmute(view_relay) };
    unsafe {
        relay_view(9, 8, 7, 6, 5, 4, 60, 2, 0.025);
    }
    assert_eq!(
        *RECEIVED.lock().unwrap(),
        vec![9, 8, 7, 6, 5, 4, 60, 2, 0.025f32.to_bits() as u64]
    );
}
#[test]
fn stop_gate_preserves_nonmovement_actions_and_forced_movement() {
    let mut actor = vec![0u64; 0x6c0 / 8];
    let address = actor.as_mut_ptr() as usize;
    let mut effects = [0u64; 5];
    unsafe {
        std::ptr::write_unaligned((address + 0x68) as *mut u32, 15);
        for action in 0..7 {
            std::ptr::write_unaligned((address + 0x70) as *mut usize, action);
            assert_eq!(can_stop_movement(address), action == 2);
        }
        std::ptr::write_unaligned((address + 0x70) as *mut usize, 2);
        std::ptr::write_unaligned(
            (address + 0x2c0) as *mut usize,
            effects.as_mut_ptr() as usize,
        );
        std::ptr::write_unaligned((address + 0x2c8) as *mut usize, 1);
        std::ptr::write_unaligned(effects.as_mut_ptr(), 6);
        assert!(!can_stop_movement(address));
        std::ptr::write_unaligned(effects.as_mut_ptr(), 3);
        assert!(can_stop_movement(address));
        std::ptr::write_unaligned((address + 0x68) as *mut u32, 0);
        assert!(!can_stop_movement(address));
    }
}
#[test]
fn stop_emits_native_event_once_and_clears_only_movement() {
    unsafe extern "system" fn notify(events: usize, actor: usize) {
        let seen = &mut *(events as *mut Vec<usize>);
        seen.push(actor);
    }
    let mut entity = vec![0u64; 0x6c0 / 8];
    let address = entity.as_mut_ptr() as usize;
    let mut seen = Vec::<usize>::new();
    let events = &mut seen as *mut Vec<usize> as usize;
    unsafe {
        std::ptr::write_unaligned((address + 0x68) as *mut u32, 15);
        std::ptr::write_unaligned((address + 0x70) as *mut usize, 2);
        assert!(stop_movement(address, 17, events, notify));
        assert_eq!(
            std::ptr::read_unaligned((address + 0x70) as *const usize),
            0
        );
        assert_eq!(seen, vec![17]);
        assert!(stop_movement(address, 17, events, notify));
        assert_eq!(seen, vec![17]);
        for action in 3..7 {
            std::ptr::write_unaligned((address + 0x70) as *mut usize, action);
            assert!(!stop_movement(address, 17, events, notify));
            assert_eq!(
                std::ptr::read_unaligned((address + 0x70) as *const usize),
                action
            );
        }
        assert_eq!(seen, vec![17]);
    }
}
#[test]
fn recall_stop_emits_once_and_preserves_timer_effects_and_other_actions() {
    unsafe extern "system" fn notify(events: usize, actor: usize) {
        (&mut *(events as *mut Vec<usize>)).push(actor);
    }
    let mut entity = vec![0xccu8; 0x6c0];
    let addr = entity.as_mut_ptr() as usize;
    entity[0x68..0x6c].copy_from_slice(&15u32.to_le_bytes());
    entity[0x70..0x78].copy_from_slice(&1usize.to_le_bytes());
    let before = entity.clone();
    let mut seen = Vec::<usize>::new();
    let events = &mut seen as *mut Vec<usize> as usize;
    unsafe {
        assert!(stop_recall(addr, 17, events, notify));
        assert_eq!(seen, vec![17]);
        assert_eq!(
            usize::from_le_bytes(entity[0x70..0x78].try_into().unwrap()),
            0
        );
        assert_eq!(&entity[..0x70], &before[..0x70]);
        assert_eq!(&entity[0x78..], &before[0x78..]);
        assert!(!stop_recall(addr, 17, events, notify));
        for action in [2usize, 3, 4, 5, 6] {
            std::ptr::write_unaligned((addr + 0x70) as *mut usize, action);
            assert!(!stop_recall(addr, 17, events, notify));
            assert_eq!(
                std::ptr::read_unaligned((addr + 0x70) as *const usize),
                action
            );
        }
        assert_eq!(seen, vec![17]);
    }
}
#[test]
fn scoped_config_restores_every_overridden_bit() {
    let mut config = [0u8; 0x60];
    config[0] = 1;
    config[0x48] = 1;
    config[0x10..0x14].copy_from_slice(&1u32.to_le_bytes());
    config[0x14..0x18].copy_from_slice(&3.0f32.to_bits().to_le_bytes());
    let before = config;
    let snapshot = unsafe { PlaybackOverride::apply(config.as_mut_ptr() as usize, true) };
    assert_eq!(config[0], 0);
    assert_eq!(config[0x48], 0);
    assert_eq!(
        u32::from_le_bytes(config[0x14..0x18].try_into().unwrap()),
        1.0f32.to_bits()
    );
    drop(snapshot);
    assert_eq!(config, before);
    let snapshot = unsafe { PlaybackOverride::apply(config.as_mut_ptr() as usize, false) };
    assert_eq!(config[0x48], 1); // Spectator pause survives AI handoff.
    drop(snapshot);
    assert_eq!(config, before);
}
#[test]
fn stack_capture_is_available_and_logs_module_relative_frames() {
    let log = crate::test_support::logger("native-stack");
    capture_trace("test stack", &log);
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-native-stack.log");
    let text = std::fs::read_to_string(path).unwrap();
    assert!(text.contains("NATIVE STACK"));
    assert!(text.contains(".exe+0x"));
    assert!(!text.contains("frames=[]"));
}
