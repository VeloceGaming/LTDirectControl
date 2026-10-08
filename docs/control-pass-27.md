# Cast intent and native readiness

The client captures a world aim or an eligible unit identity at press/confirm.
Unit picks use the same shared body-envelope ranking as movement/hover. A miss
cannot become a hit because another unit moves under the old cursor. Directional
casts use the saved world point minus the caster position at dispatch; a nearby
champion never replaces a directional point in this mod's resolver.

One intent replaces the previous one. Native validation always runs before a
cast is dispatched. A fresh busy action (native states >2) retains an eligible,
learned, charged request for at most one second. Because an action can start
after the last observer sample, an otherwise eligible rejection receives one
hold tick to refresh its action state, then is dropped unless busy/approaching.
Cooldown/unlearned rejections do not wait for later readiness. Holding preserves
attack/cast states: the existing stop hook changes only movement action 2.

For a targeted skill beyond effect range plus target radius, the intent follows
that same visible target using native Move input, then submits the skill as soon
as native validation accepts it. Approach ticks refresh the intent lifetime;
busy ticks do not, even when the target remains distant. Native target conditions
still apply. Recently-attacked-only targets have no stable query and therefore
do not start an unvalidated approach. Ground skills retain native range clamping.

All accepted casts, misses and cancellations use generation checks against new
client commands. RMB/A/B/S/Esc, loss of focus, death, handback and session reset
clear intents. Shift previews are persistent; rejected confirmations stay open.
Optional release casting snapshots on key release; confirmation consumes its
release latch to prevent a second cast on subsequent key-up.

## Verified read-only native calls

Existing 0.6.2 Q/W/R consumers at RVAs 15c4980/15b2d20/15c3560 establish action
objects at EntityData+580/+590/+5a0 and adjacent vtables. During the existing
selected entity borrow only, Action methods +90 and +a8 return base cooldown and
use count with `(action userdata, EntityData/Stat)` Windows x64 arguments.
No object, vtable or entity pointer is retained or mutated.

Q capacity is `max(3, base*100/max(1,100+CDR))`; W/R minimum is 1.
CDR is +400; R also adds +46c. These are taken from the actual consumers, not
assumed from the stable ABI. Unlearned actions are not called. Cost is integer
capacity/use-count. Consumers accept cooldown <= capacity-cost and spend cost.
The HUD therefore displays `(capacity-cooldown)/cost` available uses, and the
time until cooldown reaches capacity-cost, rather than dimming on any cooldown.
The native validator remains the authority even when metadata says ready.

All twelve previous executable/branch anchors remain checked. There are no new
patched branches. Native method calls are new, so user gameplay testing includes
mod champions. Mock function tests verify reader arithmetic, argument layout,
level gating and absence of entity writes; they do not prove native gameplay.

Body picking now covers idle/run/hit/attack/skill/ult body tags and preparation
tags, excluding separately tagged effects/end frames. It retains the previously
tested map scale and remains an envelope. It does not claim current pose/frame
alpha coverage, facing-aware weapon bounds or exact rendered-frame timing.

The Sunna result audit was already verified read-only against four game records:
18/3/0, 14/7/2, 7/7/3, 5/1/2 versus Dplus Kia (series 94, 3-1). This pass does
not alter match saving. Save/reload persistence has not been tested separately.
