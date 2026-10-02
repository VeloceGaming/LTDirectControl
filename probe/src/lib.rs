//! Diagnostic prototype: native publication-boundary coordination on 0.6.2.

mod abilities;
mod camera;
mod combat;
mod hud_icons;
mod movement_test;
mod native_adapter;
mod native_timing;
mod own_selection;
mod platform_input;
mod player_hud;
mod result_audit;
mod runtime_storage;
mod session_ui;
mod sprite_picking;
mod team_info;
mod team_status;
#[cfg(test)]
mod timing_test;
mod tooltips;
mod ui_graphics;
mod wheel;

use mod_api_stable::{
    declare_stable_mod, InputV1, LogLevel, StableAiContext, StableAiInit, StableClient,
    StableExtension, StableHost, StableMod, StablePlayerAi,
};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

const MOD_ID: &str = "lt_direct_control_probe";
const LINE_LIMIT: usize = 12_000;

struct LogState {
    file: Option<File>,
    directory: Option<PathBuf>,
    lines: usize,
    background_lines: usize,
}

struct Logger(Mutex<LogState>);

impl Logger {
    fn directory(&self) -> Option<PathBuf> {
        self.0.lock().ok()?.directory.clone()
    }
    fn next_session(&self) {
        if let Ok(mut s) = self.0.lock() {
            s.lines = 0;
            s.background_lines = 0;
        }
    }
    fn write(&self, text: &str) {
        self.write_sample(text, false);
    }

    fn write_sample(&self, text: &str, background: bool) {
        let Ok(mut state) = self.0.lock() else { return };
        if state.file.is_none() {
            return;
        }
        if background {
            if state.background_lines >= 200 {
                return;
            }
            state.background_lines += 1;
        }
        if state.lines >= LINE_LIMIT {
            return;
        }
        // Clock is diagnostic annotation only, never a simulation input.
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis());
        let _ = writeln!(state.file.as_mut().unwrap(), "{stamp} {text}");
        state.lines += 1;
        if state.lines == LINE_LIMIT {
            let _ = writeln!(
                state.file.as_mut().unwrap(),
                "LOG LIMIT REACHED; session continues with startup/heartbeat recovery guards"
            );
        }
    }
}

#[derive(Default)]
struct ClientObservations {
    elapsed_micros: u64,
    last_sample_micros: u64,
    scene: String,
    timer: Option<String>,
    playback_states: String,
    info: team_info::TeamInfo,
    hud_ui: player_hud::HudUi,
    targeting: platform_input::ChampionOnlyToggle,
    session_ui: session_ui::SessionUi,
    result_audit: result_audit::ResultAudit,
    team_ui: team_status::TeamUi,
}

struct ClientProbe {
    logger: Arc<Logger>,
    observations: Mutex<ClientObservations>,
    timing: Arc<native_timing::NativeTiming>,
    movement: Arc<movement_test::MovementTest>,
    camera: Arc<camera::CameraControl>,
    abilities: Arc<abilities::Abilities>,
    hud: Arc<player_hud::PlayerHud>,
    team: Arc<team_status::TeamStatus>,
    native_enabled: bool,
    install_attempted: AtomicBool,
    management_seen: AtomicBool,
    session_gate: Arc<RwLock<()>>,
}

impl Drop for ClientProbe {
    fn drop(&mut self) {
        wheel::shutdown();
        self.timing
            .cancel("Client extension detached", &self.logger);
    }
}

fn topology(ctx: &StableClient<'_>) -> Vec<String> {
    let mut nodes = ctx
        .ui_child_names("")
        .into_iter()
        .rev()
        .map(|name| (name, 0))
        .collect::<Vec<_>>();
    let mut result = Vec::new();
    while let Some((path, depth)) = nodes.pop() {
        if result.len() >= 240 {
            break;
        }
        if !ctx.ui_exists(&path) {
            continue;
        }
        result.push(format!(
            "{path}: {:?} visible={:?} text={:?}",
            ctx.ui_runner_name(&path),
            ctx.ui_visible(&path),
            ctx.ui_text(&path)
                .map(|text| text.chars().take(100).collect::<String>())
        ));
        if depth < 4 {
            for child in ctx.ui_child_names(&path).into_iter().take(50).rev() {
                nodes.push((format!("{path}.{child}"), depth + 1));
            }
        }
    }
    result
}

impl StableExtension for ClientProbe {
    fn post_update(&self, ctx: &mut StableClient<'_>, dt_micros: u64) {
        if self.native_enabled && !self.install_attempted.load(Ordering::Relaxed) {
            match ctx.scene_kind() {
                Some(mod_api_stable::SceneKindV1::Title) => {
                    self.install_attempted.store(true, Ordering::Relaxed);
                    match native_adapter::install() {
                        Ok(()) => self.timing.installed(
                            true,
                            "verified 0.6.2 SHA256; worker, viewer, movement, input and attack-observer branches installed at title",
                            &self.logger,
                        ),
                        Err(reason) => self.timing.installed(false, &reason, &self.logger),
                    }
                }
                Some(mod_api_stable::SceneKindV1::InGame) => {
                    self.install_attempted.store(true, Ordering::Relaxed);
                    self.timing.installed(
                        false,
                        "Restart game to install adapter at title before loading save",
                        &self.logger,
                    );
                }
                _ => {}
            }
        }
        let scene = ctx.client_scene_kind();
        let battlefield = scene == Some(mod_api_stable::ClientSceneKindV1::InGame);
        let pre_match = matches!(
            scene,
            Some(
                mod_api_stable::ClientSceneKindV1::Match
                    | mod_api_stable::ClientSceneKindV1::StadiumEntrance
                    | mod_api_stable::ClientSceneKindV1::Main
                    | mod_api_stable::ClientSceneKindV1::Lineup
            )
        );
        let save_exit = if ctx.scene_kind() == Some(mod_api_stable::SceneKindV1::Title) {
            self.management_seen.swap(false, Ordering::Relaxed)
        } else {
            if ctx.scene_kind() == Some(mod_api_stable::SceneKindV1::InGame) {
                self.management_seen.store(true, Ordering::Relaxed);
            }
            false
        };
        self.timing.set_binding_window(
            ctx.scene_kind() == Some(mod_api_stable::SceneKindV1::InGame)
                && (pre_match || battlefield),
        );
        self.movement.load_own_team(ctx, &self.logger);
        let mut keys = platform_input::poll();
        self.movement.update(
            keys,
            battlefield,
            matches!(
                scene,
                Some(
                    mod_api_stable::ClientSceneKindV1::Match
                        | mod_api_stable::ClientSceneKindV1::StadiumEntrance
                        | mod_api_stable::ClientSceneKindV1::Main
                        | mod_api_stable::ClientSceneKindV1::Lineup
                )
            ),
            &self.logger,
        );
        let prepared_choice = self.observations.lock().ok().and_then(|o| {
            o.session_ui.take_choice(
                self.timing.phase(),
                self.timing.match_key(),
                self.timing.generation(),
            )
        });
        if self.timing.phase() == Some(native_timing::Phase::Ready) {
            if let (Some(key), Some(lane)) =
                (self.timing.match_key(), prepared_choice.or(keys.selection))
            {
                if let Ok(_gate) = self.session_gate.write() {
                    if self.movement.choose_prepared(key, lane, keys, &self.logger) {
                        self.abilities.reset_session(keys);
                        self.camera.reset_session(keys);
                        if let Some(player) = self.movement.selected() {
                            self.hud.select_prepared(key, player);
                        }
                        if let Ok(mut o) = self.observations.lock() {
                            o.targeting = Default::default();
                        }
                    }
                }
            }
        }
        self.timing.heartbeat(
            battlefield,
            self.movement.selected().is_some(),
            keys,
            &self.logger,
        );
        if self.timing.needs_rearm(pre_match, save_exit) && (save_exit || !keys.release) {
            // Exclude concurrent scalar writes from an old SDK callback while
            // clearing consumers. No host calls or pacing waits under this gate.
            if let Ok(_gate) = self.session_gate.write() {
                self.logger.next_session();
                self.movement.reset_session(keys, &self.logger);
                self.abilities.reset_session(keys);
                self.hud.reset_session();
                self.team.reset_session();
                self.camera.reset_session(keys);
                native_adapter::reset_session();
                if let Ok(mut observations) = self.observations.lock() {
                    observations.targeting = Default::default();
                    observations.hud_ui = Default::default();
                    observations.team_ui = Default::default();
                    if save_exit {
                        observations.result_audit = Default::default();
                    }
                }
                self.timing.rearm(save_exit, keys, &self.logger);
            }
            self.movement.load_own_team(ctx, &self.logger);
        }
        let session_action = self.timing.take_action();
        if let Some(action) = session_action {
            self.movement.clear_commands();
            self.abilities.clear_commands();
            self.timing.apply_action(action, &self.logger);
        }
        wheel::update(
            self.timing.client_controls(None),
            &self.camera,
            &self.timing,
            &self.logger,
        );
        let running = self.timing.client_running();
        let gameplay_active = running && session_action.is_none();
        let controls = self.timing.client_controls(None);
        if let Ok(mut observations) = self.observations.lock() {
            let before = observations.targeting.enabled;
            keys.champion_only =
                observations
                    .targeting
                    .update(keys, controls, self.movement.hud_identity());
            if before != keys.champion_only {
                self.logger.write(&format!(
                    "CHAMPION_ONLY enabled={} buttons={:?} active={controls}",
                    keys.champion_only, keys.champion_toggle
                ));
            }
        }
        let snapshot = self.hud.snapshot(self.movement.hud_identity(), running);
        let skills = self.abilities.hud_skills();
        let status = self.abilities.feedback().unwrap_or_default();
        let mut hud_bounds = Vec::new();
        if let Ok(mut observations) = self.observations.lock() {
            observations.info.apply(
                ctx,
                self.timing.client_controls(None),
                keys.focused,
                keys.team_info,
                &self.logger,
            );
            observations.hud_ui.apply(
                ctx,
                controls,
                snapshot.as_ref(),
                skills,
                keys.champion_only,
                self.camera.locked(),
                self.movement.recalling(),
                &status,
                keys.cursor.filter(|_| keys.focused),
                &self.logger,
            );
            hud_bounds = observations.hud_ui.bounds(ctx);
            hud_bounds.extend(observations.team_ui.apply(
                ctx,
                controls,
                &self.team.snapshot(self.timing.match_key(), running),
                self.movement.selected(),
                &self.logger,
            ));
            hud_bounds.extend(observations.session_ui.apply(
                ctx,
                &self.timing,
                &self.movement.own_players(),
                self.movement.selected(),
                &self.logger,
            ));
            let last_player = self.hud.snapshot(self.movement.hud_identity(), false);
            observations.result_audit.update(
                ctx,
                self.timing.match_key(),
                self.timing.generation(),
                battlefield,
                last_player.as_ref(),
                &self.logger,
            );
        }
        // Read native UI geometry, respecting ancestor visibility. The native
        // renderer's minimap is separately excluded by CameraFrame.
        let mut blocked = Vec::new();
        blocked.extend(hud_bounds);
        let mut command_blocked = blocked.clone();
        for path in [
            "ingame.header",
            "ingame.player_info",
            "ingame.wide_data.camera_buttons",
            "ingame.wide_bottom",
            "ingame.center_data.camera_buttons",
            "ingame.option_buttons",
            "ingame.speed_buttons",
            "ingame.strategy_info",
            "ingame.player_detail",
            "ingame.lt_player_hud",
        ] {
            let visible = ctx.ui_visible(path) == Some(true)
                && path
                    .match_indices('.')
                    .all(|(end, _)| ctx.ui_visible(&path[..end]) != Some(false));
            if visible {
                if let Some((x, y, w, h)) = ctx.ui_node_rect(path) {
                    let rect = camera::Rect { x, y, w, h };
                    if rect.valid() {
                        blocked.push(rect);
                        if path != "ingame.player_info" {
                            command_blocked.push(rect);
                        }
                    }
                }
            }
        }
        self.camera.set_blocked(blocked);
        self.camera.set_command_blocked(command_blocked);
        let ability_action =
            self.abilities
                .update(keys, gameplay_active, &self.camera, &self.logger);
        self.movement.apply_ability_action(ability_action);
        self.movement.update_mouse_with_cast(
            keys,
            gameplay_active,
            &self.camera,
            &self.logger,
            ability_action.left_reserved,
        );
        let Ok(mut observations) = self.observations.lock() else {
            return;
        };
        observations.elapsed_micros = observations.elapsed_micros.saturating_add(dt_micros);
        for event in ctx.input_events() {
            if matches!(
                event.key.as_str(),
                "Q" | "W" | "R" | "A" | "LShift" | "RShift" | "Home" | "End"
            ) {
                self.logger
                    .write(&format!("KEY {:?} {}", event.kind, event.key));
            }
        }
        if observations
            .elapsed_micros
            .saturating_sub(observations.last_sample_micros)
            < 1_000_000
        {
            return;
        }
        observations.last_sample_micros = observations.elapsed_micros;
        self.logger.write(&format!(
            "CONTROL DIAGNOSTIC {} | {:?} | {:?} | {} | {:?} | attack_aim={}",
            self.timing.describe(),
            self.movement.describe(),
            self.abilities.status(),
            self.camera.describe(),
            self.movement.notice(),
            self.movement.attack_move_armed()
        ));
        if self.native_enabled && !native_adapter::sample_status(&self.logger) {
            self.timing
                .cancel("Native CALL bytes changed after installation", &self.logger);
        }
        let timer = [
            "ingame.header.game_time.value",
            "ingame.header.dm_scoreboard.timer",
        ]
        .into_iter()
        .find_map(|path| ctx.ui_text(path));
        if timer != observations.timer {
            self.logger.write(&format!(
                "DISPLAY timer={timer:?} thread={:?}",
                std::thread::current().id()
            ));
            observations.timer = timer;
        }
        let speed_states = [
            "speed05x",
            "speed1x",
            "speed15x",
            "speed2x",
            "speed3x",
            "speed_highlight",
        ]
        .map(|name| {
            (
                name,
                ctx.ui_selectable_selected(&format!("ingame.speed_buttons.{name}")),
            )
        });
        let playback_states = format!("{speed_states:?}");
        if battlefield && observations.playback_states != playback_states {
            self.logger
                .write(&format!("PLAYBACK speed_states={playback_states}"));
            observations.playback_states = playback_states;
        }
        let scene = format!("{:?}/{:?}", ctx.scene_kind(), ctx.client_scene_kind());
        if observations.scene != scene {
            observations.scene = scene.clone();
            self.logger.write(&format!(
                "CLIENT elapsed_us={} scene={scene} thread={:?}",
                observations.elapsed_micros,
                std::thread::current().id()
            ));
            self.logger.write(&format!("UI {:?}", topology(ctx)));
            if self.native_enabled
                && matches!(
                    ctx.client_scene_kind(),
                    Some(
                        mod_api_stable::ClientSceneKindV1::Match
                            | mod_api_stable::ClientSceneKindV1::InGame
                    )
                )
            {
                native_adapter::capture_trace("SDK client scene transition", &self.logger);
            }
        }
    }

    fn post_render(&self, ctx: &mut StableClient<'_>) {
        if !matches!(
            ctx.client_scene_kind(),
            Some(
                mod_api_stable::ClientSceneKindV1::InGame
                    | mod_api_stable::ClientSceneKindV1::Match
                    | mod_api_stable::ClientSceneKindV1::StadiumEntrance
                    | mod_api_stable::ClientSceneKindV1::Main
                    | mod_api_stable::ClientSceneKindV1::Lineup
            )
        ) || !ctx.can_draw()
        {
            return;
        }
        self.abilities.draw(ctx, &self.camera);
        if self.timing.client_running() {
            self.movement.draw_attack_range(ctx, &self.camera);
            self.movement.draw_targets(ctx, &self.camera);
        }
        if let (Some(frame), Some(marker)) = (self.camera.frame(), self.movement.marker()) {
            if let Some((x, y)) = frame.project(marker) {
                if !self.camera.blocked((x, y)) {
                    ctx.draw_circle("UI", x, y, 5., 1002, 0xffd700ff);
                    ctx.draw_rect("UI", x - 12., y - 1., 24., 2., 1002, 0., 0xffd700ff);
                    ctx.draw_rect("UI", x - 1., y - 12., 2., 24., 1002, 0., 0xffd700ff);
                }
            }
        }
        // Normal status is conveyed by icons. Only exceptional release reasons
        // require text; routine Start/Pause/AI transitions stay silent.
        if self.timing.ui_phase() == Some(native_timing::Phase::Released) {
            let message = self.timing.describe();
            if !message.contains("Ctrl+End")
                && !message.contains("Return to AI button")
                && !message.contains("Left battlefield")
            {
                ctx.draw_rect("UI", 18., 110., 700., 32., 1000, 6., 0x141414ff);
                ctx.draw_text(
                    "UI",
                    &message,
                    "asset/base/font/set/regular",
                    (28., 114., 680., 24.),
                    1001,
                    16.,
                    0xeeeeeeff,
                    mod_api_stable::TextAlignXV1::Left,
                    mod_api_stable::TextAlignYV1::Center,
                );
            }
        }
    }
}

struct AiProbe {
    logger: Arc<Logger>,
    last_sample: Option<(usize, u32, u64, u64, u64)>,
    timing: Arc<native_timing::NativeTiming>,
    movement: Arc<movement_test::MovementTest>,
    abilities: Arc<abilities::Abilities>,
    hud: Arc<player_hud::PlayerHud>,
    team: Arc<team_status::TeamStatus>,
    last_manual: bool,
    last_manual_mode: Option<&'static str>,
    session_gate: Arc<RwLock<()>>,
}

impl StablePlayerAi for AiProbe {
    fn clone_box(&self) -> Box<dyn StablePlayerAi> {
        Box::new(Self {
            logger: self.logger.clone(),
            last_sample: None,
            timing: self.timing.clone(),
            movement: self.movement.clone(),
            abilities: self.abilities.clone(),
            hud: self.hud.clone(),
            team: self.team.clone(),
            last_manual: false,
            last_manual_mode: None,
            session_gate: self.session_gate.clone(),
        })
    }

    fn id(&self) -> String {
        format!("{MOD_ID}:observer")
    }

    fn matches(&self, init: &StableAiInit) -> bool {
        // Every live player can pace; player zero logs. Only the selected player
        // receive movement; every other callback preserves the native input.
        init.player_id < 10
    }

    fn priority(&self) -> i32 {
        1000
    }

    fn think(&mut self, ctx: &mut StableAiContext<'_>, _base: Option<InputV1>) -> Option<InputV1> {
        let _session = self.session_gate.read().ok()?;
        let tick = ctx.tick();
        let player_id = ctx.player_id();
        let athlete = ctx.athlete_id();
        let lane = ctx.lane().map(|lane| lane.code() as usize);
        let side = ctx.team();
        let (selected, proposal, position, actor, match_key, skill_units) = {
            let sim = ctx.sim()?;
            let Some(origin) = sim.sim_origin() else {
                if self.last_sample.is_none() {
                    self.logger.write("SIM origin unavailable");
                    self.last_sample = Some((tick, u32::MAX, 0, 0, 0));
                }
                return None;
            };
            let key = (sim.seed(), origin.match_id, origin.set_index);
            if origin.kind == 2 {
                // Bind before touching selection; late result/re-simulation
                // callbacks cannot register players for another session.
                self.timing.observe(key, tick, &self.logger);
                if tick == 1 && self.timing.owns_worker(key) {
                    self.movement.begin_match(key, &self.logger);
                    if let Some(lane) = lane {
                        let champion = sim
                            .get_player(player_id)
                            .and_then(|player| player.champion())
                            .and_then(|champion| champion.name())
                            .unwrap_or_default();
                        self.movement.register_player(
                            key,
                            own_selection::PlayerIdentity {
                                player: player_id,
                                athlete,
                                lane,
                                side,
                                champion,
                            },
                            &self.logger,
                        );
                    }
                }
                // Identify this worker only. The SDK callback NEVER waits.
            }
            let selected = origin.kind == 2
                && self.timing.owns_worker(key)
                && self.movement.hud_identity() == Some((key, player_id));
            // Any living actor can read all ten players, including dead ones.
            // Copy only SDK scalars on this session's original live worker.
            if origin.kind == 2
                && self.timing.accepts_sample(key)
                && self.team.needs_sample(key, tick)
            {
                let players = (0..sim.player_count())
                    .filter_map(|index| {
                        let p = sim.player_at(index)?;
                        Some(team_status::Player {
                            id: p.id(),
                            side: p.team(),
                            lane: p.lane()?.code() as usize,
                            champion: p.champion().and_then(|c| c.name()).unwrap_or_default(),
                            alive: Some(p.is_alive()),
                            respawn: p.respawn_time(),
                        })
                    })
                    .collect();
                if tick == 1 {
                    for index in 0..sim.player_count() {
                        if let Some(p) = sim.player_at(index) {
                            let c = p.champion();
                            self.hud.observe_prepared(player_hud::Snapshot {
                                key,
                                player: p.id(),
                                champion: c.as_ref().and_then(|c| c.name()).unwrap_or_default(),
                                level: p.level(),
                                hp: c.as_ref().map(|c| c.hp()),
                                alive: p.is_alive(),
                                respawn: p.respawn_time(),
                                gold: p.gold(),
                                kda: (p.kills(), p.deaths(), p.assists()),
                                cs: p.cs(),
                                cooldowns: p.cooldowns().map_or([0; 3], |(_, q, w, r)| [q, w, r]),
                                items: p.item_keys(),
                            });
                        }
                    }
                }
                self.team.observe(key, players, &self.logger);
            }
            // Other living actors still run think while the selected champion
            // is dead. Read the selected player from this same worker's sim.
            if let Some((hud_key, hud_player)) =
                self.movement.hud_identity().filter(|(hud_key, _)| {
                    origin.kind == 2 && *hud_key == key && self.timing.accepts_sample(key)
                })
            {
                if self.hud.needs_sample(hud_key, hud_player, tick) {
                    if let Some(player) = sim.get_player(hud_player) {
                        let champion = player.champion();
                        let cooldowns = player.cooldowns().map_or([0; 3], |(_, q, w, r)| [q, w, r]);
                        if !player.is_alive() {
                            self.movement.observe_position(None);
                            self.abilities.observe_actor(key, None, None, cooldowns);
                        }
                        self.hud.observe_tick(
                            player_hud::Snapshot {
                                key,
                                player: hud_player,
                                champion: champion
                                    .as_ref()
                                    .and_then(|c| c.name())
                                    .unwrap_or_default(),
                                level: player.level(),
                                hp: champion.as_ref().map(|c| c.hp()),
                                alive: player.is_alive(),
                                respawn: player.respawn_time(),
                                gold: player.gold(),
                                kda: (player.kills(), player.deaths(), player.assists()),
                                cs: player.cs(),
                                cooldowns,
                                items: player.item_keys(),
                            },
                            tick,
                            &self.logger,
                        );
                    }
                }
            }
            let current_champion = selected
                .then(|| {
                    sim.get_player(player_id)
                        .and_then(|player| player.champion())
                })
                .flatten();
            if selected {
                self.movement.observe_position(
                    current_champion
                        .as_ref()
                        .filter(|c| c.is_alive())
                        .map(|c| c.pos()),
                );
                let living = current_champion.as_ref().filter(|c| c.is_alive());
                let cooldowns = sim
                    .get_player(player_id)
                    .and_then(|p| p.cooldowns())
                    .map_or([0; 3], |(_, q, w, r)| [q, w, r]);
                self.abilities.observe_actor(
                    key,
                    living.map(|c| c.id()),
                    living.map(|c| c.pos()),
                    cooldowns,
                );
                if let (Some(c), Some(p)) = (living, sim.get_player(player_id)) {
                    self.abilities.observe_level(key, c.id(), p.level());
                }
            }
            let skill_units = if selected && self.timing.allows_input(key) {
                (0..sim.entity_count())
                    .filter_map(|index| {
                        let unit = sim.entity_at(index)?;
                        (unit.is_alive()
                            && unit.is_targetable()
                            && (unit.team() == side || sim.is_visible(side, unit.id())))
                        .then(|| combat::Unit {
                            id: unit.id(),
                            position: unit.pos(),
                            radius: unit.radius() as u64,
                            is_champion: unit.is_champion(),
                            body: unit
                                .is_champion()
                                .then(|| sprite_picking::body(unit.name().as_deref())),
                        })
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            let candidate = if selected && self.timing.allows_input(key) {
                current_champion
                    .as_ref()
                    .filter(|champion| champion.is_alive())
                    .and_then(|champion| {
                        let units = (0..sim.entity_count())
                            .filter_map(|index| {
                                let unit = sim.entity_at(index)?;
                                (unit.id() != champion.id()
                                    && unit.team() != side
                                    && unit.is_alive()
                                    && unit.is_targetable()
                                    && sim.is_visible(side, unit.id()))
                                .then(|| combat::Unit {
                                    id: unit.id(),
                                    position: unit.pos(),
                                    radius: unit.radius() as u64,
                                    is_champion: unit.is_champion(),
                                    body: unit
                                        .is_champion()
                                        .then(|| sprite_picking::body(unit.name().as_deref())),
                                })
                            })
                            .collect();
                        self.movement.combat_input(player_id, champion.pos(), units)
                    })
            } else {
                None
            };
            let position = current_champion.as_ref().map(|champion| champion.pos());
            // Record each second of foreground startup, then every ten seconds.
            let startup_sample = origin.kind == 2 && tick <= 1200 && tick.is_multiple_of(60);
            let sample = (
                tick,
                origin.kind,
                origin.match_id,
                origin.replay_id,
                origin.set_index,
            );
            if player_id == 0
                && (tick <= 2 || startup_sample || tick.is_multiple_of(600))
                && self.last_sample != Some(sample)
            {
                self.last_sample = Some(sample);
                self.logger.write_sample(
            &format!(
                "SIM tick={tick} seed={} kind={} match={} replay={} set={} athlete={} players={} thread={:?}",
                sim.seed(),
                origin.kind,
                origin.match_id,
                origin.replay_id,
                origin.set_index,
                athlete,
                sim.player_count(), std::thread::current().id()
            ),
            origin.kind <= 1,
        );
                // Foreground MatchView only; do not fill the log with background rosters.
                if origin.kind == 2 && tick <= 2 {
                    native_adapter::capture_trace(
                        &format!("SDK foreground AI tick {tick}"),
                        &self.logger,
                    );
                    for index in 0..sim.player_count() {
                        let Some(player) = sim.player_at(index) else {
                            continue;
                        };
                        let champ = player
                            .champion()
                            .map(|entity| (entity.name(), entity.pos(), entity.hp()));
                        self.logger.write(&format!(
                            "ROSTER player={} team={} lane={:?} champion={champ:?} cooldowns={:?}",
                            player.id(),
                            player.team(),
                            player.lane(),
                            player.cooldowns()
                        ));
                    }
                }
            }
            let actor = current_champion
                .as_ref()
                .filter(|c| c.is_alive())
                .map(|c| c.id());
            (selected, candidate, position, actor, key, skill_units)
        };
        let skill = actor
            .zip(position)
            .filter(|_| selected && self.timing.allows_input(match_key))
            .and_then(|(actor, position)| {
                self.abilities.take_input(
                    match_key,
                    actor,
                    position,
                    &skill_units,
                    |input| ctx.is_valid_input(input),
                    &self.logger,
                )
            });
        // Rejected attacks normally mean this target needs approaching or an
        // action cannot start yet. Keep manual ownership and use a native move
        // request. Never deliver an invalid attack to the host or fake damage.
        let candidate = skill.or_else(|| {
            proposal.map(|(request, chase)| {
                if request.kind == mod_api_stable::InputKindV1::Return.code()
                    && !ctx.is_valid_input(&request)
                {
                    self.movement.recall_rejected(&self.logger);
                    let p = position.unwrap_or_default();
                    InputV1::move_to(p.0, p.1)
                } else if request.kind == mod_api_stable::InputKindV1::Attack.code()
                    && !ctx.is_valid_input(&request)
                {
                    chase
                        .map(|p| InputV1::move_to(p.0, p.1))
                        .unwrap_or_else(|| {
                            let p = position.unwrap_or_default();
                            InputV1::move_to(p.0, p.1)
                        })
                } else {
                    request
                }
            })
        });
        let valid = candidate
            .as_ref()
            .is_some_and(|input| ctx.is_valid_input(input));
        let mode = candidate.map(|input| {
            if input.kind == mod_api_stable::InputKindV1::Attack.code() {
                "attack"
            } else if input.kind == mod_api_stable::InputKindV1::Return.code() {
                "recall"
            } else if input.kind >= mod_api_stable::InputKindV1::Skill.code() {
                "skill"
            } else if position == Some((input.x, input.y)) {
                "hold"
            } else {
                "move"
            }
        });
        if selected
            && (candidate.is_some()
                && (tick.is_multiple_of(60)
                    || valid != self.last_manual
                    || mode != self.last_manual_mode)
                || candidate.is_none() && self.last_manual)
        {
            self.logger.write(&format!("MANUAL input tick={tick} player={player_id} mode={mode:?} pos={position:?} request={candidate:?} valid={valid} base={_base:?}"));
        }
        self.last_manual = valid;
        self.last_manual_mode = mode;
        if selected {
            native_adapter::arm_stop(
                match_key,
                actor.filter(|_| valid && matches!(mode, Some("hold" | "move" | "attack"))),
                valid && mode == Some("hold"),
                valid && self.movement.take_cancel_recall(player_id),
            );
        }
        if candidate.is_some() && !valid {
            // Never silently Pass a rejected manual tick while the overlay
            // still claims control. Release the session and report the reason.
            self.timing
                .cancel("Manual command rejected; native AI restored", &self.logger);
        }
        // An idle living champion now returns Some(move_to(current_position)).
        // None keeps native input only outside control, with no living actor,
        // or after an explicitly logged failure releases the coordinator.
        candidate.filter(|_| valid)
    }
}

fn init(host: &StableHost) -> StableMod {
    let mut declaration = StableMod::new(MOD_ID);
    let (file, directory) = match runtime_storage::open_log() {
        Ok((file, directory)) => (Some(file), Some(directory)),
        Err(reason) => {
            host.log(LogLevel::Warn, &format!(
                "LT Direct Control cannot open a diagnostic log: {reason}; control remains available"
            ));
            (None, None)
        }
    };
    let logger = Arc::new(Logger(Mutex::new(LogState {
        file,
        directory: directory.clone(),
        lines: 0,
        background_lines: 0,
    })));
    let version = host.game_version();
    logger.write(&format!(
        "INIT game={}.{}.{} abi={} probe={}",
        version.major,
        version.minor,
        version.patch,
        host.abi_level(),
        env!("CARGO_PKG_VERSION")
    ));
    sprite_picking::initialize(&logger);
    logger.write(
        "TIMING native_enabled=true wait=after_frame_publication bootstrap=one_frame explicit_start=true full_match=true loading_guard_seconds=15 heartbeat_guard_seconds=2 maximum_frame_lead=2 movement_enabled=true"
    );
    let timing = Arc::new(native_timing::NativeTiming::new(true));
    let movement = Arc::new(movement_test::MovementTest::new(true));
    let camera = Arc::new(camera::CameraControl::default());
    let abilities = Arc::new(abilities::Abilities::default());
    let hud = Arc::new(player_hud::PlayerHud::default());
    let team = Arc::new(team_status::TeamStatus::default());
    let session_gate = Arc::new(RwLock::new(()));
    let native_enabled = match native_adapter::configure(
        timing.clone(),
        logger.clone(),
        movement.clone(),
        camera.clone(),
        abilities.clone(),
    ) {
        Ok(()) => true,
        Err(reason) => {
            timing.installed(false, reason, &logger);
            false
        }
    };
    host.log(
        LogLevel::Info,
        &directory.map_or_else(
            || "LT Direct Control enabled; file diagnostics unavailable".to_owned(),
            |root| {
                format!(
                    "LT Direct Control enabled; recording to {}",
                    root.join("probe.log").display()
                )
            },
        ),
    );
    declaration.set_extension(ClientProbe {
        logger: logger.clone(),
        observations: Mutex::default(),
        timing: timing.clone(),
        movement: movement.clone(),
        camera,
        abilities: abilities.clone(),
        hud: hud.clone(),
        team: team.clone(),
        native_enabled,
        install_attempted: AtomicBool::new(false),
        management_seen: AtomicBool::new(false),
        session_gate: session_gate.clone(),
    });
    declaration.add_player_input_ai(AiProbe {
        session_gate,
        logger,
        last_sample: None,
        timing,
        movement,
        abilities,
        hud,
        team,
        last_manual: false,
        last_manual_mode: None,
    });
    declaration
}

declare_stable_mod!(init, requires = 6);
