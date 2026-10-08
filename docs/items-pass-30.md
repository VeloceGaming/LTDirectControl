# Effective item data and route lifetime, 0.30.0

Correction from 0.31.0: getter +0x80 returns NEXT-tier edges. This pass mislabeled
them as previous tiers and inverted them, breaking the forecast. The description
below records that implementation, not a verified semantic mapping. See
[0.31 notes](selection-items-pass-31.md) for the constructor evidence and fix.

The user approved the three current priorities recorded in open-issues.md.
The Ninja unwanted-turn and FPS-drop investigation remains deferred.

## Item source

The SDK's ItemSetting JSON contains original settings but not all registered
native additions. The tested catalogue has 262 keys; the old HUD loaded only
31 metadata aliases. Reading the item mod's JSON alone has the same limitation.

On the fingerprinted 0.6.2 executable, the live SDK AI bridge exposes its native
AI context. Native buying reads that context's settings and registered item Vec.
The adapter validates the observed AI vtable, getter addresses and five additional
instruction anchors, then uses the same ItemInfo key, icon, enabled, price, tier,
stat and previous-tier getters used by native buying and detail panels. It inverts
previous-tier edges to recover next upgrades without using an unverified slot.

Only owned strings, numbers and JSON are retained. No native object, vtable or
context address escapes the callback. Capture is restricted to the live worker
and cached by match; the SDK build catalogue must be covered by the copied keys.
The existing final-player-build reader remains in use. This pass adds no patches.

Prices are incremental getter prices, not a sum reconstructed from incomplete
files. Disabled branches are excluded. Branching choices remain uncertain.
Descriptions use the host's active localized item text plus the registered stat
values. Missing settings no longer prevent localized text lookup. Unresolved
formula placeholders remain ellipses; the mod does not guess their values.

Stat getter offsets and range conversion are established by native detail-panel
function 0x29ca4f0. The POD buffer is not interpreted as SDK BuffV1. Research
evidence is in research/items-30.asm and research/items-registry-30.asm.

## Tooltip and route changes

Preserve valid native RGBA color tags and reset markers; remove unsupported
inline-asset markup. Measure tooltip wrapping on plain visible text rather than
counting markup bytes. Invalidate tooltip and forecast caches when effective
metadata changes, even if its entry count is unchanged.

Map callbacks also run for background simulations. Once the live match binds,
its geometry is pinned and new geometry is staged for the next session. A missing
map callback does not erase known geometry. Background callbacks no longer clear
the current route or replace its waypoints. Actual arrival, cancellation and new
orders retain their existing route-clearing behavior. This correction does not
explain the reported Ninja incidents, which had no map callbacks.

Native gameplay and rendering require the user's test. See TESTING-30.md.
