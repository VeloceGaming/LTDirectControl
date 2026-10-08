# Approved Endfield HUD implementation — 0.45.0

Source: the user's `C:\LTTool\endfield_ui_lab`, reviewed through the separate
`design/hud/review-endfield.html` preview. Original HTML previews are preserved.

## Presentation

The native player HUD and session controls use static Manrope weights 500/650,
Noto Sans TC 500, and native language fallback sets. The mod packages the font
licenses and SVG icon license. Icons reuse the approved SVG paths rasterized to
white masks; native tint supplies the exact warm/accent colors.

The skill row remains three 80 px tiles. The bottom strip remains flat, 56 px high,
with the existing enlarged minimap and vanilla top bar. Health remains lime and team
availability remains on Tab. The more menu is 328×328 with 44 px control hit areas,
26 px optical glyphs, a 6 px slider track and a 50×18 capsule handle.

Tooltip cards use 529 px width, 27 px titles, 20 px body text and native stat colors.
Text is wrapped using packaged font advances with a shaping margin. Compact
unavailable/complete messages omit blank artwork and empty header space. Skill
header metadata shows declared base values only. Unknown item prices remain absent.
The native asset/UI API does not expose verified backdrop blur, so cards use
translucency without claiming parity with the HTML blur.

Hover/press use the lab's tonal curve, while selector movement uses its movement
curve. Tooltip motion is opacity-only with immediate dismissal. Buttons and skill
art remain at fixed positions/sizes, rather than blanket lift/shrink animation.

## Control and native vision

F11 replaces Ctrl+Home for start/resume. F12 replaces Ctrl+End for AI release.
The existing explicit start, ownership, pacing, buffering and gameplay policies
remain intact. F11/F12 are filtered from native spectator shortcut handling while
the mod owns the view.

The native renderer consumes the spectator-vision byte at camera configuration
value offset `0x4b` (UI owner `0x63`, accounting for its 0x18 header). Modes are
all=0, blue=1, red=2. Four exact read/compare anchors were verified and added to
both the Rust profile and patch-migration profile. Takeover snapshots the prior
byte, defaults to the controlled side, supports own/opponent/all, and restores
the saved byte on release. New sessions reset the mod's choice to own-team.

## Boundaries

No purchase-pointer guesses or construction-picking changes are included. An
unavailable purchase forecast is now clearly represented instead of an empty card.
Native rendering, font loading and vision behavior still require the user's game
test. Automated tests cover multilingual wrapping and side/mode mapping/session reset,
alongside the existing control tests. Package checks cover all fonts/icons/licenses,
exact executable/profile anchors, DLL ABI and ZIP contents.
