# Patch migration workflow

This is a local research tool, not a runtime signature scanner or an automatic
patch installer. It preserves complete build inputs, checks old anchors, and
proposes code relocations for review. It never runs the inspected executable,
edits native addresses, changes the guard, or installs a mod.

For the reusable investigation method and an AI handoff prompt, see
[Migrating a reverse-engineered mod](re-mod-migration-guide.md).

## Current checkpoint: 2026-10-07

The installed game is back on the preserved 0.6.3 executable. The authorized
0.42 migration relocates 55 checks and updates changed entity/player/settings
fields. It adds 13 loaded-layout guards and an explicit profile/source verifier.
See [migration details](patch-migration-pass-42.md) and
[native test checklist](../TESTING-42.md). Native gameplay is pending user test;
the snapshot/comparison history below describes how this review started.

The user authorized preserving 0.6.3 and starting the tooling, and asked to be
notified when a temporary revert to 0.6.2 is needed.

0.6.3 is preserved in `research/game-builds/0.6.3/`:

- 37 files, 1,240,261,756 bytes: executable, full bundle, stable SDK/template,
  Steam API DLLs and Steam build manifest.
- Executable SHA256:
  `f21cf691799a83d9afa0ccae3e2b95862f5780446a501ee97860b91b752dbe2f`.
- Steam build ID: `25769776`; SDK base version: `0.6.3`.
- Every copy was hashed against its source. A second full verification passed.
- `old-address-audit.json`: all 55 0.41 checks differ at their previous addresses.
  This does not establish that the corresponding functions changed semantically.
- `source-map.json`: 308 source paths and 2,634 candidate functions. Source
  references are investigation leads, not verified native entry points.

The user reverted to 0.6.2. Its executable matched the existing receipt exactly,
and the complete baseline is now preserved in `research/game-builds/0.6.2/`:
37 files, 1,240,049,040 bytes, copied and hash-verified. Its executable SHA256 is:

`15df9eb3b6915cdcc4c2ebb3b7f5fa232b563c4cdd32208581634817b71adc23`

The first real comparison is saved in
`research/game-builds/migration-0.6.2-to-0.6.3.json`. Of 55 anchors, 13 have unique
review-only candidates, one has two candidates, 36 are unresolved, and five need
manual boundary/data review. Two of those five are code-shaped leaf routines
without unwind entries; absence from `.pdata` does not establish that they are
data. Source references, callers and associated consumers remain necessary to
resolve and verify these locations.

The snapshot manifests differ only for the executable, full bundle, SDK base
version label, and Steam manifest. SDK code is byte-identical and both SDKs
declare ABI level 9. That does not verify private native object layouts.

The user returned to 0.6.3 and the verified 0.42.0 migration is installed.
Both preserved binaries and SDKs remain available for offline comparison;
repeated live version switching is unnecessary. Unknown executable builds are
still refused. The approved HUD, native font size/weight verification and tower
selection remain follow-up work after the compatibility test.

## Commands

Use the project Python runtime with `pefile` and `capstone` available. The tool
finds the current local dependencies in `.tools/python`, with normal Python
package resolution as a fallback.

Capture a new build before Steam replaces it:

```powershell
python tools/patch_migration.py snapshot --label 0.6.3 --output research/game-builds/0.6.3 --expected-sha256 f21cf691799a83d9afa0ccae3e2b95862f5780446a501ee97860b91b752dbe2f
```

After the user reverts, capture the exact known working 0.6.2 build:

```powershell
python tools/patch_migration.py snapshot --label 0.6.2 --output research/game-builds/0.6.2 --expected-sha256 15df9eb3b6915cdcc4c2ebb3b7f5fa232b563c4cdd32208581634817b71adc23
```

If that fingerprint differs, stop and investigate the installed branch/build.
Do not replace the expected fingerprint merely because the version says 0.6.2.

Verify snapshots and generate the migration report:

```powershell
python tools/patch_migration.py verify research/game-builds/0.6.3
python tools/patch_migration.py audit research/game-builds/0.6.3 --output research/game-builds/0.6.3/old-address-audit.json
python tools/patch_migration.py compare --old research/game-builds/0.6.2 --new research/game-builds/0.6.3 --output research/game-builds/migration-0.6.2-to-0.6.3.json
python tools/test_patch_migration.py
```

Verify the reviewed 0.6.3 profile and source agreement after implementation:

```powershell
python tools/verify_native_profile.py --profile tools/native_profiles/0.6.3.json
python tools/test_native_profile.py
```

Outputs must stay under `research/`. Snapshot contents are immutable: differing
existing files are preserved and rejected. Reports cannot overwrite a snapshot's
manifest or preserved files. Commercial game binaries/bundles and Steam metadata
are private local research inputs: `research/` is ignored by Git; never include
them in the public repository or mod releases.

## Candidate matching

The baseline executable must match the known build receipt, and every old anchor
must match its recorded bytes before comparison starts. Code matching uses x64
unwind function boundaries and decoded instruction boundaries, not prologues
alone. Whole-function fingerprints are attempted first, followed by bounded
instruction context if the function has changed.

Relative call/jump operands, RIP-relative references, and image-address
immediates are masked because relocation changes them. Opcodes, registers,
ordinary constants and object-field displacements are retained. Weak signatures
are rejected. Multiple matches remain ambiguous; hitting the search limit is
explicitly reported and cannot become a unique result after candidate filtering.
Direct-call proposals include the newly decoded call target for cross-checking.

Data/vtable records are marked for manual review. The tool does not yet relocate
them from associated function evidence. Changed instruction contexts may remain
unresolved. Every result has `runtime_approved: false`, including unique matches.

Before accepting a migrated build, inspect candidates' callers/consumers, calling
conventions, object/vtable layouts, live-worker ownership and behavior. Retain
executable identity and byte checks for the newly verified build. User game tests
remain necessary after automated validation.

## Tool verification

19 tests passed: masking preserves field/scalar offsets while tolerating moved
references; duplicates and weak patterns are handled; snapshot corruption,
wrong baseline hashes, path escapes and attempts to overwrite preserved files
are rejected. A real-PE same-build smoke check produces a review-only unique
candidate. That check validates the matching pipeline, not cross-version
compatibility or native gameplay.

The initial comparison report remains a record of unapproved candidates.
Subsequent source/caller/layout review produced the separate explicit profile
used by the installed 0.42 compatibility build; see the migration notes above.
