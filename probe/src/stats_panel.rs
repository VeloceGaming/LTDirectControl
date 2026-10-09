//! Stats panels (data side): your champion and the selected unit, read on
//! the viewed match's worker during the HUD sample, shared with the client.
//! Selection is a left-click on a unit (crate::movement); empty ground
//! clears it. Display: crate::stats_ui.
use crate::Logger;
use mod_api_stable::{StableEntity, StableSim};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};

/// Values shown, in screen order (3 columns x 4 rows).
pub const STATS: usize = 12;
pub const ICONS: [crate::stat_icons::StatIcon; STATS] = {
    use crate::stat_icons::*;
    [
        AD,
        AP,
        ATTACK_SPEED,
        ARMOR,
        MAGIC_RESIST,
        HASTE,
        CRIT,
        MOVE_SPEED,
        RANGE,
        LIFESTEAL,
        ARMOR_PEN,
        MAGIC_PEN,
    ]
};
/// How each value is written: whole number, two decimals, or a percentage.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Format {
    Whole,
    Decimal,
    Percent,
}
pub const FORMATS: [Format; STATS] = {
    use Format::*;
    [
        Whole, Whole, Decimal, Whole, Whole, Whole, Percent, Whole, Whole, Percent, Percent,
        Percent,
    ]
};
pub fn format(value: f64, format: Format) -> String {
    match format {
        Format::Whole => format!("{}", value.round() as i64),
        Format::Decimal => format!("{value:.2}"),
        Format::Percent => format!("{}%", value.round() as i64),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Champion,
    Minion,
    Tower,
    Monster,
}
#[derive(Clone, Debug)]
pub struct Unit {
    /// Champion key for the face portrait.
    pub champion: Option<String>,
    /// Picks the glyph shown when there is no face.
    pub kind: Kind,
    pub friendly: bool,
    pub level: usize,
    pub hp: (usize, usize),
    pub shield: usize,
    pub values: [f64; STATS],
    at: Instant,
}

#[derive(Default)]
struct Shared {
    own: Option<Unit>,
    target: Option<Unit>,
    logged: Option<Instant>,
}
static SHARED: Mutex<Shared> = Mutex::new(Shared {
    own: None,
    target: None,
    logged: None,
});
static TARGET: AtomicUsize = AtomicUsize::new(usize::MAX);

/// The selected unit (None clears).
pub fn select(id: Option<usize>) {
    TARGET.store(id.unwrap_or(usize::MAX), Ordering::Relaxed);
    if id.is_none() {
        if let Ok(mut s) = SHARED.lock() {
            s.target = None;
        }
    }
}
fn target_id() -> Option<usize> {
    Some(TARGET.load(Ordering::Relaxed)).filter(|id| *id != usize::MAX)
}
/// Your champion and the selected unit, when sampled within the last second.
pub fn snapshot() -> (Option<Unit>, Option<Unit>) {
    let fresh = |u: &Option<Unit>| {
        u.clone()
            .filter(|u| u.at.elapsed() < Duration::from_secs(1))
    };
    SHARED
        .lock()
        .map(|s| (fresh(&s.own), fresh(&s.target)))
        .unwrap_or((None, None))
}

/// Worker, during the HUD sample of the controlled player.
pub fn sample(sim: &StableSim<'_>, own: Option<&StableEntity<'_, '_>>, log: &Logger) {
    let side = own.map(|c| c.team());
    let own_unit = own.filter(|c| c.is_alive()).map(|c| read(sim, c, side));
    // The selected unit, while it lives and your team can see it.
    let target = target_id().and_then(|id| {
        let e = sim.get_entity(id)?;
        let seen = side.is_none_or(|team| e.team() == team || sim.is_visible(team, id));
        (e.is_alive() && seen).then(|| read(sim, &e, side))
    });
    if target_id().is_some() && target.is_none() {
        select(None);
    }
    let Ok(mut s) = SHARED.lock() else {
        return;
    };
    // Raw values once every 10 s, to check them against the game's panel.
    if let Some(c) = own.filter(|_| {
        s.logged
            .is_none_or(|t| t.elapsed() > Duration::from_secs(10))
    }) {
        s.logged = Some(Instant::now());
        let st = c.stat();
        let b = buffs(c);
        log.write(&format!(
            "STATS own raw attack={} magic_power={} defence={} mr={} move_speed={} hp_regen={} crit={} stack={} interval={} as_mult={} buffs={} haste={} vamp={} defence_pen={} mr_pen={}",
            st.attack, st.magic_power, st.defence, st.magic_resistance, st.move_speed,
            st.hp_regen, st.crit_chance, st.stack, c.attack_interval(), c.attack_speed_mult(),
            c.buff_count(), b.haste, b.vamp, b.defence_pen, b.mr_pen
        ));
    }
    s.own = own_unit;
    s.target = target;
}

#[derive(Default)]
struct Buffs {
    haste: f64,
    vamp: f64,
    defence_pen: f64,
    mr_pen: f64,
}
/// Totals over the unit's active buffs (items included where the game
/// applies them as buffs).
fn buffs(e: &StableEntity<'_, '_>) -> Buffs {
    let mut b = Buffs::default();
    for i in 0..e.buff_count().min(256) {
        if let Some(buff) = e.buff_at(i) {
            b.haste += f64::from(buff.skill_cooldown_mult);
            b.vamp += buff.vamp as f64;
            b.defence_pen += buff.defence_penetration as f64;
            b.mr_pen += buff.magic_resistance_penetration as f64;
        }
    }
    b
}
fn read(sim: &StableSim<'_>, e: &StableEntity<'_, '_>, side: Option<usize>) -> Unit {
    let st = e.stat();
    let b = buffs(e);
    let interval = e.attack_interval();
    let range =
        crate::native_adapter::attack_ranges(sim, e.id()).map_or(0., |r| r.0 as f64 / 1000.);
    let kind = if e.is_champion() {
        Kind::Champion
    } else if e.is_tower() || e.name().as_deref() == Some("nexus") {
        Kind::Tower
    } else if e.is_minion() {
        Kind::Minion
    } else {
        Kind::Monster
    };
    Unit {
        champion: e.is_champion().then(|| e.name()).flatten(),
        kind,
        friendly: side.is_some_and(|team| e.team() == team),
        level: e.level(),
        hp: e.hp(),
        shield: e.shield(),
        values: [
            st.attack as f64,
            st.magic_power as f64,
            // Ticks between attacks at 60 ticks per second.
            if interval > 0 {
                60. / interval as f64
            } else {
                0.
            },
            st.defence as f64,
            st.magic_resistance as f64,
            b.haste,
            st.crit_chance as f64,
            // Game units per tick, shown per second.
            st.move_speed as f64 * 60. / 1000.,
            range,
            b.vamp,
            b.defence_pen,
            b.mr_pen,
        ],
        at: Instant::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn values_are_written_like_the_game_panel() {
        assert_eq!(format(104.4, Format::Whole), "104");
        assert_eq!(format(60. / 90., Format::Decimal), "0.67");
        assert_eq!(format(15., Format::Percent), "15%");
        assert_eq!(ICONS.len(), FORMATS.len());
    }
    #[test]
    fn selection_is_cleared_with_none() {
        select(Some(7));
        assert_eq!(target_id(), Some(7));
        select(None);
        assert_eq!(target_id(), None);
    }
}
