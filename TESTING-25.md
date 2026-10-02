# Twenty-fifth test: champion body picking

Restart the game with Harbinger disabled. The working 0.24 package and its
recording are preserved. Start control normally.

1. **Body clicks:** hover a champion's head, shoulders, torso and feet. The
   gold ring should appear over a much more useful area. Right-click and direct
   A-click on those parts should attack/chase that same champion. Move away from
   the sprite: nearby empty ground must still accept movement/ground A-click.
2. **Zoom:** repeat a few head/shoulder clicks zoomed in and out. The picking
   area should scale with the champion on screen, rather than keep a fixed height.
3. **Crowds:** near minions or another champion, move between their bodies and
   watch which unit gets the hover ring before clicking. Check that minions
   remain easy to select. Champion-only should exclude them for right-click;
   A remains unrestricted. Hover/attack rings keep their existing foot-ring style.
4. **Targeted skills:** quickcast and Shift + skill / left-click should accept
   the same head/torso areas. Native ally/enemy, cooldown, range and CC conditions
   remain. If a mod champion is drafted, repeat a few body/zoom clicks on it.

Report any body part that still misses, or empty area that selects a champion,
with the champion name and zoom if known. A screenshot with cursor placement is
useful if a particular overlap chooses the wrong unit. Full prior regression
testing and drafting Exorcist specifically are unnecessary.

These areas are padded envelopes from normal standing/running poses. The SDK
does not expose the exact displayed animation frame or facing silhouette, so
transparent gaps, weapon swings and unusually displaced poses may differ.
Half-sheet map scaling and the body anchor are calibration choices; this test
checks their actual on-screen fit. Combat collision and attack ranges are not
changed. Unknown or unsupported sprite metadata uses a forgiving fallback.

Validation: 103 probe + 18 core tests pass, with all-target Clippy and formatting
checks. Tests cover upper-body picking at multiple zoom levels, overlap/core
versus margin ranking, visible body with offscreen feet, targeted quick/normal
cast, animation effect exclusion, visible layers/linked Aseprite cels, malformed
metadata and scoped asset paths. An explicit installed-asset test loads 96
profiles and verifies Harpy and CF Archangel metadata, recorded in
`research/picking-installed-0.25.0.log`. Actual visual fit awaits this test.
