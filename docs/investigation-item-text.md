# Investigation: item effect text values

Why the mod's item tooltips showed "…" for some values, and how 1.5.0-dev29
asks the game for its own text instead. Everything here is **read from the
code** of games 0.6.4, 0.6.5 and 0.7.0 Beta 2 unless a line says otherwise.

## The report

**Seen in game** (the author, 1.5.0-dev28 on Beta 2, 2026-10-21): the item
tooltips of 憤怒巨靈的碎角 and 不破壁壘 show "…" where the game shows
numbers.

## Why

The mod built item text from the game's text sheet: the stat lines (each
`spec.<stat>` template with the item's stat filled in) and the effect
sentence `#asset/base/text/item?<key>.option`. That sentence holds value
slots (`{RegenFlat}`, `{Range}`, ...). Since an early build every slot was
shown as "…" (`tooltips::rich_markup`); nothing filled them.

Four base items have slots, the same on 0.6.5 and Beta 2:
`giants_horn_shard`, `impregnable_fortress`, `ring_of_reincarnation`,
`hourglass_of_eternity`. Their values sit in the item settings under other
names (`flat_regen`, `aoe_range` 30000, ...), and only the game's code knows
which value goes in which slot and how it is scaled (the range is shown
divided by 1000).

## The game's own path

The game's item panel (0.6.4 `0x2300390`, 0.6.5 `0x1f219a0`, Beta 2
`0x2d81ea0`) receives the item key and the game's Assets context, and:

1. gets the item sheet from Assets (`asset/base/setting/item_setting`; the
   getter is the twin of the champion sheet getter the skill tooltips use,
   reading the same Assets fields);
2. builds the list of item objects from the sheet (a
   `Vec<Box<dyn Item>>`; the builder takes only the output and the sheet);
3. finds the item by its key (method table `+0x58`);
4. asks its effect text (`+0x90`, with the output, the item and Assets);
5. drops the list with the game's general routine for such lists and frees
   its buffer from the process heap when its capacity is not 0.

The method table holds the slots the mod's item reader already uses
(`+0x50` enabled to `+0x80` next tiers) and `+0x90` after them. Reviewed
item tables: 20 on 0.6.4, 17 on 0.6.5, 20 on Beta 2 (Workshop items
included); one key method per version. Their key and text methods are
guarded in each version's table.

## The one way it could stop the game

The item-specific text methods (the horn shard's, for example) stop the
game if their text lookup finds nothing. That lookup (Beta 2 `0x1dd8b0`)
fails only when the key does not start with `#`, does not split as
`path?key`, or the text sheet `asset/base/text/item` is missing; it does
not check the individual text. The SDK's own text lookup, which the mod
uses, goes through `0x1de250`, which makes the same three checks and more.
So the mod asks the game only after the SDK found the exact text
`#asset/base/text/item?<key>.option`; then the game's lookup cannot fail.
The generic and Workshop text methods return empty text instead.

Checked on 0.6.4, 0.6.5 and Beta 2 for every text method of every
reviewed item table (15, 14 and 15 distinct methods): each uses only the
SDK's lookup or this one (same key parser and sheet lookup on each
version), and every key it builds is in the same item text sheet (some
are fixed keys, not the item's own). Not covered: an abort of a method's
own that has nothing to do with text lookup; the game's item panel would
hit it with the same arguments.

## What the mod does (1.5.0-dev29)

- An item whose effect sentence has a value slot is queued
  (`native_tooltips::item_text`), only after the SDK check above. Workshop
  items with their own `description` keep it.
- In the viewer callback, where the game's own item panel and the skill
  tooltips run, every queued item is answered from one list built the
  game's way: only items of a reviewed table are asked; the list goes back
  to the game's drop only when its shape is believable (otherwise it is
  left allocated); no game pointer is kept.
- The game's text is used when it has words and no value slot left;
  otherwise the template is shown as before. Results are cached per item,
  language and match. Log: `NATIVE ITEM_TEXT key=… source=game` or
  `fallback reason=…`; the text itself at Verbose.

## 1.5.0-dev30

- **Seen in game (1.5.0-dev29 on Beta 2, the author, 2026-10-22):** the
  item tooltips still showed "…". Read from its log: the game's text came
  back complete for all four items (for example 不破壁壘 "…造成 10 + 2%
  物理防禦的魔法傷害"); the HUD tooltip and the shop kept the text they built
  on the first hover, before the game's answer arrived. Now a game item
  text arriving rebuilds them (`native_tooltips::item_revision`).
- The first item that needs the game's text queues every item with value
  slots, so the game's item list is built once per match however many an
  item mod adds (the author: item mods can have 20+ such items). Answers
  are still kept per match, not per game session: a balance mod can patch
  items between matches (the author).

## Not settled

- Nothing of this is seen in game yet (1.5.0-dev29).
- Workshop items through the Workshop text method: read from the code, it
  looks up `.option` and returns it; whether a Workshop item's slots are
  filled there is not known.
