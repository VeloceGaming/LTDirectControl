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
    pub body: Option<crate::sprite_picking::Body>,
}
fn distance(a: (u64, u64), b: (u64, u64)) -> u128 {
    let x = u128::from(a.0.abs_diff(b.0));
    let y = u128::from(a.1.abs_diff(b.1));
    (x * x).saturating_add(y * y)
}
impl Order {
    pub fn resolve(self, position: (u64, u64), units: &[Unit]) -> (InputV1, Option<(u64, u64)>) {
        let target = match self {
            Self::Move(goal) => return (ground(position, goal), None),
            Self::Attack(id) => units.iter().find(|u| u.id == id),
            Self::AttackMove(goal) => units
                .iter()
                .filter(|u| {
                    let limit = u128::from(ACQUISITION_RADIUS).pow(2);
                    distance(goal, u.position) <= limit || distance(position, u.position) <= limit
                })
                .min_by_key(|u| (distance(goal, u.position), u.id)),
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
    if !frame.valid() || !frame.viewport.contains(cursor) || frame.minimap.contains(cursor) {
        return Vec::new();
    }
    let mut hits = units
        .iter()
        .filter(|u| !champion_only || u.is_champion)
        .filter_map(|u| {
            let p = if u.is_champion {
                frame.project_unclipped(u.position.0 as f32 / 1000., u.position.1 as f32 / 1000.)
            } else {
                frame.project(u.position)?
            };
            // Body picking is independent of combat collision/attack range.
            // Rank core sprite hits before forgiving margin hits, then distance
            // to the visible body center (not the entity's feet), then stable ID.
            if u.is_champion {
                let body = u.body.unwrap_or_default();
                let sx = 2048. / frame.extent.0;
                let sy = 2048. / frame.extent.1;
                let half = body.width * sx / 2.;
                let height = body.height * sy;
                let dx = cursor.0 - p.0;
                let dy = cursor.1 - p.1;
                let margin = 3.; // Small screen-space forgiveness at every zoom.
                if dx.abs() > half + margin || dy < -height - margin || dy > margin {
                    return None;
                }
                let core = dx.abs() <= half && dy >= -height && dy <= 0.;
                let center_y = dy + height * 0.5;
                return Some((
                    u8::from(!core),
                    (dx * dx + center_y * center_y) as u64,
                    u.id,
                ));
            }
            // Preserve the smaller legacy minion/structure picker.
            let radius = (u.radius as f32 / 1000. * 2048. / frame.extent.0).max(10.);
            let dx = cursor.0 - p.0;
            let dy = cursor.1 - p.1;
            (dx.abs() <= radius && dy >= -radius - 18. && dy <= radius).then_some((
                0,
                (dx * dx + (dy + 9.).powi(2)) as u64,
                u.id,
            ))
        })
        .collect::<Vec<_>>();
    hits.sort_unstable();
    hits.into_iter().map(|(_, _, id)| id).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn units() -> [Unit; 2] {
        [
            Unit {
                id: 2,
                position: (210_000, 200_000),
                radius: 10_000,
                is_champion: true,
                body: None,
            },
            Unit {
                id: 9,
                position: (300_000, 200_000),
                radius: 10_000,
                is_champion: true,
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
    fn heads_and_shoulders_are_pickable_at_each_zoom_without_changing_collision() {
        let unit = Unit {
            body: Some(crate::sprite_picking::Body {
                width: 24.,
                height: 40.,
            }),
            ..units()[1]
        };
        for extent in [512., 819.2, 1024., 2048.] {
            let frame = frame(extent);
            for world in [(300., 163.), (310., 180.), (300., 198.)] {
                let cursor = frame.project_unclipped(world.0, world.1);
                assert_eq!(clicked_unit(frame, cursor, &[unit], false), Some(9));
            }
            let cursor = frame.project_unclipped(320., 180.);
            assert!(clicked_unit(frame, cursor, &[unit], false).is_none());
        }
        assert_eq!(unit.radius, 10_000); // Picking never mutates combat range/collision.
    }
    #[test]
    fn overlapping_body_centers_replace_feet_ranking_and_margin_does_not_steal_core() {
        let champion = Unit {
            id: 9,
            body: Some(crate::sprite_picking::Body {
                width: 24.,
                height: 40.,
            }),
            ..units()[1]
        };
        let minion = Unit {
            id: 2,
            position: (300_000, 180_000),
            is_champion: false,
            body: None,
            ..units()[0]
        };
        let f = frame(1024.);
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
        assert_eq!(clicked_unit(f, cursor, &[champion, minion], false), Some(2));
        assert_eq!(clicked_unit(f, cursor, &[champion, minion], true), Some(9));
        let edge = f.project_unclipped(313., 180.); // Only padded champion edge.
        let nearby = Unit {
            position: (313_000, 184_500),
            ..minion
        };
        assert_eq!(clicked_unit(f, edge, &[champion, nearby], false), Some(2));
    }
    #[test]
    fn visible_body_can_be_picked_with_offscreen_feet_but_cursor_must_be_in_battlefield() {
        let f = frame(1024.);
        let unit = Unit {
            position: (300_000, 470_000),
            body: Some(crate::sprite_picking::Body {
                width: 24.,
                height: 40.,
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
    fn attack_move_prefers_click_over_champion_and_retains_anchor() {
        let order = Order::AttackMove((305_000, 200_000));
        assert_eq!(
            order
                .resolve((200_000, 200_000), &units())
                .0
                .target
                .target_id,
            9
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
        assert_eq!(order.resolve((0, 0), &u).0.target.target_id, 2);
        u.reverse();
        assert_eq!(order.resolve((0, 0), &u).0.target.target_id, 2);
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
            9
        );
        assert_eq!(
            order.resolve((0, 0), &units()).0,
            InputV1::move_to(700_000, 200_000)
        );
    }
}
