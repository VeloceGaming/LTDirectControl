//! The state plate's draw list (1.6.0-perf-dev3): the plate, its key cap
//! (the start binding, F11 by default) and the respawn count, drawn from the values `state_plate.rs`
//! computes. Its glyph stays an image node in the tree (its tint fades with
//! the plate, which a baked PNG cannot do).
use crate::draw_list::{at, Align, DrawList, Rect, TextStyle};

/// The plate's root sits at (x, 100) in `ingame`, 48 px high.
pub const PLATE_Y: f32 = 100.;
pub const PLATE_H: f32 = 48.;
/// One frame of a shown plate, as the plate's nodes held it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlateView {
    /// The plate's left edge and width (`ingame` units).
    pub x: f32,
    pub width: f32,
    /// The plate's colour (the themed `#~1e1e1d` at the plate's alpha).
    pub back: u32,
    /// The key cap and its text ink, when shown.
    pub key: Option<(u32, u32)>,
    /// The key cap's width: 36 px at F11, wider for a wider key (the
    /// plate widens with it and stays centred).
    pub cap: f32,
    /// The respawn count and its ink, when shown.
    pub count: Option<(usize, u32)>,
}
/// The key cap at F11: 36 x 26 at (60, 11), 14 px text.
pub const KEY: Rect = (60., 11., 36., 26.);
pub const KEY_SIZE: f32 = 14.;
const COUNT: Rect = (56., 8., 44., 32.);

/// The plate's commands, its root at `origin` (the `ingame` corner); `key`
/// is the cap's text.
pub fn build(v: &PlateView, key: &str, origin: (f32, f32), list: &mut DrawList) {
    let root = (origin.0 + v.x, origin.1 + PLATE_Y);
    list.rect(at(root, (0., 0., v.width, PLATE_H)), 1120, 2., v.back);
    if let Some((cap, ink)) = v.key {
        let rect = at(root, (KEY.0, KEY.1, v.cap, KEY.3));
        list.rect(rect, 1121, 2., cap);
        list.text(
            key,
            rect,
            1122,
            TextStyle::numeric(KEY_SIZE, Align::Center),
            ink,
        );
    }
    if let Some((count, ink)) = v.count {
        list.text_fmt(
            format_args!("{count}"),
            at(root, COUNT),
            1122,
            TextStyle::numeric(23., Align::Center),
            ink,
        );
    }
}
