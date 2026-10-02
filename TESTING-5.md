# Fifth test: startup hold and own-team movement

Probe **0.5.0** attempts to hold initial generation until the battlefield is
ready. Its 60-second movement timer starts at readiness. This is an experiment:
the game may need more frames to load, in which case the five-second startup
guard releases generation and displays `StartupTimeout`.

1. Enable **LT Direct Control - Diagnostic Probe 0.5.0**, keep Harbinger disabled,
   and use a single-player practice match.
2. Before starting the match, press **Ctrl+1/2/3/4/5** for your team's
   top/jungle/mid/bottom/support. The confirmation line should say
   **Selected: your ...**. Top is the default. Selection locks when simulation
   starts; later attempts should visibly report that it is locked.
3. On the battlefield, check the overlay identifies your champion and its
   **BLUE** or **RED** side. Use your normal camera-follow shortcut for that lane
   (F1 for your top). Once **Control ready** appears, hold **L** for 3–5 seconds,
   release, then hold **I** for 3–5 seconds. J is left; K is down.
4. Keep normal playback speed for this test. The test should stop after 60
   seconds of readiness and restore AI. If it stops earlier, note its exact
   reason. Quit the game, then report **movement retest done**, whether your
   champion responded, and whether selection confirmation appeared.

The log is collected locally; no upload is needed. Mouse movement, abilities,
and saved-result authority are still pending.
