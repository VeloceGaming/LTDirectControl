//! Thread-local Windows message observer for wheel zoom. Only a removed wheel
//! message over the owned battlefield is consumed; all other messages continue.
pub fn zoom(current: f32, notches: i32) -> Option<f32> {
    current
        .is_finite()
        .then(|| (current + notches as f32 * 0.25).clamp(0.5, 3.0))
}
#[cfg(windows)]
mod windows {
    use crate::{camera::CameraControl, Logger};
    use std::ffi::c_void;
    use std::sync::{
        atomic::{AtomicBool, AtomicI32, AtomicIsize, Ordering},
        Arc, OnceLock,
    };
    static ACTIVE: AtomicBool = AtomicBool::new(false);
    static DELTA: AtomicI32 = AtomicI32::new(0);
    static HANDLE: AtomicIsize = AtomicIsize::new(0);
    static ATTEMPTED: AtomicBool = AtomicBool::new(false);
    static CAMERA: OnceLock<Arc<CameraControl>> = OnceLock::new();
    static TIMING: OnceLock<Arc<crate::native_timing::NativeTiming>> = OnceLock::new();
    #[repr(C)]
    struct Message {
        window: *mut c_void,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        point: [i32; 2],
        private: u32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn SetWindowsHookExW(
            id: i32,
            callback: unsafe extern "system" fn(i32, usize, isize) -> isize,
            module: *mut c_void,
            thread: u32,
        ) -> isize;
        fn UnhookWindowsHookEx(handle: isize) -> i32;
        fn CallNextHookEx(handle: isize, code: i32, wparam: usize, lparam: isize) -> isize;
        fn GetForegroundWindow() -> *mut c_void;
    }
    unsafe extern "system" fn message_hook(code: i32, removed: usize, message: isize) -> isize {
        if code == 0
            && removed == 1
            && message != 0
            && ACTIVE.load(Ordering::Relaxed)
            && TIMING.get().is_some_and(|t| t.client_controls(None))
        {
            let msg = &mut *(message as *mut Message);
            if msg.message == 0x20a && msg.window == GetForegroundWindow() {
                let keys = crate::platform_input::poll();
                let over_shop = keys.focused
                    && keys.cursor.is_some_and(|p| {
                        crate::shop_ui::AREA
                            .lock()
                            .is_ok_and(|a| a.is_some_and(|r| r.contains(p)))
                    });
                if over_shop && !crate::ui_state::SETTINGS_OPEN.load(Ordering::Relaxed) {
                    let delta = ((msg.wparam >> 16) as u16 as i16) as i32;
                    crate::shop_ui::SCROLL.fetch_add(delta / 120, Ordering::Relaxed);
                    msg.message = 0;
                    msg.wparam = 0;
                    msg.lparam = 0;
                    return CallNextHookEx(0, code, removed, message);
                }
                if keys.focused && crate::ui_state::SETTINGS_OPEN.load(Ordering::Relaxed) {
                    let delta = ((msg.wparam >> 16) as u16 as i16) as i32;
                    crate::settings_ui::SCROLL.fetch_add(delta / 120, Ordering::Relaxed);
                    msg.message = 0;
                    msg.wparam = 0;
                    msg.lparam = 0;
                    return CallNextHookEx(0, code, removed, message);
                }
                let over_tooltip = keys.focused
                    && keys.cursor.is_some_and(|p| {
                        crate::player_hud::TOOLTIP_SCROLL_AREA
                            .lock()
                            .is_ok_and(|area| {
                                area.is_some_and(|(panel, tile)| {
                                    panel.contains(p) || tile.contains(p)
                                })
                            })
                    });
                if over_tooltip {
                    let delta = ((msg.wparam >> 16) as u16 as i16) as i32;
                    crate::player_hud::TOOLTIP_SCROLL.fetch_add(delta / 120, Ordering::Relaxed);
                    msg.message = 0;
                    msg.wparam = 0;
                    msg.lparam = 0;
                    return CallNextHookEx(0, code, removed, message);
                }
                let battlefield = keys.focused
                    && !crate::ui_state::SETTINGS_OPEN.load(std::sync::atomic::Ordering::Relaxed)
                    && CAMERA.get().is_some_and(|camera| {
                        keys.cursor.is_some_and(|p| {
                            !camera.blocked(p)
                                && camera.frame().is_some_and(|f| f.unproject(p).is_some())
                        })
                    });
                if battlefield {
                    let delta = ((msg.wparam >> 16) as u16 as i16) as i32;
                    if keys.camera_options.is_none_or(|o| o.2) {
                        let _ = DELTA.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
                            Some(old.saturating_add(delta).clamp(-1200, 1200))
                        });
                    }
                    // Prevent a second native wheel handler from zooming twice.
                    msg.message = 0;
                    msg.wparam = 0;
                    msg.lparam = 0;
                }
            }
        }
        CallNextHookEx(0, code, removed, message)
    }
    pub fn update(
        active: bool,
        camera: &Arc<CameraControl>,
        timing: &Arc<crate::native_timing::NativeTiming>,
        log: &Logger,
    ) {
        ACTIVE.store(active, Ordering::Relaxed);
        if active && !ATTEMPTED.swap(true, Ordering::Relaxed) {
            let _ = CAMERA.set(camera.clone());
            let _ = TIMING.set(timing.clone());
            let handle = unsafe {
                SetWindowsHookExW(
                    3,
                    message_hook,
                    std::ptr::null_mut(),
                    crate::platform_input::thread_id() as u32,
                )
            };
            HANDLE.store(handle, Ordering::Relaxed);
            log.write(if handle == 0 {
                "CAMERA wheel message hook unavailable; native +/- zoom retained"
            } else {
                "CAMERA wheel message hook installed on owned client thread"
            });
        } else if !active {
            shutdown();
        }
    }
    pub fn shutdown() {
        ACTIVE.store(false, Ordering::Relaxed);
        let handle = HANDLE.swap(0, Ordering::Relaxed);
        if handle != 0 {
            unsafe {
                UnhookWindowsHookEx(handle);
            }
        }
        DELTA.store(0, Ordering::Relaxed);
        ATTEMPTED.store(false, Ordering::Relaxed);
    }
    pub fn take() -> i32 {
        let delta = DELTA.swap(0, Ordering::Relaxed);
        let remainder = delta % 120;
        DELTA.fetch_add(remainder, Ordering::Relaxed);
        delta / 120
    }
}
#[cfg(windows)]
pub use windows::{shutdown, take, update};
#[cfg(not(windows))]
pub fn update(
    _: bool,
    _: &std::sync::Arc<crate::camera::CameraControl>,
    _: &std::sync::Arc<crate::native_timing::NativeTiming>,
    _: &crate::Logger,
) {
}
#[cfg(not(windows))]
pub fn take() -> i32 {
    0
}
#[cfg(not(windows))]
pub fn shutdown() {}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_zoom_steps_limits_and_nonfinite_values() {
        assert_eq!(zoom(1.0, 1), Some(1.25));
        assert_eq!(zoom(1.0, -1), Some(0.75));
        assert_eq!(zoom(0.5, -20), Some(0.5));
        assert_eq!(zoom(3.0, 20), Some(3.0));
        assert_eq!(zoom(f32::NAN, 1), None);
    }
}
