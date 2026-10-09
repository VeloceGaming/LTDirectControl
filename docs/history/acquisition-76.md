# Configurable attack-move acquisition — 0.76.0

## 0.76.4 polish

The user confirmed 0.76.3's Advanced subpage buttons work and that a minimum
around 120 feels appropriate. Automatic's default and reset minimum now use
120, with +5 buffer. Existing saved minima/buffers and per-champion overrides
are preserved; a regression covers migration plus registration with a saved
70-unit minimum and a Custom override.

Search now uses the portrait grid's actual width: seven 64-unit pitches plus
the last 58-unit cell, or 506 units. The mirrored text uses the same width less
its padding. Ignore/Honor labels are shortened without changing their values.

The shop title uses the supplied transparent PNG, never the earlier JPG. The
original bytes are embedded in a centred SVG image wrapper with -7.5 degrees
rotation. The wrapper is rendered to a 128px PNG and displayed at 64px using
the existing image runner, with white tint to retain the original colours.
Its bounds stay within the header and the generated image has transparent
edges. tools/generate_shop_stamp.cjs and tools/records/shop-stamp.json record
the source fingerprint, angle, size, wrapper and generated asset fingerprints.
No new native hook or renderer is introduced.

Validation: 355 release tests, Clippy with warnings denied, formatting and release
build passed. Exported Acquisition contains 139 in-bounds nodes and 40 portrait
slots; search and grid both end at 506, mirrored text is 478 wide, and the stamp
fits within the header. All 555 native anchors, 34 layout guards, ABI/null-host
checks, 86 UI/font assets and the 88-member ZIP passed verification. Installed
over fingerprint-checked 0.76.3, with all destination fingerprints matching.
Native rendering and gameplay remain pending the user's in-game check.

## 0.76.3 navigation repair

The user reported Acquisition and Debug did not respond in-game. Their handlers
were registered, but the new `advanced_nav` empty container had no explicit
input layer and preceded the event-blocking clipping masks. Child draw layers
alone did not ensure access through that arrangement.

The entire navigation subtree now follows the masks. Its passive container uses
`ignore_event: true; z: 2013`, with child buttons at 2014, above masks at 2012.
The masks remain event-blocking so clipped rows cannot receive clicks. Subpage
changes are logged as `SETTINGS advanced_subpage=...` for follow-up diagnosis.
The regression checks the layer/order contract and repeated switching across
General, Acquisition and Debug, including draft preservation and hidden events.

Validation: 355 release tests, Clippy with warnings denied, formatting and release
build passed. All 555 native anchors, 34 layout guards, ABI/null-host rejection,
85 assets and the 87-member ZIP passed verification. Installed over hash-checked
0.76.2 with matching destination fingerprints. Actual native click routing and
rendering remain pending the user's in-game test.

## 0.76.2 follow-up

Advanced has three internal pages: **General**, **Acquisition**, **Debug**.
General contains Cancel attack wind-down and Manual shopping. Debug contains
logging, selection collision markers, drawing tests and the new live acquisition
overlay. The schema uses page 4 for General, 5 for Debug, and 6 for Acquisition;
these are internal IDs, not additional top-level navigation tabs.

Acquisition fits a 475-unit body below the subpage bar. An 8 × 5 portrait pool
scrolls complete rows. Empty slots explicitly hide the button and image and
clear both cached bindings; returning to an occupied slot restores interaction
and loads its face. Search and clicks still use the last-rendered slot map.

Shared Automatic minimum and AA buffer fields accept finite numbers from 0 to
10,000 game units. The defaults remain 70 and +5. Shared Reset changes only
these two values; the champion Reset clears only that champion's Custom radius.
Restore this page clears all acquisition preferences only on Acquisition.
Invalid field text prevents Apply across all pages. Cancel discards the draft.

Persistence version 3 removes generated `default_baseline_radius` metadata so
all Automatic champions inherit the shared minimum. Version 2 shared values
are preserved, including deliberate 120/+10 settings; older generated defaults
still receive the prior migration. Champion overrides and unknown fields survive.
Automatic remains max(shared minimum, maximum AA + buffer, current AA + buffer);
Custom remains max(chosen radius, current AA).

Enable **Advanced → Debug → Show live acquisition radius** and close settings.
Yellow is the functioning acquisition radius; white is actual AA reach. A label
shows both in game units. Both numbers and the position are copied from the
selected player's targeting callback. Other players/matches cannot replace or
clear the sample. Running samples expire after 250 ms; paused matches retain
the last observation. A missing actor/range clears it, and session/selection
resets discard it. It draws only during manual Running/Paused phases. No native
pointers are retained and no new hook is required.

The bounded caret pass confirmed that the SDK exposes only committed `text`
and `is_editing`, with no caret or selection offsets. Correct restoration needs
native rendering work beyond this small change. The user authorized deferring
it; the focus underline and committed-text mirror remain. Native rendering,
input and gameplay verification remain the user's in-game checks.

Validation: 354 release tests passed; Clippy with warnings denied, formatting
and release build passed. Exported source was balanced, with 139 Acquisition
nodes checked within the 475-unit body (excluding intentional contact shadows;
slider thumb checked at its runtime clamp), 40 portrait slots and three subpages.
All 555 native anchors, 34 layout guards, ABI/null-host checks, 85 UI/font assets
and the 87-member ZIP were verified. Installed 0.76.2 over fingerprint-checked
0.76.1; all destination hashes match. User in-game testing is pending.

## 0.76.1 follow-up

Acquisition now lives in **Advanced**, in a fixed 535-unit body beside an 8 × 6
portrait slot pool. The SDK supplies the game's face crop. Only complete grid
rows scroll; entering Advanced bypasses the previous page's scroll animation.
Search matches internal IDs and bundled translations across languages, plus
available enabled-mod `.i18n` names. The current-language display name still
comes from the game's formatter.

Automatic defaults are now **70 minimum / +5 buffer**. Maximum scaling and live
AA floors are unchanged. Generated 120/+10 values migrate once, and custom
overrides survive. Combat's page reset no longer resets these Advanced values.

The old SDK's `TextEditRunner::render` emits text, selection and background at
fixed z=100; unlike labels, its properties have no z. This explains why a field
could accept text while appearing blank under a z=2000 modal. The input node
keeps native editing/IME, with a high-layer plate, committed-text label and focus
underline. Native caret/selection and uncommitted IME composition are not
mirrored; this is a remaining visual limitation, not a claim of full native
editor rendering. Research: `text-edit-fields.json` and emitted assembly under
`research/` (read-only SDK analysis; no native function patch was added).

Validation: 350 release tests passed, Clippy with warnings denied, formatting,
exported template structure and body bounds, release build, all 555 native
anchors, 34 layout guards, ABI/null-host rejection, and 85 packaged UI/font
assets. Installed 0.76.1 over hash-checked 0.76.0. In-game checks remain with the
user: scroll the full grid, search English/Traditional Chinese/mod names, select
a face, edit Custom, Apply/Cancel, and reopen settings after scrolling another
page. The historical 0.76.0 notes below retain their original values/layout.

Ground A-click and Shift + right-click now use a search radius distinct from
actual AA reach. With no acquired enemy, the champion continues to the ground
destination. With an enemy inside acquisition reach, the champion approaches it
even if the ground destination is in the other direction. Visibility, targetability,
near-champion/near-cursor preference and champion-only policy are unchanged.
Direct target orders keep their explicit target and approach behaviour.

Automatic: `max(default baseline, maximum scaling AA reach + buffer, current AA reach + buffer)`.
Initial baseline is 120 game units; buffer is 10. These are mod tuning values,
not measured League constants. Custom: `max(chosen radius, current AA reach)`.
Temporary range bonuses do not overwrite the chosen value.

The live SDK `GameSetting.need_exp` table supplies the level cap (number of
level-ups + 1). The existing guarded native AA descriptor supplies base range,
growth and current reach. Maximum is base + growth × (cap − 1), excluding
temporary bonus range. Current reach independently covers bonuses above it.
Soldier's shipped declaration is base 60, growth 3, giving 93 at level 12.
Data-defined/mod champions with the same descriptor scaling work identically.
Arbitrary custom passive code or uncapped stacks cannot reveal their future
maximum through this descriptor; current reach + buffer remains the fallback.
No entity writes, new native hooks or start-of-function patches were added.

Settings → Combat & casting → Attack-move acquisition provides a name search,
champion previous/next selector, Automatic/Custom buttons, slider and number
field. Current AA reach is shown when observed for that champion; static
declarations help describe champions not currently controlled. Native data wins
for the controlled champion. Settings pause ownership and Apply/Cancel are
unchanged. Radius accepts 0–10,000 game units; the effective floor is always
applied at runtime. Invalid text blocks Apply rather than silently saving it.

Persistence uses `%LOCALAPPDATA%\LTDirectControl\controls.json`, under
`acquisition`: shared `baseline_radius`, `aa_range_buffer`, and `champions`
keyed by exact champion ID, with `default_baseline_radius`, nullable
`override_radius`, and informational `maximum_scaling_aa_range`.
Apply registers the active champion list and preserves existing overrides and
unknown settings. No external editing is needed. Restore default clears the
selected champion's override; Restore this page resets acquisition preferences.
Input snapshots exclude the table; worker acquisition reads only the chosen
champion while holding the settings lock. No file IO happens in simulation.

Code boundaries:

- `acquisition.rs`: formulas, validated settings values and copied champion metadata.
- `settings_ui/acquisition_panel.rs`: search/selection, draft edits and rendering.
- `settings.rs`: persistence and transactional settings values.
- `simulation.rs`: actual AA reach vs acquisition reach; existing cooldown-stop scope.
- `movement.rs`: identical acquisition radius for orders and target highlights.
- `acquisition_defaults.json`: numeric base declarations from the reviewed game bundle.

Automated regression checks cover scaling, live bonuses, custom minimum and
preference preservation, unavailable scaling, malformed settings, unusual mod
IDs, settings save/reload, Apply/Cancel, invalid input, native descriptor growth,
and order/highlight agreement. Native rendering and gameplay require user tests.
