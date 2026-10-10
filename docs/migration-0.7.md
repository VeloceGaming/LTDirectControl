# Migrating to game 0.7.0

Working notes for the `game-0.7` branch. `main` stays on 0.6.3. The method
is in [patch-migration.md](patch-migration.md) and
[re-mod-migration-guide.md](re-mod-migration-guide.md); this file records
what has been established for 0.7.0 and what has not.

Nothing here is verified in game unless a line says so.

## Builds

| | 0.6.3 | 0.7.0 (beta branch) |
| --- | --- | --- |
| Executable SHA256 | `f21cf691…2dbe2f` | `6c9cd659e727b4130766241e9d43b222454a8455609b40f9bf73c01d5136a94f` |
| PE timestamp / image size | `0x6ac5c567` / `0x52d6000` | `0x6ac5fab4` / `0x56d8000` |
| Steam build | 25769776 | 25773452 |
| SDK | ABI 9 | byte-identical apart from `base_version.txt` |

Both are preserved and hash-verified under `research/game-builds/`
(private, not in git).

Baseline, in game (2026-10-10): mod 1.0.5 on 0.7.0 loads, logs
`NATIVE install ok=false Executable fingerprint differs`, installs no
hooks, and a match starts and ends without a crash.

## Triage

- Of 552 profile anchors, 6 are unchanged in place. `patch_migration.py
  compare` proposes one candidate for 241, several for 103, none for 133,
  and leaves 75 data records for manual review
  (`research/game-builds/migration-0.6.3-to-0.7.0.json`).
- Bundle: nothing removed, 179 entries added, 252 changed. The `ingame`
  layout differs by 11 lines (a new `palette` property). New champions:
  bombardier, ferryman, gladiator, hypnotist. A theme system was added
  (`style/main_d`, `style/theme/d`, `palette`).
- Besides the profile anchors, the hooks hard-code about 100 distinct field
  offsets (view.rs 39, combat.rs 32, movement.rs 9, shop.rs 7). Each needs
  a 0.7.0 consumer before its family is enabled.

## Method notes

- **Do not match call sites by their order among callers.** The linker
  reordered functions: the viewer's caller that is 24th of 27 in both
  builds is a different function in 0.7.0 and passes a null `ingame` node.
  Align the enclosing functions instead (`tools/compare_functions.py`,
  `--at <old rva>`), or compare how each caller sets up its arguments.
- `tools/map_functions.py` follows aligned calls outward from known
  function pairs (1,436 pairs so far, in
  `research/game-builds/function-map-0.6.3-to-0.7.0.json`);
  `tools/find_context.py` looks for an instruction by its neighbours with
  shrinking windows.
- `tools/compare_functions.py` compares one function across the two
  builds with relocated addresses masked; field displacements and
  constants are compared. `tools/show_code.py` prints the instructions
  around an address.

## Families

Hooks install per family (`native_profile::family`, `ENABLED`). Status for
0.7.0:

| Family | Status |
| --- | --- |
| Session | Enabled in 1.1.0; passed in game 2026-10-10 (hold at start, start, pause, resume, AI hand-back and reclaim, a full match, keys normal) |
| Camera | Enabled in 1.1.1; passed in game 2026-10-10 (layout, lock and follow, pan, zoom, vision, hand-back) |
| Minimap | Enabled in 1.1.1 (all 54 operands); passed in game 2026-10-10 |
| Movement | Enabled in 1.1.2; passed in game 2026-10-10 |
| Combat | Enabled in 1.1.2; passed in game 2026-10-10 (attacks, Q/W, stop, hand-back; played the new champion Ferryman) |
| Shop (with the item catalogue) | Enabled in 1.1.3; passed in game 2026-10-10 (1.1.5: items that are not enabled are hidden; registry 237, enabled 30) |
| Outline | Enabled in 1.1.6; passed in game 2026-10-10 |
| Readers (skill previews) | Known effects passed in game (1.1.8); new 0.7.0 effect `0x160da70` (Hypnotist) open |
| Tooltips (native ability text) | Open |

### Session (worker, viewer, input)

| Item | 0.6.3 | 0.7.0 | Evidence |
| --- | --- | --- | --- |
| Worker send (`WORKER_ORIGINAL`) | `0xd1c2a0` | `0x8bf9d0` | 420/420 instructions equal |
| Worker call site (`WORKER_SITE`) | `0xbfc77a` | `0x979089` | Caller `0xbfc320`→`0x978c20` aligns (372/420; the differences are stack-slot shifts and a register swap); same three arguments; the result is still compared with -1 at `[rbp+0x1f8]` after the call |
| Viewer update (`VIEW_ORIGINAL`) | `0x930640` | `0xb194c0` | 1045/1045 instructions equal, so its uses of view `+0x70`, `+0x290`, `+0x358` and config `+0x10`, `+0x14`, `+0x48` are unchanged |
| Viewer call site (`VIEW_SITE`) | `0xcdcce0` | `0x877560` | Caller `0xcd6d80`→`0x871540` aligns (10180/14941); identical argument setup: config `+0x18` in r9, tps from `[rcx+0x12f8]`, the `ingame` node lookup result in the eighth argument |
| Pause guard (`VIEW_LAYOUT_PAUSE`) | `0x93078b` | `0xb1960b` | Same instruction in the equal function |
| Input call site (`INPUT_SITE`) | `0xd03c7f` | `0x89fd1d` | Caller `0xd03920`→`0x89f9b0`: all 222 old instructions present, two `lea` arguments inserted elsewhere |
| Input handler (`INPUT_ORIGINAL`) | `0x7b37f0` | `0xa3ac10` | Recompiled (821/1517 equal). Same argument registers; the key press tag `0x8000000000000006` is still tested. The key codes the hook lets through (`0x21`, `0x3d`, `0x3e`) are not re-proven: a wrong code would leak or swallow a key, not corrupt memory |

Not part of the session family and still open inside the viewer hook: the
`ingame` UI downcast (node `+0x230/+0x238`, type id, `+0x9150`) and all
camera, vision and layout fields. Those run only with the camera family.

### Camera

Enclosing functions were located and aligned with
`tools/compare_functions.py`; each guard below is the same instruction in
both builds (same field displacement).

| Function | 0.6.3 | 0.7.0 | Found by |
| --- | --- | --- | --- |
| Client update (viewer caller) | `0xcd6d80` | `0x871540` | Argument setup at the viewer call |
| Client UI update | `0xb9c970` | `0x9148a0` | Call from the viewer update (`tools/map_functions.py`) |
| Layout config link | `0xa43be0` | `0xb9bcb0` | 253/253 instructions equal |
| Input handler | `0x7b37f0` | `0xa3ac10` | Call from its aligned caller |
| Renderer | `0x231f310` | `0x243d020` | Instruction context (`tools/find_context.py`), 3410/5554 aligned |
| In-game UI update | `0x280bba0` | `0x24a3390` | Instruction context, 4725/7998 aligned |

| Guard | 0.6.3 | 0.7.0 |
| --- | --- | --- |
| `VIEW_LAYOUT_INGAME_ARGUMENT` | `0xcdcbf6` | `0x877476` |
| `VIEW_LAYOUT_INGAME_NODE_ARGUMENT` | `0xcdcc79` | `0x8774f9` |
| `VIEW_LAYOUT_UI_DOWNCAST` | `0xb9cfc5` | `0x914f03` |
| `VIEW_LAYOUT_UI_CONFIG_LINK` | `0xa43e02` | `0xb9bed2` |
| `VIEW_LAYOUT_NATIVE_TOGGLE` | `0x7b4de5` | `0xa3c219` |
| `VIEW_LAYOUT_UI_WIDE_BINDING` | `0x280bd60` | `0x24a3550` |
| `VIEW_LAYOUT_CONTROLS_HIDE` | `0x280f42f` | `0x24a7384` |
| `VIEW_LAYOUT_ANNOUNCE_BINDINGS` | `0x280d99d` | `0x24a551f` |
| `VIEW_LAYOUT_KILLS_BINDINGS` | `0x280d9de` | `0x24a5560` |
| `VIEW_LAYOUT_ANNOUNCE_WIDE` | `0x1e71e40` | `0x2b594c0` |
| `VIEW_LAYOUT_KILLS_WIDE` | `0x1e71f60` | `0x2b595e0` |
| `VIEW_LAYOUT_FULL_WIDTH` | `0x23228d4` | `0x24408ba` |
| `VIEW_LAYOUT_COMPACT_SIDE` | `0x2322906` | `0x24408ec` |
| `VISION_RENDER_FIELD` | `0x231f8ae` | `0x243d5f8` |
| `VISION_CACHE_COMPARE` | `0x2320bcd` | `0x243e98d` |
| `VISION_CACHE_WRITE` | `0x2320f0a` | `0x243ed0d` |
| `VISION_UI_FIELD` | `0x280bfe2` | `0x24a37d0` |

Guards that contain a stack slot or a relative jump (for example
`UI_DOWNCAST`, whose frame slot moved from `rbp+0x10e18` to `rbp+0x10e20`)
need their 0.7.0 bytes read from the new address; the field offsets in
them are unchanged.

Fields:

- The `ingame` UI object check is unchanged: node `+0x230/+0x238`, the
  `+0x50` and `+0x18` calls, the type id pair
  (`0x810a3f1ff5177337`, `0xaf7a6919a6336e28`, 124 uses in both builds),
  `+0x9234` (UI full width) and `+0x9150` (config link).
- Config `+0x45` (full width), `+0x46` (compact side) and `+0x4b` (vision)
  are read by the aligned renderer and UI instructions above.
- View `+0x110`..`+0x120` and `+0x130` are used identically by the equal
  viewer update; `+0xf8`..`+0x12c` by aligned renderer instructions;
  `+0x458` (pan velocity) by the input handler with the same constants.
- Config `+0x18` (camera mode), `+0x1c` and `+0x20` (lane and team when
  following, x and y when free): the follow routine (`0x2439fd0`) reads
  `[rdi+0x20]` and `[rdi+0x1c]` and copies `[rdi+0x1c]` to view `+0x114`
  in free mode; the own-mid shortcut in the input handler (`0xa3bd8e`)
  writes mode 2 and the pair through the config holder (`+0x30`, `+0x38`,
  the config being the holder `+0x18`). The addresses in the older notes
  (`1fcfd5d`, `cb020e`) were 0.6.2 addresses.

### Minimap

The camera's click handling assumes the mod's enlarged minimap
(`minimap::content_rect`), so camera and minimap are enabled together.
All 54 patched operands (37 in the renderer, 17 in the input handler) are
relocated in the mapped functions: 17 with identical bytes, 37 whose
RIP-relative operand moved. The native constants they replace are
unchanged. They are listed in `tools/native_profiles/0.7.0.json`.

### Movement and combat

The entity layout is unchanged. `tools/field_shifts.py` aligns function
pairs with field displacements masked and then compares the displacements:
in the nine functions below every entity field the hooks use keeps its
offset (`+0x68`, `+0x70`..`+0x80`, `+0xb0`..`+0xc8`, `+0x2a0`..`+0x2c8`,
`+0x3f4`, `+0x3f8`, `+0x430`, `+0x464`, `+0x488`..`+0x4b8`, `+0x4c0`,
`+0x4f8`, `+0x530`, `+0x578`..`+0x598`, `+0x5b8`, `+0x5c0`, `+0x658`,
`+0x660`).

| Function | 0.6.3 | 0.7.0 | Alignment |
| --- | --- | --- | --- |
| Input dispatcher (move, attack, skill sites) | `0x11d8fc0` | `0x1463780` | 757/757 |
| Move | `0x1703fa0` | `0x1562680` | 49/49 |
| Attack | `0x16f3e30` | `0x1551cc0` | 236/236 |
| Skill Q / W / R | `0x17046b0` / `0x16f4450` / `0x1703390` | `0x1562fe0` / `0x15522e0` / `0x1561a30` | recompiled, about 70% |
| Steer | `0x1702bd0` | `0x1561270` | 462/462 |
| Direct step | `0x16f5100` | `0x1553290` | equal |
| Stop event / recall cancel | `0x13cccc0` / `0x13cda10` | `0x1695430` / `0x16957b0` | equal |
| Entity update (auto attack, aim, steer sites) | `0x16f6cc0` | `0x1554ec0` | 3857/5736 with fields masked |
| Aim correction | `0x1707b00` | `0x1568ff0` | 547/547 |
| Entity getter checked by the range reader | `0x2e16650` | `0x315ba70` | equal |

Call sites: move `0x1463914`, attack `0x14639c0`, skills `0x14639f1`,
`0x1463940`, `0x1463aa3`, auto attack `0x1555f9f`, aim `0x15590e9`, steer
`0x155b676`; each has the same argument setup as in 0.6.3.

Changed in 0.7.0:

- The skill functions take 16 more bytes of stack and, before the range
  check, call a new method on the skill's effect object (vtable `+0x120`)
  whose result is added to the range. The mod's own skill range
  (`read_effect_metadata`) does not include that term: ranges used for
  casting decisions and previews may be short for effects that return a
  bonus. Not a memory-safety matter.
- The action table slots the hooks call (`+0x90` cooldown, `+0xa8` use
  count, `+0x48`/`+0x50`/`+0x58`) are called the same way by the skill
  functions. A different vtable used by the entity update gained two
  entries below `+0x2a0` (now `+0x2b0`); the mod does not call through it,
  but the shop family's `+0x158` slot must be re-proven.

### Shop and item catalogue

`tools/relocate.py` maps an old address through the aligned functions.

| Item | 0.6.3 | 0.7.0 | Evidence |
| --- | --- | --- | --- |
| Gold getter | `0x2e17b30` | `0x315cf50` | 22/22 equal: slot `+0x158`, gold `+0x868` |
| Build vector reads | `0xea1ecb`, `0xea1ed2` | `0xe947ab`, `0xe947b2` | Next-step function 232/232 equal |
| Goal search / next step | `0xea1b90` / `0xea1ea0` | `0xe94470` / `0xe94780` | Equal |
| Selector gate | `0xea216a` | `0xe94a4a` | Same function |
| Purchase executor | `0x1465e80` | `0x1745420` | Recompiled (2662/4935); slot gate `0x174ab91`, payments `0x174a6a8` and `0x174ac4d` |
| Upgrade / new-item decisions | `0xf3f3a0` / `0xf3f510` | `0xfd19c0` / `0xfd1b30` | 100/101 and 95/96 |
| Their thunks | `0xf60620` / `0xf61b80` | `0xff9040` / `0xffa590` | The only jumps to the decisions outside a function |
| Controller slots | `0x3b08c60` / `0x3b08c58` | `0x3e5ddd0` / `0x3e5ddc8` | The only pointers to the thunks |
| AI bridge table | `0x3d10330` | `0x407dbf8` | Size word 120; entries +8 and +48 are the two functions below |
| Settings getter / AI settings entry | `0x2e83dd0` / `0x2e84800` | `0x31a13d0` / `0x31a1e00` | Byte-identical, one match each |
| Item wrapper constructor | `0x2e66f50` | `0x3184010` | 137/137 equal |
| Wrapper next-tier getter | `0x1322f80` | `0x12ce710` | Byte-identical, one match |
| Game item tooltip | `0x27e9090` | `0x23646e0` | 1601/1936; all 166 field uses unchanged |

The executor loads the controller slots `+0x80` and `+0x88` and passes the
same registers and stack arguments as before, so the hooks' calling
convention is unchanged. The game's goal search calls the item methods
`+0x70`, `+0x80`, `+0x50`, `+0x68` in the same order in both builds.

### Outline

| Item | 0.6.3 | 0.7.0 | Evidence |
| --- | --- | --- | --- |
| Unit body renderer | `0x230ec30` | `0x2399810` | Target of all 15 relocated call sites; recompiled (16 more bytes of stack) |
| Its 15 call sites | see `OUTLINE_SITES` | see `OUTLINE_SITES` | Each found by aligning its calling function; 7 of those functions are equal; 15 callers in both builds |
| Render command drop glue | `0x1c5190` | `0x1a3e10` | 179/179 equal: same variants and fields |
| Its jump table | `0x3a199ac` | `0x3d69354` | Referenced by the equal drop glue |
| Drop glue caller in `shader()` | `0x1ca533` | `0x1a9356` | Calls the new drop glue |
| Shader setter | `0x1c91f0` | `0x1a8130` | Recompiled (about half aligned); same four arguments at the composite call |
| Composite shader call | `0x2322b12` | `0x2440b01` | In the aligned renderer |
| f32 / colour parameter setters | `0x21725a0` / `0x2171ae0` | `0x1cfc5d0` / `0x1cfbaf0` | 13 and 24 aligned callers agree; same arguments |
| DrawLine tag write | `0x2311692` | `0x239c1e6` | The one `0x8000000000000008` in the unit renderer |
| NinePatch tag write | `0x1c7a8a` | `0x1a6a0d` | Aligned |

The sprite command fields the code reads and writes (source rectangle
`+0x68`, draw position `+0x78`) are set by two functions that are equal in
0.7.0 with every field use unchanged (`0x1c76e0` -> `0x1a6670`, 84/84;
`0x1c9050` -> `0x1a7f90`, 86/86). The command stride (`0xd0`) and the unit
view's centre (`+0x17c`) are used the same way in the unit renderer. The
unit id at view `+0x100` has no located 0.7.0 consumer yet: a wrong id
would make clicks land on the wrong unit, so that is a stop condition of
the in-game test. Inside the shader setter a few byte
fields beyond `+0x90` moved by one (consistent with the new `palette`
flag); the mod does not touch them.

### Readers (skill previews)

The preview reader walks a skill's effect tree and recognises each node by
its vtable: `[drop, size, align, method, apply, ...]`, the `apply` address
at `+0x20`. `tools/match_vtables.py` lists the new build's vtables of the
same size and ranks their `+0x20` method by alignment with the old one.

- The vtable shape is unchanged (the same header and slot for every
  matched type), and the entity still holds each skill as a fat pointer
  at `+0x4c0`, `+0x4f8`, `+0x530`.
- 36 of the 40 applies in `native_preview.rs` score 0.97 to 1.00 with the
  same size.
- Rewritten in 0.7.0 with the same size (about a third of the
  instructions align): `RANGE_PERIOD` `0x14294c0` -> `0x1bde9d0` (152
  bytes), `RANGE_PROJECTILE` `0x1489140` -> `0x1aa08c0` (120),
  `ICE_MAGE_ULT` `0x160fd80` -> `0x1709a30` (88). The payload layout is
  assumed unchanged; Ice Mage R and Sand Mage Q and R passed in game
  (1.1.8).
- `TargetProjectile` `0x14e0490` -> `0x17d70f0` is 80 bytes (was 72): one
  field was inserted at `+0x40`. A second 72-byte vtable method
  (`0x1804b60`) is similar and unidentified.
- `Attack` is two addresses in 0.7.0 (`0x161f660` in nine vtables,
  `0x163cdc0` in one), with the same companion method.
- Section bounds used to validate a vtable pointer: `.text` ends at
  `0x3d1c000`, `.rdata` at `0x53d046a`.

Open: the range term the skill functions add through effect method
`+0x120`; the four new champions' hand-written effects, if any.

### New champions (1.1.12)

Gladiator, Ferryman, Bombardier and Hypnotist build every skill from one
effect: apply `0x160da70`, 160 bytes (20 words), about 3,900 instructions
with one hand-written branch per skill, chosen by the byte at `+0x98`:
0-2 Gladiator, 4/6/7 Ferryman, 8-10 Bombardier, 12/14/15 Hypnotist (Q, W,
R). Branch tables: tags 4-7 at `0x3ef83f4`, 12-15 at `0x3ef8404`; 0-3
enter at `0x160ddfb`, 8-11 at `0x160e02e`.

Values seen in game logs (`PREVIEW TREE`):

| Tag | Skill | Words |
|---|---|---|
| 0 | Gladiator Q | `afc8 0 3c 46 0 0 f618 0 0 0 0 0 0 3c 0 0 46 0 0` |
| 1 | Gladiator W | `9c40 0 46 5a 0 0 2710 0 f0 0 19 3` |
| 2 | Gladiator R | `ea60 1964 50 50 0 0 0 0 0 0 28 0 0 f0` |
| 4 | Ferryman Q | `15f90 1964 8c 64 0 0 36b0 0 78 78 1e 0 1388 0 0 0 0 32 3c` |
| 6 | Ferryman W | `11170 0 0 0 0 0 3e80 0 12c 0 32 0 0 0 0 0 0 0 3c` |
| 7 | Ferryman R | `11170 0 190 64 0 0 0 0 5a 78 0 0 ea60 0 0 0 0 28 1e` |
| 8 | Bombardier Q | `0 0 0 0 0 0 0 0 5a 0 14 0 c350 0 0 0 0 0 32` |
| 9 | Bombardier W | `186a0 0 64 64 55f0 0 0 24 0 5a 0 0 0 0 0 0 0 0 2d` |
| 10 | Bombardier R | `1d4c0 0 5a 50 88b8 3e80 0 24 0 0 0 0 d6d8 0 6 12` |
| 12 | Hypnotist Q | `13880 1770 82 6e 0 0 61a8 0 0 5a 0 0 0 0 0 0 0 0 1e` |
| 14 | Hypnotist W | `13880 0 0 0 4e20 0 0 28 78 0 4b` |
| 15 | Hypnotist R | `15f90 0 0 0 9c40 0 0 2d 96 0 5a` |

Read in the code: Ferryman W (`0x160e40e`, then `0x160eff3`) takes the
direction to the input, clamps the distance to word 0 and builds an end
point; words 6 and 8 go to the effect it queues. Everything else in the
reader is inferred from the values and the player's descriptions: word 0
range, 1 shot hit radius, 4 area radius, 6 strip half width, 12
Bombardier Q's range bonus. Open: Gladiator Q's radii and angle; whether
word 6 is a half or full width.

## Open questions

- One build for both game versions, or a switch-over when 0.7 is released.
- Whether the Workshop can serve different files per game branch.
- The theme system's effect on the mod's windows.
