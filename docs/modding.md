# Modding guide

How the mod is put together and where to change things. Build and install
steps are in the [README](../README.md). Everything below lives in
`probe/src` unless a path says otherwise.

## The big picture

The game's mod SDK calls the mod through these entry points:

| Entry point | File | Runs on | Does |
|---|---|---|---|
| `Client` | `client.rs` | the client thread, every frame | input, session controls, HUD, Tab, settings, shop, cursor, battlefield drawing |
| `Simulation` | `simulation.rs` | simulation workers, once per player per tick | binds the session, samples HUD/Tab/shop data, returns the controlled athlete's input |
| `CooldownHook` | `test_cheats.rs` | simulation workers, once per match per tick | the testing aid only (Home+End: no cooldowns); the only code that changes a match |

`Simulation` reads the match but cannot change it: the SDK ignores changes
made from an AI callback. Anything that must change a match (a buff, a
teleport) belongs in a match hook (`StableMatchHook::on_match_tick`), which
runs for every match, so it must first check that the match is the viewed
one (see `CooldownHook`).

On top of the SDK, `native_adapter/` hooks a few places inside the game
executable (movement, combat, outlines, camera/pacing, hotkeys, the item
buyer). Those hooks install only on the fingerprinted game build (0.6.3); on
any other build the mod stays inactive.

`native_timing.rs` holds the session state machine shared by both sides:
Loading → Ready (choose your athlete) → Running ⇄ Paused → Released (F12 or
Return to AI). It also keeps the simulation at most one frame ahead of the
screen, which keeps input latency low.

### Threads: the rules

There are three kinds of threads:

1. **Client thread**: SDK client callbacks, the native viewer hook, all UI,
   the cursor and mouse-wheel observers.
2. **The viewed match's worker**: `Simulation::think` for its ten players
   and the native hooks for that match.
3. **Background simulations**: other matches the game runs at the same
   time. They also call `think` and **every native hook**, hundreds of
   thousands of times a second.

Rules that follow from this:

- In hook code, turn away anything that is not the controlled athlete
  **before** taking any lock. See `Abilities::selected_key` and
  `Shop::answer`, which check an atomic copy first.
- `think` must never wait. Only the worker hook waits, deliberately, for the
  viewer (frame pacing).
- Pointers the game passes into a hook are valid only during that call.
  Never keep them.
- Every hook body runs inside `catch_unwind`. A panic releases control
  (`NativeTiming::cancel`) instead of crashing the game.
- Shared state between threads is either an `Arc<Mutex<..>>` owned by one
  module (`movement`, `abilities`, `player_hud`, `team_status`, `shop`) or an
  atomic flag (`ui_state`). Keep critical sections short and never call the
  game while holding a lock.

## One frame, step by step

`Client` (in `client.rs`):

1. `pre_update`: on the battlefield, read input early and turn it into
   orders and casts, so a click reaches the simulation this frame.
2. `post_update`, in this order:
   `install_once` (title screen only) → scene flags → movement input →
   `choose_prepared_athlete` (before Start) → heartbeat → `rearm_if_needed`
   (new match or save exit) → session buttons → `apply_windows` (Tab, HUD
   strip, session bar, settings, shop, screen effect, result audit) →
   `block_native_ui` (UI rectangles where clicks are not orders) → gameplay
   input → `update_pointer` (hover, outlines, cursor) →
   `log_frame_diagnostics`.
3. `post_render`: battlefield drawing (skill previews, minimap frame, attack
   range, target markers, click markers).

`Simulation::think` (in `simulation.rs`): `bind_session` →
`sample_prepared_roster` → `sample_team` → `trace_purchases` →
`publish_shop` → `sample_hud` → `observe_controlled` → unit lists for
targeting → `log_sample` → choose the controlled athlete's input (skill,
order or hold) and validate it with the SDK.

## Common changes

### Add or change a key binding

1. Add a `bind!(...)` entry to `BINDINGS` in `settings.rs`: a key name, a
   settings group, a label, and the default chord. The chord is a Windows
   virtual-key code plus modifiers (1 = Shift, 2 = Ctrl, 4 = Alt). Mouse
   buttons are codes 1–6.
2. Read it in `platform_input::mapped` with `pressed("your_key")` into a
   field of `Keys`.
3. Use that field where the action belongs (`abilities.rs` for casting,
   `movement.rs` for orders, a `Client` step for windows).

Home+End (the no-cooldown test toggle) is deliberately not a binding: it
is read directly in `platform_input` and cannot be rebound.

The Settings window lists new bindings automatically and saves them in
`controls.json`. Its layout tests in `settings_ui.rs` count rows, so they
may need a new number.

### Add a setting

Add an `OptionDef` to `OPTIONS` in `settings.rs`:

- `page`: 0 Combat & casting, 2 Camera, 3 Interface, 4 Advanced General, 5 Advanced Debug; 9 hides it from the
  window. Advanced Acquisition uses a dedicated page 6 body.
- `control`: `Toggle`, `Choice(&[..])` or `Slider(min, max, step, unit)`.
- `default`.

Read it anywhere with `crate::settings::option("key")`, or with
`self.settings.number("key")` inside `Client`. The settings window draws
the row and saves the value; no other code is needed. Its label, hint,
section and choices are drawn through the translation table, so add their
wording to every `probe/lang/*.json` (see "Text and languages"; the tests
list what is missing).

Advanced uses General / Acquisition / Debug subpages. Per-champion acquisition
settings are the fixed 475-unit Acquisition body in
`settings_ui/acquisition_panel.rs`; formulas and copied range metadata live in
`acquisition.rs`. Extend those modules for related controls. They edit the same
settings draft and use the same Apply/Cancel/pause lifecycle. Full snapshots
include the champion table; input snapshots deliberately exclude it.
The portrait grid scrolls by complete rows and reuses a fixed slot pool. Keep its
last-rendered slot mapping separate from the current filtered list. Search uses
`acquisition_names.json` for base translations and reads enabled mods' `.i18n`
files at initialization; native `i18n` supplies the current display name.
Automatic defaults are a shared 120-unit minimum and +5 buffer, editable in-game.
Version 3 removes generated per-champion minima so shared edits affect every
Automatic champion; custom overrides and version 2 shared edits survive. Legacy
generated 120/+10 defaults still migrate once. The Debug overlay uses the exact
selected-player targeting sample (position, AA, acquisition), retained while
paused and expired after 250 ms while running. Never calculate it from browsed
champion metadata or draft settings. Native text edits retain
input and IME handling; high-layer labels mirror committed text because the
native editor's own text/selection/caret draw at fixed z=100. The focus underline
is visible; native selection/caret and uncommitted IME text are not mirrored.

### Text and languages

Every text the mod shows goes through `crate::lang`:

- `tr("Cancel")` returns the current language's wording, or the English
  when a translation is missing. The English text itself is the key.
- `trf("Need {gold} · {count} steps", &[("gold", &g), ("count", &n)])`
  fills named values. Never glue words around a number with `format!`:
  word order and plurals differ per language. Write separate texts for one
  and several (`"1 step"` / `"{count} steps"`); each language may phrase
  both its own way (Russian and Polish use "шагов: {count}" style).
- Translations live in `probe/lang/<code>.json` (English text → wording),
  one file per game language; English needs no file. They are built into
  the DLL.
- `lang::tests` fail when any `tr`/`trf` text (or a settings label, hint,
  choice, page name, stat filter or emote slot) is missing from a language,
  is empty, or has different `{values}`, and when a file holds a stale key.
  `cargo test --release lang::tests::list_missing -- --ignored` writes the
  missing texts to `target/lang-missing.json`.
- Settings > Interface > Mod language: Auto (default) follows the game,
  identified from its own word for "Close" (`lang::GAME_CLOSE`) through the
  SDK text lookup and rechecked every second; or a fixed language. The
  stored value is an index into `lang::CHOICES`: never reorder `LANGS`.
- A language change rebuilds every window (`ui_theme::refresh` keys on
  background colour and language), so template text is simply rebuilt.
- Kept in English on purpose: log lines (their leading tag sets the log
  level), key names, emote names, setting keys, node names and asset paths.
- Game words (items, champions, skills) already come from the game in its
  language; do not translate them.

Fonts: the game picks a font set entry by ITS language, while the mod's
text can be in another. `tools/font_fallbacks.py` makes every entry of
`probe/font/*.font_set` fall back to all the game's script fonts; the HUD
font generator applies the same rule.

### Add a HUD element or a window

The native UI is built from template strings (see `player_hud::template`
and `shop_ui::template`), spawned with `ctx.ui_spawn_source`, and updated
through small cached helpers (`props`, `text`, `visible`) that skip writes
when nothing changed. Clicks are registered with
`ctx.ui_register_path_events`; check `crate::ui_click::once(path)` in the
callback. The game keeps a path's callbacks when your window is removed and
rebuilt within a match (background colour or language change), and your
rebuild registers again, so without it every click fires twice and toggles
undo themselves. Put shown text through `tr`/`trf`.

To add one:

1. Give it a struct with an `apply(ctx, ...)` method, like `ShopUi` or
   `TeamUi`.
2. Store it in `ClientObservations` and call it from
   `Client::apply_windows`.
3. Return its screen rectangle, so `block_native_ui` stops clicks there
   from becoming battlefield orders.
4. Pick a z-order band that does not collide: HUD below 1120, shop
   1500–1560, settings around 2000.

### Change how the shop buys

All purchase logic is pure functions in `shop.rs`:

- `assign`: which slot each queued order uses;
- `next_step`: the next purchase;
- `offer`: what one more Buy would mean;
- `project`: the preview of upcoming purchases.

They have unit tests. The native buyer only asks two questions,
`upgrade_answer` and `new_item_answer`, and the game itself still validates
and performs every purchase. With Manual shopping on, missing data fails
closed and nothing is bought.

### Colours

The UI background (`#1c1a18` in the design) is a player setting. Build any
new surface with `crate::ui_theme::hex(alpha)` (templates) or
`ui_theme::rgba(alpha)` (drawn colours) instead of the literal, and call
`ui_theme::refresh(ctx, PATH)` for a new window (see `Client::post_update`)
so it rebuilds when the colour changes. Dark text on bright buttons keeps
its fixed ink.

The rest of the design's warm greys (rows, frames, hovers, wells) move with
the background by the same offset. In templates and property strings write
them as `#~rrggbbaa` (e.g. `#~3a3837ff`); `ui_theme::themed` rewrites them
at spawn and in `hud_motion::properties`. Drawn or blended colours use
`ui_theme::tone(0x3a3837ff)`, and a colour held as text is read with
`ui_theme::color("~3a3837ff")`. Keep a plain `#` for text inks so they stay
readable on any background.

### Draw on the battlefield

Draw in `Client::post_render`. `camera::CameraFrame` projects world
positions to the screen. `skill_preview::Drawing` collects lines and shapes
and clips them against the HUD and minimap. What the game's drawing calls
can and cannot do is in [preview-drawing.md](preview-drawing.md).

### Change what a skill preview shows

Previews come from the game's own skill data, never per-skill code:

- `native_preview.rs` reads the controlled champion's live effect tree and
  turns each recognised effect type (identified by its apply function and
  size) into footprints: a shape (circle, corridor, rectangle, cone,
  movement) and a placement (caster, aim, ahead, projectile end, landing).
  The known types and their field layouts are listed in
  [investigation-preview-selection.md](investigation-preview-selection.md).
  To add a type, find it in the PREVIEW TREE log lines, match its words to
  a champion JSON that uses it, then add a decoder and a test with the
  logged words.
- `skill_preview.rs` places the footprints (`placed`) and draws them as the
  design's pieces (`drawing`: area, corridor with the spearhead arrow,
  cone, wall, dash, blink, unit target, range ring). Data-driven and
  Workshop champions' JSON trees are the fallback.
- `preview_style.json` holds every colour, width, opacity and size of the
  look (`preview_style.rs` loads it). Players can override any subset of
  its keys in `%LOCALAPPDATA%\LTDirectControl\preview_style.json`; it is
  read at start-up. A new kind of detail (a glow, a pulse) is a code
  addition to the pieces; after that it is a style value.

### Change hover and click selection

- `sprite_art.rs` measures each unit kind's body from the installed art at
  start-up (background thread) and places it with the game's own draw data,
  recorded per unit by the outline hook (`native_adapter::sprite_draw`).
- `sprite_picking.rs` maps a native unit name to its art (`sheet_name`,
  `art_key`) and keeps the older size-based fallback. A unit still on the
  fallback is logged once as `SPRITE ART unmatched name=…`.
- `combat::picked` ranks the units under the cursor (enemies first,
  structures last, body before edge, the previous hover kept, then the most
  central body). Settings › Show selection markers draws each area: green
  is measured, grey is the fallback.

### Logging

Write `logger.write("TAG details")`. `logging.rs` assigns each tag a level
(Safety, Normal, Verbose); a new tag counts as Normal. Add it to the
`VERBOSE` list if it fires every click or frame. Use Settings › Interface ›
Debug › Log detail = Verbose when investigating.

## Native hook code

`native_adapter/`:

| File | Holds |
|---|---|
| `mod.rs` | the safe functions the rest of the mod calls |
| `windows/mod.rs` | fingerprint check, patch installation (`install`), runtime patch audit |
| `windows/layout.rs` | every 0.6.3 code address, call site and expected byte pattern |
| `windows/movement.rs` | move, steering, stop, recall |
| `windows/combat.rs` | attacks, Q/W/R, aim, ability reads |
| `windows/outline.rs` | sprite outlines, death greyscale, each unit's drawn sprites for selection |
| `windows/view.rs` | viewer/worker hooks: pacing, camera, full-screen layout |
| `windows/input.rs` | blocking game hotkeys during control |
| `windows/shop.rs` | buyer hooks, item/gold/build reads |
| `windows/tooltips.rs` | native ChampionInfo descriptions and allocation ownership; called from the existing viewer hook |

`native_tooltips.rs` outside the adapter holds only owned requests/results.
Since 0.69.1 the formatter accepts all valid champion IDs; the Windows adapter
still permits only reviewed ChampionInfo implementations. `tooltips.rs` keeps the ordinary resolver as fallback, and
`player_hud.rs` refreshes eligible hovered skills when a native result arrives.
Keep game calls outside the cache lock and native pointers inside the viewer
borrow. See [the formatter investigation](investigation-tooltips-native.md)
before changing the register bridge or ownership code.

`tools/audit_tooltips.py` reads the user's captured log results against the
base inventory in `tools/records/tooltip-skills.json`. Use `--refresh-inventory`
to rebuild that inventory from installed bundle metadata. Missing observations
must never be counted as passing tests; Workshop entries are reported separately.

Each hook replaces a `call` (or a jump thunk) at a reviewed site. It is
installed only if the site and the target's opening bytes match
`layout.rs`. For the core hooks, `install` stops at the first mismatch and
direct control stays off. The shop hooks are optional: if they fail, only
Manual shopping reports itself unavailable.

## Updating for a new game version

0. Run `python tools/prepare_sdk.py` to copy the new game's SDK into `sdk/`
   (it re-adds the mod's two local SDK additions and stops if the SDK
   changed where they go).
1. Start from the three places that hold version knowledge:
   - `native_adapter/windows/layout.rs` (hook sites and byte patterns);
   - `native_profile.rs` (executable identity and layout guards);
   - `tools/native_profiles/0.6.3.json` (the reviewed profile the tools
     verify against).
2. Entity and player field offsets are still written inline in the hook
   files (search for `+ 0x`).
3. Follow [re-mod-migration-guide.md](re-mod-migration-guide.md) and
   [patch-migration.md](patch-migration.md). `tools/patch_migration.py`
   proposes candidates; `tools/verify_native_profile.py` checks a profile
   against the executable and against these sources.
4. Features that use only the SDK (HUD, Tab, settings and session windows,
   the shop window) need no native changes.

## Testing

- `cargo test --release` in `probe/` covers the pure logic: orders,
  targeting, casting rules, shop planning, layouts, log levels, and that
  every text is translated and every window template stays whole in all
  17 languages.
- Rendering and real gameplay can only be checked in game. Play a match,
  then read `%LOCALAPPDATA%\LTDirectControl\probe.log`.
- `tools/verify_build.py` checks a staged build against the installed game
  before packaging.
