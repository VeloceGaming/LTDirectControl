# AA movement stop and attack-move acquisition — 0.75.4

The user reported that Ice Mage continued moving after an AA order while
waiting for cooldown. The old session log was no longer available. Static
review found a gap in 0.74.0: `simulation.rs` issued a hold only when
`is_valid_input` rejected the AA. The native consumer at RVA 0x16f3e30
independently returns at 0x16f3ed0..0x16f3ed8 while cooldown (+0xb0) is
nonzero, without clearing movement. This explains a possible route to the
reported behavior; that particular Ice Mage session was not reproduced.

## Stop before an accepted attack

The current simulation borrow supplies AA range once per selected think.
When an SDK-accepted AA target is in range, the existing one-use stop ticket
carries its entity ID. The existing input attack hook consumes it only for
the owned actor and matching match, checks the native target tag/ID, and
uses the existing movement-only stop and event before forwarding the
original attack. The native-auto hook does not consume this ticket.

There are no new hooks or addresses. The stop only clears ordinary movement
(action 2); it leaves attack windups, skills, CC/forced movement, cooldown,
position and pending-effect queues untouched. Native attack execution still
runs even if the stop is unavailable. SDK-rejected AAs keep the earlier
in-range hold fallback.

Verbose `MANUAL input` records include `attack_in_range`; `ATTACK WAIT_STOP`
records the target and cooldown when ordinary movement is actually stopped.

## Explicit attacks versus ground attack-move

The user clarified that approaching an out-of-range enemy applies to direct
right-click or A-click on that enemy, not automatic target acquisition.

- `Order::Attack` retains the clicked target and its approach fallback.
- `Order::AttackMove` acquires only inside the current AA range, using the
  existing nearest-champion/cursor preference and champion-only policy.
- Without an in-range enemy (or readable range), attack-move retains the
  clicked destination and continues there. It re-evaluates every tick and
  does not chase an enemy after that enemy leaves range.
- The target highlight resolves with the same range as the order.

Range remains the native centre-distance rule, including floor rounding;
sprite selection bounds and target collision radii do not extend it.

## Verification

Regression coverage includes accepted attacks with positive/zero cooldown,
byte-for-byte preservation of all entity fields except movement's action
tag, target mismatch, cast/windup/forced-movement guards, one-use match/actor
tickets, live range changes, loss/reacquisition of an attack-move target,
cursor-preference range filtering and retained explicit-target approach.
Build and package checks are recorded in `dist/build-0.75.4.json`.
Native gameplay still requires the user's Ice Mage test.
