# Shop investigation (read-only, game 0.6.3)

Static disassembly of the preserved exact executable
`research/game-builds/0.6.3/TeamfightManager2.exe`. Nothing was patched,
built or run. In-game behaviour still needs the user's test.

## How the game buys

Player fields (same object the HUD already reads):

| Field | Meaning |
|---|---|
| `+0x310/+0x318` | owned items, Vec of 16-byte `dyn Item` (ptr, vtable) |
| `+0x350/+0x358/+0x360` | final-item target list, Vec<usize> (cap/ptr/len); entries are indices into the registered catalogue |
| `+0x868` | gold (u64) |
| `+0x800` | team (0/1) |

Catalogue: simulation settings `+0x30` -> boxed Vec of `dyn Item`. Item vtable
`+0x58` key, `+0x68` price, `+0x70` next-tier/upgrade data, `+0x80` next-tier
list, `+0x50` a boolean predicate (used when counting items).

Purchase execution lives in the simulation update `0x1465e80`:

1. For each player, the champion position (`+0x658/+0x660`) is compared to its
   team's base rectangle. Purchases only happen inside it.
2. The player's controller object is asked two questions through its vtable
   (native controller vtable at RVA `0x3b08bd8`):
   - `+0x88` "upgrade?" -> thunk `0xf60620` -> `0xf3f3a0`. Returns
     `{1, owned slot, item index}` or `{0}`.
   - `+0x80` "buy new?" -> thunk `0xf61b80` -> `0xf3f510`. Returns
     `(1, item index)` or 0.
3. The simulation then validates on its own:
   - upgrade: the chosen item must be listed in the owned slot's next-tier list
     (key comparison), and gold >= price;
   - new item: the number of owned items found in the enabled catalogue
     (`+0x50` is the enabled getter, per `native_items.rs`) must be <= 3, and
     gold >= price. This is the vanilla four-item cap (`cmp rax, 3` at
     `0x146baf3`; the same rule appears in `0xea1ea0`). The Riot item mod
     (`riot_items_tfm2.dll`, native) logs `patch_final_gate`, `patch_row_floor`
     and `patch_tick_cap`, so it evidently patches this gate at runtime to allow
     six. A shop must therefore keep the game's own validation, never re-check
     slot counts itself.
4. It executes: `sub [player+0x868], price` (`0x146b60b` upgrade,
   `0x146bbaf` new), replaces/pushes the owned item and runs the item's
   upgrade carry-over (`+0x78`, the SDK `on_upgrade`/`on_upgraded_from`).
5. This repeats each tick while in base, so "everything affordable at once".

Decision helpers (shared by the two questions and the AI's recall logic):

- `0xea1b90`: walks the target list in order against owned items; returns the
  first unfinished goal and which owned slot is on its path.
- `0xea1ea0`: picks the next step toward that goal and compares gold
  (`+0x868`) with its price (`+0x68`).
- `0xea1670` / `0xea2240`: "can I buy something now?" predicates used by the AI
  (e.g. recall decisions at `0xda5290`, `0xe0ac70`).
- Empty target list: falls back to `0xea1730` / `0xea23f0` (goal search),
  which is why AI-decided builds still work.

## Answers to the four questions

1. **Re-read every visit?** Yes. The target list is read live by every
   decision; nothing is cached per match. A changed list affects the very next
   in-base tick.
2. **Entry format?** Plain catalogue indices (usize), the same indices the SDK
   `decide_build` hook uses. Swapping one entry in place is a single aligned
   8-byte write; no allocation.
3. **Half-built parts on retarget?** Not proven statically. The goal matcher
   only follows owned items that lead to the current goal, so an abandoned
   component should simply stay in its slot and the buyer starts the new goal
   from the next free step. Combined with the "<= 3" new-item count rule, too
   many abandoned parts could block new purchases. Needs a logging test.
4. **When does buying happen?** Only while the champion is inside its own base
   rectangle, every tick, until nothing affordable remains.

## Options re-assessed

- **B. Live retarget:** write a final-item index into `+0x358[i]` for the
  controlled champion. The native buyer does all purchasing. Smallest change.
- **C. Real shop (newly feasible):** answer the two controller questions
  ourselves for the controlled champion only. These are vtable-indirect calls
  (`call rax`), not CALL sites: the route is redirecting the `E9` thunks
  `0xf60620`/`0xf61b80` (each referenced by exactly one `.rdata` slot,
  `0x3b08c60`/`0x3b08c58`) or that `.rdata` slot, filtered to the controlled
  player so every other controller keeps its native answer. A click queues
  "buy X"; at base the game's own validation, gold deduction and upgrade
  carry-over run unchanged. Allows buying any single component in any order.
  No selling: the game has no sell path.
  Constraints: the native buyer answers every in-base tick and spends all
  affordable gold on the plan the moment the champion arrives, so while the
  shop is on, the controlled champion's native answers must be suppressed
  (only queued purchases go through), or an explicit auto-buy toggle kept.
  Writes (B's target entry, C's queue) are applied on the simulation side
  (per-tick AI callback), never from the UI thread. F12 restores native
  answers.

Open: the exact Riot gate patch (runtime only), whether other controller
types implement `+0x80/+0x88` elsewhere, and abandoned-component behaviour.
A logging-only build (in base: each decision, owned count, gold, target list)
answers all three in one user test.

## 0.59.0 user log (Riot item mod 0.11.12, one match, user on player 2)

Buyer code at runtime:

| Anchor | Live | Meaning |
|---|---|---|
| `new_item_gate` 0x146baf3 | `cmp rax, 5` (was 3) | Riot raises the executor cap from 4 to 6 items |
| `selector_gate` 0xea216a | `jmp` (was `jbe`) | Riot removes the selector's 4-item check |
| `new_item_decision` 0xf3f510 | `mov rax, 0x21d263000; jmp rax` | Riot replaces the buy-new decision (absolute jump to an allocated trampoline; owner not named by this build, which only decodes E8/E9) |
| payments, upgrade decision, goal/step helpers, both thunks, both vtable slots | native | untouched |

76 purchase steps, all ten players:

- One validated step per tick (e.g. player 2, ticks 19836-19841: ironsword ->
  soldiers_longsword -> caulfields_warhammer -> deaths_dance ->
  radiant_deaths_dance, then dagger for the next item).
- Upgrades replace the item in its slot; new items append. `native_owned`
  always equalled the SDK item count: no placeholder entries for empty slots.
- The AI keeps exactly one unfinished item and completes it (five levels, up
  to `radiant_*`; boots two levels) before starting the next final item.
- Each champion owned one starting component before the first sample.
- The match ended at four items, so five/six were not observed; the patched
  gate is direct evidence of the six-item cap.

Design consequences:

- Hook the controller's two answers at the native thunks `0xf60620` /
  `0xf61b80` (or their `.rdata` slots), upstream of Riot's prologue patch. For
  the controlled champion with Manual shopping On, answer from the user's
  queue; for every other case call through, so Riot's decision still runs.
- Never fingerprint `0xf3f510`'s prologue, and never check slot counts in the
  mod: the executor gate (4 vanilla, 6 Riot) remains the authority.
- With Manual shopping On the half-built question is moot for the player
  (any owned item may be upgraded). It still matters for F12 hand-back and
  for an auto-buy retarget option; that needs a later write experiment.
