# Endfield HUD HTML proposal — design QA

Date: 2026-10-07. Final result: **passed** (HTML proposal only).

## Source visual truth and scope

Authoritative source: `C:/LTTool/endfield_ui_lab/ui/index.html`, live component
styles in `ui/src/css/tokens.css`, `components.css`, `motion.css` and SVG library.
Source capture: `D:/LTTM2/LTDirectControl/research/endfield-audit-tooltip.png`
and `endfield-audit-slider-hover.png`, captured during the preceding investigation
at 1400×1000, device scale 1. Source files were not modified.

Approved layout inherited from `design/hud/review.html`; the original preview,
`review-assets.js` and original mockup `index.html` remained byte-identical.
The new proposal is `design/hud/review-endfield.html`; comparison is
`design/hud/compare-endfield.html`. No native mod code/build/install changed.

## Evidence and normalization

Screenshots: `D:/LTTM2/LTDirectControl/research/endfield-preview/`.

- Full comparison: `comparison-playing.png`, `comparison-menu.png`,
  `comparison-before.png`, `comparison-item.png`, `comparison-unknown.png`,
  `comparison-tab.png`, `comparison-low.png`, `comparison-dead.png`,
  `comparison-recall.png`.
- Focused source/new comparison: `source-new-tooltip.png` (1106×245).
  Source panel 529×168, proposed panel 529×173, both CSS pixel density 1.
  No resampling. Five pixels of height difference come from actual description
  wrapping/header content; this is an intentional HUD adaptation.
- Focused menu: `new-menu.png` (328×328); descriptive panel: `new-tooltip.png`.
- Chinese wrapping: `comparison-chinese.png`.
- Smaller browser: `comparison-1401.png`, viewport 1401×790.
- Primary viewport 1920×1080, device scale 1. Full-page screenshots can exceed
  viewport height. Side-by-side stages scale equally from 1920×1080; the lower
  proposed HUD view stays at 1:1 CSS pixel size with horizontal scrolling.

## Fidelity review

**Typography:** Manrope and Noto Sans TC are served locally and checked loaded.
Lab sizes/weights applied to descriptive text: title 27/500, body 20/500 with
1.25 line height. Ordinary numerical HUD text uses Manrope, not display Archivo.
KDA/CS 24/650 and HP 18/650 retain priority. Chinese sample wraps without clipping.

**Spacing/layout:** Existing group placement and 80×80 skills are retained.
Strip is 56px, matching current gameplay layout; old HTML retains its 50px strip.
No team portraits outside Tab. Menu adapts the lab's full-page controls to a
328px popover, with 44px action hit targets and no full settings panel. Slider
retains 6px track, 50×18 capsule, trailing numeric value and enlarged hit area.
Capsule endpoints are constrained inside the compact track. Tooltip uses the
source 529px width, 8px radius, divider, padding rhythm and optional slots; icon
and key/cooldown/range are intentional game-context additions.

**Color/tokens:** Source warm grays, yellow selection, lime HP, cyan values and
orange physical ratios are used. Bottom strip is flat. Descriptive overlay uses
source 85% background, blur 14px and saturation .78. Native rendering support for
that blur has not been established. Low-health gradient is a battlefield-only
edge effect, separate from the flat strip and excluded from minimap/top UI.

**Images/icons:** Original native skill/item/portrait artwork reused with pixel
rendering. Menu and general controls reuse the lab's Lucide source SVGs, shared
26px box and stroke treatment, with source licenses. No image generation or
handmade substitutes for the lab's supplied icons. Original minimap artwork and
muted corner asset preserved. Blurred screenshot spectator panels are review
background masks, not a proposed gameplay element.

**Copy/content:** Champion identity is tooltip-only; selection is outline-only,
as requested. HUD remains icon-led. Descriptive tooltip has no redundant
telemetry headings/slot labels. Unknown purchase has one compact status, no
reserved empty art or blank description block. Short labels are confined to
the more/settings popover. Prices/skills/visibility are explicitly sample data;
this preview makes no claim to repair the game data or actual vision.

## Interaction verification and comparison history

Browser checked nine alternate states plus playing; slider value changes and
reset, persistent vision selection, low-health toggle, Chinese sample and
smaller viewport. Console/runtime errors and missing required resources: none.
An optional favicon 404 from the unchanged old page is excluded from required
resource failures. Reduced-motion styles disable transitions/animations.

Initial capture caught entry animations before completion, making text appear
faint. Evidence was recaptured after fonts and finite animations settled. This
was a capture-method correction, not accepted low-contrast styling. Initial
cropped death/Tab views concealed upper content: those states now automatically
show the full battlefield. Capsule endpoints were constrained before final QA.

No actionable P0/P1/P2 visual findings remain in the reviewed proposal. The
user's aesthetic approval and native in-game typography/motion remain untested.
No automated result is presented as native game rendering verification.

## Implementation checklist after user approval

1. Preserve the approved layout and use the final lab component typography and
   behavior consistently across existing/new controls.
2. Resolve native item catalogue, vision restoration and structure sprite-pivot
   issues using the investigation evidence; HTML does not resolve them.
3. Establish native font/blur support and review actual in-game text thickness,
   colors, readability, motion and scale with the user's recorded test.
