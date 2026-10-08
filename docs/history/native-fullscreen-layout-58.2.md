# Coordinated native full-screen layout (0.58.2)

## Why the previous correction was incomplete

The user confirmed 0.58.1's pulse but showed a kill announcement still centered
on the old left-half viewport. Forcing GameViewSharedConfig.view_wide made the
battlefield wide; hiding compact option buttons did not change IngameUI's own
layout state. Native announcements and the small kill feed read that separate
state, so the screen contained two different layouts.

## Reviewed 0.6.3 evidence

All RVAs refer to the exact fingerprinted executable in the current native
profile. The native key toggle at 7b4de5..7b4df1 changes both the shared config's
wide flag (+0x45 within its payload, +0x5d within Arc/RefCell) and IngameUI's
view_wide byte (+0x9234). The button callback at a43e02 links IngameUI's shared
config (+0x9150) to that same payload.

The existing viewer CALL at cdcce0 already receives the live `ingame` UiNode as
its eighth argument: cdcbf6 loads the six-byte ingame path, cdcc79 obtains the
node and the caller places it at stack argument eight. Earlier wrapper naming
called this argument `runner`; the local name now reflects its actual role.
The native viewer forwards it to b9c970, whose b9cfc5..b9d033 path downcasts
node userdata (+0x230/+0x238, get_mut vtable slot +0x50) to IngameUI. The exact
128-bit TypeId is checked before any IngameUI field access. The native Rust
fat-pointer result uses RAX/RDX; it is not a Win64 C aggregate return.

IngameUI update at 280bd60 binds its own wide byte. At 280d99d..280d9f8 it passes
that byte to native announcement and kill-feed layout helpers 1e71e40 and
1e71f60. Those helpers choose the full layout when the byte is nonzero.
The full layout branch also hides option_buttons (280f42f..280f459), so native
UI code can handle its own compact controls without our extra visibility/event
workaround. All these relevant consumers are added as exact profile guards;
no new hook or patch at a function entry is introduced.

## Implementation and lifetime

The existing viewer wrapper downcasts its live node and verifies that
IngameUI.shared_config + 0x18 equals the currently borrowed viewer config.
CameraLease saves both flags independently, including cases where their prior
values disagree, plus the existing camera/vision snapshot. It forces both
wide flags before native viewer processing and reasserts them afterward.
Native UI update continues to lay out its own announcements and controls.

Restoration requires the same live viewer, config and IngameUI object and a
fresh matching config link. Cached UI addresses are never dereferenced after a
match reset. F12 restores both original flags; next-match reset discards the
lease. Missing node, wrong TypeId or shared-config mismatch cancels takeover
before applying a partial renderer-only override. Existing custom-HUD panel
suppression stays; only 0.58.1's extra option_buttons hide/event overrides are
removed. Camera side, zoom, minimap operands, input pacing and outlines stay
unchanged.

## Verification and limits

256 probe tests and 18 core tests pass. New/expanded coverage exercises both
original flags in all four combinations, reassertion, exact full-buffer
restoration, mismatched viewer/config/UI, changed config link, fresh viewer,
and the native-style live-node downcast/type check. Profile tests, 0.6.3 byte
and pointer guards, all 15 outline CALL/cleanup guards, Clippy with warnings
denied and formatting pass. Release checks and package fingerprints are
recorded in dist/build-0.58.2.json. Native rendering remains pending the user's
TESTING-58.2.md run; automated tests do not establish in-game placement.
