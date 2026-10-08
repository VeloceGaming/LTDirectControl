# Codebase review after 0.65.2

Date: 2026-10-08. Read-only review; nothing was moved, renamed or built.
Goal set by the user: this mod should be a clean base that other people can
take and modify into exactly the direct-control mod they want. Frame rate
already matches vanilla, so this review is about clarity, structure and how
easy the project is to pick up, not speed. Counts marked "about" come from
text searches and are approximate.

## Progress

- Stage 0 done (2026-10-08): git works again; 0.65.2 committed and tagged.
- Stage 1 done (2026-10-08): Unlicense and third-party notices, new README,
  CHANGELOG.md, pass notes in `docs/history/`, test sheets kept local
  (`local/`, not published), verification records in `tools/records/`, tool
  paths in `tools/paths.py`, `tools/stage_package.py`, verifier renamed
  `tools/verify_build.py`, unused root crate removed. Line references to the
  old verifier below describe the state before this stage.
- Stage 2 done (0.66.0): Log detail setting (Quiet / Normal / Verbose),
  classified by line tag in `probe/src/logging.rs`.
- Stage 3 done (0.66.1): `movement_test` → `movement` (`MovementTest` →
  `Movement`), `timing_test` → `test_support`, `ClientProbe`/`AiProbe` →
  `Client`/`Simulation`, new crate and movement module docs. The mod ID and
  the `probe.log` file names stay until the public release.
- Stage 4 done (0.66.2): `native_adapter.rs` split into `native_adapter/mod.rs`
  (public API) and `windows/` (`mod.rs` patch install, `layout.rs` with every
  0.6.3 address and byte pattern, `movement`, `combat`, `outline`, `view`,
  `input`, `shop`, `tests`). Verified as pure moves. Entity field offsets are
  still inline; `native_profile.rs` was not merged into `layout.rs` (it holds
  build identity and layout guards checked at startup).
- Stage 5 done (0.66.3): `lib.rs` keeps only the logger and start-up
  wiring; `client.rs` and `simulation.rs` hold the two entry points, whose
  callbacks now call one named step per feature (moved verbatim, verified
  block by block). Window flags moved to `ui_state.rs` (`settings::MODAL` →
  `ui_state::SETTINGS_OPEN`, `shop_ui::OPEN` → `ui_state::SHOP_OPEN`), so
  input code no longer depends on the windows. Settings are created at
  start-up and shared with the cursor and client instead of being created
  inside the cursor. Left as they are: the purchase forecast and HUD use
  each other, and click markers are drawn in `cursor.rs`.

## Summary

The mod works and is carefully guarded, but it still looks like the research
prototype it grew from. A newcomer would struggle with five things:

1. **They cannot reproduce a release.** Git stops at 0.26.1 (2 October), with
   about 238 changed files since. Verifying and packaging a build needs
   ignored or machine-specific files (list below).
2. **Nothing tells them where to start.** There is no license, the README is a
   changelog that stops at 0.63, and the docs are mostly per-pass notes, with
   some (e.g. `docs/native-adapter.md`) stuck at 0.22.
3. **Diagnostics are mixed into features.** About 5–6 MB of log per match is
   written synchronously from game threads, mostly per-click and per-move
   traces, with no switch.
4. **Two files hold most of the hard parts.** `native_adapter.rs` (about 4,100
   lines: every game hook, patching, outline renderer, camera, shop hooks) and
   `lib.rs`, whose two main callbacks run about 480 and 530 lines each.
5. **Names still say "probe" and "test"** for core code.

None of this needs behaviour changes to fix. The plan below does it in small
stages, each kept behaviour-identical and play-tested.

## Already good (keep)

- Executable fingerprint guard: hooks install only on the reviewed 0.6.3 build,
  148 instruction anchors are checked at build time, and patch bytes are
  re-audited at runtime.
- Fail-closed design (e.g. Manual shopping buys nothing if item data is
  missing), F12 emergency release, heartbeat and loading guards.
- Every native hook body runs inside `catch_unwind`; a panic releases control
  instead of crashing the game.
- Clippy with warnings denied, formatting checks, 274 unit tests.
- Verified packaging, fingerprinted install and a rollback zip for every build.

## Findings, by importance to the goal

### 1. Reproducibility (critical)

- **Git:** last commit is `84ddb5b` (0.26.1). The `.git` folder is owned by
  another Windows account (`CodexSandboxOffline`), so git refuses to work for
  the current user without a `safe.directory` exception. Forty releases
  exist only as zips.
- **A clean clone can build the DLL, but cannot verify or package it:**
  - `tools/verify_ability_build.py` reads ignored `research/` files:
    `cursor-assets-0.39.0.json` (line 38), `hud-glyphs-0.44.0.json` (47),
    `ui-graphics-{version}.json` (52), `settings-glyphs-0.53.0.json` (56),
    `shop-glyphs-0.61.0.json` (61), `endfield-glyphs-0.45.0.json` (66),
    `fonttools-runtime` (71), `hud-fonts-0.45.0.json` (73).
  - It checks font sources at `C:/LTTool/endfield_ui_lab/ui/src/fonts` (75).
  - `tools/package_mod.py` line 19 needs `research/ui-graphics-{version}.json`.
  - The Steam path is fixed in `install_verified_build.ps1` and other tools.
- **Release notes live inside the verifier.** Each build hand-edits the
  `changes` and `previous_game_result` text in `verify_ability_build.py`.
- **"18 core tests" is misleading.** It is hard-coded in that script (lines 98
  and 113) and reported for every build, but those tests cover the root crate,
  which the mod does not use (finding 6).

### 2. Orientation for newcomers (critical)

- No LICENSE file.
- README is a per-build changelog stopping at 0.63.0; nothing explains what
  the mod is, how to install, build or modify it.
- 52 docs, mostly pass notes; 76 `TESTING-*.md` files at the repository root.
- No architecture overview, threading rules or "where do I change X" guide.

### 3. Diagnostics mixed with features (high)

Log volume over one full match (0.65.1), largest first: `MANUAL CLICK` ~1.1
MB, `MOVEMENT TRACE` ~0.6 MB, `MANUAL input` ~0.55 MB, `CONTROL DIAGNOSTIC`
(every second) ~0.48 MB, `MOVEMENT STEERING` ~0.42 MB, `NATIVE TRAFFIC` ~0.39
MB, `OUTLINE status` ~0.27 MB, `ABILITY RESULT` ~0.25 MB, `NATIVE
SPECTATOR_KEY` ~0.21 MB, then `INPUT TRACE`, `PERF`, `ATTACK TRACE_*`.
Diagnostic-only modules: `attack_trace`, `shop_trace`, `input_trace`,
`result_audit`, `perf`, the cursor trace in `cursor.rs`, proof counters in
`shop.rs` and `native_adapter.rs`.

Proposed three groups:

| Group | Examples | Default |
|---|---|---|
| Safety and proof | install/fingerprint results, release reasons, `SHOP MODE`, `SHOP UNEXPECTED`, patch audits | always on |
| Test | purchases, session phases, shop UI actions, slow frames | on (current workflow) |
| Verbose traces | per-click/move/steer/input, `CONTROL DIAGNOSTIC`, `NATIVE TRAFFIC`, `OUTLINE status`, `PERF`, `CURSOR TRACE`, attack traces | off |

### 4. Structure: two oversized files (high)

- `native_adapter.rs` (~4,100 lines, ~45 statics, ~190 `unsafe`, ~80 code
  addresses and ~160 raw field offsets) mixes: patch installation and
  relays, movement/steering, attacks and skills, aim, outline renderer and
  shader, camera and view leases, worker/viewer pacing hooks, shop hooks and
  native item reads.
- `lib.rs`: `ClientProbe::post_update_inner` (~480 lines) runs every client
  feature in one function; `AiProbe::think` (~530 lines) does session binding,
  HUD/roster sampling, shop publishing, purchase tracing and input.
- These two callbacks have no automated coverage (they need a live game), so
  splitting them must be pure moves with a play-test after each step.

### 5. Game-version knowledge is scattered (high)

What breaks on the next game patch, and where that knowledge lives:

| Feature | Runs on | Version-specific parts |
|---|---|---|
| Movement, stop, recall, steering | SDK input + native hooks | `native_adapter.rs` consts (MOVE/STEER/DIRECT_STEP sites) |
| Attacks, skills, aim | SDK input + native hooks | `native_adapter.rs` (ATTACK/SKILL/AIM sites, entity offsets) |
| Session pacing (start/pause/one-frame lead) | native worker/viewer hooks | `native_adapter.rs`, `native_timing.rs` |
| Camera, full-screen layout | native viewer hook | `native_adapter.rs`, `native_profile.rs` layout guards |
| Hover/target outlines | native render hook | `native_adapter.rs` outline sites, command tags |
| Manual shopping | native buyer hooks | `native_adapter.rs` thunks, gate at 0x146baf3 |
| Items, build, gold | SDK + native reads | `native_items.rs`, `native_adapter.rs` player fields |
| Skill previews | SDK drawing + native reads | `native_preview.rs` |
| Minimap | SDK drawing + profile | `minimap.rs` embeds `tools/native_profiles/0.6.3.json` |
| HUD strip, Tab, settings, session buttons, shop window | SDK UI | none beyond the data above |
| Cursor | Windows + exe import table | none (import name, not address) |

Version knowledge lives in three places: constants in `native_adapter.rs`,
`native_profile.rs`, and `tools/native_profiles/0.6.3.json`. A patch update
means hunting through all three.

### 6. Dead weight (medium)

- Root crate `lt-direct-control-core` (`src/lib.rs`, `src/pacing.rs`, `tests/`)
  describes itself as policy for "a future 0.6.2 adapter". The mod lists it as
  a dependency but never uses it.
- `movement-test.enabled` and `pacing-test.enabled` at the root are not read
  by any code.

### 7. Naming (medium)

- Crate and mod ID `lt_direct_control_probe`; `lib.rs` opens with "Diagnostic
  prototype"; `ClientProbe`, `AiProbe`.
- `movement_test.rs` is the real movement/order module; `timing_test.rs` is the
  shared test logger.
- Renaming the mod ID changes the install folder and is a breaking change for
  existing installs; internal names can change freely.

### 8. Layering inversions (medium)

- The input layer reads UI state: `platform_input.rs:232` reads
  `shop_ui::OPEN`, `:174` and `:260` read `settings::MODAL`.
- `Settings` is created inside `Cursor::new` (`cursor.rs`), and features reach
  it as `self.cursor.settings` (9 uses in `lib.rs`) or the `settings::GLOBAL`
  static.
- `purchase_tracker` and `player_hud` depend on each other; `cursor` depends on
  `movement_test` for click markers.

### 9. Threading rules are implicit (medium)

Three kinds of threads touch the code:

- **Client thread**: SDK client callbacks, the native viewer hook, cursor and
  wheel observers, all UI.
- **Viewed match's worker**: SDK `think` for the ten players, worker hook,
  movement/attack/skill/shop hooks for this match.
- **Background simulations**: other matches the game runs at the same time.
  They pass through every native hook (about 220,000 steering calls a second
  in 0.64.2), so hook code must reject them cheaply and never block on a
  shared lock. 0.65 added lock-free checks for this.

This is the most important "safe to touch" rule and it is written nowhere.

## Proposed clean-up, in stages

Each code stage: formatting, Clippy, all tests, anchors, then one play-test
match to confirm nothing changed. Pure moves are never mixed with logic edits.

0. **Snapshot (your decision, no code):** fix git ownership, commit 0.65.2,
   tag it. Optionally publish a remote.
1. **Repository tidy (no behaviour change):**
   - Rewrite README: what it is, install, build, project map.
   - Move `TESTING-*.md` to `docs/testing/` and pass notes to `docs/history/`;
     add `CHANGELOG.md` and take release notes out of the verifier.
   - Make tool paths configurable (game folder, font sources); keep the asset
     records the verifier needs in a tracked folder.
   - Remove the unused root crate and the two `.enabled` files; report real
     test counts only.
2. **Logging levels:** one setting (Quiet / Normal / Verbose) routing the three
   groups above; Verbose keeps today's output for investigations.
3. **Internal renames:** `movement_test` → `movement`, `timing_test` →
   `test_support`, `ClientProbe`/`AiProbe` → `Client`/`Simulation`, module docs
   updated. Mod ID unchanged.
4. **Split `native_adapter.rs`:** `hooks/` per family (movement, combat,
   outline, view/camera, pacing, shop) plus one `game_0_6_3` layout module
   holding every address and offset, merged with `native_profile.rs`.
5. **Split `lib.rs`:** one function or module per client feature (session,
   HUD, Tab, settings, shop, cursor, hover) and per simulation duty (binding,
   sampling, shop, input); fix the layering inversions with a small shared
   context instead of statics read across layers.
6. **Modder guide** (`docs/modding.md`): threads and their rules, how a frame
   flows, where to add a key binding, a setting, a HUD element or a shop rule,
   and how to update for a new game version.

## Decisions needed from the user

| Decision | Recommended default |
|---|---|
| Git: fix ownership and commit 0.65.2 now? Publish a remote (e.g. GitHub)? | Commit locally now; remote later |
| License | Your choice (e.g. MIT for maximum reuse, GPL to keep forks open) |
| Redistribution: may the bundled fonts (OFL), the Endfield-styled design assets and the vendored `sdk/mod-api-stable` be shared? Does any tracked file contain copied game artwork? | Confirm before any public release |
| Mod ID rename (`lt_direct_control_probe`) | Keep for now; rename only at a deliberate public release |
| Default log level | Normal (safety + test lines), Verbose on request |
| Remove the unused root crate | Yes |
