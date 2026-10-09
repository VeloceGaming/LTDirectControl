//! Local cosmetic state and one reused native overlay. No simulation input,
//! retained entity pointers, filesystem work or hooks are needed for rendering.
use crate::{
    camera::{CameraControl, Rect},
    native_timing::MatchKey,
    platform_input::Keys,
    Logger,
};
use mod_api_stable::StableClient;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

pub const PATH: &str = "ingame.lt_emotes";
const DISPLAY_TICKS: usize = 120;
const HALF: f32 = 152.;
const OFFSETS: [(i32, i32); 5] = [(116, 116), (116, 12), (220, 116), (116, 220), (12, 116)];
// Keep the catalogue data separate from wheel layout and input policy. More
// packs/slot assignment can reuse this representation without new native UI.
pub struct Emote {
    pub name: &'static str,
    pub glyph: &'static str,
}
pub const CATALOGUE: [Emote; 5] = [
    Emote {
        name: "GG",
        glyph: "emote_gg",
    },
    Emote {
        name: "Nice",
        glyph: "emote_nice",
    },
    Emote {
        name: "Hype",
        glyph: "emote_hype",
    },
    Emote {
        name: "Oops",
        glyph: "emote_oops",
    },
    Emote {
        name: "Focus",
        glyph: "emote_focus",
    },
];
/// Applied values and Settings draft use the same geometry and timing policy.
#[derive(Clone, Copy)]
pub struct Config {
    pub height: f32,
    pub scale: f32,
    pub follow_zoom: bool,
    pub cooldown_ticks: usize,
    pub sound: bool,
}
impl Config {
    pub fn from_values(values: &crate::settings::Values) -> Self {
        Self {
            height: values.number("emote_height") as f32,
            scale: values.number("emote_scale") as f32 / 100.,
            follow_zoom: values.number("emote_zoom") == 1.,
            cooldown_ticks: (values.number("emote_cooldown") * 60.).round() as usize,
            sound: values.number("emote_sound") == 1.,
        }
    }
    pub fn geometry(self, anchor: (f32, f32), age: f32, zoom: f32) -> (Rect, u8) {
        let zoom = if self.follow_zoom && zoom.is_finite() {
            zoom.clamp(0.5, 3.)
        } else {
            1.
        };
        let (size, lift, alpha) = animation(age);
        let size = size * self.scale * zoom;
        (
            Rect {
                x: anchor.0 - size / 2.,
                y: anchor.1 - (self.height + lift) * zoom - size,
                w: size,
                h: size,
            },
            alpha,
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub generation: u64,
    pub key: MatchKey,
    pub player: usize,
}
#[derive(Clone, Copy)]
pub struct Context {
    pub identity: Option<Identity>,
    pub tick: usize,
    pub controlling: bool,
    pub running: bool,
    pub alive: bool,
    pub enabled: bool,
    pub blocked: bool,
    pub aiming: bool,
    pub may_open: bool,
    pub cooldown_ticks: usize,
}
#[derive(Clone, Copy)]
struct Wheel {
    center: (f32, f32),
    origin: (f32, f32),
    slot: usize,
}
#[derive(Clone, Copy)]
struct Active {
    slot: usize,
    at: usize,
}
#[derive(Default)]
struct State {
    identity: Option<Identity>,
    eligible: bool,
    previous_key: bool,
    previous_cancel: bool,
    wheel: Option<Wheel>,
    active: Option<Active>,
    cooldown: Option<usize>,
    drain: bool,
    tick: usize,
    cooldown_ticks: usize,
}
fn finite_point(p: (f32, f32)) -> bool {
    p.0.is_finite() && p.1.is_finite()
}
fn wheel_center(p: (f32, f32)) -> (f32, f32) {
    (
        p.0.clamp(HALF + 8., 1920. - HALF - 8.),
        p.1.clamp(HALF + 8., 1080. - HALF - 56.),
    )
}
fn pick(center: (f32, f32), p: (f32, f32)) -> usize {
    let (x, y) = (p.0 - center.0, p.1 - center.1);
    if x * x + y * y <= 38. * 38. {
        0
    } else if x.abs() > y.abs() {
        if x > 0. {
            2
        } else {
            4
        }
    } else if y > 0. {
        3
    } else {
        1
    }
}
fn held_command(k: Keys) -> bool {
    k.escape
        || k.right
        || k.left
        || k.attack_click
        || k.attack_move
        || k.stop
        || k.recall
        || k.abilities.into_iter().any(|b| b)
}
impl State {
    /// Capture physical edges even while blocked. A held key on focus return,
    /// takeover, respawn or modal close never becomes a fresh emote press.
    fn input(&mut self, k: Keys, c: Context) -> (bool, Option<usize>) {
        if self.identity != c.identity || c.tick < self.tick {
            *self = Self {
                identity: c.identity,
                previous_key: k.emote,
                previous_cancel: k.right || k.attack_click,
                tick: c.tick,
                ..Self::default()
            };
        }
        self.tick = c.tick;
        self.cooldown_ticks = c.cooldown_ticks.clamp(15, 360);
        let eligible = c.identity.is_some() && c.controlling && c.alive && c.enabled && k.focused;
        let available = eligible && c.running && !c.blocked && !c.aiming;
        let edge = k.emote && !self.previous_key && self.eligible;
        let cancel = k.escape || ((k.right || k.attack_click) && !self.previous_cancel);
        self.previous_key = k.emote;
        self.previous_cancel = k.right || k.attack_click;
        self.eligible = available;
        if !eligible {
            self.active = None;
        }
        if self
            .active
            .is_some_and(|a| c.tick.saturating_sub(a.at) >= DISPLAY_TICKS)
        {
            self.active = None;
        }
        let was_open = self.wheel.is_some();
        let mut fired = None;
        if !available || cancel || k.shop || k.start || k.release {
            self.wheel = None;
        } else if edge && c.may_open {
            if let Some(p) = k.cursor.filter(|p| finite_point(*p)) {
                self.wheel = Some(Wheel {
                    center: wheel_center(p),
                    origin: p,
                    slot: 0,
                });
            }
        }
        if let Some(wheel) = self.wheel.as_mut() {
            if let Some(p) = k.cursor.filter(|p| finite_point(*p)) {
                wheel.slot = pick(wheel.origin, p);
            } else {
                self.wheel = None;
            }
        }
        if !k.emote && self.wheel.is_some() {
            let slot = self.wheel.take().unwrap().slot;
            if self
                .cooldown
                .is_none_or(|at| c.tick.saturating_sub(at) >= self.cooldown_ticks)
            {
                self.active = Some(Active { slot, at: c.tick });
                self.cooldown = Some(c.tick);
                fired = Some(slot);
            }
        }
        // Closing/cancel clicks and skills pressed while choosing must be
        // drained, not replayed as orders on the next client frame.
        if was_open || self.wheel.is_some() {
            self.drain = true;
        }
        let capture = available && (self.drain || self.wheel.is_some());
        if self.wheel.is_none() && !k.emote && !held_command(k) {
            self.drain = false;
        }
        if !eligible {
            self.drain = false;
        }
        (capture, fired)
    }
    fn remaining(&self) -> usize {
        self.cooldown.map_or(0, |at| {
            self.cooldown_ticks
                .saturating_sub(self.tick.saturating_sub(at))
        })
    }
}
fn label(id: &str, y: i32, text: &str) -> String {
    format!("#{id}:label {{ x: 0px; y: {y}px; width: 304px; height: 25px; size: 16; font: \"asset/lt_direct_control_probe/font/medium\"; align_x: Center; align_y: Center; color: #eeececff; text: {}; ignore_event: true; z: 2307; }}\n", serde_json::to_string(text).unwrap())
}
fn template() -> String {
    let mut s = String::from("lt_emotes:empty { x: 0px; y: 0px; width: 1920px; height: 1080px; visible: false; ignore_event: true; z: 2300;\n#capture:color { x: 0px; y: 0px; width: 1920px; height: 1080px; color: #00000000; visible: false; ignore_event: false; z: 2300; }\n#wheel:empty { width: 304px; height: 350px; visible: false; ignore_event: true; z: 2301;\n#vertical:color { x: 151px; y: 80px; width: 2px; height: 144px; color: #eeecec44; ignore_event: true; z: 2302; }\n#horizontal:color { x: 80px; y: 151px; width: 144px; height: 2px; color: #eeecec44; ignore_event: true; z: 2302; }\n");
    for (i, (x, y)) in OFFSETS.into_iter().enumerate() {
        s.push_str(&format!("#slot{i}:color {{ x: {x}px; y: {y}px; width: 72px; height: 72px; color: #~4b4a49ff; ignore_event: true; z: 2303;\n#shadow:color {{ x: -2px; y: 4px; width: 76px; height: 74px; color: #00000066; ignore_event: true; z: 2302; }}\n#fill:color {{ x: 2px; y: 2px; width: 68px; height: 68px; color: #~1c1a18ee; ignore_event: true; z: 2304; }}\n"));
        s.push_str(&crate::ui_graphics::image(
            "face",
            CATALOGUE[i].glyph,
            4,
            4,
            64,
            2305,
        ));
        s.push_str("}\n");
    }
    s.push_str(&label("name", 304, "GG"));
    s.push_str(&label(
        "hint",
        329,
        "Release to show · Esc / right-click cancel",
    ));
    s.push_str("}\n#floating:image { width: 88px; height: 88px; visible: false; ignore_event: true; z: 2290; }\n}\n");
    s
}
#[derive(Default)]
pub struct Emotes {
    state: State,
    pending_sound: bool,
    cache: HashMap<String, String>,
    spawn_attempt: Option<Instant>,
    shown: bool,
    ready: bool,
    active_art: Option<crate::emote_library::Art>,
}
impl Emotes {
    pub fn input(&mut self, keys: &mut Keys, context: Context) {
        if self.state.identity != context.identity {
            self.ready = false;
            self.spawn_attempt = None;
            self.pending_sound = false;
        }
        let (capture, fired) = self.state.input(*keys, context);
        if fired.is_some() {
            self.pending_sound = true;
        }
        if let Some(slot) = fired {
            self.active_art = Some(
                crate::emote_library::snapshot()
                    .playable(&crate::settings::current_shared(), slot)
                    .clone(),
            );
        }
        if self.state.active.is_none() {
            self.active_art = None;
        }
        keys.emote_capture = capture;
        crate::ui_state::EMOTE_CAPTURE.store(capture, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn wheel_open(&self) -> bool {
        self.state.wheel.is_some()
    }
    pub fn input_pending(&self) -> bool {
        self.state.wheel.is_some() || self.state.previous_key || self.state.drain
    }
    fn props(&mut self, ctx: &mut StableClient<'_>, path: &str, value: &str) {
        crate::hud_motion::properties(ctx, &mut self.cache, path, value);
    }
    fn text(&mut self, ctx: &mut StableClient<'_>, path: &str, value: &str) {
        let key = format!("text:{path}");
        if self.cache.get(&key).map(String::as_str) != Some(value) && ctx.ui_set_text(path, value) {
            self.cache.insert(key, value.into());
        }
    }
    pub fn apply(
        &mut self,
        ctx: &mut StableClient<'_>,
        camera: &CameraControl,
        position: Option<(u64, u64)>,
        show: bool,
        config: Config,
        log: &Logger,
    ) {
        let wanted = show && (self.state.wheel.is_some() || self.state.active.is_some());
        if !wanted && !self.shown && (self.ready || !show) {
            self.pending_sound = false;
            return;
        }
        if !ctx.ui_exists(PATH) {
            self.shown = false;
            if !show
                || self
                    .spawn_attempt
                    .is_some_and(|t| t.elapsed() < Duration::from_secs(2))
            {
                return;
            }
            self.spawn_attempt = Some(Instant::now());
            self.cache.clear();
            if !ctx.ui_spawn_source("ingame", &crate::ui_theme::themed(&template()))
                || !ctx.ui_exists(PATH)
            {
                log.write("EMOTE UI spawn failed; cosmetic overlay will retry, gameplay continues");
                return;
            }
        }
        self.ready = true;
        self.props(ctx, PATH, &format!("visible: {wanted};"));
        self.shown = wanted;
        if !wanted {
            self.pending_sound = false;
            return;
        }
        let wheel_path = format!("{PATH}.wheel");
        self.props(
            ctx,
            &format!("{PATH}.capture"),
            &format!("visible: {};", self.wheel_open()),
        );
        self.props(
            ctx,
            &wheel_path,
            &format!("visible: {};", self.wheel_open()),
        );
        if let Some(w) = self.state.wheel {
            let library = crate::emote_library::snapshot();
            let values = crate::settings::current_shared();
            for i in 0..5 {
                self.props(
                    ctx,
                    &format!("{wheel_path}.slot{i}.face"),
                    &format!(
                        "source: {};",
                        serde_json::to_string(&library.playable(&values, i).source).unwrap()
                    ),
                );
            }
            self.props(
                ctx,
                &wheel_path,
                &format!(
                    "x: {:.0}px; y: {:.0}px;",
                    w.center.0 - HALF,
                    w.center.1 - HALF
                ),
            );
            for i in 0..CATALOGUE.len() {
                self.props(
                    ctx,
                    &format!("{wheel_path}.slot{i}"),
                    if i == w.slot {
                        "color: #fdee00ff;"
                    } else {
                        "color: #~4b4a49ff;"
                    },
                );
            }
            let remaining = self.state.remaining();
            let mut name = library.playable(&values, w.slot).label.clone();
            let width = if remaining == 0 { 290. } else { 175. };
            if crate::hud_style::width(&name, 16.) > width {
                while crate::hud_style::width(&format!("{name}…"), 16.) > width {
                    if name.pop().is_none() {
                        break;
                    }
                }
                name.push('…');
            }
            let text = if remaining == 0 {
                name
            } else {
                format!("{} · ready in {:.1}s", name, remaining as f64 / 60.)
            };
            self.text(ctx, &format!("{wheel_path}.name"), &text);
        }
        let path = format!("{PATH}.floating");
        let projected = self.state.active.and_then(|active| {
            let frame = camera.frame()?;
            let p = frame.project(position?)?;
            let age = self.state.tick.saturating_sub(active.at) as f32 / 60.;
            let (rect, alpha) = config.geometry(p, age, frame.zoom);
            // Do not float an off-screen champion's emote over HUD/minimap.
            let bottom = (rect.x + rect.w, rect.y + rect.h);
            let minimap_overlap = rect.x < frame.minimap.x + frame.minimap.w
                && rect.x + rect.w > frame.minimap.x
                && rect.y < frame.minimap.y + frame.minimap.h
                && rect.y + rect.h > frame.minimap.y;
            (frame.viewport.contains((rect.x, rect.y))
                && frame.viewport.contains(bottom)
                && !frame.minimap.contains((rect.x, rect.y))
                && !frame.minimap.contains(bottom)
                && !minimap_overlap
                && !camera.overlay_intersects(rect))
            .then_some((active.slot, rect, alpha))
        });
        if let Some((slot, r, alpha)) = projected {
            let source = self.active_art.as_ref().map_or_else(
                || format!("asset/lt_direct_control_probe/ui/{}", CATALOGUE[slot].glyph),
                |a| a.source.clone(),
            );
            self.props(ctx, &path, &format!("visible: true; x: {:.1}px; y: {:.1}px; width: {:.1}px; height: {:.1}px; color: #ffffff{alpha:02x}; source: {};", r.x, r.y, r.w, r.h, serde_json::to_string(&source).unwrap()));
        } else {
            self.props(ctx, &path, "visible: false;");
        }
        if std::mem::take(&mut self.pending_sound) && config.sound {
            // This small original asset is prepackaged; SDK queues playback.
            if !ctx.play_sound("asset/lt_direct_control_probe/sound/sfx/emote", 0.3) {
                log.write("EMOTE sound request unavailable; image remains usable");
            }
        }
    }
}
fn animation(age: f32) -> (f32, f32, u8) {
    let age = age.clamp(0., 2.);
    let scale = if age < 0.12 {
        0.5 + 0.65 * (age / 0.12)
    } else if age < 0.24 {
        1.15 - 0.15 * ((age - 0.12) / 0.12)
    } else {
        1.
    };
    let alpha = if age < 0.08 {
        age / 0.08
    } else {
        ((2. - age) / 0.3).clamp(0., 1.)
    };
    (
        88. * scale,
        8. * (age / 0.24).min(1.),
        (alpha * 255.).round() as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context(tick: usize) -> Context {
        Context {
            identity: Some(Identity {
                generation: 1,
                key: (1, 2, 3),
                player: 4,
            }),
            tick,
            controlling: true,
            running: true,
            alive: true,
            enabled: true,
            blocked: false,
            aiming: false,
            may_open: true,
            cooldown_ticks: 90,
        }
    }
    fn keys(down: bool, p: (f32, f32)) -> Keys {
        Keys {
            emote: down,
            cursor: Some(p),
            focused: true,
            ..Keys::default()
        }
    }
    fn ready() -> State {
        let mut s = State::default();
        s.input(keys(false, (500., 500.)), context(1));
        s
    }
    fn fire(s: &mut State, at: usize) -> (bool, Option<usize>) {
        s.input(keys(false, (500., 500.)), context(at));
        s.input(keys(true, (500., 500.)), context(at));
        s.input(keys(false, (500., 390.)), context(at))
    }
    #[test]
    fn release_selects_once_and_cooldown_follows_played_ticks() {
        let mut s = ready();
        assert_eq!(fire(&mut s, 10), (true, Some(1)));
        assert_eq!(s.input(keys(false, (500., 500.)), context(10)).1, None);
        assert_eq!(fire(&mut s, 11).1, None);
        assert_eq!(fire(&mut s, 99).1, None);
        assert_eq!(fire(&mut s, 100).1, Some(1));
        s.input(keys(false, (500., 500.)), context(219));
        assert!(s.active.is_some());
        s.input(keys(false, (500., 500.)), context(220));
        assert!(s.active.is_none());
    }
    #[test]
    fn focus_takeover_and_modal_close_do_not_replay_held_key() {
        for mode in 0..5 {
            let mut s = ready();
            s.input(keys(true, (500., 500.)), context(2));
            let mut c = context(3);
            let mut k = keys(true, (500., 500.));
            match mode {
                0 => k.focused = false,
                1 => c.controlling = false,
                2 => c.alive = false,
                3 => c.blocked = true,
                _ => c.aiming = true,
            }
            s.input(k, c);
            assert!(s.wheel.is_none());
            s.input(keys(true, (500., 500.)), context(4));
            assert!(s.wheel.is_none());
            s.input(keys(false, (500., 500.)), context(5));
            s.input(keys(true, (500., 500.)), context(6));
            assert!(s.wheel.is_some());
        }
    }
    #[test]
    fn right_cancel_is_drained_until_buttons_are_released() {
        let mut s = ready();
        s.input(keys(true, (500., 500.)), context(2));
        let mut k = keys(true, (500., 500.));
        k.right = true;
        assert_eq!(s.input(k, context(3)), (true, None));
        assert!(s.wheel.is_none());
        k.emote = false;
        assert!(s.input(k, context(4)).0);
        s.input(keys(false, (500., 500.)), context(5));
        assert!(!s.input(keys(false, (500., 500.)), context(6)).0);
    }
    #[test]
    fn escape_cancel_cannot_clear_the_existing_order_on_later_frames() {
        let mut s = ready();
        s.input(keys(true, (500., 500.)), context(2));
        let mut cancel = keys(false, (500., 500.));
        cancel.escape = true;
        for tick in 3..10 {
            assert_eq!(s.input(cancel, context(tick)), (true, None));
        }
        s.input(keys(false, (500., 500.)), context(10));
        assert!(!s.input(keys(false, (500., 500.)), context(11)).0);
    }
    #[test]
    fn pause_freezes_duration_and_session_change_clears_everything() {
        let mut s = ready();
        fire(&mut s, 10);
        let mut paused = context(11);
        paused.running = false;
        for _ in 0..1000 {
            s.input(keys(false, (500., 500.)), paused);
        }
        assert!(s.active.is_some());
        assert_eq!(s.remaining(), 89);
        let mut next = context(0);
        next.identity.as_mut().unwrap().generation = 2;
        s.input(keys(true, (500., 500.)), next);
        assert!(s.active.is_none() && s.cooldown.is_none() && s.wheel.is_none());
    }
    #[test]
    fn center_is_default_sectors_and_edges_are_bounded() {
        assert_eq!(pick((0., 0.), (20., 20.)), 0);
        assert_eq!(pick((0., 0.), (0., -80.)), 1);
        assert_eq!(pick((0., 0.), (80., 0.)), 2);
        assert_eq!(pick((0., 0.), (0., 80.)), 3);
        assert_eq!(pick((0., 0.), (-80., 0.)), 4);
        for p in [(0., 0.), (1920., 1080.)] {
            let c = wheel_center(p);
            assert!(c.0 - HALF >= 8. && c.0 + HALF <= 1912.);
            assert!(c.1 - HALF >= 8. && c.1 + HALF + 46. <= 1080.);
        }
        let mut s = ready();
        s.input(keys(true, (f32::NAN, 10.)), context(2));
        assert!(s.wheel.is_none());
    }
    #[test]
    fn animation_is_bounded_and_settles_without_catch_up_steps() {
        for n in 0..121 {
            let (size, lift, _) = animation(n as f32 / 60.);
            assert!((44. ..=101.2).contains(&size) && (0. ..=8.).contains(&lift));
        }
        assert_eq!(animation(2.).2, 0);
        assert_eq!(animation(1.).0, 88.);
        assert!(template().contains("ignore_event: true;"));
    }
    #[test]
    fn fixed_and_follow_zoom_use_the_same_anchor_without_stretching() {
        let mut values = crate::settings::Values::default();
        let fixed = Config::from_values(&values);
        let (normal, _) = fixed.geometry((500., 800.), 1., 1.);
        for zoom in [0.5, 1., 2., 3.] {
            assert_eq!(fixed.geometry((500., 800.), 1., zoom).0, normal);
        }
        values.set("emote_zoom", 1.);
        values.set("emote_scale", 150.);
        values.set("emote_height", 120.);
        let follow = Config::from_values(&values);
        for zoom in [0.5, 1., 2., 3.] {
            let (r, _) = follow.geometry((500., 800.), 1., zoom);
            assert_eq!(r.w, 132. * zoom);
            assert_eq!(r.w, r.h);
            assert_eq!(r.x + r.w / 2., 500.);
            assert_eq!(800. - r.y - r.h, 128. * zoom);
        }
        assert_eq!(
            follow.geometry((0., 0.), 1., f32::NAN).0,
            follow.geometry((0., 0.), 1., 1.).0
        );
    }
    #[test]
    fn shorter_cooldown_replaces_image_and_applied_changes_take_effect() {
        let mut s = ready();
        fire(&mut s, 10);
        assert_eq!(s.active.unwrap().at, 10);
        assert_eq!(fire(&mut s, 99).1, None);
        assert_eq!(fire(&mut s, 100).1, Some(1));
        assert_eq!(s.active.unwrap().at, 100);
        let mut c = context(101);
        c.cooldown_ticks = 15;
        s.input(keys(false, (500., 500.)), c);
        c.tick = 115;
        s.input(keys(true, (500., 500.)), c);
        assert_eq!(s.input(keys(false, (600., 500.)), c).1, Some(2));
        assert_eq!(s.active.unwrap().slot, 2);
        assert_eq!(s.active.unwrap().at, 115);
    }
}
