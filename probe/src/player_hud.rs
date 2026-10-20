//! Selected-player scalar snapshots and a scoped native UI overlay.
//! No host context or entity pointer is retained across callbacks.
use crate::{
    abilities::HudSkills,
    camera::Rect,
    hud_icons::{self, Icon},
    // motion: pass
    hud_motion_pass as pass,
    native_timing::{MatchKey, Phase},
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
/// Skill slot cues (design note "HUD cues", 2026-10-17). The drain edge
/// warms from its white to signal yellow over the last tenth of a cooldown.
const EDGE_WARM_FROM: f32 = 0.9;
/// The icon's brightness while a learned, usable skill recovers; it lifts
/// to full when the skill is ready. Multiplied with the press tint.
const COOLING_ICON: f32 = 0.75;
/// At ready, the edge holds signal yellow for one beat (the 83 ms press
/// step), then fades on the tonal curve over 100 ms while the icon lifts.
const READY_BEAT: f32 = 0.083;
const READY_FADE: f32 = 0.1;
/// One-time whole-slot flash: 100 ms held, then a 300 ms tonal fade.
const READY_FLASH_HOLD: f32 = 0.1;
const READY_FLASH_FADE: f32 = 0.3;
/// Shared top stop for the countdown line and ready cue, inside R's border.
const READY_EDGE_Y: f32 = 2.;
/// Shorter cooldowns (and one-frame readiness blips) get no ready cue.
const READY_CUE_MIN_TICKS: usize = 30;
/// Only a cooldown of at least 4 s ends with the chime (when it is on), so a
/// skill that returns every two seconds does not chime all match.
const READY_CHIME_MIN_TICKS: usize = 240;
pub static TOOLTIP_SCROLL: AtomicI32 = AtomicI32::new(0);
/// Overflow panel and its source tile, in screen coordinates, for wheel routing.
pub static TOOLTIP_SCROLL_AREA: Mutex<Option<(Rect, Rect)>> = Mutex::new(None);

/// What the state plate under the game's clock shows (design note "HUD
/// motion, second pass", S1). No words: a glyph, then a key cap or a count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plate {
    /// The match is paused: the pause glyph in signal yellow, then F11.
    Paused,
    /// Your athlete is dead: the skull, then whole seconds to respawn
    /// (simulation ticks, rounded up); None while unknown (the skull alone).
    Dead(Option<usize>),
    /// The game's AI drives your athlete: the processor glyph, then F11.
    Ai,
}
/// The plate for this frame: paused over dead over AI; nothing outside a
/// match you play or watch. The count comes from simulation ticks only, as
/// the strip's `respawn` label does (no wall-clock extrapolation).
pub fn plate_state(
    phase: Option<Phase>,
    alive: Option<bool>,
    respawn_ticks: usize,
) -> Option<Plate> {
    match phase? {
        Phase::Paused => Some(Plate::Paused),
        Phase::Running | Phase::Ai if alive == Some(false) => Some(Plate::Dead(
            (respawn_ticks > 0).then(|| respawn_ticks.div_ceil(60)),
        )),
        Phase::Ai => Some(Plate::Ai),
        _ => None,
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
/// Whether a hovered HUD tile may show its tooltip: setting 0 on hover,
/// 1 only with the tooltip key held, 2 never; and never while a mouse
/// button is held (a click or drag passing over the HUD).
fn tooltip_allowed(setting: f64, key: bool, button: bool) -> bool {
    !button
        && match setting.round() as i64 {
            1 => key,
            2 => false,
            _ => true,
        }
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
            // Not every registered item is active in this save.
            let enabled = items.as_ref().map_or(0, |i| {
                i.values()
                    .filter(|spec| spec.get("enabled").and_then(|v| v.as_bool()) != Some(false))
                    .count()
            });
            log.write(&format!("PLAYER HUD registered items key={key:?} count={} enabled={enabled} failure={failure:?}; owned copies from live AI callback",items.as_ref().map_or(0,|i| i.len())));
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
    /// The last sample for this match and player. When samples stop while
    /// the battlefield shows (every actor stops calling think while the
    /// nexus explodes), the last values stay frozen, as in a pause (the
    /// author, 2026-10-20, option a: it blanked to "-"). Timers are never
    /// extrapolated from wall time.
    pub fn snapshot(&self, identity: Option<(MatchKey, usize)>) -> Option<Snapshot> {
        let (key, player) = identity?;
        let s = self.0.lock().ok()?;
        let (snapshot, _) = s.as_ref()?;
        (snapshot.key == key && snapshot.player == player).then(|| snapshot.clone())
    }
    /// `snapshot`, only while samples arrive (the AI-control state plate,
    /// where a stale dead state must not stick).
    pub fn fresh_snapshot(&self, identity: Option<(MatchKey, usize)>) -> Option<Snapshot> {
        let at = self.0.lock().ok()?.as_ref()?.1;
        (at.elapsed() <= Duration::from_millis(250))
            .then(|| self.snapshot(identity))
            .flatten()
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
/// Tenths below three seconds ("2.4"), whole seconds from three ("12"),
/// both rounded up so the number never reaches zero before the skill is ready.
fn cooldown(ticks: usize) -> String {
    let tenths = ticks.div_ceil(6);
    if ticks == 0 {
        crate::lang::tr("Ready").into()
    } else if tenths < 30 {
        format!("{}.{}", tenths / 10, tenths % 10)
    } else {
        ticks.div_ceil(60).to_string()
    }
}
/// How far the drain edge has warmed (0 white, 1 signal yellow).
fn edge_warmth(remaining: usize, peak: usize) -> f32 {
    let progress = 1. - remaining as f32 / peak.max(1) as f32;
    ((progress - EDGE_WARM_FROM) / (1. - EDGE_WARM_FROM)).clamp(0., 1.)
}
/// Keep the moving edge from passing the ready cue's resting position.
/// The shade still drains all the way to zero; this only positions the line.
fn cooldown_edge_y(drain: f32, ready_cue: bool) -> f32 {
    if ready_cue {
        READY_EDGE_Y
    } else {
        drain.max(READY_EDGE_Y)
    }
}
/// The ready cue `seconds` after the skill became ready: (edge opacity,
/// icon lift from the cooling brightness, 0 to 1), or None once it is over.
fn ready_cue(seconds: f32) -> Option<(f32, f32)> {
    let fade = ((seconds - READY_BEAT) / READY_FADE).clamp(0., 1.);
    (seconds < READY_BEAT + READY_FADE).then(|| {
        let eased = crate::hud_motion::easing(fade, true);
        (1. - eased, eased)
    })
}
fn ready_flash_strength(seconds: f32) -> f32 {
    1. - crate::hud_motion::easing(
        ((seconds - READY_FLASH_HOLD) / READY_FLASH_FADE).clamp(0., 1.),
        true,
    )
}
/// Whether the end of a cooldown that peaked at `peak` ticks asks for the chime.
fn chimes(peak: usize) -> bool {
    peak >= READY_CHIME_MIN_TICKS
}
/// An opaque grey at `level` (0 black, 1 white), as `rrggbbaa`.
fn grey(level: f32) -> String {
    let v = (level.clamp(0., 1.) * 255.).round() as u8;
    format!("{v:02x}{v:02x}{v:02x}ff")
}
/// A slot shows "--": not learned, or the champion's data says it has no
/// effect. Death is not unavailability: its cooldowns keep showing, like
/// League (the author, 2026-10-20: all "-" while dead).
fn skill_unavailable(s: &Snapshot, skills: HudSkills, slot: usize) -> bool {
    s.level < UNLOCK_LEVELS[slot] || skills.available[slot] == Some(false)
}

const CHARGE_MARKS: usize = 6;
const CHARGE_MARK_W: usize = 10;
const CHARGE_MARK_H: usize = 8;
const CHARGE_MARK_Y: usize = 64;
const CHARGE_MARK_GAP: usize = 2;
const DENIED_BANDS: usize = 10;
/// A short held edge warning, then a fade. Repeated presses restart it;
/// holding a key does not. This is functional feedback, like the ready cue.
fn denied_strength(seconds: f32) -> f32 {
    1. - crate::hud_motion::easing(((seconds - 0.08) / 0.22).clamp(0., 1.), true)
}
fn charge_mark_x(total: usize, mark: usize) -> usize {
    let width = total * CHARGE_MARK_W + (total - 1) * CHARGE_MARK_GAP;
    (SKILL_SIZE - width) / 2 + mark * (CHARGE_MARK_W + CHARGE_MARK_GAP)
}
/// Both feedback cues share the same permanent rings. Red takes priority;
/// gold begins at band 1 (x/y 2), inside R's existing 2 px yellow border.
fn slot_glow_color(band: usize, denied: f32, ready: f32) -> u32 {
    let feather = 1. - band as f32 / DENIED_BANDS as f32;
    let (rgb, alpha) = if denied > 0. {
        (0x8f162400, 210. * feather * feather * denied)
    } else {
        let alpha = match band {
            0 => 0.,
            1 | 2 => 230.,
            _ => 125. * feather * feather,
        };
        (0xfff1b300, alpha * ready)
    };
    rgb | alpha.round().clamp(0., 255.) as u32
}
#[derive(Debug, PartialEq)]
enum ChargeDisplay {
    Hidden,
    Marks { filled: usize, total: usize },
    Count(String),
}
fn charge_display(remaining: Option<usize>, total: Option<usize>) -> ChargeDisplay {
    let Some(remaining) = remaining else {
        return ChargeDisplay::Hidden;
    };
    match total {
        Some(total @ 2..=CHARGE_MARKS) => ChargeDisplay::Marks {
            filled: remaining.min(total),
            total,
        },
        Some(total) if total > CHARGE_MARKS => {
            ChargeDisplay::Count(format!("{} / {total}", remaining.min(total)))
        }
        None => ChargeDisplay::Count(remaining.to_string()),
        _ => ChargeDisplay::Hidden,
    }
}
/// Code-drawn feathered rings: no bitmap dependency, no change to hit areas.
fn skill_feedback_nodes() -> String {
    let mut s = String::from(
        "#slot_glow:empty { width: 80px; height: 80px; visible: false; ignore_event: true;\n",
    );
    for band in 0..DENIED_BANDS {
        let inset = 1 + band;
        let size = 78 - 2 * band;
        for (side, (x, y, w, h)) in [
            (inset, inset, size, 1),
            (inset, inset + size - 1, size, 1),
            (inset, inset + 1, 1, size - 2),
            (inset + size - 1, inset + 1, 1, size - 2),
        ]
        .into_iter()
        .enumerate()
        {
            s.push_str(&format!("#b{band}_{side}:color {{ x: {x}px; y: {y}px; width: {w}px; height: {h}px; color: #8f162400; z: 1007; ignore_event: true; }}\n"));
        }
    }
    s.push_str("}\n");
    for mark in 0..CHARGE_MARKS {
        let fill_w = CHARGE_MARK_W - 2;
        let fill_h = CHARGE_MARK_H - 2;
        s.push_str(&format!("#charge{mark}:color {{ y: {CHARGE_MARK_Y}px; width: {CHARGE_MARK_W}px; height: {CHARGE_MARK_H}px; color: #817c77ff; z: 1008; visible: false; ignore_event: true; #fill:color {{ x: 1px; y: 1px; width: {fill_w}px; height: {fill_h}px; color: #141311ff; z: 1009; ignore_event: true; }} }}\n"));
    }
    s
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
        "cooldown" | "uses" | "respawn" | "level" | "camera_key" | "target_key" | "recall_key"
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
#[cfg(test)]
pub(crate) fn template_for_test() -> String {
    template()
}
/// An inventory tile, spawned on its own when the HUD needs more slots.
#[cfg(test)]
pub(crate) fn tile_for_test() -> String {
    crate::hud_style::fonts(inventory_tile(6))
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
    let inset = crate::ui_theme::shade(5, 0xff);
    for (i, key) in KEYS.iter().enumerate() {
        let x = SKILL_X + i * SKILL_STEP;
        s.push_str(&format!("#skill{i}:color {{ x: {x}px; y: {SKILL_Y}px; width: 80px; height: 80px; z: 1001; color: #~4b4a49ff; ignore_event: true; rounding: Uniform {{ rounding: 2; }} #shadow:color {{ x: 0px; y: 3px; width: 80px; height: 80px; z: 1000; color: #00000066; ignore_event: true; }} #background:color {{ x: 1px; y: 1px; width: 78px; height: 78px; z: 1002; color: #{inset}; ignore_event: true; }} #icon:image {{ x: 2px; y: 2px; width: 74px; height: 74px; z: 1003; sample_linear: false; ignore_event: true; visible: false; }} #png:image {{ x: 2px; y: 2px; width: 74px; height: 74px; z: 1003; sample_linear: false; ignore_event: true; visible: false; }} #shade:color {{ x: 1px; y: 1px; width: 78px; height: 78px; z: 1004; color: #0a0908cc; visible: false; ignore_event: true; }} #drain_edge:color {{ x: 1px; y: 1px; width: 78px; height: 2px; z: 1005; color: #ffffffff; visible: false; ignore_event: true; }} #keycap:color {{ x: 3px; y: 3px; width: 22px; height: 22px; z: 1005; color: #eeececff; ignore_event: true; rounding: Uniform {{ rounding: 2; }} }} "));
        label(&mut s, "cooldown", (2, 25, 76, 34), 30, "ffffffff");
        // motion: A3
        s.push_str(&pass::a3_nodes());
        label(&mut s, "uses", (4, 61, 72, 18), 15, "fdee00ff");
        s.push_str(&skill_feedback_nodes());
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
    // motion: A2
    s.push_str(&pass::a2_nodes());
    // motion: B6
    s.push_str(&pass::b6_nodes(SKILL_X, SKILL_STEP, SKILL_Y));
    // motion: D1
    s.push_str(&pass::d1_nodes());
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

/// The state plate's own root: it shows while the AI drives your athlete,
/// when the HUD strip is hidden, so it cannot live under the strip's root.
pub const PLATE_PATH: &str = "ingame.lt_state_plate";
/// Centred under the game's clock (60 x 28 at y 44); its bottom edge (148)
/// stays above the kill callouts, which start at y 150.
const PLATE_Y: usize = 100;
const PLATE_H: usize = 48;
/// Glyph and key cap (or count): 116 px. The skull alone: 48 px.
const PLATE_WIDE: usize = 116;
const PLATE_NARROW: usize = 48;
/// The glyph is 30 px, inset 9 px from the top; 20 px from the left on the
/// wide plate (glyph, 10 px gap, 36 px cap, 20 px), 9 px on the narrow one.
const PLATE_GLYPH: usize = 30;
const PLATE_PAD_WIDE: usize = 20;
const PLATE_PAD_NARROW: usize = 9;
fn plate_template() -> String {
    let x = WIDTH / 2 - PLATE_WIDE / 2;
    let mut s = format!("lt_state_plate:color {{ x: {x}px; y: {PLATE_Y}px; width: {PLATE_WIDE}px; height: {PLATE_H}px; z: 1120; color: #~1e1e1df0; visible: false; ignore_event: true; rounding: Uniform {{ rounding: 2; }}\n");
    s.push_str(&crate::ui_graphics::image(
        "glyph",
        "ef_skull",
        PLATE_PAD_WIDE,
        (PLATE_H - PLATE_GLYPH) / 2,
        PLATE_GLYPH,
        1121,
    ));
    // The key cap is drawn like the strip's Tab cap; key names are not translated.
    s.push_str("#key:color { x: 60px; y: 11px; width: 36px; height: 26px; z: 1121; color: #eeececff; visible: false; ignore_event: true; rounding: Uniform { rounding: 2; } #text:label { @\"asset/base/style/main#bold_label\"; width: 100%; height: 100%; size: 14; align_x: Center; align_y: Center; color: #393939ff; text: \"F11\"; ignore_event: true; z: 1122; } }\n");
    s.push_str("#count:label { @\"asset/base/style/main#bold_label\"; x: 56px; y: 8px; width: 44px; height: 32px; size: 23; align_x: Center; align_y: Center; color: #ffffffff; text: \"\"; visible: false; ignore_event: true; z: 1122; }\n}\n");
    crate::hud_style::fonts(s)
}
#[cfg(test)]
pub(crate) fn plate_template_for_test() -> String {
    plate_template()
}
/// The state plate (S1): paused, dead or the AI driving, under the game's
/// clock. A feature, shown with "HUD motion" Off too. It takes no clicks
/// and is not in the camera's blocked rectangles.
#[derive(Default)]
pub struct StatePlate {
    cache: HashMap<String, String>,
    spawn_attempted: Option<Instant>,
    /// The last plate shown, kept while it fades out.
    last: Option<Plate>,
    // motion: pass
    /// The HUD motion pass (hud_motion_pass.rs).
    pass: pass::Pass,
}
impl StatePlate {
    fn props(&mut self, ctx: &mut StableClient<'_>, node: &str, source: String) {
        let path = if node.is_empty() {
            PLATE_PATH.to_owned()
        } else {
            format!("{PLATE_PATH}.{node}")
        };
        crate::hud_motion::properties(ctx, &mut self.cache, &path, &source);
    }
    /// Per client frame. `phase`: the session phase while you play or
    /// watch (None otherwise); `snapshot`: your athlete, fresh or None.
    pub fn apply(
        &mut self,
        ctx: &mut StableClient<'_>,
        phase: Option<Phase>,
        snapshot: Option<&Snapshot>,
        log: &Logger,
    ) {
        let state = plate_state(
            phase,
            snapshot.map(|s| s.alive),
            snapshot.map_or(0, |s| s.respawn),
        );
        if phase.is_none() {
            self.last = None;
            self.pass.reset(); // motion: pass
        }
        if state.is_some() {
            self.last = state;
        }
        // motion: pass
        self.pass.frame();
        // motion: S1 (restore `let alpha = f32::from(u8::from(state.is_some()));`)
        let alpha = pass::s1_plate_fade(&mut self.pass, state.is_some());
        let Some(plate) = self.last.filter(|_| alpha > 0.) else {
            // Write only on a change (this runs every frame).
            if ctx.ui_visible(PLATE_PATH) != Some(false) && ctx.ui_exists(PLATE_PATH) {
                ctx.ui_set_visible(PLATE_PATH, false);
            }
            return;
        };
        if !ctx.ui_exists(PLATE_PATH) {
            if self
                .spawn_attempted
                .is_some_and(|t| t.elapsed() < Duration::from_secs(1))
            {
                return;
            }
            self.spawn_attempted = Some(Instant::now());
            self.cache.clear();
            let spawned =
                ctx.ui_spawn_source("ingame", &crate::ui_theme::themed(&plate_template()));
            log.write(&format!(
                "STATE PLATE spawn={spawned} exists={}",
                ctx.ui_exists(PLATE_PATH)
            ));
            if !spawned || !ctx.ui_exists(PLATE_PATH) {
                return;
            }
        }
        if ctx.ui_visible(PLATE_PATH) != Some(true) {
            ctx.ui_set_visible(PLATE_PATH, true);
        }
        let (glyph, ink, key, count) = match plate {
            Plate::Paused => ("ef_pause", 0xfdee00ff, true, None),
            Plate::Ai => ("ef_cpu", 0xeeececff, true, None),
            Plate::Dead(seconds) => ("ef_skull", 0xeeececff, false, seconds),
        };
        let width = if key || count.is_some() {
            PLATE_WIDE
        } else {
            PLATE_NARROW
        };
        // The plate's own alpha (it must not need the motion module).
        let a = |color: u32| (color & !255) | ((color & 255) as f32 * alpha).round() as u32;
        self.props(
            ctx,
            "",
            format!(
                "x: {}px; width: {width}px; color: #~1e1e1d{:02x};",
                WIDTH / 2 - width / 2,
                (240. * alpha).round() as u8
            ),
        );
        self.props(
            ctx,
            "glyph",
            format!(
                "x: {}px; source: \"asset/lt_direct_control/ui/{glyph}\"; color: #{:08x};",
                if width == PLATE_WIDE {
                    PLATE_PAD_WIDE
                } else {
                    PLATE_PAD_NARROW
                },
                a(ink)
            ),
        );
        self.props(
            ctx,
            "key",
            format!("visible: {key}; color: #{:08x};", a(0xeeececff)),
        );
        self.props(ctx, "key.text", format!("color: #{:08x};", a(0x393939ff)));
        self.props(
            ctx,
            "count",
            format!(
                "visible: {}; color: #{:08x};",
                count.is_some(),
                a(0xffffffff)
            ),
        );
        let text = count.map_or(String::new(), |n| n.to_string());
        let path = format!("{PLATE_PATH}.count");
        if self.cache.get(&path) != Some(&text) && ctx.ui_set_text(&path, &text) {
            self.cache.insert(path, text);
        }
    }
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
    // motion: pass (restore private: A2 writes from client.rs)
    pub(crate) cache: HashMap<String, String>,
    // motion: pass
    /// The HUD motion pass (hud_motion_pass.rs).
    pub(crate) pass: pass::Pass,
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
    /// Last frame: the slot was learned, usable and recovering a cooldown.
    cooling: [bool; 3],
    /// When each slot last became ready, for the edge cue and 400 ms glow.
    ready_at: [Option<Instant>; 3],
    motion_identity: Option<(MatchKey, usize)>,
    shop_pressed: bool,
    shop_click: bool,
    /// Manual shopping with an empty queue: no next purchase is shown.
    purchase_blank: bool,
}
impl HudUi {
    /// Keep the existing zero-usable-to-ready trigger for both visual cues.
    /// The return value is the existing optional chime request.
    fn track_readiness(
        &mut self,
        slot: usize,
        live: bool,
        ready: bool,
        remaining: usize,
        now: Instant,
    ) -> bool {
        let recovered = live && ready && self.cooling[slot];
        let chime = recovered && chimes(self.cooldown_peak[slot]);
        if recovered {
            self.ready_at[slot] = Some(now);
        }
        if ready {
            self.cooldown_peak[slot] = 0;
        } else {
            self.cooldown_peak[slot] = self.cooldown_peak[slot].max(remaining);
        }
        self.cooling[slot] = live && !ready && self.cooldown_peak[slot] >= READY_CUE_MIN_TICKS;
        chime
    }
    fn skill_feedback(
        &mut self,
        ctx: &mut StableClient<'_>,
        slot: usize,
        denied_at: Option<Instant>,
        ready_flash: f32,
        charges: ChargeDisplay,
        log: &Logger,
    ) {
        let denied = denied_at.map_or(0., |at| denied_strength(at.elapsed().as_secs_f32()));
        let shown = denied > 0. || ready_flash > 0.;
        self.props(
            ctx,
            &format!("skill{slot}.slot_glow"),
            format!("visible: {shown};"),
            log,
        );
        if shown {
            for band in 0..DENIED_BANDS {
                let color = slot_glow_color(band, denied, ready_flash);
                for side in 0..4 {
                    self.props(
                        ctx,
                        &format!("skill{slot}.slot_glow.b{band}_{side}"),
                        format!("color: #{color:08x};"),
                        log,
                    );
                }
            }
        }
        for mark in 0..CHARGE_MARKS {
            let (visible, filled, x) = match charges {
                ChargeDisplay::Marks { filled, total } => {
                    (mark < total, mark < filled, charge_mark_x(total, mark))
                }
                _ => (false, false, 0),
            };
            self.props(
                ctx,
                &format!("skill{slot}.charge{mark}"),
                format!("visible: {visible}; x: {x}px;"),
                log,
            );
            if visible {
                self.props(
                    ctx,
                    &format!("skill{slot}.charge{mark}.fill"),
                    format!("color: #{};", if filled { "fdee00ff" } else { "141311ff" }),
                    log,
                );
            }
        }
        self.text(
            ctx,
            &format!("skill{slot}.uses"),
            match charges {
                ChargeDisplay::Count(text) => text,
                _ => String::new(),
            },
            log,
        );
    }
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
            if crate::hud_motion::unchanged(&self.cache, &format!("{PATH}.{node}"), &properties) {
                continue;
            }
            let applied = self.props(ctx, &node, properties.clone(), log);
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
    /// True once the host holds `source` (written now or before).
    fn props(
        &mut self,
        ctx: &mut StableClient<'_>,
        node: &str,
        source: String,
        log: &Logger,
    ) -> bool {
        let applied =
            crate::hud_motion::properties(ctx, &mut self.cache, &format!("{PATH}.{node}"), &source);
        if !applied && !self.failed {
            log.write(&format!("PLAYER HUD property failed node={node}"));
            self.failed = true;
        }
        applied
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
                crate::lang::tr("Manual shopping: nothing queued. Open the shop (P) to buy.")
                    .into(),
            ),
            _ if manual.is_some() => {
                let (target, next, price, gold) = manual.clone().flatten().expect("queued step");
                let short = price.saturating_sub(gold);
                let tooltip = format!(
                    "{}\n{}\n{}",
                    name(&target),
                    crate::lang::trf(
                        "Next: {item} · {price} gold",
                        &[("item", &name(&next)), ("price", &price)]
                    ),
                    if short == 0 {
                        crate::lang::tr("Buys at base").to_owned()
                    } else {
                        crate::lang::trf("{gold} more gold", &[("gold", &short)])
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
                        "{}\n{}\n",
                        crate::lang::trf(
                            "Next: {item} · {price} gold",
                            &[("item", &name(&next.key)), ("price", &next.price)]
                        ),
                        crate::lang::trf(
                            "{gold} more gold",
                            &[("gold", &next.price.saturating_sub(s.gold))]
                        )
                    ));
                }
                if !f.affordable.is_empty() {
                    tooltip.push_str(crate::lang::tr("At base now:"));
                    tooltip.push('\n');
                    for p in &f.affordable {
                        tooltip.push_str(&crate::lang::trf(
                            "{item} · {price} gold",
                            &[("item", &name(&p.key)), ("price", &p.price)],
                        ));
                        tooltip.push('\n');
                    }
                }
                if f.branching {
                    tooltip.push_str(crate::lang::tr(
                        "The game chooses an upgrade branch when buying:",
                    ));
                    tooltip.push('\n');
                    tooltip.push_str(crate::lang::tr("A + after the HUD amount means the minimum shortfall; the chosen branch may cost more."));
                    tooltip.push('\n');
                    for p in &f.alternatives {
                        tooltip.push_str(&crate::lang::trf(
                            "{item} · {price} gold · {gold} more gold",
                            &[
                                ("item", &name(&p.key)),
                                ("price", &p.price),
                                ("gold", &p.price.saturating_sub(s.gold)),
                            ],
                        ));
                        tooltip.push('\n');
                    }
                    tooltip.push_str(crate::lang::tr("Further purchases are not yet fixed."));
                }
                (self.item_icon(key), text, progress, tooltip)
            }
            (Some(_), Some(_)) => (
                None,
                "✓".into(),
                100.,
                crate::lang::tr("Automatic build complete").into(),
            ),
            _ => (
                None,
                "—".into(),
                0.,
                crate::lang::tr("Automatic purchase information is not available yet.").into(),
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
        let allowed = tooltip_allowed(
            crate::settings::current_shared().number("hud_tooltips"),
            crate::ui_state::HUD_TOOLTIP_KEY.load(Ordering::Relaxed),
            crate::ui_state::POINTER_BUTTON.load(Ordering::Relaxed),
        );
        let hovered = cursor.filter(|_| allowed).and_then(|cursor| {
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
        let mut full = self.tooltip_text[&key].clone();
        // Skill values computed from your current stats, like League; Ctrl
        // shows the game's own "base + percentage" wording.
        if skill && !crate::ui_state::DETAIL_KEY.load(Ordering::Relaxed) {
            if let Some(own) = crate::stats_panel::snapshot().0 {
                full = crate::tooltips::computed(&full, own.values[0], own.values[1]);
            }
        }
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
                crate::lang::tr("Purchase information unavailable")
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
            self.pass.reset(); // motion: pass
            self.motion_identity = None;
            self.spawn_attempted = None;
            // Write only on a change (this runs every frame).
            if ctx.ui_visible(PATH) != Some(false) && ctx.ui_exists(PATH) {
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
        self.pass.frame(); // motion: pass
                           // motion: B6, D1
        let shown = snapshot.filter(|_| playing);
        if let Some(s) = shown {
            if self.motion_identity != Some((s.key, s.player)) {
                self.motion.reset();
                self.pass.reset(); // motion: pass
                self.cooldown_peak = [0; 3];
                self.cooling = [false; 3];
                self.ready_at = [None; 3];
                self.motion_identity = Some((s.key, s.player));
            }
        }
        // motion: B6
        let b6 = pass::b6_death_dim(&mut self.pass, shown.map(|s| s.alive), playing);
        pass::write(ctx, &mut self.cache, PATH, b6);
        // motion: D1
        let d1 = pass::d1_stats_hairline(&mut self.pass, shown.map(|s| s.kda.0), playing);
        pass::write(ctx, &mut self.cache, PATH, d1);
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
                self.skill_feedback(ctx, i, None, 0., ChargeDisplay::Hidden, log);
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
            // motion: B2 (restore `s.gold`)
            ("gold", pass::b2_gold_roll(&mut self.pass, s.gold)),
            ("level", s.level),
        ] {
            self.text(ctx, node, value.to_string(), log);
        }
        // motion: B4
        let level = pass::b4_level_lift(&mut self.pass, s.level, 0xffffffff);
        self.props(ctx, "level", format!("color: #{level:08x};"), log);
        self.purchase(ctx, Some(s), log);
        for node in ["coin", "purchase_coin", "purchase_check"] {
            // motion: A1 (restore one `color: #fdee00ff` for all three)
            let color = if node == "coin" {
                0xfdee00ff
            } else {
                pass::a1_purchase_breathe(&mut self.pass, self.purchase_ready, 0xfdee00ff)
            };
            self.props(ctx, node, format!("color: #{color:08x};"), log);
        }
        self.props(ctx, "hourglass", format!("visible: {};", !s.alive), log);
        self.props(ctx, "respawn", format!("visible: {};", !s.alive), log);
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
        // motion: B1 (restore the trail write without `color`)
        let trail_color = pass::b1_damage_chunk(&mut self.pass, hp_percent(s), 0xefe6cfff);
        self.props(
            ctx,
            "health.trail",
            format!("width: {trail:.2}%; color: #{trail_color:08x};"),
            log,
        );
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
            // Ready cues and sounds wait for the respawn.
            let live = learned && !unavailable && s.alive;
            if self.track_readiness(i, live, ready, remaining, Instant::now()) {
                crate::ui_sound::ready();
            }
            // motion: B4
            let (b4_shade, b4_cue) = pass::b4_learned_drain(&mut self.pass, i, learned);
            if b4_cue && live && ready {
                self.ready_at[i] = Some(Instant::now());
            }
            let ready_age = self.ready_at[i]
                .filter(|_| live && ready)
                .map(|at| at.elapsed().as_secs_f32());
            let cue = ready_age.and_then(ready_cue);
            let ready_flash = ready_age.map_or(0., ready_flash_strength);
            if ready_flash <= 0. {
                self.ready_at[i] = None;
            }
            let drain = if unavailable {
                78.
            } else {
                78. * remaining as f32 / self.cooldown_peak[i].max(1) as f32
            };
            let edge_y = cooldown_edge_y(drain, cue.is_some());
            let edge = match cue {
                Some((opacity, _)) => format!("fdee00{:02x}", (opacity * 255.).round() as u8),
                None => crate::hud_motion::color(
                    0xffffffff,
                    0xfdee00ff,
                    edge_warmth(remaining, self.cooldown_peak[i]),
                ),
            };
            self.props(
                ctx,
                &format!("skill{i}.drain_edge"),
                // The cue sits just inside the slot's border, which is 2 px
                // and yellow on a ready R.
                format!(
                    "visible: {}; y: {:.2}px; color: #{edge};",
                    (!unavailable && !ready) || cue.is_some(),
                    edge_y
                ),
                log,
            );
            // motion: A3
            let edge = u32::from_str_radix(&edge, 16).unwrap_or(0xffffffff);
            let a3 = pass::a3_drain_comet(
                &mut self.pass,
                i,
                !unavailable && !ready && cue.is_none(),
                edge_y,
                edge,
            );
            pass::write(ctx, &mut self.cache, PATH, a3);
            // motion: C1 (node and hover moved up from the border code below)
            let node = format!("skill{i}");
            let hover = self.tile_hover(ctx, &node, cursor);
            // motion: C1
            let c1 = pass::c1_slot_hover(&mut self.pass, i, hover);
            self.props(
                ctx,
                &format!("{node}.keycap"),
                format!(
                    "color: #{};",
                    crate::hud_motion::color(0xeeececff, 0xffffffff, c1)
                ),
                log,
            );
            let brightness = match cue {
                Some((_, lift)) => COOLING_ICON + (1. - COOLING_ICON) * lift,
                // motion: B4 (restore `None if live && !ready`)
                None if live && (!ready || b4_shade.is_some()) => COOLING_ICON,
                None => 1.,
            };
            // motion: B4, C1 (restore the 1.3.2 shade write)
            let shade = if learned { 0x0a0908cc } else { 0x141311dd };
            self.props(
                ctx,
                &format!("skill{i}.shade"),
                format!(
                    "visible: {}; height: {:.2}px; color: #{:08x};",
                    unavailable || !ready || b4_shade.is_some(),
                    b4_shade.unwrap_or(drain),
                    pass::fade(shade, 1. - 0.3 * c1)
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
            self.skill_feedback(
                ctx,
                i,
                skills.denied_at[i].filter(|_| s.alive),
                ready_flash,
                charge_display(skills.uses[i].filter(|_| !unavailable), skills.uses_max[i]),
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
                    // The press tint (85 %) on top of the cooldown brightness.
                    format!(
                        "width: 74px; height: 74px; color: #{};",
                        grey(brightness * (1. - 0.15 * press))
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
                // motion: B5 (restore the 1.3.2 border write)
                let item = s.items.get(i).map(String::as_str);
                let (b5_alpha, b5_edge) = pass::b5_item_bought(&mut self.pass, i, item);
                let border =
                    crate::hud_motion::color(crate::ui_theme::tone(0x4b4a49ff), 0xffffffff, h);
                let border = u32::from_str_radix(&border, 16).unwrap_or(0x4b4a49ff);
                self.props(
                    ctx,
                    &node,
                    format!(
                        "color: #{};",
                        crate::hud_motion::color(border, 0xfdee00ff, b5_edge)
                    ),
                    log,
                );
                // motion: C2, B5
                let brightness = pass::c2_item_press(&self.pass, press);
                self.props(
                    ctx,
                    &format!("{node}.icon"),
                    pass::icon_tint(brightness, b5_alpha),
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
                    255.
                } else {
                    140. + 115. * h
                };
                // motion: B5 (restore `opacity` as before)
                let source = self.purchase_icon.as_ref().map(|i| i.source.clone());
                let b5 = pass::b5_next_item(&mut self.pass, source.as_deref());
                let opacity = (opacity * b5).round() as u8;
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
    fn larger_charge_rows_stay_centred_inside_the_slot_clear_of_the_countdown() {
        for total in 2..=CHARGE_MARKS {
            let left = charge_mark_x(total, 0);
            let right = charge_mark_x(total, total - 1) + CHARGE_MARK_W;
            assert!(left >= 2 && right <= SKILL_SIZE - 2);
            assert_eq!(left, SKILL_SIZE - right);
            for mark in 1..total {
                assert_eq!(
                    charge_mark_x(total, mark) - charge_mark_x(total, mark - 1),
                    CHARGE_MARK_W + CHARGE_MARK_GAP
                );
            }
        }
        const {
            assert!(CHARGE_MARK_Y > 25 + 34); // Cooldown label's bottom edge.
            assert!(CHARGE_MARK_Y + CHARGE_MARK_H <= SKILL_SIZE - 2);
        }
    }
    #[test]
    fn ready_flash_fades_once_and_red_feedback_keeps_priority() {
        assert_eq!(ready_flash_strength(0.), 1.);
        assert_eq!(ready_flash_strength(0.1), 1.);
        assert!(ready_flash_strength(0.25) > 0. && ready_flash_strength(0.25) < 1.);
        assert_eq!(ready_flash_strength(0.41), 0.);
        assert_eq!(slot_glow_color(0, 0., 1.) & 255, 0); // Inside R's border.
        assert!(slot_glow_color(1, 0., 1.) & 255 > slot_glow_color(8, 0., 1.) & 255);
        for band in 0..DENIED_BANDS {
            assert_eq!(
                slot_glow_color(band, 0.5, 1.),
                slot_glow_color(band, 0.5, 0.)
            );
            assert_eq!(slot_glow_color(band, 0., ready_flash_strength(1.)) & 255, 0);
        }
        // The existing line/icon cue can finish before the new slot glow.
        assert!(ready_cue(0.25).is_none());
        assert!(ready_flash_strength(0.25) > 0.);
    }
    #[test]
    fn slot_flash_tracks_zero_to_usable_and_does_not_replay_for_extra_charges() {
        let mut hud = HudUi::default();
        let now = Instant::now();
        assert!(!hud.track_readiness(0, true, true, 0, now));
        assert!(hud.ready_at[0].is_none()); // Already ready at first sight.
        assert!(!hud.track_readiness(0, true, false, 120, now)); // Zero usable charges.
        hud.track_readiness(0, true, false, 6, now);
        assert!(!hud.track_readiness(0, true, true, 0, now)); // One charge, no long-cooldown chime.
        assert_eq!(hud.ready_at, [Some(now), None, None]);
        let later = now + Duration::from_millis(200);
        hud.track_readiness(0, true, true, 0, later); // Extra charge, still usable.
        assert_eq!(hud.ready_at[0], Some(now));
        hud.ready_at[0] = None; // Render finished the cue.
        hud.track_readiness(0, true, true, 0, later);
        assert!(hud.ready_at[0].is_none()); // No repeated ready glow.
        hud.track_readiness(1, true, false, 300, now);
        assert!(hud.track_readiness(1, true, true, 0, later)); // Existing 4 s chime gate.
        hud.track_readiness(2, true, false, 120, now);
        hud.track_readiness(2, false, true, 0, later); // Dead/unavailable.
        assert!(hud.ready_at[2].is_none());
    }
    #[test]
    fn unavailable_cue_fades_and_charge_marks_handle_empty_unknown_and_large_counts() {
        assert_eq!(denied_strength(0.), 1.);
        assert_eq!(denied_strength(0.08), 1.);
        assert!(denied_strength(0.19) > 0. && denied_strength(0.19) < 1.);
        assert_eq!(denied_strength(0.31), 0.);
        assert_eq!(
            charge_display(Some(0), Some(3)),
            ChargeDisplay::Marks {
                filled: 0,
                total: 3
            }
        );
        assert_eq!(
            charge_display(Some(2), Some(3)),
            ChargeDisplay::Marks {
                filled: 2,
                total: 3
            }
        );
        assert_eq!(
            charge_display(Some(99), Some(3)),
            ChargeDisplay::Marks {
                filled: 3,
                total: 3
            }
        );
        assert_eq!(
            charge_display(Some(7), Some(8)),
            ChargeDisplay::Count("7 / 8".into())
        );
        assert_eq!(
            charge_display(Some(2), None),
            ChargeDisplay::Count("2".into())
        );
        assert_eq!(charge_display(None, Some(3)), ChargeDisplay::Hidden);
        assert_eq!(charge_display(Some(1), Some(1)), ChargeDisplay::Hidden);
    }
    #[test]
    fn hud_has_no_match_start_effect_nodes() {
        let source = template();
        for removed in ["b7_veil", "s4_left", "s4_right"] {
            assert!(!source.contains(removed));
        }
        assert!(source.contains("#slot_glow:empty") && source.contains("#charge5:color"));
        assert!(source.contains("#d1_line")); // Kill feedback stays.
    }
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
    fn state_plate_prefers_paused_then_dead_then_ai_and_counts_from_ticks() {
        use Phase::*;
        // Paused wins over dead (and over everything else).
        assert_eq!(
            plate_state(Some(Paused), Some(false), 600),
            Some(Plate::Paused)
        );
        assert_eq!(
            plate_state(Some(Paused), Some(true), 0),
            Some(Plate::Paused)
        );
        // Dead wins over AI; whole seconds rounded up from ticks.
        assert_eq!(
            plate_state(Some(Ai), Some(false), 601),
            Some(Plate::Dead(Some(11)))
        );
        assert_eq!(
            plate_state(Some(Running), Some(false), 60),
            Some(Plate::Dead(Some(1)))
        );
        // An unknown count is not invented: the skull alone.
        assert_eq!(
            plate_state(Some(Running), Some(false), 0),
            Some(Plate::Dead(None))
        );
        assert_eq!(plate_state(Some(Ai), Some(true), 0), Some(Plate::Ai));
        assert_eq!(plate_state(Some(Ai), None, 0), Some(Plate::Ai));
        // Playing and alive, or not in a match: no plate.
        for (phase, alive) in [
            (Some(Running), Some(true)),
            (Some(Running), None),
            (Some(Ready), Some(false)),
            (Some(Loading), None),
            (None, Some(false)),
        ] {
            assert_eq!(plate_state(phase, alive, 300), None);
        }
        assert!(!template().contains("death_banner"));
        let plate = plate_template();
        assert!(plate.contains("#key:color") && plate.contains("text: \"F11\""));
        // Lower and bigger (1.3.6): still centred, still clear of the kill callouts.
        assert!(plate.contains("x: 902px; y: 100px; width: 116px; height: 48px;"));
        assert!(plate.contains("width: 36px; height: 26px;"));
        const { assert!(PLATE_Y + PLATE_H < 150) };
        const { assert!(PLATE_PAD_WIDE * 2 + PLATE_GLYPH + 10 + 36 == PLATE_WIDE) };
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
        let s = hud.snapshot(Some((first.key, 8))).unwrap();
        assert_eq!(s.champion, "harpy");
        assert_eq!(s.gold, 999);
        assert!(hud.snapshot(Some((first.key, 7))).is_none());
        hud.select_prepared((1, 2, 4), 8);
        assert!(hud.snapshot(Some(((1, 2, 4), 8))).is_none());
        hud.reset_session();
        hud.select_prepared(first.key, 8);
        assert!(hud.snapshot(Some((first.key, 8))).is_none());
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
        assert!(hud.snapshot(Some((s.key, s.player))).is_some());
        assert!(!hud.needs_sample(s.key, s.player, 6));
        hud.reset_session();
        assert!(hud.snapshot(Some((s.key, s.player))).is_none());
        assert!(hud.needs_sample(s.key, s.player, 6));
    }
    #[test]
    fn snapshots_are_match_and_player_scoped_and_stay_frozen_when_samples_stop() {
        let hud = PlayerHud::default();
        hud.observe(sample());
        assert!(hud.snapshot(Some(((1, 2, 3), 7))).is_some());
        assert!(hud.snapshot(Some(((1, 2, 4), 7))).is_none());
        assert!(hud.snapshot(Some(((1, 2, 3), 6))).is_none());
        hud.0.lock().unwrap().as_mut().unwrap().1 = Instant::now() - Duration::from_secs(1);
        // Samples stopped (nexus explosion, pause): the last values stay.
        assert_eq!(hud.snapshot(Some(((1, 2, 3), 7))).unwrap().hp, sample().hp);
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
        assert!(hud.snapshot(identity).is_some());
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
        let s = hud.snapshot(Some(((1, 2, 3), 7))).unwrap();
        assert_eq!(s.champion, "lancer");
        assert_eq!(s.hp, None);
        assert!(!s.alive);
        dead.key = (1, 2, 4);
        hud.observe(dead);
        assert!(hud
            .snapshot(Some(((1, 2, 4), 7)))
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
        assert_eq!(hud.snapshot(Some(((1, 2, 3), 7))).unwrap().respawn, 600);
        hud.0.lock().unwrap().as_mut().unwrap().1 = Instant::now() - Duration::from_secs(1);
        // Samples stopped: the actual timer stays frozen; no guessed
        // wall-clock countdown.
        let retained = hud.snapshot(Some(((1, 2, 3), 7))).unwrap();
        assert_eq!(retained.champion, "lancer");
        assert_eq!(retained.items.len(), 2);
        assert_eq!(retained.respawn, 600);
        assert!(retained.hp.is_none());
        assert!(hud.fresh_snapshot(Some(((1, 2, 3), 7))).is_none());
        hud.observe(sample());
        assert!(hud.snapshot(Some(((1, 2, 3), 7))).unwrap().alive);
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
        assert_eq!(cooldown(1), "0.1");
        assert_eq!(cooldown(6), "0.1");
        assert_eq!(cooldown(61), "1.1");
        // Tenths until the rounded-up value reaches three seconds.
        assert_eq!(cooldown(174), "2.9");
        assert_eq!(cooldown(175), "3");
        assert_eq!(cooldown(180), "3");
        assert_eq!(cooldown(181), "4");
        assert_eq!(cooldown(720), "12");
    }
    #[test]
    fn cooldown_edge_never_steps_back_when_recovery_becomes_the_ready_cue() {
        // Ten-second cooldown, including its final sampled ticks and the
        // ready frame (whose observed cooldown peak has already been reset).
        let mut previous = 78.;
        for remaining in [600, 300, 60, 12, 6, 1, 0] {
            let y = cooldown_edge_y(78. * remaining as f32 / 600., remaining == 0);
            assert!(
                y <= previous,
                "edge moved down at {remaining} ticks: {previous} -> {y}"
            );
            assert!(y >= READY_EDGE_Y);
            previous = y;
        }
        assert_eq!(previous, READY_EDGE_Y);
        assert_eq!(cooldown_edge_y(39., false), 39.); // Mid-cycle position stays.
        assert_eq!(cooldown_edge_y(0., false), READY_EDGE_Y); // Short cycle, no ready cue.
        assert_eq!(cooldown_edge_y(0., true), READY_EDGE_Y);
        // At a low frame rate the last sample may still be above the stop;
        // becoming ready can only move toward the top, never back down.
        assert!(cooldown_edge_y(0., true) <= cooldown_edge_y(7.8, false));
    }
    #[test]
    fn drain_edge_warms_over_the_last_tenth_and_the_ready_cue_holds_then_fades() {
        assert_eq!(edge_warmth(100, 100), 0.);
        assert_eq!(edge_warmth(10, 100), 0.);
        assert!((edge_warmth(5, 100) - 0.5).abs() < 1e-4);
        assert_eq!(edge_warmth(0, 100), 1.);
        assert_eq!(edge_warmth(3, 0), 0.);
        // One beat of full yellow with the icon still at its cooling level.
        assert_eq!(ready_cue(0.), Some((1., 0.)));
        assert_eq!(ready_cue(0.08), Some((1., 0.)));
        let (edge, lift) = ready_cue(0.133).unwrap();
        assert!(edge > 0. && edge < 1. && (edge + lift - 1.).abs() < 1e-5);
        assert_eq!(ready_cue(0.183), None);
        // The chime: cooldowns of 4 s (240 ticks) or more only.
        assert!(!chimes(READY_CUE_MIN_TICKS));
        assert!(!chimes(120));
        assert!(!chimes(239));
        assert!(chimes(240));
        assert!(chimes(3600));
        assert_eq!(grey(1.), "ffffffff");
        assert_eq!(grey(COOLING_ICON * 0.85), "a3a3a3ff");
    }
    #[test]
    fn hud_tooltips_follow_the_setting_and_never_show_with_a_button_held() {
        // On hover (default), hold key and hover, off.
        assert!(tooltip_allowed(0., false, false));
        assert!(!tooltip_allowed(1., false, false));
        assert!(tooltip_allowed(1., true, false));
        assert!(!tooltip_allowed(2., true, false));
        // A click or drag passing over the HUD.
        assert!(!tooltip_allowed(0., false, true));
        assert!(!tooltip_allowed(1., true, true));
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
                                                   // Death keeps the cooldowns showing, like League (the author, 2026-10-20).
        s.alive = false;
        assert_eq!(
            std::array::from_fn::<_, 3, _>(|i| skill_unavailable(&s, skills, i)),
            [false, true, false]
        );
    }
}
