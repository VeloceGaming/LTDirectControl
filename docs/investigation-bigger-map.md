# Investigation: a bigger battlefield (2026-10-23)

The question (the author): can the mod, or a standalone mod, offer a bigger
battlefield that reuses the existing map scaled up (same layout, lanes,
jungle and objectives, everything proportionally larger); how, at what cost,
which home fits, and does it serve the mod's direction?

This is a read-only study. Everything here is **read from the code** of
games 0.6.5 (`research/game-builds/0.6.5`) and 0.7.0 Beta 2
(`research/game-builds/0.7.0b2`) unless a line says **Guess** or
**Rejected**. Nothing is seen in game. Addresses are RVAs of the version
named; "file offset" marks the few places read straight from the file.

## Verdict

Not worth it. The map's *objects* (towers, camps, lanes, bases, fountains,
the wall and bush grids) are data a mod can edit through the official mod
interface's `StableMapCustomizer` hook. The map's *size* is not data: the
world is 960 000 by 960 000 units in 30 by 30 cells of 32 000 units, and
those numbers are literals in dozens of functions of both executables
(section 2), while the shipped line-of-sight and path tables are baked for
exactly that grid (section 1). A data-only mod cannot enlarge the world; a
native scale factor is a patching campaign to be repeated on every game
patch; and any map change applies to every match the game simulates,
including the ones the player only watches and the pre-simulation, which
contradicts "AI-versus-AI stays vanilla". Recommendation: build it in
neither home (section 4).

## 1. Where the battlefield is defined

### 1.1 Two settings assets in the game's bundle

**Read from the code (`bundle.game_data` of 0.6.5 and Beta 2; both files
identical byte for byte in both versions).**

- `asset/base/setting/game_setting` (17 863 bytes) is JSON: `width` and
  `height` 960 000, `visible_distance` 130 000, `tick_per_second` 60,
  `champion_radius` 10 000, and the minion, jungle, tower and nexus
  definitions with ranges in absolute units (tower `range` 75 000, ranged
  minion 35 000, epic and serpent 80 000, `minion_wave_setting.exp_range`
  150 000, `epic_minion_buff_range` 140 000). `death_match/` and
  `single_lane/` carry their own copies. The mod interface exposes it as
  `SettingTargetV1::GameSetting`: the server side may read and write it
  ("affect matches created afterwards", `sdk/mod-api-stable/src/server.rs`),
  the client side reads only.
- `asset/base/setting/map_setting` (1 451 980 bytes, the same in all three
  modes) is a binary serialisation, not JSON. It holds, in order:
  `visible_view`, a 30 by 30 by 30 by 30 table of bytes (810 000 bytes;
  entry `[a][b][c][d]` is 1 when cell (a, b) sees cell (c, d)); a packed
  table of 810 000 four-bit entries (405 000 bytes; values 0 to 7 and 15,
  read as the next step from each cell towards each other cell, 15 for
  none; Guess on the meaning of the digits); and a packed table of 27 000
  four-bit entries (13 500 bytes; 900 cells by 30, meaning not established).
  The executable's serialisation names are `MapSetting`, `visible_view`,
  `path`, and a packed struct with `data`, `width`, `height` (0.6.5 file
  offset `0x3ad14f0`). The interface's own comment says it is
  "offline-baked visibility/path data and is deliberately not exposed"
  (`sdk/mod-api-stable/src/values.rs`, `SettingTargetV1`).

**Read from the asset; a correction to `investigation-vision-ward.md`
section 1.** That document says the sight radius is baked into
`visible_view`. It is not: the table is pure line of sight. From the shipped
file, the corner cell (0, 0) sees 214 cells, the farthest baked view is 41
cells (corner to corner), the average cell sees 119 of the 900, and the
table is not symmetric (a sees b does not imply b sees a). So the 130 000
unit `visible_distance` is applied elsewhere. **Guess:** when the world
builds its per-cell lists at world `+0x5d20` (described in the vision ward
document) it intersects this table with the radius. The function that builds
those lists was not identified; the two functions besides the sight builder
that touch both `+0x5d20` and `+0xd068` (0.6.5 `0x16499a0`, `0x1329bb0`) are
Debug formatters, not builders. The vision ward document is left as written;
this paragraph is the cross-reference.

### 1.2 The map geometry is compiled into the executable

**Read from the code (0.6.5 and Beta 2).** The source path
`game-core\src\simulation\map_def.rs` is in both executables, with the
serialisation field names `MapDef { walls, bushes, regions, region_adj,
region_dist, region_centers, is_line_region, lane_regions, nexus_pos,
fountains, lanes, towers, camps }`, `TowerDef { ty, pos }`,
`LaneDef { path }`, `LaneRegionSeq { line, team, seq }` and `CampDef`
(0.6.5 file offset `0x3d403e8`). No JSON map document exists in either
executable or bundle: `"nexus_pos"`, `"region_centers"` and `"fountains"`
have zero hits; the one `"walls"` hit in the bundle is the gaming-house
decoration setting.

**Read from the code (0.6.5).** A 30 by 30 table of 0/1 values stored as
900 eight-byte integers sits at file offset `0x3cc2710`, 90 cells set in a
scattered pattern (walls or bushes; which was not established). It is the
only 900-entry 0/1 table in the file with between 5 and 95 percent ones,
as bytes, four-byte or eight-byte values. Six coordinates `288 000,
288 000, 672 000, 672 000, 480 000, 480 000` sit at file offset `0x3baafc0`
(**Guess:** objective and centre positions). The map-setting loaders that
reference the `asset/base/setting/map_setting` string (0.6.5 `0x2520e0`,
`0x97b410`, `0xae8000`, `0xbba800`, `0xc00e70`, `0xc67a40`, `0xcbc790`,
`0xcbeab0`, `0xcbfde0`, `0xce4e60`) carry no other coordinate-like
immediates apart from 480 000 three times in `0xc00e70`.

### 1.3 The official hook: `StableMapCustomizer`

**Read from the code (`sdk/mod-api-stable/src/traits.rs:319-332`,
`json_doc.rs`, both versions' SDK copies).** The interface hands a mod the
"freshly built map of a match (tower/camp/lane/nexus/fountain placement,
wall/bush grids) through a JSON document view. Runs at match creation for
every game mode; must be deterministic." Its own note: "offline-baked
pathfinding/visibility stays based on the original geometry, so large
wall/bush rework has limits — object placement (towers, camps, lanes,
nexus, fountains) is fully live." `customize(mode, doc)` receives only the
mode (`Moba`, `SingleLane`, `DeathMatch`) and the document: no match id, so
a mod cannot tell the viewed match from a background or pre-simulated one.
A bad edit voids the whole edit pass when the host applies the document.
This mod already registers a read-only customizer (`probe/src/map_path.rs`
`MapObserver`, registered in `probe/src/lib.rs`), which reads `walls` as a
30 by 30 array.

### 1.4 The grid constants are code, not data

**Read from the code (0.6.5 sight builder `0x15b6f30`; Beta 2
`0x169f320`).** The builder compares positions with the literal 960 000
(`cmp r8, 0xea600`, twice), divides by 32 000 with compiled magic numbers
(`imul 0x10624dd3; shr 43`, and `shr 8; mul; cmp rdx, 0x1d`), clamps the
cell to 29 (`0x1d`, nine times in 0.6.5, eleven in Beta 2), uses a row
stride of 30 (`0x1e`) and a per-team grid of 0xe10 bytes (900 by 4). None
of these come from `game_setting`. The mod's own code assumes the same
numbers (`map_path.rs` 30 and 32 000; `camera.rs` and `minimap.rs`
960 000).

**Read from the code (whole `.text` section, by function from the unwind
table).**

| Constant | 0.6.5 | 0.7.0 Beta 2 |
| --- | --- | --- |
| `0x10624dd3`, the divide-by-32 000 magic | 236 hits in 75 functions | 222 hits in 89 functions |
| 960 000 as a 32-bit immediate | 53 hits in 28 functions | 53 hits in 26 functions |
| 480 000 as a 32-bit immediate | 7 hits in 3 functions (`0xc00e70`, `0x158bd30`, `0x158e8f0`) | 7 hits in 3 functions (`0xb67630`, `0x15b2aa0`, `0x15b5660`) |
| 32 000 as a 32-bit immediate | 452 hits in 130 functions (not all are the cell size: skill ranges use it too) | 435 hits in 129 functions |
| 16 000 (half a cell) as a 32-bit immediate | 364 hits in 104 functions | 375 hits in 105 functions |
| 130 000 as a 32-bit immediate | 2 hits (`0x1018510`, `0x1699320`) | not counted |

The 0.6.5 functions holding 960 000: `0xd40740`, `0xd642e0`, `0xde4c80`,
`0xde7d70`, `0xdeaba0`, `0xdec580`, `0xdecd70`, `0xdf47e0`, `0xdfaaf0`,
`0xe09670`, `0xe4a890`, `0xe8faa0`, `0xed0ab0`, `0xeebd10`, `0xf0f5b0`,
`0xf48600`, `0xf4aa50`, `0xf51eb0`, `0xf77ff0`, `0xf7ba70`, `0xf84830`,
`0xf94700`, `0x130dd80`, `0x158e8f0`, `0x15b6c10`, `0x15b6f30`, `0x1815800`
and one site outside any unwind entry. The 0.6.5 functions holding the
magic divisor: `0x51a30`, `0x6b2e0`, `0x6e900`, `0xfe620`, `0xff2c0`,
`0x9bdfb0`, `0xb8ed60`, `0xd789e0`, `0xde4c80`, `0xdecd70`, `0xdf0c20`,
`0xdf47e0`, `0xdfac10`, `0xee2120`, `0xef6250`, `0xf0d520`, `0xf0f5b0`,
`0xf25ad0`, `0xf30320`, `0xf309e0`, `0xf38320`, `0xf48600`, `0xf5ffc0`,
`0xf60f60`, `0xf61ee0`, `0xf62ca0`, `0xf63a60`, `0xf64820`, `0xf655e0`,
`0xf66630`, `0xf673f0`, `0xf682c0`, `0xf69080`, `0xf6c680`, `0xf6d6d0`,
`0xf6e990`, `0xf6fc30`, `0xf70c80`, `0xf7b400`, `0xf92e20`, `0xf9fb30`,
`0xfa4190`, `0xfa9e10`, `0xfb1000`, `0xfb26d0`, `0xfb5990`, `0xfcb590`,
`0xfdd9e0`, `0x1013ff0`, `0x10ed500`, `0x1279b90`, `0x127aae0`, `0x127b8a0`,
`0x12fbb00`, `0x130dd80`, `0x158f320`, `0x158fbf0`, `0x15a0fc0`, `0x15b6f30`,
`0x15ce3c0`, `0x175f5c0`, `0x176ab80`, `0x176fea0`, `0x18a79a0`, `0x18fee20`,
`0x1c75890`, `0x1d8e5f0`, `0x1dd42f0`, `0x1df14b0`, `0x1e45660`, `0x27ed1c0`,
`0x288f840`, `0x2a7f850`, `0x2ba1110`. The Beta 2 960 000 functions:
`0xe34ad0`, `0xe58b40`, `0xee3a10`, `0xee5ed0`, `0xeed3f0`, `0xf30140`,
`0xf35d40`, `0xf39160`, `0xf3bca0`, `0xf434e0`, `0xf499f0`, `0xf58f20`,
`0xfb2980`, `0xfbb170`, `0xfbfef0`, `0x101a1d0`, `0x101b780`, `0x1035cc0`,
`0x10c9410`, `0x11043a0`, `0x13b2a60`, `0x15b5660`, `0x169f000`, `0x169f320`,
`0x16b7d00` and one site outside any unwind entry. The lists are a
re-check aid; the roles of these functions were not established.

### 1.5 Art, minimap and camera

**Read from the code (bundle, both versions).** The 5v5 battlefield art
(`asset/base/aseprite_resources/ingame/5v5/background_5v5`, `wall_5v5`,
`wall_5v5_front`, `wall_shadow_5v5`, `bush_5v5`, `bush_shadow_5v5`) is
1280 by 1280 pixels each; `minimap_5v5_bg` is 320 by 320; the single-lane
art (`ingame/background`, `wall`, `bush`) is 1320 by 900. The renderer works
in thousandths of a world unit (the mod writes the camera centre as
position / 1000, `native_adapter/windows/view.rs`; `camera.rs` converts the
same way) on a 2048-square Game texture (`docs/history/camera.md`), so the
world is 960 pixels across. How a 1280-pixel sheet is placed on a 960-pixel
world was not established (**Guess:** drawn at 0.75, or with a 160-pixel
margin).

**Read from the code (0.6.5, Beta 2).** The literal 960.0 as a float appears
in four 0.6.5 functions (`0xcda710`, `0x23a1e00`, `0x2766d20`, `0x27e5aa0`)
and six Beta 2 functions; every site read is screen-space layout (half of
1920), not map size. No native camera clamp expressed as a map-size literal
was found. The mod's minimap maps 960 000 units onto its 352-pixel square
with its own constants (`minimap.rs`, `camera.rs`) and patches the native
minimap's operands (`v065/identity.rs` `MINIMAP_PROFILE`).

## 2. What "scaling up" touches

| Thing | Where it lives | Kind |
| --- | --- | --- |
| Map bounds 960 000 | `game_setting` `width`/`height`, and literals in 28 (0.6.5) or 26 (Beta 2) functions | data and code |
| Sight grid 30 by 30, cell 32 000 | literals in the sight builder, the path finder, the navigation builder and the rest of the 75 to 89 functions carrying the divisor | code |
| Baked line of sight and path | `map_setting`, 30 by 30; the same layout stays valid for a scaled map only if the cell size scales with it | data, not exposed |
| `visible_distance` 130 000 | `game_setting`; two literal sites | data (and code) |
| Towers, camps, nexus, fountains, lane paths, regions | `MapDef`, compiled constants, editable per match by the customizer | data at run time |
| Walls, bushes | `MapDef` 30 by 30 grids, editable by the customizer; the baked tables do not follow | data, limited |
| Skill ranges, projectile speeds, durations | champion data in absolute units (for example `range` 132 000, `speed` 2 800) | data per champion; do not scale |
| Movement speed | champion and minion `move_speed`, units per tick | data; does not scale |
| Collision | `champion_radius` 10 000 in `game_setting`; wall cells of 32 000 | data and code |
| AI pathing | run-time path finder over the 30 by 30 wall grid (`game-core\src\simulation\path_finder.rs`) plus the baked `path` table; AI side `game-ai\src\path_finder.rs`, `path_field.rs`, `free_dist.rs` | code and baked data |
| AI decisions | the `game-ai` crate (`lane_positioning.rs`, `position_eval.rs`, `plan\...`, `rl.rs`), a reinforcement-learning policy for death match (`death_match/ai_policy.rlp`, header `RLP3`) and `base_network` (a float table) | code and trained data |
| Pre-simulation | the same simulation; the customizer runs "at match creation for every game mode" | as above |
| Spawn and respawn | `fountains`, `nexus_pos` in `MapDef` | data |
| Renderer | 1280-pixel art, 2048 Game texture, 1/1000 units per pixel | code and art |
| Minimap | native operands (patched by the mod) and the mod's 352-pixel constants | code |

**Guess.** The AI's distance thresholds (`attack_range_margin`,
`commit_dist`, `explicit_min_range` and the like, all field names in the
executable) and the trained policy assume the current distances; on a
bigger map with unchanged champion ranges the AI keeps working but its
spacing and timing are off before anything else breaks.

## 3. Three approaches

**(a) Data only. Rejected for the stated goal.** The customizer plus
`game_setting` can move objects inside the 960 000 world but cannot enlarge
it: a position beyond 960 000 lands in cell 29 for sight, path and
navigation. A relative variant (keep the map, shrink every speed, range and
radius by the same factor through `game_setting` and champion data) is
possible in principle, but it changes every athlete's stats in every match
and the sprites keep their size (**Guess** on the visual result).

**(b) A native scale factor at load.** Patch every literal in section 1.4
on both versions (roughly 100 to 150 instruction sites per version; the
magic divisions must be recomputed, not just replaced), scale `MapDef`
through the customizer, keep the 30 by 30 baked tables (valid because the
layout is unchanged), then deal with champion ranges and speeds that stay
absolute, the AI's thresholds, the art's scale and the mod's own camera,
minimap and route code. Estimated four to six stages, each with its own
in-game test, and the whole site list re-found on every game patch. This
is not a hook that replaces one instruction; it is a constant-patching
campaign.

**(c) A run-time transform. Rejected.** Positions are integers read
everywhere in the simulation (sight, path, collision, projectiles, AI);
there is no single place to scale them.

**What breaks first (Guess).** The AI (section 2), then replays (a replay
is a re-simulation, `SimOriginV1` kind `ClientReplay` = 4 in the SDK; a
vanilla match's stored replay re-simulated with a changed map differs, and
the other way round), then Workshop champions whose ranges assume the
current distances, then the mod's own camera, minimap and route code.

## 4. Standalone mod or inside this mod

A standalone mod reaches players who do not use direct control, but has
none of this mod's fingerprint checks, relays and build verifier, all of
which approach (b) needs. Inside this mod the infrastructure exists, but the
feature works against "play as one athlete" and "AI-versus-AI stays
vanilla" (`AGENTS.md`; the customizer cannot be limited to the player's
match, section 1.3), and doubles the per-patch upkeep. Recommendation:
neither. If the author wants the *feel* of a larger field, the native zoom
(0.5 to 3.0, `docs/history/camera.md`) is the only lever that costs
nothing.

## 5. Direction check

- Play as one athlete: a bigger map changes the arena, not the athlete's
  role. Neutral at best.
- Honest mechanics: unaffected by the map itself; the relative variant of
  (a) changes every athlete's stats and fails it.
- No input delay: the customizer runs once at match creation on the
  simulation thread; nothing per tick. Fine.
- AI-versus-AI stays vanilla: fails. The hook runs for every match of every
  mode, including background matches and the pre-simulation, and cannot
  tell them apart.
- Threads: no per-tick work; the "Threads: the rules" section of
  `docs/modding.md` is not touched.

## 6. Open questions (only an in-game test answers)

1. Does the game clamp the camera to the map, and where?
2. How does the renderer place the 1280-pixel sheets on the 960-pixel
   world?
3. Where is `visible_distance` applied to the baked line-of-sight table
   (section 1.1)?
4. Does a replay re-simulate with mods loaded, so a customized map replays
   consistently, or does it diverge?
