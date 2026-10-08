# Supplied design integration decisions

User sources: `design/Athlete HUD.zip`, `Athlete Cursor Set.zip`,
`Athlete Skill Previews.zip`. Each contains an HTML file with reusable inline SVG
and a local reference image. Extracted originals are retained in `design/hud`,
`design/cursor`, `design/previews`; they are not modified to conceal assumptions.
All three rendered without script errors using an isolated offline headless
browser; source and screenshots were inspected. Online fonts were blocked, so
that inspection uses fallback fonts. Screenshots are local research artifacts.

Confirmed by the user in this pass:

- Team availability belongs exclusively on Tab, replacing the mock's four allies.
- HUD panels use strictly neutral grayscale, replacing the warm/tinted tokens.
- Existing health/status meaning colors and the yellow accent remain applicable.
- The top stripe remains vanilla; use the bottom strip rather than changing the
  battlefield render allocation. Keep important numbers/icons readable.

Adaptations needed before later HUD/cursor implementation:

- Six inventory slots, correct mod artwork/descriptions and the working purchase
  forecast/missing-gold/recall affordability behavior; the mock has three slots.
- Automatic skill unlocks at levels 1/3/5 and verified cast-count/cooldown state;
  generic skill-rank pips must not imply manual leveling.
- Start/pause/resume/return-to-AI controls and selection feedback; camera action
  maps to actual Y lock. The mock's screenshot button is not a verified feature.
- Preserve A's persistent aiming mode and Mouse 4/backtick toggle. The cursor
  demo holds A/backtick and mentions ordinary left-click attacks; those examples
  do not replace the current right-click and normal-cast command rules.
- Cursor dispatch also needs ability-aiming and invalid-target states. The six
  shown cursor examples do not specify those states completely.
- Preview samples use illustrative League ranges, widths and enemy hit markers.
  Apply their visual grammar to real game data, without adding unsupported
  charging/vector mechanics or presenting speculative future hits as confirmed.

0.37 implements the supported preview grammar and cursor-aim preservation only.
Full HUD and cursor work must follow the project requirement for an explicitly
approved next implementation step.

0.39 implements the approved cursor integration from these SVGs, with strictly
neutral ink/light, semantic signal colors, persistent A mode, the existing
Mouse 4/backtick toggle and additional skill-validity states. It also adds saved
24–64 px size control and the supplied click-marker motion. The combat HUD has
not been replaced. See [cursor notes](cursor-selection-pass-39.md); native
interaction/rendering await the user's game test.

0.40 shortens supplied click motion to 250 ms and retains only the newest
accepted marker. Skill HUD hover shows known reach or self-centered area without
arming a cast; normal-cast geometry has priority. A true current-sprite outline
remains planned, pending verified renderer frame/transform access. See
[hover feedback notes](hover-feedback-pass-40.md).

0.41 implements the supplied HUD anatomy, with user-approved larger 80 px skill
tiles and strictly neutral panels. Six inventory slots move to the right;
purchase progress uses the left-side space freed by omitting ally portraits.
The existing top stripe and minimap remain untouched. Tooltips retain real
colored descriptions; XP/manual ranks, screenshot capture and unverified recall
channel durations from the mock are not invented. See
[implementation notes](hud-selection-pass-41.md) and [native test](../../local/testing/TESTING-41.md).
