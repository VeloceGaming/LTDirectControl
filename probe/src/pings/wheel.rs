//! The ping key on the client thread (1.7.1-dev17; built like the emote
//! wheel since 1.7.1-dev1), the author's spec section 10.1, League's model:
//!
//! - **G enters ping mode; the mouse pings.** A tap arms it until the next
//!   left press or a cancel; holding the key keeps it for as long as the
//!   key is down. The key alone never opens the wheel and never fires. In
//!   ping mode the cursor is the ping ring (the on-target ring over a unit
//!   the click would ping) and a small pin with the glyph it would give
//!   follows it; the left button is reserved (`left_reserved`), never a
//!   selection or a move.
//! - **Left click** (press and release within [`DEAD`]): the target ping on
//!   "that", the unit under the press point, else Take here on the ground
//!   there (`intent::click`).
//! - **Left press and drag** past [`DEAD`]: the two-level wheel at the
//!   press point (`gesture.rs`), about "here"; the left release fires the
//!   chosen slot (`intent::wheel`). A release with no slot chosen cancels.
//! - **Cancel:** right click, Esc, a command key, a press over the HUD, or
//!   [`ARMED_FOR`] after the key was released without a press. Every
//!   threshold is a drag distance; no hold time anywhere.
//!
//! A ping goes to the worker (`ping_orders`, the fight call
//! `signal_call::request`, the team strategy `plan_call`); the pin shows
//! its order's glyph (`mark.rs`) and the answer stack the teammates'
//! answers (`stack.rs`). A repeat of the same ping while its pin lives
//! refreshes the pin and asks nothing new of those who answered. On my
//! way's hold ends when the player arrives or stops coming ([`COMING_EVERY`]).
//!
//! A target ping names only what the team sees (`pings::team_sees`): a
//! click on an enemy the team cannot see is a fog ping at that point.
//!
//! **V, the retreat ping** (1.7.1-dev19, spec section 12.4, League's
//! caution ping): enters ping mode exactly as G does, the one difference
//! being the left click, which fires Retreat at the click point (the
//! wheel's Retreat; a unit under the cursor only gives the point, no
//! "that" reading). Its preview shows the leave glyph. A drag in V's mode
//! opens the same wheel. The last key pressed sets the mode.
//!
//! Superseded 2026-11-04: dev15's tap-then-click and hold-for-the-wheel;
//! dev16's key-driven wheel (hold the key and move) and its four slots.
use super::gesture::{Gesture, Selection};
use super::intent::{self, Choice, Clicked, Enemy, Glyph, Here, Order, Place, Slot, Team};
use super::mark::{self, Mark, On};
use super::places;
use super::stack::{self, Stack};
use super::subject;
use super::wheel_draw::{self, Hover};
use crate::{
    camera::{CameraFrame, Rect},
    client::{Client, ClientObservations},
    draw_list::DrawList,
    map_path::Grid,
    native_adapter::{ping_orders, signal_call as call},
    native_timing::MatchKey,
    plan_call,
    platform_input::Keys,
    Logger,
};
use mod_api_stable::StableClient;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

pub const PATH: &str = "ingame.lt_pings";
// Superseded 2026-11-04 (1.7.1-dev21, spec section 14.2): `Shape`, the
// strokes and discs another part of the mod had the window draw (research
// builds: the lever keys' choice marks; none in public builds), with them.
/// The emote wheel's "the cursor has moved" distance (screen pixels; the
/// ping wheel's own threshold is [`DEAD`]).
pub const MOVE_PX: f32 = 2.;
/// Ping mode is cancelled after this long without a press, counted from
/// the key's release.
pub const ARMED_FOR: Duration = Duration::from_secs(3);
/// On my way's hold (1.7.1-dev17): checked this often; it ends when the
/// player is within [`COMING_NEAR`] map units of the laner it was sent to
/// (his unit), when the player's distance to the lane's centre line no
/// longer shrinks between two checks, or after [`COMING_FOR`] (the worker's
/// own cap, `ping_orders::HOLD_SECS`).
pub const COMING_EVERY: Duration = Duration::from_secs(2);
pub const COMING_FOR: Duration = Duration::from_secs(ping_orders::HOLD_SECS);
pub const COMING_NEAR: f32 = 200.;
/// The stack's place (1.7.1-dev16, interim until the author names the
/// in-match log it met in dev15): its top left at (6, 224) of the ingame
/// layout, below the game's lower centre banner (`ingame.center_log`, its
/// messages at y 164 and 214, x 198 to 798), and its bottom (at most 590
/// since 1.7.1-dev21) under 770, above the chat list (`ingame.chat_list`,
/// from y 780). The kill feed (`ingame.kills`, right-aligned near x 848 to
/// 978) is far from it either way. Since 1.7.1-dev21 (spec section 14.1:
/// the row widened to 208 for the Tab panel's portraits) its right edge is
/// x 214, so the lower banner's left end (x 198) can touch its top rows
/// while a message shows; dev16 to dev20 ended at x 190.
const STACK_AT: (f32, f32) = (6., 224.);
#[cfg_attr(not(test), allow(dead_code))]
const STACK_FLOOR: f32 = 780.;

/// The ping wheel is open: the emote wheel stays shut meanwhile.
static WHEEL_OPEN: AtomicBool = AtomicBool::new(false);
pub fn wheel_open() -> bool {
    WHEEL_OPEN.load(Ordering::Relaxed)
}
/// The left click is the ping's (armed, or the firing click still held).
static LEFT_RESERVED: AtomicBool = AtomicBool::new(false);
pub fn left_reserved() -> bool {
    LEFT_RESERVED.load(Ordering::Relaxed)
}
/// The armed cursor: 0 none, 1 the ping ring, 2 over a unit it would ping.
static ARMED: AtomicU8 = AtomicU8::new(0);
/// The cursor style while armed: Some(over a unit the click would ping).
pub fn armed_cursor() -> Option<bool> {
    match ARMED.load(Ordering::Relaxed) {
        1 => Some(false),
        2 => Some(true),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub generation: u64,
    pub key: MatchKey,
    pub player: usize,
}
#[derive(Clone, Copy)]
pub struct Context {
    pub identity: Option<Identity>,
    pub controlling: bool,
    pub running: bool,
    /// The shop or the settings window is open.
    pub blocked: bool,
    pub emote_open: bool,
    /// A skill or attack-move is being aimed.
    pub aiming: bool,
}
/// Where the key went down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    /// The world point under the cursor on the field.
    Field((u64, u64)),
    /// The world point under the cursor on the minimap.
    Minimap((u64, u64)),
}
impl Anchor {
    fn world(self) -> (u64, u64) {
        match self {
            Anchor::Field(w) | Anchor::Minimap(w) => w,
        }
    }
}
/// The wheel while the left button is held.
#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    gesture: Gesture,
    anchor: Anchor,
    opened: Instant,
}
impl Wheel {
    /// The slot a release fires (None: the hub or a half with no slot).
    fn fired(&self) -> Option<Slot> {
        let (h, n) = self.gesture.selection().slot()?;
        Slot::at(h, n)
    }
}
/// A left press waiting to leave the dead zone: where, and its anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Press {
    at: (f32, f32),
    anchor: Anchor,
}
/// A press that moves this far opens the wheel at the press point (the
/// author's 24 px); released inside, it is the quick ping.
pub const DEAD: f32 = 24.;
#[derive(Default)]
struct State {
    identity: Option<Identity>,
    eligible: bool,
    previous_key: bool,
    previous_cancel: bool,
    previous_left: bool,
    wheel: Option<Wheel>,
    /// Ping mode (the armed state), since then (refreshed while the key is
    /// held).
    armed: Option<Instant>,
    /// The key is held since an edge this match saw.
    key_held: bool,
    /// A press inside the dead zone.
    press: Option<Press>,
    drain: bool,
    /// The left button is the ping's until released.
    reserve: bool,
    /// Ping mode came from the retreat key (V): its click is Retreat.
    retreat: bool,
}
/// Either ping key is down (G, or V, the retreat ping).
fn ping_key(k: Keys) -> bool {
    k.ping || k.ping_retreat
}
fn finite(p: &(f32, f32)) -> bool {
    p.0.is_finite() && p.1.is_finite()
}
/// The drawn centre: the press point, moved inward near the screen edges
/// so the ring and the fan stay on screen. Fixed when the wheel opens, so
/// the gesture is measured from it before anything is drawn.
pub fn wheel_center(p: (f32, f32)) -> (f32, f32) {
    let (rx, ry) = (wheel_draw::REACH_X, wheel_draw::REACH_Y);
    (p.0.clamp(rx, 1920. - rx), p.1.clamp(ry, 1080. - ry))
}
fn held_command(k: Keys) -> bool {
    k.escape
        || k.right
        || k.left
        || k.attack_click
        || k.attack_move
        || k.stop
        || k.recall
        || k.abilities.into_iter().any(|b| b)
}
/// A command key that cancels ping mode (the clicks are handled apart).
fn armed_command(k: Keys) -> bool {
    k.attack_move || k.stop || k.recall || k.abilities.into_iter().any(|b| b)
}
/// What one frame of input did.
#[derive(Debug, Default)]
struct Input {
    /// Capture the frame's clicks.
    capture: bool,
    /// The wheel at a release on a slot.
    fired: Option<Wheel>,
    /// The wheel closed without firing, with its last state, for its fade.
    cancelled: Option<Wheel>,
    /// The ping key entered ping mode.
    armed: bool,
    /// Ping mode ended without a ping, and why (the log's words).
    disarmed: Option<&'static str>,
    /// The quick ping, at this point (the press point).
    click: Option<(f32, f32)>,
}
impl State {
    /// League's input model (1.7.1-dev17, spec section 10.1). The ping key
    /// enters ping mode (a tap arms it, holding keeps it); it never opens
    /// the wheel. In it a left click is the quick ping on what is under the
    /// press point; a left press that moves past [`DEAD`] opens the wheel at
    /// the press point, the drag selects and the release fires, also when
    /// the move and the release come in the same frame. Right click, Esc, a
    /// command key, a press over the HUD or [`ARMED_FOR`] after the key's
    /// release cancel.
    ///
    /// Physical edges are tracked even while blocked: a key held through a
    /// takeover, a respawn or a closing window never opens anything.
    /// `open` gives the anchor of a press (None over the HUD); `pingable`
    /// says whether a click there pings (the field or the minimap).
    fn input(
        &mut self,
        k: Keys,
        c: Context,
        now: Instant,
        open: impl Fn((f32, f32)) -> Option<Anchor>,
        pingable: impl Fn((f32, f32)) -> bool,
    ) -> Input {
        if self.identity != c.identity {
            *self = Self {
                identity: c.identity,
                previous_key: ping_key(k),
                previous_cancel: k.right || k.attack_click,
                previous_left: k.left,
                ..Self::default()
            };
        }
        let eligible = c.identity.is_some() && c.controlling && k.focused;
        let available = eligible && c.running && !c.blocked && !c.aiming && !c.emote_open;
        let key_edge = ping_key(k) && !self.previous_key && self.eligible;
        let right_edge = (k.right || k.attack_click) && !self.previous_cancel;
        let left_edge = k.left && !self.previous_left;
        // The key held since an edge this match saw (never one held
        // through a block).
        let key_held = ping_key(k) && (key_edge || self.key_held);
        self.previous_key = ping_key(k);
        self.previous_cancel = k.right || k.attack_click;
        self.previous_left = k.left;
        self.eligible = available;
        let was_open = self.wheel.is_some();
        let active = self.armed.is_some() || self.wheel.is_some() || self.press.is_some();
        let mut out = Input::default();
        let cancel = if !available || k.shop || k.start || k.release {
            Some("blocked")
        } else if k.escape {
            Some("esc")
        } else if right_edge {
            Some("right_click")
        } else if armed_command(k) {
            Some("command")
        } else {
            None
        };
        self.key_held = key_held && cancel.is_none();
        if let Some(why) = cancel.filter(|_| active) {
            out.cancelled = self.wheel.take();
            self.press = None;
            self.armed = None;
            out.disarmed = Some(why);
            if right_edge {
                // The cancelling right click is not a move order.
                self.drain = true;
            }
        } else if cancel.is_none() {
            if key_edge {
                out.armed = self.armed.is_none();
                // G wins when both go down together.
                self.retreat = k.ping_retreat && !k.ping;
            }
            // Held: ping mode, the 3 s counted from the release.
            if self.key_held {
                self.armed = Some(now);
            }
            if left_edge && self.armed.is_some() && self.wheel.is_none() && self.press.is_none() {
                let press = k
                    .cursor
                    .filter(|p| finite(p) && pingable(*p))
                    .and_then(|p| {
                        Some(Press {
                            at: p,
                            anchor: open(p)?,
                        })
                    });
                match press {
                    Some(p) => {
                        self.press = Some(p);
                        self.reserve = true;
                    }
                    // Over the HUD: the click is the HUD's.
                    None => {
                        self.armed = None;
                        out.disarmed = Some("hud");
                    }
                }
            }
            // A press: out of the dead zone opens the wheel at the press
            // point (whether or not the button is still down: distance,
            // never time); released inside, the quick ping.
            if let (Some(p), None) = (self.press, self.wheel) {
                let far = k
                    .cursor
                    .filter(finite)
                    .is_some_and(|q| (q.0 - p.at.0).hypot(q.1 - p.at.1) > DEAD);
                if far {
                    self.press = None;
                    self.wheel = Some(Wheel {
                        gesture: Gesture::new(p.at, wheel_center(p.at)),
                        anchor: p.anchor,
                        opened: now,
                    });
                } else if !k.left {
                    self.press = None;
                    out.click = Some(p.at);
                    self.armed = self.key_held.then_some(now);
                }
            }
            if let Some(w) = self.wheel.as_mut() {
                match k.cursor.filter(finite) {
                    Some(p) => w.gesture.update(p),
                    None => {
                        out.cancelled = self.wheel.take();
                        out.disarmed = Some("blocked");
                        self.armed = None;
                    }
                }
            }
            // The left release: a slot fires; the hub or a half with no
            // slot cancels. Ping mode ends with the press (it stays while
            // the key is held).
            if let Some(w) = self.wheel.filter(|_| !k.left) {
                self.wheel = None;
                self.armed = self.key_held.then_some(now);
                if w.fired().is_some() {
                    out.fired = Some(w);
                } else {
                    out.cancelled = Some(w);
                    out.disarmed = Some("no_slot");
                }
            }
            let idle = self.wheel.is_none() && self.press.is_none() && !self.key_held;
            if let Some(since) = self.armed.filter(|_| idle) {
                if now.saturating_duration_since(since) >= ARMED_FOR {
                    self.armed = None;
                    out.disarmed = Some("timeout");
                }
            }
        }
        if self.reserve && !k.left {
            self.reserve = false;
        }
        // Clicks and skills pressed while choosing are drained, not
        // replayed as orders on the next frame.
        if was_open || self.wheel.is_some() {
            self.drain = true;
        }
        out.capture = available && (self.drain || self.wheel.is_some() || self.reserve);
        if self.wheel.is_none() && !ping_key(k) && !held_command(k) || !eligible {
            self.drain = false;
        }
        out
    }
    /// The left click belongs to the ping (ping mode, or its press held).
    fn reserved(&self) -> bool {
        self.armed.is_some() || self.reserve
    }
}

/// The order a ping sent, for the stack's answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ref {
    Fight(u64),
    Order(u64),
    /// A team strategy window, with whose rows the stack shows.
    Plan(u64, intent::Mask),
    /// A take at a pit the mod walks (1.7.1-dev22): the walk's rows.
    Walk(u64),
}
impl Ref {
    fn answers(self) -> [Option<ping_orders::Status>; 5] {
        match self {
            Ref::Fight(serial) => call::fight_answers(serial).unwrap_or_default(),
            Ref::Order(serial) => ping_orders::answers(serial),
            Ref::Plan(serial, to) => plan_call::answers(serial, to),
            Ref::Walk(serial) => super::walk::answers(serial),
        }
    }
}
/// The latest ping, for repeats: its choice, its order's reference, and
/// the enemy a fight call names.
#[derive(Clone, Copy, Debug)]
struct Last {
    choice: Choice,
    reference: Option<Ref>,
    enemy: Option<usize>,
}
/// On my way's hold while the player is on the way.
#[derive(Clone, Copy, Debug)]
struct Coming {
    serial: u64,
    lane: u8,
    to: intent::Mask,
    since: Instant,
    /// The last check and the player's distance to the lane then.
    last: Instant,
    gap: Option<f32>,
}
/// A wheel being drawn after it closed: the fire's pulse, the fade.
struct Closing {
    wheel: Wheel,
    hover: Hover,
    at: Instant,
    fired: Option<Selection>,
}

#[derive(Default)]
pub struct Pings {
    state: State,
    /// The wall grid, loaded once per opening: a mark snaps onto open
    /// ground.
    grid: Option<Arc<Grid>>,
    serial: u64,
    hover: Hover,
    closing: Option<Closing>,
    mark: Option<Mark>,
    last: Option<Last>,
    stack: Option<Stack>,
    coming: Option<Coming>,
    /// A driven window or a team strategy window was sent: ended when
    /// control is released.
    driving: bool,
    /// The place at the open wheel's press point (its tooltip).
    place: Option<Place>,
    /// The armed preview's glyph this frame.
    preview: Option<Glyph>,
    /// The stack's box last frame (not a battlefield click).
    bounds: Option<crate::draw_list::Rect>,
    /// The champion icon each portrait node holds, and the last attempt.
    faces: HashMap<&'static str, String>,
    face_tried: HashMap<&'static str, (String, Instant)>,
    /// Research builds: when a shown row without a name was last logged.
    #[cfg(feature = "research")]
    face_missing: HashMap<&'static str, Instant>,
    cache: crate::ui_writes::PropCache,
    spawn_attempt: Option<Instant>,
    ready: bool,
    shown: bool,
    pub(crate) list: DrawList,
}
impl Pings {
    pub fn input_pending(&self) -> bool {
        self.state.wheel.is_some()
            || self.state.previous_key
            || self.state.drain
            || self.state.reserved()
            || self.state.press.is_some()
    }
}

impl Client {
    fn ping_context(&self, o: &ClientObservations, battlefield: bool) -> Context {
        let identity = self.movement.cosmetic_anchor().map(|a| Identity {
            generation: self.timing.generation(),
            key: a.key,
            player: a.player,
        });
        Context {
            identity,
            controlling: battlefield && self.timing.client_controls(None),
            running: self.timing.client_running(),
            blocked: crate::ui_state::SETTINGS_OPEN.load(Ordering::Relaxed)
                || crate::ui_state::SHOP_OPEN.load(Ordering::Relaxed),
            emote_open: o.emotes.wheel_open(),
            aiming: self.abilities.interaction_busy() || self.movement.attack_move_armed(),
        }
    }
    /// Where a press at `p` anchors: the minimap, or the field (not over
    /// the HUD).
    fn ping_anchor(&self, p: (f32, f32)) -> Option<Anchor> {
        let frame = self.camera.frame()?;
        if let Some(world) = subject::minimap_point(frame, p) {
            return Some(Anchor::Minimap(world));
        }
        // Off the map's edge is still the field: the point is clamped.
        if self.camera.command_blocked(p) {
            return None;
        }
        subject::field_point(frame, p).map(Anchor::Field)
    }
    /// The controlled athlete's team as the orders see it: each teammate's
    /// brain index (from the native mapping) with his champion's position.
    fn ping_team(&self) -> Team {
        let mut team = Team {
            me: call::controlled_brain().map(|b| b.1),
            reach: crate::native_adapter::ping_reach(),
            ..Team::default()
        };
        let side = call::controlled_brain().map(|b| b.0);
        for u in super::team_units() {
            if !(u.is_champion && u.friendly) {
                continue;
            }
            if let Some((s, i)) = call::brain_of(u.id) {
                if Some(s) == side && usize::from(i) < 5 {
                    team.pos[usize::from(i)] = Some(u.position);
                }
            }
        }
        team
    }
    /// The enemy champions the team sees: (entity, position, player index).
    fn seen_enemies(&self) -> Vec<(usize, (u64, u64), Option<u8>)> {
        super::team_units()
            .into_iter()
            .filter(|u| u.is_champion && !u.friendly)
            .map(|u| (u.id, u.position, call::brain_of(u.id).map(|b| b.1)))
            .collect()
    }
    /// The enemies the team sees now, by player index (the fog ping's
    /// choice among the others).
    fn seen_mask(&self) -> [bool; 5] {
        let mut mask = [false; 5];
        for (_, _, index) in self.seen_enemies() {
            if let Some(slot) = index.and_then(|i| mask.get_mut(usize::from(i))) {
                *slot = true;
            }
        }
        mask
    }
    /// The plain ground at `at`, with the nearest enemy the team sees
    /// within `intent::NEAR_ENEMY`.
    fn ground(&self, at: (u64, u64)) -> Place {
        let gap =
            |p: (u64, u64)| (p.0 as f32 - at.0 as f32).hypot(p.1 as f32 - at.1 as f32) / 1000.;
        let enemy = self
            .seen_enemies()
            .into_iter()
            .map(|(entity, pos, index)| (gap(pos), Enemy { entity, index }))
            .filter(|(g, _)| *g <= intent::NEAR_ENEMY)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, e)| e);
        Place::Ground { at, enemy }
    }
    /// The armed click's ground: a pit when the point is within its radius
    /// and the pit is a place (seen alive, or by the game's clock: spec
    /// section 15.3), else the plain ground.
    fn ground_or_pit(&self, at: (u64, u64)) -> Place {
        places::pit_at(at).map_or_else(|| self.ground(at), Place::Pit)
    }
    /// The enemy champion the team sees nearest the controlled athlete,
    /// within `intent::NEAR_ENEMY` (Assist me).
    fn near_me(&self, team: &Team) -> Option<Enemy> {
        match self.ground(team.me_at()?) {
            Place::Ground { enemy, .. } => enemy,
            _ => None,
        }
    }
    /// The place at a world point: a live pit, the own base, a lane, or
    /// the ground.
    fn ping_place(&self, at: (u64, u64)) -> Place {
        if let Some(pit) = places::pit_at(at) {
            return Place::Pit(pit);
        }
        let side = call::controlled_brain().map(|b| b.0);
        if side.is_some_and(|s| places::in_base(at, s)) {
            return Place::Base(at);
        }
        match places::lane_at(at) {
            Some(lane) => match self.ground(at) {
                Place::Ground { enemy, .. } => Place::Lane { lane, at, enemy },
                _ => Place::Lane {
                    lane,
                    at,
                    enemy: None,
                },
            },
            None => self.ground(at),
        }
    }
    /// What an armed click at screen point `p` lands on (None: not on the
    /// field or the minimap). Ignores champion-only: the ping names any
    /// unit, but only one the team sees.
    fn ping_clicked(&self, p: (f32, f32)) -> Option<(Clicked, (u64, u64))> {
        let frame = self.camera.frame().filter(|f| f.valid())?;
        // The team's units (1.7.1-dev16: also while the controlled champion
        // is dead), and those the player's vision shows the team does not
        // (a click on one is a fog ping).
        let team = super::team_units();
        let mut units = self.movement.hover_units();
        units.retain(|u| !team.iter().any(|t| t.id == u.id));
        units.extend(team);
        let fog = |at: (u64, u64)| Clicked::Fog(at, places::lane_at(at));
        if let Some(world) = subject::minimap_point(frame, p) {
            // On the minimap: an enemy champion's icon under the cursor.
            let icon = units.iter().find(|u| {
                let s = frame.project_minimap(u.position);
                u.is_champion
                    && !u.friendly
                    && (s.0 - p.0).hypot(s.1 - p.1) <= subject::MINIMAP_ICON
            });
            return Some(match icon {
                Some(u) if super::team_sees(u.id) => (
                    Clicked::Enemy(Enemy {
                        entity: u.id,
                        index: call::brain_of(u.id).map(|b| b.1),
                    }),
                    u.position,
                ),
                Some(u) => (fog(u.position), u.position),
                None => (Clicked::Ground(self.ground_or_pit(world)), world),
            });
        }
        if self.camera.command_blocked(p) {
            return None;
        }
        let world = subject::field_point(frame, p)?;
        let under = crate::combat::picked(frame, p, &units, false, None)
            .first()
            .and_then(|id| units.iter().find(|u| u.id == *id).copied());
        let Some(u) = under else {
            return Some((Clicked::Ground(self.ground_or_pit(world)), world));
        };
        if !u.friendly && !super::team_sees(u.id) {
            return Some((fog(u.position), u.position));
        }
        let on = if u.friendly && u.is_champion {
            Clicked::Ally(u.id)
        } else if u.is_champion {
            Clicked::Enemy(Enemy {
                entity: u.id,
                index: call::brain_of(u.id).map(|b| b.1),
            })
        } else if u.is_tower {
            Clicked::Tower(u.id, places::nearest_lane(u.position))
        } else if let Some(pit) = places::pit_of(u.id) {
            Clicked::Monster(u.id, pit)
        } else {
            Clicked::Ground(self.ground(world))
        };
        Some((on, u.position))
    }
    /// The early-input path (`pre_update`): only while the ping is in use.
    pub(crate) fn ping_early(&self, keys: &mut Keys) {
        if !ping_key(*keys)
            && self
                .observations
                .lock()
                .is_ok_and(|o| !o.pings.input_pending())
        {
            return;
        }
        if let Ok(mut o) = self.observations.lock() {
            self.ping_input(&mut o, keys, true);
        }
    }
    /// The key's input; fires at a release or an armed click. Runs after
    /// the emote wheel's input and adds its capture to the shared flag.
    fn ping_input(&self, o: &mut ClientObservations, keys: &mut Keys, battlefield: bool) {
        let c = self.ping_context(o, battlefield);
        if o.pings.state.identity != c.identity {
            // Another match or athlete: the old windows end.
            self.end_windows(o);
            o.pings.mark = None;
            o.pings.closing = None;
            o.pings.stack = None;
            o.pings.last = None;
            o.pings.coming = None;
            o.pings.place = None;
        }
        let now = crate::clock::now();
        let opening = o.pings.state.wheel.is_none();
        let mut state = std::mem::take(&mut o.pings.state);
        let input = state.input(
            *keys,
            c,
            now,
            |p| self.ping_anchor(p),
            |p| self.ping_clicked(p).is_some(),
        );
        o.pings.state = state;
        let opened = opening && (o.pings.state.wheel.is_some() || input.fired.is_some());
        if opened {
            o.pings.grid = self.movement.navigation().map(Arc::new);
            let anchor = o.pings.state.wheel.or(input.fired).map(|w| w.anchor);
            o.pings.place = anchor.map(|a| self.ping_place(a.world()));
        }
        if let Some(w) = o.pings.state.wheel.as_ref() {
            let cursor = keys.cursor.filter(finite).unwrap_or(w.gesture.center());
            o.pings
                .hover
                .update(w.gesture.selection(), w.gesture.fan(), cursor, now);
        } else if opening && input.fired.is_none() {
            o.pings.hover = Hover::default();
        }
        WHEEL_OPEN.store(o.pings.state.wheel.is_some(), Ordering::Relaxed);
        LEFT_RESERVED.store(o.pings.state.reserved(), Ordering::Relaxed);
        // The armed cursor and preview: what the click would give here.
        let armed = o.pings.state.armed.is_some();
        let clicked = armed
            .then(|| keys.cursor.and_then(|p| self.ping_clicked(p)))
            .flatten();
        let retreat = o.pings.state.retreat;
        let choice = if retreat {
            clicked.map(|_| Choice {
                glyph: Glyph::Leave,
                order: Order::Mark,
            })
        } else {
            clicked.and_then(|(on, _)| intent::click(on, &self.ping_team()))
        };
        // V names no unit: the ping ring alone.
        let names = !retreat
            && matches!(
                clicked.map(|c| c.0),
                Some(Clicked::Enemy(_) | Clicked::Monster(..) | Clicked::Tower(..))
            );
        ARMED.store(
            match (armed && clicked.is_some(), names) {
                (false, _) => 0,
                (true, false) => 1,
                (true, true) => 2,
            },
            Ordering::Relaxed,
        );
        o.pings.preview = choice.map(|c| c.glyph);
        keys.emote_capture |= input.capture;
        crate::ui_state::EMOTE_CAPTURE.store(keys.emote_capture, Ordering::Relaxed);
        if input.armed {
            self.logger.write(if retreat {
                "PING armed (retreat): the next left click pings retreat"
            } else {
                "PING armed: the next left click pings"
            });
        }
        if let Some(why) = input.disarmed {
            self.logger
                .write(&format!("PING armed ping cancelled: {why}"));
        }
        if let Some(w) = input.cancelled {
            o.pings.closing = Some(Closing {
                wheel: w,
                hover: o.pings.hover,
                at: now,
                fired: None,
            });
            o.pings.hover = Hover::default();
        }
        if c.identity.is_none() {
            return;
        }
        if let Some(p) = input.click.filter(|_| retreat) {
            // V: Retreat at the click point, whatever is under it.
            if let Some(at) = self.ping_anchor(p).map(|a| a.world()) {
                let place = self.ping_place(at);
                let here = Here {
                    place,
                    near_me: None,
                };
                let choice = intent::wheel(here, Slot::Retreat, &self.ping_team());
                self.fire(o, choice, On::Point(at), at, "click", "retreat", now);
            }
        } else if let Some(p) = input.click {
            if let Some((on, at)) = self.ping_clicked(p) {
                match intent::click(on, &self.ping_team()) {
                    Some(choice) => {
                        let pin = match on {
                            Clicked::Enemy(e) => On::Unit(e.entity),
                            _ => On::Point(at),
                        };
                        self.fire(o, choice, pin, at, "click", &on.text(), now);
                    }
                    None => self
                        .logger
                        .write(&format!("PING click {} order=none (reserved)", on.text())),
                }
            }
        }
        let Some(w) = input.fired else {
            return;
        };
        let sel = w.gesture.selection();
        o.pings.closing = Some(Closing {
            wheel: w,
            hover: o.pings.hover,
            at: now,
            fired: Some(sel),
        });
        o.pings.hover = Hover::default();
        if let Some(slot) = w.fired() {
            let at = w.anchor.world();
            let place = o.pings.place.unwrap_or_else(|| self.ping_place(at));
            let team = self.ping_team();
            let here = Here {
                place,
                near_me: self.near_me(&team),
            };
            let choice = intent::wheel(here, slot, &team);
            let gesture = format!("slot={}", slot.log_name());
            let pin = match (choice.order, slot) {
                (Order::Fight(entity), _) => On::Unit(entity),
                // Assist me with nobody near: the mark at the player.
                (_, Slot::AssistMe) => On::Point(team.me_at().unwrap_or(place.at())),
                _ => On::Point(place.at()),
            };
            self.fire(o, choice, pin, at, &gesture, &place.text(), now);
        }
    }
    /// Every driven window and the team strategy window end (once).
    fn end_windows(&self, o: &mut ClientObservations) {
        super::walk::end_all(super::walk::End::Released, &self.logger);
        if std::mem::take(&mut o.pings.driving) {
            ping_orders::close(None);
            plan_call::end();
            o.pings.coming = None;
            self.logger.write("PING windows ended: control released");
        }
    }
    /// A ping: its order to the worker (or a repeat's refresh), the pin and
    /// the stack.
    #[allow(clippy::too_many_arguments)]
    fn fire(
        &self,
        o: &mut ClientObservations,
        choice: Choice,
        pin: On,
        at: (u64, u64),
        gesture: &str,
        what: &str,
        now: Instant,
    ) {
        o.pings.serial += 1;
        let n = o.pings.serial;
        let live = o.pings.mark.is_some_and(|m| m.alpha(now).is_some());
        let repeat = o
            .pings
            .last
            .filter(|l| live && l.choice == choice && choice.order != Order::Mark);
        let all = |to: intent::Mask| -> Vec<usize> { (0..5).filter(|i| to[*i]).collect() };
        // A newer order to a teammate hands his walk back (spec 15.2); a fog
        // note, a mark or a team plan does not.
        let ordered: Option<intent::Mask> = match choice.order {
            Order::Fight(_) => Some([true; 5]),
            Order::Send(ping_orders::Send::Objective(_), to) | Order::Drive(_, to) => Some(to),
            Order::Missing { regroup, .. } => Some(regroup),
            _ => None,
        };
        if let Some(to) = ordered.filter(|_| repeat.is_none()) {
            super::walk::end_for(to, super::walk::End::Replaced, &self.logger);
        }
        let (reference, enemy) = match choice.order {
            Order::Fight(entity) => {
                // The worker refreshes a live call on the same enemy.
                let sent = call::request(entity);
                self.logger.write(&format!(
                    "PING n={n} {gesture} {what} order=fight request={}{}",
                    sent.map_or("none".to_owned(), |(serial, _)| serial.to_string()),
                    if repeat.is_some() {
                        " (repeat refreshed)"
                    } else {
                        ""
                    }
                ));
                (sent.map(|(s, _)| Ref::Fight(s)), Some(entity))
            }
            _ if repeat.is_some() => {
                self.logger.write(&format!(
                    "PING n={n} {gesture} {what} repeat refreshed: nothing new asked"
                ));
                (repeat.and_then(|l| l.reference), None)
            }
            Order::Send(send, to) => {
                let serial = ping_orders::post(send, to);
                self.logger.write(&format!(
                    "PING n={n} {gesture} {what} {} to={:?} request={}",
                    send.text(),
                    all(to),
                    serial.map_or("none".to_owned(), |s| s.to_string())
                ));
                // A take at a pit: the mod walks them there (spec 15.2);
                // the stack shows the walk's rows.
                let walk = self.start_walk(n, send, to);
                // At a pit also the team's finishing rule for a while; the
                // stack shows the objective's answers.
                if let Some(rule) =
                    intent::finish(choice.order, crate::native_adapter::ping_reach())
                {
                    let plan = plan_call::request(rule);
                    self.logger.write(&format!(
                        "PING n={n} plan={} for {} s request={plan}",
                        rule.text(),
                        plan_call::WINDOW / super::SECOND
                    ));
                    o.pings.driving = true;
                }
                if walk {
                    (Some(Ref::Walk(n)), None)
                } else {
                    (serial.map(Ref::Order), None)
                }
            }
            Order::Drive(drive, to) => {
                let serial = ping_orders::post_window(drive, to);
                self.logger.write(&format!(
                    "PING n={n} {gesture} {what} drive={} to={:?} request={}",
                    drive.text(),
                    all(to),
                    serial.map_or("none".to_owned(), |s| s.to_string())
                ));
                o.pings.driving = true;
                if let (ping_orders::Drive::Hold { lane }, Some(serial)) = (drive, serial) {
                    let pos = self.movement.cosmetic_anchor().and_then(|a| a.position);
                    o.pings.coming = Some(Coming {
                        serial,
                        lane,
                        to,
                        since: now,
                        last: now,
                        gap: pos.map(|p| places::lane_gap(p, lane)),
                    });
                }
                (serial.map(Ref::Order), None)
            }
            Order::Missing {
                enemy,
                fog,
                regroup,
            } => {
                let f = ping_orders::Fog {
                    at,
                    enemy,
                    visible: self.seen_mask(),
                };
                let serial = ping_orders::post_missing(f, fog, regroup);
                self.logger.write(&format!(
                    "PING n={n} {gesture} {what} {} to={:?} drive=regroup to={:?} request={}",
                    ping_orders::Send::Fog(f).text(),
                    all(fog),
                    all(regroup),
                    serial.map_or("none".to_owned(), |s| s.to_string())
                ));
                o.pings.driving = true;
                (serial.map(Ref::Order), None)
            }
            Order::Fog(enemy, to) => {
                let send = ping_orders::Send::Fog(ping_orders::Fog {
                    at,
                    enemy,
                    visible: self.seen_mask(),
                });
                let serial = ping_orders::post(send, to);
                self.logger.write(&format!(
                    "PING n={n} {gesture} {what} {} to={:?} request={}",
                    send.text(),
                    all(to),
                    serial.map_or("none".to_owned(), |s| s.to_string())
                ));
                (serial.map(Ref::Order), None)
            }
            Order::Plan(plan, to) => {
                let serial = plan_call::request(plan);
                self.logger.write(&format!(
                    "PING n={n} {gesture} {what} plan={} for {} s request={serial}",
                    plan.text(),
                    plan_call::WINDOW / super::SECOND
                ));
                o.pings.driving = true;
                (Some(Ref::Plan(serial, to)), None)
            }
            Order::Mark => {
                self.logger.write(&format!(
                    "PING n={n} {gesture} {what} order=mark (nothing sent)"
                ));
                (None, None)
            }
        };
        // A mark alone snaps onto open ground; an order's pin stays where
        // the order is about.
        let pin = match (choice.order, pin) {
            (Order::Mark, On::Point(p)) => On::Point(
                o.pings
                    .grid
                    .as_deref()
                    .and_then(|g| g.nearest_open(p))
                    .unwrap_or(p),
            ),
            _ => pin,
        };
        o.pings.mark = Some(Mark {
            on: pin,
            glyph: choice.glyph,
            since: now,
        });
        let same_stack = repeat.is_some()
            || o.pings
                .last
                .is_some_and(|l| l.reference.is_some() && l.reference == reference);
        match (reference, o.pings.stack.as_mut()) {
            (Some(_), Some(s)) if same_stack => s.since = now,
            (Some(_), _) => o.pings.stack = Some(Stack::new(choice.glyph, now)),
            // A ping nobody hears: the ping row only.
            (None, _) => o.pings.stack = Some(Stack::new(choice.glyph, now)),
        }
        o.pings.last = Some(Last {
            choice,
            reference,
            enemy,
        });
    }
    /// A take at a pit (Take here, the click on a pit or its monster): the
    /// walk (spec section 15.2) for every teammate in `to` who is not
    /// fighting; the fighters' rows busy. False for any other send, or with
    /// nobody to walk.
    fn start_walk(&self, n: u64, send: ping_orders::Send, to: intent::Mask) -> bool {
        let ping_orders::Send::Objective(o) = send else {
            return false;
        };
        if o.pit_battle() != Some(false) {
            return false;
        }
        let Some(key) = self.movement.cosmetic_anchor().map(|a| a.key) else {
            return false;
        };
        let mut walkers = Vec::new();
        let mut excluded = Vec::new();
        for i in (0..5u8).filter(|i| to[usize::from(*i)]) {
            let Some((_, player)) = call::teammate(i) else {
                continue;
            };
            let hp = super::walk::health(player);
            if ping_orders::fighting(i) {
                excluded.push((i, ping_orders::Status::Busy));
            } else if let Some((now, ttd)) = hp.filter(|(now, ttd)| super::walk::low(*now, *ttd)) {
                // Low at the ping (spec section 15.6): declined at once.
                excluded.push((i, ping_orders::Status::Declined));
                self.logger.write(&format!(
                    "PING walk n={n} to={i} player={player} declined hp={now:.0} ttd={}",
                    super::walk::ttd_text(ttd)
                ));
            } else {
                walkers.push((i, player));
            }
        }
        if walkers.is_empty() && excluded.is_empty() {
            return false;
        }
        let target = places::walk_target(o.tag());
        super::walk::start(key, n, o.tag(), target, &walkers, &excluded, &self.logger);
        true
    }
    /// On my way's hold while the player is on the way: every
    /// [`COMING_EVERY`] the player's distance to the lane's centre line
    /// must have shrunk; it ends on arrival within [`COMING_NEAR`] of the
    /// laner it was sent to (his unit), or at [`COMING_FOR`]. The end goes
    /// to the worker, which puts the laner's flags back.
    fn ping_coming(&self, o: &mut ClientObservations, now: Instant) {
        let Some(c) = o.pings.coming else {
            return;
        };
        let pos = self.movement.cosmetic_anchor().and_then(|a| a.position);
        let team = self.ping_team();
        let near = pos.and_then(|p| {
            (0..5)
                .filter(|i| c.to[*i])
                .filter_map(|i| team.pos[i])
                .map(|l| (p.0 as f32 - l.0 as f32).hypot(p.1 as f32 - l.1 as f32) / 1000.)
                .min_by(f32::total_cmp)
        });
        let gap = pos.map(|p| places::lane_gap(p, c.lane));
        let why = if now.saturating_duration_since(c.since) >= COMING_FOR {
            Some("10 s".to_owned())
        } else if let Some(g) = near.filter(|g| *g <= COMING_NEAR) {
            Some(format!("arrived gap={g:.0}"))
        } else if now.saturating_duration_since(c.last) >= COMING_EVERY {
            match (c.gap, gap) {
                (Some(before), Some(after)) if after < before => {
                    o.pings.coming = Some(Coming {
                        last: now,
                        gap,
                        ..c
                    });
                    None
                }
                _ => Some(format!(
                    "not coming lane_gap={}",
                    gap.map_or("?".to_owned(), |g| format!("{g:.0}"))
                )),
            }
        } else {
            None
        };
        if let Some(why) = why {
            ping_orders::close(Some(c.serial));
            self.logger.write(&format!(
                "PING coming ended n={} lane={}: {why}",
                c.serial, c.lane
            ));
            o.pings.coming = None;
        }
    }
    /// The champion name of a unit, from the Tab panel's sample.
    fn champion_name(&self, player: usize) -> Option<String> {
        self.team
            .snapshot(self.timing.match_key())
            .into_iter()
            .find(|p| p.id == player)
            .map(|p| p.champion)
            .filter(|c| !c.is_empty())
    }
    /// The key's input and the window for one frame (`apply_windows`,
    /// after the emote wheel). Returns the stack's box, which is not
    /// battlefield.
    pub(crate) fn ping_frame(
        &self,
        ctx: &mut StableClient<'_>,
        o: &mut ClientObservations,
        keys: &mut Keys,
        battlefield: bool,
    ) -> Option<Rect> {
        self.ping_input(o, keys, battlefield);
        let controls = battlefield && self.timing.client_controls(None);
        let now = crate::clock::now();
        if !controls {
            o.pings.mark = None;
            o.pings.stack = None;
            o.pings.coming = None;
        }
        // The windows end with control handed back to the AI (not with a
        // pause, a window over the field or a heartbeat gap, which also
        // clear `controls`): put back at each teammate's next think, the
        // team strategy at the next tick. Each also has its own cap.
        let handed_back = self.timing.client_session(None) && !self.timing.client_controls(None);
        if handed_back {
            self.end_windows(o);
        }
        self.ping_coming(o, now);
        if o.pings.mark.is_some_and(|m| m.alpha(now).is_none()) {
            o.pings.mark = None;
        }
        if o.pings.closing.as_ref().is_some_and(|c| {
            now.duration_since(c.at) >= wheel_draw::closing_span(c.fired.is_some())
        }) {
            o.pings.closing = None;
        }
        // The stack: this frame's answers; ended once its rows faded.
        let reference = o.pings.last.and_then(|l| l.reference);
        if let (Some(s), Some(r)) = (o.pings.stack.as_mut(), reference) {
            s.update(r.answers(), now);
        }
        if o.pings.stack.as_ref().is_some_and(|s| s.ended(now)) {
            o.pings.stack = None;
        }
        // The portraits the stack shows: the fight call's enemy, then each
        // teammate with a row.
        let faces = o.pings.stack.as_ref().map(|s| {
            let shown = s.shown(now);
            let target = o
                .pings
                .last
                .and_then(|l| l.enemy)
                .and_then(call::player_of)
                .and_then(|p| self.champion_name(p));
            let rows: [Option<String>; 5] = std::array::from_fn(|i| {
                let mate = shown[i].then(|| call::teammate(i as u8)).flatten();
                let name = mate.and_then(|(_, player)| self.champion_name(player));
                #[cfg(feature = "research")]
                if shown[i] && name.is_none() {
                    let node = stack::NODES[2 + i];
                    let due = o
                        .pings
                        .face_missing
                        .get(node)
                        .is_none_or(|at| crate::clock::since(at) >= Duration::from_secs(1));
                    if due {
                        o.pings.face_missing.insert(node, now);
                        self.logger.write(&format!(
                            "PING face node={node} name=none player={}",
                            mate.map_or("none".to_owned(), |(_, p)| p.to_string())
                        ));
                    }
                }
                name
            });
            (target, rows)
        });
        let show = controls && keys.focused;
        let frame = self.camera.frame().filter(|f| f.valid());
        // The pinged champion's position while the player sees him.
        let mark_at = o.pings.mark.and_then(|m| match m.on {
            On::Unit(id) => super::team_units()
                .into_iter()
                .find(|u| u.id == id)
                .map(|u| u.position),
            On::Point(p) => Some(p),
        });
        // Each recipient's champion, while his answer shows over it.
        let heads = o
            .pings
            .stack
            .as_ref()
            .map(|s| s.heads(now))
            .unwrap_or_default();
        let heads_at: [Option<(u64, u64)>; 5] = if heads.iter().any(Option::is_some) {
            let units = super::team_units();
            std::array::from_fn(|i| {
                let (entity, _) = heads[i].and_then(|_| call::teammate(i as u8))?;
                units.iter().find(|u| u.id == entity).map(|u| u.position)
            })
        } else {
            [None; 5]
        };
        let cursor = keys.cursor;
        o.pings.render(
            ctx,
            Frame {
                camera: frame,
                show,
                mark_at,
                heads: (heads, heads_at),
                cursor,
                faces,
                now,
            },
            &self.logger,
        );
        o.pings.bounds.map(|(x, y, w, h)| Rect { x, y, w, h })
    }
}

/// Each teammate's answer glyph and opacity, and his champion's position.
type Heads = ([Option<(Glyph, f32)>; 5], [Option<(u64, u64)>; 5]);
/// What the window shows this frame.
struct Frame {
    camera: Option<CameraFrame>,
    show: bool,
    mark_at: Option<(u64, u64)>,
    /// Each teammate's answer over his champion, and where he is.
    heads: Heads,
    cursor: Option<(f32, f32)>,
    /// The stack's portraits: the target's champion, each teammate's.
    faces: Option<(Option<String>, [Option<String>; 5])>,
    now: Instant,
}

/// The window's single image nodes, outside the wheel: the pin and its
/// glyph, the minimap pin, the armed preview, then the stack's.
const PIN_NODES: [&str; 6] = [
    "pin",
    "pin_glyph",
    "mini",
    "mini_glyph",
    "preview",
    "preview_glyph",
];

impl Pings {
    fn props(&mut self, ctx: &mut StableClient<'_>, path: &str, value: &str) {
        crate::hud_motion::properties(ctx, &mut self.cache, path, value);
    }
    /// A stack portrait node shows `name`'s champion icon (set once per
    /// name, tried again a second later on failure, as the session strip),
    /// fitted to the Tab panel's 44 x 48 box at sprite scale 2
    /// (1.7.1-dev21, spec section 14.1). Superseded 2026-11-04: dev15 to
    /// dev19 passed scale 0, which sizes the face to nothing (seen in game,
    /// dev19); dev20's 22 px box was too small and cropped (seen, dev20).
    /// Research builds log each attempt (`PING face`).
    #[cfg_attr(not(feature = "research"), allow(unused_variables))]
    fn face(
        &mut self,
        ctx: &mut StableClient<'_>,
        node: &'static str,
        name: Option<&str>,
        log: &Logger,
    ) -> bool {
        let Some(name) = name else {
            return false;
        };
        if self.faces.get(node).is_some_and(|n| n == name) {
            return true;
        }
        let due = self.face_tried.get(node).is_none_or(|(old, at)| {
            old != name || crate::clock::since(at) >= Duration::from_secs(1)
        });
        if due {
            self.face_tried
                .insert(node, (name.to_owned(), crate::clock::now()));
            let (w, h) = stack::FACE;
            let ok = ctx.ui_set_champion_icon(&format!("{PATH}.{node}"), name, w, h, 2.);
            #[cfg(feature = "research")]
            log.write(&format!("PING face node={node} name={name} ok={ok}"));
            if ok {
                self.faces.insert(node, name.to_owned());
                return true;
            }
        }
        false
    }
    fn render(&mut self, ctx: &mut StableClient<'_>, f: Frame, log: &Logger) {
        let Frame {
            camera: frame,
            show,
            mark_at,
            heads,
            cursor,
            faces,
            now,
        } = f;
        self.list.clear();
        self.bounds = None;
        let open = self.state.wheel.is_some();
        let armed = self.state.armed.is_some();
        let wanted = show
            && (open
                || armed
                || self.closing.is_some()
                || self.mark.is_some()
                || self.stack.is_some());
        if !wanted && !self.shown && (self.ready || !show) {
            return;
        }
        if !ctx.ui_exists(PATH) {
            self.shown = false;
            if !show
                || self
                    .spawn_attempt
                    .is_some_and(|t| t.elapsed() < Duration::from_secs(2))
            {
                return;
            }
            self.spawn_attempt = Some(Instant::now());
            if !self.spawn(ctx) {
                log.write("PING UI spawn failed; the window retries, pings still work");
                return;
            }
        }
        self.ready = true;
        let visible = wanted && !crate::ui_root::hidden();
        self.props(ctx, PATH, &format!("visible: {visible};"));
        // The click catcher, while the wheel is open.
        self.props(
            ctx,
            &format!("{PATH}.capture"),
            &format!("visible: {};", visible && open),
        );
        self.shown = wanted;
        if !visible {
            return;
        }
        // The pins and the armed preview.
        let hidden = || "visible: false;".to_owned();
        let mut pins: [String; 6] = std::array::from_fn(|_| hidden());
        if let (Some(m), Some(frame)) = (self.mark, frame) {
            let [a, b, c, d] = mark::nodes(frame, &m, mark_at, now);
            pins[..4].clone_from_slice(&[a, b, c, d]);
        }
        if let (true, Some(g)) = (armed, self.preview) {
            let [a, b] = mark::preview(cursor, g);
            pins[4..].clone_from_slice(&[a, b]);
        }
        for (node, props) in PIN_NODES.iter().zip(pins.iter()) {
            self.props(ctx, &format!("{PATH}.{node}"), props);
        }
        // Every order's answers over the recipients' champions.
        let answers = match frame {
            Some(frame) => mark::heads(frame, heads.0, heads.1),
            None => std::array::from_fn(|_| hidden()),
        };
        for (node, props) in mark::HEAD_NODES.iter().zip(answers.iter()) {
            self.props(ctx, &format!("{PATH}.{node}"), props);
        }
        // The answer stack, under the target state panel.
        let mut stack_nodes: [String; 12] = std::array::from_fn(|_| hidden());
        if let Some((target, rows)) = faces {
            let target_face = self.face(ctx, "stack_face", target.as_deref(), log);
            let row_faces: [bool; 5] = std::array::from_fn(|i| {
                let node = stack::NODES[2 + i];
                self.face(ctx, node, rows[i].as_deref(), log)
            });
            let ingame = ctx.ui_node_rect("ingame");
            let origin = crate::draw_list::anchored(
                ingame,
                (stack::ROW.0, stack::HEIGHT),
                (0., 0.),
                (0., 0.),
                STACK_AT,
            );
            if let Some(s) = self.stack.as_mut() {
                let (bounds, nodes) = s.draw(&mut self.list, origin, (target_face, row_faces), now);
                self.bounds = Some(bounds);
                stack_nodes = nodes;
            }
        }
        for (node, props) in stack::NODES.iter().zip(stack_nodes.iter()) {
            self.props(ctx, &format!("{PATH}.{node}"), props);
        }
        // The wheel, open or closing.
        let hover = self.hover;
        let view = match (self.state.wheel.as_ref(), self.closing.as_ref()) {
            (Some(w), _) => Some((w.gesture, w.opened, hover, None)),
            (None, Some(c)) => Some((
                c.wheel.gesture,
                c.wheel.opened,
                c.hover,
                Some((c.at, c.fired)),
            )),
            _ => None,
        };
        let drawn = view.and_then(|(g, opened, hover, closing)| {
            let v = wheel_draw::View {
                center: g.center(),
                selection: g.selection(),
                fan: g.fan(),
                hover,
                opened,
                closing,
                now,
            };
            let nodes = wheel_draw::nodes(&v)?;
            // The tooltip: the chosen slot's name and the place it reads.
            let shown = closing.and_then(|c| c.1).unwrap_or(v.selection);
            let tip = shown
                .slot()
                .and_then(|(h, n)| Slot::at(h, n))
                .and_then(|slot| {
                    let place = self.place.and_then(|p| slot.place_word(p));
                    Some((
                        wheel_draw::tooltip(&v, place.is_some())?,
                        slot.word(),
                        place,
                    ))
                });
            Some((nodes, tip))
        });
        let mut tip_shown = false;
        match drawn {
            Some((nodes, tip)) => {
                for (node, props) in wheel_draw::NODES.iter().zip(nodes.iter()) {
                    self.props(ctx, &format!("{PATH}.{node}"), props);
                }
                if let Some((t, name, place)) = tip {
                    tip_shown = true;
                    let (x, y, w, h) = t.strip;
                    self.list.rect(
                        (x - 1., y - 1., w + 2., h + 2.),
                        2328,
                        3.,
                        wheel_draw::TIP_FRAME,
                    );
                    self.list.rect((x, y, w, h), 2329, 2., wheel_draw::TIP_FILL);
                    let label = format!("{PATH}.tip_name");
                    self.cache.text_at(ctx, &label, name);
                    self.props(
                        ctx,
                        &label,
                        &format!("visible: true; x: {:.0}px; y: {:.0}px;", t.name.0, t.name.1),
                    );
                    let label = format!("{PATH}.tip_place");
                    match (t.place, place) {
                        (Some((px, py)), Some(word)) => {
                            self.cache.text_at(ctx, &label, word);
                            self.props(
                                ctx,
                                &label,
                                &format!("visible: true; x: {px:.0}px; y: {py:.0}px;"),
                            );
                        }
                        _ => self.props(ctx, &label, "visible: false;"),
                    }
                }
            }
            None => self.props(ctx, &format!("{PATH}.wheel"), "visible: false;"),
        }
        if !tip_shown {
            self.props(ctx, &format!("{PATH}.tip_name"), "visible: false;");
            self.props(ctx, &format!("{PATH}.tip_place"), "visible: false;");
        }
        // The stroke's faint ink trail from the press point, while open.
        if let Some(w) = self.state.wheel.as_ref() {
            for pair in w.gesture.trail().windows(2) {
                self.list.line(
                    pair[0],
                    pair[1],
                    wheel_draw::TRAIL_WIDTH,
                    2299,
                    wheel_draw::TRAIL_COLOR,
                );
            }
        }
    }
    fn spawn(&mut self, ctx: &mut StableClient<'_>) -> bool {
        self.cache.clear();
        self.faces.clear();
        self.face_tried.clear();
        ctx.ui_spawn_source("ingame", &crate::ui_theme::themed(&template())) && ctx.ui_exists(PATH)
    }
}

/// The window's nodes, built like the emote wheel's template: the click
/// catcher, the wheel (`wheel_draw::NODES`: its pictures as image nodes, in
/// draw order), the tooltip's two labels (the emote wheel's label node),
/// the pins and the preview, and the stack's glyphs and portraits. The
/// tooltip's strip, the stack's boxes and countdown are drawn
/// (`Pings::list`).
fn template() -> String {
    let node = wheel_draw::NODE;
    let mut s = format!("lt_pings:empty {{ x: 0px; y: 0px; width: 1920px; height: 1080px; visible: false; ignore_event: true; z: 1040;\n#capture:color {{ x: 0px; y: 0px; width: 1920px; height: 1080px; color: #00000000; visible: false; ignore_event: false; z: 2300; }}\n#wheel:empty {{ width: {node}px; height: {node}px; visible: false; ignore_event: true; z: 2301;\n");
    for (z, n) in (2302..).zip(wheel_draw::NODES.iter().skip(1)) {
        let id = n.trim_start_matches("wheel.");
        s.push_str(&format!("#{id}:image {{ width: 1px; height: 1px; visible: false; ignore_event: true; z: {z}; }}\n"));
    }
    s.push_str("}\n");
    // The tooltip's two labels (the emote wheel's label node), placed each
    // frame beside the chosen slot.
    let size = wheel_draw::TIP_SIZE;
    let (width, height) = (wheel_draw::TIP_LABEL, wheel_draw::TIP_LINE);
    for (id, color) in [
        ("tip_name", 0xeeececffu32),
        ("tip_place", wheel_draw::TIP_PLACE),
    ] {
        s.push_str(&format!("#{id}:label {{ x: 0px; y: 0px; width: {width}px; height: {height}px; size: {size}; font: \"asset/lt_direct_control/font/medium\"; align_x: Left; align_y: Center; color: #{color:08x}; text: \"\"; visible: false; ignore_event: true; z: 2331; }}\n"));
    }
    let singles = [
        ("pin", 1046),
        ("pin_glyph", 1047),
        ("mini", 1106),
        ("mini_glyph", 1107),
        ("preview", 2340),
        ("preview_glyph", 2341),
    ];
    for id in mark::HEAD_NODES {
        s.push_str(&format!("#{id}:image {{ width: 1px; height: 1px; visible: false; ignore_event: true; z: 1045; }}
"));
    }
    for (id, z) in singles {
        s.push_str(&format!("#{id}:image {{ width: 1px; height: 1px; visible: false; ignore_event: true; z: {z}; }}\n"));
    }
    for id in stack::NODES {
        s.push_str(&format!("#{id}:image {{ width: 1px; height: 1px; visible: false; ignore_event: true; z: 1115; }}\n"));
    }
    s.push_str("}\n");
    s
}

impl crate::prewarm::Prespawn for Pings {
    fn built(&self, ctx: &StableClient<'_>) -> bool {
        ctx.ui_exists(PATH)
    }
    /// The template's root is hidden; the first frame that shows it writes
    /// as after a build.
    fn prespawn(&mut self, ctx: &mut StableClient<'_>, log: &Logger) -> bool {
        let ok = self.spawn(ctx);
        log.write(&format!("PING UI spawned ahead ok={ok}"));
        ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> Context {
        Context {
            identity: Some(Identity {
                generation: 1,
                key: (1, 2, 3),
                player: 4,
            }),
            controlling: true,
            running: true,
            blocked: false,
            emote_open: false,
            aiming: false,
        }
    }
    fn keys(ping: bool, left: bool, p: (f32, f32)) -> Keys {
        Keys {
            ping,
            left,
            cursor: Some(p),
            focused: true,
            ..Keys::default()
        }
    }
    /// V held or not, the left button, the cursor.
    fn v(held: bool, left: bool, p: (f32, f32)) -> Keys {
        Keys {
            ping_retreat: held,
            ..keys(false, left, p)
        }
    }
    #[test]
    fn v_enters_ping_mode_as_g_does_and_its_click_is_retreat() {
        let t = Instant::now();
        // A tap then a click: the quick ping, in retreat mode.
        let mut s = ready();
        assert!(input(&mut s, v(true, false, (500., 500.)), t).armed);
        input(&mut s, v(false, false, (500., 500.)), t);
        assert!(s.retreat && s.armed.is_some() && s.wheel.is_none());
        input(&mut s, v(false, true, (700., 300.)), t);
        let out = input(&mut s, v(false, false, (702., 301.)), t);
        assert_eq!(out.click, Some((700., 300.)));
        assert!(s.retreat);
        // A drag in V's mode opens the same wheel and fires its slots.
        let mut s = ready();
        input(&mut s, v(true, false, (500., 500.)), t);
        input(&mut s, v(false, false, (500., 500.)), t);
        input(&mut s, v(false, true, (500., 500.)), t);
        input(&mut s, v(false, true, (640., 500.)), t);
        assert!(s.wheel.is_some(), "the wheel");
        let out = input(&mut s, v(false, false, (640., 420.)), t);
        assert_eq!(out.fired.and_then(|w| w.fired()), Some(Slot::AssistMe));
        // Esc and right click cancel V's mode as G's.
        for cancel in [
            Keys {
                escape: true,
                ..v(false, false, (500., 500.))
            },
            Keys {
                right: true,
                ..v(false, false, (500., 500.))
            },
        ] {
            let mut s = ready();
            input(&mut s, v(true, false, (500., 500.)), t);
            input(&mut s, v(false, false, (500., 500.)), t);
            let out = input(&mut s, cancel, t);
            assert!(
                out.disarmed.is_some() && s.armed.is_none(),
                "{:?}",
                out.disarmed
            );
        }
        // G after V: the generic ping again.
        let mut s = ready();
        input(&mut s, v(true, false, (500., 500.)), t);
        input(&mut s, v(false, false, (500., 500.)), t);
        input(&mut s, keys(true, false, (500., 500.)), t);
        assert!(!s.retreat);
    }
    fn field(p: (f32, f32)) -> Option<Anchor> {
        Some(Anchor::Field((p.0 as u64, p.1 as u64)))
    }
    fn anywhere(_: (f32, f32)) -> bool {
        true
    }
    fn input(s: &mut State, k: Keys, at: Instant) -> Input {
        s.input(k, context(), at, field, anywhere)
    }
    fn ready() -> State {
        let mut s = State::default();
        input(&mut s, keys(false, false, (500., 500.)), Instant::now());
        s
    }
    /// A tap of the key: ping mode.
    fn tapped() -> State {
        let mut s = ready();
        let t = Instant::now();
        assert!(input(&mut s, keys(true, false, (500., 500.)), t).armed);
        input(&mut s, keys(false, false, (500., 500.)), t);
        assert!(s.armed.is_some() && s.wheel.is_none());
        s
    }
    /// A left-button stroke in ping mode, one frame per point, then the
    /// release; the release's result.
    fn drag(s: &mut State, path: &[(f32, f32)]) -> Input {
        let t = Instant::now();
        for p in path {
            input(s, keys(false, true, *p), t);
        }
        input(s, keys(false, false, *path.last().unwrap()), t)
    }
    #[test]
    fn in_ping_mode_a_click_is_the_quick_ping_and_a_drag_the_wheel() {
        // A click: the quick ping at the press point, reserved until the
        // button is up; held still for any length it is still a click.
        let mut s = tapped();
        let t = Instant::now();
        let out = input(&mut s, keys(false, true, (700., 300.)), t);
        assert!(out.click.is_none() && out.capture && s.reserved());
        let out = input(
            &mut s,
            keys(false, true, (710., 305.)),
            t + Duration::from_secs(9),
        );
        assert!(
            s.wheel.is_none() && out.click.is_none(),
            "inside the dead zone"
        );
        let out = input(
            &mut s,
            keys(false, false, (712., 306.)),
            t + Duration::from_secs(9),
        );
        assert_eq!(out.click, Some((700., 300.)));
        assert!(
            s.armed.is_none() && !s.reserved(),
            "the press ends ping mode"
        );
        // A drag: right across the ring, then up and out: On my way.
        let mut s = tapped();
        let out = drag(
            &mut s,
            &[(500., 500.), (530., 500.), (640., 500.), (680., 470.)],
        );
        let w = out.fired.unwrap();
        assert_eq!(w.anchor, Anchor::Field((500, 500)));
        assert_eq!(w.fired(), Some(Slot::OnMyWay));
        assert!(s.armed.is_none());
        // Left then straight up: Take here.
        let mut s = tapped();
        let out = drag(&mut s, &[(500., 500.), (360., 500.), (360., 440.)]);
        assert_eq!(out.fired.unwrap().fired(), Some(Slot::TakeHere));
        // Back to the hub and released: cancelled, nothing fires.
        let mut s = tapped();
        let out = drag(&mut s, &[(500., 500.), (560., 500.), (505., 500.)]);
        assert!(out.fired.is_none() && out.cancelled.is_some());
        assert_eq!(out.disarmed, Some("no_slot"));
    }
    #[test]
    fn a_fast_stroke_fires_from_the_path_alone_even_in_one_frame() {
        // Press, then the move and the release in the same frame: the wheel
        // opens, takes the segment and fires.
        let mut s = tapped();
        let t = Instant::now();
        input(&mut s, keys(false, true, (500., 500.)), t);
        input(&mut s, keys(false, true, (640., 500.)), t);
        let out = input(&mut s, keys(false, false, (680., 470.)), t);
        assert_eq!(out.fired.and_then(|w| w.fired()), Some(Slot::OnMyWay));
        // Right, then up with the release in the same frame: up (Assist
        // me), from the corner, with nothing drawn.
        let mut s = tapped();
        input(&mut s, keys(false, true, (500., 500.)), t);
        input(&mut s, keys(false, true, (640., 500.)), t);
        let out = input(&mut s, keys(false, false, (640., 420.)), t);
        assert_eq!(out.fired.and_then(|w| w.fired()), Some(Slot::AssistMe));
        // One frame from press to release, straight out on the left: no
        // corner, nothing fires (superseded 2026-11-04: dev17 fired Defend
        // here, the fan having locked 20 px past the rim).
        let mut s = tapped();
        input(&mut s, keys(false, true, (500., 500.)), t);
        let out = input(&mut s, keys(false, false, (300., 500.)), t);
        assert!(out.fired.is_none() && out.disarmed == Some("no_slot"));
        // Right then back toward the wheel: nothing fires.
        let mut s = tapped();
        input(&mut s, keys(false, true, (500., 500.)), t);
        input(&mut s, keys(false, true, (660., 500.)), t);
        let out = input(&mut s, keys(false, false, (610., 500.)), t);
        assert!(out.fired.is_none() && out.disarmed == Some("no_slot"));
        // Short of the rim: a half, no slot; nothing fires.
        let mut s = tapped();
        input(&mut s, keys(false, true, (500., 500.)), t);
        let out = input(&mut s, keys(false, false, (390., 500.)), t);
        assert!(
            out.fired.is_none() && out.click.is_none(),
            "a half, no slot"
        );
    }
    #[test]
    fn the_key_never_opens_the_wheel_and_holding_it_keeps_ping_mode() {
        let mut s = ready();
        let t = Instant::now();
        input(&mut s, keys(true, false, (500., 500.)), t);
        input(&mut s, keys(true, false, (500., 700.)), t);
        assert!(s.wheel.is_none() && s.armed.is_some());
        // Held past 3 s: still ping mode; a click, and ping mode stays.
        let later = t + ARMED_FOR * 2;
        assert!(input(&mut s, keys(true, false, (500., 700.)), later)
            .disarmed
            .is_none());
        input(&mut s, keys(true, true, (600., 600.)), later);
        let out = input(&mut s, keys(true, false, (600., 600.)), later);
        assert_eq!(out.click, Some((600., 600.)));
        assert!(s.armed.is_some(), "held: ping mode stays");
        // Released: the 3 s count from the release.
        input(&mut s, keys(false, false, (600., 600.)), later);
        let out = input(&mut s, keys(false, false, (600., 600.)), later + ARMED_FOR);
        assert_eq!(out.disarmed, Some("timeout"));
    }
    #[test]
    fn ping_mode_cancels_on_right_click_esc_a_command_the_hud_or_three_seconds() {
        let t = Instant::now();
        for (mode, why) in ["right_click", "esc", "command", "timeout", "hud"]
            .into_iter()
            .enumerate()
        {
            let mut s = tapped();
            let mut k = keys(false, false, (500., 500.));
            let mut at = t;
            match mode {
                0 => k.right = true,
                1 => k.escape = true,
                2 => k.stop = true,
                3 => at = t + ARMED_FOR + Duration::from_millis(600),
                _ => k.left = true,
            }
            let out = if mode == 4 {
                s.input(k, context(), at, field, |_| false)
            } else {
                input(&mut s, k, at)
            };
            assert_eq!(out.disarmed, Some(why), "mode {mode}");
            assert!(s.armed.is_none() && out.click.is_none());
            if mode == 0 {
                assert!(out.capture, "the cancelling right click is drained");
            }
            if mode == 4 {
                assert!(!s.reserved(), "the HUD click is the HUD's");
            }
        }
        // A right click while dragging cancels the wheel.
        let mut s = tapped();
        input(&mut s, keys(false, true, (500., 500.)), t);
        input(&mut s, keys(false, true, (640., 500.)), t);
        let mut k = keys(false, true, (680., 470.));
        k.right = true;
        let out = input(&mut s, k, t);
        assert!(out.cancelled.is_some() && s.wheel.is_none());
    }
    #[test]
    fn near_an_edge_the_wheel_moves_inward_and_the_gesture_follows_it() {
        let (rx, ry) = (wheel_draw::REACH_X, wheel_draw::REACH_Y);
        assert_eq!(wheel_center((10., 500.)), (rx, 500.));
        for p in [(0., 0.), (1920., 1080.)] {
            let c = wheel_center(p);
            assert!(c.0 - rx >= 0. && c.0 + rx <= 1920.);
            assert!(c.1 - ry >= 0. && c.1 + ry <= 1080.);
        }
        // Pressed at the left edge: the wheel's centre at x 152, the press
        // outside its rim; a stroke right through the ring, then up.
        let mut s = tapped();
        let out = drag(
            &mut s,
            &[(10., 500.), (60., 500.), (300., 500.), (272., 440.)],
        );
        let w = out.fired.unwrap();
        assert_eq!(
            w.anchor,
            Anchor::Field((10, 500)),
            "the order is about the press point"
        );
        assert_eq!(w.fired(), Some(Slot::AssistMe));
    }
    #[test]
    fn nothing_opens_while_blocked_or_not_controlling() {
        for mode in 0..5 {
            let mut s = ready();
            let mut c = context();
            match mode {
                0 => c.blocked = true,
                1 => c.emote_open = true,
                2 => c.controlling = false,
                3 => c.aiming = true,
                _ => c.running = false,
            }
            let t = Instant::now();
            s.input(keys(false, false, (500., 500.)), c, t, field, anywhere);
            s.input(keys(true, false, (500., 500.)), c, t, field, anywhere);
            s.input(keys(true, true, (500., 500.)), c, t, field, anywhere);
            s.input(keys(true, true, (700., 500.)), c, t, field, anywhere);
            assert!(s.wheel.is_none() && s.armed.is_none(), "mode {mode}");
        }
        // A key held through a block does not arm.
        let mut s = ready();
        let mut c = context();
        c.blocked = true;
        s.input(
            keys(true, false, (500., 500.)),
            c,
            Instant::now(),
            field,
            anywhere,
        );
        input(&mut s, keys(true, false, (500., 500.)), Instant::now());
        assert!(s.armed.is_none());
        input(&mut s, keys(false, false, (500., 500.)), Instant::now());
        input(&mut s, keys(true, false, (500., 500.)), Instant::now());
        assert!(s.armed.is_some());
    }
    #[test]
    fn the_stack_stays_clear_of_the_banners_and_the_chat_list() {
        assert!(STACK_AT.0 + stack::ROW.0 <= 214.);
        assert!(STACK_AT.1 >= 214. + 10.);
        assert!(STACK_AT.1 + stack::HEIGHT < 770. && 770. < STACK_FLOOR);
    }
    #[test]
    fn the_template_holds_every_node() {
        let t = template();
        assert!(t.contains("#capture:"));
        for node in wheel_draw::NODES.iter().skip(1) {
            let id = format!("#{}:image", node.trim_start_matches("wheel."));
            assert!(t.contains(&id), "{id}");
        }
        for id in PIN_NODES
            .iter()
            .chain(stack::NODES.iter())
            .chain(mark::HEAD_NODES.iter())
        {
            assert!(t.contains(&format!("#{id}:image")), "{id}");
        }
        assert!(t.contains("#tip_name:label") && t.contains("#tip_place:label"));
        assert!(t.contains("color: #5bd16eff"));
        assert_eq!(t.matches('{').count(), t.matches('}').count());
    }
}
