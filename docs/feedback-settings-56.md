# Hover feedback settings and vision hover (0.56.0)

Correction in **0.56.1**: the debug option gates the entire ground target-marker
draw, including the selected attack target. Keeping that marker independent in
0.56.0 did not match the user's intent. Orders, picking, sprite outlines and
skill/range previews remain independent of this debug presentation setting.
The paragraphs below describe the original 0.56.0 implementation.

The user confirmed all unit types show proper outlines in 0.55.3, then approved
keeping the old hover markers as a debug tool and making outlines optional.

The declarative `settings::OPTIONS` catalogue now declares `hover_outline`
(Interface / Battlefield feedback, default 1) and `selection_debug` (Interface /
Debug, default 0). The existing row renderer, scrolling, draft Apply/Cancel,
page restore and controls.json persistence handle both. Older settings files
use the declared defaults; no saved preferences are overwritten during install.

Outline suppression happens only when publishing the native outline target.
The shared hover picker still runs, so toggling the outline leaves cursor,
debug markers, eligibility and single-target priority intact. F10 comparison is
inactive while outlines are disabled. The renderer still performs normal game
drawing and does no extra outline passes without a published target.

The debug option gates only the hover member in `MovementTest::draw_targets`.
The attack-order member is untouched. Existing circles and construction corner
markers are retained as tuning aids; this pass does not change collision areas
or claim the elliptical ground marker is the exact rectangular hit envelope.

Vision controls previously animated the selected indicator whenever the pointer
entered the full group. They now compute the pointed segment from the resting
button rectangles in scaled strip coordinates, excluding the gaps. Each button
has its own non-interactive hover layer and animation; the selected indicator
retains its position and colour independently. A selected hovered button uses
the darker selected hover shade, while other hovered buttons use the neutral
hover shade. Tooltips and click handlers retain their existing hit rectangles.

Automated verification covers persistence/default migration, the extended
Interface layout and row capacity, scaled vision hits/gaps/hidden state, native
fingerprints, compiler lints, DLL entry and packaged files. Native UI appearance
is pending the user's test; see `TESTING-56.md`.
