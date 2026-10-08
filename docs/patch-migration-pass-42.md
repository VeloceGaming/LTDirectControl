# 0.6.2 to 0.6.3 migration

The user authorized the migration after returning the installed game to 0.6.3.
Both complete game inputs are privately preserved and independently hashed.
No game binary, asset bundle, Steam manifest or full disassembly belongs in the
public repository or release.

## What changed

This update moved native code and changed private layouts. Replacing addresses
and the executable hash alone would have left incorrect readers.

| Reader | 0.6.2 | 0.6.3 | Evidence |
| --- | --- | --- | --- |
| Native champion kind | 13 | 15 | Movement consumer and stable SDK getter |
| Position X/Y | +660/+668 | +658/+660 | Movement/steering callers and effect consumers |
| Actor ID / level | +5c0/+5c8 | +5b8/+5c0 | Attack/skill consumers and stable level getter |
| Q/W/R action objects | +580/+590/+5a0 | +578/+588/+598 | Native action method calls |
| Q/W/R effect metadata | +4c8/+500/+538 | +4c0/+4f8/+530 | Skill/effect consumers |
| Pending-effect Vec | +2a8/+2b0/+2b8 | +2a0/+2a8/+2b0 | Attack and three skill queue writes |
| Active-effect pointer/count | +2c8/+2d0 | +2c0/+2c8 | Native movement effect gate |
| Player build Vec | +550/+558/+560 | +350/+358/+360 | Live purchase selector |
| Native player getter slot | +140 | +158 | Stable SDK gold getter |
| AI settings borrow | Context +8 | Context → world → +1b8 | AI bridge settings getter |

Offsets are hexadecimal. The early action tag/goals, basic-attack elapsed time,
attack/skill budgets, queue stride/target payload, item getter slots and preview
effect payload dimensions remain unchanged. Field shifts were reviewed by
consumer, not applied globally across arbitrary native objects.

## Validation and maintenance

The initial conservative matcher resolved only 13 unique candidates. Remaining
locations were traced using source references, callers, dispatch branches,
consumers, SDK bridge tables and side-by-side instruction review. The foreground
worker publication boundary and actual viewer branch remain essential; a nearby
similar routine is not interchangeable.

`tools/native_profiles/0.6.3.json` records 55 explicit locations, call targets,
the exact executable identity and 13 additional layout checks. The runtime
checks the disk hash, loaded PE identity, layout instructions and relocated
table pointers before installing hooks. Unknown executables still fail closed.
The profile verifier cross-checks the Rust constants to prevent stale-source
packaging. Candidate matching never approves or edits runtime locations.

All 316 UI/style entries were compared between the bundles. Seventeen changed;
the in-match templates used by this mod did not. Stable SDK code is identical
between the snapshots; the base-version label changed. No SDK ABI rewrite was
needed.

Local reports are in `research/game-builds/`, including paired review,
source maps and the UI-template comparison. Source before this pass is preserved
in `research/backups/0.41.0-before-0.42.0/`. Release receipts record artifact
hashes, checks and installation state. Native gameplay remains pending user test.

0.42.0 is packaged and installed locally. Validation passed: 199 probe tests,
18 core tests, 19 conservative-migration tests, seven profile-refusal tests,
Clippy, formatting, release build, ABI/null-host loading and all 53 package and
installed-file hashes. The mod-file rollback directory is
`dist/backups/0.41.0-before-0.42.0-20261007-153407/`.
