# LT Takeover: Open-Source Direct Control

League of Legends–style direct control for **Teamfight Manager 2**: take over
one of your athletes during a match and play them yourself, with right-click
movement, attack-move, Q/W/R casting, a HUD, stats panels, a Tab scoreboard,
an item shop and an emote wheel.

This is meant first as a **modder resource**: a working base, with all source
code and reusable original assets open and released into the public domain,
that you can reshape into exactly the direct-control mod you want. It still
plays (mostly).

> **Just want to play?** Try
> [Control on the Steam Workshop](https://steamcommunity.com/sharedfiles/filedetails/?id=3813636149).
> It is a much more complete gameplay mod. This project did not reverse-engineer
> Control or Harbinger Direct Control; all respect to both creators.

## Status

- Version **1.0.0**, for game version **0.6.3** on Windows x64 only. The mod
  checks the game executable's fingerprint and stays inactive on any other
  build; a game update needs a mod update.
- Single player; one controlled athlete per match.
- Works with the vanilla item set (4 slots) and the Riot item mod (6 slots),
  and with Workshop champions (tested with Leef's Variety, Glimmer's Variety
  Pack and Oppi OC). Skill previews are approximate, especially for modded
  champions.
- The mod's text follows the game's language (all 17), or any language you
  pick in Settings › Interface › Mod language.

## Controls

All bindings can be changed in the mod's Settings window (gear button).

| Action | Default |
|---|---|
| Move / attack the clicked target | Right-click |
| Attack-move | A, then left-click; or Shift + right-click |
| Select a unit to see its stats | Left-click (ground clears it) |
| Target champions only | Hold \` or mouse button 4 (can be set to toggle) |
| Stop / Recall | S / B |
| Abilities | Q, W, R (quick cast by default; normal cast or cast-on-release per ability) |
| Show range / normal-cast override | Shift + Q/W/R |
| Self-cast | Alt + Q/W/R |
| Choose your athlete (before Start) | Ctrl + 1–5 (top, jungle, mid, bottom, support) |
| Start, pause, resume or reclaim control / hand control to AI | F11 / F12 |
| Your stats panel | C (toggle) |
| Team details | Hold Tab |
| Shop | P, or the gold button on the HUD |
| Emote wheel | Hold T, choose a direction, release; Escape/right-click cancels |
| Lock camera / hold to follow | Y / Space |
| Drag or pan the camera | Middle mouse / I J K L, edge scrolling, mouse wheel to zoom |

## What's in it

- **Controls**: persistent orders like League (move, attack, attack-move,
  stop, recall), skill aiming with range previews, attack wind-down
  cancelling (on by default; it frees movement earlier, it does not raise
  attack speed), an acquisition radius for ground attack-move, and
  League-style cursors.
- **HUD**: a bottom strip with portrait, health, skills and items; your
  stats beside the skills (C); a target frame at the top left for any unit
  you left-click; a Tab scoreboard for all ten players; hover and target
  outlines on units; an optional low-health screen effect. One UI colour
  setting recolours every panel.
- **Shop**: turn on *Manual shopping* (Settings › Advanced › General) and the
  game stops auto-buying for your athlete. Press P to buy anything,
  League-style: the AI's whole build plan as Recommended, an item grid with
  stat filters, complete alternative upgrade paths and Builds into. Prices
  show the gold still needed from your inventory. *Vanilla order* (on by
  default) finishes one item before starting the next, like the game. With
  Manual shopping the shop opens by itself at the start of a match, and it
  can pause the match while open.
- **Session controls**: Start, Pause and Return to AI buttons. F12 hands the
  selected champion to AI; F11 or Take control reclaims the same champion in
  that match. AI playback stays at 1x so the worker remains synchronized.
  Changing the game speed or pressing View Match Result in AI control hands
  the match fully to the game (it can no longer be reclaimed).
- **Emotes**: five original faces plus four bundled images, shown locally
  above your champion for 2 seconds. Settings › Emotes controls height,
  scale, camera zoom behaviour and cooldown (default 1.5 seconds), with a
  silent live preview. Custom static PNGs are imported through Settings ›
  Emotes › Library: Open folder, add PNGs (up to 256 × 256 and 1 MiB),
  Refresh, assign to the wheel, Apply, then restart the game.
- **Languages**: all 17 game languages, detected from the game, with the
  game's own fonts for every script.

## Install

- **Steam Workshop**: subscribe, start the game from the title screen (the
  mod installs its game hooks there), then load your save.
- **Manually**: extract the `lt_direct_control` folder from a release zip
  into `Teamfight Manager2\mods\`. If an older build (before 1.0) is
  installed, delete its `lt_direct_control_probe` folder first: the game
  would otherwise load both.

In a match: pick your athlete with Ctrl + 1–5 while the match waits, then
press F11 or click Start.

Settings are saved in `%LOCALAPPDATA%\LTDirectControl\controls.json`; the log
is `probe.log` in the same folder (send it with a bug report). Settings ›
Advanced › Debug › Log detail chooses how much it records (Normal by
default; Verbose for investigations). The same page has optional detailed
performance capture; see
[emotes and performance](docs/emotes-and-performance.md).

Skill preview colours, widths and sizes can be overridden in
`%LOCALAPPDATA%\LTDirectControl\preview_style.json` (any subset of
`probe/src/preview_style.json`), then restart.

## Build from source

Requirements: Windows x64, Rust (stable, tested with 1.98), Python 3.12 for
the build tools; `pip install fonttools pillow` for the optional font and
glyph checks; Node.js only to regenerate artwork; the game installed (its
mod SDK and executable are used).

```
python tools/prepare_sdk.py
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

- `prepare_sdk.py` copies the game's mod SDK (`<game>/mod-sdk-stable`) into
  `sdk/`, which is not in this repository, and adds the mod's two local SDK
  additions. Run it once after cloning and after every game update.
- The verifier checks the staged DLL against the installed game (555 code
  anchors), the packaged artwork against `tools/records/`, and takes the
  release notes from `CHANGELOG.md`.
- Set `TFM2_GAME_DIR` if the game is not in Steam's default folder.

## Project map

Everything lives in `probe/src`. The mod ID, crate and DLL are
`lt_direct_control` (before 1.0: `lt_direct_control_probe`; settings and logs
stay in `%LOCALAPPDATA%\LTDirectControl`).

| Area | Modules |
|---|---|
| Entry points | `lib.rs` (logger, start-up wiring), `client.rs` (`Client`: the SDK client extension, one named step per feature each frame), `simulation.rs` (`Simulation`: the per-player AI callback, one step per duty) |
| Game hooks (0.6.3 only) | `native_adapter/`: `mod.rs` (the safe API the rest of the mod calls), `windows/mod.rs` (patch installation), `windows/layout.rs` (every 0.6.3 address and byte pattern), one file per hook family (`movement`, `combat`, `outline`, `view`, `input`, `shop`); `native_timing.rs` (session phases and frame pacing), `native_profile.rs`, `native_items.rs`, `native_preview.rs`, `worker_watch.rs` (stall report), `ai_handback.rs` |
| Orders and input | `platform_input.rs` (keyboard/mouse), `movement.rs` (orders and movement), `combat.rs` (targets), `acquisition.rs`, `abilities.rs` (casting), `camera.rs`, `wheel.rs`, `map_path.rs` |
| Picking | `sprite_picking.rs` (body sizes), `own_selection.rs` |
| HUD and windows | `player_hud.rs`, `stats_panel.rs` / `stats_ui.rs`, `team_status.rs`, `team_info.rs`, `session_ui.rs`, `settings_ui.rs` (+ `settings_ui/`), `shop_ui.rs`, `emotes.rs` / `emote_library.rs`, `minimap.rs`, `screen_effect.rs`, `skill_preview.rs` / `preview_style.rs`, `cursor.rs`, `tooltips.rs`, `hud_*.rs`, `ui_graphics.rs`, `ui_theme.rs` (UI colour), `ui_click.rs` |
| Text | `lang.rs` (translations, language detection) and `probe/lang/*.json` |
| Shop logic | `shop.rs` (orders, purchase plans, buyer answers), `shop_recipe.rs` (structural upgrade graph and alternative paths), `purchase_tracker.rs`, `inventory.rs` |
| Settings and shared state | `settings.rs` (options and key bindings), `ui_state.rs` (which windows are open, read by input and camera code) |
| Logging and diagnostics | `logging.rs` (log detail levels), `perf.rs`, `attack_trace.rs`, `shop_trace.rs`, `input_trace.rs`, `result_audit.rs`, `runtime_storage.rs` |

Other folders: `probe/ui`, `probe/font`, `probe/cursor`, `probe/sound`
(packaged artwork and sound), `probe/lang` (translations), `design/` (only
the design sources the art generators read; design pages, screenshots and
QA files stay local), `tools/` (build, verification and reverse-engineering
scripts), `docs/` (guides; `docs/history` holds notes from each development
pass).

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

- [docs/modding.md](docs/modding.md): **start here to modify the mod**: threads,
  the frame flow, texts and languages, and where to add keys, settings, HUD
  elements or shop rules.
- [CHANGELOG.md](CHANGELOG.md): every build.
- [docs/reverse-engineering-guide.md](docs/reverse-engineering-guide.md) and
  [docs/patch-migration.md](docs/patch-migration.md): how the game hooks were
  found and how to update them for a new game version.
- [docs/codebase-review-66.md](docs/codebase-review-66.md): known structural
  issues and the clean-up plan.

## License

Public domain ([The Unlicense](LICENSE)): use it for anything, no credit
needed. Fonts and icons keep their own licenses; the shop stamp and the four
bundled emote images are excluded from the dedication; the game's mod SDK is
not part of this repository. See
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
