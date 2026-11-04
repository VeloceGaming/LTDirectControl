//! The ping system's orders (1.7.1-dev13, stage 1), the pure half. The
//! hook side is `windows/ping_step.rs`, called from the step hook at each
//! brain's first step in a tick, on the viewed match's worker.
//!
//! The client resolves what a ping means and who hears it
//! (`pings/intent.rs`) and leaves one order per teammate ([`post`],
//! [`post_window`], [`post_missing`]). The worker acts during each
//! recipient's own step:
//!
//! - an objective goes through the game's own setter and then stands for
//!   [`STANDING_TICKS`]: at that teammate's later steps it is set again
//!   while he holds no objective, never over one the AI chose, and the
//!   request ends at once when the game's drop reason names a permanent
//!   refusal ([`permanent`]) or the same reason three times ([`SAME_REASON`]);
//! - a fog ping (1.7.1-dev15) writes the believed position of one enemy
//!   into the teammate's vision table. The enemy is chosen once for all
//!   teammates: that lane's enemy laner, else the one the first recipient
//!   has not seen for longest among those the team cannot see now;
//! - a driven window (1.7.1-dev17, [`Drive`], the author's spec section
//!   10.3) writes one of the plan's own flags at every think of the
//!   window, the plan variant that owns the field only: hold the wave
//!   (`hold_wave` and `waiting` on a PassiveLine), regroup (`regroup_now`
//!   on a PassiveLine) and retreat (the fight plan's run-away latch on a
//!   Battle, `regroup_now` on a PassiveLine). At its end, or when a newer
//!   order replaces it, the values it changed are put back at that
//!   teammate's next think ([`Window`]).
//!
//! "On it" (1.7.1-dev15, the author's rule): a teammate who already holds
//! the asked objective's tag (any phase: Setup is never written over Hunt)
//! or the same lane order on the same lane is written nothing: his answer
//! is accepted and the order is his.
//!
//! Each teammate's answer is kept for the answer stack ([`Status`],
//! [`answers`]): pending until his step, then as the order fares.
//!
//! Superseded 2026-11-04 (1.7.1-dev17): the inbox calls (stop, Mia, the
//! hide-lane call of I'm coming) and "leave"; their slots are gone.
//!
//! The fight call keeps its own path (`signal_call::Request`, the dev8
//! proof). Everything here is plain values: nothing the game passes in is
//! kept, and each brain is read and written only during its own step.

/// A standing request asks again for 5 s of match time (dev12: 10 s).
pub(crate) const STANDING_TICKS: u64 = 300;

/// The game's objective, 3 bytes (tag, byte 1, byte 2), tag 0xff none.
/// For Morgard and Serpen byte 1 is the phase and byte 2 `with_battle`
/// (read from the code, 2026-11-04: the Debug formatter 0xed9170 prints
/// byte 1 as `phase` and byte 2 as `with_battle`, names at 0x4018098 and
/// 0x401809d); for the lane objectives byte 1 is the lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Objective(pub [u8; 3]);
impl Objective {
    /// Morgard's tag (the wheel takes the tag from the pit it learned).
    pub(crate) const MORGARD: u8 = 0;
    pub(crate) const SERPEN: u8 = 1;
    pub(crate) const DEFENSE_LINE: u8 = 3;
    pub(crate) const NEXUS: u8 = 4;
    pub(crate) const GANK: u8 = 6;
    /// Dive (tag 7, a lane in byte 1): with Gank, the objectives that keep
    /// the game's bottom-support rule off on the own lane.
    pub(crate) const DIVE: u8 = 7;
    pub(crate) const NONE: Self = Self([0xff, 0, 0]);
    /// Defense (tag 2, no payload): the base's defend (1.7.1-dev15).
    pub(crate) const DEFENSE: Self = Self([2, 0, 0]);
    /// The phase Setup (formatter 0xeda000: 1 Setup, 3 Hunt).
    pub(crate) const SETUP: u8 = 1;
    /// Serpen, phase Setup, with battle: the constant the game's Serpen
    /// handler passes (0x10101 at 0xeb75ed); seen in game (dev10, dev11).
    #[cfg(test)]
    pub(crate) const SERPEN_SETUP_BATTLE: Self = Self::pit(Self::SERPEN, Self::SETUP, true);
    /// Morgard, phase Setup, with battle: the Serpen order's twin (the
    /// tests).
    #[cfg(test)]
    pub(crate) const MORGARD_SETUP_BATTLE: Self = Self::pit(Self::MORGARD, Self::SETUP, true);
    // Superseded 2026-11-04 (1.7.1-dev15): Serpen Hunt with battle
    // (0x10301 at 0xeb768a), the dev13 research key F10, is gone; dev15's
    // F10, the back-off probe, is gone too (1.7.1-dev17).
    /// A pit's objective: Morgard or Serpen, a phase, with or without battle.
    pub(crate) const fn pit(tag: u8, phase: u8, battle: bool) -> Self {
        Self([tag, phase, battle as u8])
    }
    /// `(lane << 8) | tag`, as the game passes Gank, Nexus, DefenseLine.
    pub(crate) fn with_lane(tag: u8, lane: u8) -> Self {
        Self([tag, lane, 0])
    }
    /// The u24 the setter takes in edx.
    pub(crate) fn u24(self) -> u32 {
        u32::from(self.0[0]) | u32::from(self.0[1]) << 8 | u32::from(self.0[2]) << 16
    }
    /// A pit's objective (Morgard or Serpen): whether with battle; None
    /// for any other.
    pub(crate) fn pit_battle(self) -> Option<bool> {
        matches!(self.0[0], Self::MORGARD | Self::SERPEN).then_some(self.0[2] != 0)
    }
    pub(crate) fn is_none(self) -> bool {
        self.0[0] == Self::NONE.0[0]
    }
    pub(crate) fn tag(self) -> u8 {
        self.0[0]
    }
    fn name(self) -> String {
        let [tag, one, two] = self.0;
        let phase = |p: u8| match p {
            0 => "None",
            1 => "Setup",
            2 => "Assemble",
            3 => "Hunt",
            _ => "?",
        };
        let lane = |kind: &str| format!("{kind}({})", lane_text(one));
        match tag {
            0 | 1 => format!(
                "{}({}{})",
                if tag == 0 { "Morgard" } else { "Serpen" },
                phase(one),
                if two == 1 { ",battle" } else { "" }
            ),
            2 => "Defense".to_owned(),
            3 => lane("DefenseLine"),
            4 => lane("Nexus"),
            5 => "Repair".to_owned(),
            6 => lane("Gank"),
            7 => lane("Dive"),
            8 => lane("PressTower"),
            9 => lane("ComebackPick"),
            0xff => "none".to_owned(),
            _ => "?".to_owned(),
        }
    }
    /// `0x010101[Serpen(Setup,battle)]`: the u24, then its name.
    pub(crate) fn text(self) -> String {
        format!("{:#08x}[{}]", self.u24(), self.name())
    }
}

/// A lane by the game's number and name. Read from the code (1.7.1-dev12):
/// the big plan's payload formatters print the lane through name tables
/// that give 0 Top, 1 Mid, 2 Bottom; the dev12 log's `OBJ lanes` line
/// agreed (seen in game, 2026-11-03).
pub(crate) fn lane_text(lane: u8) -> String {
    let name = match lane {
        0 => "Top",
        1 => "Mid",
        2 => "Bottom",
        _ => "?",
    };
    format!("{lane}={name}")
}
/// A roster slot's own lane by the game's table 0x202010000 (slot 0 lane
/// 0, slot 2 lane 1, slots 3 and 4 lane 2); None for slot 1, the jungler.
pub(crate) fn own_lane(slot: u8) -> Option<u8> {
    match slot {
        0 => Some(0),
        2 => Some(1),
        3 | 4 => Some(2),
        _ => None,
    }
}

/// The drop reasons that end a standing request at once (read from the
/// code, docs/investigation-team-ai.md section 14): the monster not up
/// (0x21 Serpen, 0x24 Morgard), the map-mode gates (0x03 Nexus, 0x05
/// DefenseLine, 0x08 Dive, 0x20 Serpen, 0x23 Morgard, 0x26 Gank) and no
/// laner (0x28). Every other reason, 0x29, 0x30 and 0x31 among them, is
/// asked through.
pub(crate) fn permanent(reason: u8) -> bool {
    matches!(
        reason,
        0x21 | 0x24 | 0x03 | 0x05 | 0x08 | 0x20 | 0x23 | 0x26 | 0x28
    )
}

/// A fog ping: the believed point, the enemy named (None: the worker picks
/// the one not seen for longest) and the enemies the team sees now (by
/// player index). Superseded 2026-11-04 (1.7.1-dev21): the research key
/// F9's Mia stamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Fog {
    pub at: (u64, u64),
    pub enemy: Option<u8>,
    pub visible: [bool; 5],
}
/// What the mod sends one teammate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Send {
    Objective(Objective),
    Fog(Fog),
}
impl Send {
    pub(crate) fn text(self) -> String {
        match self {
            Send::Objective(o) => format!("objective={}", o.text()),
            Send::Fog(f) => format!(
                "fog at={},{} enemy={}",
                f.at.0,
                f.at.1,
                f.enemy
                    .map_or("unseen_longest".to_owned(), |k| k.to_string())
            ),
        }
    }
}

/// The teammate already does what `asked` asks, reading `held`: the same
/// pit or base tag in any phase, or the same lane order on the same lane.
pub(crate) fn on_it(asked: Objective, held: Objective) -> bool {
    match (asked.0, held.0) {
        ([a, ..], [h, ..]) if a != h || asked.is_none() => false,
        ([3 | 4 | 6..=9, lane, _], [_, held_lane, _]) => lane == held_lane,
        _ => true,
    }
}

/// The squared distance the game keeps as `last_visible_distance`: unsigned
/// differences, squared and added, wrapping as its `imul` does (0xecc02b).
pub(crate) fn squared_distance(a: (u64, u64), b: (u64, u64)) -> u64 {
    let (dx, dy) = (a.0.abs_diff(b.0), a.1.abs_diff(b.1));
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))
}
/// The four vision fields of enemy `k` the probe writes, as (brain offset,
/// length) given the table's offsets (position, seen, checked, distance).
pub(crate) fn vision_writes(k: u8, at: [usize; 4]) -> [(usize, usize); 4] {
    let k = usize::from(k);
    [
        (at[0].wrapping_add(k * 16), 16),
        (at[1].wrapping_add(k * 8), 8),
        (at[2].wrapping_add(k * 8), 8),
        (at[3].wrapping_add(k * 8), 8),
    ]
}

/// A standing objective request on one teammate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Standing {
    objective: Objective,
    until: u64,
    /// The reason byte and its tick right after the latest write.
    snapshot: Option<(u8, u64)>,
    held: u16,
    retries: u16,
    /// The reason the AI last cleared it with, and how many times in a
    /// row (1.7.1-dev16: three times the same is a refusal).
    cleared: Option<(u8, u8)>,
    /// The plan rebuild was asked (at most once, 1.7.1-dev18).
    rebuilt: bool,
}
/// The AI clearing a standing request this many times in a row with the
/// same fresh reason refuses it (dev15 rewrote "defend mid" 17 to 22 times
/// in 5 s with reason 0x06).
pub(crate) const SAME_REASON: u8 = 3;
/// A teammate's answer to an order, for the answer stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    /// Sent, waiting for his step or his answer (a dim row since dev16).
    Pending,
    /// A write that acts by itself (the fog ping's vision write): done.
    Sent,
    /// Never written (the setter refused, or tracing was on).
    Unsent,
    /// Holds the objective, or his plan takes the window's write.
    Accepted,
    /// Asked again: he holds none yet, or his plan does not take the
    /// window's write now.
    Busy,
    /// Refused, overridden by his own choice, or never taken.
    Declined,
}
/// What one teammate is owed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Owed {
    serial: u64,
    send: Send,
    standing: Option<Standing>,
    /// His first step under it (the publisher's wait, 1.7.1-dev18).
    since: Option<u64>,
}
/// Serpen's and Morgard's index in [`Play`]'s pairs: 0 and 1.
pub(crate) fn monster(o: Objective) -> Option<usize> {
    match o.tag() {
        Objective::SERPEN => Some(0),
        Objective::MORGARD => Some(1),
        _ => None,
    }
}
/// The team's own Serpen or Morgard play as one teammate's step shows it
/// (1.7.1-dev18, the author's spec section 11.9; read from the code,
/// docs/investigation-team-ai.md sections 27 to 30). Pairs are (Serpen,
/// Morgard).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Play {
    /// `sync_follower` as the last decision left it: Some(false) on the
    /// one teammate who publishes the team's play; None unread.
    pub follower: Option<bool>,
    /// The team's shared record for that monster is present: the team is
    /// on that play.
    pub team_on: [bool; 2],
    /// His plan is that play's hunt (SerpenHuntAndPoke, EpicHuntAndPoke).
    pub hunting: [bool; 2],
    /// The game's builder would let him walk now: the monster up, or its
    /// spawn within his travel time + 2 s.
    pub walk: [bool; 2],
}

/// One teammate's step, as the hook read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Facts {
    pub tick: u64,
    pub objective: Objective,
    /// The drop reason byte and the tick it was written (brain + 0x1b0,
    /// + 0x1b8).
    pub reason: (u8, u64),
    /// The brain's last-seen tick of each enemy (the fog ping's choice).
    pub seen: [u64; 5],
    /// His big plan is a Battle (raw 0 or 1): never rebuilt.
    pub battle: bool,
    /// The team's Serpen and Morgard play (1.7.1-dev18).
    pub play: Play,
}
/// What to do at that step.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Plan {
    pub serial: u64,
    pub set: Option<Objective>,
    /// The fog ping's vision write: the enemy index and the point.
    pub vision: Option<(u8, (u64, u64))>,
    /// Ask the game for one plan rebuild after the set (1.7.1-dev18, route
    /// C of docs/investigation-team-ai.md section 30): the hook calls the
    /// game's own replace helper with ForcePassive, while the plan is not a
    /// Battle and tracing is off.
    pub rebuild: bool,
    pub lines: Vec<String>,
}

// ---- Driven windows (1.7.1-dev17) ----

/// A driven window's kind (the author's spec section 10.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Drive {
    /// On my way: hold the wave on this lane (`hold_wave`, `waiting`).
    Hold { lane: u8 },
    /// Enemy missing: back to the own tower (`regroup_now`).
    Regroup,
    /// Retreat: the fight plan's run-away latch, or `regroup_now` on a
    /// laner.
    Retreat,
}
/// On my way's cap: our "I'm coming" cap, twice the game's 5 s "too far"
/// travel limit for a fight call. The client ends it sooner
/// (`wheel.rs`, arrival or no longer coming).
pub(crate) const HOLD_SECS: u64 = 10;
/// Enemy missing: the game's own roam length (a laner sent to a
/// neighbouring lane gets 10 s; docs/investigation-team-ai.md section 16).
pub(crate) const REGROUP_SECS: u64 = 10;
/// Retreat: the game's own run-away length (its fountain exit sets the same
/// latch to now + 5 s, section 22).
pub(crate) const RETREAT_SECS: u64 = 5;
impl Drive {
    pub(crate) fn secs(self) -> u64 {
        match self {
            Drive::Hold { .. } => HOLD_SECS,
            Drive::Regroup => REGROUP_SECS,
            Drive::Retreat => RETREAT_SECS,
        }
    }
    pub(crate) fn text(self) -> String {
        match self {
            Drive::Hold { lane } => format!("hold lane={}", lane_text(lane)),
            Drive::Regroup => "regroup".to_owned(),
            Drive::Retreat => "retreat".to_owned(),
        }
    }
}
/// The big plan's kind at a think, as the hook names it from this
/// version's tags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PlanKind {
    /// Battle (raw 0 or 1): the run-away latch is its field.
    Battle,
    /// PassiveLine (raw 3): the flags are its fields.
    PassiveLine,
    /// Any other plan, by its raw tag.
    #[default]
    Other,
}
/// One think of a teammate as the hook read it for the windows: the plan,
/// its PassiveLine flags (meaningful only on a PassiveLine), the fight
/// plan's latch (only on a Battle), the game's support rule, and the
/// match's ticks per second.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Think {
    pub tick: u64,
    pub plan: PlanKind,
    pub raw_plan: u64,
    pub hold_wave: u8,
    pub waiting: u8,
    pub regroup_now: u8,
    pub line: u8,
    pub push_basis: u8,
    /// `well_runaway`: the tag and the tick it runs to.
    pub latch: (u64, u64),
    /// The game's own bottom-support rule decides `hold_wave` itself at
    /// this think (`ping_step::support_rule`).
    pub support_rule: bool,
    /// The match's ticks per second (the word the game multiplies for its
    /// own 5 s and 30 s), 0 when unknown.
    pub tps: u64,
    /// The AI environment is kind 0 (the viewed match's).
    pub kind0: bool,
}
/// A PassiveLine flag a window writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Flag {
    HoldWave,
    Waiting,
    RegroupNow,
}
/// One write at a think. Each one fits exactly one plan kind; the hook
/// writes only while the plan read at this think is that kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Write {
    Flag(Flag, u8),
    /// `well_runaway` = (1, this tick).
    Latch(u64),
    /// `well_runaway` cleared (only the mod's own, at an early end).
    Unlatch,
}
impl Write {
    /// The plan kind that owns the field.
    pub(crate) fn owner(self) -> PlanKind {
        match self {
            Write::Flag(..) => PlanKind::PassiveLine,
            Write::Latch(_) | Write::Unlatch => PlanKind::Battle,
        }
    }
    fn text(self) -> String {
        match self {
            Write::Flag(f, v) => format!(
                "{}={v}",
                match f {
                    Flag::HoldWave => "hold_wave",
                    Flag::Waiting => "waiting",
                    Flag::RegroupNow => "regroup_now",
                }
            ),
            Write::Latch(t) => format!("well_runaway=1,{t}"),
            Write::Unlatch => "well_runaway=0".to_owned(),
        }
    }
}
/// The hold's values at its first write, put back at its end while the
/// plan is the same PassiveLine (`line` and `push_basis` unchanged).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Saved {
    hold_wave: u8,
    waiting: u8,
    line: u8,
    push_basis: u8,
}
/// A driven window on one teammate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Window {
    serial: u64,
    drive: Drive,
    /// The tick it ends, fixed at his first think in it.
    end: Option<u64>,
    accepted: bool,
    saved: Option<Saved>,
    regrouped: bool,
    /// The latch the mod wrote (its tick is `end`).
    latched: bool,
    /// The client ended it (arrival, a released control).
    closing: bool,
    status: Status,
}
impl Window {
    fn new(serial: u64, drive: Drive) -> Self {
        Self {
            serial,
            drive,
            end: None,
            accepted: false,
            saved: None,
            regrouped: false,
            latched: false,
            closing: false,
            status: Status::Pending,
        }
    }
}
/// The writes one think applies, in order, and `t` as they leave it (a
/// later window in the same think sees the restored values).
fn apply(writes: &mut Vec<Write>, t: &mut Think, w: Write) {
    if w.owner() != t.plan {
        return;
    }
    match w {
        Write::Flag(Flag::HoldWave, v) => t.hold_wave = v,
        Write::Flag(Flag::Waiting, v) => t.waiting = v,
        Write::Flag(Flag::RegroupNow, v) => t.regroup_now = v,
        Write::Latch(tick) => t.latch = (1, tick),
        Write::Unlatch => t.latch = (0, t.latch.1),
    }
    writes.push(w);
}
/// The values a window changed, put back: the hold's saved flags while the
/// plan is the same PassiveLine; `regroup_now` to 0 while a PassiveLine;
/// and at an early end the mod's own latch.
fn restore(w: &Window, early: bool, t: &mut Think, writes: &mut Vec<Write>) {
    if let Some(s) = w.saved {
        if t.plan == PlanKind::PassiveLine && t.line == s.line && t.push_basis == s.push_basis {
            apply(writes, t, Write::Flag(Flag::HoldWave, s.hold_wave));
            apply(writes, t, Write::Flag(Flag::Waiting, s.waiting));
        }
    }
    if w.regrouped {
        apply(writes, t, Write::Flag(Flag::RegroupNow, 0));
    }
    if early && w.latched && w.end.is_some_and(|end| t.latch == (1, end)) {
        apply(writes, t, Write::Unlatch);
    }
}

/// Every teammate's orders for one match.
#[derive(Default)]
pub(crate) struct Orders {
    serial: u64,
    owed: [Option<Owed>; 5],
    /// Each teammate's answer to the latest order he was sent.
    answers: [Option<(u64, Status)>; 5],
    /// Each teammate's fog note, apart from what he is owed (1.7.1-dev18,
    /// spec section 11.6: a vision note never replaces an order).
    notes: [Option<(u64, Fog)>; 5],
    /// The fog ping's enemy, chosen at its first recipient's step.
    fog: Option<(u64, u8)>,
    /// Each teammate's driven window, and one a newer order replaced,
    /// waiting for his next think to put back what it changed.
    windows: [Option<Window>; 5],
    ending: [Option<Window>; 5],
    /// Each teammate's big plan was a Battle at his latest step (the walk's
    /// exclusion, 1.7.1-dev22).
    fighting: [bool; 5],
}
impl Orders {
    /// Order `serial`'s answers by teammate index (None: not sent to him).
    pub(crate) fn answers(&self, serial: u64) -> [Option<Status>; 5] {
        self.answers.map(|a| {
            a.filter(|(n, _)| *n == serial && serial > 0)
                .map(|(_, s)| s)
        })
    }
    fn answer(&mut self, i: usize, status: Option<Status>) {
        if let Some(owed) = self.owed.get(i).copied().flatten() {
            self.answer_for(i, owed.serial, status);
        }
    }
    /// Order `serial`'s answer on teammate `i`, unless a window posted with
    /// the same serial answers for itself.
    fn answer_for(&mut self, i: usize, serial: u64, status: Option<Status>) {
        if self.windows[i].is_some_and(|w| w.serial == serial) {
            return;
        }
        if let Some(slot) = self.answers.get_mut(i) {
            *slot = status.map(|s| (serial, s));
        }
    }
    /// Teammate `i`'s window ends because a newer order replaces it: put
    /// back at his next think, if he had one in it.
    fn replace_window(&mut self, i: usize) {
        if let Some(old) = self.windows[i].take() {
            if old.end.is_some() {
                self.ending[i] = Some(old);
            }
        }
    }
    /// The client's order: `send` to every teammate index in `to`. A newer
    /// objective replaces what a teammate was still owed and his driven
    /// window. A fog ping replaces neither (1.7.1-dev18, spec section 11.6:
    /// a vision note, not an order he acts on): it waits in its own slot.
    /// Its serial.
    pub(crate) fn post(&mut self, send: Send, to: [bool; 5]) -> u64 {
        self.serial += 1;
        self.post_at(self.serial, send, to);
        self.serial
    }
    fn post_at(&mut self, serial: u64, send: Send, to: [bool; 5]) {
        for i in (0..5).filter(|i| to[*i]) {
            match send {
                Send::Fog(f) => self.notes[i] = Some((serial, f)),
                Send::Objective(_) => {
                    self.replace_window(i);
                    self.owed[i] = Some(Owed {
                        serial,
                        send,
                        standing: None,
                        since: None,
                    });
                }
            }
            self.answers[i] = Some((serial, Status::Pending));
        }
    }
    /// A driven window on every teammate index in `to`. Its serial.
    pub(crate) fn post_window(&mut self, drive: Drive, to: [bool; 5]) -> u64 {
        self.serial += 1;
        self.window_at(self.serial, drive, to);
        self.serial
    }
    fn window_at(&mut self, serial: u64, drive: Drive, to: [bool; 5]) {
        for i in (0..5).filter(|i| to[*i]) {
            self.replace_window(i);
            self.owed[i] = None;
            self.windows[i] = Some(Window::new(serial, drive));
            self.answers[i] = Some((serial, Status::Pending));
        }
    }
    /// Enemy missing: the fog ping to `fog`, the regroup window on
    /// `regroup`, one serial.
    pub(crate) fn post_missing(&mut self, f: Fog, fog: [bool; 5], regroup: [bool; 5]) -> u64 {
        self.serial += 1;
        self.post_at(self.serial, Send::Fog(f), fog);
        self.window_at(self.serial, Drive::Regroup, regroup);
        self.serial
    }
    /// The client ends order `serial`'s windows (On my way arrived, or no
    /// longer coming); None ends every window (control released).
    pub(crate) fn close(&mut self, serial: Option<u64>) {
        for w in self.windows.iter_mut().flatten() {
            if serial.is_none_or(|n| n == w.serial) {
                w.closing = true;
            }
        }
    }
    /// Teammate `index`'s step (the hook passes only the controlled
    /// athlete's teammates).
    pub(crate) fn step(&mut self, index: u8, f: Facts) -> Plan {
        let i = usize::from(index);
        let mut plan = Plan::default();
        if i >= 5 {
            return plan;
        }
        if let Some((serial, fog)) = self.notes[i].take() {
            plan.serial = serial;
            self.note(i, index, serial, fog, f, &mut plan);
        }
        let Some(mut owed) = self.owed[i] else {
            return plan;
        };
        let serial = owed.serial;
        plan.serial = serial;
        let since = *owed.since.get_or_insert(f.tick);
        let end = |why: String| format!("PING ended n={serial} to={index} {why}");
        if let Send::Objective(o) = owed.send {
            if let Some(k) = monster(o) {
                // The team's own play first (spec section 11.9).
                if f.objective.is_none() && f.play.hunting[k] {
                    plan.lines.push(end(format!(
                        "objective={} answer=on_it play plan={}",
                        o.text(),
                        if k == 0 {
                            "SerpenHuntAndPoke"
                        } else {
                            "EpicHuntAndPoke"
                        }
                    )));
                    self.answer(i, Some(Status::Accepted));
                    self.owed[i] = None;
                    return plan;
                }
                if f.play.follower == Some(false) {
                    // The publisher: never planted; he carries the team's
                    // play or starts one himself.
                    let status = if f.play.team_on[k] {
                        Status::Accepted
                    } else {
                        Status::Busy
                    };
                    self.answer(i, Some(status));
                    if f.tick > since + STANDING_TICKS {
                        plan.lines.push(end(format!(
                            "objective={} publisher result={status:?} (not planted)",
                            o.text()
                        )));
                        self.owed[i] = None;
                    } else {
                        self.owed[i] = Some(owed);
                    }
                    return plan;
                }
            }
        }
        match owed.send {
            Send::Fog(fog) => {
                // Fog pings wait in their own slot ([`Self::note`]).
                self.owed[i] = None;
                self.note(i, index, serial, fog, f, &mut plan);
                return plan;
            }
            Send::Objective(o) if on_it(o, f.objective) && owed.standing.is_none() => {
                plan.lines.push(end(format!(
                    "objective={} answer=on_it holds {}",
                    o.text(),
                    f.objective.text()
                )));
                self.answer(i, Some(Status::Accepted));
                self.owed[i] = None;
                return plan;
            }
            Send::Objective(o) => match owed.standing.as_mut() {
                None => {
                    owed.standing = Some(Standing {
                        objective: o,
                        until: f.tick + STANDING_TICKS,
                        snapshot: None,
                        held: 0,
                        retries: 0,
                        cleared: None,
                        rebuilt: false,
                    });
                    plan.set = Some(o);
                }
                Some(s) => {
                    let verdict = |s: &Standing, why: &str| {
                        end(format!(
                            "objective={} {why} held={} retries={} reason={:#04x}",
                            s.objective.text(),
                            s.held,
                            s.retries,
                            f.reason.0
                        ))
                    };
                    let fresh = s.snapshot.is_none_or(|(_, tick)| f.reason.1 > tick);
                    let (status, ended) = if f.objective == o {
                        s.held = s.held.saturating_add(1);
                        let ended =
                            (f.tick > s.until).then(|| verdict(s, "ended=window result=holds"));
                        // A pit order is taken once he walks the hunt;
                        // holding the objective alone is not (dev17: check
                        // marks without anyone moving).
                        let walking = monster(o).is_none_or(|k| f.play.hunting[k]);
                        let status = if walking {
                            Status::Accepted
                        } else {
                            Status::Busy
                        };
                        (status, ended)
                    } else if on_it(o, f.objective) {
                        let by = format!("ended=on_it holds {}", f.objective.text());
                        (Status::Accepted, Some(verdict(s, &by)))
                    } else if !f.objective.is_none() {
                        let by = format!("ended=ai_choice by={}", f.objective.text());
                        (Status::Declined, Some(verdict(s, &by)))
                    } else if fresh && permanent(f.reason.0) {
                        (Status::Declined, Some(verdict(s, "ended=refused")))
                    } else if fresh && {
                        let same = match s.cleared {
                            Some((r, k)) if r == f.reason.0 => k.saturating_add(1),
                            _ => 1,
                        };
                        s.cleared = Some((f.reason.0, same));
                        same >= SAME_REASON
                    } {
                        let why = format!("ended=refused same_reason x{SAME_REASON}");
                        (Status::Declined, Some(verdict(s, &why)))
                    } else if f.tick > s.until {
                        (
                            Status::Declined,
                            Some(verdict(s, "ended=window result=none")),
                        )
                    } else {
                        s.retries = s.retries.saturating_add(1);
                        plan.set = Some(o);
                        (Status::Busy, None)
                    };
                    self.answer(i, Some(status));
                    if let Some(line) = ended {
                        plan.lines.push(line);
                        self.owed[i] = None;
                        return plan;
                    }
                }
            },
        }
        // Route C: one rebuild per teammate and order, when the team is not
        // on the play and the game's own walk rule would pass.
        if let (Send::Objective(o), Some(s)) = (owed.send, owed.standing.as_mut()) {
            let wanted = monster(o).is_some_and(|k| !f.play.team_on[k] && f.play.walk[k]);
            let holds = plan.set.is_some() || f.objective == o;
            if wanted && holds && !f.battle && !s.rebuilt {
                s.rebuilt = true;
                plan.rebuild = true;
            }
        }
        self.owed[i] = Some(owed);
        plan
    }
    /// A fog note at teammate `i`'s step: the enemy, once for every
    /// teammate: named, or the one this first recipient has not seen for
    /// longest among those the team cannot see.
    fn note(&mut self, i: usize, index: u8, serial: u64, fog: Fog, f: Facts, plan: &mut Plan) {
        let k = match (fog.enemy, self.fog.filter(|(n, _)| *n == serial)) {
            (Some(k), _) | (None, Some((_, k))) => k,
            (None, None) => (0..5u8)
                .filter(|k| !fog.visible[usize::from(*k)])
                .min_by_key(|k| f.seen[usize::from(*k)])
                .unwrap_or(0),
        };
        self.fog = Some((serial, k));
        plan.vision = Some((k.min(4), fog.at));
        plan.lines.push(format!(
            "PING fog n={serial} to={index} enemy={k} at={},{}",
            fog.at.0, fog.at.1
        ));
        self.answer_for(i, serial, Some(Status::Sent));
    }
    /// A control swap (spec section 15.5): every order owed and every fog
    /// note dropped (the windows are closed apart, so they put back).
    pub(crate) fn drop_orders(&mut self) {
        self.owed = [None; 5];
        self.notes = [None; 5];
        self.fog = None;
    }
    /// Teammate `index`'s big plan at this step is a Battle, or not.
    pub(crate) fn set_fighting(&mut self, index: u8, battle: bool) {
        if let Some(f) = self.fighting.get_mut(usize::from(index)) {
            *f = battle;
        }
    }
    /// Teammate `index` was fighting at his latest step.
    pub(crate) fn fighting(&self, index: u8) -> bool {
        self.fighting
            .get(usize::from(index))
            .copied()
            .unwrap_or(false)
    }
    /// Teammate `index` is owed a pit order (Serpen or Morgard): only then
    /// does the hook read the team's play for him.
    pub(crate) fn owes_pit(&self, index: u8) -> bool {
        self.owed
            .get(usize::from(index))
            .copied()
            .flatten()
            .is_some_and(|o| matches!(o.send, Send::Objective(x) if monster(x).is_some()))
    }
    /// After the hook's setter call at that step: the reason snapshot.
    pub(crate) fn written(&mut self, index: u8, reason: (u8, u64)) {
        if let Some(s) = self
            .owed
            .get_mut(usize::from(index))
            .and_then(|o| o.as_mut())
            .and_then(|o| o.standing.as_mut())
        {
            s.snapshot = Some(reason);
        }
    }
    /// The setter was not called (tracing on, or not checked): the request
    /// ends.
    pub(crate) fn not_written(&mut self, index: u8) {
        self.answer(usize::from(index), Some(Status::Unsent));
        if let Some(owed) = self.owed.get_mut(usize::from(index)) {
            *owed = None;
        }
    }
    /// Teammate `index`'s think for the driven windows: the writes, each
    /// for the plan kind read at this think, in order (first what a
    /// replaced window put back, then the current window's), and the lines.
    pub(crate) fn drive(&mut self, index: u8, think: Think) -> (Vec<Write>, Vec<String>) {
        let i = usize::from(index);
        let (mut writes, mut lines) = (Vec::new(), Vec::new());
        if i >= 5 {
            return (writes, lines);
        }
        let mut t = think;
        if let Some(old) = self.ending[i].take() {
            restore(&old, true, &mut t, &mut writes);
            lines.push(format!(
                "PING drive replaced n={} to={index} {} put_back=[{}] plan={}",
                old.serial,
                old.drive.text(),
                texts(&writes),
                t.raw_plan
            ));
        }
        let Some(mut w) = self.windows[i] else {
            return (writes, lines);
        };
        let first = writes.len();
        let before = w.status;
        // Only in the viewed match's AI kind, with the match's clock known.
        if !t.kind0 || t.tps == 0 {
            w.status = Status::Declined;
            self.windows[i] = None;
            self.window_answer(i, w);
            lines.push(format!(
                "PING drive ended n={} to={index} {} result=declined (kind0={} tps={})",
                w.serial,
                w.drive.text(),
                t.kind0,
                t.tps
            ));
            return (writes, lines);
        }
        let end = *w.end.get_or_insert(t.tick + w.drive.secs() * t.tps);
        let mut why = None;
        if w.closing || t.tick >= end {
            why = Some(if w.closing { "closed" } else { "window" });
        } else {
            match w.drive {
                Drive::Hold { .. } if t.support_rule => {
                    w.status = Status::Declined;
                    why = Some("support_rule");
                }
                Drive::Hold { lane } if t.plan == PlanKind::PassiveLine && t.line == lane => {
                    w.saved.get_or_insert(Saved {
                        hold_wave: t.hold_wave,
                        waiting: t.waiting,
                        line: t.line,
                        push_basis: t.push_basis,
                    });
                    apply(&mut writes, &mut t, Write::Flag(Flag::HoldWave, 1));
                    apply(&mut writes, &mut t, Write::Flag(Flag::Waiting, 1));
                    w.accepted = true;
                    w.status = Status::Accepted;
                }
                Drive::Regroup | Drive::Retreat if t.plan == PlanKind::PassiveLine => {
                    apply(&mut writes, &mut t, Write::Flag(Flag::RegroupNow, 1));
                    w.regrouped = true;
                    w.accepted = true;
                    w.status = Status::Accepted;
                }
                Drive::Retreat if t.plan == PlanKind::Battle => {
                    // Re-asserted while the latch is off (a new fight plan
                    // starts with it clear); never over the game's own.
                    if t.latch.0 == 0 {
                        apply(&mut writes, &mut t, Write::Latch(end));
                        w.latched = true;
                    }
                    w.accepted = true;
                    w.status = Status::Accepted;
                }
                Drive::Retreat if !w.accepted => {
                    w.status = Status::Declined;
                    why = Some("own_plan");
                }
                _ => w.status = Status::Busy,
            }
        }
        if let Some(why) = why {
            restore(&w, t.tick < end, &mut t, &mut writes);
            if why != "support_rule" && why != "own_plan" {
                w.status = if w.accepted {
                    Status::Accepted
                } else {
                    Status::Declined
                };
            }
            lines.push(format!(
                "PING drive ended n={} to={index} {} why={why} result={:?} put_back=[{}] plan={}",
                w.serial,
                w.drive.text(),
                w.status,
                texts(&writes[first..]),
                t.raw_plan
            ));
            self.windows[i] = None;
        } else {
            if w.status != before {
                lines.push(format!(
                    "PING drive n={} to={index} {} answer={:?} wrote=[{}] plan={} until={end}",
                    w.serial,
                    w.drive.text(),
                    w.status,
                    texts(&writes[first..]),
                    t.raw_plan
                ));
            }
            self.windows[i] = Some(w);
        }
        self.window_answer(i, w);
        (writes, lines)
    }
    /// A window's answer, unless a later order (a fog ping that left the
    /// window running) holds teammate `i`'s answer now.
    fn window_answer(&mut self, i: usize, w: Window) {
        if self.answers[i].is_none_or(|(n, _)| n == w.serial) {
            self.answers[i] = Some((w.serial, w.status));
        }
    }
}
fn texts(writes: &[Write]) -> String {
    writes
        .iter()
        .map(|w| w.text())
        .collect::<Vec<_>>()
        .join(",")
}

/// Client: order `serial`'s answers by teammate index.
pub(crate) fn answers(serial: u64) -> [Option<Status>; 5] {
    super::signal_call::with(|s| s.pings.answers(serial)).unwrap_or_default()
}
/// Client: leave `send` for every teammate index in `to`. Its serial; None
/// when no lock.
pub(crate) fn post(send: Send, to: [bool; 5]) -> Option<u64> {
    super::signal_call::with(|s| s.pings.post(send, to))
}
/// Client: a driven window on every teammate index in `to`.
pub(crate) fn post_window(drive: Drive, to: [bool; 5]) -> Option<u64> {
    super::signal_call::with(|s| s.pings.post_window(drive, to))
}
/// Client: Enemy missing's fog ping and regroup window.
pub(crate) fn post_missing(f: Fog, fog: [bool; 5], regroup: [bool; 5]) -> Option<u64> {
    super::signal_call::with(|s| s.pings.post_missing(f, fog, regroup))
}
/// Client: teammate `index` was fighting at his latest step.
pub(crate) fn fighting(index: u8) -> bool {
    super::signal_call::with(|s| s.pings.fighting(index)).unwrap_or(false)
}
/// Client: end order `serial`'s windows (None: all of them).
pub(crate) fn close(serial: Option<u64>) {
    let _ = super::signal_call::with(|s| s.pings.close(serial));
}

#[cfg(test)]
mod tests {
    use super::*;

    const SERPEN_TAKE: Objective = Objective::pit(Objective::SERPEN, Objective::SETUP, false);
    fn facts(tick: u64, objective: Objective, reason: (u8, u64)) -> Facts {
        Facts {
            tick,
            objective,
            reason,
            seen: [0; 5],
            battle: false,
            play: Play {
                follower: Some(true),
                ..Play::default()
            },
        }
    }
    fn one(index: usize) -> [bool; 5] {
        std::array::from_fn(|i| i == index)
    }
    /// A laner's think on PassiveLine, lane `line`.
    fn laning(tick: u64, line: u8) -> Think {
        Think {
            tick,
            plan: PlanKind::PassiveLine,
            raw_plan: 3,
            line,
            push_basis: 2,
            tps: 60,
            kind0: true,
            ..Think::default()
        }
    }
    fn fighting(tick: u64) -> Think {
        Think {
            plan: PlanKind::Battle,
            raw_plan: 1,
            ..laning(tick, 0)
        }
    }

    #[test]
    fn the_battle_byte_is_byte_two_and_the_phase_byte_one() {
        assert_eq!(Objective::SERPEN_SETUP_BATTLE.u24(), 0x01_01_01);
        assert_eq!(Objective::MORGARD_SETUP_BATTLE.u24(), 0x01_01_00);
        assert_eq!(SERPEN_TAKE.u24(), 0x00_01_01);
        assert_eq!(SERPEN_TAKE.text(), "0x000101[Serpen(Setup)]");
        assert_eq!(
            Objective::with_lane(Objective::NEXUS, 2).text(),
            "0x000204[Nexus(2=Bottom)]"
        );
        assert_eq!(Objective::NONE.u24(), 0xff);
    }

    #[test]
    fn an_objective_is_set_then_asked_again_only_while_none() {
        let mut o = Orders::default();
        let n = o.post(Send::Objective(SERPEN_TAKE), one(3));
        let p = o.step(3, facts(100, Objective::NONE, (0x20, 10)));
        assert_eq!((p.serial, p.set), (n, Some(SERPEN_TAKE)));
        o.written(3, (0x20, 10));
        assert_eq!(o.step(3, facts(110, SERPEN_TAKE, (0x20, 10))).set, None);
        let p = o.step(3, facts(120, Objective::NONE, (0x29, 115)));
        assert_eq!(p.set, Some(SERPEN_TAKE));
        o.written(3, (0x29, 115));
        assert_eq!(
            o.step(3, facts(130, Objective::NONE, (0x29, 115))).set,
            Some(SERPEN_TAKE)
        );
        assert_eq!(
            o.step(2, facts(130, Objective::NONE, (0, 0))),
            Plan::default()
        );
    }

    #[test]
    fn a_permanent_reason_newer_than_the_write_ends_it_and_an_old_one_does_not() {
        let mut o = Orders::default();
        o.post(Send::Objective(SERPEN_TAKE), one(0));
        o.step(0, facts(100, Objective::NONE, (0x21, 40)));
        o.written(0, (0x21, 40));
        assert_eq!(
            o.step(0, facts(110, Objective::NONE, (0x21, 40))).set,
            Some(SERPEN_TAKE)
        );
        o.written(0, (0x21, 40));
        let p = o.step(0, facts(120, Objective::NONE, (0x21, 118)));
        assert_eq!(p.set, None);
        assert!(p.lines[0].contains("ended=refused"), "{:?}", p.lines);
        assert!(p.lines[0].contains("reason=0x21"));
        assert_eq!(o.step(0, facts(130, Objective::NONE, (0, 0))).set, None);
    }

    #[test]
    fn it_never_writes_over_the_ais_own_choice_and_ends_with_the_window() {
        let mut o = Orders::default();
        let nexus = Objective::with_lane(Objective::NEXUS, 0);
        o.post(Send::Objective(nexus), one(2));
        o.step(2, facts(100, Objective::NONE, (0, 0)));
        o.written(2, (0, 0));
        let p = o.step(2, facts(105, Objective::SERPEN_SETUP_BATTLE, (0, 0)));
        assert_eq!(p.set, None);
        assert!(p.lines[0].contains("ended=ai_choice by=0x010101"));
        o.post(Send::Objective(nexus), one(2));
        o.step(2, facts(200, Objective::NONE, (0, 0)));
        o.written(2, (0, 0));
        assert!(o.step(2, facts(400, nexus, (0, 0))).lines.is_empty());
        let p = o.step(2, facts(200 + STANDING_TICKS + 1, nexus, (0, 0)));
        assert!(p.lines[0].contains("ended=window result=holds held=2"));
        assert!(!permanent(0x04) && permanent(0x03) && permanent(0x28));
        assert!(!permanent(0x30) && !permanent(0x29) && !permanent(0x31));
    }

    #[test]
    fn a_teammate_on_it_is_written_nothing_and_accepted() {
        let mut o = Orders::default();
        let hunt = Objective::pit(Objective::SERPEN, 3, true);
        let n = o.post(Send::Objective(SERPEN_TAKE), one(1));
        let p = o.step(1, facts(100, hunt, (0, 0)));
        assert_eq!(p.set, None, "Setup never over Hunt");
        assert!(p.lines[0].contains("answer=on_it"), "{:?}", p.lines);
        assert_eq!(o.answers(n)[1], Some(Status::Accepted));
        let top = Objective::with_lane(Objective::NEXUS, 0);
        let mid = Objective::with_lane(Objective::NEXUS, 1);
        assert!(on_it(top, top) && !on_it(top, mid) && !on_it(top, Objective::NONE));
        assert!(on_it(Objective::DEFENSE, Objective::DEFENSE));
        o.post(Send::Objective(top), one(2));
        assert_eq!(o.step(2, facts(120, mid, (0, 0))).set, Some(top));
    }

    #[test]
    fn a_fog_ping_writes_the_unseen_longest_enemy_once_for_all() {
        let mut o = Orders::default();
        let fog = Fog {
            at: (300_000, 400_000),
            enemy: None,
            visible: [true, false, false, true, false],
        };
        let n = o.post(Send::Fog(fog), [true, true, false, false, false]);
        let mut f = facts(100, Objective::NONE, (0, 0));
        // Enemy 0 is seen longest ago but visible now: enemy 4 it is.
        f.seen = [1, 90, 50, 2, 40];
        assert_eq!(o.step(0, f).vision, Some((4, (300_000, 400_000))));
        assert_eq!(o.answers(n)[0], Some(Status::Sent));
        // The second teammate gets the same enemy, whatever he saw.
        f.seen = [0, 0, 0, 0, 99];
        assert_eq!(o.step(1, f).vision.map(|v| v.0), Some(4));
        o.post(
            Send::Fog(Fog {
                enemy: Some(2),
                ..fog
            }),
            one(3),
        );
        assert_eq!(o.step(3, f).vision.map(|v| v.0), Some(2));
    }

    #[test]
    fn the_same_reason_three_times_in_a_row_is_a_refusal() {
        let mut o = Orders::default();
        let defense = Objective::with_lane(Objective::DEFENSE_LINE, 1);
        let n = o.post(Send::Objective(defense), one(3));
        o.step(3, facts(100, Objective::NONE, (0, 0)));
        o.written(3, (0, 0));
        for (k, tick) in [110u64, 120].into_iter().enumerate() {
            let p = o.step(3, facts(tick, Objective::NONE, (0x06, tick - 2)));
            assert_eq!(p.set, Some(defense), "clear {}", k + 1);
            o.written(3, (0x06, tick - 2));
        }
        let p = o.step(3, facts(130, Objective::NONE, (0x06, 128)));
        assert_eq!(p.set, None);
        assert!(
            p.lines[0].contains("ended=refused same_reason x3 held=0 retries=2 reason=0x06"),
            "{:?}",
            p.lines
        );
        assert_eq!(o.answers(n)[3], Some(Status::Declined));
    }

    #[test]
    fn a_newer_order_replaces_the_old_and_the_setter_refused_ends_it() {
        // Superseded 2026-11-04 (1.7.1-dev18, spec section 11.6): a fog
        // ping no longer replaces the take; the take is asked again at the
        // same step.
        let mut o = Orders::default();
        let take = o.post(Send::Objective(SERPEN_TAKE), one(3));
        o.step(3, facts(100, Objective::NONE, (0, 0)));
        let fog = Fog {
            at: (1, 1),
            enemy: Some(0),
            visible: [false; 5],
        };
        o.post(Send::Fog(fog), one(3));
        let p = o.step(3, facts(101, Objective::NONE, (0, 0)));
        assert_eq!(
            (p.serial, p.set, p.vision.is_some()),
            (take, Some(SERPEN_TAKE), true)
        );
        let n = o.post(Send::Objective(SERPEN_TAKE), one(3));
        o.step(3, facts(200, Objective::NONE, (0, 0)));
        o.not_written(3);
        assert_eq!(o.answers(n)[3], Some(Status::Unsent));
        assert_eq!(
            o.step(3, facts(201, Objective::NONE, (0, 0))),
            Plan::default()
        );
    }

    #[test]
    fn a_hold_writes_both_flags_at_every_think_and_puts_them_back() {
        let mut o = Orders::default();
        let n = o.post_window(Drive::Hold { lane: 2 }, one(3));
        assert_eq!(o.answers(n)[3], Some(Status::Pending));
        let mut t = laning(1000, 2);
        t.waiting = 1;
        let (w, lines) = o.drive(3, t);
        assert_eq!(
            w,
            [
                Write::Flag(Flag::HoldWave, 1),
                Write::Flag(Flag::Waiting, 1)
            ]
        );
        assert!(lines[0].contains("answer=Accepted") && lines[0].contains("until=1600"));
        assert_eq!(o.answers(n)[3], Some(Status::Accepted));
        // Every think again; a fight meanwhile: busy, nothing written.
        assert_eq!(o.drive(3, laning(1100, 2)).0.len(), 2);
        assert!(o.drive(3, fighting(1200)).0.is_empty());
        assert_eq!(o.answers(n)[3], Some(Status::Busy));
        // The window's end: the saved values back (hold 0, waiting 1).
        let (w, lines) = o.drive(3, laning(1600, 2));
        assert_eq!(
            w,
            [
                Write::Flag(Flag::HoldWave, 0),
                Write::Flag(Flag::Waiting, 1)
            ]
        );
        assert!(lines[0].contains("why=window result=Accepted"), "{lines:?}");
        assert_eq!(o.answers(n)[3], Some(Status::Accepted));
        assert!(o.drive(3, laning(1700, 2)).0.is_empty(), "done");
    }

    #[test]
    fn a_rebuilt_plan_keeps_its_own_values_and_the_client_can_close() {
        let mut o = Orders::default();
        let n = o.post_window(Drive::Hold { lane: 0 }, one(0));
        o.drive(0, laning(100, 0));
        o.close(Some(n));
        // Closed while the plan was rebuilt (push_basis changed): nothing
        // is put back.
        let mut t = laning(130, 0);
        t.push_basis = 5;
        t.hold_wave = 1;
        let (w, lines) = o.drive(0, t);
        assert!(w.is_empty());
        assert!(lines[0].contains("why=closed"), "{lines:?}");
        // On another lane, or never laning: declined at the end.
        let n = o.post_window(Drive::Hold { lane: 1 }, one(0));
        o.drive(0, laning(200, 0));
        assert_eq!(o.answers(n)[0], Some(Status::Busy));
        o.drive(0, laning(200 + 600, 0));
        assert_eq!(o.answers(n)[0], Some(Status::Declined));
    }

    #[test]
    fn the_bottom_supports_hold_is_refused_once_the_games_rule_runs() {
        let mut o = Orders::default();
        let n = o.post_window(Drive::Hold { lane: 2 }, one(4));
        let mut t = laning(100, 2);
        t.support_rule = true;
        let (w, lines) = o.drive(4, t);
        assert!(w.is_empty());
        assert!(
            lines[0].contains("why=support_rule result=Declined"),
            "{lines:?}"
        );
        assert_eq!(o.answers(n)[4], Some(Status::Declined));
    }

    #[test]
    fn retreat_latches_a_fight_regroups_a_laner_and_declines_the_rest() {
        let mut o = Orders::default();
        let n = o.post_window(Drive::Retreat, [true, true, true, false, false]);
        // A fighter: the latch to the window's end (5 s at 60 ticks).
        let (w, _) = o.drive(0, fighting(100));
        assert_eq!(w, [Write::Latch(400)]);
        // Latched (the game's update keeps it): not written again.
        let mut t = fighting(110);
        t.latch = (1, 400);
        assert!(o.drive(0, t).0.is_empty());
        // A new fight plan starts with it clear: re-asserted.
        assert_eq!(o.drive(0, fighting(120)).0, [Write::Latch(400)]);
        // A laner: regroup.
        assert_eq!(
            o.drive(1, laning(100, 0)).0,
            [Write::Flag(Flag::RegroupNow, 1)]
        );
        // The jungler: his own plan, declined at once.
        let jungle = Think {
            plan: PlanKind::Other,
            raw_plan: 7,
            ..laning(100, 0)
        };
        let (w, lines) = o.drive(2, jungle);
        assert!(
            w.is_empty() && lines[0].contains("why=own_plan"),
            "{lines:?}"
        );
        assert_eq!(o.answers(n)[2], Some(Status::Declined));
        // At the end the latch clears itself; the laner's flag goes back.
        let mut t = fighting(400);
        t.latch = (1, 400);
        assert!(o.drive(0, t).0.is_empty());
        assert_eq!(
            o.drive(1, laning(400, 0)).0,
            [Write::Flag(Flag::RegroupNow, 0)]
        );
    }

    #[test]
    fn a_newer_window_puts_back_the_old_one_without_undoing_itself() {
        // Enemy missing, then Retreat on the same laner.
        let mut o = Orders::default();
        let fog = Fog {
            at: (1, 1),
            enemy: None,
            visible: [false; 5],
        };
        let n = o.post_missing(fog, [true; 5], one(2));
        assert_eq!(o.step(2, facts(100, Objective::NONE, (0, 0))).serial, n);
        assert_eq!(o.answers(n)[2], Some(Status::Pending), "the window answers");
        o.drive(2, laning(100, 1));
        assert_eq!(o.answers(n)[2], Some(Status::Accepted));
        o.post_window(Drive::Retreat, one(2));
        let mut t = laning(110, 1);
        t.regroup_now = 1;
        let (w, _) = o.drive(2, t);
        assert_eq!(
            w,
            [
                Write::Flag(Flag::RegroupNow, 0),
                Write::Flag(Flag::RegroupNow, 1)
            ],
            "the old put back first, then the new: 1 stays"
        );
        // An early end of a retreat clears only the mod's own latch.
        let mut o = Orders::default();
        let n = o.post_window(Drive::Retreat, one(0));
        o.drive(0, fighting(100));
        o.close(Some(n));
        let mut t = fighting(130);
        t.latch = (1, 400);
        assert_eq!(o.drive(0, t).0, [Write::Unlatch]);
        let n = o.post_window(Drive::Retreat, one(0));
        o.drive(0, fighting(200));
        o.close(Some(n));
        let mut t = fighting(230);
        t.latch = (1, 999);
        assert!(o.drive(0, t).0.is_empty(), "the game's own latch");
    }

    #[test]
    fn a_fog_ping_never_ends_a_running_objective() {
        let mut o = Orders::default();
        let take = o.post(Send::Objective(SERPEN_TAKE), one(3));
        o.step(3, facts(100, Objective::NONE, (0, 0)));
        o.written(3, (0, 0));
        let fog = Fog {
            at: (1, 1),
            enemy: Some(0),
            visible: [false; 5],
        };
        let n = o.post(Send::Fog(fog), [true; 5]);
        // His next step: the vision write and the take asked again.
        let p = o.step(3, facts(110, Objective::NONE, (0, 0)));
        assert_eq!(p.vision, Some((0, (1, 1))));
        assert_eq!(p.set, Some(SERPEN_TAKE));
        assert_eq!(o.answers(n)[0], Some(Status::Pending), "not his step yet");
        assert_eq!(o.answers(take)[3], Some(Status::Busy));
        let p = o.step(0, facts(110, Objective::NONE, (0, 0)));
        assert_eq!(
            (p.vision.is_some(), o.answers(n)[0]),
            (true, Some(Status::Sent))
        );
    }

    #[test]
    fn the_take_order_follows_the_teams_own_play() {
        let take = Send::Objective(SERPEN_TAKE);
        let with = |play: Play| Facts {
            play,
            ..facts(100, Objective::NONE, (0, 0))
        };
        // 1. The publisher is never planted: busy, accepted once the team
        // plays, ended after the standing window.
        let mut o = Orders::default();
        let n = o.post(take, one(0));
        let publisher = Play {
            follower: Some(false),
            ..Play::default()
        };
        let p = o.step(0, with(publisher));
        assert_eq!((p.set, p.rebuild), (None, false));
        assert_eq!(o.answers(n)[0], Some(Status::Busy));
        let on = Play {
            team_on: [true, false],
            ..publisher
        };
        o.step(0, with(on));
        assert_eq!(o.answers(n)[0], Some(Status::Accepted));
        let p = o.step(
            0,
            Facts {
                tick: 100 + STANDING_TICKS + 1,
                ..with(on)
            },
        );
        assert!(p.lines[0].contains("publisher"), "{:?}", p.lines);
        assert_eq!(o.step(0, with(on)), Plan::default(), "done");
        // 2. A member of the play is on it: nothing written.
        let mut o = Orders::default();
        let n = o.post(take, one(1));
        let member = Play {
            follower: Some(true),
            team_on: [true, false],
            hunting: [true, false],
            walk: [true, true],
        };
        let p = o.step(1, with(member));
        assert_eq!((p.set, p.rebuild), (None, false));
        assert_eq!(o.answers(n)[1], Some(Status::Accepted));
        // Morgard's hunt is not Serpen's.
        let mut o = Orders::default();
        o.post(take, one(1));
        let p = o.step(
            1,
            with(Play {
                hunting: [false, true],
                ..member
            }),
        );
        assert_eq!(p.set, Some(SERPEN_TAKE));
        assert!(!p.rebuild, "the team is on Serpen: no rebuild");
        // 3. The team is not on it: planted, one rebuild when he can walk,
        // never on a fight plan; busy until he walks the hunt.
        let follower = Play {
            follower: Some(true),
            ..Play::default()
        };
        let mut o = Orders::default();
        let n = o.post(take, one(2));
        let p = o.step(2, with(follower));
        assert_eq!(
            (p.set, p.rebuild),
            (Some(SERPEN_TAKE), false),
            "cannot walk yet"
        );
        o.written(2, (0, 0));
        let walk = Play {
            walk: [true, false],
            ..follower
        };
        let fight = Facts {
            battle: true,
            objective: SERPEN_TAKE,
            ..with(walk)
        };
        assert!(!o.step(2, Facts { tick: 110, ..fight }).rebuild);
        let held = Facts {
            tick: 120,
            objective: SERPEN_TAKE,
            ..with(walk)
        };
        assert!(o.step(2, held).rebuild);
        assert_eq!(o.answers(n)[2], Some(Status::Busy), "holds, does not walk");
        assert!(
            !o.step(2, Facts { tick: 130, ..held }).rebuild,
            "once per order"
        );
        let hunting = Play {
            hunting: [true, false],
            ..walk
        };
        o.step(
            2,
            Facts {
                tick: 140,
                play: hunting,
                ..held
            },
        );
        assert_eq!(o.answers(n)[2], Some(Status::Accepted));
        // A new order may rebuild again.
        o.post(take, one(2));
        let p = o.step(
            2,
            Facts {
                tick: 150,
                objective: Objective::NONE,
                ..held
            },
        );
        assert_eq!((p.set, p.rebuild), (Some(SERPEN_TAKE), true));
    }

    #[test]
    fn a_fog_ping_leaves_a_running_window_alone_and_answers_for_itself() {
        let fog = Fog {
            at: (1, 1),
            enemy: Some(0),
            visible: [false; 5],
        };
        // On my way's hold on the bottom laner, then a fog ping to everyone
        // (the armed click) and Enemy missing elsewhere (its fog part).
        for missing in [false, true] {
            let mut o = Orders::default();
            let hold = o.post_window(Drive::Hold { lane: 2 }, one(3));
            o.drive(3, laning(100, 2));
            let n = if missing {
                o.post_missing(fog, [true; 5], one(2))
            } else {
                o.post(Send::Fog(fog), [true; 5])
            };
            assert_eq!(o.step(3, facts(110, Objective::NONE, (0, 0))).serial, n);
            assert_eq!(o.answers(n)[3], Some(Status::Sent));
            // The hold keeps writing, nothing put back, and the fog's
            // answer stays.
            let (w, lines) = o.drive(3, laning(120, 2));
            assert_eq!(
                w,
                [
                    Write::Flag(Flag::HoldWave, 1),
                    Write::Flag(Flag::Waiting, 1)
                ],
                "{lines:?}"
            );
            assert_eq!(o.answers(n)[3], Some(Status::Sent));
            assert_eq!(o.answers(hold)[3], None, "the latest order's row");
        }
        // An objective order still replaces the window.
        let mut o = Orders::default();
        o.post_window(Drive::Hold { lane: 2 }, one(3));
        let mut t = laning(100, 2);
        t.hold_wave = 0;
        o.drive(3, t);
        o.post(Send::Objective(SERPEN_TAKE), one(3));
        let (_, lines) = o.drive(3, laning(110, 2));
        assert!(lines[0].contains("drive replaced"), "{lines:?}");
    }

    #[test]
    fn no_write_ever_goes_under_another_plan() {
        let kinds = [PlanKind::Battle, PlanKind::PassiveLine, PlanKind::Other];
        for drive in [Drive::Hold { lane: 0 }, Drive::Regroup, Drive::Retreat] {
            for a in kinds {
                for b in kinds {
                    for c in kinds {
                        let mut o = Orders::default();
                        o.post_window(drive, one(0));
                        for (k, plan) in [a, b, c].into_iter().enumerate() {
                            let t = Think {
                                plan,
                                ..laning(100 + 200 * k as u64, 0)
                            };
                            let (writes, _) = o.drive(0, t);
                            assert!(
                                writes.iter().all(|w| w.owner() == plan),
                                "{drive:?} {writes:?}"
                            );
                            if k == 1 {
                                o.post_window(Drive::Retreat, one(0));
                            }
                        }
                    }
                }
            }
        }
        // Not kind 0, or no clock: nothing written, declined.
        let mut o = Orders::default();
        let n = o.post_window(Drive::Regroup, one(0));
        let t = Think {
            kind0: false,
            ..laning(100, 0)
        };
        assert!(o.drive(0, t).0.is_empty());
        assert_eq!(o.answers(n)[0], Some(Status::Declined));
    }
}
