# Skill preview sources: why the live reader cannot cover Workshop champions, and what can

Research date: 2026-10-16. Status: **read-only investigation and a proposed
design; nothing decided, nothing built.** Written at the author's request
after they rejected the current approach ("having an unread champion every
time a user installs a mod is not a good design"). The decision belongs in
[backlog.md](backlog.md) item 2.

Every finding is tagged as AGENTS.md asks: **Read from the code** (the mod's
source and the game's SDK), **Read from the data** (champion files on this
PC), **Seen in game** (the author's play), or **Guess**.

## 1. What the preview does today

**Read from the code** (`probe/src/skill_preview.rs`,
`native_adapter/windows/preview.rs`, `native_adapter/v<version>/effects.rs`).

When a skill is hovered, the mod walks the live effect objects the game built
for that skill and recognises each by the address of its `apply` method, from
a table per game version (about 30 addresses on 0.6.4: 13 generic effect
types, four hand-written base skills, a "no area" list). A skill whose tree
holds an unrecognised effect and yields no shape is "not understood": for a
direction or position cast a white crosshair is drawn at the aim point
(superseded 2026-10-20, 1.5.0-dev5: an arrow toward the aim for a
direction cast, a small landing ring for a point cast); a
unit-targeted skill draws nothing (the hovered unit is the aim). A skill whose
every effect is recognised and none has an area draws only the range and aim
cue. A decoded shape is kept even when a status effect beside it is
unrecognised.

Declared shapes from the 8 data-defined base champions are baked into the
DLL at build time (`preview_assets.json`, from `tools/read_preview_assets.py`)
and used only as a fallback. Workshop champion files are never read.

Preview-related hooks today: the preview part of the `readers` family (7
anchors per version in `tools/native_profiles/<version>.json`) and the
effect address table in each version folder.

## 2. What Workshop champions are made of

**Read from the data.** 54 `.data_champion` files in 30 Workshop items on the
author's PC (`steamapps/workshop/content/3009300/<item>/…/champion/`), the
same JSON format as the 8 base data champions in the game bundle
(`asset/base/setting/<name>`, extension `data_champion`). All parsed.

- They use about 55 effect `type` names. The ones with a shape: LinearProjectile
  (33 uses), BackToCasterLinearProjectile (1), RangeEffect (79),
  RangeProjectile (6), RangePeriodProjectile (19), LineRangeProjectile (5),
  ParabolicProjectile (8), TargetSplashProjectile (2), Rush (1), RushTime (12),
  RushMoveToBack (5), MoveTo (1), MoveToTarget (9), MoveBack (8), Teleport (3);
  target-only movers Knockback (8), Pull (7), Grab (9). The rest are
  containers (Combine, Delayed, Animated, Animation, WithSelf, SwitchByBuff,
  ThreePhase, RandomTarget) or have no shape (Attack, ApAttack, FixedAttack,
  Heal, Shield, Stun, Airborne, Bind, Charm, Fear, Taunt, Banish, Invisible,
  buffs, Sfx, TargetSfx, ViewEffect, CasterViewEffect, CasterAnimation,
  LoopAnimation, Block*).
- 170 effects are `{"type": "Native", "effect_ref": "<name>"}` with 133
  distinct names (for example `chen_ult_step`, `irene_ult_hit`). In the base
  data champions a `Native` entry carries its parameters (speed, range,
  radius) beside `effect_ref`; in the Workshop files it carries only the name.
  The name occurs in the item's DLL and nowhere else on disk (checked for
  `chen_ult_step`: `arknights.dll` and the champion file only). **Read from
  the data, conclusion:** these are custom effects compiled into the champion
  mod, whose geometry is not in any file.

Consequences for today's reader:

- Shapes in JSON types the reader does not decode (line areas, splash
  projectiles, return bolts, backward dashes, teleports: about 25 uses) show
  as a crosshair although their numbers are in the champion file.
- Shapes inside custom DLL effects (170 uses) show as a crosshair and no file
  can fix it.
- Skills with no area built from unrecognised pieces (stuns, heals, buffs)
  show a crosshair at the aim point instead of nothing; harmless.
- The author is asked to describe each new champion to an agent; that is the
  design failure, not a testing gap.

## 3. What the official interface exposes

**Read from the code** (`sdk/mod-api-stable/src/sim.rs`, `contract.rs`,
`values.rs`).

- Per tick, from any simulation the mod's callback runs in: every projectile
  (`projectile_count`, `projectile_at` → `ProjectileInfoV1 { x, y, caster_id,
  team, is_end }`; no radius), every entity (position, team, level, alive,
  champion/tower/minion flags, shield, attack interval, collision `radius`,
  targetable, buffs, cc, name).
- How a champion mod acts on the match: host functions in the versioned table
  `SimVtableV1`: `spawn_projectile(name, effect_name, ProjectileSpawnV1 {
  caster, team, x, y, radius, speed, move_kind, target or target point,
  penetrate, attack type, casting type, casting target })`,
  `queue_effect(effect_name, attack type, caster, input target, delay)`,
  `play_view_effect(name, caster, input target, range, radius)`,
  `spawn_unit(...)`. The SDK calls these value types "frozen"; the table is
  versioned (`V1`), not an address in the executable.
- Cast events: the mod already sees every athlete's cast through its existing
  skill hook, which logs `PREVIEW TREE viewed=… actor=…` for every actor
  (`combat.rs`). No new hook is needed to know that a skill was cast, by whom,
  and with which effect tree.

## 4. Proposed design (awaiting the author's decision)

Three sources, in this order of value, plus a floor. None asks a player or
the author to describe a champion.

1. **Read the champion files.** At start, read every `.data_champion` the
   game can load: the bundle's base files, each installed mod folder, each
   Workshop item. Decode geometry generically from the JSON vocabulary in
   section 2; skip the no-shape types by rule. This replaces the baked
   `preview_assets.json` and the live reading for data champions. Covers
   every data champion on install day; no hook, no address, no per-patch
   work unless the game adds a type (visible in the files). Open: the game's
   load order and override rules between a Workshop item and the base files
   must be copied, and a champion must be matched to its file by id (the
   interface gives each entity's name).
2. **Base-game champions** (the 60 compiled ones): keep the live reader as it
   is, or replace it with a table generated per version by a tool from the
   executable and the bundle's `champion_info` sheet (ranges live there and
   are read at runtime). Today's cost either way: the readers family and the
   effect table per version, already carried by the migration tools; the
   tool route would remove the preview hooks entirely.
3. **Watch and learn**, for custom DLL effects.
   - **Passive**: after any cast (the existing cast event), watch the
     interface's projectile list for new projectiles by that caster and
     record their path, speed and flight length; store per champion and slot
     under `%LOCALAPPDATA%\LTDirectControl` and use from the next hover on.
     No hook. Does not give a projectile's width or any instant area.
   - **Active**: wrap `spawn_projectile` and `play_view_effect` in the mod
     interface's table, record the radius, speed, range and target of every
     spawn by any mod, and store them the same way. Exact shapes the moment
     any mod uses them. A hook, but on the versioned interface table, with no
     address and no byte pattern. **Guess:** other mods must never notice the
     wrapper; this needs a design check before it is called clean.
4. **Floor**: when nothing is known, draw the range ring and the cast style
   (direction, point or target), which the interface always provides; a unit-
   targeted skill shows its reticle on the hovered unit. No "could not read"
   mark. Optionally, a champion mod may ship a `preview.json` beside its
   champion file describing its custom effects in the same vocabulary; the
   mod reads it if present.

Tradeoff summary:

| Source | Hooks | Per-patch upkeep | Covers |
| --- | --- | --- | --- |
| 1 files | none | none (new game type: small addition) | all JSON shapes, every data champion |
| 2 base | today's readers preview part, or none with a generated table | today's, or a tool run | 60 compiled champions |
| 3a passive | none | none | custom projectiles' path, speed, length |
| 3b active | two interface-table wrappers | none unless the interface is revised | custom projectiles and visual areas exactly |

Recommendation: 1 and 4 first, then 3a; 3b only if custom projectiles prove
to matter in play; 2 unchanged until the rest has landed.

## 5. Open items

1. The game's file load order and override rules (Workshop item over base,
   mod over mod).
2. Whether the interface's `name()` of a champion entity equals the
   `data_champion` id in every case (base, Workshop, renamed).
3. For 3b, how other mods observe the interface table and whether a wrapper
   can be invisible to them.
4. Whether `queue_effect` by name can ever be mapped to a shape without the
   mod's DLL (none found on disk; guess: no).

## 6. Measurement prototype results (2026-10-20)

Build 1.5.0-dev1 (`probe/src/skill_measure.rs`, log only) tried source 3a
in a broadened form, approved by the author on 2026-10-20. The author played
on game 0.6.4 and let background match days run; they noticed no slowdown
(**Seen in game**; the performance capture was off, so no number).

**How it measured** (**Read from the code**): in the match on screen and the
background pre-simulations, a cooldown that jumps up marks a cast. For 2 s
it recorded the caster's projectiles from the interface's projectile list
and every enemy whose health dropped or who gained a crowd-control effect,
first time only.

**Results** (**Read from the data**: `research/skill-measure.json` in the
mod's folder, 155 skills, about 1.28 million casts, nearly all from the
background pre-simulations):

- **Projectile speed: consistent for 41 skills** (90th percentile within
  10 % of the median). Where a file declares it, it agrees: Crossbowman Q
  and W declare range 150 and speed 6.0 per tick; measured 146 / 147 and
  5.9 / 6.0.
- **Projectile range: consistent for 13 skills only.** A ranged champion's
  auto-attacks are projectiles too and the interface's projectile entry has
  no id or kind, so many "ranges" are auto-attack distances (Alchemist Q,
  which declares no projectile, measured 69 against its attack range 70).
- **Width and area from hits: failed.** Across skills the median hit lay
  about 300 units from the caster and hits spread about 500 units (half the
  map). Counting every enemy on the map that lost health in the window
  picked up towers, minions and other fights; the numbers say nothing about
  a skill's shape. **Rejected** in this form.
- **No projectile at all for 25 skills** (Demon, Exorcist, Ghost, Hammerer,
  Inquisitor, Siege Breaker, Squirrel, Vampire, Dokkaebi W): melee and area
  skills, which this method cannot see.
- Speed estimates are the farthest point's distance over its flight time,
  so slow-starting or returning projectiles read low; a few ranges were
  implausible outliers (Crossbowman Q's maximum 1762).

**Conclusion:** measuring play can confirm a projectile's speed and, where
auto-attacks do not interfere, its reach; it cannot supply the widths and
area sizes the preview mostly draws. The author chose on 2026-10-20 to
build source 1 (read the champion files) next and set the measurement
aside. A refined measurement (ignore auto-attack projectiles, count a hit
only where a projectile passed near the enemy) stays possible later, only
for custom projectiles no file describes.
