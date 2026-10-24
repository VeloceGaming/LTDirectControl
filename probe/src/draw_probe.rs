//! A hidden drawing probe (1.6.0-perf-dev2), for the Stage 2 design in
//! `docs/investigation-direct-draw-hud.md` (build "2a"). Nothing here is
//! part of the mod's look: it answers four questions in one match.
//!
//! `perf_draw_probe` (controls.json only, no control in the Settings
//! window, default off). While it is on and you are on the battlefield,
//! `post_render` draws in the "UI" map (1920 x 1080):
//!
//! 1. A load that steps every 20 s through 0, 1, 2, 4 and 8 hundred small
//!    boxes (with a fifth as many short texts), top right: the cost of a
//!    direct draw command, read against the PERF `post_render` average and
//!    Frame Lab's 10-second windows. One `PERF PROBE level=...` line at
//!    each step.
//! 2. Layering boxes A-D at chosen z values over the shop, the HUD strip
//!    and the Settings window: how direct draws sort against tree nodes.
//! 3. Image samples, top left: `draw_sprite` of a 32 px PNG glyph (nearest
//!    and smooth) and `draw_svg` of a mod SVG asset at 32 and 17 px.
//! 4. Two "88"s in the numeric font beside the HUD's level number: a drawn
//!    number next to a tree label, plain and with an offset outline.
//!
//! With the switch off, `draw` returns before anything else: no draw, no
//! lock, no log. The only line is `PERF perf_draw_probe=...` when the
//! switch changes.
use crate::Logger;
use mod_api_stable::{StableClient, StableSpriteParams, TextAlignXV1, TextAlignYV1};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::Mutex;
use std::time::{Duration, Instant};

type Rect = (f32, f32, f32, f32);

static ON: AtomicBool = AtomicBool::new(false);
static STATE: Mutex<State> = Mutex::new(State {
    start: None,
    level: None,
    frame: 0,
});

struct State {
    /// When the current battlefield stretch with the probe on began.
    start: Option<Instant>,
    /// The level last logged (None: not yet on the battlefield).
    level: Option<usize>,
    /// Frames drawn, for the changing text and alpha.
    frame: u32,
}

/// The load cycle: hundreds of boxes per level (texts: a fifth as many).
pub const LEVELS: [usize; 5] = [0, 1, 2, 4, 8];
/// How long each level lasts: two of Frame Lab's and PERF's 10 s windows.
pub const STEP: Duration = Duration::from_secs(20);
const BOXES_PER_LEVEL: usize = 100;
const TEXTS_PER_LEVEL: usize = 20;

// 1. The load, top right: under the game's 50 px header and the input-delay
// readout (1702-1902 x 56-92), right of the shop (ends x 1540) and the
// Settings window (ends x 1640).
const BOX_ORIGIN: (f32, f32) = (1652., 104.);
const BOX_COLUMNS: usize = 16;
const BOX_PITCH: f32 = 7.;
const BOX_SIZE: f32 = 6.;
const BOX_Z: i32 = 1000;
const TEXT_ORIGIN: (f32, f32) = (1772., 104.);
const TEXT_COLUMNS: usize = 5;
const TEXT_CELL: (f32, f32) = (28., 13.);
const TEXT_Z: i32 = 1001;
const NUMERIC: &str = "asset/lt_direct_control/font/numeric";

// 2. Layering boxes (40 x 40, 2 px lighter border).
const SAMPLE: f32 = 40.;
/// Inside the shop (`shop_ui::WINDOW` at (180, 78)), 20 px in from its
/// left edge, across the bottom edge of its opaque header band (`top`,
/// window (1, 1, 1358, 67), z 1521): window y 44-84, so the top 24 px lie
/// under the band and the bottom 16 px over the window's fill (z 1501),
/// left of the tab rail's indicator (from window y 84). z 1510: over the
/// fill and the grid layer (z 1505-1509), under the header.
pub const BOX_A: (f32, f32, i32) = (200., 122., 1510);
/// Strip-relative (the HUD root, 1920 x 146, bottom-centred): the top
/// 30 px over the battlefield, the bottom 10 px over the opaque `#strip`
/// (strip y 90-146, z 1000). Between the recall key (ends x 700) and the
/// skill slots (from x 832), clear of the session strip (x 8-664).
pub const BOX_B: (f32, f32, i32) = (702., 60., 999);
pub const BOX_C: (f32, f32, i32) = (760., 60., 1001);
/// Centre of the screen, inside the Settings window (280, 115, 1360,
/// 850; its nodes run z 1999-2090): just under its backdrop (z 1999), so
/// it is hidden behind an open Settings window only if direct draws sort
/// by z with tree nodes (or land below the tree, which B and C tell apart).
pub const BOX_D: (f32, f32, i32) = (940., 520., 1998);

// 3. Image samples, top left: right of the stats target panel (x 8-300,
// y 56-185), above the drawn notices (from y 110).
const IMAGE_BACK: Rect = (320., 58., 149., 44.);
const IMAGE_Z: i32 = 1001;
const IMAGE_ROW_Y: f32 = 64.;
const SPRITE: &str = "asset/lt_direct_control/ui/ef_coin";
const SVG: &str = "asset/lt_direct_control/ui/probe_glyph";

// 4. The "88"s: strip-relative, 100 px right of the level label (730, 100,
// 40, 36; numeric font 24, centred, z 1005), then 50 px further, over the
// health bar's left part (x 780-1140, y 106-130).
pub const LEVEL_LABEL: Rect = (730., 100., 40., 36.);
const EIGHTY_EIGHT_Z: i32 = 1010;
const STRIP: (f32, f32) = (1920., 146.);

/// Once per frame (client thread): read the switch; log when it changes.
pub fn configure(on: bool, log: &Logger) {
    if ON.swap(on, Relaxed) != on {
        if let Ok(mut s) = STATE.lock() {
            *s = State {
                start: None,
                level: None,
                frame: 0,
            };
        }
        log.write(&format!(
            "PERF perf_draw_probe={on}: {}",
            if on {
                "drawing probe on (on the battlefield; load steps 0,1,2,4,8 every 20 s)"
            } else {
                "drawing probe off"
            }
        ));
    }
}

/// The level after `elapsed` on the battlefield with the probe on.
pub fn level_at(elapsed: Duration) -> usize {
    LEVELS[(elapsed.as_secs() / STEP.as_secs()) as usize % LEVELS.len()]
}
/// Boxes and texts drawn at `level`.
pub fn counts(level: usize) -> (usize, usize) {
    (level * BOXES_PER_LEVEL, level * TEXTS_PER_LEVEL)
}
/// Whether anything is drawn this frame.
pub fn active(on: bool, in_game: bool, can_draw: bool) -> bool {
    on && in_game && can_draw
}
/// The `i`-th load box (row by row).
pub fn box_rect(i: usize) -> Rect {
    let (col, row) = ((i % BOX_COLUMNS) as f32, (i / BOX_COLUMNS) as f32);
    (
        BOX_ORIGIN.0 + col * BOX_PITCH,
        BOX_ORIGIN.1 + row * BOX_PITCH,
        BOX_SIZE,
        BOX_SIZE,
    )
}
/// The `j`-th load text's box (row by row).
pub fn text_rect(j: usize) -> Rect {
    let (col, row) = ((j % TEXT_COLUMNS) as f32, (j / TEXT_COLUMNS) as f32);
    (
        TEXT_ORIGIN.0 + col * TEXT_CELL.0,
        TEXT_ORIGIN.1 + row * TEXT_CELL.1,
        TEXT_CELL.0,
        TEXT_CELL.1,
    )
}
/// Screen position of a HUD-strip rectangle, from the game's `ingame` root
/// (the strip is 1920 x 146, bottom-centred, scaled with `ingame` like the
/// HUD's own hit tests in `player_hud.rs`). Fallback: the full UI map.
pub fn strip_rect(ingame: Option<Rect>, (x, y, w, h): Rect) -> Rect {
    let (ix, iy, iw, ih) = ingame.unwrap_or((0., 0., 1920., 1080.));
    let scale = iw / 1920.;
    let (sw, sh) = (STRIP.0 * scale, STRIP.1 * scale);
    let (ox, oy) = (ix + (iw - sw) / 2., iy + ih - sh);
    (ox + x * scale, oy + y * scale, w * scale, h * scale)
}
/// The text a frame draws: the frame counter modulo 1000, three digits.
fn digits(frame: u32, out: &mut [u8; 3]) -> &str {
    let n = frame % 1000;
    *out = [
        b'0' + (n / 100) as u8,
        b'0' + (n / 10 % 10) as u8,
        b'0' + (n % 10) as u8,
    ];
    std::str::from_utf8(&out[..]).unwrap_or("000")
}

/// In `post_render`, before the mod's other drawing.
pub fn draw(ctx: &mut StableClient<'_>, log: &Logger) {
    if !ON.load(Relaxed) {
        return;
    }
    let in_game = ctx.client_scene_kind() == Some(mod_api_stable::ClientSceneKindV1::InGame);
    let Ok(mut s) = STATE.lock() else {
        return;
    };
    if !active(true, in_game, ctx.can_draw()) {
        if !in_game {
            // A new battlefield stretch starts the cycle again at level 0.
            s.start = None;
            s.level = None;
        }
        return;
    }
    let start = *s.start.get_or_insert_with(crate::clock::now);
    let level = level_at(crate::clock::since(start));
    let (rects, texts) = counts(level);
    if s.level != Some(level) {
        s.level = Some(level);
        log.write(&format!(
            "PERF PROBE level={level} rects={rects} texts={texts}"
        ));
    }
    s.frame = s.frame.wrapping_add(1);
    let frame = s.frame;
    drop(s);

    // 1. The load. The alpha pulses and the digits count, so no frame
    // repeats the last one exactly.
    let alpha = 0x60 + (frame % 32) * 3;
    let grey = 0x9a9a9a00 | alpha;
    for i in 0..rects {
        let (x, y, w, h) = box_rect(i);
        ctx.draw_rect("UI", x, y, w, h, BOX_Z, 0., grey);
    }
    let mut buffer = [0u8; 3];
    let text = digits(frame, &mut buffer);
    for j in 0..texts {
        ctx.draw_text(
            "UI",
            text,
            NUMERIC,
            text_rect(j),
            TEXT_Z,
            14.,
            0xeeececff,
            TextAlignXV1::Left,
            TextAlignYV1::Center,
        );
    }

    // 2. Layering boxes.
    let ingame = ctx.ui_node_rect("ingame");
    let sample = |ctx: &mut StableClient<'_>, (x, y, w, h): Rect, z: i32, fill: u32| {
        ctx.draw_rect("UI", x, y, w, h, z, 0., 0xffffffff);
        ctx.draw_rect("UI", x + 2., y + 2., w - 4., h - 4., z, 0., fill);
    };
    sample(ctx, (BOX_A.0, BOX_A.1, SAMPLE, SAMPLE), BOX_A.2, 0x2e7dd6ff);
    for ((x, y, z), fill) in [(BOX_B, 0xd63a2eff), (BOX_C, 0x2ed66aff)] {
        sample(ctx, strip_rect(ingame, (x, y, SAMPLE, SAMPLE)), z, fill);
    }
    sample(ctx, (BOX_D.0, BOX_D.1, SAMPLE, SAMPLE), BOX_D.2, 0xd6c02eff);

    // 3. Image samples on a dark backing.
    let (bx, by, bw, bh) = IMAGE_BACK;
    ctx.draw_rect("UI", bx, by, bw, bh, IMAGE_Z - 1, 4., 0x141414e6);
    let sprite = |ctx: &mut StableClient<'_>, x: f32, sample_nearest: bool| {
        ctx.draw_sprite(
            "UI",
            SPRITE,
            &StableSpriteParams {
                x,
                y: IMAGE_ROW_Y,
                z: IMAGE_Z,
                sample_nearest,
                ..StableSpriteParams::default()
            },
        );
    };
    sprite(ctx, 326., true);
    ctx.draw_svg(
        "UI",
        SVG,
        (366., IMAGE_ROW_Y, 32., 32.),
        IMAGE_Z,
        0xffffffff,
    );
    ctx.draw_svg(
        "UI",
        SVG,
        (406., IMAGE_ROW_Y + 7.5, 17., 17.),
        IMAGE_Z,
        0xffffffff,
    );
    sprite(ctx, 431., false);

    // 4. The "88"s beside the level number.
    let (lx, ly, lw, lh) = LEVEL_LABEL;
    let plain = strip_rect(ingame, (lx + 100., ly, lw, lh));
    let outlined = strip_rect(ingame, (lx + 150., ly, lw, lh));
    let eighty_eight = |ctx: &mut StableClient<'_>, (x, y, w, h): Rect, z: i32, color: u32| {
        ctx.draw_text(
            "UI",
            "88",
            NUMERIC,
            (x, y, w, h),
            z,
            24.,
            color,
            TextAlignXV1::Center,
            TextAlignYV1::Center,
        );
    };
    eighty_eight(ctx, plain, EIGHTY_EIGHT_Z, 0xffffffff);
    let (ox, oy, ow, oh) = outlined;
    for (dx, dy) in [(-1., 0.), (1., 0.), (0., -1.), (0., 1.)] {
        eighty_eight(
            ctx,
            (ox + dx, oy + dy, ow, oh),
            EIGHTY_EIGHT_Z - 1,
            0x0e0d0cff,
        );
    }
    eighty_eight(ctx, outlined, EIGHTY_EIGHT_Z, 0xffffffff);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlaps(a: Rect, b: Rect) -> bool {
        a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
    }
    /// The mod's windows and drawn readouts the load must stay clear of.
    const KEEP_CLEAR: [(&str, Rect); 7] = [
        ("header", (0., 0., 1920., 50.)),
        ("input delay", (1702., 56., 200., 36.)),
        ("shop", (180., 78., 1360., 872.)),
        ("settings", (280., 115., 1360., 850.)),
        ("stats target", (8., 56., 292., 129.)),
        ("hud strip", (0., 934., 1920., 146.)),
        ("released notice", (18., 110., 700., 32.)),
    ];

    #[test]
    fn the_level_steps_every_20_seconds_and_wraps() {
        let at = |s| level_at(Duration::from_secs(s));
        assert_eq!(
            [
                at(0),
                at(19),
                at(20),
                at(39),
                at(40),
                at(60),
                at(80),
                at(99)
            ],
            [0, 0, 1, 1, 2, 4, 8, 8]
        );
        assert_eq!(at(100), 0);
        assert_eq!(at(120), 1);
        assert_eq!(level_at(Duration::from_millis(19_999)), 0);
        assert_eq!(counts(8), (800, 160));
        assert_eq!(counts(0), (0, 0));
    }

    #[test]
    fn the_load_at_level_8_stays_in_the_top_right_quarter_and_off_the_windows() {
        let (rects, texts) = counts(*LEVELS.iter().max().unwrap());
        let all = (0..rects)
            .map(box_rect)
            .chain((0..texts).map(text_rect))
            .collect::<Vec<_>>();
        for r in &all {
            assert!(r.0 >= 960. && r.0 + r.2 <= 1920., "{r:?}");
            assert!(r.1 >= 50. && r.1 + r.3 <= 540., "{r:?}");
            for (name, clear) in KEEP_CLEAR {
                assert!(!overlaps(*r, clear), "{r:?} touches {name}");
            }
        }
        // Texts sit beside the boxes, not on them.
        for i in 0..rects {
            for j in 0..texts {
                assert!(!overlaps(box_rect(i), text_rect(j)));
            }
        }
    }

    #[test]
    fn the_image_samples_and_layering_boxes_sit_where_intended() {
        // The images sit under the shop's top edge (it opens over them,
        // as over the battlefield) but clear of everything else.
        for (name, clear) in KEEP_CLEAR {
            if name != "shop" {
                assert!(!overlaps(IMAGE_BACK, clear), "images touch {name}");
            }
        }
        assert!(IMAGE_BACK.0 + IMAGE_BACK.2 < 960. && IMAGE_BACK.1 + IMAGE_BACK.3 < 540.);
        // A inside the shop window, D inside the Settings window.
        let a = (BOX_A.0, BOX_A.1, SAMPLE, SAMPLE);
        // Window-relative: across the header band's bottom edge (y 68),
        // above the tab indicator (y 84).
        let (wx, wy) = (a.0 - 180., a.1 - 78.);
        assert!(wx >= 1. && wy < 68. && wy + a.3 > 69. && wy + a.3 <= 84.);
        assert!(BOX_A.2 > 1509 && BOX_A.2 < 1521);
        let d = (BOX_D.0, BOX_D.1, SAMPLE, SAMPLE);
        assert!(d.0 >= 280. && d.0 + d.2 <= 1640. && d.1 >= 115. && d.1 + d.3 <= 965.);
        // B and C: the bottom 10 px over the strip's opaque band
        // (screen y 1024-1080 at 1920 x 1080), clear of the session strip.
        for (x, y, _) in [BOX_B, BOX_C] {
            let (sx, sy, sw, sh) = strip_rect(None, (x, y, SAMPLE, SAMPLE));
            assert_eq!((sy, sy + sh), (994., 1034.));
            assert!(sx > 664. && sx + sw < 832.);
        }
        assert_eq!((BOX_B.2, BOX_C.2), (999, 1001));
    }

    #[test]
    fn the_eighty_eights_follow_the_strip() {
        let (lx, ly, lw, lh) = LEVEL_LABEL;
        assert_eq!(
            strip_rect(None, (lx + 100., ly, lw, lh)),
            (830., 1034., 40., 36.)
        );
        // An `ingame` root that does not start at the corner moves it along.
        assert_eq!(
            strip_rect(Some((10., 20., 1920., 1080.)), (lx, ly, lw, lh)),
            (740., 1054., 40., 36.)
        );
    }

    #[test]
    fn nothing_is_drawn_unless_on_in_game_and_drawing() {
        assert!(active(true, true, true));
        for (on, in_game, can_draw) in [
            (false, true, true),
            (false, false, false),
            (true, false, true),
            (true, true, false),
        ] {
            assert!(!active(on, in_game, can_draw));
        }
        // The switch starts off, so `draw` returns at its first check.
        assert!(!ON.load(Relaxed));
    }

    #[test]
    fn the_digits_count_frames_modulo_1000() {
        let mut b = [0u8; 3];
        assert_eq!(digits(7, &mut b), "007");
        assert_eq!(digits(1234, &mut b), "234");
    }
}
