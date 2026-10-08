# Fixed hover width and compact spectator controls (0.58.1)

The user rejected changing pulse duration or easing: its animation already feels
right. Instead, hover is reduced from 1.5 to 1.2, with the unused 2.5-wide mode and
F10 comparison switch removed. Attack offset stays 2/3 and peak click offset stays
3.0. The 120 ms interpolation, click > hover > target priority, visibility/state
gates, independent debug markers and four-pass renderer are unchanged.

Removal includes the alternate width, atomic mode/held state, F10-only polling,
client switch argument, mode logs and package description. F10 is no longer
reserved for this prototype; ordinary configurable keybind polling still works
if the user binds F10 to another action. Historical build notes retain the earlier
comparison experiment; current code has one fixed hover width.

The screenshot's leftover controls match `ingame.option_buttons` in the native
ingame template: auto camera, view-category selectors, result and pause. This is
a sibling of `center_data`, so hiding that detail panel did not hide these buttons.
TeamInfo now saves/restores this group's visibility along with other spectator UI
and reasserts hidden state while owned. The eight known interactive descendant
buttons are added to the scoped event suppression/restoration list. The group's
hidden state also removes its rectangle from existing native command blocking.
No native hook, layout flag, minimap geometry or top scoreboard change is added.

255 probe tests (including fixed width and existing priority/pulse/lifecycle
regressions), 18 core tests, Clippy, formatting, 0.6.3 profile and all 15 outline
CALL/cleanup checks pass. This small visibility fix is not validated by an
implementation-mirroring test; actual spectator button disappearance, click-through
and restoration must be checked in-game with TESTING-58.1.md.
