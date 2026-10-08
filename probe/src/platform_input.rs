//! Public Windows keyboard, cursor and focus queries. No input injection.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Raw(pub [bool; 256]);
impl Default for Raw {
    fn default() -> Self {
        Self([false; 256])
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Keys {
    pub focused: bool,
    pub raw: Raw,
    pub attack_click: bool,
    pub previews: [bool; 3],
    pub self_casts: [bool; 3],
    pub cast_modes: [u8; 3],
    pub mapped: bool,
    pub champion_hold: Option<bool>,
    pub camera_options: Option<(bool, bool, bool, f32)>,
    pub camera_lock_default: bool,
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
    pub alt: bool,
    pub abilities: [bool; 3],
    pub recall: bool,
    pub shop: bool,
    /// Current primary/secondary champion-only binding states.
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
        if keys.champion_hold == Some(true) {
            self.enabled = active && keys.focused && keys.champion_toggle.into_iter().any(|b| b);
            self.binding = binding;
            self.focused = keys.focused;
            self.previous = keys.champion_toggle;
            return self.enabled;
        }
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
    let mut raw = Raw::default();
    if crate::ui_state::SETTINGS_OPEN.load(std::sync::atomic::Ordering::Relaxed) {
        raw.0 = std::array::from_fn(|i| down(i as i32));
    } else {
        let v = crate::settings::current();
        let mut needed = [false; 256];
        for d in crate::settings::BINDINGS {
            for c in v.binding(d.key).into_iter().flatten() {
                needed[c.code as usize] = true;
            }
        }
        for i in [1, 0x10, 0x11, 0x12, 0x1b] {
            needed[i] = true;
        }
        for (i, query) in needed.into_iter().enumerate() {
            if query {
                raw.0[i] = down(i as i32);
            }
        }
    }

    mapped(raw, focused, cursor)
}

pub fn mapped(raw: Raw, focused: bool, cursor: Option<(f32, f32)>) -> Keys {
    let v = crate::settings::current();
    let pressed = |key| v.pressed(key, &raw.0);
    let previews = [
        pressed("preview_q"),
        pressed("preview_w"),
        pressed("preview_r"),
    ];
    let self_casts = [pressed("self_q"), pressed("self_w"), pressed("self_r")];
    let mut keys = Keys {
        focused,
        raw,
        cursor,
        mapped: true,
        dx: i8::from(pressed("pan_right")) - i8::from(pressed("pan_left")),
        dy: i8::from(pressed("pan_down")) - i8::from(pressed("pan_up")),
        selection: [
            "select_top",
            "select_jungle",
            "select_mid",
            "select_bottom",
            "select_support",
        ]
        .iter()
        .position(|k| pressed(k)),
        start: pressed("start"),
        release: pressed("release"),
        right: pressed("move"),
        left: raw.0[1],
        team_info: pressed("tab"),
        middle: pressed("drag_camera"),
        stop: pressed("stop"),
        attack_move: pressed("attack_aim"),
        attack_click: pressed("attack_click"),
        // While the shop is open, Esc closes it and must not cancel a recall.
        escape: raw.0[0x1b]
            && !crate::ui_state::SHOP_OPEN.load(std::sync::atomic::Ordering::Relaxed),
        camera_toggle: pressed("camera_toggle"),
        space: pressed("follow"),
        shift: raw.0[0x10],
        alt: raw.0[0x12],
        abilities: std::array::from_fn(|i| {
            pressed(["q", "w", "r"][i]) || previews[i] || self_casts[i]
        }),
        previews,
        self_casts,
        cast_modes: std::array::from_fn(|i| v.number(["cast_q", "cast_w", "cast_r"][i]) as u8),
        recall: pressed("recall"),
        shop: pressed("shop"),
        champion_toggle: std::array::from_fn(|i| {
            v.binding("champion_only")[i].is_some_and(|c| {
                raw.0[c.code as usize] && crate::settings::modifiers(&raw.0) & c.mods == c.mods
            })
        }),
        champion_hold: Some(v.number("champion_mode") == 0.),
        camera_options: Some((
            v.number("edge_pan") == 1.,
            v.number("drag") == 1.,
            v.number("zoom") == 1.,
            v.number("pan_speed") as f32,
        )),
        camera_lock_default: v.number("camera_lock") == 1.,
        ..Keys::default()
    };
    if crate::ui_state::SETTINGS_OPEN.load(std::sync::atomic::Ordering::Relaxed) {
        keys = Keys {
            focused,
            raw,
            cursor,
            left: raw.0[1],
            escape: raw.0[0x1b],
            mapped: true,
            champion_hold: Some(v.number("champion_mode") == 0.),
            ..Keys::default()
        };
    }
    keys
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
    fn hold_mode_releases_without_a_second_press_and_drops_on_focus_loss() {
        let mut mode = ChampionOnlyToggle::default();
        let hold = Keys {
            champion_hold: Some(true),
            ..keys([true, false])
        };
        assert!(mode.update(hold, true, IDENTITY));
        assert!(!mode.update(
            Keys {
                champion_hold: Some(true),
                ..keys([false, false])
            },
            true,
            IDENTITY
        ));
        assert!(mode.update(hold, true, IDENTITY));
        assert!(!mode.update(
            Keys {
                focused: false,
                ..hold
            },
            true,
            IDENTITY
        ));
        assert!(!mode.update(hold, false, IDENTITY));
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
