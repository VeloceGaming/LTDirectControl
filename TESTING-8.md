# Eighth test: locate the actual running path

Probe **0.8.0** corrects misleading status and the missing client-side timeout.
It records every native entrance in counters before match/thread filtering,
samples why calls were accepted/rejected, and records the live game's call stack
at foreground AI ticks 1 and 2. This helps distinguish an unused candidate path
from a failed filter or overwritten patch. The selected native addresses have
not changed; a successful pause is not promised by this diagnostic.

1. Start the game fresh with **LT Direct Control - Diagnostic Probe 0.8.0**
   enabled and Harbinger disabled. Load your save and enter one practice match.
   Use your usual lane choice; selection is not the focus of this test.
2. Watch the battlefield and status line for **about 20 seconds**.
   If it keeps playing, the overlay should say **Waiting for live worker hook**
   and then **Control inactive: Startup interception not confirmed within 15s**.
   Do not spend time trying movement if READY never appears.
3. If it instead reaches **READY**, wait three seconds to check it stays stopped,
   then use Ctrl+Home, briefly test I/J/K/L and use Ctrl+End to release.
4. Quit the game and report **eighth test done**, whether it paused, and the
   final overlay message. Logs are collected from the local project.

The hotkeys are temporary. A separate, clear way to begin control, pause and
hand the champion back to AI will be designed after live control works.
Zero gameplay advancement, mouse commands and saved-result authority remain
unverified. This test is intended to identify the actual interception path.
