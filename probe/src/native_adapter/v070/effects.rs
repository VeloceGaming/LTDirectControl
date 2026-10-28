//! Game 0.7.0: the skill effects the preview reader recognises.
//! Not reviewed for 0.7.0: the reader family (research builds only) is off
//! on this executable, so these are never read. Each holds a distinct value
//! no apply address can have (`usize::MAX - n`), never another version's
//! address.

pub(crate) const COMBINE: usize = usize::MAX - 1;
pub(crate) const RANGE: usize = usize::MAX - 2;
pub(crate) const LINEAR: usize = usize::MAX - 3;
pub(crate) const WHIP_LINE: usize = usize::MAX - 4;
pub(crate) const DELAYED: usize = usize::MAX - 5;
pub(crate) const RANGE_PERIOD: usize = usize::MAX - 6;
pub(crate) const RUSH: usize = usize::MAX - 7;
pub(crate) const MOVE: usize = usize::MAX - 8;
pub(crate) const RANGE_PROJECTILE: usize = usize::MAX - 9;
pub(crate) const PARABOLIC: usize = usize::MAX - 10;
pub(crate) const RUSH_TIME: usize = usize::MAX - 11;
pub(crate) const SWITCH_BY_BUFF: usize = usize::MAX - 12;
pub(crate) const MOVE_TO_TARGET: usize = usize::MAX - 13;
pub(crate) const ICE_MAGE_ULT: usize = usize::MAX - 14;
pub(crate) const BARD_ULT: usize = usize::MAX - 15;
pub(crate) const EXORCIST_ULT: usize = usize::MAX - 16;
pub(crate) const DANCER_ULT: usize = usize::MAX - 17;
pub(crate) const CHAMPION_SKILL: usize = usize::MAX - 18;
pub(crate) const RUSH_TO_TARGET: usize = usize::MAX - 19;
pub(crate) const SWORDMAN_ULT: usize = usize::MAX - 20;
pub(crate) const GAMBLER_Q: usize = usize::MAX - 21;
/// Recognised effects with no area of their own: none on Beta 2 yet.
pub(crate) const NO_AREA: &[(usize, usize, &str)] = &[];
pub(crate) const RDATA_TABLE_BOUNDS: std::ops::RangeInclusive<usize> = 0..=0;
