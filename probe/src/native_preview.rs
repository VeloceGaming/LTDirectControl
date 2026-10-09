//! Borrowed effect-family reader for the fingerprinted 0.6.3 build.
//! No native code is invoked and no native address leaves this callback.
use crate::skill_preview::{Geometry, Placement, Shape};

const COMBINE: usize = 0x1adaf00;
const RANGE: usize = 0x16974f0;
const LINEAR: usize = 0x18bf7a0;
const WHIP_LINE: usize = 0x1611090;
// 0.71, identified from the 0.70 effect-tree inventory (layouts checked
// against every logged instance; see docs/investigation-preview-selection.md).
const DELAYED: usize = 0x1adb160;
const RANGE_PERIOD: usize = 0x14294c0;
const RUSH: usize = 0x1698550;
const MOVE: usize = 0x1adb2f0;
// 0.73, named by pairing logged trees with Workshop and data-driven champion
// JSON (field values checked against those files).
const RANGE_PROJECTILE: usize = 0x1489140;
const PARABOLIC: usize = 0x1612510;
const RUSH_TIME: usize = 0x15b28f0;
const SWITCH_BY_BUFF: usize = 0x18bf2d0;
const MOVE_TO_TARGET: usize = 0x1af4470;
// 0.75.2, hand-written base-game skills: fields matched word for word to
// the champion's `champion_info` entry (ranges after patch adjustment).
const ICE_MAGE_ULT: usize = 0x160fd80;
const BARD_ULT: usize = 0x18c1cc0;
const EXORCIST_ULT: usize = 0x18bec80;
const DANCER_ULT: usize = 0x1aebb90;
/// The caster's current stack (stat block `stack`, entity +0x648, just
/// before its position at +0x658): Dancer R's extra blades. Set by `read`.
const ENTITY_STACK: usize = 0x648;
thread_local! {
    static CASTER_STACK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
/// Recognised effects with no area of their own (buffs, single-target
/// shots, visuals, damage on units already hit): named, not reported.
const NO_AREA: &[(usize, usize, &str)] = &[
    (0x1279800, 288, "AddBuff"),
    (0x1281860, 296, "AddCasterBuff"),
    (0x18c1480, 24, "Sfx"),
    (0x18e40b0, 24, "TargetSfx"),
    (0x18e2630, 48, "ViewEffect"),
    (0x18c6c20, 24, "CasterViewEffect"),
    (0x19de6f0, 32, "CasterAnimation"),
    (0x18c76c0, 72, "Attack"),
    (0x18d36b0, 64, "ApAttack"),
    (0x1990300, 64, "Heal"),
    (0x1698810, 16, "Knockback"),
    (0x19df350, 8, "Fear"),
    (0x14dfcf0, 8, "BlockAttack"),
    (0x14f8d10, 8, "BlockMoveSkill"),
    (0x14e0490, 72, "TargetProjectile"),
    (0x1ad9600, 48, "Bleed"),
    (0x1a1c090, 96, "Gunner Q shot"),
    (0x1285690, 16, "caster wrapper"),
    (0x1278940, 40, "Gunner R sub-shots"),
    (0x18d0a10, 24, "Bard Q buff"),
    (0x1426d40, 16, "Exorcist W buff"),
    // Cavalry Knight R: a speed road from the cast point to the knight,
    // shaped by where she moves afterwards (user, 0.75.2).
    (0x127d060, 344, "Cavalry Knight R buff"),
    (0x19de840, 112, "Cavalry Knight R road"),
];
/// A champion's body radius (game setting `champion_radius`, 10000): the
/// sweep of a timed dash, whose own hit width is not in its payload.
const CHAMPION_RADIUS: f32 = 10.;
const RDATA_TABLE_BOUNDS: std::ops::RangeInclusive<usize> = 0x39ca000..=0x4fe8652;

fn distance(value: u64) -> Option<f32> {
    (value > 0 && value <= 1_000_000).then_some(value as f32 / 1000.)
}
fn coordinate(value: u64) -> Option<f32> {
    (value <= 10_000_000).then_some(value as f32 / 1000.)
}
/// Current collision consumer, not just the older named enum layout, establishes
/// tag order and the DirDot cosine threshold (its old field name is `range`).
fn shape(words: [u64; 6]) -> Option<Shape> {
    Some(match words[0] {
        0 => Shape::Circle {
            radius: distance(words[1])?,
        },
        1 => Shape::Segment {
            radius: distance(words[1])?,
            from: (coordinate(words[2])?, coordinate(words[3])?),
            to: (coordinate(words[4])?, coordinate(words[5])?),
        },
        2 => Shape::Rectangle {
            width: distance(words[1])?,
            height: distance(words[2])?,
        },
        3 => {
            let cosine = words[2] as i64;
            if !(-1000..=1000).contains(&cosine) {
                return None;
            }
            Shape::Cone {
                radius: distance(words[1])?,
                cosine: cosine as f32 / 1000.,
            }
        }
        _ => return None,
    })
}

unsafe fn word(address: usize) -> usize {
    std::ptr::read_unaligned(address as *const usize)
}

/// Caller proves that `arc` is a borrowed Arc<dyn EffectType> from the selected
/// live actor. Only recognized game-owned tables authorize payload traversal.
unsafe fn effect(
    base: usize,
    arc: usize,
    table: usize,
    inherited: Placement,
    out: &mut Geometry,
    nodes: &mut usize,
    depth: usize,
) {
    if depth > 8 || *nodes >= 48 {
        out.issue("native tree limit");
        return;
    }
    *nodes += 1;
    // Exact executable's read-only data section. Foreign DLL effects are opaque.
    let Some(rva) = table.checked_sub(base) else {
        out.issue("foreign effect table");
        return;
    };
    if !RDATA_TABLE_BOUNDS.contains(&rva) || !table.is_multiple_of(8) || arc == 0 {
        out.issue("unrecognized effect table");
        return;
    }
    let size = word(table + 8);
    let align = word(table + 16);
    let apply = word(table + 0x20).checked_sub(base);
    if align != 8 {
        out.issue("unrecognized effect alignment");
        return;
    }
    let object = arc + 16; // Current Combine apply computes this Arc payload offset.
    payload(base, object, apply, size, inherited, out, nodes, depth);
}
/// (payload, apply rva, size) of a game-owned effect, without recording
/// anything; None where `effect` would refuse it.
unsafe fn peek(base: usize, arc: usize, table: usize) -> Option<(usize, usize, usize)> {
    let rva = table.checked_sub(base)?;
    (RDATA_TABLE_BOUNDS.contains(&rva)
        && table.is_multiple_of(8)
        && arc != 0
        && word(table + 16) == 8)
        .then(|| {
            Some((
                arc + 16,
                word(table + 0x20).checked_sub(base)?,
                word(table + 8),
            ))
        })
        .flatten()
}
/// A timed dash among a Combine's direct effects, directly or inside a
/// Delayed: (tick it ends, length).
unsafe fn timed_dash(base: usize, arc: usize, table: usize) -> Option<(usize, f32)> {
    let dash = |object: usize| {
        let ticks = word(object + 32);
        distance((word(object + 24) as u64).saturating_mul(ticks as u64)).map(|l| (ticks, l))
    };
    match peek(base, arc, table)? {
        (object, RUSH_TIME, 56) => dash(object),
        (object, DELAYED, 32) => {
            let (cap, ptr, len) = (word(object), word(object + 8), word(object + 16));
            if len > 32 || len > cap || (len > 0 && (ptr == 0 || !ptr.is_multiple_of(8))) {
                return None;
            }
            let delay = word(object + 24);
            (0..len).find_map(
                |i| match peek(base, word(ptr + i * 16), word(ptr + i * 16 + 8))? {
                    (child, RUSH_TIME, 56) => {
                        dash(child).map(|(t, l)| (delay.saturating_add(t), l))
                    }
                    _ => None,
                },
            )
        }
        _ => None,
    }
}
/// Where a footprint lands when its effect runs after the caster dashed
/// `length` toward the aim: caster-relative places move to the dash end.
fn after_dash(p: Placement, length: f32) -> Placement {
    match p {
        Placement::Caster => Placement::End { length },
        Placement::End { length: l } => Placement::End { length: length + l },
        // Dropped at the caster's feet (Candygel R's pools: range 1).
        Placement::Landing { length: l } if l < 2. => Placement::End { length },
        other => other,
    }
}
/// A Combine's effects. Effects delayed until a timed dash among them has
/// finished happen where the dash ends (Candygel R: a pool at each end).
unsafe fn combine(
    base: usize,
    at: usize,
    inherited: Placement,
    out: &mut Geometry,
    nodes: &mut usize,
    depth: usize,
) {
    let (cap, ptr, len) = (word(at), word(at + 8), word(at + 16));
    if len > 32 || len > cap || (len > 0 && (ptr == 0 || !ptr.is_multiple_of(8))) {
        out.issue("invalid native effect vector");
        return;
    }
    let items: Vec<(usize, usize)> = (0..len)
        .map(|i| (word(ptr + i * 16), word(ptr + i * 16 + 8)))
        .collect();
    let dash = items
        .iter()
        .find_map(|&(arc, table)| timed_dash(base, arc, table));
    for &(arc, table) in &items {
        let delay = match peek(base, arc, table) {
            Some((object, DELAYED, 32)) => Some(word(object + 24)),
            _ => None,
        };
        match (dash, delay) {
            (Some((end, length)), Some(delay)) if delay >= end => {
                let mut moved = Geometry::default();
                effect(base, arc, table, inherited, &mut moved, nodes, depth + 1);
                for f in moved.footprints {
                    out.add(f.shape, after_dash(f.placement, length));
                }
                for issue in moved.issues {
                    out.issue(issue);
                }
                for family in moved.families {
                    out.family(family);
                }
            }
            _ => effect(base, arc, table, inherited, out, nodes, depth + 1),
        }
    }
}
/// The effects of a native `Vec<Arc<dyn EffectType>>` at `at` (cap, ptr, len).
unsafe fn children(
    base: usize,
    at: usize,
    inherited: Placement,
    out: &mut Geometry,
    nodes: &mut usize,
    depth: usize,
) {
    let (cap, ptr, len) = (word(at), word(at + 8), word(at + 16));
    if len > 32 || len > cap || (len > 0 && (ptr == 0 || !ptr.is_multiple_of(8))) {
        out.issue("invalid native effect vector");
        return;
    }
    for i in 0..len {
        effect(
            base,
            word(ptr + i * 16),
            word(ptr + i * 16 + 8),
            inherited,
            out,
            nodes,
            depth + 1,
        );
    }
}
#[allow(clippy::too_many_arguments)]
unsafe fn payload(
    base: usize,
    object: usize,
    apply: Option<usize>,
    size: usize,
    inherited: Placement,
    out: &mut Geometry,
    nodes: &mut usize,
    depth: usize,
) {
    match (apply, size) {
        (Some(COMBINE), 24) => {
            out.family("native Combine");
            combine(base, object, inherited, out, nodes, depth);
        }
        (Some(DELAYED), 32) => {
            // Delayed { effects, delay }: the delayed effects keep the placement.
            out.family("native Delayed");
            children(base, object, inherited, out, nodes, depth);
        }
        (Some(RANGE_PERIOD), 152) => {
            // A lingering area: the same shape words as a projectile.
            out.family("native RangePeriodProjectile");
            let words = std::array::from_fn(|i| word(object + i * 8) as u64);
            if let Some(shape) = shape(words) {
                out.add(shape, inherited);
            } else {
                out.issue("native period area shape unresolved");
            }
        }
        (Some(RUSH), 56) => {
            // Dash toward the aim that stops at the first unit it hits:
            // { applied effects, radius, _, speed }. Its length is the cast range.
            out.family("native Rush");
            match distance(word(object + 24) as u64) {
                Some(radius) => out.add(Shape::Corridor { radius, length: 0. }, Placement::Caster),
                None => out.issue("invalid native dash width"),
            }
        }
        (Some(MOVE), 40) => {
            // Movement to the aimed point: { effects, speed, maximum distance }.
            out.family("native MoveTo");
            out.add(Shape::Movement { blink: false }, Placement::Aim);
        }
        (Some(RANGE_PROJECTILE), 120) => {
            // An area at its placement after a delay: { shape (words 0-5),
            // name, hit effects (words 9-11), delay, apply, target }. The hit
            // effects apply to units already hit, so they are not followed.
            out.family("native RangeProjectile");
            let words = std::array::from_fn(|i| word(object + i * 8) as u64);
            match shape(words) {
                Some(shape) => out.add(shape, inherited),
                None => out.issue("native range projectile shape unresolved"),
            }
        }
        (Some(PARABOLIC), 168) => {
            // A lobbed shot landing at its placement: { shape (words 0-5),
            // name, range-effect name, applied effects, landing effects
            // (words 15-17), travel time, range, target }. It passes over
            // units on the way, so only the landing area is drawn.
            out.family("native ParabolicProjectile");
            let words = std::array::from_fn(|i| word(object + i * 8) as u64);
            match shape(words) {
                Some(shape) => out.add(shape, inherited),
                None => out.issue("native parabolic shape unresolved"),
            }
            children(base, object + 120, inherited, out, nodes, depth);
        }
        (Some(RUSH_TIME), 56) => {
            // A dash for a fixed time: { applied effects, speed, ticks, range,
            // flags }. Its length is speed x ticks. With effects on units it
            // hits it sweeps the body (a corridor); without any it hits
            // nothing (Candygel R's slide) and is drawn as movement.
            out.family("native RushTime");
            let length = (word(object + 24) as u64).saturating_mul(word(object + 32) as u64);
            match distance(length) {
                Some(length) if word(object + 16) > 0 => out.add(
                    Shape::Corridor {
                        radius: CHAMPION_RADIUS,
                        length,
                    },
                    Placement::Caster,
                ),
                Some(length) => {
                    out.add(Shape::Movement { blink: false }, Placement::End { length })
                }
                None => out.issue("invalid native timed dash"),
            }
        }
        (Some(SWITCH_BY_BUFF), 56) => {
            // { buff name, effect without the buff, effect with it }. The
            // caster's buffs are not read here: preview the usual branch.
            out.family("native SwitchByBuff");
            effect(
                base,
                word(object + 24),
                word(object + 32),
                inherited,
                out,
                nodes,
                depth + 1,
            );
        }
        (Some(ICE_MAGE_ULT), 88) => {
            // { range, attack, ratio, cosine x1000, half angle (deg),
            // knockback speed/ticks, slow, slow ticks, silence, sweep }: a
            // cone swept from the caster toward the aim.
            out.family("native IceMage ult");
            let cosine = word(object + 24) as i64;
            match distance(word(object) as u64).filter(|_| (-1000..=1000).contains(&cosine)) {
                Some(radius) => out.add(
                    Shape::Cone {
                        radius,
                        cosine: cosine as f32 / 1000.,
                    },
                    Placement::Caster,
                ),
                None => out.issue("invalid native ice mage cone"),
            }
        }
        (Some(BARD_ULT), 80) => {
            // { range, attack/speed/power boosts, ratio, armour and resist
            // reduction, channel ticks, period }: an aura around the bard.
            out.family("native Bard ult");
            match distance(word(object) as u64) {
                Some(radius) => out.add(Shape::Circle { radius }, Placement::Caster),
                None => out.issue("invalid native bard aura"),
            }
        }
        (Some(EXORCIST_ULT), 48) => {
            // { attack, ratio, damage per buff, its ratio, range, delay }: an
            // area of `range` where the cast lands (its cast range is separate).
            out.family("native Exorcist ult");
            match distance(word(object + 32) as u64) {
                Some(radius) => out.add(Shape::Circle { radius }, inherited),
                None => out.issue("invalid native exorcist area"),
            }
        }
        (Some(DANCER_ULT), 80) => {
            // { attack, ratio, return attack, its ratio, range, hit radius,
            // speed, base count, max count, _ }: blades fanned from the
            // caster; count = min(max, base + the caster's stack).
            out.family("native Dancer ult");
            let count = (word(object + 56) + CASTER_STACK.with(|c| c.get())).min(word(object + 64));
            match (
                distance(word(object + 32) as u64),
                distance(word(object + 40) as u64),
            ) {
                (Some(length), Some(radius)) if (1..=16).contains(&count) => out.add(
                    Shape::Fan {
                        count,
                        radius,
                        length,
                    },
                    Placement::Caster,
                ),
                _ => out.issue("invalid native dancer fan"),
            }
        }
        (Some(MOVE_TO_TARGET), 40) => {
            // Movement onto the target unit: { applied effects, speed, _ }.
            out.family("native MoveToTarget");
            out.add(Shape::Movement { blink: false }, Placement::Aim);
        }
        (Some(RANGE), 96) => {
            out.family("native RangeEffect");
            let placement = match word(object) {
                0 => Placement::Caster,
                1 => {
                    let offset = word(object + 8);
                    if offset > 1_000_000 {
                        out.issue("invalid native forward offset");
                        return;
                    }
                    Placement::Forward {
                        offset: offset as f32 / 1000.,
                    }
                }
                _ => {
                    out.issue("native range placement unresolved");
                    return;
                }
            };
            let words = std::array::from_fn(|i| word(object + 16 + i * 8) as u64);
            if let Some(shape) = shape(words) {
                if matches!(shape, Shape::Cone { .. }) && placement == Placement::Caster {
                    out.issue("native DirDot facing unresolved at caster center");
                } else {
                    out.add(shape, placement);
                }
            } else {
                out.issue("native range shape unresolved");
            }
            // Its children apply to already-hit units; their geometry must not
            // be moved to the original cursor/caster by flattening the tree.
        }
        (Some(LINEAR), 152) => {
            out.family("native LinearProjectile");
            let tag = word(object);
            if tag != 0 {
                out.issue("noncircle projectile sweep unresolved");
                return;
            }
            let radius = word(object + 8);
            match distance(word(object + 128) as u64).filter(|_| radius <= 1_000_000) {
                Some(length) => {
                    // Effects on each unit hit on the way (words 9-11). Without
                    // any, nothing stops or is hit in flight (Bomber Q/W, Poison
                    // Dart Hunter Q): no corridor, and the end effects land
                    // where the cast is aimed. Otherwise the path is a corridor
                    // (radius 0 is still drawn as a line) and the end effects
                    // happen at its end.
                    let hits_in_flight = word(object + 88) > 0;
                    if hits_in_flight {
                        out.add(
                            Shape::Corridor {
                                radius: radius as f32 / 1000.,
                                length,
                            },
                            Placement::Caster,
                        );
                    }
                    let end = if hits_in_flight {
                        Placement::End { length }
                    } else {
                        Placement::Landing { length }
                    };
                    // End effects (words 12-14).
                    children(base, object + 96, end, out, nodes, depth);
                }
                None => out.issue("invalid native projectile dimensions"),
            }
        }
        (Some(WHIP_LINE), 88) => {
            // Dedicated effect, identified by implementation rather than champion
            // name. Constructor and launch consumer copy these live parameters.
            out.family("native WhipMasterUlt line");
            if let (Some(length), Some(radius)) = (
                distance(word(object + 24) as u64),
                distance(word(object + 32) as u64),
            ) {
                out.add(Shape::Corridor { radius, length }, Placement::Caster);
            } else {
                out.issue("invalid native channel line dimensions");
            }
        }
        (Some(apply), size) if NO_AREA.iter().any(|(a, s, _)| *a == apply && *s == size) => {
            let name = NO_AREA
                .iter()
                .find(|(a, _, _)| *a == apply)
                .map_or("", |n| n.2);
            out.family(format!("native {name}"));
        }
        _ => {
            let label = apply.map_or_else(
                || "foreign apply".to_owned(),
                |r| format!("apply={r:x} size={size}"),
            );
            out.family(format!("opaque {label}"));
            out.issue(format!("unsupported native {label}"));
        }
    }
}

/// Used only after native_adapter's owned_entity/worker/build checks. The cached
/// Option<Effect> has an Arc in its first two words; no object is constructed.
pub unsafe fn read(base: usize, entity: usize, offset: usize) -> Geometry {
    let mut result = Geometry::default();
    let mut nodes = 0;
    CASTER_STACK.with(|c| c.set(word(entity + ENTITY_STACK).min(64)));
    effect(
        base,
        word(entity + offset),
        word(entity + offset + 8),
        Placement::Aim,
        &mut result,
        &mut nodes,
        0,
    );
    result
}

/// 0.70 diagnostic inventory: one skill's effect tree as text, with every
/// node's apply function, size and raw payload words, so unknown effect
/// layouts can be matched against known values offline. A child is followed
/// only where a game-owned effect table sits beside its pointer (an
/// `Arc<dyn EffectType>`), and only into memory `readable` confirms.
pub unsafe fn dump(
    base: usize,
    entity: usize,
    offset: usize,
    readable: &dyn Fn(usize, usize) -> bool,
) -> Option<String> {
    let mut out = String::new();
    let mut nodes = 0;
    let arc = word(entity + offset);
    let table = word(entity + offset + 8);
    effect_table(base, table)?;
    dump_node(base, arc, table, readable, &mut out, &mut nodes, 0);
    Some(out)
}
/// (size, apply rva) of a plausible game-owned effect vtable.
unsafe fn effect_table(base: usize, table: usize) -> Option<(usize, usize)> {
    let rva = table.checked_sub(base)?;
    if !RDATA_TABLE_BOUNDS.contains(&rva) || !table.is_multiple_of(8) {
        return None;
    }
    let size = word(table + 8);
    let align = word(table + 16);
    let apply = word(table + 0x20).checked_sub(base)?;
    (align == 8 && size <= 1024 && size.is_multiple_of(8) && (0x1000..0x39ca000).contains(&apply))
        .then_some((size, apply))
}
unsafe fn dump_node(
    base: usize,
    arc: usize,
    table: usize,
    readable: &dyn Fn(usize, usize) -> bool,
    out: &mut String,
    nodes: &mut usize,
    depth: usize,
) {
    use std::fmt::Write;
    let Some((size, apply)) = effect_table(base, table) else {
        out.push('?');
        return;
    };
    if depth > 6 || *nodes >= 64 || arc == 0 || !readable(arc, 16 + size) {
        let _ = write!(out, "<{apply:x}/{size} unread>");
        return;
    }
    *nodes += 1;
    let object = arc + 16;
    let words: Vec<usize> = (0..size / 8).map(|i| word(object + i * 8)).collect();
    let _ = write!(out, "<{apply:x}/{size} [");
    for (i, w) in words.iter().enumerate() {
        let _ = write!(out, "{}{w:x}", if i > 0 { " " } else { "" });
    }
    out.push(']');
    // Direct children: (arc, table) word pairs.
    for i in 0..words.len().saturating_sub(1) {
        if effect_table(base, words[i + 1]).is_some() && words[i] != 0 {
            let _ = write!(out, " @{}:", i * 8);
            dump_node(
                base,
                words[i],
                words[i + 1],
                readable,
                out,
                nodes,
                depth + 1,
            );
        }
    }
    // Vec<Arc<dyn EffectType>> children: (capacity, pointer, length).
    for i in 0..words.len().saturating_sub(2) {
        let (cap, ptr, len) = (words[i], words[i + 1], words[i + 2]);
        if len == 0 || len > 32 || len > cap || cap > 64 || !ptr.is_multiple_of(8) {
            continue;
        }
        if !readable(ptr, len * 16) || effect_table(base, word(ptr + 8)).is_none() {
            continue;
        }
        let _ = write!(out, " @{}*{len}:", i * 8);
        for e in 0..len {
            dump_node(
                base,
                word(ptr + e * 16),
                word(ptr + e * 16 + 8),
                readable,
                out,
                nodes,
                depth + 1,
            );
        }
    }
    out.push('>');
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inventory_identified_types_decode_from_logged_payloads() {
        let decode = |apply: usize, size: usize, words: &[usize], inherited: Placement| {
            let mut g = Geometry::default();
            unsafe {
                payload(
                    0,
                    words.as_ptr() as usize,
                    Some(apply),
                    size,
                    inherited,
                    &mut g,
                    &mut 0,
                    0,
                )
            };
            g
        };
        // Ghost W dash: {applied, radius 5000, _, speed 12000, flags}.
        let g = decode(RUSH, 56, &[0, 8, 0, 0x1388, 0, 0x2ee0, 7], Placement::Aim);
        assert_eq!(
            g.footprints[0].shape,
            Shape::Corridor {
                radius: 5.,
                length: 0.
            }
        );
        assert_eq!(g.footprints[0].placement, Placement::Caster);
        // Ghost Q movement: {effects, speed 4500, maximum 80000}.
        let g = decode(MOVE, 40, &[0, 8, 0, 0x1194, 0x13880], Placement::Aim);
        assert_eq!(g.footprints[0].shape, Shape::Movement { blink: false });
        // Poison Dart Hunter's poison area, radius 48000, keeps its placement.
        let mut area = [0usize; 19];
        area[1] = 0xbb80;
        let g = decode(RANGE_PERIOD, 152, &area, Placement::End { length: 50. });
        assert_eq!(g.footprints[0].shape, Shape::Circle { radius: 48. });
        assert_eq!(g.footprints[0].placement, Placement::End { length: 50. });
        // A shot that hits units on the way: radius 0 (a line), length 50000.
        let mut shot = [0usize; 19];
        shot[9] = 1; // one applied effect (capacity, pointer, length)
        shot[10] = 8;
        shot[11] = 1;
        shot[13] = 8; // empty end-effects vector
        shot[16] = 0xc350;
        let g = decode(LINEAR, 152, &shot, Placement::Aim);
        assert_eq!(
            g.footprints[0].shape,
            Shape::Corridor {
                radius: 0.,
                length: 50.
            }
        );
        // Poison Dart Hunter's dart hits nothing in flight (no applied
        // effects): no corridor; its splash lands at the aim (not followed here).
        let mut dart = shot;
        (dart[9], dart[11]) = (0, 0);
        let g = decode(LINEAR, 152, &dart, Placement::Aim);
        assert!(g.footprints.is_empty() && g.issues.is_empty());
        // Bomber W (base, logged): a 40000 circle after 61 ticks at the aim.
        let bomb = [
            0, 0x9c40, 0x64, 0x1770, 0x10, 0, 0x13, 0, 0x13, 2, 0, 2, 0x3d, 0x3c, 6,
        ];
        let g = decode(RANGE_PROJECTILE, 120, &bomb, Placement::Aim);
        assert_eq!(g.footprints[0].shape, Shape::Circle { radius: 40. });
        assert_eq!(g.footprints[0].placement, Placement::Aim);
        // Alchemist Q (JSON: Circle 9000, travel 28, range 70000), no
        // landing effects here.
        let mut flask = [0usize; 21];
        flask[1] = 0x2328;
        flask[13] = 8; // empty applied effects
        flask[16] = 8; // empty landing effects
        flask[18] = 0x1c;
        flask[19] = 0x11170;
        let g = decode(PARABOLIC, 168, &flask, Placement::Aim);
        assert_eq!(g.footprints[0].shape, Shape::Circle { radius: 9. });
        assert_eq!(g.footprints[0].placement, Placement::Aim);
        // Harpy R (JSON: speed 2500, 70 ticks, four effects on units hit):
        // a 175-unit body-wide dash.
        let g = decode(
            RUSH_TIME,
            56,
            &[4, 8, 4, 0x9c4, 0x46, 0x2710, 7],
            Placement::Aim,
        );
        assert_eq!(
            g.footprints[0].shape,
            Shape::Corridor {
                radius: CHAMPION_RADIUS,
                length: 175.
            }
        );
        assert_eq!(g.footprints[0].placement, Placement::Caster);
        // Candygel W (JSON: MoveToTarget speed 4000).
        let g = decode(MOVE_TO_TARGET, 40, &[0, 8, 0, 0xfa0, 0], Placement::Aim);
        assert_eq!(g.footprints[0].shape, Shape::Movement { blink: false });
        // Ice Mage R (logged; champion_info range 70000, half angle 45°).
        let ult = [
            0x11170, 0x50, 0x32, 0x2c3, 0x2d, 0xbb8, 0xf, 0x3c, 0xb4, 0x78, 0x14,
        ];
        let g = decode(ICE_MAGE_ULT, 88, &ult, Placement::Aim);
        assert_eq!(
            g.footprints[0].shape,
            Shape::Cone {
                radius: 70.,
                cosine: 0.707
            }
        );
        assert_eq!(g.footprints[0].placement, Placement::Caster);
        // Bard R (logged; range 100000, 99000 after patch): an aura.
        let ult = [0x182b8, 0x64, 0x6a, 0x33, 0x96, 0x32, 0x1e, 0x1e, 0xf0, 3];
        let g = decode(BARD_ULT, 80, &ult, Placement::Aim);
        assert_eq!(g.footprints[0].shape, Shape::Circle { radius: 99. });
        assert_eq!(g.footprints[0].placement, Placement::Caster);
        // Exorcist R (logged; range 60000, 61000 after patch) at the cast.
        let ult = [0x96, 0x4c, 0x64, 0xa, 0xee48, 0x3c];
        let g = decode(EXORCIST_ULT, 48, &ult, Placement::Aim);
        assert_eq!(g.footprints[0].shape, Shape::Circle { radius: 61. });
        assert_eq!(g.footprints[0].placement, Placement::Aim);
        // Dancer R (logged; champion_info range 130000 -> 134000, attack
        // range 10000, base 4, max 10): 4 blades without stacks.
        let ult = [0x14, 0x1f, 0x28, 0x32, 0x20b70, 0x2710, 0xe0e, 4, 10, 7];
        let g = decode(DANCER_ULT, 80, &ult, Placement::Aim);
        assert_eq!(
            g.footprints[0].shape,
            Shape::Fan {
                count: 4,
                radius: 10.,
                length: 134.
            }
        );
        // Stacks add blades up to the maximum.
        CASTER_STACK.with(|c| c.set(9));
        let g = decode(DANCER_ULT, 80, &ult, Placement::Aim);
        assert!(matches!(
            g.footprints[0].shape,
            Shape::Fan { count: 10, .. }
        ));
        CASTER_STACK.with(|c| c.set(0));
        // Recognised effects without an area are named, not reported.
        let g = decode(0x1281860, 296, &[0; 37], Placement::Aim);
        assert!(g.footprints.is_empty() && g.issues.is_empty());
        assert_eq!(g.families, ["native AddCasterBuff"]);
        // An empty Delayed wrapper is traversed without inventing a footprint.
        let g = decode(DELAYED, 32, &[0, 8, 0, 0x24], Placement::Aim);
        assert!(g.footprints.is_empty() && g.issues.is_empty());
        assert_eq!(g.families, ["native Delayed"]);
    }
    #[test]
    fn effects_after_a_timed_dash_happen_where_it_ends() {
        // Candygel R's shape: Delayed 10 { slide 2700 x 44, hits nothing },
        // Delayed 10 { drop: range-1 projectile ending in a 42-unit pool },
        // Delayed 54 { the same drop }.
        // Fake vtables [_, size, align, _, apply]: Delayed, RushTime,
        // LinearProjectile, area.
        let mut tables = [0usize; 20];
        let base = tables.as_ptr() as usize - 0x39ca000;
        for (i, (size, apply)) in [
            (32, DELAYED),
            (56, RUSH_TIME),
            (152, LINEAR),
            (152, RANGE_PERIOD),
        ]
        .into_iter()
        .enumerate()
        {
            tables[i * 5 + 1] = size;
            tables[i * 5 + 2] = 8;
            tables[i * 5 + 4] = base + apply;
        }
        let table = |i: usize| tables.as_ptr() as usize + i * 5 * 8;
        // Arc allocations: strong, weak, then the payload.
        let slide = [1usize, 1, 0, 8, 0, 2700, 44, 70000, 7];
        let mut pool = [0usize; 21];
        pool[3] = 42000; // circle radius
        let pool_list = [pool.as_ptr() as usize, table(3)];
        let mut drop = [0usize; 21];
        drop[2 + 10] = 8; // no effects on units hit in flight
        (drop[2 + 12], drop[2 + 13], drop[2 + 14]) = (1, pool_list.as_ptr() as usize, 1);
        drop[2 + 16] = 1; // range 1
        let slide_list = [slide.as_ptr() as usize, table(1)];
        let drop_list = [drop.as_ptr() as usize, table(2)];
        let delayed =
            |list: &[usize; 2], tick: usize| [1usize, 1, 1, list.as_ptr() as usize, 1, tick];
        let start = delayed(&slide_list, 10);
        let first = delayed(&drop_list, 10);
        let last = delayed(&drop_list, 54);
        let effects = [
            start.as_ptr() as usize,
            table(0),
            first.as_ptr() as usize,
            table(0),
            last.as_ptr() as usize,
            table(0),
        ];
        let combine = [3usize, effects.as_ptr() as usize, 3];
        let mut g = Geometry::default();
        unsafe {
            payload(
                base,
                combine.as_ptr() as usize,
                Some(COMBINE),
                24,
                Placement::Aim,
                &mut g,
                &mut 0,
                0,
            )
        };
        let slide_end = Placement::End {
            length: distance(118_800).unwrap(),
        };
        let at_feet = Placement::Landing {
            length: distance(1).unwrap(),
        };
        let circle = Shape::Circle { radius: 42. };
        let found: Vec<_> = g
            .footprints
            .iter()
            .map(|f| (f.shape, f.placement))
            .collect();
        assert_eq!(
            found,
            [
                (Shape::Movement { blink: false }, slide_end),
                (circle, at_feet),
                (circle, slide_end),
            ],
            "{g:?}"
        );
        assert_eq!(
            after_dash(Placement::End { length: 20. }, 100.),
            Placement::End { length: 120. }
        );
        assert_eq!(after_dash(Placement::Aim, 100.), Placement::Aim);
    }
    #[test]
    fn switch_by_buff_previews_the_branch_without_the_buff() {
        // Fake vtables [_, size, align, _, apply] for two RangeEffect-free
        // branches: a RangeProjectile (none) and a MoveTo (buff).
        let mut tables = [0usize; 10];
        let base = tables.as_ptr() as usize - 0x39ca000;
        tables[1] = 120;
        tables[2] = 8;
        tables[4] = base + RANGE_PROJECTILE;
        tables[6] = 40;
        tables[7] = 8;
        tables[9] = base + MOVE;
        let (none_table, buff_table) = (tables.as_ptr() as usize, tables.as_ptr() as usize + 5 * 8);
        // Arc allocations: strong, weak, then the payload.
        let mut none = [0usize; 17];
        none[3] = 0x9c40; // circle radius 40000
        let buff = [1usize, 1, 0, 8, 0, 0x1194, 0];
        let switch = [
            0,
            1,
            0,
            none.as_ptr() as usize,
            none_table,
            buff.as_ptr() as usize,
            buff_table,
        ];
        let mut g = Geometry::default();
        unsafe {
            payload(
                base,
                switch.as_ptr() as usize,
                Some(SWITCH_BY_BUFF),
                56,
                Placement::Aim,
                &mut g,
                &mut 0,
                0,
            )
        };
        assert_eq!(g.footprints.len(), 1);
        assert_eq!(g.footprints[0].shape, Shape::Circle { radius: 40. });
        assert!(g.families.iter().all(|f| f != "native MoveTo"));
    }
    #[test]
    fn inventory_dump_follows_only_effect_pointers_into_readable_memory() {
        // Two fake vtables side by side: [_, size, align, _, apply].
        let mut tables = [0usize; 10];
        let base = tables.as_ptr() as usize - 0x39ca000;
        tables[1] = 32;
        tables[2] = 8;
        tables[4] = base + 0x2000;
        tables[6] = 16;
        tables[7] = 8;
        tables[9] = base + 0x3000;
        let (table_a, table_b) = (tables.as_ptr() as usize, tables.as_ptr() as usize + 5 * 8);
        // Arc allocations: strong, weak, then the payload.
        let child = [1usize, 1, 0x1f4, 0x2a];
        let child_arc = child.as_ptr() as usize;
        let parent = [1usize, 1, 7, child_arc, table_b, 9];
        let slot = [parent.as_ptr() as usize, table_a];
        let entity = slot.as_ptr() as usize - 0x4c0;
        let text = unsafe { dump(base, entity, 0x4c0, &|_, _| true) }.unwrap();
        assert_eq!(
            text,
            format!("<2000/32 [7 {child_arc:x} {table_b:x} 9] @8:<3000/16 [1f4 2a]>>")
        );
        // Nothing is read where memory is not confirmed readable.
        let text = unsafe { dump(base, entity, 0x4c0, &|_, _| false) }.unwrap();
        assert_eq!(text, "<2000/32 unread>");
        // No effect table: no dump at all.
        let empty = [0usize, 0];
        assert!(
            unsafe { dump(base, empty.as_ptr() as usize - 0x4c0, 0x4c0, &|_, _| true) }.is_none()
        );
    }
    #[test]
    fn borrowed_payloads_keep_live_dimensions_and_do_not_traverse_hit_children() {
        let mut bytes = [0usize; 19];
        bytes[0] = 1;
        bytes[1] = 20000; // forward center
        bytes[2] = 0;
        bytes[3] = 40000; // circle radius
        bytes[9] = 1;
        bytes[10] = 1;
        bytes[11] = 1; // intentionally unreadable hit-child vector
        let mut g = Geometry::default();
        unsafe {
            payload(
                0,
                bytes.as_ptr() as usize,
                Some(RANGE),
                96,
                Placement::Aim,
                &mut g,
                &mut 0,
                0,
            );
        }
        assert_eq!(
            g.footprints,
            [crate::skill_preview::Footprint {
                shape: Shape::Circle { radius: 40. },
                placement: Placement::Forward { offset: 20. }
            }]
        );
        let mut projectile = [0usize; 19];
        projectile[1] = 6000;
        (projectile[9], projectile[10], projectile[11]) = (1, 8, 1); // hits in flight
        projectile[16] = 75000;
        let mut g = Geometry::default();
        unsafe {
            payload(
                0,
                projectile.as_ptr() as usize,
                Some(LINEAR),
                152,
                Placement::Aim,
                &mut g,
                &mut 0,
                0,
            );
        }
        assert_eq!(
            g.footprints[0].shape,
            Shape::Corridor {
                radius: 6.,
                length: 75.
            }
        );
        let mut channel = [0usize; 11];
        channel[3] = 130000;
        channel[4] = 3000;
        let mut g = Geometry::default();
        unsafe {
            payload(
                0,
                channel.as_ptr() as usize,
                Some(WHIP_LINE),
                88,
                Placement::Aim,
                &mut g,
                &mut 0,
                0,
            );
        }
        assert_eq!(
            g.footprints[0].shape,
            Shape::Corridor {
                radius: 3.,
                length: 130.
            }
        );
    }
    #[test]
    fn opaque_payloads_and_invalid_vectors_are_not_dereferenced() {
        let mut g = Geometry::default();
        unsafe {
            payload(0, 1, Some(123), 152, Placement::Aim, &mut g, &mut 0, 0);
        }
        assert!(g.footprints.is_empty());
        assert_eq!(g.issues, ["unsupported native apply=7b size=152"]);
        let bad = [1usize, 1, 2];
        unsafe {
            payload(
                0,
                bad.as_ptr() as usize,
                Some(COMBINE),
                24,
                Placement::Aim,
                &mut g,
                &mut 0,
                0,
            );
        }
        assert!(g.issues.iter().any(|s| s == "invalid native effect vector"));
        unsafe {
            effect(0, 1, 1, Placement::Aim, &mut g, &mut 0, 0);
        }
        assert!(g.issues.iter().any(|s| s == "unrecognized effect table"));
    }
    #[test]
    fn native_tags_use_current_collision_dimensions_not_cast_range() {
        assert_eq!(
            shape([0, 10000, 0, 0, 0, 0]),
            Some(Shape::Circle { radius: 10. })
        );
        assert_eq!(
            shape([2, 20000, 40000, 0, 0, 0]),
            Some(Shape::Rectangle {
                width: 20.,
                height: 40.
            })
        );
        assert_eq!(
            shape([3, 40000, 500, 0, 0, 0]),
            Some(Shape::Cone {
                radius: 40.,
                cosine: 0.5
            })
        );
        assert!(shape([3, 40000, 40000, 0, 0, 0]).is_none());
        assert!(shape([0, u64::MAX, 0, 0, 0, 0]).is_none());
        assert_eq!(
            shape([1, 3000, 100000, 200000, 230000, 200000]),
            Some(Shape::Segment {
                radius: 3.,
                from: (100., 200.),
                to: (230., 200.),
            })
        );
    }
}
