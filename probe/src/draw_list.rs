//! Draw lists (1.6.0-perf-dev3): the mod's windows drawn with the SDK's
//! direct draw calls instead of the game's UI tree. The design and its
//! measured limits are in `docs/investigation-direct-draw-hud.md`.
//!
//! A window keeps its state step as before. Its draw step is a pure
//! function (`hud_draw.rs`, `session_draw.rs`, `stats_draw.rs`, ...) that
//! turns the window's state values into a [`DrawList`] in `post_update`; the
//! client sends every window's list to the game in `post_render`
//! ([`submit`]). A list keeps its storage from frame to frame.
//!
//! Every command is in the "UI" map (1920 x 1080), at the z of the node it
//! replaced, so drawn elements sort with the tree's remaining nodes and
//! with the shop, Settings and Tab windows exactly as the nodes did (seen
//! in game with the 1.6.0-perf-dev2 probe). Commands of equal z keep their
//! order in the list, which follows the old template's node order (a
//! guess: the probe did not measure how the game orders equal z).
//!
//! What a draw command cannot do, and so stays in the tree: images cut from
//! the game's sprite sheets, champion portraits, wrapped, scrolled or rich
//! text, a scaled or tinted image (see `drawn_glyphs.rs`).
use mod_api_stable::{StableClient, StableSpriteParams, TextAlignXV1, TextAlignYV1};

/// A rectangle (x, y, width, height).
pub type Rect = (f32, f32, f32, f32);

/// The mod's two fonts (`hud_style::fonts`: the game's `bold_label` style
/// became numeric, `label` became medium).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    Numeric,
    Medium,
}
impl Font {
    pub fn asset(self) -> &'static str {
        match self {
            Font::Numeric => "asset/lt_direct_control/font/numeric",
            Font::Medium => "asset/lt_direct_control/font/medium",
        }
    }
}
/// A text's place in its box, along one axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
}
impl Align {
    fn x(self) -> TextAlignXV1 {
        match self {
            Align::Start => TextAlignXV1::Left,
            Align::Center => TextAlignXV1::Center,
        }
    }
    fn y(self) -> TextAlignYV1 {
        match self {
            Align::Start => TextAlignYV1::Top,
            Align::Center => TextAlignYV1::Center,
        }
    }
}
/// How a label lays out its text: font, size and alignment (x, y).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub font: Font,
    pub size: f32,
    pub align: (Align, Align),
}
impl TextStyle {
    /// The HUD's `bold_label` labels: numeric font, vertically centred.
    pub const fn numeric(size: f32, align_x: Align) -> Self {
        Self {
            font: Font::Numeric,
            size,
            align: (align_x, Align::Center),
        }
    }
    pub const fn medium(size: f32, align_x: Align) -> Self {
        Self {
            font: Font::Medium,
            size,
            align: (align_x, Align::Center),
        }
    }
}
/// One draw command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cmd {
    Rect {
        rect: Rect,
        z: i32,
        rounding: f32,
        color: u32,
    },
    /// A line from one point to another (no window draws one yet).
    Line {
        from: (f32, f32),
        to: (f32, f32),
        width: f32,
        z: i32,
        color: u32,
    },
    /// `text` is a range of the list's text buffer.
    Text {
        rect: Rect,
        z: i32,
        style: TextStyle,
        color: u32,
        text: (u32, u32),
    },
    /// A PNG at its own pixel size, its top-left corner at `at`.
    Sprite {
        at: (f32, f32),
        z: i32,
        asset: &'static str,
        nearest: bool,
    },
    /// An SVG scaled into `rect` and tinted (no window draws one yet: the
    /// game fills every path and ignores strokes, seen in game 2026-10-24).
    Svg {
        rect: Rect,
        z: i32,
        asset: &'static str,
        color: u32,
    },
}
/// One window's commands for a frame.
#[derive(Default)]
pub struct DrawList {
    cmds: Vec<Cmd>,
    text: String,
}
/// The four 1 px offsets of a text outline (accepted in game 2026-10-24
/// for the HUD's outlined numbers).
const OUTLINE: [(f32, f32); 4] = [(-1., 0.), (1., 0.), (0., -1.), (0., 1.)];

fn moved((x, y, w, h): Rect, (dx, dy): (f32, f32)) -> Rect {
    (x + dx, y + dy, w, h)
}
impl DrawList {
    /// Empty the list, keeping its storage.
    pub fn clear(&mut self) {
        self.cmds.clear();
        self.text.clear();
    }
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }
    #[cfg(test)]
    pub fn commands(&self) -> &[Cmd] {
        &self.cmds
    }
    /// A command's text.
    pub fn text_of(&self, (start, end): (u32, u32)) -> &str {
        &self.text[start as usize..end as usize]
    }
    /// A filled box; nothing for a fully transparent colour.
    pub fn rect(&mut self, rect: Rect, z: i32, rounding: f32, color: u32) {
        if color & 0xff != 0 && rect.2 > 0. && rect.3 > 0. {
            self.cmds.push(Cmd::Rect {
                rect,
                z,
                rounding,
                color,
            });
        }
    }
    #[allow(dead_code)] // For windows that need a line; none does yet.
    pub fn line(&mut self, from: (f32, f32), to: (f32, f32), width: f32, z: i32, color: u32) {
        if color & 0xff != 0 {
            self.cmds.push(Cmd::Line {
                from,
                to,
                width,
                z,
                color,
            });
        }
    }
    /// Text in `rect`; nothing for an empty text or a transparent colour.
    pub fn text(&mut self, text: &str, rect: Rect, z: i32, style: TextStyle, color: u32) {
        if text.is_empty() || color & 0xff == 0 {
            return;
        }
        let start = self.text.len() as u32;
        self.text.push_str(text);
        self.cmds.push(Cmd::Text {
            rect,
            z,
            style,
            color,
            text: (start, self.text.len() as u32),
        });
    }
    /// Text formatted straight into the list's buffer (no allocation once
    /// the buffer has grown): `list.text_fmt(format_args!("{n}"), ...)`.
    pub fn text_fmt(
        &mut self,
        args: std::fmt::Arguments<'_>,
        rect: Rect,
        z: i32,
        style: TextStyle,
        color: u32,
    ) {
        use std::fmt::Write;
        let start = self.text.len();
        let _ = self.text.write_fmt(args);
        if self.text.len() == start || color & 0xff == 0 {
            self.text.truncate(start);
            return;
        }
        self.cmds.push(Cmd::Text {
            rect,
            z,
            style,
            color,
            text: (start as u32, self.text.len() as u32),
        });
    }
    /// [`Self::outlined_text`] with the text formatted into the buffer once.
    pub fn outlined_fmt(
        &mut self,
        args: std::fmt::Arguments<'_>,
        rect: Rect,
        z: i32,
        style: TextStyle,
        (color, outline): (u32, u32),
    ) {
        use std::fmt::Write;
        let start = self.text.len();
        let _ = self.text.write_fmt(args);
        let text = (start as u32, self.text.len() as u32);
        if text.0 == text.1 {
            return;
        }
        for (offset, color) in OUTLINE
            .iter()
            .map(|o| (*o, outline))
            .chain(std::iter::once(((0., 0.), color)))
        {
            if color & 0xff != 0 {
                self.cmds.push(Cmd::Text {
                    rect: moved(rect, offset),
                    z,
                    style,
                    color,
                    text,
                });
            }
        }
    }
    /// A label with `outline: 1`: four dark copies 1 px off, then the text.
    pub fn outlined_text(
        &mut self,
        text: &str,
        rect: Rect,
        z: i32,
        style: TextStyle,
        (color, outline): (u32, u32),
    ) {
        for offset in OUTLINE {
            self.text(text, moved(rect, offset), z, style, outline);
        }
        self.text(text, rect, z, style, color);
    }
    /// A baked glyph PNG at its own size (`drawn_glyphs.rs`).
    pub fn sprite(&mut self, asset: &'static str, at: (f32, f32), z: i32, nearest: bool) {
        self.cmds.push(Cmd::Sprite {
            at,
            z,
            asset,
            nearest,
        });
    }
    #[allow(dead_code)] // For filled SVG shapes; no window draws one yet.
    pub fn svg(&mut self, asset: &'static str, rect: Rect, z: i32, color: u32) {
        self.cmds.push(Cmd::Svg {
            rect,
            z,
            asset,
            color,
        });
    }
    /// Send the commands to the game (in `post_render` only).
    pub fn submit(&self, ctx: &mut StableClient<'_>) {
        for cmd in &self.cmds {
            match *cmd {
                Cmd::Rect {
                    rect: (x, y, w, h),
                    z,
                    rounding,
                    color,
                } => {
                    ctx.draw_rect("UI", x, y, w, h, z, rounding, color);
                }
                Cmd::Line {
                    from,
                    to,
                    width,
                    z,
                    color,
                } => {
                    ctx.draw_line("UI", from.0, from.1, to.0, to.1, width, z, color);
                }
                Cmd::Text {
                    rect,
                    z,
                    style,
                    color,
                    text,
                } => {
                    ctx.draw_text(
                        "UI",
                        self.text_of(text),
                        style.font.asset(),
                        rect,
                        z,
                        style.size,
                        color,
                        style.align.0.x(),
                        style.align.1.y(),
                    );
                }
                Cmd::Sprite {
                    at,
                    z,
                    asset,
                    nearest,
                } => {
                    ctx.draw_sprite(
                        "UI",
                        asset,
                        &StableSpriteParams {
                            x: at.0,
                            y: at.1,
                            z,
                            sample_nearest: nearest,
                            ..StableSpriteParams::default()
                        },
                    );
                }
                Cmd::Svg {
                    rect,
                    z,
                    asset,
                    color,
                } => {
                    ctx.draw_svg("UI", asset, rect, z, color);
                }
            }
        }
    }
}
/// In `post_render`: send `lists` (every window's), unless
/// `perf_hide_mod_ui` keeps the mod's windows hidden or nothing can draw.
pub fn submit(ctx: &mut StableClient<'_>, lists: &[&DrawList]) {
    if crate::ui_root::hidden() || !ctx.can_draw() {
        return;
    }
    for list in lists {
        list.submit(ctx);
    }
}
/// The corner of a window root laid out in the game's `ingame` root, as the
/// tree places it: `anchor` (fraction of `ingame`) plus `offset` minus
/// `pivot` (fraction of the root's own `size`). No scaling: the tree's
/// sizes are in UI-map pixels (1920 x 1080) and so are the draw calls'.
pub fn anchored(
    ingame: Option<Rect>,
    size: (f32, f32),
    anchor: (f32, f32),
    pivot: (f32, f32),
    offset: (f32, f32),
) -> (f32, f32) {
    let (ix, iy, iw, ih) = ingame.unwrap_or((0., 0., 1920., 1080.));
    (
        ix + iw * anchor.0 + offset.0 - size.0 * pivot.0,
        iy + ih * anchor.1 + offset.1 - size.1 * pivot.1,
    )
}
/// The drawn place of a window against its tree root's place, as the game
/// laid it out: one log line the first time they are compared (they agree,
/// or not) and one more if they come to differ by more than half a pixel.
/// The drawn windows assume the anchoring in [`anchored`]; the tests share
/// that assumption, so only the game can confirm it. `state`: 0 not yet
/// compared, 1 logged agreeing, 2 logged differing. A root the game has not
/// laid out yet (0 x 0 at (0, 0) on the frame it was spawned, seen in the
/// author's dev3 log as one DIFFERENT line per window) is not compared;
/// the next frame's is (1.6.0-dev4).
pub fn check_origin(
    window: &str,
    drawn: (f32, f32),
    root: Option<Rect>,
    state: &mut u8,
    log: &crate::Logger,
) {
    let Some((x, y, w, h)) = root.filter(|r| r.2 > 0. && r.3 > 0.) else {
        return;
    };
    let apart = (drawn.0 - x).abs().max((drawn.1 - y).abs()) > 0.5;
    if *state == 0 || (apart && *state == 1) {
        *state = if apart { 2 } else { 1 };
        log.write(&format!(
            "DRAW origin {window}: drawn at ({:.1}, {:.1}), tree root at ({x:.1}, {y:.1}) {w:.0} x {h:.0}: {}",
            drawn.0,
            drawn.1,
            if apart { "DIFFERENT" } else { "same" }
        ));
    }
}
/// `rect` moved by `origin`.
pub fn at((ox, oy): (f32, f32), (x, y, w, h): Rect) -> Rect {
    (ox + x, oy + y, w, h)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_list_keeps_its_storage_and_skips_what_shows_nothing() {
        let mut list = DrawList::default();
        list.rect((0., 0., 10., 10.), 1000, 0., 0x11223300);
        list.rect((0., 0., 0., 10.), 1000, 0., 0x112233ff);
        list.text(
            "",
            (0., 0., 10., 10.),
            1000,
            TextStyle::numeric(12., Align::Start),
            0xffffffff,
        );
        assert!(list.is_empty());
        list.outlined_text(
            "12",
            (5., 5., 20., 10.),
            1005,
            TextStyle::numeric(18., Align::Start),
            (0xffffffff, 0x0e0d0cff),
        );
        assert_eq!(list.commands().len(), 5);
        let Cmd::Text {
            rect, color, text, ..
        } = list.commands()[0]
        else {
            panic!()
        };
        assert_eq!(
            (rect, color, list.text_of(text)),
            ((4., 5., 20., 10.), 0x0e0d0cff, "12")
        );
        let Cmd::Text { rect, color, .. } = list.commands()[4] else {
            panic!()
        };
        assert_eq!((rect, color), ((5., 5., 20., 10.), 0xffffffff));
        let capacity = list.cmds.capacity();
        for _ in 0..100 {
            list.clear();
            list.outlined_text(
                "12",
                (5., 5., 20., 10.),
                1005,
                TextStyle::numeric(18., Align::Start),
                (0xffffffff, 0x0e0d0cff),
            );
        }
        assert_eq!(list.cmds.capacity(), capacity);
    }
    #[test]
    fn a_root_not_laid_out_yet_is_compared_on_the_next_frame() {
        let log = crate::test_support::logger("draw-origin");
        let mut state = 0;
        let drawn = (0., 934.);
        // The frame the root was spawned: 0 x 0 at (0, 0). Nothing logged.
        check_origin("hud strip", drawn, Some((0., 0., 0., 0.)), &mut state, &log);
        check_origin(
            "hud strip",
            drawn,
            Some((0., 0., 1920., 0.)),
            &mut state,
            &log,
        );
        check_origin("hud strip", drawn, None, &mut state, &log);
        assert_eq!(state, 0);
        // Laid out: compared once, agreeing.
        check_origin(
            "hud strip",
            drawn,
            Some((0., 934., 1920., 146.)),
            &mut state,
            &log,
        );
        assert_eq!(state, 1);
        // A real difference later is still logged.
        check_origin(
            "hud strip",
            drawn,
            Some((0., 930., 1920., 146.)),
            &mut state,
            &log,
        );
        assert_eq!(state, 2);
    }
    #[test]
    fn roots_are_anchored_as_the_tree_anchors_them() {
        // The HUD strip: 1920 x 146, bottom-centred.
        assert_eq!(
            anchored(None, (1920., 146.), (0.5, 1.), (0.5, 1.), (0., 0.)),
            (0., 934.)
        );
        // The session chooser: 512 x 80, bottom-centred, 62 px up.
        assert_eq!(
            anchored(None, (512., 80.), (0.5, 1.), (0.5, 1.), (0., -62.)),
            (704., 938.)
        );
        assert_eq!(
            anchored(
                Some((10., 20., 1920., 1080.)),
                (656., 56.),
                (0., 1.),
                (0., 1.),
                (8., 0.)
            ),
            (18., 1044.)
        );
    }
}
