//! The answer stack (1.7.1-dev15, design/pings/stack-pixel.svg): top left
//! (its place: `wheel.rs`, `STACK_AT`). The ping row first (the order's glyph,
//! the target's portrait for a fight call, the countdown as blocks, its
//! top line in the glyph's colour), then one row per teammate who answered,
//! in roster order (top, jungle, mid, bot, support): his portrait and the
//! answer's glyph ([`answer_glyph`]). No words. Since 1.7.1-dev16 every
//! recipient has a row the moment the ping fires, waiting (the ping's own
//! glyph, dim) until the answer comes; a row slides in and fades in over
//! [`SLIDE`], holds [`HOLD`] once answered, fades over [`FADE`]; a changed
//! answer pulses and holds again. A ping nobody hears shows the ping row
//! only. Replaces dev14's card above the minimap.
//!
//! Pure: the answers in, the drawn boxes (the draw list) and the image
//! nodes' properties out. The portraits are image nodes the window fills
//! with the game's champion icons (`ui_set_champion_icon`, as the Tab
//! panel); without a name a dark square stands in.
use super::intent::Glyph;
use super::wheel_draw::{faded, glyph, image, pulse, ratio};
use crate::draw_list::{DrawList, Rect};
use crate::native_adapter::ping_orders::Status;
use std::time::{Duration, Instant};

/// The ping row's countdown: the pin's life.
pub const LIFE: Duration = super::mark::LIFE;
pub const SLIDE: Duration = Duration::from_millis(150);
pub const HOLD: Duration = Duration::from_secs(3);
pub const FADE: Duration = Duration::from_millis(300);
/// A row: 208 x 56 with the 2 px corner cut, 62 px apart (1.7.1-dev21, the
/// author's spec section 14.1: grown for the Tab panel's 44 x 48 portrait;
/// 208 keeps the twelve countdown blocks beside the fight call's
/// portrait). Superseded 2026-11-04: dev16's 184 x 34, 40 apart (the
/// design's 200 narrowed to end left of x 190).
pub const ROW: (f32, f32) = (208., 56.);
/// A portrait: the Tab panel's 44 x 48 box (`team_status.rs`), in a 2 px
/// frame, centred on the row's height.
pub const FACE: (f32, f32) = (44., 48.);
const FACE_FRAME: (f32, f32) = (FACE.0 + 4., FACE.1 + 4.);
/// The glyphs: 26 px, centred on the row's height.
const GLYPH: f32 = 26.;
/// Each answer over its teammate's champion (1.7.1-dev18, spec section
/// 11.2): shown this long from its last change, then faded over [`FADE`].
pub const ANSWER_SHOWN: Duration = Duration::from_secs(3);
/// The most rows: the ping row and five answers.
pub const HEIGHT: f32 = PITCH * 5. + ROW.1;
const PITCH: f32 = 62.;
const BLOCKS: usize = 12;
const FILL: u32 = 0x1a1510cc;
const FRAME: u32 = 0xfff3c473;
const SPENT: u32 = 0xfff3c438;
const STAND_IN: u32 = 0x384860ff;
/// The stack's z order: boxes under the nodes.
const Z: i32 = 1110;

#[derive(Clone, Copy, Debug, Default)]
struct Row {
    status: Option<Status>,
    arrived: Option<Instant>,
    changed: Option<Instant>,
    y: (f32, f32),
    moved: Option<Instant>,
}
impl Row {
    /// Its opacity at `now`; None once it ended.
    fn alpha(&self, now: Instant) -> Option<f32> {
        let (arrived, changed) = (self.arrived?, self.changed?);
        let fade_in = ratio(now.saturating_duration_since(arrived), SLIDE);
        // Waiting holds until the answer comes (or the stack ends).
        if self.status == Some(Status::Pending) {
            return Some(fade_in);
        }
        let since = now.saturating_duration_since(changed);
        if since >= HOLD + FADE {
            return None;
        }
        let fade_out = 1. - ratio(since.saturating_sub(HOLD), FADE);
        Some(fade_in * fade_out)
    }
    fn end(&self) -> Option<Instant> {
        self.changed
            .filter(|_| self.status != Some(Status::Pending))
            .map(|c| c + HOLD)
    }
}

/// A dim glyph (waiting, sent, unsent) is drawn at this alpha.
pub const DIM: f32 = 0.4;
/// A row's glyph for a status, and whether it is dim (1.7.1-dev16): the
/// ping's own glyph while waiting; a check for accepted or on it, dim for
/// sent; an hourglass for busy; a cross for declined, dim for unsent.
pub fn answer_glyph(s: Status, ping: Glyph) -> (Glyph, bool) {
    match s {
        Status::Pending => (ping, true),
        Status::Accepted => (Glyph::Accepted, false),
        Status::Sent => (Glyph::Accepted, true),
        Status::Busy => (Glyph::Busy, false),
        Status::Declined => (Glyph::Declined, false),
        Status::Unsent => (Glyph::Declined, true),
    }
}

/// The image nodes the stack writes, in [`Stack::draw`]'s order.
pub const NODES: [&str; 12] = [
    "stack_glyph",
    "stack_face",
    "row_face0",
    "row_face1",
    "row_face2",
    "row_face3",
    "row_face4",
    "row_answer0",
    "row_answer1",
    "row_answer2",
    "row_answer3",
    "row_answer4",
];

pub struct Stack {
    pub glyph: Glyph,
    /// The ping's clock (a repeat starts it again).
    pub since: Instant,
    rows: [Row; 5],
}
impl Stack {
    pub fn new(glyph: Glyph, since: Instant) -> Self {
        Self {
            glyph,
            since,
            rows: [Row::default(); 5],
        }
    }
    /// This frame's answers by teammate index.
    pub fn update(&mut self, answers: [Option<Status>; 5], now: Instant) {
        for (row, answer) in self.rows.iter_mut().zip(answers) {
            let Some(s) = answer else {
                continue;
            };
            if row.status != Some(s) {
                if row.alpha(now).is_none() {
                    row.arrived = Some(now);
                }
                row.changed = Some(now);
                row.status = Some(s);
            }
        }
    }
    /// When the ping row ends: its countdown, or the last answer's hold.
    fn end(&self) -> Instant {
        self.rows
            .iter()
            .filter_map(Row::end)
            .fold(self.since + LIFE, Instant::max)
    }
    /// The stack has ended (nothing more to draw).
    pub fn ended(&self, now: Instant) -> bool {
        now >= self.end() + FADE
    }
    /// Each teammate's answer over his champion now (spec section 11.2):
    /// the row's glyph ([`answer_glyph`]) and its opacity, dim ones at
    /// [`DIM`], for [`ANSWER_SHOWN`] from the last change, then fading.
    /// Every order, every recipient, including one still waiting.
    pub fn heads(&self, now: Instant) -> [Option<(Glyph, f32)>; 5] {
        std::array::from_fn(|i| {
            let row = &self.rows[i];
            let (s, changed) = (row.status?, row.changed?);
            let since = now.saturating_duration_since(changed);
            if since >= ANSWER_SHOWN + FADE {
                return None;
            }
            let a = 1. - ratio(since.saturating_sub(ANSWER_SHOWN), FADE);
            let (g, dim) = answer_glyph(s, self.glyph);
            Some((g, if dim { a * DIM } else { a }))
        })
    }
    /// The teammates whose row shows now (their portraits are wanted).
    pub fn shown(&self, now: Instant) -> [bool; 5] {
        std::array::from_fn(|i| self.rows[i].alpha(now).is_some())
    }

    /// Draws at `origin` (the stack's top left): its boxes into `list`, the
    /// nodes' properties ([`NODES`] order) out, and its bounds. `faces` says
    /// which portraits are filled (the target's, then each teammate's);
    /// without one a dark square stands in.
    pub fn draw(
        &mut self,
        list: &mut DrawList,
        origin: (f32, f32),
        faces: (bool, [bool; 5]),
        now: Instant,
    ) -> (Rect, [String; 12]) {
        let mut out: [String; 12] = std::array::from_fn(|_| "visible: false;".to_owned());
        let a = 1. - ratio(now.saturating_duration_since(self.end()), FADE);
        let (x, y) = origin;
        // The ping row.
        backing(list, (x, y), a);
        list.rect(
            (x + 2., y, ROW.0 - 4., 2.),
            Z + 1,
            0.,
            faded(self.glyph.color(), a),
        );
        let mid = y + ROW.1 / 2.;
        out[0] = image(
            Some((x + 4., mid - GLYPH / 2., GLYPH, GLYPH)),
            a,
            &glyph(self.glyph, 26),
        );
        let mut blocks_at = x + 4. + GLYPH + 4.;
        if self.glyph == Glyph::Contest {
            out[1] = portrait(list, (blocks_at, mid - FACE_FRAME.1 / 2.), faces.0, a);
            blocks_at += FACE_FRAME.0 + 4.;
        }
        let left = 1. - ratio(now.saturating_duration_since(self.since), LIFE);
        let lit = (left * BLOCKS as f32).ceil() as usize;
        for b in 0..BLOCKS {
            let color = if b < lit { self.glyph.color() } else { SPENT };
            list.rect(
                (blocks_at + 10. * b as f32, mid - 5., 8., 10.),
                Z + 1,
                0.,
                faded(color, a),
            );
        }
        // The answers, in roster order.
        let mut rank = 0.;
        for i in 0..5 {
            let row = &mut self.rows[i];
            let (Some(alpha), Some(s)) = (row.alpha(now), row.status) else {
                continue;
            };
            rank += 1.;
            let target = y + PITCH * rank;
            if row.y.1 != target {
                let shown = if row.moved.is_some() {
                    lerp(
                        row.y,
                        ratio(
                            now.saturating_duration_since(row.moved.unwrap_or(now)),
                            SLIDE,
                        ),
                    )
                } else {
                    target
                };
                row.y = (shown, target);
                row.moved = Some(now);
            }
            let ry = lerp(
                row.y,
                ratio(
                    now.saturating_duration_since(row.moved.unwrap_or(now)),
                    SLIDE,
                ),
            );
            let arrive = ratio(
                now.saturating_duration_since(row.arrived.unwrap_or(now)),
                SLIDE,
            );
            let rx = x - 16. * (1. - arrive);
            let b = alpha * a;
            backing(list, (rx, ry), b);
            out[2 + i] = portrait(
                list,
                (rx + 4., ry + (ROW.1 - FACE_FRAME.1) / 2.),
                faces.1[i],
                b,
            );
            // A changed answer pulses.
            let k = match (row.arrived, row.changed) {
                (Some(arr), Some(ch)) if ch > arr => pulse(now.saturating_duration_since(ch)),
                _ => 1.,
            };
            let (g, dim) = answer_glyph(s, self.glyph);
            let size = GLYPH * k;
            let c = (rx + ROW.0 - 4. - GLYPH / 2., ry + ROW.1 / 2.);
            out[7 + i] = image(
                Some((c.0 - size / 2., c.1 - size / 2., size, size)),
                if dim { b * DIM } else { b },
                &glyph(g, 26),
            );
        }
        ((x - 16., y, ROW.0 + 16., PITCH * rank + ROW.1), out)
    }
}
fn lerp((from, to): (f32, f32), t: f32) -> f32 {
    from + (to - from) * t
}
/// A row's dark box with its 2 px corner cut, as three boxes.
fn backing(list: &mut DrawList, (x, y): (f32, f32), a: f32) {
    let fill = faded(FILL, a);
    list.rect((x + 2., y, ROW.0 - 4., 2.), Z, 0., fill);
    list.rect((x, y + 2., ROW.0, ROW.1 - 4.), Z, 0., fill);
    list.rect((x + 2., y + ROW.1 - 2., ROW.0 - 4., 2.), Z, 0., fill);
}
/// A portrait frame at `at` ([`FACE`] and 2 px around it): the pale frame
/// drawn, the portrait node 2 px inside (its properties), or a dark box
/// standing in. The node's
/// size is the icon call's fitted box (1.7.1-dev20): only its place and
/// fade are written. Superseded 2026-11-04: dev15 to dev19 wrote 22 x 22
/// every frame.
fn portrait(list: &mut DrawList, at: (f32, f32), filled: bool, a: f32) -> String {
    list.rect(
        (at.0, at.1, FACE_FRAME.0, FACE_FRAME.1),
        Z + 1,
        0.,
        faded(FRAME, a),
    );
    let inner = (at.0 + 2., at.1 + 2., FACE.0, FACE.1);
    if filled && a > 0.004 {
        format!(
            "visible: true; x: {:.1}px; y: {:.1}px; color: #ffffff{:02x};",
            inner.0,
            inner.1,
            (a * 255.).round() as u32
        )
    } else {
        list.rect(inner, Z + 2, 0., faded(STAND_IN, a));
        "visible: false;".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rows_arrive_in_roster_order_hold_three_seconds_and_fade() {
        let t = Instant::now();
        let mut s = Stack::new(Glyph::Contest, t);
        let mut answers = [None; 5];
        answers[3] = Some(Status::Busy);
        answers[0] = Some(Status::Pending);
        s.update(answers, t);
        assert_eq!(
            s.shown(t + SLIDE),
            [true, false, false, true, false],
            "waiting has a dim row"
        );
        let mut list = DrawList::default();
        let (_, waiting) = s.draw(&mut list, (8., 193.), (false, [false; 5]), t + SLIDE);
        assert!(
            waiting[7].contains("glyph26_contest") && waiting[7].contains("#ffffff66"),
            "{}",
            waiting[7]
        );
        answers[0] = Some(Status::Accepted);
        s.update(answers, t + Duration::from_millis(500));
        let mut list = DrawList::default();
        let (_, nodes) = s.draw(
            &mut list,
            (8., 193.),
            (false, [false; 5]),
            t + Duration::from_secs(1),
        );
        // Top first: row 0 at the first answer place, row 3 below it.
        assert!(
            nodes[2].is_empty() || nodes[2] == "visible: false;",
            "no portrait: a stand-in"
        );
        assert!(
            nodes[7].contains("glyph26_accepted") && nodes[7].contains("y: 270.0px"),
            "{}",
            nodes[7]
        );
        assert!(nodes[10].contains("glyph26_busy"));
        for n in &nodes {
            super::super::wheel_draw::assert_generated(n);
        }
        // A changed answer holds again from its change.
        answers[3] = Some(Status::Accepted);
        s.update(answers, t + Duration::from_secs(2));
        let late = t + Duration::from_secs(2) + HOLD;
        assert_eq!(s.shown(late), [false, false, false, true, false]);
        // A waiting row lasts until the stack ends, and does not hold it.
        let mut w = Stack::new(Glyph::Take, t);
        w.update([Some(Status::Pending), None, None, None, None], t);
        assert!(w.shown(t + HOLD + FADE)[0]);
        assert!(w.ended(t + LIFE + FADE));
        assert!(!s.ended(late));
        assert!(s.ended(late + FADE));
    }
    #[test]
    fn every_answer_shows_over_its_champion_for_three_seconds_from_its_change() {
        let t = Instant::now();
        let mut s = Stack::new(Glyph::Take, t);
        let mut answers = [None; 5];
        answers[2] = Some(Status::Pending);
        answers[4] = Some(Status::Pending);
        s.update(answers, t);
        let heads = s.heads(t);
        assert_eq!(
            heads[2],
            Some((Glyph::Take, DIM)),
            "waiting: the ping's glyph, dim"
        );
        assert_eq!(heads[0], None, "not sent to him");
        answers[2] = Some(Status::Accepted);
        s.update(answers, t + Duration::from_secs(2));
        let later = t + ANSWER_SHOWN + Duration::from_millis(500);
        assert_eq!(s.heads(later)[2], Some((Glyph::Accepted, 1.)));
        assert_eq!(s.heads(later)[4], None, "three seconds of waiting shown");
        assert_eq!(
            s.heads(t + Duration::from_secs(2) + ANSWER_SHOWN + FADE)[2],
            None
        );
    }
    #[test]
    fn the_ping_row_counts_down_its_pin_and_a_repeat_starts_again() {
        let t = Instant::now();
        let mut s = Stack::new(Glyph::Take, t);
        assert!(!s.ended(t + LIFE));
        assert!(s.ended(t + LIFE + FADE));
        s.since = t + Duration::from_secs(3);
        assert!(!s.ended(t + LIFE + FADE));
        assert_eq!(
            answer_glyph(Status::Declined, Glyph::Take),
            (Glyph::Declined, false)
        );
        assert_eq!(
            answer_glyph(Status::Unsent, Glyph::Take),
            (Glyph::Declined, true)
        );
        assert_eq!(
            answer_glyph(Status::Sent, Glyph::Take),
            (Glyph::Accepted, true)
        );
        assert_eq!(
            answer_glyph(Status::Pending, Glyph::Take),
            (Glyph::Take, true)
        );
    }
}
