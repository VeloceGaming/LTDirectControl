# Direct-control investigation

Investigated: 2026-09-30 (Asia/Taipei).

## Reference mod

[Harbinger Direct Control](https://steamcommunity.com/sharedfiles/filedetails/?id=3807134574)
by patrickehopkins, installed version 0.1.2. The Workshop description lists
Windows/Steam, game 0.6.1, and single-player compatibility.

Its stated controls are Ctrl+Home to start, F1–F10 champion selection, contextual
right-click movement/attack, A then left-click attack-move, Q/W/R abilities,
H hold, B return, middle-button camera dragging, wheel zoom, End temporary AI
release, and Ctrl+End permanent release.

The description warns about native spectator key conflicts. It also identifies
champion-specific behavior, such as Gunfighter attacking while moving, that a
generic implementation must preserve. Manual shopping and AI pings are listed
as planned, rather than existing capabilities.

The screenshot provided by the user suggests a bottom-centre Q/W/R bar with
cooldowns/resource information, a KDA/CS strip, an AI/player control toggle,
camera/settings controls, and a bottom-left inventory/gold area. This is visual
reference evidence; it does not establish how the unreleased implementation
works internally.

## Verified reason Harbinger fails here

The installed game log at
`C:\Users\j9010\AppData\Roaming\TeamSamoyed\TeamfightManager2\data\log.log`
contains these two messages at 20:04:56 on 2026-09-30:

```text
[stable mod] TFM2 Direct Control replay safety gate FAILED: unsupported replay action lookup build: timestamp=0x6ABC597E, image=0x052B8000
[stable mod] TFM2 Direct Control failed to initialize the supported simulation hook: unsupported TeamfightManager2.exe build (timestamp=0x6ABC597E, image=0x052B8000)
```

Reading the current executable's PE headers independently confirmed the same
timestamp and image size. Steam app 3009300 reports build ID `25616391`; the
installed SDK's `base_version.txt` reports `0.6.2`.

Executable SHA-256:
`15DF9EB3B6915CDCC4C2EBB3B7F5FA232B563C4CDD32208581634817B71ADC23`

The DLL contains the stable entry symbols `tfm2_mod_entry_stable` and
`tfm2_mod_required_abi_level`. It also contains diagnostic strings for simulation,
replay, and camera detours, build checks, and signature mismatches. Together with
the log this supports the conclusion that its stable mod entry loads, while its
additional build-specific hooks reject the current executable.

This is not evidence of the precise 0.6.2 instruction addresses or structure
layouts. It is not sufficient to patch the hooks safely. Editing metadata or
removing the unsupported-build check would not supply those missing addresses.

## Resources already available

| Resource | Location |
| --- | --- |
| Project | `D:\LTTM2\LTDirectControl` |
| Game | `C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2` |
| Current SDK | Game directory, `mod-sdk-stable` (base version 0.6.2) |
| Reference package | `C:\Program Files (x86)\Steam\steamapps\workshop\content\3009300\3807134574` |
| Existing reference diagnostics | Game data directory, `harbinger-diagnostics.log` |
| Older local docs | `D:\LTTM2\docs` (verify against current SDK before use) |
| Rust | `C:\Users\j9010\.cargo\bin` (rustc/cargo 1.98.1 detected) |

The reference Workshop package has metadata, preview images, and the compiled
DLL, but no source files. No public source repository was located during this
initial search. This does not establish that none exists.

Official documentation:

- [Modding guide](https://github.com/teamsamoyed/TeamfightManager2Mod)
- [Stable API reference](https://github.com/teamsamoyed/TeamfightManager2Mod/blob/main/docs/stable-api-reference.md)

The official guide says the classic SDK is no longer shipped or updated from
0.6. Older local classic SDKs are not an appropriate 0.6.2 build foundation.

## Supported API versus missing integration

Inspection of the installed 0.6.2 SDK found:

- Client lifecycle hooks, raw keyboard press/release events, custom UI, and
  overlay drawing.
- Simulation/player reads, including positions, health, cooldowns, items, KDA,
  and CS, inside simulation callbacks.
- A final player-input AI replacement hook and input validation.
- Simulation-origin information to distinguish foreground/background work.

The installed public client wrapper does not expose a live-match takeover,
simulation pacing/rewind controller, native pointer targeting, or a full
runtime ability descriptor for every existing champion. Simulation reads are
callback-scoped; they are not a retained foreground-match handle.

Therefore keyboard events plus `StablePlayerAi` alone are not a demonstrated
solution. The timing and identity of the controlled simulation must be solved
first. In particular, feeding shared UI state into background pre-simulation
would not establish that the on-screen match or persisted result is correct.

Use the stable API for supported features. Isolate any necessary executable
hooks in a small version-specific adapter, with executable identification and
instruction verification before activation. Do not reuse old addresses or
disable Harbinger's build checks as a substitute for identifying new hooks.

## Implementation order and acceptance checks

### 1. Establish the live-match adapter

The user confirmed no source/demo is available. Independently investigate the
foreground simulation path. Locate pacing, input ownership, presentation synchronisation,
and the path by which played results become authoritative. Confirm that normal
replay playback and background simulation can be distinguished.

Acceptance: right-click movement affects a continuing match; other players keep
native AI; match outcomes reflect the played simulation; unsupported executable
builds leave direct control inactive with a clear diagnostic.

### 2. Implement core controls

Pre-match champion selection, contextual move/attack, Q/W/R, hold, recall, attack-move,
and release to AI. Route actions through native validation rather than
reimplementing combat rules. Handle death, respawn, changing champion, target
death, scene exit, loss of focus, and held keys being released outside the game.

Acceptance: movement cannot accidentally become an ability cast; ally/enemy
targets follow each ability's native targeting rules; pending commands do not
transfer to another champion or match; Gunfighter and other exceptional
champions retain their native mechanics.

### 3. Add preview and cast preferences

Confirmed: Shift + Q/W/R previews only, with no cast on release. Normal Q/W/R
quick-casts toward the cursor; targeted skills resolve a valid unit under it.
Attack-move must choose the eligible enemy nearest the click, and a champion-only
modifier filters unit targeting. Mid-match switching is low priority.
Obtain actual runtime
range, level scaling, shape, targeting rules, and patched values. Unknown or
unsupported ability geometry should be identified honestly rather than shown
as an invented accurate circle. Previewing must not spend resources or trigger
an ability unless the chosen interaction explicitly includes casting.

Acceptance: cooldown/resource state matches native behavior; Escape cancels;
previews clear on champion changes, death, focus loss, and match exit; targeted,
directional, position, self-cast, and multi-stage abilities are handled correctly.

### 4. Build the HUD and camera controls

Implement the supplied layout's useful components, resolving icon assets and
scaling for different resolutions. Add camera follow/temporary unlock and
settings for bindings. Suppress conflicting spectator actions only while direct
control owns the match. Avoid hiding unrelated spectator UI until needed.

Acceptance: HUD elements do not intercept unintended battlefield clicks;
settings and item information remain readable; native shortcuts resume after
release; controls work with changed spectator bindings.

### 5. Package a personal test build

Keep editable source and build instructions in this project. Package the DLL
with a distinct mod ID and honest supported-version metadata. Test a disposable
match/save before asking the user to playtest responsiveness and preferences.
Workshop publication is a separate future request.

## Remaining uncertainty

The 0.6.2 hook locations and data layouts, live-result persistence, and complete
runtime ability geometry are not yet established. The user confirmed independent
implementation is required. Initial PE inspection located candidate simulation
and presentation functions via Rust source-location anchors; these addresses
are research leads, not verified hook sites. A passthrough stable-API diagnostic
probe has been built to observe runtime simulation origins before any detours.

## First runtime observation

The user completed the first test on 2026-09-30. Probe 0.1.0 loaded on 0.6.2 with
host ABI 9. The retained recording is `research/probe-2026-09-30-first.log`; its
summary is `research/probe-first-summary.json` (generated by `tools/analyze_probe.py`).

Origin counts: Unknown 3263, ServerPresim 200 (capped), ClientMatchView 46.
Foreground match 33/set 1/seed 11704687984684596155 advanced from tick 1 to tick
33600 in 6.643 seconds: 559.983 simulation seconds at the default 60 ticks/second,
or 84.297 times wall-clock speed. Native roster and cooldown reads worked.

This confirms foreground identification, not live pacing or result authority.
The first UI scan returned empty because it queried a nonexistent `body` node.
The SDK explicitly supports empty-path root child enumeration; the revised
probe uses that and queries the native game-time labels. Unknown-origin samples
are now capped as well as server samples.

Probe 0.2.0 uses the supported AI callback to attempt a maximum 20-second pacing
experiment on one ClientMatchView worker, without returning any input overrides.
It compares callback thread identity against the client display/update thread,
stops if they coincide, and stops when the client heartbeat is missing or older
than 250 ms. Individual waits are at most 2 ms; the budget uses monotonic time.
This test is optional via `pacing-test.enabled`. It will establish whether the
viewer tolerates incremental frame generation or needs a separate pacing hook.
No executable detour has been installed.

## Second runtime observation

Probe 0.2.0 recorded worker ThreadId(19) and display ThreadId(1). The timing
window started at tick 1 and stopped for MissingHeartbeat at tick 15 after
226612 microseconds. The first sampled battlefield clock appeared 988 ms
after worker start, already with a latest sampled worker tick of 4200. Playback
continued normally from 00:00 to 00:37. The user confirmed normal battlefield
movement and a visible diagnostic line.

The retained recording is `research/probe-2026-09-30-second.log`; the summary
is `research/probe-second-summary.json`. This is not evidence of successful
pacing: the strict guard released during startup. Whether this pause came from
ordinary asset loading or a dependency on generated frames is still unknown.

Probe 0.3.0 permits up to two seconds of startup loading before the first actual
battlefield callback. It never permits a missing client thread/heartbeat, never
sleeps on the display thread, and restores the 250 ms heartbeat limit once the
battlefield is seen. Battlefield exit also releases the worker. The overall
20-second budget is unchanged. Logging samples each initial foreground
simulation second and records the tick at which battlefield readiness is seen.
Tests cover the recorded startup gap, strict post-readiness heartbeat behavior,
finite startup timeout, and exit release. All 20 core/probe tests pass.

Read-only disassembly of source-anchored view functions is retained as research
via `tools/disassemble.py`; these remain unverified candidates. No executable
patch or private structure access has been implemented.

## Third runtime observation and movement prototype

Probe 0.3.0 successfully paced foreground match 33/set 1, seed
17832636336880816315, on worker ThreadId(15). It observed battlefield readiness
at tick 63 after 1.030218 seconds; the display thread was ThreadId(2). It reached
tick 1201 and stopped for Finished after 20.000378 seconds, advancing exactly
1200 ticks (20 simulation seconds). The paced simulation/wall-clock ratio is
0.999981. The user observed the test stopping at displayed match time 0:19;
the recording agrees. Native display labels continued advancing during pacing.

The archived log is `research/probe-2026-09-30-third.log`, with summary
`research/probe-third-summary.json`. Startup did not require the entire match
to be generated. The visible clock starts about one second after generation,
so the current worker has a roughly one-second lead; exact input/display delay
is not measured by whole-second labels. This is the first successful supported
incremental-pacing experiment, not proof of result persistence or cursor input.

Probe 0.4.0 adds a finite movement experiment enabled by `movement-test.enabled`
alongside the timing marker. The budget becomes 60 seconds. Ctrl+1..5 selects
one blue lane before simulation starts; selection locks at the initial worker
callback. I/J/K/L polls use public Windows keyboard/focus APIs only while the
game window is foreground. Owned directions cross threads with a 250 ms lease;
no host context, player/entity handle, or pointer is retained. Held directions
produce a bounded offset from the selected alive champion's current position,
then pass through `StableAiContext::is_valid_input`. Invalid inputs, key release,
focus loss, stale client samples, scene exit, timeout, and other match identities
preserve native AI. Other players always retain their native input.
Pacing runs from each registered live player's callback, because a dead player's
AI callback may be absent. Repeated callbacks at the same tick do not add a new
delay. A period with no AI callbacks at all remains an integration limitation
to investigate for complete control of a full match.

This experiment deliberately verifies movement before mouse projection or
ability descriptors. It has no attack-move, quick-cast, range preview, or final
HUD integration yet. Native outcome and replay persistence remain unknown.
All 24 targeting/timing/probe tests pass; Clippy and release building pass.

## Fourth runtime observation and startup barrier

Probe 0.4.0 stopped for MissingHeartbeat at tick 120 after 1.976405 seconds.
The first sampled battlefield clock appeared 2.603 seconds after worker start,
beyond the two-second loading allowance. Key presses were recorded with game
focus, but no manual inputs were dispatched. The retained recording is
`research/probe-2026-09-30-fourth.log`, with summary
`research/probe-fourth-summary.json`.

The user also reported that their team was red. Existing read-only exporter
records identify the human team as team 7, Cute and Stunny; the selected blue
athlete belonged to team 3. Blue-side selection was therefore incorrect. SDK
athlete contracts expose ownership through `InContract.team_id`.

Probe 0.5.0 resolves ownership using the public human team ID and athlete contract
records, then binds the chosen lane to the simulation's athlete/player identity.
Unknown or ambiguous ownership leaves native AI active. Ctrl+1..5 works on
pre-match management, lineup, stadium and draft screens; accepted selections and
attempts after locking receive separate visible confirmations.

The installed stable SDK has no direct playback pause/resume or native button
invocation API. UI state setters cannot invoke the pause button's action. The
new experimental barrier allows ticks 1–2, then holds further foreground worker
generation until a battlefield heartbeat arrives. It starts the running budget
at that readiness signal, rather than consuming it during loading. A five-second
startup limit releases the worker with StartupTimeout if readiness depends on
more generated frames or a resource held by the AI callback. It never waits on
the display thread. This is a bounded generation barrier, not a verified native
playback pause hook; the next game test must establish whether it can load.

All 29 targeting/timing/probe tests pass, including ownership on either side and
a loading gap longer than the running test budget. Clippy and release building
pass. Actual movement, complete-match pacing and result authority remain pending.

## Fifth runtime observation and reference synchronization research

Probe 0.5.0 started at tick 1 and held at tick 3. It stopped for StartupTimeout
after 5.000651 seconds with running_us=0. The first sampled battlefield clock
arrived 520 ms after that release. Holding two frames did not allow startup.
This supports a generation/publication or held-resource dependency; it does not
distinguish the two. The log and summary are retained as
`research/probe-2026-09-30-fifth.log` and `research/probe-fifth-summary.json`.

The ownership lookup occurred before management data was loaded: team 0,
fallback name "team 0", empty athlete set. Caching it prevented later resolution.
The Ctrl+3 selection itself was accepted. Probe 0.6.0 restricts ownership reads
to the top-level InGame scene and caches only a real name with a nonempty owned
roster; incomplete reads retry. A regression test covers the recorded placeholder
followed by a valid team 7 roster.

The user supplied a Harbinger 0.6.1 log with separate READY, synchronized and
RUNNING states, ready_tick=60 and explicit Ctrl+Home activation. This is evidence
of its lifecycle design, not a proof of its implementation or input latency.
Read-only inspection of its installed DLL located synchronization checks and
speed-button state changes. Its stable AI callback waits relative to a captured
tick, with native detour diagnostics also present. No old hook was activated,
copied into our DLL, or modified to accept 0.6.2.

The reference synchronization routine compares the visible whole-second clock
with a captured tick divided by 60 and changes spectator speed-selection states.
The exact effect of those UI writes needs runtime verification. Thus the absence
of a direct SDK pause command does not rule out every indirect integration.
Conversely, successful selectable-state writes alone do not prove playback paused.
Current-game UI disassembly shows selection flags being refreshed from native
view state. The remaining investigation must trace actual time advancement and
frame publication, rather than assume visual state owns either.

Static research artifacts include `reference-sync-strings.json`,
`reference-sync-globals.json`, `reference-sync.asm`, `reference-worker.asm`,
`reference-presentation-helpers.asm`, `game-playback-strings.json`,
`game-playback-update.asm` and `simulation-runner.asm`. Source anchors identify
the current game's ingame UI update candidate at RVA 0x26c7cc0; they do not
establish a callable ABI or a verified hook site. The scripts remain read-only.

Probe 0.6.0 tests a 60-tick bootstrap before holding at tick 61, informed by the
earlier successful pacing test's readiness at tick 63 and the reference log.
This is explicitly an unverified buffer hypothesis, not a recovered native
publication boundary. The five-second startup guard and readiness-based running
budget remain. Speed selections are observed for research, not changed by this
build. All 30 core/probe tests pass; build-specific live integration is still
pending.

## Sixth diagnostic and subsequent read-only investigation

Probe 0.6.0 resolved the user's mid-lane crossbowman on RED, but held at tick 61
and stopped for StartupTimeout after 5.000924 seconds with zero running time.
The first display clock arrived 610 ms after release. The recording is archived
as `research/probe-2026-09-30-sixth.log`, with `probe-sixth-summary.json`.

After the user requested discussion before more implementation, further probe
builds, installations, and playtests stopped. The subsequently authorized work
was static investigation. See [startup-research.md](startup-research.md) for
native playback action evidence, generation-lock/publication ordering,
corrections to discarded leads, and the proposed lifecycle. This evidence
supports moving coordination outside AI callbacks; it does not yet prove the
normal foreground loading boundary or feasibility of a true 00:00 start.

## Static follow-up: normal viewer separated from tutorials

The follow-up connected the current executable's normal `game_view` installation
at database allocation +0x960 to its frame receiver and playback consumer inside
0x9f7b90. The adjacent thread creation leads to the owned-runner loop 0xbe57d0;
mapping its exact packet/match-origin assignment remains pending. That loop
publishes shared output, releases its output locks, sends the frame, and cleans
up before repeating. This supplies a candidate wait boundary outside the AI
callback, rather than a verified native hook.

The two shared-runner loops previously inspected are created in paths storing
Tutorial5v5 scene tag 11. The sixth recording instead traversed Match → InGame
with ClientMatchView origin 2. We cannot attribute its loading failure to the
tutorial runner lock. Likewise, the normal viewer's refill limit of 100 frames
does not establish a 100-frame loading requirement. Exact loading readiness and
initial-state publication without gameplay advancement are still unresolved.

Selected older SDK LLVM members were translated to static annotated assembly
using the existing Rust linker. Named CodeView types guided current-build
instruction checks; no old layout or callable ABI was adopted. This work added
research scripts/artifacts and documentation only. It did not build/install a
new mod or operate the game.

## Final bounded pass and probe 0.7.0 (2026-10-01)

At the user's instruction, static investigation ended and a native diagnostic
was built and installed. It redirects the verified normal worker CALL before
its next tick (after previous-frame publication) and the normal playback CALL.
Stable AI callbacks identify the current run and return without waiting. It
publishes/applies one initialization frame, attempts paused loading, and requires
explicit Ctrl+Home Start after separate publication, application, battlefield
and owned-champion acknowledgements. Ctrl+End releases directly from the worker
boundary. No title-screen ownership or blue-side convention is used.

Eighteen core tests and fourteen probe tests passed, along with Clippy denying
warnings, executable relay/argument forwarding checks, SHA256 verification,
scoped config restoration, finite coordinator guards, and DLL ABI/null-host
loading checks. The release DLL and metadata were installed and their hashes
verified. The previous installed 0.6.0 package is retained in
`dist/backups/0.6.0`; `dist/build-0.7.0.json` records the new package fingerprints.
No game match was operated during this build.

The native first tick includes initialization and ordinary simulation logic.
An untouched 00:00 simulation is therefore unproven. The exact loading predicate
and final saved-result authority remain game-test obligations, rather than
reasons to keep investigating before building. See `TESTING-7.md` and
`docs/native-adapter.md` for the concrete test and implementation limits.

## Seventh result and probe 0.8.0 runtime correction

The user reported the battlefield never stopped. The preserved seventh log
confirms installation reported success and the SDK bound player 7, athlete 56,
RED mid lancer. It contains no matched native publication/viewer event or READY.
Tick 1 to 34800 spans 6.319 wall-clock seconds, approximately 579.983 simulation
seconds at 60 ticks/s. The displayed timer continued from 00:00 to 00:45.

The old overlay's "Loading paused" was an unsupported claim. The startup deadline
was only checked in native paths, so an unmatched path could leave Loading
indefinitely. Probe 0.8.0 fixes both: client updates check the unchanged deadline,
overlay states describe acknowledgements, and READY additionally requires a
held update with zero consumed frames and unchanged native played tick.

To locate the actual executing path, 0.8 adds unfiltered entrance counters,
sampled rejection reasons, consistent Windows OS thread IDs, coordinator identity,
CALL-byte readback/audits and bounded live call stacks at SDK AI ticks 1/2 and
client scene transitions. It retains the candidate CALL addresses. The next
runtime recording must distinguish unused candidates from rejected calls or
modified patches; no successful pause is claimed yet. This is a targeted runtime
diagnostic correction, not another broad static investigation.

Eighteen probe tests and eighteen core tests passed; Clippy denied all warnings,
DLL loading/ABI/null-host checks passed. Probe 0.8.0 was installed and its DLL and
metadata fingerprints verified. The 0.7.0 package is backed up in
`dist/backups/0.7.0`; the seventh log and summary are retained in `research/`.
`TESTING-8.md` describes a roughly 20-second test if no pause occurs.

The user requested a custom way to start playing and return control to AI after
the mod works. Original Harbinger interactions are not requirements. Current
hotkeys are temporary diagnostic controls; user experience work is deferred
until the live-control milestone.

## Eighth result and probe 0.9.0 foreground-path correction

The user saw Waiting for live worker hook, followed by the independent 15-second
startup timeout. Both unconditional native entrance counters remained zero and
patch audits continued to match. Tick 1 to 45600 spans 8.337 wall-clock seconds
(759.983 simulation seconds), so this recording still did not establish live
control. The deadline released at 15,001 ms with zero counted publications.

Recorded foreground AI return addresses identified the shared worker be42b0,
with tick CALL be43f5. Earlier static research associated a tutorial caller with
this loop and incorrectly treated it as exclusive. The normal match uses it too.
Focused disassembly shows be43f5 owns the runner write guard; probe 0.9 therefore
intercepts the later successful frame-send CALL be470a, after runner, highlight
and segment write guards have been released. Original send runs before the gate.

The SDK client stack identified b1fb00. Probe 0.9 selected playback CALL b240e9
from pointer flow near SDK post_update at b2f909. Test 9 later disproved the
classification: this hook received zero calls. Being inside the SDK's enclosing
update did not establish that the call played the visible battlefield. Current
calling conventions and fingerprints had matched, but those checks could not
prove execution. This was machine-code inspection, not game operation under
a debugger.

The source correction retains independent startup deadlines, scope/ownership
checks, patch audits, and playback pause acknowledgement. Single-frame loading,
actual movement and result authority still require the ninth game test. See
TESTING-9.md. The eighth log and parsed summary are preserved in research/.

Validation: 18 core and 18 probe tests passed, Clippy all targets with warnings
denied passed, release DLL loading reported ABI 6 and rejected a null host.
The two new CALL instructions and function-entry fingerprints matched the
installed executable. Probe 0.9.0 was installed and both package hashes matched;
0.8.0 is preserved in dist/backups/0.8.0. No game match was operated during this
correction. The ninth runtime result and next correction follow below.

## Ninth result and approved probe 0.10.0 playback correction

The user reported the battlefield stopped initially and then continued with
Startup interception not confirmed. The recording confirms a successful worker
send, one published frame, and a hold until the independent 15-second deadline.
Viewer entrances remained zero throughout. Release occurred after 15,001 ms
with produced=1, consumed=0 and no initial application, PAUSE_ACK or READY.
This confirms worker interception, not successful playback coordination.

The user required discussion and explicit approval before any edit or build.
A focused read-only trace was completed and its proposed correction approved
before changing source. Disassembly of the installed 0.6.2 executable followed
the current SDK scene conversion (2e45930/table 3cec520), native tag 11 = InGame,
and dispatch (9f8f73/table 3ac4790) to branch 9f9c1c. Older SDK internal scene
numbering differs. Outer update b1fb00 calls this scene handler at b2226e.

The InGame view is database+13e0. Receive CALL 9fbbe5 reads the frame channel
at database+1908; frames enter the received collection and then the view queue.
Playback CALL 9fdaf0 invokes a8b090 on this view using the existing nine-argument
convention and scoped config borrow. Playback accepts one queued frame; the
100-frame refill cap is not a requirement for starting playback.

Probe 0.10 replaces only the playback CALL site and its expected instruction
bytes. Worker gating, bootstrap/held-update acknowledgement, deadlines and
movement behavior are unchanged. Documentation now distinguishes this static
trace from live game proof and retracts the old display-path assertion.

Validation: 18 core and 18 probe tests passed; Clippy all targets with warnings
denied passed; release DLL loading reported ABI 6 and rejected a null host.
The executable SHA256, both CALL targets/bytes, both function prologues and
current InGame dispatch mapping were verified. The 0.10.0 DLL and metadata
were installed while the game was closed and their hashes matched the package.
The previous package is in dist/backups/0.9.0 and the recording is archived as
research/probe-2026-10-01-ninth.log. See dist/build-0.10.0.json and TESTING-10.md.
Live playback entry, paused loading, movement and saved-result authority remain
pending the next user game test. No match was operated during this build.

## Tenth result and approved probe 0.11.0 persistent input change

The 0.10 recording confirms viewer entry at played_tick=0 with one queued frame,
initial application at tick 1, PAUSE_ACK and READY in 61 ms. Explicit Start
occurred 12.757 seconds after READY; playback advanced at normal speed until
Ctrl+End released with produced=1268, consumed=1266, played_tick=1266. The user
confirmed the test looked good, but the champion mixed manual movement with AI.

Source and SDK inspection established a definite cause: zero-direction input
returned None, which means keep native input. Tick 150 shows request=None and
a native Move destination. After discussion, the user explicitly approved a
persistent-control correction and the next test build.

Current executable validation was traced through the GameRunner World vtable
3b9be28 (stored by constructor 172a048), SDK validation shim 2e61390 and native
validator 177bf90. Its Input Move branch maps through table 3ba5fc8 to 177c058
and returns true for a living champion without comparing destination distance.
The SDK/native shim converts Some(Move) into Replace with the supplied position.
Move-to-current-position is therefore a valid replacement, though static
validation does not prove cancellation of every actor-side attack/cast/order.

Probe 0.11 returns a hold destination at the champion's current position whenever
directions are zero, missing or expired. Short heartbeat gaps do not relinquish
ownership; the original two-second session guard still explicitly releases.
Rejected commands release with a reason. Hold/move transitions are logged.
Other players, background matches and dead/absent champions retain native input.
Startup hooks, deadlines and the 60-second test limit remain unchanged.

Validation: 18 core and 20 probe tests passed, including key-release destination
replacement and short-stall ownership/explicit heartbeat release. Clippy all
targets with warnings denied passed and the release DLL built successfully.
Test 11 checks initial idle behavior, movement then release of keys, unsolicited
attacks/casts near minions and explicit restoration of AI. Persistent ownership
and saved-result authority remain pending user game verification.

The 0.11.0 package also passed DLL loading (ABI 6; null host rejected). It was
installed with the game closed; both installed file hashes match the package.
The previous package is preserved in dist/backups/0.10.0; the recording and
summary are research/probe-2026-10-01-tenth.log and probe-tenth-summary.json.
See dist/build-0.11.0.json for fingerprints and TESTING-11.md for the test.


## Eleventh result and approved probe 0.12.0 mouse/camera build

The user observed idle stopping but 1–2 seconds of continued travel after
releasing diagnostic keys. The log records READY in 114 ms, correct red mid
selection, and valid hold inputs while position continued changing. Static
reverse engineering read the installed PE using Capstone disassembly, following
native input application, entity movement, action dispatch and stop events. It
found the near-destination early return described in docs/native-adapter.md.

After discussing priorities and camera behavior, the user approved ground
right-click movement, explicit stop and accurate cursor targeting as the camera
moves/zooms. Version 0.12 replaces diagnostic steering with persistent click
destinations, adds the selected-actor native stop hook, and integrates free/Y
lock/Space follow/middle drag/edge camera movement. Native minimap and zoom stay
in use. Current renderer disassembly established the texture crop and picking
scale; the SDK has no camera getter, so the existing viewer borrow supplies it.

The eleventh recording and parsed summary are archived in research/. Validation:
18 core and 26 probe tests pass, including crop/pan/zoom mapping, native pan
unlock, paused dragging, persistent click destinations, selected ownership,
movement-only stop events and native argument forwarding. Clippy all targets
with warnings denied and release compilation pass. Live cursor alignment,
visible stopping, unsolicited combat near minions and result authority still
require the user game test. See TESTING-12.md.

The release DLL exports ABI 6 and rejects a null host. Current executable
branch and native entry fingerprints match. Both files were installed with
the game closed and match package hashes; 0.11.0 is backed up. See
dist/build-0.12.0.json and research/mouse-camera-0.6.2.json. No live game test
was performed by the agent.


## Twelfth result and approved probe 0.13.0 camera/input corrections

The user confirmed right-click movement, S stops, Y lock and no unsolicited
champion actions near minions, but Space only snapped, information HUD caused
drag stutter and vertical edge scrolling was obstructed. S also triggers vanilla
spectator pause. The user accepted leaving movement pacing as it is, requested
hold-Tab team information, and kept manual shopping optional. The local SDK
exposes gold/items and one-time item build selection but no live purchase command.

After explicit approval, source inspection corrected drag capture and edge bands.
Read-only Capstone disassembly verified the normal InGame input route, keyboard
variant layout and follow camera payload. Native Follow takes team side and lane;
the 0.12 SDK player-ID interpretation was incorrect. Current native follow and
F-key shortcut code plus archived field names establish the corrected layout.
The input route a270cd shares the real battlefield pointer with playback; the
outer b234b6 route uses another view. Four branch redirects are now installed
only after fingerprint checks. See docs/native-adapter.md for addresses/scopes.

The twelfth recording and parsed summary are preserved in research/. Version
0.13 adds the corrected follow payload, continuous held-Space requests, captured
HUD-crossing drag, window-edge scrolling, scoped spectator key interception and
hold-Tab visibility restoration. It leaves movement pacing and the 60-second
guard unchanged. Full skill/gold HUD, attack-move, casting and shopping remain
future work. Validation: 18 core and 33 probe tests passed, Clippy all targets
with warnings denied passed, and the release DLL built. Live behavior is pending
TESTING-13.md; no game match was operated by the agent.

The release DLL exports ABI 6 and rejects a null host. All four branch bytes
and five native entry headers match the current executable. Version 0.13.0
was installed with the game closed; both installed hashes match the package.
The previous package is in dist/backups/0.12.0. See dist/build-0.13.0.json
and research/camera-hotkeys-0.6.2.json. Live test remains pending.


## 0.14 combat and HUD extension (2026-10-01)

The user reports 0.13 Tab, continuous Space/Y, camera navigation, edge scroll,
shortcut suppression and release work. The 0.13 log is preserved as
research/probe-2026-10-01-thirteenth.log. Ctrl+End released at worker boundary,
produced=1300 consumed=1298 played=1298, after about 23 seconds of manual play.

For this build, the installed bundle.game_data was read as its counted
extension/path/payload records. Current ingame.ui and relevant component
layouts were extracted to research, without altering game assets. The native
scoreboard template starts at x994/y64, width907/height300; wide_data separately
contains spectator cards, lower stats and auto-camera controls. SDK visibility
and layout property functions implement the scoped HUD overrides. The installed
0.6.2 zoom-key handler was disassembled read-only to confirm +/-0.25 steps and
0.5–3.0 limits. Existing fingerprinted viewer/input/movement/worker sites remain.

Combat snapshots use SDK entity ID/position/radius, alive/targetable/team and
own-team visibility. Commands use InputV1 Attack with InputTargetV1 Target and
native input validation, falling back to native Move when not yet accepted.
Native movement routine 15c4210 preserves action kinds above 2, including
committed attacks/casts, so chase/hold requests do not forcibly interrupt them.
There are no direct damage writes or native combat function calls.

The mod's initial acquisition radius is 120 world units around click or actor;
ranking is nearest click with stable ID tie-breaking. This is an explicit mod
policy, not a recovered native range. Sprite clicks use collision radius plus a
small UI margin. Right-click attacks retain one target, while attack-move retains
its clicked anchor and reacquires each simulation tick. Right-click/S/Esc/focus
loss/death/release clear the pending order. Live target selection, attack versus
chase behavior, HUD layout and wheel message delivery need TESTING-14.

## 0.14 result and approved 0.15 ability controls (2026-10-01)

The user reports all 0.14 checks work. The recording is preserved as
research/probe-2026-10-01-fourteenth.log. The next approved scope is Q/W/R
quickcasting and Shift-only range previews, with release never casting.

Literal work: read SDK CastingType/Target and input/validation/drawing APIs;
read counted records from installed bundle.game_data and extracted the champion
sheet and explicit mod-champion settings; disassembled current native effect
refresh, attack and Q/W/R application functions with Capstone. Native jump-table
mapping, POD Effect offsets, level growth and range bonus are established from
current code. Built-in settings often omit casting types, so there is no guessed
champion-name table. The refresh/consumer instructions are saved in
research/ability-native-current.txt; docs/abilities.md records the exact fields.

The existing move hook and an additional fingerprinted Attack CALL observer
copy scalar metadata on controlled ticks. No pointer leaves either borrow.
Quickcasting consumes a physical press once and validates its appropriate
native target form. Unit skills consider cursor sprite hits, including allies;
native validation resolves per-skill eligibility. Shift previews draw a nominal
cast-range ring and ground/direction aim aid without changing the current order.
Animation locks, level unlocks, recasts and target-dependent effect behavior
remain native. Effect impact shapes are not inferred. The running guard stays
60 seconds. Visible ability behavior and champion coverage need TESTING-15.

Validation: 18 core and 47 probe tests pass; both packages pass Clippy all
targets with warnings denied and formatting checks. The optimized release DLL
loads, exports ABI 6 and rejects a null host. All five branches and six native
entry headers match the fingerprinted executable. Version 0.15 was installed
with the game closed, and both installed hashes match the verified package.
Version 0.14 remains in dist/backups/0.14.0. See dist/build-0.15.0.json.
No game match was operated by the agent.

## Confirmed 0.16 and approved 0.17 player HUD (2026-10-01)

The user reports all 0.16 checks work, including persistent normal casting and
native recall. The log is preserved in research/probe-2026-10-01-sixteenth.log.
The user also observed hover tooltips from hidden spectator detail UI, and
explicitly approved a compact HUD pass with that minor cleanup included.

Literal work: read installed SDK stat/inventory, native portrait and UI APIs;
read installed ingame/detail/item UI templates; parse counted bundle records
for skill/item sprite-sheet metadata and item settings. A bounded read-only
disassembly of the native skill panel confirms default icon indices 0/1/2.
Embedded references respect explicit champion source/tag overrides and item
key aliases. No texture is shipped or modified. See docs/player-hud.md and
research/hud-assets-index.json / hud-native-icon.txt.

The bottom-center native UI panel receives only same-match/player SDK scalar
snapshots, every six ticks. Running freshness is 250 ms; paused READY keeps its
initial sample. It shows portrait, level, health, Q/W/R icons/cooldowns, gold,
six inventory slots, K/D/A, CS and action state. Its own brief hover hints and
panel rectangles join command/camera masks. Spectator toolbars and portraits
are hidden; known entry hover surfaces ignore events, and native stray tooltips
are suppressed. Release hides the HUD and restores spectator visibility and
the known template event defaults. Running diagnostics shrink with their mask.
No new native redirect or simulation mutation is introduced.

Validation: 18 core and 61 probe tests pass; both packages pass Clippy all
targets with warnings denied and formatting checks. The optimized DLL loads,
exports ABI 6 and rejects a null host. All twelve branch/header anchors match.
Installed 0.17 DLL/metadata hashes match the verified package, with the game
closed. Confirmed 0.16 is in dist/backups/0.16.0. See dist/build-0.17.0.json.
Native UI parsing, visible layout, icons and hover timing need TESTING-17.md;
no game match was operated by the agent. The first HUD uses English labels
and brief hints; richer descriptions/localization/manual shopping remain later.

## 0.15 result and approved 0.16 normal casting/recall (2026-10-01)

The user reports all 0.15 checks work on Lancer. Its Q requires an enemy under
the cursor, consistent with logged Targeting=0 / EnemyWithoutTower=6, range35000.
The recording is preserved as research/probe-2026-10-01-fifteenth.log.
The user revised Shift to persistent normal casting and requested B recall,
then explicitly approved implementation.

Literal work: read SDK Return input and validation; disassembled native Return,
Move, action dispatcher and return-cancellation event with Capstone. Return
starts action 1; current-position Move leaves it running. Explicit cancellation
uses native event 17069c0 plus action 1 -> idle on the already verified selected
actor borrow. No timer/effect/cooldown values or channel duration are invented.
The bounded inspection is in research/recall-native-current.txt and
tools/read_recall_native.py. The native header adds one verification anchor,
with no additional redirect.

Shift + press selects persistent aiming; key release keeps it, a valid click
casts, invalid confirmation keeps it without retrying. New gameplay commands
cancel aiming; camera controls and Tab preserve it. Physical mouse edges are
tracked even when the ability handler consumes a confirmation. A generation
check prevents a superseded cast being dispatched after native validation.
B clears the old order and sends Return once; later idle ticks preserve recall.
Visible completion and normal-cast behavior require TESTING-16.md.

Validation: 18 core and 56 probe tests pass, both packages pass Clippy all
targets with warnings denied, and formatting checks pass. The optimized DLL
loads, exports ABI 6 and rejects a null host. All twelve branch/header anchors
match the executable. Version 0.16 was installed with the game closed; both
installed hashes match the package. See dist/build-0.16.0.json.
The working 0.15 package is preserved in dist/backups/0.15.0. No game match was
operated by the agent.

## 0.17 HUD failure and approved 0.18 correction (2026-10-01)

The user's screenshots show HUD contents faintly underneath its large dark
background and continued champion skill-info hover despite hidden spectator
panels. The 0.17 log is archived as research/probe-2026-10-01-seventeenth.log.
The user explicitly approved correcting layer order, shrinking the panel and
adding the missing popup to the active-session cleanup.

Literal work: read player_hud.rs and team_info.rs, installed SDK UI APIs and
counted bundle UI records; inspect bounded native instructions with Capstone.
The root had z=1000 but almost all drawing descendants lacked explicit z.
The cleanup handled champion_tooltip but missed the separate
champion_info_tooltip named in InGame initialization (reference 0xb9d327,
string RVA 0x3ad5658) and helper 0x1ffe3a0. Its asset template is 600 x 542,
matching the large screenshot popup; the short popup is 335 x 106.
These static findings guide the fix; visible behavior requires the user test.

0.18 gives all drawing nodes explicit layer order, reduces the HUD from
740 x 154 to 600 x 120 and resizes slot hover geometry, portrait and fallback
command/camera mask together. Active client post-update now also checks
ingame.champion_info_tooltip, including lazy creation, and hides it when
visible. Bounded log messages record existence, discovery and up to four
suppression results. Release stops suppressing it so native hover resumes.
There are no additional hooks or changes to gameplay/casting/recall policy.

Validation: all 18 core and 61 probe tests passed, including updated hover
geometry; both packages pass all-target Clippy with warnings denied and format
checks. The optimized DLL loads, exports ABI 6 and rejects a null host; all
twelve native branch/header anchors match the exact 0.6.2 executable.
0.18 was installed with the game closed, and both installed SHA256 hashes
match the verified package. Prior 0.17 and confirmed 0.16 packages are preserved
in dist/backups. See dist/build-0.18.0.json and TESTING-18.md. Native rendering
and hover timing remain pending; no game match was operated by the agent.

## Confirmed 0.18 and approved 0.19 targeting toggle (2026-10-01)

The user confirms the HUD works and the unwanted popup is gone. The archived
research/probe-2026-10-01-eighteenth.log shows champion_info_tooltip exists,
four capped suppression messages applied=true/after=Some(false), and native
camera/UI restoration on release. Their level-2 Lancer screenshot shows W/R
still look ready. They supplied the unlock levels Q=1, W=3, R=5 and asked to
bundle that small HUD correction with a larger step. They then explicitly
approved champion-only direct targeting, with both backtick and Mouse 4 toggles
instead of a held modifier. General configurable bindings remain optional.

Literal work: read the current input polling, direct-click/skill cursor ranking,
SDK is_champion getter and native-validator call site. Copy the SDK champion
flag into target snapshots, then filter non-champions before direct-hit ranking.
No further reverse engineering or native hook is needed. Backtick/Mouse 4 rising
edges share a match/player-scoped toggle; either flips it once, both simultaneous
edges count once, held buttons on takeover/focus return are not fresh presses.
Release or identity change resets the mode. A dedicated CHAMPIONS ONLY label
shows its state. Direct clicks over no eligible champion move to ground;
unit-skill rejection cannot fall back to a minion. Normal-cast confirmation
captures the current mode without closing aiming on toggle; quickcast captures
it at press. Already issued attacks are not canceled solely by mode changes.
Attack-move still resolves all eligible enemies nearest the click. Ground,
direction, self and no-target skill requests retain native validation.

HUD unlock checks at 1/3/5 now override effect presence for display. Unlearned
slots dim, use a gray border even during aiming, and show the required level
in the overlay and hint. Level-up restores ordinary effect/cooldown display;
learning/casting rules themselves are not modified.

Validation: 18 core and 67 probe tests passed. New checks cover independent and
simultaneous toggles, focus/session reset, overlapping champion/minion targets,
native rejection, confirmation-time mode, unchanged attack-move and unlock
thresholds. Both packages pass Clippy all targets with warnings denied and
format checks. The optimized DLL loads, exports ABI 6 and rejects a null host;
all twelve native branch/header anchors remain unchanged and verified.
0.19 installed with the game closed after verifying the executable and prior
0.18 installation; both installed hashes match the package. Confirmed 0.18 is
preserved in dist/backups/0.18.0. Physical buttons and visible unlock display
require TESTING-19.md. See dist/build-0.19.0.json. No game match was operated
by the agent. Next recommendation after confirmation is clearer activation
and full-match validation, subject to the user's approval.

## Confirmed 0.19 and approved 0.20 session/range work (2026-10-01)

The user confirms all 0.19 features work, asks whether A should show attack
range, then approves the proposed range aiming, session buttons and removal of
the 60-second cutoff. Confirmed 0.19 is preserved in dist/backups/0.19.0 and
the previous recording is research/probe-2026-10-01-nineteenth.log.

Literal work: read the local stable SDK UI creation/path-event APIs and history
query APIs, the existing timing/view/worker adapter and command state machines.
Revisited bounded native disassembly from the ability investigation to confirm
the cached basic-attack Effect at EntityData+490; preserved the relevant excerpt
in research/attack-range-current.txt. No game was operated by the agent and no
additional native hook was added.

UI event callbacks queue match/phase-scoped actions. Client post-update clears
pending commands before applying Start/Pause/Resume/Return AI. Pause holds
publication and playback while camera/Tab remain available; physical input
histories keep advancing to prevent held-input replay. Manual ownership remains
held through an in-flight tick. Running no longer expires at 60 seconds, while
startup/READY/heartbeat and identity/exit recovery remain. Return AI is terminal;
the lifecycle still controls one foreground battle/set per process launch.

A aiming uses checked fresh native base-attack range including level growth and
range bonus. Its range circle persists until confirmation/gameplay cancellation;
camera/Tab preserve it and champion-only mode does not alter attack-move choice.
Native collision radii and validation remain authoritative. Post-battle SDK
history/replay candidates are copied to a diagnostic JSON file, explicitly
UNVERIFIED: numeric IDs can collide across categories and the last SDK player
sample may precede the terminal tick. No history or outcome is changed.

Validation: 75 probe and 18 core tests pass. Checks cover scoped/stale UI actions,
actual worker pause/wakeup, guards without running cutoff, range metadata and
aim cancellation, held movement/ability inputs across pause and strict history
reference extraction. Both packages pass all-target Clippy with warnings denied.
Native button rendering/click callbacks and full-battle saved-result comparison
remain pending in TESTING-20.md.

The optimized 0.20 DLL loads with ABI 6 and rejects a null host. Format checks
pass and all twelve native branch/header anchors match the fingerprinted 0.6.2
executable. Installed with the game closed after verifying the prior 0.19 hashes;
both installed hashes match the package. See dist/build-0.20.0.json. Confirmed
0.19 backup hashes and its archived INIT version were also verified.

## 0.20 feedback and approved 0.21 modded/death HUD correction (2026-10-02)

The user reports successful 0.20 behavior overall and a native result after
handing control back to AI. They report a blank HUD during death and clarify
that modded skill/item icons are also absent while alive. The 0.20 recording
is preserved in research/probe-2026-10-02-twentieth.log; the prior package is
backed up in dist/backups/0.20.0. It confirms Pause/Resume, release around 08:10
and a 10:00 result. The prior result JSON has no replay candidates, and its
player sample could be overwritten by a later same-key analysis worker. It
does not establish that the saved result reflects the controlled battle.

Literal work: read the current HUD code and SDK player/ItemSetting/UI APIs,
the game's enabled-mod configuration and installed data-champion declarations,
item settings, mod.override_info and sprite-sheet tag tables. Read local SDK
data-item/asset-override documentation. A bounded strings read of the item DLL
identified the logical base item source; no DLL was executed for inspection.
Leef's pack and Variety declare standalone PNG skill_icons; the Riot pack
redirects the base item's #sheet/#data to its 640X640 resource. Its 243 tags
include native registered items missing from our embedded base metadata.
No new executable disassembly or native hook was needed.

0.21 reads enabled data-champion PNG paths and applies active item metadata,
retaining host asset override resolution. A native item's key may be used as
its tag only when the active sheet's declared tags contain it. Image children
are recreated on reference changes so PNGs do not inherit an old rect_tag.
Installed declarations were checked: 18 enabled PNG champions and 243 item
tags. Only metadata references are read; no textures are copied. Arbitrary
DLL-only champion skill metadata remains outside this discovery path.

Selected-player samples now come from any living actor callback on the exact
original worker, every six ticks with deduplication. A dead player's portrait,
skills and inventory remain; native respawn ticks populate the countdown and
Pause retains that timer. Death clears pending orders. If all actor callbacks
cease, stale dead identity/inventory remains but no countdown is extrapolated.
Original-worker samples continue after Return AI while the battlefield is
open. Result evidence freezes at battlefield exit; a bounded read-only result
UI label capture provides a comparison with the displayed result. History
polling and saved authority remain explicitly unverified.

Validation: 82 probe and 18 core tests pass, all-target Clippy with warnings
denied and format checks pass. Tests exercise enabled/disabled asset discovery,
PNG path/tag handling, native-item fallback evidence, death retention/timer
freshness, per-tick deduplication, original-worker/exit restrictions and frozen
result samples. Native icon drawing and death/respawn timing await TESTING-21.md.

The optimized 0.21 DLL loads with ABI 6 and rejects a null host; all twelve
native branch/header anchors match the exact 0.6.2 executable fingerprint.
Installed with the game closed after checking prior 0.20 hashes; both installed
hashes match the verified package. The prior backup hashes and archived 0.20
INIT version also match. See dist/build-0.21.0.json. No game was operated by
the agent; rendering, respawn timing and saved-result comparison remain pending.

## 0.21 feedback and approved 0.22 permanent images/death camera correction (2026-10-02)

The user confirms the death countdown but reports missing vanilla Circus Blade
skill icons, older owned-item icons disappearing after another purchase, and
MMB camera dragging unavailable during death. The prior log is preserved in
research/probe-2026-10-02-twenty-first.log and package in dist/backups/0.21.0.
Its references contain valid Circus Blade skill tags and multiple occupied
item slots. It records death/respawn cycles and an 11:00 result with 91 captured
labels. Saved-result authority remains unverified.

Literal work: read the HUD image update code, SDK node property/existence/bounds
APIs, camera input handling and the camera-target position requirement. No new
executable disassembly, native offsets or hooks were needed. Per-icon node
removal/recreation is the strongest rendering suspect, but a specific native
node collision has not been demonstrated. The camera path was explicitly
skipped whenever the selected champion lacked a living position.

0.22 updates permanent image children in place. Each skill has separate PNG
and sprite-sheet nodes, so standalone images do not inherit rect_tag state.
Item nodes remain independent. Changed references log property application,
node existence and bounds; those diagnostics do not prove actual drawing.
Camera identity now survives an absent living position. MMB dragging and edge
pan continue during death; follow/recenter require a living position. Untouched
lock intent resumes on respawn, while deliberate panning unlocks the camera.

Validation: 85 probe and 18 core tests pass, both packages pass all-target Clippy
with warnings denied and format checks. New tests cover permanent image-format
separation and item-slot isolation, death dragging/edge pan, HUD drag exclusion
and respawn lock intent. Installed assets were checked: 18 enabled PNG champion
declarations and 243 overridden item tags; no textures were copied. Native
rendering and death camera behavior await the checks in TESTING-22.md.

The optimized 0.22 DLL loads with ABI 6 and rejects a null host. All twelve
native branch/header anchors match the fingerprinted 0.6.2 executable. Installed
with the game closed after checking the prior 0.21 hashes; both installed hashes
match the verified package. The 0.21 backup hashes and archived INIT version
were also verified. See dist/build-0.22.0.json. Native rendering and death-camera
behavior remain pending user testing; no game was operated by the agent.

## Confirmed 0.22 and approved 0.23 consecutive-battle lifecycle (2026-10-02)

The user confirms all 0.22 checks, including correct icons on the mod champion
Harpy. They consider the placeholder hover hints unhelpful and approve removing
the one-battle-per-launch restriction with that small accompanying change.
Harpy's last original-worker KDA 4/5/2 agrees with captured result labels;
saved-record authority remains unverified. The prior recording is preserved in
research/probe-2026-10-02-twenty-second.log and confirmed package in
dist/backups/0.22.0.

Literal work: read coordinator release/binding code, own-team caching, SDK
scene enums, camera lease handling, command/metadata/HUD state and result audit.
No new executable disassembly, field offsets or native hooks were required.
The old coordinator never returned from Released to Armed, ownership stayed
cached and the camera retained its first viewer lease. Those process-lifetime
assumptions are now replaced by per-session state.

0.23 rearms after release outside the battlefield on Main, Lineup,
StadiumEntrance or Match, or when returning to title. Result/SetFeedback/replay
screens cannot bind a new worker. A new foreground tick-1 worker repeats the
existing publication/bootstrap/pause acknowledgement. Retired match keys reject
late same-battle re-simulation; title clears them to permit replaying a loaded
pre-match save. Return to AI remains final for the current battle.

Rearm clears counters, worker/sender/view bindings, UI actions, team/roster,
actor metadata, orders, recalls, HUD snapshots/caches, targeting and camera.
Lane choice persists as a default but unlocks. Physical histories are seeded to
avoid replaying held controls. Publication waits carry a session generation;
an old waiter cannot cancel or hold the next session. A short exclusive gate
excludes SDK scalar writes during consumer reset, with no host calls or pacing
waits inside that reset. Finished camera leases are discarded without accessing
old addresses. The placeholder hover behavior and its obsolete tests are removed.

Read-only result files include seed and session generation to preserve repeated
saved battles. Prior result polling continues between sessions until a new key
is bound and clears at title. Logging budgets restart per session in the same
appended file. Installation failures remain terminal; patches still install once.

Validation: 91 probe and 18 core tests pass; both packages pass all-target Clippy
with warnings denied and format checks. Seven new tests cover lifecycle and
consumer resets, including old waits, retired keys, identical-save keys, changed
ownership/side, stale casts, HUD sampling and held camera gestures. One obsolete
hover-only test is removed. Native consecutive-set/match/save transitions await
TESTING-23.md; no game was operated by the agent.

The optimized 0.23 DLL loads with ABI 6 and rejects a null host. All twelve
native branch/header anchors match the fingerprinted 0.6.2 executable. Installed
with the game closed after checking prior installed 0.22 fingerprints; both new
installed fingerprints match the verified package. Confirmed 0.22 backup hashes
and archived INIT version also match. See dist/build-0.23.0.json. Native lifecycle
transitions remain pending the user test.

## Confirmed 0.23 results and 0.24 gameplay polish (2026-10-02)

The user reports 0.23 works well. The log shows three separate live-worker
sessions and correct own mid selection: athlete 56 on red in sets 1/2, blue in
set 3. The final match-33 record references replay IDs 882/883/884 and ends in
team 7's 2–1 win against team 3. The replays match the controlled workers:

| Set | Replay | Seed | Final tick | Player/champion | KDA | Own result |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 882 | 15051353321345200540 | 39315 | 7 / cf_archangel | 7/6/0 | Win |
| 2 | 883 | 4019982477785389105 | 31875 | 7 / exorcist | 3/3/1 | Loss |
| 3 | 884 | 14400468899550163220 | 35033 | 2 / hunter | 13/1/2 | Win |

This verifies recorded series authority for this run; disk save reload
persistence was not separately tested. The generic capture's original
UNVERIFIED label is preserved as collection-time provenance. See the archived
`research/probe-2026-10-02-twenty-third.log` and final set-3 result JSON.

Literal work for 0.24: read our Rust mouse-order code, local stable SDK player
status/portrait getters, and the existing result JSON/logs. No new reverse
engineering or native hook was needed. Prior embedded champion text and live
descriptor checks show Exorcist Q/Purification requires an ally under crowd
control (`AllyChampionInCc`, target code 2). Its 82 logged rejected Q attempts
do not establish a casting bug; the user has not confirmed a CC-valid target.
The new failure message explains that native condition without overriding it.

The attack-move code computed a cursor hit but discarded it whenever A was
armed. A direct enemy hit now creates `Attack(id)`; a ground hit still creates
`AttackMove(point)`. Native attack rejection retains the existing approach
request toward the committed target's fresh position. Dead, untargetable or
unseen targets clear explicit attacks without silently choosing a minion.
A remains unrestricted by the champion-only toggle. Hover uses the same click
picker; pale gold and orange foot rings identify hover and attack targets.
Freshness, focus, camera/HUD masks and death/session cleanup apply.

Ten permanent portrait cards use the game's own portrait helper and live
side/lane/alive/respawn scalars copied from the bound original worker. Dead
portraits retain identity and show native timers; pause freezes them, stale
running samples hide countdowns. Team state and UI caches reset alongside the
existing session consumers. Manual shopping remains optional and deferred.

Validation: 96 probe and 18 core tests pass; both packages pass all-target
Clippy with warnings denied and formatting checks. The optimized DLL builds.
Direct-target chase/loss, ground acquisition, hover filter/masks, stale/dead/
paused markers, roster identities/resets, timer handling and ally-CC validation
are covered. Actual rendering and live gameplay require TESTING-24.md. The
working 0.23 package and recording are preserved. No game was operated by us.

All twelve native branch/header anchors still match the fingerprinted game
executable. DLL loading reports ABI 6 and rejects a null host. The enabled-pack
asset check passes for 18 PNG champion declarations and 243 overridden item
tags. Installed 0.24 with the game closed after checking the game and prior
installed 0.23 fingerprints. Both installed files match the verified package;
the 0.23 backup hashes and archived INIT version are verified. See
`dist/build-0.24.0.json` and `research/series-33-verification-0.23.0.json`.

## Confirmed 0.24 and approved 0.25 body picking (2026-10-02)

The user confirms direct A-click feels great, markers work and team portraits
work. They report clicks on visible champion artwork missing the small picking
area. Exorcist was not drafted; they now understand Q's native CC restriction.
No Exorcist eligibility override is needed. The 0.24 files are preserved in
`dist/backups/0.24.0`, with hashes verified against the installed/package files;
the recording is archived as `research/probe-2026-10-02-twenty-fourth.log`.

Literal work: read the shared Rust picker and installed animation metadata in
the counted game bundle and enabled Workshop packs. Base sprites use exported
`.fanim` rectangles; mod packs use both exported rectangles and Aseprite files.
Read the [official Aseprite format specification](https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md)
for layer/cel/tag layouts. No additional reverse engineering or native hook was
used. `tools/read_picking_assets.py` extracts 78 base normal-pose dimension
profiles without copying texture data. Runtime enabled-pack metadata loading
uses the HUD's pack discovery and finds all 18 current mod declarations with
no fallback. The explicit installed-asset test loads 96 profiles total, including
Harpy and CF Archangel; `research/picking-installed-0.25.0.log` records them.

Champions carry a copied body envelope alongside unchanged collision radius.
Idle/run/walk dimensions exclude large attack effects. Width and height scale
with current camera extent; small screen padding forgives edges. Core body hits
rank before padding hits, then by body-center distance and stable entity ID.
The shared picker serves hover, right-click, direct A-click and targeted quick/
normal cast. Visible upper-body picking no longer requires visible feet. Caller
HUD/visibility/targetability masks and native skill validation remain. Ground
attack-move and combat collision/range do not change. Non-champion envelopes
remain the legacy sizes. Missing or unsupported metadata uses a bounded fallback.

The SDK does not provide the displayed frame/facing. These are normal-pose
envelopes, not exact opaque silhouettes. Half-sheet map scaling and a bottom-
center anchor are explicit calibration choices pending the user's visual fit
test; transparent gaps, swings and asymmetric/displaced poses can differ.

Validation: 103 probe + 18 core tests pass; both packages pass all-target Clippy
with warnings denied and format checks. Upper-body clicks across zoom, crowd
ranking, margin versus core, visible bodies/offscreen feet, targeted casts,
effect exclusion, visible layers/linked cels, corrupt metadata and scoped paths
are covered. The optimized 0.25 build succeeds. No game was operated by us.
TESTING-25.md focuses on actual body fit and crowded/zoomed clicking.

The DLL loads with ABI 6 and rejects a null host; all twelve native anchors
match the current game executable. Enabled HUD assets still pass checks for 18
PNG declarations and 243 item tags. Installed 0.25 with the game closed after
checking the executable and prior installed 0.24 hashes. Both new installed
fingerprints match the verified package. See `dist/build-0.25.0.json`. Visual
fit remains pending the user test.


## 0.26: approved portrait selection and graphical UI

User rejected text-heavy designs and requested grayscale surfaces, visual
recognition, yellow active accents and red deaths. Read current selection/session
UI and local SDK i18n/UI APIs, and installed description metadata; no new native
reverse engineering or hooks. Added original procedural monochrome PNG glyphs,
portrait-only READY selection, prepared scalar HUD snapshots, phase/key/generation
choice guards, command clearing on rebind, numeric/icon HUD and localized hover
text. Removed normal debug strips and their invisible command masks. READY waits
for a user action while startup/heartbeat guards retain fail-open behavior.

0.25 body picking was improved in the user's test; precise moving-pose coverage
was not measured. Its package and probe.log are archived. 0.26 has 110 probe tests
and 18 core tests, both all-target Clippy/format checks, an optimized DLL build,
ABI/null-host checks, unchanged twelve executable anchors and packaged PNG
palette/fingerprint verification. Native UI rendering, event hit tests and
localized tooltip wrapping await TESTING-26.md. Unknown tooltip formula values
are deliberately ellipses; exact per-skill damage interpolation is not exposed
by the stable SDK. Previously proven match 33 series record remains unchanged.
# 0.26.1 portable release follow-up

The first public ZIP exposed a development-only dependency: initialization used
the compiler's project directory for logs and activation flags, and failed log
creation returned before registering control. The C:-only laptop cannot use it.
0.26.1 enables control when the mod is enabled and writes diagnostics under
Windows LOCALAPPDATA with TEMP fallback. Failure to open both locations leaves
control available without file diagnostics. Read-only result reports use the
selected diagnostic directory. Native hooks, executable guards, UI and gameplay
remain unchanged; this does not claim the other laptop or native UI was tested.
