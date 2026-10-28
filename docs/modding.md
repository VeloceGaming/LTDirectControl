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
buyer). Those hooks install only on the fingerprinted game builds (0.6.4 and
0.6.5); on any other build the mod stays inactive.

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
   strip, session bar, settings, shop, screen effect, result audit; each
   drawn window also builds its draw list here) →
   `block_native_ui` (UI rectangles where clicks are not orders) → gameplay
   input → `update_pointer` (hover, outlines, cursor) →
   `log_frame_diagnostics`.
3. `post_render`: the drawing probe while its hidden switch is on, then
   the drawn windows (1.6.0-perf-dev3: each window's draw list, built in
   `post_update`: screen effect, HUD strip, state plate, stats panels,
   session strip, and since 1.6.1-dev1 the Tab panel; `draw_list::submit`),
   then battlefield drawing (skill
   previews, minimap frame, attack range, target markers, click markers)
   and the top-right readouts (input delay, and since 1.6.2-dev1 the live
   rating: `live_rating.rs`, read in `sample_hud`).

`Simulation::think` (in `simulation.rs`): `bind_session` →
`sample_prepared_roster` → `sample_team` → `trace_purchases` →
`publish_shop` → `sample_hud` → `observe_controlled` → unit lists for
targeting → `log_sample` → choose the controlled athlete's input (skill,
order or hold) and validate it with the SDK.

## Common changes

### Keep files small

Five files were already over 3,000 lines on 2026-10-23 (`movement.rs`,
`abilities.rs`, `player_hud.rs`, `shop_ui.rs`, `skill_preview.rs`), with
functions of 200 to 320 lines at the movement and input plug-in points.
The author declined a refactor on the condition that they stop growing
([backlog.md](backlog.md), Declined). So:

- A new feature goes in its own module with its own tests. The existing
  file takes only the few lines that wire it in.
- A long function you must change gives up the part you touch to a named
  helper. Nothing more: a refactor is its own stage with its own test
  sheet.
- `python tools/check_source_size.py` refuses a `.rs` file under
  `probe/src` over 3,000 lines, or over its own ceiling for the five above;
  `tools/verify_build.py` runs it. When a file is split, lower its ceiling
  in that script.

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

Key caps and tooltips that name a key show the binding as it is now
(1.6.0-dev4), never a typed-in letter, so a rebind shows at once:

- `Chord::short` (`settings.rs`) is the short name a cap shows (`S+B`,
  `F11`, `M4`, `Spc`, `PgUp`, `N5`); `Chord::label` stays the Settings
  window's full name (`Shift + B`, `Page Up`, `Num 5`). Both read one
  table, `key_names.rs`, with its tests; a key with no name there is
  `K<code>` / `Key <code>`. `Values::shown_chord` is the first chord bound
  to an action.
- The punctuation keys (codes 0xBA-0xC0, 0xDB-0xDF, 0xE2) are named by
  the character they type on the player's keyboard layout (1.6.0-dev5):
  `platform_input::layout_char` asks Windows (`MapVirtualKeyW`), and
  `key_names::punctuation` turns its answer into the name, with a US
  table as the fallback (`;` `=` `,` `-` `.` `/` `` ` `` `[` `\` `]`
  `'`). The call runs only where a name is made: a cap rebuilds its text
  only when its chord changes (the Settings window names its visible
  rows every frame while it is open). It answers 0 in tests,
  so the tests do not depend on the machine's layout: test
  `punctuation` with the answer you want instead. A layout switched while
  the game runs shows on a cap at its next rebind.
- A window keeps a `key_cap::KeyText` per cap in its view and calls
  `follow(&settings::current_shared(), "your_key")` every frame; the text
  is rebuilt only when the chord changes. `key_cap::width` gives the cap's
  width: today's width at the default key, wider by the text's extra
  width for a wider key. An unbound action has an empty text: draw no
  cap.
- A tooltip that names a key is a translation text ending in
  `" · {key}"` (`"Start · {key}"`), looked up with `lang::tr` at the call
  and passed to `key_cap::tip`, which fills in the short name or drops the
  " · {key}" part when the action is unbound. Every `probe/lang/*.json`
  needs the same text; `key_cap`'s tests check the six that exist. A
  text naming several keys, one per line, goes through
  `key_cap::tip_lines` with a name per key (the camera tooltip,
  `"Lock camera · {key}\nHold {follow} to follow"`): an unbound key's
  " · {name}" part goes at a line's end, and a line naming it elsewhere
  goes altogether.
- Caps today: the HUD's shop, recall and skill caps (`hud_draw::HudKeys`;
  since 1.6.0-dev5 the recall cap grows left so the recall icon stays
  clear, the shop cap grows left until the gold number's end and then
  right, a skill cap grows right up to its slot's 3 px inset), the
  session strip's Tab cap (growing left with its button) and "Take control" label (`session_draw::SessionKeys`), the
  state plate's start cap (the plate widens and stays centred).

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

Since 1.6.0-perf-dev3 there are two ways to put something on screen. Pick
the drawn one unless the element needs what only the game's UI tree can do.

| Need | Path |
|---|---|
| A box, bar, hairline, tick, a single-colour text in one line, a glyph from the mod's own PNGs | **Drawn**: a few lines in the window's draw list |
| An image cut from one of the game's sprite sheets (skill, item, stat icons), a champion portrait (`ui_set_champion_icon`), wrapped, scrolled or several-colour text (tooltips), a scaled or tint-animated image, the game's own widgets (text fields, sliders, the shop and Settings windows) | **A tree node**: a template and the write cache |
| One short line of several colours in the mod's own fonts (the Tab panel's K/D/A) | **Drawn**, one text per colour run, placed with the font's advance widths (`team_draw::KdaView`); the advances are the medium weight's, so a numeric-font K/D/A comes out up to about 3 px narrower than the numeric weight's own advances (measured offline, CHANGELOG 1.6.1-dev1) |

The design, the measured cost per draw command and the limits of the draw
calls are in [investigation-direct-draw-hud.md](investigation-direct-draw-hud.md).

#### A drawn element

Each drawn window has three parts:

1. **State** (the window's module, e.g. `player_hud.rs`): unchanged in
   kind from before. Everything with a side effect happens here, every
   frame: tweens (`hud_motion::Motion`), the motion pass, the ready chime,
   press and release, hover sounds, tooltip timers.
2. **View** (e.g. `hud_draw::HudView`): a plain struct with one field per
   drawn element, set by the state step where it used to write the node.
   A field not set this frame keeps its value, as a node kept its last
   write; the defaults are what the old template said.
3. **Draw list** (e.g. `hud_draw::build`): a pure function from the view
   to a `draw_list::DrawList` of boxes, texts and sprites, in UI-map
   pixels (1920 x 1080), at a z value. The client sends every window's
   list in `post_render` (`draw_list::submit`), which sends nothing while
   `perf_hide_mod_ui` is on.

So adding a readout to the strip is: a field in `HudView`, one line that
sets it in `player_hud.rs`, and in `hud_draw::build`:

```rust
if v.my_readout.shown {
    list.text(&v.my_readout.text, at(o, (600., 102., 60., 32.)), 1005,
              TextStyle::numeric(24., Align::Start), 0xffffffff);
}
```

Rules:

- **z sorts with the tree.** A drawn command lands between tree nodes by
  its z (seen in game with the 1.6.0-perf-dev2 probe): the HUD strip is z
  1000-1113, the session strip 1100-1201, the shop 1500 and up, Tab 1600
  and up, Settings 1999 and up. Commands of equal z keep their order in
  the list (a guess the probe did not measure); add them in back-to-front
  order.
- **Positions** come from the game's `ingame` root (`ctx.ui_node_rect("ingame")`)
  through `draw_list::anchored`, as a template root's anchor and pivot
  placed it. No scaling: the UI map is 1920 x 1080.
- **Text**: `list.text` or, without allocating, `list.text_fmt(format_args!(..))`
  in the mod's fonts (`Font::Numeric` was the `bold_label` style,
  `Font::Medium` was `label`). One colour per call; nothing wraps or
  clips. An outline is four 1 px offset copies (`outlined_text`).
- **Glyphs**: `draw_sprite` draws a PNG at its own size and cannot tint,
  so a glyph ships once per size and tint
  (`node tools/generate_endfield_glyphs.cjs --drawn`, listed in
  `tools/records/drawn-glyphs.json` and `drawn_glyphs::BAKED`; then add the
  files to `tools/records/ui-graphics.json` and the count in
  `tools/verify_build.py`). A glyph whose tint animates stays a tree node.
- **Clicks**: a draw takes no events. Hit-test the cursor yourself, as
  the strip's tiles, its shop button and the session strip's buttons do
  (`SessionUi::clicks`: the press starts on a shown control and the release
  comes over the same one, through `ui_click::once`), and return the
  rectangle so `block_native_ui` keeps clicks there off the battlefield.
- **Tests**: the window tests (`ui_identity.rs`) record each window's
  commands per frame through a fake draw table (`fake_draw.rs`) under
  `probe/testdata/ui_draw/`; a deliberate change records anew with
  `LT_UPDATE_UI_RECORDS=1`. `tools/compare_draw_review.cjs` renders the
  dev2 tree and the dev3 tree plus draw list for review states and counts
  the differing pixels.

#### A tree node

The native UI is built from template strings (see `shop_ui::template`),
spawned with `ctx.ui_spawn_source`, and updated through the window's write
cache (`ui_writes::PropCache`), which sends the game only what changed.
Clicks on a node are registered with `ctx.ui_register_path_events`; check
`crate::ui_click::once(path)` in the callback. The game keeps a path's
callbacks when your window is removed and rebuilt within a match
(background colour or language change), and your rebuild registers again,
so without it every click fires twice and toggles undo themselves. Put
shown text through `tr`/`trf`.

To add a window:

1. Give it a struct with an `apply(ctx, ...)` method, like `ShopUi` or
   `TeamUi`.
2. Store it in `ClientObservations` and call it from
   `Client::apply_windows`; if it draws, add its list to the
   `draw_list::submit` call in `Client::post_render_inner`.
3. Return its screen rectangle, so `block_native_ui` stops clicks there
   from becoming battlefield orders.
4. Pick a z-order band that does not collide: HUD below 1120, shop
   1500–1560, settings around 2000.
5. If its tree part is built the first time it is needed in a match,
   implement `prewarm::Prespawn` and add it to the list in
   `Client::apply_windows`, so it is built hidden while the player chooses
   an athlete.

#### A tree node's writes: a state, a comparison and a write (1.6.0)

A window runs every frame, and most frames nothing on it changes. So the
per-frame work is split in three:

1. **State**: from the frame's inputs, work out what each node shows as
   plain values: numbers, flags, colours as `u32`. Everything with a side
   effect happens here, every frame, whatever is drawn: tweens
   (`hud_motion::Motion`), the motion pass, the ready chime, press and
   release, hover sounds, tooltip timers.
2. **Comparison**: pass those values to the node's write site. The cache
   keeps, per node and site, the values it last had accepted. Unchanged
   values (and an unchanged background colour and language) mean nothing
   to do, not even formatting the text.
3. **Write**: only on a change, the site's function formats the property
   source and the cache applies the usual rules (fields compared after
   theming; a refused write is retried next frame).

In code: look a node up once (`cache.node(path)` gives a `NodeId`; keep
them in a node table such as `hud_nodes::Nodes`), then per frame:

```rust
// The values the source is made of, then how it is written.
cache.props_keyed(ctx, nodes.shade, SITE, (shown, height, color), |(shown, height, color)| {
    format!("visible: {shown}; height: {height:.2}px; color: #{color:08x};")
});
cache.text_keyed(ctx, nodes.kills, (kills,), |(kills,)| kills.to_string());
```

Rules that keep this exact:

- The format function may use only its values and constants of that
  node. Anything else it reads (a setting, a name) belongs in the values.
- Two different sources on one node need two site numbers (`VIS` = 0 is
  always `visible: {on};`). A write from one site makes the others check
  again, so the node always ends as 1.5.3's rules would leave it.
- Show and hide a root with `ui_root::show` / `ui_root::hide`, which
  write only when the game's state differs, and keep the
  `perf_hide_mod_ui` switch working.
- `ui_identity.rs` drives frame sequences through each window on a fake
  UI tree (`fake_ui.rs`) and compares every write with a saved record
  (`probe/testdata/ui_identity/`). A change that should not change the
  screen must keep those records; a deliberate change records anew
  (`LT_UPDATE_UI_RECORDS=1 cargo test --release ui_identity`) and the
  record's diff is reviewed like the code.

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

Three hidden switches (controls.json only, no control in the Settings
window, default off, read with `Settings::hidden_switch`, a `PERF` line
when they change) exist for performance measurements:
`perf_hide_mod_ui` keeps every mod window hidden (`ui_root.rs`; since 1.6.0-perf-dev3 it also stops `draw_list::submit`, so the drawn windows hide too),
`perf_draw_probe` draws a measurement load, layering boxes, image samples
and text samples directly in the UI map (`draw_probe.rs`, 1.6.0-perf-dev2;
how to read it in `CHANGELOG.md`). With the probe off its function returns
at its first check. And `perf_pacing_off` (1.6.1-dev2) is read each frame
into `NativeTiming::configure_pacing_off` and latched when the next match
is bound: that match's running viewer frames go to the game's update
untouched (`NativeTiming::untouched`, checked in `view_hook_body`: no
playback-field override, no frame-clock write, the game's own dt) and the
worker keeps a fixed lead of `PACING_OFF_LEAD` ticks; the start hold and
pause work as usual.

### Change what a skill preview shows

Previews come from the game's own skill data, never per-skill code:

- `preview_table.json` (compiled in; read by `preview_table.rs`) holds each
  recorded champion's skill shapes, with every size given as where it comes
  from: one of the skill's numbers by name (`attack_range`), the live cast
  range, or a fixed number. During a match the mod asks the controlled
  athlete's own skill objects for those numbers
  (`native_adapter/windows/skill_numbers.rs`), so previews follow the
  player's database edits and the save's balance patches. Do not edit the
  table by hand; see "Add a champion's preview" below.
- Research builds only (`cargo build --release --features research`):
  `native_adapter/windows/preview.rs` reads the controlled champion's live effect tree and
  turns each recognised effect type (identified by its apply function and
  size) into footprints: a shape (circle, corridor, rectangle, cone,
  movement) and a placement (caster, aim, ahead, projectile end, landing).
  The known types and their field layouts are listed in
  [investigation-preview-selection.md](investigation-preview-selection.md).
  To add a type, find it in the PREVIEW TREE log lines, match its words to
  a champion JSON that uses it, then add a decoder and a test with the
  logged words.
- `skill_preview.rs` places the footprints (`placed`) and draws them as the
  design's pieces (`drawing`: area, corridor (a strip with chevrons and an end cap),
  cone, wall, dash, blink, unit target, range ring). Data-driven and
  Workshop champions' JSON trees are the fallback.
- `preview_style.json` holds every colour, width, opacity and size of the
  look (`preview_style.rs` loads it). Players can override any subset of
  its keys in `%LOCALAPPDATA%\LTDirectControl\preview_style.json`; it is
  read at start-up. A new kind of detail (a glow, a pulse) is a code
  addition to the pieces; after that it is a style value.

### Add a champion's preview

The shipped table is learned from play, then reviewed in a viewer:

1. Build and install a research build (`cargo build --release
   --features research`; the tools name it `<version>-research`, and it
   is never for the Workshop). Play matches with the champion, or leave
   the game running: background matches count. The mod records the
   champion's shapes as the live reader decodes them and each skill's
   numbers by name with the shapes read at the same moment, into
   `%LOCALAPPDATA%\LTDirectControl\research\skill-shapes.json`.
2. Open the viewer (`tools/preview_viewer.cmd`). Compare each drawing with
   the game; assign a preview type to skills the reader cannot read; under
   "Sizes in the mod", pick the number for sizes where two fit. Assignments
   go to `skill-overrides.json`, picks to `skill-sources.json`, both in the
   same folder.
3. Run `python tools/build_preview_table.py`: it writes
   `probe/src/preview_table.json` and lists what still needs a pick or has
   no numbers yet. Build the public mod as usual. The viewer's Save runs it
   for you and shows its one-line result, and `tools/verify_build.py` fails
   while the table is stale. It never drops a champion: one no recording
   covers keeps its shipped entry (only `--drop <champion>` removes one),
   and a pack's champion with a data file needs only the pack enabled to
   edit, no match.

A Workshop champion that is not in the table is drawn from its champion
file, with the file's numbers (they do not follow patches).

### Change hover and click selection

- **Ground ring** uses `Movement::draw_rings` and the shared debug drawing
  helper `movement::draw_marker`, restored to the 1.3.16 drawing path in
  1.3.19. **Read from the code:** hover uses the team colour, 2 px width and
  2 px extra padding; the attack target uses red, 3 px width and 4 px padding.
  Its existing 120 ms click pulse adds up to 6 px padding and 1.5 px width.
  Ordinary units use the original `combat::highlight_radii` ellipse; towers
  use corner brackets around their selection envelope. The helper rejects
  line endpoints outside the viewport or covered by the HUD/minimap.
  Off / Outline / Ground ring and the automatic compatibility fallback are
  unchanged. No dedicated ring renderer remains. **Rejected, 2026-10-18:**
  the 1.3.17 segmented and 1.3.18 crescent redesigns; the author's 1.3.18
  game test found champion rings too wide and the behind-champion fading
  unsuccessful. Their design history remains in CHANGELOG.md.

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
(Safety, Normal, Verbose); a new tag counts as Normal, so decide its level
when you add it. Normal is for what a tester reads after a match: session
and settings events, refusals, and lines written once per start or per
match. Add a tag to the `VERBOSE` list if it fires on every click, press,
purchase or frame, every few seconds, or for every champion of a match, or if
it only serves an investigation (research builds' recording traces
included). Lines containing "failed" or "panic" are always written. Use
Settings › Interface › Debug › Log detail = Verbose when investigating; a
test in `logging.rs` pins the level of the busiest tags.

## Native hook code

`native_adapter/`:

| File | Holds |
|---|---|
| `mod.rs` | the safe functions the rest of the mod calls; picks the hooks built for the running game version |
| `v064/`, `v065/`, `v070b2/` | one folder per supported game version (0.6.4, 0.6.5, 0.7.0 Beta 2), holding everything that differs between versions: `layout.rs` (every code address, call site and expected byte pattern), `identity.rs` (executable fingerprint, layout guards, enabled hook families), `effects.rs` (skill effects the preview reader recognises), `items.rs` (item reader anchors) |
| `windows/mod.rs` | fingerprint check, patch installation (`install`), runtime patch audit |
| `windows/preview.rs` | skill effect tree reader (skill previews) |
| `windows/movement.rs` | move, steering, stop, recall |
| `windows/combat.rs` | attacks, Q/W/R, aim, ability reads |
| `windows/outline.rs` | sprite outlines, each unit's drawn sprites for selection |
| `windows/view.rs` | viewer/worker hooks: pacing, camera, full-screen layout |
| `windows/input.rs` | blocking game hotkeys during control |
| `windows/shop.rs` | buyer hooks, item/gold/build reads |
| `windows/stats.rs` | athlete stat rules for manually played matches: instruction-level sites (a relay replays the displaced bytes, a bridge saves the registers, `adjust` changes one register or frame slot, or a gate's three band registers); sites in each version's `stat_sites.rs`, rules in `athlete_rules.rs` |
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

The full checklist, in the order that worked for 0.7.0, is
[version-migration.md](version-migration.md); `tools/draft_version.py`
drafts the new version's tables. In short:

0. Run `python tools/prepare_sdk.py` to copy the new game's SDK into `sdk/`
   (it re-adds the mod's two local SDK additions and stops if the SDK
   changed where they go).
1. Start from the two places that hold version knowledge. The hook code in
   `native_adapter/windows/` is built once per supported game version,
   each time against that version's tables, and names no address itself:
   - `native_adapter/v064/` and `v065/` (copy the newest folder: hook sites and byte
     patterns, executable identity and layout guards, effect and item
     addresses), with a block for the new folder at the end of
     `native_adapter/mod.rs`;
   - `tools/native_profiles/<version>.json` (the reviewed profile the tools
     verify that folder against). `tools/verify_build.py` checks every
     version's folder against its own executable: the installed game for
     its version, a kept copy under `research/game-builds/<version>/` for
     the others.
2. Entity and player field offsets are still written inline in the hook
   files (search for `+ 0x`). An offset that differs between versions
   becomes a constant in every `layout.rs` (the viewer's
   `VIEW_FRAME_CLOCK` and `VIEW_PAN_VELOCITY` since 0.6.4; the player
   record's `PLAYER_ITEMS_*` and `PLAYER_PLAN_*` since 0.6.5).
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
