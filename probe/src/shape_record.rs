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
    /// (actor, slot) pairs whose named skill numbers were already read in
    /// this match (`patch_fields`, 1.5.0-dev13 research).
    fields: HashSet<(usize, usize)>,
    /// (actor, slot) pairs found without a skill object yet in this match
    /// (unlocked by level but not learned), logged once.
    unlearned: HashSet<(usize, usize)>,
}
thread_local! {
    static ROSTER: RefCell<Roster> = RefCell::new(Roster::default());
}
struct Table {
    game: String,
    champions: BTreeMap<String, [Value; 3]>,
    /// Each skill's numbers by name, with the shapes the live reader read at
    /// the same moment (1.5.0-dev15, step 4 stage A): the viewer and
    /// `tools/build_preview_table.py` learn from these pairs which number
    /// each size is.
    numbers: BTreeMap<String, [Option<Value>; 3]>,
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
                fields: HashSet::new(),
                unlearned: HashSet::new(),
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
/// Combat hook: the champion name of `actor` while its skill in `slot`
/// (0 Q, 1 W, 2 R) has not been read yet in this match on this thread, for
/// the named-number research log (`patch_fields`). Unlike `wanted`, this
/// does not stop once the champion is in the shape table. Asking does not
/// count as reading: `fields_done` does, so a skill the champion has not
/// learned yet is tried again later (1.5.0-dev16: Amazon learns her R after
/// level 5, and dev13-dev15 never tried again in that match).
pub fn fields_wanted(actor: usize, slot: usize) -> Option<String> {
    ROSTER.with(|r| {
        let r = r.borrow();
        if r.fields.contains(&(actor, slot)) {
            return None;
        }
        r.names.get(&actor).cloned()
    })
}
/// Combat hook: `actor`'s skill in `slot` was read (or refused for good) in
/// this match.
pub fn fields_done(actor: usize, slot: usize) {
    ROSTER.with(|r| {
        r.borrow_mut().fields.insert((actor, slot));
    });
}
/// Combat hook: true the first time in this match that `actor`'s skill in
/// `slot` is found without a skill object, for one log line.
pub fn first_unlearned(actor: usize, slot: usize) -> bool {
    ROSTER.with(|r| r.borrow_mut().unlearned.insert((actor, slot)))
}
/// How a skill is cast, as the game holds it: the casting type (0 a unit,
/// 1 a point, 2 a direction, 3 none), the target type (mod-api-stable
/// `CastingTargetV1`), and the range with its growth per level, in
/// thousandths of a world unit. Recorded for the viewer only.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cast {
    pub casting: u32,
    pub target: u32,
    pub range: u64,
    pub growth: u64,
}
/// Combat hook: store `name`'s decoded Q/W/R, how each is cast, and what
/// its champion file declares (if any), then rewrite the file.
pub fn record(name: String, skills: [Geometry; 3], casts: [Cast; 3], log: &Logger) {
    let Ok(mut guard) = TABLE.lock() else {
        return;
    };
    let table = guard.get_or_insert_with(|| load(log));
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
    let slots = [0, 1, 2].map(|slot| {
        let mut entry = skill(&skills[slot]);
        let c = casts[slot];
        entry["cast"] = json!({"casting": c.casting, "target": c.target,
            "range": c.range as f64 / 1000., "growth": c.growth as f64 / 1000.});
        // The champion file's own declaration, the preview's fallback.
        let declared = crate::skill_preview::geometry(Some(&name), slot);
        if !declared.footprints.is_empty() || declared.known_without_area() {
            let mut file = skill(&declared);
            file["source"] = json!("file");
            entry["file"] = file;
        }
        entry
    });
    table.champions.insert(name, slots);
    write(table, log);
}
/// The table so far: what earlier sessions recorded on the same game
/// version, so it grows across sessions; a different game version starts
/// over.
fn load(log: &Logger) -> Table {
    let game = GAME.get().cloned().unwrap_or_default();
    let file = log
        .directory()
        .map(|root| root.join("research").join("skill-shapes.json"))
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .filter(|v| v.get("game").and_then(Value::as_str) == Some(game.as_str()));
    Table {
        champions: earlier(file.as_ref()),
        numbers: earlier_numbers(file.as_ref()),
        game,
    }
}
fn earlier(file: Option<&Value>) -> BTreeMap<String, [Value; 3]> {
    file.and_then(|v| v.get("champions")?.as_object().cloned())
        .map(|champions| {
            champions
                .into_iter()
                .filter_map(|(name, slots)| {
                    // Entries from 1.5.0-dev8 lack `cast`: record them again.
                    let slot = |s: &str| slots.get(s).filter(|v| v.get("cast").is_some()).cloned();
                    Some((name, [slot("Q")?, slot("W")?, slot("R")?]))
                })
                .collect()
        })
        .unwrap_or_default()
}
fn earlier_numbers(file: Option<&Value>) -> BTreeMap<String, [Option<Value>; 3]> {
    file.and_then(|v| v.get("numbers")?.as_object())
        .map(|numbers| {
            numbers
                .iter()
                .map(|(name, slots)| {
                    let slot = |s: &str| slots.get(s).filter(|v| v.is_object()).cloned();
                    (name.clone(), [slot("Q"), slot("W"), slot("R")])
                })
                .collect()
        })
        .unwrap_or_default()
}
/// Combat hook: whether `name`'s skill in `slot` still needs its numbers
/// recorded on this game version.
pub fn numbers_wanted(name: &str, slot: usize, log: &Logger) -> bool {
    let Ok(mut guard) = TABLE.lock() else {
        return false;
    };
    let table = guard.get_or_insert_with(|| load(log));
    table
        .numbers
        .get(name)
        .is_none_or(|slots| slots.get(slot).is_none_or(Option::is_none))
}
/// Combat hook: store `name`'s skill numbers in `slot` (its patch name and
/// every number it answered, as the game keeps them: distances in
/// thousandths) with the live cast range and the shapes the live reader
/// read at the same moment, then rewrite the file. The cast range is kept
/// apart because some skills use `range` for their area (Knight R).
pub fn record_numbers(
    name: &str,
    slot: usize,
    kind: &str,
    values: &[(String, Option<i64>)],
    cast_range: u64,
    live: &Geometry,
    log: &Logger,
) {
    let Ok(mut guard) = TABLE.lock() else {
        return;
    };
    let table = guard.get_or_insert_with(|| load(log));
    let slots = table.numbers.entry(name.to_owned()).or_default();
    if slot > 2 || slots[slot].is_some() {
        return;
    }
    let answered: serde_json::Map<String, Value> = values
        .iter()
        .filter_map(|(field, v)| Some((field.clone(), json!((*v)?))))
        .collect();
    slots[slot] = Some(numbers_entry(kind, answered, cast_range, live));
    log.write(&format!(
        "SHAPE NUMBERS champion={name} slot={} type={kind} live={}",
        ["Q", "W", "R"][slot],
        live.summary()
    ));
    write(table, log);
}
fn numbers_entry(
    kind: &str,
    values: serde_json::Map<String, Value>,
    cast_range: u64,
    live: &Geometry,
) -> Value {
    let mut entry = json!({"kind": kind, "values": values,
        "cast_range": cast_range as f64 / 1000.});
    if !live.footprints.is_empty() {
        entry["live"] = json!(live.footprints.iter().map(footprint).collect::<Vec<_>>());
    }
    entry
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
    let numbers: serde_json::Map<String, Value> = table
        .numbers
        .iter()
        .map(|(name, slots)| {
            let mut entry = serde_json::Map::new();
            for (slot, value) in ["Q", "W", "R"].iter().zip(slots) {
                if let Some(value) = value {
                    entry.insert(slot.to_string(), value.clone());
                }
            }
            (name.clone(), Value::Object(entry))
        })
        .collect();
    let report = json!({
        "about": "Skill shapes as the live reader decoded them (source live) and, under `file`, as the champion file declares them. Sizes in world units. `cast` is how the game casts the skill (casting 0 unit, 1 point, 2 direction, 3 none; target: mod-api-stable CastingTargetV1; range at level 1 and growth per level); at play these stay live. `numbers`: each skill's patch name (`kind`) and the numbers it answered by name (`values`, as the game keeps them: distances in thousandths), with the live cast range (`cast_range`) and the shapes the live reader read at the same moment (`live`), for learning which number each size is.",
        "game": table.game,
        "mod": env!("CARGO_PKG_VERSION"),
        "champions": champions,
        "numbers": numbers,
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
        Shape::Spread {
            count,
            radius,
            length,
            degrees,
        } => json!({"kind": "spread", "count": count, "radius": r(radius), "length": r(length),
            "degrees": r(degrees)}),
        Shape::Backstep { length } => json!({"kind": "backstep", "length": r(length)}),
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
    fn a_skill_is_offered_until_it_has_been_read_in_this_match() {
        observe_roster((5, 6, 7), || vec![(3, "amazon".into())]);
        // Not learned yet: still offered next time, logged once.
        assert_eq!(fields_wanted(3, 2).as_deref(), Some("amazon"));
        assert!(first_unlearned(3, 2));
        assert!(!first_unlearned(3, 2));
        assert_eq!(fields_wanted(3, 2).as_deref(), Some("amazon"));
        fields_done(3, 2);
        assert_eq!(fields_wanted(3, 2), None);
        assert_eq!(fields_wanted(3, 1).as_deref(), Some("amazon"));
        // Not a champion of this match.
        assert_eq!(fields_wanted(99, 0), None);
        observe_roster((5, 6, 8), || vec![(3, "amazon".into())]);
        assert_eq!(fields_wanted(3, 2).as_deref(), Some("amazon"));
    }
    #[test]
    fn numbers_keep_the_answered_values_with_the_shapes_read_at_the_same_moment() {
        let mut live = Geometry::default();
        live.add(
            Shape::Circle { radius: 62. },
            Placement::Forward { offset: 12. },
        );
        let values = serde_json::Map::from_iter([("attack_range".to_owned(), json!(62000))]);
        assert_eq!(
            numbers_entry("hammerer_skill2", values.clone(), 12000, &live),
            json!({"kind": "hammerer_skill2", "values": {"attack_range": 62000},
                "cast_range": 12.0,
                "live": [{"kind": "circle", "radius": 62.0, "at": {"forward": 12.0}}]})
        );
        assert_eq!(
            numbers_entry("x", values, 0, &Geometry::default()),
            json!({"kind": "x", "values": {"attack_range": 62000}, "cast_range": 0.0})
        );
        let file = json!({"numbers": {"hammerer": {"W": {"kind": "hammerer_skill2"}}}});
        let loaded = earlier_numbers(Some(&file));
        assert!(loaded["hammerer"][0].is_none() && loaded["hammerer"][1].is_some());
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
