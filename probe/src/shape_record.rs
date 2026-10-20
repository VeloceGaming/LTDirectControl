//! Records every champion's skill shapes as the live reader decodes them,
//! once per champion, into `research/skill-shapes.json` in the mod's folder
//! (step 1 of the agreed preview plan, the author 2026-10-21: learn the
//! shapes once, ship them as a table, keep the live reader for development
//! builds only; see docs/backlog.md item 2). Nothing here changes what the
//! game or the player sees.
//!
//! The combat hook sees every champion of every simulation, including the
//! game's background matches, but not their names; the simulation callback
//! on the same thread knows them. `observe_roster` stores that thread's
//! actor names; `wanted` and `record` are asked from the hook.
use crate::native_timing::MatchKey;
use crate::skill_preview::{Footprint, Geometry, Placement, Shape};
use crate::Logger;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
struct Roster {
    key: Option<MatchKey>,
    names: HashMap<usize, String>,
    /// Actors this thread has already handled in this match.
    done: HashSet<usize>,
}
thread_local! {
    static ROSTER: RefCell<Roster> = RefCell::new(Roster::default());
}
struct Table {
    game: String,
    champions: BTreeMap<String, [Value; 3]>,
}
static TABLE: Mutex<Option<Table>> = Mutex::new(None);
static GAME: OnceLock<String> = OnceLock::new();

/// At start-up: the running game's version, stamped on the table.
pub fn set_game(version: String) {
    let _ = GAME.set(version);
}
/// Simulation callback: this thread's match and its champions by actor id.
/// `names` runs only when the match changes on this thread, or while no
/// champion is known yet (before the champions exist).
pub fn observe_roster(key: MatchKey, names: impl FnOnce() -> Vec<(usize, String)>) {
    ROSTER.with(|r| {
        let mut r = r.borrow_mut();
        if r.key != Some(key) || r.names.is_empty() {
            *r = Roster {
                key: Some(key),
                names: names().into_iter().collect(),
                done: HashSet::new(),
            };
        }
    });
}
/// Combat hook: the champion name of `actor` when its shapes still need
/// recording (first sight on this thread and not yet in the table).
pub fn wanted(actor: usize) -> Option<String> {
    let name = ROSTER.with(|r| {
        let mut r = r.borrow_mut();
        if !r.done.insert(actor) {
            return None;
        }
        r.names.get(&actor).cloned()
    })?;
    let recorded = TABLE
        .lock()
        .ok()?
        .as_ref()
        .is_some_and(|t| t.champions.contains_key(&name));
    (!recorded).then_some(name)
}
/// Combat hook: store `name`'s decoded Q/W/R and rewrite the file.
pub fn record(name: String, skills: [Geometry; 3], log: &Logger) {
    let Ok(mut guard) = TABLE.lock() else {
        return;
    };
    let table = guard.get_or_insert_with(|| {
        let game = GAME.get().cloned().unwrap_or_default();
        let champions = earlier(log, &game);
        Table { game, champions }
    });
    if table.champions.contains_key(&name) {
        return;
    }
    log.write(&format!(
        "SHAPE RECORD champion={name} Q={} W={} R={} recorded={}",
        skills[0].summary(),
        skills[1].summary(),
        skills[2].summary(),
        table.champions.len() + 1
    ));
    table.champions.insert(name, skills.map(|g| skill(&g)));
    write(table, log);
}
/// Champions recorded by earlier sessions on the same game version, so the
/// table grows across sessions; a different game version starts over.
fn earlier(log: &Logger, game: &str) -> BTreeMap<String, [Value; 3]> {
    let file = log
        .directory()
        .map(|root| root.join("research").join("skill-shapes.json"));
    file.and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .filter(|v| v.get("game").and_then(Value::as_str) == Some(game))
        .and_then(|v| v.get("champions")?.as_object().cloned())
        .map(|champions| {
            champions
                .into_iter()
                .filter_map(|(name, slots)| {
                    let slot = |s: &str| slots.get(s).cloned();
                    Some((name, [slot("Q")?, slot("W")?, slot("R")?]))
                })
                .collect()
        })
        .unwrap_or_default()
}
fn write(table: &Table, log: &Logger) {
    let Some(root) = log.directory() else { return };
    let folder = root.join("research");
    if std::fs::create_dir_all(&folder).is_err() {
        return;
    }
    let champions: serde_json::Map<String, Value> = table
        .champions
        .iter()
        .map(|(name, [q, w, r])| (name.clone(), json!({"Q": q, "W": w, "R": r})))
        .collect();
    let report = json!({
        "about": "Skill shapes as the live reader decoded them (source live). Sizes in world units; range, cast style and target type are read live and not stored.",
        "game": table.game,
        "mod": env!("CARGO_PKG_VERSION"),
        "champions": champions,
    });
    if let Ok(text) = serde_json::to_string_pretty(&report) {
        let _ = std::fs::write(folder.join("skill-shapes.json"), text);
    }
}
/// One skill: its shapes, or why it could not be read.
pub fn skill(g: &Geometry) -> Value {
    if !g.footprints.is_empty() {
        json!({"source": "live", "shapes": g.footprints.iter().map(footprint).collect::<Vec<_>>()})
    } else if g.known_without_area() {
        json!({"source": "live", "shapes": [], "no_area": true})
    } else {
        json!({"source": "live", "unread": g.issues.first().cloned().unwrap_or_default()})
    }
}
fn footprint(f: &Footprint) -> Value {
    let r = |x: f32| (x * 100.).round() / 100.;
    let mut v = match f.shape {
        Shape::Circle { radius } => json!({"kind": "circle", "radius": r(radius)}),
        Shape::Corridor { radius, length } => {
            json!({"kind": "strip", "radius": r(radius), "length": r(length)})
        }
        Shape::Rectangle { width, height } => {
            json!({"kind": "rectangle", "width": r(width), "height": r(height)})
        }
        Shape::Cone { radius, cosine } => {
            json!({"kind": "cone", "radius": r(radius), "cosine": (cosine * 1000.).round() / 1000.})
        }
        Shape::Segment { radius, from, to } => json!({"kind": "segment", "radius": r(radius),
            "from": [r(from.0), r(from.1)], "to": [r(to.0), r(to.1)]}),
        Shape::Movement { blink } => json!({"kind": if blink { "blink" } else { "dash" }}),
        Shape::Shot {
            radius,
            length,
            pierce,
        } => json!({"kind": "shot", "radius": r(radius), "length": r(length), "pierce": pierce}),
        Shape::Reach { extra } => json!({"kind": "reach", "extra": r(extra)}),
        Shape::Fan {
            count,
            radius,
            length,
        } => json!({"kind": "fan", "count": count, "radius": r(radius), "length": r(length)}),
    };
    v["at"] = match f.placement {
        Placement::Caster => json!("caster"),
        Placement::Aim => json!("aim"),
        Placement::Forward { offset } => json!({"forward": r(offset)}),
        Placement::End { length } => json!({"end": r(length)}),
        Placement::Landing { length } => json!({"landing": r(length)}),
    };
    if f.allies {
        v["allies"] = json!(true);
    }
    if f.projectile {
        v["projectile"] = json!(true);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_actor_is_handled_once_per_match_and_named_from_the_roster() {
        observe_roster((1, 2, 3), || vec![(7, "crossbowman".into())]);
        assert_eq!(wanted(7).as_deref(), Some("crossbowman"));
        assert_eq!(wanted(7), None);
        // Minions and towers are not in the roster.
        assert_eq!(wanted(99), None);
        // A new match on this thread starts over.
        observe_roster((1, 2, 4), || vec![(7, "harpooner".into())]);
        assert_eq!(wanted(7).as_deref(), Some("harpooner"));
    }
    #[test]
    fn skills_are_written_as_shapes_no_area_or_unread() {
        let mut shot = Geometry::default();
        shot.add_projectile(
            Shape::Corridor {
                radius: 9.,
                length: 150.,
            },
            Placement::Caster,
        );
        assert_eq!(
            skill(&shot),
            json!({"source": "live", "shapes": [
                {"kind": "strip", "radius": 9.0, "length": 150.0, "at": "caster", "projectile": true}]})
        );
        let mut buff = Geometry::default();
        buff.family("AddCasterBuff");
        assert_eq!(skill(&buff)["no_area"], json!(true));
        let mut opaque = Geometry::default();
        opaque.issue("opaque Native");
        assert_eq!(skill(&opaque)["unread"], json!("opaque Native"));
    }
}
