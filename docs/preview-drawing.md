# What the game can draw: a sheet for skill-preview design

Date: 2026-10-09. For designing skill previews (and other battlefield
overlays) that the mod can actually build. Every preview is drawn fresh each
frame by the mod with the calls below, in screen pixels, through the
stable SDK (`sdk/mod-api-stable`, `StableClient::draw_*`). There is no
other way to put pixels on the battlefield.

## The calls

| Call | What it draws | Rotation | Size | Transparency |
|---|---|---|---|---|
| `draw_line` | A solid straight strip from A to B, any width | Any angle | Any length and width | Yes (colour alpha) |
| `draw_circle` | A filled disc | n/a | Any radius | Yes |
| `draw_rect` | A filled box, optional rounded corners | Upright only | Any | Yes |
| `draw_svg` | An SVG file stretched into a box, tinted by one colour | Upright only | Stretched to any box | Yes (tint alpha) |
| `draw_sprite` | A PNG at its own pixel size | Any angle, pivot, flip | Original size only | From the PNG |
| `draw_text` | Text in a box | Upright only | Font size | Yes |

Each call also takes a **z** (draw order); higher is drawn on top.

Already used by this mod and seen in game: `draw_line` (all current
previews, including alpha), `draw_circle` (cursor dot), `draw_rect`
(diagnostic bar). **Confirmed in game by the 0.75.0 drawing test**: lines
up to 64 px wide have square ends and an even fill at any angle; 3 px
strips fill a shape without visible seams; two crossing 25% strips darken
only mildly; a filled disc sits exactly inside a segmented rim (the game
camera has the same scale on both axes). **Not yet tried in game**:
`draw_svg` and `draw_sprite` (how the SVG tint mixes with its own colours,
sprite filtering).

## What each preview piece can be built from

| Piece | Good fits | Notes |
|---|---|---|
| Area circle | `draw_circle` fill; stacked circles for a soft edge; ring from short `draw_line`s; SVG decal stretched to the radius | The camera may squash circles slightly into ellipses; `draw_circle` is always round, an SVG can stretch to the ellipse. |
| Corridor (skillshot, dash) | One wide `draw_line` for the body; thinner lines for borders; rotated `draw_sprite` for the tip arrow and chevrons | No gradient or texture along its length; a taper must be built from a few strips. |
| Rotated rectangle | One wide `draw_line` (its length is the rectangle's length) | Same limits as a corridor. |
| Cone | Outline from lines; chevrons or hatch inside | A solid cone needs overlapping strips, which darken where they overlap (see below). |
| Range ring | Short `draw_line` segments (dashed or solid); or an SVG ring stretched to the range | A thin ring is cheap; an SVG ring allows texture. |
| Movement line | `draw_line` plus a rotated arrow `draw_sprite` | |
| Target brackets / reticle | Lines, or a `draw_sprite` icon at a fixed size | |

## Limits to design around

- **No gradients or stretched textures on rotated shapes.** Only upright
  SVGs stretch, and only upright boxes and circles fill.
- **Overlapping see-through shapes get darker where they overlap.** There
  are no blend modes or masks, so stacked fills (and wide-line fans) show
  seams. Prefer one fill plus borders, or non-overlapping strips.
- **No holes.** A filled ring is drawn as many short segments, not one shape.
- **Images do not scale.** A PNG detail stays the same pixel size at every
  camera zoom (good for icons, arrowheads and chevrons; bad for anything
  that must match a world size).
- **Animation is free.** Every frame is drawn fresh, so pulses, a fill that
  grows over the cast time or marching chevrons only need a rule, not new
  calls.
- **Cost.** One fill call is far cheaper than today's hatching (hundreds of
  short lines for a large area). A design with a few large fills plus
  borders will be cheaper than the current one.

## How a design becomes a preview

The skill decoders (`native_preview.rs`) only produce **shapes and sizes**:
circle, corridor, rectangle, cone, movement, each with a placement
(around the caster, at the aim point, where a projectile ends). How each
piece looks is separate (`skill_preview.rs`, `Drawing`). The plan is for
that look to become a list of **layers per piece** in a style file (fill,
border, glow, hatch, chevrons, tip image, SVG decal), so a design is
composed from layers and edited without code. A layer the design needs
that does not exist yet is a small code addition; after that it is
settings.

To keep the design and the game identical, the design page
(kept outside git) can be limited to the same calls: strips,
discs, upright boxes, upright stretched SVGs and fixed-size rotated images.
