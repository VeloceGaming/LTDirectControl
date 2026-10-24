//! The session strip's draw list (1.6.0-perf-dev3): the buttons' boxes and
//! glyphs, the lane and swap tiles' boxes, the vision group's track,
//! indicator, hover shades and marks, the rules, the Tab key cap, the
//! "Take control" label, the loading hourglass and the swap sweep (B8).
//!
//! `SessionView` holds what those elements show, as their nodes held it
//! (`session_ui.rs` sets a field where it wrote the node). What stays in the
//! tree under the strip's root: the root itself (its anchoring), the lane
//! tiles as containers of their champion portraits, the vision buttons as
//! containers of their eye glyphs (whose ink tweens), and the tooltip
//! (wrapped text). The buttons' clicks are the mod's own hit tests now
//! ([`SessionView::button_at`]).
use crate::draw_list::{anchored, at, Align, DrawList, Rect, TextStyle};
use crate::drawn_glyphs::asset;

/// The root in the pre-match chooser and on the strip.
pub const CHOOSER: (f32, f32) = (512., 80.);
pub const STRIP: (f32, f32) = (656., 56.);
pub const VISION_X: f32 = 200.;
pub const VISION_STEP: f32 = 46.;
/// The drawn buttons (the glyph buttons and the swap button), in the old
/// template's order.
pub const BUTTONS: [&str; 7] = [
    "primary", "watch", "release", "camera", "settings", "tab", "swap",
];
/// Glyph images the strip's buttons show.
pub const GLYPHS: [&str; 8] = [
    "ef_pause",
    "ef_play",
    "ef_eye",
    "ef_cpu",
    "ef_camera",
    "ef_settings",
    "ef_list",
    "ef_swap",
];
/// A `color_icon_button`'s look: its place, visibility, border, fill and
/// border width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonView {
    pub rect: Rect,
    pub shown: bool,
    pub border: u32,
    pub back: u32,
    pub stroke: f32,
}
/// A button's glyph: which, its size, its place in the button, its ink.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphView {
    pub glyph: u8,
    pub size: f32,
    pub at: (f32, f32),
    pub ink: u32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SessionView {
    /// The root is shown (nothing draws otherwise).
    pub shown: bool,
    /// The root is the pre-match chooser (512 x 80, bottom-centred), not the
    /// strip (656 x 56, bottom-left).
    pub preparing: bool,
    pub loading: bool,
    pub lanes: [ButtonView; 5],
    pub buttons: [ButtonView; 7],
    pub glyphs: [GlyphView; 7],
    /// Each vision button: shown, then its hover shade's colour.
    pub vision: [(bool, u32); 3],
    pub take_label: bool,
    pub tab_key: bool,
    pub rules: bool,
    /// The vision indicator: shown, x.
    pub indicator: (bool, f32),
    /// B8: the sweep's width and colour, when shown.
    pub sweep: Option<(f32, u32)>,
}
impl Default for SessionView {
    fn default() -> Self {
        let bg = crate::ui_theme::rgba(0xff);
        let border = crate::ui_theme::tone(0x4b4a49ff);
        let button = |rect| ButtonView {
            rect,
            shown: false,
            border,
            back: bg,
            stroke: 1.,
        };
        Self {
            shown: false,
            preparing: true,
            loading: true,
            lanes: std::array::from_fn(|lane| ButtonView {
                shown: true,
                ..button((80. * lane as f32, 4., 72., 72.))
            }),
            buttons: [button((0., 0., 48., 44.)); 7],
            // The template's glyphs: play, eye, processor, camera, settings,
            // list, swap; 26 px at (11, 9).
            glyphs: [1u8, 2, 3, 4, 5, 6, 7].map(|glyph| GlyphView {
                glyph,
                size: 26.,
                at: (11., 9.),
                ink: 0xeeececff,
            }),
            vision: [(false, 0); 3],
            take_label: false,
            tab_key: true,
            rules: true,
            indicator: (true, 202.),
            sweep: None,
        }
    }
}
impl SessionView {
    /// The root's corner on screen.
    pub fn origin(&self, ingame: Option<Rect>) -> (f32, f32) {
        if self.preparing {
            anchored(ingame, CHOOSER, (0.5, 1.), (0.5, 1.), (0., -62.))
        } else {
            anchored(ingame, STRIP, (0., 1.), (0., 1.), (8., 0.))
        }
    }
    /// A clickable control's place in the root (the vision buttons are
    /// 44 x 44 at their segment).
    fn control(&self, name: &str) -> Option<(Rect, bool)> {
        if let Some(i) = BUTTONS.iter().position(|b| *b == name) {
            let b = &self.buttons[i];
            return Some((b.rect, b.shown));
        }
        if let Some(lane) = name
            .strip_prefix("lane")
            .and_then(|l| l.parse::<usize>().ok())
        {
            let b = self.lanes.get(lane)?;
            return Some((b.rect, b.shown));
        }
        let i = ["vision_own", "vision_other", "vision_all"]
            .iter()
            .position(|v| *v == name)?;
        Some((
            (VISION_X + VISION_STEP * i as f32, 6., 44., 44.),
            self.vision[i].0,
        ))
    }
    /// A control's rectangle on screen, given the root's, while it shows.
    pub fn control_rect(&self, name: &str, root: Option<Rect>) -> Option<Rect> {
        let (rx, ry, rw, rh) = root?;
        let ((x, y, w, h), shown) = self.control(name)?;
        let (sw, sh) = if self.preparing { CHOOSER } else { STRIP };
        let (sx, sy) = (rw / sw, rh / sh);
        shown.then_some((rx + x * sx, ry + y * sy, w * sx, h * sy))
    }
    /// The topmost shown control under `cursor`: the old tree's last node
    /// first (it took the click), so the swap button wins over the lane
    /// tiles that unfold from under it.
    pub fn button_at(&self, root: Option<Rect>, cursor: (f32, f32)) -> Option<&'static str> {
        const TREE_ORDER: [&str; 15] = [
            "lane0",
            "lane1",
            "lane2",
            "lane3",
            "lane4",
            "primary",
            "watch",
            "release",
            "camera",
            "settings",
            "tab",
            "vision_own",
            "vision_other",
            "vision_all",
            "swap",
        ];
        TREE_ORDER.iter().rev().copied().find(|name| {
            self.control_rect(name, root).is_some_and(|(x, y, w, h)| {
                cursor.0 >= x && cursor.0 < x + w && cursor.1 >= y && cursor.1 < y + h
            })
        })
    }
}
/// A `color_icon_button`: its border, `stroke` px wide, inside the box, and
/// the fill within it, both with 2 px rounded corners.
fn button(list: &mut DrawList, o: (f32, f32), b: &ButtonView, z: i32) {
    if !b.shown {
        return;
    }
    let (x, y, w, h) = at(o, b.rect);
    let s = b.stroke;
    let inner = (x + s, y + s, w - 2. * s, h - 2. * s);
    if b.back & 0xff == 0xff {
        list.rect((x, y, w, h), z, 2., b.border);
    } else if s > 0. {
        for r in [
            (x, y, w, s),
            (x, y + h - s, w, s),
            (x, y + s, s, h - 2. * s),
            (x + w - s, y + s, s, h - 2. * s),
        ] {
            list.rect(r, z, 0., b.border);
        }
    }
    list.rect(inner, z, (2. - s).max(0.), b.back);
}
fn glyph(list: &mut DrawList, o: (f32, f32), b: &ButtonView, g: &GlyphView) {
    if !b.shown {
        return;
    }
    let name = GLYPHS[g.glyph as usize];
    match asset(name, g.size as u32, g.ink) {
        Some(a) => list.sprite(
            a,
            (o.0 + b.rect.0 + g.at.0, o.1 + b.rect.1 + g.at.1),
            1105,
            false,
        ),
        None => debug_assert!(false, "no baked glyph {name} {} {:08x}", g.size, g.ink),
    }
}
/// The strip's commands for one frame (none while its root is hidden).
pub fn build(v: &SessionView, ingame: Option<Rect>, list: &mut DrawList) {
    list.clear();
    if !v.shown {
        return;
    }
    let o = v.origin(ingame);
    for lane in &v.lanes {
        button(list, o, lane, 1102);
    }
    if v.loading {
        if let Some(a) = asset("hourglass", 32, 0xffffffff) {
            list.sprite(a, (o.0 + 240., o.1 + 24.), 1104, false);
        }
    }
    // primary .. tab, then the vision group, then swap (the template's order).
    for k in 0..6 {
        button(list, o, &v.buttons[k], 1102);
        glyph(list, o, &v.buttons[k], &v.glyphs[k]);
    }
    vision(list, o, v);
    button(list, o, &v.buttons[6], 1102);
    glyph(list, o, &v.buttons[6], &v.glyphs[6]);
    if let Some((width, color)) = v.sweep {
        list.rect(at(o, (0., -10., width, 2.)), 1106, 0., color);
    }
    if v.take_label {
        list.text(
            "Take control · F11",
            at(o, (52., 6., 204., 44.)),
            1106,
            TextStyle::numeric(18., Align::Start),
            0xeeececff,
        );
    }
    if v.tab_key {
        let cap = at(o, (592., 17., 30., 22.));
        list.rect(cap, 1104, 2., 0xeeececff);
        list.text(
            "Tab",
            cap,
            1105,
            TextStyle::numeric(13., Align::Center),
            0x393939ff,
        );
    }
    if v.rules {
        let edge = crate::ui_theme::tone(0x4b4a49ff);
        list.rect(at(o, (194., 17., 1., 22.)), 1101, 0., edge);
        list.rect(at(o, (342., 17., 1., 22.)), 1101, 0., edge);
        list.rect(
            at(o, (200., 6., 136., 44.)),
            1101,
            3.,
            crate::ui_theme::tone(0x3a3837ff),
        );
    }
    if v.indicator.0 {
        list.rect(at(o, (v.indicator.1, 8., 40., 40.)), 1102, 2., 0xeeececff);
    }
}
/// The vision group's hover shades and team marks (its eye glyphs are tree
/// nodes at z 1105).
fn vision(list: &mut DrawList, o: (f32, f32), v: &SessionView) {
    for (i, (shown, hover)) in v.vision.iter().enumerate() {
        if !shown {
            continue;
        }
        let b = (o.0 + VISION_X + VISION_STEP * i as f32, o.1 + 6.);
        list.rect(at(b, (2., 2., 40., 40.)), 1103, 2., *hover);
        if let Some(mark) = [Some(0x53b8e4ff), Some(0xff642eff), None][i] {
            list.rect(at(b, (6., 13., 3., 18.)), 1104, 0., mark);
        }
    }
}
