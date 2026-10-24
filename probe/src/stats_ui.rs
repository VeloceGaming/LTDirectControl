//! Stats panels (display side). Your champion's stats left of the Q slot,
//! toggled with the Stats panel binding (C); the selected unit's portrait,
//! level, health and stats at the top left. Same style as the skill slots:
//! grey frame, inset fill (background + 5), drop shadow. Data:
//! crate::stats_panel.
use crate::{
    camera::Rect,
    stats_draw,
    stats_panel::{self, Kind, Unit, ICONS},
    ui_writes::NodeId,
    Logger,
};
use mod_api_stable::StableClient;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

pub const OWN: &str = "ingame.lt_stats_own";
pub const TARGET: &str = "ingame.lt_stats_target";
const OWN_PANEL: &str = "ingame.lt_stats_own.panel";
const TARGET_PANEL: &str = "ingame.lt_stats_target.panel";

/// One grid of stat icons inside `parent` (the value texts are drawn).
fn cells(s: &mut String, grid: ((f32, f32), (f32, f32))) {
    for (k, icon) in ICONS.iter().enumerate() {
        let (x, y) = stats_draw::cell(grid, k);
        s.push_str(
            &icon
                .node(&format!("i{k}"), (x as i32, y as i32, 18), 1004)
                .replace("visible: false;", "visible: true;"),
        );
    }
}
/// The panel's container: the frame itself is drawn (`stats_draw.rs`).
fn panel(s: &mut String, (x, y, w, h): (f32, f32, f32, f32)) {
    s.push_str(&format!(
        "#panel:empty {{ x: {x}px; y: {y}px; width: {w}px; height: {h}px; ignore_event: true; z: 1001;\n"
    ));
}
#[cfg(test)]
pub(crate) fn templates_for_test() -> [String; 2] {
    [own_template(), target_template()]
}
fn own_template() -> String {
    // The HUD's root (bottom-anchored, 1920 wide) 4 px taller, so the panel
    // stays beside the Q slot (HUD y 4, 80 tall) at any resolution. The panel
    // is 92 tall for row spacing, centred on the slots (HUD y -2 = root y 2).
    let (w, h) = stats_draw::OWN_ROOT;
    let mut s = format!("lt_stats_own:empty {{ width: {w}px; height: {h}px; anchor_x: 0.5; pivot_x: 0.5; anchor_y: 1; pivot_y: 1; z: 1000; ignore_event: true; visible: false;\n");
    panel(&mut s, stats_draw::OWN_PANEL);
    cells(&mut s, stats_draw::OWN_CELLS);
    s.push_str("}\n}\n");
    s
}
fn target_template() -> String {
    // Top left, under the team-name header: health across the top, the
    // portrait beside the stats grid (centred on its four rows).
    let (x, y) = stats_draw::TARGET_AT;
    let (_, _, w, h) = stats_draw::TARGET_PANEL;
    let mut s = format!("lt_stats_target:empty {{ x: {x}px; y: {y}px; width: {w}px; height: {h}px; z: 1000; ignore_event: true; visible: false;\n");
    panel(&mut s, stats_draw::TARGET_PANEL);
    let (fx, fy, fw, fh) = stats_draw::FACE;
    s.push_str(&format!("#face:empty {{ x: {fx}px; y: {fy}px; width: {fw}px; height: {fh}px; ignore_event: true; z: 1003;\n#portrait:image {{ x: 1px; y: 1px; width: 62px; height: 62px; sample_linear: false; visible: false; ignore_event: true; z: 1005; }}\n#unit:image {{ x: 11px; y: 11px; width: 40px; height: 40px; source: \"asset/lt_direct_control/ui/hud_minion\"; color: #cbc9c7ff; visible: false; ignore_event: true; z: 1005; }}\n}}\n"));
    cells(&mut s, stats_draw::TARGET_CELLS);
    s.push_str("}\n}\n");
    s
}

/// The panels' nodes in the cache (1.6.0).
#[derive(Clone, Copy)]
struct Nodes {
    own: NodeId,
    target: NodeId,
    portrait: NodeId,
    unit: NodeId,
}
#[derive(Default)]
pub struct StatsUi {
    nodes: Option<Nodes>,
    cache: crate::ui_writes::PropCache,
    spawn: HashMap<&'static str, Instant>,
    portrait: Option<(String, bool)>,
    previous_key: bool,
    /// This frame's commands (`stats_draw.rs`), sent in `post_render`.
    pub(crate) list: crate::draw_list::DrawList,
    /// `draw_list::check_origin`'s state for the own and target panels.
    origin_checked: [u8; 2],
}
impl StatsUi {
    /// Spawn a root if missing; false while it does not exist.
    fn ensure(&mut self, ctx: &mut StableClient<'_>, path: &'static str, log: &Logger) -> bool {
        if ctx.ui_exists(path) {
            return true;
        }
        if self
            .spawn
            .get(path)
            .is_some_and(|t| crate::clock::since(t) < Duration::from_secs(1))
        {
            return false;
        }
        self.spawn.insert(path, crate::clock::now());
        self.spawn_root(ctx, path, log)
    }
    fn spawn_root(&mut self, ctx: &mut StableClient<'_>, path: &'static str, log: &Logger) -> bool {
        self.cache.forget(path);
        if path == TARGET {
            self.portrait = None;
        }
        let source = if path == OWN {
            own_template()
        } else {
            target_template()
        };
        let ok =
            ctx.ui_spawn_source("ingame", &crate::ui_theme::themed(&source)) && ctx.ui_exists(path);
        log.write(&format!("STATS UI spawn {path} ok={ok}"));
        ok
    }
    fn nodes(&mut self) -> Nodes {
        let cache = &mut self.cache;
        *self.nodes.get_or_insert_with(|| {
            let mut id = |p: String| cache.node(&p);
            Nodes {
                own: id(OWN.into()),
                target: id(TARGET.into()),
                portrait: id(format!("{TARGET}.panel.face.portrait")),
                unit: id(format!("{TARGET}.panel.face.unit")),
            }
        })
    }
    fn show(&mut self, ctx: &mut StableClient<'_>, id: NodeId, on: bool) {
        let on = on && !crate::ui_root::hidden();
        self.cache
            .props_keyed(ctx, id, 0, (on,), |(on,)| format!("visible: {on};"));
    }
    /// `battlefield`: in a match; `controls`: under direct control. Returns
    /// the visible panels' screen rectangles (clicks there are not orders).
    pub fn apply(
        &mut self,
        ctx: &mut StableClient<'_>,
        battlefield: bool,
        controls: bool,
        toggle: bool,
        log: &Logger,
        bounds: &mut Vec<Rect>,
    ) {
        self.list.clear();
        let ingame = ctx.ui_node_rect("ingame");
        // The binding toggles the own panel; the choice is remembered.
        if toggle && !self.previous_key && battlefield {
            let shown = crate::settings::current_shared().number("stats_panel_shown") == 1.;
            crate::settings::set_option("stats_panel_shown", if shown { 0. } else { 1. });
        }
        self.previous_key = toggle;
        let (own, target) = if battlefield {
            stats_panel::snapshot()
        } else {
            (None, None)
        };
        let shown = crate::settings::current_shared().number("stats_panel_shown") == 1.;
        let n = self.nodes();
        // Own panel.
        let own = own.filter(|_| controls && shown);
        if own.is_some() || ctx.ui_exists(OWN) {
            if let Some(unit) = own.as_ref() {
                if self.ensure(ctx, OWN, log) {
                    self.show(ctx, n.own, true);
                    stats_draw::own(unit, ingame, &mut self.list);
                    crate::draw_list::check_origin(
                        "own stats panel",
                        stats_draw::own_origin(ingame),
                        ctx.ui_node_rect(OWN),
                        &mut self.origin_checked[0],
                        log,
                    );
                    if !crate::ui_root::hidden() {
                        bounds.extend(rect(ctx, OWN_PANEL));
                    }
                }
            } else {
                self.show(ctx, n.own, false);
            }
        }
        // Target frame.
        if let Some(unit) = target.as_ref() {
            if self.ensure(ctx, TARGET, log) {
                self.show(ctx, n.target, true);
                self.target(ctx, &n, unit);
                stats_draw::target(unit, ingame, &mut self.list);
                crate::draw_list::check_origin(
                    "target stats panel",
                    stats_draw::target_origin(ingame),
                    ctx.ui_node_rect(TARGET),
                    &mut self.origin_checked[1],
                    log,
                );
                if !crate::ui_root::hidden() {
                    bounds.extend(rect(ctx, TARGET_PANEL));
                }
            }
        } else if ctx.ui_exists(TARGET) {
            self.show(ctx, n.target, false);
        }
    }
    fn target(&mut self, ctx: &mut StableClient<'_>, n: &Nodes, unit: &Unit) {
        // Champions show their face; other units (or a face that failed to
        // load) a glyph: the game's own tower and jungle icons, the mod's
        // minion.
        let face_shown = match unit.champion.as_ref() {
            Some(name) => {
                if self.portrait.as_ref().map(|p| &p.0) != Some(name) {
                    let ok = ctx.ui_set_champion_icon(
                        &format!("{TARGET_PANEL}.face.portrait"),
                        name,
                        62.,
                        62.,
                        2.,
                    );
                    self.portrait = Some((name.clone(), ok));
                }
                self.portrait.as_ref().is_some_and(|p| p.1)
            }
            None => false,
        };
        let glyph = match unit.kind {
            Kind::Tower => "asset/base/ui/icons/tower",
            Kind::Monster => "asset/base/ui/icons/jungle",
            Kind::Minion | Kind::Champion => "asset/lt_direct_control/ui/hud_minion",
        };
        self.cache
            .props_keyed(ctx, n.portrait, 0, (face_shown,), |(on,)| {
                format!("visible: {on};")
            });
        let glyph_index = unit.kind as u8;
        self.cache
            .props_keyed(ctx, n.unit, 1, (!face_shown, glyph_index), |(on, _)| {
                format!("visible: {on}; source: \"{glyph}\";")
            });
    }
}
fn rect(ctx: &StableClient<'_>, path: &str) -> Option<Rect> {
    let (x, y, w, h) = ctx.ui_node_rect(path)?;
    let r = Rect { x, y, w, h };
    r.valid().then_some(r)
}

impl crate::prewarm::Prespawn for StatsUi {
    fn built(&self, ctx: &StableClient<'_>) -> bool {
        ctx.ui_exists(OWN) && ctx.ui_exists(TARGET)
    }
    /// Both templates' roots are hidden.
    fn prespawn(&mut self, ctx: &mut StableClient<'_>, log: &Logger) -> bool {
        let mut ok = true;
        for path in [OWN, TARGET] {
            if !ctx.ui_exists(path) {
                ok &= self.spawn_root(ctx, path, log);
            }
        }
        ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn templates_have_a_cell_per_stat_and_balanced_braces() {
        for t in [own_template(), target_template()] {
            assert_eq!(t.matches('{').count(), t.matches('}').count());
            for k in 0..crate::stats_panel::STATS {
                assert!(t.contains(&format!("#i{k}:image")));
            }
        }
        assert!(target_template().contains("#portrait:image"));
    }
}
