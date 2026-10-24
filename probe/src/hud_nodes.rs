//! The player HUD strip's remaining tree nodes in its write cache (1.6.0):
//! every path `HudUi` writes, found once instead of formatted per write.
//! Also the names of its hover and press tweens. Since 1.6.0-perf-dev3 the
//! strip's boxes, texts and glyphs are drawn (`hud_draw.rs`); what stays
//! here are the sprite-sheet images, their containers, the purchase coin
//! and check, and the tooltip.
use crate::player_hud::PATH;
use crate::ui_writes::{NodeId, PropCache};

pub use crate::hud_draw::{CHARGE_MARKS, DENIED_BANDS};

/// Tree nodes shown with the strip while you play (`visible: {playing}`).
pub const SHOWN_WHILE_PLAYING: [&str; 4] = ["next", "skill0", "skill1", "skill2"];
/// Tree nodes hidden while the strip shows without your athlete.
pub const HIDDEN_WHILE_WATCHING: [&str; 3] = ["tooltip", "purchase_coin", "purchase_check"];

pub struct Skill {
    /// The slot's container (its box is drawn).
    pub root: NodeId,
    pub icon: NodeId,
    pub png: NodeId,
    pub hover_key: String,
    pub press_key: String,
}
pub struct Item {
    /// The tile's container (its box is drawn).
    pub root: NodeId,
    pub icon: NodeId,
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
    pub shown_while_playing: [NodeId; 4],
    pub hidden_while_watching: [NodeId; 3],
    pub purchase_coin: NodeId,
    pub purchase_check: NodeId,
    pub tooltip: NodeId,
    pub next: Item,
    pub skills: [Skill; 3],
    /// Inventory tiles, grown with the strip's allocated slots.
    pub items: Vec<Item>,
}
fn id(c: &mut PropCache, node: &str) -> NodeId {
    c.node(&format!("{PATH}.{node}"))
}
fn item(c: &mut PropCache, node: &str) -> Item {
    Item {
        root: id(c, node),
        icon: id(c, &format!("{node}.icon")),
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
                icon: id(c, &format!("{s}.icon")),
                png: id(c, &format!("{s}.png")),
                hover_key: format!("hover_{s}"),
                press_key: format!("press_{s}"),
            }
        });
        Self {
            shown_while_playing: SHOWN_WHILE_PLAYING.map(|n| id(c, n)),
            hidden_while_watching: HIDDEN_WHILE_WATCHING.map(|n| id(c, n)),
            purchase_coin: id(c, "purchase_coin"),
            purchase_check: id(c, "purchase_check"),
            tooltip: id(c, "tooltip"),
            next: item(c, "next"),
            skills,
            items: Vec::new(),
        }
    }
    /// Inventory tile nodes for slots `0..count`.
    pub fn grow_items(&mut self, c: &mut PropCache, count: usize) {
        for i in self.items.len()..count {
            self.items.push(item(c, &format!("item{i}")));
        }
    }
}
