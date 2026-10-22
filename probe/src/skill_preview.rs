//! Skill previews: geometry from the game's effect data, drawn in the
//! user's Endfield x League design (kept outside git). Every colour and
//! size comes from crate::preview_style; this file only places the pieces.
//! Live effect families supply footprints; explicit declarations are a fallback.
//! Unsupported effects retain a reach/aim guide, without guessed hits.
use crate::preview_style::{alpha, style, Style};
use crate::{
    camera::{CameraFrame, Rect},
    hud_icons, Logger,
};
use mod_api_stable::StableClient;
use serde_json::Value;
use std::{collections::HashMap, sync::OnceLock};

type Point = (f32, f32);
/// Input acquisition and area placement are deliberately independent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    Caster,
    Aim,
    Forward {
        offset: f32,
    },
    /// Where a projectile ends: the target unit for unit-targeted casts,
    /// otherwise its full flight length toward the aim.
    End {
        length: f32,
    },
    /// Where a projectile that hits nothing in flight lands: the target for
    /// unit casts, the aimed point (within its flight length) for location
    /// casts, its full flight length for direction casts.
    Landing {
        length: f32,
    },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Circle {
        radius: f32,
    },
    Corridor {
        radius: f32,
        length: f32,
    },
    Rectangle {
        width: f32,
        height: f32,
    },
    Cone {
        radius: f32,
        cosine: f32,
    },
    /// Native Line contains absolute world endpoints, not a cast-relative vector.
    Segment {
        radius: f32,
        from: Point,
        to: Point,
    },
    Movement {
        blink: bool,
    },
    /// A shot from the caster of hit radius `radius` over `length`, known to
    /// pass through units (`pierce`) or to stop at the first one.
    Shot {
        radius: f32,
        length: f32,
        pierce: bool,
    },
    /// A ring around the caster at its basic attack range plus `extra`
    /// (Bombardier Q's cannon mode). Drawn only while that range is known.
    Reach {
        extra: f32,
    },
    /// Dancer R: `count` blades from the caster, each `length` long with hit
    /// radius `radius`, spread by the game's rule (`fan_offsets`).
    Fan {
        count: usize,
        radius: f32,
        length: f32,
    },
    /// `count` lines from the caster spread evenly over `degrees` in total,
    /// each with hit radius `radius` over `length` (0: the cast range);
    /// from the author's skill assignments (Harpooner R, Boomerang Hunter W).
    Spread {
        count: usize,
        radius: f32,
        length: f32,
        degrees: f32,
    },
    /// A jump straight back, away from the aim, `length` long (Archer W).
    Backstep {
        length: f32,
    },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footprint {
    pub shape: Shape,
    pub placement: Placement,
    /// The area affects allies (a heal, a shield), not enemies: drawn in
    /// the ally colour. False when it affects enemies, both, or is unknown.
    pub allies: bool,
    /// A projectile: the stat rules widen its drawn corridor to the width it
    /// registers (the radius plus the fixed hit margin on each side).
    pub projectile: bool,
}
/// The game's `CastingTarget` codes 0-4 are the ally targets (Ally,
/// AllyChampion, AllyChampionInCc, AllyNotSelf, AllyOnlySelf); 5-9 enemies,
/// 10-12 both, 13 none (mod-api-stable `CastingTargetV1`).
pub fn targets_allies(code: usize) -> bool {
    code as u32 <= 4
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Geometry {
    pub footprints: Vec<Footprint>,
    pub issues: Vec<String>,
    pub families: Vec<String>,
    /// Draw nothing for this skill, not even its range (the author's "no
    /// preview" assignment: passives and instant triggers).
    pub hidden: bool,
    /// An assignment that adds its shapes to what the game reads instead of
    /// replacing it (the author, 2026-10-21: Zook W keeps its read blast
    /// around Zook and adds the jump away the game cannot be read for).
    pub adds: bool,
    /// The author's custom preview (pieces built in the viewer, 1.5.2-dev2):
    /// drawn exactly as listed, so `main_purpose` leaves it alone.
    pub exact: bool,
}
impl Geometry {
    pub fn add(&mut self, shape: Shape, placement: Placement) {
        self.add_for(shape, placement, false);
    }
    /// A shape that is the path of a projectile (see `Footprint::projectile`).
    pub fn add_projectile(&mut self, shape: Shape, placement: Placement) {
        self.push(shape, placement, false, true);
    }
    pub fn add_for(&mut self, shape: Shape, placement: Placement, allies: bool) {
        self.push(shape, placement, allies, false);
    }
    pub(crate) fn push(
        &mut self,
        shape: Shape,
        placement: Placement,
        allies: bool,
        projectile: bool,
    ) {
        let footprint = Footprint {
            shape,
            placement,
            allies,
            projectile,
        };
        if !self.footprints.contains(&footprint) && self.footprints.len() < 8 {
            self.footprints.push(footprint);
        }
    }
    pub fn issue(&mut self, reason: impl Into<String>) {
        let reason = reason.into();
        if !self.issues.contains(&reason) && self.issues.len() < 8 {
            self.issues.push(reason);
        }
    }
    pub fn family(&mut self, name: impl Into<String>) {
        let name = name.into();
        if !self.families.contains(&name) && self.families.len() < 16 {
            self.families.push(name);
        }
    }
    fn self_area(&self) -> bool {
        !self.footprints.is_empty()
            && self.footprints.iter().all(|f| {
                f.placement == Placement::Caster
                    && !matches!(
                        f.shape,
                        Shape::Corridor { .. }
                            | Shape::Shot { .. }
                            | Shape::Cone { .. }
                            | Shape::Movement { .. }
                            | Shape::Fan { .. }
                            | Shape::Spread { .. }
                            | Shape::Backstep { .. }
                    )
            })
    }
    /// One word for a coverage report: the shapes found ("strip+circle"),
    /// "no-area" when every effect is recognised and none has one, else
    /// "unknown:" and the first reason.
    pub fn summary(&self) -> String {
        if !self.footprints.is_empty() {
            let mut kinds: Vec<&str> = Vec::new();
            for f in &self.footprints {
                let kind = match f.shape {
                    Shape::Circle { .. } if f.allies => "ally-circle",
                    Shape::Circle { .. } => "circle",
                    Shape::Corridor { .. } | Shape::Shot { .. } => "strip",
                    Shape::Rectangle { .. } => "rectangle",
                    Shape::Cone { .. } => "cone",
                    Shape::Segment { .. } => "segment",
                    Shape::Movement { blink: true } => "blink",
                    Shape::Movement { blink: false } => "dash",
                    Shape::Reach { .. } => "reach",
                    Shape::Fan { .. } | Shape::Spread { .. } => "fan",
                    Shape::Backstep { .. } => "backstep",
                };
                if !kinds.contains(&kind) {
                    kinds.push(kind);
                }
            }
            kinds.join("+")
        } else if self.known_without_area() {
            "no-area".into()
        } else {
            let reason = self.issues.first().map_or("nothing read", String::as_str);
            format!("unknown:{}", reason.replace(' ', "_"))
        }
    }
    /// Every effect was recognised and none has an area (a self buff, a
    /// single-target shot): no direction guide is drawn for it.
    pub fn known_without_area(&self) -> bool {
        self.footprints.is_empty() && self.issues.is_empty() && !self.families.is_empty()
    }
}
/// A declared effect's `target` (or `applied_target`) names an ally group.
fn declared_allies(v: &Value) -> bool {
    ["target", "applied_target"]
        .iter()
        .filter_map(|key| v.get(key).and_then(Value::as_str))
        .any(|name| name.starts_with("Ally"))
}
static SHAPES: OnceLock<HashMap<String, [Geometry; 3]>> = OnceLock::new();
/// The controlled champion's basic attack range in thousandths of a world
/// unit (0 = unknown), for `Shape::Reach`. Written by the movement module.
static ATTACK_RANGE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// The controlled champion is in the mode its reach skill toggles, so its
/// attack range already holds the extra: the version's move-blocking
/// status (Beta 2: Bombardier deployed). Written with the attack range.
static REACH_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn set_attack_range(range: Option<u64>) {
    ATTACK_RANGE.store(range.unwrap_or(0), std::sync::atomic::Ordering::Relaxed);
    if range.is_none() {
        set_reach_active(false);
    }
}
pub fn set_reach_active(active: bool) {
    REACH_ACTIVE.store(active, std::sync::atomic::Ordering::Relaxed);
}
/// The reach ring's radius: the attack range after the cast. Not in the
/// mode: the range plus the extra (deploying adds it). In the mode: the
/// range already holds the extra, and casting again takes it away (the
/// author, 2026-10-22, option A).
fn reach_radius(range: f32, extra: f32, active: bool) -> f32 {
    if active {
        (range - extra).max(0.)
    } else {
        range + extra
    }
}
fn attack_range() -> Option<f32> {
    let range = ATTACK_RANGE.load(std::sync::atomic::Ordering::Relaxed);
    (range > 0).then_some(range as f32 / 1000.)
}
/// The game's own data-defined base champions (`asset/base/setting/<id>`,
/// `data_champion`; 8 on 0.6.4), read from the installed bundle so they
/// follow game updates (the author, 2026-10-20, stage 2). Empty when the
/// bundle cannot be read. The same pass hands the bundle's balance patch
/// rules (`patch_setting`) to `patch_fields`.
#[cfg_attr(not(feature = "research"), allow(unused_variables))]
fn bundle_champions(log: &Logger) -> Vec<(String, Value)> {
    let Some(bundle) = crate::game_bundle::installed() else {
        return Vec::new();
    };
    let mut champions = Vec::new();
    for (path, bytes) in crate::game_bundle::files(&bundle, |extension, path| {
        (extension == "data_champion"
            || (cfg!(feature = "research") && extension == "patch_setting"))
            && path.starts_with("asset/base/")
    }) {
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        if path.ends_with("/patch_setting") {
            #[cfg(feature = "research")]
            crate::patch_fields::set_rules(&value, log);
        } else if let Some(id) = value.get("id").and_then(Value::as_str) {
            champions.push((id.to_owned(), value));
        }
    }
    champions
}
pub fn initialize(log: &Logger) {
    let mut shapes = HashMap::new();
    let mut base = bundle_champions(log);
    let source = if base.is_empty() {
        // The copy taken by `tools/read_preview_assets.py` at build time.
        let baked: Value = serde_json::from_str(include_str!("preview_assets.json"))
            .expect("preview declarations");
        base = baked
            .as_object()
            .expect("preview map")
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        "built-in copy (bundle unreadable)"
    } else {
        "game bundle"
    };
    base.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, value) in base {
        let geometry = skills(&value);
        log.write(&format!(
            "PREVIEW BASE champion={name} source={source} Q={} W={} R={}",
            geometry[0].summary(),
            geometry[1].summary(),
            geometry[2].summary()
        ));
        shapes.insert(name, geometry);
    }
    // One line per enabled mod champion: what its file gives each skill
    // (checking coverage needs no description from the player).
    let mut files: Vec<_> = hud_icons::enabled_assets()
        .descriptions
        .into_iter()
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, value) in files {
        let geometry = skills(&value);
        log.write(&format!(
            "PREVIEW FILE champion={name} Q={} W={} R={}",
            geometry[0].summary(),
            geometry[1].summary(),
            geometry[2].summary()
        ));
        shapes.insert(name, geometry);
    }
    let count = shapes
        .values()
        .flatten()
        .filter(|s| !s.footprints.is_empty())
        .count();
    log.write(&format!("PREVIEW declared_slots={count}; runtime effect geometry preferred; input and footprint independent"));
    let _ = SHAPES.set(shapes);
    let (champions, skills) = crate::preview_table::counts();
    log.write(&format!(
        "PREVIEW TABLE champions={champions} skills={skills}; sizes from the game's numbers by name"
    ));
    let _ = OVERRIDES.set(load_overrides(log));
}
/// The author's skill assignments (step 3 of the preview plan, 2026-10-21):
/// `%LOCALAPPDATA%\LTDirectControl\skill-overrides.json`, written by the
/// viewer (`tools/preview_viewer.py`). Keyed "<champion> <Q|W|R>".
static OVERRIDES: OnceLock<HashMap<(String, usize), Geometry>> = OnceLock::new();
fn load_overrides(log: &Logger) -> HashMap<(String, usize), Geometry> {
    let path = std::env::var_os("LOCALAPPDATA").map(|root| {
        std::path::PathBuf::from(root)
            .join("LTDirectControl")
            .join("skill-overrides.json")
    });
    let Some(value) = path
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
    else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    let mut custom = HashMap::new();
    let (mut refused, mut shipped) = (0, 0);
    for (key, entry) in value
        .get("skills")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        let skill = key.rsplit_once(' ').and_then(|(champion, slot)| {
            let slot = ["Q", "W", "R"].iter().position(|s| *s == slot)?;
            Some((champion.to_owned(), slot))
        });
        // A custom preview (pieces with their size sources) goes through
        // the shipped table's path, so its sizes follow the game's numbers
        // like the table's; the author's file wins over the table.
        if entry.get("type").and_then(Value::as_str) == Some("custom") {
            match (skill, entry.get("pieces").filter(|p| p.is_array())) {
                (Some(k), Some(pieces)) => {
                    custom.insert(k, pieces.clone());
                }
                _ => refused += 1,
            }
            continue;
        }
        // The shipped table holds this assignment with sizes that follow
        // the game's numbers: that copy is used (the author's own file).
        if let Some((champion, slot)) = &skill {
            if crate::preview_table::ships_assignment(champion, *slot, entry) {
                shipped += 1;
                continue;
            }
        }
        match skill.and_then(|k| Some((k, assigned(entry)?))) {
            Some((k, g)) => {
                out.insert(k, g);
            }
            None => refused += 1,
        }
    }
    log.write(&format!(
        "PREVIEW OVERRIDES skills={} custom={} refused={refused} shipped={shipped}",
        out.len(),
        custom.len()
    ));
    crate::preview_table::set_custom(custom);
    out
}
/// One assignment as a preview: a type and its values in world units.
pub fn assigned(v: &Value) -> Option<Geometry> {
    let n = |name: &str| {
        v.get(name)
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite() && *n >= 0. && *n <= 10_000.)
            .map(|n| n as f32)
    };
    let allies = v.get("allies").and_then(Value::as_bool).unwrap_or(false);
    let mut g = Geometry::default();
    g.family("override");
    g.adds = v.get("add").and_then(Value::as_bool).unwrap_or(false);
    match v.get("type")?.as_str()? {
        "no_preview" => g.hidden = true,
        "unit_target" => {
            if let Some(radius) = n("area_radius").filter(|r| *r > 0.) {
                g.add_for(Shape::Circle { radius }, Placement::Aim, allies);
            }
        }
        "self_circle" => g.add_for(
            Shape::Circle {
                radius: n("radius")?,
            },
            Placement::Caster,
            allies,
        ),
        "aim_circle" => g.add_for(
            Shape::Circle {
                radius: n("radius")?,
            },
            Placement::Aim,
            allies,
        ),
        "aim_rectangle" => g.add_for(
            Shape::Rectangle {
                width: n("width")?,
                height: n("depth")?,
            },
            Placement::Aim,
            allies,
        ),
        "line" => g.add_for(
            Shape::Corridor {
                radius: n("width")? / 2.,
                length: n("length").unwrap_or(0.),
            },
            Placement::Caster,
            allies,
        ),
        "fan" => g.add_for(
            Shape::Spread {
                count: n("count")?.round().clamp(1., 16.) as usize,
                radius: n("width")? / 2.,
                length: n("length").unwrap_or(0.),
                degrees: n("spread")?.min(360.),
            },
            Placement::Caster,
            allies,
        ),
        "cone" => g.add_for(
            Shape::Cone {
                radius: n("radius")?,
                cosine: (n("angle")?.min(360.) / 2.).to_radians().cos(),
            },
            Placement::Caster,
            allies,
        ),
        "dash" => {
            let blink = v.get("blink").and_then(Value::as_bool).unwrap_or(false);
            let placement = match n("distance").filter(|d| *d > 0.) {
                Some(length) => Placement::End { length },
                None => Placement::Aim,
            };
            g.add(Shape::Movement { blink }, placement);
        }
        "jump_back" => g.add(
            Shape::Backstep {
                length: n("distance")?,
            },
            Placement::Caster,
        ),
        // A ring at the caster's attack range plus `extra` (Bombardier Q).
        "attack_reach" => g.add(Shape::Reach { extra: n("extra")? }, Placement::Caster),
        _ => return None,
    }
    Some(g)
}
/// The author's assignment for this champion's skill, if any.
pub fn assignment(name: Option<&str>, slot: usize) -> Option<Geometry> {
    OVERRIDES.get()?.get(&(name?.to_owned(), slot)).cloned()
}
/// The skill's preview with the author's assignment applied: a replacing
/// assignment wins over everything read (a deliberate correction); an
/// adding one is drawn on top of what `read` gives.
pub fn with_assignment(
    name: Option<&str>,
    slot: usize,
    read: impl FnOnce() -> Geometry,
) -> Geometry {
    combine(assignment(name, slot), read)
}
fn combine(assigned: Option<Geometry>, read: impl FnOnce() -> Geometry) -> Geometry {
    match assigned {
        Some(a) if a.adds => {
            let mut g = read();
            for f in a.footprints {
                g.push(f.shape, f.placement, f.allies, f.projectile);
            }
            g.family("override");
            g.hidden |= a.hidden;
            g
        }
        Some(a) => a,
        None => read(),
    }
}
pub fn geometry(name: Option<&str>, slot: usize) -> Geometry {
    name.and_then(|n| SHAPES.get()?.get(n)?.get(slot).cloned())
        .unwrap_or_default()
}
/// Recognized runtime footprints are authoritative, even if sibling effects are
/// unresolved. A file declaration can fill an opaque tree, not replace live dimensions.
/// An assignment from the shipped table wins even when it draws nothing
/// ("no preview", or a unit target without an area).
pub fn resolve(native: Option<&Geometry>, declared: Geometry) -> Geometry {
    match native {
        Some(g) if !g.footprints.is_empty() || g.hidden => g.clone(),
        Some(g)
            if g.families
                .iter()
                .any(|f| f == crate::preview_table::ASSIGNMENT) =>
        {
            g.clone()
        }
        Some(g) if !declared.footprints.is_empty() => {
            let mut result = declared;
            for reason in &g.issues {
                result.issue(format!("runtime: {reason}"));
            }
            result
        }
        Some(g) => g.clone(),
        None => declared,
    }
}
/// What a preview shows of a skill that moves the caster, by the skill's
/// main purpose (the author, 2026-10-20, from League: Zac R shows no jump
/// path, Corki W shows its dash):
/// - cast on a unit: no path; the range and the target cue say it all;
/// - a dash that hits along its way: its corridor (kept: not a `Movement`);
/// - a dash that does nothing else: the path, since moving is the point;
/// - a dash that leaves areas (Candygel R's pools): the areas only.
///
/// A projectile beside a dash is not an area left behind: both stay.
pub fn main_purpose(mut geometry: Geometry, unit_target: bool) -> Geometry {
    if geometry.exact {
        return geometry;
    }
    let leaves_areas = geometry
        .footprints
        .iter()
        .any(|f| !f.projectile && !matches!(f.shape, Shape::Movement { .. }));
    if unit_target || leaves_areas {
        geometry
            .footprints
            .retain(|f| !matches!(f.shape, Shape::Movement { .. }));
    }
    geometry
}
fn number(v: &Value, name: &str) -> Option<f32> {
    let n = v.get(name)?.as_f64()?;
    (n.is_finite() && n > 0. && n <= 1_000_000.).then_some(n as f32 / 1000.)
}
fn scalar(v: &Value, name: &str) -> Option<f32> {
    let n = v.get(name)?.as_f64()?;
    (n.is_finite() && n.abs() <= 1_000_000.).then_some(n as f32 / 1000.)
}
fn declared_shape(v: &Value) -> Option<Shape> {
    let shape = v.get("shape")?;
    if let Some(c) = shape.get("Circle") {
        return Some(Shape::Circle {
            radius: number(c, "radius")?,
        });
    }
    if let Some(r) = shape.get("Rect") {
        return Some(Shape::Rectangle {
            width: number(r, "width")?,
            height: number(r, "height")?,
        });
    }
    if let Some(c) = shape.get("DirDot") {
        let cosine = scalar(c, "range")?;
        if !(-1. ..=1.).contains(&cosine) {
            return None;
        }
        return Some(Shape::Cone {
            radius: number(c, "radius")?,
            cosine,
        });
    }
    if let Some(l) = shape.get("Line") {
        return Some(Shape::Segment {
            radius: number(l, "width")?,
            from: (scalar(l, "from_x")?, scalar(l, "from_y")?),
            to: (scalar(l, "to_x")?, scalar(l, "to_y")?),
        });
    }
    None
}
fn skills(v: &Value) -> [Geometry; 3] {
    ["skill", "skill2", "ult"].map(|key| {
        let mut geometry = Geometry::default();
        let Some(spec) = v.get(key) else {
            return geometry;
        };
        let inherited = if spec.get("casting_type").and_then(Value::as_str) == Some("None") {
            Placement::Caster
        } else {
            Placement::Aim
        };
        if let Some(effect) = spec.get("effect") {
            collect(effect, inherited, &mut geometry, 0);
        }
        geometry
    })
}
fn collect(v: &Value, inherited: Placement, out: &mut Geometry, depth: usize) {
    if depth > 12 || out.footprints.len() >= 8 {
        out.issue("declaration tree limit");
        return;
    }
    let Some(kind) = v.get("type").and_then(Value::as_str) else {
        out.issue("missing effect type");
        return;
    };
    out.family(kind);
    match kind {
        "Combine" => {
            if let Some(list) = v
                .get("effects")
                .and_then(Value::as_array)
                .filter(|s| s.len() <= 32)
            {
                collect_list(list, inherited, out, depth);
            } else {
                out.issue("invalid Combine children");
            }
        }
        "Delayed" | "Animated" | "Animation" | "WithSelf" => {
            let placement = if kind == "WithSelf" {
                Placement::Caster
            } else {
                inherited
            };
            if let Some(e) = v.get("effect") {
                collect(e, placement, out, depth + 1);
            } else if let Some(list) = v
                .get("effects")
                .and_then(Value::as_array)
                .filter(|s| s.len() <= 32)
            {
                collect_list(list, placement, out, depth);
            } else {
                out.issue(format!("unsupported {kind} wrapper"));
            }
        }
        "LinearProjectile" | "BackToCasterLinearProjectile" => {
            if let (Some(Shape::Circle { radius }), Some(length)) =
                (declared_shape(v), number(v, "range"))
            {
                out.add_projectile(Shape::Corridor { radius, length }, Placement::Caster);
            } else {
                out.issue("unsupported linear projectile shape");
            }
        }
        "RangeEffect" => {
            let placement = match v.get("apply_type") {
                Some(Value::String(s)) if s == "AroundCaster" => Some(Placement::Caster),
                Some(Value::String(s)) if s == "AroundTarget" => Some(Placement::Aim),
                Some(Value::Object(o)) => o
                    .get("Foward")
                    .or_else(|| o.get("Forward"))
                    .and_then(|v| scalar(v, "offset"))
                    .map(|offset| Placement::Forward { offset }),
                _ => None,
            };
            if let (Some(shape), Some(placement)) = (declared_shape(v), placement) {
                if matches!(shape, Shape::Cone { .. }) && placement == Placement::Caster {
                    out.issue("DirDot facing unresolved at caster center");
                } else {
                    out.add_for(shape, placement, declared_allies(v));
                }
            } else {
                out.issue("RangeEffect placement/shape unresolved");
            }
            // Applied descendants are effects on hit units, not additional primary footprints.
        }
        "RangeProjectile" | "RangePeriodProjectile" => {
            if let Some(shape) = declared_shape(v) {
                out.add_for(shape, inherited, declared_allies(v));
            } else {
                out.issue("range projectile shape unresolved");
            }
        }
        // A line hit from the caster toward the aim (Workshop examples:
        // abbie W, `width` 27000 and `length` 100000; bulldozer Q). Read
        // from the game's code (0.6.4 and 0.6.5, 1.5.2-dev1): `width` is
        // the hit's distance on each side of the line (the radius of the
        // game's segment shape), so the line is twice `width` wide. Before
        // 1.5.2-dev1 this guessed `width` was the full width.
        "LineRangeProjectile" => match (number(v, "width"), number(v, "length")) {
            (Some(radius), Some(length)) => out.add_for(
                Shape::Corridor { radius, length },
                Placement::Caster,
                declared_allies(v),
            ),
            _ => out.issue("LineRangeProjectile width/length missing"),
        },
        // Movers end where the caster lands, so their `end_effects` drawn
        // "around the caster" belong at the aim. MoveToTarget dashes to the
        // target unit; RushTime and Rush charge toward the aim; RushMoveToBack
        // dashes to the target (its `applied_effects` hit that target only).
        "MoveTo" | "Teleport" | "MoveToTarget" | "Rush" | "RushTime" | "RushMoveToBack" => {
            out.add(
                Shape::Movement {
                    blink: kind == "Teleport",
                },
                Placement::Aim,
            );
            landed(v, out, depth);
        }
        // A lob that lands at the target; its `end_effects` happen there.
        // Its own `shape` is the area it hits on landing when effects are
        // attached (Workshop example: bulldozer R, a 120000 circle that
        // damages and stuns, seen in game by the author 2026-10-20), and
        // only a flight marker otherwise (league_annie R: radius 1, no
        // effects).
        "ParabolicProjectile" => {
            let hits = v
                .get("applied_effects")
                .and_then(Value::as_array)
                .is_some_and(|e| !e.is_empty());
            if let Some(Shape::Circle { radius }) = declared_shape(v).filter(|_| hits) {
                if radius > LOB_MARKER {
                    out.add_for(Shape::Circle { radius }, inherited, declared_allies(v));
                }
            }
            landed(v, out, depth);
        }
        // A shot at the target unit that bounces on to further units within
        // `range` of the unit it just hit, like League's Jhin Q (the author,
        // 2026-10-20, of voidling Q: 45000 on a 90000 cast; cf_void_mage W
        // 30000 on 60000). Later bounces cannot be foreseen, so the circle is
        // the bounce range around the first target.
        "TargetSplashProjectile" => match number(v, "range") {
            Some(radius) => out.add_for(Shape::Circle { radius }, Placement::Aim, declared_allies(v)),
            None => out.issue("TargetSplashProjectile radius missing"),
        },
        // Two variants by a buff on the caster (Workshop example: heavyblade
        // Q, a larger circle while charged). The preview cannot know the
        // buff, so it shows the usual state, `effect_none`.
        "SwitchByBuff" => match v.get("effect_none").or_else(|| v.get("effect_buff")) {
            Some(e) => collect(e, inherited, out, depth + 1),
            None => out.issue("SwitchByBuff without variants"),
        },
        // Astrologer (player's description, 1.1.19): Q and R land a circle
        // at the aim after a delay; W is a line from her toward the aim, as
        // long as the cast range and `line_width` wide.
        "Native"
            if matches!(
                v.get("effect_ref").and_then(Value::as_str),
                Some("astrologer_skill1" | "astrologer_ult")
            ) && number(v, "radius").is_some() =>
        {
            out.add(
                Shape::Circle {
                    radius: number(v, "radius").unwrap_or(0.),
                },
                Placement::Aim,
            );
        }
        "Native"
            if v.get("effect_ref").and_then(Value::as_str) == Some("astrologer_skill2")
                && number(v, "line_width").is_some() =>
        {
            out.add(
                Shape::Corridor {
                    radius: number(v, "line_width").unwrap_or(0.) / 2.,
                    length: 0.,
                },
                Placement::Caster,
            );
        }
        // A hand-written effect is opaque, but one that names a single
        // projectile's hit radius is a shot from the caster over its `range`
        // (the cast range when it has none): Crossbowman, Harpooner Q and W.
        "Native"
            if v.get("projectile_count").is_none() && number(v, "projectile_radius").is_some() =>
        {
            out.add_projectile(
                Shape::Corridor {
                    radius: number(v, "projectile_radius").unwrap_or(0.),
                    length: number(v, "range").unwrap_or(0.),
                },
                Placement::Caster,
            );
        }
        "Native" | "ShrinkingBarrier" => out.issue(format!("opaque {kind}")),
        "Attack"
        | "ApAttack"
        | "FixedAttack"
        | "Heal"
        | "Shield"
        | "AddBuff"
        | "AddCasterBuff"
        | "RemoveCasterBuff"
        | "AddStatScaledBuff"
        | "AddCasted"
        | "Sfx"
        | "TargetSfx"
        | "ViewEffect"
        | "CasterViewEffect"
        | "CasterAnimation"
        | "RemoveCasterAnimation"
        | "Stun"
        | "Bind"
        | "Slow"
        | "Airborne"
        | "Knockback"
        | "Pull"
        | "Fear"
        | "Charm"
        | "BlockSkill"
        | "BlockMoveSkill"
        | "BlockAttack"
        | "Invisible"
        | "CasterInvisible"
        | "TargetProjectile"
        | "MoveBack"
        | "Grab"
        | "Taunt"
        // Effects on one random unit near the hit (Workshop examples: ruby
        // Q, kharazim R): nothing to aim.
        | "RandomTarget" => {}
        _ => out.issue(format!("unknown declaration {kind}")),
    }
}
/// World units at or below which a lob's own circle is a flight marker,
/// not an area it hits.
const LOB_MARKER: f32 = 5.;
/// A mover's or lob's `end_effects`, which happen where it lands: an area
/// "around the caster" there is drawn at the aim.
/// A list of effects. When it holds a move to the aimed point or onto the
/// target unit, the other effects go with that move: a circle around the
/// caster is around where the caster lands (the author, 2026-10-21:
/// Invincible R slams where she lands, as League's Fizz E).
fn collect_list(list: &[Value], inherited: Placement, out: &mut Geometry, depth: usize) {
    let is_move = |v: &Value| {
        matches!(
            v.get("type").and_then(Value::as_str),
            Some("MoveTo" | "MoveToTarget" | "Teleport")
        )
    };
    let mover = list.iter().any(is_move);
    for child in list {
        if mover && !is_move(child) {
            let mut beside = Geometry::default();
            collect(child, inherited, &mut beside, depth + 1);
            to_landing(beside, out);
        } else {
            collect(child, inherited, out, depth + 1);
        }
    }
}
fn landed(v: &Value, out: &mut Geometry, depth: usize) {
    let Some(list) = v
        .get("end_effects")
        .and_then(Value::as_array)
        .filter(|s| s.len() <= 32)
    else {
        return;
    };
    let mut at_landing = Geometry::default();
    for child in list {
        collect(child, Placement::Aim, &mut at_landing, depth + 1);
    }
    to_landing(at_landing, out);
}
/// `at_landing`'s footprints where the caster lands, with its issues and
/// families.
fn to_landing(at_landing: Geometry, out: &mut Geometry) {
    for f in at_landing.footprints {
        // Only a circle moves to the landing point; a line or cone keeps its
        // own placement rather than being drawn from a guessed origin.
        let placement = match (f.placement, f.shape) {
            (Placement::Caster, Shape::Circle { .. }) => Placement::Aim,
            (other, _) => other,
        };
        out.push(f.shape, placement, f.allies, f.projectile);
    }
    for reason in at_landing.issues {
        out.issue(reason);
    }
    for name in at_landing.families {
        out.family(name);
    }
}

// Draw order bands (higher on top): fills, dark under-strokes, glow, main
// strokes, white cores, top details.
const Z_FILL: i32 = 986;
const Z_INK: i32 = 987;
const Z_GLOW: i32 = 988;
const Z_MAIN: i32 = 989;
const Z_CORE: i32 = 990;
const Z_TOP: i32 = 991;
const MAX_STROKES: usize = 4096;
#[derive(Clone, Copy)]
struct Stroke {
    a: Point,
    b: Point,
    width: f32,
    color: u32,
    z: i32,
}
#[derive(Clone, Copy)]
struct Disc {
    center: Point,
    radius: f32,
    color: u32,
}
/// Screen-space strokes and filled discs, clipped against the viewport and
/// the UI when rendered.
#[derive(Default)]
pub struct Drawing {
    strokes: Vec<Stroke>,
    discs: Vec<Disc>,
}
/// Horizontal strips (each `step` tall, centred on its scan line) filling a
/// polygon without overlapping: (from, to, height). Even/odd crossings, so
/// concave shapes and wide cones fill correctly.
pub(crate) fn strips(points: &[Point], step: f32) -> Vec<(Point, Point, f32)> {
    let mut out = Vec::new();
    if points.len() < 3 || step <= 0. {
        return out;
    }
    let lo = points.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
    let hi = points.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);
    let mut y = lo;
    while y < hi && out.len() < 1024 {
        let h = step.min(hi - y);
        let scan = y + h / 2.;
        let mut hits = Vec::new();
        for i in 0..points.len() {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            if (a.1 <= scan && b.1 > scan) || (b.1 <= scan && a.1 > scan) {
                hits.push(a.0 + (b.0 - a.0) * (scan - a.1) / (b.1 - a.1));
            }
        }
        hits.sort_by(f32::total_cmp);
        for pair in hits.as_chunks::<2>().0 {
            out.push(((pair[0], scan), (pair[1], scan), h));
        }
        y += step;
    }
    out
}
/// Points along an ellipse arc from angle `a` to `b` (radians).
fn arc_points(center: Point, rx: f32, ry: f32, a: f32, b: f32) -> Vec<Point> {
    let n = (((b - a).abs() * (rx.max(ry) / 0.7).sqrt()).ceil() as usize).clamp(8, 256);
    (0..=n)
        .map(|i| {
            let t = a + (b - a) * i as f32 / n as f32;
            (center.0 + rx * t.cos(), center.1 + ry * t.sin())
        })
        .collect()
}
impl Drawing {
    pub(crate) fn line(&mut self, a: Point, b: Point, width: f32, color: u32, z: i32) {
        if self.strokes.len() < MAX_STROKES
            && [a.0, a.1, b.0, b.1, width].into_iter().all(f32::is_finite)
            && width > 0.
            && color & 0xff > 0
        {
            self.strokes.push(Stroke {
                a,
                b,
                width,
                color,
                z,
            });
        }
    }
    fn poly(&mut self, points: &[Point], width: f32, color: u32, closed: bool, z: i32) {
        for pair in points.windows(2) {
            self.line(pair[0], pair[1], width, color, z);
        }
        if closed && points.len() > 2 {
            self.line(points[points.len() - 1], points[0], width, color, z);
        }
    }
    /// The design's bright edge: dark under-stroke, soft glow, main stroke
    /// and a fine white core.
    fn rim(
        &mut self,
        s: &Style,
        points: &[Point],
        width: f32,
        color: u32,
        white: u32,
        closed: bool,
    ) {
        self.poly(
            points,
            width + s.rim_ink_extra,
            alpha(s.ink, s.rim_ink_opacity),
            closed,
            Z_INK,
        );
        self.poly(
            points,
            width + s.rim_glow_extra,
            alpha(color, s.rim_glow_opacity),
            closed,
            Z_GLOW,
        );
        self.poly(points, width, alpha(color, s.rim_opacity), closed, Z_MAIN);
        self.poly(
            points,
            s.rim_core_width,
            alpha(white, s.rim_core_opacity),
            closed,
            Z_CORE,
        );
    }
    /// A translucent polygon fill from non-overlapping strips.
    fn fill(&mut self, s: &Style, points: &[Point], color: u32, opacity: f32) {
        for (a, b, h) in strips(points, s.strip_step) {
            self.line(a, b, h, alpha(color, opacity), Z_FILL);
        }
    }
    fn disc(&mut self, center: Point, radius: f32, color: u32) {
        if self.discs.len() < 64 && radius > 0. && center.0.is_finite() && center.1.is_finite() {
            self.discs.push(Disc {
                center,
                radius,
                color,
            });
        }
    }
    fn dashed(&mut self, a: Point, b: Point, width: f32, color: u32, dash: f32, gap: f32) {
        let length = (b.0 - a.0).hypot(b.1 - a.1);
        if length <= 0. || dash <= 0. {
            return;
        }
        for i in 0..((length / (dash + gap)).ceil() as usize).min(512) {
            let start = i as f32 * (dash + gap) / length;
            let end = ((i as f32 * (dash + gap) + dash) / length).min(1.);
            self.line(lerp(a, b, start), lerp(a, b, end), width, color, Z_MAIN);
        }
    }
    pub fn render(self, ctx: &mut StableClient<'_>, frame: CameraFrame, blockers: &[Rect]) {
        let mut strokes = self.strokes;
        for d in self.discs {
            // One native disc when it is clear of every edge; otherwise the
            // same fill from strips, which clip like any other stroke.
            let (c, r) = (d.center, d.radius);
            let bounds = Rect {
                x: c.0 - r,
                y: c.1 - r,
                w: 2. * r,
                h: 2. * r,
            };
            let v = frame.viewport;
            let inside = bounds.x >= v.x
                && bounds.y >= v.y
                && bounds.x + bounds.w <= v.x + v.w
                && bounds.y + bounds.h <= v.y + v.h;
            if inside && !blockers.iter().any(|b| b.valid() && overlaps(*b, bounds)) {
                ctx.draw_circle("UI", c.0, c.1, r, Z_FILL, d.color);
            } else {
                let circle = arc_points(c, r, r, 0., std::f32::consts::TAU);
                for (a, b, h) in strips(&circle, style().strip_step) {
                    strokes.push(Stroke {
                        a,
                        b,
                        width: h,
                        color: d.color,
                        z: Z_FILL,
                    });
                }
            }
        }
        for s in strokes {
            for (a, b) in visible(s.a, s.b, frame.viewport, blockers, s.width + 2.) {
                ctx.draw_line("UI", a.0, a.1, b.0, b.1, s.width, s.z, s.color);
            }
        }
    }
}
fn overlaps(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}
fn lerp(a: Point, b: Point, t: f32) -> Point {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}
pub fn endpoint(origin: Point, aim: Point, range: f32, directional: bool) -> Point {
    let dx = aim.0 - origin.0;
    let dy = aim.1 - origin.1;
    let length = dx.hypot(dy);
    if length <= f32::EPSILON {
        return origin;
    }
    let distance = if directional {
        range
    } else {
        length.min(range)
    };
    (
        origin.0 + dx / length * distance,
        origin.1 + dy / length * distance,
    )
}
pub struct Preview {
    pub origin: (u64, u64),
    pub aim: Option<(u64, u64)>,
    pub casting: u32,
    pub self_target: bool,
    /// The skill targets allies (heals, shields): its range is drawn in the
    /// pale ally colour.
    pub allies: bool,
    pub range: u64,
    pub ready: bool,
    pub geometry: Geometry,
    pub target: Option<crate::combat::Unit>,
}
fn placed(origin: Point, aim: Point, p: Placement, range: f32, casting: u32) -> Point {
    match p {
        Placement::Caster => origin,
        Placement::Aim if casting == 3 => origin,
        Placement::Aim if casting == 0 => aim,
        Placement::Aim => endpoint(origin, aim, range, false),
        // RangeEffect's current consumer offsets unit/point inputs only.
        Placement::Forward { offset } if casting <= 1 => endpoint(origin, aim, offset, true),
        Placement::Forward { .. } => origin,
        Placement::End { .. } if casting == 0 => aim,
        Placement::End { length } => endpoint(origin, aim, length, true),
        Placement::Landing { .. } if casting == 0 => aim,
        Placement::Landing { .. } if casting == 3 => origin,
        Placement::Landing { length } => endpoint(origin, aim, length, casting == 2),
    }
}
fn corridor(from: Point, to: Point, radius: f32) -> Option<[Point; 4]> {
    let delta = (to.0 - from.0, to.1 - from.1);
    let len = delta.0.hypot(delta.1);
    if len <= f32::EPSILON {
        return None;
    }
    let n = (-delta.1 / len * radius, delta.0 / len * radius);
    Some([
        (from.0 + n.0, from.1 + n.1),
        (to.0 + n.0, to.1 + n.1),
        (to.0 - n.0, to.1 - n.1),
        (from.0 - n.0, from.1 - n.1),
    ])
}
fn cone(center: Point, aim: Point, radius: f32, cosine: f32) -> Vec<Point> {
    let facing = (aim.1 - center.1).atan2(aim.0 - center.0);
    let half = cosine.clamp(-1., 1.).acos();
    let mut points = vec![center];
    for i in 0..=64 {
        let a = facing - half + 2. * half * i as f32 / 64.;
        points.push((center.0 + radius * a.cos(), center.1 + radius * a.sin()));
    }
    points
}
/// The colours of one preview: the skill colour and its white detail, both
/// gray while the skill is unavailable.
#[derive(Clone, Copy)]
struct Ink {
    col: u32,
    white: u32,
}
impl Ink {
    /// The ally colour for an area that affects allies; a skill that is
    /// not ready stays grey.
    fn side(self, s: &Style, allies: bool, ready: bool) -> Self {
        if allies && ready {
            Self {
                col: s.ally,
                ..self
            }
        } else {
            self
        }
    }
    fn of(s: &Style, ready: bool) -> Self {
        if ready {
            Self {
                col: s.yellow,
                white: s.white,
            }
        } else {
            Self {
                col: s.gray,
                white: s.gray,
            }
        }
    }
}
fn unit_vector(from: Point, to: Point) -> Option<Point> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = dx.hypot(dy);
    (length > f32::EPSILON).then(|| (dx / length, dy / length))
}
/// The range's colour: a sibling of the skill's own colour, so range and
/// shape read as one skill yet stay distinct (the author, 2026-10-20: pale
/// cream beside the yellow shape, a pale blue beside an ally skill's blue;
/// grey while not ready, like the shape).
fn range_color(s: &Style, ready: bool, allies: bool) -> u32 {
    match (ready, allies) {
        (false, _) => s.gray,
        (true, true) => s.range_ally,
        (true, false) => s.range_ready,
    }
}
/// Cast range: a faint fill over the whole range and a bright rim over a
/// dark edge, like League's range indicator (the author, 2026-10-20: the
/// 1 px grey ring was barely visible on grass).
fn range_ring(d: &mut Drawing, s: &Style, center: Point, radius: f32, color: u32) {
    d.disc(center, radius, alpha(color, s.range_fill));
    let ring = arc_points(center, radius, radius, 0., std::f32::consts::TAU);
    d.poly(
        &ring,
        s.range_ink_width,
        alpha(s.ink, s.range_ink_opacity),
        true,
        Z_INK,
    );
    d.poly(
        &ring,
        s.range_width,
        alpha(color, s.range_opacity),
        true,
        Z_MAIN,
    );
}
/// The small diamond grip with a white cross marking a placement point.
fn grip(d: &mut Drawing, s: &Style, p: Point, ink: Ink, size: f32) {
    let diamond = [
        (p.0, p.1 - size),
        (p.0 + size, p.1),
        (p.0, p.1 + size),
        (p.0 - size, p.1),
    ];
    d.rim(s, &diamond, s.grip_rim_width, ink.col, ink.white, true);
    let c = s.grip_cross;
    d.line(
        (p.0 - c, p.1),
        (p.0 + c, p.1),
        s.grip_cross_width,
        alpha(ink.white, 1.),
        Z_TOP,
    );
    d.line(
        (p.0, p.1 - c),
        (p.0, p.1 + c),
        s.grip_cross_width,
        alpha(ink.white, 1.),
        Z_TOP,
    );
}
/// Ground or self area: faint field, bright rim, four clipped accents, and
/// a grip at its centre unless it surrounds the caster.
fn area(d: &mut Drawing, s: &Style, center: Point, radius: f32, ink: Ink, with_grip: bool) {
    d.disc(center, radius, alpha(ink.col, s.area_fill));
    let rim = arc_points(center, radius, radius, 0., std::f32::consts::TAU);
    d.rim(s, &rim, s.rim_width, ink.col, ink.white, true);
    let inner = (radius - s.accent_inset).max(1.);
    for i in 0..4 {
        let a = i as f32 * std::f32::consts::FRAC_PI_2 + s.accent_offset;
        let accent = arc_points(center, inner, inner, a, a + s.accent_span);
        d.poly(
            &accent,
            s.accent_width,
            alpha(ink.col, s.accent_opacity),
            false,
            Z_TOP,
        );
    }
    if with_grip {
        grip(d, s, center, ink, s.grip_size);
    }
}
/// A skillshot's hit corridor: a faint full-width body with thin white
/// edges (the direction arrow is drawn separately).
/// How a strip ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    /// Not a direction of travel (absolute endpoints): closed, no chevrons.
    Still,
    /// Nothing is known about what stops it: a thin closing edge.
    Plain,
    /// It passes through units: both edges trail on past the end.
    Open,
    /// It stops at the first unit it hits: a solid bar.
    Capped,
}
/// A strip at its true width: fill, both edges, chevrons along it for the
/// direction it travels, and its end.
fn corridor_body(
    d: &mut Drawing,
    s: &Style,
    from: Point,
    to: Point,
    half: f32,
    ink: Ink,
    end: End,
) {
    let Some(body) = corridor(from, to, half) else {
        return;
    };
    d.fill(s, &body, ink.col, s.corridor_fill);
    let edge = alpha(ink.col, s.corridor_edge_opacity);
    d.line(body[0], body[1], s.corridor_edge_width, edge, Z_CORE);
    d.line(body[3], body[2], s.corridor_edge_width, edge, Z_CORE);
    let Some(dir) = unit_vector(from, to) else {
        return;
    };
    let length = (to.0 - from.0).hypot(to.1 - from.1);
    let wing = s.corridor_chevron_half.min(half - 2.);
    if end != End::Still && wing >= 2. && s.corridor_chevron_step >= 4. {
        let color = alpha(ink.white, s.corridor_chevron_opacity);
        let mut x = s.corridor_chevron_step;
        while x < length - wing {
            let tip = (from.0 + dir.0 * x, from.1 + dir.1 * x);
            d.poly(
                &chevron(tip, dir, wing),
                s.corridor_chevron_width,
                color,
                false,
                Z_CORE,
            );
            x += s.corridor_chevron_step;
        }
    }
    match end {
        End::Still | End::Plain => d.line(body[1], body[2], s.corridor_edge_width, edge, Z_CORE),
        End::Open => {
            let reach = 2. * (s.corridor_open_dash + s.corridor_open_gap);
            for corner in [body[1], body[2]] {
                let start = (
                    corner.0 + dir.0 * s.corridor_open_gap,
                    corner.1 + dir.1 * s.corridor_open_gap,
                );
                let stop = (corner.0 + dir.0 * reach, corner.1 + dir.1 * reach);
                d.dashed(
                    start,
                    stop,
                    s.corridor_edge_width,
                    edge,
                    s.corridor_open_dash,
                    s.corridor_open_gap,
                );
            }
        }
        End::Capped => {
            let n = (-dir.1, dir.0);
            let over = half + s.corridor_cap_over;
            d.line(
                (to.0 - n.0 * over, to.1 - n.1 * over),
                (to.0 + n.0 * over, to.1 + n.1 * over),
                s.corridor_cap_width,
                alpha(ink.col, 1.),
                Z_MAIN,
            );
        }
    }
}
/// A chevron pointing along `dir` with its point at `tip`.
fn chevron(tip: Point, dir: Point, half: f32) -> [Point; 3] {
    let n = (-dir.1, dir.0);
    let back = (tip.0 - dir.0 * half, tip.1 - dir.1 * half);
    [
        (back.0 - n.0 * half, back.1 - n.1 * half),
        tip,
        (back.0 + n.0 * half, back.1 + n.1 * half),
    ]
}
/// Dash (the author's design K, "Bullet", `design/previews/dash-k.svg`,
/// 1.5.2-dev3): one connected shape from the caster to where the dash ends.
/// In world units, scaled by `scale` (screen px per world unit) like every
/// other preview: a body `dash_body_width` wide in three equal strips laid
/// end to end, brightening toward the front, between two dark edges; then a
/// nose `min(dash_nose_length, dash_nose_share x L)` long in four equal steps
/// narrowing to a white point, each over a dark step a little wider. The
/// same rule for every length: no separate mark for short dashes.
fn dash(d: &mut Drawing, s: &Style, caster: Point, end: Point, ink: Ink, scale: f32) {
    let Some(dir) = unit_vector(caster, end) else {
        return;
    };
    if scale <= f32::EPSILON {
        return;
    }
    let length = (end.0 - caster.0).hypot(end.1 - caster.1) / scale;
    let nose = s.dash_nose_length.min(s.dash_nose_share * length);
    let body = length - nose;
    // A point `t` world units along the dash, `off` to its side.
    let at = |t: f32, off: f32| {
        (
            caster.0 + (dir.0 * t - dir.1 * off) * scale,
            caster.1 + (dir.1 * t + dir.0 * off) * scale,
        )
    };
    for side in [-1., 1.] {
        let off = side * s.dash_edge_offset;
        d.line(
            at(0., off),
            at(body, off),
            s.dash_edge_width * scale,
            alpha(s.ink, s.dash_edge_opacity),
            Z_INK,
        );
    }
    let strips = [
        s.dash_body_opacity_1,
        s.dash_body_opacity_2,
        s.dash_body_opacity_3,
    ];
    for (i, opacity) in strips.into_iter().enumerate() {
        let (from, to) = (body * i as f32 / 3., body * (i + 1) as f32 / 3.);
        d.line(
            at(from, 0.),
            at(to, 0.),
            s.dash_body_width * scale,
            alpha(ink.col, opacity),
            Z_FILL,
        );
    }
    for i in 0..4 {
        let (from, to) = (
            body + nose * i as f32 / 4.,
            body + nose * (i + 1) as f32 / 4.,
        );
        let width = s.dash_body_width * (4 - i) as f32 / 4.;
        d.line(
            at(from, 0.),
            at(to, 0.),
            (width + s.dash_nose_ink_extra) * scale,
            alpha(s.ink, s.dash_nose_ink_opacity),
            Z_INK,
        );
        let (color, z) = if i == 3 {
            (ink.white, Z_CORE)
        } else {
            (ink.col, Z_MAIN)
        };
        d.line(
            at(from, 0.),
            at(to, 0.),
            width * scale,
            alpha(color, s.dash_nose_opacity),
            z,
        );
    }
}
/// A skill whose area is not understood shows only how it is cast (the
/// author's option A, 2026-10-20, replacing the crosshair): a thin arrow
/// from the caster to `at` for a direction cast, a small ring where it lands
/// for a point cast. Neither claims a width or an area.
fn cast_cue(d: &mut Drawing, s: &Style, caster: Point, at: Point, direction: bool, ink: Ink) {
    let color = alpha(ink.col, s.guide_opacity);
    if direction {
        let Some(dir) = unit_vector(caster, at) else {
            return;
        };
        let start = (
            caster.0 + dir.0 * s.dash_start,
            caster.1 + dir.1 * s.dash_start,
        );
        if (at.0 - start.0) * dir.0 + (at.1 - start.1) * dir.1 <= 0. {
            return;
        }
        d.line(start, at, s.guide_width, color, Z_MAIN);
        // Arrowhead: two strokes back from the tip, 30 degrees off the line.
        let head = s.guide_ring + s.guide_tick;
        for side in [-30f32, 30.] {
            let back = turned((-dir.0, -dir.1), side);
            d.line(
                at,
                (at.0 + back.0 * head, at.1 + back.1 * head),
                s.guide_width,
                color,
                Z_MAIN,
            );
        }
    } else {
        let ring = arc_points(at, s.guide_ring, s.guide_ring, 0., std::f32::consts::TAU);
        d.poly(&ring, s.guide_width, color, true, Z_MAIN);
        d.disc(at, s.guide_width, color);
    }
}
/// Blink: a four-part destination aperture, a core dot and a white pin.
fn blink(d: &mut Drawing, s: &Style, p: Point, ink: Ink) {
    let quarter = std::f32::consts::FRAC_PI_2;
    for i in 0..4 {
        let a = i as f32 * quarter + s.blink_gap;
        let part = arc_points(p, s.blink_rx, s.blink_ry, a, a + quarter - 2. * s.blink_gap);
        d.rim(s, &part, s.blink_width, ink.col, ink.white, false);
    }
    d.disc(p, s.blink_core_radius, alpha(ink.col, s.blink_core_fill));
    let pin = alpha(ink.white, 1.);
    d.line(
        (p.0, p.1 - s.blink_pin_height),
        (p.0, p.1 + s.blink_pin_height),
        s.blink_pin_stroke,
        pin,
        Z_TOP,
    );
    d.line(
        (p.0 - s.blink_pin_width, p.1),
        (p.0 + s.blink_pin_width, p.1),
        s.blink_pin_stroke,
        pin,
        Z_TOP,
    );
    let y = p.1 - s.blink_tick_offset;
    d.line(
        (p.0 - s.blink_tick_half, y),
        (p.0 + s.blink_tick_half, y),
        s.blink_tick_width,
        alpha(ink.col, 1.),
        Z_TOP,
    );
}
/// Cone: filled sector, rim, centre spine and a faint inner arc.
fn cone_area(d: &mut Drawing, s: &Style, points: &[Point], ink: Ink) {
    d.fill(s, points, ink.col, s.area_fill);
    d.rim(s, points, s.rim_width, ink.col, ink.white, true);
    let (Some(&c), Some(&mid)) = (points.first(), points.get(1 + (points.len() - 1) / 2)) else {
        return;
    };
    if let Some(dir) = unit_vector(c, mid) {
        let reach = (mid.0 - c.0).hypot(mid.1 - c.1);
        let i = s.cone_spine_inset.min(reach / 3.);
        d.line(
            (c.0 + dir.0 * i, c.1 + dir.1 * i),
            (c.0 + dir.0 * (reach - i), c.1 + dir.1 * (reach - i)),
            s.cone_spine_width,
            alpha(ink.col, s.cone_spine_opacity),
            Z_TOP,
        );
    }
    let inner: Vec<Point> = points[1..]
        .iter()
        .map(|p| {
            (
                c.0 + (p.0 - c.0) * s.cone_inner_arc,
                c.1 + (p.1 - c.1) * s.cone_inner_arc,
            )
        })
        .collect();
    d.poly(
        &inner,
        s.cone_inner_width,
        alpha(ink.white, s.cone_inner_opacity),
        false,
        Z_CORE,
    );
}
/// Wall (an upright rectangle in the game's data): one wide strip as fill,
/// rim, bright short ends and a centre grip.
fn wall(d: &mut Drawing, s: &Style, center: Point, half: (f32, f32), ink: Ink) {
    let (hx, hy) = half;
    let (x0, y0, x1, y1) = (center.0 - hx, center.1 - hy, center.0 + hx, center.1 + hy);
    let fill = alpha(ink.col, s.area_fill);
    let ends = if hx >= hy {
        d.line((x0, center.1), (x1, center.1), 2. * hy, fill, Z_FILL);
        [((x0, y0), (x0, y1)), ((x1, y0), (x1, y1))]
    } else {
        d.line((center.0, y0), (center.0, y1), 2. * hx, fill, Z_FILL);
        [((x0, y0), (x1, y0)), ((x0, y1), (x1, y1))]
    };
    d.rim(
        s,
        &[(x0, y0), (x1, y0), (x1, y1), (x0, y1)],
        s.rim_width,
        ink.col,
        ink.white,
        true,
    );
    for (a, b) in ends {
        d.line(
            a,
            b,
            s.wall_end_width,
            alpha(ink.col, s.wall_end_opacity),
            Z_TOP,
        );
    }
    grip(d, s, center, ink, s.wall_grip_size);
}
/// Unit target: a team-coloured ground arc under the unit (its outline is
/// the hover outline) and the ability marker above it: yellow, orange
/// beyond range, gray while unavailable.
fn target_marker(d: &mut Drawing, s: &Style, feet: Point, top: Point, team: u32, marker: u32) {
    let g = s.target_arc_gap;
    let pi = std::f32::consts::PI;
    let front = arc_points(feet, s.target_rx, s.target_ry, g, pi - g);
    d.poly(
        &front,
        s.target_front_width,
        alpha(team, s.target_front_opacity),
        false,
        Z_MAIN,
    );
    let back = arc_points(feet, s.target_rx, s.target_ry, pi + g, 2. * pi - g);
    d.poly(
        &back,
        s.target_back_width,
        alpha(team, s.target_back_opacity),
        false,
        Z_MAIN,
    );
    let tip = (top.0, top.1 - s.target_marker_lift);
    let triangle = [
        (tip.0 - s.target_marker_half, tip.1 - s.target_marker_height),
        (tip.0 + s.target_marker_half, tip.1 - s.target_marker_height),
        tip,
    ];
    d.fill(s, &triangle, marker, s.target_marker_fill);
    d.rim(s, &triangle, s.target_marker_rim, marker, s.white, true);
}
/// Aim beyond cast range: a dashed line from the clamped point to the
/// cursor and a small ring there.
fn beyond(d: &mut Drawing, s: &Style, end: Point, aim: Point, ready: bool) {
    let color = alpha(if ready { s.orange } else { s.gray }, s.beyond_opacity);
    d.dashed(end, aim, s.beyond_width, color, s.beyond_dash, s.beyond_gap);
    let ring = arc_points(aim, s.beyond_ring, s.beyond_ring, 0., std::f32::consts::TAU);
    d.poly(&ring, s.beyond_width, color, true, Z_MAIN);
}
pub fn drawing(frame: CameraFrame, p: Preview) -> Drawing {
    let mut out = Drawing::default();
    if !frame.valid() || p.geometry.hidden {
        return out;
    }
    let s = style();
    let origin = (p.origin.0 as f32 / 1000., p.origin.1 as f32 / 1000.);
    let project = |p: Point| frame.project_unclipped(p.0, p.1);
    let center = project(origin);
    let range = p.range as f32 / 1000.;
    let ink = Ink::of(s, p.ready);
    let (sx, sy) = (2048. / frame.extent.0, 2048. / frame.extent.1);
    let self_area = p.self_target || p.casting == 3 || p.geometry.self_area();
    if range > 0. && !self_area {
        let color = range_color(s, p.ready, p.allies);
        range_ring(&mut out, s, center, range * sx, color);
    }
    let world_target = p
        .target
        .map(|t| (t.position.0 as f32 / 1000., t.position.1 as f32 / 1000.));
    let aim = if p.self_target {
        Some(origin)
    } else {
        world_target.or(p.aim.map(|a| (a.0 as f32 / 1000., a.1 as f32 / 1000.)))
    };
    // Unit input marks the unit; it never erases a corridor or an area
    // declared by the effect.
    if p.casting == 0 && !p.self_target {
        if let (Some(target), Some(world)) = (p.target, world_target) {
            let in_range = (world.0 - origin.0).hypot(world.1 - origin.1) <= range;
            let team = if !p.ready {
                s.gray
            } else if target.friendly {
                s.ally
            } else {
                s.enemy
            };
            let marker = if !p.ready {
                s.gray
            } else if in_range {
                s.yellow
            } else {
                s.orange
            };
            let (x0, y0, x1, _, _) = crate::combat::body_screen_area(frame, &target);
            target_marker(
                &mut out,
                s,
                project(world),
                ((x0 + x1) / 2., y0),
                team,
                marker,
            );
        }
    }
    for f in &p.geometry.footprints {
        // A unit-targeted footprint needs a selected unit unless it is self-centered.
        if p.casting == 0 && !self_area && world_target.is_none() {
            continue;
        }
        let ink = ink.side(s, f.allies, p.ready);
        let aiming = aim.unwrap_or(origin);
        let base = placed(origin, aiming, f.placement, range, p.casting);
        let at = project(base);
        // A projectile registers a hit at its radius plus the stat rules'
        // fixed margin on each side; the preview shows that width.
        let margin = if f.projectile {
            crate::athlete_rules::preview_margin()
        } else {
            0.
        };
        match f.shape {
            Shape::Circle { radius } => {
                let around_caster = (base.0 - origin.0).hypot(base.1 - origin.1) < 0.5;
                area(&mut out, s, at, radius * sx, ink, !around_caster);
            }
            Shape::Rectangle { width, height } => {
                wall(&mut out, s, at, (width / 2. * sx, height / 2. * sy), ink);
            }
            Shape::Cone { radius, cosine } => {
                // Away from the caster, a cone faces caster -> area centre
                // (not centre -> cursor, which can reverse on short clicks);
                // at the caster it faces the aim (Ice Mage R's sweep).
                let away = (base.0 - origin.0).hypot(base.1 - origin.1) > f32::EPSILON;
                let facing = if away {
                    Some((2. * base.0 - origin.0, 2. * base.1 - origin.1))
                } else {
                    Some(aiming).filter(|a| (a.0 - origin.0).hypot(a.1 - origin.1) > f32::EPSILON)
                };
                if let Some(facing) = facing {
                    let points = cone(base, facing, radius, cosine)
                        .into_iter()
                        .map(project)
                        .collect::<Vec<_>>();
                    cone_area(&mut out, s, &points, ink);
                }
            }
            Shape::Corridor { radius, length } => {
                // Length 0: as far as the cast range (dashes). A zero-width
                // projectile is still drawn as a thin line.
                let length = if length > 0. { length } else { range };
                let end = project(endpoint(base, aiming, length, true));
                let half = (radius.max(1.5) + margin) * sx;
                corridor_body(&mut out, s, at, end, half, ink, End::Plain);
            }
            Shape::Shot {
                radius,
                length,
                pierce,
            } => {
                let end = project(endpoint(base, aiming, length, true));
                let cap = if pierce { End::Open } else { End::Capped };
                corridor_body(
                    &mut out,
                    s,
                    at,
                    end,
                    (radius.max(1.5) + margin) * sx,
                    ink,
                    cap,
                );
            }
            Shape::Segment { radius, from, to } => {
                // Absolute endpoints: no direction of travel is implied.
                let (a, b) = (project(from), project(to));
                corridor_body(&mut out, s, a, b, radius * sx, ink, End::Still);
            }
            Shape::Movement { blink: true } => blink(&mut out, s, at, ink),
            Shape::Reach { extra } => {
                if let Some(range) = attack_range() {
                    let active = REACH_ACTIVE.load(std::sync::atomic::Ordering::Relaxed);
                    area(
                        &mut out,
                        s,
                        project(origin),
                        reach_radius(range, extra, active) * sx,
                        ink,
                        false,
                    );
                }
            }
            Shape::Fan {
                count,
                radius,
                length,
            } => {
                if let Some(dir) = unit_vector(base, aiming) {
                    for offset in fan_offsets(count) {
                        let d = turned(dir, offset);
                        let end = project((base.0 + d.0 * length, base.1 + d.1 * length));
                        corridor_body(
                            &mut out,
                            s,
                            at,
                            end,
                            (radius + margin) * sx,
                            ink,
                            End::Plain,
                        );
                    }
                }
            }
            Shape::Spread {
                count,
                radius,
                length,
                degrees,
            } => {
                let length = if length > 0. { length } else { range };
                if let Some(dir) = unit_vector(base, aiming) {
                    for offset in spread_offsets(count, degrees) {
                        let d = turned(dir, offset);
                        let end = project((base.0 + d.0 * length, base.1 + d.1 * length));
                        let half = (radius.max(1.5) + margin) * sx;
                        corridor_body(&mut out, s, at, end, half, ink, End::Plain);
                    }
                }
            }
            Shape::Backstep { length } => {
                // Straight away from the aim (the target of a unit cast).
                if let Some(dir) = unit_vector(aiming, origin) {
                    let end = (origin.0 + dir.0 * length, origin.1 + dir.1 * length);
                    dash(&mut out, s, center, project(end), ink, sx);
                }
            }
            Shape::Movement { blink: false } => dash(&mut out, s, center, at, ink, sx),
        }
    }
    if let Some(aim) = aim.filter(|_| !self_area && p.casting != 0) {
        let end = endpoint(origin, aim, range, p.casting == 2);
        let at = project(end);
        if p.geometry.footprints.is_empty() && !p.geometry.known_without_area() {
            // A skill not understood: only how it is cast, no shape.
            cast_cue(&mut out, s, center, at, p.casting == 2, ink);
        }
        if p.casting == 1 && (end.0 - aim.0).hypot(end.1 - aim.1) > 0.1 {
            beyond(&mut out, s, at, project(aim), p.ready);
        }
    }
    out
}
/// The game's Dancer R spread, in degrees, for `count` blades: blade 0 on
/// the aim, then +60 x i / half for i up to half, -60 x (i - half) / half
/// after it (half = count / 2), in the game's integer millidegrees.
/// Positive turns clockwise on screen.
pub(crate) fn fan_offsets(count: usize) -> Vec<f32> {
    let half = (count / 2) as i64;
    (0..count.min(16) as i64)
        .map(|i| match i {
            0 => 0,
            _ if half == 0 => 0,
            i if i <= half => i * 60_000 / half,
            i => -(i - half) * 60_000 / half,
        } as f32
            / 1000.)
        .collect()
}
/// `count` lines spread evenly over `degrees` in total, centred on the aim.
pub(crate) fn spread_offsets(count: usize, degrees: f32) -> Vec<f32> {
    let count = count.clamp(1, 16);
    if count == 1 {
        return vec![0.];
    }
    let step = degrees / (count - 1) as f32;
    (0..count)
        .map(|i| -degrees / 2. + step * i as f32)
        .collect()
}
/// `v` turned by `degrees`, clockwise on screen (y grows downward).
fn turned(v: Point, degrees: f32) -> Point {
    let (s, c) = degrees.to_radians().sin_cos();
    (c * v.0 - s * v.1, s * v.0 + c * v.1)
}
/// HUD hover has no battlefield aim. Show known reach and caster-centered
/// areas only; never orient a shot toward the icon or reuse a previous aim.
pub fn hover_drawing(frame: CameraFrame, p: Preview) -> Drawing {
    let mut out = Drawing::default();
    if !frame.valid() || p.geometry.hidden {
        return out;
    }
    let s = style();
    let center = frame.project_unclipped(p.origin.0 as f32 / 1000., p.origin.1 as f32 / 1000.);
    let (sx, sy) = (2048. / frame.extent.0, 2048. / frame.extent.1);
    let self_area = p.self_target || p.casting == 3 || p.geometry.self_area();
    let ink = Ink::of(s, p.ready);
    if p.range > 0 && !self_area {
        // No aim while the pointer is on the HUD: the range is the whole
        // preview, so its rim is also drawn in the skill's own colour.
        let radius = p.range as f32 / 1000. * sx;
        range_ring(
            &mut out,
            s,
            center,
            radius,
            range_color(s, p.ready, p.allies),
        );
        let ring = arc_points(center, radius, radius, 0., std::f32::consts::TAU);
        out.poly(
            &ring,
            s.hover_range_width,
            alpha(ink.col, s.hover_range_opacity),
            true,
            Z_MAIN,
        );
    }
    for footprint in &p.geometry.footprints {
        let centered = footprint.placement == Placement::Caster
            || footprint.placement == Placement::Aim && (p.self_target || p.casting == 3);
        if !centered {
            continue;
        }
        let ink = ink.side(s, footprint.allies, p.ready);
        match footprint.shape {
            Shape::Circle { radius } => area(&mut out, s, center, radius * sx, ink, false),
            Shape::Rectangle { width, height } => wall(
                &mut out,
                s,
                center,
                (width / 2. * sx, height / 2. * sy),
                ink,
            ),
            _ => {} // Directional footprints need an actual battlefield aim.
        }
    }
    out
}
fn interval(a: Point, b: Point, r: Rect) -> Option<(f32, f32)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut low, mut high) = (0f32, 1f32);
    for (p, q) in [
        (-dx, a.0 - r.x),
        (dx, r.x + r.w - a.0),
        (-dy, a.1 - r.y),
        (dy, r.y + r.h - a.1),
    ] {
        if p == 0. {
            if q < 0. {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0. {
                low = low.max(t);
            } else {
                high = high.min(t);
            }
            if low > high {
                return None;
            }
        }
    }
    Some((low, high))
}
fn visible(
    a: Point,
    b: Point,
    viewport: Rect,
    blockers: &[Rect],
    width: f32,
) -> Vec<(Point, Point)> {
    let inset = width / 2.;
    let safe = Rect {
        x: viewport.x + inset,
        y: viewport.y + inset,
        w: viewport.w - width,
        h: viewport.h - width,
    };
    if safe.w <= 0. || safe.h <= 0. {
        return Vec::new();
    }
    let Some((lo, hi)) = interval(a, b, safe) else {
        return Vec::new();
    };
    let mut spans = vec![(lo, hi)];
    for r in blockers.iter().filter(|r| r.valid()).take(64) {
        let r = Rect {
            x: r.x - inset,
            y: r.y - inset,
            w: r.w + width,
            h: r.h + width,
        };
        if let Some((start, end)) = interval(a, b, r) {
            spans = spans
                .into_iter()
                .flat_map(|(s, e)| {
                    if end <= s || start >= e {
                        return vec![(s, e)];
                    }
                    let mut result = Vec::new();
                    if s < start {
                        result.push((s, start));
                    }
                    if e > end {
                        result.push((end, e));
                    }
                    result
                })
                .collect();
        }
    }
    spans
        .into_iter()
        .filter(|(s, e)| e - s > 0.00001)
        .map(|(s, e)| (lerp(a, b, s), lerp(a, b, e)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rgb(color: u32) -> u32 {
        color & 0xffff_ff00
    }
    fn near(p: Point, q: Point) -> bool {
        (p.0 - q.0).abs() < 0.01 && (p.1 - q.1).abs() < 0.01
    }
    fn frame() -> CameraFrame {
        CameraFrame {
            viewport: Rect {
                x: 0.,
                y: 0.,
                w: 1920.,
                h: 1080.,
            },
            minimap: Rect {
                x: 1600.,
                y: 750.,
                w: 320.,
                h: 330.,
            },
            center: (480., 480.),
            extent: (1024., 1024.),
            zoom: 1.,
        }
    }
    #[test]
    fn hud_hover_shows_reach_without_reusing_direction_or_target() {
        let mut geometry = Geometry::default();
        geometry.add(
            Shape::Corridor {
                radius: 5.,
                length: 80.,
            },
            Placement::Caster,
        );
        let preview = Preview {
            origin: (480000, 480000),
            aim: Some((600000, 600000)),
            casting: 2,
            self_target: false,
            allies: false,
            range: 45000,
            ready: true,
            geometry,
            target: None,
        };
        let d = hover_drawing(frame(), preview);
        let st = style();
        assert!(!d.strokes.is_empty());
        assert!(d
            .strokes
            .iter()
            .all(|s| [st.gray, st.ink, st.yellow, st.range_ready].contains(&rgb(s.color))));
        // The range's own rim is the cream sibling of the skill's yellow.
        assert!(d
            .strokes
            .iter()
            .any(|s| rgb(s.color) == st.range_ready && s.width == st.range_width));
        // The range itself is drawn in the skill's colour.
        assert!(d
            .strokes
            .iter()
            .any(|s| rgb(s.color) == st.yellow && s.width == st.hover_range_width));
        assert!(d
            .strokes
            .iter()
            .all(|s| (s.a.0 - 960.).hypot(s.a.1 - 540.) <= 90.01));
    }
    #[test]
    fn hud_hover_self_area_is_centered_and_unknown_shape_does_not_invent_area() {
        let mut geometry = Geometry::default();
        geometry.add(Shape::Circle { radius: 10. }, Placement::Aim);
        let preview = Preview {
            origin: (480000, 480000),
            aim: Some((600000, 600000)),
            casting: 3,
            self_target: true,
            allies: false,
            range: 45000,
            ready: false,
            geometry,
            target: None,
        };
        let d = hover_drawing(frame(), preview);
        let st = style();
        assert!(!d.strokes.is_empty());
        assert!(d
            .strokes
            .iter()
            .all(|s| rgb(s.color) == st.gray || rgb(s.color) == st.ink));
        assert_eq!(d.discs.len(), 1);
        assert_eq!(rgb(d.discs[0].color), st.gray);
        assert!(d
            .strokes
            .iter()
            .all(|s| (s.a.0 - 960.).hypot(s.a.1 - 540.) <= 20.01));
        let unknown = Preview {
            origin: (480000, 480000),
            aim: None,
            casting: 3,
            self_target: true,
            allies: false,
            range: 0,
            ready: true,
            geometry: Geometry::default(),
            target: None,
        };
        let d = hover_drawing(frame(), unknown);
        assert!(d.strokes.is_empty() && d.discs.is_empty());
    }
    #[test]
    fn self_burst_uses_area_radius_without_reach_ring_or_self_brackets() {
        let mut geometry = Geometry::default();
        geometry.add(Shape::Circle { radius: 10. }, Placement::Caster);
        let d = drawing(
            frame(),
            Preview {
                origin: (480000, 480000),
                aim: Some((600000, 600000)),
                casting: 0,
                self_target: true,
                allies: false,
                range: 45000,
                ready: true,
                geometry,
                target: None,
            },
        );
        assert!(!d.strokes.is_empty());
        // Everything lies on or inside the 20 px area: no reach ring.
        assert!(d
            .strokes
            .iter()
            .all(|s| (s.a.0 - 960.).hypot(s.a.1 - 540.) <= 20.01));
        // No grip diamond at the caster.
        let top = (960., 540. - style().grip_size);
        assert!(!d.strokes.iter().any(|s| near(s.a, top) || near(s.b, top)));
        assert_eq!(d.discs.len(), 1);
    }
    #[test]
    fn unit_input_can_draw_corridor_beyond_target_and_forward_circle() {
        let target = crate::combat::Unit {
            id: 29,
            position: (510000, 480000),
            radius: 1000,
            is_champion: true,
            is_minion: false,
            friendly: false,
            in_cc: false,
            is_tower: false,
            body: None,
        };
        let mut geometry = Geometry::default();
        geometry.add(
            Shape::Corridor {
                radius: 3.,
                length: 130.,
            },
            Placement::Caster,
        );
        let d = drawing(
            frame(),
            Preview {
                origin: (480000, 480000),
                aim: Some((480000, 600000)),
                casting: 0,
                self_target: false,
                allies: false,
                range: 45000,
                ready: true,
                geometry,
                target: Some(target),
            },
        );
        // The corridor edge runs the full 130 units past the target.
        assert!(d.strokes.iter().any(|s| near(s.b, (1220., 546.))));
        // The target gets the enemy ground arc.
        assert!(d.strokes.iter().any(|s| rgb(s.color) == style().enemy));
        let mut geometry = Geometry::default();
        geometry.add(
            Shape::Circle { radius: 40. },
            Placement::Forward { offset: 20. },
        );
        let d = drawing(
            frame(),
            Preview {
                origin: (480000, 480000),
                aim: Some((500000, 480000)),
                casting: 1,
                self_target: false,
                allies: false,
                range: 40000,
                ready: true,
                geometry,
                target: None,
            },
        );
        // The 40-unit circle 20 units ahead: its rim passes (1080, 540).
        assert!(d
            .strokes
            .iter()
            .any(|s| rgb(s.color) == style().yellow && near(s.a, (1080., 540.))));
    }
    #[test]
    fn strips_cover_a_shape_once_without_overlap() {
        let square = [(0., 0.), (10., 0.), (10., 9.), (0., 9.)];
        let s = strips(&square, 3.);
        assert_eq!(s.len(), 3);
        assert!(s
            .iter()
            .all(|(a, b, h)| a.0 == 0. && b.0 == 10. && *h == 3.));
        assert_eq!(
            s.iter().map(|(a, _, _)| a.1).collect::<Vec<_>>(),
            [1.5, 4.5, 7.5]
        );
        let s = strips(&[(0., 0.), (10., 0.), (10., 7.), (0., 7.)], 3.);
        assert_eq!(s.last().unwrap().2, 1.);
        assert!(strips(&[(0., 0.), (1., 1.)], 3.).is_empty());
    }
    #[test]
    fn a_dash_is_the_bullet_of_design_k() {
        let st = style();
        let ink = Ink::of(st, true);
        // The design's sample: 150 world units along +x at 1 px per unit.
        let mut d = Drawing::default();
        dash(&mut d, st, (0., 0.), (150., 0.), ink, 1.);
        let has = |d: &Drawing, a: Point, b: Point, width: f32, color: u32, z: i32| {
            d.strokes.iter().any(|s| {
                near(s.a, a)
                    && near(s.b, b)
                    && (s.width - width).abs() < 0.01
                    && s.color == color
                    && s.z == z
            })
        };
        let dark = alpha(st.ink, 0.8);
        // Edges, body strips (nose 14, body 136), nose steps and their ink.
        assert!(has(&d, (0., -5.75), (136., -5.75), 1.5, dark, Z_INK));
        assert!(has(&d, (0., 5.75), (136., 5.75), 1.5, dark, Z_INK));
        let third = 136. / 3.;
        assert!(has(
            &d,
            (0., 0.),
            (third, 0.),
            10.,
            alpha(st.yellow, 0.14),
            Z_FILL
        ));
        assert!(has(
            &d,
            (third, 0.),
            (2. * third, 0.),
            10.,
            alpha(st.yellow, 0.3),
            Z_FILL
        ));
        assert!(has(
            &d,
            (2. * third, 0.),
            (136., 0.),
            10.,
            alpha(st.yellow, 0.5),
            Z_FILL
        ));
        assert!(has(&d, (136., 0.), (139.5, 0.), 13., dark, Z_INK));
        assert!(has(
            &d,
            (136., 0.),
            (139.5, 0.),
            10.,
            alpha(st.yellow, 0.95),
            Z_MAIN
        ));
        assert!(has(
            &d,
            (143., 0.),
            (146.5, 0.),
            5.,
            alpha(st.yellow, 0.95),
            Z_MAIN
        ));
        assert!(has(&d, (146.5, 0.), (150., 0.), 5.5, dark, Z_INK));
        assert!(has(
            &d,
            (146.5, 0.),
            (150., 0.),
            2.5,
            alpha(st.white, 0.95),
            Z_CORE
        ));
        assert_eq!(d.strokes.len(), 2 + 3 + 8);
        // Nothing behind the caster or past the end.
        assert!(d
            .strokes
            .iter()
            .all(|s| s.a.0 >= -0.01 && s.b.0 <= 150.01 && s.a.0 <= s.b.0));
        // A short dash: the nose is 30 % of it (20 units: nose 6, body 14),
        // and widths and lengths scale with the camera.
        let mut d = Drawing::default();
        dash(&mut d, st, (0., 0.), (40., 0.), ink, 2.);
        assert!(has(
            &d,
            (28., 0.),
            (31., 0.),
            20.,
            alpha(st.yellow, 0.95),
            Z_MAIN
        ));
        assert!(has(
            &d,
            (37., 0.),
            (40., 0.),
            5.,
            alpha(st.white, 0.95),
            Z_CORE
        ));
        // Not ready: grey, with a grey point.
        let mut d = Drawing::default();
        dash(&mut d, st, (0., 0.), (0., 150.), Ink::of(st, false), 1.);
        assert!(has(
            &d,
            (0., 146.5),
            (0., 150.),
            2.5,
            alpha(st.gray, 0.95),
            Z_CORE
        ));
        // No length, nothing drawn.
        let mut d = Drawing::default();
        dash(&mut d, st, (5., 5.), (5., 5.), ink, 1.);
        assert!(d.strokes.is_empty());
    }
    #[test]
    fn a_strip_ends_open_when_it_passes_through_and_capped_when_it_stops() {
        let st = style();
        let ink = Ink::of(st, true);
        let bar = |d: &Drawing| {
            d.strokes.iter().any(|s| {
                s.width == st.corridor_cap_width && near(s.a, (100., -8. - st.corridor_cap_over))
            })
        };
        let beyond = |d: &Drawing| d.strokes.iter().any(|s| s.a.0 > 100.5 || s.b.0 > 100.5);
        let mut d = Drawing::default();
        corridor_body(&mut d, st, (0., 0.), (100., 0.), 8., ink, End::Capped);
        assert!(bar(&d) && !beyond(&d));
        let mut d = Drawing::default();
        corridor_body(&mut d, st, (0., 0.), (100., 0.), 8., ink, End::Open);
        assert!(!bar(&d) && beyond(&d));
        // Chevrons along a travelling strip, none on a still one.
        let inner = |d: &Drawing| {
            d.strokes
                .iter()
                .filter(|s| rgb(s.color) == st.white && s.width == st.corridor_chevron_width)
                .count()
        };
        assert!(inner(&d) >= 4);
        let mut d = Drawing::default();
        corridor_body(&mut d, st, (0., 0.), (100., 0.), 8., ink, End::Still);
        assert_eq!(inner(&d), 0);
        assert!(!bar(&d) && !beyond(&d));
    }
    #[test]
    fn a_cone_at_the_caster_sweeps_toward_the_aim() {
        // Ice Mage R: 70-unit, 90° cone from the caster, aimed straight down.
        let mut geometry = Geometry::default();
        geometry.add(
            Shape::Cone {
                radius: 70.,
                cosine: 0.707,
            },
            Placement::Caster,
        );
        let d = drawing(
            frame(),
            Preview {
                origin: (480000, 480000),
                aim: Some((480000, 600000)),
                casting: 2,
                self_target: false,
                allies: false,
                range: 70000,
                ready: true,
                geometry,
                target: None,
            },
        );
        // Its spine runs from the caster (960, 540) toward +y.
        let st = style();
        assert!(d.strokes.iter().any(|s| s.z == Z_TOP
            && rgb(s.color) == st.yellow
            && (s.a.0 - 960.).abs() < 0.01
            && s.b.1 > s.a.1 + 50.));
    }
    #[test]
    fn dancer_fan_follows_the_game_spread() {
        // 4 blades: 0, +30, +60, -30; 5: symmetric; 10: 12-degree steps.
        assert_eq!(fan_offsets(4), [0., 30., 60., -30.]);
        assert_eq!(fan_offsets(5), [0., 30., 60., -30., -60.]);
        assert_eq!(fan_offsets(10)[1], 12.);
        assert_eq!(fan_offsets(10)[9], -48.);
        assert_eq!(fan_offsets(1), [0.]);
        // +90 turns +x (screen right) to +y (screen down): clockwise.
        let v = turned((1., 0.), 90.);
        assert!(v.0.abs() < 1e-6 && (v.1 - 1.).abs() < 1e-6);
        // Drawn: two edges and a closing end per blade.
        let mut geometry = Geometry::default();
        geometry.add(
            Shape::Fan {
                count: 4,
                radius: 10.,
                length: 134.,
            },
            Placement::Caster,
        );
        let d = drawing(
            frame(),
            Preview {
                origin: (480000, 480000),
                aim: Some((600000, 480000)),
                casting: 2,
                self_target: false,
                allies: false,
                range: 130000,
                ready: true,
                geometry,
                target: None,
            },
        );
        let st = style();
        let edges = d
            .strokes
            .iter()
            .filter(|s| {
                s.z == Z_CORE && s.width == st.corridor_edge_width && rgb(s.color) == st.yellow
            })
            .count();
        assert_eq!(edges, 12);
    }
    #[test]
    fn understood_skills_without_an_area_get_no_direction_guide() {
        let mut buff = Geometry::default();
        buff.family("native AddCasterBuff");
        assert!(buff.known_without_area());
        let preview = |geometry, range| Preview {
            origin: (480000, 480000),
            aim: Some((600000, 480000)),
            casting: 2,
            self_target: false,
            allies: false,
            range,
            ready: true,
            geometry,
            target: None,
        };
        assert!(drawing(frame(), preview(buff, 0)).strokes.is_empty());
        let mut unknown = Geometry::default();
        unknown.issue("unsupported native apply=1 size=8");
        assert!(!drawing(frame(), preview(unknown, 50000)).strokes.is_empty());
    }
    #[test]
    fn wall_fills_with_one_strip_and_brightens_its_short_ends() {
        let st = style();
        let mut d = Drawing::default();
        wall(&mut d, st, (100., 100.), (50., 10.), Ink::of(st, true));
        // One 20 px strip along the long (horizontal) axis.
        let fills: Vec<_> = d.strokes.iter().filter(|s| s.z == Z_FILL).collect();
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].width, 20.);
        assert!(near(fills[0].a, (50., 100.)) && near(fills[0].b, (150., 100.)));
        // The short ends are the vertical edges.
        let ends: Vec<_> = d
            .strokes
            .iter()
            .filter(|s| s.z == Z_TOP && s.width == st.wall_end_width)
            .collect();
        assert_eq!(ends.len(), 2);
        assert!(ends.iter().all(|s| s.a.0 == s.b.0));
    }
    #[test]
    fn unavailable_skills_are_gray_and_beyond_range_is_orange() {
        let st = style();
        let mut geometry = Geometry::default();
        geometry.add(Shape::Circle { radius: 10. }, Placement::Aim);
        let preview = |ready| Preview {
            origin: (480000, 480000),
            aim: Some((600000, 480000)),
            casting: 1,
            self_target: false,
            allies: false,
            range: 40000,
            ready,
            geometry: geometry.clone(),
            target: None,
        };
        let d = drawing(frame(), preview(true));
        assert!(d.strokes.iter().any(|s| rgb(s.color) == st.orange));
        assert!(d.strokes.iter().any(|s| rgb(s.color) == st.yellow));
        let d = drawing(frame(), preview(false));
        assert!(d
            .strokes
            .iter()
            .all(|s| [st.gray, st.ink].contains(&rgb(s.color))));
    }
    #[test]
    fn delayed_and_self_wrappers_preserve_multiple_primary_areas() {
        let v = serde_json::json!({"skill":{"effect":{"type":"Delayed","effects":[
            {"type":"RangeProjectile","shape":{"Circle":{"radius":20000}}},
            {"type":"WithSelf","effects":[{"type":"RangeProjectile","shape":{"Circle":{"radius":10000}}}]}]}}});
        let g = skills(&v)[0].clone();
        assert_eq!(g.footprints.len(), 2);
        assert_eq!(g.footprints[0].placement, Placement::Aim);
        assert_eq!(g.footprints[1].placement, Placement::Caster);
        assert!(g.issues.is_empty());
    }
    #[test]
    fn areas_beside_a_move_happen_where_the_caster_lands() {
        // invincible R, trimmed: after a move onto the target, a delayed slam
        // around the caster, so around where she lands.
        let r = serde_json::json!({"ult": {"casting_type": "Targeting", "effect": {"type": "Combine", "effects": [
            {"type": "Delayed", "tick": 33, "effects": [
                {"type": "MoveToTarget", "speed": 2800, "range": 32000, "end_effects": []},
                {"type": "Delayed", "tick": 12, "effects": [
                    {"type": "RangeEffect", "shape": {"Circle": {"radius": 46000}},
                     "target": "EnemyWithoutTower", "apply_type": "AroundCaster", "effects": []}]}]}]}}});
        let g = skills(&r)[2].clone();
        let found: Vec<_> = g
            .footprints
            .iter()
            .map(|f| (f.shape, f.placement))
            .collect();
        assert_eq!(
            found,
            [
                (Shape::Movement { blink: false }, Placement::Aim),
                (Shape::Circle { radius: 46. }, Placement::Aim)
            ]
        );
        // Without a move the slam stays around the caster.
        let alone = serde_json::json!({"skill": {"effect": {"type": "Combine", "effects": [
            {"type": "RangeEffect", "shape": {"Circle": {"radius": 46000}}, "apply_type": "AroundCaster"}]}}});
        assert_eq!(skills(&alone)[0].footprints[0].placement, Placement::Caster);
    }
    #[test]
    fn workshop_effect_types_from_installed_champion_files_are_read() {
        // Trimmed from Workshop champion files on the author's PC, 2026-10-20.
        let one = |effect: serde_json::Value| {
            skills(&serde_json::json!({"skill": {"effect": effect}}))[0].clone()
        };
        // abbie W: a line hit from the caster, `width` on each side.
        let g = one(
            serde_json::json!({"type": "LineRangeProjectile", "width": 27000, "length": 100000, "applied_target": "Enemy"}),
        );
        assert_eq!(
            g.footprints[0].shape,
            Shape::Corridor {
                radius: 27.,
                length: 100.
            }
        );
        assert_eq!(g.footprints[0].placement, Placement::Caster);
        // invincible Q: a dash to the target, its punch has no area.
        let g = one(
            serde_json::json!({"type": "MoveToTarget", "speed": 5200, "range": 60000, "end_effects": [
            {"type": "CasterAnimation", "name": "skill_land"}, {"type": "Attack", "damage": 70}]}),
        );
        assert_eq!(g.summary(), "dash");
        assert!(g.issues.is_empty());
        // abbie R: the circle "around the caster" after the dash is at the landing point.
        let g = one(
            serde_json::json!({"type": "MoveToTarget", "speed": 3000, "range": 70000, "end_effects": [
            {"type": "RangeEffect", "shape": {"Circle": {"radius": 120000}}, "target": "EnemyChampion", "apply_type": "AroundCaster", "effects": []}]}),
        );
        assert!(g.footprints.contains(&Footprint {
            shape: Shape::Circle { radius: 120. },
            placement: Placement::Aim,
            allies: false,
            projectile: false
        }));
        // league_annie R: a lob whose landing circle is at the target.
        let g = one(
            serde_json::json!({"type": "ParabolicProjectile", "range": 1000000, "shape": {"Circle": {"radius": 1}}, "end_effects": [
            {"type": "RangeProjectile", "shape": {"Circle": {"radius": 30000}}, "applied_target": "EnemyWithoutTower"}]}),
        );
        assert_eq!(g.footprints[0].shape, Shape::Circle { radius: 30. });
        assert_eq!(g.footprints[0].placement, Placement::Aim);
        assert_eq!(g.footprints.len(), 1);
        // bulldozer R: the lob's own circle damages and stuns on landing.
        let g = one(
            serde_json::json!({"type": "ParabolicProjectile", "travel_time": 20, "range": 40000,
            "shape": {"Circle": {"radius": 120000}}, "applied_target": "EnemyWithoutTower",
            "applied_effects": [{"casting_type": "Targeting", "effect": {"type": "Stun", "duration": 90}}], "end_effects": []}),
        );
        assert_eq!(g.footprints[0].shape, Shape::Circle { radius: 120. });
        assert_eq!(g.footprints[0].placement, Placement::Aim);
        // voidling Q: a splash around the target.
        let g = one(
            serde_json::json!({"type": "TargetSplashProjectile", "speed": 2500, "range": 45000, "applied_target": "EnemyWithoutTower"}),
        );
        assert_eq!(g.footprints[0].shape, Shape::Circle { radius: 45. });
        // heavyblade Q: the usual (uncharged) variant.
        let g = one(
            serde_json::json!({"type": "SwitchByBuff", "buff_name": "heavyblade_charged",
            "effect_none": {"type": "RangeEffect", "shape": {"Circle": {"radius": 60000}}, "apply_type": "AroundCaster", "effects": []},
            "effect_buff": {"type": "RangeEffect", "shape": {"Circle": {"radius": 90000}}, "apply_type": "AroundCaster", "effects": []}}),
        );
        assert_eq!(g.footprints[0].shape, Shape::Circle { radius: 60. });
        // candygel R: a charge; kharazim R's random strike has nothing to aim.
        assert_eq!(one(serde_json::json!({"type": "RushTime", "speed": 2700, "tick": 44, "range": 72000, "penetrate": true, "applied_effects": []})).summary(), "dash");
        assert_eq!(
            one(serde_json::json!({"type": "RandomTarget", "range": 78000, "effects": []}))
                .summary(),
            "no-area"
        );
    }
    #[test]
    fn the_range_is_a_sibling_of_the_skill_colour_with_a_faint_fill() {
        let st = style();
        assert_eq!(range_color(st, true, false), st.range_ready);
        assert_eq!(range_color(st, true, true), st.range_ally);
        assert_eq!(range_color(st, false, true), st.gray);
        // Distinct from the shapes' colours, and fainter than their fill.
        assert!(st.range_ready != st.yellow && st.range_ally != st.ally);
        assert!(st.range_fill < st.area_fill);
        let mut d = Drawing::default();
        range_ring(&mut d, st, (960., 540.), 100., st.range_ready);
        assert_eq!(d.discs.len(), 1);
        assert_eq!(rgb(d.discs[0].color), st.range_ready);
    }
    #[test]
    fn an_adding_assignment_keeps_what_the_game_reads() {
        // Zook W: the game reads the 32-unit blast around Zook; the jump
        // away is the mod's own code, added by the author.
        let read = || {
            let mut g = Geometry::default();
            g.add(Shape::Circle { radius: 32. }, Placement::Caster);
            g
        };
        let add = assigned(&serde_json::json!({"type": "jump_back", "distance": 26, "add": true}));
        let g = main_purpose(combine(add, read), true);
        assert_eq!(
            g.footprints.iter().map(|f| f.shape).collect::<Vec<_>>(),
            [
                Shape::Circle { radius: 32. },
                Shape::Backstep { length: 26. }
            ]
        );
        // Without "add", the assignment replaces what is read.
        let replace = assigned(&serde_json::json!({"type": "jump_back", "distance": 26}));
        assert_eq!(combine(replace, read).footprints.len(), 1);
        // No assignment: what is read.
        assert_eq!(combine(None, read), read());
    }
    #[test]
    fn every_preview_type_becomes_its_shape() {
        let one = |v: serde_json::Value| assigned(&v).map(|g| g.footprints);
        // Harpooner R: 5 harpoons over 36 degrees, 18 wide, the cast range.
        assert_eq!(
            one(serde_json::json!({"type": "fan", "count": 5, "width": 18, "spread": 36})).unwrap()
                [0]
            .shape,
            Shape::Spread {
                count: 5,
                radius: 9.,
                length: 0.,
                degrees: 36.
            }
        );
        // Archer W: a 30-unit jump back.
        assert_eq!(
            one(serde_json::json!({"type": "jump_back", "distance": 30})).unwrap()[0].shape,
            Shape::Backstep { length: 30. }
        );
        // Chef W: a heal around herself, in the ally colour.
        let heal =
            one(serde_json::json!({"type": "self_circle", "radius": 50, "allies": true})).unwrap();
        assert_eq!(
            (heal[0].placement, heal[0].allies),
            (Placement::Caster, true)
        );
        // Barrier Magician R: a ring on the target; a plain unit target has no area.
        assert_eq!(
            one(serde_json::json!({"type": "unit_target", "area_radius": 60})).unwrap()[0]
                .placement,
            Placement::Aim
        );
        let plain = assigned(&serde_json::json!({"type": "unit_target"})).unwrap();
        assert!(plain.known_without_area());
        // Soldier Q (passive), Spirit Caller R (instant): nothing at all,
        // not even the range.
        let hidden = assigned(&serde_json::json!({"type": "no_preview"})).unwrap();
        assert!(hidden.hidden && hidden.footprints.is_empty());
        let none = drawing(
            frame(),
            Preview {
                origin: (480000, 480000),
                aim: Some((520000, 480000)),
                casting: 1,
                self_target: false,
                allies: false,
                range: 90000,
                ready: true,
                geometry: hidden,
                target: None,
            },
        );
        assert!(none.strokes.is_empty() && none.discs.is_empty());
        for (v, shape) in [
            (
                serde_json::json!({"type": "aim_circle", "radius": 40}),
                "circle",
            ),
            (
                serde_json::json!({"type": "aim_rectangle", "width": 60, "depth": 30}),
                "rectangle",
            ),
            (
                serde_json::json!({"type": "line", "width": 20, "length": 90}),
                "strip",
            ),
            (
                serde_json::json!({"type": "cone", "radius": 70, "angle": 90}),
                "cone",
            ),
            (serde_json::json!({"type": "dash", "distance": 50}), "dash"),
            (serde_json::json!({"type": "dash", "blink": true}), "blink"),
        ] {
            assert_eq!(assigned(&v).unwrap().summary(), shape, "{v}");
        }
        // Unknown types, missing values and nonsense numbers are refused.
        for v in [
            serde_json::json!({"type": "spiral"}),
            serde_json::json!({"type": "aim_circle"}),
            serde_json::json!({"type": "aim_circle", "radius": -5}),
        ] {
            assert!(assigned(&v).is_none(), "{v}");
        }
        assert_eq!(spread_offsets(5, 36.), vec![-18., -9., 0., 9., 18.]);
        assert_eq!(spread_offsets(1, 36.), vec![0.]);
    }
    #[test]
    fn a_skill_not_understood_shows_only_its_cast_style() {
        let mut unknown = Geometry::default();
        unknown.issue("opaque Native");
        let preview = |casting| Preview {
            origin: (480000, 480000),
            aim: Some((520000, 480000)),
            casting,
            self_target: false,
            allies: false,
            range: 90000,
            ready: true,
            geometry: unknown.clone(),
            target: None,
        };
        let base = drawing(
            frame(),
            Preview {
                aim: None,
                ..preview(2)
            },
        );
        // Direction cast: an arrow, three strokes beyond the range ring,
        // no disc; point cast: a ring with a centre dot.
        let arrow = drawing(frame(), preview(2));
        assert!(arrow.strokes.len() > base.strokes.len());
        assert_eq!(arrow.discs.len(), base.discs.len());
        let ring = drawing(frame(), preview(1));
        assert_eq!(ring.discs.len(), base.discs.len() + 1);
        // Neither is the old white crosshair.
        let white = style().white;
        assert!(arrow
            .strokes
            .iter()
            .chain(&ring.strokes)
            .all(|s| rgb(s.color) != white));
    }
    #[test]
    fn a_dash_is_drawn_only_when_moving_is_the_skills_main_purpose() {
        let dash = Footprint {
            shape: Shape::Movement { blink: false },
            placement: Placement::Aim,
            allies: false,
            projectile: false,
        };
        let pool = Footprint {
            shape: Shape::Circle { radius: 42. },
            placement: Placement::Aim,
            ..dash
        };
        let shot = Footprint {
            shape: Shape::Corridor {
                radius: 5.,
                length: 70.,
            },
            placement: Placement::Caster,
            projectile: true,
            ..dash
        };
        let kept = |footprints: Vec<Footprint>, unit: bool| {
            main_purpose(
                Geometry {
                    footprints,
                    ..Geometry::default()
                },
                unit,
            )
            .footprints
        };
        // A pure dash: the path.
        assert_eq!(kept(vec![dash], false), vec![dash]);
        // Candygel R: the pools only.
        assert_eq!(kept(vec![dash, pool, pool], false), vec![pool, pool]);
        // Candygel W, cast on an ally: no path.
        assert!(kept(vec![dash], true).is_empty());
        // A shot beside a dash is not an area left behind: both stay.
        assert_eq!(kept(vec![dash, shot], false), vec![dash, shot]);
    }
    #[test]
    fn projectile_end_is_the_target_or_the_full_flight_length() {
        let end = Placement::End { length: 50. };
        // Unit-targeted: where the target is.
        assert_eq!(placed((0., 0.), (30., 0.), end, 60., 0), (30., 0.));
        // Skillshot: the full length toward the aim, even past a near cursor.
        assert_eq!(placed((0., 0.), (10., 0.), end, 60., 1), (50., 0.));
        // A lobbed shot lands where a location cast is aimed, within its
        // flight length; a direction cast still flies its full length.
        let landing = Placement::Landing { length: 50. };
        assert_eq!(placed((0., 0.), (10., 0.), landing, 60., 1), (10., 0.));
        assert_eq!(placed((0., 0.), (90., 0.), landing, 60., 1), (50., 0.));
        assert_eq!(placed((0., 0.), (10., 0.), landing, 60., 2), (50., 0.));
        assert_eq!(placed((0., 0.), (30., 0.), landing, 60., 0), (30., 0.));
    }
    #[test]
    fn live_range_and_point_clamp_are_distinct_and_keep_diagonal_aim() {
        assert_eq!(endpoint((0., 0.), (3., 4.), 10., true), (6., 8.));
        assert_eq!(endpoint((0., 0.), (3., 4.), 10., false), (3., 4.));
        assert_eq!(endpoint((0., 0.), (30., 40.), 10., false), (6., 8.));
        assert_eq!(endpoint((5., 5.), (5., 5.), 10., true), (5., 5.));
    }
    #[test]
    fn declarations_keep_placement_distinct_and_do_not_flatten_hit_children() {
        let base: Value = serde_json::from_str(include_str!("preview_assets.json")).unwrap();
        assert_eq!(
            skills(&base["nightmare"])[0].footprints[0],
            Footprint {
                shape: Shape::Corridor {
                    radius: 5.,
                    length: 46.
                },
                placement: Placement::Caster,
                allies: false,
                projectile: true,
            }
        );
        assert_eq!(
            skills(&base["nightmare"])[1].footprints[0].shape,
            Shape::Movement { blink: false }
        );
        let spec = serde_json::json!({"ult":{"casting_type":"Targeting","effect":{
            "type":"RangeEffect","apply_type":"AroundCaster","shape":{"Circle":{"radius":10000}},
            "effects":[{"type":"RangeProjectile","shape":{"Circle":{"radius":90000}}}]}}});
        let g = skills(&spec)[2].clone();
        assert_eq!(g.footprints.len(), 1);
        assert!(g.self_area());
        let spec = serde_json::json!({"skill":{"effect":{"type":"SwitchByBuff","effects":[{"type":"RangeEffect"}]}}});
        assert!(skills(&spec)[0].footprints.is_empty());
        assert!(!skills(&spec)[0].issues.is_empty());
        assert!(number(&serde_json::json!({"radius":-1}), "radius").is_none());
    }
    #[test]
    fn live_dimensions_override_declarations_and_opaque_families_are_reported() {
        let mut live = Geometry::default();
        live.add(Shape::Circle { radius: 13. }, Placement::Caster);
        live.issue("opaque sibling");
        let mut file = Geometry::default();
        file.add(Shape::Circle { radius: 10. }, Placement::Aim);
        assert_eq!(resolve(Some(&live), file.clone()), live);
        let mut unknown = Geometry::default();
        unknown.issue("foreign table");
        let fallback = resolve(Some(&unknown), file.clone());
        assert_eq!(fallback.footprints, file.footprints);
        assert_eq!(fallback.issues, ["runtime: foreign table"]);
        // A shipped assignment that draws nothing still wins over the file.
        let hidden = Geometry {
            hidden: true,
            ..Geometry::default()
        };
        assert!(resolve(Some(&hidden), file.clone()).hidden);
        let mut bare = Geometry::default();
        bare.family(crate::preview_table::ASSIGNMENT);
        assert!(resolve(Some(&bare), file.clone()).footprints.is_empty());
    }
    #[test]
    fn forward_centers_and_cone_threshold_match_native_geometry() {
        assert_eq!(
            placed(
                (10., 20.),
                (13., 24.),
                Placement::Forward { offset: 10. },
                100.,
                0
            ),
            (16., 28.)
        );
        assert_eq!(
            placed(
                (10., 20.),
                (13., 24.),
                Placement::Forward { offset: 10. },
                100.,
                2
            ),
            (10., 20.)
        );
        assert_eq!(
            placed((10., 20.), (130., 240.), Placement::Aim, 100., 0),
            (130., 240.)
        );
        let p = cone((0., 0.), (1., 0.), 40., 0.5);
        assert!((p[1].0 - 20.).abs() < 0.01 && (p[1].1 + 34.641).abs() < 0.01);
        assert_eq!(
            corridor((0., 0.), (10., 0.), 3.).unwrap(),
            [(0., 3.), (10., 3.), (10., -3.), (0., -3.)]
        );
    }
    #[test]
    fn clipping_keeps_offscreen_crossing_lines_and_cuts_minimap_and_hud() {
        let viewport = Rect {
            x: 0.,
            y: 0.,
            w: 200.,
            h: 100.,
        };
        let blockers = [
            Rect {
                x: 40.,
                y: 10.,
                w: 20.,
                h: 80.,
            },
            Rect {
                x: 150.,
                y: 0.,
                w: 50.,
                h: 100.,
            },
        ];
        let segments = visible((-50., 50.), (250., 50.), viewport, &blockers, 2.);
        assert_eq!(segments.len(), 2);
        for (a, b) in segments {
            let mid = lerp(a, b, 0.5);
            assert!(viewport.contains(mid));
            assert!(!blockers.iter().any(|r| r.contains(mid)));
            assert!((a.1 - 50.).abs() < 0.01 && (b.1 - 50.).abs() < 0.01);
        }
        assert!(visible((50., 20.), (50., 80.), viewport, &blockers, 2.).is_empty());
    }
    #[test]
    fn an_area_for_allies_is_drawn_in_the_ally_colour_unless_on_cooldown() {
        let st = style();
        let preview = |ready| {
            let mut geometry = Geometry::default();
            geometry.add_for(Shape::Circle { radius: 60. }, Placement::Caster, true);
            geometry.add(Shape::Circle { radius: 40. }, Placement::Caster);
            Preview {
                origin: (480000, 480000),
                aim: Some((600000, 480000)),
                casting: 3,
                self_target: false,
                allies: false,
                range: 0,
                ready,
                geometry,
                target: None,
            }
        };
        let colours = |d: &Drawing| {
            let has = |c| d.strokes.iter().any(|s| rgb(s.color) == c);
            (has(st.ally), has(st.yellow))
        };
        // The shield ring in the ally colour, the pull ring in yellow.
        assert_eq!(colours(&drawing(frame(), preview(true))), (true, true));
        assert_eq!(
            colours(&hover_drawing(frame(), preview(true))),
            (true, true)
        );
        assert_eq!(colours(&drawing(frame(), preview(false))), (false, false));
        // Declared effects name their target.
        let ally = serde_json::json!({"target": "AllyChampion"});
        let enemy = serde_json::json!({"applied_target": "EnemyWithoutTower"});
        assert!(declared_allies(&ally) && !declared_allies(&enemy));
        assert!(targets_allies(4) && !targets_allies(5) && !targets_allies(13));
    }
    #[test]
    fn a_declared_native_shot_is_a_strip_and_other_native_effects_stay_opaque() {
        let shot = |v: Value| {
            let spec = serde_json::json!({"skill": {"range": 150000, "effect": {"type": "Combine", "effects": [v]}}});
            skills(&spec)[0].clone()
        };
        // Crossbowman W: radius 6, range 150.
        let g = shot(
            serde_json::json!({"type": "Native", "effect_ref": "crossbowman_skill2",
            "speed": 6000, "range": 150000, "projectile_radius": 6000}),
        );
        assert_eq!(
            g.footprints[0].shape,
            Shape::Corridor {
                radius: 6.,
                length: 150.
            }
        );
        // Harpooner Q has no range of its own: the cast range (length 0).
        let g = shot(serde_json::json!({"type": "Native", "projectile_radius": 11000}));
        assert_eq!(
            g.footprints[0].shape,
            Shape::Corridor {
                radius: 11.,
                length: 0.
            }
        );
        // Astrologer: circles at the aim, and a line half its width each side.
        let g = shot(
            serde_json::json!({"type": "Native", "effect_ref": "astrologer_ult", "radius": 60000, "delay": 90}),
        );
        assert_eq!(
            (g.footprints[0].shape, g.footprints[0].placement),
            (Shape::Circle { radius: 60. }, Placement::Aim)
        );
        let g = shot(
            serde_json::json!({"type": "Native", "effect_ref": "astrologer_skill2", "line_width": 32000}),
        );
        assert_eq!(
            g.footprints[0].shape,
            Shape::Corridor {
                radius: 16.,
                length: 0.
            }
        );
        // Several projectiles, or no projectile: not understood.
        for v in [
            serde_json::json!({"type": "Native", "projectile_radius": 9000, "projectile_count": 5}),
            serde_json::json!({"type": "Native", "radius": 60000}),
        ] {
            let g = shot(v);
            assert!(g.footprints.is_empty() && !g.issues.is_empty());
        }
    }
    #[test]
    fn footprint_world_dimensions_scale_with_camera_and_render_is_bounded() {
        let frame = CameraFrame {
            viewport: Rect {
                x: 0.,
                y: 0.,
                w: 1920.,
                h: 1080.,
            },
            minimap: Rect {
                x: 1600.,
                y: 750.,
                w: 320.,
                h: 330.,
            },
            center: (480., 480.),
            extent: (1024., 1024.),
            zoom: 1.,
        };
        for extent in [512., 1024., 2048.] {
            let frame = CameraFrame {
                extent: (extent, extent),
                zoom: 1.,
                ..frame
            };
            let d = drawing(
                frame,
                Preview {
                    origin: (480000, 480000),
                    aim: Some((550000, 480000)),
                    casting: 2,
                    range: 120000,
                    ready: true,
                    self_target: false,
                    allies: false,
                    geometry: Geometry {
                        footprints: vec![Footprint {
                            shape: Shape::Corridor {
                                radius: 10.,
                                length: 120.,
                            },
                            placement: Placement::Caster,
                            allies: false,
                            projectile: false,
                        }],
                        ..Geometry::default()
                    },
                    target: None,
                },
            );
            assert!(d.strokes.len() < MAX_STROKES);
            let edge = 540. + 10. * 2048. / extent;
            assert!(d
                .strokes
                .iter()
                .any(|s| rgb(s.color) == style().yellow && (s.a.1 - edge).abs() < 0.01));
            assert!(d
                .strokes
                .iter()
                .all(|s| [s.a.0, s.a.1, s.b.0, s.b.1].into_iter().all(f32::is_finite)));
        }
    }
    #[test]
    fn the_attack_range_ring_shows_the_range_after_the_cast() {
        // Bombardier Q: attack range 65, deploying adds 50.
        assert_eq!(reach_radius(65., 50., false), 115.);
        // Deployed, the attack range already reads 115; packing returns to 65.
        assert_eq!(reach_radius(115., 50., true), 65.);
        assert_eq!(reach_radius(30., 50., true), 0.);
        let g = assigned(&serde_json::json!({"type": "attack_reach", "extra": 50})).unwrap();
        assert!(matches!(g.footprints[0].shape, Shape::Reach { extra } if extra == 50.));
        assert!(assigned(&serde_json::json!({"type": "attack_reach"})).is_none());
    }
}
