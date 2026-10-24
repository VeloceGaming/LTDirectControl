//! The screen effect's draw list (1.6.0-perf-dev3): the low-health edge
//! tint and the death veil, as boxes at z 990 (under the HUD, over the
//! battlefield), from the screen rectangles `screen_effect.rs` computes.
//! Nothing of it stays in the tree.
use crate::camera::Rect;
use crate::draw_list::DrawList;

/// The bands' z (the effect's old root and bands).
pub const Z: i32 = 990;

/// One frame's bands: (screen rectangle, colour), in their order.
pub fn build(pieces: &[(Rect, u32)], list: &mut DrawList) {
    for (r, color) in pieces {
        list.rect((r.x, r.y, r.w, r.h), Z, 0., *color);
    }
}
