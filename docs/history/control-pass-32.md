# 0.32.0 responsiveness, tooltip and picking pass

## Confirmed causes from 0.31.0

- Purchase forecasts excluded targets and inventory with `tier >= 4`. The
  captured Jiangshi build consisted entirely of tier-4 Radiant/custom finals.
  Bomber's build retained only its tier-2 boots. Catalogue entries were present:
  this failure was in forecasting, not missing metadata.
- The skill tooltip formatter replaced every `{...}` placeholder with “…”.
  Bomber's localized description is a parameterized template, so this erased
  damage, ratio and crowd-control duration even though its declaration exists.
- Picking used sprite dimensions but modest fixed pads, while marker rings
  still used combat collision radii. These geometries did not agree.
- Running pacing allowed two published frames ahead and waited with 2 ms
  sleeps. Input collection followed HUD work in client post-update.

The log is preserved at `research/probe-2026-10-06-thirty-first.log`.
The previous log's capture/dispatch samples do not measure visible motion. All
recorded Bomber ground-cast destinations matched captured world aims; this does
not prove the native effect honors them after queueing.

## Implemented changes

Running worker lead is one frame. A condition variable uses the coordinator
mutex for the predicate and wait, avoiding a lost viewer notification. Viewer,
phase changes, release and rearm notify it. A 25 ms timeout polls the emergency
release key and checks the existing startup/heartbeat guards if the client
stalls. SDK AI callbacks never wait. Old generations cannot modify new ones.
The native viewer can still consume up to two frames after a hitch; tests do
not assume one consumption per render. Native playback starvation is pending
user verification.

A lightweight `pre_update` collects running gameplay commands against the last
visible camera/masks. Post-update consumes that same key sample and does not
repeat command edges. Installation, session preparation, heartbeat, HUD and
result inspection remain post-update. Pending pause/release button actions
prevent early acquisition; transitions keep the inactive clearing fallback.

Forecasting includes all enabled assigned build targets, irrespective of tier.
Owned items count by membership/reachability in the assigned upgrade graph,
not by an arbitrary special-item tier. Completed slots must match build order;
inconsistent inventory fails unknown. Root/direct items and branch alternatives
use registered prices. The forecast does not buy, alter a build or advance RNG.

Tooltip parameters use declared fields from the enabled custom champion asset
or bundled base metadata, including nested effects when their value agrees.
Damage ratios follow native inline AD/AP markers; colors are preserved. Tick
lengths convert to seconds at 60 ticks/s, distances use native /1000 units.
Animation `duration` is never assumed to be buff/CC duration. Unsupported or
ambiguous formulas remain “…”. Effective native String-returning formatter ABI
has not been established, so this is not a complete live native-description
bridge; unavailable dynamic patches are not inferred from a base declaration.

Champion/monster envelopes scale width/height by 1.15 plus 4 world units per
horizontal side, 2 at the top, and max(12, height*0.20) below the origin. A 3 UI
pixel edge allowance remains. Explicit minion flags retain their prior smaller
bounds. Highlights use those same envelopes to size foot ovals; actual combat
collision, ability range and attack range are untouched.

## Focused diagnostics and limits

Commands have a monotonic diagnostic ID and capture timestamp. For at most
300 dispatched commands per match, logs connect capture age, worker dispatch,
publication and frame consumption. They describe frame playback, not visible
movement onset. Native acceptance continues to use observed cooldown spend.
At most 600 point/direction cast attempts trace incoming and queued native
24-byte targets. Inactive target/None payload words are not read. The native
queue layout is guarded by the executable fingerprint and three additional
Q/W/R instruction anchors; native pointers stay inside the worker hook.

No projectile flight/destination mutation is implemented or claimed verified.
The next test can distinguish a queued-aim change from a later native effect.
Ninja turns, wider FPS investigation, cursor and aesthetic work remain deferred.

## Validation

Automated checks: 152 probe tests plus 18 core tests, formatting and Clippy for
both crates, offline optimized DLL build, native fingerprint/anchors, package
asset hashes and zip-member comparison. Native gameplay/rendering remain
pending user verification. Exact build/install hashes are in `dist/build-0.32.0.json`.
