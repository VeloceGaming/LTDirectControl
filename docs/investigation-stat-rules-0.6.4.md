# Athlete stats in combat, game 0.6.4

Research date: 2026-10-15. Status: **read-only research pass on game 0.6.4**;
nothing was built or played for it. Game 0.6.5 changed the stats' scale,
the Dodging factor and the reaction gates: see "Athlete stat rules: what
0.6.5 changed" in [migration-0.6.5.md](migration-0.6.5.md). Findings only; the design they feed is
recorded in [backlog.md](backlog.md) ("Athlete stat rules in manual
matches"). This extends
[investigation-athlete-stats-manual-control.md](investigation-athlete-stats-manual-control.md)
(0.6.3 and 0.7.0) and
[investigation-derived-collision-prototype.md](investigation-derived-collision-prototype.md).

Every finding is tagged as AGENTS.md asks: **Read from the code** (static
analysis of the 0.6.4 executable), **Read from the data** (the 0.6.4
`bundle.game_data`), **Seen in game** (the mod's own log lines from played
matches), or **Guess**.

Executable: `research/game-builds/0.6.4/TeamfightManager2.exe`, SHA-256
`a1d05045405a12e2…` as recorded in `tools/native_profiles/0.6.4.json`. All
addresses are RVAs in that file. Tools: `tools/show_code.py`,
`tools/compare_functions.py` (function alignment), `tools/trace_pe.py`
(callers), and the preview reader's word layouts in
`probe/src/native_adapter/windows/preview.rs` to decode logged effect dumps.
Never reuse an address from this document in another game version.

## 1. The 0.7.0 stat sites carried to 0.6.4

**Read from the code.** Every site the withdrawn 1.2.1 build patched on 0.7.0
(branch `backup/1.2.1-stat-rules`) was carried by aligning its enclosing
function with the 0.6.4 candidate found by byte search. 30 of 33 sites align
with identical bytes; the other three are explained in the table. A scan of
the whole `.text` section for the patterns `sub r64, [r64+0x6a0]`,
`imul r64, r64, 0x46` and `imul r64, r64, 0x96` finds no consumer on 0.6.4
that 0.7.0 does not have (one extra `sub` hit at `0x100434a` indexes an
unrelated table with the value 1 − field and is not an entity).

Reaction gate sites (the hash constant load and the range-select tail):

| Role | 0.7.0 | 0.6.4 | Function (0.6.4) |
| --- | --- | --- | --- |
| Projectile gate, capture / complete | `0xf93db6` / `0xf93e32` | `0xf27386` / `0xf27402` | `0xf272c0..0xf27426`, 92 instructions, aligns 1.00 |
| Big AI function, path 1 | `0xf83180` / `0xf83209` | `0xf169e2` / `0xf16a6b` | `0xf15f90..0xf1b15b` |
| Big AI function, path 2 | `0xf834d0` / `0xf8355d` | `0xf16d4f` / `0xf16ddc` | same |
| Area/casted-effect gate | `0x108d3c1` / `0x108d43d` | `0xe51451` / `0xe514cd` | `0xe51360..0xe514e5`, aligns 1.00 |

Collision sites, grouped by the shape of the hit test (section 3):

| Shape | 0.6.4 sites | Function |
| --- | --- | --- |
| A, margin clamped at 5000 | `0x10457ac`, `0x10458f4` | `0x1045710` |
| A | `0x1045d0c`, `0x1045e64` | `0x1045c70` |
| A | `0x104635d`, `0x104658a` | `0x10461f0` |
| A | `0x1049bc1`, `0x1049d1d` | `0x1049b50` |
| A | `0x13db810`, `0x13dbba5` | `0x13da020` (the second site uses `r14`, 0.7.0 used `rdi`) |
| A | `0x13f8f39` | `0x13f8f10` |
| B, body radius clamped at 0 | `0x1045134`, `0x1045384` | `0x1044f60` |
| B | `0x13d2441`, `0x13d2685` | `0x13d2260` |
| B | `0x13d2b82`, `0x13d2fa1` | `0x13d27a0` |
| B | `0x1a5a882`, `0x1a5ae36`, `0x1a5afe6`, `0x1a5b680` | `0x1a57f30` |

Producers of the derived values:

| Producer | 0.6.4 | Note |
| --- | --- | --- |
| Accuracy × 70, stored to a frame slot | `0x13da155` (store `+0xa0`) | function `0x13da020` |
| Accuracy × 70 | `0x1a59f78`, `0x1a5a0b4` (store `+0x2d8`) | function `0x1a57f30`; 0.7.0 stored to `+0x2d0`, hence the failed byte match |
| Dodging × 150, stored to champion `+0x6a0` | multiply `0x1741a7f`, store `0x1741a86` | input phase `0x1740e00..0x1746f33` |

## 2. The reaction gate

**Read from the code.** `0xf272c0` is a predicate: given the AI context and a
projectile, has the projectile been in flight long enough for this athlete
to react to it? Decoded:

```text
D = defender Dodging (player+0x1e8), capped at 100
A = caster Accuracy (player+0x1f0), capped at 100; 50 if the caster is not a player (tower, minion)
q = max(100 − D, 0)
lo = 2 + floor(58·q²/10000)
hi = 6 + floor(114·q²/10000)
seed = (projectile id (+0xf0) << 20) xor player seed (+0x7f8)
gate = lo + splitmix64(seed) mod (hi − lo + 1)
extra = floor(q · floor(12·A²/10000) / 100)
react if (current tick (sim+0x1b8) − projectile tick (+0x120)) >= gate + extra
```

The arithmetic equals the 0.7.0 reconstruction in the earlier document
(function aligns 1.00). The choice is deterministic per projectile and
defender, not rolled each tick.

**Where it counts from. Read from the code.** `+0x120` on the projectile is
written at creation from the simulation tick: linear projectile apply
`0x1a6e7b0` stores it at `0x1a6eb4e`, parabolic apply `0x1a05ee0` at
`0x1a06101`, and the AI feature builder (below) uses `current − [+0x120]` as
the projectile's age at `0xf20d9f`. So the gate starts at spawn, after the
skill's wind-up (`start_timing`). During the wind-up there is no projectile
and the gate plays no part.

**A global switch. Read from the code.** A byte at `0x504e180` (`.data`,
value 1 in the file) is read before every gate (`0xf272dd` and nine other
places in `0xf11c70`, `0xf15f90`, `0xf1b8e0`, `0xf1fa50`). Zero would make
every gate return true at once. No writer was found; it is a compiled-in
constant.

**Who calls it. Read from the code.**

```text
0xf272c0 gate  <-  0xef5cb0 (collects the projectiles the athlete may react to, with squared distance)
               <-  0xf1fa50 (builds 5 slots × 14 features per threatening projectile)
               <-  0xf15a50, 0xf1b8e0  <-  0xd6f960 (native AI think, 27 KB)
0xe51360 area gate  <-  0xdd1fb0, 0xf97e50  <-  …  <-  0xf87a70 (native AI)
```

The feature builder normalises each value to 0..1000 or 0..30000 (relative
position ÷ 320 clamped ±3000, direction, time, size, age). The bundle ships
`macro_weights` and an `rlp` asset. **Guess:** the dodge movement is chosen
by a learned policy fed these features, not by a scripted side-step. Whether
the policy also receives Accuracy or Dodging as features was not checked.

Everything above sits under the native AI's think function. The SDK
replacement for the controlled athlete runs after it (earlier document), so
no gate ever acts on the player's own champion.

## 3. Two shapes of hit test

**Read from the code.** The 21 collision sites subtract the target's derived
Dodging value (`+0x6a0`) in one of two ways. Shape A (11 sites) adds a
margin to the body and clamps the margin at 5000:

```text
reach = body radius + max(Accuracy·70 − Dodging·150, 5000)
```

Shape B (10 sites) applies the stats to the body radius itself and clamps
the whole at zero (`0x13dbba5` shows the A form: `cmp rcx, 0x1389;
cmovl rcx, 0x1388`; the B sites have no such clamp):

```text
reach = max(scaled body radius + Accuracy·70 − Dodging·150, 0)
```

**Superseded 2026-10-16** (read from the code, 0.7.0, with the same
instructions at the 0.6.3 and 0.6.4 sites): the scaling is the other way
round. Shape A scales the body: when the target's dword at `+0x468` is not
zero the radius `+0x678` becomes `radius · (+0x468 + 100) / 100`
(**guess:** a size change in percent, e.g. from a buff). Shape B adds the
plain radius `+0x678`. Both read the shot's value as an option (a tag of 1
in front of the value); without one the margin is 0 in both shapes. So:

```text
shape A: reach = scaled body + (option ? max(value − Dodging·150, 5000) : 0)
shape B: reach = option ? max(body + value − Dodging·150, 0) : body
```

Internal units are 1000 per game unit; a champion body is about 10 units
(SDK note). Accuracy adds 0 to 7 units, Dodging removes 0 to 15. Under
shape A the margin is 5 units for almost every real pairing. Under shape B a
10-unit body against Dodging 100 reaches 0 units unless the caster's Accuracy
exceeds about 70: only a shot through the exact centre registers. The four
shape-B functions include `0x1a57f30`, which also holds two Accuracy
producers and sits in the effects module next to the projectile apply
functions. **Guess:** `0x1a57f30` is the skill projectile hit test; which
skill types pass through which site was not established statically and
needs the prototype's in-game observer (backup branch).

## 4. The AI's aim

**Read from the code.** The aim evaluator the mod already hooks
(`AIM_CORRECTION_CALL`, target `0x1a69ad0..0x1a6a4f4`) reads the caster's
Accuracy at `0x1a69bce` and the target's Dodging at `0x1a6a151`, then rolls
a random number in 0..10000 (`0x1abd010`) against

```text
P(lead) = A² · (10000 − D²) / 10000 / 10000  =  (A/100)² · (1 − (D/100)²)
```

Accuracy 0 skips the correction outright. When the roll passes, the shot is
aimed at where the target will be when the projectile arrives: the travel
time comes from distance ÷ speed, a moving champion (entity tag 15, action
2) is advanced along its path through the steering routine `0x1a633e0`, and
a dash or rush (actions 4, 5, 6) is advanced to its end point; the result is
clamped to the cast range and written at `0x1a6a487` / `0x1a6a492`. When
the roll fails, the original aim stands, which for the native AI is the
target's current position. For the controlled athlete the mod gives the
routine a copy, so the cursor aim is never changed (`combat.rs`,
`evaluate_aim`).

Consequence, **read from the code**: a target's Dodging already lowers how
often AI shooters lead it, and a shooter's Accuracy raises it. Dodging 100
is never led. This is independent of the collision and reaction effects.

## 5. Wind-up and travel times

**Read from the data.** The champion info sheet (`asset/base/setting/
champion_info`, 68 champions) carries `start_timing`, `duration`, `range`
and `cooltime` for every skill. Projectile speed is in the data only for the
8 data-defined champions; the other 60 champions' effects are native objects
whose constants live in the executable. Rows with a projectile:

| Champion | Slot | Effect | Wind-up (ticks) | Range | Speed (units/tick) | Travel (ticks) |
| --- | --- | --- | --- | --- | --- | --- |
| nightmare | Q | LinearProjectile | 10 | 46000 | 6500 | 7.1 |
| nightmare | R | Native projectile | 4 | 80000 | 6800 | 11.8 |
| strongman | Q | Native | 12 | 30000 | 4000 | 7.5 |
| strongman | R | Native | 10 | 90000 | 6000 | 15.0 |
| harpooner | Q | Native | 14 | 90000 | 7000 | 12.9 |
| harpooner | W | Native | 16 | 110000 | 7500 | 14.7 |
| harpooner | R | Native | 10 | 130000 | 7000 | 18.6 |
| alchemist | R | LinearProjectile | 18 | 92000 | 6500 | 14.2 |
| alchemist | Q | ParabolicProjectile | 16 | 70000 | travel_time 28 | 28 |
| crossbowman | Q | Native | 18 | 150000 | 6000 | 25.0 |
| crossbowman | W | Native | 14 | 150000 | 6000 | 25.0 |
| crossbowman | R | Native | 20 | 150000 | 6000 | 25.0 |
| spellbreaker, sand_mage, alchemist Q | – | area effects after a delay | 10..20 | – | – | delay 8..20 |

**Seen in game** (mod 1.2.3 on game 0.6.4, `PREVIEW TREE` lines in the
author's log, decoded with the preview reader's layout: word 1 speed, word
15 radius, word 16 range of the native LinearProjectile `0x1a6e7b0`;
parabolic word 18 travel time). Actor ids in the log are not mapped to
champion names, so this is the distribution, not a per-champion table:

| Speed (units/tick) | Radius | Range | Travel (ticks) | Distinct skills seen |
| --- | --- | --- | --- | --- |
| 30000..40000 | 2000..6000 | 70000..300000 | 1.8..3.3 | 6 |
| 15000..20000 | 2500..5000 | 70000..120000 | 4.0..8.0 | 5 |
| 8000..12000 | 750..8000 | 70000..180000 | 7.5..22.5 | 12 |
| 5000..6000 | 3000..6500 | 46000..300000 | 9.2..50 | 7 |
| 1600..3500 | 2800..4300 | 90000..116000 | 33..56 | 2 |
| parabolic | – | 70000..150000 | 28, 60 | 2 |

Champion movement is about 0.9 game units per tick (stat `move_speed` 900,
shown by the mod's stats panel as units per tick × 60 ÷ 1000). Clearing a
shot sideways needs roughly body radius plus projectile radius, about 15
units, or about 15 ticks of movement. **Read from the data, conclusion:**
after spawn, a shot with less than about 15 ticks of flight cannot be
dodged by reaction whatever the gate; the gate matters for the slow third
of the table (Crossbowman Q at 25 ticks, two ultimates at 34, the slow
skills at 50..56). Faster shots are dodged, if at all, during the wind-up
or by chance.

## 6. Enemy projectiles through the official interface

**Read from the code (SDK).** `mod-api-stable/src/sim.rs` exposes
`projectile_count()` and `projectile_at(index)` returning
`ProjectileInfoV1 { x, y, caster_id, team, is_end }`. No direction or speed
field; both follow from two consecutive positions. **Guess:** this is
enough to draw an incoming projectile's path one tick after spawn without a
new native hook; whether the list is filtered by the viewer's visibility
was not checked.

## 7. Which shots reach which hit test (2026-10-16)

**Seen in game** (mod 1.2.7 development build on the 0.7.0 beta, the
author's play with the rules On; `STATRULE trace` lines). The 0.7.0 sites
pair one to one with the 0.6.4 ones above (same table order, functions
aligned): **read from the code**.

The hit code that starts with the projectile Accuracy producer
(`0x13da020` on 0.6.4) is the projectile hit dispatcher: its argument is a
projectile object, and it branches on the projectile's kind (`+0x40`). The
entity function with the other two producers (`0x1a57f30`) branches on the
entity's action state (`+0x300`) and handles hits that are not
projectiles. **Read from the code.**

| Shot | Producer, word | Hit test reached | Shape |
| --- | --- | --- | --- |
| Crossbowman Q | projectile, kind 2 | 0.7.0 `0x14cfdb1` (0.6.4 `0x13d2441`) | B |
| Crossbowman W | projectile, kind 2 | 0.7.0 `0x10c9c44`, `0x10c9e94` (0.6.4 `0x1045134`, `0x1045384`) | B |
| Harpy R | entity, state 4 | 0.7.0 `0x1557792`, `0x1558570` (0.6.4 `0x1a5a882`, `0x1a5b680`) | B |
| Crossbowman, a skill (not settled which) | projectile, kind 6 | none of the shape-B tests | A (or missed) |
| Pyromancer Q | projectile, kind 3 | none | A (or missed) |
| Pyromancer W | projectile, kind 4 | none | A (or missed) |
| Crossbowman, one shot (R?) | projectile, kind 1 | none | A (or missed) |

- Shape A is not hooked, so "A" means "produced, then no shape-B test":
  a miss looks the same. **Read from the code** (0.6.4 dispatcher): kinds 3
  and 4 branch to shape-A functions (`0x1045c70`, `0x1049b50`), which
  agrees with Pyromancer.
- Which skill a line belongs to comes from the slot whose cooldown last
  restarted. Basic attacks restart theirs all the time, so `last_used=attack`
  says nothing about the shot, and a line after two quick casts can name
  the wrong skill (the kind 1 line).
- **Rejected** (2026-10-16): an earlier reading of these lines took the
  kind 6 shot for Crossbowman's basic attack, from `last_used=attack` alone.
  The author: a basic attack is not a skillshot or a projectile.
- This explains the 1.2.6 logs: the entity hit tests never saw a marked
  shot because only projectile skills had been used (superseding the open
  question in the 1.2.6 changelog, 2026-10-16).
- Projectile kinds seen from other casters on the same thread: 1, 2, 3, 4,
  5, 6, 10; entity states 3 and 4.

**Seen in game, cause unknown:** the author noticed small stutters and
sudden speed-ups in these test matches (0.7.0 beta, rules On). Nothing in
the Normal log marks them.

**Seen in game** with 1.2.9 and 1.2.10 (0.7.0 beta, the author, practice
tool; trace lines and the readout):

| Shot | Producer, word | Shape-B test reached |
| --- | --- | --- |
| Bomber Q | kind 2, then kind 3 18 ticks later | the kind 2: `0x10c9c44`, `0x10c9e94`; the kind 3: none |
| Dynamo Q | kind 2 | `0x10c9c44`, `0x10c9e94` |
| Alchemist R | kind 2, then kind 8 13 ticks later | the kind 2: `0x14cfdb1`; the kind 8: none |
| Alchemist Q | kind 10 | none (the author: shape A or missed throughout) |
| Pyromancer Q, W, R | kinds 3, 4 (R: not in the log) | none (the author: all shape A or missed) |

- **Rejected** (2026-10-16): that kind 2 is "a projectile that sweeps
  through targets", and then that it is "a projectile that hits the first
  target and is gone". The author: Crossbowman Q is the latter, Bomber Q,
  Dynamo Q and Alchemist R's first part are not. A kind does not stand for
  one behaviour, and no pattern between kinds and preview shapes is
  claimed.
- **Read from the code** (0.7.0, `0x10ca220`): a hit test runs for each
  candidate target before the overlap test (`0x14eef10`), so "reached a hit
  test" means a target was checked against the shot, not that it was hit.
- The trace writes each producer and word once (per thread, among the last
  64), so a later skill with an already-seen kind leaves no line: the
  readout, not the log, shows every shot.

## 7a. The shape-A floors are hooked (2026-10-16, mod 1.2.11)

Why: the "Collision margin" setting reached only shape B. Shape A raised
every marked shot back to 5 units at its floor, so with the default margin
of 0 the player's shape-A shots had 5 units more room than the shape-B
ones. Mapping skill by skill to shape would never end (modded champions);
hooking the 11 floors makes the margin apply to every shot without knowing
the skill. **Read from the code** (0.7.0; the same instructions at the
0.6.3 and 0.6.4 sites, call paths not re-traced there):

- Each shape-A site is `sub r, [target + 0x6a0]`, then `cmp r, 0x1389`,
  then a `cmovl` to 5000 (or `mov r11d, 0x1388; cmovge r11, rax` at the
  last one). A search of the whole executable for that pair finds exactly
  11 per version, the 11 of section 1.
- The hook replaces the compare. For a marked shot (the register plus the
  target's Dodging is 5000, which Accuracy · 70 never is) it sets the
  register to the chosen margin and the flags to "not below", so the floor
  keeps it. The bridge already saves and restores the flags.
- The mark reaches all 11: 10 are in functions the projectile dispatcher
  (`0x14d6ef0`) calls with `&[rbp + 0x98]`, the option the projectile
  producer writes (tag at `+0x98`, value at `+0xa0`); `0x14d8647` and
  `0x14d8999` read that slot directly. `0x1500250` gets it two pointers
  further (`0x14d8259` → `0x10c5290` → `0x1500220`); of its two call paths
  the first was followed to the end, the second only in part.

| 0.7.0 | 0.6.4 | 0.6.3 | Register, target |
| --- | --- | --- | --- |
| `0x10ca2c3` | `0x10457b3` | `0x103bac3` | rcx, r15 |
| `0x10ca40b` | `0x10458fb` | `0x103bc0b` | rcx, r13 |
| `0x10ca823` | `0x1045d13` | `0x103c023` | rcx, r15 |
| `0x10ca97b` | `0x1045e6b` | `0x103c17b` | rcx, r13 |
| `0x10cae75` | `0x1046365` | `0x103c675` | rcx, r12 |
| `0x10cb0a2` | `0x1046592` | `0x103c8a2` | rcx, r12 |
| `0x10ce9c8` | `0x1049bc8` | `0x103fed8` | r14, r15 |
| `0x10ceb24` | `0x1049d24` | `0x1040034` | r14, r13 |
| `0x14d8647` | `0x13db817` | `0x1262947` | rcx, rdi |
| `0x14d8999` | `0x13dbbac` | `0x1262cdc` | rcx, rdi (0.6.4 and 0.6.3: r14) |
| `0x1500250` | `0x13f8f40` | `0x1281050` | rax, rdi |

With the hook, a marked shot's reach is the target's body plus the chosen
margin in both shapes, except that shape A uses the scaled body (section
3). The readout now names the shape of the first hit test that checked a
target, or says "no hit test". Nothing of this is seen in game yet.

## 8. Open items

1. ~~Which collision shape (A or B) each skill type uses.~~ **Superseded
   2026-10-16:** not needed once the shape-A floors are hooked (section 7a);
   the rules no longer depend on the shape.
2. Whether the learned dodge policy receives Accuracy or Dodging as
   features (feature builder `0xf1fa50`, 7720 bytes, read only in part).
3. Whether `projectile_at` lists projectiles outside the player's vision.
4. Actor-id-to-champion mapping for the logged native speeds (the roster
   log line was not in the inspected log).
5. A fifth gate-shaped hash in each version (0.7.0 `0xf803f6`, 0.6.4
   `0xf13d1d`, 0.6.3 `0xe13a5d`; found by the stat census of
   `tools/verify_stat_sites.py`, 2026-10-16). **Read from the code:** the
   same splitmix, squaring, `>> 0x2d` and `shr 0x11; add` ending as the
   four hooked gates, but no Accuracy read (`+ 0x1f0`) in the 45
   instructions before it. **Guess:** a timing roll on other inputs. Not
   examined.
