# Derived collision values: tracing and test prototype

Status: **research and separately testable prototype**, 2026-10-13. The
prototype is not an accepted release change. See
`investigation-athlete-stats-manual-control.md` for the original stat model.
The normal build still uses the existing 21 collision sites. The prototype
is compiled only with `collision-prototype` and stages outside the release
package.

## Question and concrete finding

Could manual play use fewer collision changes by intercepting the numbers
*before* the 21 collision calculations, while leaving the athlete's stored
stats alone? The two reviewed native versions contain three producers of a
temporary Skillshot Accuracy allowance and one producer of a per-entity
Dodging adjustment. Replacing their **derived values** gives the intended
5-game-unit allowance without writing the athlete's stats. The original
21 collision locations remain as numerical observers in this prototype;
they never correct its result. Eight reaction sites retain the previously
approved bounded reaction model while the prototype switch is on.

| Version | Accuracy producers | Dodging producer | Existing collision consumers |
| --- | --- | --- | --- |
| 0.6.3 | `12612c9`, `16f8d18`, `16f8e54` | `1466aff` | 21 verified sites |
| 0.7.0 beta 1 | `14d7029`, `1556ee2`, `155702d` | `174608f` | 21 verified sites |

These numbers are relative addresses in their respective game executables.
The recorded SHA-256 identities are `f21cf691799a83d9afa0ccae3e2b95862f5780446a501ee97860b91b752dbe2f`
for 0.6.3 and `6c9cd659e727b4130766241e9d43b222454a8455609b40f9bf73c01d5136a94f`
for 0.7.0 beta 1. The exact bytes are recorded in both native profiles and
checked against the corresponding saved executable before a test build.

Native Accuracy is capped at 100 then multiplied by 70: 0–7,000 internal
units. This is a temporary local value, not a demonstrated persistent
projectile property. Native Dodging is capped at 100 then multiplied by 150:
0–15,000 units, stored in the entity's `+0x6a0` derived adjustment. The
physical radius is `+0x678`; the size modifier is `+0x468`. Internal units
are 1,000 per displayed game unit. Collision sites use either
`max(scaled_radius + accuracy - avoidance, 0)` or
`physical_radius + max(accuracy - avoidance, 5000)`.
Setting the actual Dodging stat to 50 would produce **7,500**, so it would
neither match the desired fixed geometry nor leave player data untouched.

In the proposed smaller policy, a manual match writes 5,000 to the three
temporary Accuracy results, and zero to the derived Dodging value just before
the native store. The original owner-absent branch still takes its own path.
Turning the prototype off passes through the native producers. AI/spectator
scope also passes through. No athlete stat field or projectile is edited.
The final implementation, if the prototype passes, could remove the 21
observer hooks and have 8 reaction + 4 producer locations instead of 8 + 21.
The **test build uses all 33 sites**, so its performance does not establish
the performance of that possible final version.

## Lifetime and handoff evidence

- The default entity constructor zeros `+0x6a0` (0.6.3 `17266b6`;
  0.7.0 `1588ee6`). The reviewed summoned-unit constructors call that
  constructor and copy its resulting field. The Illusionist creation path
  also uses a fresh default entity, not an owner-stat clone.
- Entity cloning copies `+0x6a0` unchanged (0.6.3 read `149d709`, write
  `149db2d`; 0.7.0 read `1773219`, write `177363d`). Traced tower/nexus
  creation starts from a zeroed template. State and lookup-cache clones can
  also copy the value; the examined lookup cache is destroyed after its
  callback, rather than becoming a new battlefield unit.
- The native input phase updates the bound champion's derived Dodging value:
  0.6.3 multiply `1466aff`, store `1466b06`; 0.7.0 multiply `174608f`,
  store `1746096`. In three traced match-update paths per version, that
  input phase is dispatched before casting and collision work. This is
  **ordering evidence**, not a proof that every custom event traverses it:
  input dispatch is conditional. A return to AI normally restores native
  derived values on its next update; the prototype measures observed
  restoration and treats paths not observed as incomplete.
- Other reads of `+0x6a0` increment shared counters (`1265e15` and
  `17028cc` on 0.6.3; `14dbad5` and `1560f75` on 0.7.0). The exact
  purpose of those counters is unresolved. No collision geometry write was
  identified at those sites; this is not a claim that no indirect reader
  exists.

The tracing followed named constructors, clones, native stores, match update
paths and the 21 verified consumers. It is not exhaustive for Workshop code,
all effect categories, or every pointer/reference obtained indirectly.
The saved profile checks exact executable bytes and whole instructions and
fails closed if a version changes.

## Prototype and objective test standard

The prototype has an Advanced > Debug switch. **On** applies the four
derived-value changes and bounded reactions in a manually controlled match.
**Off** uses native stats and reactions. It switches at a simulation boundary;
measurements continue in both modes and while watching AI. The 21 old
collision sites observe the actual native result and compare it with the
expected result. Their reports include a count for each site, numerical
mismatches, invalid inputs, and checks of `+0x6a0` after a native write.
Those checks also record whether native values return after switching off
or handing control back to AI. Logging is aggregated once every 300 worker
publications, not written for every collision calculation.

Run `python -B tools/collision_prototype_report.py PATH_TO_GAME_LOG` on the
game log in `%LOCALAPPDATA%\LTDirectControl\probe.log`. For a long session,
pass `probe.segment.previous.log`, `probe.segment.log`, and `probe.log` in
that order when those files exist. Each launch has a separate run ID; the
checker uses only the newest run and keeps its latest cumulative measurement,
so old launches and rotated snapshots are not counted twice. It returns one of:

- `FAIL` / exit 1: any compared number or observed native restoration
  disagreed. One mismatch is enough; do not proceed with the reduced hooks.
- `INCOMPLETE` / exit 2: no numerical failure, but at least one of the 21
  consumer sites or four producers was not reached while on; a mode has no
  restoration check; an On→Off/manual→AI transition was not observed; or an
  invalid input/observer table overflow occurred.
  This is **not** a pass. The JSON names the missing sites.
- `MEASURED GATE PASS` / exit 0: all 25 sites ran, every measured number
  agreed, and prototype-on, manual-off and AI/inactive modes each produced
  at least one field check. This passes only the **observed paths of that
  game log**, not all champions or every future version.

Additional acceptance gates: verify an observed mid-match On→Off switch and
manual→AI handoff in the log; compare repeated Crossbowman Q shots at fixed
target positions with Accuracy 3/50/100 and fixed opponent Dodging, then vary
Dodging. Record aim point, target distance, hits/attempts and health lost
for each set of at least 20 shots. Use the same champion, opponent, items,
level and skill rank for all sets. Check a unit size modifier, a summon or
illusion, a tower, a ground effect, a projectile, a Workshop champion, and
simultaneous skill threats. A run with incomplete path coverage is a
diagnostic result, not a reason to guess. The player performs the in-game
tests; automated checks establish executable identity, numerical behaviour
of test fixtures and package integrity only.

The startup-role prompt and first-base spectator shop popup reported earlier
are separate open problems and are not part of this prototype.

### Player test checklist

1. Back up the current installed `lt_direct_control` folder. Replace that
   single folder with the staged test folder in
   `releases/collision-prototype-package/lt_direct_control`; do not load two
   copies with the same mod ID. Launch 0.7.0 beta 1. In Settings > Advanced
   > Debug, confirm **Collision prototype and bounded reactions** is On.
2. Start manual play as Crossbowman. At a repeatable distance, fire Q at the
   same stationary or predictably moving opponent at least 20 times. Count
   attempts, visible contacts and health-reducing hits. Note the target's
   champion, level, items, distance, and your Accuracy and their Dodging.
   Repeat under the same setup at Accuracy 3, 50 and 100, then hold Accuracy
   fixed and vary Dodging. These are recorded observations; no hit-rate
   prediction replaces the numerical gate above.
3. During manual play, turn the Debug switch Off and continue for at least
   300 simulation updates (about 10 seconds at normal speed). Confirm Q and
   ordinary attacks still work. Turn it On again for at least 300 updates.
   The log must record a real
   prototype→manual vanilla transition and native field checks in both modes.
4. Press F12 to return control to AI and let the match run at least 300
   updates (about 10 seconds at normal speed). Take control again with F11.
   The log must record manual→AI and
   a native field restoration check in inactive mode.
5. Exercise a tower, a summon or illusion, an enlarged/shrunken unit if
   available, a ground effect, and a Workshop skill. Ask for the exact log
   report; if a site is still unvisited, that is **incomplete**, not a pass.
6. Close the game. From the project root in PowerShell, run
   `python -B tools/collision_prototype_report.py (Join-Path $env:LOCALAPPDATA 'LTDirectControl\probe.log')`.
   If there were rotated logs, include those as
   additional arguments, oldest first. Save the printed JSON and report
   any visible mismatch as well as its `status`. Restore the backed-up
   installed folder after the test.
