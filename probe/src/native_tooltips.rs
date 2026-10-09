//! Owned tooltip requests/results only. Native access lives in the Windows
//! adapter and runs inside the existing viewer borrow, outside this lock.
//! All champion IDs use the same guarded native interface, including Workshop
//! definitions. Unsupported implementations keep the ordinary resolver.
use std::collections::{HashMap, VecDeque};
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

#[derive(Default)]
struct Cache {
    generation: Option<u64>,
    entries: HashMap<Request, Option<String>>,
    pending: VecDeque<Request>,
}
impl Cache {
    fn observe(&mut self, generation: Option<u64>) {
        if self.generation != generation {
            self.generation = generation;
            self.entries.clear();
            self.pending.clear();
        }
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
