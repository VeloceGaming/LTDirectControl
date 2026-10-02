# Fifteenth test: Q/W/R quickcast and Shift previews

Historical instructions for 0.15. The user's requested normal-cast behavior
supersedes hold-only previews in 0.16; see TESTING-16.md.

Probe **0.15.0** keeps the working 0.14 camera, combat and HUD behavior, and adds
ability controls. Fresh launch, Harbinger disabled, choose your lane before the
match, confirm your champion/side at READY, then Ctrl+Home. Ctrl+End releases.
The running test still has a **60-second guard**; use another fresh launch for a
different champion if needed. The previous working package is backed up.

1. **Preview:** hold Shift, then hold Q, W or R. A cast-range circle should follow
   your champion; aimed abilities also show a direction line or cursor marker.
   The yellow status line names the skill, targeting type, range and cooldown.
   Release the ability or Shift: the indicator must disappear without a cast.
   Releasing Shift while the ability stays held must also not cast. Previews
   should leave your existing movement/attack order running.
2. **Quickcast:** point at the battlefield and tap Q/W/R without Shift. Ground
   and direction abilities should aim at the cursor without a left-click.
   Unit-targeted abilities need an eligible unit under the cursor; allies qualify
   for native ally-targeted skills. Self-target and no-cursor-target skills use
   their native input forms. Holding an ability must not repeat it.
3. **Rejection:** press a skill on cooldown or aim a unit-targeted skill at empty
   ground. The yellow line should report that it was not cast. It must not fire
   later on its own when the cooldown finishes or a unit walks under the cursor.
   Normal movement/attacks should continue and control should remain active.
4. **Camera alignment:** repeat preview/quickcast while panned and at another
   zoom level. The circle stays centered on the champion, and the aim should
   follow the cursor. Real HUD/minimap areas block aimed casts. The centered Tab
   scoreboard still permits battlefield commands through it.
5. **Cancellation:** S, Esc, Ctrl+End, focus loss or champion death should dismiss
   previews and cancel pending requests. Returning focus with an ability still
   held must not cast. Existing camera/combat controls should still work.

Please report the **champion name, which key, intended target and what happened**
for any failed or misdirected cast. If possible, test an aimed skill and a
unit-targeted skill across your matches; healing/self-cast and form/recast
abilities need separate champion coverage. Report whether the preview appears
and whether the status says “cast sent” or “not cast.” The log records native
Q/W/R descriptors, the input submitted, and native validation results.

These are **cast-range and aim indicators**, not damage-area shapes, projectile
widths or guaranteed hit indicators. Native target collision radii and
target-dependent range modifiers can extend unit eligibility beyond the ring.
No cooldown, damage, projectile or animation values are overwritten. Native
animation locks, level unlocks and recast rules still apply; requests are not
queued until an animation ends. Passive slots with no active effect cannot cast.
Ground casts retain the game's native behavior at the edge of cast range.

Only one pending skill is kept; simultaneous key edges in one client frame use
R, then W, then Q priority. Holding right-click does not block ability presses;
a new right-click cancels an earlier pending request. Self/no-cursor skills can
be pressed even when the cursor is over HUD. Runtime metadata expires after
250 ms; missing/stale data rejects a cast rather than guessing.

Automated tests verify the input policy, metadata field reads and native relay
forwarding. They do not establish visible casting or all-champion compatibility.
