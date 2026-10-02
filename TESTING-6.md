# Sixth test: enough startup frames to load

Probe **0.6.0** generates 60 startup ticks before holding further generation.
The previous test held at tick 3 and prevented battlefield loading. This tests
whether a small initial buffer is enough; it does not yet prove full playback
synchronization. Its 60-second control budget still begins at battlefield readiness.

1. Enable **LT Direct Control - Diagnostic Probe 0.6.0** and keep Harbinger
   disabled. Use one single-player practice match.
2. Before starting, check the overlay shows your actual team name. Press
   **Ctrl+1–5** to select top/jungle/mid/bottom/support; confirm **Selected: your ...**.
3. Once the battlefield appears, check your champion and BLUE/RED side are named
   in the overlay. Follow your chosen lane with F1–F5. If **Control ready** appears,
   hold **L** for 3–5 seconds, release, then hold **I** for 3–5 seconds.
4. Quit after the test ends and report **sixth test done**, whether movement
   responded, and any stop reason. If it times out, that is enough information;
   the remaining loading dependency will need a different integration point.

The log is collected locally. Mouse movement and abilities are still pending.
