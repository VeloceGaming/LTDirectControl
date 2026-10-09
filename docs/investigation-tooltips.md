# Investigation: skill tooltip numbers showing "…"

Date: 2026-10-09. Read-only: game data, executable strings and our code
(`probe/src/tooltips.rs`, `probe/src/tooltip_assets.json`). Backlog item 1.

## How it works today

A skill description is the game's localized template with placeholders, for
example the Illusionist's Q: `{Damage} + {Coef}% … taunt for {Time} seconds`.
`tooltips::parameter` maps each placeholder name to a hand-written list of data
fields (`Time` → `bind_duration`, `stun_duration`, …) and looks them up in the
champion's skill data (`tooltip_assets.json` for base champions, the mod's own
data for Workshop champions). Anything unmapped, missing or ambiguous prints
"…" on purpose, so no number is ever guessed.

## Measurement (English templates, base game)

- 120 of 190 skills show at least one "…".
- Placeholders: 429 filled; 149 mapped but the field was not found; 106 have
  no mapping at all; 1 ambiguous.
- Over 70 distinct placeholder names occur; most occur once or twice
  (`CloudRadius`, `DotTick`, `TimeCoef`, `StunTime`, `SpeedCoef`, …).

## Causes

1. **Missing data.** `tooltip_assets.json` has no numbers at all for 37 skills
   of 20 champions (alchemist, astrologer, crossbowman, dancer, dokkaebi,
   gambler, gunner, harpooner, jiangshi, lightning_mage, monk, nightmare,
   ogre, plague_doctor, poison_dart_hunter, pole_warrior, sand_mage, soldier,
   spellbreaker, strongman). These champions are defined by the game's
   generic effect system; their values sit in nested effect trees in the
   bundle's `data_champion` files (e.g. Alchemist Q: poison 8 + 10% every 30
   ticks for 180 ticks, cloud radius 32000, heal reduction 50% for 60 ticks),
   which our extraction did not include.
2. **Field names differ per champion.** The Illusionist's taunt is
   `taunt_duration` and its illusion `illusion_duration`; neither is in the
   `{Time}` list.
3. **Damage-type guess.** `{Coef}` after a magic-damage marker only looks for
   magic-ratio fields, but the Illusionist's data calls it `attack_ratio`.
4. **Unmapped placeholder names** (106 uses across about 60 names).

## How the game does it

The executable contains a generic list of description parameters
(`…Damage BonusDamage Coef MarkCoef UseCount HealCoef Speed SpeedCoef
HealReduce … Range Radius CloudRadius ProjectileRadius … Tick DotTime DotTick
CloudTime …`) and per-champion modules (`game-core/src/setting/champion/
taoist.rs` with `TimeCoef`, `circus_blade.rs` with `DamageCoef Slow SlowTime
ChargeCount`, `android.rs` with `StunTime`, …). Built-in champions fill their
placeholders in code; data-driven and Workshop champions (whose texts also use
`{Damage}`, `{Coef}`, …) must go through a generic resolver over effect trees.
The SDK exposes no description formatter.

## Options

1. **Use the game's own formatter** (native): find and call the function the
   game's champion-info screen uses. This should match the game's own text
   for built-in and data-defined champions, and follow supported mod
   overrides. It cannot guarantee correct text from every Workshop author.
   The static reverse-engineering findings and proposed guarded prototype
   are in [investigation-tooltips-native.md](investigation-tooltips-native.md).
2. **Improve our resolver, measured**: add the effect-tree data for the 20
   data-driven champions; map the game's own parameter names (recovered from
   the executable) to fields and effect types (`DotTime` → the poison's
   duration, `CloudRadius` → the cloud's radius, `Tick` → `tick`, …); use the
   only ratio present when the damage-type marker disagrees. Prototype the
   rules in the audit script first, measure the "…" count, then port them.
   Unknown values stay "…".
