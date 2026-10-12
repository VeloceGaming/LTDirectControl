# Athlete stats during manual control

Research date: 2026-10-12. Status: research and 1.2.1 implementation; gameplay verification pending.

Active build: user-installed 0.7.0 beta 1. The final continuation below supersedes
the earlier unresolved scheduling findings. Historical 0.6.3 addresses remain
explicitly labelled for comparison.

## Question and reproduction

The user compared Crossbowman Q with low Skillshot Accuracy (3/100) and a much
higher value, keeping champion, team, opponent and aim comparable. The opponent
appeared to dodge far more successfully against the low-accuracy athlete.
This is user-observed behavior, not an instrumented deterministic replay.

## Evidence and version boundaries

The earlier pass inspected game 0.7.0. The subsequent read-only pass found the
installed game had changed to 0.6.3, running mod 1.2.0. Addresses below are RVAs,
not absolute process addresses. Never reuse them across versions.

0.6.3 executable SHA256:
`F21CF691799A83D9AFA0CCAE3E2B95862F5780446A501EE97860B91B752DBE2F`.
Installed executable matched `research/game-builds/0.6.3/TeamfightManager2.exe`.

Method: static PE/Capstone inspection with `tools/trace_pe.py` and
`tools/show_code.py`; older named SDK assembly in
`research/hud-core-full.s.lto.hud-player-old.s` supplies field/function names.
Older SDK formulas and layouts are clues, not proof of the current binary.
No game execution, build, or native gameplay verification was performed.

## Our manual override

`probe/src/simulation.rs`, `StablePlayerAi::think`, constructs its own input and
does not use the native `_base` command to choose the controlled player's command.
An idle living champion returns a move-to-current-position command. Invalid input
can deliberately cancel manual control and restore native AI.

`probe/src/native_adapter/windows/combat.rs`, `evaluate_aim`, gives the original
native evaluator a copy of the target, then preserves the user's original manual
direction/point. This bypasses native aim correction, not all stat consumers.
Attacks and skill execution still call native routines.

## Confirmed combat consumers

### Skillshot Accuracy: enemy response and collision allowance

0.7.0: projectile reaction function `0xF93CF0` reads defender Dodging at player
offset `0x1E8` and caster Accuracy at `0x1F0` through projectile caster identity.
Another projectile path at `0xF83149..0xF8322A` contains the same arithmetic.
For stats in 0..100, the reconstructed calculation is:

```text
A = min(caster Accuracy, 100)
q = max(100 - defender Dodging, 0)
lo = 2 + floor(58*q*q/10000)
hi = 6 + floor(114*q*q/10000)
extra = floor(q * floor(12*A*A/10000) / 100)
delay = deterministic_hash_choice(lo..hi) + extra
```

This is a native response gate; the values are simulation ticks. It is not an
observed reaction measurement for the user's particular enemy. At Dodging 50,
Accuracy 3 produces 16..34 ticks and Accuracy 100 produces 22..40 ticks.

0.7.0 collision function `0x14D6EF0`, including branch `0x14D8630`, combines
caster Accuracy and defender avoidance adjustment, with a minimum clamp.

0.6.3 independently confirmed: native casting at `0x16F8D04..0x16F8D1C` and
`0x16F8E40..0x16F8E58` reads capped Accuracy and multiplies by 70. Collision
branches at `0x1262940` and `0x1262CD5` subtract target field `0x6A0` from an
incoming allowance and clamp to at least 5000. The current build's full
projectile reaction-delay arithmetic has not yet been re-derived.

The code establishes mechanisms consistent with the user's report. It does not
prove which collision/reaction branches a particular Crossbowman Q took live.

### Last Hitting: finishing remaining HP

0.6.3 `0x1273A20..0x1274A14`, identified through older symbols as
`simulation::apply_attack_inner`:

- `0x12748E0`: reads attacker player Last Hitting (`+0x1E0`).
- Caps at 100; `0x12748F4` shifts right by one.
- `0x127490D`: reads target HP (`+0x668`).
- Decrements the comparison value; compares it to the finishing threshold.
- `0x127491C`: writes zero HP when positive remaining HP is within the threshold.

```text
threshold = floor(min(Last Hitting, 100) / 2)
if eligible && 1 <= remaining_hp <= threshold:
    remaining_hp = 0
```

Examples: stat 3 -> 1 HP; 50 -> 25 HP; 100 -> 50 HP. There is a target eligibility
flag and entity-type predicate. Exact eligible classes and summon-owner details
remain unresolved; do not generalize this to all champion damage or all attacks.
There is no manual-control exemption in the observed finishing branch.

### Dodging: persistent champion collision adjustment

0.6.3 input-phase function `0x1465E80..0x146C223`, identified through older SDK
assembly as `mode::MobaBrain::run_input_phase`:

- `0x1466ACA`: reads player Dodging (`+0x1E8`) and caps at 100.
- Resolves that player's champion.
- `0x1466AFF`: multiplies by 150.
- `0x1466B06`: writes champion `nontarget_avoid_range` (`+0x6A0`).

Native collision consumers subtract this field and apply their minimum clamp.
Thus manual movement alone does not remove the stat-based collision adjustment.
The clamp means different Dodging values need not produce different outcomes.

## Other stats: leads, not confirmed manual effects

Concentration (`player+0x208`) and Mental (`+0x210`) feed an
`ai_interface::awareness_lapse_state` helper in the native input phase. This
establishes native perception processing, not manual input delay or altered aim.

Older named SDK methods include Control Speed `input_delay_min/max`, Positioning
accuracy/error ranges, Judgement latency, and Order/Roaming/Aggression/Ego ratios.
Their current callers and position relative to the SDK override need tracing.
Byte-pattern searches are incomplete and are not evidence that a stat is absent.

## Next checks and policy

1. Trace Control Speed and the SDK override scheduling boundary.
2. Resolve Last Hitting's exact eligibility and summon ownership.
3. Distinguish AI-only perception/decision effects from combat resolution effects.
4. Agree on a manual-control policy before implementation. Do not set all stats
   to 100: that grants native finishing and collision advantages.
5. Validate any eventual change with user-run low/high-stat comparisons; preserve
   opponents' ordinary AI behavior and restore normal behavior on AI handoff.

Only this research note is authorized for writing in the current pass. No code
edits or builds are authorized.

## Continuation: SDK replacement and scheduling boundary

The 0.6.3 SDK source provides a useful independent contract:

- `research/game-builds/0.6.3/mod-sdk-stable/mod-api-stable/src/ai.rs:230`
  describes player AI as per-tick and says the callback writes `out_input` on
  Replace.
- `mod_shim.rs:1419..1438` calls the mod's `think`; Some(input) writes the
  input and returns Replace, while None returns Pass. Our idle hold therefore
  matters: returning None would preserve a native command rather than stop it.

Historical 0.6.3 native input-phase instruction chain:

| RVA | Observed operation |
| --- | --- |
| `0x1468726` | Native AI virtual call, slot `+0x78`; result at stack `+0x4030` |
| `0x1468729..0x1468847` | Looks up the per-player callback collection and loops it |
| `0x1468889` | Calls a callback through internal trait slot `+0x40`, passing context and a copy of the current command |
| `0x146888D` | Sentinel `-1` means no replacement; continue callback loop |
| `0x14688DC` | Checks a replacement using the simulation interface's input-validation slot `+0x298` |
| `0x14688EC..0x1468904` | Accepted replacement overwrites the current command at stack `+0x4030` |
| `0x1468A1F..0x1468A88` | Appends nonempty final command to the current input list; 40-byte entries consist of player identity plus command |
| `0x146AF30..0x146AF63` | Iterates that list and calls `0x16F2660` for each entry |

`0x16F2660` is a hash-map insertion/update routine receiving identity and command.
Do not mistake it for attack execution. Likewise `0x11D5400` called later at
`0x146AFDF` copies a vector; it is not the native command executor.

The callback chain is consistent with the SDK replacement contract and happens
after the native AI's get-input result. No Control Speed read or stat-based
waiting branch was observed between replacement acceptance and storing the final
command. This supports bypassing a delay inside native AI decision-making; it
does not by itself prove the whole downstream execution chain has no delay.

`0x11D8FC0..0x11D9C2F` contains the known movement and attack call sites used by
our mod (`0x11D9154`, `0x11D9200`). Its initial validity call is `0x1439450`.
No direct Control Speed field read was found in either routine. Reads at player
offsets `0x1E8/0x1F0` around `0x11D9735` feed instrumentation counters; those
reads alone do not establish a command modification. The callers connecting
stored input to this executor remain to be resolved.

### Control Speed: what was and was not established

Older named SDK assembly defines:

```text
input_delay_min = 400 + 3 * max(100 - Control Speed, 0)
input_delay_max = 1200 - 4 * min(Control Speed, 100)
```

The units and current consumers are not established. These numbers must not be
presented as milliseconds, ticks, or a measured manual input delay. Searching
for the old arithmetic at player offset `0x200` and parameter offset `0x80`
did not identify a current consumer. This is a negative search result, not proof
that Control Speed is unused or that all indirect/copied stat accesses were
covered. Native AI implementation code is only partly available as named
assembly; compiler inlining and layout changes remain possible.

### Last Hitting eligibility: additional bounded result

At `0x1274702`, the 0.6.3 executable reads entity type at `+0x68`, transforms
the tag and tests mask `0x72`. Tags 0, 1, 3, 6, 7, 8 take the eligible branch;
champion tag 15 does not. The current binary independently checks champion tag
15 at `0x1265E0B`. This narrows the earlier finding: the finishing effect's
target-type predicate excludes champions. Names for every accepted current tag
and any additional attack eligibility conditions still need confirmation.

### Remaining work after this continuation

1. Resolve the map-to-entity execution dispatch and any delay there.
2. Identify current native AI Control Speed consumers rather than relying on
   old method names or arithmetic.
3. Trace Concentration/Mental lapse outputs into the context to determine whether
   any affect simulation state outside native AI perception.
4. Resolve the remaining finishing eligibility and credit/owner rules.

Recommendation remains a targeted manual-only policy, not global stat mutation.
No production code or build output changed in this continuation.


## Continuation: installed 0.7.0 beta 1

### Build identity and research scope

The installed executable at
`C:/Program Files (x86)/Steam/steamapps/common/Teamfight Manager2/TeamfightManager2.exe`
is 91,006,464 bytes and matches the preserved
`research/game-builds/0.7.0/TeamfightManager2.exe` byte-for-byte by SHA256:

`6C9CD659E727B4130766241E9D43B222454A8455609B40F9BF73C01D5136A94F`.

The last existing probe log reports 0.6.3. That is an older run and is not
runtime evidence for beta 1. All addresses in this continuation are RVAs in
this verified 0.7.0 executable. No production source edits, builds, game
launches, or gameplay verification were performed.

Important tooling limitation: `PeResearch.callers()` requires an unwind-table
entry for the referring function. Tiny leaf jump thunks can have no such entry;
therefore an empty caller result can miss a real virtual dispatch. Raw relative
call/jump candidates were decoded locally, then their vtable pointers checked.
Likewise, a pointer table from the stable SDK interface is not interchangeable
with the native core simulation interface.

### Native AI decision timer: Control Speed resolved

The native AI vtable at `0x3E5DD48`, slot `+0x78`, points to leaf thunk
`0xFFA5B0`, which jumps to get-input implementation `0xFDCAC0`.
At `0xFDCB62..0xFDCB71`, an unexpired next-decision tick causes a None result.
At `0xFDCB76..0xFDCBEF`, the function reads Control Speed (`player+0x200`),
samples an inclusive integer range using `0x10637B0`, divides by 100, and stores
current tick plus that interval at native AI field `+0x44E8`:

```text
S = Control Speed
lo = 400 + 3 * max(100 - S, 0)
hi = 1200 - 4 * min(S, 100)
next_decision_tick = current_tick + floor(random_inclusive(lo, hi) / 100)
```

| Control Speed | Sample range | Decision interval |
| --- | --- | --- |
| 0 | 700..1200 | 7..12 ticks |
| 50 | 550..1000 | 5..10 ticks |
| 100 | 400..800 | 4..8 ticks |

Correction to an intermediate spoken update: stat 0 is **7..12**, not 4..12.
These are native AI scheduling intervals, not measured manual input latency.
Another Control Speed consumer at `0x1047E90`, inside native AI decision routine
`0x10478D0`, uses the maximum-delay arithmetic for an additional gate. Its exact
behavioral label is not established; it is not the common final command dispatch.

### Replacement-to-execution chain now resolved

Input-phase function: `0x1745420..0x174B28A`.

| RVA | Observed operation |
| --- | --- |
| `0x1747D1F` | Native AI get-input virtual call, slot `+0x78` |
| `0x1747E89` | Per-player mod callback, internal trait slot `+0x40` |
| `0x1747E8D` | Replacement sentinel test; `-1` preserves the existing result |
| `0x1747EDC` | Validates a replacement through native simulation slot `+0x298` |
| `0x1747EEC..0x1747F04` | Accepted replacement overwrites current command |
| `0x1749FDA` | Calls `0x18826C0` to update the identity/command map |
| `0x174A05F` | Calls `0x145F360` to copy the final input vector |
| `0x174A092` | Calls `0x186AD50` to shuffle inputs |
| `0x174A0E0` | Loads native simulation command-execution slot `+0x2A0` |
| `0x174A0F0..0x174A15A` | Iterates 40-byte entries and executes nonempty commands; call at `0x174A14E` |

Native core simulation vtables `0x3EB08F8` and `0x3F04DD0` both resolve:

- `+0x298` -> `0x1718FC0`, input validation.
- `+0x2A0` -> leaf thunk `0x17162A0` -> `0x1463780`, command executor.

The command executor starts with validation at `0x14637C8`, then dispatches
movement, attack and skill inputs. It contains our existing movement and attack
call sites `0x1463914` and `0x14639C0`.

The SDK callback still runs after native get-input returns None because of the
Control Speed timer. Our `StablePlayerAi::think` ignores `_base` when constructing
manual input, reads `ctx.sim()` and actual entities/visibility, and provides an
idle hold command for the living controlled champion. Accepted replacements
reach the executor in this same input phase. No Control Speed delay was found
in this traced common path. This resolves the earlier map-to-executor gap; it
does not prove every champion's native skill execution is independent of every
athlete stat.

### Combat stat effects remain in beta 1

The previously documented 0.7.0 Accuracy reaction and collision consumers apply
to this exact executable. Replacing aim does not remove the caster stat from the
opponent's response calculation or native collision allowance.

Dodging was independently relocated in the current input phase:

- `0x174605A`: read Dodging from `player+0x1E8`, capped at 100.
- `0x174608F`: multiply by 150.
- `0x1746096`: write champion collision adjustment at `+0x6A0`.

This runs outside the manual command replacement. Native collision paths consume
the adjustment with their minimum clamp; it is not merely AI movement advice.

Last Hitting was independently relocated in attack processing:

- `0x14F4A4B`: read Last Hitting from `player+0x1E0`, capped at 100.
- `0x14F4A5F`: divide by two.
- `0x14F4A75..0x14F4A84`: compare remaining HP at `target+0x668` against the
  finishing threshold and write zero when eligible.
- `0x14F485B..0x14F487B`: target-type predicate with mask `0x72`; accepted raw
  tags remain 0, 1, 3, 6, 7, 8. Champion tag 15 is excluded.

The threshold remains `floor(min(Last Hitting,100)/2)` HP. The complete
eligibility and summon-owner credit rules are not yet established. This result
must not be described as a universal bonus against every target or every skill.

### Concentration and Mental: bounded perception finding

Current helper `0x16F52D0..0x16F560F`, matched to the older named
`awareness_lapse_state`, returns two boolean results. Its decoded body has no
calls and no non-stack memory writes.

Input-phase calls `0x1745EB4`, `0x1745F14`, `0x1745FE3` use the result to update
lapse/statistical counters (`player+0x558`, `+0x560`, `+0x568`). Later calls at
`0x174758F`, `0x17476BD`, `0x17477B9`, `0x17478BC`, `0x174799C` feed selection of
self/team awareness sources and construction of native perception context.
A potentially suspicious branch to `0x1748259` sets an awareness flag to zero
and rejoins at `0x1747A01`; it does **not** bypass the SDK replacement callback.

Our manual callback obtains entity lists and team visibility from StableSim,
not the native AI's chosen awareness source or native base command. The traced
lapse branches therefore establish native perception and counters, not a
manual input wait or aim alteration. They do not establish that all indirect
Concentration/Mental effects elsewhere in simulation have been excluded.
Older `AthleteParameter::update` also maintains morale state and match counters;
its old offsets/formulas are not promoted to current-build findings.

### Recommended next boundary

The confirmed problem is wider than native aim correction: Accuracy affects
opponent response and collision, Dodging affects collision, and Last Hitting
can finish eligible targets after damage. Control Speed, in contrast, gates
native decision-making and is bypassed by the traced manual replacement chain.

Do not solve this by setting all athlete stats to 100. That would preserve or
increase hidden combat advantages and could alter AI teammates/opponents.
Before implementation, agree on the manual-control rule for each confirmed
combat consumer, especially how enemy dodging should respond to a human-fired
projectile. Then scope changes to manually controlled actions/entities and
preserve ordinary AI behavior after handoff. Further work should establish
Last Hitting eligibility/ownership and inspect remaining decision-stat consumers
before claiming complete stat independence, particularly for Workshop champions.

Only this research document changed. Implementation requires a separate go-ahead.


## Design follow-up: modest stat influence with consistent collision

User clarified the goal: keep a modest, manageable stat advantage during manual
control rather than remove all stat influence. Their own Dodging can later unlock
an enemy-skill trajectory display; that future feature is outside this pass.
No reaction/collision implementation is approved by this research continuation.

### Native timing rechecked and quantified

Re-decoded both current projectile paths (`0xF93CF0..0xF93E56` and
`0xF82F95..0xF8322A`). Exhaustively evaluated their integer arithmetic over
Accuracy/Dodging 0..100 (10,201 pairs): the reconstructed formula above matches
the instruction arithmetic for every pair. This verifies the mathematical
reconstruction, not live skill behavior or hit rates.

| Defender Dodging | Accuracy 0: delay ticks | Accuracy 50 | Accuracy 100 |
| --- | --- | --- | --- |
| 0 | 60..120 | 63..123 | 72..132 |
| 25 | 34..70 | 36..72 | 43..79 |
| 50 | 16..34 | 17..35 | 22..40 |
| 75 | 5..13 | 5..13 | 8..16 |
| 100 | 2..6 | 2..6 | 2..6 |

The SDK documents 60 simulation ticks per second (`values.rs:513`). At Accuracy
50, the endpoints above correspond to 1.05..2.05 seconds for Dodging 0 versus
about 33..100 ms for Dodging 100. These are eligibility thresholds, not measured
movement onset. Other AI decisions, movement, crowd control and available escape
space still determine what actually happens.

Important details from the current instructions:

- The final comparison at `0xF93E38..0xF93E49` uses current simulation tick minus
  projectile field `+0x120`. The second path at `0xF8320F..0xF8322A` matches.
  This gates reaction against projectile age/recorded start; it is not a newly
  started countdown whenever the AI detects a projectile. Exact lifecycle of
  that start field remains to be followed for each projectile class.
- The hash is seeded by projectile identity (`+0xF0`, shifted 20) and defender
  identity-related field (`player+0x7F8`). The choice is deterministic for that
  pair, rather than a fresh random reaction delay on every check.
- Missing caster-player lookup falls back to Accuracy 50 (`0xF93D61` and
  `0xF83169`). That is a native fallback, not evidence that 50 is a balanced
  manual-control baseline.
- A global flag and projectile classification/identity checks can bypass this
  timing gate (`0xF93D0D..0xF93D27`). Its timing table does not apply universally
  to every projectile in every configuration.
- Because the Accuracy term is multiplied by `100-Dodging`, it disappears at
  Dodging 100. At Accuracy 0..28 its inner integer term is zero; low values are
  indistinguishable for this gate. This differs from a smooth opposed-stat rule.
- An additional native threat/effect path at `0xF8347E..0xF83566` repeats the
  arithmetic with a different time/identity seed. Its exact effect categories
  and timestamp semantics are not yet established. Changing only the small
  `0xF93CF0` helper would not cover all observed reaction calculations.

### Starting model (implemented in 1.2.1; gameplay tuning pending)

Use Accuracy and AI defender Dodging in one bounded timing adjustment. Keep
projectile/target collision independent of athlete stats. Preserve normal
physical consequences: range, projectile speed, width, terrain, CC and movement.

```text
A = clamp(caster Accuracy, 0, 100)
D = clamp(AI defender Dodging, 0, 100)
offset_ticks = floor((3 * (A-D) + 50) / 100) # nearest tick, ties toward +infinity
base_ticks = deterministic_pair_choice(12..18)
reaction_threshold = base_ticks + offset_ticks
```

The integer offset is -3..+3 ticks. For an otherwise identical threat, all stat
matchups therefore span at most 6 ticks / 100 ms. Changing one stat from 0 to
100 with the other held fixed shifts the threshold by at most 3 ticks / 50 ms.
The neutral 12..18-tick band is 200..300 ms; the union including both extreme
matchups is 9..21 ticks / 150..350 ms. Variation within the base band is separate
from the stat advantage. These are proposed tunable constants, not values
validated in play or an assertion about human reaction-time distributions.

| AI defender Dodging | Accuracy 0: proposed ticks | Accuracy 50 | Accuracy 100 |
| --- | --- | --- | --- |
| 0 | 12..18 | 14..20 | 15..21 |
| 50 | 11..17 | 12..18 | 14..20 |
| 100 | 9..15 | 11..17 | 12..18 |

Original implementation requirements (the implementation record below supersedes the approval status):

1. Agree on match scope. Prior recommendation was consistent collision for all
   participants in a direct-control match, leaving ordinary AI-only matches
   unchanged. This affects balance and is not yet an implementation decision.
2. Trace threat timestamp lifecycle and effect categories before selecting a
   hook. Do not reset a countdown each tick or alter explosion/damage timings.
3. Preserve pair-stable variation and native threat eligibility; change the
   response threshold only. Do not introduce per-frame random sampling.
4. Establish a fixed collision allowance that fits visible contact. Fixed does
   not automatically mean zero; native projectile and target geometry remain.
5. Test identical seeds/positions with low/mid/high Accuracy and opponent
   Dodging, short- and long-range shots, multiple simultaneous threats, CC, AI
   handoff and Workshop effects. The user performs gameplay verification.

Recommendation: discuss the 100-ms total stat budget and neutral timing band
first. Continue read-only tracing of timestamp/category coverage if approved;
production edits/builds still require an explicit go-ahead.


## 1.2.1 implementation record — 2026-10-12

The user approved bounded stat effects first, followed by match-start modes.
The hook tables were checked independently for 0.6.3 and 0.7.0 beta 1;
no address is shared between the two executable versions.

- Four reaction calculations per executable use eight paired instruction
  sites: helper, projectile loop, ground/effect loop and action helper. The
  early bridge captures the native identity/time seed and the two stat inputs;
  the late bridge replaces only the calculated eligibility threshold. Native
  eligibility branches and elapsed-time comparisons still execute. No
  projectile start timestamp, speed or damage schedule is written. The exact
  effect-category taxonomy remains incomplete; coverage here means the four
  stat calculations found in the binary, not a claim that all skills dodge.
- The base remains pair-stable through the native SplitMix64 seed mixing. No
  random generator is advanced. The consistent tie rule above gives a maximum
  three-tick shift from changing either stat across its entire 0..100 range.
  Rounded offsets are deliberately stepped, rather than a continuous curve.
- Twenty-one collision consumers per executable replace the temporary
  stat-dependent allowance with a fixed 5,000 internal units (5 game units).
  Some consumers need that delta alone; others need native entity radius
  (including the native size modifier where used) plus the delta. The tables
  distinguish them. This retains physical geometry without the athlete's
  Accuracy/Dodging changing it. The 5-unit allowance needs visual testing.
- Rules apply to all participants on the bound foreground worker while manual
  control is Running/Paused. Handing back to AI, starting as a spectator,
  leaving the battlefield and background matches retain native behavior.
  Athlete stats, entities and projectiles are never temporarily overwritten.
  Last Hitting and other athlete-stat mechanics remain unchanged. Showing
  enemy trajectories based on the controlled athlete's Dodging is deferred.
- Relays replay displaced instructions, preserve flags, all general registers
  and XMM state, and change only the intended result register. The 29 sites
  join the existing verified atomic patch plan; no function entry is patched.
  Invalid version/bytes fail installation. Version-profile verification checks
  whole displaced instructions and excludes control-flow/RIP-relative spans.

Validation includes exhaustive bounded/monotonic stat-policy checks, native
ABI bridge fixtures compiled for both profiles, and worker/ownership scope
checks. These tests do not verify the native game's hit behavior. Compare
Crossbowman Q at Accuracy 3/50/100 against fixed Dodging, then vary Dodging;
check visible contact, short/long shots, concurrent threats, CC, Workshop
skills and F12/F11 handoff. The user performs those gameplay tests.


### Unapproved 1.2.1 tuning revision

After the user's initial gameplay test, collision felt consistent but skillshots
were too easy to land. With permission, the next test lowers the base from
12..18 ticks to 9..15 ticks (150..250 ms). The opposed-stat shift stays -3..+3
ticks, giving an overall 6..18-tick / 100..300-ms band. Collision allowance,
projectile geometry, pair seed and native threat eligibility remain unchanged.
This is a test revision, not an accepted balance change. The earlier model and
table above describe the first 1.2.1 build. No changelog update is made yet.

The preparation spectator action is now a 48 px eye icon next to Start with
its existing translated tooltip; the full-width text row is removed.


### Unapproved startup/shop fix — 2026-10-13

Always play my role no longer waits for the chooser when manual shopping is
on. Starting manually grants one session-owned startup-shop claim, consumed
when the native build plan is ready. Existing Pause while open applies. Starting
as spectator grants no claim; Watch this match or any subsequent AI handoff
forfeits a pending claim. Takeover, recall and death cannot recreate it. A fresh
match resets the lifecycle. P remains an independent manual opening action.
Changelog stays untouched pending user acceptance; revised skill timing and
collision are unchanged by this fix.
