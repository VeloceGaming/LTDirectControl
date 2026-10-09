//! Selected-player scalar snapshots and a scoped native UI overlay.
//! No host context or entity pointer is retained across callbacks.
use crate::{
    abilities::HudSkills,
    camera::Rect,
    hud_icons::{self, Icon},
    native_timing::MatchKey,
    Logger,
};
use mod_api_stable::{SettingTargetV1, StableClient};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

pub const PATH: &str = "ingame.lt_player_hud";
const KEYS: [&str; 3] = ["Q", "W", "R"];
const UNLOCK_LEVELS: [usize; 3] = [1, 3, 5];
// Native UI coordinates are 1920 x 1080, independent of window scaling.
const WIDTH: usize = 1920;
const HEIGHT: usize = 146;
const SKILL_X: usize = 832;
const SKILL_Y: usize = 4;
const SKILL_STEP: usize = 88;
const SKILL_SIZE: usize = 80;
const ITEM_Y: usize = 100;
const ITEM_SIZE: usize = 36;
pub static TOOLTIP_SCROLL: AtomicI32 = AtomicI32::new(0);
/// Overflow panel and its source tile, in screen coordinates, for wheel routing.
pub static TOOLTIP_SCROLL_AREA: Mutex<Option<(Rect, Rect)>> = Mutex::new(None);

fn death_countdown(alive: bool, ticks: usize) -> String {
    if alive {
        String::new()
    } else if ticks > 0 {
        format!("Respawning in {}", ticks.div_ceil(60))
    } else {
        "Respawning…".into()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub key: MatchKey,
    pub player: usize,
    pub champion: String,
    pub level: usize,
    pub hp: Option<(usize, usize)>,
    pub alive: bool,
    pub respawn: usize,
    pub gold: usize,
    pub kda: (usize, usize, usize),
    pub cs: usize,
    pub cooldowns: [usize; 3],
    pub items: Vec<String>,
    pub build: Option<Vec<String>>,
}

// Static artwork may outlive a telemetry gap, but never a selected identity.
// This deliberately contains no HP, gold, cooldown or respawn readings.
#[derive(Clone, Debug, PartialEq)]
pub struct Artwork {
    pub identity: (MatchKey, usize),
    pub champion: String,
    pub items: Vec<String>,
}

#[derive(Default)]
pub struct PlayerHud(
    Mutex<Option<(Snapshot, Instant)>>,
    Mutex<Option<(MatchKey, usize, usize)>>,
    Mutex<HashMap<(MatchKey, usize), Snapshot>>,
    Mutex<Vec<String>>,
    Mutex<Option<RegisteredItems>>,
);
struct RegisteredItems {
    key: MatchKey,
    read_at: Instant,
    items: Option<Arc<crate::native_items::Catalogue>>,
}
impl PlayerHud {
    pub fn capture_items(
        &self,
        ctx: &mod_api_stable::StableAiContext<'_>,
        key: MatchKey,
        log: &Logger,
    ) {
        let Ok(mut current) = self.4.lock() else {
            return;
        };
        if current.as_ref().is_some_and(|s| {
            s.key == key && (s.items.is_some() || s.read_at.elapsed() < Duration::from_secs(2))
        }) {
            return;
        }
        let first = current.as_ref().is_none_or(|s| s.key != key);
        let read = crate::native_items::read(ctx).and_then(|items| {
            if self.3.lock().is_ok_and(|keys| {
                !keys.is_empty() && keys.iter().all(|key| items.contains_key(key))
            }) {
                Ok(items)
            } else {
                Err("registered keys do not cover the SDK build catalogue".into())
            }
        });
        let failure = read.as_ref().err().cloned();
        let items = read.ok();
        if items.is_some() || first {
            log.write(&format!("PLAYER HUD registered items key={key:?} count={} failure={failure:?}; owned copies from live AI callback",items.as_ref().map_or(0,|i| i.len())));
        }
        *current = Some(RegisteredItems {
            key,
            read_at: Instant::now(),
            items: items.map(Arc::new),
        });
    }
    pub fn item_metadata(&self, key: MatchKey) -> Option<Arc<crate::native_items::Catalogue>> {
        self.4
            .lock()
            .ok()?
            .as_ref()
            .filter(|s| s.key == key)?
            .items
            .clone()
    }
    /// Registered item keys in engine order: the native buyer's item indices.
    pub fn catalog_keys(&self) -> Vec<String> {
        self.3.lock().map(|keys| keys.clone()).unwrap_or_default()
    }
    pub fn item_catalog(&self, keys: Vec<String>) {
        if let Ok(mut current) = self.3.lock() {
            *current = keys;
        }
    }
    pub fn build(&self, sim: &mod_api_stable::StableSim<'_>, player: usize) -> Option<Vec<String>> {
        let indices = crate::native_adapter::player_build(sim, player)?;
        let keys = self.3.lock().ok()?;
        indices
            .iter()
            .map(|index| keys.get(*index).cloned())
            .collect()
    }

    pub fn reset_session(&self) {
        if let Ok(mut snapshot) = self.0.lock() {
            *snapshot = None;
        }
        if let Ok(mut tick) = self.1.lock() {
            *tick = None;
        }
        if let Ok(mut prepared) = self.2.lock() {
            prepared.clear();
        }
        if let Ok(mut items) = self.4.lock() {
            *items = None;
        }
    }
    pub fn observe_prepared(&self, snapshot: Snapshot) {
        if let Ok(mut prepared) = self.2.lock() {
            prepared.insert((snapshot.key, snapshot.player), snapshot);
        }
    }
    pub fn select_prepared(&self, key: MatchKey, player: usize) {
        let snapshot = self
            .2
            .lock()
            .ok()
            .and_then(|p| p.get(&(key, player)).cloned());
        if let Ok(mut current) = self.0.lock() {
            *current = snapshot.map(|s| (s, Instant::now()));
        }
        if let Ok(mut tick) = self.1.lock() {
            *tick = None;
        }
    }
    pub fn needs_sample(&self, key: MatchKey, player: usize, tick: usize) -> bool {
        let due = tick.is_multiple_of(6)
            || self.0.lock().is_ok_and(|s| {
                s.as_ref()
                    .is_none_or(|(s, _)| s.key != key || s.player != player)
            });
        if !due {
            return false;
        }
        self.1.lock().is_ok_and(|mut at| {
            if *at == Some((key, player, tick)) {
                return false;
            }
            *at = Some((key, player, tick));
            true
        })
    }
    pub fn observe_tick(&self, snapshot: Snapshot, tick: usize, log: &Logger) {
        let transition = self
            .0
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(old, _)| old.alive != snapshot.alive));
        let build_changed = self.0.lock().is_ok_and(|s| {
            s.as_ref()
                .is_none_or(|(old, _)| old.build != snapshot.build || old.items != snapshot.items)
        });
        if tick == 6 || build_changed {
            log.write(&format!(
                "PLAYER HUD purchase data player={} build={:?} owned={:?} gold={} catalog_count={} tick={tick}",
                snapshot.player, snapshot.build, snapshot.items, snapshot.gold, self.3.lock().map_or(0, |keys| keys.len())
            ));
        }
        if transition == Some(true) {
            log.write(&format!(
                "PLAYER HUD life alive={} respawn_ticks={} tick={tick} player={} key={:?}",
                snapshot.alive, snapshot.respawn, snapshot.player, snapshot.key
            ));
        }
        self.observe(snapshot);
    }
    pub fn observe(&self, mut snapshot: Snapshot) {
        if let Ok(mut s) = self.0.lock() {
            // A dead player's entity may be absent. Retain its last known
            // champion label only for this same match/player identity.
            if snapshot.champion.is_empty() {
                if let Some((old, _)) = s
                    .as_ref()
                    .filter(|(old, _)| old.key == snapshot.key && old.player == snapshot.player)
                {
                    snapshot.champion = old.champion.clone();
                }
            }
            *s = Some((snapshot, Instant::now()));
        }
    }
    pub fn artwork(&self, identity: Option<(MatchKey, usize)>) -> Option<Artwork> {
        let (key, player) = identity?;
        let s = self.0.lock().ok()?;
        let (snapshot, _) = s.as_ref()?;
        (snapshot.key == key && snapshot.player == player).then(|| Artwork {
            identity: (key, player),
            champion: snapshot.champion.clone(),
            items: snapshot.items.clone(),
        })
    }
    pub fn snapshot(&self, identity: Option<(MatchKey, usize)>, running: bool) -> Option<Snapshot> {
        let (key, player) = identity?;
        let s = self.0.lock().ok()?;
        let (snapshot, at) = s.as_ref()?;
        if snapshot.key != key || snapshot.player != player {
            return None;
        }
        let fresh = at.elapsed() <= Duration::from_millis(250);
        if running && !fresh && snapshot.alive {
            return None;
        }
        let mut result = snapshot.clone();
        // Preserve identity/inventory if every actor has stopped calling think.
        // Do not extrapolate a respawn timer from wall time or claim stale HP.
        if running && !fresh {
            result.respawn = 0;
            result.hp = None;
        }
        Some(result)
    }
}

fn assets() -> &'static serde_json::Value {
    static ASSETS: OnceLock<serde_json::Value> = OnceLock::new();
    ASSETS.get_or_init(|| {
        serde_json::from_str(include_str!("hud_assets.json")).expect("embedded icon metadata")
    })
}
fn image(champion: &str, slot: usize) -> Option<(&'static str, &'static str)> {
    let value = assets().get("champions")?.get(champion)?;
    Some((
        value.get("source")?.as_str()?,
        value.get("tags")?.get(slot)?.as_str()?,
    ))
}
fn item(key: &str) -> Option<&'static serde_json::Value> {
    assets().get("items")?.get(key)
}
fn hp_percent(s: &Snapshot) -> f32 {
    if !s.alive {
        return 0.;
    }
    s.hp.filter(|(_, max)| *max > 0).map_or(0., |(hp, max)| {
        (hp.min(max) as f64 / max as f64 * 100.) as f32
    })
}
fn cooldown(ticks: usize) -> String {
    if ticks == 0 {
        "Ready".into()
    } else {
        format!("{:.1}s", (ticks as f64 / 6.).ceil() / 10.)
    }
}
fn skill_unavailable(s: &Snapshot, skills: HudSkills, slot: usize) -> bool {
    s.level < UNLOCK_LEVELS[slot] || !s.alive || skills.available[slot] == Some(false)
}
fn icon_updates(parent: &str, icon: Option<&Icon>, skill: bool) -> Vec<(String, String)> {
    let target = if skill && icon.is_some_and(|i| i.tag.is_none()) {
        "png"
    } else {
        "icon"
    };
    let nodes: &[&str] = if skill { &["icon", "png"] } else { &["icon"] };
    nodes
        .iter()
        .map(|node| {
            let source = if *node == target {
                icon.map_or("visible: false;".into(), |i| {
                    format!("visible: true; {}", i.properties())
                })
            } else {
                "visible: false;".into()
            };
            (format!("{parent}.{node}"), source)
        })
        .collect()
}
fn label(
    source: &mut String,
    id: &str,
    rect: (usize, usize, usize, usize),
    size: usize,
    color: &str,
) {
    let (x, y, w, h) = rect;
    let outline = if matches!(id, "hp_current" | "hp_max") {
        "outline: 1; outline_color: #0e0d0cff;"
    } else {
        ""
    };
    let align = if matches!(
        id,
        "cooldown" | "respawn" | "level" | "camera_key" | "target_key" | "recall_key"
    ) {
        "align_x: Center;"
    } else {
        ""
    };
    source.push_str(&format!("#{id}:label {{ @\"asset/base/style/main#bold_label\"; x: {x}px; y: {y}px; width: {w}px; height: {h}px; z: 1005; size: {size}; color: #{color}; align_y: Center; {align} {outline} ignore_event: true; }}\n"));
}
fn inventory_tile(i: usize) -> String {
    // Slot insets: background + 5.
    let inset = crate::ui_theme::shade(5, 0xff);
    format!("item{i}:color {{ x: 0px; y: 100px; width: 36px; height: 36px; z: 1001; visible: false; color: #~4b4a49ff; ignore_event: true; rounding: Uniform {{ rounding: 2; }} #background:color {{ x: 1px; y: 1px; width: 34px; height: 34px; z: 1002; color: #{inset}; ignore_event: true; }} #icon:image {{ x: 2px; y: 2px; width: 32px; height: 32px; z: 1003; sample_linear: false; ignore_event: true; visible: false; }} #unknown:label {{ @\"asset/base/style/main#label\"; width: 100%; height: 100%; z: 1005; size: 16; align_x: Center; align_y: Center; text: \"?\"; visible: false; ignore_event: true; }} }}\n")
}
fn template() -> String {
    let mut s = format!("lt_player_hud:empty {{ width: {WIDTH}px; height: {HEIGHT}px; anchor_x: 0.5; pivot_x: 0.5; anchor_y: 1; pivot_y: 1; z: 1000; ignore_event: true;\n");
    // Battlefield ends at y1024. Controls remain in the last 50 px.
    s.push_str(&"#strip:color { y: 90px; width: 1560px; height: 56px; z: 1000; color: #1c1a18ff; ignore_event: true; }\n#combat:empty { x: 832px; y: 4px; width: 256px; height: 80px; ignore_event: true; }\n#edge:color { y: 90px; width: 1560px; height: 1px; z: 1001; color: #~4b4a49ff; ignore_event: true; }\n".replace("#1c1a18ff", &format!("#{}", crate::ui_theme::hex(0xff))));
    // Session controls (Pause, AI, Camera, Settings, vision group) occupy x 8-350
    // with their own dividers; K/D/A, CS and Tab follow, then B recall beside HP.
    for x in [0, 1544] {
        s.push_str(&format!("#corner{x}:color {{ x: {x}px; y: 90px; width: 16px; height: 2px; z: 1002; color: #cbc9c7ff; ignore_event: true; }}\n"));
    }
    for (id, glyph, x) in [
        ("kda_icon", "ef_swords", 356),
        ("cs_label", "hud_minion", 510),
    ] {
        s.push_str(&crate::ui_graphics::image(id, glyph, x, 105, 26, 1003));
    }
    for (id, x, w, size, color) in [
        ("kills", 386, 30, 24, "ffffffff"),
        ("slash1", 416, 10, 24, "989694ff"),
        ("deaths", 426, 30, 24, "ff642eff"),
        ("slash2", 456, 10, 24, "989694ff"),
        ("assists", 466, 30, 24, "ffffffff"),
        ("cs", 538, 50, 24, "ffffffff"),
    ] {
        label(&mut s, id, (x, 102, w, 32), size, color);
    }
    label(&mut s, "level", (730, 100, 40, 36), 24, "ffffffff");
    s.push_str("#health:color { x: 780px; y: 106px; width: 360px; height: 24px; z: 1001; color: #~0e0d0cff; ignore_event: true; #trail:color { width: 0%; height: 100%; z: 1002; color: #efe6cfff; ignore_event: true; } #fill:color { width: 0%; height: 100%; z: 1003; color: #2e7d46ff; ignore_event: true; } }\n");
    for i in 0..32 {
        s.push_str(&format!("#hp_tick{i}:color {{ x: 780px; y: 106px; width: 1px; height: 24px; z: 1004; color: #00000073; visible: false; ignore_event: true; }}\n"));
    }
    label(&mut s, "hp_current", (944, 106, 84, 24), 18, "ffffffff");
    label(&mut s, "hp_max", (1030, 106, 104, 24), 18, "ffffffff");
    s.push_str(&crate::ui_graphics::image(
        "hourglass",
        "ef_skull",
        919,
        106,
        22,
        1005,
    ));
    label(&mut s, "respawn", (953, 106, 56, 24), 22, "ffffffff");
    s.push_str("#death_banner:color { x: 810px; y: -844px; width: 300px; height: 52px; z: 1120; color: #~1e1e1df0; visible: false; ignore_event: true; rounding: Uniform { rounding: 2; } #text:label { @\"asset/base/style/main#bold_label\"; width: 100%; height: 100%; z: 1121; size: 26; align_x: Center; align_y: Center; color: #ffffffff; ignore_event: true; } }\n");
    let inset = crate::ui_theme::shade(5, 0xff);
    for (i, key) in KEYS.iter().enumerate() {
        let x = SKILL_X + i * SKILL_STEP;
        s.push_str(&format!("#skill{i}:color {{ x: {x}px; y: {SKILL_Y}px; width: 80px; height: 80px; z: 1001; color: #~4b4a49ff; ignore_event: true; rounding: Uniform {{ rounding: 2; }} #shadow:color {{ x: 0px; y: 3px; width: 80px; height: 80px; z: 1000; color: #00000066; ignore_event: true; }} #background:color {{ x: 1px; y: 1px; width: 78px; height: 78px; z: 1002; color: #{inset}; ignore_event: true; }} #icon:image {{ x: 2px; y: 2px; width: 74px; height: 74px; z: 1003; sample_linear: false; ignore_event: true; visible: false; }} #png:image {{ x: 2px; y: 2px; width: 74px; height: 74px; z: 1003; sample_linear: false; ignore_event: true; visible: false; }} #shade:color {{ x: 1px; y: 1px; width: 78px; height: 78px; z: 1004; color: #0a0908cc; visible: false; ignore_event: true; }} #drain_edge:color {{ x: 1px; y: 1px; width: 78px; height: 2px; z: 1005; color: #ffffffff; visible: false; ignore_event: true; }} #keycap:color {{ x: 3px; y: 3px; width: 22px; height: 22px; z: 1005; color: #eeececff; ignore_event: true; rounding: Uniform {{ rounding: 2; }} }} "));
        label(&mut s, "cooldown", (2, 27, 76, 30), 27, "ffffffff");
        label(&mut s, "uses", (56, 4, 20, 22), 19, "fdee00ff");
        s.push_str(&crate::ui_graphics::image("lock", "lock", 28, 28, 24, 1005));
        s.push_str(&format!("#key:label {{ @\"asset/base/style/main#bold_label\"; x: 3px; y: 3px; width: 22px; height: 22px; z: 1006; size: 14; align_x: Center; align_y: Center; color: #393939ff; ignore_event: true; text: {key:?}; }} }}\n"));
    }
    for i in 0..6 {
        s.push('#');
        s.push_str(&inventory_tile(i));
    }
    let next = String::from("#next:color { x: 1390px; y: 100px; width: 36px; height: 36px; z: 1001; color: #989694ff; ignore_event: true; #background:color { x: 1px; y: 1px; width: 34px; height: 34px; z: 1002; color: #141311ff; ignore_event: true; } #icon:image { x: 3px; y: 3px; width: 30px; height: 30px; z: 1003; sample_linear: false; ignore_event: true; visible: false; } #progress:color { y: 40px; width: 36px; height: 3px; z: 1001; color: #ffffff26; ignore_event: true; #fill:color { width: 0%; height: 100%; z: 1002; color: #fdee00ff; ignore_event: true; } } }\n");
    let mut next = next.strip_suffix("}\n").unwrap().to_owned();
    for side in 0..4 {
        for j in 0..4 {
            let t = 6 + j * 8;
            let (x, y, w, h) = match side {
                0 => (t, 0, 4, 1),
                1 => (t, 35, 4, 1),
                2 => (0, t, 1, 4),
                _ => (35, t, 1, 4),
            };
            let bg = crate::ui_theme::hex(0xff);
            next.push_str(&format!("#gap{side}_{j}:color {{ x: {x}px; y: {y}px; width: {w}px; height: {h}px; z: 1002; color: #{bg}; ignore_event: true; }}\n"));
        }
    }
    next.push_str("}\n");
    s.push_str(&next);
    s.push_str(&crate::ui_graphics::image(
        "next_unavailable",
        "ef_unavailable",
        1398,
        108,
        20,
        1004,
    ));

    s.push_str(&crate::ui_graphics::image(
        "coin", "ef_coin", 1430, 99, 17, 1003,
    ));
    label(&mut s, "gold", (1450, 94, 58, 26), 21, "fdee00ff");
    // Shop button: the whole purchase column (next item, gold, required gold
    // and the key cap) is one hit area that opens the shop.
    s.push_str("#shop_hit:color { x: 1386px; y: 90px; width: 148px; height: 52px; z: 1001; color: #00000000; ignore_event: true; rounding: Uniform { rounding: 2; } }\n");
    s.push_str("#shop_key:color { x: 1512px; y: 97px; width: 18px; height: 20px; z: 1002; color: #eeececff; ignore_event: true; rounding: Uniform { rounding: 2; } #text:label { @\"asset/base/style/main#bold_label\"; width: 100%; height: 100%; size: 12; align_x: Center; align_y: Center; text: \"P\"; color: #393939ff; z: 1003; ignore_event: true; } }\n");
    s.push_str(&crate::ui_graphics::image(
        "purchase_coin",
        "ef_coin",
        1430,
        121,
        17,
        1003,
    ));
    s.push_str(&crate::ui_graphics::image(
        "purchase_check",
        "ef_check",
        1430,
        121,
        17,
        1003,
    ));
    label(&mut s, "purchase", (1450, 117, 58, 23), 16, "cbc9c7ff");
    s.push_str("#recall_key:color { x: 682px; y: 108px; width: 18px; height: 20px; z: 1002; color: #eeececff; ignore_event: true; rounding: Uniform { rounding: 2; } #text:label { @\"asset/base/style/main#bold_label\"; width: 100%; height: 100%; size: 12; align_x: Center; align_y: Center; text: \"B\"; color: #393939ff; z: 1003; ignore_event: true; } }\n");
    s.push_str(&crate::ui_graphics::image(
        "recall_icon",
        "ef_recall",
        702,
        109,
        20,
        1003,
    ));
    s.push_str("#feedback:color { x: 700px; y: -42px; width: 520px; height: 30px; z: 1112; color: #~1e1e1df0; visible: false; ignore_event: true; #text:label { @\"asset/base/style/main#label\"; x: 10px; width: 500px; height: 30px; z: 1113; size: 16; color: #cbc9c7ff; ignore_event: true; } }\n");
    s.push_str(r##"#tooltip:color { x: 730px; y: -300px; width: 529px; height: 173px; z: 1110; color: #~1e1e1dd9; visible: false; ignore_event: true; rounding: Uniform { rounding: 8; }
        #art:color { x: 16px; y: 24px; width: 48px; height: 48px; z: 1111; color: #00000000; ignore_event: true; #icon:image { width: 48px; height: 48px; z: 1112; visible: false; ignore_event: true; sample_linear: false; } #png:image { width: 48px; height: 48px; z: 1112; visible: false; ignore_event: true; sample_linear: false; } }
        #title:label { @"asset/base/style/main#label"; x: 76px; y: 23px; width: 419px; height: 32px; z: 1112; size: 27; line_height: 32; align_y: Top; color: #ffffffff; ignore_event: true; }
        #meta:label { @"asset/base/style/main#label"; x: 100px; y: 61px; width: 150px; height: 26px; z: 1112; size: 20; color: #d6d6d675; ignore_event: true; }
        #range:label { @"asset/base/style/main#label"; x: 208px; y: 61px; width: 100px; height: 26px; z: 1112; size: 20; color: #d6d6d675; ignore_event: true; }
        #key:label { @"asset/base/style/main#label"; x: 487px; y: 8px; width: 26px; height: 26px; z: 1112; size: 21; color: #d6d6d66b; ignore_event: true; }
        #meta_clock:image { x: 76px; y: 61px; width: 18px; height: 18px; source: "asset/lt_direct_control/ui/ef_clock"; color: #d6d6d675; z: 1112; ignore_event: true; }
        #meta_range:image { x: 180px; y: 61px; width: 18px; height: 18px; source: "asset/lt_direct_control/ui/ef_range"; color: #d6d6d675; z: 1112; ignore_event: true; }
        #status:image { x: 16px; y: 16px; width: 24px; height: 24px; source: "asset/lt_direct_control/ui/ef_unavailable"; z: 1112; visible: false; ignore_event: true; }
        #rule:color { x: 16px; y: 98px; width: 501px; height: 1px; z: 1111; color: #ffffff3b; ignore_event: true; }
        #text:label { @"asset/base/style/main#label"; x: 16px; y: 106px; width: 501px; height: 100px; z: 1111; size: 20; line_height: 28; align_y: Top; color: #d6d6d6ff; ignore_event: true; }
        #scroll_track:color { width: 3px; z: 1112; color: #ffffff30; visible: false; ignore_event: true; }
        #scroll_thumb:color { width: 3px; z: 1113; color: #d6d6d6aa; visible: false; ignore_event: true; }
    } }"##);
    crate::hud_style::fonts(s)
}

#[derive(Clone, PartialEq)]
struct PurchaseInputs {
    key: MatchKey,
    player: usize,
    gold: usize,
    items: Vec<String>,
    build: Option<Vec<String>>,
    metadata_count: usize,
    /// Shop queue while Manual shopping applies; None for the native buyer.
    queue: Option<Vec<usize>>,
}
#[derive(Default)]
pub struct HudUi {
    cache: HashMap<String, String>,
    failed: bool,
    spawn_attempted: Option<Instant>,
    custom_icons: Option<hud_icons::EnabledAssets>,
    live_items: HashMap<String, serde_json::Value>,
    registered_items: Option<Arc<crate::native_items::Catalogue>>,
    item_identity: Option<(MatchKey, usize)>,
    playing: bool,
    allocated_slots: usize,
    inventory_count: Option<usize>,
    inventory_report: Option<((MatchKey, usize), usize)>,
    purchase_tooltip: String,
    purchase_icon: Option<Icon>,
    purchase_ready: bool,
    purchase_price: Option<u64>,
    purchase_inputs: Option<PurchaseInputs>,
    item_read: Option<Instant>,
    hover: Option<(String, Instant)>,
    tooltip_text: HashMap<String, String>,
    tooltip_source: Option<(bool, usize)>,
    tooltip_layout: crate::tooltip_layout::Layout,
    motion: crate::hud_motion::Motion,
    cooldown_peak: [usize; 3],
    motion_identity: Option<(MatchKey, usize)>,
    shop_pressed: bool,
    shop_click: bool,
    /// Manual shopping with an empty queue: no next purchase is shown.
    purchase_blank: bool,
}
impl HudUi {
    /// A completed click on the strip's shop button since the last call.
    pub fn take_shop_click(&mut self) -> bool {
        std::mem::take(&mut self.shop_click)
    }
    fn tile_hover(&self, ctx: &StableClient<'_>, node: &str, cursor: Option<(f32, f32)>) -> bool {
        let Some(point) = cursor else { return false };
        let Some((x, y, w, h)) = ctx.ui_node_rect(PATH) else {
            return false;
        };
        let (left, top, size) = if let Some(i) = node
            .strip_prefix("skill")
            .and_then(|v| v.parse::<usize>().ok())
        {
            (
                (SKILL_X + i * SKILL_STEP) as f32,
                SKILL_Y as f32,
                SKILL_SIZE as f32,
            )
        } else if node == "next" {
            (1390., ITEM_Y as f32, 36.)
        } else if let Some(i) = node
            .strip_prefix("item")
            .and_then(|v| v.parse::<usize>().ok())
        {
            {
                let count = self.inventory_count.unwrap_or(0);
                if i >= count {
                    return false;
                }
                let (x, y) = crate::inventory::hud_slot(count, i);
                (x as f32, y as f32, ITEM_SIZE as f32)
            }
        } else {
            return false;
        };
        Rect {
            x: x + left * w / WIDTH as f32,
            y: y + top * h / HEIGHT as f32,
            w: size * w / WIDTH as f32,
            h: size * h / HEIGHT as f32,
        }
        .contains(point)
    }
    pub fn hovered_skill(
        &self,
        ctx: &StableClient<'_>,
        cursor: Option<(f32, f32)>,
    ) -> Option<usize> {
        if !self.playing || ctx.ui_visible(PATH) != Some(true) {
            return None;
        }
        let point = cursor?;
        (0..3).find(|slot| {
            let path = format!("{PATH}.skill{slot}");
            ctx.ui_visible(&path) == Some(true)
                && self.tile_hover(ctx, &format!("skill{slot}"), Some(point))
        })
    }
    fn load_icons(&mut self, log: &Logger) {
        if self.custom_icons.is_none() {
            let icons = hud_icons::enabled_assets();
            log.write(&format!(
                "PLAYER HUD enabled custom PNG champions={} overridden item tags={}",
                icons.champions.len(),
                icons.item_tags.len()
            ));
            self.custom_icons = Some(icons);
        }
    }
    fn refresh_assets(
        &mut self,
        ctx: &StableClient<'_>,
        snapshot: &Snapshot,
        hud: &PlayerHud,
        log: &Logger,
    ) {
        self.load_icons(log);
        let identity = (snapshot.key, snapshot.player);
        if self.item_identity != Some(identity) {
            self.item_identity = Some(identity);
            self.inventory_count = None;
            self.live_items.clear();
            self.registered_items = None;
            self.tooltip_text.clear();
            self.purchase_inputs = None;
            self.item_read = None;
        }
        if let Some(items) = hud.item_metadata(snapshot.key) {
            if self
                .registered_items
                .as_ref()
                .is_none_or(|old| !Arc::ptr_eq(old, &items))
            {
                self.live_items = (*items).clone();
                self.tooltip_text.clear();
                self.purchase_inputs = None;
                self.registered_items = Some(items);
                log.write(&format!(
                    "PLAYER HUD effective registered metadata count={}",
                    self.live_items.len()
                ));
            }
            return;
        }
        let missing = snapshot
            .items
            .iter()
            .chain(snapshot.build.iter().flatten())
            .any(|key| !self.live_items.contains_key(key));
        if self
            .item_read
            .is_some_and(|at| !missing || at.elapsed() < Duration::from_secs(2))
        {
            return;
        }
        self.item_read = Some(Instant::now());
        let table = ctx
            .setting_get_json(SettingTargetV1::ItemSetting, "")
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
        if let Some(table) = table.and_then(|v| v.as_object().cloned()) {
            let mut entries = HashMap::new();
            for (name, mut spec) in table {
                if !spec.is_object() {
                    continue;
                }
                spec["name"] = name.clone().into();
                if let Some(key) = spec.get("key").and_then(serde_json::Value::as_str) {
                    entries.insert(key.to_owned(), spec.clone());
                }
                entries.insert(name, spec);
            }
            if entries != self.live_items {
                self.tooltip_text.clear();
                self.purchase_inputs = None;
                log.write(&format!(
                    "PLAYER HUD active item metadata aliases={}",
                    entries.len()
                ));
            }
            self.live_items = entries;
        }
    }
    fn skill_icon(&self, champion: &str, slot: usize) -> Option<Icon> {
        self.custom_icons
            .as_ref()
            .and_then(|c| c.champions.get(champion))
            .and_then(|icons| icons.get(slot))
            .cloned()
            .or_else(|| {
                image(champion, slot).map(|(source, tag)| Icon {
                    source: source.into(),
                    tag: Some(tag.into()),
                })
            })
    }
    fn item_spec(&self, key: &str) -> Option<&serde_json::Value> {
        self.live_items.get(key).or_else(|| item(key))
    }
    /// Localized item text, as the strip's item tooltip shows it: title line,
    /// stat lines and the item's effect (with inline stat icons).
    pub(crate) fn item_text(&self, ctx: &StableClient<'_>, key: &str) -> String {
        crate::tooltips::item(ctx, key, self.item_spec(key))
    }
    pub(crate) fn item_icon(&self, key: &str) -> Option<Icon> {
        self.item_spec(key)
            .and_then(hud_icons::item_icon)
            .or_else(|| {
                self.custom_icons
                    .as_ref()
                    .filter(|c| c.item_tags.contains(key))
                    .map(|_| Icon {
                        source: "asset/base/aseprite_resources/ingame/item_icons_18x18".into(),
                        tag: Some(key.into()),
                    })
            })
    }
    fn icon(
        &mut self,
        ctx: &mut StableClient<'_>,
        parent: &str,
        icon: Option<Icon>,
        skill: bool,
        log: &Logger,
    ) {
        // All image children are built with the HUD once. Sheet and PNG
        // skill images use separate permanent nodes; no remove/spawn here.
        for (node, properties) in icon_updates(parent, icon.as_ref(), skill) {
            let key = format!("properties:{node}");
            if crate::hud_motion::unchanged(&self.cache, &format!("{PATH}.{node}"), &properties) {
                continue;
            }
            self.props(ctx, &node, properties.clone(), log);
            let applied = self.cache.get(&key) == Some(&properties);
            let path = format!("{PATH}.{node}");
            log.write(&format!("PLAYER HUD stable image node={node} applied={applied} exists={} rect={:?} source={properties}",ctx.ui_exists(&path),ctx.ui_node_rect(&path)));
        }
    }
    fn inventory_layout(
        &mut self,
        ctx: &mut StableClient<'_>,
        count: usize,
        identity: Option<(MatchKey, usize)>,
        log: &Logger,
    ) {
        for i in self.allocated_slots..count {
            if !ctx.ui_spawn_source(
                PATH,
                &crate::ui_theme::themed(&crate::hud_style::fonts(inventory_tile(i))),
            ) {
                log.write(&format!("INVENTORY slot spawn rejected index={i}"));
                break;
            }
            self.allocated_slots = i + 1;
        }
        for i in 0..self.allocated_slots {
            let visible = i < count && self.playing;
            let props = if visible {
                let (x, y) = crate::inventory::hud_slot(count, i);
                format!("visible: true; x: {x}px; y: {y}px;")
            } else {
                "visible: false;".into()
            };
            self.props(ctx, &format!("item{i}"), props, log);
        }
        self.inventory_count = Some(count);
        if let Some(identity) = identity {
            if self.inventory_report != Some((identity, count)) {
                log.write(&format!("INVENTORY HUD identity={identity:?} slots={count}; native target vector/owned lower bound"));
                self.inventory_report = Some((identity, count));
            }
        }
    }
    fn props(&mut self, ctx: &mut StableClient<'_>, node: &str, source: String, log: &Logger) {
        let key = format!("properties:{node}");
        if crate::hud_motion::properties(ctx, &mut self.cache, &format!("{PATH}.{node}"), &source) {
            self.cache.insert(key, source);
        } else if !self.failed {
            log.write(&format!("PLAYER HUD property failed node={node}"));
            self.failed = true;
        }
    }
    fn text(&mut self, ctx: &mut StableClient<'_>, node: &str, text: String, log: &Logger) {
        let key = format!("text:{node}");
        if self.cache.get(&key) == Some(&text) {
            return;
        }
        if ctx.ui_set_text(&format!("{PATH}.{node}"), &text) {
            self.cache.insert(key, text);
        } else if !self.failed {
            log.write(&format!("PLAYER HUD text failed node={node}"));
            self.failed = true;
        }
    }
    fn purchase(&mut self, ctx: &mut StableClient<'_>, snapshot: Option<&Snapshot>, log: &Logger) {
        // Manual shopping: the next purchase follows the player's shop queue.
        let shop = crate::shop::SHOP.view().filter(|v| v.manual);
        let inputs = snapshot.map(|s| PurchaseInputs {
            key: s.key,
            player: s.player,
            gold: s.gold,
            items: s.items.clone(),
            build: s.build.clone(),
            metadata_count: self.live_items.len(),
            queue: shop.as_ref().map(|v| v.queue.clone()),
        });
        if inputs.is_some() && inputs == self.purchase_inputs {
            return;
        }
        // Log once per inventory/build/metadata change, never on gold ticks.
        let trace = inputs.as_ref().is_some_and(|new| {
            self.purchase_inputs.as_ref().is_none_or(|old| {
                new.key != old.key
                    || new.player != old.player
                    || new.items != old.items
                    || new.build != old.build
                    || new.metadata_count != old.metadata_count
            })
        });
        self.purchase_inputs = inputs;
        let metadata = crate::purchase_tracker::metadata(&self.live_items);
        let name = |key: &str| {
            crate::tooltips::translated(ctx, &format!("#asset/base/text/item?{key}.name"))
                .unwrap_or_else(|| {
                    metadata
                        .get(key)
                        .map_or(key, |i| i.name.as_str())
                        .replace('_', " ")
                })
        };
        let forecast = snapshot.and_then(|s| {
            s.build
                .as_ref()
                .and_then(|b| crate::purchase_tracker::forecast(&metadata, b, &s.items, s.gold))
        });
        self.purchase_price = forecast
            .as_ref()
            .and_then(|f| f.next.as_ref())
            .map(|n| n.price as u64);
        let unavailable = forecast.is_none();
        if trace {
            if let Some(s) = snapshot {
                let mut relevant = std::collections::BTreeSet::new();
                relevant.extend(s.items.iter().cloned());
                relevant.extend(s.build.iter().flatten().cloned());
                // Include connected components, capped to this catalogue.
                for _ in 0..16 {
                    let before = relevant.len();
                    for (key, item) in &metadata {
                        if relevant.contains(key) || item.next.iter().any(|n| relevant.contains(n))
                        {
                            relevant.insert(key.clone());
                        }
                    }
                    if relevant.len() == before {
                        break;
                    }
                }
                let chain = relevant
                    .iter()
                    .map(|key| match metadata.get(key) {
                        Some(i) => format!(
                            "{key}:price={},tier={},enabled={},next={:?}",
                            i.price, i.tier, i.enabled, i.next
                        ),
                        None => format!("{key}:MISSING"),
                    })
                    .collect::<Vec<_>>();
                log.write(&format!("PURCHASE TRACE player={} build={:?} owned={:?} gold={} forecast={forecast:?} chain={chain:?}",s.player,s.build,s.items,s.gold));
            }
        }
        let manual = shop.as_ref().zip(snapshot).map(|(v, s)| {
            let vanilla = crate::settings::option("shop_vanilla_order") == 1.;
            let (target, step) = crate::shop::upcoming(&v.cat, &v.live, &v.orders, vanilla)?;
            let next = v.cat[step.item()].key.clone();
            let price = v.cat[step.item()].price;
            Some((v.cat[target].key.clone(), next, price, s.gold))
        });
        self.purchase_blank = matches!(manual, Some(None));
        let (icon, text, progress, tooltip) = match (snapshot, forecast) {
            _ if self.purchase_blank => (
                None,
                String::new(),
                0.,
                "Manual shopping: nothing queued. Open the shop (P) to buy.".into(),
            ),
            _ if manual.is_some() => {
                let (target, next, price, gold) = manual.clone().flatten().expect("queued step");
                let short = price.saturating_sub(gold);
                let tooltip = format!(
                    "{}\nNext: {} · {price} gold\n{}",
                    name(&target),
                    name(&next),
                    if short == 0 {
                        "Buys at base".into()
                    } else {
                        format!("{short} more gold")
                    }
                );
                let progress = if price == 0 {
                    100.
                } else {
                    (gold as f64 / price as f64 * 100.).min(100.) as f32
                };
                let text = if short == 0 {
                    "0".into()
                } else {
                    format!("−{short}")
                };
                (self.item_icon(&next), text, progress, tooltip)
            }
            (Some(s), Some(f)) if !f.completed => {
                let key = f
                    .next
                    .as_ref()
                    .map_or(f.target.as_str(), |n| n.key.as_str());
                let text = purchase_requirement(&f, s.gold);
                let progress = f.next.as_ref().map_or(0., |n| {
                    if n.price == 0 {
                        100.
                    } else {
                        (s.gold as f64 / n.price as f64 * 100.).min(100.) as f32
                    }
                });
                let mut tooltip = format!("{}\n", name(&f.target));
                if let Some(next) = &f.next {
                    tooltip.push_str(&format!(
                        "Next: {} · {} gold\n{} more gold\n",
                        name(&next.key),
                        next.price,
                        next.price.saturating_sub(s.gold)
                    ));
                }
                if !f.affordable.is_empty() {
                    tooltip.push_str("At base now:\n");
                    for p in &f.affordable {
                        tooltip.push_str(&format!("{} · {} gold\n", name(&p.key), p.price));
                    }
                }
                if f.branching {
                    tooltip.push_str("The game chooses an upgrade branch when buying:\n");
                    tooltip.push_str("A + after the HUD amount means the minimum shortfall; the chosen branch may cost more.\n");
                    for p in &f.alternatives {
                        tooltip.push_str(&format!(
                            "{} · {} gold · {} more gold\n",
                            name(&p.key),
                            p.price,
                            p.price.saturating_sub(s.gold)
                        ));
                    }
                    tooltip.push_str("Further purchases are not yet fixed.");
                }
                (self.item_icon(key), text, progress, tooltip)
            }
            (Some(_), Some(_)) => (None, "✓".into(), 100., "Automatic build complete".into()),
            _ => (
                None,
                "—".into(),
                0.,
                "Automatic purchase information is not available yet.".into(),
            ),
        };
        self.purchase_icon = icon.clone();
        let unavailable = unavailable && manual.is_none();
        self.props(
            ctx,
            "next_unavailable",
            format!("visible: {unavailable};"),
            log,
        );
        self.icon(ctx, "next", icon, false, log);
        let ready = text == "0" || text == "✓";
        self.purchase_ready = ready;
        for side in 0..4 {
            for j in 0..4 {
                self.props(
                    ctx,
                    &format!("next.gap{side}_{j}"),
                    format!("visible: {};", !ready),
                    log,
                );
            }
        }

        self.props(
            ctx,
            "next",
            format!("color: #{};", if ready { "fdee00ff" } else { "989694ff" }),
            log,
        );
        let blank = self.purchase_blank;
        self.props(
            ctx,
            "purchase_coin",
            format!("visible: {};", !ready && !blank),
            log,
        );
        self.props(
            ctx,
            "purchase_check",
            format!("visible: {};", ready && !blank),
            log,
        );
        self.props(
            ctx,
            "purchase",
            format!(
                "x: {}px; width: {}px; color: #{};",
                1450,
                58,
                if ready { "fdee00ff" } else { "cbc9c7ff" }
            ),
            log,
        );
        self.text(ctx, "purchase", text, log);
        self.props(
            ctx,
            "next.progress.fill",
            format!("width: {progress:.2}%;"),
            log,
        );
        self.purchase_tooltip = tooltip;
    }
    fn tooltip(
        &mut self,
        ctx: &mut StableClient<'_>,
        s: &Snapshot,
        cursor: Option<(f32, f32)>,
        log: &Logger,
    ) {
        let hovered = cursor.and_then(|cursor| {
            (0..3)
                .map(|i| (true, i))
                .chain((0..s.items.len().min(self.allocated_slots)).map(|i| (false, i)))
                .chain(std::iter::once((false, crate::inventory::PURCHASE)))
                .find(|(skill, i)| {
                    let node = if *skill {
                        format!("skill{i}")
                    } else if *i == crate::inventory::PURCHASE {
                        "next".into()
                    } else {
                        format!("item{i}")
                    };
                    self.tile_hover(ctx, &node, Some(cursor))
                })
        });
        // Keep the card open when entering it to read or scroll. The small
        // bridge below it covers the gap from the originating HUD tile.
        let hovered = hovered.or_else(|| {
            let (skill, i) = self.tooltip_source?;
            if !skill
                && i != crate::inventory::PURCHASE
                && i >= s.items.len().min(self.allocated_slots)
            {
                return None;
            }
            let (x, y, w, h) = ctx.ui_node_rect(&format!("{PATH}.tooltip"))?;
            (ctx.ui_visible(&format!("{PATH}.tooltip")) == Some(true)
                && cursor.is_some_and(|p| {
                    Rect {
                        x,
                        y,
                        w,
                        h: h + 24.,
                    }
                    .contains(p)
                }))
            .then_some((skill, i))
        });
        let mut notches = TOOLTIP_SCROLL.swap(0, Ordering::Relaxed);
        let Some((skill, i)) = hovered else {
            self.hover = None;
            self.tooltip_source = None;
            self.props(ctx, "tooltip", "visible: false;".into(), log);
            return;
        };
        let key = if skill {
            format!("skill:{}:{i}", s.champion)
        } else if i == crate::inventory::PURCHASE {
            "purchase".into()
        } else {
            format!("item:{}", s.items[i])
        };
        if self.hover.as_ref().is_none_or(|(old, _)| old != &key) {
            self.hover = Some((key.clone(), Instant::now()));
            self.tooltip_layout.reset_scroll();
            notches = 0;
        }
        self.tooltip_source = Some((skill, i));
        if key == "purchase" {
            self.tooltip_text
                .insert(key.clone(), self.purchase_tooltip.clone());
        }
        // Native results arrive on the next viewer callback. Refresh eligible
        // skill text even while hovered; item and other champion caches stay put.
        if !self.tooltip_text.contains_key(&key)
            || (skill && crate::native_tooltips::enabled_for(&s.champion))
        {
            let text = if skill {
                let spec = self
                    .custom_icons
                    .as_ref()
                    .and_then(|a| a.descriptions.get(&s.champion))
                    .or_else(|| crate::tooltips::base_champion(&s.champion));
                crate::tooltips::skill(ctx, &s.champion, i, spec)
            } else {
                crate::tooltips::item(ctx, &s.items[i], self.item_spec(&s.items[i]))
            };
            if self.tooltip_text.get(&key) != Some(&text) {
                log.write(&format!(
                    "HUD TOOLTIP key={key} chars={}",
                    text.chars().count()
                ));
                self.tooltip_text.insert(key.clone(), text);
            }
        }
        let full = self.tooltip_text[&key].clone();
        let (raw_title, body) = full.split_once('\n').unwrap_or((&full, ""));
        let body = body.trim();
        let title = if skill {
            raw_title
                .strip_prefix(&format!("{}   ", KEYS[i]))
                .unwrap_or(raw_title)
        } else {
            raw_title
        };
        let fade = self.hover.as_ref().map_or(1., |(_, at)| {
            crate::hud_motion::easing((at.elapsed().as_secs_f32() / 0.117).min(1.), true)
        });
        let alpha = (217. * fade).round() as u8;
        let ink = (255. * fade).round() as u8;
        let icon = if skill {
            self.skill_icon(&s.champion, i)
        } else if i == crate::inventory::PURCHASE {
            self.purchase_icon.clone()
        } else {
            self.item_icon(&s.items[i])
        };
        let has_art = icon.is_some();
        self.icon(ctx, "tooltip.art", icon, true, log);
        self.props(
            ctx,
            "tooltip.art",
            format!("visible: {has_art}; color: #00000000;"),
            log,
        );
        for node in ["tooltip.art.icon", "tooltip.art.png"] {
            self.props(ctx, node, format!("color: #ffffff{ink:02x};"), log);
        }
        let compact = !skill && body.is_empty();
        for node in [
            "tooltip.rule",
            "tooltip.meta",
            "tooltip.meta_clock",
            "tooltip.meta_range",
            "tooltip.range",
            "tooltip.text",
            "tooltip.key",
            "tooltip.scroll_track",
            "tooltip.scroll_thumb",
        ] {
            self.props(ctx, node, format!("visible: {};", !compact), log);
        }
        self.props(
            ctx,
            "tooltip.status",
            format!(
                "visible: {}; source: \"asset/lt_direct_control/ui/{}\"; color: #ffffff{ink:02x};",
                compact,
                if i == 6 && self.purchase_ready {
                    "ef_check"
                } else {
                    "ef_unavailable"
                }
            ),
            log,
        );
        if compact {
            self.props(ctx, "tooltip.art", "visible: false;".into(), log);
            let title = if i == crate::inventory::PURCHASE && !self.purchase_ready {
                "Purchase information unavailable"
            } else {
                title
            };
            let width = (crate::hud_style::width(title, 20.) + 64.).clamp(160., 529.);
            let (title, lines) = crate::hud_style::wrap(title, 20., width - 64.);
            let height = lines * 26 + 32;
            self.props(ctx,"tooltip",format!("visible: true; x: {}px; y: {}px; width: {width:.2}px; height: {height}px; color: #~1e1e1d{alpha:02x};",1556.-width,78.-height as f32),log);
            self.props(ctx,"tooltip.title",format!("x: 52px; y: 16px; width: {}px; height: {}px; size: 20; line_height: 26; color: #ffffff{ink:02x};",width-64.,height-32),log);
            self.text(ctx, "tooltip.title", title, log);
            return;
        }
        let left = if has_art { 76. } else { 16. };
        let bottom = (1080 - HEIGHT) as f32 + if skill { -8. } else { 78. };
        self.tooltip_layout.prepare(
            title,
            body,
            has_art,
            (bottom - crate::tooltip_layout::TOP) as usize,
        );
        self.tooltip_layout.scroll(notches);
        let width = self.tooltip_layout.width;
        let height = self.tooltip_layout.height;
        let title_lines = self.tooltip_layout.title_lines;
        let title = self.tooltip_layout.title.clone();
        let body = self.tooltip_layout.text();
        let extra = self.tooltip_layout.extra as f32;
        let max_scroll = self.tooltip_layout.max();
        let x = if skill { 730 } else { 1556 - width };
        let y = if skill {
            -8 - height as i32
        } else {
            78 - height as i32
        };
        self.props(ctx,"tooltip",format!("visible: true; x: {x}px; y: {y}px; width: {width}px; height: {height}px; color: #~1e1e1d{alpha:02x};"),log);
        self.props(ctx,"tooltip.title",format!("x: {left}px; y: 23px; width: {}px; height: {}px; size: 27; line_height: 32; color: #ffffff{ink:02x};",width as f32-34.-left,32.*title_lines as f32),log);
        self.text(ctx, "tooltip.title", title, log);
        self.props(
            ctx,
            "tooltip.rule",
            format!("width: {}px;", width - 32),
            log,
        );
        self.props(ctx, "tooltip.key", format!("x: {}px;", width - 42), log);
        for node in ["tooltip.scroll_track", "tooltip.scroll_thumb"] {
            self.props(ctx, node, format!("visible: {};", max_scroll > 0), log);
        }
        if max_scroll > 0 {
            let view = (self.tooltip_layout.page * crate::tooltip_layout::LINE) as f32;
            let thumb = (view * self.tooltip_layout.page as f32
                / (self.tooltip_layout.page + max_scroll) as f32)
                .max(24.);
            let top = 106. + extra;
            let thumb_y =
                top + (view - thumb) * self.tooltip_layout.first as f32 / max_scroll as f32;
            self.props(
                ctx,
                "tooltip.scroll_track",
                format!("x: {}px; y: {top}px; height: {view}px;", width - 10),
                log,
            );
            self.props(
                ctx,
                "tooltip.scroll_thumb",
                format!("x: {}px; y: {thumb_y}px; height: {thumb}px;", width - 10),
                log,
            );
            let tile = if skill {
                format!("skill{i}")
            } else if i == crate::inventory::PURCHASE {
                "next".into()
            } else {
                format!("item{i}")
            };
            if let Some((tx, ty, tw, th)) = ctx.ui_node_rect(&format!("{PATH}.{tile}")) {
                if let Ok(mut area) = TOOLTIP_SCROLL_AREA.lock() {
                    *area = Some((
                        Rect {
                            x: x as f32,
                            y: bottom - height as f32,
                            w: width as f32,
                            h: height as f32,
                        },
                        Rect {
                            x: tx,
                            y: ty,
                            w: tw,
                            h: th,
                        },
                    ));
                }
            }
        }
        self.props(
            ctx,
            "tooltip.rule",
            format!(
                "y: {}px; color: #ffffff{:02x};",
                98. + extra,
                (59. * fade) as u8
            ),
            log,
        );
        self.props(
            ctx,
            "tooltip.text",
            format!(
                "y: {}px; width: {}px; height: {}px; color: #d6d6d6{ink:02x};",
                106. + extra,
                width - 36,
                height.saturating_sub(122 + extra as usize)
            ),
            log,
        );
        self.text(
            ctx,
            "tooltip.text",
            crate::hud_motion::fade_rich(&body, fade),
            log,
        );
        self.text(
            ctx,
            "tooltip.key",
            if skill { KEYS[i].into() } else { String::new() },
            log,
        );
        self.props(
            ctx,
            "tooltip.key",
            format!("color: #d6d6d6{:02x};", (107. * fade) as u8),
            log,
        );
        let (meta, range, meta_glyph) = if skill {
            let spec = self
                .custom_icons
                .as_ref()
                .and_then(|a| a.descriptions.get(&s.champion))
                .or_else(|| crate::tooltips::base_champion(&s.champion));
            let spec = spec.and_then(|root| crate::tooltips::skill_spec(&s.champion, i, root));
            let spec = spec.as_ref();
            let cd = spec
                .and_then(|s| s.get("cooltime"))
                .and_then(serde_json::Value::as_u64);
            let range = spec
                .and_then(|s| s.get("range"))
                .and_then(serde_json::Value::as_u64)
                .filter(|n| *n > 0);
            (
                cd.map_or(String::new(), |n| format!("{:.1}s", n as f32 / 60.)),
                range.map(|n| format!("{}", n / 1000)),
                "ef_clock",
            )
        } else {
            let price = if i < 6 {
                self.item_spec(&s.items[i])
                    .and_then(|v| v.get("price"))
                    .and_then(serde_json::Value::as_u64)
            } else {
                self.purchase_price
            };
            (
                price.map_or(String::new(), |n| n.to_string()),
                None,
                "ef_coin",
            )
        };
        let show_meta = !meta.is_empty();
        let show_range = range.is_some();
        self.props(
            ctx,
            "tooltip.meta",
            format!(
                "visible: {show_meta}; x: {}px; y: {}px; color: #d6d6d6{:02x};",
                left + 24.,
                61. + extra,
                (117. * fade) as u8
            ),
            log,
        );
        self.text(ctx, "tooltip.meta", meta.clone(), log);
        self.props(ctx,"tooltip.meta_clock",format!("visible: {show_meta}; x: {left}px; y: {}px; source: \"asset/lt_direct_control/ui/{meta_glyph}\"; color: #d6d6d6{:02x};",61.+extra,(117.*fade)as u8),log);
        let range_x = left + 24. + crate::hud_style::width(&meta, 20.) + 20.;
        self.props(
            ctx,
            "tooltip.meta_range",
            format!(
                "visible: {show_range}; x: {range_x}px; y: {}px; color: #d6d6d6{:02x};",
                61. + extra,
                (117. * fade) as u8
            ),
            log,
        );
        self.props(
            ctx,
            "tooltip.range",
            format!(
                "visible: {show_range}; x: {}px; y: {}px; color: #d6d6d6{:02x};",
                range_x + 24.,
                61. + extra,
                (117. * fade) as u8
            ),
            log,
        );
        self.text(ctx, "tooltip.range", range.unwrap_or_default(), log);
    }
    pub fn bounds(&self, ctx: &StableClient<'_>) -> Vec<Rect> {
        if ctx.ui_visible(PATH) != Some(true) {
            return Vec::new();
        }
        // The transparent root spans the viewport; only opaque surfaces block input.
        let mut bounds = Vec::new();
        for node in ["combat", "strip", "tooltip", "feedback"] {
            let path = format!("{PATH}.{node}");
            if ctx.ui_visible(&path) == Some(true) {
                if let Some((x, y, w, h)) = ctx.ui_node_rect(&path) {
                    let r = if node == "combat" {
                        Rect {
                            x,
                            y: y - 2.,
                            w,
                            h: h + 4.,
                        }
                    } else {
                        Rect { x, y, w, h }
                    };
                    if r.valid() {
                        bounds.push(r);
                    }
                }
            }
        }
        for i in 0..self.inventory_count.unwrap_or(0) {
            if crate::inventory::hud_slot(self.inventory_count.unwrap_or(0), i).1 < 90 {
                if let Some((x, y, w, h)) = ctx.ui_node_rect(&format!("{PATH}.item{i}")) {
                    bounds.push(Rect { x, y, w, h });
                }
            }
        }
        if bounds.is_empty() {
            if let Some((x, y, _, h)) = ctx.ui_node_rect("ingame") {
                if self.playing {
                    bounds.push(Rect {
                        x: x + SKILL_X as f32,
                        y: y + h - HEIGHT as f32 + SKILL_Y as f32,
                        w: (SKILL_STEP * 2 + SKILL_SIZE) as f32,
                        h: SKILL_SIZE as f32,
                    });
                }
                bounds.push(Rect {
                    x,
                    y: y + h - 56.,
                    w: crate::minimap::STRIP_END as f32,
                    h: 56.,
                });
            }
        }
        bounds
    }
    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &mut self,
        ctx: &mut StableClient<'_>,
        active: bool,
        playing: bool,
        snapshot: Option<&Snapshot>,
        artwork: Option<&Artwork>,
        skills: HudSkills,
        status: &str,
        cursor: Option<(f32, f32)>,
        pressed: bool,
        hud: &PlayerHud,
        log: &Logger,
    ) {
        if let Ok(mut area) = TOOLTIP_SCROLL_AREA.lock() {
            *area = None;
        }
        if !active {
            self.hover = None;
            self.tooltip_source = None;
            TOOLTIP_SCROLL.store(0, Ordering::Relaxed);
            self.motion.reset();
            self.motion_identity = None;
            self.spawn_attempted = None;
            if ctx.ui_exists(PATH) {
                ctx.ui_set_visible(PATH, false);
            }
            return;
        }
        if !ctx.ui_exists(PATH) {
            if self
                .spawn_attempted
                .is_some_and(|t| t.elapsed() < Duration::from_secs(1))
            {
                return;
            }
            self.spawn_attempted = Some(Instant::now());
            self.cache.clear();
            self.allocated_slots = 6;
            self.purchase_inputs = None;
            self.failed = false;
            let spawned = ctx.ui_spawn_source("ingame", &crate::ui_theme::themed(&template()));
            log.write(&format!(
                "PLAYER HUD spawn={spawned} exists={}",
                ctx.ui_exists(PATH)
            ));
            if !spawned || !ctx.ui_exists(PATH) {
                return;
            }
        }
        if ctx.ui_visible(PATH) != Some(true) {
            ctx.ui_set_visible(PATH, true);
        }
        self.playing = playing;
        for node in [
            "combat",
            "edge",
            "kda_icon",
            "kills",
            "deaths",
            "assists",
            "slash1",
            "slash2",
            "cs_label",
            "cs",
            "level",
            "health",
            "coin",
            "gold",
            "next",
            "purchase",
            "skill0",
            "skill1",
            "skill2",
            "recall_key",
            "recall_icon",
            "shop_hit",
            "shop_key",
        ] {
            let blank = self.purchase_blank && matches!(node, "next" | "purchase");
            self.props(ctx, node, format!("visible: {};", playing && !blank), log);
        }
        // Shop button: hover tint over 100 ms; a press-and-release opens the shop.
        let shop_hover = playing
            && cursor.is_some_and(|p| {
                ctx.ui_node_rect(&format!("{PATH}.shop_hit"))
                    .is_some_and(|(x, y, w, h)| Rect { x, y, w, h }.contains(p))
            });
        if shop_hover && self.shop_pressed && !pressed {
            self.shop_click = true;
        }
        self.shop_pressed = pressed && (shop_hover || self.shop_pressed);
        let tint = self.motion.tonal("shop_hover", f32::from(shop_hover), 0.1);
        let back = crate::hud_motion::color(
            crate::ui_theme::tone(0x3a38_3700),
            crate::ui_theme::tone(0x3a38_37ff),
            tint,
        );
        self.props(ctx, "shop_hit", format!("color: #{back};"), log);
        if !playing {
            self.hover = None;
            self.tooltip_source = None;
            TOOLTIP_SCROLL.store(0, Ordering::Relaxed);
            for i in 0..self.allocated_slots {
                self.props(ctx, &format!("item{i}"), "visible: false;".into(), log);
            }
            self.health_ticks(ctx, None, log);
            self.purchase_inputs = None;
            for node in [
                "tooltip",
                "feedback",
                "respawn",
                "death_banner",
                "hourglass",
                "hp_current",
                "hp_max",
                "purchase_coin",
                "purchase_check",
                "next_unavailable",
            ] {
                self.props(ctx, node, "visible: false;".into(), log);
            }
            return;
        }
        self.props(
            ctx,
            "feedback",
            format!("visible: {};", !status.is_empty()),
            log,
        );
        self.text(ctx, "feedback.text", status.into(), log);
        self.text(ctx, "slash1", "/".into(), log);
        self.text(ctx, "slash2", "/".into(), log);
        let Some(s) = snapshot else {
            if artwork.is_some() {
                self.load_icons(log);
            }
            for node in ["kills", "deaths", "assists", "cs", "gold", "level"] {
                self.text(ctx, node, "—".into(), log);
            }
            self.text(ctx, "respawn", String::new(), log);
            self.props(ctx, "death_banner", "visible: false;".into(), log);
            self.text(ctx, "hp_current", "—".into(), log);
            self.text(ctx, "hp_max", " / —".into(), log);
            self.props(ctx, "hourglass", "visible: false;".into(), log);
            for node in ["hp_current", "hp_max"] {
                self.props(ctx, node, "visible: true;".into(), log);
            }
            self.purchase(ctx, None, log);
            self.props(ctx, "health.fill", "width: 0%;".into(), log);
            self.props(ctx, "health.trail", "width: 0%;".into(), log);
            self.health_ticks(ctx, None, log);
            for i in 0..3 {
                let icon = artwork.and_then(|a| self.skill_icon(&a.champion, i));
                self.icon(ctx, &format!("skill{i}"), icon, true, log);
                self.props(
                    ctx,
                    &format!("skill{i}.shade"),
                    "visible: true; color: #00000088;".into(),
                    log,
                );
                self.text(ctx, &format!("skill{i}.cooldown"), "--".into(), log);
                self.text(ctx, &format!("skill{i}.uses"), String::new(), log);
                self.props(
                    ctx,
                    &format!("skill{i}.lock"),
                    "visible: false;".into(),
                    log,
                );
                self.props(ctx, &format!("skill{i}"), "color: #555555ff;".into(), log);
            }
            let retained = artwork
                .filter(|a| self.item_identity == Some(a.identity))
                .and(self.inventory_count);
            let count =
                crate::inventory::slots(None, artwork.map_or(0, |a| a.items.len()), retained);
            self.inventory_layout(ctx, count, artwork.map(|a| a.identity), log);
            for i in 0..count {
                let icon = artwork
                    .filter(|a| self.item_identity == Some(a.identity))
                    .and_then(|a| a.items.get(i))
                    .and_then(|key| self.item_icon(key));
                self.icon(ctx, &format!("item{i}"), icon, false, log);
                self.props(
                    ctx,
                    &format!("item{i}.unknown"),
                    "visible: false;".into(),
                    log,
                );
            }
            self.props(ctx, "tooltip", "visible: false;".into(), log);
            return;
        };
        if self.motion_identity != Some((s.key, s.player)) {
            self.motion.reset();
            self.cooldown_peak = [0; 3];
            self.motion_identity = Some((s.key, s.player));
        }
        self.refresh_assets(ctx, s, hud, log);
        let count = crate::inventory::slots(
            s.build.as_ref().map(Vec::len),
            s.items.len(),
            self.inventory_count,
        );
        self.inventory_layout(ctx, count, Some((s.key, s.player)), log);
        for (node, value) in [
            ("kills", s.kda.0),
            ("deaths", s.kda.1),
            ("assists", s.kda.2),
            ("cs", s.cs),
            ("gold", s.gold),
            ("level", s.level),
        ] {
            self.text(ctx, node, value.to_string(), log);
        }
        self.purchase(ctx, Some(s), log);
        for node in ["coin", "purchase_coin", "purchase_check"] {
            self.props(ctx, node, "color: #fdee00ff;".into(), log);
        }
        self.props(ctx, "hourglass", format!("visible: {};", !s.alive), log);
        self.props(ctx, "respawn", format!("visible: {};", !s.alive), log);
        self.props(ctx, "death_banner", format!("visible: {};", !s.alive), log);
        self.text(
            ctx,
            "death_banner.text",
            death_countdown(s.alive, s.respawn),
            log,
        );
        self.text(
            ctx,
            "respawn",
            if !s.alive {
                if s.respawn > 0 {
                    s.respawn.div_ceil(60).to_string()
                } else {
                    "—".into()
                }
            } else {
                String::new()
            },
            log,
        );
        for node in ["hp_current", "hp_max"] {
            self.props(ctx, node, format!("visible: {};", s.alive), log);
        }
        self.text(
            ctx,
            "hp_current",
            s.hp.map_or("—".into(), |(hp, _)| hp.to_string()),
            log,
        );
        self.text(
            ctx,
            "hp_max",
            s.hp.map_or(" / —".into(), |(_, max)| format!(" / {max}")),
            log,
        );
        let health_fill = self.motion.value("hp", hp_percent(s), 0.2);
        let health_fill = if s.alive { health_fill } else { 0. };
        self.props(
            ctx,
            "health.fill",
            format!("width: {:.2}%; color: #{};", health_fill, "2e7d46ff"),
            log,
        );
        let trail = self.motion.value("hp_trail", hp_percent(s), 0.5);
        let trail = if s.alive { trail } else { 0. };
        self.props(ctx, "health.trail", format!("width: {trail:.2}%;"), log);
        self.health_ticks(ctx, s.hp.filter(|_| s.alive).map(|(_, max)| max), log);
        for (i, unlock_level) in UNLOCK_LEVELS.into_iter().enumerate() {
            self.icon(
                ctx,
                &format!("skill{i}"),
                self.skill_icon(&s.champion, i),
                true,
                log,
            );
            let learned = s.level >= unlock_level;
            let unavailable = skill_unavailable(s, skills, i);
            let ready = skills.ready[i].unwrap_or(s.cooldowns[i] == 0);
            let remaining = skills.wait[i].unwrap_or(s.cooldowns[i]);
            if ready {
                self.cooldown_peak[i] = 0;
            } else {
                self.cooldown_peak[i] = self.cooldown_peak[i].max(remaining);
            }
            let drain = if unavailable {
                78.
            } else {
                78. * remaining as f32 / self.cooldown_peak[i].max(1) as f32
            };
            self.props(
                ctx,
                &format!("skill{i}.drain_edge"),
                format!("visible: {}; y: {:.2}px;", !unavailable && !ready, drain),
                log,
            );
            self.props(
                ctx,
                &format!("skill{i}.shade"),
                format!(
                    "visible: {}; height: {drain:.2}px; color: #{};",
                    unavailable || !ready,
                    if learned { "0a0908cc" } else { "141311dd" }
                ),
                log,
            );
            self.text(
                ctx,
                &format!("skill{i}.cooldown"),
                if !learned {
                    String::new()
                } else if unavailable {
                    "--".into()
                } else if !ready {
                    cooldown(skills.wait[i].unwrap_or(s.cooldowns[i]))
                } else {
                    String::new()
                },
                log,
            );
            self.text(
                ctx,
                &format!("skill{i}.uses"),
                if unavailable {
                    String::new()
                } else {
                    skills.uses[i].map_or(String::new(), |n| n.to_string())
                },
                log,
            );
            self.props(
                ctx,
                &format!("skill{i}.lock"),
                format!("visible: {};", !learned),
                log,
            );
            let color = if !learned {
                "~4b4a49ff"
            } else if skills.aiming == Some(i) {
                "fdee00ff"
            } else if unavailable || skills.available[i].is_none() || !ready {
                "~4b4a49ff"
            } else if i == 2 {
                "fdee00ff"
            } else {
                "~4b4a49ff"
            };
            let node = format!("skill{i}");
            let hover = self.tile_hover(ctx, &node, cursor);
            let h = self
                .motion
                .tonal(&format!("hover_{node}"), f32::from(hover), 0.1);
            let press =
                self.motion
                    .tonal(&format!("press_{node}"), f32::from(hover && pressed), 0.083);
            let border = crate::ui_theme::color(color).unwrap_or(crate::ui_theme::tone(0x4b4a49ff));
            let border = crate::hud_motion::color(border, 0xffffffff, h);
            self.props(
                ctx,
                &node,
                format!("color: #{border}; y: {:.2}px;", SKILL_Y as f32),
                log,
            );
            let inset = if color == "fdee00ff" { 2. } else { 1. };
            self.props(
                ctx,
                &format!("{node}.background"),
                format!(
                    "x: {inset}px; y: {inset}px; width: {}px; height: {}px;",
                    80. - 2. * inset,
                    80. - 2. * inset
                ),
                log,
            );
            for art in ["icon", "png"] {
                self.props(
                    ctx,
                    &format!("{node}.{art}"),
                    format!(
                        "width: 74px; height: 74px; color: #{};",
                        crate::hud_motion::color(0xffffffff, 0xd9d9d9ff, press)
                    ),
                    log,
                );
            }
        }
        for i in 0..count {
            let icon = s.items.get(i).and_then(|key| self.item_icon(key));
            let unknown = s.items.get(i).is_some() && icon.is_none();
            self.icon(ctx, &format!("item{i}"), icon, false, log);
            self.props(
                ctx,
                &format!("item{i}.unknown"),
                format!("visible: {unknown};"),
                log,
            );
        }
        for i in (0..count).chain(std::iter::once(crate::inventory::PURCHASE)) {
            let node = if i == crate::inventory::PURCHASE {
                "next".into()
            } else {
                format!("item{i}")
            };
            let hover = self.tile_hover(ctx, &node, cursor);
            let h = self
                .motion
                .tonal(&format!("hover_{node}"), f32::from(hover), 0.1);
            let press =
                self.motion
                    .tonal(&format!("press_{node}"), f32::from(hover && pressed), 0.083);
            // Inventory rest positions are owned by inventory_layout.

            self.props(
                ctx,
                &format!("{node}.background"),
                format!(
                    "color: #{};",
                    crate::hud_motion::color(
                        // Filled: background + 5; empty: + 13.
                        crate::ui_theme::shade_rgba(
                            if s.items.get(i).is_some() { 5 } else { 13 },
                            0xff,
                        ),
                        crate::ui_theme::tone(0x3a3837ff),
                        (h - press * 0.25).clamp(0., 1.)
                    )
                ),
                log,
            );
            if i != crate::inventory::PURCHASE {
                self.props(
                    ctx,
                    &node,
                    format!(
                        "color: #{};",
                        crate::hud_motion::color(crate::ui_theme::tone(0x4b4a49ff), 0xffffffff, h)
                    ),
                    log,
                );
            } else {
                let border = if self.purchase_ready {
                    "fdee00ff".into()
                } else {
                    crate::hud_motion::color(0x989694ff, 0xffffffff, h)
                };
                self.props(ctx, &node, format!("color: #{border};"), log);
                let opacity = if self.purchase_ready {
                    255
                } else {
                    (140. + 115. * h).round() as u8
                };
                self.props(
                    ctx,
                    "next.icon",
                    format!("color: #ffffff{opacity:02x};"),
                    log,
                );
            }
        }
        self.tooltip(ctx, s, cursor, log);
    }
    fn health_ticks(&mut self, ctx: &mut StableClient<'_>, max: Option<usize>, log: &Logger) {
        for i in 0usize..32 {
            let offset = max.filter(|max| *max > 0).and_then(|max| {
                let step = max.div_ceil(3200).saturating_mul(100);
                let value = (i + 1).saturating_mul(step);
                (value < max).then(|| 360. * value as f64 / max as f64)
            });
            self.props(
                ctx,
                &format!("hp_tick{i}"),
                offset.map_or("visible: false;".into(), |x| {
                    format!("visible: true; x: {:.2}px;", 780. + x)
                }),
                log,
            );
        }
    }
}

// An uncertain branch uses one compact minimum, never a wrapping range.
fn purchase_requirement(f: &crate::purchase_tracker::Forecast, gold: usize) -> String {
    if !f.affordable.is_empty() {
        return "0".into();
    }
    if let Some(next) = &f.next {
        return format!("−{}", next.price.saturating_sub(gold));
    }
    let mut costs = f.alternatives.iter().map(|p| p.price.saturating_sub(gold));
    let Some(first) = costs.next() else {
        return "?".into();
    };
    let (lo, hi) = costs.fold((first, first), |(lo, hi), n| (lo.min(n), hi.max(n)));
    if hi == 0 {
        "0".into()
    } else if lo == hi {
        lo.to_string()
    } else {
        format!("{lo}+")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn purchase_branch_shortfall_is_compact_and_updates_with_held_gold() {
        use crate::purchase_tracker::{Forecast, Purchase};
        let mut f = Forecast {
            target: "target".into(),
            next: None,
            affordable: Vec::new(),
            branching: true,
            completed: false,
            alternatives: vec![
                Purchase {
                    key: "cheap".into(),
                    price: 450,
                },
                Purchase {
                    key: "dear".into(),
                    price: 800,
                },
            ],
        };
        assert_eq!(purchase_requirement(&f, 254), "196+");
        assert_eq!(purchase_requirement(&f, 255), "195+");
        assert_eq!(purchase_requirement(&f, 450), "0+");
        f.affordable.push(f.alternatives[0].clone());
        assert_eq!(purchase_requirement(&f, 450), "0");
        f.affordable.clear();
        f.alternatives[1].price = 450;
        assert_eq!(purchase_requirement(&f, 254), "196");
        f.next = Some(f.alternatives[0].clone());
        assert_eq!(purchase_requirement(&f, 254), "−196");
        f.next = None;
        f.alternatives.clear();
        assert_eq!(purchase_requirement(&f, 254), "?");
    }
    #[test]
    fn exported_build_observer_receives_engine_catalog_without_overriding_build() {
        use mod_api_stable::{ItemBuildCtxV1, StableMod, StrV1};
        let hud = std::sync::Arc::new(PlayerHud::default());
        let mut declaration = StableMod::new("catalog-test");
        declaration.add_item_build_hook(crate::purchase_tracker::BuildObserver(hud.clone()));
        let export = declaration.into_export_raw();
        let keys = [StrV1::from_str("blade"), StrV1::from_str("modded-final")];
        let mut out = [99usize; 2];
        // All fields are integers or raw pointers. Empty host slices remain null.
        let mut context: ItemBuildCtxV1 = unsafe { std::mem::zeroed() };
        context.size = std::mem::size_of::<ItemBuildCtxV1>();
        context.item_keys_ptr = keys.as_ptr();
        context.item_keys_len = keys.len();
        unsafe {
            assert_eq!((*export).item_build_hooks_len, 1);
            assert!(!(*export).item_build_hooks_ptr.is_null());
            let hook = *(*export).item_build_hooks_ptr;
            let vtable = &*hook.vtable;
            assert_eq!(
                vtable.decide_build.unwrap()(hook.userdata, &context, out.as_mut_ptr(), out.len()),
                0
            );
            assert_eq!(&*hud.3.lock().unwrap(), &["blade", "modded-final"]);
            assert_eq!(out, [99; 2]);
            vtable.destroy.unwrap()(hook.userdata);
            (*export).destroy.unwrap()(export);
        }
    }
    #[test]
    fn death_banner_uses_simulation_ticks_without_inventing_missing_countdown() {
        assert_eq!(death_countdown(false, 601), "Respawning in 11");
        assert_eq!(death_countdown(false, 60), "Respawning in 1");
        assert_eq!(death_countdown(false, 0), "Respawning…");
        assert_eq!(death_countdown(true, 600), "");
        assert!(template().contains("#death_banner:color"));
    }
    #[test]
    fn prepared_hud_switches_identity_without_reusing_previous_stats_or_next_set_data() {
        let hud = PlayerHud::default();
        let mut first = sample();
        hud.observe_prepared(first.clone());
        first.player = 8;
        first.champion = "harpy".into();
        first.gold = 999;
        hud.observe_prepared(first.clone());
        hud.select_prepared(first.key, 8);
        let s = hud.snapshot(Some((first.key, 8)), false).unwrap();
        assert_eq!(s.champion, "harpy");
        assert_eq!(s.gold, 999);
        assert!(hud.snapshot(Some((first.key, 7)), false).is_none());
        hud.select_prepared((1, 2, 4), 8);
        assert!(hud.snapshot(Some(((1, 2, 4), 8)), false).is_none());
        hud.reset_session();
        hud.select_prepared(first.key, 8);
        assert!(hud.snapshot(Some((first.key, 8)), false).is_none());
    }
    fn sample() -> Snapshot {
        Snapshot {
            key: (1, 2, 3),
            player: 7,
            champion: "lancer".into(),
            level: 1,
            hp: Some((900, 1000)),
            alive: true,
            respawn: 0,
            gold: 100,
            kda: (0, 0, 0),
            cs: 0,
            cooldowns: [0, 61, 0],
            items: vec!["ironsword".into()],
            build: None,
        }
    }
    #[test]
    fn session_reset_rejects_old_same_key_hud_and_restarts_tick_sampling() {
        let hud = PlayerHud::default();
        let log = crate::test_support::logger("hud-session-reset");
        let s = sample();
        assert!(hud.needs_sample(s.key, s.player, 6));
        hud.observe_tick(s.clone(), 6, &log);
        assert!(hud.snapshot(Some((s.key, s.player)), false).is_some());
        assert!(!hud.needs_sample(s.key, s.player, 6));
        hud.reset_session();
        assert!(hud.snapshot(Some((s.key, s.player)), false).is_none());
        assert!(hud.needs_sample(s.key, s.player, 6));
    }
    #[test]
    fn snapshots_are_match_and_player_scoped_and_running_data_expires() {
        let hud = PlayerHud::default();
        hud.observe(sample());
        assert!(hud.snapshot(Some(((1, 2, 3), 7)), true).is_some());
        assert!(hud.snapshot(Some(((1, 2, 4), 7)), true).is_none());
        assert!(hud.snapshot(Some(((1, 2, 3), 6)), true).is_none());
        hud.0.lock().unwrap().as_mut().unwrap().1 = Instant::now() - Duration::from_secs(1);
        assert!(hud.snapshot(Some(((1, 2, 3), 7)), true).is_none());
        assert!(hud.snapshot(Some(((1, 2, 3), 7)), false).is_some()); // READY deliberately holds the worker.
        assert!(hud.needs_sample((1, 2, 4), 7, 1));
        assert!(!hud.needs_sample((1, 2, 3), 7, 1));
    }
    #[test]
    fn artwork_survives_telemetry_gaps_but_not_identity_changes_or_reset() {
        let hud = PlayerHud::default();
        let mut s = sample();
        s.champion = "jiangshi".into();
        hud.observe(s.clone());
        let identity = Some((s.key, s.player));
        let art = hud.artwork(identity).unwrap();
        hud.0.lock().unwrap().as_mut().unwrap().1 = Instant::now() - Duration::from_secs(1);
        assert!(hud.snapshot(identity, true).is_none());
        assert_eq!(hud.artwork(identity), Some(art));
        assert!(hud.artwork(None).is_none());
        assert!(hud.artwork(Some(((1, 2, 4), 7))).is_none());
        assert!(hud.artwork(Some((s.key, 8))).is_none());

        let ui = HudUi::default();
        for i in 0..3 {
            assert!(ui
                .skill_icon(&hud.artwork(identity).unwrap().champion, i)
                .is_some());
        }
        s.items.push("registered_item".into());
        hud.observe(s.clone());
        assert_eq!(hud.artwork(identity).unwrap().items, s.items);
        assert!(hud.snapshot(identity, true).is_some());
        hud.reset_session();
        assert!(hud.artwork(identity).is_none());
    }
    #[test]
    fn dead_entity_keeps_only_its_own_known_champion_identity() {
        let hud = PlayerHud::default();
        hud.observe(sample());
        let mut dead = sample();
        dead.champion.clear();
        dead.alive = false;
        dead.hp = None;
        dead.respawn = 180;
        hud.observe(dead.clone());
        let s = hud.snapshot(Some(((1, 2, 3), 7)), true).unwrap();
        assert_eq!(s.champion, "lancer");
        assert_eq!(s.hp, None);
        assert!(!s.alive);
        dead.key = (1, 2, 4);
        hud.observe(dead);
        assert!(hud
            .snapshot(Some(((1, 2, 4), 7)), true)
            .unwrap()
            .champion
            .is_empty());
    }
    #[test]
    fn dead_hud_retains_identity_and_items_without_extrapolating_stale_timer() {
        let hud = PlayerHud::default();
        hud.observe(sample());
        let mut dead = sample();
        dead.alive = false;
        dead.hp = None;
        dead.champion.clear();
        dead.respawn = 600;
        dead.items.push("registered_item".into());
        hud.observe(dead);
        assert_eq!(
            hud.snapshot(Some(((1, 2, 3), 7)), true).unwrap().respawn,
            600
        );
        hud.0.lock().unwrap().as_mut().unwrap().1 = Instant::now() - Duration::from_secs(1);
        let retained = hud.snapshot(Some(((1, 2, 3), 7)), true).unwrap();
        assert_eq!(retained.champion, "lancer");
        assert_eq!(retained.items.len(), 2);
        assert_eq!(retained.respawn, 0); // No guessed wall-clock countdown.
        assert!(retained.hp.is_none());
        assert_eq!(
            hud.snapshot(Some(((1, 2, 3), 7)), false).unwrap().respawn,
            600
        ); // Pause keeps the actual timer.
        hud.observe(sample());
        assert!(hud.snapshot(Some(((1, 2, 3), 7)), true).unwrap().alive);
    }
    #[test]
    fn shared_actor_sampling_deduplicates_ticks_and_native_item_tags_require_evidence() {
        let hud = PlayerHud::default();
        hud.observe(sample());
        assert!(hud.needs_sample((1, 2, 3), 7, 6));
        assert!(!hud.needs_sample((1, 2, 3), 7, 6));
        assert!(!hud.needs_sample((1, 2, 3), 7, 7));
        assert!(hud.needs_sample((1, 2, 3), 7, 12));
        let mut ui = HudUi {
            custom_icons: Some(hud_icons::EnabledAssets::default()),
            ..Default::default()
        };
        assert!(ui.item_icon("registered_item").is_none());
        ui.custom_icons
            .as_mut()
            .unwrap()
            .item_tags
            .insert("registered_item".into());
        assert_eq!(
            ui.item_icon("registered_item").unwrap().tag.as_deref(),
            Some("registered_item")
        );
        ui.live_items.insert(
            "registered_item".into(),
            serde_json::json!({"name":"Registered item","icon":"overridden_tag","tier":3}),
        );
        assert_eq!(
            ui.item_icon("registered_item").unwrap().tag.as_deref(),
            Some("overridden_tag")
        );
    }
    #[test]
    fn native_icon_references_include_custom_source_and_item_alias() {
        assert_eq!(image("lancer", 0).unwrap().1, "lancer_0");
        assert!(image("harpooner", 2)
            .unwrap()
            .0
            .ends_with("harpooner_skill_icon"));
        assert_eq!(item("ironsword"), item("iron_blade"));
        assert!(image("unknown_champion", 0).is_none());
        assert!(item("unknown_item").is_none());
    }
    #[test]
    fn png_and_sheet_skills_have_separate_permanent_nodes_and_item_updates_are_scoped() {
        let sheet = Icon {
            source: "asset/base/skills".into(),
            tag: Some("lancer_0".into()),
        };
        let png = Icon {
            source: "asset/pack/icons/q".into(),
            tag: None,
        };
        let normal = icon_updates("skill0", Some(&sheet), true);
        assert!(normal[0].1.contains("lancer_0"));
        assert_eq!(normal[1], ("skill0.png".into(), "visible: false;".into()));
        let custom = icon_updates("skill0", Some(&png), true);
        assert_eq!(custom[0], ("skill0.icon".into(), "visible: false;".into()));
        assert_eq!(custom[1].0, "skill0.png");
        assert!(!custom[1].1.contains("rect_tag"));
        assert!(custom[1].1.contains("asset/pack/icons/q"));
        assert!(icon_updates("skill0", None, true)
            .iter()
            .all(|(_, p)| p == "visible: false;"));
        let mut slots = HashMap::new();
        for i in 0..6 {
            let item = Icon {
                source: "asset/base/items".into(),
                tag: Some(format!("item{i}")),
            };
            slots.extend(icon_updates(&format!("item{i}"), Some(&item), false));
        }
        let before = slots.clone();
        slots.extend(icon_updates("item5", Some(&sheet), false));
        for i in 0..5 {
            assert_eq!(
                slots[&format!("item{i}.icon")],
                before[&format!("item{i}.icon")]
            );
        }
        assert!(slots["item5.icon"].contains("lancer_0"));
        let source = template();
        if let Ok(dir) = std::env::var("LT_HUD_EXPORT_DIR") {
            std::fs::write(std::path::Path::new(&dir).join("player.ui"), &source).unwrap();
        }
        assert_eq!(source.matches("#png:image").count(), 4); // Three skills + tooltip artwork.
        assert_eq!(source.matches("#icon:image").count(), 11);
    }
    #[test]
    fn health_death_extremes_and_cooldowns_do_not_claim_false_readiness() {
        let mut s = sample();
        assert_eq!(hp_percent(&s), 90.);
        s.hp = Some((usize::MAX, 1));
        assert_eq!(hp_percent(&s), 100.);
        s.hp = Some((1, 0));
        assert_eq!(hp_percent(&s), 0.);
        s.alive = false;
        s.hp = Some((900, 1000));
        assert_eq!(hp_percent(&s), 0.);
        assert_eq!(cooldown(1), "0.1s");
        assert_eq!(cooldown(61), "1.1s");
    }
    #[test]
    fn unlock_levels_override_effect_presence_and_update_on_level_up() {
        let mut s = sample();
        s.cooldowns = [0; 3];
        let mut skills = HudSkills {
            available: [Some(true); 3],
            ..HudSkills::default()
        };
        for (level, expected) in [
            (0, [true, true, true]),
            (1, [false, true, true]),
            (2, [false, true, true]),
            (3, [false, false, true]),
            (4, [false, false, true]),
            (5, [false, false, false]),
        ] {
            s.level = level;
            assert_eq!(
                std::array::from_fn::<_, 3, _>(|i| skill_unavailable(&s, skills, i)),
                expected
            );
        }
        s.level = 5;
        skills.available[1] = Some(false);
        assert!(skill_unavailable(&s, skills, 1)); // Learned does not imply native effect availability.
        s.alive = false;
        assert!((0..3).all(|i| skill_unavailable(&s, skills, i)));
    }
}
