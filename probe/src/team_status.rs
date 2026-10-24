//! Ten player status cards from the bound live worker, never replay candidates.
use crate::ui_writes::{NodeId, PropCache};
use crate::{own_selection::MatchKey, Logger};
use mod_api_stable::StableClient;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

pub const PATH: &str = "ingame.lt_team_status";

#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    pub id: usize,
    pub side: usize,
    pub lane: usize,
    pub champion: String,
    pub alive: Option<bool>,
    pub respawn: usize,
    pub level: usize,
    pub gold: usize,
    pub kda: (usize, usize, usize),
    pub cs: usize,
    pub items: Vec<String>,
    pub build: Option<Vec<String>>,
}
#[derive(Default)]
struct State {
    key: Option<MatchKey>,
    sampled: Option<usize>,
    at: Option<Instant>,
    players: Vec<Player>,
}
#[derive(Default)]
pub struct TeamStatus(Mutex<State>);
impl TeamStatus {
    pub fn reset_session(&self) {
        if let Ok(mut s) = self.0.lock() {
            *s = State::default();
        }
    }
    pub fn needs_sample(&self, key: MatchKey, tick: usize) -> bool {
        self.0.lock().is_ok_and(|mut s| {
            if s.key != Some(key) {
                *s = State {
                    key: Some(key),
                    ..State::default()
                };
            }
            if s.sampled == Some(tick) || (s.at.is_some() && !tick.is_multiple_of(6)) {
                return false;
            }
            s.sampled = Some(tick);
            true
        })
    }
    pub fn observe(&self, key: MatchKey, mut players: Vec<Player>, log: &Logger) {
        let Ok(mut s) = self.0.lock() else { return };
        if s.key != Some(key) {
            return;
        }
        for p in &mut players {
            if let Some(old) = s
                .players
                .iter()
                .find(|old| old.id == p.id && old.side == p.side && old.lane == p.lane)
            {
                if p.champion.is_empty() {
                    p.champion.clone_from(&old.champion);
                }
                if p.build.is_none() {
                    p.build.clone_from(&old.build);
                }
                if old.alive != p.alive {
                    log.write(&format!("TEAM STATUS key={key:?} player={} side={} lane={} alive={:?} respawn_ticks={}", p.id, p.side, p.lane, p.alive, p.respawn));
                }
            }
        }
        s.players = players;
        s.at = Some(crate::clock::now());
    }
    /// This match's last sample. When every actor stops calling think (the
    /// nexus explosion) the last values stay frozen, as in a pause (the
    /// author, 2026-10-20, option a); timers never run on wall time.
    pub fn snapshot(&self, key: Option<MatchKey>) -> Vec<Player> {
        let Ok(s) = self.0.lock() else {
            return Vec::new();
        };
        if key.is_none() || s.key != key {
            return Vec::new();
        }
        s.players.clone()
    }
}

fn slot(players: &[Player], side: usize, lane: usize) -> Option<&Player> {
    let mut matches = players.iter().filter(|p| p.side == side && p.lane == lane);
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}
fn caption(p: &Player) -> String {
    match p.alive {
        Some(true) => String::new(),
        Some(false) if p.respawn > 0 => p.respawn.div_ceil(60).to_string(),
        Some(false) => "—".into(),
        None => "?".into(),
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Geometry {
    columns: usize,
    rows: usize,
    row_height: usize,
    side_width: usize,
    width: usize,
    height: usize,
}
fn geometry(count: usize) -> Geometry {
    let columns = count.min(crate::inventory::TAB_COLUMNS);
    let rows = count.div_ceil(crate::inventory::TAB_COLUMNS).max(1);
    let row_height = (rows * 36 + 16).max(68);
    let side_width = 439 + columns * 36;
    Geometry {
        columns,
        rows,
        row_height,
        side_width,
        width: 40 + 26 + side_width * 2,
        height: 32 + 34 + row_height * 5,
    }
}
/// The columns of one side (1.6.0-dev4), as the game's own scoreboard
/// lays them out: the portrait at 6-54, then LV, K/D/A and CS centred in
/// their boxes with their headers centred over them, the items from their
/// header's left edge, gold right-aligned with its header 12 px inside the
/// side's edge; 16 px between neighbouring boxes. (x, width) in the side.
const GAP: usize = 16;
const PORTRAIT_END: usize = 54;
const LEVEL: (usize, usize) = (PORTRAIT_END + GAP, 40);
const KDA: (usize, usize) = (LEVEL.0 + LEVEL.1 + GAP, 110);
const CS: (usize, usize) = (KDA.0 + KDA.1 + GAP, 54);
const ITEMS_X: usize = CS.0 + CS.1 + GAP;
/// The items header's box: four tiles (the vanilla slot count).
const ITEMS_HEADER_W: usize = 4 * 36 - 4;
const GOLD_W: usize = 93;
const GOLD_RIGHT: usize = 12;
/// The gold column's left edge in a side `side_width` wide.
fn gold_x(side_width: usize) -> usize {
    side_width - GOLD_RIGHT - GOLD_W
}
/// Item tile `i`'s place in a row.
fn tile_at(i: usize) -> (usize, usize) {
    (
        ITEMS_X + i % crate::inventory::TAB_COLUMNS * 36,
        18 + i / crate::inventory::TAB_COLUMNS * 36,
    )
}
fn label(s: &mut String, id: &str, x: usize, w: usize, size: usize, color: &str, align: &str) {
    s.push_str(&format!("#{id}:label {{ x: {x}px; width: {w}px; height: 100%; z: 1604; font: \"asset/lt_direct_control/font/numeric\"; size: {size}; color: #{color}; align_y: Center; align_x: {align}; ignore_event: true; }}\n"));
}
#[cfg(test)]
pub(crate) fn tile_for_test() -> String {
    item_tile(6)
}
fn item_tile(i: usize) -> String {
    // Insets: background + 8.
    let inset = crate::ui_theme::shade(8, 0xff);
    format!("item{i}:color {{ width: 32px; height: 32px; z: 1603; color: #~4b4a49ff; rounding: Uniform {{ rounding: 2; }} ignore_event: true; #bg:color {{ x: 1px; y: 1px; width: 30px; height: 30px; z: 1604; color: #{inset}; ignore_event: true; }} #icon:image {{ x: 1px; y: 1px; width: 30px; height: 30px; z: 1605; sample_linear: false; visible: false; ignore_event: true; }} #unknown:label {{ font: \"asset/lt_direct_control/font/numeric\"; width: 100%; height: 100%; z: 1606; size: 18; text: \"?\"; align_x: Center; align_y: Center; color: #eeececff; visible: false; ignore_event: true; }} }}")
}
#[cfg(test)]
pub(crate) fn template_for_test() -> String {
    template()
}
fn template() -> String {
    let inset = crate::ui_theme::shade(8, 0xff);
    let mut s = String::from("lt_team_status:color { anchor_x: 0.5; pivot_x: 0.5; anchor_y: 0.5; pivot_y: 0.5; y: -80px; z: 1600; color: #~1e1e1df5; rounding: Uniform { rounding: 2; } ignore_event: true;\n#top:color { height: 1px; width: 100%; z: 1601; color: #~4b4a49ff; ignore_event: true; }\n#bottom:color { anchor_y: 1; pivot_y: 1; height: 1px; width: 100%; z: 1601; color: #~4b4a49ff; ignore_event: true; }\n");
    s.push_str("#left:color { width: 1px; height: 100%; z: 1601; color: #~4b4a49ff; ignore_event: true; }\n#right:color { anchor_x: 1; pivot_x: 1; width: 1px; height: 100%; z: 1601; color: #~4b4a49ff; ignore_event: true; }\n");
    for side in 0..2 {
        s.push_str(&format!("#side{side}:empty {{ y: 16px; ignore_event: true;\n#headers:empty {{ width: 100%; height: 34px; ignore_event: true;\n"));
        // Gold's x follows the item columns (written each frame).
        for (id, (x, w), align) in [
            ("level", LEVEL, "Center"),
            ("kda", KDA, "Center"),
            ("cs", CS, "Center"),
            ("items", (ITEMS_X, ITEMS_HEADER_W), "Left"),
            ("gold", (555, GOLD_W), "Right"),
        ] {
            label(&mut s, id, x, w, 13, "989694ff", align);
            // Header text is set once after spawning, using the host text API.
        }
        s.push_str("}\n");
        for lane in 0..5 {
            s.push_str(&format!("#lane{lane}:color {{ color: #00000000; ignore_event: true; z: 1601;\n#selected:color {{ width: 2px; height: 100%; z: 1602; color: #fdee00ff; visible: false; ignore_event: true; }}\n#line:color {{ anchor_y: 1; pivot_y: 1; width: 100%; height: 1px; z: 1602; color: #~4b4a4973; ignore_event: true; }}\n#portrait:color {{ x: 6px; y: 8px; width: 48px; height: 52px; z: 1603; color: #~4b4a49ff; ignore_event: true; rounding: Uniform {{ rounding: 2; }} #bg:color {{ x: 1px; y: 1px; width: 46px; height: 50px; z: 1604; color: #{inset}; ignore_event: true; }} #icon:image {{ anchor_x: 0.5; pivot_x: 0.5; anchor_y: 0.5; pivot_y: 0.5; width: 44px; height: 48px; z: 1605; sample_linear: false; visible: false; ignore_event: true; }} #shade:color {{ width: 100%; height: 100%; z: 1606; color: #00000099; visible: false; ignore_event: true; }} #timer:label {{ font: \"asset/lt_direct_control/font/numeric\"; width: 100%; height: 100%; z: 1607; size: 25; align_x: Center; align_y: Center; color: #ffffffff; outline: 1; outline_color: #000000ff; ignore_event: true; }} }}\n"));
            for (id, (x, w), size, color, align) in [
                ("level", LEVEL, 17, "cbc9c7ff", "Center"),
                ("kda", KDA, 22, "eeececff", "Center"),
                ("cs", CS, 21, "eeececff", "Center"),
                ("gold", (555, GOLD_W), 21, "fdee00ff", "Right"),
            ] {
                label(&mut s, id, x, w, size, color, align);
            }
            for i in 0..6 {
                s.push('#');
                s.push_str(&crate::ui_theme::themed(&item_tile(i)));
                s.push('\n');
            }
            s.push_str("}\n");
        }
        s.push_str("}\n");
    }
    s.push('}');
    s
}
/// One row of the panel: its nodes in the cache (1.6.0).
struct Row {
    root: NodeId,
    selected: NodeId,
    icon: NodeId,
    icon_path: String,
    portrait: NodeId,
    shade: NodeId,
    timer: NodeId,
    level: NodeId,
    cs: NodeId,
    gold: NodeId,
    kda: NodeId,
    /// (tile, icon, unknown) per inventory slot, grown with the panel.
    items: Vec<(NodeId, NodeId, NodeId)>,
    path: String,
}
struct Side {
    root: NodeId,
    /// level, kda, cs, items, gold
    headers: [NodeId; 5],
    rows: [Row; 5],
}
struct Nodes {
    root: NodeId,
    sides: [Side; 2],
}
impl Nodes {
    fn new(c: &mut PropCache) -> Self {
        let root = c.node(PATH);
        let sides = std::array::from_fn(|side| {
            let side_path = format!("{PATH}.side{side}");
            Side {
                root: c.node(&side_path),
                headers: ["level", "kda", "cs", "items", "gold"]
                    .map(|h| c.node(&format!("{side_path}.headers.{h}"))),
                rows: std::array::from_fn(|lane| {
                    let path = format!("{side_path}.lane{lane}");
                    let mut row = |node: &str| c.node(&format!("{path}.{node}"));
                    Row {
                        selected: row("selected"),
                        icon: row("portrait.icon"),
                        portrait: row("portrait"),
                        shade: row("portrait.shade"),
                        timer: row("portrait.timer"),
                        level: row("level"),
                        cs: row("cs"),
                        gold: row("gold"),
                        kda: row("kda"),
                        root: c.node(&path),
                        icon_path: format!("{path}.portrait.icon"),
                        items: Vec::new(),
                        path,
                    }
                }),
            }
        });
        Self { root, sides }
    }
    fn grow(&mut self, c: &mut PropCache, count: usize) {
        for s in self.sides.iter_mut() {
            for r in s.rows.iter_mut() {
                for i in r.items.len()..count {
                    let item = format!("{}.item{i}", r.path);
                    r.items.push((
                        c.node(&item),
                        c.node(&format!("{item}.icon")),
                        c.node(&format!("{item}.unknown")),
                    ));
                }
            }
        }
    }
}
fn vis_source((on,): (bool,)) -> String {
    format!("visible: {on};")
}
#[derive(Default)]
pub struct TeamUi {
    nodes: Option<Nodes>,
    cache: crate::ui_writes::PropCache,
    spawn_at: Option<Instant>,
    allocated: usize,
    portraits: HashMap<String, (String, Instant, bool)>,
    layout: Option<Geometry>,
    failed: bool,
}
impl TeamUi {
    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &mut self,
        ctx: &mut StableClient<'_>,
        active: bool,
        players: &[Player],
        selected: Option<usize>,
        artwork: &crate::player_hud::HudUi,
        log: &Logger,
    ) {
        let mut n = self
            .nodes
            .take()
            .unwrap_or_else(|| Nodes::new(&mut self.cache));
        self.panel(ctx, &mut n, active, players, selected, artwork, log);
        self.nodes = Some(n);
    }
    #[allow(clippy::too_many_arguments)]
    fn panel(
        &mut self,
        ctx: &mut StableClient<'_>,
        n: &mut Nodes,
        active: bool,
        players: &[Player],
        selected: Option<usize>,
        artwork: &crate::player_hud::HudUi,
        log: &Logger,
    ) {
        if !active || crate::ui_root::hidden() {
            if ctx.ui_exists(PATH) {
                let ok = self.cache.props_keyed(ctx, n.root, 0, (false,), vis_source);
                self.refused(ok, PATH, log);
            }
            if !active {
                self.spawn_at = None;
            }
            return;
        }
        if !ctx.ui_exists(PATH) {
            if self
                .spawn_at
                .is_some_and(|at| crate::clock::since(at) < Duration::from_secs(1))
            {
                return;
            }
            self.spawn_at = Some(crate::clock::now());
            if !self.spawn(ctx, log) {
                return;
            }
        }
        // The game buyer iterates this target vector; catalogue length does not
        // represent capacity. All rows share a column width, even for mixed plans.
        let count = players
            .iter()
            .map(|p| crate::inventory::slots(p.build.as_ref().map(Vec::len), p.items.len(), None))
            .max()
            .unwrap_or(0);
        for i in self.allocated..count {
            let mut ok = true;
            for side in 0..2 {
                for lane in 0..5 {
                    ok &= ctx.ui_spawn_source(
                        &format!("{PATH}.side{side}.lane{lane}"),
                        &crate::ui_theme::themed(&item_tile(i)),
                    );
                }
            }
            if !ok {
                log.write(&format!("TAB additional item node rejected index={i}"));
                break;
            }
            self.allocated = i + 1;
        }
        n.grow(&mut self.cache, self.allocated);
        let g = geometry(count);
        if self.layout != Some(g) {
            log.write(&format!("TAB layout slots={count} geometry={g:?}"));
            self.layout = Some(g);
        }
        let (width, height) = (g.width, g.height);
        let ok = self
            .cache
            .props_keyed(ctx, n.root, 1, (width, height), |(width, height)| {
                format!("visible: true; width: {width}px; height: {height}px;")
            });
        self.refused(ok, PATH, log);
        for side in 0..2 {
            let s = &n.sides[side];
            let ok = self.cache.props_keyed(
                ctx,
                s.root,
                1,
                (side, g.side_width, g.height),
                |(side, side_width, height)| {
                    format!(
                        "x: {}px; width: {side_width}px; height: {}px;",
                        20 + side * (side_width + 26),
                        height - 32
                    )
                },
            );
            self.refused(ok, PATH, log);
            let gold_x = gold_x(g.side_width);
            for (k, text) in [
                "LV",
                "K / D / A",
                "CS",
                crate::lang::tr("ITEMS"),
                crate::lang::tr("GOLD"),
            ]
            .into_iter()
            .enumerate()
            {
                self.cache.text(ctx, s.headers[k], text);
            }
            self.cache
                .props_keyed(ctx, s.headers[4], 1, (gold_x,), |(x,)| format!("x: {x}px;"));
            self.cache
                .props_keyed(ctx, s.headers[3], 0, (count > 0,), vis_source);
            for lane in 0..5 {
                let r = &s.rows[lane];
                let Some(p) = slot(players, side, lane) else {
                    let ok = self.cache.props_keyed(ctx, r.root, 0, (false,), vis_source);
                    self.refused(ok, &r.path, log);
                    continue;
                };
                self.cache.props_keyed(
                    ctx,
                    r.root,
                    1,
                    (lane, g.side_width, g.row_height),
                    |(lane, side_width, row_height)| {
                        format!(
                            "visible: true; y: {}px; width: {side_width}px; height: {row_height}px;",
                            34 + lane * row_height,
                        )
                    },
                );
                let mine = selected == Some(p.id);
                self.cache
                    .props_keyed(ctx, r.selected, 0, (mine,), vis_source);
                self.cache.props_keyed(ctx, r.root, 2, (mine,), |(mine,)| {
                    format!("color: #{};", if mine { "fdee0009" } else { "00000000" })
                });
                let retry = self
                    .portraits
                    .get(&r.icon_path)
                    .is_none_or(|(name, at, ok)| {
                        name != &p.champion
                            || (!ok && crate::clock::since(at) >= Duration::from_secs(1))
                    });
                if retry && !p.champion.is_empty() {
                    let ok = ctx.ui_set_champion_icon(&r.icon_path, &p.champion, 44., 48., 2.);
                    self.portraits.insert(
                        r.icon_path.clone(),
                        (p.champion.clone(), crate::clock::now(), ok),
                    );
                }
                let visible = self
                    .portraits
                    .get(&r.icon_path)
                    .is_some_and(|(name, _, ok)| *ok && name == &p.champion);
                self.cache
                    .props_keyed(ctx, r.icon, 0, (visible,), vis_source);
                self.cache
                    .props_keyed(ctx, r.portrait, 1, (side,), |(side,)| {
                        format!(
                            "color: #{};",
                            if side == 0 { "536979ff" } else { "79535dff" }
                        )
                    });
                self.cache
                    .props_keyed(ctx, r.shade, 0, (p.alive != Some(true),), vis_source);
                let alive = match p.alive {
                    Some(true) => 1u8,
                    Some(false) => 0,
                    None => 2,
                };
                self.cache
                    .text_keyed(ctx, r.timer, (alive, p.respawn), |_| caption(p));
                self.cache
                    .text_keyed(ctx, r.level, (p.level,), |(v,)| v.to_string());
                self.cache
                    .text_keyed(ctx, r.cs, (p.cs,), |(v,)| v.to_string());
                self.cache
                    .text_keyed(ctx, r.gold, (p.gold,), |(v,)| v.to_string());
                let (k, d, a) = p.kda;
                self.cache
                    .text_keyed(ctx, r.kda, (k, d, a), |(k, d, a)| kda_caption(k, d, a));
                self.cache
                    .props_keyed(ctx, r.gold, 1, (gold_x,), |(x,)| format!("x: {x}px;"));
                let capacity =
                    crate::inventory::slots(p.build.as_ref().map(Vec::len), p.items.len(), None);
                for i in 0..self.allocated {
                    let (tile, icon_node, unknown) = r.items[i];
                    let visible = i < capacity;
                    self.cache
                        .props_keyed(ctx, tile, 1, (visible, i), |(visible, i)| {
                            let (x, y) = tile_at(i);
                            format!("visible: {visible}; x: {x}px; y: {y}px;")
                        });
                    if !visible {
                        continue;
                    }
                    let icon = p.items.get(i).and_then(|key| artwork.item_icon(key));
                    let props = icon.as_ref().map_or_else(
                        || "visible: false;".into(),
                        |icon| format!("visible: true; {}", icon.properties()),
                    );
                    self.cache.props(ctx, icon_node, &props);
                    self.cache.props_keyed(
                        ctx,
                        unknown,
                        0,
                        (p.items.get(i).is_some() && icon.is_none(),),
                        vis_source,
                    );
                }
            }
        }
        // No input-blocking rectangles: battlefield commands pass through Tab.
    }
    /// The first refused write is logged once per panel build.
    fn refused(&mut self, ok: bool, path: &str, log: &Logger) {
        if !ok && !self.failed {
            log.write(&format!("TAB property rejected path={path}"));
            self.failed = true;
        }
    }
}

fn kda_caption(kills: usize, deaths: usize, assists: usize) -> String {
    format!("<#eeececff>{kills}<#989694ff>/<#ff855bff>{deaths}<#989694ff>/<#eeececff>{assists}<>")
}

impl TeamUi {
    fn spawn(&mut self, ctx: &mut StableClient<'_>, log: &Logger) -> bool {
        let ok = ctx.ui_spawn_source("ingame", &crate::ui_theme::themed(&template()));
        log.write(&format!(
            "TAB styled panel spawn={ok} exists={}",
            ctx.ui_exists(PATH)
        ));
        if !ok || !ctx.ui_exists(PATH) {
            return false;
        }
        self.cache.clear();
        self.portraits.clear();
        self.layout = None;
        self.allocated = 6;
        self.failed = false;
        true
    }
}
impl crate::prewarm::Prespawn for TeamUi {
    fn built(&self, ctx: &StableClient<'_>) -> bool {
        ctx.ui_exists(PATH)
    }
    /// The template's root shows; it is hidden at once, as with Tab released.
    fn prespawn(&mut self, ctx: &mut StableClient<'_>, log: &Logger) -> bool {
        if !self.spawn(ctx, log) {
            return false;
        }
        let root = self
            .nodes
            .get_or_insert_with(|| Nodes::new(&mut self.cache))
            .root;
        self.cache.props_keyed(ctx, root, 0, (false,), vis_source);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kda_uses_valid_native_rgba_runs_and_resets_colour() {
        let text = kda_caption(12, 3, 24);
        assert_eq!(crate::tooltips::rich(&text), text);
        assert_eq!(crate::tooltips::plain(&text), "12/3/24");
        assert!(text.ends_with("<>"));
    }
    fn player(side: usize, lane: usize, alive: bool, respawn: usize) -> Player {
        Player {
            id: side * 5 + lane,
            side,
            lane,
            champion: "harpy".into(),
            alive: Some(alive),
            respawn,
            level: 1,
            gold: 0,
            kda: (0, 0, 0),
            cs: 0,
            items: Vec::new(),
            build: None,
        }
    }
    #[test]
    fn adaptive_item_columns_stay_aligned_without_reducing_icon_size() {
        for (count, width) in [(4, 1232), (5, 1304), (6, 1376)] {
            let g = geometry(count);
            assert_eq!(g.width, width);
            assert_eq!(g.height, 406);
            assert_eq!(g.row_height, 68);
            assert!(tile_at(count - 1).0 + 32 + GAP <= gold_x(g.side_width));
        }
        let g = geometry(12);
        assert_eq!(g.columns, 10);
        assert_eq!(g.row_height, 88);
        assert!(g.width < 1920);
        let source = template();
        assert!(!source.contains("Your team"));
        assert!(!source.contains("Opposing team"));
        assert!(source.contains("size: 22"));
    }
    #[test]
    fn columns_sit_16_px_apart_centred_or_aligned_as_the_games_scoreboard() {
        // Box edge to box edge, the portrait included.
        assert_eq!(LEVEL.0, PORTRAIT_END + GAP);
        assert_eq!(KDA.0, LEVEL.0 + LEVEL.1 + GAP);
        assert_eq!(CS.0, KDA.0 + KDA.1 + GAP);
        assert_eq!(ITEMS_X, CS.0 + CS.1 + GAP);
        for count in [4, 5, 6, 10] {
            let g = geometry(count);
            let items_end = tile_at(g.columns - 1).0 + 32;
            assert_eq!(items_end + GAP, gold_x(g.side_width), "{count}");
            assert_eq!(gold_x(g.side_width) + GOLD_W + GOLD_RIGHT, g.side_width);
        }
        // The widest values fit their boxes (packaged advances; the numeric
        // weight is about 2 % wider): level 30, K/D/A 99/99/99, CS 999,
        // 99999 gold, and every language's GOLD header.
        let fits =
            |text: &str, size: f32, w: usize| crate::hud_style::width(text, size) * 1.03 < w as f32;
        assert!(fits("30", 17., LEVEL.1) && fits("LV", 13., LEVEL.1));
        assert!(fits("99/99/99", 22., KDA.1) && fits("K / D / A", 13., KDA.1));
        assert!(fits("999", 21., CS.1) && fits("CS", 13., CS.1));
        assert!(fits("99999", 21., GOLD_W) && fits("ЗОЛОТО", 13., GOLD_W));
        // Headers and values share each column's box and alignment.
        let t = template();
        let numeric = "height: 100%; z: 1604; font: \"asset/lt_direct_control/font/numeric\";";
        for (x, w) in [LEVEL, KDA, CS] {
            let header = format!(
                "x: {x}px; width: {w}px; {numeric} size: 13; color: #989694ff; align_y: Center; align_x: Center;"
            );
            assert_eq!(t.matches(&header).count(), 2, "{x}");
            let value = format!("x: {x}px; width: {w}px; {numeric}");
            assert_eq!(t.matches(&value).count(), 12, "{x}");
        }
        assert!(t.contains(&format!(
            "#items:label {{ x: {ITEMS_X}px; width: {ITEMS_HEADER_W}px;"
        )));
        assert_eq!(t.matches("align_x: Left;").count(), 2);
        assert_eq!(t.matches("align_x: Right;").count(), 12);
        // The panel keeps its width.
        assert_eq!(geometry(4).width, 1232);
    }
    #[test]
    fn missing_build_read_retains_only_same_match_capacity() {
        let team = TeamStatus::default();
        let key = (1, 33, 1);
        let log = crate::test_support::logger("team-capacity");
        assert!(team.needs_sample(key, 1));
        let mut first = player(0, 0, true, 0);
        first.build = Some(vec!["target".into(); 5]);
        team.observe(key, vec![first], &log);
        team.observe(key, vec![player(0, 0, true, 0)], &log);
        assert_eq!(team.snapshot(Some(key))[0].build.as_ref().unwrap().len(), 5);
        let next = (2, 34, 2);
        assert!(team.needs_sample(next, 1));
        team.observe(next, vec![player(0, 0, true, 0)], &log);
        assert!(team.snapshot(Some(next))[0].build.is_none());
    }
    #[test]
    fn dead_identity_survives_absent_entity_and_values_freeze_when_samples_stop() {
        let team = TeamStatus::default();
        let key = (1, 33, 1);
        let log = crate::test_support::logger("team-death");
        assert!(team.needs_sample(key, 1));
        team.observe(key, vec![player(1, 2, true, 0)], &log);
        let mut dead = player(1, 2, false, 121);
        dead.champion.clear();
        team.observe(key, vec![dead], &log);
        team.0.lock().unwrap().at = Some(Instant::now() - Duration::from_secs(2));
        // Samples stopped (pause, nexus explosion): the last values stay.
        let frozen = team.snapshot(Some(key));
        assert_eq!(frozen[0].champion, "harpy");
        assert_eq!(caption(&frozen[0]), "3");
        team.observe(key, vec![player(1, 2, true, 0)], &log);
        assert_eq!(caption(&team.snapshot(Some(key))[0]), "");
    }
    #[test]
    fn sides_lanes_and_new_session_do_not_reuse_old_identity_or_sampling() {
        let team = TeamStatus::default();
        let key = (1, 33, 1);
        let log = crate::test_support::logger("team-session");
        assert!(team.needs_sample(key, 1));
        assert!(!team.needs_sample(key, 1));
        let players = vec![player(1, 0, false, 60), player(0, 0, true, 0)];
        team.observe(key, players.clone(), &log);
        assert!(!team.needs_sample(key, 2));
        assert!(team.needs_sample(key, 6));
        assert!(!team.needs_sample(key, 6));
        assert_eq!(slot(&players, 0, 0).unwrap().id, 0);
        assert_eq!(slot(&players, 1, 0).unwrap().id, 5);
        assert!(team.snapshot(Some((2, 33, 2))).is_empty());
        team.reset_session();
        assert!(team.snapshot(Some(key)).is_empty());
        assert!(team.needs_sample(key, 1));
        let mut new = player(0, 0, false, 60);
        new.champion.clear();
        team.observe(key, vec![new], &log);
        assert!(team.snapshot(Some(key))[0].champion.is_empty());
        let mut ambiguous = players.clone();
        ambiguous.push(player(0, 0, true, 0));
        assert!(slot(&ambiguous, 0, 0).is_none());
    }
}
