# Supplied HUD implementation — 0.41.0

The user approved implementation of the supplied HTML design, with the skill
area larger than its mockup, alongside three smaller fixes: invisible cursor
slider, insufficient minion vertical picking and undersized structures.

## Layout and priorities

The supplied design's anatomy is implemented in native UI coordinates at 1080p:
separate 80×80 skill tiles with 8 px gaps, centered over a 360×24 health bar in
a flat 50 px neutral-grey strip. There is no solid combat panel behind the tiles.
Health and skills have priority. KDA/CS use larger glyphs and 22 px numbers;
deaths retain their warning color. Inventory has six 38 px slots.

The mock's four ally portraits are omitted as agreed. This leaves room for gold,
next automatic purchase and missing gold on the left, while inventory and recall
occupy the right. The strip stops before the vanilla minimap. The top stripe,
minimap placement and battlefield rendering allocation are unchanged. Small
left controls operate pause/resume, camera lock and the more menu; the mock's
screenshot icon is not implemented as an invented screenshot feature.

Eight reusable SVG glyphs are rasterized directly from design/hud/index.html,
with neutral ink instead of tinted black. Their source and output hashes are
recorded in research/hud-glyphs-0.41.0.json. Existing skill and item image nodes
remain permanent. Artwork loading, registered item catalogue, purchase rules,
colored descriptions, death banner and hover preview behavior are preserved.
Tooltips are positioned above the hovered group rather than always above the
center. R readiness uses a yellow edge; unavailable skills keep their shade.

Health ticks use known maximum HP, normally one division per 100 HP, aggregating
at very high values to keep at most 32 nodes. Unknown HP does not produce ticks.
XP, arbitrary skill ranks, invented tooltip formulas and a speculative recall
channel duration from the mock are not introduced. Actual automatic unlocks
and native cooldown/cast-count readings remain authoritative.

## Slider and selection

The native slider already reports a ratio; the user screenshot shows a changed
size but no visible track. Its style resembles vanilla slider declarations, so
the exact internal drawing/z cause is not established. Explicit color children
now draw the track, ratio fill and endpoint-clamped handle above the menu.
They ignore events, leaving native slider input in charge. Save/reset behavior
is unchanged. Vanilla slider examples are in research/slider-styles-41.json.

Minion bounds gain 5 world units upward and change their lower pad from 2 to 5,
without widening. Structures with a recognized tower/nexus profile use its full
pixel dimensions instead of the half-scale champion/monster baseline; native
towers without a readable profile use a 48×80 body. Tower bounds add a 6-unit
upper/lateral margin and 12-unit lower pad. Resource-path names are normalized.
Known nexus names also receive structure sizing if the tower flag is absent.
The native tower flag, targetability and visibility filtering remain intact.
Static full-body sizing is a conservative correction to the reported miss,
not a verified current-frame pivot measurement. Combat collision is untouched.
Minion and structure regressions verify vertical coverage across zoom, bounded
width, champion priority and champion-only filtering.

## Verification and remaining limits

199 probe + 18 core tests pass, along with both Clippy checks, formatting and
release compilation. The existing native hook bytes remain unchanged and are
checked against the exact supported 0.6.2 executable.

research/hud-layout-41 contains exported native templates and browser-rendered
alive/dead/menu previews. The render reads the actual template geometry but
approximates native fonts and uses sample artwork/values. It is a layout check,
not evidence that the game's renderer accepted every property. No native
gameplay or live rendering is claimed. The user checks those with TESTING-41.md.

True sprite outlines remain planned with the prerequisite documented in
hover-feedback-pass-40.md. No new renderer hook is added in this pass.
