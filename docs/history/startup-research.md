# Startup investigation and proposed integration

## Correction after test 9

Test 9 confirmed one worker publication and a 15-second hold, but zero entries
at the 0.9 playback hook `0xb240e9`. The prior display classification below is
superseded. The current executable's SDK scene conversion maps native tag 11
to InGame; the older SDK's internal scene numbering differs. The observed outer
update calls scene handler `0x9f7b90` at `0xb2226e`. Following its current InGame
dispatch reaches the frame receiver `0x9fbbe5`, view queue and playback CALL
`0x9fdaf0 -> 0xa8b090`. Native playback can consume a single queued frame.

After user approval, probe 0.10 replaces only the unused playback site and its
fingerprint, retaining the working worker hold and startup acknowledgement.
Live entry, initial application, held playback and movement remain pending
test 10. See `native-adapter.md` and `TESTING-10.md`.

## Runtime correction after test 8

The historical address classifications below were hypotheses from static
callers. Test 8 recorded the normal foreground AI stack through `0xbe43fa`
inside the shared worker `0xbe42b0`, disproving its tutorial-only classification.
The SDK client stack points to `0xb1fb00`, which calls scene handler `0x9f7b90`.
Probe 0.9 redirected frame-send CALL `0xbe470a` after all three write locks were
released, and selected playback CALL `0xb240e9` from SDK pointer proximity.
Test 9 confirmed the worker but disproved the playback classification. See
`native-adapter.md` for the current implementation and remaining limits.

Research updated 2026-10-01. These passes inspected saved logs, the installed
0.6.2 executable, the reference DLL, and SDK files. It added research artifacts
inside this project. It did not build or install a new mod, launch or manipulate
the game, patch process memory, or modify saves.

## What the latest test establishes

The archived sixth recording is `research/probe-2026-09-30-sixth.log`; its parsed
summary is `research/probe-sixth-summary.json`.

- The mod resolved team 7, Cute and Stunny, after the management save loaded.
- It bound player 7 / athlete 56 / RED / mid / crossbowman for this match.
- Worker ThreadId(15) started at tick 1, then waited at tick 61.
- StartupTimeout occurred after 5.000924 seconds, with `running_us=0`.
- The first recorded display clock was 00:00 on ThreadId(2), 610 ms after the
  timeout released the worker. By then, the latest sampled worker tick was 4200.

This failure happened before the movement test's running phase. It does not
establish whether movement commands work. The whole-second clock and periodic
worker samples cannot measure precise input latency or exact frame backlog.
The timing supports a loading dependency on worker progress or released
resources, but does not by itself identify the dependency.

The recorded scene sequence is Title → Main → Lineup → StadiumEntrance → Match
→ InGame. The SDK simulation origin was `ClientMatchView` (2). This is evidence
for a normal watched match, rather than a Tutorial5v5 session. Research into a
tutorial's loading dependency must not be substituted for this match's startup.

## Evidence from the current executable

All addresses below are RVAs in this particular executable, not callable APIs
or ready-to-use hook sites. Image base: 0x140000000. SHA-256:
`15df9eb3b6915cdcc4c2ebb3b7f5fa232b563c4cdd32208581634817b71adc23`.

| Finding | Current-build evidence | Interpretation and limit |
| --- | --- | --- |
| Spectator controls share native playback state. | UI handlers load a shared object through the ingame UI object's +0x9150 field. UI refresh at 0x26c7cc0 reads its speed fields and writes button selection flags. | Selection flags reflect native state. Setting a flag alone has not been shown to pause playback. |
| The pause button follows an input/action path. | Construction at 0xbb7950 binds `time_control.pause` through 0x7f1ac0. Its callback table leads to 0x82d710 / 0x862a50, which enqueue an input event. Input handling at 0xcaf090 reaches 0x1dcda20. | The native button performs an action; it is not merely a selectable-state write. No native function was invoked during this research. |
| A native routine toggles speed between zero and the saved setting. | 0x1dcda20 reads the shared object through a client object's +0x448 field. When mode/speed are already zero, it restores saved fields at +0x380/+0x384. Otherwise it saves them and writes mode 0 / speed 0 to shared-object +0x28/+0x2c. Source-location records point to `game-view/src/view.rs` lines 414–419. | This is strong static evidence of the actual pause/resume operation. The older SDK also names `GameClient::pause_time`, but does not supply a verified 0.6.2 ABI. |
| Playback processes queued frames separately from generation. | View update 0xa8b090 reads playback speed, accumulates time, tests whether enough time has elapsed, and consumes queued frame pointers before applying updates through 0xa7bfb0. Native state also contains timing correction, highlight, and seek logic. | Pausing the viewer and pausing generation are different operations. Setting speed zero does not establish that the worker stops. |
| Some generation loops hold an exclusive lock through a tick. | At 0xbe42b0: acquire writer-state pattern at 0xbe4398; run tick at 0xbe43f5; process output at 0xbe4448; release at 0xbe446b. A second loop at 0xbe4d40 has the same ordering. Their creation paths later store scene tag 11, corresponding to Tutorial5v5 in the SDK. | These paths demonstrate a lock hazard, but are not evidence that the user's normal match waits on this runner lock. |
| Output publication follows the tick, rather than preceding the AI work inside it. | In 0xbe42b0, subsequent shared-output replacement and releases appear at 0xbe455b–0xbe4594 and 0xbe467f–0xbe46b6, before the loop repeats. A different loop at 0xbe57d0 runs its tick at 0xbe58e6 and replaces shared output only afterward. | Holding the tick can prevent new output from reaching consumers even in a loop without the same runner lock. The exact foreground consumer and initial publication requirement remain unresolved. |

The stable SDK exposes callback-scoped simulation reads and player AI overrides.
The inspected client API does not expose a direct playback pause/resume command
or an operation that invokes the existing pause button's native action.

The SDK 0.5.8 archive was used for named research leads, including
`GameClient::pause_time`, `GameViewSharedConfig`, `GameView::add_frame`, and
`GameRunner::run_tick_ext`. Its relevant members are LLVM bitcode, not COFF
objects. The existing Rust LLVM linker translated selected members into static
assembly with CodeView type records. No translated object was loaded into the
game. This recovered field names and helped distinguish `GamePlayView` from
tutorial state. Older layouts and addresses are not accepted as current-build
evidence; current instruction sequences are checked separately.

## Follow-up: normal viewer and its candidate generator

The next pass connected the ordinary `game_view` storage to its receiver and
playback consumer. This is substantially stronger than identifying arbitrary
functions that call `run_tick_ext`, but it is still a static trace rather than a
runtime identification of the worker in the sixth recording.

| Stage | Current-build trace | What it establishes |
| --- | --- | --- |
| Worker creation | Packet-processing function 0x977b70 calls thread creation 0xbeafc0 at 0x97bfcd. The thread callback table leads to 0xc045b0 and the owned-runner loop 0xbe57d0. | A generator is created beside the normal viewer's frame channel and shared output. The precise packet variant and match-origin assignment still need to be mapped. |
| Viewer installation | At 0x97c04d–0x97c0d7, the creator copies a 0x468-byte client, adds the receiver and two shared-output references, and installs the 0x488-byte aggregate at database allocation +0x960. | This matches the older SDK's named `GamePlayView` aggregate. The current producer and consumer agree on its storage and fields. |
| No thread join at that site | The import called at 0x97c00d is `kernel32!CloseHandle`. The subsequent code drops thread-handle references and installs the viewer. | This particular call is not a wait for the simulation thread to complete. It does not rule out other startup dependencies. |
| Normal receiver | In 0x9f7b90, 0x9f81ca takes the client at database +0x960; 0x9f81d8 takes its receiver at +0xdc8. The receiver call at 0x9f85eb appends frame output to the client's recorded-frame list. | The receiver's offset equals client start +0x468, consistent with the aggregate installed above. This is distinct from tutorial receiver code at 0xa8d050. |
| Queue and playback | 0x9f8a9c–0x9f8b85 refills the playback queue with available recorded frames, stopping at 100 or when no next frame is available. 0x9f8d11 calls the playback update 0xa8b090 with this normal client. | The value 100 limits refill; it is not a demonstrated loading threshold. Receipt, buffering, and playback are separate stages. |
| Publication and possible wait boundary | In 0xbe57d0, the tick returns at 0xbe58eb. Output locks are released at 0xbe5a5e and 0xbe5b7d, the frame is sent at 0xbe5bc4, and send-result cleanup completes before the back edge at 0xbe5bfd. | The normal-success path after publication/cleanup and before the next tick is a candidate coordinator boundary. The tick-return site alone occurs too early. This does not yet establish an installable hook or exit/cancellation behavior. |

The owned-runner loop does not use the shared runner write-lock pattern seen
in the two tutorial loops. Therefore, the earlier explanation that our wait
necessarily held the normal loader's runner lock is too strong. What remains
established is that an AI-callback wait prevents the current tick from returning
and publishing its output. The specific condition that delayed the user's
normal loading screen has not yet been recovered.

Normal scene/UI processing tests whether `game_view` exists, and several
loading-action branches also check pending operations and scene flags. Those
presence checks are not a complete definition of loading readiness. In
particular, this pass did not prove a requirement for 60 ticks, 100 frames,
simulation completion, or any fixed wall-clock allowance.

There is also an initialization branch inside the tick routine: it checks the
runner's byte at +0x208a, sets it, and performs initialization before continuing
the tick. Its offset agrees with the older named `is_init` field. Thus merely
stopping before the first tick is not yet proven to leave a renderable match.
We still need to distinguish initial-state publication from gameplay advancement.

## Corrections to earlier leads

- The first-byte branch in the playback payload at 0xa8b090 bypasses a timing
  adjustment branch. It has not been established as the pause switch.
- The helper at 0x1fcfb60 handles view configuration/camera requests; it is not
  the recovered simulation-publication boundary.
- `set_feedback.rs` functions process between-set feedback/replay paths. They
  do not establish the normal match's loading requirements.
- `ExpectedGame` is not evidence of a future-frame queue. Likewise, helpers
  first suspected of receiving frames were hash functions.
- Neither Harbinger's `ready_tick=60` nor our tick-63 readiness recording proves
  that exactly one second of simulation is required for loading.
- The shared-runner tutorial paths cannot explain the normal match's timeout
  without additional evidence. The actual recording never entered Tutorial5v5.
- 0xa8d050 consumes viewer state embedded in a scene object; its sole caller
  passes database +0x13d8 (the scene), rather than the normal `game_view` at
  +0x960. It must not be labelled the ordinary `GamePlayView` update.
- Filenames such as `normal-match-lifecycle.asm` are provisional research labels.
  The normal storage/receiver trace above is the relevant evidence, not those
  filenames.

## Proposed approach

Keep the stable SDK for champion state, input validity, and AI overrides. If
further investigation confirms the necessary boundaries, use a narrowly scoped,
build-checked native integration for startup and generation pacing. This private
integration would need revalidation when the executable changes.

The required lifecycle is:

```text
Saved game loaded → current team and athlete identity resolved
    → champion choice confirmed for the upcoming match
    → match initializes while presentation remains paused
    → initial state published; worker releases shared resources
    → viewer loads and acknowledges the same match / initial state
    → READY, with no gameplay time advanced
    → explicit Start → generation and presentation advance together
```

Any wait for display readiness must happen outside the AI callback and outside
the relevant locks. AI callbacks should resolve current orders and return
promptly. A generation coordinator should limit how far the simulation gets
ahead of the displayed state, so cursor commands affect the scene the user sees.
The final acceptable lead requires in-game publication and command measurements.
Probe 0.7 uses two unconsumed frames as an experimental limit; its actual input
latency and displayed timing still require verification.

The loading phase must allow resource loading, event processing, initial-state
publication, and cancellation to continue. A paused clock must not prevent
these tasks from finishing. Merely increasing the timeout or allowing more AI
ticks does not establish that property.

Starting from 00:00 means preserving the simulation's initial state as well as
the displayed clock. Letting native AI simulate 10–15 seconds, pausing, and
rewinding the viewer would leave future decisions already computed. It would
not satisfy the requested behavior unless the simulation itself were correctly
restored and subsequent output regenerated.

Team ownership must come from the loaded save. Match side, lane, player index,
and champion must come from the current match roster. Blue/red is not a fixed
ownership convention. The earlier title-screen cache and blue-side assumption
must not reappear in the integration design.

## Closed investigation and next-build decision (2026-10-01)

The user's final-investigation limit closes this static pass. Probe 0.7.0 now
implements the publication-boundary gate and scoped native playback override
described in [native-adapter.md](native-adapter.md). The normal worker publishes
and releases its output locks before returning to the next `run_tick_ext` CALL.
The native consumer call receives the live view and already-borrowed mutable
config; no opaque stable context is reinterpreted. Its first initialization call
also executes ordinary gameplay logic, so an untouched simulation state is not
assumed. The first publication, queue application and native played tick are
recorded separately.

The Match branch was traced through its stage dispatch (`0x3ac498c`) and UI
progress/fade work, including a stage change at `0x9fdfe4`. Generic loading paths
check loading visibility, pending work, and viewer availability. These do not
establish a fixed required frame count or the complete readiness predicate.
That question is tested directly by publishing/applying one frame and logging
the acknowledgements. Remaining uncertainty is carried into this build as
diagnostics; it is no longer a reason to extend this investigation before building.

## Proof obligations retained for the playable milestone

1. Confirm the native publication boundary is observed on the same thread/run
   identified by the stable SDK as ClientMatchView. Static address labels alone
   are insufficient; probe 0.7 logs both observations.
2. Trace the initial frame publication and loading transition: can the viewer
   become ready with an initial state and no advancing gameplay ticks? Resolve
   the exact pending operation/scene condition, rather than inferring a buffer
   size from a queue-refill loop.
3. Verify that the implemented wait occurs after publication in-game and that
   explicit Start, frame-lead limiting and cancellation behave as intended.
4. Verify scoped playback overrides and initial-frame application keep the
   battlefield stopped through loading, with its exact tick recorded.
5. Establish how controlled output reaches the actual match result and how exit,
   cancellation, and scene changes release the integration.

Consequently, a true 00:00 start and a playable direct-control mod remain
unproven. This investigation supports changing the coordination architecture;
it does not establish a playable mod. Probe 0.7 is an experimental native
integration with guarded installation and fail-open coordination, requiring
game verification at the newly traced boundary.

## Reproducible research artifacts

`tools/trace_pe.py` verifies instruction references against decoded function
starts. `tools/disassemble.py` writes static disassembly. `tools/inspect_game.py
--all-paths` expands the earlier source-path search; the new all-paths map has
the same executable hash as the original map. `tools/inspect_rlib.py` reads SDK
archive symbol names and explicitly rejects LLVM members in COFF-body mode.
`tools/read_sdk_debug.py` extracts archived bitcode members and reads named
structure fields from the linker's textual CodeView output. It does not turn
older types into a current-build memory-writing API.

Relevant files under `research/` include `game-0.6.2-all-paths.json`,
`gameclient-playback-controls.asm`, `native-pause-binding.asm`,
`native-pause-handler.asm`, `native-pause-callers.json`,
`view-startup-candidates.asm`, `runner-loop-candidates.asm`,
`generator-thread-entry.asm`, and `generator-reference-graph.json`.
Follow-up artifacts include `frame-worker-startup.asm`,
`client-scene-update.asm`, `view-update-callers.json`,
`gameplay-view-update-callers.json`, `runner-worker-vtable-references.json`,
`frame-worker-vtable-references.json`, `old-view-fields.json`, and
`old-client-data-fields.json`. The file `gameplay-view-update-callers.json`
records a tested candidate which turned out to receive scene state; the filename
does not establish its meaning.

The `.asm` files match the project's `.gitignore` patterns; this directory is
not currently a Git repository. Generated SDK `.bc` and `.s` files are local
research artifacts, not distributable mod assets. Filenames throughout research
are labels, not verified semantic function names.
