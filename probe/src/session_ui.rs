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
/// In-match strip controls: Pause, AI, Camera, Settings, vision group, Tab.
const STRIP_W: f32 = 656.;
const VISION_X: f32 = 200.;
const VISION_STEP: f32 = 46.;
const TAB_X: f32 = 586.;
/// Match the three resting button hit rectangles at any HUD scale. Selection
/// is deliberately absent: hover belongs to the pointer's segment only.
fn vision_hover_index(
    strip: bool,
    cursor: Option<(f32, f32)>,
    bounds: Option<Rect>,
) -> Option<usize> {
    let (cursor, bounds) = (cursor?, bounds?);
    if !strip || !bounds.valid() {
        return None;
    }
    let sx = bounds.w / STRIP_W;
    let sy = bounds.h / 56.;
    (0..3).find(|i| {
        Rect {
            x: bounds.x + (VISION_X + VISION_STEP * *i as f32) * sx,
            y: bounds.y + 6. * sy,
            w: 44. * sx,
            h: 44. * sy,
        }
        .contains(cursor)
    })
}
#[derive(Default)]
struct Events {
    choice: Option<(MatchKey, u64, usize)>,
    settings_open: bool,
    team: bool,
    vision: Option<crate::camera::Vision>,
}
fn button(s: &mut String, name: &str, glyph: &str, x: usize, y: usize) {
    s.push_str(&format!("#{name}:color_icon_button {{ x: {x}px; y: {y}px; width: 48px; height: 44px; z: 1102; visible: false; btn: {{ color: #4b4a49ff; back_color: #1c1a18ff; stroke: 1; rounding: Uniform {{ rounding: 2; }} }} text: {{ text: \"\"; }}"));
    s.push_str(&ui_graphics::image("glyph", glyph, 11, 9, 26, 1105));
    s.push_str("}\n");
}
fn template() -> String {
    let mut s=String::from("lt_session_controls:empty { width: 456px; height: 80px; anchor_x: 0.5; anchor_y: 1; pivot_x: 0.5; pivot_y: 1; y: -62px; z: 1100; ignore_event: true;\n");
    for lane in 0..5 {
        s.push_str(&format!("#lane{lane}:color_icon_button {{ x: {}px; y: 4px; width: 72px; height: 72px; z: 1102; btn: {{ color: #4b4a49ff; back_color: #1c1a18ff; stroke: 1; rounding: Uniform {{ rounding: 2; }} }} text: {{ text: \"\"; }} #portrait:image {{ x: 0px; y: 0px; anchor_x: 0.5; anchor_y: 0.5; pivot_x: 0.5; pivot_y: 0.5; width: 56px; height: 56px; z: 1104; sample_linear: false; ignore_event: true; visible: false; }} }}\n",lane*80));
    }
    s.push_str(&ui_graphics::image(
        "loading",
        "hourglass",
        212,
        24,
        32,
        1104,
    ));
    for (name, glyph) in [
        ("primary", "ef_play"),
        ("release", "ef_cpu"),
        ("camera", "ef_camera"),
        ("settings", "ef_settings"),
        ("tab", "ef_list"),
        ("vision_own", "ef_eye"),
        ("vision_other", "ef_eye"),
        ("vision_all", "ef_eye"),
    ] {
        button(&mut s, name, glyph, 0, 0);
    }
    s.push_str(r##"#tab_key:color { x: 592px; y: 17px; width: 30px; height: 22px; z: 1104; color: #eeececff; ignore_event: true; rounding: Uniform { rounding: 2; } #text:label { @"asset/base/style/main#bold_label"; width: 100%; height: 100%; size: 13; align_x: Center; align_y: Center; color: #393939ff; text: "Tab"; ignore_event: true; z: 1105; } }
    #rule_controls:color { x: 194px; y: 17px; width: 1px; height: 22px; z: 1101; color: #4b4a49ff; ignore_event: true; }
    #rule_vision:color { x: 342px; y: 17px; width: 1px; height: 22px; z: 1101; color: #4b4a49ff; ignore_event: true; }
    #vision_track:color { x: 200px; y: 6px; width: 136px; height: 44px; z: 1101; color: #3a3837ff; rounding: Uniform { rounding: 3; } ignore_event: true; }
    #vision_indicator:color { x: 202px; y: 8px; width: 40px; height: 40px; z: 1102; color: #eeececff; rounding: Uniform { rounding: 2; } ignore_event: true; }
    #tooltip:color { x: 0px; y: -68px; width: 310px; height: 56px; z: 1200; color: #1e1e1dd9; ignore_event: true; visible: false; rounding: Uniform { rounding: 3; } #text:label { @"asset/base/style/main#label"; x: 12px; y: 8px; width: 286px; height: 40px; size: 20; color: #ffffffff; ignore_event: true; z: 1201; } }
    }"##);
    for (node, color) in [("vision_own", "53b8e4ff"), ("vision_other", "ff642eff")] {
        s=s.replace(&format!("#{node}:color_icon_button {{"),&format!("#{node}:color_icon_button {{ #mark:color {{ x: 6px; y: 13px; width: 3px; height: 18px; z: 1104; color: #{color}; ignore_event: true; }}"));
    }
    for node in ["vision_own", "vision_other", "vision_all"] {
        s = s.replace(
            &format!("#{node}:color_icon_button {{"),
            &format!("#{node}:color_icon_button {{ #hover:color {{ x: 2px; y: 2px; width: 40px; height: 40px; z: 1103; color: #00000000; ignore_event: true; rounding: Uniform {{ rounding: 2; }} }}"),
        );
    }
    s = s.replace("#tab_key:color", "#take_label:label { @\"asset/base/style/main#bold_label\"; x: 52px; y: 6px; width: 204px; height: 44px; size: 18; align_y: Center; color: #eeececff; text: \"Take control · F11\"; visible: false; ignore_event: true; z: 1106; } #tab_key:color");
    crate::hud_style::fonts(s)
}
#[derive(Default)]
pub struct SessionUi {
    spawn_at: Option<Instant>,
    registered: bool,
    events: Arc<Mutex<Events>>,
    cache: HashMap<String, String>,
    portraits: HashMap<String, String>,
    attempted: HashMap<String, (String, Instant)>,
    motion: crate::hud_motion::Motion,
    tip: Option<(String, Instant)>,
}
impl SessionUi {
    pub fn take_settings(&self) -> bool {
        self.events
            .lock()
            .is_ok_and(|mut e| std::mem::take(&mut e.settings_open))
    }
    pub fn team_open(&self) -> bool {
        self.events.lock().is_ok_and(|e| e.team)
    }
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
        if crate::hud_motion::properties(ctx, &mut self.cache, &format!("{PATH}.{node}"), &text) {
            self.cache.insert(node.into(), text);
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn animate(
        &mut self,
        ctx: &mut StableClient<'_>,
        node: &str,
        rect: (f32, f32, f32, f32),
        preparing: bool,
        cursor: Option<(f32, f32)>,
        pressed: bool,
        accent: bool,
        visible: bool,
        glyph: &str,
        gsize: f32,
    ) {
        let (x, y, w, h) = rect;
        let hover = visible
            && cursor.is_some_and(|p| {
                ctx.ui_node_rect(PATH).is_some_and(|(rx, ry, rw, rh)| {
                    let sx = rw / if preparing { 456. } else { STRIP_W };
                    let sy = rh / if preparing { 80. } else { 56. };
                    Rect {
                        x: rx + x * sx,
                        y: ry + y * sy,
                        w: w * sx,
                        h: h * sy,
                    }
                    .contains(p)
                })
            });
        let hover = self
            .motion
            .tonal(&format!("hover_{node}"), if hover { 1. } else { 0. }, 0.1);
        let press = self.motion.tonal(
            &format!("press_{node}"),
            if hover > 0.5 && pressed { 1. } else { 0. },
            0.083,
        );
        let selected =
            self.motion
                .value(&format!("select_{node}"), if accent { 1. } else { 0. }, 0.1);
        let primary = node == "primary" && preparing;
        let base = if preparing { 0x4b4a49ff } else { 0x4b4a4900 };
        let border = crate::hud_motion::color(base, 0xfdee00ff, selected);
        let border = crate::hud_motion::color(
            u32::from_str_radix(&border, 16).unwrap_or(base),
            0xffffffff,
            hover * (1. - selected),
        );
        let background = if primary {
            crate::hud_motion::color(0xfdee00ff, 0xc9be00ff, hover)
        } else {
            crate::hud_motion::color(0x1c1a18ff, 0x3a3837ff, hover)
        };
        let background = crate::hud_motion::color(
            u32::from_str_radix(&background, 16).unwrap_or(0x1c1a18ff),
            if primary { 0xaea400ff } else { 0x5b5b5bff },
            press,
        );
        let stroke = if accent && !primary { 2 } else { 1 };
        let offset = 0.;
        self.props(ctx,node,format!("x: {x}px; y: {:.2}px; width: {w}px; height: {h}px; visible: {visible}; ignore_event: {}; btn: {{ color: #{border}; back_color: #{background}; stroke: {stroke}; }}",y+offset,!visible));
        if !glyph.is_empty() {
            let size = gsize;
            self.props(ctx,&format!("{node}.glyph"),format!("source: \"asset/lt_direct_control_probe/ui/{glyph}\"; x: {:.2}px; y: {:.2}px; width: {size:.2}px; height: {size:.2}px;",(w-size)/2.,(h-size)/2.));
        } else {
            self.props(
                ctx,
                &format!("{node}.portrait"),
                // The host resolves a fitted face rectangle. Keep its size;
                // animation must not stretch that rectangle into a square.
                "x: 0px; y: 0px;".into(),
            );
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
    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &mut self,
        ctx: &mut StableClient<'_>,
        timing: &Arc<NativeTiming>,
        players: &[PlayerIdentity],
        selected: Option<usize>,
        camera: &Arc<crate::camera::CameraControl>,
        _settings: &crate::settings::Settings,
        cursor: Option<(f32, f32)>,
        pressed: bool,
        log: &Logger,
    ) -> Vec<Rect> {
        let Some(
            phase @ (Phase::Loading | Phase::Ready | Phase::Running | Phase::Paused | Phase::Ai),
        ) = timing.ui_phase()
        else {
            if ctx.ui_exists(PATH) {
                ctx.ui_set_visible(PATH, false);
            }
            self.motion.reset();
            if let Ok(mut e) = self.events.lock() {
                e.team = false;
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
            let owner = camera.clone();
            ctx.ui_register_path_events(&format!("{PATH}.camera"), move |ctx| {
                if ctx
                    .ui_current_event()
                    .is_some_and(|e| e.kind == Some(UiEventKindV1::Click))
                {
                    owner.request_toggle();
                }
            });
            let events = self.events.clone();
            ctx.ui_register_path_events(&format!("{PATH}.settings"), move |ctx| {
                if ctx
                    .ui_current_event()
                    .is_some_and(|e| e.kind == Some(UiEventKindV1::Click))
                {
                    if let Ok(mut e) = events.lock() {
                        e.settings_open = true;
                    }
                }
            });
            let events = self.events.clone();
            ctx.ui_register_path_events(&format!("{PATH}.tab"), move |ctx| {
                if ctx
                    .ui_current_event()
                    .is_some_and(|e| e.kind == Some(UiEventKindV1::Click))
                {
                    if let Ok(mut e) = events.lock() {
                        e.team = !e.team;
                    }
                }
            });
            for (node, vision) in [
                ("vision_own", crate::camera::Vision::Own),
                ("vision_other", crate::camera::Vision::Other),
                ("vision_all", crate::camera::Vision::All),
            ] {
                let events = self.events.clone();
                ctx.ui_register_path_events(&format!("{PATH}.{node}"), move |ctx| {
                    if ctx
                        .ui_current_event()
                        .is_some_and(|e| e.kind == Some(UiEventKindV1::Click))
                    {
                        if let Ok(mut e) = events.lock() {
                            e.vision = Some(vision);
                        }
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

        let properties = if preparing {
            format!("width: 456px; height: 80px; anchor_x: 0.5; anchor_y: 1; pivot_x: 0.5; pivot_y: 1; y: {:.2}px; x: 0px;",-62.)
        } else {
            format!("width: {STRIP_W}px; height: 56px; anchor_x: 0; anchor_y: 1; pivot_x: 0; pivot_y: 1; x: 8px; y: 0px;")
        };
        crate::hud_motion::properties(ctx, &mut self.cache, PATH, &properties);
        self.props(
            ctx,
            "loading",
            format!("visible: {};", phase == Phase::Loading),
        );
        for lane in 0..5 {
            let p = players.iter().find(|p| p.lane == lane);
            self.props(
                ctx,
                &format!("lane{lane}"),
                format!(
                    "visible: {}; ignore_event: {};",
                    preparing && phase == Phase::Ready,
                    phase != Phase::Ready || p.is_none()
                ),
            );
            if let Some(p) = p {
                self.portrait(ctx, &format!("lane{lane}.portrait"), &p.champion, 56.);
            }
        }
        let ai = phase == Phase::Ai;
        if preparing || ai {
            if let Ok(mut e) = self.events.lock() {
                e.team = false;
                e.settings_open = false;
            }
        }
        let strip = !preparing && !ai;
        self.props(ctx, "take_label", format!("visible: {ai};"));
        if let Some(vision) = self
            .events
            .lock()
            .ok()
            .and_then(|mut e| e.vision.take())
            .filter(|_| strip)
        {
            camera.set_vision(vision);
        }
        // Vision: one segmented group on the strip (formerly inside the "..." menu).
        for node in ["vision_track", "rule_controls", "rule_vision"] {
            self.props(ctx, node, format!("visible: {strip};"));
        }
        let vision = camera.vision();
        let vision_position = self
            .motion
            .value("vision_indicator", vision.index() as f32, 0.167);
        let hovered_vision = vision_hover_index(
            strip,
            cursor,
            ctx.ui_node_rect(PATH)
                .map(|(x, y, w, h)| Rect { x, y, w, h }),
        );
        self.props(
            ctx,
            "vision_indicator",
            format!(
                "visible: {strip}; x: {:.2}px; color: #eeececff;",
                VISION_X + 2. + VISION_STEP * vision_position,
            ),
        );
        for (i, node) in ["vision_own", "vision_other", "vision_all"]
            .into_iter()
            .enumerate()
        {
            self.props(ctx,node,format!("x: {}px; y: 6px; width: 44px; height: 44px; visible: {strip}; ignore_event: {}; btn: {{ color: #00000000; back_color: #00000000; stroke: 0; }}",VISION_X+VISION_STEP*i as f32,!strip));
            let selected = self.motion.tonal(
                &format!("vision_sel{i}"),
                f32::from(vision.index() == i),
                0.1,
            );
            let hover = self.motion.tonal(
                &format!("vision_hover{i}"),
                f32::from(hovered_vision == Some(i)),
                0.1,
            );
            let shade = crate::hud_motion::color(0x5b5b5bff, 0xb8b6b5ff, selected);
            self.props(
                ctx,
                &format!("{node}.hover"),
                format!(
                    "visible: {strip}; color: #{}{:02x};",
                    &shade[..6],
                    (255. * hover) as u8,
                ),
            );
            self.props(
                ctx,
                &format!("{node}.glyph"),
                format!(
                    "x: 10px; y: 10px; width: 24px; height: 24px; color: #{};",
                    crate::hud_motion::color(0xb8b6b5ff, 0x393939ff, selected)
                ),
            );
        }
        let glyph = if phase == Phase::Running {
            "ef_pause"
        } else {
            "ef_play"
        };
        for (node, x, y, w, h, visible, accent, glyph, gsize) in [
            (
                "primary",
                if preparing { 408. } else { 0. },
                if preparing { 16. } else { 6. },
                if preparing {
                    48.
                } else if ai {
                    260.
                } else {
                    44.
                },
                if preparing { 48. } else { 44. },
                phase != Phase::Loading,
                preparing || ai,
                glyph,
                26.,
            ),
            ("release", 48., 6., 44., 44., strip, false, "ef_cpu", 26.),
            (
                "camera",
                96.,
                6.,
                44.,
                44.,
                strip,
                camera.locked(),
                "ef_camera",
                26.,
            ),
            (
                "settings",
                144.,
                6.,
                44.,
                44.,
                strip,
                false,
                "ef_settings",
                26.,
            ),
            ("tab", TAB_X, 6., 68., 44., strip, false, "ef_list", 26.),
        ] {
            self.animate(
                ctx,
                node,
                (x, y, w, h),
                preparing,
                cursor,
                pressed,
                accent,
                visible,
                glyph,
                gsize,
            );
        }
        self.props(ctx, "tab_key", format!("visible: {strip};"));
        if ai {
            self.props(ctx, "primary.glyph", "x: 9px;".into());
        }
        self.props(
            ctx,
            "primary.glyph",
            format!(
                "color: #{};",
                if preparing { "1c1a18ff" } else { "eeececff" }
            ),
        );
        self.props(
            ctx,
            "camera.glyph",
            format!(
                "color: #{};",
                if camera.locked() {
                    "fdee00ff"
                } else {
                    "cbc9c7ff"
                }
            ),
        );
        self.props(
            ctx,
            "tab.glyph",
            "x: 38px; y: 9px; width: 26px; height: 26px;".into(),
        );
        for lane in 0..5 {
            let p = players.iter().find(|p| p.lane == lane);
            let node = format!("lane{lane}");
            self.animate(
                ctx,
                &node,
                (lane as f32 * 80., 4., 72., 72.),
                true,
                cursor,
                pressed,
                p.is_some_and(|p| selected == Some(p.player)),
                preparing && phase == Phase::Ready,
                "",
                56.,
            );
        }
        let mut tip = None;
        if let Some(cursor) = cursor {
            for (node, text) in [
                (
                    "primary",
                    if preparing {
                        "Start · F11"
                    } else if ai {
                        "Take control · F11"
                    } else {
                        if phase == Phase::Paused {
                            "Resume · F11"
                        } else {
                            "Pause · F11"
                        }
                    },
                ),
                ("camera", "Lock camera · Y\nHold Space to follow"),
                ("tab", "Team details · Tab"),
                ("release", "Return control to AI · F12"),
                ("settings", "Settings"),
                ("vision_own", "Own-team vision"),
                ("vision_other", "Opposing-team vision"),
                ("vision_all", "All-team vision"),
            ] {
                let path = format!("{PATH}.{node}");
                if ctx.ui_visible(&path) == Some(true)
                    && ctx
                        .ui_node_rect(&path)
                        .is_some_and(|(x, y, w, h)| Rect { x, y, w, h }.contains(cursor))
                {
                    tip = Some((text.to_owned(), ctx.ui_node_rect(&path).unwrap()));
                    break;
                }
            }
            if preparing && tip.is_none() {
                for p in players {
                    if ctx
                        .ui_node_rect(&format!("{PATH}.lane{}", p.lane))
                        .is_some_and(|(x, y, w, h)| Rect { x, y, w, h }.contains(cursor))
                    {
                        tip = Some((
                            crate::tooltips::champion_name(ctx, &p.champion),
                            ctx.ui_node_rect(&format!("{PATH}.lane{}", p.lane)).unwrap(),
                        ));
                        break;
                    }
                }
            }
        }
        if let Some((text, (tx, ty, tw, _))) = tip {
            if self.tip.as_ref().is_none_or(|(old, _)| old != &text) {
                self.tip = Some((text.clone(), Instant::now()));
            }
            let fade = self.tip.as_ref().map_or(1., |(_, at)| {
                crate::hud_motion::easing((at.elapsed().as_secs_f32() / 0.117).min(1.), true)
            });
            let width =
                (crate::hud_style::width(&text.replace('\n', " "), 20.) + 28.).clamp(120., 440.);
            let (text, lines) = crate::hud_style::wrap(&text, 20., width - 24.);
            let height = lines * 26 + 16;
            let (bx, by, bw, bh) = ctx.ui_node_rect(PATH).unwrap_or((0., 0., 1920., 1080.));
            let sx = if preparing { 456. } else { STRIP_W } / bw;
            let sy = if preparing { 80. } else { 56. } / bh;
            let x =
                ((tx + tw / 2. - bx) * sx - width / 2.).clamp(-bx * sx, 1920. - bx * sx - width);
            let y = (ty - by) * sy - height as f32 - 12.;
            self.props(ctx,"tooltip",format!("visible: true; x: {x:.2}px; y: {y:.2}px; width: {width:.2}px; height: {height}px; color: #1e1e1d{:02x};",(217.*fade)as u8));
            self.props(
                ctx,
                "tooltip.text",
                format!(
                    "width: {}px; height: {}px; color: #ffffff{:02x};",
                    width - 24.,
                    height - 16,
                    (255. * fade) as u8
                ),
            );
            ctx.ui_set_text(&format!("{PATH}.tooltip.text"), &text);
        } else {
            self.tip = None;
            self.props(ctx, "tooltip", "visible: false;".into());
        }
        let mut bounds = Vec::new();
        if let Some((x, y, w, h)) = ctx.ui_node_rect(PATH) {
            bounds.push(Rect {
                x,
                y,
                w: if ai { w * 260. / STRIP_W } else { w },
                h,
            });
        }
        bounds
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vision_hover_tracks_each_segment_at_hud_scale_and_ignores_gaps() {
        for scale in [0.5, 1., 1.5] {
            let bounds = Rect {
                x: 100.,
                y: 700.,
                w: STRIP_W * scale,
                h: 56. * scale,
            };
            for i in 0..3 {
                let p = (
                    bounds.x + (VISION_X + VISION_STEP * i as f32 + 22.) * scale,
                    bounds.y + 28. * scale,
                );
                assert_eq!(vision_hover_index(true, Some(p), Some(bounds)), Some(i));
                assert_eq!(vision_hover_index(false, Some(p), Some(bounds)), None);
            }
            let gap = (bounds.x + (VISION_X + 45.) * scale, bounds.y + 28. * scale);
            assert_eq!(vision_hover_index(true, Some(gap), Some(bounds)), None);
            assert_eq!(
                vision_hover_index(true, Some((bounds.x, bounds.y)), Some(bounds)),
                None
            );
            assert_eq!(vision_hover_index(true, None, Some(bounds)), None);
        }
    }
    #[test]
    fn selection_events_expire_on_start_or_next_set() {
        if let Ok(dir) = std::env::var("LT_HUD_EXPORT_DIR") {
            std::fs::write(std::path::Path::new(&dir).join("session.ui"), template()).unwrap();
        }
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
    #[test]
    fn strip_holds_controls_in_the_requested_order_without_a_more_menu() {
        let t = template();
        assert!(!t.contains("#more:") && !t.contains("#menu:") && !t.contains("ef_more"));
        assert!(t.contains("#settings:color_icon_button") && t.contains("ef_settings"));
        // Pause 0, AI 48, camera 96, settings 144, vision 200-336, Tab after.
        const {
            assert!(VISION_X >= 144. + 44. + 8.);
            assert!(VISION_X + 2. * VISION_STEP + 44. <= 342.);
            assert!(TAB_X + 68. <= STRIP_W);
        }
    }
}
