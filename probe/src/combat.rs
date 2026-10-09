//! Persistent player orders. The simulation supplies fresh, visible targets;
//! native input validation and combat routines retain damage/timing authority.
use mod_api_stable::{InputKindV1, InputTargetV1, InputV1};

pub const ACQUISITION_RADIUS: u64 = 120_000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Move((u64, u64)),
    Attack(usize),
    AttackMove((u64, u64)),
}
#[derive(Clone, Copy, Debug)]
pub struct Unit {
    pub id: usize,
    pub position: (u64, u64),
    pub radius: u64,
    pub is_champion: bool,
    pub is_minion: bool,
    pub friendly: bool,
    pub in_cc: bool,
    pub is_tower: bool,
    pub body: Option<crate::sprite_picking::Body>,
}
fn distance(a: (u64, u64), b: (u64, u64)) -> u128 {
    let x = u128::from(a.0.abs_diff(b.0));
    let y = u128::from(a.1.abs_diff(b.1));
    (x * x).saturating_add(y * y)
}
/// Native 0.6.3 compares floor(sqrt(centre distance²)) with the current AA
/// effect range (16f6edd..16f6faf). Sprite picking bounds are unrelated.
pub fn attack_in_range(position: (u64, u64), target: (u64, u64), range: u64) -> bool {
    distance(position, target) < (u128::from(range) + 1).saturating_pow(2)
}
pub fn rejected_attack(
    position: (u64, u64),
    chase: Option<(u64, u64)>,
    in_range: Option<bool>,
) -> InputV1 {
    let p = chase.filter(|_| in_range != Some(true)).unwrap_or(position);
    InputV1::move_to(p.0, p.1)
}
impl Order {
    #[cfg(test)]
    pub fn resolve(self, position: (u64, u64), units: &[Unit]) -> (InputV1, Option<(u64, u64)>) {
        self.resolve_filtered(position, units, false)
    }
    pub fn resolve_filtered(
        self,
        position: (u64, u64),
        units: &[Unit],
        champion_only: bool,
    ) -> (InputV1, Option<(u64, u64)>) {
        let options = crate::settings::current();
        self.resolve_policy(
            position,
            units,
            champion_only,
            options.number("attack_move_filter") == 1.,
            options.number("attack_move_preference") == 1.,
        )
    }
    fn resolve_policy(
        self,
        position: (u64, u64),
        units: &[Unit],
        champion_only: bool,
        honor: bool,
        near_cursor: bool,
    ) -> (InputV1, Option<(u64, u64)>) {
        let target = match self {
            Self::Move(goal) => return (ground(position, goal), None),
            Self::Attack(id) => units.iter().find(|u| u.id == id),
            Self::AttackMove(goal) => units
                .iter()
                .filter(|u| !honor || !champion_only || u.is_champion)
                .filter(|u| {
                    let limit = u128::from(ACQUISITION_RADIUS).pow(2);
                    distance(position, u.position) <= limit
                })
                .min_by_key(|u| {
                    (
                        distance(if near_cursor { goal } else { position }, u.position),
                        u.id,
                    )
                }),
        };
        if let Some(target) = target {
            (
                InputV1::action(InputKindV1::Attack, InputTargetV1::target(target.id)),
                Some(target.position),
            )
        } else {
            (
                match self {
                    Self::AttackMove(goal) => ground(position, goal),
                    _ => InputV1::move_to(position.0, position.1),
                },
                None,
            )
        }
    }
}
fn ground(position: (u64, u64), goal: (u64, u64)) -> InputV1 {
    if distance(position, goal) <= 4_000_000 {
        InputV1::move_to(position.0, position.1)
    } else {
        InputV1::move_to(goal.0, goal.1)
    }
}
/// Screen-space half-width, height above origin, feet below origin and
/// edge forgiveness. Picking and markers share this sprite-derived envelope.
pub fn selection_envelope(frame: crate::camera::CameraFrame, unit: &Unit) -> (f32, f32, f32, f32) {
    let body = unit.body.unwrap_or_default();
    let sx = 2048. / frame.extent.0;
    let sy = 2048. / frame.extent.1;
    if unit.is_minion {
        return (body.width / 2. * sx, (body.height + 5.) * sy, 5. * sy, 1.);
    }
    if unit.is_tower {
        return (
            (body.width.max(31.) / 2. + 3.) * sx,
            (body.height.max(63.) - 8. + 3.) * sy,
            11. * sy,
            3.,
        );
    }
    if !unit.is_champion && !unit.is_tower {
        // Monsters use stable idle/walk art and modest bounded forgiveness.
        // Champion-sized minimum feet pads swamp the smallest jungle bodies.
        return (
            (body.width / 2. + 2.) * sx,
            (body.height + 2.) * sy,
            (body.height * 0.08).clamp(3., 6.) * sy,
            2.,
        );
    }
    // Retain the tested champion body poses, not their combat collision.
    // The foot pad also grows on large sprites, rather than using one size.
    let head = if unit.is_champion {
        (body.height * 0.20).clamp(6., 12.)
    } else {
        0.
    };
    (
        (body.width * 1.05 / 2. + 2.) * sx,
        // Keep the tight lateral/foot envelope from 0.39. Accommodate heads
        // above that baseline with only a
        // bounded upper cap, without restoring full attack-pose dimensions.
        (body.height * 1.05 + 1. + head) * sy,
        (body.height * 0.20).max(12.) * sy,
        3.,
    )
}
pub fn highlight_radii(frame: crate::camera::CameraFrame, unit: &Unit) -> (f32, f32) {
    if unit.is_tower {
        let (half, _, _, margin) = selection_envelope(frame, unit);
        return (half + margin, (half + margin) * 0.45);
    }
    if unit.body.is_some() || unit.is_champion || unit.is_tower {
        let (half, height, bottom, margin) = selection_envelope(frame, unit);
        // Extra head picking must not enlarge the ground highlight.
        let height = if unit.is_champion {
            height
                - (unit.body.unwrap_or_default().height * 0.20).clamp(6., 12.) * 2048.
                    / frame.extent.1
        } else {
            height
        };
        (
            half + margin,
            (height * 0.18 + bottom).max((half + margin) * 0.45),
        )
    } else {
        let r = (unit.radius as f32 / 1000. * 2048. / frame.extent.0).max(12.);
        (r, r * 0.55)
    }
}
pub fn clicked_unit(
    frame: crate::camera::CameraFrame,
    cursor: (f32, f32),
    units: &[Unit],
    champion_only: bool,
) -> Option<usize> {
    clicked_units(frame, cursor, units, champion_only)
        .first()
        .copied()
}
pub fn clicked_units(
    frame: crate::camera::CameraFrame,
    cursor: (f32, f32),
    units: &[Unit],
    champion_only: bool,
) -> Vec<usize> {
    picked(frame, cursor, units, champion_only, None)
}
/// Units under the cursor, best first, in the agreed League-style order
/// (docs/investigation-pass-65.md): enemies before allies, structures last,
/// a body hit before an edge hit, the `sticky` (previously hovered) unit
/// within its tier, then how central the cursor is on each body relative
/// to its size, then the unit drawn in front, then a stable id. There is
/// no blanket champion-first rule: the champion-only key covers fights.
pub fn picked(
    frame: crate::camera::CameraFrame,
    cursor: (f32, f32),
    units: &[Unit],
    champion_only: bool,
    sticky: Option<usize>,
) -> Vec<usize> {
    if !frame.valid() || !frame.viewport.contains(cursor) || frame.minimap.contains(cursor) {
        return Vec::new();
    }
    let mut hits = units
        .iter()
        .filter(|u| !champion_only || u.is_champion)
        .filter_map(|u| {
            let (x0, y0, x1, y1, margin) = screen_area(frame, u);
            if cursor.0 < x0 - margin
                || cursor.0 > x1 + margin
                || cursor.1 < y0 - margin
                || cursor.1 > y1 + margin
            {
                return None;
            }
            let core = cursor.0 >= x0 && cursor.0 <= x1 && cursor.1 >= y0 && cursor.1 <= y1;
            let (hw, hh) = (((x1 - x0) / 2.).max(1.), ((y1 - y0) / 2.).max(1.));
            let (cx, cy) = ((x0 + x1) / 2., (y0 + y1) / 2.);
            let depth = ((cursor.0 - cx) / hw).powi(2) + ((cursor.1 - cy) / hh).powi(2);
            Some((
                u8::from(u.friendly),
                u8::from(u.is_tower),
                u8::from(!core),
                u8::from(sticky != Some(u.id)),
                (depth * 1000.) as u64,
                -(y1 * 10.) as i64, // lower on screen is drawn in front
                u.id,
            ))
        })
        .collect::<Vec<_>>();
    hits.sort_unstable();
    hits.into_iter().map(|h| h.6).collect()
}
/// A unit's measured body in world units (x0, y0, x1, y1), placed with the
/// game's own draw data, with click padding: champions are 3 units wider on
/// each side, and every body reaches 2 units below the feet. None without
/// loaded art or a recent draw.
pub fn body_world_rect(unit: &Unit) -> Option<[f32; 4]> {
    let set = crate::sprite_art::set(unit.body?.art?)?;
    let (center, sprites) = crate::native_adapter::sprite_draw(unit.id)?;
    let [x0, y0, x1, y1] = crate::sprite_art::placed(set, center, &sprites)?;
    let side = if unit.is_champion { 3. } else { 0. };
    Some([x0 - side, y0, x1 + side, y1 + 2.])
}
/// Screen rectangle (x0, y0, x1, y1) of a unit's selection area and its
/// edge forgiveness: the measured body when available, otherwise the
/// bounded envelope.
pub fn screen_area(frame: crate::camera::CameraFrame, u: &Unit) -> (f32, f32, f32, f32, f32) {
    if let Some([x0, y0, x1, y1]) = body_world_rect(u) {
        let a = frame.project_unclipped(x0, y0);
        let b = frame.project_unclipped(x1, y1);
        return (a.0, a.1, b.0, b.1, 3.);
    }
    let p = frame.project_unclipped(u.position.0 as f32 / 1000., u.position.1 as f32 / 1000.);
    if u.is_champion || u.is_tower || u.body.is_some() {
        let (half, height, bottom, margin) = selection_envelope(frame, u);
        return (p.0 - half, p.1 - height, p.0 + half, p.1 + bottom, margin);
    }
    // Compatibility fallback for structures without an art profile.
    let radius = (u.radius as f32 / 1000. * 2048. / frame.extent.0).max(10.);
    (
        p.0 - radius,
        p.1 - radius - 18.,
        p.0 + radius,
        p.1 + radius,
        0.,
    )
}
pub fn hover_color(unit: Unit) -> u32 {
    if unit.friendly {
        0x5caeffff
    } else {
        0xff5b63ff
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cooldown_wait_holds_in_range_but_keeps_target_for_later_attack() {
        let position = (300_000, 200_000);
        let target = Unit {
            position: (350_000, 200_000),
            ..units()[1]
        };
        let order = Order::Attack(target.id);
        for _ in 0..4 {
            let (attack, chase) = order.resolve(position, &[target]);
            assert_eq!(attack.target.target_id, target.id);
            let waiting = rejected_attack(
                position,
                chase,
                Some(attack_in_range(position, target.position, 50_000)),
            );
            assert_eq!((waiting.x, waiting.y), position);
        }
        let outside = Unit {
            position: (350_001, 200_000),
            ..target
        };
        let (_, chase) = order.resolve(position, &[outside]);
        let approach = rejected_attack(
            position,
            chase,
            Some(attack_in_range(position, outside.position, 50_000)),
        );
        assert_eq!((approach.x, approach.y), outside.position);
        let unknown = rejected_attack(position, chase, None);
        assert_eq!((unknown.x, unknown.y), outside.position);
        let (ready, _) = order.resolve(position, &[target]);
        assert_eq!(ready.kind, InputKindV1::Attack.code());
        assert_eq!(ready.target.target_id, target.id);
    }
    #[test]
    fn attack_range_uses_native_floor_distance_and_not_sprite_size() {
        assert!(attack_in_range((0, 0), (3, 4), 5));
        assert!(attack_in_range((0, 0), (5, 3), 5)); // floor(sqrt(34)) = 5.
        assert!(!attack_in_range((0, 0), (6, 0), 5));
        assert!(attack_in_range((u64::MAX, 10), (u64::MAX - 5, 10), 5));
        assert!(!attack_in_range((0, 0), (u64::MAX, u64::MAX), 10_000_000));
    }
    #[test]
    fn minion_vertical_extension_keeps_width_and_structure_top_is_pickable() {
        let minion = Unit {
            is_champion: false,
            is_minion: true,
            body: crate::sprite_picking::entity_body(None, false, true, false, 1000),
            ..units()[1]
        };
        let tower = Unit {
            id: 19,
            is_champion: false,
            is_tower: true,
            body: crate::sprite_picking::entity_body(Some("tower"), false, false, true, 1000),
            ..units()[1]
        };
        for extent in [512., 1024., 2048.] {
            let f = frame(extent);
            for y in [182., 204.] {
                assert_eq!(
                    clicked_unit(f, f.project_unclipped(300., y), &[minion], false),
                    Some(9)
                );
            }
            assert!(clicked_unit(f, f.project_unclipped(312., 190.), &[minion], false).is_none());
            assert_eq!(
                clicked_unit(f, f.project_unclipped(300., 145.), &[tower], false),
                Some(19)
            );
            assert!(clicked_unit(f, f.project_unclipped(350., 180.), &[tower], false).is_none());
            let champion = units()[1];
            assert_eq!(
                clicked_unit(
                    f,
                    f.project_unclipped(300., 182.),
                    &[tower, champion],
                    false
                ),
                Some(9)
            );
            assert!(clicked_unit(f, f.project_unclipped(300., 145.), &[tower], true).is_none());
            assert_eq!(tower.radius, 10_000); // Selection never changes attack range.
        }
    }
    #[test]
    fn enlarged_feet_and_highlights_scale_with_sprite_across_zoom() {
        let small = Unit {
            body: Some(crate::sprite_picking::Body {
                width: 24.,
                height: 40.,
                art: None,
            }),
            ..units()[1]
        };
        let large = Unit {
            is_champion: false,
            body: Some(crate::sprite_picking::Body {
                width: 80.,
                height: 100.,
                art: None,
            }),
            ..small
        };
        for extent in [512., 1024., 2048.] {
            let f = frame(extent);
            assert_eq!(
                clicked_unit(f, f.project_unclipped(300., 211.), &[small], false),
                Some(9)
            );
            assert_eq!(
                clicked_unit(f, f.project_unclipped(340., 205.), &[large], false),
                Some(9)
            );
            assert!(clicked_unit(f, f.project_unclipped(355., 225.), &[large], false).is_none());
            let a = highlight_radii(f, &small);
            let b = highlight_radii(f, &large);
            assert!(b.0 > a.0 * 2. && b.1 > a.1);
        }
    }
    #[test]
    fn monster_bounds_cover_bodies_without_champion_sized_feet_or_attack_extents() {
        let monster = Unit {
            is_champion: false,
            body: Some(crate::sprite_picking::Body {
                width: 29.5,
                height: 39.5,
                art: None,
            }),
            ..units()[1]
        };
        let champion = Unit {
            is_champion: true,
            ..monster
        };
        let small = Unit {
            body: Some(crate::sprite_picking::Body {
                width: 18.5,
                height: 20.5,
                art: None,
            }),
            ..monster
        };
        for extent in [512., 1024., 2048.] {
            let f = frame(extent);
            for point in [(300., 162.), (314., 180.), (300., 203.)] {
                assert_eq!(
                    clicked_unit(f, f.project_unclipped(point.0, point.1), &[monster], false),
                    Some(9)
                );
            }
            assert!(clicked_unit(f, f.project_unclipped(300., 210.), &[monster], false).is_none());
            assert_eq!(
                clicked_unit(f, f.project_unclipped(300., 210.), &[champion], false),
                Some(9)
            );
            // The old maximum attack-frame width accepted distant empty ground.
            assert!(clicked_unit(f, f.project_unclipped(350., 180.), &[monster], false).is_none());
            assert!(clicked_unit(f, f.project_unclipped(300., 209.), &[small], false).is_none());
            let m = highlight_radii(f, &monster);
            let s = highlight_radii(f, &small);
            assert!(m.0 > s.0 && m.1 > s.1);
            assert!(m.0 < highlight_radii(f, &champion).0);
        }
    }
    #[test]
    fn champion_only_acquisition_filters_every_tick_without_losing_destination() {
        let pos = (200_000, 200_000);
        let mut list = units();
        list[1].is_champion = false;
        let order = Order::AttackMove(list[1].position);
        assert_eq!(
            order
                .resolve_policy(pos, &list, false, true, true)
                .0
                .target
                .target_id,
            9
        );
        assert_eq!(
            order
                .resolve_policy(pos, &list, true, true, true)
                .0
                .target
                .target_id,
            2
        );
        assert_eq!(
            order.resolve_policy(pos, &list[1..], true, true, true).0,
            InputV1::move_to(300_000, 200_000)
        );
    }
    fn units() -> [Unit; 2] {
        [
            Unit {
                id: 2,
                position: (210_000, 200_000),
                radius: 10_000,
                is_champion: true,
                is_minion: false,
                friendly: false,
                in_cc: false,
                is_tower: false,
                body: None,
            },
            Unit {
                id: 9,
                position: (300_000, 200_000),
                radius: 10_000,
                is_champion: true,
                is_minion: false,
                friendly: false,
                in_cc: false,
                is_tower: false,
                body: None,
            },
        ]
    }
    fn frame(extent: f32) -> crate::camera::CameraFrame {
        crate::camera::CameraFrame {
            viewport: crate::camera::Rect {
                x: 0.,
                y: 50.,
                w: 1920.,
                h: 974.,
            },
            center: (200., 200.),
            extent: (extent, extent),
            minimap: crate::camera::Rect {
                x: 1581.,
                y: 740.,
                w: 320.,
                h: 320.,
            },
        }
    }
    #[test]
    fn head_padding_extends_only_upward_and_keeps_ground_highlight() {
        let unit = Unit {
            body: Some(crate::sprite_picking::Body {
                width: 24.,
                height: 40.,
                art: None,
            }),
            ..units()[1]
        };
        for extent in [512., 1024., 2048.] {
            let f = frame(extent);
            let sy = 2048. / extent;
            let (half, height, feet, margin) = selection_envelope(f, &unit);
            assert!((half - 14.6 * sy).abs() < 0.001);
            assert!((height - 51. * sy).abs() < 0.001);
            assert!((feet - 12. * sy).abs() < 0.001);
            assert_eq!(margin, 3.);
            assert_eq!(
                clicked_unit(f, f.project_unclipped(300., 150.), &[unit], false),
                Some(9)
            );
            for point in [(300., 140.), (326., 180.), (300., 222.)] {
                assert!(
                    clicked_unit(f, f.project_unclipped(point.0, point.1), &[unit], false)
                        .is_none()
                );
            }
            let ring = highlight_radii(f, &unit);
            assert!((ring.0 - (14.6 * sy + 3.)).abs() < 0.001);
            assert!((ring.1 - (43. * 0.18 + 12.) * sy).abs() < 0.001);
        }
    }
    #[test]
    fn heads_and_shoulders_are_pickable_at_each_zoom_without_changing_collision() {
        let unit = Unit {
            body: Some(crate::sprite_picking::Body {
                width: 24.,
                height: 40.,
                art: None,
            }),
            ..units()[1]
        };
        for extent in [512., 819.2, 1024., 2048.] {
            let frame = frame(extent);
            for world in [
                (300., 163.),
                (310., 180.),
                (300., 198.),
                (300., 207.),
                (315., 204.),
            ] {
                let cursor = frame.project_unclipped(world.0, world.1);
                assert_eq!(clicked_unit(frame, cursor, &[unit], false), Some(9));
            }
            let cursor = frame.project_unclipped(326., 180.);
            assert!(clicked_unit(frame, cursor, &[unit], false).is_none());
        }
        assert_eq!(unit.radius, 10_000); // Picking never mutates combat range/collision.
    }
    #[test]
    fn minion_sprite_boxes_ignore_large_combat_radii_and_monsters_keep_their_size() {
        let minion = Unit {
            is_champion: false,
            is_minion: true,
            radius: 40_000,
            body: crate::sprite_picking::entity_body(None, false, true, false, 40_000),
            ..units()[1]
        };
        let monster = Unit {
            is_minion: false,
            body: Some(crate::sprite_picking::Body {
                width: 60.,
                height: 70.,
                art: None,
            }),
            ..minion
        };
        for extent in [512., 1024., 2048.] {
            let f = frame(extent);
            assert_eq!(
                clicked_unit(f, f.project_unclipped(305., 189.), &[minion], false),
                Some(9)
            );
            assert!(clicked_unit(f, f.project_unclipped(312., 190.), &[minion], false).is_none());
            assert_eq!(
                clicked_unit(f, f.project_unclipped(325., 135.), &[monster], false),
                Some(9)
            );
            assert!(clicked_unit(f, f.project_unclipped(350., 150.), &[monster], false).is_none());
        }
    }
    #[test]
    fn hostile_units_beat_allies_and_identical_overlaps_fall_back_to_stable_ids() {
        let f = frame(1024.);
        let enemy = units()[1];
        let ally = Unit {
            id: 1,
            friendly: true,
            ..enemy
        };
        let minion = Unit {
            id: 2,
            is_champion: false,
            is_minion: true,
            ..enemy
        };
        let cursor = f.project_unclipped(300., 200.);
        for list in [
            [ally, minion, enemy],
            [enemy, minion, ally],
            [minion, ally, enemy],
        ] {
            // No champion-first rule (investigation-pass-65): the minion and
            // champion overlap exactly, so the stable id decides.
            assert_eq!(
                clicked_units(f, cursor, &list, false),
                vec![minion.id, enemy.id, ally.id]
            );
            assert_eq!(
                clicked_units(f, cursor, &list, true),
                vec![enemy.id, ally.id]
            );
        }
        assert_eq!(
            clicked_unit(f, cursor, &[ally, minion], false),
            Some(minion.id)
        );
        assert_eq!(hover_color(ally), 0x5caeffff);
        assert_eq!(hover_color(minion), 0xff5b63ff);
    }
    #[test]
    fn hover_stickiness_and_structures_ranking_last() {
        let f = frame(1024.);
        let a = units()[1];
        let b = Unit { id: 12, ..a };
        let cursor = f.project_unclipped(300., 200.);
        // Identical overlap: the stable id first, unless the other is sticky.
        assert_eq!(picked(f, cursor, &[a, b], false, None), vec![9, 12]);
        assert_eq!(picked(f, cursor, &[a, b], false, Some(12)), vec![12, 9]);
        // A unit in front of a tower beats the tower, even off its centre.
        let tower = Unit {
            id: 3,
            is_champion: false,
            is_tower: true,
            ..a
        };
        let off = f.project_unclipped(305., 205.);
        assert_eq!(picked(f, off, &[tower, a], false, None)[0], 9);
    }
    #[test]
    fn friendly_champions_use_same_legs_box_and_class_priority_as_enemies() {
        let enemy = units()[1];
        let ally = Unit {
            friendly: true,
            ..enemy
        };
        let f = frame(1024.);
        let cursor = f.project_unclipped(300., 207.);
        assert_eq!(clicked_unit(f, cursor, &[enemy], false), Some(9));
        assert_eq!(clicked_unit(f, cursor, &[ally], false), Some(9));
    }
    #[test]
    fn the_more_central_body_wins_and_a_body_hit_beats_an_edge_hit() {
        let champion = Unit {
            id: 9,
            body: Some(crate::sprite_picking::Body {
                width: 24.,
                height: 40.,
                art: None,
            }),
            ..units()[1]
        };
        let minion = Unit {
            id: 2,
            position: (300_000, 180_000),
            is_champion: false,
            is_minion: false,
            friendly: false,
            in_cc: false,
            is_tower: false,
            body: None,
            ..units()[0]
        };
        let f = frame(1024.);
        // Both bodies are hit; the champion's body centre is nearer the
        // cursor than the small unit's, so it ranks first in either order.
        let cursor = f.project_unclipped(300., 180.);
        assert_eq!(
            clicked_units(f, cursor, &[minion, champion], false),
            vec![9, 2]
        );
        assert_eq!(
            clicked_units(f, cursor, &[champion, minion], false),
            vec![9, 2]
        );
        let cursor = f.project_unclipped(300., 175.5);
        // Here the cursor is at the small unit's centre: it wins; the
        // champion-only key still picks the champion.
        assert_eq!(clicked_unit(f, cursor, &[champion, minion], false), Some(2));
        assert_eq!(clicked_unit(f, cursor, &[champion, minion], true), Some(9));
        // Only the champion's forgiveness edge, but the minion's body: the body hit wins.
        let edge = f.project_unclipped(315.5, 180.);
        let nearby = Unit {
            position: (315_500, 184_500),
            ..minion
        };
        assert_eq!(clicked_unit(f, edge, &[champion, nearby], false), Some(2));
        // The champion-only key still picks the champion.
        assert_eq!(clicked_unit(f, edge, &[champion, nearby], true), Some(9));
    }
    #[test]
    fn visible_body_can_be_picked_with_offscreen_feet_but_cursor_must_be_in_battlefield() {
        let f = frame(1024.);
        let unit = Unit {
            position: (300_000, 470_000),
            body: Some(crate::sprite_picking::Body {
                width: 24.,
                height: 40.,
                art: None,
            }),
            ..units()[1]
        };
        assert!(f.project(unit.position).is_none());
        assert_eq!(clicked_unit(f, (1160., 1000.), &[unit], false), Some(9));
        assert!(clicked_unit(f, (1160., 1040.), &[unit], false).is_none());
        assert!(clicked_unit(f, (1600., 760.), &[unit], false).is_none());
        assert!(clicked_unit(f, (f32::NAN, 500.), &[unit], false).is_none());
    }
    #[test]
    fn attack_move_preference_selects_near_champion_or_cursor_and_retains_anchor() {
        let order = Order::AttackMove((305_000, 200_000));
        assert_eq!(
            order
                .resolve_policy((200_000, 200_000), &units(), true, false, true)
                .0
                .target
                .target_id,
            9
        );
        let mut mixed = units();
        mixed[0].is_champion = false;
        assert_eq!(
            order
                .resolve_policy((200_000, 200_000), &mixed, true, false, false)
                .0
                .target
                .target_id,
            2
        );
        assert_eq!(
            order
                .resolve_policy((200_000, 200_000), &mixed, true, true, false)
                .0
                .target
                .target_id,
            9
        );
        assert_eq!(
            order
                .resolve((200_000, 200_000), &units())
                .0
                .target
                .target_id,
            2
        );
        assert_eq!(
            order
                .resolve((200_000, 200_000), &units()[..1])
                .0
                .target
                .target_id,
            2
        );
        assert_eq!(
            order.resolve((200_000, 200_000), &[]).0,
            InputV1::move_to(305_000, 200_000)
        );
        assert_eq!(
            order.resolve((305_000, 200_000), &[]).0,
            InputV1::move_to(305_000, 200_000)
        );
    }
    #[test]
    fn explicit_attack_never_retargets_and_distant_units_do_not_acquire() {
        assert_eq!(
            Order::Attack(9)
                .resolve((200_000, 200_000), &units()[..1])
                .0
                .kind,
            InputKindV1::Move.code()
        );
        assert_eq!(
            Order::AttackMove((700_000, 700_000))
                .resolve((0, 0), &units())
                .0,
            InputV1::move_to(700_000, 700_000)
        );
    }
    #[test]
    fn acquisition_ties_are_stable_across_enumeration_order() {
        let mut u = units();
        u[1].position = u[0].position;
        let order = Order::AttackMove(u[0].position);
        assert_eq!(order.resolve((200_000, 200_000), &u).0.target.target_id, 2);
        u.reverse();
        assert_eq!(order.resolve((200_000, 200_000), &u).0.target.target_id, 2);
    }
    #[test]
    fn attack_move_acquires_on_the_path_toward_an_empty_destination() {
        let order = Order::AttackMove((700_000, 200_000));
        assert_eq!(
            order
                .resolve((200_000, 200_000), &units())
                .0
                .target
                .target_id,
            2
        );
        assert_eq!(
            order.resolve((0, 0), &units()).0,
            InputV1::move_to(700_000, 200_000)
        );
    }
}
