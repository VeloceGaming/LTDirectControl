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
    /// Once per match (buy-new question only): let the game decide first
    /// and discard its answer, then ask again. The game's decision is where
    /// the AI (and the Riot item mod) completes the build plan, which Manual
    /// shopping would otherwise skip for the controlled champion.
    AskGameFirst,
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
    /// Explicit base-to-target chain; None keeps automatic cheapest planning.
    /// Stored on the order so preview, HUD and buyer never silently diverge.
    pub path: Option<Vec<usize>>,
}
/// A new order for `target` against the inventory the shop shows.
pub fn new_order(live: &Live, target: usize) -> Order {
    Order {
        item: target,
        path: None,
        excluded: (0..live.owned.len())
            .filter(|i| live.owned[*i] == target)
            .collect(),
    }
}

/// A chosen chain must be a real, acyclic route from a base to its target.
fn valid_path(cat: &[Item], path: &[usize], target: usize) -> bool {
    path.last() == Some(&target)
        && path
            .first()
            .is_some_and(|i| cat.get(*i).is_some_and(|p| p.tier == 0))
        && path
            .iter()
            .enumerate()
            .all(|(n, i)| *i < cat.len() && !path[..n].contains(i))
        && path
            .windows(2)
            .all(|edge| cat[edge[0]].next.contains(&edge[1]))
}

/// Plan strictly along the player's chosen chain. Other owned branches cannot
/// substitute for it, even if they are cheaper. A full bag therefore waits.
fn plan_on_path(
    cat: &[Item],
    owned: &[usize],
    avail: &dyn Fn(usize) -> bool,
    target: usize,
    path: &[usize],
    full: bool,
) -> Option<Vec<Step>> {
    if !valid_path(cat, path, target) {
        return None;
    }
    let mut best = None;
    for (slot, item) in owned.iter().enumerate().filter(|(i, _)| avail(*i)) {
        if let Some(at) = path[..path.len() - 1].iter().position(|p| p == item) {
            if best.is_none_or(|(_, previous)| at > previous) {
                best = Some((slot, at));
            }
        }
    }
    let (mut slot, start) = match best {
        Some((slot, at)) => (Some(slot), at + 1),
        None if full => return None,
        None => (None, 0),
    };
    Some(
        path[start..]
            .iter()
            .map(|&item| {
                let step = match slot {
                    Some(slot) => Step::Upgrade { slot, item },
                    None => Step::New { item },
                };
                slot = Some(slot.unwrap_or(owned.len()));
                step
            })
            .collect(),
    )
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
            let avail = |i: usize| !free[i] && owned[i] != o.item;
            let steps = match &o.path {
                Some(path) => plan_on_path(cat, owned, &avail, o.item, path, full),
                None => plan_from(cat, owned, &avail, o.item, full),
            };
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

/// The next step the buyer hands out, at the game's price. Normally the first
/// open order whose next step is affordable. With `vanilla` (the game's own
/// rule, "Vanilla order" in the shop) only the first open order buys: later
/// orders wait until it is finished, so slots are not filled with parts of
/// several items.
pub fn next_step(
    cat: &[Item],
    live: &Live,
    orders: &[Order],
    vanilla: bool,
) -> Option<(usize, Step)> {
    let open = open_steps(cat, live, orders);
    let affordable = |(_, step): &&(usize, Step)| live.gold >= cat[step.item()].price;
    if vanilla {
        open.first().filter(affordable).copied()
    } else {
        open.iter().find(affordable).copied()
    }
}
/// Each order still to buy (by queue position) with its next step.
fn open_steps(cat: &[Item], live: &Live, orders: &[Order]) -> Vec<(usize, Step)> {
    assign(cat, &live.owned, orders, slots_full(live))
        .into_iter()
        .enumerate()
        .filter_map(|(n, a)| match a {
            Assigned::Open(Some(steps)) => Some((n, *steps.first()?)),
            _ => None,
        })
        .collect()
}
/// The strip's "next purchase": the order item and step the buyer will buy
/// next. Without Vanilla order that is the first affordable step, or, while
/// none is affordable, the cheapest one (the first that gold will reach).
/// With Vanilla order it is always the first open order.
pub fn upcoming(
    cat: &[Item],
    live: &Live,
    orders: &[Order],
    vanilla: bool,
) -> Option<(usize, Step)> {
    let open = open_steps(cat, live, orders);
    let price = |step: &Step| cat[step.item()].price;
    let pick = if vanilla {
        open.first()
    } else {
        open.iter()
            .find(|(_, step)| live.gold >= price(step))
            .or_else(|| open.iter().min_by_key(|(_, step)| price(step)))
    };
    pick.map(|(n, step)| (orders[*n].item, *step))
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
    offer_with_path(cat, live, orders, target, None)
}

/// Preview the same order the buyer receives, including an optional path.
pub fn offer_with_path(
    cat: &[Item],
    live: &Live,
    orders: &[Order],
    target: usize,
    path: Option<&[usize]>,
) -> Offer {
    if target >= cat.len() {
        return Offer::Blocked;
    }
    // No purchase is blocked: any item can be bought again (user's rule).
    // Only a new chain needs a free slot, which the game itself enforces.
    let mut all = orders.to_vec();
    let mut order = new_order(live, target);
    order.path = path.map(<[usize]>::to_vec);
    all.push(order);
    match assign(cat, &live.owned, &all, slots_full(live)).pop() {
        Some(Assigned::Open(Some(steps))) => Offer::Plan(steps),
        Some(Assigned::Open(None)) => Offer::Blocked,
        _ => Offer::Owned,
    }
}

/// The steps the buyer will hand out next while the champion stays in base,
/// run ahead of time exactly as `publish` chooses them (`next_step`), one step
/// at a time. Returns the projected inventory/gold and the items bought.
pub fn project(cat: &[Item], live: &Live, orders: &[Order], vanilla: bool) -> (Live, Vec<usize>) {
    let mut out = live.clone();
    let mut bought = Vec::new();
    for _ in 0..64 {
        let Some((_, step)) = next_step(cat, &out, orders, vanilla) else {
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
    /// The game completed the build plan this match (Answer::AskGameFirst)
    /// and `live.build` was read after it.
    pub planned: bool,
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
    /// "Vanilla order": finish the first order before buying for the next.
    vanilla_order: bool,
    /// The game's own buy-new decision was requested once this match (see
    /// `Answer::AskGameFirst`), and has run; its log line waits for the
    /// next publish.
    planned: bool,
    planned_log: Option<String>,
    /// A publish after that decision: `live.build` holds the whole plan.
    /// (The decision runs in the buyer, after this tick's publish.)
    plan_ready: bool,
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
                vanilla_order: false,
                planned: false,
                planned_log: None,
                plan_ready: false,
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

    /// The "Vanilla order" setting, read each tick by the simulation.
    pub fn set_vanilla_order(&self, on: bool) {
        if let Ok(mut s) = self.0.lock() {
            s.vanilla_order = on;
        }
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
    pub fn enqueue_back(
        &self,
        items: &[usize],
        path: impl Fn(usize) -> Option<Vec<usize>>,
    ) -> usize {
        let Ok(mut s) = self.0.lock() else {
            return 0;
        };
        let mut added = 0;
        for item in items {
            if !s.queue.iter().any(|o| o.item == *item) && !s.live.owned.contains(item) {
                let mut order = new_order(&s.live, *item);
                order.path = path(*item);
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
            planned: s.plan_ready,
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
        if let Some(line) = s.planned_log.take() {
            write(&line);
            if live.is_some() {
                s.plan_ready = true;
            }
        }
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
        let step = next_step(&cat, &live, &s.queue, s.vanilla_order).map(|(_, step)| step);
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
        // After Start only: before it the player may still switch champion.
        if !upgrade && !s.planned && s.hold.is_empty() {
            s.planned = true;
            return Answer::AskGameFirst;
        }
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
    /// After `Answer::AskGameFirst`: what the game wanted (discarded) and
    /// the build plan's length before and after its decision.
    pub fn game_decided(&self, wanted: Option<usize>, build: (Option<usize>, Option<usize>)) {
        if let Ok(mut s) = self.0.lock() {
            let wanted = wanted.map(|i| {
                s.cat
                    .as_ref()
                    .and_then(|c| c.get(i))
                    .map_or_else(|| i.to_string(), |item| item.key.clone())
            });
            s.planned_log = Some(format!(
                "SHOP DRY RUN game decision ran once for the controlled champion; build {:?} -> {:?} items; game wanted {wanted:?} (discarded)",
                build.0, build.1
            ));
        }
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
    /// Skip the once-per-match game decision (tested on its own below).
    fn plan_done(shop: &Shop) {
        shop.0.lock().unwrap().planned = true;
    }
    #[test]
    fn the_game_decides_once_after_start_before_manual_answers() {
        let shop = Shop::new();
        shop.due((1, 1, 1), 1);
        shop.publish(
            Mode::Prestart {
                controlled: Some(0xa),
                hold: vec![0xa, 0xb],
            },
            Some(riot()),
            live(&[], 500),
            |_| {},
        );
        // Before Start: held, no game decision.
        assert_eq!(shop.new_item_answer(0xa), Answer::Nothing);
        shop.publish(Mode::Manual(0xa), Some(riot()), live(&[], 500), |_| {});
        assert!(!shop.view().unwrap().planned);
        // Upgrade questions never ask the game.
        assert_eq!(shop.upgrade_answer(0xa), Answer::Nothing);
        assert_eq!(shop.new_item_answer(0xa), Answer::AskGameFirst);
        assert_eq!(shop.new_item_answer(0xa), Answer::Nothing);
        // Ready only after a publish that follows the game's decision (it
        // runs in the buyer, after the tick's publish).
        shop.game_decided(Some(0), (Some(4), Some(6)));
        assert!(!shop.view().unwrap().planned);
        let lines = std::sync::Mutex::new(Vec::new());
        shop.publish(Mode::Manual(0xa), Some(riot()), live(&[], 500), |l| {
            lines.lock().unwrap().push(l.to_owned())
        });
        assert!(lines
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.contains("SHOP DRY RUN") && l.contains("Some(4) -> Some(6)")));
        assert!(shop.view().unwrap().planned);
        // A new match asks again.
        shop.due((2, 1, 1), 1);
        shop.publish(Mode::Manual(0xa), Some(riot()), live(&[], 500), |_| {});
        assert_eq!(shop.new_item_answer(0xa), Answer::AskGameFirst);
    }
    fn order(item: usize) -> Order {
        Order {
            item,
            excluded: Vec::new(),
            path: None,
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
        plan_done(&shop);
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
        plan_done(&shop);
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
        let (projected, bought) = project(&cat, &start, &orders, false);
        // Dirk first (400) from the longsword; then flare needs a new chain
        // (equal-cost routes: the iron sword one is planned first):
        // ironsword 250 -> longsword 500; axe 500 is unaffordable at 350 left.
        assert_eq!(bought, vec![2, 0, 1]);
        assert_eq!(projected.gold, 1500 - 400 - 250 - 500);
        assert_eq!(projected.owned, vec![2, 1]);
        // Step by step through the real hooks gives the same purchases.
        let shop = Shop::new();
        shop.due((1, 1, 1), 1);
        plan_done(&shop);
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
        assert_eq!(shop.enqueue_back(&[2, 7, 6], |_| None), 1);
        assert_eq!(shop.view().unwrap().queue, vec![6, 7]);
    }

    #[test]
    fn lock_free_rejection_matches_the_locked_answers() {
        let shop = Shop::new();
        shop.due((1, 1, 1), 1);
        plan_done(&shop);
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
        plan_done(&shop);
        assert_eq!(shop.new_item_answer(0xdef), Answer::Native);
    }

    #[test]
    fn an_unqueued_purchase_is_reported() {
        let shop = Shop::new();
        let lines = std::cell::RefCell::new(Vec::new());
        let w = |l: &str| lines.borrow_mut().push(l.to_owned());
        shop.due((1, 1, 1), 1);
        plan_done(&shop);
        shop.publish(Mode::Manual(0xabc), Some(riot()), live(&[1], 900), w);
        shop.publish(Mode::Manual(0xabc), Some(riot()), live(&[3], 400), w);
        assert!(lines
            .borrow()
            .iter()
            .any(|l| l.starts_with("SHOP UNEXPECTED")));
    }

    #[test]
    fn vanilla_order_waits_for_the_first_order_instead_of_buying_parts_of_later_ones() {
        let cat = riot();
        let me = live(&[1], 450).unwrap(); // longsword; flare's next step (axe) costs 500
        let orders = [order(6), order(7)]; // flare, then boots (250)
                                           // Default rule: the affordable boots are bought now.
        assert_eq!(
            next_step(&cat, &me, &orders, false),
            Some((1, Step::New { item: 7 }))
        );
        // Vanilla order: wait for the axe; nothing for the boots yet.
        assert_eq!(next_step(&cat, &me, &orders, true), None);
        let richer = Live {
            gold: 500,
            ..me.clone()
        };
        assert_eq!(
            next_step(&cat, &richer, &orders, true),
            Some((0, Step::Upgrade { slot: 0, item: 3 }))
        );
        // The projection follows the same rule.
        assert!(project(&cat, &me, &orders, true).1.is_empty());
        assert_eq!(project(&cat, &me, &orders, false).1, vec![7]);
        // The strip shows what will really be bought next.
        assert_eq!(
            upcoming(&cat, &me, &orders, true),
            Some((6, Step::Upgrade { slot: 0, item: 3 }))
        );
        assert_eq!(
            upcoming(&cat, &me, &orders, false),
            Some((7, Step::New { item: 7 }))
        );
        // Nothing affordable: the cheapest next step is the one gold reaches first.
        let broke = Live {
            gold: 100,
            ..me.clone()
        };
        assert_eq!(
            upcoming(&cat, &broke, &orders, false),
            Some((7, Step::New { item: 7 }))
        );
        assert_eq!(
            upcoming(&cat, &broke, &orders, true),
            Some((6, Step::Upgrade { slot: 0, item: 3 }))
        );
    }

    #[test]
    fn a_later_affordable_item_is_not_blocked_and_keeps_off_earlier_parts() {
        let shop = Shop::new();
        shop.due((1, 1, 1), 1);
        plan_done(&shop);
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
        let (after, bought) = project(&cat, &me, &orders, false);
        assert_eq!((after.owned.clone(), bought), (vec![0, 0], vec![0]));
        // The queue is satisfied by the new copy, not the old one.
        assert!(matches!(
            assign(&cat, &after.owned, &orders, false)[0],
            Assigned::Done(1)
        ));
        // Two pending orders of the same part: two new copies.
        let two = vec![new_order(&me, 0), new_order(&me, 0)];
        assert_eq!(project(&cat, &me, &two, false).0.owned, vec![0, 0, 0]);
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

    fn branch_catalogue() -> Arc<Vec<Item>> {
        Arc::new(vec![
            item("sword", 0, 250, &[1]),
            item("pickaxe", 1, 500, &[2, 3]),
            item("scepter", 2, 800, &[4]),
            item("axe", 2, 500, &[4]),
            item("blade", 3, 750, &[]),
        ])
    }

    fn pinned_order(live: &Live, path: &[usize]) -> Order {
        let mut order = new_order(live, *path.last().unwrap());
        order.path = Some(path.to_vec());
        order
    }

    #[test]
    fn chosen_expensive_path_drives_preview_projection_hud_and_buyer() {
        let cat = branch_catalogue();
        let mut me = Live {
            owned: vec![1],
            gold: 1550,
            capacity: 4,
            ..Live::default()
        };
        let path = [0, 1, 2, 4];
        let order = pinned_order(&me, &path);
        assert_eq!(
            offer_with_path(&cat, &me, &[], 4, Some(&path)),
            Offer::Plan(vec![
                Step::Upgrade { slot: 0, item: 2 },
                Step::Upgrade { slot: 0, item: 4 },
            ])
        );
        assert_eq!(
            order_costs(&cat, &me, std::slice::from_ref(&order)),
            vec![Some(1550)]
        );
        assert_eq!(
            upcoming(&cat, &me, std::slice::from_ref(&order), true),
            Some((4, Step::Upgrade { slot: 0, item: 2 }))
        );
        let (after, bought) = project(&cat, &me, std::slice::from_ref(&order), true);
        assert_eq!(after.owned, vec![4]);
        assert_eq!(after.gold, 0);
        assert_eq!(bought, vec![2, 4]);
        // The actual decision hooks consume the identical order as projection.
        let shop = Shop::new();
        shop.due((1, 1, 1), 1);
        plan_done(&shop);
        shop.buy(order);
        for (owned, gold, expected) in [
            (1, 1550, Answer::Upgrade { slot: 0, item: 2 }),
            (2, 750, Answer::Upgrade { slot: 0, item: 4 }),
            (4, 0, Answer::Nothing),
        ] {
            me.owned = vec![owned];
            me.gold = gold;
            shop.publish(
                Mode::Manual(0xabc),
                Some(cat.clone()),
                Some(me.clone()),
                |_| {},
            );
            assert_eq!(shop.upgrade_answer(0xabc), expected);
        }
        assert!(shop.view().unwrap().queue.is_empty());
    }

    #[test]
    fn selected_branch_never_substitutes_an_owned_alternative_when_full() {
        let cat = branch_catalogue();
        let full = Live {
            owned: vec![3],
            gold: 5000,
            capacity: 1,
            ..Live::default()
        };
        let path = [0, 1, 2, 4];
        assert_eq!(
            offer(&cat, &full, &[], 4),
            Offer::Plan(vec![Step::Upgrade { slot: 0, item: 4 }])
        );
        assert_eq!(
            offer_with_path(&cat, &full, &[], 4, Some(&path)),
            Offer::Blocked
        );
        let order = pinned_order(&full, &path);
        assert_eq!(
            next_step(&cat, &full, std::slice::from_ref(&order), false),
            None
        );
        assert_eq!(
            project(&cat, &full, std::slice::from_ref(&order), false),
            (full.clone(), vec![])
        );
        let room = Live {
            capacity: 2,
            ..full
        };
        assert_eq!(
            next_step(&cat, &room, &[order], false),
            Some((0, Step::New { item: 0 }))
        );
    }

    #[test]
    fn chosen_duplicate_orders_claim_components_once_and_keep_independent_paths() {
        let cat = branch_catalogue();
        let me = Live {
            owned: vec![1],
            gold: 5000,
            capacity: 4,
            ..Live::default()
        };
        let orders = [
            pinned_order(&me, &[0, 1, 2, 4]),
            pinned_order(&me, &[0, 1, 3, 4]),
        ];
        assert_eq!(
            order_costs(&cat, &me, &orders),
            vec![Some(1550), Some(2000)]
        );
        for vanilla in [false, true] {
            let (after, bought) = project(&cat, &me, &orders, vanilla);
            assert_eq!(after.owned, vec![4, 4]);
            assert_eq!(after.gold, 1450);
            assert_eq!(bought, vec![2, 4, 0, 1, 3, 4]);
            assert_eq!(
                assign(&cat, &after.owned, &orders, false),
                vec![Assigned::Done(0), Assigned::Done(1)]
            );
        }
    }

    #[test]
    fn invalid_chosen_paths_fail_closed_without_reverting_to_automatic() {
        let cat = branch_catalogue();
        let me = Live {
            gold: 5000,
            capacity: 4,
            ..Live::default()
        };
        for bad in [
            vec![],
            vec![4],
            vec![0, 4],
            vec![999, 4],
            vec![0, 1, 2, 1, 4],
            vec![0, 1, 2],
        ] {
            assert_eq!(
                offer_with_path(&cat, &me, &[], 4, Some(&bad)),
                Offer::Blocked,
                "{bad:?}"
            );
        }
        let mut owned = me.clone();
        owned.owned = vec![1, 2];
        assert_eq!(
            offer_with_path(&cat, &owned, &[], 4, Some(&[0, 1, 2, 4])),
            Offer::Plan(vec![Step::Upgrade { slot: 1, item: 4 }])
        );
    }

    #[test]
    fn whole_build_queue_copies_preferences_and_automatic_orders_stay_unpinned() {
        let cat = branch_catalogue();
        let me = Live {
            gold: 5000,
            capacity: 4,
            ..Live::default()
        };
        assert_eq!(new_order(&me, 4).path, None);
        let shop = Shop::new();
        shop.due((1, 1, 1), 1);
        plan_done(&shop);
        shop.publish(Mode::Manual(0xabc), Some(cat), Some(me), |_| {});
        assert_eq!(shop.enqueue_back(&[4], |_| Some(vec![0, 1, 2, 4])), 1);
        let view = shop.view().unwrap();
        assert_eq!(view.orders[0].path, Some(vec![0, 1, 2, 4]));
        assert_eq!(
            order_costs(&view.cat, &view.live, &view.orders),
            vec![Some(2300)]
        );
    }
}
