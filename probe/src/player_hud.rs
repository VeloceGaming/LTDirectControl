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
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

pub const PATH: &str = "ingame.lt_player_hud";
const KEYS: [&str; 3] = ["Q", "W", "R"];
const UNLOCK_LEVELS: [usize; 3] = [1, 3, 5];
const WIDTH: usize = 600;
const HEIGHT: usize = 120;
const BOTTOM_GAP: usize = 12;
const SKILL_X: usize = 76;
const SKILL_Y: usize = 34;
const SKILL_STEP: usize = 60;
const SKILL_SIZE: usize = 52;
const ITEM_X: usize = 310;
const ITEM_Y: usize = 58;
const ITEM_STEP: usize = 44;
const ITEM_SIZE: usize = 38;

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
}

#[derive(Default)]
pub struct PlayerHud(
    Mutex<Option<(Snapshot, Instant)>>,
    Mutex<Option<(MatchKey, usize, usize)>>,
    Mutex<HashMap<(MatchKey, usize), Snapshot>>,
);
impl PlayerHud {
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
    let align = if matches!(
        id,
        "cooldown" | "respawn" | "level" | "camera_key" | "target_key" | "recall_key"
    ) {
        "align_x: Center;"
    } else {
        ""
    };
    source.push_str(&format!("#{id}:label {{ @\"asset/base/style/main#label\"; x: {x}px; y: {y}px; width: {w}px; height: {h}px; z: 1005; size: {size}; color: #{color}; align_y: Center; {align} ignore_event: true; }}\n"));
}
fn template() -> String {
    let mut s = format!("lt_player_hud:color {{ width: {WIDTH}px; height: {HEIGHT}px; anchor_x: 0.5; pivot_x: 0.5; anchor_y: 1; pivot_y: 1; y: -{BOTTOM_GAP}px; z: 1000; color: #141414ff; ignore_event: true; rounding: Uniform {{ rounding: 8; }}\n");
    for (id, x, w, color) in [
        ("kills", 76, 28, "eeeeeeff"),
        ("slash1", 104, 12, "999999ff"),
        ("deaths", 116, 28, "ef5350ff"),
        ("slash2", 144, 12, "999999ff"),
        ("assists", 156, 28, "eeeeeeff"),
    ] {
        label(&mut s, id, (x, 6, w, 24), 14, color);
    }
    s.push_str(&crate::ui_graphics::image(
        "cs_icon", "minion", 190, 8, 20, 1003,
    ));
    label(&mut s, "cs", (216, 6, 40, 24), 14, "bbbbbbff");
    s.push_str("#portrait:image { x: 12px; y: 36px; width: 48px; height: 48px; z: 1003; sample_linear: false; ignore_event: true; }\n#portrait_shade:color { x: 12px; y: 36px; width: 48px; height: 48px; z: 1004; color: #000000aa; visible: false; ignore_event: true; }\n");
    label(&mut s, "respawn", (12, 48, 48, 28), 22, "eeeeeeff");
    label(&mut s, "level", (12, 88, 48, 22), 14, "eeeeeeff");
    for (i, key) in KEYS.iter().enumerate() {
        let x = SKILL_X + i * SKILL_STEP;
        s.push_str(&format!("#skill{i}:color {{ x: {x}px; y: {SKILL_Y}px; width: {SKILL_SIZE}px; height: {SKILL_SIZE}px; z: 1001; color: #777777ff; ignore_event: true; rounding: Uniform {{ rounding: 5; }} #background:color {{ x: 2px; y: 2px; width: 48px; height: 48px; z: 1002; color: #242424ff; ignore_event: true; }} #icon:image {{ x: 4px; y: 4px; width: 44px; height: 44px; z: 1003; sample_linear: false; ignore_event: true; visible: false; }} #png:image {{ x: 4px; y: 4px; width: 44px; height: 44px; z: 1003; sample_linear: false; ignore_event: true; visible: false; }} #shade:color {{ x: 2px; y: 2px; width: 48px; height: 48px; z: 1004; color: #000000bb; visible: false; ignore_event: true; }} "));
        label(&mut s, "cooldown", (2, 12, 48, 22), 16, "eeeeeeff");
        s.push_str(&crate::ui_graphics::image("lock", "lock", 17, 10, 18, 1005));
        s.push_str(&format!("#key:label {{ @\"asset/base/style/main#bold_label\"; x: 3px; y: 34px; width: 46px; height: 16px; z: 1005; size: 12; align_x: Center; color: #eeeeeeff; ignore_event: true; text: {key:?}; }} }}\n"));
    }
    s.push_str("#health:color { x: 76px; y: 92px; width: 172px; height: 18px; z: 1001; color: #333333ff; ignore_event: true; #fill:color { width: 0%; height: 100%; z: 1002; color: #999999ff; ignore_event: true; } #value:label { @\"asset/base/style/main#bold_label\"; width: 100%; height: 100%; z: 1005; size: 13; align_x: Center; align_y: Center; color: #ffffffff; outline_color: #000000ff; outline: 1; ignore_event: true; } }\n");
    s.push_str(&crate::ui_graphics::image(
        "target", "target", 266, 6, 24, 1003,
    ));
    label(&mut s, "target_key", (256, 30, 48, 14), 10, "999999ff");
    s.push_str(&crate::ui_graphics::image(
        "camera", "camera", 266, 47, 24, 1003,
    ));
    label(&mut s, "camera_key", (266, 73, 24, 14), 10, "999999ff");
    s.push_str(&crate::ui_graphics::image(
        "recall", "recall", 267, 92, 20, 1003,
    ));
    label(&mut s, "recall_key", (290, 94, 16, 14), 10, "999999ff");
    s.push_str(&crate::ui_graphics::image(
        "coin", "coin", 310, 31, 20, 1003,
    ));
    label(&mut s, "gold", (337, 30, 240, 24), 16, "eeeeeeff");
    for i in 0..6 {
        let x = ITEM_X + i * ITEM_STEP;
        s.push_str(&format!("#item{i}:color {{ x: {x}px; y: {ITEM_Y}px; width: {ITEM_SIZE}px; height: {ITEM_SIZE}px; z: 1001; color: #333333ff; ignore_event: true; rounding: Uniform {{ rounding: 4; }} #icon:image {{ x: 3px; y: 3px; width: 32px; height: 32px; z: 1003; sample_linear: false; ignore_event: true; visible: false; }} #unknown:label {{ @\"asset/base/style/main#label\"; width: 100%; height: 100%; z: 1005; size: 16; align_x: Center; align_y: Center; text: \"?\"; visible: false; ignore_event: true; }} }}\n"));
    }
    // Failure feedback occupies its own bubble only briefly, never routine prose.
    s.push_str("#feedback:color { x: 0px; y: -42px; width: 600px; height: 30px; z: 1010; color: #141414ff; visible: false; ignore_event: true; #text:label { @\"asset/base/style/main#label\"; x: 10px; width: 580px; height: 30px; z: 1011; size: 14; color: #eeeeeeff; ignore_event: true; } }\n");
    s.push_str("#tooltip:color { x: 0px; y: -350px; width: 520px; height: 338px; z: 1110; color: #141414ff; visible: false; ignore_event: true; rounding: Uniform { rounding: 6; } #text:label { @\"asset/base/style/main#label\"; x: 12px; y: 10px; width: 496px; height: 318px; z: 1111; size: 15; color: #eeeeeeff; ignore_event: true; } } }");
    s
}

#[derive(Default)]
pub struct HudUi {
    cache: HashMap<String, String>,
    portrait: String,
    portrait_attempted: String,
    failed: bool,
    spawn_attempted: Option<Instant>,
    custom_icons: Option<hud_icons::EnabledAssets>,
    live_items: HashMap<String, serde_json::Value>,
    item_identity: Option<(MatchKey, usize)>,
    item_read: Option<Instant>,
    hover: Option<(String, Instant)>,
    tooltip_text: HashMap<String, String>,
}
impl HudUi {
    fn refresh_assets(&mut self, ctx: &StableClient<'_>, snapshot: &Snapshot, log: &Logger) {
        if self.custom_icons.is_none() {
            let icons = hud_icons::enabled_assets();
            log.write(&format!(
                "PLAYER HUD enabled custom PNG champions={} overridden item tags={}",
                icons.champions.len(),
                icons.item_tags.len()
            ));
            self.custom_icons = Some(icons);
        }
        let identity = (snapshot.key, snapshot.player);
        if self.item_identity != Some(identity) {
            self.item_identity = Some(identity);
            self.live_items.clear();
            self.item_read = None;
        }
        let missing = snapshot
            .items
            .iter()
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
                if !spec.is_object() || hud_icons::item_icon(&spec).is_none() {
                    continue;
                }
                spec["name"] = name.clone().into();
                if let Some(key) = spec.get("key").and_then(serde_json::Value::as_str) {
                    entries.insert(key.to_owned(), spec.clone());
                }
                entries.insert(name, spec);
            }
            if entries != self.live_items {
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
    fn item_icon(&self, key: &str) -> Option<Icon> {
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
            if self.cache.get(&key) == Some(&properties) {
                continue;
            }
            self.props(ctx, &node, properties.clone(), log);
            let applied = self.cache.get(&key) == Some(&properties);
            let path = format!("{PATH}.{node}");
            log.write(&format!("PLAYER HUD stable image node={node} applied={applied} exists={} rect={:?} source={properties}",ctx.ui_exists(&path),ctx.ui_node_rect(&path)));
        }
    }
    fn props(&mut self, ctx: &mut StableClient<'_>, node: &str, source: String, log: &Logger) {
        let key = format!("properties:{node}");
        if self.cache.get(&key) == Some(&source) {
            return;
        }
        if ctx.ui_set_properties(&format!("{PATH}.{node}"), &source) {
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
                .chain((0..s.items.len().min(6)).map(|i| (false, i)))
                .find(|(skill, i)| {
                    let node = if *skill {
                        format!("skill{i}")
                    } else {
                        format!("item{i}")
                    };
                    ctx.ui_node_rect(&format!("{PATH}.{node}"))
                        .is_some_and(|(x, y, w, h)| Rect { x, y, w, h }.contains(cursor))
                })
        });
        let Some((skill, i)) = hovered else {
            self.hover = None;
            self.props(ctx, "tooltip", "visible: false;".into(), log);
            return;
        };
        let key = if skill {
            format!("skill:{}:{i}", s.champion)
        } else {
            format!("item:{}", s.items[i])
        };
        if self.hover.as_ref().is_none_or(|(old, _)| old != &key) {
            self.hover = Some((key.clone(), Instant::now()));
            self.props(ctx, "tooltip", "visible: false;".into(), log);
            return;
        }
        if self
            .hover
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() < Duration::from_millis(180))
        {
            return;
        }
        if !self.tooltip_text.contains_key(&key) {
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
            log.write(&format!(
                "HUD TOOLTIP key={key} chars={}",
                text.chars().count()
            ));
            self.tooltip_text.insert(key.clone(), text);
        }
        let text = self.tooltip_text[&key].clone();
        let lines = text
            .lines()
            .map(|l| {
                l.chars()
                    .map(|c| if c.is_ascii() { 1 } else { 2 })
                    .sum::<usize>()
                    .max(1)
                    .div_ceil(56)
            })
            .sum::<usize>();
        let height = (lines * 21 + 24).clamp(96, 650);
        self.props(
            ctx,
            "tooltip",
            format!("visible: true; y: -{}px; height: {height}px;", height + 12),
            log,
        );
        self.props(
            ctx,
            "tooltip.text",
            format!("height: {}px;", height - 20),
            log,
        );
        self.text(ctx, "tooltip.text", text, log);
    }
    pub fn bounds(&self, ctx: &StableClient<'_>) -> Vec<Rect> {
        if ctx.ui_visible(PATH) != Some(true) {
            return Vec::new();
        }
        let mut bounds = Vec::new();
        if let Some((x, y, w, h)) = ctx.ui_node_rect(PATH) {
            let rect = Rect { x, y, w, h };
            if rect.valid() {
                bounds.push(rect);
            }
        }
        // Layout can still be pending in the same callback as first spawn.
        if bounds.is_empty() {
            if let Some((x, y, w, h)) = ctx.ui_node_rect("ingame") {
                let rect = Rect {
                    x: x + (w - WIDTH as f32) / 2.,
                    y: y + h - (HEIGHT + BOTTOM_GAP) as f32,
                    w: WIDTH as f32,
                    h: HEIGHT as f32,
                };
                if rect.valid() {
                    bounds.push(rect);
                }
            }
        }
        bounds
    }
    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &mut self,
        ctx: &mut StableClient<'_>,
        active: bool,
        snapshot: Option<&Snapshot>,
        skills: HudSkills,
        champion_only: bool,
        camera_locked: bool,
        recalling: bool,
        status: &str,
        cursor: Option<(f32, f32)>,
        log: &Logger,
    ) {
        if !active {
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
            self.portrait.clear();
            self.portrait_attempted.clear();
            self.failed = false;
            let spawned = ctx.ui_spawn_source("ingame", &template());
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
        self.props(
            ctx,
            "feedback",
            format!("visible: {};", !status.is_empty()),
            log,
        );
        self.text(ctx, "feedback.text", status.into(), log);
        self.props(
            ctx,
            "target",
            format!(
                "source: \"asset/lt_direct_control_probe/ui/target{}\";",
                if champion_only { "_yellow" } else { "" }
            ),
            log,
        );
        self.props(
            ctx,
            "camera",
            format!(
                "source: \"asset/lt_direct_control_probe/ui/{}\";",
                if camera_locked {
                    "camera_lock_yellow"
                } else {
                    "camera"
                }
            ),
            log,
        );
        self.props(
            ctx,
            "recall",
            format!(
                "source: \"asset/lt_direct_control_probe/ui/recall{}\";",
                if recalling { "_yellow" } else { "" }
            ),
            log,
        );
        self.text(ctx, "target_key", "M4 / `".into(), log);
        self.text(ctx, "camera_key", "Y".into(), log);
        self.text(ctx, "recall_key", "B".into(), log);
        self.text(ctx, "slash1", "/".into(), log);
        self.text(ctx, "slash2", "/".into(), log);
        let Some(s) = snapshot else {
            for node in ["kills", "deaths", "assists", "cs", "gold", "level"] {
                self.text(ctx, node, "—".into(), log);
            }
            self.props(ctx, "portrait_shade", "visible: false;".into(), log);
            self.text(ctx, "respawn", String::new(), log);
            self.text(ctx, "health.value", "-- / --".into(), log);
            self.props(ctx, "health.fill", "width: 0%;".into(), log);
            self.props(ctx, "portrait", "visible: false;".into(), log);
            for i in 0..3 {
                self.icon(ctx, &format!("skill{i}"), None, true, log);
                self.props(
                    ctx,
                    &format!("skill{i}.shade"),
                    "visible: true;".into(),
                    log,
                );
                self.text(ctx, &format!("skill{i}.cooldown"), "--".into(), log);
                self.props(ctx, &format!("skill{i}"), "color: #555555ff;".into(), log);
            }
            for i in 0..6 {
                self.icon(ctx, &format!("item{i}"), None, false, log);
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
        self.refresh_assets(ctx, s, log);
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
        self.props(
            ctx,
            "portrait_shade",
            format!("visible: {};", !s.alive),
            log,
        );
        self.text(
            ctx,
            "respawn",
            if !s.alive && s.respawn > 0 {
                s.respawn.div_ceil(60).to_string()
            } else {
                String::new()
            },
            log,
        );
        self.text(
            ctx,
            "health.value",
            if !s.alive {
                s.hp.map_or("— / —".into(), |(_, max)| format!("0 / {max}"))
            } else {
                s.hp.map_or("— / —".into(), |(hp, max)| format!("{hp} / {max}"))
            },
            log,
        );
        self.props(
            ctx,
            "health.fill",
            format!("width: {:.2}%;", hp_percent(s)),
            log,
        );
        if self.portrait_attempted != s.champion {
            self.portrait_attempted = s.champion.clone();
            let set =
                ctx.ui_set_champion_icon(&format!("{PATH}.portrait"), &s.champion, 48., 48., 2.);
            log.write(&format!(
                "PLAYER HUD portrait champion={} applied={set}",
                s.champion
            ));
            if set {
                self.portrait = s.champion.clone();
            }
        }
        self.props(
            ctx,
            "portrait",
            format!("visible: {};", self.portrait == s.champion),
            log,
        );
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
            self.props(
                ctx,
                &format!("skill{i}.shade"),
                format!(
                    "visible: {}; color: #{};",
                    unavailable || s.cooldowns[i] > 0,
                    if learned { "000000bb" } else { "141414dd" }
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
                } else if s.cooldowns[i] > 0 {
                    cooldown(s.cooldowns[i])
                } else {
                    String::new()
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
                "555555ff"
            } else if skills.aiming == Some(i) {
                "ffd700ff"
            } else if unavailable || skills.available[i].is_none() || s.cooldowns[i] > 0 {
                "555555ff"
            } else {
                "999999ff"
            };
            self.props(ctx, &format!("skill{i}"), format!("color: #{color};"), log);
        }
        for i in 0..6 {
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
        self.tooltip(ctx, s, cursor, log);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        }
    }
    #[test]
    fn session_reset_rejects_old_same_key_hud_and_restarts_tick_sampling() {
        let hud = PlayerHud::default();
        let log = crate::timing_test::tests::logger("hud-session-reset");
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
        assert_eq!(source.matches("#png:image").count(), 3);
        assert_eq!(source.matches("#icon:image").count(), 9);
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
