# Tooltip overflow and unit click feedback — 0.69.2

The user confirmed Ghost and Harpy native descriptions worked in 0.69.1,
then reported clipping in longer descriptions and distracting cursor click
animations on actual targets. Both fixes were explicitly authorized.

## Tooltips

`tooltip_layout.rs` owns cached wrapping, adaptive card dimensions and the
scroll offset. The normal width is 529 px. Cards grow upward, keeping their
usual bottom anchor, and try widths of 640 then 760 px when the text would
extend above the battlefield's safe top (66 px in 1920×1080 coordinates).
Text still exceeding that space gets a whole-line viewport and slim scrollbar.
The header stays fixed. Titles retain 27 px text and 32 px line spacing;
descriptions retain 20 px text with explicit 28 px line spacing. Compact status
cards use their existing 20 px title and explicit 26 px line spacing.

Shared wrapping counts inline images instead of treating them as zero width.
The viewport carries inherited rich-text colors into every line, preserves
icons and blank lines, resets on a different description, and reaches the final
line. Moving from a HUD tile into its card keeps it open. The existing wheel
observer routes input over an overflowing card or its tile before camera zoom;
shop and settings retain their routing priority. No new native hook is added.

## Click feedback

The movement command path previously appended a ground marker for every
accepted order, including `Attack(id)`. It now checks for a body hit at click
time, with the existing picker and champion-only filter. Fresh all-unit hover
data includes allies; the enemy hit used for an actual attack also suppresses
the marker. A target click clears an earlier ground marker. The actual order,
attack-click sprite pulse, and target selection are unchanged. Ground movement,
ground attack-move and minimap destination markers remain available.

## Verification limits

Regression tests cover long localized/icon descriptions, card size limits,
scroll-to-end, reset and color continuity; unit/structure click suppression,
retained attack orders/pulses, and ground/minimap marker expiry. Formatting,
Clippy, release/package/install checks are recorded with the build.
The user verifies native tooltip rendering and gameplay.
