# Experimental native adapter (current: 0.22)

0.22 updates permanent HUD image nodes and permits bound camera processing with
an optional living position. Free navigation works during death; Follow applies
only with a current living position. Existing view/config leases, restoration,
five branch sites and twelve anchors are unchanged. See TESTING-22.md.

0.21 adds HUD asset discovery, selected-player sampling through other actors
on the original worker, and frozen result evidence. There are no new branches
or native memory offsets. SDK getters supply inventory, alive state and respawn
time; bounded installed JSON declarations supply custom skill PNG paths. The
native timer and gameplay remain authoritative. See TESTING-21.md/player-hud.md.

## Approved 0.20 session controls and attack-range aiming; game test pending

The user confirms all 0.19 features work. Version 0.20 adds native UI Start,
Pause/Resume and Return to AI controls and removes the 60-second running cutoff.
Pause reuses publication/view holds while retaining camera/Tab and selected-actor
ownership; pending orders are cleared before the transition. Stale button events
and held paused gameplay inputs cannot become new orders. The 15-second startup,
120-second READY and two-second heartbeat guards remain. Return to AI is terminal
for this launch's single controlled foreground battle. See session-controls.md.

A aiming draws fresh basic-attack range copied from the existing EntityData+490
cache, with current level growth and range bonus. The existing five branch sites
and all twelve anchors are unchanged. Read-only SDK history/replay evidence is
captured after battlefield exit; it does not establish saved-result authority.
See TESTING-20.md. The sections below record earlier implementation stages.

## Approved 0.15 ability extension; live test pending

The user reports all 0.14 controls work. Version 0.15 adds physical Q/W/R
quickcasting and Shift-only range/aim previews. Runtime effect metadata is
copied from the selected champion during native move/attack calls; SDK native
validation accepts or rejects each one-shot skill. Rejection preserves manual
ownership and does not queue a later cast. Shift previews never send inputs.
See docs/abilities.md for literal RE steps, fields, targeting policy and limits,
and TESTING-15.md for the game test.

One additional CALL is redirected: 162f387 -> 15b2700, bytes e87433f8ff,
three arguments (EntityData, InputTarget reference, events), returning unit.
Its 19-byte native entry header is 5541574156415541545657534881ec88000000.
The observer copies only scalar metadata and forwards the original attack
unchanged. The existing move hook also copies the fields before its scoped
hold/forward decision. Tickets now include a hold flag and can observe both
move and attack ticks; they still match selected actor, match and worker, and
clear at publication. The fifth site joins full fingerprint/prologue checks,
transactional installation, periodic branch audit and argument-forwarding tests.
The SDK dispatches all skill inputs; the mod does not directly invoke native
skill functions or alter cooldowns, effects, damage or committed action states.

## Approved 0.14 combat/HUD extension; confirmed by user

Test 13 confirmed held-Space/Y follow, navigation, dragging, window-edge scroll,
Tab visibility, shortcut suppression and release. Version 0.14 adds SDK-native
Attack orders and A-then-click attack-move, centered click-through Tab information,
automatic spectator detail hiding and client-thread wheel zoom. Target snapshots
are copied during the selected player's SDK simulation callback and never retain
native entity pointers. Attack inputs are validated with the SDK before delivery;
rejected attacks use native chase movement. Ordinary idle/ground orders do not
acquire enemies. No direct damage or cooldown writes are used.

The four fingerprinted executable branch sites are unchanged. Wheel input uses a
separate Windows WH_GETMESSAGE observer scoped to the owned UI thread. Removed
wheel messages are consumed only over battlefield while coordinator ownership,
focus and camera geometry are valid; they become WM_NULL to avoid duplicate zoom.
Accumulated notches change view+110 inside the existing viewer borrow. Current
native keyboard zoom establishes 0.25 steps and 0.5–3.0 limits. The observer is
removed when ownership ends. Source UI layouts were read from the installed
bundle; SDK visibility/layout properties implement HUD cleanup and centering.
The minimap, header and playback/zoom toolbar remain. See TESTING-14.md and
docs/camera.md; acquisition and sprite hit margins are explicit mod policies.

## Approved 0.13 camera/input correction; confirmed by test 13

Test 12 confirms native movement cancellation: actor 25 received a stop event
15 ms after one detected S press. The first sampled click was replaced into the
simulation 2 ms after detection; these timestamps do not measure visible display
latency. The user confirmed movement and stopping, no automatic actions near
minions, and reported Space, HUD dragging and vertical-edge issues.

The native follow routine 1fcfd5d reads config+20 team side and config+1c lane,
then looks up the player map. Current native own-mid shortcut cb020e writes the
same layout. Archived LLVM debug field names confirm Follow { team, position }.
The previous SDK player-ID payload was wrong and is superseded. Both Y and Space
now use the selected side/lane. The camera no longer infers deliberate panning
from any native mode change; minimap intent and captured dragging interrupt follow.

The fourth branch is CALL a270cd -> caf090. Native tag-11 input dispatch 9f9dfb
passes database+13e0 into a26d60, whose shared call uses the same viewer pointer
as playback 9fdaf0. The outer b234b6 target is a different view and is not used.
Current caf090 has seven arguments: view/client, UI, system, f32 dt, event,
window, database. Keyboard press/release are POD variants 8000000000000006/7,
with key byte at +8. These can be skipped without dropping an owned allocation.
Mouse/non-keyboard variants forward unchanged and retain native cleanup.

READY/running interception requires the exact bound view, SDK client thread,
selected champion, battlefield and fresh heartbeat; it stops on release or test
expiry. Spectator keyboard events are suppressed except default zoom keys 3d/3e
and unassigned spectator key 21. Native keybinding preferences are not changed.
Default action/key mapping comes from current 28d9b70 and its names table;
default spectator pause maps to key 17 (S). Native pan velocity is cleared during
owned camera updates and release so a key released while owned cannot revive
stale panning. Pan offsets +458/+45c are confirmed by caf522 and native pan actions.

Captured drags and window-edge scrolling are tested across HUD; Tab temporarily
overrides native player_info visibility, preserving its original value for
release. Core pacing/input dispatch is otherwise unchanged. See TESTING-13.md.


## Approved 0.12 mouse/camera build; live test pending

Test 11 replaced selected AI input consistently, but the champion continued
travelling after key release. Current native input application (World slot
+288, thunk 1779810 to 162f0d0) clamps a Move destination through 171fdf0 and tail
jumps at 162f276 into 15c4210. The latter returns for squared distance below
4,000,001 without clearing an existing action. Entity action 2 is movement;
action 0 is idle. The action dispatcher at 15b53e0 and table 3b7c9d0 confirms
this distinction. Native completion emits event 1705840 then clears action.

The new third branch redirects only that movement-consumer tail JMP. A
thread-local ticket from the selected SDK hold input must match the actor,
foreground match and active worker. It is cleared on use and publication.
A matching movement action emits the native stop event and changes action 2
to 0. Nonmovement states and the original effect gate are preserved. Calls
outside this scope forward to the original native routine.

The executable hash, three branch instructions and four native entry headers
are checked before installation. All three branch pages are acquired before
writing. Cache-flush failure restores the original branches; protection/readback
failures leave control disabled. Original functions and stop notification stay
intact. The DLL is pinned for the process lifetime.

Camera config edits occur only inside the existing viewer call's config borrow;
selected camera mode is restored through that same borrow on release. Native
renderer 1fd2c00 confirms camera center at view+114/+118, extent at +11c/+120
and central cropping from a 2048-square Game texture. UI-to-world mapping is
center + (UI - viewport center) * extent / 2048, then multiplied by 1000.
Native mouse/minimap code caf090 supplies window-to-1920x1080 conversion and
minimap rectangles. Native zoom and minimap handlers are reused.

Static traces and automated tests support this implementation; they do not
prove live pointer accuracy, native actor ID matching or visible cancellation.
The next recording logs CLICK, CAMERA and NATIVE_STOP to verify those points.
See TESTING-12.md and docs/camera.md. The following sections are historical.

## Test 10 confirmed startup; probe 0.11 tests persistent ownership

Test 10 entered the InGame viewer at played_tick=0, queued=1. Native playback
applied one frame and acknowledged a held update. READY appeared in 61 ms;
Start followed 12.757 seconds later. Playback then advanced at normal speed
until Ctrl+End released with produced=1268, consumed=1266, played_tick=1266.
The user confirmed movement response but reported it mixing with AI.

The movement test returned None on zero direction, which the stable SDK shim
converts to Pass. Tick 150 visibly contains request=None and a native Move
destination in base input. Probe 0.11 sends Some(move_to(current_position))
instead, including when directions are absent or older than 250 ms. The input
scope no longer implicitly relinquishes ownership at a 250-ms heartbeat gap;
the original two-second session guard still explicitly releases coordination.
Manual input rejection also releases with a reason rather than silently passing.
Every hold/move transition is logged for the selected champion.

Native validation evidence: GameRunner constructor 172a048 stores World vtable
3b9be28 into runner+1dc8. The stable AI validator 2e61390 delegates through its
slot +280 to 177bf90. That function resolves a living champion and dispatches
Input Move (0) through table 3ba5fc8 to 177c058, which returns true without a
destination-distance test. Thus a move-to-current-position command is accepted.
The SDK converts Some(input) into Replace; its native adapter converts Move
without altering coordinates. These checks do not establish every actor-side
cancellation effect. An attack/cast already underway and any existing target
must still be checked in test 11. No entity memory or additional native hook
is modified by this change.

## Correction from the ninth runtime test

The worker at `0xbe470a` entered, successfully published one frame and held for
15 seconds. The playback hook at `0xb240e9` had zero entrances throughout the
recording, including after release. Therefore the worker hold worked, but
playback confirmation could never complete. The fallback released at 15,001 ms.
The eighth-test pointer-flow claim below was insufficient and is superseded.

Read-only disassembly of the installed 0.6.2 executable established the current
scene mapping: SDK conversion `0x2e45930`, table `0x3cec520`, maps native tag 11
to InGame. Older SDK internal scene numbering differs and cannot label current
branches reliably. Outer update `0xb1fb00` calls scene handler `0x9f7b90` at
`0xb2226e`; the handler dispatch table `0x3ac4790` selects `0x9f9c1c` for tag 11.
Thus the scene handler runs inside the observed SDK client update; its old
unused hook did not prove that the whole handler was unused.

The InGame view is at database `+0x13e0`. The receive CALL `0x9fbbe5` reads frames
from database `+0x1908`, appends them to the received-frame collection, and the
following refill code copies them into this view's queue. Playback CALL
`0x9fdaf0` invokes `0xa8b090` on that view. Its nine-argument convention and
config borrow match the existing adapter. Native playback accepts a nonempty
queue, including one frame; it does not require a 100-frame buffer to proceed.

Probe 0.10 changes only the playback CALL site and its expected bytes
(`e8 9b d5 08 00`). Worker gating and startup acknowledgement remain unchanged.
This establishes a specific static path, not a successful game test. Viewer
entrances, initial application, PAUSE_ACK, READY and movement must be verified
in test 10 before claiming the correction works.

## Correction from the eighth runtime test

Both old entrance counters remained zero while patch readback stayed valid.
The client independently released at 15,001 ms, as intended. This proves the
old CALL locations were unused in this match; it was not a thread filter or
an overwritten patch. The recorded foreground AI stack returned through
`0xbe43fa`, inside the shared-runner worker `0xbe42b0`. Earlier research found
a tutorial caller for that worker and incorrectly treated it as exclusive.
The normal match also uses it.

The SDK client's recorded stack returns through `0xb2f90c` in `0xb1fb00`.
Within that function, `0xb240e9` was selected from pointer flow near SDK
post_update. Test 9 disproved its relevance to the displayed battlefield:
its entrance count stayed zero. The current build redirects `0x9fdaf0` instead.
The recording and focused disassembly are retained under `research/`.

That correction identified the active worker from runtime evidence but failed
to identify an active display update. Test 9 confirmed only the worker hold.

## Correction after the seventh game test

Probe 0.7 installed its CALL patches and bound the RED mid lancer, but the
battlefield kept running. No matched worker publication, viewer acknowledgement,
READY or Start was recorded. The simulation advanced roughly 580 seconds in
6.319 wall-clock seconds. Static labels in the architecture below identify
candidate paths; their relevance to the actual match has not been established.

The old "Loading paused" label reported an intended phase rather than a verified
pause. The 15-second deadline was checked inside native paths only, leaving
Loading stuck if those paths never reached the coordinator. Both are corrected
in 0.8. No candidate address or startup allowance was changed.

Probe 0.8 counts entrances unconditionally before SHARED lookup or filtering.
It samples accepted/rejected scopes before inspecting view memory, uses Windows
OS thread IDs for matching, and logs coordinator identity. Configuration rejects
duplicate initialization rather than silently retaining an older coordinator.
Installation reads back the exact patch bytes and the client audits them once
per second. AI ticks 1/2 and client Match/InGame transitions record up to 64
module-relative return addresses using Windows RtlCaptureStackBackTrace. This
is runtime call-stack sampling, not stepping the game under a debugger.

The client heartbeat enforces the unchanged 15-second startup deadline without
depending on interception. READY requires an additional held native update with
zero queue consumption and unchanged played tick after initial application.
Overlay states distinguish awaiting a worker, awaiting playback acknowledgement,
Ready and inactive. A later custom play/pause/AI interaction is deferred until
control works; current hotkeys are temporary diagnostic controls.

## Current architecture

The 0.16 extension retains the five verified redirects used by 0.15. B submits
SDK Return once. The selected-actor Move/Attack hooks additionally support
explicit cancellation of recall action 1 via native event 17069c0, whose header
is verified before installation. There is no new branch redirect. Idle Move
at the current position preserves the native channel. See docs/abilities.md
and research/recall-native-current.txt for the scoped state change and evidence.

This is a diagnostic build, not a claim of a completed direct-control mod.
Test 10 established one-frame startup and a live playback hook. This build tests
whether continuous replacement commands provide exclusive manual movement.

## Evidence and scope

The executable SHA256 must be
`15df9eb3b6915cdcc4c2ebb3b7f5fa232b563c4cdd32208581634817b71adc23`.
The adapter also checks the loaded PE timestamp/image size, both CALL instructions
and both original function prologues. Installation happens only during a title
screen callback. Mismatches disable the coordinator and preserve native behavior.

The runtime-identified worker loop at RVA `0xbe42b0` calls `run_tick_ext` at
`0xbe43f5` while owning the runner write lock. That CALL is not patched.
Runner, highlight and segment locks are released before its frame-send CALL
at `0xbe470a`, target `0xa3fbe0`. The adapter forwards the original send first,
checks the native success sentinel in the output Result, then counts this
publication and holds before the next tick. Its three arguments are the output
Result, Sender and frame pointers. The native caller ignores the return register.
The SDK identifies the
current run by origin `ClientMatchView`, seed, match ID, set and worker thread.
The native gate never waits for other threads/origins. It counts publications
even if no champion AI callback occurs on later ticks.

The traced InGame receiver/playback path calls native update at `0x9fdaf0`,
target `0xa8b090`. Its argument list was checked against current instructions:
four register pointers, four stack pointer/usize arguments, then stack f32 dt.
The caller already owns the mutable playback-config borrow. The adapter uses
the view/config pointers only within that call; it never casts an opaque SDK
context into an internal object or retains a pointer for later dereferencing.

The native queue length is at view `+0x70`, played tick at `+0x290`, and remaining
playback delta at `+0x358`. Temporary config overrides set normal 1x playback and
disable the target-tick catchup during this call, then restore every changed
config bit. Held updates receive zero delta and clear remaining playback delta.
One initial frame is applied with a single frame interval. During running,
per-update delta is capped at two intervals and generation lead is limited to
two unconsumed published frames. Queue consumption is counted before/after
native update. Native ticks and frame counts are logged independently of the
whole-second clock.

## Lifecycle and recovery

Stable AI tick 1 identifies the worker without sleeping. After this tick returns
and its frame send completes, the adapter enters the hold. The viewer can apply the
initial frame and continue loading while simulation generation is held. READY
requires a publication boundary, an applied initial frame, the SDK battlefield
scene and an unambiguous owned champion. Ctrl+Home starts only on a fresh press
after READY; an earlier press is not queued.

Ctrl+End releases the hold directly from the worker as well as the client. A
15-second loading deadline, 120-second Ready deadline (the historical 60-second
running budget was removed in 0.20),
two-second client-heartbeat guard after Ready, battlefield exit, view/sender
identity changes and inconsistent frame counts also release. Movement directions
require a client sample newer than 250 ms; expired samples send a hold command
during manual ownership. Commands require correct match/thread/ownership and
native input validation. On release both hooks pass through to original calls.

The DLL is pinned for the process lifetime so a mod unload cannot leave patched
CALLs pointing into freed code. Extension destruction releases coordination.
Relays and patches stay in memory as inert forwarding calls until process exit;
they are not removed while worker threads may execute them. Restart before
replacing or enabling the adapter again. The executable file and saves are never
patched by the adapter.

## Limits still requiring game evidence

- Initialization and gameplay logic occur in the same first worker call.
  Publishing/applying this frame is not proof of an untouched simulation state.
- The exact Match-to-InGame loading predicate has not been fully decoded. The
  traced Match stage includes UI progress/fade work, and the generic loading
  paths check pending operations and viewer availability. The build records
  whether one published frame is actually sufficient; it does not infer that
  from a queue-refill limit.
- A stable SDK origin/thread is combined with a native call-site observation;
  static address labels alone are not treated as proof of a live worker.
- This build does not establish whether final persisted match results use the
  controlled worker's output. It must not be presented as a playable milestone
  until command response and result authority are checked in-game.
- The native pause/speed controls do not yet drive this coordinator. Ctrl+Home
  and Ctrl+End are the test controls. A later game update requires a new adapter
  validation; stable SDK ABI compatibility does not cover these private calls.

Checks cover Windows SHA256, rel32 encoding/range, executable relay forwarding,
all three send arguments and its output memory/all nine viewer arguments, scoped config restoration,
nonblocking SDK observation, publication-boundary holds, frame-lead limiting,
explicit Start, cancellation, timeouts and ownership/input isolation.
