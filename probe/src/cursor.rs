//! Native pointer artwork; no input injection or smoothed pointer position.
use crate::Logger;
use std::{
    path::Path,
    sync::{atomic::Ordering, Arc, Mutex},
    time::Duration,
};
pub const DEFAULT_SIZE: u32 = 32;
pub fn size_from_ratio(ratio: f64) -> u32 {
    if !ratio.is_finite() {
        return DEFAULT_SIZE;
    }
    (24. + ratio.clamp(0., 1.) * 40.).round() as u32
}
pub fn ratio_from_size(size: u32) -> f64 {
    (size.clamp(24, 64) - 24) as f64 / 40.
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillCursor {
    Free,
    Valid,
    Invalid,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Style {
    Normal,
    Hostile,
    Attack,
    Locked,
    Champion,
    ChampionHostile,
    ChampionAttack,
    ChampionLocked,
    Skill,
    SkillChampion,
    SkillValid,
    SkillValidChampion,
    SkillInvalid,
    SkillInvalidChampion,
}
impl Style {
    pub fn choose(champion: bool, attack: bool, enemy: bool, skill: Option<SkillCursor>) -> Self {
        if let Some(skill) = skill {
            return match (skill, champion) {
                (SkillCursor::Free, false) => Self::Skill,
                (SkillCursor::Free, true) => Self::SkillChampion,
                (SkillCursor::Valid, false) => Self::SkillValid,
                (SkillCursor::Valid, true) => Self::SkillValidChampion,
                (SkillCursor::Invalid, false) => Self::SkillInvalid,
                (SkillCursor::Invalid, true) => Self::SkillInvalidChampion,
            };
        }
        match (champion, attack, enemy) {
            (false, false, false) => Self::Normal,
            (false, false, true) => Self::Hostile,
            (false, true, false) => Self::Attack,
            (false, true, true) => Self::Locked,
            (true, false, false) => Self::Champion,
            (true, false, true) => Self::ChampionHostile,
            (true, true, false) => Self::ChampionAttack,
            (true, true, true) => Self::ChampionLocked,
        }
    }
    fn hotspot(self, size: u32) -> (u32, u32) {
        let (x, y) = match self {
            Self::Normal | Self::Champion => (3., 2.),
            Self::Hostile | Self::ChampionHostile => (2., 2.),
            _ => (16., 16.),
        };
        (
            (x * size as f32 / 32.).round() as u32,
            (y * size as f32 / 32.).round() as u32,
        )
    }
}
const STYLES: [Style; 14] = [
    Style::Normal,
    Style::Hostile,
    Style::Attack,
    Style::Locked,
    Style::Champion,
    Style::ChampionHostile,
    Style::ChampionAttack,
    Style::ChampionLocked,
    Style::Skill,
    Style::SkillChampion,
    Style::SkillValid,
    Style::SkillValidChampion,
    Style::SkillInvalid,
    Style::SkillInvalidChampion,
];
const PIXELS: [&[u8]; 14] = [
    include_bytes!("../cursor/normal.bgra"),
    include_bytes!("../cursor/hostile.bgra"),
    include_bytes!("../cursor/amove.bgra"),
    include_bytes!("../cursor/alock.bgra"),
    include_bytes!("../cursor/champ.bgra"),
    include_bytes!("../cursor/champHostile.bgra"),
    include_bytes!("../cursor/achamp.bgra"),
    include_bytes!("../cursor/achampLock.bgra"),
    include_bytes!("../cursor/skill.bgra"),
    include_bytes!("../cursor/skillChamp.bgra"),
    include_bytes!("../cursor/skillValid.bgra"),
    include_bytes!("../cursor/skillValidChamp.bgra"),
    include_bytes!("../cursor/skillInvalid.bgra"),
    include_bytes!("../cursor/skillInvalidChamp.bgra"),
];
fn pixels(style: Style, size: u32) -> Vec<u8> {
    let size = size.clamp(24, 64) as usize;
    let source = PIXELS[style as usize];
    let mut output = vec![0; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let sample = |i: usize| ((i as f32 + 0.5) * 64. / size as f32 - 0.5).clamp(0., 63.);
            let (sx, sy) = (sample(x), sample(y));
            let (ix, iy) = (sx.floor() as usize, sy.floor() as usize);
            let (fx, fy) = (sx - ix as f32, sy - iy as f32);
            for c in 0..4 {
                let at = |a: usize, b: usize| source[(b.min(63) * 64 + a.min(63)) * 4 + c] as f32;
                let top = at(ix, iy) * (1. - fx) + at(ix + 1, iy) * fx;
                let bottom = at(ix, iy + 1) * (1. - fx) + at(ix + 1, iy + 1) * fx;
                output[(y * size + x) * 4 + c] = (top * (1. - fy) + bottom * fy).round() as u8;
            }
        }
    }
    output
}
pub use crate::settings::Settings;
pub struct Cursor {
    pub settings: Arc<Settings>,
    backend: Mutex<windows::Backend>,
}
impl Cursor {
    pub fn new(root: Option<&Path>) -> Self {
        let settings = Arc::new(Settings::new(root));
        let _ = crate::settings::GLOBAL.set(settings.clone());
        Self {
            settings,
            backend: Mutex::default(),
        }
    }
    /// `Err` names why the game keeps its own cursor this frame (diagnostic).
    pub fn update(
        &self,
        style: Result<Style, &'static str>,
        point: Option<(f32, f32)>,
        log: &Logger,
    ) {
        self.settings.flush(false, log);
        if let Ok(mut backend) = self.backend.lock() {
            backend.trace(style, point, log);
            backend.update(style.ok(), self.settings.size(), log);
        }
    }
    pub fn shutdown(&self, log: &Logger) {
        self.settings.flush(true, log);
        if let Ok(mut b) = self.backend.lock() {
            b.shutdown();
        }
    }
}

// Solve the CSS cubic-bezier x coordinate, rather than using t as wall time.
fn easing(time: f32, controls: (f32, f32, f32, f32)) -> f32 {
    let (x1, y1, x2, y2) = controls;
    let cubic = |t: f32, a: f32, b: f32| {
        3. * (1. - t).powi(2) * t * a + 3. * (1. - t) * t * t * b + t.powi(3)
    };
    let (mut lo, mut hi) = (0., 1.);
    for _ in 0..16 {
        let t = (lo + hi) / 2.;
        if cubic(t, x1, x2) < time {
            lo = t;
        } else {
            hi = t;
        }
    }
    cubic((lo + hi) / 2., y1, y2)
}
fn marker_motion(elapsed: Duration) -> Option<(f32, f32)> {
    let ms = elapsed.as_secs_f32() * 1000.;
    if ms >= 250. {
        return None;
    }
    let scale = if ms >= 167. {
        1.
    } else {
        1.9 - 0.9 * easing(ms / 167., (0.15, 0.8, 0.3, 1.))
    };
    let opacity = if ms <= 100. {
        1.
    } else {
        1. - easing((ms - 100.) / 150., (0.4, 0., 0.5, 1.))
    };
    Some((scale, opacity))
}
pub fn draw_clicks(
    ctx: &mut mod_api_stable::StableClient<'_>,
    camera: &crate::camera::CameraControl,
    clicks: &[crate::movement::ClickFeedback],
) {
    let Some(frame) = camera.frame() else {
        return;
    };
    let blockers = camera.overlay_blockers();
    for click in clicks {
        let Some((scale, opacity)) = marker_motion(click.at.elapsed()) else {
            continue;
        };
        let (center, clip, scale) = if click.minimap {
            (
                frame.project_minimap(click.position),
                crate::camera::CameraFrame {
                    viewport: frame.minimap,
                    ..frame
                },
                scale * 0.5,
            )
        } else {
            (
                frame.project_unclipped(
                    click.position.0 as f32 / 1000.,
                    click.position.1 as f32 / 1000.,
                ),
                frame,
                scale,
            )
        };
        let mut blockers = blockers.clone();
        if !click.minimap {
            blockers.push(frame.minimap);
        }
        let point = |x: f32, y: f32| (center.0 + (x - 24.) * scale, center.1 + (y - 24.) * scale);
        let alpha = (opacity * 255.).round() as u32;
        let color = if click.attack {
            0xff642e00 | alpha
        } else {
            0xb3d54300 | alpha
        };
        let ink = 0x1c1c1c00 | alpha;
        let mut drawing = crate::skill_preview::Drawing::default();
        let corners = [
            ((8., 18.), (8., 8.)),
            ((8., 8.), (18., 8.)),
            ((30., 8.), (40., 8.)),
            ((40., 8.), (40., 18.)),
            ((40., 30.), (40., 40.)),
            ((40., 40.), (30., 40.)),
            ((18., 40.), (8., 40.)),
            ((8., 40.), (8., 30.)),
        ];
        for (width, c, z) in [(5., ink, 1002), (2.5, color, 1003)] {
            for (a, b) in corners {
                drawing.line(point(a.0, a.1), point(b.0, b.1), width * scale, c, z);
            }
        }
        // Filled center marks, scanlines clipped with the same HUD masks.
        if click.attack {
            for y in -6i32..=6 {
                let half = (6 - y.abs()) as f32;
                drawing.line(
                    point(24. - half, 24. + y as f32),
                    point(24. + half, 24. + y as f32),
                    scale,
                    color,
                    1003,
                );
            }
            let diamond = [(24., 18.), (30., 24.), (24., 30.), (18., 24.)];
            for i in 0..4 {
                let a = diamond[i];
                let b = diamond[(i + 1) % 4];
                drawing.line(point(a.0, a.1), point(b.0, b.1), 1.5 * scale, ink, 1004);
            }
        } else {
            for y in 22..=26 {
                drawing.line(
                    point(22., y as f32),
                    point(26., y as f32),
                    scale,
                    color,
                    1003,
                );
            }
            let square = [(22., 22.), (26., 22.), (26., 26.), (22., 26.)];
            for i in 0..4 {
                let a = square[i];
                let b = square[(i + 1) % 4];
                drawing.line(point(a.0, a.1), point(b.0, b.1), scale, ink, 1004);
            }
        }
        drawing.render(ctx, clip, &blockers);
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize};
    use std::{ffi::c_void, sync::atomic::AtomicIsize};
    static CURRENT: AtomicIsize = AtomicIsize::new(0);
    static WINDOW: AtomicIsize = AtomicIsize::new(0);
    static WINDOW_THREAD: AtomicU32 = AtomicU32::new(0);
    // The game's own SetCursor calls (import-table hook: ours is shown in
    // their place while our cursor is active) and the WM_SETCURSOR observer.
    static REAL_SET_CURSOR: AtomicUsize = AtomicUsize::new(0);
    static IMPORT_SLOT: AtomicUsize = AtomicUsize::new(0);
    static GAME_CALLS: AtomicU64 = AtomicU64::new(0);
    static GAME_CALLS_WHILE_OURS: AtomicU64 = AtomicU64::new(0);
    static GAME_CALLS_OTHER_THREAD: AtomicU64 = AtomicU64::new(0);
    static GAME_HANDLE: AtomicIsize = AtomicIsize::new(0);
    static SUBSTITUTED: AtomicU64 = AtomicU64::new(0);
    static SETCURSOR_MESSAGES: AtomicU64 = AtomicU64::new(0);
    static REAPPLIED: AtomicU64 = AtomicU64::new(0);
    #[repr(C)]
    struct ReturnMessage {
        result: isize,
        lparam: isize,
        wparam: usize,
        message: u32,
        window: isize,
    }
    #[repr(C)]
    struct BitmapInfo {
        size: u32,
        width: i32,
        height: i32,
        planes: u16,
        bits: u16,
        compression: u32,
        image_size: u32,
        xppm: i32,
        yppm: i32,
        used: u32,
        important: u32,
        colors: [u32; 2],
    }
    #[repr(C)]
    struct IconInfo {
        icon: i32,
        x: u32,
        y: u32,
        mask: isize,
        color: isize,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
        fn VirtualProtect(address: *mut c_void, size: usize, protect: u32, old: *mut u32) -> i32;
        fn GetCurrentThreadId() -> u32;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> usize;
    }
    #[link(name = "gdi32")]
    extern "system" {
        fn CreateDIBSection(
            dc: isize,
            info: *const BitmapInfo,
            usage: u32,
            bits: *mut *mut c_void,
            section: isize,
            offset: u32,
        ) -> isize;
        fn DeleteObject(object: isize) -> i32;
    }
    #[link(name = "user32")]
    extern "system" {
        fn CreateIconIndirect(info: *const IconInfo) -> isize;
        fn DestroyCursor(cursor: isize) -> i32;
        fn GetCursor() -> isize;
        fn SetCursor(cursor: isize) -> isize;
        fn GetForegroundWindow() -> *mut c_void;
        fn GetWindowThreadProcessId(window: *mut c_void, process: *mut u32) -> u32;
        fn LoadCursorW(instance: *mut c_void, name: *const u16) -> isize;
        fn SetWindowsHookExW(
            id: i32,
            callback: unsafe extern "system" fn(i32, usize, isize) -> isize,
            module: *mut c_void,
            thread: u32,
        ) -> isize;
        fn UnhookWindowsHookEx(handle: isize) -> i32;
        fn CallNextHookEx(handle: isize, code: i32, wparam: usize, lparam: isize) -> isize;
    }
    unsafe fn create(style: Style, size: u32) -> isize {
        let image = pixels(style, size);
        let mut info = BitmapInfo {
            size: 40,
            width: size as i32,
            height: -(size as i32),
            planes: 1,
            bits: 32,
            compression: 0,
            image_size: 0,
            xppm: 0,
            yppm: 0,
            used: 0,
            important: 0,
            colors: [0, 0xffffff],
        };
        let mut bits = std::ptr::null_mut();
        let color = CreateDIBSection(0, &info, 0, &mut bits, 0, 0);
        if color == 0 {
            return 0;
        }
        std::ptr::copy_nonoverlapping(image.as_ptr(), bits.cast::<u8>(), image.len());
        info.bits = 1;
        let mask = CreateDIBSection(0, &info, 0, &mut bits, 0, 0);
        if mask == 0 {
            DeleteObject(color);
            return 0;
        }
        let stride = (size as usize).div_ceil(32) * 4;
        std::ptr::write_bytes(bits, 0, stride * size as usize);
        let mask_bits = std::slice::from_raw_parts_mut(bits.cast::<u8>(), stride * size as usize);
        for y in 0..size as usize {
            for x in 0..size as usize {
                if image[(y * size as usize + x) * 4 + 3] == 0 {
                    mask_bits[y * stride + x / 8] |= 0x80 >> (x % 8);
                }
            }
        }
        let (x, y) = style.hotspot(size);
        let result = CreateIconIndirect(&IconInfo {
            icon: 0,
            x,
            y,
            mask,
            color,
        });
        DeleteObject(mask);
        DeleteObject(color);
        result
    }
    unsafe extern "system" fn after_message(code: i32, wparam: usize, lparam: isize) -> isize {
        // Observer only: run after the game's WM_SETCURSOR handler, never consume
        // mouse input or replace the window procedure.
        if code >= 0 && lparam != 0 {
            let msg = &*(lparam as *const ReturnMessage);
            let cursor = CURRENT.load(Ordering::Acquire);
            if msg.message == 0x20
                && msg.lparam & 0xffff == 1
                && msg.window == WINDOW.load(Ordering::Relaxed)
            {
                SETCURSOR_MESSAGES.fetch_add(1, Ordering::Relaxed);
            }
            if cursor != 0
                && msg.message == 0x20
                && msg.lparam & 0xffff == 1
                && msg.window == WINDOW.load(Ordering::Relaxed)
                && msg.window == GetForegroundWindow() as isize
            {
                REAPPLIED.fetch_add(1, Ordering::Relaxed);
                SetCursor(cursor);
            }
        }
        CallNextHookEx(0, code, wparam, lparam)
    }
    /// The game's SetCursor import. The game sets its cursor on every
    /// WM_SETCURSOR (each mouse move); setting it and then ours flashed the
    /// game's cursor (0.65 log). While ours is active on the foreground game
    /// window, the game's call shows ours instead, so only one is ever set.
    unsafe extern "system" fn game_set_cursor(cursor: isize) -> isize {
        GAME_CALLS.fetch_add(1, Ordering::Relaxed);
        let ours = CURRENT.load(Ordering::Acquire);
        let on_window_thread = GetCurrentThreadId() == WINDOW_THREAD.load(Ordering::Relaxed);
        if !on_window_thread {
            GAME_CALLS_OTHER_THREAD.fetch_add(1, Ordering::Relaxed);
        }
        let mut shown = cursor;
        if ours != 0 && cursor != ours {
            GAME_CALLS_WHILE_OURS.fetch_add(1, Ordering::Relaxed);
            GAME_HANDLE.store(cursor, Ordering::Relaxed);
            if on_window_thread && GetForegroundWindow() as isize == WINDOW.load(Ordering::Relaxed)
            {
                SUBSTITUTED.fetch_add(1, Ordering::Relaxed);
                shown = ours;
            }
        }
        let real: unsafe extern "system" fn(isize) -> isize =
            std::mem::transmute(REAL_SET_CURSOR.load(Ordering::Acquire));
        real(shown)
    }
    /// Locate `dll!name` in the executable's import address table.
    unsafe fn import_slot(dll: &str, name: &str) -> Option<usize> {
        let base = GetModuleHandleW(std::ptr::null()) as usize;
        let u16_at = |a: usize| std::ptr::read_unaligned(a as *const u16);
        let u32_at = |a: usize| std::ptr::read_unaligned(a as *const u32) as usize;
        let text = |a: usize| std::ffi::CStr::from_ptr(a as *const std::ffi::c_char).to_bytes();
        if base == 0 || u16_at(base) != 0x5a4d {
            return None;
        }
        let nt = base + u32_at(base + 0x3c);
        let optional = nt + 24;
        if u32_at(nt) != 0x4550 || u16_at(optional) != 0x20b {
            return None;
        }
        let imports = u32_at(optional + 112 + 8);
        if imports == 0 {
            return None;
        }
        let mut descriptor = base + imports;
        for _ in 0..512 {
            let (lookup, dll_name, thunks) = (
                u32_at(descriptor),
                u32_at(descriptor + 12),
                u32_at(descriptor + 16),
            );
            if dll_name == 0 {
                return None;
            }
            if lookup != 0 && text(base + dll_name).eq_ignore_ascii_case(dll.as_bytes()) {
                for i in 0..4096 {
                    let entry = std::ptr::read_unaligned((base + lookup + i * 8) as *const u64);
                    if entry == 0 {
                        break;
                    }
                    if entry >> 63 == 0
                        && text(base + (entry as u32) as usize + 2) == name.as_bytes()
                    {
                        return Some(base + thunks + i * 8);
                    }
                }
            }
            descriptor += 20;
        }
        None
    }
    unsafe fn write_slot(slot: usize, value: usize) -> bool {
        let mut old = 0;
        if VirtualProtect(slot as *mut c_void, 8, 0x04, &mut old) == 0 {
            return false;
        }
        std::ptr::write_volatile(slot as *mut usize, value);
        VirtualProtect(slot as *mut c_void, 8, old, &mut old);
        true
    }
    /// Returns whether the counter is in place and a description proving the
    /// slot really is user32's SetCursor (or what else sits there).
    unsafe fn install_import_counter() -> (bool, String) {
        let user32: Vec<u16> = "user32.dll\0".encode_utf16().collect();
        let expected = GetProcAddress(
            GetModuleHandleW(user32.as_ptr()),
            c"SetCursor".as_ptr().cast(),
        );
        let Some(slot) = import_slot("user32.dll", "SetCursor") else {
            return (
                false,
                format!("import not found; user32_setcursor=0x{expected:x}"),
            );
        };
        let real = std::ptr::read_volatile(slot as *const usize);
        let detail = format!(
            "import_slot_original=0x{real:x} user32_setcursor=0x{expected:x} same={}",
            real == expected
        );
        if real == 0 || real == game_set_cursor as *const () as usize {
            return (false, detail);
        }
        REAL_SET_CURSOR.store(real, Ordering::Release);
        if !write_slot(slot, game_set_cursor as *const () as usize) {
            return (false, detail);
        }
        IMPORT_SLOT.store(slot, Ordering::Release);
        (true, detail)
    }
    unsafe fn remove_import_counter() {
        let slot = IMPORT_SLOT.swap(0, Ordering::AcqRel);
        if slot != 0
            && std::ptr::read_volatile(slot as *const usize)
                == game_set_cursor as *const () as usize
        {
            write_slot(slot, REAL_SET_CURSOR.load(Ordering::Acquire));
        }
    }
    #[derive(Default)]
    struct Trace {
        started: Option<std::time::Instant>,
        owned_frames: u64,
        released: Vec<(&'static str, u64)>,
        handbacks: Vec<(&'static str, u64)>,
        regains: u64,
        stolen: u64,
        stolen_handles: Vec<isize>,
        lines: u64,
        last_set: isize,
        previous: Option<Result<(), &'static str>>,
    }
    fn bump(list: &mut Vec<(&'static str, u64)>, reason: &'static str) {
        match list.iter_mut().find(|(r, _)| *r == reason) {
            Some((_, n)) => *n += 1,
            None => list.push((reason, 1)),
        }
    }
    #[derive(Default)]
    pub struct Backend {
        cache: std::collections::HashMap<u32, [isize; 14]>,
        hook: isize,
        saved: isize,
        owned: bool,
        attempted: bool,
        thread_warning: bool,
        import_counter: bool,
        trace: Trace,
    }
    impl Backend {
        /// 0.65 diagnostic: ownership changes and cursor replacements.
        pub fn trace(
            &mut self,
            style: Result<Style, &'static str>,
            point: Option<(f32, f32)>,
            log: &Logger,
        ) {
            let owned = self.owned;
            let ours = self.cache.values().next().map(|h| h[0]);
            let t = &mut self.trace;
            let now = std::time::Instant::now();
            let started = *t.started.get_or_insert(now);
            match style {
                Ok(_) => {
                    t.owned_frames += 1;
                    if matches!(t.previous, Some(Err(_))) {
                        t.regains += 1;
                    }
                    // Ours was set last frame; anything else now was set by
                    // someone else in between (and not undone by WM_SETCURSOR).
                    if owned && t.last_set != 0 {
                        let current = unsafe { GetCursor() };
                        if current != t.last_set {
                            t.stolen += 1;
                            if t.stolen_handles.len() < 4 && !t.stolen_handles.contains(&current) {
                                t.stolen_handles.push(current);
                            }
                        }
                    }
                }
                Err(reason) => {
                    bump(&mut t.released, reason);
                    if matches!(t.previous, Some(Ok(()))) {
                        bump(&mut t.handbacks, reason);
                        if t.lines < 6 {
                            t.lines += 1;
                            log.write(&format!(
                                "CURSOR HANDBACK reason={reason} point={point:?} owned_frames_in_window={}",
                                t.owned_frames
                            ));
                        }
                    }
                }
            }
            t.previous = Some(style.map(|_| ()));
            if now.duration_since(started) < Duration::from_secs(10) {
                return;
            }
            let calls = GAME_CALLS.swap(0, Ordering::Relaxed);
            let messages = SETCURSOR_MESSAGES.swap(0, Ordering::Relaxed);
            let while_ours = GAME_CALLS_WHILE_OURS.swap(0, Ordering::Relaxed);
            let other_thread = GAME_CALLS_OTHER_THREAD.swap(0, Ordering::Relaxed);
            let reapplied = REAPPLIED.swap(0, Ordering::Relaxed);
            let substituted = SUBSTITUTED.swap(0, Ordering::Relaxed);
            if t.owned_frames > 0 || !t.handbacks.is_empty() {
                log.write(&format!(
                    "CURSOR TRACE window_s={:.1} owned_frames={} released_frames={:?} handbacks={:?} regains={} replaced_between_frames={} replaced_by={:x?} game_setcursor_calls={calls} game_setcursor_asked_for_its_own_while_ours={while_ours} shown_ours_instead={substituted} last_game_handle={:x} game_setcursor_other_thread={other_thread} wm_setcursor={messages} reapplied_after_wm_setcursor={reapplied} our_normal_handle={:x?} import_counter={}",
                    now.duration_since(started).as_secs_f32(),
                    t.owned_frames,
                    t.released,
                    t.handbacks,
                    t.regains,
                    t.stolen,
                    t.stolen_handles,
                    GAME_HANDLE.load(Ordering::Relaxed),
                    ours,
                    self.import_counter,
                ));
            }
            let (previous, last_set) = (t.previous, t.last_set);
            *t = Trace {
                started: Some(now),
                previous,
                last_set,
                ..Default::default()
            };
        }
        fn restore(&mut self) {
            CURRENT.store(0, Ordering::Release);
            unsafe {
                let current = GetCursor();
                if self.owned
                    && self.saved != 0
                    && self.cache.values().any(|a| a.contains(&current))
                {
                    SetCursor(self.saved);
                }
            }
            self.owned = false;
        }
        pub fn update(&mut self, style: Option<Style>, size: u32, log: &Logger) {
            let Some(style) = style else {
                self.restore();
                return;
            };
            let size = size.clamp(24, 64);
            let window = unsafe { GetForegroundWindow() };
            let mut process = 0;
            let thread = unsafe { GetWindowThreadProcessId(window, &mut process) };
            if window.is_null() || process != std::process::id() {
                self.restore();
                return;
            }
            if thread != crate::platform_input::thread_id() as u32 {
                if !self.thread_warning {
                    log.write(&format!("CURSOR window thread={thread} differs from client thread={}; native cursor retained",crate::platform_input::thread_id()));
                    self.thread_warning = true;
                }
                self.restore();
                return;
            }
            if !self.cache.contains_key(&size) {
                let handles = STYLES.map(|s| unsafe { create(s, size) });
                if handles.contains(&0) {
                    for h in handles {
                        if h != 0 {
                            unsafe {
                                DestroyCursor(h);
                            }
                        }
                    }
                    log.write("CURSOR native artwork creation failed; game cursor retained");
                    self.restore();
                    return;
                }
                self.cache.insert(size, handles);
            }
            if !self.owned {
                self.saved = unsafe { GetCursor() };
                if self.saved == 0 {
                    self.saved =
                        unsafe { LoadCursorW(std::ptr::null_mut(), 32512usize as *const u16) };
                }
                self.owned = true;
            }
            WINDOW.store(window as isize, Ordering::Relaxed);
            let handle = self.cache[&size][style as usize];
            CURRENT.store(handle, Ordering::Release);
            if !self.attempted {
                self.attempted = true;
                WINDOW_THREAD.store(thread, Ordering::Relaxed);
                self.hook =
                    unsafe { SetWindowsHookExW(12, after_message, std::ptr::null_mut(), thread) };
                let (counter, detail) = unsafe { install_import_counter() };
                self.import_counter = counter;
                log.write(&format!("CURSOR native pointer active; owner thread={thread} message_observer={} game_setcursor_counter={counter} ({detail}) size={size}px",self.hook!=0));
            }
            unsafe {
                SetCursor(handle);
            }
            self.trace.last_set = handle;
        }
        pub fn shutdown(&mut self) {
            self.restore();
            if self.import_counter {
                unsafe { remove_import_counter() };
                self.import_counter = false;
            }
            if self.hook != 0 {
                unsafe {
                    UnhookWindowsHookEx(self.hook);
                }
                self.hook = 0;
            }
            self.attempted = false;
            for handles in self.cache.drain().map(|(_, v)| v) {
                for h in handles {
                    unsafe {
                        DestroyCursor(h);
                    }
                }
            }
        }
    }
    impl Drop for Backend {
        fn drop(&mut self) {
            self.shutdown();
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[link(name = "user32")]
        extern "system" {
            fn GetIconInfo(cursor: isize, info: *mut IconInfo) -> i32;
        }
        #[test]
        fn native_cursor_resources_preserve_hotspots_at_small_and_large_sizes() {
            assert_eq!(std::mem::size_of::<IconInfo>(), 32);
            assert_eq!(std::mem::size_of::<ReturnMessage>(), 40);
            assert_eq!(std::mem::size_of::<BitmapInfo>(), 48);
            for size in [24, 32, 64] {
                for style in STYLES {
                    let handle = unsafe { create(style, size) };
                    assert_ne!(handle, 0);
                    let mut info = IconInfo {
                        icon: 1,
                        x: 0,
                        y: 0,
                        mask: 0,
                        color: 0,
                    };
                    assert_ne!(unsafe { GetIconInfo(handle, &mut info) }, 0);
                    assert_eq!(info.icon, 0);
                    assert_eq!((info.x, info.y), style.hotspot(size));
                    unsafe {
                        DeleteObject(info.mask);
                        DeleteObject(info.color);
                        DestroyCursor(handle);
                    }
                }
            }
        }
    }
}
#[cfg(not(windows))]
mod windows {
    #[derive(Default)]
    pub struct Backend;
    impl Backend {
        pub fn update(&mut self, _: Option<super::Style>, _: u32, _: &crate::Logger) {}
        pub fn trace(
            &mut self,
            _: Result<super::Style, &'static str>,
            _: Option<(f32, f32)>,
            _: &crate::Logger,
        ) {
        }
        pub fn shutdown(&mut self) {}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn click_motion_keeps_contraction_curve_but_finishes_by_250_ms() {
        let (scale, alpha) = marker_motion(Duration::ZERO).unwrap();
        assert!((scale - 1.9).abs() < 0.001);
        assert_eq!(alpha, 1.);
        assert_eq!(marker_motion(Duration::from_millis(100)).unwrap().1, 1.);
        let (scale, alpha) = marker_motion(Duration::from_millis(167)).unwrap();
        assert_eq!(scale, 1.);
        assert!(alpha > 0. && alpha < 1.);
        assert_eq!(marker_motion(Duration::from_millis(250)), None);
    }
    #[test]
    fn sizes_hotspots_and_scaled_alpha_keep_the_pointer_click_point() {
        assert_eq!(size_from_ratio(f64::NAN), 32);
        assert_eq!(size_from_ratio(-1.), 24);
        assert_eq!(size_from_ratio(2.), 64);
        for size in 24..=64 {
            assert_eq!(size_from_ratio(ratio_from_size(size)), size);
            for style in STYLES {
                let (x, y) = style.hotspot(size);
                assert!(x < size && y < size);
                let data = pixels(style, size);
                assert_eq!(data.len(), size as usize * size as usize * 4);
                assert!(data
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]));
            }
        }
    }
    #[test]
    fn targeting_modes_choose_distinct_shapes_and_skill_mode_has_priority() {
        assert_eq!(Style::choose(false, false, false, None), Style::Normal);
        assert_eq!(Style::choose(true, true, true, None), Style::ChampionLocked);
        assert_eq!(Style::choose(true, false, false, None), Style::Champion);
        assert_eq!(
            Style::choose(true, true, true, Some(SkillCursor::Invalid)),
            Style::SkillInvalidChampion
        );
    }
    #[test]
    fn saving_size_preserves_other_preferences_and_rejects_malformed_files() {
        let root = std::env::temp_dir().join(format!("lt-cursor-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("controls.json");
        std::fs::write(&file, b"{\"cast_on_release\":true,\"future_setting\":123}").unwrap();
        let settings = Settings::new(Some(&root));
        assert!(!settings.low_health());
        settings.toggle_low_health();
        settings.set(48);
        settings.save().unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(value["cast_on_release"], true);
        assert_eq!(value["future_setting"], 123);
        assert_eq!(value["cursor_size"], 48);
        assert_eq!(Settings::new(Some(&root)).size(), 48);
        assert!(Settings::new(Some(&root)).low_health());
        std::fs::write(&file, b"broken").unwrap();
        assert!(settings.save().is_err());
        assert_eq!(std::fs::read(&file).unwrap(), b"broken");
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
