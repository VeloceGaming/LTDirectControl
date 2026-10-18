//! Experimental adapter for the fingerprinted executables listed at the end
//! of this file, not a stable API. The hook code (windows/) is built once
//! per supported game version, against that version's tables (v*/); the one
//! whose fingerprint the running executable has is installed and answers
//! the calls below, the others stay inert.
//! Eleven decoded CALLs and one movement-consumer tail JMP are redirected; originals stay
//! intact. Installing is permitted only at the title screen, before a viewer
//! or its normal worker exists. No patch is made to the executable on disk.
use crate::{
    native_timing::{NativeTiming, ViewMode},
    Logger,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

struct Shared {
    timing: Arc<NativeTiming>,
    logger: Arc<Logger>,
    movement: Arc<crate::movement::Movement>,
    camera: Arc<crate::camera::CameraControl>,
    abilities: Arc<crate::abilities::Abilities>,
}
static SHARED: OnceLock<Shared> = OnceLock::new();
static WORKER_ENTRIES: AtomicUsize = AtomicUsize::new(0);
static VIEW_ENTRIES: AtomicUsize = AtomicUsize::new(0);
static INPUT_ENTRIES: AtomicUsize = AtomicUsize::new(0);
static STEER_ENTRIES: AtomicUsize = AtomicUsize::new(0);
static OWNED_STEER_ENTRIES: AtomicUsize = AtomicUsize::new(0);
static DIRECT_STEPS: AtomicUsize = AtomicUsize::new(0);
static STEER_POINTER_REJECTIONS: AtomicUsize = AtomicUsize::new(0);
/// The outline hook records each unit's drawn sprites only while selection
/// can use them: you control an athlete on the battlefield.
static SELECTION_DRAWS: AtomicBool = AtomicBool::new(false);
/// The game version whose hooks answer: 0 until `install` recognises the
/// executable, then that version's number in `hooks!`.
#[cfg(all(windows, target_arch = "x86_64"))]
static ACTIVE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
/// `hooks!(default, call)`: `call` in the hooks of the running game
/// version, or `default` while no supported executable is recognised. (The
/// stack helpers name no game address; their default is the first copy.)
#[cfg(all(windows, target_arch = "x86_64"))]
macro_rules! hooks {
    ($default:expr, $($call:tt)+) => {
        match ACTIVE.load(Ordering::Acquire) {
            1 => v070::windows::$($call)+,
            2 => v063::windows::$($call)+,
            3 => v064::windows::$($call)+,
            _ => $default,
        }
    };
}
pub(crate) fn asset_digest(bytes: &[u8]) -> Result<String, String> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        host::sha256(bytes)
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let _ = bytes;
        Err("Image checksums require the supported Windows host".into())
    }
}
#[cfg(all(windows, target_arch = "x86_64"))]
/// The executable's base and the addresses the item catalogue reader
/// checks, once the shop family (which that reader belongs to) is installed.
pub(crate) fn item_reader() -> Option<(usize, crate::native_items::Addresses)> {
    hooks!(None, item_reader())
}
/// Client, once per frame: whether selection reads drawn sprites (you
/// control an athlete on the battlefield). While false, the outline hook
/// records nothing (docs/investigation-takeover-cost-2026-10-18.md, 3.6).
pub fn set_selection_draws(active: bool) {
    SELECTION_DRAWS.store(active, Ordering::Release);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutlineRole {
    Click,
    Hover,
    Attack,
}
impl OutlineRole {
    fn offset(self) -> f32 {
        match self {
            Self::Click => 3.,
            Self::Hover => 1.2,
            Self::Attack => 2. / 3.,
        }
    }
}
#[derive(Default, Clone, Copy)]
struct OutlineTargets {
    hover: Option<crate::combat::Unit>,
    attack: Option<crate::combat::Unit>,
    click: Option<(usize, std::time::Instant)>,
}
impl OutlineTargets {
    fn for_unit(
        self,
        id: usize,
        now: std::time::Instant,
    ) -> Option<(crate::combat::Unit, OutlineRole, f32)> {
        let (unit, role) = self
            .hover
            .filter(|u| u.id == id)
            .map(|u| (u, OutlineRole::Hover))
            .or_else(|| {
                self.attack
                    .filter(|u| u.id == id && !u.friendly)
                    .map(|u| (u, OutlineRole::Attack))
            })?;
        let base = role.offset();
        if let Some(age) = self
            .click
            .filter(|(clicked, _)| *clicked == id)
            .filter(|_| self.attack.is_some_and(|u| u.id == id && !u.friendly))
            .and_then(|(_, at)| now.checked_duration_since(at))
            .filter(|age| *age < crate::movement::ATTACK_CLICK_DURATION)
        {
            let remaining =
                1. - age.as_secs_f32() / crate::movement::ATTACK_CLICK_DURATION.as_secs_f32();
            return Some((
                unit,
                OutlineRole::Click,
                base + (OutlineRole::Click.offset() - base) * remaining,
            ));
        }
        Some((unit, role, base))
    }
}
pub fn set_outline_targets(
    hover: Option<crate::combat::Unit>,
    attack: Option<crate::combat::Unit>,
    click: Option<(usize, std::time::Instant)>,
) {
    #[cfg(all(windows, target_arch = "x86_64"))]
    hooks!(
        (),
        set_outline_targets(OutlineTargets {
            hover,
            attack,
            click,
        })
    );
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    let _ = (hover, attack, click);
}

/// Current AA range, copied only during the controlled worker's SDK borrow.
pub fn attack_ranges(
    sim: &mod_api_stable::StableSim<'_>,
    actor: usize,
) -> Option<(u64, Option<u64>)> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    unsafe {
        sim.with_native_context(|state, table| {
            hooks!(None, attack_ranges(state as usize, table as usize, actor))
        })
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let _ = (sim, actor);
        None
    }
}

/// Copy the final native build, after all item-build overrides have run.
/// Borrowed SDK/game pointers are consumed here and never retained.
pub fn player_build(sim: &mod_api_stable::StableSim<'_>, id: usize) -> Option<Vec<usize>> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    unsafe {
        sim.with_native_context(|state, table| {
            hooks!(None, player_build(state as usize, table as usize, id))
        })
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let _ = (sim, id);
        None
    }
}

/// Native owned-item Vec length (+0x318), read-only, for the shop trace.
pub fn player_owned_len(sim: &mod_api_stable::StableSim<'_>, id: usize) -> Option<usize> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    unsafe {
        sim.with_native_context(|state, table| {
            hooks!(None, player_owned_len(state as usize, table as usize, id))
        })
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let _ = (sim, id);
        None
    }
}

/// Native player object for the shop hooks' identity check, read-only.
pub fn native_player(sim: &mod_api_stable::StableSim<'_>, id: usize) -> Option<usize> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    unsafe {
        sim.with_native_context(|state, table| {
            hooks!(None, native_player(state as usize, table as usize, id))
        })
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let _ = (sim, id);
        None
    }
}

/// Item slots the game allows, read from the purchase executor's own gate
/// (`cmp rax, N; ja` at 0x146baf3 allows N + 1 items). Vanilla has 3 (four
/// slots); item mods such as the Riot pack patch it (5: six slots). Read-only.
pub fn item_slot_capacity() -> Option<usize> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        hooks!(None, item_slot_capacity())
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        None
    }
}

/// True when both buyer hooks are installed and Manual shopping can apply.
pub fn shop_ready() -> bool {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        hooks!(false, shop_ready())
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        false
    }
}

/// Compares the live native buyer code with the reviewed 0.6.3 bytes and
/// names the module owning any redirected target. Reads only.
pub fn buyer_anchor_report() -> Vec<String> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        hooks!(
            vec!["SHOP ANCHORS skipped: no supported executable recognised".into()],
            buyer_anchor_report()
        )
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        vec!["SHOP ANCHORS unsupported platform".into()]
    }
}

pub fn configure(
    timing: Arc<NativeTiming>,
    logger: Arc<Logger>,
    movement: Arc<crate::movement::Movement>,
    camera: Arc<crate::camera::CameraControl>,
    abilities: Arc<crate::abilities::Abilities>,
) -> Result<(), &'static str> {
    logger.write(&format!(
        "NATIVE CONFIG coordinator={:p} shared_slot={:p} os_thread={}",
        Arc::as_ptr(&timing),
        &SHARED,
        crate::platform_input::thread_id()
    ));
    SHARED
        .set(Shared {
            timing,
            logger,
            movement,
            camera,
            abilities,
        })
        .map_err(|_| "Native adapter already configured; restart game")
}
#[derive(Clone, Copy)]
struct StopTicket {
    key: crate::native_timing::MatchKey,
    actor: usize,
    hold: bool,
    cancel_recall: bool,
    /// The input stands in for a basic attack waiting on its cooldown.
    attack_wait: bool,
}
thread_local! {
    static STOP_TICKET: std::cell::Cell<Option<StopTicket>> = const { std::cell::Cell::new(None) };
}
pub fn arm_stop(
    key: crate::native_timing::MatchKey,
    actor: Option<usize>,
    hold: bool,
    cancel_recall: bool,
    attack_wait: bool,
) {
    STOP_TICKET.set(actor.map(|actor| StopTicket {
        key,
        actor,
        hold,
        cancel_recall,
        attack_wait,
    }));
}

pub fn capture_trace(label: &str, logger: &Logger) {
    #[cfg(all(windows, target_arch = "x86_64"))]
    hooks!(
        v070::windows::capture_trace(label, logger),
        capture_trace(label, logger)
    );
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    logger.write(&format!(
        "NATIVE STACK label={label:?} unavailable on this platform"
    ));
}

/// `thread`'s call stack as `module+0xoffset`, innermost first, or why it
/// could not be read. For the worker stall report (crate::worker_watch).
pub fn thread_stack(thread: u64) -> Result<Vec<String>, &'static str> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        hooks!(v070::windows::thread_stack(thread), thread_stack(thread))
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let _ = thread;
        Err("unavailable on this platform")
    }
}
/// Whether `thread` is still running (assumed so where it cannot be told).
pub fn thread_alive(thread: u64) -> bool {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        hooks!(v070::windows::thread_alive(thread), thread_alive(thread))
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let _ = thread;
        true
    }
}

/// Called from the SDK client, independent of whether a native hook arrives.
pub fn sample_status(logger: &Logger) -> bool {
    let worker = WORKER_ENTRIES.load(Ordering::Relaxed);
    let viewer = VIEW_ENTRIES.load(Ordering::Relaxed);
    let input = INPUT_ENTRIES.load(Ordering::Relaxed);
    #[cfg(all(windows, target_arch = "x86_64"))]
    let patches = hooks!(None, audit_patches());
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    let patches: Option<bool> = None;
    logger.write(&format!("NATIVE TRAFFIC worker_entries={worker} viewer_entries={viewer} input_entries={input} steering_entries={} owned_steering_entries={} direct_steps={} steering_pointer_rejections={} patch_bytes_match={patches:?} shared_configured={} os_thread={}",
        STEER_ENTRIES.load(Ordering::Relaxed), OWNED_STEER_ENTRIES.load(Ordering::Relaxed), DIRECT_STEPS.load(Ordering::Relaxed), STEER_POINTER_REJECTIONS.load(Ordering::Relaxed), SHARED.get().is_some(), crate::platform_input::thread_id()));
    #[cfg(all(windows, target_arch = "x86_64"))]
    hooks!((), outline_status(logger));
    patches != Some(false)
}
/// A unit's drawn centre and body sprites from the last 250 ms (selection).
pub fn sprite_draw(id: usize) -> Option<((f32, f32), Vec<crate::sprite_art::Drawn>)> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        let draws = hooks!(return None, SPRITE_DRAWS.lock()).ok()?;
        let (_, center, sprites, at) = draws.iter().find(|d| d.0 == id)?;
        (at.elapsed() < std::time::Duration::from_millis(250)).then(|| (*center, sprites.clone()))
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let _ = id;
        None
    }
}
/// The running executable's SHA-256, computed once per session. Reading and
/// hashing the 87 MB file took about 100 ms on the client thread at the
/// title screen (docs/investigation-takeover-cost-2026-10-18.md, section 1),
/// so it runs on a background thread started when the mod loads. It touches
/// no game address. Nothing is cached across runs: every session hashes the
/// whole file again.
#[cfg(all(windows, target_arch = "x86_64"))]
static FINGERPRINT: OnceLock<Result<String, String>> = OnceLock::new();
/// The background fingerprint thread is running or has finished.
static FINGERPRINT_STARTED: AtomicBool = AtomicBool::new(false);
/// Start hashing the executable in the background (once). If the thread
/// cannot be started, `install` hashes on the client thread as before.
pub fn start_fingerprint() {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        if FINGERPRINT_STARTED.swap(true, Ordering::AcqRel) {
            return;
        }
        let spawned = std::thread::Builder::new()
            .name("lt-fingerprint".into())
            .spawn(|| {
                let sha = std::panic::catch_unwind(host::executable_sha)
                    .unwrap_or_else(|_| Err("Executable fingerprint failed".into()));
                let _ = FINGERPRINT.set(sha);
            })
            .is_ok();
        if !spawned {
            FINGERPRINT_STARTED.store(false, Ordering::Release);
        }
    }
}
/// `install` can run without waiting: the background fingerprint is done,
/// or none was started (then `install` hashes itself).
pub fn fingerprint_ready() -> bool {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        fingerprint_settled(
            FINGERPRINT_STARTED.load(Ordering::Acquire),
            FINGERPRINT.get().is_some(),
        )
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        true
    }
}
/// Ready when the result is there, or when no background hash runs.
#[cfg_attr(not(all(windows, target_arch = "x86_64")), allow(dead_code))]
fn fingerprint_settled(started: bool, done: bool) -> bool {
    done || !started
}
/// Installs the hooks built for the running executable, recognised by its
/// fingerprint, except the families in `skip` (`native_profile::family`).
/// On success: the game version and the hook families installed. An
/// executable no version names gets no hooks.
pub fn install(skip: u32) -> Result<(&'static str, u32), String> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        let sha = FINGERPRINT.get_or_init(host::executable_sha).clone()?;
        if sha == v070::identity::EXPECTED_SHA {
            ACTIVE.store(1, Ordering::Release);
            let families = v070::windows::install(&sha, skip)?;
            Ok((v070::identity::GAME, families))
        } else if sha == v063::identity::EXPECTED_SHA {
            ACTIVE.store(2, Ordering::Release);
            let families = v063::windows::install(&sha, skip)?;
            Ok((v063::identity::GAME, families))
        } else if sha == v064::identity::EXPECTED_SHA {
            ACTIVE.store(3, Ordering::Release);
            let families = v064::windows::install(&sha, skip)?;
            Ok((v064::identity::GAME, families))
        } else {
            Err("Executable fingerprint differs; no hooks installed".into())
        }
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let _ = skip;
        Err("Native adapter requires Windows x64".into())
    }
}
/// The outline hook is installed (not skipped, nor stepped aside for
/// another mod that holds the unit renderer).
pub fn outline_installed() -> bool {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        hooks!(false, installed(crate::native_profile::family::OUTLINE))
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        false
    }
}
/// Patches the athlete stat rules in, once a session, when this game
/// version supports them. Call only at the title screen, with the setting
/// On. None when there is nothing to do.
pub fn install_stat_rules() -> Option<Result<usize, String>> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        hooks!(None, install_stats_at_title())
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        None
    }
}
/// The stat rules are supported here and will be installed at the next title
/// screen if their setting is On.
pub fn stat_rules_pending() -> bool {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        hooks!(false, stats_pending())
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        false
    }
}
/// Verbose per-site collision counts of the stat rules (every few seconds).
/// `context` is appended to each `STATRULE trace` line (who and what). The
/// controlled athlete's shots the hooks saw since the last call.
pub fn stat_rules_report(
    logger: &Logger,
    context: &dyn Fn() -> String,
) -> Vec<crate::athlete_rules::ShotEvent> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        hooks!(Vec::new(), stat_rules_report(logger, context))
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let _ = (logger, context);
        Vec::new()
    }
}
/// Forget a destroyed viewer's lease without dereferencing its old addresses.
/// Live release restoration still happens inside the original viewer borrow.
pub fn reset_session() {
    crate::athlete_rules::clear();
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        hooks!((), clear_outline_targets());
        hooks!((), reset_session());
    }
}

// One block per supported game version: its tables (v*/), and the hook code
// (windows/) built against them. A new version also needs its number in
// `hooks!` and its fingerprint in `install`.
// (clippy: loading windows/ more than once is the point.)
#[cfg(all(windows, target_arch = "x86_64"))]
mod v070 {
    use super::*;
    mod effects;
    pub(super) mod identity;
    mod items;
    mod layout;
    mod stat_sites;
    #[allow(clippy::duplicate_mod)]
    #[path = "../windows/mod.rs"]
    pub(super) mod windows;
}
#[cfg(all(windows, target_arch = "x86_64"))]
mod v063 {
    use super::*;
    mod effects;
    pub(super) mod identity;
    mod items;
    mod layout;
    mod stat_sites;
    #[allow(clippy::duplicate_mod)]
    #[path = "../windows/mod.rs"]
    pub(super) mod windows;
}
#[cfg(all(windows, target_arch = "x86_64"))]
mod v064 {
    use super::*;
    mod effects;
    pub(super) mod identity;
    mod items;
    mod layout;
    mod stat_sites;
    #[allow(clippy::duplicate_mod)]
    #[path = "../windows/mod.rs"]
    pub(super) mod windows;
}
#[cfg(all(windows, target_arch = "x86_64"))]
mod host;
#[cfg(test)]
mod fingerprint_tests {
    #[test]
    fn install_waits_only_while_the_background_hash_runs() {
        // Running: wait at the title. Done: install. Never started (the
        // thread could not be spawned): install hashes itself, as before.
        assert!(!super::fingerprint_settled(true, false));
        assert!(super::fingerprint_settled(true, true));
        assert!(super::fingerprint_settled(false, false));
    }
}
