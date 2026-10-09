//! Spectator key input: keeps game hotkeys from acting while controlling.
use super::*;

pub(crate) static ORIGINAL_INPUT: AtomicUsize = AtomicUsize::new(0);
pub(crate) type NativeInputFn =
    unsafe extern "system" fn(usize, usize, usize, f32, usize, usize, usize);
pub(crate) fn spectator_key_is_owned(tag: u64, key: u8) -> bool {
    matches!(tag, 0x8000000000000006 | 0x8000000000000007) && !matches!(key, 0x21 | 0x3d | 0x3e)
}
pub(crate) unsafe extern "system" fn input_hook(
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
        if !shared.timing.client_session(Some(view)) || !keys.focused {
            return false;
        }
        let tag = std::ptr::read_unaligned(event as *const u64);
        if !matches!(tag, 0x8000000000000006 | 0x8000000000000007) {
            return false;
        }
        let key = std::ptr::read((event + 8) as *const u8);
        if !shared.timing.client_controls(Some(view)) {
            return keys.start || keys.release;
        }
        if crate::ui_state::SETTINGS_OPEN.load(Ordering::Relaxed)
            || spectator_key_is_owned(tag, key)
            || keys.start
            || keys.release
        {
            shared.logger.verbose(|| {
                format!(
                    "NATIVE SPECTATOR_KEY suppressed tag={tag:x} key={key:x} bound_view={view:x}"
                )
            });
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
