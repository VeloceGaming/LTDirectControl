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
| Camera | Two guards relocated (below); the rest open |
| Movement, Combat, Outline, Minimap, Shop, Readers | Open |

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

### Camera (partial)

| Guard | 0.6.3 | 0.7.0 |
| --- | --- | --- |
| `VIEW_LAYOUT_INGAME_ARGUMENT` | `0xcdcbf6` | `0x877476` |
| `VIEW_LAYOUT_INGAME_NODE_ARGUMENT` | `0xcdcc79` | `0x8774f9` |

## Open questions

- One build for both game versions, or a switch-over when 0.7 is released.
- Whether the Workshop can serve different files per game branch.
- The theme system's effect on the mod's windows.
