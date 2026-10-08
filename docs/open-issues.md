# Open issues and deferred work

Latest correction: **0.58.2 installed**. The user approved 0.58.1's outline/pulse
but found kill announcements still positioned over the old left-half viewport.
The native full-screen toggle changes two separate flags: renderer config and
IngameUI. Both are now leased together through the existing viewer hook, with
live UI type/config identity checks and independent restoration on F12. The
compact option-button hiding/event workaround is removed; native full-screen
layout manages those controls. Baseline spectator-panel suppression for the
custom HUD remains. Outlines and minimap operands are unchanged. Automated
checks pass; full-screen placement, release and later-match behavior await the
user's test. See [checklist](../TESTING-58.2.md) and
[notes](native-fullscreen-layout-58.2.md).

Latest pass: **0.58.0 installed**. The user found 0.57 hover/target outlines too
similar and reported the spectator full detail panel leaving a compact battlefield
in later direct-control matches. Target thickness is reduced by one-third;
explicit attack clicks pulse for 120 ms with click > hover > attack priority.
Hover is unchanged. The native full-width flag is leased during ownership and
restored on AI release; fresh viewers capture their own preference. Automated
checks pass; native feedback/layout testing is pending. See
[checklist](../TESTING-58.md) and [notes](selection-pulse-layout-58.md).

Latest pass: **0.57.0 installed**. The user confirmed 0.56.1 and approved persistent
attack-target outlines. The new thin red target outline coexists with one stronger
hover outline, merging when they refer to the same unit. Cursor stays hover-only;
ground markers remain debug-only. Native test pending; see [checklist](../TESTING-57.md).

Latest correction: **0.56.1 installed**. The user reported selected-target markers
still showing with Debug off in 0.56.0. Both hover and attack-target ground
markers are now gated together, without changing orders, outlines or skill/range
previews. Native verification pending; see [checklist](../TESTING-56.1.md).

Latest pass: **0.56.0 installed**. The user confirmed proper sprite outlines on
all unit types in 0.55.3. Interface now has an outline toggle (On by default)
and Debug selection markers (Off by default); vision hover feedback targets
the pointed segment. Native UI testing is pending; see [checklist](../TESTING-56.md).

Latest outline pass: **0.55.3 installed**, native gameplay test pending. The user's 0.55.2 test still showed towers only. Static inspection found ordinary unit ground circles are NinePatch commands, which the Sprite/DrawLine filter still rejected. Extra passes now move only sprites and release other known commands through verified native cleanup. Single hover priority, blue ally/red hostile colours, F10 crisp/wide and ground indicators remain. See [outline notes](outline-investigation.md) and [test checklist](../TESTING-55.3.md). Earlier deferred-outline notes below are history.

2026-10-07 HUD completion: user confirmed the 0.43 minimap works and approved
0.44 warm HUD, motion, strip-seam closure and optional battlefield-only low-health
effect. This pass is implemented; native appearance/play awaits the user. See
[HUD details](hud-motion-pass-44.md) and [checklist](../TESTING-44.md). Low-health
is now default-off with a persistent skull-button toggle in the more menu.
Tower fit and true outlines remain separate; earlier pending-HUD entries are
history. Native font families differ from browser fonts; verify weight and fit.


2026-10-07 minimap implementation: 0.43 expands content to 352 px within the
original 360 px footprint, with a 4 px warm-grey surround and muted corners.
Native render layers and input geometry have been adjusted together; automated
checks pass, native testing is pending. See [minimap details](minimap-pass-43.md).
The broader approved HUD, button motion, native text fidelity and tower fit remain
follow-ups. Optional low-health screen effect is discussed but not approved.

2026-10-07 migration implementation: 0.42 updates the authorized 0.6.3 native
locations and changed entity/player/settings layouts. Both full game baselines
are preserved, and the exact identity plus 55 locations, 13 layout guards and
source/profile agreement have been checked. Native gameplay awaits the user's
test; see [migration details](patch-migration-pass-42.md). The approved revised
HTML HUD (warm palette, outline-only selection, motion and native text fidelity)
and tower selection fit remain next. The earlier checkpoint below is history.

2026-10-07 patch checkpoint: the game updated to 0.6.3. Its executable, full
asset bundle and SDK are privately preserved and hash-verified. A review-only
migration tool is implemented and 19 tests passed. The user reverted to 0.6.2;
its executable matched the known receipt and its full snapshot is preserved.
The first comparison found 13 unique review-only candidates, one ambiguous
match, 36 unresolved checks and five manual boundary/data cases (including two
code-shaped leaf routines without unwind entries). SDK code is identical;
private native layouts still need verification. The user may return to 0.6.3.
All 55 existing checks differ at their
old addresses. No native addresses or runtime guards have changed; 0.41 remains
unsupported on the new executable. See [patch migration](patch-migration.md).
The HTML HUD is approved with outline-only champion selection and particular
attention to native text size/weight; implementation and tower correction follow
compatibility work. Later references to deferred migration tooling are history.

Current approved build: 0.41 implements the supplied HUD with a larger skill
area, visible cursor-slider track/handle, minion vertical extension and larger
structure bounds. The user reported those problems after 0.40; a blanket 0.40
pass was not reported. Native fit and interaction remain pending; see
[HUD notes](hud-selection-pass-41.md) and [checklist](../TESTING-41.md).
True outlines remain deferred until current entity frame/transform access is
verified. This HUD pass does not add a renderer hook.

Latest user test: 0.39 champion selection is almost perfect, but heads are often
missed. Click marks last too long and skill hover does not show range. Approved
0.40 adds upward-only head coverage, newest-only 250 ms feedback, and visual-only
HUD hover range/self-area previews. Native testing is pending; see
[hover notes](hover-feedback-pass-40.md) and [checklist](../TESTING-40.md).
True sprite outlines remain planned: the bounded renderer review did not verify
an entity-to-current-frame/transform association. Existing ground highlights
remain until that prerequisite is met. No new native outline hook is installed.
Full 0.39 cursor transition/slider coverage was not reported by the user.
Other skill previews remain opportunistic testing rather than a per-champion
hardcoding pass. Later visual HUD integration remains separate work.

Earlier test: 0.37 directional projectiles no longer seem to auto-aim.
Whip Master W/R and Archangel R previews were reported incorrect. Approved 0.38
adds shared effect-family readers and independent casting/area placement; native
Whip Master W/R passed; others remain pending. See [geometry notes](preview-geometry-pass-38.md) and
[test checklist](../TESTING-38.md). Unknown families retain a limited guide.

Whip Master W's base compiled damage geometry is a forward-offset circle despite
its cone-like animation; do not replace it with a visual cone without evidence
from the active setup. Foreign effect DLLs and unresolved branch/timing effects
remain limited. Learning is deferred until opaque behavior needs it. Full
HUD/cursor integration follows the [confirmed design decisions](design-integration.md).
Pre-hit attack cancellation, FPS profiling, patch-resilience tooling and optional
manual shopping remain separate work.

0.36.0 corrects live-worker ownership in every native observer, authorizes steering
from the current position borrow, and observes the native automatic-attack path.
It adds concrete cast-drop/cancellation/rejection reasons. See
[control notes](control-pass-36.md) and [test checklist](../TESTING-36.md).
The user verified movement/casting improvement and no phantom Q. Pre-hit attack swing cancellation is deferred: this pass
does not remove pending attack effects or reset attack cooldowns.

The 0.35 Ninja test reproduced diagonal detours at 02:33, phantom Q 3 at
00:39-00:44, Q failure at 00:18 and W failures around 01:55-01:57. Its log had no
steering or attack-start records. Skill observation admitted background workers
with reused actor IDs; the old steering guard depended on an earlier command's
address. These are confirmed code defects, not proof that all reported symptoms
are resolved. Aim-assist/projectile behavior, FPS instability and patch-resilience
tooling remain open.

0.35.0 implements the approved command priority, native clear-segment steering,
guarded basic-attack backswing release, cast-count display corrections and rolling
late-match logs. See [control notes](control-pass-35.md) and
[test checklist](../TESTING-35.md). User testing found the failures above. These changes must
not be presented as confirmation of the exact Archangel 06:48 or Jiangshi Q cause.
The user verified 0.34 artwork and self-hover fixes. Aim-assist/projectile behavior,
FPS instability and game-patch compatibility tooling remain separate open work.

Latest user feedback (2026-10-06): 0.33.0 ally hover and monster bounds work well.
Jiangshi skill artwork sometimes disappears/reappears repeatedly; correlation
with casting/form changes is unknown. Ordinary self-hover feedback is distracting.
The earlier version-specific sections below are investigation history.

0.34.0 preserves static artwork through transient live-reading expiry and
suppresses ordinary self-hover by entity ID. Native testing is pending; see
[notes and read-only route findings](artwork-self-pass-34.md) and
[game check](../TESTING-34.md). The artwork-clearing path is confirmed in code;
it is not yet proven to explain the reported flicker. Movement remains continuous,
with grid-derived intermediate goals; unwanted-turn and FPS causes remain open.

## Current approved pass: 0.31.0

The user verified 0.30 tooltip colors and minimap route persistence. Purchase
tracking still failed; next-item icon and missing gold were unavailable. They
approved sprite rectangles with champion priority, smaller minion hit areas, and
a bounded investigation of the supplied item mod's preset purchase chain.

Two item-mod presets exactly matched the final builds captured in the 0.30 log.
The registered-item reader reversed next-tier links by misidentifying their
meaning. 0.31 corrects that direction and adds inventory/build-change diagnostics
without broadly rewriting the predictor. Gameplay verification is pending.
See [TESTING-31](../TESTING-31.md) and [notes](selection-items-pass-31.md).

Updated 2026-10-06. The user approved implementation of the three priorities
below. The user has explicitly deferred the Ninja movement and frame
drop investigation until the other issues are handled.

## Previous priorities, 0.30.0 implemented and tested by the user

1. Purchase tracker and item descriptions: the live match catalogue has 262
   items, while the HUD's SDK ItemSetting reader logged only 31 metadata aliases.
   Riot Items changes original items through asset overrides and adds items
   through native code. Its item-setting file contains 30 definitions, so loading
   that file alone would not supply the full live catalogue. Read the effective
   registered item data and check the active buying rules before forecasting the
   next purchase, missing gold, and purchases affordable on recall. The vanilla
   detail panel and actual purchases work according to the user. Our tooltip also
   returns early when metadata is absent, without trying localized descriptions.
2. Skill description colors: our plain-text formatter strips the game's color
   markup. Preserve the host's stat-type colors, including colored ratios and
   values; do not invent formula values that have not been resolved.
3. Disappearing minimap path: the map observer calls set_navigation, which clears
   the active route. The first tested match's log shows a callback clearing two
   waypoints while the same Move order remained active and the champion was far
   from its destination. This also changed the requested movement waypoint.
   Scope geometry to the live match and retain the route until arrival,
   cancellation, or a replacement order; unrelated callbacks must not erase it.

Implemented in 0.30.0 and installed after 160 passing automated tests.
The user verified the tooltip and route corrections; purchase tracking still
failed. See [TESTING-30](../TESTING-30.md) and
[implementation notes](items-pass-30.md).

## Deferred: unwanted movement and intermittent frame drops

User report: in the second test match, playing Ninja on Sunna, the champion
briefly moved in directions the user had not clicked several times. It felt like
AI interference. FPS also dropped several times, worsening the perceived delay.
Earlier feedback established that unwanted turns can happen during plain walking,
without attacks or skills; investigation must not assume a combat-only cause.

The complete previous log is preserved locally at
`research/probe-2026-10-06-twenty-ninth.log`.

Evidence from the second match (log timestamps 1791244331329 through
1791244972357):

- Ninja was bound to player 7, athlete 56, red-side mid.
- No recorded native release back to AI during this match. All 949 logged manual
  input samples were valid and had a candidate. Sampling does not prove that every
  intervening command or native movement change was correct.
- No map-observer callbacks occurred during this match. The route-clearing bug
  above therefore does not explain the reported Ninja incidents.
- The sampled generated/presented simulation counters stayed close to 60 ticks
  per second, with at most two frames of lead. These counters are not rendering
  FPS and do not rule out short stalls or rendering frame drops.
- MOVEMENT TRACE reads the native destination before applying the current move
  request. A difference between those fields is not by itself proof of AI input.

Questions for the later investigation:

- Can intermediate waypoints submitted by our planner, combined with native
  navigation, cause unnecessary turns? This is a hypothesis, not a finding.
- Is captured navigation geometry actually associated with the current live
  match, rather than an earlier or background simulation?
- Does any command source or native action change the selected champion's
  destination without a matching accepted player command?
- Where do frame stalls occur: rendering, simulation, mod callbacks, shared locks,
  asset/tooltip reads, or logging?

Proposed later diagnostics: bounded records connecting the accepted click/order,
our current waypoint, native action/destination after command application, and
actual movement; measure frame and callback duration rather than infer rendering
performance from simulation ticks. Avoid continuous expensive tracing.

Do not report these issues as fixed by the minimap lifetime correction, and do
not describe them as normal Ninja behavior without evidence. The user performs
native gameplay and rendering verification.

## Existing deferred work

- Game-patch maintenance: build an offline migration helper that locates candidate
  native functions using multiple instruction/source-reference/call-graph clues,
  compares ABI and data-layout assumptions, and produces a review report and
  per-build compatibility profile. Existing fixed-RVA byte checks and exact
  executable hashing verify the known build; they do not relocate hooks.
  Keep released native integration restricted to verified profiles. Signature
  matches alone must not authorize unknown calling conventions or layouts.
  The user requested remembering this for later, not implementation now.
- Cursor replacement and champion-only targeting cursor indication.
- Aesthetic changes, pending the user's design principles.
- Manual shopping remains optional and is not the next priority.


## 0.32.0 implementation and user test

One-frame running lead, viewer wake-up signalling, early gameplay capture,
tier-4 purchase forecasting, declared tooltip parameters and sprite-sized
selection/highlights are implemented. The user confirmed much better
responsiveness, great selection, and working item/gold reminders. See
[notes](control-pass-32.md) and [game check](../TESTING-32.md).
Bomber aim assist remains unproven/unfixed;
bounded command/queued-aim traces were added. Unsupported tooltip formulas and
native runtime patches absent from declarations remain unresolved. The deferred
Ninja movement/FPS report above is retained, not considered fixed by pacing.

## Current FPS observation and external analysis

The user measured actual FPS with RTSS, reporting fluctuations around 100 to
70 FPS with small stutters, rather than consistently low FPS. The vanilla game
was already CPU-heavy and unstable. The user suspects the direct-control mod
may not be the cause and will try a fixed cap in the 60-70 FPS range before
further performance work. No profiling/code changes are authorized by this
update. A smoother capped run would establish an improvement under that cap,
not independently identify the source of the original stalls.

Another AI's supplied analysis is preserved in
`research/performance-analysis-0.32.0-external.md`. Its numeric summaries and
causal conclusions have not been independently reproduced in this pass.
Important reported findings to retain:

- Log caps leave late-match intervals unobserved. Per-frame Tab logging spent
  many lines, while synchronous writes share a mutex with the worker.
- Viewer-hook frequency is a client-update proxy, not a direct measurement of
  GPU presentation/frame time. Existing simulation counters are not render FPS.
- One-frame pacing can translate prolonged client/viewer stalls into slower
  simulation progress. That possible amplification is distinct from causing the
  original rendering/FPS drop; exact impact needs measurement.
- Entity enumeration, UI updates and lock/log work are candidate mod costs;
  none has been established as the bottleneck. The analysis found no unbounded
  cache/queue growth and reported concurrent background simulations.
- Death-window correlations are mixed and do not establish the grayscale shader
  as a cause. Other enabled mods and vanilla workload remain alternatives.

If resumed, prioritize useful late-match evidence with reduced repetitive
logging and bounded aggregate profiling, then controlled comparisons. Preserve
the improved responsiveness rather than reverting pacing without evidence.


## 0.46.0 pending user check

- Purchase catalogue: fixed native context/settings pointer chain; detailed rejection diagnostics added. Runtime parity with actual automatic purchases remains to be tested.
- Construction picking: generic tower/nexus name aliases now resolve full sprite metadata, with body corner markers and modest feet-origin estimate. Crystal/base alignment at multiple zooms remains a visual game check.
- HP: deep green fill and outlined white numbers; readability pending user confirmation.
- Optional low-HP effect: stronger static tint through cached native UI nodes, battlefield/minimap clipping preserved; visible rendering and performance pending user test.
- Tab style: independent HTML proposal/comparison created and browser checked. User review is required before native implementation. Previous HTML files retained unchanged.


## 0.47.0 pending user check

User confirms all 0.46.0 fixes work. Tab design approved with team-name stripe removed; user authorized direct adaptive-slot implementation. Native styled Tab and adaptive HUD slots are now installed. Live rows use native roster stats/items/respawn; capacity follows the automatic buyer's target vector. Synthetic four/five/six layout, unknown-read retention and match reset tests passed. Native Tab rendering, all six current-save tooltips, native restoration on F12 and actual vanilla/five-slot environments await user testing. No new HTML approval is needed. Prior previews remain as historical references.


## 0.48.0 pending user test

0.47 Tab reopen failure corrected by synchronizing cached hide/show properties. KDA colour tags corrected; unidentified erroneous field needs visual recheck. Branch purchase shortfall range compacted with per-branch remaining gold in tooltip. Native verification pending; see TESTING-48.md.


## 0.49.0 pending user test

User confirms both 0.48 corrections work. Ghost Q/W parameter resolution, metadata icon alignment and informational/click-through Tab updated; native verification pending (TESTING-49.md).


## 0.50.0 pending user test (diagnostic only)

Investigation (docs/investigation-pass-48.md) found the 0.35 attack-backswing release has never fired: the native attack consumer either queues a delayed effect without locking the action, or locks action 3 without queuing, while the gate required both. The display switches to idle on the native stop event and to run on movement, so a post-hit release should look like a real cancel without display writes (generic view only; champion-specific views unreviewed). 0.50 adds bounded `ATTACK TRACE_*` records only. No cancel behaviour is changed. Hover outline remains deferred. See TESTING-50.md.

0.50 result (Ninja, Gunner): Ninja attacks lock for about 20 ticks with a
declared hit at tick 13. Move clicks after the hit are ignored until the lock
ends, and Q waited about 90 ms. Gunner attacks never lock, which is native
behaviour. See the results section of docs/investigation-pass-48.md.
Post-hit release is proposed as 0.51 and awaits the user's go-ahead.


## 0.51.0 pending user test

The user approved post-hit release, and it is implemented and installed.

- Locked BaseAttacks record their hit tick: start timing `+4a8` × 100 / speed
  factor `+80`.
- Release needs `+78` strictly greater than the hit tick, an empty queue, the
  native CC gate and the attack's own cooldown.
- The release reason is either a buffered skill or a manual ground Move / S
  made after the attack started.
- The native stop event is emitted and only the action is cleared.
- Windup cancel and skill-recovery cancel remain out of scope.

Open questions for the game test:

- Does Ninja's view (custom display code) visually cut the swing?
- Does the hit always land when clicking away right after it?
- Is the attack rhythm unchanged when standing still?

See TESTING-51.md.

2026-10-08 user result (Ninja, base attack speed): the swing cuts cleanly and
every hit landed. The log shows 36 manual and 3 buffered-skill releases at
tick 14 (16–17 on later clicks), and an unchanged rhythm when standing still.
One attack (id 11) had ground clicks but no release; samples stopped at
elapsed 9, possibly a stall, so watch for it. Still untested: attack-speed
items, other locked melee champions, modded champions, and ranged locked
champions.

Implemented in 0.53 (toggle below): Settings → Combat & casting → Attacks →
Cancel attack wind-down, On by default; pending game check (TESTING-53.md).
Hover outline toggle will be added with the outline prototype.

Deferred (user decision, 2026-10-08): add an on/off toggle for post-hit attack
cancelling. It belongs in the new settings window the user is designing, not
the current more-menu. Default and placement follow that design. Until then the
cancel is always on while controlling. The compatibility notes behind the
toggle:

- Attack speed is handled by the native formula.
- Modded attacks with no data, a non-BaseAttack kind, queued effects or the
  queued branch stay off.
- A residual risk remains for scripted mods with late-swing effects that the
  data does not reveal.
- Champion-specific views may not cut visually.

## Shop (0.59 logging)

User chose a Manual shopping settings option (auto-buy fully off for the
controlled champion while On, not tied to the shop window). Read-only pass in
docs/shop-investigation.md. 0.59.0 logs purchase steps and buyer-code
integrity; awaiting the user's match log (ideally with the Riot item mod).
Half-built-part behaviour after a retarget cannot be observed without a write
and needs a later experiment build.

## After the shop (2026-10-08)

The shop is done (0.64.2). Remaining investigations — cursor flicker, selection
shape and hover priority, League-style emotes, performance — are recorded in
[investigation-pass-65.md](investigation-pass-65.md), including the agreed
hover-priority order (no blanket champion-first rule). 0.65 is the diagnostic
build: cursor ownership trace, performance measurement, pre-lock rejection in
the steering and shop hooks.
