# Eleventh test: continuous manual ownership

Probe **0.11.0** keeps the tested worker/playback hooks. After Ctrl+Home, the
selected living champion receives either movement or a hold command on every
AI callback. Releasing movement keys no longer returns that champion to AI.
Idle and expired key samples use move-to-current-position; a rejected command
explicitly releases with a logged reason. Other champions retain native AI.

1. Launch fresh with probe 0.11.0 enabled and Harbinger disabled. Enter a practice
   match and wait for READY as before.
2. Press Ctrl+Home **without pressing movement keys**. Watch for about five
   seconds: the selected champion should stay still while other units play.
3. Hold a movement key for a few seconds, release it, and watch again. Check
   whether the champion stops rather than resumes its AI destination. Repeat
   with another direction and, if practical, near enemy minions. Watch for
   unsolicited attacks or ability casts while idle as well as movement drift.
4. Press Ctrl+End. Check that native AI resumes, then quit and report the result.

If the status reports Control inactive or a rejected command, report that text.
Running still ends automatically after 60 seconds; longer pauses in the client
or leaving the battlefield also release control through the existing guards.

The current executable accepts move-to-current-position as a valid input, and
the SDK uses it to replace built-in input. Static validation does not prove
that this cancels every existing actor order or an attack/cast already underway.
This test checks those visible effects. Skills and right-click movement remain
unimplemented; these movement keys are temporary diagnostic controls.
