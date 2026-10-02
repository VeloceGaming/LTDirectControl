# Ninth test: corrected foreground hooks

**Historical test: the worker hold succeeded, but the playback hook received
zero calls and startup timed out. The foreground-view claim below was incorrect.
Use [TESTING-10.md](TESTING-10.md) for the current build.**

Probe **0.9.0** replaces the two unused hook locations. Test 8's recorded
call stacks identified the active worker and the SDK client update. The worker
hook now waits after a successful frame send, outside the runner and output
locks. The display hook updates the same foreground view passed to the SDK.

1. Start the game fresh with **LT Direct Control - Diagnostic Probe 0.9.0**
   enabled and Harbinger disabled. Load your save and enter one practice match.
   Choose your lane before the match with Ctrl+1..5 if needed.
2. Watch the battlefield and status for up to **20 seconds**. It should hold
   during loading and then reach **READY**. If READY never appears, stop the
   test, quit the game and report the final status. Do not try movement.
3. If READY appears, wait three seconds and check the battlefield and clock
   remain stopped. Then press **Ctrl+Home** and briefly steer with **I/J/K/L**.
   Check whether your selected champion responds and the battlefield continues.
4. Press **Ctrl+End** to return to native playback/AI, quit the game and report
   whether it held, reached READY, responded to movement and released normally.

Running still ends automatically after 60 seconds. These hotkeys are temporary;
the final play/pause/AI interaction will be designed once live control works.
The first initialization frame contains simulation logic, so an untouched
00:00 start is not established. Single-frame loading and movement are still
pending this test. The previous log has been preserved locally.
