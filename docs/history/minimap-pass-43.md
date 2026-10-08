# Minimap integration — 0.43.0

The user approved expanding the content to use the original frame footprint,
after the first thin-frame preview left an empty margin. HTML uses a 360 px
frame at x1560/y720 with a 352 px map at x1564/y724, 4 px padding, warm base,
1 px grey edge, 3 px rounding and four muted stepped corner accents.

## Native implementation

`probe/src/minimap.rs` supplies content geometry, border strokes and a reviewed
operand plan embedded from `tools/native_profiles/0.6.3.json`. The 54 operand
edits cover native origin/inset, background dimensions, fog-cell pitch,
world-to-map marker scale, marker dimensions/offsets, objective timers,
camera rectangle and both native pointer-down/move navigation branches.
The two smaller-layout map locations use the same content size and padding;
the renderer's custom origin is reflected in the mod camera snapshot.

Original instructions and their original referenced constants are checked
before installing. Each RIP-relative operand is redirected to its own aligned
16-byte constant slot in a nearby read-only allocation. Shared game constants
remain unchanged. Immediate edits preserve other fields, including the fog
command's integer z value. These changes are process-local; no executable or
bundle on disk is modified.

The existing branch patch transaction now includes the map operands. It obtains
each executable page's protection once, applies edits only after all pages are
writable, flushes instructions, restores page protections and reads every edit
back. The traffic audit also checks map operands and the private constant block.
Per-page protection avoids retaining a temporary writable protection when
several operands share one page.

Native camera navigation uses the same new origins and `960 / 352` conversion.
Mod right-click destinations, route drawing and map click feedback continue to
use `CameraFrame.minimap`. Border strokes are emitted through the stable drawing
API in UI space, so battlefield-only death grayscale does not affect them.
Fresh map geometry is captured during ordinary viewer playback as well as
direct control. The current HUD strip and its input blocker end at x1560;
combat text and icon sizes are retained. Full HUD restyling is not this pass.

## Review and validation

Instruction review is preserved privately in
`research/minimap-063-disassembly.txt`. The profile verifier checks original
bytes, instruction boundaries, RIP targets, original constant bytes and immediate
field boundaries. It also accounts for Capstone's incorrect displacement-size
report on prefixed UNPCKLPD: RIP-relative addressing still encodes disp32.
The build receipt includes these operands and source-data guards in its native
anchor list, so the migration tool audits them on the next game patch. Data
addresses remain manual-review cases rather than code signature candidates.

Projection tests cover wide, both smaller layouts and custom placement, plus
map edge exclusion and equivalence with native camera conversion. The operand
plan checks allocation alignment, size, reach and byte lengths. The existing
control suite remains applicable; see [native checklist](../../local/testing/TESTING-43.md).
Automated checks and HTML screenshots do not verify native in-game rendering.
User testing remains pending.

Source before this pass is preserved in
`research/backups/0.42.0-before-0.43.0/`. Build/install receipts record the checked
package, installed fingerprints and the preceding mod-file rollback location.
