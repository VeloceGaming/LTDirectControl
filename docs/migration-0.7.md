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
| Camera | Enabled in 1.1.1; in-game test pending |
| Minimap | Enabled in 1.1.1 (all 54 operands); in-game test pending |
| Movement, Combat, Outline, Shop, Readers | Open |

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

## Open questions

- One build for both game versions, or a switch-over when 0.7 is released.
- Whether the Workshop can serve different files per game branch.
- The theme system's effect on the mod's windows.
