# Investigation: a WASD movement mode (2026-10-23)

The question (the author): can the mod get a movement mode like League of
Legends' WASD mode ("we can just copy League's WASD mode if feasible"):
hold W, A, S, D or a combination to walk the controlled athlete in that
direction on the screen for as long as the keys are held, release to stop,
with attacks and skills aimed with the cursor; how, at what cost, and does
it fit the mod's direction?

This is a read-only study. Everything here is **read from the code** of
the mod and of games 0.6.5 (`probe/src/native_adapter/v065/`,
`research/game-builds/0.6.5`) and 0.7.0 Beta 2 (`v070b2/`,
`research/game-builds/0.7.0b2`) unless a line says **Guess**, **Rejected**
or **Read from the log**. Nothing new is seen in game; one earlier finding
seen in game is cited with its date. Addresses are RVAs of the version
named.

## Verdict

Feasible, through the official mod interface alone. The mod already sends
the game a fresh "walk to this point" order every simulation tick; WASD is
a new source for those orders. While keys are held, each tick's order is
"walk toward where the athlete is now, plus a short lead in the held
direction"; on release the order becomes the existing hold, which the mod
already turns into the game's own stop. No new native hook, no new address
to find again on a game patch. The game's own AI sends every champion a
fresh move order every tick, so per-tick orders are the engine's normal
diet. Five feel questions remain for an in-game test (section 7), and the
key layout is decided (section 5).

## 1. League's WASD mode

From Riot's two posts and the LoL wiki (sources in section 9); nothing
here is read from League's code.

- Hold W, A, S, D to walk up, left, down, right on the screen; combinations
  give diagonals. Release to stop. An option rotates the keys 45 degrees to
  the lane layout ("Rotate WASD inputs relative to map lane orientation").
  **Guess:** diagonals are normalised so they are not faster.
- Default binds: basic attack moves from right-click to left-click, the Q
  ability to right-click, the W ability to Left Shift; E and R stay. Shift
  is therefore unusable as a modifier in WASD mode. Attack-move stays a
  bindable key. "Select Object", "Interact", "Auto Attack" and "Cast
  Ability" are separate binds that may share one button.
- Abilities aim at the cursor with the usual quick cast or normal cast.
- Rules Riot added: a basic attack cannot be cancelled by a movement
  command once it begins (still by casting or by leaving vision); ranged
  champions get an added delay between attacks when movement and attack
  inputs are both used, avoided by releasing the keys before the attack
  fires (an anti-kiting rule); held keys no longer cancel an ability as
  soon as it begins (channelled ults); some champions auto-attack after an
  ability, tuned per champion.
- Walls: the champion slides around curves or stops against a wall, and is
  routed around walls when off camera.
- Camera: a "Dynamic Camera" (locked camera that drifts with the mouse,
  with a speed slider) and a middle-mouse "Scout Ahead" free look that
  snaps back on release.

## 2. How the mod moves the athlete today

**Read from the code (SDK `sdk/mod-api-stable/src/values.rs:415`, `:698`,
`:945`; both game versions).** A move order is a destination point
(`InputV1::move_to(x, y)`). The input kinds are Move 0, Return 1, Attack 2,
Skill 3, Skill2 4, Ult 5, documented as a "Mirror of game-core `Input`
discriminants"; the target kinds are None, Target, Dir, Pos, and Dir is used
by skills only. There is no "walk in a direction" kind.

**Read from the code (`probe/src/movement.rs`, `probe/src/simulation.rs:273`,
`probe/src/client.rs:188`, `:274`).** Orders are persistent (`State.target`).
The client reads the keyboard and mouse every frame (`platform_input::poll`,
in `pre_update` so a click reaches the simulation that frame). The viewed
match's worker calls `Movement::combat_input` every tick from
`Simulation::decide` and returns one `InputV1` per tick: the next waypoint
of the mod's route, the goal, an attack, or `move_to(position)`, the hold.

**Read from the code (`probe/src/native_adapter/mod.rs:304`,
`windows/movement.rs:216`).** The hold leaves a thread-local ticket
(`arm_stop`, `hold = true`); the native move hook (`move_hook`) turns it into
the game's own stop (`stop_movement`: the stop event is emitted and the
action goes 2 to 0). "Release to stop" therefore already exists and takes
one tick.

**Read from the code (0.6.2 research `research/steering-cancellation-35-notes.md`,
`docs/history/control-pass-29.md`; the same shape on 0.6.5 and Beta 2, see
the anchors below).** The game's move consumer writes the goal into the
entity (`+0x78`, `+0x80`), sets action 2 and emits a movement reset event
on every call. The action-2 dispatcher then steers each tick: along the
game's route table (eight directions over the 30 by 30 grid of 32 000-unit
cells, through cell centres `cell * 32000 + 16000`), or, when the entity's
flag at `+0x489` is set, by an arbitrary-angle integer step at the supplied
speed, clamped to the map.

**Read from the code (0.6.5 `v065/layout.rs:162-170`: steer site
`0x176601f`, steer `0x176ab80`, direct step `0x175db80`; Beta 2
`v070b2/layout.rs:172-180`: `0x1c43bf6`, `0x1c4a120`, `0x1c3b400`;
`windows/movement.rs:16`).** The mod's steer hook calls the game's direct
step for the controlled athlete when the segment from its position to the
goal is clear in the mod's copy of the wall grid (`map_path::Grid::clear`),
otherwise the game's route-table steer. The game's speed argument passes
through untouched.

**Read from the code (`movement.rs:17`, `HOLD_REPEAT`). Seen in game
2026-10-19 and 2026-10-20 by the author (game 0.6.4, CHANGELOG 1.4.1 and
1.4.2):** holding right-click re-issues the ground order to the cursor
every 100 ms, so the goal changes ten times a second, and "holding
right-click moves correctly".

**Read from the log (the mod's runtime log of the author's play, `MANUAL
input` lines, `base=` field).** The game's own AI input for a champion is
a Move to a cell centre on most ticks, re-sent every tick, with the goal
changing as the champion crosses cells (for example (208000, 784000), then
(240000, 784000), then (240000, 752000)); 102 of 187 sampled base inputs
were Move. So per-tick Move inputs with changing goals, each emitting the
reset event, are what every AI champion already gets. The mod's
`repeated_move` suppression (same goal only, selected champion only,
`windows/movement.rs:172`) was a 0.29 hypothesis about unwanted turns, not
about stutter.

**Read from the code (`movement.rs:1465-1477`, `map_path.rs:84-99`,
`camera.rs:65-77`).** A clicked goal inside a wall is snapped to the nearest
open cell centre before it is sent; a goal with no route is ignored ("no
traversable map route"). Off-map goals are never sent: the camera's
unproject rejects them and the steer hook requires every coordinate below
960 000. **Guess:** what the game's own steer does with an unreachable goal
(probably stands).

## 3. Three ways to steer

**(a) A fresh point order every tick through the SDK. Recommended.** Each
tick, while a direction is held, `combat_input` returns
`move_to(position + direction * lead)`; on release it returns the hold. It
buys everything: no new hook, nothing to re-find per patch; thread-safe (the
held direction is a small value written on the client frame and read inside
the existing per-tick lock, with no game call under a lock); no added input
delay, because it is the same path a click takes today, the one the "Show
input delay" display (`probe/src/input_delay.rs`) measures. The existing
steer hook then direct-steps clear segments and the game's path finder
handles the rest.

**(b) Write the unit's velocity or position through a native hook.
Rejected as unnecessary.** The steer hook already holds the position
pointers and the speed and could call the direct step with a goal of the
mod's choosing, but the entity only reaches the steer while it is in action
2 (walking), which only a Move input starts. So (b) needs (a) anyway and
adds nothing (a) lacks, while adding one more thing to keep right per patch.

**(c) A "walk in direction" input kind. Rejected.** None in the SDK enum
(section 2) and none in the game's input dispatch: Beta 2 `0x163a6f0` has
exactly the arms move `0x163a884`, attack `0x163a930`, Q `0x163a961`, W
`0x163a8b0`, R `0x163aa13`, and is instruction-for-instruction 0.6.5's
(`docs/migration-0.7.0b2.md`).

## 4. Details that decide the feel

- **Lead distance.** It must exceed one tick's step (move speed is in units
  per tick) so the athlete never "arrives" mid-hold, and should stay within
  one or two cells (32 000 to 64 000 units) so the clear-segment check
  usually passes. Stopping on release does not depend on the lead.
- **Wall sliding.** Mod-side arithmetic on the 30 by 30 grid: if the lead
  point's cell is blocked, try the horizontal-only and the vertical-only
  part of the direction; if both are blocked, hold. League-like on a coarse
  grid (a cell is 32 of the map's 960 screen pixels). **Guess:** the feel at
  corners when the game's route-table steer takes over for a blocked
  segment.
- **Speed and the stat rules.** `probe/src/athlete_rules.rs` touches
  collision and reaction only, never movement. Move speed, slows, roots and
  forced movement stay the game's (`can_stop_movement` keeps the game's
  effect gate).
- **Camera.** The map does not rotate, so screen-up is always the same map
  direction. The existing lock (`camera_lock` default, Y toggle), Space
  follow, edge pan, middle-drag and the I/J/K/L pan keys
  (`probe/src/settings.rs:553-556`) do not collide with W, A, S, D.
  League's lane-rotation option would be a later optional 45-degree turn.
- **Key conflicts today** (`settings.rs:450-556`): W is the W ability
  (0x57), S is Stop (0x53), A is attack-move aiming (0x41), D is free. Shift
  is the attack-move click modifier and the preview or normal-cast override
  modifier (Shift + Q/W/R), so League's "W ability on Shift" collides with
  both. The mod's keys are rebindable with two chords each; what is missing
  is a second binding set that switches with the mode. The decision is in
  section 5.
- **Skills.** Unchanged. Quick cast (cast mode 0) fires at the cursor on
  the press; normal cast (1) aims and a left-click confirms; cast on release
  (2) exists (`probe/src/abilities.rs:664-690`). Unit-targeted skills take
  the unit under the cursor or the nearest within 50 px. A right-click
  cancels aiming (`abilities.rs:633-642`); in the WASD set right-click is
  the Q ability, so the cancel is Esc or S, which already cancel.
- **Attack.** The `carry` model already lays an attack over a walk and
  decides each tick whether the walk continues: free attackers like Gunner
  keep walking, locked attackers stand (`movement.rs:615-690`). A held
  direction is simply that walk.
- **Holding a key during a cast.** `take_input` already parks a pending
  skill while the athlete's native action is above 2 and sends
  `move_to(position)` meanwhile (`abilities.rs:1260-1266`). Not sending
  direction orders while the native action is a cast (the mod observes it,
  `native_action`) and resuming afterwards is cheap and matches Riot's fix.
  What a move order would otherwise do to a cast is whatever today's
  right-click during a cast does; not read (question 2 in section 7).
- **Attack-move and Shift + right-click.** They stay in the mouse-move set;
  the WASD set keeps attack-move on a key, as League does.

## 5. Rules to decide, and the author's decision

Rules from `AGENTS.md`:

- **No anti-kiting delay.** Riot's added delay between a ranged champion's
  attacks while moving is literally input delay. The mod never adds input
  delay, so this part of League's mode is not copied.
- **Attack cancel is the author's call.** Riot made movement unable to
  cancel a begun basic attack. Today a move order cancels per the accepted
  attack-cancel behaviour; which rule the WASD mode follows is a design
  choice, to be decided before stage 3.

**The author's decision (2026-10-23), recorded apart from the findings:**
the mod gets two hotkey sets, one for WASD mode and one for mouse-move mode,
switching with the mode. The WASD set's skill hotkeys copy Riot's WASD
layout: basic attack on left-click, Q on right-click, the W skill on Left
Shift, E and R as they are, attack-move on a key. The binding-set switch
belongs with the pending settings window.

## 6. Direction check

- **Play as one athlete:** an action-game feel for the controlled athlete;
  Riot added the mode for the same reason.
- **Honest mechanics:** the steering is the player's own input; speed,
  slows and forced movement are the game's stats and effects. No stat
  alters the steering.
- **No input delay:** same path as a click, measurable with the existing
  display; Riot's kiting delay is not copied.
- **Glyphs over text:** nothing needed on the HUD; at most a mode glyph.
- **Threads:** the direction is an atomic written on the client frame and
  read in `think`; no new per-tick work on background simulations (the
  steer hook's fast reject is unchanged).
- **Console UI:** untouched. **AI-versus-AI:** vanilla; the AI's own inputs
  are untouched.

## 7. Open questions (only an in-game test answers)

1. Is the walk smooth with a new goal every tick? Held right-click at ten
   goals a second was seen working on 2026-10-19 (section 2). **Guess:**
   yes, since the AI's own champions get the same.
2. What does a direction order do during a cast or an attack wind-up?
3. How do corners feel when the game's route-table steer takes over from
   the direct step?
4. Does the sprite's facing behave with rapid diagonal changes?
5. How does the walk behave under slows, roots and forced movement?

## 8. Suggested stages

Each stage is one build and one test sheet.

1. **Core walk.** A mode setting and four bindings; the held direction in
   `Keys` (`probe/src/platform_input.rs`); a walk order in
   `probe/src/movement.rs` producing per-tick lead goals with the grid
   slide, release to hold; tests. Roughly 200 to 300 lines, no native
   change. For this test the author rebinds the W ability and Stop in the
   existing settings. Answers questions 1, 3, 4 and 5.
2. **The WASD hotkey set** of section 5, switched with the mode; depends on
   the settings window.
3. **Feel.** Hold the direction through casts and resume; the attack cancel
   rule; the camera default for the mode (lock plus follow); the optional
   lane rotation. Answers question 2.

## 9. Sources for League's mode

- Riot, "/dev: WASD Controls Are On the Way":
  https://www.leagueoflegends.com/en-us/news/dev/dev-wasd-controls-are-on-the-way/
- Riot, "/dev: WASD's Ranked Release":
  https://www.leagueoflegends.com/en-us/news/dev/dev-wasds-ranked-release/
- League of Legends wiki, "Controls and Hotkeys":
  https://wiki.leagueoflegends.com/en-us/Hotkeys
- The Spike, "How to turn on and use WASD" (third party, default binds):
  https://www.thespike.gg/league-of-legends/beginner-guides/how-to-turn-on-and-use-wasd
