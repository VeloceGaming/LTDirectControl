# Migrating to game 0.7.0 Beta 2

What was established for 0.7.0 Beta 2, the beta branch of 2026-10-21. The
method is the runbook in [version-migration.md](version-migration.md); the
previous record is [migration-0.6.5.md](migration-0.6.5.md). Beta 2 reports
itself as "0.7.0"; the mod's tables call it `0.7.0b2` (`native_adapter/v070b2`,
`tools/native_profiles/0.7.0b2.json`).

Everything below is **read from the code** unless a line says otherwise.

**Seen in game** (the author, 1.5.0-dev27 on Beta 2, one match as Harpy,
2026-10-21): the hooks installed with session, camera, movement, combat,
shop and stats; `STATRULE installed sites=35`; the camera took over; W and
R were refused until learned; the mod's own cancel-recall event fired 9
times; 5 purchases; all four stat rules fired (the area gate 4130 times);
no errors or stalls; the match ran to the result screen (tick 31895).
The author: recalls ended correctly ("Looks correct"). Bombardier was not
in the match; the author will test it on the next version.

**Seen in game** (the author, 1.5.0-dev28 on Beta 2, 2026-10-21/22): the
minimap, outlines and native skill tooltips "look correct" (Construct's
three skill texts came from the game, `source=game`). Bombardier played
2026-10-22: "all looks correct except the preview". Read from that log:
the preview table has no Bombardier entry and the game's data calls its
three skills `Native` (built-in code), so no shape is drawn; its three
skill tooltips came from the game. Two old limits found in the same
match: skill health parts are not worked out (kept, the author's choice;
docs/backlog.md, Declined) and item effect values showed "…" (fixed in
1.5.0-dev29, docs/investigation-item-text.md).

## Build

| | 0.6.5 | 0.7.0 Beta 2 |
| --- | --- | --- |
| Executable SHA256 | `daf040db…11d30b5` | `79423383a353b63f4a4f907889eae2d6cc778c53519e9e08828b200c2a88e744` |
| PE timestamp / image size | `0x6ad85238` / `0x5309000` | `0x6ad85082` / `0x58fb000` |
| SDK | ABI 10 | ABI 10 (identical apart from the version text) |

## What is supported

- **1.5.0-dev27:** session, camera, movement, combat, shop and the athlete
  stat rules (the author's choice: core families first).
- **1.5.0-dev28:** adds the minimap, the outline and the native tooltips
  (below). Not seen in game yet.
- The research skill reader (research builds only) stays off on Beta 2: its
  table (`effects.rs`) holds values no address can have, never another
  version's, and the profile names none of its anchors.

## How the tables were found

1. Drafted from 0.6.5 (`tools/draft_version.py --old 0.6.5 --new 0.7.0b2`).
2. The function map grew from 856 to 1870 functions after seeding four
   pairs found by hand (the viewer dispatcher, the viewer, the worker
   caller and its twin). Beta 2's combat functions are 35-110 % larger, so
   alignment alone carried less than on 0.6.5.
3. The stat census found all 35 stat sites and every one passed the field
   check, gates included (the tools built for 1.5.0-dev23).
4. Everything else by hand: aligned call sites, callers, byte searches,
   vtable slots. `tools/field_shifts.py` over every hooked function: the
   entity fields the hooks read are unchanged.

## What changed that the hooks depend on

- **Worker.** The hooked caller is `0x9aef00` (it loads `rsi = [rcx+0x28]`,
  `rdx = [rcx+0x30]`; its twin `0x9afc30` loads `rdx = [rcx+0x28]`). It now
  calls the channel send as a function (`0xc9dd90`; 0.6.5 had it inlined)
  with the same arguments and the same "-1 = sent" result. Sends at
  `0x9af41b` and the drain loop `0x9af673`; the screen-delay queue length
  at `0x9af00d`, rewritten as on 0.6.5.
- **Viewer and input.** View site `0xc5d0b0` (the third of six viewer calls
  in the dispatcher, as on 0.6.5) -> `0x99a910`; input `0xc853dd` ->
  `0xbd4ed0`. Frame clock `+0x2f0` and pan velocity `+0x3f0` unchanged.
- **Camera: the in-game UI's type id changed.** The camera family checks
  the in-game UI's Rust type id before using it. 0.6.4/0.6.5:
  `[0x810a3f1ff5177337, 0xaf7a6919a6336e28]`; Beta 2:
  `[0xcde80fac27312f00, 0x0c137aa247a3e0e1]`. It was written in the shared
  code; it is now `layout::INGAME_UI_TYPE_ID` per version. The UI's fields
  are unchanged (full width `+0x9234`, config link `+0x9150`), and every
  camera guard was found: the renderer's five config reads (same order),
  the full-screen toggle, the pause check, and the UI builder's bindings.
  The announce and kills layouts now take extra numbers but read the same
  two flags first. Beta 2's console UI code is only read, never changed.
- **Movement.**
  - Move `0x1c4b640` (49 -> 104 instructions). It refuses while the
    champion has a `bombardier_deployed` status (a new list at
    `+0x2d8/+0x2e0`, 0x120-byte entries, kind 0x13, the name at +4; new
    champion Bombardier). The mod's own move check has the same rule
    (`MOVE_BLOCK_STATUS`).
  - **No cancel-recall function.** Beta 2 builds that event inline in move,
    attack, Q, W, R and two other functions. The draft's candidate was the
    move function itself, whose first bytes match the old function's. The
    mod builds the event the game's way on Beta 2
    (`CANCEL_RECALL_INLINE`): an entry appended with the game's stop-event
    function, turned into tag 0 with a pointer at +8 to a payload
    `{0x1c, 0, actor}` from the process heap, which the game's allocator
    uses (`0x3572150`: `GetProcessHeap` + `HeapAlloc`).
  - Stop event `0x1c07440`: identical to 0.6.5's, still used by the game.
    Steer `0x1c4a120` (site `0x1c43bf6`), direct step `0x1c3b400` (site
    `0x1c436b5`): same arguments.
- **Combat.** The input dispatch `0x163a6f0` is instruction-for-instruction
  0.6.5's: move `0x163a884`, attack `0x163a930` -> `0x1c39900`, Q
  `0x163a961` -> `0x1c4c140`, W `0x163a8b0` -> `0x1c3a1a0`, R `0x163aa13`
  -> `0x1c4a7c0`. Auto-attack `0x1c3dfcf`, aim `0x1c41369` -> `0x1c57b70`.
  Same arguments everywhere; the attack function keeps the entity in `rbx`
  (was `rdi`). The entity getter `0x33cf030` is 0.6.5's.
- **Ranges gained a method.** Auto-attack and skill cast ranges add a call
  to the slot's effect object (`vtable + 0x120`). All 276 effect types
  found return 0 (`0xa1250`: `xor eax, eax; ret`), so the mod's own ranges
  still equal the game's. **Guess:** Bombardier might add range when
  deployed; to check in game. *Update 2026-10-22:* read from the data,
  deployment adds `move_distance` (50) to its attack and W range (its R
  has its own); the author saw nothing wrong with Bombardier in play, but
  whether the mod's range ring grew while deployed was not checked
  specifically. **Seen in game** (the author, 2026-10-22, 1.5.0-dev28):
  deployed, Bombardier's attack and W showed the correct range in the skill
  preview's range ring.
- **Shop.** next_step `0xf06e50` and goal_search `0xf06b40` are 0.6.5's;
  decisions `0x107b440` (upgrade) and `0x107b5b0` (new item), thunks
  `0x10ab120`/`0x10ac5f0`, slots `0x4042fb0`/`0x4042fa8`, executor
  `0x16c6cd0` (gate `0x16cd372`, payments `0x16cce82`/`0x16cd42e`). Player
  record as on 0.6.5 (items `+0x330`, plan `+0x370..`, gold `+0x888`).
  Item AI bridge `0x426cc58` (player `0x3441300`, settings `0x3441fd0`).
- **Skill numbers.** Hammerer W's skill method table `0x40fdcf0`: name
  `0x1b272f0` ("hammerer_skill2"), get `0x1b27110`, cooldown `0x13a9580`.
- **Stats.** 35 sites. One Accuracy producer stores at frame `+0x2c8`
  (0.6.5: `+0x2d8`); the hit tests that read it follow.

## Minimap, outline, tooltips (1.5.0-dev28)

- **Minimap.** All 54 operands the mod repoints are the same instructions
  with the same values on Beta 2: the renderer `0x2e8a550` (its minimap part
  matches 0.6.5's once frame offsets and registers are ignored) and the
  input handler `0xbd4ed0`. Every constant they use has as many uses in
  these two functions as on 0.6.5, so they gained no use the mod would miss
  (a console minimap drawn by another function was not looked at; the mod
  does not touch it). The mod points the
  instructions at its own constants and never writes the game's data.
- **Outline.** The unit renderer is `0x2b641b0`: its 15 callers are the 15
  call sites the draft placed (0.6.5: 15), and each passes 8 arguments, as
  on 0.6.5. The shader setter `0x1cb310` (recompiled; same arguments and
  command fields), the float and colour setters `0x26ea470`/`0x26e9990`
  (equal to 0.6.5's apart from relocations) were confirmed by their calls
  inside the unit renderer. The command drop `0x1c6f30` and its table are
  instruction for instruction 0.6.5's; commands are still 0xd0 bytes and
  the Sprite fields the outline and selection use (+0x68..+0x8c) are
  unchanged. Two of the 15 callers are near twins whose pairing with
  0.6.5's is not settled; it does not matter, all 15 are hooked the same
  way.
- **Tooltips.**
  - The sheet getter is `0x3a39a0`: every caller of 0.6.5's getter that
    aligns calls it (24 callers, as on 0.6.5). The draft's `0x127f290` was
    wrong (one caller).
  - The game's Assets context grew: the getter reads `+0x368..+0x388`
    (0.6.5 `+0x30..+0x50`), its callee up to `+0x4b8`. It is the same
    object: the viewer's fifth argument goes to the in-game UI, which
    passes it to the getter. The mod's check that the context is readable
    used a size written in the shared code (0x70); it is now
    `TOOLTIP_ASSETS_READ_SIZE` in each version's table (0.6.4/0.6.5 0x70,
    Beta 2 0x4c0). On 0.6.5 the getter's callee already read up to
    `+0x148`, beyond that 0x70 check; unchanged.
  - The champion-info lookup `0x1a77f00` (same arguments, same pair
    return, same order: mod champions, data sheet, built-in lookup
    `0x1a64f60`) and the Arc drop `0x28b50` (identical).
  - All 65 champion-info tables carried; the description methods sit at
    `+0xe0/+0xe8/+0xf0`, confirmed by the game's own calls. Two data
    layouts grew by 0x20 bytes (mod champions 0xb38, data-defined
    champions 0x9f0); the draft had kept the old sizes.
  - No new champion-info type: the four new champions (Gladiator,
    Ferryman, Bombardier, Hypnotist) have none of their own. **Guess:**
    they use the data-defined types; otherwise their tooltips show the
    mod's own text.
  - The view-assets guard (`0x99b076`) is the same instructions in a new
    order.

## Not settled

- Bombardier: settled 2026-10-22 (seen in game, above), apart from its
  previews, which need table entries.
- Outline: the view's unit id at `+0x100` is not read by any code checked;
  only play shows the outline on the right unit. The same id keys the drawn
  bodies click selection uses: never matching means selection falls back to
  the areas used without the outline; matching another unit would use that
  unit's body.
- The research reader: not reviewed (its effect inventory, the four new
  champions' effects, the preview headers).

## Lessons for the next version

- Seeding the function map with a large hand-confirmed function (here the
  viewer dispatcher, 15k instructions) unlocked far more than the draft's
  own exact pairs: 856 -> 1870 functions.
- A drafted function can be wrong because another function now starts with
  the same bytes (cancel recall -> move). Check by the callers.
- Byte patterns the draft "read at the carried address" pass the byte check
  whatever they are; compare them with the old version's (the worklist
  script) instead of trusting a pass.
- Version data hides in shared code as constants too (the in-game UI's type
  id), not only addresses.
- The stat census and field-checked sites (1.5.0-dev23) carried all 35 stat
  sites with no hand work.
- Check a drafted function the mod calls by its callers, not by its
  bytes: the draft's sheet getter started like the right one but had one
  caller instead of 24.
- The draft carries a table's address, not the sizes stored in it: read
  them again (two champion-info sizes grew).
- A structure the game passes to the mod can grow; a size check written in
  the shared code then becomes version data (the Assets context).
- The analysis scripts of this hand check are kept with it
  (`research/game-builds/draft-0.7.0b2-from-0.6.5/scripts/`, not in git).
