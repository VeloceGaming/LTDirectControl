# Investigation: the team-level AI, and which pings it can carry

Written 2026-11-03. Question from the design session: the inbox lever
(`docs/investigation-ai-signals.md`, sections 10 to 12) carries only
fight calls, and the strategy document is team-wide. Which per-player
objectives, objective plays and enemy knowledge does the game's AI hold,
and can a ping reach them?

Every finding is labelled:

- **Seen in game**: someone played it (build and game version given).
- **Read from the code**: static reading of the 0.7.0 executable
  (sha256 `d678e012…7d30`; addresses are RVAs of that build only).
- **Guess**: plausible, not tested.
- **Rejected**: tried and wrong.

Nothing in this document was played. Every new claim is read from the
code or a guess; "seen in game" lines quote earlier documents.

## 1. Short answer

- **Read from the code.** The team rules that pick a player's objective
  run inside that player's own brain: the function the earlier documents
  call "the team handler", `0xe381b0`, is one player's planner step
  (`plan/handler.rs`), and it calls the team plan
  (`plan/team_plan/*.rs`) with that player's own brain. The players also
  read a shared per-team block (TeamScenes and per-player records,
  section 5) that no planner function writes. **Guess:** its owner is
  `MobaBrain.blackboard` (formatter `0x18e9fb0`: `agents`,
  `blackboard`, `prev_champion_cache`, …), the one team-level object
  found; its writer was not read.
- **Read from the code.** A player's AI holds two layers:
  - the **objective** (3 bytes at `brain + 0xe45`): Morgard, Serpen,
    Defense, DefenseLine(lane), Nexus(lane), Repair, Gank(lane),
    Dive(lane), PressTower(lane), ComebackPick(lane), or none. One small
    setter writes it, `0xec3820`.
  - the **big plan** (`brain + 0x16a0`, 0x400 bytes): what the player is
    doing now (PassiveLine, PassiveJungle, Battle, LineGanker,
    SerpenHuntAndPoke, AttackNexus …). The earlier documents call its tag
    "scene kind". It is rebuilt from the objective by `0xe52ab0`.
- **Read from the code.** The objective setter is the per-player entry
  point a ping is looking for: one call, no allocation (unless tracing is
  on), arguments the step hook already has. Writing an objective does not
  make it stick: the team plan rewrites objectives at many sites every
  decision. A ping would have to set it and then keep it, or win against
  the cascade's own rules (section 4).
- **Read from the code.** Enemy knowledge is a plain per-player table,
  `VisionTrackingState` at `brain + 0xbf0` (last seen position, tick,
  distance, hp, and the Mia call ticks). The team plan refreshes the
  entries of visible enemies each decision and leaves the others alone,
  so a believed position written there would last until the enemy is
  seen again (section 6).
- **Read from the code.** The strategy is read fresh from the match at
  every decision (about 60 call sites), never cached in a brain. That
  agrees with dev9's live change being kept (section 7).
- **Read from the code.** The objective kinds of the `Signal` enum
  (15 to 27) are not inbox calls. The team plan writes them into the
  brain's own `TeamPlan.signals` list and reads them back; no path that
  carries them to another brain was found (section 8).

## 2. Map of the planner (read from the code)

The executable keeps the planner's source file names for its error
messages. Matching each function to the files it names gives a map
(script `srcmap.py`, scratch only). The crate `game-ai` has four layers:

| Layer | Files | What it is |
| --- | --- | --- |
| Handler | `plan/handler.rs` and `plan/handler/{assign, auction, engage, lane_ops, modes, obs, relays, scene_feedback, signal}.rs` | One player's planner step. `0xe381b0` is `plan/handler.rs`. |
| Team plan | `plan/team_plan.rs` and `plan/team_plan/{audit, cascade, gank_gates, obj_contest, objective_discipline, objective_handlers, objective_helpers, opportunities}.rs` | Team rules, run inside each player's step. |
| Big plan | `plan/big_plan/{active_recall, attack_nexus, battle, death_battle, defense_nexus, epic, epic/hunt_and_poke, fight_model, line_gank/ganker, passive_jungle, passive_line, serpen, serpen/hunt_and_poke, single_battle, single_line}.rs` | A player's current plan. |
| Sub plan | `plan/sub_plan/{act_gates, attack_nexus, battle, battle_common, death_battle, defense_nexus, epic_poke, hide, jungle, line_defense, line_safe, line_wait, obj_contest, objective_check, objective_hunt, recall, serpen_poke}.rs` | The step inside a plan. |

Call tree of one decision (read from the code):

| Function | File | Role |
| --- | --- | --- |
| `0xf16d60` | | Per-player AI step (known anchor). |
| `0xe381b0` | `plan/handler.rs` | Planner step. Its first act is the environment kind (slot `0x40`): kind 2 continues at `0xe3825e`, kind 1 at `0xe38353`, kind 0 (the viewed match) at `0xe3872b`. Where the three paths join again was not read. |
| `0xec9270` | `team_plan.rs`, `team_plan/audit.rs` | Vision update and audit (section 6). |
| `0xec4090` | `team_plan.rs`, `cascade.rs`, `objective_handlers.rs` | The cascade: decides this player's objective. Called at `0xe404c5` and `0xe405c6`. |
| `0xeb9e60`, `0xebf4d0` | `cascade.rs` | The two cascade bodies, called from `0xec4090` (`0xec8214`, `0xec82f0`). |
| `0xeb7060`, `0xeb7770` | `objective_handlers.rs` | Objective handlers ending in Serpen and Morgard (section 5). |
| `0xec3820` | `team_plan.rs` | Objective setter (section 4). |
| `0xe52ab0` | `handler/assign.rs` | Builds a big plan from the objective (section 4). |

**The brain.** The structure passed as "brain" is the agent
`AgentVerHamster` (Debug formatter `0xf57d70`). Field offsets from that
formatter agree with the known anchors (`next_input_tick` at `+0x47c0`):

| Offset | Field |
| --- | --- |
| +0x0 | `plan_system` (the planner; holds everything below `+0x2c70`) |
| +0xe8 | `TeamPlan` (section 4 table) |
| +0x16a0 | big plan (0x400 bytes) |
| +0x1aa0 | the big plan to return to after a fight (0x400 bytes, -1 = none) |
| +0x2c80 | `small_action` |
| +0x3c70, +0x3c88 | `events`, `deferred_events` (the broadcast reads these) |
| +0x47c0 | `next_input_tick` |

**Strings.** Many of the planner's own labels are Korean. The 26 battle
end reasons at `0x401ddc8` read, in English: unknown, target lost, no
enemies left, timeout, objective priority, chase outside siege
direction, fountain, cannot chase, tower, wave, dive, resolver lost,
ladder disadvantage, re-entry gate, spawn retreat, BlockB, turn and
flee, swap: death, swap: mode guard, swap unknown. A search for plan
names in ASCII alone misses these.

## 3. Name tables found (read from the code)

**The big plan enum** (Debug formatter `0xe766c0`, jump table
`0x4010114`). 0x400 bytes: a u64 tag at +0, the payload from +8. The
variant that carries data, `Battle`, keeps its own inner tag in the same
word (raw 0 = no target, 1 = target id at +8); every other variant is
raw value = index + 2:

| Raw | Variant (payload fields) |
| --- | --- |
| 0, 1 | Battle(BattlePlan) |
| 2 | ForcePassive |
| 3 | PassiveLine(line, signals, in_recall, commit, recall_committers, flee…, hold_wave, waiting, split_role, receiving, push_basis, regroup_now, cancel_low_hp_sent, hide_line_too_sent, join_gank) |
| 4 | SinglePlanLine(line, signals, in_recall) |
| 5 | SinglePlanBattle(main_goal, sub_goal, signals, support_target, with_dive, dive_tower, start_tick, help_called, well_runaway, main_objective) |
| 6 | DeathMatchBattle |
| 7 | PassiveJungle(jungle, team, player_team, signals, last_lead_action_tick, cj_meet_latch, cj_seen_alive, cj_ambush, cj_leave_until, cj_meet_decided, invader, invade_line, invade_steals, clear_recall) |
| 8 | ActiveRecall(to_fountain) |
| 9 | (unused: Battle's niche) |
| 10 | LineGanker(line, wait_limit, signals, phase, settled_tick, dbg_cancel, roam_from, exposed_at_start, deadline_said, exposed_screen_tick) |
| 11 | EpicHuntAndPoke(flee, flee_threats, rally_passed, hunt_recall) |
| 12 | SerpenHuntAndPoke(flee, flee_threats, rally_passed, hunt_recall) |
| 13 | AttackNexus(team, line) |
| 14 | DefenseNexus(team) |

"Epic" is the Morgard side of the code (**guess** from the names: the
objective Morgard leads to EpicHuntAndPoke, Serpen to SerpenHuntAndPoke).

**The objective enum** (Debug formatter `0xed9170`, jump table
`0x4018754`), 3 bytes, tag 0xff = none:

| Tag | Objective | Bytes 1, 2 |
| --- | --- | --- |
| 0 | Morgard | phase, with_battle |
| 1 | Serpen | phase, with_battle |
| 2 | Defense | |
| 3 | DefenseLine | lane |
| 4 | Nexus | lane |
| 5 | Repair | |
| 6 | Gank | lane |
| 7 | Dive | lane |
| 8 | PressTower | lane |
| 9 | ComebackPick | lane |

Phase (formatter `0xeda000`, tables `0x4018450`/`0x4018470`): 0 None,
1 Setup, 2 Assemble, 3 Hunt.

**The goal enum** (`plan/handler/modes.rs`, formatter `0xe80830`): Line
(line), Jungle (camp, team), Epic, Serpen, Nexus (team), Battle (focus),
Recall. Mode names (`0x4009990`): PassiveLine, PassiveJungle, Epic,
Serpen, ActiveRecall, Nexus. Short trace labels (`0x40097f8`): Force,
Line, Jungle, ARecall, Battle, Ganker, EpicHP, SerpHP, AtkNex, DefNex;
sub plans (`0x4009832`): Other, LDef, LSafe, LWait, Recall, Hide, Hunt,
Poke, ObjC. Where these two sit in the brain was not read.

**TeamScene** (formatter `0x18d2000`, 0x50 bytes): target +0x0, anchor
+0x10, owner +0x20, since_tick +0x28, arrive_at +0x30, quorum +0x38,
members +0x40, line +0x45, kind (byte) +0x46, intent +0x47, phase +0x48.
**Guess** (from a layout match, section 5): TeamScenes live in a shared
per-team block, not in a brain.

## 4. Question 1: per-player objectives

### The big plan is what the earlier documents call the scene kind

**Read from the code.** The u64 at `brain + 0x16a0` is the tag of the big
plan enum above. Four checks agree:

- The scene opener's "in battle" rule (`kind <= 1`) is "the plan is
  Battle". Its 150-unit rule (`0xe66566`) turns raw 0 (Battle, no
  target) into raw 1 with the target's id at `+0x16a8`.
- The values seen in game (0, 1, 3, 7, 13) read as Battle, Battle,
  PassiveLine, PassiveJungle, AttackNexus.
- The no_help test (`0xe667be`..`0xe66814`): raw 3 and 4 (PassiveLine,
  SinglePlanLine) may always help; raw 7 (PassiveJungle) may help when
  the two words at `+0x16e8`/`+0x16f0` are equal; otherwise the index
  (raw − 2, Battle = 7) must be in mask `0xf26`, bits 1, 2, 5, 8, 9, 10,
  11. So a teammate in LineGanker, EpicHuntAndPoke, SerpenHuntAndPoke or
  AttackNexus may help; one in ForcePassive, SinglePlanBattle,
  DeathMatchBattle, ActiveRecall, DefenseNexus or its own Battle answers
  `no_help`. **Superseded 2026-11-03:** section 11 of
  `investigation-ai-signals.md` read the mask as over the raw kind.
- `0xe37db0` (the plan trace) names the old and the new plan through the
  same table.

The anchor's name "scene kind" is therefore superseded: it is the plan.
The block at `+0x15f0..+0x1690` written by set-scene `0xe72740` lies
just before the plan and was not read further.

### Who writes the big plan

**Read from the code.** Direct stores to `+0x16a0`:

| Site | What it writes |
| --- | --- |
| `0xe662f0` (scene opener) at `0xe66566` | Battle with a target (raw 1). |
| `0xe74980` (`assign.rs:1131`) at `0xe74a07` | Restores the stashed plan from `+0x1aa0` after a fight (memcpy 0x400), then may rebuild through `0xe52ab0`. |
| `0xe381b0` at `0xe3ea7c` | PassiveJungle with an empty payload. |
| `0xf33630` | Clone of the brain. |

All other writes are whole-plan copies (memcpy 0x400) inside the handler
after a builder fills a stack copy: about 14 sites (`0xe3c0c0`,
`0xe3d3ca`, `0xe3d78a`, `0xe4011c`, `0xe40651`, `0xe40826`, `0xe408c7`,
`0xe431e1`, `0xe4364c`, `0xe4381d`, `0xe4469b`, `0xe446f8`, `0xe4491e`,
`0xe45424`), each followed by a `plan` trace (`0xe37db0`, tag
`0x6e616c70`). Most are preceded by `0xe52ab0`.

### The builder: assign `0xe52ab0`

**Read from the code.** `0xe52ab0(out, brain, ctx, sim, player, pair)`
(`assign.rs:140..895`, 13.8 KB) switches on the objective byte at
`brain + 0xe45` and writes a 0x400-byte plan plus a reason byte at
`out + 0x400`:

| Objective | Plan built |
| --- | --- |
| 2 Defense | DefenseNexus (own side), reason 3 |
| 3 DefenseLine(lane) | PassiveLine(lane), reason 4 |
| 4 Nexus(lane) | AttackNexus (enemy side, lane), reason 2 |
| 7 Dive(lane) | PassiveLine(lane), reason 5 |
| 5, 6, 8, 9, 0, 1 | longer paths (env kind check, Gank, PressTower `0xe52d76`, ComebackPick `0xe53841`) not read in full |

It reads the strategy three times (section 7) and allocates (vectors in
the plan payload). It is the step that turns an objective into a plan,
not an entry point for a ping.

### The objective and its setter `0xec3820`

**Read from the code.** `TeamPlan` sits at `brain + 0xe8` (Debug
formatter `0xe801a0`, 53 fields). Offsets below are brain offsets
(`TeamPlan` offset + 0xe8):

| Brain offset | Field |
| --- | --- |
| +0xe8 | `obs` (TeamObs) |
| +0x840 | `nexus_basis` |
| +0x860, +0x870, +0x880 | `def_depart`, `siege_lock_h55`, `repair_owner` |
| +0x8e0, +0x920 | `serpen_lock`, `morgard_lock` |
| +0xac0 | `signals` (Vec, 24-byte entries; section 8) |
| +0xad8 | `pos_ring` (own position history) |
| +0xb18 | `defender_keep_scene` |
| +0xb58, +0xb78 | `serpen_scene`, `morgard_scene` |
| +0xb98 | `press_timeout` |
| +0xbf0 | `vision` (VisionTrackingState, section 6) |
| +0xd08 | `obj_spawn` (ObjectiveSpawnState) |
| +0xd28, +0xd30, +0xd38 | `gank_start_tick`, `press_tower_start_tick`, `comeback_pick_start_tick` |
| +0xdb0 | `obj_deadline` |
| +0xdc0, +0xdd0 | `obj_arrive_at`, `obj_quorum_k` |
| +0xe32, +0xe34, +0xe36 | `window_intent`, `assemble_arrived`, `assemble_departed` |
| +0xe30 | `hunt_call_pending` |
| +0xe3f | `scene_publisher` |
| +0xe40 | `press_follower` |
| +0xe43, +0xe44 | `repair_line`, `press_signal_line` |
| +0xe45 | `objective` (3 bytes) |
| +0xe48 | `first_serpen_state` |

Offsets come from the formatter's value array; a few were checked
against code (objective, signals, pos_ring, vision).

`0xec3820(team_plan = brain + 0xe8, objective u24 in edx, &Location)`:

1. If tracing is on (`team_plan + 0x748`), records the change (allocates
   a 9-byte name, "objective").
2. Clears side fields by the new tag (`0xec3a28`..`0xec3a9c`; offsets
   inside `TeamPlan`):

   | New objective | `def_depart` +0x778 | `press_follower` +0xd58 | `repair_line` +0xd5b, `repair_owner` +0x798 |
   | --- | --- | --- | --- |
   | Defense (2) | kept | cleared | reset |
   | PressTower (8) | cleared | kept | reset |
   | Repair (5) | cleared | cleared | kept |
   | none (0xff) and the rest | cleared | cleared | reset |

3. Writes the 3 bytes at `+0xd5d` (= `brain + 0xe45`).

The handler saves `press_follower` and `repair_line` before the cascade
and writes them back after it (`0xe4046f`, `0xe40623`).

No environment call, no kind check, no pointer kept, nothing shared.

Callers (`callers.py`): the handler (6 sites), `0xe4dc10`, `0xe4e2d0`
(`handler.rs`), relays `0xe64fd0`/`0xe658a0`, the receive handler
`0xe67300` (2 sites), objective handlers `0xeb7060`/`0xeb7770`, and the
cascade bodies `0xeb9e60`/`0xebf4d0` (about 60 sites between them).
Objectives seen as constants at the call sites: `0x10301` Serpen Hunt
with battle (`0xeb768a`), `0x10101` Serpen Setup with battle
(`0xeb75ed`), `0x10300` Morgard Hunt with battle (`0xeb7d55`), `0x103`
Serpen Hunt (`0xebfcaf`), `0x203` DefenseLine(Bottom) (`0xec0bde`), 2
Defense, 5 Repair, `(lane << 8) | 4` Nexus, `| 6` Gank, `| 8`
PressTower, `| 9` ComebackPick, 0xff none.

### Two inbox paths set an objective

**Read from the code**, receive handler `0xe67300`:

- **HideLine / HideLineToo (tags 6, 7, case `0xe67353`).** After the
  lane-ops part, at `0xe6782d`: if the receiver has no objective, its
  roster slot is 0, 2, 3 or 4 (not the jungler), and the call's lane is
  that slot's own lane (table `0x202010000`: slot 0 lane 0, slot 2
  lane 1, slots 3 and 4 lane 2), it sets the objective Gank(lane)
  (`0xe67872`) and stamps `gank_start_tick`. **This path has no
  environment kind check** (the kind-0 exit found in dev6 is in the Mia,
  Battle, BattleDive and BattleHelp cases only). **Guess:** the inbox
  HideLine therefore works in the viewed match, and a laner told
  "HideLine" on its own lane joins a gank. Not seen in game; dev6 counted
  75 HideLine calls heard but did not log objectives.
- **Cancel (tag 11).** Clears a Gank, Dive or ComebackPick objective
  (mask `0x2c0`), but only when the environment kind is not 0
  (`0xe67472`). Not in the viewed match.

### Verdict, question 1

- **The objective setter `0xec3820`: callable from a hook: yes, with a
  caveat.** Arguments: `brain + 0xe8`, a 3-byte objective, a pointer to
  a 24-byte Location (any static the mod owns; only read when tracing is
  on). No allocation unless tracing is on (`brain + 0x830` non-zero), no
  environment call, nothing shared, plain byte writes. It must run
  during the target teammate's own AI step, on the viewed match's
  worker, with that teammate's brain (the dev7 pattern for the scene
  opener).
- **The caveat, persistence.** The step hook at `0xf16de7` (the
  broadcast call) runs before that step's handler and cascade. So an
  objective set there is judged by the cascade in the same step: the
  cascade calls the same setter at about 60 sites, many of them `0xff`
  (clear). Whether the objective survives depends on the cascade's rules
  for it, not read site by site. A probe must read `brain + 0xe45` and
  the plan tag `+0x16a0` at the teammate's next step, not right after
  the call. **Guess:** objectives the cascade itself would pick (Gank on
  a lane where the jungler ganks, Serpen when Serpen is up) survive;
  others are cleared at once. Writing the 3 bytes directly is
  equivalent, with the same caveat.
- **The plan builder `0xe52ab0`: callable: no** (allocates, many
  arguments, and the handler commits its result itself).
- **The inbox HideLine: callable already (the existing inbox write):
  unknown, likely yes.** It is the one inbox call that reaches an
  objective without a kind check.

## 5. Question 2: objective plays (Serpen, Morgard)

**Read from the code.** A Serpen or Morgard play is an objective
(Morgard/Serpen with a phase Setup, Assemble or Hunt and a with_battle
flag) in each member's own brain, chosen by its own cascade, plus shared
TeamScenes. The objective handlers `0xeb7060` (ends in Serpen) and
`0xeb7770` (ends in Morgard), `objective_handlers.rs:48..640`, run in
this order for one player:

1. **Mode gate.** The receiver's mode byte (`[pair + 8] + 0x38`) must be
   0, 5, 7 or 8 (mask `0x1a1`); otherwise the objective is cleared
   (reason byte `0x20` at `team_plan + 0xc8`).
2. **Nexus first.** `0xeb62c0` (`objective_handlers.rs:1888`, reads
   `game_finish`) may return a lane: objective Nexus(lane).
3. **Defense next.** `defense_nexus.rs` tests `0xe9df90`, `0xe9e8e0`,
   `0xe9e7f0`, `0xe9c6d0`, then `0xfd1650` and `0xe9fb10`: objective
   Defense (2) and a DefenseNexus team call (tag 22) on
   `TeamPlan.signals`.
4. **Serpen.** Only in environment kind 0: the handler calls slot `0x40`
   and panics on any other kind (`0xeb7329` → `0xeb7714`), and uses the
   returned kind-0 object's Serpen fields (`+0x1d0`, `+0x1d8`, `+0x1e0`).
   - Serpen dead and respawning more than 15 × delay ticks away (delay
     = `[[pair+8]+8]+0x12f8`, 60 in game, so 15 s): objective cleared
     (reason `0x21`, `0xeb74a7`).
   - Otherwise, after the Serpen entity is looked up (env slot `0x208`,
     an hp test) and two times are compared with 4 × delay (4 s), it
     sets Setup with battle (`0x10101`, `0xeb75ed`). When one of the five
     per-player records in the shared block (`+0x808 + k*0x20`) holds 3,
     a further time must exceed 2 × delay first (`0xeb75bf`).
   - Hunt with battle (`0x10301`, `0xeb768a`) when phase is already 1
     and `0xeb9540` or `0xed2c50` (`serpen.rs`) agree; it pushes a
     SerpenHunt team call (tag 17) on `TeamPlan.signals`.
5. After setting Nexus(lane) or Defense, a player whose
   `scene_publisher` byte (`brain + 0xe3f`) is 1 scans the shared
   TeamScene list: if no other player owns a scene of kind 2 (for
   Nexus) or kind 1 (for Defense), it pushes AttackNexus(lane) (tag 21,
   `0xeb741d`) or DefenseNexus (tag 22, `0xeb7457`) onto its own
   `TeamPlan.signals`. **Guess:** that is how a publisher asks for a
   team scene to be opened, and the list's consumer turns it into a
   TeamScene (consumer not found).

**The shared block.** The handlers' last stack argument is a context
whose word at `+0x10` points to a per-team block, `side × 0xea0` bytes
apart: a Vec of 0x50-byte entries (pointer `+0x2b8`, length `+0x2c0`)
and five 0x20-byte per-player records from `+0x808`. **Guess:** the
entries are TeamScenes (same size; the code tests a byte at +0x46 and
compares +0x20 with the player's index, which are TeamScene's `kind`
and `owner`). **Read from the code:** over 300 planner functions index
this block by `side × 0xea0`, and none of them stores to the Vec or the
records (the step caller `0xf19810` only reads it); the writer is
outside the planner step. **Guess:** `MobaBrain.blackboard`. TeamPlan's
`scene_publisher` and `sync_out` are the leads from the brain side.

**Who is gathered.** Each player's own cascade decides; there is no
member list given out. The per-team TeamScene carries `members`,
`quorum`, `arrive_at` and the brain carries `obj_quorum_k`,
`obj_arrive_at`, `obj_deadline`, `assemble_arrived`,
`assemble_departed`, `obj_enemy_gather_since`. **Guess:** the scene
lists the members and the quorum, and each member's cascade joins when
it is in the list.

**Distances and times.** Rules read so far are in multiples of the delay
word (60 ticks = 1 s): 15 s to the Serpen respawn, 4 s and 2 s on two
travel or wait times. The fight call's 5 s travel rule (scene opener)
is a separate rule. **Not read:** the meaning of the two compared times
(`team_plan + 0xb8`, `+0xc8`). **Superseded 2026-11-03** (section 14):
the two words are read through another pointer, not the TeamPlan.

**What ends a play.** The objective is cleared (0xff) by the mode gate,
by the 15 s rule, and at many cascade sites (section 4). **Not read in
full.**

**Strategy fields in these handlers** (section 7 table): `early_serpen`
at `0xeb1730`, `0xeb2510`, `0xeb4a00`, `0xec9270`; `early_serpen_top` at
`0xeadc20` (`obj_contest.rs`); `object_buildup` at `0xee8960` and
`0xee9c40` (the two HuntAndPoke plans), `0xed4520` (`serpen.rs`),
`0xec4090`; `object_finish` at `0xefbfe0`, `0xee4e60`, `0xf2a320`;
`morgard_use` at `0xe520d0`, `0xe52ab0`, `0xe94560`, `0xe947c0`,
`0xecea40`; `morgard_defense` at `0xe52ab0`, `0xe573c0`, `0xe5c5f0`,
`0xe9e220`, `0xe381b0` (`0xe3b588`). `object_battle` has no direct read
(section 7).

### Verdict, question 2

- **Starting a Serpen or Morgard play for one player: callable through
  the objective setter: yes, unknown whether it holds** (the next
  cascade pass judges it).
- **Starting it for the team: no single entry point.** Each member's own
  cascade must agree; the shared TeamScene list is the likely place
  where a play becomes the team's, and its writer was not found.

## 6. Question 3: enemy knowledge

**Read from the code.** `VisionTrackingState` (formatter `0xe7ab30`) at
`brain + 0xbf0`, five entries per field, one per enemy roster index:

| Brain offset | Field | Size |
| --- | --- | --- |
| +0xbf0 | `last_visible_pos` | 5 × 16 bytes (x, y; 1000 per unit) |
| +0xc40 | `mia_call_ticks` | 5 × u64 |
| +0xc68 | `last_visible_ticks` | 5 × u64 |
| +0xc90 | `last_checked_ticks` | 5 × u64 |
| +0xcb8 | `last_visible_distance` | 5 × u64 |
| +0xce0 | `last_hp_ratio` | 5 × u64 (written at `0xecbf15`, compared with 60 at `0xecc188`) |

The Mia stamps of the earlier documents (`brain + 0xc40 + enemy*8`) are
`mia_call_ticks`. **Superseded 2026-11-03:** they are also written by the
player's own vision update when it decides to call Mia (`0xecc1d6`), not
only by a received Mia.

`ObjectiveSpawnState` at `brain + 0xd08` (formatter `0xe7af80`):
`epic_spawn_call_tick`, `serpen_spawn_call_tick` (`+0xd10`, read at
`0xecbc8a`), `epic_camp_last_visible_tick`,
`serpen_camp_last_visible_tick`.

**Writer.** The team plan's vision update in `0xec9270`
(`team_plan.rs:613..668`), every decision of the viewed match: for each
enemy the environment reports (slot `0x220`), `last_visible_ticks` and
`last_checked_ticks` = now, `last_visible_distance` = 10^12 then the
distance, `last_visible_pos` = the unit's `+0x658`/`+0x660`, hp ratio;
then it may stamp `mia_call_ticks` and queue a Mia call. Entries of
enemies not reported are left as they were.

**Readers** (indexed reads found): `last_visible_ticks` in `0xe97570`
(`lane_ops.rs`) and `0xf8df90` (`team_plan.rs:734..766`, together with
`mia_call_ticks`); `last_checked_ticks` in `0xe72d80`
(`scene_feedback.rs`), `0xeaca30` (`obj_contest.rs`), `0xeb34f0`,
`0xec33a0` (`team_plan.rs`). Readers of `last_visible_pos` were not
enumerated (16-byte entries need a different search).

### Verdict, question 3

- **Writing a believed position: callable from a hook: yes for the
  write, unknown for the effect.** The table holds plain integers, no
  pointers, fixed size, inside the brain; writing an entry of an enemy
  the team cannot see survives until the enemy is seen again. Safe to
  write: `last_visible_pos`, `last_visible_ticks`, `last_checked_ticks`,
  `last_visible_distance` of one index 0..4, in each teammate's brain,
  from the step hook on the viewed match's worker. Not safe: anything
  outside the five entries, and `mia_call_ticks` (writing it may make
  the AI call Mia itself).
- **Whether a play triggers from a believed position: unknown.** The
  readers found are lane and objective-contest rules that look at how
  long ago an enemy was seen, which is the shape a "the enemy is at
  Serpen" ping needs. Which decision each one changes was not read.
- **Honest mechanics:** a believed position is what the player saw and
  told the team; the design session should decide whether a ping may
  plant one for an enemy nobody on the team saw (the author's rule:
  vision choice is playable, stats never change hidden outcomes).

## 7. Question 4: the strategy fields

**Read from the code.** The game-core `Strategy` (formatters `0x18d2650`,
`0x1900c60`; the pairing checked by hand against a third formatter,
`0xcb62b0`, which gives the same offsets; `game_finish` also matches the
reader in `game_finish.rs`), 24 bytes, offsets:

| Offset | Field | Offset | Field |
| --- | --- | --- | --- |
| +0x0 | object_buildup (with Split position) | +0x10 | minion_wave |
| +0x4 | morgard_use (with Split positions) | +0x11 | focused |
| +0xc | object_battle | +0x12 | early_jungle |
| +0xd | tower_press | +0x13 | early_serpen |
| +0xe | morgard_defense | +0x14 | early_serpen_top |
| +0xf | object_finish | +0x15 | game_finish |
| | | +0x16 | dm_tactic |

**Where the AI gets it.** Environment slot `0x108` (kind-0 vtables
`0x40a2530` → `0x16c78b0`, `0x40c9f28` → `0x18d5c20`) copies 24 bytes
from `sim + 0xb3a8 + side × 0x18`. Before some reads the code asks slot
`0xe8` (`sim + 0xf03d`, set by the sim constructor `0x16ee2b0`) and, when
it is true, reads a copy in the player state at `player + 0x388` instead
(`0xe3b374`). **Not read:** which of the two the viewed match uses, and
where the SDK's `strategy_set_json` writes. dev9's observation (a change
kept and acted on, seen in game) fits the sim copy.

**When.** Every read is a fresh call inside a decision; no field is
copied into a brain. About 60 call sites. So every field is read "at
every decision" of the rule that uses it; the phase-bound ones are
phase-bound because their rules only run in that phase.

| Field | Read in (function, file) | Decision |
| --- | --- | --- |
| focused | `0x10cd420`, `0x10ced80`, `0x10e2a90`, `0x10e3610` (`passive_jungle.rs`), `0xe5c5f0` (`engage.rs`) | Jungle routing; one fight rule |
| early_jungle | `0x10cd420`, `0x10d1180`, `0x10e2a90` (`passive_jungle.rs`) | Jungle plan |
| early_serpen | `0xeb1730`, `0xeb4a00` (`objective_handlers.rs`), `0xeb2510`, `0xec9270` (`team_plan.rs`) | Serpen objective |
| early_serpen_top | `0xeadc20` (`obj_contest.rs`) | Objective contest |
| object_buildup | `0xe93bd0` (`lane_ops.rs`), `0xec4090` (cascade, 2), `0xed4520` (`serpen.rs`), `0xee8960`, `0xee9c40` (HuntAndPoke), `0xe52ab0` | Assembling for an objective |
| object_battle | no direct byte read found | **Not read** (may be read through the `player + 0x388` copy or by pointer) |
| object_finish | `0xe5abe0` (`engage.rs`), `0xece670`, `0xecec90`, `0xee4e60` (`single_battle.rs`), `0xefbfe0` (`battle.rs`), `0xf2a320` (`objective_hunt.rs`), `0x108d750`, `0x10b0e30` | Finishing an objective under fight |
| morgard_use | `0xe520d0`, `0xe52ab0` (`assign.rs`), `0xe94560`, `0xe947c0` (`lane_ops.rs`), `0xecea40` | Morgard and lane split |
| tower_press | `0xe58970`, `0xe5c5f0` (`engage.rs`), `0x10a52a0` | Pressing towers |
| morgard_defense | `0xe52ab0`, `0xe573c0` (`assign.rs`), `0xe5c5f0` (`engage.rs`), `0xe9e220` (`defense_nexus.rs`), `0xe381b0` (`0xe3b588`) | Defending against Morgard |
| minion_wave | `0xe57230`, `0xe89af0`, `0xe946b0`, `0xfdb300` (`lane_ops.rs`), `0xe97240` | Wave or join priority |
| game_finish | `0xe381b0` (`0xe428f9`), `0xe9aa10`, `0xeabc20`, `0xeb62c0` (`objective_handlers.rs`), `0xff6060` (`active_recall.rs`), `0x1053610` (`game_finish.rs`, 4), `0xe5c5f0` | Ending the game, recall, Nexus push |
| dm_tactic | `0x1034940`, `0x1040c40`, `0x1089070` (`death_battle.rs`) | Death-match fights |

Method: script `stratreads2.py` follows each slot `0x108` call to the
first byte read of the returned copy within 30 instructions (the copy
is often reused later, so later reads are not counted). A field read
only through a pointer to the whole struct is missed.

### Verdict, question 4

- **Confirms** section 12's guess for `early_*` (phase-bound by their
  rules), `focused` (continuous: read at every jungle decision),
  `object_*` and `morgard_*` (read in objective rules).
- **Rejects** any reading in which a live change waits for a phase: no
  field is cached.
- **The strategy setter: callable: yes** (official interface, seen in
  game, dev9). A per-player lever it is not.

## 8. Question 5: the signal kinds that never travel on the inbox

**Read from the code.** `TeamPlan.signals` (`brain + 0xac0` capacity,
`+0xac8` pointer, `+0xad0` length; 24-byte entries: tag byte, lane byte
at +1, a u64 at +8, a u64 at +0x10) is a second list of `Signal`
values, written and read by the team plan of the same brain:

| Tag | Kind | Produced at |
| --- | --- | --- |
| 15 | SerpenPrepare (u64 = a time ratio) | `0xecbcff` (vision update, when the Serpen respawn nears) |
| 17, 20 | SerpenHunt, MorgardHunt | `0xec417b` (cascade, `0x11 + 3k`), `0xeb76dd` (Serpen handler) |
| 21 | AttackNexus (lane) | `0xeb741d` |
| 22 | DefenseNexus | `0xeb7457` |
| others | | more push sites in `0xeb9e60`, `0xebf4d0`, `0xec4090`, `0xeb4a00`, `0xeabda0`, `0xeb6d40`, `0x101b470` (not decoded) |

Consumers: the cascade `0xec4090` runs the compatibility check
`0xfcd5e0` over every entry (`0xec4a40`) against the receiver's mode,
and the same functions read it. `TeamPlan.signals` is the `signals`
field of the TeamPlan; the big plans have their own `signals` fields.

**Second channel?** The inbox broadcast `0xf163a0` reads the agent's
`events` (`brain + 0x3c70..0x3c98`), not `TeamPlan.signals`; in game
only tags 1..11 were seen on queue A. No code was found that copies
`TeamPlan.signals` into another brain. **Guess:** these kinds are a
brain talking to itself across decisions (a record of what it decided,
used by later rules and by the trace), and team agreement comes from
every brain reading the same shared block (section 5). Lists C and D
(`+0x1eb8`, `+0x1ed0`) hold replies and are written by the opener, the
receive handler, `0xe4e770`, `0xe5c5f0` and `0xf10bf0`; their consumers
were not read.

### Verdict, question 5

- **Injecting objective kinds into the inbox: no** (the receive handler
  dispatches 1..11 only; seen in game, ten matches, earlier document).
- **Writing them into a teammate's `TeamPlan.signals`: unknown.** It is
  a Vec (writing needs `len < cap`, like the inbox write); what reading
  a foreign entry does was not read.

## 9. What a ping could call

| Candidate | Address | Arguments | Kind 0 | Allocates | Shared state | Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| Objective setter | `0xec3820` | `brain + 0xe8`, u24 objective, `&Location` | no check | only when tracing | none (own brain) | **yes**; persistence unknown (cascade may undo it next decision) |
| Objective bytes | `brain + 0xe45..0xe47` | 3 bytes | — | no | none | **yes**, same caveat; the setter also clears `press_follower`, `repair_line`, `def_depart`, `repair_owner`, so calling it is cleaner |
| Inbox HideLine(lane) | queue A entry, tag 6 | existing inbox write | no check on the objective path | no | none | **likely yes**: may give a laner Gank(lane) |
| Scene opener | `0xe662f0` | known | no check | no | counters | **yes** (seen in game, dev7/dev8) |
| Believed enemy position | `brain + 0xbf0`, `+0xc68`, `+0xc90`, `+0xcb8` | index 0..4 | — | no | none | **yes for the write**, effect unknown |
| Strategy setter | SDK `strategy_set_json` | team, path, JSON | — | — | team | **yes** (seen in game, dev9); team-wide only |
| Plan builder | `0xe52ab0` | 6 arguments | checks | yes | — | **no** |
| Cascade | `0xec4090` | 8 arguments | — | yes | shared block | **no** (it is the AI's own decision step) |
| TeamPlan.signals entry | `brain + 0xac0` Vec | tag, lane, value | — | yes on growth | none | **unknown** |

Checks for each "yes" (read from the code): no slot `0x40` early exit on
the path; arguments available at the step hook (brain, ctx, player,
pair); no allocation; no clearing of shared state.

What this means for the ping kinds the author listed (**guess** until
probed):

| Ping | Possible lever |
| --- | --- |
| Lane: push / defend this lane | Objective Nexus(lane) / DefenseLine(lane) through the setter |
| Gank this lane (to the jungler or a laner) | Objective Gank(lane) through the setter; HideLine on the inbox for the laner |
| Take Serpen / Morgard | Objective Serpen / Morgard (phase Setup) through the setter, for each teammate the ping names |
| Defend base | Objective Defense |
| Order one named player | The setter on that one brain: the per-player lever the earlier documents lacked |
| Enemy missing / seen there | Believed position in `VisionTrackingState` |
| Trade one objective for another | Not found as a rule; two objectives set on two groups is the nearest |

The first probe worth building: call the setter for one teammate with
Serpen Setup during that teammate's step, and log `brain + 0xe45` and the
plan tag at `+0x16a0` at its next 20 steps. It answers persistence, the
main open question.

## 10. Not read

- The cascade bodies `0xeb9e60` (21 KB) and `0xebf4d0` (15.6 KB) site by
  site: when each objective is set and cleared. This decides whether a
  mod-set objective survives. **Partly read 2026-11-03:** Gank in full
  (section 13), the other objectives by reason value (section 14).
- Who writes the per-team shared block (`ctx + 0x10`, TeamScene Vec at
  `+0x2b8`, records at `+0x808`): `scene_publisher`, `sync_out`,
  `relays.rs` are the leads.
- The assign paths for Morgard, Serpen, Repair, Gank, PressTower and
  ComebackPick objectives (`0xe52c86` onwards).
- The meaning of the receiver's mode byte (`[pair + 8] + 0x38`) beyond
  the masks; the mode names table has six names for nine values.
- Which strategy copy the viewed match reads (`sim + 0xf03d`), and where
  `strategy_set_json` writes; `object_battle`'s reader.
- Readers of `last_visible_pos`; what each vision reader decides.
- Consumers of lists C and D and of `TeamPlan.signals` entries beyond
  the compatibility check.
- The goal enum and modes: where they sit in the brain.
- `MobaBrain` (`0x18e9fb0`): its `blackboard`, and whether it writes the
  shared per-team block.

The read-only scripts used here (annotated disassembly, source-file map,
field names from Debug formatters, store and read search, strategy
reads) are not in git; the hand-over note for this session lists them.

## 11. Contradictions with the earlier anchors

- "The team handler `0xe381b0`" is one player's planner step
  (`plan/handler.rs`); the team rules run inside it, per player.
- "Scene kind at `brain + 0x16a0`" is the big plan's tag.
- The no_help mask `0xf26` is over the plan index (raw − 2), not the raw
  kind (section 4). Traced: a failed test (`0xe67115`) clears the allowed
  flag and reaches the `no_help` increment at `0xe66a04`.
- The Mia stamps are `VisionTrackingState.mia_call_ticks` and are also
  written by the player's own vision update.
- The team-plan objective logic for Serpen runs only in environment
  kind 0; the viewed match is kind 0, so it runs there (the receive
  handler's kind-0 exit does not reach it).

## 12. Seen in game: the per-player levers (1.7.1-dev10 and dev11, two matches, 2026-11-03)

Probe keys plant an objective on the teammate nearest to the cursor
through the setter `0xec3820` at that teammate's step, then log the
objective and the big plan at the next 20 steps and once a second all
match. Match 1: side 0, slot 2; match 2: side 1, slot 2. Game 0.7.0.

| Order | Presses | Outcome |
| --- | --- | --- |
| Serpen, Setup, with battle | 4 | Kept every time. One laner's plan became SerpenHuntAndPoke after about 12 s and the laner walked to Serpen (28 s until the play ended); one was advanced to Hunt by the AI itself after 10 decisions; one kept through a fight; one became SerpenHuntAndPoke at the 20th decision. |
| Nexus(lane) | 1 | Kept; the plan became AttackNexus at the very next decision; 21 s until a fight. |
| DefenseLine(lane) | 2 | On the jungler: cleared at the next decision. On a laner: kept 27 s, plan rebuilt as PassiveLine; the laner walked a map edge. Which lane the rebuilt plan holds is not logged. |
| Gank(lane) | 9 | Cleared or replaced at the next decision every time (the jungler on camps three times, a laner, a laner already on a Morgard play, the jungler on its own Nexus push). |
| HideLine on the receiver's own lane (inbox) | 2 | On a laner holding Nexus: nothing (the path needs no objective; expected). On an idle laner: heard a second later, Gank(own lane) at the next decision, kept about 80 ticks, the laner moved up the lane and engaged (plan Battle). |
| Believed position (vision table) | 3 | Not a valid test: the key picked an enemy on screen, hence visible, each time. Match 1: the game rewrote the position at the next decision. Match 2: the entries kept the written values for the few decisions logged; no team reaction attributable. |
| Strategy `focused` = All | 1 | Accepted and kept (nothing to conclude from "All"). |

**Read from the log, the AI's own gank:** a pair. The jungler holds
Gank(lane) with the LineGanker plan while the laner of that lane holds
Gank(own lane) with PassiveLine, at the same ticks. The inbox HideLine
gives the laner's half; the jungler's half is behind the cascade's gank
gates (`gank_gates.rs`), not read.

**Lane numbers.** The probe names lane 0 Top, 1 Mid, 2 Bottom from the
roster slots (slot 0 top, slot 2 mid, slots 3 and 4 bottom); a guess. A
side-0 laner with Nexus(0) walked the y ≈ 30 edge from x 299 to 478 in
match 1, which fits lane 0 = the lane along the y ≈ 0 edge. The plan's
own lane field is the way to settle it.

**Consequences for pings.** Objective orders for Serpen, a lane push and
a lane defence are accepted at the teammate's next decision and followed
for tens of seconds; Morgard is the Serpen order's twin and untried; the
laner's gank setup is reachable through the inbox; the jungler's gank
needs its gates read; a planted objective is written once, so a refusal
is final unless the mod asks again (a standing request, proposed).

## 13. The gank gates: who sets Gank, what keeps it, what clears it (read 2026-11-03)

Read from the 0.7.0 executable after the section 12 tests. Offsets are
brain offsets unless TeamPlan (`brain + 0xe8`) is named. A "decision" is
one run of the planner step `0xe381b0` for one player.

### Correction to the starting question

**Read from the code.** The cascade (`0xec4090`, bodies `0xeb9e60` and
`0xebf4d0`) never sets Gank. When it finds a Gank, its own setter calls
are clears, the Dive promotion (`0xec78f2`) and replacements by other
objectives. `gank_gates.rs` is not part of the cascade. Two of its three
functions, `0xf8cca0` (line 18) and `0x106fed0` (line 65), sit in the
jungle plan's own gank decision. The third, `0x1070960` (line 9), is
used by the cascade for a different test, not for Gank. Section 12's
"the jungler's half is behind the cascade's gank gates" should read
"behind the jungle plan's gank decision".

### Where Gank is set

**Read from the code.** Gank is set in three places, all outside the
cascade. Each one stamps `gank_start_tick` (TeamPlan `+0xc40` = brain
`+0xd28`) with the current tick right after the setter call:

| Who | Setter call (source line the binary passes) | Condition |
| --- | --- | --- |
| The jungler | `0xe400c8` (`handler.rs:987`) | The new big plan just built is LineGanker (raw 10) and the objective is none. The lane is the plan's lane byte (plan `+0x3b`). When the last recorded gank start is over 10 s old, a counter at brain `+0x11d0` goes up. |
| A laner | `0xe3d8ad` (`relays.rs:837`) | All must hold: the laner is in roster slot 0, 2, 3 or 4. The own team's shared block holds a TeamScene of kind 10 (`+0x46`) on the laner's own lane (`+0x45`, table `0x202010000`). That scene is owned (`+0x20`) by another player. It is the first step the scene appears (edge flag brain `+0x2c4f`). The objective is none. Stamp at `0xe3d8c4`. |
| A laner, inbox | `0xe67872` (`signal.rs`) | HideLine or HideLineToo on the laner's own lane, objective none (section 4). |

**Read from the code.** The setter `0xec3820` itself does not write
`gank_start_tick`. A Gank planted through the setter alone keeps the
stamp of the player's last real gank (or 0).

### The jungler's gank decision (plan layer)

**Read from the code, in part.** LineGanker is chosen inside the
PassiveJungle plan. `0x10cd420` (`passive_jungle.rs:571..725`) calls
`0x10cf830` (`ganker.rs:75`, `passive_jungle.rs:1240`) at `0x10cd85a`.
Its inputs, in order:

1. **Strategy, read fresh** (env slot `0x108` at `0x10cd75a`): `focused`
   (`+0x11`, read at `0x10cd77c`) and `early_jungle` (`+0x12`,
   `0x10cd7d2`). Which values the decision gets depends on two tests,
   the mode byte (`[world + 0x38]`) and the time:
   - **Fixed values** (focus 2, strict flag on, `0x10cd810`): when the
     mode byte is 0, 5, 7 or 8 and the tick has reached 30 s before the
     tick at `[world + 0x8a8]`.
   - **Strategy values** otherwise: the team's `focused` value, and the
     strict flag set to "`early_jungle` is not 1".

   **Guess:** `world + 0x8a8` is the first Serpen spawn. The cascade
   uses the same 30 s rule with `first_serpen_state` at `0xec79f2`.
2. **An athlete test** (`0x10cf877`): when player byte `+0x2c0` is 1, in
   the viewed match, `0x18c3470` with player values `+0x210` (capped at
   100), `+0x278`, `+0x298`; a negative result means no gank.
   **Guess:** a stat-based roll.
3. **Two lane tests** `0x10e1970` (`passive_jungle.rs:1626..1647`) with
   the focus value.
4. **Travel estimate** `0x106fed0` (`gank_gates.rs:65`): distances from
   the jungler to fixed map points by side (constants 176.0 to 800.0,
   stored × 1000) over its move speed (entity `+0x638`); returns 0
   outside environment kind 0.
5. **Candidate lanes** `0xf8cca0` (`gank_gates.rs:18`), each lane
   filtered by `0x10e5680`. The filter refuses a lane when:
   - the jungler has a Serpen or Morgard scene;
   - the jungler holds Morgard, Serpen, Defense or Nexus (mask `0x17`);
   - the same mode and 30 s rule as step 1 fails;
   - the check `0x1070540` fails.

   A Gank objective passes this filter, so a planted Gank does not steer
   the lane choice.
6. **A score per lane** `0x10e4290` (`passive_jungle.rs:1159..1207`); the
   best lane is kept.
7. **Thresholds** (`0x10cfb3a`): with the strict flag, travel estimate
   non-zero and score at least 30; without it, score at least −40.
8. **Arrival in time** (`0x10cfb94`..`0x10cfe84`): the travel estimate
   against a window from `0x10e4df0` and `0x10e5050`
   (`ganker.rs:466..511`); too slow means no gank. Not read in full.

The result is a LineGanker plan for that lane; the step then sets
Gank(lane) (table above).

**Read from the code.** The plan builder `0xe52ab0` has no branch for
the Gank objective. Objective 6 skips the branches for:
- Defense, Nexus and Dive (`0xe52afd`);
- Repair (`0xe52cad`);
- PressTower (`0xe52d76`);
- ComebackPick (`0xe53841`).

It falls into the shared path at `0xe53a6f`.

- **Guess:** that path builds the role's usual plan: PassiveLine for a
  laner, PassiveJungle for the jungler.
- **Seen in game** (section 12 log): laners holding Gank(own lane) held
  PassiveLine.
- The only LineGanker the builder makes is for ComebackPick on the
  jungler slot (`0xe538b7` → `0xe54420`, builder reason 8).

### What one decision does with a Gank it finds

**Read from the code.** The steps below run in this order. Every
completed cascade pass reaches step 2 (a control-flow check finds no
return that avoids `0xec7061`); steps 3 to 5 run only while the
objective is still Gank after the step before.

1. **Map-mode gate, silent** (`0xec4973`..`0xec49f9`,
   `team_plan.rs:422`). The mode byte must be in the set for the lane:

   | Lane | Allowed modes | Mask |
   | --- | --- | --- |
   | 0 | {0, 2, 7, 8} | `0x185` |
   | 1 | {0, 4, 5, 7, 8} | `0x1b1` |
   | 2 | {0, 1, 3, 5, 7, 8} | `0x1ab` |

   The byte is `[world + 0x38]`, shared by the whole match. On failure
   the objective is cleared with no reason written. **Guess:** a map or
   match mode. Nexus and DefenseLine orders pass the same gate in
   section 12, so the played matches pass it.
2. **No Serpen or Morgard scene** (`0xec70d1`, `0xec722f`). Either scene
   counts as pending while its tag byte is not 2:
   - `serpen_scene`: TeamPlan `+0xa70`, tag byte brain `+0xb73`;
   - `morgard_scene`: TeamPlan `+0xa90`, tag byte brain `+0xb93`.

   While one is pending, a Gank or ComebackPick is cleared with reason
   `0x30` (`cascade.rs:252` for Serpen, `:270` for Morgard). The first
   body then turns the scene into the Serpen or Morgard objective.
3. **The mode gate again** (`0xec72ed`): fail clears, reason `0x26`
   (`objective_handlers.rs:1736`).
4. **The gank timer** (`0xec7359`). Here `delay` is
   `[[world + 8] + 0x12f8]` (60 ticks = 1 s). The rule depends on the
   plan:
   - **Plan LineGanker:** cleared when more than 20 × delay (20 s) has
     passed since `gank_start_tick`, reason `0x27`
     (`objective_handlers.rs:1765`).
   - **Any other plan, inside the window:** kept while the tick is within
     `delay × (1 + 0.0003 × s²)` of `gank_start_tick`. That is 1 s to
     4 s, with `s` = player value `+0x2b0` capped at 100
     (`0xec73d2`..`0xec741f`).
   - **Any other plan, after the window:** kept only while the own
     team's shared block holds a kind-10 TeamScene on that lane
     (`0xec7421`..`0xec745f`). Otherwise cleared, reason `0x31`
     (`objective_handlers.rs:1756`). This test does not check who owns
     the scene.
5. **The lane's laner** (`0xec7461`). The roster slot for the lane must
   hold an entity with at least 25 % hp (`+0x668 × 100 / +0x620`). The
   slot comes from table `0x4018328`: lane 0 is slot 0, lane 1 is
   slot 2, lane 2 is slot 3. On failure:
   - the Gank is cleared, reason `0x28` (`objective_handlers.rs:1779`);
   - a Cancel (tag 11) goes onto the player's own `TeamPlan.signals`,
     with 2 (no laner) or 1 (low hp).
6. **Dive promotion** (`0xec76d0`..`0xec7940`). This is a replacement,
   not a refusal. It needs all of:
   - the plan is LineGanker;
   - that lane's enemy tower is hitting an allied champion;
   - two counts agree (`0xed8930`, `0xed8400`).

   Then the Gank becomes Dive(lane) (`0xec78f2`) and tag 23 is pushed on
   `TeamPlan.signals`.
7. **Replacement by a better pick** (body `0xebf4d0`, case 6 →
   `0xebf579`). A Gank is handled exactly like no objective: the full
   pick runs and any pick that fires replaces the Gank. The picks are
   the Nexus finisher `0xeb62c0`, then Defense, Repair, Serpen Hunt,
   DefenseLine, PressTower and ComebackPick. A reachability check from
   `0xebf579` finds no clear and no reason write on that path, and none
   of the 31 functions it calls is a caller of the setter.
8. **After the cascade, in the step.** Two more clears:
   - **A fight ended.** When the plan was Battle and the new plan is
     not, Gank, Dive and ComebackPick are cleared, reason `0x29`
     (`0xe4006a`, `handler.rs:982`).
   - **A LineGanker episode closed.** `0xe4e2d0` (`handler.rs:1140`)
     closes the episode and clears Gank or Dive, reason `0x2a`. It also
     clears ComebackPick when the plan fell back to Battle with a
     stashed lane. The step calls it at `0xe412a0` and `0xe434ed`; when
     it does so was not read.

**Not taken in game: the restore path.** The step can run the cascade on
a restore path (`0xe40427`..`0xe40651`). There, the objective is put
back afterwards unless the cascade cleared it. The path is taken when
the step's seventh argument is true. That argument is brain byte
`+0x47e2`. The only stores found write 0, and `0xf16d60` clears it just
before the call (`0xf16e74`).

### Why the plants failed, and what a mod can satisfy

**Guess, consistent with the dev11 log:** most of the nine planted
Ganks failed at step 4.
- The setter did not stamp `gank_start_tick`, so the timer had long run
  out.
- No kind-10 scene existed on that lane.
- Together these give reason `0x31` at the first decision.
- The laner already on a Morgard play probably failed earlier, at
  step 2 (reason `0x30`), if its Morgard scene was still pending.
- Step 4 runs before the second body. A timer clear followed by the
  body's re-pick (for example Nexus for the jungler on its own push)
  shows in the log as "replaced". So section 12's "cleared" and
  "replaced" need not mean different gates.

The inbox HideLine does stamp the tick. Its laner held Gank "about 80
ticks", which fits the window above with `s` near 33. Not proven. The
next probe should log brain `+0xd28`, `+0x1b0` and `+0x1b8`, and player
`+0x2b0`, with each plant.

| Input | RVA | Can a mod satisfy or write it? |
| --- | --- | --- |
| `gank_start_tick` (brain `+0xd28`) | `0xec7372` | **Write: yes.** A plain u64 the game's own setters stamp. Stamping it with the current tick at plant time is what the game does. Re-stamping it every decision would be a standing request, a choice for the design session. |
| Serpen or Morgard scene pending (brain `+0xb73`, `+0xb93`) | `0xec70d1` | **Check: yes, write: no.** Plant only when both tag bytes are 2. |
| Mode byte `[world + 0x38]` | `0xec4973`, `0xec72ed` | **No.** Passes in the matches played. |
| Kind-10 TeamScene on the lane (shared block `+0x2b8`) | `0xec7421` | **No.** Writer unknown, and a push would allocate. |
| The plan (LineGanker for 20 s) | `0xec7379` | **No.** Built by the game; a Gank objective does not make one. |
| The lane's laner, hp ≥ 25 % | `0xec7461` | **Check: yes, write: no.** Read the roster entity before planting. |
| Player value `+0x2b0` (window length) | `0xec73d2` | **Do not write.** Unnamed. **Guess:** an athlete stat, which honest mechanics forbids changing. |
| Other picks (Nexus finisher, Defense, Serpen, ...) | `0xebf579` | **No.** A real reason the AI changes its mind. |
| `early_jungle`, `focused` (strategy) | `0x10cd75a` | **Write: yes** (dev9 kept a live change). They steer the jungler's own gank decision, not whether a planted Gank survives. |

### Verdict, question 1 of this read

- **A planted jungler gank can survive the next decision when** all of
  these hold (**read from the code**):
  - `gank_start_tick` is stamped with the current tick at plant time;
  - the jungler has no Serpen or Morgard scene pending;
  - the lane's laner is there with at least 25 % hp;
  - no other pick fires.

  It then lasts 1 s to 4 s, not more, unless the mod re-stamps the tick
  every decision or a kind-10 scene appears on that lane.
- **Whether the jungler walks to the lane while holding it** is untested,
  and from the code unlikely (**Guess**). The objective alone does not
  turn the jungler's plan into LineGanker: the builder has no Gank branch
  and the jungle decision ignores a Gank objective.
- **A planted laner gank** follows the same rules. It lasts as long as
  the window, or as long as a kind-10 scene (the jungler's LineGanker
  play) exists on its lane. That is the laner's half of the AI's own
  pair; the jungler's half comes only from the jungle plan's decision.

## 14. Objective drop reasons (read 2026-11-03)

### The field

**Read from the code.** `TeamObs` is `TeamPlan.obs`, at TeamPlan `+0x0`
(Debug formatter `0xe77110`, 138 fields). It names TeamPlan `+0xc8` as
`mf_obj_clear`, a pair `(u8, u64)` printed by `0x1072e30`:
- the reason byte: TeamPlan `+0xc8` = **brain `+0x1b0`**;
- the tick it was written: TeamPlan `+0xd0` = **brain `+0x1b8`**.

The binary holds no names for the reason values, so the meanings below
come from the code at each writer. The "site" column is the `Location`
(source file and line) each writer passes to the setter, which is what
the game's own trace records. No other reason field was found. The
builder's reason byte (plan `+0x400`) is a different thing: it says why
a plan was built.

Values written, all as constants unless noted:

| Value | Writer | Site | Objective dropped | Meaning |
| --- | --- | --- | --- | --- |
| `0x02` | `0xebfc15` | `objective_handlers.rs:104` | Defense | None of the defence threat tests (`0xe9c9d0`, `0xe9df90`, `0xe9e8e0`, `0xe9e7f0`, `0xe9c6d0`) holds any more, and the plan is not Battle. |
| `0x03` | `0xec0afd` | `:115` | Nexus | Map-mode gate for the lane. |
| `0x04` | `0xec17b1` | `:172` | Nexus | After `0x1070a50`/`0x1070cb0` and `nexus_basis`; not read. |
| `0x05` | `0xec0a19` | `:185` | DefenseLine | Map-mode gate. |
| `0x06` | `0xec1439` | `:214` | DefenseLine | A TeamScene owner test; not read. |
| `0x07` | `0xec12c4` | `:220` | DefenseLine | After a distance test `0x10f4e00`; not read. |
| `0x08` | `0xec0aa7` | `:230` | Dive | Map-mode gate. |
| `0x09` | `0xec0d48` | `:249` | Dive | `0xeb6df0` (`objective_handlers.rs:1852`) false; not read. |
| `0x0b` | `0xec0b44` | `:262` | PressTower | Map-mode gate. |
| `0x0c` | `0xec19a2` | `:294` | PressTower | Enemy tower slots (`+0x1f8`/`+0x208` or `+0x218`/`+0x228`); not read. |
| `0x0d` | `0xec1a18` | `:302` | PressTower | Same towers, after a tick test; not read. |
| `0x0e` | `0xec28ed` | `:327` | PressTower | Not read. |
| `0x0f`, `0x10`, `0x11` | `0xec2944` (from `dl`) | `:338` | PressTower | Returned by `0x103b4c0` (`opportunities.rs:217`), one value per exit; conditions not read. |
| `0x18` | `0xec0a60` | `:357` | ComebackPick | Map-mode gate. |
| `0x19`..`0x1f` | `0xec1b75`, `0xec0eb3`, `0xec1729`, `0xec192b`, `0xec1b18`, `0xec2cdc`, `0xec2721` | `:403`, `:412`, `:429`, `:455`, `:464`, `:472`, `:480` | ComebackPick | Seven checks, several after a TeamObs counter; not read. |
| `0x20` | `0xeb72d0` | `:508` | Serpen | Mode gate (mask `0x1a1`). |
| `0x21` | `0xeb74ac` | `:535` | Serpen | Serpen respawns more than 15 s away. |
| `0x23` | `0xeb7871` | `:627` | Morgard | Mode gate (mode 1 to 6). |
| `0x24` | `0xeb7831`; `0xec5102` | `:638`; `cascade.rs:117` | Morgard; Serpen or Morgard | The monster respawns more than 15 s away (in the Morgard handler; and in the cascade, viewed match only). |
| `0x26` | `0xec74c6` | `:1736` | Gank | Map-mode gate (section 13, step 3). |
| `0x27` | `0xec739c` | `:1765` | Gank | LineGanker and over 20 s since `gank_start_tick`. |
| `0x28` | `0xec7656` | `:1779` | Gank | The lane's laner missing or under 25 % hp (plus a Cancel signal). |
| `0x29` | `0xe4006a` | `handler.rs:982` | Gank, Dive, ComebackPick | A fight ended (plan Battle → not Battle). |
| `0x2a` | `0xe4e418` | `handler.rs:1140` | Gank, Dive (ComebackPick) | A LineGanker episode closed (`0xe4e2d0`). |
| `0x2b` | `0xe674aa` | `signal.rs:299` | Gank, Dive, ComebackPick | Inbox Cancel; only when the environment kind is not 0 (section 4), so not in the viewed match. |
| `0x2e` | `0xec0837` | `cascade.rs:676` | Repair | A `repair_owner` slot test; not read. |
| `0x2f` | `0xe3d6da` | `relays.rs:602` | PressTower | `press_follower` set and a one-shot flag (brain `+0x2c4e`) was 1. **Guess:** the press leader's call ended. |
| `0x30` | `0xec71ec`, `0xec72b6`; `0xe4dd07` | `cascade.rs:252`, `:270`; `handler.rs:468` | Gank, ComebackPick; any | A Serpen or Morgard scene is pending (the two cascade sites). `handler.rs:468` clears when the Morgard scene slot is dropped outside the viewed match (read in part). **Also written without a clear** at `0xec71a5` and `0xec7268`: the objective is kept and the scene parked or dropped. **Superseded 2026-11-04** (sections 27, 28): the "scene" is the team's mirrored Serpen or Morgard record; a brain holding any objective other than none, Gank or ComebackPick throws it away (only the publisher parks it, for Defense, DefenseLine, Nexus or PressTower). Not a pending state. |
| `0x31` | `0xec7aa9` | `:1756` | Gank | Not LineGanker, the 1 s to 4 s window passed, and no kind-10 scene on the lane. |

Values not found written: 0, 1, `0x0a`, `0x12`..`0x17`, `0x22`, `0x25`,
`0x2c`, `0x2d`. A search for 16-byte stores at TeamPlan `+0xc0`/`+0xc8`
(brain `+0x1a8`/`+0x1b0`) found only the plan builders writing their own
output structs, not this pair. They were not checked one by one.

### Silent clears and replacements

**Read from the code.** These change the objective and leave
`mf_obj_clear` as it was:

- The map-mode gate `team_plan.rs:422`, inlined at `0xec49f9` (cascade),
  `0xeca891` (`0xec9270`) and `0xec9004` (`0xec8f20`).
- `relays.rs:535` and `:544` (`0xe3d040`, `0xe3d0ce`), in the step's
  Serpen and Morgard relay code.
- The seven clears in the first body `0xeb9e60` (`0xebd836`, `0xebd898`,
  `0xebde84`, `0xebe588`, `0xebeb67`, `0xebec2e`, `0xebecf1`), during a
  Serpen or Morgard scene play.
- Every replacement: a setter call with another objective writes no
  reason.

**Read from the code: the game's own trace.** Tracing is switched on by
`TeamObs.trace_on` (TeamPlan `+0x748` = brain `+0x830`). When it is on,
the setter pushes a 0x80-byte `ChangeEvent` onto `TeamObs.trace_changes`
(TeamPlan `+0x18` = brain `+0x100`). The event has six fields:

| Field | Content |
| --- | --- |
| `tick` | A tick field; where the setter fills it from was not traced (it gets no tick argument). |
| `layer` | The 9-byte string "objective". |
| `from`, `to` | The old and new objective, formatted as text. |
| `reason` | An empty string (`0xec398f`..`0xec399f`): the reason byte is not copied. |
| `site` | The setter's `Location` argument (source file and line). |

Each change allocates three strings on the game thread. Who sets
`trace_on` was not read. **Superseded 2026-11-04** (section 26): the
agent step copies it from the world byte `+0x3c` at every think.

### What a dev panel should read

**Read from the code; the procedure itself is untested.** Do this for
one teammate at the existing step hook, which runs before that
teammate's decision:

1. Save the objective (brain `+0xe45`, 3 bytes), the reason (brain
   `+0x1b0`, u8) and its tick (brain `+0x1b8`, u64). For an order the
   mod plants, take this snapshot right after the mod's own setter call.
   A snapshot taken before the plant would read a cleared plant as "none
   → none, kept", which is exactly the case the panel is for.
2. At the same teammate's next step hook, read them again:
   - Objective unchanged: kept (a `0x30` with a new tick may still have
     been written; ignore it).
   - Objective now `0xff` and the tick is new: cleared, and the reason
     byte is the cause (table above).
   - Objective now `0xff` and the tick unchanged: a silent clear, most
     likely the map-mode gate or a Serpen or Morgard scene play.
   - Objective changed to another value: replaced by the AI's own pick;
     show the new objective, not a reason.
3. For a planted Gank, also show the four inputs that decide it:
   - `gank_start_tick` (brain `+0xd28`) against the current tick;
   - the two scene tag bytes (brain `+0xb73`, `+0xb93`);
   - the lane's laner hp.

The exact site of every change, silent ones and replacements included,
is only available in two ways: through the trace (which allocates), or
through a hook on the setter `0xec3820`. That is exactly what a new hook
would buy. The reason byte and the four inputs above cover the Gank case
without it.

### Corrections to earlier sections

- **Section 5,** "the meaning of the two compared times
  (`team_plan + 0xb8`, `+0xc8`)": **superseded 2026-11-03.**
  - At `0xeb7354` those two words are read through another pointer
    (`[rsp + 0x68]`), not the TeamPlan. What that pointer is was not
    read.
  - TeamPlan `+0xc8` is `mf_obj_clear`'s reason byte.
- **Section 10,** first item:
  - The cascade's rules for Gank are now read (section 13).
  - The rules for the other objectives are listed by reason value above.
    The conditions marked "not read" are still open.

### Not read in this pass

- The jungle decision after the thresholds (`0x10cfb94`..`0x10cfe84`),
  the lane score `0x10e4290`, the athlete test `0x18c3470`, and what
  player `+0x2b0`, `+0x210`, `+0x278`, `+0x298` are.
- Who writes the kind-10 TeamScene (the shared block's writer is still
  unknown, section 5).
- When the step calls `0xe4e2d0` (reason `0x2a`).
- The conditions behind the reasons marked "not read".

## 15. Why an order is refused: reasons 0x04, 0x06 and 0x07 (read 2026-11-04)

All three are written by the second cascade body `0xebf4d0`, which
switches on the objective tag (jump table `0x401858c`: case 3 DefenseLine
`0xebf646`, case 4 Nexus `0xebf8d1`). Register `rsi` there is the
TeamPlan (brain `+0xe8`). "World" below is `[ctx]` (the sim the step
reads the tick from at `+0x1b8`). Distances are squared and in the
game's 1000-per-unit integers.

**World fields used here (read from the code; names are guesses unless
stated):**

| World offset | Content |
| --- | --- |
| `+0x170 + side*0x20` | A per-side list (pointer `+0`, length `+0x18`) of entities. **Guess:** the side's other buildings. |
| `+0x1b8` | Current tick. |
| `+0x1c8 + side*8` | One entity per side. **Guess:** the Nexus. |
| `+0x1d8 + lane*0x20 + tier*0x10 + side*8` | Lane towers: tier 0 the outer tower, tier 1 the inner; 0 when destroyed. Same layout as section 14's PressTower slots. |
| `+0x238 + side*0x28 + slot*8` | The five roster champions (known). |

### 0x04: Nexus(lane) dropped (`objective_handlers.rs:172`, `0xec17b1`)

**Read from the code.** One decision with objective Nexus(lane) runs:

1. Map-mode gate for the lane; failure is reason 0x03 (`0xec0afd`).
2. `0xeb6df0` (`objective_handlers.rs:1852`) true: replaced by Defense
   (no reason).
3. `0x1053610` (`game_finish.rs:82..99`) true: **kept**, and
   `nexus_basis` (brain `+0x840`) is stored (`0xec0183`). Its inputs,
   skimmed only: the strategy (env slots `0xe8`/`0x108`, several reads),
   the enemy `+0x1c8` entity, allies with at least 40 % hp, a 600-unit
   distance, the tick.
4. If the enemy `+0x1c8` entity exists: `0xf9d6e0`
   (`objective_handlers.rs:137`) lists own living champions within 600
   units of it. Two or more, and one of them with shared-block record
   byte (`+0x808 + k*0x20`) equal to 5: **kept** (`0xec0a03`).
5. The strategy is read fresh (`0xec151f`: env slot `0xe8` picks the
   player copy at `player + 0x388`, else slot `0x108`). The byte used is
   `+0x15`, `game_finish` (`[rbp-0x1b]` in a copy at `[rbp-0x30]`).
6. `game_finish` not 0: the basis `0x1070a50`
   (`objective_handlers.rs:24`) is three counts: own champions alive,
   own champions alive with at least 40 % hp, enemy champions alive.
   - No basis stored yet (the case after a plant through the setter):
     store it and **keep** (`0xec175f`).
   - Basis stored: **keep** while own alive and own healthy have not
     fallen and enemy alive has not risen (`0xec1597`..`0xec15c1`).
7. Otherwise (`game_finish` = 0, or the basis got worse): `nexus_basis`
   cleared, `siege_lock` (brain `+0xaf8`) refilled by `0x1070cb0`,
   `siege_lock_h55` (brain `+0x870`) set to (1, `0x1052d10(..)`) when
   `game_finish` is 0 or (0, ..) otherwise, then reason 0x04 and a clear.

**Read from the code:** the serde name table `0x425bac8` lists
`game_finish` as Stable, Flexible, Aggressive. **Guess:** 0 is Stable.

**For a player:** "not now: the team plays a Stable end game and the
game's own finishing rule does not call for it yet", or "the push has
lost ground since it began: an ally died or fell below 40 % hp, or an
enemy came back".

**Checks against what was played (guess until logged):**
- The 0:43 plant refused at every decision fits a Stable team. A
  Flexible or Aggressive team stores the basis at the first decision
  and keeps the order (step 6).
- Section 12's kept Nexus fits a team that was not Stable, or
  `0x1053610` agreeing.
- The probe should log the team's `game_finish` with each Nexus plant.
- `game_finish` can be changed live through the official strategy
  interface (seen in game for `focused`, dev9). **Guess:** setting it
  to Flexible makes a planted Nexus hold while the basis holds; that is
  a team-wide change, a choice for the design session.

### 0x06: DefenseLine(lane) dropped, nothing to defend (`:214`, `0xec1439`)

**Read from the code.** One decision with objective DefenseLine(lane):

1. Map-mode gate for the lane; failure is reason 0x05 (`0xec0a19`).
2. `0xeb6df0` true: replaced by Defense. Only in this replacement does
   the code scan the shared TeamScene list for a kind-1 scene owned by
   another player (`0xebfde0`..`0xebfe01`).
3. **The anchor.** The own side's outer tower on that lane, else its
   inner tower (`0xec0ff6`..`0xec1017`). With neither, the entity of
   the world `+0x170` list nearest to a fixed lane point (`0x10f4e00`,
   a nearest-pick, not a threshold; points from tables
   `0x40182f8`/`0x4018310`, x and y swapped by side: lane 0 (80, 820),
   lane 1 (144, 817), lane 2 (144, 880) for side 0). No anchor: reason
   0x07 (below).
4. **A threat near the anchor** keeps the order (`0xec1180`..`0xec1422`):
   - an enemy champion within 300 units of the anchor (`0xec11d5`,
     constant `0x14f46b0401`) that env slot `0xf8` reports for the own
     side (**guess:** visible), or that is not reported but whose tick
     at the enemy side's shared block `+0x8a0 + index*8` is at most 120
     ticks old (**guess:** seen in the last 2 s). The test uses the
     enemy's real position in both cases;
   - or `0xe9fd10` (`defense_nexus.rs:824`): the own side's per-lane
     record in the shared block (`+ lane*0x28`) has `+0x10` below -3000
     or `+0x20` at most -3 (**guess:** the lane is losing), and an enemy
     unit of kind 3 (entity `+0x68`) on that lane has its target
     (`+0x88` = 1, `+0x90`) on the anchor (**guess:** minions hitting
     the tower).
5. No threat: reason 0x06 and a clear (`0xec1439`).

**For a player:** "nothing to defend: no enemy champion is near that
lane's tower, and the lane is not being pushed into it".

**The dev15 case.** The test reads enemy champions, enemy units and the
anchor only. It reads neither the lane's own laner nor the controlled
athlete. "Defend mid to everyone" while the controlled athlete was the
mid laner was refused because no enemy was near the mid tower at those
decisions, not because of who held mid. **Guess:** the DefenseLine
kept 27 s on a laner (section 12, dev10 and dev11) had an enemy near
the tower for that time.

### 0x07: DefenseLine(lane) dropped, no anchor (`:220`, `0xec12c4`)

**Read from the code.** Step 3 above found no anchor: both own towers
of that lane are destroyed and the world `+0x170` list is empty
(`0xec10eb`..`0xec12b6`). Reason 0x07 and a clear.

**For a player:** "nothing left to defend there: that lane's towers have
fallen". **Guess:** rare in practice, since the fallback list would
have to be empty too.

After each of the three clears the body calls `0xec3ba0`, which only
writes the game's own trace (it exits at once unless `trace_on`, brain
`+0x830`).

## 16. The HideLine inbox case before the gank setup (read 2026-11-04)

Receive handler `0xe67300`, tags 6 (HideLine) and 7 (HideLineToo), case
`0xe67353`; `bl` is the call's lane (signal byte 1). The handler's sixth
argument (`r13d` in the receive gate `0xe65db0`) is the sender index.

### Planner fields named here

**Read from the code:** `PlanHandler` (the agent's `plan_system`,
Debug formatter `0xf42290`, 86 fields, script `fmtpairs.py`). Brain
offsets:

| Brain offset | Field | Brain offset | Field |
| --- | --- | --- | --- |
| +0xe50 | `obs` | +0x1ee8 | `received_signals` (queue A) |
| +0x14e8 | `pending_global_ult_target` | +0x1f00 | `sub_plan` |
| +0x1500 | `sup_escort` | +0x2a38 | `lane_sync_out` |
| +0x1510 | `journey_dest` | +0x2ab8 | `decide_tick` |
| +0x15f0 | `pending_trait_scene` | +0x2ac8 | `gank_cancel_tick` |
| +0x1690 | `flip_prev_sub` | +0x2b78 | `mf_swap` (u8, then tick at +0x2b80) |
| +0x16a0 | `plan` (the big plan) | +0x2bb0 | `trait_roam_last_tick` |
| +0x1aa0 | `base_plan` | +0x2c4b | `scene_publisher` |
| +0x1ea0 | `misunderstood_received_signals` | +0x2c4f | `gank_relay_seen` |
| +0x1eb8 | `signals` (list C) | +0x2c5e | `gank_cancel_line` |
| +0x1ed0 | `signals_wait` (list D) | +0x2c6d | `trait_ticket_used` |

The full list is in the hand-over note's `planhandler_fields.txt`.

`PassiveLinePlan` (formatter `0x1127710`) at plan `+8`: `line` brain
`+0x17c4`, `hold_wave` `+0x17be`, `waiting` `+0x17bf`, `split_role`
`+0x17c0`, `receiving` `+0x17c1`, `regroup_now` `+0x17c2`,
`cancel_low_hp_sent` `+0x17c3`, `hide_line_too_sent` `+0x17c5`,
`push_basis` `+0x17c7`.

`LineGankerPlan` (formatter `0x10e7320`) at plan `+8`: `signals` +0,
`wait_limit` +0x18, `settled_tick` +0x20, `phase` +0x30,
`exposed_at_start` +0x31, `deadline_said` +0x32, `line` +0x33 (brain
`+0x16db`), `roam_from` +0x34, `dbg_cancel` +0x35.

**Read from the code:** the name run `TopMidBottom` sits next to these
plan field names (`0x404af58`). **Guess** (the order of names, not a
formatter's jump table): Line 0 is Top, 1 Mid, 2 Bottom. That agrees
with section 12's lane numbering.

### The roam part (`0xe67353`..`0xe6782d`)

**Read from the code.** It runs only when all of these hold:

1. The receiver's plan is PassiveLine (raw 3, `0xe67357`).
2. Its own champion is on the map (`0xe6738f`).
3. **The lanes are neighbours** (`0xe67398`..`0xe675c9`): a receiver on
   `line` 1 (Mid) needs a call on lane 0 or 2; a receiver on line 0 or 2
   needs a call on lane 1. A call on the receiver's own lane never runs
   this part, and Top and Bottom never answer each other.
4. `0x1bee010(env, own position, lane)` is false (`0xe67604`).
   **Guess:** the receiver is not already on the call's lane.
5. `0xe761a0` (`lane_ops.rs:669`) agrees. In environment kind 0:
   - after one of the two outer towers on the receiver's own lane fell
     (either side), it returns the own side's lane record byte `+0x24`;
   - while both stand, or when the own lane's map-mode gate fails, it
     asks `0x1105dc0` (`passive_line.rs:1130`). That test is true when
     the enemy's front building on the own lane (outer tower, else
     inner, else the `+0x170` list's first entry, else `+0x1c8`) is
     a kind-4 entity whose target is a kind-3 unit that targets it back
     (**guess:** the own wave is at the enemy tower). In kind 0 only, it
     is also true when the kind-0 object (the second word slot `0x40`
     returns) has a non-zero `+0x240 + side*8`. Outside kind 0 only
     the first test counts.
6. Own hp at least 60 % (`0xe6766f`).
7. The jungler's lane score `0x10e4290` (`passive_jungle.rs:1159`) for
   the call's lane is not negative, or else `0x10cad50(.., 0, 1000)`
   returns at least 350 (**guess:** a 65 % random pass).
8. `0x10e4ca0` (`ganker.rs:529`, which calls the arrival window
   functions `0x10e4df0`/`0x10e5050` of section 13) agrees.
9. No gank on that lane was cancelled in the last 10 s
   (`gank_cancel_line`, `gank_cancel_tick`, `0xe676d3`..`0xe67709`).

Then it writes, in order:

| Write | Site | What |
| --- | --- | --- |
| Reply | `0xe67736` | HideLineToo(lane) (tag 7, u64 0) pushed on `PlanHandler.signals` (list C). **Guess:** "I am coming too". How list C reaches the team was not read (section 8: the broadcast reads `events`, `+0x3c70`). |
| `mf_swap` | `0xe67758` | (0x15, now). |
| New plan | `0xe677ee` | LineGanker on the call's lane: `wait_limit` = now + 10 × delay (10 s), `roam_from` 0xff, the rest 0. Committed by `0xe37db0` (the old plan dropped at `0xe38037`, 0x400 bytes copied at `0xe38048`). |
| `pending_trait_scene` | `0xe72550` | Kind 5 (`+0x168c`), the lane (`+0x168b`), the tick (`+0x1630`, `+0x1640`), the receiver's position (`+0x1648`, `+0x1668`); skipped when the same kind-5 scene for that lane is already pending. Read by `scene_feedback.rs` `0xe6fb40` (it sets bit 2 of `trait_ticket_used` for kind 5); not read further. |

The gank-objective part (section 4) runs after it either way
(`0xe6782d`).

**What it does not write:** `hold_wave`, `waiting`,
`hide_line_too_sent`, or anything that holds the wave, pulls back or
hides. The HideLine case is a roam call, not a lane-hold call.

**How long.** **Read from the code:** the two Korean trace strings at
`0x4048080` read "gank end: window closed, tick ≥ wait_limit" and "gank
folded: lifetime not over (… ticks left until wait_limit), outside
replacement". So the roam lasts until `wait_limit` (10 s) or until
something else replaces the plan. Whether the next decision keeps a
laner's LineGanker plan was not read.

**Environment kind 0.** **Read from the code:** the roam part has no
kind exit. `0xe761a0` and `0x1105dc0` have kind-0 branches that can
pass (gate 5). So all four writes can run in the watched match. Early
in a match, with all outer towers up, the roam needs gate 5's wave test
or the kind-0 flag. **Guess:** the 8 HideLineToo calls dev6 heard in two
matches (`investigation-ai-signals.md`, section 10) are such replies.

**For pings (guess until probed).** One HideLine(lane X) sent to
everyone would ask:
- the laner of X for Gank(X) (section 4);
- the neighbouring laners (Mid for Top or Bottom; Top and Bottom for
  Mid) to roam to X for up to 10 s, if they pass the nine gates;
- nothing of the jungler (plan PassiveJungle).

## 17. The fight plan's exits and the BattleStop record (read 2026-11-04)

### The BattlePlan

**Read from the code.** The Battle variant of the big plan is a
`BattlePlan` at brain `+0x16a0` itself (Debug formatter `0xf0b0c0`, 98
fields; constructor `0xefbc60`). Selected offsets (plan offsets; brain =
`+0x16a0` + offset):

| Offset | Field | Offset | Field |
| --- | --- | --- | --- |
| +0x20 | `well_runaway` (Option, tick at +0x28) | +0x2e0, +0x2e8 | `eval_die`, `eval_reenter` |
| +0x40 | `main_goal` | +0x2f0 | `start_tick` |
| +0xe0 | `sub_goal` (u64 tag first) | +0x3cc, +0x3cd | `with_dive`, `team_dive` |
| +0x100 | `signals` | +0x3d5 | `help_called` |
| +0x280 | `retreat_tick` | +0x3d7 | `dive_abandoned` |
| +0x288 | `hit_by_tick` | +0x3e9 | `last_dir` |
| +0x2b0 | `hit_ally_tick` | +0x3ef, +0x3f1, +0x3fa | `entry_src`, `exit_src`, `exit_sub` |

The full list is in the hand-over note's `battleplan_fields.txt`.

**The 26 battle end reasons** (`0x401ddc8`, section 2) are the names of
`exit_src` (plan `+0x3f1`, brain `+0x1a91`). Both traces print them:
`0xe37db0` reads brain `+0x1a91` for a Battle plan, `0xec3ba0` reads plan
`+0x3f1`. The constructor sets `exit_src` to 14 (spawn retreat) when the
new sub goal's tag is 3, else 0 (`0xefbefd`).

### Who writes each end reason

**Read from the code:** byte stores of a constant at `+0x3f1` or
`+0x1a91` in `0xd00000..0x1300000` (script `stores.py`). The inputs
column names what the few instructions before the store read; the full
conditions were not read.

| Value | Name | Writers | Inputs seen |
| --- | --- | --- | --- |
| 1 | target lost | `0xef1a6a` (`0xeedc10`, `battle.rs:1008..1878`) | a lookup returning none |
| 2 | no enemies left | `0xef1fb6` (`0xeedc10`); `0xefcbe0` (`0xefbfe0`, `battle.rs:700..802`) | a compare with 50 |
| 4 | objective priority | `0xefe6f7`, `0xeff640` (`0xefbfe0`) | the hp of an entity looked up by id through env slot `0x208` (**guess:** the objective monster, as in section 5); `0x10b0e30`, the `object_finish` reader of section 7 |
| 5 | chase outside siege direction | `0xef1502` (`0xeedc10`) | |
| 6 | fountain | `0xefcdb5`, `0xefd02f` (`0xefbfe0`) | position tests against four boxes (sizes 160 and 64 units, offsets 800 and 896 units; **guess:** the fountain areas); sets `well_runaway` = now + 5 s, sub goal tag 3, `dive_abandoned`; the second site fires when `well_runaway` runs out (**superseded 2026-11-04**, section 22: it fires at every think while `well_runaway` is still running) |
| 7 | cannot chase | `0xef2aac`, `0xef3534` (`0xeedc10`); `0xefa02b` (`0xef9e60`) | delay multiples |
| 8 | tower | `0xeee16d` (`0xeedc10`); `0xef61dc`, `0xef6206` (`0xef5f00`, `battle.rs:2772`) | the dive flags `with_dive`, `team_dive`; positions |
| 9 | wave | `0xef67cd` (`0xef62c0`) | |
| 10 | dive | `0xef290b` (`0xeedc10`); `0xef6a8b`; five in `0xef7be0`; `0xef8e80`; `0xefa864` | |
| 11 | resolver lost | `0xef734e`, `0xef74a2`, `0xef74cf` (`0xef7010`, `battle.rs:3075`) | calls `0xf00a60` (`battle.rs:528`) |
| 12 | ladder disadvantage | `0xeee341`, `0xeefe81` (`0xeedc10`); `0xef6f21`; `0xefb8c0` | |
| 13 | re-entry gate | `0xeec951` (`0xeec740`); `0xeedab2`; `0xef36cd` | `0xeec740`: a product of player `+0x288`, `+0x218`, `+0x280` capped at 100 (**guess:** athlete stats) |
| 14 | spawn retreat | constructor `0xefbc60` | sub goal tag 3 at creation |
| 15 | BlockB | `0xe5d3a7` (`engage.rs`, brain `+0x1a91`); `0xef26bb`, `0xef3517` (`0xeedc10`); `0xf000ff` (`0xeff890`, `battle.rs:2976`) | |
| 16 | turn and flee | `0xef4233` (`0xeedc10`) | after `0x10a7670` |

No constant store found for 3 (timeout), 17 (swap: death), 23 (swap: mode
guard) or 25 (swap unknown). `0xeedc10` saves `exit_src` at `0xef4588`
and puts it back at `0xef476c`. **Guess:** the "swap" names describe a
plan replaced from outside, filled in by the trace, not stored.
Search limits: byte stores of constants only; wider stores that cover
`+0x3f1`, and base_plan's copy (`+0x1e91`), were not searched.

**Read from the code:** `exit_src` is written while the plan runs. When a
write actually ends the plan was not read; the step's fight-end rule is
section 13, step 8.

**What makes the AI disengage, by kind of input** (sites above; full
conditions not read):
- **Hp:** objective priority tests an entity's hp (guess: the
  objective monster); the re-entry gate uses an athlete-stat product. **Guess:** turn and flee and resolver
  lost come from the fight model's win estimate.
- **Allies:** the fight model leaves out allies who asked to stop in
  the last 6 s (next part). **Guess:** ladder disadvantage is a numbers
  comparison.
- **Towers:** tower (8) and dive (10).
- **Position:** fountain, chase outside siege direction, cannot chase.
- **Time:** `well_runaway` 5 s; the 4 s re-entry window after
  `retreat_tick` (section 18); the 6 s BattleStop window.
- **A teammate's request:** BattleStop, next part.
- **Strategy:** **read from the code:** the objective-priority exit
  calls `0x10b0e30`, which reads `object_finish` (section 7). The mod
  can change the strategy live.

### The BattleStop record

**Read from the code.** The record the inbox writes in non-watched kinds
is `TeamPlan.ally_battle_stop_tick` (TeamPlan `+0x7a8` = brain `+0x890`):
five 16-byte `Option<u64>` (tag u64 1, tick), indexed by the sender (the
receive gate passes the index it compares with the receiver's own to
skip own calls, `0xe65ddd`). Its neighbours in the TeamPlan formatter
are `my_battle_stop_tick` (brain `+0xd88`), `my_battle_call` (brain
`+0xd90`) and `last_battle_tick` (brain `+0xbb0`), not read.

Writer: the receive handler, `0xe675ad`..`0xe675b9`, only when env slot
`0x40` is non-zero (not kind 0).

Readers (a scan of every memory operand with displacement `+0x7a8`..
`+0x7f0` or `+0x890`..`+0x8d8` in `0xe00000..0x1200000`, script
`dispscan.py`):

| Function | Source | What it does with the record | Kinds |
| --- | --- | --- | --- |
| `0x10abb30` | `fight_model.rs:1387..1466` | Loops over the five allies and leaves out ally i while entry i is set (`0x10abd7f`). | Non-zero kinds. In kind 0 the same loop leaves out ally i while the shared block's `+0xd58 + i*8` holds a tick from the last 6 s (`0x10abcb6`, `0x10abdb6`). |
| `0x10ad780` | `team_plan.rs:696` (called from `engage.rs` `0xe5cdae` and `fight_model.rs` `0x10a5418`) | Reads the entry at the reader's own roster index; when set, skips to an evaluation at `0x10adc1c` (not read). The receive gate drops a player's own calls, so the inbox never writes that entry; this reader's purpose is unresolved. | Non-zero kinds only. |
| `0xf00a60` | `battle.rs:528` (from the resolver-lost exit and `0xeedc10`) | `0xf00b77`: a byte at `+0x7a8` of its fifth argument (base not resolved). `0xf00ba0` reads the shared block's `+0x7a8` (`r12` set at `0xf00b3a`), not the record. | Non-zero kinds only. |
| `0x10a82a0` | `fight_model.rs:1447` | A byte at `+0x7a8` of its fourth argument. | Non-zero kinds only. |
| `0xec9270` | `team_plan.rs` (vision and audit) | Clears each entry once its tick is more than 6 s old (`0xecaf08`..). | Whether this path runs in kind 0 was not resolved. |

Not the record: the compares at `+0x890` that follow a test of `+0x888`
against 5 (in `0xe5b200`, `0xe5c5f0`, `0xecfd40`, `0xed2c50`,
`0xee4e60`, `0xefbfe0`, `0xf00a60`, `0xf994f0`, `0x10aafc0`) read the
shared block's per-player record 4 (`+0x808 + 4*0x20`).

**The watched match's counterpart.** **Read from the code:** about a
dozen planner functions read the shared block's `+0xd58 + i*8` (u64
tick, 0 = none), among them `0xe59200`, `0xe5c5f0`, `0xf8e940`,
`0xf91460`, `0xf924a0`, `0xf92c50`, `0x10a82a0`, `0x10a8c20`,
`0x10aa460`, `0x10abb30`. No planner function writes it (like the rest
of the shared block, section 5). **Guess:** it is the per-ally
BattleStop tick of the watched match, kept by the shared block's
writer.

**What to expect from the research key** that writes (1, tick) at
brain `+0x890 + sender*16` on a teammate:
- **Read from the code:** every reader found in the scanned range
  `0xe00000..0x1200000` (the functions that name game-ai source files
  lie in `0xe35380..0x1125a60`) checks the kind first and skips in
  kind 0. In the watched match the write changes no decision.
- **Expected in the log:** the entry stays as written, or disappears
  about 360 ticks (6 s) later if the expiry path of `0xec9270` runs in
  kind 0. No change in how the teammate fights.
- **The lever for "stop fighting" in kind 0** is the shared block's
  `+0xd58 + i*8` (**guess**). It is shared state with an unknown writer;
  writing it is not recommended before that writer is found. A probe
  worth running: log the own team's shared block `+0xd58..+0xd80` while
  the AI sends BattleStop calls on its own, to see whether a call
  reaches it.

## 18. The fields `retreat_tick` and `on_my_way` (read 2026-11-04)

### `retreat_tick`

**Read from the code.** A field of `BattlePlan` (section 17), plan
`+0x280`: brain `+0x1920` while the plan is Battle. A u64 tick.

| Who | Site | What |
| --- | --- | --- |
| Constructor `0xefbc60` | `0xefbe10` | Set to 0 with its neighbours. |
| `0xefbfe0` (`battle.rs:700..802`) | `0xefce52` | Write: now, when `last_dir` (`+0x3e9`) is 1 and the sub goal's tag is 2 or 3. Reached from the fountain exit; other entries to `0xefce22` not checked (**superseded 2026-11-04**, section 22: the `well_runaway` path reaches it too). **Guess:** the moment the fighter turned to retreat. |
| `0xefbfe0` | `0xefcebf` | Read: with `last_dir` 0xff and sub goal tag 0, a branch runs only while at most 4 × delay (4 s) has passed since `retreat_tick` and `eval_die` is at most `eval_reenter` + delay. **Guess:** a fighter may turn back into the fight within 4 s of retreating. |
| Clone `0xf33630`, formatter `0xf0b0c0` | | Copy and print. |

No other access was found. A scan of `+0x280` in `0xd00000..0x1300000`
kept to functions that also touch `+0x3f1`, `+0x3e9` or `+0x2e0`/`+0x2e8`
found only these. The other `+0x280` reads are player `+0x280` (the stat
product in `0xeec740`, `0xed1ae0`, `0x1027ce0`) or world roster lists
(`0xf025f0`, `0xf04590`, `0xf07180`). No access at brain `+0x1920` or at
base_plan's copy (`+0x1d20`) was found in the planner.

### `on_my_way`

**Read from the code.** Not a structure field. The only occurrence (case
insensitive, also "on my way") is the resource key
`ping_on_my_way_resource` in the game view (`game-view\src\view\game.rs`).
The view's resource setup `0x9dc660` registers five ping resources, all
from one Aseprite file, `asset/base/aseprite_resources/ingame/ping`, each
with an animation tag:

| Resource key | Tag | Registered at |
| --- | --- | --- |
| `ping_battle_resource` | `fight` | `0x9e0b3e` |
| `ping_on_my_way_resource` | `going` | `0x9e3a2a` |
| `ping_warning_resource` | `danger` | near `0x9e2487` |
| `ping_help_resource` | `help` | `0x9e2346` |
| `ping_mia_resource` | `mia` | `0x9e2697` |

Each key has one code reference, the loader. Where the view draws them
was not found by name. **Guess:** these are the base game's own ping
sprites for the AI's calls (the inbox carries Battle, HideLine,
BattleHelp and Mia; section 16 and `investigation-ai-signals.md`). The
design session may want to compare them with the mod's ping glyphs.

## 19. Corrections and open items from this read (2026-11-04)

### Superseded

- **Section 14's table, rows 0x04, 0x06, 0x07:** superseded 2026-11-04
  by section 15.
  - 0x06 is not "a TeamScene owner test". The only owner scan nearby
    belongs to the Defense replacement.
  - 0x07's `0x10f4e00` is a nearest-pick, not a distance test.
- **Section 4's "plan trace" `0xe37db0`** (the scene-kind bullet and
  "Who writes the big plan"): superseded 2026-11-04. It sets the plan: it writes the trace when brain `+0x14c8` is set, then
  drops the old plan and copies the new one into brain `+0x16a0`
  (`0xe3802d`..`0xe38048`). Section 4's table of plan writers should
  count every `0xe37db0` call, including the HideLine roam at
  `0xe677ee`.
- **Section 4, "Two inbox paths set an objective", HideLine:** the
  lane-ops part before the gank setup is the roam of section 16. It
  also switches a neighbouring laner's plan to LineGanker.
- **Section 8:** queue A at brain `+0x1ee8` is
  `PlanHandler.received_signals`; lists C (`+0x1eb8`) and D (`+0x1ed0`)
  are `signals` and `signals_wait`.

### Not read

- `0x1053610` (`game_finish.rs`) in full: when the game's own rule
  finishes the game.
- Shared-block record byte 5 (`+0x808 + k*0x20`) and the per-lane
  records (`+0x10`, `+0x20`, `+0x24`).
- What the world `+0x170` list and `+0x1c8` entity are.
- `0x10e4ca0`, and what the kind-0 object's `+0x240 + side*8` is
  (`0x1105dc0`).
- How `pending_trait_scene` is consumed (`0xe6fb40`), and whether a
  laner's LineGanker plan survives the next decision.
- The battle exits' full conditions, and where 3, 17, 23 and 25 are set.
- The kind-0 condition of the `0xec9270` expiry, and the writer of the
  shared block's `+0xd58 + i*8`.
- Who draws the five ping resources.

Scripts used (scratch, copied to the hand-over note): `dispscan.py`
(every memory operand with given displacements), `fmtpairs.py` (field
names from a `debug_struct_fields_finish` formatter), `callers.py`, and
the earlier session's `ann.py`, `stores.py`, `fmtfields.py`, `jt.py`,
`leas.py`, `strtab.py`, `findfmt.py`.

## 20. Driven windows: how one decision becomes a command (read 2026-11-04)

Question from the design session: three calls (Retreat, Careful / enemy
missing, Hold the lane / on my way) need a teammate to do something for
7 to 15 s. Which of the game's own states produce each, and where would
a direct override sit? Sections 20 to 26 answer that. Nothing here was
played: every line is **read from the code** or a **guess**.

### The decision, from the agent step to the command

**Read from the code.** One "think" of one AI player, in order:

| Step | Function (source) | What it does |
| --- | --- | --- |
| 1 | `0xf19810` (`lib.rs:779..874`) | The agent step. When the tick is below `next_input_tick` (brain `+0x47c0`) it returns "no input" at once (`0xf198b5`). Otherwise it sets the next think time from two player values (`+0x288` × `+0x200`; **guess:** athlete stats giving a reaction interval), copies the world's trace byte (`[world + 0x3c]`) into brain `+0x14c8` and `+0x830` (`0xf19a00`..`0xf19a0d`), stamps `decide_tick` (`+0x2ab8`), and calls step 2. |
| 2 | `0xf16d60` (`lib.rs:730`) | The per-player step. It calls the broadcast `0xf163a0` at `0xf16de7`, **the mod's existing hook**, then the planner. |
| 3 | `0xe50330` (`handler.rs:537`) | Calls the planner step `0xe381b0` (`0xe50418`). |
| 4 | inside `0xe381b0`, kind-0 path (from `0xe3872b`) | In this order (script `reach2.py`: each later site is reachable from the earlier one and not the other way): the cascade (`0xe404c5`), the ForcePassive rebuild (`0xe407c4`), the slot-4 hold rule (`0xe40a6b`, section 21), the recover switch (`0xe41260`, section 22). |
| 5 | `0xf15970` (no source line), through `0xe4eaf0` | Runs the current big plan's update, by plan: PassiveLine `0x1103560` (`passive_line.rs:67..446`), Battle `0xefbfe0` (`battle.rs:700..802`), LineGanker `0x10d53d0`, and others. On the kind-0 path it is called through `0xe4eaf0` at `0xe419bd` or `0xe438a0`; every path to the commit below passes one of them. The other two callers (`0xe39ab4`, `0xe3a9b3`, each followed at once by a choice and a commit) are not reachable from the kind-0 entry. |
| 6 | `0xf15f10`, then `0xf24690` | Builds the sub plan from the big plan (table below) into a stack slot (`0xe44584`, or `0xe44772`, `0xe44996`, `0xe454ac` after late plan commits at `handler.rs:1298`, `:1311`, `:1328`) and commits it to brain `+0x1f00` at `0xe44e20`, the last planner step of the think. |
| 7 | `0xe6fb40`, `0xe74370` | Scene feedback, observation (from `0xf16d60`). |
| 8 | `0x104a1e0` (`small_action/*.rs`) and the cache rule `0xf171cc`..`0xf175b1` | Keeps the current small action without a new choice when all of these hold: it is still valid; the tick is within 10 of `last_eval_tick` (`+0x47c8`); the environment kind is not 2; it is not Attack, Skill, Skill2 or Ult with byte `+0x2c98` set; own hp is at least `last_eval_hp` (`+0x47d0`); the nearby-enemy count (`+0x47e0`), the plan (`+0x3c18`..`+0x3c38`) and the sub plan (`+0x2c70`/`+0x2c78`) are those of the last choice. Then only the action's tick is refreshed. |
| 9 | `0xf178c0` (`lib.rs:679..696`), called at `0xf1741e` | Otherwise chooses a new small action. The auction `0xe678c0` (`auction.rs`) asks the sub plan's step (dispatcher `0xf21f70`) for candidates and scores them. The winner goes to `small_action` (brain `+0x2c80`), its score to `+0x4788`, and the plan and sub plan of this choice are recorded (`+0x3c18`, `+0x2c70` = 1, `+0x2c78`). |
| 10 | `0x104a990` (`small_action.rs:180..216`), called at `0xf19c52` | Back in the agent step: turns `small_action` into the command for the simulation (the agent step's out struct; first word −1 = none). One executor per kind: RunAway and the move kinds `0x101b470` (`move_actions.rs`), Skill `0xe8fab0`, Skill2 `0xe8fcc0`, Ult `0xe8fee0` (`cast.rs`), Trace `0xf766c0`, Around `0x10fdb80`, and so on. |
| 11 | `0xf19d92`, `0xf19f78`, then `0xf1a0bd` | When the executor gives no command: choose again (step 9) and execute again. When there is still none, and the own champion is inside the fountain box and hurt: `0xe4ec70` (`handler.rs:369`) and choose again. Last fallback: the step writes a RunAway to the own fountain corner in place (next part) and executes it. |

So the hook at `0xf16de7` runs **before** steps 3 to 9 of the same
think:
- anything it writes into the plan is read by that think's plan update
  and sub plan choice, unless a rule of step 4 or the update rewrites it
  first;
- anything it writes into `small_action` is chosen over in step 9,
  unless the cache rule keeps it.

### The small action (brain `+0x2c80`)

**Read from the code:** enum Debug formatter `0xf40aa0`, jump table
`0x4026e24`. Tag u64 at `+0x2c80`; payload from `+0x2c90` (kinds 0..9)
or `+0x2c88` (kinds 10..13).

| Tag | Kind | Payload (formatter) |
| --- | --- | --- |
| 0 | RunAway | `SmallActionRunAway` (`0x103f080`), table below |
| 1 | Recall | `SmallActionRecall` (`0x103edb0`): start_tick, goal_x, goal_y, path_finder, end_delay, goal_committed, goal_risk, prog_best_dist, prog_best_tick |
| 2 | Around | `SmallActionAround` (`0x1126a20`): start_tick, target, goal_x, goal_y, goal_gain, path_finder, range, end_delay, escape_mode, position_eval_purpose |
| 3 | AroundRegion | |
| 4 | AroundRunAway | |
| 5 | AroundPosition | `SmallActionAroundPosition` (`0x1127200`) |
| 6 | AroundPositionBush | |
| 7 | AroundBush | |
| 8 | LaneMinionPosition | `SmallActionLaneMinionPosition` (`0x10741c0`): start_tick, target, goal_x, goal_y, goal_score, path_finder, end_delay, position_eval_purpose, line |
| 9 | Trace | |
| 10 | Attack | `SmallActionAttack` (`0xea1a00`): start_tick, target (+8), is_act (+0x10) |
| 11, 12, 13 | Skill, Skill2, Ult | the same three fields |
| 14 | Stop | none |

RunAway, brain offsets (enum `+0x10` + field):

| Brain offset | Field | Brain offset | Field |
| --- | --- | --- | --- |
| +0x2c90..+0x2cef | `path_finder` (0x60 bytes; state byte at +0x2ce5) | +0x2d10 | `goal_risk` |
| +0x2cf0 | `start_tick` | +0x2d18 | `prog_best_dist` |
| +0x2cf8, +0x2d00 | `goal_x`, `goal_y` (1000 per unit) | +0x2d20 | `prog_best_tick` |
| +0x2d08 | `end_delay` | +0x2d28, +0x2d29 | `with_skill`, `with_ult` |
| | | +0x2d2a, +0x2d2b | `dodge_trajectory`, `goal_committed` |

**The drop.** `0xf0dcd0` drops a small action. For kinds 0..9 it frees
three heap buffers of the path finder (process heap, `HeapFree`), unless
the path state byte (enum `+0x65`, or `+0x75` for some kinds) is 2.
Kinds 10..14 own no heap.

**The game's own in-place RunAway** (`0xf1a0bd`..`0xf1a128`, step 11):
1. drop (`0xf0dcd0`);
2. tag 0, path state `+0x2ce5` = 2 (no path yet), `start_tick` = now;
3. goal = the own fountain corner: (32, 928) for side 0, (928, 32) for
   side 1;
4. `end_delay` 5, `goal_risk` 0, `prog_best_dist` −1, `prog_best_tick`
   0, `with_skill` 1, the other three bytes 0;
5. then the executor.

This is a complete, game-made template for a small action written by a
mod.

### The sub plan (brain `+0x1f00`)

**Read from the code:** formatter `0xf463c0`, jump table `0x4026f0c`.
The tag is the u64 at `+0x1f00`: `0x8000000000000000 | index`, except
Jungle, whose payload uses the word itself (any value without the top
bit). Payload from `+0x1f08`.

| Index | Sub plan | Payload |
| --- | --- | --- |
| 0 | LineDefense | style, receive, line, minion_action_type, siege, mistake_tick |
| 1 | LineSafe | `line` (+0x1f08) |
| 2 | LineWait | `at_tower` (+0x1f08), `line` (+0x1f09) |
| 3 | Recall | none |
| 4 | Jungle | (niche) |
| 5 | Battle | |
| 6 | DeathBattle | |
| 7 | Hide | arrival tick, flag |
| 8..13 | EpicCheck, EpicHunt, EpicPoke, SerpenCheck, SerpenHunt, SerpenPoke | |
| 14, 15, 16 | AttackNexus, DefenseNexus, ObjContest | |

**Which plan gives which sub plan** (`0xf15f10`, jump table `0x40267c8`,
index = plan raw − 2, Battle = 7):

| Big plan | Sub plan |
| --- | --- |
| ForcePassive (2) | Recall, always |
| PassiveLine (3) | chosen by `0x1105f80` (section 21) |
| SinglePlanLine (4) | `0xff5490` (`single_line.rs`) |
| SinglePlanBattle (5), DeathMatchBattle (6) | Battle, DeathBattle (payload copied) |
| PassiveJungle (7) | `0x10d0050` (`passive_jungle.rs`) |
| ActiveRecall (8) | Recall, always |
| Battle (0, 1) | `0xf00610` (`battle.rs:3537`) |
| LineGanker (10) | Hide (arrival tick from `0x10e4df0`) |
| EpicHuntAndPoke, SerpenHuntAndPoke | `0xee8960`, `0xee9c40` |
| AttackNexus, DefenseNexus | `0x10ff290`, `0xe8f770` |

**The commit `0xf24690`.**
- A different kind: drop the old one (`0xe349c0`) and copy.
- The same kind (jump table `0x4026cc0`): LineSafe takes the new `line`,
  LineWait the new `at_tower` and `line`, Recall keeps itself.
- No sub plan field is sticky across choices.

**What each sub plan offers** (dispatcher `0xf21f70`; candidates are
0xc0-byte small actions):

| Sub plan | Step | Candidates |
| --- | --- | --- |
| Recall | `0x1071410` (`recall.rs:35`) | Recall to the own fountain corner (end_delay 5); then `0x1022470` (`fight_check.rs:646`), `0xf8aa10` with the own champion, `0x10248f0` (`fight_check.rs:824`). |
| LineSafe | `0x10831b0` (`line_safe.rs:24`) | Around (range 80 units, end_delay 5) the own front tower of `line`; or around entity X of the side's per-lane record (shared block `+ lane*0x28`, tag 1, id at +8) when X is farther from the own Nexus than that tower. Then `0xfd2590` (`line_wait.rs:150`), which adds `fight_check.rs` 646, 782 and 824, lane minions (`0x10639c0`, `lane_minion.rs:114..`) and tower discipline (`0x10de930`, `tower_discipline.rs:583`). |
| LineWait, `at_tower` 1 | `0xfe5690` (`line_wait.rs:34..103`) | Around the own front tower, always (`0xfe57a7`). Then `0xfd2590` as above. |
| LineWait, `at_tower` 0 | same | Around the tower when X is behind it or within 180 units of it; otherwise a point on the lane's path by X (X must be a kind-3 unit, `+0x68`). Then `0xfd2590`. |

"Own front tower" is `0x10db3e0(world, lane, side)`: the outer tower of
that lane (world `+0x1d8 + lane*0x20 + side*8`), else the inner
(`+0x10`), else the nearest of the world `+0x170` list. **Guess:** X is
the side's front minion on that lane (kind 3 is the minion kind of
section 15).

**Guess, from the candidate sources:** in all three sub plans the
athlete still farms, trades and casts. The `fight_check.rs` and
`lane_minion.rs` candidates stay in the auction next to the positioning
one. The scoring was not read, so how often a trade wins over standing
at the tower is unknown.

## 21. The PassiveLine flags (read 2026-11-04)

### The fields, brain offsets

**Read from the code:** `PassiveLinePlan` formatter `0x1127710` (script
`fmtfields.py`); payload at plan `+8` = brain `+0x16a8`. The stashed
copy in `base_plan` is `+0x400` further.

| Field | Brain offset | Field | Brain offset |
| --- | --- | --- | --- |
| `in_recall` | +0x17b8 | `split_role` | +0x17c0 |
| `commit` | +0x17b9 | `receiving` | +0x17c1 |
| `flee` | +0x17ba | `regroup_now` | +0x17c2 (`base_plan` +0x1bc2) |
| `flee_acute` | +0x17bb | `cancel_low_hp_sent` | +0x17c3 |
| `flee_approached` | +0x17bc | `line` | +0x17c4 (`base_plan` +0x1bc4) |
| `flee_cover` | +0x17bd | `hide_line_too_sent` | +0x17c5 |
| `hold_wave` | +0x17be (`base_plan` +0x1bbe) | `flee_entry` | +0x17c6 |
| `waiting` | +0x17bf (`base_plan` +0x1bbf) | `push_basis` | +0x17c7 |

Vecs (heap): `signals` +0x16c0, `recall_committers` +0x16d8,
`flee_threats` +0x16f0, `lane_recall_pending` +0x1708. u64s:
`flee_hp_entry` +0x17a8, `flee_hp_min` +0x17b0.

### The sub plan choice `0x1105f80` (`passive_line.rs:1016..1123`)

**Read from the code.** The first match wins:

| Order | Test (site) | Sub plan |
| --- | --- | --- |
| 1 | `in_recall` set (`0x1105fab`) and own hp below max | Recall |
| 1b | `in_recall` set, hp full | Recall when `0xffd4e0` and `0xeb0860` are both true; else LineWait(`at_tower` 0) |
| 2 | `regroup_now` set (`0x1106003`) | **LineWait(`at_tower` 1)** |
| 3 | `hold_wave` set (`0x110601c`) | **LineWait(`at_tower` 0)** |
| 4 | `flee` = 1 and `flee_cover` = 0 (`0x1106084`) | LineWait(0) when `flee_acute`, else **LineSafe** |
| 5 | otherwise | the rest of the function: TeamScene, Gank and mode tests (not read). **Guess:** LineDefense, the usual laning. |

`line` is always the plan's own `line`. So `regroup_now` and `hold_wave`
override the laning sub plan outright, with nothing else tested.

### Who writes them, and whether a hook write survives

**Read from the code:** every store whose destination covers the field
(script `coverstores.py`, any width, any base) in `0xe30000..0x1130000`.

| Field | Writers | Rewritten every think? | A write at the step hook |
| --- | --- | --- | --- |
| `regroup_now` | Plan builders only: `0xe56430` (dword at payload +0x118 = 0) and whole-payload copies in `0xe520d0`, `0xe56ab0`, `0xe573c0`, `0xe52ab0` (`0xe53f43`). The carry-over `0x10ff3d0` (from `0xe74980`, after a fight) does not copy it. | No | **Survives** until the plan is replaced, and is read by the same think's sub plan choice (step 6 runs after the hook). Its only reader is that choice (`0x1106003`). |
| `hold_wave`, `waiting` | Builders: `0xe56430` writes both 1 (with `push_basis` 5) in its fallback lane pick, 0 otherwise. The handler's support rule `0xe40a6b`..`0xe40dd3`. | Only for roster slot 4 | **Survives** for roster slots 0, 2 and 3. For slot 4, whenever the rule's gates pass, the rule runs before the choice (step 4 before step 6) and overwrites the mod's write before it is read. |
| `in_recall` | `0x1103560` at `0x11035bc`: `in_recall = 0x10ff610(..)` (`passive_line.rs:1191..1359`), at every PassiveLine update | Yes | **Rewritten** in step 5, before the choice. Not a lever. |
| `flee` group | `0x1103560` (`0x110463f`, `0x1104ff6` clear; `0x1105725`, `0x11056a7` set), together with `flee_threats` and `flee_entry` | Yes (an episode kept by the update) | **Rewritten**, and tied to a heap Vec. Not a lever. |

**The support rule** (`0xe40a6b`, kind-0 branch) runs only when all of
these hold:
- the roster slot (`[rbp+0x1b90]`, player `+0x8b0`) is 4;
- the plan is PassiveLine and `split_role` is 0;
- the map mode is in mask `0x1a1`, and the tick is past the first Serpen
  time minus 30 s (`world + 0x8a8`, as in section 13);
- the objective is not Gank or Dive on the own lane.

It then sets `hold_wave` from `0xfcdb60` and `0xe946b0` (`lane_ops.rs`,
a `minion_wave` reader in section 7), and `waiting` = `hold_wave` and
`push_basis` ≠ 7. **Guess:** the bottom support holding the wave for
its carry.

**Where `regroup_now` = 1 comes from.** **Read from the code:** no store
of the value 1 was found, only the builders' zeroes and their 16-byte
copies of a computed payload. **Guess:** the game sets it inside one of
those copies (a builder's regroup case). The LineWait(`at_tower` 1) sub
plan is the game's own code, but the game was not seen to enter it.

**Other readers.** `engage.rs` `0xe5c5f0` reads "holding" (plan is
PassiveLine and `waiting` or `hold_wave`) at `0xe5e151` and `0xe5e781`.
It then picks a radius of 450 units instead of 250 for a test that was
not read. **Guess:** a holding laner judges fights over a wider area.

**When is a PassiveLine replaced?** **Read from the code, in part.**
- The plan setter `0xe37db0` always drops the old plan (`0xe34850`) and
  copies the new one. There is no "same plan, keep the flags" path.
- The handler rebuilds from the objective when the plan is ForcePassive
  (`0xe407c4`: `cmp dword [plan], 2`, then `0xe52ab0` and `0xe56ab0`,
  committed at `0xe408e5`, `handler.rs:1002`).
- The other commit sites of section 4 (relays, LineGanker roams, fights)
  also replace it.
- A plain laning think with nothing changing was not seen to rebuild.

**Guess:** the flags last until the next objective, relay, roam or fight
change. A driven window should re-assert them every think, which costs
one byte write.

## 22. Retreat (read 2026-11-04)

### In a fight: the fight plan's own run-away timer

**Read from the code.** `BattlePlan.well_runaway` is a run-away latch
inside the fight plan's update `0xefbfe0`. It is an Option: tag u64 at
plan `+0x20`, tick at `+0x28`; brain `+0x16c0` and `+0x16c8` while the
plan is Battle.

| Site | What |
| --- | --- |
| `0xefcd99`..`0xefce01` | The fountain exit: the fighter stands in one of the four fountain boxes. Sets `sub_goal` = RunAway, `exit_src` = 6 (fountain), `dive_abandoned`, and `well_runaway` = (1, now + 5 s). |
| `0xefd014`..`0xefd05f` | Every later think outside those boxes, while `well_runaway` is set and now is below its tick: `exit_src` = 6, `sub_goal_src_line` (+0x3ca) = 794, `sub_goal` (+0xe0) = 3 RunAway, `retreat_kite` (+0x3da) = 0. Then the common path at `0xefce22`. |
| `0xefd8ae` | Once the tick is reached: `well_runaway` cleared, and the normal fight logic runs from there. |
| constructor `0xefbc60` | A new BattlePlan starts with it clear. |

**The sub goal enum** (formatter `0xf0bb00`, jump table `0x401c9b4`):
0 Trace(focus), 1 Kiting, 2 KitingBack, 3 RunAway, 4 End.

So writing `well_runaway` = (1, now + 7 s) on a teammate whose plan is
Battle makes that teammate's own fight plan run away for 7 s, then hand
back to its own judgment, with nothing for the mod to clear.

**The plan stays Battle.** **Read from the code:** the plans' own done
test `0xf158a0` (jump table `0x402672c`) ends a Battle only when its sub
goal is End (case 7, `0xf15928`: `cmp dword [plan+0xe0], 4`). RunAway
(3) does not end it, so `exit_src` 6 here is a label, not an exit.
(Section 17 left open when a label ends a plan: for the plan's own test,
never; the handler's other rules can still replace the plan.) The same
test treats ForcePassive as done at once (case 0) and never ends
PassiveLine or SinglePlanLine (cases 1, 2).

**Fighting back.** **Guess:** the Battle sub plan's step `0x1014b50`
(`sub_plan/battle.rs`) calls `fight_check.rs:1024` (`0x1027090`), and
the RunAway small action carries `with_skill` and `with_ult`. How the
RunAway sub goal weighs attacks against fleeing was not read.

**Survival.** A new Battle plan (the scene opener's raw 0 → 1 rule, a
new fight) starts clear. So the window should re-assert the latch at
every think while the plan is Battle and the latch is missing.

**Other writers.** **Read from the code, sample-checked:** a
covering-store search (`coverstores.py`) over every function naming
`big_plan/battle.rs`, `fight_model.rs` or `sub_plan/battle.rs` finds the
(tag, tick) pair written only by the three sites above in `0xefbfe0`.
The other stores at `+0x20`/`+0x28` that were opened write through other
structs: a cursor at `[x + 0x10] + 0x20` in `0xeedc10`, output records in
`0xfba890`, `0x109df30`, `0x10aa460`. Not every one of them was opened.
A search for brain `+0x16c0..+0x16d0` across the planner finds only a
PassiveJungle payload byte (`0xe3eaa8`) and the brain clone.

### The 4 s re-entry window

**Read from the code** (refines section 18, see section 26).
- `last_dir` (+0x3e9) is set at `0xefd010`/`0xefd064` from the sub
  goal: 0xff for KitingBack and RunAway, 1 for Trace, unchanged for
  Kiting and End.
- `retreat_tick` is stamped at `0xefce52` when the last direction was 1
  and the sub goal is now KitingBack or RunAway: the moment a chase
  turns into a retreat.
- The 4 s branch (`0xefcebf`) runs when the last direction was 0xff and
  the sub goal is now Trace: a fighter turning back. Within 4 × delay of
  `retreat_tick`, and while `eval_die` is at most `eval_reenter` +
  delay, its sub goal is replaced by a value computed earlier in the
  step (`0xefcfe2`, not read).

For a driven retreat of 7 s, `retreat_tick` is stamped at the first
forced think (when the fighter was chasing). When the latch expires the
4 s have passed, so the branch does not fire, and the teammate is free
to turn straight back into the fight if its own fight model says so.
**Guess:** that is the "return to the AI's own judgment" the design
wants. A window shorter than 4 s would let the re-entry branch judge the
turn back instead.

### Out of a fight

**Read from the code.** Three native routes exist; none fits a healthy
teammate for exactly 7 s without help.

| Route | Writes | What the game does | Limits |
| --- | --- | --- | --- |
| `regroup_now` on a PassiveLine | brain `+0x17c2` = 1 | LineWait at the own front tower (section 21). | Laners only (plan PassiveLine). Walks to the tower, not to the base. |
| `recover_commit` (PlanHandler, brain `+0x2c48`) | 1 | `0xe4f040` (`handler.rs:1022`, called at `0xe41260` and `0xe43871`) commits ActiveRecall(`to_fountain` 0) when the plan index is in mask `0x1923` (ForcePassive, PassiveLine, PassiveJungle, LineGanker, AttackNexus, DefenseNexus). It moves the plan's signals to list C and sets `mf_swap` (0x10, now). The game sets the flag in `engage.rs` (`0xe5ca04`, `0xe5db27`, `0xe5ed6d`). | Cleared earlier in the same think (`0xe401ab`..`0xe40300`) when own hp is full and `0xe59790` (`engage.rs:349`, not read) is false, or within 160 units of the fountain corner, or when `0xe9abe0` (`defense_nexus.rs`) is true. A healthy teammate is not sent. |
| ActiveRecall(`to_fountain` 1) as the plan | plan tag 8, byte +8 = 1, through the setter `0xe37db0` | Sub plan Recall: the Recall small action to the fountain, plus the `fight_check` candidates. Its end test `0xff6060` (`active_recall.rs:25`, through `0xf158a0`) ends it at full hp when `to_fountain` is 0, or once inside the fountain box. With `to_fountain` 1 and outside the box it asks `0x1094260` (not read). | A plan write: the old plan's Vecs must be dropped, which `0xe37db0` does (`0xe34850`, then a 0x400 copy; it also allocates a trace record when brain `+0x14c8` is set). Walks all the way home; the mod ends it by writing ForcePassive, which the handler rebuilds from the objective in the same think (`0xe407c4`). |

**ForcePassive** (raw 2, no payload) is the handler's "rebuild me".
Five relay and handler sites write it (`relays.rs:94`, `:625`, `:753`,
`:872`, `handler.rs:970`), and `0xe407c4` turns it back into the role's
plan. Its own sub plan is Recall, but it is an end signal, not a state:
on the kind-0 path the rebuild (`0xe407c4`) runs before the plan update
and the sub plan choice, and the done test `0xf158a0` counts it as done.
A ForcePassive written at the step hook is therefore rebuilt in the same
think (**read from the code**; the relay sites that write it may sit
after the rebuild, not traced).

## 23. The override route (read 2026-11-04)

**Read from the code.**

- **Where the plan becomes a command:** `small_action` (brain
  `+0x2c80`), chosen in step 9 and executed in step 10 (section 20). The
  events lists (`+0x3c70`) carry calls to teammates, not commands.
- **A write at the existing hook** (`0xf16de7`) is chosen over. Step 9
  rewrites `small_action` in the same think unless the cache rule keeps
  it, and the cache rule needs the plan and sub plan of the last choice,
  and the hp and nearby count, unchanged. Not honoured in general.
- **A write after step 9** is executed. Between the return of
  `0xf16d60` (`0xf19a37`) and the executor call (`0xf19c52`) the agent
  step only times (the position ring is recorded before the call, at
  `0xf19a6a`..`0xf19be5`). The next think chooses
  again, so an override must be written at every think of the window.
  Two sites would carry it, each one instruction:
  - `0xf19a32`, the agent step's `call 0xf16d60` (`e8 29 d3 ff ff`):
    after the whole planner and chooser, before the executor, with
    brain (`rcx`), context, player and sim pair in hand. It buys a write
    that the same think executes. The retry paths (`0xf19d92`,
    `0xf19f78`) choose again only when the executor gives no command.
  - `0xf1741e`, `0xf16d60`'s `call 0xf178c0` (the chooser). Skipping the
    chooser for a driven teammate is "answering the think", the shape of
    the other mod's log ("answered 872 thinks, left 388 to the AI"). It
    misses the cache path (step 8) and the retries.
- **Casting.** The AI casts only through the small actions Attack,
  Skill, Skill2 and Ult (tags 10..13) and the RunAway flags
  `with_skill`, `with_ult`. An override that writes RunAway or a move
  every think replaces those, which explains why the other mod's driven
  allies stop casting. **Guess:** an override that keeps the chooser's
  pick when it is tag 10..13 and replaces only the move kinds would keep
  casting, at the price of the teammate stepping back into range to
  cast.
- **The cheapest safe form** is the game's own template (section 20):
  call the drop `0xf0dcd0` on `+0x2c80`, then write tag 0 and the
  RunAway fields with path state byte 2. Writing over a move kind
  without the drop leaks three path buffers each time. Tags 10..14 need
  no drop.
- **The raw command** is the agent step's out struct. Writing it is a
  third site, below `0x104a990`. It skips the AI's path finding and is
  not recommended.

## 24. Recipients and cost (read 2026-11-04)

**Read from the code** (sections 12, 13, 15 and 16 have the rest):

| Need | Field |
| --- | --- |
| Side, roster slot | player `+0x820`, `+0x8b0` |
| A laner's lane | the plan's `line`: brain `+0x17c4` while the plan is PassiveLine, `+0x1bc4` in `base_plan` during a fight. Roster slot to lane: table `0x202010000` (slot 0 Top, 2 Mid, 3 and 4 Bottom). |
| The plan kind | brain `+0x16a0` (section 3); `base_plan` `+0x1aa0` during a fight |
| Tower positions | world `+0x1d8 + lane*0x20 + tier*0x10 + side*8` (0 = destroyed), entity `+0x658`, `+0x660` (1000 per unit). The game's own "front tower" choice is `0x10db3e0`. |
| Champion positions | world `+0x238 + side*0x28 + slot*8`, then `+0x658`/`+0x660` (the step hook's existing reads) |
| Neighbours | Mid's neighbours are Top and Bottom; a side lane's neighbour is Mid. This is the game's own rule for the HideLine roam (section 16). |

**Cost per driven teammate per think** (read from the code; not timed):

| Route | Work |
| --- | --- |
| `regroup_now`, `hold_wave` | Read the plan tag, write one byte (two for `hold_wave` and `waiting`). |
| `well_runaway` | Read the plan tag and two words, write two words. |
| ActiveRecall plan | One call to `0xe37db0` (drop the old plan's Vecs, copy 1 KB), and a second one to end it. |
| Override | A new hook; a drop call and about twelve stores at every think of the window. |

## 25. Recommendation per call (2026-11-04)

What the game does is **read from the code**; how it looks in a match
is a **guess**. Nothing here was played.

### Hold the lane / on my way (12 s)

- **Route:** native, on the laner at the point. Pick the own laner
  nearest to the point whose plan is PassiveLine and whose `line` is the
  point's lane. Write `hold_wave` (brain `+0x17be`) = 1 and `waiting`
  (`+0x17bf`) = 1, the pair the game's own builder writes. At the
  window's end, write back the values saved at its start, but only when
  `line` and `push_basis` (`+0x17c7`) still match the saved ones; a plan
  rebuilt during the window keeps its own values.
- **Window:** re-assert at every think for 12 s. Skip a think whose plan
  is not PassiveLine; when the plan comes back, write again.
- **The bottom support (roster slot 4):** refused once the game's own
  support rule runs (map mode in mask `0x1a1`, from the first Serpen
  time minus 30 s): the rule decides `hold_wave` itself before the
  choice. Before that time the write works as for the other laners.
- **Effect:** LineWait(`at_tower` 0). The laner stays with its own
  wave, or at the tower when the wave is under it, and still farms and
  trades.
- **Refused or paused:** any think whose plan is not PassiveLine (a
  fight, a LineGanker roam, an objective play). The hold ends when the
  teammate's own plan changes, which is honest.

### Careful / enemy missing (15 s)

- **Route:** native, on the neighbouring laners (Mid ping: the Top and
  Bottom laners; side-lane ping: the Mid laner), each with plan
  PassiveLine. Write `regroup_now` (brain `+0x17c2`) = 1 for 15 s, then
  0.
- **Effect:** LineWait(`at_tower` 1): around its own lane's front
  tower, range 80 units, with farm, fight-back and tower-discipline
  candidates. The flag has no other reader, so nothing else changes.
- **Refused:** a teammate not in PassiveLine at that think (the jungler,
  a roam, a fight) is skipped for that think; re-assert while the window
  lasts.

### Retreat (7 s)

- **In a fight** (plan raw 0 or 1): native. Write `well_runaway` =
  (1, window end) at brain `+0x16c0`/`+0x16c8`, re-asserted while the
  plan is Battle and the latch is missing. The fight plan runs away until
  the tick, clears the latch itself and goes back to its own judgment.
  The 4 s re-entry branch does not fire after 7 s.
- **Laning, not fighting** (plan PassiveLine): `regroup_now` as for
  Careful, for 7 s: back to the own tower. Also write it into
  `base_plan` (`+0x1bc2`) when a fight has stashed a PassiveLine, so the
  restored plan keeps it. **Guess:** the restore at `0xe74980` is a
  plain copy unless it rebuilds; not read in full.
- **Other plans** (the jungler, objective plays, LineGanker): no flag
  route. Either leave them to their own plan (honest, refused), or write
  the plan ActiveRecall(`to_fountain` 1) through `0xe37db0` and end it
  with ForcePassive. This is the only route here that calls a game
  function with a plan buffer.
- **Override** (section 23): only if the native routes prove too weak in
  play. It needs a new site, and it stops casting unless it keeps tags
  10..13.

### Verifier anchors (expected bytes, 0.7.0)

**Read from the code** (`insnbytes.py`), for
`tools/native_profiles/0.7.0.json` later:

| Site | Bytes | Instruction | Guards |
| --- | --- | --- | --- |
| `0x1105fab` | `80 ba 10 01 00 00 00` | `cmp byte [rdx+0x110], 0` | `in_recall` at payload +0x110 |
| `0x1106003` | `80 ba 1a 01 00 00 00` | `cmp byte [rdx+0x11a], 0` | `regroup_now` |
| `0x110601c` | `80 ba 16 01 00 00 00` | `cmp byte [rdx+0x116], 0` | `hold_wave` |
| `0x1106084` | `80 ba 12 01 00 00 01` | `cmp byte [rdx+0x112], 1` | `flee` |
| `0x11060b1` | `48 b8 02 00 00 00 00 00 00 80` | `movabs rax, 0x8000000000000002` | LineWait tag |
| `0xe40a6b` | `83 bd 90 1b 00 00 04` | `cmp dword [rbp+0x1b90], 4` | the slot-4 hold rule |
| `0xe40ba8` | `41 80 ba c0 17 00 00 00` | `cmp byte [r10+0x17c0], 0` | `split_role` at brain +0x17c0 |
| `0xefd014` | `80 7f 20 00` | `cmp byte [rdi+0x20], 0` | `well_runaway` tag |
| `0xefd025` | `48 3b 47 28` | `cmp rax, [rdi+0x28]` | `well_runaway` tick |
| `0xefd8ae` | `48 c7 47 20 00 00 00 00` | `mov qword [rdi+0x20], 0` | the latch clears itself |
| `0xe37e78` | `0f b6 86 91 1a 00 00` | `movzx eax, byte [rsi+0x1a91]` | BattlePlan at brain +0x16a0 (`exit_src` +0x3f1) |
| `0xf15928` | `83 b9 e0 00 00 00 04` | `cmp dword [rcx+0xe0], 4` | Battle done only at sub goal End |
| `0xe419bd` | `e8 2e d1 00 00` | `call 0xe4eaf0` | kind-0 plan update |
| `0xe44e20` | `e8 6b f8 0d 00` | `call 0xf24690` | kind-0 sub plan commit |
| `0xefcebf` | `48 2b 97 80 02 00 00` | `sub rdx, [rdi+0x280]` | `retreat_tick` |
| `0xe407c4` | `83 38 02` | `cmp dword [rax], 2` | ForcePassive rebuild |
| `0xe4f049` | `80 b9 48 2c 00 00 01` | `cmp byte [rcx+0x2c48], 1` | `recover_commit` |
| `0xff6124` | `80 39 00` | `cmp byte [rcx], 0` | ActiveRecall `to_fountain` |
| `0xe3802d` | `48 81 c6 a0 16 00 00` | `add rsi, 0x16a0` | the plan setter's drop and copy |
| `0xf16de7` | `e8 b4 f5 ff ff` | `call 0xf163a0` | the existing hook (already guarded) |
| `0xf19a32` | `e8 29 d3 ff ff` | `call 0xf16d60` | candidate override site |
| `0xf1741e` | `e8 9d 04 00 00` | `call 0xf178c0` | the chooser |
| `0xf1a0bd` | `e8 0e 3c ff ff` | `call 0xf0dcd0` | the drop before the game's RunAway |
| `0xf1a0c2` | `49 c7 85 80 2c 00 00 00 00 00 00` | `mov qword [r13+0x2c80], 0` | `small_action` at +0x2c80 |
| `0xf1a0cd` | `41 c6 85 e5 2c 00 00 02` | `mov byte [r13+0x2ce5], 2` | path state byte |
| `0xf0dcf6` | `80 79 65 02` | `cmp byte [rcx+0x65], 2` | the drop's "no path" test |
| `0xf171e0` | `8b 83 80 2c 00 00` | `mov eax, [rbx+0x2c80]` | the cache rule's tag read |

## 26. Corrections and open items from this read (2026-11-04)

### Superseded

- **Section 17, fountain row:** "the second site fires when
  `well_runaway` runs out" is superseded 2026-11-04. The second site
  (`0xefd02f`) fires at every think while `well_runaway` is still
  running; when it runs out, `0xefd8ae` clears it (section 22).
- **Section 18, `retreat_tick`:** "Reached from the fountain exit; other
  entries to `0xefce22` not checked" is superseded 2026-11-04: the
  `well_runaway` path also reaches it (`0xefd05f`). The guess "a fighter
  may turn back into the fight within 4 s of retreating" is refined in
  section 22: the branch judges a turn back made within 4 s.
- **Section 14, "Who sets `trace_on` was not read":** superseded
  2026-11-04. The agent step copies the world byte `+0x3c` into brain
  `+0x830` (`trace_on`) and `+0x14c8` (the plan trace switch) at every
  think (`0xf19a00`..`0xf19a0d`).
- **Section 2, call tree:** `0xf16d60` reaches `0xe381b0` through
  `0xe50330` (`handler.rs:537`, call at `0xe50418`). The small action is
  chosen after the planner by `0xf178c0` and executed by `0x104a990` in
  the agent step `0xf19810` (section 20).

### Not read

- How often a laning think replaces the PassiveLine plan.
- The auction's scoring: how often a trade wins over the positioning
  candidate in LineSafe and LineWait.
- The rest of `0x1105f80` after rule 4 (the usual sub plan).
- The meaning of the per-lane record's entity X.
- `0xe59790` (`engage.rs:349`), `0x1094260`, and what the 450-unit test
  in `engage.rs` decides.
- The restore at `0xe74980`: when it copies `base_plan` back and when it
  rebuilds.
- How the Battle sub plan turns the RunAway sub goal into small actions.

Scripts used (scratch, listed in the hand-over note): `ann.py`,
`callers.py`, `fmtfields.py`, `jt.py`, `leas.py`, `dispscan.py`,
`stores.py`, and, new this session, `coverstores.py` (every store whose
destination covers a field, any width), `reach2.py` (which sites are
reachable from an entry, from the earlier `reach.py`), `strs.py`,
`hexat.py`, `loc.py` (panic `Location` decoder) and `insnbytes.py`.

## 27. Reason 0x30 and the objective scene record (read 2026-11-04)

Question from the design session: in 1.7.1-dev17 (seen in game,
2026-11-04) a "take Serpen" or "take Morgard" ping sets the objective on
every teammate, the objective is kept, and nobody who did not already
hold it walks; the reason byte (brain `+0x1b0`) reads 0x30 at decision
after decision. What is the AI waiting for, and can the mod end the
wait? Sections 27 to 31 answer for game 0.7.0; section 32 is the
cross-check with the other mod. Nothing new was played for these
sections: every line is **read from the code** or a **guess**, except
the dev17 log lines quoted in section 28 (**seen in game**, the
author's match).

### The record: `ObjScene`

**Read from the code.** TeamPlan holds two optional scene records,
`serpen_scene` and `morgard_scene` (TeamPlan formatter `0xe801a0`, both
with the value vtable `0x400f280`: size 0x20, Debug `0xe7b890`). The
Debug formatter prints `None` when byte `+0x1b` is 2 and otherwise
`Some(ObjScene { … })` through `0xed9530`, whose field order, read by
hand (the script loses two fields through the stack), is:

| Offset in the record | Field | Size |
| --- | --- | --- |
| +0x0 | `entry_dead` | u64 |
| +0x8 | `entry_enemies` | u64 |
| +0x10 | `assemble_due` | u64 (a tick) |
| +0x18 | `phase` | u8 (the objective's phase enum: 0 None, 1 Setup, 2 Assemble, 3 Hunt) |
| +0x19 | `with_battle` | bool |
| +0x1a | `stance_grind` | bool |
| +0x1b | `take_born` | bool; 2 = the Option is None |

Where the records sit:

| Record | TeamPlan offset | Brain offset | None byte (brain) |
| --- | --- | --- | --- |
| `defender_keep_scene` (0x40 bytes, two records) | +0xa30 | +0xb18 | +0xb33, +0xb53 |
| `serpen_scene` | +0xa70 | +0xb58 | +0xb73 |
| `morgard_scene` | +0xa90 | +0xb78 | +0xb93 |

The brain offsets `+0xb73` and `+0xb93` are the "two scene tag bytes" of
section 14's dev panel.

### Where 0x30 is written, and what keeps it there

**Read from the code.** In the cascade `0xec4090`, after the loop that
sets `window_intent` for k = 0 (Serpen) and k = 1 (Morgard)
(`0xec65a8`..`0xec6a04`, exit `ja 0xec7061`), one block reconciles the
two records with the objective (Locations at
`0xec71fa` = line 252 and `0xec72c4` = line 270). `rsi` is the TeamPlan
(brain `+0xe8`), `r15` the objective byte.

1. `edi` = 1 when the objective is Defense, DefenseLine, Nexus or
   PressTower (`0x11c >> tag`, `0xec70ad`) and TeamPlan `+0xd5a` is 0
   (`0xec7061`; **guess:** `+0xd5a` is `sync_follower`, the one TeamPlan
   field the formatter script could not place). Otherwise 0.
2. **Serpen record** (`0xec7081`: None byte `+0xa8b` ≠ 2):
   - objective none (0xff): the record is left alone (`0xec70ec`);
   - objective Gank (6) or ComebackPick (9): reason 0x30 with the tick,
     objective cleared (`0xec71ec`, `cascade.rs:252`);
   - **any other objective, Serpen and Morgard included:** reason 0x30
     with the tick, **objective kept** (`0xec71a5`), and the record is
     thrown away: moved into `defender_keep_scene` (`0xec71bd`..
     `0xec71d9`) when `edi` is 1, otherwise simply set to None
     (`0xec7507`).
3. **Morgard record** (`0xec7212`: `+0xaab` ≠ 2): the same, with
   `0xec7268` (kept), `0xec72b6` (cleared, `cascade.rs:270`), the copy
   into `+0xa50` and `0xec7513`.

So 0x30 is not a waiting state. It means: this brain held an objective
**and** a Serpen or Morgard scene record at the same time, and the
cascade discarded the record. A 0x30 with a new tick at every decision
means a fresh record is written into this brain at every decision and is
discarded at every decision because the objective is already set.

**Superseded 2026-11-04,** section 14's row `0x30`: "a Serpen or Morgard
scene is pending … the objective is kept and the scene parked or
dropped". The two kept sites drop the record outright unless the
objective is one of the four parking ones; "pending" is the wrong word,
the record is discarded. The design label "objective play pending
(0x30)" (pings spec) follows from the old row.

## 28. A Serpen or Morgard play belongs to one teammate and is mirrored (read 2026-11-04)

### The publisher and the mirror

**Read from the code.** One brain per team owns the play; the others
copy it at every decision.

| Step | Site | What it does |
| --- | --- | --- |
| Who publishes | `0xeceb50(side, ctx)`, called at `0xec45a5` | Returns (found, slot). If shared-block byte `+0x78` is set, the slot is the word at `+0x80`. Otherwise the lowest roster slot whose unit exists in `sim + 0x238 + side*0x28 + slot*8` and whose bit in the shared byte `+0xe9d` is clear (`0xeceb98`..`0xecec5c`). |
| `sync_follower` | `0xec45cb`: TeamPlan `+0xd5a` (brain `+0xe42`) | 1 unless this brain is the publisher. The TeamPlan formatter `0xe801a0` names it (`lea r9, [rcx + 0xd5a]` at `0xe801c4`), which confirms the guess in section 27. |
| The mirror | `0xec45f8`..`0xec496d`, at the start of every cascade | When the shared-block word `+0xd8` is not −1, copies the team's `ObjSceneSync` (shared `+0xd8`, 0x160 bytes) into this brain: `serpen_lock` (TeamPlan `+0x7f8`), `morgard_lock` (`+0x838`), `serpen_scene` (`+0xa70`, from shared `+0x158`), `morgard_scene` (`+0xa90`, from `+0x178`), `obj_deadline`, `obj_arrive_at`, `obj_quorum_k`, `obj_enemy_gather_since`, `window_intent`, `obj_pred_*`, `obj_commit_at`, `assemble_arrived`, `obj_released`, `first_serpen_state`. **There is no follower test:** the publisher copies too (its own last publish). |
| The publish | `0xec835a`..`0xec8913`, at the end of every cascade | A follower writes `sync_out` (TeamPlan `+0x878`, brain `+0x960`, 0x160 bytes) = None (−1). The publisher packs its own locks, scenes and `obj_*` fields into it. |
| Shared → everyone | not found | The writer of shared `+0xd8` is outside `game-ai`. Section 5 already found no planner store to the shared block. `0xe6fb40` (`scene_feedback.rs`) only reads it (`0xe70a49`). `0x18e1ac0` and `0x18b8a30` copy the whole 0xea0-byte block (clone or (de)serialise). |

`ObjSceneSync` (Debug formatter `0x10063f0`; offsets inside the 0x160
bytes; None when the first dword is −1):

| Offset | Field | Offset | Field |
| --- | --- | --- | --- |
| +0x0 | `lock` (the two 0x40-byte locks) | +0x110 | `pred_n` |
| +0x80 | `scene` (the two ObjScene records) | +0x120 | `pred_idle_since` |
| +0xc0 | `arrive_at` | +0x130 | `commit_at` |
| +0xd0 | `deadline` | +0x140 | `last_commit` |
| +0xe0 | `quorum_k` | +0x150, +0x152 | `arrived`, `departed` |
| +0xf0 | `enemy_gather_since` | +0x154, +0x156 | `intent`, `released` |
| +0x100 | `pred_dpt` | +0x158 | `first_serpen` |

### How a teammate is on a play

**Read from the code.** The cascade (`0xec4090`) runs, in order:

1. The mirror (above).
2. The `window_intent` loop and the 0x30 block (section 27). A brain
   that holds **any** objective other than Gank or ComebackPick throws
   the mirrored record away. With objective none it keeps it.
3. Body 1 `0xeb9e60` for k = 0 (Serpen), then k = 1 (Morgard), called at
   `0xec8214`. When this brain's record k is Some (`0xeb9ea6`), body 1:
   - **takes** it (sets it None, `0xeb9f20`);
   - sets the objective from it: `(k == 0 ? 1 : 0) | phase << 8 |
     with_battle << 16` (`0xeb9f3d`, `cascade.rs:346`);
   - runs the objective handler (`0xeb7060` Serpen, `0xeb7770`
     Morgard) and plays the phase;
   - at the end stores the record back (`0xebe53f`..`0xebe56f`) and
     clears the objective again (the silent clears of section 14).
4. Only when body 1 handled nothing: body 2 `0xebf4d0` (`0xec82f0`),
   the switch on the objective. Objective Serpen (case 1) only calls
   the Serpen handler (`0xebf714`). Objective none ends in the AI's own
   pick chain; one of its last steps can start a Serpen play through
   `0xeabda0` (`epic.rs:222`: record {0, 0, −1, Setup, with_battle 1,
   0, 0}, `0xeabe5f`..`0xeabe71`, after the mode gate, `obj_pred_dpt`,
   `0xed54f0` (`serpen.rs:50`) and the lock gate `0xec3510`).

So a teammate on the team's play shows **objective none and a scene
record Some** between decisions; the objective exists only inside the
decision. A planted objective is the other way round: objective set,
no record, and it takes the body 2 path.

**The lock gate `0xec3510(team_plan, ?, k, player, ctx)`**: true when
the lock k (TeamPlan `+0x7f8 + k*0x40`) is not set. When it is set, true
only if the alive counts and the stored thresholds have moved enough
(`0xec3595`..`0xec36be`). **Guess:** a failed play locks the monster
until the fight situation changes.

### Joining from a TeamScene (the relay)

**Read from the code,** handler relay `0xe3cb39`..`0xe3d414`
(`relays.rs:5xx`), before the cascade, for k = 0 and 1. It scans the
shared TeamScene Vec (`+0x2b8`, `+0x2c0`) for scenes of kind 5 (Serpen)
or 6 (Morgard) owned by another player. It keeps the highest `phase`
(+0x48) and `intent` (+0x47). When they rose since its last look:

- **Already on it** (record Some, or objective tag = the monster,
  `0xe3cbd0`/`0xe3cbe0`): it only sets `with_battle`.
- **Otherwise it joins**, unless the objective is Nexus, or Defense
  under threat. The lock gate `0xec3510` must pass, and the shared sync
  must hold a record for that monster (`0xe3cfd5`, `0xe3d004`). Joining
  writes:
  - objective none (`0xe3d040`, `relays.rs:535`; Morgard `0xe3d0ce`,
    `:544`);
  - its own record = {0, 0, `assemble_due` −1, the scene's phase,
    `with_battle` = intent ≠ 0, 0, 0} (`0xe3d058`);
  - `obj_arrive_at`, `obj_deadline` (= arrive + 4 s) and `obj_quorum_k`
    for k;
  - then a rebuild through the builder (`0xe3d3aa`) and the plan setter
    (`0xe3d407`, `relays.rs:577`). This is skipped when the plan is
    Battle, or when the plan's goal (`0xf157b0`) is already Serpen (3)
    or Epic (2).

The kind-5 and kind-6 TeamScenes are written by the same unknown writer
as the rest of the shared block. **Guess:** the writer turns the
publisher's record into the TeamScene.

### What this means for the dev17 observation

**Seen in game** (the author's match, 1.7.1-dev17 research build, game
0.7.0, 2026-11-04; read from `probe.log`, ping n=14, team 0, the player
at slot 2, ticks 15774 to 16315). The watch lines log the reason byte
but not its tick.

| When | Slot 0 (top) | Slots 1, 3, 4 | Slot 2 (the player's brain) |
| --- | --- | --- | --- |
| Before the ping (t 15774..15873) | objective none, PassiveLine, reason 0x30 | objective none, **SerpenHuntAndPoke**, reason 0x31 | objective none, SerpenHuntAndPoke |
| The ping plants Serpen Setup on 0, 1, 3, 4 (t 15882..15889) | | | |
| After (t 15891..16250) | Setup, then Setup with battle; plan 12 and 3 in turn; small moves near its lane; cleared at t 16315 with 0x24 (Serpen down) | The plan falls back to the role's plan at the first watch line (3 and 4 PassiveLine, 1 PassiveJungle); 3 and 4 are on SerpenHuntAndPoke again about 60 ticks later, 1 is still PassiveJungle at t 15980. Objective: Setup, then Setup with battle, then Hunt (1, 4 by t 15980; 3 by t 16138). Reason **0x30** from the first watch line on. | unchanged |

- **Read from the code, matching the log.** Slots 1 to 4 were already
  on the team's own Serpen play before the ping: objective none and plan
  SerpenHuntAndPoke is the state section 28 describes for play members.
- **Seen in game, same lines.** The plant knocked them off the play for
  a while: their SerpenHuntAndPoke gave way to the role's plan at once.
  **Guess:** the hunt plan's own update or done test ends it when the
  brain's record is gone (not read), and the rebuild from the planted
  Setup then follows the builder's Setup rule (section 29).
- **Read from the code.** After the plant, their byte changed from 0x31
  to 0x30. So the 0x30 block ran at least once on each of them with a
  Serpen record mirrored and their objective set: they dropped the
  team's record. Whether it ran again at every decision cannot be told
  from the byte. Slot 0 shows 0x30 **before** any plant, with objective
  none, which the 0x30 block never writes, so the byte there is left
  over from an earlier decision.
- **Read from the code.** The step from Setup to Setup with battle on a
  planted brain is the relay's "already on it" branch (`0xe3ce5c`..
  `0xe3ce76`: objective tag = Serpen, `with_battle` = 1). The relay
  treats a planted objective as a member already on the play and does
  not adopt the record.
- **Guess, two readings of who publishes.** Section 12 puts the player
  at slot 2. By `0xeceb50` the publisher is then slot 0 whenever it is
  alive and not masked.
  1. **Slot 0 published and was planted.** It dropped its own record at
     its first decision after the plant (Serpen is not a parking
     objective), and with it the team's play. Its body 2 then ran the
     Serpen case, never the pick chain that calls `0xeabda0`. So the
     team could not start a new play while slot 0 held the planted
     objective. This fits "nobody walks" without any slot being dead.
  2. **Another brain published** (shared `+0x78`/`+0x80` override, or a
     masked slot 0), possibly the player's own brain at slot 2, which
     was on the play. The play then continued, and the planted brains
     kept refusing it.
  The probe at the end of section 30 tells the two apart.

**Superseded 2026-11-04,** in this section as first written: "0x30 with
a new tick at every decision … the team already had a play running",
and "the publisher was a brain the ping did not plant". The tick was
not logged, and the log shows a stale 0x30; both are replaced by the
table and the two readings above.

## 29. What a planted Setup waits for (read 2026-11-04)

### The plan is built only on a rebuild

**Read from the code.** A planted objective does not rebuild the plan.
The plan changes only when one of these happens:
- something writes ForcePassive (the handler rebuilds it from the
  objective in the same decision, `0xe407c4`, after the cascade);
- a relay rebuilds it;
- the plan ends.

For a Serpen objective the handler's own Serpen rule writes ForcePassive
at one point only:

| Site | Rule |
| --- | --- |
| `0xeb760a`..`0xeb7674` | Phase is Setup (`cmp dil, 1`) and `first_serpen_state` (TeamPlan `+0xd60`) is not 1: ask `0xeb9540` (`objective_discipline.rs:36`), then `0xed2c50` (`serpen.rs:74..88`). |
| `0xeb7685`, `0xeb768a` | Either agrees: objective Serpen Hunt with battle (0x10301, `objective_handlers.rs:608`). |
| `0xeb7694`..`0xeb76b1` | The plan is not SerpenHuntAndPoke (raw 12): `0xec3ba0(team_plan, plan, ForcePassive, :613)`. This is the game's own "replace the plan" helper: trace when `trace_on` is set, drop the old plan (`0xea3030`), copy 0x400 bytes. The rebuild then builds the hunt. |

So the 10 decisions and 12 s seen in dev11 (section 12) are the wait
for `0xeb9540` or `0xed2c50` to agree. **Not read:** their conditions.
`0xed2c50` (6.3 KB) loops five times over units through three virtual
calls (`+0xf8`, `+0x168`, `+0x28`). **Guess:** counts and strength of
the two teams near the pit.

### The builder's Setup rule

**Read from the code.** The builder `0xe52ab0` takes the play from the
scene records first, then from the objective (`0xe544a6`):
`serpen_scene` phase if Some, else `morgard_scene`, else the objective
bytes. For Serpen it calls `0xed4520` (`serpen.rs:186..263`) with the
phase:

| Phase | Rule in `0xed4520` | Result |
| --- | --- | --- |
| any | Mode byte in mask `0x1a1`; the own unit exists; for slot 0 before the first Serpen time minus 30 s, also `early_serpen_top` (strategy `+0x14`, `0xed5400`). | otherwise −1 |
| Setup (1) | `wait` = Serpen's spawn tick (kind-0 object `+0x1e0`) minus now, 0 if past. `travel` = distance from the unit to the side's rally point (world `+0x7108 + side*0x10`) divided by the unit's speed (`+0x638`). **Walk only when `wait ≤ travel + 2 × delay`** (`0xed4a55`, `0xed4a59`, 2 s); then `0xee9a30` (`hunt_and_poke.rs:221`) must be false. | SerpenHuntAndPoke (raw 12, `0xed4ac3`), else −1 |
| Assemble, Hunt (2, 3) | The slot's `obj_released` bit (`+0xd50`), the record's `stance_grind`, the shared mask byte `+0xe9e`, then the `obj_contest.rs` tests `0xeac5a0`, `0xeade40`, `0xeaf440`. | SerpenHuntAndPoke or −1; not read in full |

When it returns −1 the builder falls back to the role's usual plan
(PassiveLine and the like). Morgard has the same shape through
`0xed1ae0`.

### What keeps Hunt

**Read from the code,** Serpen handler `0xeb7318`..`0xeb75ed`, with
Serpen alive. The rule runs when all of these hold:
- the phase is Hunt (`0xeb73d4`);
- now is past `obj_deadline` (TeamPlan `+0xcc8`, `0xeb73c7`);
- `0xeb8040` gives a smaller first value than its second.

It then looks for a per-player record with byte 3 in the shared block
(`+0x808 + i*0x20`):
- with one, after 2 s, it demotes the objective to Setup with battle
  (0x10101, `0xeb75ed`, `objective_handlers.rs:585`);
- without one, it goes on to `objective_handlers.rs:569`.

`obj_deadline` is one of the mirrored fields, so a follower's deadline
is the publisher's.

## 30. Routes for "take Serpen / take Morgard" (2026-11-04)

What the game does is **read from the code**; whether a teammate then
walks is a **guess** until played. "Hook" is the existing step hook at
`0xf16de7`: it runs before the planner, has brain, context, player and
sim in hand, and runs inside `catch_unwind`.

| Route | Writes, at the hook | Gates that refuse | A refusal looks like |
| --- | --- | --- | --- |
| A. Plant Setup (dev17) | setter `0xec3820`, 0x10101 | Mode gate (0x20). Serpen dead and over 15 s away (0x21, or 0x24 in the cascade). Nexus or Defense first (replaced). **A running team play is dropped** (0x30; on the publisher it ends the play for everyone, section 28). The walk waits for the handler's own Hunt promotion. | objective kept, plan unchanged, 0x30 |
| B. Plant Hunt (0x10301) | setter | As A. Hunt is demoted to Setup after `obj_deadline` (mirrored: the publisher's) under the `0xeb8040` test. **No rebuild:** only the handler's own promotion writes ForcePassive. | objective replaced by 0x10101, no reason; plan unchanged |
| C. A plus a rebuild | setter 0x10101, then `0xec3ba0(brain + 0xe8, brain + 0x16a0, &buf, &loc)`, where `buf` is a 0x400-byte buffer whose first u64 is 2 (ForcePassive). Only when the plan is not Battle (raw 0, 1). | As A. Then the builder's Setup rule: it walks only when Serpen is up or spawns within travel + 2 s, and `0xee9a30` is false. | plan rebuilt as the role's usual plan (tag 3 or 7 again; lane flags reset); objective kept |
| D. Clear the objective when the team already plays, then rebuild | setter 0xff on a teammate whose objective is set, when the shared sync holds a record for that monster; then one `0xec3ba0` rebuild (as in C) when the plan is not Battle | None of the 0x30 block applies; body 1 then takes the mirrored record and stores it back, and the rebuild after the cascade sees it (the builder reads the record's phase first, `0xe544a6`). Without the rebuild the plan may not follow: the relay stores the highest phase and intent before its "already on it" test, so after a clear it sees no rise, and the handler writes ForcePassive only on its own Setup → Hunt step. | record Some, objective none between decisions; plan = the role's plan when the builder refuses |
| E. Strategy | SDK `strategy_set_json(team, "early_serpen", "\"Must\"")`, `object_buildup` Gather, from the match hook | The AI's own rules: `0xeb1730`, `0xeb2510`, `0xeb4a00` and `0xec9270` read `early_serpen`; `0xec4090`, `0xed4520`, `0xee8960` and `0xee9c40` read `object_buildup`. The timing is still bound to the spawn. | no change visible in a brain |
| F. Write the team's record | shared block `+0xd8 + 0x80` (Serpen) or `+0xa0` (Morgard): 32 bytes, laid out as `0xeabda0` writes them ({0, 0, −1, phase 1, with_battle 1, 0, 0}) | Shared state with an unknown writer. The publisher's play may refuse it through the lock gate `0xec3510`. The next publish overwrites it. | record gone at the next decision |
| G. Call the game's start routine `0xeabda0` (`epic.rs:222`) | `(team_plan, ?, player, ctx)` on the publisher | It writes the brain's own record, which the mirror overwrites at the start of the same cascade (the hook runs before it). | nothing |

**Notes on the writes.**

- `0xec3ba0` allocates only when `trace_on` (TeamPlan `+0x748`) is set
  (`0xec3bc6`). Otherwise it drops the old plan (`0xea3030`) and copies
  the 0x400-byte buffer (`0xec3ee1`). It is the helper the Serpen
  handler itself uses. The plan setter `0xe37db0` (section 22) is the
  alternative, with the same caveats.
- The ForcePassive rebuild runs after the cascade (`0xe404c5` before
  `0xe407c4`), so it sees the objective the cascade kept. The mirror
  and the 0x30 block run before both.
- Route F breaks the rule of section 9 ("nothing shared"). Route G does
  nothing. F would also need the shared writer read first.

### Recommendation

**One rule first, then route D or C per teammate, re-asserted at most
once a second for the window. Guess until played.**

0. **Never plant Serpen or Morgard (or any objective other than the
   four parking ones) on the publisher.** A planted publisher drops the
   team's record, which ends the team's own play for everyone, and its
   body 2 never reaches the pick chain that starts a new one. The hook
   can tell who publishes: brain `+0xe42` (`sync_follower`, as the
   previous decision left it) is 0 only on the publisher; or apply the
   `0xeceb50` rule (lowest present slot not masked by shared `+0xe9d`,
   unless shared `+0x78` is set). With the player at slot 2 this is
   usually slot 0.
1. **The team is already on it (route D).** The shared sync is Some
   (`+0xd8` ≠ −1) and its record for the monster the ping names is
   present: the None byte, shared `+0x173` for Serpen or `+0x193` for
   Morgard, is not 2.
   - For each teammate whose objective is set (and is not a parking one
     the player wants kept), clear it (setter 0xff), then request one
     rebuild through `0xec3ba0` when the plan is not Battle.
   - Plant nothing. Teammates already on the play (objective none, plan
     SerpenHuntAndPoke, as slots 1, 3 and 4 were in the dev17 log) are
     left alone: planting knocked them off it.
2. **The team is not on it (route C).** Plant Setup with battle on the
   followers (not the publisher).
   - When the game's own walk rule would pass, request one rebuild
     through `0xec3ba0`. The rule: Serpen alive (kind-0 object
     `+0x1d8` ≠ 0), or its spawn tick (`+0x1e0`) within the teammate's
     travel time + 2 s. The mod can compute both.
   - Never while the plan is Battle.
   - The game's builder then decides. A refusal leaves the role's own
     plan.

**Risks.**

- **Rule 0** depends on `+0xe42` being current; it is written at every
  cascade, so at the hook it is one decision old. A change of publisher
  (slot 0 dies) between two decisions is possible.
- **Route D** relies on the mirror and on the unknown shared writer. A
  play the publisher drops ends for everyone at once (honest: it is the
  team's).
- **Both routes write the plan** through `0xec3ba0`.
  - It resets the PassiveLine flags, so a driven window must re-assert
    them.
  - It must never run on a Battle plan.
  - SerpenHuntAndPoke built from a Setup objective was seen in the dev17
    log (slot 0, plan 12 lines) but flipped back to PassiveLine and
    again within seconds. Its own update (`0xee9c40`) and the done test for
    raw 12 were not read, so a hunt built this way may end at once.
- **Route C does not make the team gather.** Quorum, `assemble_due` and
  the Assemble phase belong to the publisher's record. Followers who
  walk under route C each go alone.
- **The player's own brain may publish** (section 28, second reading).
  The team's plays are then decided by an AI whose body the player
  drives. A ping cannot fix that from a teammate's brain.

**First probe** (one research build). Log at every teammate's hook, and
for the player's own brain:
- `sync_follower` (brain `+0xe42`);
- the shared sync tag (`+0xd8`) and the two shared record None bytes
  (`+0x173`, `+0x193`);
- the brain's own None bytes (`+0xb73`, `+0xb93`);
- the objective, the reason byte **and its tick** (brain `+0x1b8`), and
  the plan tag.

It settles who publishes, whether the planted publisher ended the play,
and whether 0x30 is written at every decision.

## 31. Verifier anchors (expected bytes, 0.7.0)

**Read from the code** (`insnbytes.py`), for
`tools/native_profiles/0.7.0.json` if a route is built:

| Site | Bytes | Instruction | Guards |
| --- | --- | --- | --- |
| `0xec45a5` | `e8 a6 a5 00 00` | `call 0xeceb50` | the publisher pick |
| `0xec45cb` | `88 86 5a 0d 00 00` | `mov byte [rsi+0xd5a], al` | `sync_follower` at TeamPlan +0xd5a |
| `0xec45f8` | `4c 8b 94 0b d8 00 00 00` | `mov r10, [rbx+rcx+0xd8]` | shared sync at block +0xd8 |
| `0xec4612` | `0f 84 5b 03 00 00` | `je 0xec4973` | no mirror when the sync is None |
| `0xec47b9` | `44 0f 11 86 70 0a 00 00` | `movups [rsi+0xa70], xmm8` | mirror into `serpen_scene` |
| `0xec47dc` | `88 8e 8b 0a 00 00` | `mov byte [rsi+0xa8b], cl` | Serpen record None byte |
| `0xec4841` | `44 88 b6 ab 0a 00 00` | `mov byte [rsi+0xaab], r14b` | Morgard record None byte |
| `0xec7061` | `80 be 5a 0d 00 00 00` | `cmp byte [rsi+0xd5a], 0` | parking only for the publisher |
| `0xec7081` | `80 be 8b 0a 00 00 02` | `cmp byte [rsi+0xa8b], 2` | Serpen record present |
| `0xec70ad` | `bf 1c 01 00 00` | `mov edi, 0x11c` | parking objectives mask |
| `0xec71a5` | `c6 86 c8 00 00 00 30` | `mov byte [rsi+0xc8], 0x30` | 0x30, objective kept (Serpen) |
| `0xec71b3` | `40 f6 c7 01` | `test dil, 1` | park or drop |
| `0xec7507` | `c6 86 8b 0a 00 00 02` | `mov byte [rsi+0xa8b], 2` | Serpen record dropped |
| `0xec7212` | `80 be ab 0a 00 00 02` | `cmp byte [rsi+0xaab], 2` | Morgard record present |
| `0xec7268` | `c6 86 c8 00 00 00 30` | `mov byte [rsi+0xc8], 0x30` | 0x30, objective kept (Morgard) |
| `0xec7513` | `c6 86 ab 0a 00 00 02` | `mov byte [rsi+0xaab], 2` | Morgard record dropped |
| `0xec835a` | `80 be 5a 0d 00 00 00` | `cmp byte [rsi+0xd5a], 0` | a follower publishes None |
| `0xec8889` | `4c 89 a6 78 08 00 00` | `mov [rsi+0x878], r12` | `sync_out` at TeamPlan +0x878 |
| `0xec8214` | `e8 47 1c ff ff` | `call 0xeb9e60` | body 1 (scene) |
| `0xec82f0` | `e8 db 71 ff ff` | `call 0xebf4d0` | body 2 (objective switch) |
| `0xeb9ea6` | `44 0f b6 bc 01 8b 0a 00 00` | `movzx r15d, byte [rcx+rax+0xa8b]` | body 1 reads the record |
| `0xeb9f20` | `c6 44 07 1b 02` | `mov byte [rdi+rax+0x1b], 2` | body 1 takes the record |
| `0xebf714` | `e8 47 79 ff ff` | `call 0xeb7060` | body 2, case Serpen |
| `0xeb760a` | `40 80 ff 01` | `cmp dil, 1` | Hunt promotion only from Setup |
| `0xeb7619` | `80 b8 60 0d 00 00 01` | `cmp byte [rax+0xd60], 1` | `first_serpen_state` |
| `0xeb7685` | `ba 01 03 01 00` | `mov edx, 0x10301` | Serpen Hunt with battle |
| `0xeb7694` | `83 3a 0c` | `cmp dword [rdx], 0xc` | plan already SerpenHuntAndPoke |
| `0xeb76b1` | `e8 ea c4 00 00` | `call 0xec3ba0` | the handler's own ForcePassive |
| `0xeb73c7` | `4c 3b 81 c8 0c 00 00` | `cmp r8, [rcx+0xcc8]` | `obj_deadline` |
| `0xeb73d4` | `40 80 ff 03` | `cmp dil, 3` | Hunt demotion |
| `0xeb75ed` | `ba 01 01 01 00` | `mov edx, 0x10101` | demoted to Setup with battle |
| `0xec3bc6` | `80 b9 48 07 00 00 00` | `cmp byte [rcx+0x748], 0` | `0xec3ba0` traces only when `trace_on` |
| `0xec3ed0` | `e8 5b f1 fd ff` | `call 0xea3030` | `0xec3ba0` drops the old plan |
| `0xec3ee1` | `e8 ad 16 f9 02` | `call` memcpy (0x400) | `0xec3ba0` copies the new plan |
| `0xe404c5` | `e8 c6 3b 08 00` | `call 0xec4090` | the cascade runs before the rebuild |
| `0xe544a6` | `45 8b be 70 0b 00 00` | `mov r15d, [r14+0xb70]` | the builder prefers the record's phase |
| `0xed46c5` | `80 fa 02` | `cmp dl, 2` | the builder's phase split |
| `0xed4a55` | `48 8d 04 48` | `lea rax, [rax+rcx*2]` | travel + 2 × delay |
| `0xed4a59` | `49 39 c3` | `cmp r11, rax` | the spawn wait against it |
| `0xed4ac3` | `49 c7 04 24 0c 00 00 00` | `mov qword [r12], 0xc` | SerpenHuntAndPoke built |
| `0xe3cbd0` | `80 b9 73 0b 00 00 02` | `cmp byte [rcx+0xb73], 2` | relay: already on Serpen |
| `0xe3cfd5` | `49 83 bc 24 d8 00 00 00 ff` | `cmp qword [r12+0xd8], -1` | relay needs the shared sync |
| `0xe3d004` | `80 b8 9b 00 00 00 02` | `cmp byte [rax+0x9b], 2` | relay: the sync holds a Serpen record |
| `0xe3d058` | `48 c7 80 68 0b 00 00 ff ff ff ff` | `mov qword [rax+0xb68], -1` | relay writes the record (`assemble_due`) |
| `0xe3d3aa` | `e8 01 57 01 00` | `call 0xe52ab0` | relay rebuild |
| `0xeceb6d` | `42 80 7c 08 78 00` | `cmp byte [rax+r9+0x78], 0` | publisher override byte |
| `0xeceb98` | `48 83 bc d0 38 02 00 00 00` | `cmp qword [rax+rdx*8+0x238], 0` | publisher = lowest present slot |
| `0xecebab` | `0f 84 98 00 00 00` | `je 0xecec49` | mask byte `+0xe9d` |

`0xe407c4` (ForcePassive rebuild) is already in section 25's table.

### Not read in this pass

- The writer of the shared block's `ObjSceneSync` (`+0xd8`) and of the
  kind-5 and kind-6 TeamScenes; whether it runs after every think or
  once a tick.
- The conditions of `0xeb9540` (`objective_discipline.rs:36`) and
  `0xed2c50` (`serpen.rs:74..88`), the two tests that promote Setup to
  Hunt.
- `0xeb8040`, `0xee9a30` (`hunt_and_poke.rs:221`), `0xed54f0`
  (`serpen.rs:50`), and the Assemble/Hunt half of `0xed4520`.
- SerpenHuntAndPoke's own update (`0xee9c40`) and its done test (raw 12
  in `0xf158a0`): how long a hunt built on a Setup objective lasts.
- Who sets the shared publisher override (`+0x78`, `+0x80`) and the
  slot masks `+0xe9d`, `+0xe9e`.
- The body 1 phase logic between `0xeba08b` and `0xebe588` (Assemble,
  quorum, `assemble_due`), and the body 2 case that reaches the play
  start `0xeabda0` (`0xec2c6d`: a kind-0 object word at
  `+0x240 + side*8` against 10 × a computed value).
- Morgard's twins (`0xeb7770`, `0xed1ae0`) site by site; they were
  assumed to mirror the Serpen ones from the shared code paths.

Scripts used (scratch, listed in the hand-over note
`research/notes/team-ai-read-4.md`): `ann2.py` (the earlier `ann.py`
plus Rust `Location` decoding), `vt.py` (vtable entries: drop, size,
align, Debug), `find160.py` (functions holding both the shared-block
stride and a given copy size), `povscan.py` and `povxref.py` (the POV
DLL scans of section 32), with the earlier `insnbytes.py`, `callers.py`,
`coverstores.py`, `stores.py`, `fmtfields.py`, `loc.py`, `strs.py`.

---

## 32. Cross-check: the POV mod's objective call (read 2026-11-04)

Firkin's "Control: POV gameplay mod" 0.15.4 (`tft2_pov.dll`, 8.4 MB,
Rust, no symbols), the Workshop copy installed on the author's machine.
This is a mechanism cross-check only: nothing in it is to be copied into
this mod (its numbers, layout and wording are its author's). **Read from
POV's code** means static reading of that DLL; **seen in its log** means
the author's own `tft2_pov.log` and `tft2_pov.prev.log` (2026-11-04, game
0.7.0), which hold no Serpen or Morgard call.

| Question | Finding | Label |
| --- | --- | --- |
| Does it hook the game's AI natively? | Its log lists two native hooks only: the chat pipeline (`exe+0x1880930`) and `Sender::send` (`exe+0xc68680`), plus a window procedure hook. No AI function is named. | seen in its log |
| Does it touch brain fields? | A scan of every instruction in its 5,897 functions for memory operands with the brain offsets of this document (objective `+0xe45`..`+0xe47`, TeamPlan objective `+0xd5d`, the scene records `+0xb58`..`+0xb93`, reason `+0x1b0`/`+0x1b8`, plan `+0x16a0`, `base_plan` `+0x1aa0`, `small_action` `+0x2c80`, `next_input_tick` `+0x47c0`, `sync_follower` `+0xe42`, `sync_out` `+0x960`, the PassiveLine and BattlePlan flags, the inbox `+0x1ee8`) found none, apart from `+0xe8`, which is too common to mean anything. | read from POV's code |
| Does it call a game function? | No immediate holds the RVA of the setter `0xec3820`, the opener `0xe662f0`, the Serpen handler `0xeb7060`, the play start `0xeabda0`, the replace helper `0xec3ba0` or the plan setter `0xe37db0`. Of the objective constants, 0x10301 and 0x10300 never appear; 0x10101, 0x10100 and 0x10200 appear as byte-broadcast multipliers (`imul …, 0x10101`) and as a register argument in four functions that were not opened (`0xb1e40`, `0x3842f0`, `0x398490`, `0x399310`). Their prologues are too generic to match as signatures, so a lookup by pattern cannot be ruled out from bytes alone; the log names none. | read from POV's code; the pattern lookup is a **guess** (ruled out only as far as the log goes) |
| Does it write the strategy for Serpen or Morgard? | The DLL holds the field names `tower_press` ("Poking", "Dive"), `game_finish` ("Aggressive"), `object_battle` ("Initiating") and `minion_wave` ("JoinPriority"), next to the End call's messages ("CALL End: tower_press -> …", "CALL End: team … -> …: … (was …)"). It holds no `early_serpen`, `early_serpen_top`, `object_buildup`, `object_finish`, `morgard_use` or `morgard_defense`. The strategy is written through `strategy_set_json(team, path, json)`, whose path is a field name, so the objective calls write none of these fields. | read from POV's code |
| How does it drive the teammates? | Every call ends with the same message, printed by one function (`0xab40`): "CALL … over … (reason): answered N thinks, left M to the AI". It has its own `src\player_ai.rs`. The official interface offers a per-player AI (`PlayerAiVtableV1.think`, `sdk/mod-api-stable/src/ai.rs`) that is shown the game's own input for the tick and may return its own instead. So "answered" counts the ticks it replaced, "left to the AI" the ticks it let through. | read from POV's code and the SDK; that the objective call uses it is a **guess** from the shared end message |
| How does an objective call end? | The end reasons in the DLL: "its unit is down", "it was on cooldown", "it was not on the map", "nobody left to hear it", "time", "given up: the team is too hurt", "the monster is down", "together". Its tooltips read "Start Serpen: Team starts Serpen" and the same for Morgard. "The monster is down" belongs to the objective calls; "together" is also an Omw end (seen in its log). | read from POV's code; which reason belongs to which call is a **guess** |

**Verdict (guess, consistent with every finding above).** POV's
"Start Serpen" and "Start Morgard" do not use the game's objective,
scene or strategy machinery. They drive the named teammates directly
through the official per-player AI `think`, replacing their input on
some ticks, and end when the monster dies, when they arrive together,
when time runs out or when the team is too hurt. That is the same
family as its Retreat (section 23's override route, done through the
official interface instead of a native site).

**What it means here.** POV gets "everyone walks" because it never asks
the game's AI. It also never makes the AI's own play (the publisher's
record, quorum, Assemble, Hunt) start; teammates stop casting where it
answers the think (section 23). This mod's route in section 30 asks the
game's AI instead.

**Not checked:** the body of POV's `think` (the Serpen case, the target
point, whether it lets casts through), and whether it also plants a
game objective through a pattern lookup the log does not mention. The
next step would be to follow the `player_ai.rs` wrappers (`0x227680`,
`0x2277f0`) to the registered `think`.
