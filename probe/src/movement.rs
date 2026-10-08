//! Persistent player orders (move, attack, attack-move, stop), movement
//! steering toward them, click feedback and own-team athlete selection.
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
pub(crate) const ATTACK_CLICK_DURATION: Duration = Duration::from_millis(120);
#[derive(Clone, Copy)]
pub struct ClickFeedback {
    pub position: (u64, u64),
    pub attack: bool,
    pub minimap: bool,
    pub at: Instant,
}

#[derive(Default)]
struct State {
    navigation: Option<crate::map_path::Grid>,
    pending_navigation: Option<crate::map_path::Grid>,
    route: std::collections::VecDeque<(u64, u64)>,
    trace_at: Option<Instant>,
    trace_count: usize,
    steering_trace: Option<((u64, u64), bool)>,
    selection: OwnSelection,
    previous_choice: Option<usize>,
    notice: String,
    updated: Option<Instant>,
    target: Option<Order>,
    command_stamp: Option<crate::input_trace::Stamp>,
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
    /// Latest manual ground Move or S stop. Attack, A-click and recall clear
    /// it; automatic chase/hold inputs never set it.
    manual_release: Option<Instant>,
    native_action: Option<(usize, Instant)>,
    mouse_focused: bool,
    attack_range: Option<(u64, Instant)>,
    hover_keys: Option<Keys>,
    hover_units: Vec<Unit>,
    hover_updated: Option<Instant>,
    hover_actor: Option<usize>,
    frame_markers: (Option<Unit>, Option<Unit>),
    attack_click: Option<(usize, Instant)>,
    frame_click: Option<(usize, Instant)>,
    champion_only: bool,
    clicks: std::collections::VecDeque<ClickFeedback>,
}
pub struct Movement {
    enabled: bool,
    state: Mutex<State>,
}
impl Movement {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            state: Mutex::default(),
        }
    }
    pub fn reset_session(&self, keys: Keys, logger: &Logger) {
        if let Ok(mut s) = self.state.lock() {
            let lane = s.selection.lane;
            let navigation = s.pending_navigation.take().or_else(|| s.navigation.take());
            *s = State::default();
            s.navigation = navigation;
            s.selection.lane = lane;
            s.previous_choice = keys.selection;
            s.previous_left = keys.left;
            s.previous_right = keys.right || keys.attack_click;
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
        if !battlefield {
            state.target = None;
            state.route.clear();
            state.marker = None;
            state.attack_move_armed = false;
            state.pending_recall = None;
            state.hover_units.clear();
            state.hover_updated = None;
            state.hover_actor = None;
        }
        state.updated = Some(Instant::now());
    }
    pub fn begin_match(&self, key: MatchKey, logger: &Logger) {
        if !self.enabled {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            if state.selection.begin(key) {
                if let Some(grid) = state.pending_navigation.take() {
                    state.navigation = Some(grid);
                }
                state.target = None;
                state.route.clear();
                state.marker = None;
                state.champion_position = None;
                state.updated = None;
                state.units.clear();
                state.units_updated = None;
                state.hover_units.clear();
                state.hover_updated = None;
                state.hover_actor = None;
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
        s.route.clear();
        s.marker = None;
        s.champion_position = None;
        s.units.clear();
        s.units_updated = None;
        s.hover_units.clear();
        s.hover_updated = None;
        s.hover_actor = None;
        s.attack_move_armed = false;
        s.pending_recall = None;
        s.cancel_recall = false;
        s.native_action = None;
        s.attack_range = None;
        s.hover_keys = None;
        s.frame_markers = (None, None);
        s.attack_click = None;
        s.frame_click = None;
        s.notice.clear();
        s.previous_left = keys.left;
        s.previous_right = keys.right || keys.attack_click;
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
        // Accepted world orders outlive the foreground input heartbeat.
        // Focus loss stops collecting input; it does not mean a Stop command.
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
            state.route.clear();
            state.marker = None;
        }
        if let Some(target) = state.target {
            if let Order::Move(goal) = target {
                while state.route.front().is_some_and(|p| {
                    position.0.abs_diff(p.0).pow(2) + position.1.abs_diff(p.1).pow(2) <= 4_000_000
                }) {
                    state.route.pop_front();
                }
                if let Some(waypoint) = state.route.front().copied() {
                    return Some((InputV1::move_to(waypoint.0, waypoint.1), None));
                }
                if position.0.abs_diff(goal.0).pow(2) + position.1.abs_diff(goal.1).pow(2)
                    <= 4_000_000
                {
                    state.target = None;
                    state.route.clear();
                    state.marker = None;
                }
            }
            let result = target.resolve_filtered(position, &state.units, state.champion_only);
            if matches!(target, Order::Move(_))
                && result.0 == InputV1::move_to(position.0, position.1)
            {
                state.target = None;
                state.route.clear();
            }
            return Some(result);
        }
        Some((InputV1::move_to(position.0, position.1), None))
    }
    /// A manual ground Move or S stop was issued after `since` and is still the
    /// player's latest intent (no later attack, A-click or recall order).
    pub fn manual_release_after(&self, since: Instant) -> bool {
        self.state.lock().is_ok_and(|s| {
            s.manual_release.is_some_and(|at| at > since)
                && s.pending_recall.is_none()
                && !matches!(s.target, Some(Order::Attack(_) | Order::AttackMove(_)))
        })
    }
    pub fn observe_position(&self, position: Option<(u64, u64)>) {
        if let Ok(mut s) = self.state.lock() {
            s.champion_position = position;
            if position.is_none() {
                s.target = None;
                s.route.clear();
                s.marker = None;
                s.units.clear();
                s.hover_units.clear();
                s.hover_updated = None;
                s.hover_actor = None;
                s.attack_move_armed = false;
                s.pending_recall = None;
                s.cancel_recall = false;
                s.native_action = None;
                s.attack_range = None;
            }
        }
    }
    pub fn observe_hover_units(&self, key: MatchKey, player: usize, actor: usize, units: &[Unit]) {
        if !self.enabled {
            return;
        }
        if let Ok(mut s) = self.state.lock() {
            if s.selection.match_key != Some(key)
                || s.selection.selected().is_none_or(|p| p.player != player)
                || s.champion_position.is_none()
            {
                return;
            }
            s.hover_actor = Some(actor);
            s.hover_units = units.to_vec();
            s.hover_updated = Some(Instant::now());
        }
    }
    pub fn camera_target(&self) -> Option<(PlayerIdentity, Option<(u64, u64)>)> {
        let s = self.state.lock().ok()?;
        Some((s.selection.selected()?.clone(), s.champion_position))
    }
    pub fn set_navigation(&self, grid: Option<crate::map_path::Grid>) {
        if let Ok(mut s) = self.state.lock() {
            // Map callbacks also run for background simulations. Once the live
            // match is bound, pin its geometry and retain accepted waypoints.
            // Stage new geometry for the next session instead.
            if let Some(grid) = grid {
                if s.selection.match_key.is_some() {
                    s.pending_navigation = Some(grid);
                } else {
                    s.navigation = Some(grid);
                }
            }
        }
    }
    pub fn direct_segment(
        &self,
        key: MatchKey,
        actor: usize,
        from: (u64, u64),
        goal: (u64, u64),
        log: &Logger,
    ) -> bool {
        let Ok(mut s) = self.state.lock() else {
            return false;
        };
        if s.selection.match_key != Some(key) || s.hover_actor != Some(actor) {
            return false;
        }
        let clear = [from.0, from.1, goal.0, goal.1]
            .into_iter()
            .all(|v| v < 960_000)
            && s.navigation.as_ref().is_some_and(|g| g.clear(from, goal));
        if s.steering_trace != Some((goal, clear)) {
            log.write(&format!("MOVEMENT STEERING actor={actor} from={from:?} goal={goal:?} direct={clear}; native speed retained"));
            s.steering_trace = Some((goal, clear));
        }
        clear
    }
    #[allow(clippy::too_many_arguments)]
    pub fn trace_native_move(
        &self,
        actor: usize,
        position: (u64, u64),
        action: usize,
        requested: (u64, u64),
        native_goal: (u64, u64),
        repeated: bool,
        log: &Logger,
    ) {
        if let Ok(mut s) = self.state.lock() {
            if s.trace_at
                .is_some_and(|at| at.elapsed() < Duration::from_millis(200))
            {
                return;
            }
            s.trace_count += 1;
            s.trace_at = Some(Instant::now());
            log.write(&format!("MOVEMENT TRACE actor={actor} position={position:?} action={action} order={:?} requested={requested:?} native_goal={native_goal:?} repeated_suppressed={repeated} remaining_waypoints={}",s.target,s.route.len()));
        }
    }
    pub fn draw_minimap_path(
        &self,
        ctx: &mut StableClient<'_>,
        camera: &crate::camera::CameraControl,
    ) {
        let Some(frame) = camera.frame() else { return };
        let Ok(s) = self.state.lock() else { return };
        let Some(mut from) = s.champion_position else {
            return;
        };
        let Some(Order::Move(goal)) = s.target else {
            return;
        };
        for to in s.route.iter().copied() {
            let a = frame.project_minimap(from);
            let b = frame.project_minimap(to);
            ctx.draw_line("UI", a.0, a.1, b.0, b.1, 2., 1100, 0xffd700ff);
            from = to;
        }
        let p = frame.project_minimap(goal);
        ctx.draw_circle("UI", p.0, p.1, 3., 1101, 0xffd700ff);
    }
    pub fn attack_move_armed(&self) -> bool {
        self.state.lock().is_ok_and(|s| s.attack_move_armed)
    }
    pub fn cursor_enemy(&self) -> Option<Unit> {
        self.state
            .lock()
            .ok()?
            .frame_markers
            .0
            .filter(|u| !u.friendly)
    }
    pub fn click_feedback(&self) -> Vec<ClickFeedback> {
        let Ok(mut s) = self.state.lock() else {
            return Vec::new();
        };
        s.clicks
            .retain(|c| c.at.elapsed() < Duration::from_millis(250));
        if !s.hover_keys.is_some_and(|k| k.focused) {
            return Vec::new();
        }
        s.clicks.iter().copied().collect()
    }
    pub fn take_command_stamp(&self) -> Option<crate::input_trace::Stamp> {
        self.state.lock().ok()?.command_stamp.take()
    }
    pub fn clear_commands(&self) {
        if let Ok(mut s) = self.state.lock() {
            s.target = None;
            s.route.clear();
            s.marker = None;
            s.attack_move_armed = false;
            s.pending_recall = None;
            s.command_stamp = None;
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
        let enemies_fresh = s
            .units_updated
            .is_some_and(|t| t.elapsed() <= Duration::from_millis(250));
        // A present but empty/expired all-unit snapshot is authoritative. Never
        // replace it with enemy-only data, or let enemy freshness hide allies.
        let hover_units = match s.hover_updated {
            Some(t) if t.elapsed() <= Duration::from_millis(250) => &s.hover_units[..],
            None if enemies_fresh => &s.units[..],
            _ => &[],
        };
        let hover = keys
            .cursor
            .filter(|p| !camera.command_blocked(*p))
            .and_then(|p| {
                crate::combat::clicked_units(frame, p, hover_units, keys.champion_only)
                    .into_iter()
                    .find(|id| Some(*id) != s.hover_actor)
            })
            .and_then(|id| hover_units.iter().find(|u| u.id == id))
            .copied();
        let attack = s
            .target
            .filter(|_| enemies_fresh)
            .map(|order| {
                order
                    .resolve_filtered(position, &s.units, s.champion_only)
                    .0
            })
            .filter(|input| input.kind == mod_api_stable::InputKindV1::Attack.code())
            // The all-unit snapshot is authoritative for visibility/death,
            // including the interval before combat_input refreshes enemies.
            .and_then(|input| {
                hover_units
                    .iter()
                    .find(|u| u.id == input.target.target_id && !u.friendly)
            })
            .copied();
        (hover, attack)
    }
    /// Capture once after input and UI masks. Native drawing, ground feedback
    /// and cursor state consume this same frame's winner, even if the worker
    /// publishes a newer unit snapshot before post_render.
    pub fn refresh_hover(
        &self,
        camera: &crate::camera::CameraControl,
        active: bool,
        skill_target: Option<Option<Unit>>,
    ) -> (Option<Unit>, Option<Unit>) {
        let mut markers = if active {
            self.target_markers(camera)
        } else {
            (None, None)
        };
        if active {
            if let Some(target) = skill_target {
                markers.0 = target.filter(|u| {
                    self.state.lock().is_ok_and(|s| {
                        s.champion_position.is_some()
                            && Some(u.id) != s.hover_actor
                            && s.hover_keys
                                .filter(|k| k.focused)
                                .and_then(|k| k.cursor)
                                .is_some_and(|p| !camera.command_blocked(p))
                    })
                });
            }
        }
        if let Ok(mut s) = self.state.lock() {
            s.frame_markers = markers;
            // Keep the original click time: simulation ticks and held buttons
            // must not restart the pulse. Death, fog, replacement orders and
            // inactive frames discard it rather than replaying it later.
            s.attack_click = s.attack_click.filter(|(id, at)| {
                markers.1.is_some_and(|u| u.id == *id) && at.elapsed() < ATTACK_CLICK_DURATION
            });
            s.frame_click = s.attack_click;
        }
        markers
    }
    pub fn attack_click_feedback(&self) -> Option<(usize, Instant)> {
        self.state.lock().ok().and_then(|s| s.frame_click)
    }
    pub fn draw_targets(
        &self,
        ctx: &mut StableClient<'_>,
        camera: &crate::camera::CameraControl,
        selection_debug: bool,
    ) {
        if !selection_debug {
            return;
        }
        let Some(frame) = camera.frame() else { return };
        let Ok(s) = self.state.lock() else { return };
        let (hover, attack) = s.frame_markers;
        drop(s);
        // Both hovered and selected attack-target markers are debug feedback.
        // Picking and the actual attack order remain independent of drawing.
        for (unit, color, extra) in [
            (
                hover,
                hover.map(crate::combat::hover_color).unwrap_or(0),
                3.,
            ),
            (attack, 0xff7858ff, 7.),
        ] {
            let Some(unit) = unit else { continue };
            let Some(center) = frame.project(unit.position) else {
                continue;
            };
            if unit.is_tower {
                let (half, height, bottom, margin) =
                    crate::combat::selection_envelope(frame, &unit);
                let pad = margin + extra * 0.3;
                let left = center.0 - half - pad;
                let right = center.0 + half + pad;
                let top = center.1 - height - pad;
                let bottom = center.1 + bottom + pad;
                let length = ((right - left) * 0.25).clamp(6., 18.);
                for (x, y, dx, dy) in [
                    (left, top, 1., 1.),
                    (right, top, -1., 1.),
                    (left, bottom, 1., -1.),
                    (right, bottom, -1., -1.),
                ] {
                    for end in [(x + dx * length, y), (x, y + dy * length)] {
                        if !camera.blocked((x, y)) && !camera.blocked(end) {
                            crate::abilities::draw_segment(ctx, frame, (x, y), end, color);
                        }
                    }
                }
                continue;
            }
            let (rx, ry) = crate::combat::highlight_radii(frame, &unit);
            let (rx, ry) = (rx + extra, ry + extra * 0.55);
            for i in 0..48 {
                let point = |j: usize| {
                    let theta = j as f32 * std::f32::consts::TAU / 48.;
                    (center.0 + theta.cos() * rx, center.1 + theta.sin() * ry)
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
        s.champion_only = keys.champion_only;
        let focus_returned = keys.focused && !s.mouse_focused;
        let right_down = keys.right || keys.attack_click;
        let click = right_down && !s.previous_right && !focus_returned;
        let stop = keys.stop && !s.previous_stop && !focus_returned;
        let left_click = keys.left && !s.previous_left && !focus_returned && !left_reserved;
        let arm = keys.attack_move && !s.previous_attack_move && !focus_returned;
        let recall = keys.recall && !s.previous_recall && !focus_returned;
        s.mouse_focused = keys.focused;
        s.previous_right = keys.right || keys.attack_click;
        s.previous_stop = keys.stop;
        s.previous_left = keys.left;
        s.previous_attack_move = keys.attack_move;
        s.previous_recall = keys.recall;
        if !active {
            s.clicks.clear();
            s.target = None;
            s.route.clear();
            s.marker = None;
            s.attack_move_armed = false;
            s.pending_recall = None;
            s.command_stamp = None;
            s.cancel_recall = false;
            s.manual_release = None;
            s.hover_units.clear();
            s.hover_updated = None;
            s.hover_actor = None;
            return;
        }
        if !keys.focused {
            s.attack_move_armed = false;
            s.pending_recall = None;
            return;
        }
        if stop || keys.escape {
            s.target = None;
            s.route.clear();
            s.marker = None;
            s.attack_move_armed = false;
            s.pending_recall = None;
            if stop || keys.escape {
                s.cancel_recall = true;
            }
            if stop {
                s.manual_release = Some(Instant::now());
                s.command_stamp = Some(crate::input_trace::Stamp::new("S"));
                log.write("MANUAL STOP S; cancel selected champion movement");
            }
        } else {
            if recall && !click && !arm {
                s.target = None;
                s.route.clear();
                s.marker = None;
                s.attack_move_armed = false;
                s.cancel_recall = false;
                s.manual_release = None;
                s.pending_recall = Some(Instant::now());
                s.command_stamp = Some(crate::input_trace::Stamp::new("B"));
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
            let attack_move = (s.attack_move_armed && left_click) || (click && keys.attack_click);
            if !click && !attack_move {
                return;
            }
            if click {
                s.attack_move_armed = false;
                s.pending_recall = None;
            }
            let frame = camera.frame();
            let minimap_target =
                frame.and_then(|f| keys.cursor.and_then(|p| f.unproject_minimap(p)));
            let target = minimap_target.or_else(|| {
                keys.cursor
                    .filter(|p| !camera.command_blocked(*p))
                    .and_then(|p| frame.and_then(|f| f.unproject(p)))
            });
            if let Some(target) = target {
                let hit = minimap_target
                    .is_none()
                    .then(|| {
                        s.units_updated
                            .filter(|t| t.elapsed() <= Duration::from_millis(250))
                            .and_then(|_| {
                                crate::combat::clicked_unit(
                                    frame?,
                                    keys.cursor?,
                                    &s.units,
                                    keys.champion_only,
                                )
                            })
                    })
                    .flatten();
                let order = if let Some(id) = hit.filter(|_| !keys.attack_click) {
                    Order::Attack(id)
                } else if attack_move {
                    Order::AttackMove(target)
                } else {
                    Order::Move(target)
                };
                s.target = Some(order);
                s.attack_click = match order {
                    Order::Attack(id) => Some((id, Instant::now())),
                    _ => None,
                };
                s.command_stamp = Some(crate::input_trace::Stamp::new(if attack_move {
                    "A-click"
                } else {
                    "right-click"
                }));
                s.route.clear();
                if matches!(order, Order::Move(_)) {
                    if let (Some(grid), Some(start)) = (&s.navigation, s.champion_position) {
                        if let Some(route) = grid.route(start, target) {
                            if let Some(end) = route.last().copied() {
                                s.target = Some(Order::Move(end));
                            }
                            s.route = route.into();
                        } else {
                            s.target = None;
                            s.route.clear();
                            log.write("MANUAL MOVE no traversable map route; command ignored");
                        }
                    }
                }
                s.cancel_recall = true;
                s.attack_move_armed = false;
                s.manual_release = matches!(s.target, Some(Order::Move(_))).then(Instant::now);
                s.marker = s.target.map(|order| match order {
                    Order::Move(p) => p,
                    _ => target,
                });
                if let Some(position) = s.marker {
                    s.clicks.clear();
                    s.clicks.push_back(ClickFeedback {
                        position,
                        attack: !matches!(order, Order::Move(_)),
                        minimap: minimap_target.is_some(),
                        at: Instant::now(),
                    });
                }
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
    use super::*;
    use crate::test_support::logger;
    #[test]
    fn attack_move_click_keeps_ground_order_and_held_click_does_not_repeat() {
        let m = movement();
        let log = logger("shift-rmb-order");
        start(&m, &log, 0);
        let camera = recall_camera();
        m.update_mouse(keys(0, 0), true, &camera, &log);
        m.combat_input(5, (200_000, 200_000), vec![]);
        let click = Keys {
            attack_click: true,
            cursor: Some((1160., 537.)),
            ..keys(0, 0)
        };
        m.update_mouse(click, true, &camera, &log);
        assert!(matches!(
            m.state.lock().unwrap().target,
            Some(Order::AttackMove(_))
        ));
        assert_eq!(m.click_feedback().len(), 1);
        let stamp = m.take_command_stamp().unwrap().id;
        m.update_mouse(click, true, &camera, &log);
        assert!(m.take_command_stamp().is_none());
        assert!(stamp > 0);
    }
    #[test]
    fn rapid_accepted_clicks_replace_marker_without_replacing_it_on_a_hud_miss() {
        let m = movement();
        let log = logger("latest-click-marker");
        start(&m, &log, 0);
        let camera = recall_camera();
        m.update_mouse(keys(0, 0), true, &camera, &log);
        m.combat_input(5, (200_000, 200_000), vec![]);
        for point in [(1160., 537.), (1200., 577.), (1240., 617.)] {
            let hover = Keys {
                cursor: Some(point),
                ..keys(0, 0)
            };
            m.update_mouse(hover, true, &camera, &log);
            m.update_mouse(
                Keys {
                    right: true,
                    ..hover
                },
                true,
                &camera,
                &log,
            );
            let feedback = m.click_feedback();
            assert_eq!(feedback.len(), 1);
            assert_eq!(
                feedback[0].position,
                camera.frame().unwrap().unproject(point).unwrap()
            );
        }
        let last = m.click_feedback()[0].position;
        let blocked = Keys {
            cursor: Some((100., 100.)),
            ..keys(0, 0)
        };
        camera.set_command_blocked(vec![crate::camera::Rect {
            x: 80.,
            y: 80.,
            w: 40.,
            h: 40.,
        }]);
        m.update_mouse(blocked, true, &camera, &log);
        m.update_mouse(
            Keys {
                right: true,
                ..blocked
            },
            true,
            &camera,
            &log,
        );
        assert_eq!(m.click_feedback()[0].position, last);
        assert_eq!(m.state.lock().unwrap().target, Some(Order::Move(last)));
    }
    #[test]
    fn only_newer_manual_ground_move_or_stop_releases_an_attack() {
        let m = movement();
        let log = logger("manual-release");
        start(&m, &log, 0);
        let camera = recall_camera();
        m.update_mouse(keys(0, 0), true, &camera, &log);
        m.combat_input(5, (200_000, 200_000), vec![]);
        let before = Instant::now();
        std::thread::sleep(Duration::from_millis(2));
        assert!(!m.manual_release_after(before));
        let hover = Keys {
            cursor: Some((1160., 537.)),
            ..keys(0, 0)
        };
        m.update_mouse(hover, true, &camera, &log);
        m.update_mouse(
            Keys {
                right: true,
                ..hover
            },
            true,
            &camera,
            &log,
        );
        assert!(m.manual_release_after(before));
        // An attack that starts after the click is not released by it.
        std::thread::sleep(Duration::from_millis(2));
        assert!(!m.manual_release_after(Instant::now()));
        // A later attack order supersedes the earlier move.
        m.state.lock().unwrap().target = Some(Order::Attack(9));
        assert!(!m.manual_release_after(before));
        m.state.lock().unwrap().target = None;
        let later = Instant::now();
        std::thread::sleep(Duration::from_millis(2));
        m.update_mouse(keys(0, 0), true, &camera, &log);
        m.update_mouse(
            Keys {
                stop: true,
                ..keys(0, 0)
            },
            true,
            &camera,
            &log,
        );
        assert!(m.manual_release_after(later));
        // Automatic hold/chase inputs never create manual intent.
        let idle = Instant::now();
        std::thread::sleep(Duration::from_millis(2));
        m.combat_input(5, (200_000, 200_000), vec![]);
        assert!(!m.manual_release_after(idle));
    }
    #[test]
    fn pointer_feedback_shares_enemy_picker_and_click_markers_expire_without_clearing_orders() {
        let m = movement();
        let log = logger("cursor-picker");
        start(&m, &log, 0);
        let camera = recall_camera();
        let hover = Keys {
            cursor: Some((1160., 537.)),
            ..keys(0, 0)
        };
        m.update_mouse(hover, true, &camera, &log);
        let minion = Unit {
            id: 29,
            friendly: false,
            is_champion: false,
            is_minion: true,
            ..hover_ally()
        };
        m.combat_input(5, (200_000, 200_000), vec![minion]);
        m.refresh_hover(&camera, true, None);
        assert_eq!(m.cursor_enemy().unwrap().id, 29);
        m.update_mouse(
            Keys {
                champion_only: true,
                ..hover
            },
            true,
            &camera,
            &log,
        );
        m.refresh_hover(&camera, true, None);
        assert!(m.cursor_enemy().is_none());
        let champion = Unit {
            is_champion: true,
            is_minion: false,
            ..minion
        };
        m.combat_input(5, (200_000, 200_000), vec![champion]);
        m.refresh_hover(&camera, true, None);
        assert_eq!(m.cursor_enemy().unwrap().id, 29);
        m.update_mouse(
            Keys {
                right: true,
                champion_only: true,
                ..hover
            },
            true,
            &camera,
            &log,
        );
        assert!(m.click_feedback()[0].attack);
        {
            let mut s = m.state.lock().unwrap();
            s.clicks[0].at = Instant::now() - Duration::from_millis(251);
        }
        assert!(m.click_feedback().is_empty());
        assert_eq!(m.state.lock().unwrap().target, Some(Order::Attack(29)));
        camera.set_command_blocked(vec![crate::camera::Rect {
            x: 1140.,
            y: 520.,
            w: 40.,
            h: 40.,
        }]);
        m.refresh_hover(&camera, true, None);
        assert!(m.cursor_enemy().is_none());
    }
    #[test]
    fn direct_steering_requires_current_actor_clear_map_and_in_bounds_goal() {
        let m = movement();
        let log = logger("direct-steering");
        start(&m, &log, 0);
        m.observe_position(Some((200_000, 200_000)));
        m.observe_hover_units((1, 33, 1), 5, 100, &[]);
        let from = (200_000, 200_000);
        let goal = (250_000, 220_000);
        assert!(!m.direct_segment((1, 33, 1), 100, from, goal, &log));
        let mut grid = vec![vec![0; 30]; 30];
        m.state.lock().unwrap().navigation =
            crate::map_path::Grid::from_json(&serde_json::to_string(&grid).unwrap());
        assert!(m.direct_segment((1, 33, 1), 100, from, goal, &log));
        assert!(!m.direct_segment((2, 34, 1), 100, from, goal, &log));
        assert!(!m.direct_segment((1, 33, 1), 101, from, goal, &log));
        assert!(!m.direct_segment((1, 33, 1), 100, from, (960_000, 220_000), &log));
        // Replace only geometry in this isolated test; normal live callbacks retain it.
        grid[6][7] = 1;
        m.state.lock().unwrap().navigation =
            crate::map_path::Grid::from_json(&serde_json::to_string(&grid).unwrap());
        assert!(!m.direct_segment((1, 33, 1), 100, from, goal, &log));
        m.observe_position(None);
        assert!(!m.direct_segment((1, 33, 1), 100, from, goal, &log));
    }
    fn hover_ally() -> Unit {
        Unit {
            id: 91,
            position: (300_000, 200_000),
            radius: 10_000,
            is_champion: true,
            is_minion: false,
            friendly: true,
            in_cc: false,
            is_tower: false,
            body: None,
        }
    }
    #[test]
    fn frame_hover_has_one_hostile_winner_and_survives_worker_updates_until_refresh() {
        let m = movement();
        let log = logger("shared-hover-55-2");
        start(&m, &log, 0);
        let camera = recall_camera();
        let keys = Keys {
            cursor: Some((1160., 537.)),
            ..keys(0, 0)
        };
        m.update_mouse(keys, true, &camera, &log);
        m.observe_position(Some((200_000, 200_000)));
        let ally = hover_ally();
        let enemy = Unit {
            id: 29,
            friendly: false,
            is_champion: false,
            is_minion: true,
            ..ally
        };
        m.combat_input(5, (200_000, 200_000), vec![enemy]);
        m.observe_hover_units((1, 33, 1), 5, 100, &[ally, enemy]);
        assert_eq!(m.refresh_hover(&camera, true, None).0.unwrap().id, enemy.id);
        assert_eq!(m.cursor_enemy().unwrap().id, enemy.id);
        // An intervening native snapshot cannot change this frame's outline,
        // ground marker or hostile cursor independently.
        m.observe_hover_units((1, 33, 1), 5, 100, &[ally]);
        assert_eq!(
            m.state.lock().unwrap().frame_markers.0.unwrap().id,
            enemy.id
        );
        assert_eq!(m.cursor_enemy().unwrap().id, enemy.id);
        assert_eq!(m.refresh_hover(&camera, true, None).0.unwrap().id, ally.id);
        assert!(m.cursor_enemy().is_none());
        m.observe_hover_units((1, 33, 1), 5, 100, &[]);
        assert!(m.refresh_hover(&camera, true, None).0.is_none());
        assert!(m.cursor_enemy().is_none()); // Never substitutes enemy-only data.
        m.observe_hover_units((1, 33, 1), 5, 100, &[ally, enemy]);
        assert_eq!(
            m.refresh_hover(&camera, true, Some(Some(ally)))
                .0
                .unwrap()
                .id,
            ally.id
        );
        assert!(m.cursor_enemy().is_none()); // Ally-target skill eligibility.
        assert!(m.refresh_hover(&camera, true, Some(None)).0.is_none());
        assert!(m
            .refresh_hover(&camera, true, Some(Some(Unit { id: 100, ..ally })))
            .0
            .is_none());
        assert!(m.refresh_hover(&camera, false, None).0.is_none());
        assert!(m.state.lock().unwrap().frame_markers.0.is_none());
        camera.set_command_blocked(vec![crate::camera::Rect {
            x: 1140.,
            y: 520.,
            w: 40.,
            h: 40.,
        }]);
        assert!(m.refresh_hover(&camera, true, Some(Some(ally))).0.is_none());
    }
    #[test]
    fn attack_feedback_persists_off_hover_but_clears_on_visibility_loss_and_stop() {
        let m = movement();
        let log = logger("attack-outline-57");
        start(&m, &log, 0);
        let camera = recall_camera();
        let pointed = Unit {
            id: 29,
            friendly: false,
            ..hover_ally()
        };
        let target = Unit {
            id: 30,
            position: (500_000, 500_000),
            ..pointed
        };
        let hover = Keys {
            cursor: Some((1160., 537.)),
            ..keys(0, 0)
        };
        m.update_mouse(hover, true, &camera, &log);
        m.combat_input(5, (200_000, 200_000), vec![pointed, target]);
        m.observe_hover_units((1, 33, 1), 5, 100, &[pointed, target]);
        m.state.lock().unwrap().target = Some(Order::Attack(target.id));
        let (h, a) = m.refresh_hover(&camera, true, None);
        assert_eq!(h.unwrap().id, pointed.id);
        assert_eq!(a.unwrap().id, target.id);
        assert_eq!(m.cursor_enemy().unwrap().id, pointed.id);
        m.update_mouse(
            Keys {
                cursor: None,
                ..hover
            },
            true,
            &camera,
            &log,
        );
        let (h, a) = m.refresh_hover(&camera, true, None);
        assert!(h.is_none() && m.cursor_enemy().is_none());
        assert_eq!(a.unwrap().id, target.id);
        camera.set_command_blocked(vec![crate::camera::Rect {
            x: 1140.,
            y: 520.,
            w: 40.,
            h: 40.,
        }]);
        m.update_mouse(hover, true, &camera, &log);
        let (h, a) = m.refresh_hover(&camera, true, None);
        assert!(h.is_none() && m.cursor_enemy().is_none());
        assert_eq!(a.unwrap().id, target.id);
        camera.set_command_blocked(Vec::new());
        // The new visibility snapshot must beat the still-fresh enemy list.
        m.observe_hover_units((1, 33, 1), 5, 100, &[pointed]);
        assert!(m.refresh_hover(&camera, true, None).1.is_none());
        m.combat_input(5, (200_000, 200_000), vec![pointed]);
        assert!(m.state.lock().unwrap().target.is_none());
        m.observe_hover_units((1, 33, 1), 5, 100, &[pointed, target]);
        m.combat_input(5, (200_000, 200_000), vec![pointed, target]);
        m.state.lock().unwrap().target = Some(Order::Attack(pointed.id));
        assert_eq!(
            m.refresh_hover(&camera, true, None).1.unwrap().id,
            pointed.id
        );
        m.update_mouse(
            Keys {
                stop: true,
                ..hover
            },
            true,
            &camera,
            &log,
        );
        assert!(m.refresh_hover(&camera, true, None).1.is_none());
    }
    #[test]
    fn accepted_attack_click_pulses_once_and_repeated_click_restarts_it() {
        let m = movement();
        let log = logger("attack-click-pulse-58");
        start(&m, &log, 0);
        let camera = recall_camera();
        let hover = Keys {
            cursor: Some((1160., 537.)),
            ..keys(0, 0)
        };
        let enemy = Unit {
            id: 29,
            friendly: false,
            ..hover_ally()
        };
        m.update_mouse(hover, true, &camera, &log);
        m.combat_input(5, (200_000, 200_000), vec![enemy]);
        m.observe_hover_units((1, 33, 1), 5, 100, &[enemy]);
        let click = Keys {
            right: true,
            ..hover
        };
        m.update_mouse(click, true, &camera, &log);
        m.refresh_hover(&camera, true, None);
        let first = m.attack_click_feedback().unwrap();
        assert_eq!(first.0, enemy.id);
        // Held right click and worker attack resolution never restart it.
        m.update_mouse(click, true, &camera, &log);
        m.combat_input(5, (200_000, 200_000), vec![enemy]);
        m.refresh_hover(&camera, true, None);
        assert_eq!(m.attack_click_feedback(), Some(first));
        // A fresh explicit click on the same target must get a fresh timestamp.
        let earlier = Instant::now() - Duration::from_millis(30);
        m.state.lock().unwrap().attack_click = Some((enemy.id, earlier));
        m.update_mouse(hover, true, &camera, &log);
        m.update_mouse(click, true, &camera, &log);
        m.refresh_hover(&camera, true, None);
        assert!(m.attack_click_feedback().unwrap().1 > earlier);
        // Blocked UI clicks don't trigger or replace accepted feedback.
        let accepted = m.attack_click_feedback();
        camera.set_command_blocked(vec![crate::camera::Rect {
            x: 1140.,
            y: 520.,
            w: 40.,
            h: 40.,
        }]);
        m.update_mouse(hover, true, &camera, &log);
        m.update_mouse(click, true, &camera, &log);
        m.refresh_hover(&camera, true, None);
        assert_eq!(m.attack_click_feedback(), accepted);
        // Expiration clears only the pulse; the attack and its outline remain.
        m.state.lock().unwrap().attack_click =
            Some((enemy.id, Instant::now() - ATTACK_CLICK_DURATION));
        assert_eq!(m.refresh_hover(&camera, true, None).1.unwrap().id, enemy.id);
        assert!(m.attack_click_feedback().is_none());
    }
    #[test]
    fn attack_click_pulse_clears_on_fog_stop_inactive_death_and_session_reset() {
        let m = movement();
        let log = logger("attack-click-clear-58");
        start(&m, &log, 0);
        let camera = recall_camera();
        let enemy = Unit {
            id: 29,
            friendly: false,
            ..hover_ally()
        };
        let hover = Keys {
            cursor: Some((1160., 537.)),
            ..keys(0, 0)
        };
        for reason in 0..5 {
            m.update_mouse(hover, true, &camera, &log);
            m.combat_input(5, (200_000, 200_000), vec![enemy]);
            m.observe_hover_units((1, 33, 1), 5, 100, &[enemy]);
            m.update_mouse(
                Keys {
                    right: true,
                    ..hover
                },
                true,
                &camera,
                &log,
            );
            m.refresh_hover(&camera, true, None);
            assert!(m.attack_click_feedback().is_some());
            match reason {
                0 => m.observe_hover_units((1, 33, 1), 5, 100, &[]),
                1 => m.update_mouse(
                    Keys {
                        stop: true,
                        ..hover
                    },
                    true,
                    &camera,
                    &log,
                ),
                2 => {
                    m.refresh_hover(&camera, false, None);
                }
                3 => m.observe_position(None),
                _ => m.reset_session(hover, &log),
            }
            m.refresh_hover(&camera, true, None);
            assert!(m.attack_click_feedback().is_none());
        }
    }
    #[test]
    fn attack_move_auto_acquisition_does_not_create_attack_click_pulses() {
        let m = movement();
        let log = logger("attack-click-acquisition-58");
        start(&m, &log, 0);
        let camera = recall_camera();
        let enemy = Unit {
            id: 29,
            friendly: false,
            position: (230_000, 200_000),
            ..hover_ally()
        };
        m.update_mouse(keys(0, 0), true, &camera, &log);
        m.combat_input(5, (200_000, 200_000), vec![enemy]);
        m.observe_hover_units((1, 33, 1), 5, 100, &[enemy]);
        m.update_mouse(
            Keys {
                attack_click: true,
                cursor: Some((1160., 537.)),
                ..keys(0, 0)
            },
            true,
            &camera,
            &log,
        );
        assert_eq!(m.refresh_hover(&camera, true, None).1.unwrap().id, enemy.id);
        assert!(m.attack_click_feedback().is_none());
    }
    #[test]
    fn attack_move_feedback_follows_acquisition_and_expires_with_state() {
        let m = movement();
        let log = logger("attack-move-outline-57");
        start(&m, &log, 0);
        let camera = recall_camera();
        m.update_mouse(keys(0, 0), true, &camera, &log);
        let first = Unit {
            id: 29,
            friendly: false,
            position: (230_000, 200_000),
            ..hover_ally()
        };
        let second = Unit {
            id: 30,
            position: (300_000, 200_000),
            ..first
        };
        m.combat_input(5, (200_000, 200_000), vec![first, second]);
        m.observe_hover_units((1, 33, 1), 5, 100, &[first, second]);
        m.state.lock().unwrap().target = Some(Order::AttackMove((400_000, 200_000)));
        assert_eq!(m.refresh_hover(&camera, true, None).1.unwrap().id, first.id);
        m.combat_input(5, (300_000, 200_000), vec![second]);
        m.observe_hover_units((1, 33, 1), 5, 100, &[second]);
        assert_eq!(
            m.refresh_hover(&camera, true, None).1.unwrap().id,
            second.id
        );
        // No enemies acquired: ground attack-move has no target outline.
        m.combat_input(5, (500_000, 200_000), vec![second]);
        assert!(m.refresh_hover(&camera, true, None).1.is_none());
        m.combat_input(5, (300_000, 200_000), vec![second]);
        m.state.lock().unwrap().units_updated = Some(Instant::now() - Duration::from_secs(1));
        assert!(m.refresh_hover(&camera, true, None).1.is_none());
        m.combat_input(5, (300_000, 200_000), vec![second]);
        assert!(m.refresh_hover(&camera, false, None).1.is_none());
        assert!(m.state.lock().unwrap().frame_markers.1.is_none());
        m.state.lock().unwrap().target = Some(Order::Move((600_000, 200_000)));
        assert!(m.refresh_hover(&camera, true, None).1.is_none());
        m.observe_position(None);
        assert!(m.refresh_hover(&camera, true, None).1.is_none());
    }
    #[test]
    fn passive_hover_skips_self_and_still_finds_overlapping_allies() {
        let m = movement();
        let log = logger("self-hover");
        start(&m, &log, 0);
        let camera = recall_camera();
        m.update_mouse(
            Keys {
                cursor: Some((1160., 537.)),
                ..keys(0, 0)
            },
            true,
            &camera,
            &log,
        );
        m.observe_position(Some((300_000, 200_000)));
        let own = Unit {
            id: 100,
            ..hover_ally()
        };
        m.observe_hover_units((1, 33, 1), 5, own.id, &[own]);
        assert!(m.target_markers(&camera).0.is_none());
        // The all-unit data still contains self for ability targeting. Only
        // passive feedback filters it, using ID rather than shared positions.
        {
            let s = m.state.lock().unwrap();
            assert_eq!(s.hover_units.len(), 1);
            assert_eq!(s.hover_units[0].id, own.id);
        }
        m.observe_hover_units((1, 33, 1), 5, own.id, &[own, hover_ally()]);
        assert_eq!(m.target_markers(&camera).0.unwrap().id, 91);
        m.observe_position(None);
        assert!(m.state.lock().unwrap().hover_actor.is_none());
        assert!(m.target_markers(&camera).0.is_none());
    }
    #[test]
    fn ally_snapshot_survives_other_players_and_matches_and_has_its_own_freshness() {
        let m = movement();
        let log = logger("hover-owner");
        start(&m, &log, 0);
        let camera = recall_camera();
        let hover = Keys {
            cursor: Some((1160., 537.)),
            ..keys(0, 0)
        };
        m.update_mouse(hover, true, &camera, &log);
        m.combat_input(5, (200_000, 200_000), Vec::new());
        m.observe_hover_units((1, 33, 1), 5, 100, &[hover_ally()]);
        for player in 0..10 {
            if player != 5 {
                m.observe_hover_units((1, 33, 1), player, 100, &[]);
                assert_eq!(m.target_markers(&camera).0.unwrap().id, 91);
            }
        }
        m.observe_hover_units((2, 34, 1), 5, 100, &[]);
        assert_eq!(m.target_markers(&camera).0.unwrap().id, 91);
        m.state.lock().unwrap().units_updated = Some(Instant::now() - Duration::from_secs(1));
        assert_eq!(m.target_markers(&camera).0.unwrap().id, 91);

        // Fresh enemy data must not substitute for an expired hover snapshot.
        let enemy = Unit {
            id: 92,
            friendly: false,
            ..hover_ally()
        };
        m.combat_input(5, (200_000, 200_000), vec![enemy]);
        m.state.lock().unwrap().hover_updated = Some(Instant::now() - Duration::from_secs(1));
        assert!(m.target_markers(&camera).0.is_none());
        // An empty authoritative snapshot is also not an enemy-only fallback.
        m.observe_hover_units((1, 33, 1), 5, 100, &[]);
        assert!(m.target_markers(&camera).0.is_none());
        m.observe_hover_units((1, 33, 1), 5, 100, &[hover_ally()]);
        assert_eq!(m.target_markers(&camera).0.unwrap().id, 91);
    }
    #[test]
    fn hover_snapshot_clears_on_death_inactive_control_and_session_change() {
        let m = movement();
        let log = logger("hover-lifecycle");
        start(&m, &log, 0);
        let camera = recall_camera();
        let hover = Keys {
            cursor: Some((1160., 537.)),
            ..keys(0, 0)
        };
        m.update_mouse(hover, true, &camera, &log);
        m.observe_position(Some((200_000, 200_000)));
        m.observe_hover_units((1, 33, 1), 5, 100, &[hover_ally()]);
        assert_eq!(m.target_markers(&camera).0.unwrap().id, 91);
        m.observe_position(None);
        m.observe_hover_units((1, 33, 1), 5, 100, &[hover_ally()]);
        assert!(m.state.lock().unwrap().hover_updated.is_none());
        assert!(m.target_markers(&camera).0.is_none());

        m.observe_position(Some((200_000, 200_000)));
        m.observe_hover_units((1, 33, 1), 5, 100, &[hover_ally()]);
        m.update_mouse(hover, false, &camera, &log);
        assert!(m.state.lock().unwrap().hover_updated.is_none());
        assert!(m.target_markers(&camera).0.is_none());

        m.observe_hover_units((1, 33, 1), 5, 100, &[hover_ally()]);
        m.begin_match((2, 34, 1), &log);
        // An unrelated begin callback cannot replace the bound session.
        assert!(m.state.lock().unwrap().hover_updated.is_some());
        // Actual session changes go through the existing reset/rearm path.
        m.reset_session(hover, &log);
        m.begin_match((2, 34, 1), &log);
        assert!(m.state.lock().unwrap().hover_updated.is_none());
        m.observe_hover_units((1, 33, 1), 5, 100, &[hover_ally()]);
        assert!(m.state.lock().unwrap().hover_units.is_empty());
        m.reset_session(hover, &log);
        assert!(m.state.lock().unwrap().hover_updated.is_none());
    }
    #[test]
    fn background_map_callbacks_preserve_live_geometry_route_and_destination() {
        let m = movement();
        let log = logger("route-lifetime");
        let original = crate::map_path::Grid::from_json(
            &serde_json::to_string(&vec![vec![0; 30]; 30]).unwrap(),
        )
        .unwrap();
        m.set_navigation(Some(original));
        start(&m, &log, 0);
        let goal = (800_000, 800_000);
        {
            let mut s = m.state.lock().unwrap();
            s.target = Some(Order::Move(goal));
            s.route = [(300_000, 200_000), goal].into();
        }
        let next = crate::map_path::Grid::from_json(
            &serde_json::to_string(&vec![vec![1; 30]; 30]).unwrap(),
        )
        .unwrap();
        m.set_navigation(Some(next));
        m.set_navigation(None);
        assert_eq!(
            m.input(5, (200_000, 200_000)),
            Some(InputV1::move_to(300_000, 200_000))
        );
        {
            let s = m.state.lock().unwrap();
            assert!(s
                .navigation
                .as_ref()
                .unwrap()
                .clear((200_000, 200_000), goal));
            assert_eq!(s.route.len(), 2);
            assert_eq!(s.target, Some(Order::Move(goal)));
        }
        m.reset_session(keys(0, 0), &log);
        let s = m.state.lock().unwrap();
        assert!(s.route.is_empty() && s.target.is_none());
        assert!(!s
            .navigation
            .as_ref()
            .unwrap()
            .clear((200_000, 200_000), goal));
    }
    #[test]
    fn minimap_move_survives_focus_loss_and_stale_heartbeat_without_replaying_click() {
        let m = movement();
        let log = logger("minimap-focus");
        start(&m, &log, 0);
        let camera = recall_camera();
        let pos = (200_000, 200_000);
        let idle = keys(0, 0);
        m.update_mouse(idle, true, &camera, &log);
        m.input(5, pos);
        let click = Keys {
            right: true,
            cursor: Some((1741., 900.)),
            ..idle
        };
        m.update_mouse(click, true, &camera, &log);
        let expected = InputV1::move_to(480_000, 480_000);
        assert_eq!(m.input(5, pos), Some(expected));
        let background = Keys {
            focused: false,
            ..idle
        };
        m.update(background, true, false, &log);
        m.update_mouse(background, true, &camera, &log);
        m.state.lock().unwrap().updated = Some(Instant::now() - Duration::from_secs(2));
        assert_eq!(m.input(5, pos), Some(expected));
        m.update_mouse(
            Keys {
                cursor: Some((1581., 740.)),
                ..click
            },
            true,
            &camera,
            &log,
        );
        assert_eq!(m.input(5, pos), Some(expected)); // Held button on focus return is ignored.
        m.observe_position(None);
        assert!(m.state.lock().unwrap().target.is_none());
    }
    fn keys(dx: i8, dy: i8) -> Keys {
        Keys {
            focused: true,
            dx,
            dy,
            selection: None,
            ..Keys::default()
        }
    }
    fn movement() -> Movement {
        let movement = Movement::new(true);
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
        assert_eq!(
            movement
                .combat_input(5, (200_000, 200_000), units.clone())
                .unwrap()
                .0
                .target
                .target_id,
            9
        );
        assert_eq!(movement.target_markers(&camera).1.unwrap().id, 9);
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
    fn start(movement: &Movement, log: &Logger, lane: usize) {
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
    fn champion_only_direct_filters_minions_but_default_attack_move_ignores_mode() {
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
            9 // Champion wins the overlapping minion even in normal mode.
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
                .0,
            InputV1::action(
                mod_api_stable::InputKindV1::Attack,
                mod_api_stable::InputTargetV1::target(2)
            )
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
        let cursor = Some((1220., 527.)); // Ground beside enemy: keep acquisition/retarget behavior.
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
            2 // Default attack-move prefers the enemy nearest the champion.
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
