# Persistent attack-target outline (0.57.0)

The user confirmed 0.56.1, including the debug marker correction, then approved
adding selection feedback separate from hover. This implements the current
active attack order, not a new left-click inspection/selection system.

`MovementTest::refresh_hover` returns both entries of the same captured frame
tuple. The second is the valid attack resolved from the existing Attack or
AttackMove order and fresh enemy snapshot. It is then checked against the
authoritative all-unit snapshot, so a death/visibility removal wins even before
the enemy-only snapshot refresh. SDK collection already filters alive,
targetable entities using `sim.is_visible(controlled_side, id)`. Ground moves,
stops, no acquired attack-move target and inactive state return no attack target.

The adapter publishes hover and attack identity/position/team together in one
mutex value. A rendered entity matches by native ID plus the existing position
guard. Hover role wins over attack role for the same ID, so only one set of four
outline passes is generated. A different hover and attack target may each have
one outline; only the hover winner affects the cursor. Friendly entities cannot
receive the attack role. Targeted skill hover eligibility remains unchanged.

Attack role uses four flash silhouette passes at 1.0 sprite-coordinate offset.
Hover remains 1.5 crisp or 2.5 wide, with existing team colours. No new native
hooks or game-function patches are added. Existing normal drawing, bounded merge,
native cleanup and fallback rules remain. Worst-case extra work increases from
one to two outlined bodies when hover differs from attack; the same body is
never outlined twice for these two roles. Native performance must be tested.

Interface label is now Sprite outlines, covering both roles. The saved
`hover_outline` key remains for compatibility; it gates both published targets.
Debug markers retain their independent `selection_debug` setting. F10 changes
hover thickness only. `OUTLINE` logging distinguishes hover/target draws.

Regression tests cover separate and merged roles, relative widths, no friendly
attack outline, persistent attack feedback with no cursor/over blocked HUD,
authoritative visibility loss before enemy refresh, target changes/stop,
attack-move acquisition/no acquisition, stale snapshots, inactive state and
death. Full native instructions/cleanup and package fingerprints are verified;
native appearance and gameplay remain pending. See `TESTING-57.md`.
