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
}
thread_local! {
    static STOP_TICKET: std::cell::Cell<Option<StopTicket>> = const { std::cell::Cell::new(None) };
}
pub fn arm_stop(
    key: crate::native_timing::MatchKey,
    actor: Option<usize>,
    hold: bool,
    cancel_recall: bool,
) {
    STOP_TICKET.set(actor.map(|actor| StopTicket {
        key,
        actor,
        hold,
        cancel_recall,
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
mod windows {
    use super::*;
    use crate::native_profile::{EXPECTED_SHA, IMAGE_SIZE, PE_TIMESTAMP};
    use std::ffi::{c_void, OsString};
    use std::os::windows::ffi::OsStringExt;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    pub(super) fn verified_base() -> Option<usize> {
        PATCHES.get()?;
        let base = unsafe { GetModuleHandleW(std::ptr::null()) } as usize;
        (base != 0).then_some(base)
    }

    // 0.6.3: foreground worker send after runner locks; source/call graph
    // and all 420 decoded instructions reviewed against the 0.6.2 baseline.
    const WORKER_SITE: usize = 0xbfc77a;
    const WORKER_ORIGINAL: usize = 0xd1c2a0;
    // Actual foreground viewer dispatch, paired with the unchanged playback
    // implementation. The alternate dispatch branches are not this hook.
    const VIEW_SITE: usize = 0xcdcce0;
    const VIEW_ORIGINAL: usize = 0x930640;
    const MOVE_SITE: usize = 0x11d9154;
    const MOVE_ORIGINAL: usize = 0x1703fa0;
    const STOP_EVENT: usize = 0x13cccc0;
    // Native Return cancellation event (also used before native skills/attacks
    // when action == 1); &events, actor ID -> unit. No new branch redirect.
    const CANCEL_RECALL_EVENT: usize = 0x13cda10;
    const CANCEL_RECALL_PROLOGUE: [u8; 16] = [
        0x55, 0x56, 0x57, 0x48, 0x83, 0xec, 0x30, 0x48, 0x8d, 0x6c, 0x24, 0x30, 0x48, 0xc7, 0x45,
        0xf8,
    ];
    // GameClient::input_event: view/client, UI, system, f32 dt, event, window, database.
    // Native tag 6 = key press, tag 7 = key release, with key byte at event+8.
    // These two variants are POD; skipping them needs no native allocation drop.
    // The foreground dispatch forwards the SAME view to the input and
    // playback branches; the alternative scene dispatch is not this hook.
    const INPUT_SITE: usize = 0xd03c7f;
    // Native Input::Attack variant: EntityData, &InputTarget, &events -> unit.
    // This observes POD skill metadata on attack ticks as well as move ticks.
    const ATTACK_SITE: usize = 0x11d9200;
    const ATTACK_ORIGINAL: usize = 0x16f3e30;
    const ATTACK_BYTES: [u8; 5] = [0xe8, 0x2b, 0xac, 0x51, 0x00];
    const AUTO_ATTACK_SITE: usize = 0x16f7160;
    const AUTO_ATTACK_BYTES: [u8; 5] = [0xe8, 0xcb, 0xcc, 0xff, 0xff];
    // Entity update passes RNG, sim data/table, navigation, caster ID,
    // &Effect Arc and &mut InputTarget to the native skillshot correction.
    // Direction/point targets can be rewritten using enemy motion and stats.
    const AIM_SITE: usize = 0x16fb398;
    const AIM_ORIGINAL: usize = 0x1707b00;
    const AIM_BYTES: [u8; 5] = [0xe8, 0x63, 0xc7, 0x00, 0x00];
    const AIM_PROLOGUE: [u8; 19] = [
        0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x55, 0x53, 0x48, 0x81, 0xec,
        0xa8, 0x01, 0x00, 0x00,
    ];
    const AIM_WRITES: [(usize, &[u8]); 2] = [
        (0x17083fd, &[0x4c, 0x89, 0x40, 0x08]),
        (0x1708414, &[0x4c, 0x89, 0x40, 0x08]),
    ];
    static ORIGINAL_AIM: AtomicUsize = AtomicUsize::new(0);
    const ATTACK_PROLOGUE: [u8; 19] = [
        0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec,
        0x88, 0x00, 0x00, 0x00,
    ];
    // GameView::render's #Game -> UI composite only. Minimap and HUD are
    // separate commands. Four GP args: sret, owned RenderCommand, shader &str.
    const SHADER_SITE: usize = 0x2322b12;
    const SHADER_ORIGINAL: usize = 0x1c91f0;
    const SHADER_BYTES: [u8; 5] = [0xe8, 0xd9, 0x66, 0xea, 0xfd];
    const SHADER_PROLOGUE: [u8; 19] = [
        0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec,
        0x38, 0x03, 0x00, 0x00,
    ];
    static ORIGINAL_SHADER: AtomicUsize = AtomicUsize::new(0);
    // The entity body renderer is version-pinned. Renderer output is an owned
    // Vec<RenderCommand>; the wrapper moves entire commands, never clones a
    // String, ShaderInfo or other Rust allocation by copying it twice.
    const OUTLINE_ORIGINAL: usize = 0x230ec30;
    const OUTLINE_SITES: [(usize, [u8; 5]); 15] = [
        (0x1f077eb, [0xe8, 0x40, 0x74, 0x40, 0x00]),
        (0x215ae74, [0xe8, 0xb7, 0x3d, 0x1b, 0x00]),
        (0x215c5c5, [0xe8, 0x66, 0x26, 0x1b, 0x00]),
        (0x231faa6, [0xe8, 0x85, 0xf1, 0xfe, 0xff]),
        (0x2347def, [0xe8, 0x3c, 0x6e, 0xfc, 0xff]),
        (0x2348c33, [0xe8, 0xf8, 0x5f, 0xfc, 0xff]),
        (0x234a2c9, [0xe8, 0x62, 0x49, 0xfc, 0xff]),
        (0x234b334, [0xe8, 0xf7, 0x38, 0xfc, 0xff]),
        (0x2350312, [0xe8, 0x19, 0xe9, 0xfb, 0xff]),
        (0x2370fe0, [0xe8, 0x4b, 0xdc, 0xf9, 0xff]),
        (0x2371024, [0xe8, 0x07, 0xdc, 0xf9, 0xff]),
        (0x2516bd7, [0xe8, 0x54, 0x80, 0xdf, 0xff]),
        (0x2518cc9, [0xe8, 0x62, 0x5f, 0xdf, 0xff]),
        (0x2519be1, [0xe8, 0x4a, 0x50, 0xdf, 0xff]),
        (0x251b4b4, [0xe8, 0x77, 0x37, 0xdf, 0xff]),
    ];
    const OUTLINE_PROLOGUE: [u8; 16] = [
        0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec,
        0x78,
    ];
    const OUTLINE_PARAM_PROLOGUE: [u8; 16] = [
        0x55, 0x41, 0x57, 0x41, 0x56, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec, 0xd8, 0x01, 0x00, 0x00,
        0x48,
    ];
    // Generic renderer's DrawLine discriminant write, reviewed on 0.6.3.
    const OUTLINE_LINE_SITE: usize = 0x2311692;
    const OUTLINE_LINE_BYTES: [u8; 10] = [0x48, 0xb8, 8, 0, 0, 0, 0, 0, 0, 0x80];
    // RenderCommand drop glue: one borrowed command address in RCX, unit
    // return. Its dispatch covers every owned variant; it does not free the
    // containing command slot. Reviewed against the named SDK drop glue and
    // live shader() cleanup caller, including the tag-to-field dispatch table.
    const OUTLINE_DROP_COMMAND: usize = 0x1c5190;
    const OUTLINE_DROP_BYTES: [u8; 65] = [
        0x55, 0x56, 0x57, 0x48, 0x83, 0xec, 0x30, 0x48, 0x8d, 0x6c, 0x24, 0x30, 0x48, 0xc7, 0x45,
        0xf8, 0xfe, 0xff, 0xff, 0xff, 0x48, 0x89, 0xcf, 0x48, 0x8b, 0x01, 0x48, 0xba, 0, 0, 0, 0,
        0, 0, 0, 0x80, 0x48, 0x31, 0xc2, 0x48, 0x85, 0xc0, 0xb9, 7, 0, 0, 0, 0x48, 0x0f, 0x48,
        0xca, 0x48, 0x83, 0xc1, 0xfe, 0x48, 0x83, 0xf9, 0x0c, 0x0f, 0x87, 0x54, 2, 0, 0,
    ];
    const OUTLINE_DROP_TABLE: usize = 0x3a199ac;
    const OUTLINE_DROP_TABLE_BYTES: [u8; 52] = [
        0x35, 0xb8, 0x7a, 0xfc, 0x35, 0xb8, 0x7a, 0xfc, 0x35, 0xb8, 0x7a, 0xfc, 0x29, 0xb9, 0x7a,
        0xfc, 0xf6, 0xb9, 0x7a, 0xfc, 0x9c, 0xb8, 0x7a, 0xfc, 0x79, 0xba, 0x7a, 0xfc, 0x88, 0xb8,
        0x7a, 0xfc, 0x79, 0xba, 0x7a, 0xfc, 0x88, 0xb8, 0x7a, 0xfc, 0xb8, 0xb9, 0x7a, 0xfc, 0x79,
        0xba, 0x7a, 0xfc, 0x88, 0xb8, 0x7a, 0xfc,
    ];
    const OUTLINE_DROP_CALLER: usize = 0x1ca533;
    const OUTLINE_DROP_CALL_BYTES: [u8; 5] = [0xe8, 0x58, 0xac, 0xff, 0xff];
    const OUTLINE_NINEPATCH_SITE: usize = 0x1c7a8a;
    const OUTLINE_NINEPATCH_BYTES: [u8; 10] = [0x48, 0xb8, 4, 0, 0, 0, 0, 0, 0, 0x80];
    static ORIGINAL_OUTLINE: AtomicUsize = AtomicUsize::new(0);
    static OUTLINE_READY: AtomicBool = AtomicBool::new(false);
    static OUTLINE_TARGETS: std::sync::Mutex<OutlineTargets> =
        std::sync::Mutex::new(OutlineTargets {
            hover: None,
            attack: None,
            click: None,
        });
    static OUTLINE_MATCHES: AtomicUsize = AtomicUsize::new(0);
    static OUTLINE_HOVER_DRAWS: AtomicUsize = AtomicUsize::new(0);
    static OUTLINE_TARGET_DRAWS: AtomicUsize = AtomicUsize::new(0);
    static OUTLINE_CLICK_DRAWS: AtomicUsize = AtomicUsize::new(0);
    static OUTLINE_ID_MISSES: AtomicUsize = AtomicUsize::new(0);
    static OUTLINE_UNSUPPORTED: AtomicUsize = AtomicUsize::new(0);
    static OUTLINE_MIXED: AtomicUsize = AtomicUsize::new(0);
    static OUTLINE_BAD_VEC: AtomicUsize = AtomicUsize::new(0);
    static OUTLINE_NO_SPRITE: AtomicUsize = AtomicUsize::new(0);
    static OUTLINE_TAG_COUNTS: [AtomicUsize; 20] = [const { AtomicUsize::new(0) }; 20];
    static OUTLINE_UNKNOWN_TAG: AtomicUsize = AtomicUsize::new(usize::MAX);
    const STEER_SITE: usize = 0x16fd5cf;
    const STEER_ORIGINAL: usize = 0x1702bd0;
    const STEER_BYTES: [u8; 5] = [0xe8, 0xfc, 0x55, 0x00, 0x00];
    const DIRECT_STEP: usize = 0x16f5100;
    const STEER_PROLOGUE: [u8; 16] = [
        0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x83, 0xec,
        0x58,
    ];
    const DIRECT_PROLOGUE: [u8; 16] = [
        0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x83, 0xec,
        0x38,
    ];
    static ORIGINAL_STEER: AtomicUsize = AtomicUsize::new(0);
    static ORIGINAL_DIRECT_STEP: AtomicUsize = AtomicUsize::new(0);
    // SDK SimVtable and gold getter establish the live AbstractGame::get_player
    // borrow. Native purchase selector ea1ea0 reads the final build Vec
    // at +350 (+358 pointer, +360 length); SDK offsets are not substituted.
    const GOLD_GETTER: usize = 0x2e17b30;
    const GOLD_GETTER_BYTES: [u8; 70] = [
        0x55, 0x48, 0x83, 0xec, 0x30, 0x48, 0x8d, 0x6c, 0x24, 0x30, 0x48, 0xc7, 0x45, 0xf8, 0xfe,
        0xff, 0xff, 0xff, 0x48, 0x85, 0xd2, 0x74, 0x27, 0x48, 0xff, 0xca, 0x48, 0x8b, 0x01, 0x4c,
        0x8b, 0x41, 0x08, 0x48, 0x89, 0xc1, 0x41, 0xff, 0x90, 0x58, 0x01, 0x00, 0x00, 0x90, 0x48,
        0x85, 0xc0, 0x74, 0x0d, 0x48, 0x8b, 0x80, 0x68, 0x08, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x30,
        0x5d, 0xc3, 0x31, 0xc0, 0x48, 0x83, 0xc4, 0x30, 0x5d, 0xc3,
    ];
    const BUILD_LEN_SITE: usize = 0xea1ed2;
    const BUILD_LEN_BYTES: [u8; 7] = [0x4c, 0x8b, 0xa2, 0x60, 0x03, 0x00, 0x00];
    const BUILD_PTR_SITE: usize = 0xea1ecb;
    const BUILD_PTR_BYTES: [u8; 7] = [0x48, 0x8b, 0x9a, 0x58, 0x03, 0x00, 0x00];

    unsafe fn player_pointer(state: usize, table: usize, id: usize) -> Option<usize> {
        if PATCHES.get().is_none() || state == 0 || id == usize::MAX {
            return None;
        }
        let base = GetModuleHandleW(std::ptr::null()) as usize;
        if table == 0
            || std::ptr::read_unaligned(table as *const usize) < 240
            || std::ptr::read_unaligned((table + 232) as *const usize) != base + GOLD_GETTER
        {
            return None;
        }
        let object = std::ptr::read_unaligned(state as *const usize);
        let vtable = std::ptr::read_unaligned((state + 8) as *const usize);
        if object == 0 || vtable == 0 {
            return None;
        }
        let get: unsafe extern "system" fn(usize, usize) -> usize =
            std::mem::transmute(std::ptr::read_unaligned((vtable + 0x158) as *const usize));
        Some(get(object, id)).filter(|player| *player != 0)
    }

    // Purchase executor 1465e80 indexes owned items at +310 (pointer) and
    // bounds them by +318 (length) before each upgrade (146b4b0/146b4cb).
    pub unsafe fn player_owned_len(state: usize, table: usize, id: usize) -> Option<usize> {
        let player = player_pointer(state, table, id)?;
        let ptr = std::ptr::read_unaligned((player + 0x310) as *const usize);
        let len = std::ptr::read_unaligned((player + 0x318) as *const usize);
        (len <= 64 && (len == 0 || (ptr != 0 && ptr.is_multiple_of(8)))).then_some(len)
    }

    // Reviewed 0.6.3 buyer: executor gates/payments, the controller vtable
    // slots +80 (buy new) and +88 (upgrade), their E9 thunks and decisions.
    const BUYER_CODE: [(&str, usize, &[u8]); 10] = [
        (
            "new_item_gate",
            0x146baf3,
            &[0x48, 0x83, 0xf8, 0x03, 0x0f, 0x87, 0x4a, 0x02, 0x00, 0x00],
        ),
        (
            "selector_gate",
            0xea216a,
            &[0x48, 0x83, 0xfb, 0x03, 0x76, 0x37],
        ),
        (
            "upgrade_payment",
            0x146b60b,
            &[0x48, 0x29, 0x86, 0x68, 0x08, 0x00, 0x00],
        ),
        (
            "new_item_payment",
            0x146bbaf,
            &[0x48, 0x29, 0x86, 0x68, 0x08, 0x00, 0x00],
        ),
        ("upgrade_thunk", 0xf60620, &[0xe9, 0x7b, 0xed, 0xfd, 0xff]),
        ("new_item_thunk", 0xf61b80, &[0xe9, 0x8b, 0xd9, 0xfd, 0xff]),
        (
            "goal_search",
            0xea1b90,
            &[
                0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81,
                0xec, 0x98,
            ],
        ),
        (
            "next_step",
            0xea1ea0,
            &[
                0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x55, 0x53, 0x48, 0x81,
                0xec, 0xa8,
            ],
        ),
        (
            "upgrade_decision",
            0xf3f3a0,
            &[
                0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x55, 0x53, 0x48, 0x83,
                0xec, 0x58,
            ],
        ),
        (
            "new_item_decision",
            0xf3f510,
            &[
                0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x55, 0x53, 0x48, 0x83,
                0xec, 0x58,
            ],
        ),
    ];
    const BUYER_SLOTS: [(&str, usize, usize); 2] = [
        ("new_item_slot", 0x3b08c58, 0xf61b80),
        ("upgrade_slot", 0x3b08c60, 0xf60620),
    ];

    unsafe fn module_offset(address: usize) -> String {
        let mut module = std::ptr::null_mut();
        if GetModuleHandleExW(0x06, address as *const u16, &mut module) == 0 {
            return format!("unmapped@{address:x}");
        }
        let mut name = [0u16; 512];
        let length = GetModuleFileNameW(module, name.as_mut_ptr(), name.len() as u32) as usize;
        let path = OsString::from_wide(&name[..length.min(name.len())]);
        let filename = std::path::Path::new(&path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        format!("{filename}+0x{:x}", address - module as usize)
    }

    pub fn buyer_anchor_report() -> Vec<String> {
        if PATCHES.get().is_none() {
            return vec!["SHOP ANCHORS skipped: host identity not verified".into()];
        }
        unsafe {
            let base = GetModuleHandleW(std::ptr::null()) as usize;
            let mut lines = Vec::new();
            for (name, rva, expected) in BUYER_CODE {
                let live = std::slice::from_raw_parts((base + rva) as *const u8, expected.len());
                if live == expected {
                    lines.push(format!("SHOP ANCHOR {name} rva=0x{rva:x} native"));
                    continue;
                }
                // A leading E9/E8 is a redirect; name its destination.
                let target = matches!(live[0], 0xe8 | 0xe9).then(|| {
                    let rel = std::ptr::read_unaligned((base + rva + 1) as *const i32);
                    module_offset((base + rva + 5).wrapping_add_signed(rel as isize))
                });
                lines.push(format!(
                    "SHOP ANCHOR {name} rva=0x{rva:x} CHANGED live={live:02x?} expected={expected:02x?} redirect={target:?}"
                ));
            }
            for (name, rva, thunk) in BUYER_SLOTS {
                let live = std::ptr::read_unaligned((base + rva) as *const usize);
                if live == base + thunk {
                    lines.push(format!("SHOP ANCHOR {name} rva=0x{rva:x} native"));
                } else {
                    lines.push(format!(
                        "SHOP ANCHOR {name} rva=0x{rva:x} CHANGED points_to={}",
                        module_offset(live)
                    ));
                }
            }
            lines
        }
    }

    pub unsafe fn native_player(state: usize, table: usize, id: usize) -> Option<usize> {
        player_pointer(state, table, id)
    }

    // Manual shopping. The purchase executor 1465e80 asks the controller
    // vtable (3b08bd8) slot +88 "upgrade?" (thunk f60620 -> f3f3a0, 24-byte
    // sret {kind, owned slot, item}) and slot +80 "buy new?" (thunk f61b80 ->
    // f3f510, kind in RAX and item in RDX) while a champion is in base. The
    // executor validates and performs every purchase. Redirecting the E9
    // thunks leaves any other mod's patch inside f3f510 intact; nothing here
    // fingerprints those decision bodies.
    const SHOP_UPGRADE_THUNK: usize = 0xf60620;
    const SHOP_UPGRADE_BYTES: [u8; 5] = [0xe9, 0x7b, 0xed, 0xfd, 0xff];
    const SHOP_NEW_THUNK: usize = 0xf61b80;
    const SHOP_NEW_BYTES: [u8; 5] = [0xe9, 0x8b, 0xd9, 0xfd, 0xff];
    const SHOP_SLOTS: [(usize, usize); 2] =
        [(0x3b08c60, SHOP_UPGRADE_THUNK), (0x3b08c58, SHOP_NEW_THUNK)];
    static ORIGINAL_SHOP_UPGRADE: AtomicUsize = AtomicUsize::new(0);
    static ORIGINAL_SHOP_NEW: AtomicUsize = AtomicUsize::new(0);
    static SHOP_READY: AtomicBool = AtomicBool::new(false);
    type ShopUpgradeFn =
        unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, usize) -> usize;

    pub(super) fn item_slot_capacity() -> Option<usize> {
        PATCHES.get()?;
        unsafe {
            let base = GetModuleHandleW(std::ptr::null()) as usize;
            let gate = std::slice::from_raw_parts((base + 0x146baf3) as *const u8, 6);
            // cmp rax, imm8 ; ja rel32
            (gate[..3] == [0x48, 0x83, 0xf8] && gate[4..6] == [0x0f, 0x87] && gate[3] < 64)
                .then(|| gate[3] as usize + 1)
        }
    }
    pub(super) fn shop_ready() -> bool {
        SHOP_READY.load(Ordering::Acquire)
    }

    unsafe extern "system" fn shop_upgrade_hook(
        out: usize,
        this: usize,
        a3: usize,
        player: usize,
        a5: usize,
        a6: usize,
        a7: usize,
    ) -> usize {
        use crate::shop::Answer;
        let answer =
            catch_unwind(|| crate::shop::SHOP.upgrade_answer(player)).unwrap_or(Answer::Native);
        let fields: [u64; 3] = match answer {
            Answer::Native => {
                let original: ShopUpgradeFn =
                    std::mem::transmute(ORIGINAL_SHOP_UPGRADE.load(Ordering::Acquire));
                return original(out, this, a3, player, a5, a6, a7);
            }
            Answer::Upgrade { slot, item } => [1, slot as u64, item as u64],
            Answer::Nothing | Answer::New { .. } => [0, 0, 0],
        };
        for (i, v) in fields.into_iter().enumerate() {
            std::ptr::write_unaligned((out + i * 8) as *mut u64, v);
        }
        out
    }

    /// 0 = answer natively; 1 = `out` holds {kind, item} for RAX/RDX.
    unsafe extern "system" fn shop_new_decide(player: usize, out: *mut [u64; 2]) -> u32 {
        use crate::shop::Answer;
        let answer =
            catch_unwind(|| crate::shop::SHOP.new_item_answer(player)).unwrap_or(Answer::Native);
        let fields = match answer {
            Answer::Native => return 0,
            Answer::New { item } => [1, item as u64],
            Answer::Nothing | Answer::Upgrade { .. } => [0, 0],
        };
        std::ptr::write_unaligned(out, fields);
        1
    }

    // Native ABI: RCX this, RDX, R8 player, R9, stack args; returns a pair in
    // RAX:RDX. The shim keeps all argument registers and stack arguments for
    // the pass-through jump, so the original runs exactly as if called.
    #[unsafe(naked)]
    unsafe extern "system" fn shop_new_hook() {
        std::arch::naked_asm!(
            "push rcx",
            "push rdx",
            "push r8",
            "push r9",
            "sub rsp, 0x38",
            "mov rcx, r8",
            "lea rdx, [rsp + 0x20]",
            "call {decide}",
            "test eax, eax",
            "jz 2f",
            "mov rax, qword ptr [rsp + 0x20]",
            "mov rdx, qword ptr [rsp + 0x28]",
            "add rsp, 0x58",
            "ret",
            "2:",
            "add rsp, 0x38",
            "pop r9",
            "pop r8",
            "pop rdx",
            "pop rcx",
            "jmp qword ptr [rip + {original}]",
            decide = sym shop_new_decide,
            original = sym ORIGINAL_SHOP_NEW,
        );
    }

    /// Optional: a failure leaves Manual shopping unavailable and every
    /// other hook installed.
    unsafe fn install_shop(base: usize) -> Result<(), String> {
        for (site, expected) in [
            (SHOP_UPGRADE_THUNK, SHOP_UPGRADE_BYTES),
            (SHOP_NEW_THUNK, SHOP_NEW_BYTES),
        ] {
            if read_call(base + site) != expected {
                return Err(format!("buyer thunk 0x{site:x} differs (another mod?)"));
            }
        }
        for (slot, thunk) in SHOP_SLOTS {
            if std::ptr::read_unaligned((base + slot) as *const usize) != base + thunk {
                return Err(format!(
                    "controller slot 0x{slot:x} no longer uses its thunk"
                ));
            }
        }
        let target = |site: usize| {
            let rel = std::ptr::read_unaligned((base + site + 1) as *const i32);
            (base + site + 5).wrapping_add_signed(rel as isize)
        };
        ORIGINAL_SHOP_UPGRADE.store(target(SHOP_UPGRADE_THUNK), Ordering::Release);
        ORIGINAL_SHOP_NEW.store(target(SHOP_NEW_THUNK), Ordering::Release);
        let mut patches = Vec::new();
        for (site, expected, hook) in [
            (
                SHOP_UPGRADE_THUNK,
                SHOP_UPGRADE_BYTES,
                shop_upgrade_hook as *const () as usize,
            ),
            (
                SHOP_NEW_THUNK,
                SHOP_NEW_BYTES,
                shop_new_hook as *const () as usize,
            ),
        ] {
            let site = base + site;
            let relay = relay(site, hook)?;
            patches.push((site, expected, relative_jump(site, relay)?));
        }
        for (site, expected, patch) in &patches {
            let mut old = 0;
            if VirtualProtect(*site as *mut c_void, 5, 0x40, &mut old) == 0 {
                return Err("cannot unprotect buyer thunk".into());
            }
            std::ptr::copy_nonoverlapping(patch.as_ptr(), *site as *mut u8, 5);
            let applied = read_call(*site) == *patch;
            if !applied {
                std::ptr::copy_nonoverlapping(expected.as_ptr(), *site as *mut u8, 5);
            }
            FlushInstructionCache(GetCurrentProcess(), *site as *const c_void, 5);
            let mut ignored = 0;
            VirtualProtect(*site as *mut c_void, 5, old, &mut ignored);
            if !applied {
                return Err("buyer thunk readback mismatch; original restored".into());
            }
        }
        SHOP_READY.store(true, Ordering::Release);
        Ok(())
    }

    pub unsafe fn player_build(state: usize, table: usize, id: usize) -> Option<Vec<usize>> {
        if PATCHES.get().is_none() || state == 0 || id == usize::MAX {
            return None;
        }
        let base = GetModuleHandleW(std::ptr::null()) as usize;
        if table == 0
            || std::ptr::read_unaligned(table as *const usize) < 240
            || std::ptr::read_unaligned((table + 232) as *const usize) != base + GOLD_GETTER
        {
            return None;
        }
        let object = std::ptr::read_unaligned(state as *const usize);
        let vtable = std::ptr::read_unaligned((state + 8) as *const usize);
        if object == 0 || vtable == 0 {
            return None;
        }
        let get: unsafe extern "system" fn(usize, usize) -> usize =
            std::mem::transmute(std::ptr::read_unaligned((vtable + 0x158) as *const usize));
        let player = get(object, id);
        if player == 0 {
            return None;
        }
        let cap = std::ptr::read_unaligned((player + 0x350) as *const usize);
        let ptr = std::ptr::read_unaligned((player + 0x358) as *const usize);
        let len = std::ptr::read_unaligned((player + 0x360) as *const usize);
        if len > 64 || cap < len || cap > 256 || ptr == 0 || !ptr.is_multiple_of(8) {
            return None;
        }
        Some(std::slice::from_raw_parts(ptr as *const usize, len).to_vec())
    }
    const INPUT_ORIGINAL: usize = 0x7b37f0;
    const INPUT_BYTES: [u8; 5] = [0xe8, 0x6c, 0xfb, 0xaa, 0xff];
    const INPUT_PROLOGUE: [u8; 19] = [
        0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec,
        0xb8, 0x01, 0x00, 0x00,
    ];
    const MOVE_BYTES: [u8; 5] = [0xe9, 0x47, 0xae, 0x52, 0x00];
    const MOVE_PROLOGUE: [u8; 16] = [
        0x56, 0x57, 0x4c, 0x8b, 0x91, 0xc8, 0x02, 0x00, 0x00, 0x4d, 0x85, 0xd2, 0x74, 0x34, 0x48,
        0x8b,
    ];
    const STOP_PROLOGUE: [u8; 16] = [
        0x55, 0x56, 0x57, 0x48, 0x83, 0xec, 0x30, 0x48, 0x8d, 0x6c, 0x24, 0x30, 0x48, 0xc7, 0x45,
        0xf8,
    ];
    const WORKER_BYTES: [u8; 5] = [0xe8, 0x21, 0xfb, 0x11, 0x00];
    const VIEW_BYTES: [u8; 5] = [0xe8, 0x5b, 0x39, 0xc5, 0xff];
    const WORKER_PROLOGUE: [u8; 19] = [
        0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec,
        0xb8, 0x05, 0x00, 0x00,
    ];
    const VIEW_PROLOGUE: [u8; 19] = [
        0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec,
        0x58, 0x01, 0x00, 0x00,
    ];
    static ORIGINAL_WORKER: AtomicUsize = AtomicUsize::new(0);
    static ORIGINAL_VIEW: AtomicUsize = AtomicUsize::new(0);
    static ORIGINAL_MOVE: AtomicUsize = AtomicUsize::new(0);
    static NOTIFY_STOP: AtomicUsize = AtomicUsize::new(0);
    static NOTIFY_CANCEL_RECALL: AtomicUsize = AtomicUsize::new(0);
    static ORIGINAL_INPUT: AtomicUsize = AtomicUsize::new(0);
    static ORIGINAL_ATTACK: AtomicUsize = AtomicUsize::new(0);
    static ATTACK_TRACE: crate::attack_trace::AttackTrace = crate::attack_trace::AttackTrace::new();
    const SKILL_SITES: [usize; 3] = [0x11d9231, 0x11d9180, 0x11d92e3];
    const SKILL_ORIGINALS: [usize; 3] = [0x17046b0, 0x16f4450, 0x1703390];
    const SKILL_BYTES: [[u8; 5]; 3] = [
        [0xe8, 0x7a, 0xb4, 0x52, 0x00],
        [0xe8, 0xcb, 0xb2, 0x51, 0x00],
        [0xe8, 0xa8, 0xa0, 0x52, 0x00],
    ];
    const SKILL_PROLOGUE: [u8; 16] = [
        0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec,
        0xa8,
    ];
    const SKILL_QUEUE_ANCHORS: [(usize, &[u8]); 3] = [
        (
            0x170498d,
            &[
                0x48, 0x8b, 0x83, 0xa8, 0x02, 0x00, 0x00, 0x49, 0x6b, 0xce, 0x38,
            ],
        ),
        (
            0x16f474d,
            &[
                0x48, 0x8b, 0x83, 0xa8, 0x02, 0x00, 0x00, 0x49, 0x6b, 0xce, 0x38,
            ],
        ),
        (
            0x170369d,
            &[
                0x48, 0x8b, 0x83, 0xa8, 0x02, 0x00, 0x00, 0x49, 0x6b, 0xce, 0x38,
            ],
        ),
    ];
    static ORIGINAL_SKILLS: [AtomicUsize; 3] = [const { AtomicUsize::new(0) }; 3];
    struct PatchRecord {
        worker: usize,
        viewer: usize,
        worker_bytes: [u8; 5],
        viewer_bytes: [u8; 5],
        movement: usize,
        movement_bytes: [u8; 5],
        input: usize,
        input_bytes: [u8; 5],
        attack: usize,
        attack_bytes: [u8; 5],
        shader: usize,
        shader_bytes: [u8; 5],
        skills: [(usize, [u8; 5]); 3],
        steering: (usize, [u8; 5]),
        auto_attack: (usize, [u8; 5]),
        aim: (usize, [u8; 5]),
        outlines: Vec<(usize, [u8; 5])>,
        minimap: Vec<(usize, Vec<u8>)>,
        minimap_constants: (usize, Vec<u8>),
    }
    static PATCHES: OnceLock<PatchRecord> = OnceLock::new();

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
        fn GetModuleHandleExW(flags: u32, name: *const u16, module: *mut *mut c_void) -> i32;
        fn GetModuleFileNameW(module: *mut c_void, name: *mut u16, size: u32) -> u32;
        fn VirtualAlloc(
            address: *mut c_void,
            size: usize,
            allocation: u32,
            protection: u32,
        ) -> *mut c_void;
        fn VirtualProtect(address: *mut c_void, size: usize, protection: u32, old: *mut u32)
            -> i32;
        fn FlushInstructionCache(process: *mut c_void, address: *const c_void, size: usize) -> i32;
        fn GetCurrentProcess() -> *mut c_void;
        fn GetProcessHeap() -> *mut c_void;
        fn HeapAlloc(heap: *mut c_void, flags: u32, size: usize) -> *mut c_void;
        fn HeapFree(heap: *mut c_void, flags: u32, memory: *mut c_void) -> i32;
        fn RtlCaptureStackBackTrace(
            skip: u32,
            count: u32,
            frames: *mut *mut c_void,
            hash: *mut u32,
        ) -> u16;
    }

    pub(super) fn capture_trace(label: &str, logger: &Logger) {
        let mut frames = [std::ptr::null_mut(); 64];
        let count =
            unsafe { RtlCaptureStackBackTrace(1, 64, frames.as_mut_ptr(), std::ptr::null_mut()) }
                as usize;
        let frames = frames[..count]
            .iter()
            .map(|frame| unsafe {
                let mut module = std::ptr::null_mut();
                if GetModuleHandleExW(0x06, *frame as *const u16, &mut module) == 0 {
                    return format!("unmapped@{:x}", *frame as usize);
                }
                let mut name = [0u16; 512];
                let length =
                    GetModuleFileNameW(module, name.as_mut_ptr(), name.len() as u32) as usize;
                let path = OsString::from_wide(&name[..length.min(name.len())]);
                let filename = std::path::Path::new(&path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                format!("{filename}+0x{:x}", *frame as usize - module as usize)
            })
            .collect::<Vec<_>>();
        logger.write(&format!(
            "NATIVE STACK label={label:?} os_thread={} rust_thread={:?} frames={frames:?}",
            crate::platform_input::thread_id(),
            std::thread::current().id()
        ));
    }
    pub(super) fn clear_outline_targets() {
        if let Ok(mut targets) = OUTLINE_TARGETS.lock() {
            *targets = OutlineTargets::default();
        }
    }
    pub(super) fn set_outline_targets(targets: OutlineTargets) {
        // Publish both identities, positions and teams together with no
        // intermediate empty target. Rendering copies and releases the lock
        // before calling any native renderer or command setter.
        if let Ok(mut current) = OUTLINE_TARGETS.lock() {
            *current = targets;
        }
    }
    pub(super) fn outline_status(logger: &Logger) {
        let tags = OUTLINE_TAG_COUNTS
            .iter()
            .enumerate()
            .filter_map(|(tag, counter)| {
                let count = counter.swap(0, Ordering::Relaxed);
                (count != 0).then(|| {
                    if tag == 19 {
                        format!(
                            "Unknown({:#x}):{count}",
                            OUTLINE_UNKNOWN_TAG.load(Ordering::Relaxed)
                        )
                    } else {
                        format!("{}:{count}", outline_command_name(tag))
                    }
                })
            })
            .collect::<Vec<_>>()
            .join(",");
        logger.write(&format!(
            "OUTLINE status={} hover_offset=1.2 drawn={} hover_drawn={} target_drawn={} click_drawn={} near_id_misses={} unsupported={} mixed={} bad_vec={} no_sprite={} commands=[{}]",
            OUTLINE_READY.load(Ordering::Relaxed),
            OUTLINE_MATCHES.swap(0, Ordering::Relaxed),
            OUTLINE_HOVER_DRAWS.swap(0, Ordering::Relaxed),
            OUTLINE_TARGET_DRAWS.swap(0, Ordering::Relaxed),
            OUTLINE_CLICK_DRAWS.swap(0, Ordering::Relaxed),
            OUTLINE_ID_MISSES.swap(0, Ordering::Relaxed),
            OUTLINE_UNSUPPORTED.swap(0, Ordering::Relaxed),
            OUTLINE_MIXED.swap(0, Ordering::Relaxed),
            OUTLINE_BAD_VEC.swap(0, Ordering::Relaxed),
            OUTLINE_NO_SPRITE.swap(0, Ordering::Relaxed),
            tags,
        ));
    }
    unsafe fn read_call(site: usize) -> [u8; 5] {
        let mut bytes = [0; 5];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = std::ptr::read_volatile((site + i) as *const u8);
        }
        bytes
    }
    pub(super) fn audit_patches() -> Option<bool> {
        // These addresses belong to the resident executable, not a transient view.
        PATCHES.get().map(|p| unsafe {
            read_call(p.worker) == p.worker_bytes
                && read_call(p.viewer) == p.viewer_bytes
                && read_call(p.movement) == p.movement_bytes
                && read_call(p.input) == p.input_bytes
                && read_call(p.attack) == p.attack_bytes
                && read_call(p.shader) == p.shader_bytes
                && p.skills
                    .iter()
                    .all(|(site, bytes)| read_call(*site) == *bytes)
                && read_call(p.steering.0) == p.steering.1
                && read_call(p.auto_attack.0) == p.auto_attack.1
                && read_call(p.aim.0) == p.aim.1
                && p.outlines
                    .iter()
                    .all(|(site, bytes)| read_call(*site) == *bytes)
                && p.minimap.iter().all(|(site, bytes)| {
                    std::slice::from_raw_parts(*site as *const u8, bytes.len()) == bytes
                })
                && std::slice::from_raw_parts(
                    p.minimap_constants.0 as *const u8,
                    p.minimap_constants.1.len(),
                ) == p.minimap_constants.1
        })
    }
    #[link(name = "bcrypt")]
    extern "system" {
        fn BCryptOpenAlgorithmProvider(
            handle: *mut *mut c_void,
            algorithm: *const u16,
            implementation: *const u16,
            flags: u32,
        ) -> i32;
        fn BCryptHash(
            handle: *mut c_void,
            secret: *const u8,
            secret_size: u32,
            input: *const u8,
            input_size: u32,
            output: *mut u8,
            output_size: u32,
        ) -> i32;
        fn BCryptCloseAlgorithmProvider(handle: *mut c_void, flags: u32) -> i32;
    }

    fn sha256(bytes: &[u8]) -> Result<String, String> {
        let algorithm: Vec<u16> = "SHA256\0".encode_utf16().collect();
        let mut handle = std::ptr::null_mut();
        let mut digest = [0u8; 32];
        let size = u32::try_from(bytes.len()).map_err(|_| "Executable too large")?;
        unsafe {
            if BCryptOpenAlgorithmProvider(&mut handle, algorithm.as_ptr(), std::ptr::null(), 0) < 0
            {
                return Err("SHA256 provider unavailable".into());
            }
            let result = BCryptHash(
                handle,
                std::ptr::null(),
                0,
                bytes.as_ptr(),
                size,
                digest.as_mut_ptr(),
                32,
            );
            BCryptCloseAlgorithmProvider(handle, 0);
            if result < 0 {
                return Err("Executable SHA256 failed".into());
            }
        }
        Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
    }

    fn relative_call(site: usize, destination: usize) -> Result<[u8; 5], String> {
        let relative = i32::try_from(destination as i128 - (site as i128 + 5))
            .map_err(|_| "Relay outside CALL rel32 range")?;
        let mut bytes = [0xe8, 0, 0, 0, 0];
        bytes[1..].copy_from_slice(&relative.to_le_bytes());
        Ok(bytes)
    }
    fn relative_jump(site: usize, destination: usize) -> Result<[u8; 5], String> {
        let mut bytes = relative_call(site, destination)?;
        bytes[0] = 0xe9;
        Ok(bytes)
    }
    unsafe fn allocate_near(site: usize) -> Result<usize, String> {
        // Resident private pages: never patch shared game constants in .rdata.
        let anchor = site & !0xffff;
        for distance in (0x10000..0x70000000usize).step_by(0x10000) {
            for address in [anchor.checked_sub(distance), anchor.checked_add(distance)]
                .into_iter()
                .flatten()
            {
                if address < 0x10000 {
                    continue;
                }
                let block = VirtualAlloc(address as *mut c_void, 0x1000, 0x3000, 0x04);
                if !block.is_null() {
                    return Ok(block as usize);
                }
            }
        }
        Err("Cannot allocate nearby native page".into())
    }
    unsafe fn relay(site: usize, destination: usize) -> Result<usize, String> {
        let block = allocate_near(site)?;
        let mut code = [0u8; 14];
        code[..6].copy_from_slice(&[0xff, 0x25, 0, 0, 0, 0]);
        code[6..].copy_from_slice(&destination.to_le_bytes());
        std::ptr::copy_nonoverlapping(code.as_ptr(), block as *mut u8, code.len());
        let mut old = 0;
        if VirtualProtect(block as *mut c_void, 0x1000, 0x20, &mut old) == 0 {
            return Err("Cannot make relay executable".into());
        }
        if FlushInstructionCache(GetCurrentProcess(), block as *const c_void, code.len()) == 0 {
            return Err("Cannot flush relay instruction cache".into());
        }
        relative_call(site, block)?;
        Ok(block)
    }
    pub(super) fn install() -> Result<(), String> {
        unsafe {
            let module = GetModuleHandleW(std::ptr::null());
            if module.is_null() {
                return Err("Executable module unavailable".into());
            }
            let mut path = vec![0u16; 32768];
            let length = GetModuleFileNameW(module, path.as_mut_ptr(), path.len() as u32) as usize;
            if length == 0 || length >= path.len() {
                return Err("Executable path unavailable".into());
            }
            let bytes = std::fs::read(OsString::from_wide(&path[..length]))
                .map_err(|e| format!("Cannot verify executable: {e}"))?;
            if sha256(&bytes)? != EXPECTED_SHA {
                return Err("Executable fingerprint differs; no hooks installed".into());
            }
            let base = module as usize;
            let pe_offset = std::ptr::read_unaligned((base + 0x3c) as *const u32) as usize;
            if pe_offset > 0x1000
                || std::ptr::read_unaligned((base + pe_offset) as *const u32) != 0x4550
                || std::ptr::read_unaligned((base + pe_offset + 8) as *const u32) != PE_TIMESTAMP
                || std::ptr::read_unaligned((base + pe_offset + 0x50) as *const u32) != IMAGE_SIZE
            {
                return Err("Loaded PE identity differs; no hooks installed".into());
            }
            if !crate::native_profile::verify_layout(base) {
                return Err("Native layout consumers differ; no hooks installed".into());
            }
            for (rva, expected) in [
                (WORKER_ORIGINAL, WORKER_PROLOGUE.as_slice()),
                (VIEW_ORIGINAL, VIEW_PROLOGUE.as_slice()),
                (MOVE_ORIGINAL, MOVE_PROLOGUE.as_slice()),
                (STOP_EVENT, STOP_PROLOGUE.as_slice()),
                (CANCEL_RECALL_EVENT, CANCEL_RECALL_PROLOGUE.as_slice()),
                (INPUT_ORIGINAL, INPUT_PROLOGUE.as_slice()),
                (ATTACK_ORIGINAL, ATTACK_PROLOGUE.as_slice()),
                (SHADER_ORIGINAL, SHADER_PROLOGUE.as_slice()),
                (GOLD_GETTER, GOLD_GETTER_BYTES.as_slice()),
                (BUILD_LEN_SITE, BUILD_LEN_BYTES.as_slice()),
                (BUILD_PTR_SITE, BUILD_PTR_BYTES.as_slice()),
                (SKILL_ORIGINALS[0], SKILL_PROLOGUE.as_slice()),
                (SKILL_ORIGINALS[1], SKILL_PROLOGUE.as_slice()),
                (SKILL_ORIGINALS[2], SKILL_PROLOGUE.as_slice()),
                SKILL_QUEUE_ANCHORS[0],
                SKILL_QUEUE_ANCHORS[1],
                SKILL_QUEUE_ANCHORS[2],
                (STEER_ORIGINAL, STEER_PROLOGUE.as_slice()),
                (DIRECT_STEP, DIRECT_PROLOGUE.as_slice()),
                (AIM_ORIGINAL, AIM_PROLOGUE.as_slice()),
                (OUTLINE_ORIGINAL, OUTLINE_PROLOGUE.as_slice()),
                (OUTLINE_LINE_SITE, OUTLINE_LINE_BYTES.as_slice()),
                (OUTLINE_DROP_COMMAND, OUTLINE_DROP_BYTES.as_slice()),
                (OUTLINE_DROP_TABLE, OUTLINE_DROP_TABLE_BYTES.as_slice()),
                (OUTLINE_DROP_CALLER, OUTLINE_DROP_CALL_BYTES.as_slice()),
                (OUTLINE_NINEPATCH_SITE, OUTLINE_NINEPATCH_BYTES.as_slice()),
                (0x21725a0, OUTLINE_PARAM_PROLOGUE.as_slice()),
                (0x2171ae0, OUTLINE_PARAM_PROLOGUE.as_slice()),
                AIM_WRITES[0],
                AIM_WRITES[1],
            ] {
                if std::slice::from_raw_parts((base + rva) as *const u8, expected.len()) != expected
                {
                    return Err("Native entry differs or another mod hooked it".into());
                }
            }
            let specs = [
                (
                    WORKER_SITE,
                    WORKER_BYTES,
                    worker_hook as *const () as usize,
                    false,
                ),
                (
                    VIEW_SITE,
                    VIEW_BYTES,
                    view_hook as *const () as usize,
                    false,
                ),
                (MOVE_SITE, MOVE_BYTES, move_hook as *const () as usize, true),
                (
                    INPUT_SITE,
                    INPUT_BYTES,
                    input_hook as *const () as usize,
                    false,
                ),
                (
                    ATTACK_SITE,
                    ATTACK_BYTES,
                    attack_hook as *const () as usize,
                    false,
                ),
                (
                    SHADER_SITE,
                    SHADER_BYTES,
                    shader_hook as *const () as usize,
                    false,
                ),
                (
                    SKILL_SITES[0],
                    SKILL_BYTES[0],
                    skill_q_hook as *const () as usize,
                    false,
                ),
                (
                    SKILL_SITES[1],
                    SKILL_BYTES[1],
                    skill_w_hook as *const () as usize,
                    false,
                ),
                (
                    SKILL_SITES[2],
                    SKILL_BYTES[2],
                    skill_r_hook as *const () as usize,
                    false,
                ),
                (
                    STEER_SITE,
                    STEER_BYTES,
                    steer_hook as *const () as usize,
                    false,
                ),
                (
                    AUTO_ATTACK_SITE,
                    AUTO_ATTACK_BYTES,
                    auto_attack_hook as *const () as usize,
                    false,
                ),
                (AIM_SITE, AIM_BYTES, aim_hook as *const () as usize, false),
            ];
            for (rva, expected, _, _) in specs {
                if read_call(base + rva) != expected {
                    return Err("Native branch bytes differ; no hooks installed".into());
                }
            }
            for (rva, expected) in OUTLINE_SITES {
                if read_call(base + rva) != expected {
                    return Err("Unit renderer CALL bytes differ; no hooks installed".into());
                }
            }
            let mut pinned = std::ptr::null_mut();
            if GetModuleHandleExW(0x05, worker_hook as *const () as *const u16, &mut pinned) == 0 {
                return Err("Cannot pin adapter DLL for process lifetime".into());
            }
            let constants_address = allocate_near(base + SHADER_SITE)?;
            let minimap_plan = crate::minimap::native::plan(base, constants_address)?;
            if !minimap_plan.verify() {
                return Err(
                    "Minimap operands or shared source constants differ; no hooks installed".into(),
                );
            }
            std::ptr::copy_nonoverlapping(
                minimap_plan.constants.as_ptr(),
                constants_address as *mut u8,
                minimap_plan.constants.len(),
            );
            let mut constants_old = 0;
            if VirtualProtect(
                constants_address as *mut c_void,
                0x1000,
                0x02,
                &mut constants_old,
            ) == 0
            {
                return Err("Cannot protect private minimap constants".into());
            }
            let mut patches: Vec<(usize, Vec<u8>, Vec<u8>)> = Vec::new();
            for (rva, expected, destination, jump) in specs {
                let site = base + rva;
                let target = relay(site, destination)?;
                let patch = if jump {
                    relative_jump(site, target)?
                } else {
                    relative_call(site, target)?
                };
                patches.push((site, expected.to_vec(), patch.to_vec()));
            }
            for (rva, expected) in OUTLINE_SITES {
                let site = base + rva;
                let target = relay(site, outline_hook as *const () as usize)?;
                let patch = relative_call(site, target)?;
                patches.push((site, expected.to_vec(), patch.to_vec()));
            }
            let outline_end = patches.len();
            for p in &minimap_plan.patches {
                patches.push((p.site, p.expected.clone(), p.bytes.clone()));
            }
            // Protect each page once. Multiple edits in one page must not save
            // an already-writable protection and leave that page writable.
            let mut pages = std::collections::BTreeSet::new();
            for (site, _, bytes) in &patches {
                pages.insert(*site & !0xfff);
                pages.insert((*site + bytes.len() - 1) & !0xfff);
            }
            let mut protected: Vec<(usize, u32)> = Vec::new();
            for page in pages {
                let mut old = 0;
                if VirtualProtect(page as *mut c_void, 0x1000, 0x40, &mut old) == 0 {
                    for (page, old) in &protected {
                        let mut ignored = 0;
                        VirtualProtect(*page as *mut c_void, 0x1000, *old, &mut ignored);
                    }
                    return Err("Cannot protect all native patch pages; none changed".into());
                }
                protected.push((page, old));
            }
            ORIGINAL_WORKER.store(base + WORKER_ORIGINAL, Ordering::Release);
            ORIGINAL_VIEW.store(base + VIEW_ORIGINAL, Ordering::Release);
            ORIGINAL_MOVE.store(base + MOVE_ORIGINAL, Ordering::Release);
            NOTIFY_STOP.store(base + STOP_EVENT, Ordering::Release);
            NOTIFY_CANCEL_RECALL.store(base + CANCEL_RECALL_EVENT, Ordering::Release);
            ORIGINAL_INPUT.store(base + INPUT_ORIGINAL, Ordering::Release);
            ORIGINAL_ATTACK.store(base + ATTACK_ORIGINAL, Ordering::Release);
            ORIGINAL_SHADER.store(base + SHADER_ORIGINAL, Ordering::Release);
            ORIGINAL_STEER.store(base + STEER_ORIGINAL, Ordering::Release);
            ORIGINAL_DIRECT_STEP.store(base + DIRECT_STEP, Ordering::Release);
            ORIGINAL_AIM.store(base + AIM_ORIGINAL, Ordering::Release);
            ORIGINAL_OUTLINE.store(base + OUTLINE_ORIGINAL, Ordering::Release);
            for i in 0..3 {
                ORIGINAL_SKILLS[i].store(base + SKILL_ORIGINALS[i], Ordering::Release);
            }
            for (site, _, patch) in &patches {
                std::ptr::copy_nonoverlapping(patch.as_ptr(), *site as *mut u8, patch.len());
            }
            let flushed = patches.iter().all(|(site, _, bytes)| {
                FlushInstructionCache(GetCurrentProcess(), *site as *const c_void, bytes.len()) != 0
            });
            if !flushed {
                for (site, expected, _) in &patches {
                    std::ptr::copy_nonoverlapping(
                        expected.as_ptr(),
                        *site as *mut u8,
                        expected.len(),
                    );
                    FlushInstructionCache(
                        GetCurrentProcess(),
                        *site as *const c_void,
                        expected.len(),
                    );
                }
            }
            let mut restored = true;
            for (page, old) in &protected {
                let mut ignored = 0;
                restored &= VirtualProtect(*page as *mut c_void, 0x1000, *old, &mut ignored) != 0;
            }
            if !flushed {
                return Err("Cache flush failed; branches restored".into());
            }
            if !restored {
                return Err("Protection restore failed; relays remain pass-through".into());
            }
            if patches.iter().any(|(site, _, patch)| {
                std::slice::from_raw_parts(*site as *const u8, patch.len()) != patch
            }) {
                return Err("Branch readback mismatch; coordinator disabled".into());
            }
            let _ = PATCHES.set(PatchRecord {
                worker: patches[0].0,
                worker_bytes: patches[0].2.as_slice().try_into().unwrap(),
                viewer: patches[1].0,
                viewer_bytes: patches[1].2.as_slice().try_into().unwrap(),
                movement: patches[2].0,
                movement_bytes: patches[2].2.as_slice().try_into().unwrap(),
                input: patches[3].0,
                input_bytes: patches[3].2.as_slice().try_into().unwrap(),
                attack: patches[4].0,
                attack_bytes: patches[4].2.as_slice().try_into().unwrap(),
                shader: patches[5].0,
                shader_bytes: patches[5].2.as_slice().try_into().unwrap(),
                skills: std::array::from_fn(|i| {
                    (
                        patches[6 + i].0,
                        patches[6 + i].2.as_slice().try_into().unwrap(),
                    )
                }),
                steering: (patches[9].0, patches[9].2.as_slice().try_into().unwrap()),
                auto_attack: (patches[10].0, patches[10].2.as_slice().try_into().unwrap()),
                aim: (patches[11].0, patches[11].2.as_slice().try_into().unwrap()),
                outlines: patches[12..outline_end]
                    .iter()
                    .map(|(site, _, bytes)| (*site, bytes.as_slice().try_into().unwrap()))
                    .collect(),
                minimap: patches[outline_end..]
                    .iter()
                    .map(|(site, _, bytes)| (*site, bytes.clone()))
                    .collect(),
                minimap_constants: (constants_address, minimap_plan.constants),
            });
            if let Some(shared) = SHARED.get() {
                shared.logger.write("MINIMAP installed content=352 padding=4 wide_origin=1564,724 frame=360; native markers/fog/camera/input scaled; private constants read-only");
                shared.logger.write(&format!("NATIVE PATCH_READBACK worker_rva={WORKER_SITE:x} viewer_rva={VIEW_SITE:x} move_rva={MOVE_SITE:x} input_rva={INPUT_SITE:x} attack_rva={ATTACK_SITE:x} auto_attack_rva={AUTO_ATTACK_SITE:x} shader_rva={SHADER_SITE:x} skill_rvas={SKILL_SITES:x?} steering_rva={STEER_SITE:x} aim_rva={AIM_SITE:x} outline_sites={} verified=true", OUTLINE_SITES.len()));
            }
            OUTLINE_READY.store(true, Ordering::Release);
            let shop = install_shop(base);
            if let Some(shared) = SHARED.get() {
                shared.logger.write(&match &shop {
                    Ok(()) => format!("SHOP HOOKS installed upgrade_thunk_rva={SHOP_UPGRADE_THUNK:x} new_thunk_rva={SHOP_NEW_THUNK:x}; native answers unless Manual shopping applies"),
                    Err(e) => format!("SHOP HOOKS unavailable: {e}; Manual shopping disabled, other hooks unaffected"),
                });
            }
            Ok(())
        }
    }

    type ShaderFn = unsafe extern "system" fn(usize, usize, *const u8, usize) -> usize;
    type OutlineRenderFn =
        unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, usize, usize) -> usize;
    type FloatParamFn = unsafe extern "system" fn(usize, usize, *const u8, usize, f32) -> usize;
    type ColorParamFn = unsafe extern "system" fn(usize, usize, *const u8, usize, usize) -> usize;

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct NativeCommandVec {
        capacity: usize,
        pointer: usize,
        length: usize,
    }
    // Source type and 0xd0-byte stride are checked against the game's own
    // Vec grow sites. The alignment also satisfies the temporary command ABI.
    #[repr(C, align(16))]
    struct NativeCommand([u8; 0xd0]);
    const COMMAND_SIZE: usize = std::mem::size_of::<NativeCommand>();
    const MAX_BODY_COMMANDS: usize = 64;
    type DropCommandFn = unsafe extern "system" fn(usize);

    fn outline_command_name(tag: usize) -> &'static str {
        [
            "SetCamera",
            "SetImageScale",
            "Svg",
            "Sprite",
            "NinePatch",
            "Mesh",
            "SpriteInstance",
            "Text",
            "DrawLine",
            "DrawLineEx",
            "RoundingBox",
            "FogOverlay",
            "Circle",
            "AnnularSector",
            "FilledPath",
            "StartMaskingLayer",
            "ApplyMaskingLayer",
            "AdjustCanvas",
            "RestoreCanvas",
        ]
        .get(tag)
        .copied()
        .unwrap_or("Unknown")
    }

    unsafe fn command_tag(command: *const u8) -> u64 {
        let raw = std::ptr::read_unaligned(command as *const u64);
        if raw & (1 << 63) != 0 {
            raw ^ (1 << 63)
        } else {
            7
        }
    }
    unsafe fn valid_command_vec(vector: NativeCommandVec) -> bool {
        vector.length <= MAX_BODY_COMMANDS
            && vector.capacity >= vector.length
            && vector.capacity <= 128
            && (vector.capacity == 0
                || (vector.pointer >= 0x10000 && vector.pointer.is_multiple_of(8)))
    }
    unsafe fn supported_outline_commands(vector: NativeCommandVec) -> bool {
        (0..vector.length)
            .all(|i| command_tag((vector.pointer + i * COMMAND_SIZE) as *const u8) <= 18)
    }
    unsafe fn record_outline_commands(vector: NativeCommandVec) {
        for i in 0..vector.length {
            let tag = command_tag((vector.pointer + i * COMMAND_SIZE) as *const u8) as usize;
            if tag > 18 {
                OUTLINE_UNKNOWN_TAG.store(tag, Ordering::Relaxed);
            }
            OUTLINE_TAG_COUNTS[tag.min(19)].fetch_add(1, Ordering::Relaxed);
        }
    }
    unsafe fn sprite_count(vector: NativeCommandVec) -> usize {
        (0..vector.length)
            .filter(|i| command_tag((vector.pointer + i * COMMAND_SIZE) as *const u8) == 3)
            .count()
    }
    /// Consume an independently rendered pass: move sprites to the supplied
    /// destination and drop every other known command through the game. With
    /// no destination, drop all commands for a normal-drawing fallback.
    /// Unknown tags are never passed to a destructor that may interpret them
    /// incorrectly. The caller disables the prototype on such a layout fault.
    unsafe fn take_outline_commands(
        vector: NativeCommandVec,
        destination: Option<*mut u8>,
        drop_command: DropCommandFn,
    ) -> usize {
        let mut moved = 0;
        for i in 0..vector.length {
            let source = (vector.pointer + i * COMMAND_SIZE) as *const u8;
            let tag = command_tag(source);
            if let Some(destination) = destination.filter(|_| tag == 3) {
                std::ptr::copy_nonoverlapping(
                    source,
                    destination.add(moved * COMMAND_SIZE),
                    COMMAND_SIZE,
                );
                moved += 1;
            } else if tag <= 18 {
                drop_command(source as usize);
            }
        }
        moved
    }
    unsafe fn free_command_buffer(vector: &mut NativeCommandVec, heap: *mut c_void) {
        if vector.capacity != 0 && HeapFree(heap, 0, vector.pointer as *mut c_void) == 0 {
            OUTLINE_READY.store(false, Ordering::Release);
        }
        *vector = NativeCommandVec::default();
    }
    unsafe fn discard_extra_passes(
        copies: &mut [NativeCommandVec],
        heap: *mut c_void,
        drop_command: DropCommandFn,
    ) {
        for copy in copies {
            // A malformed native Vec cannot be read or freed safely. Other
            // independently produced, valid passes are still released.
            if valid_command_vec(*copy) {
                take_outline_commands(*copy, None, drop_command);
                free_command_buffer(copy, heap);
            }
        }
    }
    unsafe fn rewrite_command(
        command: *mut u8,
        shader: &[u8],
        float_parameters: &[(&[u8], f32)],
        color: Option<[f32; 4]>,
    ) {
        let base = GetModuleHandleW(std::ptr::null()) as usize;
        let shader_fn: ShaderFn = std::mem::transmute(base + 0x1c91f0);
        let float_fn: FloatParamFn = std::mem::transmute(base + 0x21725a0);
        let color_fn: ColorParamFn = std::mem::transmute(base + 0x2171ae0);
        // Each setter consumes an owned command and writes its replacement.
        // Copying bytes to a temporary is a move here: the former source slot
        // is immediately overwritten and the temporary is never dropped.
        let mut temporary = std::mem::MaybeUninit::<NativeCommand>::uninit();
        let mut apply =
            |setter: unsafe extern "system" fn(usize, usize, *const u8, usize) -> usize,
             name: &[u8]| {
                std::ptr::copy_nonoverlapping(
                    command,
                    temporary.as_mut_ptr() as *mut u8,
                    COMMAND_SIZE,
                );
                setter(
                    command as usize,
                    temporary.as_ptr() as usize,
                    name.as_ptr(),
                    name.len(),
                );
            };
        apply(shader_fn, shader);
        for (key, value) in float_parameters {
            std::ptr::copy_nonoverlapping(command, temporary.as_mut_ptr() as *mut u8, COMMAND_SIZE);
            float_fn(
                command as usize,
                temporary.as_ptr() as usize,
                key.as_ptr(),
                key.len(),
                *value,
            );
        }
        if let Some(color) = color {
            std::ptr::copy_nonoverlapping(command, temporary.as_mut_ptr() as *mut u8, COMMAND_SIZE);
            color_fn(
                command as usize,
                temporary.as_ptr() as usize,
                b"flash_color".as_ptr(),
                11,
                color.as_ptr() as usize,
            );
        }
    }
    unsafe fn restyle_outline(vector: NativeCommandVec, pass: usize, weight: f32, color: [f32; 4]) {
        for i in 0..vector.length {
            let command = (vector.pointer + i * COMMAND_SIZE) as *mut u8;
            if command_tag(command) != 3 {
                continue;
            }
            rewrite_command(
                command,
                b"asset/base/shader/flash",
                &[(b"flash", 1.)],
                Some(color),
            );
            let (dx, dy) = [(-weight, 0.), (weight, 0.), (0., -weight), (0., weight)][pass];
            let x = command.add(0x78) as *mut f32;
            let y = command.add(0x7c) as *mut f32;
            *x += dx;
            *y += dy;
        }
    }
    unsafe extern "system" fn outline_hook(
        out: usize,
        view: usize,
        a3: usize,
        a4: usize,
        a5: usize,
        a6: usize,
        a7: usize,
        a8: usize,
    ) -> usize {
        let original: OutlineRenderFn =
            std::mem::transmute(ORIGINAL_OUTLINE.load(Ordering::Acquire));
        let result = original(out, view, a3, a4, a5, a6, a7, a8);
        crate::perf::hook(crate::perf::Hook::Outline);
        if !OUTLINE_READY.load(Ordering::Acquire) || view < 0x10000 {
            return result;
        }
        let Ok(targets) = OUTLINE_TARGETS.lock().map(|t| *t) else {
            return result;
        };
        if targets.hover.is_none() && targets.attack.is_none() {
            return result;
        }
        // 0.6.3's EntityView shifted 0x18 bytes from the preserved type map:
        // its own id is +0x100, while the hash-map key at view-8 is only valid
        // for the map-owned instance, not every champion-specific view copy.
        let x = std::ptr::read_unaligned((view + 0x17c) as *const f32);
        let y = std::ptr::read_unaligned((view + 0x180) as *const f32);
        let near = |unit: crate::combat::Unit| {
            x.is_finite()
                && y.is_finite()
                && (x - unit.position.0 as f32 / 1000.).abs() < 8.
                && (y - unit.position.1 as f32 / 1000.).abs() < 8.
        };
        if ![targets.hover, targets.attack]
            .into_iter()
            .flatten()
            .any(near)
        {
            return result;
        }
        let id = std::ptr::read_unaligned((view + 0x100) as *const usize);
        let Some((unit, role, weight)) = targets.for_unit(id, std::time::Instant::now()) else {
            OUTLINE_ID_MISSES.fetch_add(1, Ordering::Relaxed);
            return result;
        };
        if !near(unit) {
            return result;
        }
        let normal = std::ptr::read_unaligned(out as *const NativeCommandVec);
        if !valid_command_vec(normal) {
            OUTLINE_BAD_VEC.fetch_add(1, Ordering::Relaxed);
            OUTLINE_UNSUPPORTED.fetch_add(1, Ordering::Relaxed);
            return result;
        }
        record_outline_commands(normal);
        if !supported_outline_commands(normal) {
            OUTLINE_UNSUPPORTED.fetch_add(1, Ordering::Relaxed);
            return result;
        }
        let sprites = sprite_count(normal);
        if sprites == 0 {
            OUTLINE_NO_SPRITE.fetch_add(1, Ordering::Relaxed);
            return result;
        }
        if sprites < normal.length {
            OUTLINE_MIXED.fetch_add(1, Ordering::Relaxed);
        }
        let base = GetModuleHandleW(std::ptr::null()) as usize;
        let drop_command: DropCommandFn = std::mem::transmute(base + OUTLINE_DROP_COMMAND);
        let rgba = crate::combat::hover_color(unit);
        let color = [24, 16, 8, 0].map(|shift| ((rgba >> shift) & 255) as f32 / 255.);
        let count = 4;
        // Reserve for the bounded maximum BEFORE producing any owned extra
        // commands. No allocation/reallocation can then strand a valid pass.
        let capacity = normal.length + MAX_BODY_COMMANDS * count;
        let expected_bytes = capacity * COMMAND_SIZE;
        let heap = GetProcessHeap();
        let merged = HeapAlloc(heap, 0, expected_bytes) as *mut u8;
        if merged.is_null() {
            return result;
        }
        let mut copies = [NativeCommandVec::default(); 4];
        let mut failed = false;
        for copy in &mut copies {
            original(copy as *mut _ as usize, view, a3, a4, a5, a6, a7, a8);
            if !valid_command_vec(*copy) {
                OUTLINE_BAD_VEC.fetch_add(1, Ordering::Relaxed);
                OUTLINE_READY.store(false, Ordering::Release);
                failed = true;
                break;
            }
            if !supported_outline_commands(*copy) {
                record_outline_commands(*copy);
                OUTLINE_UNSUPPORTED.fetch_add(1, Ordering::Relaxed);
                OUTLINE_READY.store(false, Ordering::Release);
                failed = true;
                break;
            }
            if sprite_count(*copy) == 0 {
                OUTLINE_NO_SPRITE.fetch_add(1, Ordering::Relaxed);
                failed = true;
                break;
            }
        }
        if failed {
            // Original vector and commands have not been moved or altered.
            // Release independently owned extras instead of drawing duplicates.
            discard_extra_passes(&mut copies, heap, drop_command);
            HeapFree(heap, 0, merged as *mut c_void);
            return result;
        }
        let mut offset = 0;
        for (pass, copy) in copies.iter_mut().enumerate() {
            restyle_outline(*copy, pass, weight, color);
            offset +=
                take_outline_commands(*copy, Some(merged.add(offset * COMMAND_SIZE)), drop_command);
            free_command_buffer(copy, heap);
        }
        std::ptr::copy_nonoverlapping(
            normal.pointer as *const u8,
            merged.add(offset * COMMAND_SIZE),
            normal.length * COMMAND_SIZE,
        );
        if normal.capacity != 0 && HeapFree(heap, 0, normal.pointer as *mut c_void) == 0 {
            OUTLINE_READY.store(false, Ordering::Release);
        }
        std::ptr::write_unaligned(
            out as *mut NativeCommandVec,
            NativeCommandVec {
                capacity,
                pointer: merged as usize,
                length: offset + normal.length,
            },
        );
        OUTLINE_MATCHES.fetch_add(1, Ordering::Relaxed);
        match role {
            OutlineRole::Click => &OUTLINE_CLICK_DRAWS,
            OutlineRole::Hover => &OUTLINE_HOVER_DRAWS,
            OutlineRole::Attack => &OUTLINE_TARGET_DRAWS,
        }
        .fetch_add(1, Ordering::Relaxed);
        result
    }
    unsafe extern "system" fn shader_hook(
        out: usize,
        command: usize,
        name: *const u8,
        len: usize,
    ) -> usize {
        let original: ShaderFn = std::mem::transmute(ORIGINAL_SHADER.load(Ordering::Acquire));
        let grey = b"asset/base/shader/greyscale";
        if DEATH_GREYSCALE.load(Ordering::Acquire) && PATCHES.get().is_some() {
            original(out, command, grey.as_ptr(), grey.len())
        } else {
            original(out, command, name, len)
        }
    }

    // Sender::send: output Result pointer, sender pointer, owned frame pointer.
    // The native caller inspects the output memory, not RAX, after return.
    type WorkerFn = unsafe extern "system" fn(usize, usize, usize);
    // Verified: four GP arguments, four stack pointer/usize arguments, then
    // stack f32 dt. Native caller ignores the result (update returns unit).
    type ViewFn =
        unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, usize, usize, f32);
    type MoveFn = unsafe extern "system" fn(usize, u64, u64, usize);
    type StopEventFn = unsafe extern "system" fn(usize, usize);
    type AttackFn = unsafe extern "system" fn(usize, usize, usize);
    type SteerFn =
        unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, usize, u64, u64, usize);
    type DirectStepFn =
        unsafe extern "system" fn(usize, usize, usize, usize, usize, u64, u64, usize);
    type AimFn = unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, usize);
    static PRESERVED_AIMS: AtomicUsize = AtomicUsize::new(0);
    unsafe fn manual_aim_words(
        shared: &Shared,
        actor: usize,
        target: usize,
    ) -> Option<(crate::native_timing::MatchKey, [usize; 3])> {
        let key = owned_key(shared, actor)?;
        if target == 0 || !target.is_multiple_of(8) {
            return None;
        }
        let words = std::ptr::read_unaligned(target as *const [usize; 3]);
        // Low dword is the tag; upper dword is enum padding.
        matches!(words[0] as u32, 1 | 2).then_some((key, words))
    }
    /// The native routine borrows the POD target only for this call. Run its
    /// stat/RNG evaluation normally, but give it a copy for manual ground/direction
    /// casts. The effect below the call still receives the original cursor aim.
    unsafe fn evaluate_aim(
        original: AimFn,
        mut args: [usize; 7],
        preserve: bool,
    ) -> Option<[usize; 3]> {
        if !preserve {
            original(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6],
            );
            return None;
        }
        let mut copy = std::ptr::read_unaligned(args[6] as *const [usize; 3]);
        args[6] = copy.as_mut_ptr() as usize;
        original(
            args[0], args[1], args[2], args[3], args[4], args[5], args[6],
        );
        Some(copy)
    }
    #[allow(clippy::too_many_arguments)]
    unsafe extern "system" fn aim_hook(
        rng: usize,
        sim: usize,
        table: usize,
        navigation: usize,
        actor: usize,
        effect: usize,
        target: usize,
    ) {
        crate::perf::hook(crate::perf::Hook::Aim);
        let original: AimFn = std::mem::transmute(ORIGINAL_AIM.load(Ordering::Acquire));
        let manual = catch_unwind(AssertUnwindSafe(|| {
            let shared = SHARED.get()?;
            PATCHES.get()?;
            manual_aim_words(shared, actor, target)
        }))
        .unwrap_or(None);
        let corrected = evaluate_aim(
            original,
            [rng, sim, table, navigation, actor, effect, target],
            manual.is_some(),
        );
        if let (Some((key, before)), Some(corrected)) = (manual, corrected) {
            let count = PRESERVED_AIMS.fetch_add(1, Ordering::Relaxed) + 1;
            if (before[1..] != corrected[1..] || count <= 3) && SHARED.get().is_some() {
                let _ = catch_unwind(AssertUnwindSafe(|| {
                    SHARED.get().unwrap().logger.write(&format!(
                        "AIM PRESERVED actor={actor} key={key:?} kind={} cursor=({},{}) native_suggestion=({},{}) count={count}",
                        before[0] as u32, before[1] as i64, before[2] as i64,
                        corrected[1] as i64, corrected[2] as i64,
                    ));
                }));
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe extern "system" fn steer_hook(
        world: usize,
        navigation: usize,
        bounds: usize,
        actor: usize,
        x: usize,
        y: usize,
        speed: usize,
        goal_x: u64,
        goal_y: u64,
        events: usize,
    ) {
        STEER_ENTRIES.fetch_add(1, Ordering::Relaxed);
        crate::perf::hook(crate::perf::Hook::Steer);
        let original: SteerFn = std::mem::transmute(ORIGINAL_STEER.load(Ordering::Acquire));
        let direct = catch_unwind(AssertUnwindSafe(|| {
            let shared = SHARED.get()?;
            PATCHES.get()?;
            let key = owned_steering(shared, actor, x, y)?;
            OWNED_STEER_ENTRIES.fetch_add(1, Ordering::Relaxed);
            // These fields belong to the current borrowed native entity. No
            // pointer from an earlier input call or published frame is reused.
            let from = (
                std::ptr::read_unaligned(x as *const u64),
                std::ptr::read_unaligned(y as *const u64),
            );
            shared
                .movement
                .direct_segment(key, actor, from, (goal_x, goal_y), &shared.logger)
                .then_some(())
        }))
        .unwrap_or(None)
        .is_some();
        if direct {
            DIRECT_STEPS.fetch_add(1, Ordering::Relaxed);
            let step: DirectStepFn =
                std::mem::transmute(ORIGINAL_DIRECT_STEP.load(Ordering::Acquire));
            step(world, actor, x, y, speed, goal_x, goal_y, events);
        } else {
            original(
                world, navigation, bounds, actor, x, y, speed, goal_x, goal_y, events,
            );
        }
    }

    fn owned_key(shared: &Shared, actor: usize) -> Option<crate::native_timing::MatchKey> {
        let key = shared.abilities.selected_key(actor)?;
        shared.timing.allows_input(key).then_some(key)
    }
    unsafe fn owned_entity(
        shared: &Shared,
        entity: usize,
    ) -> Option<(crate::native_timing::MatchKey, usize)> {
        let actor = std::ptr::read_unaligned((entity + 0x5b8) as *const usize);
        let key = owned_key(shared, actor)?;
        (std::ptr::read_unaligned((entity + 0x68) as *const u32) == 15).then_some((key, actor))
    }
    unsafe fn owned_steering(
        shared: &Shared,
        actor: usize,
        x: usize,
        y: usize,
    ) -> Option<crate::native_timing::MatchKey> {
        // The fingerprinted caller establishes x/y as adjacent EntityData
        // position fields. Recover only this call's current borrow.
        let key = owned_key(shared, actor)?;
        let entity = x.checked_sub(0x658)?;
        if y != x.checked_add(8)? || owned_entity(shared, entity) != Some((key, actor)) {
            let failures = STEER_POINTER_REJECTIONS.fetch_add(1, Ordering::Relaxed);
            if failures < 8 {
                shared.logger.write(&format!("MOVEMENT STEERING_REJECT actor={actor} reason=current-position-fields-mismatch; native navigation retained"));
            }
            return None;
        }
        Some(key)
    }
    fn take_stop_ticket(key: crate::native_timing::MatchKey, actor: usize) -> Option<StopTicket> {
        STOP_TICKET.with(|slot| {
            slot.get()
                .filter(|t| t.actor == actor && t.key == key)
                .inspect(|_| slot.set(None))
        })
    }
    /// Copy scalar state from the live worker only; never retain native objects.
    unsafe fn observe_owned_abilities(
        shared: &Shared,
        entity: usize,
        key: crate::native_timing::MatchKey,
        actor: usize,
    ) -> bool {
        if owned_entity(shared, entity) != Some((key, actor)) {
            return false;
        }
        for line in ATTACK_TRACE.sample(key, actor, attack_snapshot(entity)) {
            shared.logger.write(&line);
        }
        // Use the same borrowed state for readiness and the native action/count
        // snapshot, including immediately after a consumer spends cooldown.
        shared.abilities.observe_actor(
            key,
            Some(actor),
            Some((
                std::ptr::read_unaligned((entity + 0x658) as *const u64),
                std::ptr::read_unaligned((entity + 0x660) as *const u64),
            )),
            [0xb8, 0xc0, 0xc8].map(|o| std::ptr::read_unaligned((entity + o) as *const usize)),
        );
        shared.abilities.observe_level(
            key,
            actor,
            std::ptr::read_unaligned((entity + 0x5c0) as *const usize),
        );
        let metadata = read_ability_metadata(entity);
        shared
            .abilities
            .observe_metadata(key, actor, metadata, &shared.logger);
        if let Some(base) = verified_base().filter(|_| shared.abilities.geometry_due(key, actor)) {
            let offsets = [0x4c0, 0x4f8, 0x530];
            let geometry = std::array::from_fn(|slot| {
                if metadata[slot].is_some() {
                    crate::native_preview::read(base, entity, offsets[slot])
                } else {
                    crate::skill_preview::Geometry::default()
                }
            });
            shared
                .abilities
                .observe_geometry(key, actor, geometry, &shared.logger);
        }
        let charges = [0x578, 0x588, 0x598].map(|offset| read_charges(entity, offset));
        if shared.abilities.observe_native(
            key,
            actor,
            std::ptr::read_unaligned((entity + 0x70) as *const usize),
            charges,
        ) {
            shared.logger.write(&format!("ABILITY CHARGES actor={actor} Q/W/R={charges:?}; declared use count, not inferred capacity/cost"));
        }
        shared.movement.observe_action(
            std::ptr::read_unaligned((entity + 0x70) as *const usize),
            &shared.logger,
        );
        shared.movement.observe_attack_range(
            key,
            read_effect_metadata(entity, 0x488)
                .map(|d| d.range)
                .filter(|r| *r > 0),
            &shared.logger,
        );
        true
    }
    /// The Q/W/R consumers call these shared-borrow Action methods with
    /// (action userdata, EntityData/Stat). Current 0.6.3 consumers establish
    /// slots +90 (cooltime), +a8 (use count), CDR +3f8 (+464 for R),
    /// minimum Q=3 ticks, W/R=1. No calls for unlearned actions.
    /// Neither the object nor its vtable is retained beyond this hook.
    unsafe fn read_charges(entity: usize, offset: usize) -> Option<crate::abilities::Charges> {
        type Read = unsafe extern "system" fn(usize, usize) -> usize;
        let level = std::ptr::read_unaligned((entity + 0x5c0) as *const usize);
        let unlock = match offset {
            0x578 => 1,
            0x588 => 3,
            0x598 => 5,
            _ => return None,
        };
        if level < unlock {
            return None;
        }
        let object = std::ptr::read_unaligned((entity + offset) as *const usize);
        let table = std::ptr::read_unaligned((entity + offset + 8) as *const usize);
        if object == 0 || table == 0 {
            return None;
        }
        let cool: Read =
            std::mem::transmute(std::ptr::read_unaligned((table + 0x90) as *const usize));
        let count: Read =
            std::mem::transmute(std::ptr::read_unaligned((table + 0xa8) as *const usize));
        let raw = cool(object, entity);
        let uses = count(object, entity);
        let mut cdr = std::ptr::read_unaligned((entity + 0x3f8) as *const i32);
        if offset == 0x598 {
            cdr = cdr.checked_add(std::ptr::read_unaligned((entity + 0x464) as *const i32))?;
        }
        let divisor = cdr.checked_add(100)?.max(1) as usize;
        let capacity = (raw.checked_mul(100)? / divisor).max(if offset == 0x578 { 3 } else { 1 });
        if !(1..=64).contains(&uses) || capacity > 3_600_000 {
            return None;
        }
        let cost = capacity / uses;
        (cost > 0).then_some(crate::abilities::Charges {
            capacity,
            cost,
            uses: if raw == 0 { 1 } else { uses },
        })
    }
    unsafe fn read_ability_metadata(entity: usize) -> [Option<crate::abilities::Descriptor>; 3] {
        [0x4c0, 0x4f8, 0x530].map(|offset| read_effect_metadata(entity, offset))
    }
    unsafe fn read_effect_metadata(
        entity: usize,
        offset: usize,
    ) -> Option<crate::abilities::Descriptor> {
        let level = std::ptr::read_unaligned((entity + 0x5c0) as *const u64);
        let bonus = std::ptr::read_unaligned((entity + 0x430) as *const u64);
        let data = entity + offset;
        let casting = std::ptr::read_unaligned((data + 0x30) as *const u32);
        if casting > 3 {
            return None;
        }
        crate::abilities::Descriptor::from_fields(
            casting,
            std::ptr::read_unaligned((data + 0x28) as *const u32),
            std::ptr::read_unaligned((data + 0x10) as *const u64),
            std::ptr::read_unaligned((data + 0x18) as *const u64),
            level,
            bonus,
        )
    }
    unsafe extern "system" fn attack_hook(entity: usize, input: usize, events: usize) {
        attack_hook_from("input", entity, input, events);
    }
    unsafe extern "system" fn auto_attack_hook(entity: usize, input: usize, events: usize) {
        attack_hook_from("native-auto", entity, input, events);
    }
    unsafe fn attack_hook_from(source: &str, entity: usize, input: usize, events: usize) {
        crate::perf::hook(crate::perf::Hook::Attack);
        let original: AttackFn = std::mem::transmute(ORIGINAL_ATTACK.load(Ordering::Acquire));
        let selected = SHARED.get().and_then(|shared| {
            catch_unwind(AssertUnwindSafe(|| {
                let (key, actor) = owned_entity(shared, entity)?;
                observe_owned_abilities(shared, entity, key, actor);
                if source == "input"
                    && take_stop_ticket(key, actor).is_some_and(|t| t.cancel_recall)
                {
                    cancel_recall(shared, entity, actor, events);
                }
                Some((
                    shared,
                    key,
                    actor,
                    std::ptr::read_unaligned((entity + 0x2b0) as *const usize),
                    std::ptr::read_unaligned((entity + 0xb0) as *const usize),
                    attack_snapshot(entity),
                ))
            }))
            .unwrap_or_else(|_| {
                shared
                    .timing
                    .cancel("Ability observation adapter panic", &shared.logger);
                None
            })
        });
        original(entity, input, events);
        if let Some((shared, key, actor, previous_len, before_cooldown, before)) = selected {
            let after = attack_snapshot(entity);
            if after.cooldown > before_cooldown {
                let start = crate::attack_trace::Start {
                    before,
                    after,
                    kind: std::ptr::read_unaligned((entity + 0x4b4) as *const u32),
                    start_timing: (std::ptr::read_unaligned((entity + 0x4b8) as *const i32) != -1)
                        .then(|| std::ptr::read_unaligned((entity + 0x4a8) as *const u64)),
                    speed: std::ptr::read_unaligned((entity + 0x3f4) as *const i32),
                    queued_delay: queued_attack_delay(entity, previous_len),
                };
                for line in ATTACK_TRACE.start(key, actor, source, start) {
                    shared.logger.write(&line);
                }
            }
            if let Some(hit) = locked_attack_hit_tick(entity, previous_len) {
                let cooldown = std::ptr::read_unaligned((entity + 0xb0) as *const usize);
                shared
                    .abilities
                    .observe_attack_started(key, actor, hit, cooldown);
                shared.logger.write(&format!("ATTACK START source={source} actor={actor} branch=locked hit_tick={hit} cooldown={cooldown}; release allowed only after the hit tick"));
            } else if std::ptr::read_unaligned((entity + 0xb0) as *const usize) > before_cooldown {
                shared.logger.write(&format!("ATTACK OBSERVED source={source} actor={actor} pending-effect-unconfirmed; preserving native animation"));
            }
            observe_owned_abilities(shared, entity, key, actor);
        }
    }
    /// Hit tick of a BaseAttack that locked action 3 in this call. The native
    /// consumer either locks action 3 without a queue entry or queues a delayed
    /// effect without locking (0.50 trace: Ninja locked, Gunner queued). The
    /// locked hit is the declared start timing (+4a8, present unless +4b8 is
    /// -1) scaled by the action's attack-speed factor (+80 = 100 + bonus).
    /// Release later requires elapsed (+78) strictly beyond this tick.
    unsafe fn locked_attack_hit_tick(entity: usize, previous_len: usize) -> Option<usize> {
        if std::ptr::read_unaligned((entity + 0x70) as *const usize) != 3
            || std::ptr::read_unaligned((entity + 0x78) as *const usize) != 0
            || std::ptr::read_unaligned((entity + 0x2b0) as *const usize) != previous_len
            || std::ptr::read_unaligned((entity + 0x4b4) as *const u32) != 0
            || std::ptr::read_unaligned((entity + 0x4b8) as *const i32) == -1
        {
            return None;
        }
        let start = std::ptr::read_unaligned((entity + 0x4a8) as *const usize);
        let factor = std::ptr::read_unaligned((entity + 0x80) as *const usize);
        if !(1..=10_000).contains(&factor) || start > 3600 {
            return None;
        }
        let hit = start * 100 / factor;
        (hit < 3600).then_some(hit)
    }
    /// Delay of the entry appended by this call, if exactly one was appended.
    unsafe fn queued_attack_delay(entity: usize, previous_len: usize) -> Option<usize> {
        let ptr = std::ptr::read_unaligned((entity + 0x2a8) as *const usize);
        let cap = std::ptr::read_unaligned((entity + 0x2a0) as *const usize);
        let len = std::ptr::read_unaligned((entity + 0x2b0) as *const usize);
        if ptr == 0
            || !ptr.is_multiple_of(8)
            || cap > 4096
            || len > cap
            || len != previous_len.checked_add(1)?
        {
            return None;
        }
        let delay = std::ptr::read_unaligned((ptr + previous_len * 0x38 + 0x10) as *const usize);
        (delay < 3600).then_some(delay)
    }
    /// Scalar copies only: action tag, its two payload words, pending-effect
    /// queue length and attack cooldown (0.6.3 EntityData layout).
    unsafe fn attack_snapshot(entity: usize) -> crate::attack_trace::Snapshot {
        crate::attack_trace::Snapshot {
            action: std::ptr::read_unaligned((entity + 0x70) as *const usize),
            elapsed: std::ptr::read_unaligned((entity + 0x78) as *const usize),
            counter: std::ptr::read_unaligned((entity + 0x80) as *const usize),
            queue: std::ptr::read_unaligned((entity + 0x2b0) as *const usize),
            cooldown: std::ptr::read_unaligned((entity + 0xb0) as *const usize),
        }
    }

    unsafe fn skill_hook(slot: usize, entity: usize, input: usize, events: usize) {
        let original: AttackFn = std::mem::transmute(ORIGINAL_SKILLS[slot].load(Ordering::Acquire));
        let selected = catch_unwind(AssertUnwindSafe(|| {
            SHARED.get().and_then(|shared| {
                let (key, actor) = owned_entity(shared, entity)?;
                observe_owned_abilities(shared, entity, key, actor);
                Some((shared, key, actor))
            })
        }))
        .unwrap_or(None);
        let offset = [0xb8, 0xc0, 0xc8][slot];
        let trace = selected
            .and_then(|(shared, _, _)| {
                shared
                    .abilities
                    .trace_point_stamp(slot)
                    .filter(|_| shared.timing.permit_native_trace())
            })
            .map(|stamp| {
                let words = std::array::from_fn::<_, 3, _>(|i| {
                    std::ptr::read_unaligned((input + i * 8) as *const u64)
                });
                let length = std::ptr::read_unaligned((entity + 0x2b0) as *const usize);
                (stamp.id, words, length)
            });
        let before = selected.map(|_| std::ptr::read_unaligned((entity + offset) as *const usize));
        original(entity, input, events);
        if let (Some((shared, key, actor)), Some(before)) = (selected, before) {
            if catch_unwind(AssertUnwindSafe(|| {
                if let Some((trace_id, words, previous_len)) = trace {
                    let ptr = std::ptr::read_unaligned((entity + 0x2a8) as *const usize);
                    let cap = std::ptr::read_unaligned((entity + 0x2a0) as *const usize);
                    let len = std::ptr::read_unaligned((entity + 0x2b0) as *const usize);
                    let queued = (ptr != 0 && ptr.is_multiple_of(8) && cap <= 4096 && len <= cap && len == previous_len + 1)
                        .then(|| std::array::from_fn::<_, 3, _>(|i| std::ptr::read_unaligned((ptr + previous_len * 0x38 + 0x20 + i * 8) as *const u64)));
                    shared.logger.write(&format!("ABILITY NATIVE_AIM trace_id={trace_id} slot={slot} actor={actor} input_words={words:?} queued_words={queued:?} queue_before={previous_len} queue_after={len}; queued intent, not projectile trajectory"));
                }
                let after = std::ptr::read_unaligned((entity + offset) as *const usize);
                let action = std::ptr::read_unaligned((entity + 0x70) as *const usize);
                shared.abilities.acknowledge_native(
                    key,
                    actor,
                    slot,
                    after > before,
                    action,
                    &shared.logger,
                );
                observe_owned_abilities(shared, entity, key, actor);
            }))
            .is_err()
            {
                shared
                    .timing
                    .cancel("Native skill acknowledgement panic", &shared.logger);
            }
        }
    }
    unsafe extern "system" fn skill_q_hook(entity: usize, input: usize, events: usize) {
        crate::perf::hook(crate::perf::Hook::Skill);
        skill_hook(0, entity, input, events);
    }
    unsafe extern "system" fn skill_w_hook(entity: usize, input: usize, events: usize) {
        crate::perf::hook(crate::perf::Hook::Skill);
        skill_hook(1, entity, input, events);
    }
    unsafe extern "system" fn skill_r_hook(entity: usize, input: usize, events: usize) {
        crate::perf::hook(crate::perf::Hook::Skill);
        skill_hook(2, entity, input, events);
    }

    unsafe fn can_stop_movement(entity: usize) -> bool {
        can_stop_action(entity, 2)
    }
    unsafe fn can_stop_action(entity: usize, action: usize) -> bool {
        if std::ptr::read_unaligned((entity + 0x68) as *const u32) != 15
            || std::ptr::read_unaligned((entity + 0x70) as *const usize) != action
        {
            return false;
        }
        let count = std::ptr::read_unaligned((entity + 0x2c8) as *const usize);
        if count > 512 {
            return false;
        }
        let effects = std::ptr::read_unaligned((entity + 0x2c0) as *const usize);
        if count != 0 && effects == 0 {
            return false;
        }
        // Preserve the native effect gate, including forced movement and CC.
        (0..count).all(|i| {
            let kind = std::ptr::read_unaligned((effects + i * 0x28) as *const u32);
            kind < 32 && (0x3b8u32 & (1u32 << kind)) != 0
        })
    }
    unsafe fn repeated_move(entity: usize, x: u64, y: u64, hold: bool) -> bool {
        !hold
            && can_stop_movement(entity)
            && std::ptr::read_unaligned((entity + 0x78) as *const u64) == x
            && std::ptr::read_unaligned((entity + 0x80) as *const u64) == y
    }
    unsafe fn stop_movement(
        entity: usize,
        actor: usize,
        events: usize,
        notify: StopEventFn,
    ) -> bool {
        if std::ptr::read_unaligned((entity + 0x70) as *const usize) == 0 {
            return true;
        }
        if !can_stop_movement(entity) {
            return false;
        }
        notify(events, actor);
        std::ptr::write_unaligned((entity + 0x70) as *mut usize, 0);
        true
    }
    unsafe fn stop_recall(entity: usize, actor: usize, events: usize, notify: StopEventFn) -> bool {
        if std::ptr::read_unaligned((entity + 0x68) as *const u32) != 15
            || std::ptr::read_unaligned((entity + 0x70) as *const usize) != 1
        {
            return false;
        }
        notify(events, actor);
        std::ptr::write_unaligned((entity + 0x70) as *mut usize, 0);
        true
    }
    unsafe fn cancel_recall(shared: &Shared, entity: usize, actor: usize, events: usize) {
        let notify: StopEventFn = std::mem::transmute(NOTIFY_CANCEL_RECALL.load(Ordering::Acquire));
        if stop_recall(entity, actor, events, notify) {
            shared.movement.observe_action(0, &shared.logger);
            shared.logger.write(&format!("RECALL NATIVE_CANCEL actor={actor} action=1->0; native return cancellation event emitted"));
        }
    }
    unsafe extern "system" fn move_hook(entity: usize, x: u64, y: u64, events: usize) {
        crate::perf::hook(crate::perf::Hook::Move);
        let original: MoveFn = std::mem::transmute(ORIGINAL_MOVE.load(Ordering::Acquire));
        let Some(shared) = SHARED.get() else {
            original(entity, x, y, events);
            return;
        };
        let handled = catch_unwind(AssertUnwindSafe(|| {
            // A skill held during windup reaches this hook as a current-position
            // Move. An empty effect queue plus observed attack timing proves the
            // accepted attack is no longer pending; only its animation remains.
            let Some((key, actor)) = owned_entity(shared, entity) else {
                return false;
            };
            observe_owned_abilities(shared, entity, key, actor);
            if finish_attack_backswing(shared, key, actor, entity, events) {
                observe_owned_abilities(shared, entity, key, actor);
            }
            let Some(ticket) = take_stop_ticket(key, actor) else { return false; };
            if ticket.cancel_recall { cancel_recall(shared,entity,ticket.actor,events); }
            let action = std::ptr::read_unaligned((entity + 0x70) as *const usize);
            let goal = (std::ptr::read_unaligned((entity + 0x78) as *const u64), std::ptr::read_unaligned((entity + 0x80) as *const u64));
            let position = (std::ptr::read_unaligned((entity + 0x658) as *const u64), std::ptr::read_unaligned((entity + 0x660) as *const u64));
            let repeated = repeated_move(entity, x, y, ticket.hold);
            shared.movement.trace_native_move(ticket.actor, position, action, (x, y), goal, repeated, &shared.logger);
            if !ticket.hold { return repeated; }
            let actor = ticket.actor;
            let action = std::ptr::read_unaligned((entity + 0x70) as *const usize);
            let notify: StopEventFn = std::mem::transmute(NOTIFY_STOP.load(Ordering::Acquire));
            let stopped = stop_movement(entity, actor, events, notify);
            if stopped && action == 2 {
                shared.logger.write(&format!(
                    "MANUAL NATIVE_STOP actor={actor} action=2->0; native stop-animation event emitted"
                ));
            }
            stopped
        }))
        .unwrap_or_else(|_| {
            shared
                .timing
                .cancel("Movement stop adapter panic", &shared.logger);
            false
        });
        if !handled {
            original(entity, x, y, events);
        }
    }
    unsafe fn finish_attack_backswing(
        shared: &Shared,
        key: crate::native_timing::MatchKey,
        actor: usize,
        entity: usize,
        events: usize,
    ) -> bool {
        if !can_stop_action(entity, 3)
            || std::ptr::read_unaligned((entity + 0x4b4) as *const u32) != 0 // BaseAttack only
            || std::ptr::read_unaligned((entity + 0x2b0) as *const usize) != 0
        {
            return false;
        }
        // Player preference (Settings > Combat & casting > Attacks).
        if crate::settings::option("attack_cancel") != 1. {
            return false;
        }
        let elapsed = std::ptr::read_unaligned((entity + 0x78) as *const usize);
        let cooldown = std::ptr::read_unaligned((entity + 0xb0) as *const usize);
        let reason = if shared
            .abilities
            .attack_interrupt(key, actor, elapsed, cooldown)
        {
            "buffered-skill"
        } else if shared
            .abilities
            .committed_attack_start(key, actor, elapsed, cooldown)
            .is_some_and(|start| shared.movement.manual_release_after(start))
        {
            "manual-move-or-stop"
        } else {
            return false;
        };
        let notify: StopEventFn = std::mem::transmute(NOTIFY_STOP.load(Ordering::Acquire));
        notify(events, actor);
        std::ptr::write_unaligned((entity + 0x70) as *mut usize, 0);
        shared.logger.write(&format!("ATTACK COMMITTED_INTERRUPT actor={actor} reason={reason} elapsed={elapsed} cooldown={cooldown}; hit tick passed, empty pending queue, animation released, cooldown retained"));
        true
    }

    type NativeInputFn = unsafe extern "system" fn(usize, usize, usize, f32, usize, usize, usize);
    fn spectator_key_is_owned(tag: u64, key: u8) -> bool {
        matches!(tag, 0x8000000000000006 | 0x8000000000000007) && !matches!(key, 0x21 | 0x3d | 0x3e)
    }
    unsafe extern "system" fn input_hook(
        view: usize,
        ui: usize,
        system: usize,
        dt: f32,
        event: usize,
        window: usize,
        database: usize,
    ) {
        INPUT_ENTRIES.fetch_add(1, Ordering::Relaxed);
        let original: NativeInputFn = std::mem::transmute(ORIGINAL_INPUT.load(Ordering::Acquire));
        let Some(shared) = SHARED.get() else {
            original(view, ui, system, dt, event, window, database);
            return;
        };
        let owned = catch_unwind(AssertUnwindSafe(|| {
            let keys = crate::platform_input::poll();
            if !shared.timing.client_controls(Some(view)) || !keys.focused {
                return false;
            }
            let tag = std::ptr::read_unaligned(event as *const u64);
            if !matches!(tag, 0x8000000000000006 | 0x8000000000000007) {
                return false;
            }
            let key = std::ptr::read((event + 8) as *const u8);
            if crate::settings::MODAL.load(Ordering::Relaxed)
                || spectator_key_is_owned(tag, key)
                || keys.start
                || keys.release
            {
                shared.logger.write(&format!(
                    "NATIVE SPECTATOR_KEY suppressed tag={tag:x} key={key:x} bound_view={view:x}"
                ));
                true
            } else {
                false
            }
        }))
        .unwrap_or_else(|_| {
            shared
                .timing
                .cancel("Spectator input adapter panic", &shared.logger);
            false
        });
        if !owned {
            original(view, ui, system, dt, event, window, database);
        }
    }

    /// The viewer's eighth argument is the live `ingame` UiNode (cdcbf6..
    /// cdcce0), also consumed by b9cfc5..b9d02c. Use the same checked downcast
    /// as native code; do not retain a registry lookup or dereference an old UI.
    unsafe fn ingame_ui_from_node(node: usize, config: usize) -> Option<usize> {
        if node == 0 {
            return None;
        }
        let data = std::ptr::read_unaligned((node + 0x230) as *const usize);
        let table = std::ptr::read_unaligned((node + 0x238) as *const usize);
        if data == 0 || table == 0 {
            return None;
        }
        // Native Rust Option<&mut dyn Any> returns its two words in RAX/RDX,
        // unlike a Win64 C aggregate return, which would require a hidden sret.
        let get: unsafe fn(usize) -> (usize, usize) =
            std::mem::transmute(std::ptr::read_unaligned((table + 0x50) as *const usize));
        let (object, any_table) = get(data);
        if object == 0 || any_table == 0 {
            return None;
        }
        let type_id: unsafe extern "system" fn(*mut [u64; 2], usize) =
            std::mem::transmute(std::ptr::read_unaligned((any_table + 0x18) as *const usize));
        let mut identity = [0u64; 2];
        type_id(&mut identity, object);
        if identity != [0x810a3f1ff5177337, 0xaf7a6919a6336e28] {
            return None;
        }
        ingame_matches_config(object, config).then_some(object)
    }
    unsafe fn ingame_matches_config(ingame: usize, config: usize) -> bool {
        ingame != 0
            && config >= 0x18
            && std::ptr::read_unaligned((ingame + 0x9150) as *const usize) == config - 0x18
    }
    struct CameraLease {
        view: usize,
        config: usize,
        ingame: usize,
        original: [u8; 16],
        original_vision: u8,
        original_full_width: u8,
        original_ui_full_width: u8,
        effective_vision: u8,
    }
    impl CameraLease {
        unsafe fn capture(view: usize, config: usize, ingame: usize) -> Self {
            let mut original = [0u8; 16];
            std::ptr::copy_nonoverlapping((config + 0x18) as *const u8, original.as_mut_ptr(), 16);
            Self {
                view,
                config,
                ingame,
                original,
                original_vision: std::ptr::read((config + 0x4b) as *const u8),
                original_full_width: std::ptr::read((config + 0x45) as *const u8),
                original_ui_full_width: std::ptr::read((ingame + 0x9234) as *const u8),
                effective_vision: u8::MAX,
            }
        }
        fn matches(&self, view: usize, config: usize) -> bool {
            self.view == view && self.config == config
        }
        unsafe fn layout_matches(&self, view: usize, config: usize, ingame: usize) -> bool {
            self.matches(view, config)
                && self.ingame == ingame
                && ingame_matches_config(ingame, config)
        }
        unsafe fn full_width(&self, view: usize, config: usize, ingame: usize) -> bool {
            if !self.layout_matches(view, config, ingame) {
                return false;
            }
            // Native toggle 7b4de5..7b4df1 changes both flags. UI layout reads
            // +9234 separately to position announcements, kills and controls.
            std::ptr::write((ingame + 0x9234) as *mut u8, 1);
            std::ptr::write((config + 0x45) as *mut u8, 1);
            true
        }
        unsafe fn restore(&self, view: usize, config: usize, ingame: usize) -> bool {
            if !self.layout_matches(view, config, ingame) {
                return false;
            }
            std::ptr::copy_nonoverlapping(self.original.as_ptr(), (config + 0x18) as *mut u8, 16);
            std::ptr::write((config + 0x4b) as *mut u8, self.original_vision);
            std::ptr::write((config + 0x45) as *mut u8, self.original_full_width);
            std::ptr::write((ingame + 0x9234) as *mut u8, self.original_ui_full_width);
            true
        }
    }
    static CAMERA_LEASE: std::sync::Mutex<Option<CameraLease>> = std::sync::Mutex::new(None);
    pub(super) fn reset_session() {
        if let Ok(mut lease) = CAMERA_LEASE.lock() {
            *lease = None;
        }
    }
    unsafe fn camera_frame(view: usize, config: usize) -> Option<crate::camera::CameraFrame> {
        use crate::camera::{CameraFrame, Rect};
        if std::ptr::read((view + 0x130) as *const u8) != 0 {
            return None;
        }
        let f = |offset| std::ptr::read_unaligned((view + offset) as *const f32);
        let wide = std::ptr::read((config + 0x45) as *const u8) != 0;
        let left = std::ptr::read((config + 0x46) as *const u8) != 0;
        // Viewport follows 0.6.3 renderer 23228cd..2322ae2. Minimap geometry
        // matches the reviewed render/input operand plan, including custom placement.
        let viewport = if wide {
            Rect {
                x: 0.,
                y: 50.,
                w: 1920.,
                h: 974.,
            }
        } else {
            Rect {
                x: if left { f(0xf8) } else { 942. },
                y: f(0xfc),
                w: f(0x100),
                h: f(0x104),
            }
        };
        let custom = (std::ptr::read_unaligned((view + 0x124) as *const u32) == 1)
            .then(|| (f(0x128), f(0x12c)));
        let minimap = crate::minimap::content_rect(wide, left, custom);
        let frame = CameraFrame {
            viewport,
            minimap,
            center: (f(0x114), f(0x118)),
            extent: (f(0x11c), f(0x120)),
        };
        frame.valid().then_some(frame)
    }
    unsafe fn restore_camera(view: usize, config: usize, ingame: Option<usize>, shared: &Shared) {
        let Some(ingame) = ingame else { return };
        if let Ok(mut lease) = CAMERA_LEASE.lock() {
            if lease
                .as_ref()
                .is_some_and(|l| l.layout_matches(view, config, ingame))
            {
                let saved = lease.take().unwrap();
                saved.restore(view, config, ingame);
                shared.logger.write(&format!(
                    "VISION restored original={} on release; LAYOUT restored full_width={} ui_full_width={}",
                    saved.original_vision, saved.original_full_width, saved.original_ui_full_width
                ));
                // Keys released while owned must not revive an old pan velocity.
                std::ptr::write_unaligned((view + 0x458) as *mut u64, 0);
                shared.camera.reset();
                shared
                    .logger
                    .write("CAMERA native camera selection restored on release");
            }
        }
    }
    unsafe fn set_follow_config(config: usize, side: usize, lane: u32) {
        // CURRENT 1fcfd5d reads +20 team and +1c lane to look up its player map.
        // Native own-mid shortcut cb020e writes the same pair (team, lane=2).
        std::ptr::write_unaligned((config + 0x18) as *mut u32, 2);
        std::ptr::write_unaligned((config + 0x1c) as *mut u32, lane);
        std::ptr::write_unaligned((config + 0x20) as *mut usize, side);
    }
    unsafe fn prepare_camera(
        view: usize,
        config: usize,
        ingame: usize,
        mode: ViewMode,
        dt: f32,
        shared: &Shared,
    ) {
        // Bind before reading the viewport, including the bootstrap frame. The
        // spectator's compact preference can survive into a different match.
        if let Ok(mut lease) = CAMERA_LEASE.lock() {
            if lease.is_none() {
                let saved = CameraLease::capture(view, config, ingame);
                shared.logger.write(&format!(
                    "CAMERA BOUND; LAYOUT original_full_width={} original_ui_full_width={} owned_full_width=1 owned_ui_full_width=1; native full-screen UI layout retained",
                    saved.original_full_width, saved.original_ui_full_width
                ));
                *lease = Some(saved);
            }
            if !lease
                .as_ref()
                .is_some_and(|l| l.full_width(view, config, ingame))
            {
                return;
            }
        } else {
            return;
        }
        if mode == ViewMode::Bootstrap {
            return;
        }
        let notches = crate::wheel::take();
        if notches != 0 {
            let current = std::ptr::read_unaligned((view + 0x110) as *const f32);
            if let Some(zoom) = crate::wheel::zoom(current, notches) {
                std::ptr::write_unaligned((view + 0x110) as *mut f32, zoom);
                shared.logger.write(&format!(
                    "CAMERA WHEEL notches={notches} zoom={current}->{zoom}"
                ));
            }
        }
        let Some((target, champion)) = shared.movement.camera_target() else {
            return;
        };
        let Some(frame) = camera_frame(view, config) else {
            return;
        };
        if let Ok(mut lease) = CAMERA_LEASE.lock() {
            if !lease.as_ref().is_some_and(|l| l.matches(view, config)) {
                return;
            }
            if let Some(value) = shared.camera.vision().native(target.side) {
                let saved = lease.as_mut().expect("camera lease bound");
                if saved.effective_vision != value {
                    shared.logger.write(&format!(
                        "VISION controlled_side={} original={} mode={:?} effective={value}",
                        target.side,
                        saved.original_vision,
                        shared.camera.vision()
                    ));
                    saved.effective_vision = value;
                }
                std::ptr::write((config + 0x4b) as *mut u8, value);
            }
        } else {
            return;
        }
        std::ptr::write_unaligned((view + 0x458) as *mut u64, 0);
        let native_mode = std::ptr::read_unaligned((config + 0x18) as *const u32);
        let request = shared.camera.step(
            frame,
            native_mode,
            crate::platform_input::poll(),
            target.player,
            champion,
            mode == ViewMode::Running,
            dt,
            &shared.logger,
        );
        match request {
            Some(crate::camera::Request::Free((x, y))) => {
                std::ptr::write_unaligned((config + 0x18) as *mut u32, 1);
                std::ptr::write_unaligned((config + 0x1c) as *mut f32, x);
                std::ptr::write_unaligned((config + 0x20) as *mut f32, y);
            }
            Some(crate::camera::Request::Follow(player)) => {
                let Some(champion) = champion else { return };
                if player != target.player || target.side > 1 || target.lane >= 5 {
                    return;
                }
                if native_mode != 2
                    || std::ptr::read_unaligned((config + 0x1c) as *const u32) != target.lane as u32
                    || std::ptr::read_unaligned((config + 0x20) as *const usize) != target.side
                {
                    // Recenter on entry even if playback dt is zero while held.
                    std::ptr::write_unaligned(
                        (view + 0x114) as *mut f32,
                        champion.0 as f32 / 1000.,
                    );
                    std::ptr::write_unaligned(
                        (view + 0x118) as *mut f32,
                        champion.1 as f32 / 1000.,
                    );
                }
                set_follow_config(config, target.side, target.lane as u32);
            }
            None => {}
        }
    }

    unsafe extern "system" fn worker_hook(output: usize, sender: usize, frame: usize) {
        STOP_TICKET.set(None);
        WORKER_ENTRIES.fetch_add(1, Ordering::Relaxed);
        let original: WorkerFn = std::mem::transmute(ORIGINAL_WORKER.load(Ordering::Acquire));
        // Publish before counting or waiting. This call site is AFTER runner,
        // highlights and highlight-segments write guards have been released.
        crate::perf::worker_thread();
        let tick = crate::perf::time(crate::perf::Section::WorkerSend);
        original(output, sender, frame);
        drop(tick);
        let _after = crate::perf::time(crate::perf::Section::WorkerAfterSend);
        if let Some(shared) = SHARED.get() {
            if catch_unwind(AssertUnwindSafe(|| {
                let (accepted, trace) = shared.timing.hook_entry(true, &shared.logger);
                if trace {
                    capture_trace("native worker entrance", &shared.logger);
                }
                if accepted {
                    // Native be4710 checks the same niche sentinel: -1 means
                    // successful send; other values carry the unsent frame.
                    let sent = std::ptr::read_unaligned(output as *const usize) == usize::MAX;
                    if sent {
                        shared.timing.after_publication(sender, &shared.logger);
                    } else {
                        shared
                            .timing
                            .cancel("Native frame send failed", &shared.logger);
                    }
                }
            }))
            .is_err()
            {
                shared.timing.cancel("Worker adapter panic", &shared.logger);
            }
        }
    }

    /// Snapshot only these three config fields; their borrow belongs to the
    /// native caller and spans this call. Restore before that borrow is released.
    struct PlaybackOverride {
        config: usize,
        sync: u8,
        mode: u32,
        speed: u32,
    }
    impl PlaybackOverride {
        unsafe fn apply(config: usize) -> Self {
            let snapshot = Self {
                config,
                sync: std::ptr::read(config as *const u8),
                mode: std::ptr::read_unaligned((config + 0x10) as *const u32),
                speed: std::ptr::read_unaligned((config + 0x14) as *const u32),
            };
            std::ptr::write(config as *mut u8, 0);
            std::ptr::write_unaligned((config + 0x10) as *mut u32, 0);
            std::ptr::write_unaligned((config + 0x14) as *mut f32, 1.0);
            snapshot
        }
    }
    impl Drop for PlaybackOverride {
        fn drop(&mut self) {
            unsafe {
                std::ptr::write(self.config as *mut u8, self.sync);
                std::ptr::write_unaligned((self.config + 0x10) as *mut u32, self.mode);
                std::ptr::write_unaligned((self.config + 0x14) as *mut u32, self.speed);
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe extern "system" fn view_hook(
        view: usize,
        ui: usize,
        system: usize,
        config: usize,
        assets: usize,
        systems: usize,
        tps: usize,
        ingame_node: usize,
        dt: f32,
    ) {
        // Includes the native view update itself (0.65 timing).
        let _t = crate::perf::time(crate::perf::Section::Viewer);
        view_hook_body(
            view,
            ui,
            system,
            config,
            assets,
            systems,
            tps,
            ingame_node,
            dt,
        );
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn view_hook_body(
        view: usize,
        ui: usize,
        system: usize,
        config: usize,
        assets: usize,
        systems: usize,
        tps: usize,
        ingame_node: usize,
        dt: f32,
    ) {
        VIEW_ENTRIES.fetch_add(1, Ordering::Relaxed);
        let original: ViewFn = std::mem::transmute(ORIGINAL_VIEW.load(Ordering::Acquire));
        let Some(shared) = SHARED.get() else {
            original(
                view,
                ui,
                system,
                config,
                assets,
                systems,
                tps,
                ingame_node,
                dt,
            );
            return;
        };
        let accepted = catch_unwind(AssertUnwindSafe(|| {
            let (accepted, trace) = shared.timing.hook_entry(false, &shared.logger);
            if trace {
                capture_trace("native viewer entrance", &shared.logger);
            }
            accepted
        }))
        .unwrap_or_else(|_| {
            shared
                .timing
                .cancel("Viewer entrance diagnostic panic", &shared.logger);
            false
        });
        let ingame = ingame_ui_from_node(ingame_node, config);
        if !accepted {
            restore_camera(view, config, ingame, shared);
            original(
                view,
                ui,
                system,
                config,
                assets,
                systems,
                tps,
                ingame_node,
                dt,
            );
            if let Some(frame) = camera_frame(view, config) {
                shared.camera.capture(frame);
            }
            return;
        }
        let played = std::ptr::read_unaligned((view + 0x290) as *const usize);
        let queued = std::ptr::read_unaligned((view + 0x70) as *const usize);
        let mode = catch_unwind(AssertUnwindSafe(|| {
            shared
                .timing
                .before_view(view, played, queued, &shared.logger)
        }))
        .unwrap_or_else(|_| {
            shared.timing.cancel("Viewer adapter panic", &shared.logger);
            ViewMode::Native
        });
        if mode == ViewMode::Native {
            restore_camera(view, config, ingame, shared);
            original(
                view,
                ui,
                system,
                config,
                assets,
                systems,
                tps,
                ingame_node,
                dt,
            );
            if let Some(frame) = camera_frame(view, config) {
                shared.camera.capture(frame);
            }
            return;
        }
        if tps != 60 || queued > 10000 {
            shared.timing.cancel(
                "Unexpected viewer frame rate or queue layout",
                &shared.logger,
            );
            restore_camera(view, config, ingame, shared);
            original(
                view,
                ui,
                system,
                config,
                assets,
                systems,
                tps,
                ingame_node,
                dt,
            );
            return;
        }
        let Some(ingame_ui) = ingame else {
            shared.timing.cancel(
                "Native full-screen UI/config identity unavailable",
                &shared.logger,
            );
            restore_camera(view, config, ingame, shared);
            original(
                view,
                ui,
                system,
                config,
                assets,
                systems,
                tps,
                ingame_node,
                dt,
            );
            return;
        };
        let snapshot = PlaybackOverride::apply(config);
        if catch_unwind(AssertUnwindSafe(|| {
            prepare_camera(view, config, ingame_ui, mode, dt, shared)
        }))
        .is_err()
        {
            shared.timing.cancel("Camera adapter panic", &shared.logger);
            restore_camera(view, config, ingame, shared);
        }
        let actual_dt = match mode {
            ViewMode::Bootstrap => {
                std::ptr::write_unaligned((view + 0x358) as *mut f32, 0.0);
                1.0 / tps as f32
            }
            ViewMode::Paused => {
                std::ptr::write_unaligned((view + 0x358) as *mut f32, 0.0);
                0.0
            }
            ViewMode::Running => dt.min(2.0 / tps as f32),
            ViewMode::Native => dt,
        };
        original(
            view,
            ui,
            system,
            config,
            assets,
            systems,
            tps,
            ingame_node,
            actual_dt,
        );
        drop(snapshot);
        // Reassert after the viewer too, before render/camera capture, without
        // recapturing our own forced value as the spectator's preference.
        if let Ok(lease) = CAMERA_LEASE.lock() {
            if let Some(saved) = lease.as_ref() {
                saved.full_width(view, config, ingame_ui);
            }
        }
        if let Some(frame) = camera_frame(view, config) {
            shared.camera.capture(frame);
        }
        let remaining = std::ptr::read_unaligned((view + 0x70) as *const usize);
        let played_after = std::ptr::read_unaligned((view + 0x290) as *const usize);
        if mode != ViewMode::Running {
            std::ptr::write_unaligned((view + 0x358) as *mut f32, 0.0);
        }
        if catch_unwind(AssertUnwindSafe(|| {
            shared
                .timing
                .after_view(mode, queued, remaining, played_after, &shared.logger)
        }))
        .is_err()
        {
            shared
                .timing
                .cancel("Viewer acknowledgement panic", &shared.logger);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn outline_roles_merge_same_unit_and_keep_target_thinner_than_hover() {
            let enemy = crate::combat::Unit {
                id: 29,
                position: (200_000, 200_000),
                radius: 20_000,
                is_champion: true,
                is_minion: false,
                friendly: false,
                in_cc: false,
                is_tower: false,
                body: None,
            };
            let other = crate::combat::Unit { id: 30, ..enemy };
            let targets = OutlineTargets {
                hover: Some(enemy),
                attack: Some(other),
                click: None,
            };
            let now = std::time::Instant::now();
            assert_eq!(
                targets.for_unit(enemy.id, now).unwrap().1,
                OutlineRole::Hover
            );
            assert_eq!(
                targets.for_unit(other.id, now).unwrap().1,
                OutlineRole::Attack
            );
            assert!(targets.for_unit(31, now).is_none());
            let merged = OutlineTargets {
                hover: Some(enemy),
                attack: Some(enemy),
                click: None,
            };
            assert_eq!(
                merged.for_unit(enemy.id, now).unwrap().1,
                OutlineRole::Hover
            );
            let target_only = OutlineTargets {
                hover: None,
                attack: Some(enemy),
                click: None,
            };
            assert_eq!(
                target_only.for_unit(enemy.id, now).unwrap().1,
                OutlineRole::Attack
            );
            let ally = crate::combat::Unit {
                friendly: true,
                ..enemy
            };
            assert!(OutlineTargets {
                hover: None,
                attack: Some(ally),
                click: Some((ally.id, now)),
            }
            .for_unit(ally.id, now)
            .is_none());
            assert!(OutlineRole::Attack.offset() < OutlineRole::Hover.offset());
            assert_eq!(OutlineRole::Attack.offset(), 2. / 3.);
            assert_eq!(OutlineRole::Hover.offset(), 1.2);
            assert!(OutlineTargets::default().for_unit(enemy.id, now).is_none());
        }
        #[test]
        fn click_pulse_wins_once_then_settles_to_hover_or_target() {
            use std::time::{Duration, Instant};
            let unit = crate::combat::Unit {
                id: 29,
                position: (200_000, 200_000),
                radius: 20_000,
                is_champion: true,
                is_minion: false,
                friendly: false,
                in_cc: false,
                is_tower: false,
                body: None,
            };
            let at = Instant::now();
            for hovered in [false, true] {
                let targets = OutlineTargets {
                    hover: hovered.then_some(unit),
                    attack: Some(unit),
                    click: Some((unit.id, at)),
                };
                let first = targets.for_unit(unit.id, at).unwrap();
                assert_eq!((first.1, first.2), (OutlineRole::Click, 3.));
                let middle = targets
                    .for_unit(unit.id, at + Duration::from_millis(60))
                    .unwrap();
                assert_eq!(middle.1, OutlineRole::Click);
                let end = targets
                    .for_unit(unit.id, at + crate::movement::ATTACK_CLICK_DURATION)
                    .unwrap();
                assert_eq!(
                    end.1,
                    if hovered {
                        OutlineRole::Hover
                    } else {
                        OutlineRole::Attack
                    }
                );
                assert!(first.2 > middle.2 && middle.2 > end.2);
                // A repeated click restarts even on the same identity.
                let repeated = OutlineTargets {
                    click: Some((unit.id, at + Duration::from_secs(1))),
                    ..targets
                };
                assert_eq!(
                    repeated
                        .for_unit(unit.id, at + Duration::from_secs(1))
                        .unwrap()
                        .2,
                    3.
                );
                // A stale click cannot pulse another attack, an ally, or
                // a hovered unit after its attack order was cancelled.
                let cancelled = OutlineTargets {
                    attack: None,
                    ..targets
                };
                let role = cancelled.for_unit(unit.id, at).map(|(_, r, _)| r);
                assert_eq!(role, hovered.then_some(OutlineRole::Hover));
                let replaced = OutlineTargets {
                    click: Some((30, at)),
                    ..targets
                };
                assert_eq!(replaced.for_unit(unit.id, at).unwrap().1, end.1);
            }
        }
        #[test]
        fn spectator_layout_lease_switches_and_restores_both_native_flags() {
            for (original_wide, original_ui_wide) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                let mut config = [0x55u8; 0x60];
                config[0x45] = original_wide;
                config[0x46] = 1;
                let original = config;
                let addr = config.as_mut_ptr() as usize;
                let mut ui = vec![0x33u8; 0x9250];
                let live = ui.as_mut_ptr() as usize;
                unsafe {
                    std::ptr::write_unaligned((live + 0x9150) as *mut usize, addr - 0x18);
                    ui[0x9234] = original_ui_wide;
                    let original_ui = ui.clone();
                    let lease = CameraLease::capture(101, addr, live);
                    assert!(lease.full_width(101, addr, live));
                    assert_eq!((config[0x45], ui[0x9234]), (1, 1));
                    config[0x18..0x28].fill(2);
                    config[0x4b] = 2;
                    ui[0x9234] = 0;
                    assert!(lease.full_width(101, addr, live));
                    let overridden = config;
                    let overridden_ui = ui.clone();
                    let mut other = [0x33u8; 0x60];
                    let other_before = other;
                    let other_addr = other.as_mut_ptr() as usize;
                    let mut other_ui = ui.clone();
                    let other_live = other_ui.as_mut_ptr() as usize;
                    for (v, c, u) in [
                        (102, addr, live),
                        (101, other_addr, live),
                        (101, addr, other_live),
                        (101, addr, 0),
                    ] {
                        assert!(!lease.full_width(v, c, u));
                        assert!(!lease.restore(v, c, u));
                    }
                    assert_eq!(config, overridden);
                    assert_eq!(ui, overridden_ui);
                    assert_eq!(other, other_before);
                    assert_eq!(other_ui, overridden_ui);
                    // Even an identical UI address must still link to this config.
                    std::ptr::write_unaligned((live + 0x9150) as *mut usize, other_addr - 0x18);
                    assert!(!lease.full_width(101, addr, live));
                    assert!(!lease.restore(101, addr, live));
                    assert_eq!(config, overridden);
                    assert_eq!(ui[0x9234], 1);
                    std::ptr::write_unaligned((live + 0x9150) as *mut usize, addr - 0x18);
                    assert!(lease.restore(101, addr, live));
                    assert_eq!(config, original);
                    assert_eq!(ui, original_ui);
                    other[0x45] = 1 - original_wide;
                    other_ui[0x9234] = 1 - original_ui_wide;
                    std::ptr::write_unaligned(
                        (other_live + 0x9150) as *mut usize,
                        other_addr - 0x18,
                    );
                    let fresh_original = other;
                    let fresh_ui = other_ui.clone();
                    let fresh = CameraLease::capture(102, other_addr, other_live);
                    assert!(fresh.full_width(102, other_addr, other_live));
                    assert!(fresh.restore(102, other_addr, other_live));
                    assert_eq!(other, fresh_original);
                    assert_eq!(other_ui, fresh_ui);
                }
            }
        }
        unsafe fn fake_ui_any(data: usize) -> (usize, usize) {
            let pair = std::ptr::read(data as *const [usize; 2]);
            (pair[0], pair[1])
        }
        unsafe extern "system" fn fake_ui_type_id(out: *mut [u64; 2], _: usize) {
            std::ptr::write(out, [0x810a3f1ff5177337, 0xaf7a6919a6336e28]);
        }
        unsafe extern "system" fn fake_other_type_id(out: *mut [u64; 2], _: usize) {
            std::ptr::write(out, [0, 0]);
        }
        #[test]
        fn live_ingame_downcast_checks_type_and_shared_config_before_layout_access() {
            let config = [0u8; 0x60];
            let addr = config.as_ptr() as usize;
            let mut ui = vec![0u8; 0x9250];
            let object = ui.as_mut_ptr() as usize;
            let mut any_table = [0usize; 4];
            any_table[3] = fake_ui_type_id as *const () as usize;
            let mut pair = [object, any_table.as_ptr() as usize];
            let mut table = [0usize; 11];
            table[10] = fake_ui_any as *const () as usize;
            let mut node = [0usize; 0x240 / 8];
            node[0x230 / 8] = pair.as_ptr() as usize;
            node[0x238 / 8] = table.as_ptr() as usize;
            let node_addr = node.as_ptr() as usize;
            unsafe {
                assert_eq!(ingame_ui_from_node(0, addr), None);
                assert_eq!(ingame_ui_from_node(node_addr, addr), None);
                std::ptr::write_unaligned((object + 0x9150) as *mut usize, addr - 0x18);
                assert_eq!(ingame_ui_from_node(node_addr, addr), Some(object));
                std::ptr::write(
                    any_table.as_mut_ptr().add(3),
                    fake_other_type_id as *const () as usize,
                );
                assert_eq!(ingame_ui_from_node(node_addr, addr), None);
                std::ptr::write(pair.as_mut_ptr(), 0);
                assert_eq!(ingame_ui_from_node(node_addr, addr), None);
                std::ptr::write(node.as_mut_ptr().add(0x238 / 8), 0);
                assert_eq!(ingame_ui_from_node(node_addr, addr), None);
            }
        }
        thread_local! {
            static DROPPED_COMMANDS: std::cell::RefCell<Vec<(u64, u8)>> = const { std::cell::RefCell::new(Vec::new()) };
        }
        unsafe extern "system" fn spy_drop_command(command: usize) {
            let pointer = command as *const u8;
            DROPPED_COMMANDS.with(|d| {
                d.borrow_mut()
                    .push((command_tag(pointer), *pointer.add(0x90)))
            });
        }
        fn fake_commands<const N: usize>(tags: [u64; N]) -> [NativeCommand; N] {
            std::array::from_fn(|i| {
                let mut command = NativeCommand([0; COMMAND_SIZE]);
                // Text uses the String capacity niche, rather than an enum tag.
                let raw = if tags[i] == 7 {
                    16
                } else {
                    tags[i] | (1 << 63)
                };
                command.0[..8].copy_from_slice(&raw.to_le_bytes());
                command.0[0x90] = i as u8 + 10;
                command
            })
        }
        #[test]
        fn mixed_sprite_circle_text_pass_moves_sprites_and_drops_auxiliaries_once() {
            let commands = fake_commands([3, 4, 7, 8, 3]);
            let vector = NativeCommandVec {
                capacity: commands.len(),
                pointer: commands.as_ptr() as usize,
                length: commands.len(),
            };
            let before = commands.each_ref().map(|c| c.0);
            let mut destination = fake_commands([0, 0]);
            DROPPED_COMMANDS.with(|d| d.borrow_mut().clear());
            unsafe {
                assert!(valid_command_vec(vector));
                assert!(supported_outline_commands(vector));
                assert_eq!(sprite_count(vector), 2);
                assert_eq!(
                    take_outline_commands(
                        vector,
                        Some(destination.as_mut_ptr() as *mut u8),
                        spy_drop_command
                    ),
                    2
                );
            }
            assert_eq!(destination[0].0, before[0]);
            assert_eq!(destination[1].0, before[4]);
            assert_eq!(commands.each_ref().map(|c| c.0), before);
            DROPPED_COMMANDS.with(|d| assert_eq!(*d.borrow(), [(4, 11), (7, 12), (8, 13)]));
        }
        #[test]
        fn fallback_drops_every_known_command_once_without_moving_original() {
            let commands = fake_commands([3, 4, 7, 8, 3]);
            let vector = NativeCommandVec {
                capacity: commands.len(),
                pointer: commands.as_ptr() as usize,
                length: commands.len(),
            };
            let before = commands.each_ref().map(|c| c.0);
            DROPPED_COMMANDS.with(|d| d.borrow_mut().clear());
            unsafe {
                assert_eq!(take_outline_commands(vector, None, spy_drop_command), 0);
            }
            assert_eq!(commands.each_ref().map(|c| c.0), before);
            DROPPED_COMMANDS
                .with(|d| assert_eq!(*d.borrow(), [(3, 10), (4, 11), (7, 12), (8, 13), (3, 14)]));
        }
        #[test]
        fn unknown_commands_are_rejected_and_never_given_to_native_destructor() {
            let commands = fake_commands([4, 19, 7]);
            let vector = NativeCommandVec {
                capacity: commands.len(),
                pointer: commands.as_ptr() as usize,
                length: commands.len(),
            };
            assert!(!unsafe { supported_outline_commands(vector) });
            DROPPED_COMMANDS.with(|d| d.borrow_mut().clear());
            unsafe {
                take_outline_commands(vector, None, spy_drop_command);
            }
            DROPPED_COMMANDS.with(|d| assert_eq!(*d.borrow(), [(4, 10), (7, 12)]));
        }
        #[test]
        fn all_native_command_variants_are_accepted_with_bounded_pass_size() {
            let commands = fake_commands(std::array::from_fn::<_, 19, _>(|i| i as u64));
            let mut vector = NativeCommandVec {
                capacity: commands.len(),
                pointer: commands.as_ptr() as usize,
                length: commands.len(),
            };
            assert!(unsafe { supported_outline_commands(vector) });
            vector.capacity = MAX_BODY_COMMANDS + 1;
            vector.length = MAX_BODY_COMMANDS + 1;
            assert!(!unsafe { valid_command_vec(vector) });
        }
        #[test]
        fn aim_copy_preserves_cursor_and_native_rng_and_seven_argument_abi() {
            unsafe extern "system" fn correct(
                a: usize,
                b: usize,
                c: usize,
                d: usize,
                e: usize,
                f: usize,
                target: usize,
            ) {
                let seen = &mut *(a as *mut Vec<usize>);
                seen.extend([b, c, d, e, f]);
                let input = &mut *(target as *mut [usize; 3]);
                seen.extend(*input);
                input[1] = 999;
                input[2] = 555;
            }
            let mut seen = Vec::new();
            let mut target = [1usize, (-47_291i64) as usize, (-125_996i64) as usize];
            let cursor = target;
            let args = [
                &mut seen as *mut Vec<usize> as usize,
                2,
                3,
                4,
                468,
                6,
                target.as_mut_ptr() as usize,
            ];
            unsafe {
                assert_eq!(evaluate_aim(correct, args, true), Some([1, 999, 555]));
            }
            assert_eq!(target, cursor);
            assert_eq!(seen, [2, 3, 4, 468, 6, cursor[0], cursor[1], cursor[2]]);
            seen.clear();
            // Same relay mechanism as the installed hook; exercises args 5-7
            // on the Windows stack, not just a direct Rust call.
            let address =
                unsafe { relay(correct as *const () as usize, correct as *const () as usize) }
                    .unwrap();
            let relayed: AimFn = unsafe { std::mem::transmute(address) };
            unsafe {
                assert_eq!(evaluate_aim(relayed, args, false), None);
            }
            assert_eq!(target, [1, 999, 555]);
            assert_eq!(seen.len(), 8);
            let shared = owned_fixture("aim-worker-scope");
            assert!(owned_key(&shared, 7).is_some());
            assert!(owned_key(&shared, 8).is_none());
            for kind in 0usize..5 {
                let words = [kind | (0xaabbccddusize << 32), 30, 40];
                let result = unsafe { manual_aim_words(&shared, 7, words.as_ptr() as usize) };
                assert_eq!(result.is_some(), matches!(kind, 1 | 2));
                assert!(unsafe { manual_aim_words(&shared, 8, words.as_ptr() as usize) }.is_none());
            }
            assert!(unsafe { manual_aim_words(&shared, 7, 0) }.is_none());
            // Foreign workers must fail before touching even an aligned invalid pointer.
            let worker = std::thread::spawn(move || unsafe { manual_aim_words(&shared, 7, 8) });
            assert_eq!(worker.join().unwrap(), None);
        }
        fn owned_fixture(name: &str) -> Shared {
            let key = (1, 33, 1);
            let shared = Shared {
                timing: Arc::new(NativeTiming::running_test_worker(key)),
                logger: Arc::new(crate::test_support::logger(name)),
                movement: Arc::new(crate::movement::Movement::new(true)),
                camera: Arc::new(crate::camera::CameraControl::default()),
                abilities: Arc::new(crate::abilities::Abilities::default()),
            };
            shared
                .abilities
                .observe_actor(key, Some(7), Some((200_000, 200_000)), [0; 3]);
            shared.abilities.update(
                crate::platform_input::Keys {
                    focused: true,
                    ..Default::default()
                },
                true,
                &shared.camera,
                &shared.logger,
            );
            shared
        }
        fn owned_bytes() -> [u8; 0x6c0] {
            let mut bytes = [0u8; 0x6c0];
            bytes[0x68..0x6c].copy_from_slice(&15u32.to_le_bytes());
            bytes[0x5b8..0x5c0].copy_from_slice(&7usize.to_le_bytes());
            bytes[0x5c0..0x5c8].copy_from_slice(&1usize.to_le_bytes());
            bytes
        }
        #[test]
        fn reused_actor_on_background_worker_cannot_replace_live_charge_state() {
            let shared = Arc::new(owned_fixture("owned-native-36"));
            let charges = Some(crate::abilities::Charges {
                capacity: 300,
                cost: 100,
                uses: 3,
            });
            shared
                .abilities
                .observe_native((1, 33, 1), 7, 2, [charges; 3]);
            let other = shared.clone();
            std::thread::spawn(move || {
                let mut bytes = owned_bytes();
                bytes[0x70..0x78].copy_from_slice(&5usize.to_le_bytes());
                let entity = bytes.as_ptr() as usize;
                assert!(unsafe { owned_entity(&other, entity) }.is_none());
                assert!(!unsafe { observe_owned_abilities(&other, entity, (1, 33, 1), 7) });
            })
            .join()
            .unwrap();
            assert_eq!(shared.abilities.hud_skills().uses, [Some(3); 3]);
            let bytes = owned_bytes();
            assert!(unsafe {
                observe_owned_abilities(&shared, bytes.as_ptr() as usize, (1, 33, 1), 7)
            });
            assert_eq!(shared.abilities.hud_skills().ready, [None; 3]);
            assert!(owned_key(&shared, 8).is_none());
            shared
                .abilities
                .observe_actor((1, 33, 2), Some(7), Some((200_000, 200_000)), [0; 3]);
            assert!(owned_key(&shared, 7).is_none());
        }
        #[test]
        fn steering_uses_current_borrow_without_previous_command_or_saved_address() {
            let shared = owned_fixture("owned-steering-36");
            STOP_TICKET.set(None);
            let first = owned_bytes();
            let second = owned_bytes();
            for bytes in [&first, &second] {
                let entity = bytes.as_ptr() as usize;
                assert_eq!(
                    unsafe { owned_steering(&shared, 7, entity + 0x658, entity + 0x660) },
                    Some((1, 33, 1))
                );
                assert!(
                    unsafe { owned_steering(&shared, 7, entity + 0x658, entity + 0x668) }.is_none()
                );
                assert!(
                    unsafe { owned_steering(&shared, 8, entity + 0x658, entity + 0x660) }.is_none()
                );
            }
            let mut wrong = owned_bytes();
            wrong[0x68..0x6c].copy_from_slice(&12u32.to_le_bytes());
            let entity = wrong.as_ptr() as usize;
            assert!(
                unsafe { owned_steering(&shared, 7, entity + 0x658, entity + 0x660) }.is_none()
            );
            assert!(unsafe { owned_steering(&shared, 7, 0, 8) }.is_none());
        }
        #[test]
        fn identical_move_does_not_suppress_changed_goals_holds_busy_or_forced_actions() {
            let mut bytes = [0u8; 0x6c0];
            bytes[0x68..0x6c].copy_from_slice(&15u32.to_le_bytes());
            bytes[0x70..0x78].copy_from_slice(&2usize.to_le_bytes());
            bytes[0x78..0x80].copy_from_slice(&300_000u64.to_le_bytes());
            bytes[0x80..0x88].copy_from_slice(&400_000u64.to_le_bytes());
            let address = bytes.as_ptr() as usize;
            assert!(unsafe { repeated_move(address, 300_000, 400_000, false) });
            assert!(!unsafe { repeated_move(address, 300_001, 400_000, false) });
            assert!(!unsafe { repeated_move(address, 300_000, 400_000, true) });
            for action in [0usize, 1, 3, 4, 5, 6] {
                bytes[0x70..0x78].copy_from_slice(&action.to_le_bytes());
                assert!(!unsafe { repeated_move(address, 300_000, 400_000, false) });
            }
            bytes[0x70..0x78].copy_from_slice(&2usize.to_le_bytes());
            let mut effect = [0u8; 0x28];
            effect[..4].copy_from_slice(&10u32.to_le_bytes());
            bytes[0x2c0..0x2c8].copy_from_slice(&(effect.as_ptr() as usize).to_le_bytes());
            bytes[0x2c8..0x2d0].copy_from_slice(&1usize.to_le_bytes());
            assert!(!unsafe { repeated_move(address, 300_000, 400_000, false) });
        }
        static RECEIVED: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());
        unsafe extern "system" fn fake_worker(a: usize, b: usize, c: usize) {
            *RECEIVED.lock().unwrap() = vec![a as u64, b as u64, c as u64];
            // Model send's sret output so forwarding verifies output memory too.
            std::ptr::write(a as *mut usize, usize::MAX);
        }
        unsafe extern "system" fn fake_move(a: usize, b: u64, c: u64, d: usize) {
            *RECEIVED.lock().unwrap() = vec![a as u64, b, c, d as u64];
        }
        #[allow(clippy::too_many_arguments)]
        unsafe extern "system" fn fake_steer(
            a: usize,
            b: usize,
            c: usize,
            d: usize,
            e: usize,
            f: usize,
            g: usize,
            h: u64,
            i: u64,
            j: usize,
        ) {
            *RECEIVED.lock().unwrap() = vec![
                a as u64, b as u64, c as u64, d as u64, e as u64, f as u64, g as u64, h, i,
                j as u64,
            ];
        }
        #[allow(clippy::too_many_arguments)]
        unsafe extern "system" fn fake_step(
            a: usize,
            b: usize,
            c: usize,
            d: usize,
            e: usize,
            f: u64,
            g: u64,
            h: usize,
        ) {
            *RECEIVED.lock().unwrap() = vec![
                a as u64, b as u64, c as u64, d as u64, e as u64, f, g, h as u64,
            ];
        }
        #[test]
        fn attack_commit_observer_rejects_unaccepted_attacks_and_preserves_entity() {
            let mut bytes = [0u8; 0x6c0];
            let mut queue = [0usize; 7];
            queue[2] = 5;
            bytes[0x68..0x6c].copy_from_slice(&15u32.to_le_bytes());
            bytes[0x70..0x78].copy_from_slice(&3usize.to_le_bytes());
            bytes[0x2a0..0x2a8].copy_from_slice(&1usize.to_le_bytes());
            bytes[0x2a8..0x2b0].copy_from_slice(&(queue.as_ptr() as usize).to_le_bytes());
            bytes[0x2b0..0x2b8].copy_from_slice(&1usize.to_le_bytes());
            // Queued-branch entry: delay readable, but it is not a locked attack.
            let actor = bytes.as_ptr() as usize;
            assert_eq!(unsafe { queued_attack_delay(actor, 0) }, Some(5));
            assert_eq!(unsafe { locked_attack_hit_tick(actor, 0) }, None);
            // Locked branch as traced for Ninja: no new entry, start 13, factor 100.
            bytes[0x2b0..0x2b8].copy_from_slice(&0usize.to_le_bytes());
            bytes[0x80..0x88].copy_from_slice(&100usize.to_le_bytes());
            bytes[0x4a8..0x4b0].copy_from_slice(&13usize.to_le_bytes());
            let before = bytes;
            assert_eq!(unsafe { locked_attack_hit_tick(actor, 0) }, Some(13));
            bytes[0x80..0x88].copy_from_slice(&130usize.to_le_bytes());
            assert_eq!(unsafe { locked_attack_hit_tick(actor, 0) }, Some(10));
            bytes[0x80..0x88].copy_from_slice(&100usize.to_le_bytes());
            assert_eq!(bytes, before);
            assert!(unsafe { can_stop_action(actor, 3) });
            // Not BaseAttack, absent declared timing, zero factor, started
            // earlier (elapsed != 0) or a queue entry from this call: no proof.
            for (range, value) in [
                (0x4b4..0x4b8, 1u32.to_le_bytes().to_vec()),
                (0x4b8..0x4bc, (-1i32).to_le_bytes().to_vec()),
                (0x80..0x88, 0usize.to_le_bytes().to_vec()),
                (0x78..0x80, 1usize.to_le_bytes().to_vec()),
                (0x2b0..0x2b8, 1usize.to_le_bytes().to_vec()),
            ] {
                let mut changed = before;
                changed[range].copy_from_slice(&value);
                let entity = changed.as_ptr() as usize;
                assert_eq!(unsafe { locked_attack_hit_tick(entity, 0) }, None);
            }
            bytes[0x78..0x80].copy_from_slice(&1usize.to_le_bytes());
            let mut effect = [0u8; 0x28];
            effect[..4].copy_from_slice(&10u32.to_le_bytes());
            bytes[0x2c0..0x2c8].copy_from_slice(&(effect.as_ptr() as usize).to_le_bytes());
            bytes[0x2c8..0x2d0].copy_from_slice(&1usize.to_le_bytes());
            assert!(!unsafe { can_stop_action(actor, 3) });
        }
        unsafe extern "system" fn fake_attack(a: usize, b: usize, c: usize) {
            *RECEIVED.lock().unwrap() = vec![a as u64, b as u64, c as u64];
        }
        unsafe extern "system" fn fake_cooltime(_: usize, _: usize) -> usize {
            360
        }
        unsafe extern "system" fn fake_uses(_: usize, _: usize) -> usize {
            3
        }
        #[test]
        fn native_observer_refreshes_spent_cooldown_before_charge_readiness() {
            let shared = owned_fixture("owned-cooldown-36");
            let mut bytes = owned_bytes();
            let mut table = [0usize; 24];
            table[0x90 / 8] = fake_cooltime as *const () as usize;
            table[0xa8 / 8] = fake_uses as *const () as usize;
            bytes[0x578..0x580].copy_from_slice(&1usize.to_le_bytes());
            bytes[0x580..0x588].copy_from_slice(&(table.as_ptr() as usize).to_le_bytes());
            let before = bytes;
            assert!(unsafe {
                observe_owned_abilities(&shared, bytes.as_ptr() as usize, (1, 33, 1), 7)
            });
            assert_eq!(shared.abilities.hud_skills().uses[0], Some(3));
            assert_eq!(bytes, before);
            bytes[0xb8..0xc0].copy_from_slice(&360usize.to_le_bytes());
            let after = bytes;
            assert!(unsafe {
                observe_owned_abilities(&shared, bytes.as_ptr() as usize, (1, 33, 1), 7)
            });
            assert_eq!(shared.abilities.hud_skills().uses[0], Some(0));
            assert_eq!(shared.abilities.hud_skills().ready[0], Some(false));
            assert_eq!(bytes, after);
        }
        #[test]
        fn charge_reader_matches_native_cdr_recast_budget_and_never_writes_entity() {
            let mut bytes = [0u8; 0x6c0];
            let mut table = [0usize; 24];
            table[0x90 / 8] = fake_cooltime as *const () as usize;
            table[0xa8 / 8] = fake_uses as *const () as usize;
            for offset in [0x578, 0x588, 0x598] {
                bytes[offset..offset + 8].copy_from_slice(&1usize.to_le_bytes());
                bytes[offset + 8..offset + 16]
                    .copy_from_slice(&(table.as_ptr() as usize).to_le_bytes());
            }
            bytes[0x5c0..0x5c8].copy_from_slice(&5usize.to_le_bytes());
            bytes[0x3f8..0x3fc].copy_from_slice(&100i32.to_le_bytes());
            bytes[0x464..0x468].copy_from_slice(&100i32.to_le_bytes());
            let before = bytes;
            let entity = bytes.as_ptr() as usize;
            for offset in [0x578, 0x588] {
                assert_eq!(
                    unsafe { read_charges(entity, offset) },
                    Some(crate::abilities::Charges {
                        capacity: 180,
                        cost: 60,
                        uses: 3
                    })
                );
            }
            assert_eq!(
                unsafe { read_charges(entity, 0x598) },
                Some(crate::abilities::Charges {
                    capacity: 120,
                    cost: 40,
                    uses: 3
                })
            );
            assert_eq!(bytes, before);
            bytes[0x5c0..0x5c8].copy_from_slice(&1usize.to_le_bytes());
            assert!(unsafe { read_charges(bytes.as_ptr() as usize, 0x588) }.is_none());
            assert!(unsafe { read_charges(bytes.as_ptr() as usize, 0x598) }.is_none());
        }
        #[test]
        fn native_effect_reader_uses_three_skill_slots_and_rejects_absent_effects() {
            let mut bytes = [0xccu8; 0x6c0];
            // Set distinct metadata with deliberately meaningless Arc/vtable
            // bytes: the reader must never dereference or clone those fields.
            bytes[0x5c0..0x5c8].copy_from_slice(&3u64.to_le_bytes());
            bytes[0x430..0x438].copy_from_slice(&2_000u64.to_le_bytes());
            for (slot, offset) in [0x4c0, 0x4f8, 0x530].into_iter().enumerate() {
                bytes[offset + 0x10..offset + 0x18]
                    .copy_from_slice(&(70_000u64 + slot as u64 * 10_000).to_le_bytes());
                bytes[offset + 0x18..offset + 0x20].copy_from_slice(&5_000u64.to_le_bytes());
                bytes[offset + 0x28..offset + 0x2c]
                    .copy_from_slice(&(6u32 + slot as u32).to_le_bytes());
                bytes[offset + 0x30..offset + 0x34].copy_from_slice(&(slot as u32).to_le_bytes());
            }
            let values = unsafe { read_ability_metadata(bytes.as_ptr() as usize) };
            for (slot, value) in values.into_iter().enumerate() {
                assert_eq!(
                    value.unwrap(),
                    crate::abilities::Descriptor {
                        casting: slot as u32,
                        target: 6 + slot as u32,
                        range: 82_000 + slot as u64 * 10_000
                    }
                );
            }
            bytes[0x4f0..0x4f4].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(unsafe { read_ability_metadata(bytes.as_ptr() as usize) }[0].is_none());
        }
        #[test]
        fn basic_attack_metadata_uses_current_effect_growth_and_bonus_without_pointer_calls() {
            let mut bytes = [0xccu8; 0x6c0];
            bytes[0x5c0..0x5c8].copy_from_slice(&3u64.to_le_bytes());
            bytes[0x430..0x438].copy_from_slice(&2_000u64.to_le_bytes());
            bytes[0x498..0x4a0].copy_from_slice(&60_000u64.to_le_bytes());
            bytes[0x4a0..0x4a8].copy_from_slice(&5_000u64.to_le_bytes());
            bytes[0x4b0..0x4b4].copy_from_slice(&6u32.to_le_bytes());
            bytes[0x4b8..0x4bc].copy_from_slice(&0u32.to_le_bytes());
            let entity = bytes.as_ptr() as usize;
            assert_eq!(
                unsafe { read_effect_metadata(entity, 0x488) }
                    .unwrap()
                    .range,
                72_000
            );
            bytes[0x498..0x4a0].copy_from_slice(&u64::MAX.to_le_bytes());
            assert!(unsafe { read_effect_metadata(bytes.as_ptr() as usize, 0x488) }.is_none());
            bytes[0x498..0x4a0].copy_from_slice(&60_000u64.to_le_bytes());
            bytes[0x4b8..0x4bc].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(unsafe { read_effect_metadata(bytes.as_ptr() as usize, 0x488) }.is_none());
        }
        #[allow(clippy::too_many_arguments)]
        unsafe extern "system" fn fake_view(
            a: usize,
            b: usize,
            c: usize,
            d: usize,
            e: usize,
            f: usize,
            g: usize,
            h: usize,
            i: f32,
        ) {
            *RECEIVED.lock().unwrap() = vec![
                a as u64,
                b as u64,
                c as u64,
                d as u64,
                e as u64,
                f as u64,
                g as u64,
                h as u64,
                i.to_bits() as u64,
            ];
        }
        unsafe extern "system" fn fake_input(
            a: usize,
            b: usize,
            c: usize,
            dt: f32,
            e: usize,
            f: usize,
            g: usize,
        ) {
            *RECEIVED.lock().unwrap() = vec![
                a as u64,
                b as u64,
                c as u64,
                dt.to_bits() as u64,
                e as u64,
                f as u64,
                g as u64,
            ];
        }
        #[test]
        fn follow_payload_uses_team_and_lane_instead_of_sdk_player_id() {
            for side in 0..2 {
                for lane in 0..5 {
                    let mut config = [0xccu8; 0x60];
                    let addr = config.as_mut_ptr() as usize;
                    unsafe {
                        set_follow_config(addr, side, lane);
                    }
                    assert_eq!(
                        u32::from_le_bytes(config[0x18..0x1c].try_into().unwrap()),
                        2
                    );
                    assert_eq!(
                        u32::from_le_bytes(config[0x1c..0x20].try_into().unwrap()),
                        lane
                    );
                    assert_eq!(
                        usize::from_le_bytes(config[0x20..0x28].try_into().unwrap()),
                        side
                    );
                    assert!(config[..0x18]
                        .iter()
                        .chain(config[0x28..].iter())
                        .all(|b| *b == 0xcc));
                }
            }
        }
        #[test]
        fn spectator_filter_covers_press_release_and_preserves_mouse_zoom() {
            for tag in [0x8000000000000006, 0x8000000000000007] {
                assert!(spectator_key_is_owned(tag, 0x17)); // default S pause shortcut
                assert!(spectator_key_is_owned(tag, 4)); // default Tab info shortcut
                assert!(!spectator_key_is_owned(tag, 0x3d));
                assert!(!spectator_key_is_owned(tag, 0x3e));
                assert!(!spectator_key_is_owned(tag, 0x21));
            }
            for tag in [
                0,
                0x8000000000000001,
                0x8000000000000003,
                0x8000000000000004,
                0x8000000000000005,
                0x800000000000000d,
            ] {
                assert!(!spectator_key_is_owned(tag, 0x17));
            }
        }
        #[test]
        fn windows_hash_rel32_and_forwarding_abi() {
            assert_eq!(
                relative_call(STEER_SITE, STEER_ORIGINAL).unwrap(),
                STEER_BYTES
            );
            ORIGINAL_STEER.store(fake_steer as *const () as usize, Ordering::Release);
            unsafe {
                steer_hook(1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
            }
            assert_eq!(
                *RECEIVED.lock().unwrap(),
                vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
            );
            let step: DirectStepFn = fake_step;
            unsafe {
                step(1, 2, 3, 4, 5, 6, 7, 8);
            }
            assert_eq!(*RECEIVED.lock().unwrap(), vec![1, 2, 3, 4, 5, 6, 7, 8]);
            assert_eq!(
                sha256(b"abc").unwrap(),
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
            );
            assert_eq!(
                relative_call(WORKER_SITE, WORKER_ORIGINAL).unwrap(),
                WORKER_BYTES
            );
            assert_eq!(relative_call(VIEW_SITE, VIEW_ORIGINAL).unwrap(), VIEW_BYTES);
            assert_eq!(relative_jump(MOVE_SITE, MOVE_ORIGINAL).unwrap(), MOVE_BYTES);
            assert_eq!(
                relative_call(INPUT_SITE, INPUT_ORIGINAL).unwrap(),
                INPUT_BYTES
            );
            assert_eq!(
                relative_call(ATTACK_SITE, ATTACK_ORIGINAL).unwrap(),
                ATTACK_BYTES
            );
            ORIGINAL_ATTACK.store(fake_attack as *const () as usize, Ordering::Release);
            unsafe {
                attack_hook(11, 22, 33);
            }
            assert_eq!(*RECEIVED.lock().unwrap(), vec![11, 22, 33]);
            assert_eq!(
                relative_call(AUTO_ATTACK_SITE, ATTACK_ORIGINAL).unwrap(),
                AUTO_ATTACK_BYTES
            );
            unsafe {
                auto_attack_hook(44, 55, 66);
            }
            assert_eq!(*RECEIVED.lock().unwrap(), vec![44, 55, 66]);
            for slot in 0..3 {
                assert_eq!(
                    relative_call(SKILL_SITES[slot], SKILL_ORIGINALS[slot]).unwrap(),
                    SKILL_BYTES[slot]
                );
                ORIGINAL_SKILLS[slot].store(fake_attack as *const () as usize, Ordering::Release);
                unsafe {
                    skill_hook(slot, 11, 22, 33);
                }
                assert_eq!(*RECEIVED.lock().unwrap(), vec![11, 22, 33]);
            }
            let attack_relay = unsafe {
                relay(
                    fake_attack as *const () as usize,
                    fake_attack as *const () as usize,
                )
            }
            .unwrap();
            let relay_attack: AttackFn = unsafe { std::mem::transmute(attack_relay) };
            unsafe {
                relay_attack(44, 55, 66);
            }
            assert_eq!(*RECEIVED.lock().unwrap(), vec![44, 55, 66]);
            ORIGINAL_INPUT.store(fake_input as *const () as usize, Ordering::Release);
            unsafe {
                input_hook(1, 2, 3, 0.0125, 5, 6, 7);
            }
            assert_eq!(
                *RECEIVED.lock().unwrap(),
                vec![1, 2, 3, 0.0125f32.to_bits() as u64, 5, 6, 7]
            );
            let input_relay = unsafe {
                relay(
                    fake_input as *const () as usize,
                    fake_input as *const () as usize,
                )
            }
            .unwrap();
            let relay_input: NativeInputFn = unsafe { std::mem::transmute(input_relay) };
            unsafe {
                relay_input(7, 6, 5, 0.025, 3, 2, 1);
            }
            assert_eq!(
                *RECEIVED.lock().unwrap(),
                vec![7, 6, 5, 0.025f32.to_bits() as u64, 3, 2, 1]
            );
            assert!(relative_call(0, usize::MAX).is_err());
            ORIGINAL_WORKER.store(fake_worker as *const () as usize, Ordering::Release);
            ORIGINAL_VIEW.store(fake_view as *const () as usize, Ordering::Release);
            ORIGINAL_MOVE.store(fake_move as *const () as usize, Ordering::Release);
            unsafe {
                move_hook(11, 22, 33, 44);
            }
            assert_eq!(*RECEIVED.lock().unwrap(), vec![11, 22, 33, 44]);
            unsafe {
                let mut sent = 0usize;
                worker_hook(&mut sent as *mut usize as usize, 22, 33);
                assert_eq!(sent, usize::MAX);
                assert_eq!(
                    *RECEIVED.lock().unwrap(),
                    vec![&mut sent as *mut usize as u64, 22, 33]
                );
            }
            let worker_relay = unsafe {
                relay(
                    fake_worker as *const () as usize,
                    fake_worker as *const () as usize,
                )
            }
            .unwrap();
            let relay_worker: WorkerFn = unsafe { std::mem::transmute(worker_relay) };
            unsafe {
                let mut sent = 0usize;
                relay_worker(&mut sent as *mut usize as usize, 55, 66);
                assert_eq!(sent, usize::MAX);
                assert_eq!(
                    *RECEIVED.lock().unwrap(),
                    vec![&mut sent as *mut usize as u64, 55, 66]
                );
            }
            // SHARED is intentionally absent: passthrough preserves all nine
            // arguments, including the fifth pointer and ninth stack f32 slot.
            unsafe {
                view_hook(1, 2, 3, 4, 5, 6, 60, 8, 0.0125);
            }
            assert_eq!(
                *RECEIVED.lock().unwrap(),
                vec![1, 2, 3, 4, 5, 6, 60, 8, 0.0125f32.to_bits() as u64]
            );
            let view_relay = unsafe {
                relay(
                    fake_view as *const () as usize,
                    fake_view as *const () as usize,
                )
            }
            .unwrap();
            let relay_view: ViewFn = unsafe { std::mem::transmute(view_relay) };
            unsafe {
                relay_view(9, 8, 7, 6, 5, 4, 60, 2, 0.025);
            }
            assert_eq!(
                *RECEIVED.lock().unwrap(),
                vec![9, 8, 7, 6, 5, 4, 60, 2, 0.025f32.to_bits() as u64]
            );
        }
        #[test]
        fn stop_gate_preserves_nonmovement_actions_and_forced_movement() {
            let mut actor = vec![0u64; 0x6c0 / 8];
            let address = actor.as_mut_ptr() as usize;
            let mut effects = [0u64; 5];
            unsafe {
                std::ptr::write_unaligned((address + 0x68) as *mut u32, 15);
                for action in 0..7 {
                    std::ptr::write_unaligned((address + 0x70) as *mut usize, action);
                    assert_eq!(can_stop_movement(address), action == 2);
                }
                std::ptr::write_unaligned((address + 0x70) as *mut usize, 2);
                std::ptr::write_unaligned(
                    (address + 0x2c0) as *mut usize,
                    effects.as_mut_ptr() as usize,
                );
                std::ptr::write_unaligned((address + 0x2c8) as *mut usize, 1);
                std::ptr::write_unaligned(effects.as_mut_ptr(), 6);
                assert!(!can_stop_movement(address));
                std::ptr::write_unaligned(effects.as_mut_ptr(), 3);
                assert!(can_stop_movement(address));
                std::ptr::write_unaligned((address + 0x68) as *mut u32, 0);
                assert!(!can_stop_movement(address));
            }
        }
        #[test]
        fn stop_emits_native_event_once_and_clears_only_movement() {
            unsafe extern "system" fn notify(events: usize, actor: usize) {
                let seen = &mut *(events as *mut Vec<usize>);
                seen.push(actor);
            }
            let mut entity = vec![0u64; 0x6c0 / 8];
            let address = entity.as_mut_ptr() as usize;
            let mut seen = Vec::<usize>::new();
            let events = &mut seen as *mut Vec<usize> as usize;
            unsafe {
                std::ptr::write_unaligned((address + 0x68) as *mut u32, 15);
                std::ptr::write_unaligned((address + 0x70) as *mut usize, 2);
                assert!(stop_movement(address, 17, events, notify));
                assert_eq!(
                    std::ptr::read_unaligned((address + 0x70) as *const usize),
                    0
                );
                assert_eq!(seen, vec![17]);
                assert!(stop_movement(address, 17, events, notify));
                assert_eq!(seen, vec![17]);
                for action in 3..7 {
                    std::ptr::write_unaligned((address + 0x70) as *mut usize, action);
                    assert!(!stop_movement(address, 17, events, notify));
                    assert_eq!(
                        std::ptr::read_unaligned((address + 0x70) as *const usize),
                        action
                    );
                }
                assert_eq!(seen, vec![17]);
            }
        }
        #[test]
        fn recall_stop_emits_once_and_preserves_timer_effects_and_other_actions() {
            unsafe extern "system" fn notify(events: usize, actor: usize) {
                (&mut *(events as *mut Vec<usize>)).push(actor);
            }
            let mut entity = vec![0xccu8; 0x6c0];
            let addr = entity.as_mut_ptr() as usize;
            entity[0x68..0x6c].copy_from_slice(&15u32.to_le_bytes());
            entity[0x70..0x78].copy_from_slice(&1usize.to_le_bytes());
            let before = entity.clone();
            let mut seen = Vec::<usize>::new();
            let events = &mut seen as *mut Vec<usize> as usize;
            unsafe {
                assert!(stop_recall(addr, 17, events, notify));
                assert_eq!(seen, vec![17]);
                assert_eq!(
                    usize::from_le_bytes(entity[0x70..0x78].try_into().unwrap()),
                    0
                );
                assert_eq!(&entity[..0x70], &before[..0x70]);
                assert_eq!(&entity[0x78..], &before[0x78..]);
                assert!(!stop_recall(addr, 17, events, notify));
                for action in [2usize, 3, 4, 5, 6] {
                    std::ptr::write_unaligned((addr + 0x70) as *mut usize, action);
                    assert!(!stop_recall(addr, 17, events, notify));
                    assert_eq!(
                        std::ptr::read_unaligned((addr + 0x70) as *const usize),
                        action
                    );
                }
                assert_eq!(seen, vec![17]);
            }
        }
        #[test]
        fn scoped_config_restores_every_overridden_bit() {
            let mut config = [0u8; 0x60];
            config[0] = 1;
            config[0x10..0x14].copy_from_slice(&1u32.to_le_bytes());
            config[0x14..0x18].copy_from_slice(&3.0f32.to_bits().to_le_bytes());
            let before = config;
            let snapshot = unsafe { PlaybackOverride::apply(config.as_mut_ptr() as usize) };
            assert_eq!(config[0], 0);
            assert_eq!(
                u32::from_le_bytes(config[0x14..0x18].try_into().unwrap()),
                1.0f32.to_bits()
            );
            drop(snapshot);
            assert_eq!(config, before);
        }
        #[test]
        fn stack_capture_is_available_and_logs_module_relative_frames() {
            let log = crate::test_support::logger("native-stack");
            capture_trace("test stack", &log);
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("target/test-native-stack.log");
            let text = std::fs::read_to_string(path).unwrap();
            assert!(text.contains("NATIVE STACK"));
            assert!(text.contains(".exe+0x"));
            assert!(!text.contains("frames=[]"));
        }
    }
}
