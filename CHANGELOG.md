# Changelog

Every build of LT Direct Control, newest first. Entries are the technical
notes recorded when each build was verified; dates are build dates (UTC+8).

## 1.7.1 — 2026-11-04

Release of 1.7.1-dev1 to dev23 below (the author, 2026-11-04: "make 1.7.1 dev 23 official"), as one DLL for game 0.6.5 and 0.7.0. Nothing changed between dev23 and this build apart from the version number and this entry. Not seen in game as a whole: dev23 was not played before release (the author's call).

- **Pings** (dev1, dev4 to dev23): call your AI teammates from the match with the ping key and the mouse; the calls reach the game AI's own team signals and per-player orders on game 0.7.0. Details in the dev entries and docs/investigation-team-ai.md, docs/investigation-ai-signals.md.
- **Season repair removed** (dev3): the game records played matches itself since 0.6.5.
- **Fixes from an outside code review** (dev2): the settings writer, the ping and emote wheels near screen edges.
- **Not in this release:** the newly signed athletes not being selectable until the save is reloaded (found 2026-10-30, fix proposed, not built).
- **Checks:** `cargo fmt`; `cargo clippy --all-targets -D warnings` for both builds; `cargo test --release` 726 passed, 754 with `--features research`; 71 Python tool tests; `verify_build.py --version 1.7.1 --probe-tests 726` passed for 0.6.5 and 0.7.0 against the kept copies (759 anchors); package `LTDirectControl-1.7.1.zip` (179 files, SHA-256 `e6e2160e…0c46`, no personal paths). Not installed: the Steam game is already 0.7.1.

## 1.7.1-dev23 (development build) — 2026-11-04

Not seen in game. Development build, never for the Workshop. Built in both variants: `LTDirectControl-1.7.1-dev23.zip` (public) and `LTDirectControl-1.7.1-dev23-research.zip`. For game 0.6.5 and 0.7.0, verified against the kept 0.7.0 copy (Steam's folder is 0.7.1). No review (the author's call).

The author's spec section 15.6 (design/pings-spec-stage1.md, approved 2026-11-04): the walk's health rule. Everything else is dev22's.

- **15.6 "Low" means dying soon** (`pings/walk.rs`): a teammate is low when his time-to-die is under `DIE_WITHIN_SECS` 5 s, or his hp is under `HP_FLOOR` 15 % whatever the rate. Time-to-die = his hp percent ÷ the percent lost per second over `DAMAGE_WINDOW_TICKS` (3 s); infinite with no loss (`time_to_die`, `low`). **Superseded:** dev22's flat `WALK_HP` 35 %, decided at the first think.
  - **The history:** each teammate's hp percent (the SDK champion's current ÷ max hp), sampled with the team's sight every 6 ticks on the viewed match's step (`pings::observe` → `walk::record`), keeping the last 3 s and the sample before; a tick earlier than the last starts it again (a new match).
  - **At the ping, on the client:** a low teammate is not walked and his row is declined at once (`PING walk n=… to=… player=… declined hp=… ttd=…`). A teammate with no samples yet walks.
  - **In the walk:** the same rule at every think, from his latest samples, as the backstop; it hands back when it becomes true (`PING walk … end why=hp hp=… ttd=…`).
- **Tests:** 35 % with no recent loss walks; 60 % losing 25 % a second is declined (2.4 s); 14 % is declined; the samples keep 3 s, and a rate rising mid-walk ends it (`why=hp`); a new match restarts the history.
- **Left out of section 15.6:** nothing.
- **Checked for this build:** `cargo fmt`; `cargo clippy --all-targets -- -D warnings` with and without `--features research`; `cargo test` 726 passed, 754 with `--features research`; 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors) and 0.7.0 (225 anchors); the public DLL holds the walk's `declined hp=` line and none of the research text; `tools/build_dll.py` and `--research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev23 --probe-tests 726` / `754` passed for 0.6.5 and 0.7.0 (759 anchors) with `TFM2_GAME_DIR` set to `research/game-builds/0.7.0`; `package_mod.py --version 1.7.1-dev23`: `LTDirectControl-1.7.1-dev23.zip` (179 files, SHA-256 `87bda227…930e234b`; DLL `4abe1c6a…f0173dcb`) and `LTDirectControl-1.7.1-dev23-research.zip` (179 files, SHA-256 `19fa453a…c924f3eca`; DLL `c18d675e…29a45cd`). Not installed: the author installs.

## 1.7.1-dev22 (development build) — 2026-11-04

Not seen in game. Development build, never for the Workshop. Built in both variants: `LTDirectControl-1.7.1-dev22.zip` (public) and `LTDirectControl-1.7.1-dev22-research.zip`. For game 0.6.5 and 0.7.0, verified against the kept 0.7.0 copy (Steam's folder is 0.7.1). No review (the author's call).

The author's spec section 15 (design/pings-spec-stage1.md, approved 2026-11-04): the take at a pit walks the teammates itself, pits count in fog, and the mapping follows a control swap. Everything else is dev21's.

- **15.1 and 15.2 The walk** (`pings/walk.rs`, all builds; the official per-player AI interface, no hook, no game address):
  - **Registration:** the mod's one registered per-player AI (`simulation.rs`, `matches` every player, priority 1000) now also asks the walk for any player the controlled athlete's step leaves alone: `think` → the controlled athlete's input, else the walk's, else the practice hold, else Pass. No second registration.
  - **Who walks:** on a take at a pit (Take here at a pit, the armed click on a pit monster, or on the ground within `PIT_RADIUS` of a pit that is a place, 15.3), every teammate who hears it except one whose plan was a Battle at his latest step (`Orders::fighting`, published by the step hook each step; row busy) and one under `WALK_HP` 35 % at his first think (row declined). The player and the dead never hear it. Defend here at a pit, the fight call, lanes, the windows and the plan calls stay on the game's AI. The 11.9 objective order and the 11.5 finishing rule are still sent as before.
  - **Think:** for a walker of the viewed match (the match key from the read-only simulation), when the game's own input is a move, a return or nothing, it is replaced by `InputV1::move_to` the target, checked with `is_valid_input`; an attack, a skill or an ultimate passes. The target is the controlled side's rally point for that monster as the hook reads it (published with the clock, 15.3), else the pit's position.
  - **Hand back** (Pass from then on), checked in this order: health under `WALK_HP`; the pit no longer a place (`places::is_place`: the monster dead and not due within the 15 s gate); within `WALK_NEAR` 250 units of the target; `WALK_FOR_SECS` 10 s from the first think; and from the client: a newer order to that teammate (an objective, a driven window, Enemy missing's regroup, the fight call; not a fog note, a mark or a team plan), a newer take, control released.
  - **Rows:** `Ref::Walk`: walkers accepted at once (declined when his first think finds him under 35 %), fighters busy.
  - **Log:** `PING walk n=… to=… player=… start target=x,y pit=…`, `PING walk n=… to=… end why=arrived|monster|hp|replaced|released|cap ticks=… replaced=… passed=…`; research builds one line a second per walker (`PING walk n=… to=… t=… replaced=… passed=… distance=…`).
  - **Threads:** the client writes the table at the ping (`walk::start`); `think` checks one atomic, then reads the table under a short lock, never calls into the client and keeps no pointer.
  - **Left out:** "if another registered AI of higher priority decides first, Pass and log it once": the interface does not tell an AI that another decided, so it is not detectable; the mod's AI is at priority 1000.
- **15.3 Pits in fog** (`pings/places.rs`, `windows/ping_step.rs`): the step hook publishes once a second (`places::publish`, kind 0 only), per monster, up (Serpen's alive word +0x1d8, or the spawn tick reached), the ticks until it spawns (+0x1e0 Serpen, +0x1b0 Morgard, against the world tick) and the controlled side's rally point (world +0x7108 / +0x70e8 + side × 0x10; none when the world takes the builder's other branch). A pit is a place while its monster was seen alive lately (as before) or while the clock (under 2.5 s old) says up or spawning within `PIT_GATE_SECS` 15 s, at its learned position, else the map's fixed pit (`SEEDED`: Morgard 288000,288000, Serpen 672000,672000). The clock is forgotten at a new match. The armed click on the ground within `PIT_RADIUS` of such a pit is the take at that pit (`ground_or_pit`); the wheel reads the pit through `ping_place` as before. No new game address: the reads are dev18's walk-rule offsets, already anchored.
- **15.5 The mapping follows a control swap** (`signal_call::Owners`): when the roster's controlled entity is not the mapping's and the brain being stepped shows it, that brain is the mapping (`PING map re-resolved team=… index=… entity=… was …`); when no brain shows it yet the mapping is cleared (`PING map cleared: …`) until one does. The same brain with a new entity (a respawn) only updates the entity. On a change of side or index every owed order and fog note is dropped, the fight call request ended, every window closed (each puts back at its teammate's next think) and every walk ended (`why=replaced`), then `PING map swap: orders, windows and walks ended`. The respawn "disagrees" line now only shows while the roster still names the mapped entity. **Superseded:** "kept for the match once resolved" (seen in game, dev21: after a swap from mid to bottom the orders skipped mid and reached the player's own brain).
- **Tests:** the think's Pass/Replace table (attack, skill, skill 2 and ultimate pass; move, return and nothing walk) and the hand-back reasons in order; the rows (walkers accepted, fighters busy) and the client's ends; a pit in fog with a spawn 10 s away (a place) and 20 s away (not); a control swap from brain 2 to brain 3 re-resolves to 3 (mid hears again), an unknown entity clears and the next resolve is a swap.
- **Left out of section 15:** the higher-priority-AI log (above). The low-health exclusion is decided at the walker's first think, so his row reads accepted for that first tick.
- **Checked for this build:** `cargo fmt`; `cargo clippy --all-targets -- -D warnings` with and without `--features research`; `cargo test` 725 passed, 753 with `--features research`; 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors) and 0.7.0 (225 anchors, none new); the public DLL holds the walk's and the mapping's lines and none of the research text; `tools/build_dll.py` and `--research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev22 --probe-tests 725` / `753` passed for 0.6.5 and 0.7.0 (759 anchors) with `TFM2_GAME_DIR` set to `research/game-builds/0.7.0`; `package_mod.py --version 1.7.1-dev22`: `LTDirectControl-1.7.1-dev22.zip` (179 files, SHA-256 `a86e63ef…e21d84`; DLL `55349d20…25b6f1e`) and `LTDirectControl-1.7.1-dev22-research.zip` (179 files, SHA-256 `17d47082…c9619cd`; DLL `3003b1c7…79ecaff`). Not installed: the author installs.

## 1.7.1-dev21 (development build) — 2026-11-04

Not seen in game. Development build, never for the Workshop. Built in both variants: `LTDirectControl-1.7.1-dev21.zip` (public) and `LTDirectControl-1.7.1-dev21-research.zip`. For game 0.6.5 and 0.7.0, verified against the kept 0.7.0 copy (Steam's folder is 0.7.1). No review (the author's call).

The author's spec section 14 (design/pings-spec-stage1.md, approved 2026-11-04), after dev20 in game (research: the faces show, too small and cropped). Two changes; everything else is dev20's.

- **14.1 The stack's portraits at the Tab panel's size** (`pings/stack.rs`, `pings/wheel.rs`): the portrait box is 44 × 48 (`stack::FACE`, the Tab panel's), filled at sprite scale 2.0 with that box, in a 2 px frame (48 × 52). Rows are 208 × 56 (`ROW`; dev16 to dev20 184 × 34) and 62 px apart (`PITCH`; was 40); the glyphs stay 26 px, centred on the row's height, the answer glyph 4 px from the row's right edge; the countdown blocks centred on the ping row, after the fight call's portrait. The stack is at most 366 px tall, so its bottom is at most y 590, under 770. **Consequence:** the row widened to keep the twelve blocks beside the portrait, so the stack's right edge is x 214 (dev16 to dev20: 190); the game's lower centre banner starts at x 198 (its messages at y 214, 45 px tall), so it can touch the stack's top rows while a message shows. `stack-pixel.svg`'s look (the dark rows with the corner cut, the pale frame, the gold top line and blocks) is kept.
- **14.2 The research F keys removed:**
  - **F1 to F6, the lever tests:** `native_adapter/team_order.rs` and `native_adapter/windows/team_order.rs` deleted (the objectives, HideLine and vision presses, their watches and verdicts, `OBJ press`, `OBJ watch`, `OBJ lanes` and the rest of their lines, and their tests), with `Client::team_order_keys`, `nearest_teammate`, `subject::nearest` and its test, the lever keys' choice marks (`battle_help::order_shapes`) and the window's `Shape` list that only they filled (empty in public builds), the HideLine telling-apart in the delivery hooks (`signal_fate::ours`), the lever state in `signal_call::Shared`, `signal_call::tick` and `SIGNAL_HIDE_LINE`.
  - **F7 and F8, the strategy keys:** `strategy_probe.rs`'s key part (`Field`, `next_value`, `set_attempts`, `keys`, `set`, `check`, the `STRATEGY request/set/check/watch` lines and their tests). Its match-start document reads (`STRATEGY doc`, `get`, `start`) and `STRATEGY game changed` stay.
  - **F9, the Mia stamp probe:** `Fog::stamp`, the plan's stamp and follow steps, `STAMP_FOLLOW`, `ping_step::research_stamp` (`OBJ mia_stamp`) and `SIGNAL_MIA_STAMPS`.
  - **`team_order::Feed`** with the lever tests' file, and `Keys::fkeys` (research builds only).
  - **Verifier:** `SIGNAL_HIDE_LINE` and `SIGNAL_MIA_STAMPS` leave its placeholder lists. Kept, because they still guard offsets the pings use: the HideLine case's dispatch check (now with HideLine's tag 6 written in the verifier; the case anchors guard `SIGNAL_OBJECTIVE`, `SIGNAL_TEAM_PLAN` and `SIGNAL_BIG_PLAN`) and the Mia case's stamp place (now computed from `SIGNAL_VISION_POS`; it guards the vision table the fog ping writes). No anchor removed.
  - **Kept:** the ping system's research lines (`PING …`, `OBJ play …`, `PING face …`, `PING center_log children …`, `PING watch …`), the signal probe's dumps and counters, and the strategy probe's reads.
  - **Public build:** no behaviour change from 14.2; the code it lost was never run there (the shape list was always empty, the stamp always off).
- **Left out of section 14:** nothing.
- **Checked for this build:** `cargo fmt`; `cargo clippy --all-targets -- -D warnings` with and without `--features research`; `cargo test` 720 passed (dev20 721: `subject::nearest`'s test went), 748 with `--features research` (dev20 770: the lever tests', the strategy keys', the choice mark's and that one); 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors) and 0.7.0 (225 anchors); the research DLL holds none of the removed lines (`OBJ press`, `OBJ watch`, `OBJ mia_stamp`, `STRATEGY request`, `STRATEGY set` absent; `OBJ play`, `PING face`, `PING watch`, `center_log children`, `STRATEGY start`, `STRATEGY game changed` present), the public DLL none of the research text; `tools/build_dll.py` and `--research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev21 --probe-tests 720` / `748` passed for 0.6.5 and 0.7.0 (759 anchors) with `TFM2_GAME_DIR` set to `research/game-builds/0.7.0`; `package_mod.py --version 1.7.1-dev21`: `LTDirectControl-1.7.1-dev21.zip` (179 files, SHA-256 `b2f4f99d…bc3cdf5`; DLL `5754fc03…7dd001`) and `LTDirectControl-1.7.1-dev21-research.zip` (179 files, SHA-256 `167e574c…717e9003`; DLL `aac7610a…184e94c`). Not installed: the author installs.

## 1.7.1-dev20 (development build) — 2026-11-04

Not seen in game. Development build, never for the Workshop. Built in both variants: `LTDirectControl-1.7.1-dev20.zip` (public) and `LTDirectControl-1.7.1-dev20-research.zip`. For game 0.6.5 and 0.7.0, verified against the kept 0.7.0 copy (Steam's folder is 0.7.1).

The author's spec section 13 (design/pings-spec-stage1.md, approved 2026-11-04), after dev19 in game (research, 0.7.0: the fan, the V key and the take order worked, followers walked to the objective). Two changes; everything else is dev19's.

- **13.1 The research dev panel is removed:** `pings/dev_panel.rs` is deleted with its draw call, the window's panel field and `team_order::feed_lines`; nothing is drawn in its place in any build. The research log lines (`PING drive`, `OBJ play`, `PING rebuild`, …) stay in `probe.log`. The lever keys' feed (`team_order::Feed`), which only the panel read, stays bounded in memory for its own tests; its text helpers are test-only now.
- **13.2 The stack's portraits** (`pings/wheel.rs`, `pings/stack.rs`): read from the code, `Pings::face` passed sprite scale 0 to `ui_set_champion_icon`, which reports success and sizes the face to nothing (seen in game, dev19: the frame, no face, no stand-in). It now passes 2.0 with the 22 px box, as the Tab panel (44 × 48, 2.0) and the session strip do. **Also changed:** the stack wrote the portrait node's width and height (22 × 22) every frame over the icon call's fitted box; it now writes only the place and the fade, as the Tab panel leaves its icon node's size to the call. The Tab sample's key (`timing.match_key()`, the Tab panel's own) and the roster's player id (`p.id()` on both sides) were checked and are the same; no change there. Research lines: `PING face node=… name=… ok=…` at each attempt (at most once a second per node), and `PING face node=… name=none player=…` once a second for a shown row without a name.
- **Left out of section 13:** nothing.
- **Checked for this build:** `cargo fmt`; `cargo clippy --all-targets -- -D warnings` with and without `--features research`; `cargo test` 721 passed, 770 with `--features research` (the panel's own test went with it); 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors) and 0.7.0 (225 anchors, none new); the public DLL holds none of the research text (`OBJ play`, `PING face`, `center_log children`, `OBJ press` absent); `tools/build_dll.py` and `--research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev20 --probe-tests 721` / `770` passed for 0.6.5 and 0.7.0 (759 anchors) with `TFM2_GAME_DIR` set to `research/game-builds/0.7.0`; `package_mod.py --version 1.7.1-dev20`: `LTDirectControl-1.7.1-dev20.zip` (179 files, SHA-256 `55ef6787…2c734b`; DLL `faa37c3d…574ddb`) and `LTDirectControl-1.7.1-dev20-research.zip` (179 files, SHA-256 `0f0194b1…69d481`; DLL `7aca8554…1057b7`). Not installed: the author installs.

## 1.7.1-dev19 (development build) — 2026-11-04

Not seen in game. Development build, never for the Workshop. Built in both variants: `LTDirectControl-1.7.1-dev19.zip` (public) and `LTDirectControl-1.7.1-dev19-research.zip`. For game 0.6.5 and 0.7.0, verified against the kept 0.7.0 copy (Steam's folder is 0.7.1).

The author's spec section 12 (design/pings-spec-stage1.md, approved 2026-11-04), after the author played dev18 (research, 0.7.0). Three changes; everything else is dev18's.

- **12.1 No game node hidden; the dev panel moves.** Seen in game (dev18): `ingame.center_log` held only the deathmatch counter and the kill message (`PING center_log children` lines); the panel over the answer stack was the mod's own research dev panel. **Superseded:** dev18's 11.7 hide (`team_info::CENTER_LOG` in the takeover's hidden list); no `visible:` write on any game node for it now. Research builds still log the container's children. The research dev panel (`pings/dev_panel.rs`) moves from (8, 193) to `AT` (8, 470), below the stack (at (6, 224), bottom 458) and above the chat list (780), and shows at most `MAX_LINES` 14 lines (the newest; the oldest dropped first), so its bottom stays at or above 762. Public builds have no panel, as before.
- **12.2 The fan's pause:** `wheel_draw::SHOW_PAUSE` 100 ms (dev18 330); `STILL` 5 px unchanged. Drawing only; nothing drawn while the stroke moves.
- **12.3 The fan does not close on a wobble** (`pings/gesture.rs`): the part toward the wheel chooses nothing (a half, no slot) and the fan stays open and drawn; moving back out into a slot's part chooses it again; the fan closes only when the cursor is back inside the ring (`RIM` − `UNLOCK`), as in dev17. `Level2::Closed` is gone. `MOTION` 12 px (dev18 5): a shorter motion from the following centre is ignored, so a twitch fixes no corner. `REACH_X` follows from it: 144 (dev18 137). Tests: back toward the wheel past `SUB_DEAD` is no slot with the fan still open, then up is the up slot, back inside the ring closes it; an 8 px sideways twitch after the crossing fixes no corner; the panel's place and line cap.
- **12.4 V, the retreat ping** (League's caution ping; `pings/wheel.rs`, `settings.rs`, `platform_input.rs`): a new binding `ping_retreat`, default V (0x56), "Retreat ping" in the settings window, translated in every `probe/lang/*.json`; the mod's own conflict check covers it as it covers G (no game binding of V was found). V enters ping mode exactly as G does (a tap arms it until the next left press or a cancel, holding keeps it, the key alone opens nothing); the last ping key pressed sets the mode (G wins when both go down together). Its left click fires Retreat at the click point: the wheel's Retreat order, allies within `RETREAT_NEAR` of the point; a unit under the cursor only gives the point. The cursor is the plain ping ring and the preview pin the leave glyph. A drag in V's mode opens the same wheel with the same gesture. Log: `PING armed (retreat): the next left click pings retreat`, `PING n=… click retreat drive=retreat to=[…]` (or `order=mark` with nobody near). Tests: a V tap then a click is the quick ping in retreat mode; a V drag opens the wheel and fires its slot; Esc and right click cancel V's mode; G after V is the generic ping again.
- **Left out of section 12:** nothing.
- **Checked for this build:** `cargo fmt`; `cargo clippy --all-targets -- -D warnings` with and without `--features research`; `cargo test` 721 passed, 771 with `--features research`; 71 Python tool tests; `check_native_settings_layout.cjs`; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors) and 0.7.0 (225 anchors, none new); the public DLL holds none of the research text (`OBJ play`, `center_log children`, `OBJ press`, `VIS request`, the 0x30 label absent); `tools/build_dll.py` and `--research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev19 --probe-tests 721` / `771` passed for 0.6.5 and 0.7.0 (759 anchors) with `TFM2_GAME_DIR` set to `research/game-builds/0.7.0`; `package_mod.py --version 1.7.1-dev19`: `LTDirectControl-1.7.1-dev19.zip` (179 files, SHA-256 `8c934d6c…fed100e`; DLL `3d06bebf…c105ad`) and `LTDirectControl-1.7.1-dev19-research.zip` (179 files, SHA-256 `991654c9…225dc22`; DLL `349f13ea…9e43e3`). Not installed: the author installs.

## 1.7.1-dev18 (development build) — 2026-11-04

Not seen in game. Development build, never for the Workshop. Built in both variants: `LTDirectControl-1.7.1-dev18.zip` (public) and `LTDirectControl-1.7.1-dev18-research.zip`. Still for game 0.6.5 and 0.7.0 (verified against the 0.7.0 snapshot, not the Steam folder's 0.7.1).

The author's spec section 11 (design/pings-spec-stage1.md, approved and corrected 2026-11-04 evening), after the author played dev17 (one match, research, 0.7.0). Behind 11.9: docs/investigation-team-ai.md sections 27 to 31 (read from the code, nothing played). The fan is held to design/marking-menu-reference.md. Everything not named here is dev17's.

- **11.1 The fan opens where the stroke turns** (`pings/gesture.rs`, `pings/wheel_draw.rs`):
  - **The corner:** after the stroke crosses the ring on a half, the fan's centre follows the cursor while each motion of at least `MOTION` (5 px) from it stays within `STRAIGHT` (18°, half the 36° to the nearest slot) of the half's outward axis. The first motion outside that band fixes the centre where it was: the corner (a stop and a restart in a new direction is the same). Motions shorter than `MOTION` have no direction, so a hand's jitter is not a turn.
  - **Five equal parts of 72°** from the corner, measured from the outward axis, the left half mirrored: up −108°, up-and-out −36°, down-and-out +36°, down +108° (`ANGLES` right [−108, −36, 36, 108], left [−72, −144, 144, 72]); boundaries go to the earlier slot, so straight out from the corner is up-and-out. The slot counts once the cursor is `SUB_DEAD` (24 px) from the corner. The fifth part, toward the wheel, chooses nothing and closes the fan until the stroke is back inside the ring: our adaptation (Maya backs up by a dwell, which is time).
  - **Release** with the centre still following (a straight drag), in the back part or after it closed: no slot, cancelled (`no_slot`).
  - **Reading of the spec:** when the stroke crosses the ring off the outward band (straight up, straight down, 45°), the crossing point is the corner, so such a flick picks a slot with no turn (a straight-up flick on the right fires Assist me, straight down Retreat). "A straight drag never selects" holds along the outward axis. `MOTION` is the mod's own control constant (not in the spec). Once drawn, the fan moves with its centre while it still follows.
  - **Drawing (Maya's rule):** nothing of the fan while the stroke moves, corners included; drawn once the cursor holds still for `SHOW_PAUSE` (330 ms, to tune) within `STILL` (5 px), at the fan's centre (the corner, or the following centre), then kept until the fan closes. The tooltip only with the fan. The ring is drawn at once as before. A one-frame turn fires with nothing drawn. The pause gates drawing only.
  - `wheel_draw::REACH_X` is now `RIM + MOTION + SUB_DEAD + 8` = 137 (dev17 152): the cursor's reach to open and choose both halves; the out slots sit about 78 px further out than the corner and may be drawn off screen near an edge, still chosen by direction.
  - **Superseded** (dev17): `LOCK` (the fan opened 20 px past the ring), `WIDE` (nearest slot within 60°) and the slot angles −90/−30/+30/+90. A straight drag no longer fires up-and-out.
- **11.2 Answers over the champions** (`stack::Stack::heads`, `mark::heads`, both builds): every order shows each recipient's answer above his champion: the stack's glyph for him (waiting dim, accepted, busy, declined) at 2 px per art px (26 px), following him, its bottom where a pin's tip would be, for `ANSWER_SHOWN` 3 s from the last change, then the stack's 300 ms fade. Five image nodes `head0..head4`. The research fight-call reply marks (dev8, `PING reply` lines) are retired into it; the research lever keys' choice mark stays.
- **11.3 The tooltip on two lines** (`wheel_draw::tooltip`): the slot's name, then the place word in green below; a fixed strip of `TIP_WIDTH` 176 px (`TIP_LINE` 22, `TIP_PAD` 8, labels `TIP_LABEL` 160 wide); no text measured or estimated (`text_width` and `TIP_HEIGHT` removed). Where outward would leave the screen the strip goes on the slot's inner side. The spec calls 176 "the stack row's"; the stack row is 184 since dev16: 176 is used as written.
- **11.4 Enemy missing reaches every other laner** (`intent::Team::others`): the regroup window on every laner (roster slots 0, 2, 3, 4) off the pinged lane, never the jungler or the player. The fog write stays to everyone. **Superseded:** dev17's neighbouring lanes (`neighbours`).
- **11.5 Take and Defend at a pit set the finishing rule** (`intent::finish`, `plan_call.rs`): Take here at a pit and the armed click on a monster also set `object_finish` KillPriority for `plan_call::WINDOW` (20 s); Defend here at a pit sets BattlePriority (the spellings of a logged strategy, docs/investigation-ai-signals.md). Off on 0.6.5, as Poke and All-in. `object_finish` is the fourth saved field. **Decision:** the strategy windows now run per field: a call replaces only a running window that sets one of its fields (Poke and All-in replace each other; a pit order leaves a running All-in alone), each with its own serial and end. Requests queue (`KEPT` 8) and answers are kept per serial. New `PlanCall::KillFirst`, `PlanCall::FightFirst`, `PlanCall::text`. Lines `PING n= plan=kill_first|fight_first for 20 s request=`.
- **11.6 A fog write never replaces anything** (`ping_orders::Orders`): a fog ping (Enemy missing's, the armed click's, the research F9) waits in its own per-teammate slot (`notes`): it replaces neither a driven window nor an objective the teammate is still carrying out; at his step the vision write goes first, then what he is owed. A window's answer no longer overwrites a later order's. Only objective orders, fight calls and windows replace each other. **Superseded:** dev17's "a fog ping ends every teammate's window", and a fog ending a running take (found in the dev18 report, decided by the author).
- **11.7 The game's centre log hidden while controlling** (`team_info.rs`, `CENTER_LOG`): `ingame.center_log` joins the takeover's hidden panels: hidden while the player controls (re-checked each frame), its own visibility put back when control is released or the match ends. **The whole container**, because which runtime child holds the log entries is not known yet: research builds log its children two levels deep each time the set changes (`PING center_log children n= […]`, at most 12 lines per control). That also hides `dm_center` (the deathmatch countdown and overtime title, in the layout file) and most likely the kill and notification banners the game spawns there (a guess).
- **11.9 The take order and the team's own play** (`ping_orders::Play`, `windows/ping_step.rs`; read from the code, sections 27 to 31). For Take here or Defend here at a pit and the click on a monster (Serpen or Morgard):
  1. **Never planted on the publisher** (`sync_follower` at brain +0xe42 reads 0, as the last decision left it): his row accepted while the team's shared record for that monster is present, else busy; nothing written; his request ends after the standing window (5 s).
  2. **A member of the play is on it:** objective none and plan SerpenHuntAndPoke (raw 12) or EpicHuntAndPoke (raw 11): nothing written, accepted (`answer=on_it play`).
  3. **Otherwise planted as dev17,** and when the team is not on that play, **one plan rebuild** per teammate and order through the game's replace helper `0xec3ba0(team plan, plan, &buffer, &Location)` with a 0x400-byte buffer whose first u64 is 2 (ForcePassive), after the set at the same step, only when the game's walk rule would pass (Serpen alive at the kind-0 object +0x1d8, or the monster's spawn tick, Serpen +0x1e0 / Morgard +0x1b0, minus now within his travel to the side's rally point, world +0x7108 / +0x70e8 + side × 0x10, over his speed, + 2 s), never on a Battle plan (re-read right before the call; an unread plan counts as Battle), and only while `trace_on` (team plan +0x748) is 0, when the helper drops the old plan and copies the buffer without allocating. The world is `[receiver + 0x20]`; when its dword +0x7168 is not 0 the builder takes another branch and the mod asks no rebuild. The kind-0 object is the AI environment's data + 0xf078 (its kind method 0x16ab380), inside the wrapper when there is one. The helper's first 51 bytes, through its trace test, are checked at install (`PING objective setter ready …; plan rebuild ready at 0xec3ba0`). Lines `PING rebuild n= team= to= plan=<before> now=<after> objective=`, or `PING rebuild skipped …: tracing on|fighting|no plan|helper not checked`.
  - **Status for a pit order** (decision): accepted once he walks the play's hunt plan; holding the objective without it is busy (dev17's check marks without anyone moving). Lane and base orders as before.
  - **A teammate the team's play does not include while the team is on it** (team record present, not hunting, not the publisher): planted as dev17, no rebuild (the spec names only the three cases; section 30's route D, clearing his objective, is not built).
  - **Morgard:** the alive word was not read; only its spawn-tick rule counts (an alive monster's spawn tick is past, so the rule passes; a guess that the field keeps the last spawn).
  - **Probe (research builds), `OBJ play`:** at every think of each of the controlled side's brains, the player's own included, once a second per brain and at every order write (`write`): `follower` (+0xe42), `sync` (shared +0xd8, −1 none), `shared=[+0x173,+0x193]`, `own=[+0xb73,+0xb93]`, the objective, `reason=<byte>@<tick>` (+0x1b0, +0x1b8), `plan` and the walk rule per monster.
  - **Read from the code, in part:** the step calls the handler for every brain in AI kind 0 (the skip at 0xf16eb7 is kind 2's, where no rebuild is asked); the two early handler exits found pass a plan setter of their own first. Not proven for every path that a ForcePassive written before the handler is always rebuilt in that think; the probe's next line after a rebuild shows it.
  - The play's fields are read only while the teammate is owed a pit order (`Orders::owes_pit`); the research probe reads them always. After a rebuild the probe logs that teammate's next think too, so the builder's choice (12, 11 or the role's plan) shows beside the rebuild.
  - The shared block is read only when the step's context's first word is the simulation (as the publisher pick reads it); the hook now passes the step's context and receiver to the orders (`ping_step::Site`).
  - **Superseded:** section 10's label "objective play pending (0x30)"; the research dev panel names 0x30 "collided with the team's play".
- **New constants** (`v070/signals.rs`; 0.6.5 placeholders, the family off): `SIGNAL_SYNC_FOLLOWER` 0xe42, `SIGNAL_CONTEXT_SHARED` 0x10, `SIGNAL_SHARED_STRIDE` 0xea0, `SIGNAL_SHARED_SYNC` 0xd8, `SIGNAL_SHARED_RECORDS` [0x173, 0x193], `SIGNAL_BRAIN_RECORDS` [0xb73, 0xb93], `SIGNAL_RECORD_NONE` 2, `SIGNAL_PLAN_PLAY` [12, 11], `SIGNAL_PLAN_REPLACE` 0xec3ba0 with `SIGNAL_PLAN_REPLACE_PROLOGUE` (51 bytes), `SIGNAL_FORCE_PASSIVE` 2, `SIGNAL_PLAN_SIZE` 0x400, `SIGNAL_RECEIVER_WORLD` 0x20, `SIGNAL_WORLD_OTHER_RALLY` 0x7168, `SIGNAL_RALLY` [0x7108, 0x70e8], `SIGNAL_RALLY_STRIDE` 0x10, `SIGNAL_ENV_OBJECT` 0xf078, `SIGNAL_SPAWN_TICK` [0x1e0, 0x1b0], `SIGNAL_SERPEN_ALIVE` 0x1d8, `SIGNAL_WALK_SLACK` 2.
- **Other new constants (shared code):** `gesture::STRAIGHT` 18, `MOTION` 5, `PART` 72, new `ANGLES`; `wheel_draw::SHOW_PAUSE` 330 ms, `STILL` 5, `TIP_WIDTH` 176, `TIP_LINE` 22, `TIP_LABEL` 160, `REACH_X` 137; `stack::ANSWER_SHOWN` 3 s; `mark::HEAD_NODES`, `mark::HEAD` 26; `plan_call::KEPT` 8; `team_info::CENTER_LOG`. Removed: `gesture::LOCK`, `WIDE`; `wheel_draw::TIP_HEIGHT`, `text_width`; `intent::neighbours`; `signal_call::stored_replies` and the `Answer` re-export; `battle_help`'s reply marks.
- **New anchors** (`tools/native_profiles/0.7.0.json`, 225 now, 32 new): `SIGNAL_HANDLER_PLAN` 0xe3c052 (the handler passes `brain + 0x16a0` on as the plan, the pointer the mod gives the helper), `SIGNAL_REBUILD_TEST` 0xe407c4 (the handler rebuilds a ForcePassive plan after the cascade), `SIGNAL_PLAY_PUBLISHER_CALL` 0xec45a5, `SIGNAL_PLAY_FOLLOWER_WRITE` 0xec45cb, `SIGNAL_PLAY_SHARED` 0xec45db, `SIGNAL_PLAY_MIRROR` 0xec45f8, `SIGNAL_PLAY_PICK_SHARED` 0xeceb5e, `SIGNAL_PLAY_PICK_UNIT` 0xeceb98, `SIGNAL_PLAY_SERPEN_RECORD` 0xec7081, `SIGNAL_PLAY_MORGARD_RECORD` 0xec7212, `SIGNAL_PLAY_SYNC_TEST` 0xe3cfd5, `SIGNAL_PLAY_SYNC_RECORD` 0xe3d004, `SIGNAL_PLAN_REPLACE_PROLOGUE` 0xec3ba0, `SIGNAL_REPLACE_TRACE` 0xec3bc6, `SIGNAL_REPLACE_DROP` 0xec3ed0, `SIGNAL_REPLACE_COPY` 0xec3ed5 (through the copy call at 0xec3ee1), `SIGNAL_HUNT_PLAN_TEST` 0xeb7694, `SIGNAL_FORCE_PASSIVE_ARG` 0xeb7699, `SIGNAL_HANDLER_REPLACE_CALL` 0xeb76b1, `SIGNAL_WALK_WORLD` 0xed46e4, `SIGNAL_WALK_RALLY` 0xed46fc, `SIGNAL_WALK_SPAWN` 0xed4726, `SIGNAL_WALK_SPEED` 0xed47fc, `SIGNAL_WALK_RULE` 0xed4a4e (0xed4a55, 0xed4a59), `SIGNAL_WALK_HUNT` 0xed4ac3, `SIGNAL_WALK_ALIVE` 0xed4ad0, `SIGNAL_WALK_MORGARD_WORLD` 0xed1bde, `SIGNAL_WALK_MORGARD_RALLY` 0xed1bf8, `SIGNAL_WALK_MORGARD_SPAWN` 0xed1c22, `SIGNAL_WALK_MORGARD_RULE` 0xed1ef4, `SIGNAL_WALK_MORGARD_HUNT` 0xed2442, `SIGNAL_ENV_KIND0_OBJECT` 0x16ab380. `verify_native_profile.py` checks every new constant against them (`verify_take_play`) and the helper's prologue as a called function; a wrong spawn offset was tried and refused.
- **Tests** (spec section 11.8): the corner rule (a straight drag never selects; a one-frame right then up fires up; back toward the wheel cancels and closes the fan; a stop then a move picks from the stop point; jitter is no turn); the fan drawn only after a pause and then kept; the tooltip's fixed two lines; the recipient rule of 11.4; the strategy fields and per-field windows of 11.5; the fog rule of 11.6 (a window and a take keep running); the take order's publisher, member and rebuild rules; the answers over the champions.
- **Left out of section 11:** nothing by name. Not built beyond it: section 30's route D (clearing a non-member's objective while the team plays) and Morgard's alive word (not read).
- **Checked for this build:** `cargo fmt`; `cargo clippy --all-targets -- -D warnings` with and without `--features research`; `cargo test` 720 passed, 770 with `--features research`; 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors, placeholders only) and 0.7.0 (225 anchors, 32 new; a wrong spawn offset tried and refused); the public DLL holds none of the research text (`OBJ play`, `center_log children`, `OBJ press`, `OBJ mia_stamp`, `VIS request`, `STRATEGY`, the 0x30 label absent; `PING rebuild`, `PING drive`, `PING plan` present); `tools/build_dll.py` and `--research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev18 --probe-tests 720` / `770` passed for 0.6.5 and 0.7.0 (759 anchors) with `TFM2_GAME_DIR` set to the kept 0.7.0 copy (`research/game-builds/0.7.0`; Steam's folder is 0.7.1 since 2026-11-04 and matches no profile); `package_mod.py --version 1.7.1-dev18`: `LTDirectControl-1.7.1-dev18.zip` (179 files, SHA-256 `e9e7ebbc…7e17a`; DLL `7692c5d3…97009`) and `LTDirectControl-1.7.1-dev18-research.zip` (179 files, SHA-256 `31326c79…947c5`; DLL `b42d42fd…f8c4c`). Not installed: the author installs.

## 1.7.1-dev17 (development build) — 2026-11-04

Not seen in game. Development build, never for the Workshop. Built in both variants: `LTDirectControl-1.7.1-dev17.zip` (public) and `LTDirectControl-1.7.1-dev17-research.zip`.

The author's spec section 10 (design/pings-spec-stage1.md, approved 2026-11-04): one key, the two-level wheel, three driven windows. Behind the windows: docs/investigation-team-ai.md sections 20 to 26 (read from the code, nothing played). Everything not named here is dev16's: the pin, the stack and its place, the minimap pin, the cursors, the armed click, the research keys F1 to F9.

- **One key, League's model** (`pings/wheel.rs`):
  - **G enters ping mode; the mouse pings.** A tap arms it until the next left press or a cancel; holding G keeps it while the key is down (a click while held pings and ping mode stays). The key alone never opens the wheel and never fires. V and F are not used.
  - **Left click** (press and release within `DEAD`, 24 px, held still for any length): the target ping on the unit under the press point, else Take here on the ground there.
  - **Left press and drag** past `DEAD`: the wheel at the press point; the release fires the chosen slot. A release in the hub or in a half with no slot cancels (no mark). Ping mode ends with the press unless G is still held.
  - **Cancel:** right click, Esc, a command key, a press over the HUD, or 3 s after the key's release without a press. The log: `PING armed ping cancelled: right_click|esc|command|hud|timeout|no_slot|blocked`.
  - **Distance, never time:** a press that leaves the dead zone opens the wheel in the same frame even if the release comes in that frame too; the gesture takes the segment and the release fires it.
  - **Superseded** (dev16): holding the key and moving opened the wheel; the 3 s ran while the key was held.
- **The two-level wheel** (`pings/gesture.rs`, `pings/wheel_draw.rs`, the mock `design/pings/mock-wheel-2level.png`):
  - **Level 1:** the hub (26 px) and the ring 34..100 px in two halves split by the vertical axis (x = 0 counts as right): right personal, left team, each with its white marker (`personal (me)`, `team (everyone)` from `glyphs-pixel-extra.svg`) at 3 screen px per art px.
  - **Level 2:** once the stroke is `LOCK` (20 px) past the ring, the fan opens centred where the path crossed that radius (interpolated, so a one-frame flick and a slow drag put it in the same place). Four 72 px slots, 96 px from that centre, at -90/-30/+30/+90 (right) and -90/-150/+150/+90 (left). Once the cursor is `SUB_DEAD` (24 px) from the centre the slot is the nearest angle within 60°; equal distances go to up-and-out (so straight out on the right is On my way, on the left Defend here); a stroke back toward the wheel chooses none. Back inside the ring closes the fan.
  - **Control never waits for the visual:** the gesture is resolved from the cursor path alone; the drawing follows. Fades, the fan's 60 ms growth and the hovered crossfade are drawing only.
  - **Drawing:** the halves, the hub and the slot squares in the emote wheel's materials; the chosen slot framed yellow; the level-1 ring dims to 50 % while the fan is open; glyphs at 4 px per art px; the ink trail as dev16. One tooltip strip beside the chosen slot, outward: the slot's name and, where it reads the place, the place in green (`Take here · Serpen`), the only words on the wheel, translated.
  - **Near a screen edge** (`wheel_draw::REACH_X` 152, `REACH_Y` 108): the wheel's centre keeps far enough from the edge that both halves' fans can be opened and chosen with a cursor that cannot leave the screen; a fan may be drawn partly off screen near the top or bottom (its slots still chosen by direction). A stroke that starts outside the ring (a press near an edge) crosses nothing until it has been inside the ring.
  - **Superseded** (dev16): the four fixed slots (go, defend, missing, careful), the variants and the "everyone" modifier, the sub-menu, the label under the wheel, the four sectors 62..148 px and the disc.
- **The eight slots** (`pings/intent.rs`):
  - **Assist me** (right, up): the fight call on the enemy champion nearest the player within 300 units, to everyone; none near: a mark at the player.
  - **On my way** (right, up-and-out): hold the wave on the laner of the point's lane nearest the point (the nearest lane off-lane; never the jungler or the player). The hide-lane call is no longer sent.
  - **Enemy missing** (right, down-and-out): the fog ping's vision write to everyone (as dev16), the mark with "?", and a regroup window on the neighbouring lanes' laners (mid: top and bottom; a side lane: mid). The Mia call is dropped.
  - **Retreat** (right, down): the retreat window on the allies within 300 units of the point (`RETREAT_NEAR`).
  - **Take here** and **Defend here** (left, up and up-and-out): dev16's go and defend read from the place (pit, lane, base, ground).
  - **Poke** and **All-in** (left, down-and-out and down): the team strategy for 20 s (`plan_call.rs`).
  - **The armed click** is dev16's; on the ground it is Take here, and its fog ping is the vision write only (the Mia call dropped there too).
- **The driven windows** (`native_adapter/ping_orders.rs`, `Drive` and `Window`, the hook `windows/ping_step.rs`):
  - **Where:** at the existing step hook (the per-player step's broadcast call, before the same think's planner), only on the controlled athlete's teammates, in AI kind 0 only, inside the hook's `catch_unwind`. Each write only while the big plan read at that think is the variant that owns the field (PassiveLine raw 3 for the flags, Battle raw 0 or 1 for the latch); nothing allocated, no game function called. The window's end is in the game's own clock: start + seconds × the clock word `[[receiver + 8] + 0x12f8]`, the word the game multiplies for its own 5 s and 30 s (read from the code); seen in game as 60 for every player (the dev6 log, as the delivery's delay).
  - **On my way:** `hold_wave` (+0x17be) and `waiting` (+0x17bf) = 1 at each think on a PassiveLine of that lane; the values saved at the first write are put back at the end only while `line` (+0x17c4) and `push_basis` (+0x17c7) are unchanged. The client ends it on arrival within 200 units of that laner, when the player's distance to the lane's centre line has not shrunk over 2 s, or at 10 s (the worker's cap too). Refused for roster slot 4 when the game's own support rule decides `hold_wave` at that think.
  - **Enemy missing:** `regroup_now` (+0x17c2) = 1 at each think on a PassiveLine for 10 s, 0 at the end while still a PassiveLine.
  - **Retreat:** on a Battle, `well_runaway` = (1, the window's end) at +0x16c0/+0x16c8, written again whenever the tag reads 0 (never over the game's own); on a PassiveLine `regroup_now`; any other plan at the first think: Declined ("own plan"); 5 s. An early end clears only the mod's own latch (its tick is the window's end).
  - **A newer order replaces a window:** what it changed is put back at that teammate's next think, before the newer one writes (Enemy missing then Retreat on the same laner leaves `regroup_now` at 1). A fog ping (Enemy missing's, or the armed click's) goes to every teammate, so it ends every teammate's window. Control handed back to the AI, or a new match or athlete, ends every window (`PING windows ended: control released`); a pause, the shop or a heartbeat gap does not.
  - **Guess, to check in play:** a window that ends while the teammate is in a fight puts nothing back (the flags are PassiveLine's; the fight stashes the plan and its restore at 0xe74980 was not read), so a laner might keep `regroup_now` or `hold_wave` after the fight until his plan is rebuilt.
  - **Status:** Pending until the first think; Accepted at a write the plan takes; Busy while the plan does not take it; Declined at the end with none taken, at once for "own plan" and the support rule. Lines `PING drive …`, `PING drive ended … why=window|closed|support_rule|own_plan … put_back=[…]`, `PING drive replaced …`, each naming the bytes written and the plan tag; the research dev panel shows them.
- **The bottom-support rule as the code reads it** (deviation from the spec's wording): in AI kind 0, the viewed match's, the handler takes mask 0x1ab on the receiver's mode byte and either simulation word +0x218 or +0x220 being zero (0xe40b24..0xe40b77), not mask 0x1a1 and the first Serpen time minus 30 s (0xe40ad7..0xe40b1d), which is the branch for the other AI kinds. The mod mirrors the kind-0 branch with the plan, `split_role` (+0x17c0) and the own-lane Gank or Dive gates; the unit gate at `sim + 0x258 + [rbp + 0x1518] * 8` is not mirrored (its index was not read), so the mod refuses a little more often than the game.
- **Poke and All-in** (`plan_call.rs`, all builds; the official interface, but sent only where the other orders are, so on 0.6.5 the slot marks the place as every other order there): `object_battle` and `tower_press` Poking, or `object_battle` Initiating and `object_buildup` Gather, set at the match hook with the bare field name and value (seen accepted in game since dev9). The three fields are saved at the viewed match's first tick and put back at the window's end (20 s), when a later call replaces it (its other field first) and when control is released; the game itself restores the manager's strategy at the match's end (seen in dev9). Lines `PING plan n= … ok=`, `PING plan ended … restored=`. The stack shows a row per living teammate: accepted when the team took it. The game was not seen to read `object_battle` directly (investigation section 7).
- **New constants** (`v070/signals.rs`; 0.6.5 placeholders, the family off): `SIGNAL_PLAN_BATTLE` [0, 1], `SIGNAL_PLAN_PASSIVE_LINE` 3, `SIGNAL_PLAN_HOLD_WAVE` 0x17be, `SIGNAL_PLAN_WAITING` 0x17bf, `SIGNAL_PLAN_SPLIT_ROLE` 0x17c0, `SIGNAL_PLAN_REGROUP_NOW` 0x17c2, `SIGNAL_PLAN_PUSH_BASIS` 0x17c7, `SIGNAL_BATTLE_RUNAWAY` 0x16c0, `SIGNAL_BATTLE_RUNAWAY_TICK` 0x16c8, `SIGNAL_SUPPORT_SLOT` 4, `SIGNAL_SUPPORT_MODES` 0x1ab, `SIGNAL_SUPPORT_SIM_WORDS` [0x218, 0x220]; `line` reuses `SIGNAL_PLAN_LANES`' PassiveLine entry; `Objective::DIVE` 7.
- **Other new constants (shared code):** the gesture's `HUB` 26, `RING_IN` 34, `RIM` 100 (dev16: 148), `LOCK` 20, `SUB_DEAD` 24, `FAN_AT` 96, `SLOT` 72, `ANGLES`, `WIDE` 60; the drawing's `REACH_X` 152 and `REACH_Y` 108 (replacing `REACH`), `RING_BOX` 220, `SLOT_BOX` 80, `MARKER_SIZE` 39, `TIP_*`; `intent::RETREAT_NEAR` 300; `ping_orders::HOLD_SECS` 10, `REGROUP_SECS` 10, `RETREAT_SECS` 5; `plan_call::WINDOW` 1200 ticks; `wheel::COMING_FOR` taken from `HOLD_SECS`.
- **New anchors** (`tools/native_profiles/0.7.0.json`, 193 now): `SIGNAL_REGROUP_READ` 0x1106003, `SIGNAL_HOLD_READ` 0x110601c, `SIGNAL_HOLD_BUILDER_WRITE` 0xe56886, `SIGNAL_PUSH_BASIS_WRITE` 0xe568a8, `SIGNAL_SUPPORT_SLOT` 0xe40a6b, `SIGNAL_SUPPORT_MODE_READ` 0xe40aa6, `SIGNAL_SUPPORT_MODE_MASK` 0xe40b49, `SIGNAL_SUPPORT_SIM_A` 0xe40b5e, `SIGNAL_SUPPORT_SIM_B` 0xe40b6f, `SIGNAL_SUPPORT_PLAN` 0xe40b7d, `SIGNAL_SUPPORT_SPLIT` 0xe40ba8, `SIGNAL_SUPPORT_OBJECTIVE` 0xe40bb6, `SIGNAL_SUPPORT_LANE` 0xe40bd0, `SIGNAL_SUPPORT_HOLD_WRITE` 0xe40d9a, `SIGNAL_SUPPORT_WAIT_WRITE` 0xe40dd3, `SIGNAL_RUNAWAY_SET` 0xefcde7, `SIGNAL_RUNAWAY_TAG_READ` 0xefd014, `SIGNAL_RUNAWAY_TICK_READ` 0xefd01e (with 0xefd025), `SIGNAL_RUNAWAY_CLEAR` 0xefd8ae, `SIGNAL_BATTLE_DONE` 0xf15928. `verify_native_profile.py` checks every new constant against them (`verify_drive_windows`).
- **Removed:**
  - The inbox calls: stop (careful's), Mia (the fog ping's) and the hide-lane call (I'm coming's); `SIGNAL_STOP`, `SIGNAL_MIA` and their tag checks in the verifier (their case anchors stay); "leave" (careful on a pit).
  - **F10** (the research back-off probe): `Action::StopRecord`, `SIGNAL_STOP_RECORD` and its stride, their verifier checks. The Mia stamps (F9) stay.
  - Lang keys GO, DEFEND, MISSING, CAREFUL, that laner, with a fight, everyone; added: the eight slot names, Serpen, Morgard, base, Top lane, Mid lane, Bottom lane in every `probe/lang/*.json`.
- **Assets** (`tools/generate_ping_assets.py`, 53 ping pictures): reads `glyphs-pixel.svg` and `glyphs-pixel-extra.svg` by label; new glyphs assist, poke, all in (the contest swords until a sheet has the label `all in`: the generator then takes it with no change; if its colour is not the contest orange, `Glyph::color` needs that one value too, which the colour test names), the markers at 39 px; the halves, the hub and the slot square with their hovered pictures. Removed: careful, everyone, the sectors and the disc.
- **Left out of section 10, and why:**
  - The `base_plan` +0x1bc2 write during a fight (section 25's Retreat note): not in the spec's constant list.
  - A separate `SIGNAL_PLAN_LINE`: the spec says reuse `SIGNAL_PLAN_LANES`.
  - The support rule's unit gate (see above).
- **Deviations:**
  - The support rule's kind-0 branch (see above).
  - "Press, right, up" fires Assist me (straight up is -90); On my way is right then up-and-out (-30), as the slot angles say. The tests use that stroke.
  - Ping mode ends with a cancelled drag (the spec: armed until the next left press); dev16 kept it.
  - On my way's arrival is measured to the laner's unit (dev16's rule); "coming" is the distance to the lane's centre line shrinking between two checks 2 s apart.
  - On my way picks the laner by roster lane and position (the client cannot read plans); the worker then checks his plan and lane at each think. On bottom, when the support is nearer, the hold goes to roster slot 4 and may be refused by the support rule while the carry could have held.
  - On my way's client checks (2 s, the 10 s cap) run on wall-clock time; the worker's window on match ticks. A pause longer than 2 s can end the hold as "not coming".
  - The `PING drive` lines log in public builds too, on status changes only (the spec names research feed lines; the research dev panel shows them).
  - The tooltip's lane words read "Top lane", "Mid lane", "Bottom lane" in English (the spec: "Top/Mid/Bottom"), on their own keys: the existing `Top`/`Mid`/`Bottom` keys name roles, and some languages word them so (pt-BR "Base", ru "Керри").
  - Serpen and Morgard keep their names in every language.
  - The tooltip's text widths are estimated (no text measuring call): about 0.55 of the size per Latin letter, the full size per wide character.
- **Checked for this build:** `cargo fmt`; `cargo clippy --all-targets -- -D warnings` with and without `--features research`; `cargo test` 712 passed, 763 with `--features research`; 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors, placeholders only) and 0.7.0 (193 anchors, 20 new); `tools/generate_ping_assets.py` (53 pictures); the public DLL holds none of the research text (`OBJ press`, `OBJ mia_stamp`, `OBJ stop_record`, `VIS request`, `STRATEGY` absent; `PING drive`, `PING plan`, `PING coming ended`, `PING windows ended` present); `tools/build_dll.py` and `--research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev17 --probe-tests 712` / `763` passed for 0.6.5 and 0.7.0 (727 anchors, 177 asset files, 53 of them ping pictures, 16 cursors); `package_mod.py --version 1.7.1-dev17`: `LTDirectControl-1.7.1-dev17.zip` (179 files, SHA-256 `36b3d7ce…464d`; DLL `a5170e23…e24c`) and `LTDirectControl-1.7.1-dev17-research.zip` (179 files, SHA-256 `0647cc6a…36ac4`; DLL `72990385…288e`). Not installed: the author installs.

## 1.7.1-dev16 (development build) — 2026-11-04

Not seen in game. Development build, never for the Workshop. Built in both variants: `LTDirectControl-1.7.1-dev16.zip` (public) and `LTDirectControl-1.7.1-dev16-research.zip`.

After dev15 in game (the author and the dev15 log, 2026-11-04). No lever changes; input, resolution and feedback change. Everything else is dev15's, F9 and F10 included.

- **League's input model** (`pings/wheel.rs`):
  - The ping key enters ping mode, with the cursor and the pin preview as in dev15. In ping mode the mouse does everything.
    - **Click:** a left click is the quick ping on what is under the cursor: a unit the team sees, else the ground.
    - **Drag:** a left press that moves past a 24 px dead zone (`DEAD`) opens the wheel at the press point; the drag selects and the release fires. Released inside the dead zone, it is the quick ping at the press point.
    - **Holding the key** and moving past the dead zone also opens the wheel.
  - Released in the wheel's centre, the wheel closes and ping mode stays.
  - **Cancel:** right click, Esc, a command key, a click over the HUD, or 3 s. The log says why: `PING armed ping cancelled: right_click|esc|command|hud|timeout` (`blocked` when a window or takeover ends it).
  - **Superseded** (dev15): the tap armed and the next left click fired, and holding the key opened the wheel at once on the press.
- **The sub-menu, Maya's rules** (`pings/gesture.rs`, `pings/wheel_draw.rs`):
  - **Lock:** once the stroke crosses the rim and travels 20 px further (`LOCK`), the slot locks. A slot with variants opens its sub-menu centred where the cursor IS at that moment.
  - **Compass items:** the slot's own direction is plain, and the variants sit at the neighbouring compass points, 90° apart. Go: with a fight on the left, everyone on the right. Defend: everyone below.
  - **Dead zone and back strokes:** within 24 px of its centre (`SUB_DEAD`) the release is plain, and so is a stroke backwards. Crossing back inside the rim closes it. No delay.
  - **Drawing:** the ring's own sector pictures and disc at 0.6; the main ring dims to 50 %. The stroke leaves a faint ink trail from the press point, drawn lines like the emote wheel's (`TRAIL_COLOR`).
  - **Superseded** (dev15): the sub-ring centred on the rim crossing, variants 60° apart, and its 11 pictures. 50 ping pictures now.
- **Pings while dead:**
  - **Where targets come from:** the team-visible unit list from the simulation (`simulation::team_units`: the team's own units and every one the game shows that side), not the hover list, which is empty then.
  - **Why the feed moved:** the feed (`pings::observe`) now runs once a tick from any living actor of the viewed match, because the controlled athlete's own step stops while he is dead.
  - **Unchanged:** a click on a unit the player's vision shows but the team cannot see is still the fog ping.
- **A row for every ping** (`pings/stack.rs`, `ping_orders::Status`, `signal_call::fight_answers`):
  - **When the ping fires:** every recipient gets a row at once, waiting (the ping's own glyph, dim). It becomes the answer when it comes.
  - **Calls:** the stop, Mia, hide-lane and fog calls show sent (a dim check) once written. A call never written in its window shows unsent (a dim cross); so does a fight call that ended without reaching a teammate, and an objective the setter could not write.
  - **Nobody to call:** a ground mark shows only the ping row.
  - A waiting row lasts until its answer or the stack's end, and does not keep the stack alive.
- **The verdict bug** (`ping_orders::SAME_REASON`): a standing request that the AI clears with the same fresh reason three times in a row is refused with that reason and stops (`ended=refused same_reason x3 … reason=0x06`). dev15 rewrote "defend mid" 17 to 22 times in 5 s with reason 0x06 and reported result=none.
- **Own-lane orders** (`intent::Team::crew`): when the lane is the controlled athlete's own, go, defend and careful there go to everyone (the four teammates). "I'm coming" on the own lane is the ground's go: the fight call on a near enemy the team sees, else a mark. dev15 marked the place and sent nothing for the mid laner.
- **"I'm coming" arrival:** within 200 map units of the laner the call was sent to (his unit), not within 400 of the lane line. The 2 s re-send and the 10 s cap stay.
- **The stack's place, interim** (`STACK_AT` (6, 224), right edge at x 190, rows 184 px wide, at most 458 px down):
  - The author said only "partially blocked by the in-match log", without a position.
  - The 0.7.0 in-match layout (`asset/base/ui/layout/ingame`) has three candidates:
    - `ingame.kills`, the kill feed, right-aligned near x 848–978 from y 141;
    - `ingame.chat_list`, team chat lines, at x 19 from y 780;
    - `ingame.center_log`, the centre banners, at about x 198–798 and y 164 and 214.
  - This place clears all three. dev17 moves it if the author names another panel.
- **Deviations:**
  - **Row width:** 184 px instead of the design's 200, to keep the right edge at x 190; the countdown starts 4 px after the portrait.
  - **Release in the wheel's centre:** after a drag, closes the wheel and keeps ping mode (a League-style cancel). It does not ping.
- **Checked for this build:** `cargo fmt`; `cargo clippy --all-targets -- -D warnings` with and without `--features research`; `cargo test` 702 passed, 753 with `--features research`; 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors) and 0.7.0 (173); `tools/generate_ping_assets.py` (50 pictures); the public DLL holds none of the research text (`OBJ stop_record`, `OBJ mia_stamp`, `OBJ press`, `STRATEGY` absent); `tools/build_dll.py` and `--research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev16 --probe-tests 702` / `753` passed for 0.6.5 and 0.7.0 (707 anchors, 174 asset files, 50 of them ping pictures, 16 cursors); `package_mod.py --version 1.7.1-dev16`: `LTDirectControl-1.7.1-dev16.zip` (176 files, SHA-256 `40832a32…5580`; DLL `dcfe8c3b…fd53`) and `LTDirectControl-1.7.1-dev16-research.zip` (176 files, SHA-256 `aa8b0f66…874e`; DLL `1ac40fd1…55e7`). Not installed: the author installs.

## 1.7.1-dev15 (development build) — 2026-11-04

Not seen in game. Development build, never for the Workshop. Built in both variants: `LTDirectControl-1.7.1-dev15.zip` (public) and `LTDirectControl-1.7.1-dev15-research.zip`.

The author's revision after dev13 and dev14 in game (design/pings-spec-stage1.md, section 9, and the six rules added the same day): "here" versus "that", pixel art, the ping cursors, the answer stack. It supersedes dev13's and dev14's input, order and feedback rules where they differ.

- **Input** (`pings/wheel.rs`, `pings/gesture.rs`):
  - **Tap G** arms a target ping. The cursor becomes the ping ring, or the on-target ring over a unit the click would ping (an enemy champion, a live pit monster, a tower). A small pin with the glyph the click would give follows the cursor. The next left click fires on the unit under it, or on the ground there as a ground "go". Right click, Esc, a command key (attack-move, stop, recall, a skill) or 3 s cancel. While armed, the left click is reserved through `left_reserved` (`client.rs`), so it is never a selection or a move. The firing click is drained until released, and so is a cancelling right click. A click over the HUD cancels and stays the HUD's.
  - **Hold G**: the fixed wheel. Go is up, defend right, missing down and careful left, and the slots never change or grey. A stroke that crosses the rim on go or defend opens the sub-ring (Maya's marking menu): a ring 0.6 of the size, centred on the crossing point, with its variants over the outward half 60° apart. The main ring dims to half. Inside the sub-ring's own disc it fires straight on; crossing back inside the rim closes it. Fast strokes select as they go, with no pause. Go's variants are with a fight, plain (straight on) and everyone; defend's are that laner (straight on) and everyone.
  - **Superseded 2026-11-04** (dev13, dev14): the tap fired the default of what was under the cursor, snapping to an enemy champion; the wheel's slots followed that target and greyed; the rim arc's notches. The snap rule is removed from `pings/subject.rs`.
- **Orders** (`pings/intent.rs`). The wheel works by the place at the press point: a live pit, the own base, a lane, or the ground. The armed click works by the unit under the cursor.
  - **Ground**: go (any variant) sends the fight call on the nearest enemy champion the team sees within 300 map units; otherwise a mark only.
  - **Lane**:
    - go: Nexus(lane) to that lane's laner; everyone: to all four.
    - go with a fight: "I'm coming", the hide-lane call to the lane's laner. It is sent again every 2 s while the player is on the way, for up to 10 s or until he is within 400 map units of the lane's centre line.
    - defend: DefenseLine(lane) to the lane's laner; everyone: to all four.
    - careful: the stop call to the lane's laner (inert in the viewed match; logged).
  - **Pit, monster alive**: go is Setup without battle (Serpen 0x000101, Morgard 0x000100); go with a fight is Setup with battle (0x010101 / 0x010100); defend is the same as with a fight; careful clears our own order (leave).
  - **Own base**: defend sends the Defense objective (2) to everyone; go is the nearest lane's go.
  - **Missing anywhere** is the fog ping (below).
  - **Clicks**: an enemy champion the team sees gets the fight call; a live pit monster, take; a tower, push its lane; an ally, nothing yet (reserved); anything else, the ground's go. An order nobody can hear, or that this version cannot send, marks the place.
- **The author's six rules (same day)**:
  - **On it** (`ping_orders::on_it`, `signal_call::Request::decide`). A teammate already doing what is asked is written nothing; his answer is accepted and the order is his:
    - fight call: his big plan is Battle (`brain + 0x16a0` raw 1) on the pinged enemy (`+ 0x16a8`);
    - objective: the same tag in any phase (Serpen Setup is never written over Serpen Hunt), or the same lane order on the same lane;
    - I'm coming: his Gank on that lane.

    Logged `answer=on_it`.
  - **Busy means fighting someone else**. After the scene call, a teammate the game answered "in battle" whose Battle now targets the pinged enemy is accepted (`answer=on_it`, `signal_fate::repointed`); otherwise he is busy.
  - **The ping lives for its pin**. A fight call lives 4 s (`REQUEST_TICKS` 240; it was 2 s, ending once all were written). A busy teammate who leaves his fight within that time is written once more and his row flips to the new answer. Nobody else is asked again.
  - **Repeats**. The same ping (same order and target or place) while its pin lives refreshes the pin and the stack's clock and asks nothing new (`PING … repeat refreshed`). For a fight call, a still-busy teammate may be asked once more. Throttles:
    - at most one fight-call write per teammate per second (`WRITE_EVERY`);
    - every inbox write needs two free entries (`signal_send::append`; it was one), so the AI's own calls keep room; a write without room is tried at a later step.
  - **Team vision**. A click names a unit only if the controlled athlete's team sees it (`sim.is_visible(side, id)`, read every 6 ticks, `pings::team_sees`), whatever the player's vision choice shows. The ground's nearest enemy follows the same rule.
  - **The fog ping** (`ping_orders::Fog`). It comes from missing on the wheel, or from a click on a unit the team cannot see.
    - The enemy is that lane's enemy laner on a lane; otherwise the first recipient's longest-unseen enemy among those the team cannot see, the same for everyone.
    - Every teammate gets that enemy's vision entry at the point (position; last seen and last checked now; the squared distance from his unit), as dev10's F6 wrote it, and then the Mia call naming him.
    - The pin shows the missing glyph at the point.
    - The write is in every build now; `squared_distance` and `vision_writes` moved to `ping_orders.rs`.
- **Visuals**:
  - **Glyphs**: the author's 13 pixel glyphs (`design/pings/glyphs-pixel.svg`) and the "everyone" placeholder (`glyphs-pixel-placeholders.svg`). They are read back cell by cell, matched by their labels, and written at 4, 2 and 1 screen pixels per art pixel (52, 26 and 13 px), so every pixel is an art pixel's colour; the generator asserts each glyph has one colour plus the outline.
  - **Ring and sub-ring**: dev14's sectors and disc in the emote wheel's materials, and the sub-ring at 0.6 of the size.
  - **Pin**: a placeholder pixel pin (34 × 40: a dark box with an outline and a point) carrying the order's glyph as its own node; small (17 × 20) on the minimap.
  - **61 pictures**: `tools/generate_ping_assets.py`; dev14's 45 removed.
- **The answer stack** (`pings/stack.rs`, `design/pings/stack-pixel.svg`). It sits top left, 8 px under the target state panel, and replaces the card.
  - **The ping row**: the order's glyph, the enemy's portrait for a fight call, and 12 countdown blocks over the pin's 4 s; its top line is in the glyph's colour.
  - **Answer rows** go in roster order. Each has the teammate's portrait (the game's champion icon through `ui_set_champion_icon`, as the Tab panel; a dark square until it loads) and the answer glyph: check for accepted or on it, hourglass for busy, cross for declined. A row slides in over 150 ms, holds 3 s and fades over 300 ms; a changed answer pulses and holds again.
  - **No answer, no row**: pending, the inert stop and Mia calls, and the fog ping show no row.
- **Cursors**: `Style::Ping` and `Style::PingTarget`, from `design/cursor/index_ping.html` (`ping` and `pingLock`, hotspot 16,16), rendered by `tools/generate_cursor_assets.cjs` like the other 14, now from that page. The 14 existing cursors are byte-identical.
- **Research only**:
  - **F10, the back-off probe**: on the teammate nearest the cursor, at his step, the record the inbox's BattleStop case writes in the other match kinds, then his plan at his next 20 steps (`OBJ stop_record`). Read from the code at 0xe675a0..0xe675c1: 16 bytes per sender at `brain + 0x890 + sender * 16`, a u64 1 then the u64 world tick (from the AI environment's `+ 0x1b8`). The sender is the handler's sixth argument, the roster index the receive gate compares with the receiver's own (0xe65ddd); the mod writes the controlled athlete's index. `SIGNAL_STOP_RECORD` and `SIGNAL_STOP_RECORD_STRIDE` (0.6.5 placeholders); the verifier now reads both from them and checks `mov eax, ebx; mov rcx, [rdi + world tick]; shl rax, 4`.
  - **F9, the stamp probe**: a fog ping at the cursor point that also writes the Mia stamp (`brain + 0xc40 + k * 8` = now, `SIGNAL_MIA_STAMPS`) on every teammate, then logs their plans for 20 steps (`OBJ mia_stamp`).
  - **Superseded**: dev13's F10 (Serpen Hunt) and dev12's F9 (Morgard Setup); the wheel reaches Morgard now. F7 and F8 stay the strategy keys.
- **Deviations and guesses**:
  - **The wheel fires about the press point**, not the release point: the release of a flick lies 150 to 240 px from where the player aimed.
  - **Defend's "that laner"** is the lane's own laner (dev14: the laner nearest the point).
  - **Focus has no glyph of its own**: the fight call shows contest's crossed swords, so a focus pin and a contest pin look alike.
  - **Bases (guess)**: side 0's base is the (0, 960) corner, side 1's the (960, 0) corner, radius 130 map units. Every lane meets there, so the base's go is the lane nearest the press point.
  - **Pits** count only while their monster was seen alive within 2.5 s.
  - **The cursors' ink** is the generator's `#1c1c1c`, as for the other 14, not the page's `#1c1a18`.
  - **The 400-unit arrival rule** often ends "I'm coming" at once: on the 960-unit map, most of the jungle lies within 400 units of some lane line, the mid line especially. Each re-send logs the distance.
  - **No row for inert calls**: the stop and Mia calls are heard and do nothing in the viewed match, so they show no answer rather than a check.
- **Checked for this build:** `cargo fmt`; `cargo clippy --all-targets -- -D warnings` with and without `--features research`; `cargo test` 700 passed, 751 with `--features research`; 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors, placeholders only) and 0.7.0 (173 anchors; the stop record and Mia stamps checked against their constants); `tools/generate_ping_assets.py` (61 pictures) and `tools/generate_cursor_assets.cjs` (16 cursors, the 14 old ones byte-identical); the public DLL holds none of the research text (`OBJ stop_record`, `OBJ mia_stamp`, `OBJ press`, `VIS request`, `STRATEGY` absent; `PING fog`, `PING armed`, `answer=on_it`, `repeat refreshed` present); `tools/build_dll.py` and `--research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev15 --probe-tests 700` / `751` passed for 0.6.5 and 0.7.0 (707 anchors, 185 asset files, 61 of them ping pictures, 16 cursors); `package_mod.py --version 1.7.1-dev15`: `LTDirectControl-1.7.1-dev15.zip` (187 files, SHA-256 `81415c9d…2b07`; DLL `1ffb91c3…1f84`) and `LTDirectControl-1.7.1-dev15-research.zip` (187 files, SHA-256 `0f0e9301…47ac`; DLL `f6e83fe8…c4d3e7`). Not installed: the author installs.

## 1.7.1-dev14 (development build) — 2026-11-04

Not seen in game. Development build, never for the Workshop. Built in both variants: `LTDirectControl-1.7.1-dev14.zip` (public) and `LTDirectControl-1.7.1-dev14-research.zip`.

**Seen in game with dev13 (the author, 2026-11-04):** the wheel drew as a solid white disc with no glyphs, and the rim arc appeared slowly. The cause was dev13's tinted white SVG layers: the game drew each file's own white fill and ignored the tint. The October drawing probe only ever tinted white, so the tint was never proven; dev13's "read from the code" claim that the tint works is **superseded**.

dev14 follows the author's instruction, "copy the emote wheel and expand it", as amended twice the same day. The ring of sectors stays, in the emote wheel's materials; the celestial style waits until this works in game ("clean, simple, high quality; we change the style once it works in game"). Everything else is dev13's: the orders, the standing request, the research keys and the log lines.

- **Rendering** (`pings/wheel_draw.rs`, `pings/mark.rs`, `pings/wheel.rs`):
  - **How it draws:** as the emote wheel does. A template spawned with `ui_spawn_source` holds the wheel as `:image` nodes, written every frame through the property cache: place, size, `source:` and `color: #ffffffAA` for the fade.
  - **Pictures:** 45 PNG files with their colours baked in, made by `tools/generate_ping_assets.py` with sharp (`tools/rasterise_svg.cjs`) at exactly their shown size. dev13's 68 SVG files are removed.
  - **Ring geometry:** the mock's. Four sectors from 62 to 148 px with a 6 px gap between them, a 57 px centre disc, and the rim arc's notches from 151 to 194 px (three up, two right).
  - **Materials:** the emote slot squares'. Fill `#1c1a18` at 0xee over a 2 px `#4b4a49` frame, the soft shadow (`#000000` at 0x66, a little left and down), the hovered sector or notch framed `#fdee00` with a lighter fill. No gold rim and no ornament.
  - **Greyed:** the sheet's greyed glyph, with the slot at half alpha.
  - **Glyphs:** the celestial glyphs from `glyphs-celestial-a.svg`, at the emote face's 64 px in a slot and 40 px in a notch.
  - **Pins:** the celestial pin with its glyph baked in, one per intent plus the fight call's focus (red swords), and the minimap pin in four colours.
  - **The only text:** the label under the wheel, as the emote wheel has one: the hovered notch's name, else the slot's.
  - **The card:** its box and seconds stay drawn as before. Its glyph is an image node now.
- **Timing:** four named constants in `wheel_draw.rs`.
  - `FADE_IN` 60 ms. The wheel shows at once on hold; dev13's 150 ms draw delay is removed.
  - `ARC_FAN` 60 ms. The rim arc shows with no pause, as soon as a slot with variants is hovered in the ring (past 62 px) or locked; dev13 waited for a 150 ms pause.
  - `PULSE` 150 ms.
  - `CANCEL_FADE` 80 ms.
- **The early-inbox gap:** the dev13 log showed `PING dropped: queue full len=0/0` at 0:14, and the fight call expired unsent. The 2 s retry is kept. The card now shows an hourglass while no teammate has been written to, and a cross once the order ended unsent; the seconds show only once sent (`mark::Delivery`, `signal_call::fight_delivery`, `ping_orders::delivery`).
- **Deviations from the brief:**
  - **When the rim arc shows:** in the ring past 62 px, not past 100 px. The amendment says "beyond the inner radius".
  - **Greys not themed:** the frame and fill greys are the emote wheel's design greys, without the HUD theme's tone, which a picture cannot take.
  - **Pins:** five, not three: four intents plus the focus.
- **Files:**
  - **Rewritten:** `pings/wheel_draw.rs`, `pings/mark.rs` and the window part of `pings/wheel.rs`; `tools/generate_ping_assets.py`.
  - **New:** `tools/rasterise_svg.cjs`.
  - **Changed:** `pings/gesture.rs` (no pause), `pings/intent.rs` (the SVG layer table removed), `native_adapter/signal_call.rs` and `ping_orders.rs` (delivery), `windows/ping_step.rs`, `tools/verify_build.py` (169 asset files), `tools/records/ping-assets.json`, `tools/records/ui-graphics.json`.
- **Not verified:** anything in game. This includes how the image nodes stack with the HUD, the gesture's feel at the new timings, and everything dev13 lists.
- **Checked for this build:** `cargo fmt`; `cargo clippy --release --all-targets -- -D warnings` with and without `--features research`; `cargo test --release` 692 passed, 743 with `--features research`; 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors) and 0.7.0 (173); the public DLL holds none of the research text (`OBJ verdict`, `OBJ press`, `VIS request`, `STRATEGY` absent); `tools/build_dll.py` and `--research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev14 --probe-tests 692` / `743` passed for 0.6.5 and 0.7.0 (707 anchors, 169 asset files, 45 of them ping pictures); `package_mod.py --version 1.7.1-dev14`: `LTDirectControl-1.7.1-dev14.zip` (171 files, SHA-256 `9cdb41bb…768c`; DLL `cb64ca8e…4878`) and `LTDirectControl-1.7.1-dev14-research.zip` (171 files, SHA-256 `525a5d99…94e0`; DLL `ddf3f4f9…8107`). Not installed: the author installs.

## 1.7.1-dev13 (development build) — 2026-11-04

Not seen in game. Development build, never for the Workshop. Built as both variants: `LTDirectControl-1.7.1-dev13.zip` (public) and `LTDirectControl-1.7.1-dev13-research.zip`. Ping system stage 1 (design/pings-spec-stage1.md, the author's go-ahead of 2026-11-04): the research probes of dev7 to dev12 become the feature, in the public build too. The research build keeps F1 to F9 (plus a new F10), the `OBJ`, `VIS` and `STRATEGY` lines and the dev panel. Stage 2 (the answer stack, portraits, sounds) and stage 3 (the team-plan arc, the gank steer) are not in this build. **On game 0.6.5 the signal family keeps its placeholders and stays off: the wheel works and every choice fires the place; nothing is sent.**

- **Input** (`pings/wheel.rs`, `pings/gesture.rs`):
  - **Tap G:** the default intent for what is under the cursor ("go"), without showing anything.
  - **Hold G:** the wheel, with four directions: go (up), defend (right), missing (down), careful (left). The press-point edge rule is kept.
  - **Gesture:** the path selects and the drawing follows. A flick into a direction picks the slot. Going on past the rim (148 px) locks that slot, taken where the stroke crossed the rim on the segment between two frames, so a fast flick keeps it. Past 151 px a slot with variants picks a notch by angle; release fires. Coming back inside the rim unlocks.
  - **Drawing:** the wheel draws once the key has been held 150 ms (`DRAW_DELAY`; a placeholder, so a tap or a quick flick never shows it). The rim arc draws on a 150 ms pause on the slot, or when the cursor reaches it.
  - **Greyed:** a greyed slot or notch fires the place. Teammate portraits no longer anchor a ping.
- **Targets and orders** (`pings/intent.rs`, `pings/places.rs`):

  | Target | Go (tap) | Defend | Missing | Careful |
  | --- | --- | --- | --- | --- |
  | Enemy champion | fight call to every teammate | greyed | Mia with his index | stop call |
  | Serpen or Morgard pit | take, plus the rim arc take / contest / leave | greyed | greyed | greyed |
  | Lane or tower | push, plus the rim arc I'm coming / push / everyone | DefenseLine, plus the rim arc that laner / everyone | Mia with that lane's enemy laner | stop call |
  | Empty ground | the place | the place | the place | the place |

  - **Recipients:** never the controlled athlete. "Everyone" is every living teammate. A lane's laners are slot 0 for top, slot 2 for mid, slots 3 and 4 for bottom; never the jungler. Defend goes to the laner nearest to the point.
  - **The with_battle byte:** read from the code, it is byte 2 (u24 bits 16 to 23); byte 1 is the phase. The Debug formatter `0xed9170` prints, for Morgard and Serpen, byte 1 as `phase` and byte 2 as `with_battle` (case `0xed9192`, names at `0x4018098`).
  - **Take and contest:** take is Setup without battle (Serpen `0x000101`, Morgard `0x000100`), contest is Setup with battle (`0x010101`, `0x010100`).
  - **Deviation:** the brief's "take = 0x010001" would change the phase to None and keep the battle flag, so it is not used.
  - **Leave** writes none only where the objective the mod planted still holds.
  - **Lanes (guess):** the lane is the nearest of three centre lines, within 70 units. The lines are measured from the dev12 log's PassiveLine positions: top x 73 then y 49, mid x + y 960, bottom y 911 then x 887. Further out is empty ground.
  - **Pits:** a pit is a point within 250 units of the Serpen or Morgard unit's position. The controlled athlete's step learns that position once a second from the units named `serpen` and `epic` and keeps it for the session (`PING pit learned`). Guess: the units carry those names. Until a monster was seen, its pit is a lane or the ground.
- **Standing request** (`native_adapter/ping_orders.rs`, `windows/ping_step.rs`):
  - **Writing:** the setter is called at each recipient's own step, with tracing off. The reason byte and its tick (brain + `0x1b8`, new) are snapshotted after the write.
  - **Window:** for 5 s the request is written again only while the teammate holds none, and never over an objective the AI chose.
  - **Stops at once:** on a permanent reason written after the mod's own write: 0x21, 0x24, 0x03, 0x05, 0x08, 0x20, 0x23, 0x26, 0x28. An old byte from minutes before does not stop it.
  - **Asks through:** 0x29, 0x30, 0x31 and every other reason.
  - **Verdict:** one `PING ended n= to= objective= ended=held|window|ai_choice|refused held= retries= reason=` line per teammate, in both builds.
  - **First write:** it replaces whatever the teammate held; that is the player's order. Only the re-writes wait for none.
  - **Calls** (stop, Mia, I'm coming): written when the inbox has room, tried again at later steps for 2 s.
- **Fight call:** due now (dev12: tick + 15), so it is heard at the next delivery.
- **Read from the code: the stop and Mia calls do nothing in the viewed match.** The receive handler's Mia (tag 1, `0xe6750b`) and BattleStop (tag 5, `0xe6758a`) cases end at the AI environment's kind check in kind 0, which the viewed match is. So the careful and missing slots are written, heard and ignored. In other kinds, Mia stamps `brain + 0xc40 + enemy * 8` with the tick, and BattleStop writes `brain + 0x890 + sender * 16` = (1, tick). Untested in game; logged like the other calls (`PING call sent`).
- **Research build:** F10 plants Serpen, Hunt, with battle (`0x010301`) on the nearest teammate, to measure against F1's Setup. F1 to F9 and their lines are unchanged.
- **Visuals** (`pings/wheel_draw.rs`, `pings/mark.rs`, `tools/generate_ping_assets.py`, `probe/ui/pings/`, 68 SVG files; the author's four design files are read, never written):
  - **The SVG loader:** seen in game on 2026-10-24 (1.6.0-perf-dev2), `draw_svg` fills every path, ignores strokes and tints the whole file with one colour. Not known: whether it keeps a path's own fill colour. So each glyph is split by colour into single-colour, fill-only layers tinted when drawn: the main shape, the gold accent, and the dark outline (the design's 2.4 px stroke turned into the shape grown by 1.2 px).
  - **Bands:** a stroked rim or edge is a band, one closed contour that fills the same under either fill rule.
  - **Not checked in game:** that these multi-layer glyphs draw as previewed (an offline raster of the stacked layers matched the author's wheel).
  - **Wheel:** the design's geometry at scale 1. Ring 62 to 148 px, centre disc 57 px, the hovered slot's gold fill (0.12) and tapered rim arc. The rim arcs are 151 to 194 px over 76°: three notches up, two right. The centre's two words, the ring's name and the hovered choice, are the only text (16 languages). Backing `#1a1510` at 0.8, gold `#ffd54a`.
  - **Motion:** a 120 ms fade-in; the rim arc fans out from the crossed slot's rim over 120 ms; the hover fill crossfades in 80 ms. On fire the slot or notch pulses 1.0, 1.12, 1.0 over 150 ms while the wheel fades out over 120 ms. A cancel fades over 120 ms.
  - **Target mark:** a pin with the choice's glyph above the pinged champion, following him; at the point for the other targets; small on the minimap. It lasts 4 s and fades over the last second. A champion out of sight has no pin.
  - **Removed:** dev5's ground ring and minimap ring.
  - **Kept:** the card (glyph and seconds left), 24 px above the minimap's left edge, until stage 2's stack.
  - **Research only, as before:** the dev8 and dev11 reply marks and choice marks.
- **Hooks** (no new site):
  - **Public builds:** they now install the signal family's step hook and the two heard sites. The late, gate and scene-site hooks stay research-only. The scene opener is called without being hooked, once its prologue matches. The setter's prologue is checked as before.
  - **Kept rules:** every write is inside `catch_unwind`. No game pointer is kept. Writes happen only on the viewed match's worker and only on the controlled athlete's teammates. The scene call happens only in kind 0. Inbox writes need len < cap.
  - **Verifier:** seven new 0.7.0 anchors: the reason tick's two stores, the Mia and stop cases, and the objective formatter's fields and names (`verify_ping_orders`). The 0.6.5 placeholders now include `SIGNAL_OBJECTIVE_REASON_TICK`, `SIGNAL_STOP` and `SIGNAL_MIA`.
- **Files:**
  - **New:** `native_adapter/ping_orders.rs`, `native_adapter/windows/ping_step.rs`, `pings/gesture.rs`, `pings/intent.rs`, `pings/places.rs`, `pings/mark.rs`, `pings/wheel_draw.rs`, `tools/generate_ping_assets.py`, `tools/records/ping-assets.json`.
  - **Rewritten:** `pings.rs` and `pings/wheel.rs`.
  - **Gated for both builds:** `signal_hook.rs`, `signal_fate.rs` (windows), `signal_send.rs`, `signal_call.rs`, `native_adapter/mod.rs` and `windows/mod.rs`.
  - **The objective type moved:** from `team_order.rs` to `ping_orders.rs`, and the setter call from `windows/team_order.rs` to `ping_step.rs`.
  - **Also changed:** `simulation.rs` (the roster and the pits in both builds), `battle_help.rs` (the watch only), `platform_input.rs` (F10), `sprite_picking.rs` (`sheet_name`), the 16 language files (12 words; the four old call names removed), both `signals.rs`, `tools/native_profiles/0.7.0.json`, `tools/verify_native_profile.py`, `tools/verify_build.py` (192 asset files) and `tools/records/ui-graphics.json`.
- **Not verified:** anything in game. This includes:
  - the wheel's look and motion, the gesture at match speed, and the delay and pause values;
  - the lane band and the pit names;
  - take versus contest, Morgard, Hunt (F10);
  - the stop and Mia calls (expected to do nothing; see above);
  - the public build's hooks.
- **Checked for this build:** `cargo fmt`; `cargo clippy --release --all-targets -- -D warnings` with and without `--features research`; `cargo test --release` 692 passed (dev12: 653), 743 with `--features research` (dev12: 729); 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors, placeholders only) and 0.7.0 (173 anchors, 7 new), and refuses a wrong reason-tick offset (0x1c0), Mia tag 2 and stop tag 11; the public DLL holds none of the research text (`OBJ verdict`, `OBJ press`, `OBJ retry`, `OBJ lanes`, `VIS request`, `STRATEGY`, `unseen_for`, `gank set`, `not heard`, `SIGNAL player`, `SIGNAL heard` absent; `PING set`, `PING pit learned` and the wheel assets present); `tools/build_dll.py` and `--research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev13 --probe-tests 692` / `743` passed for 0.6.5 and 0.7.0 (707 anchors, 192 asset files); `package_mod.py --version 1.7.1-dev13`: `LTDirectControl-1.7.1-dev13.zip` (194 files, SHA-256 `906d9b84…2c3f`; DLL `63b0e521…6a45`) and `LTDirectControl-1.7.1-dev13-research.zip` (194 files, SHA-256 `a42afaa9…c973`; DLL `29c88230…5b2c`). Not installed: the author installs.

## 1.7.1-dev12 (development build, research feature only) — 2026-11-03

Not seen in game. Development build, never for the Workshop. Built only as the research variant (`LTDirectControl-1.7.1-dev12-research.zip`). After the two dev11 matches (`docs/investigation-team-ai.md` section 12): objective orders become a standing request, two read-only fields join every OBJ line, F9 plants Morgard, F6 picks an enemy the team cannot see, and a small dev panel says in words what each key led to. No new hook site: everything rides on the dev10 step hook.

- **Standing request** (F1 to F4 and F9; `native_adapter/team_order.rs`, `Standing`): after the first set, at each later step of that teammate for 10 s of match time (600 ticks): ours counts as held; none (0xff) is set again through the setter (`OBJ retry n= team= index= request= reason= t=`, no `OBJ before` line); any other objective, the AI's own, ends it with no retry. One `OBJ verdict request= to=team,index asked= result= held= retries= replaced_by= plan= lane= reason= ended= t=` line per request:
  - `result=accepted`: ours at 20 decisions in a row (`ended=held`) or ours at the window's end (`ended=window`);
  - `result=refused`: never held more than one decision in a row;
  - `result=replaced` (not in the brief, which leaves this case out): held more than one decision in a row, then cleared or replaced;
  - `ended=ai_choice` when the AI's own objective ended it, `ended=new_press` when a newer press on the same teammate did; a teammate who stops stepping (dead) gets his verdict at another teammate's step a second after the window.
  The 20 `OBJ step` lines are kept as they were.
- **Two read-only fields** on every line that shows a brain (`OBJ watch`, `OBJ step`, `OBJ before`, `OBJ verdict`), read from the code on 2026-11-03 and anchored (`SIGNAL_REASON_*`, `SIGNAL_PLAN_*`, `SIGNAL_LANE_*`):
  - `reason=`: the objective handlers' drop reason byte at `team_plan + 0xc8` (brain + 0x1b0): 0x20 the mode gate, 0x21 the Serpen respawn rule; 0x23 and 0x24 are also stored, in the Morgard handler (meaning not read).
  - `lane=`: the big plan's lane, from its Debug formatter `0xe766c0`: PassiveLine at brain + 0x17c4, SinglePlanLine + 0x16c1, LineGanker + 0x16db, AttackNexus + 0x16b0 (after its `team` word); `-` for a plan without one.
- **Lane numbers:** read from the code, the game's own name tables for the lane field give 0 Top, 1 Mid, 2 Bottom, as the dev10 guess had it (the guess is marked superseded). Once a match, `OBJ lanes slot0= slot2= slot3= slot4= table=agrees|differs`: each laner's lane while his plan is PassiveLine, said when all four are known or a minute after the first watch (not at the very first watch: a laner may not be on PassiveLine yet).
- **F9:** objective Morgard, Setup, with battle (0x010100), like F1. F7 and F8 stay the strategy keys.
- **F6** picks an enemy the team cannot see: among the living enemies not visible to the player at the press, the one with the oldest team sighting (per enemy the freshest last-seen tick over the teammates' vision tables, the controlled athlete's own left out; ties to the lowest index). With every living enemy visible: the oldest of them (`by=oldest_visible`); with no table yet: index 1 (`by=fallback`). `VIS request enemy= unseen_for=<ticks> by= visible=[…]`. The writes are dev10's. `VIS next` now follows each teammate's next 5 steps (dev10: 1). The hook also reads the other team's five units' health for this (read only).
- **Dev feedback panel** (`pings/dev_panel.rs`, research builds only): top left below the target stats panel's place, the last six events with their age in seconds, in plain words: objective orders (`F1 Serpen -> #3: pending`, then `accepted, held 20`, `refused, AI chose none (jungle camps), reason 00` or `replaced after held 7, …`), HideLine (`not heard yet`, `heard`, `heard, gank set`, `heard, no gank (had Nexus top)`, `not heard`), the fight call (`ping enemy 25: #0 joined, #1 busy, #3 too far`), the strategy keys (`F7 focused=All: ok` or `rejected`) and vision (`F6 enemy 1 unseen 1200: written to 4 brains`). Drawn through the ping window's draw list while the player controls the athlete; the events live in the lever tests' state (`team_order::Feed`), cleared with each match. Words are allowed in this dev tool only. ASCII `->` and `...` instead of the brief's arrow and ellipsis (the HUD font's coverage not checked).
- **Files:** `native_adapter/team_order.rs` (the standing request, the verdicts, the lanes line, the unseen pick, the panel's events; 18 tests, 9 new), `native_adapter/windows/team_order.rs` (the reads, the retry), `native_adapter/windows/signal_fate.rs` (heard), `native_adapter/signal_call.rs` (the fight call's events), `strategy_probe.rs` (the strategy events), `pings/wheel.rs` (F9, the visible enemies, the panel), `pings/dev_panel.rs` (new, 1 test), `platform_input.rs` (F9), `native_adapter/v070/signals.rs` and `v065/signals.rs` (`SIGNAL_OBJECTIVE_REASON`, `SIGNAL_PLAN_LANES`), `tools/native_profiles/0.7.0.json` (12 anchors), `tools/verify_native_profile.py` (`verify_plan_lanes`).
- **Not verified:** anything in game. Whether a retry is ever kept; what the reason bytes mean past 0x20 and 0x21; whether the panel's text fits and reads well over the field.
- **Checked for this build:** `cargo fmt`; `cargo clippy --release --all-targets -- -D warnings` with and without `--features research`; `cargo test --release` 653 passed (unchanged), 729 with `--features research` (719 plus 9 lever tests and 1 panel test; the lever tests rewritten for the new press and step facts); 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors, placeholders only) and 0.7.0 (166 anchors), and refuses 3 deliberately wrong 0.7.0 lane values (LineGanker 0x16dc, PassiveLine 0x17c5, tag 14 for AttackNexus) and a wrong reason offset (0x1b1); the public DLL built from this source holds none of the new text (`OBJ verdict`, `OBJ retry`, `OBJ lanes`, `unseen_for`, `AI chose`, `written to`, `gank set`, `ping enemy`, `not heard`, `Morgard` absent); `tools/build_dll.py --research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev12 --probe-tests 729` passed for 0.6.5 and 0.7.0 (700 anchors); `package_mod.py --version 1.7.1-dev12`: `LTDirectControl-1.7.1-dev12-research.zip` (126 files, SHA-256 `449132d1…5739`; DLL `42e59297…ca40`). Not installed: the author installs.

## 1.7.1-dev11 (development build, research feature only) — 2026-11-03

Not seen in game. Development build, never for the Workshop. Built only as the research variant (`LTDirectControl-1.7.1-dev11-research.zip`). The dev10 lever keys made usable at match speed: the author found dev10's two-key chords, held while keeping the cursor on a moving champion, not humanly possible, and nothing on screen showed which teammate a press chose. No new probe logic otherwise: the worker's tests, lines and checks are dev10's.

- **Keys** (research builds only, plain function keys with no modifier, not rebindable, only on the battlefield; F1 to F8 are polled only in research builds; superseded: dev10's Home+1 to Home+6 and dev9's Home+PageUp and Home+PageDown):

  | Key | What |
  | --- | --- |
  | F1 | Objective Serpen, Setup, with battle (0x10101) |
  | F2 | Objective Gank(lane), lanes 0, 1, 2 in turn per press |
  | F3 | Objective Nexus(lane), the same |
  | F4 | Objective DefenseLine(lane), the same |
  | F5 | One HideLine, on the teammate's own lane |
  | F6 | The enemy believed at the point under the cursor |
  | F7 | Strategy `focused` (Top, Bottom, All; dev9) |
  | F8 | Strategy `object_buildup` (Gather, Flexible; dev9) |

  The mod's own bindings use Ctrl+1 to 5, Alt+Q/W/R, F11 and F12; F1 to F10 are free.
- **Which champion** (`pings/wheel.rs`, `pings/subject.rs`): the one nearest to the cursor on screen, at any distance, with no radius and no need to be under the sprite, among those whose feet are in the field view:
  - F1 to F4: a teammate whose brain the probe knows, never the controlled athlete;
  - F5: the same, never the jungler (index 1);
  - F6: an enemy champion; the point stays the cursor's world point.
  - With no eligible champion on screen: the living teammate with the lowest index (F1 to F5) or enemy index 1 (F6), as in dev10. The worker checks the choice again from the brains' owners.
- **Shown on screen** (`pings/battle_help.rs`, `order_shapes`): for 3 s of match time after a press (the reply marks' clock), at the top right of the chosen teammate's champion, the reply marks' disc with a filled triangle pointing down in the HUD's accent colour (0xaea400, the primary buttons'). For F6 the same mark on the enemy whose entry was written, when he is on screen, and a ring at the believed point. Glyphs only; the shapes are placeholders. The worker records the choice when it makes it (`team_order::Mark`) in the state the reply marks use; cleared with each match.
- **Lines:** `OBJ press key=F1 … nearest=<entity>|none gap=<n>px|- request=`; the worker's `OBJ request … by=nearest gap=<n>px` (dev10 `by=hover`) or `by=fallback`; `VIS request enemy=k by=nearest gap=` or `by=fallback`; `STRATEGY request field= (F7)`.
- **Files:** `pings/wheel.rs` (the keys, the nearest pick), `pings/subject.rs` (`nearest`, 1 test), `pings/battle_help.rs` (the mark and the ring), `native_adapter/team_order.rs` (the gap, the marks; 1 test added, 5 changed), `native_adapter/signal_call.rs` (`controlled_brain`), `platform_input.rs`, `client.rs`, `strategy_probe.rs` (the keys).
- **Not verified:** anything in game. Whether the marks are seen (the dev8 and dev9 reply marks use the same clock and drawing), and whether the game itself answers F1 to F8 while the player is not controlling.
- **Checked for this build:** `cargo fmt`; `cargo clippy --all-targets -- -D warnings` with and without `--features research`; `cargo test` 653 passed (652 plus the screen-gap test, which runs in both variants), 719 with `--features research` (717 plus the gap and marks tests; 5 lever tests changed for the gap and `by=nearest`); 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors) and 0.7.0 (154 anchors); the public DLL built from this source holds none of the new text (`OBJ press key=F`, `nearest=`, `OBJ request`, `VIS request` absent; the research DLL holds them); `tools/build_dll.py --research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev11 --probe-tests 719` passed for 0.6.5 and 0.7.0 (688 anchors); package `LTDirectControl-1.7.1-dev11-research.zip` (126 files, SHA-256 `a3434478…b39e`; DLL `1ad6c20b…3ec9`). Not installed: the author installs.

## 1.7.1-dev10 (development build, research feature only) — 2026-11-03

Not seen in game. Development build, never for the Workshop. Built only as the research variant (`LTDirectControl-1.7.1-dev10-research.zip`). Three tests of the per-player levers the team-AI read found (`docs/investigation-team-ai.md`, sections 4 and 6), plus a baseline of what the AI picks on its own. The dev8 probe lines and dev9's strategy keys are unchanged. No new hook site: everything rides on the step hook and the inbox write. The public build of the same source carries none of it.

- **Keys** (research builds only, fixed chords, not rebindable, only on the battlefield; 1 to 6 are polled only in research builds). Each press is one request; the champion under the cursor is chosen by the ping tap's own rule (the sprite under the cursor, else a near one):
  - **Home+1:** objective Serpen, phase Setup, with battle (0x10101).
  - **Home+2 / Home+3 / Home+4:** objective Gank / Nexus / DefenseLine on a lane. The cursor's lane is not used: nothing in the mod resolves a lane from the cursor yet (the wheel does not), so each key cycles lanes 0, 1, 2 per press and the line names the lane used.
  - **Home+5:** one HideLine (inbox tag 6) to a teammate, on his own lane.
  - **Home+6:** the enemy believed at the point under the cursor, for every teammate.
- **Lane numbers** are the game's: lane 0 is roster slot 0's lane, lane 1 slot 2's, lane 2 slots 3 and 4's (the game's table 0x202010000, read from the code). The names logged with them, 0 Top, 1 Mid, 2 Bottom, are a guess (slot 0 is the top laner, slots 3 and 4 the bottom duo, and the team-AI document reads 0x203 as DefenseLine(Bottom)); the `pos=` on the lines can confirm them.
- **Which teammate** (Home+1 to Home+5): the hovered friendly champion when he is a teammate (not the controlled athlete), else the living teammate with the lowest index; never the jungler for a HideLine (the game's path skips slot 1). The worker chooses and checks this again, from the brains' own owners.
- **Test 1, the objective** (`native_adapter/team_order.rs`, `windows/team_order.rs`):
  - At the chosen teammate's next step (the step hook runs before that step's handler and cascade), the game's own setter 0xec3820 is called with his team plan (brain + 0xe8), the objective and a zeroed Location the mod owns.
  - When the team plan's trace flag (brain + 0x830) is set, the setter would allocate: it is not called, and the line says `skipped: tracing on`.
  - With tracing off the setter clears its side fields by the new tag and writes the 3 bytes; it calls nothing (read from the code, 0xec3a28..0xec3a7f), so no game panic can unwind into the mod.
  - The setter's first 30 bytes are checked once at install (`OBJ setter ready` or `differs`).
- **Test 2, the HideLine:** one entry in the teammate's inbox, written as the BattleHelp call is (room in the vector, nothing allocated), with the mod's mark and the lane at +0x11, target 0. Read from the code (0xe6782d..0xe67877): after its lane rules the receive handler sets Gank(lane) when the receiver has no objective, is in slot 0, 2, 3 or 4 and the lane is that slot's own; no environment kind check on that path. Every branch of the case reaches that test except the game's own panic stub (0xe678af). The delivery lines (`SIGNAL late/gate/heard`) now say `ours=yes` for the mod's HideLine too; the scene call stays BattleHelp's.
- **Test 3, a believed enemy position:** in each teammate's brain (not the athlete's), during that teammate's own step, enemy k's entry in the vision table: position = the point, last seen = last checked = now, and the distance from that teammate's unit. Never the Mia stamps (+0xc40) or the hp ratios (+0xce0); a test checks every k's writes stay clear of both. k is the hovered enemy champion's index (his brain's +0x8b0, which the game's vision update indexes by), else 1.
  - **Correction to the brief and the team-AI document (read from the code, 0xecc02b..0xecc036):** `last_visible_distance` is the squared distance, dx² + dy² over unsigned differences in milli-units, with no square root; 10¹² = (1000 units)² is the game's value when the unit is missing. The probe writes it squared, as the game does (`dist2=` on the lines), not the isqrt the brief asked for.
- **Lines** (all past the probe's line budget):
  - Client: `OBJ press key=Home+n objective=|kind= hover= request=`.
  - Worker, the choice: `OBJ request objective=<u24>[name] to=team,index entity= by=hover|fallback hover= request=` (`kind=HideLine` or `kind=vision` for the others), `VIS request enemy=k by= pos= request=`, `OBJ waiting` (the athlete's brain not known yet), `OBJ expired` (not acted on within 2 s).
  - Test 1: `OBJ before team= index= objective= plan= arg= mode= pos=`, then `OBJ set team= index= objective= ok read=` (the 3 bytes read back) or `OBJ set team= index= skipped: tracing on`; then at that teammate's next 20 steps `OBJ step n= after=set t= team= index= objective= plan= arg= mode= pos=` (read at the step hook, so each shows the previous step's cascade result).
  - Test 2: `OBJ hide sent team= to= lane= from= due= mark= len= objective= plan= mode= pos=` (Gank is set only when that objective is none), or `OBJ hide dropped: queue full` (tried again at its next step); then 20 `OBJ step … after=hide` lines.
  - Test 3: `VIS before` and `VIS after t= team= index= enemy=k pos= seen= checked= dist2=`, then `VIS next` at that teammate's next step: the after line only echoes the write; the next one shows whether the game kept it (it rewrites a seen enemy's entry at every decision).
  - Baseline all match: `OBJ watch t= team= index= objective= plan= arg= mode= pos=` once a second for each brain of the controlled athlete's team, the athlete's own included (read only).
  - `mode=` is the receiver mode byte: the objective handlers' mode gate (mask 0x1a1) clears an objective, so it tells "cleared by the gate" from "cleared by the cascade". Plans are named from the big plan table.
- **Version table** (`v070/signals.rs`): `SIGNAL_TEAM_PLAN` 0xe8, `SIGNAL_OBJECTIVE` 0xe45, `SIGNAL_TRACE_ON` 0x830, `SIGNAL_OBJECTIVE_SETTER` 0xec3820 with its first 30 bytes, `SIGNAL_BIG_PLAN` 0x16a0 and `_ARG` 0x16a8 (the dev8 "scene kind" fields under their right name; the dev8 lines keep the old one), `SIGNAL_VISION_POS` 0xbf0, `_SEEN` 0xc68, `_CHECKED` 0xc90, `_DISTANCE` 0xcb8, `SIGNAL_HIDE_LINE` 6. 0.6.5 keeps placeholders, the family off.
- **Verifier:** 9 new 0.7.0 anchors (the setter's opening, trace test and tracing-off stores; the handler's tag dispatch and jump table, the HideLine case and its Gank path to the setter; the vision update's position base and its tick, squared-distance and position stores) and the checks tying each constant to them. 18 planted mistakes (13 in the 0.7.0 table, 5 in the 0.6.5 placeholders) are all refused.
- **Files:** `native_adapter/team_order.rs` (new, the pure half, 8 tests), `native_adapter/windows/team_order.rs` (new), `signal_hook.rs` (calls it; the setter check at install), `signal_send.rs` (the queue write shared), `signal_fate.rs` (`ours=` for the HideLine), `signal_call.rs` (the entry's lane byte; the tests' state, cleared with each match; 1 test extended), `pings/wheel.rs`, `client.rs`, `platform_input.rs` (the keys).
- **Not verified:** anything in game. Whether a planted objective survives the next cascade, whether the HideLine reaches its receiver (gate mode) and gives Gank, whether the AI acts on a believed position, and the lane names.
- **Checked for this build:** `cargo fmt`; `cargo clippy --all-targets -- -D warnings` with and without `--features research`; `cargo test` 652 passed (unchanged), 717 with `--features research` (709 plus 8 lever tests; 1 test extended for the entry's lane byte); 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors, placeholders only) and 0.7.0 (154 anchors), and refuses each of 13 deliberately wrong 0.7.0 values (every new offset, the setter, a prologue byte, HideLine tags 7 and 4) and 5 wrong 0.6.5 placeholders; the public DLL built from this source holds none of the new text (`OBJ request`, `OBJ watch`, `OBJ press`, `OBJ setter`, `VIS before` absent); `tools/build_dll.py --research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev10 --probe-tests 717` passed for 0.6.5 and 0.7.0 (688 anchors); package `LTDirectControl-1.7.1-dev10-research.zip` (126 files, SHA-256 `227afe61…eac3`; DLL `60cad77d…bde6`). Not installed: the author installs.

## 1.7.1-dev9 (development build, research feature only) — 2026-11-03

Not seen in game. Development build, never for the Workshop. Built only as the research variant (`LTDirectControl-1.7.1-dev9-research.zip`). Two probes: the official interface's live team strategy as a lever for pings, and why the dev8 reply marks were never seen. The dev8 probe lines are unchanged. The public build of the same source carries none of it.

- **What the dev8 log showed** (seen in game by the author, game 0.7.0, build 1.7.1-dev8-research, 2026-11-03): 28 answers, 3 joins, each with an Ok on list C; the scene kind switched to 1 with the target as its argument. No reply mark was seen on screen.
- **Strategy probe** (`strategy_probe.rs`, new):
  - **What it uses.** The SDK's `strategy_get_json` and `strategy_set_json`. These run from the match hook (`test_cheats::CooldownHook::on_match_tick`), on the viewed match only. That hook is where the SDK honours changes: the SDK's own note says changes made through the sim inside the AI step (where the mod's simulation code runs) are inert. No game address, no native read.
  - **At the viewed match's first tick:**
    - `STRATEGY doc team= path="" ok= json=` for both teams. When the empty path fails, `STRATEGY get` for `"/"` and `"."`.
    - For the controlled athlete's team, the field paths `focused`, `/focused`, `$.focused` and `team.focused`, until one works.
    - `STRATEGY start team= doc_path= field_form=` names what was found.
  - **Debug keys:** Home+PageUp cycles that team's `focused` (Top, Bottom, All); Home+PageDown its `object_buildup` (Gather, Flexible; Split left out, it carries a position).
    - Fixed chords, not rebindable. PageUp and PageDown are polled only in research builds.
    - Only on the battlefield. A request left before a match is dropped when the match starts.
    - The client leaves the request; the match hook performs it.
  - **Each press:**
    - `STRATEGY request field=`.
    - The attempts until one succeeds, each `STRATEGY set team= path= json= ok=`: the bare value (`"Top"`) at the field, then `{"Top":null}` at the field, then `{"focused":"Top"}` at the document. With no working field form, the bare value is tried at every form first.
    - Then the field and the whole document again.
  - **For 20 s after a set, once a second:** `STRATEGY check team= field= value= changed=yes|no`, and `STRATEGY watch t= player= pos=x,y` for each living teammate (`you` marks the controlled athlete).
  - **Once a second all match:** `STRATEGY game changed team= field= from= to=`, once per team and field, when a document differs from the last one seen (the match start's, or after a set by the mod).
- **Reply-mark diagnostics:**
  - **Worker:** after each `SIGNAL answer`, `PING reply stored entity= answer= tick=`, or `PING reply dropped: no entity for team= index=`.
  - **Client:** once per answer, the first frame the ping window considers it: `PING reply entity= answer= glyph=joined|busy|refused|none tick= now= played= in_window= show= units= unit=found|missing on_screen=yes|no|n/a at=x,y|-`.
    - `now` is the simulation's latest tick, which the marks are timed by.
    - `played` is the screen's own tick (`presentation_tick`), to see whether the two clocks differ.
  - **Item 8 of the brief (a render condition to fix):** none found in the code. The window draws when it shows and has shapes; the draw list is sent at z 1048/1049 like the ping's other drawn rings. Guess, not tested: the marks are timed by the simulation's tick while the screen plays its own; the new line shows both.
  - **Test setting:** the marks are larger (disc radius 18, glyph scaled with it; dev8 11) and last 3 s (180 ticks; dev8 1.5 s).
- **Files:**
  - `strategy_probe.rs`: new, 3 tests.
  - `test_cheats.rs`: the match hook calls the probe.
  - `platform_input.rs` and `client.rs`: the two chords.
  - `pings/battle_help.rs`: the reply line, the scaled marks; 1 test changed.
  - `pings/wheel.rs`: passes what the marks need.
  - `native_adapter/signal_call.rs` (answers return their line; `stored_replies`; `Reply::shown_at`; 1 test changed) and `windows/signal_fate.rs` (logs it).
- **Not verified:** anything in game. Whether the strategy calls work outside the AI step, which path and value forms work, whether the team AI follows a change, why the marks were not seen.
- **Checked for this build:** `cargo fmt`; `cargo clippy --offline --all-targets -- -D warnings` with and without `--features research`; `cargo test --offline --release` 652 passed (unchanged), 709 with `--features research` (706 plus 3 strategy tests; 2 tests changed); 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors) and 0.7.0 (145 anchors); the public DLL built from this source holds none of the new text (`STRATEGY`, `PING reply`, `Home+PageUp` absent; the research DLL holds them); `tools/build_dll.py --research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev9 --probe-tests 709` passed for 0.6.5 and 0.7.0 (679 anchors); package `LTDirectControl-1.7.1-dev9-research.zip` (126 files, SHA-256 `50c43e8c…a947`; DLL `557d9c94…7f46`). Not installed: the author installs.

## 1.7.1-dev8 (development build, research feature only) — 2026-11-03

Not seen in game. Development build, never for the Workshop. Built only as the research variant (`LTDirectControl-1.7.1-dev8-research.zip`). One probe build meant to answer, per tap and teammate: was the call the mod's, what did the game answer, did the scene change, was an Ok reply queued, and the teammate's hp, speed and position. It also shows the answer for a moment by each teammate. The public build of the same source carries none of it.

- **What the dev7 log showed** (seen in game by the author, game 0.7.0, build 1.7.1-dev7-research, 2026-11-03; `docs/investigation-ai-signals.md` section 11):
  - The mapping resolved in every match. 13 taps, 46 scene calls made by the mod, no crash.
  - The game's join rule judged every call.
  - Requests 4 and 9/10 also counted the controlled athlete's own AI sending the same BattleHelp.
- **The mod's mark** (`SIGNAL_MARK`, `0x4c545047`, the bytes "GPTL"):
  - Every entry the mod writes now carries it at +0x14 (the signal's word at +4).
  - A call is the mod's own (`ours=yes`) only with the mark. The same sender, kind and target without it is `ours=unmarked`: the athlete's own AI. Those lines are also written past the budget, so they can be counted.
  - The mod acts only on its own.
  - `PING sent` shows `mark=`.
  - **Read from the code (2026-11-03):** that word is free for BattleHelp. Only tag 1 (Mia) reads it, in three places: the receive handler (`0xe6750b`), the compatibility check (`0xfcd67e`, as an index into its second jump table) and the late drop's queue-B match (`0xe76149`).
    - For tag 4, the compatibility check returns true without reading the signal (jump table entry `0xfcd6af`).
    - The late drop compares the target and the word at +0x10 (`0xe76131`).
    - The handler's BattleHelp case reads only the target (`0xe67538`).
    - The gate reads the receiver's mode and passes the signal on to the handler and to a trace record.
    - A sweep of every instruction using the inbox's pointer or length found no other reader of its entries' fields. The rest are the broadcast that appends, its clone (`0xf33630`, a copy) and its drop (`0xf0f630`).
  - **Superseded 2026-11-03:** until dev7, nothing in the entry told the mod's call from the AI's own.
- **`SIGNAL scene_state team= to= request= before{…} after{…}`**, around each scene call the mod makes. Each side shows:
  - `kind=`: the scene kind at `brain + 0x16a0`;
  - `arg=`: its argument at `+0x16a8`;
  - `active=` (`+0x15f0`), `tick=` (`+0x1638`, the world tick), `pos=x,y` (`+0x1648`), `dist=` (`+0x1658`, milli-units) and `b88=` (the byte at `+0x1688`).
- **`SIGNAL answer team= to= request= answer= delta= replyC= hp=% speed= pos=`**:
  - **Answer** (`join`, `in_battle`, `too_far`, `ignored`, `no_help`, `low_hp`, `bail` or `none`): from the eight call-join counters read before and after the call. When several rose, the one that ended the call wins, in this order: join, bail, low_hp, too_far, no_help, ignored, in_battle. `none` means only `recv` rose, or nothing did.
  - **`delta=`**: every counter that changed, for example `recv+1,join+1`.
  - **`replyC=`**: the tag of the entry the call pushed onto list C (`brain + 0x1eb8`, 24-byte entries; the pointer is read after the call, since the push may grow the list). `none` if the list did not grow, `?` if it does not read as a list.
  - **`hp=`, `speed=`, `pos=`**: the teammate's unit before the call, `[[pair] + 0x238 + (side * 5 + index) * 8]`, the receive handler's own read. hp percent = `[+0x668] * 100 / [+0x620]`; speed is `[+0x638]`; pos is `[+0x658]` and `[+0x660]`.
  - The distance to the target is not logged. The target's unit comes from the AI environment's slot 0x208, a call the probe does not make. `dist=` in the scene block and the `PING watch` lines give it instead.
  - The join lines after a mod scene are kept.
- **Reply marks** (a first version of the design's "replies shown"):
  - For 1.5 s of match time after a teammate's answer, a small mark appears at the top right of his champion on the field: a green check for `join`, a yellow clock for `in_battle`, a red cross for the five refusals, nothing for `none`. Each sits on a dark disc.
  - Glyphs only, with placeholder shapes. The ping window draws them with its draw list.
  - The answer goes from the worker to the client through `signal_call`'s shared state (`answered`, `replies`), the same short lock the request and the watch use.
  - **Deviation from the brief:** the brief named the teammates' HUD markers in `team_draw.rs`/`team_status.rs`. Those draw only the Tab panel, which shows while Tab is held, and the mod draws no other teammate marker on the field.
- **Read from the code, the scene opener `0xe662f0` and the set-scene function `0xe72740` (2026-11-03):**
  - The scene kind at `+0x16a0` is a u64: read as a qword at `0xe664c8`, written at `0xe66566`, compared as a dword once at `0xe7292d`.
  - In the in-battle branch (kind at most 1), when the kind is 0 and the target is within 150 units (squared distance `0x53d1ac100`), the opener itself sets the kind to 1 and its argument to the target unit's `[+0x5b8]` (`0xe66566`).
  - A join pushes `{tag 9, 0}` onto list C (`0xe66f59`), raises `join` (`0xe66f70`), then calls `0xe72740(brain, 4, …)`. That function returns without writing when the distance is under 250 units (`0x3d091`, `0xe727f0`). Otherwise it clears the old scene (`0xe73a10`) and writes the block (`0xe729ee..0xe72a83`), the tick from the simulation's world tick.
  - hp percent uses an unsigned division (`0xe6691d`).
- **Files:**
  - Pure modules:
    - `native_adapter/signal_fate.rs`: the signal's word, `Mine`, `SceneState`, `Answer`, `answer`, the scene-state and answer lines; 2 tests, 3 changed.
    - `native_adapter/signal_call.rs`: the mark in the entry, the answers for the HUD; 1 test, 1 changed.
    - `pings/battle_help.rs`: the marks; 2 tests.
  - Hooks:
    - `windows/signal_fate.rs`: whose a call is, from the mark; the heard hook passes it to the scene hook nested in it, since the scene opener is not passed the signal; the reads around the mod's scene call.
    - `windows/signal_send.rs`: the mark.
  - `pings/wheel.rs`: the window draws shapes another part passes it.
  - Tables: `v070/signals.rs` and `v065/signals.rs`. List C keeps its existing name, `SIGNAL_TEAM_LIST_C`.
  - Profile and verifier:
    - `tools/native_profiles/0.7.0.json`: 7 new anchors (the scene kind read, the in-battle write, a unit's position and hp reads, the Ok reply and join, the set-scene block, the BattleHelp case's target read).
    - `tools/verify_native_profile.py`: each scene block field by its own store, the list C push, the unsigned division, and the mark (0 for 0.6.5).
- **Not verified:** anything in game. Whether the mark changes nothing for the game, what the scene state shows, whether a counted join writes the scene block, whether an Ok is queued every time, how the marks look.
- **Checked for this build:** `cargo fmt`; `cargo clippy --offline --all-targets -- -D warnings` with and without `--features research`; `cargo test --offline --release` 652 passed (unchanged), 706 with `--features research` (701 plus 2 answer and line tests, 1 reply-store test, 2 mark tests; 4 tests changed for the mark); 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors, placeholders only, mark 0) and 0.7.0 (145 anchors), and refuses each of 14 deliberately wrong values (every new offset, list C, a zero mark) and a nonzero 0.6.5 mark; the public DLL built from this source holds none of the probe's text (`SIGNAL `, `scene_state`, `SIGNAL answer`, `replyC=`, `ours=unmarked`, `PING acted`, `mark=` absent; the research DLL holds them); `tools/build_dll.py --research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev8 --probe-tests 706` passed for 0.6.5 and 0.7.0 (679 anchors); package `LTDirectControl-1.7.1-dev8-research.zip` (126 files, SHA-256 `dcecd066…5dc3`; DLL `138c4484…aa70`). Not installed: the author installs.

## 1.7.1-dev7 (development build, research feature only) — 2026-11-03

Not seen in game. Development build, never for the Workshop. Built only as the research variant (`LTDirectControl-1.7.1-dev7-research.zip`). This build:

- fixes the mapping that stopped the mod's call in dev6;
- corrects the scene reason labels;
- logs the AI environment's kind once a match;
- for the mod's own BattleHelp, opens the scene the game's receive handler skips in a viewed match.

This is the first build that calls a game function on a path the game does not take. The public build of the same source carries none of it.

- **What the dev6 log showed** (seen in game by the author, game 0.7.0, build 1.7.1-dev6-research, 2026-11-03):
  - The delivery hooks fired: 911 gate and 911 heard lines, 0 late, 0 scene, 293 join lines.
  - Every receiver showed delay 60, mode 0, `mask_ok=yes` and handler site 2.
  - The mod sent nothing: every request met `PING map unresolved: no field holds the roster's entities`.
- **The mapping bug, two causes** (found in the log and the code):
  - **Match 1:** the one-time scan resolved (`player+0x8:u64`, 10 brains). Then every roster change re-scored those old hits against the new roster. Champions leave the roster when they die and come back with new entity ids, so the old hits matched nothing ("0 candidate fields"), and the scan was never repeated.
  - **Match 2 and later:** each match runs on a new worker thread (`worker_os` 28208, 44960, 18996 in the log). The probe's memory is per thread, so on a fresh thread the tick never went back and no new match was seen. The shared state therefore kept match 1's ten "scanned" brains, and no brain was scanned again (not one `PING map brain` line).
- **Fix:**
  - `Memory::observe_match` treats a thread's first step as a new match.
  - The scan is gone (`Mapper`, `Field`, `scan`, 6 tests; `SIGNAL_PLAYER_SCAN`, `SIGNAL_BRAIN_SCAN`).
  - At every brain's step the probe reads the owner's entity id at `player + 0x8` (`SIGNAL_PLAYER_ENTITY`; seen in game in the dev6 log, the same at `+0x8c0`; not read from the code, so not anchored). It compares that id with the controlled athlete's entity in the roster (republished every second).
  - The brain showing his entity is his. Teammates are the other indexes of his side, as in dev5.
  - Once resolved, the mapping is kept for the match: his side and index cannot change within one.
- **Mapping log lines:**
  - `PING map owner team= index= entity=` when a brain's owner id changes. The next log shows whether `+0x8` follows a respawn.
  - `PING map resolved team= index= entity= by=player+0x8 sdk_side= lane_agrees=yes|no`: in the dev6 log every brain's index equals the SDK lane.
  - `PING map disagrees: …`, once per pair of ids, when his brain shows another id than the roster (expected for up to a second after a respawn) or another brain shows his id. Log only: the mapping is kept.
  - The brief's "pin brains by address" was declined: a hook keeps no pointer the game passed in (AGENTS.md). So `SIGNAL late` lines show `team=? to=?`, with `now=` the latest tick any step saw and `delay=?`. There were 0 late lines in dev6.
- **Superseded 2026-11-03:**
  - dev6 labelled scene reason 3 as Battle. The receive handler's codes are 1 Battle (`0xe673be`), 2 BattleHelp (`0xe67538`) and 3 BattleDive (`0xe674dc`).
  - dev6 read the call before each scene as a target lookup. It asks the AI environment its kind.
  - The tick stamped at `brain + 0xc40` is tag 1's (Mia), not BattleDive's.
  - The "24,000 lines per session" budget is per worker thread, so in practice per match.
- **Why no scene opened** (read from the code, 2026-11-03):
  - Before opening a scene, each of the three cases calls method slot 0x40 of the AI environment (a trait object at the start of the simulation: `call [[sim + 8] + 0x40]`), and opens nothing when it returns 0.
  - The environment's vtables (Rust layout: drop, size, align 8, 102 methods) are:
    - `0x40a2530` and `0x40c9f28`: size 0xf410, kind 0 (`0x16ab380`: `lea rdx,[rcx+0xf078]; xor eax,eax; ret`).
    - `0x40a33a0`: size 0xf058, kind 1 (`0x16ca720`: `… mov eax,1; ret`).
    - `0x40a3f98` and `0x40c9898`: size 0xf058, kind 2 (`0x16d9eb0`: `… mov eax,2; ret`).
    - `0x40398e0`: a wrapper of size 0xa8, whose slot 0x40 (`0x18ebb40`) forwards to the inner object's.
  - The scene opener `0xe662f0` itself asks no kind: its only indirect calls are slots 0x208 and 0x120.
  - The team handler `0xe381b0` calls it directly (`0xe3e241`) with its own brain, context, player state and pair. The reason there comes from a per-entry table, so whether kind 0 ever runs reason 2 is not known (a guess that it does).
- **`SIGNAL env vtable= kind=0|1|2|wrapper|unknown`** (plus `inner= inner_kind=` for the wrapper), once a match from the step hook (`[sim + 8]`, minus the executable base). The table is `SIGNAL_ENV_VTABLES` in `v070/signals.rs`, empty in `v065`.
- **The mod's own scene** (`windows/signal_fate.rs`): in the heard hook, after the receive handler returned, the mod calls the scene opener when all of these hold:
  - the call is the mod's own (sender = the controlled index, BattleHelp, the open request's target, a receiver on his side that the mod wrote it to);
  - this match's environment kind is 0 (in other kinds the handler already did it);
  - the delivery hooks are in;
  - this teammate was not acted on yet for this request (one scene per teammate and tap).

  How it calls: exactly as the handler does at `0xe67566..0xe67580`, with the handler's own brain, context, third argument, player state and pair, the sender as a dword, reason 2, the signal's target and the handler's last argument. Same thread, one level below the gate, with no lock or borrow held.

  What it logs: `SIGNAL scene … ours=yes by=mod` (the hooked scene site does not see this call) and `PING acted team= to= target= request=`.

  After it: that brain's next `SIGNAL join` line is always written, and forced past the budget for 10 s; the counters are the verdict.

  The dev5 write path is unchanged: the call still goes through the inbox, and the game's own late, gate and handler steps still run on it.

  **Risk:** `catch_unwind` does not stop a crash inside the game's function.
- **Files:**
  - Pure modules: `native_adapter/signal_call.rs` (`Owners`, `Mapping` {side, index, entity}, `act_on`, `Request::acted`; 2 tests replace 6), `native_adapter/signal_fate.rs` (reason labels, the environment kind and line; 1 test), `native_adapter/signal_probe.rs` (the first step of a thread is a new match, the environment, owed and forced join lines; 1 test; the per-brain delay for the late line is gone).
  - Hooks: `windows/signal_hook.rs` (owner read, environment line, join lines), `windows/signal_fate.rs` (late line, mod scene), `windows/signal_send.rs` (the mapping from `Owners`; the write is unchanged).
  - Tables: `v070/signals.rs` and `v065/signals.rs`.
  - Profile and verifier: `tools/native_profiles/0.7.0.json` (25 new anchors: each vtable's drop pointer, size and align, slot 0x40; the four kind methods; the three handler kind checks) and `tools/verify_native_profile.py`.
- **Not verified:** anything in game: that the mapping now resolves and holds, what kind a viewed match is, whether the mod's scene opens, what the join rules decide, whether a teammate moves.
- **Checked for this build:** `cargo fmt`; `cargo clippy --all-targets -- -D warnings` with and without `--features research`; `cargo test --offline --release` 652 passed (unchanged), 701 with `--features research` (703 minus the 6 scan tests, plus 2 owner and act tests, 1 environment test, 1 owed-join test); 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors, placeholders only) and 0.7.0 (138 anchors), and refuses a wrong kind, a wrapper listed as a kind and an unknown vtable; the public DLL built from this source holds none of the probe's text (`SIGNAL `, `SIGNAL env`, `SIGNAL scene`, `PING map owner`, `PING acted`, `by=mod` absent; the research DLL holds them); `tools/build_dll.py --research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev7 --probe-tests 701` passed for 0.6.5 and 0.7.0 (672 anchors); package `LTDirectControl-1.7.1-dev7-research.zip` (126 files, SHA-256 `570f6a26…8e6e`). Not installed: the author installs.

## 1.7.1-dev6 (development build, research feature only) — 2026-11-03

Not seen in game. Development build, never for the Workshop. Built only as the research variant (`LTDirectControl-1.7.1-dev6-research.zip`). A delivery probe: five more read-only hooks inside the game's delivery of AI calls show the fate of every call in a brain's inbox. A call can be dropped as late, stopped at the receive gate, heard by the receive handler, or open a scene. The dev5 send path (a tap on an enemy champion → one BattleHelp in each teammate's queue A) and the wheel are unchanged. The public build of the same source carries none of it.

- **Why:** in dev5 (seen in game by the author, game 0.7.0, build 1.7.1-dev5-research, one match, slot 2) the mapping resolved and six taps each wrote four entries. They were byte-identical to the game's own BattleHelp entries, never hit a full queue, and were removed between full steps like the game's own. There was no visible reaction. The dev5 log could not say whether they were heard, because the delivery `0xe75a60` is called from paths the broadcast hook never sees.
- **The delivery, read from the code** (2026-11-03, 0.7.0):
  - Callers: the delivery is called directly from `0xe381b0` (at `0xe3edb9`) and `0xe4dc10` (at `0xe4dfaa`), not from the step `0xf16d60`. The brief's `0xf177c0 → 0xe4dc10` link was not checked.
  - Draining: it takes the inbox entries whose due tick has passed (`now = [sim + 0x1b8]`), and stops at an entry whose sender is −1.
  - Late drop: an entry whose `due + delay < now` is late. Only the queue-B removal `0xe76090` runs (at `0xe75c32`), and the entry is never heard.
  - Gate: otherwise, after the compatibility check and the B removal, the gate `0xe65db0` runs (at `0xe75d27`). It returns at once for the receiver's own call (`sender == [player + 0x8b0]`). It then takes a rule from a jump table (`0x400fe70`, indexed by the sender's slot, no bounds check) and requires the receiver's mode, the byte `[[pair + 8] + 0x38]`. Then it runs the compatibility check again.
  - Handler and scene: the gate calls the receive handler `0xe67300` at `0xe65f25` (when `[[pair + 8] + 0x39]` is set, with a trace record) or at `0xe660ef`. For Battle (reason 3) and BattleHelp (reason 2), the handler looks the target up and, when it is found, calls the scene opener `0xe662f0` at `0xe67580`.
  - The pair: `[pair]` is the simulation and `[pair + 8]` the receiver state. The delay is `[[receiver + 8] + 0x12f8]`.
  - The signal passed on is a 24-byte copy of the entry from +0x10: tag @0, lane @1, a dword @4, target @8, a word @0x10.
- **The gate's rules per sender slot:**
  - Slot 0 accepts modes 0, 2, 7 and 8 (mask `0x185`).
  - Slot 1 accepts modes 0, 6, 8 and anything above 8 (code of its own at `0xe660b0`: it rejects modes 1 to 7 except 6). **This contradicts the brief's reading "0, 7 or 8"**; the verifier refuses that reading.
  - Slot 2 accepts 0, 4, 5, 7 and 8 (`0x1b1`).
  - Slots 3 and 4 accept 0, 1, 3, 5, 7 and 8 (`0x1ab`).
  - Every slot except slot 1 rejects modes above 8. The rules sit in `SIGNAL_GATE_RULES` in `v070/signals.rs`, checked by the verifier against the table and each case's code.
- **Superseded 2026-11-03:** dev4 named the step hook's last two arguments `simulation` and `vtable`. The step unpacks the pair before the call (`mov r15, [rsi]; mov rax, [rsi + 8]` at `0xf16dc4`): the fourth argument is the simulation itself, the fifth is the receiver state. **The brief read the fourth as the pair**; dereferencing it as one would have read the simulation's first word.
- **Superseded 2026-11-03:** the dev4/dev5 `delay?=` column read `player + 0x12f8`, the wrong structure (the log showed values such as 528 and 1198509630577458497). It is replaced in the per-step header by `mode=` (`[receiver + 0x38]`) and `delay=` (`[[receiver + 8] + 0x12f8]`), the two values the delivery uses, with null and alignment checks (`?` when not readable). `SIGNAL_PLAYER_DELAY` is gone; the new constants are `SIGNAL_RECEIVER_MODE`, `SIGNAL_RECEIVER_STATE` and `SIGNAL_STATE_DELAY`.
- **Hooks** (`windows/signal_fate.rs`):
  - Sites: the late drop `0xe75c32` (`e8 59 04 00 00` → `0xe76090`), the gate `0xe75d27` (`e8 84 00 ff ff` → `0xe65db0`), the handler's two calls `0xe65f25` (`e8 d6 13 00 00`) and `0xe660ef` (`e8 0c 12 00 00`) → `0xe67300`, and the scene opener `0xe67580` (`e8 6b ed ff ff` → `0xe662f0`).
  - Each replaces one call with a function of the same signature: four, nine, eight and nine arguments, the stack slots each target reads, checked in the code. Each calls the original first and returns its result unchanged. Then, inside `catch_unwind` and only on the viewed match's worker, it does bounded reads and writes one line.
  - Before anything is written, all five sites and the four targets' opening bytes (20, 19, 19 and 19 bytes) are checked. If one does not read back, the ones already in are put back, so the five are in or out together.
  - They install after the dev4/dev5 hook, and a differing site turns off only them (`SIGNAL delivery hooks installed sites=[…]` or `… unavailable: …`). The patch audit covers them.
  - Nested lines (gate → handler → scene) are written in that order when the outermost hook returns.
  - The step hook's patch code is now shared (`check_call`, `write_call`, `restore_call`); behaviour unchanged.
- **Log lines** (`native_adapter/signal_fate.rs`, 4 tests):
  - `SIGNAL late team= to= from= tag= name= target= due= now= delay= ours=`. Only the brain is passed here, so the receiver is named from a brain field the dev5 mapping found (the resolved field when it is in the brain, else the best candidate that is; `?` before that). `now` and `delay` are the tick and delay that brain's last step read.
  - `SIGNAL gate team= to= from= tag= name= target= now= mode= delay= mask_ok=yes|no|? ours=`. The receiver comes from the player state passed in, and now, mode and delay from the pair. **The gate is not passed the due tick, so this line has no `due=`** (a difference from the brief). `mask_ok=?` is shown for a sender slot above 4 or an unreadable mode. An own call (sender = receiver) also shows here; the gate drops it before the rule.
  - `SIGNAL heard team= to= from= tag= name= target= now= mode= site=1|2 ours=`.
  - `SIGNAL scene team= to= from= reason=2[BattleHelp]|3[Battle] target= now= ours=`.
  - `SIGNAL join team= index= recv= ignored= in_battle= no_help= too_far= low_hp= bail= join=`. The brain's call-join counters (the game's CallJoinStats: eight u64 at `brain + 0x12c0`, in the order its serialiser `0xf4d660` writes them), read at the brain's step and logged when they change.
  - Two caveats on these lines. The counters are incremented inside the scene opener, which has a second caller (`0xe3e241` in `0xe381b0`) that the `scene` hook does not see: so the join counters are the complete count and `scene` lines may be fewer. `ours=yes` marks a call from the controlled athlete's index, BattleHelp, with the open request's target (from its first write until the watch ends), to his side. The game's own AI may send the same call and the entry cannot tell them apart.
- **Budget:** the new lines share the probe's line budget (8,000 per match, 24,000 per session); lines marked `ours=yes` are always written.
- **Files:**
  - New: `native_adapter/signal_fate.rs` and `windows/signal_fate.rs`.
  - Changed: `windows/signal_hook.rs` (shared patch code, the receiver read, join lines, the fifth argument renamed); `native_adapter/signal_probe.rs` (the dump header, each brain's last delay and join counters, 1 test); `native_adapter/signal_call.rs` (the open call, the brain field, 1 test); `windows/signal_send.rs` (sets the brain field with the mapping; the send path itself is unchanged); `native_adapter/mod.rs` and `windows/mod.rs`.
  - Tables: `v070/signals.rs` (the five sites, prologues, rules, receiver and join fields, superseded notes); `v065/signals.rs` (placeholders; the family stays off there).
  - Profile and verifier: `tools/native_profiles/0.7.0.json` (21 new `SIGNAL_*` anchors and a review note); `tools/verify_native_profile.py` (calls, the receiver fields, the join counters, the gate's table and rules, and 0.6.5's placeholders).
- **Not verified:** anything in game: that the hooks fire, what the lines show, whether the mod's entries are heard.
- **Checked for this build:** `cargo fmt`; `cargo clippy --all-targets -- -D warnings` with and without `--features research`; `cargo test --offline --release` 652 passed (unchanged), 703 with `--features research` (697 plus 4 delivery-line tests, 1 brain-memory test, 1 brain-field test); 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors, placeholders only) and 0.7.0 (113 anchors), and refuses each of seven deliberately wrong table values, among them the brief's slot-1 rule; the public DLL built from this source (`tools/build_dll.py`) holds none of the probe's text (`SIGNAL `, `step_player=`, `SIGNAL late|gate|heard|scene|join`, `delivery hooks`, `mask_ok=`, `PING sent`, `PING map` absent; the research DLL holds the new ones); `tools/build_dll.py --research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev6 --probe-tests 703` passed for 0.6.5 and 0.7.0 (647 anchors); package `LTDirectControl-1.7.1-dev6-research.zip` (126 files, SHA-256 `1932b83f…9815`). Not installed: the author installs.

## 1.7.1-dev5 (development build, research feature only) — 2026-11-03

Not seen in game. Development build, never for the Workshop. Built only as the research variant (`LTDirectControl-1.7.1-dev5-research.zip`). Step 1 of the ping system's second stage: the mod sends **one** call into the game AI's own signal queues: a tap of the ping key on an enemy champion is a BattleHelp from the controlled athlete. It is the first build that writes into the game's memory outside the stat rules: one 40-byte entry per teammate, appended within a queue's existing capacity. The dev4 probe stays on, so the log shows the entry, its drain and the reaction. The public build of the same source carries none of the probe, the scan or the write; there, a tap does nothing.

- **What the dev4 log settled** (seen in game by the author, game 0.7.0, build 1.7.1-dev4-research, ten matches, 2026-11-03): a queue-A entry is `u64 due tick @0`, `u32 index @8` (0..4), `u32 15 @0xc` (the same in every entry: the game's delay; the probe's `tagC` is this constant), `u8 tag @0x10` (as the queue-B comparison said), `u8 lane @0x11`, `u32 @0x14` (Mia: the missing enemy's index 0..4; 0 otherwise), `u64 target entity id @0x18` (Battle, BattleHelp, BattleDive: real unit ids), `u64 @0x20` (0 for the Battle kinds). `due − tick` is 15..20 in almost every entry (59.7 ticks a second). Capacity 4, length mostly 0..2. Only Mia, Battle, BattleHelp, BattleStop, HideLine, HideLineToo, Cancel and BattleDive travelled (the receive handler `0xe67300` dispatches tags 1..11). Queue B carried 6 entries in ten matches, list C none. `delay?` is 0, 1 or 2 per athlete.
- **Three dev4 readings superseded 2026-11-03:**
  - **"Team planner":** the hook's first argument is one player's AI brain. Evidence: in the author's log, every match logged one player per "team" and then a "third team planner" at t=5..6, so each player has his own. Read from the code: the step's only caller `0xf19810` stores that player's next decision tick in it (`+0x47c0`), worked out from the athlete's stats. So queue A is per player.
  - **"`from` = the receiving teammate":** read from the code, the delivery `0xe75a60` checks every entry against the owner's own state (`[[arg5+8]+8]`, the drop-unheard delay and the compatibility check), whatever the entry's index. Queue A is therefore the owner's inbox and the index at +8 is the **sender**, as the author's log suggested. Still a reading of the code; this build's watch is the test.
  - **"Identity at `+0x820`":** it is the player's side (0 or 1). Evidence: only 0 and 1 in the log, one value per team. The table constant is renamed `SIGNAL_PLAYER_SIDE`.
- **Changed from the brief, because of the above:** one call is up to **four** entries, one in each teammate's own queue A, with the controlled athlete's index as the sender. The brief had one entry in a team queue.
- **The target is the enemy** (read from the dev4 log, a guess): the same sender called Battle and then BattleHelp on the same id; many targets are large ids (towers, monsters); in the first match the controlled actor was 20 and the champion targets were 18 to 27, the mod's own id space (the SDK's unit id).
- **The input** (`pings/wheel.rs`, research builds): pressing G and releasing it over the wheel's centre is a tap. With an enemy champion under the press (the same snap rule as allies: the sprite under the cursor, else the nearest within 50 px, else a lone one within 120 px), it leaves one pending request: kind BattleHelp, that champion's entity id. A new tap replaces it. Log: `PING request kind=BattleHelp target=<id> tick=<last tick the probe saw> request=<n>`. With no enemy, the tap does nothing (it cancels, as before). The four slots send nothing in this build: `PING slot <kind> not sent in this build`. Stage 1's record and live-ping window stay, behind `pings::SLOTS_SEND = false`, for the next stage.
- **The two mappings** (`native_adapter/signal_call.rs`, `pings/battle_help.rs`): the controlled athlete's simulation step publishes the SDK roster every second. Each athlete has a player id, side, lane and champion entity id; the list is logged once per match as `PING map roster …`. At each brain's first step with a roster known, the probe scans that brain's first 0x2000 bytes and its player state's first 0x1300 bytes (the furthest already read safely in dev4) for those entity ids: 64-bit words at 8-byte offsets, 32-bit at 4-byte offsets. It logs `PING map brain team=<side> index=<i> hits=<n> [field=entity …]`. A field counts when at least five brains hold distinct roster ids there, and every one of those ids belongs to an athlete whose SDK side equals the brain's side (or every one is the opposite side: logged as `sides=swapped`). The fields covering the most brains are used. The controlled athlete's brain is the one holding his entity id there, or, when his own brain does not show it, the one index of his side that the field leaves out after covering his four teammates ("by elimination"). That gives (a) his team, as a side at `+0x820`, and (b) his index. Those fields must agree. Then: `PING map resolved team= index= field= brains= sides= by=own_brain|elimination candidates: …`. Else `PING map unresolved: <why>; candidates: …` and, for a request, `PING map unresolved: nothing sent`. The candidates list shows the best three fields with every brain's value (`side:index=entity`), enough to finish the mapping by hand. No hit is cut off before the mapping uses it. The scan only reads.
- **The injection** (`windows/signal_send.rs`): right after the game's own broadcast ran for a brain, on the viewed match's worker, if a request is pending and the brain is a teammate (same side, not the controlled index, not yet written for this request), one entry is appended to its queue A: due = tick + 15, sender = the controlled index, 15 at +0xc, tag 4, lane 0, 0 at +0x14, the target at +0x18, 0 at +0x20. Only when length < capacity (the bytes at `ptr + len·40`, then `len + 1`; nothing is allocated, the capacity is never touched). Else `PING dropped: queue full team= to= len= (kept for its next step)`, once per teammate and request. Queue B and list C are not touched. Log: `PING sent team= to= from= kind=BattleHelp target= due= len=n/cap request=`. A request ends when all four teammates have it, or 2 s (120 ticks) after the probe first saw it: `PING expired request= sent_to=[…]`. A new match drops it.
- **The watch** (`pings/battle_help.rs`, read-only): for 10 s after the first entry was written, once a second, each living teammate's distance to the target and whether it shrank. Log: `PING watch t= p<index> player=<SDK id> dist= closing=yes|no|first since_send=`; `p?` while the mapping is unresolved. A dead teammate, or the target not alive, gets its own line.
- **The probe** (dev4's lines unchanged in form): brains are now known by side and index, so all ten are logged. That fixes the dev4 first-sight bug and the "third team planner" line, which is gone. `team=` in `SIGNAL` lines is now the side at `+0x820`. A brain whose side is above 1 or whose index is above 4 is not logged; that is said once a match (`SIGNAL probe: brain out of range …`), so a quiet log is never mistaken for a hook that never fired. B and C headers appear only when not empty. Budgets: 8,000 `SIGNAL` lines per match and 24,000 per session, because ten brains log about five times dev4's 350 to 1,045 lines a match. A dump that holds an entry the mod just wrote is always written, and `PING` lines are outside the budget. The `SIGNAL controlled` line is gone; its identity check never matched.
- **Stage 1's teammate actions removed:** `pings/act.rs` (the reply gates, the replacement inputs, the routes; 10 tests) and the `ping_step` call in `simulation.rs`. Teammates are entirely the game's. In `pings.rs` the gates, the act states, the teammate records and the ping's sender, team and route fields went with it. In `wheel.rs` the replies are no longer drawn (their window nodes stay, hidden).
- **Wheel edge rule** (`pings/wheel.rs`, `emotes.rs`): until the cursor first moves more than 2 px (`MOVE_PX`, a decision) from the press, the slot is measured from the press point; then from the drawn, inward-moved centre. So a quick press and release near a screen edge is a tap (ping) or the centre emote again, as before 1.7.1-dev2. The dev2 tests now check both cases and the 2 px rule.
- **Visible change:** releasing the wheel over a slot no longer shows a ping (ring, glyph and card): it only writes `PING slot … not sent in this build`. On purpose, not a fault.
- **Files:** new `native_adapter/signal_call.rs` (8 tests), `windows/signal_send.rs`, `pings/battle_help.rs`. Changed: `windows/signal_hook.rs`, `native_adapter/signal_probe.rs` (memory by brain, budgets, forced dumps), `native_adapter/mod.rs`, `windows/mod.rs`, `v070/signals.rs` (superseded notes, `SIGNAL_PLAYER_SIDE`, scan bounds), `v065/signals.rs` (placeholders, scan bounds 0), `tools/verify_native_profile.py` (the renamed constant), `pings.rs`, `pings/wheel.rs`, `emotes.rs`, `simulation.rs`. Removed `pings/act.rs` (it was never in git; a copy is kept outside the repository with the session notes).
- **Not verified:** anything in game: that the mapping resolves, that the entries are accepted and delivered, whether teammates react, whether the index at +8 is the sender.
- **Checked for this build:** `cargo fmt`; `cargo clippy --offline --all-targets -- -D warnings` with and without `--features research`; `cargo test --offline --release` 652 passed (663 minus the 10 `act.rs` tests and the removed ping-record test), 697 with `--features research` (also minus dev4's two team-memory tests, plus their two brain-memory replacements and 8 call tests); 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors) and 0.7.0 (92 anchors); the public DLL built from this source (`tools/build_dll.py`) holds none of the probe's or the call's log text (`SIGNAL `, `step_player=`, `PING sent`, `PING request`, `PING map`, `PING watch` absent; the research DLL holds them); `tools/build_dll.py --research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev5 --probe-tests 697` passed for 0.6.5 and 0.7.0 (626 anchors); package `LTDirectControl-1.7.1-dev5-research.zip` (126 files, SHA-256 `4953293a…64d5`). Not installed: the author installs.

## 1.7.1-dev4 (development build, research feature only) — 2026-11-02

Not seen in game. Development build, never for the Workshop. Built only as the research variant (`LTDirectControl-1.7.1-dev4-research.zip`); the public build of the same source carries none of it. A read-only probe that logs the game AI's own team call queues on game 0.7.0, so the layout read from the code can be confirmed in game and the AI's natural call traffic seen before the ping system talks to the game's planner. It writes nothing to the game and changes no input. Everything else is 1.7.1-dev3 unchanged.

- **Why:** a static reading of the 0.7.0 executable (2026-11-02) found that the AI already has a call language: a 16-byte signal of 28 kinds (Start, Mia, Battle, BattleDive, BattleHelp, BattleStop, HideLine, HideLineToo, DefenseLine, Ok, Reject, Cancel, Press, PressChange, Repair, SerpenPrepare, SerpenSetup, SerpenHunt, MorgardPrepare, MorgardSetup, MorgardHunt, AttackNexus, DefenseNexus, GankDive, PressTower, ComebackPick, SerpenDisband, MorgardDisband), broadcast through a per-team delay queue to every teammate with a per-receiver due tick. The exact byte layout of a queue entry is uncertain (the broadcast writes the signal from +0xc, the delivery's comparison reads a tag at +0x10), so the probe logs every entry raw.
- **The hook** (read from the code, game 0.7.0; family `SIGNALS`, research builds only): the per-player AI step `0xf16d60` calls the team broadcast `0xf163a0` at `0xf16de7` (`e8 b4 f5 ff ff`) right after its prologue, with the team planner in `rcx`, the context in `rdx`, the player planner in `r8`, the simulation in `r9` and its method table as the fifth argument. The mod replaces that one call: it calls the broadcast first and returns its result unchanged, then reads (so the step player's own new calls are already in the queue). Chosen over an instruction site near the entry because it is a plain call replacement like the mod's other hooks and sees each player's fresh sends. Installed on its own after every other family: if the call or the broadcast's opening bytes differ, only the probe is off (`SIGNAL probe unavailable: …`) and every other hook stays in.
- **What it reads** (read from the code): the team planner's queue A (`+0x1ee8`, a vector of 40-byte entries the broadcast appends and the delivery `0xe75a60` drains), queue B (`+0x1ea0`, 40-byte entries, also appended by the broadcast) and list C (`+0x1eb8`, 24-byte entries), each as {capacity, pointer, length}; the simulation's current tick (`+0x1b8`); the player planner's index in the team (`+0x8b0`), the word the broadcast passes to the simulation as its identity (`+0x820`) and a delay (`+0x12f8`; the delivery reads it from a receiver's state, that it is the same structure as the step player's is a guess, so the log calls it `delay?`).
- **When it logs** (threads: see "Threads: the rules" in docs/modding.md): only on the viewed match's worker thread (one compare, before anything else; background matches are turned away), the queues at most once per team per tick, and only when they changed since the team's last dump (a hash of all three lists). A header's length must be within its capacity and the capacity at most 64, else one line `SIGNAL probe: queue out of bounds` per match and nothing is read. At most 3,000 `SIGNAL` lines per match, then `SIGNAL budget reached`, and at most 12,000 per game session, then `SIGNAL session limit reached`. A new match is recognised only by the tick going back (nothing shows that the other arguments keep their addresses for a whole match; the session limit guards the log if that rule ever misses). A third team planner in one match is not expected; if one appears it is said once (`SIGNAL probe: a third team planner …`) and its steps are not logged. No pointer is kept: a team planner's address is remembered only as a number that tells the two teams apart. The lines go straight to the log after the change check (Normal level, so no change to Log detail is needed), as the research inventory does from the combat hook.
- **The lines:** `SIGNAL t=<tick> team=<0|1> step_player=<index> delay?=<n> A len=<n>/<cap>`, then per entry `SIGNAL A[i] raw=<40 bytes hex> due= from= tagC= tag10= name=<name of the byte at +0x10>[<name of the byte at +0xc>] lane= d14= q18= q20=`; the same for B. `from` is the dword at +8, labelled as briefed; read from the code (the broadcast's loop at `0xf1676d` skips the sender and writes each player's own index, `[r14+0x18]`), it is the **receiving** teammate's index, so one call shows as up to four entries with different `from` values. `SIGNAL C[i] raw=<24 bytes hex> kind= lane= q8=`. A name outside the 28 kinds shows as `?<number>`. `team` is the order the probe first saw the team in this match, not the game's side. Once per match and player: `SIGNAL player t= team= index= id=<identity>`. Once per match, when a player's identity equals the controlled athlete's actor (a guess that they are the same numbers; if not, this line never appears and the ids are compared by hand): `SIGNAL controlled index= team= id=`.
- **0.6.5:** no probe. The family is not in 0.6.5's `ENABLED` and its table (`v065/signals.rs`) holds values no address can have.
- **Files:** new `probe/src/native_adapter/signal_probe.rs` (names, line formats, the per-team memory and line limits, 10 unit tests), `windows/signal_hook.rs` (install and hook), `v070/signals.rs` and `v065/signals.rs` (tables); wiring in `native_adapter/mod.rs`, `windows/mod.rs` (install call, patch audit, the family kept out of public builds), `native_profile.rs` (`family::SIGNALS`), `v070/identity.rs` (`ENABLED`); `tools/native_profiles/0.7.0.json` (13 `SIGNAL_*` anchors, family `signals`, a review note) and `tools/verify_native_profile.py` (each offset of the table must be the displacement the anchored instructions use; the entry sizes; the hooked call's argument set-up; 0.6.5 must hold placeholders only). No ping system file changed.
- **Not verified:** anything in game: that the hook fires, the queue layout, the tag position, the lane numbers (0 Top, 1 Mid, 2 Bottom, from the planner's name table, a guess), the delay field, whether the identity is the controlled athlete's actor.
- **Checked for this build:** `cargo fmt`; `cargo clippy --offline --all-targets -- -D warnings` with and without `--features research`; `cargo test --offline --release` 663 passed (unchanged: the probe is research only), 700 with `--features research` (690 plus the 10 probe tests); 71 Python tool tests; `check_source_size.py` passed; `verify_native_profile.py` passed for 0.6.5 (87 anchors) and 0.7.0 (92 anchors, family `signals`), and a one-off run showed its new check refuses a wrong queue, index or delay offset; the public DLL built from this source (`tools/build_dll.py`) holds none of the probe's code or log text (`SIGNAL `, `step_player=`, `GankDive` absent; the research DLL holds them; the 0.7.0 profile, which every build embeds, now names the `SIGNAL_*` anchors); `tools/build_dll.py --research`, `stage_package.py`, `verify_build.py --version 1.7.1-dev4 --probe-tests 700` passed for 0.6.5 and 0.7.0 (626 anchors); package `LTDirectControl-1.7.1-dev4-research.zip` (126 files, SHA-256 `2f267e05…e21a`). Not installed: the author installs.

## 1.7.1-dev3 (development build) — 2026-11-02

Not seen in game. Development build, never for the Workshop. The season correction and repair are removed completely; nothing else changed. The author's decision of 2026-11-02: "remove it". Official interface only: no native hook, no address.

- **Why:** from 1.2.3 the mod corrected each athlete's season statistics after every series and repaired the past once per save, because the game added its AI-only pre-simulation's numbers to the athlete records instead of the played game ([docs/investigation-played-result.md](docs/investigation-played-result.md)). The game fixed that in its 0.6.5 update (patch note "Fixed an issue where the recorded stats doesn't align with spectated games"), carried into 0.7.0, and the mod now supports only 0.6.5 and 0.7.0. Read from the author's logs (game 0.7.0, builds 1.7.1-dev1, 2026-10-29 to 31): 11 series, all 110 athlete records unchanged, 0 corrections. Logs of 0.6.5 were not checked (none holds a finished series).
- **Removed** (read from the code): `probe/src/season_fix.rs` whole (the client side: watching the series, the per-series correction, the repair of the past, the save mark; the server side: the server extension handling the `lt_direct_control.season_*` commands and the Verbose-only match-record dump to `research/server-match-<id>.json`; 7 unit tests); in `lib.rs` the module line and the server extension registration (the mod now registers no server extension; the SDK's `set_server_extension` is an optional setter); in `client.rs` the observations field, the reset on leaving a save with its `SEASON save left` log line (the save-exit logic itself stays, other code uses it) and the per-frame call; in `simulation.rs` the call that noted the athletes of the match on screen. The log writes no `SEASON` lines any more. `result_audit.rs` (a read-only diagnostic) stays.
- **The two season fixes of 1.7.1-dev2** (reset on leaving a save, the repair marked done only when every correction landed) go with the rest. They were never committed; they are described only in the 1.7.1-dev2 entry below.
- **Old saves:** the value `season_repair_v1` that earlier builds wrote into a save stays there and is harmless; nothing reads it.
- **Not verified:** anything in game.
- **Checked for this build:** `cargo fmt`; `cargo clippy --offline --all-targets -D warnings` for both builds (with and without `--features research`); `cargo test --offline --release` 663 passed (670 minus the 7 season tests), 690 with `--features research` (697 minus 7); `check_source_size.py` passed; `verify_build.py --version 1.7.1-dev3 --probe-tests 663` passed for 0.6.5 and 0.7.0 (613 anchors); package `LTDirectControl-1.7.1-dev3.zip` (126 files, SHA-256 `ce2eab78…6251`). Not installed: the author installs.

## 1.7.1-dev2 (development build) — 2026-11-02

Not seen in game. Development build, never for the Workshop. Four fixes from an outside code review, each confirmed in the code first; nothing else changed. Official interface only: no native hook, no address.

- **Season repair: leaving a save forgets that save's corrections** (`client.rs`). When the player went back to the title screen and loaded another save, the season repair kept its state: the "this save is repaired" flag (so the repair of the past never ran on the second save), the series being watched, its pending corrections and the league watch. Now all of it is reset when a save is left, next to the other per-save state. Reset outside the session rearm, because that only runs when the native hooks are installed and the season repair does not need them. Log: `SEASON save left: season corrections of that save forgotten`.
- **Season repair: the save is marked repaired only when every correction landed** (`season_fix.rs`). Before, the repair of the past sent its corrections and the "repaired" mark straight after, so a correction that was not sent, refused by the server or never seen by the client still left the save marked done, and it was never tried again. Now each repair correction is tracked (series corrections are kept apart); the mark is sent only when every one was seen by the client, else the log names the failures (`SEASON repair incomplete: … corrections failed (first: …)`) and the repair runs again later (after its usual 5-minute wait). **Expected side effect:** on 1.2.3 the client's copy showed a repair's new values only after the next time skip (seen in game 2026-10-14), so on a save that still needs the repair the first run will likely log "repair incomplete" (corrections "not seen"); until a time skip each retry re-reads the season and re-sends the same corrections, which the server refuses (the value it holds is already the new one, compare-and-set), so nothing is written twice; after a time skip the retry finds nothing to fix and marks the save repaired. Cost: one full re-read every 5 minutes in the management screens until then.
- **Ping and emote wheels: the slot is measured from where the wheel is drawn** (`pings/wheel.rs`, `emotes.rs`). Near a screen edge the wheel is drawn moved inward, but the slot was picked from the press point, so the highlighted slot did not match the one under the cursor. Both wheels now pick from the drawn centre. The ping's point is unchanged: it is still the point under the press (`anchor`). The press point was only used for picking and is gone from both wheels. **Behaviour change (a decision for the designer):** pressed within about 120 px of the left, right or top edge (170 px of the bottom), the cursor already sits on a side slot of the drawn wheel, so a quick press and release there now sends that slot (ping: e.g. Back off at the left edge; emote: the left emote) instead of cancelling (ping) or the centre emote (emote). One alternative, not built: keep the centre until the cursor first moves.
- **Settings: an older save can no longer overwrite a newer one** (`settings_writer.rs`, `settings.rs`). The Settings window's Apply waits up to 2 s for a queued background save; if the disk was slower than that, the older queued save could be written after Apply's. Each save now carries a save number taken with its values, and a save older than the last one written to that folder is dropped (`SETTINGS an older save skipped: newer values are already in controls.json`). Counted per folder, so tests with their own folders do not interfere.
- **Season repair on game 0.7.0, finding (read from the author's logs, game 0.7.0, build 1.7.1-dev1, 2026-10-29 to 31):** 11 series checked, all 110 athlete records "unchanged" (their season record had already moved by exactly the stored replays), 0 corrections, 1 series skipped (a new series started first). The stored replays equal the played games (the controlled athlete's last K/D/A matches the replay in all 14 games compared). The mod wrote nothing in those series (no correction; the save was already marked repaired), so the game itself records the played games in the athlete records on 0.7.0; it fits the game's patch note "Fixed an issue where the recorded stats doesn't align with spectated games", which belongs to game 0.6.5 and is carried into 0.7.0 (corrected 2026-11-02: this said "the 0.7.0 patch note"). Nothing changed in the repair; whether to skip it on 0.7.0 is the author's decision.
- **Not verified:** anything in game; the season reset across two saves and the repair retry need a play test.
- **Checked for this build:** `cargo fmt`; `cargo clippy --all-targets -D warnings` for both builds; `cargo test --release` 670 passed (3 new: both wheels pressed 10 px from the left edge, released over the drawn centre and at the press point, the ping keeping the point under the press; a settings save that runs late after a newer one is dropped, counted per folder), 697 with `--features research`; `check_source_size.py` passed; `verify_build.py --version 1.7.1-dev2 --probe-tests 670` passed for 0.6.5 and 0.7.0 (613 anchors); package `LTDirectControl-1.7.1-dev2.zip` (126 files, SHA-256 `d253a9fa…12e8`). No unit test for the two season fixes: both need the game's client interface, which the tests cannot stand in for. Not installed: the installer refused while the game was running.

## 1.7.1-dev1 (development build) — 2026-10-29

Not seen in game. Development build, never for the Workshop. Stage 1 of the ping system: an interface-only testbed for how the calls feel in a match. Every teammate answers the same way (uniform hearing delay, same gates); the game's own planner is not told about the call (a later stage). Official interface only: no native hook, no address. New module `probe/src/pings.rs` with `pings/wheel.rs`, `pings/subject.rs` and `pings/act.rs`; other files take wiring lines only.

- **The wheel** (`pings/wheel.rs`): hold G (new binding "Ping wheel (hold)" in Control & camera) and a wheel opens at the cursor, built like the emote wheel. Four slots with glyphs and no text: Help here (up, crossed swords), Regroup here (right, the scan frame), Push here (down, chevron), Back off (left, the warning circle). The slot follows the cursor's direction; releasing sends; Esc, a right-click or a release over the centre cancels; clicks while it is open are drained (the emote wheel's capture flag, shared). The call's name in words shows as a tooltip under the wheel once the cursor rests on a slot (0.45 s). It does not open with the shop, Settings or the emote wheel open (the emote wheel does not open while it is), while aiming a skill or attack-move, while paused or while not controlling an athlete.
- **The subject** (`pings/subject.rs`): anchored where G went down, not at the moving cursor (the cursor moves to choose the slot; the brief's "following the cursor" read that way, a decision), and resolved again every frame while the wheel is open. Help and Back off snap to an allied champion (your own included, which makes "help me"): the sprite under the cursor, else the nearest within 50 px, else a lone one within 120 px with no other within 200 px, else the place. On the minimap the radii are three times larger (150, 360, 600 px, as asked; a guess to tune: on a 352 px minimap the nearest-within rule catches most allies). Regroup and Push are a place only. A place snaps to the nearest walkable cell of the wall grid; a press on the field beyond the map's edge is clamped onto it. G pressed over a teammate's portrait in the Tab panel targets that teammate. Before release: a ring on the ground (or around the ally) with the call's glyph, a ring on the minimap, or a frame around the portrait.
- **The ping**: one at a time, a new one replaces the old. Help, Regroup and Push last 10 s, Back off 4 s, by the simulation's clock (stamped by the first teammate step that sees it). Shown as a glyph and a ground ring at its point and on the minimap, and a card above the minimap's left edge (glyph and seconds left; the call in words in the card's tooltip). Sending never touches the player's own orders: the client stores the ping, the simulation reads it.
- **Replies** (`pings/act.rs`): each teammate hears the call 8 ticks after it was sent and answers ok, or no when a gate refuses: health below 35 %, the game's run-away helper returns an input (fleeing), the subject within 80 000 of a living enemy tower, the subject farther than 500 000 from him, or dead. Back off is refused only when dead or too far (stepping away is the safe answer; a decision). Dead teammates do not run their own step, so a living teammate's step answers for them. The reply shows for 3 s as a check or a cross on his Tab panel portrait (while the panel shows) and over his head on the field (an addition to the brief: the Tab panel is closed most of the time). Log: `PING reply player=… kind=… answer=ok|no reason=… hp=… run_away=…`.
- **Acting**, through the per-step AI call the mod already runs for all ten athletes (`Simulation::think` asks `ping_step` when the mod's own step returns None): Help and Regroup move to the point (an ally's live position), routed around walls like the player's own orders; within 60 000 he arrives and keeps the game AI's input for the rest of the call (latched, so he does not bounce at the edge). Push moves to the point, then the AI's own input (it pushes on its own). Back off takes the run-away helper's input when it gives one, else moves 120 000 from his own position straight away from the nearest visible enemy champion (else away from the pinged place), never toward it. Every tick the AI's input is kept for a skill cast or recall, while fleeing (other calls), when a gate now holds, or when the game rejects the replacement. Logs `PING act player=… kind=… goal=x,y` once and `PING done player=…` at the end.
- **Threads**: a step checks the `ACTIVE` atomic first, then the team and sender atomics, the match on screen (`origin.kind == 2`), the owning worker, not the controlled athlete, input allowed, the session gate; only then the ping lock, briefly, with no game call under it. Each ping carries a serial: a step that copied an older ping never writes into or ends a newer one. The run-away helper (it takes the AI's state as writable) is asked only at the hearing tick and while he may still act (not after arrival, not while the AI casts or recalls). Ended when control returns to the AI, on a swap or a new session, and by the screen's clock 2 s after its duration (a ping nobody heard, or whose teammates all died).
- **Wiring**: `client.rs` (the observations field, one call in `pre_update`, one in `apply_windows`, the draw list, the prewarm list, the theme refresh, the emote wheel blocked while the ping wheel is open), `simulation.rs` (`think`), `settings.rs` (the binding), `platform_input.rs` (the key), `team_status.rs`/`team_draw.rs` (portrait boxes on screen), `map_path.rs` (`Grid::nearest_open`), `movement.rs` (`navigation`), `lib.rs`. Five texts in all 16 languages.
- **Guesses**: whether the run-away helper returns an input only while fleeing (if every reply says `no reason=fleeing`, it returns one almost always and the gate must change); the minimap radii; the reply glyph height over heads; the glyph choices.
- **Not verified:** anything in game; the install (the game was running).
- **Checked for this build:** `cargo fmt`; `cargo clippy --all-targets -D warnings` for both builds; `cargo test --release` 667 passed (27 new: the snap rule's four cases and the minimap radii, the minimap inversion against the route drawing's mapping, the field point clamp, each reply gate and none, Back off's gates, the 8-tick hearing delay and expiry, the shared record (a step of an older ping finds no record) and the countdown, keeping the AI for skill, recall, fleeing, a gate that now holds and a replacement the game rejects, arrival latch, the route around a wall, the Back off point away from the nearest enemy, the wheel's slot pick, sending, cancelling and blocking), 694 with `--features research`; `check_source_size.py` passed; `verify_build.py --version 1.7.1-dev1 --probe-tests 667` passed for 0.6.5 and 0.7.0 (613 anchors); package `LTDirectControl-1.7.1-dev1.zip` (126 files, SHA-256 `5546c606…e7ea`). Not installed: the installer refused while the game was running.

## 1.7.0 — 2026-10-29

Release of 1.7.0-dev1 to dev3 below, as one DLL for game 0.6.5 and 0.7.0 (the release of 2026-10-28). Game 0.6.4 and 0.7.0 Beta 2 are no longer supported; `mod.mod_info` needs game 0.6.5 or later. Nothing changed between dev3 and this build apart from the version number, this entry and the README's version lines. Includes 1.6.2-dev1 (Live rating). Seen in game by the author, 2026-10-29: the dev1, dev2 and dev3 test checklists all passed (dev2 on both versions).

- **Game 0.7.0 supported** (dev1, dev2): every part of the mod is on for 0.7.0, the research skill reader excepted (research builds only). The two layout changes in 0.7.0 that the shared code had to follow: the viewer's playback config grew by 8 bytes (camera, pause, full width and vision settings), and the simulation's event entries grew from 0x70 to 0x90 bytes (the mod's cancel-recall event). The unit renderer has 16 callers (was 15); 17 item types have the game's item text (was 20). Findings: [docs/migration-0.7.0.md](docs/migration-0.7.0.md).
- **Athlete stat rules: the reaction curve has the base game's shape** (dev3): Reaction floor 2 and ceiling 132 give the game's own reaction delays; the ceiling slider goes to 132 ticks; the defaults stay 12 and 35.
- **Live rating** (1.6.2-dev1): the game's own match rating, top right, off by default.
- **Checks:** `cargo fmt`; `cargo clippy --all-targets -D warnings` for both builds; `cargo test --release` 640 passed, 667 with `--features research`; 71 Python tool tests, 97 viewer checks; `verify_native_profile.py` for 0.6.5 and 0.7.0; `verify_build.py --version 1.7.0 --probe-tests 640` passed for both (613 anchors); package `LTDirectControl-1.7.0.zip` (126 files, SHA-256 `e06c2179…9249`); installed over 1.7.0-dev3.

## 1.7.0-dev3 (development) — 2026-10-29

**Seen in game by the author, 2026-10-29: "All tests passed"** (the dev3 test checklist). Superseded 2026-10-29: "Not seen in game". Development build, never for the Workshop. The athlete stat rules' reaction curve takes the base game's shape, so the settings can come close to vanilla. The author's go-ahead of 2026-10-29: "go ahead with A, keep the defaults". Nothing else changed.

- **The curve** (`athlete_rules::reaction_band`): the base game's projectile gate (docs/investigation-stat-rules-0.6.4.md, section 2) on the line Accuracy + Dodging = 100, its extra Accuracy delay included, stretched so the low end at gap -100 is the Reaction floor and the high end at +100 the Reaction ceiling. With floor 2 and ceiling 132 (`VANILLA_FLOOR`, `VANILLA_CEILING`) it is the game's own band at every such matchup: -100 2..6, 0 17..35, +100 72..132 ticks. Off that line it is close, not exact: the game's gate depends mostly on the enemy's Dodging, the mod's on the gap. Before, the curve was nearly straight (sixteenths of the span).
- **The defaults stay 12 and 35** (the author), and now give -100 12..13, 0 15..18, +100 24..35 (before: 12..16, 18..24, 26..35): faster in the middle.
- **Reaction ceiling slider:** up to 132 ticks (was 60). The floor slider (1 to 40) already reaches 2. Hints and translations unchanged.
- **Tests:** the curve checked against the game's formula written out independently for all 101 matchups on that line; the defaults' table; order and limits for four settings; the sliders reach the vanilla values. One spread test now uses the vanilla band (the 6..22 test band became 3 ticks wide at gap 0).
- **Checks:** `cargo fmt`; `cargo clippy --all-targets -D warnings` for both builds; `cargo test --release` 640 passed, 667 with `--features research`; 71 Python tool tests, 97 viewer checks; `verify_native_profile.py` for 0.6.5 and 0.7.0; `verify_build.py --version 1.7.0-dev3 --probe-tests 640` passed for both (621 anchors); package `LTDirectControl-1.7.0-dev3.zip` (126 files, SHA-256 `740b5644…59fe2`, no personal paths); installed over 1.7.0-dev2.
- **Not verified:** anything in game.

## 1.7.0-dev2 (development) — 2026-10-29

**Seen in game by the author, 2026-10-29: "All tests passed on both versions"** (0.6.5 and 0.7.0, the 1.7.0-dev2 test checklist). Superseded 2026-10-29: "Not seen in game". Development build, never for the Workshop. The minimap, the hover outline and the native tooltips on game 0.7.0, reviewed by hand (the author's go-ahead of 2026-10-29: "go ahead with outline, minimap and tooltips"). Nothing else changed. Findings: [docs/migration-0.7.0.md](docs/migration-0.7.0.md) section 3. Everything below is read from the code unless it names a tool.

- **Minimap:** all 54 operands the mod repoints are the same instructions with the same game data as on Beta 2: the renderer `0x2f26790` and the input handler `0xb9be00` align with Beta 2's instruction for instruction at each one, including the two the draft could not place (`fog_cell_pitch`, one `input_narrow_right_x`). The renderer's constants are used as often as on Beta 2; the input handler's 0.5 and 3.0 each have one use more, a copy of the zoom limit (view `+0x110`), not a mod operand. The operands are in the 0.7.0 profile.
- **Outline:** the unit renderer `0x2466400` (2554 of 2558 instructions equal Beta 2's; the rest is the vision flag at config `+0x53` and registers), its shader, float and colour setters (found by its calls), the line and nine-patch markers, and the command drop `0x1c6d10` (Beta 2's instruction for instruction; commands still 0xd0 bytes). **16 renderer calls, Beta 2 15:** the shared draw method of 11 unit types now has an identical copy for one more unit view type (guess: Jiangshi's, from a helper two calls down that loads its sprites), which on Beta 2 drew through another call the mod already hooked; every call sets up its arguments as on Beta 2. The drop's jump table moved (`0x3f43114`): the draft had kept Beta 2's bytes.
- **Tooltips:** all 65 champion tables carried. Each built-in table is paired with Beta 2's by its lookup branch and its own type and source-file names; the Workshop champions' table by its four methods (all equal). Table sizes unchanged. The game's ability panel still calls the descriptions at `+0xe0/+0xe8/+0xf0`. The data-defined champions' resolver only gained stack slots (3097 of 3101 instructions equal with frame slots and registers ignored; no field moved). The Assets read size stays 0x4c0, now measured (the sheet getters and their callee are Beta 2's instruction for instruction).
- **Item effect text:** 17 item tables (Beta 2: 20), the only tables holding the item marker method at `+0x60`. Every text method is Beta 2 code with the same strings, the same calls and the same system calls, and the game's item text sheet is Beta 2's byte for byte, so Beta 2's check that the text lookup cannot fail carries over. The game's item panel `0x22e1ef0` (the PC item info screen; its caller aligns with Beta 2's) passes output, item and Assets as before; the list builder and its drop are Beta 2's apart from registers.
- **Files:** `v070/identity.rs` (`ENABLED` adds minimap, outline, tooltips), `v070/layout.rs` (16 outline sites, the drop function and table, 17 item tables, the tooltip guards regenerated from the reviewed anchors), `tools/native_profiles/0.7.0.json` (families, `SHADER_PROLOGUE`, 443 tooltip anchors, the minimap section), `minimap.rs` test (0.7.0's plan back in), `windows/tests.rs` (the Assets note).
- **Checks:** `cargo fmt`; `cargo clippy --all-targets -D warnings` for both builds; `cargo test --release` 639 passed, 666 with `--features research`; 71 Python tool tests, 97 viewer checks; `verify_native_profile.py` for 0.6.5 and 0.7.0 (0.7.0: all nine families, 79 anchors, 38 layout guards, 443 tooltip guards); a one-off script checked every outline constant against the 0.7.0 executable (no tool does: the 16 calls, the renderer's only callers, each entry, the drop call, its jump table); `verify_build.py --version 1.7.0-dev2 --probe-tests 639` passed for 0.6.5 and 0.7.0 (613 anchors); package `LTDirectControl-1.7.0-dev2.zip` (126 files, SHA-256 `af1246fc…4c84`, no personal paths); installed over 1.7.0-dev1.
- **Not verified:** anything in game for these three families on 0.7.0, and 0.6.5 with this DLL.

## 1.7.0-dev1 (development) — 2026-10-28

**Seen in game by the author on 0.7.0, 2026-10-29: "All tests passed"** (the 1.7.0-dev1 test checklist: session, camera, movement, combat, shop, stat rules). Not reported: 0.6.5 with this DLL. Superseded 2026-10-29: "Not seen in game". Development build, never for the Workshop. Game 0.7.0 (the release, Steam update of 2026-10-28) supported next to 0.6.5; 0.6.4 and 0.7.0 Beta 2 dropped. The author's decisions of 2026-10-28: "0.6.5 and 0.7.0 official"; the grouped approach ("go ahead, and i prefer grouped approach"). Includes 1.6.2-dev1 (Live rating) unchanged. Findings: [docs/migration-0.7.0.md](docs/migration-0.7.0.md).

- **0.7.0 tables** (`native_adapter/v070/`, `tools/native_profiles/0.7.0.json`), read from the code: drafted from Beta 2, the function map reseeded, the rest found by hand. Families on: session, camera, movement, combat, shop, athlete stat rules. Off until reviewed: minimap (the game's own minimap shows), outline (the renderer has 16 callers, Beta 2's 15), tooltips (the mod's own text resolver), and the research reader. Their entries are not in the profile.
- **The viewer's playback config grew by 8 bytes on 0.7.0:** camera target, full width, compact side, paused and vision are each 8 bytes further. The shared hook code wrote them at fixed offsets; they are now `layout::CONFIG_*` per version (0.6.5 unchanged). Without this the camera, the pause and the vision choice would have written the wrong bytes on 0.7.0.
- **The simulation's event entries grew from 0x70 to 0x90 bytes:** the mod's own cancel-recall event uses the 0.7.0 stride (`CANCEL_RECALL_INLINE`).
- **Dropped:** `native_adapter/v064/`, `v070b2/` and their profiles; `mod.mod_info` needs game 0.6.5 or later. The kept copies stay in `research/game-builds/` (the first 0.7.0 beta's copy is renamed `0.7.0b1`; `0.7.0` is the release). Tests moved to 0.6.5 and 0.7.0; the reaction-gate shape only 0.6.4 used (`Kind::Capture`/`Complete`) is kept for the census and the verifier.
- **Checks:** `cargo fmt`; `cargo clippy --all-targets -D warnings` for both builds; `cargo test --release` 639 passed (the per-version hook tests now run for two versions, not three), 666 with `--features research`; 71 Python tool tests, 97 viewer checks; `verify_native_profile.py` for 0.6.5 (all families) and 0.7.0 (78 anchors, 38 layout guards, 35 stat sites).
- **Not verified:** everything in game on 0.7.0, and 0.6.5 with this DLL.

## 1.6.2-dev1 (development) — 2026-10-27

**Seen in game by the author on 0.6.5, 2026-10-27 to 28: the test sheet passed** (log: 10 matches, each `RATING first reading 66 ... (shown 6.60; read field)`, so the `rating` field read works and the whole-document fallback was never needed). Not tested on 0.6.4 or 0.7.0 Beta 2. Development build, never for the Workshop. One new option, **Live rating** (Interface > Appearance, after HUD motion, **Off by default**), and nothing else. Design approved by the author 2026-10-27 (local design page `design/hud/review-live-rating.html`, round 3: top right under the input delay, stays put, star, no tooltip).

- **What it shows** (read from the code): your athlete's match rating as the game keeps it, the `rating` field of the athlete's match statistics, divided by 10 with two decimals like the result screen (66 is 6.60). Nothing is added, smoothed or predicted. Only the athlete you control.
- **How the game makes the number** (read from the game's code, 0.6.5, in a read-only investigation 2026-10-26; the full-map formula matched all 595,650 player results recorded by the LT AI Coach exporter): on the normal map it starts at 66 and moves with "win probability added" credit, paid about 4 s after a fight ends, when a tower, Morgard or Serpen dies, every 30 s, and at the end of the match. So the number moves in steps, and the result screen can differ a little from the last number seen in the match (the end-of-match share). The single-lane and death-match formats use a stat-based formula instead (starts lower, moves more continuously).
- **Where**: the row under the input delay's place in the top right corner (UI space 1920 x 1080: number right-aligned to x 1902, row y 94 to 118), whether "Show input delay" is on or not. The mod's numeric font at 20 px (the input delay is 28 px), grey #cbc9c7, with the input delay's 2 px dark shadow; an 18 px star 6 px before the number. No box, no tooltip. Shown while you play (Running or Paused), like the input delay.
- **How it is read** (read from the code): `Player::statistics_json("rating")` through the official mod tools (no new game hook), in `sample_hud` for the controlled athlete only (every 6 ticks), and only while the option is on. If the field read gives nothing, the whole statistics document is read once instead and that read is kept. One log line per match: `RATING first reading <n> for athlete <id> (shown <text>; read field|document)`, or `RATING unreadable ...` (then nothing is drawn). Stored per match and athlete (`live_rating.rs`), so a new match never shows the last one's number, and a paused match keeps its number.
- **Files**: `probe/src/live_rating.rs` (new), `simulation.rs` (the read), `client.rs` (switch and drawing), `settings.rs` (the option), `drawn_glyphs.rs` and `tools/records/drawn-glyphs.json` (the star at 18 px in grey and in shadow black: `probe/ui/drawn_ef_star_18_cbc9c7.png`, `..._000000.png`; the existing 32 px `ef_star.png`, the shop's "Recommended" glyph, is now listed in `tools/generate_endfield_glyphs.cjs` and regenerated byte-identical), the 16 language files (label and hint, appended), `docs/modding.md` (the render step), `tools/records/ui-graphics.json` and `tools/verify_build.py` (the two star images counted: 124 UI files, 22 drawn glyphs), `settings_ui.rs` test (the Interface page scrolls 62 px further for the new switch row).
- **Checks**: `cargo fmt`, `cargo clippy --all-targets -D warnings`, `cargo test --release` 678 passed (4 new in `live_rating`), `tools/build_dll.py`, `verify_build.py --version 1.6.2-dev1 --probe-tests 678` passed for game 0.6.4, 0.6.5 and 0.7.0 Beta 2 (621 anchors), package `LTDirectControl-1.6.2-dev1.zip`.
- Not changed: the HUD strip, the stats panel, the input delay, and the Settings design reference `design/hud/settings-preview.js` (it has no Appearance section yet; left for the author to decide).

## 1.6.1 — 2026-10-26

Release of 1.6.1-dev1 and 1.6.1-dev2 below, as one DLL for game 0.6.4, 0.6.5 and 0.7.0 Beta 2. Nothing changed between dev2 and this build apart from the version number and this entry.

- **The Tab panel drawn directly** (dev1). Seen in game by the author on 0.6.5, 2026-10-25 and 26, over many matches: holding Tab no longer drops the frame rate the way it did on 1.6.0 (the game's cost for the panel fell from about 6 ms to about 2.4 ms a frame in the author's PERF logs); the panel looks as before. Not yet looked at on 0.6.4 and the 0.7 beta.
- **A hidden diagnosis switch, `perf_pacing_off`** (dev2), default off; with it off the mod behaves exactly as 1.6.0. With `"perf_pacing_off": 1` in controls.json, from the next match the game's viewer runs on its own frame clock and the worker is held at a fixed lead of 2 ticks, which can add up to one tick of input delay above 60 fps; the details are in the dev2 entry. Seen in game by the author with it on, 2026-10-25 and 26: control, shopping and the HUD as usual, several capped matches smooth. Why it ships: with a frame cap, some matches on 1.5.x and 1.6.0 grow slower over 5-10 minutes; whole-game profiles of three such matches (xperf, 2026-10-25) put the growth in one call chain inside the game's own viewer update, switched on by an in-match event not yet identified, with the mod's own work flat at 0.2-0.3 ms a frame and the pacing idle in the match that grew most. Whether the pacing is involved in the trigger is open (paced matches grew in 3 of 5 recorded, unpaced in 0 of 4); the switch lets a player with the problem try without it. Log detail = Verbose adds a `PACING` line every 10 s.
- Other slowdowns looked at the same days were not the mod's: a Workshop shop-button mod reading the game every frame (1.9 ms a frame while loaded), and desktop programs sharing the processor with the game (lock stalls and hyper-thread sharing with Firefox, an AI chat app and the Claude app open: slow; closed: smooth), measured with the processor's cycle counters at an unchanged 4.36 GHz.

## 1.6.1-dev2 (development) — 2026-10-25

**Not seen in game. Development build, never for the Workshop.** One hidden diagnosis switch, `perf_pacing_off`, and nothing else. Why: seen in game 2026-10-25, with a frame cap (RivaTuner 70, the game's own 60 or the NVIDIA driver at 72) the game's own viewer update grows in some matches from about 1 ms to 5-12 ms a frame over 5-10 minutes while the mod's own work stays flat, and with the mod disabled two capped games stayed flat at 72 fps. The growing game's end line read `TIMING slow playback 41.0 ticks/s of 60; lead=2; changes=217; dropped=187.8 ticks`. The pacing's writes to the viewer are the prime suspect; this switch lets a capped match run without them while everything else stays on. Everything below is read from the code or measured by the tests named.

### The switch (read from the code)

- `"perf_pacing_off": 1` (or `true`) in controls.json; no control in the Settings window; default off. Read once per frame in `Client::post_update` like `perf_hide_mod_ui` and `perf_draw_probe` (`NativeTiming::configure_pacing_off`), with one `PERF perf_pacing_off=... : from the next match, ...` line when it changes (Verbose, like the other two).
- **It takes effect at the next match.** It is latched when a match is bound (`observe_start`), so a match is never half paced; turning it on or off mid-match changes nothing until the next one. A match bound with it on writes `TIMING pacing off for this match (perf_pacing_off): ...` (at every log level), so a log says which kind of match it was.

### What it turns off, for the whole match (read from the code)

- **Running viewer frames go to the game untouched** (control and AI control alike; `NativeTiming::untouched`, checked in `view_hook_body`): no write to the viewer's frame clock (no owed time held back, none paid back, none dropped), no trim of `dt` (the game's own `dt` is passed), no `PlaybackOverride` (sync, mode, speed and pause stay as the game has them; they are only read, for the AI-control speed multiplier), no lead computation. The frame time is still measured on the wall clock, only for the PACING line.
- **The worker keeps a fixed lead of 2 ticks** (`PACING_OFF_LEAD`; in AI control 2 times the speed button, rounded up, as today). Why 2: it is what the mod settles on under a 60-72 fps cap today (the growing match logged `lead=2`), so the worker's side matches the matches that grew, and at 60 fps and up it leaves a tick of slack for the game's own frame clock.
- **Why the worker is still held at all** (read from the code and the mod's logs, 0.6.x): the game does not pace its on-screen simulation by itself. Its worker loop sends a frame and starts the next tick without waiting, and the client moves every sent frame into the viewer's list, which has no limit. After the "View result" hand-back in a kept log, the worker ran about 3,300 ticks/s (about 55x real time). With no hold at all the match would be simulated in 10-20 s while the screen plays it at 1x, and every click would land a minute or more ahead of the picture. Built as option (a) of the director's choice, 2026-10-25.

### What stays as it is (read from the code)

- **The start hold and Pause work as today**, viewer included: before Start and while paused the worker waits at the publication boundary, and the held viewer frames keep the usual handling (dt 0 and the frame clock cleared; the one initial frame; speed and pause overridden for the held frame only, then put back). Why: an untouched viewer would keep the whole hold's time in its frame clock and play it back fast after Start or Resume. So the switch tests the running frames, which is where the update grew.
- **The camera family still steers the camera** (follow, free camera, wheel zoom, the vision setting and the forced full-screen layout, `prepare_camera`). These write the camera fields of the config (+0x18 to +0x20, +0x45, +0x4b) and the view's zoom and pan, not the playback fields or the frame clock. Kept because control needs a usable camera. So "untouched" means the playback path; the camera writes are the same as in dev1.
- Everything else: the think, steer, move, attack, skill and outline hooks, the shop and its hooks, the stat rules, the HUD, the session strip, the stats panels, the Tab panel, input, sounds and the PERF lines (`viewer_incl_native` then measures the untouched game update, which is the point). The HUD, strip, panels and Tab draw and write exactly as in dev1 (the record tests pass with no record changed).
- **Nothing releases control because pacing is off.** Every release path was read: `check_limits` (15 s loading, 2 s heartbeat) is unchanged and still reachable only as before, since the start hold still sets the initial frame and the pause acknowledgement; the frame-count guard in `after_view` holds because one frame can play at most what is queued, and the queue is at most the lead (2, or 2 times the speed in AI control), under the guard's limit of at least 4; "Playback advanced while held" cannot fire because held frames still get dt 0; the `tps`/queue and viewer-identity checks in `view_hook_body` are unchanged and the queue stays small; `watch_worker` only logs; "TIMING slow" only logs.

### What the author should expect in the test match (read from the code; a guess until played)

- Control, buying and the HUD as usual. Input timing is a little less exact: the worker is always allowed 2 ticks ahead (up to about 17 ms more than the lead of 1 at 60 fps and up), and the game's own frame clock decides when a tick is shown. When the queue runs empty the game keeps the time and catches up by itself instead of the mod paying it back gently, so a short run of two ticks in a frame can happen. The input-delay readout and the `INPUT TRACE` lines measure this as they always do.
- **Keep the game's speed at 1x and do not use the game's own pause button while in control:** in running frames the game's speed field is no longer forced to 1x, and the game's own pause and highlight fields are no longer overridden.
- The camera behaves as in dev1.

### How to read the result

- Play a capped series (the same cap as when it grew) with the switch on and Log detail = Verbose, and compare the PERF `viewer_incl_native` and Frame Lab's windows with the matches that grew.
- If the game's update stays flat in a capped series with the switch on, the cause is the pacing's work on the viewer: the frame-clock rewrite, the dt trim or the playback-field override. If it still grows, those are cleared, and the cause is elsewhere.
- **This switch tests the viewer-side manipulation only.** The worker gate (the wait after each publication) stays on at a fixed lead, so it is not tested by this switch; if the update still grows with the switch on, the worker gate stays a suspect.

### The PACING line (read from the code; format tested)

- `PACING playback ...` (every 10 s of running, Verbose; prefix unchanged, and the same text after `TIMING slow` when a window plays slow) now carries the current owed time, `owed=<n> ticks`, beside the rate, lead, average client frame in ms (already there), changes and dropped. A match bound with the switch on adds `; pacing off (perf_pacing_off), the game's own frame clock`, and its owed and dropped stay 0 (the mod holds nothing back). Format, with made-up numbers: `PACING playback 59.8 ticks/s of 60 over 10 s; lead=2 average client frame 13.9 ms owed=0.31 ticks changes=3 dropped=0.0 ticks this match`.

### Tests and checks

- New unit tests in `native_timing/pacing_off_tests.rs` (a child module of `native_timing`, kept apart so `native_timing.rs` stays under the 3,000-line limit of `tools/check_source_size.py`): the switch is read every frame, logged once per change and latched only at the next bind (`the_pacing_switch_is_read_each_frame_and_applies_from_the_next_match`); with it on, running frames (control and AI control) are untouched, the start hold and pause are not, the game's update playing the whole queue in one frame releases nothing, and control and input stay allowed (`with_pacing_off_running_frames_are_untouched_and_holds_and_control_stay`); the worker publishes up to the fixed lead without waiting and waits only beyond it, and 30 fps never changes the lead, owes nothing and writes the new PACING fields (`with_pacing_off_the_worker_keeps_the_fixed_lead_and_the_frame_time_only_measures`). The existing timing tests are unchanged and pass.

## 1.6.1-dev1 (development) — 2026-10-25

**Not seen in game. Development build, never for the Workshop.** Build "2c" of the Stage 2 design (`docs/investigation-direct-draw-hud.md`, section 3): the Tab panel is drawn with the SDK's direct draw calls, as the HUD strip was in 1.6.0-perf-dev3. Why: on 1.6.0 the author saw holding Tab drop the frame rate from about 70 to about 50, the game's cost about 6 ms a frame while the mod's own Tab work is 7-13 µs (seen in game, 2026-10-25). Nothing else changed. Everything below is read from the code or measured by the tests and tools named; nothing of it is seen in game.

### What moved and what stayed (read from the code; node counts measured by the template test and the pixel tool's tree model)

- **Drawn** (`team_draw.rs`, at the z values the nodes had, 1600-1607, in the old template's node order): the panel and its top, bottom, left and right rules; per side the five headers (LV, K/D/A, CS, and the translated ITEMS and GOLD, read through `lang::tr` each frame as before; ITEMS only while a row has a slot); per row the row tint (yellow on your athlete's row), the selected bar, the row line, the portrait's frame (blue or red by side), its inset and the dark shade while dead or unknown, the outlined respawn count, the level, K/D/A, CS and gold texts; per item tile its box, inset and the "?" of an item without an icon.
- **Stays in the tree** (`team_status::template`): the root `lt_team_status` (now a container, shown and hidden as before), the two `side{s}` and ten `lane{l}` containers, each row's `portrait` container with its `icon` image (`ui_set_champion_icon` needs an image node), and the `item{i}` tiles as containers of their `icon` images (sprite-sheet frames), so every path the existing code writes is unchanged. The template has 153 nodes instead of 379 (no colour box or label left; a unit test counts them). Shown while the panel is open in the scripted test: 83 nodes instead of 210-211 (the fake tree has no champion portraits; in game add the 10 portraits: about 93, up to 113 with four items in every row, against about 220-240 before). The brief aimed at 70-80: reaching that would mean dropping the `portrait` and `item{i}` containers, which changes the paths (and the write lines) of the portraits and item icons, so the paths were kept.
- **Draw commands while open:** 207 a frame in the scripted test, 213 with one row dead (the countdown is five texts: four dark copies and the white one). At the probe's 2.1-2.5 µs a command (section 7 of the design) that is roughly 0.4-0.5 ms, against the about 6 ms the tree panel cost (a guess until the PERF capture says so).
- **When it draws:** only while the root is shown in this frame (Tab held, the panel built), in `post_update` inside the `Team` PERF section (`tab` in the log; name unchanged); sent in `post_render` with the other windows' lists (`Client::post_render_inner`); nothing while `perf_hide_mod_ui` is on (`draw_list::submit` returns first; the panel's own step also hides it), nothing during the one-second rebuild gap after a background or language change. One `DRAW origin tab panel: ...` log line compares the drawn corner with the root's laid-out rectangle, as the other drawn windows do.
- **The state step is unchanged:** the same values every frame (`slot`, capacity, portrait retries, the respawn caption), no input-blocking rectangle (none before either), the panel still built ahead by `prewarm.rs` (fewer nodes now). `draw_list::Align` gained `End` (right-aligned gold and its header; the game's `Right`).

### The K/D/A (read from the code; measured by a unit test and the pixel tool)

- The rich label (`<#eeececff>k<#989694ff>/<#ff855bff>d<#989694ff>/<#eeececff>a<>`, size 22, centred in its 110 px box) cannot be a draw call, so it is **five** single-colour texts: kills `#eeececff`, slash `#989694ff`, deaths `#ff855bff`, slash `#989694ff`, assists `#eeececff` (the brief's "three" read as three colours: kills and assists share one but are not adjacent). Each run's width is `hud_style::width(run, 22)`; the five are laid side by side and the whole centred in the box; each run is drawn left-aligned from its place (`team_draw::KdaView::set`, recomputed only when the numbers change). A unit test checks the run texts, that they touch, and that the whole is centred.
- **Expected deviation (measured offline):** `hud_style::width` reads the advance table `tools/generate_hud_fonts.py` writes from the *medium* weight, while the K/D/A is set in the *numeric* weight, whose digits and slash are 0-0.5 px wider each at 22 px (canvas measurement of `manrope_numeric.ttf` in the pixel tool's browser: "0/0/13" 69.5 px against 66.5 px from the table, "12/3/24" 79.8 against 77.5, "99/99/99" 100.8 against 99.7). So the drawn K/D/A is up to about 3 px narrower: its first glyph sits up to about 1.5 px right and its last up to about 1.5 px left of where a label laid out with the numeric advances puts them, and the inner glyphs move by less. How the game itself lays out rich text (rounding per run, kerning) is not known; a guess: the same advances, so the same 1-3 px.
- Not done (the brief named `hud_style::width`): a small numeric-weight advance table for 0-9 and "/" would remove this difference. Left for the author to decide.

### Records (measured by tests)

- `probe/testdata/ui_identity/team.txt` (rewritten on purpose with `LT_UPDATE_UI_RECORDS=1 cargo test --offline --release team_panel`): 1,493 → 217 lines. A frame-by-frame check of the kept nodes (every line of the root, the sides, the rows' visibility and place, `portrait.icon` and `item{i}`/`item{i}.icon`, against the 1.6.0 record) found them identical in all 80 frames. The only changed line is the spawn line (`len=54530 fnv=73555dbd…` → `len=15213 fnv=fdbf1e34…`, the shrunk template). The 1,276 lines that left all belong to drawn elements: 500 CS texts, 500 gold texts, 100 K/D/A texts, 60 level texts, 11 respawn texts, 10 headers' texts, 2 GOLD header places, 2 ITEMS header visibilities, 10 row gold places, 10 row tints (`lane{l} | color`), 10 selected bars, 10 portrait frame colours, 11 portrait shades and 40 "?" visibilities.
- `probe/testdata/ui_draw/team.txt` (new, 10,460 lines): the same scripted Tab test's draw commands per frame, as the other windows' draw records. The scripts never show a "?" (every scripted item has an icon); `team_draw::tests` covers it.
- No other record changed (`git diff --stat probe/testdata`: only these two files and the new pixel fixture).

### Pixel comparison (measured by `tools/compare_draw_review.cjs`)

- **Against 1.6.0's own Tab** (new fixture set `probe/testdata/ui_pixels/dev5/team/`, 136 KB: 1.6.0's write record from git and its template, whose hash is the record's spawn line; 1.6.0-dev5 and 1.6.0 drew the same Tab): `node tools/compare_draw_review.cjs --base=dev5 team`

```
team      7  Tab open, one row dead (countdown)               nodes 379 (211 shown) -> 153 ( 83 shown), 213 drawn  differing px   947  largest region 47 px 11x12 at (519,604)
team     20  Tab held open                                    nodes 379 (210 shown) -> 153 ( 83 shown), 207 drawn  differing px   806  largest region 34 px 4x14 at (1136,332)
team     65  Tab open again, values changed                   nodes 379 (210 shown) -> 153 ( 83 shown), 207 drawn  differing px  1166  largest region 58 px 7x15 at (514,400)
Largest differing region over all states: 15 px across.
```

  Every differing pixel of the three states lies inside the two K/D/A boxes (counted on the diff images: 947 of 947, 806 of 806, 1,166 of 1,166), the glyph-edge shift above; the panel, rules, rows, tints, tiles, portrait frames, the dead row's shade and its outlined countdown (four offset copies against the tool's stroke) show no difference at the tool's threshold. Frame 7 (one row dead) is a review state new in this build.
- **Against dev3** (`--base=dev3`, the brief's command): 0 differing pixels in all 33 HUD, plate, session, stats and effect states (unchanged windows). The three Tab states differ by 34,024, 35,878 and 36,281 pixels, largest region 810 px (27 × 30) at (659,326): the dev3 fixture holds the Tab **before** 1.6.0-dev4 moved its columns (dev4's entry reported the same 35,800 and 36,143 for that intended move), so these numbers measure dev4's layout change, not this build.

### Allocation (read from the code)

The view keeps its texts and formats a number again only when it changes (`NumberView`, `TimerView`, `KdaView`; a unit test checks a steady value keeps its buffer); the list keeps its storage. What still allocates on a steady frame, as in 1.6.0 and left as it was: `TeamStatus::snapshot` clones the ten players every frame, and the item icon write builds its property text (`artwork.item_icon` and `format!("visible: true; …")`) for every shown tile every frame before the cache compares it.

### Tests and tools

- New unit tests: the K/D/A runs (texts, contiguity, centring, colours), steady texts keep their buffers, headers and values share each column's box and alignment for 4, 5 and 6 slots (ported from the template test), a hidden panel or row draws nothing and the selected and dead row draw their bar, shade and countdown, and the shrunk template's node count and kept paths. Removed: the rich-text caption test (no rich text left). The template assertions of `columns_sit_16_px_apart_...` moved to the draw-list test.
- `PropCache::text_keyed` has no caller now (the Tab panel was the last tree label written that way); it stays, marked `allow(dead_code)`, because `docs/modding.md` documents it.
- `tools/compare_draw_review.cjs`: the Tab comment updated, review state Tab frame 7 added.
- Docs: `docs/investigation-direct-draw-hud.md` section 3 (Tab's "stays in the tree" line marked superseded, with what moved), `docs/modding.md` (the drawn windows sent in `post_render`, and a line on drawing several-colour text run by run).

### Choices made where the brief allowed two readings

- The portrait frame, inset and shade and the countdown are drawn (the brief lists "the alive/dead shade if it is a drawn box"); only the portrait image stays.
- Kept the `portrait` and `item{i}` containers so no kept path or write line changes (the node target of 70-80 is therefore not met: 83 in the test, about 93-113 in game).
- The pixel base for this build is a new Tab-only fixture of 1.6.0 (`dev5`), not a re-export of this build: the comparison needs the tree panel as the base, and the dev3 set's Tab predates dev4's columns.

### Not seen in game: what to look at

- The K/D/A: its glyphs up to about 1.5 px inward at each end (above).
- Equal z: the selected bar and the row line (both 1602) are sent in the template's order, as were the HUD's equal-z commands (accepted in game in 1.6.0).
- The drawn corner against the tree root: the `DRAW origin tab panel` line in the first match's log.
- The gold column right-aligned by the draw call's `Right` alignment (first use of it).
- The frame rate while holding Tab.

### Checks

`cargo fmt`; `cargo clippy --offline --all-targets -- -D warnings` clean; `cargo test --offline --release`: 671 passed, 1 ignored (1.6.0: 667 and 1; 5 new, 1 removed). `python tools/build_dll.py`: DLL 5,678,080 bytes. `python tools/stage_package.py`: 124 files staged, public build. `python tools/verify_build.py --version 1.6.1-dev1 --probe-tests 671`: tables verified for game 0.6.4, 0.6.5 and 0.7.0b2 against their own executables, 621 native anchors, ABI 6 and null-host rejection; DLL SHA256 52dc2916…5f4942. `python tools/package_mod.py --version 1.6.1-dev1`: 124 matching files, SHA256 5f1d92e7…21c0e9. `python tools/check_source_size.py` passed (`team_status.rs` 719 → 687 lines; `team_draw.rs` new, 462 with its tests). `node tools/compare_draw_review.cjs --base=dev3` and `--base=dev5`: as above. The first verifier run refused `ui/probe_glyph.svg`: the working copy had been rewritten with Windows line endings at 02:08 (the 1.6.0 commit's time, before this build) while the committed file and its recorded digest are LF; it was written back with the committed bytes (`git show HEAD:probe/ui/probe_glyph.svg`, same blob hash) and the chain ran again. `.svg` files have no `eol=lf` rule in `.gitattributes`, so a later checkout can do this again (not changed here). Not run: the installer (the director installs after review).

## 1.6.0 — 2026-10-25

Release of the performance overhaul and its tidy-up: 1.6.0-perf-dev1 to dev3 and 1.6.0-dev4 and dev5 below, as one DLL for game 0.6.4, 0.6.5 and 0.7.0 Beta 2. The author played the dev5 build on all three versions on 2026-10-25 ("works well"); nothing changed between dev5 and this build apart from the version number and this entry.

- **Seen in game (the author, 2026-10-24 and 25):** the same league match ran at about 96 frames per second against 82 on 1.5.3 (+17 %); the game's own drawing time per frame fell by about 1 ms and the mod's own work per frame roughly halved (656 → about 350 µs). The HUD, strip, panels and screen effect look as they did; the Tab panel's columns and the shop's item order changed on purpose; key caps and tooltips show the player's own bindings. Numbers and method: [docs/investigation-direct-draw-hud.md](docs/investigation-direct-draw-hud.md).
- **What changed, in short (read from the code):** whole-program optimisation for the release build; a typed write cache for every window that still uses the game's UI tree (`ui_writes.rs`); the HUD strip, state plate, session strip, stats panels and screen effect drawn directly with the SDK's draw calls (`draw_list.rs` and the `*_draw.rs` modules), leaving about 20 live tree nodes on a normal frame instead of about 130; the session strip's clicks by the mod's own hit testing; the settings file written off the drawing thread; windows built ahead where the chooser phase allows; key caps and key tooltips that follow the bindings with short names and layout-aware punctuation keys; the Tab panel's columns centred as the game's own scoreboard; the shop's tiers in the game's order (AD, attack speed, armour, magic resistance, AP, HP, then other mods' items); the failed-cast banner texts removed.
- **Hidden switches that ship, default off, in controls.json:** `perf_hide_mod_ui` (hides every mod window, for measuring) and `perf_draw_probe` (the drawing probe); neither changes anything while off. The dev3 entry's open points (equal-z ordering, label defaults) were settled by the author's matches.
- **Known and accepted:** the match-start hitch of about 0.1 s (the shop opening by itself and the HUD's first item frame) is as in 1.5.3 and does not interrupt play (the author's decision); the drawn glyphs are baked PNGs per size and tint, so a glyph whose tint animates stays a tree node; a very wide key chord overflows its cap.
- Tested for this release: formatting, lint with warnings denied, 667 native tests, the source-size check, the verifier for 0.6.4, 0.6.5 and 0.7.0 Beta 2 against their own executables, the pixel comparison of the drawn windows against the dev3 fixture (0 differing pixels in 33 states) and the package check.

## 1.6.0-dev5 (development) — 2026-10-25

**Not seen in game. Development build, never for the Workshop.** The author's finding from playing 1.6.0-dev4 (a binding on the colon/semicolon key showed as `K186`), the three leftovers dev4's entry named, and one behaviour change the author asked for, on top of 1.6.0-dev4. Everything below is read from the code or measured by the tests and tools named; nothing of it is seen in game. **One visible change at the default bindings:** the camera tooltip's second line now says "Hold Spc to follow" (the short name of Space, as every cap uses), and every translation shows the key's short name there instead of its own word for Space (Leertaste, Espacio, 空格键, ...).

### 1. Key names from the keyboard layout (read from the code; measured by tests)

- **What changed.** `Chord::label` (the Settings window) and `Chord::short` (caps and tooltips) now read one table, the new `key_names.rs`, instead of two `match`es in `settings.rs`. The punctuation keys (Windows virtual-key codes 0xBA-0xC0, 0xDB-0xDF, 0xE2) are named by the character the key types on the current keyboard layout: `platform_input::layout_char` calls user32's `MapVirtualKeyW` with `MAPVK_VK_TO_CHAR`, declared like the other user32 imports, and `key_names::punctuation` turns the answer into the name. Without a usable answer (0, a control or space character, not a character at all) the US character is shown: `;` `=` `,` `-` `.` `/` `` ` `` `[` `\` `]` `'`. So the author's key is `;` on a US layout, and whatever it types on another (for example `Ü` on a German one).
- **New names** (full / short): Enter / Ent, Esc / Esc, Caps Lock / Caps, Pause / Paus, Print Screen / PrtSc, Insert / Ins, Delete / Del, Page Up / PgUp, Page Down / PgDn, Num Lock / NumLk, Scroll Lock / ScrLk, Num 0..Num 9 / N0..N9, Num * / N*, Num + / N+, Num - / N-, Num . / N., Num / / N/. All names dev4 had are unchanged.
- **Choices where the brief left two readings:** short names for Pause, Print Screen, Num Lock and Scroll Lock (the brief named none): `Paus`, `PrtSc`, `NumLk`, `ScrLk`. A letter the layout answers is shown upper case, as the letter keys are (`ü` → `Ü`; a character whose upper case is two characters, such as `ß`, stays as it is). A dead key (Windows sets the answer's top bit; for example `^` on a French layout) shows its character. 0xDF and 0xE2 have no US character, so with no layout answer they stay `K223` / `K226` (`Key 223` / `Key 226`); 0x6C (the numeric pad's separator, absent on most keyboards) stays `K108`. The backtick key (0xC0) was a fixed `` ` `` in dev4 and now follows the layout too (it is `Ö` on a German one); the champion-only binding's default is that key, so its Settings row changes on such a layout.
- **When the call runs.** Where a name is made. For the caps and tooltips that is only when a chord changes (`KeyText` rebuilds its text then), so not per frame during play. The Settings window names its keys as it did before: it writes its visible rows every frame while it is open, so there the call runs once a frame per visible punctuation binding (the "Target champions only" row's default is the backtick key, 0xC0). That is one user32 call that reads the layout's table, only while the window is open; it was left as the brief said. In tests the call answers 0 (`#[cfg(all(windows, not(test)))]`), so no test depends on this machine's layout; the tests feed `punctuation` the answers instead.
- **Not done:** a layout switched while the game runs shows on a cap only at its next rebind (the cap does not rebuild on its own); the Settings window shows it the next time it writes its rows.

### 2. The recall, shop and skill caps (read from the code; measured by tests)

- **Recall** (`hud_draw::recall_cap`): grows **left**; its right edge stays at 700 px, 2 px before the recall icon (702 px), which never moves. It stops at the CS number's box end (588 px, 94 px of growth); a wider text overflows the cap. Examples (packaged font advances): `S+B` 667-700 px, `Bksp` 662-700, `C+S+A+F12` 626-700.
- **Shop** (`hud_draw::shop_cap`): grows left while the gold number's box (ending 1508 px) stays uncovered, that is up to 4 px, and the rest to the right as in dev4, past the shop button's frame (1534 px). `S+B`: 1508-1541 px; `W`: 1508-1530. Touching 1508 exactly counts as uncovered.
- **Skills** (`hud_draw::skill_cap`): grows right as in dev4 and is clamped to 74 px, the 80 px slot less the cap's 3 px inset on both sides (the other reading, 77 px to the slot's edge, would cover the slot's border). Only `C+S+A+F12`-sized chords reach it (82 px unclamped); the key text is centred on the cap and overflows it.
- At the default keys every cap is where it was: the HUD write and draw records are unchanged.
- The dev4 test became `caps_name_the_bound_keys_grow_clear_of_their_neighbours_and_hide_when_unbound`: default places, the shop cap 4 px left then right, a 2 px wider key growing left only, the recall cap's right edge fixed at 700 px, the widest chord stopping at 588 px, the unbound recall cap gone, the skill cap clamped to 74 px.

### 3. The skill tooltip's key corner (read from the code)

- The corner shows the skill's binding by its short name (`view.keys.skills[i]`, the same `KeyText` as the skill cap), so a rebind shows there too; an unbound skill shows an empty corner.
- **The title.** `tooltips::skill` builds the title as `"Q   Name"` (the slot letter, then the game's skill name; the letter alone when the game has none), and `player_hud` strips the letter because the corner names the key. The brief asked that the strip use the binding's name too. That would only work if the title were built with the binding's name, and the built text is cached per skill and logged (`HUD TOOLTIP key=... chars=...`), so a rebind would need the cache emptied. Done instead: the letter is one constant, `tooltips::SKILL_LETTERS` (Q, W, R: the skills' names, like `abilities::NAMES`), used by both the builder and the strip, so the strip always matches and the title never names a key; the corner alone names the binding. A skill the game has no name for keeps "Q", "W" or "R" as its title, as its name.
- **Overlap with a wide binding, not fixed (not asked for):** the corner is the template's `key` label, 26 × 26 px at x = tooltip width − 42 px, 21 px medium text, no alignment set (so drawn from its left edge, growing right, away from the title). From the packaged advances: `M4` 30 px and `S+B` 38 px stay inside the tooltip; `PgDn` (53 px) runs about 10 px past the tooltip's right edge and `C+S+A+F12` (110 px) about 68 px.
- No test covers the corner (the scripted HUD frames hover Q at its default key, whose record line `T ingame.lt_player_hud.tooltip.key = Q` is unchanged).

### 4. The camera tooltip (read from the code; measured by tests)

- `"Lock camera · Y\nHold Space to follow"` became `"Lock camera · {key}\nHold {follow} to follow"`, filled from the `camera_toggle` and `follow` bindings' short names (`session_draw::SessionKeys` has two new `KeyText`s, followed every frame like the others; the text is made only while the camera button is hovered). The new `key_cap::tip_lines` fills a text that names a key per line: an unbound key at a line's end loses its " · {name}" part as `tip` does, and a line naming an unbound key elsewhere is dropped. So: camera unbound → "Lock camera" with the follow line; follow unbound → "Lock camera · Y" alone; both unbound → "Lock camera".
- **All 16 tables** changed the same way: `· Y` became `· {key}` and each language's word for Space became `{follow}` (de "Leertaste", es-ES "Espacio", fr "Espace", haw/ja/ko/th/vi "Space", it "Spazio", nl "spatie", pl "spację", pt-BR "Espaço", ru "пробел", tr "Boşluk" (before "tuşunu"), zh-hans "空格键", zh-hant "空白鍵"). Korean keeps its particle 를, which reads right after "Spc" spoken as "space" but not after every key name; the wording is otherwise unchanged. In Japanese the follow key starts its line, so an unbound follow key drops that line.
- The translation test checks every table has the camera text with two lines, `{key}` once at the first line's end after " · ", `{follow}` once on the second, and none has the old text; a new test checks bound, rebound and unbound keys (`the_camera_tooltip_names_both_keys_or_drops_an_unbound_ones_part`).

### 5. No failed-cast tips (read from the code)

- The feedback banner no longer shows "{key} not cast: invalid aim/target or native cast unavailable" or "{key} could not start" (the author: they were for debugging). Chosen: they are not put into the cast state's message at all (`abilities.rs`); in their place the message is cleared, as the refusal's text used to replace the previous one, so an older text (for example "W: Lv 3") does not stay in the banner for the rest of its 1.4 s. Not chosen: making `feedback` skip them, which would have to recognise translated texts by their wording.
- Everything else those paths do stays: the red denied flash (`denied_at`), the denied sound, the `CAST TIMING ... denied` / `refused` and `ABILITY REJECTED` log lines. The other refusal texts stay in the banner (`{key}: Lv {level}`, the cooldown, "{key} unavailable: current ability data not ready", "{key} needs an ally under crowd control"), and so do the aiming hint and the shop's texts. The two texts also leave the verbose `CONTROL DIAGNOSTIC` log line, which read the same message.
- The two texts are removed from all 16 tables (the translation test refuses texts the code no longer uses).
- **Records:** none moved. The brief expected the scripted "refused cast" frames to change, but the HUD script sets the banner's text itself (`ui_identity.rs`, "Not enough gold" and empty) and never runs `abilities.rs`, so no record held either text.

### Records

Rewritten with `LT_UPDATE_UI_RECORDS=1` after the tests showed only `session_strip_writes_match_1_5_3` failing; reviewed line by line. `probe/testdata/ui_identity/session.txt`: 19 lines, all the camera tooltip while it is hovered: its second text line ("Hold Space to follow" → "Hold Spc to follow") and the tooltip box's and text's widths over the 9 frames of its fade-in (365.14 → 342.10 px and 341.14 → 318.1 px, the shorter line). No other line of any write or draw record changed (`hud`, `plate`, `stats`, `effect`, `team` and all `ui_draw` records byte-identical to dev4).

### Tests and tools

- New unit tests: `key_names` (the US fallback table and the keys without one, the layout's answers: a letter, a dead key, 0xE2, unusable answers; every new full and short name), the camera tooltip with bound, rebound and unbound keys (`key_cap`), and the translation test's camera check; `settings`' short-name test gained `;`, `K223`, `Ent`, `C+Del`, `N1` (the old `K186` line is now `;`) and two full names; the HUD cap test as in item 2.
- **Outside the five items:** `tools/compare_ui_records.py` (the dev2 baseline check, not in the brief's list) learned one expected change: the camera tooltip's text line and its box and text widths (`Hold Spc`, 342.10 and 318.1 px, written into the rule, so it needs editing if that tooltip changes again). Without it the check reported exactly these 19 lines as unexpected; nothing else changed in it.
- Pixel test against dev3 (`node tools/compare_draw_review.cjs --base=dev3`, after exporting this build with `LT_UI_EXPORT_DIR=research/ui-pixels/dev3`): 0 differing pixels in all 33 drawn states; the two Tab states 35,800 and 36,143 pixels, the same numbers as dev4 (dev4's intended Tab columns against the dev3 fixture). No review state hovers the camera button, so the tooltip change is not in the pictures.

```
window frame state                                           tree nodes dev3 -> now, draw commands      pixel comparison
hud       5  HUD up, waiting for Start (not playing)          nodes  41 (  1 shown) ->  41 (  1 shown),   3 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
hud       8  control started, no sample yet                   nodes  41 ( 10 shown) ->  41 ( 10 shown),  75 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
hud      14  alive, all skills ready                          nodes  41 ( 14 shown) ->  41 ( 14 shown),  90 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
hud      60  Q and W cooling                                  nodes  41 ( 14 shown) ->  41 ( 14 shown),  94 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
hud     262  cooling with a denied flash, R multi-use refill  nodes  41 ( 15 shown) ->  41 ( 15 shown), 130 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
hud     160  Q hovered (tooltip open)                         nodes  41 ( 25 shown) ->  41 ( 25 shown),  90 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
hud     195  next purchase tile hovered                       nodes  41 ( 17 shown) ->  41 ( 17 shown),  91 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
hud     220  shop button hovered                              nodes  41 ( 17 shown) ->  41 ( 17 shown),  91 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
hud     230  purchase, feedback banner                        nodes  41 ( 15 shown) ->  41 ( 15 shown),  93 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
hud     250  purchase made, level up                          nodes  41 ( 15 shown) ->  41 ( 15 shown),  89 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
hud     290  R multi-use, W aiming                            nodes  41 ( 15 shown) ->  41 ( 15 shown), 122 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
hud     315  low health                                       nodes  41 ( 14 shown) ->  41 ( 14 shown),  86 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
hud     340  dead (respawn count, veils, plate)               nodes  43 ( 16 shown) ->  43 ( 16 shown),  72 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
hud     380  respawned, recalling                             nodes  43 ( 14 shown) ->  43 ( 14 shown),  93 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
hud     400  new background colour                            nodes  43 ( 14 shown) ->  43 ( 14 shown),  86 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
hud     430  paused (plate)                                   nodes   2 (  2 shown) ->   2 (  2 shown),   3 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
hud     450  AI control (plate)                               nodes   2 (  2 shown) ->   2 (  2 shown),   3 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
session   5  session strip, Loading                           nodes  19 (  1 shown) ->  19 (  1 shown),   1 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
session  10  session strip, Ready (chooser)                   nodes  19 (  6 shown) ->  19 (  6 shown),  16 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
session  20  Ready, lane hovered                              nodes  19 (  8 shown) ->  19 (  8 shown),  16 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
session  30  Running                                          nodes  19 (  7 shown) ->  19 (  7 shown),  21 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
session  45  Running, Pause hovered (tooltip)                 nodes  19 (  9 shown) ->  19 (  9 shown),  22 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
session  49  Running, Pause pressed                           nodes  19 (  9 shown) ->  19 (  9 shown),  22 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
session  66  own-team vision hovered                          nodes  19 (  9 shown) ->  19 (  9 shown),  23 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
session 110  swap column open                                 nodes  19 ( 10 shown) ->  19 ( 10 shown),  27 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
session 120  swap column folding                              nodes  19 ( 10 shown) ->  19 ( 10 shown),  27 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
session 135  Paused                                           nodes  19 (  9 shown) ->  19 (  9 shown),  22 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
session 145  AI control                                       nodes  19 (  1 shown) ->  19 (  1 shown),   7 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
session 155  AI control, Take control hovered                 nodes  19 (  3 shown) ->  19 (  3 shown),   7 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
stats    20  stats panels, champion target                    nodes  31 ( 30 shown) ->  31 ( 30 shown),  41 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
stats    70  own panel only (target cleared)                  nodes  14 ( 14 shown) ->  14 ( 14 shown),  15 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
effect   58  low-health tint                                  nodes   0 (  0 shown) ->   0 (  0 shown),  47 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
effect   80  death veil                                       nodes   0 (  0 shown) ->   0 (  0 shown),  53 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
team     20  Tab held open                                    nodes 379 (210 shown) -> 379 (210 shown),   0 drawn  differing px 35800  largest region 810 px 27x30 at (659,326)
team     65  Tab open again, values changed                   nodes 379 (210 shown) -> 379 (210 shown),   0 drawn  differing px 36143  largest region 810 px 27x30 at (659,326)
Largest differing region over all states: 30 px across.
```

### Checks

`cargo fmt`; `cargo clippy --offline --all-targets -- -D warnings` clean; `cargo test --offline --release`: 667 passed, 1 ignored (1.6.0-dev4: 663 and 1; 4 new). `python tools/build_dll.py`: DLL 5,694,976 bytes (dev4: 5,693,440). `python tools/stage_package.py`: 124 files staged, public build. `python tools/verify_build.py --version 1.6.0-dev5 --probe-tests 667`: tables verified for game 0.6.4, 0.6.5 and 0.7.0b2 against their own executables, 625 native anchors (dev4's run reported 619; the count follows the installed game's profile, and the installed copy was not checked here), ABI 6 and null-host rejection; DLL SHA256 d02c9d78…4b0fa7. `python tools/package_mod.py --version 1.6.0-dev5`: 124 matching files, SHA256 385769e5…997c85. (The first chain run stopped at `verify_build.py` because this entry was not written yet; build and stage were not repeated, the verifier and packager ran after it.) `python tools/check_source_size.py` passed (`abilities.rs` 3,136 → 3,140 lines, ceiling 3,200; `player_hud.rs` 2,938 → 2,943, ceiling 3,200). `python tools/compare_ui_records.py`: passed. Not run: the installer (the director installs after review).

## 1.6.0-dev4 (development) — 2026-10-24

**Not seen in game. Development build, never for the Workshop.** Three fixes to look and behaviour the author asked for before a release, and two tidy-ups, on top of 1.6.0-perf-dev3. Everything below is read from the code or measured by the tests and tools named; nothing of it is seen in game. With the default key bindings the HUD, plate, session strip, stats panels and screen effect draw exactly what dev3 drew (their write and draw records are unchanged, and the pixel test shows 0 differing pixels against dev3 in all 33 states); only the Tab panel's look changes on purpose.

### 1. Key caps and tooltips show the real binding (read from the code; measured by tests)

- **What changed.** Every cap and tooltip that had a key typed in now shows the first chord bound to its action (`Values::shown_chord`), read every frame from the applied settings (`settings::current_shared()`, no copy of the settings), so a rebind in Settings shows at once: the HUD's shop (P) and recall (B) caps and the Q, W, R skill caps (`hud_draw::HudKeys`), the session strip's Tab cap and its "Take control · F11" label (`session_draw::SessionKeys`), the state plate's F11 cap (`state_plate.rs`), and the session tooltips "Start", "Pause", "Resume", "Take control", "Return control to AI" and "Team details". Each cap keeps its text in the view (`key_cap::KeyText`) and rebuilds it only when the chord changes (a test checks the buffer is not reallocated); a tooltip's text (the translation lookup and the key) is now made only for the control under the cursor.
- **Short names** (`Chord::short`, `settings.rs`, one table with its test; `Chord::label` stays the Settings window's full name): letters and digits as they are; `F1`..`F24`; `M1`..`M5`; `Tab`, `Spc`, `Home`, `End`, `Bksp`; `Up`, `Dn`, `Lft`, `Rgt`; modifiers as `C+`, `S+`, `A+` in the order Ctrl, Shift, Alt (Shift + B is `S+B`); a key with no name `K<code>`. Two keys the brief did not name: Shift, Ctrl and Alt bound as the key itself keep their names (`Shift`, `Ctrl`, `Alt`), and the backtick stays `` ` `` as in Settings.
- **An unbound action** (both chords cleared) draws no cap, and its tooltip loses its " · key" part ("Start · F11" becomes "Start").
- **Widths.** A cap is today's width at its default key and wider by the extra width of a wider text, rounded up to a whole pixel, so the padding either side stays what the default key had (`key_cap::width`). The padding comes from the widest default text a cap family was drawn for (B for the 18 px shop and recall caps, W for the 22 px skill caps), so no default cap moves. Measured with the packaged font advances (`hud_style::width`); that table is the medium weight only and the caps use the numeric weight, about 2 % wider (Tab at 13 px: 22.9 against 23.3 px), so a grown cap is up to a pixel tighter than the default one. Heights, colours, fonts and positions are as before. Examples (from the same advances): shop or recall `S+B` 33 px, `Bksp` 38 px; skill `M4` 29 px, `S+B` 35 px; Tab `S+Tab` 46 px; plate `S+F11` 53 px.
- **Which way a cap grows** (the brief fixed the first, the rest are choices): the P, B and skill caps grow right. The Tab cap grows **left** together with its button (the button's left edge moves left by the same pixels, its glyph stays where it is, the cap's right edge stays 2 px before the glyph; the strip is empty from 343 to 586 px), so its click and tooltip area grows with it. The state plate **widens by the same pixels and stays centred** under the clock (glyph, gap and padding unchanged; the cap grows right inside it). The "Take control" label keeps its 204 px box while its text fits (the default text is about 141 px) and widens the AI-phase button to the right only when it does not (`Take control · C+S+A+F12` is about 211 px). The label stays English, as the template's was (it was never translated).
- **Overlaps a wide binding causes, as briefed and not fixed:** the B cap ends at 700 px and the recall icon starts at 702 px, so any key wider than B (for example `S+B`, 682-715 px) covers part of the icon; growing left (94 px free up to the CS number) would avoid it. A P cap wider than 22 px passes the shop button's frame (1534 px) and the strip's right corner (1544 px). The widest skill chord (`C+S+A+F12`, about 82 px) runs past its 80 px slot.
- **Translations.** The six texts became format texts with `{key}` ("Start · F11" → "Start · {key}", "Pause · {key}", "Resume · {key}", "Take control · {key}", "Return control to AI · {key}", "Team details · {key}"), in the code and in all 16 tables under `probe/lang/` (each wording keeps its own text, only the key changed). A test checks every table has the six texts, each with one `{key}` at its end after " · ", and that none of the old texts is left. The brief named five tooltips; "Take control · F11" is the sixth, the same text as the label, shown as the primary button's tooltip while the AI drives.
- **Not done (outside the brief's list):** the camera tooltip "Lock camera · Y / Hold Space to follow" still names Y and Space, and the HUD skill tooltip's key corner (`player_hud.rs`, `KEYS`) still shows Q, W or R after a rebind.

### 2. The Tab panel's columns (read from the code; measured by tests and the pixel tool)

- **New layout** (`team_status.rs`, constants `LEVEL`, `KDA`, `CS`, `ITEMS_X`, `GOLD_W`), as the game's own scoreboard (`wide_player_info.ui`) centres K/D/A and CS: LV, K/D/A and CS are centred in their columns with their headers centred over them; the items start at their header's left edge; gold is right-aligned with its header. 16 px between every two neighbouring columns, box edge to box edge, the portrait included: portrait 6-54, LV 70-110, K/D/A 126-236, CS 252-306, items from 322, gold ending 12 px inside the side's edge as before. Before: LV 64 (34 wide), K/D/A 104 (122), CS 234 (54), items 294, gold 100 px from the edge (88 wide), all left-aligned but gold. The K/D/A box is 110 px instead of 122 so the panel keeps its width (1232, 1304 and 1376 px for 4, 5 and 6 slots, as the existing test checks); a test checks that 30, 99/99/99, 999, 99999 and the longest GOLD header fit their boxes with the numeric weight's margin. Row height, portrait, selected bar, row lines, tile count, fonts, sizes, colours and the K/D/A colour runs are unchanged. The items header's box is 140 px (four tiles) instead of 220.
- **Records.** `probe/testdata/ui_identity/team.txt`: 73 lines changed, each one expected: the template line (the label boxes and alignments above), 60 item tile writes (x + 28 px: 294, 330, ... → 322, 358, ...), 10 row gold writes and 2 header gold writes (x 483 → 478 px for 4 slots). No other line of any record changed.
- **Picture.** Neither `tools/check_hud_review.cjs` nor `tools/qa_tab_46.cjs` reads the panel's template (they check the design prototype). The pixel tool now has two Tab states (frames 20 and 65 of the scripted Tab test) against the new dev3 fixture set: 35,800 and 36,143 differing pixels, the intended move; `node tools/compare_draw_review.cjs --base=dev3 --keep team` writes both pictures to `research/ui-pixels/out/`.

### 3. The shop's item order (read from the code; measured by tests)

- `shop_ui::layout` sorted a tier by price, then key, which scattered the item families. It now calls `shop::sort_tier`: items whose sprite tag has the form `t<digits>_<digits>` by the number after the underscore (the game's order: 0 sword, 1 dagger, 2 armour, 3 cloak, 4 crystal, 5 orb), then every other item (another mod's) in registry order (the catalogue's key order); equal numbers keep registry order too. The same in every tier. `shop::Item` carries the item's `icon` string.
- **The live tag** (read from the code): `native_items.rs` reads each registered item's icon string at getter 0x60 into the spec's `icon`, and `hud_icons::item_icon` uses that string unchanged as the tag (`rect_tag`) in the game's item sheet `item_icons_18x18`, the same tags the shipped table `hud_assets.json` lists (`t1_0`..`t5_5`). No live dump of the registry is in `research/` to confirm the values; that the HUD shows the right item icons in game is the indirect evidence.
- **The Riot pack (guess).** Read from its installed files: its `item_setting` lists only the 30 base items, with the same `t1_0`..`t5_5` tags; its own items (about 200) are registered by its DLL, and its item sheet names their images by item name (`bf_sword`, `heartsteel`, ...) beside the 30 base tags. So its own items' live icon strings are probably their names and they sort after the base items in registry order; that is a guess, their live strings were not read. Were one of them tagged `t<n>_<m>`, it would sit among the base items by its number.
- Tests: base items registered out of order; base items mixed with other mods' items (no tag, an own tag, `t1_x`, `riot_t1_2`, `t_3`) at lower and higher prices; untagged items only; a higher tier.

### 4. Tidy-ups

- `draw_list::check_origin` no longer compares against a root that is not laid out yet (zero width or height, which the game reports as 0 × 0 at (0, 0) on the frame the root is spawned; the author's dev3 log showed one DIFFERENT line per window there): it waits for the next frame. A test checks nothing is logged before a real rectangle, then one comparison, and that a later difference is still logged. (Read from the code; measured by a test.)
- `tools/verify_ui_graphics.py` is **retired** (deleted). It was a 0.65.2 generator, not a check: it imports PIL, which is not installed here, asserts 74 images all 32 × 32 and white (the 20 baked `drawn_*.png` glyphs are sized and tinted), and rewrites `tools/records/ui-graphics.json`, dropping the `sound/` and `ui/bundled/` entries the record holds now (its version field still says 0.65.2). `tools/verify_build.py` is the one check of the packaged assets: it asserts the 122 files (the probe SVG and the 20 baked glyphs among them) and every file's digest. `docs/reverse-engineering-guide.md` and `docs/investigation-direct-draw-hud.md` (a dated note beside the old line) say so.

### Tests and tools

- New unit tests: the short key names, a cap's text kept until its chord changes, the default caps' widths and grown widths, tooltips with a bound and an unbound key, every language's six key texts, the HUD's caps rebound and unbound (`hud_draw`), the Tab cap growing left and the "Take control" label (`session_draw`), the plate widening and staying centred (`state_plate`), the Tab column geometry and template (`team_status`), the shop order (`shop`), and the origin check (`draw_list`).
- **Pixel fixtures.** `probe/testdata/ui_pixels/dev3/` (new, 1.9 MB) is dev3's export (write records, templates and draw lists), made before any change of this build; the dev2 set is kept. `tools/compare_draw_review.cjs` learned `--base=<set>` (default dev2): the base side also replays the set's draw lists, so later builds compare against what dev3 drew; a window the base set lacks is skipped (dev2 has no Tab panel); the Tab states were added; labels draw the game's rich-text colour runs (`<#rrggbbaa>`, `<>`), which the renderer had drawn as raw text, so the K/D/A colours show (the HUD and session numbers did not move with it); `--keep` names the pictures `<window>-<frame>-<base>.png` and `-now.png`.
- Pixel test against dev2 (`node tools/compare_draw_review.cjs`): every one of the 33 drawn states has the same numbers as in 1.6.0-perf-dev3 (0-68 differing pixels, largest region 7 px); the two Tab states are skipped. Against dev3 (`--base=dev3`): 0 differing pixels in all 33 drawn states; Tab as above.

### Checks

`cargo fmt`; `cargo clippy --offline --all-targets -- -D warnings` clean; `cargo test --offline --release`: 663 passed, 1 ignored (1.6.0-perf-dev3: 651 and 1; 12 new). `python tools/build_dll.py`: DLL 5,693,440 bytes (dev3: 5,649,408). `python tools/stage_package.py`: 124 files staged, public build. `python tools/verify_build.py --version 1.6.0-dev4 --probe-tests 663`: tables verified for game 0.6.4, 0.6.5 and 0.7.0b2 against their own executables, 619 native anchors (the count of the installed game's profile: 0.7.0 Beta 2 is installed now, dev3's 621 was 0.6.5's), ABI 6 and null-host rejection; DLL SHA256 d7ac50a5…234c34. `python tools/package_mod.py --version 1.6.0-dev4`: 124 matching files, SHA256 0e93cb9a…cffed4. (The chain ran twice: after the first run three edited files were found with Windows line endings and converted back; the second run is the one recorded here, its DLL hash differs from the first run's d6aa4798…3b1391.) `python tools/check_source_size.py` passed (`shop_ui.rs` 3,517 → 3,519 lines, ceiling 3,550). `python tools/compare_ui_records.py`: passed. `node tools/compare_draw_review.cjs` and `--base=dev3`: as above. Not run: the installer (the director installs after review).


**Not seen in game. Development build, never for the Workshop.** Build "2b" of the Stage 2 design in `docs/investigation-direct-draw-hud.md` (sections 3 and 7): the HUD strip, the state plate, the session strip, both stats panels and the screen effect are drawn with the SDK's direct draw calls in `post_render` instead of being nodes of the game's UI tree. The goal is "play identical" with 1.6.0-perf-dev2 and far fewer tree nodes. Everything below is read from the code or measured by the tests named; nothing of it is seen in game. The 1.6.0-perf-dev2 drawing probe (`draw_probe.rs`, `perf_draw_probe`) is unchanged.

### How it is built (read from the code)

- **One model for every drawn window.** The state step of each window is as before (same values every frame, same side effects in the same order: motion pass, tweens, chime, press and release, hover sounds, tooltip timers, blocked rectangles, hovered skill). Where a node used to be written, the window now sets a field of a plain view struct (`hud_draw::HudView`, `session_draw::SessionView`, `plate_draw::PlateView`; the stats panels and the screen effect draw straight from their unit and bands). A field not set in a frame keeps its value, as the node kept its last write, and the defaults are the old template's, so conditional writes behave as before (the shade keeps its height when no sample has arrived, the next purchase's dashes keep their state while `purchase()` skips an unchanged frame, and so on). A pure function per window (`hud_draw.rs`, `plate_draw.rs`, `session_draw.rs`, `stats_draw.rs`, `effect_draw.rs`) turns the view into a draw list (`draw_list.rs`: boxes, texts, sprites; lines and SVGs exist for later use) in the old template's node order and at the old nodes' z values, in `post_update`. `Client::post_render_inner` sends every list right after the drawing probe and before the battlefield drawing (`draw_list::submit`). A list keeps its storage from frame to frame; texts are formatted into the list's own buffer.
- **Positions** come from `ui_node_rect("ingame")`, through the old roots' anchor and pivot (`draw_list::anchored`): the HUD strip 1920 × 146 bottom-centred, the session strip 512 × 80 bottom-centred 62 px up in the chooser and 656 × 56 bottom-left 8 px in on the battlefield, the own stats panel's root 1920 × 150 bottom-centred, the target panel at (8, 56), the plate under the clock. No scaling: the UI map is 1920 × 1080 (as `draw_probe::strip_rect` at scale 1, which the probe confirmed in game). Positions the nodes received to 0.01 px (`{:.2}px`) are rounded the same way.
- **Nothing is drawn while `perf_hide_mod_ui` is on** (`draw_list::submit` returns first), so the A/B still hides every mod window. A window draws only while its root is shown this frame (a root removed for a background or language rebuild draws nothing until it is built again, as before).

### What moved and what stayed, per window (read from the code; node counts measured by the pixel test's tree model)

- **HUD strip** (`player_hud.rs` → `hud_draw.rs`). Drawn: the strip, edge, corners, the health bar with trail, fill and 32 ticks, every skill slot's box, shadow, background, shade, drain edge, comet (A3), the 40 denied/ready bands, the 6 charge marks and fills, key cap and letter, lock glyph, cooldown and uses numbers; the inventory tiles' boxes, backgrounds and "?" marks; the next purchase's box, background, 16 gap strips, progress bar and unavailable glyph; coin, gold, the purchase line, the shop button's tint and P key, the recall key, icon and bar (A2); K/D/A, CS, level, HP and respawn texts; the death veils (B6), the stats hairline (D1) and the feedback banner. **Stays in the tree:** the root; the three skill icons and their PNG twins, the six item icons and the next-purchase icon (sprite-sheet images) inside containers `skill{i}`, `item{i}`, `next` that keep the old tiles' place and visibility and draw nothing; the purchase coin and check (their tint breathes, A1); the tooltip (wrapped, scrolled, rich text). Nodes: 333 → 41 (shown on a normal frame, alive with all skills ready: 97 → 14).
- **State plate** (`state_plate.rs` → `plate_draw.rs`). Drawn: the plate, the F11 key cap and its text, the respawn count. **Stays:** the root (now a container) and the glyph, whose tint fades with the plate (S1). Nodes: 5 → 2.
- **Session strip** (`session_ui.rs` → `session_draw.rs`). Drawn: every button's box and border (the game's `color_icon_button` look: a border `stroke` px wide inside the box, the fill within, 2 px corners), the glyphs of Pause/Play, Watch, AI, Camera, Settings, Tab and Swap, the lane and swap tiles' boxes, the vision track, indicator, hover shades and team marks, the two rules, the Tab key cap, the "Take control · F11" label, the loading hourglass and the swap sweep (B8). **Stays:** the root (its anchoring follows the phase), the lane tiles as containers of the champion portraits (`ui_set_champion_icon`), the three vision buttons as containers of their eye glyphs (their ink tweens), and the tooltip (wrapped text). Nodes: 47 → 19 (shown on the battlefield: 30 → 7).
- **Stats panels** (`stats_ui.rs` → `stats_draw.rs`). Drawn: both frames (edge, shadow, inset fill), the 24 value texts, the target's face box and well, level badge and number, health bar back and fill and its outlined text. **Stays:** the roots and `panel`/`face` containers, the 24 stat icons (frames of the game's stat sheet drawn at 18 px, which `draw_sprite` cannot scale), the portrait, and the unit glyph (the game's tower and jungle icons, of unknown native size; one node). Nodes: 65 → 31 (both shown: 64 → 30).
- **Screen effect** (`screen_effect.rs` → `effect_draw.rs`). All 196 band nodes and the root leave the tree; the same bands are drawn at z 990. It is no longer built ahead (`prewarm.rs`). Nodes: 197 → 0.
- **On a normal battlefield frame** (HUD alive, session strip, no panel): 127 shown tree nodes → 21, with about 111 draw commands; with both stats panels 191 → 51 nodes and about 152 commands. All windows' trees together, hidden nodes included (all are built ahead): 647 → 93. (The design's 350-400 counted differently: every node of the HUD template, hidden or not, plus the session strip and panels.)

### Glyphs (read from the code)

`draw_sprite` draws a PNG at its own size and cannot tint; `draw_svg` fills strokes (seen in game with dev2). No stroke-to-outline converter is available offline in the repository's tool chain (the Node folder has sharp, playwright, pixelmatch and lucide; Python has no imaging library), so: **20 baked PNGs** (`probe/ui/drawn_<glyph>_<size>[_<rrggbb>].png`), each the shipped 32 px glyph scaled to the size its node showed it at with a linear filter (the node's default smooth sampling) and multiplied by the node's tint, made by `node tools/generate_endfield_glyphs.cjs --drawn` from `tools/records/drawn-glyphs.json` (variants and digests; `drawn_glyphs::BAKED` lists the same, a test checks both and the files). The existing 32 px `ef_*.png` are not regenerated (their digests are unchanged). The loading hourglass is the existing 32 px PNG. Glyphs whose tint animates stay image nodes: the purchase coin and check (A1), the vision eyes and the state plate's glyph. New assets go through `tools/records/ui-graphics.json` (102 → 122 files) and the verifier (which also checks `drawn-glyphs.json`); `tools/stage_package.py` refuses to stage when one is missing.

### Clicks and hover on the session strip (read from the code; measured by tests)

The strip's buttons are no longer the game's `color_icon_button` runners, so `ui_register_path_events` is gone. `SessionUi::clicks` hit-tests `keys.cursor` against the buttons' rectangles from the layout that was on screen (last frame's, as the game's click events were): a press that starts on a shown button and a release over the same button is one click, sent through `ui_click::once` (the same 50 ms guard and the same click sound) to the same actions as the old handlers (Start/Pause/Resume, Return to AI and Watch, Camera lock, Settings, Tab, the three vision buttons, Swap, the lane tiles). A hidden button takes no click; where controls overlap the old tree's last node wins (the swap button over a tile unfolding from under it); no click while the Settings window or the shop covers the cursor (the same `covered` rule the hover sound used; both windows take the game's clicks there). The clicks run before the strip's state step, so a click acts in the frame its release is seen, as before; the strip's event lock is taken only by the actions that touch it, as in the old handlers. The hover sound, the tooltip's anchor and `block_native_ui` use the same computed rectangles. The HUD strip's tiles and shop button already hit-tested this way; the shop button now uses the strip root's rectangle and the drawn button's place.

### The two tests

- **Golden draw lists** (`fake_draw.rs`, records in `probe/testdata/ui_draw/`: `hud`, `plate`, `session`, `stats`, `effect`): the scripted frame sequences of `ui_identity.rs` record every window's draw commands per frame through a fake draw table, beside the write records of the nodes that stay (`LT_UPDATE_UI_RECORDS=1` writes both). A frame that draws what the one before drew is saved as `= as before` (the HUD record is still 1.4 MB: its tweens change something most frames).
- **Offline pixel comparison** (`tools/compare_draw_review.cjs`, Edge headless through playwright, one 1920 × 1080 canvas, pixelmatch): before any template changed, dev2's templates and write records were exported as fixtures (`probe/testdata/ui_pixels/dev2/`); dev3's are exported by `LT_UI_EXPORT_DIR=… cargo test ui_identity`. For each review state both sides replay the write record onto a tree model (template, writes, texts, visibility, layout as the fake tree's), dev3 adds the frame's draw list, and both are drawn flat sorted by (z, order) with the mod's fonts and PNGs (sprite-sheet images and portraits as the same placeholder on both sides). Result, 33 states:

| Window | States | Differing pixels per state | Largest region |
|---|---|---|---|
| HUD strip and plate | 17 (waiting for Start, no sample, alive, cooling, denied flash with R multi-use, Q hovered with tooltip, next tile hovered, shop button hovered, feedback banner, purchase and level-up, W aiming, low health, dead with veils and plate, recalling, new background colour, paused, AI control) | 0-68 | 6 px (2 × 4): the 17 px coin glyph's anti-aliasing |
| Session strip | 12 (Loading, Ready, lane hovered, Running, Pause hovered with tooltip, pressed, vision hovered, swap column open and folding, Paused, AI, Take control hovered) | 0-4 | 1 px: the processor glyph's anti-aliasing |
| Stats panels | 2 (champion target with both panels, own panel only) | 0-68 | 7 × 3 px: the outlined health text (stroke against four offset copies) |
| Screen effect | 2 (low-health tint, death veil) | 0 | none |

  Every state, as `node tools/compare_draw_review.cjs` printed it (tree nodes in the replay model, all and shown; draw commands; pixels):

```
window frame state                                           tree nodes dev2 -> dev3, draw commands      pixel comparison
hud       5  HUD up, waiting for Start (not playing)          nodes 333 (  4 shown) ->  41 (  1 shown),   3 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
hud       8  control started, no sample yet                   nodes 333 ( 80 shown) ->  41 ( 10 shown),  75 drawn  differing px    49  largest region 6 px 2x4 at (1444,1037)
hud      14  alive, all skills ready                          nodes 333 ( 97 shown) ->  41 ( 14 shown),  90 drawn  differing px    64  largest region 5 px 4x2 at (1439,1034)
hud      60  Q and W cooling                                  nodes 333 (100 shown) ->  41 ( 14 shown),  94 drawn  differing px    64  largest region 5 px 4x2 at (1439,1034)
hud     262  cooling with a denied flash, R multi-use refill  nodes 333 (142 shown) ->  41 ( 15 shown), 130 drawn  differing px    63  largest region 5 px 4x2 at (1439,1034)
hud     160  Q hovered (tooltip open)                         nodes 333 (108 shown) ->  41 ( 25 shown),  90 drawn  differing px    64  largest region 5 px 4x2 at (1439,1034)
hud     195  next purchase tile hovered                       nodes 333 (100 shown) ->  41 ( 17 shown),  91 drawn  differing px    46  largest region 5 px 3x5 at (1442,1035)
hud     220  shop button hovered                              nodes 333 (100 shown) ->  41 ( 17 shown),  91 drawn  differing px    46  largest region 5 px 3x5 at (1442,1035)
hud     230  purchase, feedback banner                        nodes 333 (100 shown) ->  41 ( 15 shown),  93 drawn  differing px    58  largest region 5 px 3x5 at (1442,1035)
hud     250  purchase made, level up                          nodes 333 ( 97 shown) ->  41 ( 15 shown),  89 drawn  differing px    63  largest region 5 px 4x2 at (1439,1034)
hud     290  R multi-use, W aiming                            nodes 333 (139 shown) ->  41 ( 15 shown), 122 drawn  differing px    63  largest region 5 px 4x2 at (1439,1034)
hud     315  low health                                       nodes 333 ( 93 shown) ->  41 ( 14 shown),  86 drawn  differing px    46  largest region 5 px 4x2 at (1439,1034)
hud     340  dead (respawn count, veils, plate)               nodes 338 ( 90 shown) ->  43 ( 16 shown),  72 drawn  differing px    56  largest region 5 px 4x2 at (1439,1034)
hud     380  respawned, recalling                             nodes 338 (100 shown) ->  43 ( 14 shown),  93 drawn  differing px    68  largest region 5 px 4x2 at (1439,1034)
hud     400  new background colour                            nodes 338 ( 93 shown) ->  43 ( 14 shown),  86 drawn  differing px    63  largest region 5 px 4x2 at (1439,1034)
hud     430  paused (plate)                                   nodes   5 (  4 shown) ->   2 (  2 shown),   3 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
hud     450  AI control (plate)                               nodes   5 (  4 shown) ->   2 (  2 shown),   3 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
session   5  session strip, Loading                           nodes  47 (  2 shown) ->  19 (  1 shown),   1 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
session  10  session strip, Ready (chooser)                   nodes  47 ( 10 shown) ->  19 (  6 shown),  16 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
session  20  Ready, lane hovered                              nodes  47 ( 12 shown) ->  19 (  8 shown),  16 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
session  30  Running                                          nodes  47 ( 30 shown) ->  19 (  7 shown),  21 drawn  differing px     4  largest region 1 px 1x1 at (69,1043)
session  45  Running, Pause hovered (tooltip)                 nodes  47 ( 32 shown) ->  19 (  9 shown),  22 drawn  differing px     4  largest region 1 px 1x1 at (69,1043)
session  49  Running, Pause pressed                           nodes  47 ( 32 shown) ->  19 (  9 shown),  22 drawn  differing px     4  largest region 1 px 1x1 at (69,1043)
session  66  own-team vision hovered                          nodes  47 ( 32 shown) ->  19 (  9 shown),  23 drawn  differing px     4  largest region 1 px 1x1 at (69,1043)
session 110  swap column open                                 nodes  47 ( 33 shown) ->  19 ( 10 shown),  27 drawn  differing px     4  largest region 1 px 1x1 at (69,1043)
session 120  swap column folding                              nodes  47 ( 33 shown) ->  19 ( 10 shown),  27 drawn  differing px     4  largest region 1 px 1x1 at (69,1043)
session 135  Paused                                           nodes  47 ( 32 shown) ->  19 (  9 shown),  22 drawn  differing px     4  largest region 1 px 1x1 at (69,1043)
session 145  AI control                                       nodes  47 (  6 shown) ->  19 (  1 shown),   7 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
session 155  AI control, Take control hovered                 nodes  47 (  8 shown) ->  19 (  3 shown),   7 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
stats    20  stats panels, champion target                    nodes  65 ( 64 shown) ->  31 ( 30 shown),  41 drawn  differing px    68  largest region 8 px 7x3 at (127,78)
stats    70  own panel only (target cleared)                  nodes  28 ( 28 shown) ->  14 ( 14 shown),  15 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
effect   58  low-health tint                                  nodes 197 ( 48 shown) ->   0 (  0 shown),  47 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
effect   80  death veil                                       nodes 197 ( 54 shown) ->   0 (  0 shown),  53 drawn  differing px     0  largest region 0 px 0x0 at (0,0)
Largest differing region over all states: 7 px across.
```

  Largest differing region over all states: 7 px across. The renderer's own first version erased the area under a see-through `color_icon_button` (the vision buttons); that was a bug of the tool, fixed before these numbers.
- **The write records of the nodes that stay** (`probe/testdata/ui_identity/`) keep passing; `python tools/compare_ui_records.py` checks, frame by frame, that every line still sent to a kept node is the dev2 line, apart from these expected changes:
  - HUD (4,648 → 1,160 lines; 1,154 kept lines identical): 3,489 lines left with their nodes (every drawn element's writes: bands, shades, labels, ticks, veils...); the containers `skill{i}`, `item{i}`, `next` lost their colour writes; the template line changed; the plate root is written only when its width changes (its colour carried the fade) and now gets one visibility write when it first shows (the fake tree read the old plate template's first line, which held a `#~` colour before `visible: false`, as visible; the game did not, so dev2 sent that write in game too).
  - Session strip (1,071 → 482; 359 identical): 589 lines left (the drawn buttons, glyphs, rules, track, indicator, key cap, label, hourglass, sweep, and the 15 click registrations); the lane and vision containers' writes lost their `ignore_event` and `btn` parts, and a lane container is written only when its place or visibility changes.
  - Stats panels (216 → 101; 98 identical): the value, level, health fill and text writes left.
  - Screen effect (1,361 → 0): the whole window left the tree.

### The test rig, made again first (measured by tests)

Before the windows changed, the fake UI tree learned node geometry: `fake_layout.rs` lays nodes out from their templates and writes (anchor, pivot, px and %) instead of answering the whole screen for every node; the cursor points of the scripts sit on the real controls (the shop button at its real place, the chooser's lane 0 in Ready); the session record notes hover and click sound requests. The write records were then made again on the unchanged dev2 windows and kept as the baseline (they differ from the 1.5.3 records only where geometry decided something: blocked rectangles, the shop button's hover tint, the chooser's lane tooltip). Two artefacts of the test clock stay: after the background and language rebuilds the HUD and the stats panels are missing for a second in the scripts (their next build is not due yet), so the pixel test has no "paused" or "AI control" state with the strip shown and no tower target. The scripts never reach a ready purchase either (the gold stays under the next item's price), so the yellow next tile without its dashes is covered by a unit test of the draw list (`hud_draw::tests`), not by the pixel test.

### Choices made where the brief allowed two readings

- Tooltips stay in the tree, including the session strip's (the brief lists it; the design's section 3 had moved its box and text).
- The stat icons and the target's unit glyph stay (sprite-sheet frames scaled to 18 px; game textures of unknown size).
- Positions from `ingame` without scaling, as the tree places the roots (identical to `strip_rect` at 1920 × 1080).
- Session clicks are tested against last frame's layout and handled before the strip's state step, so they act in the same frame as the old callbacks did.
- Containers kept for the sprite-sheet icons, portraits and eyes so their paths, write records and the code that writes them stay as they were.

### Not seen in game: what to look at

- **Equal z.** Commands of equal z are sent in the old template's order; the probe did not measure whether the game keeps that order. Where it would show: the drain edge, key cap, cooldown number and comet (all z 1005 on a skill slot), the slot shade and death veil (1004), the feedback banner text (1113) beside tooltip nodes (1112). The outlined HP numbers draw their dark copies at the text's own z.
- **The button look:** the drawn `color_icon_button` puts its border inside the box with 2 px corners; the game's runner may draw its stroke differently by a pixel.
- **Label defaults:** the feedback banner's text set no alignment, so it is drawn left and top (a guess from the game's own templates, which spell out every centred alignment); the "?" on an unknown item and the banner use the label's default colour white (a guess). The "Take control · F11" label is drawn with the English text the template had (as before: the template text was never translated).
- **Drawn place against the tree's:** the drawing assumes the roots' anchoring (`draw_list::anchored`) and the tests' fake tree lays nodes out the same way, so neither test can catch a difference in the game; hover and clicks use the game's own root rectangle. Each drawn window writes one `DRAW origin <window>: drawn at (x, y), tree root at (x, y) ...: same` line the first time it shows (and one `DIFFERENT` line if they ever differ by more than half a pixel): the first match's log settles it.
- **Sprites do not scale:** if the game's `ingame` root were not 1920 wide, boxes and texts would follow it and the glyph PNGs would not (the probe showed 1920 × 1080).
- **Glyph sharpness:** the baked glyphs are scaled down once with a linear filter; the tree scaled the 32 px PNGs on the GPU. Expect at most a one-pixel softness difference (the pixel test sees 4-6 px regions on the coin and processor glyphs).
- **Cost:** about 111-152 draw commands a frame against 127-191 shown nodes. At the probe's 2.1-2.5 µs a command that is roughly 0.25-0.35 ms; the A/B with `perf_hide_mod_ui` and Frame Lab will tell the real gain.

### Tried and not done

- Converting the glyph SVGs to filled outlines for `draw_svg` (tints could then animate): no offline stroke-to-path tool in the repository's tool chain.
- Re-recording the dev2 baseline with the stats and language frames rearranged to cover a tower target and a paused strip: the dev2 window code is only in git now, and the scripts were left as they were.
- A PERF section for the submit: its cost lands in the unchanged `post_render` section.

### Checks

`cargo fmt`; `cargo clippy --offline --all-targets -- -D warnings` clean; `cargo test --offline --release`: 651 passed, 1 ignored (1.6.0-perf-dev2: 643 and 1; new: the fake tree's layout, the fake draw recorder, two draw-list tests, the baked glyph table, the ready next tile, and two session click tests; the HUD's steady-frame test now expects at least 20 typed writes a frame instead of 100, since the drawn elements are values, not writes). `python tools/build_dll.py`: DLL 5,649,408 bytes (dev2: 5,769,728). `python tools/stage_package.py`: 124 files staged (dev2: 104), public build. `python tools/verify_build.py --version 1.6.0-perf-dev3 --probe-tests 651`: tables verified for game 0.6.4, 0.6.5 and 0.7.0b2 against their own executables, 621 native anchors, ABI 6 and null-host rejection; DLL SHA256 3bcd58cf…a673a1. `python tools/package_mod.py --version 1.6.0-perf-dev3`: 124 matching files, SHA256 d0af2e34…bbc5775. `python tools/check_source_size.py` passed (`player_hud.rs` 3,178 → 2,936 lines). `python tools/compare_ui_records.py`: passed. `node tools/compare_draw_review.cjs`: the table above. Not run: the installer (the director installs after review), `tools/verify_ui_graphics.py` (stale since dev2: it asserts older counts and rewrites `ui-graphics.json`).

## 1.6.0-perf-dev2 (development) — 2026-10-24

**Not seen in game. Development build, never for the Workshop.** The only change from 1.6.0-perf-dev1 is a hidden drawing probe (`probe/src/draw_probe.rs`), build "2a" of the Stage 2 design in `docs/investigation-direct-draw-hud.md`. It measures, in one match, four things the code cannot tell: what a direct draw command costs the game, how direct draws layer against the game's UI tree, whether `draw_svg` loads an SVG from the mod's folder, and how drawn glyphs and numbers look beside tree ones. Everything below is read from the code; nothing of it is seen in game.

### The switch

`perf_draw_probe` in controls.json (1 or true = on; no control in the Settings window; default off; a Settings save keeps it, as it keeps `perf_hide_mod_ui`). Read once a frame; one line `PERF perf_draw_probe=true: ...` or `=false: ...` when it changes. With it off, the probe's drawing function returns at its first check: no draw, no lock, no log line, so the build behaves like 1.6.0-perf-dev1. With it on, it draws only on the battlefield (`InGame`), in `post_render`, in the "UI" map (1920 × 1080), before the mod's other drawing; its cost lands in the PERF `post_render` section, whose name is unchanged.

### 1. The load: what a direct draw costs

Every 20 seconds the probe moves to the next level of the cycle 0, 1, 2, 4, 8, then back to 0 (a full cycle is 100 s), starting at level 0 each time you enter a battlefield or switch the probe on. Each change writes `PERF PROBE level=k rects=N texts=M`. Level k draws k × 100 boxes and k × 20 texts:

- Boxes: 6 × 6 px, grey `#9a9a9a` with an alpha pulsing between 0x60 and 0xf9 every 32 frames, z 1000, in a 16-column grid at a 7 px pitch from (1652, 104): at level 8, 50 rows, x 1652-1763, y 104-453.
- Texts: the frame counter modulo 1000 as three digits (formatted once a frame into a fixed buffer), numeric font, size 14, `#eeecec`, left-aligned in 28 × 13 cells, 5 columns from (1772, 104), z 1001: at level 8, 32 rows, x 1772-1912, y 104-520.

Both stay in the top-right quarter, under the game's 50 px header and the input-delay readout (1702-1902 × 56-92), right of the shop (ends x 1540) and the Settings window (ends x 1640) (a test checks this at level 8).

**How to read it.** 20 s per level means every level covers at least one full Frame Lab window and one full PERF window (both 10 s); the window that straddles a change mixes two levels, so use the windows that start after a `PERF PROBE` line. Per level, compare Frame Lab's fps and `other_ms_p50` (the game's own time per frame) and PERF's `post_render` average. The mod's own cost per command is the rise of `post_render` from level 0 to level 8, divided by 960 commands; the game's cost per command is the rise of `other_ms_p50` divided by the same. Compare per element: the game spends about 1.5 ms a frame on the mod's 350-400 tree nodes (seen in game, 2026-10-24), about 4 µs a node. A direct command cheaper than about 4 µs (a rise of less than about 3.8 ms from level 0 to level 8) makes a drawn element cheaper than a node and Stage 2 pays; a command costing about as much as a node or more means it does not. Every level draws boxes and texts in the same 5:1 ratio, so the probe gives one blended cost per command: it cannot separate a box's cost from a text's.

### 2. Layering: do direct draws sort with tree nodes by z?

Four 40 × 40 boxes with a 2 px white border, at every level while on:

- **A** (blue), z 1510, at (200, 122): inside the shop window (`shop_ui::WINDOW`, 180, 78, 1360 × 872), 20 px in from its left edge and across the bottom edge of the shop's opaque header band (`top`, window y 1-68, z 1521): the top 24 px under the band, the bottom 16 px over the window's fill (z 1501), above the left tab rail's indicator (from window y 84). z 1510 is over the fill and the grid layer (z 1505-1509) and under the header. Open the shop (P). Box whole, on top of the header: direct draws land above the whole tree. Only its bottom 16 px show, below the header's edge: they sort by z with the tree. Box gone: they land below the tree. **Changed from the brief**, which asked for z 1520 at 20 px in from the corner, (200, 98): that spot lies wholly under the header band (z 1521) and next to the shop's grid masks at exactly z 1520, so "sorted" and "below the tree" would both look hidden; the brief's own rule (over the background, under the content) is kept.
- **B** (red), z 999, and **C** (green), z 1001, over the HUD strip at strip x 702 and 760, strip y 60 (screen 702 and 760, y 994-1034 at 1920 × 1080): the top 30 px over the battlefield, the bottom 10 px over the strip's opaque band `#strip` (strip y 90-146, z 1000). They sit between the recall key (ends x 700) and the skill slots (from x 832), clear of the session strip (x 8-664). **Changed from the brief**, which put them at x 300 and 350, 40 px above the bottom edge: there the session strip's vision track (screen x 208-344, y 1030-1074, z 1101) covers B, a 1 px divider (z 1101) and the K/D/A icon (z 1003) sit under C, and above y 1024 the HUD strip has no opaque node at all (its root is empty), so the boxes would not straddle one known node's z. Straddling the strip's top edge gives each box its own reference: both whole → direct draws above the tree; B's bottom 10 px cut off, C whole → they sort by z; both cut off → below the tree. (The own stats panel, x 612-820, y 932-1024, z 1001+, also covers them while it is shown; keep it closed for this reading.)
- **D** (yellow), z 1998, at (940, 520), in the middle of the Settings window (280, 115, 1360 × 850). Every Settings node is z 1999-2090, so D sits just under the window's backdrop: hidden behind an open Settings window → direct draws sort by z with tree nodes (or land below the tree; B and C tell those apart); shown over it → they land above the whole tree. (The coder first placed D at z 2100, above every Settings node, where only "below the tree" could hide it; the director moved it to 1998 before the build so the Settings window gives a second sorted-or-above reading beside the shop's.)

### 3. Images: do SVGs load, and how do sprites look?

At every level while on, top left on a dark backing box (320, 58, 149 × 44, z 1000, rounded 4): `draw_sprite` of `asset/lt_direct_control/ui/ef_coin` at its own 32 px at (326, 64), nearest sampling; `draw_svg` of the new asset `asset/lt_direct_control/ui/probe_glyph` stretched to 32 × 32 at (366, 64) and at 17 × 17 at (406, 71.5), tinted white; the same PNG again with `draw_sprite` at (431, 64) with `sample_nearest` false. All at z 1001. The backing is right of the stats target panel (x 8-300, y 56-185) and above the drawn notices (from y 110); the shop's top edge (y 78) opens over it.

- Both SVGs show: the game loads a mod SVG asset and scales it, so Stage 2 can draw glyphs at any size from one file. Nothing shows in their place: the asset loader does not take `.svg` from a mod folder, or does not find it by the path without extension (the way PNG assets are named), or the renderer does not draw this SVG (it is stroke-only, no fill); Stage 2 then ships one PNG per size.
- The sprites' position is their top-left corner (pivot 0, 0; the SDK calls the pivot "the normalized anchor the x/y position refers to"). A sprite 16 px up and left of the backing's slot would mean the position is its centre. Compare the two sprites for sharpness against the 32 px tree glyphs in the HUD.

**The new asset:** `probe/ui/probe_glyph.svg`, the Lucide `coins` source the coin glyph is made from (`design/hud/endfield-assets/icons/coins.svg`), turned into a white mask exactly as `tools/generate_endfield_glyphs.cjs` does before rasterising (`currentColor` → white, square caps, mitre joins), with its licence line kept. `tools/stage_package.py` copies all of `probe/ui`, so it is staged; it is listed with its SHA-256 in `tools/records/ui-graphics.json`, which the verifier, `package_mod.py` and the installer check; the asset count goes from 101 to 102 (`verify_build.py`).

### 4. Text: a drawn number beside the level label

Two "88"s in the numeric font at size 24, centred in 40 × 36 boxes like the HUD's level label (strip (730, 100, 40, 36), z 1005), positioned from `ui_node_rect("ingame")` the way the strip is anchored (1920 × 146, bottom-centred): one plain, white, z 1010, exactly 100 px right of the label (screen 830, 1034 at 1920 × 1080); one 50 px further (880, 1034) as four 1 px offset copies in `#0e0d0c` (z 1009, the HUD's outline colour) under a white copy (z 1010), to show an outline built from offsets (Stage 2's replacement for `outline: 1`). Both sit over the left part of the health bar (strip x 780-1140, y 106-130), clear of the HP numbers (from x 944). Compare their size, weight and baseline with the level number.

### Checks

Formatting, lint with warnings denied, 643 tests passed and 1 ignored (1.6.0-perf-dev1: 637 and 1; new: 6 in `draw_probe.rs`: the level schedule, the load at level 8 inside its quarter and off the mod's windows, the image and layering positions, the "88" placement, nothing drawn unless on, in game and drawing, the digits), the release build, the source size check, and the verifier for 0.6.4, 0.6.5 and 0.7.0 Beta 2.

## 1.6.0-perf-dev1 (development) — 2026-10-24

**Not seen in game. Development build, never for the Workshop.** Stage 1 of the performance overhaul (the author, 2026-10-24): the player must see and feel exactly what 1.5.3 gives, and the goal is less of the mod's own work per frame. That the work goes down is read from the code and measured by tests below; the author's next PERF capture is what will show it in game. Nothing was added to or removed from what the player sees.

### Why

Seen in game with 1.5.3 on 0.6.5 (2026-10-24, five-minute match, PERF capture and Frame Lab): the mod's three callbacks took about 656 µs a frame, 584 of them in `post_update` (HUD 250, session strip 121). Read from the code: the HUD rebuilt about 100 property texts every frame and checked each through a cache keyed by freshly formatted strings; the frame's HUD sample was copied twice; closed windows sent a hide every frame; several windows were built from their templates the first time they opened mid-match; the release build used Cargo's default profile.

### What changed (read from the code; measured by tests)

- **Build profile:** `[profile.release]` with `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, and `panic = "unwind"` stated with the reason (every hook and the shop window run inside `catch_unwind`). DLL size (`tools/build_dll.py`): 5,520,384 bytes for 1.5.3, 5,762,560 bytes now (+4.4 %); the verifier finds no build-machine folder in it.
- **Indexed write cache** (`ui_writes.rs`, `PropCache`): every window's property and text cache. Each node path has a slot found once and then addressed by index (`NodeId`), instead of `format!`-built keys (`field:{path}:{key}`, `whole:{path}`, `text:{node}`) on every call. The 1.5.3 rules are kept: fields compared after theming, the two whole-text slots, a refused write stores nothing. 1.5.3's cache is kept only in a test module (`ui_writes::legacy`); a randomized test runs 39 sequences of 3,000 writes, refusals, texts, clears, forgets and background changes through both and finds the same sends and answers.
- **Typed writes** for every window updated each frame: the HUD strip (`player_hud.rs`, node table in `hud_nodes.rs`), the state plate (moved to `state_plate.rs`), the session strip, both stats panels, the screen effect, Tab (`team_status.rs`) and the motion pass items A2, A3, B6, D1 and B8 (now typed values with their own write; their tests read them through `sources()`). A write site passes the plain values its text is made of; while they, the node, the background and the language are unchanged since its last accepted write, nothing is formatted or compared. An accepted write to the node from another site makes the others check again, which keeps nodes with two sources (a skill slot's visibility and its border) exactly as 1.5.3 left them. Hover and press tweens, the motion pass, the chime, press and release, hover sounds and tooltip timers still run every frame, in the same order. The tooltip while shown and the next-purchase block (written only when its inputs change) keep plain writes. Measured by a test (`steady_hud_frames_format_almost_nothing`): on 60 steady playing frames the HUD makes 9,600 typed writes (160 a frame) and formats none of them, and sends the game nothing; 1.5.3 formatted and looked up every one.
- **Identity tests** (`ui_identity.rs`, on a fake UI tree in `fake_ui.rs` with a stopped clock, `clock.rs`): fixed frame sequences through the HUD strip with the recall bar, state plate, blocked rectangles and hovered skill (control start, artwork before the first sample, cooldowns ticking to the chime, hover and press on a skill, the next purchase, the shop button click, a purchase and level-up with a refused-gold message, a refused cast, a multi-use R refilling, aiming, low health, death with the respawn count, a recall, a background colour change and a language change with their rebuilds, pause, AI control, a refusing host), the session strip (Loading, the chooser, every button hovered and pressed, the swap column, pause, AI, release), Tab, the stats panels (with a rebuild) and the screen effect (tint, breathing, death veil). Each compares every write with the record saved from the 1.5.3 code (`probe/testdata/ui_identity/`, 9,556 lines); all five match. A visibility write that changes nothing is left out of the record, since 1.6.0 stops sending those.
- **Copies and pointless writes:** the HUD sample is shared (`Arc<Snapshot>`) instead of copied twice a frame; the strip's rectangles, the camera's blocked lists and the session strip's swap rectangles reuse their storage; your team's players are read once a frame instead of twice; the stats panel reads the shared settings instead of copying them; the next-purchase check compares its inputs before copying them (`shop::Shop::manual_queue_is`); tweens and motion pass items no longer make a new key string every frame. Closed Settings, shop and screen effect windows hide their root only when the game shows it (`ui_root::hide`, `ui_root::show`); shown, they no longer re-send "visible" every frame. `ui_theme::refresh_all` does its eight lookups only when the background or language changed since the last frame.
- **Settings saves off the drawing thread** (`settings_writer.rs`): a change is still taken on the client thread 300 ms after it was made, then written by one writer thread (the newest queued values win; a lock keeps any two writes apart). Apply in the Settings window and the mod's shutdown still save synchronously, after waiting for any queued save. A failed save is tried again 5 s later, as before; its log line comes a frame later.
- **Built ahead** (`prewarm.rs`): the stats panels, state plate, screen effect, Tab, shop, Settings and emote wheel windows are built hidden while you choose your athlete (session phase Ready, after the HUD strip exists, which it does in Ready), all in the first such frame, each at most once a second, through each window's own build code, so their first opening sends what 1.5.3 sent. All in one frame because Ready can be short: about 90 ms between the lock and Start in the author's log of 2026-10-24 (read from the log). Tooltips need nothing: they are nodes of the HUD, session strip and shop templates. Clicks still register on the first opening (`ui_click::once` unchanged).
- **Fonts warmed:** once per game session, the first drawing frame of a pre-match scene draws a short text in the mod's two fonts (numeric, medium) and the game's regular font its drawn notices use, at five sizes, 4,000 px left of the screen. Guess: this rasterises their glyphs before the match; whether the game draws text that far off screen at all is not known.
- **Hidden switch `perf_hide_mod_ui`** (controls.json only, default off; a `PERF perf_hide_mod_ui=...` line when it changes): every mod window stays hidden while state, sounds, input and battlefield drawing continue, for an A/B of what the game spends on the mod's UI nodes. While on, the HUD's, session strip's and stats panels' rectangles stop blocking clicks, and an open Settings window still pauses the match, invisibly.
- **The 125 ms hitch after closing Settings:** read from the logs of 2026-10-24, it is the frame of Apply and close (`SETTINGS saved controls.json` and `SETTINGS close` in the same millisecond). In that window the mod's own `post_update` was at most 17.6 ms (`settings` 16.2 ms, which includes the synchronous save the rules keep); the rest of the 124.8 ms was under the viewer hook (the game's own update, which Frame Lab counts as the mod's) and in DirectWrite. Guess: the resumed match's first update and text drawing. Nothing in `close` or the Resume path was changed beyond the background save and the pre-built windows.

### Tried and not done

- 1.5.3's window code could not run beside the new code in the tests without shipping both, so the "1.5.3 path" of the identity tests is the record it produced, saved before any window code changed (the clock seam was the only change then; it does nothing in the game build).
- Your team's player list is still copied once a frame (taking it under the movement lock would mean calling the game while holding a lock).
- The Settings and shop entrance (S6) still builds its two small texts every frame, also while closed (a few short allocations; left as it was).
- The HUD-motion Off setting is not in the identity records (the tests run with it On, the default); with it Off the same write sites run with the motion values at rest (read from the code).
- Settings, shop and emote windows are covered by the cache-equivalence test only, not by frame records.

### Checks

Formatting, lint with warnings denied, 637 tests passed and 1 ignored (1.5.3: 623 and 1; new: identity 6, write cache 7 of which 4 are the 1.5.3 cache tests moved from `hud_motion.rs`, clock 1, fake UI 1, settings writer 1, colour mix 1, state plate 1), the release build, the source size check (player_hud.rs 3,047 of 3,200 lines; shop_ui.rs 3,517 of 3,550), and the verifier for 0.6.4, 0.6.5 and 0.7.0 Beta 2.

## Source size check (tool and documents only) — 2026-10-23

**No mod build.** A brief code review (2026-10-23) found the mod healthy (format, lint with warnings denied and 624 tests pass; the movement hooks guarded and filtered; the verifier keeps addresses out of shared code) but five source files over 3,000 lines, with 200 to 320 line functions at the movement and input plug-in points where a WASD mode would go. The author declined a refactor on the condition that the files stop growing.

- **Rule:** docs/modding.md "Keep files small": a new feature is its own module with its own tests; an existing file takes only the wiring; a long function you must change gives up the touched part to a helper.
- **Check:** `tools/check_source_size.py` refuses a `.rs` file under `probe/src` over 3,000 lines, or over its own ceiling for the five files already past it (their size that day plus a little room for wiring: movement 4,150, skill preview 3,700, shop UI 3,550, player HUD 3,200, abilities 3,200); `tools/verify_build.py` runs it after the preview table check. Tests: `tools/test_check_source_size.py` (3).
- **Recorded:** docs/backlog.md (Declined) and a superseded note in docs/codebase-review-66.md section 4.

## Skill preview viewer, cast and what it hits (tool only) — 2026-10-22

**Not used by the author yet. Tool only: no mod build.** The author's reorganisation of 2026-10-22: a skill has two parts, how it is cast and what it hits.

- **Cast** (first on every card): how the game aims the skill, in words ("Cast on a unit · on enemies · range 30"), from the game and not editable, since it decides how the mod aims. A skill cast on a unit says that the game marks the target and that "aim" means the target unit.
- **What it hits:** what the game reads, the author's pieces, or nothing (no preview, no range ring). The old preview types (unit target, around self, circle at aim, line, fan, cone, dash or blink, jump back, attack range ring), the "Add to game preview / Replace" choice and the size-source rows are gone; every change is a piece, with its sources in the Pieces panel. Pieces start as a copy of what the card draws (an unreadable skill: the viewer's suggestion).
- **Old saved previews become pieces when the viewer opens** (`legacyPieces`, `convertLegacy`), in memory until Save; a banner counts them. A line's or fan's full width becomes half on each side; named sizes keep their numbers (the author's picks included); an old preview added to what the game reads becomes both; a dash the old rule hid (a unit cast, or one beside an area: the mod's `main_purpose`) is left out, since pieces are drawn as listed. A preview whose champion is not loaded is kept as saved and still works (the mod reads both forms).
- **Numbers:** the game's recorded answers by name come first (they include balance patches), then the champion's file. A name a skill uses more than once can be chosen: the viewer shows the value the game answered in a recorded match, or says it is unknown until a match records it.
- **Range ring of a skill aimed in a direction** (the author's choice A): the ring shows how far the skill reaches, the longest line, shot or fan from the champion or a dash of a fixed length, never less than the cast range, labelled "reach 180 (cast range 100)" (Tale Q). The mod does not draw this yet: it comes with the next build.
- **The two parts lead** (the author, 2026-10-22: "the cast type should be the most important thing about the skill, along with what it hits"): every card starts with **Cast** and **Hits** under the skill's name, in larger text; Hits says the pieces in words and where they come from (what the game reads, your pieces, the champion file, not readable yet). The old type label and the grey line of cast and range under the drawing are gone. The gear button and the card's **Edit** button open the **Edit** panel (was Pieces): "1 · Cast" (from the game, not editable) and "2 · What it hits" (the same three-way choice, then the pieces).
- **Filters follow the two parts:** **Cast** (on a unit, at a point, in a direction, no aim, passive) and **Hits** (circle, line, shot, rectangle, cone, fan of lines, dash, blink, jump back, attack range ring, nothing or the target only, not readable yet; a skill counts under every kind it draws) replace the Shape filter.
- Checked (read from the tools, not seen by the author): on the author's data, all 37 saved old previews convert to pieces that draw the same shapes, 31 sizes still named; the table a Save would write changes only those 37 skills from `assigned` to `custom`, with the same shapes. Tests: 86 viewer checks, 65 Python tool tests. In a browser on a copy of the author's data: the banner, the two parts on the cards and in the Edit panel, the Cast and Hits filters, Tale Q's reach ring, and a Save; the author's files and the shipped table were left as they were.

## 1.5.3 — 2026-10-23

Release of 1.5.3-dev1 below, rounded fan tips (this entry) and the 1.5.2 release build, which was built and played but never uploaded: the last public build was 1.5.1, so this release carries everything in "1.5.2" below as well. One DLL for game 0.6.4, 0.6.5 and 0.7.0 Beta 2. Built straight as a release at the author's request (2026-10-23: "straight up build 1.5.3 ... I'll release on workshop after rough test"), including the author's viewer edits of the day.

- **Rounded fan tips** (the author's go-ahead, 2026-10-23): each line of a fan of projectiles ends in a half circle its width past the end, as a projectile reaches (read from the code, docs/investigation-line-hit.md "Projectiles"). `Shape::Fan` and `Shape::Spread` gain `rounded`; the readers' fans (Dancer R) are rounded (`Geometry::add_projectile`), the shipped Dancer R gained `"rounded": true` (`preview_sources.rounded_projectiles`, nothing else changed), a new Fan of lines or Blades piece starts rounded, and the author's saved fans keep flat tips (Boomerang Hunter W and others: tick Rounded ends to change them). Drawn in the fan's own strip style (`End::Round`); fans keep their look otherwise. Tests: the strip test's half circle, two viewer checks.
- **The author's viewer edits** in the shipped table (111 champions, 15 of them from data files).
- Tested for this release: formatting, lint with warnings denied for both builds, native tests in the public and research builds, Python tool tests and viewer checks, the verifier for 0.6.4, 0.6.5 and 0.7.0 Beta 2 with the preview table up to date and no build-machine folder in the DLL, the public package, installed. Not seen in game: the Aurum lane (1.5.3-dev1) and the rounded fan tips; seen in game on Beta 2 with the 1.5.2 build: rounded ends on line hits and projectiles.

## 1.5.3-dev1 (development build) — 2026-10-23

**Not seen in game. Development build, never for the Workshop.** Single line skills are drawn in the author's design "Aurum" (`design/previews/skillshot-aurum.svg`, local, not in git). The 1.5.2 release, tested in game but not yet uploaded, is kept as built in `local/release/source-1.5.2/` (not in git).

### Why (the author, 2026-10-23)

After rounded ends, the author disliked the half circle under the champion and had the skillshot redesigned: "it looks cool without being too distracting". The author's choices: the mod's colours (yellow, ally blue, grey when not ready) instead of the design's gold; every single line gets it, fans keep their look; no different look for shots that pass through or stop (as in League), and no hit marks on enemies for now.

### Now (read from the code; tests; seen in the viewer on a copy of the author's data)

- **The lane** (`skill_preview::lane`, the viewer's `lane`): from just ahead of the champion to the end of the line, nothing behind the champion. A soft fill fading in near the champion; two thin crescents near the start on a lane at least three times longer than its half width; an edge of a dark shadow, a soft glow and a bright core that goes from deep to the skill's colour toward the end, with a pale inner edge; dots and two small stars along the centre, a star on each edge and one at the tip. A line hit or projectile (rounded ends) ends in a half circle with a fine ring inside; a flat line ends straight.
- **Adjusted from the design** (it was drawn for a 520-long, 64-wide sample): the start gap, the fade-in and the crescents scale with each skill's length (at most 14 px, 40 px), the inner edge and the ring with its width; edge widths, dots and stars are fixed screen px; the design's star image is drawn from lines (the mod has no images in previews); the design's gold becomes the skill's own colour.
- **Style keys:** `lane_*` (27, in `preview_style.json`, overridable without a build); gone: `corridor_cap_width`, `corridor_cap_over`, `corridor_open_dash`, `corridor_open_gap` and the end bar and trailing edges of shots (`End::Open`, `End::Capped`). Fans of lines and lines between two fixed points keep the old strip.
- Tests: `a_line_is_an_aurum_lane` (70 px past the line on Kassadin W, nothing behind, the tip star, the core's colours, no crescents on a short wide lane, the stroke budget, grey when not ready); the strip test keeps its chevron part; 1 viewer check changed, 1 added.

## 1.5.2 — 2026-10-22

**Built, verified and played, but never uploaded to the Workshop: released as part of 1.5.3** (its exact source is kept locally in `local/release/source-1.5.2/`). Release of the 1.5.2-dev1 to 1.5.2-dev3 development builds below, together with "Custom previews in the viewer (tool only)" and "Skill preview viewer, cast and what it hits (tool only)", plus the range ring of direction skills, the champion-file report and the build without personal folders (this entry). The last public build was 1.5.1. One DLL for game 0.6.4, 0.6.5 and 0.7.0 Beta 2.

Seen in game by the author on 0.7.0 Beta 2 with the 1.5.2 build of 2026-10-23: rounded ends on line hits and projectiles ("it works"; the author dislikes the rounded part behind the champion, a redesign of the look is planned). Seen in game by the author on 0.6.5: the wider line hits (Bulldozer Q, Swordman R; 1.5.2-dev2), a custom preview from the viewer (Bulldozer Q; 1.5.2-dev2), and the new dash and jump back (1.5.2-dev3, "all passed").

- **Line hits at their true width:** skills that hit along a line from the champion (Bulldozer Q, Swordman R, Abbie W, Heavyblade R and many Workshop skills) are drawn twice as wide as before, as wide as they hit (read from the game's code).
- **New dash preview:** dashes and jump backs are one solid arrow from the champion to where it lands, brightening toward a white point, instead of small chevrons (the author's design K).
- **Bulldozer Q** shows its push box, its charge's hit line and its 84-unit charge, past the range circle as in the game.
- **Range ring of a skill aimed in a direction:** the ring shows how far the skill really reaches (the longest line, shot or fan from the champion, or a dash of a fixed length), never less than the cast range, also when hovering the skill (the author's choice A, 2026-10-22; read from the code: the mod sends such a skill only a direction, so its cast range does not limit the player). Tale Q: 180 on a cast range of 100. Unit and point casts keep their cast range. Tests: `a_direction_skill_ring_shows_how_far_it_reaches`; the hover test now expects the reach.
- **Rounded ends for line pieces:** a Line or Shot piece can have "Rounded ends" (`"rounded": true`): drawn as one capsule, one fill and one outline, reaching its width behind its start and past its end, as the game's line hit does (read from its shape check, docs/investigation-line-hit.md). Kassadin W (Wythm's pack): 35 each side of a 35 line, so 70 forward and 35 behind, with no crossing outlines (the author, 2026-10-22: three overlapping pieces were hard to read). New shape `Shape::Capsule`, used only by such pieces; shipped previews, recordings and the readers are unchanged; its ring on a skill aimed in a direction includes the rounded end. Tests: `a_rounded_line_is_one_capsule`, the table reading it, six viewer checks.
- **Line hits have rounded ends by default** (the author, 2026-10-23, after Taliyah R hit a jungle monster just past the drawn end with "Show hit areas" on; seen in game): the champion-file reader and the live reader draw every line hit (`LineRangeProjectile`, "Swordman ult") as a capsule, and the shipped table's seven recorded line hits (Abbie W, Heavyblade R, Hunter R, Magic Knight Q, Spellbreaker Q, Swordman R, White Mage Q) gained `"rounded": true` (`preview_sources.widened`; nothing else in them changed). A new Line or Shot piece starts with Rounded ends ticked; to turn them off for one skill, copy it to My pieces and untick. Projectiles, dash sweeps and the author's saved pieces (Bulldozer Q) keep their ends.
- **Projectiles have rounded ends by default** (the author, 2026-10-23; read from the code, docs/investigation-line-hit.md "Projectiles": the game checks a projectile's circle after every step of its flight and once more on its last tick, so it reaches its radius past its end). A single projectile's line from either reader is a capsule (`Geometry::add_projectile`), and 38 shipped projectile skills gained `"rounded": true` (`preview_sources.rounded_projectiles`; nothing else in them changed), among them Crossbowman Q, W and R, Tale Q, Hellion Q and Shadowmancer R; Workshop projectiles no match has recorded (Taliyah Q) come rounded from the champion-file reader. A shot that stopped at the first unit shows the rounded end instead of its bar. Fans of several projectiles (Dancer R) keep their lines. Not seen in game.
- **Champion files reported at start:** the mod writes what its champion-file reader found for every champion it read (`file-shapes.json` in its folder, `shape_json::write_file_shapes`); the viewer shows that for skills no match has recorded, so it draws what the game draws (Jhin Q's bounce circle; 58 of the author's 158 champions had no recording). The shape serializer moved from the research recorder to `shape_json.rs`, shared by both builds.
- **No personal folders in the DLL:** `tools/build_dll.py` builds with the builder's user folder and the project folder rewritten to neutral names (`--remap-path-prefix`), and `tools/verify_build.py` refuses a DLL that still holds one. Read from the files: 1.5.0 and 1.5.1 carried about 20 such paths, including the Windows account folder.
- **Viewer** (tool): a skill has two parts, the cast (from the game) and what it hits; old preview types become pieces; changing a piece's block keeps the sizes both blocks have (Line to Shot keeps width and length); see the "tool only" entries below.
- **For modders and the author** (read from the code): custom previews built from pieces in the viewer (`tools/preview_viewer.py`), drawn exactly as listed, sized from the game's numbers, from the shipped table or the author's own `skill-overrides.json`; the dash look is in `preview_style.json` (`dash_*` keys).
- Tested for this release: formatting, lint with warnings denied for both builds, 623 native tests in the public build and 658 in the research build (one existing helper ignored in each), 68 Python tool tests and 95 viewer checks, the verifier for 0.6.4, 0.6.5 and 0.7.0 Beta 2 with the preview table up to date (619 code anchors) and no build-machine folder in the DLL, the 103-file public package, installed (built with `tools/build_dll.py`; the account name found in the installed 1.5.2 DLL before, none after). Not played: this release build; 1.5.2-dev3 was played on 0.6.5 only, not on 0.6.4 or Beta 2. The champion-file report and the direction-skill ring not seen in game.

## 1.5.2-dev3 (development build) — 2026-10-22

**Seen in game 2026-10-22 by the author on 0.6.5 with 1.5.2-dev3: "all passed" (the bullet dash and jump back, grey when not ready, Bulldozer Q's custom preview). Development build, never for the Workshop.** The dash preview is the author's design K, "Bullet", and the shipped table carries the author's viewer work of today.

### Why (the author, 2026-10-22)

The chevron dash (since 1.3.x) was "terrible and borderline unreadable" in play (Bulldozer Q's custom preview). The author had the new look designed separately: `design/previews/dash-k.svg` (local, not in git), with its rules for any length.

### Now

- **Dash and jump back** (`skill_preview::dash`, read from the code; tests): one connected shape from the champion to where the dash ends, in world units scaled with the camera like every other preview. A body 10 wide in three equal strips at 14, 30 and 50 % between two dark edges at 5.75 each side (1.5 wide, 80 %); a nose `min(14, 0.3 x length)` long in four equal steps 10, 7.5, 5 and 2.5 wide (95 %), the last the white point, each over a dark step 3 wider (80 %). Yellow when ready, grey when not, the same rule as every piece. Every length uses this rule: the short-dash grip and the 22 px gap at the champion are gone (the author's choices of 2026-10-22: start at the champion, no special case).
- **Style keys:** `dash_body_width`, `dash_body_opacity_1` to `_3`, `dash_edge_offset`, `dash_edge_width`, `dash_edge_opacity`, `dash_nose_length`, `dash_nose_share`, `dash_nose_opacity`, `dash_nose_ink_extra`, `dash_nose_ink_opacity` (overridable in the user's `preview_style.json`). Gone: `dash_short`, `dash_short_grip` and the twelve `chevron_*` keys (an override of them is ignored). `dash_start` stays for the cast cue.
- **Viewer** (tool): draws the same bullet for dashes and jump backs (before, a cyan dashed line). Against the viewer before this change on the author's data, only the drawings of the 7 skills it shows with a dash or jump back changed (Archer Q and W, Dummy W, Ghost Q, Nightmare W, Nullifier Q, Zook W).
- **Shipped preview table:** Bulldozer Q is the author's custom preview (box with `width`, the 84 hit line, an 84 dash) and Heavyblade R's width follows `width` (both from the author's viewer Saves today).
- Unchanged: circles, lines and shots, cones, rectangles, blinks, the cast cue, the range ring, and which skills show a dash.

## Custom previews in the viewer (tool only) — 2026-10-22

**Seen in game 2026-10-22 by the author on 0.6.5 with 1.5.2-dev2 (log: `custom=1`): Bulldozer Q built in the viewer as box, hit line and a fixed 84 dash, drawn in game; its charge ends at 84 as the code says. The author: the dash's look is hard to read. Tool only: no mod build; the mod side is 1.5.2-dev2.** Stage 2 of custom previews built from pieces (the author's go-ahead of 2026-10-22).

- **Custom (pieces):** a new preview type on every card. It starts as a copy of what the card draws now, with each size's source kept where the source gives that very value. The **Pieces** tab of the skill panel (also the new card button) lists the pieces: block (circle, line, shot, rectangle, cone, fan of lines, Dancer blades, dash path, blink, jump back, attack range ring), where it starts (at the champion, at the aim, ahead by, at the end of a dash of, where a jump lands; a dash or blink goes to the aim or a fixed distance), its sizes and options (for allies, projectile, passes through units). Pieces can be added (at most 8), duplicated, moved and removed; the card redraws as you type.
- **Size sources per size:** a typed number, a game number (distances divided by 1000, times a factor), two game numbers multiplied, or the cast range; each row shows the value and whether it follows edits and patches. A cone's opening is typed in degrees. Names a skill holds more than once (Bulldozer Q: `speed` 7 and 4200, `tick` 1 and 20) cannot be asked of the game by name: they are shown but cannot be chosen, with their values listed for typing.
- **Drawing as the mod draws:** a custom preview keeps every piece (no dash hidden beside an area); a shot that stops at the first unit ends in a bar; a dash of a fixed length shows its length. Shapes that start ahead of the caster, at a dash's end or at a landing are placed as the mod places them (`skill_preview.rs` `placed`): before, the viewer cut a dash's end at the cursor for point casts.
- **Save:** the viewer checks every piece (`preview_viewer.py` `clean_custom`: known blocks, sizes in range, a recorded value for every named number, at most 8) and writes it to `skill-overrides.json`; the mod uses it at the next game start; the table rebuild ships it.
- Read from the code and the tools, not seen by the author: 65 Python tool tests (new `tools/test_custom_previews.py`), 61 viewer checks, and on the author's data copied aside: 3,900 skill and assignment cases and 5,400 drawings unchanged against the viewer before this change. In a browser on that copy: Bulldozer Q switched to Custom (box and hit line copied, the box keeping `width`), a dash piece added at a fixed 84, and saved; the table rebuild then held Bulldozer Q as custom and nothing else changed. The table and the author's files were left as they were.

## 1.5.2-dev2 (development build) — 2026-10-22

**Seen in game 2026-10-22 by the author on 0.6.5: a custom preview from the author's own file (Bulldozer Q, log `custom=1`) is drawn with its dash beside the areas. Development build, never for the Workshop.** Stage 1 of custom previews built from pieces (the author's plan of 2026-10-22, all three recommended choices): the mod side. Nothing on screen changes yet: no custom preview exists until the viewer can build one (stage 2).

### Why (the author, 2026-10-22)

Fixing one skill at a time in code (Bulldozer Q's charge) leaves the next unseen kind of skill wrong again. The author wants to build a correct preview for any skill in the viewer, from pieces, without code reading each time, and without touching any preview already shipped.

### Now (read from the code; tests)

- **Custom previews:** a table entry `"from": "custom"` holds the author's list of `pieces`, in the same form as recorded shapes: a block (circle, strip, shot, rectangle, cone, spread, fan, dash, blink, backstep, reach), where it starts (caster, aim, forward, end, landing), its sizes and options (allies, projectile, pierce). The mod draws it exactly as listed: the rule that hides a dash beside an area (`main_purpose`) leaves it alone (`Geometry::exact`). An empty list draws nothing and still wins, like an assignment. At most 8 pieces, as for every preview.
- **From the author's own file:** an entry of type `custom` in `skill-overrides.json` goes through the shipped table's path (sizes follow the game's numbers) and wins over the table, so a custom preview can be tried in game after a restart, before a build. The log's `PREVIEW OVERRIDES` line counts them (`custom=`).
- **Size source "two numbers multiplied":** `{"name": "speed", "by": "tick", "recorded": 84}` is the first number as a distance (divided by 1000) times the second as a plain count, times `times` if given: a charge's length (speed per tick x ticks) follows balance patches. The mod asks the game for both names.
- **Tools:** `build_preview_table.py` ships a custom preview's pieces as written (`preview_sources.skill` returns `from: custom`).
- Unchanged: the shipped table (the table check finds no difference), the old preview types (kept as quick options) and every skill without a custom preview.
- Not yet: the viewer's pieces editor and its drawing (stage 2); saving a custom preview from today's viewer is refused there.

## 1.5.2-dev1 (development build) — 2026-10-22

**Seen in game 2026-10-22 by the author on 0.6.5 with 1.5.2-dev2: Bulldozer Q and Swordman R show their correct width. Development build, never for the Workshop.** Line hits are drawn at their full width: twice as wide as before, for every skill that uses the game's line hit.

### Why

- **Seen in game (the author, 2026-10-22, 1.5.1):** Bulldozer Q's push area is extremely wide; the preview drew a 60-wide box.
- **Read from the game's code (0.6.4 apply `0x1705730`, 0.6.5 `0x1537f40`):** the line hit (`LineRangeProjectile` in champion files; the live reader's "Swordman ult" family) hits the game's segment shape from the caster to `length` ahead, with `width` as the segment's radius. So it reaches `width` on each side of the line: Bulldozer Q (`width` 60000) is 120 wide, 30 deep. Both readers took `width` as the full width (a guess since 1.5.0-dev2 for the file reader; matched by eye to another mod's display for Swordman R in 1.1.x).

### Now

- **Live reader and champion-file reader:** a line hit's radius is its `width`, no longer half of it. About 70 Workshop skills use this effect (most League-style packs, Abbie W, Heavyblade R, Bulldozer Q), and base Swordman R.
- **Shipped preview table:** eight skills twice as wide as before. Abbie W 27 (was 13.5), Bulldozer Q 60 (30), Heavyblade R 90 (45), Hunter R 20 (10), Magic Knight Q 5 (2.5), Spellbreaker Q 28 (14), Swordman R 40 (20), White Mage Q 16 (8); each is a radius, the line is twice as wide. Nothing else in the table changed.
- **Recordings:** a recorded shot now carries `hit_radius`. The viewer and `tools/build_preview_table.py` double piercing shots recorded before this build (`preview_sources.widened`), except Bombardier, Ferryman, Gladiator and Hypnotist, whose shots come from another effect that always recorded the hit radius. The recording file itself is not rewritten.
- **Size picks:** a pick of one of the game's numbers counts only while that number still gives the size (`preview_sources.still_fits`). The author's Bulldozer Q pick `width x 0.5` no longer fits and is ignored; the size now follows `width`. Heavyblade R's 90 fits both `range` and `width`, so it ships fixed at 90 until a pick (`width`) in the viewer.
- Not changed: Bulldozer Q's charge stays a 20-wide line of the mod's assumed body size (`CHAMPION_RADIUS`); the rounded ends the game's segment may have are not drawn (guess, unchecked).
- Not read: 0.7.0 Beta 2's line hit (the live reader is off there; the table and the file reader apply).

## 1.5.1 — 2026-10-22

Release of the 1.5.1-dev1 to 1.5.1-dev3 development builds below, together with the three preview-viewer "tool only" entries (the two above the 1.5.0 heading and "Skill preview viewer (tool only)" just below it): the viewer's changes since 1.5.0-dev21, which 1.5.0 kept local, ship in this release's source. The last public build was 1.5.0. One DLL for game 0.6.4, 0.6.5 and 0.7.0 Beta 2.

Seen in game by the author: on 0.6.5 with 1.5.1-dev1, the speed buttons while the AI drives, F11 at 3x, pause and View Match Result; on 0.7.0 Beta 2 with 1.5.1-dev3, the speed buttons, the two attack cursors, the skill previews (Archer W included), and the viewer's Save with the table check. On 0.6.4 with the 1.5.1 release build: the speed buttons, pause and resume at 2x, F11 at 3x, View Match Result, the two attack cursors and a skill preview.

- **Speed buttons while the AI drives:** after F12 the game's speed buttons (0.5x to 3x) play the match at the chosen speed (seen in game, 0.6.5 and Beta 2) and its pause works (seen in game, 0.6.5). F11 takes the match back at 1x (seen in game at 3x, 0.6.5); control always runs at 1x. The highlight button stays without effect, the author's decision. Read from the code (stage two, after the 0.6.5 play): a pause keeps the last speed, and the slow-playback warning skips a window in which the speed changed, the game paused or F11 took control. View Match Result still hands the match to the game.
- **Attack cursors:** "Normal · over enemy" and "Champion only · over champion" are the orange strike dart from the author's cursor design, generated by the cursor tool (seen in game, Beta 2).
- **Skill previews:** the shipped preview table keeps every champion, including those whose pack is disabled, and follows the author's preview viewer (seen in game, Beta 2; the keep rule read from the code).
- **For modders and the author** (read from the code): research-build recordings merge into `research/skill-shapes.json` instead of replacing it, with the game version per entry and an atomic write; `build_preview_table.py` ships the union of recorded, assigned and shipped champions and removes only with `--drop`; the viewer's Save rebuilds the table; `verify_build.py` fails while the table is stale. The viewer: cards from the game's files, the attack-range ring type, one shared scale with zoom, and the 2026-10-22 redesign with quick editing, with new tests (`tools/test_build_preview_table.py`, `tools/test_file_cards.py`, `tools/test_preview_viewer.cjs`).
- Tested for this release: formatting, lint with warnings denied for both builds, 619 native tests in the public build and 654 in the research build (one existing helper ignored in each), 57 Python tool tests and 44 viewer checks, the verifier for 0.6.4, 0.6.5 and 0.7.0 Beta 2 with the preview table up to date (619 code anchors against the installed Beta 2), the 103-file package for each variant, and the author's play listed above. Not played on 0.6.4 for this release; not installed by this build step.

## 1.5.1-dev3 (development build) — 2026-10-22

**Not seen in game. Development build, never for the Workshop.** The skill preview recordings and the shipped preview table never lose a champion, and the table follows the viewer by itself. Three pieces in one build.

### Why (seen in game, the author, 2026-10-22)

- The author enables a champion pack only to set up its previews, then disables it. Today the author found that this loses work:
  - Bombardier, Ferryman, Gladiator and Hypnotist are gone from `research/skill-shapes.json` (read from the files: only the shipped table still has them).
  - The viewer edit to Archer W (`jump_back` with the cast range) had not reached the shipped table, because nothing ran `tools/build_preview_table.py`.
- The causes (read from the code):
  - The recorder rewrote the file with only what it held in memory. That meant this session's champions plus the earlier ones it accepted, and it accepted none when the game version changed or an entry lacked `cast`.
  - `preview_sources.all_skills` walks only recorded champions, so the table tool would have dropped those four on its next run.

### Piece 1: the recorder merges (`shape_record.rs`, research builds only; read from the code)

- **The write merges.** Before writing, the recorder reads the existing file:
  - Every champion, and every skill's numbers, not recorded this session is kept as it is.
  - Every one recorded this session replaces the old entry.
  - Numbers merge per skill: a champion re-recorded in Q keeps its old W and R.
  - Anything the session knows that the file lacks is written back, for example if the file was deleted while the game ran.
- **Game version per entry.** Each champion entry, and each skill's numbers, now carries the game version it was recorded on (`game`).
  - Old entries get the old file's version.
  - The top-level `game` is the version of the last session that wrote the file.
  - A champion counts as already recorded only on its own version, so a new game version still records everyone again and replaces them one by one, without dropping the others.
- **Atomic write.** The file is written to `skill-shapes.json.tmp`, then renamed over the old one, so a crash cannot leave it empty.
- **A damaged file.**
  - A file that cannot be parsed is renamed to `skill-shapes.json.bad`, and one `SHAPE RECORD unreadable` line is logged. The write goes ahead with this session's recordings.
  - A file that cannot be read at all (locked) is left alone, and nothing is written.
- **Not done:** the viewer page does not show the per-champion `game` yet. The data is there for it.
- **Tests, on a fake folder:**
  - Old champions survive a write.
  - A re-recorded champion is replaced, and its numbers merge per skill.
  - A corrupt file is set aside, the write still happens, and the next write logs nothing.
  - A missing file is written from memory.
  - The loader's per-version rule.

### Piece 2: the table tool keeps every champion (`build_preview_table.py`; read from the code)

- **Which champions ship.** The champions shipped are the union of three groups: recorded, assigned, and already in `probe/src/preview_table.json`. The tool reads the shipped table first. The rules, decided with the author's session today:
  - **Recorded:** built from the recording, as before.
  - **Not recorded, but its pack is enabled and ships a data file:** its assignments are built with the file's numbers, as before (the viewer's file cards, `with_file_cards`). A champion whose pack ships a data file therefore needs no match to edit its previews, only the pack enabled. A match is needed only when the file gives no numbers.
  - **Not recorded and no data file (its pack disabled), but shipped:** its shipped entry is kept unchanged. It is reported as "kept from the shipped table (no recording): N" with the names.
  - **Assigned, with no recording, no data file and no shipped entry:** nothing is shipped. It is reported as "needs one match with the champion in play: N".
- In the first two cases, a skill the table shipped that nothing builds now also keeps its shipped entry.
- A file card never counts as a recording.
- **Removing.** Nothing is removed unless `--drop <champion>` (or `--drop "<champion> <Q|W|R>"`) is given. Dropping a name the table does not have is refused.
- **"No recorded shapes" is no longer an error.** Without any recording, the whole table is kept.
- **Writing.** The table is rewritten only when what it ships changes, so a Save that changes nothing does not make the DLL stale for `stage_package.py`. It is written with LF line ends, as `.gitattributes` asks.
- **`--check`.** It writes nothing:
  - If the shipped table differs from what a build would write, it exits 1, naming each champion and skill that differs (added, removed, changed).
  - Otherwise it exits 0 and prints "up to date".
  - It compares champions only, not the `about` text or the game version.
- **Every run ends with one line:** `Preview table: N champions; kept from the shipped table N; [from data files N;] needs a match N; …`.
- `preview_sources.py` is unchanged. The viewer still uses `all_skills` for what it shows; the table build no longer uses it.
- **The real build, run once (read from the tool's output; run here, 2026-10-22):**
  - **Champions before: 100. Champions after: 100.**
  - `--check` before the build named exactly one difference, `archer: W added`.
  - Archer W now ships `{"type": "jump_back", "distance": {"cast_range": true, "recorded": 30}}`.
  - Bombardier, Ferryman, Gladiator and Hypnotist are kept, byte-identical. Their packs are disabled, so no data file was used.
  - Enchanter, Hitman, Necromancer and Ralsei are recorded but build to nothing, as before.
  - Every other champion is identical. The `about` text gained one sentence.
- **Tests:**
  - The union.
  - The keep rule, for a champion and for a single skill.
  - A file card is not a recording.
  - The three no-recording cases: file card plus assignment, no card but shipped, and neither.
  - Needs-a-match.
  - `--drop`, for a champion, a skill, and an unknown name.
  - The difference lines.
  - The command on a fake folder: `--check` fails, then the build writes, then `--check` passes; an unchanged table is not rewritten; with no recording, everything is kept.

### Piece 3: automatic (read from the code)

- **Viewer Save rebuilds the table.** `tools/preview_viewer.py` runs `build_preview_table.py` after Save, and after each size pick. It runs as its own process, after the save is written, so a failed build cannot undo the save.
  - The page shows the tool's one-line result at the top, next to the save state, until the next save.
  - Builds run one at a time (a size pick saves by itself, so two could overlap), and the table is written to a temporary file and renamed, so it is never half written.
  - The first Save after a research match also ships that match's new recordings, without a separate review.
  - A failure shows in red as "Preview table not rebuilt (errors): …".
  - Tests: a failed build keeps the saved file and reports the error; the result line is the tool's last summary line.
- **The verifier checks the table.** `tools/verify_build.py` runs `build_preview_table.py --check` before anything else is verified. If the table is stale, the build fails with the tool's message. The result line goes into the build record (`preview_table_check`).
- **Docs:** `docs/version-migration.md` section 6 and the preview steps in `docs/modding.md` have one line each about this.
- **Unchanged:** `stage_package.py`'s stale-DLL check. The regenerated table is older than the DLL in the normal order: Save, then build.
- **Note for testing:** the public build also reads `skill-overrides.json` at game start (`skill_preview.rs`), and a local assignment the shipped table does not hold wins. So a changed preview can show in game even with a stale table. What shows that the table followed the viewer is the check's "up to date".

### Verified for this build

- Formatting; Clippy with warnings denied for the public and the research build.
- 619 passing tests in the public build (unchanged: the recorder is research-only) and 654 in the research build (651 + 3). One translation-export helper is ignored in each, as before.
- Python: 57 tests in `tools/test_*.py` (12 new in `test_build_preview_table.py`). Viewer page checks (`test_preview_viewer.cjs`): 44 passed.
- Research build: built, staged, verified (`dist/build-1.5.1-dev3-research.json`) and zipped (`dist/LTDirectControl-1.5.1-dev3-research.zip`), so it can be installed for the recording test.
- Public build, staged last: release build, `stage_package.py` (103 files), `verify_build.py` (`dist/build-1.5.1-dev3.json`), zipped (`dist/LTDirectControl-1.5.1-dev3.zip`).
- `verify_build.py`, for both builds: the tables of 0.6.4, 0.6.5 and 0.7.0 Beta 2, each against its own executable; all 621 native instruction/header anchors; ABI 6 and null-host refusal; preview table up to date.
- The verifier's failure on a stale table was not run against the real table. The tool's non-zero exit is tested on a fake folder.
- Not installed, not committed or uploaded. Not verified: anything in game.

## 1.5.1-dev2 (development build) — 2026-10-22

**Not seen in game. Development build, never for the Workshop.** Two separate pieces in one build, from the author's decisions of 2026-10-22: stage two of `design/pacing/spectator-speed-2026-10-22.md` (the game's speed buttons work while the AI drives, now as a feature), and the second refinement of the two attack cursors.

### Piece 1: the game's play speed while the AI drives (stage two)

- **What stage one showed (seen in game, the author, 1.5.1-dev1, game 0.6.5, 2026-10-22):**
  - The speed buttons write 0.5, 1, 1.5, 2 and 3 into the speed field, and the match played at that many times 60 ticks a second. At 3x the simulation waited 92 % of the time, so it keeps up easily.
  - F11 at 3x: `take_control lead 3 -> 1 queue=1`, drained after one frame, no release.
  - The highlight button writes mode 1 and speed 1. The mod forces mode 0, so it does nothing. The author decided to leave it that way.
  - The pause button writes speed 0. The field the override calls "pause" (`config+0x48`) read 1 throughout; it is not the game's pause.
  - `TIMING slow playback` fired six times, each in a window in which the speed changed.
  - The `spectator_fields_free` match: the game keeps sync and mode at 0 by itself, and the queue sat at the cap, so the game would run further ahead without it. The cap stays.
  - Not tested: 0.6.4 and 0.7.0 Beta 2.
- **Kept exactly as in stage one (read from the code):** the AI phase follows the game's speed field; lead = ceil(multiplier) x frame lead, at most 12; the frame-count guard tracks the queue; F11 forces 1x and drains. Control always runs at 1x.
- **Removed: the `spectator_fields_free` switch** (read from the code). Gone are its settings read in `client.rs`, the per-match latch, the refusal of F11 and the Take control button, its log lines and its test. `settings::hidden_switch` stays, because practice mode uses it. The viewer hook forces sync off and mode 0 in every phase, the AI phase included, as before stage one. Forcing changes nothing the game would not do by itself, and mode 0 keeps the highlight button dead. `PlaybackPlan` lost its `force_sync_mode` field.
- **The game's pause (read from the code):** a speed field of 0 is the pause.
  - The multiplier stays at the last non-zero speed, so the lead does not change, and no `SPECTATOR fields` line is written. Resuming at the same speed is no change either.
  - Before, 0 counted as "out of range" and set the multiplier to 1.
  - The design note's stage-two section says "multiplier 1" for the pause. This build follows the later instruction (keep the last speed), which is the reading that also gives "no lead change".
- **The slow-playback artifact: a window in which the speed changed is skipped (read from the code).**
  - The `TIMING slow` check judged each 10-second playback window against the multiplier at its end. Now a window in which the multiplier changed, the game paused, or F11 took control is not judged. Its line is written as `PACING ... ; speed changed or paused in this window, not judged`.
  - The next steady window is judged as before.
  - Skipping was chosen over a "most of the window" average because a window with a pause has no expected rate at all.
- **Log levels (read from the code):**
  - Every level, including Quiet: `SESSION AI control` (F12) and `SESSION spectator start` ("Watch this match"). Both now say "the game's speed buttons are followed; F11 ... at 1x".
  - Every level: `SPECTATOR take_control` and `SPECTATOR drained`, two lines per retake.
  - Verbose only: `SPECTATOR fields` and the ten-second `SPECTATOR multiplier=` line (which lost its `fields_free=` field).
  - `TIMING slow` stays at every level; `PACING` stays Verbose.
- **README:** the session-controls entry said AI playback stays at 1x and that changing the game speed hands the match back. Both were out of date since 1.5.1-dev1. That sentence now reads: while the AI drives, the game's speed buttons work; control always runs at normal speed. The View Match Result hand-back sentence is kept.
- **No new text anywhere.** The highlight button stays dead (mode forced to 0).
- **Tests (read from the code, run here):**
  - Added:
    - The pause value (+0 and -0) keeps the multiplier, the lead and the line count; resuming at the same speed writes nothing, and a new speed does.
    - Pause and resume at 2x in the simulated viewer: no release, and 2x again after resuming. While paused the test allows up to two ticks, what the test viewer's clock may still owe; how the game itself plays out a pause is a guess.
    - Skipped windows: 0.5x→3x mid-window, a pause mid-window, and F11.
    - A counter-test: a steady slow window is still reported, including the one right after a skipped window, and nothing is reported at the selected speed.
  - Removed: the fields_free test, the fields_free cases of the playback-plan test, and the fields_free section of the hook's restore test.
  - The logging test now checks both levels.
  - All other stage-one tests are unchanged and pass.
- **Not known:** whether 0.6.4 and 0.7.0 Beta 2 treat the speed field the same way (the test sheet's Part A).

### Piece 2: the two attack cursors, second refinement

- **The author's refined design** `design/cursor/index_new_2nd.html` was copied over `design/cursor/index.html`; `index_new.html` and `index_new_2nd.html` are both kept. Compared with `index_new.html` only `strike()` differs: the dart is narrowed about 20 %, the hazard bars are shortened to match, and the inner stroke moved with it. Hotspots stay at (2,2).
- **Generated:** `node tools/generate_cursor_assets.cjs` rendered all 14 cursors; nothing was drawn by hand.
  - A hash comparison of `probe/cursor/*` and `probe/ui/cursor_preview.png` before and after shows that only `hostile.{svg,png,bgra}` and `champHostile.{svg,png,bgra}` changed against 1.5.1-dev1.
  - The generator updated their two hashes and the design file's hash in `tools/records/cursor-assets.json`.
- **Tests:** the cursor tests pass (hotspots at small and large sizes, scaled premultiplied alpha), and the verifier checks the new hashes.

### Verified for this build

- Formatting; Clippy with warnings denied for the public and the research build.
- 619 passing tests in the public build (1.5.1-dev1 had 616: four added, one removed) and 651 in the research build. One existing translation-export helper is ignored in each, as before. The research build is linted and tested, not built, staged or verified.
- Release build.
- `stage_package.py`: 103 files in `dist/lt_direct_control`, public build.
- `verify_build.py`: the tables of 0.6.4, 0.6.5 and 0.7.0 Beta 2, each against its own executable; all 621 native instruction/header anchors; ABI 6 and null-host refusal; the cursor hashes. Record: `dist/build-1.5.1-dev2.json`.
- Not packaged as a zip, not installed, not committed or uploaded. Not verified: anything in game.

## 1.5.1-dev1 (development build) — 2026-10-22

**Not seen in game. Development build, never for the Workshop.** Two separate pieces in one build, both approved by the author on 2026-10-22: stage one of `design/pacing/spectator-speed-2026-10-22.md` (a diagnostic build: the game's speed buttons work again while the AI drives), and two cursors from the newer cursor design.

### Piece 1: the game's play speed while the AI drives (stage one, diagnostic)

- **Why (seen in game, the author, 2026-10-22; read from the code):** after F12 the game's speed buttons did nothing. The viewer hook wrote speed 1.0 into the game's playback speed field (`config+0x14`) on every frame, including AI control, so each click was overwritten on the next frame. *Superseded 2026-10-22:* the 0.78.2 hand-back on a speed-button change (`ai_handback.rs`) never fired. The SDK reports no selection state for those buttons (`PLAYBACK speed_states=` shows `None` for all of them), so the comparison never changed.
- **AI phase only (read from the code):** the viewer hook no longer writes the speed field. Sync and mode are still forced, and pause is still left to the game, exactly as before. The value the hook reads there becomes the pacing multiplier; a value outside 0.25..=4, or not a number, counts as 1. The buttons' values (0.5, 1, 1.5, 2, 3) are a **guess** that this build's log measures. Ready, Running, Paused and Loading are unchanged: control always plays at 1x.
- **Only overwritten fields are put back.** The hook used to restore all four fields when it returned. That would have undone a speed the game writes during the viewer update. It now restores only the fields it overwrote; in AI control that is sync and mode.
- **Lead:** in AI control the simulation may run `ceil(multiplier) x frame lead` ticks ahead, at most `MAX_SPECTATOR_LEAD` = 12 (3x at 15 fps). That is 3 ticks at 3x and 60 fps. The viewer's frame-time cap uses the same figure. The frame-lead rule and the owed-time smoothing (`lead_for`, `smooth`) are unchanged.
- **The "Unexpected publication/playback frame counts" guard** now allows as many ticks in one frame as can be queued, and never fewer than before (4). A plain "AI lead" limit would release the session when the lead falls with ticks still queued: F11 at 3x, or a slower speed button, at low frame rates. The tests reproduce that release: with the plain limit, the 15 fps F11 test and the speed-change test both fail. The queue is tracked instead: the limit is the highest lead the worker produced under, until the queue is back under the current lead.
- **F11 (TakeControl):** control forces 1x from the next frame, and the lead returns to the frame lead (1 at 60 fps). Ticks already queued play at 1x while the worker waits. This is the one gap the design note names: at most 3 ticks (50 ms) at 3x and 60 fps, paid once per retake. Log: `SPECTATOR take_control multiplier=<m> lead <ai> -> <frame lead> queue=<n>`, then `SPECTATOR drained lead <ai> -> <lead> queue=<n> after <frames> frames`.
- **Diagnostics, written at every log level:**
  - `SPECTATOR fields sync=<u8> mode=<u32> speed=<f32> paused=<u8> multiplier=<m> lead=<n>`: once on entering AI control, then whenever one of the four raw values changes. At most one line per frame and 200 per match.
  - Every 10 seconds of AI control: `SPECTATOR multiplier=<avg> ticks_per_frame=<avg> queue=<avg> worker_wait_pct=<share> frames=<n> window_s=<s> lead=<n> frame_lead=<n> fields_free=<bool>`.
    - `queue` is produced minus played just before each frame plays.
    - `worker_wait_pct` is the share of the window the simulation worker spent waiting for playback. It is counted by the coordinator itself, because perf's own counter is emptied by the PERF window.
  - In AI control, the `PACING`/`TIMING slow` playback-rate line expects 60 x multiplier ticks a second, so 0.5x is not reported as slow.
- **Hidden diagnostic switch `spectator_fields_free`:** set `"spectator_fields_free": 1` in `%LOCALAPPDATA%\LTDirectControl\controls.json` with the game closed. The file is read at game start, and the switch is latched at the start of each match. In AI control the hook then leaves all four playback fields to the game: sync, mode, speed and pause.
  - F11 and the Take control button are refused for that match (`SPECTATOR fields_free: F11 ignored`).
  - The simulation's lead and both guards stay as above, so the queue is bounded at 12 by construction. A `queue=` pinned at the lead means the game would run further ahead. A release in that match is itself a data point.
  - The switch has no control in the settings window. Remove the line, or set 0, to turn it off.
- **Removed:** the dead speed-button comparison in `ai_handback.rs`. The "view result" hand-back is unchanged; its click test is now a separate function with a test. The `PLAYBACK speed_states=` log line in `client.rs` is left as it was.
- **Tests (read from the code, run here):**
  - The multiplier from each button value, from out-of-range values and from NaN.
  - The AI lead arithmetic and its cap, and the playback plan per phase.
  - Restore-only-what-was-overwritten, on a fake config.
  - The field log and its budget.
  - The guard follows the AI lead and never drops below 4.
  - Simulated playback at every speed and at 60, 30 and 15 fps, with no release and ticks per frame within 5% of speed x frame time. This assumes the game scales its frame clock by the speed field (**guess**).
  - F11 at 3x at 60 fps (no frame plays more than 2 ticks) and at 15 fps (drains, no release), with 1x forced on the next frame.
  - Speed changes at 15 fps without a release.
  - fields_free latched per match, with F11 and the button refused.
  - View result still releases.
  - Existing pacing tests unchanged; the playback-field test now builds its plans from the phase.
- **Not known (open question of the design note):** where the game applies the speed field to its frame clock, and whether 0.7.0 Beta 2 does it the same way. If the game ignores the field, the match simply runs at 1x as before.

### Piece 2: two cursors from the newer design

- **The author's updated design** `design/cursor/index_new.html` was copied over `design/cursor/index.html`; `index_new.html` is kept. It replaces the sword with an orange strike dart. `hostile` (Normal · over enemy) has two hazard bars; `champHostile` (Champion only · over champion) has one bar plus the badge. Hotspots stay at (2,2). The other twelve cursors are unchanged.
- **Generated:** `node tools/generate_cursor_assets.cjs` rendered all 14 cursors from the design file; nothing was drawn by hand.
  - Only `probe/cursor/hostile.{svg,png,bgra}` and `champHostile.{svg,png,bgra}` changed, together with their hashes and the design file's hash in `tools/records/cursor-assets.json`.
  - The settings preview `probe/ui/cursor_preview.png` (the normal arrow) is byte-identical.
- **Tests:** the existing cursor tests pass (hotspots at every size, premultiplied alpha), and the verifier checks the new hashes.

### Verified for this build

- Formatting; Clippy with warnings denied for the public and the research build.
- 616 passing tests in the public build (1.5.0 had 601) and 648 in the research build; one existing translation-export helper is ignored in each, as before. The research build is linted and tested, not built, staged or verified.
- Release build.
- `stage_package.py`: 103 files in `dist/lt_direct_control`, public build.
- `verify_build.py`: the tables of 0.6.4, 0.6.5 and 0.7.0 Beta 2, each against its own executable; all 621 native instruction/header anchors; ABI 6 and null-host refusal; the cursor hashes. Record: `dist/build-1.5.1-dev1.json`.
- Not packaged as a zip, not installed, not committed or uploaded. Not verified: anything in game.

## Skill preview viewer, quick editing restored (tool only) — 2026-10-22

**Read from the code (viewer source):** the author's approved follow-up keeps the second visual design and restores direct editing. Only the viewer, its local checks and this entry change; no native mod or server changes, release, installation, commit or upload.

- **Superseded, 2026-10-22 (the author's review):** the second pass's panel-only editing made the viewer slower as an editing tool. Its earlier record remains below. Shape selection, numeric values, flags, add/replace, suggestions, reset and size-source choices now stay directly beneath each drawing. Compact field groups retain the original handlers, validation, drafts and save formats; reset's complete meaning and the add/replace preview description remain available through labels and tooltips.
- Neutral charcoal and white replace the green interface accent in both themes. The game's drawing colors remain unchanged. Numbers and complete descriptions retain their floating reference panel, keyboard tabs and Escape; opening, switching or closing references never moves the quick-edit controls.
- Validation: formatting, code checking, Clippy with warnings denied, release build, 601 passing native tests (one existing ignored helper), 45 Python tests and 48 viewer checks pass. The original-viewer comparison still matches 4,056 skill/assignment cases and 5,616 drawings. Native profile verification passes for 0.6.4, 0.6.5 and 0.7.0 Beta 2 against their own executables.
- Browser checks on isolated copies cover all eleven editable preview types, immediate numeric updates, allies and add mode, invalid-value exclusion, Ctrl+S, reload persistence, direct size-source saving, and reference tabs/Escape. Measurements at 1440, 720 and 390 pixel frame widths confirm three/two/one columns, no horizontal overflow, and identical card heights and row positions when numbers open. The quick editor remains on its card while the reference panel is open.
- Local, uncommitted tool changes only. No test edit was saved to the author's real files; not verified in game.

## Skill preview viewer, second visual pass (tool only) — 2026-10-22

**Read from the code (viewer source):** the author's approved revision after reviewing the first design. This remains a local tool change; native mod code, the Python server, launcher, game settings and saved-file formats are unchanged. No release version, package, installation, commit, push or upload in this stage.

- **Rejected, 2026-10-22 (the author's review):** the first design was too generic, and opening Game numbers grew the skill row and pushed later rows down. The first design's entry below and a local source snapshot are retained.
- Visuals: a flat comparison board, larger champion and Q/W/R headings, uninterrupted drawing surfaces, embedded Manrope typography and existing Lucide icons, compact viewing controls, and light/dark appearances. Font and icon licences are embedded with the assets; the standalone page needs no external font or icon requests.
- Editing, size sources, game numbers and complete descriptions now share a floating inspector. Existing controls move into it and return to their original skill when it closes; their values and event handlers are retained. Game numbers use a two-column list. The inspector chooses a side with space, fits narrow screens, supports keyboard tabs and Escape, and returns focus to its opening button. It does not reserve gallery space or lock the page scroll.
- Validation: formatting, code checking, Clippy with warnings denied, release build, 601 passing native tests (one existing ignored helper), 45 Python tool tests and 45 viewer checks. The comparison with the original viewer covers 4,056 skill/assignment cases and 5,616 drawings; preview calculations and shape coordinates match. All supported native profiles pass against their own executables: 0.6.4, 0.6.5 and 0.7.0 Beta 2.
- Browser measurements at 1440, 720 and 390 CSS-pixel frame widths confirm three, two and one skill columns respectively, no horizontal overflow, and identical card/row positions and heights before and after opening numbers. Direct pointer checks also preserve scroll position. Checks on isolated data copies cover editing, validation, add/ally flags, Ctrl+S, automatic size-source saving, backups, retained assignments, recovery, save failure, standalone rendering, filters, help, keyboard tabs and Escape. An initial background-tab viewport attempt did not apply the requested sizes and is marked rejected in the private verification record; the final responsive evidence uses explicit frame widths.
- Not verified in game. No test assignment or checked mark was written to the author's real files.

## 1.5.0 — 2026-10-22

Release of the 1.4.1 to 1.5.0-dev34 development builds below; the last public build was 1.4.0. One DLL for game 0.6.4, 0.6.5 and 0.7.0 Beta 2; 0.6.3 and 0.7.0 Beta 1 are no longer supported.

Seen in game by the author: full matches on 0.6.5 (1.5.0-dev25) and 0.7.0 Beta 2 (1.5.0-dev27); short matches with the final features on all three versions (0.6.4 with 1.5.0-dev31, 0.6.5 and Beta 2 with 1.5.0-dev33, Beta 2 with 1.5.0-dev34): every hook family installed (camera, minimap, outline, movement, combat, shop, native tooltips, athlete stat rules), the item texts with the game's numbers (0.6.5 and Beta 2), the refill display, the ready chime, Bombardier's deployed rule and ranges (Beta 2). The controls and preview work of 1.4.1 to 1.5.0-dev12 as recorded in each build.

- **Game versions:** 0.6.5 and 0.7.0 Beta 2 supported next to 0.6.4 (the Steam rollback). On 0.6.5 the screen shows the live match again (the game's 15 s screen delay is removed while the mod plays). 0.6.3 and 0.7.0 Beta 1 are dropped, which brings the official interface's cooldown read (ABI 10). On an unknown game version the mod installs nothing and matches play as the plain game.
- **Controls (like League):** hold right-click to keep moving; skills survive right-clicks; forgiving unit-target skills (a nearby target when the cursor misses); a crowd-control hold; the plain left-click works during a pause; a left-click on empty ground closes the shop. A skill that is not learned yet is never sent (casting some stopped the match).
- **Skill previews:** drawn from a table shipped with the mod, each size read from the game's own numbers by name, so they follow balance patches and database edits; the game's champion files and the author's assignments cover what the game's code does not show; a visible range indicator and a cast-style cue; dashes, lobs and areas drawn where the caster lands; a ring at the attack range plus a skill's extra (Bombardier Q). The new Beta 2 champions (Bombardier, Ferryman, Gladiator, Hypnotist) have previews from the author's assignments.
- **HUD:** skill tooltips show values computed from your current stats (Ctrl shows the game's wording); item effect text shows the game's own numbers; a multi-use skill shows its next use refilling while some are left; the cooldown bar is sized from the game's cooldown read; the optional "Skill ready sound" chimes for every skill and every use that comes back; death darkens only the skill slots; the HUD keeps its last values when samples stop.
- **Athlete stat rules:** back on 0.6.5 and on Beta 2 (35 sites); the reaction band defaults to floor 12 / ceiling 35 ticks; your champion's body counts as a neutral Dodging 50.
- **For modders and the author:** public and research builds, tidier log levels, the migration runbook and records for 0.6.5 and Beta 2. The preview viewer's changes since 1.5.0-dev21 (cards from the game's files, the attack-range ring type, one shared scale with zoom, and the 2026-10-22 redesign) stay local and are not in this release's source (the author, 2026-10-22); the shipped preview table they produced is.
- Tested for this release: formatting, lint with warnings denied for both builds, 601 native tests in the public build and 633 in the research build (one existing helper ignored in each), 45 Python tool tests and 32 viewer checks, the verifier for 0.6.4, 0.6.5 and 0.7.0 Beta 2 (619 code anchors against the installed Beta 2), the 103-file package for each variant, and the author's play listed above. Rebuilt once after the author saved Bombardier Q's preview (the ring at attack range plus `move_distance` by name; only Bombardier changed in the table) and reinstalled over the first 1.5.0 build, checked against a temporary record of the installed files (all fingerprints match). Not played on 0.6.4 for this release: a full match (part of one with every family), an item with value slots and recall.

## Skill preview viewer (tool only) — 2026-10-22

**Read from the code (viewer source):** a visual redesign of `tools/preview_viewer.html`, approved by the author on 2026-10-22. No native mod code, Python server, launcher, saved-file format, or game settings changed in this stage. No new mod version, staged package, installation, commit, push, or Workshop upload.

- Visuals first: light and dark themes, charcoal drawing stages, a lime accent, three skill columns on desktop, compact filters and view controls, and layouts for smaller windows. Guidance opens on demand. Complete shape descriptions and suggestion provenance remain available through drawing labels and tooltips; descriptions and game numbers still expand.
- Existing workflows retained: every preview type and flag, add/replace, suggestions, size sources, saved assignments (including champions outside the current list), checks, recovery drafts, filters, level and shared zoom, and standalone assignment downloads. Quick status filters use the same existing status filter; zero-result states remain selectable. Keyboard description expansion, canvas zoom keys and explicit field labels are added.
- A browser check caught input arriving before data finished loading. Controls now wait for the data, and the save shortcut cannot write an empty assignment list during loading.
- **Superseded, 2026-10-22:** the old help text claimed that each drawing zoomed to fit and referred to a "Same scale" checkbox. The dev32 tool already used a shared scale and zoom slider. Help now describes that existing behavior.
- Validation: formatting, code checking, Clippy with warnings denied, release build, 600 passing native tests (one existing ignored helper), 45 passing Python tool tests, and 36 viewer checks. The optional comparison covers 4,056 skill/assignment cases and 5,616 drawings against the pre-redesign viewer, using a frozen local data copy; shape coordinates match at three levels, three zoom scales and two aim positions. All three supported native profiles pass against their own executables: 0.6.4, 0.6.5 and 0.7.0 Beta 2.
- Browser checks use isolated copies for all writes: assignments, checked marks, add/ally flags, size-source picks, backups, invalid values, recovery, save failure and standalone rendering. Light/dark and narrow layouts inspected. The first restricted native test run blocked two temporary-file writes; the unchanged suite passed outside those restrictions. Browser checks and builds do not verify anything in game.

## 1.5.0-dev34 (development build) — 2026-10-22

**Not seen in game. Development build, never for the Workshop.** The skill ready chime plays for every skill and for every use of a multi-use skill that comes back. Approved by the author on 2026-10-22 ("go ahead with 1.5.0-dev34").

- **Seen in game (1.5.0-dev33, the author):** on 0.6.5 and on 0.7.0 Beta 2 (Ghost), the next use's refill shows while uses are left, all four item texts come from the game in one go, the unlearned W is refused (the dash seen at level 1 was Q), and the charge counts match the game's. Beta 2 log: every hook family and 35 stat sites, the mod's minimap, skill tooltips and hover outlines. The author's caveat: the ready chime played only for the first use back.
- **Chime ("Skill ready sound", still off by default):** asked for every skill that comes back, with no 4-second minimum any more (the author; the chime was skipped for cooldowns under 4 s). The half-second guard against a one-frame readiness flicker stays (no real cooldown is that short). A multi-use skill now also chimes when a further use refills while it is already usable (the count rises from one or more, with the same number of uses held: a level-up that adds one does not chime). The gold ready glow still marks only zero to usable.
- **Code comment:** the comparison log's comment now says the older counter is a tick behind the newer read (it repeated the superseded "hook is fresher" explanation).
- **Tests:** every use that comes back asks for the chime, once from zero, not for a level-up or while dead; two-second cooldowns chime.
- Verified: formatting and Clippy with warnings denied for both builds; 601 passing tests in the public build and 633 in the research build (one existing translation-export helper intentionally ignored in each); 45 passing tool tests; both release builds; the native profiles of 0.6.4 / 0.6.5 / 0.7.0 Beta 2, each against its own executable; all 619 native instruction/header anchors counted against the installed game (Beta 2), DLL interface/null-host refusal and the 103-file package, for each variant. The research build is verified and packaged, not installed. The public build is installed over 1.5.0-dev33 (installed DLL, metadata and all 101 assets match; it carries 1.5.0-dev34 and `variant=public`; the installed game is Beta 2) and is what `dist/lt_direct_control` holds. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.5.0-dev33 (development build) — 2026-10-22

**Not seen in game. Development build, never for the Workshop.** A skill with several uses shows the next use refilling while some are left. Approved by the author on 2026-10-22 ("A, go ahead with 1.5.0-dev33").

- **Seen in game (1.5.0-dev32 on 0.6.5, the author, Ghost, a short match):** every hook family installed (stat rules 35 sites), the mod's minimap, skill tooltips from the game and hover outlines worked, no errors. The author: the skill slot showed no cooldown while a multi-use skill refilled with uses left. **Read from the code:** the HUD has always shown the bar and countdown only when no use is left (the charge marks of 1.3.x), so this is not new in dev32. **Read from the log:** the mod's count matched the game's (Ghost Q 0 and 0; Ghost W after one use 2 and 2); `ABILITY COOLDOWN_READ` said `agree=no` only because the hook's counter is a tick fresher than the game's read (240 against 239). *Superseded 2026-10-22 (the author's dev33 match on 0.6.5):* the one-tick gap is between the two SDK cooldown readings in the same callback (the older counter is a tick behind the game's newer read: counter 1 while the read says 0), not the hook being fresher; the mod sees a refilled use one tick (1/60 s) after the game.
- **Refill shown (the author's option A, like League):** while some uses are left but not all, the slot shows the next use refilling: the bar drains from one use's refill time, under a lighter shade (`REFILL_SHADE`) over the still-bright icon, with the bar's edge line and the countdown. The ready glow and chime still fire only from zero uses to usable. The refill time comes from the hook's counter and the game's model (`Charges::refill`), the same source as the count.
- **Log:** `ABILITY COOLDOWN_READ` compares the counts only (`agree=yes` when the mod's count equals the game's). *Seen in game (dev33 on 0.6.5, the author):* counts agree (Q, and W at 2 of 3 uses); `agree=no` only at the exact tick a use refills; the unlearned W was refused nine times at level 1 (the dash seen then was Q); all four item texts answered from one item list build.
- **Tests:** the refill time (Ghost W's numbers from play: two left, the third in 300 ticks; none when all or none are left; none for a single-use skill).
- Verified: formatting and Clippy with warnings denied for both builds; 600 passing tests in the public build and 632 in the research build (one existing translation-export helper intentionally ignored in each); 45 passing tool tests; both release builds; the native profiles of 0.6.4 / 0.6.5 / 0.7.0 Beta 2, each against its own executable; all 621 native instruction/header anchors counted against the installed game (0.6.5), DLL interface/null-host refusal and the 103-file package, for each variant. The research build is verified and packaged, not installed. The public build is installed over 1.5.0-dev32 (installed DLL, metadata and all 101 assets match; it carries 1.5.0-dev33 and `variant=public`; the installed game is 0.6.5) and is what `dist/lt_direct_control` holds. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.5.0-dev32 (development build) — 2026-10-22

**Not seen in game. Development build, never for the Workshop.** The cooldown bar takes its full length from the game's own cooldown read; a log line compares the mod's charge count with the game's. Approved by the author on 2026-10-22 ("go ahead with dev32").

- **Seen in game (1.5.0-dev31 on 0.6.4, the author; part of a match as Sun Mage, about 11 minutes):** every hook family installed (stat rules 33 sites), the outline drew on hover, skill tooltips came from the game, camera, minimap and shop (42 purchases) worked, no errors. Not exercised: an item with value slots, recall, the result screen.
- **Cooldown bar:** its full length is now one use's refill time from the game's read (`CooldownV1.cooltime` per use, after cooldown reduction), so the bar, its warm edge, the ready cue and the 4 s chime are right from the first frame, including when control starts mid-cooldown. The highest remaining time seen since the skill was last ready is kept only for a moment without a fresh read.
- **Charge count unchanged, on purpose:** the plan was to take the game's `charges` too. Read from the code (a hook test): the combat hook samples the cooldown counter the moment a use is spent, fresher than the game's read (once per tick), so a second press right after a cast is refused; the game's count would arrive a tick later. The count stays the counter's, from the game's model (cycle and uses).
- **New log line `ABILITY COOLDOWN_READ`** (once per match and skill while on cooldown, and at a first disagreement): the counter, the game's `remaining`, `cooltime`, `max_charges`, `charges` and `next_charge_ticks`, the mod's count and `agree=yes|no`.
- **Preview viewer (tool, before this build):** every card uses one scale like the game's camera (260 units across by default) with a zoom slider and the mouse wheel; the grid step adapts and is labelled. It replaces the per-card fit, which ignored the attack-range ring (Bombardier Q's ring was drawn off the card), and the "Same scale" checkbox. Saving keeps the assignments of champions the installed game does not list.
- **Tests:** the bar and chime take the game's cycle from the first frame; one use's refill time per model; the existing readiness tests run on the fallback.
- Verified: formatting and Clippy with warnings denied for both builds; 600 passing tests in the public build and 632 in the research build (one existing translation-export helper intentionally ignored in each); 45 passing tool tests; both release builds; the native profiles of 0.6.4 / 0.6.5 / 0.7.0 Beta 2, each against its own executable; all 625 native instruction/header anchors counted against the installed game (0.6.4), DLL interface/null-host refusal and the 103-file package, for each variant. The research build is verified and packaged, not installed. The public build is installed over 1.5.0-dev31 (installed DLL, metadata and all 101 assets match; it carries 1.5.0-dev32 and `variant=public`; the installed game is 0.6.4) and is what `dist/lt_direct_control` holds. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.5.0-dev31 (development build) — 2026-10-22

**Not seen in game. Development build, never for the Workshop.** A preview type "Ring at attack range + extra" that follows the game's numbers (Bombardier Q). Approved by the author on 2026-10-22 ("A, go ahead with 1.5.0-dev31").

- **Seen in game (1.5.0-dev30, the author):** item texts and the viewer cards "all look good"; one caveat: Bombardier Q assigned by hand as "Around self" radius 115 (attack range 65 + deploy bonus 50) is right now but goes wrong when either number changes.
- **Read from the code:** the mod already draws a ring at the live attack range plus an extra (`Shape::Reach`, made for Bombardier Q on 0.7.0 Beta 1), and the shipped table can size the extra by name; only an assignment could not ask for it.
- **New assignment type `attack_reach`** (viewer: "Ring at attack range + extra"; `skill_preview::assigned`): the extra is suggested from `move_distance`; in the shipped table it becomes the skill's number by name, so the ring follows the attack range (level, items) and balance patches.
- **Deployed (the author's option A):** while the champion carries the move-blocking status (Beta 2: `bombardier_deployed`, the same check as the move rule), the attack range already holds the extra, so the ring shows the range after packing (attack range minus the extra). Reported by the combat hook with the attack range each tick (`skill_preview::set_reach_active`).
- **Viewer:** draws the ring from each champion's basic attack range in the game's files (sheet or data file), labelled "attack range 65 + 50".
- **Tests:** the ring's radius in both states and the assignment; the extra sized from `move_distance` by name (tool test).
- **Docs:** backlog item 2.
- Verified: formatting and Clippy with warnings denied for both builds; 599 passing tests in the public build and 631 in the research build (one existing translation-export helper intentionally ignored in each); 45 passing tool tests; both release builds; the native profiles of 0.6.4 / 0.6.5 / 0.7.0 Beta 2, each against its own executable; all 619 native instruction/header anchors counted against the installed game (Beta 2), DLL interface/null-host refusal and the 103-file package, for each variant; the viewer page offers the new type with Bombardier's attack range 65 (built-in browser; nothing saved). The research build is verified and packaged, not installed. The public build is installed over 1.5.0-dev30 (installed DLL, metadata and all 101 assets match; it carries 1.5.0-dev31 and `variant=public`; the installed game is Beta 2) and is what `dist/lt_direct_control` holds. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.5.0-dev30 (development build) — 2026-10-22

**Not seen in game. Development build, never for the Workshop.** Item tooltips show the game's numbers once they arrive; every item text is asked for at once; preview viewer cards straight from the game's files. Approved by the author on 2026-10-22 ("go ahead with 1.5.0-dev30 and the viewer cards").

- **Seen in game (1.5.0-dev29 on Beta 2, the author):** 不破壁壘 still showed "…". Read from its log: the game's item text came back complete for all four items; the HUD tooltip and the shop kept the text they had built on the first hover, before the game's answer arrived. **Also seen in game (1.5.0-dev28, the author):** deployed Bombardier's attack and W show the correct range in the preview's range ring.
- **Fix:** each game item text that arrives raises a counter (`native_tooltips::item_revision`); the HUD drops its stored item tooltips and the shop its stored item names and texts when it changes, so they are built again with the game's numbers.
- **One ask for all items:** the first item that needs the game's text queues every item whose effect sentence has value slots (same rules: an item's own description wins, and the SDK must find the exact text), so the game's item list is built once per match however many items an item mod adds. Answers stay per match (a balance mod can patch items between matches: the author).
- **Preview viewer (tool, not the mod):** a champion the game can field that no match has recorded gets a card from its data file: how each skill is cast and what it targets (the game's names mapped to the codes a recording uses), its range and range growth, and its numbers; a name the file holds twice with different values is not offered as a size (Alchemist Q has two `radius`). Marked "from the game's files, not seen in play"; a recording always wins. `tools/build_preview_table.py` uses the same cards, so assignments for these champions reach the shipped table with sizes as numbers by name. Beta 2's Bombardier, Ferryman, Gladiator and Hypnotist now have cards (checked in the viewer page; Bombardier W suggests a circle of radius 22 at the aim, R's suggestion "dash 55 long" is wrong and is the author's to replace). Champions whose skills run custom code answer no sizes in play: their sizes are typed.
- **Tests:** the once-per-match ask and the counter (in the item queue test); four viewer card tests (cast and range from the file, duplicated names left out, a recording wins, an assignment's size comes from the number by name).
- **Docs:** investigation-item-text.md (dev30), backlog item 2 (viewer cards), migration-0.7.0b2.md (Bombardier range seen in game).
- Verified: formatting and Clippy with warnings denied for both builds; 598 passing tests in the public build and 630 in the research build (one existing translation-export helper intentionally ignored in each); 44 passing tool tests; both release builds; the native profiles of 0.6.4 / 0.6.5 / 0.7.0 Beta 2, each against its own executable; all 619 native instruction/header anchors counted against the installed game (Beta 2), DLL interface/null-host refusal and the 103-file package, for each variant; the viewer page loaded with the four Beta 2 champions as cards (built-in browser). The research build is verified and packaged, not installed. The public build is installed over 1.5.0-dev29 (installed DLL, metadata and all 101 assets match; it carries 1.5.0-dev30 and `variant=public`; the installed game is Beta 2) and is what `dist/lt_direct_control` holds. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.5.0-dev29 (development build) — 2026-10-22

**Not seen in game. Development build, never for the Workshop.** Item effect text shows the game's own values instead of "…". Approved by the author on 2026-10-22 ("go ahead with 1.5.0-dev29"; earlier: "A for both").

- **Seen in game (1.5.0-dev28 on Beta 2, the author, 2026-10-21/22):** minimap, outlines and native skill tooltips look correct; Bombardier "all looks correct except the preview" (no table entry; its skills are built-in `Native` effects). Reported in the same match: item effect values shown as "…", and skill health parts not worked out.
- **Skill health parts stay as the game writes them** (the author's choice, option A): whose health a skill means is not in its text or data (Voodoo Shaman R uses the enemies'). Recorded in docs/backlog.md, Declined.
- **Item effect text from the game (all three versions):** the item sheet from Assets, the game's list of item objects built from it, the item found by its key, its effect text asked of the item itself, then the list released the game's way (its drop, then the buffer from the process heap). The same path as the game's own item panel; no new hook (tooltips family). Only items of a reviewed item type are asked (20 on 0.6.4, 17 on 0.6.5, 20 on Beta 2; key and text methods guarded); a list whose shape is not believable is never handed back to the game. Read from the code: the item-specific text methods stop the game if the item text sheet is missing; the mod asks only after the SDK found that exact text, which rules this out (docs/investigation-item-text.md).
- **When:** only for an item whose effect sentence has a value slot; Workshop items with their own description keep it. All waiting items are answered from one list in the viewer callback; results are cached per item, language and match. The game's text is used when it has words and no value slot left, otherwise the sentence shows as before. Log: `NATIVE ITEM_TEXT key=… source=game` or `fallback reason=…`; the text at Verbose (`NATIVE ITEM_TEXT_TEXT`).
- **Tables:** `TOOLTIP_ITEM_*` in each version's `layout.rs` (getter, list builder, list drop, key and text slots, reviewed item tables) with byte and pointer guards; 44/38/44 new tooltip anchors in the profiles (0.6.4/0.6.5/Beta 2); the verifier links each constant to its anchor and each reviewed table to its two guarded methods (it refused a wrong list builder and a wrong table address). Generator: `research/item-text/gen_item_text.py`.
- **Tests:** the item request queue (once per item, all together, never past the match or a language change); the fill rule; the list goes back to the game's drop only when believable, and items of an unknown table are never asked.
- **Docs:** investigation-item-text.md (new); backlog (item text; health parts declined); migration-0.7.0b2.md (dev28 seen in game, Bombardier).
- Verified: formatting and Clippy with warnings denied for both builds; 598 passing tests in the public build and 630 in the research build (one existing translation-export helper intentionally ignored in each); 40 passing tool tests; both release builds; the native profiles of 0.6.4 / 0.6.5 / 0.7.0 Beta 2, each against its own executable (451 / 443 / 449 tooltip guards); all 619 native instruction/header anchors counted against the installed game (Beta 2), DLL interface/null-host refusal and the 103-file package, for each variant. The research build is verified and packaged, not installed. The public build is installed over 1.5.0-dev28 (installed DLL, metadata and all 101 assets match; it carries 1.5.0-dev29 and `variant=public`; the installed game is Beta 2) and is what `dist/lt_direct_control` holds. Not verified: anything in game, including the game's item text on 0.6.4 and 0.6.5. Not committed, pushed or uploaded.

## 1.5.0-dev28 (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** Game 0.7.0 Beta 2, second stage: the minimap, the outline and the native ability tooltips. Approved by the author on 2026-10-21 ("A, go ahead with 1.5.0-dev28": the research skill reader gets its own stage later).

- **Beta 2 families now on:** session, camera, minimap, movement, combat, shop, outline, tooltips, stats. Only the research skill reader (research builds) stays off. Read from the code (docs/migration-0.7.0b2.md):
  - **Minimap:** all 54 operands are the same instructions with the same values; every constant they use has as many uses in the minimap renderer and the input handler as on 0.6.5, so those two functions gained no use the mod leaves out (a console minimap drawn elsewhere was not looked at, and is not touched).
  - **Outline:** the unit renderer `0x2b641b0` (its 15 callers are the 15 hooked sites; 8 arguments at each); shader, float and colour setters confirmed by their calls inside the renderer; the command drop and its table identical to 0.6.5's; command size and Sprite fields unchanged. With the outline on, the outline hook also records each unit's drawn body for click selection (as on 0.6.4/0.6.5), keyed by the view's unit id at `+0x100`, which no code checked reads: if it never matches, selection falls back to the areas dev27 used; if it matched another unit, that unit's body would be used.
  - **Tooltips:** sheet getter `0x3a39a0` (the draft's pick was wrong: one caller instead of 24), info lookup and Arc drop found again; all 65 champion-info tables with Beta 2's sizes (two grew by 0x20: mod champions 0xb38, data-defined champions 0x9f0; the draft had kept the old sizes); the view-assets guard found by hand.
- **Shared code:** the tooltip reader's check that the game's Assets context is readable used a size written in the shared code (0x70). Beta 2's context grew (the game reads up to `+0x4c0`), so the size is now `TOOLTIP_ASSETS_READ_SIZE` in each version's table: 0.6.4/0.6.5 keep 0x70, Beta 2 0x4c0. Side finding, unchanged: on 0.6.5 the game already read up to `+0x148`.
- **Tables:** `v070b2/layout.rs` (outline and tooltip blocks), `identity.rs` (`ENABLED`), `tools/native_profiles/0.7.0b2.json` (reviewed minimap section, `SHADER_PROLOGUE`, 405 tooltip anchors); every byte read from the Beta 2 executable by the hand-check generator, which asserts each call target and caller count. The draft (research folder) was corrected too.
- **Tests:** the minimap reach test covers Beta 2; each version's Assets size covers the fields its sheet getter reads.
- **Docs:** migration-0.7.0b2.md (the three families, what is not settled, lessons); version-migration.md (objects the game passes can grow; sizes stored in data are not carried by the draft).
- Verified: formatting and Clippy with warnings denied for both builds; 593 passing tests in the public build and 625 in the research build (one existing translation-export helper intentionally ignored in each); 40 passing tool tests; both release builds; the native profiles of 0.6.4 / 0.6.5 / 0.7.0 Beta 2, each against its own executable (Beta 2's now with 79 anchors, 38 layout guards, 405 tooltip guards and the reviewed minimap section); all 575 native instruction/header anchors counted against the installed game (Beta 2), DLL interface/null-host refusal and the 103-file package, for each variant. The research build is verified and packaged, not installed. The public build is installed over 1.5.0-dev27 (installed DLL, metadata and all 101 assets match; it carries 1.5.0-dev28 and `variant=public`; the installed game is Beta 2) and is what `dist/lt_direct_control` holds. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.5.0-dev27 (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** Game 0.7.0 Beta 2 supported next to 0.6.4 and 0.6.5, core families first. Approved by the author on 2026-10-21 ("A, go ahead with 1.5.0-dev27": camera on; earlier: core families first, cancel recall rebuilt the game's way, never touch the console UI).

- **Beta 2 tables (`native_adapter/v070b2/`, `tools/native_profiles/0.7.0b2.json`, docs/migration-0.7.0b2.md), read from the code:** drafted from 0.6.5, then hand-checked. Families on: session, camera, movement, combat, shop, stats. Outline, minimap, tooltips and the research skill reader are off on Beta 2: their entries hold 0 (research effects: values no address can have), never another version's, and the profile names none of their anchors.
- **What Beta 2 changed that the mod follows:**
  - The worker caller calls the channel send as a function (`0xc9dd90`; same arguments and result).
  - The in-game UI's Rust type id is new. It was written in the shared hook code; it is now `layout::INGAME_UI_TYPE_ID` in each version's table (0.6.4/0.6.5 unchanged). Without this the camera family would have handed control back at every match start.
  - No cancel-recall function is left (the game builds the event inline). On Beta 2 the mod builds it the game's way (`CANCEL_RECALL_INLINE`, `movement::push_inline_cancel`): the payload from the process heap (the game's allocator), an entry appended with the game's stop-event function, then turned into the cancel event; nothing is appended if allocation fails.
  - Move refuses while a champion carries `bombardier_deployed` (new champion Bombardier). The mod's own move check has the same rule (`MOVE_BLOCK_STATUS`; none on 0.6.4/0.6.5).
  - Ranges gained an effect method; it returns 0 for all 276 effect types found, so nothing changes (Bombardier to check in game).
  - With the minimap family off, Beta 2 keeps its own minimap look: the mod's minimap frame is drawn only when that family is installed (`native_adapter::minimap_installed`; no change on 0.6.4/0.6.5). Beta 2's own minimap sits where the mod expects (same positions, size and scale, read from its renderer and input code), so clicks there are still recognised as minimap clicks. Tooltips fall back to the mod's own text resolver.
  - Checked and unchanged: the keys the mod lets through during control (Equal, NumpadAdd, NumpadSubtract: same key codes), the config fields the camera writes, and the game frees memory from the same process heap (`HeapFree`).
  - Everything else found again with the same arguments and fields: move, stop, steer, direct step, attack, auto-attack, Q/W/R, aim, the shop chain, item AI, skill-number methods, the 38 layout guards, the 35 stat sites (one Accuracy producer stores at `+0x2c8`).
- **Tests:** the inline cancel event has the game's layout and nothing changes when nothing is appended; the deployed rule; the camera test uses each version's type id; the research reader's decoding tests skip a version whose reader family is off.
- **Docs:** migration-0.7.0b2.md (new); version-migration.md (inlined functions, type ids, new conditions in moved functions; 35 stat sites from 0.6.5); modding.md and lib.rs (supported versions).
- Also carries 1.5.0-dev26's new defaults (dev26 was never installed).
- Verified: formatting and Clippy with warnings denied for both builds; 590 passing tests in the public build and 622 in the research build (one existing translation-export helper intentionally ignored in each); 40 passing tool tests; the verifier refused three deliberately wrong Beta 2 tables (a hook site, a guard byte, an item anchor); both release builds; the native profiles of 0.6.4 / 0.6.5 / 0.7.0 Beta 2, each against its own executable (Beta 2 installed); its 78 anchors, DLL interface/null-host refusal and the 103-file package, for each variant. The research build is verified and packaged, not installed. The public build is installed over 1.5.0-dev25 (rebuilt and reinstalled after the minimap-frame gate, checked against the first install's fingerprints; installed DLL, metadata and all 101 assets match; it carries 1.5.0-dev27 and `variant=public`; the installed game is Beta 2). Not verified: anything in game. Not committed, pushed or uploaded.

## 1.5.0-dev26 (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** New defaults for the athlete stat rules' reaction band. Approved by the author on 2026-10-21 ("go ahead with 1.5.0-dev26"; "keep my current setting as default to follow the base game reaction difference better").

- **Defaults:** Reaction floor 12 ticks (was 6), Reaction ceiling 35 ticks (was 14); Collision margin stays 2; neutral Dodging 50 unchanged (the author: "feels good"). Saved settings are kept: only a controls.json without these values gets the new defaults.
- `athlete_rules::FLOOR_DEFAULT`/`CEILING_DEFAULT` and the settings window's defaults changed together; a test checks that they agree.
- The comment on the unlearned-skill refusal now says it happens on 0.6.4 and 0.6.5 (it said 0.6.5 only).
- Docs: backlog item 15 (new defaults).
- Verified: formatting and Clippy with warnings denied for both builds; 550 passing tests in the public build and 574 in the research build (one existing translation-export helper intentionally ignored in each); 40 passing tool tests; both release builds; the native profiles of 0.6.4 / 0.6.5, each against its preserved copy (the installed game is 0.7.0 Beta 2, which this build does not support), all 583 native instruction/header anchors, DLL interface/null-host refusal and the 103-file package, for each variant. Not installed: the installer refuses while the game is on Beta 2; 1.5.0-dev25 stays installed (it gives Beta 2 no hooks). Not verified: anything in game. Not committed, pushed or uploaded.

## 1.5.0-dev25 (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** The mod never sends a skill that is not learned yet. Approved by the author on 2026-10-21 ("go ahead with 1.5.0-dev25").

- **The author's report (1.5.0-dev24, game 0.6.5):** the game froze halfway through a match; the author pointed at a Crossbowman W cast before level 3.
- **What the log showed:** after a switch to Crossbowman, the game's cooldown read reported its W locked. At tick 5475 the mod sent W with `learned=false`, the game started it (`ABILITY EXECUTED W … action=5`), and the match thread exited at tick 5486 (`WORKER ENDED` with the match unfinished; the screen kept drawing the last frame). The game's own error is not visible, so "casting a locked skill stops the game's match thread" is a strong inference, not proven.
- **Cause (read from the code):** the mod sent a skill whenever the game's own input check (`is_valid_input`) accepted it, and never refused an unlearned one itself. That check accepts a skill not learned yet.
- **Seen in game, 0.6.4 too (the author, 1.5.0-dev24, after this build was made):** Dynamo's W was sent unlearned and cast without harm; Dynamo's R was sent unlearned at about tick 3925 (`ABILITY EXECUTED R … action=6`) and the match stopped producing ticks at 3932. So the bug is old and not specific to 0.6.5: some unlearned skills cast harmlessly, others stop the match. *Superseded the same day:* the first reading said 0.6.5's check had started accepting unlearned skills.
- **Now (`abilities::take_input`):** a press is sent only when the skill is learned by the level table (Q 1, W 3, R 5) and a fresh sample of the game's cooldown read does not report it locked (`observe_locked`, fed from `CooldownV1::unlocked` in `simulation.rs`). A refused press shows the existing "W: Lv 3" message (or the existing "not cast" message when the level is enough but the game says locked); the log gives `reason=unlearned` or `reason=locked-by-game`.
- Tests: a press the game's check would accept is not sent below the level or when locked, and is sent when learned and unlocked (or before any lock reading).
- Verified: formatting and Clippy with warnings denied for both builds; 550 passing tests in the public build and 574 in the research build (one existing translation-export helper intentionally ignored in each); 40 passing tool tests; both release builds; the native profiles of 0.6.4 / 0.6.5, all 581 native instruction/header anchors counted against the installed game (now 0.6.4; 583 when 0.6.5 is installed), DLL interface/null-host refusal and the 103-file package, for each variant. The research build is verified and packaged, not installed. The public build is installed over 1.5.0-dev24 (installed DLL, metadata and all 101 assets match; it carries 1.5.0-dev25 and `variant=public`; the installed game is 0.6.4) and is what `dist/lt_direct_control` holds. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.5.0-dev24 (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** With the athlete stat rules On, your champion's body counts as a neutral Dodging 50 for enemy shots, instead of 0. Approved by the author on 2026-10-21 ("go ahead with 1.5.0-dev24, A").

- **Why (the author):** your own Dodging does nothing for you in a match, so your hitbox should stay at the neutral middle ground. With 0, enemy shots got their full Accuracy room against you (Accuracy × 0.07 units), making you easier to hit than any real athlete. Superseded decision of 2026-10-15: "the player's own Dodging term is zero".
- **Now:** the Dodging producer writes `NEUTRAL_DODGING` (50) times the version's factor (`STAT_DODGING_FACTOR`: 150 on 0.6.4, 200 on 0.6.5) for the controlled athlete's entity. Enemy shots at you get their Accuracy × 0.07 minus 10 units (0.6.5; 7.5 on 0.6.4), then the game's own floor (0 or 5 units by the kind of hit check). Your Dodging stat itself still plays no part. AI against AI is unchanged.
- **Read from the code (0.6.5):** besides the 21 hit checks, the derived Dodging field is read only to choose between two counters (`0x1300825`, `0x176a885`) and copied with whole entities (`0x10894db`, `0x16544b9`); no other gameplay use.
- `STATRULE dodging: derived Dodging of athlete N set to neutral (Dodging 50)` replaces the "set to 0" line.
- The verifier checks `STAT_DODGING_FACTOR` against the Dodging producer's `imul` (a wrong factor is refused).
- Tests: your entity gets the neutral value; another athlete's keeps the game's.
- Docs: backlog item 15 (the decision superseded, with the reason).
- Verified: formatting and Clippy with warnings denied for both builds; 549 passing tests in the public build and 573 in the research build (one existing translation-export helper intentionally ignored in each); 40 passing tool tests; both release builds; the native profiles of 0.6.4 / 0.6.5, all 583 native instruction/header anchors, DLL interface/null-host refusal and the 103-file package, for each variant. The research build is verified and packaged, not installed. The public build is installed over 1.5.0-dev23 (installed DLL, metadata and all 101 assets match; it carries 1.5.0-dev24 and `variant=public`) and is what `dist/lt_direct_control` holds. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.5.0-dev23 (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** The athlete stat rules are back on game 0.6.5. Approved by the author on 2026-10-21 ("go ahead with 1.5.0-dev23"; earlier choices: effective values, hook the fifth gate).

- **Read from the code (docs/migration-0.6.5.md, "How the rules hook 0.6.5"):** 0.6.5's reaction gates roll their delay in a band (`low + (range + 1) × splitmix(seed) / 2^64 + extra`) and compare it at once; in three of the five gates no safe place is left to replace the final value as on 0.6.4.
- **New gate hook (`athlete_rules::Kind::Inputs` and `Kind::Band`):**
  - `Inputs` records the caster's effective Accuracy and the defender's effective Dodging (the value itself, or `100 − Dodging` from the frame) when the caster is the controlled athlete.
  - `Band`, at the hash constant, sets the gate's `low` and `range` to the human band and `extra` to 0, so the game's own roll lands in it (`rolled_gate`).
  - 0.6.4 keeps its `Capture`/`Complete` sites unchanged.
- **Five gates hooked on 0.6.5:** projectile, the big AI function's two paths, area, and the fifth gate (`0xf9fb30`, new in 0.6.5 to read Accuracy).
  - Corrections to the plan, found while decoding: in both AI paths the `mov reg, 100` near the hash caps Accuracy, so their inputs are read just after Accuracy is final (`0xfa4cb9`, `0xfa50ea`). The fifth gate's are read at `0xfa1d2c`, before the record holding the player seed is overwritten.
  - The plan said 33 sites; it is 35: 10 gate sites, 3 Accuracy producers, 1 Dodging producer, 21 hit tests.
- **Producers and seed:**
  - The projectile Accuracy producer stores at frame `+0xa8` (the draft had 0.6.4's `+0xa0`).
  - The Dodging producer is `imul rcx, rsi, 200` (the draft had 0.6.4's bytes).
  - `STAT_PLAYER_SEED` is `0x818`.
  - `ENABLED` includes Stats on 0.6.5, and the profile reviews the 35 sites (`STATS_SITE_0..34`, `stats` family).
- **Log:** the per-shot `STATRULE gate site=…` line (Verbose) now names the gate's site, so the fifth gate's lines can be told apart; the game's own gate is still shown next to the band's. `logging.rs` lists the line by its new start, and a test pins it at Verbose (the first-fire and `installed` lines stay Normal).
- **Verifier and census (`tools/verify_stat_sites.py`):**
  - It derives every field of `Inputs` and `Band` from the code: Accuracy from the register the gate squares; Dodging from `mov 100; sub` or the frame slot it multiplies with; the caster from the Accuracy lookup's id, followed along every path to the site.
  - It refuses an `Inputs`/`Band` pair with a call or a branch in or out between them.
  - A branch onto the first instruction of a producer is now allowed (a jump lands on all three 0.6.5 Accuracy producers; it runs the hook, which needs nothing before it); a branch into the middle of any site is still refused.
  - The player-seed offset comes from the table instead of a fixed `0x7f8`; the census also knows `× 200` and band-shaped gates (`draft_version.py` reports them).
  - Checked against eight deliberately wrong tables (each refused) and the plan's original AI capture spot (refused: a jump lands inside that gate).
- **Tests:**
  - the game's roll in the human band gives the mod's tick;
  - both forms of Dodging give the same gap;
  - every Inputs/Band pair of the table, driven through the hook with fake registers: the controlled caster's band registers get the band and the seed is kept; another caster's registers are left alone;
  - tests that change the shared rule state now take a lock.
- **Docs:** migration-0.6.5.md (how the rules hook 0.6.5); version-migration.md (the two gate shapes, the census, the branch rule); modding.md; investigation-stat-rules-0.6.4.md (pointer); backlog item 15 (on again in 1.5.0-dev23; the author's later refinement: bring the manual average hit rate closer to 0.6.5's own calculation).
- Verified: formatting and Clippy with warnings denied for both builds; 547 passing tests in the public build and 571 in the research build (one existing translation-export helper intentionally ignored in each); 40 passing tool tests; both release builds; the native profiles of 0.6.4 / 0.6.5 (now with 0.6.5's 35 stat sites), all 583 native instruction/header anchors, DLL interface/null-host refusal and the 103-file package, for each variant. The research build is verified and packaged, not installed. The public build is installed (first over 1.5.0-dev22b; rebuilt and reinstalled after the log-level fix, checked against the first install's fingerprints; installed DLL, metadata and all 101 assets match; it carries 1.5.0-dev23 and `variant=public`; the installed game is 0.6.5) and is what `dist/lt_direct_control` holds. Not verified: anything in game, including whether each gate fires and what the fifth gate covers. Not committed, pushed or uploaded.

## 1.5.0-dev22b (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** On 0.6.5 the screen shows the live simulation again. Approved by the author on 2026-10-21 ("go ahead with 1.5.0-dev22b, always remove the delay").

- **The author's report (1.5.0-dev22, game 0.6.5):**
  - the controlled champion walked on its own, back and forth near the nexus;
  - no outline on minions, champions and some monsters ("what I see is not actually what's going on in the simulation");
  - the shop opened by itself at 1:48.
- **What the verbose log showed:** the controlled athlete's input ran 900 ticks (15 s) ahead of the frames shown; for seconds the mod's own trace had the champion standing still while the screen showed it walking.
- **Read from the code:** 0.6.5 delays the match on screen. The hooked worker caller builds a frame queue `15 × [settings + 0x1308]` frames long and releases each frame only once the queue is full; probably so match chat can comment on what is about to happen (guess). The player's orders act on the live simulation, so they showed up 15 s later; hover and outlines matched units that had already moved or died; and before the first frame was shown the simulation ran 15 s unpaced. The second worker send found in dev22 is this queue's drain.
- **Now (`VIEW_DELAY_SITES`, session family; 0.6.5 only):** the queue length load at `0xb074cd` (`mov rdx, [rax + 0x1308]`) becomes `xor edx, edx` and a 5-byte nop, so every frame goes to the screen at once and the mod's pacing works as on 0.6.4.
  - It is done for every match on screen while the mod is installed (the author's choice): match chat loses its head start even when you only watch.
  - The game's bytes are checked before the change and read back after it. The profile names the site (`VIEW_DELAY_BYTES_0`) and the verifier checks the table against it. A more graceful answer is backlog item 18 (low priority).
- Docs: migration-0.6.5.md "The screen delay"; backlog item 18.
- Tests: a delay rewrite keeps the instruction length and is `xor edx, edx` plus a nop.
- Verified: formatting and Clippy with warnings denied for both builds; 543 passing tests in the public build and 567 in the research build (one existing translation-export helper intentionally ignored in each); 40 passing tool tests; both release builds; the native profiles of 0.6.4 / 0.6.5, all 548 native instruction/header anchors, DLL interface/null-host refusal and the 103-file package, for each variant. The research build is verified and packaged, not installed. The public build is installed over 1.5.0-dev22 (installed DLL, metadata and all 101 assets match; it carries 1.5.0-dev22b and `variant=public`) and is what `dist/lt_direct_control` holds. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.5.0-dev22 (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** Game 0.6.5 supported next to 0.6.4; 0.6.3 and 0.7.0 Beta 1 dropped; skill charges from the game's official cooldown read. Approved by the author on 2026-10-21 ("go ahead, effective, hook it, two builds"): the stat rules come back on 0.6.5 in 1.5.0-dev23.

- **Why:** game 0.6.5 is the live branch; 0.6.4 stays as Steam's rollback. 0.6.3 and 0.7.0 Beta 1 offer only ABI 9, and the author chose the ABI 10 cooldown read over keeping them. 0.7.0 Beta 2 (ABI 10) follows once its hand check is done.
- **0.6.5 tables (`native_adapter/v065/`, `tools/native_profiles/0.6.5.json`), read from the code** (docs/migration-0.6.5.md): drafted from 0.6.4. A function map seeded with the draft's exact pairs grew from 682 to 1216 functions; all 483 carried code addresses land exactly by alignment; the 69 unknown addresses were found by hand (callers and callees, byte searches, method tables, aligned call order). The draft's `next_step` candidate was wrong (both shop decisions call `0xf0c160`).
- **What 0.6.5 changed that the hooks rely on:**
  - The player record grew by 0x20 bytes: the shop's owned-item and build-plan fields became per-version constants (`PLAYER_ITEMS_*`, `PLAYER_PLAN_*`); gold and the item AI's settings owner moved too (byte guards).
  - The simulation's event list changed format; the mod only calls the game's own stop and cancel-recall functions, so no change.
  - The worker's caller now also sends from a drain loop over new code: both sends go to the same hook (author's choice, `WORKER_EXTRA_SITES`), and the profile and the verifier name the second one.
  - The unit renderer has 15 callers (one duplicate caller gone).
  - The outline setters and the tooltip lookup grew their stack frames but take the same arguments.
- **Charges from the official read (ABI 10):** `simulation.rs` reads each skill's `CooldownV1` (cycle after cooldown reduction, maximum charges, locked or not) for your athlete. `abilities.rs` keeps the champion's action and the charges as two samples. The combat hook's own charge reader is gone: two native skill-method calls, the cooldown-reduction fields and its own arithmetic. The SDK copy is now 0.6.5's (`tools/prepare_sdk.py`).
- **Stat rules off on 0.6.5 in this build:** 0.6.5 scales Accuracy and Dodging by a per-athlete slump factor, multiplies Dodging by 200 and reworked the reaction gates (details in docs/migration-0.6.5.md). `v065/stat_sites.rs` holds labelled placeholders; `ENABLED` leaves Stats out and the profile reviews no stat site. On 0.6.4 the rules are unchanged.
- **Removed:** `v063/`, `v070/` and their profiles. `mod.mod_info` now needs base `>=0.6.4`. Tools that named the 0.6.3 or 0.7.0 profile accept any supported profile (`audit_tooltips.py`, `read_acquisition_assets.py`) or use 0.6.4/0.6.5 (`verify_native_profile.py` default, `test_native_profile.py`).
- **Docs:** new docs/migration-0.6.5.md; version-migration.md (seeding the map, the stat census's blind spots, player-record fields, extra worker sends); modding.md (supported versions); backlog item 15 (the stat rules on 0.6.5).
- Tests: the extra worker sends call the same original; charges built from the official read (a 3-use 360-tick slot, a no-cooldown slot, a locked slot); the background-worker test now expects the live hook to leave the official charges alone.
- Verified: formatting and Clippy with warnings denied for both builds; 543 passing tests in the public build and 567 in the research build (one existing translation-export helper intentionally ignored in each; fewer than dev21 because the per-version hook tests now run for two game versions, not three); 40 passing tool tests; both release builds; the native profiles of 0.6.4 / 0.6.5, all 547 native instruction/header anchors, DLL interface/null-host refusal and the 103-file package, for each variant. The research build is verified and packaged (`dist/build-1.5.0-dev22-research.json`), not installed. The public build is installed over verified 1.5.0-dev21 (installed DLL, metadata and all 101 assets match; it carries 1.5.0-dev22 and `variant=public`) and is what `dist/lt_direct_control` holds. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.5.0-dev21 (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** The plain left-click works during a pause. Approved by the author on 2026-10-21 ("every pause").

- **The author's report (1.5.0-dev20):** a left-click outside the shop closed it only with the shop's "Pause when open" option off. **Read from the code:** battlefield clicks are handled only while the match runs; during any pause the click handling stopped before the left-click, and each paused frame also emptied the list of units a click can hit. So during a pause neither closing the shop nor selecting a unit for the stats panel worked.
- **Mod (`movement::plain_click`, `client::gameplay_input`):** during a pause (the shop's own included), the plain left-click is handled: on a unit it selects it for the stats panel, on empty ground it closes an open shop and keeps the selection, or clears the selection without a shop, the same as while running. The unit list from before the pause is kept (units do not move while paused). Every other order stays off during a pause, as before: moving, attack-move, skills and recall.
- Tests: a pause keeps the clickable units, and any other inactive state clears them as before.
- Verified: formatting and Clippy with warnings denied for both builds; 577 passing tests in the public build and 609 in the research build (one existing translation-export helper intentionally ignored in each); 40 passing tool tests; both release builds; the native profiles of 0.6.3 / 0.6.4 / 0.7.0, all 581 native instruction/header anchors, DLL interface/null-host refusal and the 103-file package, for each variant. The research build is verified and packaged (`dist/build-1.5.0-dev21-research.json`), not installed. The public build is installed over verified 1.5.0-dev20 (installed DLL, metadata and all 101 assets match; it carries 1.5.0-dev21 and `variant=public`) and is what `dist/lt_direct_control` holds. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.5.0-dev20 (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** Log levels tidied, and a left-click on empty ground closes the shop. Approved by the author on 2026-10-21.

- **What the 1.5.0-dev19 log showed (the author's matches as Invincible, game 0.6.4):** `variant=public`, the table and all 14 assignments in use, Invincible R's numbers read in each of three matches, no preview refusals, no errors. Ten `CAST TIMING W outcome=refused` lines (the game did not spend W's cooldown) come from the existing cast log, not from this work.
- **The author's request (logging):** some newly added lines ignored the log level. **Read from the code and the author's Normal logs:** a line's level comes from its first words (`logging.rs`), and a tag not listed counts as Normal, so new tags landed at Normal unless listed. Three short matches gave 522 Normal lines, most of them per-press and per-purchase traces. Now Verbose: successful presses in `CAST TIMING` (`outcome=cast`; split by outcome, the author's choice after I found that 1.4.1 had made the line Normal on purpose for the buffering investigation: denied, refused, cancelled and dropped presses stay at Normal, including the mod's own refusals before the game is asked, which have no `ABILITY REJECTED` line), `ABILITY CANCEL`, `SHOP PURCHASE` (every purchase by all ten champions), `STATS own raw` (every 10 s), the preview start-up and per-match diagnostics (`PREVIEW FILE`, `PREVIEW BASE`, `PREVIEW declared_slots`, `PREVIEW ROSTER`, `PREVIEW DECLARED`) and the research build's traces (`PREVIEW TREE`, `PREVIEW COVERAGE`, `SHAPE RECORD`, `SHAPE NUMBERS`, `PATCH FIELDS`; the shape table is still written whatever the level). Still Normal: `CAST TIMING` presses that went wrong, `ABILITY REJECTED`, `PREVIEW TABLE`, `PREVIEW OVERRIDES`, `PREVIEW NAMES`, `PREVIEW COMPARE`, `SHOP STATE`, `SHOP UI`. The same three matches would give 262 Normal lines instead of 522. `docs/modding.md` says when a new tag belongs at Verbose; `tools/audit_previews.py` and `docs/version-migration.md` say the audit needs Verbose.
- **The author's request (shop):** a left-click outside the shop closes it, only when the click means nothing else. **Read from the code:** a left-click already belongs to the minimap, an armed attack-move (A + click), an aimed skill, a unit (the stats panel), the mod's windows and buttons (HUD skill, item and shop buttons, stats panel, Tab view, athlete portraits, settings, emote wheel) and the game's own on-screen buttons; the shop's own window keeps its clicks. Only a plain left-click on empty ground was left, which cleared the stats panel's selection. **Now (`movement::plain_left_click`, `ui_state::SHOP_CLOSE`, `shop_ui`):** with the shop open, that click closes the shop and keeps the selection (the author's choice); a second click clears it as before. Right-click still moves the champion with the shop open. `SHOP UI closed by a left-click outside it` in the log.
- Tests: the busiest tags' levels are pinned (Verbose: successful `CAST TIMING` presses, `ABILITY CANCEL`, `SHOP PURCHASE`, `STATS own raw`, `PREVIEW FILE`, `PREVIEW TREE`, `SHAPE NUMBERS`, `PATCH FIELDS`; Normal: denied, refused and cancelled `CAST TIMING` presses, `ABILITY REJECTED`, `PREVIEW NAMES`, `PREVIEW TABLE`, `PREVIEW COMPARE`, `SHOP STATE`); empty ground closes an open shop and keeps the selection, a unit is selected with the shop open or closed.
- Verified: formatting and Clippy with warnings denied for both builds; 576 passing tests in the public build and 608 in the research build (one existing translation-export helper intentionally ignored in each); 40 passing tool tests; both release builds; the native profiles of 0.6.3 / 0.6.4 / 0.7.0, all 581 native instruction/header anchors, DLL interface/null-host refusal and the 103-file package, for each variant. The research build is verified and packaged (`dist/build-1.5.0-dev20-research.json`), not installed. The public build is installed over verified 1.5.0-dev19 (installed DLL, metadata and all 101 assets match; it carries 1.5.0-dev20 and `variant=public`) and is what `dist/lt_direct_control` holds. Counted on the author's dev19 log: 262 of its 522 lines would be written at Normal now. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.5.0-dev19 (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** Areas that go with a move are drawn where the caster lands, and the table carries the author's last picks. Approved by the author on 2026-10-21.

- **What the 1.5.0-dev18 public log showed (the author's matches, game 0.6.4):** `variant=public`, hook families `0x37f` (all but the readers family), `PREVIEW TABLE champions=96`, all 14 assignments from the table, numbers read for Pole Warrior W, Harpy W and Sun Mage Q and W with no refusals, and no research lines at all. The research build was not run in game (the author: the same as 1.5.0-dev17 for recording).
- **The author's report:** Pole Warrior W was read as an area around the caster, but it works like League's Fizz E: she moves to the aimed point, then damages around where she lands, so "Circle at aim" is right (the author's assignment, shipped since 1.5.0-dev17, looked right in game). **Read from the recorded table and old logs:** the live reader drew a move to the aimed point (`MoveTo`) and a move onto the target (`MoveToTarget`) as the movement only, without the effects they apply on arrival (Nightmare W's 40 circle), and drew an area beside such a move around the caster (Pole Warrior W 32). The champion-file reader already put a move's own `end_effects` where it lands, but not an area beside the move (Invincible R, a Workshop champion cast on a unit: a 46 slam after `MoveToTarget`). Only these two recorded skills had a move to the aim with a circle around the caster.
- **Rule (both readers):** in a list of effects holding a move to the aimed point or onto the target unit (`MoveTo`, `MoveToTarget`, `Teleport` in champion files; the live `MoveTo`, `MoveToTarget` and rush to the clicked unit), a circle around the caster from the other effects is around where the caster lands; so are the circles a live move applies on arrival. A line or cone keeps its own placement, as the file reader already did for `end_effects`. Live reader: research builds (`windows/preview.rs`); file reader: every build (`skill_preview::collect_list`, `to_landing`), so a Workshop champion not in the table follows it too.
- **Recorded shapes (`tools/preview_sources.py` `landed`, the viewer):** shapes recorded before this build get the same rule when the table is written and in the viewer (a circle around the caster beside a move to the aim); the recording itself is left alone. Invincible R in the table: the slam around the target, its radius from the game's `radius`.
- **Table:** regenerated with the author's picks (Hellion R and Mistguide R: `radius`, saved after the 1.5.0-dev17 table was written and missed by 1.5.0-dev18; no visible change, both 1000). Sizes: 131 by name, 46 the cast range, 74 fixed, 4 picked, none left to pick.
- Tests: an area beside a live move and a live move's arrival circle land at the aim, and stay around the caster without a move (live reader, research build); Invincible R's slam lands around the target, and stays around the caster without a move (file reader); recorded circles beside a move to the aim are placed where the caster lands, lines and lone circles are not (tools).
- Verified: formatting and Clippy with warnings denied for both builds; 575 passing tests in the public build and 607 in the research build (one existing translation-export helper intentionally ignored in each); 40 passing tool tests; both release builds; the native profiles of 0.6.3 / 0.6.4 / 0.7.0, all 581 native instruction/header anchors, DLL interface/null-host refusal and the 103-file package, for each variant. The research build is verified and packaged (`dist/build-1.5.0-dev19-research.json`), not installed. The public build is installed over verified 1.5.0-dev18 (installed DLL, metadata and all 101 assets match; it carries 1.5.0-dev19 and `variant=public`) and is what `dist/lt_direct_control` holds. Not verified: anything in game; the live reader's rule in a recording (research build not run). Not committed, pushed or uploaded.

## 1.5.0-dev18 (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** Stage C of step 4 of the preview plan: public and research builds. Approved by the author on 2026-10-21.

- **What the 1.5.0-dev17 log showed (the author's match, game 0.6.4):** `PREVIEW TABLE champions=96 skills=233`; all 14 of the author's assignments came from the table (`shipped=14`); every skill read gave its numbers (Sun Mage Q, Void Mage Q, Hammerer W), none refused. One difference from the live reader in the session: Hammerer W, whose hit detection range the author had set to 150, was drawn at its recorded 60 while not yet learned (its numbers cannot be read until then), and at 150 once learned (`read 1 of 1`). Every other skill drew the same shape both ways.
- **Build switch (`--features research`, `probe/Cargo.toml`):** a public build leaves out the live skill reader (`windows/preview.rs`), its per-version tables (`v*/effects.rs`), the shape recorder (`shape_record`), the numbers log (`patch_fields`) and the research code of the combat hook, now in `windows/research.rs` (effect-tree inventory, shape and number recording, the live reader's preview beside the table and `PREVIEW COMPARE`); it does not install the readers family, so its guards are not checked. A research build keeps all of it. Drawing is the same in both: the shipped table, sized by the game's numbers by name. The INIT line says `variant=public` or `variant=research`.
- **Guards:** the skill method guards (Hammerer W's table, three positions and three code guards per game version) now belong to the combat family, which calls those methods, so a public build checks them; the live reader's pointer guards stay with the readers family (`Guards::pointer_family`). The profile verifier passes for 0.6.3, 0.6.4 and 0.7.0.
- **`PREVIEW COMPARE`** (research builds) is logged only once a skill's numbers are read, so an edit made before the skill is learned no longer shows as a difference.
- **Tools:** `stage_package.py`, `verify_build.py` and `package_mod.py` read the variant from the DLL; a research build's record and package are named `<version>-research` (install with `-Version <version>-research`).
- **Docs:** `docs/modding.md` (the preview table, "Add a champion's preview"), `docs/version-migration.md` (the readers family is research-only; carrying the skill method guards to a new game version; building a research build), `docs/backlog.md` item 2 (the "only a few radius or width values" line superseded; step 4 done in stages).
- Tests: a pointer guard listed with the combat family is checked without the live reader; the others belong to the readers family. The research build runs every test; the public build runs all but the research code's own.
- Verified: formatting and Clippy with warnings denied for both builds; 574 passing tests in the public build and 603 in the research build (one existing translation-export helper intentionally ignored in each); 39 passing tool tests; both release builds (the public DLL 5.35 MB, the research DLL larger; staging recognised each variant); the native profiles of 0.6.3 / 0.6.4 / 0.7.0, all 581 native instruction/header anchors, DLL interface/null-host refusal and the 103-file package, for each variant. The public build is installed over verified 1.5.0-dev17 (installed DLL, metadata and all 101 assets match; it carries 1.5.0-dev18 and `variant=public`, not `variant=research`). The research build is verified and packaged (`dist/build-1.5.0-dev18-research.json`, `LTDirectControl-1.5.0-dev18-research.zip`) and is what `dist/lt_direct_control` holds now; not installed. Not verified: anything in game; the public build without the readers family on 0.6.3 and 0.7.0 (verifier only). Not committed, pushed or uploaded.

## 1.5.0-dev17 (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** Stage B of step 4 of the preview plan: previews drawn from the shipped table, sized by the game's own numbers by name. Approved by the author on 2026-10-21.

- **What the 1.5.0-dev16 sessions showed (the author left the game running for hours, then played a 3-minute match):** every recorded skill got its numbers, Amazon R and Yang R included; no refusals, no errors. The new "no skill object yet" line never appeared, so the guessed cause of the missed ultimates was not confirmed (the dev15 log that could have told was overwritten); the fix stays, harmless.
- **The table (`probe/src/preview_table.json`, compiled in; written by `tools/build_preview_table.py`):** 96 champions. Sizes: 131 from a game number by name, 46 from the live cast range, 74 fixed (projectile widths, offsets and numbers the game does not hold by name), 2 the author picked (Alchemist R and Void Mage Q: `projectile_radius`) and 2 still to pick (Hellion R and Mistguide R, kept at their recorded 1000). The author's assignments are in it, with their sizes from the game's numbers where one fits. Dancer R's fan carries its most blades (10, from the game's champion sheet).
- **Mod (`preview_table`, `skill_numbers::named`, `combat`):** for the controlled athlete, each skill with a table entry is drawn from it: its shapes (or the author's assignment) with each size from the skill's own number by name (read once per match, once learned), the live cast range, or a fixed number; a number that cannot be read uses its recorded value; Dancer R's blades add her stack up to the most. Skills without an entry use the live reader as before. Before a skill's numbers are used, its name must be a short lowercase identifier and its `cooltime` must equal what its cooldown method returns; otherwise its recorded values are used (`PREVIEW NAMES … refused=…`, once per skill and match; `read N of M` when it works). The live reader still runs in this development stage, and `PREVIEW COMPARE` logs, once per skill and match, where its shapes differ from the table's.
- **Per game version (verifier and start-up guards):** Hammerer W's skill method table must hold the name, get-by-name and cooldown methods at +0x18, +0x20 and +0x90, and those methods must start with their reviewed bytes (three position and three code guards per version, in `identity.rs` and `tools/native_profiles`; positions in `layout.rs` as `SKILL_PATCH_NAME_SLOT` and `SKILL_PATCH_GET_SLOT`). Found for 0.6.3 (table 0x3ba3e08), 0.6.4 (0x3bb8168) and 0.7.0 (0x3ed3ee0); the profile verifier passes for all three. They are checked with the live reader's guards for now; stage C gives them their own place.
- **The author's own assignment file:** an entry the shipped table already holds with the same values is skipped, so the shipped copy (whose sizes follow the game's numbers) is drawn (`PREVIEW OVERRIDES … shipped=N`). A shipped assignment wins even when it draws nothing ("no preview", or a unit target without an area). At start-up: `PREVIEW TABLE champions=… skills=…`.
- Tests: sizes follow the game's numbers and fall back to their recorded values (Hammerer W 150 after an edit, 62 recorded); lengths follow the live cast range and widths scale; Dancer's blades grow with her stack up to the most; assignments take their sources too; names and recorded assignments; the shipped table parses; a shipped assignment that draws nothing wins over the champion file.
- **Tool (`tools/stage_package.py`):** the stale-DLL check now also counts the data compiled into the DLL (the shipped table and other JSON in `probe/src`, the translations and the per-version native profiles), so a table written after the last build can no longer ship with an old DLL (the 1.4.5/1.4.6 failure). Found in review before staging.
- **Read from the author's newest save:** Hammerer W's hit detection range is 60 (62 in the patched copies); the 620 seen in the dev13 log is no longer there.
- Verified: formatting, Clippy with warnings denied, 602 passing tests (one existing translation-export helper intentionally ignored), 39 passing tool tests, release build (staging confirmed the DLL carries 1.5.0-dev17), the native profiles of 0.6.3 / 0.6.4 / 0.7.0 with the new guards, all 581 native instruction/header anchors, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.5.0-dev16; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.5.0-dev17. Record: `dist/build-1.5.0-dev17.json`. Not verified: anything in game; the refusal path (a `cooltime` that differs) has not run; on 0.6.3 and 0.7.0 only the verifier ran. The new guards are checked only while the live reader's family is installed; stage C moves them. Not committed, pushed or uploaded.

## 1.5.0-dev16 (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** A fix to stage A's recording, approved by the author on 2026-10-21. Nothing the player sees or plays changes.

- **The author's report (1.5.0-dev15):** Amazon's R still showed "numbers not recorded yet" after a match against her. **Read from the log and the match record:** Amazon reached level 15, her Q and W numbers were recorded, and her R left no line at all, not even a refusal; Yang the same. About half of the 38 skills still waiting were ultimates whose Q and W had been recorded. 213 skills were recorded in that session, none refused, no crash.
- **Cause (read from the code):** each skill was checked once per match as soon as the level allowed it (Q 1, W 3, R 5), and marked as checked before its skill object was looked at; a champion that learns R later than level 5 has no R object yet then, so the skill was skipped quietly and not tried again in that match.
- **Mod (`shape_record::fields_wanted`, `fields_done`, `first_unlearned`; `skill_numbers`):** a skill counts as checked only once its numbers were read (or refused for good); until then it is tried again on later attacks. One `PATCH FIELDS … waiting=no skill object yet (level N)` line per match and skill shows when this happens.
- Tests: a skill is offered again until it has been read in this match, the waiting line is written once, and a new match starts over.
- Verified: formatting, Clippy with warnings denied, 596 passing tests (one existing translation-export helper intentionally ignored), 39 passing tool tests, release build (staging confirmed the DLL carries 1.5.0-dev16), all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.5.0-dev15; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.5.0-dev16. Record: `dist/build-1.5.0-dev16.json`. Not verified: anything in game, including that late-learned ultimates are now recorded. Not committed, pushed or uploaded.

## 1.5.0-dev15 (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** Stage A of step 4 of the preview plan (draw from a shipped table and the game's named numbers), approved by the author on 2026-10-21 with the table compiled into the mod. Nothing the player sees or plays changes.

- **What the 1.5.0-dev14 log showed (the author's match, game 0.6.4):** no crash; 210 skills, none refused. Workshop skills answer size names from their effects: 31 of the 44 sizes recorded for them come back by name (radius, projectile radius, width); the others are projectile lines longer than their cast range, the champion author's own code (which answers many names with 0) and Sun Mage W's second circle. **Counted again with a stricter rule** (a size is a number only if that number is a distance in the patch rules and no other has the same value): compiled base champions 80 of 108 sizes from one name, 2 where two fit, 11 fixed, 15 without a number; Workshop 28 of 44, 3 where two fit. **Read from the code:** the live reader has 20-21 effect addresses and 25-30 "no area" entries per game version (about 50; "about 37" earlier was wrong), and only Dancer R's blade count depends on the match (her kill stack, read from the champion). The plan: `local/plans/skill-preview-step4.md` (not in git).
- **Mod (`shape_record`, `native_adapter/windows/skill_numbers.rs`):** the first time a skill's numbers are read on this game version, they are stored in `research/skill-shapes.json` under `numbers` with the live cast range and the shapes the live reader reads at the same moment, so a size and its number come from the same match (`SHAPE NUMBERS` in the log). Champions recorded before get their numbers this way without being recorded again.
- **`tools/preview_sources.py` (new):** decides where each size comes from: one of the skill's numbers by name (`attack_range`, half or twice a width where that fits), the live cast range for lengths that equal it, or a fixed number; two numbers with the same value wait for the author's pick. Only distances in the game's patch rules (and the common size names asked of skills without rules) count, so a speed or duration cannot pass for a size. **`tools/build_preview_table.py` (new):** writes the table the mod is to ship (`probe/src/preview_table.json`) with each size replaced by its source, and reports the sizes to pick and the skills still waiting for numbers. Not written yet: the mod does not read it until stage B.
- **Viewer:** under each drawing, "Sizes in the mod" lists every size with its source and a note (follows edits and patches; follows level and patches; fixed; two numbers fit: pick one; numbers not recorded yet); the source can be changed, and the pick saves at once to `skill-sources.json` in the mod's folder ("automatic" undoes it); sources follow unsaved assignments; "Show: Size source to pick" and "Show: Numbers not recorded yet"; the header counts both; a list of champions still waiting for numbers. Checked on a copy of the author's data with numbers taken from the dev13 and dev14 logs (not the author's files): 87 sizes by name, 37 the cast range, 60 fixed, 4 to pick (Alchemist R, Void Mage Q, Hellion R, Mistguide R); a pick saved and the count fell to 3; "automatic" undid it; choosing a type for Hammerer W gave its radius from `attack_range` before saving; no script or server errors; the one-file page still builds.
- Tests: numbers are kept with the shapes read at the same moment (mod); a size is the one distance number with its value; non-distances never count; two equal numbers wait for a pick, which then wins; a length equal to the cast range follows it; assignments and skills without numbers; the shipped table replaces sizes with sources (tools).
- Verified: formatting, Clippy with warnings denied, 595 passing tests (one existing translation-export helper intentionally ignored), 39 passing tool tests, release build (staging confirmed the DLL carries 1.5.0-dev15), all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.5.0-dev14; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.5.0-dev15. Record: `dist/build-1.5.0-dev15.json`. Not verified: anything in game; the viewer with numbers the mod itself recorded (checked only with numbers taken from the logs). Not committed, pushed or uploaded.

## 1.5.0-dev14 (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** A log-only follow-up to 1.5.0-dev13, approved by the author on 2026-10-21. Nothing the player sees or plays changes.

- **What the 1.5.0-dev13 log showed (the author's match, game 0.6.4):** no crash; 237 skills logged, none refused. 201 skills give a `range` equal to the live cast range, which carries patches (Whip Master W 41 and R 132 against 40 and 130 in the files, Hammerer R 81, Executioner Q 101), and sizes carry them too: Hammerer W's hit detection range 62 (60 × 1.033) and Werewolf Q's 31, the two sizes the save's patches changed. Matches right after start-up showed the original numbers, later ones the save's: each match carries its own. Four skills use `range` for their area, not their cast distance (Knight R range 60, cast range 80; Clown W and Shield Bearer W, cast on self), so the table must say which name each size is. A background match showed Hammerer W at 620: the author typed 620 in the database editor, which keeps distances in thousandths (62 units is 62000). The author also observed that a patch writes its result back into the save's database. The 8 file-based base champions answer with their sizes (Alchemist Q radius 32, Sand Mage R radius 42). Workshop champions' skills report the generic patch name `data_action`, whose rules list no sizes, so dev13 never asked them for one; their numbers do follow patches (Sun Mage Q attack ratio 50 to 46).
- **Mod (`patch_fields::names_for`):** `data_action` skills and skills whose patch name the rules lack are now also asked the common size names (attack range, radius, width, line width, projectile radius, explosion range, area, buff and heal range, cone angle, fan angle, projectile count, move distance and range), without asking any name twice.
- **What the log decides:** whether Workshop skills' sizes come back by name from the data-driven lookup (it searches the skill's effects, first match first).
- Tests: a `data_action` skill is asked its rules' fields first, then the size names, each once.
- Verified: formatting, Clippy with warnings denied, 594 passing tests (one existing translation-export helper intentionally ignored), release build (staging confirmed the DLL carries 1.5.0-dev14), all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.5.0-dev13; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.5.0-dev14. Record: `dist/build-1.5.0-dev14.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.5.0-dev13 (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** A log-only research build for step 5 of the preview plan, approved by the author on 2026-10-21. Nothing the player sees or plays changes.

- **Why:** the author pointed out that the database editor lets players change skill numbers (in the new-game and the current-save database, while in-save balance patches change them as well), so a preview made of fixed numbers would fail completely after a big edit. The author asked for a reader of the current numbers instead of a balance patch reader. **Seen in game (1.5.0-dev12, game 0.6.4, the author):** Hammerer W's preview grew when its hit detection range was edited from 60 to 150 in the current save.
- **Tried first and set aside:** reading the active balance patch from the save file. **Read from the files:** the save, after its logo, is one gzip stream holding each champion's database numbers by name and the multipliers of every stored patch (8 in the author's save; only Hammerer W and Werewolf Q ever changed a size, by 1-2 units). The mod cannot tell which save is loaded, the file lags behind patches made during play, and the format is undocumented, so it serves as evidence only. My earlier claims that patches change "only a few" sizes, and that the live reader "probably" misses patches, are superseded: the rules allow a size change on 87 of 236 skill kinds, and whether the live reader sees patches is unknown. My recommendation to keep the per-version live reader instead of looking for this reader was wrong and is withdrawn (the author: reverse engineering is how this mod is built).
- **Read from the code (0.6.4; the same table shape found in 0.6.3 and 0.7.0, not checked further):** every skill object in a match, at the three slots whose cooldown the mod already reads, starts its method table with the game's patch methods: the skill's patch name (+0x18), get a number by name (+0x20), set one (+0x28) and list its names (+0x30). They are what the database editor and the patch engine use; the editor's fields per skill are exactly that skill's entry in the bundle's `patch_setting` (Hammerer W: attack, attack ratio, hit detection range, slow speed, slow duration, cooldown, and no range, which is why its range could not be edited). 189 compiled skill types have them; data-driven skills (champion files, Workshop) answer from the skill and then search its effects. Of the 108 sizes the live reader recorded for compiled champions, 89 are such editable named numbers, 10 are fixed numbers the editor cannot change, and 9 are projectile widths in code or lengths from the cast range. Notes with addresses: `research/patch-fields-0.6.4.md` (not in git).
- **Mod (`native_adapter/windows/skill_numbers.rs`, `patch_fields.rs`):** when a champion's learned skill is first seen in a match (background matches included, as the shape recorder does), the mod asks the skill object for its patch name and for each name its `patch_setting` entry lists, plus `range`, and writes `PATCH FIELDS champion=… slot=… type=… cast_range=… range_check=same|differs|none name=value … absent=[…]`; a skill is logged again only when its values change. Workshop skills without rules are asked the common size names. The two game methods return two registers each, so they are called through a small assembly shim; the method addresses must lie inside the game's executable and the patch name must be a short lowercase identifier, or the skill is skipped with a `refused=` line. The bundle pass that reads the base champions also reads `patch_setting` (`PATCH FIELDS rules kinds=N` at start-up).
- **What the log decides:** `range_check=same` on skills whose live cast range is patched (Whip Master R reads 132 live against 130 in the files) would show that the skill objects carry patches; with the database edit already seen, sizes could then be read by name instead of by the live reader.
- Tests: the rules list each kind's fields with `range` first; unknown kinds are asked the common size names; the line compares `range` with the live cast range.
- Verified: formatting, Clippy with warnings denied, 593 passing tests (one existing translation-export helper intentionally ignored), release build (staging confirmed the DLL carries 1.5.0-dev13; the built calls checked in its disassembly: object, name and length in rcx/rdx/r8, 32 bytes of shadow space, results from rax/rdx), all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.5.0-dev12; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.5.0-dev13. Record: `dist/build-1.5.0-dev13.json`. Not verified: anything in game, including that the game's methods behave as read (a wrong reading could crash the game when a champion's skill is first seen); 0.6.3 and 0.7.0 only share the table shape. Not committed, pushed or uploaded.

## 1.5.0-dev12 (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** An assignment can add to what the game reads; the viewer shows sizes on the drawings. Approved by the author on 2026-10-21.

- **The author's report (viewer):** Zook W is a unit target with a jump back, but an assignment holds one type, and its cast range looked far too big although it is melee in game. **Read from the table and its champion file:** Zook W is cast on an enemy champion at range 25 (the file says 26; 25 is the patched live value), blasts a 32-unit circle around Zook (read by the game) and then rockets away (`zook_rocket_away`, the mod's own code). Each drawing zooms to fit its skill, and the 32 blast is wider than the 25 range, so the blast looked like the range; the author's "jump back" assignment had replaced the read blast.
- **Mod (`skill_preview::with_assignment`, `Geometry::adds`):** an assignment with `"add": true` draws its shapes on top of what the game reads (live reader, then champion file) instead of replacing it; without it, it still replaces. Zook W: the 32 blast around Zook plus a 26 jump back, under the cast range ring.
- **Viewer:** "Replace what the game reads" or "Add to what the game reads" for an assignment on a skill the game already reads, with the note saying which; the type chip reads "Around self + Unit target + jump back"; drawings carry their values ("range 25", "r 32", "jump 26") and a 25-unit scale bar; the "How to use" panel explains both. Fixed: a hidden banner showed as an empty orange box.
- **Viewer, tool only, the same day (approved by the author after asking how to know what is still missing):** a "Checked by me" tick on every skill, saved at once to `skill-checked.json` in the mod's folder (changing a skill's assignment clears its tick); the header counts "N of M checked by you"; "Show: Not checked yet" and "Checked by me"; a "Not recorded yet" list of the champions the player's game can field (base game and enabled mods) that no match has shown. Read from the viewer: 84 champions recorded (Strongman and Voidling since the last count), 10 not recorded yet (Amazon, Beastman, Candy Gel, Archangel, Construct, Dummy, Fireblade, Kharazim, Oni, Plane Mage). Checked in a separate tab: a tick saves and counts, the list shows names, ids and mods; the test tick was removed and the file deleted again.
- Tests: an adding assignment keeps the read shape and adds its own (Zook W), a replacing one replaces, no assignment leaves the read shape. Checked in a separate browser tab: Zook W with "Add" shows the 25 range, the 32 blast and the 26 jump, and no script errors.
- Verified: formatting, Clippy with warnings denied, 590 passing tests (one existing translation-export helper intentionally ignored), release build (staging confirmed the DLL carries 1.5.0-dev12), all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.5.0-dev11; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.5.0-dev12. Record: `dist/build-1.5.0-dev12.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.5.0-dev11 (development build) — 2026-10-21

**Seen in the viewer by the author 2026-10-21 ("Looks good, great job"). Development build, never for the Workshop.** Viewer follow-ups from the author's review, approved on 2026-10-21.

- **The author's review (1.5.0-dev10 viewer):** Soldier Q is a passive and had no option; Spirit Caller R detonates existing wisps, is not really aimed and has no range; skills already read should be filterable by type too; and the numbers a skill holds should tell its likely shape. The Forge (the champion-making tool) shows the same casting type and casting target lists the mod already reads, and presets (Skillshot, Delayed Meteor Zone, Cleave and so on) that all map onto the preview types.
- **Read from the table:** the game marks a passive with a cast type outside 0-3 (Soldier Q: 4294967295); Spirit Caller R is cast type "none" with range 0. In game the mod already draws nothing for both. Every circle-at-aim skill recorded so far has a cast range.
- **Tenth preview type, "no preview"** (`skill_preview::assigned`, `Geometry::hidden`): nothing is drawn for that skill, not even its range.
- **Viewer:** passives and skills cast without aiming that have no shape are labelled "passive" and "instant, no aim" instead of "unread"; every skill, read or assigned, has a type (from its shapes), and a type filter shows the counts; an unread aimed skill gets a suggested type from its cast type and the game's numbers (a direction cast with `projectile_count` and `fan_angle` is a fan, with a move distance a dash, with `angle` a cone, else a line; a point cast with width and height a rectangle, with a move distance a dash, else a circle at aim; heal or shield numbers tick "for allies"), drawn and tagged "suggested", with a "Use suggestion" button. Nothing is saved until the author uses or picks a type.
- **Read from the viewer (82 champions):** 4 unread aimed skills remain, each with a suggestion (Boomerang Hunter R, Demon Q, Druid R, Wind Mage W); types: unit target 98, line 50, circle at aim 35, around self 23, dash 17, cone 5, passive 5, no area 4, rectangle 2, fan 2, instant 1. The six assignments the author had made in the browser panel were left untouched.
- Tests: "no preview" draws nothing, not even the range. Checked in the browser panel: no script errors.
- **Viewer rework, tool only, the same day (approved by the author, who asked for a complete tool instead of one gap at a time):**
  - Start: double-click `tools/preview_viewer.cmd`; `tools/preview_viewer.py` now runs a small viewer on this PC only (127.0.0.1) and opens the browser; every page load reads the latest table, so nothing needs regenerating. `--static` still writes one self-contained page. The page is `tools/preview_viewer.html`.
  - Save writes `%LOCALAPPDATA%\LTDirectControl\skill-overrides.json` directly (the previous file kept as `.bak`; only valid entries written, the others named), Ctrl+S saves, the header shows unsaved changes or invalid values, closing with unsaved work asks first, and unsaved work from last time is offered back (including the first viewer's browser-only assignments).
  - Every card names its preview type ("Circle at aim") and source (read by the game, from champion file, your assignment, unit target, passive, instant, not readable yet), with the English champion name, internal id, skill name, description and the mod it comes from (English by the author's choice); filters for status, preview type and mod, with counts, and search across names, skills and mods; filters, level and scale are remembered.
  - Drawings: the mouse aims like in a match (point casts stop at the range), a level slider applies range growth, a 25-unit grid, and "same scale" to compare sizes.
  - Editing: "Use what the game reads (type)" as the first choice, a note when an assignment replaces a shape the game reads, units and help on each box, the game number each value came from, empty or out-of-range values marked, "Use suggestion", "Back to what the game reads", a "How to use" panel and a colour legend.
  - Value guesses: an area's size is searched in `radius`, `attack_range`, `explosion_range`, the heal and buff ranges, never the cast range (Wind Mage Q now 30 from `attack_range`; this order finds 26 of the 30 circles the live reader reads).
  - **Learning, after the author's test the same day** (Barrier Magician Q got no width or depth though her W is the same square; Harpooner R's length looked missing; Wind Mage W's "spread r4 l70" read as a value; small text): for every skill the game reads (or its file declares), the viewer finds which of that skill's numbers holds each size and tries those fields first for unread skills, ranked by how often they matched; times, damage and ratios are skipped so they cannot pass for sizes. Learned from 161 read skills, for example: rectangle width and depth from `barrier_range` (Barrier Mage W), so Barrier Magician Q gets 40 × 40; circle radius from `attack_range` (15 skills); line width from `attack_range` × 2 (15), so Wind Mage W as a line gets 16; line length from `range` (42), so an empty length stays "the cast range". Each value says where it came from ("barrier_range, learned from Barrier Mage W"); a value missing from the game's data says "not in the game's data: enter it" (Wind Mage W's line count and spread); empty optional boxes show their meaning ("130 (cast range)", "to the aim", "none"); shapes are described in words ("fan: 5 lines, 18 wide, cast range long, 36° spread"); the reader's technical reason became plain words (the detail stays in a tooltip); "What the viewer learned" lists every learned field. Text 15 px (small text 13 px), wider cards. Cards now fill in at once instead of on the next paint, which a background tab skipped.
  - Checked: the viewer loads with no script errors and shows 82 champions with all filters; in a separate browser tab, choosing a type, an empty value and removing the assignment behave as intended; saving to a scratch folder wrote 3 valid entries, named 3 refused ones and kept a backup. Not checked: a save into the mod's folder (the author's own file was not touched).
- Verified: formatting, Clippy with warnings denied, 589 passing tests (one existing translation-export helper intentionally ignored), release build (staging confirmed the DLL carries 1.5.0-dev11), all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.5.0-dev10; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.5.0-dev11. Record: `dist/build-1.5.0-dev11.json`. Not verified: anything in game; the suggestions against the game. Not committed, pushed or uploaded.

## 1.5.0-dev10 (development build) — 2026-10-21

**Not seen in game. Development build, never for the Workshop.** Step 3 of the agreed skill-preview plan, approved by the author on 2026-10-21: the author assigns previews to unread skills in the viewer, and the mod draws them.

- **The author's review (1.5.0-dev9 viewer, 2026-10-21):** unit-target skills need no shape, only the hover highlight; Archer W also jumps back; Barrier Magician Q is a smaller rectangle like her W, and some skills change from a skill level on; Barrier Magician R a shrinking ring on the target champion; Boomerang Hunter W three boomerangs that fly out and return; Boomerang Hunter R bounces like Voidling Q; Chef W heals around herself. Lines: like League (Hwei QE and EQ share one preview, only the width differs), Harpy R and Harpooner W stay one line type. The dash preview is hard to see in fights: noted for a later redesign (backlog item 2).
- **Nine preview types** (`skill_preview::assigned`): unit target (optional area radius around the target), around self, circle at aim, rectangle at aim, line from caster (width, length; 0 = the cast range), fan of lines (count, width, length, spread degrees: new `Shape::Spread`, evenly spread), cone (radius, angle), dash or blink (distance), unit target with a jump back (distance: new `Shape::Backstep`, drawn straight away from the target). Optional "for allies" on the area types.
- **The mod reads `%LOCALAPPDATA%\LTDirectControl\skill-overrides.json`** at start (`PREVIEW OVERRIDES skills=… refused=…` in the log; entries with an unknown type, a missing value or a number out of range are refused). An assignment wins over the live reader and the champion file for that skill: it is a deliberate correction. Range, cast style and target type stay live. Shipping the file inside the mod is step 4.
- **Viewer editor (`tools/preview_viewer.py`):** each skill has a type list; the value boxes are pre-filled from the game's own numbers for that skill (the bundle's `champion_info` sheet for base champions, champion files for mod champions; shown under "Game numbers"), for example Harpooner R: count 5, width 18 (twice `projectile_radius`), spread 36 (`fan_angle`). The drawing follows the values live; "Download assignments" saves `skill-overrides.json`; the page keeps unsaved work in the browser. Unread skills cast on a unit are shown as "unit target" (the preview already draws them right), which replaces the planned marking of the shared homing-shot effect in the live reader: that would have needed a new game address per version.
- **Read from the viewer (80 champions recorded):** 12 skills still show the wrong preview: Barrier Magician Q, Boomerang Hunter W and R, Demon Q, Druid R, Guardian Spirit R, Harpooner R, Soldier Q, Spirit Caller R, Voodoo Shaman R, Wind Mage Q and W.
- Not done here: the dashed white unknown-skill arrow (the author's option (a) for unread aimed skills) is still the 1.5.0-dev5 arrow in game; the viewer already draws it dashed.
- Tests: every preview type becomes its shape, the area on a unit target, the ally colour, refused entries, the even spread offsets. Checked in the browser panel: the viewer loads with no script errors, and assigning Harpooner R a fan pre-fills 5, 18 and 36 and draws it.
- Verified: formatting, Clippy with warnings denied, 589 passing tests (one existing translation-export helper intentionally ignored), release build (staging confirmed the DLL carries 1.5.0-dev10), all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.5.0-dev9; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.5.0-dev10. Record: `dist/build-1.5.0-dev10.json`. Not verified: anything in game, including an assignment drawn in a match. Not committed, pushed or uploaded.

## 1.5.0-dev9 (development build) — 2026-10-21

**Seen 2026-10-21 by the author: after a match the table grew, the regenerated viewer's drawings looked correct on a random sample, and most missing previews belong to champions not yet in the author's save. Development build, never for the Workshop.** Step 2 of the agreed skill-preview plan, approved by the author on 2026-10-21: a viewer that draws every recorded skill outside the game.

- **New `tools/preview_viewer.py`:** reads `research/skill-shapes.json` and the preview style (built-in plus the user's override) and writes one self-contained page, `preview-viewer.html`, next to the table. Each champion shows Q, W and R drawn on grass: the range in cream (pale blue for ally skills), the shapes in yellow (ally blue), a target dot for skills cast on a unit, a dashed white arrow for unread aimed skills, and a tag (live, from file, no area, unread) with the cast type, range and sizes. A search box and an "only champions with unread skills" filter. It follows the mod's drawing rules closely but not exactly (aim placed up and to the right; ranges at level 1). Served locally for checking via the `preview-viewer` entry in `.claude/launch.json` (Python's built-in web server on 127.0.0.1).
- **The table also records, per skill** (`shape_record::Cast`): the casting type, target type, range at level 1 and growth per level, read live in the combat hook; and under `file`, what the champion file declares, so the viewer shows the same fallback the preview uses. Entries recorded by 1.5.0-dev8 lack these and are recorded again.
- Checked: the viewer page loads with no script errors and draws the 1.5.0-dev8 table (ranges unknown there).
- Verified: formatting, Clippy with warnings denied, 588 passing tests (one existing translation-export helper intentionally ignored), release build (staging confirmed the DLL carries 1.5.0-dev9), all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.5.0-dev8; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.5.0-dev9. Record: `dist/build-1.5.0-dev9.json`. Not verified: anything in game; the viewer's drawings against the game's previews. Not committed, pushed or uploaded.

## 1.5.0-dev8 (development build) — 2026-10-21

**Played by the author 2026-10-21 (two matches). Read from its log (`probe=1.5.0-dev8`) and table: 67 champions recorded (52 base, 15 mod), 201 skills; 16 base champions not yet seen. 65 skills unread by the live reader, 17 of them covered by champion files, 48 still unread; several share one effect type (the homing shot `1705150` in 5, effect `1670c60` in 4). Recording works across threads and background matches. Development build, never for the Workshop.** Step 1 of the agreed skill-preview plan (the author, 2026-10-21; docs/backlog.md item 2): record every champion's skill shapes once, so the published build can later ship them as a table instead of the live reader.

- **Read from the code:** the skill range, cast style and target type come live from the combat hook (`native_adapter/windows/combat.rs`), which stays; only the shapes come from the live reader. So the table holds shapes only, and ranges keep their in-game patched values.
- **New `shape_record.rs`:** the simulation callback stores, per thread, which champion each actor is; the combat hook, which runs for every champion of every simulation including the game's background matches, decodes a champion's Q, W and R with the live reader the first time it sees that champion (`record_shapes`, with the same readability check as the coverage lines) and adds them to `%LOCALAPPDATA%\LTDirectControl\research\skill-shapes.json`: per skill its shapes (kind, sizes in world units, where it sits, allies, projectile), "no area", or why it could not be read, marked `source: live`, with the game version. A `SHAPE RECORD champion=… Q=… W=… R=… recorded=N` line is logged per champion. Earlier sessions' champions are kept when the game version is the same.
- Nothing changes on screen.
- Tests: one recording per actor and match, names from the roster, a new match starts over; skills written as shapes, no area or unread.
- Verified: formatting, Clippy with warnings denied, 588 passing tests (one existing translation-export helper intentionally ignored), release build (staging confirmed the DLL carries 1.5.0-dev8), all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.5.0-dev7; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.5.0-dev8. Record: `dist/build-1.5.0-dev8.json`. Not verified: anything in game, including that the combat hook runs on the same thread as each match's simulation callback (otherwise nothing is recorded). Not committed, pushed or uploaded.

## 1.5.0-dev7 (development build) — 2026-10-20

**Seen in game 2026-10-20 by the author: "Test passed". Development build, never for the Workshop.** A visible, League-like range indicator in a sibling of the skill's colour, approved by the author on 2026-10-20 ("cream").

- **Seen in game (1.5.0-dev6, the author):** Bulldozer R shows its circle. The range while aiming (Shift+W) is barely visible: a 1 px grey line at 48 % over a 3 px dark line at 30 % (`preview_style.json`). League draws a soft fill over the whole range and a bright rim, in the same colour as the skill's shape. The author chose a related but distinct colour, so range and shape read as one skill yet mean different things.
- **Now (`skill_preview::range_ring`, `range_color`):** the range gets a faint fill (5 %, below the shapes' 6.5 %) and a 2.5 px rim at 90 % over a 5 px dark edge at 35 %. Its colour is pale cream (`range_ready` #fff6c2) beside the yellow shape, pale blue (`range_ally` #b4e2f5) for a skill that targets allies (`Preview::allies`, from the skill's casting target), and grey while the skill is not ready, like the shape. Amber was declined: too close to the orange "target out of range" marker. The HUD-hover preview keeps its rim in the skill's own colour on top.
- All new values are style keys (`range_ready`, `range_ally`, `range_fill`, and the existing widths and opacities), so they can be tuned in `%LOCALAPPDATA%\LTDirectControl\preview_style.json` without a build.
- Tests: the range colours by state and side, a fill fainter than the shapes', one fill disc in the range colour; the HUD-hover test now allows the cream rim.
- Verified: formatting, Clippy with warnings denied, 586 passing tests (one existing translation-export helper intentionally ignored), release build (staging confirmed the DLL carries 1.5.0-dev7), all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.5.0-dev6; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.5.0-dev7. Record: `dist/build-1.5.0-dev7.json`. Not verified: anything in game, including how the fill reads on dark and bright ground. Not committed, pushed or uploaded.

## 1.5.0-dev6 (development build) — 2026-10-20

**Seen in game 2026-10-20 by the author: Bulldozer R shows its circle. Development build, never for the Workshop.** A lob's own landing area in champion files, approved by the author on 2026-10-20.

- **Seen in game (1.5.0-dev5, the author, Bulldozer from the Bowsori Champions Workshop item):** W is like League's Singed flip, a skill cast on an enemy champion; R is cast on a unit and hits an area. R's description says "Can only target towers", yet champions and minions can be targeted, and R then really hits, damages and stuns. **Read from the champion file:** R's `casting_target` is `Enemy` (any enemy; the game's list has no "towers only"), so the game allows it and Takeover follows the game; the tower rule is only in the description. W, cast on a unit, shows the range and the target cue by the 1.5.0-dev3 rule.
- **Now:** a `ParabolicProjectile` whose own circle has `applied_effects` and is larger than 5 units is drawn as an area where it lands (Bulldozer R: 120 units, damage and stun). 1.5.0-dev2 treated a lob's own shape as its flight only, from League Annie R's 1-unit circle with no effects, which keeps that reading.
- Tests: Bulldozer R's 120-unit landing circle; League Annie R still only its landing effects.
- Verified: formatting, Clippy with warnings denied, 585 passing tests (one existing translation-export helper intentionally ignored), release build (staging confirmed the DLL carries 1.5.0-dev6), all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.5.0-dev5; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.5.0-dev6. Record: `dist/build-1.5.0-dev6.json`. Not verified: anything in game, including whether the live reader already drew this circle (it comes first). Not committed, pushed or uploaded.

## 1.5.0-dev5 (development build) — 2026-10-20

**Seen in game 2026-10-20 by the author with Bulldozer (W shows range and target, R hits champions and minions); the cast-style cue itself not yet commented on. Development build, never for the Workshop.** Stage 3 of the skill preview redesign, approved by the author on 2026-10-20 (option A of three shown as sketches: today's crosshair; A, the cast style only; B, A plus a "?" glyph).

- **A skill the mod cannot read** (no shape from the live reader or a champion file, and not known to have no area) no longer shows a white crosshair at the aim point, which looked broken and said nothing. It shows only how it is cast (`skill_preview::cast_cue`), in the skill's colour: a direction cast draws a thin arrow from just ahead of the caster to the aimed end of its range; a point cast draws a small ring with a centre dot where it lands. Neither claims a width or an area. The range ring is unchanged; skills cast on a unit keep the range ring and the target highlight.
- Examples (read from the 1.5.0-dev2 coverage log, not seen): bulldozer W and R, dummy Q and R, witness Q, zook R, and base Strongman Q and R, Harpooner R, Nightmare R and Spellbreaker R where the live reader cannot decode them.
- Docs: `investigation-preview-sources.md` section 1 marks the crosshair as superseded; backlog item 2 lists stages 1-3 as built.
- Tests: an arrow for a direction cast, a ring with a dot for a point cast, no white crosshair strokes.
- Verified: formatting, Clippy with warnings denied, 585 passing tests (one existing translation-export helper intentionally ignored), release build (staging confirmed the DLL carries 1.5.0-dev5), all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.5.0-dev4; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.5.0-dev5. Record: `dist/build-1.5.0-dev5.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.5.0-dev4 (development build) — 2026-10-20

**Started once by the author 2026-10-20 (no match). Read from its log (`probe=1.5.0-dev4`): eight `PREVIEW BASE … source=game bundle` lines; 63 declared skill slots, the same as 1.5.0-dev3, so the base previews are unchanged. Development build, never for the Workshop.** Stage 2 of reading the champion files for skill previews, approved by the author on 2026-10-20: the game's own data-defined base champions come from the installed bundle at start.

- **New `game_bundle.rs`:** reads chosen files from `bundle.game_data` beside the executable (entry count, then per entry extension, path and size as little-endian u32-prefixed fields, then the bytes; unwanted entries are skipped with a seek; files over 4 MB are skipped; a malformed bundle ends the listing without a panic). `sprite_art.rs` keeps its own reader for artwork for now.
- **`skill_preview::initialize`:** the base champions' declarations (`asset/base/setting/<id>`, `data_champion`) are read from the bundle, so they follow game updates without a rebuild. The copy baked in at build time (`preview_assets.json`, from `tools/read_preview_assets.py`) is used only when the bundle cannot be read. A `PREVIEW BASE champion=… source=… Q=… W=… R=…` line is logged for each.
- **Read from the data:** on game 0.6.4 the bundle's 8 data champions match the baked copy exactly, so nothing changes on the author's game; game 0.7.0's bundle has 12, and Bombardier, Ferryman, Gladiator and Hypnotist were missing from the baked copy.
- The live reader still comes first; these declarations fill in where it cannot decode a skill.
- Tests: wanted entries read and the rest skipped, a truncated bundle, a missing bundle.
- Verified: formatting, Clippy with warnings denied, 584 passing tests (one existing translation-export helper intentionally ignored), release build (staging confirmed the DLL carries 1.5.0-dev4), all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.5.0-dev3; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.5.0-dev4. Record: `dist/build-1.5.0-dev4.json`. Not verified: anything in game, including that the bundle is read at start. Not committed, pushed or uploaded.

## 1.5.0-dev3 (development build) — 2026-10-20

**Seen in game 2026-10-20 by the author: "Test passed" (Candygel R pools only, Candygel W no path, Voidling Q bounce circle). No base or installed champion has a pure dash; the nearest, Nullifier Q (a dash that empowers the next attack), keeps its path by the rule. Development build, never for the Workshop.** Dashes in skill previews follow the skill's main purpose, approved by the author on 2026-10-20 ("pools only for Candygel R").

- **The author's rule (2026-10-20, from League):** Zac R shows no jump path because its purpose is the crowd control and damage; Corki W shows its dash because moving is the point. Candygel R's slide hits nothing and does not stop at units: its purpose is the pools. Candygel W is cast on an ally: range and the target cue only.
- **Now (`skill_preview::main_purpose`, applied to every preview, from the live reader and from champion files alike):** a skill cast on a unit draws no dash or blink path; a dash or blink that leaves areas (circles, rectangles, cones) draws only those areas; a dash that hits along its way keeps its corridor (it was never drawn as a path); a dash with nothing else keeps its path; a projectile beside a dash stays with it. Before, every dash and blink was drawn. Examples by the rule (read from the code, not seen): Candygel R shows its two pools, Candygel W and Nullifier W no path, Nightmare W only its landing circle.
- **Voidling Q, wording only:** a target-splash skill bounces on to further units within `range` of the unit it just hit, like League's Jhin Q (the author, 2026-10-20); the preview's circle is that bounce range around the first target. This replaces 1.5.0-dev2's guess "range is the splash radius"; the drawing is unchanged.
- Tests: pure dash kept, pools only for Candygel R, no path when cast on a unit, a shot beside a dash kept.
- Verified: formatting, Clippy with warnings denied, 583 passing tests (one existing translation-export helper intentionally ignored), release build (staging confirmed the DLL carries 1.5.0-dev3), all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.5.0-dev2; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.5.0-dev3. Record: `dist/build-1.5.0-dev3.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.5.0-dev2 (development build) — 2026-10-20

**Started once by the author 2026-10-20 (no match). Read from its log (`probe=1.5.0-dev2`): 78 skills of the 26 enabled mod champions: 46 with a shape, 26 with no area, 6 unknown (before: about 26 % used types the decoder did not know). The 6: bulldozer W and R and dummy Q and R are custom code only; witness Q is a linear projectile with a `Rect` shape (only `Circle` is read for those); zook R combines a map-wide ally shield with custom mines. Development build, never for the Workshop.** Stage 1 of reading the champion files for skill previews, approved by the author on 2026-10-20 (option A after the measurement; the measurement is removed).

- **Seen in game (1.5.0-dev1, the author):** played and let background match days run; no slowdown felt. **Read from its data:** projectile speed was consistent for 41 skills and range for 13; widths and areas from hits failed (hits spread over half the map); 25 skills had no projectile. Recorded in `docs/investigation-preview-sources.md` section 6. `skill_measure.rs` is removed with its perf label.
- **Read from the code:** the mod already reads the champion files of every enabled mod (local and Workshop, in the enabled order, `hud_icons::enabled_assets`) and the preview uses them when the live reader cannot decode a skill; the live reader stays first because it sees in-game balance patches. **Read from the data** (26 data champions in the author's 11 enabled mods, 78 skills): 23 % decoded fully, 28 % decoded with a skipped part, 26 % used shape types the decoder did not know, 18 % have no area, 5 % only custom code.
- **The file decoder now reads**, each tested with a trimmed example from the author's installed Workshop files:
  - `LineRangeProjectile` (abbie W): a line from the caster, `width` wide and `length` long. Guess: `width` is the full width, as Astrologer W's `line_width`. **Superseded 2026-10-22 (1.5.2-dev1):** read from the game's code, `width` is the hit's distance on each side of the line, so the line is twice `width` wide.
  - `MoveToTarget` (invincible Q, abbie R), `Rush`, `RushTime` (candygel R), `RushMoveToBack` (ruby Q): a dash to the aim, like `MoveTo`.
  - Movers' and `ParabolicProjectile`'s `end_effects` (league_annie R): circles they leave are drawn where the caster or the lob lands; `MoveTo` and `Teleport` now do this too (before, their landing areas were ignored).
  - `TargetSplashProjectile` (voidling Q, cf_void_mage W): a circle at the target. Guess: `range` is the splash radius.
  - `SwitchByBuff` (heavyblade Q): the usual variant (`effect_none`), as the live reader already does; the buffed one is not shown.
  - `RandomTarget` (ruby Q, kharazim R): an effect on a random nearby unit, counted as no area.
  - `Pull`, `Knockback` and `Grab` were already counted as no area.
- **`PREVIEW FILE champion=… Q=… W=… R=…`** is logged at start for every enabled mod champion, so coverage can be checked from one start of the game.
- **Backlog item 2:** the stages, and replacing the live reader later with a generated table (about 37 version-specific entries per game version saved; patched numbers lost until the active balance patch can be read).
- Tests: the eight new cases above.
- Verified: formatting, Clippy with warnings denied, 582 passing tests (one existing translation-export helper intentionally ignored), release build (staging confirmed the DLL carries 1.5.0-dev2), all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.5.0-dev1; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.5.0-dev2. Record: `dist/build-1.5.0-dev2.json`. Not verified: anything in game, including the guessed meanings of `width` and the splash `range`. Not committed, pushed or uploaded.

## 1.5.0-dev1 (development build) — 2026-10-20

**Seen in game 2026-10-20 by the author: no slowdown felt; results in `docs/investigation-preview-sources.md` section 6. Development build, never for the Workshop.** Skill measurement prototype, log only, approved by the author on 2026-10-20 (watching both the match on screen and the background pre-simulations). First step of the skill preview redesign (`docs/investigation-preview-sources.md`, layer 3a "watch and learn"); nothing on screen changes.

- **Question:** how much of a skill's shape can be measured from play alone through the official interface, with no hook and no address, before the new preview is designed.
- **New `skill_measure.rs`.** In the match on screen and in the game's background pre-simulations: a cooldown that jumps up marks a cast (player, champion, slot). For 2 s it records the caster's projectiles (the interface gives position and caster only, no id and no width) and every enemy whose health drops or who gains a crowd-control effect, first time only, at its position. Per champion and slot it keeps projectile range and speed, how far beside the projectile's path hits landed (a width estimate), and how far from the caster and from each other hits landed (an area estimate), as median, 90th percentile and maximum.
- **Output:** `%LOCALAPPDATA%\LTDirectControl\research\skill-measure.json`, rewritten at most every 30 s while casts finish, and a `SKILL MEASURE` log line when a champion's skill reaches 5, 20, 50 and 100 casts. The comparison with what today's preview believes is done when the file is analysed, not inside the mod (the preview's numbers exist only for the controlled champion).
- **Limits (guesses until it runs):** an auto-attack or another skill in the same 2 s counts as a hit too; a ranged champion's auto-attack projectiles count as its projectiles; AI aim points are unknown, so areas are judged from where hits landed.
- **Cost bounds:** at most 16 casts watched per match and 8 matches at once; enemies checked every second tick; each match has its own lock, so parallel background simulations never wait for each other; time is reported as `PERF WORK skill_measure_all_threads` when performance capture is on.
- Tests: projectile range, speed and distance beside the path; area-only hits; the summary's median, 90th percentile and maximum.
- Verified: formatting, Clippy with warnings denied, 584 passing tests (one existing translation-export helper intentionally ignored), release build (staging confirmed the DLL carries 1.5.0-dev1), all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.4.9; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.5.0-dev1. Record: `dist/build-1.5.0-dev1.json`. Not verified: anything in game, including the measurement's cost in background simulations. Not committed, pushed or uploaded.

## 1.4.9 (development build) — 2026-10-20

**Seen in game 2026-10-20 by the author: "Test passed". Development build, never for the Workshop.** Death darkens only the skill slots, like League, approved by the author on 2026-10-20 (crowd control left alone).

- **Seen in game (1.4.8, the author):** "Test passed". Then: while dead, the stats panels stood in front of the death effect while the skill slots looked behind it. **Read from the code:** every mod panel is drawn above the death effect; the skill strip also had its own death dim (motion pass B6: a 50 % black veil over the whole strip plus one per skill slot). **Seen in League (the author's screenshots, alive and dead):** death leaves the frame, stats panel and items as they are and darkens (and greys) only the skill icons; their cooldown numbers stay visible. The author's rule: a skill darkens while it cannot be used.
- **Now:** the whole-strip veil is gone; only the three skill-slot veils remain, moved under each slot's cooldown number, key and lock (z 1004; they covered the numbers at z 1050), so cooldowns read clearly while dead. The respawn lift, slot by slot, is unchanged. Not done: greying the icons (no grey filter is known in the game's UI toolkit), and darkening under crowd control (left for later at the author's request).
- Tests: only the three skill veils exist, under the cooldown numbers, and only they are written.
- Verified: formatting, Clippy with warnings denied, 581 passing tests (one existing translation-export helper intentionally ignored), release build (staging confirmed the DLL carries 1.4.9), all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.4.8; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.4.9. Record: `dist/build-1.4.9.json`. Not verified: anything in game, including that the UI draws layers in the same order across panels. Not committed, pushed or uploaded.

## 1.4.8 (development build) — 2026-10-20

**Seen in game 2026-10-20 by the author: "Test passed" (death and nexus fixes, two-stat values). Development build, never for the Workshop.** Includes 1.4.7, which was never installed on its own. The HUD keeps its last values when samples stop, approved by the author on 2026-10-20 (option a).

- **Reported (the author):** while the nexus explodes at the end of a match, the HUD's information was cleared and everything showed "-". **Guess, supported by the log:** once the nexus falls, the game stops calling the players' think while the explosion plays (the log shows the match worker ending soon after); the HUD, the stats panels and the team list treated samples older than 0.25-1 s as stale and blanked them.
- **Now:** when samples stop while the battlefield shows, the last values stay frozen, as in a pause: `player_hud::snapshot`, `stats_panel::snapshot` and `team_status::snapshot` no longer drop or blank old samples. Timers are never run on wall time. One exception kept: in AI control the state plate still uses only fresh samples (`player_hud::fresh_snapshot`), so a stale "dead" state cannot stick there. Tradeoff: if samples ever stopped for another reason, the HUD would show slightly old numbers instead of dashes.
- Removed, now unused: the team list's `fresh` flag and its "—" captions, the stats panel's sample time, and `NativeTiming::client_sampling` with its 0.5 s resume grace (it only fed the staleness checks) and its test.
- Tests: the HUD and the team list keep their last values after samples stop; the AI-control plate drops them.
- Verified: formatting, Clippy with warnings denied, 580 passing tests (one existing translation-export helper intentionally ignored), release build (staging confirmed the DLL carries 1.4.8), all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.4.6; installed DLL, metadata and all 101 assets match, and the installed DLL carries 1.4.8. Record: `dist/build-1.4.8.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.4.7 (development build) — 2026-10-20

**Not seen in game; never installed on its own (superseded by 1.4.8 before the game closed). Development build, never for the Workshop.** Five fixes from the author's report of 2026-10-20, approved the same day ("one number, read live").

- **Two-stat skill values (`tooltips::computed`).** Reported with the Workshop champion "tale" (Leef's champion mod), R in Traditional Chinese: "90 + [AD] 100% AD + [AP] 100% ability power" showed "[AD] 370 + [AP] 100%魔法攻擊". **Read from the mod's text:** after the AD part was folded, the AP part's plus sign followed a computed number rather than a plain base, so it was left as written. Now every further "+ [stat] N%" term joins the same total: one number behind all the icons ("[AD][AP] 520"), in the first term's colour, like League. A later term that cannot be read ends the total there.
- **While dead, the stats panel and the computed skill values stayed empty.** The mod read your champion's stats only while it lived. It now reads them while dead too (items bought then show), and keeps the last values if the game gives no champion object.
- **While dead, every cooldown showed "--".** Death counted as "skill unavailable". Cooldowns now keep showing; skills still cannot be cast, and ready cues and sounds wait for the respawn as before.
- **While dead, the selected unit's panel vanished.** The unit list for left-click selection is collected in your champion's own think, which the game does not run while it is dead, so a click found nothing and cleared the selection. A separate list for selection only is now collected from the HUD sample while dead; hover highlights and orders stay off while dead. The selected unit's panel now also takes your team from the player, not from the champion object, so it cannot show units your vision does not.
- Tests: the "tale" text (520 with 280 attack damage and 150 ability power), a pure-ratio chain stopped by a health term; cooldowns not "--" while dead. Not covered by a test: the dead-time selection list and reading stats while dead (both need the game).

## 1.4.6 (development build) — 2026-10-20

**Seen in game 2026-10-20 by the author (rebuilt DLL, log `probe=1.4.6`, game 0.6.4): "Unit-target test passed" (1.4.5's nearby targeting; the log shows 35 target=cursor and 2 target=nearby casts, 3 presses with no target). The crowd-control hold is not yet played (Nullifier was banned). Development build, never for the Workshop.** A brief hold for skills that need a crowd-controlled target, approved by the author on 2026-10-20 (option A, 0.5 s).

- **Rebuilt 2026-10-20 (approved by the author):** the first 1.4.5 and 1.4.6 packages carried the **1.4.4 DLL**: `cargo build --release` was skipped, and formatting, Clippy, tests, the verifier and the install all passed regardless (they do not write or compare the DLL's own version). **Seen in game (the author, Witness):** "the forgiving unit-target doesn't seem to work"; **read from the log:** `INIT ... probe=1.4.4`, no `target=` fields, the old `bound-target-disappeared` reason. So 1.4.5's nearby targeting and 1.4.6's hold have not been played yet. Rebuilt from the same code, no code change.
- **`tools/stage_package.py` now refuses a stale DLL:** one older than any `probe/src` file, `Cargo.toml` or `Cargo.lock`, or one that does not contain `mod.mod_info`'s version. Checked: it refused the stale DLL, then staged the rebuilt one (which contains 1.4.6).
- Rebuilt and installed: release build, verifier (all 581 anchors, 0.6.3 / 0.6.4 / 0.7.0) and package passed again. The installer's check of the files it replaces needed a record of the stale install (`dist/build-1.4.6-stale.json`: the 1.4.4 DLL plus 1.4.6's `mod.mod_info`); it matched, and the installed DLL now contains 1.4.6.

- **Reported (the author):** with Nullifier, the combo Q, empowered attack (stun), W onto the stunned target, R (stun again), then W again: the second W "never comes off". **Read from the game's text:** W teleports to a crowd-controlled enemy within 120 (cooldown 2 s); R's dagger stuns for 0.3 s and repeats every 2 s for 4 s. **Read from the log (1.4.4; the match was played before 1.4.5 was installed):** the second W presses started about 0.6 s after R was cast, after W's cooldown, and were all turned away by the mod before the game was asked, because no controlled enemy was under the cursor at the press; one found a stunned enemy whose stun ended before the cast went out. **Guess:** the presses fell just before the dagger landed or just after the 0.3 s stun.
- **Now:** a unit-target press for a skill that needs a crowd-controlled target, which finds none near the cursor, waits up to 0.5 s (`CONTROLLED_HOLD`) and goes off on the first controlled enemy (or ally, for ally skills) under or near the live cursor. Nothing is sent while it waits, so walking and attacking carry on; after 0.5 s it is denied as before. Other skills are unchanged.
- **Log:** `CAST TIMING` shows `target=uncontrolled` when the enemy at the cursor would qualify but is not under crowd control, and `target=held` when the hold found one; `ABILITY HOLD` lines mark the wait. A press that found no target is now `reason=no-target`; `bound-target-disappeared` (used for it since 1.4.2) now means only a bound target that left the match.
- Tests: the hold waits, fires when the target becomes controlled, is denied after 0.5 s, and does not apply to other skills.
- Verified: formatting, Clippy with warnings denied, 581 passing tests (one existing translation-export helper intentionally ignored), release build, all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.4.5; installed DLL, metadata and all 101 assets match. Record: `dist/build-1.4.6.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.4.5 (development build) — 2026-10-20

**Not seen in game: its package carried the stale 1.4.4 DLL (see 1.4.6). Development build, never for the Workshop.** Forgiving unit-target skills, approved by the author on 2026-10-20 ("our goal is to make the gameplay feel close to League").

- **Seen in game (1.4.4, the author):** "Test passed" (computed tooltip values).
- **Why:** the author asked whether League makes unit-target skills easier to aim. **Guess (from knowledge of League, not checked against Riot's documentation):** League's quick cast takes the nearest valid target near the cursor when the cursor is on none. Until now the mod bound a unit-target skill only to a target under the cursor's selection area; a press that missed by a few pixels was refused. **Read from the log (1.4.2-1.4.4 play):** 271 casts, 214 presses denied on cooldown, 85 refused by the game; the log could not tell which refusals were target misses.
- **Now:** when nothing the skill may target is under the cursor, a unit-target skill takes the allowed unit whose selection area is closest to the cursor, within 50 screen pixels (`combat::SKILL_NEARBY_RADIUS`; the author's choice, to be tuned in play). A target under the cursor always wins; the skill's own target rules (ally, enemy, champion, not towers, ...) and the champion-only key still apply; the target is still fixed at the press. The aiming highlight and cursor use the same lookup, so the nearby target shows before the press. Skill-shots and self-casts are unchanged.
- **`CAST TIMING` names the target's source** for unit-target presses: `target=cursor`, `target=nearby` or `target=none`.
- Tests: the closest allowed unit within the radius, not beyond it, skill rules and the champion-only key respected, ties; the new log field.
- Verified: formatting, Clippy with warnings denied, 580 passing tests (one existing translation-export helper intentionally ignored), release build, all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.4.4; installed DLL, metadata and all 101 assets match. Record: `dist/build-1.4.5.json`. Not verified: anything in game, including whether 50 pixels feels right. Not committed, pushed or uploaded.

## 1.4.4 (development build) — 2026-10-20

**Seen in game 2026-10-20 by the author: "Test passed". Development build, never for the Workshop.** Fix for 1.4.3's computed values (the author's screenshot, 2026-10-20).

- **Seen in game (1.4.3, the author, Crossbowman Q in Traditional Chinese, 85 attack damage):** "Doesn't work": the tooltip still read "攻擊力的200% 物理傷害" and "攻擊力的60%". **Read from the game's text:** these are pure ratios with no base number; 1.4.3 converted only "base + percentage" (175 of the 205 English formulas) and left the 30 pure ratios as written.
- **Now pure ratios are converted too** (the percentage of attack damage or ability power): "攻擊力的200%" becomes 170 with 85 attack damage. The stat icon now stands before the number in both cases ("[AD] 154"), as the game places it. A plus sign whose base cannot be read still leaves the text as written. In most languages the converted run is the stat phrase alone; in Russian it also names the damage type, which then disappears from the computed view (Ctrl shows it). This supersedes 1.4.3's "values without a base stay as written", 2026-10-20.
- Tests: Crossbowman's Traditional Chinese wording (170 and 51), the new icon order, an unresolved base kept.
- Verified: formatting, Clippy with warnings denied, 579 passing tests (one existing translation-export helper intentionally ignored), release build, all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.4.3 after the game closed; installed DLL, metadata and all 101 assets match. Record: `dist/build-1.4.4.json`. Not verified: anything in game. Built from the author's "Doesn't work" report without a separate go-ahead (noted to the author; AGENTS.local.md now says a bug report is never a go-ahead). Not committed, pushed or uploaded.

## 1.4.3 (development build) — 2026-10-20

**Seen in game 2026-10-20 by the author: pure ratios not converted; see 1.4.4. Development build, never for the Workshop.** Step 3 of the controls plan, approved by the author on 2026-10-20: skill tooltips show values computed from the champion's current stats, like League; Ctrl shows the game's own wording.

- **Seen in game (1.4.2, the author):** "test all passed". **Read from its log:** 208 casts waited 12 ms on average (5 over 100 ms); right-clicks cancelled 2 presses; 67 presses were refused on cooldown, but only 1 was followed by the same skill within 0.3 s and 47 not within 3 s, so those were early presses, not near misses. Buffering is left as it is.
- **Computed values (`tooltips::computed`, applied when the HUD shows a skill tooltip):** the game's own description text writes each scaling value as a base number, a plus sign, a stat icon and a colour run with the percentage (read from `asset/base/text/champion`: `ad_0` 110 uses, `ap_0` 95). Where the icon is attack damage or ability power, "60 + [AD] 80% Attack Damage" becomes "154 [AD]" from the champion's current attack damage or ability power (the stats panel's values), in the same colour. Holding **Ctrl** while hovering shows the original wording. Only that exact shape changes: other stat icons (health, armour, speeds; whose stat they mean is not always clear), values without a base, unresolved values and runs with more than one percentage stay as written. It works in every language: the percentage may be "80%", "80 %" or "%80". Without the game's own formatted text (the older fallback path, which drops stat icons), nothing is converted.
- New: `ui_state::DETAIL_KEY` (Ctrl, polled every frame).
- Tests: English and Korean text, ability power, a base outside a colour run, French and Turkish percentages, two formulas in one text, and the cases left as written.
- Verified: formatting, Clippy with warnings denied, 579 passing tests (one existing translation-export helper intentionally ignored), release build, all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.4.2; installed DLL, metadata and all 101 assets match. Record: `dist/build-1.4.3.json`. Not verified: anything in game, including how the game's own text actually reads after formatting. Not committed, pushed or uploaded.

## 1.4.2 (development build) — 2026-10-19

**Seen in game 2026-10-20 by the author (game 0.6.4): "test all passed". Development build, never for the Workshop.** Fixes from the author's 1.4.1 test and the first `CAST TIMING` log, approved on 2026-10-19 (option b for held drags).

- **Seen in game (1.4.1, the author):** holding right-click moves correctly, but dragging onto an enemy does not attack it, and the held move shows no click marker on the ground as League does. A Ninja W "didn't come off" around 4:07 of the second match.
- **Read from the log:** no press was recorded near 4:07 (W had cast at about 3:59.6; its cooldown is 300 ticks), so the press was most likely refused by the mod's own check before the game was asked (for example no enemy under the cursor for Ninja's unit-targeted W); that refusal had no `CAST TIMING` line. Separately, in the two matches about 10 of some 60 skill presses made while the champion was busy were **cancelled by a right-click 6-50 ms after the key**.
- **Right-click no longer cancels a pressed skill.** A skill that waits for the champion's current action, or for the game's answer, now fires when the champion is free, and the move carries on. A right-click still cancels aiming a normal cast and walking into range for a skill; S, Escape and the release still cancel a pressed skill. This supersedes the "later right-click cancels the pending skill" rule, 2026-10-19.
- **Holding right-click:** a held move now attacks an enemy under the cursor, as a click on it would, and walks again when the cursor leaves it (the author's option b); held attack-move still picks its own targets. The click marker follows the cursor with every repeat. A press directly on an enemy still attacks it and holding keeps that attack. Fixed: a repeat with no walkable route no longer clears the current route.
- **`CAST TIMING` also logs refusals before the game is asked** (`outcome=denied reason=...`: on cooldown, no eligible target under the cursor, and the other checks).
- Tests: a held move attacks an enemy under the cursor and walks again after; the marker follows repeats; a later right-click keeps a pressed skill and still cancels aiming; stop, Escape and release still cancel.
- Verified: formatting, Clippy with warnings denied, 578 passing tests (one existing translation-export helper intentionally ignored), release build, all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.4.1; installed DLL, metadata and all 101 assets match. Record: `dist/build-1.4.2.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.4.1 (development build) — 2026-10-19

**Seen in game 2026-10-19 by the author (game 0.6.4): holding right-click moves correctly; drag onto an enemy did not attack, no click marker while held; see 1.4.2. Development build, never for the Workshop.** Steps 1 and 2 of the controls plan agreed with the author on 2026-10-19: holding right-click, and a log line to find where skill presses wait.

- **Holding right-click** (the author: League behaviour): after a right-click on the ground (a move) or Shift + right-click (attack-move), holding the button re-issues that order to the cursor every 100 ms (`HOLD_REPEAT`), routed like a click, without new click markers, log lines or input-delay samples. A right-click on a unit attacks it as before, and holding never repeats or cancels that attack; repeats never turn into an attack on a unit the cursor passes over. Releasing the button, stop, Escape, recall, losing focus, the emote wheel or leaving control ends the hold. Before, a held button did nothing after the press (the old test said so).
- **`CAST TIMING` log line** (normal log level, one per skill press): the slot, the outcome (`cast`, `refused`, `cancelled`, `dropped`, with the reason), the champion's native action at the press (2 walking, above 2 attacking or casting, `?` without a fresh sample), how long the press waited, and whether it was held while the champion was busy. The author: buffering "feels not as easy" as League. **Read from the 1.3.26 log:** the 43 refused presses were nearly all one Workshop champion's unit-targeted W, refused by the game itself, and successful casts are logged only at Verbose, so nothing showed where presses wait. **Read from the code (a guess about the feel):** a press while the champion attacks or casts waits for that action to finish, then fires within 1 second; League lets an ability cut an attack's wind-up. The buffer itself is unchanged.
- Tests: holding repeats a move after 100 ms and follows the cursor quietly, does not attack a unit passed over, keeps a clicked attack, and ends on release and stop; the timing line's format and log level.
- Verified: formatting, Clippy with warnings denied, 577 passing tests (one existing translation-export helper intentionally ignored), release build, all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.4.0 after the game closed; installed DLL, metadata and all 101 assets match. Record: `dist/build-1.4.1.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.4.0 — 2026-10-19

Release of the 1.3.1 to 1.3.26 development builds below; the last public build was 1.3.0. One DLL for game 0.6.4 (the live game), 0.6.3 and the 0.7.0 beta, as since 1.2.0.

Seen in game by the author, mostly on 0.6.4 (1.3.1 on 0.7.0): the HUD cues and sounds, the second HUD motion pass, charge marks and the cooldown line fix, the load-hitch fixes, the cheaper HUD update, larger selection areas and vision-choice play, the compatibility with League of Legends Heroes, the death veil and the final ground rings (1.3.26). Not played for this release: 0.6.3 (the verifier passes).

- **Unit highlight** (Settings › Interface › Battlefield feedback; Off / Outline / Ground ring, replacing the "Sprite outlines" switch): **Ground ring** draws the hover and attack-target marks on the ground under the unit, in the match world (the SDK's "Game" map at z -1, above the ground and below the units), with no hook on the game's unit drawing. Hover gets a team-coloured ring with a soft glow; the attack target adds a thin outer ring with four slowly turning chevrons, which fly in on an attack-click. Champion rings lie on the game's own shadow; minions, monsters, towers and the nexus each have their own placement. Rings stay, frozen, while paused. Switching to or from Ground ring takes effect after a game restart.
- **Compatibility:** when another mod already hooks the game's unit renderer (League of Legends Heroes on 0.6.4), only the outline steps aside and every other part of the mod installs; before, the whole mod refused and matches played as AI simulations. Outline then falls back to ground rings by itself. Any other changed game spot still refuses everything, and the log now names the address.
- **Death screen:** no greyscale filter any more (its composite hook is removed, one fewer game spot per version); a cool dark veil with darker edges fades in instead.
- **HUD cues and sounds:** cooldown numbers larger, with tenths under three seconds; a ready cue and gold flash when a skill comes back, and a chime after cooldowns of 4 seconds or more; a red flash in the skill slot and a short sound when a cast is refused; charge marks for multi-use skills; the game's own hover and click sounds in the mod's windows; one "Mod sound volume" for all of them. A "HUD motion" switch (Settings › Interface › Appearance) adds small animations: state plate under the clock, gold roll, purchase breathe and others.
- **Selection:** every unit's clickable area reaches 8 px past its body (a body hit still wins over a neighbour's padding). Whatever the vision choice shows can be hovered, selected, attacked and aimed at, and the vision choice stays when switching to a teammate.
- **Performance:** the title-screen and management-screen hitches are gone (the executable fingerprint is hashed in the background; the team roster scan is spread over frames); the HUD, state plate and session strip update cost about 0.3 ms less per frame.
- Fixes: the cooldown line no longer steps back at the frame a skill recovers; Tab's team window opens above the shop.
- Tested for this release: formatting, lint with warnings denied, 575 tests, the verifier for 0.6.3, 0.6.4 and 0.7.0 (581 code anchors), and the author's play listed above.

## 1.3.26 (development build) — 2026-10-19

**Seen in game 2026-10-19 by the author (game 0.6.4): "All look good now." Development build, never for the Workshop.** Champion rings match the game's own shadow; tower rings move to the base (option A), approved by the author on 2026-10-19.

- **Seen in game (1.3.25, the author, three screenshots):** minion rings good; champion rings at the hips (position offset 0); tower rings still on the painted shadow under the base. The author: for champions "the oval shape shadow would be ideal, both position and size".
- **Read from the screenshots (pixel estimates, a guess):** the game draws one oval shadow under every champion: a small and a large champion agree once converted to world units, centre about 9.5 below the position, about 27 wide and 6 tall. It is not in the champion art (no semi-transparent pixels in any measured frame); the game draws it separately, probably from its 32 x 32 `sprite/circle`. `style/champion_view` gives each champion face (about y -34) and centre (about y -12) points, not the shadow.
- **The change (`ground_ring.rs`):** champions use `CHAMPION_SHADOW` = half width 13.5, half height 3, 9.5 below the position, for every champion whatever its body; towers and the nexus sit 6 below the position (12 before) at 45 % of the body width (65 % before), about their base. Minions and monsters unchanged. This supersedes 1.3.25's champion and tower values, 2026-10-19.
- Tests: champion radii and offset are the shadow's; tower half-width 10.8 for a 24-unit test body, offset 6.
- Verified: formatting, Clippy with warnings denied, 575 passing tests (one existing translation-export helper intentionally ignored), release build, all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.3.25 after the game closed; installed DLL, metadata and all 101 assets match. Record: `dist/build-1.3.26.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.3.25 (development build) — 2026-10-19

**Seen in game 2026-10-19 by the author (game 0.6.4): minions good; champions at the hips, towers on the shadow; see 1.3.26. Development build, never for the Workshop.** Ground ring positions per kind of unit, approved by the author on 2026-10-19 (the front/back ring split is held back: "perhaps splitting the ring won't be needed if we have a good position").

- **Seen in game (1.3.24, the author, four screenshots):** tower ring too low (the tower art includes a painted shadow below it); champion rings hidden under the health bar and name tag; minion rings too low and too wide; jungle monster rings "quite nice".
- **Measured (read from the game's art, a throwaway script mirroring `sprite_art::frame_body`):** every base champion's feet are 11.5 units below its frame centre (10.5-16.5 for a few large forms), with about 14 empty pixels below; minion kinds 6.5-8.5 (Morgard kinds the lowest); the tower base sheet 29.5 including its shadow; monsters at their frame's bottom (stump 18.5, rhino 20.5, mushroom 18.5). **Compared with the screenshots (pixel estimates, a guess):** a champion's position is at its feet; a minion's feet are about 5 below its position; a tower's base ends about 12-13 below.
- **The change (`ground_ring::feet`):** champions 0, minions 5 (the selection boxes' minion value), towers and the nexus 12, jungle monsters the lowest point of their measured art as in 1.3.24. This supersedes 1.3.24's "lowest art point for every unit", 2026-10-19. Minion rings use 55 % of the body width instead of 80 %.
- Tests: each kind's feet offset; minion half-width 13.2 for a 24-unit test body.
- Verified: formatting, Clippy with warnings denied, 575 passing tests (one existing translation-export helper intentionally ignored), release build, all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.3.24 after the game closed; installed DLL, metadata and all 101 assets match. Record: `dist/build-1.3.25.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.3.24 (development build) — 2026-10-19

**Seen in game 2026-10-19 by the author (game 0.6.4): towers, champions and minions too low, minions too wide, monsters good; see 1.3.25. Development build, never for the Workshop.** (Built from the author's 1.3.23 test report without asking for a separate go-ahead, which the working rules require; noted to the author.)

**Not seen in game. Development build, never for the Workshop.** Fixes to the 1.3.23 Ground ring from the author's test, approved on 2026-10-19.

- **Seen in game (1.3.23, the author, game 0.6.4, a screenshot of a blue minion):** the sprite covered both the back and the front of the ring; the ring sat around the unit's middle, not at its feet. Champion rings were "a bit too wide". A pause made all rings disappear. "The rest is good."
- **Feet:** the game's unit position is the middle of the sprite, so each ring now sits `ground_ring::feet` world units lower: the bottom of the unit's measured body art (bodies are stored around the frame centre, one sprite pixel per world unit; the lowest across the set's sheets), or half the body height without loaded art. **Guess:** the frame centre is the unit position for minions and towers as it is for champions; the test shows.
- **Width:** champion rings use 50 % of the measured body width instead of 60 %, about the body's own width. Minions (80 %) and towers (65 %) unchanged.
- **Pause:** the game stops hover and target updates while paused, and the rings were drawn only while the match ran. Now a pause keeps the rings of the last running frame, frozen (`GroundRings::paused`); leaving the match clears them. The drawing moved into `Client::draw_ground_rings`.
- Tests: rings sit at the feet (half the body height, else the radius, without art); a pause keeps the last rings and leaving clears them; champion half-width 12 units for a 24-unit body.
- Verified: formatting, Clippy with warnings denied, 575 passing tests (one existing translation-export helper intentionally ignored), release build, all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.3.23 after the game closed; installed DLL, metadata and all 101 assets match. Record: `dist/build-1.3.24.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.3.23 (development build) — 2026-10-19

**Seen in game 2026-10-19 by the author (game 0.6.4): rings at the unit's middle, champion rings too wide, gone while paused; the rest good. See 1.3.24. Development build, never for the Workshop.** The redesigned Ground ring, drawn into the match world under the units, approved by the author on 2026-10-19 from the proposal page (made a little smaller at the author's request).

- **Seen in game (1.3.22, the author, game 0.6.4):** of the eight bars only z -1 showed; the author saw it between the ground and the champion. **Read from the three tests:** in the "Game" map after the frame, z <= -100 is under the ground, z -1 above the ground and below the units, z 0 over the units (the units are taken to sit at 0, later draws winning ties: a guess consistent with all three tests).
- **The rings (`ground_ring.rs`, replacing `Movement::draw_rings`):** drawn with the mod interface's lines into the match world at z -1, so a unit's sprite covers the part of its ring behind it and the ring follows the camera and zoom by itself. A soft ground glow (six concentric bands, 18 % at the centre, 26 % for the target), a rim with two halo strokes (2 px core for hover, 2.5 px for the attack target, never under 1.5 px), team colours `#ff4d5e` / `#4fa9ff`. The attack target adds a hairline ring 3 units out with four inward chevrons turning at 14 degrees per second; an attack-click flies the chevrons in from 1.9x over 200 ms while the rim flashes toward white and settles over 260 ms (this replaces the outward pulse). Hover fades in over 90 ms from 92 % size and out over 120 ms. Hovering the target only adds halo. Sizes: 60 % of the measured body width for champions, 80 % for minions, 65 % for towers and the nexus, 6-40 world units, flattened to 38 % like the units' ground shadows.
- **Changed from the proposal:** towers get the same ground ring at their base instead of corner brackets, which would be hidden behind the tower itself in the world layer; the glow is drawn with strokes instead of an image, so no asset or package change.
- **Removed:** the ring depth test tool (`ring_depth_test.rs`, its two render hooks) and its controls.json switch. Unchanged: Off / Outline / Ground ring routing, the automatic fallback while another mod holds the unit renderer, the debug markers, selection and combat.
- **Guess:** ground-level skill effects may lie under z -1 and be drawn over by a ring; translucent strokes may show faint dots where segments meet; the frame cost (about 350 lines per ring) is not measured.
- Tests: compact, flattened radii by body and class; the map centred on the camera; hover fades in and out and yields to the target; the attack-click fly-in and flash settle; strokes keep 1.5 px when zoomed out and stay near the unit.
- Verified: formatting, Clippy with warnings denied, 573 passing tests (one existing translation-export helper intentionally ignored), release build, all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.3.22 after the game closed; installed DLL, metadata and all 101 assets match; the `ring_depth_test` switch removed from the author's controls.json (Unit highlight already on Ground ring). Record: `dist/build-1.3.23.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.3.22 (development build) — 2026-10-19

**Seen in game 2026-10-19 by the author (game 0.6.4): only z -1 showed, between the ground and the champion; see 1.3.23. Development build, never for the Workshop.** The third ring depth test, approved by the author on 2026-10-19.

- **Seen in game (1.3.21, the author, game 0.6.4, two screenshots):** the magenta and cyan dots showed at the screen centre, stayed there while the camera moved and grew when zooming in. The z 0 dot covered the author's champion completely; no bar passed behind another champion; the z -1 000 000 bar never showed. The bars sat well above the champion. **The log:** extent 1024 at that zoom; the dots measured about 2 px per map unit, which is 2048 / 1024 px per world unit.
- **Read from the results:** after the frame the "Game" map draws with the camera centre at the map centre (1024, 1024) and one map unit per world unit; 1.3.21's formula scaled the offset by 2048 / extent a second time, which put the bars too far out. z -1 000 000 is under the ground, z 0 and above over everything; the units' own depth lies between, or depends on their position.
- **Test 3 (`ring_depth_test.rs`, same switch):** the dots are removed; eight 1.5-unit bars cross the champion's body in the corrected coordinates, at z -500 000, -100 000, -10 000, -1 000, -100, -1, minus the champion's world y and minus that y x 1000, labelled on the screen; one `RING TEST 3` log line.
- Test: the camera centre is the map centre, one map unit per world unit, and the eight bars climb the body between z -1 000 000 and 0.
- Verified: formatting, Clippy with warnings denied, 569 passing tests (one existing translation-export helper intentionally ignored), release build, all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.3.21 after the game closed; installed DLL, metadata and all 101 assets match; `ring_depth_test` still 1. Record: `dist/build-1.3.22.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.3.21 (development build) — 2026-10-19

**Seen in game 2026-10-19 by the author (game 0.6.4): map drawing works; see 1.3.22. Development build, never for the Workshop.** The second ring depth test, approved by the author on 2026-10-19.

- **Seen in game (1.3.20, the author, game 0.6.4):** the labels followed the champion; none of the eight bars showed. **The log:** every draw was accepted (`drawn=true`); in `pre_render` the "Game" map does not exist yet (`game_map=None`), after the frame it reports `(2048.0, 2048.0)`. So a mark cannot be drawn first and covered by the units this way (pre_render), and world coordinates in post_render put nothing on screen.
- **Test 2 (`ring_depth_test.rs`, same controls.json switch):** after the frame only, in the map's own coordinates, taken to be the camera's view (centre and extent) spread over 2048 x 2048 (a guess): a 120-unit magenta dot at the map centre at z 0, a 60-unit cyan dot over it at z 1 000 000, and five bars across the champion at z -1 000 000, 0, their own map y, 1 000 and 1 000 000, labelled on the screen. One `RING TEST 2` log line records the map size, the camera and each result. The pre-render bars are removed.
- **Guess:** if the dots show at the screen's centre, map coordinates are right and the bars answer the depth question; if nothing shows, drawing into the match world is not usable and Ground ring stays on the screen overlay ("ghosted back").
- Test: the camera centre is the map centre, and the bars climb the body in their own colours.
- Verified: formatting, Clippy with warnings denied, 569 passing tests (one existing translation-export helper intentionally ignored), release build, all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.3.20 after the game closed; installed DLL, metadata and all 101 assets match; `ring_depth_test` still 1. Record: `dist/build-1.3.21.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.3.20 (development build) — 2026-10-19

**Seen in game 2026-10-19 by the author (game 0.6.4): no bar showed; see 1.3.21. Development build, never for the Workshop.** A test tool for the ground ring redesign, approved by the author on 2026-10-19: can a mark drawn into the match world sit under a unit's sprite? Nothing changes unless switched on.

- **Why:** the 1.3.18 rings "fail to mimic fading behind the champion" (the author, recorded in 1.3.19); every Ground ring so far is drawn on the screen overlay, over the sprites. The mod interface also draws into the match world ("Game" map), and its documentation says `pre_render` draws go under the frame's own commands and `post_render` draws over them, "z permitting". Whether that puts a mark under unit sprites, or under the ground too, is not known.
- **The tool (`ring_depth_test.rs`):** switched on only by `"ring_depth_test": 1` in controls.json, like the practice tool. While the match runs it draws eight 2-unit-thick coloured bars across your controlled champion's body into the "Game" map, rows 0-3 in `pre_render` and 4-7 in `post_render`, each step at z = -1 000 000, 0, the champion's own world y (a guess at the per-unit depth key) and 1 000 000. Each bar is labelled on the screen with its row, step and z. The log gets one `RING TEST` line per step with the "Game" map size and whether each draw was accepted. Ground ring itself is unchanged (1.3.19).
- **Guess:** a bar the champion's sprite hides is one a ground ring could use; a bar that does not show at all is under the ground or off the map's coordinates.
- Test: eight bars, one per step and z, climbing the body in their own colours.
- Verified: formatting, Clippy with warnings denied, 569 passing tests (one existing translation-export helper intentionally ignored), release build, all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.3.19 after the game closed; installed DLL, metadata and all 101 assets match; `ring_depth_test` then set to 1 in the author's controls.json for the test. Record: `dist/build-1.3.20.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.3.19 (development build) — 2026-10-18

**Rollback not seen in game. Development build, never for the Workshop.** Restore Ground ring to its 1.3.16 behavior and remove both redesigns. The author approved the rollback, build and subsequent installation together.

- **Seen in game (1.3.18, author; game version not stated):** the champion ring is too wide and fails to mimic fading behind the champion. **Rejected / superseded, 2026-10-18:** the 1.3.17 segmented and 1.3.18 crescent designs. This supersedes their expected readability/depth benefits; their earlier records and offline previews remain as history.
- **Read from the code:** `Movement::draw_rings` is restored from the original pre-redesign source recorded in the earlier session. Hover again calls the shared debug marker with the original team colour, 2 px padding and 2 px width. Attack targets again use `0xff5b63ff`, 4 px padding and 3 px width, with the existing 120 ms click pulse adding up to 6 px padding and 1.5 px width. Original `combat::highlight_radii` ellipses and selection-envelope tower corners replace the dedicated visual footprints, layers, clamps and base brackets. The shared debug helper is unchanged.
- Removed `ground_highlight.rs`, its module registration, redesign tests and offline export helper. Removed the redesign's use of skill-preview clipping from Ground ring; skill previews themselves are unchanged. Native sprite outlines, Off / Outline / Ground ring routing, automatic compatibility fallback, debug markers, selection, combat and unrelated HUD updates are unchanged. Version 1.3.19 identifies the rollback; this is not a wholesale downgrade of the mod.
- Verified: exact source comparison of the restored Ground ring method with the recorded pre-redesign method, compiler/type check, formatting, Clippy with warnings denied, 568 passing tests (one existing translation export helper excluded), release build, all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Record: `dist/build-1.3.19.json`. Installed with the author's approval over verified 1.3.18, with the game closed; installed DLL, metadata and all 101 assets match. Installation record: `dist/install-1.3.19.json`. No game settings changed. In-game rollback appearance and performance await the author's test. Checklist: `local/testing/TESTING-1.3.19.md`. Local only; not committed, pushed or uploaded.

## 1.3.18 (development build) — 2026-10-18

**Rejected / superseded by 1.3.19, 2026-10-18. Seen in game (author; game version not stated): champion rings too wide and behind-champion fading unsuccessful.** The build-stage statements below are retained as history.

**Not seen in game. Development build, never for the Workshop.** The League-inspired Ground ring direction chosen by the author on 2026-10-18, addressing readability and appearance together. Installation is a separate stage.

- **Seen in game (1.3.17, the author; game version not stated):** the segmented Ground ring is hard to read and lacks the outline's visual impact. The author also asked for an aesthetic improvement. This supersedes the expected clarity of 1.3.17's thin four-arc design, 2026-10-18; the earlier design and its offline preview are retained as history.
- **Read from the code:** the four equal arcs become one broad front crescent with tapered ends and one clean, slimmer rear arc, leaving gaps only at the sides. Front coloured strokes are 3 px for hover / 4 px for attack; rear strokes are 2 / 2.5 px at 75 % opacity. Both get a pale highlight placed on the inner edge of the coloured band, a dark backing and restrained coloured glow. The richer blue / red palette belongs only to Ground ring; native sprite outlines are unchanged.
- The visual footprint is about 15 % larger within the existing bounds, still following body width and camera scale. Attack targets get larger five-point side clamps integrated into the side gaps. Tower-base brackets use cut corners and the same stronger front/slimmer rear styling. Attack clicks retain the 120 ms brightness flash with fixed geometry; no idle pulse, new game hook or image asset. Off / Outline / Ground ring routing, automatic compatibility fallback, debug markers, selection and combat remain unchanged.
- Tests retain sizing/zoom, invalid-data, tower-base and click stability checks; they now check the two-arc layout, readable rear opacity, tapered front, inner-edge highlight placement and continuous bottom crescent. Geometry is bounded at 256 attack strokes / 224 hover strokes before clipping. **Guess:** the fuller silhouette, stronger colour and shaped ends should be easier to read and more coherent aesthetically. Native fit, depth overlap and frame cost need in-game testing.
- Verified: compiler/type check, formatting, Clippy with warnings denied, 574 passing tests (translation and visual export helpers excluded from the normal suite), release build, all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. The visual export helper ran separately and passed; its generated strokes were rendered and inspected on sample backgrounds with placeholder bodies, not game sprites. Record: `dist/build-1.3.18.json`. Not verified: in-game rendering, custom-art fit, occlusion, gameplay or frame cost. Staged locally, not installed, committed, pushed or uploaded in this stage. Test sheet: `local/testing/TESTING-1.3.18.md`.

- Installation update, 2026-10-18: installed with the author's separate approval over verified 1.3.17, with the game closed; installed DLL, metadata and all 101 assets match. Record: `dist/install-1.3.18.json`. This supersedes the build-stage pending installation status above. No game settings changed; in-game appearance and frame cost remain unverified. Not committed, pushed or uploaded.

## 1.3.17 (development build) — 2026-10-18

**Rejected / superseded by the author's requested 1.3.16 Ground ring rollback in 1.3.19, 2026-10-18.** The build-stage statements below are retained as history.

**Not seen in game. Development build, never for the Workshop.** The segmented Ground ring design approved by the author on 2026-10-18. Installation is a separate stage.

- **Read from the code:** normal Ground ring highlights now have a dedicated `ground_highlight.rs` renderer instead of calling the debug marker. Hover shows four long oval arcs with 20-degree gaps at the four axes, a dark backing and a slim team-coloured rim (blue allies, red enemies). The front half is brighter and the rear quieter. Attack targets add a stronger red rim, a fine pale core, a restrained glow and two inward-pointing side chevrons. No overhead arrow is added; the skill preview keeps its own marker.
- Attack clicks brighten the target for the existing 120 ms feedback interval; all geometry and stroke widths stay fixed. The old outward size/thickness pulse is removed. Hover and attack on the same unit still produce only the attack marker. These drawing choices supersede the placeholder rings in 1.3.15 on 2026-10-18.
- Footprints follow stable body-art width, bounded by unit class and camera scale. Body height, hit radius, selection padding and attack range do not enlarge them. Minions use smaller footprints; towers get short brackets at their base instead of corners around the full debug box. The SDK stroke renderer clips them against the viewport, HUD/windows and minimap with the existing skill-preview clipping. No new hook, game address, image asset or setting; the Off / Outline / Ground ring choice and automatic compatibility fallback are unchanged. Debug markers remain on their original drawing path.
- **Guess:** the layered arcs will read more clearly against the ground and the subdued back will suggest depth. These are overlay strokes: actual behind-sprite occlusion is not established, and custom hero art may need size tuning after play. Frame cost has not been measured. Geometry is capped at 272 strokes for an attack marker and 128 for hover before clipping; ordinarily fewer are generated.
- Tests cover four distinct arcs and team colours, front/rear contrast, footprint independence from collision/head size, zoom and smaller minions, fixed geometry during click feedback, target-only side marks, tower-base bounds, invalid-data refusal and the stroke budget. An ignored export helper writes an offline SVG review from the actual generated strokes; it is not a game test.
- Verified: compiler/type check, formatting, Clippy with warnings denied, 573 passing tests (translation and visual export helpers excluded from the normal suite), release build, all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. The visual export helper ran separately and passed; its SVG was rendered and inspected on sample backgrounds with placeholder bodies. Browser preview attempts were unavailable in the execution environment, so the existing SVG rasterizer was used; no browser or library was installed. Record: `dist/build-1.3.17.json`. Not verified: in-game rendering, custom-art fit, occlusion, gameplay or frame cost. Staged locally, not installed, committed, pushed or uploaded in this stage.

## 1.3.16 (development build) — 2026-10-18

**Not seen in game. Development build, never for the Workshop.** The death screen without the greyscale filter, for everyone, with a League-like veil, approved by the author on 2026-10-18 ("remove the grey filter for everyone, dark veil only but with some filter adjustment so it looks closer to League").

- **Seen in game (1.3.14 with "League of Legends Heroes", the author):** the death screen without greyscale (the outline family stepped aside) is "closer to League's death screen".
- **The greyscale composite hook is removed**, not just switched off: `SHADER_SITE` / `SHADER_BYTES` leave the three version tables and their `SHADER_BYTES` anchors leave the three native profiles and `tools/verify_native_profile.py`; `shader_hook`, `DEATH_GREYSCALE` and `set_death_greyscale` are gone. One fewer game spot to find on each patch and one fewer for other mods to collide with. The shader function the outline passes use (`SHADER_ORIGINAL`, checked by `SHADER_PROLOGUE`) stays. This supersedes the death greyscale, 2026-10-18.
- **The new veil (`screen_effect.rs`):** a cool slate tint over the battlefield (`#141a24` at 18 %; 15 % black before), plus a vignette of twelve nested edge bands up to 240 px deep, darkest at the outer edge (`#05080e`, up to 44 % on top of the tint), faded in over 350 ms from the moment of death and gone at once on revival. Both stay off the minimap. The low-health edge warning shares the band code (`edge_bands`) and looks as before. **Guess:** close enough to League's death screen; the colours and depth are to be judged in game.
- Tests: the veil's centre gets the tint only, the outer edge the most, the minimap nothing, within the 196 bands; the fade rises over 350 ms and resets on revival.
- Verified: formatting, Clippy with warnings denied, 568 passing tests (one existing translation-export helper intentionally ignored), release build, all 581 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0 (582 before: the composite hook's spot is gone), DLL interface/null-host refusal and the 103-file package. Installed over verified 1.3.15; installed DLL, metadata and all 101 assets match. Record: `dist/build-1.3.16.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.3.15 (development build) — 2026-10-18

**Not seen in game. Development build, never for the Workshop.** Stage 2 of the compatibility work, approved by the author on 2026-10-18: ground rings as the hover and target cue, automatic while the outline steps aside and as a setting.

- **Seen in game (1.3.14, the author, game 0.6.4):** with "League of Legends Heroes" enabled, LT Takeover now works ("Works with League mod now"). The author also prefers the death screen without the full greyscale filter: "closer to League's death screen". Not changed in this build; asked separately.
- **Setting:** "Sprite outlines" (a switch) becomes **Unit highlight** with three choices: Off, Outline, Ground ring (`hover_outline` 0 / 1 / 2; saved Off and On carry over as Off and Outline). The hint names Ground ring as the more compatible option with mods that redraw units, and says switching to or from it needs a game restart. Ground ring passes the outline family to `native_adapter::install(skip)`, so the unit renderer is never hooked (the install line notes `outline hook skipped (Unit highlight: Ground ring)`); the family set it reports is what was installed. Like the step-aside, this also turns off the death greyscale and the measured sprite boxes (selection uses the envelope boxes with the 8 px padding). Outline draws rings automatically while the outline is not installed (`movement::ground_rings`). Translated in all 16 languages; the two old texts are removed.
- **The rings (`Movement::draw_rings`):** a 2 px ring in the team colour under the hovered unit; a 3 px red ring under the attack target, which pulses 6 px outward and thicker on an attack-click and settles over the outline flash's 120 ms; towers get corner brackets. They use the same markers, ellipses and blockers as the debug markers (now one `draw_marker`), drawn with the SDK's lines, no hook. **Guess:** the lines are drawn over the battlefield, so the ring's front edge crosses the unit's feet instead of passing behind them.
- Tests: rings are drawn when chosen or when the outline is missing, never when Off; the Interface page is 28 px taller (a three-way choice row).
- Verified: formatting, Clippy with warnings denied, 567 passing tests (one existing translation-export helper intentionally ignored), release build, all 582 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.3.14; installed DLL, metadata and all 101 assets match. Record: `dist/build-1.3.15.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.3.14 (development build) — 2026-10-18

**Seen in game 2026-10-18 by the author (game 0.6.4): works with "League of Legends Heroes". Development build, never for the Workshop.** Stage 1 of the compatibility work approved by the author on 2026-10-18: the outline steps aside when another mod holds the unit renderer, instead of the whole mod refusing. Stage 2 (ground rings as the fallback cue and as a setting) is separate.

- **Seen in game (the author, game 0.6.4):** with the Workshop mod "League of Legends Heroes" (3808793258, `league.dll` 0.66.0) enabled, matches played as plain AI simulations. The author's log: `NATIVE install ok=false Native entry differs or another mod hooked it`, then `NATIVE RELEASE`.
- **Read from the code (`league.dll`, strings and its 0.6.4 check table):** it draws high-definition ("dense", PPU) champion frames by hooking the unit body renderer (`0x2aaf630`, 24 bytes checked) and checks one of the renderer's 16 calls (`0x2abfcd5`, 5 bytes) among other spots of its own. It knows game 0.6.4 only (`a1d05045...`); on other builds it says it installs nothing. Of LT Takeover's 0.6.4 addresses only `OUTLINE_ORIGINAL` appears in it. LT Takeover installs at the title screen; the other mod had hooked first (when it installs is a guess), so the renderer entry no longer held `OUTLINE_PROLOGUE` and the install refused every family.
- **The change:** the install now checks every function entry, byte pattern, the outline family's 16 call sites and its shader call first, and decides with `native_profile::families_to_install`. A differing spot of a family listed in `family::MAY_STEP_ASIDE` (only `OUTLINE`) drops that family; every other family installs as before, with all its checks. A differing spot of any other family still refuses everything, and the refusal now names the address (`... another mod hooked it (rva 0x...)`). The layout guards are checked for the families that remain. The log says what stepped aside: `NATIVE OUTLINE stepped aside: another mod changed the unit renderer spots [...]`. The install line now reports the families actually installed (the outline bit missing), not the planned ones.
- **What is off while the outline steps aside (read from the code):** hover and target outlines, the attack-click flash, the death greyscale (the death veil stays), and the sprite draw records, so selection uses the bounded envelope boxes (with the 8 px padding) instead of the measured bodies. **Guess:** if LT Takeover ever installed first, it would take the renderer call and the other mod would fall back to plain frames; stage 2's setting addresses that.
- Test: only a family that may step aside is dropped; any other differing spot refuses everything; a family that is not enabled is not checked.
- Verified: formatting, Clippy with warnings denied, 566 passing tests (one existing translation-export helper intentionally ignored), release build, all 582 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.3.13; installed DLL, metadata and all 101 assets match. Record: `dist/build-1.3.14.json`. Not verified: anything in game, including the step-aside with the other mod. Not committed, pushed or uploaded.

## 1.3.13 (development build) — 2026-10-18

**Not seen in game. Development build, never for the Workshop.** The author's decisions on 1.3.12, 2026-10-18: a fixed padding instead of 10 %, and every unit the vision choice shows is fully playable.

- **Seen in game (1.3.12, the author):** the larger selection areas were not noticeable: "either not working or 10% is too small". **Read from the code:** 10 % of a measured body adds about 2-3 px a side on a champion and about 1 px on a minion at normal zoom.
- **Fixed padding.** Every selection area reaches `combat::SELECTION_PADDING` = 8 screen px past the body on each side, at any zoom, plus the 3 px edge forgiveness as before. This supersedes 1.3.12's `SELECTION_SCALE` (10 %), 2026-10-18. The padding counts as an edge hit in the ranking, like the forgiveness: within a tier, a hit on a unit's body still beats a neighbour's padding. The selection debug drawing (Settings, Debug, Show selection markers) shows the padded rectangle; skill preview markers stay on the body.
- **Every unit the vision choice shows is playable.** The author, 2026-10-18: full vision is the player's choice, so what it shows can be played; there should be no gap between what the player sees and can do. The unit list now follows the strip's vision choice (`Vision::shows`) for hover, the outline, the attack cursor, right-click attacks, skill aiming and the stats frame. This supersedes 1.3.12's split (fogged units for the stats frame only), 2026-10-18; the separate list is removed. The vision choice reaches the simulation workers through `camera::publish_vision` (moved from the stats panel). On Your team nothing changes. **Not known:** whether the game carries out an attack or a unit-target cast on a unit the athlete's team cannot see; it may refuse it. To be seen in game.
- Unchanged from 1.3.12: the vision choice survives teammate swaps.
- Tests: the area is the body plus 8 px at every zoom, just past it does not pick, and a body hit beats a neighbour's padding; the body-shape tests now check the unpadded body (`body_hit`), which they pin.
- Verified: formatting, Clippy with warnings denied, 565 passing tests (one existing translation-export helper intentionally ignored), release build, all 582 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.3.12 after the game closed; installed DLL, metadata and all 101 assets match. Record: `dist/build-1.3.13.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.3.12 (development build) — 2026-10-18

**Seen in game 2026-10-18 by the author: the 10 % larger areas were not noticeable (superseded by 1.3.13). Development build, never for the Workshop.** Larger selection areas, selection of units the vision choice shows, and a vision choice that survives teammate swaps, approved by the author on 2026-10-18.

- **Selection areas 10 % larger.** Every unit's selection rectangle (measured body or bounded envelope; champions, minions, monsters, structures) grows by `combat::SELECTION_SCALE` = 1.10 around its centre, in `combat::screen_area`, the one function every pick goes through. The 3 px edge forgiveness is unchanged. The skill preview's target marker keeps its place on the unscaled body (`combat::body_screen_area`). The selection debug drawing shows the larger rectangle; the debug hover rings (`highlight_radii`) are unchanged.
- **Units the vision choice shows are selectable for the stats frame.** Before, the click only knew units your team sees. The worker now also collects, apart, the units only the strip's vision choice shows (`Vision::shows`): Your team adds none, Opponent adds what the opponent's team sees, All adds every unit. A left-click selects from both lists (`State::stats_pick`). The selected unit stays in the stats frame while the vision choice shows it (it was dropped when your team lost sight). The choice reaches the worker through `stats_panel::set_vision`, written by the client each frame. **Attacks, skill aiming, the hover outline and the attack cursor still use only the units your team sees** (honest mechanics: full vision is for watching).
- **The vision choice stays when you switch to a teammate.** Swaps (Ctrl + 1-5, a portrait, and the "preferred role unavailable" fallback) now use `CameraControl::reset_for_swap`: the camera starts over on the new champion as before, and the vision choice is kept. Only a new match or a save exit resets it to your team. Release still hands the game back the spectator's original vision (unchanged; the next match starts on your team anyway). This supersedes the reset of the vision choice on every swap, 2026-10-18.
- **Guess:** units in the fog may have no recent draw record, in which case their selection area is the bounded envelope instead of the measured body.
- New tests: the area is the body 10 % larger around its centre, and a point just past the old edge now picks; each vision choice shows what that side sees; a unit only the vision choice shows is selectable but never hovered, given the attack cursor or attacked; a swap keeps the vision choice and a new match resets it.
- Verified: formatting, Clippy with warnings denied, 566 passing tests (one existing translation-export helper intentionally ignored), release build, all 582 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.3.11; installed DLL, metadata and all 101 assets match. Record: `dist/build-1.3.12.json`. Not verified: anything in game. Not committed, pushed or uploaded.

## 1.3.11 (development build) — 2026-10-18

**Seen in game 2026-10-18 (game 0.6.4): the speed gain measured, nothing visibly different from 1.3.10. Development build, never for the Workshop.** A cheaper per-frame update for the HUD, the state plate and the session strip, approved by the author on 2026-10-18 from `docs/investigation-takeover-cost-2026-10-18.md` section 3. Nothing on screen is meant to change. The fast path is in the shared property cache (`hud_motion::properties`), so it also serves every other mod window that writes through it: the shop, Settings (with the acquisition panel), the stats panels, the Tab team window, emotes, the screen effect (death veil, low-health bands) and every motion-pass write. Those files are unchanged.

- **Seen in game (the author's log, before):** a steady match window put the HUD at 379 µs and the session strip at 199 µs per frame on average (`PERF ... breakdown: hud=379/763/238 session=199/634/125`), of the mod's 0.84 ms; LT Frame Lab put LT Takeover at 2.4 % of the drawing thread. **After (seen in game 2026-10-18, game 0.6.4, twenty steady match windows of the author's log): `hud=` about 190-230 µs, `session=` about 120 µs, `post_update=` about 450-520 µs per frame against 755 before, so about 0.3 ms a frame less. LT Frame Lab's own-code share stayed at 2.3 %: the removed work was text building and heap allocation, which Frame Lab books under Windows, so its figure was never going to move much.** The `hud=` figure also contains the stats panels (`stats_ui`), which this build does not touch.
- **Read from the code (the cause):** each property write themed its text, split it into fields and built one `field:{path}:{key}` cache key per field, every frame, for every node, even when nothing had changed. `HudUi::props` then stored a second copy of the text under `properties:{node}`, and the session strip a third under the bare node name.
- **The fix (stage 1, `hud_motion.rs`):** each path keeps two whole-text slots, `whole:{path}` (the text last written) and `whole2:{path}` (a text the field comparison found already applied since that write; several HUD nodes get a visibility text and a colour text each frame). A text equal to either returns at once: no theming, no parsing, no per-field keys, one lookup with a reused key buffer. A slot holds the text before theming, behind the background colour it was themed with, so a background change always falls through to the field comparison, also for windows the theme refresh does not rebuild (the state plate). Any other text still goes through the field-by-field comparison, so a geometry change still does not resend unchanged visibility or image setters. The `field:` keys are kept as before. Both slot keys contain the path, so every `cache.clear()` and the stats panel's path-scoped clean-up drop them together with the fields. `HudUi::props` no longer stores its copy; it returns whether the host holds the text, which the image-node log line (`applied=`) now uses. The session strip's unused copy is gone.
- **Hide only when shown:** the state plate's "no plate" branch (every frame of normal play), the HUD's inactive branch and the session strip's "no match" branch now hide their window only when it is not already known to be hidden. Before, they hid it every frame. When the host cannot report visibility, they hide it as before. **Guess:** the SDK does not say whether `ui_visible` reports the node's own setting or its effective visibility (including its parents). If it is the effective one, a hide could be skipped while the game's `ingame` layer is hidden, and the window could show again when that layer returns; the test sheet checks the cases.
- **Session tooltip text:** written only when it changes (cached under `text:tooltip`). Before, it was written every frame while a strip tooltip showed.
- **Not done, deliberately (stage 2, skipping the whole rebuild when the inputs are unchanged):** for `HudUi` and the session strip the inputs cannot be listed with confidence. Their `apply` also runs per-frame work that must not be skipped: the ready chime (`track_readiness`), the shop button's press-and-release, the global tooltip-scroll area, the purchase breathe, the hover sound, closing the swap column on a press elsewhere, the portrait retry timer and the camera-blocking rectangles. The mouse moves on almost every frame of play, so a skip keyed on the cursor would rarely apply. The state plate could be skipped, but its gain is a handful of short texts. All three windows still build their text every frame; only the comparison and the writes got cheaper.
- **Already done before this build:** the session strip's per-frame `ui_set_visible` (brief's stage 3) was fixed in 1.3.7; `themed` already returned at once for text without `#~`, and now is not called at all for an unchanged text (stage 4).
- **Measured in a unit test, not in game:** comparing 150 unchanged, representative node texts took about 247 µs per frame with the old field-by-field path and about 15 µs with the whole-text slot (release build, this PC; a throwaway test, not kept). This leaves out building the texts, which still happens every frame. **Guess:** the in-game `hud=` and `session=` averages drop well below 379 and 199 µs, but not by the same factor.
- New tests: an equal text returns without parsing or writing (even with the field keys gone); alternating visibility and position texts both hit, and a position change sends only the position text; a failed write is retried; a slot written under another background is ignored.
- Verified: formatting, Clippy with warnings denied, 563 passing tests (one existing translation-export helper intentionally ignored), release build, all 582 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package (`tools/records/ui-graphics.json` unchanged at 101 files). Record: `dist/build-1.3.11.json`. Seen in game 2026-10-18: the author went through the whole test sheet (`local/testing/TESTING-1.3.11.md`, Parts A and B) and found nothing different from 1.3.10. Not committed, pushed or uploaded.

## 1.3.10 (development build) — 2026-10-18

**Not seen in game. Development build, never for the Workshop.** Larger charge marks and a one-time ready flash, approved by the author on 2026-10-18. Installation is a separate stage.

- **Seen in game:** the author likes the multi-use skill style but finds the marks hard to see. This was reported after 1.3.9 was installed; the game version was not stated.
- **Read from the code:** charge marks grow from 8 × 6 to 10 × 8 px, with their filled area doubling from 6 × 4 to 8 × 6. The row moves from y 66 to y 64, retains a 2 px gap and remains centred; six marks occupy 70 px inside the 80 px slot. The filled/empty colours and numeric fallback for more than six uses stay as before. This supersedes the smaller marks in 1.3.8 on 2026-10-18.
- **Read from the code:** a skill becoming usable gets a one-time pale-gold inset frame and inward glow: held for 100 ms, then faded over 300 ms. It uses the same fixed rings as the unavailable-press effect; red takes priority if both cues overlap. Gold begins 2 px inside the slot, with a clear centre; key labels, cooldown text and charge marks retain their positions. No pointer or battlefield effect, new image asset, sound or hook.
- The glow shares the existing ready trigger: after an observed recovery of at least half a second, for a learned, usable skill. For multi-use skills it plays when zero usable charges become available, not when another charge refills while the skill is already usable. First sight of an already-ready skill does not flash. The existing learned-slot ready cue also gets the glow after its unlock animation. Both visual cues remain functional with HUD motion Off; the ready chime's four-second gate is unchanged. The original top-line/icon cue keeps its 183 ms timing, while its event timestamp now lives for the full 400 ms glow. This extends the brief ready cue from 1.3.1; the 1.3.9 cooldown endpoint fix stays.
- **Guess:** the larger marks and full-slot ready flash will be easier to see during play; visual strength and readability need the author's test. Tests cover charge-row fit, centring and spacing, flash fade and red priority, first-sight suppression, zero-to-usable recovery, no replay on extra charges, unavailable skills and the existing chime gate.
- Verified: compiler/type check, formatting, Clippy with warnings denied, 560 passing tests (one existing translation-export helper intentionally ignored), release build, all 582 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Record: `dist/build-1.3.10.json`. Not verified: in-game appearance, readability, gameplay or frame cost. Staged locally, not installed, committed, pushed or uploaded in this stage.

## 1.3.9 (development build) — 2026-10-18

**Fix not seen in game. Development build, never for the Workshop.** The focused cooldown-line fix approved by the author on 2026-10-18; installation also approved after building.

- **Seen in game:** after testing 1.3.8, the author said "Looks good" and reported that the cooldown counting bar jumps back down slightly at the frame a skill recovers. The author attributes the bug to a previous update; the game version was not stated.
- **Read from the code:** the moving drain edge could reach less than 2 px from the slot's top, then the ready cue reused that edge at y 2. The position change matches the reported direction. **Guess:** it accounts for the in-game jump; the fix must still be played.
- The countdown edge now stops at the ready cue's same 2 px inset, and the trailing line follows that bounded position. The dark shade still drains to zero. Cooldown readings, readiness, charge counts, cue timing, chime and input handling are unchanged. This supersedes the countdown edge's old near-zero endpoint on 2026-10-18; the ready cue stays inside R's yellow border as designed in 1.3.1.
- Regression test follows the end of a cooldown into the ready frame and checks that the line never steps down, including a skipped final sample and a short cooldown without a ready cue.
- Verified: compiler/type check, formatting, Clippy with warnings denied, 557 passing tests (one existing translation-export helper intentionally ignored), release build, all 582 native instruction/header anchors across game 0.6.3 / 0.6.4 / 0.7.0, DLL interface/null-host refusal and the 103-file package. Installed over verified 1.3.8 with the game closed, as approved; installed DLL, metadata and all 101 assets match. Records: `dist/build-1.3.9.json` and `dist/install-1.3.9.json`. Not verified: the fix's in-game appearance or gameplay. No failed approaches in this stage. Not committed, pushed or uploaded.

## 1.3.8 (development build) — 2026-10-18

**Not seen in game. Development build, never for the Workshop.** The four HUD changes requested by the author on 2026-10-18. Read from the code; native appearance and gameplay await the author's test.

- **Unavailable skill press:** only the affected bottom skill slot gets a dark red inset vignette, held for 80 ms then faded over 220 ms. Ten feathered rings are drawn inside the existing slot; there is no pointer or battlefield effect. A fresh press on an unlearned, missing or cooling skill triggers it, including normal-cast aiming; terminal cast refusals and native-consumer refusals also identify their slot. A held key does not retrigger; a repeated press restarts the cue. Death, identity changes, lost focus and control cancellation clear or hide it. This functional cue stays available with HUD motion Off, like the existing ready cue. Cast validation, targeting, command retention and sounds are unchanged.
- **Multi-use skills:** the existing native charge budget now supplies total uses alongside remaining uses. Two through six uses are shown as centred filled yellow / empty outlined marks at the bottom of the skill slot; larger capacities use a centred `remaining / total` number. When only a retained count is known, the count stays numeric. Single-use, unavailable and missing-data slots show no marks. Retained readings follow the existing one-second display limit and never authorize a cast.
- **Match-start effects removed:** B7's staged HUD reveal and S4's screen-wide sweep are deleted, along with their nodes and once-per-match bookkeeping. D1's strip sweep now triggers on kills only; its first observed kill count is a baseline, including after switching athletes. This supersedes the match-start behavior in the 1.3.3 / 1.3.5 notes, by the author's decision on 2026-10-18.
- **Shop opening:** removed the progress-like top-edge sweep and its node. The short 150 ms fade stays. The settings window's entrance is unchanged. This supersedes the shop sweep in the 1.3.5 notes, by the author's decision on 2026-10-18.
- Tests cover per-slot refusal delivery, held presses, normal-cast preservation, usable charges during recovery, native-consumer refusals, charge counts/capacities, cue fade, missing-data display, and removal of entrance nodes.
- Verified: compiler/type check, formatting, Clippy with warnings denied, 556 passing tests (one existing translation-export helper intentionally ignored), release build, all 582 native instruction/header anchors for 0.6.3, 0.6.4 and 0.7.0, DLL interface/null-host refusal, and 103 staged files. The first test run failed two existing settings-save tests because the default temporary folder denied file replacement; rerunning with TEMP/TMP inside research passed without changing settings code. Package verification is recorded in `dist/build-1.3.8.json`. Not verified: in-game appearance, gameplay or frame cost. Not installed, committed, pushed or uploaded in this stage.

## 1.3.7 (development build) — 2026-10-18

**Nothing here has been seen in game. Development build, never for the Workshop.** It acts on the findings of `docs/investigation-takeover-cost-2026-10-18.md` (the source for every cost below), plus two settings the author asked for. No game address changes and no new hook.

- **Title-screen hitch (investigation, section 1):** the executable's SHA-256 is now computed on a background thread started when the mod loads, instead of on the client thread at the first title frame. That was about 100 ms, seen in game in the logs. `install_once` installs on the first title frame where the fingerprint is ready; while it is not ready, it waits at the title. Everything else is as before: the whole file is hashed every session, nothing is cached across runs, and every fingerprint check, expected-byte check and layout guard still runs. A save reached before the hash finished still gets "Restart game to install adapter at title before loading save". If the thread cannot be started, `install` hashes on the client thread as before. Not done: memory-mapping the file. The read now happens off the drawing thread, so the 87 MB copy no longer costs a frame, and mapping would add unsafe code for no frame-time gain.
- **Management-screen hitch (investigation, section 2), the scan:** `Movement::load_own_team` reads at most 50 athletes' contracts per client frame (`own_selection::TeamScan`). It keeps the partial result between frames and starts over when your team or the save's athlete list changes. A finished scan that found no one (data not ready yet) starts again, as the single-frame version retried. Callers already treat the team as unknown until then.
- **Management-screen hitch, after a match:** the team found is kept across matches. After a match's reset, only the known athletes' contracts are re-checked (about 7 reads). The full scan runs again only if one of them has left, or after a save exit (`Movement::forget_team`). Changed, by design of the brief: an athlete signed between matches is not seen until the next save exit or departure.
- **Management-screen hitch, the parse:** a contract is now read for its `team_id` only, with a small `serde` visitor. Every other value is skipped without building a JSON tree. The rules are the same as the full parse: `InContract` first, then `in_contract`, then the top level, and only an unsigned whole number counts (unit test). `serde` 1.0.229, already built as part of `serde_json`, is now a direct dependency.
- **Diagnostic:** once per game session, at the first management frame where your team is known, one log line lists the keys of your team's record, cut to 300 characters: `MANUAL team record keys team=...`. It is there so the author can see whether the record lists its roster, for a later and cheaper path; no logic uses it.
- **Session strip guard (investigation 3.2):** the strip is set visible only when it is not already, as the HUD does. Before, it was written every frame.
- **Scoreboard guard (investigation 3.3):** the centred layout of the game's scoreboard panel (`ingame.player_info`) is written once when the takeover engages. After that it is written again only when its rectangle shows that the game has moved it off centre (more than 1 px from the screen centre). Before, it was written every frame. Open, a guess: that per-frame write may have been deliberate, because the game may reset other properties of that panel (its colour, its click handling) without moving it; to be checked in game.
- **Outline hook (investigation 3.6):** each unit's drawn sprites are recorded only while you control an athlete on the battlefield. Selection reads them only then (read from the code: hover, clicks, casting, the stats panel's selection and the debug selection areas all run only under direct control); an atomic set by the client frame is checked before anything else. Each unit's slot now reuses its buffer, and the sprites are gathered on the stack, so a frame makes no allocation per unit. Selection stays fresh while you control (records are made every drawn frame, and the 250 ms rule is unchanged).
- **Skill ready chime:** `READY_VOLUME` is 0.6 of "Mod sound volume" instead of 0.4 (the author: "a little quiet").
- **"HUD motion" is On by default for release:** the setting already had one default, On, used by every build. Only its comment changed to say so. The hint stays "Small animations on the HUD; Off keeps it still."
- New tests: the fingerprint wait rule; the team-only contract parse against the full parse's rules; the scan's 50-per-frame chunking and restart on a change of team or save; keeping, re-checking and forgetting the team; the scoreboard write rule.
- Tested: formatting, lint with warnings denied, 552 tests, the verifier for 0.6.3, 0.6.4 and 0.7.0.

## 1.3.6 (development build) — 2026-10-17

**Nothing here has been seen in game. Development build, never for the Workshop.** Three changes the author asked for after playing 1.3.5.

- Seen in game with 1.3.5 (the author, 2026-10-17): the author wanted the respawn line (S3b) removed. The state plate (S1) sat too high and was too small ("move it a bit lower and slightly bigger"). Tab's team-details window opened under the shop window when the shop was open.
- **S3b Respawn wipe removed:** `s3b_respawn_wipe`, `s3b_wipe_pieces`, their test and every `motion: S3b` site in `screen_effect.rs` are gone. That includes the `alive` field and the wipe mode, and `subtract` is private again. The 15 % death veil (S3a) is unchanged and clears at once on respawn, as before 1.3.5.
- **S1 State plate, lower and bigger:** top edge y 100 (was 90). It is 116 × 48 px for the two-element states and 48 × 48 px for the skull alone (was 96 × 40 and 40 × 40). Glyphs are 30 px (was 24), and the F11 key cap is 36 × 26 px (was 30 × 22). Still centred with 2 px rounding; its bottom edge, 148, stays above the kill callouts at y 150 (a compile-time check in the test). All three states change together.
- S1 layout: the content sits 20 px in from each side (glyph, 10 px gap, cap), or 9 px for the skull alone.
- S1, interpreted: "one size larger" read as one point: the key text goes 13 → 14 and the respawn count 22 → 23.
- **Tab window above the shop:** every z in the team-details window (`lt_team_status`, its template and its extra item tile) is raised by 400. They go from 1200–1207 to 1600–1607. Read from the code, the shop's highest z is 1591 (the S6 entrance hairline). Its tooltip is 1550–1552 and its toast 1530–1531. The settings window starts at 1999. The team window has no hover or click handling of its own: every node ignores events. How Tab opens and closes it is unchanged. That a higher z puts it above the shop is a guess, not seen in game.
- Tested: formatting, lint with warnings denied, 547 tests, the verifier for 0.6.3, 0.6.4 and 0.7.0.

## 1.3.5 (development build) — 2026-10-17

**Nothing here has been seen in game. Development build, never for the Workshop.** The second HUD motion pass, approved by the author after playing 1.3.4 (local design note `design/hud/motion-pass-2-2026-10-17.md`). Drawing through the SDK and the mod's own code only: no native hook, no game address.

- Seen in game with 1.3.4 (the author, 2026-10-17): "mostly good", but too conservative to notice and almost all on the bottom strip; B3 (stat lift) not noticeable and not good looking; the denied cast sound (the game's menu click) does not read as "cast failed". The author added two principles: no HUD text where a glyph or graphic does the job, and motion where the eye is.
- **B3 dropped:** `b3_stat_lift` and its colour writes are gone; the kills, deaths, assists and CS labels keep their template colours, as before 1.3.3.
- **Denied cast sound:** its own clip, `probe/sound/sfx/cast_denied.wav`, which is Kenney's `error_008.wav` (Interface Sounds pack, CC0, 139 ms), copied unedited and chosen by the author by ear. It plays at 0.4 of "Mod sound volume" (`DENIED_VOLUME`), the chime's level. The 150 ms spacing, the "Denied cast sound" setting and the shop's refused purchase (which shares it) are unchanged. Listed in THIRD_PARTY_NOTICES.md.
- **Install fix (found while checking the clip; read from the scripts, not seen):** the installer and `package_mod.py` copy only the files listed in `tools/records/ui-graphics.json`, and `skill_ready.wav` was never listed. So the 1.3.2 chime was staged but never copied by `install_verified_build.ps1`, and packaging would have refused the staged folder. Both `skill_ready.wav` and `cast_denied.wav` are now listed, and `verify_build.py` expects 101 files instead of 99.
- **S1 State plate (a feature, shown with "HUD motion" Off too):** the death banner is gone; a plate with no words takes its place under the game's clock. Paused: the pause glyph in signal yellow and an "F11" key cap drawn like the strip's Tab cap. AI driving your athlete: the processor glyph and the F11 cap. Dead: the skull and the respawn count in the mod's numeric font (size 22, whole seconds rounded up, from simulation ticks), or the skull alone while the count is unknown. Paused wins over dead, and dead over AI; that choice is a pure function with a unit test (`plate_state`). With HUD motion On it fades in and out over 150 ms; Off, it switches. The strings "Respawning in {seconds}" and "Respawning…" are deleted from all 16 language files; the strip's own `respawn` number is unchanged.
- S1, changed: the plate is a root of its own (`lt_state_plate`), not a node of the HUD strip. The strip is hidden while the AI drives your athlete, so a plate inside it could never show the AI state. It keeps the banner's top edge (y 90, centred) and is 96 × 40 px, or 40 × 40 px for the skull alone. It takes no clicks and is not in the camera's blocked rectangles. It does not depend on the motion module: dropping the pass keeps it.
- S1, read from the code: the worker keeps sampling your athlete during AI control, but the frame's snapshot skips the freshness check outside Running. During AI control the plate applies that check (samples older than 250 ms are dropped), so a stale dead state or count cannot stick on it. "Watch this match" is also AI control, so it shows the AI plate.
- S1 and S2, open for the author (a guess, not seen): opening Settings pauses the match, and so does the shop with "Pause while shopping". The plate then shows the pause glyph and F11, and the S2 brackets draw. The plate sits under the settings window's backdrop; whether the S2 lines draw above the windows is not known.
- **S3a Lighter death veil (a feature):** the battlefield veil while you are dead is 15 % black (`0x00000026`) instead of 40 %. The strip's B6 veil is separate.
- **S2 Pause brackets:** while paused, four signal-yellow corner brackets sit 16 px in from the screen's corners. Each is the minimap frame's tick at twice its size (legs 23 px), 2 px wide. They fade in over 200 ms tonal and out on resume, and stay steady while paused. They are drawn lines (post-render, like the minimap frame), so they take no clicks. Taken literally, as the note says: the bottom-left bracket overlaps the Pause button's area and the bottom-right one sits over the minimap's corner.
- **S3b Respawn wipe:** when your athlete's alive flag turns true while you play (not on an AI handoff), a 2 px signal-yellow line sweeps the battlefield left to right over 300 ms ease. The 15 % veil's rectangles are cut at the line each frame with the existing `subtract` helper: dark to the right, clear to the left. The line then fades over 100 ms at the right edge. It stays inside the viewport and off the minimap (unit test). The low-health tint draws alongside it.
- **S4 Match start sweep:** once per match, at the HUD's first frame, a 2 px signal-yellow line sweeps the full 1920 px width at y 52 over 500 ms ease, then fades over 150 ms; then B7 plays.
- S4, changed: the line has a gap over the game's clock (x 930–990). A line at y 52 crosses the clock (y 44–72), and the note's guard says nothing may cover the clock.
- **S5 Low-health heartbeat (replaces A4):** the edge tint breathes 70–100 % of its strength on a cycle that shortens as health falls. The phase advances by each frame's share of the current cycle, so the breathing does not jump as health changes.
- S5, changed: the note's `3 − 2 × severity` gives 2.3 s at the threshold, because the existing severity is 0.35 there, not 0. So the formula is applied to severity's own 0.35–1 band. That gives the note's named 3 s at the threshold and 1 s at near-zero health (unit test).
- **S6 Window entrance:** the frame the shop or the settings window becomes visible, its content fades in over 150 ms, while a 2 px signal-yellow hairline sweeps the window's top edge over 300 ms ease and then fades over 100 ms. Once per show: not on a page or tab change, and not when the window was hidden less than 200 ms before.
- S6, changed: there is no group opacity, so the "surface alpha" fade is a veil in the background colour fading out over the window, as B7 does. The window's frame and the settings window's dark backdrop appear at once. Neither window had a "shown this frame" moment, so each now has a motion `Pass` field that keeps its shown flag and hide time.
- **S7 Target panel brackets (B9 moved):** when the selected unit changes (including a first selection), four corner brackets converge on the top-left target panel's corners from 10 px outside over 300 ms ease, then fade over 150 ms. Each is the minimap frame's tick, 1.5 px, `#817c77`. They are drawn lines. B9's attack-target markers (a debug option) are back to 1.3.2. `stats_panel::target_id` is now crate-visible for this.
- A1 Purchase slot breathe, retuned: 70–100 %.
- A2 Recall progress, retuned: the line is 3 px and the notches pass every 0.3 s.
- A3 Drain-edge comet, retuned: 60 % alpha, 150 ms follow.
- B1 Damage chunk, changed: the trail is fully in the warning colour (`#ff642e`) for 300 ms, then blends back into its own colour over the remaining 200 ms. The start is a drop of your target health (not the eased fill), so every new hit restarts it; healing does not.
- B2 Gold roll, retuned: 400 ms.
- B4 Level-up, retuned: the learned shade drains over 300 ms.
- B5 Item bought, retuned: the edge is held yellow for 300 ms, then fades over 200 ms; the icon fades in over 150 ms. The next item's 100 ms fade-in is not in the note's table and is unchanged.
- B6 Death and respawn, retuned: the strip veil is 50 % and darkens and lifts over 400 ms, the skill slots 60 ms apart. The banner fade left B6 for S1.
- B7 Match entrance, retuned: 200 ms per group, 60 ms apart, starting after S4 has swept and faded (650 ms). The groups stay veiled while the line sweeps.
- B8 Swap stagger, retuned: the portraits appear 50 ms apart and the sweep takes 300 ms; the fade after the sweep stays 117 ms.
- D1 Stats hairline, retuned: a 2 px line across the full strip width (1560 px, from x 0, at y 138), over 500 ms.
- C1, C2 and D2: unchanged.
- Every new item keeps the pruning rule: one function per item in `hud_motion_pass.rs` and a `motion: <item>` tag at every site outside it (the module header's table lists them). S1 and S3a are features outside the module; only S1's fade is in it.
- New and changed tests: the plate's priority and count; the S5 cycle end points and depth; the S3b wipe cut (viewport, minimap and line); S6 once per show and no replay on a quick reopen; S7's motion; B7 waiting for S4; B1 starting on a drop only; the 60 ms B6 stagger; Off writing every new node's end state; the template check now also covers the state plate, in all 17 languages.
- Tested: formatting, lint with warnings denied, 548 tests, the verifier for 0.6.3, 0.6.4 and 0.7.0. Not measured: the frame cost with the pass on (the PERF window lines, On versus Off).

## 1.3.4 (development build) — 2026-10-17

**The fix itself has not been seen in game. Development build, never for the Workshop.** Repairs the session strip that 1.3.3 broke.

- Seen in game with 1.3.3 (the author, 0.6.4): no preparation window at match start, and after F11 the strip's top controls, swap column and vision buttons were missing. The game and the match were fine.
- Cause, read from the code: the B8 edit pushed the `#b8_sweep` node after the session strip template's closing brace, outside its root node, so the game built no strip. The "HUD motion" switch could not help, because the template is assembled the same way with it Off. The HUD's and the screen effect's templates were checked the same way and were whole: their new nodes sit inside their roots.
- Fix: `#b8_sweep` is inserted before the Tab key cap, inside the strip's root, with the same position, size, z and hidden default. Nothing else changed: no new motion items, no retuning.
- New test: the template check (`lang::check_template`) now also requires exactly one root node with nothing after it, not just balanced braces. It runs, in all 17 languages (English and the 16 translations), over every template the mod spawns: settings, shop, session strip, player HUD and its extra item tile, emotes, Tab panel and its extra item tile, both stats panels and the screen effect. A second test feeds it a template shaped like the 1.3.3 bug and expects it refused. The stats panels, screen effect and both item tiles gained test access to their template text for this.
- Tested: formatting, lint with warnings denied, 543 tests, the verifier for 0.6.3, 0.6.4 and 0.7.0.

## 1.3.3 (development build) — 2026-10-17

**Nothing here has been seen in game. Development build, never for the Workshop.** The experimental HUD motion pass, approved by the author (local design note `design/hud/motion-2026-10-17.md`). It is built so the author can prune it after playing:
- **"HUD motion"** (Settings › Interface › Appearance) is one switch for the whole pass, On by default in this development build. Off writes every item's end state, so the HUD should look as in 1.3.2.
- Every item lives in `probe/src/hud_motion_pass.rs` as functions named after the note (`a1_purchase_breathe`, `b2_gold_roll`, ...). Items that draw new nodes also have a `*_nodes` template function.
- Not every item is a single call in the existing code: B3, B4, B5, C1, B8, B9 and A2 touch two to four places each. Every edit outside the module carries the comment `motion: <item>`, and where an item changed an existing expression the comment says what to restore. To prune an item, delete its functions and every place its tag marks. To drop the pass, delete the module and everything tagged (`motion: pass` and `motion: switch` included). The module's header has the table.
- No new animation system: `hud_motion::Motion` (it gained `seed` and `remove`), its two curves and the property cache. Presentation only: no hit rectangle or input changes.

Items:
- **A1 Purchase slot breathe:** as specified. The purchase coin and check breathe 90 % → 100 % on a 2.4 s tonal cycle while the next item is affordable.
- **A2 Recall progress, changed:** the client does not read the recall's duration. So while the game reports your athlete recalling, a 2 px yellow line shows under the recall icon with two notches marching along it every 0.6 s, but no progress fill. The notches stand in for the note's chevrons. "Recalling" is the game's action number 1 for your athlete, read from the code (the state the `RECALL NATIVE_ACTION` log line follows), seen within the last 250 ms.
- **A3 Drain-edge comet:** a second edge line at 40 % alpha follows the drain edge, in its warming colour. It is an eased 100 ms follow, not an exact 100 ms delay. Hidden with the edge and during the ready cue.
- **A4 Low-health breathe:** the existing low-health edge tint's strength breathes 70 % → 100 % on a 3 s tonal cycle while the tint shows. Its own setting still decides whether it shows. The note gave no depth; 70 % is mine.
- **B1 Damage chunk:** the health trail takes the warning colour (`#ff642e`, the design sheet's `--signal-warning`) while it lingers behind the fill. 100 ms tonal.
- **B2 Gold roll:** as specified. The gold number counts to the new value over 167 ms ease, in whole gold, with one tween key.
- **B3 Stat lift:** a changed kills, deaths, assists or CS number shows at 70 % of its own colour and lifts to full over 100 ms tonal. Deaths stay in their orange. One key per stat; nothing lifts at first sight (match start, swap).
- **B4 Level-up:** the level numeral lifts as B3. A newly learned slot's dark shade drains away over 167 ms ease, then the 1.3.1 ready cue plays. That cue did not play on learning before, because a learned slot was never cooling: B4 starts it, without the chime. While the shade drains, the icon stays at the 75 % cooling brightness, so the cue lifts it from there without a dark blink. Switch Off: as 1.3.2, no cue on learning.
- **B5 Item bought, changed:** a new item's icon fades in over 117 ms, and its slot edge is signal yellow for one beat (83 ms), then fades over 100 ms. The purchase slot has one image node, so a new "next item" fades in over 100 ms instead of cross-fading from the old one.
- **B6 Death and respawn, changed:** the game's UI has no group opacity or brightness, so the dim is a 40 % black veil over the strip and one over each skill slot (not over the gaps between them, which show the battlefield). That gives 60 %, and it multiplies the skills' 75 % cooling brightness instead of replacing it. Timing: the banner fades in over 117 ms and the veils darken over 300 ms tonal. On respawn they lift over 300 ms, the skill slots 40 ms apart, left to right. The session strip (pause, AI, camera, ...) is drawn above the HUD and is not dimmed.
- **B7 Match entrance, changed:** once per match (not on a swap), at the HUD's first frame. The groups appear in their real left-to-right order on this HUD, which is not the note's list: stats; level, recall and health; the skill slots; items, gold and purchase. 117 ms each, 30 ms apart. With no group opacity, each group appears out of the strip colour (a veil in the background colour fading out) rather than from transparent; over the battlefield the skill slots appear out of dark squares.
- **B8 Swap stagger, changed:** while the swap column unfolds, the portrait images appear 30 ms apart, support (nearest the button) first. Only the image's opacity changes; the tiles and their clicks do not. When a swap lands, a yellow hairline sweeps the swap button's bottom edge over 167 ms ease, then fades over 117 ms. The column folds away as the swap lands, so the button carries the sweep rather than the chosen portrait. Two calls: the sweep in the session strip, and `b8_swap_landed` where a swap succeeds.
- **B9 Target brackets, changed:** the only target frames are the attack-target markers drawn with Advanced › Debug › "Show selection markers"; normal play shows nothing. When the attack target changes, a tower's corner brackets converge from 6 px out over 167 ms ease. Other units have a ring, which fades in over 117 ms instead (the note's edge-fade case).
- **C1 Slot hover:** skill slots only, since item slots already responded to hover. The dark shade lightens (its alpha down to 70 %) and the key cap brightens to white while the pointer is over the slot, 100 ms tonal.
- **C2 Slot press:** item icons only, since skill icons already dimmed to 85 % on press. A pressed item slot's icon dims to 85 % over 83 ms.
- **C3 Tooltip fade:** already present, nothing added. The HUD, shop and session strip tooltips fade in over 117 ms; the Tab and stats panels have none.
- **C4 Controls press:** already present, nothing added. The session controls have had a 100 ms hover tint and an 83 ms press dim since they were built.
- **D1 Stats hairline:** as specified. A 1 px yellow line sweeps left to right under the K/D/A and CS row, once at match start and once per kill of your athlete, over 300 ms ease, then is gone.
- **D2 Minimap ticks settle, changed:** the client reads no pan velocity. "Moving" is the camera frame's centre changing by more than 0.5 between frames (a guess, meant only to ignore rounding drift). With camera lock on, the ticks stay out the whole time your athlete walks.
- Tested: formatting, lint with warnings denied, 542 tests (new: the shared timings, B6's dim and stagger, first sight never animating, once per match surviving a swap, Off returning every input unchanged and writing every new node's end state, turning On late replaying nothing), the verifier for 0.6.3, 0.6.4 and 0.7.0. Not measured: the frame cost with the pass on (see the test sheet's PERF step).

## 1.3.2 (development build) — 2026-10-17

**Nothing new here has been seen in game. Development build, never for the Workshop.** The ready chime, approved by the author after playing 1.3.1.

- Seen in game with 1.3.1 (the author, 0.7.0): "Looking good, and I can hear the base game sfx just fine". The game plays its own `UI_mouse_hover` and `UI_mouse_click` through `play_sound`, so that guess is now seen in game. The icon dimming while a skill recovers, the number format, and the click for an item queued while gold is short were accepted as built.
- **The chime ships:** `probe/sound/sfx/skill_ready.wav` is `confirmation_001.wav` from Kenney's Interface Sounds pack (www.kenney.nl, CC0; mono 44.1 kHz 16-bit, 0.29 s), copied unedited. It is packaged beside `emote.wav` and listed in THIRD_PARTY_NOTICES.md.
- It plays at 0.4 of "Mod sound volume" (`READY_VOLUME` in `ui_sound.rs`), so the level can be tuned without editing the file.
- It sounds only when the cooldown that just ended was at least 4 seconds long (240 ticks, `READY_CHIME_MIN_TICKS` in `player_hud.rs`, judged by the slot's highest remaining cooldown seen). The author's reason: a champion whose skill returns every two seconds must not chime all match. The visual ready cue is unchanged and still plays for every learned, usable skill whose cooldown lasted at least half a second. "Skill ready sound" stays Off by default.
- Tested: formatting, lint with warnings denied, 536 tests (the ready-cue test now also covers the 4-second gate), the verifier for 0.6.3, 0.6.4 and 0.7.0.

## 1.3.1 (development build) — 2026-10-17

**Nothing here has been seen in game. Development build, never for the Workshop.** The four HUD cues agreed with the author on 2026-10-17 (local design note `design/hud/cues-2026-10-17.md`). Client drawing and the SDK's `play_sound` only: no new native hook, no game address.

- **Skill ready cue.** The frame a learned, usable skill's cooldown reaches zero, the drain edge shows signal yellow for one beat (83 ms, the motion language's press step; the note does not say how long a beat is), then fades on the tonal curve over 100 ms while the icon lifts to full brightness over the same 100 ms. Each slot on its own, nothing else on the HUD reacts; no flash, no scale. During the cue the line sits just inside the slot's border (y 2), so it shows on R, whose ready border is itself 2 px of yellow. Not after cooldowns under half a second, for unlearned or `--` slots, or while dead.
  - Interpretation, for the author to judge: the note has the icon lift "from the cooldown dimming", but the only dimming today is the drain shade, about 1 px tall at the ready frame. So the icon now sits at 75 % brightness while a learned, usable skill recovers (`COOLING_ICON` in `player_hud.rs`, multiplied with the 85 % press tint) and lifts from there. The alternative, dimming the icon only at the ready frame and lifting it, would read as a dark blink.
  - **Skill ready sound** (Settings › Interface › Sounds, Off by default): plays `asset/lt_direct_control/sound/sfx/skill_ready`. No file ships yet. If `play_sound` reports failure, one `SOUND play request refused path=...` line is logged (Normal level) and nothing else happens. Whether `play_sound` reports a missing file as failure is not known.
- **Denied cast sound** (Sounds, On by default): sound only. It plays for every cast press the mod turns away (on cooldown, native validation rejected, no eligible target under the cursor, self-cast unsupported, aim unavailable, ability data not ready, bound target gone) and when the game's skill consumer did not spend the cooldown ("pressed during an action"). Not for an unlearned skill, not while dead, and not for a press that was waiting and expired. At most one sound every 150 ms. Presses count, not held keys: holding a key gives one sound. Clip: the game's `asset/base/sound/sfx/UI_mouse_click` at 0.35 of the volume slider, for the author to judge by ear. The worker thread only adds one to an atomic counter; the client frame plays it.
- **Cooldown numbers:** size 30 instead of 27 in the same place. Tenths below three seconds (`2.4`), whole seconds from three (`12`), both rounded up, with no `s` (before: `12.3s` throughout). The weight was already 650: `tools/generate_hud_fonts.py` builds the numeric font at 650. The drain edge warms from its white to signal yellow over the last 10 % of the cooldown. The note says "grey"; the edge has been white since it was added, and white was kept as the start. The vertical drain stays.
- **Interface sounds** (Sounds, On by default): the game's own `UI_mouse_hover` when the pointer enters a clickable element of the shop, the settings window or the session strip (top controls and swap column), and `UI_mouse_click` on clicking one. Hover sounds once per element entered; not for elements under a still pointer (a window opening, a page switching, a list scrolling), not in the first 100 ms of a window, not while the left button is held (dragging a slider), not for the strip under the settings or shop window, not for settings rows scrolled under the window's header or footer. Also a click on pause and resume (key or button) and on a champion swap, and at most one click every 100 ms, so a click and the pause it causes sound once.
  - Shop: an order the shop takes plays the click, including an item queued because gold is short ("Queued · not enough gold yet"). Orders the shop refuses play the denied sound: an owned item, no free slot, manual shopping off. The note listed "not enough gold" as refused, but the shop queues such an item, so this build follows what the shop does. The author decides.
- **Mod sound volume:** the emote volume slider (Emotes › Display) is renamed and now sets the volume of emotes, the cues and the interface sounds. Its saved key is still `emote_volume`, so existing settings keep their value. The three switches are on the Interface page, in a new Sounds section.
- Read from the game data, not tested: all three supported games (0.6.3, 0.6.4, 0.7.0) ship both clips under exactly these paths, and the game's own widgets name them as `hover_sound`/`click_sound`. That `play_sound` accepts a base-game path, as it does the mod's emote path, is a guess. The SDK passes the path through, and its doc example is a mod path. Any clip the game refuses is logged once (`SOUND play request refused path=...`); no such line proves nothing if `play_sound` only reports that the request was queued.
- Tried and not used: the game's UI events report no "pointer entered" event (only click, right-click, checkbox, tree view, text edit, remove, custom, changed), so hover is found from the clickable nodes' rectangles and visibility each frame.
- Tested: formatting, lint with warnings denied, 536 tests (new: number format and boundaries, edge warming, ready cue timing, the 150 ms denied limit, click and hover rules), the verifier for 0.6.3, 0.6.4 and 0.7.0.

## 1.3.0 — 2026-10-17

Release of the 1.2.4 to 1.2.16 development builds below (1.2.6 was uploaded on 2026-10-15 and withdrawn the same day, so the last public build was 1.2.3). One DLL for game 0.6.4 (the live game), 0.6.3 and the 0.7.0 beta, as since 1.2.0.

Seen in game by the author: on 0.7.0 throughout (1.2.7 to 1.2.16), and with 1.2.16 on 0.6.4 (the stat rules install with 33 sites, shape-B hits traced, game speed even when the frame rate dips, the input delay number, the practice tool). Not played for this release: 0.6.3 (the verifier passes; the author no longer tests the stat rules or later changes there), and a shape-A hit on 0.6.4 (seen on 0.7.0 only).

- **Athlete stat rules** (experimental, Off by default; Settings › Advanced › General › Athlete stats). In matches you play yourself, your athlete's stats stop deciding hits behind the scenes:
  - Your skillshots get a fixed hit margin, **Collision margin** (default 2 game units, 0 to 7), instead of one that grows with your Skillshot Accuracy and shrinks with the enemy's Dodging (read from the code: up to 15 units off a 10-unit body). It applies to every shot of yours, whichever of the game's two kinds of hit test it goes through (1.2.11; the author's tests showed collision size is the biggest factor in whether the AI dodges).
  - Your own Dodging no longer shrinks your body for enemy shots, and how soon an enemy reacts to your shot follows your Accuracy against its Dodging, between **Reaction floor** (6 ticks) and **Reaction ceiling** (14 ticks).
  - AI-versus-AI fights and AI shots at you stay as the game made them; Off is the game's own behaviour. Turning it on takes effect after returning to the main menu, where the hooks are installed (33 instruction sites per game version, each checked against its executable).
  - Skill previews widen projectile corridors by the margin while the rules are in effect. **Show hit areas** (Advanced › Debug) draws the hit bodies of enemies near the cursor.
- **Show input delay** (Advanced › Debug, Off by default): the measured time from your click or key to the frame that plays it, averaged over your last 20 commands, as a number like `19ms` in the top right corner under the game's header. Windows' input handling and the display itself are not included.
- **Even game speed just below 60 fps:** the instant speed-ups when the frame rate dipped to 57 to 59 fps are gone (1.2.13). The game's viewer kept time it could not play and paid it back two ticks a frame; owed time now comes back a quarter tick a frame, and beyond 2 ticks it is dropped. No input delay is added.
- **Settings window:** slider values like "14 ticks" fit on one line in every language (1.2.6).
- Testing aids outside the settings window, for modders: the practice tool (`"practice_mode": 1` in controls.json: everyone but you holds still, Home+Insert lines the enemies up and pins them, a readout of your shots' hit tests) and Home+End for no cooldowns.
- For modders: `tools/verify_stat_sites.py` works each stat site's registers and offsets out from the code and lists every stat-site-shaped instruction of an executable; `tools/draft_version.py` uses both to carry `stat_sites.rs` to a new game version (docs/version-migration.md).
- Tested for this release: formatting, lint with warnings denied, 531 tests, the verifier for 0.6.3, 0.6.4 and 0.7.0, and the author's play listed above.

## 1.2.16 (development build) — 2026-10-16

**Nothing here has been seen in game. Development build, never for the Workshop.** The input delay number, placed and set as the author asked.

- Seen in game with 1.2.15 (0.7.0, the author, screenshot): the number overlapped the game's top header (the red team's name) and was not in the mod's font.
- Now just under the game's header bar, which is 50 px tall across the whole width (read from the game's `ingame.ui`, `#header`): y 56 instead of 10, still right-aligned at the right edge.
- In the mod's numeric font, `asset/lt_direct_control/font/numeric` (Manrope; it has the letters too, so `ms` and `--` render in the same font), as the team panels' numbers.
- Tested: formatting, lint with warnings denied, 531 tests, the verifier for 0.6.3, 0.6.4 and 0.7.0.

## 1.2.15 (development build) — 2026-10-16

**Nothing here has been seen in game. Development build, never for the Workshop.** The input delay display as the author asked, like League's ping.

- Seen in game with 1.2.14 (0.7.0, the author): the input delay line shows fine.
- Now a plain number in the top right corner, `19ms` (`--ms` before a measurement), size 28 instead of 16, right-aligned in the 1920 x 1080 UI space; no box and no label. A dark shadow 2 px down and right keeps it readable on bright ground. Whether it overlaps the game's own top-right UI has not been seen.
- Tested: formatting, lint with warnings denied, 531 tests, the verifier for 0.6.3, 0.6.4 and 0.7.0.

## 1.2.14 (development build) — 2026-10-16

**Nothing here has been seen in game. Development build, never for the Workshop.** An input delay display, as the author chose (measured average; a setting, Off by default).

- Seen in game with 1.2.13 (0.7.0, the author): "it feels better now". The traced frames after lead changes show no two 2-tick frames in a row (one 2 in 180 frames), owed time up to 0.75 ticks, nothing dropped. A flaw of that diagnostic: a new lead change restarts the trace, and the lead fell back within about 12 frames each time, so only the `2 -> 1` traces completed.
- **"Show input delay"** (Advanced > Debug, Off by default): a line top left during your matches, `Input delay 19 ms`, the average over your last 20 commands of the measured time from the mod reading the click or key to the client frame that plays the tick carrying it (the existing command traces; with performance capture on, the 1.2.13 log measured 18 to 23 ms on average, about 35 ms usually at most). Not included: Windows before the mod reads the input, and rendering and the display after. Measurements older than 30 s are not shown. While it is on, commands are traced past the usual 300 per session (the trace lines themselves stay Verbose-only). The on-screen line is English only, like the test tags; the setting's label and hint are translated.
- Tested: formatting, lint with warnings denied, 531 tests (the average and the 30-second window), the verifier for 0.6.3, 0.6.4 and 0.7.0.

## 1.2.13 (development build) — 2026-10-16

**Nothing here has been seen in game. Development build, never for the Workshop.** The speed bursts near 60 fps, approved by the author (diagnostic and gentle payback).

- **Seen in game** (1.2.12 on 0.7.0, the author): an instant of faster game speed when the frame rate dropped from 60 to 58. The log has 8 `TIMING frame lead 1 -> 2` / `2 -> 1` switches in 30 s, each up when the average frame reached 17.5 to 17.9 ms and down 0.2 to 0.4 s later. The same lines are in the 1.2.9 logs: not caused by the stat hooks.
- **Read from the code** (viewer update `0xb194c0` on 0.7.0): the viewer adds each frame's time to its clock (`+0x358`) and plays one queued tick per tick of clock; when the queue runs empty the rest stays in the clock, with no bound on the path examined (a flag-gated path at `0xb19c47`..`0xb19c95` that caps the clock was not examined). With the worker one tick ahead (lead 1, kept at up to 5% slow frames for the lowest input delay), owed time piled up (about 1.7 ticks a second at 58 fps) and came back as two ticks in every frame once the lead rose. That this is the burst the author saw is a guess. The diagnostic below records frames after the fix, so it can show that owed time built up (`dropped` above 0) and that no two frames in a row play two ticks; whether the bursts are gone has to be seen in game.
- **Fix:** before each running frame, whole ticks the clock could not play are held back and paid back at most a quarter tick per frame (at most 25% fast for a few frames), and owed time beyond 2 ticks is dropped (the match then runs slow by at most the 5% the lead already tolerates, as intended, instead of catching up in a burst later). A pause forgets owed time. No input delay is added: the lead rule is unchanged. Tests: the arithmetic, and a 58 fps simulation of a model of the viewer (not the game) in which no two frames in a row play two ticks once the lead has risen.
- **Diagnostic:** for 60 frames after each of a match's first 8 lead changes, `TIMING after lead a -> b: ticks per frame 1112111...; owed up to x ticks; dropped y ticks`. The ten-second `PACING`/`TIMING slow` lines add the ticks dropped this match.
- Tested: formatting, lint with warnings denied, 530 tests, the verifier for 0.6.3, 0.6.4 and 0.7.0.

## 1.2.12 (development build) — 2026-10-16

**Development build, never for the Workshop.** Settings changes asked for by the author, and the migration tools learn the stat sites (runbook plan item 5).

- Seen in game with 1.2.11 (0.7.0, the author): `STATRULE installed sites=33`; Pyromancer Q (kind 3) and Alchemist Q (kind 10, then kind 4) reached shape-A floors (`0x10ca823`, `0x10ca97b`, `0x10ca2c3`, `0x10ca40b`, `0x10ce9c8`, `0x10ceb24`) with margin 7; "looking fine".
- "Athlete stat rules" hint: turning it on takes effect after returning to the main menu (the hooks install only at the title screen). English and the 15 translations.
- "Collision margin" defaults to 2 units (was 0), the author's choice as friendlier. A value already saved in controls.json is kept (only development builds wrote one).
- The practice tool is out of the settings window: a debug tool like no-cooldown, switched on only by `"practice_mode": 1` (or `true`) in controls.json, read at game start, so the lineup still applies from the draft. Its label and hint are removed from the language files. `Settings::hidden_switch` reads it; a test checks it survives a save.
- **`tools/verify_stat_sites.py`** now works the fields out instead of only comparing them: `site_fields` derives each site's registers, frame slots and sources from its instructions (the same rules as before), `check` lists every disagreement with a reason instead of stopping at the first, and `census` finds every instruction of an executable shaped like a stat site (both consumer shapes, both producers, gate-shaped hashes, other Dodging subtractions). Checked: refuses a wrong field for every kind; on 0.6.3, 0.6.4 and 0.7.0 the census finds exactly the tabled sites, plus one gate-shaped hash per version that reads no Accuracy (open item 5 of the 0.6.4 investigation) and, on 0.6.4, two `1 - frame slot` indexes.
- **`tools/draft_version.py`** makes a second pass over `stat_sites.rs`: fields rewritten from the new code (with a REVIEW line), sites alignment missed filled from the census when the counts agree (never the gates), and a census summary per kind. Measured: 0.7.0 to 0.6.4, 33 of 33 placed, 0 wrong, and the verifier passes the drafted table as it is (before: 5 unplaced and 4 with stale fields); 0.6.3 to 0.7.0, 29 of 33, 0 wrong, the 4 big-AI gates left. The runbook (`docs/version-migration.md`) describes the pass and adds the Stats family to the play list.
- Tested: formatting, lint with warnings denied, 528 tests, the verifier for 0.6.3, 0.6.4 and 0.7.0.

## 1.2.11 (development build) — 2026-10-16

**Nothing here has been seen in game. Development build, never for the Workshop.** The shape-A collision floors are hooked, approved by the author after a read-only research pass.

- **Why:** the "Collision margin" setting reached only the 10 shape-B hit tests. The 11 shape-A ones floor the margin at 5 units, so the player's shots that went through them always had 5 units of room, whatever the setting (default 0). The author's tests showed collision size is the biggest factor in whether the AI dodges, and mapping each skill to a shape would never end (modded champions).
- **What:** a new site kind, `FlooredCollision`, at each shape-A floor's `cmp r, 0x1389`, which comes right after `sub r, [target + 0x6a0]`. For the controlled athlete's marked shot (the register plus the target's Dodging is the marker 5000; Accuracy x 70 never is) the register becomes the chosen margin and the flags read "not below", so the game's floor keeps it. The bridge already saves and restores the flags. Every shot of the controlled athlete now gets the target's body plus the chosen margin, in both shapes (shape A uses the target's scaled body, read from the code). Other athletes' shots are unchanged.
- **Read from the code** (0.7.0; same instructions at the 0.6.3 and 0.6.4 sites): a search of the whole executable for the pair finds exactly 11 per version; the mark reaches all 11 (they are called from the projectile hit dispatcher with the producer's slot; one site's second call path traced only in part). Stat sites per version: 22 → 33. Counter slots 16 → 32.
- **Verifier:** each floor must be `cmp output, 0x1389` right after the Dodging `sub` on the target the table names, followed by the floor's `cmovl` (or `cmovge`); for every stat site, no branch in its function may land on a replaced instruction. Checked to refuse a wrong register and a wrong place.
- **Readout:** names the shape of the first hit test that checked a target against the shot (`shape A (0x...)`, `shape B (0x...)`), or `no hit test`. A hit test runs for each candidate target before the overlap test, so it means "checked", not "hit" (read from the code).
- **Docs:** the 0.6.4 investigation gets section 7a (the floors), the 1.2.9 and 1.2.10 results, a correction (shape A, not B, scales the body; the old text is kept, marked superseded) and open item 1 superseded; backlog item 15 likewise. Rejected: the guesses that kind 2 is one kind of projectile behaviour.
- Tested: formatting, lint with warnings denied, 528 tests (marked-shot test at the floor, the flags), the verifier for 0.6.3, 0.6.4 and 0.7.0.

## 1.2.10 (development build) — 2026-10-16

**Nothing here has been seen in game. Development build, never for the Workshop.** Practice tool fixes to 1.2.9, approved by the author.

- Seen in game with 1.2.9 (0.7.0, the author): Home+Insert works (`PRACTICE pulled 5 enemies` four times), but afterwards one enemy stayed on its spot, three ran back to base, and one was always at base; the author's teammates played normally. Why the enemies left is not known (guess: a route the game planned before the teleport, or a spot off walkable ground).
- Everyone but you holds still now, teammates included (before: only enemies; that was a design choice not asked about).
- The pulled enemies are pinned: the match tick puts each one back on its spot whenever it has moved. Knockbacks and pulls on them are undone at once. Turning the tool off releases them.
- The row is half as wide (12.5 units between enemies).
- Diagnostic: the `PRACTICE pulled` line lists enemies whose teleport the SDK refused, and about one second later `PRACTICE after 1 s` gives each enemy's spot and the furthest it moved in one tick before being put back.
- Hint text updated in English and the 15 translations.
- Tested: formatting, lint with warnings denied, 526 tests, the verifier for 0.6.3, 0.6.4 and 0.7.0.

## 1.2.9 (development build) — 2026-10-16

**Development build, never for the Workshop.** One fix to 1.2.8.

- Home+Insert (practice tool: line the enemies up at the cursor) did nothing: the mod reads only the keys that a binding uses plus a fixed list, and Insert was not on it (seen in game by the author on 0.7.0 with 1.2.8: no `PRACTICE pulled` line after several presses). The fixed list is now `ALWAYS_POLLED` in `platform_input.rs`, with a test that every key of the fixed chords is on it.
- Seen in game with 1.2.8 (0.7.0, the author): `PRACTICE ON` from controls.json at start; all five lineup champions were drafted, split between both teams in pick order, as built. The draft hook steers the player's own coach too. Kept as it is by the author (2026-10-16): on 0.7.0 and 0.6.3 the draft does not say which team is picking.
- Tested: formatting, lint with warnings denied, 525 tests, the verifier for 0.6.3, 0.6.4 and 0.7.0.

## 1.2.8 (development build) — 2026-10-16

**Nothing here has been seen in game. Development build, never for the Workshop.** The practice tool, stage 1 (backlog item 11), as agreed with the author; and better shot attribution in the stat rule trace.

- **Practice tool** (Advanced > Debug, Off by default; it changes the real match, so a tag is drawn). While it is on, in the match on screen:
  - Every enemy athlete holds still: after the mod's normal step for that player (which also paces every player's tick), its input becomes "move to where you stand", so it neither walks, attacks nor casts.
  - Every champion's health is refilled each tick (yours too), and the no-cooldown cheat is on for your champion (`test_cheats::active` is on while practising).
  - Home+Insert lines the enemy champions up in a row at the cursor, 25 units apart (`entity_set_pos`). Not Home+PageUp: the input hook passes PageUp through to the game.
  - `practice_lineup` in controls.json (comma-separated champion names, e.g. `"crossbowman, pyromancer, harpy"`) is drafted by the game's AI in that order and never banned. The draft hook cannot tell matches apart, so this applies to every AI draft while the tool is on, including the league's other fixtures when days advance; whether it also covers the player's own team's picks is not known (read from the code: the hook decides the AI's picks).
  - A readout under the tag lists your last 8 shots: the skill cast most recently before the shot and how long before, the projectile kind or action state, and the shape-B hit test reached, or "shape A or missed" after 0.7 s. It needs the stat rule hooks installed; without them it says so.
  - English-only on screen, like the no-cooldown tag. The setting's label and hint are translated in the 16 language files.
- **Trace attribution:** `STATRULE trace` lines now name the skill (Q, W or R) whose cooldown restarted most recently and how many ticks before, ignoring basic attacks (`cast=Q 4 ticks before`). Before, `last_used` was usually "attack", because basic attacks restart their cooldown all the time; one reading of the 1.2.7 trace took a skill shot for a basic attack because of it (rejected, see the investigation, section 7).
- Tested: formatting, lint with warnings denied, 524 tests (lineup parsing, merging one shot's events, attribution), the verifier for 0.6.3, 0.6.4 and 0.7.0.

## 1.2.7 (development build) — 2026-10-16

**Nothing here has been seen in game. Development build, never for the Workshop.** Review fixes to the athlete stat rules and a diagnostic, approved by the author.

- **The stat rule hooks are installed only while "Athlete stat rules" is On, and only at the title screen** (no match runs there, the same reason every other hook is installed there). Off at startup, the game's code is not touched and nothing runs for any match; switched On later, the rules take effect at the next title screen (`STATRULE waiting` names this once in game; `STATRULE installed ... at the title screen`). Switched Off after installing, the hooks stay in and do nothing, as before. Before, the 22 sites were patched at every start and every simulation thread passed through them, setting On or Off (the cost was never measured). The skill preview widens projectile corridors only when the rules are actually in effect.
  - Read from the code, not verified: that no simulation thread runs at the title screen after quitting a save to it. The first install of a session has always relied on this for the title screen at startup.
- **No log write inside a hook.** The hooks used to write their first-fire and Verbose lines through the log's lock and file from inside the game's code. They now queue them on their thread, and the controlled player's tick on that same thread writes them. The 1.2.4 note "No lock is taken anywhere in a hook" was wrong until this build (corrected 2026-10-16).
- **Removed** the one-time move of the 1.2.4/1.2.5 development defaults (On, 22, 5) to the 1.2.6 ones: only the author ran those builds, and the author's file is already moved. A test checks the defaults instead.
- **Checker:** `tools/verify_stat_sites.py` now also proves, at every reaction gate, that the id the table reads is the one the game looks the caster's Accuracy up with (before, it proved only the register, through a neighbouring field), and that each producer's traced word below is read from the caster's object.
- **Diagnostic** (`STATRULE trace`, Normal level, each combination once, while the rules are in effect): which hit tests a shot goes through, for docs/investigation-stat-rules-0.6.4.md section 3.
  - At each of the three Accuracy producers, the word the game switches on next: a projectile's kind (`+0x40`) at the projectile producer, an entity's action state (`+0x300`, niche-encoded) at the two entity producers; for your shots and, separately, anyone else's on your match's thread.
  - At each shape-B hit test a marked shot reaches, the producer and kind or state it came from. Your lines carry your champion, the slot whose cooldown last restarted (`last_used`: attack, Q, W, R) and the tick.
  - Shape A is not hooked: a projectile kind that reaches no shape-B test is shape A or missed.
- Tested: formatting, lint with warnings denied, 522 tests, the verifier for 0.6.3, 0.6.4 and 0.7.0 (and that the two new checks fail on a wrong offset).

## 1.2.6 — 2026-10-15

Release of the 1.2.4 to 1.2.6 development builds below, as tested by the author. Seen in game by the author on 0.6.4 (the live game), 0.6.3 and 0.7.0: the family installs, the rules fire, the settings page and the sliders. One DLL for the three versions, as since 1.2.0.

- **Athlete stat rules (experimental, Off by default).** Settings › Advanced › General › Athlete stats. In matches you play yourself, your athlete's stats stop deciding hits behind the scenes:
  - Your skillshots have a fixed hit margin instead of one that grows with your Skillshot Accuracy and shrinks with the enemy's Dodging. In the game's own rules an enemy with high Dodging becomes a much smaller target for everyone's shots (read from the code: up to 15 game units off a 10-unit body), which is why a low-Accuracy athlete could not land anything on a good dodger. With the rules on, every enemy is its real body plus the "Collision margin" setting (default 0).
  - How soon an enemy starts to react to your shot now follows your Accuracy against its Dodging, within a band from "Reaction floor" (default 6 ticks, the fastest, when the enemy's Dodging leads) to "Reaction ceiling" (default 14 ticks, the slowest, when your Accuracy leads). The same shot at the same enemy always gets the same answer. Read from the code: an AI athlete acts only at its own decision moments, paced by its Control Speed (4 to 12 ticks apart), so settings below about 6 ticks make no visible difference.
  - Everything the AI does on its own stays as the game made it: AI-versus-AI fights, and AI shots at you. Off restores the game's own rules entirely.
  - Design and reasons: `docs/backlog.md` item 15; the findings behind it: `docs/investigation-stat-rules-0.6.4.md`.
- **Defaults chosen in play by the author** (Crossbowman Q, 20 shots per row at a fixed spot, against an athlete with Dodging 81, seen in game on 0.6.4): with the game's rules, Accuracy 11 landed 2 and Accuracy 100 landed 6; with a 5-unit margin both landed 14 to 16 whatever the reaction band, so the margin went to 0 and the ceiling from 22 to 14. The rules ship Off because they change how the game plays.
- **"Show hit areas"** (Advanced › Debug, Off): rings around enemies near the cursor showing the body the hit tests use and the margin on top of it. The body is the game's own collision radius, not the sprite: a champion with a large sprite can have an ordinary ring, which is the game's data.
- **Skill previews** draw projectile corridors at the width that registers when the rules are on (the skill's radius plus the margin); at margin 0 the width is unchanged.
- **Settings window:** slider tracks are 24 px shorter and the value column wider, so values like "14 ticks" fit on one line in every language.
- **Known limits, read from the code and the author's logs:**
  - Seen in the author's logs on 0.6.4: the shot marker, the own-Dodging rule and the reaction gate all fired. Two of the three places that mark shots, and the two hit tests fed by them, never carried a marked shot (`seen` in the thousands, `marked` 0); whether they handle shots the player never fires or miss the player's is not settled. An earlier reading of the same logs, that the own-Dodging rule never fired, was a search mistake and is withdrawn (2026-10-15).
  - Eleven of the game's hit tests are not changed, because with the fixed marker they already yield the game's 5-unit margin; shots through them do not follow the Collision margin setting.
  - Whether the enemy AI's learned movement reads Accuracy or Dodging directly is not known.
- Tested for this release: formatting, lint with warnings denied, 521 tests, the verifier for 0.6.3, 0.6.4 and 0.7.0 (every hook site decoded against its executable), and the author's play on all three versions.

## 1.2.6 (development build) — 2026-10-15

**Nothing here has been seen in game. Development build, never for the Workshop.** Three changes on top of 1.2.5, as approved by the author:

- New defaults for the athlete stat rules: **Athlete stat rules Off** (it is experimental), Reaction floor 6 (unchanged), **Reaction ceiling 14** (was 22), **Collision margin 0** (was 5). A saved settings file holds every value written out, so a saved On, 22 or 5 is taken as the old default and moved to the new one once (`stat_defaults_version` 1, same approach as the acquisition defaults); any other saved value is the player's own and is kept. Because the author's file holds the old default, the rules must be switched On again after installing. Switched on afterwards, it stays on.
- Descriptions are short and plain, in English and all 16 language files: the feature hint is "Experimental: reworks how the player's stats affect gameplay."; the floor, ceiling, margin and overlay hints are one sentence each with no internal terms; the overlay's label is now "Show hit areas". The units "ticks" and "units" stay as the slider units.
  - What the rules do belongs here, not in the hints. Under direct control, with the rules on: the Accuracy producers give the player's shots a fixed hit margin (the marker 5000, turned into the Collision margin setting at the 10 hooked shape-B consumers; the 11 shape-A consumers stay at the game's own 5 units whatever the slider says), the player's own derived Dodging is 0, and the enemy's reaction delay to the player's shots comes from caster Accuracy minus defender Dodging in a band running from the floor (largest Dodging lead) to the ceiling (largest Accuracy lead), the middle scaling with them. Off is the game's own behaviour.
- Settings window: the slider's value text sat in a 70 px column and wrapped in Traditional Chinese ("14 ticks"). The slider track is now 270 px (was 294) and the value column 110 px wide, right-aligned to the same edge, so the longest value and unit have room; the unit words are unchanged. This narrows every slider in the window by 24 px. **The text width was not measured in the game's fonts (no way to do that here): whether it fits on one line in every language is unverified.**
- Not verified: everything in game, including the settings page.

## 1.2.5 (development build) — 2026-10-15

**Nothing here has been seen in game. Development build, never for the Workshop.** (An earlier 1.2.5 with producer diagnostics was reverted the same day and is not part of this build.) One change on top of 1.2.4, at the author's request ("margin slider first"):

- New testing setting Advanced › General › Athlete stats › **Collision margin**: a slider from 0 to 7 game units, step 1, default 5 (the 1.2.4 behaviour). It is the fixed hit margin on the controlled athlete's shots.
  - The Accuracy producers still store the fixed marker 5000, so the value stays unique against natural Accuracy values (multiples of 70; 0 and 7000 would not be unique). At the 10 hooked shape-B consumers, when the marker is seen, the output is the body plus `margin x 1000` internal units: the target's Dodging is added back as before, plus `margin x 1000 - 5000`.
  - **The 11 shape-A consumers are not hooked and stay at the game's own 5 units whatever the slider says** (with the marker, `max(5000 - avoidance, 5000)` is 5000 there). The slider therefore acts only on paths through the hooked shape-B sites; a shot that reaches only shape-A sites keeps 5 units at every setting. Which skills reach which shape is not known (read from the code: not settled).
  - The value is copied into an atomic by `configure()` every client frame, like the reaction floor and ceiling; hooks never read the settings store.
  - The skill preview's extra width and the overlay's yellow ring follow the slider. The preview still adds the margin to the skill's radius on each side, as in 1.2.4; at 0 it shows the old width.
  - `STATRULE collision` lines (first fire and the Verbose counts) now name the margin in units. The text is translated in the 16 language files.
- Unchanged: the hint text of "Athlete stat rules", the reaction band defaults (6 and 22), the preview gating, no new diagnostics.
- Not verified: everything in game. Tests and the verifier show only that the code builds, the margin arithmetic turns the marker into 0..7 units exactly (unit test), and the hook tables match the executables.

## 1.2.4 (development build) — 2026-10-15

**Nothing here has been seen in game. This is a development build for the author's tests and must never go to the Workshop.** Every statement below is marked as read from the code or as a guess. Design: backlog item 15; findings: `docs/investigation-stat-rules-0.6.4.md`.

- New hook family `stats` (athlete stat rules), in `native_adapter/windows/stats.rs` with a table per game version (`v063/`, `v064/`, `v070/` `stat_sites.rs`) and pure logic in `athlete_rules.rs`. 22 sites per version, all checked against their executable: bytes, whole instructions, no `call`/`jmp`/`rip`/`rsp` in the replayed bytes, and every register, frame slot and offset the table names (`tools/verify_stat_sites.py`, run by the profile verifier; a changed register is refused). The family is optional and all or none: if one byte check fails on the installed executable, no site is patched and every other hook still installs (`STATRULE unavailable: ...`).
- The rules apply only to the controlled athlete (read from the code):
  - Collision: the three Accuracy producers store 5000 (5 game units) for the controlled athlete's shots, and the 10 shape-B consumers (clamp at zero, `0x1045134`, `0x1045384`, `0x13d2441`, `0x13d2685`, `0x13d2b82`, `0x13d2fa1`, `0x1a5a882`, `0x1a5ae36`, `0x1a5afe6`, `0x1a5b680` on 0.6.4) skip the target's Dodging term when the shot's Accuracy value is exactly 5000. Natural values are multiples of 70, so 5000 marks these shots without a caster lookup at 10 of the 21 consumers. The 11 shape-A consumers are not hooked: `max(5000 - avoidance, 5000)` is 5000 already (decided with the project lead, 2026-10-15). The controlled athlete's own derived Dodging (the input-phase producer) is 0.
  - Reaction: at the four gates (`0xf27386/0xf27402`, `0xf169e2/0xf16a6b`, `0xf16d4f/0xf16ddc`, `0xe51451/0xe514cd` on 0.6.4) the capture site remembers caster Accuracy, defender Dodging and the game's seed when the caster is the controlled athlete, and the complete site replaces the game's gate by one from the gap (Accuracy - Dodging): bands 6..9 at -100, 10..14 at 0, 16..22 at +100, interpolated linearly; the tick inside the band is the game's own splitmix64 of the pair's seed, so the same situation gives the same gate. The Accuracy "extra" term is dropped. Two gates also mix the projectile's age into the seed; those bits are removed so the choice does not change every tick.
  - Everything else is untouched: AI-versus-AI fights, AI shots at the player (they keep the AI caster's Accuracy; against the player's body Dodging is 0), and every shot by anyone else.
- Who is "the controlled athlete": an atomic holds the worker thread and actor id, stamped by the controlled player's `think` every tick and cleared on release, Return to AI, leaving the match and when the setting is turned off (a 600 ms freshness check is the backstop). Every hook first compares the thread (one atomic load), then the actor id read from the instruction's own operands, and returns to the original path before it touches a lock. No lock is taken anywhere in a hook (the 1.2.1 attempt locked the session state on every collision call and was withdrawn). Settings are copied into atomics each client frame; hooks never read the settings store.
- Settings: Advanced › General › "Athlete stat rules" (on by default in this development build; Off is the game's own behaviour), "Reaction floor" (6) and "Reaction ceiling" (22) in ticks, the middle points scale with them. Advanced › Debug › "Show collision bodies near cursor" (off). All 9 new texts are translated in the 16 language files. The Advanced page now scrolls.
- Visuals: with the rules on, the skill preview draws projectile corridors (linear projectiles, native projectile skills, Gambler Q, shots, the Dancer fan) with the radius plus 5 units on each side. Dashes, beams and lines keep their old width. The testing overlay draws, around enemies within 260 px of the cursor, the body's collision radius (white) and radius plus 5 units (yellow) on the ground at the unit's position, using the SDK's `radius()` (the radius the hit tests use), not the measured art. Whether the projectile's own radius adds to the hit test is a guess; the preview adds it as the brief says.
- Logging: `STATRULE installed` and one line the first time each rule fires in a control session (Normal). Verbose: `STATRULE gate caster=... A= D= gap= gate= game_gate= seed=` once per projectile-and-defender pair, and `STATRULE collision site= seen= marked=` every five seconds (consumer runs on the controlled match's thread, and how many carried the marker). The defender's id is not logged: the gate sites hold no cheap defender id, so the pair's seed stands for it.
- Tried and not done: hooking all 21 consumers and recomputing the radius (the 1.2.1 way), because the shape-A ones need nothing and the lead removed them; a global "controlled match" flag with no caster check, because it also changes shots by others; scoping by actor id alone, because background matches reuse the ids.
- Not verified: everything in game. Open (from the investigation): which skill types pass through shape-B sites (the Verbose `seen`/`marked` counts will show), whether the producers' value is read anywhere besides the hit tests, and whether the learned dodge policy reads the stats. Tests, the bridge test and the verifier prove only that the code builds, the bridge preserves registers, flags and XMM, and that the tables match the executables.

## 1.2.3 — 2026-10-14

- Game 0.6.4 (the live branch, 2026-10-14) is supported, alongside 0.6.3 (players can roll back on Steam) and 0.7.0, from the same DLL: a new table folder `native_adapter/v064/` and profile `tools/native_profiles/0.6.4.json`, every hook family enabled. Details and evidence in `docs/migration-0.6.4.md`. Passed in game on 0.6.4 (2026-10-14): the author's full test list, one family after another, and a full match.
  - 0.6.4 is built from the 0.7.0 code with the 0.6.3 roster, so changed functions were matched against 0.7.0. About 50 addresses the draft could not carry were found by hand; every address in the tables was then checked by aligning its function with the old one, and every byte pattern equals the executable. The verifier passes for 0.6.3, 0.6.4 and 0.7.0.
  - The viewer lost its fog cache, so two viewer fields the hooks write moved down by 0x68 (frame-time accumulator, camera pan velocity). They are the first shared offsets that differ between versions and are now `VIEW_FRAME_CLOCK` and `VIEW_PAN_VELOCITY` in each version's `layout.rs`. Entity, player, config and UI fields are unchanged.
  - Changed by the game and reviewed: the attack and skill functions (a new action-table method; the slots the mod calls are unchanged), the shop's next-step function (rewritten around the same build vector), the vision guards (no cache to compare), the unit renderer (one caller now exists twice, so 16 outline sites), the shader parameter setters, and three status effects whose names in the preview reader are not settled.
  - All families were enabled at once, against the runbook's one at a time (a session-only build was the fallback, not needed).
- A basic attack order now has priority over walking instead of being preceded by a forced stop. The attack input is sent only on a tick when the game can act on it (target inside basic attack reach, attack cooldown from the official interface at 0 or 1 tick); the mod does nothing else to the champion, so whether he stops is the game's own attack deciding.
  - While an in-reach attack waits for its cooldown, a champion whose attack locks him in place stands and waits (as before, and as in League: the attack order interrupts the walk). A champion whose attack leaves him free to move (Gunner) keeps walking: to the clicked spot on attack-move (A-click, Shift + right-click), and along the walk he was already on after a right-click on an enemy in reach. If that enemy dies he goes on walking; if it is out of reach he approaches it and the earlier walk is dropped. Attack-move still never follows an enemy that leaves reach.
  - Which kind a champion is comes from his own attacks, read by the existing attack hook after the game has run it: action 3 means locked, a queued effect without a lock means free (`ATTACK KIND`, verbose). The latest attack decides; nothing is assumed before the first one, so until then he stands. No champion is named, and no hook or address is added.
  - Superseded: the 0.75.4 stop before an accepted in-range attack (`stop_for_manual_attack`, the stop ticket's attack target, `ATTACK WAIT_STOP`) is removed. It covered for sending the attack input on every tick: on cooldown the game ignores that input and leaves the previous movement running (the Ice Mage report), so the mod stopped the champion itself, which also stopped the Gunner, who never has to. The attack hook still takes the ticket to cancel a recall.
  - A target outside basic attack reach (a right-clicked enemy, or one acquired by the wider attack-move radius) is approached as before by the attack input when the attack is ready, and by a plain move toward it while the attack is on cooldown. Before, that case also sent the attack input the game ignores on cooldown (read from the 0.75.4 note: the consumer returns early on a nonzero cooldown), so the champion kept his previous movement until the cooldown ended; not seen in game either way. An unreadable cooldown or reach never withholds an attack.
  - While a locked attack is in progress (action 3) the waiting input is not passed to the game's move code (`attack_wait` on the stop ticket), so wind-up and backswing see exactly what they saw before: nothing.
  - `MANUAL input` (verbose) shows `attack_cooldown`.
- A → left-click and Shift + right-click are now different orders (the author's rule, as in League). Shift + right-click always attacks the enemy nearest the champion within his acquisition radius while walking to the click. A → left-click follows "Attack-move target preference": with "Near champion" it behaves the same; with "Near cursor" the enemy nearest the click, within the acquisition radius around the click, is walked up to and attacked even beyond the champion's reach, re-chosen every tick around the click; with no enemy near the click it walks on, attacking what comes into reach, nearest the click first. Before, both inputs only ever considered enemies within the champion's radius, so "Near cursor" picked among the enemies around the champion (reported by the author). Champion-only (Honor) applies to all. The preference's hint says it is for A → left-click only, in all 16 languages.
- Attack-move acquisition defaults (author's play test of this build): the shared Automatic minimum is 80 (was 120). A settings file still holding 120 is moved to 80 once (`defaults_version` 4), because every file has the old default written out; a different value is kept. The Gunner's Automatic radius is exactly his basic attack reach (50), with no minimum and no buffer: an enemy acquired beyond his reach makes him leave his walk to approach it. Custom radii are unchanged. `AT_REACH` in `acquisition.rs` lists such champions by name (one today); the settings panel shows their buffer and minimum as 0.
- Season statistics follow the played match (`season_fix.rs`; findings in `docs/investigation-played-result.md`). The game adds every athlete's numbers from its AI-only server pre-simulation to the athlete record a few seconds into each game (seen in game). After each game the mod replaces that contribution with the stored replay's numbers, for all ten athletes: games, wins, kills, deaths, assists, MVP, rating, level, solo kills and deaths, damage dealt and taken, healing, gold, and the per-champion breakdown.
  - Corrected once per series, because the game stores the replays of a series only when the series ends (seen in game: a first version corrected per game and lost game 1 of series 620, whose replay did not exist yet when game 2 began; superseded the same day). The records are copied when game 1 starts, before its pre-simulation lands. When the replay of every game exists, the pre-simulation's contribution is the difference, accepted only if in each position it is exactly the replays' games of the replays' champions; it is replaced by the summed replays. Replays are matched by the games' seeds. A late copy, a missing replay (a game left before it ended, or a new series started first), or any other shape is skipped and logged; AI-played games change nothing.
  - When the series ends the league's per-athlete statistics (season record screen, player ranking) move. If they moved by the pre-simulated games they are corrected the same way; if by the played games (copied from the corrected athlete records) nothing is written. Which of the two the game does is not known yet. Team standings already follow the played games and are not touched.
  - Records can only be written by a server extension: the client sends each correction (`lt_direct_control.season_fix`) and the server applies it only if the value is still the one the client read (compare-and-set), so a late or repeated command never overwrites a newer game. Each athlete gets a `SEASON` log line per game (sent, applied, unchanged or skipped with the reason); `SEASON server` lines report each write and any refusal by the game's schema check. At Verbose detail the server also saves the server-only match record (`research/server-match-<id>.json`).
  - Repair of the past, once per save: the stored replays hold the played games, and for every athlete of the author's league who never met his team the league statistics equal the sum of their replays exactly (14 of 14), while all 41 who did differ (read from the exporter's replays and the league record, 2026-10-14). In the management main screen, with no series waiting for its correction, the mod reads this year's non-practice matches, their replays and the athletes' records a few dozen per frame, checks the rule on every athlete who never shared a game with the player's team (their records must equal their replays exactly, whole documents, or nothing is written), then rebuilds each involved athlete's current-year entry per position and his statistics in each unfinalized league competition from the replays (compare-and-set as above). The server records `season_repair_v1` in the mod's save data when done. Earlier years and finalized competitions are not touched.
  - Not covered: champion proficiency, loyalty, condition and stamina also change when the pre-simulation lands; whether they depend on performance is not known yet. Games played before this build stay as they are.
  - Writes to the career save. No setting to turn it off.
- Diagnostic for `docs/investigation-played-result.md` (the athlete's season totals do not follow the played match), at Verbose log detail only and read-only. Each simulation of a fixture (server pre-simulation and the match on screen) keeps every player's latest K/D/A, CS, level and gold, sampled every 30 ticks by whichever player's `think` comes first. Around a played match the own athletes' records, the team record and the league competition records are saved to `research/session-records-<match>-set<n>-seed<seed>-<start|during|exit|after>.json` next to the log (`RESULT RECORDS`): at the first frame with a roster, one minute in (athletes only), on leaving the battlefield and 30 seconds later. Bounded to 32 records and 12 MB per kind.
- Two Workshop reports of the shop window staying on screen, frozen (one with the match running behind it, one after Esc), not reproduced. The game's mod interface catches a panic in a mod callback and carries on silently, and a panic while the client's interface state was locked left every window of the mod frozen for the session. What panics, if anything, is not known.
  - A panic is now logged with its file, line and message (`PANIC`, at every log level, the first 20).
  - A panic inside the shop window's frame closes the shop, hands back a pause the shop made and frees the input it was blocking (`SHOP UI closed after a panic`); the interface state and the shop's own locks are usable again afterwards (`CLIENT interface state recovered`). Both lines are logged once a session.
  - Checked and rejected as the cause on the author's setup: Esc with "Pause while the shop is open" on, off and at match start resumes normally.

## 1.2.2 — 2026-10-13

- Start modes (Settings › Advanced › General, section "Match start"): **Ask every match** (as before), **Always play my role**, **Start spectating**. Preparation remembers the role chosen there as **My role**; a mid-match swap does not change it.
  - Always play my role starts by itself with that role selected. With Manual shopping on, the shop still opens at the start as before. If the preferred role is not available, an available teammate is selected and the match waits for confirmation instead of starting by itself.
  - Start spectating skips preparation and the start-of-match shop; F11 takes control later.
  - Preparation has a **Watch this match** button, which does the same for one match.
  - The start-of-match shop opening is now a claim owned by the session, discarded by a spectator start, Watch and an AI hand-back, so a late build plan cannot open the shop at the first recall or death after taking over. P is unaffected.
- This is the start-mode half of an unreleased 1.2.1 that also changed how athlete stats act in manually played matches. That half was withdrawn after review (it is kept on a local branch; the research is in `docs/investigation-athlete-stats-manual-control.md`). No native address, hook or game-version table changes in this build.

## 1.2.0 — 2026-10-11

- One DLL for game 0.6.3 and game 0.7.0. At the title screen the mod hashes the executable once and installs the hooks built for that game version; any other executable gets no hooks, as before. Release candidate: to be tested in game on both versions.
  - `native_adapter/v063/` holds the 0.6.3 tables, taken from the released 1.0.x code (`layout.rs`, `native_profile.rs`, `native_preview.rs`, `native_items.rs` on main). Every address was compared with that code: none changed. The values main wrote inline are now named (`ENTITY_GETTER`, `OUTLINE_FLOAT_PARAM`, `OUTLINE_COLOR_PARAM`, `SHOP_SLOT_GATE`, the item bridge table). All hook families are enabled for 0.6.3.
  - The hook code (`native_adapter/windows/`) is built twice, once per table folder; entity field offsets in it are the same in both games (checked against main). For 0.6.3 players this is the 0.7.0 branch's code path (hooks installed per family) with the released addresses.
  - 0.6.3 gains what the 0.7.0 development builds added: Astrologer shapes, and five effects carried back with `tools/match_vtables.py` (0.7.0 to 0.6.3): Swordman R 0x18c09e0, Gambler Q 0x18c98a0, Vampire Q 0x14edc50 (similarity 1.00), a timed status 0x1a1e470 (0.98), and the rush to a clicked unit of Lancer W and Tale W 0x1ad97b0 (0.54, the only candidate; no field is read). The second timed status has two equal candidates on 0.6.3 and is left out.
  - `mod.mod_info` requires base `>=0.6.3` again.
  - Hashing moved to `native_adapter/host.rs` (it names no game address). The tooltip bridge test fixture is a naked function instead of a global symbol, because the hook tests now build once per game version (455 tests).
  - `tools/verify_build.py` verifies every profile in `tools/native_profiles` against its own executable (the installed game, and a kept copy under `research/game-builds/` for the other version) and checks the DLL carries each fingerprint.

## 1.1.22 — 2026-10-11

- Development build for game 0.7.0. First step toward one DLL for game 0.6.3 and 0.7.0; no behaviour change intended, and this build still supports 0.7.0 only.
  - Everything that differs between game versions now sits in one folder per version, `native_adapter/v070/`: `layout.rs` (moved from `windows/`), `identity.rs` (fingerprint, layout guards and enabled families, from `native_profile.rs`), `effects.rs` (the preview reader's apply addresses, from `native_preview.rs`) and `items.rs` (the item reader's anchors and bridge identity, from `native_items.rs`).
  - The hook code in `native_adapter/windows/` (now with the preview reader, `preview.rs`) names no address and no version. `native_adapter/mod.rs` builds it once per version block against that block's tables, hashes the executable once, installs the copy whose fingerprint matches and routes its public functions to that copy (`hooks!`); an unrecognised executable still gets no hooks.
  - Inline values moved into the tables: the purchase gate (`SHOP_SLOT_GATE`), the item bridge table and its two methods, the read-only data start used by the effect reader and its tests. The minimap plan takes the version's profile as an argument.
  - Every hexadecimal value in the version-specific sources was compared with 1.1.21: none added; the only ones gone are the five repeats replaced by references to the tables.
  - `tools/verify_native_profile.py` checks a profile against its own version folder and refuses an address written in the shared hook code.

## 1.1.21 — 2026-10-11

- Development build for game 0.7.0. Swordman R: the strip is word 6 (40) wide and word 7 (150) long. **Superseded 2026-10-22 (1.5.2-dev1):** read from the game's code (0.6.4, 0.6.5), word 6 is the hit's distance on each side, so the strip is 80 wide. 1.1.20 drew word 6 as the length at one champion's width, which was far too short and thin (user screenshots; the proportions now match the area another mod shows for the skill).

## 1.1.20 — 2026-10-11

- Development build for game 0.7.0. The skills the coverage report listed as not understood (one match by the user with Astrologer, Construct, Lancer, Soldier and Tale; shapes from the data and the user's descriptions):
  - Migration correction: the single-target projectile. 0.7.0 rewrote the 72-byte form the champions use (apply 0x17c6440, in five vtables; also 0x1804b60), while the 0.6.3 code survives only in two 80-byte forms (0x17d70f0, 0x17cf560). 1.1.8 matched by code and listed only 0x17d70f0, so Soldier Q and W and Construct R showed the fallback on 0.7.0. All four are now "no area".
  - Astrologer, from her definition file (both game versions): Q and R a circle of `radius` at the aim, W a strip `line_width` wide from her over the cast range.
  - Lancer W and Tale W (apply 0x16157c0, { effects, speed }): a rush to the clicked unit, drawn as movement to the target.
  - Swordman R (0x1a88ef0): a strip a champion wide over word 6 (40), passing through; the distance is to be checked in game.
  - Gambler Q (0x189ab00): a strip of hit radius word 7 (3) over the range word 5 (80) toward the hovered champion.
  - No area: Vampire Q (0x157d6a0) and the timed statuses 0x164e960 and 0x1a89990 (Tale R and others).

## 1.1.19 — 2026-10-11

- Development build for game 0.7.0. The mid-match swap (1.1.18) passed in game. Its column, after the user's test: portraits only (the name plates and key numbers are gone), in lane order from the top (top, jungle, mid, bottom, support; it was reversed), and the floating swap button has a light two-pixel frame so it reads against the battlefield; the portrait tiles have a visible grey frame.

## 1.1.18 — 2026-10-11

- Development build for game 0.7.0. Swap the controlled champion to a teammate during the match: while playing, paused, or with the AI in control (after F12), by Ctrl + 1-5 or from a new swap button floating above the pause button. The button unfolds a column of the five teammates' portraits upward, then a name plate with the key number beside each; a pick, Esc, a press elsewhere or the button again folds it away (the user's layout; design/hud/swap-column-v2.svg).
- How: the controlled player was already decided every tick from the selection (`hud_identity`), and the pre-match chooser re-bound it under the session gate; the same re-binding is now allowed in the Running, Paused and Ai phases. The previous champion is the game AI's again from its next decision, as after Return to AI (which does no per-champion cleanup either). On a mid-match swap the mod's pending commands, the shop queue and the stats panel history are cleared, and the attack-range used by the reach ring is reset; the no-cooldown test buff already follows the controlled entity.
- Each own player's HUD snapshot is refreshed once a second (it was taken at tick 1 only), so a swap while paused shows current level, gold, items and cooldowns; a dead teammate keeps its last snapshot until it respawns. Skill previews for the new champion need one simulation step, so they appear after resuming.
- No swap while the shop or settings window is open (the shop shows, and would queue against, the previous champion's inventory).
- Read for per-champion state and found to follow the selection each tick: simulation input, shop mode, HUD sampling, inventory, emotes, purchase tracker, test cheat. Not read: every native hook's per-actor statics.
- New strip icon `ef_swap` (lucide arrow-left-right, as the other strip icons).

## 1.1.17 — 2026-10-11

- Development build for game 0.7.0. Preview coverage report: for every champion of a watched match the mod logs what its reader made of each skill (`PREVIEW COVERAGE actor=.. Q=.. W=.. R=..`: the shapes, `no-area`, or `unknown:` and the first reason; its own budget of 600 lines a session) and what the champion's definition file alone gives (`PREVIEW DECLARED`). New tools/audit_previews.py joins them with the roster as the mod does (live shape, else declared shape, else no area, else unknown) and prints a table of champions and the list of skills still not understood. It shows whether a shape exists, not whether it is right.

## 1.1.16 — 2026-10-11

- Development build for game 0.7.0. Native ability descriptions (1.1.15) passed in game for Crossbowman (three `source=game` results, nothing unresolved).
- Skill previews: a declared `Native` effect that names one projectile's hit radius is drawn as a strip of that radius over its `range` (the cast range when it has none): Crossbowman Q, W, R and Harpooner Q, W. Other `Native` effects, and ones with several projectiles (Harpooner R), stay not understood. These skills never had a real preview: until 1.1.13 the fallback for a skill that is not understood was an arrow, which looked like a line preview; the crosshair that replaced it showed the gap (user report on Crossbowman).

## 1.1.15 — 2026-10-11

- Development build for game 0.7.0: native ability descriptions (the tooltips family) are back, so every hook family is now enabled on 0.7.0.
- The 407 tooltip anchors were carried from the 0.6.3 profile with the new tools/migrate_tooltips.py: code bytes by aligning the enclosing functions, the 60 built-in ChampionInfo tables through the `lea rdx, [table]` each aligned return of the built-in lookup now loads (1808 of its 1809 instructions align; all 60 keep their size), the mod and the four data tables as the vtables whose Q/W/R functions align with the old ones (0.90 against 0.19 for the next candidate). The sheet getter, the info lookup, the Arc drop, the three dispatch sites and the consumers keep their instructions; only frame and field offsets inside the game's own functions moved. The mod table grew from 0xb18 to 0xb38 bytes and the data tables from 0x9d0 to 0x9f0; the reader takes sizes from the tables.
- Nine built-in champions' description functions align only 0.52 to 0.65 with their 0.6.3 forms: the game changed them. Their tables are identified by position in the lookup, not by those functions.
- The built-in lookup returns no table that 0.6.3 did not have, so the four 0.7.0 champions get their descriptions through another implementation (the mod or data tables, to be confirmed in game). An implementation the reader does not know falls back to the old resolver.

## 1.1.14 — 2026-10-11

- Development build for game 0.7.0. Skill previews: an area that affects allies (a heal, a shield) is drawn in the preview style's ally colour; areas for enemies, both or unknown stay yellow, and a skill that is not ready stays grey. A skill with an ally area and an enemy area (user example: a shield for allies within 60 and a pull on enemies) now shows which is which. The game's RangeEffect, RangeProjectile, RangePeriodProjectile and ParabolicProjectile carry their target as the last word (CastingTarget codes: 0-4 allies, 5-9 enemies, 10-12 both, 13 none, as in mod-api-stable and as seen in PREVIEW TREE logs); declared (data and Workshop champion) effects name it in `target`. Hand-written effects, lines and dashes are not coloured. Tested for the code mapping and the colours drawn; not checked in game.

## 1.1.13 — 2026-10-11

- Development build for game 0.7.0. Skill previews restyled (the user's choices from mock-ups; design/previews/dash-arrows-v2.svg): the spearhead arrow is gone.
  - Line skills: the strip at its true width is the main element, with a visible fill (0.2, was 0.045), yellow edges (were faint white) and small chevrons along it for the direction. No arrow is laid over it.
  - Strip ends: both edges trail on past the end when the shot is known to pass through units, a solid bar when it is known to stop at the first one (`Shape::Shot`); a thin closing edge otherwise. Known so far: Hypnotist Q passes through, Ferryman Q stops. Strips with absolute endpoints get no chevrons.
  - Dashes and movement: up to three chevrons closing on the landing point, the nearest the largest and brightest, with a faint dotted lead-in when there is room; never drawn behind the start (tested).
  - A skill whose area is not understood: a small white crosshair where it is aimed (with the range ring), no arrow and no strip, so it cannot be mistaken for a line skill.
  - Circles are unchanged.
  - preview_style.json: the `arrow_*`, `skillshot_*`, `dash_head` and `dash_wing` keys are gone (overrides of them are ignored); `corridor_*`, `chevron_*`, `guide_*`, `dash_start` and `hover_range_*` are new.
- Hovering a skill slot on the HUD: its range ring is drawn in the skill's colour (was only the faint grey aiming ring, easy to miss). Shapes centred on the champion are shown as before; shapes that need an aim are not.

## 1.1.12 — 2026-10-11

- Shop: several copies of one item queued together are each bought. Each order excludes the copies owned when it was queued; queued together they excluded none, so the first finished copy also satisfied the next order, and one order left the queue at every later purchase (user log: four queued, one bought, the queue emptied over the next three purchases). The slot that finishes an order is now excluded from the later orders for the same item (tested). Present since 1.0.3.
- Skill previews for the four 0.7.0 champions, whose twelve skills are one effect (apply 0x160da70, 160 bytes, skill number in the byte at +0x98): Ferryman Q and Hypnotist Q as a strip of the shot's hit radius (word 1) over the range (word 0); Gladiator W and Ferryman W as a strip of half width word 6 over the range; Bombardier W/R and Hypnotist W/R as a circle of radius word 4 at the aim; Gladiator R and Ferryman R have no area; Gladiator Q is not read and keeps the generic guide. The field meanings come from the values and the player's description of each skill; only Ferryman W's use of the range was read in the code. The strips' width may be half or full: to be checked in game.
- Bombardier Q: a marker at the aim and a ring around him at his basic attack range plus word 12 (50), taken to be the range bonus of his cannon mode; to be checked in game.
- Log: `MANUAL rebound` names the champion chosen before the match starts (the earlier `MANUAL bound` line names the default lane).

## 1.1.11 — 2026-10-11

- Development build for game 0.7.0. "HUD tooltips" (Settings, Interface, Appearance): On hover (default, as before), Hold key and hover, or Off, for the skill and item tooltips of the HUD; the shop window's tooltips are not affected. The key is the new binding "HUD tooltip (hold)", Alt by default. In every mode the tooltip is hidden while a mouse button is held, so a click or drag passing over the HUD does not open it (requested on the Workshop). Translated in all 16 languages.
- New champions (read-only finding): Gladiator, Ferryman, Bombardier and Hypnotist build all twelve skills from one 0.7.0 effect (apply 0x160da70, 160 bytes) whose byte at +0x98 selects the skill: 0-2 Gladiator, 4/6/7 Ferryman, 8-10 Bombardier, 12/14/15 Hypnotist. Not read yet.

## 1.1.10 — 2026-10-11

- Development build for game 0.7.0: "Keep pointer in window" carried over from 1.0.7 (released for 0.6.3), unchanged. The frame-rate fix of 1.1.9, the shop filter and the emote volume slider were released for 0.6.3 as 1.0.6.

## 1.1.9 — 2026-10-11

- Development build for game 0.7.0. Match speed no longer follows the frame rate. The worker was held to one tick ahead of playback and a client frame can only play what is queued, so below 60 fps the match slowed in proportion (30 fps played at half speed; reported on the Workshop). The lead now follows the average client frame time: 1 tick at 60 fps and above (unchanged), up to 4 below, which keeps full speed down to 15 fps.
- The lead rises once the match would run more than 5% slow and falls back only when the smaller lead is nearly enough, so it does not flip at a boundary; one long frame does not raise it (tested). The per-frame playback limit and the "unexpected frame counts" guard follow the same maximum. Frames are timed on the wall clock between running viewer calls, not from the game's own frame time; a gap over 250 ms is a pause and is not counted. `TIMING frame lead a -> b` is logged for the first eight changes of a match. The playback rate is measured over ten-second windows: `PACING playback` at verbose detail, and `TIMING slow playback` (at every detail level, eight a match) when a window ran under 95% of full speed, which also shows a slow simulation thread that this change does not address.
- Skill previews (1.1.8): Ice Mage R and Sand Mage Q and R, the effects the game rewrote, passed in game. Hypnotist's three skills use a new 0.7.0 effect (apply 0x160da70, 160 bytes) the reader does not know yet.

## 1.1.8 — 2026-10-10

- Development build for game 0.7.0: skill previews are back (the readers family). Native ability tooltips, split off as their own family, are the only part still off.
- The preview reader recognises an effect by the address of its `apply` method and its size. New tools/match_vtables.py pairs the two builds' effect vtables by size and by how the method at that slot aligns: 36 of the 40 known applies equal their 0.6.3 forms and keep their size, and the vtable shape (size, align, apply at +0x20) is unchanged.
- Rewritten by the game in 0.7.0, same size, layout not re-proven: ranged-period (0x1bde9d0), ranged-projectile (0x1aa08c0) and Ice Mage's ultimate (0x1709a30). Their previews use the 0.6.3 payload layout and need checking in game. TargetProjectile grew by one word (80 bytes); it has no area and is only named. One Attack vtable carries a second equal copy of the method; both addresses are listed.
- Not yet included: the per-effect range term the 0.7.0 skill functions add (effect method +0x120), and preview shapes for the four new champions beyond what their effect trees give.

## 1.1.7 — 2026-10-10

- Development build for game 0.7.0. An "Emote volume" slider (0 to 100% in steps of 5, 70% by default) replaces the emote sound switch; 0% is silent. The game accepts a playback volume from 0 to 1, so the setting can only turn a sound down: 100% is the level of the sound file, and the game's SFX volume applies on top. Translated in all 16 languages.
- The built-in sound is generated at a higher level (peak 20% of full scale, was 14%) and was played at a fixed 0.375: measured against the WAV effects in the game bundle, its average level at 70% is now a little above the game's median effect (it was under half of it). A steady tone is far denser than its peak suggests, so it is not raised to the game's peak level.
- Settings saved with the old switch off stay silent; with it on they get the default 70% (tested).
- Groundwork for imported emote sounds (WAV, levelled at import), which would use the same slider.

## 1.1.6 — 2026-10-10

- Development build for game 0.7.0: the outline family joins (unit outlines, exact unit hit boxes from drawn sprites, the death greyscale). Only the readers family (native tooltips and skill previews) is still off.
- The render command the outline code moves and restyles is unchanged: its drop glue is equal instruction for instruction (179/179), with the same 0xd0-byte stride and variant tags in the unit renderer. The unit body renderer (0x2399810) was recompiled; all 15 of its call sites are located (there are still exactly 15) and seven of the calling functions are equal, so its arguments are unchanged.
- The shader setter (0x1a8130) and the two parameter setters (f32 0x1cfc5d0, colour 0x1cfbaf0) were recompiled; the game calls each with the same arguments as before. The two parameter setters no longer share a prologue, so each has its own guard, and their addresses are named constants in layout.rs instead of literals in outline.rs.

## 1.1.5 — 2026-10-10

- Development build for game 0.7.0. Confirmed in game with 1.1.4: the registry held 237 items of which the game reported 30 enabled, and the grid showed only those. The filter counts still counted every registered item ("All 237"); they now count enabled items only, so a filter no enabled item matches is not listed.

## 1.1.4 — 2026-10-10

- Development build for game 0.7.0. The shop listed every item in the game's registry, including the items of an installed mod that is switched off (the Riot item mod: 237 registered items, scrambled icons and raw names, while the match used base items and 4 slots). The reader already took each item's `enabled` flag from the game; the shop now uses it: an item that is not enabled keeps its registry index (the game's build lists refer to it) but is not shown in the grid or the stat filters, and no recipe leads into or out of it (tested).
- `PLAYER HUD registered items` logs `enabled=` beside `count=`. Whether the game reports those items as not enabled is to be confirmed by that line; if it does not, this build changes nothing visible.
- Not specific to 0.7.0: 1.0.5 has the same shop code.

## 1.1.3 — 2026-10-10

- Development build for game 0.7.0: the shop family joins, with the item catalogue reader (moved from the readers family to the shop family, which is its only user). Outlines, native tooltips and skill previews are still off.
- Purchase hooks: the two decision thunks (0xff9040 upgrade, 0xffa590 new item), their controller slots (0x3e5ddd0, 0x3e5ddc8) and decision bodies are located; the executor (0x1745420, recompiled) calls both slots with the same arguments and stack layout, and its slot gate and both gold payments are the same instructions (one now through another register).
- Gold and build readers: the gold getter (still through slot +0x158, gold at +0x868) and the build-vector reads are in functions equal to their 0.6.3 forms; owned-item fields (+0x310, +0x318) are used the same way by the executor.
- Item catalogue: the AI bridge table (0x407dbf8) and its two identity functions, the registered-item vector path, and the item methods the reader calls (+0x50 enabled, +0x68 price, +0x70 tier, +0x80 next tiers, called in the same order by the game's goal search; +0x78 stats) are unchanged; the stat block fields are used at the same offsets by the game's item tooltip.
- New tools/relocate.py (old address to new address through aligned functions).

## 1.1.2 — 2026-10-10

- Development build for game 0.7.0: the movement and combat families join session, camera and minimap. Outlines, the purchase hooks, the item catalogue, native tooltips and skill previews are still off.
- The entity layout did not move: across the nine functions these hooks live in, every entity field the hooks read or write (action, position, cooldowns, level, range, skill descriptors, charges, id, inventory queue) is used at the same offset in 0.7.0 (new tools/field_shifts.py: 601 field uses unchanged in the entity update alone).
- Movement: move, steer, direct step, stop and recall-cancel functions equal their 0.6.3 forms; call sites aligned with identical argument setup.
- Combat: the attack and aim-correction functions are equal; the dispatcher that calls move, attack and the three skills is equal. The three skill functions were recompiled (16 more bytes of stack) and now add a per-effect range term through a new effect method (+0x120); cooldown, action state and the queued-intent layout are unchanged. The mod's own skill range does not include that new term yet.
- Three guards changed shape with the recompile (attack range, its base, skill level range) and were re-read; the entity getter the attack-range reader checks moved to 0x315ba70.

## 1.1.1 — 2026-10-10

- Development build for game 0.7.0: the camera and minimap families join the session family. Movement, attacks, skills, outlines, the purchase hooks and native tooltips are still off.
- Camera: all 17 layout and vision guards relocated by aligning their six enclosing functions; the `ingame` UI object check, the config fields (+0x18/+0x1c/+0x20 camera mode and target, +0x45/+0x46 layout, +0x4b vision) and the view fields (viewport, centre, extent, zoom, pan velocity) are used by the same instructions as in 0.6.3. The camera-follow evidence in the old notes was from 0.6.2 addresses; it is re-established for 0.7.0 (follow routine 0x2439fd0, own-mid shortcut 0xa3bd8e).
- Minimap: all 54 operands relocated (37 in the renderer, 17 in the input handler); the native constants they replace are unchanged. The camera needs the enlarged minimap's geometry, so the two families are enabled together.
- Stall report: a worker thread that has exited is logged as `WORKER ENDED` (normal when a match finishes) instead of `WORKER STALL`; a stack that cannot be read says why.
- New tools/map_functions.py (follows aligned calls from known function pairs) and tools/find_context.py (finds an instruction by its neighbours).

## 1.1.0 — 2026-10-10

- Development build for game 0.7.0 (beta branch), on the `game-0.7` branch. Not a release: only the session hooks are installed. Movement, attacks, skills, camera control, outlines, the minimap scaling, the purchase hooks and native tooltips are off until each is reviewed for 0.7.0 (docs/migration-0.7.md).
- Native hooks install per family (session, camera, movement, combat, outline, minimap, shop, readers): each is verified and patched on its own, and code that needs a family that is not installed switches off. `native_profile::ENABLED` lists the families reviewed for the executable.
- Identity and the session family moved to 0.7.0: worker send 0x979089 -> 0x8bf9d0, viewer update 0x877560 -> 0xb194c0, input 0x89fd1d -> 0xa3ac10, pause guard 0xb1960b. The worker and viewer functions equal their 0.6.3 forms instruction for instruction.
- tools/native_profiles/0.7.0.json reviews the session family only; verify_native_profile.py and verify_build.py compare only the families a profile lists. New tools/compare_functions.py and tools/show_code.py.

The 1.0.6 to 1.0.9 entries below were built for game 0.6.3 from the 1.1.x work above (1.0.6 to 1.0.8 released; 1.0.9 built but not released, superseded by 1.2.0).

## 1.0.9 — 2026-10-11

Developed and passed in game on the game-0.7 branch (game 0.7.0, builds 1.1.16 to 1.1.19); this build is the same code for game 0.6.3.

- Swap the controlled champion to a teammate during the match: while playing, paused, or with the AI in control (after F12), by Ctrl + 1-5 or from a swap button floating above the pause button, which unfolds a column of the five teammates' portraits (top lane at the top). A pick, Esc, a press elsewhere or the button again folds it away. The controlled player was already decided every tick from the selection, and the pre-match chooser re-bound it under the session gate; the same re-binding is now allowed in the Running, Paused and Ai phases. The previous champion is the game AI's again from its next decision. On a swap the pending commands, the shop queue and the stats panel history are cleared. No swap while the shop or settings window is open.
- Each own player's HUD snapshot is refreshed once a second (it was taken at tick 1 only), so a swap while paused shows current level, gold, items and cooldowns. Skill previews for the new champion appear after resuming.
- Skill previews: a declared `Native` effect that names one projectile's hit radius is drawn as a strip of that radius over its `range` (the cast range when it has none): Crossbowman Q, W, R and Harpooner Q, W. These skills never had a real preview; the arrow that used to be the fallback only looked like one.
- Preview coverage report: `PREVIEW COVERAGE` and `PREVIEW DECLARED` log lines for every champion of a watched match, and tools/audit_previews.py, which lists every champion seen and the skills still not understood.
- New strip icon `ef_swap`.

## 1.0.8 — 2026-10-11

All of this was developed and passed in game on the game-0.7 branch (game 0.7.0, builds 1.1.11 to 1.1.14); this build is the same code for game 0.6.3.

- Shop: several copies of one item queued together are each bought. Each order excludes the copies owned when it was queued; queued together they excluded none, so the first finished copy also satisfied the next order and one order left the queue at every later purchase. The slot that finishes an order is now excluded from the later orders for the same item (tested). Present since 1.0.3.
- "HUD tooltips" (Settings, Interface, Appearance): On hover (default, as before), Hold key and hover, or Off, for the HUD's skill and item tooltips; the shop window's tooltips are not affected. The key is the new binding "HUD tooltip (hold)", Alt by default. In every mode the tooltip is hidden while a mouse button is held. Translated in all 16 languages.
- Skill previews restyled; the spearhead arrow is gone.
  - Line skills: the strip at its true width with a visible fill, yellow edges and small chevrons along it.
  - Strip ends: trailing edges for a shot known to pass through units, a solid bar for one known to stop at the first, a thin closing edge otherwise (none is known yet for the champions of game 0.6.3).
  - Dashes and movement: up to three chevrons closing on the landing point, with a faint dotted lead-in; never drawn behind the start.
  - A skill whose area is not understood: the range ring and a small white crosshair at the aim.
  - Circles, cones, walls and blinks are unchanged.
  - preview_style.json: the `arrow_*`, `skillshot_*`, `dash_head` and `dash_wing` keys are gone (overrides of them are ignored); `corridor_*`, `chevron_*`, `guide_*`, `dash_start` and `hover_range_*` are new.
- An area that affects allies (a heal, a shield) is drawn in the ally colour; enemy, both and unknown stay yellow. Read from the last word of the game's RangeEffect, RangeProjectile, RangePeriodProjectile and ParabolicProjectile and from `target` in declared effects.
- Hovering a skill slot on the HUD draws its range ring in the skill's colour (was only the faint grey aiming ring).
- Log: `MANUAL rebound` names the champion chosen before the match starts.
- The reader for the four champions added in game 0.7.0 is present but never matches in this build.

## 1.0.7 — 2026-10-11

- "Keep pointer in window" (Settings, Camera, Navigation; on by default): while the player controls a running match the pointer is confined to the game window's client area, so edge scrolling works next to a second monitor (reported on the Workshop: the pointer slid onto the other screen in every window mode). The clip is renewed each frame and released when the match is paused, the shop or settings window is open, control is with the AI, the match is over, the game is not the foreground window, or the extension unloads. `POINTER clip on/off` at verbose log detail. Translated in all 16 languages.

## 1.0.6 — 2026-10-11

- Match speed no longer follows the frame rate. The worker was held to one tick ahead of playback and a client frame can only play what is queued, so below 60 fps the match slowed in proportion (30 fps played at half speed; reported on the Workshop). The lead now follows the average client frame time: 1 tick at 60 fps and above (unchanged), up to 4 below, which keeps full speed down to 15 fps. It rises once the match would run more than 5% slow and falls back only when the smaller lead is nearly enough; one long frame or a pause does not raise it (tested). Frames are timed on the wall clock between running viewer calls. `TIMING frame lead a -> b` is logged for the first eight changes of a match, and `TIMING slow playback` when a ten-second window ran under 95% of full speed (which also shows a slow simulation thread, not addressed here). Passed in game on 0.7.0 at 30 and 45 fps (build 1.1.9 of the game-0.7 branch).
- The shop hides items the game reports as not enabled (an item mod that is installed but switched off for the save), in the grid, the filters, the filter counts and the recipes.
- An "Emote volume" slider (0 to 100% in steps of 5, 70% by default) replaces the emote sound switch; 0% is silent. The game accepts a playback volume from 0 to 1, so 100% is the level of the sound file, under the game's SFX volume. The built-in sound is generated at a higher level. Settings saved with the old switch off stay silent; with it on they get 70% (tested). Translated in all 16 languages.
- Stall report: a worker thread that has exited is reported as `WORKER ENDED` (normal when a match finishes), not as a stall, and an unreadable stack says why.
- All four were developed and passed in game on the game-0.7 branch (game 0.7.0); this build is the same code for game 0.6.3.

## 1.0.5 — 2026-10-10

- The emote sound plays 25% louder (playback volume 0.3 to 0.375); the sound file is unchanged.

## 1.0.4 — 2026-10-10

- The camera jumped back to the champion after the shop closed, and after any pause: it centred whenever the match went from not running to running. It now centres at match start and when control is taken back from the AI only (`centred` in camera.rs, tested); a pause and resume leaves it where it was.
- Both stats panels disappeared about a second into a pause: the stats reader dropped any sample older than a second, and the simulation samples nothing while paused. It now keeps the last sample while paused, as the HUD and Tab panel already did, and clears at the start of a match.
- The whole HUD blinked once on resume: for the frames between "running" and the worker's first new sample, the HUD, Tab panel and stats panels applied their staleness rules to samples from before the pause. They now enforce staleness only after the match has been running for 500 ms (`NativeTiming::client_sampling`, tested).
- Sliders: a drag starts with the press on the slider and lasts until the button is released, wherever the cursor goes (it used to end when the cursor left the slider's 294 x 50 box, whose track sat 6 px from the top). The value follows the cursor along the thumb's real travel, so the ends are exactly the minimum and maximum and the thumb stays under the cursor (`slide_ratio`, tested). A press that starts elsewhere grabs no slider. The track, thumb and number sit on the same centre line as the other controls.

## 1.0.3 — 2026-10-10

- "Queue whole build" queued an item the build lists twice only once, and skipped it entirely when one copy was owned: it tested "queued or owned" by item, not by count. It now queues each listed copy that an owned or already queued copy does not cover (tested).
- The queue strip shows four orders; longer queues looked cut off (the log showed six orders queued, four drawn). A "+N" count after the fourth chip shows how many more are queued. The chips are 10 px narrower to make room.
- Item grid: an item you cannot buy right now is dimmed even when you own a copy, matching its red price; owned items used to stay bright because a second copy could not be bought when that rule was written. The recipe tree and Builds into are unchanged.

## 1.0.2 — 2026-10-10

- The Background colour field went blank after scrolling the Interface page. Settings rows are a recycled pool: scrolling moved the option into another row slot, whose own native text field was empty, and the empty text was read as typed input. The field now carries its text over when its row slot changes (`hex_step`, tested). The saved colour was never affected.
- Text fields show keyboard focus as a yellow frame with a lighter fill (Background colour and the four Acquisition fields), replacing the thin yellow underline. No caret is drawn on purpose: the native field draws beneath the window and the game reports no caret position, so a drawn caret would not follow the arrow keys.

## 1.0.1 — 2026-10-10

- Diagnostics only, no gameplay change. A second match freeze (1.0.0, Riot item mod, tick 6592) was caught by the stall report with `last_step=Outside`: the worker was not in the mod's per-player work or its pacing wait, but the report could not tell the game, another mod or one of our native hooks apart.
- The stall report now logs the frozen worker's call stack as `module+0xoffset`, three times two seconds apart (`WORKER STALL stack n/3`): the same stack each time is a wait, a changing one is a loop. New native_adapter/windows/stack.rs suspends the worker only while it copies up to 48 return addresses into a fixed array, and allocates and logs after the worker runs again. The stacks are still logged if the player pauses or hands the frozen match back.
- The native hooks on the worker mark their own steps (`Send`, `Steer`, `Move`, `Attack`, `Skill`, `Aim`, `ShopHook`), each covering the hook and the game function it forwards to, and put back the step they interrupted. `Outside` now means none of the mod's code.
- The stall line adds `players_begun` (players whose per-tick work started since the last published frame) and `last_player`.

## 1.0.0 — 2026-10-10

- First public release, as "LT Takeover: Open-Source Direct Control" (mod ID `lt_direct_control`) by Layton, on the Steam Workshop and GitHub. Same code as 0.83.2.
- Documentation for release: the README covers Workshop and manual install, every control (stats panel, unit selection, camera), the languages, the `tools/prepare_sdk.py` build step and an updated project map; the modding guide adds "Text and languages" (`tr`/`trf`, the language files and their tests, font fallbacks), the `ui_click::once` rule for click callbacks and the SDK step for game updates; the third-party notices list the bundled emote images and the shop stamp as outside the public-domain dedication and no longer mention design files that are not in the repository; the backlog moves the release work to Done.
- The Home+End no-cooldown test option stays in as a small easter egg; the mod is single player only.

## 0.83.2 — 2026-10-10

- The language menu still would not reopen after a change. The 0.83.1 log showed why: every click on a rebuilt window arrived twice (open, then close, in the same millisecond). The game keeps a path's click callbacks when our window is removed and rebuilt within a match, and the rebuild registers again. New ui_click.rs lets each node's click through once per 50 ms; all 8 registration sites (settings, shop, session bar) use it. This also covers the shop checkboxes, camera lock and Tab toggles after a background-colour or language rebuild, which would have flipped twice.

## 0.83.1 — 2026-10-10

- Boxes instead of characters (Simplified Chinese, Thai and others in the language list): the game picks a font set entry by ITS language, and each entry only covered that language's script. Every entry of the mod's two font sets now falls back to all the game's script fonts (Latin/Cyrillic/Vietnamese, SC, TC, JP, KR, Thai), keeping its own fonts first. New tools/font_fallbacks.py (also used by generate_hud_fonts.py) rewrites them and their digests.
- The three two-choice switches (Hold/Toggle, Ignore/Honor, Near champion/Near cursor) are translated.
- A select menu could not be reopened from the same field: once closed, its invisible panel stayed over the lower half of the field it opened from and caught the click. The closed menu now moves off-screen. `SETTINGS select ...` lines log field clicks and menu closes in case it persists.
- Test: every window template (settings, shop, session bar, HUD, emote wheel, Tab panel) stays structurally whole in all 17 languages.

## 0.83.0 — 2026-10-10

- All of the mod's own text follows the mod language (342 texts in each of the 16 translated languages): the whole Settings window (every option, choice, keybind, the acquisition and emote panels), the shop (filters, headings, buttons, status, messages, recipe paths), the HUD (respawn, cooldown, purchase tooltips), ability feedback, the session bar tooltips, the Tab panel headers and the emote wheel hint. Stat names follow League's localised terms; Traditional Chinese matches the Riot item mod (物理防禦, 技能疾速).
- Plurals and word order are per language: the code passes named values (`{count}`, `{gold}`, `{item}`) and separate texts for one and several, so Russian and Polish use their own counting forms.
- Kept in English on purpose: key names (Q, Shift, Mouse 4…), emote names (GG, Nice, Hype, Oops, Focus), log lines, emote-import error details, and the developer no-cooldown tag. The lang test collects every text from `tr`/`trf` calls plus the settings tables, page names, stat filters and slot names.

## 0.82.0 — 2026-10-10

- Mod text in the game's language, first slice. New lang.rs: English source text is the key (`tr`, `trf` with named `{values}`); translations in `probe/lang/<code>.json` for all 16 other game languages; a test requires every text in every language with the same `{values}`. Settings > Interface > Language > Mod language: Auto (the game's language, identified from its own word for "Close" via the SDK text lookup, rechecked every second) or any of the 17 languages; `LANGUAGE ...` log lines record the detection. Windows rebuild when the language changes (ui_theme::refresh keys on background + language).
- Translated so far: the Settings window frame (title, pause notes, footer buttons, status), page names and hints, and the Mod language option. Every option label, hint, section and choice already goes through the table and shows English until translated.
- Select menus with more than three choices open as a two-column grid (the language list has 18).

## 0.81.0 — 2026-10-10

- Renamed for the public release: mod ID, crate and DLL `lt_direct_control` (was `lt_direct_control_probe`), in-game name "LT Takeover: Open-Source Direct Control", author Layton. Every packaged art path follows (`asset/lt_direct_control/...`, including the two font sets, re-fingerprinted). Settings, keybinds, logs and emotes stay in `%LOCALAPPDATA%\LTDirectControl`.
- The installer checks a pre-1.0 install in `mods\lt_direct_control_probe` against its build record and removes that folder after a successful install, so the game never loads both (its `ui\imported` emote cache is rebuilt from the user's emote folder).
- The game's mod SDK is no longer in the repository: `tools/prepare_sdk.py` copies it from the game and adds the mod's two local additions.

## 0.80.1 — 2026-10-10

- Add four user-provided bundled Library emotes: Bomber Nice, Clown Son, Ghost Wajaja and Monk No Kills, using the corrected `Bomber_Nice.png` and `Clown_Son.png`. They are separate normalized 256 × 256 PNG assets, with visible art fitted to 224 pixels, and are available without copying files to AppData or restarting to import them. The original five wheel defaults stay the same.
- Preserve filename-derived assignments, including the old Bomber/Clown filenames, and avoid duplicate Library entries when originals are also in the personal import folder. Personal variants still use the existing import/restart flow; originals and saved settings are not overwritten. Custom sounds remain deferred.
- Imported emotes now also normalize to 256 × 256, preserving the existing padding proportions and on-screen size. Originals remain untouched. Updating cached personal images still needs a restart after staging; bundled images are ready immediately after launching this release. Custom sounds remain deferred.

## 0.80.0 — 2026-10-10

- Emotes > Library: Open folder, Refresh, paged emote grid and assignment to the five existing wheel slots. Assignments use the Settings draft, Apply/Cancel and Restore this page. Unicode filenames are supported; missing or unavailable imports use the slot's built-in emote without changing saved assignments. The original five emotes remain available.
- Static PNG imports from the user log/settings folder's `emotes` subfolder (`%LOCALAPPDATA%\LTDirectControl\emotes` normally): up to 256 × 256 pixels, 1 MiB per file and 64 imports. Decode to RGBA, trim transparent margins, preserve aspect, fit visible art into 112 × 112 and centre on a transparent 128 × 128 canvas. Originals remain untouched. Animated PNGs, malformed/oversized/empty images and file links are rejected with reasons in the library and log. Custom sounds and animated imports are deferred.
- Imports are prepared at startup or on explicit Refresh (a single background worker, with bounded scanning/decoding); rendering reads an immutable catalogue, with no file loading or image decoding per tick. Content-versioned files in the installed mod's `ui/imported` cache preserve textures that may still be in use. New/changed images require a game restart and display Restart in the library until then. No texture-reload hook or game save modification is introduced. Native imported texture loading still requires user testing after restart.
- Emotes > Display keeps placement, scale, zoom mode, cooldown and shared sound controls. Preview uses a fixed display scale and pans at extreme settings instead of shrinking to fit, so 200% and 300% camera samples differ. Long imported names are bounded in the wheel/library.

## 0.79.1 — 2026-10-10

- Dedicated Emotes settings page with height (0–240 UI px), scale (50–200%), fixed-screen/follow-camera zoom behavior and shared cooldown (0.25–6 seconds, default 1.5). Existing enabled/sound settings keep their values. Defaults preserve the previous image size and height. Following camera zoom scales image size, vertical gap and pop lift using the actual native zoom value.
- Silent live draft preview beside the controls, Replay and 50/100/200/300% sample camera zoom buttons. The preview shares gameplay geometry and animation, fits extreme settings without clipping, runs while Settings pauses the match and never changes the match camera, cooldown or sound. Apply/Cancel/Restore this page use the existing settings transaction.
- A newly triggered emote replaces the current image after the configured cooldown. Two-second display and pause behavior are unchanged. Minimum cooldown exceeds the packaged 160 ms sound duration; unsupported sound requests now log a reason. Native rendering, audio and zoom appearance remain pending user testing.

## 0.79.0 — 2026-10-10

- Local emote wheel: hold T (rebindable), choose GG/Nice/Hype/Oops/Focus and release; Escape/right-click cancels. Five original images, one reused native overlay, 2-second display/pop/fade and 6-second cooldown, optional quiet sound (off by default). Match playback drives durations, including pause. No network/AI/replay integration. Settings > Interface can disable emotes; existing world orders continue while choosing.
- Emote input is blocked during shop/settings and skill/attack-move aiming. Focus loss, death, AI handover and new matches clear the wheel/image; held keys and cancel clicks are drained to prevent accidental commands. Cooldown persists across same-match focus/death/AI transitions. The wheel fits screen edges; the floating image stays off HUD/minimap. UI spawn failures retry slowly and do not release gameplay control.
- Advanced > Debug > Capture performance measurements: fixed-size opt-in timing histograms cover SDK callbacks, unit scanning, combat, selected mod hook work, native-inclusive attack/skill/move/outline hooks, shop lock/decision work, log I/O/rotation and capture-to-playback delay. Ten-second reports include counts, averages, maxima and p95/p99 upper bounds. Client callback totals remain explicitly separate from native-inclusive/overlapping work. Input trace storage is bounded and capture can continue past the ordinary diagnostic sample limit.
- Performance: reuse one entity scan for skills and attacks; retain enemy/hover/ability snapshot vector capacity; share applied input settings without per-poll JSON copies; skip disabled attack trace construction and mutex work; lazily format selected verbose messages; remove lowercase allocations in log severity checks and the client frame counter's temporary vector. Left-match rejection now precedes the session gate. Input frequency, worker lead, game timing and safety guards are unchanged.
- Added a read-only numeric performance report tool and a compatibility/lower-spec test guide in docs/emotes-and-performance.md. Native rendering, sound, gameplay and actual lower-spec performance remain pending user testing; automated checks do not establish an FPS improvement.

## 0.78.2 — 2026-10-09

- AI control: the game's layout toggle works again ("exit full screen", F). The mod forced full screen every frame even in AI control; it now restores the spectator's own camera, vision and layout once on entering AI control and leaves the toggle alone, forcing full screen again on taking control. A session released from AI control keeps the layout the spectator chose.
- AI control keeps the match paced at 1x, one frame ahead, so F11 can take control back; the game's speed buttons and its "view result" button need the game to run ahead. Changing speed or pressing 立即查看比賽結果 (`ingame.option_buttons.view_result` / `pad_view_result`) in AI control now hands the match fully back to the game (new ai_handback.rs, logged as NATIVE RELEASE with the reason); the result button may need a second press.
- Leaving a match mid-way: the game finishes it in the background at full speed, and the mod's per-tick code still ran for every player and tick (session locks shared with the UI). The left match's ticks now return at the first step (lock-free key check, `SESSION left mid-match ...`). Suspected cause of the brief ~3 FPS interactions after leaving; not yet measured.

## 0.78.1 — 2026-10-09

- The shop's self-opening waited too little: 0.78.0 opened it (and paused) in the same tick as the game's one-time decision, which runs in the buyer after that tick's build read, so Recommended kept the four-item read until the match resumed. The user's log proved the decision itself works (`SHOP DRY RUN ... build Some(4) -> Some(6)`, nothing bought). The shop now opens after the next publish, whose build read follows the decision.

## 0.78.0 — 2026-10-09

- Recommended shows the whole build plan from the start. The AI (and the Riot item mod, which adds slots 5-6 then, pinned items included) completes the plan inside the game's buy-new decision at its first purchase; Manual shopping answered that question for the controlled champion, so the plan stayed at the game's first four items until the first manual purchase. Now, once per match after Start, the buy-new hook calls the game's decision with the caller's six arguments (four registers plus the two stack arguments of call site 0x146b959), discards its answer and asks the shop again (`Answer::AskGameFirst`; a second pass-through is never allowed). Logged as `SHOP DRY RUN ... build N -> M items; game wanted ... (discarded)`; any purchase it made would show as SHOP UNEXPECTED.
- With Manual shopping, the shop opens by itself once per match when the plan is complete, on Recommended; "Pause while open" applies as usual.

## 0.77.4 — 2026-10-09

- Housekeeping: removed the 0.75 Drawing test setting (Settings › Advanced › Debug) and drawing_test.rs; the preview redesign it checked is accepted. A saved `drawing_test` value is kept as an unknown field and ignored. Backlog refreshed to 0.77.3.

## 0.77.3 — 2026-10-09

- Stall report (0.77.2 froze mid-match: the worker stopped at tick 4919 with nothing logged). The viewer now writes `WORKER STALL` when a running match has published no frame for 3 s (not while paused, not while the worker waits for playback), with played tick, frame counts, phase and the last step the worker entered (`last_step=` Bind, Team, Shop, StatsPanel, Hud, AttackRange, Combat, Abilities, Dispatch, Publication, ...; `Outside` = not in the mod's per-tick code), and `WORKER RESUMED` if it continues. Both are Safety lines, written at every log level. Markers are two relaxed atomics on the viewed match's worker only (new worker_watch.rs).
- Target frame glyphs: towers (and the nexus) show the game's tower icon, monsters its jungle icon (`asset/base/ui/icons/tower`, `jungle`); minions keep the mod's minion glyph.

## 0.77.2 — 2026-10-09

- The background colour now moves the design's whole warm-grey ladder, not only the darkest layer: settings rows and hint box, text-field fill, frames and rules (`#4b4a49`), scroll tracks, the shop pill, plates and disabled buttons, the vision toggle track, tooltips and toasts (`#1e1e1d`), hover and pressed tones, health wells and modal shades. Each shifts by the background's distance from `#1c1a18` per channel (new `ui_theme::tone`), so the default looks exactly as before. Templates and properties write these as `#~rrggbbaa`, rewritten by `ui_theme::themed` at spawn and in `hud_motion::properties`; the minimap frame line uses `tone` directly. Text inks stay fixed.

## 0.77.1 — 2026-10-09

- Own stats panel: rows 21 px apart instead of 18 (panel 92 px tall instead of 80), still vertically centred on the skill slots.
- Target frame relaid out for a unit with no mana: health bar across the top (shield appended as `(+N)`), portrait with level badge beside the 3 x 4 stats grid; 292 x 129 instead of 292 x 172. The unit-type note is gone (the glyph shows it).
- Fixed the Background colour swatch: on a stroked node `color` is the edge and `back_color` the fill, so it now sets `back_color`.
- More surfaces follow the background colour: HUD skill and inventory slot insets (background + 5, empty inventory slot + 13) and the Tab panel's item and portrait insets (+ 8); the Tab panel now rebuilds when the colour changes.

## 0.77.0 — 2026-10-09

- Stats panels (the vanilla spectator panel is hidden under direct control). Your champion's 12 stats sit left of the Q slot in the HUD's skill-slot style (grey frame, inset fill = background + 5, drop shadow), toggled by the new Stats panel binding (default C, rebindable; remembered as `stats_panel_shown`). A left-click on any unit selects it, as in League (attack-move and pending casts keep the click; UI and the minimap are ignored); empty ground clears it. The selected unit's frame at the top left shows its head portrait (the game's face icon; a unit glyph for minions, towers and monsters), level, health bar (green ally, red enemy) and the same stats. It hides when the unit dies or leaves your team's vision.
- Stats, icons only: AD, AP, attack speed (60 / attack interval), armor, magic resist, ability haste, crit, move speed (per second), range (live AA range), lifesteal, armor and magic penetration. Haste, lifesteal and penetration are summed from the unit's buffs. Read on the viewed worker during the HUD sample (new stats_panel.rs, stats_ui.rs); `STATS own raw ...` is logged every 10 s to check the values against the game's own panel. The stat icon code moved to the shared stat_icons.rs.

## 0.76.8 — 2026-10-09

- The shop's top and bottom strips follow the background colour, keeping the design's relation (`#22201e` = background + 6 on each channel), via the new `ui_theme::shade(delta, alpha)` for surfaces derived from the background.

## 0.76.7 — 2026-10-09

- Fixed shop filters not responding (0.76.6): each filter label received two alternating property strings per frame (position, then colour), so both were re-sent to the game every frame; they are one string again. The filter icons also moved out of the filter buttons to sit above each row, leaving the buttons exactly as before 0.76.6.
- Fixed the garbled detail tag for the mod's own icons: inline text images need a sprite sheet and a frame name, so the seven icons now ship as one sheet, `ui/stat_icons#sheet.png` + `stat_icons#data.sprite_sheet` (frames `crit_0`, `haste_0`, ...), the format Workshop mods use. UI assets 93 -> 88.

## 0.76.6 — 2026-10-09

- Shop stat filters show an 18 px icon before each label (the "All" row keeps its label at the left edge); an icon dims with its label when no item matches. The item detail tags show the same icon before each stat name, inline in the text. Icons: the game's stat sheet for Attack Damage, Ability Power, Attack Speed, Armor, Magic Resist, Health and Move Speed; the mod's new 9x9 icons for Critical Strike, Ability Haste, Life Steal, Armor Pen and Magic Pen.
- The seven new icons ship as `ui/stat_*.png` (UI assets 86 -> 93), generated and recorded by tools/generate_stat_icons.py; verify_build checks the count and digests.

## 0.76.5 — 2026-10-09

- Settings › Interface › Appearance › Background colour: a free hex colour ("#rrggbb", in the settings text-field style with a swatch) replacing the design's `#1c1a18` on every mod surface: HUD strip and slot gaps, session buttons and their hover tone, settings window, masks, conflict dialog and Acquisition page, shop window, masks and detail panel, and the battlefield notice plate. Dark text on bright buttons keeps its fixed ink. Invalid text shows orange and keeps the last valid colour; the colour applies on Apply, when the windows rebuild through their usual missing-node path (new ui_theme.rs).
- New tools/generate_stat_icons.py: original 9x9 single-colour icons for stats the game has no icon for (crit, armor and magic penetration, HP regen, shield, lifesteal, haste), written to design/hud/stat-icons/ with a review page. Not used in game yet.

## 0.76.4 — 2026-10-09

- Aligned the Acquisition search field's right edge with the champion portrait grid, including its text mirror and focus decoration. Shortened champion-only attack-move choices to Ignore and Honor without changing their behaviour.
- Replaced the shop header bag icon with the supplied Nerdge_Stamp_Transparent.png artwork, tilted 7.5 degrees counter-clockwise. A reproducible SVG wrapper renders the unchanged source to a transparent UI asset; the stamp fits inside the header and retains its original colours.
- Set Automatic acquisition's default/reset minimum to 120 game units, with the existing +5 AA buffer. Saved shared values and champion Custom overrides are preserved; maximum scaling AA reach and live AA floors are unchanged.

## 0.76.3 — 2026-10-09

- Fixed Advanced's Acquisition and Debug buttons being blocked by the settings clipping mask. The navigation subtree now follows the masks, its enclosing input layer is explicitly above them, and its passive container lets the child buttons receive clicks. Scrolling masks remain event-blocking for clipped content; settings drafts and subpage layout are preserved.

## 0.76.2 — 2026-10-09

- Fixed stale champion faces in unused portrait slots. Empty cells are hidden, cleared and non-interactive; a reused cell reloads its face and becomes clickable again. The grid uses five complete rows beneath Advanced's new subpage bar.
- Advanced now has General, Acquisition and Debug subpages. Cancel attack wind-down and Manual shopping moved to General; logging, selection collision markers and drawing tests moved to Debug. Each subpage restores only its own options.
- Acquisition exposes the shared Automatic minimum and AA buffer as numeric game-unit fields (defaults 70 and 5), with a separate reset that preserves champion overrides. All Automatic champions now inherit the shared minimum; obsolete generated per-champion minima are removed once. Custom radii still floor at current AA reach. Invalid numbers block Apply; changes retain the existing draft, Cancel and pause behaviour.
- Debug's Show live acquisition radius draws a yellow acquisition ring, a thinner white AA ring and both numeric values around the controlled champion. Values come from the same selected-player simulation callback as targeting, independent of settings browsing or draft edits. Background matches cannot overwrite them; paused matches retain their last sample and running matches suppress stale samples.
- Caret restoration was investigated and deferred with user permission. The native editor draws at fixed z=100 and the SDK exposes no caret/selection offsets. Committed text and the focus underline remain visible; the caret, selection and uncommitted IME composition remain a known visual limitation. No new native hook was added.

## 0.76.1 — 2026-10-09

- Moved champion acquisition controls to Advanced. A fixed settings pane sits beside a compact, scrollable grid of game-cropped champion faces; the selected face has a yellow outline. Complete rows stay inside the body, including when switching from a scrolled page. Combat & casting retains its original layout. Restore this page resets acquisition preferences only on Advanced.
- Champion search accepts internal IDs and names in the game's supplied languages, plus available translations from enabled mods. The controlled champion is initially visible. Portrait clicks resolve through the last displayed slot map, and face images are refreshed only when their slot changes.
- Search and radius fields have explicit labels and visible text above the modal: native text-edit rendering has a fixed low layer. Native typing/IME state is retained, with a visible focus indicator; Automatic shows a read-only computed value and Custom enables the numeric editor and slider.
- Reduced Automatic's minimum from 120 to 70 game units and its buffer from 10 to 5. It still uses maximum scaling AA reach, and Custom still floors at current AA reach. Old generated defaults migrate once; per-champion custom radii and other saved preferences are preserved.

## 0.76.0 — 2026-10-09

- Attack-move has a separate acquisition radius: Automatic uses the larger of the 120-unit baseline, maximum level-scaled AA reach + 10, and current AA reach + 10. The live game experience table supplies the level cap; the controlled champion's fingerprinted native AA descriptor supplies range growth. Unknown or uncapped scaling falls back to current reach. Acquired enemies outside AA range are approached, including enemies away from the clicked destination; actual AA range still controls cooldown waiting and movement stops.
- Combat & casting settings include a searchable active champion selector (Workshop champions included), Automatic/Custom choice, radius slider and numeric entry, current/max AA reach, effective radius, and per-champion restore. Custom radius is floored at current AA reach without rewriting the chosen value. Apply/Cancel and existing pause ownership apply to the entire draft; preferences and champion defaults persist in controls.json. Input polling avoids copying this potentially large champion table.

## 0.75.4 — 2026-10-09

- In-range manual basic attacks stop ordinary movement before the native attack consumer checks cooldown, including orders accepted by the SDK. The current target and worker/match must match; native attacks still run normally, and cooldowns, windups, skill casts, pending hits and forced movement are preserved. Verbose logs include the in-range decision and each movement stop.
- Ground A-click and Shift + right-click acquire enemies only within the champion's current native AA range, replacing the fixed 120-unit search radius. If no enemy is in reach, they continue toward the clicked destination. Direct right-click and A-click on an enemy retain their explicit target and approach it when out of range. Target highlights use the same acquisition range.

## 0.75.3 — 2026-10-09

- Dancer R previews as a fan of blades (like Ashe's W in League), read from its apply code: count = min(max_count 10, base_count 4 + the caster's kill stack, entity +0x648, the stat block's `stack`); blade 0 flies at the aim, the next half turn +60° x i / half and the rest -60° x (i - half) / half, clockwise on screen, so even counts are lopsided exactly as in game. Each blade is drawn as its 134-unit corridor (10-unit hit radius) with the smaller dash-size head.
- Effects recognised without an area (buffs, single-target shots, visuals, damage on units already hit, Cavalry Knight R's buff and speed road) are named instead of reported as unknown; a skill made only of them shows no direction guide. Cavalry Knight R, Gunner, Bard Q and Exorcist W no longer draw a misleading arrow.

## 0.75.2 — 2026-10-09

- Skill previews for three hand-written base-game ultimates, their fields matched word for word to `champion_info` (ranges after the patch adjustment): Ice Mage R is a 70-unit, 90° cone swept from the caster toward the aim (it showed only the direction guide); Bard R an aura around Bard (99 units); Exorcist R an area where the cast lands (61 units, separate from its 100-unit cast range). Cones placed at the caster now face the aim.
- The other unknown types in the logged trees are single-target or self effects and correctly show no area: Gunner Q (shot), Gunner R (sub-shots buff), Bard Q (attack-speed buff), Exorcist W (buff), Executioner W (bleed, `1ad9600/48`). Cavalry Knight R's `19de840/112` stays unidentified.

## 0.75.1 — 2026-10-09

- Skill previews drawn in the user's Endfield x League design (design/previews/concept.html), replacing the hatching. Pieces: range as a quiet gray ring with no fill; ground areas with a 6.5% field, bright rim (dark under-stroke, glow, main stroke, white core), four clipped yellow accents and a centre grip (self areas without the grip); skillshots as a 4.5% corridor with thin white edges plus the open-spearhead arrow (48 px head); cones with a strip-filled sector, rim, centre spine and inner arc; walls (the game's upright rectangles) with one wide strip as fill, bright short ends and a grip; dashes as the open spearhead (37 px head, a grip when very short); blinks as a four-part aperture with a pin; unit targets as a team-coloured ground arc (red enemy, blue ally; the hover outline is the existing one) and a yellow overhead marker, orange beyond range. Unavailable skills are gray; aim beyond range adds an orange dashed line and ring.
- Every colour, width, opacity and size lives in the new preview_style.json (preview_style.rs); any subset can be overridden in `%LOCALAPPDATA%\LTDirectControl\preview_style.json`, read at start-up (logged as PREVIEW STYLE). Fills use non-overlapping 3 px strips; area fills use one native disc when clear of the UI and strips (which clip) otherwise. The skill decoders are unchanged.
- Not built: the design's 550 ms dash reveal. The 0.75.0 Drawing test setting stays until the design is accepted.

## 0.75.0 — 2026-10-09

- Diagnostic for the skill-preview redesign (design/previews/concept.html), no behaviour change: Settings › Interface › Debug › Drawing test draws a fixed card on the battlefield with the calls the design depends on and the mod has not used in game yet: 64/24/8 px strips at 6.5% and 25% opacity, flat and at 30° (square ends, even transparency); two crossing 25% strips (stacked transparency); the open-spearhead head strip-filled with 3 px strips (seams); a 90 px disc at 6.5% inside a segmented rim (fill matches the rim). New drawing_test.rs, with the strip fill the renderer will reuse.
- Logged camera extents are always square (819.2 x 819.2, otherwise 1024 x 1024), so filled discs stay round on screen.

## 0.74.4 — 2026-10-09

- Tidy-up, no behaviour change: the 0.70 SPRITE CALIBRATION / SPRITE SAMPLE diagnostics are removed (the selection work they served is done). PREVIEW TREE / PREVIEW ROSTER stay until the previews are finished.
- docs/modding.md covers the match hook (`CooldownHook`), skill-preview decoding and art-measured selection; docs/backlog.md lists what is left.

## 0.74.3 — 2026-10-09

- Skill previews: a timed dash with no effects on units it hits (Candygel R's slide, Nullifier Q) is drawn as cyan movement of its exact length instead of a body-wide corridor; dashes that hit (Harpy R) keep the corridor.
- Effects scheduled at or after the end of a timed dash in the same skill happen where the dash ends: caster-centred areas and drops at the caster's feet move to the dash end. Candygel R now shows a 42-unit pool at both ends of the slide (dropped at tick 10 and at tick 54 = 10 + 44). Dashes that stop at the first unit (Rush) have no fixed end and are not used.
- No-cooldown test option: the game's formula is haste-style (0.74.2 test: +99 halved cooldowns), so the buff now uses +900, leaving a tenth of each cooldown.

## 0.74.2 — 2026-10-09

- No-cooldown test option fixed: 0.74.1's -100 cooldown multiplier gave Bomber Q a 300 s cooldown and froze the cast mid-animation. The game's own items reduce cooldowns with positive values (Staff of Rapture and Angel's Fang +10, Prophet of the Abyss +15), so the buff now uses +99 for skill and ultimate, which is safe under either plausible formula (about 1% of the cooldown if linear, about half if haste-style).
- While the option is on, each cast's remaining cooldown is logged (`TEST no cooldowns cast slot=… remaining_ticks=…`), read from the game, to settle the formula.

## 0.74.1 — 2026-10-09

- Skill previews: a projectile with no effects on units hit in flight (Bomber Q and W, Poison Dart Hunter Q and W, Bard W) draws no corridor, and its end effects land where the cast is aimed (new placement Landing: the target for unit casts, the aimed point within the flight length for location casts, the full length for direction casts). Bomber Q is now a 20-unit and W a 40-unit circle at the cursor, matching `explosion_range` / `attack_range` in the champion data.
- Bomber R stays one 25-unit circle (`explosion_range` 25000): in game, the edge of the game's own landing marker (about 50 units, drawn at twice the damage size) takes no damage while the halfway point does.
- Testing aid: Home+End toggles "no cooldowns" for your own champion in the viewed match (new test_cheats.rs). Off at every start and never saved; while on, a match hook keeps one permanent `lt_test_no_cooldowns` buff (skill and ultimate cooldown -100%) on the champion, re-applied after respawn, and removes it when turned off. Background matches are never touched. A red "TEST: no cooldowns" tag is drawn and each toggle is logged. It changes the real match.

## 0.74.0 — 2026-10-09

- Rejected basic attacks now hold position when their target is already within the current native AA range, retaining the target until the attack becomes ready. Out-of-range targets are still approached; range uses the current effect's base, level growth and bonus rather than sprite selection bounds. Native cooldown, windup and damage remain authoritative.
- F12 hands control to AI without releasing the match coordinator. F11 or the Take control button reclaims the same champion. AI remains paced at 1x with one-frame lead; handoffs clear pending movement/casts and camera gestures, retain the full-screen layout and preserve manual camera/vision choices. Startup cancellation and adapter failures still release permanently for that match. AI spectator pause is respected; reclaim uses the manual pause coordinator.
- Shop: Everything is now All; the header uses the existing shop bag icon. Purchase controls show only Purchase, Queue or Queue another plus the remaining price, keeping long item names out of the button.

## 0.73.0 — 2026-10-09

- Skill previews decode five more native effect types, named by pairing the logged effect trees with Workshop and data-driven champion JSON (field values checked against those files): RangeProjectile (an area at its placement after a delay), ParabolicProjectile (the landing area at the aim, plus its landing effects), RushTime (a timed dash, speed x ticks long, body wide), SwitchByBuff (the branch without the buff is previewed; the caster's buffs are not read) and MoveToTarget (movement onto the target). No new drawing: each maps onto an existing preview piece.
- Replaying the 57 logged skill trees: 33 now have a footprint, up from 23 (Alchemist Q, Bomber R, Pyromancer Q and R, Harpy R, Nullifier Q, Candygel W, Squirrel W, Spirit Latcher R, Cavalry Knight Q; Bomber Q and W gain their explosion areas).
- New docs/preview-drawing.md: what the game's drawing calls can and cannot do, for designing previews; docs/investigation-preview-selection.md lists the named effect types.

## 0.72.3 — 2026-10-09

- Monster names fixed: the game names them `<sheet>_monster` (bee_monster, mushroom_monster, rhino_monster, stump_monster, as logged by 0.72.2), not `_jungle`; the wrong 0.72.2 mapping is removed.
- Mod champions shipped only as `.aseprite` files (Leef's Variety: Harpy, Nullifier, Candygel and the rest) get measured bodies: the idle frames' pixels are read from the Aseprite file (layer and cel opacity applied; `miniz_oxide` 0.8.9, already built as part of `png`, is now a direct dependency) and the body is placed around the drawn centre with the drawn facing, since the game packs these sheets itself and their atlas positions are unknown. One body per champion (no form switching for these).

## 0.72.2 — 2026-10-09

- Monsters get their measured bodies: native names are `<sheet>_jungle` (as in the game settings), so `bee_jungle` now maps to the `bee` sheet, `tree_jungle` to `stump`, and so on, for both the measured body and the old fallback size table (one shared `sheet_name`). Before this, every monster used the large radius-based fallback box.
- `SPRITE ART unmatched name=…` is logged once per name for any unit still on the fallback after the art has loaded.
- 0.72.1 Verbose log: hover picking costs about 25 µs per frame on average, 104 µs at most.

## 0.72.1 — 2026-10-09

- Minions get measured bodies: their shared sheet (UI_aseprite/minion) is loaded as `ui/minion`, and the drawn frame picks the kind. One sheet can now hold several kinds: animations named `<kind>_<tag>` form a kind when a `<kind>_idle`/`_run`/`_walk` exists, and each frame uses its kind's standing body. Minions get 8 bodies (melee/ranged, each side, Morgard); champion forms get their own too (Druid bear/eagle, Demon Archfiend, Berserker, Ghoul berserk, Bombardier deployed, Dokkaebi, Cavalry Knight fire, Gunner forward/backward run).
- Click padding on measured bodies: champions 3 units wider on each side, every body 2 units below the feet.

## 0.72.0 — 2026-10-09

- Selection bodies measured from the installed art on a background thread at start-up (new sprite_art.rs, `png` 0.17.16): the opaque pixels of each unit type's standing frames, with sideways protrusions trimmed (columns under a quarter of the tallest: guns, spears), keeping heads, hats and raised staffs. Base sheets come from the game bundle, mod champions from their own `#anim.fanim` / `#sheet.png`; towers and the nexus add their orb sheet. Nothing is copied.
- Placement from the game's own draw data: the outline hook records each unit's drawn centre and body sprites; the drawn frame is found by its atlas position, giving the true anchor and facing (mirrored when flipped). Units without loaded art or a recent draw keep the previous envelope.
- Hover and click ranking follows the agreed order: enemies before allies, structures last, body hit before edge hit, the previously hovered unit within its tier (stickiness), then the body the cursor is most central on relative to its size, then the unit drawn in front, then a stable id. The blanket champion-first rule is gone; the champion-only key covers fights. Skill target brackets use the same area.
- Show selection markers now draws each unit's selection area: green for a measured body, grey for the fallback. The 0.70 magenta/cyan overlay is removed; the 0.70 inventory logging remains.

## 0.71.0 — 2026-10-09

- Skill previews decode four more native effect types, identified from the 0.70 effect-tree inventory with layouts checked against every logged instance: Delayed (its effects keep their placement), lingering area projectiles (circle/rectangle at their placement), Rush dashes (a corridor from the caster to the cast range, its hit width from the effect) and MoveTo movement (a line to the aimed point).
- Linear projectiles with hit radius 0 (they stop at the first unit) are drawn as thin lines instead of being rejected, and their end effects are now read and drawn where the projectile ends: on the target for unit-targeted skills, otherwise at its full flight length (new placement End).
- Replaying the 60 named skill trees of the two 0.70.1 test matches: 22 now have a footprint, up from 9, including Illusionist W (60-unit fear circle around the caster), Poison Dart Hunter Q (dart and 48-unit poison splash), Ghost Q (movement) and Ghost W (dash).

## 0.70.1 — 2026-10-09

- Diagnostics, no behaviour change. PREVIEW TREE: the viewed match's champions get their own budget (240 lines; each champion's Q/W/R once per match) so background matches (200 lines, each tree shape twice) cannot crowd them out; PREVIEW ROSTER is written once per match instead of once per player.
- Show selection markers also draws each unit's body sprite placement from the game's draw commands: magenta = full-size sprite centred on the unit, cyan = half-size from its corner to the unit, beside the existing hover markers, to settle which matches the art.

## 0.70.0 — 2026-10-09

- Diagnostic build for the skill-preview and selection work (no behaviour change). PREVIEW TREE: the Q/W/R effect trees of champions in the viewed match and in background matches, with each node's apply function, size and raw payload words, children followed only beside game-owned effect tables and into memory confirmed readable; each entity once per thread, each tree shape at most three times, at most 300 lines. PREVIEW ROSTER: actor id to champion for the viewed match.
- SPRITE CALIBRATION / SPRITE SAMPLE: once a minute on the battlefield (at most 10 times) the camera frame and each unit's simulation position and projected screen point, and the next 40 unit draws from the outline hook (view id, drawn position, raw sprite command words).

## 0.69.2 — 2026-10-09

- HUD tooltips grow upward to the available battlefield height, then widen from 529 to 640 or 760 pixels when needed. Descriptions exceeding the largest card scroll in whole lines, with a slim scrollbar; wheel input works over the card or its originating HUD tile without zooming the camera. Moving into the card keeps it open. Font size and weight are unchanged.
- Explicit native line spacing now matches measured body/title heights. Shared text measurement counts inline stat icons, preserves their source spacing, and wraps adjacent icons; scrolling retains color runs, icons and the final description line. Layout is cached until content or available space changes, including when a native description replaces fallback text.
- Accepted clicks on unit/structure bodies no longer create the ground cursor animation. The decision uses the click-time picker, including allies, and clears any earlier ground animation. Direct attack orders retain their sprite outline pulse; ground move/attack-move and minimap destination feedback remain available.
- User confirmed 0.69.1 Ghost and Harpy (mod champion) tooltips worked. Regression checks cover card growth/widening, long multilingual/icon descriptions, scroll clamping/reset/color continuity, target click suppression, retained attack orders/pulses, and ground/minimap marker expiry. Native layout and click feedback await user testing.

## 0.69.1 — 2026-10-09

- Enable native skill descriptions for all champions, retaining the ordinary resolver when native text is unavailable. Expand the checked ChampionInfo implementations from six to 65 by tracing the built-in lookup's return branches; 407 tooltip code/table guards now cover built-in, data-defined and registered mod wrappers. No new hooks, and the tested register/ownership bridge is unchanged.
- Workshop descriptions can be requested even when the SDK cannot provide a localized template. Open skill tooltips refresh with cached native results; the bounded session/language cache accommodates the complete base skill inventory without clearing pending results after 24 entries.
- Add structured per-skill coverage diagnostics and tools/audit_tooltips.py. Unresolved parameter names are counted separately from ellipsis punctuation; failed lookups and unobserved skills remain explicit. The refreshed installed bundle contains 204 English Q/W/R description entries across 68 champions. Existing 0.69.0 logs confirm six native results with zero unresolved placeholders; the remaining 198 entries await runtime observation.
- Regression checks cover all base champion IDs, Workshop text without SDK translation, missing optional skills, cache deduplication, unresolved-parameter parsing, and structured/legacy coverage reports. User confirmed the 0.69.0 Illusionist/Alchemist prototype worked perfectly; broader native rendering/gameplay remains pending user testing.

## 0.69.0 — 2026-10-09

- Guarded native skill-description prototype for Illusionist and Alchemist only. Hovering Q/W/R requests the same localized, parameter-filled description interface used by the game's champion-info screen. Other champions and unavailable native results retain the existing resolver; tooltip typography, metadata and layout stay unchanged.
- Native access runs after the existing viewer update, on the bound client thread with its borrowed Assets. The register-pair lookup bridge, ChampionInfo Arc release and native String allocation are isolated in the Windows adapter. No new hooks or function patches. Fifty-two additional code/table guards are verified against the reviewed 0.6.3 executable.
- Owned descriptions are cached per skill, localized template and control-session generation. Pending/failed requests are deduplicated, an already-open tooltip refreshes when its result arrives, and release/new-session transitions discard cached results. Normal logs identify native/fallback results; Verbose logs include returned text for comparison. Native game calls and rendering still require the user's test.
- Regression checks cover native register returns, stack alignment/shadow space, heap text copies, last-owner Arc cleanup, cache invalidation and stale responses, optional-skill fallback, and refusal of changed tooltip profile guards.

## 0.68.2 — 2026-10-09

- Item descriptions scroll independently beneath the fixed item heading, with a slim scrollbar only when the text overflows. The entire description is reachable without ellipsis truncation; wheel input affects the description or item grid under the pointer. The description resets for a different item, preserves color runs and inline icons across scrolling, and clamps when the available height changes.
- Regression checks cover reaching the final line, extreme wheel input, item/source changes, viewport resizing, multilingual wrapping, blank lines and color/icon continuity.

## 0.68.1 — 2026-10-09

- Shop path selectors use larger plates with 16px bold text, visible borders and layered contact shadows. Automatic (cheapest) has its own row; Use this path occupies the empty space beside each chain. The selected mode/path has a yellow fill and dark text, without a checkmark, and hover darkens the actual hovered button.
- Branching chains show five components per page to leave room for the path button; paging retains every component and the fixed event slots. Single-path and vanilla items keep their compact six-column layout and existing purchase behavior.

## 0.68.0 — 2026-10-09

- Recipe prices again show the gold still needed from current inventory, cumulatively along the displayed path (250 → 900 → 1,650), rather than individual upgrade charges. Competing branches use their own costs; a full inventory does not reduce a displayed price to the last upgrade charge.
- Alternative paths gain Use this path and Automatic (cheapest). A choice is remembered per item for the current match and copied into each new purchase order, including Queue whole build. Buyer decisions, projected purchases, queue prices and the HUD's next purchase all use that order's path. Existing queued orders keep their choice; remove and requeue to change one.
- A chosen chain reuses the most advanced available owned component on that chain; it never substitutes a different branch. Invalid paths fail closed, and full inventory blocks a new chain. Automatic planning and queue-order rules are unchanged.
- Vanilla and every single-path item retain the compact linear recipe with no alternate rows, path labels, automatic-route markers or choice controls. Branching is detected from registered item data. Checks cover all 30 vanilla items and six linear chains, cumulative prices, pinned expensive routes through the real buyer interface, projection/HUD consistency, duplicate orders and blocked/invalid paths.

## 0.67.2 — 2026-10-09

- Shop recipes now use a structural upgrade graph instead of the remaining purchase plan: ancestors of owned items and all alternative paths remain accessible, even when buying is blocked or the final item is owned. Two complete chains per page, with path and component navigation for larger trees; alternative rows are explicitly marked OR.
- Builds Into preserves the branch followed. Auto-buy labels and yellow links identify the cheapest remaining purchase route independently; browsing never forces a more expensive route. Recipe tiles show per-step prices and each row its full-chain total; list/detail/Buy prices keep their existing remaining-cost calculation.
- Recipe graph and path counts are cached by live catalogue/root; paths are addressed directly without enumerating combinations. Cycles, duplicate and unknown edges are guarded. Regression checks cover the missing base component, alternate Blade routes, browsing across pages, ownership/full-inventory changes, long chains and a graph with over a billion possible chains.

## 0.67.1 — 2026-10-09

- The HUD strip's next purchase now shows what the buyer will really buy next. With Vanilla order off: the first affordable next part in queue order, or, while none is affordable, the cheapest one (the first gold will reach). With Vanilla order on: the first unfinished item, as before (shop::upcoming replaces first_open).

## 0.67.0 — 2026-10-09

- F11 now also pauses: pressed while control is running it queues the same Pause action as the Pause button (pending orders cleared as for a click); it still starts and resumes. Button tooltip "Pause · F11"; key binding label "Start, pause or resume control".
- Shop: "Vanilla order" checkbox in the shop header, on by default (saved like a setting, shop_vanilla_order). With it on only the first unfinished order buys and later orders wait, like the game's own buyer, instead of buying parts of several items; the in-base preview follows the same rule.

## 0.66.3 — 2026-10-09

- lib.rs split into lib.rs (logger and start-up wiring), client.rs and simulation.rs; Client::post_update and Simulation::think now call one named step per feature (install, athlete choice, re-arm, windows, UI blocking, pointer, diagnostics; session binding, roster, Tab, purchase trace, shop, HUD, controlled athlete, samples), moved verbatim.
- Layering: settings::MODAL and shop_ui::OPEN moved to ui_state.rs (SETTINGS_OPEN, SHOP_OPEN); settings are created at start-up and shared with the cursor and client, and saved by the client each frame instead of by the cursor. No behaviour change.

## 0.66.2 — 2026-10-08

- native_adapter.rs (about 4,100 lines) split into native_adapter/mod.rs (the API the rest of the mod calls) and native_adapter/windows/: mod.rs (patch installation and shared helpers), layout.rs (every 0.6.3 address, call site and byte pattern), movement, combat, outline, view, input, shop and tests. Pure moves: every item verified unchanged apart from formatting and crate-internal visibility.

## 0.66.1 — 2026-10-08

- Internal renames, no behaviour change: movement_test.rs → movement.rs (MovementTest → Movement), timing_test.rs → test_support.rs, ClientProbe → Client, AiProbe → Simulation; crate documentation describes the two entry points. Mod ID and probe.log names unchanged.

## 0.66.0 — 2026-10-08

- Log detail setting (Settings › Interface › Debug): Quiet, Normal (default) or Verbose. Lines are classified by tag in one place (probe/src/logging.rs); install, fingerprint and release results, fail-closed shop decisions and anything reporting a failure or panic are always written; per-click, per-move and per-second traces (MANUAL CLICK, MOVEMENT, CONTROL DIAGNOSTIC, NATIVE TRAFFIC, OUTLINE, PERF, CURSOR TRACE, attack and ability traces) only at Verbose. A full match log drops from about 5.4 MB to about 0.12 MB at Normal.

## 0.65.2 — 2026-10-08

- Shop recipe tree: every part not owned shows the gold it still needs from what you own (League price, red when unaffordable), not just its own step price; owned parts keep their muted own price

## 0.65.1 — 2026-10-08

- Cursor flicker fix: the game sets its cursor on every WM_SETCURSOR (0.65 log: game SetCursor calls equal WM_SETCURSOR count); while our cursor is active on the foreground game window, the game import call shows ours instead, so the game cursor no longer flashes before ours while moving
- Removed the full native UI-tree log dump on scene changes (196 ms freeze at match start in 0.65 log)
- Shop window caches purchase plans (shop::offer) and the in-base projection until catalogue, inventory or queue change, instead of recomputing them several times per frame while open
- 0.65 diagnostics (PERF, CURSOR TRACE) and lock-free hook rejection kept; CURSOR TRACE also reports shown_ours_instead

## 0.65.0 — 2026-10-08

- Diagnostic build: PERF summary every 10 s (client update intervals p50/p95/p99/max, mod callback time with per-part breakdown, native viewer/worker time kept separate, lock and worker pacing waits, hook calls split by viewed-match worker vs other threads, log bytes) and up to 8 PERF SLOW lines per window for battlefield frames of 25 ms or more
- Cursor trace: CURSOR TRACE every 10 s and up to 6 CURSOR HANDBACK lines per window with reasons; counts replacements of our cursor between frames, WM_SETCURSOR reapplies and the game own SetCursor calls through a pass-through import-table counter (nothing substituted; restored at shutdown)
- Lock-free rejection before the abilities lock (movement/attack/skill hooks) and the shop lock (buyer hooks) for entities and players that are not the controlled ones; outcomes unchanged, full ownership checks kept

## 0.64.2 — 2026-10-08

- Item slot count comes from the game purchase gate (cmp rax,N at 0x146baf3 allows N+1: 4 vanilla, 6 Riot), read-only, instead of the build plan length; used by the shop, the strip inventory and Tab, with the plan length only as a fallback
- SHOP MODE first decision logs game_item_slots

## 0.64.1 — 2026-10-08

- Orders instead of a set: each Buy is an order that excludes the copies already owned, so the same item can be bought several times; orders claim their satisfying slot or the part their route builds on, in queue order
- No purchase is blocked (user rule; the Riot mod manual plan does not restrict either); only the game rule that a new item needs a free slot remains
- Messages on a solid plate under the Buy button with a tone bar (bought/queued/refused), 2.5 s; chips remove a single order and show buying

## 0.64.0 — 2026-10-08

- Purchases shown at once: in base the shop projects the queued steps exactly as the hooks hand them out (shop::project) and uses the projected inventory and gold for prices, dimming and Buy; gold reads real -> left; pending slots get a yellow outline and clock; pill says purchases apply when the match resumes while paused
- Queue is first come, first served (unaffordable items never block); Recommended gets Queue whole build
- Recommended tab label widened to fit

## 0.63.3 — 2026-10-08

- Item list: owned badge removed (owned items look normal); only queue positions are badged
- Item frames get an opaque inner fill so a dimmed icon never shows the frame colour (fixes the yellow tint on a selected owned part)
- Selected highlight is blue RGB 16,162,217 in the item list, the recipe tree focus and Recommended rows

## 0.63.2 — 2026-10-08

- Starting items: the game buys them at tick 1 while the lane can still change; before Start (Loading/Ready) Manual shopping now holds every champion of this match by native object (other simulations untouched); at Start only the controlled champion stays manual and the others buy on the next tick. First-decision log shows the controlled champion items
- Dimming: item list dims items you cannot buy now (gold or full inventory), owned look normal; recipe tree and Builds into dim owned parts with a tick glyph and dim unbuyable parts without one
- Hook proof counts prestart_held answers

## 0.63.1 — 2026-10-08

- Fix: Manual shopping read the session phase with the client-only phase(), which is None on the simulation worker, so every decision was native (shop showed Manual shopping off, starter and later items auto-bought). New worker_phase(); Manual applies in Loading, Ready, Running and Paused once the champion is locked, until F12
- Every change of shop mode/reason is logged (SHOP MODE first decision / changed)
- Builds into click moves the recipe tree to that item (root and focus); clicks inside the tree still only move the focus

## 0.63.0 — 2026-10-08

- Manual shopping applies from champion lock (Ready, Running, Paused) until F12, and the shop publishes as soon as this worker owns the match, so starting items are no longer auto-bought; the shop can be opened during the Start-control wait
- League-style right panel: Builds into (focused item) -> recipe tree (root, fixed until an item is picked from the list) -> Buy (focused item) -> detail; clicks inside the tree or Builds into only move the focus
- Hover tooltip following the cursor over any item (grid, Recommended, tree, Builds into, slots, queue), 117 ms fade-in, instant hide
- Prices are League-style: gold still needed from what you own (tiles, Builds into, tree root, Recommended, tooltip), red when gold is below it; tree parts show their step price
- SHOP MODE first decision of match logged

## 0.62.0 — 2026-10-08

- Shop text: native line_height is pixels; body uses 23 px (0.61 used 1.4, which stacked lines)
- Shop detail panel flows top to bottom (stats and effect, Recipe, Builds into) above a fixed Buy area; the Buy button names its item
- Stat filters from live item stats (AD, AP, attack speed, crit, haste, life steal, armor, magic resist, health, move speed, armor/magic pen), multi-select AND with live counts; works for vanilla and item mods
- Pause while open: header checkbox in the shop, saved as a hidden setting; uses the Settings pause request and only resumes a pause the shop made; in-base state held while paused
- Strip: bag icon removed; the purchase column (next item, gold, required gold, P cap) is the shop button; with Manual shopping on, next purchase and required gold follow the shop queue and are blank when nothing is queued

## 0.61.0 — 2026-10-08

- Native shop window (P or the strip shop button beside the gold): Recommended (the game build list) and All items with category filters, level-grouped item grid with a fixed 63-tile pool and wheel scrolling, detail panel with stats and effect text (inline stat icons), cheapest recipe route, Builds into, one Buy/Queue button, owned slots and queue chips; right-click buys; Esc closes without cancelling a recall; hidden while Settings is open
- Manual shopping: nothing is ever queued automatically (P no longer queues the build list); Buy moves the item to the queue front and the first queued item with an affordable next step is bought; Manual shopping on with missing item data fails closed (buys nothing)
- Hook proof logging: champions seen by the buyer hooks, answers given to the controlled champion, SHOP UNEXPECTED for any item that arrives without a shop step; the In base indicator is driven by the hook being asked about the controlled champion
- New Lucide glyphs ef_star, ef_grid, ef_bag (white-only verified)

## 0.60.0 — 2026-10-08

- Manual shopping (Combat & casting > Shopping, Off by default): optional redirects of the native buyer controller thunks 0xf60620 (upgrade) and 0xf61b80 (buy new) answer from the player queue for the controlled champion only; everyone else, AI control and the setting Off get the native answer (another mod patch inside 0xf3f510 is preserved); hook failure disables only Manual shopping
- The simulation still validates and performs every purchase; the mod only answers one step at a time, never buys a non-base item as new, and never answers the same step twice
- Temporary queue source until the shop window: P queues the next unfinished item of the champion build list
- Item tooltips keep the item mod inline stat icons (<i#asset/...:name>); skill tooltips unchanged
- 0.59 diagnostic shop trace retained

## 0.59.0 — 2026-10-08

- Diagnostic-only shop trace: logs every purchase step of all ten players (gold before/after, owned items before/after, native owned-slot count, final build list); copies item lists only when gold falls or every 30 ticks; 400 lines per match
- Once per match, compares the live native buyer code (executor slot gates and payments, controller buy/upgrade vtable slots, their thunks and decision prologues) with reviewed 0.6.3 bytes and names the module owning any redirect
- No native state is written; 0.58.2 behaviour, outlines and layout are unchanged

## 0.58.2 — 2026-10-08

- Coordinate native renderer/config and IngameUI full-screen flags using the existing viewer hook and its live ingame node, with exact TypeId and shared-config identity checks.
- Let native UI update position announcements, kill feed and spectator controls in the full-screen layout. Remove 0.58.1 option_buttons visibility/event workaround; retain baseline custom-HUD panel suppression.
- Save and independently restore both original layout flags on F12; reject different viewers/configs/UI objects and changed shared-config links; discard stale lease at match reset.
- Keep user-approved outlines unchanged: hover 1.2, attack target 2/3, click peak 3.0, decay 120 ms, click > hover > attack.
- Add exact native profile guards and regression coverage for coordinated layout restoration, live-node downcast and identity checks.

## 0.58.1 — 2026-10-08

- Fix hover offset at 1.2; remove 2.5 wide-hover mode, F10 prototype polling/switch, mode state, client switch argument and package description.
- Retain pulse peak 3.0, 120 ms interpolation, attack offset 2/3 and click > hover > attack same-unit priority.
- Hide the separate compact spectator ingame.option_buttons group during direct control; save/restore native visibility on AI release and suppress/restore its eight known interactive descendant surfaces.
- Retain 0.58 full-width battlefield lease, native profile guards, outline cleanup, all-role outline setting and independent debug selection markers.

## 0.58.0 — 2026-10-08

- Reduce persistent attack-target outline offset from 1.0 to 2/3; retain exact 1.5 crisp and 2.5 wide hover appearance.
- Accepted explicit attack clicks pulse from offset 3.0 to the settled offset over 120 ms. Repeated clicks restart; held buttons, worker ticks and ground attack-move acquisition do not. One role per unit: click > hover > attack.
- Discard pulse on expiration, replacement/cancelled order, fog/death, inactive control and session reset; Sprite outlines gates all roles; debug ground markers remain independent.
- Lease the native full-width battlefield flag before owned bootstrap/update and restore the spectator preference on return to AI. Bind camera/vision/layout restoration to the matching live viewer/config only; fresh sessions save fresh preferences.
- Add two reviewed native layout consumer guards to the runtime profile and source verifier; retain all existing outline calls, ownership cleanup and safe fallback.

## 0.57.0 — 2026-10-08

- Add persistent thin red sprite outline for the valid active Attack/AttackMove target, independent of the single stronger hover winner and cursor.
- Publish captured hover and attack target together; hover role wins for the same entity to avoid a double border or duplicate outline pass set.
- Use authoritative alive/targetable/visible snapshot before enemy refresh to clear death/fog targets; stop, move, no acquisition, stale/inactive state and reset clear target feedback.
- Rename Interface option to Sprite outlines and gate both roles using existing saved hover_outline key; debug ground markers remain independently gated.
- Use 1.0 offset for attack target and existing 1.5/2.5 crisp/wide hover; record hover_drawn and target_drawn diagnostics; no new native hooks.

## 0.56.1 — 2026-10-08

- Correct Show selection markers to gate both hovered and selected attack-target ground circles/construction corner markers. Debug Off hides both, Debug On restores both.
- Update debug hint to cover hover and selected targets. Selection, orders, sprite outlines, skill/range previews and minimap paths retain their existing behaviour.

## 0.56.0 — 2026-10-08

- Add persistent Interface > Battlefield feedback > Hover sprite outlines (On default), gating only the published native outline target and retaining picking/cursor feedback.
- Add persistent Interface > Debug > Show selection markers (Off default), retaining existing hover circles/construction markers as a tuning tool independently of outlines and attack-order markers.
- Fix vision-mode hover: compute pointed segment at HUD scale and animate its own non-interactive hover layer; selected indicator and tooltips remain independent.
- Reuse declarative option catalogue, Apply/Cancel, restore, controls.json and scrolling; older settings receive new defaults without overwriting user preferences.

## 0.55.3 — 2026-10-08

- Correct tower-only hover outline: accept NinePatch ground circles, Text labels and all reviewed native RenderCommand variants; outline sprites only and append complete original drawing unchanged.
- Release unused independently owned extra-pass commands through fingerprinted game RenderCommand cleanup; preallocate bounded merge before extra renders; clean up valid extras on fallback without drawing duplicates.
- Retain single shared hover winner, enemy-before-ally priority, blue allies/red hostiles, F10 crisp/wide and ground markers pending approval.
- Log observed command variants plus malformed-vector and missing-sprite counters; guard native cleanup head/table/caller and NinePatch tag; verify all cleanup bytes and dispatch statically.

## 0.55.2 — 2026-10-08

- Accept mixed native Sprite and DrawLine body commands; outline only the independently owned sprite copies and preserve original commands.
- Capture one hover winner per frame for the cursor, sprite outline and ground marker; publish native identity, position and team together.
- Hostile units outrank allies, including enemy minion over allied champion; champions retain priority within each side and champion-only filtering is preserved.
- Targeted aiming uses the skill-eligible single unit or clears hover on a miss; area previews retain multi-target geometry.
- Blue allied and red enemy/hostile-neutral outlines and hover markers; F10 compares crisp and wider coloured outlines, ground indicators retained.
- Guard the native DrawLine tag write; include accepted mixed-command draw counts in outline diagnostics.

## 0.55.1 — 2026-10-08

- Hover-only sprite outline prototype: four shifted gold flash passes or one soft glow pass, switched with F10 in the same match
- All 15 generic unit-body renderer calls are version-fingerprinted redirects; ordinary units keep original rendering and existing ground rings remain
- Native render commands are moved into a new vector; empty vectors and failed heap frees are guarded, and changed command shapes disable the prototype

## 0.55.0 — 2026-10-08

- Hover-only sprite outline prototype: four shifted gold flash passes or one soft glow pass, switched with F10 in the same match
- All 15 generic unit-body renderer calls are version-fingerprinted redirects; ordinary units keep original rendering and existing ground rings remain
- The wrapper checks native entity identity and position, uses game shader setters, moves independently owned commands into a new vector, and disables itself if the render command shape changes

## 0.54.0 — 2026-10-08

- On/Off settings use compact full-row checkboxes from the updated HTML preview, preserving saved values and click routing
- Segmented-control hover brightens the unselected track while the selected pill remains stable until clicked
- Raised settings buttons and the dropdown have contact shadows and crisp outlines modeled on the Endfield UI lab

## 0.53.0 — 2026-10-08

- Settings window rebuilt to the approved preview geometry: section headings, segmented track with sliding indicator, select field with chevron and fading option menu, icon close button, scrolling content with clipping masks and scrollbar, keybind capture hint and column headings
- New Combat & casting > Attacks option: Cancel attack wind-down (On default) gates the 0.51 post-hit release
- Bottom strip reordered: Pause, Return control to AI, Lock camera, Settings (Lucide gear), three-way vision group, K/D/A, CS, Tab; the more menu is removed; B recall moves beside the level and HP bar
- Option labels and hints match the preview wording; real line breaks via native text instead of escaped source text

## 0.52.0 — 2026-10-08

- Schema-driven native settings window: Combat and casting, Keybinds, Camera, Interface; reusable choices, sliders and binding capture/conflict replacement; old more-menu preferences migrated
- Settings own only the pause they caused in the same match/generation; modal input and camera gating, held input protection, Apply/Cancel drafts and required bindings
- Shift-right-click attack-move; configurable Hold/Toggle champion-only (Hold default), attack-move filter and nearest champion/cursor preference; per-slot cast modes and primary/secondary keybinds
- Versioned controls.json retains cursor/low-health preferences and unknown fields; new defaults populated on Apply; native selection, targeting, item and post-hit attack behavior retained

## 0.51.0 — 2026-10-08

- Post-hit attack release: a locked BaseAttack (action 3, no queue entry) records its hit tick from declared start timing (+4a8) scaled by the action speed factor (+80); release requires elapsed (+78) strictly beyond it, an empty pending queue, native CC gate and the attack's own cooldown
- Release reasons: a waiting buffered skill (previously dead gate, now reachable) or a manual ground Move / S stop issued after the attack started; attack, A-click, recall and automatic chase/hold inputs never release
- Release uses the native stop event and clears only the action; cooldown, damage and queued (Gunner-style) attacks unchanged; 0.50 attack trace retained

## 0.50.0 — 2026-10-08

- Diagnostic-only basic-attack trace: per accepted attack, classify the native queued versus locked (action 3) outcome, record raw start timing, attack-speed bonus, expected hit tick, both action payload words, queue and cooldown, then follow action/queue changes at existing observation points
- Bounded to 400 attacks and 12 changes per attack per match; no native writes, hooks or behaviour changes
- Retain all 0.49 Ghost tooltip, glyph alignment and click-through Tab behaviour unchanged

## 0.49.0 — 2026-10-08

- Resolve Ghost Q from skill1 declarations and champion-level takedown bonus fields; Ghost W Time placeholder uses declared charge count
- Align cooldown/range tooltip glyphs with their numeric text
- Remove Tab row hover tint and all Tab input-blocking geometry so battlefield commands pass through the informational panel
- Retain accepted 0.48 repeated Tab reopening, KDA colour formatting, compact purchase shortfalls and adaptive inventory

## 0.48.0 — 2026-10-08

- Keep styled Tab visibility and property cache synchronized on close/reopen, including focus loss and AI handoff
- Correct KDA rich text to native eight-digit RGBA tags with a final colour reset
- Show uncertain purchase branch shortfall as one compact minimum plus sign; retain exact branch prices and remaining gold in tooltip
- Preserve approved Tab layout, adaptive inventory slots and accepted controls, native vision, minimap, HP and screen effects

## 0.47.0 — 2026-10-08

- Implement approved styled native Tab panel without team-name stripe, with live level/KDA/CS/held gold/items/respawn and selected-row edge
- Use native automatic-buy target vector for dynamic slot count, owned-item lower bound and same-match/player retention; no mod-ID or catalogue-size assumptions
- Right-align four/five/six HUD slots at fixed tile/art size; align Tab item columns using match-wide capacity and preserve per-player slot counts
- Update inventory tooltip hit areas and separate next-purchase identity from item indices; allocate additional rows/slots beyond six within bounded read limit
- Preserve accepted 0.46 purchase-reader, construction selection, HP contrast, optional low-health effect, native vision and F11/F12 control behavior

## 0.46.0 — 2026-10-08

- Fix registered item lookup: native buying context +8 points to settings; remove erroneous simulation tick +1b8 chain, retain stage-specific rejection diagnostics
- Preserve valid catalogue entries when unrelated upgrade destinations are absent; forecast still requires complete selected paths
- Map generic tower/nexus names to base art profiles, use full body bounds, and draw construction body corner highlights instead of an inflated ground ellipse
- Use dark green HP fill with outlined white current/maximum values
- Render optional stronger low-health warning using cached native UI nodes, clipped to battlefield and excluded from minimap; retain static fade and death shading
- Provide separate Tab/HP HTML proposal and side-by-side comparison; installed Tab remains unchanged pending review

## 0.45.0 — 2026-10-07

- Implement approved Endfield HTML tooltip typography, 529 px content cards, native stat colors and compact purchase-unavailable state
- Package Manrope 500/650 and Noto Sans TC 500, language fallback font sets and 16 SVG-derived HUD/menu glyphs with licenses
- Implement 328 px cursor/vision/effect menu, capsule slider and component-specific tonal hover/press, segmented selector and opacity-only tooltip motion
- Bind native vision to controlled side by default, offer own/opponent/all and restore original spectator vision on release, with four exact consumer guards
- Change default start/resume to F11 and AI return to F12; retain existing skill/input/purchase policies and expanded minimap

## 0.44.0 — 2026-10-07

- Implement reviewed warm HUD palette, native bold text, 80 px skills, adjacent inventory/purchase group, lime HP and outline-only champion selection
- Close six-pixel strip gap above controls without changing battlefield rendering allocation
- Add bounded presentation-only hover/press, selection, health trail, tooltip fade/rise and panel entrance motion; cache individual properties
- Add optional default-off low-health static edge tint with smooth onset, minimap/top-bar exclusions, death/release cleanup and persistent toggle
- Retain native cooldown, charge, modded item/skill data and purchase branch uncertainty; no speculative gameplay timers

## 0.43.0 — 2026-10-07

- Expand native minimap to 352 px inside a 360 px footprint with 4 px warm-grey surround and muted stepped corner strokes
- Resize native map background, fog grid, markers, objective timers and camera rectangle using 54 reviewed instruction operands and private read-only constants
- Align native left-click camera navigation and mod right-click destinations, path and click indicators with shared live map geometry
- Trim the bottom strip to x1560 and preserve current combat HUD text/icon sizes; broader approved HUD redesign remains separate
- Guard every new instruction and original constant; protect each executable page once and audit patched bytes/private constants

## 0.42.0 — 2026-10-07

- Migrate reviewed native worker/view/input/attack/skill/steering/shader locations to exact game 0.6.3
- Update entity kind/position/identity/skill/effect fields and native purchase build/settings readers for changed layouts
- Add independent loaded-code field-consumer and ASLR-correct table-pointer guards before installing hooks
- Verify explicit profiles against executable identity, call targets, layout guards and Rust constants; preserve both full game baselines privately
- Reuse 0.41 HUD, controls, cursor art, previews and input-buffer policies; approved revised HTML implementation remains separate

## 0.41.0 — 2026-10-07

- Implement supplied HUD anatomy with separate 80px skill tiles, flat neutral 50px bottom strip, centered 360x24 HP bar and larger KDA/CS glyphs and values
- Use supplied SVG HUD glyphs; six inventory slots on right, working automatic-purchase forecast/gold on left; preserve mod icons, colored descriptions, death banner and visual-only hover preview
- Add explicit visible slider track, ratio fill and clamped handle; keep native slider input, saved 24-64 px sizing and reset
- Extend minion selection upward five world units and downward from two to five without widening; known towers/nexus use full structure profiles and unidentified towers get a 48x80 fallback
- Keep team availability on Tab, vanilla top stripe and minimap placement, native movement/casting/timing, and cursor assets unchanged; true outlines deferred

## 0.40.0 — 2026-10-07

- Extend champion picking upward by 20 percent of baseline height, bounded to 6-12 world units; preserve lateral/feet envelopes, monster profiles and ground-ring dimensions
- Keep only the latest accepted click marker; preserve contraction and fade from 100 to 250 ms, expiring at 250 ms without cancelling movement orders
- HUD skill hover shows known live reach or caster-centered area without arming, aiming or queueing a cast; active normal-cast preview has priority
- Reuse verified cursor artwork and size settings; native buffering, skill input, geometry readers, movement and timing remain unchanged
- Bounded outline feasibility review found no verified per-entity current-frame association; retain existing highlight and defer true silhouette outline

## 0.39.0 — 2026-10-07

- Champion picking uses idle/run/walk baseline with at most 15 percent combat-pose expansion; retain feet forgiveness and unchanged monster profiles
- Embed fourteen cursor states from supplied SVGs using native Windows pointers, scaled hotspots and a thread-local post-WM_SETCURSOR observer; restore host cursor over UI and after release
- Cursor states share direct-click enemy picking and normal-cast ally/CC/self eligibility; preserve persistent A mode and champion-only toggle behavior
- Add saved 24-64 screen-pixel size slider, default/reset 32, to existing more-controls menu without replacing other controls.json preferences
- Replace permanent battlefield destination cross with clipped move/attack click markers: 167 ms contraction, fade from 367 to 567 ms; minimap clicks receive a smaller minimap marker
- Keep native command buffering, preview geometry, movement and timing unchanged

## 0.38.0 — 2026-10-07

- Separate casting input from shape and placement; support multiple primary footprints without flattening secondary hit effects
- Copy bounded geometry from exact-build live selected-actor Combine, RangeEffect, LinearProjectile and channel-line effect families at most 10 times per second; retain no native pointers in HUD
- Prefer recognized live dimensions; enabled explicit declarations are a fallback for opaque effects, with bounded per-skill source/family/unsupported diagnostics
- Render self-centered areas without misleading cast-reach rings; allow unit-targeted corridors and native forward-offset areas; decode finite segments, rectangles and DirDot cosine thresholds
- Keep existing native cast validation, input modes, manual aim preservation, HUD/cursor and startup timing

## 0.37.0 — 2026-10-06

- Preserve manually controlled direction/point skillshot aim at native effect launch: native stat/RNG evaluation runs once on a POD copy; AI players retain original correction
- Log intended and native-suggested directions when correction would redirect a manual skillshot
- Adapt supplied preview HTML geometry into native drawing: white dashed reach, yellow declared corridors/circles, cyan declared movement, sprite-sized unit brackets and orange approach/out-of-range feedback
- Unknown, conflicting or state-dependent effect footprints use reach/aim guides rather than guessed widths or hit predictions; native range includes live level/bonus changes
- Preview strokes clip to battlefield and exclude minimap/HUD; unavailable abilities grey out; existing normal/quick/Alt/release cast and cancellation rules retained
- Record supplied design integration decisions: Tab-only availability, neutral grayscale panels, six item slots and existing input modes

## 0.36.0 — 2026-10-06

- All native movement, attack and skill observers require the registered live simulation worker, current match key and selected actor; background actor-ID reuse cannot change charges, busy state or acknowledge casts
- Clear-segment steering recovers current native position fields without a previous-command address token; native speed, events, wall fallback and routing are retained
- Native automatic-attack call is observed in addition to explicit attack input; eligible buffered skills can release only proven committed BaseAttack animation, preserving cooldowns and pending effects
- Cast cancellation, expiry, target loss, busy transitions and native rejection include trace IDs and concrete reasons; retries do not flood the log
- Native traffic records distinguish steering entry, selected-worker entry, direct steps and pointer rejection; attack-start records identify input versus native-auto source

## 0.35.0 — 2026-10-06

- Skill edge wins a simultaneous mouse-click poll; a later click cancels the older cast and native acknowledgement prevents replay
- Selected actor uses native arbitrary-angle stepping on verified clear segments, retaining speed/events and normal navigation fallback
- Ready buffered skill can release a BaseAttack backswing only after observed start delay and an empty pending-effect queue, retaining attack cooldown and CC guards
- HUD uses declared cast count with bounded display retention rather than deriving count from rounded capacity/cost; zero-cooldown display hides phantom multi-use indicators
- Runtime diagnostics rotate at 4 MiB through current and two earlier segments; late-match movement sampling no longer ends after 1800 samples

## 0.34.0 — 2026-10-06

- Static skill artwork and last-known item icons survive transient telemetry expiry for the same selected identity; live readings remain freshness gated
- Ordinary self-hover highlight is filtered by controlled entity ID while overlapping allies and self-target ability data remain available
- Movement route code and native hooks unchanged; archived 0.33 traces and route review distinguish continuous motion from intermediate grid-derived goals

## 0.33.0 — 2026-10-06

- Hover snapshots publish only from the selected living player in the owned live match; unrelated callbacks cannot clear allies
- Hover snapshots have independent freshness and clear on death, inactive controls, prepared selection and session reset
- Base jungle monster profiles use idle/run/walk frames instead of expanded attacks; champion and structure profiles are retained
- Monster selection and highlight envelopes use smaller bounded foot/edge padding; tested champion and minion bounds are retained

## 0.32.0 — 2026-10-06

- One-frame running lead and viewer/phase condition-variable wake with bounded emergency fallback
- Running gameplay capture moved into lightweight pre-update using visible camera/masks; edge processing is not repeated post-update
- Purchase forecast includes tier-4 Radiant/custom finals with assigned-build graph and inventory consistency checks
- Declared skill values resolve with preserved native colors and AD/AP ratios; unsupported formulas remain unknown
- Champion/monster picking and highlight dimensions share enlarged sprite envelopes and foot padding; minions retain smaller bounds
- Bounded capture-dispatch-publication-playback and native queued-aim diagnostics; three queue-layout anchors added

## 0.31.0 — 2026-10-06

- Champion-first sprite rectangles include legs and forgiving edges for allied/enemy hovering, attack clicks and targeted skills
- Minion mouse boxes use normal sprite dimensions rather than combat radius; base monster art profiles bundled with bounded fallbacks
- Correct reversed registered upgrade edges: native getter +0x80 is next_tier; three constructor/getter anchors added
- Purchase traces report effective chain data and forecast only when build, inventory or metadata changes
- No additional native gameplay patches or HUD layout changes; Ninja movement and FPS remain deferred

## 0.30.0 — 2026-10-06

- Read effective registered item keys, icons, enabled state, incremental prices, tiers, native stats and upgrade edges inside live AI callback
- Owned metadata cached per match and validated against SDK catalogue; no native pointers retained and no new gameplay patches
- Purchase forecast uses registered modded prices and enabled branches; tooltip names use active localization
- Item descriptions resolve active localization even without base settings; registered stats and native RGBA color markup retained
- Background map callbacks stage geometry without clearing live destination or remaining route
- Five additional native item-reader anchors verified; Ninja movement and FPS investigation remains deferred

## 0.29.0 — 2026-10-06

- Respawn banner follows native ticks; existing battlefield-only death grayscale retained
- Accepted world orders survive focus loss and expired foreground heartbeats
- Minimap right-click destinations and remaining commanded obstacle route from read-only map geometry
- Identical active movement goals skip redundant native movement-reset events; bounded plain-walking traces
- Busy casts wait regardless of SDK command validity; three native skill consumer hooks acknowledge cooldown spending
- SDK item-build observer export pointer/length repaired; catalogue capture tested through host-facing vtable
- Three additional native CALL redirects and eleven additional byte anchors verified

## 0.28.0 — 2026-10-06

- Approved bottom-strip layout: fixed 224x124 combat panel, 64px skills, green HP, horizontal KDA/CS, gray flat strip, read-only inventory tracker
- Compact champion selection above strip; play/pause, camera lock and AI popover controls below; no persistent portrait/name/target-mode rows
- Team death status restricted to held Tab native scoreboard; right-click passthrough retained
- Death greyscale replaces only the #Game composite shader; HUD, top stripe and minimap composition remain unchanged
- Read-only final native build lookup after all overrides; active mod item prices and graphs; random branch costs are explicitly uncertain
- Original monochrome toolbar glyphs, yellow selection/check accents, retained custom champion/item texture handling
- One additional native CALL redirect for battlefield shader; five additional byte anchors verified, seventeen total

## 0.27.0 — 2026-10-05

- One replaceable cast intent: animation buffering up to one second, press-time unit binding, fixed world aim, targeted approach-to-range and Alt self-casting
- Optional cast-on-release preference; default quickcast and persistent Shift normal casting retained
- Champion-only mode filters A-click and ongoing attack-move acquisition; allies receive hover feedback
- Native cooldown/use-count shared-borrow reads drive remaining-use indicators and readiness
- Body-pose envelopes cover attack and skill tags while excluding separate effect tags; exact live-frame alpha picking remains unavailable
- Startup prepared-HUD capture and nested button-color properties corrected; redesigned layout remains a separate review mockup
- No new patched branches; twelve existing anchors unchanged; new read-only Action method calls verified against current consumers

## 0.26.1 — 2026-10-02

- Direct control activates when the mod is enabled; no development path or activation flags required
- Per-user LOCALAPPDATA diagnostic folder with TEMP fallback; unavailable logging no longer blocks mod initialization
- Read-only result reports use the selected portable diagnostic folder rather than the compile-time project path
- Retains 0.26 graphical UI and gameplay; C-only laptop and native UI testing pending
- No new native branches or offsets; all twelve anchors unchanged

## 0.26.0 — 2026-10-02

- Portrait-only own-team selector on the held battlefield; role changes allowed only before Start and bound to current match/generation
- Prepared snapshots for all players; re-selection clears movement, skill, recall and camera state without releasing the worker
- Preparation waits for Start or AI without 120-second automatic release; startup and two-second heartbeat guards retained
- Original graphical session controls and HUD symbols, opaque grayscale framing, exact yellow active accents and red death count
- Numeric level/HP/KDA/CS/gold, dimmed locked abilities and numeric portrait respawn timer; existing permanent sheet/PNG nodes retained
- Host-localized skill/item descriptions on delayed hover; unresolved formula values shown as ellipses; tooltip bodies ignore events and do not mask world commands
- Brief rejection feedback distinguishes known unlock levels/cooldowns; ordinary startup/control/aiming diagnostics kept in log
- No new native branches or offsets; all twelve anchors unchanged

## 0.25.0 — 2026-10-02

- Champion picking uses normal-pose body dimensions independently of combat collision and attack range
- 78 base animation profiles plus enabled-pack exported animation/Aseprite metadata discovered at initialization; unreadable art uses a fallback
- Shared picker for hover, right-click, direct A-click and targeted quick/normal cast; width and height scale with current camera extent
- Core body hits rank ahead of margin hits, then distance to visible body center and stable ID; champion-only filter retained
- Pick visible upper body with offscreen feet; reject off-viewport/minimap cursor; legacy minion/structure envelopes retained
- SDK does not expose exact animation frame/facing silhouette; padded normal-pose proxies require in-game fit verification
- Retain user-confirmed direct chase, target markers and team portraits; existing HUD, camera, casting and recall guards unchanged
- No new native branches or offsets; twelve anchors unchanged

## 0.24.0 — 2026-10-02

- A direct hit commits Attack(id) and follows the moving target; ground A preserves nearest-to-click acquisition
- Explicit target loss clears the order without fallback; A remains unrestricted by champion-only mode
- Enemy hover uses click hit ranking and masks; separate persistent current attack-target ring; stale/dead/hidden targets excluded
- Two compact five-player portrait rows use original-worker SDK side/lane/alive/respawn scalars; permanent mod-compatible portrait nodes
- Dead identity retained without entity; countdown freezes on pause and stale samples hide numeric timers; session reset clears roster and sampling
- Targeted ally-CC skill rejection explains its native requirement; no native validation override
- Retain confirmed consecutive-session lifecycle, compact HUD icons, camera, casting, recall and recovery guards
- No additional native branches or offsets; twelve anchors unchanged; match 33 recorded controlled series verified as 2-1

## 0.23.0 — 2026-10-02

- Rearm outside the battlefield on pre-match screens or return to title; new worker tick 1 must repeat startup acknowledgement
- Refresh own-team ownership and roster, unlock lane choice while retaining its default, clear orders and actor metadata
- Generation-scoped publication waits and retired-key/binding-window checks reject old worker and same-battle reentry
- Serialize session consumer reset against SDK scalar writes; clear HUD, targeting and camera state including finished native lease
- Seed physical input history; disable placeholder skill/item hover popups
- Retain confirmed permanent icons and death camera, native countdown, combat and session guards
- Use seed/generation result evidence filenames and retain bounded polling between battles; clear audit at title
- No additional native branches or offsets; twelve anchors unchanged; saved authority unverified

## 0.22.0 — 2026-10-02

- Restore in-place updates on permanent HUD image children; no per-icon remove/spawn operations
- Separate permanent PNG and sprite-sheet skill nodes to avoid inherited rect_tag state
- Log property application, node existence and bounds for changed icon references
- Retain selected camera identity without a living position; continue drag and edge pan during death
- Follow/recenter require a living position; preserve lock intent until manual pan, with no free-camera respawn snap
- Native death countdown, active mod asset/item metadata, session/combat controls and frozen result evidence retained
- No additional native branches or offsets; twelve anchors unchanged; saved authority unverified

## 0.21.0 — 2026-10-02

- Read enabled data-champion PNG skill declarations in configured order; exclude disabled packs and reject invalid paths
- Use active SDK item icon metadata; retain logical sheet overrides and evidence-based native item key/tag fallback
- Recreate image children on source/tag changes to avoid stale sprite-sheet cropping on standalone PNGs
- Sample selected player through other living actors on the original worker, deduplicated per tick, including death/respawn
- Keep dead portrait/inventory/skill art and native respawn timer; dim skills and clear pending commands
- Freeze original-worker player evidence at battlefield exit and capture visible result-screen labels read-only
- Existing session/combat/camera controls, guards and twelve native anchors retained; saved authority unverified

## 0.20.0 — 2026-10-01

- Scoped native UI buttons queue Start, Pause, Resume and Return to AI; Ctrl+Home/End retained as backups
- Pause holds publication and playback while retaining camera/Tab; clears pending commands and prevents held-input replay
- Removes the 60-second running cutoff; startup, Ready, heartbeat, identity and exit recovery guards retained
- A persistently displays fresh native basic-attack Effect range until confirmation or a canceling gameplay command
- Basic-attack metadata copied at the existing consumer hook; nearest-to-click attack-move and champion-only policy retained
- Read-only post-battle history/replay candidate capture; saved-result authority explicitly unverified
- No additional native hooks; one controlled foreground battle per process launch remains

## 0.19.0 — 2026-10-01

- Backtick and Mouse 4 independently toggle champion-only direct targeting once per press; simultaneous presses toggle once
- Match/player-bound mode resets on release or identity change; takeover and focus return cannot replay held buttons
- SDK entity is_champion flag filters direct-click and unit-skill cursor hits before ranking and native validation
- Attack-move keeps all eligible enemies and nearest-to-click ranking; ground/direction/self/no-target skills unchanged
- Normal casting keeps aiming through mode changes and snapshots mode at confirmation; quickcast snapshots mode at press
- Persistent CHAMPIONS ONLY HUD indicator; unlearned Q/W/R dim at unlock levels 1/3/5 with required-level label and hint
- No new native hooks or simulation mutations; existing controls, confirmed HUD cleanup and 60-second guard retained

## 0.18.0 — 2026-10-01

- Explicit draw order for HUD background, frames, icons, cooldown shading, text and owned tooltip
- Smaller 600x120 bottom-center panel; portrait, skill/item slots, hover geometry and fallback command mask resized together
- Separate native champion_info_tooltip suppressed during control, including lazy creation; bounded existence and suppression diagnostics
- Native spectator hover resumes on release; existing HUD snapshot policy, controls and twelve native anchors retained
- No new native hook or simulation mutation; 60-second guard retained

## 0.17.0 — 2026-10-01

- Bottom-center native UI champion portrait, level, health, Q/W/R icons/cooldowns, gold, six owned-item slots, K/D/A and CS
- Match/player-scoped SDK scalar snapshots every six ticks; 250 ms running freshness and paused READY retention
- Native skill indices 0/1/2 and explicit champion overrides; item key aliases resolve installed sprite tags
- Scoped spectator toolbar/portrait hiding; known portrait hover surfaces ignore events; native stray tooltips suppressed
- HUD-owned brief skill/item hover hints; HUD/tooltip rectangles block unintended world and camera commands
- Running diagnostics and hit mask shrink together; READY/failure diagnostics remain
- No new native branch or simulation mutation; existing twelve anchors, controls and 60-second guard retained

## 0.16.0 — 2026-10-01

- Shift plus physical Q/W/R press selects persistent normal-cast aiming; key release keeps it
- Battlefield left-click confirms with current camera/cursor; rejection keeps mode without auto-retry
- Gameplay commands cancel aiming; camera/Tab preserve it; confirmation never also issues attack-move
- Generation check rejects requests superseded during native skill validation
- B sends native Return once and clears previous order; idle ticks preserve native recall action
- Selected-actor movement/S/Esc cancellation emits verified native return-cancel event and clears only action 1
- Five existing branch redirects retained; an additional native cancel-event header verified
- Existing quickcast/combat/camera/HUD controls and 60-second guard retained

## 0.15.0 — 2026-10-01

- Q/W/R physical press quickcasts using native skill targeting and validation
- Shift plus held ability previews nominal cast range/aim only; release never casts
- Read-only scalar effect snapshots from native move and attack consumers; 250 ms freshness gate
- Native Attack CALL observer with full fingerprint/header/branch verification and relay forwarding tests
- Rejected casts consumed once; no cooldown/animation queue and no AI takeover on skill rejection
- Targeted skill cursor hits include allies; native per-skill eligibility decides target
- 0.14 camera/combat/HUD controls and 60-second guard retained

## 0.14.0 — 2026-10-01

- Contextual right-click attacks with fresh own-team-visible target snapshots
- A then left-click attack-move; clicked-point priority with 120-world-unit acquisition around click or actor
- Native attack input validation; rejected attacks fall back to native chase; no direct damage writes
- Centered hold-Tab native scoreboard; world command clicks through scoreboard only
- Spectator detail panels hidden during READY/control; visibility and scoreboard layout restored on release
- Client-thread WM_MOUSEWHEEL observer; native zoom step 0.25, limits 0.5-3.0
- Startup, pacing and 60-second guard unchanged

## 0.13.0 — 2026-10-01

- Native Follow uses selected team/lane instead of SDK player ID
- Held Space keeps following unless explicit minimap or captured drag interrupts
- Middle drag captures only on battlefield start and continues across HUD
- Free camera edge-scroll uses 12px outer window bands including HUD
- Hold Tab shows native team info; release/focus loss hides; previous visibility restored on release
- Normal InGame spectator keyboard interception scoped to bound client/view/session; native mouse and default zoom keys retained
- Pacing unchanged; full skill/gold HUD and manual purchases remain future work

## 0.12.0 — 2026-10-01

- (no notes recorded)

## 0.11.0 — 2026-10-01

- (no notes recorded)

## 0.10.0 — 2026-10-01

- (no notes recorded)

## 0.9.0 — 2026-10-01

- (no notes recorded)

## 0.8.0 — 2026-10-01

- unconditional entrance counters and rejection samples
- actual AI/client/native call-stack samples
- Windows OS thread identity
- patch readback and client-side audit
- client-enforced startup deadline
- pause acknowledgement required for Ready
- status reports only acknowledged states

## 0.7.0 — 2026-10-01

- (no notes recorded)
