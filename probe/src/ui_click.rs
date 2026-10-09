//! Native click callbacks registered by path survive our window being
//! removed and rebuilt within a match (background colour or language
//! change), and the rebuild registers again: the game then calls both
//! callbacks for one click, so a toggle (a select menu, a checkbox, camera
//! lock) opens and closes in the same instant. Callbacks pass through
//! `once`, which lets a node's click through only once per 50 ms.
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

static LAST: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);

/// True for the first callback of a click on `path`; false for duplicates.
pub fn once(path: &str) -> bool {
    let Ok(mut last) = LAST.lock() else {
        return true;
    };
    let now = Instant::now();
    let map = last.get_or_insert_with(HashMap::new);
    if map
        .get(path)
        .is_some_and(|t| now.duration_since(*t) < Duration::from_millis(50))
    {
        return false;
    }
    map.insert(path.to_owned(), now);
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_second_callback_for_the_same_click_is_dropped() {
        assert!(super::once("test.node"));
        assert!(!super::once("test.node"));
        assert!(super::once("test.other"));
    }
}
