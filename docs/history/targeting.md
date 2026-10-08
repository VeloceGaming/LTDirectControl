# Direct targeting and markers, probe 0.25

Backtick (Windows VK_OEM_3) and Mouse 4 (VK_XBUTTON1) toggle the same mode.
This is a toggle, not a hold modifier; simultaneous rising edges count once.
Either button can turn the mode off while the other stays held. General key
configuration remains optional future work.

The client resolves the toggle before updating the HUD and acquiring commands.
It is bound to selected match/player identity while control is active, including
READY. Release, identity change or leaving the session resets it. Focus loss
retains the mode while the bound session remains active, but held buttons on
takeover/focus return cannot toggle it. A dedicated gold HUD label remains
visible while enabled, independent of aiming/recall status.

In 0.20, pause retains the mode and indicator while disabling gameplay orders.
Return to AI resets the mode. A attack-range aiming uses the same unrestricted
attack-move target policy; camera and Tab preserve the range display.

Literal implementation: read the existing Windows input polling and SDK
`EntityView::is_champion()` getter, then add that scalar flag to the copied
target snapshots. No native disassembly, new hook or retained entity pointer
is needed for this feature. Direct click/skill acquisition excludes non-
champions before cursor hit ranking, allowing champions behind overlapping
minions to be selected. SDK enemy visibility/targetability checks remain.
Native skill validation still controls side, range, status and cooldown.

Right-click retains its empty-target behavior: no eligible champion under the
cursor produces ground movement. Quickcast captures mode at the ability press;
normal cast captures it at confirmation. Changing mode does not close aiming,
replay a rejected request or cancel an already issued attack. Unit-skill
rejection does not fall back to non-champions; invalid normal confirmation keeps
aiming. Ground/direction/self/no-target requests are unchanged.

Attack-move deliberately resolves against all eligible enemy snapshots. It
keeps nearest-to-click acquisition and may attack minions while the mode is on.
The toggle only limits direct target acquisition. See TESTING-19.md for physical
button, overlapping-target and attack-move checks.

In 0.24, A + left-click on an eligible enemy creates the same persistent
`Attack(id)` order as right-click. It follows that target's fresh position;
native validation controls attack readiness/range and the existing approach
path moves toward it when it cannot attack yet. Other units cannot steal the
order. Target death, lost vision or lost targetability clears it. A ground
click still creates `AttackMove(point)` and keeps the existing acquisition
radius and nearest-to-click ranking. A remains unrestricted by champion-only.

Enemy hover uses `combat::clicked_unit`, the same hit ranking as commands, with
the champion-only filter except while A is armed. A pale gold foot ring marks
hover; an outer orange ring marks the attack order's current resolved target.
Only fresh, visible, alive, targetable enemy snapshots can produce a marker.
HUD/minimap picking is excluded; rendering clips rings around visible UI.
Pause, death, focus loss and session resets prevent stale markers. These are
SDK overlay rings, not a native sprite silhouette outline. See TESTING-24.md.

0.25 replaces champion collision-based click boxes with normal-pose body
envelopes. The same picker serves hover, right-click, direct A-click and unit
skills. Base animation dimensions are embedded as references; enabled local/
Workshop pack declarations load exported `.fanim` or Aseprite cel/tag metadata
once at initialization. Idle/run/walk frames determine dimensions; oversized
attack effects do not. Enabled-pack discovery is shared with HUD assets.
No pixel images are copied and no native animation pointer is retained.

Width/height scale with camera extent. A small three-UI-pixel margin forgives
edge clicks. Core body hits rank before margin hits, then by distance to body
center and entity ID. The legacy minion/structure areas stay smaller. Champion
body picking can accept a visible upper body even when its feet are offscreen;
cursor viewport/minimap and caller HUD/visibility/targetability filters remain.
Combat collision, attack ranges, ground attack-move ranking and champion-only
policy are unchanged. Unknown art uses a bounded fallback. Metadata validation
rejects invalid dimensions, paths, frame/chunk lengths and unsupported layers.

This is a stable normal-pose proxy, not per-animation opaque-pixel selection.
The half-sheet map scale and bottom-center anchor require TESTING-25.md fit
verification. Transparent holes, asymmetric poses and swings can differ from
the envelope. Aseprite metadata parsing follows the [official format specification](https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md).
