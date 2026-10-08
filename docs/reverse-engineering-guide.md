# How this project investigated Teamfight Manager 2

Rough guide for another AI or developer, written on 2026-10-06. This describes
the methods actually used in LTDirectControl, including approaches that failed.
It is a starting map, not a complete description of the game's internals.

For the later 0.6.2-to-0.6.3 update process, see the separate
[RE mod migration guide](re-mod-migration-guide.md). Addresses below describe
the historical 0.6.2 investigation; the current 0.42 adapter targets 0.6.3.

## What “reverse engineering” meant here

Most of the work was **static analysis**: reading the installed SDK, parsing
the Windows executable and asset bundles, disassembling selected functions,
and following their callers and data accesses. Small **runtime probes** in our
mod then recorded what those functions did during real matches. The user
played the game and reported the visible behavior.

The main tools were custom Python scripts using `pefile` and Capstone,
SDK archive inspection, LLVM output and CodeView debug records, and bounded
logs from our Rust mod. We did not reconstruct the game's complete source,
single-step the whole game in an interactive debugger, or personally verify
gameplay by playing it. Reference mods' logs and DLL strings provided clues;
they did not establish how those mods implemented every feature.

The recurring method was:

> Find a working native feature → trace the code and data it uses → establish
> its thread, lifetime and calling convention → probe one specific hypothesis
> → compare the log with the user's observation.

## 1. Start with the supported API and an exact build identity

Read the stable SDK before touching native code. In this game it already
provided client lifecycle callbacks, UI drawing, callback-scoped simulation
reads, player AI input replacement, input validation and simulation-origin
information. Those were useful foundations. Native work filled particular gaps:
live worker/playback coordination, some ability execution details, and data
registered by other mods that the original settings JSON did not expose.

Two questions must stay separate:

- Can the SDK replace this player's input in a simulation?
- Is that simulation producing the battlefield currently visible to the user?

A foreground match can still be simulated far ahead of its presentation.
“Foreground” alone does not prove live control.

Record the executable hash, PE timestamp, image size, SDK version and enabled
mods. The native adapter at the time of this investigation targeted this
specific 0.6.2 executable:

```text
SHA256: 15df9eb3b6915cdcc4c2ebb3b7f5fa232b563c4cdd32208581634817b71adc23
PE timestamp: 0x6ABC597E
SizeOfImage: 0x052B8000
Preferred image base: 0x140000000
```

These values identify this target, not every installation called “0.6.2.”
Historical research addresses are relative virtual addresses (RVAs).
Runtime address = actual loaded module base + RVA. File offsets are different,
and ASLR means the preferred image base is not a universal runtime address.
Do not reuse offsets on a different binary by merely removing a version check.

Start reading [investigation.md](investigation.md), then the current code in
[`probe/src/lib.rs`](../probe/src/lib.rs) and
[`probe/src/native_adapter.rs`](../probe/src/native_adapter.rs).

## 2. Turn strings and metadata into candidate functions

The executable retained Rust source-path strings and source-location metadata,
including paths under `game-core/src` and `game-view/src`. Those were a useful
index into an otherwise largely unnamed binary.

[`tools/inspect_game.py`](../tools/inspect_game.py) parses the PE, locates those
strings and plausible location records, finds candidate references, and maps
them to function ranges from the x64 `.pdata` unwind table.
[`tools/find_pe_references.py`](../tools/find_pe_references.py) performs more
targeted string-reference searches.

This narrows the search; it does not recover source code or prove a hook.
A source path can belong to shared infrastructure, an error branch or an
inlined operation. A byte pattern can occur inside another instruction.
Some leaf functions have no unwind entry. Treat the output as leads.

Next, use [`tools/trace_pe.py`](../tools/trace_pe.py) and
[`tools/disassemble.py`](../tools/disassemble.py) to decode instructions from
established function boundaries. Follow verified direct calls and jumps,
RIP-relative references and specific object-field displacements. Trace helpers
cover particular instruction forms; no result is not proof of no reference.

For example, a decoded load from `[rcx + 0x30]` establishes a field access at
that offset. It does not by itself tell you whether the field is a queue,
upgrade list or some unrelated vector. Its constructors and consumers supply
the meaning.

## 3. Use older SDK debug information as a dictionary

Older Rust SDK `.rlib` archives contained useful symbol names and, in some
members, LLVM bitcode with debug metadata.
[`tools/inspect_rlib.py`](../tools/inspect_rlib.py) reads archive indexes and
COFF symbols/relocations. It explicitly distinguishes LLVM bitcode from COFF.
[`tools/read_sdk_debug.py`](../tools/read_sdk_debug.py) extracts bitcode members
and parses textual CodeView structure/field records from emitted assembly.

The existing Rust LLVM tooling was used to produce static assembly and debug
records from relevant archived members. This gave names and layout clues for
client, view and worker structures. It did not require loading the old SDK
code into the game. Saved examples include
`research/old-server-worker.bc` and its emitted assembly.

An old named field is a hypothesis for the current executable. Revalidate its
offset, access pattern and callers against the current binary. Old scene tags
and apparent tutorial-specific names were particularly misleading here.

## 4. Follow consumers to prove layout and ABI

The strongest static evidence usually came from a feature the game already
handled correctly: native item buying, an item tooltip, frame playback, or a
skill consumer. Work backward from that consumer to its data source, and
forward from constructors to see how that data was populated.

For a native call, establish:

- Argument registers and whether an object pointer is adjusted first.
- Whether a return value is in registers or a hidden return buffer.
- Which fields are read, and how the caller interprets them.
- Object ownership, lifetime, and the thread on which access is valid.

Do not assume Rust types in the game have the layout of similarly named types
compiled into the mod. For example, the registered-item vector's raw words
were established from this binary as capacity, pointer and length. That is
evidence about this object, not a promise about all Rust vectors.

The item-stat getter used a hidden return buffer. Its calling convention and
numeric field offsets were established from the native tooltip caller; it was
not treated as an SDK `BuffV1`. Returning a native Rust `String` is a different
problem with ownership and ABI implications. We have not established the
complete effective native ability-description formatter ABI, so some dynamic
tooltip formulas remain unsupported.

The current example is
[`probe/src/native_items.rs`](../probe/src/native_items.rs). It checks the
binary and instruction anchors, validates the expected bridge, bounds lengths,
and copies owned values during the valid callback. Native object pointers and
vtables are not retained for later HUD use. Bounds and alignment checks alone
would not establish that an arbitrary pointer is valid.

## 5. Probe a question, not everything at once

Once static analysis gives a plausible site, record a small set of facts:
hook entrances, thread IDs, stack RVAs, session identity, simulation origin,
tick number, publication count and playback count. Log entrance before scope
filters when distinguishing “hook never ran” from “hook ran but was rejected.”
Bound the capture so logging does not become a source of stutter.

Define an expected result and a competing explanation before the test. For
example: “If this is the live publication boundary, a brief hold after the
initial frame should stop new battlefield frames while the client UI remains
responsive.” The user's report of whether the battlefield stopped is part of
the evidence; a selected pause icon is not enough.

Keep these evidence levels explicit:

| Evidence | What it establishes |
| --- | --- |
| String, old symbol or nearby code | A research lead |
| Decoded current caller and data flow | A specific structural relationship |
| Runtime log | What the instrumented site observed in that run |
| User's game test | Visible behavior and control feel in that scenario |
| Unit tests and packaging checks | Our logic, selected binary anchors and package integrity |

None of these alone establishes every other level. In particular, an input
validation success is not proof that an ability executed, and a recorded aim
point is not proof that the eventual projectile used it.

## 6. Three examples, including wrong turns

### Live startup: producer and viewer were separate

Initially, waiting inside the AI callback seemed sufficient to stop the match.
It could prevent the tick from returning and producing the frame needed for
startup. Other early attempts watched the wrong playback site or used stale
scene assumptions. A shared worker was also incorrectly classified as
tutorial-only until a real foreground match's stack disproved that label.

The useful reconstruction separated:

```text
simulation tick → output preparation and locks → successful frame publication
                → client reception/queue → viewer consumption → presentation
```

The hold/pacing boundary had to follow the relevant publication and lock
release, rather than wait inside the SDK AI callback. This distinction also
explains why “paused” UI state could coexist with a moving battlefield.
Not every normal-loader dependency was conclusively identified; historical
notes contain corrections rather than a claim that every early lock hypothesis
was proven.

See [startup-research.md](startup-research.md) for the experiments and
[native-adapter.md](native-adapter.md) for the integration. Version 0.32 uses
a one-frame running lead and viewer notifications, but its responsiveness and
starvation behavior still require the user's current test.

### Items: correct data, wrong interpretation

Modded items could appear correctly in the game's detail panel and be bought
correctly at base, yet our tracker could not explain them. The original SDK
settings JSON was incomplete for runtime registrations. We followed native
buying and tooltip consumers to the registered item catalogue instead.

One initial interpretation still failed: an upgrade list was named as previous
tier links, when constructor, getter and buyer analysis later established it
contained forward links. The buyer traversed those links backward from a goal,
which made the first reading misleading. Later, forecasting also wrongly
excluded tier-4 items; those were legitimate final items in the captured builds.

This is why finding a pointer is only half the work. Prove graph direction and
algorithmic meaning with constructors, getters, consumers and observed builds.
See [selection-items-pass-31.md](selection-items-pass-31.md) and
[control-pass-32.md](control-pass-32.md). The earlier
[items-pass-30.md](items-pass-30.md) explicitly marks its superseded assumption.

### Skills and selection: physics is not presentation

A command accepted by the SDK could still meet a busy-action or cooldown gate
in the native Q/W/R consumer. Comparing cooldown budget before and after that
consumer supplied execution evidence, supporting bounded command buffering.
It did not prove projectile direction or damage. The reported aim assistance
remains an investigation rather than a settled cause.

Likewise, combat collision radius was a poor proxy for the sprite the user
tries to click. Asset animation dimensions gave body envelopes, excluding
separate effect animations. Picking needed camera projection, zoom and
padding; highlights needed to derive from the same visual envelope. Atlas
coordinates are texture coordinates, not automatically world-space pivots.
Actual combat collision and ability range stayed separate from click geometry.

See [control-pass-29.md](control-pass-29.md),
[targeting.md](targeting.md), and [control-pass-32.md](control-pass-32.md).

## 7. Keep native integration narrow and recoverable

The current adapter verifies the executable fingerprint, loaded PE headers and
expected instruction bytes before touching process memory. It redirects
specific decoded CALL/JMP sites, preserves known calling conventions, and
calls the original handlers where required. This is not arbitrary replacement
of whole functions based on nearby strings.

It handles relative-branch range with nearby relays, changes executable memory
permissions deliberately, flushes the instruction cache, and keeps hooked code
resident. Installation happens before live worker/view activity. The game
executable on disk is not patched by this adapter.

Scope checks keep native control tied to the selected player and session;
release and timeout guards restore normal behavior. An unexpected build or
changed hook bytes causes rejection rather than a best-effort jump. Rust
`catch_unwind` catches Rust panics, not arbitrary bad-pointer access violations.

Put unsafe access in a small native boundary. Return owned snapshots to the
ordinary UI/control code. A working implementation should not require every
HUD element to understand offsets, native allocation or thread ownership.

## Tool map and a first read-only session

| Tool | Purpose |
| --- | --- |
| `tools/inspect_game.py` | PE fingerprint, source anchors, candidate functions |
| `tools/find_pe_references.py` | Targeted string-reference leads |
| `tools/trace_pe.py` | Decode candidate callers, field accesses and references |
| `tools/disassemble.py` | Annotated current function disassembly |
| `tools/inspect_rlib.py`, `tools/read_sdk_debug.py` | SDK symbols, archive members and debug layouts |
| `tools/read_ability_native.py`, `tools/read_recall_native.py` | Version-specific consumer investigations |
| `tools/read_hud_assets.py`, `tools/read_picking_assets.py` | Targeted asset metadata extraction |
| `tools/verify_ability_build.py`, `tools/verify_ui_graphics.py` | Selected native anchors and asset checks |

From the repository root, with a Python interpreter that has `pefile` and
Capstone available (this project also looks in `.tools/python`):

```powershell
$gameExe = 'C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2\TeamfightManager2.exe'
python -X utf8 tools/inspect_game.py --exe $gameExe --output research/my-build-map.json
python -X utf8 tools/find_pe_references.py --exe $gameExe --needle 'game-view/src/view/game.rs' --output research/my-view-references.json
python -X utf8 tools/inspect_rlib.py --archive 'PATH_TO_SDK_ARCHIVE.rlib' --pattern 'GameClient|GameView' --output research/my-sdk-symbols.tsv
```

Replace the executable/archive paths with actual local paths. These commands
read source binaries and write research artifacts; they do not install hooks.
Use unique output filenames to preserve earlier evidence. UTF-8 mode matters
for localized text on Windows.

After identifying a candidate, `trace_pe.py --call 0xRVA --output PATH` queries
decoded direct callers, and `--disp 0xOFFSET` queries field accesses. Supply one
query mode per invocation and the target executable. RVAs and field offsets
mean different things. `disassemble.py --rva 0xRVA --output PATH` additionally
expects the project's `research/game-0.6.2-map.json` to exist; adapting it to a
new build needs attention to that dependency. Feature-specific scripts contain
hardcoded current-build assumptions and are examples, not universal tools.

## What another AI should do first

1. Read the SDK and current adapter; identify the exact missing behavior.
2. Fingerprint the binary and list relevant enabled mods and runtime phases.
3. Find the working native consumer closest to that behavior.
4. Establish current instruction boundaries, data flow, ABI and lifetime.
5. Write down a falsifiable hypothesis and one small runtime test.
6. Obtain authorization before editing/building under this project's rules.
7. Run bounded probes, then have the user verify the visible result.
8. Record rejected explanations as well as successful evidence.

A useful handoff note contains: **question; binary identity; candidate RVAs;
decoded evidence files; interpretation and alternatives; thread/lifetime;
expected test result; actual log and user observation; unresolved limits.**

Treat `research/` and the early investigation documents as an experiment
history. Later documents often correct earlier claims. Current code tells you
what is implemented, but even current code is not proof of correct native
behavior. Keep the distinction visible so the next AI can continue the
investigation rather than inherit our assumptions as facts.
