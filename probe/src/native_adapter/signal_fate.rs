//! Research builds only (1.7.1-dev6): the pure half of the delivery probe
//! (the hooks are `windows/signal_fate.rs`). It follows each call in an
//! AI brain's inbox to its fate, at the game's own delivery:
//!
//! - `late`: older than its due tick plus the receiver's delay, dropped
//!   without being heard;
//! - `gate`: offered to the receive gate, with the receiver's mode and
//!   whether the gate's rule for the sender's slot accepts that mode;
//! - `heard`: past the gate, into the receive handler;
//! - `scene`: a Battle, BattleHelp or BattleDive call that opened a scene
//!   (the AI environment's kind was not 0; 1.7.1-dev7: or the mod opened
//!   it for its own call, `by=mod`);
//! - `join`: a brain's call-join counters, when they change (they also
//!   count scenes opened without a call, which the `scene` line misses).
//!
//! `ours=yes` marks a line about a call that matches the mod's own: the
//! controlled athlete as sender, BattleHelp, the open request's target, on
//! his side, and (1.7.1-dev8) the mod's mark in the signal's word at +4.
//! `ours=unmarked` is the same call without the mark: the controlled
//! athlete's own AI planner sending it (dev7 counted those as the
//! player's). Superseded 2026-11-03: until dev7 nothing in the entry told
//! the two apart.
//!
//! Since 1.7.1-dev8, around the mod's own scene call: the brain's scene
//! state before and after ([`scene_state_line`]) and the game's answer from
//! the call-join counters ([`answer`], [`answer_line`]).
use super::signal_probe::name;
use std::fmt::Display;

/// The AI environment's kind as the receive handler asks it (method slot
/// 0x40 of the environment at the start of the simulation; 1.7.1-dev7). In
/// kind 0 the handler opens no scene for Battle, BattleHelp or BattleDive.
/// A version's `SIGNAL_ENV_VTABLES` lists (vtable RVA, kind), this value
/// marking the wrapper type that forwards to an inner environment.
pub(crate) const ENV_WRAPPER: u8 = u8::MAX;
/// The kind of the environment whose vtable is at `rva` (or
/// [`ENV_WRAPPER`]); None for a vtable not in the table.
pub(crate) fn env_kind(table: &[(usize, u8)], rva: usize) -> Option<u8> {
    table.iter().find(|(v, _)| *v == rva).map(|(_, k)| *k)
}
fn kind_text(kind: Option<u8>) -> String {
    match kind {
        Some(ENV_WRAPPER) => "wrapper".to_owned(),
        Some(k) => k.to_string(),
        None => "unknown".to_owned(),
    }
}
/// Once a match: the environment's vtable and kind; for the wrapper, the
/// inner one's too.
pub(crate) fn env_line(
    vtable: usize,
    kind: Option<u8>,
    inner: Option<(usize, Option<u8>)>,
) -> String {
    let mut line = format!("SIGNAL env vtable={vtable:#x} kind={}", kind_text(kind));
    if let Some((rva, k)) = inner {
        line.push_str(&format!(" inner={rva:#x} inner_kind={}", kind_text(k)));
    }
    line
}

/// The signal the delivery passes on (a copy of the inbox entry from +0x10
/// on): tag @0, lane @1, a dword @4 (Mia's enemy; the mod's mark on its
/// own BattleHelp), the target entity id @8, a word @0x10.
pub(crate) const SIGNAL_BYTES: usize = 24;
/// The call-join counters in the game's own order and names.
pub(crate) const JOIN_NAMES: [&str; 8] = [
    "recv",
    "ignored",
    "in_battle",
    "no_help",
    "too_far",
    "low_hp",
    "bail",
    "join",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Signal {
    pub tag: u8,
    pub lane: u8,
    pub word: u32,
    pub target: u64,
}
impl Signal {
    pub(crate) fn read(raw: &[u8; SIGNAL_BYTES]) -> Self {
        let mut target = [0; 8];
        target.copy_from_slice(&raw[8..16]);
        let mut word = [0; 4];
        word.copy_from_slice(&raw[4..8]);
        Self {
            tag: raw[0],
            lane: raw[1],
            word: u32::from_le_bytes(word),
            target: u64::from_le_bytes(target),
        }
    }
}

/// Whether the gate accepts a call from sender slot `sender` while the
/// receiver is in `mode`, by this version's rules ((mask over modes 0..8,
/// accepts a mode above 8) per slot). None for a slot the game's table
/// does not cover: the gate does not check it, so nothing is guessed.
pub(crate) fn mask_ok(rules: &[(u16, bool); 5], sender: u32, mode: u8) -> Option<bool> {
    let (mask, above) = *rules.get(usize::try_from(sender).ok()?)?;
    Some(if mode > 8 {
        above
    } else {
        mask >> mode & 1 == 1
    })
}

/// The mod's own call while it is open: the controlled athlete's side and
/// index, the target and the mark the mod writes (this version's
/// `SIGNAL_MARK`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Ours {
    pub side: u8,
    pub index: u8,
    pub target: usize,
    pub mark: u32,
}
/// Whose a call is, as a line says it (`ours=`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mine {
    No,
    /// The mod's own: it matches and carries the mark.
    Yes,
    /// It matches but carries no mark: the athlete's own AI planner.
    Unmarked,
    /// Not known where the line is written (no signal passed there).
    Unknown,
}
impl Mine {
    /// Lines about a call like the mod's are written past the budget.
    pub(crate) fn forced(self) -> bool {
        matches!(self, Mine::Yes | Mine::Unmarked)
    }
    fn text(self) -> &'static str {
        match self {
            Mine::No => "no",
            Mine::Yes => "yes",
            Mine::Unmarked => "unmarked",
            Mine::Unknown => "?",
        }
    }
}
impl Ours {
    /// A call like it: his index as sender, BattleHelp, the same target, to
    /// his side (when the receiver's side is known); the mod's own when
    /// its word at +4 is the mark.
    pub(crate) fn mine(
        &self,
        to: Option<(u8, u8)>,
        sender: u32,
        tag: u8,
        target: u64,
        word: u32,
    ) -> Mine {
        let like = sender == u32::from(self.index)
            && tag == super::signal_call::BATTLE_HELP
            && target == self.target as u64
            && to.is_none_or(|(side, _)| side == self.side);
        match (like, word == self.mark) {
            (false, _) => Mine::No,
            (true, true) => Mine::Yes,
            (true, false) => Mine::Unmarked,
        }
    }
}

fn shown<T: Display>(value: Option<T>) -> String {
    value.map_or("?".to_owned(), |v| v.to_string())
}
fn yes(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}
/// `team=<side> to=<index>`, `?` where the receiver is not known.
fn receiver(to: Option<(u8, u8)>) -> String {
    format!(
        "team={} to={}",
        shown(to.map(|t| t.0)),
        shown(to.map(|t| t.1))
    )
}
fn call(sender: u32, s: &Signal) -> String {
    format!(
        "from={sender} tag={} name={} target={}",
        s.tag,
        name(s.tag),
        s.target
    )
}

/// `now` and `delay` here are what the step hook last published for this
/// brain: the late drop is not passed either.
pub(crate) fn late_line(
    to: Option<(u8, u8)>,
    sender: u32,
    s: &Signal,
    due: u64,
    now: Option<u64>,
    delay: Option<u64>,
    ours: Mine,
) -> String {
    format!(
        "SIGNAL late {} {} due={due} now={} delay={} ours={}",
        receiver(to),
        call(sender, s),
        shown(now),
        shown(delay),
        ours.text()
    )
}
/// The gate is not passed the due tick, so the line has none.
#[allow(clippy::too_many_arguments)]
pub(crate) fn gate_line(
    to: Option<(u8, u8)>,
    sender: u32,
    s: &Signal,
    now: Option<u64>,
    mode: Option<u8>,
    delay: Option<u64>,
    accepted: Option<bool>,
    ours: Mine,
) -> String {
    format!(
        "SIGNAL gate {} {} now={} mode={} delay={} mask_ok={} ours={}",
        receiver(to),
        call(sender, s),
        shown(now),
        shown(mode),
        shown(delay),
        accepted.map_or("?", yes),
        ours.text()
    )
}
pub(crate) fn heard_line(
    to: Option<(u8, u8)>,
    sender: u32,
    s: &Signal,
    now: Option<u64>,
    mode: Option<u8>,
    site: u8,
    ours: Mine,
) -> String {
    format!(
        "SIGNAL heard {} {} now={} mode={} site={site} ours={}",
        receiver(to),
        call(sender, s),
        shown(now),
        shown(mode),
        ours.text()
    )
}
/// The scene opener's reason for a call: 1 Battle, 2 BattleHelp, 3
/// BattleDive (the receive handler's `mov cl, N`). Superseded 2026-11-03:
/// dev6 labelled 3 as Battle.
pub(crate) const REASON_BATTLE_HELP: u8 = 2;
/// `by_mod`: the mod opened it for its own call (1.7.1-dev7); the hooked
/// site does not see that call. `ours` comes from the receive handler's
/// call it is nested in (the scene opener is not passed the signal).
pub(crate) fn scene_line(
    to: Option<(u8, u8)>,
    sender: u32,
    reason: u8,
    target: u64,
    now: Option<u64>,
    ours: Mine,
    by_mod: bool,
) -> String {
    let kind = match reason {
        1 => "Battle",
        2 => "BattleHelp",
        3 => "BattleDive",
        _ => "?",
    };
    format!(
        "SIGNAL scene {} from={sender} reason={reason}[{kind}] target={target} now={} ours={}{}",
        receiver(to),
        shown(now),
        ours.text(),
        if by_mod { " by=mod" } else { "" }
    )
}
/// A brain's scene state, each field as read (None where it could not be).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SceneState {
    /// The current scene kind and its argument.
    pub kind: Option<u64>,
    pub arg: Option<u64>,
    /// The scene block: set (1), its world tick, the target's position,
    /// the distance and a byte.
    pub active: Option<u64>,
    pub tick: Option<u64>,
    pub pos: Option<(u64, u64)>,
    pub dist: Option<u64>,
    pub flag: Option<u8>,
}
impl SceneState {
    fn text(&self) -> String {
        format!(
            "kind={} arg={} active={} tick={} pos={} dist={} b88={}",
            shown(self.kind),
            shown(self.arg),
            shown(self.active),
            shown(self.tick),
            point(self.pos),
            shown(self.dist),
            shown(self.flag)
        )
    }
}
fn point(p: Option<(u64, u64)>) -> String {
    p.map_or("?".to_owned(), |(x, y)| format!("{x},{y}"))
}
/// Before and after the mod's scene call, one line.
pub(crate) fn scene_state_line(
    to: (u8, u8),
    request: u64,
    before: &SceneState,
    after: &SceneState,
) -> String {
    format!(
        "SIGNAL scene_state team={} to={} request={request} before{{{}}} after{{{}}}",
        to.0,
        to.1,
        before.text(),
        after.text()
    )
}

/// The game's answer to one call, from the call-join counters that rose
/// during it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Answer {
    Join,
    InBattle,
    TooFar,
    Ignored,
    NoHelp,
    LowHp,
    Bail,
    /// Only `recv` rose, or nothing (the target was not found).
    None,
}
impl Answer {
    pub(crate) fn text(self) -> &'static str {
        match self {
            Answer::Join => "join",
            Answer::InBattle => "in_battle",
            Answer::TooFar => "too_far",
            Answer::Ignored => "ignored",
            Answer::NoHelp => "no_help",
            Answer::LowHp => "low_hp",
            Answer::Bail => "bail",
            Answer::None => "none",
        }
    }
}
/// The answer from the counters before and after the call, and every
/// counter that changed (`recv+1,join+1`, or `none`). One call can raise
/// several: `in_battle` goes on to the later checks, and dev7 saw
/// `too_far` and `join` rise together. The answer is the one that ended
/// the call: join, then bail, low_hp, too_far, no_help, ignored, in_battle.
pub(crate) fn answer(before: &[u64; 8], after: &[u64; 8]) -> (Answer, String) {
    let rose = |i: usize| after[i] > before[i];
    // JOIN_NAMES order: recv, ignored, in_battle, no_help, too_far,
    // low_hp, bail, join.
    let order = [
        (7, Answer::Join),
        (6, Answer::Bail),
        (5, Answer::LowHp),
        (4, Answer::TooFar),
        (3, Answer::NoHelp),
        (1, Answer::Ignored),
        (2, Answer::InBattle),
    ];
    let answer = order
        .iter()
        .find(|(i, _)| rose(*i))
        .map_or(Answer::None, |(_, a)| *a);
    let delta: Vec<String> = JOIN_NAMES
        .iter()
        .enumerate()
        .filter(|(i, _)| after[*i] != before[*i])
        .map(|(i, n)| {
            let d = i128::from(after[i]) - i128::from(before[i]);
            format!("{n}{d:+}")
        })
        .collect();
    let delta = if delta.is_empty() {
        "none".to_owned()
    } else {
        delta.join(",")
    };
    (answer, delta)
}
/// What else the answer line carries: the Ok reply pushed onto list C (None
/// when the list could not be read, `Some(None)` when nothing was pushed),
/// and the teammate's hp percent, move speed and position, read before the
/// call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct AnswerFacts {
    pub reply: Option<Option<u8>>,
    pub hp: Option<u64>,
    pub speed: Option<u64>,
    pub pos: Option<(u64, u64)>,
}
pub(crate) fn answer_line(
    to: (u8, u8),
    request: u64,
    answer: Answer,
    delta: &str,
    facts: &AnswerFacts,
) -> String {
    let reply = match facts.reply {
        None => "?".to_owned(),
        Some(None) => "none".to_owned(),
        Some(Some(tag)) => tag.to_string(),
    };
    format!(
        "SIGNAL answer team={} to={} request={request} answer={} delta={delta} replyC={reply} hp={}% speed={} pos={}",
        to.0,
        to.1,
        answer.text(),
        shown(facts.hp),
        shown(facts.speed),
        point(facts.pos)
    )
}

pub(crate) fn join_line(side: u8, index: u8, counts: &[u64; 8]) -> String {
    let fields: Vec<String> = JOIN_NAMES
        .iter()
        .zip(counts)
        .map(|(n, c)| format!("{n}={c}"))
        .collect();
    format!("SIGNAL join team={side} index={index} {}", fields.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This version's rules as read from the 0.7.0 code (the table itself
    /// is in `v070/signals.rs` and checked by the verifier).
    const RULES: [(u16, bool); 5] = [
        (0x185, false),
        (0x141, true),
        (0x1b1, false),
        (0x1ab, false),
        (0x1ab, false),
    ];

    #[test]
    fn the_gate_accepts_by_the_senders_slot_and_the_receivers_mode() {
        let accepted = |sender: u32| -> Vec<u8> {
            (0..=10u8)
                .filter(|m| mask_ok(&RULES, sender, *m) == Some(true))
                .collect()
        };
        assert_eq!(accepted(0), [0, 2, 7, 8]);
        // Slot 1 rejects 1..7 except 6, and nothing above 8.
        assert_eq!(accepted(1), [0, 6, 8, 9, 10]);
        assert_eq!(accepted(2), [0, 4, 5, 7, 8]);
        assert_eq!(accepted(3), [0, 1, 3, 5, 7, 8]);
        assert_eq!(accepted(4), accepted(3));
        assert_eq!(mask_ok(&RULES, 5, 0), None);
        assert_eq!(mask_ok(&RULES, u32::MAX, 0), None);
    }

    #[test]
    fn a_signal_reads_tag_lane_and_target() {
        let mut raw = [0u8; SIGNAL_BYTES];
        raw[0] = 4;
        raw[1] = 2;
        raw[4..8].copy_from_slice(&7u32.to_le_bytes());
        raw[8..16].copy_from_slice(&25u64.to_le_bytes());
        raw[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            Signal::read(&raw),
            Signal {
                tag: 4,
                lane: 2,
                word: 7,
                target: 25
            }
        );
    }

    #[test]
    fn ours_needs_his_index_battle_help_the_target_his_side_and_the_mark() {
        const MARK: u32 = 0x4c54_5047;
        let ours = Ours {
            side: 1,
            index: 2,
            target: 25,
            mark: MARK,
        };
        assert_eq!(ours.mine(Some((1, 0)), 2, 4, 25, MARK), Mine::Yes);
        assert_eq!(ours.mine(None, 2, 4, 25, MARK), Mine::Yes);
        // The athlete's own planner: the same call without the mark.
        assert_eq!(ours.mine(Some((1, 0)), 2, 4, 25, 0), Mine::Unmarked);
        assert_eq!(ours.mine(Some((0, 0)), 2, 4, 25, MARK), Mine::No);
        assert_eq!(ours.mine(Some((1, 0)), 3, 4, 25, MARK), Mine::No);
        assert_eq!(ours.mine(Some((1, 0)), 2, 2, 25, MARK), Mine::No);
        assert_eq!(ours.mine(Some((1, 0)), 2, 4, 26, MARK), Mine::No);
        assert!(Mine::Yes.forced() && Mine::Unmarked.forced());
        assert!(!Mine::No.forced() && !Mine::Unknown.forced());
    }

    #[test]
    fn each_fate_line_names_receiver_call_and_what_is_unknown() {
        let s = Signal {
            tag: 4,
            lane: 0,
            word: 0,
            target: 25,
        };
        assert_eq!(
            late_line(Some((1, 3)), 2, &s, 115, Some(140), Some(1), Mine::Yes),
            "SIGNAL late team=1 to=3 from=2 tag=4 name=BattleHelp target=25 due=115 now=140 delay=1 ours=yes"
        );
        assert_eq!(
            late_line(None, 2, &s, 115, None, None, Mine::No),
            "SIGNAL late team=? to=? from=2 tag=4 name=BattleHelp target=25 due=115 now=? delay=? ours=no"
        );
        assert_eq!(
            gate_line(Some((1, 3)), 2, &s, Some(116), Some(6), Some(0), Some(false), Mine::Yes),
            "SIGNAL gate team=1 to=3 from=2 tag=4 name=BattleHelp target=25 now=116 mode=6 delay=0 mask_ok=no ours=yes"
        );
        assert!(
            gate_line(None, 7, &s, None, None, None, None, Mine::No).ends_with("mask_ok=? ours=no")
        );
        assert_eq!(
            heard_line(Some((0, 4)), 1, &s, Some(116), Some(0), 2, Mine::Unmarked),
            "SIGNAL heard team=0 to=4 from=1 tag=4 name=BattleHelp target=25 now=116 mode=0 site=2 ours=unmarked"
        );
        assert_eq!(
            scene_line(Some((0, 4)), 1, 2, 25, Some(116), Mine::No, false),
            "SIGNAL scene team=0 to=4 from=1 reason=2[BattleHelp] target=25 now=116 ours=no"
        );
        assert!(scene_line(None, 1, 1, 25, None, Mine::Unknown, false).contains("reason=1[Battle]"));
        assert!(scene_line(None, 1, 3, 25, None, Mine::No, false).contains("reason=3[BattleDive]"));
        assert!(scene_line(Some((1, 0)), 2, 2, 25, Some(9), Mine::Yes, true)
            .ends_with("ours=yes by=mod"));
        assert_eq!(
            join_line(1, 0, &[1, 2, 3, 4, 5, 6, 7, 8]),
            "SIGNAL join team=1 index=0 recv=1 ignored=2 in_battle=3 no_help=4 too_far=5 low_hp=6 bail=7 join=8"
        );
    }

    #[test]
    fn the_answer_is_the_counter_that_ended_the_call() {
        let before = [10, 1, 2, 3, 4, 5, 6, 7];
        let after = |rises: &[usize]| {
            let mut a = before;
            for i in rises {
                a[*i] += 1;
            }
            a
        };
        assert_eq!(
            answer(&before, &after(&[0, 7])),
            (Answer::Join, "recv+1,join+1".to_owned())
        );
        // In battle goes on to the later checks; the later one answers.
        assert_eq!(answer(&before, &after(&[0, 2, 7])).0, Answer::Join);
        assert_eq!(answer(&before, &after(&[0, 2])).0, Answer::InBattle);
        assert_eq!(answer(&before, &after(&[0, 2, 4])).0, Answer::TooFar);
        // dev7 saw too_far and join rise together.
        assert_eq!(answer(&before, &after(&[0, 4, 7])).0, Answer::Join);
        assert_eq!(answer(&before, &after(&[0, 1])).0, Answer::Ignored);
        assert_eq!(answer(&before, &after(&[0, 3])).0, Answer::NoHelp);
        assert_eq!(answer(&before, &after(&[0, 5])).0, Answer::LowHp);
        assert_eq!(answer(&before, &after(&[0, 6])).0, Answer::Bail);
        assert_eq!(
            answer(&before, &after(&[0])),
            (Answer::None, "recv+1".to_owned())
        );
        assert_eq!(answer(&before, &before), (Answer::None, "none".to_owned()));
    }

    #[test]
    fn the_scene_state_and_answer_lines_show_what_was_read() {
        let before = SceneState {
            kind: Some(5),
            arg: Some(0),
            active: Some(0),
            tick: Some(100),
            pos: Some((1, 2)),
            dist: Some(0),
            flag: Some(0),
        };
        let after = SceneState {
            active: Some(1),
            tick: Some(160),
            pos: Some((52000, 61000)),
            dist: Some(300400),
            ..before
        };
        assert_eq!(
            scene_state_line((0, 3), 7, &before, &after),
            "SIGNAL scene_state team=0 to=3 request=7 before{kind=5 arg=0 active=0 tick=100 pos=1,2 dist=0 b88=0} after{kind=5 arg=0 active=1 tick=160 pos=52000,61000 dist=300400 b88=0}"
        );
        assert!(scene_state_line((0, 3), 7, &SceneState::default(), &after)
            .contains("before{kind=? arg=? active=? tick=? pos=? dist=? b88=?}"));
        let facts = AnswerFacts {
            reply: Some(Some(9)),
            hp: Some(80),
            speed: Some(330),
            pos: Some((50000, 60000)),
        };
        assert_eq!(
            answer_line((0, 3), 7, Answer::Join, "recv+1,join+1", &facts),
            "SIGNAL answer team=0 to=3 request=7 answer=join delta=recv+1,join+1 replyC=9 hp=80% speed=330 pos=50000,60000"
        );
        let unread = answer_line((1, 0), 8, Answer::None, "none", &AnswerFacts::default());
        assert!(unread.ends_with("answer=none delta=none replyC=? hp=?% speed=? pos=?"));
        let none = AnswerFacts {
            reply: Some(None),
            ..facts
        };
        assert!(
            answer_line((1, 0), 8, Answer::TooFar, "recv+1,too_far+1", &none)
                .contains("replyC=none")
        );
    }

    #[test]
    fn the_environment_kind_comes_from_its_vtable() {
        let table = [(0x40398e0, ENV_WRAPPER), (0x40a2530, 0), (0x40a33a0, 1)];
        assert_eq!(env_kind(&table, 0x40a2530), Some(0));
        assert_eq!(env_kind(&table, 0x40398e0), Some(ENV_WRAPPER));
        assert_eq!(env_kind(&table, 0x1234), None);
        assert_eq!(
            env_line(0x40a33a0, Some(1), None),
            "SIGNAL env vtable=0x40a33a0 kind=1"
        );
        assert_eq!(
            env_line(0x40398e0, Some(ENV_WRAPPER), Some((0x40a2530, Some(0)))),
            "SIGNAL env vtable=0x40398e0 kind=wrapper inner=0x40a2530 inner_kind=0"
        );
        assert!(env_line(0x99, None, None).ends_with("kind=unknown"));
    }
}
