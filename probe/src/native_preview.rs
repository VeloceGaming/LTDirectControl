//! Borrowed effect-family reader for the fingerprinted 0.6.3 build.
//! No native code is invoked and no native address leaves this callback.
use crate::skill_preview::{Geometry, Placement, Shape};

const COMBINE: usize = 0x1adaf00;
const RANGE: usize = 0x16974f0;
const LINEAR: usize = 0x18bf7a0;
const WHIP_LINE: usize = 0x1611090;
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
    payload(base, object, apply, size, out, nodes, depth);
}
unsafe fn payload(
    base: usize,
    object: usize,
    apply: Option<usize>,
    size: usize,
    out: &mut Geometry,
    nodes: &mut usize,
    depth: usize,
) {
    match (apply, size) {
        (Some(COMBINE), 24) => {
            out.family("native Combine");
            let (cap, ptr, len) = (word(object), word(object + 8), word(object + 16));
            if len > 32 || len > cap || (len > 0 && (ptr == 0 || !ptr.is_multiple_of(8))) {
                out.issue("invalid native Combine vector");
                return;
            }
            for i in 0..len {
                effect(
                    base,
                    word(ptr + i * 16),
                    word(ptr + i * 16 + 8),
                    out,
                    nodes,
                    depth + 1,
                );
            }
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
            if let (Some(radius), Some(length)) = (
                distance(word(object + 8) as u64),
                distance(word(object + 128) as u64),
            ) {
                out.add(Shape::Corridor { radius, length }, Placement::Caster);
            } else {
                out.issue("invalid native projectile dimensions");
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
    effect(
        base,
        word(entity + offset),
        word(entity + offset + 8),
        &mut result,
        &mut nodes,
        0,
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;
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
        projectile[16] = 75000;
        let mut g = Geometry::default();
        unsafe {
            payload(
                0,
                projectile.as_ptr() as usize,
                Some(LINEAR),
                152,
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
            payload(0, 1, Some(123), 152, &mut g, &mut 0, 0);
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
                &mut g,
                &mut 0,
                0,
            );
        }
        assert!(g
            .issues
            .iter()
            .any(|s| s == "invalid native Combine vector"));
        unsafe {
            effect(0, 1, 1, &mut g, &mut 0, 0);
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
