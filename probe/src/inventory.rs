//! Presentation capacity follows the buyer's full native target vector, not
//! catalogue size, the current number of purchases, or a recognized mod ID.
pub const READ_LIMIT: usize = 64;
pub const HUD_COLUMNS: usize = 6;
pub const TAB_COLUMNS: usize = 10;
pub const PURCHASE: usize = usize::MAX;

pub fn slots(plan: Option<usize>, owned: usize, retained: Option<usize>) -> usize {
    // The game's own purchase gate is the real slot limit (four in vanilla,
    // six with the Riot pack); the build plan can be shorter (a 4-item plan
    // in a 6-slot game). The plan length is only the fallback.
    let plan = crate::native_adapter::item_slot_capacity().or(plan);
    // A missing/empty plan is unknown. Retain same-player knowledge; owned
    // items are always a lower bound and never discarded due to plan mismatch.
    plan.filter(|n| *n > 0 && *n <= READ_LIMIT)
        .or(retained)
        .unwrap_or(owned)
        .max(owned)
        .min(READ_LIMIT)
}
pub fn hud_slot(count: usize, index: usize) -> (usize, isize) {
    let columns = count.clamp(1, HUD_COLUMNS);
    let rows = count.div_ceil(HUD_COLUMNS);
    let x = 1390 - columns * 40 + (index % HUD_COLUMNS) * 40;
    let y = 100 - ((rows.saturating_sub(1) - index / HUD_COLUMNS) * 40) as isize;
    (x, y)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_plans_keep_empty_slots_stable_while_purchases_grow() {
        for n in [4, 5, 6] {
            for owned in 0..=n {
                assert_eq!(slots(Some(n), owned, None), n);
                assert_eq!(hud_slot(n, n - 1), (1350, 100));
                assert_eq!(hud_slot(n, 0).0, 1390 - n * 40);
            }
        }
    }
    #[test]
    fn unknown_or_inconsistent_plans_never_hide_observed_items() {
        assert_eq!(slots(None, 3, Some(5)), 5);
        assert_eq!(slots(Some(0), 3, None), 3);
        assert_eq!(slots(Some(4), 6, None), 6);
        assert_eq!(slots(None, 0, None), 0);
        assert_eq!(slots(Some(65), 8, None), 8);
        assert_eq!(hud_slot(8, 6), (1150, 100));
        assert_eq!(hud_slot(8, 0), (1150, 60));
    }
}
