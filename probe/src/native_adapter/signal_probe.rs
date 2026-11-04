//! Research builds only: the pure half of the signal probe (the hook is
//! `windows/signals.rs`). The game's AI teammates call each other through
//! a per-team queue of signals; this module names the signals, writes the
//! `SIGNAL` log lines and remembers, per team, what was last logged, so a
//! queue is written only when it changed.
//!
//! The queue layout is read from the code and not yet seen in game, so
//! every entry is logged raw (all its bytes) beside the fields as read.

/// The AI's signal kinds by tag byte, read from the planner's own name
/// table (0.7.0).
pub(crate) const NAMES: [&str; 28] = [
    "Start",
    "Mia",
    "Battle",
    "BattleDive",
    "BattleHelp",
    "BattleStop",
    "HideLine",
    "HideLineToo",
    "DefenseLine",
    "Ok",
    "Reject",
    "Cancel",
    "Press",
    "PressChange",
    "Repair",
    "SerpenPrepare",
    "SerpenSetup",
    "SerpenHunt",
    "MorgardPrepare",
    "MorgardSetup",
    "MorgardHunt",
    "AttackNexus",
    "DefenseNexus",
    "GankDive",
    "PressTower",
    "ComebackPick",
    "SerpenDisband",
    "MorgardDisband",
];
/// Size of one entry of queue A and queue B, and of list C.
pub(crate) const QUEUE_ENTRY: usize = 40;
pub(crate) const LIST_ENTRY: usize = 24;
/// The largest capacity the probe reads; anything larger is treated as a
/// wrong layout.
pub(crate) const MAX_CAPACITY: usize = 64;
/// `SIGNAL` lines per match; then one `SIGNAL budget reached` line. Since
/// 1.7.1-dev5 all ten brains are logged (dev4: two, 350 to 1,045 lines a
/// match), so 8,000 (about 2 MB); a dump holding an entry the mod wrote is
/// written whatever the budget says.
pub(crate) const LINES_PER_MATCH: usize = 8000;
/// `SIGNAL` lines for the whole session (never reset), in case a match is
/// not recognised as new; then one `SIGNAL session limit reached` line.
pub(crate) const LINES_PER_SESSION: usize = 3 * LINES_PER_MATCH;
/// After the mod opened a scene for a brain, its join lines are written
/// whatever the budget for this long (10 s).
pub(crate) const MOD_SCENE_TICKS: u64 = 600;

/// A signal name, or `?<tag>` outside the 28 known kinds.
pub(crate) fn name(tag: u8) -> String {
    NAMES
        .get(usize::from(tag))
        .map_or_else(|| format!("?{tag}"), |n| (*n).to_owned())
}
pub(crate) fn hex(raw: &[u8]) -> String {
    raw.iter().map(|b| format!("{b:02x}")).collect()
}
fn u32_at(raw: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(raw[at..at + 4].try_into().unwrap_or_default())
}
fn u64_at(raw: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(raw[at..at + 8].try_into().unwrap_or_default())
}

/// One list as read: capacity, length and its entries' bytes.
pub(crate) struct Queue<'a> {
    pub capacity: usize,
    pub bytes: &'a [u8],
}
/// A list's header is plausible: length within capacity, capacity at most
/// `MAX_CAPACITY`, and a pointer whenever there is something to read.
pub(crate) fn in_bounds(capacity: usize, pointer: usize, len: usize) -> bool {
    len <= capacity
        && capacity <= MAX_CAPACITY
        && (len == 0 || (pointer != 0 && pointer.is_multiple_of(8)))
}

/// One queue A or B entry: the raw bytes, then every field as read. The
/// name is the tag at +0x10, then in brackets the byte at +0xc (the
/// broadcast writes its record from +0xc; which byte is the tag is open).
pub(crate) fn queue_line(queue: char, index: usize, raw: &[u8]) -> String {
    format!(
        "SIGNAL {queue}[{index}] raw={} due={} from={} tagC={} tag10={} name={}[{}] lane={} d14={} q18={} q20={}",
        hex(raw),
        u64_at(raw, 0),
        u32_at(raw, 8),
        raw[0xc],
        raw[0x10],
        name(raw[0x10]),
        name(raw[0xc]),
        raw[0x11],
        u32_at(raw, 0x14),
        u64_at(raw, 0x18),
        u64_at(raw, 0x20),
    )
}
/// One list C entry.
pub(crate) fn list_line(index: usize, raw: &[u8]) -> String {
    format!(
        "SIGNAL C[{index}] raw={} kind={} lane={} q8={}",
        hex(raw),
        raw[0],
        raw[1],
        u64_at(raw, 8)
    )
}
/// Every line of one brain's dump: a header and the entries of queue A,
/// then queue B and list C each with its own header, only when not empty
/// (1.7.1-dev5: they were empty in almost every dev4 dump). The header
/// shows the receiver's mode and delay (1.7.1-dev6; `?` when not
/// readable), which replace dev4's `delay?=` (read from the wrong
/// structure, superseded 2026-11-03).
pub(crate) fn dump(
    tick: u64,
    team: usize,
    step_player: u32,
    receiver: (Option<u8>, Option<u64>),
    lists: [&Queue; 3],
) -> Vec<String> {
    let [a, b, c] = lists;
    let (mode, delay) = receiver;
    let mode = mode.map_or("?".to_owned(), |m| m.to_string());
    let delay = delay.map_or("?".to_owned(), |d| d.to_string());
    let mut lines = vec![format!(
        "SIGNAL t={tick} team={team} step_player={step_player} mode={mode} delay={delay} A len={}/{}",
        a.bytes.len() / QUEUE_ENTRY,
        a.capacity
    )];
    lines.extend(
        a.bytes
            .as_chunks::<QUEUE_ENTRY>()
            .0
            .iter()
            .enumerate()
            .map(|(i, raw)| queue_line('A', i, raw)),
    );
    if !b.bytes.is_empty() {
        lines.push(format!(
            "SIGNAL t={tick} team={team} B len={}/{}",
            b.bytes.len() / QUEUE_ENTRY,
            b.capacity
        ));
    }
    lines.extend(
        b.bytes
            .as_chunks::<QUEUE_ENTRY>()
            .0
            .iter()
            .enumerate()
            .map(|(i, raw)| queue_line('B', i, raw)),
    );
    if !c.bytes.is_empty() {
        lines.push(format!(
            "SIGNAL t={tick} team={team} C len={}/{}",
            c.bytes.len() / LIST_ENTRY,
            c.capacity
        ));
    }
    lines.extend(
        c.bytes
            .as_chunks::<LIST_ENTRY>()
            .0
            .iter()
            .enumerate()
            .map(|(i, raw)| list_line(i, raw)),
    );
    lines
}
/// FNV-1a over the three lists, each with its length, so moving an entry
/// from one list to another still counts as a change.
pub(crate) fn digest(lists: [&Queue; 3]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for list in lists {
        for byte in (list.bytes.len() as u64)
            .to_le_bytes()
            .iter()
            .chain(list.bytes)
        {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    hash
}

/// What the probe remembers on the viewed match's worker thread, per AI
/// brain. Since 1.7.1-dev5 a brain is known by its player's side
/// (`+0x820`) and index (`+0x8b0`): the dev4 log showed that the hook's
/// first argument is one player's brain, not a team planner, so dev4
/// logged two of the ten (superseded 2026-11-03). No address is kept. A new
/// match is recognised by its tick going back.
#[derive(Default)]
pub(crate) struct Memory {
    /// Whether this thread has seen a step yet (1.7.1-dev7).
    started: bool,
    last_tick: u64,
    brains: [Brain; 10],
    /// The AI environment's kind this match (1.7.1-dev7: `signal_fate`),
    /// once read; for the wrapper, the inner one's. None: not in the table.
    pub env: Option<Option<u8>>,
    pub bounds_logged: bool,
    /// A brain with a side above 1 or an index above 4 was logged.
    pub range_logged: bool,
    lines: usize,
    budget_logged: bool,
    /// Kept across matches on this thread. Each match runs on its own
    /// worker thread, so in practice this is per match too.
    session_lines: usize,
    session_logged: bool,
}
#[derive(Clone, Copy, Default)]
struct Brain {
    seen: bool,
    tick: Option<u64>,
    digest: Option<u64>,
    /// Its call-join counters as last logged.
    join: Option<[u64; 8]>,
    /// The tick the mod last opened a scene for it (1.7.1-dev7), and
    /// whether its next join line is still owed.
    mod_scene: Option<u64>,
    join_owed: bool,
}
/// The slot of brain (side, index), when both are in range.
pub(crate) fn slot(side: u64, index: u32) -> Option<usize> {
    (side < 2 && index < 5).then(|| side as usize * 5 + index as usize)
}
impl Memory {
    /// Starts over when the tick goes back (a new match or set); the
    /// session's line count stays. True when it started over.
    ///
    /// Also true at the first step this thread sees (1.7.1-dev7): each match
    /// runs on a new worker thread, so there the tick never goes back. dev5
    /// and dev6 missed that, so a second match kept the first one's mapping
    /// state (seen in the dev6 log, 2026-11-03).
    pub(crate) fn observe_match(&mut self, tick: u64) -> bool {
        let new = tick < self.last_tick || !self.started;
        if new {
            *self = Self {
                session_lines: self.session_lines,
                session_logged: self.session_logged,
                ..Self::default()
            };
        }
        self.started = true;
        self.last_tick = tick;
        new
    }
    /// True the first time this brain is seen this match.
    pub(crate) fn first_sight(&mut self, slot: usize) -> bool {
        !std::mem::replace(&mut self.brains[slot].seen, true)
    }
    /// True for the brain's first step at `tick` (its queues are read at
    /// most once per tick).
    pub(crate) fn first_step(&mut self, slot: usize, tick: u64) -> bool {
        self.brains[slot].tick.replace(tick) != Some(tick)
    }
    /// True when the brain's lists differ from the last ones logged.
    pub(crate) fn changed(&mut self, slot: usize, digest: u64) -> bool {
        self.brains[slot].digest.replace(digest) != Some(digest)
    }
    /// True when the brain's call-join counters differ from the last ones
    /// logged.
    pub(crate) fn join_changed(&mut self, slot: usize, counts: [u64; 8]) -> bool {
        self.brains[slot].join.replace(counts) != Some(counts)
    }
    /// The mod opened a scene for this brain at `tick`: its next join line
    /// is owed, and forced for [`MOD_SCENE_TICKS`] after.
    pub(crate) fn mod_scene(&mut self, slot: usize, tick: u64) {
        if let Some(brain) = self.brains.get_mut(slot) {
            brain.mod_scene = Some(tick);
            brain.join_owed = true;
        }
    }
    /// Whether to write the brain's join line now: Some(forced) when the
    /// counters changed or a line is owed after a mod scene.
    pub(crate) fn join_line(&mut self, slot: usize, counts: [u64; 8], tick: u64) -> Option<bool> {
        let changed = self.join_changed(slot, counts);
        let brain = &mut self.brains[slot];
        let owed = std::mem::replace(&mut brain.join_owed, false);
        let forced = brain
            .mod_scene
            .is_some_and(|t| tick >= t && tick <= t + MOD_SCENE_TICKS);
        (changed || owed).then_some(forced)
    }
    /// Whether `count` more lines fit this match's and the session's
    /// budgets; the first time they do not, the one line to write instead.
    /// `forced` lines (a dump holding the mod's own entry) are always
    /// written and still counted.
    pub(crate) fn spend_or_force(
        &mut self,
        count: usize,
        forced: bool,
    ) -> Result<(), Option<&'static str>> {
        if forced {
            self.lines += count;
            self.session_lines += count;
            return Ok(());
        }
        if self.session_lines + count > LINES_PER_SESSION {
            if std::mem::replace(&mut self.session_logged, true) {
                return Err(None);
            }
            return Err(Some(
                "SIGNAL session limit reached: no more SIGNAL lines until the game restarts",
            ));
        }
        if self.lines + count <= LINES_PER_MATCH {
            self.lines += count;
            self.session_lines += count;
            Ok(())
        } else if !std::mem::replace(&mut self.budget_logged, true) {
            Err(Some(
                "SIGNAL budget reached: no more SIGNAL lines this match",
            ))
        } else {
            Err(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(due: u64, from: u32, tag_c: u8, tag: u8, lane: u8) -> [u8; QUEUE_ENTRY] {
        let mut raw = [0u8; QUEUE_ENTRY];
        raw[..8].copy_from_slice(&due.to_le_bytes());
        raw[8..12].copy_from_slice(&from.to_le_bytes());
        raw[0xc] = tag_c;
        raw[0x10] = tag;
        raw[0x11] = lane;
        raw[0x14..0x18].copy_from_slice(&7u32.to_le_bytes());
        raw[0x18..0x20].copy_from_slice(&8u64.to_le_bytes());
        raw[0x20..0x28].copy_from_slice(&9u64.to_le_bytes());
        raw
    }

    #[test]
    fn names_cover_the_28_kinds_and_mark_the_rest() {
        assert_eq!(name(0), "Start");
        assert_eq!(name(23), "GankDive");
        assert_eq!(name(27), "MorgardDisband");
        assert_eq!(name(28), "?28");
        assert_eq!(name(255), "?255");
    }

    #[test]
    fn a_queue_entry_logs_its_raw_bytes_and_each_field_at_its_offset() {
        let raw = entry(1234, 3, 12, 23, 1);
        let line = queue_line('A', 0, &raw);
        assert!(line.starts_with(&format!("SIGNAL A[0] raw={} ", hex(&raw))));
        assert!(line.ends_with(
            "due=1234 from=3 tagC=12 tag10=23 name=GankDive[Press] lane=1 d14=7 q18=8 q20=9"
        ));
        assert_eq!(hex(&raw).len(), 80);
    }

    #[test]
    fn a_list_c_entry_logs_kind_lane_and_word() {
        let mut raw = [0u8; LIST_ENTRY];
        raw[0] = 9;
        raw[1] = 2;
        raw[8..16].copy_from_slice(&77u64.to_le_bytes());
        assert!(list_line(4, &raw).ends_with("kind=9 lane=2 q8=77"));
    }

    #[test]
    fn a_dump_has_a_header_per_list_then_its_entries() {
        let a_bytes = [entry(10, 1, 0, 4, 0), entry(11, 2, 0, 5, 0)].concat();
        let a = Queue {
            capacity: 4,
            bytes: &a_bytes,
        };
        let b = Queue {
            capacity: 0,
            bytes: &[],
        };
        let c_bytes = [0u8; LIST_ENTRY];
        let c = Queue {
            capacity: 4,
            bytes: &c_bytes,
        };
        let lines = dump(9, 1, 3, (Some(6), Some(2)), [&a, &b, &c]);
        assert_eq!(lines.len(), 5, "an empty B has no header");
        assert_eq!(
            lines[0],
            "SIGNAL t=9 team=1 step_player=3 mode=6 delay=2 A len=2/4"
        );
        assert!(dump(9, 1, 3, (None, None), [&a, &b, &c])[0].contains(" mode=? delay=? "));
        assert!(lines[1].starts_with("SIGNAL A[0] "));
        assert_eq!(lines[3], "SIGNAL t=9 team=1 C len=1/4");
    }

    #[test]
    fn bounds_refuse_wrong_headers() {
        assert!(in_bounds(0, 8, 0));
        assert!(in_bounds(64, 0x1000, 64));
        assert!(!in_bounds(4, 0x1000, 5));
        assert!(!in_bounds(128, 0x1000, 1));
        assert!(!in_bounds(4, 0, 1));
        assert!(!in_bounds(4, 0x1004, 1));
    }

    #[test]
    fn the_digest_sees_entries_moving_between_lists() {
        let one = entry(1, 1, 0, 2, 0);
        let full = Queue {
            capacity: 4,
            bytes: &one,
        };
        let empty = Queue {
            capacity: 4,
            bytes: &[],
        };
        assert_ne!(
            digest([&full, &empty, &empty]),
            digest([&empty, &full, &empty])
        );
        assert_eq!(
            digest([&full, &empty, &empty]),
            digest([&full, &empty, &empty])
        );
    }

    #[test]
    fn memory_keeps_each_brain_apart_reads_once_per_tick_and_logs_only_changes() {
        let mut m = Memory::default();
        m.observe_match(5);
        assert_eq!(slot(0, 0), Some(0));
        assert_eq!(slot(1, 4), Some(9));
        assert_eq!(slot(2, 0), None);
        assert_eq!(slot(0, 5), None);
        for s in 0..10 {
            assert!(m.first_sight(s), "all ten brains are seen");
            assert!(!m.first_sight(s));
        }
        assert!(m.first_step(3, 5));
        assert!(!m.first_step(3, 5));
        assert!(m.first_step(4, 5));
        assert!(m.first_step(3, 6));
        assert!(m.changed(3, 42));
        assert!(!m.changed(3, 42));
        assert!(m.changed(3, 43));
        assert!(m.changed(4, 42), "brains do not share a digest");
    }

    #[test]
    fn memory_keeps_each_brains_join_counters_for_the_match() {
        let mut m = Memory::default();
        m.observe_match(5);
        assert!(m.join_changed(3, [0; 8]));
        assert!(!m.join_changed(3, [0; 8]));
        assert!(m.join_changed(3, [1, 0, 0, 0, 0, 0, 0, 0]));
        assert!(m.join_changed(4, [0; 8]), "each brain apart");
        m.observe_match(0);
        assert!(m.join_changed(3, [1, 0, 0, 0, 0, 0, 0, 0]), "a new match");
    }

    #[test]
    fn memory_starts_over_at_its_first_step_and_when_the_tick_goes_back() {
        let mut m = Memory::default();
        assert!(m.observe_match(50), "a new worker thread is a new match");
        assert!(m.first_sight(1));
        assert!(!m.observe_match(60));
        assert!(!m.first_sight(1));
        assert!(m.observe_match(3));
        assert!(m.first_sight(1));
    }

    #[test]
    fn after_a_mod_scene_the_join_line_is_owed_and_forced() {
        let mut m = Memory::default();
        m.observe_match(1);
        assert_eq!(m.join_line(3, [0; 8], 10), Some(false), "first sight");
        assert_eq!(m.join_line(3, [0; 8], 11), None, "unchanged");
        m.mod_scene(3, 20);
        assert_eq!(m.join_line(3, [0; 8], 21), Some(true), "owed, forced");
        assert_eq!(m.join_line(3, [0; 8], 22), None, "said once");
        assert_eq!(m.join_line(3, [1, 0, 0, 0, 0, 0, 0, 0], 300), Some(true));
        assert_eq!(
            m.join_line(3, [2, 0, 0, 0, 0, 0, 0, 0], 20 + MOD_SCENE_TICKS + 1),
            Some(false),
            "forced only for a while"
        );
        m.mod_scene(42, 5);
    }

    #[test]
    fn the_budget_stops_lines_and_says_so_once() {
        let mut m = Memory::default();
        m.observe_match(10);
        assert_eq!(m.spend_or_force(LINES_PER_MATCH, false), Ok(()));
        assert!(matches!(m.spend_or_force(1, false), Err(Some(_))));
        assert_eq!(m.spend_or_force(1, false), Err(None));
        m.observe_match(0);
        assert_eq!(m.spend_or_force(1, false), Ok(()));
        // A forced dump is written past the budget.
        let mut m = Memory::default();
        assert_eq!(m.spend_or_force(LINES_PER_MATCH, false), Ok(()));
        assert_eq!(m.spend_or_force(5, true), Ok(()));
        assert!(matches!(m.spend_or_force(1, false), Err(Some(_))));
    }

    #[test]
    fn the_session_limit_holds_across_matches() {
        let mut m = Memory::default();
        for tick in (0..3u64).rev() {
            m.observe_match(tick);
            assert_eq!(m.spend_or_force(LINES_PER_MATCH, false), Ok(()));
        }
        m.observe_match(0);
        assert_eq!(
            m.spend_or_force(1, false),
            Err(Some(
                "SIGNAL session limit reached: no more SIGNAL lines until the game restarts"
            ))
        );
        m.observe_match(0);
        assert_eq!(m.spend_or_force(1, false), Err(None));
    }
}
