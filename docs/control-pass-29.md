# 0.29.0 control and diagnostics

## Confirmed causes

- The SDK wrapper retained item-build hooks but failed to publish their pointer
  and length in ModExportV1. The observer never populated the item catalogue.
  Export wiring is repaired and exercised through the actual hook vtable in a
  regression test. The observer returns no override and changes no build or RNG.
- Focus loss and an expired foreground heartbeat erased accepted movement.
  New input is ignored in the background while an accepted destination persists.
- A valid SDK skill command could still be rejected by the native consumer's
  action gate. Busy requests wait; three additional verified CALL redirects
  observe actual Q/W/R cooldown-budget spending, retaining a request rejected
  because an action became busy after validation. No animation is forcibly canceled.

## Plain walking remains a gameplay hypothesis

The current native move consumer writes its destination and emits a movement
reset event every time it is called. Repeated submissions of the same active
destination now skip that reset only for the selected champion in ordinary
movement with the native effect gate satisfied. Attacks, casts, forced movement,
changed goals and other champions retain native behavior. This removes a confirmed
redundant event; it is not proof of the cause of every user-reported unwanted turn.
Bounded traces record position, action, requested and native destinations.

## Minimap route

A read-only SDK map observer captures the current match's 30 x 30 wall grid.
Current executable navigation construction at 0x192e91c/0x192eb0c treats wall
value 1 as blocked. Cells are 32 world units; outer index y, inner index x.
The mod constructs a bounded obstacle route, simplifies visible segments, and
submits its remaining waypoints. The minimap draws those same commanded waypoints,
not a claim to reproduce the native AI's pathfinder. Clear routes retain the
exact clicked goal. Wall goals snap to an open cell; disconnected routes are
rejected. The planner prevents diagonal corner cutting but does not model
dynamic unit collision or every champion's movement effects.

If map observation is unavailable, prior direct movement is retained and only
the destination marker is shown; no obstacle route is invented. A later map mod
that changes geometry after this observer can invalidate the captured grid.
Native gameplay must confirm map orientation, wall clearance and route drawing.

## Death and UI scope

The banner reads native respawn ticks, rounds up to seconds, and never substitutes
a wall-clock estimate. The previously verified battlefield-only grayscale and
existing bottom-strip layout remain. Cursor replacement and aesthetic changes
are deferred as requested.
