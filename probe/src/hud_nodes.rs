//! The player HUD strip's nodes in its write cache (1.6.0-perf): every path
//! `HudUi` writes each frame, found once instead of formatted per write.
//! Also the names of its hover and press tweens.
use crate::hud_motion_pass as pass;
use crate::player_hud::PATH;
use crate::ui_writes::{NodeId, PropCache};

/// Nodes shown with the strip while you play (`visible: {playing}`).
pub const SHOWN_WHILE_PLAYING: [&str; 23] = [
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
];
/// Nodes hidden while the strip shows without your athlete.
pub const HIDDEN_WHILE_WATCHING: [&str; 9] = [
    "tooltip",
    "feedback",
    "respawn",
    "hourglass",
    "hp_current",
    "hp_max",
    "purchase_coin",
    "purchase_check",
    "next_unavailable",
];
pub const DENIED_BANDS: usize = 10;
pub const CHARGE_MARKS: usize = 6;
pub const HEALTH_TICKS: usize = 32;

pub struct Skill {
    pub root: NodeId,
    pub slot_glow: NodeId,
    pub bands: [[NodeId; 4]; DENIED_BANDS],
    pub charge: [NodeId; CHARGE_MARKS],
    pub charge_fill: [NodeId; CHARGE_MARKS],
    pub uses: NodeId,
    pub shade: NodeId,
    pub cooldown: NodeId,
    pub lock: NodeId,
    pub drain_edge: NodeId,
    pub a3: NodeId,
    pub keycap: NodeId,
    pub background: NodeId,
    pub icon: NodeId,
    pub png: NodeId,
    pub hover_key: String,
    pub press_key: String,
}
pub struct Item {
    pub root: NodeId,
    pub background: NodeId,
    pub icon: NodeId,
    pub unknown: NodeId,
    pub hover_key: String,
    pub press_key: String,
}
/// A tile the cursor can hover.
#[derive(Clone, Copy)]
pub enum Tile {
    Skill(usize),
    Next,
    Item(usize),
}
pub struct Nodes {
    /// The opaque surfaces that block battlefield clicks: combat, strip,
    /// tooltip, feedback.
    pub surfaces: [NodeId; 4],
    pub shown_while_playing: [NodeId; 23],
    pub hidden_while_watching: [NodeId; 9],
    pub shop_hit: NodeId,
    pub feedback: NodeId,
    pub feedback_text: NodeId,
    pub slash: [NodeId; 2],
    pub kills: NodeId,
    pub deaths: NodeId,
    pub assists: NodeId,
    pub cs: NodeId,
    pub gold: NodeId,
    pub level: NodeId,
    pub respawn: NodeId,
    pub hourglass: NodeId,
    pub hp_current: NodeId,
    pub hp_max: NodeId,
    pub health_fill: NodeId,
    pub health_trail: NodeId,
    pub coin: NodeId,
    pub purchase_coin: NodeId,
    pub purchase_check: NodeId,
    pub tooltip: NodeId,
    pub next: Item,
    pub ticks: [NodeId; HEALTH_TICKS],
    pub skills: [Skill; 3],
    /// Inventory tiles, grown with the strip's allocated slots.
    pub items: Vec<Item>,
    pub a2: [NodeId; 3],
    pub b6: [NodeId; 3],
    pub d1: NodeId,
}
fn id(c: &mut PropCache, node: &str) -> NodeId {
    c.node(&format!("{PATH}.{node}"))
}
fn item(c: &mut PropCache, node: &str) -> Item {
    Item {
        root: id(c, node),
        background: id(c, &format!("{node}.background")),
        icon: id(c, &format!("{node}.icon")),
        unknown: id(c, &format!("{node}.unknown")),
        hover_key: format!("hover_{node}"),
        press_key: format!("press_{node}"),
    }
}
impl Nodes {
    pub fn new(c: &mut PropCache) -> Self {
        let skills = std::array::from_fn(|i| {
            let s = format!("skill{i}");
            Skill {
                root: id(c, &s),
                slot_glow: id(c, &format!("{s}.slot_glow")),
                bands: std::array::from_fn(|band| {
                    std::array::from_fn(|side| id(c, &format!("{s}.slot_glow.b{band}_{side}")))
                }),
                charge: std::array::from_fn(|m| id(c, &format!("{s}.charge{m}"))),
                charge_fill: std::array::from_fn(|m| id(c, &format!("{s}.charge{m}.fill"))),
                uses: id(c, &format!("{s}.uses")),
                shade: id(c, &format!("{s}.shade")),
                cooldown: id(c, &format!("{s}.cooldown")),
                lock: id(c, &format!("{s}.lock")),
                drain_edge: id(c, &format!("{s}.drain_edge")),
                a3: id(c, pass::A3_NODES[i]),
                keycap: id(c, &format!("{s}.keycap")),
                background: id(c, &format!("{s}.background")),
                icon: id(c, &format!("{s}.icon")),
                png: id(c, &format!("{s}.png")),
                hover_key: format!("hover_{s}"),
                press_key: format!("press_{s}"),
            }
        });
        Self {
            surfaces: ["combat", "strip", "tooltip", "feedback"].map(|n| id(c, n)),
            shown_while_playing: SHOWN_WHILE_PLAYING.map(|n| id(c, n)),
            hidden_while_watching: HIDDEN_WHILE_WATCHING.map(|n| id(c, n)),
            shop_hit: id(c, "shop_hit"),
            feedback: id(c, "feedback"),
            feedback_text: id(c, "feedback.text"),
            slash: [id(c, "slash1"), id(c, "slash2")],
            kills: id(c, "kills"),
            deaths: id(c, "deaths"),
            assists: id(c, "assists"),
            cs: id(c, "cs"),
            gold: id(c, "gold"),
            level: id(c, "level"),
            respawn: id(c, "respawn"),
            hourglass: id(c, "hourglass"),
            hp_current: id(c, "hp_current"),
            hp_max: id(c, "hp_max"),
            health_fill: id(c, "health.fill"),
            health_trail: id(c, "health.trail"),
            coin: id(c, "coin"),
            purchase_coin: id(c, "purchase_coin"),
            purchase_check: id(c, "purchase_check"),
            tooltip: id(c, "tooltip"),
            next: item(c, "next"),
            ticks: std::array::from_fn(|i| id(c, &format!("hp_tick{i}"))),
            skills,
            items: Vec::new(),
            a2: pass::A2_NODES.map(|n| id(c, n)),
            b6: pass::B6_NODES.map(|n| id(c, n)),
            d1: id(c, pass::D1_NODE),
        }
    }
    /// Inventory tile nodes for slots `0..count`.
    pub fn grow_items(&mut self, c: &mut PropCache, count: usize) {
        for i in self.items.len()..count {
            self.items.push(item(c, &format!("item{i}")));
        }
    }
}
