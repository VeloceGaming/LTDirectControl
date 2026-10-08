# Control pass 0.36.0

Approved scope: live-worker isolation, current-borrow steering, native automatic
attack observation, and explicit cast diagnostics. Pre-hit attack cancellation
is not implemented in this pass.

## What the 0.35 test established

The preserved log is `research/probe-2026-10-06-thirty-fifth.log`. Ninja is actor
20, player 2 (Sunna), in live match key `(2212415758926372487,1301,1)`.

- At 02:33 the destination remains `(104800,906000)`, but positions initially
  move right/down from `(32000,928000)`, then turn back toward the destination.
  This confirms a detour without a later click; it is not evidence of AI takeover.
- There are no `MOVEMENT STEERING`, `ATTACK START` or committed-interruption
  records. The old steering authorization reused an earlier input-call address
  and expired at publication. The log did not distinguish rejected guards.
- The Q count shown in foreground observations is one use, while the user saw a
  yellow 3 at 00:39-00:44. The skill hook also updated counts and action state
  from any worker with a matching actor ID. Actor IDs are reused between matches.
  That admission bug is established; its complete symptom impact needs testing.
- Q at 00:18 waits on action state, then a movement click about 130 ms later
  cancels it under the existing rules. The second Q executes. This pass retains
  later-click cancellation, rather than treating every missed Q as native delay.
- W approaches a target around 01:54-01:55, then later attempts return no cast
  around 01:57. Old diagnostics do not distinguish every rejection reason.

## Implementation

Native movement, skill and attack observations now require all of: current
selected actor/key, a known current position, the live match's registered worker,
and control ownership. The same gate precedes native skill acknowledgement.
Observations refresh metadata and action/count state before and after native
consumers, so an earlier callback's busy state is not carried through dispatch.
Native objects and entity addresses are never retained by these observers.

Steering no longer requires an earlier input's address token. The fingerprinted
caller at RVA `15bc83f` supplies actor ID and adjacent position fields at
EntityData `+660/+668`. The hook recovers this call's current entity, verifies
kind/identity, and evaluates clearance using that current position. Clear
segments use native arbitrary-angle stepping; uncertain or blocked segments
retain ordinary navigation. Native speed, integer movement, events and route
planning are unchanged. Counters distinguish all steering entries, authorized
entries, direct steps and rejected position-field shapes.

The additional CALL at `15b58dd` targets the same native attack function
`15b2700` as the explicit-input call. Its caller builds a target on the stack and
passes three arguments: current entity, target pointer and events. The new hook
forwards exactly once, recording an accepted start only when the native action
and new delayed-effect queue entry prove acceptance. Start records identify
`input` versus `native-auto`. Unknown effect scheduling retains native animation.
Coverage of Ninja's actual attack path remains for the user's next test.

A buffered learned/ready skill may still release only proven post-commit
BaseAttack animation: observed start delay has elapsed, the pending queue is
empty, native CC/forced-movement guards allow it, and attack cooldown is retained.
This pass neither removes pending attack effects nor changes cooldowns. It does
not interrupt active skill casts or implement pre-hit attack swing cancellation.

Cast logs include trace ID, cancellation/drop/rejection reason, age, native
action, readiness and bound target. Wait records occur on action/approach state
changes rather than on every retry. A later click, stop, recall, replacement,
expiry, target miss/loss and native consumer rejection can be distinguished.
Existing command priority, buffer duration and cancellation behavior are retained.

## Verification and limits

169 probe tests and 18 core tests pass, along with formatting, Clippy,
release compilation, 43 exact executable/branch anchors, DLL ABI 6/null-host
checks, and all 44 package/installed-file fingerprints. A verified 0.35.0 rollback
copy was saved before installation. Added tests exercise reused actor IDs on
another worker, charge-state preservation, steering across separate current
borrows without an input token, invalid position fields, three-argument automatic
attack forwarding, post-consumer cooldown readiness, and bounded cast diagnostics.

These are code/ABI checks. They do not verify native hook execution, gameplay,
damage, navigation or rendering. See `TESTING-36.md` for user verification.

Read-only evidence: `research/steering-cancellation-35.asm` includes both native
callers; `research/native-movement-35.asm` and
`research/native-route-refresh-35.asm` document native movement. The executable
fingerprint remains the exact supported 0.6.2 build; no executable on disk is
modified.
