//! Stats panels (display side). Your champion's stats left of the Q slot,
//! toggled with the Stats panel binding (C); the selected unit's portrait,
//! level, health and stats at the top left. Same style as the skill slots:
//! grey frame, inset fill (background + 5), drop shadow. Data:
//! crate::stats_panel.
use crate::{
    camera::Rect,
    stats_panel::{self, Kind, Unit, FORMATS, ICONS},
    Logger,
};
use mod_api_stable::StableClient;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

pub const OWN: &str = "ingame.lt_stats_own";
pub const TARGET: &str = "ingame.lt_stats_target";
const NUMERIC: &str = "asset/lt_direct_control/font/numeric";

/// One grid of stat cells (icon + value) inside `parent`.
fn cells(s: &mut String, (x0, y0): (i32, i32), (dx, dy): (i32, i32)) {
    for (k, icon) in ICONS.iter().enumerate() {
        let (x, y) = (x0 + (k as i32 % 3) * dx, y0 + (k as i32 / 3) * dy);
        s.push_str(
            &icon
                .node(&format!("i{k}"), (x, y, 18), 1004)
                .replace("visible: false;", "visible: true;"),
        );
        s.push_str(&format!("#v{k}:label {{ font: \"{NUMERIC}\"; x: {}px; y: {y}px; width: {}px; height: 18px; size: 15; align_x: Left; align_y: Center; color: #eeececff; text: \"\"; ignore_event: true; z: 1004; }}\n", x + 23, dx - 25));
    }
}
/// A skill-slot style frame: grey edge, shadow, inset fill.
fn frame(s: &mut String, (x, y, w, h): (i32, i32, i32, i32)) {
    s.push_str(&format!(
        "#panel:color {{ x: {x}px; y: {y}px; width: {w}px; height: {h}px; color: #~4b4a49ff; rounding: Uniform {{ rounding: 2; }} ignore_event: true; z: 1001;\n#shadow:color {{ x: 0px; y: 3px; width: {w}px; height: {h}px; color: #00000066; ignore_event: true; z: 1000; }}\n#fill:color {{ x: 1px; y: 1px; width: {}px; height: {}px; color: #{}; ignore_event: true; z: 1002; }}\n",
        w - 2,
        h - 2,
        crate::ui_theme::shade(5, 0xff)
    ));
}
fn own_template() -> String {
    // The HUD's root (bottom-anchored, 1920 wide) 4 px taller, so the panel
    // stays beside the Q slot (HUD y 4, 80 tall) at any resolution. The panel
    // is 92 tall for row spacing, centred on the slots (HUD y -2 = root y 2).
    let mut s = String::from("lt_stats_own:empty { width: 1920px; height: 150px; anchor_x: 0.5; pivot_x: 0.5; anchor_y: 1; pivot_y: 1; z: 1000; ignore_event: true; visible: false;\n");
    frame(&mut s, (612, 2, 208, 92));
    cells(&mut s, (7, 5), (67, 21));
    s.push_str("}\n}\n");
    s
}
fn target_template() -> String {
    // Top left, under the team-name header: health across the top, the
    // portrait beside the stats grid (centred on its four rows).
    let mut s = String::from("lt_stats_target:empty { x: 8px; y: 56px; width: 292px; height: 129px; z: 1000; ignore_event: true; visible: false;\n");
    frame(&mut s, (0, 0, 292, 129));
    s.push_str("#face:color { x: 10px; y: 46px; width: 64px; height: 64px; color: #~4b4a49ff; rounding: Uniform { rounding: 2; } ignore_event: true; z: 1003;\n#well:color { x: 1px; y: 1px; width: 62px; height: 62px; color: #~0e0d0cff; ignore_event: true; z: 1004; }\n#portrait:image { x: 1px; y: 1px; width: 62px; height: 62px; sample_linear: false; visible: false; ignore_event: true; z: 1005; }\n#unit:image { x: 11px; y: 11px; width: 40px; height: 40px; source: \"asset/lt_direct_control/ui/hud_minion\"; color: #cbc9c7ff; visible: false; ignore_event: true; z: 1005; }\n#badge:color { x: 0px; y: 46px; width: 26px; height: 18px; color: #1c1a18ee; ignore_event: true; z: 1006; }\n");
    s.push_str(&format!("#level:label {{ font: \"{NUMERIC}\"; x: 0px; y: 46px; width: 26px; height: 18px; size: 14; align_x: Center; align_y: Center; color: #eeececff; text: \"\"; ignore_event: true; z: 1007; }}\n}}\n"));
    s.push_str("#hp_back:color { x: 10px; y: 10px; width: 272px; height: 20px; color: #~0e0d0cff; ignore_event: true; z: 1003; }\n#hp_fill:color { x: 11px; y: 11px; width: 270px; height: 18px; color: #6aff55ff; ignore_event: true; z: 1004; }\n");
    s.push_str(&format!("#hp_text:label {{ font: \"{NUMERIC}\"; x: 10px; y: 10px; width: 272px; height: 20px; size: 13; align_x: Center; align_y: Center; color: #ffffffff; outline: 1; outline_color: #000000ff; text: \"\"; ignore_event: true; z: 1005; }}\n"));
    cells(&mut s, (84, 38), (68, 21));
    s.push_str("}\n}\n");
    s
}

#[derive(Default)]
pub struct StatsUi {
    cache: HashMap<String, String>,
    spawn: HashMap<&'static str, Instant>,
    portrait: Option<(String, bool)>,
    previous_key: bool,
}
impl StatsUi {
    fn props(&mut self, ctx: &mut StableClient<'_>, path: &str, value: &str) {
        crate::hud_motion::properties(ctx, &mut self.cache, path, value);
    }
    fn text(&mut self, ctx: &mut StableClient<'_>, path: &str, value: &str) {
        let key = format!("text:{path}");
        if self.cache.get(&key).map(String::as_str) != Some(value) && ctx.ui_set_text(path, value) {
            self.cache.insert(key, value.to_owned());
        }
    }
    /// Spawn a root if missing; false while it does not exist.
    fn ensure(&mut self, ctx: &mut StableClient<'_>, path: &'static str, log: &Logger) -> bool {
        if ctx.ui_exists(path) {
            return true;
        }
        if self
            .spawn
            .get(path)
            .is_some_and(|t| t.elapsed() < Duration::from_secs(1))
        {
            return false;
        }
        self.spawn.insert(path, Instant::now());
        self.cache.retain(|k, _| !k.contains(path));
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
    fn values(&mut self, ctx: &mut StableClient<'_>, root: &str, unit: &Unit) {
        for (k, (value, format)) in unit.values.iter().zip(FORMATS).enumerate() {
            let text = stats_panel::format(*value, format);
            self.text(ctx, &format!("{root}.panel.v{k}"), &text);
        }
    }
    /// `battlefield`: in a match; `controls`: under direct control. Returns
    /// the visible panels' screen rectangles (clicks there are not orders).
    pub fn apply(
        &mut self,
        ctx: &mut StableClient<'_>,
        battlefield: bool,
        controls: bool,
        sampling: bool,
        toggle: bool,
        log: &Logger,
    ) -> Vec<Rect> {
        // The binding toggles the own panel; the choice is remembered.
        if toggle && !self.previous_key && battlefield {
            let shown = crate::settings::current().number("stats_panel_shown") == 1.;
            crate::settings::set_option("stats_panel_shown", if shown { 0. } else { 1. });
        }
        self.previous_key = toggle;
        let (own, target) = if battlefield {
            stats_panel::snapshot(sampling)
        } else {
            (None, None)
        };
        let shown = crate::settings::current().number("stats_panel_shown") == 1.;
        let mut bounds = Vec::new();
        // Own panel.
        let own = own.filter(|_| controls && shown);
        if own.is_some() || ctx.ui_exists(OWN) {
            if let Some(unit) = own.as_ref() {
                if self.ensure(ctx, OWN, log) {
                    self.props(ctx, OWN, "visible: true;");
                    self.values(ctx, OWN, unit);
                    bounds.extend(rect(ctx, &format!("{OWN}.panel")));
                }
            } else {
                self.props(ctx, OWN, "visible: false;");
            }
        }
        // Target frame.
        if let Some(unit) = target.as_ref() {
            if self.ensure(ctx, TARGET, log) {
                self.props(ctx, TARGET, "visible: true;");
                self.target(ctx, unit);
                bounds.extend(rect(ctx, &format!("{TARGET}.panel")));
            }
        } else if ctx.ui_exists(TARGET) {
            self.props(ctx, TARGET, "visible: false;");
        }
        bounds
    }
    fn target(&mut self, ctx: &mut StableClient<'_>, unit: &Unit) {
        let face = format!("{TARGET}.panel.face");
        // Champions show their face; other units (or a face that failed to
        // load) a glyph: the game's own tower and jungle icons, the mod's
        // minion.
        let face_shown = match unit.champion.as_ref() {
            Some(name) => {
                if self.portrait.as_ref().map(|p| &p.0) != Some(name) {
                    let ok =
                        ctx.ui_set_champion_icon(&format!("{face}.portrait"), name, 62., 62., 2.);
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
        self.props(
            ctx,
            &format!("{face}.portrait"),
            &format!("visible: {face_shown};"),
        );
        self.props(
            ctx,
            &format!("{face}.unit"),
            &format!("visible: {}; source: \"{glyph}\";", !face_shown),
        );
        self.text(ctx, &format!("{face}.level"), &unit.level.to_string());
        let (hp, max) = unit.hp;
        let ratio = if max > 0 { hp as f32 / max as f32 } else { 0. };
        let color = if unit.friendly {
            "6aff55ff"
        } else {
            "ff5c6cff"
        };
        self.props(
            ctx,
            &format!("{TARGET}.panel.hp_fill"),
            &format!(
                "width: {:.0}px; color: #{color};",
                270. * ratio.clamp(0., 1.)
            ),
        );
        let shield = if unit.shield > 0 {
            format!(" (+{})", unit.shield)
        } else {
            String::new()
        };
        self.text(
            ctx,
            &format!("{TARGET}.panel.hp_text"),
            &format!("{hp} / {max}{shield}"),
        );
        self.values(ctx, TARGET, unit);
    }
}
fn rect(ctx: &StableClient<'_>, path: &str) -> Option<Rect> {
    let (x, y, w, h) = ctx.ui_node_rect(path)?;
    let r = Rect { x, y, w, h };
    r.valid().then_some(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn templates_have_a_cell_per_stat_and_balanced_braces() {
        for t in [own_template(), target_template()] {
            assert_eq!(t.matches('{').count(), t.matches('}').count());
            for k in 0..crate::stats_panel::STATS {
                assert!(t.contains(&format!("#i{k}:image")) && t.contains(&format!("#v{k}:label")));
            }
        }
        assert!(target_template().contains("#portrait:image"));
    }
}
