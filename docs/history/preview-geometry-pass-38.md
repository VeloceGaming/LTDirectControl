# Skill geometry pass, 0.38.0

The user approved scalable family readers after the 0.37 previews confused cast
input with affected shape. This pass does not enumerate every champion or add a
mandatory learning phase. Existing input, damage validation and manual aiming
rules remain in place.

## Sources and ownership

The exact 0.6.2 executable is fingerprinted before native hooks activate. Its
current consumers and constructors establish Arc payload alignment, shared
Combine traversal, RangeEffect placement and shape tags, LinearProjectile sweep
dimensions, and the WhipMasterUlt channel-line payload. The last is a dedicated
effect implementation identified by apply function and layout, not champion name.

Only the registered live worker's selected actor is read. Geometry is copied
into owned numeric values at most once per 100 ms. No native address is retained
by the HUD, and the reader never invokes effect apply methods, simulates damage,
or advances RNG. Tables outside the exact executable are opaque. Tree depth,
nodes, children, shapes, issues and diagnostic changes are bounded. Freshness,
match/actor identity and death clear stale snapshots.

Recognized live footprints are authoritative. Explicit base/enabled-mod JSON
declarations are a fallback when no runtime footprint is recognized. This is not
proof that a file describes a DLL override; logs identify that fallback. Compound
primary effects may contribute multiple areas. Descendants applied to hit units
are not moved back to the original cursor/caster.

## Shapes and placement

Casting input and footprint are independent. Circle, rectangle, finite segment,
projectile corridor and DirDot sector use world dimensions. Width in the shared
line predicate is a distance from its center line, not a diameter. DirDot's old
field name `range` is actually a cosine threshold scaled by 1000. Its facing is
caster to area center; a degenerate caster-centered facing remains unresolved.
RangeEffect Forward offsets only native unit/point inputs, matching the current
consumer. Self-centered effects suppress misleading cast-reach rings.

Whip Master W's current base constructor produces a RangeEffect circle, radius
40 and center offset 20 world units. Its cone-like animation is not sufficient
evidence to draw a cone. Runtime geometry and user damage observations must
confirm the active setup. Whip Master R supplies live projectile length/width
and can show a corridor through a unit target. Archangel R's explicit
AroundCaster circle is distinct from its targeting range.

## Limits

This is a nominal effect footprint, not a guaranteed-hit or movement prediction.
Unit collision-radius padding, future movement, delayed timing and already-live
projectiles are not simulated. Dedicated/foreign effects, branches dependent on
buffs, and unsupported wrappers may retain a reach/aim guide or partial geometry.
LineRangeProjectile's separate width/placement semantics remain unresolved rather
than assuming they match the shared Line enum. Unsupported families are logged
for targeted follow-up. Learning remains an option for opaque behavior, with
build/data fingerprints and uncertainty if added later.

Current static evidence is linked in [investigation notes](preview-geometry-investigation.md).
Additional constructor/consumer captures: `research/preview-combine.asm`,
`research/preview-whip-r-construction.asm`, `research/preview-whip-r-effect.asm`.
The build record verifies effect-family prologues and representative table headers
in addition to the executable hash and existing anchors. Native rendering and
gameplay remain pending the [user checklist](../../local/testing/TESTING-38.md).

## Build verification

183 probe and 18 core tests passed. Core/probe lint checks with warnings denied,
format checks and the release build passed. 55 native instruction/header anchors,
ABI 6/null-host rejection, 42 unchanged glyph assets and 44 ZIP member
fingerprints were verified. 0.38.0 was installed with the game closed after a
verified rollback copy of 0.37.0. These checks do not establish native preview
accuracy; the game test remains pending.
