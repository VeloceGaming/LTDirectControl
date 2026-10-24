//! A stand-in for the game's draw calls (test builds only, 1.6.0-perf-dev3),
//! the draw-list sibling of `fake_ui.rs`.
//!
//! [`frame`] runs a closure with a client context whose UI calls go to this
//! thread's fake UI tree and whose draw calls are recorded here, one line
//! per command, in order (numbers to two decimals):
//!
//! - `R x,y,w,h z r=<rounding> #rrggbbaa`: a box
//! - `T x,y,w,h z <font> <size> #rrggbbaa <align x><align y> "text"`
//! - `I x,y z nearest|smooth <asset>`: a sprite at its own size
//! - `L x,y x,y z w=<width> #rrggbbaa`: a line
//! - `G x,y,w,h z #rrggbbaa <asset>`: an SVG
//!
//! The window tests (`ui_identity.rs`) save these lines per frame under
//! `probe/testdata/ui_draw/`; `tools/compare_draw_review.cjs` renders them.
use mod_api_stable::{ClientCtxV1, DrawVtableV1, StableClient, TextAlignXV1, TextAlignYV1};
use std::cell::RefCell;
use std::ffi::c_void;

thread_local! {
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}
/// The commands recorded so far, emptied.
pub fn take_log() -> Vec<String> {
    LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
}
fn push(line: String) {
    LOG.with(|l| l.borrow_mut().push(line));
}
unsafe fn text<'a>(ptr: *const u8, len: usize) -> &'a str {
    std::str::from_utf8(std::slice::from_raw_parts(ptr, len)).unwrap_or("<bad utf8>")
}
fn n(v: f32) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.into()
    }
}
unsafe extern "C" fn can_draw(_: *const c_void) -> bool {
    true
}
#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn rect(
    _: *mut c_void,
    map: *const u8,
    map_len: usize,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    z: i32,
    rounding: f32,
    color: u32,
) -> bool {
    assert_eq!(text(map, map_len), "UI");
    push(format!(
        "R {},{},{},{} {z} r={} #{color:08x}",
        n(x),
        n(y),
        n(w),
        n(h),
        n(rounding)
    ));
    true
}
#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn line(
    _: *mut c_void,
    map: *const u8,
    map_len: usize,
    from_x: f32,
    from_y: f32,
    to_x: f32,
    to_y: f32,
    width: f32,
    z: i32,
    color: u32,
) -> bool {
    assert_eq!(text(map, map_len), "UI");
    push(format!(
        "L {},{} {},{} {z} w={} #{color:08x}",
        n(from_x),
        n(from_y),
        n(to_x),
        n(to_y),
        n(width)
    ));
    true
}
#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn draw_text(
    _: *mut c_void,
    map: *const u8,
    map_len: usize,
    t: *const u8,
    t_len: usize,
    font: *const u8,
    font_len: usize,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    z: i32,
    size: f32,
    color: u32,
    align_x: u32,
    align_y: u32,
) -> bool {
    assert_eq!(text(map, map_len), "UI");
    let font = text(font, font_len);
    let font = font.rsplit('/').next().unwrap_or(font);
    let ax = match TextAlignXV1::from_code(align_x) {
        Some(TextAlignXV1::Center) => 'C',
        Some(TextAlignXV1::Right) => 'R',
        _ => 'L',
    };
    let ay = match TextAlignYV1::from_code(align_y) {
        Some(TextAlignYV1::Center) => 'C',
        Some(TextAlignYV1::Bottom) => 'B',
        _ => 'T',
    };
    push(format!(
        "T {},{},{},{} {z} {font} {} #{color:08x} {ax}{ay} {:?}",
        n(x),
        n(y),
        n(w),
        n(h),
        n(size),
        text(t, t_len)
    ));
    true
}
#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn svg(
    _: *mut c_void,
    map: *const u8,
    map_len: usize,
    path: *const u8,
    path_len: usize,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    z: i32,
    color: u32,
) -> bool {
    assert_eq!(text(map, map_len), "UI");
    push(format!(
        "G {},{},{},{} {z} #{color:08x} {}",
        n(x),
        n(y),
        n(w),
        n(h),
        text(path, path_len)
    ));
    true
}
#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn sprite(
    _: *mut c_void,
    map: *const u8,
    map_len: usize,
    texture: *const u8,
    texture_len: usize,
    x: f32,
    y: f32,
    z: i32,
    rot: f32,
    flip_x: bool,
    flip_y: bool,
    pivot_x: f32,
    pivot_y: f32,
    uv_x: f32,
    uv_y: f32,
    uv_w: f32,
    uv_h: f32,
    sample_nearest: bool,
) -> bool {
    assert_eq!(text(map, map_len), "UI");
    // The mod's glyphs: whole image, upright, top-left pivot.
    assert_eq!(
        (rot, flip_x, flip_y, pivot_x, pivot_y, uv_x, uv_y, uv_w, uv_h),
        (0., false, false, 0., 0., 0., 0., 1., 1.)
    );
    push(format!(
        "I {},{} {z} {} {}",
        n(x),
        n(y),
        if sample_nearest { "nearest" } else { "smooth" },
        text(texture, texture_len)
    ));
    true
}
struct Table(DrawVtableV1);
thread_local! {
    static TABLE: &'static Table = Box::leak(Box::new(Table(draw_table())));
}
fn draw_table() -> DrawVtableV1 {
    // SAFETY: every field is a size or an optional function pointer, for
    // which all-zero bits are 0 and None.
    let mut t: DrawVtableV1 = unsafe { std::mem::zeroed() };
    t.size = std::mem::size_of::<DrawVtableV1>();
    t.can_draw = Some(can_draw);
    t.rect = Some(rect);
    t.line = Some(line);
    t.text = Some(draw_text);
    t.svg = Some(svg);
    t.sprite = Some(sprite);
    t
}
/// Run `f` with a context that draws into this recorder (and answers UI
/// calls from this thread's fake tree), as a `post_render` callback does.
pub fn frame<R>(f: impl FnOnce(&mut StableClient<'_>) -> R) -> R {
    let table = TABLE.with(|t| *t);
    // SAFETY: as above, all-zero is null pointers and a zero size.
    let mut raw: ClientCtxV1 = unsafe { std::mem::zeroed() };
    raw.size = std::mem::size_of::<ClientCtxV1>();
    raw.ui = crate::fake_ui::table();
    raw.draw = &table.0;
    // SAFETY: `raw` lives for the whole call.
    let mut ctx = unsafe { StableClient::from_raw(&mut raw) }.expect("fake client context");
    f(&mut ctx)
}

#[cfg(test)]
mod tests {
    #[test]
    fn draw_calls_are_recorded_in_order() {
        super::take_log();
        super::frame(|ctx| {
            assert!(ctx.can_draw());
            ctx.draw_rect("UI", 1., 2.5, 3., 4., 1000, 2., 0x11223344);
            ctx.draw_text(
                "UI",
                "12",
                "asset/lt_direct_control/font/numeric",
                (0., 0., 10., 10.),
                1005,
                18.,
                0xffffffff,
                mod_api_stable::TextAlignXV1::Center,
                mod_api_stable::TextAlignYV1::Center,
            );
        });
        assert_eq!(
            super::take_log(),
            [
                "R 1,2.5,3,4 1000 r=2 #11223344",
                "T 0,0,10,10 1005 numeric 18 #ffffffff CC \"12\""
            ]
        );
    }
}
