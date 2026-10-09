//! The SDK client extension: runs on the client thread every frame (input,
//! session controls, HUD, Tab, settings, shop, cursor and battlefield drawing).
use super::*;

#[derive(Default)]
pub(crate) struct ClientObservations {
    pub(crate) elapsed_micros: u64,
    pub(crate) last_sample_micros: u64,
    pub(crate) scene: String,
    pub(crate) timer: Option<String>,
    pub(crate) playback_states: String,
    pub(crate) info: team_info::TeamInfo,
    pub(crate) hud_ui: player_hud::HudUi,
    pub(crate) targeting: platform_input::ChampionOnlyToggle,
    pub(crate) session_ui: session_ui::SessionUi,
    pub(crate) settings_ui: settings_ui::SettingsUi,
    pub(crate) shop_ui: shop_ui::ShopUi,
    pub(crate) stats_ui: stats_ui::StatsUi,
    pub(crate) result_audit: result_audit::ResultAudit,
    pub(crate) team_ui: team_status::TeamUi,
    pub(crate) early_input: Option<(u64, platform_input::Keys)>,
    pub(crate) screen_effect: screen_effect::Effect,
    pub(crate) ai_handback: ai_handback::AiHandback,
    pub(crate) emotes: emotes::Emotes,
}

pub(crate) struct Client {
    pub(crate) logger: Arc<Logger>,
    pub(crate) observations: Mutex<ClientObservations>,
    pub(crate) timing: Arc<native_timing::NativeTiming>,
    pub(crate) movement: Arc<movement::Movement>,
    pub(crate) camera: Arc<camera::CameraControl>,
    pub(crate) cursor: cursor::Cursor,
    pub(crate) settings: Arc<settings::Settings>,
    pub(crate) abilities: Arc<abilities::Abilities>,
    pub(crate) hud: Arc<player_hud::PlayerHud>,
    pub(crate) team: Arc<team_status::TeamStatus>,
    pub(crate) native_enabled: bool,
    pub(crate) install_attempted: AtomicBool,
    pub(crate) management_seen: AtomicBool,
    pub(crate) session_gate: Arc<RwLock<()>>,
}

impl Drop for Client {
    fn drop(&mut self) {
        ui_state::SETTINGS_OPEN.store(false, Ordering::Relaxed);
        ui_state::EMOTE_CAPTURE.store(false, Ordering::Relaxed);
        self.settings.flush(true, &self.logger);
        self.cursor.shutdown();
        wheel::shutdown();
        self.timing
            .cancel("Client extension detached", &self.logger);
    }
}

impl Client {
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

impl Client {
    fn emote_context(
        &self,
        keys: platform_input::Keys,
        battlefield: bool,
        running: bool,
    ) -> emotes::Context {
        let anchor = self.movement.cosmetic_anchor();
        let identity = anchor.map(|a| emotes::Identity {
            generation: self.timing.generation(),
            key: a.key,
            player: a.player,
        });
        let alive = anchor.is_some_and(|a| a.position.is_some());
        emotes::Context {
            identity,
            tick: self.timing.presentation_tick(),
            controlling: battlefield && self.timing.client_controls(None),
            running,
            alive,
            enabled: self.settings.number("emotes") == 1.,
            cooldown_ticks: emotes::Config::from_values(&settings::current_shared()).cooldown_ticks,
            blocked: ui_state::SETTINGS_OPEN.load(Ordering::Relaxed)
                || ui_state::SHOP_OPEN.load(Ordering::Relaxed),
            aiming: self.abilities.interaction_busy() || self.movement.attack_move_armed(),
            may_open: keys.emote
                && keys.cursor.is_some_and(|p| {
                    !self.camera.command_blocked(p)
                        && self
                            .camera
                            .frame()
                            .is_some_and(|f| f.unproject(p).is_some())
                }),
        }
    }
    fn emote_input(&self, keys: &mut platform_input::Keys, battlefield: bool, running: bool) {
        // Idle pre_update needs only the already-polled flags. Context and
        // lifecycle are refreshed once in the normal UI step below.
        if !keys.emote
            && !keys.emote_capture
            && self
                .observations
                .lock()
                .is_ok_and(|o| !o.emotes.input_pending())
        {
            return;
        }
        let context = self.emote_context(*keys, battlefield, running);
        if let Ok(mut o) = self.observations.lock() {
            o.emotes.input(keys, context);
        }
    }
    fn pre_update_inner(&self, ctx: &mut StableClient<'_>) {
        if ctx.client_scene_kind() != Some(mod_api_stable::ClientSceneKindV1::InGame)
            || ui_state::SETTINGS_OPEN.load(Ordering::Relaxed)
            || !self.timing.client_early_input()
        {
            return;
        }
        // Use the last rendered camera and visible UI masks. These describe
        // what the user clicked, before the host advances camera/playback.
        let mut keys = platform_input::poll();
        self.emote_input(&mut keys, true, self.timing.client_running());
        self.resolve_targeting(&mut keys, true);
        self.movement.update(keys, true, false, &self.logger);
        self.gameplay_input(keys, true);
        if let Ok(mut o) = self.observations.lock() {
            o.early_input = Some((self.timing.generation(), keys));
        }
    }
    fn post_update_inner(&self, ctx: &mut StableClient<'_>, dt_micros: u64) {
        // A new background colour (applied in Settings) rebuilds the windows
        // through their usual missing-node path.
        ui_theme::sync();
        for path in [
            player_hud::PATH,
            session_ui::PATH,
            settings_ui::PATH,
            shop_ui::PATH,
            team_status::PATH,
            stats_ui::OWN,
            stats_ui::TARGET,
            emotes::PATH,
        ] {
            ui_theme::refresh(ctx, path);
        }
        self.install_once(ctx);
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
        acquisition::refresh(ctx, self.timing.generation());
        self.choose_prepared_athlete(keys);
        test_cheats::update(keys.home_end && battlefield, &self.logger);
        self.timing.heartbeat(
            battlefield,
            self.movement.selected().is_some(),
            keys,
            &self.logger,
        );
        self.rearm_if_needed(ctx, keys, pre_match, save_exit);
        if let Ok(mut o) = self.observations.lock() {
            o.ai_handback.update(ctx, &self.timing, keys, &self.logger);
        }
        let session_action = self.timing.take_action();
        if let Some(action) = session_action {
            // Exclude an in-flight SDK think while changing input ownership.
            // Publication waits happen outside this gate.
            if let Ok(_gate) = self.session_gate.write() {
                self.movement.clear_commands();
                self.abilities.clear_commands();
                if matches!(
                    action,
                    native_timing::SessionAction::ReturnAi
                        | native_timing::SessionAction::TakeControl
                ) {
                    self.camera.suspend(keys);
                }
                self.timing.apply_action(action, &self.logger);
            }
        }
        wheel::update(
            self.timing.client_controls(None),
            &self.camera,
            &self.timing,
            &self.logger,
        );
        let running = self.timing.client_running();
        let gameplay_active =
            running && session_action.is_none() && !ui_state::SETTINGS_OPEN.load(Ordering::Relaxed);
        let controls = self.timing.client_controls(None);
        native_tooltips::observe(self.timing.generation(), controls && battlefield);
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
        let mut frame = Frame {
            keys,
            battlefield,
            controls,
            running,
            dt_micros,
            snapshot,
            artwork,
            skills,
            status,
        };
        let (hud_bounds, hovered_skill) = self.apply_windows(ctx, &mut frame);
        keys = frame.keys;
        let Frame { snapshot, .. } = frame;
        self.block_native_ui(ctx, hud_bounds);
        if early.is_none() || !gameplay_active {
            self.gameplay_input(
                keys,
                gameplay_active && !ui_state::SETTINGS_OPEN.load(Ordering::Relaxed),
            );
        }
        self.update_pointer(
            keys,
            battlefield,
            controls,
            gameplay_active,
            &snapshot,
            hovered_skill,
        );
        self.log_frame_diagnostics(ctx, dt_micros, battlefield);
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
                self.settings.number("selection_debug") == 1.,
            );
            if self.settings.number("selection_debug") == 1. {
                self.draw_selection_areas(ctx);
            }
            if self.settings.number("map_path") == 1. {
                self.movement.draw_minimap_path(ctx, &self.camera);
            }
            cursor::draw_clicks(ctx, &self.camera, &self.movement.click_feedback());
        }
        if self.settings.number("acquisition_debug") == 1.
            && matches!(
                self.timing.ui_phase(),
                Some(native_timing::Phase::Running | native_timing::Phase::Paused)
            )
        {
            self.movement.draw_acquisition_debug(
                ctx,
                &self.camera,
                self.timing.ui_phase() == Some(native_timing::Phase::Paused),
            );
        }
        // The no-cooldown test option changes the real match: always shown.
        if test_cheats::active() {
            ctx.draw_rect("UI", 18., 150., 260., 32., 1000, 6., 0x8a1c1cff);
            ctx.draw_text(
                "UI",
                "TEST: no cooldowns (Home+End)",
                "asset/base/font/set/regular",
                (28., 154., 240., 24.),
                1001,
                16.,
                0xffffffff,
                mod_api_stable::TextAlignXV1::Left,
                mod_api_stable::TextAlignYV1::Center,
            );
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

impl StableExtension for Client {
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
        logging::set(self.settings.number("log_level"));
        perf::set_capture(self.settings.number("perf_capture") == 1.);
        self.post_update_inner(ctx, dt_micros);
    }
    fn post_render(&self, ctx: &mut StableClient<'_>) {
        let _t = perf::time(perf::Section::Render);
        self.post_render_inner(ctx);
    }
}

/// The parts of one client frame that the UI step needs.
struct Frame {
    keys: platform_input::Keys,
    battlefield: bool,
    controls: bool,
    running: bool,
    dt_micros: u64,
    snapshot: Option<player_hud::Snapshot>,
    artwork: Option<player_hud::Artwork>,
    skills: abilities::HudSkills,
    status: String,
}

impl Client {
    /// Install the native hooks once, at the title screen.
    fn install_once(&self, ctx: &mut StableClient<'_>) {
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
    }
    /// Before Start: switch the controlled athlete (Ctrl + 1-5 or a portrait).
    fn choose_prepared_athlete(&self, keys: platform_input::Keys) {
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
    }
    /// A new match or a save exit: reset every per-session consumer.
    fn rearm_if_needed(
        &self,
        ctx: &mut StableClient<'_>,
        keys: platform_input::Keys,
        pre_match: bool,
        save_exit: bool,
    ) {
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
    }
    /// All native UI this mod owns: Tab, HUD strip, session buttons, settings,
    /// shop, screen effect and the result audit. Returns the screen areas they
    /// cover (clicks there are not battlefield orders) and the hovered HUD skill.
    fn apply_windows(
        &self,
        ctx: &mut StableClient<'_>,
        frame: &mut Frame,
    ) -> (Vec<camera::Rect>, Option<usize>) {
        let Frame {
            keys,
            battlefield,
            controls,
            running,
            dt_micros,
            ref snapshot,
            ref artwork,
            skills,
            ref status,
        } = *frame;
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
                status,
                keys.cursor.filter(|_| keys.focused),
                keys.left && keys.focused && !keys.emote_capture,
                &self.hud,
                &self.logger,
            );
            if controls
                && keys.focused
                && !keys.emote_capture
                && snapshot.as_ref().is_some_and(|s| s.alive)
            {
                hovered_skill = observations.hud_ui.hovered_skill(ctx, keys.cursor);
            }
            hud_bounds = observations.hud_ui.bounds(ctx);
            hud_bounds.extend(observations.stats_ui.apply(
                ctx,
                battlefield,
                controls,
                keys.stats_panel && keys.focused && !keys.emote_capture,
                &self.logger,
            ));
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
                &self.settings,
                keys.cursor.filter(|_| keys.focused),
                keys.left && keys.focused && !keys.emote_capture,
                &self.logger,
            ));
            if observations.session_ui.take_settings() {
                observations
                    .settings_ui
                    .open(&self.timing, &self.settings, keys, &self.logger);
                self.movement.clear_commands();
                self.abilities.clear_commands();
            }
            drop(t);
            let t = perf::time(perf::Section::Settings);
            let camera_lock_before = self.settings.number("camera_lock");
            hud_bounds.extend(observations.settings_ui.apply(
                ctx,
                &self.timing,
                &self.settings,
                keys,
                &self.logger,
            ));
            let camera_lock_after = self.settings.number("camera_lock");
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
            let mut emote_keys = keys;
            let mut emote_context = self.emote_context(keys, battlefield, running);
            emote_context.alive &= snapshot.as_ref().is_none_or(|s| s.alive);
            observations.emotes.input(&mut emote_keys, emote_context);
            frame.keys.emote_capture = emote_keys.emote_capture;
            observations.emotes.apply(
                ctx,
                &self.camera,
                self.movement.cosmetic_anchor().and_then(|a| a.position),
                controls
                    && battlefield
                    && keys.focused
                    && !ui_state::SETTINGS_OPEN.load(Ordering::Relaxed)
                    && !ui_state::SHOP_OPEN.load(Ordering::Relaxed),
                emotes::Config::from_values(&settings::current_shared()),
                &self.logger,
            );
            observations.screen_effect.update(
                self.settings.low_health(),
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
        (hud_bounds, hovered_skill)
    }
    /// Tell the camera which screen areas are UI, so clicks there are not orders.
    fn block_native_ui(&self, ctx: &StableClient<'_>, hud_bounds: Vec<camera::Rect>) {
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
    }
    /// Hover target, sprite outlines and the cursor shape.
    fn update_pointer(
        &self,
        keys: platform_input::Keys,
        battlefield: bool,
        controls: bool,
        gameplay_active: bool,
        snapshot: &Option<player_hud::Snapshot>,
        hovered_skill: Option<usize>,
    ) {
        self.abilities.set_hud_hover(hovered_skill);
        let hover_timer = perf::time(perf::Section::Hover);
        let (hover, attack) = self.movement.refresh_hover(
            &self.camera,
            self.native_enabled
                && gameplay_active
                && battlefield
                && keys.focused
                && !keys.emote_capture,
            self.abilities.hover_target(keys.alt),
        );
        let outline_enabled = self.settings.number("hover_outline") == 1.;
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
        // Debounced save of changed settings, once per frame.
        self.settings.flush(false, &self.logger);
        self.cursor.update(cursor_style, keys.cursor, &self.logger);
        drop(cursor_timer);
    }
    /// Once a second: CONTROL DIAGNOSTIC, native traffic, timer, playback and
    /// scene-change lines (most are Verbose; see logging.rs).
    fn log_frame_diagnostics(&self, ctx: &StableClient<'_>, dt_micros: u64, battlefield: bool) {
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
        self.logger.verbose(|| {
            format!(
                "CONTROL DIAGNOSTIC {} | {:?} | {:?} | {} | {:?} | attack_aim={}",
                self.timing.describe(),
                self.movement.describe(),
                self.abilities.status(),
                self.camera.describe(),
                self.movement.notice(),
                self.movement.attack_move_armed()
            )
        });
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
}

impl Client {
    /// Show selection markers: each unit's selection area. Green: the body
    /// measured from its art and placed from the game's draw data; grey:
    /// the fallback envelope.
    fn draw_selection_areas(&self, ctx: &mut StableClient<'_>) {
        let Some(frame) = self.camera.frame() else {
            return;
        };
        let mut drawing = skill_preview::Drawing::default();
        for unit in self.movement.hover_units() {
            let (x0, y0, x1, y1, _) = combat::screen_area(frame, &unit);
            let color = if combat::body_world_rect(&unit).is_some() {
                0x5cff7aff
            } else {
                0x999999cc
            };
            let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
            for i in 0..4 {
                drawing.line(corners[i], corners[(i + 1) % 4], 1.5, color, 1005);
            }
        }
        let mut blockers = self.camera.overlay_blockers();
        blockers.push(frame.minimap);
        drawing.render(ctx, frame, &blockers);
    }
}
