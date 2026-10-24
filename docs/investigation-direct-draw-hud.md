# Investigation: a HUD drawn directly (performance Stage 2 design, 2026-10-24)

Reading only. No code was changed and nothing was built. Every claim carries
one of the labels from `AGENTS.md`: **seen in game**, **read from the
code**, **guess**, **rejected**.

Question: Stage 1 of the performance overhaul (1.6.0-perf-dev1) cut the
mod's own work per frame by about 40 %. The A/B below shows the game itself
spends about 1.5 ms a frame laying out and drawing the mod's interface
nodes. Can the HUD be drawn with the SDK's direct draw calls instead, with
no change the player notices ("play identical": a one-pixel text shift is
allowed, no feature, timing or feel change), and what would it take?

## 1. The evidence

**Seen in game** (2026-10-24, game 0.6.5, mod 1.6.0-perf-dev1, Frame Lab
0.6.3, the same league match played four times, about 5 minutes each):

| Run | Mod windows | Vision | Champion | fps mean | Game's own time per frame (Frame Lab `other_ms_p50`) | Mod own-code share |
| --- | --- | --- | --- | --- | --- | --- |
| 14:38 | shown | full | (not logged) | 86.0 | 9.3 ms | 1.65 % |
| 14:50 | shown | full | harpy | 85.8 | 9.4 ms | 1.68 % |
| 15:38 | shown | own team | fireblade | 80.6 | 10.1 ms | 1.60 % |
| 15:06 | hidden (`perf_hide_mod_ui`) | own team | dynamo | 101.4 | 7.9 ms | 1.70 % |

With the windows hidden (every mod root set invisible; state, sounds,
input and battlefield drawing unchanged) the match ran 18-26 % faster and
was faster in every one of its 34 ten-second windows. The mod's own code
share did not move, so the difference is the game's work on the mod's
nodes: about 1.3-2.1 ms a frame, call it 1.5 ms. Champion and vision mode
do not explain it (the own-vision run was the slowest of the three normal
runs; the normal runs span 80.6-86 fps, the usual match-to-match spread).

**Read from the code**, what is in the tree while you play (node counts
from the templates, loops expanded):

| Window | Nodes shown in play | Of which | Shown when |
| --- | --- | --- | --- |
| HUD strip (`player_hud.rs`) | about 265 | 120 cooldown-denied band nodes (3 skills × 10 bands × 4 sides), 32 health ticks, 18 charge marks, 16 purchase-tile gap strips, 27 other colour boxes, 9+9 labels, 8+6 images, 3 shadows, death veils, recall bar, comets | always under control |
| State plate (`state_plate.rs`) | 5 | glyph image, key cap, count label | paused, dead, AI |
| Session strip (`session_ui.rs`) | about 50 | 10 `color_icon_button` runners (5 lanes, 5 swap tiles) with portrait images, 10 glyph buttons, vision track and indicator, rules, key cap, tooltip | always |
| Stats panels (`stats_ui.rs`) | about 40 | two frames, 2 × 7 value cells, portrait, health bar | own panel on request, target on selection |
| Screen effect (`screen_effect.rs`) | up to 34 bands | colour boxes | low health, death |
| Tab (`team_status.rs`) | about 140 | 10 rows × (portrait, 4 labels, 6 item tiles, shade, line) | while Tab is held |

So roughly 350-400 nodes on a normal frame, the HUD strip two thirds of
them, and most of those are plain colour boxes that exist only to be
tinted or moved.

## 2. What the direct draw calls can do (read from `sdk/mod-api-stable/src/client_ctx.rs` and `docs/preview-drawing.md`)

All calls work only inside `post_render` (`can_draw`), in a named map:
"UI" is screen space (1920 × 1080 logical, the input-delay readout and the
test banners already draw there), "Game" is the match world.

| Need of the HUD | Direct call | Verdict |
| --- | --- | --- |
| Filled box, optional rounded corners, alpha | `draw_rect` | yes, same as a `:color` node |
| Hairline, 1 px bar, tick | `draw_rect` or `draw_line` | yes |
| Disc | `draw_circle` | yes |
| Single-colour text in a box, left/centre/right, top/centre/bottom | `draw_text(font, size, color, align)` | yes; the mod's own fonts (`asset/lt_direct_control/font/numeric`, `.../medium`) already draw this way (input-delay readout, seen in game 1.5.x) |
| Text outline (`outline: 1` on hp_current, hp_max, the stats health text) | none | draw the text once more, offset, in the outline colour (as the input-delay shadow does): a one-pixel-class change, allowed |
| Several colours in one line (`<#rrggbbaa>` runs: Tab's K/D/A, tooltip bodies, shop) | none; no text-measure call in the SDK | the mod has the advance widths of its own fonts (`hud_style::advance`, `hud_font_metrics.json`), so runs of the mod's fonts can be placed by hand; game-font runs cannot |
| Wrapped multi-line text, `line_height`, scrolling with a clip mask (tooltips) | none (no clipping, no wrapping) | stays in the tree |
| Glyph images (`ef_*`, 32 × 32 PNG, drawn at 17-32 px with a tint) | `draw_sprite` draws a PNG **at its own size only** (uv sub-rect, rotation, pivot, flip, no scale); `draw_svg` scales into any box and tints, **not yet seen in game** | either ship one PNG per used size (`tools/generate_endfield_glyphs.cjs` already rasterises the SVG sources to 32 px; other sizes are one more line) or try `draw_svg` with the SVG sources as assets (guess: the game's asset loader may not take `.svg` files from a mod folder; the game's own `.ui` files use none) |
| Skill icons, item icons, tooltip art: sub-rects of the game's sprite sheets (`source` + `rect_tag`) | `draw_sprite` can take a uv sub-rect, but the mod does not know the sheet geometry: the game's `#data.sprite_sheet` files live inside its asset pack (offsets in `research/hud-assets-index.json`) and the SDK has no asset-read call; the mod reads only mod-pack sheets from disk (`hud_icons.rs`) | stay in the tree (about 12 image nodes in the strip, 2 in the tooltip) |
| Champion portraits (`ui_set_champion_icon`: the game's own face math from the idle sheet) | none | stay in the tree (session strip lanes and swap tiles, stats target, Tab rows) |
| Clicks and hover (session strip buttons through `ui_register_path_events`; HUD tiles and the shop button already by rectangle from the cursor) | none; draws receive no events | the mod's own hit testing from `keys.cursor`/`keys.left`, which the HUD strip already does for its tiles and shop button; `block_native_ui` keeps those rectangles from becoming battlefield orders |
| The `color_icon_button` runner's own look (hover or press shading drawn by the game, if any) | none | unknown: the mod animates its colours itself every frame, but whether the runner adds its own hover effect is not known; to check in game before the session strip moves |
| Draw order against the interface tree | each call takes `z`, higher on top; whether a direct draw at z 1000 sorts with tree nodes at z 1000, or all direct draws land above (or below) the whole tree, is **not known**; **read from the code**: `skill_preview::Drawing` clips its UI-map strokes against the HUD and minimap rectangles, which only makes sense if direct draws would otherwise show over the HUD (a strong inference, not a test) | to check in game with the probe build: a direct box at the shop's z under the shop window, and one at the HUD's z over the strip |
| Cost of a direct draw call | unknown: the mod's whole `post_render` (previews, rings, markers) is about 18 µs a frame of mod time; what the game then spends per queued command is not measured; the POV mod draws most of its HUD this way at 0.8 % own code (seen in game 2026-10-18, Frame Lab) | to measure with the probe build: N boxes and M texts a frame, N and M from a hidden setting, against the PERF lines and Frame Lab |

## 3. The design: a direct-drawn HUD with a thin tree

Not everything can leave the tree, and not everything needs to. The
cost is per node, so the plan moves the many cheap-to-draw nodes and keeps
the few that need the game's runners.

**Leaves the tree (drawn in `post_render` each frame):**

- HUD strip: the strip, edge and corners, the combat frame, the health bar
  (back, trail, fill, 32 ticks), every skill slot's box, shadow,
  background, shade, glow, drain edge, comet, 40 denied bands, 6 charge
  marks and fills, key cap, lock glyph; the inventory tiles' boxes,
  borders, backgrounds, unknown marks; the next-purchase tile, its 16 gap
  strips, progress bar and unavailable mark; coin, gold, purchase line,
  recall key and icon, recall bar; K/D/A, CS, level, HP, respawn,
  cooldown and uses labels; the death veils and the stats hairline (D1).
  Estimate: about 240 of 265 nodes.
- State plate: all 5.
- Session strip: the rules, vision track and indicator, the key cap, the
  button boxes, borders and glyphs, the swap stagger hairline, the tooltip
  box and its single-colour text. Estimate: about 35 of 50.
- Stats panels: frames, cells, value texts, health bar and text. About
  36 of 40.
- Screen effect: all bands (the mod already computes their rectangles).

**Stays in the tree:**

- Images from the game's sprite sheets: skill icons (3), item icons (6 +
  next), tooltip art (2).
- Champion portraits: session strip lanes and swap tiles (10), stats
  target (1), Tab rows (10).
- Tooltips (HUD, session strip, shop): wrapped, scrolled, rich text.
- The feedback banner's text if it ever carries colour runs (today a
  single colour: it can move).
- The shop and Settings windows, the emote wheel, Tab (Tab is open only
  while held; it can move later if the per-node cost is confirmed).

That leaves roughly 40-50 live nodes on a normal frame instead of
350-400. **Guess:** 85-90 % of the 1.5 ms, less what the direct draw
calls themselves cost, which the probe build measures first.

**How the code is organised** (keeps Stage 1's three-step model and the
clean-base rule):

1. *State*: unchanged. Each window computes plain values every frame, with
   every side effect (tweens, motion pass, chime, press and release, hover
   sounds, tooltip timers) exactly as now.
2. *Draw list*: a new pure step turns those values into a list of draw
   commands (`Rect`, `Line`, `Text`, `Sprite`, `Svg` with map, z, colour)
   in UI coordinates. It is a plain data structure, built from the same
   constants the templates use today (`SKILL_X`, `SKILL_STEP`, the label
   rectangles, the z bands 1000-1120).
3. *Submit*: `post_render` sends the list through `StableClient::draw_*`.
   The few tree nodes keep the `PropCache` path from Stage 1.

The HUD root's anchoring (bottom-centre of the game's `ingame` root) is
replaced by an explicit origin from `ui_node_rect("ingame")`, which the
screen effect already uses.

**Proving "play identical" without the old write records.** Stage 1's
identity tests compare interface writes with 1.5.3 records; a draw list
is a different thing and cannot be compared write for write. Two tests
replace them:

- An offline pixel comparison: `tools/check_hud_review.cjs` already renders
  the exported templates to PNG in a headless browser for layout review.
  Extend it (or add a sibling) to render a dumped draw list the same way,
  then compare the two images for the same frame states (alive, dead,
  cooling, hovered, purchase ready, paused) with a small tolerance. This
  is a layout test, not the game's renderer, as the tool itself says.
- Golden draw lists: a `fake_draw` recorder (like `fake_ui.rs`) records
  the command list per frame for the scripted sequences of
  `ui_identity.rs`; the records guard later refactors the way the write
  records do now.

Then the author's eye in game, with the PERF capture on, decides.

## 4. Plan: two builds

**2a, a probe build (small, one match of the author's time).** A hidden
setting `perf_draw_probe` (default off) that, on the battlefield, draws N
boxes, M single-colour texts in the mod's fonts, one `draw_svg` glyph and
one `draw_sprite` glyph at a fixed corner, plus one box at the shop's z
under the shop window and one at the HUD's z over the strip. It answers,
in one match and one PERF capture: the cost of a direct draw command, how
direct draws layer against tree nodes, whether `draw_svg` loads a mod SVG
asset, and how a sprite looks at its native 32 px beside a 32 px tree
image. Nothing of it is visible with the setting off.

**2b, the rewrite (one build, Opus, reviewed before install).** The
windows and pieces listed in section 3, the draw-list step and its two
tests, the hit testing for the session strip's buttons, glyph assets at
every used size (or SVGs if 2a says they load), `docs/modding.md` updated
("Add a HUD element" becomes: add it to the window's draw list; a node
only for sprite-sheet images, portraits and rich text), the HUD review
tool fed from the draw list. `perf_hide_mod_ui` keeps working (it then
also skips the draw lists), so the same A/B measures the result.

If 2a finds that direct draws land under the interface tree or cost as
much per command as a node, 2b changes shape (fewer commands: merge the
denied bands into one textured box per skill, for example) or is dropped;
the author decides with the numbers.

## 5. Risks and open points

- **Layering** (section 2): the one finding that can sink the design;
  measured first.
- **The button runner's own hover look** on the session strip: unknown;
  checked in 2a by hovering a strip button while a direct box sits
  beside it.
- **Glyph sizes**: `draw_sprite` cannot scale, so every size in use (17,
  18, 20, 22, 24, 26, 32 px) needs its own PNG unless SVG loads. More
  assets, no code risk.
- **Text metrics**: the game lays out `draw_text` inside the given box
  with the given alignment, as the labels do now; a one-pixel baseline
  difference between a `:label` node and `draw_text` of the same font is
  possible and allowed. Outlines become offset copies.
- **Hit testing**: the session strip's clicks today come from the game's
  event system; after the move they come from the mod's cursor reading.
  Double-click guards (`ui_click::once`) and the "press on, release on the
  same button" rule must be reproduced; the HUD strip's tiles already do
  this, so the pattern exists.
- **Resolution and UI scale**: the UI map is 1920 × 1080 logical; the tree
  anchors the HUD to the bottom centre of `ingame`. Using
  `ui_node_rect("ingame")` as the origin keeps the same placement at any
  window size (the screen effect does this today; seen in game).
- **Two systems side by side**: a handful of tree nodes remain inside a
  directly drawn HUD. Their z must sit in the same bands, and the
  layering answer from 2a decides whether that works at all.

## 6. What this means for later interface work on the mod

- A HUD element becomes a few lines in a draw list, drawn fresh every
  frame: no template, no node table, no write cache, no spawn, no rebuild
  on a background or language change. Animation is free (a rule per
  frame, as the skill previews do now).
- The price is that each element costs its draw call every frame whether
  or not it changed, and that text is single-colour per call, images come
  at their own pixel size (or as SVG, if 2a says so), and nothing clips,
  wraps or scrolls. Anything that needs those stays a tree node and goes
  through the Stage 1 typed-write path: tooltips, windows with text
  fields, portraits, sprite-sheet images.
- Design review tools change: the template renderer gives way to a draw-
  list renderer with pixel comparison, which is also the regression test.
- Two drawing paths coexist on purpose; `docs/modding.md` says which to
  use for what. A modder who wants a new window with buttons, scrolling
  or rich text keeps using templates; one who wants a readout, a bar, a
  marker or a glyph draws it.
