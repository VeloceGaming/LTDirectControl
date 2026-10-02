//! Public Windows keyboard, cursor and focus queries. No input injection.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Keys {
    pub focused: bool,
    pub dx: i8,
    pub dy: i8,
    pub selection: Option<usize>,
    pub start: bool,
    pub release: bool,
    pub right: bool,
    pub left: bool,
    pub team_info: bool,
    pub middle: bool,
    pub stop: bool,
    pub attack_move: bool,
    pub escape: bool,
    pub camera_toggle: bool,
    pub space: bool,
    pub shift: bool,
    pub abilities: [bool; 3],
    pub recall: bool,
    /// Physical toggle buttons: backtick and Mouse 4 (Windows XBUTTON1).
    pub champion_toggle: [bool; 2],
    /// Session-scoped mode resolved by the client before command acquisition.
    pub champion_only: bool,
    /// Client coordinates mapped to the native 1920x1080 UI surface.
    pub cursor: Option<(f32, f32)>,
}

#[derive(Default)]
pub struct ChampionOnlyToggle {
    binding: Option<(crate::native_timing::MatchKey, usize)>,
    focused: bool,
    previous: [bool; 2],
    pub enabled: bool,
}
impl ChampionOnlyToggle {
    pub fn update(
        &mut self,
        keys: Keys,
        active: bool,
        identity: Option<(crate::native_timing::MatchKey, usize)>,
    ) -> bool {
        let binding = identity.filter(|_| active);
        let same_binding = binding.is_some() && binding == self.binding;
        if !same_binding {
            self.enabled = false;
        }
        // Held buttons cannot become presses on takeover or focus return.
        // Both rising edges in one frame count as one toggle.
        if same_binding
            && self.focused
            && keys.focused
            && keys
                .champion_toggle
                .iter()
                .zip(self.previous)
                .any(|(&down, previous)| down && !previous)
        {
            self.enabled = !self.enabled;
        }
        self.binding = binding;
        self.focused = keys.focused;
        self.previous = keys.champion_toggle;
        self.enabled
    }
}

/// Use the Windows thread identifier consistently at SDK and native boundaries.
#[cfg(windows)]
pub fn thread_id() -> u64 {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThreadId() -> u32;
    }
    u64::from(unsafe { GetCurrentThreadId() })
}
#[cfg(not(windows))]
pub fn thread_id() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    std::thread::current().id().hash(&mut hash);
    hash.finish()
}

#[cfg(windows)]
pub fn poll() -> Keys {
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
        fn GetAsyncKeyState(key: i32) -> i16;
        fn GetCursorPos(point: *mut Point) -> i32;
        fn ScreenToClient(window: *mut c_void, point: *mut Point) -> i32;
        fn GetClientRect(window: *mut c_void, rect: *mut Rect) -> i32;
    }
    let mut owner = 0;
    let focused = unsafe {
        let window = GetForegroundWindow();
        if window.is_null() {
            return Keys::default();
        }
        GetWindowThreadProcessId(window, &mut owner);
        owner == std::process::id()
    };
    if !focused {
        return Keys::default();
    }
    let down = |key| unsafe { GetAsyncKeyState(key) < 0 };
    let cursor = unsafe {
        let window = GetForegroundWindow();
        let mut point = Point::default();
        let mut rect = Rect::default();
        if GetCursorPos(&mut point) != 0
            && ScreenToClient(window, &mut point) != 0
            && GetClientRect(window, &mut rect) != 0
            && rect.right > rect.left
            && rect.bottom > rect.top
            && point.x >= rect.left
            && point.x < rect.right
            && point.y >= rect.top
            && point.y < rect.bottom
        {
            Some((
                (point.x - rect.left) as f32 * 1920.0 / (rect.right - rect.left) as f32,
                (point.y - rect.top) as f32 * 1080.0 / (rect.bottom - rect.top) as f32,
            ))
        } else {
            None
        }
    };
    Keys {
        focused,
        dx: i8::from(down(0x4c)) - i8::from(down(0x4a)), // L - J
        dy: i8::from(down(0x4b)) - i8::from(down(0x49)), // K - I
        selection: down(0x11)
            .then(|| (0..5).find(|slot| down(0x31 + *slot as i32) || down(0x61 + *slot as i32)))
            .flatten(),
        start: down(0x11) && down(0x24),
        release: down(0x11) && down(0x23),
        right: down(0x02),
        left: down(0x01),
        team_info: down(0x09),
        middle: down(0x04),
        stop: down(0x53),
        attack_move: down(0x41),
        escape: down(0x1b),
        camera_toggle: down(0x59),
        space: down(0x20),
        shift: down(0x10),
        abilities: [down(0x51), down(0x57), down(0x52)],
        recall: down(0x42),
        champion_toggle: [down(0xc0), down(0x05)], // VK_OEM_3 / VK_XBUTTON1
        champion_only: false,
        cursor,
    }
}

#[cfg(not(windows))]
pub fn poll() -> Keys {
    Keys::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    const IDENTITY: Option<(crate::native_timing::MatchKey, usize)> = Some(((1, 33, 1), 7));
    fn keys(buttons: [bool; 2]) -> Keys {
        Keys {
            focused: true,
            champion_toggle: buttons,
            ..Keys::default()
        }
    }
    #[test]
    fn either_button_toggles_once_and_simultaneous_presses_do_not_double_toggle() {
        let mut mode = ChampionOnlyToggle::default();
        assert!(!mode.update(keys([false; 2]), true, IDENTITY));
        assert!(mode.update(keys([true, false]), true, IDENTITY));
        assert!(mode.update(keys([true, false]), true, IDENTITY));
        // Mouse 4 can turn it off even while backtick remains held.
        assert!(!mode.update(keys([true, true]), true, IDENTITY));
        assert!(!mode.update(keys([false; 2]), true, IDENTITY));
        assert!(mode.update(keys([true; 2]), true, IDENTITY));
        assert!(mode.update(keys([false; 2]), true, IDENTITY));
        assert!(!mode.update(keys([false, true]), true, IDENTITY));
    }
    #[test]
    fn takeover_focus_return_release_and_identity_changes_cannot_replay_held_toggles() {
        let mut mode = ChampionOnlyToggle::default();
        assert!(!mode.update(keys([true; 2]), true, IDENTITY));
        assert!(!mode.update(keys([true; 2]), true, IDENTITY));
        mode.update(keys([false; 2]), true, IDENTITY);
        assert!(mode.update(keys([false, true]), true, IDENTITY));
        assert!(mode.update(Keys::default(), true, IDENTITY)); // Focus loss retains mode.
        assert!(mode.update(keys([true, false]), true, IDENTITY)); // Return while held.
        mode.update(keys([false; 2]), true, IDENTITY);
        assert!(!mode.update(keys([true, false]), true, IDENTITY));
        mode.update(keys([false; 2]), true, IDENTITY);
        assert!(mode.update(keys([false, true]), true, IDENTITY));
        assert!(!mode.update(keys([false, true]), false, IDENTITY)); // Release clears it.
        assert!(!mode.update(keys([false, true]), true, IDENTITY));
        mode.update(keys([false; 2]), true, IDENTITY);
        assert!(mode.update(keys([true, false]), true, IDENTITY));
        assert!(!mode.update(keys([true, false]), true, Some(((1, 34, 1), 7))));
        mode.update(keys([false; 2]), true, IDENTITY);
        assert!(mode.update(keys([true, false]), true, IDENTITY));
        assert!(!mode.update(keys([true, false]), true, Some(((1, 33, 1), 8))));
    }
}
