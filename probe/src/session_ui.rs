//! Portrait-only preparation; handlers enqueue phase/key-bound choices.
use crate::{
    camera::Rect,
    native_timing::{MatchKey, NativeTiming, Phase},
    own_selection::PlayerIdentity,
    ui_graphics,
    ui_writes::{NodeId, PropCache},
    Logger,
};
use mod_api_stable::{StableClient, UiEventKindV1};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
pub(crate) const PATH: &str = "ingame.lt_session_controls";
/// In-match strip controls: Pause, AI, Camera, Settings, vision group, Tab.
const STRIP_W: f32 = 656.;
const VISION_X: f32 = 200.;
const VISION_STEP: f32 = 46.;
const TAB_X: f32 = 586.;
/// Champion swap: a button floating above the strip's left end; its column
/// of teammate portraits unfolds upward from it, in lane order from the
/// top: top, jungle, mid, bottom, support (support next to the button).
const SWAP_TILE: f32 = 56.;
const SWAP_GAP: f32 = 4.;
const SWAP_Y: f32 = -(SWAP_TILE + 8.);
/// Tile `lane` of the column when it is `unfold` (0..1) open, in strip units.
fn swap_tile(lane: usize, unfold: f32) -> (f32, f32, f32, f32) {
    let above = (5 - lane.min(4)) as f32;
    let y = SWAP_Y - above * (SWAP_TILE + SWAP_GAP) * unfold;
    (0., y, SWAP_TILE, SWAP_TILE)
}
/// Phases in which the controlled champion may be changed.
pub fn swappable(phase: Option<Phase>) -> bool {
    matches!(
        phase,
        Some(Phase::Ready | Phase::Running | Phase::Paused | Phase::Ai)
    )
}
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
    /// The swap column is open (in a match).
    swap: bool,
}
fn button(s: &mut String, name: &str, glyph: &str, x: usize, y: usize) {
    let bg = crate::ui_theme::hex(0xff);
    s.push_str(&format!("#{name}:color_icon_button {{ x: {x}px; y: {y}px; width: 48px; height: 44px; z: 1102; visible: false; btn: {{ color: #~4b4a49ff; back_color: #{bg}; stroke: 1; rounding: Uniform {{ rounding: 2; }} }} text: {{ text: \"\"; }}"));
    s.push_str(&ui_graphics::image("glyph", glyph, 11, 9, 26, 1105));
    s.push_str("}\n");
}
#[cfg(test)]
pub(crate) fn template_for_test() -> String {
    template()
}
fn template() -> String {
    let bg = crate::ui_theme::hex(0xff);
    let mut s=String::from("lt_session_controls:empty { width: 512px; height: 80px; anchor_x: 0.5; anchor_y: 1; pivot_x: 0.5; pivot_y: 1; y: -62px; z: 1100; ignore_event: true;\n");
    for lane in 0..5 {
        s.push_str(&format!("#lane{lane}:color_icon_button {{ x: {}px; y: 4px; width: 72px; height: 72px; z: 1102; btn: {{ color: #~4b4a49ff; back_color: #{bg}; stroke: 1; rounding: Uniform {{ rounding: 2; }} }} text: {{ text: \"\"; }} #portrait:image {{ x: 0px; y: 0px; anchor_x: 0.5; anchor_y: 0.5; pivot_x: 0.5; pivot_y: 0.5; width: 56px; height: 56px; z: 1104; sample_linear: false; ignore_event: true; visible: false; }} }}\n",lane*80));
    }
    s.push_str(&ui_graphics::image(
        "loading",
        "hourglass",
        240,
        24,
        32,
        1104,
    ));
    for (name, glyph) in [
        ("primary", "ef_play"),
        ("watch", "ef_eye"),
        ("release", "ef_cpu"),
        ("camera", "ef_camera"),
        ("settings", "ef_settings"),
        ("tab", "ef_list"),
        ("vision_own", "ef_eye"),
        ("vision_other", "ef_eye"),
        ("vision_all", "ef_eye"),
        ("swap", "ef_swap"),
    ] {
        button(&mut s, name, glyph, 0, 0);
    }
    s.push_str(r##"#tab_key:color { x: 592px; y: 17px; width: 30px; height: 22px; z: 1104; color: #eeececff; ignore_event: true; rounding: Uniform { rounding: 2; } #text:label { @"asset/base/style/main#bold_label"; width: 100%; height: 100%; size: 13; align_x: Center; align_y: Center; color: #393939ff; text: "Tab"; ignore_event: true; z: 1105; } }
    #rule_controls:color { x: 194px; y: 17px; width: 1px; height: 22px; z: 1101; color: #~4b4a49ff; ignore_event: true; }
    #rule_vision:color { x: 342px; y: 17px; width: 1px; height: 22px; z: 1101; color: #~4b4a49ff; ignore_event: true; }
    #vision_track:color { x: 200px; y: 6px; width: 136px; height: 44px; z: 1101; color: #~3a3837ff; rounding: Uniform { rounding: 3; } ignore_event: true; }
    #vision_indicator:color { x: 202px; y: 8px; width: 40px; height: 40px; z: 1102; color: #eeececff; rounding: Uniform { rounding: 2; } ignore_event: true; }
    #tooltip:color { x: 0px; y: -68px; width: 310px; height: 56px; z: 1200; color: #~1e1e1dd9; ignore_event: true; visible: false; rounding: Uniform { rounding: 3; } #text:label { @"asset/base/style/main#label"; x: 12px; y: 8px; width: 286px; height: 40px; size: 20; color: #ffffffff; ignore_event: true; z: 1201; } }
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
    // motion: B8 (inside the root, before the Tab key cap)
    s = s.replace(
        "#tab_key:color",
        &format!(
            "{}#tab_key:color",
            crate::hud_motion_pass::b8_nodes(SWAP_Y + SWAP_TILE, SWAP_TILE)
        ),
    );
    s = s.replace("#tab_key:color", "#take_label:label { @\"asset/base/style/main#bold_label\"; x: 52px; y: 6px; width: 204px; height: 44px; size: 18; align_y: Center; color: #eeececff; text: \"Take control · F11\"; visible: false; ignore_event: true; z: 1106; } #tab_key:color");
    crate::hud_style::fonts(s)
}
#[derive(Default)]
pub struct SessionUi {
    spawn_at: Option<Instant>,
    registered: bool,
    events: Arc<Mutex<Events>>,
    cache: crate::ui_writes::PropCache,
    portraits: HashMap<String, String>,
    attempted: HashMap<String, (String, Instant)>,
    motion: crate::hud_motion::Motion,
    tip: Option<(String, Instant)>,
    hover: crate::ui_sound::Hover,
    // motion: pass
    /// The HUD motion pass (hud_motion_pass.rs).
    pass: crate::hud_motion_pass::Pass,
    /// The strip's nodes in `cache` (1.6.0).
    nodes: Option<std::sync::Arc<Nodes>>,
}
/// Glyph images the strip's buttons show.
const GLYPHS: [&str; 8] = [
    "ef_pause",
    "ef_play",
    "ef_eye",
    "ef_cpu",
    "ef_camera",
    "ef_settings",
    "ef_list",
    "ef_swap",
];
/// Write sites in the strip's cache: `VIS` is always `visible: {on};`.
const VIS: u8 = 0;
const S1: u8 = 1;
const S2: u8 = 2;
const S3: u8 = 3;
/// One button (or swap tile) of the strip.
struct Button {
    name: &'static str,
    root: NodeId,
    glyph: NodeId,
    portrait: NodeId,
    hover_key: String,
    press_key: String,
    select_key: String,
}
/// The strip's nodes and paths, found once.
struct Nodes {
    root: NodeId,
    loading: NodeId,
    take_label: NodeId,
    rules: [NodeId; 3],
    vision_indicator: NodeId,
    vision: [(NodeId, NodeId, NodeId); 3],
    tab_key: NodeId,
    primary_glyph: NodeId,
    camera_glyph: NodeId,
    tab_glyph: NodeId,
    tooltip: NodeId,
    tooltip_text: NodeId,
    b8: NodeId,
    /// primary, watch, release, camera, settings, tab, swap, lane0..lane4.
    buttons: Vec<Button>,
    /// Each lane's portrait node name and path (`lane{n}.portrait`).
    portraits: [(String, String); 5],
    /// Each lane tile's path.
    lane_paths: [String; 5],
    /// The tooltip's candidates' paths, in `TIPS` order.
    tip_paths: Vec<String>,
    clickable: [String; 15],
}
/// The tooltip's candidates, in search order.
const TIPS: [&str; 9] = [
    "primary",
    "camera",
    "watch",
    "tab",
    "release",
    "settings",
    "vision_own",
    "vision_other",
    "vision_all",
];
const BUTTONS: [&str; 12] = [
    "primary", "watch", "release", "camera", "settings", "tab", "swap", "lane0", "lane1", "lane2",
    "lane3", "lane4",
];
impl Nodes {
    fn new(c: &mut PropCache) -> Self {
        let root = c.node(PATH);
        let mut id = |node: &str| c.node(&format!("{PATH}.{node}"));
        let buttons = BUTTONS
            .iter()
            .map(|name| Button {
                name,
                root: id(name),
                glyph: id(&format!("{name}.glyph")),
                portrait: id(&format!("{name}.portrait")),
                hover_key: format!("hover_{name}"),
                press_key: format!("press_{name}"),
                select_key: format!("select_{name}"),
            })
            .collect();
        let vision = ["vision_own", "vision_other", "vision_all"]
            .map(|n| (id(n), id(&format!("{n}.hover")), id(&format!("{n}.glyph"))));
        Self {
            root,
            loading: id("loading"),
            take_label: id("take_label"),
            rules: ["vision_track", "rule_controls", "rule_vision"].map(&mut id),
            vision_indicator: id("vision_indicator"),
            vision,
            tab_key: id("tab_key"),
            primary_glyph: id("primary.glyph"),
            camera_glyph: id("camera.glyph"),
            tab_glyph: id("tab.glyph"),
            tooltip: id("tooltip"),
            tooltip_text: id("tooltip.text"),
            b8: id(crate::hud_motion_pass::B8_NODE),
            buttons,
            portraits: std::array::from_fn(|lane| {
                let node = format!("lane{lane}.portrait");
                let path = format!("{PATH}.{node}");
                (node, path)
            }),
            lane_paths: std::array::from_fn(|lane| format!("{PATH}.lane{lane}")),
            tip_paths: TIPS.iter().map(|n| format!("{PATH}.{n}")).collect(),
            clickable: CLICKABLE.map(|node| format!("{PATH}.{node}")),
        }
    }
    fn button(&self, name: &str) -> &Button {
        self.buttons
            .iter()
            .find(|b| b.name == name)
            .expect("a strip button")
    }
}
/// Two f32 values in one key part.
fn pair(a: f32, b: f32) -> u64 {
    u64::from(a.to_bits()) << 32 | u64::from(b.to_bits())
}
fn unpair(v: u64) -> (f32, f32) {
    (f32::from_bits((v >> 32) as u32), f32::from_bits(v as u32))
}
/// The clickable controls, for the hover sound.
const CLICKABLE: [&str; 15] = [
    "primary",
    "release",
    "watch",
    "camera",
    "settings",
    "tab",
    "vision_own",
    "vision_other",
    "vision_all",
    "swap",
    "lane0",
    "lane1",
    "lane2",
    "lane3",
    "lane4",
];
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
        (swappable(phase) && key == Some(bound) && generation == bound_generation).then_some(lane)
    }
    /// Esc, or a swap made by key: the column folds away.
    pub fn close_swap(&self) {
        if let Ok(mut e) = self.events.lock() {
            e.swap = false;
        }
    }
    /// The swap button's click (window tests cannot fake the game's event).
    #[cfg(test)]
    pub(crate) fn open_swap_for_test(&self) {
        self.events.lock().unwrap().swap = true;
    }
    fn vis(&mut self, ctx: &mut StableClient<'_>, id: NodeId, on: bool) {
        self.cache
            .props_keyed(ctx, id, VIS, (on,), |(on,)| format!("visible: {on};"));
    }
    #[allow(clippy::too_many_arguments)]
    fn animate(
        &mut self,
        ctx: &mut StableClient<'_>,
        b: &Button,
        rect: (f32, f32, f32, f32),
        preparing: bool,
        cursor: Option<(f32, f32)>,
        pressed: bool,
        accent: bool,
        visible: bool,
        glyph: Option<u8>,
        gsize: f32,
        root: Option<(f32, f32, f32, f32)>,
    ) {
        let node = b.name;
        let (x, y, w, h) = rect;
        let hover = visible
            && cursor.is_some_and(|p| {
                root.is_some_and(|(rx, ry, rw, rh)| {
                    let sx = rw / if preparing { 512. } else { STRIP_W };
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
            .tonal(&b.hover_key, if hover { 1. } else { 0. }, 0.1);
        let press = self.motion.tonal(
            &b.press_key,
            if hover > 0.5 && pressed { 1. } else { 0. },
            0.083,
        );
        let selected = self
            .motion
            .value(&b.select_key, if accent { 1. } else { 0. }, 0.1);
        let primary = node == "primary" && preparing;
        let base = if preparing {
            crate::ui_theme::tone(0x4b4a49ff)
        } else if node == "swap" {
            // Floating over the battlefield, not on the strip: a light frame.
            0xeeececff
        } else if node.starts_with("lane") {
            crate::ui_theme::tone(0x8a8886ff)
        } else {
            crate::ui_theme::tone(0x4b4a4900)
        };
        let border = crate::hud_motion::color_rgba(base, 0xfdee00ff, selected);
        let border = crate::hud_motion::color_rgba(border, 0xffffffff, hover * (1. - selected));
        let background = if primary {
            crate::hud_motion::color_rgba(0xfdee00ff, 0xc9be00ff, hover)
        } else {
            crate::hud_motion::color_rgba(
                crate::ui_theme::rgba(0xff),
                crate::ui_theme::tone(0x3a3837ff),
                hover,
            )
        };
        let background = crate::hud_motion::color_rgba(
            background,
            if primary {
                0xaea400ff
            } else {
                crate::ui_theme::tone(0x5b5b5bff)
            },
            press,
        );
        let stroke = if accent && !primary || node == "swap" {
            2u64
        } else {
            1
        };
        let flags = u64::from(visible) | stroke << 1;
        let colors = u64::from(border) << 32 | u64::from(background);
        self.cache.props_keyed(
            ctx,
            b.root,
            S1,
            (pair(x, y), pair(w, h), flags, colors),
            |(xy, wh, flags, colors)| {
                let ((x, y), (w, h)) = (unpair(xy), unpair(wh));
                let (visible, stroke) = (flags & 1 == 1, flags >> 1);
                let offset = 0.;
                format!("x: {x}px; y: {:.2}px; width: {w}px; height: {h}px; visible: {visible}; ignore_event: {}; btn: {{ color: #{:08x}; back_color: #{:08x}; stroke: {stroke}; }}", y + offset, !visible, colors >> 32, colors as u32)
            },
        );
        if let Some(glyph) = glyph {
            self.cache.props_keyed(
                ctx,
                b.glyph,
                S1,
                (glyph, pair(w, h), gsize),
                |(glyph, wh, size)| {
                    let (w, h) = unpair(wh);
                    format!("source: \"asset/lt_direct_control/ui/{}\"; x: {:.2}px; y: {:.2}px; width: {size:.2}px; height: {size:.2}px;", GLYPHS[glyph as usize], (w - size) / 2., (h - size) / 2.)
                },
            );
        } else {
            // The host resolves a fitted face rectangle. Keep its size;
            // animation must not stretch that rectangle into a square.
            self.cache
                .props_keyed(ctx, b.portrait, S1, (), |()| "x: 0px; y: 0px;".into());
        }
    }
    fn portrait(
        &mut self,
        ctx: &mut StableClient<'_>,
        n: &Nodes,
        lane: usize,
        name: &str,
        size: f32,
    ) {
        let (node, path) = &n.portraits[lane];
        if !name.is_empty()
            && self.portraits.get(node).is_none_or(|n| n != name)
            && self.attempted.get(node).is_none_or(|(old, at)| {
                old != name || crate::clock::since(at) >= Duration::from_secs(1)
            })
        {
            self.attempted
                .insert(node.clone(), (name.into(), crate::clock::now()));
            if ctx.ui_set_champion_icon(path, name, size, size, 2.) {
                self.portraits.insert(node.clone(), name.into());
            }
        }
        let shown = self.portraits.get(node).is_some_and(|n| n == name);
        let id = n.button(&node[..5]).portrait;
        self.vis(ctx, id, shown);
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
        bounds: &mut Vec<Rect>,
    ) {
        let Some(
            phase @ (Phase::Loading | Phase::Ready | Phase::Running | Phase::Paused | Phase::Ai),
        ) = timing.ui_phase()
        else {
            self.hover.update(false, None, None, false);
            // Write only on a change (this runs every frame).
            if ctx.ui_visible(PATH) != Some(false) && ctx.ui_exists(PATH) {
                ctx.ui_set_visible(PATH, false);
            }
            self.motion.reset();
            self.pass.reset(); // motion: pass
            if let Ok(mut e) = self.events.lock() {
                e.team = false;
            }
            return;
        };
        if !ctx.ui_exists(PATH) {
            if self
                .spawn_at
                .is_some_and(|t| crate::clock::since(t) < Duration::from_secs(1))
            {
                return;
            }
            self.spawn_at = Some(crate::clock::now());
            self.cache.clear();
            self.portraits.clear();
            self.attempted.clear();
            self.registered = false;
            if !ctx.ui_spawn_source("ingame", &crate::ui_theme::themed(&template()))
                || !ctx.ui_exists(PATH)
            {
                return;
            }
            log.write("SESSION portrait UI spawned");
        }
        if !self.registered {
            self.registered = true;
            for (node, primary) in [("primary", true), ("release", false), ("watch", false)] {
                let owner = timing.clone();
                let click_path = format!("{PATH}.{node}");
                ctx.ui_register_path_events(&click_path.clone(), move |ctx| {
                    if ctx
                        .ui_current_event()
                        .is_some_and(|e| e.kind == Some(UiEventKindV1::Click))
                        && crate::ui_click::once(&click_path)
                    {
                        owner.request_action(primary);
                    }
                });
            }
            let owner = camera.clone();
            let click_path = format!("{PATH}.camera");
            ctx.ui_register_path_events(&click_path.clone(), move |ctx| {
                if ctx
                    .ui_current_event()
                    .is_some_and(|e| e.kind == Some(UiEventKindV1::Click))
                    && crate::ui_click::once(&click_path)
                {
                    owner.request_toggle();
                }
            });
            let events = self.events.clone();
            let click_path = format!("{PATH}.settings");
            ctx.ui_register_path_events(&click_path.clone(), move |ctx| {
                if ctx
                    .ui_current_event()
                    .is_some_and(|e| e.kind == Some(UiEventKindV1::Click))
                    && crate::ui_click::once(&click_path)
                {
                    if let Ok(mut e) = events.lock() {
                        e.settings_open = true;
                    }
                }
            });
            let events = self.events.clone();
            let click_path = format!("{PATH}.tab");
            ctx.ui_register_path_events(&click_path.clone(), move |ctx| {
                if ctx
                    .ui_current_event()
                    .is_some_and(|e| e.kind == Some(UiEventKindV1::Click))
                    && crate::ui_click::once(&click_path)
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
                let click_path = format!("{PATH}.{node}");
                ctx.ui_register_path_events(&click_path.clone(), move |ctx| {
                    if ctx
                        .ui_current_event()
                        .is_some_and(|e| e.kind == Some(UiEventKindV1::Click))
                        && crate::ui_click::once(&click_path)
                    {
                        if let Ok(mut e) = events.lock() {
                            e.vision = Some(vision);
                        }
                    }
                });
            }
            let events = self.events.clone();
            let click_path = format!("{PATH}.swap");
            ctx.ui_register_path_events(&click_path.clone(), move |ctx| {
                if ctx
                    .ui_current_event()
                    .is_some_and(|e| e.kind == Some(UiEventKindV1::Click))
                    && crate::ui_click::once(&click_path)
                {
                    if let Ok(mut e) = events.lock() {
                        e.swap = !e.swap;
                    }
                }
            });
            for lane in 0..5 {
                let owner = timing.clone();
                let events = self.events.clone();
                let click_path = format!("{PATH}.lane{lane}");
                ctx.ui_register_path_events(&click_path.clone(), move |ctx| {
                    if ctx
                        .ui_current_event()
                        .is_some_and(|e| e.kind == Some(UiEventKindV1::Click))
                        && crate::ui_click::once(&click_path)
                    {
                        let (Some(key), Ok(mut events)) = (owner.match_key(), events.lock()) else {
                            return;
                        };
                        // Before Start the tiles are the chooser itself;
                        // in a match they are the open swap column.
                        let phase = owner.phase();
                        if phase == Some(Phase::Ready) || swappable(phase) && events.swap {
                            events.choice = Some((key, owner.generation(), lane));
                            events.swap = false;
                        }
                    }
                });
            }
        }
        // Write only on a change, as the HUD does (not every frame).
        crate::ui_root::show(ctx, PATH);
        let n = match &self.nodes {
            Some(n) => n.clone(),
            None => {
                let n = std::sync::Arc::new(Nodes::new(&mut self.cache));
                self.nodes = Some(n.clone());
                n
            }
        };
        self.strip(
            ctx, &n, phase, players, selected, camera, cursor, pressed, bounds,
        );
    }
    /// The strip's writes for one frame (after spawn and registration).
    #[allow(clippy::too_many_arguments)]
    fn strip(
        &mut self,
        ctx: &mut StableClient<'_>,
        n: &Nodes,
        phase: Phase,
        players: &[PlayerIdentity],
        selected: Option<usize>,
        camera: &Arc<crate::camera::CameraControl>,
        cursor: Option<(f32, f32)>,
        pressed: bool,
        bounds: &mut Vec<Rect>,
    ) {
        let preparing = matches!(phase, Phase::Loading | Phase::Ready);
        self.cache.props_keyed(ctx, n.root, S1, (preparing,), |(preparing,)| {
            if preparing {
                format!("width: 512px; height: 80px; anchor_x: 0.5; anchor_y: 1; pivot_x: 0.5; pivot_y: 1; y: {:.2}px; x: 0px;", -62.)
            } else {
                format!("width: {STRIP_W}px; height: 56px; anchor_x: 0; anchor_y: 1; pivot_x: 0; pivot_y: 1; x: 8px; y: 0px;")
            }
        });
        self.vis(ctx, n.loading, phase == Phase::Loading);
        for lane in 0..5 {
            if let Some(p) = players.iter().find(|p| p.lane == lane) {
                self.portrait(ctx, n, lane, &p.champion, 56.);
            }
        }
        let ai = phase == Phase::Ai;
        // The swap column: open on its button, closed by a press elsewhere.
        let in_match = !preparing;
        let open = in_match && self.events.lock().is_ok_and(|e| e.swap);
        let unfold = self.motion.value("swap_unfold", f32::from(open), 0.2);
        // motion: pass
        self.pass.frame();
        // motion: B8
        crate::hud_motion_pass::b8_swap_sweep(&self.pass, in_match, SWAP_TILE).write(
            ctx,
            &mut self.cache,
            n.b8,
        );
        let root = ctx.ui_node_rect(PATH);
        let scaled = |(x, y, w, h): (f32, f32, f32, f32)| {
            root.map(|(rx, ry, rw, rh)| {
                let (sx, sy) = (rw / STRIP_W, rh / 56.);
                Rect {
                    x: rx + x * sx,
                    y: ry + y * sy,
                    w: w * sx,
                    h: h * sy,
                }
            })
        };
        // At most the button and the unfolded column.
        let mut swap_bounds: [Option<Rect>; 2] = [None; 2];
        if in_match {
            swap_bounds[0] = scaled((0., SWAP_Y, SWAP_TILE, SWAP_TILE));
            if unfold > 0.02 {
                let (_, top, _, _) = swap_tile(0, unfold);
                swap_bounds[1] = scaled((0., top, SWAP_TILE, SWAP_Y - top));
            }
        }
        if open
            && pressed
            && cursor.is_some_and(|p| !swap_bounds.iter().flatten().any(|r| r.contains(p)))
        {
            self.close_swap();
        }
        if preparing || ai {
            if let Ok(mut e) = self.events.lock() {
                e.team = false;
                e.settings_open = false;
            }
        }
        let strip = !preparing && !ai;
        self.vis(ctx, n.take_label, ai);
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
        for id in n.rules {
            self.vis(ctx, id, strip);
        }
        let vision = camera.vision();
        let vision_position = self
            .motion
            .value("vision_indicator", vision.index() as f32, 0.167);
        let hovered_vision =
            vision_hover_index(strip, cursor, root.map(|(x, y, w, h)| Rect { x, y, w, h }));
        self.cache.props_keyed(
            ctx,
            n.vision_indicator,
            S1,
            (strip, vision_position),
            |(strip, position)| {
                format!(
                    "visible: {strip}; x: {:.2}px; color: #eeececff;",
                    VISION_X + 2. + VISION_STEP * position,
                )
            },
        );
        for (i, (node, hover_node, glyph_node)) in n.vision.iter().enumerate() {
            self.cache.props_keyed(ctx, *node, S1, (strip, i), |(strip, i)| {
                format!("x: {}px; y: 6px; width: 44px; height: 44px; visible: {strip}; ignore_event: {}; btn: {{ color: #00000000; back_color: #00000000; stroke: 0; }}", VISION_X + VISION_STEP * i as f32, !strip)
            });
            let selected = self.motion.tonal(
                ["vision_sel0", "vision_sel1", "vision_sel2"][i],
                f32::from(vision.index() == i),
                0.1,
            );
            let hover = self.motion.tonal(
                ["vision_hover0", "vision_hover1", "vision_hover2"][i],
                f32::from(hovered_vision == Some(i)),
                0.1,
            );
            let shade = crate::hud_motion::color_rgba(
                crate::ui_theme::tone(0x5b5b5bff),
                0xb8b6b5ff,
                selected,
            );
            self.cache.props_keyed(
                ctx,
                *hover_node,
                S1,
                (strip, shade, (255. * hover) as u8),
                |(strip, shade, alpha)| {
                    format!("visible: {strip}; color: #{:06x}{alpha:02x};", shade >> 8)
                },
            );
            let ink = crate::hud_motion::color_rgba(0xb8b6b5ff, 0x393939ff, selected);
            self.cache
                .props_keyed(ctx, *glyph_node, S1, (ink,), |(ink,)| {
                    format!("x: 10px; y: 10px; width: 24px; height: 24px; color: #{ink:08x};")
                });
        }
        // GLYPHS: 0 ef_pause, 1 ef_play, 2 ef_eye, 3 ef_cpu, 4 ef_camera,
        // 5 ef_settings, 6 ef_list, 7 ef_swap.
        let glyph = if phase == Phase::Running { 0 } else { 1 };
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
            (
                "watch",
                464.,
                16.,
                48.,
                48.,
                phase == Phase::Ready,
                false,
                2,
                26.,
            ),
            ("release", 48., 6., 44., 44., strip, false, 3, 26.),
            ("camera", 96., 6., 44., 44., strip, camera.locked(), 4, 26.),
            ("settings", 144., 6., 44., 44., strip, false, 5, 26.),
            ("tab", TAB_X, 6., 68., 44., strip, false, 6, 26.),
            (
                "swap", 0., SWAP_Y, SWAP_TILE, SWAP_TILE, in_match, open, 7, 32.,
            ),
        ] {
            self.animate(
                ctx,
                n.button(node),
                (x, y, w, h),
                preparing,
                cursor,
                pressed,
                accent,
                visible,
                Some(glyph),
                gsize,
                root,
            );
        }
        self.vis(ctx, n.tab_key, strip);
        if ai {
            self.cache
                .props_keyed(ctx, n.primary_glyph, S2, (), |()| "x: 9px;".into());
        }
        self.cache
            .props_keyed(ctx, n.primary_glyph, S3, (preparing,), |(preparing,)| {
                format!(
                    "color: #{};",
                    if preparing { "1c1a18ff" } else { "eeececff" }
                )
            });
        self.cache
            .props_keyed(ctx, n.camera_glyph, S1, (camera.locked(),), |(locked,)| {
                format!("color: #{};", if locked { "fdee00ff" } else { "cbc9c7ff" })
            });
        self.cache.props_keyed(ctx, n.tab_glyph, S1, (), |()| {
            "x: 38px; y: 9px; width: 26px; height: 26px;".into()
        });
        for lane in 0..5 {
            let p = players.iter().find(|p| p.lane == lane);
            let b = &n.buttons[7 + lane];
            let (rect, visible) = if preparing {
                ((lane as f32 * 80., 4., 72., 72.), phase == Phase::Ready)
            } else {
                (swap_tile(lane, unfold), unfold > 0.02 && p.is_some())
            };
            self.animate(
                ctx,
                b,
                rect,
                preparing,
                cursor,
                pressed,
                p.is_some_and(|p| selected == Some(p.player)),
                visible && (preparing || p.is_some()),
                None,
                56.,
                root,
            );
            // motion: B8
            let b8 = crate::hud_motion_pass::b8_swap_stagger(&mut self.pass, lane, open);
            self.cache.props_keyed(
                ctx,
                b.portrait,
                S2,
                ((255. * b8).round() as u8,),
                |(alpha,)| format!("color: #ffffff{alpha:02x};"),
            );
        }
        let mut tip = None;
        if let Some(cursor) = cursor {
            for (k, node) in TIPS.iter().enumerate() {
                let text = match *node {
                    "primary" => {
                        if preparing {
                            crate::lang::tr("Start · F11")
                        } else if ai {
                            crate::lang::tr("Take control · F11")
                        } else if phase == Phase::Paused {
                            crate::lang::tr("Resume · F11")
                        } else {
                            crate::lang::tr("Pause · F11")
                        }
                    }
                    "camera" => crate::lang::tr("Lock camera · Y\nHold Space to follow"),
                    "watch" => crate::lang::tr("Watch this match"),
                    "tab" => crate::lang::tr("Team details · Tab"),
                    "release" => crate::lang::tr("Return control to AI · F12"),
                    "settings" => crate::lang::tr("Settings"),
                    "vision_own" => crate::lang::tr("Own-team vision"),
                    "vision_other" => crate::lang::tr("Opposing-team vision"),
                    _ => crate::lang::tr("All-team vision"),
                };
                let path = &n.tip_paths[k];
                if ctx.ui_visible(path) == Some(true)
                    && ctx
                        .ui_node_rect(path)
                        .is_some_and(|(x, y, w, h)| Rect { x, y, w, h }.contains(cursor))
                {
                    tip = Some((text.to_owned(), ctx.ui_node_rect(path).unwrap()));
                    break;
                }
            }
            if preparing && tip.is_none() {
                for p in players {
                    let owned;
                    let path = match n.lane_paths.get(p.lane) {
                        Some(path) => path.as_str(),
                        None => {
                            owned = format!("{PATH}.lane{}", p.lane);
                            owned.as_str()
                        }
                    };
                    if ctx
                        .ui_node_rect(path)
                        .is_some_and(|(x, y, w, h)| Rect { x, y, w, h }.contains(cursor))
                    {
                        tip = Some((
                            crate::tooltips::champion_name(ctx, &p.champion),
                            ctx.ui_node_rect(path).unwrap(),
                        ));
                        break;
                    }
                }
            }
        }
        if let Some((text, (tx, ty, tw, _))) = tip {
            if self.tip.as_ref().is_none_or(|(old, _)| old != &text) {
                self.tip = Some((text.clone(), crate::clock::now()));
            }
            let fade = self.tip.as_ref().map_or(1., |(_, at)| {
                crate::hud_motion::easing(
                    (crate::clock::since(at).as_secs_f32() / 0.117).min(1.),
                    true,
                )
            });
            let width =
                (crate::hud_style::width(&text.replace('\n', " "), 20.) + 28.).clamp(120., 440.);
            let (text, lines) = crate::hud_style::wrap(&text, 20., width - 24.);
            let height = lines * 26 + 16;
            let (bx, by, bw, bh) = root.unwrap_or((0., 0., 1920., 1080.));
            let sx = if preparing { 512. } else { STRIP_W } / bw;
            let sy = if preparing { 80. } else { 56. } / bh;
            let x =
                ((tx + tw / 2. - bx) * sx - width / 2.).clamp(-bx * sx, 1920. - bx * sx - width);
            let y = (ty - by) * sy - height as f32 - 12.;
            self.cache.props(ctx, n.tooltip, &format!("visible: true; x: {x:.2}px; y: {y:.2}px; width: {width:.2}px; height: {height}px; color: #~1e1e1d{:02x};",(217.*fade)as u8));
            self.cache.props(
                ctx,
                n.tooltip_text,
                &format!(
                    "width: {}px; height: {}px; color: #ffffff{:02x};",
                    width - 24.,
                    height - 16,
                    (255. * fade) as u8
                ),
            );
            // Write only on a change, not every frame the tip shows.
            self.cache.text(ctx, n.tooltip_text, &text);
        } else {
            self.tip = None;
            self.vis(ctx, n.tooltip, false);
        }
        // The Settings window covers the strip; the shop window may.
        let covered = crate::ui_state::SETTINGS_OPEN.load(std::sync::atomic::Ordering::Relaxed)
            || cursor.is_some_and(|p| {
                crate::shop_ui::AREA
                    .lock()
                    .is_ok_and(|area| area.is_some_and(|a| a.contains(p)))
            });
        let under = crate::ui_sound::under_cursor(ctx, n.clickable.iter(), cursor);
        self.hover.update(true, under, cursor, pressed || covered);
        // Nothing shows while perf_hide_mod_ui hides the mod's windows.
        if crate::ui_root::hidden() {
            return;
        }
        if let Some((x, y, w, h)) = root {
            bounds.push(Rect {
                x,
                y,
                w: if ai { w * 260. / STRIP_W } else { w },
                h,
            });
        }
        bounds.extend(swap_bounds.into_iter().flatten());
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
        // A choice is taken in any phase the champion may be changed in
        // (the swap column in a match), never while loading or released.
        ui.events.lock().unwrap().choice = Some(((1, 33, 1), 1, 3));
        assert_eq!(
            ui.take_choice(Some(Phase::Running), Some((1, 33, 1)), 1),
            Some(3)
        );
        for phase in [Phase::Loading, Phase::Released, Phase::Armed] {
            ui.events.lock().unwrap().choice = Some(((1, 33, 1), 1, 3));
            assert_eq!(ui.take_choice(Some(phase), Some((1, 33, 1)), 1), None);
            assert!(!swappable(Some(phase)));
        }
        assert!([Phase::Ready, Phase::Running, Phase::Paused, Phase::Ai]
            .into_iter()
            .all(|p| swappable(Some(p))));
        // The column: folded onto its button, then one tile and gap per lane upward.
        // Top lane at the top, support next to the button.
        assert_eq!(swap_tile(3, 0.), (0., SWAP_Y, SWAP_TILE, SWAP_TILE));
        assert_eq!(swap_tile(4, 1.).1, SWAP_Y - 60.);
        assert_eq!(swap_tile(0, 1.).1, SWAP_Y - 300.);
        ui.events.lock().unwrap().swap = true;
        ui.close_swap();
        assert!(!ui.events.lock().unwrap().swap);
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
