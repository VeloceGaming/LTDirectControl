//! Research builds only (1.7.1-dev9): the strategy probe. It reads the
//! official mod interface's live team strategy (`strategy_get_json`):
//! how that interface names a field, and what the game changes on its own
//! mid-match. Official interface only; no game address, nothing kept from
//! the game.
//!
//! - At the viewed match's first tick: both teams' documents
//!   (`STRATEGY doc`), then which field path form works on the controlled
//!   athlete's team (`STRATEGY get`, `STRATEGY start`).
//! - Once a second all match: a field the game changed on its own, once per
//!   team and field (`STRATEGY game changed`).
//!
//! Superseded 2026-11-04 (1.7.1-dev21, the author's pings spec section
//! 14.2): the F7 and F8 keys (`focused` and `object_buildup` cycled from
//! the client, with their `STRATEGY request/set/check/watch` lines) are
//! removed. The strategy writes the pings use are `plan_call.rs`.
//!
//! Threads: only the match hook (the viewed match's simulation thread).
//! Short locks, none held while the game runs.
use crate::{movement::Movement, native_timing::MatchKey, Logger};
use mod_api_stable::StableSim;
use std::sync::Mutex;

/// The paths tried for the whole document, in order.
pub const DOC_PATHS: [&str; 3] = ["", "/", "."];
/// The forms tried for one field, in order.
pub const FIELD_FORMS: [&str; 4] = ["{}", "/{}", "$.{}", "team.{}"];
pub fn field_path(form: usize, field: &str) -> String {
    FIELD_FORMS[form].replace("{}", field)
}
/// The top-level fields that differ between two documents: (field, from,
/// to) as compact JSON. A document that is not a JSON object compares as
/// one field named `*`.
pub fn changed_fields(before: &str, after: &str) -> Vec<(String, String, String)> {
    if before == after {
        return Vec::new();
    }
    let parse = |s: &str| serde_json::from_str::<serde_json::Value>(s).ok();
    match (parse(before), parse(after)) {
        (Some(serde_json::Value::Object(a)), Some(serde_json::Value::Object(b))) => {
            let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
            keys.sort();
            keys.dedup();
            keys.into_iter()
                .filter(|k| a.get(*k) != b.get(*k))
                .map(|k| {
                    let text = |v: Option<&serde_json::Value>| {
                        v.map_or("-".to_owned(), serde_json::Value::to_string)
                    };
                    (k.clone(), text(a.get(k)), text(b.get(k)))
                })
                .collect()
        }
        _ => vec![("*".to_owned(), before.to_owned(), after.to_owned())],
    }
}

/// Ticks between checks.
const EVERY: usize = 60;

struct Probe {
    key: MatchKey,
    doc_path: Option<&'static str>,
    /// Each team's document as last seen.
    baseline: [Option<String>; 2],
    /// (team, field) already said changed by the game.
    game_logged: Vec<(usize, String)>,
}
static PROBE: Mutex<Option<Probe>> = Mutex::new(None);

fn get(sim: &StableSim<'_>, team: usize, path: &str, log: &Logger, what: &str) -> Option<String> {
    let json = sim.strategy_get_json(team, path);
    log.write(&format!(
        "STRATEGY {what} team={team} path={path:?} ok={} json={}",
        json.is_some(),
        json.as_deref().unwrap_or("-")
    ));
    json
}

/// The match hook, every tick of every match: only the viewed match on
/// screen is probed.
pub fn observe(sim: &mut StableSim<'_>, movement: &Movement, log: &Logger) {
    let Some(origin) = sim.sim_origin().filter(|o| o.kind == 2) else {
        return;
    };
    let key = (sim.seed(), origin.match_id, origin.set_index);
    let Some((viewed, player)) = movement.hud_identity() else {
        return;
    };
    if viewed != key {
        return;
    }
    let Some(team) = sim.get_player(player).map(|p| p.team()) else {
        return;
    };
    let Ok(mut slot) = PROBE.lock() else {
        return;
    };
    if slot.as_ref().is_none_or(|p| p.key != key) {
        *slot = Some(start(sim, key, team, log));
    }
    let Some(probe) = slot.as_mut() else {
        return;
    };
    if sim.tick().is_multiple_of(EVERY) {
        game_changes(sim, probe, log);
    }
}

fn start(sim: &StableSim<'_>, key: MatchKey, team: usize, log: &Logger) -> Probe {
    let mut doc_path = None;
    let mut baseline = [None, None];
    for (t, kept) in baseline.iter_mut().enumerate() {
        for path in DOC_PATHS {
            let what = if path.is_empty() { "doc" } else { "get" };
            if let Some(json) = get(sim, t, path, log, what) {
                doc_path.get_or_insert(path);
                *kept = Some(json);
                break;
            }
        }
    }
    let form = (0..FIELD_FORMS.len())
        .find(|f| get(sim, team, &field_path(*f, "focused"), log, "get").is_some());
    log.write(&format!(
        "STRATEGY start team={team} doc_path={} field_form={}",
        doc_path.map_or("none".to_owned(), |p| format!("{p:?}")),
        form.map_or("none", |f| FIELD_FORMS[f])
    ));
    Probe {
        key,
        doc_path,
        baseline,
        game_logged: Vec::new(),
    }
}

fn game_changes(sim: &StableSim<'_>, probe: &mut Probe, log: &Logger) {
    let Some(path) = probe.doc_path else {
        return;
    };
    for team in 0..2 {
        let Some(doc) = sim.strategy_get_json(team, path) else {
            continue;
        };
        let Some(before) = probe.baseline[team].as_deref() else {
            probe.baseline[team] = Some(doc);
            continue;
        };
        for (field, from, to) in changed_fields(before, &doc) {
            if probe
                .game_logged
                .iter()
                .any(|l| *l == (team, field.clone()))
            {
                continue;
            }
            log.write(&format!(
                "STRATEGY game changed team={team} field={field} from={from} to={to}"
            ));
            probe.game_logged.push((team, field));
        }
        probe.baseline[team] = Some(doc);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_fields_name_each_top_level_difference() {
        let a = r#"{"focused":"Top","game_finish":"Stable"}"#;
        let b = r#"{"focused":"All","game_finish":"Stable"}"#;
        assert_eq!(
            changed_fields(a, b),
            [(
                "focused".to_owned(),
                "\"Top\"".to_owned(),
                "\"All\"".to_owned()
            )]
        );
        assert!(changed_fields(a, a).is_empty());
        assert_eq!(changed_fields("x", "y")[0].0, "*");
    }
}
