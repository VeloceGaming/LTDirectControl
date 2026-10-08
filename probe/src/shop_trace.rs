//! Diagnostic-only purchase records for the shop investigation. Never changes
//! native state. Static review shows the simulation buys only inside the
//! team base, one validated step per tick, each step lowering gold. Records
//! every step of all ten players so a user test shows component order,
//! slot usage (vanilla four, item mods six) and leftover parts.
use crate::native_timing::MatchKey;
use std::sync::Mutex;

const LINES_PER_MATCH: usize = 400;
/// Item lists are copied only when gold falls or at this period, so the
/// tracer does not allocate ten item lists every tick.
const FULL_CHECK_TICKS: usize = 30;

#[derive(Clone, Debug, PartialEq)]
pub struct Detail {
    pub champion: String,
    pub build: Option<Vec<String>>,
    /// Native owned-item Vec length (+0x318), compared with SDK item keys to
    /// show whether empty slots are stored as placeholder items.
    pub native_owned: Option<usize>,
}

#[derive(Clone, Debug)]
struct Entry {
    id: usize,
    gold: usize,
    items: Vec<String>,
    checked: usize,
}

struct State {
    key: Option<MatchKey>,
    tick: Option<usize>,
    anchors: bool,
    lines: usize,
    players: Vec<Entry>,
}

pub struct ShopTrace(Mutex<State>);
pub static SHOP_TRACE: ShopTrace = ShopTrace::new();

impl ShopTrace {
    pub const fn new() -> Self {
        Self(Mutex::new(State {
            key: None,
            tick: None,
            anchors: false,
            lines: 0,
            players: Vec::new(),
        }))
    }

    /// True once per simulation tick of a match; many actors call think.
    pub fn due(&self, key: MatchKey, tick: usize) -> bool {
        let Ok(mut s) = self.0.lock() else {
            return false;
        };
        if s.key != Some(key) {
            *s = State {
                key: Some(key),
                tick: None,
                anchors: false,
                lines: 0,
                players: Vec::new(),
            };
        }
        if s.tick == Some(tick) {
            return false;
        }
        s.tick = Some(tick);
        true
    }

    /// True once per match: the caller logs the buyer code anchors.
    pub fn take_anchor_check(&self) -> bool {
        self.0
            .lock()
            .is_ok_and(|mut s| !std::mem::replace(&mut s.anchors, true))
    }

    /// `players` holds (id, side, gold). `items` copies one player's owned
    /// keys; `detail` is fetched only for a line that is written.
    pub fn observe(
        &self,
        tick: usize,
        players: &[(usize, usize, usize)],
        items: impl Fn(usize) -> Vec<String>,
        detail: impl Fn(usize) -> Detail,
        write: impl Fn(&str),
    ) {
        let Ok(mut s) = self.0.lock() else {
            return;
        };
        for &(id, side, gold) in players {
            let known = s.players.iter().position(|e| e.id == id);
            let Some(index) = known else {
                s.players.push(Entry {
                    id,
                    gold,
                    items: items(id),
                    checked: tick,
                });
                continue;
            };
            let e = &s.players[index];
            let fell = gold < e.gold;
            if !fell && tick.saturating_sub(e.checked) < FULL_CHECK_TICKS {
                s.players[index].gold = gold;
                continue;
            }
            let now = items(id);
            let (before_gold, before) = (e.gold, e.items.clone());
            s.players[index] = Entry {
                id,
                gold,
                items: now.clone(),
                checked: tick,
            };
            if now == before && !fell {
                continue;
            }
            if s.lines >= LINES_PER_MATCH {
                continue;
            }
            s.lines += 1;
            let kind = if now == before {
                "GOLD_DROP"
            } else if fell {
                "PURCHASE"
            } else {
                "ITEMS_CHANGED"
            };
            let d = detail(id);
            write(&format!(
                "SHOP {kind} tick={tick} player={id} side={side} champion={:?} gold={before_gold}->{gold} spent={} owned_before={before:?} owned_after={now:?} owned_count={} native_owned={:?} build={:?}",
                d.champion,
                before_gold.saturating_sub(gold),
                now.len(),
                d.native_owned,
                d.build,
            ));
            if s.lines == LINES_PER_MATCH {
                write("SHOP TRACE line limit reached for this match");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn key() -> MatchKey {
        (1, 2, 0)
    }
    fn detail(_: usize) -> Detail {
        Detail {
            champion: "ninja".into(),
            build: None,
            native_owned: Some(1),
        }
    }

    #[test]
    fn records_each_purchase_step_once_and_ignores_income() {
        let t = ShopTrace::new();
        let items = RefCell::new(vec![]);
        let lines = RefCell::new(vec![]);
        let run = |tick: usize, gold: usize| {
            if t.due(key(), tick) {
                t.observe(
                    tick,
                    &[(4, 0, gold)],
                    |_| items.borrow().clone(),
                    detail,
                    |l| lines.borrow_mut().push(l.to_owned()),
                );
            }
        };
        run(1, 500);
        run(2, 520);
        run(2, 520);
        *items.borrow_mut() = vec!["long_sword".to_string()];
        run(3, 170);
        *items.borrow_mut() = vec!["long_sword".to_string(), "dagger".to_string()];
        run(4, 0);
        let lines = lines.borrow();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].starts_with("SHOP PURCHASE tick=3 player=4"));
        assert!(lines[0].contains("gold=520->170 spent=350"));
        assert!(lines[1].contains("owned_count=2"));
    }

    #[test]
    fn periodic_check_catches_changes_without_gold_loss_and_new_match_resets() {
        let t = ShopTrace::new();
        let items = RefCell::new(vec![]);
        let lines = RefCell::new(vec![]);
        let run = |key: MatchKey, tick: usize| {
            if t.due(key, tick) {
                t.observe(
                    tick,
                    &[(4, 0, 100)],
                    |_| items.borrow().clone(),
                    detail,
                    |l| lines.borrow_mut().push(l.to_owned()),
                );
            }
        };
        run(key(), 1);
        *items.borrow_mut() = vec!["boots".to_string()];
        run(key(), 10);
        assert!(lines.borrow().is_empty());
        run(key(), 31);
        assert!(lines.borrow()[0].starts_with("SHOP ITEMS_CHANGED"));
        assert!(t.take_anchor_check());
        assert!(!t.take_anchor_check());
        run((9, 9, 9), 1);
        assert!(t.take_anchor_check());
    }
}
