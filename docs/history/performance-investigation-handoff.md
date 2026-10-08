# Starting prompt: late-match stutter and ally-hover flicker

Investigate LTDirectControl 0.32.0 in `D:\LTTM2\LTDirectControl`.
Start with read-only analysis. Follow `AGENTS.md`: explain your findings and
proposed next step in plain English, and wait for my explicit go-ahead before
editing or building code. Preserve the working implementation and existing
uncommitted changes. I play/test the game; automated tests cannot verify native
rendering or control feel.

## My latest test report

- Control is much more responsive after 0.32.0.
- Hovering and selection feel great. Some monsters' visual selection envelopes
  seem a little too large; do not broadly shrink the working champion areas.
- Item forecast and missing-gold/recall reminder now work.
- Hover highlights on teammates flicker frequently.
- As the match progresses, FPS becomes unstable and controls feel delayed again.
  I do not know whether this mod, another mod, the vanilla game, or a combination
  causes that. Do not assume the answer.

## Logs and context

Use this preserved latest-test log first:

`D:\LTTM2\LTDirectControl\research\probe-2026-10-06-thirty-second.log`

It starts with `INIT game=0.6.2 abi=9 probe=0.32.0` and contains 24,299 lines,
4,243,104 bytes. SHA256:
`5b84de9ce6223f8fb09d12077ef6d285e73e9db45d068dec4463587114c9055b`.
There are 1,800 `INPUT TRACE` lines. These are observations, not evidence that
logging caused the stutter. Logger limits reset between sessions, so the total
file line count is not itself proof that the cap failed.

Live log: `C:\Users\j9010\AppData\Local\LTDirectControl\probe.log`.
Previous-launch log: same folder, `probe.previous.log`. Launching the game
rotates logs; preserve evidence before further launches. Session-result JSONs
are under that folder's `research` subdirectory. In a sandbox, `%LOCALAPPDATA%`
may resolve to a different package directory: use the explicit user path above
for this machine.

Historical Ninja/unwanted-movement report:
`D:\LTTM2\LTDirectControl\research\probe-2026-10-06-twenty-ninth.log` and
`docs/open-issues.md`. It established that unwanted turns also occurred during
plain walking, but did not establish a cause. Do not conflate that older report
with the current FPS issue.

Read `docs/control-pass-32.md`, `TESTING-32.md`, and
`docs/reverse-engineering-guide.md` for current changes, evidence limitations
and native investigation methods. Some older notes are superseded; in
particular, the final section of `docs/open-issues.md` still says the 0.32 test
is pending. The user report above is newer.

Game executable:
`C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2\TeamfightManager2.exe`.
Workshop packages: `C:\Program Files (x86)\Steam\steamapps\workshop\content\3009300`.
The relevant item mod is package `3739568852`. Inventory the actually enabled
mods rather than assuming every installed Workshop package is active.

## Where to begin in the source

All paths below are relative to the repository root:

- `probe/src/lib.rs`: `pre_update`, `post_update`, SDK AI callbacks, entity
  enumeration, hover snapshot publication, synchronous mutex-protected logger.
- `probe/src/native_timing.rs`: publication/view coordination, condition
  variable, produced/consumed counters, empty-queue samples, command traces.
- `probe/src/native_adapter.rs`: native worker/view/input hooks and consumers.
- `probe/src/movement_test.rs`: `observe_hover_units`, `target_markers`,
  `draw_targets`, unit freshness and movement snapshots.
- `probe/src/combat.rs`, `sprite_picking.rs`, `camera.rs`: picking priority,
  body envelopes and world/screen projection.
- `probe/src/player_hud.rs`, `hud_icons.rs`, `tooltips.rs`, `team_status.rs`:
  snapshots, asset/metadata caches, UI updates and tooltip work.
- `probe/src/input_trace.rs`: diagnostic command IDs/timestamps.

0.32 changed the running publication lead from two frames to one, moved
gameplay capture to `pre_update`, and replaced the 2 ms pacing sleep with a
condition variable notified by the viewer. SDK AI callbacks must not wait.
The native viewer may consume multiple queued frames after a hitch. A smaller
lead improves latency but may expose starvation if production cannot keep up;
this is a hypothesis to measure, not the established cause of late-match FPS
instability.

## Investigation requirements

Separate client frame/render stalls, simulation-tick cost, playback starvation,
mod callback work, allocation/cache growth, file I/O, and shared-lock contention.
Check for work that scales with elapsed match time or live entity count, repeated
asset/JSON reads, redundant UI property updates, and logging on busy paths.
An expensive-looking loop is a lead, not proof of the bottleneck.

For ally flicker, follow the entire snapshot lifecycle. Check which callbacks
publish populated or empty hover lists, session/actor filtering, freshness
timestamps, fallback enemy-only lists, selection priority and camera projection.
Verify whether the target actually alternates or the same target's drawing
disappears. Investigate a possible shared cause with stalls, but allow them to
be independent issues.

Existing `INPUT TRACE` links capture, worker dispatch, publication and playback;
it does not measure first visible movement or rendering FPS. Simulation counters
near 60 Hz do not rule out brief render stalls. Logs currently do not constitute
a complete frame-time/callback profile.

First return evidence-backed findings with exact source locations, alternative
explanations and what the existing evidence cannot establish. If profiling is
needed, propose a small bounded plan measuring per-frame intervals, callback
duration, worker tick duration, queue starvation and lock waits. Prefer cheap
aggregates/histograms and sparse outliers over per-entity/per-frame log spam.
Do not add instrumentation until I approve it.

Propose a controlled comparison using comparable matches/settings: current mod
active versus a run without this direct-control mod, with other mods held
constant. Disabling manual control while leaving its hooks/UI installed is a
different comparison, not a clean no-mod baseline. State which comparison tests
which hypothesis, and ask only for the necessary missing reproduction details
(approximate match time, FPS change, champion, camera/HUD state).

Do not change the functioning purchase tracker, skill rules, HUD design or
champion click envelopes during this investigation. Recommend the smallest
supported correction after identifying the cause, and wait for my go-ahead.
