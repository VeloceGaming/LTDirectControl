//! Selected-player scalar snapshots and a scoped native UI overlay.
//! No host context or entity pointer is retained across callbacks.
use crate::{
    abilities::HudSkills,
    camera::Rect,
    hud_icons::{self, Icon},
    // motion: pass
    hud_motion_pass as pass,
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
pub static TOOLTIP_SCROLL: AtomicI32 = AtomicI32::new(0);
/// Overflow panel and its source tile, in screen coordinates, for wheel routing.
pub static TOOLTIP_SCROLL_AREA: Mutex<Option<(Rect, Rect)>> = Mutex::new(None);

pub use crate::state_plate::StatePlate;
#[cfg(test)]
use crate::{
    native_timing::Phase,
    state_plate::{plate_state, Plate},
};

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
    /// The last sample, shared with the client frame without copying it.
    Mutex<Option<(Arc<Snapshot>, Instant)>>,
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
            s.key == key
                && (s.items.is_some() || crate::clock::since(s.read_at) < Duration::from_secs(2))
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
            read_at: crate::clock::now(),
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
            *current = snapshot.map(|s| (Arc::new(s), crate::clock::now()));
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
            *s = Some((Arc::new(snapshot), crate::clock::now()));
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
    pub fn snapshot(&self, identity: Option<(MatchKey, usize)>) -> Option<Arc<Snapshot>> {
        let (key, player) = identity?;
        let s = self.0.lock().ok()?;
        let (snapshot, _) = s.as_ref()?;
        (snapshot.key == key && snapshot.player == player).then(|| snapshot.clone())
    }
    /// `snapshot`, only while samples arrive (the AI-control state plate,
    /// where a stale dead state must not stick).
    pub fn fresh_snapshot(&self, identity: Option<(MatchKey, usize)>) -> Option<Arc<Snapshot>> {
        let at = self.0.lock().ok()?.as_ref()?.1;
        (crate::clock::since(at) <= Duration::from_millis(250))
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
/// The shade over a multi-use skill's slot while the next use refills and
/// some are still usable: lighter than the cooldown shade (0xcc).
const REFILL_SHADE: u32 = 0x0a090870;
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
/// An opaque grey at `level` (0 black, 1 white), as `rrggbbaa`.
fn grey(level: f32) -> String {
    let v = (level.clamp(0., 1.) * 255.).round() as u8;
    format!("{v:02x}{v:02x}{v:02x}ff")
}
/// `v` to 0.01, as a node received it (`{v:.2}px`).
use crate::hud_motion_pass::hundredths;
/// A slot shows "--": not learned, or the champion's data says it has no
/// effect. Death is not unavailability: its cooldowns keep showing, like
/// League (the author, 2026-10-20: all "-" while dead).
fn skill_unavailable(s: &Snapshot, skills: HudSkills, slot: usize) -> bool {
    s.level < UNLOCK_LEVELS[slot] || skills.available[slot] == Some(false)
}

use crate::hud_nodes::{Nodes, Tile, CHARGE_MARKS, DENIED_BANDS};
use crate::ui_writes::{Key, NodeId};
/// Write sites in the strip's cache (`ui_writes.rs`): one per distinct
/// source shape on a node. `VIS` is always `visible: {on};`.
const VIS: u8 = 0;
const S1: u8 = 1;
const CHARGE_MARK_W: usize = 10;
const CHARGE_MARK_GAP: usize = 2;
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
fn icon_updates(icon: Option<&Icon>, skill: bool) -> Vec<(&'static str, String)> {
    let target = if skill && icon.is_some_and(|i| i.tag.is_none()) {
        "png"
    } else {
        "icon"
    };
    let nodes: &[&'static str] = if skill { &["icon", "png"] } else { &["icon"] };
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
            (*node, source)
        })
        .collect()
}
fn color_source((color,): (u32,)) -> String {
    format!("color: #{color:08x};")
}
/// [`HudUi::tile_hover`] for a root rectangle: the tile's design position
/// scaled to the root.
fn tile_hover_in(
    root: Option<(f32, f32, f32, f32)>,
    tile: Tile,
    inventory_count: Option<usize>,
    cursor: Option<(f32, f32)>,
) -> bool {
    let Some(point) = cursor else { return false };
    let Some((x, y, w, h)) = root else {
        return false;
    };
    let (left, top, size) = match tile {
        Tile::Skill(i) => (
            (SKILL_X + i * SKILL_STEP) as f32,
            SKILL_Y as f32,
            SKILL_SIZE as f32,
        ),
        Tile::Next => (1390., ITEM_Y as f32, 36.),
        Tile::Item(i) => {
            let count = inventory_count.unwrap_or(0);
            if i >= count {
                return false;
            }
            let (x, y) = crate::inventory::hud_slot(count, i);
            (x as f32, y as f32, ITEM_SIZE as f32)
        }
    };
    Rect {
        x: x + left * w / WIDTH as f32,
        y: y + top * h / HEIGHT as f32,
        w: size * w / WIDTH as f32,
        h: size * h / HEIGHT as f32,
    }
    .contains(point)
}
/// An inventory tile's container and icon (its box is drawn, `hud_draw.rs`).
fn inventory_tile(i: usize) -> String {
    format!("item{i}:empty {{ x: 0px; y: 100px; width: 36px; height: 36px; z: 1001; visible: false; ignore_event: true; #icon:image {{ x: 2px; y: 2px; width: 32px; height: 32px; z: 1003; sample_linear: false; ignore_event: true; visible: false; }} }}\n")
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
/// What stays in the UI tree (1.6.0-perf-dev3): the skill, item and next
/// purchase icons (sprite-sheet images) in containers with the old tiles'
/// place and visibility, the purchase coin and check (their tint breathes,
/// A1), and the tooltip. Everything else of the strip is drawn
/// (`hud_draw.rs`) at the z its node had.
fn template() -> String {
    let mut s = format!("lt_player_hud:empty {{ width: {WIDTH}px; height: {HEIGHT}px; anchor_x: 0.5; pivot_x: 0.5; anchor_y: 1; pivot_y: 1; z: 1000; ignore_event: true;\n");
    for i in 0..3 {
        let x = SKILL_X + i * SKILL_STEP;
        s.push_str(&format!("#skill{i}:empty {{ x: {x}px; y: {SKILL_Y}px; width: 80px; height: 80px; z: 1001; ignore_event: true; #icon:image {{ x: 2px; y: 2px; width: 74px; height: 74px; z: 1003; sample_linear: false; ignore_event: true; visible: false; }} #png:image {{ x: 2px; y: 2px; width: 74px; height: 74px; z: 1003; sample_linear: false; ignore_event: true; visible: false; }} }}\n"));
    }
    for i in 0..6 {
        s.push('#');
        s.push_str(&inventory_tile(i));
    }
    s.push_str("#next:empty { x: 1390px; y: 100px; width: 36px; height: 36px; z: 1001; ignore_event: true; #icon:image { x: 3px; y: 3px; width: 30px; height: 30px; z: 1003; sample_linear: false; ignore_event: true; visible: false; } }\n");
    for id in ["purchase_coin", "purchase_check"] {
        let glyph = if id == "purchase_coin" {
            "ef_coin"
        } else {
            "ef_check"
        };
        s.push_str(&crate::ui_graphics::image(id, glyph, 1430, 121, 17, 1003));
    }
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
    // motion: pass (restore private: A2 writes from client.rs)
    pub(crate) cache: crate::ui_writes::PropCache,
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
    /// `native_tooltips::item_revision` when the item texts were stored.
    item_revision: u64,
    tooltip_source: Option<(bool, usize)>,
    tooltip_layout: crate::tooltip_layout::Layout,
    motion: crate::hud_motion::Motion,
    cooldown_peak: [usize; 3],
    /// Last frame: the slot was learned, usable and recovering a cooldown.
    cooling: [bool; 3],
    /// Last frame's (uses now, uses held) of a multi-use skill, for the chime
    /// when a further use refills.
    use_count: [Option<(usize, usize)>; 3],
    /// When each slot last became ready, for the edge cue and 400 ms glow.
    ready_at: [Option<Instant>; 3],
    motion_identity: Option<(MatchKey, usize)>,
    shop_pressed: bool,
    shop_click: bool,
    /// Manual shopping with an empty queue: no next purchase is shown.
    purchase_blank: bool,
    /// The strip's remaining tree nodes in `cache` (1.6.0).
    nodes: Option<Nodes>,
    /// What the drawn elements show (`hud_draw.rs`), as their nodes held it.
    view: crate::hud_draw::HudView,
    /// The strip root's rectangle this frame (read once per frame).
    root_rect: Option<(f32, f32, f32, f32)>,
    /// Each image node pair's last icon, with the nodes' write counts.
    icons_shown: Vec<(NodeId, Option<Icon>, [u32; 2], u64)>,
    /// This frame's commands (`hud_draw.rs`), sent in `post_render`.
    pub(crate) list: crate::draw_list::DrawList,
    /// `draw_list::check_origin`'s state.
    origin_checked: u8,
}
impl HudUi {
    /// The visual cues keep the zero-usable-to-ready trigger. The return
    /// value asks for the chime (played only when "Skill ready sound" is on):
    /// every skill that comes back (the author, 2026-10-22: no 4 s minimum),
    /// and every further use of a multi-use skill that refills while it is
    /// already usable. `uses` is (uses now, uses it holds) for a multi-use
    /// skill.
    #[allow(clippy::too_many_arguments)]
    fn track_readiness(
        &mut self,
        slot: usize,
        live: bool,
        ready: bool,
        remaining: usize,
        full: Option<usize>,
        uses: Option<(usize, usize)>,
        now: Instant,
    ) -> bool {
        let recovered = live && ready && self.cooling[slot];
        // A further use came back: the count rose from one or more, with the
        // same number of uses held (not a level-up that adds one).
        let use_back = live
            && matches!((self.use_count[slot], uses),
                (Some((was, held_was)), Some((now_uses, held))) if held == held_was && was > 0 && now_uses > was);
        self.use_count[slot] = uses.filter(|_| live);
        let chime = recovered || use_back;
        if recovered {
            self.ready_at[slot] = Some(now);
        }
        if ready {
            self.cooldown_peak[slot] = 0;
        } else {
            self.cooldown_peak[slot] = self.cooldown_peak[slot].max(remaining);
        }
        self.cooling[slot] = live && !ready && self.full_length(slot, full) >= READY_CUE_MIN_TICKS;
        chime
    }
    /// The cooldown bar's full length: one use's refill time from the game's
    /// cooldown read (1.5.0-dev32), or the highest remaining time seen since
    /// the skill was last ready when no fresh read is at hand.
    fn full_length(&self, slot: usize, full: Option<usize>) -> usize {
        full.unwrap_or(self.cooldown_peak[slot])
    }
    /// The denied and ready rings, the charge marks and the uses count.
    fn skill_feedback(
        &mut self,
        i: usize,
        denied_at: Option<Instant>,
        ready_flash: f32,
        charges: ChargeDisplay,
    ) {
        let denied = denied_at.map_or(0., |at| {
            denied_strength(crate::clock::since(at).as_secs_f32())
        });
        let shown = denied > 0. || ready_flash > 0.;
        let k = &mut self.view.skills[i];
        k.glow.0 = shown;
        if shown {
            for band in 0..DENIED_BANDS {
                k.glow.1[band] = slot_glow_color(band, denied, ready_flash);
            }
        }
        for (mark, m) in k.charges.iter_mut().enumerate() {
            let (visible, filled, x) = match charges {
                ChargeDisplay::Marks { filled, total } => {
                    (mark < total, mark < filled, charge_mark_x(total, mark))
                }
                _ => (false, false, 0),
            };
            m.shown = visible;
            m.x = x as f32;
            if visible {
                m.fill = if filled { 0xfdee00ff } else { 0x141311ff };
            }
        }
        match &charges {
            ChargeDisplay::Count(text) => k.uses.set(text),
            _ => k.uses.set(""),
        }
    }
    // motion: A2
    /// The recall bar (from client.rs after the strip), then the strip's
    /// draw list for the frame.
    pub(crate) fn write_recall(&mut self, ctx: &mut StableClient<'_>, a2: pass::A2) {
        self.view.recall = a2.view();
        // The caps name the keys bound now (a rebind shows at once).
        self.view.keys.follow(&crate::settings::current_shared());
        crate::hud_draw::build(&self.view, ctx.ui_node_rect("ingame"), &mut self.list);
    }
    /// A completed click on the strip's shop button since the last call.
    pub fn take_shop_click(&mut self) -> bool {
        std::mem::take(&mut self.shop_click)
    }
    /// Whether the cursor is over a tile, in the strip root's rectangle
    /// read this frame.
    fn tile_hover(&self, tile: Tile, cursor: Option<(f32, f32)>) -> bool {
        tile_hover_in(self.root_rect, tile, self.inventory_count, cursor)
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
        let nodes = self.nodes.as_ref()?;
        let root = ctx.ui_node_rect(PATH);
        (0..3).find(|slot| {
            ctx.ui_visible(self.cache.path(nodes.skills[*slot].root)) == Some(true)
                && tile_hover_in(root, Tile::Skill(*slot), self.inventory_count, Some(point))
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
            .is_some_and(|at| !missing || crate::clock::since(at) < Duration::from_secs(2))
        {
            return;
        }
        self.item_read = Some(crate::clock::now());
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
    /// An image tile's icon: the sheet or PNG node shows it, the other is
    /// hidden. All image children are built with the HUD once; no
    /// remove/spawn here. An icon already shown is not checked again until
    /// one of its nodes is written by something else.
    fn icon(
        &mut self,
        ctx: &mut StableClient<'_>,
        nodes: (NodeId, Option<NodeId>),
        icon: Option<Icon>,
        log: &Logger,
    ) {
        let (icon_node, png_node) = nodes;
        let versions = [
            self.cache.version(icon_node),
            png_node.map_or(0, |n| self.cache.version(n)),
        ];
        let env = crate::ui_writes::environment();
        if self
            .icons_shown
            .iter()
            .any(|(n, i, v, e)| *n == icon_node && *i == icon && *v == versions && *e == env)
        {
            return;
        }
        let skill = png_node.is_some();
        let mut all = true;
        for (node, properties) in icon_updates(icon.as_ref(), skill) {
            let id = if node == "png" {
                png_node.unwrap_or(icon_node)
            } else {
                icon_node
            };
            let path = self.cache.path(id).to_owned();
            if self.cache.unchanged(&path, &properties) {
                continue;
            }
            let applied = self.pid(ctx, id, &properties, log);
            all &= applied;
            log.write(&format!(
                "PLAYER HUD stable image node={} applied={applied} exists={} rect={:?} source={properties}",
                path.strip_prefix(PATH).and_then(|n| n.strip_prefix('.')).unwrap_or(&path),
                ctx.ui_exists(&path),
                ctx.ui_node_rect(&path)
            ));
        }
        self.icons_shown.retain(|(n, ..)| *n != icon_node);
        if all {
            let versions = [
                self.cache.version(icon_node),
                png_node.map_or(0, |n| self.cache.version(n)),
            ];
            self.icons_shown.push((icon_node, icon, versions, env));
        }
    }
    fn inventory_layout(
        &mut self,
        ctx: &mut StableClient<'_>,
        nodes: &mut Nodes,
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
            self.view.grow_items(i + 1);
        }
        nodes.grow_items(&mut self.cache, self.allocated_slots.max(count));
        self.view.grow_items(self.allocated_slots.max(count));
        for i in 0..self.allocated_slots {
            let visible = i < count && self.playing;
            let (x, y) = if visible {
                crate::inventory::hud_slot(count, i)
            } else {
                (0, 0)
            };
            let tile = &mut self.view.items[i];
            tile.shown = visible;
            if visible {
                (tile.x, tile.y) = (x as f32, y as f32);
            }
            self.pk(
                ctx,
                nodes.items[i].root,
                S1,
                (visible, x, y),
                |(visible, x, y)| {
                    if visible {
                        format!("visible: true; x: {x}px; y: {y}px;")
                    } else {
                        "visible: false;".into()
                    }
                },
                log,
            );
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
        let id = self.cache.node(&format!("{PATH}.{node}"));
        self.pid(ctx, id, &source, log)
    }
    fn text(&mut self, ctx: &mut StableClient<'_>, node: &str, text: String, log: &Logger) {
        let id = self.cache.node(&format!("{PATH}.{node}"));
        self.tid(ctx, id, &text, log);
    }
    /// The first refused write is logged once per HUD build.
    fn refused(&mut self, ok: bool, id: NodeId, what: &str, log: &Logger) -> bool {
        if !ok && !self.failed {
            let path = self.cache.path(id);
            let node = path
                .strip_prefix(PATH)
                .and_then(|n| n.strip_prefix('.'))
                .unwrap_or(path);
            log.write(&format!("PLAYER HUD {what} failed node={node}"));
            self.failed = true;
        }
        ok
    }
    fn pid(&mut self, ctx: &mut StableClient<'_>, id: NodeId, source: &str, log: &Logger) -> bool {
        let ok = self.cache.props(ctx, id, source);
        self.refused(ok, id, "property", log)
    }
    fn tid(&mut self, ctx: &mut StableClient<'_>, id: NodeId, text: &str, log: &Logger) {
        let ok = self.cache.text(ctx, id, text);
        self.refused(ok, id, "text", log);
    }
    /// A typed write (see `ui_writes.rs`): `render` runs only on a change.
    fn pk<K: Into<Key> + Copy>(
        &mut self,
        ctx: &mut StableClient<'_>,
        id: NodeId,
        site: u8,
        values: K,
        render: impl FnOnce(K) -> String,
        log: &Logger,
    ) -> bool {
        let ok = self.cache.props_keyed(ctx, id, site, values, render);
        self.refused(ok, id, "property", log)
    }
    fn vis(&mut self, ctx: &mut StableClient<'_>, id: NodeId, on: bool, log: &Logger) {
        self.pk(ctx, id, VIS, (on,), |(on,)| format!("visible: {on};"), log);
    }
    fn purchase(&mut self, ctx: &mut StableClient<'_>, snapshot: Option<&Snapshot>, log: &Logger) {
        // Manual shopping: the next purchase follows the player's shop queue.
        if let (Some(s), Some(old)) = (snapshot, self.purchase_inputs.as_ref()) {
            if old.key == s.key
                && old.player == s.player
                && old.gold == s.gold
                && old.items == s.items
                && old.build == s.build
                && old.metadata_count == self.live_items.len()
                && crate::shop::SHOP.manual_queue_is(old.queue.as_deref())
            {
                return;
            }
        }
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
        self.view.next_unavailable = unavailable;
        let next_icon = self.cache.node(&format!("{PATH}.next.icon"));
        self.icon(ctx, (next_icon, None), icon, log);
        let ready = text == "0" || text == "✓";
        self.purchase_ready = ready;
        self.view.next.gaps = !ready;
        self.view.next.border = if ready { 0xfdee00ff } else { 0x989694ff };
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
        self.view.purchase.color = if ready { 0xfdee00ff } else { 0xcbc9c7ff };
        self.view.purchase.set(&text);
        self.view.next.progress = hundredths(progress);
        self.purchase_tooltip = tooltip;
    }
    fn tooltip(
        &mut self,
        ctx: &mut StableClient<'_>,
        s: &Snapshot,
        cursor: Option<(f32, f32)>,
        tooltip_node: NodeId,
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
                    let tile = if *skill {
                        Tile::Skill(*i)
                    } else if *i == crate::inventory::PURCHASE {
                        Tile::Next
                    } else {
                        Tile::Item(*i)
                    };
                    self.tile_hover(tile, Some(cursor))
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
            self.vis(ctx, tooltip_node, false, log);
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
            self.hover = Some((key.clone(), crate::clock::now()));
            self.tooltip_layout.reset_scroll();
            notches = 0;
        }
        self.tooltip_source = Some((skill, i));
        if key == "purchase" {
            self.tooltip_text
                .insert(key.clone(), self.purchase_tooltip.clone());
        }
        // A game item text arrived since the item texts were stored (they
        // showed "…" while waiting): build them again.
        let revision = crate::native_tooltips::item_revision();
        if self.item_revision != revision {
            self.item_revision = revision;
            self.tooltip_text.retain(|key, _| !key.starts_with("item:"));
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
        // The title's slot letter goes: the key corner names the binding.
        let title = if skill {
            raw_title
                .strip_prefix(&format!("{}   ", crate::tooltips::SKILL_LETTERS[i]))
                .unwrap_or(raw_title)
        } else {
            raw_title
        };
        let fade = self.hover.as_ref().map_or(1., |(_, at)| {
            crate::hud_motion::easing(
                (crate::clock::since(at).as_secs_f32() / 0.117).min(1.),
                true,
            )
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
        let art = (
            self.cache.node(&format!("{PATH}.tooltip.art.icon")),
            Some(self.cache.node(&format!("{PATH}.tooltip.art.png"))),
        );
        self.icon(ctx, art, icon, log);
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
            // The skill's binding, short (`key_cap`); empty while unbound.
            if skill {
                self.view.keys.skills[i].text().into()
            } else {
                String::new()
            },
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
    /// The strip's opaque surfaces on screen (clicks there are not
    /// battlefield orders), added to `out`: combat (2 px taller), strip,
    /// tooltip and feedback while shown, then item tiles above the strip.
    pub fn bounds(&self, ctx: &StableClient<'_>, out: &mut Vec<Rect>) {
        if ctx.ui_visible(PATH) != Some(true) {
            return;
        }
        let start = out.len();
        // The transparent root spans the viewport; only opaque surfaces block input.
        if let Some(nodes) = self.nodes.as_ref() {
            let ingame = ctx.ui_node_rect("ingame");
            let drawn = |r| {
                let (x, y, w, h) = crate::hud_draw::screen(ingame, r);
                Rect { x, y, w, h }
            };
            if self.view.combat {
                let (x, y, w, h) = crate::hud_draw::COMBAT;
                out.push(drawn((x, y - 2., w, h + 4.)));
            }
            out.push(drawn(crate::hud_draw::STRIP));
            let tooltip = self.cache.path(nodes.tooltip);
            if ctx.ui_visible(tooltip) == Some(true) {
                if let Some((x, y, w, h)) = ctx.ui_node_rect(tooltip) {
                    let r = Rect { x, y, w, h };
                    if r.valid() {
                        out.push(r);
                    }
                }
            }
            if self.view.feedback.shown {
                out.push(drawn(crate::hud_draw::FEEDBACK));
            }
            for i in 0..self.inventory_count.unwrap_or(0) {
                if crate::inventory::hud_slot(self.inventory_count.unwrap_or(0), i).1 < 90 {
                    let rect = match nodes.items.get(i) {
                        Some(item) => ctx.ui_node_rect(self.cache.path(item.root)),
                        None => ctx.ui_node_rect(&format!("{PATH}.item{i}")),
                    };
                    if let Some((x, y, w, h)) = rect {
                        out.push(Rect { x, y, w, h });
                    }
                }
            }
        }
        if out.len() == start {
            if let Some((x, y, _, h)) = ctx.ui_node_rect("ingame") {
                if self.playing {
                    out.push(Rect {
                        x: x + SKILL_X as f32,
                        y: y + h - HEIGHT as f32 + SKILL_Y as f32,
                        w: (SKILL_STEP * 2 + SKILL_SIZE) as f32,
                        h: SKILL_SIZE as f32,
                    });
                }
                out.push(Rect {
                    x,
                    y: y + h - 56.,
                    w: crate::minimap::STRIP_END as f32,
                    h: 56.,
                });
            }
        }
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
        // The node table lives outside `self` while the frame writes.
        let mut nodes = self
            .nodes
            .take()
            .unwrap_or_else(|| Nodes::new(&mut self.cache));
        let frame = Frame {
            active,
            playing,
            snapshot,
            artwork,
            skills,
            status,
            cursor,
            pressed,
        };
        self.apply_with(ctx, &mut nodes, frame, hud, log);
        self.nodes = Some(nodes);
    }
    fn apply_with(
        &mut self,
        ctx: &mut StableClient<'_>,
        n: &mut Nodes,
        f: Frame<'_>,
        hud: &PlayerHud,
        log: &Logger,
    ) {
        let Frame {
            active,
            playing,
            snapshot,
            artwork,
            skills,
            status,
            cursor,
            pressed,
        } = f;
        if let Ok(mut area) = TOOLTIP_SCROLL_AREA.lock() {
            *area = None;
        }
        self.root_rect = None;
        // Drawn only once the root shows this frame (it may be missing:
        // removed for a rebuild, its spawn not due yet).
        self.view.shown = false;
        if !active || crate::ui_root::hidden() {
            if !active {
                self.hover = None;
                self.tooltip_source = None;
                TOOLTIP_SCROLL.store(0, Ordering::Relaxed);
                self.motion.reset();
                self.pass.reset(); // motion: pass
                self.motion_identity = None;
                self.spawn_attempted = None;
            }
            // Write only on a change (this runs every frame).
            crate::ui_root::hide(ctx, PATH);
            if !active {
                return;
            }
        }
        if !self.ensure_spawned(ctx, log) {
            return;
        }
        crate::ui_root::show(ctx, PATH);
        self.view.shown = !crate::ui_root::hidden();
        self.root_rect = ctx.ui_node_rect(PATH);
        let drawn = crate::hud_draw::origin(ctx.ui_node_rect("ingame"));
        crate::draw_list::check_origin(
            "HUD strip",
            drawn,
            self.root_rect,
            &mut self.origin_checked,
            log,
        );
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
        self.view.veils =
            pass::b6_death_dim(&mut self.pass, shown.map(|s| s.alive), playing).view();
        // motion: D1
        self.view.hairline =
            pass::d1_stats_hairline(&mut self.pass, shown.map(|s| s.kda.0), playing).view();
        let blank = self.purchase_blank;
        for (k, id) in n.shown_while_playing.iter().enumerate() {
            let node = crate::hud_nodes::SHOWN_WHILE_PLAYING[k];
            self.vis(ctx, *id, playing && !(blank && node == "next"), log);
        }
        self.view.shown_while_playing(playing, blank);
        // Shop button: hover tint over 100 ms; a press-and-release opens the shop.
        let shop_hover = playing
            && cursor.is_some_and(|p| {
                self.root_rect.is_some_and(|(rx, ry, rw, rh)| {
                    let (x, y, w, h) = crate::hud_draw::SHOP_HIT;
                    let (sx, sy) = (rw / WIDTH as f32, rh / HEIGHT as f32);
                    Rect {
                        x: rx + x * sx,
                        y: ry + y * sy,
                        w: w * sx,
                        h: h * sy,
                    }
                    .contains(p)
                })
            });
        if shop_hover && self.shop_pressed && !pressed {
            self.shop_click = true;
        }
        self.shop_pressed = pressed && (shop_hover || self.shop_pressed);
        let tint = self.motion.tonal("shop_hover", f32::from(shop_hover), 0.1);
        let back = crate::hud_motion::color_rgba(
            crate::ui_theme::tone(0x3a38_3700),
            crate::ui_theme::tone(0x3a38_37ff),
            tint,
        );
        self.view.shop_hit.1 = back;
        if !playing {
            self.hover = None;
            self.tooltip_source = None;
            TOOLTIP_SCROLL.store(0, Ordering::Relaxed);
            n.grow_items(&mut self.cache, self.allocated_slots);
            self.view.grow_items(self.allocated_slots);
            for i in 0..self.allocated_slots {
                self.vis(ctx, n.items[i].root, false, log);
                self.view.items[i].shown = false;
            }
            self.health_ticks(None);
            self.purchase_inputs = None;
            for id in n.hidden_while_watching {
                self.vis(ctx, id, false, log);
            }
            self.view.hidden_while_watching();
            return;
        }
        self.view.feedback.shown = !status.is_empty();
        self.view.feedback.set(status);
        for slash in &mut self.view.slash {
            slash.set("/");
        }
        let Some(s) = snapshot else {
            self.no_sample(ctx, n, artwork, log);
            return;
        };
        self.refresh_assets(ctx, s, hud, log);
        let count = crate::inventory::slots(
            s.build.as_ref().map(Vec::len),
            s.items.len(),
            self.inventory_count,
        );
        self.inventory_layout(ctx, n, count, Some((s.key, s.player)), log);
        // motion: B2 (restore `s.gold`)
        let gold = pass::b2_gold_roll(&mut self.pass, s.gold);
        let v = &mut self.view;
        for (label, value) in [
            (&mut v.kills, s.kda.0),
            (&mut v.deaths, s.kda.1),
            (&mut v.assists, s.kda.2),
            (&mut v.cs, s.cs),
            (&mut v.gold, gold),
            (&mut v.level, s.level),
        ] {
            label.put(format_args!("{value}"));
        }
        // motion: B4
        self.view.level.color = pass::b4_level_lift(&mut self.pass, s.level, 0xffffffff);
        self.purchase(ctx, Some(s), log);
        self.view.coin.1 = 0xfdee00ff;
        for id in [n.purchase_coin, n.purchase_check] {
            // motion: A1 (restore one `color: #fdee00ff` for both and the coin)
            let color = pass::a1_purchase_breathe(&mut self.pass, self.purchase_ready, 0xfdee00ff);
            self.pk(ctx, id, S1, (color,), color_source, log);
        }
        let v = &mut self.view;
        v.hourglass = !s.alive;
        v.respawn.shown = !s.alive;
        match (s.alive, s.respawn) {
            (false, respawn) if respawn > 0 => {
                v.respawn.put(format_args!("{}", respawn.div_ceil(60)))
            }
            (false, _) => v.respawn.set("—"),
            _ => v.respawn.set(""),
        }
        v.hp_current.shown = s.alive;
        v.hp_max.shown = s.alive;
        match s.hp {
            Some((hp, max)) => {
                v.hp_current.put(format_args!("{hp}"));
                v.hp_max.put(format_args!(" / {max}"));
            }
            None => {
                v.hp_current.set("—");
                v.hp_max.set(" / —");
            }
        }
        let health_fill = self.motion.value("hp", hp_percent(s), 0.2);
        let health_fill = if s.alive { health_fill } else { 0. };
        self.view.health.2 = (hundredths(health_fill), 0x2e7d46ff);
        let trail = self.motion.value("hp_trail", hp_percent(s), 0.5);
        let trail = if s.alive { trail } else { 0. };
        // motion: B1 (restore the trail colour as 0xefe6cfff)
        let trail_color = pass::b1_damage_chunk(&mut self.pass, hp_percent(s), 0xefe6cfff);
        self.view.health.1 = (hundredths(trail), trail_color);
        self.health_ticks(s.hp.filter(|_| s.alive).map(|(_, max)| max));
        for (i, unlock_level) in UNLOCK_LEVELS.into_iter().enumerate() {
            self.skill_slot(
                ctx,
                &n.skills[i],
                i,
                unlock_level,
                s,
                skills,
                cursor,
                pressed,
                log,
            );
        }
        for i in 0..count {
            let icon = s.items.get(i).and_then(|key| self.item_icon(key));
            let unknown = s.items.get(i).is_some() && icon.is_none();
            self.icon(ctx, (n.items[i].icon, None), icon, log);
            self.view.items[i].unknown = unknown;
        }
        for i in (0..count).chain(std::iter::once(crate::inventory::PURCHASE)) {
            self.item_tile(ctx, n, i, s, cursor, pressed, log);
        }
        self.tooltip(ctx, s, cursor, n.tooltip, log);
    }
    /// Spawn the strip if it is missing; false while it does not exist.
    /// One try a second.
    pub fn ensure_spawned(&mut self, ctx: &mut StableClient<'_>, log: &Logger) -> bool {
        if ctx.ui_exists(PATH) {
            return true;
        }
        if self
            .spawn_attempted
            .is_some_and(|t| crate::clock::since(t) < Duration::from_secs(1))
        {
            return false;
        }
        self.spawn_attempted = Some(crate::clock::now());
        self.cache.clear();
        self.icons_shown.clear();
        self.allocated_slots = 6;
        self.view.reset(6);
        self.purchase_inputs = None;
        self.failed = false;
        let spawned = ctx.ui_spawn_source("ingame", &crate::ui_theme::themed(&template()));
        log.write(&format!(
            "PLAYER HUD spawn={spawned} exists={}",
            ctx.ui_exists(PATH)
        ));
        spawned && ctx.ui_exists(PATH)
    }
    /// Control has started, before the first sample of your athlete.
    fn no_sample(
        &mut self,
        ctx: &mut StableClient<'_>,
        n: &mut Nodes,
        artwork: Option<&Artwork>,
        log: &Logger,
    ) {
        if artwork.is_some() {
            self.load_icons(log);
        }
        let v = &mut self.view;
        for label in [
            &mut v.kills,
            &mut v.deaths,
            &mut v.assists,
            &mut v.cs,
            &mut v.gold,
            &mut v.level,
        ] {
            label.set("—");
        }
        v.respawn.set("");
        v.hp_current.set("—");
        v.hp_max.set(" / —");
        v.hourglass = false;
        v.hp_current.shown = true;
        v.hp_max.shown = true;
        self.purchase(ctx, None, log);
        self.view.health.1 .0 = 0.;
        self.view.health.2 .0 = 0.;
        self.health_ticks(None);
        for i in 0..3 {
            let k = &n.skills[i];
            self.skill_feedback(i, None, 0., ChargeDisplay::Hidden);
            let icon = artwork.and_then(|a| self.skill_icon(&a.champion, i));
            self.icon(ctx, (k.icon, Some(k.png)), icon, log);
            let v = &mut self.view.skills[i];
            // The shade keeps its height.
            v.shade.0 = true;
            v.shade.2 = 0x00000088;
            v.cooldown.set("--");
            v.uses.set("");
            v.lock = false;
            v.border = 0x555555ff;
        }
        let retained = artwork
            .filter(|a| self.item_identity == Some(a.identity))
            .and(self.inventory_count);
        let count = crate::inventory::slots(None, artwork.map_or(0, |a| a.items.len()), retained);
        self.inventory_layout(ctx, n, count, artwork.map(|a| a.identity), log);
        for i in 0..count {
            let icon = artwork
                .filter(|a| self.item_identity == Some(a.identity))
                .and_then(|a| a.items.get(i))
                .and_then(|key| self.item_icon(key));
            self.icon(ctx, (n.items[i].icon, None), icon, log);
            self.view.items[i].unknown = false;
        }
        self.pid(ctx, n.tooltip, "visible: false;", log);
    }
    /// One skill slot while you play: readiness (and its chime), the
    /// cooldown drain, hover, press and the slot's border.
    #[allow(clippy::too_many_arguments)]
    fn skill_slot(
        &mut self,
        ctx: &mut StableClient<'_>,
        k: &crate::hud_nodes::Skill,
        i: usize,
        unlock_level: usize,
        s: &Snapshot,
        skills: HudSkills,
        cursor: Option<(f32, f32)>,
        pressed: bool,
        log: &Logger,
    ) {
        let icon = self.skill_icon(&s.champion, i);
        self.icon(ctx, (k.icon, Some(k.png)), icon, log);
        let learned = s.level >= unlock_level;
        let unavailable = skill_unavailable(s, skills, i);
        let ready = skills.ready[i].unwrap_or(s.cooldowns[i] == 0);
        let remaining = skills.wait[i].unwrap_or(s.cooldowns[i]);
        // Ready cues and sounds wait for the respawn.
        let live = learned && !unavailable && s.alive;
        let uses = skills.uses[i]
            .zip(skills.uses_max[i])
            .filter(|(_, held)| *held > 1);
        if self.track_readiness(
            i,
            live,
            ready,
            remaining,
            skills.period[i],
            uses,
            crate::clock::now(),
        ) {
            crate::ui_sound::ready();
        }
        // motion: B4
        let (b4_shade, b4_cue) = pass::b4_learned_drain(&mut self.pass, i, learned);
        if b4_cue && live && ready {
            self.ready_at[i] = Some(crate::clock::now());
        }
        let ready_age = self.ready_at[i]
            .filter(|_| live && ready)
            .map(|at| crate::clock::since(at).as_secs_f32());
        let cue = ready_age.and_then(ready_cue);
        let ready_flash = ready_age.map_or(0., ready_flash_strength);
        if ready_flash <= 0. {
            self.ready_at[i] = None;
        }
        // A multi-use skill with some uses left still shows the next use
        // refilling, lighter, with its countdown (the author, 2026-10-22).
        let refill = skills.refill[i].filter(|_| ready && !unavailable && learned);
        let shown = refill.unwrap_or(remaining);
        let drain = if unavailable {
            78.
        } else {
            78. * (shown as f32 / self.full_length(i, skills.period[i]).max(1) as f32).min(1.)
        };
        let edge_y = cooldown_edge_y(drain, cue.is_some());
        let edge = match cue {
            Some((opacity, _)) => 0xfdee0000 | u32::from((opacity * 255.).round() as u8),
            None => crate::hud_motion::color_rgba(
                0xffffffff,
                0xfdee00ff,
                edge_warmth(shown, self.full_length(i, skills.period[i])),
            ),
        };
        let edge_shown = (!unavailable && !ready) || cue.is_some() || refill.is_some();
        // The cue sits just inside the slot's border, which is 2 px and
        // yellow on a ready R.
        let v = &mut self.view.skills[i];
        v.drain_edge = crate::hud_draw::Bar {
            shown: edge_shown,
            y: hundredths(edge_y),
            color: edge,
        };
        // motion: A3
        let comet = pass::a3_drain_comet(
            &mut self.pass,
            i,
            !unavailable && !ready && cue.is_none(),
            edge_y,
            edge,
        )
        .view();
        let v = &mut self.view.skills[i];
        v.comet.shown = comet.is_some();
        if let Some((y, color)) = comet {
            (v.comet.y, v.comet.color) = (y, color);
        }
        // motion: C1 (node and hover moved up from the border code below)
        let hover = self.tile_hover(Tile::Skill(i), cursor);
        // motion: C1
        let c1 = pass::c1_slot_hover(&mut self.pass, i, hover);
        self.view.skills[i].keycap = crate::hud_motion::color_rgba(0xeeececff, 0xffffffff, c1);
        let brightness = match cue {
            Some((_, lift)) => COOLING_ICON + (1. - COOLING_ICON) * lift,
            // motion: B4 (restore `None if live && !ready`)
            None if live && (!ready || b4_shade.is_some()) => COOLING_ICON,
            None => 1.,
        };
        // motion: B4, C1 (restore the 1.3.2 shade write)
        let shade = match (learned, refill) {
            (false, _) => 0x141311dd,
            // Still usable: a lighter shade over the bright icon.
            (true, Some(_)) => REFILL_SHADE,
            (true, None) => 0x0a0908cc,
        };
        let shade_on = unavailable || !ready || b4_shade.is_some() || refill.is_some();
        let shade_color = pass::fade(shade, 1. - 0.3 * c1);
        self.view.skills[i].shade = (shade_on, hundredths(b4_shade.unwrap_or(drain)), shade_color);
        // 0: nothing, 1: "--", 2: a countdown.
        let (cooldown_kind, ticks) = if !learned {
            (0u8, 0)
        } else if unavailable {
            (1, 0)
        } else if !ready {
            (2, skills.wait[i].unwrap_or(s.cooldowns[i]))
        } else if let Some(ticks) = refill {
            (2, ticks)
        } else {
            (0, 0)
        };
        let label = &mut self.view.skills[i].cooldown;
        match cooldown_kind {
            1 => label.set("--"),
            2 => label.set(&cooldown(ticks)),
            _ => label.set(""),
        }
        self.skill_feedback(
            i,
            skills.denied_at[i].filter(|_| s.alive),
            ready_flash,
            charge_display(skills.uses[i].filter(|_| !unavailable), skills.uses_max[i]),
        );
        self.view.skills[i].lock = !learned;
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
        let h = self.motion.tonal(&k.hover_key, f32::from(hover), 0.1);
        let press = self
            .motion
            .tonal(&k.press_key, f32::from(hover && pressed), 0.083);
        let border = crate::ui_theme::color(color).unwrap_or(crate::ui_theme::tone(0x4b4a49ff));
        let v = &mut self.view.skills[i];
        v.border = crate::hud_motion::color_rgba(border, 0xffffffff, h);
        v.inset = if color == "fdee00ff" { 2. } else { 1. };
        // The press tint (85 %) on top of the cooldown brightness.
        let level = brightness * (1. - 0.15 * press);
        for art in [k.icon, k.png] {
            self.pk(
                ctx,
                art,
                S1,
                (level,),
                |(level,)| format!("width: 74px; height: 74px; color: #{};", grey(level)),
                log,
            );
        }
    }
    /// One inventory tile (or the next purchase, `PURCHASE`): hover, press,
    /// a new item's edge and fade-in.
    #[allow(clippy::too_many_arguments)]
    fn item_tile(
        &mut self,
        ctx: &mut StableClient<'_>,
        n: &Nodes,
        i: usize,
        s: &Snapshot,
        cursor: Option<(f32, f32)>,
        pressed: bool,
        log: &Logger,
    ) {
        let purchase = i == crate::inventory::PURCHASE;
        let (tile, t) = if purchase {
            (Tile::Next, &n.next)
        } else {
            (Tile::Item(i), &n.items[i])
        };
        let hover = self.tile_hover(tile, cursor);
        let h = self.motion.tonal(&t.hover_key, f32::from(hover), 0.1);
        let press = self
            .motion
            .tonal(&t.press_key, f32::from(hover && pressed), 0.083);
        // Inventory rest positions are owned by inventory_layout.
        let back = crate::hud_motion::color_rgba(
            // Filled: background + 5; empty: + 13.
            crate::ui_theme::shade_rgba(if s.items.get(i).is_some() { 5 } else { 13 }, 0xff),
            crate::ui_theme::tone(0x3a3837ff),
            (h - press * 0.25).clamp(0., 1.),
        );
        if purchase {
            self.view.next.background = back;
        } else {
            self.view.items[i].background = back;
        }
        if !purchase {
            // motion: B5 (restore the 1.3.2 border write)
            let item = s.items.get(i).map(String::as_str);
            let (b5_alpha, b5_edge) = pass::b5_item_bought(&mut self.pass, i, item);
            let border =
                crate::hud_motion::color_rgba(crate::ui_theme::tone(0x4b4a49ff), 0xffffffff, h);
            self.view.items[i].border = crate::hud_motion::color_rgba(border, 0xfdee00ff, b5_edge);
            // motion: C2, B5
            let brightness = pass::c2_item_press(&self.pass, press);
            self.pk(
                ctx,
                t.icon,
                S1,
                (brightness, b5_alpha),
                |(b, a)| pass::icon_tint(b, a),
                log,
            );
        } else {
            self.view.next.border = if self.purchase_ready {
                0xfdee00ff
            } else {
                crate::hud_motion::color_rgba(0x989694ff, 0xffffffff, h)
            };
            let opacity = if self.purchase_ready {
                255.
            } else {
                140. + 115. * h
            };
            // motion: B5 (restore `opacity` as before)
            let b5 = pass::b5_next_item(
                &mut self.pass,
                self.purchase_icon.as_ref().map(|i| i.source.as_str()),
            );
            let opacity = (opacity * b5).round() as u8;
            self.pk(
                ctx,
                t.icon,
                S1,
                (opacity,),
                |(opacity,)| format!("color: #ffffff{opacity:02x};"),
                log,
            );
        }
    }
    /// The health bar's ticks: one per 100 health (more per tick past 3,200).
    fn health_ticks(&mut self, max: Option<usize>) {
        for (i, tick) in self.view.ticks.iter_mut().enumerate() {
            *tick = max.filter(|max| *max > 0).and_then(|max| {
                let step = max.div_ceil(3200).saturating_mul(100);
                let value = (i + 1).saturating_mul(step);
                (value < max).then(|| {
                    ((780. + 360. * value as f64 / max as f64) * 100.).round() as f32 / 100.
                })
            });
        }
    }
}

/// One frame's inputs to [`HudUi::apply`].
struct Frame<'a> {
    active: bool,
    playing: bool,
    snapshot: Option<&'a Snapshot>,
    artwork: Option<&'a Artwork>,
    skills: HudSkills,
    status: &'a str,
    cursor: Option<(f32, f32)>,
    pressed: bool,
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
        let (y, _, h) = crate::hud_draw::CHARGE_MARK;
        assert!(y > 25. + 34.); // Cooldown label's bottom edge.
        assert!(y + h <= SKILL_SIZE as f32 - 2.);
        assert_eq!(CHARGE_MARK_W as f32, crate::hud_draw::CHARGE_MARK.1);
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
        assert!(!hud.track_readiness(0, true, true, 0, None, None, now));
        assert!(hud.ready_at[0].is_none()); // Already ready at first sight.
        assert!(!hud.track_readiness(0, true, false, 120, None, None, now)); // Zero usable charges.
        hud.track_readiness(0, true, false, 6, None, None, now);
        // Two seconds of cooldown chime too: no 4 s minimum any more.
        assert!(hud.track_readiness(0, true, true, 0, None, None, now));
        assert_eq!(hud.ready_at, [Some(now), None, None]);
        let later = now + Duration::from_millis(200);
        hud.track_readiness(0, true, true, 0, None, None, later); // Extra charge, still usable.
        assert_eq!(hud.ready_at[0], Some(now));
        hud.ready_at[0] = None; // Render finished the cue.
        hud.track_readiness(0, true, true, 0, None, None, later);
        assert!(hud.ready_at[0].is_none()); // No repeated ready glow.
        hud.track_readiness(1, true, false, 300, None, None, now);
        assert!(hud.track_readiness(1, true, true, 0, None, None, later));
        hud.track_readiness(2, true, false, 120, None, None, now);
        hud.track_readiness(2, false, true, 0, None, None, later); // Dead/unavailable.
        assert!(hud.ready_at[2].is_none());
    }
    #[test]
    fn every_use_that_comes_back_asks_for_the_chime() {
        // Ghost W, 3 uses: spent to one, then each refill while usable.
        let mut hud = HudUi::default();
        let now = Instant::now();
        assert!(!hud.track_readiness(0, true, true, 0, Some(300), Some((3, 3)), now));
        assert!(!hud.track_readiness(0, true, true, 0, Some(300), Some((1, 3)), now));
        assert!(hud.track_readiness(0, true, true, 0, Some(300), Some((2, 3)), now));
        assert!(hud.track_readiness(0, true, true, 0, Some(300), Some((3, 3)), now));
        assert!(!hud.track_readiness(0, true, true, 0, Some(300), Some((3, 3)), now));
        // From zero: the ordinary ready chime, once (not twice).
        hud.track_readiness(0, true, false, 200, Some(300), Some((0, 3)), now);
        assert!(hud.track_readiness(0, true, true, 0, Some(300), Some((1, 3)), now));
        // A level-up that adds a use is not a refill; nor is anything while dead.
        assert!(!hud.track_readiness(0, true, true, 0, Some(300), Some((2, 4)), now));
        assert!(!hud.track_readiness(0, false, true, 0, Some(300), Some((3, 4)), now));
        assert!(!hud.track_readiness(0, true, true, 0, Some(300), Some((4, 4)), now));
    }
    #[test]
    fn the_bar_and_chime_take_the_games_full_cycle_from_the_first_frame() {
        // Seen mid-cooldown for the first time (control taken during it):
        // with the game's cycle the bar's length is right at once.
        let mut hud = HudUi::default();
        assert_eq!(hud.full_length(0, Some(300)), 300);
        assert_eq!(hud.full_length(0, None), 0);
        let now = Instant::now();
        hud.track_readiness(0, true, false, 30, Some(300), None, now);
        assert!(hud.cooling[0]);
        // A 300-tick skill chimes when ready even though only 30 were seen.
        assert!(hud.track_readiness(0, true, true, 0, Some(300), None, now));
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
        // Drawn since 1.6.0-perf-dev3, not tree nodes.
        for drawn in [
            "#slot_glow",
            "#charge5",
            "#d1_line",
            "#strip",
            "#hp_tick",
            "#b6_veil",
        ] {
            assert!(!source.contains(drawn), "{drawn}");
        }
        // Kill feedback stays: the D1 hairline is drawn while it sweeps.
        let mut view = crate::hud_draw::HudView {
            shown: true,
            hairline: Some(300.),
            ..Default::default()
        };
        let mut list = crate::draw_list::DrawList::default();
        crate::hud_draw::build(&view, None, &mut list);
        let line = crate::draw_list::Cmd::Rect {
            rect: (0., 934. + 138., 300., 2.),
            z: 1004,
            rounding: 0.,
            color: 0xfdee00ff,
        };
        assert!(list.commands().contains(&line));
        view.hairline = None;
        crate::hud_draw::build(&view, None, &mut list);
        assert!(!list.commands().contains(&line));
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
        let updates = |parent: &str, icon: Option<&Icon>, skill: bool| -> Vec<(String, String)> {
            icon_updates(icon, skill)
                .into_iter()
                .map(|(node, source)| (format!("{parent}.{node}"), source))
                .collect()
        };
        let sheet = Icon {
            source: "asset/base/skills".into(),
            tag: Some("lancer_0".into()),
        };
        let png = Icon {
            source: "asset/pack/icons/q".into(),
            tag: None,
        };
        let normal = updates("skill0", Some(&sheet), true);
        assert!(normal[0].1.contains("lancer_0"));
        assert_eq!(normal[1], ("skill0.png".into(), "visible: false;".into()));
        let custom = updates("skill0", Some(&png), true);
        assert_eq!(custom[0], ("skill0.icon".into(), "visible: false;".into()));
        assert_eq!(custom[1].0, "skill0.png");
        assert!(!custom[1].1.contains("rect_tag"));
        assert!(custom[1].1.contains("asset/pack/icons/q"));
        assert!(updates("skill0", None, true)
            .iter()
            .all(|(_, p)| p == "visible: false;"));
        let mut slots = HashMap::new();
        for i in 0..6 {
            let item = Icon {
                source: "asset/base/items".into(),
                tag: Some(format!("item{i}")),
            };
            slots.extend(updates(&format!("item{i}"), Some(&item), false));
        }
        let before = slots.clone();
        slots.extend(updates("item5", Some(&sheet), false));
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
