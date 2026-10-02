# Tenth test: InGame playback hook

Probe **0.10.0** changes only the playback hook and its instruction fingerprint.
The worker hold, startup confirmation, deadlines and movement logic are unchanged.
Test 9 confirmed the worker stops after one frame, but the old playback hook
received zero calls. The replacement was traced through the installed 0.6.2
executable's InGame branch, frame receiver and battlefield playback queue.
This is static evidence; live hook entry and successful coordination need this test.

1. Start the game fresh with **LT Direct Control - Diagnostic Probe 0.10.0**
   enabled and Harbinger disabled. Load your save and enter one practice match.
   Choose your lane before the match with Ctrl+1..5 if needed.
2. Wait for **READY**, allowing up to 20 seconds. If it reports startup
   interception not confirmed, quit and report that status; do not try movement.
3. If READY appears, wait three seconds. Check that champions/minions and the
   clock stay stopped. Then press **Ctrl+Home** once and steer with **I/J/K/L**.
   Check that the selected champion responds while the battlefield runs normally.
4. Press **Ctrl+End** to return to native AI/playback, then quit. Report whether
   READY appeared, the battlefield held, movement responded and release worked.

The log must show viewer entrances, initial frame application, a held update
consuming zero frames, READY, and explicit Start. A visible 00:00 clock alone is
insufficient. The first worker frame includes simulation logic, so an untouched
starting state is not established. Running ends after 60 seconds automatically.
These keys remain temporary; the final start/pause/AI interface is deferred.
