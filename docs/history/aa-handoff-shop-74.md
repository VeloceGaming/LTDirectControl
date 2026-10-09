# AA waiting, reversible AI handoff and shop polish — 0.74.0

The user authorized these together after describing delayed stopping on an
attack input and requesting a way to reclaim manual control from AI.

## Attack waiting

`simulation.rs` previously turned every invalid attack into a move toward its
target. Native validation can reject an attack for cooldown or action state,
as well as distance, so this kept the champion walking through an in-range
target while waiting. The fallback now holds at the current position if the
target is in range. The persistent attack order is retained and retried.

The current AA descriptor is read during the selected worker's SDK simulation
borrow. The guarded entity-position thunk at RVA 0x2e16650 establishes the
native shared entity getter at vtable +0x208, entity id +0x5b8 and position
+0x658/+0x660. The read checks the installed adapter, bound actor/thread, SDK
table identity and native entity identity; no entity pointer is retained.
AA metadata is the existing effect at +0x488: range +0x498, growth +0x4a0,
level +0x5c0 and bonus +0x430. Native approach code at 0x16f6edd..0x16f6faf
compares integer floor square-root of centre distance squared against that
range, without sprite bounds or collision-radius padding. The equivalent
overflow-safe squared comparison lives in `combat.rs`. If the descriptor
cannot be read, the previous approach fallback remains available.

Holding uses the existing native movement-stop path. It does not reset AA
cooldown, cancel a hit windup or synthesize damage. The game's input validation
still decides when an actual attack can start.

## Handoff lifecycle

`Phase::Ai` is a reversible state within the same session, unlike `Released`,
which remains terminal after exit, startup cancellation or an adapter error.
F12 queues ReturnAi on a fresh key edge; F11 or the primary button queues
TakeControl. Both actions are scoped to the current key and phase. The client
excludes an in-flight SDK think while clearing movement/cast commands and
changing ownership. Ordinary mouse input cannot request a takeover.

AI mode returns native AI input and native shopping decisions, restores the
spectator camera/vision, and keeps the worker/viewer binding, generation and
one-frame publication gate. The viewer stays at 1x for synchronized reclaim.
The native spectator pause flag (+0x48, checked at 0x93078b) is preserved in AI
mode; manual playback temporarily clears it so a paused AI viewer cannot trap
manual control. All temporary playback overrides are restored after each call.

The full-width camera/UI lease stays engaged across handoffs. Manual HUD and
settings close during AI control, leaving a Take control button. On reclaim,
manual camera lock/vision preferences return and the camera recenters on the
champion. Temporary drag/toggle gestures are cleared. The two-second heartbeat
failure guard and battlefield-exit release remain active in AI mode.

## Shop

The header uses the packaged `ef_bag` icon and the catch-all filter says All.
The purchase action shows Purchase, Queue or Queue another plus the existing
remaining total. Item names remain in the detail heading. Recipe planning,
alternative paths and purchase execution are unchanged.

## Verification

Regression coverage checks AA range boundaries and retained attack orders,
repeated handoffs, key edges, same match/view/worker identity, one-frame pacing,
stale actions, heartbeat failure, camera reclaim and playback-field restoration.
Build, native-profile, package and installation checks are recorded in dist.
Native gameplay and rendering still require the user's game test.
