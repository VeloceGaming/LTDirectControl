# LT Direct Control

Personal direct-control mod project for Teamfight Manager 2 on Windows/Steam.

## Current status

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
Keep the included `ui` folder alongside the DLL and `mod.mod_info`.
After drafting, choose your champion portrait and click the triangle to start.
Use game version **0.6.2** on Windows x64; native executable checks still apply.
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
only on Start/AI or a missing heartbeat; running has no timed cutoff. Ctrl+End,
heartbeat loss after Ready (including pause) and battlefield exit release coordination. One
foreground run is controlled per session; restart before replacing the DLL.

Build it with:

```powershell
cargo build --release --offline --manifest-path probe/Cargo.toml
```

Its installed/package folder must be named `lt_direct_control_probe` and contain
`lt_direct_control_probe.dll`, `mod.mod_info`, and the included `ui` folder.
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
hold/resume; the chip returns control to AI. Ctrl+Home/End remain backup shortcuts.

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
