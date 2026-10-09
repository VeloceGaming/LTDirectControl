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
    /// Dancer R: `count` blades from the caster, each `length` long with hit
    /// radius `radius`, spread by the game's rule (`fan_offsets`).
    Fan {
        count: usize,
        radius: f32,
        length: f32,
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
                        Shape::Corridor { .. }
                            | Shape::Cone { .. }
                            | Shape::Movement { .. }
                            | Shape::Fan { .. }
                    )
            })
    }
    /// Every effect was recognised and none has an area (a self buff, a
    /// single-target shot): no direction guide is drawn for it.
    pub fn known_without_area(&self) -> bool {
        self.footprints.is_empty() && self.issues.is_empty() && !self.families.is_empty()
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
/// Cast range: a quiet neutral ring, no fill.
fn range_ring(d: &mut Drawing, s: &Style, center: Point, radius: f32) {
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
        alpha(s.gray, s.range_opacity),
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
fn corridor_body(d: &mut Drawing, s: &Style, from: Point, to: Point, half: f32, ink: Ink) {
    let Some(body) = corridor(from, to, half) else {
        return;
    };
    d.fill(s, &body, ink.col, s.corridor_fill);
    let edge = alpha(ink.white, s.corridor_edge_opacity);
    d.line(body[0], body[1], s.corridor_edge_width, edge, Z_CORE);
    d.line(body[3], body[2], s.corridor_edge_width, edge, Z_CORE);
}
/// The open-spearhead arrow from `a` to `b`: tapered spine, two split
/// facets with a bright upper edge, a white tip and a small origin grip.
/// Skillshots use the larger head.
fn arrow(d: &mut Drawing, s: &Style, a: Point, b: Point, ink: Ink, skillshot: bool) {
    let length = (b.0 - a.0).hypot(b.1 - a.1);
    let Some(dir) = unit_vector(a, b).filter(|_| length >= 12.) else {
        d.disc(b, 2.5, alpha(ink.col, 1.));
        return;
    };
    let n = (-dir.1, dir.0);
    let q = |x: f32, y: f32| (a.0 + dir.0 * x + n.0 * y, a.1 + dir.1 * x + n.1 * y);
    let (head, wing) = if skillshot {
        (s.skillshot_head, s.skillshot_wing)
    } else {
        (s.dash_head, s.dash_wing)
    };
    let h = head.min(length * 0.42);
    let wing = h * wing;
    let neck = length - h;
    let outline = |d: &mut Drawing, points: &[Point], width: f32| {
        d.poly(
            points,
            width + s.arrow_ink_extra,
            alpha(s.ink, s.arrow_ink_opacity),
            true,
            Z_INK,
        );
        d.poly(
            points,
            width,
            alpha(ink.col, s.arrow_outline_opacity),
            true,
            Z_MAIN,
        );
    };
    let spine = [
        q(0., 0.),
        q(14f32.min(neck * 0.3), -2.5),
        q((neck - 13.).max(4.), -5.),
        q(neck + 4., 0.),
        q((neck - 13.).max(4.), 5.),
        q(14f32.min(neck * 0.3), 2.5),
    ];
    d.fill(s, &spine, ink.col, s.arrow_body_fill);
    outline(d, &spine, s.arrow_body_width);
    for sign in [-1f32, 1.] {
        let facet = [
            q(neck - 2., wing * sign),
            q(length, 0.),
            q(length - 10., 0.),
            q(neck + 9., wing * 0.4 * sign),
        ];
        let fill = if sign < 0. {
            s.arrow_upper_facet_fill
        } else {
            s.arrow_lower_facet_fill
        };
        d.fill(s, &facet, ink.col, fill);
        outline(d, &facet, s.arrow_head_width);
        if sign < 0. {
            d.line(
                facet[0],
                facet[1],
                s.arrow_head_width,
                alpha(ink.white, s.arrow_edge_opacity),
                Z_CORE,
            );
        }
    }
    d.line(
        q(length - 9., 0.),
        q(length, 0.),
        s.arrow_tip_width,
        alpha(ink.white, 1.),
        Z_TOP,
    );
    let grip_color = alpha(ink.col, s.arrow_grip_opacity);
    d.line(
        q(-5., -8.),
        q(3., -4.),
        s.arrow_grip_width,
        grip_color,
        Z_MAIN,
    );
    d.line(
        q(-5., 8.),
        q(3., 4.),
        s.arrow_grip_width,
        grip_color,
        Z_MAIN,
    );
}
/// Dash: the spearhead from just ahead of the caster to the landing point,
/// or a compact grip when the dash is very short.
fn dash(d: &mut Drawing, s: &Style, caster: Point, end: Point, ink: Ink) {
    match unit_vector(caster, end) {
        Some(dir) if (end.0 - caster.0).hypot(end.1 - caster.1) >= s.dash_short => {
            let start = (
                caster.0 + dir.0 * s.arrow_start,
                caster.1 + dir.1 * s.arrow_start,
            );
            arrow(d, s, start, end, ink, false);
        }
        _ => grip(d, s, end, ink, s.dash_short_grip),
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
    if !frame.valid() {
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
        range_ring(&mut out, s, center, range * sx);
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
            let (x0, y0, x1, _, _) = crate::combat::screen_area(frame, &target);
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
        let aiming = aim.unwrap_or(origin);
        let base = placed(origin, aiming, f.placement, range, p.casting);
        let at = project(base);
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
                corridor_body(&mut out, s, at, end, radius.max(1.5) * sx, ink);
                if let Some(dir) = unit_vector(at, end) {
                    let start = (at.0 + dir.0 * s.arrow_start, at.1 + dir.1 * s.arrow_start);
                    arrow(&mut out, s, start, end, ink, true);
                }
            }
            Shape::Segment { radius, from, to } => {
                corridor_body(&mut out, s, project(from), project(to), radius * sx, ink);
            }
            Shape::Movement { blink: true } => blink(&mut out, s, at, ink),
            Shape::Fan {
                count,
                radius,
                length,
            } => {
                if let Some(dir) = unit_vector(base, aiming) {
                    for offset in fan_offsets(count) {
                        let d = turned(dir, offset);
                        let end = project((base.0 + d.0 * length, base.1 + d.1 * length));
                        corridor_body(&mut out, s, at, end, radius * sx, ink);
                        if let Some(sd) = unit_vector(at, end) {
                            let start = (at.0 + sd.0 * s.arrow_start, at.1 + sd.1 * s.arrow_start);
                            // Smaller heads keep up to 10 blades readable.
                            arrow(&mut out, s, start, end, ink, false);
                        }
                    }
                }
            }
            Shape::Movement { blink: false } => dash(&mut out, s, center, at, ink),
        }
    }
    if let Some(aim) = aim.filter(|_| !self_area && p.casting != 0) {
        let end = endpoint(origin, aim, range, p.casting == 2);
        let at = project(end);
        if p.geometry.footprints.is_empty() && !p.geometry.known_without_area() {
            // Limited guide: the direction only, for skills not understood.
            if let Some(dir) = unit_vector(center, at) {
                let start = (
                    center.0 + dir.0 * s.arrow_start,
                    center.1 + dir.1 * s.arrow_start,
                );
                arrow(&mut out, s, start, at, ink, true);
            }
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
/// `v` turned by `degrees`, clockwise on screen (y grows downward).
fn turned(v: Point, degrees: f32) -> Point {
    let (s, c) = degrees.to_radians().sin_cos();
    (c * v.0 - s * v.1, s * v.0 + c * v.1)
}
/// HUD hover has no battlefield aim. Show known reach and caster-centered
/// areas only; never orient a shot toward the icon or reuse a previous aim.
pub fn hover_drawing(frame: CameraFrame, p: Preview) -> Drawing {
    let mut out = Drawing::default();
    if !frame.valid() {
        return out;
    }
    let s = style();
    let center = frame.project_unclipped(p.origin.0 as f32 / 1000., p.origin.1 as f32 / 1000.);
    let (sx, sy) = (2048. / frame.extent.0, 2048. / frame.extent.1);
    let self_area = p.self_target || p.casting == 3 || p.geometry.self_area();
    if p.range > 0 && !self_area {
        range_ring(&mut out, s, center, p.range as f32 / 1000. * sx);
    }
    let ink = Ink::of(s, p.ready);
    for footprint in &p.geometry.footprints {
        let centered = footprint.placement == Placement::Caster
            || footprint.placement == Placement::Aim && (p.self_target || p.casting == 3);
        if !centered {
            continue;
        }
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
            .all(|s| rgb(s.color) == st.gray || rgb(s.color) == st.ink));
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
    fn spearhead_follows_the_design_geometry() {
        let st = style();
        let ink = Ink::of(st, true);
        let mut d = Drawing::default();
        // A 200 px dash along +x: 37 px head, wings 0.48 of it.
        arrow(&mut d, st, (0., 0.), (200., 0.), ink, false);
        let (neck, wing) = (200. - st.dash_head, st.dash_head * st.dash_wing);
        // The upper facet's bright edge runs from its shoulder to the tip.
        assert!(d.strokes.iter().any(|s| rgb(s.color) == st.white
            && near(s.a, (neck - 2., -wing))
            && near(s.b, (200., 0.))));
        // The white tip ends exactly at the end point.
        assert!(d
            .strokes
            .iter()
            .any(|s| s.z == Z_TOP && near(s.a, (191., 0.)) && near(s.b, (200., 0.))));
        // Skillshots use the larger head; short arrows shrink it.
        let mut d = Drawing::default();
        arrow(&mut d, st, (0., 0.), (50., 0.), ink, true);
        let h = 50. * 0.42;
        assert!(d
            .strokes
            .iter()
            .any(|s| near(s.a, (50. - h - 2., -h * st.skillshot_wing))));
        // A very short dash is a grip, not a reversed arrow.
        let mut d = Drawing::default();
        dash(&mut d, st, (0., 0.), (10., 0.), ink);
        let top = (10., -st.dash_short_grip);
        assert!(d.strokes.iter().any(|s| near(s.a, top)));
        assert!(!d
            .strokes
            .iter()
            .any(|s| s.z == Z_TOP && rgb(s.color) == st.white && s.width == st.arrow_tip_width));
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
        // Drawn: one white tip per blade.
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
                range: 130000,
                ready: true,
                geometry,
                target: None,
            },
        );
        let st = style();
        let tips = d
            .strokes
            .iter()
            .filter(|s| s.z == Z_TOP && s.width == st.arrow_tip_width && rgb(s.color) == st.white)
            .count();
        assert_eq!(tips, 4);
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
            assert!(d.strokes.len() < MAX_STROKES);
            let edge = 540. + 10. * 2048. / extent;
            assert!(d
                .strokes
                .iter()
                .any(|s| rgb(s.color) == style().white && (s.a.1 - edge).abs() < 0.01));
            assert!(d
                .strokes
                .iter()
                .all(|s| [s.a.0, s.a.1, s.b.0, s.b.1].into_iter().all(f32::is_finite)));
        }
    }
}
