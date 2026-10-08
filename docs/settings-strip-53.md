# Settings window and strip pass (0.53.0)

## Why
The user reported that the in-game 0.52 settings window did not match the
approved `design/hud/review-settings.html`:

- a literal `\n` appeared in the navigation;
- the navigation boxes were outlined;
- section headings were missing;
- two-choice options were drawn as separate pills instead of one track;
- the dropdowns showed a text "v", overlapped the rows below, and had a stray
  blank row;
- the close button was a text "X";
- there was no scrolling or clipping.

The user also reported two strip problems:

- the "…" menu was still present;
- the "settings" icon was a hand-drawn hexagon, not a gear.

The user then requested the new strip order and moving B next to the HP bar.

## Settings window (`probe/src/settings_ui.rs`)

**Geometry.** All geometry was measured from the preview with
`getBoundingClientRect` at desktop width, in 1920×1080 coordinates, relative to
the 1360×850 window at (280, 115).

- **Content area:** viewport `y 225–760`, `x 273` (width 1042); control column
  at row `+634` (width 386).
- **Rows:** options are 84 px tall and bindings 70 px, with a 6 px gap.
- **Sections:** 22 px tall, with 12 px below and at least 24 px above.
- **Keybinds page:** the hint box is 45 px tall and the column headings 29 px.

**Content model.**

- `layout(page)` produces entries: hint, section, heading, option, binding and
  cursor preview.
- Visible entries go into fixed node pools: 9 rows, 5 sections and 3 headings.
- Opaque masks above and below the viewport clip partially visible rows.
- Wheel notches scroll 90 px, eased over 167 ms; a scrollbar shows the
  position.

**Clicks.** Click events carry a slot index. They resolve through the entries
shown in the previous frame, so scrolling cannot misroute a click.

**Controls.**

- **Segmented control:** track `#5b5b5b` with a sliding indicator `#eeecec`
  (167 ms). The indicator turns `#b8b6b5` on hover, and the selected label
  colour changes over 100 ms.
- **Select field:**
  - Capsule `#eeecec` with a chevron.
  - The menu appears 25 px below the top of the field, sized 386×230, colour
    `#d4d2d3`, fading in over 117 ms.
  - Options are 64 px tall. The selected one is `#5f5f5f` with a white bar and
    a check mark.
  - The menu opens upward when there is no room below.
  - It closes on choosing an option, Esc, scrolling, or when its row scrolls
    out of view.

**Text.** Multi-line text is set through the native text API, which keeps real
line breaks. Escaped source text rendered as a literal `\n`.

**Wording.** Option labels and hints now match the preview, using `·`, `→` and
`×`. These glyphs are present in the packaged font metrics.

## Attack-cancel toggle (`settings.rs`, `native_adapter.rs`)

`attack_cancel` adds an Off/On choice to Combat & casting → Attacks, with On as
the default.

`finish_attack_backswing` returns early when the option is not On, so
cancelling is skipped. The check reads one value through `settings::option`;
the whole settings document is not cloned.

## Strip (`session_ui.rs`, `player_hud.rs`)

**Session controls.** The strip container is 656 px wide, starting at x 8:

| Element | Container x | Width |
| --- | --- | --- |
| Pause | 0 | 44 |
| Return to AI | 48 | 44 |
| Camera | 96 | 44 |
| Settings | 144 | 44 |
| Divider | 194 | 1 |
| Vision group | 200 | 136 (3 × 46 step) |
| Divider | 342 | 1 |
| Tab | 586 | 68 |

The more menu, its button and its state were removed. Vision clicks now apply
directly, with no requirement that a menu is open.

**Player HUD.** These are screen x positions:

| Element | Screen x | Width |
| --- | --- | --- |
| K/D/A icon | 356 | 26 |
| Kills | 386 | 30 |
| First slash | 416 | 10 |
| Deaths | 426 | 30 |
| Second slash | 456 | 10 |
| Assists | 466 | 30 |
| CS icon | 510 | 26 |
| CS value | 538 | 50 |
| B recall key | 682 | 18 |
| Recall icon | 702 | 20 |
| Level (unchanged) | 730 | — |
| HP bar (unchanged) | 780 | — |

The old divider at x 151 was removed; the session controls draw their own
dividers. HP, items, gold and purchase displays are unchanged.

## Icons

`tools/generate_settings_glyphs.cjs` builds glyphs from the Lucide sources (ISC
licence):

- the gear for Settings;
- X;
- chevron down, chevron up and chevron right;
- the preview's own keyboard and panels icons.

Each glyph is checked to be white-only, and the results are recorded in
`research/settings-glyphs-0.53.0.json`. The design `settings.svg` is now the
genuine Lucide gear.

## Verification and limits

- **Automated checks:**
  - 240 probe and 18 core tests pass.
  - New tests compare the layout spacing with the preview's measurements, slot
    limits across scrolling, event routing and the strip order.
  - Clippy, formatting, release build, 148 anchors and ABI checks pass.
- **Package and install:** 82 assets, of which 5 changed or are new. Installed
  with fingerprint checks.
- **Not verified:** native rendering, motion and clipping have not been checked
  in game.
- **Possible differences from the preview:** native font weights (Manrope
  medium/bold), letter spacing and shadows. The row texture, slider hover track
  and shadows were not reproduced.
