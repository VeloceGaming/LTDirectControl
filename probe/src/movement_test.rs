//! Persistent mouse destinations and own-team selection for the bounded test.
use super::{
    combat::{Order, Unit},
    own_selection::{contract_team, MatchKey, OwnSelection, PlayerIdentity},
    platform_input::Keys,
    Logger,
};
use mod_api_stable::{InputV1, RecordKindV1, StableClient};
use std::sync::Mutex;
use std::time::{Duration, Instant};
const LANES: [&str; 5] = ["top", "jungle", "mid", "bottom", "support"];

#[derive(Default)]
struct State {
    selection: OwnSelection,
    previous_choice: Option<usize>,
    notice: String,
    updated: Option<Instant>,
    target: Option<Order>,
    units: Vec<Unit>,
    units_updated: Option<Instant>,
    previous_left: bool,
    previous_attack_move: bool,
    attack_move_armed: bool,
    marker: Option<(u64, u64)>,
    champion_position: Option<(u64, u64)>,
    previous_right: bool,
    previous_stop: bool,
    previous_recall: bool,
    pending_recall: Option<Instant>,
    cancel_recall: bool,
    native_action: Option<(usize, Instant)>,
    mouse_focused: bool,
    attack_range: Option<(u64, Instant)>,
    hover_keys: Option<Keys>,
}
pub struct MovementTest {
    enabled: bool,
    state: Mutex<State>,
}
impl MovementTest {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            state: Mutex::default(),
        }
    }
    pub fn reset_session(&self, keys: Keys, logger: &Logger) {
        if let Ok(mut s) = self.state.lock() {
            let lane = s.selection.lane;
            *s = State::default();
            s.selection.lane = lane;
            s.previous_choice = keys.selection;
            s.previous_left = keys.left;
            s.previous_right = keys.right;
            s.previous_stop = keys.stop;
            s.previous_recall = keys.recall;
            s.previous_attack_move = keys.attack_move;
            logger.write(&format!(
                "MANUAL RESET lane={lane}; ownership, roster, actor and commands cleared"
            ));
        }
    }
    pub fn load_own_team(&self, ctx: &StableClient<'_>, logger: &Logger) {
        if !self.enabled
            || ctx.scene_kind() != Some(mod_api_stable::SceneKindV1::InGame)
            || self
                .state
                .lock()
                .is_ok_and(|state| state.selection.own_team.is_some())
        {
            return;
        }
        let Some(team) = ctx.player_team_id() else {
            return;
        };
        let Some(name) = ctx.team_name(team).filter(|name| !name.is_empty()) else {
            return;
        };
        let athletes = ctx
            .athlete_ids()
            .into_iter()
            .filter(|athlete| {
                ctx.record_get_json(RecordKindV1::Athlete, *athlete, "contract")
                    .and_then(|json| contract_team(&json))
                    == Some(team)
            })
            .collect::<std::collections::BTreeSet<_>>();
        // An incomplete management-data view is not a resolved ownership
        // snapshot. Keep retrying when the real team and roster become available.
        if athletes.is_empty() {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            if state.selection.resolve_owner(team, name, athletes) {
                logger.write(&format!(
                    "MANUAL own_team={team} name={:?} athletes={:?}",
                    state.selection.team_name, state.selection.athletes
                ));
            }
        }
    }
    pub fn update(&self, keys: Keys, battlefield: bool, pre_match: bool, logger: &Logger) {
        if !self.enabled {
            return;
        }
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let choice = if keys.focused { keys.selection } else { None };
        if choice != state.previous_choice {
            if let Some(lane) = choice.filter(|lane| *lane < 5) {
                let accepted = pre_match && state.selection.choose(lane);
                state.notice = if accepted {
                    format!(
                        "Selected: your {} | Y lock / Space follow in match",
                        LANES[lane]
                    )
                } else {
                    format!(
                        "Selection locked: your {} remains selected",
                        LANES[state.selection.lane]
                    )
                };
                logger.write(&format!(
                    "MANUAL choice lane={lane} accepted={accepted} notice={:?}",
                    state.notice
                ));
            }
            state.previous_choice = choice;
        }
        if !battlefield || !keys.focused {
            state.target = None;
            state.marker = None;
            state.attack_move_armed = false;
            state.pending_recall = None;
        }
        state.updated = Some(Instant::now());
    }
    pub fn begin_match(&self, key: MatchKey, logger: &Logger) {
        if !self.enabled {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            if state.selection.begin(key) {
                state.target = None;
                state.marker = None;
                state.champion_position = None;
                state.updated = None;
                state.units.clear();
                state.units_updated = None;
                state.attack_move_armed = false;
                state.pending_recall = None;
                state.cancel_recall = false;
                state.native_action = None;
                state.attack_range = None;
                state.notice = "Selection locked for this match".into();
                logger.write(&format!(
                    "MANUAL locked own_lane={} key={key:?}",
                    state.selection.lane
                ));
            }
        }
    }
    pub fn register_player(&self, key: MatchKey, player: PlayerIdentity, logger: &Logger) {
        if !self.enabled {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            let before = state.selection.selected().map(|player| player.player);
            state.selection.register(key, player);
            if let Some(selected) = state
                .selection
                .selected()
                .filter(|player| Some(player.player) != before)
            {
                logger.write(&format!(
                    "MANUAL bound player={} athlete={} side={} lane={} champion={:?}",
                    selected.player,
                    selected.athlete,
                    selected.side,
                    selected.lane,
                    selected.champion
                ));
            }
        }
    }
    pub fn selected(&self) -> Option<usize> {
        if !self.enabled {
            return None;
        }
        self.state
            .lock()
            .ok()?
            .selection
            .selected()
            .map(|player| player.player)
    }
    pub fn own_players(&self) -> Vec<PlayerIdentity> {
        self.state
            .lock()
            .map_or(Vec::new(), |s| s.selection.own_players())
    }
    pub fn choose_prepared(&self, key: MatchKey, lane: usize, keys: Keys, logger: &Logger) -> bool {
        let Ok(mut s) = self.state.lock() else {
            return false;
        };
        if !s.selection.choose_prepared(key, lane) {
            return false;
        }
        s.target = None;
        s.marker = None;
        s.champion_position = None;
        s.units.clear();
        s.units_updated = None;
        s.attack_move_armed = false;
        s.pending_recall = None;
        s.cancel_recall = false;
        s.native_action = None;
        s.attack_range = None;
        s.hover_keys = None;
        s.notice.clear();
        s.previous_left = keys.left;
        s.previous_right = keys.right;
        s.previous_recall = keys.recall;
        s.previous_attack_move = keys.attack_move;
        logger.write(&format!(
            "MANUAL PREPARED_SELECTION key={key:?} lane={lane} player={:?}",
            s.selection.selected().map(|p| p.player)
        ));
        true
    }
    pub fn describe(&self) -> Option<String> {
        if !self.enabled {
            return None;
        }
        let state = self.state.lock().ok()?;
        let lane = LANES[state.selection.lane];
        let owner = if state.selection.team_name.is_empty() {
            "your team"
        } else {
            &state.selection.team_name
        };
        Some(if state.selection.match_key.is_none() {
            format!("{owner}: {lane} selected | Ctrl+1..5 before match")
        } else if let Some(player) = state.selection.selected() {
            let side = if player.side == 0 { "BLUE" } else { "RED" };
            format!(
                "Your {lane}: {} ({side}) | Right-click move/attack | A attack-move | Q/W/R | Shift aim | B recall | S stop",
                player.champion,
            )
        } else {
            format!("Your {lane}: waiting for own-team identity")
        })
    }
    pub fn hud_identity(&self) -> Option<(MatchKey, usize)> {
        let state = self.state.lock().ok()?;
        Some((
            state.selection.match_key?,
            state.selection.selected()?.player,
        ))
    }
    pub fn recalling(&self) -> bool {
        self.state.lock().is_ok_and(|s| {
            s.native_action
                .is_some_and(|(a, t)| a == 1 && t.elapsed() <= Duration::from_millis(250))
        })
    }
    pub fn notice(&self) -> Option<String> {
        if !self.enabled {
            return None;
        }
        let state = self.state.lock().ok()?;
        Some(
            if state
                .native_action
                .is_some_and(|(a, t)| a == 1 && t.elapsed() <= Duration::from_millis(250))
            {
                "Recalling: native channel active | Right-click / S / Esc cancels".into()
            } else if state.notice.is_empty() {
                "Ctrl+1 top / 2 jungle / 3 mid / 4 bottom / 5 support".into()
            } else {
                state.notice.clone()
            },
        )
    }
    #[cfg(test)]
    fn input(&self, player: usize, position: (u64, u64)) -> Option<InputV1> {
        self.combat_input(player, position, Vec::new()).map(|r| r.0)
    }
    pub fn combat_input(
        &self,
        player: usize,
        position: (u64, u64),
        units: Vec<Unit>,
    ) -> Option<(InputV1, Option<(u64, u64)>)> {
        if !self.enabled {
            return None;
        }
        let mut state = self.state.lock().ok()?;
        if state.selection.selected()?.player != player {
            return None;
        }
        state.units = units;
        state.units_updated = Some(Instant::now());
        state.champion_position = Some(position);
        // Some always replaces the selected champion's AI input. A hold is
        // completed by the fingerprinted native movement-consumer hook.
        if state
            .updated
            .is_none_or(|t| t.elapsed() > Duration::from_millis(250))
        {
            state.target = None;
            state.pending_recall = None;
        }
        if state
            .pending_recall
            .take()
            .is_some_and(|t| t.elapsed() <= Duration::from_millis(250))
        {
            return Some((InputV1::return_home(), None));
        }
        if matches!(state.target, Some(Order::Attack(id)) if !state.units.iter().any(|u| u.id == id))
        {
            state.target = None;
            state.marker = None;
        }
        if let Some(target) = state.target {
            let result = target.resolve(position, &state.units);
            if matches!(target, Order::Move(_))
                && result.0 == InputV1::move_to(position.0, position.1)
            {
                state.target = None;
            }
            return Some(result);
        }
        Some((InputV1::move_to(position.0, position.1), None))
    }
    pub fn observe_position(&self, position: Option<(u64, u64)>) {
        if let Ok(mut s) = self.state.lock() {
            s.champion_position = position;
            if position.is_none() {
                s.target = None;
                s.marker = None;
                s.units.clear();
                s.attack_move_armed = false;
                s.pending_recall = None;
                s.cancel_recall = false;
                s.native_action = None;
                s.attack_range = None;
            }
        }
    }
    pub fn camera_target(&self) -> Option<(PlayerIdentity, Option<(u64, u64)>)> {
        let s = self.state.lock().ok()?;
        Some((s.selection.selected()?.clone(), s.champion_position))
    }
    pub fn marker(&self) -> Option<(u64, u64)> {
        self.state.lock().ok()?.marker
    }
    pub fn attack_move_armed(&self) -> bool {
        self.state.lock().is_ok_and(|s| s.attack_move_armed)
    }
    pub fn clear_commands(&self) {
        if let Ok(mut s) = self.state.lock() {
            s.target = None;
            s.marker = None;
            s.attack_move_armed = false;
            s.pending_recall = None;
            s.cancel_recall = false;
        }
    }
    pub fn observe_attack_range(&self, key: MatchKey, range: Option<u64>, log: &Logger) {
        if let Ok(mut s) = self.state.lock() {
            if s.selection.match_key != Some(key) {
                return;
            }
            if s.attack_range.map(|(r, _)| r) != range {
                log.write(&format!(
                    "ATTACK RANGE key={key:?} native_effect_range={range:?}"
                ));
            }
            s.attack_range = range.map(|r| (r, Instant::now()));
        }
    }
    fn range_preview(&self) -> Option<((u64, u64), u64)> {
        let s = self.state.lock().ok()?;
        if !s.attack_move_armed || !s.mouse_focused {
            return None;
        }
        let (range, at) = s.attack_range?;
        (at.elapsed() <= Duration::from_millis(250)).then_some((s.champion_position?, range))
    }
    pub fn draw_attack_range(
        &self,
        ctx: &mut StableClient<'_>,
        camera: &crate::camera::CameraControl,
    ) {
        let Some((position, range)) = self.range_preview() else {
            return;
        };
        let Some(frame) = camera.frame() else { return };
        let radius = range as f32 / 1000.;
        for i in 0..128 {
            let point = |j: usize| {
                let theta = j as f32 * std::f32::consts::TAU / 128.;
                frame.project_unclipped(
                    position.0 as f32 / 1000. + theta.cos() * radius,
                    position.1 as f32 / 1000. + theta.sin() * radius,
                )
            };
            crate::abilities::draw_segment(ctx, frame, point(i), point(i + 1), 0xffd700cc);
        }
    }
    fn target_markers(
        &self,
        camera: &crate::camera::CameraControl,
    ) -> (Option<Unit>, Option<Unit>) {
        let Some(frame) = camera.frame() else {
            return (None, None);
        };
        let Ok(s) = self.state.lock() else {
            return (None, None);
        };
        let Some(keys) = s.hover_keys.filter(|k| k.focused) else {
            return (None, None);
        };
        let Some(position) = s.champion_position else {
            return (None, None);
        };
        if s.units_updated
            .is_none_or(|t| t.elapsed() > Duration::from_millis(250))
        {
            return (None, None);
        }
        let hover = keys
            .cursor
            .filter(|p| !camera.command_blocked(*p))
            .filter(|p| frame.unproject(*p).is_some())
            .and_then(|p| {
                crate::combat::clicked_unit(
                    frame,
                    p,
                    &s.units,
                    keys.champion_only && !s.attack_move_armed,
                )
            })
            .and_then(|id| s.units.iter().find(|u| u.id == id))
            .copied();
        let attack = s
            .target
            .map(|order| order.resolve(position, &s.units).0)
            .filter(|input| input.kind == mod_api_stable::InputKindV1::Attack.code())
            .and_then(|input| s.units.iter().find(|u| u.id == input.target.target_id))
            .copied();
        (hover, attack)
    }
    pub fn draw_targets(&self, ctx: &mut StableClient<'_>, camera: &crate::camera::CameraControl) {
        let Some(frame) = camera.frame() else { return };
        let (hover, attack) = self.target_markers(camera);
        // Rings mark the picked unit's feet; an outer orange ring persists for
        // the current attack target. No hidden entity or native sprite pointer.
        for (unit, color, extra) in [(hover, 0xffdf80ff, 3.), (attack, 0xff7858ff, 7.)] {
            let Some(unit) = unit else { continue };
            let Some(center) = frame.project(unit.position) else {
                continue;
            };
            let radius = (unit.radius as f32 / 1000. * 2048. / frame.extent.0).max(12.) + extra;
            for i in 0..48 {
                let point = |j: usize| {
                    let theta = j as f32 * std::f32::consts::TAU / 48.;
                    (
                        center.0 + theta.cos() * radius,
                        center.1 + theta.sin() * radius * 0.55,
                    )
                };
                let (a, b) = (point(i), point(i + 1));
                if !camera.blocked(a) && !camera.blocked(b) {
                    crate::abilities::draw_segment(ctx, frame, a, b, color);
                }
            }
        }
    }
    pub fn apply_ability_action(&self, action: crate::abilities::ClientAction) {
        if let Ok(mut s) = self.state.lock() {
            if action.disarm_attack_move {
                s.attack_move_armed = false;
            }
            if action.new_cast {
                s.pending_recall = None;
            }
        }
    }
    pub fn take_cancel_recall(&self, player: usize) -> bool {
        let Ok(mut s) = self.state.lock() else {
            return false;
        };
        if s.selection.selected().is_none_or(|p| p.player != player) {
            return false;
        }
        std::mem::take(&mut s.cancel_recall)
    }
    pub fn observe_action(&self, action: usize, log: &Logger) {
        if let Ok(mut s) = self.state.lock() {
            let before = s.native_action.map(|(a, _)| a);
            if before != Some(action) && (before == Some(1) || action == 1) {
                log.write(&format!("RECALL NATIVE_ACTION {:?}->{action}", before));
            }
            s.native_action = Some((action, Instant::now()));
        }
    }
    pub fn recall_rejected(&self, log: &Logger) {
        if let Ok(mut s) = self.state.lock() {
            s.notice =
                "Recall not started: native return command unavailable; press B again when ready"
                    .into();
        }
        log.write("RECALL REJECTED; one-shot consumed; manual ownership retained");
    }
    #[cfg(test)]
    pub fn update_mouse(
        &self,
        keys: Keys,
        active: bool,
        camera: &crate::camera::CameraControl,
        log: &Logger,
    ) {
        self.update_mouse_with_cast(keys, active, camera, log, false);
    }
    pub fn update_mouse_with_cast(
        &self,
        keys: Keys,
        active: bool,
        camera: &crate::camera::CameraControl,
        log: &Logger,
        left_reserved: bool,
    ) {
        let Ok(mut s) = self.state.lock() else { return };
        s.hover_keys = active.then_some(keys);
        let focus_returned = keys.focused && !s.mouse_focused;
        let click = keys.right && !s.previous_right && !focus_returned;
        let stop = keys.stop && !s.previous_stop && !focus_returned;
        let left_click = keys.left && !s.previous_left && !focus_returned && !left_reserved;
        let arm = keys.attack_move && !s.previous_attack_move && !focus_returned;
        let recall = keys.recall && !s.previous_recall && !focus_returned;
        s.mouse_focused = keys.focused;
        s.previous_right = keys.right;
        s.previous_stop = keys.stop;
        s.previous_left = keys.left;
        s.previous_attack_move = keys.attack_move;
        s.previous_recall = keys.recall;
        if !active {
            s.target = None;
            s.marker = None;
            s.attack_move_armed = false;
            s.pending_recall = None;
            s.cancel_recall = false;
            return;
        }
        if !keys.focused || stop || keys.escape {
            s.target = None;
            s.marker = None;
            s.attack_move_armed = false;
            s.pending_recall = None;
            if stop || keys.escape {
                s.cancel_recall = true;
            }
            if stop {
                log.write("MANUAL STOP S; cancel selected champion movement");
            }
        } else {
            if recall && !click && !arm {
                s.target = None;
                s.marker = None;
                s.attack_move_armed = false;
                s.cancel_recall = false;
                s.pending_recall = Some(Instant::now());
                log.write(
                    "RECALL PRESS B; previous order cleared; one native return request queued",
                );
                return;
            }
            if arm {
                s.attack_move_armed = true;
                s.pending_recall = None;
                s.cancel_recall = true;
                log.write("MANUAL ATTACK_MOVE armed; left-click to choose destination");
            }
            let attack_move = s.attack_move_armed && left_click;
            if !click && !attack_move {
                return;
            }
            if click {
                s.attack_move_armed = false;
                s.pending_recall = None;
            }
            let frame = camera.frame();
            let target = keys
                .cursor
                .filter(|p| !camera.command_blocked(*p))
                .and_then(|p| frame.and_then(|f| f.unproject(p)));
            if let Some(target) = target {
                let hit = s
                    .units_updated
                    .filter(|t| t.elapsed() <= Duration::from_millis(250))
                    .and_then(|_| {
                        crate::combat::clicked_unit(
                            frame?,
                            keys.cursor?,
                            &s.units,
                            keys.champion_only && !attack_move,
                        )
                    });
                let order = if let Some(id) = hit {
                    Order::Attack(id)
                } else if attack_move {
                    Order::AttackMove(target)
                } else {
                    Order::Move(target)
                };
                s.target = Some(order);
                s.cancel_recall = true;
                s.attack_move_armed = false;
                s.marker = Some(target);
                log.write(&format!(
                    "MANUAL CLICK player={:?} order={order:?} champion_only={} cursor={:?} target={target:?} camera={frame:?}",
                    s.selection.selected().map(|p| p.player),
                    keys.champion_only,
                    keys.cursor
                ));
            } else {
                log.write(&format!(
                    "MANUAL CLICK ignored cursor={:?}; UI/minimap/off-map or camera unavailable",
                    keys.cursor
                ));
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::super::timing_test::tests::logger;
    use super::*;
    fn keys(dx: i8, dy: i8) -> Keys {
        Keys {
            focused: true,
            dx,
            dy,
            selection: None,
            ..Keys::default()
        }
    }
    fn movement() -> MovementTest {
        let movement = MovementTest::new(true);
        movement.state.lock().unwrap().selection.athletes.insert(60);
        movement
    }
    #[test]
    fn prepared_switch_drops_chase_recall_actor_and_held_click_edges() {
        let m = movement();
        let log = logger("prepared-switch");
        start(&m, &log, 0);
        m.state.lock().unwrap().selection.athletes.insert(61);
        m.register_player(
            (1, 33, 1),
            PlayerIdentity {
                player: 8,
                athlete: 61,
                lane: 3,
                side: 1,
                champion: "harpy".into(),
            },
            &log,
        );
        {
            let mut s = m.state.lock().unwrap();
            s.target = Some(Order::Attack(99));
            s.marker = Some((1, 2));
            s.champion_position = Some((20, 30));
            s.pending_recall = Some(Instant::now());
            s.native_action = Some((1, Instant::now()));
            s.attack_move_armed = true;
        }
        let held = Keys {
            left: true,
            right: true,
            recall: true,
            attack_move: true,
            ..keys(0, 0)
        };
        assert!(!m.choose_prepared((2, 33, 2), 3, held, &log));
        assert_eq!(m.selected(), Some(5));
        assert!(m.choose_prepared((1, 33, 1), 3, held, &log));
        assert_eq!(m.selected(), Some(8));
        let s = m.state.lock().unwrap();
        assert!(s.target.is_none() && s.marker.is_none() && s.champion_position.is_none());
        assert!(s.pending_recall.is_none() && s.native_action.is_none() && !s.attack_move_armed);
        assert!(s.previous_left && s.previous_right && s.previous_recall && s.previous_attack_move);
    }
    #[test]
    fn a_direct_click_commits_chases_and_clears_on_target_loss_instead_of_switching() {
        let movement = movement();
        let log = logger("a-direct-chase");
        start(&movement, &log, 0);
        let camera = crate::camera::CameraControl::default();
        camera.capture(crate::camera::CameraFrame {
            viewport: crate::camera::Rect {
                x: 0.,
                y: 50.,
                w: 1920.,
                h: 974.,
            },
            center: (200., 200.),
            extent: (1024., 1024.),
            minimap: crate::camera::Rect {
                x: 1581.,
                y: 740.,
                w: 320.,
                h: 320.,
            },
        });
        let pos = (200_000, 200_000);
        let mut units = vec![
            Unit {
                id: 2,
                position: (210_000, 200_000),
                radius: 10_000,
                is_champion: false,
                body: None,
            },
            Unit {
                id: 9,
                position: (300_000, 200_000),
                radius: 10_000,
                is_champion: true,
                body: None,
            },
        ];
        let click = Keys {
            cursor: Some((1160., 527.)),
            champion_only: true,
            ..keys(0, 0)
        };
        movement.update(click, true, false, &log);
        movement.update_mouse(click, true, &camera, &log);
        movement.combat_input(5, pos, units.clone());
        movement.update_mouse(
            Keys {
                attack_move: true,
                ..click
            },
            true,
            &camera,
            &log,
        );
        assert_eq!(movement.target_markers(&camera).0.unwrap().id, 9);
        movement.update_mouse(
            Keys {
                left: true,
                ..click
            },
            true,
            &camera,
            &log,
        );
        units[1].position = (500_000, 200_000); // Leaves range and the clicked spot.
        let (request, chase) = movement.combat_input(5, pos, units.clone()).unwrap();
        assert_eq!(request.target.target_id, 9);
        assert_eq!(chase, Some(units[1].position));
        movement.update_mouse(
            Keys {
                cursor: Some((100., 300.)),
                ..click
            },
            true,
            &camera,
            &log,
        );
        let (hover, attack) = movement.target_markers(&camera);
        assert!(hover.is_none());
        assert_eq!(attack.unwrap().id, 9);
        assert_eq!(
            movement
                .combat_input(5, pos, units[..1].to_vec())
                .unwrap()
                .0,
            InputV1::move_to(pos.0, pos.1)
        );
        assert!(movement.target_markers(&camera).1.is_none());
        // Reappearance must not resurrect a canceled attack.
        assert_eq!(
            movement.combat_input(5, pos, units).unwrap().0,
            InputV1::move_to(pos.0, pos.1)
        );
    }
    #[test]
    fn hover_matches_a_and_champion_only_picking_and_hides_on_mask_stale_death_pause() {
        let movement = movement();
        let log = logger("hover-picking");
        start(&movement, &log, 0);
        let camera = crate::camera::CameraControl::default();
        camera.capture(crate::camera::CameraFrame {
            viewport: crate::camera::Rect {
                x: 0.,
                y: 50.,
                w: 1920.,
                h: 974.,
            },
            center: (200., 200.),
            extent: (1024., 1024.),
            minimap: crate::camera::Rect {
                x: 1581.,
                y: 740.,
                w: 320.,
                h: 320.,
            },
        });
        let units = vec![
            Unit {
                id: 2,
                position: (300_000, 200_000),
                radius: 10_000,
                is_champion: false,
                body: None,
            },
            Unit {
                id: 9,
                position: (300_000, 200_000),
                radius: 10_000,
                is_champion: true,
                body: None,
            },
        ];
        let click = Keys {
            cursor: Some((1160., 537.)),
            champion_only: true,
            ..keys(0, 0)
        };
        movement.update(click, true, false, &log);
        movement.update_mouse(click, true, &camera, &log);
        movement.combat_input(5, (200_000, 200_000), units.clone());
        assert_eq!(movement.target_markers(&camera).0.unwrap().id, 9);
        movement.update_mouse(
            Keys {
                attack_move: true,
                ..click
            },
            true,
            &camera,
            &log,
        );
        assert_eq!(movement.target_markers(&camera).0.unwrap().id, 2);
        movement.update_mouse(
            Keys {
                left: true,
                ..click
            },
            true,
            &camera,
            &log,
        );
        assert_eq!(
            movement
                .combat_input(5, (200_000, 200_000), units.clone())
                .unwrap()
                .0
                .target
                .target_id,
            2
        );
        assert_eq!(movement.target_markers(&camera).1.unwrap().id, 2);
        camera.set_command_blocked(vec![crate::camera::Rect {
            x: 1100.,
            y: 500.,
            w: 100.,
            h: 100.,
        }]);
        assert!(movement.target_markers(&camera).0.is_none());
        camera.set_command_blocked(Vec::new());
        movement.state.lock().unwrap().units_updated =
            Some(Instant::now() - Duration::from_secs(1));
        assert!(
            movement.target_markers(&camera).0.is_none()
                && movement.target_markers(&camera).1.is_none()
        );
        movement.combat_input(5, (200_000, 200_000), units.clone());
        movement.update_mouse(click, false, &camera, &log);
        assert!(movement.target_markers(&camera).0.is_none());
        movement.update_mouse(click, true, &camera, &log);
        movement.observe_position(None);
        assert!(
            movement.target_markers(&camera).0.is_none()
                && movement.target_markers(&camera).1.is_none()
        );
    }
    #[test]
    fn session_reset_refreshes_owner_and_drops_previous_orders_and_held_inputs() {
        let movement = movement();
        let log = logger("movement-session-reset");
        movement.state.lock().unwrap().selection.choose(2);
        start(&movement, &log, 2);
        {
            let mut s = movement.state.lock().unwrap();
            s.target = Some(Order::Move((700_000, 800_000)));
            s.champion_position = Some((200_000, 200_000));
            s.attack_move_armed = true;
            s.pending_recall = Some(Instant::now());
            s.native_action = Some((1, Instant::now()));
            s.attack_range = Some((72_000, Instant::now()));
        }
        let held = Keys {
            left: true,
            right: true,
            attack_move: true,
            recall: true,
            selection: Some(2),
            ..Keys::default()
        };
        movement.reset_session(held, &log);
        assert!(movement.hud_identity().is_none());
        assert!(movement.selected().is_none());
        assert!(movement.camera_target().is_none());
        {
            let s = movement.state.lock().unwrap();
            assert_eq!(s.selection.lane, 2);
            assert!(s.selection.own_team.is_none() && s.selection.athletes.is_empty());
            assert!(s.target.is_none() && s.pending_recall.is_none() && s.native_action.is_none());
            assert!(s.attack_range.is_none() && s.units.is_empty() && !s.attack_move_armed);
            assert!(
                s.previous_right && s.previous_left && s.previous_recall && s.previous_attack_move
            );
        }
        // A different loaded save can resolve a different team/side and lane.
        let key = (99, 33, 1);
        {
            let mut s = movement.state.lock().unwrap();
            assert!(s.selection.choose(4));
            assert!(s.selection.resolve_owner(3, "new save".into(), [91].into()));
        }
        movement.begin_match(key, &log);
        movement.register_player(
            key,
            PlayerIdentity {
                player: 4,
                athlete: 91,
                lane: 4,
                side: 0,
                champion: "harpy".into(),
            },
            &log,
        );
        assert_eq!(movement.hud_identity(), Some((key, 4)));
        assert_eq!(
            movement
                .state
                .lock()
                .unwrap()
                .selection
                .selected()
                .unwrap()
                .side,
            0
        );
    }
    fn start(movement: &MovementTest, log: &Logger, lane: usize) {
        movement.begin_match((1, 33, 1), log);
        movement.register_player(
            (1, 33, 1),
            PlayerIdentity {
                player: 5 + lane,
                athlete: 60,
                lane,
                side: 1,
                champion: "soldier".into(),
            },
            log,
        );
    }
    #[test]
    fn attack_range_aiming_persists_through_camera_and_cancels_with_gameplay_commands() {
        let movement = movement();
        let log = logger("attack-range-mode");
        start(&movement, &log, 0);
        let camera = crate::camera::CameraControl::default();
        camera.capture(crate::camera::CameraFrame {
            viewport: crate::camera::Rect {
                x: 0.,
                y: 50.,
                w: 1920.,
                h: 974.,
            },
            center: (200., 200.),
            extent: (1024., 1024.),
            minimap: crate::camera::Rect {
                x: 1581.,
                y: 740.,
                w: 320.,
                h: 320.,
            },
        });
        movement.observe_position(Some((200_000, 200_000)));
        movement.observe_attack_range((1, 33, 1), Some(72_000), &log);
        movement.update_mouse(keys(0, 0), true, &camera, &log);
        let arm = Keys {
            attack_move: true,
            ..keys(0, 0)
        };
        movement.update_mouse(arm, true, &camera, &log);
        assert_eq!(movement.range_preview(), Some(((200_000, 200_000), 72_000)));
        movement.update_mouse(
            Keys {
                camera_toggle: true,
                space: true,
                team_info: true,
                ..keys(0, 0)
            },
            true,
            &camera,
            &log,
        );
        assert!(movement.range_preview().is_some());
        movement.update_mouse(
            Keys {
                left: true,
                cursor: Some((1160., 537.)),
                ..keys(0, 0)
            },
            true,
            &camera,
            &log,
        );
        assert!(movement.range_preview().is_none());
        for cancel in [
            Keys {
                right: true,
                ..keys(0, 0)
            },
            Keys {
                stop: true,
                ..keys(0, 0)
            },
            Keys {
                escape: true,
                ..keys(0, 0)
            },
            Keys {
                recall: true,
                ..keys(0, 0)
            },
        ] {
            movement.update_mouse(keys(0, 0), true, &camera, &log);
            movement.update_mouse(arm, true, &camera, &log);
            assert!(movement.range_preview().is_some());
            movement.update_mouse(cancel, true, &camera, &log);
            assert!(movement.range_preview().is_none());
        }
        movement.update_mouse(keys(0, 0), true, &camera, &log);
        movement.update_mouse(arm, true, &camera, &log);
        movement.apply_ability_action(crate::abilities::ClientAction {
            disarm_attack_move: true,
            ..Default::default()
        });
        assert!(movement.range_preview().is_none());
        movement.update_mouse(keys(0, 0), true, &camera, &log);
        movement.update_mouse(arm, true, &camera, &log);
        movement
            .state
            .lock()
            .unwrap()
            .attack_range
            .as_mut()
            .unwrap()
            .1 = Instant::now() - Duration::from_secs(1);
        assert!(movement.range_preview().is_none());
        movement.observe_attack_range((1, 33, 2), Some(100_000), &log);
        assert!(movement.range_preview().is_none()); // Wrong match cannot refresh stale data.
        movement.observe_attack_range((1, 33, 1), Some(100_000), &log);
        assert_eq!(movement.range_preview().unwrap().1, 100_000);
        movement.observe_position(None);
        assert!(movement.range_preview().is_none());
    }
    #[test]
    fn paused_inputs_cannot_arm_or_submit_orders_when_held_through_resume() {
        let movement = movement();
        let log = logger("pause-held-orders");
        start(&movement, &log, 0);
        let camera = crate::camera::CameraControl::default();
        let held = Keys {
            attack_move: true,
            right: true,
            recall: true,
            ..keys(0, 0)
        };
        movement.update_mouse(keys(0, 0), true, &camera, &log);
        movement.clear_commands();
        movement.update_mouse(held, false, &camera, &log);
        movement.update_mouse(held, true, &camera, &log);
        assert!(!movement.attack_move_armed());
        let s = movement.state.lock().unwrap();
        assert!(s.target.is_none());
        assert!(s.pending_recall.is_none());
    }
    #[test]
    fn champion_only_direct_clicks_ignore_minions_but_attack_move_keeps_them() {
        let movement = movement();
        let log = logger("champion-only-orders");
        start(&movement, &log, 0);
        let camera = crate::camera::CameraControl::default();
        camera.capture(crate::camera::CameraFrame {
            viewport: crate::camera::Rect {
                x: 0.,
                y: 50.,
                w: 1920.,
                h: 974.,
            },
            center: (200., 200.),
            extent: (1024., 1024.),
            minimap: crate::camera::Rect {
                x: 1581.,
                y: 740.,
                w: 320.,
                h: 320.,
            },
        });
        let units = vec![
            Unit {
                id: 2,
                position: (300_000, 200_000),
                radius: 10_000,
                is_champion: false,
                body: None,
            },
            Unit {
                id: 9,
                position: (300_000, 200_000),
                radius: 10_000,
                is_champion: true,
                body: None,
            },
        ];
        let pos = (200_000, 200_000);
        movement.update(keys(0, 0), true, false, &log);
        movement.update_mouse(keys(0, 0), true, &camera, &log);
        movement.combat_input(5, pos, units.clone());
        let mut click = Keys {
            right: true,
            champion_only: true,
            cursor: Some((1160., 537.)),
            ..keys(0, 0)
        };
        movement.update_mouse(click, true, &camera, &log);
        assert_eq!(
            movement
                .combat_input(5, pos, units.clone())
                .unwrap()
                .0
                .target
                .target_id,
            9
        );
        click.right = false;
        movement.update_mouse(click, true, &camera, &log);
        click.right = true;
        click.champion_only = false;
        movement.update_mouse(click, true, &camera, &log);
        assert_eq!(
            movement
                .combat_input(5, pos, units.clone())
                .unwrap()
                .0
                .target
                .target_id,
            2
        );
        // With no champion at the cursor, the new direct click moves to ground.
        movement.combat_input(5, pos, units[..1].to_vec());
        click.right = false;
        click.champion_only = true;
        movement.update_mouse(click, true, &camera, &log);
        click.right = true;
        movement.update_mouse(click, true, &camera, &log);
        assert_eq!(
            movement
                .combat_input(5, pos, units[..1].to_vec())
                .unwrap()
                .0,
            InputV1::move_to(300_000, 200_000)
        );
        click.right = false;
        click.attack_move = true;
        movement.update_mouse(click, true, &camera, &log);
        click.attack_move = false;
        click.left = true;
        movement.update_mouse(click, true, &camera, &log);
        assert_eq!(
            movement
                .combat_input(5, pos, units[..1].to_vec())
                .unwrap()
                .0
                .target
                .target_id,
            2
        );
    }
    #[test]
    fn own_red_lane_selection_is_confirmed_and_locks_before_commands() {
        let movement = movement();
        let log = logger("selection");
        movement.update(
            Keys {
                selection: Some(2),
                ..keys(0, 0)
            },
            false,
            true,
            &log,
        );
        assert!(movement.notice().unwrap().contains("Selected: your mid"));
        start(&movement, &log, 2);
        movement.update(
            Keys {
                selection: Some(4),
                ..keys(1, 0)
            },
            true,
            true,
            &log,
        );
        assert_eq!(movement.selected(), Some(7));
        assert!(movement
            .notice()
            .unwrap()
            .contains("Selection locked: your mid"));
        assert_eq!(movement.input(2, (10, 10)), None);
        assert_eq!(movement.input(7, (10, 10)), Some(InputV1::move_to(10, 10)));
    }
    #[test]
    fn idle_focus_loss_and_expired_directions_hold_instead_of_passing_to_ai() {
        let movement = movement();
        let log = logger("release");
        start(&movement, &log, 0);
        for (pressed, battlefield) in [
            (keys(0, 0), true),
            (
                Keys {
                    focused: false,
                    ..keys(1, 0)
                },
                true,
            ),
            (keys(1, 0), false),
        ] {
            movement.update(pressed, battlefield, false, &log);
            assert_eq!(
                movement.input(5, (200_000, 200_000)),
                Some(InputV1::move_to(200_000, 200_000))
            );
            assert_eq!(movement.input(6, (200_000, 200_000)), None);
        }
        movement.update(keys(-1, -1), true, false, &log);
        assert_eq!(
            movement.input(5, (15_000, 913_000)),
            Some(InputV1::move_to(15_000, 913_000))
        );
        movement.state.lock().unwrap().updated = Some(Instant::now() - Duration::from_secs(1));
        assert_eq!(
            movement.input(5, (15_000, 913_000)),
            Some(InputV1::move_to(15_000, 913_000))
        );
    }
    #[test]
    fn old_client_commands_are_cleared_when_match_starts() {
        let movement = movement();
        let log = logger("start");
        movement.update(keys(1, 0), true, false, &log);
        start(&movement, &log, 0);
        assert_eq!(movement.input(5, (1, 1)), Some(InputV1::move_to(1, 1)));
    }
    fn recall_camera() -> crate::camera::CameraControl {
        let camera = crate::camera::CameraControl::default();
        camera.capture(crate::camera::CameraFrame {
            viewport: crate::camera::Rect {
                x: 0.,
                y: 50.,
                w: 1920.,
                h: 974.,
            },
            center: (200., 200.),
            extent: (1024., 1024.),
            minimap: crate::camera::Rect {
                x: 1581.,
                y: 740.,
                w: 320.,
                h: 320.,
            },
        });
        camera
    }
    #[test]
    fn b_clears_old_orders_submits_once_and_never_restarts_on_hold() {
        let movement = movement();
        let log = logger("recall-one-shot");
        start(&movement, &log, 0);
        let camera = recall_camera();
        let pos = (200_000, 200_000);
        movement.update(keys(0, 0), true, false, &log);
        movement.update_mouse(keys(0, 0), true, &camera, &log);
        movement.state.lock().unwrap().target = Some(Order::AttackMove((300_000, 200_000)));
        let b = Keys {
            recall: true,
            ..keys(0, 0)
        };
        movement.update_mouse(b, true, &camera, &log);
        assert_eq!(movement.input(5, pos), Some(InputV1::return_home()));
        assert_eq!(movement.input(5, pos), Some(InputV1::move_to(pos.0, pos.1)));
        assert!(movement.state.lock().unwrap().target.is_none());
        movement.update_mouse(b, true, &camera, &log);
        assert_eq!(movement.input(5, pos), Some(InputV1::move_to(pos.0, pos.1)));
        movement.observe_action(1, &log);
        assert!(movement.notice().unwrap().starts_with("Recalling:"));
        movement.observe_action(0, &log);
        assert!(!movement.notice().unwrap().starts_with("Recalling:"));
    }
    #[test]
    fn recall_cancellation_is_selected_only_and_pending_recall_expires() {
        let movement = movement();
        let log = logger("recall-cancel");
        start(&movement, &log, 0);
        let camera = recall_camera();
        let pos = (200_000, 200_000);
        movement.update(keys(0, 0), true, false, &log);
        movement.update_mouse(keys(0, 0), true, &camera, &log);
        movement.update_mouse(
            Keys {
                recall: true,
                ..keys(0, 0)
            },
            true,
            &camera,
            &log,
        );
        movement.state.lock().unwrap().pending_recall =
            Some(Instant::now() - Duration::from_secs(1));
        assert_eq!(movement.input(5, pos), Some(InputV1::move_to(pos.0, pos.1)));
        movement.update_mouse(
            Keys {
                stop: true,
                ..keys(0, 0)
            },
            true,
            &camera,
            &log,
        );
        assert!(!movement.take_cancel_recall(6));
        assert!(movement.take_cancel_recall(5));
        assert!(!movement.take_cancel_recall(5));
        movement.update_mouse(keys(0, 0), true, &camera, &log);
        movement.update_mouse(
            Keys {
                right: true,
                cursor: Some((960., 537.)),
                ..keys(0, 0)
            },
            true,
            &camera,
            &log,
        );
        assert!(movement.take_cancel_recall(5)); // Even a move click at the current position cancels recall.
        movement.recall_rejected(&log);
        assert!(movement.notice().unwrap().contains("Recall not started"));
        assert_eq!(movement.input(5, pos), Some(InputV1::move_to(pos.0, pos.1)));
    }
    #[test]
    fn aiming_disarms_attack_move_and_confirmation_click_is_not_reused() {
        let movement = movement();
        let log = logger("normalcast-order-routing");
        start(&movement, &log, 0);
        let camera = recall_camera();
        let pos = (200_000, 200_000);
        movement.update(keys(0, 0), true, false, &log);
        movement.update_mouse(keys(0, 0), true, &camera, &log);
        movement.update_mouse(
            Keys {
                attack_move: true,
                ..keys(0, 0)
            },
            true,
            &camera,
            &log,
        );
        assert!(movement.attack_move_armed());
        movement.apply_ability_action(crate::abilities::ClientAction {
            disarm_attack_move: true,
            ..Default::default()
        });
        assert!(!movement.attack_move_armed());
        let left = Keys {
            left: true,
            cursor: Some((1160., 537.)),
            ..keys(0, 0)
        };
        movement.update_mouse_with_cast(left, true, &camera, &log, true);
        // An A press while the same left button remains held must not act like a new click.
        movement.update_mouse(
            Keys {
                attack_move: true,
                ..left
            },
            true,
            &camera,
            &log,
        );
        assert!(movement.attack_move_armed());
        assert_eq!(movement.input(5, pos), Some(InputV1::move_to(pos.0, pos.1)));
    }

    #[test]
    fn mouse_goal_persists_after_button_release_and_s_stops() {
        let movement = movement();
        let log = logger("mouse-goal");
        start(&movement, &log, 0);
        let camera = crate::camera::CameraControl::default();
        camera.capture(crate::camera::CameraFrame {
            viewport: crate::camera::Rect {
                x: 0.,
                y: 50.,
                w: 1920.,
                h: 974.,
            },
            center: (200., 200.),
            extent: (1024., 1024.),
            minimap: crate::camera::Rect {
                x: 1581.,
                y: 740.,
                w: 320.,
                h: 320.,
            },
        });
        movement.update_mouse(keys(0, 0), false, &camera, &log);
        let click = Keys {
            right: true,
            cursor: Some((1160., 537.)),
            ..keys(0, 0)
        };
        movement.update(click, true, false, &log);
        movement.update_mouse(click, true, &camera, &log);
        assert_eq!(
            movement.input(5, (200_000, 200_000)),
            Some(InputV1::move_to(300_000, 200_000))
        );
        movement.update(keys(0, 0), true, false, &log);
        movement.update_mouse(keys(0, 0), true, &camera, &log);
        assert_eq!(
            movement.input(5, (203_000, 200_000)),
            Some(InputV1::move_to(300_000, 200_000))
        );
        assert_eq!(movement.input(6, (203_000, 200_000)), None);
        let stop = Keys {
            stop: true,
            ..keys(0, 0)
        };
        movement.update_mouse(stop, true, &camera, &log);
        assert_eq!(
            movement.input(5, (204_000, 200_000)),
            Some(InputV1::move_to(204_000, 200_000))
        );
        movement.update_mouse(keys(0, 0), true, &camera, &log);
        movement.update_mouse(click, false, &camera, &log);
        movement.update_mouse(click, true, &camera, &log);
        assert_eq!(
            movement.input(5, (204_000, 200_000)),
            Some(InputV1::move_to(204_000, 200_000))
        );
        movement.update_mouse(keys(0, 0), true, &camera, &log);
        movement.update_mouse(click, true, &camera, &log);
        assert_eq!(
            movement.input(5, (299_000, 200_000)),
            Some(InputV1::move_to(299_000, 200_000))
        );
    }
    #[test]
    fn live_orders_target_clicked_unit_and_attack_move_through_scoreboard() {
        let movement = movement();
        let log = logger("combat-orders");
        start(&movement, &log, 0);
        let camera = crate::camera::CameraControl::default();
        camera.capture(crate::camera::CameraFrame {
            viewport: crate::camera::Rect {
                x: 0.,
                y: 50.,
                w: 1920.,
                h: 974.,
            },
            center: (200., 200.),
            extent: (1024., 1024.),
            minimap: crate::camera::Rect {
                x: 1581.,
                y: 740.,
                w: 320.,
                h: 320.,
            },
        });
        let scoreboard = crate::camera::Rect {
            x: 506.,
            y: 390.,
            w: 907.,
            h: 300.,
        };
        camera.set_blocked(vec![scoreboard]);
        camera.set_command_blocked(vec![]);
        let units = vec![
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
        ];
        let pos = (200_000, 200_000);
        movement.update(keys(0, 0), true, false, &log);
        movement.update_mouse(keys(0, 0), true, &camera, &log);
        movement.combat_input(5, pos, units.clone());
        let cursor = Some((1160., 527.)); // Enemy sprite, ten UI pixels above its ground point.
        movement.update_mouse(
            Keys {
                right: true,
                cursor,
                ..keys(0, 0)
            },
            true,
            &camera,
            &log,
        );
        let attack = movement.combat_input(5, pos, units.clone()).unwrap().0;
        assert_eq!(
            attack,
            InputV1::action(
                mod_api_stable::InputKindV1::Attack,
                mod_api_stable::InputTargetV1::target(9)
            )
        );
        // Explicit attacks do not retarget when their target dies/disappears.
        assert_eq!(
            movement
                .combat_input(5, pos, units[..1].to_vec())
                .unwrap()
                .0,
            InputV1::move_to(pos.0, pos.1)
        );
        movement.combat_input(5, pos, units.clone());
        let cursor = Some((1200., 527.)); // Ground beside enemy: keep acquisition/retarget behavior.
        movement.update_mouse(
            Keys {
                attack_move: true,
                cursor,
                ..keys(0, 0)
            },
            true,
            &camera,
            &log,
        );
        assert!(movement.attack_move_armed());
        movement.update_mouse(
            Keys {
                left: true,
                cursor,
                ..keys(0, 0)
            },
            true,
            &camera,
            &log,
        );
        assert!(!movement.attack_move_armed());
        assert_eq!(
            movement
                .combat_input(5, pos, units.clone())
                .unwrap()
                .0
                .target
                .target_id,
            9
        );
        // Retarget after the preferred unit dies, then S clears the entire order.
        assert_eq!(
            movement
                .combat_input(5, pos, units[..1].to_vec())
                .unwrap()
                .0
                .target
                .target_id,
            2
        );
        movement.update_mouse(
            Keys {
                stop: true,
                ..keys(0, 0)
            },
            true,
            &camera,
            &log,
        );
        assert_eq!(
            movement.combat_input(5, pos, units.clone()).unwrap().0,
            InputV1::move_to(pos.0, pos.1)
        );
        // Genuine HUD still blocks commands; only the scoreboard is excluded.
        camera.set_command_blocked(vec![scoreboard]);
        movement.update_mouse(keys(0, 0), true, &camera, &log);
        movement.update_mouse(
            Keys {
                right: true,
                cursor,
                ..keys(0, 0)
            },
            true,
            &camera,
            &log,
        );
        assert_eq!(
            movement.combat_input(5, pos, units).unwrap().0,
            InputV1::move_to(pos.0, pos.1)
        );
    }
}
