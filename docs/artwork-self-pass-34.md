# 0.34.0 artwork, self-hover and route review

The user verified 0.33 ally hover and monster bounds, then reported repeating
Jiangshi artwork disappearance, distracting self-hover, and a theory that large
map cells force zig-zag movement. They approved this bounded pass on 2026-10-06.

## Artwork

`PlayerHud.snapshot` rejects an alive snapshot after 250 ms during running play.
Previously the HUD's unavailable branch explicitly hid every skill and item
image. This is an identified clearing path, not proof that it caused the observed
Jiangshi flicker: the archived log contains no repeating skill0 hide/show events
within the first match's recorded window, and the per-match cap omits late play.

`Artwork` now carries only champion/inventory identifiers and their match/player
scope. The unavailable branch reads it only when fresh telemetry is absent.
Skill images remain visible under an unknown-state shade; HP, gold, cooldowns and
cast counts remain unknown. Last-known item art uses metadata from the same
identity. Fresh readings, replacement inventory, selection changes and session
reset still apply normally. No native entity pointer or guessed timer is retained.

## Self-hover

Selected-player hover snapshots now include the controlled actor ID. Ordinary
hover picking skips that ID and continues through ranked hits, so an overlapping
teammate can still highlight. The original all-unit ability snapshot includes
self; Alt self-cast and valid ally/self targeting are unchanged. Death, release,
selection and session clearing also clear the stored actor ID.

## Read-only movement investigation

Preserved source log: `research/probe-2026-10-06-thirty-third.log`, 23,525 lines,
SHA256 `985dc5e6be225e8e570e9910f408eed4bfe5d0ba8527533ec8df0982869ca734`.

Read `map_path.rs`, `movement_test.rs` and the native movement hook, then compared
final Move orders, requested waypoints, positions and native goals in that log.
No movement code, native hook or pacing policy was changed for this pass.

- The navigation grid is 30 by 30 cells, 32 world units per cell. Of 2,745 actor-25
  movement samples with a Move order across the two matches, 2,744 have a position
  off the cell centers. Movement is continuous; the grid is not a mandatory
  series of discrete champion positions.
- A clear route goes straight to the exact clicked goal. A blocked route uses
  simplified A* waypoints at grid centers before the final goal. Our controller
  submits those intermediate goals to the game's native movement routine.
- Lines 23,051–23,052 show one route advance without a changed final Move:
  goal (744000,504000), intermediate request (720000,304000) then
  (720000,464000), remaining waypoints 3 then 2. Position changes continuously.
  This confirms our waypoint handoff exists, but does not prove an unwanted turn.
- The hook logs native state *before* applying the current request. During
  attacks/casts those fields need not describe a movement destination. Native
  goal differences alone are not evidence of AI interference.

Remaining hypothesis: our grid-derived intermediate goals plus native navigation
may add avoidable turns. A cursor-visible recording with an exact match timestamp
is the next useful evidence. If it identifies a walking-only incident, compare
the final click, our waypoint transitions and actual heading first; discuss a
bounded routing change separately instead of removing path safety speculatively.

## Validation

New regression checks retain Jiangshi art through expiry without extending live
readings, reject other identities/reset state, and exclude self while preserving
overlapping ally feedback. Existing ability/self-cast, hover lifecycle and route
checks remain applicable. Native gameplay/rendering verification is pending the
user's [0.34 check](../TESTING-34.md).

Completed: 158 probe and 18 core tests passed; formatting, Clippy and release
build passed. All 39 native anchors, ABI/null-host behavior and 44 archive
members verified. Installed 0.34.0 DLL, metadata and 42 UI assets with matching
fingerprints; verified 0.33.0 rollback location is recorded in
`research/rollback-34.json`. Native gameplay remains pending.
