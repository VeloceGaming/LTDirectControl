//! Native shop window (P or the strip's shop button). Layout follows the
//! approved HTML preview (kept outside git) in 1920x1080 coordinates;
//! positions below are relative to the 1360x872 window at (180, 78).
//! Buying only queues for crate::shop; the simulation performs purchases.
use crate::stat_icons::{self, StatIcon};
use crate::{camera::Rect, hud_motion, platform_input::Keys, player_hud::HudUi, shop, Logger};
use mod_api_stable::{StableClient, UiEventKindV1};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

type OfferKey = (usize, Option<Vec<usize>>);
type OfferCache = (
    Option<(Arc<Vec<shop::Item>>, shop::Live, Vec<shop::Order>)>,
    HashMap<OfferKey, shop::Offer>,
);
thread_local! {
    /// `shop::offer` results for the current catalogue, inventory and queue.
    /// The window asks for the same items several times per frame (dimming,
    /// price, tree, detail) and every frame; inputs change rarely.
    static OFFERS: std::cell::RefCell<OfferCache> = std::cell::RefCell::default();
}
fn offer(view: &shop::View, item: usize) -> shop::Offer {
    offer_path(view, item, None)
}
fn offer_path(view: &shop::View, item: usize, path: Option<&[usize]>) -> shop::Offer {
    OFFERS.with(|cache| {
        let mut cache = cache.borrow_mut();
        // Holding the catalogue keeps its identity unique across matches.
        let fresh = cache.0.as_ref().is_some_and(|(cat, live, orders)| {
            Arc::ptr_eq(cat, &view.cat) && *live == view.live && *orders == view.orders
        });
        if !fresh {
            cache.0 = Some((view.cat.clone(), view.live.clone(), view.orders.clone()));
            cache.1.clear();
        }
        cache
            .1
            .entry((item, path.map(<[usize]>::to_vec)))
            .or_insert_with(|| match path {
                None => shop::offer(&view.cat, &view.live, &view.orders, item),
                Some(path) => {
                    shop::offer_with_path(&view.cat, &view.live, &view.orders, item, Some(path))
                }
            })
            .clone()
    })
}

/// Cost of this displayed branch, not the cheapest competing branch. A full
/// bag blocks purchase but must not turn its price into a single step price.
fn recipe_cost(view: &shop::View, path: &[usize]) -> (usize, bool) {
    let Some(&target) = path.last() else {
        return (0, false);
    };
    match offer_path(view, target, Some(path)) {
        shop::Offer::Plan(steps) => {
            let cost = steps.iter().map(|s| view.cat[s.item()].price).sum();
            (cost, view.live.gold >= cost)
        }
        _ => (path.iter().map(|i| view.cat[*i].price).sum(), false),
    }
}

type Projection = Option<(
    (Arc<Vec<shop::Item>>, shop::Live, Vec<shop::Order>, bool),
    (shop::Live, Vec<usize>),
)>;
thread_local! {
    static PROJECTION: std::cell::RefCell<Projection> = const { std::cell::RefCell::new(None) };
}
/// `shop::project` for the real inventory and queue, recomputed on change.
/// Reads "Vanilla order" directly, so toggling it while paused shows at once.
fn projected(view: &shop::View) -> (shop::Live, Vec<usize>) {
    let vanilla = crate::settings::option("shop_vanilla_order") == 1.;
    PROJECTION.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(((cat, live, orders, was), result)) = cache.as_ref() {
            if Arc::ptr_eq(cat, &view.cat)
                && *live == view.live
                && *orders == view.orders
                && *was == vanilla
            {
                return result.clone();
            }
        }
        let result = shop::project(&view.cat, &view.live, &view.orders, vanilla);
        *cache = Some((
            (
                view.cat.clone(),
                view.live.clone(),
                view.orders.clone(),
                vanilla,
            ),
            result.clone(),
        ));
        result
    })
}

pub(crate) const PATH: &str = "ingame.lt_shop";
pub static SCROLL: AtomicI32 = AtomicI32::new(0);
/// Open shop window rectangle, for wheel routing; None when closed.
pub static AREA: Mutex<Option<Rect>> = Mutex::new(None);
const WINDOW: (f32, f32, f32, f32) = (180., 78., 1360., 872.);
// Item grid viewport (window coordinates).
const VIEW_TOP: f32 = 84.;
const VIEW_BOTTOM: f32 = 770.;
const GRID_X: i32 = 228;
const COLS: usize = 7;
const TILE_W: i32 = 72;
const TILE_H: f32 = 80.;
const STEP_X: i32 = 80;
const STEP_Y: f32 = 88.;
const HEAD_H: f32 = 30.;
const NOTCH: f32 = 88.;
const TILES: usize = 63;
const HEADS: usize = 6;
const RECS: usize = 8;
const FILTERS: usize = 13;
const RECIPE_COLS: usize = 6;
// Branching recipes reserve the right-hand space for a proper choice button.
const RECIPE_CHOICE_COLS: usize = 5;
const RECIPE_ROWS: usize = 2;
const RECIPE: usize = RECIPE_COLS * RECIPE_ROWS;
const RECIPE_ROW_H: f32 = 84.;
const RECIPE_MODE_H: f32 = 48.;
const INTO: usize = 8;
const SLOTS: usize = 6;
const CHIPS: usize = 4;
const BODY_SIZE: f32 = 16.;
const BODY_W: f32 = 476.;
const BODY_TEXT_W: f32 = BODY_W - 12.;
/// Native label line spacing is in pixels.
const LINE_H: f32 = 23.;
const BUY_Y: f32 = 272.;
const RECIPE_Y: f32 = 204.;
const DETAIL_Y: f32 = 360.;
const BODY_Y: f32 = 446.;
const BODY_BOTTOM: f32 = 770.;
const TIP_W: i32 = 420;
/// Selected item frame (RGB 16, 162, 217), in the list and the recipe tree.
const SELECTED: &str = "10a2d9ff";
/// Message tones: bought, informational, refused.
const GOOD: &str = "fdee00ff";
const INFO: &str = "cbc9c7ff";
const BAD: &str = "ff642eff";

/// Stat filters, from the live item data's stat fields: works for vanilla,
/// the Riot pack and any item mod. Ticked filters combine (AND).
const STAT_FILTERS: [(&str, &[&str]); 12] = [
    ("Attack Damage", &["attack"]),
    ("Ability Power", &["magic_power"]),
    ("Attack Speed", &["attack_speed_mult"]),
    ("Critical Strike", &["crit_chance"]),
    ("Ability Haste", &["skill_cooldown_mult"]),
    ("Life Steal", &["vamp"]),
    ("Armor", &["defence"]),
    ("Magic Resist", &["magic_resistance"]),
    ("Health", &["hp", "hp_mult", "hp_regen"]),
    ("Move Speed", &["move_speed_mult"]),
    ("Armor Pen", &["defence_penetration"]),
    ("Magic Pen", &["magic_resistance_penetration"]),
];
/// Each stat filter's icon (crate::stat_icons), in STAT_FILTERS order.
const STAT_ICONS: [StatIcon; 12] = [
    stat_icons::AD,
    stat_icons::AP,
    stat_icons::ATTACK_SPEED,
    stat_icons::CRIT,
    stat_icons::HASTE,
    stat_icons::LIFESTEAL,
    stat_icons::ARMOR,
    stat_icons::MAGIC_RESIST,
    stat_icons::HEALTH,
    stat_icons::MOVE_SPEED,
    stat_icons::ARMOR_PEN,
    stat_icons::MAGIC_PEN,
];
fn has(item: &shop::Item, filter: usize) -> bool {
    STAT_FILTERS[filter]
        .1
        .iter()
        .any(|k| item.stats.iter().any(|s| s == k))
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Entry {
    Head(usize),
    Tile(usize),
}
/// Grid content as (entry, x, top) in scroll coordinates, plus content height.
fn layout(cat: &[shop::Item], active: &[usize]) -> (Vec<(Entry, i32, f32)>, f32) {
    let mut out = Vec::new();
    let mut y = 0.;
    let max_tier = cat.iter().map(|i| i.tier).max().unwrap_or(0);
    for tier in 0..=max_tier {
        let mut items: Vec<usize> = (0..cat.len())
            .filter(|i| cat[*i].tier == tier && active.iter().all(|f| has(&cat[*i], *f)))
            .collect();
        if items.is_empty() {
            continue;
        }
        items.sort_by(|a, b| (cat[*a].price, &cat[*a].key).cmp(&(cat[*b].price, &cat[*b].key)));
        if !out.is_empty() {
            y += 6.;
        }
        out.push((Entry::Head(tier), GRID_X, y));
        y += HEAD_H;
        for (n, item) in items.iter().enumerate() {
            let row = (n / COLS) as f32;
            out.push((
                Entry::Tile(*item),
                GRID_X + (n % COLS) as i32 * STEP_X,
                y + row * STEP_Y,
            ));
        }
        y += items.len().div_ceil(COLS) as f32 * STEP_Y;
    }
    (out, y)
}
fn max_scroll(height: f32) -> f32 {
    (height + 16. - (VIEW_BOTTOM - VIEW_TOP)).max(0.)
}

/// Virtual text viewport: whole lines avoid drawing over the fixed heading
/// or inventory. Each line carries its own color state when scrolled into view.
#[derive(Default)]
struct Description {
    source: Option<(String, String)>,
    lines: Vec<String>,
    first: usize,
    page: usize,
}
impl Description {
    fn prepare(&mut self, key: &str, body: &str, height: f32) {
        if self
            .source
            .as_ref()
            .is_none_or(|(k, b)| k != key || b != body)
        {
            self.source = Some((key.into(), body.into()));
            self.first = 0;
            let (wrapped, _) = crate::hud_style::wrap(body, BODY_SIZE, BODY_TEXT_W);
            let mut color = String::new();
            self.lines = wrapped
                .split('\n')
                .map(|line| {
                    let mut text = format!("{color}{line}");
                    let mut rest = line;
                    while let Some(start) = rest.find('<') {
                        rest = &rest[start..];
                        let Some(end) = rest.find('>') else {
                            break;
                        };
                        let tag = &rest[..=end];
                        if tag == "<>" {
                            color.clear();
                        } else if tag.starts_with("<#") {
                            color = tag.into();
                        }
                        rest = &rest[end + 1..];
                    }
                    if !color.is_empty() {
                        text.push_str("<>");
                    }
                    text
                })
                .collect();
        }
        self.page = (height / LINE_H).floor().max(1.) as usize;
        self.first = self.first.min(self.max());
    }
    fn max(&self) -> usize {
        self.lines.len().saturating_sub(self.page)
    }
    fn scroll(&mut self, notches: i32) {
        let steps = (notches.unsigned_abs() as usize).saturating_mul(3);
        self.first = if notches > 0 {
            self.first.saturating_sub(steps)
        } else {
            self.first.saturating_add(steps).min(self.max())
        };
    }
    fn text(&self) -> String {
        self.lines
            .iter()
            .skip(self.first)
            .take(self.page)
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Clone, Copy, Debug)]
enum Event {
    Close,
    Tab(bool),
    Filter(usize),
    Tile(usize),
    Rec(usize),
    Recipe(usize),
    RecipePage(bool),
    RecipeColumns(bool),
    RecipeAuto,
    RecipeUse(usize),
    Into(usize),
    Slot(usize),
    Chip(usize),
    Buy,
    Pause,
    Vanilla,
    QueueBuild,
}

fn json(text: &str) -> String {
    serde_json::to_string(text).unwrap()
}
#[allow(clippy::too_many_arguments)]
fn label(
    name: &str,
    (x, y, w, h): (i32, i32, i32, i32),
    size: i32,
    text: &str,
    bold: bool,
    color: &str,
    align: &str,
    z: i32,
) -> String {
    let style = if bold { "bold_label" } else { "label" };
    format!("#{name}:label {{ @\"asset/base/style/main#{style}\"; x: {x}px; y: {y}px; width: {w}px; height: {h}px; size: {size}; text: {}; color: #{color}; align_x: {align}; align_y: Center; ignore_event: true; z: {z}; }}\n", json(text))
}
fn glyph(name: &str, glyph: &str, (x, y, s): (i32, i32, i32), color: &str, z: i32) -> String {
    format!("#{name}:image {{ x: {x}px; y: {y}px; width: {s}px; height: {s}px; source: \"asset/lt_direct_control_probe/ui/{glyph}\"; color: #{color}; ignore_event: true; z: {z}; }}\n")
}
/// Item art; its source/rect_tag is set per frame from the HUD's item icon.
fn art(name: &str, (x, y, s): (i32, i32, i32), z: i32) -> String {
    format!("#{name}:image {{ x: {x}px; y: {y}px; width: {s}px; height: {s}px; color: #ffffffff; ignore_event: true; visible: false; z: {z}; }}\n")
}
fn rect(name: &str, (x, y, w, h): (i32, i32, i32, i32), color: &str, z: i32) -> String {
    format!("#{name}:color {{ x: {x}px; y: {y}px; width: {w}px; height: {h}px; color: #{color}; ignore_event: true; z: {z}; }}\n")
}
fn button(
    name: &str,
    (x, y, w, h): (i32, i32, i32, i32),
    text: &str,
    size: i32,
    z: i32,
    children: &str,
) -> String {
    format!("#{name}:color_icon_button {{ x: {x}px; y: {y}px; width: {w}px; height: {h}px; z: {z}; btn: {{ color: #00000000; back_color: #00000000; stroke: 0; rounding: Uniform {{ rounding: 2; }} }} text: {{ @\"asset/base/style/main#bold_label\"; align_x: Center; align_y: Center; text: {}; size: {size}; color: #1c1a18ff; }} {children}}}\n", json(text))
}

/// Same two contact-shadow layers as the settings buttons, with a crisp
/// stroke supplied by the button itself. Selection uses fill, never a glyph.
fn recipe_choice_button(name: &str, bounds: (i32, i32, i32, i32), text: &str) -> String {
    let (_, _, w, h) = bounds;
    let mut c = rect("shadow_outer", (-2, 2, w + 4, h + 3), "00000024", 1521);
    c.push_str(&rect(
        "shadow_contact",
        (-1, 3, w + 2, h + 1),
        "00000050",
        1522,
    ));
    button(name, bounds, text, 16, 1523, &c)
}

fn template() -> String {
    let mut s = format!("lt_shop:color {{ x: {}px; y: {}px; width: {}px; height: {}px; z: 1500; color: #~4b4a49ff; ignore_event: false; visible: false; rounding: Uniform {{ rounding: 2; }}\n", WINDOW.0, WINDOW.1, WINDOW.2, WINDOW.3);
    s.push_str(&rect(
        "fill",
        (1, 1, 1358, 870),
        &crate::ui_theme::hex(0xf5),
        1501,
    ));
    // Grid layer (z 1505-1509) sits under the masks and panels (z 1520+).
    for i in 0..HEADS {
        s.push_str(&format!("#head{i}:color {{ x: {GRID_X}px; y: 0px; width: 592px; height: 30px; color: #00000000; ignore_event: true; visible: false; z: 1505;\n{}{}}}\n",
            label("text", (0, 0, 300, 24), 14, "", true, "989694ff", "Left", 1506),
            rect("line", (170, 12, 422, 1), "~4b4a49ff", 1506)));
    }
    for i in 0..TILES {
        let mut c = rect("frame", (11, 3, 50, 50), "~4b4a49ff", 1506);
        c.push_str(&rect("fill", (12, 4, 48, 48), "161513ff", 1506));
        c.push_str(&art("icon", (12, 4, 48), 1507));
        c.push_str(&label(
            "price",
            (0, 56, TILE_W, 20),
            14,
            "",
            true,
            "eeececff",
            "Center",
            1507,
        ));
        c.push_str(&rect("badge", (50, 0, 18, 18), "eeececff", 1508));
        c.push_str(&label(
            "badge_text",
            (50, 0, 18, 18),
            12,
            "",
            true,
            "393939ff",
            "Center",
            1509,
        ));
        c.push_str(&glyph(
            "badge_check",
            "ef_check",
            (52, 2, 14),
            "393939ff",
            1509,
        ));
        s.push_str(&button(
            &format!("tile{i}"),
            (GRID_X, 0, TILE_W, TILE_H as i32),
            "",
            12,
            1505,
            &c,
        ));
    }
    // Recommended list.
    for i in 0..RECS {
        let mut c = rect("plate", (0, 0, 592, 68), "~292726ff", 1505);
        c.push_str(&art("icon", (12, 10, 48), 1506));
        c.push_str(&label(
            "name",
            (76, 8, 330, 26),
            18,
            "",
            true,
            "eeececff",
            "Left",
            1506,
        ));
        c.push_str(&label(
            "sub",
            (76, 36, 330, 22),
            14,
            "",
            false,
            "989694ff",
            "Left",
            1506,
        ));
        c.push_str(&label(
            "state",
            (380, 0, 200, 68),
            15,
            "",
            true,
            "fdee00ff",
            "Right",
            1506,
        ));
        s.push_str(&button(
            &format!("rec{i}"),
            (GRID_X, 120 + i as i32 * 74, 592, 68),
            "",
            12,
            1505,
            &c,
        ));
    }
    s.push_str(&label(
        "rec_head",
        (GRID_X, 84, 592, 26),
        14,
        "YOUR CHAMPION'S BUILD (FROM THE GAME)",
        true,
        "989694ff",
        "Left",
        1505,
    ));
    s.push_str(&button(
        "queue_build",
        (GRID_X + 392, 80, 200, 34),
        "Queue whole build",
        15,
        1505,
        "",
    ));
    s.push_str(&label(
        "rec_note",
        (GRID_X, 716, 592, 40),
        14,
        "",
        false,
        "989694ff",
        "Left",
        1505,
    ));
    // Masks clip partly visible grid rows.
    s.push_str(&rect(
        "mask_top",
        (208, 1, 632, 83),
        &crate::ui_theme::hex(0xff),
        1520,
    ));
    s.push_str(&rect(
        "mask_bottom",
        (208, 770, 632, 10),
        &crate::ui_theme::hex(0xff),
        1520,
    ));
    s.push_str(&rect("scroll_track", (828, 84, 4, 686), "~3a3837ff", 1521));
    s.push_str(&rect("scroll_thumb", (828, 84, 4, 120), "989694ff", 1522));
    s.push_str(&rect("grid_view", (GRID_X, 84, 604, 686), "00000000", 1501));
    // Header.
    // Strips are the background + 6 (design: #22201e on #1c1a18).
    s.push_str(&rect(
        "top",
        (1, 1, 1358, 67),
        &crate::ui_theme::shade(6, 0xff),
        1521,
    ));
    s.push_str(&rect("top_rule", (1, 68, 1358, 1), "~4b4a49ff", 1522));
    // Full-colour user artwork; the asset wrapper supplies the 7.5° CCW tilt.
    s.push_str("#title:image { x: 32px; y: 2px; width: 64px; height: 64px; source: \"asset/lt_direct_control_probe/ui/nerdge_stamp\"; color: #ffffffff; ignore_event: true; z: 1523; }");
    s.push_str(&rect("pill", (128, 18, 420, 32), "~3a3837ff", 1523));
    s.push_str(&glyph(
        "pill_icon",
        "ef_recall",
        (138, 25, 18),
        "7fd36bff",
        1524,
    ));
    s.push_str(&label(
        "pill_text",
        (164, 18, 380, 32),
        15,
        "",
        true,
        "7fd36bff",
        "Left",
        1524,
    ));
    // Pause while open: an Endfield checkbox in the header, saved like a setting.
    let mut pause = rect("box", (8, 6, 20, 20), "eeececff", 1524);
    pause.push_str(&rect("rail", (11, 9, 14, 14), "fdee00ff", 1525));
    pause.push_str(&glyph("check", "ef_check", (10, 8, 16), "ffffffff", 1526));
    pause.push_str(&label(
        "label",
        (38, 0, 200, 32),
        15,
        "Pause while open",
        true,
        "cbc9c7ff",
        "Left",
        1524,
    ));
    s.push_str(&button("pause", (566, 18, 230, 32), "", 12, 1523, &pause));
    // Vanilla order: the game's own rule, finish one item before the next.
    let mut vanilla = rect("box", (8, 6, 20, 20), "eeececff", 1524);
    vanilla.push_str(&rect("rail", (11, 9, 14, 14), "fdee00ff", 1525));
    vanilla.push_str(&glyph("check", "ef_check", (10, 8, 16), "ffffffff", 1526));
    vanilla.push_str(&label(
        "label",
        (38, 0, 200, 32),
        15,
        "Vanilla order",
        true,
        "cbc9c7ff",
        "Left",
        1524,
    ));
    s.push_str(&button(
        "vanilla",
        (806, 18, 230, 32),
        "",
        12,
        1523,
        &vanilla,
    ));
    s.push_str(&glyph(
        "gold_icon",
        "ef_coin",
        (1080, 22, 24),
        "fdee00ff",
        1523,
    ));
    s.push_str(&label(
        "gold",
        (1110, 14, 170, 40),
        28,
        "",
        true,
        "fdee00ff",
        "Left",
        1523,
    ));
    s.push_str(&button(
        "close",
        (1300, 12, 44, 44),
        "",
        12,
        1523,
        &glyph("icon", "ef_x", (11, 11, 22), "cbc9c7ff", 1524),
    ));
    // Left rail.
    s.push_str(&rect("rail_rule", (208, 69, 1, 711), "~4b4a49ff", 1521));
    s.push_str(&rect("tab_indicator", (12, 84, 184, 52), "eeececff", 1522));
    for (i, (name, text, icon)) in [
        ("tab_rec", "Recommended", "ef_star"),
        ("tab_all", "All items", "ef_grid"),
    ]
    .iter()
    .enumerate()
    {
        let mut c = glyph("icon", icon, (12, 16, 20), "cbc9c7ff", 1524);
        c.push_str(&label(
            "label",
            (40, 0, 142, 52),
            18,
            text,
            true,
            "cbc9c7ff",
            "Left",
            1524,
        ));
        s.push_str(&button(
            name,
            (12, 84 + i as i32 * 56, 184, 52),
            "",
            12,
            1523,
            &c,
        ));
    }
    s.push_str(&label(
        "filter_head",
        (18, 212, 170, 20),
        13,
        "FILTER",
        true,
        "989694ff",
        "Left",
        1523,
    ));
    for i in 0..FILTERS {
        let mut c = rect("bar", (0, 0, 3, 38), "fdee00ff", 1525);
        c.push_str(&label(
            "label",
            (15, 0, 120, 38),
            16,
            "",
            false,
            "cbc9c7ff",
            "Left",
            1524,
        ));
        c.push_str(&label(
            "count",
            (130, 0, 40, 38),
            14,
            "",
            true,
            "6f6d6bff",
            "Right",
            1524,
        ));
        s.push_str(&button(
            &format!("filter{i}"),
            (12, 236 + i as i32 * 38, 184, 38),
            "",
            12,
            1523,
            &c,
        ));
    }
    // Filter icons: above each filter row (not inside its button).
    for i in 0..FILTERS {
        for (f, icon) in STAT_ICONS.iter().enumerate() {
            s.push_str(&icon.node(
                &format!("ficon{i}_{f}"),
                (12 + 15, 236 + i as i32 * 38 + 10, 18),
                1526,
            ));
        }
    }
    // Detail panel.
    s.push_str(&rect("detail_rule", (840, 69, 1, 711), "~4b4a49ff", 1521));
    s.push_str(&rect(
        "detail_fill",
        (841, 69, 518, 711),
        &crate::ui_theme::hex(0xff),
        1521,
    ));
    // Right panel, League order: Builds into (focused item) → recipe tree
    // (root, fixed until an item is picked from the list) → Buy → detail.
    s.push_str(&label(
        "into_head",
        (862, 82, 476, 20),
        13,
        "BUILDS INTO",
        true,
        "989694ff",
        "Left",
        1523,
    ));
    s.push_str(&label(
        "into_none",
        (862, 106, 476, 40),
        14,
        "Final item",
        false,
        "6f6d6bff",
        "Left",
        1523,
    ));
    for i in 0..INTO {
        let mut c = rect("frame", (3, 1, 38, 38), "~4b4a49ff", 1524);
        c.push_str(&rect("fill", (4, 2, 36, 36), "161513ff", 1524));
        c.push_str(&art("icon", (4, 2, 36), 1525));
        c.push_str(&label(
            "price",
            (-6, 40, 56, 18),
            12,
            "",
            true,
            "cbc9c7ff",
            "Center",
            1525,
        ));
        c.push_str(&glyph("check", "ef_check", (10, 8, 24), "eeececff", 1526));
        s.push_str(&button(
            &format!("into{i}"),
            (862 + i as i32 * 58, 104, 44, 58),
            "",
            12,
            1523,
            &c,
        ));
    }
    s.push_str(&rect("into_rule", (841, 170, 518, 1), "~4b4a49ff", 1522));
    s.push_str(&label(
        "recipe_head",
        (862, 180, 220, 20),
        12,
        "RECIPE",
        true,
        "989694ff",
        "Left",
        1523,
    ));
    s.push_str(&recipe_choice_button(
        "recipe_auto",
        (862, RECIPE_Y as i32, 224, 40),
        "Automatic (cheapest)",
    ));
    s.push_str(&label(
        "recipe_pages",
        (1212, 180, 72, 20),
        12,
        "",
        false,
        "989694ff",
        "Center",
        1523,
    ));
    s.push_str(&button(
        "recipe_prev",
        (1286, 180, 24, 20),
        "<",
        14,
        1523,
        "",
    ));
    s.push_str(&button(
        "recipe_next",
        (1314, 180, 24, 20),
        ">",
        14,
        1523,
        "",
    ));
    for row in 0..RECIPE_ROWS {
        s.push_str(&label(
            &format!("recipe_path{row}"),
            (
                862,
                RECIPE_Y as i32 + row as i32 * RECIPE_ROW_H as i32,
                288,
                18,
            ),
            12,
            "",
            true,
            "989694ff",
            "Left",
            1523,
        ));
        s.push_str(&recipe_choice_button(
            &format!("recipe_use{row}"),
            (
                1178,
                (RECIPE_Y + RECIPE_MODE_H + 28.) as i32 + row as i32 * RECIPE_ROW_H as i32,
                160,
                40,
            ),
            "Use this path",
        ));
    }
    s.push_str(&label(
        "recipe_steps",
        (1050, RECIPE_Y as i32, 128, 18),
        12,
        "",
        false,
        "989694ff",
        "Right",
        1523,
    ));
    s.push_str(&button(
        "recipe_left",
        (1286, RECIPE_Y as i32, 24, 18),
        "<",
        14,
        1523,
        "",
    ));
    s.push_str(&button(
        "recipe_right",
        (1314, RECIPE_Y as i32, 24, 18),
        ">",
        14,
        1523,
        "",
    ));
    for i in 0..RECIPE {
        let col = i % RECIPE_COLS;
        let y = RECIPE_Y as i32 + 20 + (i / RECIPE_COLS) as i32 * RECIPE_ROW_H as i32;
        let mut c = rect("frame", (3, 1, 38, 38), "~4b4a49ff", 1524);
        c.push_str(&rect("fill", (4, 2, 36, 36), "161513ff", 1524));
        c.push_str(&art("icon", (4, 2, 36), 1525));
        c.push_str(&label(
            "price",
            (-6, 40, 56, 18),
            12,
            "",
            true,
            "cbc9c7ff",
            "Center",
            1525,
        ));
        // Owned parts: dimmed icon with a tick over it.
        c.push_str(&glyph("check", "ef_check", (10, 8, 24), "eeececff", 1526));
        s.push_str(&button(
            &format!("recipe{i}"),
            (862 + col as i32 * 66, y, 44, 58),
            "",
            12,
            1523,
            &c,
        ));
        if col + 1 < RECIPE_COLS {
            s.push_str(&rect(
                &format!("recipe_link{i}"),
                (906 + col as i32 * 66, y + 20, 22, 2),
                "~4b4a49ff",
                1523,
            ));
        }
    }
    s.push_str(&button(
        "buy",
        (862, BUY_Y as i32, 476, 52),
        "",
        19,
        1523,
        "",
    ));
    s.push_str(&label(
        "buy_hint",
        (862, BUY_Y as i32 + 56, 476, 20),
        13,
        "",
        false,
        "989694ff",
        "Center",
        1523,
    ));
    s.push_str(&rect(
        "d_rule",
        (841, DETAIL_Y as i32 - 12, 518, 1),
        "~4b4a49ff",
        1522,
    ));
    s.push_str(&rect(
        "d_frame",
        (861, DETAIL_Y as i32, 66, 66),
        "~4b4a49ff",
        1522,
    ));
    s.push_str(&art("d_icon", (862, DETAIL_Y as i32 + 1, 64), 1523));
    s.push_str(&label(
        "d_name",
        (944, DETAIL_Y as i32 - 2, 396, 32),
        24,
        "",
        true,
        "eeececff",
        "Left",
        1523,
    ));
    s.push_str(&label(
        "d_cost",
        (944, DETAIL_Y as i32 + 30, 396, 24),
        15,
        "",
        false,
        "cbc9c7ff",
        "Left",
        1523,
    ));
    s.push_str(&label(
        "d_tags",
        (944, DETAIL_Y as i32 + 54, 396, 22),
        12,
        "",
        true,
        "989694ff",
        "Left",
        1523,
    ));
    s.push_str(&rect(
        "d_view",
        (862, BODY_Y as i32, BODY_W as i32, 300),
        "00000000",
        1521,
    ));
    s.push_str(&format!("#d_body:label {{ @\"asset/base/style/main#label\"; x: 862px; y: {}px; width: {BODY_TEXT_W}px; height: 300px; size: {}; text: \"\"; color: #d6d6d6ff; align_x: Left; align_y: Top; line_height: {}; ignore_event: true; z: 1523; }}\n", BODY_Y as i32, BODY_SIZE as i32, LINE_H as i32));
    s.push_str(&rect(
        "d_scroll_track",
        (1334, BODY_Y as i32, 4, 300),
        "~3a3837ff",
        1523,
    ));
    s.push_str(&rect(
        "d_scroll_thumb",
        (1334, BODY_Y as i32, 4, 40),
        "989694ff",
        1524,
    ));
    // Hover tooltip, following the cursor while it stays over the item.
    let mut tip = rect("art_frame", (15, 15, 50, 50), "~4b4a49ff", 1551);
    tip.push_str(&art("art", (16, 16, 48), 1552));
    tip.push_str(&label(
        "title",
        (76, 12, 330, 30),
        22,
        "",
        true,
        "ffffffff",
        "Left",
        1552,
    ));
    tip.push_str(&label(
        "price",
        (76, 42, 330, 22),
        15,
        "",
        true,
        "fdee00ff",
        "Left",
        1552,
    ));
    tip.push_str(&rect("rule", (16, 76, 388, 1), "ffffff3b", 1551));
    tip.push_str(&format!("#body:label {{ @\"asset/base/style/main#label\"; x: 16px; y: 86px; width: 388px; height: 100px; size: {}; text: \"\"; color: #d6d6d6ff; align_x: Left; align_y: Top; line_height: {}; ignore_event: true; z: 1552; }}\n", BODY_SIZE as i32, LINE_H as i32));
    s.push_str(&format!("#tip:color {{ x: 0px; y: 0px; width: {TIP_W}px; height: 160px; color: #~1e1e1dd9; rounding: Uniform {{ rounding: 8; }} ignore_event: true; visible: false; z: 1550;\n{tip}}}\n"));
    // Bottom bar.
    s.push_str(&rect(
        "bottom",
        (1, 780, 1358, 91),
        &crate::ui_theme::shade(6, 0xff),
        1521,
    ));
    s.push_str(&rect("bottom_rule", (1, 780, 1358, 1), "~4b4a49ff", 1522));
    s.push_str(&label(
        "items_head",
        (24, 796, 80, 18),
        13,
        "ITEMS",
        true,
        "989694ff",
        "Left",
        1523,
    ));
    s.push_str(&label(
        "items_count",
        (24, 816, 80, 26),
        18,
        "",
        true,
        "eeececff",
        "Left",
        1523,
    ));
    for i in 0..SLOTS {
        let mut c = rect("fill", (1, 1, 56, 56), "161513ff", 1524);
        c.push_str(&art("icon", (4, 4, 50), 1525));
        // Projected purchase not yet made by the game (e.g. while paused).
        c.push_str(&glyph(
            "pending",
            "ef_clock",
            (38, 38, 16),
            "fdee00ff",
            1526,
        ));
        s.push_str(&format!("#slot{i}:color {{ x: {}px; y: 797px; width: 58px; height: 58px; color: #~4b4a49ff; ignore_event: false; visible: false; z: 1523;\n{c}}}\n", 110 + i as i32 * 64));
    }
    s.push_str(&label(
        "queue_head",
        (520, 796, 80, 18),
        13,
        "QUEUED",
        true,
        "989694ff",
        "Left",
        1523,
    ));
    s.push_str(&label(
        "queue_total",
        (520, 816, 80, 26),
        18,
        "",
        true,
        "eeececff",
        "Left",
        1523,
    ));
    for i in 0..CHIPS {
        let mut c = rect("plate", (0, 0, 176, 50), "~292726ff", 1524);
        c.push_str(&art("icon", (6, 7, 36), 1525));
        c.push_str(&label(
            "name",
            (48, 4, 100, 24),
            14,
            "",
            true,
            "eeececff",
            "Left",
            1525,
        ));
        c.push_str(&label(
            "price",
            (48, 26, 100, 20),
            13,
            "",
            true,
            "fdee00ff",
            "Left",
            1525,
        ));
        c.push_str(&glyph("x", "ef_x", (152, 17, 16), "989694ff", 1525));
        s.push_str(&button(
            &format!("chip{i}"),
            (610 + i as i32 * 184, 801, 176, 50),
            "",
            12,
            1523,
            &c,
        ));
    }
    s.push_str(&label(
        "queue_empty",
        (610, 801, 300, 50),
        15,
        "Nothing queued",
        false,
        "6f6d6bff",
        "Left",
        1523,
    ));
    // Message plate under the Buy button: solid, with a tone marker bar.
    let mut toast = rect("bar", (0, 0, 4, 34), "fdee00ff", 1531);
    toast.push_str(&label(
        "text",
        (16, 0, 470, 34),
        15,
        "",
        true,
        "ffffffff",
        "Left",
        1531,
    ));
    s.push_str(&format!("#toast:color {{ x: 852px; y: {}px; width: 496px; height: 34px; color: #~1e1e1df2; rounding: Uniform {{ rounding: 4; }} ignore_event: true; visible: false; z: 1530;\n{toast}}}\n", BUY_Y as i32 + 54));
    s.push_str("}\n");
    crate::hud_style::fonts(s)
}

/// Only the visible paths are materialized. Ownership changes the automatic
/// route marker, never the structural graph or the branch the user browsed.
#[derive(Default)]
struct RecipeBrowser {
    cached: Option<(Arc<Vec<shop::Item>>, usize, crate::shop_recipe::Tree)>,
    paths: Vec<Vec<usize>>,
    browsed: Vec<usize>,
    page: usize,
    column: usize,
    auto: Option<usize>,
}

impl RecipeBrowser {
    fn follow(&mut self, focus: usize, target: usize) {
        let from = if self.browsed.contains(&focus) {
            Some(&self.browsed)
        } else {
            self.paths.iter().find(|path| path.contains(&focus))
        };
        let mut path = from
            .map(|p| {
                p.iter()
                    .copied()
                    .take_while(|i| *i != focus)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        path.extend([focus, target]);
        self.browsed = path;
        self.cached = None;
        self.page = 0;
        self.column = 0;
    }

    fn prepare(&mut self, view: &shop::View, root: usize, chosen: Option<&[usize]>) -> Vec<usize> {
        let fresh = self
            .cached
            .as_ref()
            .is_some_and(|(cat, item, _)| Arc::ptr_eq(cat, &view.cat) && *item == root);
        if !fresh {
            self.cached = Some((
                view.cat.clone(),
                root,
                crate::shop_recipe::Tree::new(&view.cat, root),
            ));
        }
        let auto = match offer_path(view, root, chosen) {
            shop::Offer::Plan(steps) => {
                let start = match steps.first() {
                    Some(shop::Step::Upgrade { slot, .. }) => view.live.owned.get(*slot).copied(),
                    _ => None,
                };
                start
                    .into_iter()
                    .chain(steps.iter().map(|s| s.item()))
                    .collect::<Vec<_>>()
            }
            _ => Vec::new(),
        };
        let tree = &self.cached.as_ref().unwrap().2;
        self.auto = tree.index_for_suffix(&auto);
        if !fresh {
            let chosen = tree
                .index_for_suffix(chosen.unwrap_or(&self.browsed))
                .or(self.auto)
                .unwrap_or(0);
            self.page = chosen / RECIPE_ROWS;
            self.column = 0;
        }
        self.page = self.page.min(tree.len().saturating_sub(1) / RECIPE_ROWS);
        let start = self.page * RECIPE_ROWS;
        self.paths = (start..start.saturating_add(RECIPE_ROWS).min(tree.len()))
            .filter_map(|i| tree.path(i))
            .collect();
        self.column = self.column.min(self.max_column());
        auto
    }

    fn total(&self) -> usize {
        self.cached.as_ref().map_or(0, |(_, _, tree)| tree.len())
    }

    fn max_column(&self) -> usize {
        self.paths
            .iter()
            .map(Vec::len)
            .max()
            .unwrap_or(0)
            .saturating_sub(1)
            / self.columns()
    }

    fn columns(&self) -> usize {
        if self.branched() {
            RECIPE_CHOICE_COLS
        } else {
            RECIPE_COLS
        }
    }

    fn branched(&self) -> bool {
        self.total() > 1
    }

    fn detailed(&self) -> bool {
        self.branched() || self.max_column() > 0
    }

    fn paths_y(&self) -> f32 {
        RECIPE_Y + if self.branched() { RECIPE_MODE_H } else { 0. }
    }

    fn buy_y(&self) -> f32 {
        if self.detailed() {
            self.paths_y() + self.paths.len().max(1) as f32 * RECIPE_ROW_H + 10.
        } else {
            BUY_Y
        }
    }

    fn item_y(&self, slot: usize) -> f32 {
        self.paths_y()
            + if self.detailed() { 20. } else { 0. }
            + (slot / RECIPE_COLS) as f32 * RECIPE_ROW_H
    }

    fn cells(&self) -> Vec<Option<usize>> {
        self.paths
            .iter()
            .flat_map(|path| {
                (0..RECIPE_COLS).map(|i| {
                    if i < self.columns() {
                        path.get(self.column * self.columns() + i).copied()
                    } else {
                        None
                    }
                })
            })
            .collect()
    }
}

#[derive(Default)]
pub struct ShopUi {
    open: bool,
    recommended: bool,
    /// Ticked stat filters (STAT_FILTERS indices); empty shows everything.
    active: Vec<usize>,
    scroll: f32,
    description: Description,
    /// The shop paused this match (bound to its session); only then resume.
    paused_by_me: Option<(crate::native_timing::MatchKey, u64)>,
    /// In-base state at the last running frame, held while paused.
    last_in_base: bool,
    /// Recipe-tree root: the item last picked from the list (grid,
    /// Recommended, slots). Clicks inside the tree never change it.
    root: Option<usize>,
    /// Focused item: Builds into, Buy and detail follow it.
    focus: Option<usize>,
    /// Hover tooltip item and when it appeared.
    tip: Option<(usize, Instant)>,
    /// Real inventory/gold before the projected purchases.
    real: shop::Live,
    /// Items the projection buys that the game has not bought yet.
    pending: Vec<usize>,
    paused: bool,
    events: Arc<Mutex<Vec<Event>>>,
    tiles: Vec<Option<usize>>,
    recs: Vec<usize>,
    recipe: Vec<Option<usize>>,
    recipes: RecipeBrowser,
    /// Per-item choices for this match. Queued orders hold their own copy.
    /// No preference is manufactured for a catalogue's single linear path.
    choices: HashMap<usize, Vec<usize>>,
    into: Vec<usize>,
    chips: Vec<usize>,
    /// Filter button slots: None = "All", Some(i) = STAT_FILTERS[i].
    filters: Vec<Option<usize>>,
    cache: HashMap<String, String>,
    names: HashMap<String, String>,
    bodies: HashMap<String, String>,
    spawn: Option<Instant>,
    registered: bool,
    previous_shop: bool,
    previous_escape: bool,
    previous_right: bool,
    toast: Option<(String, Instant, &'static str)>,
    motion: hud_motion::Motion,
    match_key: Option<crate::native_timing::MatchKey>,
    /// The match whose shop already opened by itself.
    auto_opened: Option<crate::native_timing::MatchKey>,
}

impl ShopUi {
    /// The strip's shop button.
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }
    fn props(&mut self, ctx: &mut StableClient<'_>, node: &str, text: String) {
        let path = if node.is_empty() {
            PATH.to_owned()
        } else {
            format!("{PATH}.{node}")
        };
        hud_motion::properties(ctx, &mut self.cache, &path, &text);
    }
    fn text(&mut self, ctx: &mut StableClient<'_>, node: &str, text: &str) {
        let path = format!("{PATH}.{node}");
        let key = format!("{path}#text");
        if self.cache.get(&key).is_none_or(|t| t != text) && ctx.ui_set_text(&path, text) {
            self.cache.insert(key, text.into());
        }
    }
    fn visible(&mut self, ctx: &mut StableClient<'_>, node: &str, on: bool) {
        self.props(ctx, node, format!("visible: {on};"));
    }
    fn hovered(ctx: &StableClient<'_>, node: &str, cursor: Option<(f32, f32)>) -> bool {
        cursor.is_some_and(|p| {
            ctx.ui_node_rect(&format!("{PATH}.{node}"))
                .is_some_and(|(x, y, w, h)| Rect { x, y, w, h }.contains(p))
        })
    }
    fn icon(&mut self, ctx: &mut StableClient<'_>, node: &str, hud: &HudUi, key: Option<&str>) {
        match key.and_then(|k| hud.item_icon(k)) {
            Some(icon) => self.props(ctx, node, format!("visible: true; {}", icon.properties())),
            None => self.visible(ctx, node, false),
        }
    }
    fn name(&mut self, ctx: &StableClient<'_>, hud: &HudUi, key: &str) -> String {
        if !self.bodies.contains_key(key) {
            let text = hud.item_text(ctx, key);
            let (title, body) = text.split_once('\n').unwrap_or((&text, ""));
            self.names.insert(key.into(), crate::tooltips::plain(title));
            self.bodies.insert(key.into(), body.trim().to_owned());
        }
        self.names[key].clone()
    }
    fn pick(&mut self, item: Option<usize>) {
        if let Some(item) = item {
            self.root = Some(item);
            self.focus = Some(item);
            self.recipes = RecipeBrowser::default();
        }
    }
    /// The item under the cursor in the grid, Recommended, recipe tree,
    /// Builds into, inventory slots or queue chips.
    fn hovered_item(
        &self,
        ctx: &StableClient<'_>,
        view: &shop::View,
        cursor: Option<(f32, f32)>,
    ) -> Option<usize> {
        cursor?;
        let over = |node: String| Self::hovered(ctx, &node, cursor);
        (0..TILES)
            .find_map(|i| {
                self.tiles
                    .get(i)
                    .copied()
                    .flatten()
                    .filter(|_| over(format!("tile{i}")))
            })
            .or_else(|| {
                (0..self.recs.len()).find_map(|i| over(format!("rec{i}")).then(|| self.recs[i]))
            })
            .or_else(|| {
                (0..self.recipe.len())
                    .find_map(|i| self.recipe[i].filter(|_| over(format!("recipe{i}"))))
            })
            .or_else(|| {
                (0..self.into.len()).find_map(|i| over(format!("into{i}")).then(|| self.into[i]))
            })
            .or_else(|| {
                (0..view.live.owned.len().min(SLOTS))
                    .find_map(|i| over(format!("slot{i}")).then(|| view.live.owned[i]))
            })
            .or_else(|| {
                (0..self.chips.len()).find_map(|i| over(format!("chip{i}")).then(|| self.chips[i]))
            })
    }
    fn say(&mut self, text: impl Into<String>) {
        self.say_tone(text, INFO);
    }
    /// Message on a solid plate above the Buy button, with a tone marker.
    fn say_tone(&mut self, text: impl Into<String>, tone: &'static str) {
        self.toast = Some((text.into(), Instant::now(), tone));
    }
    fn register(&mut self, ctx: &mut StableClient<'_>) {
        let mut items: Vec<(String, Event)> = vec![
            ("close".into(), Event::Close),
            ("tab_rec".into(), Event::Tab(true)),
            ("tab_all".into(), Event::Tab(false)),
            ("buy".into(), Event::Buy),
            ("pause".into(), Event::Pause),
            ("vanilla".into(), Event::Vanilla),
            ("queue_build".into(), Event::QueueBuild),
            ("recipe_prev".into(), Event::RecipePage(false)),
            ("recipe_next".into(), Event::RecipePage(true)),
            ("recipe_left".into(), Event::RecipeColumns(false)),
            ("recipe_right".into(), Event::RecipeColumns(true)),
            ("recipe_auto".into(), Event::RecipeAuto),
        ];
        items.extend((0..FILTERS).map(|i| (format!("filter{i}"), Event::Filter(i))));
        items.extend((0..TILES).map(|i| (format!("tile{i}"), Event::Tile(i))));
        items.extend((0..RECS).map(|i| (format!("rec{i}"), Event::Rec(i))));
        items.extend((0..RECIPE).map(|i| (format!("recipe{i}"), Event::Recipe(i))));
        items.extend((0..RECIPE_ROWS).map(|i| (format!("recipe_use{i}"), Event::RecipeUse(i))));
        items.extend((0..INTO).map(|i| (format!("into{i}"), Event::Into(i))));
        items.extend((0..SLOTS).map(|i| (format!("slot{i}"), Event::Slot(i))));
        items.extend((0..CHIPS).map(|i| (format!("chip{i}"), Event::Chip(i))));
        for (node, event) in items {
            let queue = self.events.clone();
            ctx.ui_register_path_events(&format!("{PATH}.{node}"), move |ctx| {
                if ctx
                    .ui_current_event()
                    .is_some_and(|e| e.kind == Some(UiEventKindV1::Click))
                {
                    if let Ok(mut q) = queue.lock() {
                        q.push(event);
                    }
                }
            });
        }
    }
    fn buy(&mut self, view: &shop::View, item: usize, log: &Logger) {
        let name = view.cat[item].key.clone();
        if !view.manual {
            self.say("Manual shopping is off: turn it on in Settings › Combat & casting");
            return;
        }
        match self.purchase_offer(view, item) {
            shop::Offer::Owned => {
                self.say_tone("This item cannot be bought", BAD);
                return;
            }
            shop::Offer::Blocked => {
                self.say_tone("No free slot: upgrade an item you own instead", BAD);
                return;
            }
            shop::Offer::Plan(_) => {}
        }
        let affordable = self.buyable(view, item);
        let mut order = shop::new_order(&view.live, item);
        order.path = self.chosen_path(item);
        let path_keys = order.path.as_ref().map(|path| {
            path.iter()
                .map(|i| view.cat[*i].key.clone())
                .collect::<Vec<_>>()
        });
        shop::SHOP.buy(order);
        log.write(&format!(
            "SHOP UI buy {name} in_base={} paused={} path={path_keys:?}",
            view.in_base, self.paused
        ));
        if !view.in_base {
            self.say_tone("Queued · buys at your next base visit", INFO);
        } else if !affordable {
            self.say_tone("Queued · not enough gold yet", INFO);
        } else if self.paused {
            self.say_tone("Bought · applies when the match resumes", GOOD);
        } else {
            self.say_tone("Bought", GOOD);
        }
    }

    /// Optional pause while the shop is shown, using the Settings window's
    /// pause request. Only a pause this shop made is ever resumed, and
    /// nothing changes while the (modal) Settings window is open.
    fn update_pause(
        &mut self,
        timing: &crate::native_timing::NativeTiming,
        want: bool,
        modal: bool,
        log: &Logger,
    ) {
        use crate::native_timing::Phase;
        if modal {
            return;
        }
        let session = timing.match_key().map(|k| (k, timing.generation()));
        if want && self.paused_by_me.is_none() && timing.phase() == Some(Phase::Running) {
            timing.request_action(true);
            if let Some(action) = timing.take_action() {
                timing.apply_action(action, log);
            }
            if timing.phase() == Some(Phase::Paused) {
                self.paused_by_me = session;
                log.write("SHOP UI paused the match");
            }
        } else if !want {
            if let Some(bound) = self.paused_by_me.take() {
                if Some(bound) == session && timing.phase() == Some(Phase::Paused) {
                    timing.request_action(true);
                    log.write("SHOP UI resumed the match");
                }
            }
        }
    }

    /// Per client frame. `active`: battlefield under direct control.
    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &mut self,
        ctx: &mut StableClient<'_>,
        keys: Keys,
        active: bool,
        timing: &crate::native_timing::NativeTiming,
        hud: &HudUi,
        log: &Logger,
    ) -> Vec<Rect> {
        let match_key = timing.match_key();
        let shop_edge = keys.shop && !self.previous_shop && keys.focused;
        let escape_edge = keys.raw.0[0x1b] && !self.previous_escape;
        let right_edge = keys.raw.0[2] && !self.previous_right && keys.focused;
        self.previous_shop = keys.shop;
        self.previous_escape = keys.raw.0[0x1b];
        self.previous_right = keys.raw.0[2];
        if match_key != self.match_key {
            self.match_key = match_key;
            self.open = false;
            self.root = None;
            self.focus = None;
            self.recipes = RecipeBrowser::default();
            self.choices.clear();
            self.recipe.clear();
            self.into.clear();
            self.names.clear();
            self.bodies.clear();
            self.description = Description::default();
        }
        let modal = crate::ui_state::SETTINGS_OPEN.load(Ordering::Relaxed);
        if !active {
            self.open = false;
        } else if shop_edge && !modal {
            self.open = !self.open;
        } else if escape_edge && self.open {
            self.open = false;
        }
        let mut view = shop::SHOP.view();
        // Manual shopping: open once per match when the game has completed
        // the build plan (Recommended is then whole); "Pause while open"
        // applies as for any opening.
        if active
            && match_key.is_some()
            && self.auto_opened != match_key
            && view.as_ref().is_some_and(|v| v.manual && v.planned)
        {
            self.auto_opened = match_key;
            self.open = true;
            self.recommended = true;
            log.write("SHOP UI opened by itself: build plan complete");
        }
        let shown = self.open && !modal && view.is_some();
        self.update_pause(
            timing,
            shown && crate::settings::option("shop_pause") == 1.,
            modal,
            log,
        );
        // The buyer only asks while the simulation runs: hold the last
        // in-base answer while the match is paused.
        if let Some(v) = view.as_mut() {
            if timing.phase() == Some(crate::native_timing::Phase::Paused) {
                v.in_base = self.last_in_base;
            } else {
                self.last_in_base = v.in_base;
            }
        }
        crate::ui_state::SHOP_OPEN.store(shown, Ordering::Relaxed);
        if let Ok(mut area) = AREA.lock() {
            *area = shown.then_some(Rect {
                x: WINDOW.0,
                y: WINDOW.1,
                w: WINDOW.2,
                h: WINDOW.3,
            });
        }
        let Some(mut view) = view.filter(|_| shown) else {
            if ctx.ui_exists(PATH) {
                ctx.ui_set_visible(PATH, false);
            }
            SCROLL.store(0, Ordering::Relaxed);
            return Vec::new();
        };
        // Show the result of queued purchases right away: in base the game
        // makes them one per frame (and only after resuming when paused), so
        // prices, dimming and Buy all use the projected inventory and gold.
        self.paused = timing.phase() == Some(crate::native_timing::Phase::Paused);
        self.real = view.live.clone();
        self.pending.clear();
        if view.manual && view.in_base {
            let (projected, bought) = projected(&view);
            view.live = projected;
            self.pending = bought;
        }
        if !ctx.ui_exists(PATH) {
            if self
                .spawn
                .is_some_and(|t| t.elapsed() < Duration::from_secs(1))
            {
                return Vec::new();
            }
            self.spawn = Some(Instant::now());
            self.cache.clear();
            self.registered = false;
            if !ctx.ui_spawn_source("ingame", &crate::ui_theme::themed(&template()))
                || !ctx.ui_exists(PATH)
            {
                self.open = false;
                log.write("SHOP UI spawn failed");
                return Vec::new();
            }
            log.write("SHOP UI window spawned");
        }
        if !self.registered {
            self.registered = true;
            self.register(ctx);
        }
        ctx.ui_set_visible(PATH, true);
        let cursor = keys.cursor.filter(|_| keys.focused);
        let (entries, height) = layout(&view.cat, &self.active);
        let events = self
            .events
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default();
        for event in events {
            match event {
                Event::Close => self.open = false,
                Event::Tab(rec) => {
                    if self.recommended != rec {
                        self.recommended = rec;
                        self.scroll = 0.;
                    }
                }
                Event::Filter(i) => {
                    match self.filters.get(i).copied() {
                        Some(None) => self.active.clear(),
                        Some(Some(f)) => {
                            if let Some(at) = self.active.iter().position(|a| *a == f) {
                                self.active.remove(at);
                            } else {
                                self.active.push(f);
                            }
                        }
                        None => {}
                    }
                    self.scroll = 0.;
                }
                Event::Vanilla => {
                    let on = crate::settings::option("shop_vanilla_order") == 1.;
                    crate::settings::set_option("shop_vanilla_order", if on { 0. } else { 1. });
                }
                Event::Pause => {
                    let on = crate::settings::option("shop_pause") == 1.;
                    crate::settings::set_option("shop_pause", if on { 0. } else { 1. });
                }
                Event::QueueBuild => {
                    if !view.manual {
                        self.say(
                            "Manual shopping is off: turn it on in Settings › Combat & casting",
                        );
                    } else {
                        let added = shop::SHOP.enqueue_back(&view.live.build, |item| {
                            self.choices.get(&item).cloned()
                        });
                        log.write(&format!("SHOP UI queue whole build added={added}"));
                        self.say(if added == 0 {
                            "The whole build is already queued or owned".to_owned()
                        } else {
                            format!(
                                "Queued {added} item{} from the build",
                                if added > 1 { "s" } else { "" }
                            )
                        });
                    }
                }
                // Picking from the list or Builds into sets the tree root and
                // focus; clicks inside the tree only move the focus (League).
                Event::Tile(i) => self.pick(self.tiles.get(i).copied().flatten()),
                Event::Rec(i) => self.pick(self.recs.get(i).copied()),
                Event::Slot(i) => self.pick(view.live.owned.get(i).copied()),
                Event::Recipe(i) => {
                    if let Some(item) = self.recipe.get(i).copied().flatten() {
                        self.focus = Some(item);
                        if let Some(path) = self.recipes.paths.get(i / RECIPE_COLS) {
                            self.recipes.browsed = path.clone();
                        }
                    }
                }
                Event::RecipePage(next) => {
                    self.recipes.page = if next {
                        self.recipes.page.saturating_add(1)
                    } else {
                        self.recipes.page.saturating_sub(1)
                    };
                    self.recipes.column = 0;
                }
                Event::RecipeColumns(next) => {
                    self.recipes.column = if next {
                        self.recipes.column.saturating_add(1)
                    } else {
                        self.recipes.column.saturating_sub(1)
                    };
                }
                Event::RecipeAuto => {
                    if let Some(root) = self.root {
                        self.choices.remove(&root);
                        self.recipes.browsed.clear();
                        self.recipes.cached = None;
                        self.focus = Some(root);
                        self.say("Automatic cheapest path · applies to new purchases");
                    }
                }
                Event::RecipeUse(row) => {
                    if let (Some(root), Some(path)) =
                        (self.root, self.recipes.paths.get(row).cloned())
                    {
                        if self.recipes.total() > 1 {
                            self.choices.insert(root, path);
                            self.focus = Some(root);
                            self.say("Chosen path · applies to new purchases");
                        }
                    }
                }
                // Builds into moves up the tree: the clicked item becomes the root.
                Event::Into(i) => {
                    if let (Some(focus), Some(target)) = (self.focus, self.into.get(i).copied()) {
                        self.recipes.follow(focus, target);
                        self.root = Some(target);
                        self.focus = Some(target);
                    }
                }
                Event::Chip(i) => {
                    // Chips are the first orders in queue order: remove just this one.
                    if let Some(item) = self.chips.get(i).copied() {
                        shop::SHOP.unqueue(i);
                        log.write(&format!("SHOP UI unqueue order {i} {}", view.cat[item].key));
                    }
                }
                Event::Buy => {
                    if let Some(item) = self.focus {
                        self.buy(&view, item, log);
                    }
                }
            }
        }
        if !self.open {
            ctx.ui_set_visible(PATH, false);
            return Vec::new();
        }
        let notches = SCROLL.swap(0, Ordering::Relaxed);
        if !self.recommended {
            let grid_notches = if Self::hovered(ctx, "grid_view", cursor) {
                notches
            } else {
                0
            };
            self.scroll = (self.scroll - grid_notches as f32 * NOTCH).clamp(0., max_scroll(height));
        }
        // Hover shows a tooltip at the cursor; right-click buys the hovered item.
        let hovered = self.hovered_item(ctx, &view, cursor);
        if right_edge {
            if let Some(item) = hovered {
                self.focus = Some(item);
                self.buy(&view, item, log);
            }
        }
        if self.root.is_none() {
            let first = view
                .live
                .build
                .iter()
                .copied()
                .find(|t| !view.live.owned.contains(t))
                .or(view.live.build.first().copied());
            self.pick(first);
        }
        self.render_top(ctx, &view);
        self.render_rail(ctx, &view);
        if self.recommended {
            self.hide_grid(ctx);
            self.render_recommended(ctx, &view, hud, cursor);
        } else {
            self.hide_recommended(ctx);
            self.render_grid(ctx, &view, hud, &entries, height, cursor);
        }
        if let (Some(root), Some(focus)) = (self.root, self.focus) {
            self.render_detail(ctx, &view, hud, root, focus, cursor, notches);
        }
        self.render_bottom(ctx, &view, hud);
        self.render_tip(ctx, &view, hud, hovered, cursor);
        // Message: fades in over 117 ms, shows for 2.5 s, disappears at once.
        let toast = self
            .toast
            .as_ref()
            .filter(|(_, at, _)| at.elapsed() < Duration::from_millis(2500))
            .cloned();
        self.visible(ctx, "toast", toast.is_some());
        self.visible(ctx, "buy_hint", toast.is_none());
        if let Some((t, at, tone)) = toast {
            let fade = hud_motion::easing((at.elapsed().as_secs_f32() / 0.117).min(1.), true);
            let alpha = (242. * fade).round() as u8;
            self.props(ctx, "toast", format!("color: #~1e1e1d{alpha:02x};"));
            self.props(ctx, "toast.bar", format!("color: #{tone};"));
            self.text(ctx, "toast.text", &t);
        }
        vec![Rect {
            x: WINDOW.0,
            y: WINDOW.1,
            w: WINDOW.2,
            h: WINDOW.3,
        }]
    }

    fn render_top(&mut self, ctx: &mut StableClient<'_>, view: &shop::View) {
        let steps = self.pending.len();
        let plural = if steps > 1 { "s" } else { "" };
        let (text, color, icon) = if !view.manual {
            (
                "Manual shopping off · the game auto-buys".to_owned(),
                "ff642eff",
                "ef_unavailable",
            )
        } else if view.in_base && steps > 0 && self.paused {
            (
                format!("Paused · {steps} purchase{plural} apply when the match resumes"),
                "fdee00ff",
                "ef_clock",
            )
        } else if view.in_base && steps > 0 {
            (
                format!("In base · buying {steps} step{plural}…"),
                "7fd36bff",
                "ef_recall",
            )
        } else if view.in_base {
            (
                "In base · purchases happen now".to_owned(),
                "7fd36bff",
                "ef_recall",
            )
        } else {
            (
                "Away · purchases wait for your next base visit".to_owned(),
                "cbc9c7ff",
                "ef_clock",
            )
        };
        self.text(ctx, "pill_text", &text);
        self.props(ctx, "pill_text", format!("color: #{color};"));
        self.props(
            ctx,
            "pill_icon",
            format!("color: #{color}; source: \"asset/lt_direct_control_probe/ui/{icon}\";"),
        );
        // Gold left after the projected purchases.
        let gold = if steps > 0 {
            format!("{} → {}", number(self.real.gold), number(view.live.gold))
        } else {
            number(view.live.gold)
        };
        self.props(
            ctx,
            "gold",
            format!("width: 186px; size: {};", if steps > 0 { 22 } else { 28 }),
        );
        self.text(ctx, "gold", &gold);
        // Endfield checkbox: off = light box; on = dark box, yellow rail, white check.
        let on = crate::settings::option("shop_pause") == 1.;
        self.props(
            ctx,
            "pause.box",
            format!("color: #{};", if on { "~3a3837ff" } else { "eeececff" }),
        );
        self.visible(ctx, "pause.rail", on);
        self.visible(ctx, "pause.check", on);
        let on = crate::settings::option("shop_vanilla_order") == 1.;
        self.props(
            ctx,
            "vanilla.box",
            format!("color: #{};", if on { "~3a3837ff" } else { "eeececff" }),
        );
        self.visible(ctx, "vanilla.rail", on);
        self.visible(ctx, "vanilla.check", on);
    }

    fn render_rail(&mut self, ctx: &mut StableClient<'_>, view: &shop::View) {
        // Recommended / All items indicator slides over 167 ms (ease-move).
        let at = self
            .motion
            .value("tab", if self.recommended { 0. } else { 1. }, 0.167);
        self.props(ctx, "tab_indicator", format!("y: {:.1}px;", 84. + at * 56.));
        for (node, on) in [
            ("tab_rec", self.recommended),
            ("tab_all", !self.recommended),
        ] {
            let color = if on { "393939ff" } else { "cbc9c7ff" };
            self.props(ctx, &format!("{node}.label"), format!("color: #{color};"));
            self.props(ctx, &format!("{node}.icon"), format!("color: #{color};"));
        }
        // Stat filters present in this item set. Counts show how many items
        // would remain if that filter were added to the ticked ones.
        let matching = |extra: Option<usize>, active: &[usize]| {
            view.cat
                .iter()
                .filter(|item| active.iter().chain(extra.iter()).all(|f| has(item, *f)))
                .count()
        };
        self.filters = std::iter::once(None)
            .chain(
                (0..STAT_FILTERS.len())
                    .filter(|f| view.cat.iter().any(|item| has(item, *f)))
                    .map(Some),
            )
            .take(FILTERS)
            .collect();
        let show = !self.recommended;
        self.visible(ctx, "filter_head", show);
        let active = self.active.clone();
        for i in 0..FILTERS {
            let node = format!("filter{i}");
            match self.filters.get(i).copied().filter(|_| show) {
                Some(f) => {
                    self.visible(ctx, &node, true);
                    let (label, count, on) = match f {
                        None => ("All", matching(None, &[]), active.is_empty()),
                        Some(f) => (
                            STAT_FILTERS[f].0,
                            matching(Some(f), &active),
                            active.contains(&f),
                        ),
                    };
                    self.text(ctx, &format!("{node}.label"), label);
                    self.text(ctx, &format!("{node}.count"), &count.to_string());
                    for j in 0..STAT_ICONS.len() {
                        let shown = f == Some(j);
                        let alpha = if count == 0 { "66" } else { "ff" };
                        self.props(
                            ctx,
                            &format!("ficon{i}_{j}"),
                            format!("visible: {shown}; color: #ffffff{alpha};"),
                        );
                    }
                    // The yellow bar fades with the 83 ms row wipe; the row tint follows.
                    let t = self.motion.tonal(&format!("f{i}"), f32::from(on), 0.083);
                    let back = hud_motion::color(
                        crate::ui_theme::tone(0x2927_2600),
                        crate::ui_theme::tone(0x2927_26ff),
                        t,
                    );
                    self.props(ctx, &node, format!("btn: {{ back_color: #{back}; }}"));
                    let bar = hud_motion::color(0xfdee_0000, 0xfdee_00ff, t);
                    self.props(ctx, &format!("{node}.bar"), format!("color: #{bar};"));
                    self.props(
                        ctx,
                        &format!("{node}.label"),
                        format!(
                            "x: {}px; width: {}px; color: #{};",
                            if f.is_some() { 41 } else { 15 },
                            if f.is_some() { 89 } else { 120 },
                            if on {
                                "ffffffff"
                            } else if count == 0 {
                                "6f6d6bff"
                            } else {
                                "cbc9c7ff"
                            }
                        ),
                    );
                }
                None => {
                    self.visible(ctx, &node, false);
                    for j in 0..STAT_ICONS.len() {
                        self.props(ctx, &format!("ficon{i}_{j}"), "visible: false;".into());
                    }
                }
            }
        }
    }

    /// Tile badge: a queue position, or nothing.
    fn badge(view: &shop::View, item: usize) -> String {
        view.queue
            .iter()
            .position(|q| *q == item)
            .map_or(String::new(), |n| (n + 1).to_string())
    }

    fn render_grid(
        &mut self,
        ctx: &mut StableClient<'_>,
        view: &shop::View,
        hud: &HudUi,
        entries: &[(Entry, i32, f32)],
        height: f32,
        cursor: Option<(f32, f32)>,
    ) {
        let scroll = self.scroll;
        let visible = entries.iter().filter(|(e, _, top)| {
            let h = if matches!(e, Entry::Head(_)) {
                HEAD_H
            } else {
                TILE_H
            };
            let y = VIEW_TOP + top - scroll;
            y + h > VIEW_TOP && y < VIEW_BOTTOM
        });
        let mut tiles = vec![None; TILES];
        let (mut t, mut h) = (0, 0);
        for (entry, x, top) in visible {
            let y = VIEW_TOP + top - self.scroll;
            match *entry {
                Entry::Head(tier) if h < HEADS => {
                    let node = format!("head{h}");
                    self.props(ctx, &node, format!("visible: true; y: {y:.1}px;"));
                    let name = match tier {
                        0 => "LEVEL 1 · BASE PARTS".to_owned(),
                        t => format!("LEVEL {}", t + 1),
                    };
                    self.text(ctx, &format!("{node}.text"), &name);
                    h += 1;
                }
                Entry::Tile(item) if t < TILES => {
                    let node = format!("tile{t}");
                    let key = view.cat[item].key.clone();
                    let hover = Self::hovered(ctx, &node, cursor);
                    let back = self.tone(&format!("tile{item}"), hover);
                    let selected = self.root == Some(item);
                    self.props(ctx, &node, format!("visible: true; x: {x}px; y: {y:.1}px; btn: {{ back_color: #{back:08x}; color: #{}; stroke: {}; }}", if selected { SELECTED } else if hover { "ffffffff" } else { "00000000" }, if selected { 2 } else { u32::from(hover) }));
                    self.icon(ctx, &format!("{node}.icon"), hud, Some(&key));
                    let badge = Self::badge(view, item);
                    // Item list: owned items look normal; items you cannot
                    // buy right now (gold or a full inventory) are dimmed.
                    let dim = !view.live.owned.contains(&item) && !self.buyable(view, item);
                    self.tint(ctx, &format!("{node}.icon"), dim);
                    // League price: gold still needed; red when not affordable now.
                    self.price_label(ctx, &format!("{node}.price"), view, item);
                    // Only a queue position is badged; owned items look normal.
                    self.visible(ctx, &format!("{node}.badge"), !badge.is_empty());
                    self.visible(ctx, &format!("{node}.badge_text"), !badge.is_empty());
                    self.visible(ctx, &format!("{node}.badge_check"), false);
                    self.text(ctx, &format!("{node}.badge_text"), &badge);
                    tiles[t] = Some(item);
                    t += 1;
                }
                _ => {}
            }
        }
        for i in h..HEADS {
            self.visible(ctx, &format!("head{i}"), false);
        }
        for i in t..TILES {
            self.visible(ctx, &format!("tile{i}"), false);
        }
        self.tiles = tiles;
        let max = max_scroll(height);
        let view_h = VIEW_BOTTOM - VIEW_TOP;
        let thumb = if max > 0. {
            (view_h * view_h / (height + 16.)).max(40.)
        } else {
            view_h
        };
        let y = VIEW_TOP
            + if max > 0. {
                (view_h - thumb) * self.scroll / max
            } else {
                0.
            };
        self.props(
            ctx,
            "scroll_thumb",
            format!("visible: {}; y: {y:.1}px; height: {thumb:.1}px;", max > 0.),
        );
        self.visible(ctx, "scroll_track", max > 0.);
    }
    fn hide_grid(&mut self, ctx: &mut StableClient<'_>) {
        for i in 0..HEADS {
            self.visible(ctx, &format!("head{i}"), false);
        }
        for i in 0..TILES {
            self.visible(ctx, &format!("tile{i}"), false);
        }
        self.tiles = vec![None; TILES];
        self.visible(ctx, "scroll_thumb", false);
        self.visible(ctx, "scroll_track", false);
    }
    fn tone(&mut self, key: &str, hover: bool) -> u32 {
        // Hover tint over 100 ms (tonal).
        let h = self
            .motion
            .tonal(&format!("h_{key}"), f32::from(hover), 0.1);
        u32::from_str_radix(
            &hud_motion::color(0x0000_0000, crate::ui_theme::tone(0x3a38_37ff), h),
            16,
        )
        .unwrap_or(0)
    }

    fn render_recommended(
        &mut self,
        ctx: &mut StableClient<'_>,
        view: &shop::View,
        hud: &HudUi,
        cursor: Option<(f32, f32)>,
    ) {
        self.visible(ctx, "rec_head", true);
        self.visible(ctx, "rec_note", true);
        self.text(ctx, "rec_note", "The game's build list for your champion. Click a row for details; Buy or right-click to buy it, or queue the whole build in order.");
        // Light Endfield button: hover darkens over 100 ms, as in the lab.
        let hover = Self::hovered(ctx, "queue_build", cursor);
        let t = self.motion.tonal("queue_build", f32::from(hover), 0.1);
        let back = hud_motion::color(0xeeec_ecff, 0xb8b6_b5ff, t);
        self.props(
            ctx,
            "queue_build",
            format!("visible: true; btn: {{ back_color: #{back}; }} text: {{ color: #393939ff; }}"),
        );
        self.recs = view.live.build.iter().copied().take(RECS).collect();
        for i in 0..RECS {
            let node = format!("rec{i}");
            let Some(item) = self.recs.get(i).copied() else {
                self.visible(ctx, &node, false);
                continue;
            };
            let key = view.cat[item].key.clone();
            let name = self.name(ctx, hud, &key);
            self.visible(ctx, &node, true);
            self.icon(ctx, &format!("{node}.icon"), hud, Some(&key));
            self.text(ctx, &format!("{node}.name"), &format!("{}. {name}", i + 1));
            // The build wants one copy: owned means complete.
            let plan = if view.live.owned.contains(&item) {
                None
            } else {
                match self.purchase_offer(view, item) {
                    shop::Offer::Plan(steps) => Some(steps),
                    _ => None,
                }
            };
            let (sub, state) = match &plan {
                None if view.live.owned.contains(&item) => {
                    ("Complete".to_owned(), "Owned".to_owned())
                }
                None => ("No free slot".to_owned(), String::new()),
                Some(steps) => {
                    let total: usize = steps.iter().map(|s| view.cat[s.item()].price).sum();
                    let next = view.cat[steps[0].item()].key.clone();
                    let next_name = self.name(ctx, hud, &next);
                    (
                        format!(
                            "Need <#{}>{}<> · {} step{}",
                            if view.live.gold >= total {
                                "fdee00ff"
                            } else {
                                "ff642eff"
                            },
                            number(total),
                            steps.len(),
                            if steps.len() > 1 { "s" } else { "" },
                        ),
                        format!(
                            "Next: {next_name} · {}",
                            number(view.cat[steps[0].item()].price)
                        ),
                    )
                }
            };
            self.text(ctx, &format!("{node}.sub"), &sub);
            self.text(ctx, &format!("{node}.state"), &state);
            let hover = Self::hovered(ctx, &node, cursor);
            let selected = self.root == Some(item);
            self.props(
                ctx,
                &node,
                format!(
                    "btn: {{ color: #{}; stroke: {}; }}",
                    if selected {
                        SELECTED
                    } else if hover {
                        "ffffffff"
                    } else {
                        "00000000"
                    },
                    if selected { 2 } else { u32::from(hover) }
                ),
            );
        }
    }
    fn hide_recommended(&mut self, ctx: &mut StableClient<'_>) {
        self.visible(ctx, "queue_build", false);
        self.visible(ctx, "rec_head", false);
        self.visible(ctx, "rec_note", false);
        for i in 0..RECS {
            self.visible(ctx, &format!("rec{i}"), false);
        }
        self.recs.clear();
    }

    /// Explicit item choice first; focused components can use its prefix.
    fn chosen_path(&self, item: usize) -> Option<Vec<usize>> {
        self.choices.get(&item).cloned().or_else(|| {
            let path = self.choices.get(&self.root?)?;
            let at = path.iter().position(|i| *i == item)?;
            Some(path[..=at].to_vec())
        })
    }

    fn purchase_offer(&self, view: &shop::View, item: usize) -> shop::Offer {
        match self.chosen_path(item) {
            Some(path) => offer_path(view, item, Some(&path)),
            None => offer(view, item),
        }
    }

    /// Enough gold for the complete remaining purchase and a usable slot.
    fn buyable(&self, view: &shop::View, item: usize) -> bool {
        self.remaining(view, item)
            .is_some_and(|t| view.live.gold >= t)
    }
    /// Dim an item icon (35%) or show it normally.
    fn tint(&mut self, ctx: &mut StableClient<'_>, node: &str, dim: bool) {
        self.props(
            ctx,
            node,
            format!("color: #{};", if dim { "ffffff59" } else { "ffffffff" }),
        );
    }
    /// Gold one more Buy of `item` still needs from what the champion owns
    /// (League's shop price), after the orders already queued. None when
    /// unreachable or blocked. Existing copies do not block another Buy.
    fn remaining(&self, view: &shop::View, item: usize) -> Option<usize> {
        match self.purchase_offer(view, item) {
            shop::Offer::Plan(steps) => Some(steps.iter().map(|s| view.cat[s.item()].price).sum()),
            _ => None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_detail(
        &mut self,
        ctx: &mut StableClient<'_>,
        view: &shop::View,
        hud: &HudUi,
        root: usize,
        focus: usize,
        cursor: Option<(f32, f32)>,
        notches: i32,
    ) {
        // Builds into: what the focused item can become.
        self.into = view.cat[focus].next.iter().copied().take(INTO).collect();
        self.visible(ctx, "into_none", self.into.is_empty());
        for i in 0..INTO {
            let node = format!("into{i}");
            let Some(next) = self.into.get(i).copied() else {
                self.visible(ctx, &node, false);
                continue;
            };
            self.visible(ctx, &node, true);
            let k = view.cat[next].key.clone();
            self.icon(ctx, &format!("{node}.icon"), hud, Some(&k));
            let have = view.live.owned.contains(&next);
            let dim = have || !self.buyable(view, next);
            self.tint(ctx, &format!("{node}.icon"), dim);
            self.visible(ctx, &format!("{node}.check"), have);
            self.price_label(ctx, &format!("{node}.price"), view, next);
            let hover = Self::hovered(ctx, &node, cursor);
            let back = self.tone(&format!("i{i}"), hover);
            self.props(ctx, &node, format!("btn: {{ back_color: #{back:08x}; }}"));
        }
        // Full structural chains, including ancestors of owned items. Separate
        // rows are alternatives (OR), not components to combine. Purchases
        // use the chosen chain, or automatic cheapest planning by default.
        let chosen = self.choices.get(&root).cloned();
        let auto = self.recipes.prepare(view, root, chosen.as_deref());
        self.recipe = self.recipes.cells();
        let total = self.recipes.total();
        let branched = self.recipes.branched();
        self.text(
            ctx,
            "recipe_head",
            if branched { "UPGRADE PATHS" } else { "RECIPE" },
        );
        let first = self.recipes.page * RECIPE_ROWS;
        self.text(
            ctx,
            "recipe_pages",
            &format!(
                "{}–{}/{}",
                first + 1,
                first + self.recipes.paths.len(),
                total
            ),
        );
        self.visible(ctx, "recipe_pages", total > RECIPE_ROWS);
        self.recipe_control(
            ctx,
            "recipe_prev",
            "<",
            total > RECIPE_ROWS,
            self.recipes.page > 0,
            cursor,
        );
        self.recipe_control(
            ctx,
            "recipe_next",
            ">",
            total > RECIPE_ROWS,
            first.saturating_add(RECIPE_ROWS) < total,
            cursor,
        );
        self.recipe_choice(
            ctx,
            "recipe_auto",
            "Automatic (cheapest)",
            branched,
            chosen.is_none(),
            cursor,
        );
        let max_len = self.recipes.paths.iter().map(Vec::len).max().unwrap_or(0);
        let detailed = self.recipes.detailed();
        let columns = self.recipes.columns();
        let column = self.recipes.column * columns;
        let steps_y = if branched { RECIPE_Y + 8. } else { RECIPE_Y };
        self.props(
            ctx,
            "recipe_steps",
            format!("x: 1100px; y: {steps_y:.0}px; width: 184px; height: 24px;"),
        );
        for node in ["recipe_left", "recipe_right"] {
            self.props(ctx, node, format!("y: {steps_y:.0}px; height: 24px;"));
        }
        self.text(
            ctx,
            "recipe_steps",
            &format!(
                "Steps {}–{} / {}",
                column + 1,
                (column + columns).min(max_len),
                max_len
            ),
        );
        self.visible(ctx, "recipe_steps", max_len > columns);
        self.recipe_control(
            ctx,
            "recipe_left",
            "<",
            max_len > columns,
            self.recipes.column > 0,
            cursor,
        );
        self.recipe_control(
            ctx,
            "recipe_right",
            ">",
            max_len > columns,
            self.recipes.column < self.recipes.max_column(),
            cursor,
        );
        for row in 0..RECIPE_ROWS {
            let path = self.recipes.paths.get(row);
            let automatic = path.is_some_and(|p| !auto.is_empty() && p.ends_with(&auto));
            let browsed = path.is_some_and(|p| {
                !self.recipes.browsed.is_empty() && p.ends_with(&self.recipes.browsed)
            });
            let mut label = format!(
                "{}Path {}",
                if row == 0 { "" } else { "OR · " },
                first + row + 1
            );
            if let Some(path) = path {
                let (cost, _) = recipe_cost(view, path);
                label.push_str(&format!(" · Need {}", number(cost)));
            }
            if automatic {
                label.push_str(" · Auto-buy");
            }
            if browsed && max_len <= columns {
                label.push_str(" · Browsed");
            }
            if column >= path.map_or(0, Vec::len) {
                label.push_str(" · Earlier steps");
            }
            let selected = path.is_some_and(|p| chosen.as_ref() == Some(p));
            let show = branched && path.is_some();
            self.visible(
                ctx,
                &format!("recipe_path{row}"),
                detailed && path.is_some(),
            );
            self.text(ctx, &format!("recipe_path{row}"), &label);
            self.props(
                ctx,
                &format!("recipe_path{row}"),
                format!("color: #{};", if automatic { GOOD } else { INFO }),
            );
            self.props(
                ctx,
                &format!("recipe_path{row}"),
                format!(
                    "y: {:.0}px; width: {}px;",
                    self.recipes.paths_y() + row as f32 * RECIPE_ROW_H,
                    if !branched && max_len > columns {
                        230
                    } else {
                        476
                    }
                ),
            );
            let node = format!("recipe_use{row}");
            self.recipe_choice(
                ctx,
                &node,
                if selected {
                    "Selected path"
                } else {
                    "Use this path"
                },
                show,
                selected,
                cursor,
            );
            let y = self.recipes.item_y(row * RECIPE_COLS) + 8.;
            self.props(
                ctx,
                &node,
                format!("x: 1178px; y: {y:.0}px; width: 160px; height: 40px;"),
            );
        }
        // Give alternate paths room while retaining the existing detail area.
        let buy_y = self.recipes.buy_y();
        let shift = buy_y - BUY_Y;
        for (node, y) in [
            ("buy", buy_y),
            ("buy_hint", buy_y + 56.),
            ("toast", buy_y + 54.),
            ("d_rule", DETAIL_Y + shift - 12.),
            ("d_frame", DETAIL_Y + shift),
            ("d_icon", DETAIL_Y + shift + 1.),
            ("d_name", DETAIL_Y + shift - 2.),
            ("d_cost", DETAIL_Y + shift + 30.),
            ("d_tags", DETAIL_Y + shift + 54.),
        ] {
            self.props(ctx, node, format!("y: {y:.0}px;"));
        }
        for i in 0..RECIPE {
            let node = format!("recipe{i}");
            let col = i % RECIPE_COLS;
            let y = self.recipes.item_y(i);
            self.props(ctx, &node, format!("y: {y:.0}px;"));
            if col + 1 < RECIPE_COLS {
                self.props(
                    ctx,
                    &format!("recipe_link{i}"),
                    format!("y: {:.0}px;", y + 20.),
                );
                let linked = self.recipe.get(i).copied().flatten().is_some()
                    && self.recipe.get(i + 1).copied().flatten().is_some();
                self.visible(ctx, &format!("recipe_link{i}"), linked);
                if linked {
                    let from = self.recipe[i].unwrap();
                    let to = self.recipe[i + 1].unwrap();
                    let automatic = branched && auto.windows(2).any(|edge| edge == [from, to]);
                    self.props(
                        ctx,
                        &format!("recipe_link{i}"),
                        format!("color: #{};", if automatic { GOOD } else { "~4b4a49ff" }),
                    );
                }
            }
            let Some(step) = self.recipe.get(i).copied().flatten() else {
                self.visible(ctx, &node, false);
                continue;
            };
            self.visible(ctx, &node, true);
            let k = view.cat[step].key.clone();
            self.icon(ctx, &format!("{node}.icon"), hud, Some(&k));
            let have = view.live.owned.contains(&step);
            // Each part shows the remaining cost along THIS row's prefix.
            let path = &self.recipes.paths[i / RECIPE_COLS];
            let at = column + col;
            let (cost, affordable) = recipe_cost(view, &path[..=at]);
            let shown = if have { view.cat[step].price } else { cost };
            self.text(ctx, &format!("{node}.price"), &number(shown));
            let color = if have {
                "989694ff"
            } else if affordable {
                INFO
            } else {
                BAD
            };
            self.props(ctx, &format!("{node}.price"), format!("color: #{color};"));
            let frame = if step == focus { SELECTED } else { "~4b4a49ff" };
            self.props(ctx, &format!("{node}.frame"), format!("color: #{frame};"));
            // Owned: dimmed with a tick. Not buyable now: dimmed, no tick.
            let dim = have || !affordable;
            self.tint(ctx, &format!("{node}.icon"), dim);
            self.visible(ctx, &format!("{node}.check"), have);
            let hover = Self::hovered(ctx, &node, cursor);
            let back = self.tone(&format!("r{i}"), hover);
            self.props(ctx, &node, format!("btn: {{ back_color: #{back:08x}; }}"));
        }
        // The focused item's name is already shown in the detail heading.
        let key = view.cat[focus].key.clone();
        let name = self.name(ctx, hud, &key);
        // Orders of this item still waiting (duplicates of parts are allowed).
        let queued = view.queue.iter().filter(|q| **q == focus).count();
        let offer = self.purchase_offer(view, focus);
        let buying = matches!(offer, shop::Offer::Owned)
            && !self.real.owned.contains(&focus)
            && self.pending.contains(&focus);
        let (label, enabled, back, hint) = if !view.manual {
            (
                "Manual shopping off".to_owned(),
                false,
                "~3a3837ff",
                "The game is auto-buying for this champion.".to_owned(),
            )
        } else if buying {
            (
                "Purchasing".to_owned(),
                false,
                "~3a3837ff",
                if self.paused {
                    "Applies when the match resumes.".to_owned()
                } else {
                    "Arrives within a moment.".to_owned()
                },
            )
        } else {
            match &offer {
                shop::Offer::Owned => ("Unavailable".to_owned(), false, "~3a3837ff", String::new()),
                shop::Offer::Blocked => (
                    format!("No free slot · {}/{}", view.live.owned.len(), view.capacity),
                    false,
                    "~3a3837ff",
                    if chosen.is_some() {
                        "Chosen path needs a free slot; other components stay in your bag.".into()
                    } else {
                        String::new()
                    },
                ),
                shop::Offer::Plan(steps) => {
                    let total: usize = steps.iter().map(|s| view.cat[s.item()].price).sum();
                    let first = view.cat[steps[0].item()].price;
                    let again = if queued > 0 || view.live.owned.contains(&focus) {
                        " another"
                    } else {
                        ""
                    };
                    let label = if view.in_base && view.live.gold >= first {
                        format!("Purchase · {}", number(total))
                    } else {
                        format!("Queue{again} · {}", number(total))
                    };
                    let back = if view.in_base && view.live.gold >= first {
                        "fdee00ff"
                    } else {
                        "eeececff"
                    };
                    let hint = if branched && queued > 0 {
                        "Queued purchases keep their path. Remove and requeue to change it."
                    } else if chosen.is_some() && view.in_base {
                        "Chosen path · purchases follow it through completion."
                    } else if chosen.is_some() {
                        "Chosen path · completes at your next base visit."
                    } else if view.in_base {
                        "Right-click any item to buy it. Purchases are final."
                    } else {
                        "Completes automatically at your next base visit."
                    };
                    (label, true, back, hint.to_owned())
                }
            }
        };
        let size = if crate::hud_style::width(&label, 19.) <= 440. {
            19
        } else {
            16
        };
        let hover = enabled && Self::hovered(ctx, "buy", cursor);
        let ink = if enabled { "1c1a18ff" } else { "989694ff" };
        let color = crate::ui_theme::color(back).unwrap_or(0);
        let tinted = u32::from_str_radix(
            &hud_motion::color(
                color,
                0xcfc0_00ff,
                self.motion.tonal("buy", f32::from(hover) * 0.25, 0.1),
            ),
            16,
        )
        .unwrap_or(color);
        self.props(ctx, "buy", format!("ignore_event: {}; btn: {{ back_color: #{tinted:08x}; color: #00000000; }} text: {{ text: {}; size: {size}; color: #{ink}; }}", !enabled, json(&label)));
        self.text(ctx, "buy_hint", &hint);
        // Detail: the focused item's price, tags, stats and effect.
        self.icon(ctx, "d_icon", hud, Some(&key));
        self.text(ctx, "d_name", &name);
        let cost = match self.remaining(view, focus) {
            Some(t) => format!(
                "Need <#{}>{}<>    Step price {}",
                if view.live.gold >= t {
                    "fdee00ff"
                } else {
                    "ff642eff"
                },
                number(t),
                number(view.cat[focus].price)
            ),
            None if view.live.owned.contains(&focus) => {
                format!("Owned    Step price {}", number(view.cat[focus].price))
            }
            None => format!("Step price {}", number(view.cat[focus].price)),
        };
        self.text(ctx, "d_cost", &cost);
        let tags = (0..STAT_FILTERS.len())
            .filter(|f| has(&view.cat[focus], *f))
            .take(3)
            .map(|f| {
                format!(
                    "{} {}",
                    STAT_ICONS[f].inline(),
                    STAT_FILTERS[f].0.to_uppercase()
                )
            })
            .chain(std::iter::once(format!(
                "LEVEL {}",
                view.cat[focus].tier + 1
            )))
            .collect::<Vec<_>>()
            .join("  ·  ");
        self.text(ctx, "d_tags", &tags);
        let body_y = BODY_Y + shift;
        let body = self.bodies.get(&key).cloned().unwrap_or_default();
        let height = (BODY_BOTTOM - body_y).max(LINE_H);
        self.description.prepare(&key, &body, height);
        self.props(
            ctx,
            "d_view",
            format!("y: {body_y:.0}px; height: {height:.0}px;"),
        );
        if Self::hovered(ctx, "d_view", cursor) {
            self.description.scroll(notches);
        }
        self.props(
            ctx,
            "d_body",
            format!("y: {body_y:.0}px; height: {height:.0}px;"),
        );
        self.text(ctx, "d_body", &self.description.text());
        let max = self.description.max();
        self.visible(ctx, "d_scroll_track", max > 0);
        self.visible(ctx, "d_scroll_thumb", max > 0);
        if max > 0 {
            let thumb = (height * self.description.page as f32
                / self.description.lines.len() as f32)
                .max(24.)
                .min(height);
            let y = body_y + (height - thumb) * self.description.first as f32 / max as f32;
            self.props(
                ctx,
                "d_scroll_track",
                format!("y: {body_y:.0}px; height: {height:.0}px;"),
            );
            self.props(
                ctx,
                "d_scroll_thumb",
                format!("y: {y:.1}px; height: {thumb:.1}px;"),
            );
        }
    }

    fn recipe_choice(
        &mut self,
        ctx: &mut StableClient<'_>,
        node: &str,
        label: &str,
        visible: bool,
        selected: bool,
        cursor: Option<(f32, f32)>,
    ) {
        self.visible(ctx, node, visible);
        let hover = visible && Self::hovered(ctx, node, cursor);
        let t = self
            .motion
            .tonal(&format!("choice_{node}"), f32::from(hover), 0.1);
        let (base, over, ink, border) = if selected {
            (0xfdee_00ff, 0xcfc0_00ff, "1c1a18ff", "fdee00ff")
        } else {
            (
                crate::ui_theme::tone(0x3a38_37ff),
                crate::ui_theme::tone(0x2927_26ff),
                "eeececff",
                "6f6d6bff",
            )
        };
        let back = hud_motion::color(base, over, t);
        self.props(ctx, node, format!("ignore_event: false; btn: {{ back_color: #{back}; color: #{border}; stroke: 1; }} text: {{ text: {}; size: 16; color: #{ink}; }}", json(label)));
    }

    fn recipe_control(
        &mut self,
        ctx: &mut StableClient<'_>,
        node: &str,
        label: &str,
        visible: bool,
        enabled: bool,
        cursor: Option<(f32, f32)>,
    ) {
        self.visible(ctx, node, visible);
        let hover = visible && enabled && Self::hovered(ctx, node, cursor);
        let back = self.tone(node, hover);
        let color = if enabled { INFO } else { "6f6d6bff" };
        self.props(ctx, node, format!("ignore_event: {}; btn: {{ back_color: #{back:08x}; }} text: {{ text: {}; color: #{color}; }}", !enabled, json(label)));
    }

    /// Price label in League's sense: what you still need (red when you
    /// cannot afford it now), or a muted step price when owned.
    fn price_label(
        &mut self,
        ctx: &mut StableClient<'_>,
        node: &str,
        view: &shop::View,
        item: usize,
    ) {
        let (text, color) = match self.remaining(view, item) {
            // An owned finished item (one copy each): its price, muted. Owned
            // base parts show what another copy costs.
            None if view.live.owned.contains(&item) => (number(view.cat[item].price), "989694ff"),
            Some(t) => (
                number(t),
                if view.live.gold >= t {
                    "eeececff"
                } else {
                    "ff642eff"
                },
            ),
            None => (number(view.cat[item].price), "6f6d6bff"),
        };
        self.text(ctx, node, &text);
        self.props(ctx, node, format!("color: #{color};"));
    }

    /// Wrap to `width` and cut to `lines` with an ellipsis.
    fn clip(body: &str, width: f32, lines: usize) -> (String, usize) {
        let (wrapped, n) = crate::hud_style::wrap(body, BODY_SIZE, width);
        if n <= lines {
            return (wrapped, n);
        }
        let mut cut: Vec<&str> = wrapped.lines().take(lines).collect();
        let last = format!("{}…", cut.pop().unwrap_or(""));
        cut.push(&last);
        (cut.join("\n"), lines)
    }

    /// Hover tooltip next to the cursor while it stays over an item
    /// (enters over 117 ms, disappears in one frame).
    fn render_tip(
        &mut self,
        ctx: &mut StableClient<'_>,
        view: &shop::View,
        hud: &HudUi,
        item: Option<usize>,
        cursor: Option<(f32, f32)>,
    ) {
        let (Some(item), Some((cx, cy))) = (item, cursor) else {
            self.tip = None;
            self.visible(ctx, "tip", false);
            return;
        };
        if self.tip.is_none_or(|(old, _)| old != item) {
            self.tip = Some((item, Instant::now()));
        }
        let key = view.cat[item].key.clone();
        let name = self.name(ctx, hud, &key);
        let body = self.bodies.get(&key).cloned().unwrap_or_default();
        let (text, lines) = Self::clip(&body, TIP_W as f32 - 32., 14);
        let height = if body.is_empty() {
            80.
        } else {
            94. + lines as f32 * LINE_H
        };
        // Below-right of the pointer, flipped to stay on screen.
        let mut x = cx + 18.;
        let mut y = cy + 22.;
        if x + TIP_W as f32 > 1910. {
            x = cx - 18. - TIP_W as f32;
        }
        if y + height > 1070. {
            y = (cy - 12. - height).max(10.);
        }
        let fade = self.tip.map_or(1., |(_, at)| {
            hud_motion::easing((at.elapsed().as_secs_f32() / 0.117).min(1.), true)
        });
        let alpha = (217. * fade).round() as u8;
        self.props(
            ctx,
            "tip",
            format!(
                "visible: true; x: {:.0}px; y: {:.0}px; height: {height:.0}px; color: #~1e1e1d{alpha:02x};",
                x - WINDOW.0,
                y - WINDOW.1
            ),
        );
        self.icon(ctx, "tip.art", hud, Some(&key));
        self.text(ctx, "tip.title", &name);
        let price = match self.remaining(view, item) {
            Some(t) if !view.live.owned.contains(&item) => format!(
                "<#{}>{}<>",
                if view.live.gold >= t {
                    "fdee00ff"
                } else {
                    "ff642eff"
                },
                number(t)
            ),
            _ if view.live.owned.contains(&item) => "Owned".to_owned(),
            _ => number(view.cat[item].price),
        };
        self.text(ctx, "tip.price", &price);
        self.visible(ctx, "tip.rule", !body.is_empty());
        self.props(
            ctx,
            "tip.body",
            format!("height: {:.0}px;", lines as f32 * LINE_H + 4.),
        );
        self.text(ctx, "tip.body", &text);
    }

    fn render_bottom(&mut self, ctx: &mut StableClient<'_>, view: &shop::View, hud: &HudUi) {
        let capacity = view.capacity.clamp(view.live.owned.len(), SLOTS).max(1);
        self.text(
            ctx,
            "items_count",
            &format!(
                "{} / {}",
                view.live.owned.len(),
                view.capacity.max(view.live.owned.len())
            ),
        );
        for i in 0..SLOTS {
            let node = format!("slot{i}");
            self.visible(ctx, &node, i < capacity);
            let key = view.live.owned.get(i).map(|o| view.cat[*o].key.clone());
            self.icon(ctx, &format!("{node}.icon"), hud, key.as_deref());
            // A slot the projection changed but the game has not bought yet:
            // yellow outline and clock.
            let pending = view.live.owned.get(i) != self.real.owned.get(i)
                && view.live.owned.get(i).is_some();
            self.props(
                ctx,
                &node,
                format!(
                    "color: #{};",
                    if pending { "fdee00ff" } else { "~4b4a49ff" }
                ),
            );
            self.visible(ctx, &format!("{node}.pending"), pending);
        }
        let costs = shop::order_costs(&view.cat, &view.live, &view.orders);
        let total: usize = costs.iter().flatten().sum();
        self.text(
            ctx,
            "queue_total",
            &if view.queue.is_empty() {
                "—".into()
            } else {
                number(total)
            },
        );
        self.visible(ctx, "queue_empty", view.queue.is_empty());
        self.chips = view.queue.iter().copied().take(CHIPS).collect();
        for i in 0..CHIPS {
            let node = format!("chip{i}");
            let Some(item) = self.chips.get(i).copied() else {
                self.visible(ctx, &node, false);
                continue;
            };
            let key = view.cat[item].key.clone();
            let name = self.name(ctx, hud, &key);
            self.visible(ctx, &node, true);
            self.icon(ctx, &format!("{node}.icon"), hud, Some(&key));
            let (short, _) = crate::hud_style::wrap(&name, 14., 100.);
            self.text(
                ctx,
                &format!("{node}.name"),
                short.lines().next().unwrap_or(""),
            );
            // This order's remaining cost; 0 means it is being bought.
            let price = match costs.get(i).copied().flatten() {
                Some(0) => "buying".to_owned(),
                Some(n) => number(n),
                None => "no slot".to_owned(),
            };
            self.text(ctx, &format!("{node}.price"), &price);
        }
    }
}

fn number(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_stat_filter_has_an_icon() {
        assert_eq!(STAT_ICONS.len(), STAT_FILTERS.len());
    }

    #[test]
    fn description_scroll_reaches_the_end_clamps_and_resets_for_a_new_item() {
        let body = (0..20)
            .map(|i| format!("Line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut description = Description::default();
        description.prepare("trinity", &body, LINE_H * 7. + 5.);
        assert_eq!(
            description.text(),
            "Line 0\nLine 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6"
        );
        description.scroll(-1);
        assert_eq!(description.first, 3);
        description.scroll(i32::MIN);
        assert_eq!(description.first, 13);
        assert!(description.text().ends_with("Line 19"));
        assert!(!description.text().contains('…'));
        description.prepare("trinity", &body, LINE_H * 10.);
        assert_eq!(description.first, 10);
        description.prepare("other_item", &body, LINE_H * 7.);
        assert_eq!(description.first, 0);
        description.scroll(i32::MAX);
        assert_eq!(description.first, 0);
        description.prepare("other_item", "Short text", LINE_H * 7.);
        assert_eq!(description.max(), 0);
        description.scroll(-10);
        assert_eq!(description.text(), "Short text");
    }

    #[test]
    fn description_scroll_preserves_multiline_colors_icons_and_blank_lines() {
        let body = "Stats\n\n<#ff642eff>Spellblade: colorful effect\n<i#asset/base/ui/banpick/champion_stat_icon:hp_0> 彩色效果\nlast colored line<>\nNormal text";
        let mut description = Description::default();
        description.prepare("colored", body, LINE_H * 2.);
        assert_eq!(description.text(), "Stats\n");
        description.scroll(-1);
        assert_eq!(description.text(), "<#ff642eff><i#asset/base/ui/banpick/champion_stat_icon:hp_0> 彩色效果<>\n<#ff642eff>last colored line<>");
        description.scroll(-1);
        assert_eq!(
            description.text(),
            "<#ff642eff>last colored line<>\nNormal text"
        );
        let long = format!("<#ff642eff>{}<>", "物理傷害增加".repeat(50));
        description.prepare("wrapped", &long, LINE_H * 2.);
        description.scroll(i32::MIN);
        assert!(description.text().starts_with("<#ff642eff>"));
        assert!(description.text().ends_with("<>"));
        let recovered = description
            .lines
            .iter()
            .map(|s| crate::tooltips::plain(s))
            .collect::<String>();
        assert_eq!(recovered, crate::tooltips::plain(&long));
    }

    fn item(key: &str, tier: usize, price: usize, stats: &[&str]) -> shop::Item {
        shop::Item {
            key: key.into(),
            tier,
            price,
            category: String::new(),
            stats: stats.iter().map(|s| s.to_string()).collect(),
            next: Vec::new(),
        }
    }

    #[test]
    fn grid_groups_by_level_and_never_exceeds_the_pools() {
        let cat: Vec<_> = (0..265)
            .map(|i| {
                item(
                    &format!("i{i}"),
                    i % 5,
                    250 + i,
                    [
                        &["attack", "crit_chance"][..],
                        &["hp"],
                        &["magic_power", "hp"],
                    ][i % 3],
                )
            })
            .collect();
        let (entries, height) = layout(&cat, &[]);
        assert_eq!(
            entries
                .iter()
                .filter(|(e, _, _)| matches!(e, Entry::Head(_)))
                .count(),
            5
        );
        assert_eq!(
            entries
                .iter()
                .filter(|(e, _, _)| matches!(e, Entry::Tile(_)))
                .count(),
            265
        );
        for scroll in (0..=max_scroll(height) as usize).step_by(7) {
            let visible = entries.iter().filter(|(e, _, top)| {
                let h = if matches!(e, Entry::Head(_)) {
                    HEAD_H
                } else {
                    TILE_H
                };
                let y = VIEW_TOP + top - scroll as f32;
                y + h > VIEW_TOP && y < VIEW_BOTTOM
            });
            let (heads, tiles) = visible.fold((0, 0), |(h, t), (e, _, _)| match e {
                Entry::Head(_) => (h + 1, t),
                Entry::Tile(_) => (h, t + 1),
            });
            assert!(
                heads <= HEADS && tiles <= TILES,
                "scroll {scroll}: {heads} heads {tiles} tiles"
            );
        }
        // Ticked filters combine: Health alone keeps two thirds, Health + AP one third.
        let tiles = |active: &[usize]| {
            layout(&cat, active)
                .0
                .iter()
                .filter(|(e, _, _)| matches!(e, Entry::Tile(_)))
                .count()
        };
        let health = STAT_FILTERS.iter().position(|f| f.0 == "Health").unwrap();
        let ap = STAT_FILTERS
            .iter()
            .position(|f| f.0 == "Ability Power")
            .unwrap();
        let crit = STAT_FILTERS
            .iter()
            .position(|f| f.0 == "Critical Strike")
            .unwrap();
        assert_eq!(tiles(&[health]), 176);
        assert_eq!(tiles(&[health, ap]), 88);
        assert_eq!(tiles(&[crit]), 89);
        assert_eq!(tiles(&[crit, ap]), 0);
    }

    #[test]
    fn prices_are_what_you_still_need_from_what_you_own() {
        let prices = [250, 500, 750, 875, 1000];
        let cat: Vec<_> = prices
            .iter()
            .enumerate()
            .map(|(tier, p)| shop::Item {
                key: format!("l{}", tier + 1),
                tier,
                price: *p,
                category: String::new(),
                stats: Vec::new(),
                next: if tier < 4 { vec![tier + 1] } else { Vec::new() },
            })
            .collect();
        let view = |owned: Vec<usize>| shop::View {
            cat: Arc::new(cat.clone()),
            live: shop::Live {
                owned,
                build: vec![4, 4, 4, 4],
                gold: 900,
                capacity: 4,
            },
            queue: Vec::new(),
            orders: Vec::new(),
            manual: true,
            in_base: true,
            capacity: 4,
            planned: true,
        };
        assert_eq!(ShopUi::default().remaining(&view(vec![]), 4), Some(3375));
        assert_eq!(ShopUi::default().remaining(&view(vec![1]), 4), Some(2625));
        // Tree parts use the same price: their whole chain from what you own.
        assert_eq!(ShopUi::default().remaining(&view(vec![]), 1), Some(750));
        assert_eq!(ShopUi::default().remaining(&view(vec![]), 2), Some(1500));
        assert_eq!(ShopUi::default().remaining(&view(vec![0]), 2), Some(1250));
        // Nothing is blocked: another copy of an owned item costs its whole chain.
        assert_eq!(ShopUi::default().remaining(&view(vec![4]), 4), Some(3375));
    }

    #[test]
    fn export_window_template() {
        let t = template();
        assert!(!t.contains("\\n"));
        assert_eq!(t.matches('{').count(), t.matches('}').count());
        if let Ok(dir) = std::env::var("LT_HUD_EXPORT_DIR") {
            std::fs::write(std::path::Path::new(&dir).join("shop.ui"), &t).unwrap();
        }
        assert_eq!(number(1234567), "1,234,567");
        assert!(t.contains("line_height: 23;"));
    }

    fn recipe_view(cat: Vec<shop::Item>, owned: Vec<usize>) -> shop::View {
        shop::View {
            cat: Arc::new(cat),
            live: shop::Live {
                owned,
                build: Vec::new(),
                gold: 3000,
                capacity: 4,
            },
            queue: Vec::new(),
            orders: Vec::new(),
            manual: true,
            in_base: true,
            capacity: 4,
            planned: true,
        }
    }

    #[test]
    fn recipe_browsing_keeps_the_entered_branch_even_on_another_page() {
        let mut cat: Vec<_> = (0..5)
            .map(|i| {
                item(
                    &format!("i{i}"),
                    if i == 0 { 0 } else { i.min(2) },
                    250,
                    &[],
                )
            })
            .collect();
        cat[0].next = vec![1];
        cat[1].next = vec![2, 3, 4];
        for part in &mut cat[2..5] {
            part.next = vec![5];
        }
        cat[4].price = 800;
        cat.push(item("final", 3, 750, &[]));
        let mut view = recipe_view(cat, Vec::new());
        let mut browser = RecipeBrowser::default();
        browser.prepare(&view, 4, None);
        browser.follow(4, 5);
        let auto = browser.prepare(&view, 5, None);
        assert_eq!(auto, vec![0, 1, 2, 5]);
        assert_eq!(browser.auto, Some(0));
        assert_eq!(browser.page, 1);
        assert_eq!(browser.paths, vec![vec![0, 1, 4, 5]]);
        assert_eq!(browser.browsed, vec![0, 1, 4, 5]);
        // Purchasing the intermediate part changes the buyer, not the tree.
        view.live.owned = vec![4];
        browser.prepare(&view, 5, None);
        assert_eq!(browser.auto, Some(2));
        assert_eq!(browser.page, 1);
        assert_eq!(browser.paths, vec![vec![0, 1, 4, 5]]);
        // Full inventory with no matching component still shows every path.
        view.live.owned = vec![5; 4];
        browser.prepare(&view, 5, None);
        assert_eq!(browser.auto, None);
        assert_eq!(browser.total(), 3);
        assert_eq!(browser.paths, vec![vec![0, 1, 4, 5]]);
        // Previous page and automatic-route navigation use stable path indices.
        browser.page = 0;
        browser.prepare(&view, 5, None);
        assert_eq!(browser.paths, vec![vec![0, 1, 2, 5], vec![0, 1, 3, 5]]);
    }

    #[test]
    fn recipe_component_pages_include_the_base_and_the_end_of_long_chains() {
        let mut cat: Vec<_> = (0..9).map(|i| item(&format!("i{i}"), i, 1, &[])).collect();
        for (i, part) in cat.iter_mut().enumerate().take(8) {
            part.next = vec![i + 1];
        }
        let view = recipe_view(cat, vec![7]);
        let mut browser = RecipeBrowser::default();
        browser.prepare(&view, 8, None);
        assert_eq!(
            browser.cells(),
            vec![Some(0), Some(1), Some(2), Some(3), Some(4), Some(5)]
        );
        browser.column = 1;
        browser.prepare(&view, 8, None);
        assert_eq!(
            browser.cells(),
            vec![Some(6), Some(7), Some(8), None, None, None]
        );
        assert_eq!(browser.max_column(), 1);
        // New root/catalogue cannot retain an out-of-bounds page or column.
        browser.prepare(&view, 0, None);
        assert_eq!(browser.column, 0);
        assert_eq!(browser.page, 0);
        assert_eq!(browser.cells()[0], Some(0));

        // Alternate rows reserve one tile's space for the path button, but
        // keep six event slots per row and carry every component to page two.
        let mut cat: Vec<_> = (0..7).map(|i| item(&format!("b{i}"), i, 1, &[])).collect();
        cat[1].tier = 0;
        cat[0].next = vec![2];
        cat[1].next = vec![2];
        for (i, part) in cat.iter_mut().enumerate().take(6).skip(2) {
            part.next = vec![i + 1];
        }
        let view = recipe_view(cat, vec![]);
        browser.prepare(&view, 6, None);
        assert_eq!(browser.columns(), 5);
        assert_eq!(
            browser.cells(),
            vec![
                Some(0),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                None,
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                None
            ]
        );
        browser.column = 1;
        browser.prepare(&view, 6, None);
        assert_eq!(
            browser.cells(),
            vec![
                Some(6),
                None,
                None,
                None,
                None,
                None,
                Some(6),
                None,
                None,
                None,
                None,
                None
            ]
        );
        assert_eq!(browser.max_column(), 1);
    }

    fn price_branches() -> Vec<shop::Item> {
        let mut cat = vec![
            item("base_a", 0, 250, &[]),
            item("base_b", 0, 250, &[]),
            item("middle", 1, 650, &[]),
            item("final", 2, 750, &[]),
        ];
        cat[0].next = vec![2];
        cat[1].next = vec![2];
        cat[2].next = vec![3];
        cat
    }

    #[test]
    fn recipe_prices_are_cumulative_along_each_branch_and_credit_owned_parts() {
        let mut view = recipe_view(price_branches(), vec![]);
        let path = [0, 2, 3];
        let costs = |view: &shop::View| {
            (1..=path.len())
                .map(|n| recipe_cost(view, &path[..n]).0)
                .collect::<Vec<_>>()
        };
        assert_eq!(costs(&view), vec![250, 900, 1650]);
        view.live.owned = vec![0];
        assert_eq!(costs(&view), vec![250, 650, 1400]);
        view.live.owned = vec![2];
        assert_eq!(recipe_cost(&view, &path), (750, true));
        // A competing branch's cheaper cost must not leak into this row.
        let mut cat = price_branches();
        cat[1].price = 100;
        let view = recipe_view(cat, vec![]);
        assert_eq!(ShopUi::default().remaining(&view, 3), Some(1500));
        assert_eq!(recipe_cost(&view, &path).0, 1650);
        assert_eq!(recipe_cost(&view, &[1, 2, 3]).0, 1500);
        // Full inventory blocks purchase, but the displayed cost stays cumulative.
        let mut full = view;
        full.live.owned = vec![3; 4];
        assert_eq!(recipe_cost(&full, &path), (1650, false));
    }

    #[test]
    fn route_preferences_change_new_offers_without_mutating_queued_orders() {
        let mut cat = price_branches();
        cat[1].price = 100;
        let view = recipe_view(cat, vec![]);
        let mut ui = ShopUi::default();
        ui.pick(Some(3));
        assert_eq!(ui.remaining(&view, 3), Some(1500));
        ui.choices.insert(3, vec![0, 2, 3]);
        assert_eq!(ui.remaining(&view, 3), Some(1650));
        assert_eq!(ui.remaining(&view, 2), Some(900));
        let mut order = shop::new_order(&view.live, 3);
        order.path = ui.chosen_path(3);
        ui.pick(Some(1));
        assert_eq!(ui.chosen_path(3), Some(vec![0, 2, 3]));
        ui.choices.remove(&3);
        assert_eq!(ui.remaining(&view, 3), Some(1500));
        assert_eq!(order.path, Some(vec![0, 2, 3]));
        assert_eq!(
            shop::order_costs(&view.cat, &view.live, &[order]),
            vec![Some(1650)]
        );
    }

    #[test]
    fn every_vanilla_item_keeps_the_compact_linear_display_and_original_buyer() {
        let meta: crate::native_items::Catalogue =
            serde_json::from_str(include_str!("../../research/hud-item_setting.json")).unwrap();
        // The asset also has a `mod_items` list, not an item definition.
        let mut keys: Vec<_> = meta
            .iter()
            .filter(|(_, value)| value.is_object())
            .map(|(key, _)| key.clone())
            .collect();
        keys.sort();
        let cat = shop::catalogue(&keys, &meta).unwrap();
        assert_eq!(cat.len(), 30);
        assert_eq!(cat.iter().filter(|i| i.tier == 0).count(), 6);
        assert_eq!(cat.iter().filter(|i| i.next.is_empty()).count(), 6);
        let view = recipe_view(cat, vec![]);
        let mut browser = RecipeBrowser::default();
        for target in 0..view.cat.len() {
            let route = browser.prepare(&view, target, None);
            assert_eq!(browser.total(), 1, "{}", view.cat[target].key);
            assert!(!browser.branched());
            assert!(!browser.detailed());
            assert_eq!(browser.buy_y(), BUY_Y);
            assert_eq!(browser.item_y(0), RECIPE_Y);
            assert!(browser.paths[0].len() <= RECIPE_COLS);
            assert_eq!(route, browser.paths[0]);
            let order = shop::new_order(&view.live, target);
            assert_eq!(order.path, None);
            assert_eq!(
                offer(&view, target),
                offer_path(&view, target, Some(&route))
            );
            for &part in &route[..route.len() - 1] {
                let mut owned = view.clone();
                owned.live.owned = vec![part];
                assert_eq!(
                    offer(&owned, target),
                    offer_path(&owned, target, Some(&route))
                );
            }
        }
    }
}
