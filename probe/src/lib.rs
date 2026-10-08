//! Diagnostic prototype: native publication-boundary coordination on 0.6.3.

mod abilities;
mod attack_trace;
mod camera;
mod combat;
mod cursor;
mod hud_icons;
mod hud_motion;
mod hud_style;
mod input_trace;
mod inventory;
mod logging;
mod map_path;
mod minimap;
mod movement_test;
mod native_adapter;
mod native_items;
mod native_preview;
#[cfg(all(windows, target_arch = "x86_64"))]
mod native_profile;
mod native_timing;
mod own_selection;
mod perf;
mod platform_input;
mod player_hud;
mod purchase_tracker;
mod result_audit;
mod runtime_storage;
mod screen_effect;
mod session_ui;
mod settings;
mod settings_ui;
mod shop;
mod shop_trace;
mod shop_ui;
mod skill_preview;
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
const LOG_BYTES_LIMIT: usize = 4 * 1024 * 1024;

struct LogState {
    file: Option<File>,
    directory: Option<PathBuf>,
    lines: usize,
    background_lines: usize,
    bytes: usize,
    rotating: bool,
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
        if !logging::enabled(text) {
            return;
        }
        let since = std::time::Instant::now();
        let Ok(mut state) = self.0.lock() else { return };
        perf::waited(perf::Wait::Logger, since);
        if state.file.is_none() {
            return;
        }
        if background {
            if state.background_lines >= 200 {
                return;
            }
            state.background_lines += 1;
        }
        if state.rotating && state.bytes.saturating_add(text.len() + 32) >= LOG_BYTES_LIMIT {
            let directory = state.directory.clone().unwrap();
            if runtime_storage::rotate_log(state.file.as_mut().unwrap(), &directory).is_err() {
                return;
            }
            state.bytes = 0;
            let marker = "LOG SEGMENT continued; prior records in probe.segment.log and probe.segment.previous.log\n";
            let _ = state.file.as_mut().unwrap().write_all(marker.as_bytes());
            state.bytes += marker.len();
        } else if !state.rotating && state.lines >= LINE_LIMIT {
            return;
        }
        // Clock is diagnostic annotation only, never a simulation input.
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis());
        let record = format!("{stamp} {text}\n");
        let _ = state.file.as_mut().unwrap().write_all(record.as_bytes());
        state.bytes = state.bytes.saturating_add(record.len());
        perf::logged(record.len());
        state.lines += 1;
        if !state.rotating && state.lines == LINE_LIMIT {
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
    settings_ui: settings_ui::SettingsUi,
    shop_ui: shop_ui::ShopUi,
    result_audit: result_audit::ResultAudit,
    team_ui: team_status::TeamUi,
    early_input: Option<(u64, platform_input::Keys)>,
    screen_effect: screen_effect::Effect,
}

struct ClientProbe {
    logger: Arc<Logger>,
    observations: Mutex<ClientObservations>,
    timing: Arc<native_timing::NativeTiming>,
    movement: Arc<movement_test::MovementTest>,
    camera: Arc<camera::CameraControl>,
    cursor: cursor::Cursor,
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
        settings::MODAL.store(false, Ordering::Relaxed);
        self.cursor.shutdown(&self.logger);
        wheel::shutdown();
        self.timing
            .cancel("Client extension detached", &self.logger);
    }
}

impl ClientProbe {
    fn resolve_targeting(&self, keys: &mut platform_input::Keys, controls: bool) {
        if let Ok(mut o) = self.observations.lock() {
            let before = o.targeting.enabled;
            keys.champion_only = o
                .targeting
                .update(*keys, controls, self.movement.hud_identity());
            if before != keys.champion_only {
                self.logger.write(&format!(
                    "CHAMPION_ONLY enabled={} buttons={:?} active={controls}",
                    keys.champion_only, keys.champion_toggle
                ));
            }
        }
    }
    fn gameplay_input(&self, keys: platform_input::Keys, active: bool) {
        let action = self
            .abilities
            .update(keys, active, &self.camera, &self.logger);
        self.movement.apply_ability_action(action);
        self.movement.update_mouse_with_cast(
            keys,
            active,
            &self.camera,
            &self.logger,
            action.left_reserved,
        );
    }
}

impl ClientProbe {
    fn pre_update_inner(&self, ctx: &mut StableClient<'_>) {
        if ctx.client_scene_kind() != Some(mod_api_stable::ClientSceneKindV1::InGame)
            || settings::MODAL.load(Ordering::Relaxed)
            || !self.timing.client_early_input()
        {
            return;
        }
        // Use the last rendered camera and visible UI masks. These describe
        // what the user clicked, before the host advances camera/playback.
        let mut keys = platform_input::poll();
        self.resolve_targeting(&mut keys, true);
        self.movement.update(keys, true, false, &self.logger);
        self.gameplay_input(keys, true);
        if let Ok(mut o) = self.observations.lock() {
            o.early_input = Some((self.timing.generation(), keys));
        }
    }
    fn post_update_inner(&self, ctx: &mut StableClient<'_>, dt_micros: u64) {
        if self.native_enabled && !self.install_attempted.load(Ordering::Relaxed) {
            match ctx.scene_kind() {
                Some(mod_api_stable::SceneKindV1::Title) => {
                    self.install_attempted.store(true, Ordering::Relaxed);
                    match native_adapter::install() {
                        Ok(()) => self.timing.installed(
                            true,
                            "verified 0.6.3 SHA256; worker, viewer, movement, input and attack-observer branches installed at title",
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
        let early = self
            .observations
            .lock()
            .ok()
            .and_then(|mut o| o.early_input.take())
            .filter(|(g, _)| {
                *g == self.timing.generation() && battlefield && self.timing.client_running()
            });
        let mut keys = early.map_or_else(platform_input::poll, |(_, keys)| keys);
        if early.is_none() {
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
        }
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
                shop::SHOP.reset_session();
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
        let gameplay_active =
            running && session_action.is_none() && !settings::MODAL.load(Ordering::Relaxed);
        let controls = self.timing.client_controls(None);
        if early.is_none() || !gameplay_active {
            self.resolve_targeting(&mut keys, controls);
        }
        let identity = self.movement.hud_identity();
        let snapshot = self.hud.snapshot(identity, running);
        let artwork = snapshot
            .is_none()
            .then(|| self.hud.artwork(identity))
            .flatten();
        native_adapter::set_death_greyscale(
            controls && snapshot.as_ref().is_some_and(|s| !s.alive),
        );
        let skills = self.abilities.hud_skills();
        let status = self.abilities.feedback().unwrap_or_default();
        let mut hud_bounds = Vec::new();
        let mut hovered_skill = None;
        if let Ok(mut observations) = self.observations.lock() {
            let team_open = keys.team_info || observations.session_ui.team_open();
            let t = perf::time(perf::Section::Info);
            observations.info.apply(
                ctx,
                self.timing.client_controls(None),
                keys.focused,
                false, // The styled Tab panel owns visibility during direct control.
                &self.logger,
            );
            drop(t);
            let t = perf::time(perf::Section::Hud);
            observations.hud_ui.apply(
                ctx,
                controls,
                matches!(
                    self.timing.phase(),
                    Some(native_timing::Phase::Running | native_timing::Phase::Paused)
                ),
                snapshot.as_ref(),
                artwork.as_ref(),
                skills,
                &status,
                keys.cursor.filter(|_| keys.focused),
                keys.left && keys.focused,
                &self.hud,
                &self.logger,
            );
            if controls && keys.focused && snapshot.as_ref().is_some_and(|s| s.alive) {
                hovered_skill = observations.hud_ui.hovered_skill(ctx, keys.cursor);
            }
            hud_bounds = observations.hud_ui.bounds(ctx);
            drop(t);
            let t = perf::time(perf::Section::Team);
            {
                let ClientObservations {
                    team_ui, hud_ui, ..
                } = &mut *observations;
                let team_active = controls && keys.focused && team_open;
                let roster = if team_active {
                    self.team.snapshot(self.timing.match_key(), running)
                } else {
                    Vec::new()
                };
                team_ui.apply(
                    ctx,
                    team_active,
                    &roster,
                    self.movement.selected(),
                    hud_ui,
                    &self.logger,
                );
            }
            drop(t);
            let t = perf::time(perf::Section::Session);
            hud_bounds.extend(observations.session_ui.apply(
                ctx,
                &self.timing,
                &self.movement.own_players(),
                self.movement.selected(),
                &self.camera,
                &self.cursor.settings,
                keys.cursor.filter(|_| keys.focused),
                keys.left && keys.focused,
                &self.logger,
            ));
            if observations.session_ui.take_settings() {
                observations.settings_ui.open(
                    &self.timing,
                    &self.cursor.settings,
                    keys,
                    &self.logger,
                );
                self.movement.clear_commands();
                self.abilities.clear_commands();
            }
            drop(t);
            let t = perf::time(perf::Section::Settings);
            let camera_lock_before = self.cursor.settings.number("camera_lock");
            hud_bounds.extend(observations.settings_ui.apply(
                ctx,
                &self.timing,
                &self.cursor.settings,
                keys,
                &self.logger,
            ));
            let camera_lock_after = self.cursor.settings.number("camera_lock");
            if camera_lock_before != camera_lock_after {
                self.camera.set_locked(camera_lock_after == 1.);
            }
            drop(t);
            let t = perf::time(perf::Section::Shop);
            {
                let ClientObservations {
                    shop_ui, hud_ui, ..
                } = &mut *observations;
                if hud_ui.take_shop_click() {
                    shop_ui.toggle();
                }
                // Open from champion lock (incl. the Start-control wait, to
                // buy starting items) until control returns to the AI.
                let shop_active = battlefield
                    && self.movement.hud_identity().is_some()
                    && matches!(
                        self.timing.phase(),
                        Some(
                            native_timing::Phase::Ready
                                | native_timing::Phase::Running
                                | native_timing::Phase::Paused
                        )
                    );
                hud_bounds.extend(shop_ui.apply(
                    ctx,
                    keys,
                    shop_active,
                    &self.timing,
                    hud_ui,
                    &self.logger,
                ));
            }
            drop(t);
            let t = perf::time(perf::Section::Effect);
            observations.screen_effect.update(
                self.cursor.settings.low_health(),
                controls && battlefield,
                snapshot.as_ref(),
                dt_micros,
            );
            observations
                .screen_effect
                .apply(ctx, &self.camera, &self.logger);
            drop(t);
            let _t = perf::time(perf::Section::Audit);
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
        if early.is_none() || !gameplay_active {
            self.gameplay_input(
                keys,
                gameplay_active && !settings::MODAL.load(Ordering::Relaxed),
            );
        }
        self.abilities.set_hud_hover(hovered_skill);
        let hover_timer = perf::time(perf::Section::Hover);
        let (hover, attack) = self.movement.refresh_hover(
            &self.camera,
            self.native_enabled && gameplay_active && battlefield && keys.focused,
            self.abilities.hover_target(keys.alt),
        );
        let outline_enabled = self.cursor.settings.number("hover_outline") == 1.;
        native_adapter::set_outline_targets(
            hover.filter(|_| outline_enabled),
            attack.filter(|_| outline_enabled),
            self.movement
                .attack_click_feedback()
                .filter(|_| outline_enabled),
        );
        drop(hover_timer);
        let cursor_timer = perf::time(perf::Section::Cursor);
        // The reason is diagnostic only (0.65 cursor trace).
        let cursor_style = (|| {
            if !(self.native_enabled && battlefield && controls) {
                return Err("not_controlling");
            }
            if !keys.focused {
                return Err("unfocused");
            }
            let point = keys.cursor.ok_or("no_cursor_point")?;
            let frame = self.camera.frame().ok_or("no_camera_frame")?;
            if self.camera.command_blocked(point) {
                return Err("over_hud_or_minimap");
            }
            if frame.unproject(point).is_none() {
                return Err("off_battlefield");
            }
            let playable = gameplay_active && snapshot.as_ref().is_some_and(|s| s.alive);
            Ok(cursor::Style::choose(
                keys.champion_only,
                playable && self.movement.attack_move_armed(),
                playable && self.movement.cursor_enemy().is_some(),
                playable
                    .then(|| self.abilities.cursor_feedback(keys.alt))
                    .flatten(),
            ))
        })();
        self.cursor.update(cursor_style, keys.cursor, &self.logger);
        drop(cursor_timer);
        let Ok(mut observations) = self.observations.lock() else {
            return;
        };
        observations.elapsed_micros = observations.elapsed_micros.saturating_add(dt_micros);
        for event in ctx.input_events() {
            if matches!(
                event.key.as_str(),
                "Q" | "W" | "R" | "A" | "LShift" | "RShift" | "F11" | "F12"
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
            // No full UI-tree dump here: walking it took ~200 ms at match start (0.65 log).
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

    fn post_render_inner(&self, ctx: &mut StableClient<'_>) {
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
        minimap::draw_frame(ctx, &self.camera);
        if self.timing.client_running() {
            self.movement.draw_attack_range(ctx, &self.camera);
            self.movement.draw_targets(
                ctx,
                &self.camera,
                self.cursor.settings.number("selection_debug") == 1.,
            );
            if self.cursor.settings.number("map_path") == 1. {
                self.movement.draw_minimap_path(ctx, &self.camera);
            }
            cursor::draw_clicks(ctx, &self.camera, &self.movement.click_feedback());
        }
        // Normal status is conveyed by icons. Only exceptional release reasons
        // require text; routine Start/Pause/AI transitions stay silent.
        if self.timing.ui_phase() == Some(native_timing::Phase::Released) {
            let message = self.timing.describe();
            if !message.contains("F12")
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

impl StableExtension for ClientProbe {
    fn pre_update(&self, ctx: &mut StableClient<'_>, _dt_micros: u64) {
        let _t = perf::time(perf::Section::Pre);
        self.pre_update_inner(ctx);
    }
    fn post_update(&self, ctx: &mut StableClient<'_>, dt_micros: u64) {
        let scene = ctx.client_scene_kind();
        perf::frame(
            scene == Some(mod_api_stable::ClientSceneKindV1::InGame),
            &scene,
            &self.logger,
        );
        let _t = perf::time(perf::Section::Post);
        logging::set(self.cursor.settings.number("log_level"));
        self.post_update_inner(ctx, dt_micros);
    }
    fn post_render(&self, ctx: &mut StableClient<'_>) {
        let _t = perf::time(perf::Section::Render);
        self.post_render_inner(ctx);
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
        let (selected, proposal, position, actor, match_key, skill_units, live_worker) = {
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
            // Startup tick 1 precedes presentation readiness. Capture roster
            // HUD data here rather than hiding it behind accepts_sample(),
            // whose battlefield gate only opens after startup is held.
            if origin.kind == 2 && tick == 1 && self.timing.owns_worker(key) {
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
                            build: self.hud.build(&sim, p.id()),
                        });
                    }
                }
            }
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
                            level: p.level(),
                            gold: p.gold(),
                            kda: (p.kills(), p.deaths(), p.assists()),
                            cs: p.cs(),
                            items: p.item_keys(),
                            build: self.hud.build(&sim, p.id()),
                            fresh: true,
                        })
                    })
                    .collect();
                self.team.observe(key, players, &self.logger);
            }
            // Read-only shop investigation: every purchase step of all ten
            // players, plus a once-per-match check of the native buyer code.
            if origin.kind == 2
                && self.timing.accepts_sample(key)
                && shop_trace::SHOP_TRACE.due(key, tick)
            {
                if shop_trace::SHOP_TRACE.take_anchor_check() {
                    for line in native_adapter::buyer_anchor_report() {
                        self.logger.write(&line);
                    }
                }
                let players = (0..sim.player_count())
                    .filter_map(|index| {
                        let p = sim.player_at(index)?;
                        Some((p.id(), p.team(), p.gold()))
                    })
                    .collect::<Vec<_>>();
                shop_trace::SHOP_TRACE.observe(
                    tick,
                    &players,
                    |id| {
                        sim.get_player(id)
                            .map(|p| p.item_keys())
                            .unwrap_or_default()
                    },
                    |id| shop_trace::Detail {
                        champion: sim
                            .get_player(id)
                            .and_then(|p| p.champion())
                            .and_then(|c| c.name())
                            .unwrap_or_default(),
                        build: self.hud.build(&sim, id),
                        native_owned: native_adapter::player_owned_len(&sim, id),
                    },
                    |line| self.logger.write(line),
                );
            }
            // Manual shopping: once per tick, from whichever actor runs first,
            // publish the controlled champion's shop data and next purchase
            // step. Once the champion's native object is known with Manual
            // shopping on, missing item data fails closed (buys nothing).
            // Owning the worker suffices: starting items are bought in the
            // first ticks, before the battlefield view (accepts_sample) opens.
            if origin.kind == 2
                && (self.timing.accepts_sample(key) || self.timing.owns_worker(key))
                && shop::SHOP.due(key, tick)
            {
                let controlled = self
                    .movement
                    .hud_identity()
                    .filter(|(hud_key, _)| *hud_key == key)
                    .map(|(_, player)| player);
                let keys = self.hud.catalog_keys();
                let cat = shop::SHOP.catalogue_for(key, || {
                    let meta = self.hud.item_metadata(key)?;
                    shop::catalogue(&keys, meta.as_ref())
                });
                let live = controlled.and_then(|id| {
                    let player = sim.get_player(id)?;
                    let index = |k: &String| keys.iter().position(|x| x == k);
                    Some(shop::Live {
                        owned: player
                            .item_keys()
                            .iter()
                            .map(index)
                            .collect::<Option<Vec<_>>>()?,
                        build: native_adapter::player_build(&sim, id)?,
                        gold: player.gold(),
                        capacity: native_adapter::item_slot_capacity().unwrap_or(0),
                    })
                });
                let mode = if settings::option("manual_shop") != 1. {
                    shop::Mode::Native("Manual shopping off")
                } else if !native_adapter::shop_ready() {
                    shop::Mode::Native("buyer hooks unavailable")
                } else if !matches!(
                    self.timing.worker_phase(key),
                    Some(
                        native_timing::Phase::Loading
                            | native_timing::Phase::Ready
                            | native_timing::Phase::Running
                            | native_timing::Phase::Paused
                    )
                ) {
                    // From champion lock (including loading and the
                    // Start-control wait, when the game buys starting items)
                    // until control returns to the AI (F12).
                    shop::Mode::Native("AI control")
                } else if matches!(
                    self.timing.worker_phase(key),
                    Some(native_timing::Phase::Loading | native_timing::Phase::Ready)
                ) {
                    // Starting items are bought at tick 1, while the lane may
                    // still change: hold every champion of this match (by its
                    // native object, so other simulations are untouched)
                    // until Start; the others then buy on the next tick.
                    shop::Mode::Prestart {
                        controlled: controlled
                            .and_then(|id| native_adapter::native_player(&sim, id)),
                        hold: (0..sim.player_count())
                            .filter_map(|i| {
                                native_adapter::native_player(&sim, sim.player_at(i)?.id())
                            })
                            .collect(),
                    }
                } else {
                    match controlled.and_then(|id| native_adapter::native_player(&sim, id)) {
                        Some(pointer) => shop::Mode::Manual(pointer),
                        None => shop::Mode::Native("controlled champion object unknown"),
                    }
                };
                shop::SHOP.publish(mode, cat, live, |line| self.logger.write(line));
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
                                build: self.hud.build(&sim, hud_player),
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
                    self.abilities.observe_champion(key, c.id(), c.name());
                }
            }
            let collect_units = selected
                && self.timing.allows_input(key)
                && current_champion.as_ref().is_some_and(|c| c.is_alive());
            let skill_units = if collect_units {
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
                            is_minion: unit.is_minion(),
                            friendly: unit.team() == side,
                            in_cc: (0..unit.cc_count().min(64))
                                .any(|i| unit.cc_at(i).is_some_and(|cc| cc.kind < 10)),
                            is_tower: unit.is_tower() || unit.name().as_deref() == Some("nexus"),
                            body: sprite_picking::entity_body(
                                unit.name().as_deref(),
                                unit.is_champion(),
                                unit.is_minion(),
                                unit.is_tower(),
                                unit.radius() as u64,
                            ),
                        })
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            // This callback also runs for other players and background matches.
            // Their empty local list must never erase the live player's allies.
            if let Some(c) = current_champion.as_ref().filter(|_| collect_units) {
                self.movement
                    .observe_hover_units(key, player_id, c.id(), &skill_units);
            }
            if let Some(c) = current_champion.as_ref().filter(|c| c.is_alive()) {
                self.abilities.observe_units(key, c.id(), &skill_units);
            }
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
                                    is_minion: unit.is_minion(),
                                    friendly: false,
                                    in_cc: (0..unit.cc_count().min(64))
                                        .any(|i| unit.cc_at(i).is_some_and(|cc| cc.kind < 10)),
                                    is_tower: unit.is_tower()
                                        || unit.name().as_deref() == Some("nexus"),
                                    body: sprite_picking::entity_body(
                                        unit.name().as_deref(),
                                        unit.is_champion(),
                                        unit.is_minion(),
                                        unit.is_tower(),
                                        unit.radius() as u64,
                                    ),
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
            (
                selected,
                candidate,
                position,
                actor,
                key,
                skill_units,
                origin.kind == 2 && self.timing.owns_worker(key),
            )
        };
        if live_worker {
            self.hud.capture_items(ctx, match_key, &self.logger);
        }
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
        if valid && live_worker {
            let stamp = skill
                .and_then(|input| self.abilities.dispatched_stamp(input.kind))
                .or_else(|| {
                    skill
                        .is_none()
                        .then(|| self.movement.take_command_stamp())
                        .flatten()
                });
            if let (Some(stamp), Some(input)) = (stamp, candidate.as_ref()) {
                self.timing
                    .trace_dispatch(match_key, stamp, input, &self.logger);
            }
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
        bytes: 0,
        rotating: directory.is_some(),
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
    skill_preview::initialize(&logger);
    logger.write(
        "TIMING native_enabled=true wait=after_frame_publication bootstrap=one_frame explicit_start=true full_match=true loading_guard_seconds=15 heartbeat_guard_seconds=2 maximum_frame_lead=1 movement_enabled=true"
    );
    let timing = Arc::new(native_timing::NativeTiming::new(true));
    let movement = Arc::new(movement_test::MovementTest::new(true));
    let camera = Arc::new(camera::CameraControl::default());
    let cast_on_release = runtime_storage::cast_on_release(directory.as_deref());
    let cursor = cursor::Cursor::new(directory.as_deref());
    logging::set(cursor.settings.number("log_level"));
    logger.write(&format!("CONTROLS cast_on_release={cast_on_release}; default quickcast; Shift normal cast; Alt self cast"));
    let abilities = Arc::new(abilities::Abilities::new(cast_on_release));
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
        cursor,
        abilities: abilities.clone(),
        hud: hud.clone(),
        team: team.clone(),
        native_enabled,
        install_attempted: AtomicBool::new(false),
        management_seen: AtomicBool::new(false),
        session_gate: session_gate.clone(),
    });
    declaration.add_item_build_hook(purchase_tracker::BuildObserver(hud.clone()));
    declaration.set_map_customizer(map_path::MapObserver(movement.clone(), logger.clone()));
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
