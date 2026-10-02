# Player HUD, probe 0.24

During the bound READY/running/paused session, the client creates
`ingame.lt_player_hud` through the installed stable SDK's UI source API.
The panel is 600 x 120 UI units, centered at the bottom with a 12-unit gap.
It displays portrait/level/health, three Q/W/R slots, gold, six owned-item
slots, K/D/A, CS and aiming/recall/action status. Overflow inventory is noted.
Native labels/images provide rendering; successful property/text updates are
cached to avoid reparsing unchanged values each client frame. Spawn failures
are logged and retried at most once per second. Missing portrait/icon metadata
uses a text fallback; a failed portrait lookup is attempted once per champion.

The 0.17 game screenshots showed the root background obscuring the contents.
0.18 explicitly orders every drawing node: background at z=1000, slot frames
at 1001, inner backgrounds/health fill at 1002, icons at 1003, cooldown shading
at 1004, and text at 1005. The owned tooltip uses 1010/1011. Portraits, slots,
text positions, hover rectangles and the initial-layout fallback mask were
resized together. The user confirmed this layer and size correction in 0.18.

The user confirmed 0.18 rendering and popup cleanup. 0.19 adds a dedicated
CHAMPIONS ONLY label while the backtick/Mouse 4 toggle is enabled. Q/W/R unlock
at levels 1/3/5, per the user's game information. Below that level the slot dims,
its border stays gray even during aiming, and the overlay/hint states its unlock
level. Native effect presence alone does not imply a learned skill. Level-up
returns to the ordinary cooldown/effect display; this HUD policy does not alter
cast validation or learn skills. Portrait/name space and other layout stay compact.

Any living player's SDK callback on the original bound worker can copy the
selected player's scalar stats and owned item keys every six ticks, deduplicated
by match/player/tick. This avoids relying on the dead actor's own AI callback.
Snapshot identity must match the selected match key and player; no host context
or entity pointer leaves a callback. The absent entity retains its same-player
champion name, while player getters provide current inventory and respawn ticks.
Death clears pending orders and skills dim while their art remains visible.
Pause retains the actual timer. Living running data still expires after 250 ms;
stale dead data retains identity/inventory but suppresses the countdown and HP
rather than extrapolating from wall time. READY retains the held initial sample.
The panel does not mutate stats, inventory, cooldowns, purchases or combat policy.

0.21 reads enabled mod IDs/order from the installed mods configuration, resolves
local/known Workshop roots, and reads bounded .data_champion declarations once
per process. Existing owned PNG skill paths are supported alongside the embedded
base-game sprite-sheet references. Disabled packs are excluded. In 0.22 all image
children are created with the HUD once and updated in place. Each skill has
separate permanent icon (sheet) and png children, with only the matching format
visible, so a PNG never inherits a sheet rect_tag. Each item updates only its
own permanent child. The size, explicit z layers and masks are unchanged.
0.21's per-icon remove/spawn was the strongest suspect in valid references
failing to draw and older item images disappearing. The user confirmed 0.22
rendering, including all icons on the mod champion Harpy. Logs include property
success, existence and bounds.
This supports the installed data champion packs; arbitrary DLL-defined skill
icons without declarations still require an additional discovery mechanism.

Item icons prefer the SDK's active ItemSetting JSON, indexed by name/key. Reads
are throttled and repeated when owned keys are missing; base metadata remains a
fallback. The logical base item sheet is retained so the host applies enabled
asset overrides. For native items absent from settings, key-as-tag is permitted
only if that key exists in the enabled sheet override's images table. Unknown
items retain a question mark. The Riot pack redirects both #sheet and #data to
its 640X640 sheet; its 243 tags were inspected. No external textures are copied.

Literal resource work: read the SDK player/entity getters, champion portrait
helper, UI creation/layout/text/image APIs and installed ingame/detail/item
templates. Read bundle.game_data as counted records, extracting sprite-sheet
metadata and item settings. The icon references in hud_assets.json contain
native source/tag names and basic item metadata only, not textures.
research/hud-assets-index.json records bundle offsets. tools/read_hud_assets.py
reproduces the bounded extraction and reference generation. Native read-only
disassembly of 1ffef20 confirms skill-icon indices 0/1/2; explicit champion
skill_icon overrides such as Harpooner's source/tags are respected. Findings
are preserved in research/hud-native-icon.txt and hud-icon-references.json.

Known 0.6.2 spectator toolbar/portrait roots join the saved visibility overrides.
Known event-enabled portrait entry/icon_slot nodes are set ignore_event=true,
then restored to their template default false on release. Already ignored
bg/icon children are not overwritten. Client post-update also suppresses
native champion/item/generic spectator tooltips that native hover code may
republish. The HUD has an independent brief tooltip node; release hides the
HUD and restores spectator visibility/event behavior. No saved UI preference
is changed. In 0.23 the placeholder skill/item hover hints are disabled; their
node remains invisible and contributes no hit mask. Real descriptions/localization
remain later work. Session reset clears scalar snapshots and icon/property caches
so a repeated key from a reloaded save cannot reuse stale HUD data.

0.17 missed the separate `ingame.champion_info_tooltip`. Bounded read-only
native disassembly found `champion_info_tooltip` in InGame initialization
(string RVA 0x3ad5658, reference 0xb9d327) and the tooltip helper at 0x1ffe3a0.
The helper can create it on first use from `asset/base/ui/layout/champion_tooltip`;
that bundle template is 600 x 542, matching the large popup in the user's
screenshot. This differs from the short 335 x 106 `ingame.champion_tooltip`.
0.18 checks the large popup each active client post-update and hides it when
visible, including late creation. Logs record initial existence, late discovery
and up to four suppression results per session. Suppression stops on release;
native hover can then show it normally. This is a UI change, with no new hook.

The panel and visible tooltip rectangles join both camera and battlefield
command masks, with a panel fallback while initial layout is pending. The
centered Tab panel remains command-click-through. The HUD itself does not
send commands on click. Running diagnostic drawing and its hit mask shrink
together; READY and failures keep the detailed diagnostics. Header/minimap,
normal casting, recall, worker pacing and all twelve native anchors are retained.

0.20 retains the layout and targeting/unlock display. Pause keeps the last
same-match snapshot visible and shows a paused status; Resume returns to fresh
running samples. The separate session controls sit below the top header.

See TESTING-22.md for persistent icons and death camera checks. TESTING-21.md records
modded icons and death/respawn checks. TESTING-20.md records
session/range and full-battle checks. TESTING-19.md records
confirmed targeting/unlock checks; TESTING-17.md covers death/revival observations.

0.24 adds ten permanent team portrait cards in two compact rows under the
scoreboard. Each card uses the game's `ui_set_champion_icon` helper, including
mod champions. Side and lane come from SDK player getters on the bound original
live worker. The selected player's border is gold. Dead portraits are shaded
and show native `respawn_time` ticks rounded up to seconds at 60 ticks/second.
Other living actors sample all players every six ticks, including dead players.
Champion names survive absent dead entities only within the same match/player/
side/lane identity. Pause retains native timers; stale running data hides
numeric countdowns rather than predicting them. Session reset clears roster,
sampling and UI caches. Cards ignore native UI events and their small row
rectangles mask camera/commands; the empty space between the rows is unmasked.
Return to AI restores the spectator layout and hides these cards. TESTING-24.md
covers rendering, deaths, countdowns and subsequent drafts.


## 0.26 graphical UI

Opaque #141414 panels and grayscale framing replace colored chrome and routine
English labels. Original PNG glyphs are shipped under the native mod asset root.
Active selection, camera follow, champion-only targeting and recall use exact
#FFD700 variants. Death count is red; original game artwork retains its colors.
KDA, CS, gold and level are numeric. Dead portraits are shaded with a native
respawn countdown; ability/item art remains in permanent image nodes.

The centered 400x150 preparation panel contains five confirmed own-team champion
portraits. It rebinds only in READY, before Start. Queued choices are match/phase/
generation scoped. The client excludes worker callbacks while clearing previous
command/camera/ability state and switching to initial scalar HUD data. Normal
running role changes remain locked. The panel contracts to 152x60 icon controls
once running. Loading uses an hourglass; routine diagnostic strips are removed.
Ctrl+Home starts/resumes; it does not pause, so no misleading shortcut is shown
under the pause icon. The two-second heartbeat guard remains; READY no longer
expires after 120 seconds.

Skill and occupied-item hover uses a 180ms delay. Host i18n resolves installed
translations, enabled champion declarations provide description references, and
active item settings provide stats. Rich markup is stripped for plain UI labels;
unavailable dynamic formula parameters become ellipses rather than guessed
values. Unknown translations fall back to a key-only skill heading or item key,
not a fabricated description. Tooltip height is conservatively estimated and
needs native wrapping checks. Tooltip nodes ignore events and are excluded from
world-command masks. Brief failed casts can show text; ordinary UI is graphical.
See TESTING-26.md. Native rendering and hit tests are not confirmed by unit tests.
