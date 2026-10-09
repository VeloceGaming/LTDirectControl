//! Attack-move reach, independent of AA reach. Only owned numbers cross threads.
//! No disk/SDK calls occur in the simulation's target-acquisition path.
use crate::{settings::Values, Logger};
use mod_api_stable::{SettingTargetV1, StableClient};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

pub const BASELINE: f64 = 120.;
pub const BUFFER: f64 = 5.;
pub const MAX_RADIUS: f64 = 10_000.;

#[derive(Clone, Default)]
pub struct Champion {
    pub name: String,
    pub label: String,
    pub aliases: Vec<String>,
    pub base: Option<u64>,
    pub growth: u64,
    pub maximum: Option<u64>,
    pub current: Option<u64>,
}
#[derive(Default)]
struct Catalog {
    all: BTreeMap<String, Champion>,
    active: Vec<String>,
    controlled: Option<String>,
    generation: Option<u64>,
    level_cap: Option<u64>,
    refresh_at: Option<Instant>,
}
static CATALOG: OnceLock<Mutex<Catalog>> = OnceLock::new();
fn catalog() -> &'static Mutex<Catalog> {
    CATALOG.get_or_init(Mutex::default)
}

fn declaration(name: &str, v: &Value) -> Champion {
    Champion {
        name: name.into(),
        base: v
            .get("range")
            .and_then(Value::as_u64)
            .filter(|r| *r <= 10_000_000),
        growth: v.get("growth_range").and_then(Value::as_u64).unwrap_or(0),
        ..Champion::default()
    }
}
pub fn initialize(log: &Logger) {
    let base: Value =
        serde_json::from_str(include_str!("acquisition_defaults.json")).expect("AA declarations");
    if let Ok(mut c) = catalog().lock() {
        for (name, value) in base.as_object().expect("AA declaration map") {
            c.all.insert(name.clone(), declaration(name, value));
        }
        for (name, value) in crate::hud_icons::enabled_assets().descriptions {
            if let Some(attack) = value.get("attack") {
                c.all.insert(name.clone(), declaration(&name, attack));
            }
        }
        let aliases: Value = serde_json::from_str(include_str!("acquisition_names.json"))
            .expect("champion name aliases");
        for (name, values) in aliases.as_object().expect("name map") {
            if let Some(row) = c.all.get_mut(name) {
                row.aliases = values
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect();
            }
        }
        if let Some(game) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(std::path::Path::to_owned))
        {
            for (_, root) in crate::hud_icons::enabled_roots(&game) {
                read_mod_names(&root, 0, &mut 0, &mut c.all);
            }
        }
        log.write(&format!(
            "ACQUISITION declarations={}; baseline={BASELINE} buffer={BUFFER}; native growth preferred",
            c.all.len()
        ));
    }
}
/// Once per session; retry a loading host at most once every two seconds.
pub fn refresh(ctx: &StableClient<'_>, generation: u64) {
    let ready = catalog().lock().is_ok_and(|c| {
        c.generation != Some(generation)
            || (c.active.is_empty() || c.level_cap.is_none())
                && c.refresh_at
                    .is_none_or(|t| t.elapsed() >= Duration::from_secs(2))
    });
    if !ready {
        return;
    }
    let mut names = ctx.champion_names();
    names.sort();
    names.dedup();
    let cap = ctx
        .setting_get_json(SettingTargetV1::GameSetting, "need_exp")
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| level_cap(&v));
    if let Ok(mut c) = catalog().lock() {
        if c.generation != Some(generation) {
            c.controlled = None;
            for row in c.all.values_mut() {
                row.current = None;
                row.maximum = None;
            }
        }
        c.generation = Some(generation);
        c.refresh_at = Some(Instant::now());
        c.level_cap = cap;
        c.active = names;
        let active = c.active.clone();
        for name in active {
            let label = crate::tooltips::champion_name(ctx, &name);
            let row = c.all.entry(name.clone()).or_insert_with(|| Champion {
                name,
                ..Champion::default()
            });
            row.label = label;
            row.maximum = row.base.and_then(|base| maximum(base, row.growth, cap));
        }
    }
}
fn add_names(v: &Value, rows: &mut BTreeMap<String, Champion>) {
    for locale in v.as_object().into_iter().flat_map(|v| v.values()) {
        for (id, description) in locale
            .get("description")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            if let Some(name) = description.get("name").and_then(Value::as_str) {
                let Some(row) = rows.get_mut(id) else {
                    continue;
                };
                if !row.aliases.iter().any(|a| a == name) {
                    row.aliases.push(name.into());
                }
            }
        }
    }
}
fn read_mod_names(
    root: &std::path::Path,
    depth: usize,
    count: &mut usize,
    rows: &mut BTreeMap<String, Champion>,
) {
    if depth > 8 || *count >= 512 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if *count >= 512 {
            break;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            read_mod_names(&entry.path(), depth + 1, count, rows);
        } else if entry.path().extension().is_some_and(|x| x == "i18n") {
            *count += 1;
            if let Some(v) = crate::hud_icons::json(&entry.path()) {
                add_names(&v, rows);
            }
        }
    }
}
pub fn matches(row: &Champion, query: &str) -> bool {
    fn normalize(s: &str) -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    }
    let q = normalize(query);
    std::iter::once(&row.name)
        .chain(std::iter::once(&row.label))
        .chain(row.aliases.iter())
        .any(|s| normalize(s).contains(&q))
}

/// Upgrade generated defaults once; inherited minima follow the shared setting.
pub fn migrate_defaults(v: &mut Values) {
    let Some(a) = v.0.get_mut("acquisition").and_then(Value::as_object_mut) else {
        return;
    };
    let version = a
        .get("defaults_version")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if version >= 3 {
        return;
    }
    if version < 2 {
        if a.get("baseline_radius").and_then(Value::as_f64) == Some(120.) {
            a.insert("baseline_radius".into(), json!(BASELINE));
        }
        if a.get("aa_range_buffer").and_then(Value::as_f64) == Some(10.) {
            a.insert("aa_range_buffer".into(), json!(BUFFER));
        }
    }
    if let Some(rows) = a.get_mut("champions").and_then(Value::as_object_mut) {
        for row in rows.values_mut().filter_map(Value::as_object_mut) {
            row.remove("default_baseline_radius");
        }
    }
    a.insert("defaults_version".into(), json!(3));
}
fn level_cap(v: &Value) -> Option<u64> {
    let a = v.as_array()?;
    (!a.is_empty() && a.len() < 100 && a.iter().all(|n| n.as_u64().is_some_and(|n| n > 0)))
        .then_some(a.len() as u64 + 1)
}
pub fn maximum(base: u64, growth: u64, cap: Option<u64>) -> Option<u64> {
    let level = if growth == 0 { 1 } else { cap? };
    let r = base.checked_add(growth.checked_mul(level.checked_sub(1)?)?)?;
    (r <= 10_000_000).then_some(r)
}
pub fn cap() -> Option<u64> {
    catalog().lock().ok()?.level_cap
}
pub fn observe(name: &str, current: u64, maximum: Option<u64>) {
    if let Ok(mut c) = catalog().lock() {
        if c.controlled.as_deref() != Some(name) {
            for row in c.all.values_mut() {
                row.current = None;
            }
            c.controlled = Some(name.into());
        }
        if !c.all.contains_key(name) {
            c.all.insert(
                name.into(),
                Champion {
                    name: name.into(),
                    ..Champion::default()
                },
            );
        }
        let row = c.all.get_mut(name).expect("observed champion");
        row.current = Some(current);
        // Unknown native scaling must not reuse an unverified file maximum.
        row.maximum = maximum;
    }
}
pub fn controlled() -> Option<String> {
    catalog().lock().ok()?.controlled.clone()
}
pub fn champions() -> Vec<Champion> {
    catalog()
        .lock()
        .map(|c| {
            if c.active.is_empty() {
                c.all.values().cloned().collect()
            } else {
                c.active
                    .iter()
                    .filter_map(|n| c.all.get(n).cloned())
                    .collect()
            }
        })
        .unwrap_or_default()
}
pub fn champion(name: &str) -> Champion {
    catalog()
        .lock()
        .ok()
        .and_then(|c| c.all.get(name).cloned())
        .unwrap_or_else(|| Champion {
            name: name.into(),
            ..Champion::default()
        })
}
fn valid(n: Option<f64>, fallback: f64) -> f64 {
    n.filter(|n| n.is_finite() && (0. ..=MAX_RADIUS).contains(n))
        .unwrap_or(fallback)
}
pub fn baseline(v: &Values, _name: &str) -> f64 {
    valid(
        v.0.pointer("/acquisition/baseline_radius")
            .and_then(Value::as_f64),
        BASELINE,
    )
}
pub fn set_shared(v: &mut Values, minimum: bool, value: f64) {
    if !v.0["acquisition"].is_object() {
        v.0["acquisition"] = json!({});
    }
    let key = if minimum {
        "baseline_radius"
    } else {
        "aa_range_buffer"
    };
    v.0["acquisition"][key] = json!(valid(Some(value), if minimum { BASELINE } else { BUFFER }));
}
pub fn custom(v: &Values, name: &str) -> Option<f64> {
    v.0.pointer("/acquisition/champions")?
        .get(name)?
        .get("override_radius")?
        .as_f64()
        .filter(|n| n.is_finite() && (0. ..=MAX_RADIUS).contains(n))
}
pub fn buffer(v: &Values) -> f64 {
    valid(
        v.0.pointer("/acquisition/aa_range_buffer")
            .and_then(Value::as_f64),
        BUFFER,
    )
}
pub fn set_custom(v: &mut Values, name: &str, radius: Option<f64>) {
    if !v.0["acquisition"].is_object() {
        v.0["acquisition"] = json!({});
    }
    if !v.0["acquisition"]["champions"].is_object() {
        v.0["acquisition"]["champions"] = json!({});
    }
    let row = &mut v.0["acquisition"]["champions"][name];
    if !row.is_object() {
        *row = json!({});
    }
    row["override_radius"] = json!(radius.map(|n| valid(Some(n), BASELINE)));
}
pub fn register_defaults(v: &mut Values, rows: &[Champion]) {
    if rows.is_empty() {
        return;
    }
    for row in rows {
        if v.0
            .pointer("/acquisition/champions")
            .and_then(|c| c.get(&row.name))
            .is_none()
        {
            set_custom(v, &row.name, None);
        }
        if let Some(entry) =
            v.0.pointer_mut("/acquisition/champions")
                .and_then(|c| c.get_mut(&row.name))
                .and_then(Value::as_object_mut)
        {
            entry.remove("default_baseline_radius");
            entry.insert(
                "maximum_scaling_aa_range".into(),
                json!(row.maximum.map(|n| n as f64 / 1000.)),
            );
        }
    }
    v.0["acquisition"]["baseline_radius"] = json!(valid(
        v.0.pointer("/acquisition/baseline_radius")
            .and_then(Value::as_f64),
        BASELINE
    ));
    v.0["acquisition"]["aa_range_buffer"] = json!(valid(
        v.0.pointer("/acquisition/aa_range_buffer")
            .and_then(Value::as_f64),
        BUFFER
    ));
    v.0["acquisition"]["defaults_version"] = json!(3);
}
pub fn radius(v: &Values, name: &str, current: u64, maximum: Option<u64>) -> u64 {
    if let Some(n) = custom(v, name) {
        return ((n * 1000.).round() as u64).max(current);
    }
    let buffer = (buffer(v) * 1000.).round() as u64;
    let baseline = (baseline(v, name) * 1000.).round() as u64;
    baseline.max(
        maximum
            .unwrap_or(current)
            .max(current)
            .saturating_add(buffer),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_defaults_migrate_once_without_rewriting_custom_radii() {
        let mut v = Values(
            json!({"acquisition":{"baseline_radius":120,"aa_range_buffer":10,"champions":{"soldier":{"default_baseline_radius":120,"override_radius":135},"mod":{"default_baseline_radius":82,"override_radius":45}}}}),
        );
        migrate_defaults(&mut v);
        assert_eq!(baseline(&v, "soldier"), 120.);
        assert_eq!(baseline(&v, "mod"), 120.);
        assert_eq!(custom(&v, "soldier"), Some(135.));
        assert_eq!(custom(&v, "mod"), Some(45.));
        assert_eq!(v.0["acquisition"]["aa_range_buffer"], 5.);
        v.0["acquisition"]["baseline_radius"] = json!(120);
        migrate_defaults(&mut v);
        assert_eq!(v.0["acquisition"]["baseline_radius"], 120);
        let mut v = Values(json!({"acquisition":{"baseline_radius":88,"aa_range_buffer":7}}));
        migrate_defaults(&mut v);
        assert_eq!(v.0["acquisition"]["baseline_radius"], 88);
        assert_eq!(v.0["acquisition"]["aa_range_buffer"], 7);
        let mut v = Values(
            json!({"acquisition":{"defaults_version":3,"baseline_radius":70,"aa_range_buffer":9,"champions":{"ghost":{"override_radius":82}}}}),
        );
        migrate_defaults(&mut v);
        register_defaults(
            &mut v,
            &[Champion {
                name: "ghost".into(),
                ..Champion::default()
            }],
        );
        assert_eq!(baseline(&v, "ghost"), 70.);
        assert_eq!(buffer(&v), 9.);
        assert_eq!(custom(&v, "ghost"), Some(82.));
    }
    #[test]
    fn shared_defaults_override_generated_metadata_and_preserve_custom_radii() {
        let mut v = Values(
            json!({"acquisition":{"defaults_version":2,"baseline_radius":120,"aa_range_buffer":10,"champions":{"ghost":{"default_baseline_radius":70},"soldier":{"override_radius":80}}}}),
        );
        migrate_defaults(&mut v);
        // Version 2 values may be deliberate edits, even if equal to old defaults.
        assert_eq!(baseline(&v, "ghost"), 120.);
        assert_eq!(buffer(&v), 10.);
        assert!(v.0["acquisition"]["champions"]["ghost"]
            .get("default_baseline_radius")
            .is_none());
        set_shared(&mut v, true, 75.);
        set_shared(&mut v, false, 8.);
        assert_eq!(radius(&v, "ghost", 60_000, Some(60_000)), 75_000);
        assert_eq!(radius(&v, "other", 60_000, Some(93_000)), 101_000);
        assert_eq!(radius(&v, "soldier", 60_000, Some(93_000)), 80_000);
        assert_eq!(radius(&v, "soldier", 100_000, Some(93_000)), 100_000);
        let acquisition = v.0["acquisition"].clone();
        v.set("manual_shop", 0.);
        v.set("acquisition_debug", 1.);
        v.reset_page(4);
        assert_eq!(v.0["acquisition"], acquisition);
        assert_eq!(v.number("acquisition_debug"), 1.);
        v.reset_page(5);
        assert_eq!(v.0["acquisition"], acquisition);
        assert_eq!(v.number("acquisition_debug"), 0.);
    }
    #[test]
    fn search_matches_localized_names_english_and_mod_ids() {
        let aliases: Value = serde_json::from_str(include_str!("acquisition_names.json")).unwrap();
        let mut row = Champion {
            name: "soldier".into(),
            ..Champion::default()
        };
        row.aliases = aliases["soldier"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().into())
            .collect();
        for query in ["SnIpEr", "步槍", "步枪", "小銃", "soldier"] {
            assert!(matches(&row, query), "{query}");
        }
        assert!(!matches(&row, "alchemist"));
        let mut rows = BTreeMap::from([(
            "mod_hero".into(),
            Champion {
                name: "mod_hero".into(),
                ..Champion::default()
            },
        )]);
        add_names(
            &json!({"zh-hant":{"description":{"mod_hero":{"name":"自訂英雄"},"item":{"name":"Not a champion"}}},"en":{"description":{"mod_hero":{"name":"Custom Hero"}}}}),
            &mut rows,
        );
        assert_eq!(rows.len(), 1);
        for query in ["自訂", "custom hero", "mod hero"] {
            assert!(matches(&rows["mod_hero"], query));
        }
    }
    #[test]
    fn automatic_uses_maximum_scaling_and_live_bonus_custom_only_floors_at_live_range() {
        let mut v = Values::default();
        assert_eq!(maximum(60_000, 3_000, Some(12)), Some(93_000));
        assert_eq!(radius(&v, "soldier", 60_000, Some(93_000)), 120_000);
        assert_eq!(radius(&v, "mod", 70_000, Some(150_000)), 155_000);
        assert_eq!(radius(&v, "mod", 180_000, Some(150_000)), 185_000);
        set_custom(&mut v, "mod", Some(80.));
        assert_eq!(radius(&v, "mod", 180_000, Some(200_000)), 180_000);
        assert_eq!(radius(&v, "mod", 70_000, Some(200_000)), 80_000);
        assert_eq!(custom(&v, "mod"), Some(80.));
        set_custom(&mut v, "mod", None);
        assert_eq!(radius(&v, "mod", 70_000, Some(200_000)), 205_000);
    }
    #[test]
    fn unknown_scaling_invalid_config_and_names_are_safe() {
        assert_eq!(maximum(60_000, 3000, None), None);
        assert_eq!(maximum(60_000, 0, None), Some(60_000));
        assert_eq!(maximum(u64::MAX, 1, Some(12)), None);
        assert_eq!(level_cap(&json!([150, 250, 300])), Some(4));
        assert_eq!(level_cap(&json!([])), None);
        let mut v = Values(json!({"acquisition": false}));
        assert_eq!(radius(&v, "a/b~", 150_000, None), 155_000);
        set_custom(&mut v, "a/b~", Some(175.));
        assert_eq!(custom(&v, "a/b~"), Some(175.));
        assert_eq!(custom(&v, "a"), None);
    }
    #[test]
    fn registration_preserves_other_champions_overrides_and_unknown_fields() {
        let mut v = Values(
            json!({"acquisition":{"champions":{"mod/hero":{"override_radius":145,"extra":true}}},"other":7}),
        );
        let rows = [
            Champion {
                name: "mod/hero".into(),
                maximum: Some(160_000),
                ..Champion::default()
            },
            Champion {
                name: "soldier".into(),
                ..Champion::default()
            },
        ];
        register_defaults(&mut v, &rows);
        assert_eq!(custom(&v, "mod/hero"), Some(145.));
        assert_eq!(custom(&v, "soldier"), None);
        assert_eq!(v.0["acquisition"]["champions"]["mod/hero"]["extra"], true);
        assert_eq!(v.0["other"], 7);
        v.reset_page(6);
        assert_eq!(custom(&v, "mod/hero"), None);
        assert_eq!(v.0["other"], 7);
    }
}
