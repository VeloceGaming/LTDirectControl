//! The mod's short sounds: interface hover and click in its windows, the
//! denied cast sound and the skill ready chime. All play on the client
//! thread through the SDK's `play_sound`, once at the end of each frame
//! (`Sounds::flush`). Windows and the HUD ask with `click()`, `hover()`,
//! `denied()` and `ready()`; worker threads only call `cast_denied()`,
//! which bumps an atomic counter and never locks or calls the game.
use crate::Logger;
use std::{
    sync::atomic::{AtomicU64, AtomicU8, Ordering},
    time::{Duration, Instant},
};

/// The game's own interface clips (in the game's bundle on 0.6.3, 0.6.4
/// and 0.7.0, where its widgets name them as `hover_sound`/`click_sound`).
pub const HOVER: &str = "asset/base/sound/sfx/UI_mouse_hover";
pub const CLICK: &str = "asset/base/sound/sfx/UI_mouse_click";
/// Kenney's "confirmation_001" (CC0), `probe/sound/sfx/skill_ready.wav`.
pub const READY: &str = "asset/lt_direct_control/sound/sfx/skill_ready";
/// Kenney's "error_008" (CC0), `probe/sound/sfx/cast_denied.wav`, chosen by
/// ear by the author (the game's click did not read as "cast failed").
const DENIED: &str = "asset/lt_direct_control/sound/sfx/cast_denied";
const DENIED_VOLUME: f32 = 0.4;
/// The chime file is used as shipped; its level is set here (0.6 since
/// 1.3.7: the author found 0.4 "a little quiet").
const READY_VOLUME: f32 = 0.6;
/// Mashing a key gives a steady tick, not a buzz.
const DENIED_GAP: Duration = Duration::from_millis(150);
/// A click and the pause or resume it causes a frame later sound once.
const CLICK_GAP: Duration = Duration::from_millis(100);
/// Elements under the cursor while a window appears make no hover sound.
const OPEN_QUIET: Duration = Duration::from_millis(100);

const R_HOVER: u8 = 1;
const R_CLICK: u8 = 2;
const R_DENIED: u8 = 4;
const R_READY: u8 = 8;
static REQUESTS: AtomicU8 = AtomicU8::new(0);
static CAST_DENIED: AtomicU64 = AtomicU64::new(0);

/// Worker side: a cast press the mod or the game turned away (not an
/// unlearned skill). Lock-free; the client frame decides whether it sounds.
pub fn cast_denied() {
    CAST_DENIED.fetch_add(1, Ordering::Relaxed);
}
pub fn hover() {
    REQUESTS.fetch_or(R_HOVER, Ordering::Relaxed);
    #[cfg(test)]
    HOVER_CALLS.with(|n| n.set(n.get() + 1));
}
pub fn click() {
    REQUESTS.fetch_or(R_CLICK, Ordering::Relaxed);
    #[cfg(test)]
    CLICK_CALLS.with(|n| n.set(n.get() + 1));
}
/// A refused action in a window (a purchase the shop turned away).
pub fn denied() {
    REQUESTS.fetch_or(R_DENIED, Ordering::Relaxed);
}
pub fn ready() {
    REQUESTS.fetch_or(R_READY, Ordering::Relaxed);
    #[cfg(test)]
    READY_CALLS.with(|n| n.set(n.get() + 1));
}
#[cfg(test)]
thread_local! {
    /// Chime requests made on this thread (window identity tests).
    pub static READY_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Hover and click sound requests made on this thread (window tests).
    pub static HOVER_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub static CLICK_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[derive(Clone, Copy)]
pub struct Config {
    /// "Mod sound volume", 0 to 1 (saved as `emote_volume`).
    pub volume: f32,
    pub ready: bool,
    pub denied: bool,
    pub interface: bool,
}
impl Config {
    pub fn from_values(values: &crate::settings::Values) -> Self {
        Self {
            volume: values.number("emote_volume") as f32 / 100.,
            ready: values.number("skill_ready_sound") == 1.,
            denied: values.number("denied_cast_sound") == 1.,
            interface: values.number("interface_sounds") == 1.,
        }
    }
}

/// What one frame plays: (clip, volume relative to the slider).
#[derive(Debug, PartialEq)]
enum Play {
    Hover,
    Click,
    Denied,
    Ready,
}

#[derive(Default)]
pub struct Sounds {
    seen_cast_denied: u64,
    last_denied: Option<Instant>,
    last_click: Option<Instant>,
    /// Clips the game refused, each logged once: whether it takes the
    /// base game's paths is to be seen in game.
    refused: Vec<&'static str>,
}
impl Sounds {
    /// End of the client frame: play what this frame asked for.
    /// `alive`: the controlled athlete is alive (no denied cast sound while dead).
    pub fn flush(
        &mut self,
        ctx: &mut mod_api_stable::StableClient<'_>,
        config: Config,
        alive: bool,
        log: &Logger,
    ) {
        let requests = REQUESTS.swap(0, Ordering::Relaxed);
        let count = CAST_DENIED.load(Ordering::Relaxed);
        let cast = std::mem::replace(&mut self.seen_cast_denied, count) != count && alive;
        for play in self.choose(requests, cast, config, Instant::now()) {
            let (path, volume) = match play {
                Play::Hover => (HOVER, 1.),
                Play::Click => (CLICK, 1.),
                Play::Denied => (DENIED, DENIED_VOLUME),
                Play::Ready => (READY, READY_VOLUME),
            };
            // The game accepts 0 to 1, under its own sound effects volume.
            let played = ctx.play_sound(path, (config.volume * volume).clamp(0., 1.));
            if !played && !self.refused.contains(&path) {
                self.refused.push(path);
                log.write(&format!(
                    "SOUND play request refused path={path}; logged once"
                ));
            }
        }
    }
    fn choose(&mut self, requests: u8, cast: bool, config: Config, now: Instant) -> Vec<Play> {
        let mut plays = Vec::new();
        if config.volume <= 0. {
            return plays;
        }
        let since = |at: Option<Instant>, gap| at.is_none_or(|t| now.duration_since(t) >= gap);
        let ui_denied = requests & R_DENIED != 0 && config.interface;
        if (ui_denied || cast && config.denied) && since(self.last_denied, DENIED_GAP) {
            self.last_denied = Some(now);
            plays.push(Play::Denied);
        }
        // A refused purchase replaces the click of the button that asked.
        if requests & R_CLICK != 0 && config.interface && !ui_denied {
            if since(self.last_click, CLICK_GAP) {
                plays.push(Play::Click);
            }
            self.last_click = Some(now);
        } else if requests & R_HOVER != 0 && config.interface && plays.is_empty() {
            plays.push(Play::Hover);
        }
        if requests & R_READY != 0 && config.ready {
            plays.push(Play::Ready);
        }
        plays
    }
}

/// Hover sound for one window: once when the pointer enters a clickable
/// element, never for the element under a still pointer (a window opening,
/// a page switching, a list scrolling), and not while the left button is
/// held (dragging a slider).
#[derive(Default)]
pub struct Hover {
    opened: Option<Instant>,
    last: Option<String>,
    cursor: Option<(f32, f32)>,
}
impl Hover {
    /// `under`: the clickable element under the cursor this frame.
    /// `quiet`: the left button is held, or another window covers this one.
    pub fn update(
        &mut self,
        shown: bool,
        under: Option<String>,
        cursor: Option<(f32, f32)>,
        quiet: bool,
    ) {
        if self.entered(shown, under, cursor, quiet, Instant::now()) {
            hover();
        }
    }
    fn entered(
        &mut self,
        shown: bool,
        under: Option<String>,
        cursor: Option<(f32, f32)>,
        quiet: bool,
        now: Instant,
    ) -> bool {
        if !shown {
            *self = Self::default();
            return false;
        }
        let opened = *self.opened.get_or_insert(now);
        let moved = std::mem::replace(&mut self.cursor, cursor) != cursor;
        if under == self.last {
            return false;
        }
        let sound = under.is_some() && moved && !quiet && now.duration_since(opened) >= OPEN_QUIET;
        self.last = under;
        sound
    }
}

/// The first of `paths` (searched from the last, which is drawn on top)
/// under the cursor and shown, with every ancestor shown: hidden rows,
/// tiles and menu options keep their rectangles.
pub fn under_cursor<'a>(
    ctx: &mod_api_stable::StableClient<'_>,
    paths: impl DoubleEndedIterator<Item = &'a String>,
    cursor: Option<(f32, f32)>,
) -> Option<String> {
    let (x, y) = cursor?;
    paths
        .rev()
        .find(|path| {
            ctx.ui_node_rect(path).is_some_and(|(left, top, w, h)| {
                x >= left && x < left + w && y >= top && y < top + h
            }) && ctx.ui_visible(path) == Some(true)
                && path
                    .match_indices('.')
                    .all(|(end, _)| ctx.ui_visible(&path[..end]) != Some(false))
        })
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> Config {
        Config {
            volume: 0.7,
            ready: false,
            denied: true,
            interface: true,
        }
    }
    #[test]
    fn denied_sounds_at_most_every_150_ms_and_respects_its_setting() {
        let mut s = Sounds::default();
        let t = Instant::now();
        let ms = |n| t + Duration::from_millis(n);
        assert_eq!(s.choose(0, true, config(), t), vec![Play::Denied]);
        assert!(s.choose(0, true, config(), ms(100)).is_empty());
        assert_eq!(s.choose(0, true, config(), ms(150)), vec![Play::Denied]);
        let off = Config {
            denied: false,
            ..config()
        };
        assert!(s.choose(0, true, off, ms(400)).is_empty());
        // A refused purchase follows "Interface sounds", not "Denied cast sound".
        assert_eq!(s.choose(R_DENIED, false, off, ms(600)), vec![Play::Denied]);
    }
    #[test]
    fn a_refused_purchase_replaces_the_click_and_clicks_close_together_sound_once() {
        let mut s = Sounds::default();
        let t = Instant::now();
        let ms = |n| t + Duration::from_millis(n);
        assert_eq!(
            s.choose(R_CLICK | R_DENIED | R_HOVER, false, config(), t),
            vec![Play::Denied]
        );
        assert_eq!(
            s.choose(R_CLICK | R_HOVER, false, config(), ms(200)),
            vec![Play::Click]
        );
        assert!(s.choose(R_CLICK, false, config(), ms(250)).is_empty());
        assert_eq!(
            s.choose(R_HOVER, false, config(), ms(260)),
            vec![Play::Hover]
        );
    }
    #[test]
    fn interface_sounds_off_or_volume_zero_is_silent_and_the_chime_is_opt_in() {
        let mut s = Sounds::default();
        let t = Instant::now();
        let quiet = Config {
            interface: false,
            ..config()
        };
        assert!(s
            .choose(R_CLICK | R_HOVER | R_READY, false, quiet, t)
            .is_empty());
        let chime = Config {
            ready: true,
            ..quiet
        };
        assert_eq!(s.choose(R_READY, false, chime, t), vec![Play::Ready]);
        let muted = Config {
            volume: 0.,
            ..chime
        };
        assert!(s.choose(R_READY | R_CLICK, true, muted, t).is_empty());
    }
    #[test]
    fn hover_sounds_on_entering_but_not_at_open_on_a_still_pointer_or_while_dragging() {
        let mut h = Hover::default();
        let t = Instant::now();
        let ms = |n| t + Duration::from_millis(n);
        let a = || Some("a".to_owned());
        let b = || Some("b".to_owned());
        // Opening under the pointer, even while it moves.
        assert!(!h.entered(true, a(), Some((1., 1.)), false, t));
        assert!(!h.entered(true, b(), Some((2., 1.)), false, ms(50)));
        // Moving onto another element; staying on it.
        assert!(h.entered(true, a(), Some((3., 1.)), false, ms(200)));
        assert!(!h.entered(true, a(), Some((4., 1.)), false, ms(216)));
        // Leaving to empty space, then back.
        assert!(!h.entered(true, None, Some((9., 9.)), false, ms(232)));
        assert!(h.entered(true, a(), Some((3., 1.)), false, ms(248)));
        // A page switch under a still pointer.
        assert!(!h.entered(true, b(), Some((3., 1.)), false, ms(264)));
        // Dragging a slider across other elements.
        assert!(!h.entered(true, a(), Some((5., 1.)), true, ms(280)));
        // Closing and reopening starts over.
        assert!(!h.entered(false, None, None, false, ms(300)));
        assert!(!h.entered(true, b(), Some((6., 1.)), false, ms(316)));
    }
}
