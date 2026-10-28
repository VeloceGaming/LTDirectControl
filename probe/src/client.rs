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
    pub(crate) state_plate: player_hud::StatePlate,
    pub(crate) targeting: platform_input::ChampionOnlyToggle,
    pub(crate) session_ui: session_ui::SessionUi,
    pub(crate) settings_ui: settings_ui::SettingsUi,
    pub(crate) shop_ui: shop_ui::ShopUi,
    pub(crate) stats_ui: stats_ui::StatsUi,
    pub(crate) result_audit: result_audit::ResultAudit,
    pub(crate) season_fix: season_fix::SeasonFix,
    pub(crate) team_ui: team_status::TeamUi,
    pub(crate) early_input: Option<(u64, platform_input::Keys)>,
    pub(crate) screen_effect: screen_effect::Effect,
    pub(crate) ai_handback: ai_handback::AiHandback,
    pub(crate) emotes: emotes::Emotes,
    pub(crate) sounds: ui_sound::Sounds,
    pub(crate) language: lang::Probe,
    /// Windows built hidden before the match (1.6.0, prewarm.rs).
    pub(crate) prespawn: crate::prewarm::Schedule,
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
    /// Ground ring fades and lock-on across frames.
    pub(crate) rings: Mutex<ground_ring::GroundRings>,
}

impl Drop for Client {
    fn drop(&mut self) {
        ui_state::SETTINGS_OPEN.store(false, Ordering::Relaxed);
        ui_state::EMOTE_CAPTURE.store(false, Ordering::Relaxed);
        self.settings.flush(true, &self.logger);
        self.cursor.shutdown();
        pointer_clip::shutdown();
        wheel::shutdown();
        self.timing
            .cancel("Client extension detached", &self.logger);
    }
}

impl Client {
    /// Unit highlight as ground rings: chosen, or standing in for an outline
    /// that stepped aside for another mod holding the unit renderer.
    /// Ground rings while the match runs; frozen as they were while paused;
    /// forgotten otherwise.
    fn draw_ground_rings(&self, ctx: &mut StableClient<'_>) {
        let Ok(mut rings) = self.rings.lock() else {
            return;
        };
        let paused = self.timing.ui_phase() == Some(native_timing::Phase::Paused);
        if !self.ground_rings() || !(paused || self.timing.client_running()) {
            rings.clear();
            return;
        }
        let Some(frame) = self.camera.frame() else {
            return;
        };
        let marks = if paused {
            rings.paused()
        } else {
            let (hover, attack, click) = self.movement.ring_inputs();
            rings.update(hover, attack, click)
        };
        ground_ring::draw(ctx, frame, &marks);
    }
    fn ground_rings(&self) -> bool {
        movement::ground_rings(
            self.settings.number("hover_outline"),
            native_adapter::outline_installed(),
        )
    }
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
    /// `paused`: the match is paused for this player, so only the plain
    /// left-click (select a unit, clear the selection, close the shop) works.
    fn gameplay_input(&self, keys: platform_input::Keys, active: bool, paused: bool) {
        camera::publish_vision(self.camera.vision());
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
            paused,
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
        self.gameplay_input(keys, true, false);
        if let Ok(mut o) = self.observations.lock() {
            o.early_input = Some((self.timing.generation(), keys));
        }
    }
    fn post_update_inner(&self, ctx: &mut StableClient<'_>, dt_micros: u64) {
        // A new background colour or mod language (applied in Settings, or
        // the game's language changing) rebuilds the windows through their
        // usual missing-node path.
        // A panic while the interface state was locked would otherwise leave
        // every window of the mod frozen for the rest of the session.
        if self.observations.is_poisoned() {
            self.observations.clear_poison();
            static LOGGED: AtomicBool = AtomicBool::new(false);
            if !LOGGED.swap(true, Ordering::Relaxed) {
                self.logger.write(
                    "CLIENT interface state recovered after a panic in the mod's own code; see the PANIC lines (logged once)",
                );
            }
        }
        if let Ok(mut o) = self.observations.lock() {
            o.language.update(ctx, &self.logger);
        }
        if lang::sync().is_some() {
            self.logger
                .write(&format!("LANGUAGE mod text now {}", lang::code()));
        }
        ui_theme::sync();
        ui_theme::refresh_all(
            ctx,
            &[
                player_hud::PATH,
                session_ui::PATH,
                settings_ui::PATH,
                shop_ui::PATH,
                team_status::PATH,
                stats_ui::OWN,
                stats_ui::TARGET,
                emotes::PATH,
            ],
        );
        self.install_once(ctx);
        let scene = ctx.client_scene_kind();
        let battlefield = scene == Some(mod_api_stable::ClientSceneKindV1::InGame);
        pointer_clip::update(
            pointer_clip::wanted(
                settings::current_shared().number("pointer_clip") == 1.,
                battlefield && self.timing.client_running(),
                ui_state::SHOP_OPEN.load(Ordering::Relaxed),
                ui_state::SETTINGS_OPEN.load(Ordering::Relaxed),
            ),
            &self.logger,
        );
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
        self.ensure_available_role(keys);
        self.choose_prepared_athlete(keys);
        test_cheats::update(keys.home_end && battlefield, &self.logger);
        let pull = (keys.home_insert && battlefield)
            .then(|| {
                let frame = self.camera.frame().filter(|f| f.valid())?;
                frame.unproject(keys.cursor?)
            })
            .flatten();
        practice::configure(
            self.settings.hidden_switch("practice_mode"),
            self.settings.text("practice_lineup"),
            pull,
            &self.logger,
        );
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
                if matches!(
                    action,
                    native_timing::SessionAction::Pause | native_timing::SessionAction::Resume
                ) {
                    ui_sound::click();
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
        let snapshot = self.hud.snapshot(identity);
        let artwork = snapshot
            .is_none()
            .then(|| self.hud.artwork(identity))
            .flatten();
        native_adapter::set_selection_draws(controls && battlefield);
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
        let mut hud_bounds = FRAME_RECTS.with(|r| std::mem::take(&mut *r.borrow_mut()));
        hud_bounds.clear();
        let hovered_skill = self.apply_windows(ctx, &mut frame, &mut hud_bounds);
        keys = frame.keys;
        let Frame { snapshot, .. } = frame;
        self.block_native_ui(ctx, &hud_bounds);
        FRAME_RECTS.with(|r| *r.borrow_mut() = hud_bounds);
        if early.is_none() || !gameplay_active {
            let paused = controls
                && battlefield
                && self.timing.phase() == Some(native_timing::Phase::Paused)
                && !ui_state::SETTINGS_OPEN.load(Ordering::Relaxed);
            self.gameplay_input(
                keys,
                gameplay_active && !ui_state::SETTINGS_OPEN.load(Ordering::Relaxed),
                paused,
            );
        }
        self.update_pointer(
            keys,
            battlefield,
            controls,
            gameplay_active,
            snapshot.as_deref(),
            hovered_skill,
        );
        self.log_frame_diagnostics(ctx, dt_micros, battlefield);
    }

    fn post_render_inner(&self, ctx: &mut StableClient<'_>) {
        // The hidden drawing probe (perf_draw_probe): nothing while off.
        crate::draw_probe::draw(ctx, &self.logger);
        // The mod's drawn windows (1.6.0-perf-dev3, draw_list.rs): built in
        // post_update, sent here at the z values their nodes had.
        if let Ok(o) = self.observations.lock() {
            crate::draw_list::submit(
                ctx,
                &[
                    &o.screen_effect.list,
                    &o.hud_ui.list,
                    &o.state_plate.list,
                    &o.stats_ui.list,
                    &o.session_ui.list,
                    &o.team_ui.list,
                ],
            );
        }
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
        if matches!(
            ctx.client_scene_kind(),
            Some(
                mod_api_stable::ClientSceneKindV1::StadiumEntrance
                    | mod_api_stable::ClientSceneKindV1::Lineup
                    | mod_api_stable::ClientSceneKindV1::Main
            )
        ) {
            crate::prewarm::warm_fonts(ctx, &self.logger);
        }
        self.abilities.draw(ctx, &self.camera);
        // A game version whose minimap family is off keeps its own minimap
        // look (0.7.0 Beta 2 for now).
        if native_adapter::minimap_installed() {
            minimap::draw_frame(ctx, &self.camera);
        }
        // motion: S2
        hud_motion_pass::s2_pause_brackets(
            ctx,
            self.timing.phase() == Some(native_timing::Phase::Paused),
        );
        // motion: S7
        let target_panel = (ctx.ui_visible(stats_ui::TARGET) == Some(true))
            .then(|| ctx.ui_node_rect(&format!("{}.panel", stats_ui::TARGET)))
            .flatten();
        hud_motion_pass::s7_target_brackets(ctx, target_panel, stats_panel::target_id());
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
        self.draw_ground_rings(ctx);
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
        if self.settings.number("collision_debug") == 1.
            && matches!(
                self.timing.ui_phase(),
                Some(native_timing::Phase::Running | native_timing::Phase::Paused)
            )
        {
            self.draw_collision_bodies(ctx);
        }
        // The measured input delay, while its setting is on and you play:
        // a number in the top right corner (UI space is 1920 x 1080), just
        // under the game's 50 px header bar, in the mod's numeric font, with
        // a dark shadow instead of a box so it reads on any background.
        if crate::input_delay::shown()
            && matches!(
                self.timing.ui_phase(),
                Some(native_timing::Phase::Running | native_timing::Phase::Paused)
            )
        {
            let text = crate::input_delay::text();
            for (offset, color, z) in [(2., 0x000000c0, 1000), (0., 0xffffffff, 1001)] {
                ctx.draw_text(
                    "UI",
                    &text,
                    "asset/lt_direct_control/font/numeric",
                    (1702. + offset, 56. + offset, 200., 36.),
                    z,
                    28.,
                    color,
                    mod_api_stable::TextAlignXV1::Right,
                    mod_api_stable::TextAlignYV1::Center,
                );
            }
        }
        // The live rating, while its setting is on and you play: in the row
        // under the input delay's place, whether that is shown or not.
        if crate::live_rating::shown()
            && matches!(
                self.timing.ui_phase(),
                Some(native_timing::Phase::Running | native_timing::Phase::Paused)
            )
        {
            crate::live_rating::draw(ctx, self.movement.hud_identity());
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
        // The practice tool changes the real match too: its tag, and the
        // readout of your last shots (English only, like the test tag).
        if practice::active()
            && matches!(
                self.timing.ui_phase(),
                Some(native_timing::Phase::Running | native_timing::Phase::Paused)
            )
        {
            let mut lines = vec!["PRACTICE: enemies hold, Home+Insert lines them up".to_owned()];
            if athlete_rules::in_effect() {
                lines.extend(practice::readout());
            } else {
                lines.push(
                    "Shot readout: needs Athlete stat rules On (installed at the title screen)"
                        .to_owned(),
                );
            }
            ctx.draw_rect(
                "UI",
                18.,
                190.,
                620.,
                8. + 24. * lines.len() as f32,
                1000,
                6.,
                0x1c3a8acc,
            );
            for (i, line) in lines.iter().enumerate() {
                ctx.draw_text(
                    "UI",
                    line,
                    "asset/base/font/set/regular",
                    (28., 194. + 24. * i as f32, 600., 24.),
                    1001,
                    16.,
                    0xffffffff,
                    mod_api_stable::TextAlignXV1::Left,
                    mod_api_stable::TextAlignYV1::Center,
                );
            }
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
        athlete_rules::configure(
            self.settings.number("athlete_stat_rules") == 1.,
            self.settings.number("stat_reaction_floor") as u32,
            self.settings.number("stat_reaction_ceiling") as u32,
            self.settings.number("stat_collision_margin") as u32,
        );
        perf::set_capture(self.settings.number("perf_capture") == 1.);
        crate::ui_root::configure(
            self.settings.hidden_switch("perf_hide_mod_ui"),
            &self.logger,
        );
        crate::draw_probe::configure(self.settings.hidden_switch("perf_draw_probe"), &self.logger);
        self.timing
            .configure_pacing_off(self.settings.hidden_switch("perf_pacing_off"), &self.logger);
        crate::input_delay::set_shown(self.settings.number("show_input_delay") == 1.);
        crate::live_rating::set_shown(self.settings.number("live_rating") == 1.);
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
    snapshot: Option<std::sync::Arc<player_hud::Snapshot>>,
    artwork: Option<player_hud::Artwork>,
    skills: abilities::HudSkills,
    status: String,
}

impl Client {
    /// Install the native hooks once, at the title screen.
    fn install_once(&self, ctx: &mut StableClient<'_>) {
        if self.native_enabled && !self.install_attempted.load(Ordering::Relaxed) {
            match ctx.scene_kind() {
                // The fingerprint is hashed in the background from load; at
                // the title, install on the first frame it is ready.
                Some(mod_api_stable::SceneKindV1::Title) if native_adapter::fingerprint_ready() => {
                    self.install_attempted.store(true, Ordering::Relaxed);
                    // Unit highlight "Ground ring" leaves the unit renderer
                    // to other mods: the outline hook is not installed.
                    let rings = self.settings.number("hover_outline") == 2.;
                    let skip = if rings {
                        crate::native_profile::family::OUTLINE
                    } else {
                        0
                    };
                    match native_adapter::install(skip) {
                        Ok((game, families)) => self.timing.installed(
                            true,
                            &format!(
                                "verified {game} SHA256; hook families {families:#x} installed at title{}",
                                if rings { "; outline hook skipped (Unit highlight: Ground ring)" } else { "" }
                            ),
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
        // The athlete stat rules are patched in only while their setting is
        // On, and only at the title screen (no match runs there).
        if athlete_rules::enabled() && !athlete_rules::in_effect() {
            if ctx.scene_kind() == Some(mod_api_stable::SceneKindV1::Title) {
                if let Some(result) = native_adapter::install_stat_rules() {
                    self.logger.write(&match result {
                        Ok(sites) => format!("STATRULE installed sites={sites} at the title screen; rules apply only to the controlled athlete while Athlete stat rules is on"),
                        Err(e) => format!("STATRULE unavailable: {e}; athlete stat rules off, other hooks unaffected"),
                    });
                }
            } else if ctx.scene_kind() == Some(mod_api_stable::SceneKindV1::InGame)
                && native_adapter::stat_rules_pending()
                && !STAT_WAIT_LOGGED.swap(true, Ordering::Relaxed)
            {
                self.logger.write("STATRULE waiting: Athlete stat rules is On but not installed this session; it is installed at the title screen");
            }
        }
    }
    /// Recover only after the initial roster is complete; never silently auto-play a different role.
    fn ensure_available_role(&self, keys: platform_input::Keys) {
        if self.movement.selected().is_some() || !self.timing.roster_ready() {
            return;
        }
        if !matches!(
            self.timing.ui_phase(),
            Some(native_timing::Phase::Loading | native_timing::Phase::Ai)
        ) {
            return;
        }
        let (Some(key), Some(player)) = (
            self.timing.match_key(),
            self.movement.own_players().into_iter().next(),
        ) else {
            return;
        };
        if let Ok(_gate) = self.session_gate.write() {
            if self
                .movement
                .choose_prepared(key, player.lane, keys, &self.logger)
            {
                self.timing.mark_role_fallback(key);
                self.abilities.reset_session(keys);
                self.camera.reset_for_swap(keys);
                self.hud.select_prepared(key, player.player);
                self.logger.write("SESSION preferred role unavailable; selected an available teammate; automatic play waits for confirmation");
            }
        }
    }
    /// Switch the controlled athlete to a teammate (Ctrl + 1-5 or a
    /// portrait): before Start, and during the match while playing, paused or
    /// with the AI in control. The session gate keeps a simulation step from
    /// running across the change; the previous champion is the game AI's
    /// again from its next decision.
    fn choose_prepared_athlete(&self, keys: platform_input::Keys) {
        let prepared_choice = self.observations.lock().ok().and_then(|o| {
            o.session_ui.take_choice(
                self.timing.phase(),
                self.timing.match_key(),
                self.timing.generation(),
            )
        });
        let phase = self.timing.phase();
        // Not with the shop or settings window open: the shop shows, and
        // would queue against, the previous champion's inventory.
        let window = phase != Some(native_timing::Phase::Ready)
            && (ui_state::SHOP_OPEN.load(Ordering::Relaxed)
                || ui_state::SETTINGS_OPEN.load(Ordering::Relaxed));
        if session_ui::swappable(phase) && !window {
            if let (Some(key), Some(lane)) =
                (self.timing.match_key(), prepared_choice.or(keys.selection))
            {
                if let Ok(_gate) = self.session_gate.write() {
                    if self.movement.choose_prepared(key, lane, keys, &self.logger) {
                        ui_sound::click();
                        // motion: B8
                        hud_motion_pass::b8_swap_landed();
                        if phase == Some(native_timing::Phase::Ready) {
                            crate::settings::set_option("preferred_role", lane as f64);
                        }
                        self.abilities.reset_session(keys);
                        self.camera.reset_for_swap(keys);
                        if phase != Some(native_timing::Phase::Ready) {
                            // Mid-match: nothing queued or sampled for the
                            // previous champion carries over.
                            self.movement.clear_commands();
                            self.abilities.clear_commands();
                            shop::SHOP.clear_queue();
                            stats_panel::reset_session();
                            if let Ok(o) = self.observations.lock() {
                                o.session_ui.close_swap();
                            }
                        }
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
                if save_exit {
                    self.movement.forget_team();
                }
                self.abilities.reset_session(keys);
                self.hud.reset_session();
                self.team.reset_session();
                stats_panel::reset_session();
                shop::SHOP.reset_session();
                self.camera.reset_session(keys);
                native_adapter::reset_session();
                if let Ok(mut observations) = self.observations.lock() {
                    observations.targeting = Default::default();
                    observations.hud_ui = Default::default();
                    observations.state_plate = Default::default();
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
        hud_bounds: &mut Vec<camera::Rect>,
    ) -> Option<usize> {
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
                snapshot.as_deref(),
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
            // motion: A2
            write_recall_progress(
                ctx,
                &mut observations.hud_ui,
                self.movement.recalling(),
                controls && snapshot.as_ref().is_some_and(|s| s.alive),
            );
            // The state plate under the game's clock (paused, dead, AI). The
            // worker keeps sampling in AI control, and the frame's snapshot
            // keeps the last values when samples stop; the plate checks
            // freshness in AI control so a stale dead state cannot stick.
            let phase = self.timing.phase();
            let plate_snapshot = if phase == Some(native_timing::Phase::Ai) {
                self.hud.fresh_snapshot(self.movement.hud_identity())
            } else {
                snapshot.clone()
            };
            observations
                .state_plate
                .apply(ctx, phase, plate_snapshot.as_deref(), &self.logger);
            observations.hud_ui.bounds(ctx, hud_bounds);
            observations.stats_ui.apply(
                ctx,
                battlefield,
                controls,
                keys.stats_panel && keys.focused && !keys.emote_capture,
                &self.logger,
                hud_bounds,
            );
            drop(t);
            let t = perf::time(perf::Section::Team);
            {
                let ClientObservations {
                    team_ui, hud_ui, ..
                } = &mut *observations;
                let team_active = controls && keys.focused && team_open;
                let roster = if team_active {
                    self.team.snapshot(self.timing.match_key())
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
            // One read of your team's players for the strip and the audit.
            let own_players = self.movement.own_players();
            observations.session_ui.apply(
                ctx,
                &self.timing,
                &own_players,
                self.movement.selected(),
                &self.camera,
                &self.settings,
                keys.cursor.filter(|_| keys.focused),
                keys.left && keys.focused && !keys.emote_capture,
                &self.logger,
                hud_bounds,
            );
            if keys.raw.0[0x1b] {
                observations.session_ui.close_swap();
            }
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
                // Caught here so one failing frame closes the shop instead of
                // freezing it on screen with the rest of the interface.
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    shop_ui.apply(ctx, keys, shop_active, &self.timing, hud_ui, &self.logger)
                })) {
                    Ok(bounds) => hud_bounds.extend(bounds),
                    Err(_) => shop_ui.recover(ctx, &self.timing, &self.logger),
                }
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
                snapshot.as_deref(),
                dt_micros,
            );
            observations.screen_effect.apply(&self.camera, &self.logger);
            observations.sounds.flush(
                ctx,
                ui_sound::Config::from_values(&settings::current_shared()),
                snapshot.as_ref().is_some_and(|s| s.alive),
                &self.logger,
            );
            drop(t);
            let _t = perf::time(perf::Section::Audit);
            let last_player = self.hud.snapshot(self.movement.hud_identity());
            let own_athletes = own_players.iter().map(|p| p.athlete).collect::<Vec<_>>();
            observations
                .season_fix
                .update(ctx, self.timing.match_key(), battlefield, &self.logger);
            observations.result_audit.update(
                ctx,
                self.timing.match_key(),
                self.timing.generation(),
                battlefield,
                last_player.as_deref(),
                &own_athletes,
                &self.logger,
            );
            drop(_t);
            // While you choose your athlete: windows built ahead of their
            // first use (prewarm.rs). After the HUD strip exists, which it
            // does in Ready (`phase` is None while Loading).
            if battlefield
                && self.timing.phase() == Some(native_timing::Phase::Ready)
                && ctx.ui_exists(player_hud::PATH)
            {
                let ClientObservations {
                    prespawn,
                    stats_ui,
                    state_plate,
                    team_ui,
                    shop_ui,
                    settings_ui,
                    emotes,
                    ..
                } = &mut *observations;
                prespawn.step(
                    ctx,
                    &mut [stats_ui, state_plate, team_ui, shop_ui, settings_ui, emotes],
                    &self.logger,
                );
            }
        }
        hovered_skill
    }
    /// Tell the camera which screen areas are UI, so clicks there are not orders.
    fn block_native_ui(&self, ctx: &StableClient<'_>, hud_bounds: &[camera::Rect]) {
        BLOCKED.with(|lists| self.block_native_ui_into(ctx, hud_bounds, &mut lists.borrow_mut()));
    }
    fn block_native_ui_into(
        &self,
        ctx: &StableClient<'_>,
        hud_bounds: &[camera::Rect],
        (blocked, command_blocked): &mut (Vec<camera::Rect>, Vec<camera::Rect>),
    ) {
        // Read native UI geometry, respecting ancestor visibility. The native
        // renderer's minimap is separately excluded by CameraFrame.
        blocked.clear();
        blocked.extend_from_slice(hud_bounds);
        command_blocked.clear();
        command_blocked.extend_from_slice(hud_bounds);
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
        snapshot: Option<&player_hud::Snapshot>,
        hovered_skill: Option<usize>,
    ) {
        self.abilities.set_hud_hover(hovered_skill);
        store_cursor(keys.cursor);
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
    /// Testing overlay for the stat rules: around each enemy near the cursor,
    /// the body's collision radius (white) and the radius plus the fixed hit
    /// margin (yellow), as circles on the ground at the unit's position. The
    /// radius is the one the game's hit tests use (the SDK's `radius()`), not
    /// the selection code's measured art.
    fn draw_collision_bodies(&self, ctx: &mut StableClient<'_>) {
        const NEAR_PX: f32 = 260.;
        const SEGMENTS: usize = 48;
        let Some(frame) = self.camera.frame().filter(|f| f.valid()) else {
            return;
        };
        let Some(cursor) = cursor_point() else {
            return;
        };
        let margin = athlete_rules::margin_units() as f32;
        for unit in self.movement.hover_units() {
            if unit.friendly {
                continue;
            }
            let (x, y) = (
                unit.position.0 as f32 / 1000.,
                unit.position.1 as f32 / 1000.,
            );
            let centre = frame.project_unclipped(x, y);
            if (centre.0 - cursor.0).hypot(centre.1 - cursor.1) > NEAR_PX {
                continue;
            }
            let body = unit.radius as f32 / 1000.;
            for (radius, width, color) in
                [(body, 1.5, 0xffffffdd), (body + margin, 1.5, 0xfdee00ee)]
            {
                let point = |i: usize| {
                    let theta = i as f32 * std::f32::consts::TAU / SEGMENTS as f32;
                    frame.project_unclipped(x + theta.cos() * radius, y + theta.sin() * radius)
                };
                let mut last = point(0);
                for i in 1..=SEGMENTS {
                    let next = point(i);
                    if [last, next]
                        .into_iter()
                        .all(|p| frame.viewport.contains(p) && !frame.minimap.contains(p))
                    {
                        ctx.draw_line("UI", last.0, last.1, next.0, next.1, width, 989, color);
                    }
                    last = next;
                }
            }
        }
    }
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

/// The "waiting for the title screen" line of the stat rules was logged.
static STAT_WAIT_LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The last cursor position the pointer step saw (screen pixels), kept as
/// two f32 bit patterns so the render step can read it without a lock.
static CURSOR_BITS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(u64::MAX);
// motion: A2
/// The recall bar under the HUD strip (written after the strip, every frame).
pub(crate) fn write_recall_progress(
    ctx: &mut StableClient<'_>,
    hud_ui: &mut player_hud::HudUi,
    recalling: bool,
    shown: bool,
) {
    let a2 = hud_motion_pass::a2_recall_progress(&mut hud_ui.pass, recalling, shown);
    hud_ui.write_recall(ctx, a2);
}
thread_local! {
    /// The client thread's rectangle lists, reused every frame: the UI's
    /// own areas, then (blocked, command_blocked) for the camera.
    static FRAME_RECTS: std::cell::RefCell<Vec<camera::Rect>> = const { std::cell::RefCell::new(Vec::new()) };
    static BLOCKED: std::cell::RefCell<(Vec<camera::Rect>, Vec<camera::Rect>)> =
        const { std::cell::RefCell::new((Vec::new(), Vec::new())) };
}
fn store_cursor(cursor: Option<(f32, f32)>) {
    let bits = cursor.map_or(u64::MAX, |(x, y)| {
        u64::from(x.to_bits()) << 32 | u64::from(y.to_bits())
    });
    CURSOR_BITS.store(bits, Ordering::Relaxed);
}
fn cursor_point() -> Option<(f32, f32)> {
    let bits = CURSOR_BITS.load(Ordering::Relaxed);
    (bits != u64::MAX).then(|| {
        (
            f32::from_bits((bits >> 32) as u32),
            f32::from_bits(bits as u32),
        )
    })
}
