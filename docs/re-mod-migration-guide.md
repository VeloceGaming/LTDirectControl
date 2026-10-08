# Migrating a reverse-engineered mod after a game update

Rough practical guide for another AI or developer, based on LTDirectControl's
Teamfight Manager 2 **0.6.2 → 0.6.3** migration on 2026-10-07.

Treat migration as rebuilding the evidence for every native dependency: where
it lives, what it does, how it is called, and which data layout it expects.
Instruction fingerprints help find candidates. Callers and consumers establish
whether those candidates are usable.

This example used static PE analysis with Python, `pefile` and Capstone, then
Rust tests, build/package verification and an installation with rollback.
Native gameplay was still awaiting the user's test when this guide was written.
We did not recover the game's complete source or verify gameplay through an
interactive debugger.

## 1. Preserve the working baseline and the new build

Before editing compatibility code, preserve both versions independently:

- Executable and relevant native libraries.
- SDK headers/source, ABI information and version labels, if supplied.
- Asset/configuration bundles used by the mod.
- Build identifier and a manifest of file sizes and SHA256 hashes.
- The working mod package, its source state and its verification receipt.

Hash each saved file against its source, then verify the complete snapshot
again. Keep snapshots immutable. A version label alone does not establish an
exact binary: branches, hotfixes and distribution differences can matter.

In our case the user could temporarily revert the game. We first preserved
0.6.3, asked for 0.6.2, verified that its executable matched the working mod's
receipt, and preserved that too. The user then returned to 0.6.3. Further
analysis used those local snapshots, so repeated version switching was unnecessary.

Keep full game binaries, bundles and disassemblies in private research storage.
Our `research/` directory is ignored by Git; the mod release contains mod files.

## 2. Inventory what the mod actually depends on

Read the native adapter and data readers before searching the new executable.
Create a dependency list with meaningful names and evidence:

| Dependency | Record |
| --- | --- |
| Hook site | RVA, original bytes, instruction boundary, enclosing function |
| Original function | Entry RVA, arguments, return convention, callers |
| Field reader/writer | Object type, offset, width, meaning, proving consumer |
| Vtable/trait method | Table identity, slot, callee, object-pointer adjustment |
| Queue/collection | Header layout, element stride, ownership, valid lifetime |
| Thread-sensitive hook | Foreground/background role, lock scope, callback phase |
| UI/assets | Template names, expected elements, dimensions and data format |

Include helpers called directly by the mod, even if they are not patched.
Also inspect tests: fixtures can contain old field offsets and enum tags.

Use **RVAs**, not file offsets or absolute process addresses. For this Windows
PE target, runtime address = actual loaded module base + RVA. ASLR changes the
loaded base. A disassembler's preferred base is not a runtime guarantee.

Our working receipt contained **55 code/data checks**. Auditing their old RVAs
against 0.6.3 showed that all differed there. This established that the old
locations were unsuitable; it did not establish 55 changes in behavior.

## 3. Separate supported API changes from private native changes

Compare the supplied SDK and its ABI before rewriting anything. Identify which
parts of the mod use supported interfaces and which interpret private objects.

The two SDK snapshots in this migration had byte-identical code and declared
the same ABI level; only the base-version label changed. Private game layouts
still changed. Stable interfaces and private implementation details need separate
compatibility decisions.

Compare relevant assets independently too. We hashed all **316 UI/style
entries**: seventeen changed, while the in-match templates used by this mod
were unchanged. That let us keep HUD redesign work separate from compatibility.

## 4. Try conservative instruction fingerprints first

Our matcher reads x64 function ranges from the PE `.pdata` unwind table and
disassembles from established boundaries. It tries a whole-function fingerprint,
then a bounded instruction context around the old anchor.

For this target, it masks relocation-sensitive operands:

- Relative call/jump destinations.
- RIP-relative reference displacements.
- Immediate addresses pointing inside the executable image.

It retains opcodes, registers, ordinary constants and object-field offsets.
Those retained values help detect changes that could invalidate a data reader.
Mask operands identified by the decoder; do not erase every immediate or every
four-byte sequence that looks like an address.

Require complete decoding for a whole-function match and instruction-boundary
checks for a local-context match. Report no match, multiple matches and truncated
searches explicitly. A capped search must not become “unique” merely because
later filtering leaves one result.

The first comparison produced **13 unique candidates, one ambiguous case,
36 unresolved checks and five manual boundary/data cases**. Every result kept
`runtime_approved: false`. A unique fingerprint is useful evidence for review.

## 5. Resolve difficult cases through references and relationships

The executable retained Rust source paths and plausible source-location records.
We indexed their references and associated them with enclosing function ranges
in both builds. Paths and combinations of references narrowed the search even
when code changed substantially. Source line numbers shifted, so they were
clues rather than exact identifiers.

For each unresolved dependency:

1. Read its old enclosing function and explain the operation around the anchor.
2. Find new candidates using strings, source references, known callees or table
   entries exposed through a supported API.
3. Trace callers and callees. Check the dispatch branch, object passed, event
   arguments and any surrounding queue/lock operations.
4. Compare constructors, getters and consumers of the associated data.
5. Write down why the chosen candidate implements the required operation.

Preserve alternatives until evidence excludes them. Several helpers can share
the same prologue or near-identical implementations. Rust monomorphization can
also produce multiple tables with the same implementation pointer.

One important dependency here was the **foreground worker's publication
boundary after its locks were released**. Another was the viewer branch that
actually plays the visible match. Finding a generic send function or another
viewer dispatch was insufficient to establish those particular hook sites.

Two records classified for manual data review were actually small leaf routines
without unwind entries. Missing `.pdata` coverage does not prove a location is
data. Decode those routines separately and verify their references.

## 6. Use relaxed alignment to expose changes

For explicitly chosen old/new function pairs, we also generated side-by-side
disassemblies and instruction-sequence alignments. This review tool additionally
masked non-stack memory displacements to make shifted fields easier to compare.

That relaxed alignment is unsuitable as an approval rule: it hides exactly the
field changes that can break the mod. Inspect the original instructions in the
report. Register allocation, inlining and rearranged basic blocks can also make
an alignment pair unrelated operations. The reported percentage is a similarity
measure, not a probability that a hook is correct.

In this migration, alignment helped surface position, identity, skill and
collection shifts. Individual reads were then checked through their consumers.

## 7. Re-prove layouts, calling conventions and lifetime

For each native call, inspect argument preparation and the callee's use of it.
Check integer/floating-point arguments, stack arguments, hidden return buffers,
pointer adjustments and return-value handling. A matching function role does
not guarantee a matching ABI.

For a field, follow a named getter or a working native feature that uses it.
Native item buying and SDK getter implementations were especially useful here:

| Example dependency | Old | New | Supporting evidence |
| --- | --- | --- | --- |
| Native champion kind | 13 | 15 | Movement consumer and SDK getter |
| Position X/Y | +660/+668 | +658/+660 | Movement and steering consumers |
| Actor ID / level | +5c0/+5c8 | +5b8/+5c0 | Attack/skill consumers and level getter |
| Player build Vec | +550/+558/+560 | +350/+358/+360 | Native purchase selector |
| Native player getter slot | +140 | +158 | SDK gold getter implementation |
| AI settings borrow | Context +8 | Context → world → +1b8 | AI settings getter |

Offsets in this table are hexadecimal. The settings change required a different
pointer traversal. Some attack/skill budget fields, action fields and payload
strides stayed unchanged. There was no universal offset adjustment for the game.

For collections, establish capacity/pointer/count order, bounds and element
stride from actual code. For tagged variants, establish tag values and payload
layout. For vtables, check size/alignment, method slot and associated function.
Copy native data only during its valid borrow; do not retain callback-scoped
pointers for later rendering. An old SDK type name or the mod compiler's layout
is not sufficient evidence for a private native object.

## 8. Apply an explicit reviewed profile

After review, record an exact-build profile containing:

- Executable hash, loaded-image identity and address convention.
- Hook/helper RVAs, original bytes and decoded call targets.
- Field-consumer checks and relevant table-to-function associations.
- Supporting notes and unresolved limitations.

Back up the current source before applying changes. Update native readers,
fixtures, hook constants, identity guards and metadata together. Keep unrelated
feature work separate so a regression can be attributed to the migration.

Our profile records **55 locations plus 13 additional layout checks**. Runtime
checks verify the exact disk hash, loaded PE identity, selected instructions and
ASLR-adjusted table pointers before installing hooks. Unknown builds remain
unsupported. Candidate search and runtime installation are separate stages.

The source verifier also checks the Rust constants against the profile. This
catches a package assembled with reviewed JSON but stale adapter addresses.
It checks explicit constants and selected guards; it does not prove every reader
or infer the meaning of fields automatically.

## 9. Validate, package and hand off for gameplay testing

Use several layers of verification, each with a defined claim:

| Check | What it establishes |
| --- | --- |
| Snapshot hashes | Analysis used the intended preserved files |
| Instructions/call targets | Recorded locations match the target executable |
| Source/profile agreement | Listed implementation constants agree with the review |
| Regression tests | Tested buffering, reader and refusal cases behave as expected |
| Build/lint/format | The mod compiles and passes the selected code checks |
| ABI/null-host loading | The DLL exports the expected entry and rejects a missing host |
| Package/install hashes | The intended mod files were packaged and installed |
| User gameplay test | Native behavior and rendering work in a real match |

Our automated checks passed: 199 probe tests, 18 core tests, 19 migration-tool
tests and seven profile-refusal tests, plus build and artifact checks. These did
not establish that native gameplay passed. We installed with the game closed,
saved a verified mod rollback copy, and supplied a targeted match checklist.

Test dependencies that crossed a native boundary: startup, movement, attack/skill
weaving, aim direction, camera, death, modded skills/items and result handling.
Retain logs and match timestamps. If a static conclusion remains uncertain,
design a bounded runtime probe for that question rather than declaring it resolved.

## Tools and example commands from this project

These helpers are tailored to this Windows x64 PE/Rust game. Adapt the container
format, architecture, metadata assumptions, snapshots and profiles for another
mod. The source/reference tools are partial indexes, not exhaustive decompilers.

| File | Purpose |
| --- | --- |
| `tools/patch_migration.py` | Snapshot, audit old checks, propose conservative matches |
| `tools/inspect_game.py` | Source-path/location index for candidate functions |
| `tools/find_pe_references.py` | Targeted string/reference leads |
| `tools/trace_pe.py` | Decoded callers, field displacements and pointer references |
| `tools/review_patch_functions.py` | Explicit paired-function alignment and original disassembly |
| `tools/native_profiles/0.6.3.json` | Reviewed locations and identity |
| `tools/verify_native_profile.py` | Exact executable, target, pointer and source checks |
| `probe/src/native_profile.rs` | Runtime identity/layout guards |

Example commands from the project root, with Python dependencies available:

```powershell
python tools/patch_migration.py verify research/game-builds/0.6.2
python tools/patch_migration.py verify research/game-builds/0.6.3
python tools/patch_migration.py compare --old research/game-builds/0.6.2 --new research/game-builds/0.6.3 --receipt dist/build-0.41.0.json --output research/game-builds/comparison.json
python tools/inspect_game.py --exe research/game-builds/0.6.3/TeamfightManager2.exe --all-paths --output research/game-builds/new-source-map.json
python tools/verify_native_profile.py --profile tools/native_profiles/0.6.3.json
```

Supply `--receipt` explicitly for later migrations: the conservative tool's
current default is the historical 0.41 receipt. The verifier is tailored to this
project's source constants. It is not a universal verifier for another mod.

For paired review, create a JSON mapping of investigated function-start RVAs,
then run:

```powershell
python tools/review_patch_functions.py --pairs research/game-builds/pairs.json --old research/game-builds/0.6.2/TeamfightManager2.exe --new research/game-builds/0.6.3/TeamfightManager2.exe --output research/game-builds/paired-review.json
```

The accompanying `.txt` retains the original instructions. This helper currently
requires enclosing unwind ranges; leaf functions need separate inspection.

## Suggested handoff prompt for another AI

> Investigate migration of this RE-based mod from the known working game build
> to the updated build. Read project authorization rules and existing research
> first. Preserve and hash both complete inputs and the working mod/source state.
> Inventory every hook, native helper, private field, table and asset assumption.
> Use instruction fingerprints to propose candidates, then verify their role,
> caller branch, ABI, object layout, thread and lifetime. Keep ambiguous findings
> unresolved and explain what evidence is missing. Record an explicit reviewed
> profile and retain refusal of unknown binaries. Before implementation, report
> the concrete changes and obtain any approval required by the project. Validate
> source/profile agreement, tests, build and package; install with rollback when
> authorized. Supply a focused gameplay test and logs location. Distinguish static
> evidence, automated checks and observed in-game results throughout.

Related project notes: [initial RE approach](reverse-engineering-guide.md),
[migration workflow](patch-migration.md),
[specific 0.42 changes](patch-migration-pass-42.md), and
[gameplay checklist](../TESTING-42.md).
