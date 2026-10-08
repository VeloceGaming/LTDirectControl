# HUD, motion and screen effects — 0.44.0

The user approved completing the reviewed HTML HUD, closing the reported gap,
and adding the optional battlefield-only low-health effect after confirming the
0.43 minimap works. This pass changes presentation; the verified native map
operands, input buffering, steering and skill validators remain unchanged.

## Layout and fidelity

The warm palette comes from `design/hud/review.html`: base #1c1a18, edge
#4b4a49, yellow #fdee00, lime HP #b3d543, warning #ff642e. KDA/CS use bold 22 px
figures; level 24 px, HP 16 px, skill cooldowns 27 px. Descriptions use regular
15 px with bold 20 px headers. The game font set preserves multilingual support;
it does not supply the exact Manrope/Archivo browser font families.

Skills remain 80×80 at x832/y942 with 8 px gaps. Six 36 px item slots start at
x1150, followed by the next item, current/needed gold and recall. Branching costs
remain explicit in the existing purchase tooltip. No shop, XP, recall duration
or manual skill ranks are invented. Next purchase uses a dim dashed frame until
affordable; native purchase data and modded art remain authoritative.

The wide battlefield ends at y1024; the old strip began at y1030. The background
now starts at y1024 and has height 56, while controls keep the final 50 px band.
The right edge remains x1560 beside the confirmed enlarged minimap.

Selection is a transparent 456×80 row: five 72 px cards with outline-only
selection and a 48 px start button. The host's face-icon helper supplies fitted
portraits; centering and motion preserve its aspect ratio. Names are tooltips.
AI handoff is in the more menu and keeps Ctrl+End. Team availability stays on
Tab, with a clickable Tab toggle beside the existing figures.

## Presentation timing

Client-side, bounded easing drives 100 ms hover lifts, 90 ms press settling,
100 ms selection border changes, 100 ms tooltip fade/rise and 133 ms menu/row
entrance. HP fill eases over 200 ms, with a slower damage trail. Actual health
and cooldown text remains immediate. Cooldown shading uses observed remaining
ticks and a per-cycle observed maximum; a mid-cycle first observation is a
limited visual denominator, not a claimed total cooldown.

HUD tile hover uses rest geometry so a visual lift cannot drop its own preview.
The HUD combat blocker covers that lift. Motion does not delay or queue commands.
Individual top-level property caching avoids repeatedly rewriting unchanged
source, visibility or geometry when another field is animated. Rich-text colors
keep their stat type while fading; artwork uses permanent image nodes.

The browser's review controls and sample timings are not copied as in-game
settings. Browser reduced-motion handling remains a review feature; native
motion currently uses the short approved timings.

## Effects and ownership

Low-health tint is default-off, toggled by the skull button in the more menu and
stored as `low_health_effect` in the existing portable `controls.json`, preserving
cursor size and other preferences. It uses a static muted red edge fade below
25% HP, interpolated over approximately 180 ms. At most 64 edge bands are drawn
only while needed. Death disables that tint and adds 40% black dimming to the
existing native grayscale effect, matching the HTML's battlefield brightness.

Both overlays are restricted to the live viewport and subtract the complete
minimap surround. They draw below custom HUD elements; the native top bar lies
outside the viewport. Missing state, release or scene exit clears effect state.
The simulation, game executable and battlefield allocation are unchanged.

## Validation

205 probe tests and 18 core tests, strict Clippy and formatting, release build,
seven profile refusal tests and 19 migration tests pass. The 0.6.3 identity,
55 original anchors, 13 layout guards, 54 minimap operands/source constants and
146 receipt anchors are verified. Package ABI, asset fingerprints and ZIP
contents are checked before installation. Layout exports at `research/hud-layout-44`
show no viewport overflow in alive/dead/menu/selection states; they approximate
fonts, portraits and dynamic styles and do not verify native rendering.

The user tests gameplay and appearance with `TESTING-44.md`. Tower sprite fit,
true sprite outlines and other deferred work remain separate.
