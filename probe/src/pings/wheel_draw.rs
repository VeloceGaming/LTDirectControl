//! The ping wheel's drawing (1.7.1-dev17; the emote wheel's way since
//! 1.7.1-dev14), the author's mock design/pings/mock-wheel-2level.png:
//!
//! - level 1: the hub (26 px) and the ring 34..100 px in two halves split
//!   by the vertical axis, each with its white marker (personal right, team
//!   left) at three screen pixels per art pixel;
//! - level 2: the fan, four 72 px slots around its centre (`gesture::Fan`:
//!   the corner, or the centre still following the cursor), each with its
//!   glyph at four screen pixels per art pixel; the chosen one framed
//!   yellow. Drawn only once the cursor has held still for [`SHOW_PAUSE`]
//!   within [`STILL`] (Maya's rule, spec section 11.1), never because of a
//!   corner; once drawn it stays until the fan closes. A stroke that turns
//!   and releases without a pause fires with no fan drawn. The ring is
//!   drawn at once;
//! - the tooltip strip beside the chosen slot (outward), the only words:
//!   the slot's name and, on a second line, the place it reads
//!   ([`tooltip`]; one line since 1.7.1-dev17, two since dev18);
//! - the stroke's faint ink trail (drawn lines, the emote wheel's line
//!   primitive, drawn by the window).
//!
//! All in the emote wheel's materials, as image nodes of the ping window
//! drawn the way the emote wheel draws (`emotes.rs`): PNG pictures with
//! their colours baked in (`tools/generate_ping_assets.py`), each node's
//! place, size, source and `color: #ffffffAA` (its fade) written every
//! frame. Pure: the wheel's state in, node properties out.
//!
//! The drawing only follows the gesture: the fan's geometry is the
//! gesture's; the pause and the motion here (fade in, the fan's growth, the
//! hovered crossfade, the fire's pulse) never gate a selection or a fire.
//!
//! Superseded 2026-11-04 (dev17): dev14 to dev16's four fixed sectors
//! 62..148 px, the centre disc and the sub-menu of variants.
use super::gesture::{Fan, Selection, LEFT, MOTION, RIGHT, RIM, RING_IN, SLOT, SUB_DEAD};
use super::intent::{Glyph, Slot};
use std::time::{Duration, Instant};

// ---- Timing ----
// The wheel's timings (the author's values after dev13, 2026-11-04; to
// tune in play). Drawing only.

/// The wheel fades in over this long, from the press.
pub const FADE_IN: Duration = Duration::from_millis(60);
/// The fan grows from its centre over this long.
pub const FAN_GROW: Duration = Duration::from_millis(60);
/// On fire the chosen slot pulses 1.0, 1.12, 1.0 over this long, while the
/// wheel fades out.
pub const PULSE: Duration = Duration::from_millis(150);
/// A cancelled wheel fades out over this long.
pub const CANCEL_FADE: Duration = Duration::from_millis(80);
/// The hovered fill crossfades over this long.
pub const HOVER: Duration = Duration::from_millis(80);
/// The fan is drawn once the cursor has held still this long (the
/// author's 100 ms, spec section 12.2; dev18 330, Maya's 1/3 s, felt
/// slow)...
pub const SHOW_PAUSE: Duration = Duration::from_millis(100);
/// ...within this many screen pixels (Maya's jitter tolerance).
pub const STILL: f32 = 5.;

/// The wheel node: a 600 px box centred on the wheel, room for the fan.
pub const NODE: f32 = 600.;
/// The halves' and the hub's pictures: 220 px boxes centred on the wheel.
pub const RING_BOX: f32 = 220.;
/// A slot's picture: 80 px, the 72 px square and its shadow.
pub const SLOT_BOX: f32 = 80.;
/// The glyph pictures: four and three screen pixels per art pixel.
pub const SLOT_SIZE: f32 = 52.;
pub const MARKER_SIZE: f32 = 39.;
/// The level-1 ring while the fan is open.
const DIMMED: f32 = 0.5;
/// How far the wheel's centre keeps from the screen's edges: across, the
/// rim, one motion and a slot's dead zone, so both halves' fans can be
/// opened and chosen with a cursor that cannot leave the screen; up and
/// down, the ring. Near an edge a fan may be drawn partly off screen (the
/// out slots sit 78 px further out than the corner, at ±36°); its slots
/// are still chosen by direction. The tooltip keeps itself on screen.
/// Superseded 2026-11-04 (dev18): dev17's rim + 20 px lock + dead zone.
pub const REACH_X: f32 = RIM + MOTION + SUB_DEAD + 8.;
pub const REACH_Y: f32 = RIM + 8.;

/// A ping picture by its file stem.
pub fn source(stem: &str) -> String {
    format!("asset/lt_direct_control/ui/pings/{stem}")
}
/// A glyph's name in the file stems.
pub fn stem(g: Glyph) -> &'static str {
    match g {
        Glyph::Go => "go",
        Glyph::Defend => "defend",
        Glyph::Missing => "missing",
        Glyph::Take => "take",
        Glyph::Contest => "contest",
        Glyph::Leave => "leave",
        Glyph::Push => "push",
        Glyph::Coming => "coming",
        Glyph::Accepted => "accepted",
        Glyph::Busy => "busy",
        Glyph::Declined => "declined",
        Glyph::Assist => "assist",
        Glyph::Poke => "poke",
        Glyph::AllIn => "allin",
        Glyph::Personal => "personal",
        Glyph::Team => "team",
    }
}
/// A glyph's picture at 52, 39, 26 or 13 px.
pub fn glyph(g: Glyph, size: u32) -> String {
    format!("glyph{size}_{}", stem(g))
}
const HALVES: [&str; 2] = ["right", "left"];
const MARKERS: [Glyph; 2] = [Glyph::Personal, Glyph::Team];

/// `color` with its alpha times `a`.
pub fn faded(color: u32, a: f32) -> u32 {
    let alpha = ((color & 0xff) as f32 * a.clamp(0., 1.)).round() as u32;
    (color & 0xffff_ff00) | alpha
}
pub fn ratio(since: Duration, span: Duration) -> f32 {
    (since.as_secs_f32() / span.as_secs_f32()).clamp(0., 1.)
}
/// The pulse's scale at `t` since the fire: up to 1.12 and back.
pub fn pulse(t: Duration) -> f32 {
    1. + 0.12 * (std::f32::consts::PI * ratio(t, PULSE)).sin()
}

/// An image node's properties: shown at `rect` with opacity `a`, or hidden.
pub fn image(rect: Option<(f32, f32, f32, f32)>, a: f32, src: &str) -> String {
    match rect.filter(|_| a > 0.004) {
        Some((x, y, w, h)) => format!(
            "visible: true; x: {x:.1}px; y: {y:.1}px; width: {w:.1}px; height: {h:.1}px; color: #ffffff{:02x}; source: {};",
            (a.clamp(0., 1.) * 255.).round() as u32,
            serde_json::to_string(&source(src)).unwrap_or_default()
        ),
        None => "visible: false;".to_owned(),
    }
}
/// A square of `size` centred on `c`.
pub fn square(c: (f32, f32), size: f32) -> (f32, f32, f32, f32) {
    (c.0 - size / 2., c.1 - size / 2., size, size)
}

/// The hovered fill of each half and slot, crossfading; where the cursor
/// rests and since when (the fan's pause); when the fan was first drawn
/// (its growth). Drawing state only.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hover {
    halves: [f32; 2],
    slots: [f32; 4],
    fan_half: Option<usize>,
    fan_since: Option<Instant>,
    rest: Option<((f32, f32), Instant)>,
    at: Option<Instant>,
}
impl Hover {
    /// The fan is drawn now.
    pub fn fan_shown(&self) -> bool {
        self.fan_since.is_some()
    }
    /// Move toward the selection (one call per frame), with the cursor.
    pub fn update(&mut self, sel: Selection, fan: Option<Fan>, cursor: (f32, f32), now: Instant) {
        let step = self.at.map_or(1., |t| ratio(now.duration_since(t), HOVER));
        self.at = Some(now);
        let half = fan.map(|f| f.half);
        if half != self.fan_half {
            self.fan_half = half;
            self.slots = [0.; 4];
            self.fan_since = None;
        }
        // The pause: the cursor within STILL of where it came to rest.
        let rest = match self.rest {
            Some((p, since)) if (cursor.0 - p.0).hypot(cursor.1 - p.1) <= STILL => (p, since),
            _ => (cursor, now),
        };
        self.rest = Some(rest);
        if half.is_some()
            && self.fan_since.is_none()
            && now.saturating_duration_since(rest.1) >= SHOW_PAUSE
        {
            self.fan_since = Some(now);
        }
        for (h, v) in self.halves.iter_mut().enumerate() {
            let target = f32::from(u8::from(sel.half() == Some(h)));
            *v += (target - *v) * step;
        }
        for (n, v) in self.slots.iter_mut().enumerate() {
            let target = f32::from(u8::from(sel.slot().map(|s| s.1) == Some(n)));
            *v += (target - *v) * step;
        }
    }
}

/// What the wheel shows this frame.
pub struct View {
    pub center: (f32, f32),
    pub selection: Selection,
    pub fan: Option<Fan>,
    pub hover: Hover,
    /// The wheel opened.
    pub opened: Instant,
    /// Closing: since when, and the selection fired (None: cancelled).
    pub closing: Option<(Instant, Option<Selection>)>,
    pub now: Instant,
}

/// The wheel's nodes, relative to the window, in [`nodes`]' order.
pub const NODES: [&str; 20] = [
    "wheel",
    "wheel.half0",
    "wheel.half1",
    "wheel.hot0",
    "wheel.hot1",
    "wheel.hub",
    "wheel.marker0",
    "wheel.marker1",
    "wheel.slot0",
    "wheel.slot1",
    "wheel.slot2",
    "wheel.slot3",
    "wheel.slothot0",
    "wheel.slothot1",
    "wheel.slothot2",
    "wheel.slothot3",
    "wheel.glyph0",
    "wheel.glyph1",
    "wheel.glyph2",
    "wheel.glyph3",
];

/// The wheel's opacity at `now`, and the fired selection's pulse.
fn opacity(v: &View) -> (f32, Option<(Selection, f32)>) {
    let open = ratio(v.now.duration_since(v.opened), FADE_IN);
    match v.closing {
        Some((at, fired)) => {
            let t = v.now.duration_since(at);
            (
                open * (1. - ratio(t, closing_span(fired.is_some()))),
                fired.map(|s| (s, pulse(t))),
            )
        }
        None => (open, None),
    }
}
/// How long a closing wheel is drawn: the pulse on a fire, the fade on a
/// cancel.
pub fn closing_span(fired: bool) -> Duration {
    if fired {
        PULSE
    } else {
        CANCEL_FADE
    }
}

/// Every wheel node's properties this frame (`NODES` order: each written
/// every frame, shown or hidden), or None when nothing shows.
pub fn nodes(v: &View) -> Option<[String; 20]> {
    let (a, fired) = opacity(v);
    if a <= 0.004 {
        return None;
    }
    // Inside the wheel node, whose centre is (300, 300).
    let half = NODE / 2.;
    let c = (half, half);
    let local = |p: (f32, f32)| (c.0 + p.0 - v.center.0, c.1 + p.1 - v.center.1);
    let mut out: [String; 20] = std::array::from_fn(|_| "visible: false;".to_owned());
    out[0] = format!(
        "visible: true; x: {:.0}px; y: {:.0}px;",
        v.center.0 - half,
        v.center.1 - half
    );
    let fan = v.fan.filter(|_| v.hover.fan_shown());
    let ring = if fan.is_some() { a * DIMMED } else { a };
    let marker_at = (RING_IN + RIM) / 2.;
    for h in [RIGHT, LEFT] {
        let name = format!("half_{}", HALVES[h]);
        out[1 + h] = image(Some(square(c, RING_BOX)), ring, &name);
        out[3 + h] = image(
            Some(square(c, RING_BOX)),
            ring * v.hover.halves[h],
            &format!("{name}_hot"),
        );
        let x = if h == RIGHT { marker_at } else { -marker_at };
        out[6 + h] = image(
            Some(square((c.0 + x, c.1), MARKER_SIZE)),
            ring,
            &glyph(MARKERS[h], 39),
        );
    }
    out[5] = image(Some(square(c, RING_BOX)), ring, "hub");
    if let Some(fan) = fan {
        let grow = v
            .hover
            .fan_since
            .map_or(1., |t| ratio(v.now.saturating_duration_since(t), FAN_GROW));
        let at = local(fan.center);
        for n in 0..4 {
            let k = fired
                .filter(|(s, _)| *s == Selection::Slot(fan.half, n))
                .map_or(1., |(_, k)| k)
                * grow;
            let p = local(fan.slot(n));
            let p = (at.0 + (p.0 - at.0) * grow, at.1 + (p.1 - at.1) * grow);
            let b = a * grow;
            out[8 + n] = image(Some(square(p, SLOT_BOX * k)), b, "slot");
            out[12 + n] = image(
                Some(square(p, SLOT_BOX * k)),
                b * v.hover.slots[n],
                "slot_hot",
            );
            if let Some(slot) = Slot::at(fan.half, n) {
                out[16 + n] = image(Some(square(p, SLOT_SIZE * k)), b, &glyph(slot.glyph(), 52));
            }
        }
    }
    Some(out)
}
/// The stroke's ink trail: segments from the press point, faint.
pub const TRAIL_COLOR: u32 = 0xeeecec59;
pub const TRAIL_WIDTH: f32 = 2.;

/// The tooltip's strip (drawn) and its two labels, one per line (spec
/// section 11.3): a fixed width, no text measured; a long translation may
/// clip, nothing overlaps. Each line [`TIP_LINE`] high, [`TIP_PAD`] inside
/// the strip on every side.
pub const TIP_WIDTH: f32 = 176.;
pub const TIP_LINE: f32 = 22.;
pub const TIP_PAD: f32 = 8.;
/// A label's width inside the strip.
pub const TIP_LABEL: f32 = TIP_WIDTH - 2. * TIP_PAD;
pub const TIP_GAP: f32 = 6.;
pub const TIP_FILL: u32 = 0x1c1a18ee;
pub const TIP_FRAME: u32 = 0x4b4a49ff;
/// The place word's colour (the mock's green, the go glyph's).
pub const TIP_PLACE: u32 = 0x5bd16eff;
/// The labels' font size.
pub const TIP_SIZE: f32 = 14.;
/// The tooltip: the strip's box and the two labels' top left corners
/// (screen pixels), beside the chosen slot on the outer side, kept on the
/// 1920 x 1080 layout. Superseded 2026-11-04 (dev18): dev17's one line with
/// widths estimated from the letters, which overlapped in Chinese and
/// Japanese.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tip {
    pub strip: (f32, f32, f32, f32),
    pub name: (f32, f32),
    pub place: Option<(f32, f32)>,
}
/// `place`: the slot reads a place (the second line).
pub fn tooltip(v: &View, place: bool) -> Option<Tip> {
    let fan = v.fan.filter(|_| v.hover.fan_shown())?;
    let shown = v.closing.and_then(|c| c.1).unwrap_or(v.selection);
    let (h, n) = shown.slot()?;
    if h != fan.half {
        return None;
    }
    let p = fan.slot(n);
    let lines = if place { 2. } else { 1. };
    let (w, height) = (TIP_WIDTH, TIP_PAD * 2. + TIP_LINE * lines);
    // Outward of the slot; where that leaves the screen, its inner side.
    let edge = SLOT / 2. + TIP_GAP;
    let (right, left) = (p.0 + edge, p.0 - edge - w);
    let fits = |x: f32| x >= 0. && x + w <= 1920.;
    let (outward, inward) = if h == RIGHT {
        (right, left)
    } else {
        (left, right)
    };
    let x = if fits(outward) { outward } else { inward };
    let x = x.clamp(0., 1920. - w);
    let y = (p.1 - height / 2.).clamp(0., 1080. - height);
    Some(Tip {
        strip: (x, y, w, height),
        name: (x + TIP_PAD, y + TIP_PAD),
        place: place.then_some((x + TIP_PAD, y + TIP_PAD + TIP_LINE)),
    })
}

/// Every source a property string names is a generated picture (tests).
#[cfg(test)]
pub(crate) fn assert_generated(props: &str) {
    let record: serde_json::Value =
        serde_json::from_str(include_str!("../../../tools/records/ping-assets.json")).unwrap();
    for src in props.split("source: ").skip(1) {
        let name = src.split('"').nth(1).unwrap().rsplit('/').next().unwrap();
        let key = format!("ui/pings/{name}.png");
        assert!(record["files"].get(&key).is_some(), "{key}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_motion_fades_pulses_and_ends() {
        assert_eq!(faded(0xffd54aff, 0.5), 0xffd54a80);
        assert!((pulse(Duration::ZERO) - 1.).abs() < 1e-6);
        assert!((pulse(PULSE / 2) - 1.12).abs() < 1e-3);
        let t = Instant::now();
        let mut h = Hover::default();
        h.update(Selection::Half(LEFT), None, (0., 0.), t);
        assert_eq!(h.halves[LEFT], 1., "the first frame lands");
        h.update(Selection::Half(RIGHT), None, (0., 0.), t + HOVER / 2);
        assert!((h.halves[RIGHT] - 0.5).abs() < 0.01 && (h.halves[LEFT] - 0.5).abs() < 0.01);
        assert_eq!(image(None, 1., "pin"), "visible: false;");
        assert!(image(Some((1., 2., 3., 4.)), 0.5, "pin").contains("color: #ffffff80;"));
    }
    /// The cursor held still long enough: the fan is drawn.
    fn view(sel: Selection, fan: Option<Fan>, t: Instant) -> View {
        let mut hover = Hover::default();
        hover.update(sel, fan, (1., 1.), t);
        hover.update(sel, fan, (3., 2.), t + SHOW_PAUSE);
        View {
            center: (960., 540.),
            selection: sel,
            fan,
            hover,
            opened: t,
            closing: None,
            now: t + SHOW_PAUSE + FADE_IN + FAN_GROW,
        }
    }
    #[test]
    fn every_node_names_a_generated_picture_and_a_closed_wheel_shows_nothing() {
        let t = Instant::now();
        let fan = Fan {
            half: RIGHT,
            center: (1080., 540.),
            corner: true,
        };
        let v = view(Selection::Slot(RIGHT, 1), Some(fan), t);
        let all = nodes(&v).unwrap();
        all.iter().for_each(|p| assert_generated(p));
        assert!(all[1].contains("#ffffff80"), "the ring dims to half");
        assert!(all[17].contains("glyph52_coming"), "{}", all[17]);
        assert!(all[6].contains("glyph39_personal") && all[7].contains("glyph39_team"));
        // Slot 0 up at -108° from the fan's centre, 96 px.
        assert!(all[8].contains("x: 350.3px; y: 168.7px;"), "{}", all[8]);
        let left = Fan {
            half: LEFT,
            center: (840., 540.),
            corner: true,
        };
        let all = nodes(&view(Selection::Slot(LEFT, 3), Some(left), t)).unwrap();
        all.iter().for_each(|p| assert_generated(p));
        assert!(all[19].contains("glyph52_allin"));
        // No fan: no slots.
        let plain = nodes(&view(Selection::Centre, None, t)).unwrap();
        assert_eq!(plain[8], "visible: false;");
        let mut v = view(Selection::Centre, None, t);
        v.closing = Some((v.now, None));
        assert!(nodes(&v).is_some());
        v.now += CANCEL_FADE;
        assert!(nodes(&v).is_none());
    }
    #[test]
    fn the_tooltip_sits_outward_of_the_chosen_slot_and_on_screen() {
        let t = Instant::now();
        let fan = Fan {
            half: RIGHT,
            center: (1080., 540.),
            corner: true,
        };
        let v = view(Selection::Slot(RIGHT, 1), Some(fan), t);
        let tip = tooltip(&v, true).unwrap();
        assert!(tip.strip.0 > fan.slot(1).0 + SLOT / 2.);
        // Two lines, one above the other, a fixed width: no words in.
        let place = tip.place.unwrap();
        assert_eq!((place.0, place.1 - tip.name.1), (tip.name.0, TIP_LINE));
        assert_eq!(tip.strip.2, TIP_WIDTH);
        assert_eq!(tooltip(&v, false).unwrap().strip.3, TIP_PAD * 2. + TIP_LINE);
        let left = Fan {
            half: LEFT,
            center: (150., 540.),
            corner: true,
        };
        let v = view(Selection::Slot(LEFT, 0), Some(left), t);
        // Too near the left edge for outward: the slot's inner side.
        let tip = tooltip(&v, true).unwrap();
        assert!(tip.strip.0 >= left.slot(0).0 + SLOT / 2. && tip.strip.0 + tip.strip.2 <= 1920.);
        let room = Fan {
            half: LEFT,
            center: (700., 540.),
            corner: true,
        };
        let v = view(Selection::Slot(LEFT, 0), Some(room), t);
        let tip = tooltip(&v, true).unwrap();
        assert!(tip.strip.0 >= 0. && tip.strip.0 + tip.strip.2 <= room.slot(0).0 - SLOT / 2.);
        assert!(tooltip(&view(Selection::Half(LEFT), Some(left), t), false).is_none());
        const { assert!(REACH_X >= RIM + MOTION + SUB_DEAD && REACH_Y > RIM) };
    }
    #[test]
    fn the_fan_is_drawn_only_after_a_pause_and_then_stays() {
        let t = Instant::now();
        let fan = Fan {
            half: RIGHT,
            center: (1080., 540.),
            corner: true,
        };
        let sel = Selection::Slot(RIGHT, 0);
        let mut h = Hover::default();
        // Moving every frame: never drawn, corner or not.
        for i in 0..20 {
            h.update(
                sel,
                Some(fan),
                (1100. + 10. * i as f32, 500.),
                t + HOVER * i,
            );
        }
        assert!(!h.fan_shown());
        let moving = View {
            center: (960., 540.),
            selection: sel,
            fan: Some(fan),
            hover: h,
            opened: t,
            closing: None,
            now: t + HOVER * 20,
        };
        assert_eq!(
            nodes(&moving).unwrap()[8],
            "visible: false;",
            "no slot drawn"
        );
        assert!(tooltip(&moving, true).is_none());
        // Held within STILL for the pause: drawn, and kept when it moves.
        let at = t + HOVER * 20;
        h.update(sel, Some(fan), (1300., 500.), at);
        h.update(sel, Some(fan), (1303., 502.), at + SHOW_PAUSE / 2);
        assert!(!h.fan_shown());
        h.update(sel, Some(fan), (1301., 499.), at + SHOW_PAUSE);
        assert!(h.fan_shown());
        h.update(sel, Some(fan), (1400., 400.), at + SHOW_PAUSE + HOVER);
        assert!(h.fan_shown(), "stays until the fan closes");
        h.update(
            Selection::Half(RIGHT),
            None,
            (1400., 400.),
            at + SHOW_PAUSE * 2,
        );
        assert!(!h.fan_shown(), "closed");
    }
}
