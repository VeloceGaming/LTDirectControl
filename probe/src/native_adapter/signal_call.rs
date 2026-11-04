//! One call from the controlled athlete into the game AI's own signal
//! queues, the pure half (research builds since 1.7.1-dev5; the ping
//! system's fight call in every build since 1.7.1-dev13). The hook that
//! writes it is `windows/signal_send.rs`, from the step hook.
//!
//! What the 1.7.1-dev4 log and the code showed (2026-11-03): queue A
//! belongs to one player's AI brain, not to a team, and is that player's
//! inbox: a 40-byte entry {due tick, sender index, 15, signal}. So one call
//! to the four teammates is one entry in each teammate's own queue A,
//! written while that teammate's AI step runs.
//!
//! The parts:
//! - the request the client leaves for the worker ([`request`]);
//! - the SDK roster the controlled athlete's simulation step publishes
//!   ([`publish_roster`]), and the [`Owners`] that find, at every brain's
//!   step, which brain is the controlled athlete's (1.7.1-dev7: from the
//!   owner's entity id in the brain's player state);
//! - the entry itself ([`entry`]), marked as the mod's (1.7.1-dev8);
//! - the watch window the simulation step reads ([`watch`]);
//! - the game's answers to the mod's call, which the HUD shows for a
//!   moment by each teammate (1.7.1-dev8, [`answered`], [`replies`]).
//!
//! Threads: the client only writes the request; the simulation step of the
//! viewed match writes the roster; the hook (the same worker) reads both.
//! One short lock, never held while the game runs.
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// The kind sent in this build: BattleHelp (tag 4).
pub(crate) const BATTLE_HELP: u8 = 4;
/// The game's own constant at +0xc and its delay: due = now + 15 (the
/// research lever keys kept it until 1.7.1-dev21); the ping system's calls are due at once
/// (1.7.1-dev13), heard at the receiver's next delivery (the late rule
/// keeps an entry up to 60 ticks past its due tick).
pub(crate) const DELAY: u64 = 15;
/// A fight call lives this many ticks from the hook first seeing it: the
/// pin's 4 s (1.7.1-dev15, the author's rule "the ping lives for its pin";
/// until dev14 2 s, then it ended once every teammate was written to).
pub(crate) const REQUEST_TICKS: u64 = 240;
/// At most one fight-call write per teammate in this many ticks (1 s).
pub(crate) const WRITE_EVERY: u64 = 60;
/// The watch lasts 10 s after the first entry was written, a line a second.
pub(crate) const WATCH_TICKS: u64 = 600;
#[cfg_attr(not(feature = "research"), allow(dead_code))]
pub(crate) const WATCH_EVERY: u64 = 60;
/// A teammate's answer shows for this long after it came, in match ticks.
/// A test setting in 1.7.1-dev9: 3 s (dev8: 1.5 s, 90).
pub(crate) const REPLY_TICKS: u64 = 180;

/// A queue-A entry: due tick, sender index, the constant 15, then the
/// signal (tag, lane at +0x11, the mod's mark at +0x14, target entity id
/// at +0x18). Superseded 2026-11-03 (1.7.1-dev8): until dev7 +0x14 was 0,
/// so the mod's entries could not be told from the athlete's own AI's. The
/// lane is 0 for BattleHelp; 1.7.1-dev10's HideLine carries one.
pub(crate) fn entry(due: u64, sender: u32, tag: u8, lane: u8, target: u64, mark: u32) -> [u8; 40] {
    let mut e = [0u8; 40];
    e[..8].copy_from_slice(&due.to_le_bytes());
    e[8..12].copy_from_slice(&sender.to_le_bytes());
    e[0xc..0x10].copy_from_slice(&(DELAY as u32).to_le_bytes());
    e[0x10] = tag;
    e[0x11] = lane;
    e[0x14..0x18].copy_from_slice(&mark.to_le_bytes());
    e[0x18..0x20].copy_from_slice(&target.to_le_bytes());
    e
}

/// One athlete as the SDK sees him.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RosterPlayer {
    pub player: usize,
    pub side: usize,
    pub lane: Option<u32>,
    /// His champion's entity id (the id the game's own calls target).
    pub entity: usize,
    pub controlled: bool,
}

/// The controlled athlete's brain: its side (`+0x820` of the player
/// state), its index (`+0x8b0`) and the entity id it showed.
///
/// Superseded 2026-11-03 (1.7.1-dev7): dev5 and dev6 found it by scanning
/// each brain once per match for the roster's entity ids. That broke twice
/// in the dev6 log: when a champion died or respawned the roster changed
/// and the old hits matched nothing ("0 candidate fields"), and a match on
/// a new worker thread was never seen as new, so nothing was scanned again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Mapping {
    pub side: u8,
    pub index: u8,
    pub entity: usize,
}
/// What each brain's step shows of its owner (the entity id in its player
/// state, seen in game to be the owner's champion), and the mapping it
/// gives.
///
/// Since 1.7.1-dev22 (the author's spec section 15.5) the mapping follows
/// the roster's controlled athlete: when the roster names an entity other
/// than the mapping's and a brain owns it, that brain is the mapping (the
/// same brain: a respawn, the entity only; another: a control swap,
/// [`Owners::swapped`]); when no brain owns it yet the mapping is cleared
/// until one does. Superseded 2026-11-04: "kept for the match once
/// resolved: the controlled athlete's side and index cannot change within
/// one" (seen in game, dev21: after a swap from mid to bottom the calls
/// still left out mid and reached the player's own brain).
#[derive(Default)]
pub(crate) struct Owners {
    entity: [Option<usize>; 10],
    pub mapping: Option<Mapping>,
    /// The mapping before it was cleared, to tell a swap at the next
    /// resolve.
    last: Option<Mapping>,
    /// The controlled athlete's side or index changed (a control swap):
    /// the caller ends every order, window and walk, then clears it.
    pub swapped: bool,
    /// (own brain's entity, controlled entity) last said to disagree.
    disagreed: Option<(usize, usize)>,
}
impl Owners {
    /// Brain (side, index) stepped showing `entity`; `controlled` is the
    /// roster's controlled athlete (None while dead or unknown). Lines for
    /// the log: the owner when it changes, the mapping when it resolves,
    /// and any disagreement after (log only: the mapping is kept).
    pub(crate) fn step(
        &mut self,
        side: u8,
        index: u8,
        entity: usize,
        controlled: Option<&RosterPlayer>,
    ) -> Vec<String> {
        let mut lines = Vec::new();
        let slot = usize::from(side) * 5 + usize::from(index);
        if slot >= 10 {
            return lines;
        }
        if self.entity[slot].replace(entity) != Some(entity) {
            lines.push(format!(
                "PING map owner team={side} index={index} entity={entity}"
            ));
        }
        let Some(c) = controlled else {
            return lines;
        };
        let own = self
            .mapping
            .is_some_and(|m| (m.side, m.index) == (side, index));
        let here = Mapping {
            side,
            index,
            entity,
        };
        match self.mapping {
            None if entity == c.entity => {
                self.mapping = Some(here);
                if self.last.take().is_some_and(|l| (l.side, l.index) != (side, index)) {
                    self.swapped = true;
                }
                lines.push(format!(
                    "PING map resolved team={side} index={index} entity={entity} by=player+0x8 sdk_side={} lane_agrees={}",
                    c.side,
                    if c.lane == Some(u32::from(index)) {
                        "yes"
                    } else {
                        "no"
                    }
                ));
            }
            // The roster names another entity: the brain owning it is the
            // controlled athlete's (spec section 15.5).
            Some(m) if c.entity != m.entity && entity == c.entity => {
                self.mapping = Some(here);
                if !own {
                    self.swapped = true;
                    lines.push(format!(
                        "PING map re-resolved team={side} index={index} entity={entity} was team={} index={} entity={}",
                        m.side, m.index, m.entity
                    ));
                }
            }
            Some(m) if c.entity != m.entity && !self.entity.contains(&Some(c.entity)) => {
                self.mapping = None;
                self.last = Some(m);
                lines.push(format!(
                    "PING map cleared: the controlled entity={} is no brain's yet (was team={} index={})",
                    c.entity, m.side, m.index
                ));
            }
            Some(m) if entity == c.entity && !own => self.disagree(
                &mut lines,
                (entity, c.entity),
                format!(
                    "PING map disagrees: team={side} index={index} also shows the controlled entity={entity}; kept team={} index={}",
                    m.side, m.index
                ),
            ),
            Some(m) if own && entity != c.entity && c.entity == m.entity => self.disagree(
                &mut lines,
                (entity, c.entity),
                format!(
                    "PING map disagrees: his brain team={side} index={index} shows entity={entity}, the roster entity={} (kept)",
                    c.entity
                ),
            ),
            _ => {}
        }
        lines
    }
    /// Says a disagreement once per pair of ids (a respawn shows one until
    /// the roster is published again, up to a second later).
    fn disagree(&mut self, lines: &mut Vec<String>, pair: (usize, usize), line: String) {
        if self.disagreed.replace(pair) != Some(pair) {
            lines.push(line);
        }
    }
    /// The (side, index) of the brain that last showed `entity`.
    pub(crate) fn brain_of(&self, entity: usize) -> Option<(u8, u8)> {
        let slot = self.entity.iter().position(|e| *e == Some(entity))?;
        Some(((slot / 5) as u8, (slot % 5) as u8))
    }
    /// The entity brain (side, index) last showed.
    pub(crate) fn entity_of(&self, side: u8, index: u8) -> Option<usize> {
        let slot = usize::from(side) * 5 + usize::from(index);
        *self.entity.get(slot)?
    }
}

/// The one pending call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Request {
    pub serial: u64,
    pub target: usize,
    /// The tick the hook first saw it.
    pub seen: Option<u64>,
    /// The tick of the first entry written (the watch starts there).
    pub sent: Option<u64>,
    /// Teammate indexes already written to, and those whose full queue was
    /// already logged.
    pub done: [bool; 5],
    pub dropped: [bool; 5],
    pub ended: bool,
    /// Teammates the mod opened a scene for (1.7.1-dev7): at most one per
    /// teammate and hearing.
    pub acted: [bool; 5],
    /// 1.7.1-dev15: teammates who answered busy (fighting someone else),
    /// those already asked a second time, and each one's latest write.
    pub busy: [bool; 5],
    pub again: [bool; 5],
    pub last_write: [Option<u64>; 5],
    /// The tick the hook first saw it, kept through a refresh (the stack's
    /// answers count from here).
    pub first: Option<u64>,
}
impl Request {
    fn new(serial: u64, target: usize) -> Self {
        Self {
            serial,
            target,
            seen: None,
            sent: None,
            done: [false; 5],
            dropped: [false; 5],
            ended: false,
            acted: [false; 5],
            busy: [false; 5],
            again: [false; 5],
            last_write: [None; 5],
            first: None,
        }
    }
    /// A repeat of the same ping within its life (1.7.1-dev15): the life
    /// starts again, and a busy teammate may be asked once more when free.
    /// Nobody who answered anything else is asked again.
    pub(crate) fn refresh(&mut self) {
        self.seen = None;
        self.ended = false;
        for i in 0..5 {
            if self.busy[i] {
                self.again[i] = false;
            }
        }
    }
    /// What to do for the step of brain (side, index) at `tick`; `battle`
    /// is the target of his big plan when it is Battle.
    ///
    /// - Not asked yet: on it when his Battle is on the pinged enemy
    ///   (nothing written, accepted), else written.
    /// - Asked and answered busy: written once more after he left his
    ///   fight, while the call lives.
    /// - Never more than one write per teammate per [`WRITE_EVERY`].
    pub(crate) fn decide(
        &mut self,
        map: &Mapping,
        side: u8,
        index: u8,
        tick: u64,
        battle: Option<u64>,
    ) -> Decision {
        if self.ended {
            return Decision::Nothing;
        }
        let seen = *self.seen.get_or_insert(tick);
        self.first.get_or_insert(tick);
        if tick > seen + REQUEST_TICKS || tick < seen {
            self.ended = true;
            return Decision::Expired;
        }
        let i = usize::from(index);
        if side != map.side || index == map.index || i >= 5 {
            return Decision::Nothing;
        }
        let throttled = self.last_write[i].is_some_and(|t| tick < t + WRITE_EVERY && tick >= t);
        if !self.done[i] {
            if battle == Some(self.target as u64) {
                self.done[i] = true;
                return Decision::OnIt;
            }
            return if throttled {
                Decision::Throttled
            } else {
                Decision::Write
            };
        }
        if self.busy[i] && !self.again[i] && battle.is_none() {
            if throttled {
                return Decision::Throttled;
            }
            self.again[i] = true;
            self.acted[i] = false;
            return Decision::Write;
        }
        Decision::Nothing
    }
    /// The entry for brain `index` was written at `tick`. The call lives
    /// on for its pin (busy teammates may be asked again).
    pub(crate) fn written(&mut self, index: u8, tick: u64) {
        let i = usize::from(index);
        if i < 5 {
            self.done[i] = true;
            self.last_write[i] = Some(tick);
        }
        self.sent.get_or_insert(tick);
    }
    /// True the first time the mod acts for teammate `index`, which must
    /// have been written to.
    pub(crate) fn act(&mut self, index: u8) -> bool {
        let i = usize::from(index);
        i < 5 && self.done[i] && !std::mem::replace(&mut self.acted[i], true)
    }
    /// True the first time brain `index`'s queue was full for this call.
    pub(crate) fn first_drop(&mut self, index: u8) -> bool {
        !std::mem::replace(&mut self.dropped[usize::from(index)], true)
    }
    /// The teammates written to, for the log.
    pub(crate) fn done_text(&self) -> String {
        let to: Vec<String> = (0..5)
            .filter(|i| self.done[*i])
            .map(|i| i.to_string())
            .collect();
        format!("[{}]", to.join(","))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Decision {
    Nothing,
    Write,
    /// Already fighting the pinged enemy: nothing written, accepted.
    OnIt,
    /// Written less than a second ago.
    Throttled,
    Expired,
}

/// A teammate's answer to the mod's call (1.7.1-dev8): his champion's
/// entity, the answer and the world tick it came at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Reply {
    pub entity: usize,
    pub answer: super::signal_fate::Answer,
    pub tick: u64,
}
impl Reply {
    /// Within [`REPLY_TICKS`] of `now` (the tests' view of what is kept).
    #[cfg(test)]
    pub(crate) fn shown_at(&self, now: u64) -> bool {
        now >= self.tick && now <= self.tick + REPLY_TICKS
    }
}

/// Everything the three threads share.
#[derive(Default)]
pub(crate) struct Shared {
    pub request: Option<Request>,
    pub serial: u64,
    pub roster: Vec<RosterPlayer>,
    pub owners: Owners,
    /// The request already told "unresolved: nothing sent".
    pub unresolved_serial: u64,
    /// The latest answers, newest last.
    pub replies: Vec<Reply>,
    /// The ping system's orders (1.7.1-dev13).
    pub pings: super::ping_orders::Orders,
}
pub(crate) static SHARED: Mutex<Option<Shared>> = Mutex::new(None);
/// The latest tick the hook saw (for the client's request line).
pub(crate) static TICK: AtomicU64 = AtomicU64::new(0);

pub(crate) fn with<R>(f: impl FnOnce(&mut Shared) -> R) -> Option<R> {
    let mut slot = SHARED.lock().ok()?;
    Some(f(slot.get_or_insert_with(Shared::default)))
}

/// Client: leave a BattleHelp request on `target` (replacing any older
/// one). Returns the tick for the log line.
pub fn request(target: usize) -> Option<(u64, u64)> {
    with(|s| {
        // A repeat of the live call on the same enemy refreshes it.
        if let Some(r) = s
            .request
            .as_mut()
            .filter(|r| r.target == target && !r.ended)
        {
            r.refresh();
            return (r.serial, TICK.load(Ordering::Relaxed));
        }
        s.serial += 1;
        s.request = Some(Request::new(s.serial, target));
        let tick = TICK.load(Ordering::Relaxed);
        (s.serial, tick)
    })
}
/// Simulation step of the controlled athlete: the match's roster. True when
/// it changed (the caller logs it).
pub fn publish_roster(roster: Vec<RosterPlayer>) -> bool {
    with(|s| {
        if s.roster == roster {
            return false;
        }
        s.roster = roster;
        true
    })
    .unwrap_or(false)
}
/// A new match: forget the owners, the mapping, the roster, the request,
/// the answers and the ping orders.
pub(crate) fn new_match() {
    let _ = with(|s| {
        s.owners = Owners::default();
        s.roster.clear();
        s.request = None;
        s.replies.clear();
        s.pings = Default::default();
    });
    crate::pings::places::forget_clock();
}
/// A brain's step: its owner's entity id (lines for the log).
pub(crate) fn step_owner(side: u8, index: u8, entity: usize) -> Vec<String> {
    with(|s| {
        let controlled = s.roster.iter().find(|p| p.controlled).copied();
        let mut lines = s.owners.step(side, index, entity, controlled.as_ref());
        if std::mem::take(&mut s.owners.swapped) {
            // A control swap: every order ends, as at a new match, the
            // windows putting back what they wrote at each teammate's next
            // think; the walks hand back.
            s.request = None;
            s.pings.drop_orders();
            s.pings.close(None);
            lines.extend(crate::pings::walk::end_all_lines(
                crate::pings::walk::End::Replaced,
            ));
            lines.push("PING map swap: orders, windows and walks ended".to_owned());
        }
        lines
    })
    .unwrap_or_default()
}
/// For the watch: (serial, target, first write tick) while it lasts.
#[cfg_attr(not(feature = "research"), allow(dead_code))]
pub fn watch(tick: u64) -> Option<(u64, usize, u64)> {
    with(|s| {
        let r = s.request?;
        let sent = r.sent?;
        (tick >= sent && tick <= sent + WATCH_TICKS).then_some((r.serial, r.target, sent))
    })
    .flatten()
}

/// The delivery probe (1.7.1-dev6): the mod's call while it is open (from
/// its first write to the end of the watch), with the controlled athlete's
/// side and index from the mapping and the mark its entries carry.
pub(crate) fn open_call(tick: u64, mark: u32) -> Option<super::signal_fate::Ours> {
    with(|s| open(s, tick, mark)).flatten()
}
fn open(s: &Shared, tick: u64, mark: u32) -> Option<super::signal_fate::Ours> {
    let r = s.request?;
    let sent = r.sent?;
    let map = s.owners.mapping?;
    (tick >= sent && tick <= sent + WATCH_TICKS).then_some(super::signal_fate::Ours {
        side: map.side,
        index: map.index,
        target: r.target,
        mark,
    })
}
/// The signal as the hooks see it: who sent it, its kind, target and word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Heard {
    pub sender: u32,
    pub tag: u8,
    pub target: u64,
    pub word: u32,
}
/// The mod's own scene (1.7.1-dev7): for the mod's own call (1.7.1-dev8:
/// carrying its mark), to a teammate the mod wrote it to, not yet acted
/// on for this request. Marks it acted on and returns the request's
/// serial.
pub(crate) fn act_on(tick: u64, to: (u8, u8), heard: Heard, mark: u32) -> Option<u64> {
    with(|s| {
        let ours = open(s, tick, mark)?;
        let mine = ours.mine(Some(to), heard.sender, heard.tag, heard.target, heard.word);
        if mine != super::signal_fate::Mine::Yes {
            return None;
        }
        let r = s.request.as_mut()?;
        r.act(to.1).then_some(r.serial)
    })
    .flatten()
}
/// The game answered the mod's call to brain `to` at `tick` (1.7.1-dev8).
/// Kept for the HUD by the teammate's champion entity, when known.
/// Returns its log line (1.7.1-dev9): stored, or dropped when no entity is
/// known for that brain.
pub(crate) fn answered(to: (u8, u8), answer: super::signal_fate::Answer, tick: u64) -> String {
    with(|s| s.answered(to, answer, tick))
        .unwrap_or_else(|| "PING reply dropped: no lock".to_owned())
}
impl Shared {
    fn answered(&mut self, to: (u8, u8), answer: super::signal_fate::Answer, tick: u64) -> String {
        let Some(entity) = self.owners.entity_of(to.0, to.1) else {
            return format!(
                "PING reply dropped: no entity for team={} index={}",
                to.0, to.1
            );
        };
        // Busy: fighting someone else; asked again once free.
        if let Some(r) = self
            .request
            .as_mut()
            .filter(|_| Some(to.0) == self.owners.mapping.map(|m| m.side))
        {
            if let Some(b) = r.busy.get_mut(usize::from(to.1)) {
                *b = answer == super::signal_fate::Answer::InBattle;
            }
        }
        self.replies
            .retain(|r| r.entity != entity && tick <= r.tick + REPLY_TICKS);
        self.replies.push(Reply {
            entity,
            answer,
            tick,
        });
        format!(
            "PING reply stored entity={entity} answer={} tick={tick}",
            answer.text()
        )
    }
    #[cfg(test)]
    fn replies(&self, now: u64) -> Vec<(usize, super::signal_fate::Answer)> {
        self.replies
            .iter()
            .filter(|r| r.shown_at(now))
            .map(|r| (r.entity, r.answer))
            .collect()
    }
}

/// The fight call `serial`'s answers by teammate index, for the answer
/// stack (1.7.1-dev15): each teammate's latest answer since the call was
/// first written; None once a newer call replaced it.
pub(crate) fn fight_answers(serial: u64) -> Option<[Option<super::ping_orders::Status>; 5]> {
    with(|s| {
        let r = s.request.filter(|r| r.serial == serial)?;
        let sent = r.first?;
        let map = s.owners.mapping?;
        let side = map.side;
        // Every teammate has a row (1.7.1-dev16): waiting while the call
        // lives, unsent if it ended without reaching him, then his answer.
        let mut out: [Option<super::ping_orders::Status>; 5] = std::array::from_fn(|i| {
            use super::ping_orders::Status;
            (i != usize::from(map.index)).then_some(if r.ended && !r.done[i] {
                Status::Unsent
            } else {
                Status::Pending
            })
        });
        for reply in s.replies.iter().filter(|p| p.tick >= sent) {
            if let Some((sd, i)) = s.owners.brain_of(reply.entity) {
                if sd == side && usize::from(i) < 5 {
                    if let Some(st) = fight_status(reply.answer) {
                        out[usize::from(i)] = Some(st);
                    }
                }
            }
        }
        Some(out)
    })
    .flatten()
}
/// A fight-call answer on the stack: joined is accepted, still fighting is
/// busy, every refusal declined; no answer, no row.
pub(crate) fn fight_status(a: super::signal_fate::Answer) -> Option<super::ping_orders::Status> {
    use super::ping_orders::Status;
    use super::signal_fate::Answer;
    match a {
        Answer::Join => Some(Status::Accepted),
        Answer::InBattle => Some(Status::Busy),
        Answer::TooFar | Answer::Ignored | Answer::NoHelp | Answer::LowHp | Answer::Bail => {
            Some(Status::Declined)
        }
        Answer::None => None,
    }
}
/// The champion entity of the controlled athlete's teammate `index`, and
/// the SDK player id the roster gives that entity (the stack's portraits).
pub fn teammate(index: u8) -> Option<(usize, usize)> {
    with(|s| {
        let side = s.owners.mapping?.side;
        let entity = s.owners.entity_of(side, index)?;
        let player = s.roster.iter().find(|p| p.entity == entity)?.player;
        Some((entity, player))
    })
    .flatten()
}
/// The SDK player id of the champion `entity`, from the roster.
pub fn player_of(entity: usize) -> Option<usize> {
    with(|s| {
        s.roster
            .iter()
            .find(|p| p.entity == entity)
            .map(|p| p.player)
    })
    .flatten()
}
/// The controlled athlete's brain (side, index), once resolved.
pub fn controlled_brain() -> Option<(u8, u8)> {
    with(|s| s.owners.mapping.map(|m| (m.side, m.index))).flatten()
}
/// The (side, index) of the brain whose champion is `entity` (the watch's
/// labels).
pub fn brain_of(entity: usize) -> Option<(u8, u8)> {
    with(|s| s.owners.brain_of(entity)).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roster() -> Vec<RosterPlayer> {
        (0..10)
            .map(|i| RosterPlayer {
                player: i,
                side: i / 5,
                lane: None,
                entity: 18 + i,
                controlled: i == 7,
            })
            .collect()
    }
    #[test]
    fn the_entry_has_the_games_layout_and_the_mark() {
        // Superseded 2026-11-03 (dev8): until dev7 the four bytes at +0x14
        // were 0.
        let e = entry(1453, 3, BATTLE_HELP, 0, 21, 0x4c54_5047);
        assert_eq!(
            e.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            "ad05000000000000030000000f000000040000004750544c15000000000000000000000000000000"
        );
        // 1.7.1-dev10: a HideLine on lane 2, no target.
        let e = entry(1453, 3, 6, 2, 0, 0x4c54_5047);
        assert_eq!(e[0x10..0x18], [6, 2, 0, 0, 0x47, 0x50, 0x54, 0x4c]);
    }

    #[test]
    fn answers_show_for_a_moment_one_per_teammate() {
        use super::super::signal_fate::Answer;
        let mut s = Shared::default();
        for (index, entity) in [(0, 18), (1, 19)] {
            s.owners.step(0, index, entity, None);
        }
        assert_eq!(
            s.answered((0, 0), Answer::Join, 100),
            "PING reply stored entity=18 answer=join tick=100"
        );
        s.answered((0, 1), Answer::TooFar, 110);
        assert_eq!(
            s.answered((0, 4), Answer::Join, 110),
            "PING reply dropped: no entity for team=0 index=4"
        );
        assert_eq!(
            s.replies(150),
            [(18, Answer::Join), (19, Answer::TooFar)],
            "no entity known for index 4"
        );
        // A later answer replaces the teammate's earlier one.
        s.answered((0, 0), Answer::InBattle, 160);
        assert_eq!(
            s.replies(285),
            [(19, Answer::TooFar), (18, Answer::InBattle)]
        );
        assert_eq!(s.replies(291), [(18, Answer::InBattle)], "3 s");
        assert!(s.replies(99).is_empty());
    }

    #[test]
    fn his_brain_is_the_one_showing_his_entity_and_stays_for_the_match() {
        let r = roster();
        let mut o = Owners::default();
        // Before the roster names him: owners only.
        assert_eq!(
            o.step(1, 0, 23, None),
            ["PING map owner team=1 index=0 entity=23"]
        );
        assert_eq!(o.mapping, None);
        assert!(o.step(1, 0, 23, Some(&r[7])).is_empty(), "nothing new");
        let lines = o.step(1, 2, 25, Some(&r[7]));
        assert_eq!(lines[0], "PING map owner team=1 index=2 entity=25");
        assert!(lines[1].starts_with(
            "PING map resolved team=1 index=2 entity=25 by=player+0x8 sdk_side=1 lane_agrees="
        ));
        assert_eq!(
            o.mapping,
            Some(Mapping {
                side: 1,
                index: 2,
                entity: 25
            })
        );
        // He respawns as 193 before the roster says so: said once, kept.
        let lines = o.step(1, 2, 193, Some(&r[7]));
        assert_eq!(lines.len(), 2);
        assert!(
            lines[1].starts_with("PING map disagrees: his brain team=1 index=2 shows entity=193")
        );
        assert!(o.step(1, 2, 193, Some(&r[7])).is_empty(), "said once");
        assert_eq!(o.mapping.map(|m| m.index), Some(2), "kept");
        // The roster catches up: agreement again, nothing to say.
        let mut later = r[7];
        later.entity = 193;
        assert!(o.step(1, 2, 193, Some(&later)).is_empty());
        assert_eq!(o.brain_of(193), Some((1, 2)));
        assert_eq!(o.brain_of(25), None);
        // Another brain showing his id is said, never followed.
        assert!(o.step(1, 3, 193, Some(&later))[1].contains("also shows the controlled entity=193"));
        assert_eq!(o.mapping.map(|m| m.index), Some(2));
        // Out of range: ignored.
        assert!(o.step(2, 0, 25, Some(&r[7])).is_empty());
    }

    #[test]
    fn a_control_swap_moves_the_mapping_to_the_new_brain() {
        let r = roster();
        let mut o = Owners::default();
        o.step(0, 2, 20, None);
        o.step(0, 3, 21, None);
        let mid = RosterPlayer { entity: 20, ..r[0] };
        o.step(0, 2, 20, Some(&mid));
        assert_eq!(o.mapping.map(|m| m.index), Some(2));
        // The roster now names bottom's entity: his brain's step moves it.
        let bottom = RosterPlayer { entity: 21, ..mid };
        assert!(o.step(0, 2, 20, Some(&bottom)).is_empty(), "not his entity");
        assert_eq!(
            o.mapping.map(|m| m.index),
            Some(2),
            "brain 3 has not stepped"
        );
        let lines = o.step(0, 3, 21, Some(&bottom));
        assert!(lines[0]
            .starts_with("PING map re-resolved team=0 index=3 entity=21 was team=0 index=2"));
        assert_eq!(o.mapping.map(|m| (m.index, m.entity)), Some((3, 21)));
        assert!(std::mem::take(&mut o.swapped));
        // Team::me reads this index (`controlled_brain`): mid (2) hears
        // the orders again, bottom (3) is the player.
        let me = o.mapping.map(|m| m.index);
        let hears: Vec<u8> = (0..5).filter(|i| Some(*i) != me).collect();
        assert_eq!(hears, [0, 1, 2, 4]);
        // An entity no brain owns yet: cleared, then resolved as a swap.
        let ghost = RosterPlayer { entity: 77, ..mid };
        assert!(o.step(0, 3, 21, Some(&ghost))[0].starts_with("PING map cleared"));
        assert_eq!(o.mapping, None);
        o.step(0, 4, 77, Some(&ghost));
        assert_eq!(o.mapping.map(|m| m.index), Some(4));
        assert!(o.swapped, "index 3 to 4");
    }

    #[test]
    fn the_mod_acts_once_per_teammate_it_wrote_to() {
        let mut r = Request::new(1, 21);
        assert!(!r.act(0), "not written to");
        r.done[0] = true;
        assert!(r.act(0));
        assert!(!r.act(0), "once");
        assert!(!r.act(9), "out of range");
    }

    #[test]
    fn a_request_goes_to_each_teammate_once_and_ends() {
        let map = Mapping {
            side: 1,
            index: 2,
            entity: 25,
        };
        let mut r = Request::new(1, 21);
        assert_eq!(
            r.decide(&map, 0, 0, 100, None),
            Decision::Nothing,
            "the enemy team"
        );
        assert_eq!(
            r.decide(&map, 1, 2, 100, None),
            Decision::Nothing,
            "the sender"
        );
        assert_eq!(r.decide(&map, 1, 0, 101, None), Decision::Write);
        r.written(0, 101);
        assert_eq!(
            r.decide(&map, 1, 0, 102, None),
            Decision::Nothing,
            "only once"
        );
        assert!(r.first_drop(1));
        assert!(!r.first_drop(1));
        for i in [1, 3, 4] {
            r.written(i, 110);
        }
        assert!(!r.ended, "lives for its pin");
        assert_eq!(r.sent, Some(101));
        assert_eq!(r.done_text(), "[0,1,3,4]");
    }

    #[test]
    fn on_it_busy_then_free_and_the_throttle() {
        let map = Mapping {
            side: 0,
            index: 2,
            entity: 25,
        };
        let mut r = Request::new(1, 21);
        // Already fighting the pinged enemy: on it, nothing written.
        assert_eq!(r.decide(&map, 0, 0, 100, Some(21)), Decision::OnIt);
        assert_eq!(r.decide(&map, 0, 0, 101, None), Decision::Nothing);
        // Fighting someone else: written, answers busy.
        assert_eq!(r.decide(&map, 0, 1, 100, Some(30)), Decision::Write);
        r.written(1, 100);
        r.busy[1] = true;
        assert_eq!(
            r.decide(&map, 0, 1, 120, Some(30)),
            Decision::Nothing,
            "still fighting"
        );
        // Free within the second: throttled, then asked once more.
        assert_eq!(r.decide(&map, 0, 1, 130, None), Decision::Throttled);
        assert_eq!(r.decide(&map, 0, 1, 160, None), Decision::Write);
        r.written(1, 160);
        assert_eq!(r.decide(&map, 0, 1, 230, None), Decision::Nothing, "once");
        // A repeat lets a still-busy teammate be asked once more.
        r.refresh();
        assert_eq!(r.decide(&map, 0, 1, 300, None), Decision::Write);
        // A teammate who declined is not asked again.
        r.written(3, 100);
        assert_eq!(r.decide(&map, 0, 3, 300, None), Decision::Nothing);
    }

    #[test]
    fn an_undelivered_request_expires() {
        let map = Mapping {
            side: 0,
            index: 0,
            entity: 18,
        };
        let mut r = Request::new(1, 21);
        assert_eq!(r.decide(&map, 0, 1, 1000, None), Decision::Write);
        assert_eq!(
            r.decide(&map, 0, 1, 1000 + REQUEST_TICKS + 1, None),
            Decision::Expired
        );
        assert_eq!(
            r.decide(&map, 0, 1, 1000 + REQUEST_TICKS + 2, None),
            Decision::Nothing
        );
    }
}
