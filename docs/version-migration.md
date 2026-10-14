# Supporting a new game version: runbook

The order of work that carried the mod from game 0.6.3 to 0.7.0, written
as a checklist. [migration-0.7.md](migration-0.7.md) is the detailed
record of that migration (what moved, with the evidence);
[modding.md](modding.md) explains the code layout.

The mod supports several game versions from one DLL. Everything that
differs between versions sits in one folder per version,
`probe/src/native_adapter/v<version>/` (`layout.rs`, `identity.rs`,
`effects.rs`, `items.rs`), with a reviewed profile in
`tools/native_profiles/<version>.json`. The hook code in
`native_adapter/windows/` is shared and is built once per folder. A new
version is a new folder, a new profile and three lines in
`native_adapter/mod.rs`. Older versions keep working.

Nothing is verified in game by a tool or a test. Each family below is
finished only when the game has been played with it.

## 0. Before Steam updates

- Keep a copy of every supported game version. The build verifier needs
  each one (`research/game-builds/<version>/`, not in git), and so does
  every comparison below. Take the copy before the update replaces it:

  ```
  python tools/patch_migration.py snapshot --label 0.7.1 --output research/game-builds/0.7.1
  ```

- A beta branch is the chance to do this work before the release: the
  live game stays supported from the same DLL.

## 1. What the update is

1. Start the current mod on the new game once. It must log
   `NATIVE install ok=false Executable fingerprint differs`, install no
   hooks, and a match must start and end normally. This is the baseline.
2. Compare `mod-sdk-stable/` of both copies. If only `base_version.txt`
   differs, the DLL's interface is unchanged. If more differs, run
   `python tools/prepare_sdk.py` first and fix what it reports; a changed
   ABI may rule out one DLL for both versions.
3. Compare the bundles (new champions, changed layouts or settings): new
   champions mean new skill effects (step 5, Readers) and possibly new
   tooltip tables.

## 2. Draft the new tables

```
python tools/draft_version.py --old 0.7.0 --new 0.7.1
```

It chains the comparison report, the function map, the tooltip migration
and the vtable matching, and writes to `research/game-builds/draft-0.7.1/`:

- `v071/*.rs`: the old folder with each address and byte pattern carried
  over. `UNKNOWN_0x...` (does not compile) marks what it could not carry;
  `// REVIEW` marks what it carried with less than full confidence.
- `0.7.1.json`: the profile, with `"reviewed": false` and each anchor's
  status under `draft`.
- `report.json`: every address, its status and how it was found.

Measured against the reviewed tables, in both directions:

| Run | Addresses carried | Wrong | Byte patterns carried | Wrong |
| --- | --- | --- | --- | --- |
| 0.6.3 to 0.7.0, with the function map built during the migration | 866 of 909 | 0 | 221 of 253 | 0 |
| 0.7.0 to 0.6.3, from nothing (about one minute) | 888 of 946 | 5 | 215 of 254 | 1 |

Everything wrong was marked REVIEW, nothing marked exact was wrong, and
what was not carried is listed as unknown. The wrong ones show the two
traps that code similarity cannot settle:

- An entry with no counterpart. 0.7.0 lists four forms of the
  single-target projectile and 0.6.3 has one, so three look-alikes were
  proposed. The note names the runner-up when two candidates are close.
- A function the mod calls itself, picked among near-identical variants.
  The outline's two shader parameter setters (`OUTLINE_FLOAT_PARAM`,
  `OUTLINE_COLOR_PARAM`) have siblings that differ only in stack layout,
  and the sibling aligned better than the right one. Confirm these two by
  the function that calls them in the unit renderer, never by similarity.

Then, for what is left:

| Left over | Tool |
| --- | --- |
| A function not found | `tools/compare_functions.py` (align two functions), `tools/find_context.py` (find an instruction by its neighbours), then `tools/map_functions.py --seed old=new` and run the draft again |
| A call site | Align the enclosing functions (`compare_functions.py --at`). Never match call sites by their order among callers: the linker reorders functions |
| An effect `apply` | `tools/match_vtables.py`. A rewritten method scores low; check which vtables hold each candidate and how many |
| A table or pointer slot | The functions it points to, once those are mapped |
| Tooltip anchors | `tools/migrate_tooltips.py` lists what it could not carry under `unresolved` |
| Reading code around an address | `tools/show_code.py` |

Lines marked REVIEW are mostly functions the game recompiled. For each,
check that the instruction at the new address does the same thing and
that the arguments are set up the same way. A note that names a runner-up
or identical copies means the choice has to be made by the caller.

## 3. Field offsets

The hook files use entity, player and view field offsets directly
(`+ 0x...` in `native_adapter/windows/`). They were equal in 0.6.3 and
0.7.0; 0.6.4 moved two viewer fields, which are now per-version
constants ([migration-0.6.4.md](migration-0.6.4.md)). For each hooked
function that is not instruction-for-instruction equal, run:

```
python tools/field_shifts.py --old research/game-builds/0.7.0 --new research/game-builds/0.7.1 OLD=NEW
```

If an offset moved, it has to become a constant in every version's
`layout.rs` first, since the hook code is shared.

## 4. Add the version

1. Copy the reviewed draft to `probe/src/native_adapter/v071/` and the
   profile to `tools/native_profiles/0.7.1.json` (`"reviewed": true` only
   for what was reviewed; `families` lists the reviewed families).
2. In `native_adapter/mod.rs`: a `mod v071 { ... }` block like the others,
   its number in `hooks!`, its fingerprint in `install`.
3. In `v071/identity.rs`, set `ENABLED` to the session family only.
4. `python tools/verify_native_profile.py --profile tools/native_profiles/0.7.1.json --executable research/game-builds/0.7.1/TeamfightManager2.exe`
   must pass. It also refuses an address written into the shared hook code.

## 5. One family at a time

Enable a family in `ENABLED`, build (below), play, then the next. A family
that is not enabled installs nothing, so the game stays playable while
the rest is open. The order that worked, and what to play for each:

| Family | Play |
| --- | --- |
| Session | Hold at start, start, pause, resume, AI hand-back and reclaim, a full match; game speed normal at 30 and 60 fps |
| Camera, Minimap | Layout, lock and follow, edge pan, zoom, vision; minimap size, clicks, drag |
| Movement, Combat | Move, attack-move, stop, recall and cancel, attacks, attack cancel, Q/W/R, cast on release |
| Shop | Open, queue, buy, gold, build, slot count; with an item mod installed |
| Outline | Hover and target outlines on both teams, death greyscale |
| Readers | Skill previews. One match per group of champions, then `python tools/audit_previews.py`: it lists every skill the reader did not understand, so not every champion needs playing |
| Tooltips | Ability numbers on a few champions, including a new one and a mod one |

For skill shapes, a field's meaning is settled by what the game shows, not
by its position: ask for a screenshot of the skill's real area before
deciding which word is a width and which a length.

## 6. Build and release

The usual pipeline (see CHANGELOG.md for the form of an entry):

```
cd probe
cargo fmt
cargo clippy --offline --all-targets -- -D warnings
cargo test --offline --release
cargo build --release --offline
cd ..
python tools/stage_package.py
python tools/verify_build.py --version X --probe-tests N
python tools/package_mod.py --version X
.\tools\install_verified_build.ps1 -Version X -PreviousVersion Y
```

- `verify_build.py` checks every profile against its own executable: the
  installed game for its version, the kept copies for the others.
- The hook tests run once per supported version, so the test count grows
  with each version. Test code in `native_adapter/windows/` must not
  define global symbols.
- Before release, play one match on every supported version with the
  same DLL; the log's install line names the version it recognised.
- `mod.mod_info` keeps the oldest supported game in its `base`
  dependency.
- After uploading with TFM2ModUploader: it resets the English Workshop
  description to the `description` field of `mod.mod_info` (paste the
  real one back) and rewrites `last_updated` in the installed
  `mod.mod_info` (restore that file from the release zip, or the
  installer reports a fingerprint mismatch).

## Dropping an old version

Delete its `v<version>/` folder, its profile, its block, its number in
`hooks!` and its fingerprint in `install`, and raise the `base`
dependency in `mod.mod_info`.
