# Game 0.7.0 (the release)

The Steam update of 2026-10-28 replaced the live game with 0.7.0. Its
executable (`d678e012…7d30`, timestamp `0x6ae1b2c7`) is not Beta 2's. The
author's decisions the same day: support 0.6.5 and 0.7.0, and drop 0.6.4 and
0.7.0 Beta 2. First build: 1.7.0-dev1.

Labels: **read from the code** (static analysis of the executable),
**seen in game** (with build and game version), **guess**.

The research copy of the release is `research/game-builds/0.7.0`. The
copy that held that name before, the first 0.7.0 beta of the 1.2.x builds,
is now `research/game-builds/0.7.0b1`.

## 1. What the update is

- **Read from the code:** `mod-sdk-stable/` is identical to Beta 2's, so the
  mod's interface and its ABI are unchanged.
- **The draft** (`tools/draft_version.py --old 0.7.0b2 --new 0.7.0r`) first
  mapped 844 functions and left 109 layout and 27 identity entries unknown,
  among them the whole worker send. Reseeded with every whole function
  placed exactly plus the worker send found by hand, the map grew to 1304
  functions. What was still left was found by hand (below).

## 2. Found by hand (read from the code)

- **Worker send:** the caller is `0xa00e70` (Beta 2 `0x9aef00`, the same
  size; its twin `0xa01ba0`). The send is at `0xa0138b` and the drain-loop
  send at `0xa015e3`, both to `0xc68680`, whose 73 instructions match Beta
  2's `0xc9dd90`. The screen delay is at `0xa00f7d` with the same bytes. The
  caller's frame grew; the hook reads nothing from it.
- **Viewer dispatch:** `0xc24090`, the third of six viewer calls in the
  dispatcher `0xc1dfd0`, as on Beta 2. The viewer `0x9eaf50` has the same
  entry bytes.
- **Input:** `0xc4d04d` sets up the same seven arguments for `0xb9be00`.
- **The viewer's playback config grew by 8 bytes.** The new viewer starts by
  reading a byte at `+0x18` and a float at `+0x1c`: when the byte is set it
  clears it and copies the float, clamped, to view `+0x110`. Every field from
  the old `+0x18` on is 8 bytes further:

  | Field | 0.6.5 and Beta 2 | 0.7.0 | Where it was confirmed |
  | --- | --- | --- | --- |
  | Camera target (mode, lane or x, side or y) | `+0x18` | `+0x20` | viewer: `[rdi+0x28]` became `[rdi+0x30]` |
  | Full width | `+0x45` | `+0x4d` | renderer `0x2f292da`, UI link `0x8d6b12` (`+0x5d` became `+0x65`), full-width key `0xb9d40d` |
  | Compact side | `+0x46` | `+0x4e` | renderer `0x2f2930c` |
  | Paused | `+0x48` | `+0x50` | viewer `0x9eb0b4` |
  | Vision | `+0x4b` | `+0x53` | renderer `0x2f27091`, `0x2f26adf`; UI `0x2b9aa40` (`+0x63` became `+0x6b`) |

  The shared hook code wrote these at fixed offsets. They are now
  `layout::CONFIG_*` in each version's table, with 0.6.5's values unchanged.
  Mode, speed and sync (`+0x10`, `+0x14`, `+0`) did not move. The UI still
  holds the config 0x18 bytes in (`ingame + 0x9150`).
- **The simulation's event list:** an entry grew from 0x70 to 0x90 bytes
  (the stop event `0x1630c60`: `lea rcx, [rbx+rbx*8]; shl rcx, 4`; tag 0x12
  unchanged). The cancel-recall event is still built inline (move
  `0x167c0e7`: a 0x18-byte payload with `0x1c` and the caster, then an
  entry with tag 0), so the mod's own one uses the 0x90 stride
  (`CANCEL_RECALL_INLINE`).
- **Hooked functions:** attack `0x166a330`, skills Q `0x167cab0`, W
  `0x166a9c0`, R `0x167b400` were recompiled (smaller, with fewer calls
  through the skill action's table). The champion fields each one reads are
  the same set as on Beta 2. Aim `0x1689460`, move `0x167bf90`, steer
  `0x167ad60`, direct step `0x166bbb0` and the stop event read the same
  fields. Every hook site calls its table's function, and the arguments are
  set up the same way apart from the callers' stack slots. The attack
  function's entry is shorter (`sub rsp, 0x78`): `ATTACK_PROLOGUE` is 16
  bytes on 0.7.0.
- **Layout guards:** all 38 found. 30 differ from Beta 2 only in jump
  targets, call targets and stack slots. The 8 that read the config
  differ by the 8-byte move above.
- **Hammerer W's skill methods:** the table at `0x41022e8` (name
  `0x1a80970`, get by name `0x1a80790`, cooldown `0x169bb50`), found by the
  method returning `hammerer_skill2`.
- **Shop:** the buy-new and upgrade stubs `0xf40a50` and `0xf3f580` (jumps
  to the decision functions `0xf11360` and `0xf111f0`), in the controller
  table's slots `0x401cad8` and `0x401cae0`.
- **Item reader:** the AI bridge table `0x4275420` (its eight methods line up
  with mapped functions), the player and context getters `0x346bd90` and
  `0x346caf0`, the item getter `0x1b63310`, which 17 item tables share.
- **Stat sites:** the census finds as many sites of each kind as on Beta 2
  (10 hit tests, 11 floored hit tests, 3 Accuracy producers, 1 Dodging
  producer, 5 reaction gates that roll a band). The verifier checks every
  site's registers and frame slots against the code. One drafted site kept
  Beta 2's frame slot in its bytes (`0x2c8`, now `0x2d0`).

## 3. Minimap, outline, tooltips (1.7.0-dev2, read from the code)

Off in 1.7.0-dev1, reviewed 2026-10-29. Superseded the same day: the
1.7.0-dev1 note "not reviewed yet" (about 50 minimap operands only
"likely", 43 tooltip anchors and about 70 pointers not carried).

- **Minimap.** The renderer is `0x2f26790` and the input handler
  `0xb9be00` (the input hook's function). Aligned with Beta 2's
  (`0x2e8a550`, `0xbd4ed0`) with frame slots and registers ignored, every
  one of the 54 operands is the same instruction, unmasked, and every
  operand that reads game data reads the same bytes. The draft had placed
  52; the other two (`fog_cell_pitch` at `0x2f2a18c`, one
  `input_narrow_right_x` at `0xb9c999`) were found by the same
  alignment. The renderer uses every data constant as often as Beta 2's.
  The input handler uses 0.5 and 3.0 once more each: the compiler copied
  the zoom limit (view `+0x110`, kept between 0.5 and 3.0), which is not
  a mod operand.
- **Outline.**
  - The unit renderer `0x2466400`: 2554 of 2558 instructions equal Beta
    2's; the rest is the vision flag (config `+0x53`, the move in section
    2) and registers. Its 39 uses of 0xd0 (the command size) and the
    view fields the outline reads (`+0x17c`) are as on Beta 2.
  - Found by their calls in the renderer: the shader setter `0x1cb0f0`
    (895 of 915 instructions equal, the rest registers and frame slots),
    the float and colour setters `0x2ce58e0` and `0x2ce4e00` (408/408 and
    410/410). The line marker `0x2468e3c` is the renderer's only
    `0x8000000000000008`.
  - The command drop `0x1c6d10`: 179/179 instructions equal; its jump
    table moved to `0x3f43114` and sends all 13 tags to the same code.
    The draft kept Beta 2's table bytes. The drop caller `0x1cc316` is in
    the shader setter, as on Beta 2. The nine-patch marker `0x1c99cd`.
  - **16 renderer calls (Beta 2: 15).** Beta 2's draw method `0x2052230`
    (in 11 unit types' method tables) is now `0x2096e80` (11 types) and
    an identical copy `0x2cc0050` (one type). That type's update method is
    Beta 2's instruction for instruction, and on Beta 2 its draw was
    `0x2c17e50`, a call the mod already hooked. **Guess:** it is
    Jiangshi's view (a helper two calls down from its methods loads
    Jiangshi's sprites; that helper may be shared). Its update reads a
    string that sits next to
    `game-view\src\pad_context.rs`; that is a neighbouring string, not the
    console layer. Every one of the 16 calls sets up its arguments as on
    Beta 2: the 24 instructions before each are equal, apart from three
    that read the vision flag at `+0x53`.
  - No tool checks the outline constants against the executable (the
    profile holds only `SHADER_PROLOGUE`); a one-off script checked each
    one for 1.7.0-dev2.
- **Tooltips.**
  - Champion text: the sheet getter `0x3a9b50` (24 callers, as on Beta 2),
    the info lookup `0x19d1260`, the Arc drop `0x28b50`, the built-in
    lookup `0x19be2c0` (1809/1809). The game's ability panel `0x23a36e0`
    makes the same calls as Beta 2's, including the three description
    calls at `+0xe0/+0xe8/+0xf0`; it only half aligns (4269 of 7752).
  - All 65 champion tables carried. The lookup's branches look alike, so
    each built-in pair was also checked by the table's own type and source
    file names (64 equal). The Workshop champions' table (`0x40ee048`)
    has no name; its drop, Q, W and R methods equal Beta 2's and no other
    table holds them. No table size changed.
  - The data-defined champions' resolver `0x174f370`: its frame grew by
    0x10; with frame slots and registers ignored, 3097 of 3101
    instructions equal Beta 2's; the rest stores a value and passes its
    address. `field_shifts.py`: 13 field uses, none moved.
  - Assets read size: the sheet getters and their callee are Beta 2's
    instruction for instruction (`+0x368..+0x388`, callee up to
    `+0x4b8`), so `TOOLTIP_ASSETS_READ_SIZE` stays 0x4c0, now measured.
  - Item effect text: 17 item tables (Beta 2: 20), the only tables
    holding the item marker method at `+0x60` (`0x1b632d0`; on Beta 2 its
    counterpart was held by exactly the 20). The key method at `+0x58` is
    a tiny function the compiler shares with unrelated tables, so it does
    not identify item tables. Each of the 14 distinct text methods is a
    Beta 2 text method with the same code, the same strings, the same
    calls (mapped) and the same system calls (`GetProcessHeap`,
    `HeapFree`). The item text sheet `asset/base/text/item` is Beta 2's
    byte for byte, so Beta 2's check that the game's text lookup cannot
    fail carries over.
  - The game's item panel is `0x22e1ef0`, the only function that calls the
    item sheet getter, the list builder and `+0x90`. It is the PC item
    info screen (`game-view\src\ui\item_info_ui`); its caller aligns with
    Beta 2's (0.92). It only aligns a quarter with Beta 2's panel, but its
    calls to `+0x58` and `+0x90` set up output, item and Assets in the
    same registers; list entries are still 16 bytes (object, table). The
    list builder `0x1c704a0` (3544/3573, registers and error paths) and its
    drop `0x17e930` (59/59).
- **The research reader:** not reviewed, off, as on Beta 2.

## 4. To play

- **1.7.0-dev1.** Session, camera, movement, combat, shop and athlete
  stat rules. **Seen in game by the author on 0.7.0, 2026-10-29: "All
  tests passed"** (the dev1 test checklist). 0.6.5 with that DLL was not
  reported.
- **1.7.0-dev2.** Adds the minimap, the hover outline and the native
  tooltips on 0.7.0. **Seen in game by the author, 2026-10-29: "All
  tests passed on both versions"** (0.6.5 and 0.7.0). Superseded the same
  day: "Not seen in game yet". Released in 1.7.0.
