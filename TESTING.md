# First diagnostic game test

**Completed.** See [the second timing test](TESTING-2.md) for the next run.

The observational probe was built and installed on 2026-09-30 at:

`C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2\mods\lt_direct_control_probe`

It is a development diagnostic, **not the playable direct-control mod**.

1. Open Teamfight Manager 2 and its mod list.
2. Enable **LT Direct Control - Diagnostic Probe**.
3. Disable **Harbinger Direct Control** for this run.
4. Load a single-player game and watch one match normally. Let the match reach
   the battlefield and play for at least 20 seconds; finishing it is useful.
5. Quit the game. Tell Codex the test is done; the recording is already local.

The recording is `D:\LTTM2\LTDirectControl\probe.log`. No upload or coding is
needed. Each game launch replaces the recording, so report the test before
launching the game again. Disable the probe after the test if continuing to play.

If the game reports a load error or the probe is missing from the list, report
the message. Do not remove or edit a save to work around a mod diagnostic.

## Verified before installation

- Targeting foundation: nine behavioral tests passed.
- Targeting foundation and probe: Clippy passed with warnings treated as errors.
- Probe: release DLL built against the installed 0.6.2 SDK snapshot.
- Windows loaded the DLL; its exported minimum ABI is 6.
- Its entry point safely rejects a null host.
- DLL and metadata installed into a new, distinct mod folder.

Game loading and callback execution are awaiting this test. The next research
step is to compare wall-clock sample timing, simulation ticks, origin kinds,
match identities, and UI screen transitions. Static executable research alone
has not established valid simulation hooks or the path for changed match results.
