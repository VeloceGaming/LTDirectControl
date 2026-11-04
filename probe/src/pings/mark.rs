//! The ping's pin (1.7.1-dev15): a placeholder pixel pin in the author's
//! style (a dark box with a 2 px outline and a point, `pin.png`) with the
//! ORDER's glyph over it, so the player sees how "here" was understood:
//! take, push, defend, contest, leave, I'm coming, the fight call's focus
//! (contest's crossed swords), or the intent's own glyph for a mark only.
//! Above the pinged champion and following him each frame, or at the
//! point; small on the minimap. It lasts [`LIFE`] and fades over the last
//! [`FADE_OUT`]; a repeat of the same ping starts its clock again. A
//! champion the player cannot see has no pin.
//!
//! While a target ping is armed, a small pin with the glyph the click
//! would give follows the cursor ([`preview`]).
//!
//! Over each recipient's champion, every order's answer (1.7.1-dev18,
//! spec section 11.2; [`heads`]): the stack's glyph for that teammate at
//! two screen pixels per art pixel, following him as the pin does, its
//! bottom where a pin's tip would be.
//!
//! All are image nodes of the ping window, moved, sized and faded
//! (`color: #ffffffAA`) every frame as the emote wheel's floating image is.
//!
//! Superseded 2026-11-04 (dev15): dev14's celestial pins with the glyph
//! baked in, the minimap pins by colour, and the card above the minimap
//! (replaced by the answer stack, `stack.rs`).
use super::intent::Glyph;
use super::wheel_draw::{glyph, image};
use crate::camera::CameraFrame;
use std::time::{Duration, Instant};

pub const LIFE: Duration = Duration::from_secs(4);
pub const FADE_OUT: Duration = Duration::from_secs(1);
/// The pin at two screen pixels per art pixel: 34 x 40, its tip at
/// (17, 40), the 26 px glyph at (4, 4).
const PIN: (f32, f32) = (34., 40.);
const PIN_GLYPH: (f32, f32, f32) = (4., 4., 26.);
/// The small pin (minimap, preview) at one: 17 x 20, the 13 px glyph at
/// (2, 2).
const MINI: (f32, f32) = (17., 20.);
const MINI_GLYPH: (f32, f32, f32) = (2., 2., 13.);
/// A champion's pin tip sits this far above his position (screen pixels
/// at zoom 1; a guess from the old mark at 36, raised for the pin).
const ABOVE: f32 = 44.;
/// The preview's tip sits this far right of and above the cursor's point,
/// clear of the ping ring.
const PREVIEW: (f32, f32) = (16., -6.);

/// The answers' image nodes, by teammate index.
pub const HEAD_NODES: [&str; 5] = ["head0", "head1", "head2", "head3", "head4"];
/// The answer glyph's size (the stack's, 13 art pixels at two).
const HEAD: f32 = 26.;

/// Where a pin over a champion at `world` has its tip: on screen, off the
/// minimap, or None.
fn above(frame: CameraFrame, world: (u64, u64)) -> Option<(f32, f32)> {
    let p = frame.project(world)?;
    let tip = (p.0, p.1 - ABOVE * frame.zoom.clamp(0.5, 3.));
    (frame.viewport.contains(tip) && !frame.minimap.contains(tip)).then_some(tip)
}

/// The answers' nodes ([`HEAD_NODES`]): teammate `i`'s glyph and opacity
/// over his champion at `at[i]` (None hides it).
pub fn heads(
    frame: CameraFrame,
    glyphs: [Option<(Glyph, f32)>; 5],
    at: [Option<(u64, u64)>; 5],
) -> [String; 5] {
    std::array::from_fn(|i| {
        let rect = glyphs[i]
            .zip(at[i])
            .and_then(|(_, w)| above(frame, w))
            .map(|tip| (tip.0 - HEAD / 2., tip.1 - HEAD, HEAD, HEAD));
        match glyphs[i] {
            Some((g, a)) => image(rect, a, &glyph(g, 26)),
            None => "visible: false;".to_owned(),
        }
    })
}

/// What the pin stands on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum On {
    /// A champion, by entity id.
    Unit(usize),
    /// A world point.
    Point((u64, u64)),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark {
    pub on: On,
    pub glyph: Glyph,
    pub since: Instant,
}
impl Mark {
    /// Its opacity at `now`, None once it ended.
    pub fn alpha(&self, now: Instant) -> Option<f32> {
        let age = now.checked_duration_since(self.since)?;
        if age >= LIFE {
            return None;
        }
        let left = (LIFE - age).as_secs_f32() / FADE_OUT.as_secs_f32();
        Some(left.min(1.))
    }
}

/// A pin of `size` with its glyph, its tip at `tip`: the two nodes.
fn pin(tip: Option<(f32, f32)>, a: f32, g: Glyph, small: bool) -> [String; 2] {
    let ((w, h), (gx, gy, gs), frame, px) = if small {
        (MINI, MINI_GLYPH, "pin_mini", 13)
    } else {
        (PIN, PIN_GLYPH, "pin", 26)
    };
    let corner = tip.map(|t| (t.0 - w / 2., t.1 - h));
    [
        image(corner.map(|c| (c.0, c.1, w, h)), a, frame),
        image(
            corner.map(|c| (c.0 + gx, c.1 + gy, gs, gs)),
            a,
            &glyph(g, px),
        ),
    ]
}

/// The pins' four nodes (`pin`, `pin_glyph`, `mini`, `mini_glyph`): `at`
/// is the champion's position while the player sees him (None hides a
/// champion's pin).
pub fn nodes(frame: CameraFrame, mark: &Mark, at: Option<(u64, u64)>, now: Instant) -> [String; 4] {
    let hidden = || std::array::from_fn(|_| "visible: false;".to_owned());
    let Some(a) = mark.alpha(now) else {
        return hidden();
    };
    let (world, tip) = match (mark.on, at) {
        (On::Unit(_), Some(p)) => (p, above(frame, p)),
        (On::Unit(_), None) => return hidden(),
        (On::Point(p), _) => (
            p,
            frame
                .project(p)
                .filter(|t| frame.viewport.contains(*t) && !frame.minimap.contains(*t)),
        ),
    };
    let m = frame.project_minimap(world);
    let [p, pg] = pin(tip, a, mark.glyph, false);
    let [m, mg] = pin(frame.minimap.contains(m).then_some(m), a, mark.glyph, true);
    [p, pg, m, mg]
}

/// The armed preview's two nodes (`preview`, `preview_glyph`): the small
/// pin with the glyph the click would give, by the cursor.
pub fn preview(cursor: Option<(f32, f32)>, g: Glyph) -> [String; 2] {
    pin(
        cursor.map(|c| (c.0 + PREVIEW.0, c.1 + PREVIEW.1)),
        1.,
        g,
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_pin_lasts_four_seconds_fades_over_the_last_and_names_its_pictures() {
        let t = Instant::now();
        let m = Mark {
            on: On::Unit(25),
            glyph: Glyph::Contest,
            since: t,
        };
        assert_eq!(m.alpha(t), Some(1.));
        assert_eq!(m.alpha(t + Duration::from_millis(2900)), Some(1.));
        let late = m.alpha(t + Duration::from_millis(3500)).unwrap();
        assert!((late - 0.5).abs() < 0.01);
        assert_eq!(m.alpha(t + LIFE), None);
        let [frame, g] = pin(Some((100., 100.)), 1., Glyph::Take, false);
        assert!(frame.contains("x: 83.0px; y: 60.0px; width: 34.0px; height: 40.0px"));
        assert!(g.contains("x: 87.0px; y: 64.0px; width: 26.0px"));
        for p in preview(Some((10., 10.)), Glyph::Push)
            .iter()
            .chain([frame, g].iter())
        {
            super::super::wheel_draw::assert_generated(p);
        }
        assert_eq!(preview(None, Glyph::Go)[0], "visible: false;");
    }
}
