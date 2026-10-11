//! Game 0.6.3: the skill effects the preview reader recognises, by the
//! address of their `apply` method (vtable +0x20).

pub(crate) const COMBINE: usize = 0x1adaf00;
pub(crate) const RANGE: usize = 0x16974f0;
pub(crate) const LINEAR: usize = 0x18bf7a0;
pub(crate) const WHIP_LINE: usize = 0x1611090;
// 0.71, identified from the 0.70 effect-tree inventory (layouts checked
// against every logged instance; see docs/investigation-preview-selection.md).
pub(crate) const DELAYED: usize = 0x1adb160;
pub(crate) const RANGE_PERIOD: usize = 0x14294c0;
pub(crate) const RUSH: usize = 0x1698550;
pub(crate) const MOVE: usize = 0x1adb2f0;
// 0.73, named by pairing logged trees with Workshop and data-driven champion
// JSON (field values checked against those files).
pub(crate) const RANGE_PROJECTILE: usize = 0x1489140;
pub(crate) const PARABOLIC: usize = 0x1612510;
pub(crate) const RUSH_TIME: usize = 0x15b28f0;
pub(crate) const SWITCH_BY_BUFF: usize = 0x18bf2d0;
pub(crate) const MOVE_TO_TARGET: usize = 0x1af4470;
// 0.75.2, hand-written base-game skills: fields matched word for word to
// the champion's `champion_info` entry (ranges after patch adjustment).
pub(crate) const ICE_MAGE_ULT: usize = 0x160fd80;
pub(crate) const BARD_ULT: usize = 0x18c1cc0;
pub(crate) const EXORCIST_ULT: usize = 0x18bec80;
pub(crate) const DANCER_ULT: usize = 0x1aebb90;
/// The effect of Gladiator, Ferryman, Bombardier and Hypnotist (game 0.7.0)
/// does not exist in this game: no apply address is 0, so it never matches.
pub(crate) const CHAMPION_SKILL: usize = 0;
// The three below, and Vampire Q and the timed status in NO_AREA, were
// identified on 0.7.0 (1.1.20) and carried back with tools/match_vtables.py
// (0.7.0 -> 0.6.3; similarity of the method at the same vtable slot).
/// { applied effects, speed }: the caster rushes to the clicked unit, then
/// the effects apply (Lancer W, Tale W). Similarity 0.54 with the 0.7.0
/// method, far ahead of any other 32-byte effect (0.14) and held by two
/// vtables in both games; only its presence is used, no field is read.
pub(crate) const RUSH_TO_TARGET: usize = 0x1ad97b0;
/// Swordman R: { name, applied effects, width (word 6), distance (word 7),
/// _, _, target } (1.00).
pub(crate) const SWORDMAN_ULT: usize = 0x18c09e0;
/// Gambler Q: { .., range (word 5), speed, hit radius (word 7) } (1.00).
pub(crate) const GAMBLER_Q: usize = 0x18c98a0;
/// Recognised effects with no area of their own (buffs, single-target
/// shots, visuals, damage on units already hit): named, not reported.
pub(crate) const NO_AREA: &[(usize, usize, &str)] = &[
    (0x1279800, 288, "AddBuff"),
    (0x1281860, 296, "AddCasterBuff"),
    (0x18c1480, 24, "Sfx"),
    (0x18e40b0, 24, "TargetSfx"),
    (0x18e2630, 48, "ViewEffect"),
    (0x18c6c20, 24, "CasterViewEffect"),
    (0x19de6f0, 32, "CasterAnimation"),
    (0x18c76c0, 72, "Attack"),
    (0x18d36b0, 64, "ApAttack"),
    (0x1990300, 64, "Heal"),
    (0x1698810, 16, "Knockback"),
    (0x19df350, 8, "Fear"),
    (0x14dfcf0, 8, "BlockAttack"),
    (0x14f8d10, 8, "BlockMoveSkill"),
    (0x14e0490, 72, "TargetProjectile"),
    // One unit, no area: Vampire Q (1.00) and the timed status in Tale R
    // and others (0.98). The second timed status of 0.7.0 has two equally
    // similar candidates here (0.90) and is left out.
    (0x14edc50, 32, "Vampire Q"),
    (0x1a1e470, 8, "timed status"),
    (0x1ad9600, 48, "Bleed"),
    (0x1a1c090, 96, "Gunner Q shot"),
    (0x1285690, 16, "caster wrapper"),
    (0x1278940, 40, "Gunner R sub-shots"),
    (0x18d0a10, 24, "Bard Q buff"),
    (0x1426d40, 16, "Exorcist W buff"),
    // Cavalry Knight R: a speed road from the cast point to the knight,
    // shaped by where she moves afterwards (user, 0.75.2).
    (0x127d060, 344, "Cavalry Knight R buff"),
    (0x19de840, 112, "Cavalry Knight R road"),
];
pub(crate) const RDATA_TABLE_BOUNDS: std::ops::RangeInclusive<usize> = 0x39ca000..=0x4fe8652;
