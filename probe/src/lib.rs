//! LT Direct Control: League-style direct control for Teamfight Manager 2.
//!
//! Two entry points are registered with the game's mod SDK:
//! - [`Client`]: the client extension. Each frame it reads input, drives the
//!   session (Start/Pause/Return to AI), the HUD, Tab, settings, shop and
//!   cursor, and draws battlefield feedback. Runs on the client thread.
//! - [`Simulation`]: the per-player AI callback. On the viewed match's
//!   simulation worker it binds the session, samples HUD/roster data, answers
//!   the shop and returns the controlled athlete's input.
//!
//! Native game hooks (0.6.3 only) live in `native_adapter`; the session and
//! frame pacing between the two threads in `native_timing`.

mod abilities;
mod attack_trace;
mod camera;
mod client;
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
mod movement;
mod native_adapter;
mod native_items;
mod native_preview;
#[cfg(all(windows, target_arch = "x86_64"))]
mod native_profile;
mod native_timing;
mod native_tooltips;
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
mod shop_recipe;
mod shop_trace;
mod shop_ui;
mod simulation;
mod skill_preview;
mod sprite_art;
mod sprite_picking;
mod team_info;
mod team_status;
mod test_cheats;
#[cfg(test)]
mod test_support;
mod tooltip_layout;
mod tooltips;
mod ui_graphics;
mod ui_state;
mod wheel;

use client::Client;
use mod_api_stable::{
    declare_stable_mod, InputV1, LogLevel, StableAiContext, StableAiInit, StableClient,
    StableExtension, StableHost, StableMod, StablePlayerAi,
};
use simulation::Simulation;
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
    let movement = Arc::new(movement::Movement::new(true));
    let camera = Arc::new(camera::CameraControl::default());
    let cast_on_release = runtime_storage::cast_on_release(directory.as_deref());
    let settings = Arc::new(settings::Settings::new(directory.as_deref()));
    let _ = settings::GLOBAL.set(settings.clone());
    let cursor = cursor::Cursor::new(settings.clone());
    logging::set(settings.number("log_level"));
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
    declaration.set_extension(Client {
        logger: logger.clone(),
        observations: Mutex::default(),
        timing: timing.clone(),
        movement: movement.clone(),
        camera,
        cursor,
        settings,
        abilities: abilities.clone(),
        hud: hud.clone(),
        team: team.clone(),
        native_enabled,
        install_attempted: AtomicBool::new(false),
        management_seen: AtomicBool::new(false),
        session_gate: session_gate.clone(),
    });
    declaration.set_match_hook(test_cheats::CooldownHook {
        movement: movement.clone(),
        logger: logger.clone(),
        applied: Mutex::default(),
        cooldowns: Mutex::default(),
    });
    declaration.add_item_build_hook(purchase_tracker::BuildObserver(hud.clone()));
    declaration.set_map_customizer(map_path::MapObserver(movement.clone(), logger.clone()));
    declaration.add_player_input_ai(Simulation {
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
