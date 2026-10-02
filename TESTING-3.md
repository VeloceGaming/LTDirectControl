# Third test: distinguish startup loading from playback dependency

The second test's 250 ms heartbeat guard released after 226.6 ms of pacing.
The first recorded battlefield clock appeared 988 ms after worker start.
Playback and the diagnostic overlay looked normal afterwards.

Probe **0.3.0** allows up to two seconds for battlefield startup, then restores
the original 250 ms heartbeat guard. It still attempts pacing for at most
20 seconds total, never sleeps on the display thread, and preserves all native
AI inputs. It records startup ticks more frequently. Manual controls remain
inactive.

1. Open the game, enable **LT Direct Control - Diagnostic Probe** version 0.3.0,
   and keep **Harbinger Direct Control** disabled.
2. Watch one single-player match at normal speed for **30 seconds after the
   battlefield appears**. Avoid seeking or changing speed.
3. Note whether the diagnostic line says the timing test is running or stopped,
   and whether movement/clock advances normally or waits/freezes.
4. Quit and tell Codex **“third test done”**, including what you noticed.

No upload is needed; the recording is this project's `probe.log`. Both previous
test logs are archived in `research/`. Disable the probe after testing if
continuing to play.

The next recording must show whether a slowly generated match can present
frames, the startup worker tick at battlefield readiness, and any worker lead
over the displayed clock. Neither live input responsiveness nor saved-result
authority has been established yet.
