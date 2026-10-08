# Control pass 0.35.0

The user approved the implementation following the movement and attack timing
investigation in `research/steering-cancellation-35-notes.md`.

## Command rules

- A skill edge and right-click captured in the same poll favor the skill. The
  underlying attack order can remain for continuation after casting. A later
  right-click cancels the earlier waiting cast; newer skills replace it.
- A native attack that has not started yields to the skill proposal. Once its
  windup starts, the cast waits. The skill's target identity or world aim is
  retained, rather than choosing a new target during each retry.
- Existing stop, recall, pause, release, generation and timeout rules still clear
  commands. Native validation and acknowledgement remain required; accepted
  command shapes alone do not prove that a cast executed.

## Direct movement on clear segments

The 0.6.2 movement routine at RVA `15c2d90` uses the packed route table to choose
neighboring cell centers. The native routine at `15b3a40` uses arbitrary-angle
integer vector stepping at the supplied speed and emits ordinary position events.
Continuous positions therefore did not rule out grid-guided turns.

One additional CALL at `15bc83f` is intercepted. It uses direct stepping only
for the selected actor, on the authorized simulation thread, with current-match
identity and a clear segment in the captured 30x30 wall grid. Its borrowed x/y
addresses must match the selected entity's current position fields. A per-tick
address token is compared only; it is never dereferenced later. Missing geometry,
blocked corners, other actors or out-of-bounds points use the original routine.
The native speed, bounds and position events are retained. The undocumented
entity flag at +489 is not changed. Existing route planning around walls remains.

Evidence: `research/native-movement-35.asm`,
`research/native-route-refresh-35.asm`, and the action dispatcher in
`research/steering-cancellation-35.asm`. Installer and runtime checks include the
exact game fingerprint, original branch and both function headers. This does
not establish the exact cause of the unrecorded Archangel 06:48 incident.

## Guarded attack backswing release

Attack acceptance starts native cooldown +b0, action 3 and elapsed +78=0. It
also appends a delayed effect to the entity queue. The adapter records the delay
only when an actual new queue entry appears. It does not use champion-specific
hardcoded timings.

A learned, ready buffered skill may release the remaining animation only when
elapsed time has passed that observed delay, the pending effect queue is empty,
the native attack type is BaseAttack, ownership/focus are valid, and existing
CC/forced-movement guards allow it. The native stop event is emitted and only
the action is cleared. Attack cooldown, delayed effects and skill cooldowns are
not reset. Unknown starts, special attack types and nonempty queues fall back to
normal animation completion. Active skills are not interrupted by this path.

The native Action.cancelable vtable slot has not been established, so this build
does not call a speculative slot. This is a conservative BaseAttack commit gate,
not a claim that every modded attack is generically interruptible. User testing
must check damage, attack speed, fast weaving and any unusual champion attacks.

## Cast count display and diagnostics

The HUD previously inferred the count from capacity divided by rounded cost.
This could report more uses than declared, and telemetry expiry could blank the
indicator. It now caps remaining uses to the native declared count. Zero-cooldown
budgets do not display a multi-use counter. The display may retain a known count
for one second across telemetry gaps; readiness still requires fresh data.
Known single-use data removes the counter immediately. Death and identity resets
clear retained counts. This addresses code paths capable of phantom/flickering
counts; it does not prove Jiangshi's exact runtime cause without a new test.

Change records include declared uses/capacity/cost, movement steering decisions,
attack starts and committed interruptions. Movement sampling no longer stops
after 1,800 samples. The runtime logger no longer stops after 12,000 lines; it
rotates current plus two earlier segments at about 4 MiB each. Team-panel
visibility repeats are deduplicated without changing how visibility is enforced.

## Validation

164 probe and 18 core tests pass. New tests cover simultaneous command priority,
commit timing and identity guards, native queue observation without writes,
clear/wall/unknown steering decisions, 10-argument steering forwarding,
8-argument direct-step calling shape, rounded/retained count semantics and
late-match log rotation. Formatting, Clippy and release compilation pass.
The packaged DLL must additionally pass ABI 6, null-host rejection, exact native
anchor checks and package fingerprints. Native gameplay is pending user testing.
