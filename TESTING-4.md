# Fourth test: actual champion movement

The third test successfully generated the match at 60 ticks/second for its
full 20-second budget. Battlefield playback started about one second later;
the stop at visible time 0:19 was expected.

Probe **0.4.0** adds a **60-second movement experiment**. Use one single-player
practice match. Save/result authority is not yet established. This still is a
development prototype; mouse movement and abilities are not implemented.

1. Enable **LT Direct Control - Diagnostic Probe** version 0.4.0; keep Harbinger
   disabled. Start a practice match.
2. Default selection is the **blue top champion**. Optional: during the stadium
   or draft screen, before the match starts, use **Ctrl+1/2/3/4/5** for blue
   top/jungle/mid/bottom/support. The overlay shows the selected lane.
3. After the battlefield appears, use the game's camera-follow control for that
   blue champion (default F1 for top, F2 jungle, F3 mid, F4 bottom, F5 support).
4. Hold **L** (right) for 3–5 seconds, release for a few seconds, then hold **I**
   (up) for 3–5 seconds. **J** is left and **K** is down. Keys steer only while
   held; releasing all keys returns the champion to native AI. Note whether
   movement follows your keys and whether there is a delay.
5. Keep normal playback speed and avoid pause/seek during this experiment.
   Watch until the diagnostic test stops (about match time 0:59) and confirm
   ordinary AI movement resumes. Quit the game and report **“movement test done”**
   with what you noticed.

No upload is needed. The log records key/focus changes, requested and validated
native inputs, champion positions, and automatic release. Earlier recordings
are archived in `research/`. Disable the probe afterwards if continuing to play.

The experiment never overrides inputs in background pre-simulation, replay,
another seed/set, or a second foreground run in the same launch. Losing game
focus clears steering; stale samples and death also preserve native AI. The
viewer still has a roughly one-second generation lead, so this test establishes
whether input affects the displayed champion before latency improvements.
