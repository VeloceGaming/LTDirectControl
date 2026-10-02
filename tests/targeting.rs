use lt_direct_control_core::*;

fn point(x: u64, y: u64) -> Point {
    Point { x, y }
}

fn candidate(id: usize, x: u64, champion: bool) -> Candidate {
    Candidate {
        id,
        position: point(x, 0),
        is_champion: champion,
        eligible: true,
        cursor_hit: false,
    }
}

#[test]
fn attack_move_prioritizes_click_over_champion_position() {
    // Controlled champion is at x=0. Enemy 1 is near the champion, but enemy 2
    // is nearest the click. This is the behavior missing from the reference mod.
    let targets = [candidate(1, 10, true), candidate(2, 90, true)];
    assert_eq!(
        attack_move(point(95, 0), &targets, false),
        Intent::Attack(2)
    );
}

#[test]
fn attack_move_filters_invalid_targets_and_champion_only() {
    let mut invalid = candidate(1, 99, true);
    invalid.eligible = false;
    let targets = [invalid, candidate(2, 100, false), candidate(3, 80, true)];
    assert_eq!(
        attack_move(point(100, 0), &targets, false),
        Intent::Attack(2)
    );
    assert_eq!(
        attack_move(point(100, 0), &targets, true),
        Intent::Attack(3)
    );
}

#[test]
fn attack_move_moves_when_no_valid_target_exists() {
    let targets = [candidate(2, 100, false)];
    assert_eq!(
        attack_move(point(100, 0), &targets, true),
        Intent::Move(point(100, 0))
    );
    assert_eq!(
        attack_move(point(100, 0), &[], false),
        Intent::Move(point(100, 0))
    );
}

#[test]
fn equal_distance_is_independent_of_entity_enumeration_order() {
    let targets = [candidate(9, 110, true), candidate(3, 90, true)];
    let reversed = [targets[1], targets[0]];
    assert_eq!(
        attack_move(point(100, 0), &targets, false),
        Intent::Attack(3)
    );
    assert_eq!(
        attack_move(point(100, 0), &reversed, false),
        Intent::Attack(3)
    );
}

#[test]
fn position_direction_and_self_skills_cast_immediately() {
    let cursor = point(800, 900);
    for (kind, target) in [
        (CastKind::Position, CastTarget::Position(cursor)),
        (CastKind::Direction, CastTarget::DirectionToward(cursor)),
        (CastKind::SelfCast, CastTarget::SelfCast),
    ] {
        assert_eq!(
            quick_cast(AbilitySlot::Q, kind, cursor, &[], true),
            Some(Intent::Cast {
                slot: AbilitySlot::Q,
                target
            })
        );
    }
}

#[test]
fn unit_skill_requires_cursor_hit_and_obeys_champion_only() {
    let cursor = point(100, 0);
    let mut minion = candidate(1, 100, false);
    minion.cursor_hit = true;
    let mut champion = candidate(2, 105, true);
    champion.cursor_hit = true;
    let targets = [minion, champion];
    assert_eq!(
        quick_cast(AbilitySlot::W, CastKind::Unit, cursor, &targets, false),
        Some(Intent::Cast {
            slot: AbilitySlot::W,
            target: CastTarget::Unit(1)
        })
    );
    assert_eq!(
        quick_cast(AbilitySlot::W, CastKind::Unit, cursor, &targets, true),
        Some(Intent::Cast {
            slot: AbilitySlot::W,
            target: CastTarget::Unit(2)
        })
    );
    champion.cursor_hit = false;
    assert_eq!(
        quick_cast(
            AbilitySlot::W,
            CastKind::Unit,
            cursor,
            &[minion, champion],
            true
        ),
        None
    );
}

#[test]
fn preview_release_never_casts_and_other_key_release_does_not_dismiss_it() {
    let mut controls = AbilityControls::default();
    assert_eq!(
        controls.press(
            AbilitySlot::R,
            true,
            CastKind::Position,
            point(10, 10),
            &[],
            false
        ),
        None
    );
    assert_eq!(controls.preview(), Some(AbilitySlot::R));
    controls.release(AbilitySlot::Q);
    assert_eq!(controls.preview(), Some(AbilitySlot::R));
    controls.release(AbilitySlot::R);
    assert_eq!(controls.preview(), None);
}

#[test]
fn cancel_clears_preview_and_next_normal_keypress_quick_casts() {
    let mut controls = AbilityControls::default();
    controls.press(
        AbilitySlot::Q,
        true,
        CastKind::Position,
        point(10, 10),
        &[],
        false,
    );
    controls.cancel();
    assert_eq!(controls.preview(), None);
    assert_eq!(
        controls.press(
            AbilitySlot::W,
            false,
            CastKind::Position,
            point(20, 30),
            &[],
            false
        ),
        Some(Intent::Cast {
            slot: AbilitySlot::W,
            target: CastTarget::Position(point(20, 30))
        })
    );
}

#[test]
fn extreme_coordinates_do_not_overflow() {
    let targets = [candidate(1, 0, true), candidate(2, u64::MAX, true)];
    assert_eq!(
        attack_move(point(u64::MAX, u64::MAX), &targets, false),
        Intent::Attack(2)
    );
}
