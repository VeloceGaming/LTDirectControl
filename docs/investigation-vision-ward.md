# Investigation: a vision ward (2026-10-22)

The question (the author): can the mod add a "vision ward", an object the
controlled athlete places on the map that grants sight of an area to their
team, as in MOBA games; how, at what cost, and does it fit the mod's
direction?

This is a read-only study. Everything here is **read from the code** of
games 0.6.5 (`research/game-builds/0.6.5`) and 0.7.0 Beta 2
(`research/game-builds/0.7.0b2`) unless a line says **Guess** or
**Rejected**. Nothing is seen in game yet. Addresses are RVAs of the
version named.

## Verdict

Feasible with caveats, through the official mod interface alone. The game
rebuilds each team's sight every tick from every living unit of that team,
whatever the unit's kind, and the interface can spawn a unit
(`StableSim::spawn_unit`) and hide it (`StableSim::entity_set_invisible`).
A ward is a tiny stationary unit of the player's team, invisible for its
whole life. No new native hook, no address to find again on a game patch.
The caveats: five behaviour questions that only an in-game test answers
(section 6), and one direction question for the author (the AI opponent
never places wards; section 5).

## 1. How the game decides visibility

**Read from the code (0.6.5 `view.rs` guards `0x2565d84`, `0x2565807`,
`0x28035d2`; Beta 2 `0x2e8ae51`, `0x2e8a89f`, `0x28511b0`).** What the mod
switches today (`Vision::Own / Other / All` in `probe/src/camera.rs`,
applied in `probe/src/native_adapter/windows/view.rs`) is one byte of the
camera config (`+0x4b`: All 0, Blue 1, Red 2). The renderer passes it to
the fog pass to choose which team's sight to draw. It changes nothing the
simulation knows.

**Read from the code (0.6.5 `0x15b6f30`, called once per tick by the three
game modes' tick functions `0x15fc1d0`, `0x1613f50`, `0x162aac0`; Beta 2
`0x169f320`, same structure).** Sight is a per-team grid plus a per-unit
"seen" state, both rebuilt every tick by one function in the world:

- The map is 30 by 30 cells of 32 000 world units (cell = position /
  32 000, clamped to 29; the minimap profile's `fog_cell_pitch` is the same
  grid). Per team there are two `u32` grids of 0xe10 bytes: a count of the
  team's own units per cell at world `+0xd068 + team * 0xe10`, and a
  "visible" count per cell at world `+0xb448 + team * 0xe10` (team 1 at
  `+0xc258`).
- Line of sight is baked into the map file (`MapSetting.visible_view`,
  built by the path finder's `calculate_visible_view` in the older symbol
  dictionary `research/libgame_core-symbols.tsv`). In memory it is one
  record per cell at world `+0x5d20` (rows of 0x2d0 bytes, 0x18 per cell:
  a pointer and a count of (x, y) cell pairs). When a cell's own-unit
  count goes from 0 to 1, every cell in its list gains one visible count;
  1 to 0 loses one. The sight radius is one game setting,
  `visible_distance`, 130 000 units (130 screen pixels) in the shipped data
  of both versions (a 1 000 000 entry also exists, presumably another
  mode).
- **Which units grant sight:** every id in the world's entity id list
  (`+0x760` pointer, `+0x768` length) whose entity has byte 0 equal to 0
  (not dead or removed) and a team (`+0x8`) of 0 or 1. The loop checks
  neither the unit's kind nor its invisible counter. An invisible unit
  still grants sight to its own team.
- **Seen by team:** each entity (0x6b8 bytes; array at world `+0x730`,
  count `+0x738`; id-to-index map `+0x748`, length `+0x750`) carries two
  0x18-byte records at `+0x38 + team * 0x18`: `{state, last_x, last_y}`,
  state 0 visible, 1 just lost (last known position kept), 2 hidden. The
  builder sets, per entity: own team 0; kind value 4 at `+0x68` 0 for both
  teams; a "revealed by an attacker" record (tag at `+0x10` equal to 1,
  tick `+0x18`, attacker id `+0x20`, window from a setting) 0 for the
  attacker's team; otherwise 0 only when the invisible counter (`+0x690`)
  is 0 and that team's visible count at the entity's cell is above 0, else
  1 when it was 0 last tick, else 2.
- Projectiles get a visible byte from the same grids at the end of the
  builder.

**Read from the code (0.6.5 `0x15f2290`, Beta 2 `0x1c71120`; game method
table slot `+0xf8`, shared by the four game-mode tables).**
`AbstractGame::is_visible(team, id)` returns `state == 0`. It is what the
mod interface's `StableSim::is_visible` calls (host table slot
`SimVtableV1.is_visible`, offset 344: 0.6.5 `0x2d64b70`, Beta 2
`0x33cf0d0`), the call the mod already uses to list targets
(`probe/src/simulation.rs`, `visible_units`).

**Read from the code (field names in both executables; symbol names from
the older dictionary).** The AI consumes this state: the AI structs hold
`last_visible_ticks`, `last_visible_distance`, `last_visible_pos`,
`mia_call_ticks`, `VisionTrackingState { None, LinePushedIn, ... }`,
`epic_camp_last_visible_tick`, `serpen_camp_last_visible_tick` and a
`vision` entry in the AI "signals" struct; the dictionary names
`Blackboard::is_recent_visible*` and `ExpectedGame::is_visible`. A ward
changes what the AI of both sides knows.

## 2. Anything ward-like in the game?

- **Read from the code (both versions).** String scans of the executable
  and of `bundle.game_data` for ward, sight, reveal, fog and vision find
  only awards, forward, insight, localised text and the fields above. The
  game has no ward, scout unit, reveal effect or sight item. The game
  setting `scout_dispatch_cost` is the management scout, not sight.
- **Read from the code (both versions).** Invisibility exists as a native
  effect used by champion data (`InvisibleEffect`, `CasterInvisible`,
  `invisible_duration` 120 ticks in the shipped data) and as the interface
  primitive `entity_set_invisible` (host slot offset 632: 0.6.5 `0x2d696d0`,
  Beta 2 `0x33d3c30`), which writes `entity + 0x690 = max(current, ticks)`.
- **Read from the code (both versions).** Spawning exists as the interface
  primitive `spawn_unit` (host slot offset 520: 0.6.5 `0x2d64d40`, Beta 2
  `0x33cf2a0`). The host table itself (`SimVtableV1`, size 720) sits at
  0.6.5 `0x3d2ddf8`, Beta 2 `0x4263380`. `spawn_unit` builds a normal
  entity record (constructor 0.6.5 `0x15df6f0`, action name "attack",
  `UnitAttackV1` and `StatV1` copied in) and adds it through game method
  table slot `+0x1a8` (0.6.5 `0x15f1fc0` then the world's add routine
  `0x15b55c0`, which pushes the id into the list at `+0x760` at
  `0x15b5b29..0x15b5b5a` and registers the starting cell in the grids;
  Beta 2 `0x1c70e50` then `0x169e110` then the helper `0x1d49480`, which
  pushes into the same list at its `rdi + 0x38 / + 0x40` with `rdi` the
  world's `+0x728`). A spawned unit therefore grants sight from its first
  tick.
- **Read from the code (SDK `sdk/mod-api-stable/src/ai_ctx.rs`,
  `traits.rs`, `probe/src/lib.rs:337`).** `StableAiContext::sim()` hands
  the player-input hook the same `StableSim`, and the mod already
  registers a `StableMatchHook` (`test_cheats::CooldownHook`) that acts on
  the viewed match only (`sim_origin().kind == 2`). Either place can issue
  the spawn.

## 3. Three approaches

### (a) Official mod interface only. Recommended.

- **Buys:** a ward in roughly 150 to 250 lines of mod code, no game address,
  no fingerprint dependency, on every version the SDK supports. The fog and
  the minimap show the ward's sight by themselves, because the game's own
  grid changes.
- **How:** a key press (or a click within reach of the champion) is stored
  on the client thread, the way the wheel notches are (`wheel::take`). The
  match hook, or the player-input hook, spawns the unit at that spot for
  the player's team with zero attack, zero range, zero move speed, a few
  hit points and a duration, then sets it invisible for the same duration.
  The HUD shows a ward glyph on the map and a charge glyph in the strip; no
  text, words only in the tooltip.
- **Patch cost:** none beyond the SDK contract the game keeps stable.
- **Threads:** the match hook runs on the worker of every match, background
  simulations included. The existing cheat hook shows the pattern: an
  atomic check first, then the viewed-match filter, no waiting, short lock,
  no pointer kept ("Threads: the rules" in `docs/modding.md`).
- **AI reaction:** the player's AI teammates and the enemy AI both react to
  what their team sees. The enemy cannot target what it cannot see (SDK
  documentation of `entity_set_invisible`).

### (b) A data-level change reusing an existing sight-granting object.

**Rejected.** No such object exists (section 2). The nearest data route, a
custom skill effect in a champion file that spawns a summon, is approach (a)
with more moving parts and the same open questions.

### (c) A new native hook on the sight builder or the grids.

- **Buys:** a ward with its own sight radius, no unit at all, no hit points.
- **Costs:** writing into the per-team grids every tick on the worker
  thread; finding again, on every patch, the builder, the world offsets
  (`+0x5d20`, `+0xb448`, `+0xd068`, `+0x730..+0x768`) and the grid layout;
  a crash risk in code that runs hundreds of thousands of times a second
  across background matches. **Guess:** two to three times the effort of
  (a), plus a migration entry per version. Only worth it if the in-game
  test shows (a) fails.

## 4. Direction check (`AGENTS.md`)

- **Play as one athlete:** yes; a ward is a thing one athlete does, as a
  support would. It fits best as a limited resource, bought or earned, not
  a free map hack.
- **Honest mechanics:** it changes what the player sees and what the AI
  knows, never the player's aim.
- **Input delay:** none; the spawn happens on the next tick like any order.
- **HUD:** glyphs only (a ward on the map and the minimap, a charge in the
  strip); words only in the tooltip.

## 5. The question for the author

The AI never places wards, so a player-only ward is an advantage the
opponent lacks. The backlog's rule that AI-versus-AI stays vanilla suggests
one of two choices before any build: give the enemy team an equivalent in
the player's match only (for example automatic wards at their jungle
entrances), or file the ward in the cheat/fun section. Not decided.

## 6. Open questions (only an in-game test answers them)

1. Does a unit with zero move speed and zero range stand still and never
   act? **Guess:** yes.
2. What does the viewer draw for a unit with an unknown name, and do hover
   and the health bar work on it? **Guess:** nothing or a placeholder, no
   crash.
3. Can enemy area skills still hit the invisible ward, and does the enemy
   AI path around it? **Guess:** area hits yes, pathing no.
4. Do replays of a warded match diverge, since the spawn comes from live
   input and a replay is a re-simulation (`SimOriginKindV1::ClientReplay`)?
   **Guess:** yes.
5. Does the "revealed by an attacker" rule ever expose the ward? **Guess:**
   no, the ward never attacks.

## 7. Suggested stages

Each stage is one build and one test sheet.

1. A hidden test build that spawns one invisible unit at the champion's
   feet on a key and logs whether the fog opens around it. Answers
   questions 1, 2, 3 and 5.
2. Placement by click within reach, duration, charges, the two glyphs.
3. The enemy-side decision of section 5, and tooltips.

## 8. How the addresses were found

The host's `SimVtableV1` was located by its first field (the size 720)
followed by 89 pointers into code; its slots gave `is_visible`,
`spawn_unit` and `entity_set_invisible`. The game method tables were
located as the five or six tables of more than 67 slots whose slot `+0xf8`
they share. The sight builder was located among the functions that hold
the division constant `0x20c49ba5e353f7d` (divide by 125 after a shift by
8, that is by 32 000), the grid stride 0xe10 and the invisible counter
`+0x690`; `tools/find_context.py` found no Beta 2 match for its
instructions, the constant scan did. Tools: `tools/show_code.py`,
`tools/trace_pe.py` (`--disp`, `--call`), string scans of the executables
and the data bundle.
