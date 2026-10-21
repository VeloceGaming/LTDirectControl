//! Owned tooltip requests/results only. Native access lives in the Windows
//! adapter and runs inside the existing viewer borrow, outside this lock.
//! All champion IDs use the same guarded native interface, including Workshop
//! definitions. Unsupported implementations keep the ordinary resolver.
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

pub fn enabled_for(champion: &str) -> bool {
    !champion.is_empty() && champion.len() <= 512 && !champion.chars().any(char::is_control)
}

const MAX_ENTRIES: usize = 256;

/// Parameters are evidence of unresolved substitution; ordinary punctuation is
/// not. Ignore markup, braces without a closing pair and non-identifier text.
pub(crate) fn unresolved_parameters(text: &str) -> Vec<String> {
    let mut parameters = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        if rest.starts_with('<') {
            if let Some(end) = rest.find('>') {
                rest = &rest[end + 1..];
                continue;
            }
        }
        if rest.starts_with('{') {
            if let Some(end) = rest.find('}') {
                let name = &rest[1..end];
                if name.bytes().next().is_some_and(|c| c.is_ascii_alphabetic())
                    && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
                {
                    parameters.push(name.to_owned());
                }
                rest = &rest[end + 1..];
                continue;
            }
        }
        rest = &rest[rest.chars().next().expect("nonempty text").len_utf8()..];
    }
    parameters.sort();
    parameters.dedup();
    parameters
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Request {
    pub generation: u64,
    pub champion: String,
    pub slot: usize,
    // Localized source text also invalidates cached results after a language
    // change. It is already fetched by the ordinary tooltip resolver.
    template: String,
}

/// An item's effect text, asked of the game's own item text method. Only
/// queued when the SDK finds the exact text `#asset/base/text/item?<key>.option`
/// (see `crate::tooltips::item`): the game's item-specific text methods stop
/// the game when that text sheet is missing.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ItemRequest {
    pub generation: u64,
    pub key: String,
    // The localized template: a language change asks again.
    template: String,
}

#[derive(Default)]
struct Cache {
    generation: Option<u64>,
    entries: HashMap<Request, Option<String>>,
    pending: VecDeque<Request>,
    items: HashMap<ItemRequest, Option<String>>,
    pending_items: Vec<ItemRequest>,
    /// Every item with value slots was queued for this match.
    items_prefetched: bool,
}
impl Cache {
    fn observe(&mut self, generation: Option<u64>) {
        if self.generation != generation {
            self.generation = generation;
            self.entries.clear();
            self.pending.clear();
            self.items.clear();
            self.pending_items.clear();
            self.items_prefetched = false;
        }
    }
    fn claim_item_prefetch(&mut self) -> bool {
        let claim = self.generation.is_some() && !self.items_prefetched;
        self.items_prefetched |= claim;
        claim
    }
    fn item_text(&mut self, key: &str, template: &str) -> Option<String> {
        if !enabled_for(key) {
            return None;
        }
        let request = ItemRequest {
            generation: self.generation?,
            key: key.into(),
            template: template.into(),
        };
        if let Some(value) = self.items.get(&request) {
            return value.clone();
        }
        // One request per item, template and match, failed ones included.
        if self.items.len() >= MAX_ENTRIES {
            self.items.clear();
            self.pending_items.clear();
        }
        self.items.insert(request.clone(), None);
        self.pending_items.push(request);
        None
    }
    fn take_items(&mut self, generation: u64) -> Vec<ItemRequest> {
        if self.generation == Some(generation) {
            std::mem::take(&mut self.pending_items)
        } else {
            Vec::new()
        }
    }
    fn publish_item(&mut self, request: ItemRequest, result: Option<String>) -> bool {
        let stored =
            self.generation == Some(request.generation) && self.items.contains_key(&request);
        if stored {
            self.items.insert(request, result);
        }
        stored
    }
    fn description(&mut self, champion: &str, slot: usize, template: &str) -> Option<String> {
        if !enabled_for(champion) || slot >= 3 {
            return None;
        }
        let key = Request {
            generation: self.generation?,
            champion: champion.into(),
            slot,
            template: template.into(),
        };
        if let Some(value) = self.entries.get(&key) {
            return value.clone();
        }
        // One request per skill/source/session, including failed requests.
        // Bound cache growth if localization is repeatedly changed mid-match.
        if self.entries.len() >= MAX_ENTRIES {
            self.entries.clear();
            self.pending.clear();
        }
        self.entries.insert(key.clone(), None);
        self.pending.push_back(key);
        None
    }
    fn take(&mut self, generation: u64) -> Option<Request> {
        (self.generation == Some(generation))
            .then(|| self.pending.pop_front())
            .flatten()
    }
    fn publish(&mut self, request: Request, result: Option<String>) {
        if self.generation == Some(request.generation) && self.entries.contains_key(&request) {
            self.entries.insert(request, result);
        }
    }
}
static CACHE: Mutex<Option<Cache>> = Mutex::new(None);
/// Rises each time a game item text arrives, so tooltip texts stored
/// before it (with "…") are built again.
static ITEM_REVISION: AtomicU64 = AtomicU64::new(0);

pub fn observe(generation: u64, controls: bool) {
    if let Ok(mut cache) = CACHE.lock() {
        cache
            .get_or_insert_with(Cache::default)
            .observe(controls.then_some(generation));
    }
}
pub fn description(champion: &str, slot: usize, template: &str) -> Option<String> {
    CACHE
        .lock()
        .ok()?
        .as_mut()?
        .description(champion, slot, template)
}
/// The game's effect text for an item, once resolved (None while waiting
/// or when the game gave none).
pub fn item_text(key: &str, template: &str) -> Option<String> {
    CACHE.lock().ok()?.as_mut()?.item_text(key, template)
}
/// Every item waiting: resolved together, from one list of the game's items.
pub(crate) fn take_items(generation: u64) -> Vec<ItemRequest> {
    CACHE
        .lock()
        .ok()
        .and_then(|mut cache| cache.as_mut().map(|c| c.take_items(generation)))
        .unwrap_or_default()
}
pub(crate) fn publish_item(request: ItemRequest, result: Option<String>) {
    let arrived = result.is_some();
    if let Ok(mut cache) = CACHE.lock() {
        if let Some(cache) = cache.as_mut() {
            if cache.publish_item(request, result) && arrived {
                ITEM_REVISION.fetch_add(1, Ordering::Release);
            }
        }
    }
}
/// See `ITEM_REVISION`.
pub fn item_revision() -> u64 {
    ITEM_REVISION.load(Ordering::Acquire)
}
/// True once per match: the caller then queues every item with value
/// slots, so the game's item list is built once for all of them.
pub fn claim_item_prefetch() -> bool {
    CACHE
        .lock()
        .ok()
        .and_then(|mut cache| cache.as_mut().map(Cache::claim_item_prefetch))
        .unwrap_or(false)
}
pub(crate) fn take(generation: u64) -> Option<Request> {
    CACHE.lock().ok()?.as_mut()?.take(generation)
}
pub(crate) fn publish(request: Request, result: Option<String>) {
    if let Ok(mut cache) = CACHE.lock() {
        if let Some(cache) = cache.as_mut() {
            cache.publish(request, result);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn request_is_deduplicated_and_owned_result_refreshes_the_open_tooltip() {
        let mut cache = Cache::default();
        cache.observe(Some(1));
        assert_eq!(cache.description("illusionist", 0, "Taunt {Time}"), None);
        assert_eq!(cache.description("illusionist", 0, "Taunt {Time}"), None);
        let request = cache.take(1).unwrap();
        assert!(cache.take(1).is_none());
        cache.publish(request, Some("Taunt 1.5".into()));
        assert_eq!(
            cache.description("illusionist", 0, "Taunt {Time}"),
            Some("Taunt 1.5".into())
        );
    }
    #[test]
    fn items_queue_once_resolve_together_and_never_outlive_the_match() {
        let mut cache = Cache::default();
        assert_eq!(
            cache.item_text("giants_horn_shard", "Regen {RegenFlat}"),
            None
        );
        cache.observe(Some(1));
        assert_eq!(
            cache.item_text("giants_horn_shard", "Regen {RegenFlat}"),
            None
        );
        assert_eq!(
            cache.item_text("giants_horn_shard", "Regen {RegenFlat}"),
            None
        );
        assert_eq!(
            cache.item_text("impregnable_fortress", "{Flat} + {Ratio}%"),
            None
        );
        assert!(cache.take_items(2).is_empty());
        assert!(cache.claim_item_prefetch());
        assert!(!cache.claim_item_prefetch());
        let requests = cache.take_items(1);
        assert_eq!(requests.len(), 2);
        assert!(cache.take_items(1).is_empty());
        let (shard, fortress) = (requests[0].clone(), requests[1].clone());
        cache.publish_item(shard, Some("Regen 40".into()));
        cache.publish_item(fortress, None);
        assert_eq!(
            cache.item_text("giants_horn_shard", "Regen {RegenFlat}"),
            Some("Regen 40".into())
        );
        // A failed item is not asked again in this match.
        assert_eq!(
            cache.item_text("impregnable_fortress", "{Flat} + {Ratio}%"),
            None
        );
        assert!(cache.take_items(1).is_empty());
        // Another language asks again; a new match forgets everything.
        assert_eq!(
            cache.item_text("giants_horn_shard", "回復 {RegenFlat}"),
            None
        );
        assert_eq!(cache.take_items(1).len(), 1);
        let late = ItemRequest {
            generation: 1,
            key: "giants_horn_shard".into(),
            template: "Regen {RegenFlat}".into(),
        };
        cache.observe(Some(2));
        assert!(cache.claim_item_prefetch());
        assert!(!cache.publish_item(late, Some("Late".into())));
        assert_eq!(
            cache.item_text("giants_horn_shard", "Regen {RegenFlat}"),
            None
        );
        assert!(!enabled_for(""));
        cache.observe(None);
        assert_eq!(
            cache.item_text("giants_horn_shard", "Regen {RegenFlat}"),
            None
        );
        assert!(cache.pending_items.is_empty());
        assert!(!cache.claim_item_prefetch());
    }
    #[test]
    fn language_and_match_changes_cannot_reuse_stale_text() {
        let mut cache = Cache::default();
        cache.observe(Some(1));
        cache.description("illusionist", 0, "English");
        let old = cache.take(1).unwrap();
        cache.publish(old.clone(), Some("Old text".into()));
        assert_eq!(cache.description("illusionist", 0, "中文"), None);
        assert_eq!(cache.take(1).unwrap().template, "中文");
        cache.observe(Some(2));
        cache.publish(old, Some("Late old result".into()));
        assert_eq!(cache.description("illusionist", 0, "English"), None);
        assert!(cache.take(1).is_none());
        assert_eq!(cache.take(2).unwrap().generation, 2);
        cache.observe(None);
        assert_eq!(cache.description("illusionist", 0, "English"), None);
        assert!(cache.pending.is_empty());
        assert!(cache.entries.is_empty());
    }
    #[test]
    fn failed_optional_skills_do_not_retry_every_frame_or_affect_other_champions() {
        let mut cache = Cache::default();
        cache.observe(Some(1));
        cache.description("alchemist", 1, "Optional skill");
        let request = cache.take(1).unwrap();
        cache.publish(request, None);
        assert_eq!(cache.description("alchemist", 1, "Optional skill"), None);
        assert_eq!(cache.description("alchemist", 3, "Invalid slot"), None);
        assert!(cache.take(1).is_none());
        assert_eq!(cache.description("ghost", 0, "Ghost Q"), None);
        assert_eq!(cache.take(1).unwrap().champion, "ghost");
    }
    #[test]
    fn every_base_champion_and_workshop_ids_can_queue_without_evicting_base_coverage() {
        let champions: serde_json::Value =
            serde_json::from_str(include_str!("tooltip_assets.json")).unwrap();
        let mut cache = Cache::default();
        cache.observe(Some(1));
        for champion in champions.as_object().unwrap().keys() {
            for slot in 0..3 {
                cache.description(champion, slot, "Localized template");
            }
        }
        let expected = champions.as_object().unwrap().len() * 3;
        assert_eq!(cache.entries.len(), expected);
        assert_eq!(cache.pending.len(), expected);
        cache.description("workshop/new_champion", 0, "No SDK template: locale stamp");
        assert_eq!(cache.entries.len(), expected + 1);
        assert!(!enabled_for(""));
        assert!(!enabled_for("broken\nidentifier"));
        assert!(!enabled_for(&"x".repeat(513)));
    }
    #[test]
    fn coverage_keeps_placeholders_distinct_from_ellipsis_and_markup() {
        assert_eq!(
            unresolved_parameters("60 + {Coef}% ... 等待… {Time} {Coef} <i#asset/{Icon}:x>"),
            vec!["Coef", "Time"]
        );
        assert!(unresolved_parameters("... … {literal text} {123} {unfinished").is_empty());
        assert_eq!(
            unresolved_parameters("中文 {DotTick} <#ffffffff>{CloudRadius}<>{TimeCoef}"),
            vec!["CloudRadius", "DotTick", "TimeCoef"]
        );
    }
}
