//! Skill shapes as JSON, in the form the preview tools read
//! (`research/skill-shapes.json`, `file-shapes.json`) and the shipped table
//! is built from: `{"kind", sizes in world units, "at", "allies",
//! "projectile"}` per shape. Shared by the research recorder
//! (`shape_record`) and the start-up file reader's report (1.5.2).
use crate::skill_preview::{Footprint, Geometry, Placement, Shape};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

/// One skill: its shapes, or why it could not be read.
pub fn skill(g: &Geometry, source: &str) -> Value {
    if !g.footprints.is_empty() {
        json!({"source": source, "shapes": g.footprints.iter().map(footprint).collect::<Vec<_>>()})
    } else if g.known_without_area() {
        json!({"source": source, "shapes": [], "no_area": true})
    } else {
        json!({"source": source, "unread": g.issues.first().cloned().unwrap_or_default()})
    }
}
pub fn footprint(f: &Footprint) -> Value {
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
        // `hit_radius`: the radius is the hit's distance on each side of the
        // line. Shots recorded before 1.5.2-dev1 lack it, and those from the
        // game's line hit hold half of it (tools/preview_sources.py
        // `widened` doubles them).
        Shape::Shot {
            radius,
            length,
            pierce,
        } => json!({"kind": "shot", "radius": r(radius), "length": r(length), "pierce": pierce,
            "hit_radius": true}),
        Shape::Reach { extra } => json!({"kind": "reach", "extra": r(extra)}),
        Shape::Capsule { radius, length } => {
            json!({"kind": "strip", "radius": r(radius), "length": r(length), "rounded": true})
        }
        Shape::Fan {
            count,
            radius,
            length,
            rounded,
        } => {
            let mut v =
                json!({"kind": "fan", "count": count, "radius": r(radius), "length": r(length)});
            if rounded {
                v["rounded"] = json!(true);
            }
            v
        }
        Shape::Spread {
            count,
            radius,
            length,
            degrees,
            rounded,
        } => {
            let mut v = json!({"kind": "spread", "count": count, "radius": r(radius),
                "length": r(length), "degrees": r(degrees)});
            if rounded {
                v["rounded"] = json!(true);
            }
            v
        }
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

/// What the champion-file reader found for every champion it read at start
/// (`skill_preview::initialize`): `file-shapes.json` in the mod's folder, so
/// the preview viewer shows what the game draws for champions no match has
/// recorded (the author's choice A, 2026-10-22: one reader, the mod's own).
/// Written whole or not at all.
pub fn write_file_shapes(
    path: &Path,
    game: &str,
    shapes: &HashMap<String, [Geometry; 3]>,
) -> std::io::Result<()> {
    let mut names: Vec<_> = shapes.keys().collect();
    names.sort();
    let champions: serde_json::Map<String, Value> = names
        .into_iter()
        .map(|name| {
            let [q, w, r] = &shapes[name];
            (
                name.clone(),
                json!({"Q": skill(q, "file"), "W": skill(w, "file"), "R": skill(r, "file")}),
            )
        })
        .collect();
    let report = json!({
        "about": "What the mod's champion-file reader found at game start, per champion and skill (sizes in world units): the shapes the game draws when no shipped preview or recording covers a skill. Written by the mod at every start; read by tools/preview_viewer.py.",
        "game": game,
        "mod": env!("CARGO_PKG_VERSION"),
        "champions": champions,
    });
    let text = serde_json::to_string_pretty(&report).map_err(std::io::Error::other)?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, text).and_then(|()| std::fs::rename(&temp, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_report_holds_every_champion_and_skill() {
        let mut jhin = [
            Geometry::default(),
            Geometry::default(),
            Geometry::default(),
        ];
        jhin[0].add(Shape::Circle { radius: 35. }, Placement::Aim);
        jhin[1].family("AddCasterBuff");
        jhin[2].issue("opaque Native");
        let folder = std::env::temp_dir().join(format!("ltdc-file-shapes-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("file-shapes.json");
        write_file_shapes(
            &path,
            "0.6.5",
            &HashMap::from([("test_mod_jhin".to_owned(), jhin)]),
        )
        .unwrap();
        let v: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(v["game"], json!("0.6.5"));
        let q = &v["champions"]["test_mod_jhin"]["Q"];
        assert_eq!(q["source"], json!("file"));
        assert_eq!(
            q["shapes"][0],
            json!({"kind": "circle", "radius": 35.0, "at": "aim"})
        );
        assert_eq!(v["champions"]["test_mod_jhin"]["W"]["no_area"], json!(true));
        assert_eq!(
            v["champions"]["test_mod_jhin"]["R"]["unread"],
            json!("opaque Native")
        );
        assert!(!path.with_extension("json.tmp").exists());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
