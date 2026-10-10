//! Keeps the pointer inside the game window while the player controls a
//! running match, so edge scrolling works next to a second monitor. The
//! clip is the window's client area; it is renewed every frame (the window
//! may move, and Windows drops a clip when focus changes) and released as
//! soon as it is not wanted.
use crate::Logger;
use std::sync::atomic::{AtomicBool, Ordering};

static APPLIED: AtomicBool = AtomicBool::new(false);

/// Only while the player's match runs and no window of the mod is open:
/// paused, in the shop or settings, handed to the AI or outside a match the
/// pointer is free.
pub fn wanted(setting: bool, running: bool, shop_open: bool, settings_open: bool) -> bool {
    setting && running && !shop_open && !settings_open
}

/// Client thread, every frame.
pub fn update(want: bool, log: &Logger) {
    let held = want && platform::clip();
    if APPLIED.swap(held, Ordering::Relaxed) != held {
        if !held {
            platform::release();
        }
        log.write(&format!("POINTER clip {}", if held { "on" } else { "off" }));
    }
}

/// The extension is leaving: never keep the pointer confined.
pub fn shutdown() {
    if APPLIED.swap(false, Ordering::Relaxed) {
        platform::release();
    }
}

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;
    #[repr(C)]
    #[derive(Default)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> *mut c_void;
        fn GetWindowThreadProcessId(window: *mut c_void, process_id: *mut u32) -> u32;
        fn GetClientRect(window: *mut c_void, rect: *mut Rect) -> i32;
        fn ClientToScreen(window: *mut c_void, point: *mut Point) -> i32;
        fn ClipCursor(rect: *const Rect) -> i32;
    }
    /// Confines the pointer to the client area of the game window when it
    /// is the foreground window. Returns whether the clip is in place.
    pub fn clip() -> bool {
        unsafe {
            let window = GetForegroundWindow();
            let mut owner = 0;
            if window.is_null()
                || GetWindowThreadProcessId(window, &mut owner) == 0
                || owner != std::process::id()
            {
                return false;
            }
            let mut rect = Rect::default();
            let mut origin = Point::default();
            if GetClientRect(window, &mut rect) == 0
                || ClientToScreen(window, &mut origin) == 0
                || rect.right <= rect.left
                || rect.bottom <= rect.top
            {
                return false;
            }
            let area = Rect {
                left: origin.x,
                top: origin.y,
                right: origin.x + (rect.right - rect.left),
                bottom: origin.y + (rect.bottom - rect.top),
            };
            ClipCursor(&area) != 0
        }
    }
    pub fn release() {
        unsafe {
            ClipCursor(std::ptr::null());
        }
    }
}
#[cfg(not(windows))]
mod platform {
    pub fn clip() -> bool {
        false
    }
    pub fn release() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pointer_is_confined_only_while_the_players_match_runs_with_no_window_open() {
        assert!(wanted(true, true, false, false));
        // Setting off, paused / AI / outside a match, shop, settings.
        assert!(!wanted(false, true, false, false));
        assert!(!wanted(true, false, false, false));
        assert!(!wanted(true, true, true, false));
        assert!(!wanted(true, true, false, true));
    }
}
