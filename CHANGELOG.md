# Changelog

Every build of LT Direct Control, newest first. Entries are the technical
notes recorded when each build was verified; dates are build dates (UTC+8).

## 0.74.4 — 2026-10-09

- Tidy-up, no behaviour change: the 0.70 SPRITE CALIBRATION / SPRITE SAMPLE diagnostics are removed (the selection work they served is done). PREVIEW TREE / PREVIEW ROSTER stay until the previews are finished.
- docs/modding.md covers the match hook (`CooldownHook`), skill-preview decoding and art-measured selection; docs/backlog.md lists what is left.

## 0.74.3 — 2026-10-09

- Skill previews: a timed dash with no effects on units it hits (Candygel R's slide, Nullifier Q) is drawn as cyan movement of its exact length instead of a body-wide corridor; dashes that hit (Harpy R) keep the corridor.
- Effects scheduled at or after the end of a timed dash in the same skill happen where the dash ends: caster-centred areas and drops at the caster's feet move to the dash end. Candygel R now shows a 42-unit pool at both ends of the slide (dropped at tick 10 and at tick 54 = 10 + 44). Dashes that stop at the first unit (Rush) have no fixed end and are not used.
- No-cooldown test option: the game's formula is haste-style (0.74.2 test: +99 halved cooldowns), so the buff now uses +900, leaving a tenth of each cooldown.

## 0.74.2 — 2026-10-09

- No-cooldown test option fixed: 0.74.1's -100 cooldown multiplier gave Bomber Q a 300 s cooldown and froze the cast mid-animation. The game's own items reduce cooldowns with positive values (Staff of Rapture and Angel's Fang +10, Prophet of the Abyss +15), so the buff now uses +99 for skill and ultimate, which is safe under either plausible formula (about 1% of the cooldown if linear, about half if haste-style).
- While the option is on, each cast's remaining cooldown is logged (`TEST no cooldowns cast slot=… remaining_ticks=…`), read from the game, to settle the formula.

## 0.74.1 — 2026-10-09

- Skill previews: a projectile with no effects on units hit in flight (Bomber Q and W, Poison Dart Hunter Q and W, Bard W) draws no corridor, and its end effects land where the cast is aimed (new placement Landing: the target for unit casts, the aimed point within the flight length for location casts, the full length for direction casts). Bomber Q is now a 20-unit and W a 40-unit circle at the cursor, matching `explosion_range` / `attack_range` in the champion data.
- Bomber R stays one 25-unit circle (`explosion_range` 25000): in game, the edge of the game's own landing marker (about 50 units, drawn at twice the damage size) takes no damage while the halfway point does.
- Testing aid: Home+End toggles "no cooldowns" for your own champion in the viewed match (new test_cheats.rs). Off at every start and never saved; while on, a match hook keeps one permanent `lt_test_no_cooldowns` buff (skill and ultimate cooldown -100%) on the champion, re-applied after respawn, and removes it when turned off. Background matches are never touched. A red "TEST: no cooldowns" tag is drawn and each toggle is logged. It changes the real match.

## 0.74.0 — 2026-10-09

- Rejected basic attacks now hold position when their target is already within the current native AA range, retaining the target until the attack becomes ready. Out-of-range targets are still approached; range uses the current effect's base, level growth and bonus rather than sprite selection bounds. Native cooldown, windup and damage remain authoritative.
- F12 hands control to AI without releasing the match coordinator. F11 or the Take control button reclaims the same champion. AI remains paced at 1x with one-frame lead; handoffs clear pending movement/casts and camera gestures, retain the full-screen layout and preserve manual camera/vision choices. Startup cancellation and adapter failures still release permanently for that match. AI spectator pause is respected; reclaim uses the manual pause coordinator.
- Shop: Everything is now All; the header uses the existing shop bag icon. Purchase controls show only Purchase, Queue or Queue another plus the remaining price, keeping long item names out of the button.

## 0.73.0 — 2026-10-09

- Skill previews decode five more native effect types, named by pairing the logged effect trees with Workshop and data-driven champion JSON (field values checked against those files): RangeProjectile (an area at its placement after a delay), ParabolicProjectile (the landing area at the aim, plus its landing effects), RushTime (a timed dash, speed x ticks long, body wide), SwitchByBuff (the branch without the buff is previewed; the caster's buffs are not read) and MoveToTarget (movement onto the target). No new drawing: each maps onto an existing preview piece.
- Replaying the 57 logged skill trees: 33 now have a footprint, up from 23 (Alchemist Q, Bomber R, Pyromancer Q and R, Harpy R, Nullifier Q, Candygel W, Squirrel W, Spirit Latcher R, Cavalry Knight Q; Bomber Q and W gain their explosion areas).
- New docs/preview-drawing.md: what the game's drawing calls can and cannot do, for designing previews; docs/investigation-preview-selection.md lists the named effect types.

## 0.72.3 — 2026-10-09

- Monster names fixed: the game names them `<sheet>_monster` (bee_monster, mushroom_monster, rhino_monster, stump_monster, as logged by 0.72.2), not `_jungle`; the wrong 0.72.2 mapping is removed.
- Mod champions shipped only as `.aseprite` files (Leef's Variety: Harpy, Nullifier, Candygel and the rest) get measured bodies: the idle frames' pixels are read from the Aseprite file (layer and cel opacity applied; `miniz_oxide` 0.8.9, already built as part of `png`, is now a direct dependency) and the body is placed around the drawn centre with the drawn facing, since the game packs these sheets itself and their atlas positions are unknown. One body per champion (no form switching for these).

## 0.72.2 — 2026-10-09

- Monsters get their measured bodies: native names are `<sheet>_jungle` (as in the game settings), so `bee_jungle` now maps to the `bee` sheet, `tree_jungle` to `stump`, and so on, for both the measured body and the old fallback size table (one shared `sheet_name`). Before this, every monster used the large radius-based fallback box.
- `SPRITE ART unmatched name=…` is logged once per name for any unit still on the fallback after the art has loaded.
- 0.72.1 Verbose log: hover picking costs about 25 µs per frame on average, 104 µs at most.

## 0.72.1 — 2026-10-09

- Minions get measured bodies: their shared sheet (UI_aseprite/minion) is loaded as `ui/minion`, and the drawn frame picks the kind. One sheet can now hold several kinds: animations named `<kind>_<tag>` form a kind when a `<kind>_idle`/`_run`/`_walk` exists, and each frame uses its kind's standing body. Minions get 8 bodies (melee/ranged, each side, Morgard); champion forms get their own too (Druid bear/eagle, Demon Archfiend, Berserker, Ghoul berserk, Bombardier deployed, Dokkaebi, Cavalry Knight fire, Gunner forward/backward run).
- Click padding on measured bodies: champions 3 units wider on each side, every body 2 units below the feet.

## 0.72.0 — 2026-10-09

- Selection bodies measured from the installed art on a background thread at start-up (new sprite_art.rs, `png` 0.17.16): the opaque pixels of each unit type's standing frames, with sideways protrusions trimmed (columns under a quarter of the tallest: guns, spears), keeping heads, hats and raised staffs. Base sheets come from the game bundle, mod champions from their own `#anim.fanim` / `#sheet.png`; towers and the nexus add their orb sheet. Nothing is copied.
- Placement from the game's own draw data: the outline hook records each unit's drawn centre and body sprites; the drawn frame is found by its atlas position, giving the true anchor and facing (mirrored when flipped). Units without loaded art or a recent draw keep the previous envelope.
- Hover and click ranking follows the agreed order: enemies before allies, structures last, body hit before edge hit, the previously hovered unit within its tier (stickiness), then the body the cursor is most central on relative to its size, then the unit drawn in front, then a stable id. The blanket champion-first rule is gone; the champion-only key covers fights. Skill target brackets use the same area.
- Show selection markers now draws each unit's selection area: green for a measured body, grey for the fallback. The 0.70 magenta/cyan overlay is removed; the 0.70 inventory logging remains.

## 0.71.0 — 2026-10-09

- Skill previews decode four more native effect types, identified from the 0.70 effect-tree inventory with layouts checked against every logged instance: Delayed (its effects keep their placement), lingering area projectiles (circle/rectangle at their placement), Rush dashes (a corridor from the caster to the cast range, its hit width from the effect) and MoveTo movement (a line to the aimed point).
- Linear projectiles with hit radius 0 (they stop at the first unit) are drawn as thin lines instead of being rejected, and their end effects are now read and drawn where the projectile ends: on the target for unit-targeted skills, otherwise at its full flight length (new placement End).
- Replaying the 60 named skill trees of the two 0.70.1 test matches: 22 now have a footprint, up from 9, including Illusionist W (60-unit fear circle around the caster), Poison Dart Hunter Q (dart and 48-unit poison splash), Ghost Q (movement) and Ghost W (dash).

## 0.70.1 — 2026-10-09

- Diagnostics, no behaviour change. PREVIEW TREE: the viewed match's champions get their own budget (240 lines; each champion's Q/W/R once per match) so background matches (200 lines, each tree shape twice) cannot crowd them out; PREVIEW ROSTER is written once per match instead of once per player.
- Show selection markers also draws each unit's body sprite placement from the game's draw commands: magenta = full-size sprite centred on the unit, cyan = half-size from its corner to the unit, beside the existing hover markers, to settle which matches the art.

## 0.70.0 — 2026-10-09

- Diagnostic build for the skill-preview and selection work (no behaviour change). PREVIEW TREE: the Q/W/R effect trees of champions in the viewed match and in background matches, with each node's apply function, size and raw payload words, children followed only beside game-owned effect tables and into memory confirmed readable; each entity once per thread, each tree shape at most three times, at most 300 lines. PREVIEW ROSTER: actor id to champion for the viewed match.
- SPRITE CALIBRATION / SPRITE SAMPLE: once a minute on the battlefield (at most 10 times) the camera frame and each unit's simulation position and projected screen point, and the next 40 unit draws from the outline hook (view id, drawn position, raw sprite command words).

## 0.69.2 — 2026-10-09

- HUD tooltips grow upward to the available battlefield height, then widen from 529 to 640 or 760 pixels when needed. Descriptions exceeding the largest card scroll in whole lines, with a slim scrollbar; wheel input works over the card or its originating HUD tile without zooming the camera. Moving into the card keeps it open. Font size and weight are unchanged.
- Explicit native line spacing now matches measured body/title heights. Shared text measurement counts inline stat icons, preserves their source spacing, and wraps adjacent icons; scrolling retains color runs, icons and the final description line. Layout is cached until content or available space changes, including when a native description replaces fallback text.
- Accepted clicks on unit/structure bodies no longer create the ground cursor animation. The decision uses the click-time picker, including allies, and clears any earlier ground animation. Direct attack orders retain their sprite outline pulse; ground move/attack-move and minimap destination feedback remain available.
- User confirmed 0.69.1 Ghost and Harpy (mod champion) tooltips worked. Regression checks cover card growth/widening, long multilingual/icon descriptions, scroll clamping/reset/color continuity, target click suppression, retained attack orders/pulses, and ground/minimap marker expiry. Native layout and click feedback await user testing.

## 0.69.1 — 2026-10-09

- Enable native skill descriptions for all champions, retaining the ordinary resolver when native text is unavailable. Expand the checked ChampionInfo implementations from six to 65 by tracing the built-in lookup's return branches; 407 tooltip code/table guards now cover built-in, data-defined and registered mod wrappers. No new hooks, and the tested register/ownership bridge is unchanged.
- Workshop descriptions can be requested even when the SDK cannot provide a localized template. Open skill tooltips refresh with cached native results; the bounded session/language cache accommodates the complete base skill inventory without clearing pending results after 24 entries.
- Add structured per-skill coverage diagnostics and tools/audit_tooltips.py. Unresolved parameter names are counted separately from ellipsis punctuation; failed lookups and unobserved skills remain explicit. The refreshed installed bundle contains 204 English Q/W/R description entries across 68 champions. Existing 0.69.0 logs confirm six native results with zero unresolved placeholders; the remaining 198 entries await runtime observation.
- Regression checks cover all base champion IDs, Workshop text without SDK translation, missing optional skills, cache deduplication, unresolved-parameter parsing, and structured/legacy coverage reports. User confirmed the 0.69.0 Illusionist/Alchemist prototype worked perfectly; broader native rendering/gameplay remains pending user testing.

## 0.69.0 — 2026-10-09

- Guarded native skill-description prototype for Illusionist and Alchemist only. Hovering Q/W/R requests the same localized, parameter-filled description interface used by the game's champion-info screen. Other champions and unavailable native results retain the existing resolver; tooltip typography, metadata and layout stay unchanged.
- Native access runs after the existing viewer update, on the bound client thread with its borrowed Assets. The register-pair lookup bridge, ChampionInfo Arc release and native String allocation are isolated in the Windows adapter. No new hooks or function patches. Fifty-two additional code/table guards are verified against the reviewed 0.6.3 executable.
- Owned descriptions are cached per skill, localized template and control-session generation. Pending/failed requests are deduplicated, an already-open tooltip refreshes when its result arrives, and release/new-session transitions discard cached results. Normal logs identify native/fallback results; Verbose logs include returned text for comparison. Native game calls and rendering still require the user's test.
- Regression checks cover native register returns, stack alignment/shadow space, heap text copies, last-owner Arc cleanup, cache invalidation and stale responses, optional-skill fallback, and refusal of changed tooltip profile guards.

## 0.68.2 — 2026-10-09

- Item descriptions scroll independently beneath the fixed item heading, with a slim scrollbar only when the text overflows. The entire description is reachable without ellipsis truncation; wheel input affects the description or item grid under the pointer. The description resets for a different item, preserves color runs and inline icons across scrolling, and clamps when the available height changes.
- Regression checks cover reaching the final line, extreme wheel input, item/source changes, viewport resizing, multilingual wrapping, blank lines and color/icon continuity.

## 0.68.1 — 2026-10-09

- Shop path selectors use larger plates with 16px bold text, visible borders and layered contact shadows. Automatic (cheapest) has its own row; Use this path occupies the empty space beside each chain. The selected mode/path has a yellow fill and dark text, without a checkmark, and hover darkens the actual hovered button.
- Branching chains show five components per page to leave room for the path button; paging retains every component and the fixed event slots. Single-path and vanilla items keep their compact six-column layout and existing purchase behavior.

## 0.68.0 — 2026-10-09

- Recipe prices again show the gold still needed from current inventory, cumulatively along the displayed path (250 → 900 → 1,650), rather than individual upgrade charges. Competing branches use their own costs; a full inventory does not reduce a displayed price to the last upgrade charge.
- Alternative paths gain Use this path and Automatic (cheapest). A choice is remembered per item for the current match and copied into each new purchase order, including Queue whole build. Buyer decisions, projected purchases, queue prices and the HUD's next purchase all use that order's path. Existing queued orders keep their choice; remove and requeue to change one.
- A chosen chain reuses the most advanced available owned component on that chain; it never substitutes a different branch. Invalid paths fail closed, and full inventory blocks a new chain. Automatic planning and queue-order rules are unchanged.
- Vanilla and every single-path item retain the compact linear recipe with no alternate rows, path labels, automatic-route markers or choice controls. Branching is detected from registered item data. Checks cover all 30 vanilla items and six linear chains, cumulative prices, pinned expensive routes through the real buyer interface, projection/HUD consistency, duplicate orders and blocked/invalid paths.

## 0.67.2 — 2026-10-09

- Shop recipes now use a structural upgrade graph instead of the remaining purchase plan: ancestors of owned items and all alternative paths remain accessible, even when buying is blocked or the final item is owned. Two complete chains per page, with path and component navigation for larger trees; alternative rows are explicitly marked OR.
- Builds Into preserves the branch followed. Auto-buy labels and yellow links identify the cheapest remaining purchase route independently; browsing never forces a more expensive route. Recipe tiles show per-step prices and each row its full-chain total; list/detail/Buy prices keep their existing remaining-cost calculation.
- Recipe graph and path counts are cached by live catalogue/root; paths are addressed directly without enumerating combinations. Cycles, duplicate and unknown edges are guarded. Regression checks cover the missing base component, alternate Blade routes, browsing across pages, ownership/full-inventory changes, long chains and a graph with over a billion possible chains.

## 0.67.1 — 2026-10-09

- The HUD strip's next purchase now shows what the buyer will really buy next. With Vanilla order off: the first affordable next part in queue order, or, while none is affordable, the cheapest one (the first gold will reach). With Vanilla order on: the first unfinished item, as before (shop::upcoming replaces first_open).

## 0.67.0 — 2026-10-09

- F11 now also pauses: pressed while control is running it queues the same Pause action as the Pause button (pending orders cleared as for a click); it still starts and resumes. Button tooltip "Pause · F11"; key binding label "Start, pause or resume control".
- Shop: "Vanilla order" checkbox in the shop header, on by default (saved like a setting, shop_vanilla_order). With it on only the first unfinished order buys and later orders wait, like the game's own buyer, instead of buying parts of several items; the in-base preview follows the same rule.

## 0.66.3 — 2026-10-09

- lib.rs split into lib.rs (logger and start-up wiring), client.rs and simulation.rs; Client::post_update and Simulation::think now call one named step per feature (install, athlete choice, re-arm, windows, UI blocking, pointer, diagnostics; session binding, roster, Tab, purchase trace, shop, HUD, controlled athlete, samples), moved verbatim.
- Layering: settings::MODAL and shop_ui::OPEN moved to ui_state.rs (SETTINGS_OPEN, SHOP_OPEN); settings are created at start-up and shared with the cursor and client, and saved by the client each frame instead of by the cursor. No behaviour change.

## 0.66.2 — 2026-10-08

- native_adapter.rs (about 4,100 lines) split into native_adapter/mod.rs (the API the rest of the mod calls) and native_adapter/windows/: mod.rs (patch installation and shared helpers), layout.rs (every 0.6.3 address, call site and byte pattern), movement, combat, outline, view, input, shop and tests. Pure moves: every item verified unchanged apart from formatting and crate-internal visibility.

## 0.66.1 — 2026-10-08

- Internal renames, no behaviour change: movement_test.rs → movement.rs (MovementTest → Movement), timing_test.rs → test_support.rs, ClientProbe → Client, AiProbe → Simulation; crate documentation describes the two entry points. Mod ID and probe.log names unchanged.

## 0.66.0 — 2026-10-08

- Log detail setting (Settings › Interface › Debug): Quiet, Normal (default) or Verbose. Lines are classified by tag in one place (probe/src/logging.rs); install, fingerprint and release results, fail-closed shop decisions and anything reporting a failure or panic are always written; per-click, per-move and per-second traces (MANUAL CLICK, MOVEMENT, CONTROL DIAGNOSTIC, NATIVE TRAFFIC, OUTLINE, PERF, CURSOR TRACE, attack and ability traces) only at Verbose. A full match log drops from about 5.4 MB to about 0.12 MB at Normal.

## 0.65.2 — 2026-10-08

- Shop recipe tree: every part not owned shows the gold it still needs from what you own (League price, red when unaffordable), not just its own step price; owned parts keep their muted own price

## 0.65.1 — 2026-10-08

- Cursor flicker fix: the game sets its cursor on every WM_SETCURSOR (0.65 log: game SetCursor calls equal WM_SETCURSOR count); while our cursor is active on the foreground game window, the game import call shows ours instead, so the game cursor no longer flashes before ours while moving
- Removed the full native UI-tree log dump on scene changes (196 ms freeze at match start in 0.65 log)
- Shop window caches purchase plans (shop::offer) and the in-base projection until catalogue, inventory or queue change, instead of recomputing them several times per frame while open
- 0.65 diagnostics (PERF, CURSOR TRACE) and lock-free hook rejection kept; CURSOR TRACE also reports shown_ours_instead

## 0.65.0 — 2026-10-08

- Diagnostic build: PERF summary every 10 s (client update intervals p50/p95/p99/max, mod callback time with per-part breakdown, native viewer/worker time kept separate, lock and worker pacing waits, hook calls split by viewed-match worker vs other threads, log bytes) and up to 8 PERF SLOW lines per window for battlefield frames of 25 ms or more
- Cursor trace: CURSOR TRACE every 10 s and up to 6 CURSOR HANDBACK lines per window with reasons; counts replacements of our cursor between frames, WM_SETCURSOR reapplies and the game own SetCursor calls through a pass-through import-table counter (nothing substituted; restored at shutdown)
- Lock-free rejection before the abilities lock (movement/attack/skill hooks) and the shop lock (buyer hooks) for entities and players that are not the controlled ones; outcomes unchanged, full ownership checks kept

## 0.64.2 — 2026-10-08

- Item slot count comes from the game purchase gate (cmp rax,N at 0x146baf3 allows N+1: 4 vanilla, 6 Riot), read-only, instead of the build plan length; used by the shop, the strip inventory and Tab, with the plan length only as a fallback
- SHOP MODE first decision logs game_item_slots

## 0.64.1 — 2026-10-08

- Orders instead of a set: each Buy is an order that excludes the copies already owned, so the same item can be bought several times; orders claim their satisfying slot or the part their route builds on, in queue order
- No purchase is blocked (user rule; the Riot mod manual plan does not restrict either); only the game rule that a new item needs a free slot remains
- Messages on a solid plate under the Buy button with a tone bar (bought/queued/refused), 2.5 s; chips remove a single order and show buying

## 0.64.0 — 2026-10-08

- Purchases shown at once: in base the shop projects the queued steps exactly as the hooks hand them out (shop::project) and uses the projected inventory and gold for prices, dimming and Buy; gold reads real -> left; pending slots get a yellow outline and clock; pill says purchases apply when the match resumes while paused
- Queue is first come, first served (unaffordable items never block); Recommended gets Queue whole build
- Recommended tab label widened to fit

## 0.63.3 — 2026-10-08

- Item list: owned badge removed (owned items look normal); only queue positions are badged
- Item frames get an opaque inner fill so a dimmed icon never shows the frame colour (fixes the yellow tint on a selected owned part)
- Selected highlight is blue RGB 16,162,217 in the item list, the recipe tree focus and Recommended rows

## 0.63.2 — 2026-10-08

- Starting items: the game buys them at tick 1 while the lane can still change; before Start (Loading/Ready) Manual shopping now holds every champion of this match by native object (other simulations untouched); at Start only the controlled champion stays manual and the others buy on the next tick. First-decision log shows the controlled champion items
- Dimming: item list dims items you cannot buy now (gold or full inventory), owned look normal; recipe tree and Builds into dim owned parts with a tick glyph and dim unbuyable parts without one
- Hook proof counts prestart_held answers

## 0.63.1 — 2026-10-08

- Fix: Manual shopping read the session phase with the client-only phase(), which is None on the simulation worker, so every decision was native (shop showed Manual shopping off, starter and later items auto-bought). New worker_phase(); Manual applies in Loading, Ready, Running and Paused once the champion is locked, until F12
- Every change of shop mode/reason is logged (SHOP MODE first decision / changed)
- Builds into click moves the recipe tree to that item (root and focus); clicks inside the tree still only move the focus

## 0.63.0 — 2026-10-08

- Manual shopping applies from champion lock (Ready, Running, Paused) until F12, and the shop publishes as soon as this worker owns the match, so starting items are no longer auto-bought; the shop can be opened during the Start-control wait
- League-style right panel: Builds into (focused item) -> recipe tree (root, fixed until an item is picked from the list) -> Buy (focused item) -> detail; clicks inside the tree or Builds into only move the focus
- Hover tooltip following the cursor over any item (grid, Recommended, tree, Builds into, slots, queue), 117 ms fade-in, instant hide
- Prices are League-style: gold still needed from what you own (tiles, Builds into, tree root, Recommended, tooltip), red when gold is below it; tree parts show their step price
- SHOP MODE first decision of match logged

## 0.62.0 — 2026-10-08

- Shop text: native line_height is pixels; body uses 23 px (0.61 used 1.4, which stacked lines)
- Shop detail panel flows top to bottom (stats and effect, Recipe, Builds into) above a fixed Buy area; the Buy button names its item
- Stat filters from live item stats (AD, AP, attack speed, crit, haste, life steal, armor, magic resist, health, move speed, armor/magic pen), multi-select AND with live counts; works for vanilla and item mods
- Pause while open: header checkbox in the shop, saved as a hidden setting; uses the Settings pause request and only resumes a pause the shop made; in-base state held while paused
- Strip: bag icon removed; the purchase column (next item, gold, required gold, P cap) is the shop button; with Manual shopping on, next purchase and required gold follow the shop queue and are blank when nothing is queued

## 0.61.0 — 2026-10-08

- Native shop window (P or the strip shop button beside the gold): Recommended (the game build list) and All items with category filters, level-grouped item grid with a fixed 63-tile pool and wheel scrolling, detail panel with stats and effect text (inline stat icons), cheapest recipe route, Builds into, one Buy/Queue button, owned slots and queue chips; right-click buys; Esc closes without cancelling a recall; hidden while Settings is open
- Manual shopping: nothing is ever queued automatically (P no longer queues the build list); Buy moves the item to the queue front and the first queued item with an affordable next step is bought; Manual shopping on with missing item data fails closed (buys nothing)
- Hook proof logging: champions seen by the buyer hooks, answers given to the controlled champion, SHOP UNEXPECTED for any item that arrives without a shop step; the In base indicator is driven by the hook being asked about the controlled champion
- New Lucide glyphs ef_star, ef_grid, ef_bag (white-only verified)

## 0.60.0 — 2026-10-08

- Manual shopping (Combat & casting > Shopping, Off by default): optional redirects of the native buyer controller thunks 0xf60620 (upgrade) and 0xf61b80 (buy new) answer from the player queue for the controlled champion only; everyone else, AI control and the setting Off get the native answer (another mod patch inside 0xf3f510 is preserved); hook failure disables only Manual shopping
- The simulation still validates and performs every purchase; the mod only answers one step at a time, never buys a non-base item as new, and never answers the same step twice
- Temporary queue source until the shop window: P queues the next unfinished item of the champion build list
- Item tooltips keep the item mod inline stat icons (<i#asset/...:name>); skill tooltips unchanged
- 0.59 diagnostic shop trace retained

## 0.59.0 — 2026-10-08

- Diagnostic-only shop trace: logs every purchase step of all ten players (gold before/after, owned items before/after, native owned-slot count, final build list); copies item lists only when gold falls or every 30 ticks; 400 lines per match
- Once per match, compares the live native buyer code (executor slot gates and payments, controller buy/upgrade vtable slots, their thunks and decision prologues) with reviewed 0.6.3 bytes and names the module owning any redirect
- No native state is written; 0.58.2 behaviour, outlines and layout are unchanged

## 0.58.2 — 2026-10-08

- Coordinate native renderer/config and IngameUI full-screen flags using the existing viewer hook and its live ingame node, with exact TypeId and shared-config identity checks.
- Let native UI update position announcements, kill feed and spectator controls in the full-screen layout. Remove 0.58.1 option_buttons visibility/event workaround; retain baseline custom-HUD panel suppression.
- Save and independently restore both original layout flags on F12; reject different viewers/configs/UI objects and changed shared-config links; discard stale lease at match reset.
- Keep user-approved outlines unchanged: hover 1.2, attack target 2/3, click peak 3.0, decay 120 ms, click > hover > attack.
- Add exact native profile guards and regression coverage for coordinated layout restoration, live-node downcast and identity checks.

## 0.58.1 — 2026-10-08

- Fix hover offset at 1.2; remove 2.5 wide-hover mode, F10 prototype polling/switch, mode state, client switch argument and package description.
- Retain pulse peak 3.0, 120 ms interpolation, attack offset 2/3 and click > hover > attack same-unit priority.
- Hide the separate compact spectator ingame.option_buttons group during direct control; save/restore native visibility on AI release and suppress/restore its eight known interactive descendant surfaces.
- Retain 0.58 full-width battlefield lease, native profile guards, outline cleanup, all-role outline setting and independent debug selection markers.

## 0.58.0 — 2026-10-08

- Reduce persistent attack-target outline offset from 1.0 to 2/3; retain exact 1.5 crisp and 2.5 wide hover appearance.
- Accepted explicit attack clicks pulse from offset 3.0 to the settled offset over 120 ms. Repeated clicks restart; held buttons, worker ticks and ground attack-move acquisition do not. One role per unit: click > hover > attack.
- Discard pulse on expiration, replacement/cancelled order, fog/death, inactive control and session reset; Sprite outlines gates all roles; debug ground markers remain independent.
- Lease the native full-width battlefield flag before owned bootstrap/update and restore the spectator preference on return to AI. Bind camera/vision/layout restoration to the matching live viewer/config only; fresh sessions save fresh preferences.
- Add two reviewed native layout consumer guards to the runtime profile and source verifier; retain all existing outline calls, ownership cleanup and safe fallback.

## 0.57.0 — 2026-10-08

- Add persistent thin red sprite outline for the valid active Attack/AttackMove target, independent of the single stronger hover winner and cursor.
- Publish captured hover and attack target together; hover role wins for the same entity to avoid a double border or duplicate outline pass set.
- Use authoritative alive/targetable/visible snapshot before enemy refresh to clear death/fog targets; stop, move, no acquisition, stale/inactive state and reset clear target feedback.
- Rename Interface option to Sprite outlines and gate both roles using existing saved hover_outline key; debug ground markers remain independently gated.
- Use 1.0 offset for attack target and existing 1.5/2.5 crisp/wide hover; record hover_drawn and target_drawn diagnostics; no new native hooks.

## 0.56.1 — 2026-10-08

- Correct Show selection markers to gate both hovered and selected attack-target ground circles/construction corner markers. Debug Off hides both, Debug On restores both.
- Update debug hint to cover hover and selected targets. Selection, orders, sprite outlines, skill/range previews and minimap paths retain their existing behaviour.

## 0.56.0 — 2026-10-08

- Add persistent Interface > Battlefield feedback > Hover sprite outlines (On default), gating only the published native outline target and retaining picking/cursor feedback.
- Add persistent Interface > Debug > Show selection markers (Off default), retaining existing hover circles/construction markers as a tuning tool independently of outlines and attack-order markers.
- Fix vision-mode hover: compute pointed segment at HUD scale and animate its own non-interactive hover layer; selected indicator and tooltips remain independent.
- Reuse declarative option catalogue, Apply/Cancel, restore, controls.json and scrolling; older settings receive new defaults without overwriting user preferences.

## 0.55.3 — 2026-10-08

- Correct tower-only hover outline: accept NinePatch ground circles, Text labels and all reviewed native RenderCommand variants; outline sprites only and append complete original drawing unchanged.
- Release unused independently owned extra-pass commands through fingerprinted game RenderCommand cleanup; preallocate bounded merge before extra renders; clean up valid extras on fallback without drawing duplicates.
- Retain single shared hover winner, enemy-before-ally priority, blue allies/red hostiles, F10 crisp/wide and ground markers pending approval.
- Log observed command variants plus malformed-vector and missing-sprite counters; guard native cleanup head/table/caller and NinePatch tag; verify all cleanup bytes and dispatch statically.

## 0.55.2 — 2026-10-08

- Accept mixed native Sprite and DrawLine body commands; outline only the independently owned sprite copies and preserve original commands.
- Capture one hover winner per frame for the cursor, sprite outline and ground marker; publish native identity, position and team together.
- Hostile units outrank allies, including enemy minion over allied champion; champions retain priority within each side and champion-only filtering is preserved.
- Targeted aiming uses the skill-eligible single unit or clears hover on a miss; area previews retain multi-target geometry.
- Blue allied and red enemy/hostile-neutral outlines and hover markers; F10 compares crisp and wider coloured outlines, ground indicators retained.
- Guard the native DrawLine tag write; include accepted mixed-command draw counts in outline diagnostics.

## 0.55.1 — 2026-10-08

- Hover-only sprite outline prototype: four shifted gold flash passes or one soft glow pass, switched with F10 in the same match
- All 15 generic unit-body renderer calls are version-fingerprinted redirects; ordinary units keep original rendering and existing ground rings remain
- Native render commands are moved into a new vector; empty vectors and failed heap frees are guarded, and changed command shapes disable the prototype

## 0.55.0 — 2026-10-08

- Hover-only sprite outline prototype: four shifted gold flash passes or one soft glow pass, switched with F10 in the same match
- All 15 generic unit-body renderer calls are version-fingerprinted redirects; ordinary units keep original rendering and existing ground rings remain
- The wrapper checks native entity identity and position, uses game shader setters, moves independently owned commands into a new vector, and disables itself if the render command shape changes

## 0.54.0 — 2026-10-08

- On/Off settings use compact full-row checkboxes from the updated HTML preview, preserving saved values and click routing
- Segmented-control hover brightens the unselected track while the selected pill remains stable until clicked
- Raised settings buttons and the dropdown have contact shadows and crisp outlines modeled on the Endfield UI lab

## 0.53.0 — 2026-10-08

- Settings window rebuilt to the approved preview geometry: section headings, segmented track with sliding indicator, select field with chevron and fading option menu, icon close button, scrolling content with clipping masks and scrollbar, keybind capture hint and column headings
- New Combat & casting > Attacks option: Cancel attack wind-down (On default) gates the 0.51 post-hit release
- Bottom strip reordered: Pause, Return control to AI, Lock camera, Settings (Lucide gear), three-way vision group, K/D/A, CS, Tab; the more menu is removed; B recall moves beside the level and HP bar
- Option labels and hints match the preview wording; real line breaks via native text instead of escaped source text

## 0.52.0 — 2026-10-08

- Schema-driven native settings window: Combat and casting, Keybinds, Camera, Interface; reusable choices, sliders and binding capture/conflict replacement; old more-menu preferences migrated
- Settings own only the pause they caused in the same match/generation; modal input and camera gating, held input protection, Apply/Cancel drafts and required bindings
- Shift-right-click attack-move; configurable Hold/Toggle champion-only (Hold default), attack-move filter and nearest champion/cursor preference; per-slot cast modes and primary/secondary keybinds
- Versioned controls.json retains cursor/low-health preferences and unknown fields; new defaults populated on Apply; native selection, targeting, item and post-hit attack behavior retained

## 0.51.0 — 2026-10-08

- Post-hit attack release: a locked BaseAttack (action 3, no queue entry) records its hit tick from declared start timing (+4a8) scaled by the action speed factor (+80); release requires elapsed (+78) strictly beyond it, an empty pending queue, native CC gate and the attack's own cooldown
- Release reasons: a waiting buffered skill (previously dead gate, now reachable) or a manual ground Move / S stop issued after the attack started; attack, A-click, recall and automatic chase/hold inputs never release
- Release uses the native stop event and clears only the action; cooldown, damage and queued (Gunner-style) attacks unchanged; 0.50 attack trace retained

## 0.50.0 — 2026-10-08

- Diagnostic-only basic-attack trace: per accepted attack, classify the native queued versus locked (action 3) outcome, record raw start timing, attack-speed bonus, expected hit tick, both action payload words, queue and cooldown, then follow action/queue changes at existing observation points
- Bounded to 400 attacks and 12 changes per attack per match; no native writes, hooks or behaviour changes
- Retain all 0.49 Ghost tooltip, glyph alignment and click-through Tab behaviour unchanged

## 0.49.0 — 2026-10-08

- Resolve Ghost Q from skill1 declarations and champion-level takedown bonus fields; Ghost W Time placeholder uses declared charge count
- Align cooldown/range tooltip glyphs with their numeric text
- Remove Tab row hover tint and all Tab input-blocking geometry so battlefield commands pass through the informational panel
- Retain accepted 0.48 repeated Tab reopening, KDA colour formatting, compact purchase shortfalls and adaptive inventory

## 0.48.0 — 2026-10-08

- Keep styled Tab visibility and property cache synchronized on close/reopen, including focus loss and AI handoff
- Correct KDA rich text to native eight-digit RGBA tags with a final colour reset
- Show uncertain purchase branch shortfall as one compact minimum plus sign; retain exact branch prices and remaining gold in tooltip
- Preserve approved Tab layout, adaptive inventory slots and accepted controls, native vision, minimap, HP and screen effects

## 0.47.0 — 2026-10-08

- Implement approved styled native Tab panel without team-name stripe, with live level/KDA/CS/held gold/items/respawn and selected-row edge
- Use native automatic-buy target vector for dynamic slot count, owned-item lower bound and same-match/player retention; no mod-ID or catalogue-size assumptions
- Right-align four/five/six HUD slots at fixed tile/art size; align Tab item columns using match-wide capacity and preserve per-player slot counts
- Update inventory tooltip hit areas and separate next-purchase identity from item indices; allocate additional rows/slots beyond six within bounded read limit
- Preserve accepted 0.46 purchase-reader, construction selection, HP contrast, optional low-health effect, native vision and F11/F12 control behavior

## 0.46.0 — 2026-10-08

- Fix registered item lookup: native buying context +8 points to settings; remove erroneous simulation tick +1b8 chain, retain stage-specific rejection diagnostics
- Preserve valid catalogue entries when unrelated upgrade destinations are absent; forecast still requires complete selected paths
- Map generic tower/nexus names to base art profiles, use full body bounds, and draw construction body corner highlights instead of an inflated ground ellipse
- Use dark green HP fill with outlined white current/maximum values
- Render optional stronger low-health warning using cached native UI nodes, clipped to battlefield and excluded from minimap; retain static fade and death shading
- Provide separate Tab/HP HTML proposal and side-by-side comparison; installed Tab remains unchanged pending review

## 0.45.0 — 2026-10-07

- Implement approved Endfield HTML tooltip typography, 529 px content cards, native stat colors and compact purchase-unavailable state
- Package Manrope 500/650 and Noto Sans TC 500, language fallback font sets and 16 SVG-derived HUD/menu glyphs with licenses
- Implement 328 px cursor/vision/effect menu, capsule slider and component-specific tonal hover/press, segmented selector and opacity-only tooltip motion
- Bind native vision to controlled side by default, offer own/opponent/all and restore original spectator vision on release, with four exact consumer guards
- Change default start/resume to F11 and AI return to F12; retain existing skill/input/purchase policies and expanded minimap

## 0.44.0 — 2026-10-07

- Implement reviewed warm HUD palette, native bold text, 80 px skills, adjacent inventory/purchase group, lime HP and outline-only champion selection
- Close six-pixel strip gap above controls without changing battlefield rendering allocation
- Add bounded presentation-only hover/press, selection, health trail, tooltip fade/rise and panel entrance motion; cache individual properties
- Add optional default-off low-health static edge tint with smooth onset, minimap/top-bar exclusions, death/release cleanup and persistent toggle
- Retain native cooldown, charge, modded item/skill data and purchase branch uncertainty; no speculative gameplay timers

## 0.43.0 — 2026-10-07

- Expand native minimap to 352 px inside a 360 px footprint with 4 px warm-grey surround and muted stepped corner strokes
- Resize native map background, fog grid, markers, objective timers and camera rectangle using 54 reviewed instruction operands and private read-only constants
- Align native left-click camera navigation and mod right-click destinations, path and click indicators with shared live map geometry
- Trim the bottom strip to x1560 and preserve current combat HUD text/icon sizes; broader approved HUD redesign remains separate
- Guard every new instruction and original constant; protect each executable page once and audit patched bytes/private constants

## 0.42.0 — 2026-10-07

- Migrate reviewed native worker/view/input/attack/skill/steering/shader locations to exact game 0.6.3
- Update entity kind/position/identity/skill/effect fields and native purchase build/settings readers for changed layouts
- Add independent loaded-code field-consumer and ASLR-correct table-pointer guards before installing hooks
- Verify explicit profiles against executable identity, call targets, layout guards and Rust constants; preserve both full game baselines privately
- Reuse 0.41 HUD, controls, cursor art, previews and input-buffer policies; approved revised HTML implementation remains separate

## 0.41.0 — 2026-10-07

- Implement supplied HUD anatomy with separate 80px skill tiles, flat neutral 50px bottom strip, centered 360x24 HP bar and larger KDA/CS glyphs and values
- Use supplied SVG HUD glyphs; six inventory slots on right, working automatic-purchase forecast/gold on left; preserve mod icons, colored descriptions, death banner and visual-only hover preview
- Add explicit visible slider track, ratio fill and clamped handle; keep native slider input, saved 24-64 px sizing and reset
- Extend minion selection upward five world units and downward from two to five without widening; known towers/nexus use full structure profiles and unidentified towers get a 48x80 fallback
- Keep team availability on Tab, vanilla top stripe and minimap placement, native movement/casting/timing, and cursor assets unchanged; true outlines deferred

## 0.40.0 — 2026-10-07

- Extend champion picking upward by 20 percent of baseline height, bounded to 6-12 world units; preserve lateral/feet envelopes, monster profiles and ground-ring dimensions
- Keep only the latest accepted click marker; preserve contraction and fade from 100 to 250 ms, expiring at 250 ms without cancelling movement orders
- HUD skill hover shows known live reach or caster-centered area without arming, aiming or queueing a cast; active normal-cast preview has priority
- Reuse verified cursor artwork and size settings; native buffering, skill input, geometry readers, movement and timing remain unchanged
- Bounded outline feasibility review found no verified per-entity current-frame association; retain existing highlight and defer true silhouette outline

## 0.39.0 — 2026-10-07

- Champion picking uses idle/run/walk baseline with at most 15 percent combat-pose expansion; retain feet forgiveness and unchanged monster profiles
- Embed fourteen cursor states from supplied SVGs using native Windows pointers, scaled hotspots and a thread-local post-WM_SETCURSOR observer; restore host cursor over UI and after release
- Cursor states share direct-click enemy picking and normal-cast ally/CC/self eligibility; preserve persistent A mode and champion-only toggle behavior
- Add saved 24-64 screen-pixel size slider, default/reset 32, to existing more-controls menu without replacing other controls.json preferences
- Replace permanent battlefield destination cross with clipped move/attack click markers: 167 ms contraction, fade from 367 to 567 ms; minimap clicks receive a smaller minimap marker
- Keep native command buffering, preview geometry, movement and timing unchanged

## 0.38.0 — 2026-10-07

- Separate casting input from shape and placement; support multiple primary footprints without flattening secondary hit effects
- Copy bounded geometry from exact-build live selected-actor Combine, RangeEffect, LinearProjectile and channel-line effect families at most 10 times per second; retain no native pointers in HUD
- Prefer recognized live dimensions; enabled explicit declarations are a fallback for opaque effects, with bounded per-skill source/family/unsupported diagnostics
- Render self-centered areas without misleading cast-reach rings; allow unit-targeted corridors and native forward-offset areas; decode finite segments, rectangles and DirDot cosine thresholds
- Keep existing native cast validation, input modes, manual aim preservation, HUD/cursor and startup timing

## 0.37.0 — 2026-10-06

- Preserve manually controlled direction/point skillshot aim at native effect launch: native stat/RNG evaluation runs once on a POD copy; AI players retain original correction
- Log intended and native-suggested directions when correction would redirect a manual skillshot
- Adapt supplied preview HTML geometry into native drawing: white dashed reach, yellow declared corridors/circles, cyan declared movement, sprite-sized unit brackets and orange approach/out-of-range feedback
- Unknown, conflicting or state-dependent effect footprints use reach/aim guides rather than guessed widths or hit predictions; native range includes live level/bonus changes
- Preview strokes clip to battlefield and exclude minimap/HUD; unavailable abilities grey out; existing normal/quick/Alt/release cast and cancellation rules retained
- Record supplied design integration decisions: Tab-only availability, neutral grayscale panels, six item slots and existing input modes

## 0.36.0 — 2026-10-06

- All native movement, attack and skill observers require the registered live simulation worker, current match key and selected actor; background actor-ID reuse cannot change charges, busy state or acknowledge casts
- Clear-segment steering recovers current native position fields without a previous-command address token; native speed, events, wall fallback and routing are retained
- Native automatic-attack call is observed in addition to explicit attack input; eligible buffered skills can release only proven committed BaseAttack animation, preserving cooldowns and pending effects
- Cast cancellation, expiry, target loss, busy transitions and native rejection include trace IDs and concrete reasons; retries do not flood the log
- Native traffic records distinguish steering entry, selected-worker entry, direct steps and pointer rejection; attack-start records identify input versus native-auto source

## 0.35.0 — 2026-10-06

- Skill edge wins a simultaneous mouse-click poll; a later click cancels the older cast and native acknowledgement prevents replay
- Selected actor uses native arbitrary-angle stepping on verified clear segments, retaining speed/events and normal navigation fallback
- Ready buffered skill can release a BaseAttack backswing only after observed start delay and an empty pending-effect queue, retaining attack cooldown and CC guards
- HUD uses declared cast count with bounded display retention rather than deriving count from rounded capacity/cost; zero-cooldown display hides phantom multi-use indicators
- Runtime diagnostics rotate at 4 MiB through current and two earlier segments; late-match movement sampling no longer ends after 1800 samples

## 0.34.0 — 2026-10-06

- Static skill artwork and last-known item icons survive transient telemetry expiry for the same selected identity; live readings remain freshness gated
- Ordinary self-hover highlight is filtered by controlled entity ID while overlapping allies and self-target ability data remain available
- Movement route code and native hooks unchanged; archived 0.33 traces and route review distinguish continuous motion from intermediate grid-derived goals

## 0.33.0 — 2026-10-06

- Hover snapshots publish only from the selected living player in the owned live match; unrelated callbacks cannot clear allies
- Hover snapshots have independent freshness and clear on death, inactive controls, prepared selection and session reset
- Base jungle monster profiles use idle/run/walk frames instead of expanded attacks; champion and structure profiles are retained
- Monster selection and highlight envelopes use smaller bounded foot/edge padding; tested champion and minion bounds are retained

## 0.32.0 — 2026-10-06

- One-frame running lead and viewer/phase condition-variable wake with bounded emergency fallback
- Running gameplay capture moved into lightweight pre-update using visible camera/masks; edge processing is not repeated post-update
- Purchase forecast includes tier-4 Radiant/custom finals with assigned-build graph and inventory consistency checks
- Declared skill values resolve with preserved native colors and AD/AP ratios; unsupported formulas remain unknown
- Champion/monster picking and highlight dimensions share enlarged sprite envelopes and foot padding; minions retain smaller bounds
- Bounded capture-dispatch-publication-playback and native queued-aim diagnostics; three queue-layout anchors added

## 0.31.0 — 2026-10-06

- Champion-first sprite rectangles include legs and forgiving edges for allied/enemy hovering, attack clicks and targeted skills
- Minion mouse boxes use normal sprite dimensions rather than combat radius; base monster art profiles bundled with bounded fallbacks
- Correct reversed registered upgrade edges: native getter +0x80 is next_tier; three constructor/getter anchors added
- Purchase traces report effective chain data and forecast only when build, inventory or metadata changes
- No additional native gameplay patches or HUD layout changes; Ninja movement and FPS remain deferred

## 0.30.0 — 2026-10-06

- Read effective registered item keys, icons, enabled state, incremental prices, tiers, native stats and upgrade edges inside live AI callback
- Owned metadata cached per match and validated against SDK catalogue; no native pointers retained and no new gameplay patches
- Purchase forecast uses registered modded prices and enabled branches; tooltip names use active localization
- Item descriptions resolve active localization even without base settings; registered stats and native RGBA color markup retained
- Background map callbacks stage geometry without clearing live destination or remaining route
- Five additional native item-reader anchors verified; Ninja movement and FPS investigation remains deferred

## 0.29.0 — 2026-10-06

- Respawn banner follows native ticks; existing battlefield-only death grayscale retained
- Accepted world orders survive focus loss and expired foreground heartbeats
- Minimap right-click destinations and remaining commanded obstacle route from read-only map geometry
- Identical active movement goals skip redundant native movement-reset events; bounded plain-walking traces
- Busy casts wait regardless of SDK command validity; three native skill consumer hooks acknowledge cooldown spending
- SDK item-build observer export pointer/length repaired; catalogue capture tested through host-facing vtable
- Three additional native CALL redirects and eleven additional byte anchors verified

## 0.28.0 — 2026-10-06

- Approved bottom-strip layout: fixed 224x124 combat panel, 64px skills, green HP, horizontal KDA/CS, gray flat strip, read-only inventory tracker
- Compact champion selection above strip; play/pause, camera lock and AI popover controls below; no persistent portrait/name/target-mode rows
- Team death status restricted to held Tab native scoreboard; right-click passthrough retained
- Death greyscale replaces only the #Game composite shader; HUD, top stripe and minimap composition remain unchanged
- Read-only final native build lookup after all overrides; active mod item prices and graphs; random branch costs are explicitly uncertain
- Original monochrome toolbar glyphs, yellow selection/check accents, retained custom champion/item texture handling
- One additional native CALL redirect for battlefield shader; five additional byte anchors verified, seventeen total

## 0.27.0 — 2026-10-05

- One replaceable cast intent: animation buffering up to one second, press-time unit binding, fixed world aim, targeted approach-to-range and Alt self-casting
- Optional cast-on-release preference; default quickcast and persistent Shift normal casting retained
- Champion-only mode filters A-click and ongoing attack-move acquisition; allies receive hover feedback
- Native cooldown/use-count shared-borrow reads drive remaining-use indicators and readiness
- Body-pose envelopes cover attack and skill tags while excluding separate effect tags; exact live-frame alpha picking remains unavailable
- Startup prepared-HUD capture and nested button-color properties corrected; redesigned layout remains a separate review mockup
- No new patched branches; twelve existing anchors unchanged; new read-only Action method calls verified against current consumers

## 0.26.1 — 2026-10-02

- Direct control activates when the mod is enabled; no development path or activation flags required
- Per-user LOCALAPPDATA diagnostic folder with TEMP fallback; unavailable logging no longer blocks mod initialization
- Read-only result reports use the selected portable diagnostic folder rather than the compile-time project path
- Retains 0.26 graphical UI and gameplay; C-only laptop and native UI testing pending
- No new native branches or offsets; all twelve anchors unchanged

## 0.26.0 — 2026-10-02

- Portrait-only own-team selector on the held battlefield; role changes allowed only before Start and bound to current match/generation
- Prepared snapshots for all players; re-selection clears movement, skill, recall and camera state without releasing the worker
- Preparation waits for Start or AI without 120-second automatic release; startup and two-second heartbeat guards retained
- Original graphical session controls and HUD symbols, opaque grayscale framing, exact yellow active accents and red death count
- Numeric level/HP/KDA/CS/gold, dimmed locked abilities and numeric portrait respawn timer; existing permanent sheet/PNG nodes retained
- Host-localized skill/item descriptions on delayed hover; unresolved formula values shown as ellipses; tooltip bodies ignore events and do not mask world commands
- Brief rejection feedback distinguishes known unlock levels/cooldowns; ordinary startup/control/aiming diagnostics kept in log
- No new native branches or offsets; all twelve anchors unchanged

## 0.25.0 — 2026-10-02

- Champion picking uses normal-pose body dimensions independently of combat collision and attack range
- 78 base animation profiles plus enabled-pack exported animation/Aseprite metadata discovered at initialization; unreadable art uses a fallback
- Shared picker for hover, right-click, direct A-click and targeted quick/normal cast; width and height scale with current camera extent
- Core body hits rank ahead of margin hits, then distance to visible body center and stable ID; champion-only filter retained
- Pick visible upper body with offscreen feet; reject off-viewport/minimap cursor; legacy minion/structure envelopes retained
- SDK does not expose exact animation frame/facing silhouette; padded normal-pose proxies require in-game fit verification
- Retain user-confirmed direct chase, target markers and team portraits; existing HUD, camera, casting and recall guards unchanged
- No new native branches or offsets; twelve anchors unchanged

## 0.24.0 — 2026-10-02

- A direct hit commits Attack(id) and follows the moving target; ground A preserves nearest-to-click acquisition
- Explicit target loss clears the order without fallback; A remains unrestricted by champion-only mode
- Enemy hover uses click hit ranking and masks; separate persistent current attack-target ring; stale/dead/hidden targets excluded
- Two compact five-player portrait rows use original-worker SDK side/lane/alive/respawn scalars; permanent mod-compatible portrait nodes
- Dead identity retained without entity; countdown freezes on pause and stale samples hide numeric timers; session reset clears roster and sampling
- Targeted ally-CC skill rejection explains its native requirement; no native validation override
- Retain confirmed consecutive-session lifecycle, compact HUD icons, camera, casting, recall and recovery guards
- No additional native branches or offsets; twelve anchors unchanged; match 33 recorded controlled series verified as 2-1

## 0.23.0 — 2026-10-02

- Rearm outside the battlefield on pre-match screens or return to title; new worker tick 1 must repeat startup acknowledgement
- Refresh own-team ownership and roster, unlock lane choice while retaining its default, clear orders and actor metadata
- Generation-scoped publication waits and retired-key/binding-window checks reject old worker and same-battle reentry
- Serialize session consumer reset against SDK scalar writes; clear HUD, targeting and camera state including finished native lease
- Seed physical input history; disable placeholder skill/item hover popups
- Retain confirmed permanent icons and death camera, native countdown, combat and session guards
- Use seed/generation result evidence filenames and retain bounded polling between battles; clear audit at title
- No additional native branches or offsets; twelve anchors unchanged; saved authority unverified

## 0.22.0 — 2026-10-02

- Restore in-place updates on permanent HUD image children; no per-icon remove/spawn operations
- Separate permanent PNG and sprite-sheet skill nodes to avoid inherited rect_tag state
- Log property application, node existence and bounds for changed icon references
- Retain selected camera identity without a living position; continue drag and edge pan during death
- Follow/recenter require a living position; preserve lock intent until manual pan, with no free-camera respawn snap
- Native death countdown, active mod asset/item metadata, session/combat controls and frozen result evidence retained
- No additional native branches or offsets; twelve anchors unchanged; saved authority unverified

## 0.21.0 — 2026-10-02

- Read enabled data-champion PNG skill declarations in configured order; exclude disabled packs and reject invalid paths
- Use active SDK item icon metadata; retain logical sheet overrides and evidence-based native item key/tag fallback
- Recreate image children on source/tag changes to avoid stale sprite-sheet cropping on standalone PNGs
- Sample selected player through other living actors on the original worker, deduplicated per tick, including death/respawn
- Keep dead portrait/inventory/skill art and native respawn timer; dim skills and clear pending commands
- Freeze original-worker player evidence at battlefield exit and capture visible result-screen labels read-only
- Existing session/combat/camera controls, guards and twelve native anchors retained; saved authority unverified

## 0.20.0 — 2026-10-01

- Scoped native UI buttons queue Start, Pause, Resume and Return to AI; Ctrl+Home/End retained as backups
- Pause holds publication and playback while retaining camera/Tab; clears pending commands and prevents held-input replay
- Removes the 60-second running cutoff; startup, Ready, heartbeat, identity and exit recovery guards retained
- A persistently displays fresh native basic-attack Effect range until confirmation or a canceling gameplay command
- Basic-attack metadata copied at the existing consumer hook; nearest-to-click attack-move and champion-only policy retained
- Read-only post-battle history/replay candidate capture; saved-result authority explicitly unverified
- No additional native hooks; one controlled foreground battle per process launch remains

## 0.19.0 — 2026-10-01

- Backtick and Mouse 4 independently toggle champion-only direct targeting once per press; simultaneous presses toggle once
- Match/player-bound mode resets on release or identity change; takeover and focus return cannot replay held buttons
- SDK entity is_champion flag filters direct-click and unit-skill cursor hits before ranking and native validation
- Attack-move keeps all eligible enemies and nearest-to-click ranking; ground/direction/self/no-target skills unchanged
- Normal casting keeps aiming through mode changes and snapshots mode at confirmation; quickcast snapshots mode at press
- Persistent CHAMPIONS ONLY HUD indicator; unlearned Q/W/R dim at unlock levels 1/3/5 with required-level label and hint
- No new native hooks or simulation mutations; existing controls, confirmed HUD cleanup and 60-second guard retained

## 0.18.0 — 2026-10-01

- Explicit draw order for HUD background, frames, icons, cooldown shading, text and owned tooltip
- Smaller 600x120 bottom-center panel; portrait, skill/item slots, hover geometry and fallback command mask resized together
- Separate native champion_info_tooltip suppressed during control, including lazy creation; bounded existence and suppression diagnostics
- Native spectator hover resumes on release; existing HUD snapshot policy, controls and twelve native anchors retained
- No new native hook or simulation mutation; 60-second guard retained

## 0.17.0 — 2026-10-01

- Bottom-center native UI champion portrait, level, health, Q/W/R icons/cooldowns, gold, six owned-item slots, K/D/A and CS
- Match/player-scoped SDK scalar snapshots every six ticks; 250 ms running freshness and paused READY retention
- Native skill indices 0/1/2 and explicit champion overrides; item key aliases resolve installed sprite tags
- Scoped spectator toolbar/portrait hiding; known portrait hover surfaces ignore events; native stray tooltips suppressed
- HUD-owned brief skill/item hover hints; HUD/tooltip rectangles block unintended world and camera commands
- Running diagnostics and hit mask shrink together; READY/failure diagnostics remain
- No new native branch or simulation mutation; existing twelve anchors, controls and 60-second guard retained

## 0.16.0 — 2026-10-01

- Shift plus physical Q/W/R press selects persistent normal-cast aiming; key release keeps it
- Battlefield left-click confirms with current camera/cursor; rejection keeps mode without auto-retry
- Gameplay commands cancel aiming; camera/Tab preserve it; confirmation never also issues attack-move
- Generation check rejects requests superseded during native skill validation
- B sends native Return once and clears previous order; idle ticks preserve native recall action
- Selected-actor movement/S/Esc cancellation emits verified native return-cancel event and clears only action 1
- Five existing branch redirects retained; an additional native cancel-event header verified
- Existing quickcast/combat/camera/HUD controls and 60-second guard retained

## 0.15.0 — 2026-10-01

- Q/W/R physical press quickcasts using native skill targeting and validation
- Shift plus held ability previews nominal cast range/aim only; release never casts
- Read-only scalar effect snapshots from native move and attack consumers; 250 ms freshness gate
- Native Attack CALL observer with full fingerprint/header/branch verification and relay forwarding tests
- Rejected casts consumed once; no cooldown/animation queue and no AI takeover on skill rejection
- Targeted skill cursor hits include allies; native per-skill eligibility decides target
- 0.14 camera/combat/HUD controls and 60-second guard retained

## 0.14.0 — 2026-10-01

- Contextual right-click attacks with fresh own-team-visible target snapshots
- A then left-click attack-move; clicked-point priority with 120-world-unit acquisition around click or actor
- Native attack input validation; rejected attacks fall back to native chase; no direct damage writes
- Centered hold-Tab native scoreboard; world command clicks through scoreboard only
- Spectator detail panels hidden during READY/control; visibility and scoreboard layout restored on release
- Client-thread WM_MOUSEWHEEL observer; native zoom step 0.25, limits 0.5-3.0
- Startup, pacing and 60-second guard unchanged

## 0.13.0 — 2026-10-01

- Native Follow uses selected team/lane instead of SDK player ID
- Held Space keeps following unless explicit minimap or captured drag interrupts
- Middle drag captures only on battlefield start and continues across HUD
- Free camera edge-scroll uses 12px outer window bands including HUD
- Hold Tab shows native team info; release/focus loss hides; previous visibility restored on release
- Normal InGame spectator keyboard interception scoped to bound client/view/session; native mouse and default zoom keys retained
- Pacing unchanged; full skill/gold HUD and manual purchases remain future work

## 0.12.0 — 2026-10-01

- (no notes recorded)

## 0.11.0 — 2026-10-01

- (no notes recorded)

## 0.10.0 — 2026-10-01

- (no notes recorded)

## 0.9.0 — 2026-10-01

- (no notes recorded)

## 0.8.0 — 2026-10-01

- unconditional entrance counters and rejection samples
- actual AI/client/native call-stack samples
- Windows OS thread identity
- patch readback and client-side audit
- client-enforced startup deadline
- pause acknowledgement required for Ready
- status reports only acknowledged states

## 0.7.0 — 2026-10-01

- (no notes recorded)
