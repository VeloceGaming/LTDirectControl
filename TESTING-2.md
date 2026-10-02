# Second test: foreground timing and displayed clock

The first test succeeded: the probe loaded on 0.6.2 (ABI 9), observed background
and foreground simulations, and captured all ten players in the watched match.
However, the foreground simulation computed 559.983 seconds in 6.643 seconds,
well ahead of playback. This is the next obstacle to direct control.

Probe **0.2.0** adds a finite timing experiment. When the project file
`pacing-test.enabled` exists, it attempts to run one foreground match worker at
60 ticks/second for no more than 20 seconds, then resumes ordinary processing.
It preserves native AI inputs and performs no executable patching or save writes.
If that callback runs on the display thread, or the display heartbeat becomes
stale, it immediately stops pacing. Controls remain inactive throughout.

1. Open Teamfight Manager 2 and enable **LT Direct Control - Diagnostic Probe**
   (version 0.2.0).
2. Keep **Harbinger Direct Control** disabled for this run.
3. Watch one single-player match at normal speed for at least **30 seconds after
   the battlefield appears**. Avoid seeking or changing playback speed.
4. Note whether the clock advances normally and whether the battlefield moves
   smoothly, waits/buffers, or briefly freezes. The on-screen diagnostic line may
   say the test is armed, running, or stopped.
5. Quit the game and tell Codex “second test done,” including what you saw.

The recording remains `D:\LTTM2\LTDirectControl\probe.log`; no upload is needed.
Disable the probe after testing if continuing to play. A new launch retains one
previous recording, but report the test before repeatedly restarting.

## Validation

- Nine targeting tests and five timing-policy tests pass.
- Three threaded probe tests pass: display-thread refusal, missing-heartbeat
  release, and a finite worker budget with a responsive client.
- Core and probe Clippy checks pass with warnings treated as errors.
- Release DLL builds against the installed 0.6.2 SDK snapshot.
- Windows DLL loading and entry-export checks are performed before installation.

The next recording must establish separate worker/display thread IDs, displayed
clock readability, successful/aborted pacing, and any buffering behavior. No
playable direct-control feature or changed-result persistence is claimed yet.
