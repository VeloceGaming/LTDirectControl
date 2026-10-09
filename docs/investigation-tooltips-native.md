# Native skill-description formatter investigation

Date: 2026-10-09. Scope: static inspection of the installed 0.6.3 executable,
SDK, and existing mod hooks. No mod code was changed, built, or installed.
No native formatter was called and no in-game result was tested in this pass.

## Rollout update: 0.69.1

The user confirmed both prototype champions worked perfectly. The 0.69.0 log
also contains successful native results for all six Illusionist/Alchemist
Q/W/R slots, each with zero remaining placeholders.

The formatter is now enabled for all champion IDs. The original six accepted
tables were insufficient for the game's individual built-in implementations.
Tracing `0x19beff0` identified 61 returned tables: 60 built-in implementations
and the registered mod wrapper. Together with the four previously inspected
data-defined table variants, the adapter checks 65 implementations. The 59
new built-in tables each have explicit header, drop/Q/W/R pointers, and lookup
return consumer guards. There are 407 tooltip anchors in total. The calling
and allocation-ownership bridge used by the successful prototype is unchanged.

Workshop skills no longer need an SDK-visible localized template before a
native request can be made. Missing templates use their reference and a known
localized base description as the cache's language identity. Unknown native
implementations and unavailable/empty results still fall back. No blanket
guarantee is made about descriptions supplied by third-party authors.

### Coverage audit

The old notes examined 190 skills. A fresh inventory of every nonempty English
Q/W/R description in the currently installed bundle gives **204 entries across
68 champions**. The audit uses that explicit current inventory rather than
assuming the older count was complete. It does not infer which optional skills
are usable in gameplay from translation presence alone.

- `tools/records/tooltip-skills.json` records IDs, slots, template parameter
  names and the source asset hash; it does not copy description text.
- `NATIVE TOOLTIP_AUDIT` logs one structured result per cached native request,
  including build/session, source, remaining parameter names, ellipsis count,
  byte count and fallback reason. Ordinary ellipsis punctuation is separate
  from unresolved parameters.
- `python tools/audit_tooltips.py` summarizes captured results. Pass `--logs`
  to select files explicitly; by default it uses the current user's diagnostic
  directory. Output defaults to `local/tooltip-coverage.json`.
- The pre-rollout report at `local/tooltip-coverage-before-0.69.1.json` shows
  six resolved native entries, zero observed unresolved entries, zero observed
  fallbacks, and **198 unobserved entries**. It contains no Workshop results.

The audit never starts the game or calls native functions. Wider runtime
coverage is collected as the user hovers skills in the game. All-champion
activation and static table coverage are not equivalent to having play-tested
every champion or Workshop override.

## Implementation update: 0.69.0 prototype

The user authorized implementation after this investigation. The current
cleaned-up integration points were re-read before editing:

- `probe/src/native_adapter/windows/tooltips.rs`: guarded native access and
  ownership, called after the existing viewer update; no new hook.
- `probe/src/native_tooltips.rs`: owned request/result cache only, with no
  native pointers and no game calls under its lock.
- `client.rs`: invalidates cache outside a controlled battlefield/session.
- `tooltips.rs` and `player_hud.rs`: request native text on hover and refresh
  an open tooltip when the next viewer callback provides the result.
- `native_adapter/windows/layout.rs` and `tools/native_profiles/0.6.3.json`:
  52 additional formatter anchors, table headers and method pointers, checked
  by both the adapter and the build verifier.

Only Illusionist and Alchemist are enabled. One request is serviced per viewer
callback, on demand; successful and failed requests are cached per champion,
skill, localized source and session generation. Other champions keep the
existing resolver. Native Strings and Arc references are released inside the
same viewer borrow. The explicit assembly bridge reserves Win64 shadow space
and captures RAX/RDX; its test fixture also checks stack alignment and writes
all four home slots.

### User test

1. Start control as Illusionist. Hover Q, W and R and leave each tooltip open
   briefly. Compare the filled description with the game's champion-info
   screen, especially Q's damage ratio/taunt duration and R's illusion time.
2. Repeat as Alchemist to exercise the nested-effect formatter. Empty optional
   skills should keep the ordinary fallback rather than retrying every frame.
3. Rehover and pause/resume, return to AI, then start a new match. Text should
   remain correct without stale content or crashes. A language change should
   obtain text for the new localized source.
4. `NATIVE TOOLTIP ... source=game` in probe.log identifies success. A
   `NATIVE TOOLTIP fallback ... reason=...` line identifies fallback. Set Log
   detail to Verbose if the exact returned description is needed for diagnosis.

The automated tests exercise the bridge with a stand-in callee, local heap
ownership and cache behavior. They do not execute the game's formatter or
establish its in-game appearance or stability. Broad rollout stays pending.

## Recommendation

Prototype the ChampionInfo description interface already used by the game's
champion-info screen. It returns localized descriptions after the game's
parameter substitution. Keep our current tooltip renderer and use its
existing resolver as a fallback when native access is unavailable.

This avoids reproducing dozens of champion-specific rules and the nested
effect resolver. Static evidence covers built-in, data-defined, and wrapped
mod entries. Live ABI, ownership, coverage, and performance still need a
small guarded prototype before making this the default tooltip source.

## Build identity

All addresses below are RVAs, added to the actual executable module base.
They apply only to this inspected executable:

- Game: Teamfight Manager 2, 0.6.3, Windows x64.
- SHA-256: `f21cf691799a83d9afa0ccae3e2b95862f5780446a501ee97860b91b752dbe2f`.
- Preferred image base: `0x140000000`.
- Image size: `0x52d6000`.

The 0.5.8 SDK archive symbol index helped identify method families. All
addresses and calling sequences recorded here were checked against 0.6.3;
old SDK symbols are not an ABI guarantee.

## The interface the game actually uses

The champion-info UI function at `0x22f85e0` invokes these slots of its
`dyn ChampionInfo` vtable:

| Skill | Vtable byte offset | UI call instruction RVA |
| --- | --- | --- |
| Q | `+0xe0` | `0x2300468` |
| W | `+0xe8` | `0x2300a46` |
| R | `+0xf0` | `0x2301018` |

The observed description call supplies:

- RCX: pointer to a 24-byte output String representation.
- RDX: concrete ChampionInfo object data.
- R8: borrowed game Assets context.
- RAX on return: output buffer pointer in the inspected wrappers.

The output representation is capacity, data pointer, and byte length. The
native UI moves this String into its label through `0x2308230`; the helper
releases the previous label text using the game allocator. These are owned
UTF-8 results, not borrowed template pointers.

## Built-in champion proof: Illusionist

The Illusionist ChampionInfo vtable is at `0x3beba38`.

| Skill | ChampionInfo wrapper | Action description method |
| --- | --- | --- |
| Q | `0x15e96b0` | `0x15c5860` |
| W | `0x15e9790` | `0x15bca30` |
| R | `0x15e95e0` | `0x15be1c0` |

These wrappers construct temporary actions from the champion's configured
data, call their description method, and release the temporary allocation.
The Action description slot in these vtables is `+0xd8`.

Q loads `#asset/base/text/champion?description.illusionist.skill`, inserts
Damage, Coef, and Time using its own fields, then finishes the localized
description through `0x1d2e60`. Its ratio comes from action offset `+8`;
its taunt time comes from `+0x10` and is divided by 60. W similarly fills
its own damage, ratio, time, and range. R fills its illusion duration.

This directly addresses the missing aliases and damage-type guess in our
Illusionist resolver. It is evidence of the native calculation path, not
yet a demonstration of the text returned to our mod at runtime.

The interface has no current entity or target argument. It provides the
game's configured description and formulas; it does not calculate live
damage against a particular enemy.

## Data-defined and mod-defined champions

The generic description resolver at `0x14d5f00` works with DataActionDef,
reads effect fields through `0x14cde10`, and fills the large native parameter
set, including damage, coefficients, cloud radius, and periodic-effect
parameters. Calling this low-level resolver directly is unnecessary.

Two inspected DataChampionInfo vtables, `0x3bee2e8` and `0x3b8ba70`, use
the same Q/W/R interface:

| Skill | Description method |
| --- | --- |
| Q | `0x1506e50` |
| W | `0x1507100` |
| R | `0x1506df0` |

All three reach the generic resolver. This is the path relevant to the
20 champions whose nested effects were missing from our extraction.

The ModChampionEntry vtable at `0x3be38f0` uses Q `0x19f0f10`, W
`0x19f13c0`, and R `0x19f0cf0`. These methods handle data overrides through
the generic resolver, or forward to the wrapped ChampionInfo's same
description slots. Use the effective entry returned by the registry so
installed overrides are retained.

This does not guarantee that every Workshop mod supplies valid or complete
text. The SDK's StableAction description callback is an author-provided
description interface; it is not a public live description getter for our
HUD. Native text can still contain an author's unresolved placeholders or
other mistakes. We should preserve and report those rather than invent data.

## Access without a new hook

Our existing viewer hook in `probe/src/native_adapter/windows/view.rs`
already receives Assets as its fifth argument. The native viewer at
`0x930640` passes that argument to the in-game UI at `0xb9c970`.

The in-game UI itself demonstrates this lookup sequence:

1. Obtain the typed ChampionInfoSheet asset through `0x338420`, passing
   Assets and `asset/base/setting/champion_info` (UTF-8 length 32).
   The native call is at `0xbb9924`.
2. Resolve the champion's effective entry through `0x19d1bf0`, passing
   sheet, Assets, champion ID pointer, and ID byte length.
   The native call is at `0xbb9959`.
3. Use the returned ChampionInfo to call Q/W/R description slots.
4. Copy the result into mod-owned text, then release all native ownership
   before leaving the viewer hook.

The lookup returns an optional Arc trait object in **RAX and RDX**: allocation
pointer and vtable. Do not declare this as a C function returning a two-word
struct: that ABI can use a hidden output pointer instead. A small explicit
register bridge is needed unless an equivalent return convention is proven.

For a non-null Arc, object data starts at:
`allocation + 16 + ((vtable_alignment - 1) & ~15)`.
The native consumer computes this at `0xbb9984` through `0xbb9992`.
Do not assume every third-party implementation has alignment eight.

Use the viewer thread's current borrowed Assets only. Do not retain native
objects or pointers in the HUD, or combine simulation-thread Action pointers
with viewer-thread Assets. The existing combat Action layout is unnecessary
for this interface.

## Ownership and failure handling

- The native consumer decrements the Arc's strong count at `0xbb99aa`, and
  calls `0x28b20` on a pointer to the two-word Arc when that count reaches
  zero. This helper is **drop_slow**, not a complete unconditional Arc drop;
  calling it without the preceding strong-count transition is incorrect.
- Native String allocations must not be dropped as a mod Rust String.
  The inspected native text cleanup tests nonzero capacity, then uses
  GetProcessHeap and HeapFree. Their current IAT slots are `0x39cad00`
  and `0x39cacf8`, respectively. Copy bytes first, and release the native
  allocation with the matching game allocation contract exactly once.
- Validate the expected module/profile, instruction anchors, vtable layout,
  executable method addresses, string length/capacity, and readable output.
  Unknown builds or failed validation must disable native resolution.
- A fallback handles absent assets, absent descriptions, and rejected
  validation. It does not make a bad native call safe: native faults are not
  reliably recoverable with Rust catch_unwind. Prove the bridge on a small
  sample before widening coverage.

## Proposed implementation and validation

Keep native access in one small Windows adapter module. Expose only owned
description snapshots to `tooltips.rs`, preserving our typography, wrapping,
colors, inline icons, cooldown/range rows, and layout.

The first prototype should resolve Illusionist Q/W/R and one data-defined
champion in the existing viewer callback, with opt-in diagnostics and the
current resolver available for comparison. Add profile anchors for the
lookup, UI dispatch slots, and ownership paths before enabling calls.

After the first live result is confirmed:

1. Compare English and the user's active language with the game's own
   champion-info screen, including markup and number formatting.
2. Exercise a built-in champion, nested-effect champion, a mod override,
   missing/empty optional skill, and any available Workshop champion.
3. Audit all 190 base skills again using captured native output. Count
   unresolved placeholders separately from legitimate ellipsis punctuation.
4. Reopen tooltips and enter subsequent matches; check allocation balance
   and memory use, and confirm cache invalidation after language or data
   context changes.
5. Cache successful owned text per champion and language within the current
   asset/session generation. Never resolve every tooltip on every frame.
   Invalidate safely when that context changes; avoid retaining native pointers.

The user must verify in-game appearance and behavior. No claim of complete
190-skill coverage, Workshop correctness, or runtime stability is established
by the static investigation alone.

## Private research evidence

Local disassembly extracts are under `research/tooltip-*.asm`, including
description methods, ChampionInfo bridges, generic bridges, champion lookup,
and the champion-info UI's indirect calls. They support this document and
are not release artifacts or a reason to ship executable contents.
