# Aim and preview pass 0.37.0

Approved scope: cursor aim accuracy and skill previews, using the user's supplied
HTML/SVG designs. Full HUD and cursor implementation remains a separate pass.

## Evidence from 0.36

The user verified diagonal walking, improved cast responsiveness and absence of
phantom Q counts. At 05:25 Ninja Q went upward into Jiangshi despite upper-left aim.
The preserved log is `research/test-36.log`. For actor 468, trace 1366:
caster `(704680,424207)`, cursor world `(657389,298211)`, native direction
`(-47291,-125996)`. The native Q consumer receives those exact signed values;
the immediate cast queue is empty and native cooldown acknowledgement succeeds.
This excludes our initial target resolver as the place that changes that input.

Read-only disassembly of the exact installed 0.6.2 executable found the later
skillshot correction routine at RVA `15c6f30`, called at `15b9b18` from entity
update after a skill's launch target has been copied from the action/queue.
Seven Windows x64 arguments are RNG, sim data, sim table, navigation, actor ID,
Effect Arc pointer and mutable 24-byte InputTarget. The caller ignores the return.
The routine queries linear projectile speed at effect vtable `+118`, scans living
enemy champions near the aim line, evaluates native athlete stats/RNG, predicts
target motion and writes point/direction coordinates at `15c77f5/15c7811`.
This is distinct from the SDK effect's `auto_target` flag. There is one decoded
direct caller. Files `research/effect-aim-correction-37.asm` and
`research/aim-correction-callers-37.json` preserve the local inspection.
Static evidence establishes a redirection path; it does not prove that the
routine actually redirected that specific recorded projectile. The new hook's
diagnostics and the user's next game test cover that remaining uncertainty.

## Aim intervention

For the current selected actor on the registered live match worker, with manual
ownership and point/direction input, call the original correction once with a
local POD copy. Its stat/RNG work remains; the effect receives the original aim.
Other actors, workers, released control and unit-target inputs pass through.
No borrowed pointer escapes the call. Damage, cooldowns, timing, projectile
speed, collision and explicitly homing/unit-target behavior stay native.
New anchor checks cover caller, prologue and both target writes; the exact whole
executable SHA remains required. A seven-argument relay test exercises stack
arguments and checks that copied correction preserves the original target.

`AIM PRESERVED` logs the cursor vector/point and the discarded native suggestion.
It logs changes and the first three authorized evaluations, not every frame.

## Preview coverage and limitations

Geometry comes from the supplied preview HTML's dashed/ticked rings, yellow
hatching, outlines, target brackets and orange/cyan cues. Native drawing uses
world geometry projected each frame; strokes clip to viewport, minimap and HUD.
Target brackets use the same sprite envelope and eligibility as command picking.
Unit casts retain walk-into-range; native point casts clamp to cast range, and
direction casts retain fixed reach. No input semantics are changed by preview.

Footprints require unambiguous declarations: circle-shaped linear projectiles,
explicitly centered range circles, stationary range projectiles, MoveTo/Teleport.
The embedded metadata covers the eight base data-defined champions; enabled
champion declarations supersede it by ID. The live native cast descriptor supplies
reach/growth/bonuses. Fixed projectile travel length is preserved when explicitly
different from cast reach. Conflicting shapes, native callbacks, buff switches,
secondary impacts and unresolved types fall back to a thin guide. Native compiled
champions such as Ninja currently use that guide; their collision width is not
invented from a similarly named parameter. No guarantee of future hits, bounce
order, charge-up, vector targeting, lob timing or landing damage is displayed.
Runtime native changes to undeclared footprint parameters remain unresolved.

Tests cover declaration ambiguity, diagonal endpoints/clamping, world scaling,
bounded geometry, clipping through offscreen endpoints/HUD/minimap, native aim
copy/relay ABI and worker ownership. Automated tests are not native rendering or
gameplay verification. The user's test is pending.
