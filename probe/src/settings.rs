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
        control: Control::Choice(&["Ignore mode", "Honor mode"]),
        default: 0.,
    },
    OptionDef {
        key: "attack_move_preference",
        page: 0,
        section: "Targeting",
        label: "Attack-move target preference",
        hint: "Choose which nearby enemy to acquire while moving.",
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
        key: "attack_cancel",
        page: 0,
        section: "Attacks",
        label: "Cancel attack wind-down",
        hint: "A move, stop or held skill after the hit cuts the swing short.",
        control: Control::Toggle,
        default: 1.,
    },
    OptionDef {
        key: "manual_shop",
        page: 0,
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
        key: "cursor_size",
        page: 3,
        section: "Cursor",
        label: "Cursor size",
        hint: "Changes the pointer size without moving its click point.",
        control: Control::Slider(24., 64., 1., " px"),
        default: crate::cursor::DEFAULT_SIZE as f64,
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
        key: "hover_outline",
        page: 3,
        section: "Battlefield feedback",
        label: "Sprite outlines",
        hint: "Hover and active attack target; blue allies, red hostiles.",
        control: Control::Toggle,
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
        key: "log_level",
        page: 3,
        section: "Debug",
        label: "Log detail",
        hint: "Verbose adds per-click and per-second traces for investigations.",
        control: Control::Choice(&["Quiet", "Normal", "Verbose"]),
        default: 1.,
    },
    OptionDef {
        key: "selection_debug",
        page: 3,
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
        }
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
    pub fn label(self) -> String {
        let key = match self.code {
            1 => "Mouse1".into(),
            2 => "Mouse2".into(),
            4 => "Mouse3".into(),
            5 => "Mouse4".into(),
            6 => "Mouse5".into(),
            8 => "Backspace".into(),
            9 => "Tab".into(),
            0x10 => "Shift".into(),
            0x11 => "Ctrl".into(),
            0x12 => "Alt".into(),
            0x20 => "Space".into(),
            0x24 => "Home".into(),
            0x23 => "End".into(),
            0x25 => "Left".into(),
            0x26 => "Up".into(),
            0x27 => "Right".into(),
            0x28 => "Down".into(),
            0xc0 => "`".into(),
            0x70..=0x87 => format!("F{}", self.code - 0x6f),
            0x30..=0x39 | 0x41..=0x5a => (self.code as char).to_string(),
            _ => format!("Key {}", self.code),
        };
        format!(
            "{}{}{}{key}",
            if self.mods & 2 != 0 { "Ctrl + " } else { "" },
            if self.mods & 1 != 0 { "Shift + " } else { "" },
            if self.mods & 4 != 0 { "Alt + " } else { "" }
        )
    }
}
pub struct Settings {
    values: Mutex<Values>,
    dirty: Mutex<Option<Instant>>,
    root: Option<PathBuf>,
}
impl Settings {
    pub fn new(root: Option<&Path>) -> Self {
        let value = root
            .and_then(|p| std::fs::read(p.join("controls.json")).ok())
            .filter(|b| b.len() <= 65536)
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({"version":1}));
        Self {
            values: Mutex::new(Values(value)),
            dirty: Mutex::default(),
            root: root.map(Path::to_owned),
        }
    }
    pub fn snapshot(&self) -> Values {
        self.values.lock().map(|v| v.clone()).unwrap_or_default()
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
            *saved = v;
            if let Ok(mut at) = self.dirty.lock() {
                *at = Some(Instant::now());
            }
        }
    }
    pub(crate) fn save(&self) -> std::io::Result<()> {
        let root = self
            .root
            .as_ref()
            .ok_or_else(|| std::io::Error::other("Settings folder unavailable"))?;
        std::fs::create_dir_all(root)?;
        let path = root.join("controls.json");
        let mut old = match std::fs::read(&path) {
            Ok(b) if b.len() <= 65536 => {
                serde_json::from_slice::<Value>(&b).map_err(std::io::Error::other)?
            }
            Ok(_) => return Err(std::io::Error::other("Existing controls.json too large")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
            Err(e) => return Err(e),
        };
        let object = old
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("Existing controls.json is not an object"))?;
        let current = self.snapshot();
        for (k, v) in current.0.as_object().unwrap() {
            object.insert(k.clone(), v.clone());
        }
        object.insert("version".into(), json!(1));
        object.insert("cursor_size".into(), json!(self.size()));
        object.insert("low_health_effect".into(), json!(self.low_health()));
        let tmp = root.join("controls.settings.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&old)?)?;
        std::fs::rename(tmp, path)
    }
    pub fn flush(&self, force: bool, log: &Logger) {
        let ready = self
            .dirty
            .lock()
            .is_ok_and(|at| at.is_some_and(|t| force || t.elapsed() >= Duration::from_millis(300)));
        if !ready {
            return;
        }
        match self.save() {
            Ok(()) => {
                if let Ok(mut at) = self.dirty.lock() {
                    *at = None
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
pub fn current() -> Values {
    GLOBAL.get().map_or_else(Values::default, |s| s.snapshot())
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
    fn saved_keybinds_options_and_unknown_fields_survive_reload() {
        let root = std::env::temp_dir().join(format!(
            "lt-settings-52-{}-{}",
            std::process::id(),
            crate::platform_input::thread_id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("controls.json"),
            br#"{"cursor_size":49,"low_health_effect":true,"other_mod":{"keep":3}}"#,
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
        store.apply(v);
        store.save().unwrap();
        let loaded = Settings::new(Some(&root));
        assert_eq!(loaded.size(), 49);
        assert!(loaded.low_health());
        assert_eq!(loaded.number("champion_mode"), 1.);
        assert_eq!(loaded.number("hover_outline"), 0.);
        assert_eq!(loaded.number("selection_debug"), 1.);
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
}
