//! Persisted values and declarative controls. No host UI handles or input edges live here.
use crate::Logger;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};
pub static GLOBAL: OnceLock<Arc<Settings>> = OnceLock::new();
#[derive(Clone, Copy)]
pub enum Control {
    Choice(&'static [&'static str]),
    Toggle,
    Slider(f64, f64, f64, &'static str),
    /// A 0xRRGGBB colour typed as "#rrggbb".
    Color,
}
pub struct OptionDef {
    pub key: &'static str,
    pub page: usize,
    pub section: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub control: Control,
    pub default: f64,
}
pub static OPTIONS: &[OptionDef] = &[
    OptionDef {
        key: "champion_mode",
        page: 0,
        section: "Targeting",
        label: "Champion-only activation",
        hint: "Hold the bound key to restrict direct targeting.",
        control: Control::Choice(&["Hold", "Toggle"]),
        default: 0.,
    },
    OptionDef {
        key: "attack_move_filter",
        page: 0,
        section: "Targeting",
        label: "Champion-only during attack-move",
        hint: "Applies to both A → left-click and Shift + right-click.",
        control: Control::Choice(&["Ignore", "Honor"]),
        default: 0.,
    },
    OptionDef {
        key: "attack_move_preference",
        page: 0,
        section: "Targeting",
        label: "Attack-move target preference",
        hint: "A → left-click only. Near cursor walks up to the enemy nearest the click; Shift + right-click always takes the enemy nearest you.",
        control: Control::Choice(&["Near champion", "Near cursor"]),
        default: 0.,
    },
    OptionDef {
        key: "cast_q",
        page: 0,
        section: "Ability casting",
        label: "Q · Default cast mode",
        hint: "Preview and self-cast overrides remain separate bindings.",
        control: Control::Choice(&["Quick cast", "Normal cast", "Cast on release"]),
        default: 0.,
    },
    OptionDef {
        key: "cast_w",
        page: 0,
        section: "Ability casting",
        label: "W · Default cast mode",
        hint: "Preview and self-cast overrides remain separate bindings.",
        control: Control::Choice(&["Quick cast", "Normal cast", "Cast on release"]),
        default: 0.,
    },
    OptionDef {
        key: "cast_r",
        page: 0,
        section: "Ability casting",
        label: "R · Default cast mode",
        hint: "Preview and self-cast overrides remain separate bindings.",
        control: Control::Choice(&["Quick cast", "Normal cast", "Cast on release"]),
        default: 0.,
    },
    OptionDef {
        key: "start_mode",
        page: 4,
        section: "Match start",
        label: "Start mode",
        hint: "Choose how each match starts.",
        control: Control::Choice(&["Ask every match", "Always play my role", "Start spectating"]),
        default: 0.,
    },
    OptionDef {
        key: "preferred_role",
        page: 4,
        section: "Match start",
        label: "My role",
        hint: "Remembered when you choose a role before starting a match.",
        control: Control::Choice(&["Top", "Jungle", "Mid", "Bottom", "Support"]),
        default: 0.,
    },
    OptionDef {
        key: "attack_cancel",
        page: 4,
        section: "Attacks",
        label: "Cancel attack wind-down",
        hint: "A move, stop or held skill after the hit cuts the swing short.",
        control: Control::Toggle,
        default: 1.,
    },
    OptionDef {
        key: "athlete_stat_rules",
        page: 4,
        section: "Athlete stats",
        label: "Athlete stat rules",
        hint: "Experimental: reworks how the player's stats affect gameplay. Turning it on takes effect after you return to the main menu.",
        control: Control::Toggle,
        default: 0.,
    },
    OptionDef {
        key: "stat_reaction_floor",
        page: 4,
        section: "Athlete stats",
        label: "Reaction floor",
        hint: "Testing: the fastest an enemy can react to your shots.",
        control: Control::Slider(1., 40., 1., " ticks"),
        default: 12.,
    },
    OptionDef {
        key: "stat_reaction_ceiling",
        page: 4,
        section: "Athlete stats",
        label: "Reaction ceiling",
        hint: "Testing: the slowest an enemy can react to your shots.",
        // Up to the base game's slowest gate (athlete_rules::VANILLA_CEILING).
        control: Control::Slider(1., 132., 1., " ticks"),
        default: 35.,
    },
    OptionDef {
        key: "stat_collision_margin",
        page: 4,
        section: "Athlete stats",
        label: "Collision margin",
        hint: "Testing: how much extra room your shots get to hit.",
        control: Control::Slider(0., 7., 1., " units"),
        default: 2.,
    },
    OptionDef {
        key: "manual_shop",
        page: 4,
        section: "Shopping",
        label: "Manual shopping",
        hint: "Auto-buy stops for your champion; buy from the shop (P) instead.",
        control: Control::Toggle,
        default: 0.,
    },
    // Shown as a checkbox inside the shop window only (page 9 is never laid out).
    OptionDef {
        key: "shop_pause",
        page: 9,
        section: "Shopping",
        label: "Pause while the shop is open",
        hint: "",
        control: Control::Toggle,
        default: 0.,
    },
    // Shown as a checkbox inside the shop window only (page 9 is never laid out).
    OptionDef {
        key: "stats_panel_shown",
        page: 9,
        section: "Interface",
        label: "Stats panel",
        hint: "",
        control: Control::Toggle,
        default: 1.,
    },
    // Shown as a checkbox inside the shop window only (page 9 is never laid out).
    OptionDef {
        key: "shop_vanilla_order",
        page: 9,
        section: "Shopping",
        label: "Vanilla order",
        hint: "",
        control: Control::Toggle,
        default: 1.,
    },
    OptionDef {
        key: "camera_lock",
        page: 2,
        section: "Follow",
        label: "Camera lock",
        hint: "Follow the controlled champion until unlocked.",
        control: Control::Toggle,
        default: 0.,
    },
    OptionDef {
        key: "edge_pan",
        page: 2,
        section: "Navigation",
        label: "Edge scrolling",
        hint: "Move the camera when the pointer reaches the screen edge.",
        control: Control::Toggle,
        default: 1.,
    },
    OptionDef {
        key: "pointer_clip",
        page: 2,
        section: "Navigation",
        label: "Keep pointer in window",
        hint: "Keeps the pointer inside the game window while you control a match, so it cannot slip onto another monitor. It is free while paused or in a window.",
        control: Control::Toggle,
        default: 1.,
    },
    OptionDef {
        key: "drag",
        page: 2,
        section: "Navigation",
        label: "Camera dragging",
        hint: "Hold the bound mouse button to pan.",
        control: Control::Toggle,
        default: 1.,
    },
    OptionDef {
        key: "zoom",
        page: 2,
        section: "Navigation",
        label: "Mouse-wheel zoom",
        hint: "Scroll to adjust battlefield zoom.",
        control: Control::Toggle,
        default: 1.,
    },
    OptionDef {
        key: "pan_speed",
        page: 2,
        section: "Navigation",
        label: "Pan speed",
        hint: "Applies to edge scrolling and keyboard panning.",
        control: Control::Slider(0.5, 3., 0.1, "×"),
        default: 1.4,
    },
    OptionDef {
        key: "mod_language",
        page: 3,
        section: "Language",
        label: "Mod language",
        hint: "Auto follows the game's language.",
        // Stored as an index into lang::CHOICES: never reorder.
        control: Control::Choice(&crate::lang::CHOICES),
        default: 0.,
    },
    OptionDef {
        key: "cursor_size",
        page: 3,
        section: "Cursor",
        label: "Cursor size",
        hint: "Changes the pointer size without moving its click point.",
        control: Control::Slider(24., 64., 1., " px"),
        default: crate::cursor::DEFAULT_SIZE as f64,
    },
    OptionDef {
        key: "background_color",
        page: 3,
        section: "Appearance",
        label: "Background colour",
        hint: "Hex colour of the mod's panels and HUD strip, e.g. #1c1a18. Applies when you apply settings.",
        control: Control::Color,
        default: crate::ui_theme::DEFAULT as f64,
    },
    OptionDef {
        key: "hud_tooltips",
        page: 3,
        section: "Appearance",
        label: "HUD tooltips",
        hint: "When skill and item tooltips on the HUD appear. They are hidden while a mouse button is held. The key is set under Key bindings.",
        control: Control::Choice(&["On hover", "Hold key and hover", "Off"]),
        default: 0.,
    },
    // motion: switch. Experimental (design note "HUD motion pass"): one switch for the whole
    // pass in hud_motion_pass.rs; Off shows the HUD as it was before it.
    // On by default, for releases too (the author's decision, 1.3.7).
    OptionDef {
        key: "hud_motion",
        page: 3,
        section: "Appearance",
        label: "HUD motion",
        hint: "Small animations on the HUD; Off keeps it still.",
        control: Control::Toggle,
        default: 1.,
    },
    // Display only (live_rating.rs): the game's own match rating. Off by default.
    OptionDef {
        key: "live_rating",
        page: 3,
        section: "Appearance",
        label: "Live rating",
        hint: "Your athlete's match rating, small in the top-right corner, as the game calculates it.",
        control: Control::Toggle,
        default: 0.,
    },
    OptionDef {
        key: "low_health_effect",
        page: 3,
        section: "Battlefield feedback",
        label: "Low-health screen effect",
        hint: "Battlefield only, below 25% HP.",
        control: Control::Toggle,
        default: 0.,
    },
    OptionDef {
        // 0 Off, 1 Outline (ground rings while another mod holds the unit
        // renderer), 2 Ground ring (the outline hook is not installed).
        key: "hover_outline",
        page: 3,
        section: "Battlefield feedback",
        label: "Unit highlight",
        hint: "Hover and attack target; blue allies, red hostiles. Ground ring is the more compatible option with mods that redraw units; switching to or from it needs a game restart.",
        control: Control::Choice(&["Off", "Outline", "Ground ring"]),
        default: 1.,
    },
    OptionDef {
        key: "map_path",
        page: 3,
        section: "Battlefield feedback",
        label: "Minimap movement path",
        hint: "Keep your route visible until arrival or a new command.",
        control: Control::Toggle,
        default: 1.,
    },
    OptionDef {
        key: "skill_ready_sound",
        page: 3,
        section: "Sounds",
        label: "Skill ready sound",
        hint: "A short chime when one of your skills comes off cooldown.",
        control: Control::Toggle,
        default: 0.,
    },
    OptionDef {
        key: "denied_cast_sound",
        page: 3,
        section: "Sounds",
        label: "Denied cast sound",
        hint: "A short click when a skill press cannot cast.",
        control: Control::Toggle,
        default: 1.,
    },
    OptionDef {
        key: "interface_sounds",
        page: 3,
        section: "Sounds",
        label: "Interface sounds",
        hint: "The game's hover and click sounds in the mod's windows.",
        control: Control::Toggle,
        default: 1.,
    },
    OptionDef {
        key: "emotes", page: 7, section: "Emotes", label: "Emote wheel",
        hint: "Hold the bound key, choose a direction, then release. Centre selects GG. Local display only.",
        control: Control::Toggle, default: 1.,
    },
    OptionDef {
        // Saved as "emote_volume" since before it governed every mod sound.
        key: "emote_volume", page: 7, section: "Emotes", label: "Mod sound volume",
        hint: "Loudness of the mod's sounds, relative to the game's sound effects. 0% is silent.",
        control: Control::Slider(0., 100., 5., "%"), default: 70.,
    },
    OptionDef {
        key: "emote_height", page: 7, section: "Display", label: "Height",
        hint: "Distance from the champion's ground position to the image's lower edge.",
        control: Control::Slider(0., 240., 1., " px"), default: 108.,
    },
    OptionDef {
        key: "emote_scale", page: 7, section: "Display", label: "Scale",
        hint: "100% displays an 88 px image at normal zoom.",
        control: Control::Slider(50., 200., 5., "%"), default: 100.,
    },
    OptionDef {
        key: "emote_zoom", page: 7, section: "Display", label: "Zoom scaling",
        hint: "Follow camera scales both image size and height with battlefield zoom.",
        control: Control::Choice(&["Fixed screen size", "Follow camera"]), default: 0.,
    },
    OptionDef {
        key: "emote_cooldown", page: 7, section: "Timing", label: "Cooldown",
        hint: "Shared by all emotes. A new emote replaces the previous image.",
        control: Control::Slider(0.25, 6., 0.25, " s"), default: 1.5,
    },
    OptionDef {
        key: "show_input_delay",
        page: 5,
        section: "Debug",
        label: "Show input delay",
        hint: "Shows the measured time from your click or key to the frame that plays it, averaged over your last commands.",
        control: Control::Toggle,
        default: 0.,
    },
    OptionDef {
        key: "perf_capture", page: 5, section: "Debug",
        label: "Capture performance measurements",
        hint: "Aggregates client, simulation, hook and input-to-playback timings every 10 seconds. Adds profiling overhead.",
        control: Control::Toggle, default: 0.,
    },
    OptionDef {
        key: "acquisition_debug",
        page: 5,
        section: "Debug",
        label: "Show live acquisition radius",
        hint: "Yellow: acquisition. White: AA reach. Uses live targeting values for your champion.",
        control: Control::Toggle,
        default: 0.,
    },
    OptionDef {
        key: "log_level",
        page: 5,
        section: "Debug",
        label: "Log detail",
        hint: "Verbose adds per-click and per-second traces for investigations.",
        control: Control::Choice(&["Quiet", "Normal", "Verbose"]),
        default: 1.,
    },
    OptionDef {
        key: "collision_debug",
        page: 5,
        section: "Debug",
        label: "Show hit areas",
        hint: "Testing: draws rings around enemies near the cursor to show the hit area.",
        control: Control::Toggle,
        default: 0.,
    },
    OptionDef {
        key: "selection_debug",
        page: 5,
        section: "Debug",
        label: "Show selection markers",
        hint: "Show hover and selected-target ground markers for selection tuning.",
        control: Control::Toggle,
        default: 0.,
    },
];
pub struct BindingDef {
    pub key: &'static str,
    pub group: &'static str,
    pub label: &'static str,
    pub defaults: [Option<Chord>; 2],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    pub code: u8,
    pub mods: u8,
}
const fn chord(code: u8, mods: u8) -> Option<Chord> {
    Some(Chord { code, mods })
}
macro_rules! bind {
    ($k:literal,$g:literal,$l:literal,$c:expr,$m:expr) => {
        BindingDef {
            key: $k,
            group: $g,
            label: $l,
            defaults: [chord($c, $m), None],
        }
    };
}
pub static BINDINGS: &[BindingDef] = &[
    bind!(
        "move",
        "Movement & targeting",
        "Move / attack clicked target",
        2,
        0
    ),
    bind!(
        "attack_aim",
        "Movement & targeting",
        "Attack-move aiming",
        0x41,
        0
    ),
    bind!(
        "attack_click",
        "Movement & targeting",
        "Attack-move click",
        2,
        1
    ),
    BindingDef {
        key: "champion_only",
        group: "Movement & targeting",
        label: "Target champions only",
        defaults: [chord(0xc0, 0), chord(5, 0)],
    },
    bind!("stop", "Movement & targeting", "Stop", 0x53, 0),
    bind!("recall", "Movement & targeting", "Recall", 0x42, 0),
    bind!("q", "Abilities", "Q ability", 0x51, 0),
    bind!("w", "Abilities", "W ability", 0x57, 0),
    bind!("r", "Abilities", "R ability", 0x52, 0),
    bind!(
        "preview_q",
        "Abilities",
        "Q preview / normal-cast override",
        0x51,
        1
    ),
    bind!(
        "preview_w",
        "Abilities",
        "W preview / normal-cast override",
        0x57,
        1
    ),
    bind!(
        "preview_r",
        "Abilities",
        "R preview / normal-cast override",
        0x52,
        1
    ),
    bind!("self_q", "Abilities", "Q self-cast", 0x51, 4),
    bind!("self_w", "Abilities", "W self-cast", 0x57, 4),
    bind!("self_r", "Abilities", "R self-cast", 0x52, 4),
    bind!(
        "start",
        "Control & camera",
        "Start, pause, resume or reclaim control",
        0x7a,
        0
    ),
    bind!(
        "release",
        "Control & camera",
        "Return control to AI",
        0x7b,
        0
    ),
    bind!("tab", "Control & camera", "Team details (hold)", 9, 0),
    bind!("shop", "Control & camera", "Shop", 0x50, 0),
    bind!("emote", "Control & camera", "Emote wheel (hold)", 0x54, 0),
    bind!(
        "stats_panel",
        "Control & camera",
        "Stats panel (toggle)",
        0x43,
        0
    ),
    bind!(
        "camera_toggle",
        "Control & camera",
        "Lock / unlock camera",
        0x59,
        0
    ),
    bind!(
        "follow",
        "Control & camera",
        "Follow champion (hold)",
        0x20,
        0
    ),
    bind!("drag_camera", "Control & camera", "Drag camera", 4, 0),
    bind!(
        "hud_tooltip",
        "Control & camera",
        "HUD tooltip (hold)",
        0x12,
        0
    ),
    bind!("pan_up", "Control & camera", "Pan camera up", 0x49, 0),
    bind!("pan_down", "Control & camera", "Pan camera down", 0x4b, 0),
    bind!("pan_left", "Control & camera", "Pan camera left", 0x4a, 0),
    bind!("pan_right", "Control & camera", "Pan camera right", 0x4c, 0),
    bind!("select_top", "Choose controlled athlete", "Top", 0x31, 2),
    bind!(
        "select_jungle",
        "Choose controlled athlete",
        "Jungle",
        0x32,
        2
    ),
    bind!("select_mid", "Choose controlled athlete", "Mid", 0x33, 2),
    bind!(
        "select_bottom",
        "Choose controlled athlete",
        "Bottom",
        0x34,
        2
    ),
    bind!(
        "select_support",
        "Choose controlled athlete",
        "Support",
        0x35,
        2
    ),
];
#[derive(Clone, Debug)]
pub struct Values(pub Value);
impl Default for Values {
    fn default() -> Self {
        Self(json!({"version":1}))
    }
}
impl Values {
    pub fn number(&self, key: &str) -> f64 {
        let def = OPTIONS
            .iter()
            .find(|d| d.key == key)
            .expect("declared option");
        let n = self
            .0
            .get(key)
            .and_then(|v| {
                v.as_f64()
                    .or_else(|| v.as_bool().map(|b| f64::from(b as u8)))
            })
            .filter(|n| n.is_finite())
            .or_else(|| self.before_emote_volume(key))
            .unwrap_or(def.default);
        match def.control {
            Control::Choice(v) => {
                if n.fract() == 0. && n >= 0. && n < (v.len() as f64) {
                    n
                } else {
                    def.default
                }
            }
            Control::Toggle => {
                if n == 0. || n == 1. {
                    n
                } else {
                    def.default
                }
            }
            Control::Slider(lo, hi, step, _) => {
                ((n.clamp(lo, hi) / step).round() * step).clamp(lo, hi)
            }
            Control::Color => {
                if n.fract() == 0. && (0. ..=f64::from(0xff_ffff)).contains(&n) {
                    n
                } else {
                    def.default
                }
            }
        }
    }
    /// Settings saved before the volume slider had an "emote_sound" switch,
    /// off unless the player turned it on. Off stays silent.
    fn before_emote_volume(&self, key: &str) -> Option<f64> {
        let off = |v: &Value| v.as_f64() == Some(0.) || v.as_bool() == Some(false);
        (key == "emote_volume" && self.0.get("emote_sound").is_some_and(off)).then_some(0.)
    }
    pub fn set(&mut self, key: &str, n: f64) {
        self.0[key] = json!(n);
        let n = self.number(key);
        self.0[key] = json!(n)
    }
    pub fn binding(&self, key: &str) -> [Option<Chord>; 2] {
        let def = BINDINGS
            .iter()
            .find(|d| d.key == key)
            .expect("declared binding");
        std::array::from_fn(|i| {
            match self
                .0
                .get("bindings")
                .and_then(|v| v.get(key))
                .and_then(|v| v.get(i))
            {
                None => def.defaults[i],
                Some(Value::Null) => None,
                Some(v) => {
                    let code = v.get("code")?.as_u64()?;
                    let mods = v.get("mods")?.as_u64()?;
                    (code > 0 && code < 256 && mods < 8).then_some(Chord {
                        code: code as u8,
                        mods: mods as u8,
                    })
                }
            }
        })
    }
    pub fn bind(&mut self, key: &str, slot: usize, c: Option<Chord>) {
        let mut a = self.binding(key);
        a[slot] = c;
        if !self.0.get("bindings").is_some_and(Value::is_object) {
            self.0["bindings"] = json!({})
        }
        self.0["bindings"][key] = json!(a.map(|c| c.map(|c| json!({"code":c.code,"mods":c.mods}))));
    }
    pub fn conflicts(&self, c: Chord, key: &str, slot: usize) -> Vec<(&'static str, usize)> {
        BINDINGS
            .iter()
            .flat_map(|d| {
                self.binding(d.key)
                    .into_iter()
                    .enumerate()
                    .filter_map(move |(i, b)| {
                        (b == Some(c) && (d.key != key || i != slot)).then_some((d.key, i))
                    })
            })
            .collect()
    }
    pub fn essential(&self) -> bool {
        ["move", "start", "release"]
            .iter()
            .all(|k| self.binding(k).iter().any(Option::is_some))
    }
    pub fn pressed(&self, key: &str, raw: &[bool; 256]) -> bool {
        self.binding(key).into_iter().flatten().any(|c| {
            if !raw[c.code as usize] {
                return false;
            }
            let mods = modifiers(raw);
            if mods & c.mods != c.mods {
                return false;
            } // More specific chords suppress their base command.
            !BINDINGS.iter().any(|d| {
                d.key != key
                    && self.binding(d.key).into_iter().flatten().any(|o| {
                        o.code == c.code
                            && o.mods != c.mods
                            && o.mods & c.mods == c.mods
                            && mods & o.mods == o.mods
                    })
            })
        })
    }
    pub fn reset_page(&mut self, page: usize) {
        if page == 7 {
            crate::emote_library::reset_slots(self);
        }
        if page == 6 {
            self.0
                .as_object_mut()
                .expect("settings object")
                .remove("acquisition");
        }
        if page == 1 {
            for d in BINDINGS {
                for i in 0..2 {
                    self.bind(d.key, i, d.defaults[i]);
                }
            }
        } else {
            for d in OPTIONS.iter().filter(|d| d.page == page) {
                self.set(d.key, d.default);
            }
        }
    }
}
pub fn modifiers(raw: &[bool; 256]) -> u8 {
    u8::from(raw[0x10]) | (u8::from(raw[0x11]) * 2) | (u8::from(raw[0x12]) * 4)
}
impl Chord {
    /// The full name the Settings window shows: `Shift + B`, `Mouse4`,
    /// `Page Up`; punctuation by the keyboard layout (`key_names`).
    pub fn label(self) -> String {
        let mut out = String::new();
        for (bit, prefix) in [(2, "Ctrl + "), (1, "Shift + "), (4, "Alt + ")] {
            if self.mods & bit != 0 {
                out.push_str(prefix);
            }
        }
        crate::key_names::write(self.code, false, &mut out);
        out
    }
    /// The short name a key cap or tooltip shows (1.6.0-dev4): `S+B`,
    /// `F11`, `M4`, `Tab`. Written into `out` so a caller can keep its
    /// buffer; `label` stays the Settings window's full name.
    pub fn short(self, out: &mut String) {
        for (bit, prefix) in [(2, "C+"), (1, "S+"), (4, "A+")] {
            if self.mods & bit != 0 {
                out.push_str(prefix);
            }
        }
        crate::key_names::write(self.code, true, out);
    }
}
impl Values {
    /// The chord a key cap shows: the first one bound to `key`, if any.
    pub fn shown_chord(&self, key: &str) -> Option<Chord> {
        self.binding(key).into_iter().flatten().next()
    }
}
pub struct Settings {
    values: Mutex<Values>,
    input: Mutex<Arc<Values>>,
    dirty: Mutex<Option<Instant>>,
    root: Option<PathBuf>,
}
impl Settings {
    pub fn new(root: Option<&Path>) -> Self {
        let value = root
            .and_then(|p| std::fs::read(p.join("controls.json")).ok())
            .filter(|b| b.len() <= 1_048_576)
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({"version":1}));
        let mut value = Values(value);
        let before = value.clone();
        crate::acquisition::migrate_defaults(&mut value);
        let migrated = value.0 != before.0;
        let input = Arc::new(input_values(&value));
        Self {
            values: Mutex::new(value),
            input: Mutex::new(input),
            dirty: Mutex::new(migrated.then(Instant::now)),
            root: root.map(Path::to_owned),
        }
    }
    pub fn snapshot(&self) -> Values {
        self.values.lock().map(|v| v.clone()).unwrap_or_default()
    }
    fn input_snapshot(&self) -> Values {
        (*self.input_shared()).clone()
    }
    fn input_shared(&self) -> Arc<Values> {
        self.input
            .lock()
            .map(|v| v.clone())
            .unwrap_or_else(|_| Arc::new(Values::default()))
    }
    pub fn number(&self, key: &str) -> f64 {
        self.values
            .lock()
            .map(|v| v.number(key))
            .unwrap_or_else(|_| Values::default().number(key))
    }
    pub fn size(&self) -> u32 {
        self.number("cursor_size") as u32
    }
    /// A switch only kept in controls.json (no control in the window), such
    /// as the practice tool's `practice_mode`: on for `1`, `1.0` or `true`.
    pub fn hidden_switch(&self, key: &str) -> bool {
        self.values.lock().is_ok_and(|v| {
            v.0.get(key)
                .is_some_and(|v| v.as_f64() == Some(1.) || v.as_bool() == Some(true))
        })
    }
    /// A text value only kept in controls.json (no control in the window),
    /// such as the practice tool's `practice_lineup`.
    pub fn text(&self, key: &str) -> Option<String> {
        self.values
            .lock()
            .ok()?
            .0
            .get(key)?
            .as_str()
            .map(str::to_owned)
    }
    pub fn acquisition_radius(&self, name: &str, current: u64, maximum: Option<u64>) -> u64 {
        self.values.lock().map_or_else(
            |_| crate::acquisition::radius(&Values::default(), name, current, maximum),
            |v| crate::acquisition::radius(&v, name, current, maximum),
        )
    }
    pub fn low_health(&self) -> bool {
        self.number("low_health_effect") == 1.
    }
    #[cfg(test)]
    pub fn set(&self, size: u32) {
        let mut v = self.snapshot();
        v.set("cursor_size", size as f64);
        self.apply(v);
    }
    #[cfg(test)]
    pub fn toggle_low_health(&self) {
        let mut v = self.snapshot();
        v.set("low_health_effect", if self.low_health() { 0. } else { 1. });
        self.apply(v);
    }
    pub fn apply(&self, mut v: Values) {
        crate::acquisition::register_defaults(&mut v, &crate::acquisition::champions());
        for d in OPTIONS {
            v.set(d.key, v.number(d.key));
        }
        for d in BINDINGS {
            let bindings = v.binding(d.key);
            for (i, c) in bindings.into_iter().enumerate() {
                v.bind(d.key, i, c);
            }
        }
        if let Ok(mut saved) = self.values.lock() {
            if let Ok(mut input) = self.input.lock() {
                *input = Arc::new(input_values(&v));
            }
            *saved = v;
            if let Ok(mut at) = self.dirty.lock() {
                *at = Some(Instant::now());
            }
        }
    }
    /// The values a save writes now (taken on the client thread).
    fn save_job(&self) -> std::io::Result<crate::settings_writer::Job> {
        let root = self
            .root
            .as_ref()
            .ok_or_else(|| std::io::Error::other("Settings folder unavailable"))?;
        Ok(crate::settings_writer::Job {
            root: root.clone(),
            values: self.snapshot().0,
            cursor_size: self.size(),
            low_health: self.low_health(),
        })
    }
    /// Save now, on this thread (tests).
    #[cfg(test)]
    pub(crate) fn save(&self) -> std::io::Result<()> {
        crate::settings_writer::save_to(&self.save_job()?)
    }
    /// Once per frame (`force`: now, synchronously; the Settings window's
    /// Apply and shutdown). A change is saved 300 ms after it was made, on
    /// the settings writer thread (1.6.0), so the frame does not wait for
    /// the disk; a failed save is tried again 5 s later.
    pub fn flush(&self, force: bool, log: &Logger) {
        for result in crate::settings_writer::take_results() {
            self.saved(result.map_err(std::io::Error::other), false, log);
        }
        let ready = self
            .dirty
            .lock()
            .is_ok_and(|at| at.is_some_and(|t| force || t.elapsed() >= Duration::from_millis(300)));
        if force {
            // A queued save must not land after this one.
            crate::settings_writer::settle(Duration::from_secs(2));
            for result in crate::settings_writer::take_results() {
                self.saved(result.map_err(std::io::Error::other), false, log);
            }
        }
        if !ready {
            return;
        }
        let job = match self.save_job() {
            Ok(job) => job,
            Err(e) => return self.saved(Err(e), false, log),
        };
        if !force {
            if let Ok(mut at) = self.dirty.lock() {
                *at = None;
            }
            // No writer thread: save here, as before 1.6.0.
            if let Err(job) = crate::settings_writer::queue(job) {
                self.saved(crate::settings_writer::save_to(&job), false, log);
            }
            return;
        }
        self.saved(crate::settings_writer::save_to(&job), true, log);
    }
    /// Log a save's result. `clear`: a synchronous save of the current
    /// values, so nothing is left to save; a background save leaves alone
    /// a change made after its values were taken.
    fn saved(&self, result: std::io::Result<()>, clear: bool, log: &Logger) {
        match result {
            Ok(()) => {
                if clear {
                    if let Ok(mut at) = self.dirty.lock() {
                        *at = None;
                    }
                }
                log.write("SETTINGS saved controls.json");
            }
            Err(e) => {
                log.write(&format!("SETTINGS active but save failed: {e}"));
                if let Ok(mut at) = self.dirty.lock() {
                    *at = Some(Instant::now() + Duration::from_secs(5));
                }
            }
        }
    }
}
fn input_values(v: &Values) -> Values {
    Values(Value::Object(
        v.0.as_object()
            .expect("settings object")
            .iter()
            .filter(|(k, _)| k.as_str() != "acquisition")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    ))
}
/// Immutable applied options/bindings. Constructed on change, shared by hot
/// input/targeting readers without cloning the JSON document every poll.
pub fn current_shared() -> Arc<Values> {
    static DEFAULT: OnceLock<Arc<Values>> = OnceLock::new();
    GLOBAL.get().map_or_else(
        || DEFAULT.get_or_init(|| Arc::new(Values::default())).clone(),
        |s| s.input_shared(),
    )
}
pub fn current() -> Values {
    GLOBAL
        .get()
        .map_or_else(Values::default, |s| s.input_snapshot())
}
/// Change and save one option from outside the settings window.
pub fn set_option(key: &str, n: f64) {
    if let Some(s) = GLOBAL.get() {
        let mut v = s.snapshot();
        v.set(key, n);
        s.apply(v);
    }
}
/// One applied option without cloning the whole settings document.
pub fn option(key: &str) -> f64 {
    GLOBAL
        .get()
        .map_or_else(|| Values::default().number(key), |s| s.number(key))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_input_changes_only_on_apply_and_preserves_old_readers() {
        let store = Settings::new(None);
        let first = store.input_shared();
        assert!(Arc::ptr_eq(&first, &store.input_shared()));
        let mut v = store.snapshot();
        v.set("emotes", 0.);
        v.bind(
            "emote",
            0,
            Some(Chord {
                code: 0x55,
                mods: 0,
            }),
        );
        store.apply(v);
        let next = store.input_shared();
        assert!(!Arc::ptr_eq(&first, &next));
        assert_eq!(first.number("emotes"), 1.);
        assert_eq!(next.number("emotes"), 0.);
        assert_eq!(first.binding("emote")[0].unwrap().code, 0x54);
        assert_eq!(next.binding("emote")[0].unwrap().code, 0x55);
        assert!(next.0.get("acquisition").is_none());
    }
    #[test]
    fn saved_keybinds_options_and_unknown_fields_survive_reload() {
        let root = std::env::temp_dir().join(format!(
            "lt-settings-52-{}-{}",
            std::process::id(),
            crate::platform_input::thread_id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("controls.json"),
            br#"{"cursor_size":49,"low_health_effect":true,"other_mod":{"keep":3},"practice_mode":1.0,"practice_lineup":"harpy"}"#,
        )
        .unwrap();
        let store = Settings::new(Some(&root));
        let mut v = store.snapshot();
        v.bind(
            "q",
            0,
            Some(Chord {
                code: 0x45,
                mods: 0,
            }),
        );
        v.set("champion_mode", 1.);
        v.set("hover_outline", 0.);
        v.set("selection_debug", 1.);
        crate::acquisition::set_custom(&mut v, "workshop/hero", Some(135.5));
        store.apply(v);
        store.save().unwrap();
        let loaded = Settings::new(Some(&root));
        assert_eq!(loaded.size(), 49);
        assert!(loaded.low_health());
        // Hidden switches and texts (no control in the window) survive a save.
        assert!(loaded.hidden_switch("practice_mode"));
        assert!(!loaded.hidden_switch("not_there"));
        assert_eq!(loaded.text("practice_lineup").as_deref(), Some("harpy"));
        assert_eq!(loaded.number("champion_mode"), 1.);
        assert_eq!(loaded.number("hover_outline"), 0.);
        assert_eq!(loaded.number("selection_debug"), 1.);
        assert_eq!(
            crate::acquisition::custom(&loaded.snapshot(), "workshop/hero"),
            Some(135.5)
        );
        assert_eq!(
            loaded.acquisition_radius("workshop/hero", 160_000, Some(190_000)),
            160_000
        );
        assert_eq!(
            loaded.acquisition_radius("workshop/hero", 80_000, Some(190_000)),
            135_500
        );
        assert!(loaded.input_snapshot().0.get("acquisition").is_none());
        assert_eq!(
            loaded.snapshot().binding("q")[0],
            Some(Chord {
                code: 0x45,
                mods: 0
            })
        );
        assert_eq!(loaded.snapshot().0["other_mod"]["keep"], 3);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn modified_command_suppresses_plain_move() {
        let v = Values::default();
        let mut raw = [false; 256];
        raw[2] = true;
        assert!(v.pressed("move", &raw));
        raw[0x10] = true;
        assert!(!v.pressed("move", &raw));
        assert!(v.pressed("attack_click", &raw));
        raw[0x51] = true;
        assert!(!v.pressed("q", &raw));
        assert!(v.pressed("preview_q", &raw));
    }
    #[test]
    fn stat_rules_start_off_with_the_tested_defaults() {
        let fresh = Values(json!({"version": 1}));
        assert_eq!(fresh.number("athlete_stat_rules"), 0.);
        assert_eq!(fresh.number("stat_reaction_floor"), 12.);
        assert_eq!(fresh.number("stat_reaction_ceiling"), 35.);
        assert_eq!(fresh.number("stat_collision_margin"), 2.);
        // The sliders reach the base game's own floor and ceiling.
        for (key, value) in [
            ("stat_reaction_floor", crate::athlete_rules::VANILLA_FLOOR),
            (
                "stat_reaction_ceiling",
                crate::athlete_rules::VANILLA_CEILING,
            ),
        ] {
            let def = OPTIONS.iter().find(|o| o.key == key).unwrap();
            let Control::Slider(min, max, _, _) = def.control else {
                panic!("{key} is not a slider");
            };
            assert!((min..=max).contains(&f64::from(value)), "{key}");
        }
        // The window's defaults and the rules' own defaults agree.
        assert_eq!(
            fresh.number("stat_reaction_floor"),
            f64::from(crate::athlete_rules::FLOOR_DEFAULT)
        );
        assert_eq!(
            fresh.number("stat_reaction_ceiling"),
            f64::from(crate::athlete_rules::CEILING_DEFAULT)
        );
        assert_eq!(
            fresh.number("stat_collision_margin"),
            f64::from(crate::athlete_rules::MARGIN_DEFAULT)
        );
    }
    #[test]
    fn migration_unknown_and_new_defaults() {
        let v = Values(json!({"cursor_size":49,"low_health_effect":true,"other_mod":{"keep":3}}));
        assert_eq!(v.number("cursor_size"), 49.);
        assert_eq!(v.number("low_health_effect"), 1.);
        assert_eq!(v.number("champion_mode"), 0.);
        assert_eq!(v.number("hover_outline"), 1.);
        assert_eq!(v.number("selection_debug"), 0.);
        assert_eq!(v.0["other_mod"]["keep"], 3);
    }
    #[test]
    fn emote_volume_defaults_to_70_and_an_old_sound_switch_left_off_stays_silent() {
        assert_eq!(Values::default().number("emote_volume"), 70.);
        // Saved before the slider existed.
        let off = Values(json!({"version": 1, "emote_sound": 0.0}));
        assert_eq!(off.number("emote_volume"), 0.);
        let on = Values(json!({"version": 1, "emote_sound": 1.0}));
        assert_eq!(on.number("emote_volume"), 70.);
        // A value set with the slider wins over the old switch.
        let mut set = off.clone();
        set.set("emote_volume", 43.);
        assert_eq!(set.number("emote_volume"), 45.);
    }
    #[test]
    fn rebind_restore_and_validate() {
        let mut v = Values::default();
        let c = Chord {
            code: 0x57,
            mods: 0,
        };
        assert_eq!(v.conflicts(c, "q", 0), vec![("w", 0)]);
        v.bind("move", 0, None);
        assert!(!v.essential());
        v.reset_page(1);
        assert!(v.essential());
        v.set("pan_speed", 99.);
        assert_eq!(v.number("pan_speed"), 3.);
    }
    #[test]
    fn key_caps_show_short_key_names_with_modifier_prefixes() {
        let short = |code, mods| {
            let mut s = String::new();
            Chord { code, mods }.short(&mut s);
            s
        };
        for (code, mods, name) in [
            (0x51, 0, "Q"),
            (0x37, 0, "7"),
            (0x7a, 0, "F11"),
            (0x87, 0, "F24"),
            (1, 0, "M1"),
            (2, 0, "M2"),
            (4, 0, "M3"),
            (5, 0, "M4"),
            (6, 0, "M5"),
            (9, 0, "Tab"),
            (0x20, 0, "Spc"),
            (0x24, 0, "Home"),
            (0x23, 0, "End"),
            (8, 0, "Bksp"),
            (0x26, 0, "Up"),
            (0x28, 0, "Dn"),
            (0x25, 0, "Lft"),
            (0x27, 0, "Rgt"),
            (0x12, 0, "Alt"),
            (0xc0, 0, "`"),
            (0xba, 0, ";"),
            (0xdf, 0, "K223"),
            (0x0d, 0, "Ent"),
            (0x2e, 2, "C+Del"),
            (0x61, 0, "N1"),
            (0x42, 1, "S+B"),
            (0x42, 2, "C+B"),
            (0x42, 4, "A+B"),
            (0x7b, 7, "C+S+A+F12"),
        ] {
            assert_eq!(short(code, mods), name, "{code:#x} {mods}");
        }
        // The full names stay for the Settings window.
        assert_eq!(
            Chord {
                code: 0x42,
                mods: 1
            }
            .label(),
            "Shift + B"
        );
        assert_eq!(
            Chord {
                code: 0xba,
                mods: 6
            }
            .label(),
            "Ctrl + Alt + ;"
        );
        assert_eq!(
            Chord {
                code: 0x22,
                mods: 0
            }
            .label(),
            "Page Down"
        );
        // A cap shows the first bound chord; none when both are cleared.
        let mut v = Values::default();
        assert_eq!(
            v.shown_chord("shop"),
            Some(Chord {
                code: 0x50,
                mods: 0
            })
        );
        v.bind("shop", 0, None);
        assert_eq!(v.shown_chord("shop"), None);
        v.bind(
            "shop",
            1,
            Some(Chord {
                code: 0x42,
                mods: 1,
            }),
        );
        assert_eq!(
            v.shown_chord("shop"),
            Some(Chord {
                code: 0x42,
                mods: 1
            })
        );
    }
}
