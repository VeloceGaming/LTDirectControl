# Investigation after 0.44.0 testing

Date: 2026-10-07. Scope: investigation and reference review only. No mod source,
build, installed files, or UI-lab files changed in this pass.

## Authoritative design source

`C:/LTTool/endfield_ui_lab` is the user's final style reference for existing and
new elements. The previously approved gameplay layout remains the starting
point: larger skills, bottom strip, expanded minimap, and team availability on
Tab. This reference is a component/style system, not a replacement combat-HUD
layout. Its full-page settings dimensions should not be copied into the match.

Read `README.txt`, `design-qa.md`, `ui/src/css/tokens.css`, `components.css`,
`motion.css`, and the relevant page and interaction sources. The current lab
uses Manrope for ordinary Latin/numbers, Noto Sans TC for Chinese, Archivo
Expanded for display identity, and Iosevka only for telemetry.

The in-app browser connection failed twice. A temporary local Edge browser,
previously authorized by the user, opened the original HTML without changing it.
Captures and inspection used a 1400x1000 viewport. Fonts were checked loaded on
the focused typography pass. No native game rendering was automated or claimed.

Review steps:

1. Token shelf: usable reference for colors, spacing and typography roles.
   Capture: `research/endfield-audit-initial.png`.
2. Component shelf: usable tooltip and state anatomy; the actual lab is the
   source, not older pre-font-change QA PNGs. Captures:
   `research/endfield-audit-components.png` and
   `research/endfield-audit-tooltip.png`.
3. Dark console: usable control hierarchy, capsule slider and selection states.
   Captures: `research/endfield-audit-console.png`,
   `research/endfield-audit-slider-hover.png`,
   `research/endfield-audit-selected.png`.
4. Mid-stage: usable floating-panel and tooltip composition, not an instruction
   to add its large identity plane or parallax to the combat HUD. Capture:
   `research/endfield-audit-stage.png`.

The selected-control check changed the first segmented control from its first
option to its second; aria-selected changed on both options. Screenshots prove
appearance, not native hit handling, accessibility compliance, or exact motion
timing. Motion durations/curves below come from the lab's source.

## Existing elements that need correction

| Area | Current implementation | Reference / required correction |
| --- | --- | --- |
| Typography | Native regular/bold sets; all HUD scalar labels use bold | Separate functional, numeric and descriptive roles; preserve important HUD readability |
| Descriptive tooltip | 440px wide; title 20px bold, body 15px regular; permanent icon slot | Lab sample 529x167, title 27px/500, body 20px/500 with 25px line height; optional slots and clear header/body division |
| Short tooltip | Fixed 310x56; raw champion key, anchored to entire selector | Localized compact content, position by hovered control; a name does not need a full descriptive panel |
| Empty purchase tooltip | Entire fallback sentence becomes title; blank art and body retained | Compact status message without empty descriptive-panel sections |
| Text layout | ASCII/non-ASCII width heuristic, title split at first newline | Structured content roles; determine wrapping from actual font metrics/native layout, accommodate long localized text |
| Slider | Yellow fill, 12x16 vertical handle, 16px input height | Light fill, horizontal capsule handle; lab sample 50x18, 6px track, fixed trailing value and 44px invisible hit height |
| Menu controls | Mixed padded glyphs, 40px buttons; main controls 32x36 | Consistent source icon silhouettes, stroke and optical size; minimum 44px action hit area where applicable |
| Motion | Shared cubic ease-out, 100ms hover, 90ms press, generic lift/shrink | Separate tonal and movement curves; weak controls vs card hover; selected and hovered remain distinct |
| Panel material | Tooltip alpha 240/255, 2px rounding, no blur | Lab descriptive tooltip alpha 85%, 8px rounding, subtle blur/shadow; native blur capability not established |

Important reference distinctions: light controls darken on hover; settings rows
themselves do not react. Card hover uses a bright outline and tooltip. Yellow
marks persistent selection or a meaningful primary action. Not every component
gets yellow fill, a rise, or a scale effect.

Lab timings: weak control press 83ms, card hover 100ms, segmented indicator
travel 167ms; descriptive tooltip enters in 117ms and disappears immediately.
Tonal curve is cubic-bezier(.4,.1,.5,1), movement curve is
cubic-bezier(.15,.8,.3,1). Use source component-specific variants, not one
global hover/press rule. Exact native line layout and animation remain to test.

The game bundle contains `ttf` assets and localized `font_set` JSONs, and SDK
labels/draw_text accept font asset paths. Current base Latin is Roboto, not
Manrope. There is a plausible mod-scoped font-set route without replacing the
game's shared fonts or introducing a browser overlay. Loading the lab fonts in
the native game, choosing fixed weights from variable fonts, fallback coverage,
and packaging licenses still require an implementation/test pass. Native
backdrop blur is a separate unverified capability and must not be silently
claimed equivalent to the browser reference.

## Purchase tracker failure

Evidence: `C:/Users/j9010/AppData/Local/LTDirectControl/probe.log`:

- Line 458: registered-item reader rejects the live catalogue with the combined
  error `AI bridge, registered item bounds, getter data or upgrade links unsupported`.
- Line 622: HUD falls back to 31 settings aliases.
- Line 623: planned build includes Radiant Yun Tal, Gluttonous Greaves and four
  other final items; several keys are missing, forecast is None.
- Lines 4695/4698: SDK catalogue count is 265, owned item is Noonquiver, but its
  metadata is also missing. Thus an actual native purchase happened; these logs
  establish tracker failure, not a failure of the game's purchase AI.

Read-only disassembly of the preserved exact 0.6.3 executable confirms the
expected AI bridge table header (size 0x78) and getter addresses, state+0x10 to
context/world, world+0x1b8 settings, and buying's vector access through
settings+0x30. This does not establish which runtime validation rejected the
reader, nor prove every dynamic item getter works.

Current reader is all-or-nothing. A rejected bridge, malformed entry/string,
duplicate key, or unresolved forward upgrade link can discard the catalogue.
`link_upgrades` returns None if any next-tier destination is absent. Current
logging combines these cases and only reports the first failure per match.

Recommended next implementation:

1. Add bounded, stage-specific rejection diagnostics, item index/key and counts.
2. Preserve build identity/ABI checks; do not weaken pointer/layout guards to
   force the reader through.
3. Compare the effective registered objects used by native buying and tooltips
   with the reader. Installed item mod is `riot_items_tfm2` 0.11.11, declaring
   game 0.6.3; native additions are not covered by its original settings JSON.
4. Correct the proven failing stage. Retain trustworthy entries for descriptions
   without inventing prices or claiming a forecast across unknown graph edges.
5. Check a complete 265-key catalogue, including owned and planned items, and
   compare predictions with an actual base purchase in the user's test.

## Vision controls and wrong-side view

The vanilla buttons are `ingame.speed_buttons.view_all`, `.view_blue`, and
`.view_red`; `TeamInfo` hides their parent. `spectator_key_is_owned` also blocks
nearly every keyboard event while controls are active. Vision isn't explicitly
set on takeover or included in the camera lease's saved 16 bytes (+0x18..+0x28).

Current 0.6.3 static evidence:

- UI update 0x280bba0 reads its config owner+0x63 to select Red=2, Blue=1, All=0.
  Relevant reads: 0x280bfe2, 0x280c054, 0x280c0c7.
- Viewer caller 0xcdcc9f passes the config value after the owner's +0x18 header.
- Renderer/minimap function 0x231f310 reads config value+0x4b at 0x231f8ae and
  compares/caches that field at 0x2320bcd / 0x2320f0a. The difference between
  owner+0x63 and value+0x4b is the +0x18 header, not two different settings.
- This setting is separate from the camera-follow side written at config+0x20.

Recommended: explicit own-team vision on takeover/new match; restore the prior
spectator setting on release; expose native All/Blue/Red display modes through
compact menu controls with a distinct selected state. Include original/effective
vision and controlled side in bounded transition logs. Fingerprint new field
consumers in the migration profile before using them.

An inherited spectator setting is a supported explanation, not a reproduced
wrong-side bug: the existing runtime log does not record the vision field.
Trace who writes it and whether native UI resets it in a particular mode before
declaring the issue fixed. Keep display vision separate from simulation-side
visibility and existing native targeting validation.

## Construction picking

`Body` stores width and height only. The extractor discards sprite origins/cel
offsets, and selection assumes an above-feet rectangle around the projected
simulation position. Structure sizing and extra padding do not establish the
actual draw pivot. Recognized towers use full 31x63 asset dimensions; unknown
structures use a generic 48x80 fallback. Name normalization only handles a path
basename and `#anim`, so actual runtime names also need confirmation.

The current construction problem cannot responsibly be called solved by making
that fallback larger again. Recommended diagnostic: report structure kind/name,
body-profile choice, simulation/projected position and effective selection
rectangle; compare those with native rendered sprite bounds at two zoom levels.
Then give structures the proven scale/pivot and modest padding. Picking and
hover visuals should share that geometry; champion/minion envelopes stay at
their tested sizes. The ground ring is not proof of the full body hitbox.

## Recommended next pass and limits

First implement targeted item/vision/structure diagnostics and fixes that the
evidence supports. In the same approved pass, apply the UI lab systematically to
existing and new components, beginning with typography, tooltips and the menu.
Keep skill, HP, inventory and minimap placement already approved by the user.
Do not treat this report as permission to redesign the entire match layout.

No source was edited or built and nothing was installed in this investigation.
New runtime diagnostics, native font rendering and corrected construction fit
require the user's explicit implementation go-ahead and subsequent game test.
