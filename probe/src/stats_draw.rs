//! The stats panels' draw list (1.6.0-perf-dev3): the frames, value texts,
//! the target's face box, level badge and health bar, drawn from the unit
//! `stats_ui.rs` shows. The stat icons (frames of the game's stat sheet,
//! drawn at 18 px), the champion portrait and the unit glyph stay image
//! nodes in the tree; their templates use the same geometry from here.
use crate::draw_list::{anchored, at, Align, DrawList, Rect, TextStyle};
use crate::stats_panel::{Shown, Unit, FORMATS, STATS};

/// The own panel's root: the HUD's (1920 wide, bottom-centred) 4 px taller.
pub const OWN_ROOT: (f32, f32) = (1920., 150.);
pub const OWN_PANEL: Rect = (612., 2., 208., 92.);
pub const OWN_CELLS: ((f32, f32), (f32, f32)) = ((7., 5.), (67., 21.));
/// The target panel's root, top left under the team-name header.
pub const TARGET_AT: (f32, f32) = (8., 56.);
pub const TARGET_PANEL: Rect = (0., 0., 292., 129.);
pub const TARGET_CELLS: ((f32, f32), (f32, f32)) = ((84., 38.), (68., 21.));
/// The target's face box (panel units) and, inside it, the level badge.
pub const FACE: Rect = (10., 46., 64., 64.);
const BADGE: Rect = (0., 46., 26., 18.);
const HP_BACK: Rect = (10., 10., 272., 20.);
const HP_FILL: (f32, f32, f32) = (11., 11., 18.);
/// The health bar's full width.
pub const HP_WIDTH: f32 = 270.;

/// Cell `k`'s top-left corner in its panel.
pub fn cell(((x0, y0), (dx, dy)): ((f32, f32), (f32, f32)), k: usize) -> (f32, f32) {
    (x0 + (k % 3) as f32 * dx, y0 + (k / 3) as f32 * dy)
}
/// The own panel's root corner on screen.
pub fn own_origin(ingame: Option<Rect>) -> (f32, f32) {
    anchored(ingame, OWN_ROOT, (0.5, 1.), (0.5, 1.), (0., 0.))
}
pub fn target_origin(ingame: Option<Rect>) -> (f32, f32) {
    anchored(
        ingame,
        (TARGET_PANEL.2, TARGET_PANEL.3),
        (0., 0.),
        (0., 0.),
        TARGET_AT,
    )
}
/// A skill-slot style frame: shadow, grey edge, inset fill (in the old
/// template's node order: edge, shadow, fill).
fn frame(list: &mut DrawList, (x, y, w, h): Rect) {
    list.rect((x, y, w, h), 1001, 2., crate::ui_theme::tone(0x4b4a49ff));
    list.rect((x, y + 3., w, h), 1000, 0., 0x00000066);
    list.rect(
        (x + 1., y + 1., w - 2., h - 2.),
        1002,
        0.,
        crate::ui_theme::shade_rgba(5, 0xff),
    );
}
fn values(
    list: &mut DrawList,
    panel: (f32, f32),
    grid: ((f32, f32), (f32, f32)),
    values: &[f64; STATS],
) {
    for (k, (value, format)) in values.iter().zip(FORMATS).enumerate() {
        let (x, y) = cell(grid, k);
        list.text_fmt(
            format_args!("{}", Shown(*value, format)),
            at(panel, (x + 23., y, grid.1 .0 - 25., 18.)),
            1004,
            TextStyle::numeric(15., Align::Start),
            0xeeececff,
        );
    }
}
/// Your champion's panel beside the Q slot.
pub fn own(unit: &Unit, ingame: Option<Rect>, list: &mut DrawList) {
    let panel = at(own_origin(ingame), OWN_PANEL);
    frame(list, panel);
    values(list, (panel.0, panel.1), OWN_CELLS, &unit.values);
}
/// The selected unit's panel, top left.
pub fn target(unit: &Unit, ingame: Option<Rect>, list: &mut DrawList) {
    let panel = at(target_origin(ingame), TARGET_PANEL);
    let corner = (panel.0, panel.1);
    frame(list, panel);
    let face = at(corner, FACE);
    list.rect(face, 1003, 2., crate::ui_theme::tone(0x4b4a49ff));
    list.rect(
        (face.0 + 1., face.1 + 1., 62., 62.),
        1004,
        0.,
        crate::ui_theme::tone(0x0e0d0cff),
    );
    let badge = at((face.0, face.1), BADGE);
    list.rect(badge, 1006, 0., 0x1c1a18ee);
    list.text_fmt(
        format_args!("{}", unit.level),
        badge,
        1007,
        TextStyle::numeric(14., Align::Center),
        0xeeececff,
    );
    let back = at(corner, HP_BACK);
    list.rect(back, 1003, 0., crate::ui_theme::tone(0x0e0d0cff));
    let (hp, max) = unit.hp;
    let ratio = if max > 0 { hp as f32 / max as f32 } else { 0. };
    let fill = (HP_WIDTH * ratio.clamp(0., 1.)).round();
    let color = if unit.friendly {
        0x6aff55ff
    } else {
        0xff5c6cff
    };
    list.rect(
        at(corner, (HP_FILL.0, HP_FILL.1, fill, HP_FILL.2)),
        1004,
        0.,
        color,
    );
    let style = TextStyle::numeric(13., Align::Center);
    // "hp / max", then " (+shield)" while a shield holds.
    let shield = unit.shield;
    if shield > 0 {
        list.outlined_fmt(
            format_args!("{hp} / {max} (+{shield})"),
            back,
            1005,
            style,
            (0xffffffff, 0x000000ff),
        );
    } else {
        list.outlined_fmt(
            format_args!("{hp} / {max}"),
            back,
            1005,
            style,
            (0xffffffff, 0x000000ff),
        );
    }
    values(list, corner, TARGET_CELLS, &unit.values);
}
