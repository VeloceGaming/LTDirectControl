# Settings preview update (0.54.0)

The updated `design/hud/review-settings.html` uses compact checkbox rows for
binary On/Off settings. The native settings window now follows that pattern for
attack wind-down cancellation, camera lock, edge scrolling, camera dragging,
mouse-wheel zoom, low-health effect, and minimap route display. The entire
56-pixel row is clickable. Existing saved values remain numeric 0/1, so the
visual change does not reset preferences. `Control::Toggle` is separate from
`Control::Choice`, making later settings additions explicit in the schema.

The remaining two-choice controls keep the segmented track. Hover changes the
unselected track from `#5b5b5b` toward `#7f7f7f`; the selected pill stays
`#eeecec` until the value changes. This matches the hover placement documented
in `C:/LTTool/endfield_ui_lab/ui/src/css/components.css`. The HTML preview's
local override that darkened the selected pill was corrected too.

Raised settings buttons now have a crisp stroke plus two translucent native
plates approximating the Endfield contact shadow. This covers key bindings,
dropdown fields, Close, footer actions, and conflict actions. The select menu
has its own shadow. Navigation and transparent hit surfaces remain flat, as in
the reference. The segmented pill has a close-fitting shadow and a white
top/bottom highlight.

The local browser QA checked compact rows on Combat, Camera, and Interface,
the correct segmented hover side, and action shadows. The 240 probe tests,
18 core tests, strict probe Clippy, release build, package fingerprints,
148 native anchors, ABI load check, and install fingerprints passed. Native
rendering and gameplay require the user's in-game test.
