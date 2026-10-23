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
use crate::skill_preview::Geometry;
use crate::Logger;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;
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
    /// What this session recorded: only these replace what the file
    /// already holds (1.5.1-dev3: `write` merges, so a champion that is not
    /// loaded this session keeps its recording).
    champions_now: BTreeSet<String>,
    numbers_now: BTreeSet<(String, usize)>,
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
    table.champions_now.insert(name.clone());
    table.champions.insert(name, slots);
    write(table, log);
}
/// The table so far: what earlier sessions recorded on the same game
/// version counts as recorded, so it grows across sessions; a champion
/// recorded on another game version is recorded again, and replaced in the
/// file when it is. Nothing is dropped from the file here: `write` merges.
fn load(log: &Logger) -> Table {
    let game = GAME.get().cloned().unwrap_or_default();
    let file = log
        .directory()
        .map(|root| root.join("research").join("skill-shapes.json"))
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    Table {
        champions: earlier(file.as_ref(), &game),
        numbers: earlier_numbers(file.as_ref(), &game),
        game,
        champions_now: BTreeSet::new(),
        numbers_now: BTreeSet::new(),
    }
}
/// The game version an entry was recorded on: its own `game` (written
/// since 1.5.1-dev3), else the file's.
fn recorded_on<'a>(entry: &'a Value, file: &'a Value) -> Option<&'a str> {
    entry
        .get("game")
        .or_else(|| file.get("game"))
        .and_then(Value::as_str)
}
fn earlier(file: Option<&Value>, game: &str) -> BTreeMap<String, [Value; 3]> {
    let Some(file) = file else {
        return BTreeMap::new();
    };
    file.get("champions")
        .and_then(Value::as_object)
        .map(|champions| {
            champions
                .iter()
                .filter(|(_, slots)| recorded_on(slots, file) == Some(game))
                .filter_map(|(name, slots)| {
                    // Entries from 1.5.0-dev8 lack `cast`: record them again.
                    let slot = |s: &str| slots.get(s).filter(|v| v.get("cast").is_some()).cloned();
                    Some((name.clone(), [slot("Q")?, slot("W")?, slot("R")?]))
                })
                .collect()
        })
        .unwrap_or_default()
}
fn earlier_numbers(file: Option<&Value>, game: &str) -> BTreeMap<String, [Option<Value>; 3]> {
    let Some(file) = file else {
        return BTreeMap::new();
    };
    file.get("numbers")
        .and_then(Value::as_object)
        .map(|numbers| {
            numbers
                .iter()
                .map(|(name, slots)| {
                    let slot = |s: &str| {
                        slots
                            .get(s)
                            .filter(|v| v.is_object() && recorded_on(v, file) == Some(game))
                            .cloned()
                    };
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
    table.numbers_now.insert((name.to_owned(), slot));
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
        entry["live"] = json!(live
            .footprints
            .iter()
            .map(crate::shape_json::footprint)
            .collect::<Vec<_>>());
    }
    entry
}
fn write(table: &Table, log: &Logger) {
    let Some(root) = log.directory() else { return };
    let folder = root.join("research");
    if std::fs::create_dir_all(&folder).is_err() {
        return;
    }
    for line in write_merged(&folder.join("skill-shapes.json"), table) {
        log.write(&line);
    }
}
/// Rewrites `path` with what it already holds plus what this session
/// recorded (1.5.1-dev3; the author, 2026-10-22: a champion pack is enabled
/// only to record it, then disabled, and must keep its recordings). Every
/// champion, and every skill's numbers, that this session did not record
/// is kept as it is, stamped with the game version it was recorded on; the
/// ones it did record replace them, and what it knows that the file lacks
/// is added. The text goes to a temporary file that
/// is then renamed over the old one, so a crash cannot leave it empty. An
/// old file that cannot be parsed is renamed aside (`.bad`) first, which
/// also keeps this from being logged again; one that cannot be read at all
/// (locked, say) is left alone and nothing is written. Returns log lines.
fn write_merged(path: &Path, table: &Table) -> Vec<String> {
    let mut lines = Vec::new();
    let old = match std::fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(value) if value.is_object() => value,
            _ => {
                let aside = path.with_extension("json.bad");
                let moved = match std::fs::rename(path, &aside) {
                    Ok(()) => format!("renamed to {}", aside.display()),
                    Err(error) => format!("could not be renamed aside ({error})"),
                };
                lines.push(format!(
                    "SHAPE RECORD unreadable {} {moved}; written with this session's recordings only",
                    path.display()
                ));
                json!({})
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(error) => {
            lines.push(format!(
                "SHAPE RECORD could not read {} ({error}); left as it is, not written",
                path.display()
            ));
            return lines;
        }
    };
    let old_game = old.get("game").cloned().unwrap_or(Value::Null);
    let stamp = |entry: &Value| {
        let mut entry = entry.clone();
        if entry.is_object() && entry.get("game").is_none() {
            entry["game"] = old_game.clone();
        }
        entry
    };
    let mut champions: serde_json::Map<String, Value> = old
        .get("champions")
        .and_then(Value::as_object)
        .map(|c| c.iter().map(|(name, v)| (name.clone(), stamp(v))).collect())
        .unwrap_or_default();
    // Recorded now: replaced. Known from earlier on this version but gone
    // from the file (deleted while the game ran): written back.
    for (name, [q, w, r]) in &table.champions {
        if table.champions_now.contains(name) || !champions.contains_key(name) {
            champions.insert(
                name.clone(),
                json!({"game": table.game, "Q": q, "W": w, "R": r}),
            );
        }
    }
    let mut numbers: serde_json::Map<String, Value> = old
        .get("numbers")
        .and_then(Value::as_object)
        .map(|n| {
            n.iter()
                .map(|(name, slots)| {
                    let slots = match slots.as_object() {
                        Some(slots) => Value::Object(
                            slots.iter().map(|(k, v)| (k.clone(), stamp(v))).collect(),
                        ),
                        None => json!({}),
                    };
                    (name.clone(), slots)
                })
                .collect()
        })
        .unwrap_or_default();
    for (name, values) in &table.numbers {
        for (slot, value) in values.iter().enumerate() {
            let Some(value) = value else { continue };
            let key = ["Q", "W", "R"][slot];
            let entry = numbers.entry(name.clone()).or_insert_with(|| json!({}));
            let now = table.numbers_now.contains(&(name.clone(), slot));
            if now || entry.get(key).is_none() {
                let mut value = value.clone();
                value["game"] = json!(table.game);
                entry[key] = value;
            }
        }
    }
    let report = json!({
        "about": "Skill shapes as the live reader decoded them (source live) and, under `file`, as the champion file declares them. Sizes in world units. `cast` is how the game casts the skill (casting 0 unit, 1 point, 2 direction, 3 none; target: mod-api-stable CastingTargetV1; range at level 1 and growth per level); at play these stay live. `numbers`: each skill's patch name (`kind`) and the numbers it answered by name (`values`, as the game keeps them: distances in thousandths), with the live cast range (`cast_range`) and the shapes the live reader read at the same moment (`live`), for learning which number each size is. `game` on a champion and on a skill's numbers: the game version it was recorded on. The file keeps every champion ever recorded (1.5.1-dev3); the top-level `game` is the version of the last session that wrote it.",
        "game": table.game,
        "mod": env!("CARGO_PKG_VERSION"),
        "champions": champions,
        "numbers": numbers,
    });
    let Ok(text) = serde_json::to_string_pretty(&report) else {
        return lines;
    };
    let temp = path.with_extension("json.tmp");
    if let Err(error) = std::fs::write(&temp, text).and_then(|()| std::fs::rename(&temp, path)) {
        let _ = std::fs::remove_file(&temp);
        lines.push(format!(
            "SHAPE RECORD could not write {} ({error})",
            path.display()
        ));
    }
    lines
}
/// One skill as the live reader read it (`shape_json::skill`).
pub fn skill(g: &Geometry) -> Value {
    crate::shape_json::skill(g, "live")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill_preview::{Placement, Shape};
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
        let file = json!({"game": "0.6.4", "numbers": {"hammerer": {
            "W": {"kind": "hammerer_skill2"}, "R": {"kind": "x", "game": "0.6.5"}}}});
        let loaded = earlier_numbers(Some(&file), "0.6.4");
        assert!(loaded["hammerer"][0].is_none() && loaded["hammerer"][1].is_some());
        // Recorded on another game version: asked again on this one.
        assert!(loaded["hammerer"][2].is_none());
    }
    fn fake_folder(name: &str) -> std::path::PathBuf {
        let folder =
            std::env::temp_dir().join(format!("lt-shape-record-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }
    fn slots(tag: &str) -> [Value; 3] {
        [0, 1, 2].map(|i| {
            json!({"source": "live", "shapes": [], "tag": tag, "slot": i,
            "cast": {"casting": 1}})
        })
    }
    fn session(game: &str) -> Table {
        Table {
            game: game.to_owned(),
            champions: BTreeMap::new(),
            numbers: BTreeMap::new(),
            champions_now: BTreeSet::new(),
            numbers_now: BTreeSet::new(),
        }
    }
    fn read(path: &Path) -> Value {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }
    #[test]
    fn a_write_keeps_every_champion_this_session_did_not_record() {
        let folder = fake_folder("keep");
        let path = folder.join("skill-shapes.json");
        // A file from before 1.5.1-dev3: no per-champion game.
        let old = json!({"game": "0.6.4", "champions": {
                "bombardier": {"Q": {"tag": "old"}, "W": {}, "R": {}},
                "archer": {"Q": {"tag": "old"}, "W": {}, "R": {}}},
            "numbers": {"bombardier": {"Q": {"kind": "b1"}},
                "archer": {"Q": {"kind": "old_q"}, "W": {"kind": "old_w"}}}});
        std::fs::write(&path, old.to_string()).unwrap();
        let mut table = session("0.6.5");
        table.champions.insert("archer".into(), slots("new"));
        table.champions_now.insert("archer".into());
        table.numbers.entry("archer".into()).or_default()[0] = Some(json!({"kind": "new_q"}));
        table.numbers_now.insert(("archer".into(), 0));
        assert!(write_merged(&path, &table).is_empty());
        let file = read(&path);
        assert_eq!(file["game"], json!("0.6.5"));
        // Not loaded this session: kept, with the version it was recorded on.
        assert_eq!(file["champions"]["bombardier"]["Q"]["tag"], json!("old"));
        assert_eq!(file["champions"]["bombardier"]["game"], json!("0.6.4"));
        assert_eq!(file["numbers"]["bombardier"]["Q"]["kind"], json!("b1"));
        assert_eq!(file["numbers"]["bombardier"]["Q"]["game"], json!("0.6.4"));
        // Re-recorded: replaced, on this game version.
        assert_eq!(file["champions"]["archer"]["Q"]["tag"], json!("new"));
        assert_eq!(file["champions"]["archer"]["game"], json!("0.6.5"));
        // Numbers merge per skill: Q replaced, W kept.
        assert_eq!(file["numbers"]["archer"]["Q"]["kind"], json!("new_q"));
        assert_eq!(file["numbers"]["archer"]["Q"]["game"], json!("0.6.5"));
        assert_eq!(file["numbers"]["archer"]["W"]["kind"], json!("old_w"));
        assert!(!path.with_extension("json.tmp").exists());
        // Read back by the next session on 0.6.5: only archer counts as
        // recorded there; bombardier waits for a 0.6.5 match.
        let loaded = earlier(Some(&file), "0.6.5");
        assert_eq!(loaded.keys().collect::<Vec<_>>(), ["archer"]);
        let _ = std::fs::remove_dir_all(&folder);
    }
    #[test]
    fn a_corrupt_file_is_set_aside_and_does_not_stop_the_write() {
        let folder = fake_folder("corrupt");
        let path = folder.join("skill-shapes.json");
        std::fs::write(&path, b"{\"champions\": {\"arch").unwrap();
        let mut table = session("0.6.5");
        table.champions.insert("archer".into(), slots("new"));
        table.champions_now.insert("archer".into());
        let lines = write_merged(&path, &table);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("renamed to"), "{}", lines[0]);
        assert_eq!(
            std::fs::read(path.with_extension("json.bad")).unwrap(),
            b"{\"champions\": {\"arch"
        );
        assert_eq!(read(&path)["champions"]["archer"]["Q"]["tag"], json!("new"));
        // The next write finds a good file: nothing more to log.
        assert!(write_merged(&path, &table).is_empty());
        let _ = std::fs::remove_dir_all(&folder);
    }
    #[test]
    fn a_missing_file_is_written_from_everything_this_session_knows() {
        let folder = fake_folder("missing");
        let path = folder.join("skill-shapes.json");
        let mut table = session("0.6.4");
        table.champions.insert("archer".into(), slots("new"));
        table.champions_now.insert("archer".into());
        // Loaded from the file at start, which was deleted since: written
        // back from memory.
        table.champions.insert("knight".into(), slots("memory"));
        assert!(write_merged(&path, &table).is_empty());
        let file = read(&path);
        assert_eq!(
            file["champions"]
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            ["archer", "knight"]
        );
        // Loaded and still in the file: the file's copy stays.
        std::fs::write(
            &path,
            json!({"game": "0.6.4", "champions": {"knight": {"Q": {"tag": "file"}}}}).to_string(),
        )
        .unwrap();
        write_merged(&path, &table);
        assert_eq!(
            read(&path)["champions"]["knight"]["Q"]["tag"],
            json!("file")
        );
        let _ = std::fs::remove_dir_all(&folder);
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
                {"kind": "strip", "radius": 9.0, "length": 150.0, "rounded": true, "at": "caster",
                    "projectile": true}]})
        );
        // A shot says its radius is the hit radius on each side (1.5.2-dev1).
        let mut line = Geometry::default();
        line.add(
            Shape::Shot {
                radius: 60.,
                length: 30.,
                pierce: true,
            },
            Placement::Caster,
        );
        assert_eq!(
            skill(&line)["shapes"][0],
            json!({"kind": "shot", "radius": 60.0, "length": 30.0, "pierce": true,
                "hit_radius": true, "at": "caster"})
        );
        let mut buff = Geometry::default();
        buff.family("AddCasterBuff");
        assert_eq!(skill(&buff)["no_area"], json!(true));
        let mut opaque = Geometry::default();
        opaque.issue("opaque Native");
        assert_eq!(skill(&opaque)["unread"], json!("opaque Native"));
    }
}
