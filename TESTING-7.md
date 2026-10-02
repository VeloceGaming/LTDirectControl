# Seventh test: publication gate and paused startup

Probe **0.7.0** uses a native adapter for the verified 0.6.2 executable. It waits
between worker ticks, after frame publication. AI callbacks return immediately.
It publishes and applies **one initialization frame**, then holds playback until
you explicitly start. This does not yet establish zero gameplay advancement:
the first tick contains initialization and normal simulation work.

1. Launch the game fresh with **LT Direct Control - Diagnostic Probe 0.7.0** enabled
   and Harbinger disabled. The native adapter installs at the title screen; do not
   hot-reload or replace its DLL while the game is running.
2. Load your save and use one single-player practice match. Before entering the
   match, confirm the overlay has your actual team name. Choose your lane with
   **Ctrl+1–5** (top/jungle/mid/bottom/support).
3. Let loading finish. Expected: the battlefield opens, remains stopped, and
   the overlay shows **READY** and your correct champion/side. Wait about three
   seconds before starting; note the visible clock and whether units move.
4. Press **Ctrl+Home** once after READY. Follow your lane using **F1–F5**, then
   hold **L** for 3–5 seconds, release, and hold **I** for 3–5 seconds.
5. Press **Ctrl+End** to end the control test. Native playback and AI should
   resume. Otherwise the running test releases automatically after 60 seconds.
   Quit the game after this one match/test.

If loading is stuck, **Ctrl+End** also works at the worker boundary without
depending on the battlefield UI. A 15-second loading guard releases automatically
and logs which acknowledgements were reached. A failed startup is a result;
do not repeatedly try the same build. Only one run is controlled per launch.

Report **seventh test done**, plus:

- Did the battlefield open and show READY, or what release reason appeared?
- Before Ctrl+Home, what clock did you see and did units stay still?
- After Ctrl+Home, did I/J/K/L move the correct champion?
- Did Ctrl+End resume ordinary playback?

The local log records the first publication, first viewer call, exact native
played tick, queue consumption, scene transitions, readiness, Start and release.
No extra uploads are needed. Mouse movement, abilities, attack-move, range
previews and final saved-result verification remain pending. Use the mod's
Ctrl+Home/Ctrl+End controls for this test; native speed/pause buttons are not
connected to the coordinator yet.
