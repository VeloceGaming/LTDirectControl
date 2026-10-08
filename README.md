# LT Direct Control

Build **0.63.0** makes the shop behave like League's: the recipe tree stays on the item picked from the list while clicks inside it only change Builds into, Buy and the detail; a cursor-following hover tooltip; prices show the gold still needed (red when unaffordable); and Manual shopping now also covers the starting item. See [TESTING-63.md](TESTING-63.md).

Build **0.62.0** fixes the shop window: readable stats/effect text, a flowing detail panel with a Buy button that names its item, multi-select stat filters from live item stats, an optional Pause while open checkbox, and a strip purchase column that opens the shop and follows the Manual shopping queue. See [TESTING-62.md](TESTING-62.md).

Build **0.61.0** adds the native shop window (P or the strip's bag/gold button): Recommended and All items with filters, level-grouped grid, stats and effect text, cheapest recipe, Builds into, Buy/Queue and queue chips. With Manual shopping on nothing is bought unless you buy it in the shop; missing item data fails closed. Hook proof lines in the log confirm the hooks reach your champion. See [TESTING-61.md](TESTING-61.md).

Build **0.60.0** adds the game side of Manual shopping (Combat & casting > Shopping, Off by default): optional hooks on the native buyer's two in-base questions answer from your queue for the controlled champion only; the game still validates and performs every purchase. P temporarily queues the next build-list item until the shop window exists. Item tooltips keep the item mod's inline stat icons. See [TESTING-60.md](TESTING-60.md).

Build **0.59.0** adds a diagnostic-only shop trace: every purchase step of all
ten players and a once-per-match check of the native buyer code against the
reviewed bytes (names any mod that redirects it). Nothing native is written;
0.58.2 behaviour is unchanged. See [TESTING-59.md](TESTING-59.md) and
[docs/shop-investigation.md](docs/shop-investigation.md).

Build **0.58.2** switches both native full-screen flags during direct control,
so battlefield geometry, kill announcements, kill feed and spectator controls
use the same layout. F12 restores both saved preferences. This replaces the
extra compact-button hiding workaround from 0.58.1. Accepted outlines stay at
hover **1.2**, attack target **2/3** and click peak **3.0**, with the same **120 ms**
animation. See [TESTING-58.2.md](TESTING-58.2.md) and
[implementation notes](docs/native-fullscreen-layout-58.2.md).

Build **0.58.0** makes the persistent attack-target outline one-third thinner.
An explicit attack click briefly thickens it for 120 ms, then settles back:
click pulse → hover → attack target, with one outline per unit. Hover appearance
is unchanged. Direct control now temporarily forces the full battlefield layout
and restores the spectator's saved layout on release, addressing the full detail
panel carrying into future matches. Native testing is pending. See
[TESTING-58.md](TESTING-58.md) and [implementation notes](docs/selection-pulse-layout-58.md).

Build **0.57.0** adds a thin persistent red outline for the active attack target.
Hover remains stronger and controls the cursor; hovering the attack target merges
into one outline. Visibility/death, changed orders and inactive control clear
target feedback. Interface → Sprite outlines controls both; debug ground markers
remain independent. The user confirmed 0.56.1; this change awaits native testing.
See [TESTING-57.md](TESTING-57.md).

Build **0.56.1** corrects the debug selection setting to cover selected
attack-target markers as well as hover markers. Both are hidden when Show
selection markers is Off. See [TESTING-56.1.md](TESTING-56.1.md).

Build **0.56.0** adds Interface settings for hover sprite outlines (On) and
Debug → Show selection markers (Off). Debug restores the existing hover circles
and construction markers without changing selection. Attack-order markers stay
visible independently. Vision-mode hover feedback now follows the hovered button
instead of tinting the current selection. The user confirmed 0.55.3 outlines
work on all unit types; these settings/UI changes await in-game testing. See
[TESTING-56.md](TESTING-56.md).

Build **0.55.3** addresses the tower-only outline after the 0.55.2 test failed.
Ordinary unit drawings also contain NinePatch ground circles and sometimes text;
the wrapper now preserves the original drawing and uses native cleanup for unused
commands from extra outline passes. Blue ally/red hostile colours, single hover
priority, F10 crisp/wide comparison and ground indicators remain. See
[TESTING-55.3.md](TESTING-55.3.md) and
[outline investigation](docs/outline-investigation.md). Native rendering awaits
the user's test.

Build **0.54.0** applies the updated settings preview: compact checkbox rows for On/Off options, hover on the correct side of segmented controls, and outlined controls with subtle contact shadows. See [TESTING-54.md](TESTING-54.md) and [notes](docs/settings-54.md).

Build **0.53.0** rebuilds the settings window to the approved preview's measured layout (sections, segmented controls, dropdowns, icon close button, clipped scrolling with a scrollbar), adds **Combat & casting → Attacks → Cancel attack wind-down** (On by default), and reorders the bottom strip: Pause · Return to AI · Lock camera · Settings (gear) · vision group · K/D/A · CS · Tab, with B recall beside the HP bar and the "…" menu removed. See [TESTING-53.md](TESTING-53.md) and [notes](docs/settings-strip-53.md).

Build **0.52.0** adds the approved settings window and configurable keybinds, Hold/Toggle champion-only targeting (Hold default), per-skill casting modes, camera/interface options, and Shift + right-click attack-move. Opening Settings pauses a running match and only resumes a pause it caused. Existing cursor/low-HP preferences are retained. See [TESTING-52.md](TESTING-52.md) and [settings structure](docs/settings-52.md). Native rendering and gameplay await user testing.

Build **0.51.0** adds post-hit attack cancelling for locked basic attacks: after the hit tick, a buffered skill or a manual ground move/S stop releases the remaining wind-down through the game's own stop event. Cooldowns and damage are unchanged. See [TESTING-51.md](TESTING-51.md).

Build **0.50.0** adds diagnostic-only basic-attack timing records (`ATTACK TRACE_*` log lines) to decide how attack-animation cancelling could work; gameplay is unchanged from 0.49.0. See [TESTING-50.md](TESTING-50.md) and [investigation notes](docs/investigation-pass-48.md).

Build **0.49.0** resolves Ghost's Q/W description values and adjusts cooldown/range tooltip icon alignment. Tab is informational with no row hover tint, and battlefield controls pass through it. The accepted 0.48 Tab reopening and purchase display fixes remain; native rendering and click-through await user verification.

Native rendering and gameplay await the user's test: [checklist](TESTING-47.md), [implementation notes](docs/adaptive-tab-pass-47.md). Previous HTML previews remain available for reference.


Personal direct-control mod project for Teamfight Manager 2 on Windows/Steam.

## Current status

Build **0.43.0** enlarges the minimap within its existing footprint, with a slim
warm-grey frame and muted corner details. Native markers, fog, objective timers,
camera rectangle and map controls scale together. See [TESTING-43.md](TESTING-43.md)
and [minimap notes](docs/minimap-pass-43.md). Native visual and interaction checks
await the user's test. The broader approved HUD redesign remains separate.

Build **0.42.0** migrates native hooks and skill/item readers to the exact game
**0.6.3** build. Both game baselines are preserved privately; reviewed profiles
and field-consumer guards make later patch investigation repeatable. See
[TESTING-42.md](TESTING-42.md) and [migration notes](docs/patch-migration-pass-42.md).
Native gameplay remains pending the user's test. The approved revised HTML HUD,
button motion and tower fit remain a separate follow-up.

Build **0.41.0** implements the supplied HUD anatomy with separate 80 px skill
tiles, a centered health bar and a flat neutral bottom strip. Larger KDA/CS,
six inventory slots and the working purchase tracker retain their priority.
It adds explicit cursor-slider visuals, taller minion picking and larger
tower/nexus body areas. See [TESTING-41.md](TESTING-41.md) and
[HUD/selection notes](docs/hud-selection-pass-41.md). Native rendering and play
await the user's test; offline template layout and automated checks pass.

Build **0.40.0** adds upward-only champion head coverage, newest-only click
markers expiring in 250 ms, and visual range/self-area previews when hovering
skill HUD icons. Normal-cast previews retain priority; hover cannot arm a cast.
See [TESTING-40.md](TESTING-40.md) and [hover feedback notes](docs/hover-feedback-pass-40.md).
Native fit and rendering await the user's test. True sprite outlines remain
planned, pending a verified current rendered frame/transform for each entity.

Build **0.39.0** reduces champion picking areas using idle/walk dimensions and
capped combat-pose growth, retaining foot forgiveness. Supplied SVG cursor art
now covers enemy hover, A aiming, champion-only targeting and normal-cast
validity. A saved 24–64 px cursor-size slider lives in the three-dot menu;
default/reset is 32 px. Click markers contract and fade using the supplied motion.
See [TESTING-39.md](TESTING-39.md) and [cursor/selection notes](docs/cursor-selection-pass-39.md).
The user reports selection nearly correct, with heads often missed, markers too
long-lived and no skill hover range. Full cursor transition/slider coverage was
not reported.
HUD layout stays unchanged apart from that setting; later visual HUD work follows
the [confirmed design decisions](docs/design-integration.md).

Build **0.38.0** separates casting input from affected shape and placement.
Recognized live effect families supply dimensions; enabled declarations are a
fallback for opaque effects. Self-centered areas, unit-targeted corridors and
forward-offset areas no longer share one generic preview. Unsupported families
keep a limited guide and report their identity in the log. See
[TESTING-38.md](TESTING-38.md) and [geometry notes](docs/preview-geometry-pass-38.md).
The user tested Whip Master W/R and reported both previews work well. Other
skill families remain available for opportunistic in-game testing.

The user tested **0.37.0** and reported directional projectiles no longer seem to
auto-aim. Its Whip Master W/R and Archangel R previews were incorrect. Those
reports led to this geometry pass; automated tests cannot confirm their native
rendering or effective damage areas.

Build **0.36.0** isolates native skill, attack and movement observations to the
registered live simulation worker. Steering uses the current native position
borrow, and an additional automatic-attack call is observed for buffered skill
weaving. Cast diagnostics identify cancellations, expiry, target misses and native
rejections. Pre-hit attack cancellation remains deferred. See
[TESTING-36.md](TESTING-36.md) and [control notes](docs/control-pass-36.md).
The user verified diagonal walking, improved casting and no phantom Q counts.
A skillshot still redirected at 05:25, addressed for testing in 0.37.0.

Build **0.35.0** favors a skill press captured together with a mouse click,
uses native direct stepping on the controlled champion's verified clear path
segments, and lets a ready buffered skill release an eligible basic attack's
remaining animation after its effect commits. Attack cooldown and wall routing
are retained. Cast-count display uses declared counts and bounded retention;
logs now rotate instead of stopping mid-match. See [TESTING-35.md](TESTING-35.md)
and [control pass notes](docs/control-pass-35.md). The user's Ninja test still
showed diagonal detours, a phantom Q count, and Q/W cast failures. The log had no
steering or attack-start records. Corrections to observer ownership, steering
authorization and attack coverage follow in 0.36.0; in-game results remain pending.

Build **0.34.0** retains skill artwork and last-known item icons during brief
live-data gaps, while health and cooldown readings remain freshness checked.
Ordinary self-hover feedback is suppressed without filtering self from ability
targets. Movement is unchanged; the route review found continuous positions and
grid-derived intermediate goals, not evidence that discrete tile steps force
zig-zag movement. See [TESTING-34.md](TESTING-34.md) and
[implementation/route notes](docs/artwork-self-pass-34.md). The user verified skill
artwork stability and self-hover removal; a phantom Q count and movement turns
remained, addressed for testing in 0.35.0.

Build **0.33.0** fixes unrelated AI callbacks erasing teammate hover data and
tracks its freshness separately from attack targets. Base monster picking uses
idle/walking art instead of expanded attack frames, with smaller bounded padding
and matching highlights. Champion and minion bounds are retained. All native
hook sites and pacing remain as in 0.32.0. See [TESTING-33.md](TESTING-33.md);
the user verified the hover and monster corrections.

Build **0.32.0** reduces simulation lead to one frame, wakes the worker when
playback consumes a frame, and captures running gameplay input before the HUD
update. Champion/monster mouse boxes and highlights share enlarged sprite
bounds; minions retain smaller boxes. The purchase tracker includes tier-4
final upgrades, and declared skill tooltip values are resolved with native
stat colors. Bounded records connect captured commands to dispatch, publication,
playback and native queued aim. See [TESTING-32.md](TESTING-32.md).
The user confirmed improved responsiveness, champion selection and item/gold
reminders. Ally highlight and monster-size corrections follow in 0.33.0;
aim-assist and FPS concerns remain separate investigations.

Build **0.31.0** uses champion-first sprite rectangles for hovering and clicks,
including legs, with smaller minion boxes and monster art dimensions. It corrects
a reversed registered-item upgrade graph introduced in 0.30.0 and records purchase
forecasts when the build or inventory changes. See [TESTING-31.md](TESTING-31.md).
Native selection feel and purchase accuracy require the user's game test.

Build **0.30.0** reads effective registered item prices, stats and upgrade links
for the automatic purchase tracker and item descriptions, preserves native
stat-color markup in tooltips, and prevents background map callbacks from
erasing the active route. Native item pointers never leave the AI callback.
The user verified tooltip colors and route persistence; purchase tracking still
failed. The upgrade-edge interpretation was corrected in 0.31.0. See
[TESTING-30.md](TESTING-30.md). Ninja movement and frame drops remain deferred.

Build **0.29.0** adds an on-screen respawn countdown and minimap right-click
movement with a remaining route. Accepted orders survive Alt-Tab. Cast requests
wait for native execution, and identical movement destinations no longer restart
the native movement event. The item observer export is repaired. Plain-walking
turns and cast weaving require the user's game test; see [TESTING-29.md](TESTING-29.md).

Build **0.28.0** implements the approved compact bottom-strip HUD: large skill
icons and green health above the strip, horizontal KDA/CS, controls, and a
read-only automatic purchase tracker below. Champion selection appears before
starting; team availability appears only while Tab is held. Death greyscale is
scoped to the battlefield, leaving the minimap and HUD colored. Random item
branches are shown as uncertain rather than guessed. The user verified the layout
and death shader; purchase tracking remained incomplete through 0.30.0. See [TESTING-28.md](TESTING-28.md)
and [HUD implementation notes](docs/hud-pass-28.md).

Build **0.27.0** improves cast buffering, self-casting, targeted approach-to-range,
champion-only attack-move and ally hover feedback. Multi-use skills show remaining
uses from the native cooldown budget. Body picking includes attack/skill pose
envelopes; exact live animation alpha picking remains unavailable. Startup HUD
capture and button colors are corrected. The redesigned HUD is a separate review
preview, not installed in this build. Gameplay testing is pending; see [TESTING-27.md](TESTING-27.md).

Build **0.26.1** removes the development-folder requirement. Enable the mod;
no D: drive or activation files are needed. Logs use
`%LOCALAPPDATA%\LTDirectControl`, falling back to `%TEMP%\LTDirectControl`.
Failure to open either log folder no longer prevents direct control.
Match-result diagnostic reports use the same selected folder under `research`.
The 0.26 UI and gameplay are retained and still await the next in-game test.

Build **0.26.0** adds portrait-only selection while the battlefield is held.
Click one of your five champions, then the triangle to start; selection locks
only when playing starts. Preparation has no automatic 120-second release.
Grayscale HUD framing uses original graphical controls, yellow active accents,
numeric K/D/A (deaths red), item/gold symbols and portrait respawn countdowns.
Hover abilities/items for host-localized descriptions. Unknown formula values
are shown as ellipses, not guessed; localization and wrapping await game testing.
Routine diagnostic text is kept in the log. Pause/AI controls use icons.
The user confirms improved 0.25 body picking; exact moving-pose fit remains
unmeasured. Gameplay and all twelve native hook anchors remain unchanged.
It commits A + left-click directly on an enemy to that target,
chasing it instead of switching to a minion. Ground A-click keeps nearest-to-click
attack-move. Enemy hover and current attack-target rings share the click picker.
Two compact team portrait rows show deaths and native respawn countdowns.
Exorcist Q rejection now explains its native ally-under-crowd-control requirement.
The user confirms 0.23 works; its log shows all three controlled sets and a side
swap. Match 33's recorded replays match the controlled battles and its 2–1 win.
The new UI awaits the 0.26 game test.
Existing session buttons, A attack-range aiming, targeting, casting, recall,
camera controls and unlimited running duration remain.
See [TESTING-25.md](TESTING-25.md), [session controls](docs/session-controls.md),
[targeting](docs/targeting.md), [player HUD](docs/player-hud.md) and
[ability controls](docs/abilities.md).

The user reports all 0.16 checks work, including persistent aiming and recall.
The user confirms 0.18 HUD rendering works and the unwanted popup is gone.
The user confirms all 0.19 features work and reports 0.20 successful overall,
including reaching a result after returning to AI. The 0.20 log confirms
Pause/Resume and release around 08:10, followed by a 10:00 result. They report
missing modded icons while alive and a blank HUD on death; these are the 0.21
test targets. In 0.21 the user confirms the countdown, but vanilla skill art
and simultaneous item icons still fail and death blocks MMB dragging. The run
reached 11:00; these rendering/navigation issues are the 0.22 targets.
Recorded-result authority is verified for the latest series, match 33; disk save
reload persistence was not separately tested. Startup, Ready and heartbeat
recovery guards remain. One foreground worker is controlled per session;
Return to AI ends that battle's control. Read-only history candidates from other
runs still need comparison. Manual shopping remains optional future work.

The goal is independently editable source, direct champion control on game 0.6.2,
and a compact HUD inspired by the supplied screenshot. Confirmed priorities:

- Attack-move selects the eligible enemy nearest the click, not the champion.
- A enters persistent attack-move aiming with an attack-range circle; left-click confirms.
- A-click directly on an enemy commits to it; a ground click acquires nearest the click.
- Enemy hover and persistent attack-target markers identify the picked/ordered target.
- Always-visible team portraits dim dead champions and show native respawn timers.
- Q/W/R quick-cast immediately at the cursor; targeted abilities acquire a valid
  unit under the cursor without requiring a second left-click.
- Shift + press Q/W/R enters normal-cast aiming. Releasing keeps the display;
  left-click casts and other gameplay commands cancel it.
- B starts the game's native recall channel.
- Backtick or Mouse 4 toggles champion-only direct unit targeting; attack-move is unaffected.
- Select who to play before the match. Switching during the match is low priority.
- Hold Tab to show team information; release to hide it.
- Display skill, gold and item information in the compact HUD. Manual buying is
  optional pending native purchase-path feasibility; the SDK build hook runs once
  and does not provide live purchasing.
- Camera is a core control: free by default, Y toggles lock, and holding Space
  temporarily follows. Reuse native minimap navigation. See [camera behavior](docs/camera.md).
- Use Start control, Pause/Resume and Return to AI buttons for session control.

See [the investigation](docs/investigation.md) for the verified compatibility
failure, available resources, and implementation milestones.

## First playable milestone

Before polishing the HUD, demonstrate a live single-player match in which:

1. The player selects one champion reliably.
2. Right-click movement changes the actual continuing simulation.
3. Other champions keep using the native AI.
4. Ending the match produces a result consistent with what was played.
5. Releasing control and leaving the match clean up the hooks and pending orders.

Camera behavior and accurate cursor mapping are part of mouse-control work.
Full targeting, attack-move, range previews and the final HUD follow the playable
foundation; the agreed camera design supersedes the older investigation roadmap
that deferred camera controls until HUD work.

## Development and first game test

### Install the downloadable mod

Download the ZIP from [Releases](https://github.com/VeloceGaming/LTDirectControl/releases).
Close the game, then extract its `lt_direct_control_probe` folder into
`Teamfight Manager2/mods/`. Enable **LT Direct Control** and restart the game.
Keep the included `ui` and `font` folders alongside the DLL and `mod.mod_info`.
After drafting, choose your champion portrait and click the triangle to start.
Use game version **0.6.3** with the current build on Windows x64; exact native executable
checks still apply. Older releases require their matching game build.
See [TESTING-26.1.md](TESTING-26.1.md) for the portable-install test.

### Build and diagnostics

The user confirmed that no demo/source link is available: develop independently.
The standalone targeting core lives in `src/lib.rs`; `cargo test --offline`
passes nine targeting tests and nine historical timing-policy tests. Probe
tests verify native ABI/relays, metadata reads, quickcast/preview cancellation, config restoration, publication
gating, explicit Start, cancellation, finite guards, ownership and command expiry.
This library does not dispatch game commands.

The `probe` project builds against a copy of the installed 0.6.2 stable SDK.
It observes client scenes and simulation origins through the stable SDK.
When enabled, probe 0.26.1 installs four in-memory
CALL redirects and one movement-consumer tail JMP redirect at title, after verifying the executable SHA256 and loaded code.
It leaves the executable file and saves untouched. Selected-champion mouse
movement and stop are enabled without flag files. Native AI remains in use until explicit
Start and is restored on release. Startup has a client-enforced 15-second guard; Ready expires
only on Start/AI or a missing heartbeat; running has no timed cutoff. F12,
heartbeat loss after Ready (including pause) and battlefield exit release coordination. One
foreground run is controlled per session; restart before replacing the DLL.

Build it with:

```powershell
cargo build --release --offline --manifest-path probe/Cargo.toml
```

Its installed/package folder must be named `lt_direct_control_probe` and contain
`lt_direct_control_probe.dll`, `mod.mod_info`, and the included `ui` and `font` folders.
It logs to `%LOCALAPPDATA%\LTDirectControl\probe.log`, retaining the prior
recording as `probe.previous.log` on launch. If that folder is unavailable,
it tries `%TEMP%\LTDirectControl`; logging failure does not disable the mod.
Unknown/background samples are capped so they do not consume the foreground
log budget.

The first recording identified foreground match 33, set 1. Its simulation
advanced 559.983 seconds in 6.643 wall-clock seconds (84.297x). A foreground
origin is therefore not evidence of synchronous/live gameplay. Its playback
clock and saved-result authority still need separate verification.

The second timing attempt stopped at tick 15 after 226.6 ms because the display
heartbeat paused during startup. Worker and display thread IDs differed. The
display clock then advanced steadily from 00:00 to 00:37. The user reported
normal movement and a visible overlay. This establishes successful guard
release, not successful live pacing. The third probe allows the observed
startup gap (988 ms until the first sampled battlefield clock) within a finite
grace period, with the original strict limit restored after battlefield entry.

The third probe paced 1200 simulation ticks in 20.000378 seconds. Battlefield
readiness was observed at tick 63, 1.030218 seconds after worker start. Playback
continued during incremental generation. The experiment stopped with reason
Finished, matching the user's clock observation of 0:19. A roughly one-second
generation lead remains; the whole-second UI clock cannot establish precise
input latency.

The fourth probe stopped for MissingHeartbeat after 1.976405 seconds; its first
sampled battlefield clock appeared 2.603 seconds after worker start. No manual
inputs were dispatched. It also selected blue top incorrectly while the user's
team was red. These explain the failed movement test.

The fifth probe held tick 3 for 5.000651 seconds and stopped for StartupTimeout.
The battlefield clock appeared 520 ms after release. It cached placeholder team
zero with an empty roster before management data loaded, so selection never
resolved. Probe 0.6.0 rejects incomplete ownership reads and tests a 60-tick
bootstrap before holding. That boundary is a hypothesis informed by the earlier
readiness at tick 63; actual publication and playback coordination remain pending.

See [TESTING-26.1.md](TESTING-26.1.md) for the next test. Previous recordings and
summaries are retained in `research/`. Ctrl+1..5 selects an own-team lane before
the match. Ownership is resolved after management data loads, from the human
team ID and athlete contracts, then matched to the current simulation roster.
Unknown or ambiguous ownership preserves AI. On the held battlefield, click
one of your five portraits, then the triangle to start. Pause bars/triangle
hold/resume; the chip returns control to AI. F11 starts/resumes; F12 returns control to AI.

One matching foreground run is controlled per session; background matches and
replays retain normal behavior. Later sets can prepare a fresh session in 0.23.
The publication hook enforces
pacing even when a champion dies. Broader champion coverage remains a game-test
task; the latest recorded series is verified. Historical tests above record the investigation,
not the current controls.

The game installation, current SDK, Rust compiler, original Workshop package,
and existing diagnostic logs have already been located. There is no need to
upload the entire game or learn programming to proceed.

The first implementation target is single-player. Multiplayer and replay
support require separate investigation and are not assumed to work.
