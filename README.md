# LT Direct Control

League of Legends–style direct control for **Teamfight Manager 2**: take over
one of your athletes during a match and play them yourself, with right-click
movement, attack-move, Q/W/R casting, a HUD, a Tab scoreboard and an item
shop.

This is meant first as a **modder resource**: a working base, with all source
code and assets open and released into the public domain, that you can reshape
into exactly the direct-control mod you want. It still plays (mostly).

> **Just want to play?** Try
> [Control on the Steam Workshop](https://steamcommunity.com/sharedfiles/filedetails/?id=3813636149).
> It is a much more complete gameplay mod. This project did not reverse-engineer
> Control or Harbinger Direct Control; all respect to both creators.

## Status

- Game version **0.6.3** on Windows only. The mod checks the game executable's
  fingerprint and stays inactive on any other build.
- Single player; one controlled athlete per match.
- Works with the vanilla item set (4 slots) and the Riot item mod (6 slots).

## Controls

All bindings can be changed in the mod's Settings window (gear button).

| Action | Default |
|---|---|
| Move / attack the clicked target | Right-click |
| Attack-move | A, then left-click; or Shift + right-click |
| Target champions only | Hold \` or mouse button 4 (can be set to toggle) |
| Stop / Recall | S / B |
| Abilities | Q, W, R (quick cast by default; normal cast or cast-on-release per ability) |
| Show range / normal-cast override | Shift + Q/W/R |
| Self-cast | Alt + Q/W/R |
| Choose your athlete (before Start) | Ctrl + 1–5 (top, jungle, mid, bottom, support) |
| Start or resume control / return control to the AI | F11 / F12 |
| Team details | Hold Tab |
| Shop | P, or the gold button on the HUD |
| Lock camera / hold to follow | Y / Space |
| Drag or pan the camera | Middle mouse / I J K L, edge scrolling, mouse wheel to zoom |

## What's in it

- **Controls**: persistent orders like League (move, attack, attack-move,
  stop, recall), skill aiming with range previews, attack wind-down
  cancelling, and League-style cursors.
- **HUD**: a bottom strip with portrait, health, skills and items; a Tab
  scoreboard for all ten players; hover and target outlines on units; an
  optional low-health screen effect.
- **Shop**: turn on *Manual shopping* in Settings and the game stops
  auto-buying for your athlete. Press P to buy anything, League-style:
  recommended build, item grid with stat filters, recipe tree, Builds into,
  and prices showing the gold still needed. Purchases happen when your athlete
  is in base. The shop can optionally pause the match while open.
- **Session controls**: Start, Pause and Return to AI buttons; F12 always
  returns control to the AI.

## Install

1. Download a release zip and extract the `lt_direct_control_probe` folder
   into `Teamfight Manager2\mods\`.
2. Start the game from the title screen (the mod installs its game hooks
   there), then load your save.
3. In a match: pick your athlete with Ctrl + 1–5 while the match waits, then
   press F11 or click Start.

Settings are saved in `%LOCALAPPDATA%\LTDirectControl\controls.json`; the log
is `probe.log` in the same folder. Settings › Interface › Debug › Log detail
chooses how much it records (Normal by default; Verbose for investigations).
Log levels are assigned by line tag in `probe/src/logging.rs`.

## Build from source

Requirements: Windows x64, Rust (stable, tested with 1.98), Python 3.12 for
the build tools; `pip install fonttools pillow` for the optional font and
glyph checks; Node.js only to regenerate artwork.

```
cd probe
cargo fmt --check
cargo clippy --release --all-targets -- -D warnings
cargo test --release
cargo build --release
cd ..
python tools/stage_package.py
python tools/verify_build.py --version <version> --probe-tests <count>
python tools/package_mod.py --version <version>
powershell tools/install_verified_build.ps1 -Version <version> -PreviousVersion <installed version>
```

- The verifier checks the staged DLL against the installed game (148 code
  anchors), the packaged artwork against `tools/records/`, and takes the
  release notes from `CHANGELOG.md`.
- Set `TFM2_GAME_DIR` if the game is not in Steam's default folder.
- The game's mod SDK is expected in `sdk/mod-api-stable` (it comes with the
  game's modding kit and is not covered by this project's license).

## Project map

Everything lives in `probe/src`. The crate is still named
`lt_direct_control_probe` for compatibility with existing installs.

| Area | Modules |
|---|---|
| Entry points | `lib.rs` (logger, start-up wiring), `client.rs` (`Client`: the SDK client extension, one named step per feature each frame), `simulation.rs` (`Simulation`: the per-player AI callback, one step per duty) |
| Game hooks (0.6.3 only) | `native_adapter/`: `mod.rs` (the safe API the rest of the mod calls), `windows/mod.rs` (patch installation), `windows/layout.rs` (every 0.6.3 address and byte pattern), one file per hook family (`movement`, `combat`, `outline`, `view`, `input`, `shop`); `native_timing.rs` (session phases and frame pacing), `native_profile.rs`, `native_items.rs`, `native_preview.rs` |
| Orders and input | `platform_input.rs` (keyboard/mouse), `movement.rs` (orders and movement), `combat.rs` (targets), `abilities.rs` (casting), `camera.rs`, `wheel.rs`, `map_path.rs` |
| Picking | `sprite_picking.rs` (body sizes), `own_selection.rs` |
| HUD and windows | `player_hud.rs`, `team_status.rs`, `team_info.rs`, `session_ui.rs`, `settings_ui.rs`, `shop_ui.rs`, `minimap.rs`, `screen_effect.rs`, `skill_preview.rs`, `cursor.rs`, `tooltips.rs`, `hud_*.rs`, `ui_graphics.rs` |
| Shop logic | `shop.rs` (orders, purchase plans, buyer answers), `purchase_tracker.rs`, `inventory.rs` |
| Settings and shared state | `settings.rs` (options and key bindings), `ui_state.rs` (which windows are open, read by input and camera code) |
| Logging and diagnostics | `logging.rs` (log detail levels), `perf.rs`, `attack_trace.rs`, `shop_trace.rs`, `input_trace.rs`, `result_audit.rs`, `runtime_storage.rs` |

Other folders: `probe/ui`, `probe/font`, `probe/cursor` (packaged artwork),
`design/` (HTML design previews), `tools/` (build, verification and
reverse-engineering scripts), `docs/` (guides; `docs/history` holds notes from
each development pass).

### Threads: the one rule to know

Three kinds of threads run mod code:

- the **client thread**: SDK client callbacks, the native viewer hook, all UI;
- the **viewed match's simulation worker**: the AI callback for its players
  and the native movement, combat and shop hooks for that match;
- **background simulations**: other matches the game runs at the same time.
  They also pass through every native hook, hundreds of thousands of times a
  second.

So hook code must turn away anything that is not the controlled athlete
cheaply, before taking any lock, and must never wait on the client.

## Further reading

- [CHANGELOG.md](CHANGELOG.md): every build.
- [docs/reverse-engineering-guide.md](docs/reverse-engineering-guide.md) and
  [docs/patch-migration.md](docs/patch-migration.md): how the game hooks were
  found and how to update them for a new game version.
- [docs/codebase-review-66.md](docs/codebase-review-66.md): known structural
  issues and the clean-up plan.

## License

Public domain ([The Unlicense](LICENSE)): use it for anything, no credit
needed. Fonts, icons and the game SDK keep their own licenses; see
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
