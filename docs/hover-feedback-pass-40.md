# Hover feedback pass — 0.40.0

## User evidence and changes

The 0.39 test found champion picking nearly correct but often missed heads.
The code uses a stable idle/walk body with capped combat-pose growth. A bounded
upper pad now adds 20% of that body's height, clamped to 6–12 world units.
Width, feet, monster/minion profiles, combat collision and ground-ring dimensions
stay unchanged. This is a conservative response to the observed miss, not proof
of an exact native sprite-pivot or animation-offset cause.

Accepted clicks replace the previous marker. Contraction retains the supplied
167 ms easing; fade runs from 100 to 250 ms, with expiry at 250 ms. Invalid HUD
clicks do not replace valid feedback. Marker expiry does not clear the order or
minimap path.

HUD icon rectangles now feed a separate visual hover slot. It never enters
pending/dispatched command state, reserves clicks or changes the targeting cursor.
An armed normal-cast preview wins. Live range and the existing effect geometry
readers supply reach or a caster-centered footprint; no previous battlefield aim
or target is reused. Directional shapes still need an actual battlefield aim,
so HUD hover shows their known reach only. Unavailable skills render grey.
Death, control/focus loss, stale data and leaving the icon suppress the preview.

## Outline feasibility review

This was a read-only code, SDK and static-binary/asset review. No game execution
or live memory mutation was used for it.

- The SDK's draw_sprite takes a texture, UV, pivot and transforms. It does not
  identify a live entity's current animation frame or expose a per-unit outline.
- Our verified shader site at RVA 0x1fd6412 calls the battlefield-to-UI composite
  renderer at 0x1c8f60. It supports death grayscale for the whole battlefield,
  not an isolated hovered entity's silhouette.
- Historical debug assembly contains EntityView animation/transform fields.
  Extracted hypotheses are in research/outline-view-layouts-40.json. They are
  not validated offsets for game 0.6.2 and must not be used as such.
- Scanning the inspected asset bundle yielded no loose shader entries under
  the searched prefix (research/outline-shader-assets-40.json). That does not
  establish whether the engine internally supports outlines.
- Champion face-position settings were inspected, including enabled overrides.
  Their native pivot/current-frame conversion was not verified, so they are not
  treated as authoritative world-space picking bounds.

A true outline remains planned. Its next prerequisite is a verified association
between an entity ID and its current rendered texture/frame/transform. Until
that is available, this pass retains the existing ground highlight rather than
claiming a larger ring is a sprite outline. No new native hook is added here.

## Verification

197 probe + 18 core tests pass. New regressions cover head-only bounds and
unchanged rings across zoom, newest-click replacement and HUD rejection,
visual-only hover/normal-cast priority and freshness/death/focus gates, and
range/self-area geometry without an invented aim. Both Clippy checks, formatting
and release compilation pass. Executable anchors and packaged fingerprints are
verified separately. Native fit and rendering await the user's game test.
