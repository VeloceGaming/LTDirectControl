//! Read-only automatic build forecast. Never changes builds, gold, or RNG.
use crate::player_hud::PlayerHud;
use mod_api_stable::{StableItemBuildContext, StableItemBuildHook};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub struct BuildObserver(pub Arc<PlayerHud>);
impl StableItemBuildHook for BuildObserver {
    fn id(&self) -> String {
        "lt_direct_control_item_observer".into()
    }
    fn priority(&self) -> i32 {
        i32::MAX
    }
    fn decide_build(&self, ctx: &StableItemBuildContext<'_>) -> Vec<usize> {
        self.0
            .item_catalog(ctx.item_keys().into_iter().map(str::to_owned).collect());
        Vec::new()
    }
}

#[derive(Clone, Debug)]
pub struct Item {
    pub key: String,
    pub name: String,
    pub price: usize,
    pub tier: usize,
    pub enabled: bool,
    pub next: Vec<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Purchase {
    pub key: String,
    pub price: usize,
}
#[derive(Clone, Debug)]
pub struct Forecast {
    pub target: String,
    pub next: Option<Purchase>,
    pub affordable: Vec<Purchase>,
    pub branching: bool,
    pub completed: bool,
    pub alternatives: Vec<Purchase>,
}
pub fn metadata(table: &HashMap<String, serde_json::Value>) -> HashMap<String, Item> {
    table
        .iter()
        .filter_map(|(alias, spec)| {
            let key = spec
                .get("key")
                .and_then(|v| v.as_str())
                .unwrap_or(alias)
                .to_owned();
            let item = Item {
                key: key.clone(),
                name: spec
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(alias)
                    .to_owned(),
                price: usize::try_from(spec.get("price")?.as_u64()?).ok()?,
                tier: usize::try_from(spec.get("tier")?.as_u64()?).ok()?,
                enabled: spec
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
                next: spec
                    .get("next_tier")
                    .and_then(|v| v.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect(),
            };
            Some((key, item))
        })
        .collect()
}
fn paths(
    items: &HashMap<String, Item>,
    from: &str,
    to: &str,
    seen: &mut HashSet<String>,
) -> Vec<Vec<String>> {
    if from == to {
        return vec![vec![from.into()]];
    }
    if seen.len() >= 16 || !seen.insert(from.into()) {
        return Vec::new();
    }
    let mut result = Vec::new();
    if let Some(item) = items.get(from).filter(|i| i.enabled) {
        for next in &item.next {
            for mut path in paths(items, next, to, seen) {
                path.insert(0, from.into());
                result.push(path);
                if result.len() >= 64 {
                    break;
                }
            }
            if result.len() >= 64 {
                break;
            }
        }
    }
    seen.remove(from);
    result
}
/// Game buying completes the last inventory slot before advancing to the
/// next enabled final build target. Unrelated starter/special items do not
/// consume a final-build slot. Tier numbers do not identify special items. A random branch has no fixed future answer.
pub fn forecast(
    items: &HashMap<String, Item>,
    build: &[String],
    owned: &[String],
    gold: usize,
) -> Option<Forecast> {
    // Missing metadata must not silently remove an inventory/build slot.
    if build
        .iter()
        .chain(owned)
        .any(|key| !items.contains_key(key))
    {
        return None;
    }
    let targets = build
        .iter()
        .filter_map(|key| items.get(key))
        .filter(|i| i.enabled)
        .collect::<Vec<_>>();
    let mut inventory = owned
        .iter()
        .filter_map(|key| items.get(key))
        .filter(|i| {
            targets
                .iter()
                .any(|t| !paths(items, &i.key, &t.key, &mut HashSet::new()).is_empty())
        })
        .map(|i| i.key.clone())
        .collect::<Vec<_>>();
    if targets.is_empty() {
        return None;
    }
    // Final slots must match the assigned build in order. Refuse a forecast
    // for inconsistent inventory rather than presenting a false completion.
    if inventory.len() > targets.len()
        || inventory.iter().enumerate().any(|(slot, key)| {
            let target = &targets[slot].key;
            (slot + 1 < inventory.len() && key != target)
                || paths(items, key, target, &mut HashSet::new()).is_empty()
        })
    {
        return None;
    }
    let mut remaining = gold;
    let mut affordable = Vec::new();
    let mut first = None;
    let mut target_key = String::new();
    let mut branching = false;
    let mut completed = false;
    let mut alternatives = Vec::new();
    for _ in 0..32 {
        let mut slot = inventory.len();
        let upgrading = inventory
            .last()
            .zip(targets.get(slot.saturating_sub(1)))
            .is_some_and(|(key, t)| key != &t.key);
        if upgrading {
            slot -= 1;
        }
        let Some(target) = targets.get(slot) else {
            completed = true;
            break;
        };
        if target_key.is_empty() {
            target_key = target.key.clone();
        }
        let options = if upgrading {
            paths(items, &inventory[slot], &target.key, &mut HashSet::new())
                .into_iter()
                .filter_map(|p| p.get(1).cloned())
                .collect::<HashSet<_>>()
        } else {
            items
                .values()
                .filter(|i| {
                    i.enabled
                        && (i.tier == 0
                            || !items.values().any(|p| p.enabled && p.next.contains(&i.key)))
                })
                .filter(|i| !paths(items, &i.key, &target.key, &mut HashSet::new()).is_empty())
                .map(|i| i.key.clone())
                .collect::<HashSet<_>>()
        };
        if options.len() != 1 {
            branching = true;
            alternatives = options
                .iter()
                .filter_map(|key| {
                    items.get(key).map(|i| Purchase {
                        key: key.clone(),
                        price: i.price,
                    })
                })
                .collect();
            alternatives.sort_by(|a, b| a.key.cmp(&b.key));
            break;
        }
        let key = options.into_iter().next()?;
        let purchase = Purchase {
            price: items.get(&key)?.price,
            key: key.clone(),
        };
        if first.is_none() {
            first = Some(purchase.clone());
        }
        if remaining < purchase.price {
            break;
        }
        remaining -= purchase.price;
        affordable.push(purchase);
        if upgrading {
            inventory[slot] = key;
        } else {
            inventory.push(key);
        }
    }
    let completed = completed && first.is_none();
    Some(Forecast {
        target: target_key,
        next: first,
        affordable,
        branching,
        completed,
        alternatives,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn data() -> HashMap<String, Item> {
        [
            ("blade", 500, 0, vec!["long"]),
            ("long", 600, 1, vec!["final"]),
            ("final", 800, 3, vec![]),
            ("starter", 0, 4, vec![]),
        ]
        .into_iter()
        .map(|(k, p, t, n)| {
            (
                k.into(),
                Item {
                    key: k.into(),
                    name: k.into(),
                    price: p,
                    tier: t,
                    enabled: true,
                    next: n.into_iter().map(str::to_owned).collect(),
                },
            )
        })
        .collect()
    }
    #[test]
    fn radiant_and_custom_tier_four_finals_complete_in_assigned_order() {
        let mut items = data();
        items.get_mut("final").unwrap().next = vec!["radiant".into()];
        items.insert(
            "radiant".into(),
            Item {
                key: "radiant".into(),
                name: "radiant".into(),
                price: 1000,
                tier: 4,
                enabled: true,
                next: vec![],
            },
        );
        items.insert(
            "custom".into(),
            Item {
                key: "custom".into(),
                name: "custom".into(),
                price: 2000,
                tier: 4,
                enabled: true,
                next: vec![],
            },
        );
        let build = vec!["radiant".into(), "custom".into()];
        let f = forecast(&items, &build, &["starter".into(), "final".into()], 999).unwrap();
        assert_eq!(f.target, "radiant");
        assert_eq!(f.next.unwrap().price, 1000);
        assert!(f.affordable.is_empty());
        assert!(!f.completed);
        let f = forecast(&items, &build, &["radiant".into()], 2000).unwrap();
        assert_eq!(f.target, "custom");
        assert_eq!(f.next.unwrap().key, "custom");
        assert_eq!(f.affordable.len(), 1);
        assert!(
            forecast(&items, &build, &["radiant".into(), "custom".into()], 0)
                .unwrap()
                .completed
        );
        assert!(forecast(&items, &build, &["final".into(), "radiant".into()], 9999).is_none());
    }
    #[test]
    fn radiant_branch_reports_alternatives_without_choosing_or_advancing_rng() {
        let mut items = data();
        items.get_mut("final").unwrap().tier = 4;
        items
            .get_mut("blade")
            .unwrap()
            .next
            .push("alternate".into());
        items.insert(
            "alternate".into(),
            Item {
                key: "alternate".into(),
                name: "alternate".into(),
                price: 400,
                tier: 2,
                enabled: true,
                next: vec!["final".into()],
            },
        );
        let f = forecast(&items, &["final".into()], &["blade".into()], 500).unwrap();
        assert!(f.next.is_none());
        assert!(f.branching);
        assert!(!f.completed);
        assert_eq!(
            f.alternatives.iter().map(|p| p.price).collect::<Vec<_>>(),
            [400, 600]
        );
    }
    #[test]
    fn upgrades_use_incremental_prices_and_complete_last_slot() {
        let plan = vec!["final".into(), "final".into()];
        let f = forecast(&data(), &plan, &["starter".into(), "blade".into()], 1400).unwrap();
        assert_eq!(f.next.unwrap().price, 600);
        assert_eq!(
            f.affordable
                .iter()
                .map(|p| p.key.as_str())
                .collect::<Vec<_>>(),
            vec!["long", "final"]
        );
        assert!(!f.completed);
        assert!(!f.branching);
        let entire = forecast(&data(), &["final".into()], &["blade".into()], 1400).unwrap();
        assert!(!entire.completed); // affordability is not an already purchased item
        assert_eq!(entire.affordable.len(), 2);
    }
    #[test]
    fn random_branch_is_not_presented_as_an_exact_purchase() {
        let mut items = data();
        items.insert(
            "other".into(),
            Item {
                key: "other".into(),
                name: "other".into(),
                price: 200,
                tier: 0,
                enabled: true,
                next: vec!["final".into()],
            },
        );
        let f = forecast(&items, &["final".into()], &[], 1000).unwrap();
        assert!(f.branching);
        assert!(f.next.is_none());
        assert!(f.affordable.is_empty());
    }
    #[test]
    fn complete_build_and_unknown_mod_metadata_do_not_invent_costs() {
        assert!(forecast(&data(), &["missing".into()], &[], 500).is_none());
        let f = forecast(&data(), &["final".into()], &["final".into()], 999).unwrap();
        assert!(f.completed);
        assert!(f.next.is_none());
    }
    #[test]
    fn effective_mod_prices_and_disabled_branches_are_respected() {
        let table = HashMap::from([
            (
                "base".into(),
                serde_json::json!({"price":250,"tier":0,"next_tier":["upgrade"]}),
            ),
            (
                "upgrade".into(),
                serde_json::json!({"price":400,"tier":2,"next_tier":["added-final"]}),
            ),
            (
                "added-final".into(),
                serde_json::json!({"price":2000,"tier":3,"next_tier":[]}),
            ),
            (
                "disabled".into(),
                serde_json::json!({"price":1,"tier":0,"enabled":false,"next_tier":["added-final"]}),
            ),
        ]);
        let items = metadata(&table);
        let f = forecast(&items, &["added-final".into()], &["base".into()], 399).unwrap();
        assert_eq!(f.next.unwrap().price, 400);
        assert!(f.affordable.is_empty());
        let f = forecast(&items, &["added-final".into()], &["base".into()], 2400).unwrap();
        assert_eq!(
            f.affordable.iter().map(|p| p.price).collect::<Vec<_>>(),
            [400, 2000]
        );
        let f = forecast(&items, &["added-final".into()], &[], 249).unwrap();
        assert_eq!(f.next.unwrap().key, "base");
        assert!(!f.branching);
    }
}
