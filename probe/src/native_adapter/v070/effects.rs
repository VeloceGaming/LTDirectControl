//! Game 0.7.0: the skill effects the preview reader recognises, by the
//! address of their `apply` method (vtable +0x20). 36 of the 40 known
//! applies equal their 0.6.3 forms; RANGE_PERIOD, RANGE_PROJECTILE,
//! ICE_MAGE_ULT and TargetProjectile were rewritten by the game
//! (docs/migration-0.7.md).

pub(crate) const COMBINE: usize = 0x16169d0;
pub(crate) const RANGE: usize = 0x156e070;
pub(crate) const LINEAR: usize = 0x1a86e70;
pub(crate) const WHIP_LINE: usize = 0x12cd950;
// 0.71, identified from the 0.70 effect-tree inventory (layouts checked
// against every logged instance; see docs/investigation-preview-selection.md).
pub(crate) const DELAYED: usize = 0x1571150;
pub(crate) const RANGE_PERIOD: usize = 0x1bde9d0;
pub(crate) const RUSH: usize = 0x1570430;
pub(crate) const MOVE: usize = 0x1616b50;
// 0.73, named by pairing logged trees with Workshop and data-driven champion
// JSON (field values checked against those files).
pub(crate) const RANGE_PROJECTILE: usize = 0x1aa08c0;
pub(crate) const PARABOLIC: usize = 0x1a88650;
pub(crate) const RUSH_TIME: usize = 0x1571930;
pub(crate) const SWITCH_BY_BUFF: usize = 0x156f3b0;
pub(crate) const MOVE_TO_TARGET: usize = 0x164fe90;
// 0.75.2, hand-written base-game skills: fields matched word for word to
// the champion's `champion_info` entry (ranges after patch adjustment).
pub(crate) const ICE_MAGE_ULT: usize = 0x1709a30;
pub(crate) const BARD_ULT: usize = 0x1bdf040;
pub(crate) const EXORCIST_ULT: usize = 0x1a865c0;
pub(crate) const DANCER_ULT: usize = 0x1a974f0;
/// 0.7.0: every skill of Gladiator, Ferryman, Bombardier and Hypnotist is
/// one hand-written effect of 20 words; the byte at +0x98 selects the skill
/// (0-2 Gladiator, 4/6/7 Ferryman, 8-10 Bombardier, 12/14/15 Hypnotist; Q, W,
/// R in that order). Word 0 is the range, 1 a projectile's hit radius, 4 an
/// area radius, 6 a strip's half width, 12 Bombardier Q's range bonus.
pub(crate) const CHAMPION_SKILL: usize = 0x160da70;
/// { applied effects, speed }: the caster rushes to the clicked unit, then
/// the effects apply (Lancer W, Tale W; player's description).
pub(crate) const RUSH_TO_TARGET: usize = 0x16157c0;
/// Swordman R: { name, applied effects, width (word 6), distance (word 7),
/// _, _, target }, a dash of a fixed distance through enemies. 40 wide and
/// 150 long: the proportions of the game's own area (user screenshots,
/// 1.1.20, where word 6 drawn as the length was far too short).
pub(crate) const SWORDMAN_ULT: usize = 0x1a88ef0;
/// Gambler Q: { .., range (word 5), speed, hit radius (word 7) }: shots
/// toward the hovered champion that stop at the range.
pub(crate) const GAMBLER_Q: usize = 0x189ab00;
/// Recognised effects with no area of their own (buffs, single-target
/// shots, visuals, damage on units already hit): named, not reported.
pub(crate) const NO_AREA: &[(usize, usize, &str)] = &[
    (0x17c7230, 288, "AddBuff"),
    (0x17d58f0, 296, "AddCasterBuff"),
    (0x1591510, 24, "Sfx"),
    (0x1aa1750, 24, "TargetSfx"),
    (0x1a30a00, 48, "ViewEffect"),
    (0x1a20a40, 24, "CasterViewEffect"),
    (0x17c62f0, 32, "CasterAnimation"),
    (0x161f660, 72, "Attack"),
    // 0.7.0: one vtable carries a second, equal copy of the same method.
    (0x163cdc0, 72, "Attack"),
    (0x16319e0, 64, "ApAttack"),
    (0x164ef10, 64, "Heal"),
    (0x1571760, 16, "Knockback"),
    (0x1615de0, 8, "Fear"),
    (0x1615550, 8, "BlockAttack"),
    (0x162fef0, 8, "BlockMoveSkill"),
    // A shot that follows one unit. 0.7.0 rewrote the 72-byte form the
    // champions use (five vtables hold 0x17c6440; the 0.6.3 code survives
    // only in two 80-byte forms with one more word), so matching by code
    // alone missed it: Soldier Q and W, Construct R.
    (0x17c6440, 72, "TargetProjectile"),
    (0x1804b60, 72, "TargetProjectile"),
    (0x17d70f0, 80, "TargetProjectile"),
    (0x17cf560, 80, "TargetProjectile"),
    // One unit, no area (player's description, 1.1.19): Vampire Q; the
    // timed statuses in Tale R and others ({ ticks }).
    (0x157d6a0, 32, "Vampire Q"),
    (0x164e960, 8, "timed status"),
    (0x1a89990, 8, "timed status"),
    (0x156f4e0, 48, "Bleed"),
    (0x159de10, 96, "Gunner Q shot"),
    (0x17d7e00, 16, "caster wrapper"),
    (0x12cc850, 40, "Gunner R sub-shots"),
    (0x1a93b40, 24, "Bard Q buff"),
    (0x14f8200, 16, "Exorcist W buff"),
    // Cavalry Knight R: a speed road from the cast point to the knight,
    // shaped by where she moves afterwards (user, 0.75.2).
    (0x17cfd30, 344, "Cavalry Knight R buff"),
    (0x1a1f090, 112, "Cavalry Knight R road"),
];
pub(crate) const RDATA_TABLE_BOUNDS: std::ops::RangeInclusive<usize> = 0x3d1c000..=0x53d0442;
