# Ability controls, recall and attack-range aiming, probe 0.20

## Basic-attack aiming

A enters persistent attack-move aiming. Releasing A keeps the range circle;
valid battlefield left-click submits attack-move and dismisses it. Right-click,
S, Esc, recall and ability commands cancel it. Camera navigation and Tab preserve
it. Pause clears it. Champion-only targeting does not restrict attack-move.

The existing native consumer borrow copies the cached basic-attack Effect at
EntityData+490. The refresh/attack consumers in the previously inspected 0.6.2
code establish that cache; a bounded excerpt is preserved in
research/attack-range-current.txt. No additional hook or native function call is
used. Range uses current level growth and entity range bonus, checked arithmetic
and the same 250-ms metadata freshness rule as skill previews. Missing/stale
metadata hides the circle. It shows effect reach, without guessing target-body
radius adjustments; actual attack validation stays native.

Q/W/R correspond to native Skill/Skill2/Ult. A physical key-down edge without
Shift records one cursor aim; the selected player's next SDK AI callback checks
the request using `is_valid_input` and returns that native input if accepted.
Rejecting a skill discards it and retains manual movement/combat ownership.
There is no cooldown/animation queue. Quickcast requires no confirmation click.

Shift + press Q/W/R enters persistent aiming. Releasing either key keeps the
display; a battlefield left-click captures that click's current world position
and requests one native cast. Acceptance dismisses aiming; rejection keeps it
without automatically retrying. Right-click, Q/W/R quickcast, A, B, S, Esc,
focus loss, release, death or match change cancels aiming. Shift + another skill
switches it. Camera navigation and Tab leave it active. Actual HUD/minimap clicks
do not confirm. Confirmation is reserved from the attack-move click handler;
physical mouse edges remain tracked to prevent a held click becoming a new order.
A request generation check rejects a cast superseded during native validation.
Cast range uses the live effect's
base range + (level-1)*growth range + entity range bonus. Point/direction lines
are aim aids; effect impact shapes are not inferred from cast range.

## Literal inspection and native reads

The installed SDK exposes skill input submission and validation, entity queries
and cooldowns, but no read-only getter for an existing champion's effect spec.
The installed bundle was read as counted extension/path/payload records; champion
settings establish that native targeting categories vary and built-in action
types often omit explicit casting fields. Those settings are not a hardcoded
per-champion dispatch table.

Read-only Capstone disassembly of the installed 0.6.2 executable verifies the
native Input jump table at RVA 3b8ca18: Move/Return/Attack/Skill/Skill2/Ult branch
to 162f23b/162f482/162f36c/162f391/162f27b/162f4a8. The attack branch CALL at
162f387 invokes 15b2700 with EntityData, InputTarget reference and events
reference, returning unit. Skill/Q invokes 15c4980, W invokes 15b2d20 and R
invokes 15c3560. Native action states 3/4/5/6 correspond to attack/Q/W/R.

Native refresh routine 15c1640 stores four 56-byte Option<Effect> values at
EntityData+490 (attack), +4c8 (Q), +500 (W), +538 (R), obtained from action
objects +570/+580/+590/+5a0. Within each effect, base range is +10, growth range
+18, start timing +20, casting target +28, attack type +2c, casting type +30.
Casting -1 means absent. The Q/W/R consumers read the same fields, level +5c8
and entity range bonus +438; ground-target action positions are natively clamped
to their range. These scalar offsets are verified against current code, not
assumed to match the C ABI EffectSpecV1 layout. Research is preserved in
research/ability-native-current.txt and tools/read_ability_native.py.

The existing move-consumer hook and one additional attack CALL observer copy
those scalar fields while the native caller owns a valid EntityData borrow.
A selected SDK command ticket must match actor, foreground match and active
worker. Native entity pointers, Arcs and vtables are never retained. The
observer does not alter attack execution. The move hook's movement-only stop
behavior is retained. Metadata is copied on move/attack ticks and expires after
250 ms; a skill tick is followed by the normal persistent move/attack/hold input,
which refreshes it. A form changing between that observation and the next cast
can cause a rejection; fresh native validation remains authoritative.

The fifth branch is installed at title only after full executable SHA256,
loaded PE identity, all branch bytes and native entry header checks. It joins
the same transactional page protection, cache flush, readback and periodic
audit as the four working branches.

## Targeting and limits

Targeting type 0 resolves native-valid sprite hits in cursor-distance/ID order.
Alive, targetable allies and own-team-visible enemies are copied through SDK
queries. Native validation decides team/champion/CC/recent-hit/range/cooldown
restrictions for each candidate. AllyOnlySelf type selects the actor directly.
Position type 1 submits the captured world cursor, Direction type 2 submits
caster-to-cursor vector, and None type 3 submits InputTarget NONE. A missing
active effect cannot cast. Unit hit geometry uses the existing collision-radius
plus screen-space margin policy, not recovered native sprite hit testing.

Previews draw on the UI surface using the same verified pan/zoom transform as
world commands. Segments outside the battlefield or over minimap are skipped.
The ring is a nominal cast range, not a guarantee that a skill can cast or hit:
native collision radii and per-target effect range adjustments still apply.
Probe 0.19 adds the champion-only direct-target toggle and unlock shading to the
confirmed skill/health/gold/item HUD. See targeting.md. Damage-area shapes and
manual shopping remain future work; see docs/player-hud.md.

The user confirmed 0.15 on Lancer. Its Q descriptor is Targeting=0,
EnemyWithoutTower=6, range35000: empty-ground Q rejection matches native targeting.
Broader champion, form/recast and directional coverage remains pending.

## Native recall

B uses SDK InputV1::return_home once per physical press, with native validation.
It clears the previous movement/attack order. Subsequent idle ticks submit Move
at the actor's current position; inspected native Move returns without changing
recall in this case. Holding B does not repeatedly send Return. An unavailable
request is consumed with a notice; it does not retry or restore AI ownership.

Read-only disassembly in research/recall-native-current.txt shows Return at
15b2c70 sets action kind 1 and its channel timer, and the native action tick
dispatcher retains recall progression/completion. Native Move at 15c4210 leaves
action 1 intact for a current-position goal. No duration or teleport is invented.

Explicit movement, attack-move, S and Esc can cancel action 1. The existing
selected-actor Move/Attack hooks emit the native cancellation event (17069c0)
and set action to idle before forwarding/stopping the requested input. Only
recall action 1 is changed; its timer, effects and committed casts are untouched.
The routine's entry header is verified alongside all existing anchors before
installation. No additional branch is patched. Accepted Q/W/R commands retain
their native recall interruption behavior; rejected casts do not cancel a
running channel. Damage/CC and completion remain native.

See TESTING-20.md for the new session/range test; TESTING-16.md records the
earlier persistent ability aiming, native recall completion and cancellation.
