//! Game 0.6.5: the skill effects the preview reader recognises, by the
//! address of their `apply` method (vtable +0x20). Carried from 0.6.3 with
//! tools/match_vtables.py; RANGE_PERIOD and RANGE_PROJECTILE match their
//! 0.7.0 forms (0.99), and SWORDMAN_ULT and the 8-byte statuses were
//! recompiled (docs/migration-0.6.5.md).

pub(crate) const COMBINE: usize = 0x15e4a00;
pub(crate) const RANGE: usize = 0x15e2b80;
pub(crate) const LINEAR: usize = 0x18f3870;
pub(crate) const WHIP_LINE: usize = 0x168f590;
// 0.71, identified from the 0.70 effect-tree inventory (layouts checked
// against every logged instance; see docs/investigation-preview-selection.md).
pub(crate) const DELAYED: usize = 0x180ad70;
pub(crate) const RANGE_PERIOD: usize = 0x1955490;
pub(crate) const RUSH: usize = 0x18f55e0;
pub(crate) const MOVE: usize = 0x1956390;
// 0.73, named by pairing logged trees with Workshop and data-driven champion
// JSON (field values checked against those files).
pub(crate) const RANGE_PROJECTILE: usize = 0x1557900;
pub(crate) const PARABOLIC: usize = 0x1953b10;
pub(crate) const RUSH_TIME: usize = 0x1539950;
pub(crate) const SWITCH_BY_BUFF: usize = 0x168f460;
pub(crate) const MOVE_TO_TARGET: usize = 0x19865a0;
// 0.75.2, hand-written base-game skills: fields matched word for word to
// the champion's `champion_info` entry (ranges after patch adjustment).
pub(crate) const ICE_MAGE_ULT: usize = 0x19524b0;
pub(crate) const BARD_ULT: usize = 0x180af00;
pub(crate) const EXORCIST_ULT: usize = 0x180a210;
pub(crate) const DANCER_ULT: usize = 0x18fee20;
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
pub(crate) const RUSH_TO_TARGET: usize = 0x17735e0;
/// Swordman R: { name, applied effects, width (word 6), distance (word 7),
/// _, _, target } (1.00).
pub(crate) const SWORDMAN_ULT: usize = 0x1537f40;
/// Gambler Q: { .., range (word 5), speed, hit radius (word 7) } (1.00).
pub(crate) const GAMBLER_Q: usize = 0x1811480;
/// Recognised effects with no area of their own (buffs, single-target
/// shots, visuals, damage on units already hit): named, not reported.
pub(crate) const NO_AREA: &[(usize, usize, &str)] = &[
    // 0.6.4 holds AddBuff and Attack twice each (identical code, 1.00).
    (0x16913a0, 288, "AddBuff"),
    (0x131eb30, 288, "AddBuff"),
    (0x16a0ec0, 296, "AddCasterBuff"),
    (0x15389e0, 24, "Sfx"),
    (0x15585c0, 24, "TargetSfx"),
    (0x1822be0, 48, "ViewEffect"),
    (0x180ded0, 24, "CasterViewEffect"),
    (0x1690260, 32, "CasterAnimation"),
    (0x180e4e0, 72, "Attack"),
    (0x181d2a0, 72, "Attack"),
    (0x1819550, 64, "ApAttack"),
    (0x13cf210, 64, "Heal"),
    (0x1957bf0, 16, "Knockback"),
    (0x1951fc0, 8, "BlockAttack"),
    (0x196c780, 8, "BlockMoveSkill"),
    (0x16a4600, 72, "TargetProjectile"),
    // One unit, no area: Vampire Q (0.54 with the 0.6.3 and 0.7.0 method,
    // far ahead of any other 32-byte effect at 0.25).
    (0x1963dd0, 32, "Vampire Q"),
    // Fear and the timed statuses: three 8-byte statuses recompiled in 0.6.4
    // (the crowd-control fixes), each about 0.75 like the 0.6.3 ones, so
    // which is which is not settled. An 8-byte effect holds no area.
    (0x1691440, 8, "status (Fear or timed)"),
    (0x1690b50, 8, "status (Fear or timed)"),
    (0x16903b0, 8, "status (Fear or timed)"),
    (0x13a55a0, 48, "Bleed"),
    (0x16d3c60, 96, "Gunner Q shot"),
    (0x16a5f70, 16, "caster wrapper"),
    (0x13105e0, 40, "Gunner R sub-shots"),
    (0x18fb9c0, 24, "Bard Q buff"),
    (0x145e5a0, 16, "Exorcist W buff"),
    // Cavalry Knight R: a speed road from the cast point to the knight,
    // shaped by where she moves afterwards (user, 0.75.2).
    (0x1698e90, 344, "Cavalry Knight R buff"),
    (0x189be20, 112, "Cavalry Knight R road"),
];
pub(crate) const RDATA_TABLE_BOUNDS: std::ops::RangeInclusive<usize> = 0x39f5000..=0x501972a;
