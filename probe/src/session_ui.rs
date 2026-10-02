//! Portrait-only preparation; handlers enqueue phase/key-bound choices.
use crate::{
    camera::Rect,
    native_timing::{MatchKey, NativeTiming, Phase},
    own_selection::PlayerIdentity,
    ui_graphics, Logger,
};
use mod_api_stable::{StableClient, UiEventKindV1};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
const PATH: &str = "ingame.lt_session_controls";
#[derive(Default)]
struct Events {
    choice: Option<(MatchKey, u64, usize)>,
}
fn button(s: &mut String, name: &str, glyph: &str, x: usize, y: usize) {
    s.push_str(&format!("#{name}:color_icon_button {{ x: {x}px; y: {y}px; width: 44px; height: 44px; z: 1102; color: #333333ff; hover: {{ color: #666666ff; }} text: {{ text: \"\"; }}"));
    s.push_str(&ui_graphics::image("glyph", glyph, 8, 8, 28, 1103));
    s.push_str("}\n");
}
fn template() -> String {
    let mut s = String::from("lt_session_controls:color { width: 400px; height: 150px; anchor_x: 0.5; anchor_y: 0.5; pivot_x: 0.5; pivot_y: 0.5; z: 1100; color: #141414ff; rounding: Uniform { rounding: 8; } ignore_event: true;\n");
    for lane in 0..5 {
        s.push_str(&format!("#lane{lane}:color_icon_button {{ x: {}px; y: 18px; width: 64px; height: 64px; z: 1102; color: #555555ff; hover: {{ color: #eeeeeeff; }} text: {{ text: \"\"; }} #bg:color {{ x: 2px; y: 2px; width: 60px; height: 60px; z: 1103; color: #242424ff; ignore_event: true; }} #portrait:image {{ x: 4px; y: 4px; width: 56px; height: 56px; z: 1104; sample_linear: false; ignore_event: true; visible: false; }} }}\n",24+lane*72));
    }
    s.push_str(&ui_graphics::image(
        "loading",
        "hourglass",
        184,
        30,
        32,
        1104,
    ));
    s.push_str("#selected:image { x: 6px; y: 4px; width: 36px; height: 36px; z: 1103; sample_linear: false; ignore_event: true; visible: false; }\n");
    s.push_str(&ui_graphics::image(
        "manual",
        "controller",
        31,
        28,
        18,
        1104,
    ));
    button(&mut s, "primary", "play", 150, 94);
    button(&mut s, "release", "chip", 206, 94);
    for (name, x, text) in [
        ("start_key", 140, "Ctrl Home"),
        ("release_key", 202, "Ctrl End"),
    ] {
        s.push_str(&format!("#{name}:label {{ @\"asset/base/style/main#label\"; x: {x}px; y: 137px; width: 62px; height: 12px; size: 10; align_x: Center; color: #999999ff; z: 1104; text: {text:?}; ignore_event: true; }}\n"));
    }
    s.push('}');
    s
}
#[derive(Default)]
pub struct SessionUi {
    spawn_at: Option<Instant>,
    registered: bool,
    events: Arc<Mutex<Events>>,
    cache: HashMap<String, String>,
    portraits: HashMap<String, String>,
    attempted: HashMap<String, (String, Instant)>,
}
impl SessionUi {
    pub fn take_choice(
        &self,
        phase: Option<Phase>,
        key: Option<MatchKey>,
        generation: u64,
    ) -> Option<usize> {
        let (bound, bound_generation, lane) = self.events.lock().ok()?.choice.take()?;
        (phase == Some(Phase::Ready) && key == Some(bound) && generation == bound_generation)
            .then_some(lane)
    }
    fn props(&mut self, ctx: &mut StableClient<'_>, node: &str, text: String) {
        if self.cache.get(node) == Some(&text) {
            return;
        }
        if ctx.ui_set_properties(&format!("{PATH}.{node}"), &text) {
            self.cache.insert(node.into(), text);
        }
    }
    fn portrait(&mut self, ctx: &mut StableClient<'_>, node: &str, name: &str, size: f32) {
        let path = format!("{PATH}.{node}");
        if !name.is_empty()
            && self.portraits.get(node).is_none_or(|n| n != name)
            && self
                .attempted
                .get(node)
                .is_none_or(|(old, at)| old != name || at.elapsed() >= Duration::from_secs(1))
        {
            self.attempted
                .insert(node.into(), (name.into(), Instant::now()));
            if ctx.ui_set_champion_icon(&path, name, size, size, 2.) {
                self.portraits.insert(node.into(), name.into());
            }
        }
        self.props(
            ctx,
            node,
            format!(
                "visible: {};",
                self.portraits.get(node).is_some_and(|n| n == name)
            ),
        );
    }
    pub fn apply(
        &mut self,
        ctx: &mut StableClient<'_>,
        timing: &Arc<NativeTiming>,
        players: &[PlayerIdentity],
        selected: Option<usize>,
        log: &Logger,
    ) -> Vec<Rect> {
        let Some(phase @ (Phase::Loading | Phase::Ready | Phase::Running | Phase::Paused)) =
            timing.ui_phase()
        else {
            if ctx.ui_exists(PATH) {
                ctx.ui_set_visible(PATH, false);
            }
            return Vec::new();
        };
        if !ctx.ui_exists(PATH) {
            if self
                .spawn_at
                .is_some_and(|t| t.elapsed() < Duration::from_secs(1))
            {
                return Vec::new();
            }
            self.spawn_at = Some(Instant::now());
            self.cache.clear();
            self.portraits.clear();
            self.attempted.clear();
            self.registered = false;
            if !ctx.ui_spawn_source("ingame", &template()) || !ctx.ui_exists(PATH) {
                return Vec::new();
            }
            log.write("SESSION portrait UI spawned");
        }
        if !self.registered {
            self.registered = true;
            for (node, primary) in [("primary", true), ("release", false)] {
                let owner = timing.clone();
                ctx.ui_register_path_events(&format!("{PATH}.{node}"), move |ctx| {
                    if ctx
                        .ui_current_event()
                        .is_some_and(|e| e.kind == Some(UiEventKindV1::Click))
                    {
                        owner.request_action(primary);
                    }
                });
            }
            for lane in 0..5 {
                let owner = timing.clone();
                let events = self.events.clone();
                ctx.ui_register_path_events(&format!("{PATH}.lane{lane}"), move |ctx| {
                    if ctx
                        .ui_current_event()
                        .is_some_and(|e| e.kind == Some(UiEventKindV1::Click))
                        && owner.phase() == Some(Phase::Ready)
                    {
                        if let (Some(key), Ok(mut events)) = (owner.match_key(), events.lock()) {
                            events.choice = Some((key, owner.generation(), lane));
                        }
                    }
                });
            }
        }
        ctx.ui_set_visible(PATH, true);
        let preparing = matches!(phase, Phase::Loading | Phase::Ready);
        ctx.ui_set_properties(PATH,if preparing {"width: 400px; height: 150px; anchor_x: 0.5; anchor_y: 0.5; pivot_x: 0.5; pivot_y: 0.5; x: 0px; y: 0px;"} else {"width: 152px; height: 60px; anchor_x: 0; anchor_y: 0; pivot_x: 0; pivot_y: 0; x: 18px; y: 64px;"});
        self.props(
            ctx,
            "loading",
            format!("visible: {};", phase == Phase::Loading),
        );
        for lane in 0..5 {
            let p = players.iter().find(|p| p.lane == lane);
            let node = format!("lane{lane}");
            self.props(
                ctx,
                &node,
                format!(
                    "visible: {}; ignore_event: {}; color: #{};",
                    preparing && phase == Phase::Ready,
                    phase != Phase::Ready || p.is_none(),
                    if p.is_some_and(|p| selected == Some(p.player)) {
                        "ffd700ff"
                    } else {
                        "555555ff"
                    }
                ),
            );
            if let Some(p) = p {
                self.portrait(ctx, &format!("{node}.portrait"), &p.champion, 56.);
            }
        }
        self.props(ctx, "manual", format!("visible: {};", !preparing));
        if !preparing {
            if let Some(p) = players.iter().find(|p| Some(p.player) == selected) {
                self.portrait(ctx, "selected", &p.champion, 36.);
            }
        } else {
            self.props(ctx, "selected", "visible: false;".into());
        }
        let glyph = if phase == Phase::Running {
            "pause"
        } else if preparing {
            "play_dark"
        } else {
            "play"
        };
        self.props(
            ctx,
            "primary.glyph",
            format!("source: \"asset/lt_direct_control_probe/ui/{glyph}\";"),
        );
        for (node, x, color) in [
            (
                "primary",
                if preparing { 150 } else { 52 },
                if preparing { "ffd700ff" } else { "333333ff" },
            ),
            ("release", if preparing { 206 } else { 104 }, "333333ff"),
        ] {
            self.props(
                ctx,
                node,
                format!(
                    "x: {x}px; y: {}px; visible: {}; ignore_event: {}; color: #{color};",
                    if preparing { 94 } else { 4 },
                    phase != Phase::Loading,
                    phase == Phase::Loading
                ),
            );
        }
        for (node, x) in [
            ("start_key", if preparing { 140 } else { 42 }),
            ("release_key", if preparing { 202 } else { 96 }),
        ] {
            self.props(
                ctx,
                node,
                format!(
                    "x: {x}px; y: {}px; visible: {};",
                    if preparing { 137 } else { 47 },
                    phase != Phase::Loading && (node != "start_key" || phase != Phase::Running)
                ),
            );
        }
        ctx.ui_node_rect(PATH)
            .map(|(x, y, w, h)| vec![Rect { x, y, w, h }])
            .unwrap_or_default()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_events_expire_on_start_or_next_set() {
        let ui = SessionUi::default();
        ui.events.lock().unwrap().choice = Some(((1, 33, 1), 1, 3));
        assert_eq!(
            ui.take_choice(Some(Phase::Running), Some((1, 33, 1)), 1),
            None
        );
        ui.events.lock().unwrap().choice = Some(((1, 33, 1), 1, 3));
        assert_eq!(
            ui.take_choice(Some(Phase::Ready), Some((2, 33, 2)), 1),
            None
        );
        ui.events.lock().unwrap().choice = Some(((2, 33, 2), 2, 4));
        assert_eq!(
            ui.take_choice(Some(Phase::Ready), Some((2, 33, 2)), 3),
            None
        );
        ui.events.lock().unwrap().choice = Some(((2, 33, 2), 3, 4));
        assert_eq!(
            ui.take_choice(Some(Phase::Ready), Some((2, 33, 2)), 3),
            Some(4)
        );
    }
}
