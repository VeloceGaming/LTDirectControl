//! Game 0.7.0: the signal family `SIGNALS`: the ping orders in every
//! build since 1.7.1-dev13, the signal probe in research builds. Read
//! from the code on 2026-11-02; the dev4 log (seen in game by the author,
//! ten matches, 2026-11-03) confirmed the queue A
//! layout and corrected three readings, marked below. The verifier checks each value against the
//! executable (`SIGNAL_*` anchors in tools/native_profiles/0.7.0.json).
//!
//! The site is the per-player AI step's call to the team broadcast, right
//! after the step's prologue: `call broadcast(brain, context, player, sim,
//! receiver)`. The hook calls the broadcast first, then reads the brain's
//! queues, so the step player's own new calls are already in them.
//!
//! Superseded 2026-11-03: the last two arguments were read as the
//! simulation and its vtable. The step unpacks a pair {sim, receiver} into
//! them (`mov r15, [rsi]; mov rax, [rsi + 8]` at 0xf16dc4), so the fourth is
//! the simulation itself and the fifth the receiver state the delivery reads
//! its mode and delay from (read from the code).
//!
//! Since 1.7.1-dev6 the probe also follows each call in an inbox to its
//! fate, at five more call sites inside the delivery (see "Delivery" below).

/// The per-player AI step's call to the broadcast.
pub(crate) const SIGNAL_SITE: usize = 0xf16de7;
pub(crate) const SIGNAL_BYTES: [u8; 5] = [0xe8, 0xb4, 0xf5, 0xff, 0xff];
/// The broadcast itself, and its opening bytes.
pub(crate) const SIGNAL_ORIGINAL: usize = 0xf163a0;
pub(crate) const SIGNAL_PROLOGUE: [u8; 19] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec, 0x88,
    0x01, 0x00, 0x00,
];
/// The first argument's three lists, each a vector {capacity, pointer,
/// length}: queue A (40-byte entries the broadcast appends and the delivery
/// drains), queue B (40-byte entries, also appended by the broadcast) and
/// list C (24-byte entries).
///
/// Superseded 2026-11-03: the first argument is not a team planner but one
/// player's AI brain (its caller `0xf19810` stores that player's next
/// decision tick in it, worked out from the athlete's stats, and the dev4
/// probe saw a different one for each player). Queue A is that player's
/// inbox: the delivery checks every entry against the owner's own state,
/// whatever the entry's index, so the index at +8 is the sender (read from
/// the code; the dev4 note "the receiving teammate" is superseded too).
/// Entry: due tick @0, sender @8, 15 @0xc (the game's delay), tag @0x10,
/// lane @0x11, Mia's missing enemy @0x14, target entity id @0x18 (seen in
/// game).
pub(crate) const SIGNAL_TEAM_QUEUE_A: usize = 0x1ee8;
pub(crate) const SIGNAL_TEAM_QUEUE_B: usize = 0x1ea0;
pub(crate) const SIGNAL_TEAM_LIST_C: usize = 0x1eb8;
/// The player planner state: its index in the team (0..4, a dword) and its
/// side (0 or 1; superseded 2026-11-03: read as an identity until the dev4
/// log showed only 0 and 1, one value per team).
///
/// Superseded 2026-11-03: dev4 and dev5 read the delivery's delay at
/// player + 0x12f8 (a guess, marked so). The delivery reads it from the
/// receiver state instead: `[[receiver + 8] + 0x12f8]` (0xe75c80), which is
/// why the dev4 log showed values such as 528 and 1198509630577458497.
pub(crate) const SIGNAL_PLAYER_INDEX: usize = 0x8b0;
pub(crate) const SIGNAL_PLAYER_SIDE: usize = 0x820;
/// The receiver state (the step's fifth argument; the delivery's pair at
/// +8): its mode byte (0..8; the gate accepts a call only in some modes,
/// by the sender's slot) and the object holding the delay the delivery adds
/// to a due tick.
pub(crate) const SIGNAL_RECEIVER_MODE: usize = 0x38;
pub(crate) const SIGNAL_RECEIVER_STATE: usize = 0x8;
pub(crate) const SIGNAL_STATE_DELAY: usize = 0x12f8;
/// The simulation's current tick (the broadcast and the delivery compare
/// due ticks with it).
pub(crate) const SIGNAL_WORLD_TICK: usize = 0x1b8;
/// The player state's owner: his champion's entity id, a u64 (seen in game:
/// the dev6 log, two matches, 2026-11-03, every brain's own champion at
/// +0x8, and the same at +0x8c0; not read from the code). Since 1.7.1-dev7
/// the mapping reads it at every step.
///
/// Superseded 2026-11-03: dev5's scan bounds (`SIGNAL_PLAYER_SCAN`,
/// `SIGNAL_BRAIN_SCAN`) are gone with the scan.
pub(crate) const SIGNAL_PLAYER_ENTITY: usize = 0x8;

/// The brain's call-join counters (the game's CallJoinStats, serialised at
/// 0xf4d660 in this order; incremented in the scene opener 0xe662f0, which
/// has a second caller besides the one hooked below, so these count both):
/// eight u64 from +0x12c0: recv, ignored, in_battle, no_help, too_far,
/// low_hp, bail, join.
pub(crate) const SIGNAL_JOIN_STATS: usize = 0x12c0;

// Delivery (1.7.1-dev6), read from the code on 2026-11-03. The delivery
// 0xe75a60 (called from 0xe381b0 and 0xe4dc10) drains the inbox entries
// whose due tick has passed. An entry older than due + delay is late: only
// the queue-B removal runs (site LATE) and it is never heard. Otherwise the
// gate runs (site GATE): it returns at once for the receiver's own call,
// then accepts the call only when the receiver's mode is allowed for the
// sender's slot, then calls the receive handler at one of two sites (HEARD;
// the first also writes a trace record). For Battle and BattleHelp the
// handler opens a scene when the target exists (site SCENE).
//
// Each site is a `call rel32`; the hook calls the original first and
// returns its result unchanged.

/// The late drop: `call 0xe76090(brain, due, sender, &signal)`.
pub(crate) const SIGNAL_LATE_SITE: usize = 0xe75c32;
pub(crate) const SIGNAL_LATE_BYTES: [u8; 5] = [0xe8, 0x59, 0x04, 0x00, 0x00];
pub(crate) const SIGNAL_LATE_ORIGINAL: usize = 0xe76090;
pub(crate) const SIGNAL_LATE_PROLOGUE: [u8; 20] = [
    0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x55, 0x53, 0x50, 0x48, 0x8b, 0x81,
    0xb0, 0x1e, 0x00, 0x00,
];
/// The gate: `call 0xe65db0(brain, context, ?, player state, pair, sender,
/// &signal, flag, extra)`.
pub(crate) const SIGNAL_GATE_SITE: usize = 0xe75d27;
pub(crate) const SIGNAL_GATE_BYTES: [u8; 5] = [0xe8, 0x84, 0x00, 0xff, 0xff];
pub(crate) const SIGNAL_GATE_ORIGINAL: usize = 0xe65db0;
pub(crate) const SIGNAL_GATE_PROLOGUE: [u8; 19] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec, 0x88,
    0x01, 0x00, 0x00,
];
/// The gate's rule per sender slot (0..4, the jump table at 0x400fe70):
/// the modes 0..8 it accepts as a bit mask, and whether it accepts a mode
/// above 8. Slot 1 is coded apart (0xe660b0): it rejects modes 1 to 7
/// except 6, so it accepts 0, 6, 8 and anything above.
pub(crate) const SIGNAL_GATE_RULES: [(u16, bool); 5] = [
    (0x185, false),
    (0x141, true),
    (0x1b1, false),
    (0x1ab, false),
    (0x1ab, false),
];
/// The two calls of the receive handler 0xe67300(brain, context, ?, player
/// state, pair, sender, &signal, extra).
pub(crate) const SIGNAL_HEARD_SITES: [usize; 2] = [0xe65f25, 0xe660ef];
pub(crate) const SIGNAL_HEARD_BYTES: [[u8; 5]; 2] = [
    [0xe8, 0xd6, 0x13, 0x00, 0x00],
    [0xe8, 0x0c, 0x12, 0x00, 0x00],
];
pub(crate) const SIGNAL_HEARD_ORIGINAL: usize = 0xe67300;
pub(crate) const SIGNAL_HEARD_PROLOGUE: [u8; 19] = [
    0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x55, 0x53, 0x48, 0x81, 0xec, 0x68,
    0x04, 0x00, 0x00,
];
/// The handler's Battle, BattleHelp and BattleDive cases opening a scene:
/// `call 0xe662f0(brain, context, ?, player state, pair, sender, reason,
/// target, extra)`, reason 1 Battle (0xe673be), 2 BattleHelp (0xe67538), 3
/// BattleDive (0xe674dc), one call for all three. Each case first asks the
/// AI environment its kind (`call [vtable + 0x40]`) and opens nothing in
/// kind 0 (see `SIGNAL_ENV_VTABLES`).
///
/// Superseded 2026-11-03 (1.7.1-dev7): dev6 read reason 3 as Battle and
/// the call before it as a target lookup; the tick stamped at brain +
/// 0xc40 belongs to tag 1 (Mia), not BattleDive.
pub(crate) const SIGNAL_SCENE_SITE: usize = 0xe67580;
pub(crate) const SIGNAL_SCENE_BYTES: [u8; 5] = [0xe8, 0x6b, 0xed, 0xff, 0xff];
pub(crate) const SIGNAL_SCENE_ORIGINAL: usize = 0xe662f0;
pub(crate) const SIGNAL_SCENE_PROLOGUE: [u8; 19] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec, 0x88,
    0x09, 0x00, 0x00,
];

/// The AI environment (a trait object at the start of the simulation, the
/// step hook's fourth argument): its vtables by RVA, with the kind method
/// slot 0x40 returns (`signal_fate::ENV_WRAPPER` for the wrapper, whose
/// slot 0x40 forwards to an inner environment `[[sim]] / [[sim] + 8]`). In
/// kind 0 the receive handler opens no scene for a call (read from the
/// code, 2026-11-03; both dev6 matches were kind 0, seen in game).
pub(crate) const SIGNAL_ENV_VTABLES: &[(usize, u8)] = &[
    (0x40398e0, u8::MAX),
    (0x40a2530, 0),
    (0x40c9f28, 0),
    (0x40a33a0, 1),
    (0x40a3f98, 2),
    (0x40c9898, 2),
];

// The scene call's answer (1.7.1-dev8), read from the code on 2026-11-03:
// what the scene opener 0xe662f0 reads and writes for the probe's
// before-and-after lines around the mod's own call.

/// The mod's mark at inbox entry + 0x14 (signal word + 4), the ASCII bytes
/// "GPTL" (0x4c545047, "LT PinG" read backwards). The mod's choice, not a
/// game value. That word is free for BattleHelp (tag 4), read from the
/// code: only tag 1 (Mia) reads it, in the receive handler (0xe6750b), the
/// compatibility check (0xfcd67e) and the late drop's queue-B match
/// (0xe76149); the gate passes the signal on, and the inbox's other
/// readers are the broadcast, its clone and its drop, which copy or free
/// whole entries.
pub(crate) const SIGNAL_MARK: u32 = 0x4c54_5047;
/// The brain's current scene kind (a u64; read as a dword at 0xe7292d):
/// at most 1 means in battle. In the in-battle branch, for kind 0 with the
/// target within 150 units, the scene opener itself sets it to 1 and its
/// argument (+0x16a8) to the target unit's [+0x5b8] (0xe66566).
pub(crate) const SIGNAL_SCENE_KIND: usize = 0x16a0;
pub(crate) const SIGNAL_SCENE_ARG: usize = 0x16a8;
/// The scene block the set-scene function 0xe72740 writes after a join
/// (0xe729ee..0xe72a83): +0x15f0 = 1, the world tick, the target's
/// position (x, y), the distance (milli-units), a byte. It returns before
/// writing when the distance is under 250 units (0x3d091).
pub(crate) const SIGNAL_SCENE_BLOCK: usize = 0x15f0;
pub(crate) const SIGNAL_SCENE_TICK: usize = 0x1638;
pub(crate) const SIGNAL_SCENE_POS: usize = 0x1648;
pub(crate) const SIGNAL_SCENE_DIST: usize = 0x1658;
pub(crate) const SIGNAL_SCENE_FLAG: usize = 0x1688;
/// A join pushes an Ok reply onto list C (`SIGNAL_TEAM_LIST_C`, 24-byte
/// entries): tag 9 at +0, a qword 0 at +8 (0xe66f59), then `join` rises.
/// The probe logs the new entry's tag as read (`replyC=`).
/// The simulation's units, two sides of five pointers: the unit of player
/// (side, index) is `[sim + 0x238 + (side * 5 + index) * 8]` (the receive
/// handler's own read at 0xe67387, the scene opener's at 0xe664e5).
pub(crate) const SIGNAL_SIM_UNITS: usize = 0x238;
/// A unit's fields the scene opener judges by (u64 each): position x (y 8
/// bytes on), move speed, max hp and hp (low_hp: hp * 100 / max hp under
/// 50, an unsigned division).
pub(crate) const SIGNAL_UNIT_POS: usize = 0x658;
pub(crate) const SIGNAL_UNIT_SPEED: usize = 0x638;
pub(crate) const SIGNAL_UNIT_MAX_HP: usize = 0x620;
pub(crate) const SIGNAL_UNIT_HP: usize = 0x668;

// Per-player levers (1.7.1-dev10), read from the code on 2026-11-03 (see
// docs/investigation-team-ai.md, sections 3, 4 and 6). Three tests ride on
// the step hook above and the inbox write; no new hook site.

/// The team plan inside the brain (TeamPlan, Debug formatter 0xe801a0):
/// the objective setter's first argument is `brain + SIGNAL_TEAM_PLAN`.
pub(crate) const SIGNAL_TEAM_PLAN: usize = 0xe8;
/// The objective, 3 bytes (tag, byte 1, byte 2; tag 0xff none): the setter
/// writes them at team plan + 0xd5d (0xec3a5f, 0xec3a69); the HideLine case
/// tests the tag for 0xff (0xe6782d).
pub(crate) const SIGNAL_OBJECTIVE: usize = 0xe45;
/// The team plan's trace flag (team plan + 0x748): when it is not 0 the
/// setter allocates a trace record (0xec3851); the probe then does not call
/// it.
pub(crate) const SIGNAL_TRACE_ON: usize = 0x830;
/// The objective setter `0xec3820(team plan, objective u24 in edx,
/// &Location)`. With tracing off (0xec3a28..0xec3a7f) it clears side
/// fields by the new tag and writes the 3 bytes, calling nothing; the
/// Location is read only with tracing on. Its first 30 bytes.
pub(crate) const SIGNAL_OBJECTIVE_SETTER: usize = 0xec3820;
pub(crate) const SIGNAL_OBJECTIVE_PROLOGUE: [u8; 30] = [
    0x55, 0x41, 0x57, 0x41, 0x56, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec, 0x08, 0x01, 0x00, 0x00, 0x48,
    0x8d, 0xac, 0x24, 0x80, 0x00, 0x00, 0x00, 0x48, 0xc7, 0x85, 0x80, 0x00, 0x00, 0x00,
];
/// The big plan's tag (a u64; raw 0 and 1 Battle, else the variant index +
/// 2) and its first payload word. The same fields as `SIGNAL_SCENE_KIND`
/// and `SIGNAL_SCENE_ARG`: dev8 named them the scene kind (superseded
/// 2026-11-03, investigation-team-ai.md section 11); the dev8 lines keep
/// the old names.
pub(crate) const SIGNAL_BIG_PLAN: usize = 0x16a0;
pub(crate) const SIGNAL_BIG_PLAN_ARG: usize = 0x16a8;
/// The brain's believed enemy positions (VisionTrackingState, formatter
/// 0xe7ab30), five entries per field by the enemy player's index (+0x8b0
/// of his player state, 0xecbf86): position (x, y u64, 1000 per unit; 16
/// bytes an entry), last seen and last checked ticks, and the distance.
/// Between the position and the ticks lie the Mia stamps (+0xc40), after
/// the distance the hp ratios (+0xce0): the probe never writes either.
///
/// Read from the code (the vision update 0xec9270, 0xecbfe0..0xecc050):
/// the "distance" is the squared distance, dx * dx + dy * dy over unsigned
/// differences in milli-units, with no square root; 10^12 ((1000 units)^2)
/// stands for an enemy whose unit is missing (0xecc09e). The team-AI
/// document and the dev10 brief call it a distance.
pub(crate) const SIGNAL_VISION_POS: usize = 0xbf0;
pub(crate) const SIGNAL_VISION_SEEN: usize = 0xc68;
pub(crate) const SIGNAL_VISION_CHECKED: usize = 0xc90;
pub(crate) const SIGNAL_VISION_DISTANCE: usize = 0xcb8;
// The inbox's HideLine (tag 6; tag 7 HideLineToo takes the same case,
// 0xe67353, jump table 0x400feac): after its lane rules, a receiver with
// no objective, in roster slot 0, 2, 3 or 4, told its own lane gets
// Gank(lane) through the setter (0xe67872). Superseded 2026-11-04
// (1.7.1-dev21): `SIGNAL_HIDE_LINE`, which only the research key F5 sent;
// the verifier still checks the case (tag 6), which guards the objective
// and plan offsets.

// 1.7.1-dev12, read from the code on 2026-11-03: two read-only fields for
// the standing request and the plan's lane.

/// The objective handlers' drop reason, one byte at `team_plan + 0xc8`:
/// 0x20 the mode gate (0xeb72d0), 0x21 the Serpen respawn rule (0xeb74ac,
/// the tick at `team_plan + 0xd0`); 0x24 and 0x23 in the Morgard handler
/// (0xeb7831, 0xeb7871), meaning not read. Read only.
pub(crate) const SIGNAL_OBJECTIVE_REASON: usize = 0x1b0;
/// The big plan's lane by its tag, from its Debug formatter 0xe766c0 (jump
/// table 0x4010114 by tag - 2; every payload at `big plan + 8`) and each
/// payload's own formatter: PassiveLine (3) `line` at payload + 0x11c
/// (0x1127729), SinglePlanLine (4) at + 0x19 (0x100b54d), LineGanker (10)
/// at + 0x33 (0x10e7339), AttackNexus (13) at + 8 after its u64 `team`
/// (0x112762c). One byte each, the game's Line enum: its formatters' name
/// tables give 0 Top, 1 Mid, 2 Bottom. Read only.
pub(crate) const SIGNAL_PLAN_LANES: &[(u64, usize)] =
    &[(3, 0x17c4), (4, 0x16c1), (10, 0x16db), (13, 0x16b0)];

// 1.7.1-dev13, read from the code on 2026-11-04: the ping orders' reads
// and calls (docs/investigation-team-ai.md section 14; the handler's cases
// in docs/investigation-ai-signals.md section 4).

/// The tick the drop reason was written, a u64 at `team_plan + 0xd0`, next
/// to the byte (`mov [rcx + 0xd0], rax` at 0xeb74b3 after the 0x21 store;
/// `mov [r13 + 0xd0], rax` at 0xeb72d8 after the 0x20 store, both the world
/// tick). A standing request stops on a permanent reason only when this
/// tick is newer than its own write. Read only.
pub(crate) const SIGNAL_OBJECTIVE_REASON_TICK: usize = 0x1b8;
// Superseded 2026-11-04 (1.7.1-dev17): the inbox's BattleStop (tag 5) and
// Mia (tag 1) calls, inert in kind 0 (their cases end at the environment's
// kind check), and dev15's BattleStop record (brain + 0x890, the research
// key F10) are no longer sent or written; their constants are gone. The
// cases stay anchored in the profile (SIGNAL_MIA_CASE, SIGNAL_STOP_CASE).

// The Mia stamps the inbox's Mia case writes in kinds other than 0
// (`mia_call_ticks`, five u64 by enemy index, `brain + 0xc40 + index * 8`,
// case 0xe6750b). Superseded 2026-11-04 (1.7.1-dev21): `SIGNAL_MIA_STAMPS`,
// which only the research key F9 wrote; the verifier still checks the
// place (between `SIGNAL_VISION_POS` and `SIGNAL_VISION_SEEN`).

// 1.7.1-dev17, read from the code on 2026-11-04: the driven windows
// (docs/investigation-team-ai.md sections 20 to 26; the author's spec
// design/pings-spec-stage1.md section 10.3). Written at the step hook
// above, which runs before the same think's plan update and sub plan
// choice (section 20), each only while the big plan is the variant that
// owns the field.

/// The big plan's tags the windows read (`SIGNAL_BIG_PLAN`): Battle is raw
/// 0 or 1, PassiveLine raw 3 (the plan formatter's case table by tag - 2,
/// section 3; the support rule's `cmp dword [r10 + 0x16a0], 3` at
/// 0xe40b7d).
pub(crate) const SIGNAL_PLAN_BATTLE: [u64; 2] = [0, 1];
pub(crate) const SIGNAL_PLAN_PASSIVE_LINE: u64 = 3;
/// PassiveLine's flags, brain offsets (payload at `SIGNAL_BIG_PLAN_ARG`;
/// formatter 0x1127710). The sub plan choice 0x1105f80 reads `regroup_now`
/// first (`cmp byte [rdx + 0x11a], 0` at 0x1106003: LineWait at the own
/// tower) and `hold_wave` next (`cmp byte [rdx + 0x116], 0` at 0x110601c:
/// LineWait with the wave). The builder writes `hold_wave` and `waiting`
/// together (`mov word [rsi + 0x116], 0x101` at 0xe56886) and `push_basis`
/// (`mov byte [rsi + 0x11f], 5` at 0xe568a8). `line` is PassiveLine's entry
/// of `SIGNAL_PLAN_LANES` (0x17c4).
pub(crate) const SIGNAL_PLAN_HOLD_WAVE: usize = 0x17be;
pub(crate) const SIGNAL_PLAN_WAITING: usize = 0x17bf;
pub(crate) const SIGNAL_PLAN_SPLIT_ROLE: usize = 0x17c0;
pub(crate) const SIGNAL_PLAN_REGROUP_NOW: usize = 0x17c2;
pub(crate) const SIGNAL_PLAN_PUSH_BASIS: usize = 0x17c7;
/// The fight plan's run-away latch `well_runaway` (BattlePlan + 0x20 and +
/// 0x28 with the plan at brain + 0x16a0): an Option, the tag u64 and the
/// tick it runs to. Its update reads it at every think outside the fountain
/// (`cmp byte [rdi + 0x20], 0` at 0xefd014, `cmp rax, [rdi + 0x28]` at
/// 0xefd025 with rax the world tick) and forces the sub goal RunAway while
/// the tick is ahead; once reached it clears it (`mov qword [rdi + 0x20],
/// 0` at 0xefd8ae). The fountain exit writes (1, now + 5 x the clock word
/// `[[receiver + 8] + 0x12f8]`, `SIGNAL_STATE_DELAY`) at 0xefcde7..0xefce01.
/// RunAway does not end the plan: the done test ends a Battle only at sub
/// goal End (`cmp dword [rcx + 0xe0], 4` at 0xf15928).
pub(crate) const SIGNAL_BATTLE_RUNAWAY: usize = 0x16c0;
pub(crate) const SIGNAL_BATTLE_RUNAWAY_TICK: usize = 0x16c8;
/// The handler's bottom-support rule (0xe40a6b..0xe40dd3), which sets
/// `hold_wave` and `waiting` itself before the sub plan choice. Its gates,
/// in order:
/// - the roster slot (`cmp dword [rbp + 0x1b90], 4`);
/// - in AI kind 0, the receiver's mode byte (`SIGNAL_RECEIVER_MODE`, read
///   at 0xe40aa6) in mask 0x1ab (`mov eax, 0x1ab` at 0xe40b49), and either
///   simulation word at 0x218 or 0x220 zero (0xe40b5e, 0xe40b6f);
/// - the plan PassiveLine (0xe40b7d);
/// - a unit at `sim + 0x258 + [rbp + 0x1518] * 8` (not mirrored: its index
///   was not read);
/// - `split_role` 0 (0xe40ba8);
/// - the objective not Gank or Dive on the plan's own lane
///   (0xe40bb6..0xe40bd0).
///
/// Kinds other than 0 take mask 0x1a1 and the first-Serpen time instead
/// (0xe40ad7..0xe40b1d); the viewed match is kind 0.
pub(crate) const SIGNAL_SUPPORT_SLOT: u8 = 4;
pub(crate) const SIGNAL_SUPPORT_MODES: u16 = 0x1ab;
pub(crate) const SIGNAL_SUPPORT_SIM_WORDS: [usize; 2] = [0x218, 0x220];

// 1.7.1-dev18, read from the code on 2026-11-04: the take order and the
// team's own Serpen or Morgard play (docs/investigation-team-ai.md sections
// 27 to 31; the author's spec section 11.9). Read at the step hook above;
// the one call is the game's own plan replace helper.

/// `sync_follower` (TeamPlan + 0xd5a, the cascade's
/// `mov byte [rsi + 0xd5a], al` at 0xec45cb after the publisher pick
/// `call 0xeceb50` at 0xec45a5): 0 only on the teammate who publishes the
/// team's play, as the last decision left it. Read only.
pub(crate) const SIGNAL_SYNC_FOLLOWER: usize = 0xe42;
/// The team's shared block: `[context + 0x10] + side * 0xea0` (0xec45db,
/// 0xec45df; the publisher pick reads the same at 0xeceb5e, its unit test
/// `[context] + 0x238` at 0xeceb98 is the simulation's). Its `ObjSceneSync`
/// at + 0xd8 (−1 none, 0xec45f8; the relay's test at 0xe3cfd5), whose
/// Serpen and Morgard record None bytes sit at + 0x173 and + 0x193 (2 =
/// none; the relay reads + 0x9b of the sync at 0xe3d004). Read only.
pub(crate) const SIGNAL_CONTEXT_SHARED: usize = 0x10;
pub(crate) const SIGNAL_SHARED_STRIDE: usize = 0xea0;
pub(crate) const SIGNAL_SHARED_SYNC: usize = 0xd8;
pub(crate) const SIGNAL_SHARED_RECORDS: [usize; 2] = [0x173, 0x193];
/// The brain's own Serpen and Morgard records' None bytes (TeamPlan +
/// 0xa8b, + 0xaab: the 0x30 block's tests at 0xec7081, 0xec7212). The
/// value 2 is None. Read only (the probe's lines).
pub(crate) const SIGNAL_BRAIN_RECORDS: [usize; 2] = [0xb73, 0xb93];
pub(crate) const SIGNAL_RECORD_NONE: u8 = 2;
/// The plays' hunt plans, raw tags: SerpenHuntAndPoke 12 (the builder's
/// `mov qword [r12], 0xc` at 0xed4ac3, the handler's test at 0xeb7694),
/// EpicHuntAndPoke 11 (Morgard's builder, 0xed2442).
pub(crate) const SIGNAL_PLAN_PLAY: [u64; 2] = [12, 11];
/// The game's own plan replace helper `0xec3ba0(team plan, plan, &buffer,
/// &Location)`: with tracing off (`cmp byte [rcx + 0x748], 0` at 0xec3bc6,
/// `SIGNAL_TRACE_ON`) it drops the old plan (0xec3ed0) and copies 0x400
/// bytes from the buffer (0xec3ee1), allocating nothing. Its first 51
/// bytes, through the trace test. The Serpen handler calls it with a
/// buffer whose first u64 is 2, ForcePassive (0xeb7699..0xeb76b1), and the
/// handler rebuilds the plan from the objective after the cascade.
pub(crate) const SIGNAL_PLAN_REPLACE: usize = 0xec3ba0;
pub(crate) const SIGNAL_PLAN_REPLACE_PROLOGUE: [u8; 51] = [
    0x55, 0x41, 0x56, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec, 0x40, 0x01, 0x00, 0x00, 0x48, 0x8d, 0xac,
    0x24, 0x80, 0x00, 0x00, 0x00, 0x48, 0xc7, 0x85, 0xb8, 0x00, 0x00, 0x00, 0xfe, 0xff, 0xff, 0xff,
    0x4d, 0x89, 0xc6, 0x48, 0x89, 0xd6, 0x80, 0xb9, 0x48, 0x07, 0x00, 0x00, 0x00, 0x0f, 0x84, 0xfa,
    0x02, 0x00, 0x00,
];
pub(crate) const SIGNAL_FORCE_PASSIVE: u64 = 2;
pub(crate) const SIGNAL_PLAN_SIZE: usize = 0x400;
/// The builder's walk rule for a Setup play (Serpen `0xed4520`, Morgard
/// `0xed1ae0`): walk only when `spawn tick - now <= travel + 2 x the clock
/// word` (0xed4a55/0xed4a59; Morgard 0xed1efb/0xed1eff). Travel is the
/// distance from the unit (`SIGNAL_UNIT_POS`) to the side's rally point over
/// the unit's speed (`SIGNAL_UNIT_SPEED`, at least 1). The rally point is at
/// `[receiver + 0x20] + rally + side * 0x10` (x, then y), Serpen 0x7108,
/// Morgard 0x70e8 (0xed46fc, 0xed1bf8). When the dword at + 0x7168 of that
/// object is not 0 the builder takes another branch (0xed46e8, 0xed1be2):
/// the mod then asks no rebuild. The spawn ticks are words of the AI
/// environment's kind-0 object (its kind method 0x16ab380 returns `rcx +
/// 0xf078` beside kind 0): Serpen + 0x1e0 (0xed4726), Morgard + 0x1b0
/// (0xed1c22); Serpen is alive while + 0x1d8 is not 0 (0xed4ad0).
pub(crate) const SIGNAL_RECEIVER_WORLD: usize = 0x20;
pub(crate) const SIGNAL_WORLD_OTHER_RALLY: usize = 0x7168;
pub(crate) const SIGNAL_RALLY: [usize; 2] = [0x7108, 0x70e8];
pub(crate) const SIGNAL_RALLY_STRIDE: usize = 0x10;
pub(crate) const SIGNAL_ENV_OBJECT: usize = 0xf078;
pub(crate) const SIGNAL_SPAWN_TICK: [usize; 2] = [0x1e0, 0x1b0];
pub(crate) const SIGNAL_SERPEN_ALIVE: usize = 0x1d8;
/// The walk rule's slack: two of the clock word (2 s).
pub(crate) const SIGNAL_WALK_SLACK: u64 = 2;
