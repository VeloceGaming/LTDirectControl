//! Research log (1.5.0-dev13, the author 2026-10-21): which numbers each
//! skill holds by name, read from the game's own skill objects (see
//! `native_adapter/windows/skill_numbers.rs`). The question it answers:
//! do those numbers include the player's database edits and the save's
//! balance patches, so a preview could read its sizes by name instead of
//! the per-version live reader (docs/backlog.md item 2)?
//!
//! The names asked for each skill come from the bundle's `patch_setting`
//! (the fields the database editor and the patch engine change), plus
//! `range` for comparison with the live cast range. A skill whose patch
//! name the rules do not list (Workshop champions) is asked for the common
//! size names instead. A line is written the first time a champion's skill
//! is seen with a given set of values, so background matches add little.
use crate::Logger;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};

/// Patch name -> its patchable field names, from `patch_setting`.
static RULES: OnceLock<BTreeMap<String, Vec<String>>> = OnceLock::new();
/// (champion, slot) -> the last values logged.
static LOGGED: Mutex<Option<HashMap<(String, usize), String>>> = Mutex::new(None);

/// Asked when a skill's patch name has no rules (Workshop champions).
const COMMON: [&str; 16] = [
    "range",
    "attack_range",
    "radius",
    "width",
    "line_width",
    "projectile_radius",
    "explosion_range",
    "area_range",
    "buff_range",
    "heal_range",
    "half_angle_deg",
    "fan_angle",
    "projectile_count",
    "move_distance",
    "move_range",
    "cooltime",
];

/// At start-up, from the bundle pass in `skill_preview`.
pub fn set_rules(setting: &Value, log: &Logger) {
    let rules = parse_rules(setting);
    log.write(&format!("PATCH FIELDS rules kinds={}", rules.len()));
    let _ = RULES.set(rules);
}
fn parse_rules(setting: &Value) -> BTreeMap<String, Vec<String>> {
    setting
        .get("actions")
        .and_then(Value::as_object)
        .map(|actions| {
            actions
                .iter()
                .map(|(kind, fields)| {
                    let names = fields
                        .as_object()
                        .map(|f| f.keys().cloned().collect())
                        .unwrap_or_default();
                    (kind.clone(), names)
                })
                .collect()
        })
        .unwrap_or_default()
}
/// The names to ask a skill with patch name `kind` for: its rules' fields,
/// with `range` first. Skills of champion files without their own patch
/// name (`data_action`, which Workshop champions report; its rules list no
/// sizes) and kinds the rules lack are also asked the common size names
/// (1.5.0-dev14: does the data-driven lookup find sizes in their effects?).
pub fn names_for(kind: &str) -> Vec<String> {
    names_from(RULES.get(), kind)
}
fn names_from(rules: Option<&BTreeMap<String, Vec<String>>>, kind: &str) -> Vec<String> {
    let mut names = vec!["range".to_owned()];
    let fields = rules.and_then(|r| r.get(kind));
    if let Some(fields) = fields {
        names.extend(fields.iter().filter(|f| *f != "range").cloned());
    }
    if fields.is_none() || kind == "data_action" {
        for common in COMMON.iter().skip(1) {
            if !names.iter().any(|n| n == common) {
                names.push(common.to_string());
            }
        }
    }
    names
}
/// Write one line for this skill unless the same values were logged last.
/// `cast_range` is the live cast range in thousandths (patched, read where
/// the preview reads it); `range=` beside it is what the skill object says.
pub fn report(
    champion: &str,
    slot: usize,
    kind: &str,
    cast_range: u64,
    values: Vec<(String, Option<i64>)>,
    log: &Logger,
) {
    let line = describe(kind, cast_range, &values);
    let Ok(mut logged) = LOGGED.lock() else {
        return;
    };
    let logged = logged.get_or_insert_with(HashMap::new);
    let key = (champion.to_owned(), slot);
    if logged.get(&key) == Some(&line) {
        return;
    }
    logged.insert(key, line.clone());
    let slot = ["Q", "W", "R"].get(slot).copied().unwrap_or("?");
    log.write(&format!(
        "PATCH FIELDS champion={champion} slot={slot} {line}"
    ));
}
fn describe(kind: &str, cast_range: u64, values: &[(String, Option<i64>)]) -> String {
    let range = values
        .iter()
        .find(|(name, _)| name == "range")
        .and_then(|(_, v)| *v);
    let check = match range {
        Some(r) if r == cast_range as i64 => "same",
        Some(_) => "differs",
        None => "none",
    };
    let known: Vec<String> = values
        .iter()
        .filter_map(|(name, v)| v.map(|v| format!("{name}={v}")))
        .collect();
    let absent: Vec<&str> = values
        .iter()
        .filter(|(_, v)| v.is_none())
        .map(|(name, _)| name.as_str())
        .collect();
    format!(
        "type={kind} cast_range={cast_range} range_check={check} {} absent=[{}]",
        known.join(" "),
        absent.join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rules_give_each_kind_its_fields_with_range_first() {
        let rules = parse_rules(&json!({"actions": {
            "hammerer_skill2": {"attack": {}, "attack_range": {}, "cooltime": {}},
            "whip_master_ult": {"range": {}, "attack": {}}
        }}));
        assert_eq!(
            names_from(Some(&rules), "hammerer_skill2"),
            ["range", "attack", "attack_range", "cooltime"]
        );
        assert_eq!(
            names_from(Some(&rules), "whip_master_ult"),
            ["range", "attack"]
        );
    }
    #[test]
    fn file_skills_without_their_own_patch_name_are_also_asked_sizes() {
        let rules = parse_rules(&json!({"actions": {
            "data_action": {"range": {}, "cooltime": {}, "damage": {}}
        }}));
        let names = names_from(Some(&rules), "data_action");
        assert_eq!(names[..3], ["range", "cooltime", "damage"]);
        assert!(names.contains(&"radius".to_owned()));
        assert_eq!(names.iter().filter(|n| *n == "cooltime").count(), 1);
    }
    #[test]
    fn unknown_kinds_are_asked_the_common_size_names() {
        let names = names_from(None, "league_annie_skill");
        assert_eq!(names[0], "range");
        assert!(names.contains(&"radius".to_owned()));
        assert_eq!(names.iter().filter(|n| *n == "range").count(), 1);
    }
    #[test]
    fn the_line_compares_range_with_the_live_cast_range() {
        let values = vec![
            ("range".to_owned(), Some(132_000)),
            ("attack_range".to_owned(), None),
            ("cooltime".to_owned(), Some(3000)),
        ];
        assert_eq!(
            describe("whip_master_ult", 132_000, &values),
            "type=whip_master_ult cast_range=132000 range_check=same range=132000 cooltime=3000 absent=[attack_range]"
        );
        assert!(describe("x", 130_000, &values).contains("range_check=differs"));
        assert!(describe("x", 1, &values[1..]).contains("range_check=none"));
    }
}
