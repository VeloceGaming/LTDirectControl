//! Portrait-only preparation; clicks enqueue phase/key-bound choices.
//!
//! Since 1.6.0-perf-dev3 the strip's buttons, boxes and glyphs are drawn
//! (`session_draw.rs`) and their clicks are the mod's own hit tests
//! ([`SessionUi::clicks`]): a press that starts on a shown button and is
//! released on the same button clicks it, once (`ui_click::once`), unless
//! the Settings window or the shop covers the cursor. The tree keeps the
//! root, the lane tiles' champion portraits, the vision group's eye glyphs
//! and the tooltip.
use crate::{
    camera::Rect,
    hud_motion_pass::hundredths,
    native_timing::{MatchKey, NativeTiming, Phase},
    own_selection::PlayerIdentity,
    session_draw::ButtonView,
    ui_graphics,
    ui_writes::{NodeId, PropCache},
    Logger,
};
use mod_api_stable::StableClient;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
pub(crate) const PATH: &str = "ingame.lt_session_controls";
/// In-match strip controls: Pause, AI, Camera, Settings, vision group, Tab.
const STRIP_W: f32 = crate::session_draw::STRIP.0;
const VISION_X: f32 = crate::session_draw::VISION_X;
const VISION_STEP: f32 = crate::session_draw::VISION_STEP;
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
#[cfg(test)]
pub(crate) fn template_for_test() -> String {
    template()
}
/// What stays in the UI tree: the root (its anchoring follows the phase),
/// the lane tiles as containers of the champion portraits, the vision
/// buttons as containers of their eye glyphs (whose ink tweens), and the
/// tooltip. Everything else is drawn (`session_draw.rs`) at its node's z.
fn template() -> String {
    let mut s=String::from("lt_session_controls:empty { width: 512px; height: 80px; anchor_x: 0.5; anchor_y: 1; pivot_x: 0.5; pivot_y: 1; y: -62px; z: 1100; ignore_event: true;\n");
    for lane in 0..5 {
        s.push_str(&format!("#lane{lane}:empty {{ x: {}px; y: 4px; width: 72px; height: 72px; z: 1102; ignore_event: true; #portrait:image {{ x: 0px; y: 0px; anchor_x: 0.5; anchor_y: 0.5; pivot_x: 0.5; pivot_y: 0.5; width: 56px; height: 56px; z: 1104; sample_linear: false; ignore_event: true; visible: false; }} }}\n",lane*80));
    }
    for node in ["vision_own", "vision_other", "vision_all"] {
        s.push_str(&format!("#{node}:empty {{ x: 0px; y: 0px; width: 48px; height: 44px; z: 1102; visible: false; ignore_event: true; "));
        s.push_str(&ui_graphics::image("glyph", "ef_eye", 11, 9, 26, 1105));
        s.push_str("}\n");
    }
    s.push_str(r##"#tooltip:color { x: 0px; y: -68px; width: 310px; height: 56px; z: 1200; color: #~1e1e1dd9; ignore_event: true; visible: false; rounding: Uniform { rounding: 3; } #text:label { @"asset/base/style/main#label"; x: 12px; y: 8px; width: 286px; height: 40px; size: 20; color: #ffffffff; ignore_event: true; z: 1201; } }
    }"##);
    crate::hud_style::fonts(s)
}
#[derive(Default)]
pub struct SessionUi {
    spawn_at: Option<Instant>,
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
    /// The strip's remaining tree nodes in `cache` (1.6.0).
    nodes: Option<std::sync::Arc<Nodes>>,
    /// What the drawn elements show (`session_draw.rs`), as their nodes
    /// held it.
    view: crate::session_draw::SessionView,
    /// The root's rectangle last frame (the clicks' hit tests use the
    /// layout that was on screen).
    root: Option<(f32, f32, f32, f32)>,
    /// The left button last frame, and the button a press started on.
    was_pressed: bool,
    press_on: Option<&'static str>,
    /// This frame's commands (`session_draw.rs`), sent in `post_render`.
    pub(crate) list: crate::draw_list::DrawList,
    /// `draw_list::check_origin`'s state.
    origin_checked: u8,
}
/// Write sites in the strip's cache: `VIS` is always `visible: {on};`.
const VIS: u8 = 0;
const S1: u8 = 1;
const S2: u8 = 2;
/// One button (or swap tile) of the strip: its tweens' names, and for a
/// lane tile its container and portrait in the tree.
struct Button {
    name: &'static str,
    root: NodeId,
    portrait: NodeId,
    hover_key: String,
    press_key: String,
    select_key: String,
}
/// The strip's nodes and paths, found once.
struct Nodes {
    root: NodeId,
    /// Each vision button's container and eye glyph.
    vision: [(NodeId, NodeId); 3],
    tooltip: NodeId,
    tooltip_text: NodeId,
    /// primary, watch, release, camera, settings, tab, swap, lane0..lane4.
    buttons: Vec<Button>,
    /// Each lane's portrait node name and path (`lane{n}.portrait`).
    portraits: [(String, String); 5],
    /// Each lane tile's path.
    lane_paths: [String; 5],
    /// The clickable controls' paths, in `CLICKABLE` order.
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
                portrait: id(&format!("{name}.portrait")),
                hover_key: format!("hover_{name}"),
                press_key: format!("press_{name}"),
                select_key: format!("select_{name}"),
            })
            .collect();
        let vision = ["vision_own", "vision_other", "vision_all"]
            .map(|n| (id(n), id(&format!("{n}.glyph"))));
        Self {
            root,
            vision,
            tooltip: id("tooltip"),
            tooltip_text: id("tooltip.text"),
            buttons,
            portraits: std::array::from_fn(|lane| {
                let node = format!("lane{lane}.portrait");
                let path = format!("{PATH}.{node}");
                (node, path)
            }),
            lane_paths: std::array::from_fn(|lane| format!("{PATH}.lane{lane}")),
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
        let view = ButtonView {
            rect: (x, hundredths(y), w, h),
            shown: visible,
            border,
            back: background,
            stroke: stroke as f32,
        };
        if let Some(glyph) = glyph {
            let k = crate::session_draw::BUTTONS
                .iter()
                .position(|n| *n == node)
                .expect("a drawn button");
            self.view.buttons[k] = view;
            let g = &mut self.view.glyphs[k];
            g.glyph = glyph;
            g.size = hundredths(gsize);
            g.at = (hundredths((w - gsize) / 2.), hundredths((h - gsize) / 2.));
        } else {
            // A lane tile: its container keeps the place and visibility of
            // the portrait; its box is drawn.
            let lane = b.name.as_bytes()[4] - b'0';
            self.view.lanes[usize::from(lane)] = view;
            let flags = u64::from(visible);
            self.cache.props_keyed(
                ctx,
                b.root,
                S1,
                (pair(x, y), pair(w, h), flags),
                |(xy, wh, flags)| {
                    let ((x, y), (w, h)) = (unpair(xy), unpair(wh));
                    format!(
                        "x: {x}px; y: {y:.2}px; width: {w}px; height: {h}px; visible: {};",
                        flags & 1 == 1
                    )
                },
            );
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
            self.view.shown = false;
            self.list.clear();
            (self.was_pressed, self.press_on) = (false, None);
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
        self.view.shown = false;
        self.list.clear();
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
            self.view = Default::default();
            if !ctx.ui_spawn_source("ingame", &crate::ui_theme::themed(&template()))
                || !ctx.ui_exists(PATH)
            {
                return;
            }
            log.write("SESSION portrait UI spawned");
        }
        // Write only on a change, as the HUD does (not every frame).
        crate::ui_root::show(ctx, PATH);
        self.view.shown = !crate::ui_root::hidden();
        let n = match &self.nodes {
            Some(n) => n.clone(),
            None => {
                let n = std::sync::Arc::new(Nodes::new(&mut self.cache));
                self.nodes = Some(n.clone());
                n
            }
        };
        // Clicks first, as the game's click events came before this frame's
        // update; on the layout that was on screen.
        if let Some(clicked) = self.clicks(cursor, pressed) {
            let k = CLICKABLE.iter().position(|c| *c == clicked).unwrap_or(0);
            if crate::ui_click::once(&n.clickable[k]) {
                self.click(clicked, timing, camera);
            }
        }
        self.strip(
            ctx, &n, phase, players, selected, camera, cursor, pressed, bounds,
        );
        let ingame = ctx.ui_node_rect("ingame");
        let drawn = self.view.origin(ingame);
        crate::draw_list::check_origin(
            "session strip",
            drawn,
            self.root,
            &mut self.origin_checked,
            log,
        );
        crate::session_draw::build(&self.view, ingame, &mut self.list);
    }
    /// The control a press-and-release clicked this frame: the press began
    /// on a shown control and the release came over the same one, with
    /// neither the Settings window nor the shop over the cursor (they take
    /// the game's clicks there).
    fn clicks(&mut self, cursor: Option<(f32, f32)>, pressed: bool) -> Option<&'static str> {
        let covered = covered(cursor);
        let under = cursor
            .filter(|_| !covered && self.view.shown)
            .and_then(|p| self.view.button_at(self.root, p));
        if pressed && !self.was_pressed {
            self.press_on = under;
        }
        let click = (!pressed && self.was_pressed)
            .then_some(self.press_on)
            .flatten()
            .filter(|on| under == Some(*on));
        if !pressed {
            self.press_on = None;
        }
        self.was_pressed = pressed;
        click
    }
    /// What a click on `control` does (the 1.5.3 click handlers).
    fn click(
        &self,
        control: &str,
        timing: &Arc<NativeTiming>,
        camera: &Arc<crate::camera::CameraControl>,
    ) {
        // The events lock only where the old handler took it.
        let events = |f: &mut dyn FnMut(&mut Events)| {
            if let Ok(mut e) = self.events.lock() {
                f(&mut e);
            }
        };
        match control {
            "primary" => timing.request_action(true),
            "release" | "watch" => timing.request_action(false),
            "camera" => camera.request_toggle(),
            "settings" => events(&mut |e| e.settings_open = true),
            "tab" => events(&mut |e| e.team = !e.team),
            "vision_own" => events(&mut |e| e.vision = Some(crate::camera::Vision::Own)),
            "vision_other" => events(&mut |e| e.vision = Some(crate::camera::Vision::Other)),
            "vision_all" => events(&mut |e| e.vision = Some(crate::camera::Vision::All)),
            "swap" => events(&mut |e| e.swap = !e.swap),
            lane => {
                let Some(lane) = lane.strip_prefix("lane").and_then(|l| l.parse().ok()) else {
                    return;
                };
                let Some(key) = timing.match_key() else {
                    return;
                };
                events(&mut |e| {
                    // Before Start the tiles are the chooser itself; in a
                    // match they are the open swap column.
                    let phase = timing.phase();
                    if phase == Some(Phase::Ready) || swappable(phase) && e.swap {
                        e.choice = Some((key, timing.generation(), lane));
                        e.swap = false;
                    }
                });
            }
        }
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
        self.view.preparing = preparing;
        self.view.loading = phase == Phase::Loading;
        // The keys the strip names, as bound now (a rebind shows at once).
        self.view.keys.follow(&crate::settings::current_shared());
        // A wider Tab key grows its cap and button to the left; a wider
        // "Take control" label grows the AI-phase primary button right.
        let tab_grow = self.view.keys.tab_cap() - crate::session_draw::TAB_CAP.0;
        let take_width = self.view.keys.take_box() + 56.;
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
        self.view.sweep =
            crate::hud_motion_pass::b8_swap_sweep(&self.pass, in_match, SWAP_TILE).view();
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
        self.view.take_label = ai;
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
        self.view.rules = strip;
        let vision = camera.vision();
        let vision_position = self
            .motion
            .value("vision_indicator", vision.index() as f32, 0.167);
        let hovered_vision =
            vision_hover_index(strip, cursor, root.map(|(x, y, w, h)| Rect { x, y, w, h }));
        self.view.indicator = (
            strip,
            hundredths(VISION_X + 2. + VISION_STEP * vision_position),
        );
        for (i, (node, glyph_node)) in n.vision.iter().enumerate() {
            self.cache
                .props_keyed(ctx, *node, S1, (strip, i), |(strip, i)| {
                    format!(
                        "x: {}px; y: 6px; width: 44px; height: 44px; visible: {strip};",
                        VISION_X + VISION_STEP * i as f32
                    )
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
            self.view.vision[i] = (strip, shade & !0xff | u32::from((255. * hover) as u8));
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
                    take_width
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
            (
                "tab",
                TAB_X - tab_grow,
                6.,
                68. + tab_grow,
                44.,
                strip,
                false,
                6,
                26.,
            ),
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
        self.view.tab_key = strip;
        // The glyphs' own writes after their buttons' (primary 0, camera 3, tab 5).
        let glyphs = &mut self.view.glyphs;
        if ai {
            glyphs[0].at.0 = 9.;
        }
        glyphs[0].ink = if preparing { 0x1c1a18ff } else { 0xeeececff };
        glyphs[3].ink = if camera.locked() {
            0xfdee00ff
        } else {
            0xcbc9c7ff
        };
        (glyphs[5].at, glyphs[5].size) = ((38. + tab_grow, 9.), 26.);
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
            for node in TIPS {
                let Some(rect) = self
                    .view
                    .control_rect(node, root)
                    .filter(|&(x, y, w, h)| Rect { x, y, w, h }.contains(cursor))
                else {
                    continue;
                };
                // Built for the hovered control only (no text per frame).
                let keys = &self.view.keys;
                let text = match node {
                    "primary" => crate::key_cap::tip(
                        if preparing {
                            crate::lang::tr("Start · {key}")
                        } else if ai {
                            crate::lang::tr("Take control · {key}")
                        } else if phase == Phase::Paused {
                            crate::lang::tr("Resume · {key}")
                        } else {
                            crate::lang::tr("Pause · {key}")
                        },
                        &keys.start,
                    ),
                    "camera" => crate::key_cap::tip_lines(
                        crate::lang::tr("Lock camera · {key}\nHold {follow} to follow"),
                        &[("key", &keys.camera), ("follow", &keys.follow)],
                    ),
                    "watch" => crate::lang::tr("Watch this match").to_owned(),
                    "tab" => {
                        crate::key_cap::tip(crate::lang::tr("Team details · {key}"), &keys.tab)
                    }
                    "release" => crate::key_cap::tip(
                        crate::lang::tr("Return control to AI · {key}"),
                        &keys.release,
                    ),
                    "settings" => crate::lang::tr("Settings").to_owned(),
                    "vision_own" => crate::lang::tr("Own-team vision").to_owned(),
                    "vision_other" => crate::lang::tr("Opposing-team vision").to_owned(),
                    _ => crate::lang::tr("All-team vision").to_owned(),
                };
                tip = Some((text, rect));
                break;
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
        let covered = covered(cursor);
        // The clickable control under the cursor, the last in `CLICKABLE`
        // order first (as the old search through the tree's rectangles).
        let under = cursor.and_then(|(x, y)| {
            CLICKABLE
                .iter()
                .zip(&n.clickable)
                .rev()
                .find_map(|(name, path)| {
                    let (l, t, w, h) = self.view.control_rect(name, root)?;
                    (x >= l && x < l + w && y >= t && y < t + h).then(|| path.clone())
                })
        });
        self.hover.update(true, under, cursor, pressed || covered);
        self.root = root;
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
/// The Settings window covers the strip; the shop window may.
fn covered(cursor: Option<(f32, f32)>) -> bool {
    crate::ui_state::SETTINGS_OPEN.load(std::sync::atomic::Ordering::Relaxed)
        || cursor.is_some_and(|p| {
            crate::shop_ui::AREA
                .lock()
                .is_ok_and(|area| area.is_some_and(|a| a.contains(p)))
        })
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
    /// The strip in a match with every button shown, its root on screen.
    fn strip_ui() -> (SessionUi, Option<(f32, f32, f32, f32)>) {
        let mut ui = SessionUi::default();
        ui.view.shown = true;
        ui.view.preparing = false;
        for (k, x) in [0., 464., 48., 96., 144., TAB_X, 0.]
            .into_iter()
            .enumerate()
        {
            let b = &mut ui.view.buttons[k];
            b.rect = if k == 6 {
                (0., SWAP_Y, SWAP_TILE, SWAP_TILE)
            } else {
                (x, 6., if k == 5 { 68. } else { 44. }, 44.)
            };
            b.shown = k != 1; // "watch" shows only in Ready.
        }
        ui.view.vision = [(true, 0); 3];
        (ui, Some((8., 1024., STRIP_W, 56.)))
    }
    #[test]
    fn a_click_is_a_press_and_release_on_the_same_shown_button() {
        let (mut ui, root) = strip_ui();
        ui.root = root;
        let pause = Some((30., 1050.));
        let camera = Some((126., 1050.));
        // Press and release on Pause: one click, on the release.
        assert_eq!(ui.clicks(pause, true), None);
        assert_eq!(ui.clicks(pause, true), None);
        assert_eq!(ui.clicks(pause, false), Some("primary"));
        assert_eq!(ui.clicks(pause, false), None);
        // Pressed on Pause, released on the camera button: nothing.
        ui.clicks(pause, true);
        assert_eq!(ui.clicks(camera, false), None);
        // Pressed elsewhere, dragged onto the camera button: nothing.
        ui.clicks(Some((900., 500.)), true);
        ui.clicks(camera, true);
        assert_eq!(ui.clicks(camera, false), None);
        // A hidden button takes no click ("watch" outside Ready).
        let watch = Some((8. + 480., 1050.));
        ui.clicks(watch, true);
        assert_eq!(ui.clicks(watch, false), None);
        // The vision group and Tab.
        ui.clicks(Some((230., 1050.)), true);
        assert_eq!(ui.clicks(Some((230., 1050.)), false), Some("vision_own"));
        ui.clicks(Some((640., 1050.)), true);
        assert_eq!(ui.clicks(Some((640., 1050.)), false), Some("tab"));
        // Nothing while the strip is hidden.
        ui.view.shown = false;
        ui.clicks(pause, true);
        assert_eq!(ui.clicks(pause, false), None);
    }
    #[test]
    fn the_swap_button_takes_the_click_over_a_tile_unfolding_from_under_it() {
        let (mut ui, root) = strip_ui();
        ui.root = root;
        let tile = &mut ui.view.lanes[4];
        tile.rect = swap_tile(4, 0.5);
        tile.shown = true;
        let both = (8. + 20., 1024. + SWAP_Y + 4.);
        assert_eq!(ui.view.button_at(root, both), Some("swap"));
        let above = (8. + 20., 1024. + swap_tile(4, 0.5).1 + 2.);
        assert_eq!(ui.view.button_at(root, above), Some("lane4"));
        ui.clicks(Some(both), true);
        assert_eq!(ui.clicks(Some(both), false), Some("swap"));
    }
    #[test]
    fn strip_holds_controls_in_the_requested_order_without_a_more_menu() {
        let t = template();
        assert!(!t.contains("#more:") && !t.contains("#menu:") && !t.contains("ef_more"));
        // Drawn since 1.6.0-perf-dev3: no button runners left in the tree.
        assert!(!t.contains("color_icon_button") && !t.contains("#tab_key"));
        // Pause 0, AI 48, camera 96, settings 144, vision 200-336, Tab after.
        const {
            assert!(VISION_X >= 144. + 44. + 8.);
            assert!(VISION_X + 2. * VISION_STEP + 44. <= 342.);
            assert!(TAB_X + 68. <= STRIP_W);
        }
    }
}
