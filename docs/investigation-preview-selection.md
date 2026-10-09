# Investigation: skill previews and hover/selection accuracy

Date: 2026-10-09. Read-only: source, logs, game bundle and executable
strings. Nothing was changed. Backlog item 5 and the selection pass from
[investigation-pass-65.md](investigation-pass-65.md).

## Part 1: skill previews

### How previews are built today (`skill_preview.rs`, `native_preview.rs`)

1. **Live reader** (preferred): walks the controlled champion's live skill
   effect tree (`Arc<dyn EffectType>` on the entity) and decodes the payload
   of effect types it recognises by their apply function. It recognises
   **four**: Combine, RangeEffect, LinearProjectile and WhipLine. Everything
   else is logged as `opaque apply=<address> size=<bytes>`.
2. **Declared tree** (fallback): the champion's JSON effect tree. Only **8**
   base champions have one in `preview_assets.json` (alchemist, astrologer,
   crossbowman, harpooner, nightmare, sand_mage, spellbreaker, strongman),
   plus Workshop champions from their own files.
3. **Limited guide**: when neither gives a footprint, only the cast range and
   an aim line are drawn.

### Evidence

- The preserved logs hold 18 preview lines covering 9 distinct skill slots;
  5 of the 9 fell back to the limited guide. Most native trees contain opaque types, for example
  `native Combine` → `opaque apply=1990300 size=64` + `opaque apply=18e2630`.
- When something was decoded, it may be a minor sub-effect: one ultimate
  drew only a 10-unit circle at the caster, taken from a small RangeEffect
  inside a larger tree whose other parts were opaque (likely, not yet
  confirmed).
- The executable lists the whole generic effect schema: `DataEffectDef` has
  about 60 variants with their field names (`RangeEffect {target, apply_type,
  effects, shape}`, `LineRangeProjectile {width, length, …}`,
  `RangePeriodProjectile {period, first_delay, …}`, `ShrinkingBarrier
  {start_radius, end_radius, …}`, `ParabolicProjectile {travel_time,
  range_effect_name, …}`, …). Runtime structs carry matching names
  (`RangeEffect`, `ApplyInProjectileEffect`, `RangePeriodProjectileEffect`,
  `TauntEffect`, `ShieldEffect`, `CombineEffect`, `WithSelfEffect`, …).
- Built-in champions have per-champion `…SkillAction` code (for example
  `PythonessSkillAction`, `ShieldBearerSkillAction`), but at runtime those
  actions still produce trees of the same generic effects, which is what the
  live reader sees.

### Why so many previews are wrong or missing

1. Only 4 of the roughly 60 generic effect types are decoded.
2. Only 8 base champions have a declared fallback.
3. Choosing which footprint to show, and where to place it, is decided per
   type. With most types opaque, the reader sometimes shows a minor
   sub-effect instead of the main hit area.

### Proposed general solution (no per-skill hand fixes)

Decode the generic runtime effect types, not individual skills:

1. **Inventory (diagnostic build).** At each match start, log the effect
   tree types (apply address, size, nesting) of all ten champions' Q/W/R.
   After a few matches this lists every type actually used. Data-driven and
   Workshop champions, whose JSON names are known, pair addresses with type
   names automatically.
2. **Layouts (static reverse engineering).** For each geometry-relevant type,
   read its field offsets from the game's own code, preferably its `Debug`
   formatter, which reads every field in order. That is roughly 15 types:
   the projectiles, range and period effects, barrier, rush/move, teleport
   and auto-target types.
3. **Placement rules per type** (corridor from the caster, area at the aim
   point, around the caster, travelling then exploding, …), plus a rule for
   which footprint is the main one when several exist.
4. Test with the user per champion. Unknown types keep the limited guide.

This covers built-in, data-driven and Workshop champions at once.
Hand-coded actions whose geometry lives only in their own code stay on the
limited guide.

Needed from the user: a few examples of wrong previews (champion, key, what
is shown versus what the skill does), to check the hypotheses and pick the
first types to decode.

## Part 2: hover and selection accuracy

### How picking works today (`combat.rs`, `sprite_picking.rs`, `movement.rs`)

- Each unit gets a fixed rectangle derived from the size of its idle/run
  animation frames (atlas frame boxes), with padding. Atlas frames have no
  offset data, so the rectangle is centred on the unit's position with a
  guessed foot allowance.
- Unit positions come from the simulation (`Simulation::think` → hover
  units), not from what is drawn this frame.
- Priority: hostile before friendly, champion before non-champion, body
  before padding, then distance to centre.

### Limits

1. The rectangle is not the body: empty corners, and the real sprite anchor
   and facing are unknown.
2. Attack and skill poses (lunges, weapons) are not followed.
3. Moving units: the box follows the simulation position, while the screen
   shows the viewer's interpolated position, so it can lead or lag slightly.
4. The priority order is not the one agreed in investigation-pass-65.

### Key finding: the exact drawn sprite is available

The outline hook (`native_adapter/windows/outline.rs`) already wraps the
game's generic unit renderer, which is called for every unit body each frame
with that unit's id (`EntityView`, id at `view-8`). Its sprite draw commands
hold the frame's source rectangle in the atlas (`+0x68`) and the draw
position (`+0x78`). So the mod can know, per unit and per frame, the exact
pose, facing and on-screen placement the player sees, instead of estimating.

### Proposed plan

1. **Diagnostic build:** record each unit's sprite commands (id, source
   rectangle, draw position, any scale or flip field) for a few frames and
   compare them with our camera projection and the unit's position. This
   confirms the coordinate space and the remaining command fields.
2. **Picking on the drawn sprite:** use the live sprite rectangle, combined
   with the stable idle envelope so thin attack poses do not shrink the
   target, plus small forgiveness and hover stickiness. Optional later
   step: test the cursor against the sprite's actual pixels (alpha from the
   installed atlas, slightly widened) for body-shaped picking.
3. **Priority order** as agreed: exclusions → side (skill-dependent) → own
   champion last → structures last → body over margin → stickiness →
   size-relative depth → front-most, then ID.

Units drawn by champion-specific views (extras such as weapons) still go
through the same renderer for their body, so the body is always covered.

## Update 0.73: named effect types

Pairing logged trees with the JSON trees of Workshop champions (Leef's
Variety, the `cf_` pack) and the 8 data-driven base champions names most
effect types by apply address (`size` in bytes):

| Apply / size | Type | Preview |
|---|---|---|
| 1adaf00/24 | Combine | follows children |
| 1adb160/32 | Delayed | follows children |
| 16974f0/96 | RangeEffect | area at caster or ahead |
| 18bf7a0/152 | LinearProjectile | corridor; end effects where it ends |
| 14294c0/152 | RangePeriodProjectile | area at its placement |
| 1489140/120 | RangeProjectile (0.73) | area (shape words 0-5) at its placement |
| 1612510/168 | ParabolicProjectile (0.73) | landing area (words 0-5) at the aim; landing effects (words 15-17) followed |
| 15b28f0/56 | RushTime (0.73) | corridor speed x ticks (words 3, 4), body width |
| 18bf2d0/56 | SwitchByBuff (0.73) | follows the branch without the buff (Arc at words 3-4) |
| 1af4470/40 | MoveToTarget (0.73) | movement onto the target |
| 1698550/56, 1adb2f0/40 | Rush, MoveTo | dash corridor, movement |
| 1281860/296, 1279800/288 | AddCasterBuff, AddBuff | none (buffs) |
| 14e0490/72 | TargetProjectile | none (single target: brackets) |
| 18c1480/24, 18e40b0/24, 18e2630/48, 18c6c20/24, 19de6f0/32 | Sfx, TargetSfx, ViewEffect, CasterViewEffect, CasterAnimation | none |
| 18c76c0/72, 18d36b0/64, 1990300/64 | Attack, ApAttack, Heal | none |
| 1698810/16, 19df350/8, 14dfcf0/8, 14f8d10/8 | Knockback, Fear, BlockAttack, BlockMoveSkill | none |
| 1ade340/40 | Native (hand-coded skill) | cast range only |

Replaying the logged trees (57 skills, about 19 champions): 23 had a
footprint before 0.73, 33 after. Still without one: self-buffs and
single-target skills (correct), and base-game skills built from types no
JSON names yet (Gunner Q/R, Bard Q/R, Exorcist R, Cavalry Knight R, an
unknown 1ad9600/48 inside Executioner W). The drawing options for a new
preview design are in [preview-drawing.md](preview-drawing.md).
