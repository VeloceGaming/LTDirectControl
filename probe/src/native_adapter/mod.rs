//! Experimental adapter for ONE fingerprinted executable, not a stable API.
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
static DEATH_GREYSCALE: AtomicBool = AtomicBool::new(false);
pub(crate) fn asset_digest(bytes: &[u8]) -> Result<String, String> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        windows::sha256(bytes)
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let _ = bytes;
        Err("Image checksums require the supported Windows host".into())
    }
}
#[cfg(all(windows, target_arch = "x86_64"))]
pub(crate) fn verified_base() -> Option<usize> {
    windows::verified_base()
}
pub fn set_death_greyscale(dead: bool) {
    DEATH_GREYSCALE.store(dead, Ordering::Release);
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
    windows::set_outline_targets(OutlineTargets {
        hover,
        attack,
        click,
    });
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
            windows::attack_ranges(state as usize, table as usize, actor)
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
            windows::player_build(state as usize, table as usize, id)
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
            windows::player_owned_len(state as usize, table as usize, id)
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
            windows::native_player(state as usize, table as usize, id)
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
        windows::item_slot_capacity()
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
        windows::shop_ready()
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
        windows::buyer_anchor_report()
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
    /// An SDK-accepted AA target confirmed in range on this worker tick.
    attack_target: Option<usize>,
}
thread_local! {
    static STOP_TICKET: std::cell::Cell<Option<StopTicket>> = const { std::cell::Cell::new(None) };
}
pub fn arm_stop(
    key: crate::native_timing::MatchKey,
    actor: Option<usize>,
    hold: bool,
    cancel_recall: bool,
    attack_target: Option<usize>,
) {
    STOP_TICKET.set(actor.map(|actor| StopTicket {
        key,
        actor,
        hold,
        cancel_recall,
        attack_target,
    }));
}

pub fn capture_trace(label: &str, logger: &Logger) {
    #[cfg(all(windows, target_arch = "x86_64"))]
    windows::capture_trace(label, logger);
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    logger.write(&format!(
        "NATIVE STACK label={label:?} unavailable on this platform"
    ));
}

/// Called from the SDK client, independent of whether a native hook arrives.
pub fn sample_status(logger: &Logger) -> bool {
    let worker = WORKER_ENTRIES.load(Ordering::Relaxed);
    let viewer = VIEW_ENTRIES.load(Ordering::Relaxed);
    let input = INPUT_ENTRIES.load(Ordering::Relaxed);
    #[cfg(all(windows, target_arch = "x86_64"))]
    let patches = windows::audit_patches();
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    let patches: Option<bool> = None;
    logger.write(&format!("NATIVE TRAFFIC worker_entries={worker} viewer_entries={viewer} input_entries={input} steering_entries={} owned_steering_entries={} direct_steps={} steering_pointer_rejections={} patch_bytes_match={patches:?} shared_configured={} os_thread={}",
        STEER_ENTRIES.load(Ordering::Relaxed), OWNED_STEER_ENTRIES.load(Ordering::Relaxed), DIRECT_STEPS.load(Ordering::Relaxed), STEER_POINTER_REJECTIONS.load(Ordering::Relaxed), SHARED.get().is_some(), crate::platform_input::thread_id()));
    #[cfg(all(windows, target_arch = "x86_64"))]
    windows::outline_status(logger);
    patches != Some(false)
}
/// A unit's drawn centre and body sprites from the last 250 ms (selection).
pub fn sprite_draw(id: usize) -> Option<((f32, f32), Vec<crate::sprite_art::Drawn>)> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        let draws = windows::SPRITE_DRAWS.lock().ok()?;
        let (_, center, sprites, at) = draws.iter().find(|d| d.0 == id)?;
        (at.elapsed() < std::time::Duration::from_millis(250)).then(|| (*center, sprites.clone()))
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let _ = id;
        None
    }
}
pub fn install() -> Result<(), String> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        windows::install()
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        Err("Native adapter requires Windows x64".into())
    }
}
/// Forget a destroyed viewer's lease without dereferencing its old addresses.
/// Live release restoration still happens inside the original viewer borrow.
pub fn reset_session() {
    set_death_greyscale(false);
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        windows::clear_outline_targets();
        windows::reset_session();
    }
}

#[cfg(all(windows, target_arch = "x86_64"))]
mod windows;
