//! Research builds only (`--features research`; step 4 of the preview
//! plan, stage C): what the mod records to learn skill shapes, never what it
//! draws. Public builds leave all of this out, with the live reader
//! (`preview.rs`) and its per-version tables (`v*/effects.rs`), so a new
//! game version needs none of them found for a release.
//!
//! - the effect-tree inventory (`PREVIEW TREE`, `PREVIEW COVERAGE`), for
//!   decoding new effect types;
//! - the shape table (`crate::shape_record`): every champion's shapes as the
//!   live reader decodes them, and each skill's numbers by name with the
//!   shapes read at the same moment (`SHAPE RECORD`, `SHAPE NUMBERS`);
//! - the numbers log (`PATCH FIELDS`, `crate::patch_fields`);
//! - for the controlled athlete, the live reader's preview where the
//!   shipped table has no entry, and `PREVIEW COMPARE` where the two differ.
use super::skill_numbers::{get, image_end, patch_name, SLOTS, UNLOCK};
use super::*;

/// Where the cast metadata of each skill sits (as in `record_shapes`).
const CASTS: [usize; 3] = [0x4c0, 0x4f8, 0x530];

/// From the combat hook, for every champion of every simulation.
pub(super) unsafe fn observe(shared: &Shared, entity: usize) {
    let _ = catch_unwind(AssertUnwindSafe(|| inventory(shared, entity)));
    let _ = catch_unwind(AssertUnwindSafe(|| record_shapes(shared, entity)));
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(base) = family_base(family::READERS) {
            log(shared, entity, base);
        }
    }));
}
/// The live reader's preview of the controlled athlete's learned skills.
pub(super) unsafe fn live_geometry(
    entity: usize,
    metadata: &[Option<crate::abilities::Descriptor>; 3],
) -> [crate::skill_preview::Geometry; 3] {
    let Some(base) = family_base(family::READERS) else {
        return Default::default();
    };
    std::array::from_fn(|slot| {
        if metadata[slot].is_some() {
            preview::read(base, entity, CASTS[slot])
        } else {
            crate::skill_preview::Geometry::default()
        }
    })
}
/// The live reader beside the shipped table: log, once per skill and match,
/// where their shapes differ (only once the skill's numbers are read).
pub(super) fn compare(
    shared: &Shared,
    owner: super::skill_numbers::Owner,
    champion: &str,
    slot: usize,
    table: &crate::skill_preview::Geometry,
    live: &crate::skill_preview::Geometry,
) {
    type Compared = (Option<super::skill_numbers::Owner>, [bool; 3]);
    thread_local! {
        static COMPARED: std::cell::RefCell<Compared> =
            const { std::cell::RefCell::new((None, [false; 3])) };
    }
    if live.footprints.is_empty() || table.footprints == live.footprints {
        return;
    }
    let first = COMPARED.with(|c| {
        let mut c = c.borrow_mut();
        if c.0 != Some(owner) {
            *c = (Some(owner), [false; 3]);
        }
        !std::mem::replace(&mut c.1[slot], true)
    });
    if first {
        shared.logger.write(&format!(
            "PREVIEW COMPARE champion={champion} slot={} table={:?} live={:?}",
            ["Q", "W", "R"][slot],
            table.footprints,
            live.footprints
        ));
    }
}
/// Every champion's current numbers by name (1.5.0-dev13), stored with the
/// shapes read at the same moment for learning (1.5.0-dev15).
pub(super) unsafe fn log(shared: &Shared, entity: usize, base: usize) {
    let Some(end) = image_end(base) else {
        return;
    };
    let code = |address: usize| (base..end).contains(&address);
    let actor = std::ptr::read_unaligned((entity + 0x5b8) as *const usize);
    let level = std::ptr::read_unaligned((entity + 0x5c0) as *const usize);
    for slot in 0..3 {
        if level < UNLOCK[slot] {
            continue;
        }
        let Some(champion) = crate::shape_record::fields_wanted(actor, slot) else {
            continue;
        };
        let object = std::ptr::read_unaligned((entity + SLOTS[slot]) as *const usize);
        let table = std::ptr::read_unaligned((entity + SLOTS[slot] + 8) as *const usize);
        if object == 0 || !super::tooltips::accessible(table, 0x28, false) {
            // Unlocked by level but not learned yet: try again later.
            if crate::shape_record::first_unlearned(actor, slot) {
                shared.logger.write(&format!(
                    "PATCH FIELDS champion={champion} slot={} waiting=no skill object yet (level {level})",
                    ["Q", "W", "R"][slot]
                ));
            }
            continue;
        }
        crate::shape_record::fields_done(actor, slot);
        let name_fn = std::ptr::read_unaligned((table + SKILL_PATCH_NAME_SLOT) as *const usize);
        let get_fn = std::ptr::read_unaligned((table + SKILL_PATCH_GET_SLOT) as *const usize);
        if !code(name_fn) || !code(get_fn) {
            shared.logger.write(&format!(
                "PATCH FIELDS champion={champion} slot={slot} refused=methods outside the game"
            ));
            continue;
        }
        let Some(kind) = patch_name(name_fn, object) else {
            shared.logger.write(&format!(
                "PATCH FIELDS champion={champion} slot={slot} refused=unreadable patch name"
            ));
            continue;
        };
        let values: Vec<(String, Option<i64>)> = crate::patch_fields::names_for(&kind)
            .into_iter()
            .map(|field| {
                let value = get(get_fn, object, &field);
                (field, value)
            })
            .collect();
        let cast_range = std::ptr::read_unaligned((entity + CASTS[slot] + 0x10) as *const u64);
        // Stage A of step 4: keep the numbers with the shapes the live reader
        // reads now, so each size can be matched to the number it is.
        if crate::shape_record::numbers_wanted(&champion, slot, &shared.logger) {
            let readable = super::preview::dump(base, entity, CASTS[slot], &|a, n| {
                super::tooltips::accessible(a, n, false)
            })
            .is_some();
            let live = if readable {
                super::preview::read(base, entity, CASTS[slot])
            } else {
                crate::skill_preview::Geometry::default()
            };
            crate::shape_record::record_numbers(
                &champion,
                slot,
                &kind,
                &values,
                cast_range,
                &live,
                &shared.logger,
            );
        }
        crate::patch_fields::report(&champion, slot, &kind, cast_range, values, &shared.logger);
    }
}
// 0.70 diagnostic: an inventory of skill effect trees, so the remaining
// effect types can be decoded. The viewed match's champions have their own
// budget (every champion, once per match: the roster names them); background
// matches add new tree shapes only.
const VIEWED_LIMIT: usize = 240;
const BACKGROUND_LIMIT: usize = 200;
const BACKGROUND_PER_SHAPE: usize = 2;
static VIEWED_LINES: AtomicUsize = AtomicUsize::new(0);
static BACKGROUND_LINES: AtomicUsize = AtomicUsize::new(0);
static INVENTORY_SHAPES: std::sync::Mutex<Vec<(u64, usize)>> = std::sync::Mutex::new(Vec::new());
thread_local! {
    static INVENTORIED: std::cell::RefCell<std::collections::HashSet<usize>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
}
/// Coverage lines (one per champion of the viewed match) have their own
/// budget: they are short, and a report needs every match of a session.
const COVERAGE_LIMIT: usize = 600;
static COVERAGE_LINES: AtomicUsize = AtomicUsize::new(0);
unsafe fn inventory(shared: &Shared, entity: usize) {
    let viewed = crate::perf::on_worker_thread();
    let (lines, limit) = if viewed {
        (&VIEWED_LINES, VIEWED_LIMIT)
    } else {
        (&BACKGROUND_LINES, BACKGROUND_LIMIT)
    };
    if lines.load(Ordering::Relaxed) >= limit
        && (!viewed || COVERAGE_LINES.load(Ordering::Relaxed) >= COVERAGE_LIMIT)
    {
        return;
    }
    let fresh = INVENTORIED.with(|seen| {
        let mut seen = seen.borrow_mut();
        if seen.len() >= 4096 {
            seen.clear();
        }
        seen.insert(entity)
    });
    let Some(base) = family_base(family::READERS).filter(|_| fresh) else {
        return;
    };
    let actor = std::ptr::read_unaligned((entity + 0x5b8) as *const usize);
    // What the reader makes of each skill, for tools/audit_previews.py; the
    // PREVIEW ROSTER line before it names the actor's champion.
    if viewed && COVERAGE_LINES.fetch_add(1, Ordering::Relaxed) < COVERAGE_LIMIT {
        let slots = [("Q", 0x4c0), ("W", 0x4f8), ("R", 0x530)].map(|(slot, offset)| {
            let readable = preview::dump(base, entity, offset, &|a, n| {
                super::tooltips::accessible(a, n, false)
            })
            .is_some();
            let summary = if readable {
                preview::read(base, entity, offset).summary()
            } else {
                "absent".into()
            };
            format!("{slot}={summary}")
        });
        shared.logger.write(&format!(
            "PREVIEW COVERAGE actor={actor} {}",
            slots.join(" ")
        ));
    }
    if lines.load(Ordering::Relaxed) >= limit {
        return;
    }
    for (slot, offset) in [("Q", 0x4c0), ("W", 0x4f8), ("R", 0x530)] {
        let Some(tree) = preview::dump(base, entity, offset, &|a, n| {
            super::tooltips::accessible(a, n, false)
        }) else {
            continue;
        };
        if !viewed && !new_background_shape(&tree) {
            continue;
        }
        if lines.fetch_add(1, Ordering::Relaxed) < limit {
            shared.logger.write(&format!(
                "PREVIEW TREE viewed={viewed} actor={actor} slot={slot} {tree}"
            ));
        }
    }
}
/// Every champion's decoded Q/W/R, once per champion, for the shape table
/// (`crate::shape_record`). Only skills whose effect tree is readable are
/// decoded, as for the coverage lines above.
unsafe fn record_shapes(shared: &Shared, entity: usize) {
    let Some(base) = family_base(family::READERS) else {
        return;
    };
    let actor = std::ptr::read_unaligned((entity + 0x5b8) as *const usize);
    let Some(name) = crate::shape_record::wanted(actor) else {
        return;
    };
    let offsets = [0x4c0, 0x4f8, 0x530];
    let skills = offsets.map(|offset| {
        let readable = preview::dump(base, entity, offset, &|a, n| {
            super::tooltips::accessible(a, n, false)
        })
        .is_some();
        if readable {
            preview::read(base, entity, offset)
        } else {
            let mut absent = crate::skill_preview::Geometry::default();
            absent.issue("effect tree not readable");
            absent
        }
    });
    // How each skill is cast, for the viewer (the same fields as
    // `read_effect_metadata`; at play they stay live).
    let casts = offsets.map(|offset| {
        let data = entity + offset;
        crate::shape_record::Cast {
            casting: std::ptr::read_unaligned((data + 0x30) as *const u32),
            target: std::ptr::read_unaligned((data + 0x28) as *const u32),
            range: std::ptr::read_unaligned((data + 0x10) as *const u64),
            growth: std::ptr::read_unaligned((data + 0x18) as *const u64),
        }
    });
    crate::shape_record::record(name, skills, casts, &shared.logger);
}
/// Background trees: each shape (the tree without payload words) twice.
fn new_background_shape(tree: &str) -> bool {
    let mut shape = String::new();
    let mut inside = false;
    for c in tree.chars() {
        match c {
            '[' => inside = true,
            ']' => inside = false,
            _ if !inside => shape.push(c),
            _ => {}
        }
    }
    let hash = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        shape.hash(&mut h);
        h.finish()
    };
    INVENTORY_SHAPES.lock().is_ok_and(|mut shapes| {
        match shapes.iter_mut().find(|(h, _)| *h == hash) {
            Some((_, n)) if *n >= BACKGROUND_PER_SHAPE => false,
            Some((_, n)) => {
                *n += 1;
                true
            }
            None => {
                shapes.push((hash, 1));
                true
            }
        }
    })
}
