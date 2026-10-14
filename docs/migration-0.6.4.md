# Migrating to game 0.6.4

What was established for 0.6.4, the live-branch update of 2026-10-14. The
method is the runbook in [version-migration.md](version-migration.md); the
0.7.0 record is [migration-0.7.md](migration-0.7.md).

Everything below is **read from the code** (static analysis of the
executable) unless a line says otherwise. **Seen in game** (2026-10-14, mod 1.2.3 development build, DLL
`5ae780a2...`, game 0.6.4): the author ran the whole test list (start,
session, camera and minimap, movement and combat, shop, outline, skill
previews, tooltips) and played a full match; everything worked.

## Builds

| | 0.6.3 | 0.7.0 (beta branch) | 0.6.4 |
| --- | --- | --- | --- |
| Executable SHA256 | `f21cf691…2dbe2f` | `6c9cd659…136a94f` | `a1d05045405a12e2bed182daeafcad8c8a40cf29ab68ba6ea5bfd30257439582` |
| PE timestamp / image size | `0x6ac5c567` / `0x52d6000` | `0x6ac5fab4` / `0x56d8000` | `0x6acf3112` / `0x533f000` |
| SDK | ABI 9 | ABI 9 | ABI 10 (only appended slots, fields and types) |

Preserved and hash-verified under `research/game-builds/0.6.4/` (private,
not in git). Players can roll back to 0.6.3 on Steam, so 0.6.3 stays
supported.

## What 0.6.4 is

0.6.4 is built from the 0.7.0 code but keeps the 0.6.3 roster: the effect
shared by Gladiator, Ferryman, Bombardier and Hypnotist is absent, while
the input handler aligns 1465 of 1509 instructions with 0.7.0 (and 833 of
1517 with 0.6.3). Most functions that changed since 0.6.3 therefore match
their 0.7.0 forms, and the 0.7.0 tables were the better reference for
them. The folder (`native_adapter/v064/`) keeps the 0.6.3 shape.

`tools/draft_version.py --old 0.6.3 --new 0.6.4` carried 781 of 812
addresses in `layout.rs` (751 exact); 53 distinct addresses across the
four tables were left unknown. The function map was
small (486 pairs); composing it through 0.7.0 raised it to 907 but
resolved few unknowns. The rest were found by hand: callers of a known
function, `tools/compare_functions.py` alignment from 0.7.0, unique byte
patterns for leaf functions, and pointer scans for tables.

Every code address in the four tables was then checked by aligning its
function with the 0.6.3 function at the same table position (or the
0.7.0 one): each old instruction lands exactly on the new address, apart
from the entries listed below, which were reviewed by hand. Every byte
pattern equals the executable.

## Changes the hooks depend on

- **Viewer layout.** The fog-of-war cache at view `+0x2d8` (0.7.0:
  compared and rewritten by the renderer each frame) is gone; the
  renderer now passes config `+0x4b` to the fog pass every frame
  (`0x2abf529`). Every view field from `+0x2d8` up moved down by `0x68`:
  the frame-time accumulator `+0x358` -> `+0x2f0` (viewer update, 1033
  of 1045 instructions, the 12 differences are these fields) and the pan
  velocity `+0x458` -> `+0x3f0` (input handler and renderer). These were
  the first shared offsets that differ between versions: they are now
  `VIEW_FRAME_CLOCK` and `VIEW_PAN_VELOCITY` in each version's
  `layout.rs`. Fields below `+0x2d8` (`+0x70`, `+0x110`..`+0x130`,
  `+0x290`) are used the same way.
- **Vision guards.** The two 0.6.3 cache guards (compare, write) have no
  0.6.4 form; the guards are the render field read (`0x2abfaa9`) and the
  fog pass argument (`0x2abf529`).
- **Action table.** A method was inserted above `+0xa8`: what the attack
  and skill functions called at `+0xb8` is now `+0xc0`. The slots the mod
  calls (`+0x48`, `+0x50`, `+0x58`, `+0x90`, `+0xa8`) align with the same
  calls in all three skill functions.
- **Attack function** (`0x1a55320`, 161 of 236 with 0.6.3): reordered,
  with a new call through the action table before the attack starts (the
  patch notes' match-engine changes). Its entity fields (`+0x70`..`+0x98`,
  `+0x5b8`) are unchanged; the guards were re-read (`r15` in place of
  `r14`).
- **Entity, player, config and UI fields** used by the hooks: unchanged
  in all 270 function pairs the tables touch (`tools/field_shifts.py`).

## Entries reviewed by hand

| Item | 0.6.3 | 0.6.4 | Evidence |
| --- | --- | --- | --- |
| Viewer call site | `0xcdcce0` | `0x85f1a0` | Caller `0xcd6d80` -> `0x8593d0` aligns (10368 of 14941); identical argument setup |
| `ingame` argument guards | `0xcdcbf6`, `0xcdcc79` | `0x85f0b6`, `0x85f139` | Same function; the `lea` loads `ingamemainmain` |
| UI downcast guard | `0xb9cfc5` | `0xbf4c33` | Aligned from 0.7.0; frame slot `+0x10e20` (as in 0.7.0), same fields |
| Layout leaves (announce, kills wide) | `0x1e71e40`, `0x1e71f60` | `0x278f260`, `0x278f380` | Unique byte patterns |
| Full width / compact side | `0x23228d4`, `0x2322906` | `0x2ac1a65`, `0x2ac1a97` | Same code, register `rdi` for `rax` |
| Unit renderer | `0x230ec30` | `0x2aaf630` | Target of every call site below |
| Its call sites | 15 | 16 | Each caller aligned; `0x1d10d60` and `0x2823890` are identical copies of one 0.6.3 caller, so both are hooked |
| DrawLine tag write | `0x2311692` | `0x2ab205e` | The one `0x8000000000000008` in the unit renderer |
| Shader call / setter | `0x2322b12` / `0x1c91f0` | `0x2ac1cb7` / `0x1c9130` | Aligned from 0.7.0; same four arguments |
| f32 / colour parameter setters | `0x21725a0` / `0x2171ae0` | `0x2a70650` / `0x2a6fd20` | By their calls in the unit renderer (aligned from 0.7.0); same arguments, five in each. The 0.7.0-based draft guessed other functions: similarity is not enough here |
| Render command drop glue / table | `0x1c5190` / `0x3a199ac` | `0x1c50d0` / `0x3a6b9ac` | Called from the shader setter; the table is the one it loads |
| Aim point writes | `0x17083fd`, `0x1708414` | `0x1a6a487`, `0x1a6a492` | The out target now stays in `rbp`; same `+8`/`+0x10` writes |
| Skill level guard | `0x17047c3` | `0x1a64dfc` | Same fields, registers `rsi`/`rdi` |
| Shop next step | `0xea1ea0` | `0xe933d0` | Rewritten (232 -> 569 instructions). Found as the call after the goal search in the upgrade and new-item decisions (aligned from 0.7.0, 0.99); same build vector reads (`+0x358`, `+0x360`) |
| Selector gate | `0xea216a` | `0xe9385e` | The count is now a call, then the same `cmp 3`. Only logged; the slot count is read from the executor's gate, whose encoding is unchanged |
| Decisions / thunks / slots | `0xf3f3a0`.. | `0xf75440`, `0xf755b0` / `0xfa32a0`, `0xfa47f0` / `0x3b5c730`, `0x3b5c728` | The only jumps to each decision, the only pointers to each thunk |
| Item settings getters, next-tier getter | `0x2e83dd0`, `0x2e84800`, `0x1322f80` | `0x2ed4700`, `0x2ed53d0`, `0x12afca0` | Byte-identical, one match each |
| AI bridge table | `0x3d10330` | `0x3d65528` | The only pointers to the two getters, at `+8` and `+0x30` |
| Effect pointer guards | `0x3b438a0`, `0x3b430f8` | `0x3bb9578`, `0x3ba7090` | The first of identical vtable copies, as in 0.6.3 |
| Tooltip sheet getter / path | `0x338420` / `0x3ac3298` | `0x3383a0` / `0x3b14638` | Getter aligned from 0.7.0; the path is the string the lookup consumer loads |

## Skill previews (effects)

`tools/match_vtables.py`, from 0.7.0 where 0.6.3 scored low:

- `RANGE_PERIOD` `0x1a073a0` and `RANGE_PROJECTILE` `0x1a9ca00` match
  their 0.7.0 forms (0.99).
- `SWORDMAN_ULT` `0x1705730`: 0.43, the only 88-byte candidate held by
  three vtables like the old one; the code reads the same distance word.
  Check Swordman R's preview in game.
- AddBuff and Attack exist twice each (identical code); both are listed.
- Fear and the timed statuses: three 8-byte statuses (`0x17bada0`,
  `0x17ba960`, `0x17b9f20`) at about 0.75 each, recompiled with the
  crowd-control fixes. Which is which is not settled; all three are
  listed as having no area.
- Vampire Q `0x14ec970`: 0.54, far ahead of the next (0.25).

## Minimap

The renderer and input handler are 0.7.0 code, so the 54 operands were
relocated from the reviewed 0.7.0 profile: 48 by alignment, 6 marker
operands by their unique instruction and constant. Each new instruction
carries the same original constant.

## Profile

`tools/native_profiles/0.6.4.json` was generated from the reviewed 0.6.3
profile, each anchor placed through the paired tables and its bytes read
from the executable. As in 0.7.0, six profile-only anchors (skill budgets,
wall tests, shape predicate) are not carried. `tools/verify_native_profile.py`
passes for 0.6.3, 0.7.0 and 0.6.4.

## Families

All nine families are enabled in one build, against the runbook's one at
a time: every address above is checked, and the test sheet is ordered by
family. If the first start fails, the fallback is a build with
`ENABLED = family::SESSION` in `v064/identity.rs`.

| Family | Status |
| --- | --- |
| All | Enabled in 1.2.3 (development build); passed in game 2026-10-14 (the author's full test list and a full match) |
