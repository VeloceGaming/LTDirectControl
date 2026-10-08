//! Manual shopping. While the champion stands in its base the simulation asks
//! the champion's controller two questions every tick: "upgrade an owned item
//! into what?" and "buy which new base item?". The adapter's hooks route those
//! questions here. For the controlled champion with Manual shopping on, the
//! answer is the next step toward an item the player queued in the shop, or
//! nothing: no item is ever queued automatically. Everyone else keeps the
//! native (or another mod's) answer. The simulation still validates every
//! answer (upgrade edge, gold, slot cap) and performs the purchase itself.
use crate::native_timing::MatchKey;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

/// One purchase step, in native catalogue indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Turn the owned item in `slot` into catalogue item `item`.
    Upgrade { slot: usize, item: usize },
    /// Buy base (tier 0) catalogue item `item` into a free slot.
    New { item: usize },
}
impl Step {
    pub fn item(self) -> usize {
        match self {
            Step::Upgrade { item, .. } | Step::New { item } => item,
        }
    }
}

/// What a hook should return to the simulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// Call the original decision unchanged.
    Native,
    /// Decline: the controlled champion buys only what was queued.
    Nothing,
    Upgrade {
        slot: usize,
        item: usize,
    },
    New {
        item: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub key: String,
    pub tier: usize,
    pub price: usize,
    pub category: String,
    /// Stat keys with a non-zero value in the live item data (shop filters).
    pub stats: Vec<String>,
    /// Catalogue indices this item can upgrade into.
    pub next: Vec<usize>,
}

/// Engine-ordered catalogue built from the registered item metadata.
pub fn catalogue(keys: &[String], meta: &crate::native_items::Catalogue) -> Option<Vec<Item>> {
    let index = |k: &str| keys.iter().position(|x| x == k);
    keys.iter()
        .map(|key| {
            let spec = meta.get(key)?;
            Some(Item {
                key: key.clone(),
                tier: usize::try_from(spec.get("tier")?.as_u64()?).ok()?,
                price: usize::try_from(spec.get("price")?.as_u64()?).ok()?,
                category: spec
                    .get("category")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned(),
                stats: spec
                    .get("stat")
                    .and_then(|v| v.as_object())
                    .into_iter()
                    .flatten()
                    .filter(|(_, v)| v.as_f64().is_some_and(|n| n != 0.))
                    .map(|(k, _)| k.clone())
                    .collect(),
                next: spec
                    .get("next_tier")
                    .and_then(|v| v.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|v| index(v.as_str()?))
                    .collect(),
            })
        })
        .collect()
}

/// One Buy. `excluded` lists inventory slots that already held this item
/// when it was bought, so the order is only satisfied by an additional copy.
#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub item: usize,
    pub excluded: Vec<usize>,
}
/// A new order for `target` against the inventory the shop shows.
pub fn new_order(live: &Live, target: usize) -> Order {
    Order {
        item: target,
        excluded: (0..live.owned.len())
            .filter(|i| live.owned[*i] == target)
            .collect(),
    }
}

/// Cheapest route to `target` starting from owned slots that `avail` allows,
/// or from a new base item; `slots_full` excludes routes needing a new slot.
fn plan_from(
    cat: &[Item],
    owned: &[usize],
    avail: &dyn Fn(usize) -> bool,
    target: usize,
    slots_full: bool,
) -> Option<Vec<Step>> {
    if target >= cat.len() {
        return None;
    }
    type Route = (Option<usize>, Vec<usize>);
    fn routes(
        cat: &[Item],
        owned: &[usize],
        avail: &dyn Fn(usize) -> bool,
        key: usize,
        depth: usize,
        root: bool,
    ) -> Vec<Route> {
        if !root {
            if let Some(slot) = (0..owned.len()).find(|i| owned[*i] == key && avail(*i)) {
                return vec![(Some(slot), Vec::new())];
            }
        }
        let mut out = Vec::new();
        if cat[key].tier == 0 {
            out.push((None, vec![key]));
        }
        if depth < 6 {
            for p in (0..cat.len()).filter(|i| cat[*i].next.contains(&key)) {
                for (slot, mut steps) in routes(cat, owned, avail, p, depth + 1, false) {
                    steps.push(key);
                    out.push((slot, steps));
                }
            }
        }
        out
    }
    let cost = |steps: &Vec<usize>| steps.iter().map(|i| cat[*i].price).sum::<usize>();
    let (slot, items) = routes(cat, owned, avail, target, 0, true)
        .into_iter()
        .filter(|(slot, _)| !(slots_full && slot.is_none()))
        .min_by_key(|(slot, steps)| (cost(steps), slot.is_none()))?;
    let mut slot = slot;
    let mut out = Vec::new();
    for item in items {
        out.push(match slot {
            Some(s) => Step::Upgrade { slot: s, item },
            None => Step::New { item },
        });
        // Later steps upgrade the slot the first step filled.
        slot = Some(slot.unwrap_or(owned.len()));
    }
    Some(out)
}

/// Cheapest route from what the champion owns to `target`, as purchase steps.
/// None when the target is owned or unreachable.
#[cfg(test)]
pub fn plan(cat: &[Item], owned: &[usize], target: usize, slots_full: bool) -> Option<Vec<Step>> {
    if owned.contains(&target) {
        return None;
    }
    plan_from(cat, owned, &|_| true, target, slots_full)
}

/// Where each order stands against an inventory.
#[derive(Clone, Debug, PartialEq)]
pub enum Assigned {
    /// Satisfied by this inventory slot.
    Done(usize),
    /// Still to buy: its cheapest route, None when blocked (full inventory).
    Open(Option<Vec<Step>>),
}
/// In queue order, each order claims the slot that satisfies it (a copy it
/// did not exclude) or the slot its route upgrades from, so a later order
/// never takes a part an earlier one is building on.
pub fn assign(cat: &[Item], owned: &[usize], orders: &[Order], full: bool) -> Vec<Assigned> {
    let mut claimed = vec![false; owned.len()];
    orders
        .iter()
        .map(|o| {
            if let Some(i) = (0..owned.len())
                .find(|i| !claimed[*i] && owned[*i] == o.item && !o.excluded.contains(i))
            {
                claimed[i] = true;
                return Assigned::Done(i);
            }
            let free = claimed.clone();
            let steps = plan_from(
                cat,
                owned,
                &|i| !free[i] && owned[i] != o.item,
                o.item,
                full,
            );
            if let Some(Step::Upgrade { slot, .. }) = steps.as_ref().and_then(|s| s.first()) {
                if *slot < claimed.len() {
                    claimed[*slot] = true;
                }
            }
            Assigned::Open(steps)
        })
        .collect()
}

fn slots_full(live: &Live) -> bool {
    let cap = live.slots();
    cap > 0 && live.owned.len() >= cap
}

/// The next step the buyer hands out: the first open order whose next step
/// is affordable, at the game's price.
pub fn next_step(cat: &[Item], live: &Live, orders: &[Order]) -> Option<(usize, Step)> {
    assign(cat, &live.owned, orders, slots_full(live))
        .into_iter()
        .enumerate()
        .find_map(|(n, a)| match a {
            Assigned::Open(Some(steps)) => {
                let first = *steps.first()?;
                (live.gold >= cat[first.item()].price).then_some((n, first))
            }
            _ => None,
        })
}

/// Gold each order still needs (0 when satisfied, None when blocked).
pub fn order_costs(cat: &[Item], live: &Live, orders: &[Order]) -> Vec<Option<usize>> {
    assign(cat, &live.owned, orders, slots_full(live))
        .into_iter()
        .map(|a| match a {
            Assigned::Done(_) => Some(0),
            Assigned::Open(Some(steps)) => Some(steps.iter().map(|s| cat[s.item()].price).sum()),
            Assigned::Open(None) => None,
        })
        .collect()
}
/// The first order still to buy and its route (the strip's next purchase).
pub fn first_open(cat: &[Item], live: &Live, orders: &[Order]) -> Option<(usize, Vec<Step>)> {
    assign(cat, &live.owned, orders, slots_full(live))
        .into_iter()
        .enumerate()
        .find_map(|(n, a)| match a {
            Assigned::Open(Some(steps)) if !steps.is_empty() => Some((orders[n].item, steps)),
            _ => None,
        })
}

/// What one more Buy of `target` would mean.
#[derive(Clone, Debug, PartialEq)]
pub enum Offer {
    /// Unreachable item (should not happen for catalogue items).
    Owned,
    /// No free slot for a new chain.
    Blocked,
    Plan(Vec<Step>),
}
pub fn offer(cat: &[Item], live: &Live, orders: &[Order], target: usize) -> Offer {
    if target >= cat.len() {
        return Offer::Blocked;
    }
    // No purchase is blocked: any item can be bought again (user's rule).
    // Only a new chain needs a free slot, which the game itself enforces.
    let mut all = orders.to_vec();
    all.push(new_order(live, target));
    match assign(cat, &live.owned, &all, slots_full(live)).pop() {
        Some(Assigned::Open(Some(steps))) => Offer::Plan(steps),
        Some(Assigned::Open(None)) => Offer::Blocked,
        _ => Offer::Owned,
    }
}

/// The steps the buyer will hand out next while the champion stays in base,
/// run ahead of time exactly as `publish` chooses them (`next_step`), one step
/// at a time. Returns the projected inventory/gold and the items bought.
pub fn project(cat: &[Item], live: &Live, orders: &[Order]) -> (Live, Vec<usize>) {
    let mut out = live.clone();
    let mut bought = Vec::new();
    for _ in 0..64 {
        let Some((_, step)) = next_step(cat, &out, orders) else {
            break;
        };
        out.gold -= cat[step.item()].price;
        match step {
            Step::Upgrade { slot, item } => out.owned[slot] = item,
            Step::New { item } => out.owned.push(item),
        }
        bought.push(step.item());
    }
    (out, bought)
}

/// How the controlled champion's questions are answered this tick.
#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    /// Manual shopping does not apply (off, AI control, or champion unknown).
    Native(&'static str),
    /// Manual shopping applies to this native player object.
    Manual(usize),
    /// Before the match starts (loading and the Start-control wait), the
    /// game buys starting items while the player may still switch champion:
    /// every listed player of this match buys nothing until Start. The
    /// controlled champion, once known, is already manual.
    Prestart {
        controlled: Option<usize>,
        hold: Vec<usize>,
    },
}

/// Live inputs read on the simulation side each tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Live {
    pub owned: Vec<usize>,
    pub build: Vec<usize>,
    pub gold: usize,
    /// Item slots the game allows (its purchase gate); 0 when unknown.
    pub capacity: usize,
}
impl Live {
    /// Slot limit: the game's own gate, else the build length as a fallback.
    pub fn slots(&self) -> usize {
        if self.capacity > 0 {
            self.capacity
        } else {
            self.build.len()
        }
    }
}

/// What the shop window shows; published by the simulation side.
#[derive(Clone, Debug)]
pub struct View {
    pub cat: Arc<Vec<Item>>,
    pub live: Live,
    /// Queued item per order (duplicates allowed), in queue order.
    pub queue: Vec<usize>,
    pub orders: Vec<Order>,
    pub manual: bool,
    pub in_base: bool,
    /// Inventory capacity (native final-build length), 0 when unknown.
    pub capacity: usize,
}

#[derive(Default)]
struct Proof {
    seen: Vec<usize>,
    upgrade_calls: usize,
    new_calls: usize,
    native: usize,
    nothing: usize,
    held: usize,
    upgrades: usize,
    news: usize,
    logged: Option<(String, Instant)>,
}

struct State {
    key: Option<MatchKey>,
    tick: Option<usize>,
    /// Native player object of the controlled champion while Manual shopping
    /// applies; 0 otherwise (every answer is native).
    player: usize,
    queue: Vec<Order>,
    /// One-shot answer for the next in-base question. Consumed when given, so
    /// a step the simulation already performed is never answered twice.
    step: Option<Step>,
    /// A step was handed out since the last publish.
    handed: bool,
    previous_owned: Option<Vec<usize>>,
    /// Last time a hook was asked about the controlled champion (in base).
    asked: Option<Instant>,
    cat: Option<Arc<Vec<Item>>>,
    live: Live,
    proof: Proof,
    last_log: Option<String>,
    announced: bool,
    mode_log: Option<String>,
    /// Pre-start: native player objects of this match that buy nothing.
    hold: Vec<usize>,
}

/// Fields 2-3 mirror `State::player` and "holding anyone", so the buyer
/// hooks (called for every player of every running simulation) turn others
/// away without the lock. Lock-free answers are counted in `perf`.
pub struct Shop(
    Mutex<State>,
    Mutex<Option<(MatchKey, Arc<Vec<Item>>)>>,
    AtomicUsize,
    AtomicBool,
);
pub static SHOP: Shop = Shop::new();

impl Default for Shop {
    fn default() -> Self {
        Self::new()
    }
}
impl Shop {
    pub const fn new() -> Self {
        Self(
            Mutex::new(State {
                key: None,
                tick: None,
                player: 0,
                queue: Vec::new(),
                step: None,
                handed: false,
                previous_owned: None,
                asked: None,
                cat: None,
                live: Live {
                    owned: Vec::new(),
                    build: Vec::new(),
                    gold: 0,
                    capacity: 0,
                },
                proof: Proof {
                    seen: Vec::new(),
                    upgrade_calls: 0,
                    new_calls: 0,
                    native: 0,
                    nothing: 0,
                    held: 0,
                    upgrades: 0,
                    news: 0,
                    logged: None,
                },
                last_log: None,
                announced: false,
                mode_log: None,
                hold: Vec::new(),
            }),
            Mutex::new(None),
            AtomicUsize::new(0),
            AtomicBool::new(false),
        )
    }
    /// Written only on change: hooks on other threads read these constantly.
    fn mirror(&self, s: &State) {
        if self.2.load(Ordering::Relaxed) != s.player {
            self.2.store(s.player, Ordering::Release);
        }
        let holding = !s.hold.is_empty();
        if self.3.load(Ordering::Relaxed) != holding {
            self.3.store(holding, Ordering::Release);
        }
    }

    /// Engine-ordered catalogue, built once per match.
    pub fn catalogue_for(
        &self,
        key: MatchKey,
        make: impl FnOnce() -> Option<Vec<Item>>,
    ) -> Option<Arc<Vec<Item>>> {
        let mut c = self.1.lock().ok()?;
        if let Some((_, cat)) = c.as_ref().filter(|(k, _)| *k == key) {
            return Some(cat.clone());
        }
        let cat = Arc::new(make()?);
        *c = Some((key, cat.clone()));
        Some(cat)
    }

    /// True once per simulation tick of a match.
    pub fn due(&self, key: MatchKey, tick: usize) -> bool {
        let Ok(mut s) = self.0.lock() else {
            return false;
        };
        if s.key != Some(key) {
            *s = Shop::new()
                .0
                .into_inner()
                .unwrap_or_else(|e| e.into_inner());
            s.key = Some(key);
            self.mirror(&s);
        }
        if s.tick == Some(tick) {
            return false;
        }
        s.tick = Some(tick);
        true
    }

    /// Shop window: one more order (first come, first served; unaffordable
    /// orders never block a later affordable one).
    pub fn buy(&self, order: Order) {
        if let Ok(mut s) = self.0.lock() {
            s.queue.push(order);
        }
    }
    /// Shop window: queue items after the ones already queued, in order
    /// (Recommended's "Queue whole build"); items already owned or queued are
    /// skipped. Returns how many were added.
    pub fn enqueue_back(&self, items: &[usize]) -> usize {
        let Ok(mut s) = self.0.lock() else {
            return 0;
        };
        let mut added = 0;
        for item in items {
            if !s.queue.iter().any(|o| o.item == *item) && !s.live.owned.contains(item) {
                let order = new_order(&s.live, *item);
                s.queue.push(order);
                added += 1;
            }
        }
        added
    }
    /// Shop window: drop one order (by its position in the queue).
    pub fn unqueue(&self, index: usize) {
        if let Ok(mut s) = self.0.lock() {
            if index < s.queue.len() {
                s.queue.remove(index);
            }
        }
    }
    /// Shop window data; None until the catalogue is known this match.
    pub fn view(&self) -> Option<View> {
        let s = self.0.lock().ok()?;
        let cat = s.cat.clone()?;
        Some(View {
            capacity: s.live.slots(),
            live: s.live.clone(),
            queue: s.queue.iter().map(|o| o.item).collect(),
            orders: s.queue.clone(),
            manual: s.player != 0,
            in_base: s
                .asked
                .is_some_and(|t| t.elapsed() < Duration::from_millis(400)),
            cat,
        })
    }

    /// Simulation-side update. `data` is None when the catalogue, owned keys
    /// or build could not be read: with Manual shopping applying, that fails
    /// closed (nothing is bought) rather than falling back to auto-buy.
    pub fn publish(
        &self,
        mode: Mode,
        cat: Option<Arc<Vec<Item>>>,
        live: Option<Live>,
        write: impl Fn(&str),
    ) {
        let Ok(mut s) = self.0.lock() else {
            return;
        };
        if cat.is_some() {
            s.cat = cat.clone();
        }
        if let Some(live) = &live {
            s.live = live.clone();
        }
        let described = match &mode {
            Mode::Prestart { controlled, hold } => format!(
                "Prestart(controlled={controlled:x?}, holding {} players)",
                hold.len()
            ),
            other => format!("{other:?}"),
        };
        if s.mode_log.as_deref() != Some(described.as_str()) {
            write(&format!(
                "SHOP MODE {} tick={:?}: {described} game_item_slots={:?} controlled_owned={:?}",
                if s.announced {
                    "changed"
                } else {
                    "first decision of match"
                },
                s.tick,
                live.as_ref().map(|l| l.capacity),
                live.as_ref().zip(cat.as_ref()).map(|(l, c)| l
                    .owned
                    .iter()
                    .map(|i| c[*i].key.clone())
                    .collect::<Vec<_>>())
            ));
            s.announced = true;
            s.mode_log = Some(described);
        }
        let player = match mode {
            Mode::Native(_) => {
                s.hold.clear();
                s.player = 0;
                self.mirror(&s);
                s.step = None;
                s.previous_owned = None;
                return;
            }
            Mode::Manual(player) => {
                s.hold.clear();
                player
            }
            Mode::Prestart { controlled, hold } => {
                s.hold = hold;
                match controlled {
                    Some(player) => player,
                    None => {
                        s.player = 0;
                        self.mirror(&s);
                        s.step = None;
                        s.previous_owned = None;
                        return;
                    }
                }
            }
        };
        s.player = player;
        self.mirror(&s);
        let (Some(cat), Some(live)) = (cat, live) else {
            s.step = None;
            let line = "SHOP MODE manual, item data unavailable: buying nothing".to_owned();
            if s.last_log.as_deref() != Some(line.as_str()) {
                write(&line);
                s.last_log = Some(line);
            }
            return;
        };
        // The user's rule: with Manual shopping on, nothing arrives unqueued.
        if let Some(before) = s.previous_owned.take() {
            if before != live.owned && !s.handed {
                write(&format!(
                    "SHOP UNEXPECTED owned {:?} -> {:?} without a shop step",
                    before.iter().map(|i| &cat[*i].key).collect::<Vec<_>>(),
                    live.owned.iter().map(|i| &cat[*i].key).collect::<Vec<_>>()
                ));
            }
        }
        s.previous_owned = Some(live.owned.clone());
        s.handed = false;
        // Orders the inventory already satisfies leave the queue.
        s.queue.retain(|o| o.item < cat.len());
        let assigned = assign(&cat, &live.owned, &s.queue, slots_full(&live));
        let mut done = assigned.iter().map(|a| matches!(a, Assigned::Done(_)));
        s.queue.retain(|_| !done.next().unwrap_or(false));
        // The first order whose next step is affordable now.
        let step = next_step(&cat, &live, &s.queue).map(|(_, step)| step);
        s.step = step;
        let line = format!(
            "SHOP STATE queue={:?} next={:?} owned={:?}",
            s.queue.iter().map(|o| &cat[o.item].key).collect::<Vec<_>>(),
            step.map(|st| match st {
                Step::Upgrade { slot, item } => format!("upgrade slot {slot} -> {}", cat[item].key),
                Step::New { item } => format!("new {}", cat[item].key),
            }),
            live.owned.iter().map(|i| &cat[*i].key).collect::<Vec<_>>(),
        );
        if s.last_log.as_deref() != Some(line.as_str()) {
            write(&format!("{line} gold={}", live.gold));
            s.last_log = Some(line);
        }
        let p = &s.proof;
        let summary = format!(
            "SHOP HOOK PROOF controlled=0x{player:x} seen_locked={:x?} controlled_upgrade_calls={} controlled_new_calls={} answers other_players_native_locked={} (lock-free answers: PERF shop_fast_reject) prestart_held={} controlled_nothing={} upgrade={} new={}",
            p.seen, p.upgrade_calls, p.new_calls, p.native, p.held, p.nothing, p.upgrades, p.news
        );
        if p.logged
            .as_ref()
            .is_none_or(|(old, at)| old != &summary && at.elapsed() > Duration::from_secs(5))
        {
            write(&summary);
            s.proof.logged = Some((summary, Instant::now()));
        }
    }

    fn answer(&self, player: usize, upgrade: bool) -> Answer {
        // Same outcome as the locked check below for anyone who is neither
        // the controlled player nor held before Start.
        if !self.3.load(Ordering::Acquire)
            && (player == 0 || self.2.load(Ordering::Acquire) != player)
        {
            crate::perf::hook(crate::perf::Hook::ShopFast);
            return Answer::Native;
        }
        crate::perf::hook(crate::perf::Hook::ShopLocked);
        let since = Instant::now();
        let Ok(mut s) = self.0.lock() else {
            return Answer::Native;
        };
        crate::perf::waited(crate::perf::Wait::Shop, since);
        if s.proof.seen.len() < 12 && !s.proof.seen.contains(&player) {
            s.proof.seen.push(player);
        }
        if s.player == 0 || s.player != player {
            if s.hold.contains(&player) {
                // Pre-start: this match's champions wait for Start.
                s.proof.held += 1;
                return Answer::Nothing;
            }
            s.proof.native += 1;
            return Answer::Native;
        }
        s.asked = Some(Instant::now());
        if upgrade {
            s.proof.upgrade_calls += 1;
        } else {
            s.proof.new_calls += 1;
        }
        match s.step {
            Some(Step::Upgrade { slot, item }) if upgrade => {
                s.step = None;
                s.handed = true;
                s.proof.upgrades += 1;
                Answer::Upgrade { slot, item }
            }
            Some(Step::New { item }) if !upgrade => {
                s.step = None;
                s.handed = true;
                s.proof.news += 1;
                Answer::New { item }
            }
            _ => {
                s.proof.nothing += 1;
                Answer::Nothing
            }
        }
    }
    /// The simulation's "upgrade which owned item into what?" question.
    pub fn upgrade_answer(&self, player: usize) -> Answer {
        self.answer(player, true)
    }
    /// The simulation's "buy which new base item?" question.
    pub fn new_item_answer(&self, player: usize) -> Answer {
        self.answer(player, false)
    }
    pub fn reset_session(&self) {
        if let Ok(mut s) = self.0.lock() {
            *s = Shop::new()
                .0
                .into_inner()
                .unwrap_or_else(|e| e.into_inner());
            self.mirror(&s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(key: &str, tier: usize, price: usize, next: &[usize]) -> Item {
        Item {
            key: key.into(),
            tier,
            price,
            category: String::new(),
            stats: Vec::new(),
            next: next.to_vec(),
        }
    }
    // 0 sword -> 1 longsword -> {2 dirk, 3 axe}; 4 dagger -> 5 wind -> 3 axe;
    // 3 axe -> 6 flare; 7 boots (tier 0, no upgrades)
    fn riot() -> Arc<Vec<Item>> {
        Arc::new(vec![
            item("ironsword", 0, 250, &[1]),
            item("longsword", 1, 500, &[2, 3]),
            item("dirk", 2, 400, &[]),
            item("axe", 2, 500, &[6]),
            item("dagger", 0, 250, &[5]),
            item("wind", 1, 500, &[3]),
            item("flare", 3, 700, &[]),
            item("boots", 0, 250, &[]),
        ])
    }
    fn order(item: usize) -> Order {
        Order {
            item,
            excluded: Vec::new(),
        }
    }
    fn live(owned: &[usize], gold: usize) -> Option<Live> {
        Some(Live {
            owned: owned.to_vec(),
            build: vec![6, 7, 2, 2],
            gold,
            capacity: 0,
        })
    }

    #[test]
    fn plan_upgrades_an_owned_part_and_picks_the_cheapest_branch() {
        let cat = riot();
        assert_eq!(
            plan(&cat, &[7, 1], 6, false),
            Some(vec![
                Step::Upgrade { slot: 1, item: 3 },
                Step::Upgrade { slot: 1, item: 6 },
            ])
        );
        let steps = plan(&cat, &[7], 6, false).unwrap();
        assert!(matches!(steps[0], Step::New { item: 0 | 4 }));
        assert!(matches!(
            steps.last(),
            Some(Step::Upgrade { slot: 1, item: 6 })
        ));
        assert_eq!(plan(&cat, &[6], 6, false), None);
        // Full inventory: only routes from an owned item remain.
        assert_eq!(plan(&cat, &[7], 6, true), None);
    }

    #[test]
    fn never_buys_a_non_base_item_as_new() {
        let cat = riot();
        for target in 0..cat.len() {
            if let Some(steps) = plan(&cat, &[], target, false) {
                if let Step::New { item } = steps[0] {
                    assert_eq!(cat[item].tier, 0, "{}", cat[target].key);
                }
            }
        }
    }

    #[test]
    fn nothing_is_bought_unless_the_player_queues_it() {
        let shop = Shop::new();
        let lines = std::cell::RefCell::new(Vec::new());
        let w = |l: &str| lines.borrow_mut().push(l.to_owned());
        assert!(shop.due((1, 1, 1), 5));
        assert!(!shop.due((1, 1, 1), 5));
        // Manual on, rich, nothing queued: every answer is Nothing.
        shop.publish(Mode::Manual(0xabc), Some(riot()), live(&[1], 5000), w);
        assert_eq!(shop.upgrade_answer(0xabc), Answer::Nothing);
        assert_eq!(shop.new_item_answer(0xabc), Answer::Nothing);
        // Other champions keep their native answers.
        assert_eq!(shop.upgrade_answer(0xdef), Answer::Native);
        // Buying from the shop hands out exactly one step at a time.
        shop.buy(order(6));
        shop.publish(Mode::Manual(0xabc), Some(riot()), live(&[1], 600), w);
        assert_eq!(shop.new_item_answer(0xabc), Answer::Nothing);
        assert_eq!(
            shop.upgrade_answer(0xabc),
            Answer::Upgrade { slot: 0, item: 3 }
        );
        assert_eq!(shop.upgrade_answer(0xabc), Answer::Nothing);
        // Not enough gold for the next step: nothing, and the item stays queued.
        shop.publish(Mode::Manual(0xabc), Some(riot()), live(&[3], 100), w);
        assert_eq!(shop.upgrade_answer(0xabc), Answer::Nothing);
        assert_eq!(shop.view().unwrap().queue, vec![6]);
        assert!(shop.view().unwrap().in_base);
        // Data unavailable while manual: fail closed, not native.
        shop.publish(Mode::Manual(0xabc), None, None, w);
        assert_eq!(shop.upgrade_answer(0xabc), Answer::Nothing);
        // Manual off or AI control: native answers again.
        shop.publish(Mode::Native("off"), Some(riot()), live(&[3], 900), w);
        assert_eq!(shop.upgrade_answer(0xabc), Answer::Native);
        assert!(!lines
            .borrow()
            .iter()
            .any(|l| l.starts_with("SHOP UNEXPECTED")));
    }

    #[test]
    fn before_start_this_matchs_champions_wait_and_others_are_untouched() {
        let shop = Shop::new();
        shop.due((1, 1, 1), 1);
        // Lane not final yet: no controlled champion, all ten held.
        shop.publish(
            Mode::Prestart {
                controlled: None,
                hold: vec![0xa, 0xb],
            },
            Some(riot()),
            None,
            |_| {},
        );
        assert_eq!(shop.new_item_answer(0xa), Answer::Nothing);
        assert_eq!(shop.upgrade_answer(0xb), Answer::Nothing);
        // A champion of another simulation keeps its native answer.
        assert_eq!(shop.new_item_answer(0xc), Answer::Native);
        // Started: only the controlled champion stays manual.
        shop.publish(Mode::Manual(0xa), Some(riot()), live(&[], 500), |_| {});
        assert_eq!(shop.new_item_answer(0xa), Answer::Nothing);
        assert_eq!(shop.new_item_answer(0xb), Answer::Native);
    }

    #[test]
    fn projection_matches_the_steps_the_hooks_hand_out() {
        let cat = riot();
        let queue = vec![2, 6]; // dirk, then flare
        let start = Live {
            owned: vec![1],
            build: vec![6, 7, 2, 2],
            gold: 1500,
            capacity: 0,
        };
        let orders: Vec<Order> = queue.iter().map(|t| order(*t)).collect();
        let (projected, bought) = project(&cat, &start, &orders);
        // Dirk first (400) from the longsword; then flare needs a new chain
        // (equal-cost routes: the iron sword one is planned first):
        // ironsword 250 -> longsword 500; axe 500 is unaffordable at 350 left.
        assert_eq!(bought, vec![2, 0, 1]);
        assert_eq!(projected.gold, 1500 - 400 - 250 - 500);
        assert_eq!(projected.owned, vec![2, 1]);
        // Step by step through the real hooks gives the same purchases.
        let shop = Shop::new();
        shop.due((1, 1, 1), 1);
        for t in &queue {
            shop.buy(order(*t));
        }
        let mut live_now = start.clone();
        for expected in &bought {
            shop.publish(
                Mode::Manual(0xabc),
                Some(riot()),
                Some(live_now.clone()),
                |_| {},
            );
            let answer = match shop.upgrade_answer(0xabc) {
                Answer::Nothing => shop.new_item_answer(0xabc),
                a => a,
            };
            match answer {
                Answer::Upgrade { slot, item } => {
                    live_now.owned[slot] = item;
                    live_now.gold -= cat[item].price;
                    assert_eq!(item, *expected);
                }
                Answer::New { item } => {
                    live_now.owned.push(item);
                    live_now.gold -= cat[item].price;
                    assert_eq!(item, *expected);
                }
                other => panic!("expected a step, got {other:?}"),
            }
        }
        assert_eq!(live_now, projected);
        // Dirk is owned now and left the queue; boots join after the flare.
        assert_eq!(shop.enqueue_back(&[2, 7, 6]), 1);
        assert_eq!(shop.view().unwrap().queue, vec![6, 7]);
    }

    #[test]
    fn lock_free_rejection_matches_the_locked_answers() {
        let shop = Shop::new();
        shop.due((1, 1, 1), 1);
        // Manual shopping off: everyone is native.
        shop.publish(Mode::Native("off"), Some(riot()), live(&[], 900), |_| {});
        assert_eq!(shop.new_item_answer(0xabc), Answer::Native);
        // Manual: others stay native, the controlled player gets its step.
        shop.buy(order(2));
        shop.publish(Mode::Manual(0xabc), Some(riot()), live(&[], 900), |_| {});
        assert_eq!(shop.new_item_answer(0xdef), Answer::Native);
        assert_eq!(shop.upgrade_answer(0), Answer::Native);
        assert_eq!(shop.new_item_answer(0xabc), Answer::New { item: 0 });
        // Pre-start: every held player buys nothing, even other ones.
        shop.publish(
            Mode::Prestart {
                controlled: None,
                hold: vec![0xdef],
            },
            Some(riot()),
            live(&[], 900),
            |_| {},
        );
        assert_eq!(shop.new_item_answer(0xdef), Answer::Nothing);
        assert_eq!(shop.new_item_answer(0x123), Answer::Native);
        // A new match clears the mirrored player and hold.
        shop.due((2, 1, 1), 1);
        assert_eq!(shop.new_item_answer(0xdef), Answer::Native);
    }

    #[test]
    fn an_unqueued_purchase_is_reported() {
        let shop = Shop::new();
        let lines = std::cell::RefCell::new(Vec::new());
        let w = |l: &str| lines.borrow_mut().push(l.to_owned());
        shop.due((1, 1, 1), 1);
        shop.publish(Mode::Manual(0xabc), Some(riot()), live(&[1], 900), w);
        shop.publish(Mode::Manual(0xabc), Some(riot()), live(&[3], 400), w);
        assert!(lines
            .borrow()
            .iter()
            .any(|l| l.starts_with("SHOP UNEXPECTED")));
    }

    #[test]
    fn a_later_affordable_item_is_not_blocked_and_keeps_off_earlier_parts() {
        let shop = Shop::new();
        shop.due((1, 1, 1), 1);
        shop.buy(order(6)); // flare: builds on the owned longsword (next step 500)
        shop.buy(order(7)); // boots
        shop.unqueue(1); // removes just the boots order
        shop.buy(order(2)); // dirk
        shop.publish(Mode::Manual(0xabc), Some(riot()), live(&[1], 450), |_| {});
        // Flare's next step is unaffordable, so it does not block the dirk;
        // but the longsword is flare's part, so the dirk starts a new chain.
        assert_eq!(shop.upgrade_answer(0xabc), Answer::Nothing);
        assert_eq!(shop.new_item_answer(0xabc), Answer::New { item: 0 });
        assert_eq!(shop.view().unwrap().queue, vec![6, 2]);
    }

    #[test]
    fn duplicates_need_another_copy_and_nothing_is_blocked() {
        let mut cat = (*riot()).clone();
        cat[6].tier = 3; // flare: a finished (level 4) item
        let me = Live {
            owned: vec![0],
            build: vec![6, 7, 2, 2],
            gold: 1000,
            capacity: 0,
        };
        // A second iron sword is a new purchase, not "already owned".
        assert_eq!(
            offer(&cat, &me, &[], 0),
            Offer::Plan(vec![Step::New { item: 0 }])
        );
        let orders = vec![new_order(&me, 0)];
        assert_eq!(orders[0].excluded, vec![0]);
        let (after, bought) = project(&cat, &me, &orders);
        assert_eq!((after.owned.clone(), bought), (vec![0, 0], vec![0]));
        // The queue is satisfied by the new copy, not the old one.
        assert!(matches!(
            assign(&cat, &after.owned, &orders, false)[0],
            Assigned::Done(1)
        ));
        // Two pending orders of the same part: two new copies.
        let two = vec![new_order(&me, 0), new_order(&me, 0)];
        assert_eq!(project(&cat, &me, &two).0.owned, vec![0, 0, 0]);
        // No purchase is blocked: a finished item can be bought again too.
        let with_flare = Live {
            owned: vec![6],
            ..me.clone()
        };
        assert!(matches!(offer(&cat, &with_flare, &[], 6), Offer::Plan(_)));
        assert!(matches!(
            offer(&cat, &me, &[new_order(&me, 6)], 6),
            Offer::Plan(_)
        ));
    }
}
