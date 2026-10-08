# 0.39 — selection envelope and SVG cursor integration

Authorized scope: smaller champion selection envelopes, supplied cursor art and
motion, and a saved size control. Combat HUD layout is unchanged; team availability
stays on Tab and the top stripe stays vanilla. No new cast or movement rules.

## Selection

Vanilla and enabled mod champion profiles use idle/run/walk pose dimensions.
Combat poses may expand each dimension by at most 15 percent. When these stable
tags are absent, the median normal pose supplies a bounded baseline. Side/top
padding is slightly reduced, while the previously tested foot allowance remains.
Monster profiles, minion rules and champion-first picking priority are unchanged.
Highlights use the resulting shared sprite envelope. This is a stable forgiving
rectangle, not per-frame alpha picking or a change to combat collision radius.

Examples of source-pixel envelopes: Lancer 112×73 becomes 63.25×54.05, Ogre
121×147 becomes 58.65×79.35. Existing half-scale world conversion remains.

## Cursor ownership and artwork

`design/cursor/index.html` is retained unchanged. Its inline SVG paths supply
arrow, sword, attack reticles and champion-only badge. Extra skill validity
variants reuse that grammar. Ink/light are strictly neutral; yellow/orange remain
semantic signals. Deterministic SVG rasterization creates fourteen embedded
64×64 premultiplied BGRA images, with a manifest of source and pixel hashes.
The contact sheet was visually inspected. No image generation or tracing is used.

Windows native cursors provide pointer movement independently of the game's
frame drawing. `CreateIconIndirect` copies temporary DIBs; the mod deletes those
bitmaps and retains owned cursor handles until shutdown. Handles are cached by
integer size, with a bounded 41 sizes × 14 states. Hotspots scale with size.
A client-thread `WH_CALLWNDPROCRET` observer reapplies the current handle after
the game's client-area `WM_SETCURSOR`; it does not consume input or replace the
window procedure. The override is limited to the foreground game window and
the owning GUI thread. A thread mismatch is logged and retains the game cursor.
The override clears over HUD/menu/minimap and after control release or focus loss;
the last host cursor is restored if the current cursor belongs to this mod.
Owned handles are released on shutdown. No global cursor replacement, pointer
warping, `ShowCursor` counter changes or input injection.

Implementation references: Microsoft's [cursor guidance](https://learn.microsoft.com/en-us/windows/win32/menurc/using-cursors),
[CreateIconIndirect ownership](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createiconindirect),
and [CWPRETSTRUCT layout](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-cwpretstruct).

The enemy pointer calls the same picker and champion-only filter as direct
attacks. Normal-cast validity uses fresh skill/unit data and the same ally, self
and CC eligibility rules as normal casting. Out-of-range valid targets remain
valid for approach-to-cast. Native validation still decides hidden requirements;
the cursor does not call effect validators from the rendering thread.

## Settings and click motion

The existing three-dot menu contains a preview, size number, 24–64 px slider and
reset control. Default/reset is 32 px. Size is independent of camera zoom. Saving
is debounced by 300 ms, flushes on shutdown, and preserves other JSON preferences
including `cast_on_release`. Malformed existing files are left untouched and a
save failure is logged. Changing size remains effective during that session.

Accepted move/attack clicks queue at most eight feedback markers. The supplied
SVG corner geometry contracts 1.9→1 over 167 ms using its cubic-bezier curve,
holds until 367 ms and fades over 200 ms. Markers expire at 567 ms without clearing
the accepted order. Battlefield markers anchor to world position and retain screen
size through zoom; minimap markers use a half-sized map version. Shared line
clipping excludes HUD/minimap from battlefield markers. Pointer motion is never eased.

## Verification and remaining limits

191 probe tests and 18 core tests passed. Lint/format/release and package/native
anchor verification are recorded with the build. Native resource tests create
and query cursors without displaying or moving the system pointer; they check
small/default/large hotspots and Windows structure layouts. All integer sizes
are checked for premultiplied pixels and preserved click points. Selection and
target agreement tests cover feet, overlaps, allies and champion-only filtering.

Native game cursor ownership, slider interaction and marker rendering still
require the user's play test. Rendering and gameplay cannot be declared verified
by those automated checks. See [TESTING-39](../TESTING-39.md).
