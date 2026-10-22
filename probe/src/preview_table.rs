//! The skill preview table the mod ships (step 4 of the agreed preview
//! plan, stage B; docs/backlog.md item 2). `preview_table.json` is written
//! by `tools/build_preview_table.py` from the shapes and numbers the mod
//! recorded in play and the author's assignments and picks; it is compiled
//! in, so a change to it goes through a build.
//!
//! Each skill's entry holds its shapes (or the author's assignment) with
//! every size replaced by where it comes from:
//! - `{"name": "attack_range", "recorded": 62}`: the game's number of that
//!   name divided by 1000 (distances are kept in thousandths), times
//!   `times` if given; `"unit": "count"` or `"degrees"` take it as is;
//! - `{"cast_range": true}`: the live cast range;
//! - `{"fixed": 12}`: a number in world units;
//! - `{"name": "speed", "by": "tick", "recorded": 84}`: two of the game's
//!   numbers multiplied, the first a distance (divided by 1000), the second
//!   a plain count (a charge's speed per tick times its ticks), times
//!   `times` if given.
//!
//! A custom preview (`"from": "custom"`, 1.5.2-dev2) is the author's own
//! list of `pieces`, in the same form as recorded shapes, built in the
//! viewer. It is drawn exactly as listed (`Geometry::exact`). The author's
//! own file (`skill-overrides.json`, type `custom`) wins over the shipped
//! table, so a custom preview can be tried in game before a build.
//!
//! The game's numbers are read from the controlled athlete's own skill
//! objects (`native_adapter/windows/skill_numbers.rs`), so the preview
//! follows the player's database edits and the save's balance patches
//! (seen in the 1.5.0-dev13 log). When a number cannot be read, its
//! recorded value is used.
use crate::skill_preview::{Geometry, Placement, Shape};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

const SHIPPED: &str = include_str!("preview_table.json");
/// The family a shipped assignment carries: `skill_preview::resolve` lets it
/// win even when it draws nothing.
pub const ASSIGNMENT: &str = "table assignment";
static TABLE: OnceLock<HashMap<String, [Option<Value>; 3]>> = OnceLock::new();
/// Custom previews from the author's own file, as table entries.
static CUSTOM: OnceLock<HashMap<(String, usize), Value>> = OnceLock::new();
/// At start-up: the author's custom previews by (champion, slot), each its
/// list of pieces (`skill_preview::load_overrides`).
pub fn set_custom(pieces: HashMap<(String, usize), Value>) {
    let _ = CUSTOM.set(
        pieces
            .into_iter()
            .map(|(k, p)| (k, serde_json::json!({"from": "custom", "pieces": p})))
            .collect(),
    );
}

fn table() -> &'static HashMap<String, [Option<Value>; 3]> {
    TABLE.get_or_init(|| parse(SHIPPED))
}
fn parse(text: &str) -> HashMap<String, [Option<Value>; 3]> {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return HashMap::new();
    };
    value
        .get("champions")
        .and_then(Value::as_object)
        .map(|champions| {
            champions
                .iter()
                .map(|(name, slots)| {
                    let slot = |s: &str| slots.get(s).filter(|v| v.is_object()).cloned();
                    (name.clone(), [slot("Q"), slot("W"), slot("R")])
                })
                .collect()
        })
        .unwrap_or_default()
}
/// How many champions and skills the shipped table holds (for the log).
pub fn counts() -> (usize, usize) {
    let t = table();
    (t.len(), t.values().flatten().flatten().count())
}
fn entry(champion: &str, slot: usize) -> Option<&'static Value> {
    if let Some(own) = CUSTOM
        .get()
        .and_then(|c| c.get(&(champion.to_owned(), slot)))
    {
        return Some(own);
    }
    table().get(champion)?.get(slot)?.as_ref()
}
/// What the combat hook knows of the controlled athlete's skill now.
#[derive(Default)]
pub struct Live<'a> {
    /// The skill's numbers by name, as the game keeps them; `None` when
    /// they could not be read (then the recorded values are used).
    pub numbers: Option<&'a HashMap<String, i64>>,
    /// The live cast range in world units (level, patches and bonuses).
    pub cast_range: Option<f32>,
    /// The champion's stack (Dancer R's blades grow with her kills).
    pub stack: usize,
}
/// The names of the game numbers this skill's entry uses.
pub fn names(champion: &str, slot: usize) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(e) = entry(champion, slot) {
        collect_names(e, &mut out);
    }
    out
}
fn collect_names(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            if is_source(o) {
                for key in ["name", "by"] {
                    if let Some(name) = o.get(key).and_then(Value::as_str) {
                        if !out.iter().any(|n| n == name) {
                            out.push(name.to_owned());
                        }
                    }
                }
            }
            o.values().for_each(|v| collect_names(v, out));
        }
        Value::Array(a) => a.iter().for_each(|v| collect_names(v, out)),
        _ => {}
    }
}
fn is_source(o: &Map<String, Value>) -> bool {
    o.contains_key("fixed")
        || o.get("cast_range").is_some_and(|v| v == &Value::Bool(true))
        || (o.contains_key("name")
            && (o.contains_key("recorded") || o.contains_key("unit") || o.contains_key("by")))
}
/// What a source gives now, in world units (or a count, or degrees).
fn value_of(o: &Map<String, Value>, live: &Live) -> Option<f64> {
    let recorded = o.get("recorded").and_then(Value::as_f64);
    if let Some(fixed) = o.get("fixed") {
        return fixed.as_f64();
    }
    if o.get("cast_range").is_some_and(|v| v == &Value::Bool(true)) {
        return live.cast_range.map(f64::from).or(recorded);
    }
    let name = o.get("name")?.as_str()?;
    let times = o.get("times").and_then(Value::as_f64).unwrap_or(1.);
    if let Some(by) = o.get("by") {
        let by = by.as_str()?;
        let read = live
            .numbers
            .and_then(|n| Some((*n.get(name)?, *n.get(by)?)))
            .map(|(a, b)| a as f64 / 1000. * b as f64 * times);
        return read.or(recorded);
    }
    let read = live.numbers.and_then(|n| n.get(name)).map(|&number| {
        match o.get("unit").and_then(Value::as_str) {
            Some("count" | "degrees") => number as f64,
            _ => number as f64 / 1000. * times,
        }
    });
    read.or(recorded)
}
/// `v` with every source replaced by its value now.
fn resolved(v: &Value, live: &Live) -> Value {
    match v {
        Value::Object(o) if is_source(o) => value_of(o, live).map_or(Value::Null, Value::from),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, v)| (k.clone(), resolved(v, live)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(|v| resolved(v, live)).collect()),
        other => other.clone(),
    }
}
/// The shipped preview of `champion`'s skill in `slot` (0 Q, 1 W, 2 R), or
/// `None` when the table has no entry (the mod then falls back as before).
pub fn geometry(champion: &str, slot: usize, live: &Live) -> Option<Geometry> {
    of_entry(entry(champion, slot)?, live)
}
/// One table entry as a preview, its sizes from `live`.
fn of_entry(entry: &Value, live: &Live) -> Option<Geometry> {
    let e = resolved(entry, live);
    let mut g = Geometry::default();
    match e.get("from")?.as_str()? {
        "no_area" => g.family("table no area"),
        "live" | "file" => {
            shapes(&e, "shapes", live, &mut g);
            g.family("table");
        }
        // The author's pieces, drawn as listed; it wins even when it draws
        // nothing, like an assignment.
        "custom" => {
            shapes(&e, "pieces", live, &mut g);
            g.exact = true;
            g.family("custom");
            g.family(ASSIGNMENT);
        }
        "assigned" => {
            let a = crate::skill_preview::assigned(e.get("assigned")?)?;
            if a.adds {
                shapes(&e, "shapes", live, &mut g);
                for f in a.footprints {
                    g.push(f.shape, f.placement, f.allies, f.projectile);
                }
                g.hidden |= a.hidden;
            } else {
                g = a;
            }
            g.family(ASSIGNMENT);
        }
        _ => return None,
    }
    Some(g)
}
fn shapes(e: &Value, key: &str, live: &Live, g: &mut Geometry) {
    for s in e.get(key).and_then(Value::as_array).into_iter().flatten() {
        match footprint(s, live) {
            Some((shape, placement)) => g.push(
                shape,
                placement,
                s.get("allies").and_then(Value::as_bool).unwrap_or(false),
                s.get("projectile")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            ),
            None => g.issue("table shape unreadable"),
        }
    }
}
fn footprint(s: &Value, live: &Live) -> Option<(Shape, Placement)> {
    let n = |key: &str| {
        s.get(key)
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite() && *n >= 0. && *n <= 100_000.)
            .map(|n| n as f32)
    };
    let shape = match s.get("kind")?.as_str()? {
        "circle" => Shape::Circle {
            radius: n("radius")?,
        },
        "strip" => Shape::Corridor {
            radius: n("radius")?,
            length: n("length").unwrap_or(0.),
        },
        "rectangle" => Shape::Rectangle {
            width: n("width")?,
            height: n("height")?,
        },
        "cone" => Shape::Cone {
            radius: n("radius")?,
            cosine: s.get("cosine")?.as_f64()?.clamp(-1., 1.) as f32,
        },
        "dash" => Shape::Movement { blink: false },
        "blink" => Shape::Movement { blink: true },
        "shot" => Shape::Shot {
            radius: n("radius")?,
            length: n("length")?,
            pierce: s.get("pierce").and_then(Value::as_bool).unwrap_or(false),
        },
        "reach" => Shape::Reach { extra: n("extra")? },
        "fan" => {
            // Dancer R: the base count plus the caster's stack, up to the
            // most blades (`max_count`, from the game's champion sheet).
            let base = n("count")?.round() as usize;
            let most = n("max_count").map_or(base, |m| m.round() as usize);
            Shape::Fan {
                count: (base + live.stack).min(most.max(base)).clamp(1, 16),
                radius: n("radius")?,
                length: n("length")?,
            }
        }
        "spread" => Shape::Spread {
            count: n("count")?.round().clamp(1., 16.) as usize,
            radius: n("radius")?,
            length: n("length").unwrap_or(0.),
            degrees: n("degrees")?.min(360.),
        },
        "backstep" => Shape::Backstep {
            length: n("length")?,
        },
        // Absolute world points cannot be shipped.
        _ => return None,
    };
    let at = s.get("at")?;
    let at_n = |key: &str| {
        at.get(key)
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite() && *n >= 0. && *n <= 100_000.)
            .map(|n| n as f32)
    };
    let placement = match at.as_str() {
        Some("caster") => Placement::Caster,
        Some("aim") => Placement::Aim,
        Some(_) => return None,
        None if at.get("forward").is_some() => Placement::Forward {
            offset: at_n("forward")?,
        },
        None if at.get("end").is_some() => Placement::End {
            length: at_n("end")?,
        },
        None if at.get("landing").is_some() => Placement::Landing {
            length: at_n("landing")?,
        },
        None => return None,
    };
    Some((shape, placement))
}
/// Whether the shipped table already holds this assignment of the author's
/// own file, with its values as recorded: then the shipped one (whose sizes
/// follow the game's numbers) is used instead of the file's fixed copy.
pub fn ships_assignment(champion: &str, slot: usize, local: &Value) -> bool {
    let Some(shipped) = entry(champion, slot)
        .filter(|e| e.get("from").and_then(Value::as_str) == Some("assigned"))
        .and_then(|e| e.get("assigned"))
    else {
        return false;
    };
    // The shipped copy with every source at its recorded value.
    let recorded = resolved(shipped, &Live::default());
    let same = |a: &Value, b: &Value| match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => (x - y).abs() < 0.01,
        _ => a == b,
    };
    let (Some(l), Some(r)) = (local.as_object(), recorded.as_object()) else {
        return false;
    };
    l.len() == r.len() && l.iter().all(|(k, v)| r.get(k).is_some_and(|w| same(v, w)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill_preview::Footprint;
    use serde_json::json;

    fn table_of(entry: Value) -> HashMap<String, [Option<Value>; 3]> {
        parse(&json!({"champions": {"x": {"W": entry}}}).to_string())
    }
    fn geometry_of(entry: Value, live: &Live) -> Option<Geometry> {
        let t = table_of(entry);
        of_entry(t.get("x")?[1].as_ref()?, live)
    }
    fn first(g: &Geometry) -> Footprint {
        g.footprints[0]
    }

    #[test]
    fn sizes_follow_the_games_numbers_and_fall_back_to_their_recorded_values() {
        // Hammerer W: a circle 12 ahead, radius from attack_range.
        let entry = json!({"from": "live", "shapes": [{"kind": "circle",
            "radius": {"name": "attack_range", "recorded": 62},
            "at": {"forward": {"fixed": 12}}}]});
        let edited = HashMap::from([("attack_range".to_owned(), 150_000)]);
        let live = Live {
            numbers: Some(&edited),
            ..Live::default()
        };
        let g = geometry_of(entry.clone(), &live).unwrap();
        assert_eq!(first(&g).shape, Shape::Circle { radius: 150. });
        assert_eq!(first(&g).placement, Placement::Forward { offset: 12. });
        // Numbers unreadable: the recorded radius.
        let g = geometry_of(entry, &Live::default()).unwrap();
        assert_eq!(first(&g).shape, Shape::Circle { radius: 62. });
    }
    #[test]
    fn lengths_follow_the_live_cast_range_and_widths_scale() {
        let entry = json!({"from": "live", "shapes": [{"kind": "strip", "projectile": true,
            "radius": {"name": "width", "times": 0.5, "recorded": 20},
            "length": {"cast_range": true, "recorded": 65}, "at": "caster"}]});
        let numbers = HashMap::from([("width".to_owned(), 30_000)]);
        let live = Live {
            numbers: Some(&numbers),
            cast_range: Some(70.),
            stack: 0,
        };
        let g = geometry_of(entry, &live).unwrap();
        assert_eq!(
            first(&g).shape,
            Shape::Corridor {
                radius: 15.,
                length: 70.
            }
        );
        assert!(first(&g).projectile);
    }
    #[test]
    fn dancer_blades_grow_with_her_stack_up_to_the_most() {
        let entry = json!({"from": "live", "shapes": [{"kind": "fan", "count": {"fixed": 4},
            "max_count": {"fixed": 10}, "radius": {"fixed": 10}, "length": {"cast_range": true},
            "at": "caster"}]});
        for (stack, blades) in [(0, 4), (3, 7), (30, 10)] {
            let live = Live {
                cast_range: Some(130.),
                stack,
                ..Live::default()
            };
            let g = geometry_of(entry.clone(), &live).unwrap();
            assert_eq!(
                first(&g).shape,
                Shape::Fan {
                    count: blades,
                    radius: 10.,
                    length: 130.
                }
            );
        }
    }
    #[test]
    fn assignments_take_their_sources_too() {
        let entry = json!({"from": "assigned", "assigned": {"type": "aim_circle",
            "radius": {"name": "attack_range", "recorded": 30}}});
        let numbers = HashMap::from([("attack_range".to_owned(), 33_000)]);
        let live = Live {
            numbers: Some(&numbers),
            ..Live::default()
        };
        let g = geometry_of(entry, &live).unwrap();
        assert_eq!(first(&g).shape, Shape::Circle { radius: 33. });
        assert_eq!(first(&g).placement, Placement::Aim);
    }
    #[test]
    fn a_custom_preview_is_drawn_exactly_as_listed() {
        // Bulldozer Q built from pieces: the box, the hit line of the charge
        // and the charge's own path to its end, 84 = speed 4200 x 20 ticks.
        let entry = json!({"from": "custom", "pieces": [
            {"kind": "shot", "radius": {"name": "width", "recorded": 60}, "length": {"fixed": 30},
                "pierce": true, "at": "caster"},
            {"kind": "strip", "radius": {"fixed": 10}, "length": {"name": "rush_speed", "by": "ticks",
                "recorded": 84}, "at": "caster"},
            {"kind": "dash", "at": {"end": {"name": "rush_speed", "by": "ticks", "recorded": 84}}}]});
        let numbers = HashMap::from([
            ("width".to_owned(), 70_000),
            ("rush_speed".to_owned(), 4_500),
            ("ticks".to_owned(), 20),
        ]);
        let live = Live {
            numbers: Some(&numbers),
            ..Live::default()
        };
        let g = geometry_of(entry.clone(), &live).unwrap();
        assert!(g.exact);
        assert!(g.families.iter().any(|f| f == ASSIGNMENT));
        assert_eq!(g.footprints.len(), 3);
        assert_eq!(
            g.footprints[0].shape,
            Shape::Shot {
                radius: 70.,
                length: 30.,
                pierce: true
            }
        );
        // Two numbers multiplied follow both: 4.5 x 20 = 90.
        assert_eq!(
            g.footprints[1].shape,
            Shape::Corridor {
                radius: 10.,
                length: 90.
            }
        );
        assert_eq!(g.footprints[2].placement, Placement::End { length: 90. });
        // The dash stays beside the areas: nothing is hidden by the usual
        // main-purpose rule.
        let kept = crate::skill_preview::main_purpose(g, false);
        assert_eq!(kept.footprints.len(), 3);
        // Numbers not read yet: the recorded values.
        let g = geometry_of(entry, &Live::default()).unwrap();
        assert_eq!(g.footprints[2].placement, Placement::End { length: 84. });
        // An empty list draws nothing but still wins over what is read.
        let none = geometry_of(json!({"from": "custom", "pieces": []}), &Live::default()).unwrap();
        assert!(none.footprints.is_empty() && none.families.iter().any(|f| f == ASSIGNMENT));
        // The names the mod asks the game for include both factors.
        let mut names = Vec::new();
        collect_names(
            &json!({"pieces": [{"length": {"name": "rush_speed", "by": "ticks",
            "recorded": 84}}]}),
            &mut names,
        );
        assert_eq!(names, ["rush_speed", "ticks"]);
    }
    #[test]
    fn names_and_recorded_assignments_are_found() {
        let mut names = Vec::new();
        collect_names(
            &json!({"shapes": [{"radius": {"name": "a", "recorded": 1}, "length": {"cast_range": true},
                "count": {"name": "c", "unit": "count"}}], "kind": "x_skill"}),
            &mut names,
        );
        names.sort();
        assert_eq!(names, ["a", "c"]);
        let shipped = json!({"type": "line", "width": {"name": "w", "times": 2, "recorded": 16},
            "length": {"fixed": 70}});
        let recorded = resolved(&shipped, &Live::default());
        assert_eq!(
            recorded,
            json!({"type": "line", "width": 16.0, "length": 70.0})
        );
    }
    #[test]
    fn the_shipped_table_parses() {
        // Whatever the tool last wrote must be readable by the mod.
        let t = parse(SHIPPED);
        assert!(serde_json::from_str::<Value>(SHIPPED).is_ok());
        for slots in t.values() {
            for e in slots.iter().flatten() {
                assert!(e.get("from").and_then(Value::as_str).is_some());
            }
        }
    }
}
