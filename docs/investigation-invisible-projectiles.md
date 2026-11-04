# Investigation: a homing shot fizzles on an invisible target (2026-10-29)

The question (the author): in the base game, when a shot aimed at one unit
is already flying and the target goes invisible (by a skill or a bush), the
shot does no damage. Where does the game do this?

**Seen in game** (the author; build and game version not recorded): it
happens most with Soldier W (Three-Round Burst) on Nightmare, when Nightmare
uses W before the bullets land.

This is a read-only study of games 0.6.4 (`research/game-builds/0.6.4`),
0.6.5 (`research/game-builds/0.6.5`) and 0.7.0 release
(`research/game-builds/0.7.0`, captured 2026-10-28). Everything is **read
from the code** unless a line says otherwise. Nothing here is seen in game
beyond the author's report above. Addresses are RVAs of the version named.

## Verdict

The game checks the target **when the shot arrives**, not while it flies.
For the two homing shot kinds (6: one target; 7: one target plus a splash),
if the target is an enemy whose invisible counter is above zero at that
moment, the whole effect list of the shot is skipped: damage, crowd control
and buffs. The shot still ends. The check is an explicit comparison in the
hit code, so it is written on purpose; whether the developers meant it as a
dodge is a **guess**.

The bush case is **not explained**: the check reads only the invisible
counter, and no bush code was found that raises it (section 4).

## 1. In flight

**Read from the code (0.6.4 `0x13e25a0`, 0.6.5 `0x1304050`, 0.7.0
`0x1d53360`; all 3026 bytes).** The per-tick projectile update switches on
the projectile's kind (`+0x40`). Kinds 6 and 7 share one case (0.6.4
`0x13e276a`):

- the target's actor id is at `+0x50`, the speed at `+0x48`;
- the target is looked up through game method slot `+0x208` (0.7.0
  `0x16acd40`), a plain id lookup with no visibility test;
- the shot moves one step toward the target's current position
  (`+0x658`, `+0x660`). Within one step it snaps onto the target and is
  marked finished (`+0x130 = 1`). If the target no longer exists, it is
  marked finished at once.

So the shot keeps chasing an invisible target to the end.

## 2. On arrival

**Read from the code (hit dispatcher 0.6.4 `0x13da020`, 0.6.5 `0x12fbb00`,
0.7.0 `0x1d4af80`).** The dispatcher switches on the kind again (index =
kind − 2; 0.7.0 table cases: kind 6 `0x1d4b1d7`, kind 7 `0x1d4b6e7`). For a
finished kind 6 or 7 shot:

1. Look up the target (`+0x50`). None: no effects.
2. **Gate A, `CastingTarget::check_projectile`** (name from the older
   symbol dictionary `research/libgame_core-symbols.tsv`; 0.6.4
   `0x1a65630`, 0.6.5 `0x176cd90`, 0.7.0 `0x167ddd0`; arguments: the shot's
   casting target `+0x12c`, the shot, the target). False if the target's
   `can_target` byte (`+0x6b1`) is not 1 or its `block_target_tick`
   (`+0x698`) is not 0, for every team; then the casting-target rules
   (Enemy, Ally, EnemyChampion and so on) against the teams. False: no
   effects.
3. Look up the caster (`+0xf8`). Not found: apply. Same team as the
   target: apply. Otherwise **gate B: apply only if the target's
   `invisible_tick` (`+0x690`) is 0.**
4. The flag gates the loop over the shot's effect list (0.6.4
   `test r14b` at `0x13dc9fc` for kind 6, `test dil` at `0x13dd00b` for
   kind 7; 0.7.0 `test r15b` at `0x1d4d625`, `test dil` at `0x1d4d7f8`).

Gate B sites (the compare, then the `sete` that makes the flag):

| | 0.6.4 | 0.6.5 | 0.7.0 |
| --- | --- | --- | --- |
| Kind 6 | `0x13da2d1` | `0x12fbe06` | `0x1d4b283` |
| Kind 7 | `0x13da806` | `0x12fc314` | `0x1d4b793` |

Bytes on 0.6.5 and 0.7.0 (identical): kind 6
`49 83 be 90 06 00 00 00 41 0f 94 c7` (`cmp qword [r14+0x690], 0;
sete r15b`), kind 7 `48 83 bb 90 06 00 00 00 40 0f 94 c7`
(`cmp qword [rbx+0x690], 0; sete dil`). A search of each executable for
`cmp qword [reg+0x690], 0` followed by `sete` finds exactly these two in the
hit code (0.7.0 has one more, `0x2f53462`, in unrelated view code).

0.6.4 was traced end to end. On 0.6.5 and 0.7.0 the sites, the target
check and the gates have the same instructions and layout; the path from
the `sete` to the gate was matched by layout, not traced step by step.

Consequences, all read from the code:

- Invisibility that ends before the shot lands: the shot hits.
- A shot at an invisible ally (a heal, a buff) still lands.
- If the caster has died, the shot lands even on an invisible enemy.
- An effect that raises `block_target_tick` makes gate A fail for every
  team. One effect apply raises both counters (0.6.4 `0x150f3d0`, which
  writes `+0x690` and `+0x698`). Which data effect that is (Banish?) is a
  **guess**.

## 3. The author's case: Soldier W on Nightmare

- **Read from the data (0.7.0 bundle, `nightmare` champion file).**
  Nightmare W is a `MoveTo` dash (range 60000, speed 5600, so about 11
  ticks) whose `end_effects` are `CasterInvisible` for 150 ticks, the
  vanish view effect, an animation and a 40000-radius `RangeEffect` attack.
  The invisibility starts when the dash ends.
- **Read from the code (0.7.0 Beta 1 `0x17c6440`).** Soldier has no
  champion file; its skills are native. `CHANGELOG.md` (the 0.7.0
  development notes) records Soldier Q and W as the single-target
  projectile apply. On Beta 1 that apply builds a projectile with kind 6,
  the speed at `+0x48` and the target id at `+0x50`, and hands it to game
  method slot `+0x1b0`. The release build's version of this apply was not
  located; its dispatcher has the same kind 6 case.
- So each of the three bullets chases Nightmare through the dash and is
  judged on landing: bullets that land before the dash ends hit, bullets
  that land after Nightmare vanishes do nothing.

## 4. Bushes

**Read from the code (0.6.4).** The direct writers of `+0x690` are effect
applies that raise it to `max(current, ticks)` (`0x14dfd70`, `0x14e7220`,
`0x150f3d0`, `0x17c8560`, one in `0x1a57f30`), entity constructors and a
view copy. None is in the world tick or the sight builder. A unit in a bush
is hidden through each team's sight grid and its per-team seen record
(`docs/investigation-vision-ward.md`), which gate B does not read.

Limits: the search finds only stores with a direct `+0x690` displacement,
and the per-tick countdown of the counter was not found either.

**Guess:** a bush case may be a champion whose bush skill or passive grants
real invisibility (the executable has `hunter_bush_skill` and
`hunter_bush_attack`), or Nightmare landing W in a bush. Open.

## 5. Which shots and which invisibility (2026-10-29)

**Superseded 2026-10-29** (the paragraph below is wrong for the champions
with a data file): "Not settled. The author said on 2026-10-16 that a basic
attack is not a projectile (`docs/investigation-stat-rules-0.6.4.md`
section 7). If so, basic attacks never reach gate B. **Guess**, not checked
further."

**Read from the code and the data (0.7.0):**

- The data effect `TargetProjectile` builds a kind 6 shot (its apply
  `0x1944540` stores 6 as the kind; the Beta 1 apply `0x17c6440` read in
  section 3 is the same code). Only 12 base champions have a champion data
  file; 8 of them use `TargetProjectile` as their **basic attack**:
  Alchemist, Astrologer, Bombardier, Crossbowman, Ferryman, Harpooner,
  Hypnotist, Sand Mage. Their basic attacks reach gate B.
- The other base champions (60 or more, Soldier among them) are defined in
  the game's code. Five native functions call `0x1944540` directly
  (`0x1760ec0`, `0x17746c0`, `0x1d58b60` Hitman's ult, `0x1d6f1f0`,
  `0x1e20ad0`). Which native basic attacks and skills build kind 6 or 7 is
  not listed. No base data champion uses the splash shot (kind 7).
- Effects that raise only `invisible_tick`, so gate B is the only thing
  stopping the shot: `Invisible` (`0x175c080`), `CasterInvisible`
  (`0x1766c40`, Nightmare W), one effect beside Hitman's code (`0x1d6a000`;
  **guess**: Hitman's), and the SDK's `entity_set_invisible` (`0x33cfd80`).
- Effects that raise both `invisible_tick` and `block_target_tick`, so gate
  A also stops the shot, for every team: `Banish` (`0x175a960`) and
  `CircusBladeUltVanishEffect` (`0x1d87380`). This settles the 0.6.4
  **guess** in section 2 ("Which data effect that is (Banish?)"): on 0.7.0
  there are two, Banish and Circus Blade's ult vanish.
- Effects were named through their Debug text: the function that writes the
  name, the method table that holds it, the next slot (the apply).

## 6. If it is changed

**Decided 2026-10-29 (the author):** a separate mod, "Invisible not
Invincible" (`D:\LTTM2\InvisibleNotInvincible`, game 0.7.0 only), takes
the smallest form below for every shot. Gate A stays, so Banish and Circus
Blade's ult vanish still stop shots. Not seen in game yet.

The options as written before the decision:

- What it buys: shots on an invisible enemy land as they would on a visible
  one. Gate A (untargetable, block targeting) stays.
- The smallest form makes the flag 1 at the two gate B sites, per version.
  Every shot of every match changes, background simulations included
  (season statistics follow the AI's pre-simulation).
- A player-only form needs a hook with a caster check, so AI-versus-AI stays
  vanilla.
- No official mod interface call reaches this; it is a native change, found
  again on each game patch.

## 7. Open questions

1. Build and game version of the author's observation.
2. The bush case: which champions, and whether the target had a stealth or
   bush skill.
3. Which native champions' basic attacks and skills use kind 6 or 7 shots
   (the 8 data champions' basic attacks do: section 5).

## 8. How the sites were found

The game method tables were located as in the vision ward study
(`docs/investigation-vision-ward.md` section 8); slot `+0x208` is the
entity lookup. The 0.6.4 projectile update was the one named in
`docs/investigation-line-hit.md`; its homing case calls
`check_projectile`. A scan of the update, the dispatcher and their direct
callees for `+0x690` reads found the two gate B compares. The 0.6.5 and
0.7.0 sites came from a byte search for the compare-then-`sete` pair, then
the callers of each dispatcher (`tools/trace_pe.py --call`). Tools:
`tools/show_code.py`, `tools/trace_pe.py` and scratch scripts.

Note: the 0.7.0 column of the hit-code addresses in
`docs/investigation-stat-rules-0.6.4.md` (for example `0x14d6ef0`) was
written against a 0.7.0 beta, before the release snapshot; on the release
executable those addresses are unrelated functions.
