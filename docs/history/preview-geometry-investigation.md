# Automatic preview geometry: feasibility investigation

Investigation approved after the 0.37.0 preview feedback. No gameplay code was
changed, built, or installed. Static analysis does not verify native rendering.

## Conclusion

Read geometry by effect family, independently of input type. This can cover
compiled and data-defined champions without a handwritten table for every
champion. A universal geometry getter has not been established. Custom native
effects remain exceptions; passive match observations cannot reliably establish
their exact boundaries.

## Current executable evidence

Target: the already verified 0.6.2 executable, SHA256
`15df9eb3b6915cdcc4c2ebb3b7f5fa232b563c4cdd32208581634817b71adc23`.
Addresses below are RVAs, not absolute runtime addresses.

The shared shape predicate at `16b2900` dispatches through `3b90344`:

| Tag | Branch | Operation established from current instructions |
| --- | --- | --- |
| 0 | `16b293e` | Circle: squared distance versus squared combined radii |
| 1 | `16b29cb` | Finite line segment: clamped projection and perpendicular distance |
| 2 | `16b2962` | Axis-aligned rectangle: half width/height plus target radius |
| 3 | `16b299d` | Cone: radial check, then direction dot product against a scaled cosine threshold |

The cone field at shape `+10` is used as a threshold divided by 1000. Older SDK
debug information calls that field `range`; interpreting it as a distance would
be wrong. Current consumers establish its mathematical role.

`16aaba0` is a projectile collision consumer. The range-effect filtering path
`15cc450 -> 162bbb0 -> 16b2900` also reaches this predicate. Call references are
saved in `research/preview-shape-callers.json`.

For the shared LinearProjectileEffect implementation (`apply=18143b0`), nine
read-only data references locate effect tables. Three inspected tables have size
152, alignment 8, matching shared implementations:

- `+118 -> 13aaad0`: optional linear speed, reading object `+78`.
- `+128 -> 13aaab0`: circle projectile radius, reading shape `+8` when tag is 0,
  returning zero otherwise.
- The first shape fields and travel range are used by the projectile creation
  consumer. The old debug layout is consistent with these current accesses.

These are native internal methods, not new stable SDK guarantees. Before a
runtime reader calls them, validate current callers/ABI, table identity, object
lifetime, and the registered worker scope. Neither native pointers nor vtables
should escape into the HUD. Copy owned numeric descriptions instead.

## Whip Master W: appearance is not proof of collision geometry

The current WhipMasterSkill2Action table at `3b73760` has effect constructor
`15cfeb0` at `+98`. Its result is a CombineEffect (`3b76348`, size 24) containing
a RangeEffect (`3b76a90`, size 96), plus a visual effect.

After accounting for the two Arc reference counts, the RangeEffect constructor
sets forward application, offset equal to action range/2, shape tag 0, and
circle radius equal to action range. The range-effect consumer computes its
forward center from target direction. Thus the compiled base W has a forward
circle, not a cone collision shape. Default action range is 40000 internal
units; runtime patches may change it.

This does not establish the user's effective runtime object after all enabled
mods, nor invalidate their observation of a cone-like animation. A next-build
numeric capture should confirm the active family/shape and dimensions. Do not
replace this finding with an assumed cone from appearance alone.

Whip Master R uses a custom WhipMasterUltEffect. Older debug information exposes
projectile range and width, but its dedicated behavior still needs a current
consumer mapping. A target click and a line affecting other units can coexist;
the renderer must not discard area geometry just because the input is a unit.

## Declaration survey

`research/preview-declaration-survey.json` surveys base `.data_champion` records
and declarations in currently enabled mod roots, merged by champion ID. This
is file-level evidence, not proof of runtime overrides made by DLLs.

- 8 base data-defined champions; 27 champions after enabled declarations merge.
- 81 skill slots; 37 contain explicit shape declarations somewhere in their
  effect tree; 13 contain Native effects. These categories can overlap.
- All 72 explicit shape occurrences in this particular sample are circles.
- Frequent shared families include RangeEffect, LinearProjectile,
  RangePeriodProjectile, Combine and Delayed.
- Geometry in a secondary impact is not necessarily a primary casting footprint.
  Traversal must preserve parent placement, timing and branch conditions.

Archangel R explicitly declares AroundCaster with radius 10000. It needs no
learning to establish that declaration's center. It also has cast range 45000;
that is a different quantity and should not become the advertised burst radius.

The existing 0.37 parser and renderer omit placement variants and reject many
wrappers, and the renderer gates line geometry on direction-cast input. Those
are implementation gaps, not missing game data.

## What the reference mod's learning establishes

Read-only inspection of the installed reference DLL's strings, its log, and
`data/tft2_pov/profiles.json` found saved observations for accepted input types,
target categories, reach by level, cooldown, movement, buffs, and effects on other
units. Profiles include data fingerprints. The log reports relearning changed
abilities. Saved skill-mode configuration is also present.

There are no explicit cone-angle or line-width fields in the inspected profile
schema. This does not prove the mod never learns geometry elsewhere. It also
does not establish how its probes are scheduled or that every saved observation
came from an isolated simulation. Do not attribute its preview algorithm or
authorial intent from these files alone.

## Approved implementation scope

Implemented for 0.38.0; see [pass notes](preview-geometry-pass-38.md). Native
testing is still pending.

1. Separate cast input, geometry, placement and dimensions into an owned preview
   description. Allow several footprints when justified by a compound effect.
2. Read enabled explicit declarations with wrapper/placement semantics and use
   current runtime numeric data for recognized native effect families. Begin
   with Combine, RangeEffect and LinearProjectile; represent circle, finite
   line, rectangle and cone without champion-specific visual guesses.
3. Render self areas without targeting brackets or misleading cast-reach rings;
   permit target-oriented lines and position-oriented forward areas.
4. Capture a bounded per-skill family/shape summary, including unsupported
   reasons, for the selected actor. Whip W/R and Archangel R are validation cases,
   not the entire supported set.
5. Reserve learning for opaque behavior. Do not add a long mandatory learning
   phase to startup. Cached observations should include a data/build fingerprint
   and uncertainty, and should not overwrite exact dimensions from game data.

Unknown effects retain a limited guide. No geometry claim should imply guaranteed
hits, predict target movement, or alter native damage/target validation. Runtime
tests with the user are required to confirm effective geometry and rendering.

## Saved static evidence

- `research/preview-runtime-families.json`
- `research/preview-shape-predicate.asm`
- `research/preview-shape-consumers.asm`
- `research/preview-shape-callers.json`
- `research/preview-whip-construction.asm`
- `research/preview-range-effect.asm`
- `research/preview-range-filter.asm`
- `research/preview-effect-consumers.asm`
- `research/preview-geometry-feasibility.asm`
- `research/preview-declaration-survey.json`

Old SDK records came from `research/hud-core-full.s.lto.hud-player-old.s`.
They provided names/layout hypotheses; current instructions supplied the checks
described above. No foreign mod was loaded or modified by this investigation.
