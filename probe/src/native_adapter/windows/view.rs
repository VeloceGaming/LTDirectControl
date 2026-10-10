//! The viewer (client) and worker (simulation) hooks: frame pacing,
//! camera control and the full-screen layout lease.
use super::*;

pub(crate) static ORIGINAL_WORKER: AtomicUsize = AtomicUsize::new(0);
pub(crate) static ORIGINAL_VIEW: AtomicUsize = AtomicUsize::new(0);
// Sender::send: output Result pointer, sender pointer, owned frame pointer.
// The native caller inspects the output memory, not RAX, after return.
pub(crate) type WorkerFn = unsafe extern "system" fn(usize, usize, usize);
// Verified: four GP arguments, four stack pointer/usize arguments, then
// stack f32 dt. Native caller ignores the result (update returns unit).
pub(crate) type ViewFn =
    unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, usize, usize, f32);
/// The viewer's eighth argument is the live `ingame` UiNode (cdcbf6..
/// cdcce0), also consumed by b9cfc5..b9d02c. Use the same checked downcast
/// as native code; do not retain a registry lookup or dereference an old UI.
pub(crate) unsafe fn ingame_ui_from_node(node: usize, config: usize) -> Option<usize> {
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
pub(crate) unsafe fn ingame_matches_config(ingame: usize, config: usize) -> bool {
    ingame != 0
        && config >= 0x18
        && std::ptr::read_unaligned((ingame + 0x9150) as *const usize) == config - 0x18
}
pub(crate) struct CameraLease {
    view: usize,
    config: usize,
    ingame: usize,
    original: [u8; 16],
    original_vision: u8,
    original_full_width: u8,
    original_ui_full_width: u8,
    effective_vision: u8,
    ai_camera: bool,
}
impl CameraLease {
    pub(crate) unsafe fn capture(view: usize, config: usize, ingame: usize) -> Self {
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
            ai_camera: false,
        }
    }
    fn matches(&self, view: usize, config: usize) -> bool {
        self.view == view && self.config == config
    }
    unsafe fn layout_matches(&self, view: usize, config: usize, ingame: usize) -> bool {
        self.matches(view, config) && self.ingame == ingame && ingame_matches_config(ingame, config)
    }
    pub(crate) unsafe fn full_width(&self, view: usize, config: usize, ingame: usize) -> bool {
        if !self.layout_matches(view, config, ingame) {
            return false;
        }
        // Native toggle 7b4de5..7b4df1 changes both flags. UI layout reads
        // +9234 separately to position announcements, kills and controls.
        std::ptr::write((ingame + 0x9234) as *mut u8, 1);
        std::ptr::write((config + 0x45) as *mut u8, 1);
        true
    }
    pub(crate) unsafe fn restore(&self, view: usize, config: usize, ingame: usize) -> bool {
        if !self.layout_matches(view, config, ingame) {
            return false;
        }
        std::ptr::copy_nonoverlapping(self.original.as_ptr(), (config + 0x18) as *mut u8, 16);
        std::ptr::write((config + 0x4b) as *mut u8, self.original_vision);
        // Released from AI control: the layout is the spectator's already
        // (they may have toggled it, e.g. to reach "view result").
        if !self.ai_camera {
            std::ptr::write((config + 0x45) as *mut u8, self.original_full_width);
            std::ptr::write((ingame + 0x9234) as *mut u8, self.original_ui_full_width);
        }
        true
    }
}
pub(crate) static CAMERA_LEASE: std::sync::Mutex<Option<CameraLease>> = std::sync::Mutex::new(None);
pub(crate) fn reset_session() {
    if let Ok(mut lease) = CAMERA_LEASE.lock() {
        *lease = None;
    }
}
pub(crate) unsafe fn camera_frame(
    view: usize,
    config: usize,
) -> Option<crate::camera::CameraFrame> {
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
    let custom =
        (std::ptr::read_unaligned((view + 0x124) as *const u32) == 1).then(|| (f(0x128), f(0x12c)));
    let minimap = crate::minimap::content_rect(wide, left, custom);
    let frame = CameraFrame {
        viewport,
        minimap,
        center: (f(0x114), f(0x118)),
        extent: (f(0x11c), f(0x120)),
        zoom: f(0x110),
    };
    frame.valid().then_some(frame)
}
pub(crate) unsafe fn restore_camera(
    view: usize,
    config: usize,
    ingame: Option<usize>,
    shared: &Shared,
) {
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
pub(crate) unsafe fn set_follow_config(config: usize, side: usize, lane: u32) {
    // CURRENT 1fcfd5d reads +20 team and +1c lane to look up its player map.
    // Native own-mid shortcut cb020e writes the same pair (team, lane=2).
    std::ptr::write_unaligned((config + 0x18) as *mut u32, 2);
    std::ptr::write_unaligned((config + 0x1c) as *mut u32, lane);
    std::ptr::write_unaligned((config + 0x20) as *mut usize, side);
}
pub(crate) unsafe fn prepare_camera(
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
        let Some(saved) = lease.as_mut() else {
            return;
        };
        if shared.timing.phase() == Some(crate::native_timing::Phase::Ai) {
            // AI control: the spectator's camera, vision and layout come
            // back once, then the game's own toggles (full screen F and its
            // button) work; full screen is forced again on taking control.
            if saved.layout_matches(view, config, ingame) && !saved.ai_camera {
                std::ptr::copy_nonoverlapping(
                    saved.original.as_ptr(),
                    (config + 0x18) as *mut u8,
                    16,
                );
                std::ptr::write((config + 0x4b) as *mut u8, saved.original_vision);
                std::ptr::write((config + 0x45) as *mut u8, saved.original_full_width);
                std::ptr::write((ingame + 0x9234) as *mut u8, saved.original_ui_full_width);
                saved.ai_camera = true;
                saved.effective_vision = u8::MAX;
            }
            return;
        }
        if !saved.full_width(view, config, ingame) {
            return;
        }
        saved.ai_camera = false;
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
                std::ptr::write_unaligned((view + 0x114) as *mut f32, champion.0 as f32 / 1000.);
                std::ptr::write_unaligned((view + 0x118) as *mut f32, champion.1 as f32 / 1000.);
            }
            set_follow_config(config, target.side, target.lane as u32);
        }
        None => {}
    }
}

pub(crate) unsafe extern "system" fn worker_hook(output: usize, sender: usize, frame: usize) {
    STOP_TICKET.set(None);
    WORKER_ENTRIES.fetch_add(1, Ordering::Relaxed);
    let original: WorkerFn = std::mem::transmute(ORIGINAL_WORKER.load(Ordering::Acquire));
    // Publish before counting or waiting. This call site is AFTER runner,
    // highlights and highlight-segments write guards have been released.
    crate::perf::worker_thread();
    let tick = crate::perf::time(crate::perf::Section::WorkerSend);
    let send = crate::worker_watch::hook(crate::worker_watch::Step::Send);
    original(output, sender, frame);
    drop(send);
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

/// Snapshot only these playback fields; their borrow belongs to the
/// native caller and spans this call. Restore before that borrow is released.
pub(crate) struct PlaybackOverride {
    config: usize,
    sync: u8,
    mode: u32,
    speed: u32,
    paused: u8,
}
impl PlaybackOverride {
    pub(crate) unsafe fn apply(config: usize, manual: bool) -> Self {
        let snapshot = Self {
            config,
            sync: std::ptr::read(config as *const u8),
            mode: std::ptr::read_unaligned((config + 0x10) as *const u32),
            speed: std::ptr::read_unaligned((config + 0x14) as *const u32),
            paused: std::ptr::read((config + 0x48) as *const u8),
        };
        std::ptr::write(config as *mut u8, 0);
        std::ptr::write_unaligned((config + 0x10) as *mut u32, 0);
        std::ptr::write_unaligned((config + 0x14) as *mut f32, 1.0);
        // AI spectator pause remains usable. On reclaim, the coordinator's
        // own Running/Paused phase drives playback even if AI was paused.
        if manual {
            std::ptr::write((config + 0x48) as *mut u8, 0);
        }
        snapshot
    }
}
impl Drop for PlaybackOverride {
    fn drop(&mut self) {
        unsafe {
            std::ptr::write(self.config as *mut u8, self.sync);
            std::ptr::write_unaligned((self.config + 0x10) as *mut u32, self.mode);
            std::ptr::write_unaligned((self.config + 0x14) as *mut u32, self.speed);
            std::ptr::write((self.config + 0x48) as *mut u8, self.paused);
        }
    }
}
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe extern "system" fn view_hook(
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
    if let Some(shared) = SHARED.get() {
        // Native text is resolved after the original update, while this
        // viewer's Assets borrow is still live. The adapter owns no pointers.
        tooltips::resolve_pending(view, assets, shared);
    }
}
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn view_hook_body(
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
    let snapshot = PlaybackOverride::apply(
        config,
        shared.timing.phase() != Some(crate::native_timing::Phase::Ai),
    );
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
        if let Some(saved) = lease.as_ref().filter(|l| !l.ai_camera) {
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
