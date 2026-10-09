//! Code-native geometry adapted from design/previews/index.html.
//! Live effect families supply footprints; explicit declarations are a fallback.
//! Unsupported effects retain a reach/aim guide, without guessed hits.
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
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footprint {
    pub shape: Shape,
    pub placement: Placement,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Geometry {
    pub footprints: Vec<Footprint>,
    pub issues: Vec<String>,
    pub families: Vec<String>,
}
impl Geometry {
    pub fn add(&mut self, shape: Shape, placement: Placement) {
        let footprint = Footprint { shape, placement };
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
                        Shape::Corridor { .. } | Shape::Cone { .. } | Shape::Movement { .. }
                    )
            })
    }
}
static SHAPES: OnceLock<HashMap<String, [Geometry; 3]>> = OnceLock::new();
pub fn initialize(log: &Logger) {
    let base: Value =
        serde_json::from_str(include_str!("preview_assets.json")).expect("preview declarations");
    let mut shapes = HashMap::new();
    for (name, value) in base.as_object().expect("preview map") {
        shapes.insert(name.clone(), skills(value));
    }
    for (name, value) in hud_icons::enabled_assets().descriptions {
        shapes.insert(name, skills(&value));
    }
    let count = shapes
        .values()
        .flatten()
        .filter(|s| !s.footprints.is_empty())
        .count();
    log.write(&format!("PREVIEW declared_slots={count}; runtime effect geometry preferred; input and footprint independent"));
    let _ = SHAPES.set(shapes);
}
pub fn geometry(name: Option<&str>, slot: usize) -> Geometry {
    name.and_then(|n| SHAPES.get()?.get(n)?.get(slot).cloned())
        .unwrap_or_default()
}
/// Recognized runtime footprints are authoritative, even if sibling effects are
/// unresolved. A file declaration can fill an opaque tree, not replace live dimensions.
pub fn resolve(native: Option<&Geometry>, declared: Geometry) -> Geometry {
    match native {
        Some(g) if !g.footprints.is_empty() => g.clone(),
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
                for child in list {
                    collect(child, inherited, out, depth + 1);
                }
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
                for child in list {
                    collect(child, placement, out, depth + 1);
                }
            } else {
                out.issue(format!("unsupported {kind} wrapper"));
            }
        }
        "LinearProjectile" | "BackToCasterLinearProjectile" => {
            if let (Some(Shape::Circle { radius }), Some(length)) =
                (declared_shape(v), number(v, "range"))
            {
                out.add(Shape::Corridor { radius, length }, Placement::Caster);
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
                    out.add(shape, placement);
                }
            } else {
                out.issue("RangeEffect placement/shape unresolved");
            }
            // Applied descendants are effects on hit units, not additional primary footprints.
        }
        "RangeProjectile" | "RangePeriodProjectile" => {
            if let Some(shape) = declared_shape(v) {
                out.add(shape, inherited);
            } else {
                out.issue("range projectile shape unresolved");
            }
        }
        "LineRangeProjectile" => {
            out.issue("LineRangeProjectile width/placement semantics unresolved");
        }
        "MoveTo" | "Teleport" => out.add(
            Shape::Movement {
                blink: kind == "Teleport",
            },
            Placement::Aim,
        ),
        "Native"
        | "SwitchByBuff"
        | "RandomTarget"
        | "ParabolicProjectile"
        | "Rush"
        | "RushTime"
        | "MoveToTarget"
        | "RushMoveToBack"
        | "TargetSplashProjectile"
        | "ShrinkingBarrier" => out.issue(format!("opaque {kind}")),
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
        | "Taunt" => {}
        _ => out.issue(format!("unknown declaration {kind}")),
    }
}

const YELLOW: u32 = 0xfdee00ff;
const ORANGE: u32 = 0xff642eff;
const WHITE: u32 = 0xffffffb3;
const CYAN: u32 = 0x53b8e4ff;
const GREY: u32 = 0x999999aa;
const INK: u32 = 0x1c1c1c8c;
#[derive(Clone, Copy)]
struct Stroke {
    a: Point,
    b: Point,
    width: f32,
    color: u32,
    z: i32,
}
#[derive(Default)]
pub struct Drawing {
    strokes: Vec<Stroke>,
}
impl Drawing {
    pub(crate) fn line(&mut self, a: Point, b: Point, width: f32, color: u32, z: i32) {
        if self.strokes.len() < 2048 && [a.0, a.1, b.0, b.1].into_iter().all(f32::is_finite) {
            self.strokes.push(Stroke {
                a,
                b,
                width,
                color,
                z,
            });
        }
    }
    fn outlined(&mut self, a: Point, b: Point, width: f32, color: u32) {
        self.line(a, b, width + 3., INK, 988);
        self.line(a, b, width, color, 990);
    }
    fn dashed(&mut self, a: Point, b: Point, dash: f32, gap: f32, color: u32) {
        let length = (b.0 - a.0).hypot(b.1 - a.1);
        if length <= 0. {
            return;
        }
        for i in 0..((length / (dash + gap)).ceil() as usize).min(512) {
            let start = i as f32 * (dash + gap) / length;
            let end = ((i as f32 * (dash + gap) + dash) / length).min(1.);
            self.outlined(lerp(a, b, start), lerp(a, b, end), 2., color);
        }
    }
    fn circle(&mut self, center: Point, rx: f32, ry: f32, dashed: bool, color: u32) {
        let count = ((rx.max(ry) * std::f32::consts::TAU / 8.).ceil() as usize).clamp(32, 256);
        for i in 0..count {
            if !dashed || i % 4 < 2 {
                let point = |j: usize| {
                    let a = j as f32 * std::f32::consts::TAU / count as f32;
                    (center.0 + rx * a.cos(), center.1 + ry * a.sin())
                };
                self.outlined(point(i), point(i + 1), 2., color);
            }
        }
        if dashed {
            for i in 0..12 {
                let a = i as f32 * std::f32::consts::TAU / 12.;
                self.outlined(
                    (
                        center.0 + (rx - 6.) * a.cos(),
                        center.1 + (ry - 6.) * a.sin(),
                    ),
                    (
                        center.0 + (rx + 6.) * a.cos(),
                        center.1 + (ry + 6.) * a.sin(),
                    ),
                    2.,
                    color,
                );
            }
        }
    }
    fn polygon(&mut self, points: &[Point], color: u32, viewport: Rect) {
        if points.len() < 3 {
            return;
        }
        // Screen-space 45-degree hatching, same nine-pixel pitch as the SVG.
        // Even/odd intersection pairs also support cone sectors wider than 180 degrees.
        let lo = points
            .iter()
            .map(|p| p.0 + p.1)
            .fold(f32::INFINITY, f32::min)
            .max(viewport.x + viewport.y);
        let hi = points
            .iter()
            .map(|p| p.0 + p.1)
            .fold(f32::NEG_INFINITY, f32::max)
            .min(viewport.x + viewport.y + viewport.w + viewport.h);
        let hatch = (color & 0xffffff00) | 0x60;
        let start = (lo / 9.).floor() * 9.;
        for i in 0..(((hi - start) / 9.).ceil().max(0.) as usize).min(512) {
            let c = start + i as f32 * 9.;
            let mut hits = Vec::new();
            for j in 0..points.len() {
                let (a, b) = (points[j], points[(j + 1) % points.len()]);
                let (s, e) = (a.0 + a.1, b.0 + b.1);
                if (s <= c && e > c) || (e <= c && s > c) {
                    hits.push(lerp(a, b, (c - s) / (e - s)));
                }
            }
            hits.sort_by(|a, b| a.0.total_cmp(&b.0));
            for pair in hits.as_chunks::<2>().0 {
                self.line(pair[0], pair[1], 1.5, hatch, 989);
            }
        }
        for i in 0..points.len() {
            self.outlined(points[i], points[(i + 1) % points.len()], 2.5, color);
        }
    }
    fn area_circle(&mut self, center: Point, rx: f32, ry: f32, color: u32, viewport: Rect) {
        let points = (0..64)
            .map(|i| {
                let a = i as f32 * std::f32::consts::TAU / 64.;
                (center.0 + rx * a.cos(), center.1 + ry * a.sin())
            })
            .collect::<Vec<_>>();
        self.polygon(&points, color, viewport);
    }
    pub fn render(self, ctx: &mut StableClient<'_>, frame: CameraFrame, blockers: &[Rect]) {
        for s in self.strokes {
            for (a, b) in visible(s.a, s.b, frame.viewport, blockers, s.width + 2.) {
                ctx.draw_line("UI", a.0, a.1, b.0, b.1, s.width, s.z, s.color);
            }
        }
    }
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
pub fn drawing(frame: CameraFrame, p: Preview) -> Drawing {
    let mut out = Drawing::default();
    if !frame.valid() {
        return out;
    }
    let origin = (p.origin.0 as f32 / 1000., p.origin.1 as f32 / 1000.);
    let project = |p: Point| frame.project_unclipped(p.0, p.1);
    let center = project(origin);
    let range = p.range as f32 / 1000.;
    let color = if p.ready { YELLOW } else { GREY };
    let (sx, sy) = (2048. / frame.extent.0, 2048. / frame.extent.1);
    let self_area = p.self_target || p.casting == 3 || p.geometry.self_area();
    if range > 0. && !self_area {
        out.circle(
            center,
            range * sx,
            range * sy,
            true,
            if p.ready { WHITE } else { GREY },
        );
    }
    let world_target = p
        .target
        .map(|t| (t.position.0 as f32 / 1000., t.position.1 as f32 / 1000.));
    let aim = if p.self_target {
        Some(origin)
    } else {
        world_target.or(p.aim.map(|a| (a.0 as f32 / 1000., a.1 as f32 / 1000.)))
    };
    // Unit input contributes brackets and approach feedback, never an early
    // return that erases a corridor or an area declared by the effect.
    if p.casting == 0 && !p.self_target {
        if let Some(target) = p.target {
            let world = world_target.unwrap();
            let at = project(world);
            let in_range = (world.0 - origin.0).hypot(world.1 - origin.1) <= range;
            let col = if !p.ready {
                GREY
            } else if in_range {
                YELLOW
            } else {
                ORANGE
            };
            if in_range {
                out.outlined(center, at, 2., col);
            } else {
                out.dashed(center, at, 6., 6., col);
            }
            let (x0, y0, x1, y1, margin) = crate::combat::screen_area(frame, &target);
            let (x, y, w, h) = (
                x0 - margin,
                y0 - margin,
                x1 - x0 + 2. * margin,
                y1 - y0 + 2. * margin,
            );
            let corner = w.min(h).min(24.) * 0.3;
            for (cx, cy, dx, dy) in [
                (x, y, 1., 1.),
                (x + w, y, -1., 1.),
                (x, y + h, 1., -1.),
                (x + w, y + h, -1., -1.),
            ] {
                out.outlined((cx, cy), (cx + dx * corner, cy), 3., col);
                out.outlined((cx, cy), (cx, cy + dy * corner), 3., col);
            }
        }
    }
    for f in &p.geometry.footprints {
        // A unit-targeted footprint needs a selected unit unless it is self-centered.
        if p.casting == 0 && !self_area && world_target.is_none() {
            continue;
        }
        let aiming = aim.unwrap_or(origin);
        let base = placed(origin, aiming, f.placement, range, p.casting);
        match f.shape {
            Shape::Circle { radius } => out.area_circle(
                project(base),
                radius * sx,
                radius * sy,
                color,
                frame.viewport,
            ),
            Shape::Rectangle { width, height } => {
                let points = [
                    (base.0 - width / 2., base.1 - height / 2.),
                    (base.0 + width / 2., base.1 - height / 2.),
                    (base.0 + width / 2., base.1 + height / 2.),
                    (base.0 - width / 2., base.1 + height / 2.),
                ]
                .map(project);
                out.polygon(&points, color, frame.viewport);
            }
            Shape::Cone { radius, cosine } => {
                // Current predicate's facing is caster -> area center, not
                // area center -> cursor (which can reverse on short clicks).
                if (base.0 - origin.0).hypot(base.1 - origin.1) > f32::EPSILON {
                    let facing = (2. * base.0 - origin.0, 2. * base.1 - origin.1);
                    let points = cone(base, facing, radius, cosine)
                        .into_iter()
                        .map(project)
                        .collect::<Vec<_>>();
                    out.polygon(&points, color, frame.viewport);
                }
            }
            Shape::Corridor { radius, length } => {
                // Length 0: as far as the cast range (dashes). A zero-width
                // projectile is still drawn as a thin line.
                let length = if length > 0. { length } else { range };
                let radius = radius.max(1.5);
                if let Some(points) = corridor(base, endpoint(base, aiming, length, true), radius) {
                    out.polygon(&points.map(project), color, frame.viewport);
                }
            }
            Shape::Segment { radius, from, to } => {
                if let Some(points) = corridor(from, to, radius) {
                    out.polygon(&points.map(project), color, frame.viewport);
                }
            }
            Shape::Movement { blink } => {
                let at = project(base);
                let col = if p.ready { CYAN } else { GREY };
                if blink {
                    out.dashed(center, at, 2., 8., col);
                } else {
                    out.outlined(center, at, 3., col);
                }
                out.circle(at, 12., 12., false, col);
                arrow(&mut out, at, (at.0 - center.0, at.1 - center.1), col);
            }
        }
    }
    if let Some(aim) = aim.filter(|_| !self_area && p.casting != 0) {
        let end = endpoint(origin, aim, range, p.casting == 2);
        let at = project(end);
        if p.geometry.footprints.is_empty() {
            out.outlined(center, at, 2., color);
            arrow(&mut out, at, (at.0 - center.0, at.1 - center.1), color);
        }
        if p.casting == 1 && (end.0 - aim.0).hypot(end.1 - aim.1) > 0.1 {
            let col = if p.ready { ORANGE } else { GREY };
            out.dashed(at, project(aim), 6., 6., col);
            out.circle(project(aim), 9., 9., false, col);
        }
    }
    out
}
/// HUD hover has no battlefield aim. Show known reach and caster-centered
/// areas only; never orient a shot toward the icon or reuse a previous aim.
pub fn hover_drawing(frame: CameraFrame, p: Preview) -> Drawing {
    let mut out = Drawing::default();
    if !frame.valid() {
        return out;
    }
    let center = frame.project_unclipped(p.origin.0 as f32 / 1000., p.origin.1 as f32 / 1000.);
    let (sx, sy) = (2048. / frame.extent.0, 2048. / frame.extent.1);
    let self_area = p.self_target || p.casting == 3 || p.geometry.self_area();
    if p.range > 0 && !self_area {
        let range = p.range as f32 / 1000.;
        out.circle(
            center,
            range * sx,
            range * sy,
            true,
            if p.ready { WHITE } else { GREY },
        );
    }
    let color = if p.ready { YELLOW } else { GREY };
    for footprint in &p.geometry.footprints {
        let centered = footprint.placement == Placement::Caster
            || footprint.placement == Placement::Aim && (p.self_target || p.casting == 3);
        if !centered {
            continue;
        }
        match footprint.shape {
            Shape::Circle { radius } => {
                out.area_circle(center, radius * sx, radius * sy, color, frame.viewport)
            }
            Shape::Rectangle { width, height } => {
                let points = [
                    (-width / 2., -height / 2.),
                    (width / 2., -height / 2.),
                    (width / 2., height / 2.),
                    (-width / 2., height / 2.),
                ]
                .map(|(x, y)| (center.0 + x * sx, center.1 + y * sy));
                out.polygon(&points, color, frame.viewport);
            }
            _ => {} // Directional footprints need an actual battlefield aim.
        }
    }
    out
}
fn arrow(out: &mut Drawing, tip: Point, direction: Point, color: u32) {
    let len = direction.0.hypot(direction.1);
    if len < 1. {
        return;
    }
    let (x, y) = (direction.0 / len, direction.1 / len);
    out.outlined(
        tip,
        (tip.0 - x * 12. - y * 6., tip.1 - y * 12. + x * 6.),
        2.5,
        color,
    );
    out.outlined(
        tip,
        (tip.0 - x * 12. + y * 6., tip.1 - y * 12. - x * 6.),
        2.5,
        color,
    );
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
            range: 45000,
            ready: true,
            geometry,
            target: None,
        };
        let d = hover_drawing(frame(), preview);
        assert!(!d.strokes.is_empty());
        assert!(d.strokes.iter().all(|s| s.color == WHITE || s.color == INK));
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
            range: 45000,
            ready: false,
            geometry,
            target: None,
        };
        let d = hover_drawing(frame(), preview);
        assert!(!d.strokes.is_empty());
        assert!(d
            .strokes
            .iter()
            .all(|s| s.color & 0xffffff00 == GREY & 0xffffff00 || s.color == INK));
        assert!(d
            .strokes
            .iter()
            .all(|s| (s.a.0 - 960.).hypot(s.a.1 - 540.) <= 20.01));
        let unknown = Preview {
            origin: (480000, 480000),
            aim: None,
            casting: 3,
            self_target: true,
            range: 0,
            ready: true,
            geometry: Geometry::default(),
            target: None,
        };
        assert!(hover_drawing(frame(), unknown).strokes.is_empty());
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
                range: 45000,
                ready: true,
                geometry,
                target: None,
            },
        );
        assert!(!d.strokes.is_empty());
        assert!(d.strokes.iter().all(|s| s.color != WHITE));
        assert!(d
            .strokes
            .iter()
            .all(|s| (s.a.0 - 960.).hypot(s.a.1 - 540.) <= 20.01));
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
                range: 45000,
                ready: true,
                geometry,
                target: Some(target),
            },
        );
        assert!(d.strokes.iter().any(|s| s.color == YELLOW
            && (s.a.0 - 1220.).abs() < 0.01
            && (s.a.1 - 546.).abs() < 0.01));
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
                range: 40000,
                ready: true,
                geometry,
                target: None,
            },
        );
        assert!(d.strokes.iter().any(|s| s.color == YELLOW
            && (s.a.0 - 1080.).abs() < 0.01
            && (s.a.1 - 540.).abs() < 0.01));
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
                placement: Placement::Caster
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
        };
        for extent in [512., 1024., 2048.] {
            let frame = CameraFrame {
                extent: (extent, extent),
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
                    geometry: Geometry {
                        footprints: vec![Footprint {
                            shape: Shape::Corridor {
                                radius: 10.,
                                length: 120.,
                            },
                            placement: Placement::Caster,
                        }],
                        ..Geometry::default()
                    },
                    target: None,
                },
            );
            assert!(d.strokes.len() < 2048);
            assert!(
                d.strokes
                    .iter()
                    .any(|s| s.color == YELLOW
                        && (s.a.1 - (540. + 10. * 2048. / extent)).abs() < 0.01)
            );
            assert!(d
                .strokes
                .iter()
                .all(|s| [s.a.0, s.a.1, s.b.0, s.b.1].into_iter().all(f32::is_finite)));
        }
    }
}
