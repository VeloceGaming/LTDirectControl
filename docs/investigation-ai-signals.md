# Investigation: the game AI's own call system, and how a ping can use it

Written 2026-11-02, extended 2026-11-03 with the probe's results. Question
from the author: for the teammate ping system, should the mod limit what
teammates may do, force them to do what the player wants, or is there a
third way? The third way turned out to exist: the game's AI already talks
to itself with calls, and the mod can put a call into that channel.

Every finding is labelled:

- **Seen in game**: the author played it (build and game version given).
- **Read from the code**: static reading of the 0.7.0 executable
  (sha256 `d678e012…7d30`; addresses are RVAs of that build only).
- **Guess**: plausible, not tested.
- **Rejected**: tried and wrong.

## 1. Short answer

- **Read from the code.** The planner crate `game-ai` has a `Signal`
  enum of 28 kinds (section 3). A player's planner emits calls; a team
  broadcast puts them into a per-team queue with a due tick; the delivery
  step drains due entries, drops ones the receiver is too late for, and
  hands the rest to the receive handler, which changes the receiver's
  plan flags. The AI keeps statistics on why a call was not joined
  (`recv`, `ignored`, `in_battle`, `no_help`, `too_far`, `low_hp`,
  `bail`, `join`) and a counter of misunderstood calls.
- **Read from the code.** The official mod interface exposes none of
  this (only the team strategy and player statistics as JSON).
- **Seen in game (0.7.0, 1.7.1-dev4-research, ten matches, 2026-11-03).**
  A read-only hook at the broadcast call confirmed the queue layout
  (section 4) and showed which calls players actually send to each other
  (section 5): only the fight, hide and missing calls.
- **Decision (the author, 2026-11-02): "talk to the planner".** Not
  "force" (the mod moving teammates itself, stage 1, rejected in play) and
  not "limit" (a veto on idle AI only), but one hook that appends a call
  the AI already understands. Step 1 (1.7.1-dev5) sends one call,
  BattleHelp on a hovered enemy.
- **Superseded 2026-11-03 (seen in game, 1.7.1-dev5 and dev6, three
  matches; then read from the code).** The inbox route cannot move a
  teammate in the match the player watches. The delivery probe (section
  10) saw 911 calls reach the receive handler and none open a fight
  scene, because the handler first asks the match environment for its
  *kind* and does nothing for kind 0, the viewed match. Teammates fight
  together because the team handler opens fight scenes itself and judges
  them by its join rules. The lever that works in a viewed match is that
  scene opener, called for the player's own call (step 2, 1.7.1-dev7).

## 2. Three approaches compared

| Approach | What the mod does | Cost | Verdict |
| --- | --- | --- | --- |
| Force | Replace a teammate's move with a walk to the ping; re-create the AI's safety rules | Official interface only; a second, worse AI | **Rejected** in play (1.7.1-dev1, 2026-10-29): the fleeing helper lies, teammates dragged across the map |
| Limit | Let the AI keep control; nudge only when a teammate is visibly idle | Official interface only; half-blind, cannot read intent | Fallback only |
| Talk to the planner | Append the game's own call to the team queue | One native hook site, version-folder offsets | **Chosen** |

## 3. The signal language (read from the code)

`Signal` is 16 bytes: a tag byte, a lane byte for lane kinds, a 32-bit
number at +4 for `Mia`, a 64-bit target at +8 for target kinds. Tags, from
the serializer's jump table at `0x4047218` (function `0x10b36c0`):

| Tag | Kind | Payload | Tag | Kind | Payload |
| --- | --- | --- | --- | --- | --- |
| 0 | Start | | 14 | Repair | |
| 1 | Mia | unit index | 15 | SerpenPrepare | target |
| 2 | Battle | target | 16 | SerpenSetup | |
| 3 | BattleDive | target | 17 | SerpenHunt | |
| 4 | BattleHelp | target | 18 | MorgardPrepare | target |
| 5 | BattleStop | | 19 | MorgardSetup | |
| 6 | HideLine | lane | 20 | MorgardHunt | |
| 7 | HideLineToo | lane | 21 | AttackNexus | lane |
| 8 | DefenseLine | lane | 22 | DefenseNexus | |
| 9 | Ok | | 23 | GankDive | lane |
| 10 | Reject | | 24 | PressTower | lane |
| 11 | Cancel | | 25 | ComebackPick | lane |
| 12 | Press | lane | 26 | SerpenDisband | |
| 13 | PressChange | lane | 27 | MorgardDisband | |

Lane byte 0 Top, 1 Mid, 2 Bottom (**guess** from the planner's name
table; the probe's lanes were 0..2 on lane kinds).

Related names from the executable's field tables: per-player plan state
with `signals`, `in_recall`, `commit`, `flee…`, `hold_wave`, `waiting`,
`split_role`, `receiving`, `push_basis`, `regroup_now`, `join_gank`;
`CallJoinStats` (`recv`, `ignored`, `in_battle`, `no_help`, `too_far`,
`low_hp`, `bail`, `join`); trace events `OrderResponse {to, accept}` and
`Signal {from, signal}`; `misunderstood_received_signals`,
`signals_wait`, `received_signals`; `LaneOpsSync {owner, splitter,
reason, recv, collapse, gather, defense}`; `TeamScene {kind, target,
anchor, phase, since_tick, quorum, …, arrive_at, deadline, quorum_k}`.

## 4. How a call travels (read from the code, confirmed in game where noted)

**Superseded 2026-11-03 (the 1.7.1-dev5 coder, read from the code and
from the dev4 log):** the structure called "team state" below is one
player's own AI brain, not shared by the team. Each of the ten players has
one; the step's only caller `0xf19810` stores that player's next decision
tick in it (`+0x47c0`), and the dev4 log showed one brain per side plus a
"third" at tick 5. Queue A is therefore that player's **inbox**: the
broadcast writes one entry into each teammate's brain, and the index at
+8 is the **sender**. The word at `+0x820` of the player state is the
player's side (0 or 1), not an identity. The table rows are kept as first
read; read "brain (one per player)" for "team state".

| Step | Function | What it does |
| --- | --- | --- |
| Per-player AI step | `0xf16d60` | `(team state rcx, context rdx, player state r8, sim r9, sim vtable on the stack)`. Its first call is the broadcast; later `0xe50330` runs the planning step and the team handler `0xe381b0`. |
| Broadcast | `0xf163a0` | Appends the sender's pending call as a 40-byte entry to **queue A** of the team state: capacity `+0x1ee8`, pointer `+0x1ef0`, length `+0x1ef8`. |
| Send | `0xe4e770` | Called five times from the team handler: checks the plan mode's allowed set, then removes conflicting entries from lists C (`+0x1eb8`) and D (`+0x1ed0`) and runs a compatibility retain (`0xfc9460`) on A and **queue B** (`+0x1ea0`). |
| Delivery | `0xe75a60` | Drains A entries whose due tick has passed; an entry older than `due + player.delay` (`player + 0x12f8`) is dropped; else the matching B entry is removed (`0xe76090`) and the receive gate runs. |
| Receive gate | `0xe65db0` | Skips the sender's own calls; per sender index a bitmask of allowed tags (0x1ab, 0x1b1, 0x185 over tags 0..8); a compatibility check `0xfcd5e0`; writes a trace record. **Superseded 2026-11-03:** the masks are chosen by the sender's roster slot and tested against the receiver's *mode* byte (0..8), not against the tag: slot 0 accepts modes 0, 2, 7, 8; slot 1 modes 0, 6, 8 and above 8; slot 2 modes 0, 4, 5, 7, 8; slots 3 and 4 modes 0, 1, 3, 5, 7, 8. The compatibility check always passes the four Battle kinds. In game (dev6, two matches) the mode was 0 throughout and every call passed. |
| Receive handler | `0xe67300` | A jump table over tags 1..11 only (Mia … Cancel). Sets per-player flags and timers, pushes a reply record into list C, and for three kinds opens a lane-ops scene via `0xe662f0` (reason 1, 2 or 3: collapse, gather, defense). **Superseded 2026-11-03:** the Mia, Battle, BattleDive and BattleHelp cases first ask the match environment for its kind (section 10) and do nothing in kind 0. Battle, BattleHelp and BattleDive would call the scene opener `0xe662f0` with reason 1, 2 or 3; Mia would stamp a tick at `brain + 0xc40 + enemy*8`. |

**Seen in game (ten matches):** a queue-A entry is

| Offset | Field | Observed |
| --- | --- | --- |
| 0 | due tick, u64 | current tick + 15..20 |
| 8 | index, u32 | 0..4; each call appears once, spread evenly: most likely the **sender** |
| 0xc | u32 | always 15 |
| 0x10 | tag | names from the table |
| 0x11 | lane | 0..2 on lane kinds |
| 0x14 | u32 | Mia: the missing enemy's index 0..4 |
| 0x18 | target, u64 | unit ids (21, 94, 411, 425 …) on Battle kinds |
| 0x20 | u64 | 0 on Battle kinds; small numbers or 65000 on Mia and Cancel |

Capacity is 4 after the first growth; length is 0..2 almost always, 4 once
in 931 steps. The match runs at 59.7 ticks per second (measured from the
log's timestamps), so a call is audible a quarter to a third of a second
after it is sent. Queue B held 6 entries in ten matches; list C none.

The per-player value at `+0x12f8` is 0, 1 or 2. **Guess:** a hearing
window: the delivery drops an entry once `due + value < now`, so a
teammate hears a call only if one of its decision steps lands within that
many ticks after the due tick. If so, missed pings are already modelled
by the game, per athlete.

**Superseded 2026-11-03 (read from the code; seen in game, dev6):** the
delay the delivery adds is not at `player + 0x12f8` (that read showed
values such as 528). The AI step receives a pair `{sim, receiver}`, and
the delay is `[[receiver + 8] + 0x12f8]`, the receiver's mode byte
`[receiver + 0x38]`. In game the delay was 60 ticks for every player, one
second, and in two matches no call was ever dropped as late. The hearing
window exists, but it is wide.

## 5. What players actually send each other (seen in game, ten matches)

Counts of queue-A entries by kind: Mia 177, Battle 132, BattleStop 87,
BattleHelp 83, HideLine 57, Cancel 30, BattleDive 17, HideLineToo 1. No
GankDive, Press, PressTower, PressChange, DefenseLine, AttackNexus,
DefenseNexus, Repair, objective call, Ok or Reject was ever seen. This
matches the receive handler, which dispatches tags 1..11 only: the other
seventeen kinds are the team planner's directives, not player-to-player
calls, and would not be acted on if put into this queue.

**Injectable set:** Mia, Battle, BattleDive, BattleHelp, BattleStop,
HideLine, HideLineToo, DefenseLine, Ok, Reject, Cancel.

## 6. What this means for the ping design

- A ping is a unit or a lane, never a free point.
- Four intents on the wheel map onto the injectable set without
  stretching: **Go** = BattleHelp, Battle or BattleDive on the hovered
  unit; **Defend** = DefenseLine on the lane; **Back off** = HideLine on
  the lane or BattleStop; **Missing** = Mia with the enemy's index. Push
  and gank pings cannot be sent this way.
- Replies: Ok and Reject exist on the channel but were never seen sent by
  the AI. Whether teammates reply is open.
- The gate layer the author wants (teammates who miss or refuse) becomes
  a filter in front of the queue; the AI's own `CallJoinStats` reasons
  can feed the shown replies.
- **Superseded 2026-11-03:** a call in the inbox is heard but not acted
  on in the viewed match (section 10). The ping has to open the fight
  scene itself, through the game's scene opener, which still runs the
  join rules: a ping stays a request the teammate judges. The AI's own
  fight scenes are joined rarely (dev6: 9 of 45 on one team, 4 of 46 on
  the other; "too far", "no help needed" and "already fighting" were the
  usual refusals), so refusals shown as replies will be the common case.

## 7. The hook (read from the code; probe seen in game)

The probe (1.7.1-dev4-research, family `SIGNALS`, research builds only)
replaces the AI step's call to the broadcast at `0xf16de7`
(`e8 b4 f5 ff ff`): it calls the broadcast first, returns its result, then
reads the three lists on the viewed match's worker only, bounded (length
within capacity, capacity at most 64), at most once per team per tick, and
logs only changes. Installed last, on its own: a differing site turns off
the probe and nothing else. Game 0.6.5 gets no probe (placeholder table,
family disabled).

**Injection (step 1, 1.7.1-dev5, built 2026-11-03, not yet seen in
game):** at the same site, after the game's broadcast ran for a
teammate's brain, append one entry to that brain's queue A: due = now +
15, index = the controlled athlete's index, 15 at +0xc, tag 4, target =
the hovered enemy's entity id, zeros elsewhere; only when `len < cap`
(never allocate), else drop and log. One call is therefore up to four
entries, one per teammate. The mapping from brains to the mod's players
is found at run time: the controlled athlete's step publishes the SDK
roster (player id, side, lane, champion entity id); each brain and its
player state are scanned once, read-only and bounded, for those entity
ids; a field that holds a distinct roster id in at least five brains with
consistent sides names the controlled athlete's brain, or it is the one
left over by elimination; the log shows the evidence (`PING map …`) and
nothing is sent while it is unresolved. Nothing else is written;
teammates stay entirely the game's. Upkeep per game patch: one hook site,
nine offsets and two scan bounds, the tag table, all in the version
folder.

**Step 1 in game (1.7.1-dev5-research, one match, 2026-11-03):** the
mapping resolved at about five seconds; six taps each wrote four entries,
byte-identical in layout to the game's own BattleHelp entries; the game
removed them between steps as it does its own; no teammate reacted. The
dev6 probe then showed why (section 10).

## 8. Probe quirks (seen in game)

- `SIGNAL controlled` never logged: the identity word is not the actor id.
- A transient "third team planner" at tick 5..6 of every match; both real
  teams were logged to the end.
- `SIGNAL player` lines for only two players per team: a first-sight
  bookkeeping slip, not a hook fault.

## 9. Not known yet

- Whether the index at +8 is the sender (likely) or the receiver; the
  injection settles it. **2026-11-03:** read from the code by the dev5
  coder as the sender (the delivery checks every entry against the
  brain's own state); the dev5 watch lines are the test in game.
- Whether the mapping from brains to the mod's players resolves in game
  (dev5 logs `PING map resolved` or `unresolved`). **2026-11-03:** it
  did, by the field `player + 0x8` (the owner's current entity id), but
  dev5/dev6 lost it in a second match; dev7 rebuilds it every match.
- Whether the controlled athlete's own planner still sends calls while
  the mod controls its input, and whether that clashes with a human ping.
  **2026-11-03 (seen in game):** it does, Mia, Battle, BattleHelp and
  BattleStop calls; nothing in an entry tells the two apart.
- The meaning of the per-sender tag masks in the receive gate, and of the
  value at `+0x20`. **2026-11-03:** the masks are by sender slot over the
  receiver's mode (section 4); the mode's meaning and `+0x20` stay open.
- Whether teammates ever reply with Ok or Reject. **2026-11-03 (seen in
  game, dev6):** one Ok in two matches, from a slot-4 player.
- What kinds 1 and 2 of the match environment are (the game's other
  simulations, a guess), and whether the scene opener called for the
  player's own call makes a teammate move (dev7 tests it).
- Why side-1 champions get a new entity id on respawn while side-0 ones
  kept theirs in both dev6 matches.

## 10. The delivery probe (1.7.1-dev6-research, seen in game, two matches, 2026-11-03)

Five more call swaps inside the delivery: the late drop (`0xe75c32`),
the gate call (`0xe75d27`), the two handler calls (`0xe65f25`,
`0xe660ef`) and the scene opener call in the handler (`0xe67580`); the
join counters (`CallJoinStats`, eight u64 at `brain + 0x12c0`: recv,
ignored, in_battle, no_help, too_far, low_hp, bail, join) read at each
step. All sites and the counters' order verified against the executable.

**Seen in game:** no late drop in two matches (delay 60 for everyone);
mode 0 at every gate, every call accepted; 911 calls reached the handler
(363 Mia, 175 Battle, 131 BattleStop, 75 HideLine, 72 BattleHelp, 52
BattleDive, 34 Cancel, 8 HideLineToo, 1 Ok); **no call opened a scene**.
The join counters still rose, 8 to 12 received per brain, by the team
handler's own scene calls.

**Read from the code:** the handler's Mia/Battle/BattleDive/BattleHelp
cases first call slot `0x40` of the AI environment trait object that
starts the sim (`sim = [pair]`, `[sim]` data, `[sim + 8]` vtable). The
method returns a kind: the vtables `0x40a2530` and `0x40c9f28` (type size
0xf410) return 0, `0x40a33a0` returns 1, `0x40a3f98` and `0x40c9898`
return 2, and `0x40398e0` wraps an inner object. The handler ends at once
on kind 0; the step caller `0xf19810` makes the same call and, on 0,
falls back to the entity table at `sim + 0x238 + side*0x28 + index*8`.
The viewed match is kind 0 (0 of 911). The team handler calls the scene
opener `0xe662f0` itself (`0xe3e241`), with a reason byte, a sender and a
target, and the opener applies the join rules and counts them. Reason
codes: 1 Battle, 2 BattleHelp, 3 BattleDive (the dev6 build's labels had
Battle and BattleDive swapped; superseded).

**Design consequence:** step 2 (dev7) keeps the inbox write (so the
game's own late, gate and handler steps still run on the call) and, when
the player's own BattleHelp is heard, calls the scene opener for that
teammate with reason 2 and the target, exactly as the handler does in
kinds 1 and 2. The join counters logged at the next steps are the
teammate's answer.

## 11. The scene call (1.7.1-dev7-research, seen in game, three matches, 2026-11-03)

**Seen in game:** the owner mapping resolved in every match (slot 2 on
side 0 twice, side 1 once; each match runs on a new worker thread, which
is why dev5/dev6 lost it). Thirteen taps, 50 inbox entries, every one
heard, 46 scene calls made by the mod through `0xe662f0`; no crash, no
late drop, no full queue. The four missing calls belong to a tap repeated
six ticks later; two requests reached three teammates because the fourth
was dead. Match kind 0 and mode byte 0 everywhere (1695 gate lines).

The game judged every call. Matched to the teammate's distance from the
target at the tap (the decision comes about 18 ticks later):

| Distance | Counter that rose |
| --- | --- |
| 17 to 342 units | `join`, or `in_battle` when already fighting |
| 343 to 361 units | `too_far` and `join` together (two cases) |
| 430 units and more | `too_far` |

`ignored` rose three times at a tap made while the enemy backed into
their base; `bail` twice (69 and 289 units). The author could not tell
by eye whether a joined teammate changed course: close teammates were
mostly heading into the same fight anyway, and the distance watch lines
cannot separate the two either.

Mod quirk: the mod recognised its own call by sender slot, kind and
target. The controlled athlete's own planner also sends BattleHelp about
the enemy it fights, so inside a request window those were counted and
acted on as the player's (requests 4, 9/10). Harmless, but the record is
muddied; dev8 marks the mod's entries.

**Read from the code, the scene opener `0xe662f0` (0xfb1 bytes):**

1. Looks the target up through slot `0x208` of the AI environment; a
   null result ends the call with nothing counted. Then `recv` rises.
2. `0x10ad560` true: `ignored`. That function tests the target's
   position against fixed coordinate boxes (the base zones; the
   BattleDive flag selects other boxes). Not a cooldown: the earlier
   guess is superseded.
3. `[brain + 0x16a0]` (u32, the current scene kind) at most 1:
   `in_battle`, and the call goes on (it may retarget inside the fight).
4. The current scene kind not in bit mask `0xf26` over kinds 0..11:
   `no_help` (the teammate's objective type does not allow leaving it).
5. `isqrt(dx² + dy²)` between the target and the teammate's unit
   (`+0x658`, `+0x660`), divided by the unit's move speed `[unit + 0x638]`,
   above `5 × [state + 0x12f8]` (= 300 ticks, 5 s of travel): `too_far`.
   The limit is a travel time, which is why the band above is fuzzy.
6. `[unit + 0x668] × 100 / [unit + 0x620]` (hp percent) under 50:
   `low_hp`.
7. The plan function `0xf02990` false: `bail` (accepted, could not plan).
8. Otherwise: an **Ok reply** (tag 9, second word 0) is pushed onto
   queue C (`brain + 0x1eb8` cap, `+0x1ec0` ptr, `+0x1ec8` len, 24-byte
   entries), `join` rises, and `0xe72740(brain, kind 4, target position,
   ...)` sets the scene: it clears the old one (`0xe73a10`) and writes the
   block at `brain + 0x15f0` (`+0x15f0` = 1, `+0x1638` world tick,
   `+0x1648`/`+0x1650` position, `+0x1658` distance, `+0x1688` a byte,
   `+0x1689` = 0x4ff0000). It returns without writing when the distance
   is under 250 units (`0x3d091`), and takes another branch under 350
   (`0x55731`); what those two limits mean is not read yet.

So "join" is the teammate's AI accepting the target as its objective and
answering Ok; there is no further switch. Whether a join that is counted
also writes the scene block is what the dev8 probe logs.

Signal word `+4` (entry `+0x14`) is read only by the Mia case (enemy
index); the BattleHelp case reads the target (`+8`) and passes the extra
word on; the gate and the compatibility check read the tag and sender.
A marker there is therefore free for BattleHelp entries.

**Guess:** the two distance limits in the set-scene function are "already
there" (under 250) and a short-approach branch (250 to 350). **Not
known:** the meaning of `bail`'s two cases, and the mode byte (never
other than 0 in a viewed match).

**dev8 plan (one probe build, one test run):** mark the mod's entries at
`+0x14`; log the scene block, the scene kind, queue C and the eight
counters before and after each mod scene call, with the teammate's hp
percent and move speed; show the answer as a glyph by the teammate's
marker for a moment so the player knows whom to watch.

## 12. The scene call measured, and the strategy lever (1.7.1-dev8 and dev9, seen in game, 2026-11-03)

**dev8, seen in game (one match, side 0):** 28 judged calls: 15 too
far, 6 in battle, 3 join, 2 no help, 2 none (the target lookup returned
null). Every join queued the Ok reply (tag 9) onto list C, no other
answer did. Every join set the scene kind at `brain + 0x16a0` to 1 with
the target's entity id at `+0x16a8` (from 7, 13 and 13); two "in battle"
calls did the same from kind 0 (the 150-unit rule in section 11's
reading, confirmed by the coder: `0xe66566`). The scene block at
`+0x15f0` was written once, for a join at 362 units, and not for the
closer joins, matching the 250-unit rule. So five of 28 calls changed a
teammate's objective to the ping's target: the planner-level proof. The
mod's mark at entry `+0x14` (`0x4c545047`) told its own entries apart; the
athlete's own planner's calls never matched again.

Two gaps found: a brain's inbox has capacity 0 until the game's own first
push, and the mod never allocates, so a tap before that is dropped
("queue full 0/0", 18 lines; the first request expired unsent at 14 s).
And the reply marks, drawn at 11 px, were not noticed in a fight.

**Why 3 of 28 is the wrong reading.** Five of seven taps moved at least
one teammate onto the target; the "too far" answers are teammates more
than five seconds' travel away. The AI's own fight calls get the same
treatment (dev6: 9 joins of 45 and 4 of 46). This channel is "help me
with this enemy" and nothing more: lane and objective orders never
travel on it (section 5). A League-level ping system, the author's
direction, needs a second lever for macro.

**dev9, seen in game (one match, side 0): the official interface's live
strategy.** `StableSim::strategy_get_json(team, path)` and
`strategy_set_json(team, path, json)` (sdk/mod-api-stable/src/sim.rs),
called from the match hook (changes made inside the AI step are inert,
by the SDK's own note).

- `get(team, "")` returns the whole document as JSON. Fields are plain
  names, values bare strings, variants with data are objects. Read from
  the executable's serde names and confirmed by the document:
  `focused` Top|Bottom|All; `early_jungle` GrowthAndCover|Ganking|
  CounterJungle; `early_serpen` and `early_serpen_top` Must|Flexible|
  Giveup; `object_buildup` Gather|Flexible|Split{position};
  `object_battle` Poking|Initiating; `morgard_use` Gather|Split14{position}|
  Split131{position1, position2}; `tower_press` Dive|Poking;
  `morgard_defense` Battle|Gather; `object_finish` KillPriority|
  BattlePriority; `minion_wave` WavePriority|JoinPriority; `game_finish`
  Stable|Aggressive|Flexible; `dm_tactic` Standard (other values unread).
  The match's own documents: side 0 focus Bottom, CounterJungle, Serpen
  Must, Gather, Poking, Morgard Split131 {Jungle, Mid}, KillPriority,
  Aggressive; side 1 focus All, Morgard Gather, BattlePriority, Flexible.
- `set(team, "focused", "\"All\"")` was accepted on the first attempt,
  every time (All, Top, Bottom; `object_buildup` Flexible). The game kept
  each change for the whole 20 s watched and never wrote it back; it
  restored the manager's value when the match ended (one "game changed"
  line, at the battlefield exit). So the AI reads the same copy the
  setter writes, and a change does not leak into the next match.
- After `focused` = Top, three teammates moved together from near the
  base (y about 850) to the far end of one map edge (y 66 to 205) within
  20 s; after Bottom, no clear move in 20 s. The author felt the team's
  behaviour change but had no feedback on screen. One observation, not
  proof.
- The game's in-game strategy panel (`ingame.strategy_info`, text keys
  `asset/base/text/strategy?team.<field>.<option>.name`) shows the team
  strategy; whether it follows a live change is not checked.

**Not read:** how and when the team AI consults each field. By name,
`early_*` are phase-bound; `focused` looked continuous; `object_*` and
`morgard_*` apply when an objective is up. **Guess** until read or
played.

**Reply marks, seen in game:** with the dev9 diagnostics every answer was
stored, found in the unit list, in its window and on screen, the two
clocks within a tick; at radius 18 and 3 s the author saw the clocks and
checks. dev8's marks were drawn and not noticed.

**Superseded 2026-11-03 by `docs/investigation-team-ai.md`:** three
anchors above are corrected there. The "team handler" `0xe381b0` is one
player's planner step; the team rules run inside it, per player. The
"scene kind" at `brain + 0x16a0` is the tag of the player's big plan
(Battle 0/1, PassiveLine 3, PassiveJungle 7, AttackNexus 13, …). The
no_help mask `0xf26` is tested over the plan index (raw − 2), not the
raw value. The per-player objective (3 bytes at `brain + 0xe45`) and its
setter `0xec3820` are the per-player lever this document lacked.
