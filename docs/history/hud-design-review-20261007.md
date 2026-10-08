# HUD and champion-selection HTML review

The user approved an HTML review before changing the installed HUD, and requested
button and hover motion. This pass changes design files and inspection helpers
only. No Rust files, installed mod files, release version, or tower picking were
changed. The supplied `design/hud/index.html` remains intact.

## Source and approved constraints

`design/hud/review.html` uses the source palette: warm base `#1c1a18`, row
`#3a3837`, edge `#4b4a49`, yellow `#fdee00`, lime HP `#b3d543`, cyan effects
`#53b8e4`, and orange warning/death values `#ff642e`. The user's latest color
instruction supersedes the previous strictly neutral-grey requirement.

At 1920 × 1080, skills are three 80 × 80 tiles, with 8 px gaps, at x832/y942.
They end 8 px above a flat 50 px strip. HP is 360 × 24, centered beneath them.
KDA/CS remain horizontal and use 22 px figures. Six 36 px inventory slots, the
next-purchase tile, current/needed gold, and recall fit in a 400 px right group
with 4 px gaps; icon and text sizes remain unchanged. The strip ends at x1560,
meeting the revised minimap surround. Team
availability appears only on Tab. There is no invented XP or skill-rank display.

The pre-match row replaces the skills until play starts: five 72 px champion
cards, real artwork with close upper-body portrait crops, a yellow border
on the selected card, and a distinct 48 px play button. Names are tooltip-only.
The game implementation should use its existing native face-icon function;
the preview's 24 px portrait crops are design assets, not a new native algorithm.

## Motion

- Hover: brighter edge/background and 1–2 px lift, approximately 100 ms.
- Press: slight compression and downward settling, 90–100 ms.
- Selection: outline changes in 100 ms; the yellow selection remains steady.
  The user requested outline only after reviewing the HTML, so the check badge
  and bottom selection rail were removed.
- Tooltip: 100 ms fade with a 4 px rise; dismissal is immediate.
- Panel entrance: approximately 133 ms.
- Reduced motion disables animations/transitions and demo countdown updates.

The Motion review toggle also freezes sample timers so states can be inspected.
It is a review control, not a proposed additional setting in the game.

## Verification and limits

`tools/check_hud_review.cjs` validates inline JavaScript, assets, palette, and
spacing. `tools/inspect_hud_review.cjs` uses an isolated local browser, explicitly
authorized after in-app automation failed. It checks selection/start/handoff,
hover motion, tooltips, missing-gold/affordability states, six slots, menu slider,
Tab availability, battlefield-only grayscale, reduced motion, and 1280 px scaling.
Screenshots and the check report are in `research/hud-review`.

The supplied and revised screenshots are captured at the same 1920 px stage
width, using the same offline system-font fallback. `comparison.png` presents
both. The design loads the supplied Google font families when available; native
font matching still needs investigation before implementing the actual HUD.

Tooltip names/descriptions/numbers, item costs, and recall duration are marked
sample data. Background blur masks spectator panels in a static screenshot;
it does not demonstrate native removal or battlefield rendering. This browser
verification is not a native game test.

Tower selection remains a separate follow-up: verify rendered sprite scale and
pivot, fit the click box to that sprite, and decouple the ground ring from the
sprite's vertical height. The user screenshot shows the ring, not the click box.

## Minimap frame follow-up

The user authorized a slimmer frame with small, muted corner decoration for
HTML review. The native 0.6.3 renderer separately emits a 360 × 360 surround at
x1561/y720, followed by the 320 × 320 map at x1581/y740. After reviewing the
empty margin, the user approved using that footprint for a larger map. The
prototype uses a 360 × 360 frame at x1560/y720 (aligned exactly with the screen
edge), containing a 352 × 352 map at x1564/y724: 4 px padding, flat warm base,
a 1 px edge, 3 px corner radius, and four roughly 12 px stepped
corner strokes in muted warm grey. There is no yellow accent or animation.
The reusable corner paths live in the inline SVG in `design/hud/review.html`;
`design/hud/minimap-frame.svg` also provides the complete frame with a transparent
352 × 352 interior for later implementation. The source map crop is scaled by
1.1, so all reference markers and the camera rectangle scale together. The
strip ends at x1560 and the item/recall group ends at x1550; there is no extra
terrain margin and no placeholder terrain fill.

Native implementation must resize the original surround/map and update markers,
camera rectangle, routing and input geometry together. The mod's destination,
route and click-marker helpers share `CameraFrame.minimap` through
`project_minimap`/`unproject_minimap`. The native camera navigation uses its own
hardcoded origin and 320 px scale in function RVA 0x7b37f0; renderer RVA
0x231f310 emits the map image with a 20 px inset and 320 px dimensions near
0x232355d–0x23235c2. Updating only the mod rectangle would leave native left-click
camera navigation mismatched. These are inspection findings, not an implemented
native resize. No native code or installed files changed for this design review.
