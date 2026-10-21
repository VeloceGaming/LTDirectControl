# Migrating to game 0.6.5

What was established for 0.6.5, the live-branch update of 2026-10-21. The
method is the runbook in [version-migration.md](version-migration.md); the
previous record is [migration-0.6.4.md](migration-0.6.4.md).

Everything below is **read from the code** (static analysis of the
executable) unless a line says otherwise. Nothing here has been **seen in
game** yet. The baseline was seen in game (2026-10-21, mod 1.5.0-dev21): on
0.6.5 the mod refused to hook ("fingerprint differs"), and a match ran to
the result screen.

## Builds

| | 0.6.4 | 0.6.5 |
| --- | --- | --- |
| Executable SHA256 | `a1d05045…57439582` | `daf040db8a3a3ae136f3ef8ce704e2718f049656b6b5e4872eacb599811d30b5` |
| PE timestamp / image size | `0x6acf3112` / `0x533f000` | `0x6ad85238` / `0x5309000` |
| SDK | ABI 10 | ABI 10 (identical apart from the version text and a frozen `abi_level_10.txt`) |

Preserved and hash-verified under `research/game-builds/0.6.5/` (private,
not in git).

Supported from 1.5.0-dev22: 0.6.4 (players can still roll back to it on
Steam) and 0.6.5. 0.6.3 and 0.7.0 Beta 1 (both ABI 9) were dropped, so
skill charges can come from the official ABI 10 cooldown read. 0.7.0 Beta 2
shares 0.6.5's game logic: its stop event and the shop's next-step
function are instruction-for-instruction equal. Its combat functions,
renderer and UI differ; its migration is a separate step.

## How the tables were found

1. **The draft.** `tools/draft_version.py --old 0.6.4 --new 0.6.5` mapped
   682 functions and left 69 distinct addresses unknown.
2. **A bigger function map.** Seeding `tools/map_functions.py` with the
   215 whole-function pairs the draft had placed exactly, plus the
   functions found by hand, mapped 1216 functions. In the largest table
   the "likely" entries fell from 188 to 62.
3. **Every carried code address aligned.** All 483 entries with an old
   and a new address in functions: the old instruction lands exactly on
   the drafted address, with no failures. The six inside the two largest
   functions (15k and 24.6k instructions) were aligned one by one.
4. **Every unknown resolved by hand.** By callers and callees, byte
   searches (with the moved field where one moved), the vtables that hold
   a method, and aligned call order. The draft's `next_step` candidate
   was wrong: both shop decisions call `0xf0c160` where they called it.
5. **Byte patterns re-read.** All byte patterns were re-read from the
   executable, and the minimap's 54 operands checked: 37 aligned in the
   renderer; 17 in the identical input handler, at the same offsets.
6. **Verified.** `tools/verify_native_profile.py` passes for both versions
   against the source tree.

## Changes the hooks depend on

- **Player record grew by 0x20.** The move starts at about `+0x308`:
  - owned items `+0x310/+0x318` became `+0x330/+0x338`;
  - build plan `+0x350/+0x358/+0x360` became `+0x370/+0x378/+0x380`
    (`+0x370` is inferred from the (capacity, pointer, length) layout; the
    other two are read);
  - gold `+0x868` became `+0x888`;
  - the item AI's settings owner `+0x7f8` became `+0x818`.

  The shop's reads are now per-version constants (`PLAYER_ITEMS_*`,
  `PLAYER_PLAN_*`). The gold getter and the item AI player method are
  byte guards; their new bytes carry the new offsets.
- **Fields that did not move.** These are unchanged in every aligned use:
  - champion entity fields: action, position, attack queue, cooldowns
    `+0xb0..0xc8`, skill objects `+0x578..0x5a0`, level `+0x5c0`, actor
    `+0x5b8`, stack `+0x648`, position `+0x658/0x660`;
  - viewer `+0x2f0/+0x3f0`;
  - camera config `+0x45/0x46/0x4b`;
  - in-game UI `+0x9150/0x9234`.

  The attack function (230/230) and the input handler (1509/1509) are
  instruction-for-instruction equal. Input event tags and the key at `+8`
  are unchanged. Item stat getters copy the same `0x120`-byte block.
- **Simulation events changed format.** Each element was `0x38` bytes with
  a `u64` tag (stop = 20); it is now `0x70` bytes with a `u8` tag
  (stop = 18). The mod only calls the game's own stop and cancel-recall
  functions, with the same two arguments, so nothing changes.
- **A second worker send.** The hooked caller `0xb073c0` is the twin that
  loads `[rcx+0x28]`; its sibling stores 0 there. It still sends at
  `0xb078f9`. A drain loop over new code (`0x1adcc00`, 5682 instructions,
  no 0.6.4 counterpart) also sends at `0xb07b35`, with identical
  arguments. The author chose to hook both (`WORKER_EXTRA_SITES`). When
  the loop runs during a match is not known from the code. The
  `PATCH_READBACK` line names both sites.
- **Functions the mod calls itself keep their arguments.**
  - The outline's float (`0x2025a40`) and colour (`0x2024f60`) setters,
    confirmed by their aligned calls in the renderer: the stack frames
    grew, but the registers and stack argument are the same.
  - The tooltip info lookup (`0x1683ba0`): same four arguments and two
    returns. It now holds a reference-counted value and drops it through
    the unchanged `0x28b20`.
  - The stop event (`0x157e7b0`), cancel recall (`0x157f420`), and
    Hammerer W's skill methods (method table `0x3b8fab0`).
- **Unit renderer.** 15 calls (0.6.4 had a second identical copy of one
  caller, now gone); the renderer is `0x2557a00`.

## Athlete stat rules: what 0.6.5 changed

Off on 0.6.5 in 1.5.0-dev22; on again from 1.5.0-dev23 (35 sites, "How
the rules hook 0.6.5" below).

**Seen in game** (the author, 1.5.0-dev23 on 0.6.5, one match as Sun Mage,
rules On, Verbose log, 2026-10-21):
- `STATRULE installed sites=35`; all four rules fired (Dodging, Accuracy
  marker, collision, gate); no errors. The rules carried on after a death
  and respawn, which gave the champion a new id.
- Only the area gate fired (1535 per-shot lines). The projectile gate,
  both AI paths and the fifth gate have not fired in game yet.
- The athlete's effective Accuracy was 100. The game's own gate would have
  been 17 to 131 ticks (median 83); the band (floor 6, ceiling 14) gave 9
  to 13. Against Dodging 78 (gap 22) the game gave 17 to 21, the band 9 to
  11. So with the rules On, enemies react to the player's area skills
  about a second sooner than in the plain game: the input for the
  hit-rate refinement in backlog item 15.

- **Effective stats.** Accuracy and Dodging are multiplied by a
  per-athlete factor at `+0x288` in thousandths. Effective Accuracy below
  50 is pushed down further, to `50 − 1.9 × (50 − A)`.
- **The factor.** It is set in the input phase (`0x1620794`) from two
  rolls; each failure lowers it to `1000 − 9 × shortfall`, the lower of
  the two counts, and it is reset to 1000 under some condition
  (`0x16205e4`). It is never above 1000.
  - **Guess:** the rolls follow Focus and Mental, matching the changelog's
    "loss of focus" and "attribute penalties".
- **Dodging producer.** `0x1620b0d imul rcx, rsi, 0xc8`: effective Dodging
  `× 200`, was `× 150`.
- **Accuracy producers.** `0x12fbc8b` (stored at `+0xa8`), `0x1761657`
  and `0x17617e2` (stored at `+0x2d8`), still `× 70` of effective
  Accuracy.
- **Hit tests.** All 21 pass the stat verifier's field check unchanged.
- **Reaction gates.** Projectile `0xfb5990`, area `0xf38320`, and the big
  AI function's two paths in `0xfa4190`.
  - The bounds lost their fixed +2/+6 floors and gained small linear
    terms; at full exposure they still give 60 and 120.
  - A new term, `(1.2 × A − 60) × q / 100`, delays reactions to accurate
    casters and speeds them up against inaccurate ones.
  - The per-player seed moved to `+0x818`.
  - At the old capture point Accuracy and Dodging are no longer in
    registers; the projectile gate holds them at `0xfb5a6d` (`eax`, `rdi`).
- **A fifth gate.** `0xf9fb30` read no Accuracy in 0.6.4 and was left
  alone; in 0.6.5 it reads the caster's effective Accuracy (50 for a
  non-athlete caster), with an age-mixed seed. The author chose to hook
  it.
- **A new helper, `0x15cf120`.** It computes the bounds for one caller
  that takes their midpoint. **Guess:** the AI estimating how fast a
  target would react when it plans its own shot; it is not a reaction to
  the player's shots, so it stays vanilla.
- **Author's choice.** The mod's reaction band uses the effective values.

### How the rules hook 0.6.5 (1.5.0-dev23)

**Read from the code**; each field is worked out from the instructions
again by `tools/verify_stat_sites.py`.

- **The gates roll in a band.** Every gate ends the same way:
  `gate = low + (range + 1) × splitmix(seed) / 2^64 + extra`, then compares
  it with the time since the cast. That is the mod's own `reaction_gate`
  formula. In three of the five gates the final value is formed by an
  instruction a jump lands on and is compared at once, so 0.6.4's way
  (replace the final gate, `Complete`) has no safe place.
- **Two sites per gate instead:**
  - **`Inputs`**, where the gate holds the caster's effective Accuracy and
    the defender's effective Dodging: for the controlled caster it records
    them.
  - **`Band`**, at the load of the hash constant: for a recorded caster it
    sets `low` and `range` to the human band and `extra` to 0, so the
    game's own roll lands in it.

  | Gate | Inputs | Band (seed, low, range, extra) |
  | --- | --- | --- |
  | projectile `0xfb5990` | `0xfb5a81`: Accuracy `eax`, Dodging `rdi`, projectile `rbx` | `0xfb5b7c`: `rcx, r8, rax, r9` |
  | big AI function, path 1 | `0xfa4cb9`: Accuracy `edx`, `100 − Dodging` at `[rbp + 0x5a0]`, projectile `r14` | `0xfa4d81`: `r8, r9, rax, rcx` |
  | big AI function, path 2 | `0xfa50ea`: Accuracy `edx`, `[rbp + 0x5a0]`, caster id `r13` | `0xfa51c1`: `r9, r10, rax, r8` |
  | area `0xf38320` | `0xf3843d`: Accuracy `eax`, Dodging `r14`, id mixed with the seed in `rdi` (seed at `[rsi + 0x818]`) | `0xf38537`: `rcx, r8, rax, r9` |
  | fifth `0xf9fb30` | `0xfa1d2c`: Accuracy `eax`, Dodging `rsi`, caster id `r14` | `0xfa1e57`: `rcx, r8, rax, r9` |

  In both AI paths the `mov reg, 100` near the hash caps Accuracy; it is
  not where `100 − Dodging` is formed, which was worked out once before
  the loop. In the fifth gate the record that holds the player seed is
  overwritten at the `mov edx, 100`, so the inputs are read two
  instructions earlier.
- **Seeds.** In the area gate, path 2 and the fifth gate the bits from 40
  up are the time the cast started (now minus the time since the cast),
  fixed for each cast. The game rolls with its own seed, so each cast gets
  its own point of the band.
- **Producers.** The projectile Accuracy producer now stores at frame
  `+0xa8` (the draft carried 0.6.4's `+0xa0`), and a jump lands on all
  three producers' first instruction (the Accuracy-at-least-50 path). A
  jump onto a producer runs the hook and needs nothing the instruction
  before did, so the verifier allows a branch onto the first instruction
  for producers only. The Dodging producer is
  `imul rcx, rsi, 200` (`48 69 ce c8 00 00 00`).
- **Census.** The stat census now also knows the band-shaped gates and
  `× 200`: on 0.6.5 it finds the 35 sites exactly (5 band gates, 3
  Accuracy producers, 1 Dodging producer, 21 hit tests). An `Inputs`
  site is chosen by hand.

## The screen delay (1.5.0-dev22b)

**Seen in game** (the author, 1.5.0-dev22 on 0.6.5, verbose log):
- What was on screen was not what the simulation did: the controlled
  champion seemed to walk on its own, outlines missed champions and
  monsters, and the shop opened at 1:48.
- In the log, the controlled athlete's input ran 900 ticks (15 s at 60
  ticks per second) ahead of the frames shown.

**Read from the code:**
- The hooked worker caller (`0xb073c0`) builds a frame queue before its
  loop (`0x1ad9c20`) whose length is `15 × [settings + 0x1308]`, read at
  `0xb074cd`.
- Each new frame is pushed (`0x1ada770`) and only the oldest is released
  once the queue is longer than that (`0x1adcc00`); the second worker send
  drains the rest.
- On the way, events of kinds `0x19/0x1a` are sorted into queues, and a
  helper (`0x1adc940`) scans the queued frames within that window.
  **Guess:** the look-ahead lets match chat comment on a fight just
  before it is shown (0.6.5's "adjusted when chat appears").
- The value at `+0x1308` is a small option the game also compares with
  2, 7 and 8, not the tick rate.
- Before any frame was shown, the simulation ran the first 15 seconds
  unpaced: the mod's pacing acts when a frame is sent.

**The fix (the author's choice: always):** `VIEW_DELAY_SITES` rewrites
the length load at `0xb074cd` to `xor edx, edx` plus a 5-byte nop, so the
queue releases every frame at once and the screen shows the live
simulation. Done for every match on screen while the mod is installed, so
match chat loses its head start even when the player only watches. A more
graceful answer is backlog item 18.

## Not settled

- The two built-in description tables that changed most (0.6.4
  `0x3c13438`: Q aligns 0.23; `0x3c15338`: about 0.6). **Guess:** Hunter
  is one, as the patch notes expanded Hunter's descriptions. Check their
  tooltips in game.
- Research builds only:
  - the live reader's Ice Mage R (`0x19524b0`, 0.38) and Swordman R
    (`0x1537f40`, 0.43) are weak matches;
  - the preview-table guards use the first of several identical method
    tables, as on 0.6.4.
