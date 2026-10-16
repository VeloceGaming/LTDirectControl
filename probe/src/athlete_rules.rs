//! Athlete stat rules for manually played matches (backlog item 15).
//!
//! What this module holds: the arithmetic of the rules, the stamp that says
//! which athlete the rules apply to, the settings copy the hooks read, and the
//! counters behind the `STATRULE` log lines. It names no game address and no
//! game version; those live in `native_adapter/v*/stat_sites.rs`, and the hook
//! code is `native_adapter/windows/stats.rs`.
//!
//! The rules, only for shots by the controlled athlete:
//! - Collision: the three Accuracy producers yield [`COLLISION_MARKER`]
//!   (5 game units). The collision consumers that see it skip the target's
//!   Dodging term and use the "Collision margin" setting instead: the
//!   shape-B ones (clamped at zero) and the shape-A ones (floored at 5
//!   units by the game; the hook lifts the floor for these shots). The
//!   controlled athlete's own derived Dodging value is 0.
//! - Reaction: the enemy's reaction delay comes from the gap
//!   (caster Accuracy - defender Dodging) in a human band
//!   ([`reaction_gate`]) instead of the game's gate.
//!
//! Hooks run on every simulation thread, so they ask this module two cheap,
//! lock-free questions first ([`active_on`], [`controlled`]) and return to the
//! original path when the answer is no.
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

/// The Accuracy value the producers yield for the controlled athlete's shots:
/// 5 game units (1000 internal units each), the game's own margin floor.
/// Natural values are multiples of 70 (Accuracy x 70), and 5000 is not, so
/// the consumers can tell these shots apart without knowing the caster.
pub const COLLISION_MARKER: u64 = 5_000;
pub const FLOOR_DEFAULT: u32 = 6;
pub const CEILING_DEFAULT: u32 = 14;
/// The testing setting "Collision margin", in game units.
pub const MARGIN_DEFAULT: u32 = 2;
/// Longest pause between two stamps before the rules stop applying. The
/// controlled athlete stamps once per tick; the explicit clears (release,
/// hand-back, setting off) are the primary end, this is the backstop.
const STAMP_FRESH_MS: u64 = 600;
/// Counter slots for the collision consumers (the tables hold 21 per version:
/// 10 of shape B, 11 of shape A).
pub const COLLISION_SLOTS: usize = 32;

// The Win64 saved-register order of the instruction bridge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
#[allow(dead_code)] // the full saved-register frame, including slots no table uses
pub enum Register {
    R15,
    R14,
    R13,
    R12,
    Rbp,
    Rdi,
    Rsi,
    R11,
    R10,
    R9,
    R8,
    Rdx,
    Rcx,
    Rbx,
    Rax,
}

/// Where a gate reads its "inverse Dodging" (max(100 - Dodging, 0)).
#[derive(Clone, Copy, Debug)]
pub enum Source {
    Register(Register),
    /// A dword in the function's frame, at `rbp + offset`.
    Frame(usize),
}

/// How a site finds the actor id of the athlete that cast the shot.
#[derive(Clone, Copy, Debug)]
pub enum Caster {
    /// An id stored in memory at `*reg + offset` (a projectile or an entity).
    Field { reg: Register, offset: usize },
    /// An id that the game shifted left into a register: `reg >> shift`.
    Shifted { reg: Register, shift: u32 },
    /// An id shifted and xor-ed with a per-player seed read at
    /// `*other + offset`: `(reg ^ seed) >> shift`.
    Mixed {
        reg: Register,
        other: Register,
        offset: usize,
        shift: u32,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    /// Replays `imul accuracy, 70` and its store to a frame slot; for the
    /// controlled athlete the slot becomes the marker. The register keeps
    /// the game's value: the next instruction overwrites it in every version.
    /// `trace` is the offset, in the caster's object, of the word the game
    /// switches on next (a projectile's kind, an entity's action state); the
    /// `STATRULE trace` lines report it.
    AccuracyProducer {
        frame: usize,
        caster: Caster,
        trace: usize,
    },
    /// Replays `imul output, Dodging, 150`; for the controlled athlete's own
    /// entity (`entity` holds it) the value becomes 0.
    DodgingProducer { output: Register, entity: Register },
    /// The start of a reaction gate: remembers the inputs when the caster is
    /// the controlled athlete. `age_mixed` says the game also mixed the
    /// projectile's age (bits 40 and up) into the seed.
    Capture {
        accuracy: Register,
        inverse_dodge: Source,
        caster: Caster,
        age_mixed: bool,
    },
    /// The end of the same gate: `output` holds the game's gate, replaced
    /// when the matching capture remembered a controlled caster.
    Complete { capture: usize, output: Register },
    /// A shape-B collision consumer after its replayed `sub output,
    /// [entity + dodging]`. `accuracy` is where the shot's Accuracy value was
    /// added from just before.
    Collision {
        output: Register,
        entity: Register,
        accuracy: (Register, usize),
    },
    /// A shape-A collision consumer's floor: replays `cmp output, 5001`,
    /// which follows `sub output, [entity + dodging]`. The game then raises
    /// a margin below 5001 to 5000. For a marked shot (`output` plus the
    /// target's Dodging is the marker) the margin becomes the chosen one and
    /// the flags say "not below", so the floor leaves it.
    FlooredCollision { output: Register, entity: Register },
}

pub struct Site {
    pub rva: usize,
    pub bytes: &'static [u8],
    pub kind: Kind,
}

// ---- The arithmetic -------------------------------------------------------

/// splitmix64 as the game computes it for its gate, so a pair's tick is the
/// same every time.
pub fn splitmix(seed: u64) -> u64 {
    let mut h = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^ (h >> 31)
}

/// The band `(low, high)` of reaction ticks for a gap in -100..=100 between
/// the caster's Accuracy and the defender's Dodging. Through (-100: 6..9),
/// (0: 10..14), (+100: 16..22) with the defaults; `floor` is the low end at
/// -100 and `ceiling` the high end at +100, and the middle points keep their
/// place between them (sixteenths of the span: low 0, 4, 10; high 3, 8, 16).
pub fn reaction_band(gap: i32, floor: u32, ceiling: u32) -> (u32, u32) {
    let floor = floor.max(1);
    let span = ceiling.max(floor) - floor;
    let gap = i64::from(gap.clamp(-100, 100));
    let point = |left: i64, right: i64, t: i64| left * (100 - t) + right * t;
    let at = |points: [i64; 3]| {
        let scaled = if gap < 0 {
            point(points[0], points[1], gap + 100)
        } else {
            point(points[1], points[2], gap)
        };
        // Round half up to a whole tick.
        let x1600 = i64::from(floor) * 1600 + i64::from(span) * scaled;
        ((x1600 + 800) / 1600) as u32
    };
    let (low, high) = (at([0, 4, 10]), at([3, 8, 16]));
    (low, high.max(low))
}

/// The tick this projectile-and-defender pair reacts at: a point of the band
/// chosen by the pair's seed, the same way the game picks inside its own.
pub fn reaction_gate(seed: u64, gap: i32, floor: u32, ceiling: u32) -> u32 {
    let (low, high) = reaction_band(gap, floor, ceiling);
    let width = u128::from(high - low) + 1;
    low + ((u128::from(splitmix(seed)) * width) >> 64) as u32
}

/// The seed bits that identify a projectile and defender: the age mixed in by
/// some gates (bits 40 and up) would change the choice every tick.
pub fn gate_seed(seed: u64, age_mixed: bool) -> u64 {
    if age_mixed {
        seed & ((1 << 40) - 1)
    } else {
        seed
    }
}

// ---- Settings copy and the controlled stamp --------------------------------

static ENABLED: AtomicBool = AtomicBool::new(false);
/// The hooks are patched in (only done at the title screen, with the
/// setting On).
static INSTALLED: AtomicBool = AtomicBool::new(false);
static FLOOR: AtomicU32 = AtomicU32::new(FLOOR_DEFAULT);
static CEILING: AtomicU32 = AtomicU32::new(CEILING_DEFAULT);
static MARGIN: AtomicU32 = AtomicU32::new(MARGIN_DEFAULT);
/// `thread << 32 | (actor + 1)`, or 0 when no athlete is controlled.
static CONTROLLED: AtomicU64 = AtomicU64::new(0);
static STAMPED_MS: AtomicU64 = AtomicU64::new(0);
static ORIGIN: OnceLock<Instant> = OnceLock::new();

fn now_ms() -> u64 {
    ORIGIN.get_or_init(Instant::now).elapsed().as_millis() as u64
}
fn pack(thread: u64, actor: u64) -> Option<u64> {
    (actor < u64::from(u32::MAX)).then(|| (thread << 32) | (actor + 1))
}

/// Copies the settings the hooks read. Called every client frame (cheap).
pub fn configure(on: bool, floor: u32, ceiling: u32, margin: u32) {
    MARGIN.store(margin.min(7), Ordering::Relaxed);
    ENABLED.store(on, Ordering::Relaxed);
    FLOOR.store(floor, Ordering::Relaxed);
    CEILING.store(ceiling, Ordering::Relaxed);
    if !on {
        clear();
    }
}
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}
pub fn set_installed() {
    INSTALLED.store(true, Ordering::Release);
}
/// The setting is On and the hooks are in, so the rules act.
pub fn in_effect() -> bool {
    enabled() && INSTALLED.load(Ordering::Acquire)
}
/// The collision margin in game units the player's shots get.
pub fn margin_units() -> u32 {
    MARGIN.load(Ordering::Relaxed)
}
/// What the shape-B consumers add to the output once the marker is seen: the
/// difference between the chosen margin and the marker's 5000, wrapping.
pub fn margin_delta() -> u64 {
    (u64::from(margin_units()) * 1000).wrapping_sub(COLLISION_MARKER)
}
pub fn limits() -> (u32, u32) {
    (
        FLOOR.load(Ordering::Relaxed),
        CEILING.load(Ordering::Relaxed),
    )
}

/// The controlled athlete `actor`, whose match runs on `thread`, ticked now.
/// Called from the controlled player's `think`, once per tick.
pub fn stamp(thread: u64, actor: usize) {
    if !enabled() {
        return;
    }
    let Some(word) = pack(thread, actor as u64) else {
        return;
    };
    STAMPED_MS.store(now_ms(), Ordering::Relaxed);
    if CONTROLLED.swap(word, Ordering::AcqRel) == 0 {
        for fired in &FIRED {
            fired.store(false, Ordering::Relaxed);
        }
    }
}
/// Control ended (release, hand-back to AI, left match, setting off).
pub fn clear() {
    CONTROLLED.store(0, Ordering::Release);
}

/// True when this thread runs the controlled athlete's match. The first,
/// cheapest question every hook asks.
#[inline]
pub fn active_on(thread: u64) -> bool {
    let word = CONTROLLED.load(Ordering::Relaxed);
    word != 0 && (word >> 32) == (thread & 0xffff_ffff)
}
/// True when `actor` is the controlled athlete and the stamp is fresh.
pub fn controlled(thread: u64, actor: u64) -> bool {
    let word = CONTROLLED.load(Ordering::Acquire);
    pack(thread & 0xffff_ffff, actor) == Some(word)
        && now_ms().saturating_sub(STAMPED_MS.load(Ordering::Relaxed)) <= STAMP_FRESH_MS
}

// ---- Counters and first-fire flags -----------------------------------------

#[derive(Clone, Copy)]
#[repr(usize)]
pub enum Rule {
    Accuracy,
    Dodging,
    Gate,
    Collision,
}
const RULES: usize = 4;
#[allow(clippy::declare_interior_mutable_const)]
const ZERO: AtomicU64 = AtomicU64::new(0);
#[allow(clippy::declare_interior_mutable_const)]
const OFF: AtomicBool = AtomicBool::new(false);
static FIRED: [AtomicBool; RULES] = [OFF; RULES];
static APPLIED: [AtomicU64; RULES] = [ZERO; RULES];
static SEEN: [AtomicU64; COLLISION_SLOTS] = [ZERO; COLLISION_SLOTS];
static MARKED: [AtomicU64; COLLISION_SLOTS] = [ZERO; COLLISION_SLOTS];

/// Counts one application of `rule`; true the first time since control began,
/// for the "a rule first fires" log line.
pub fn applied(rule: Rule) -> bool {
    APPLIED[rule as usize].fetch_add(1, Ordering::Relaxed);
    !FIRED[rule as usize].swap(true, Ordering::Relaxed)
}
/// One collision consumer ran on the controlled thread; `marked` when the
/// shot carried the marker and the Dodging term was skipped.
pub fn collision_seen(slot: usize, marked: bool) {
    let slot = slot.min(COLLISION_SLOTS - 1);
    SEEN[slot].fetch_add(1, Ordering::Relaxed);
    if marked {
        MARKED[slot].fetch_add(1, Ordering::Relaxed);
    }
}
/// Totals since start: applications per rule, then (seen, marked) per slot.
pub fn totals() -> ([u64; RULES], Vec<(u64, u64)>) {
    (
        std::array::from_fn(|i| APPLIED[i].load(Ordering::Relaxed)),
        (0..COLLISION_SLOTS)
            .map(|i| {
                (
                    SEEN[i].load(Ordering::Relaxed),
                    MARKED[i].load(Ordering::Relaxed),
                )
            })
            .collect(),
    )
}

// ---- What the controlled athlete cast (for the trace and the readout) -----

static COOLDOWNS: [AtomicU64; 4] = [ZERO; 4];
/// Tick + 1 at which each of Q, W, R last restarted its cooldown, 0 never.
static RESTARTED: [AtomicU64; 3] = [ZERO; 3];
/// The controlled athlete's cooldowns (attack, Q, W, R) at `tick`. A
/// cooldown that went up restarted: that skill was just used.
pub fn observe_cooldowns(now: [usize; 4], tick: usize) {
    for (slot, value) in now.into_iter().enumerate() {
        let before = COOLDOWNS[slot].swap(value as u64, Ordering::Relaxed);
        if slot > 0 && value as u64 > before {
            RESTARTED[slot - 1].store(tick as u64 + 1, Ordering::Relaxed);
        }
    }
}
/// The skill (Q, W, R) cast most recently before `tick` and how many ticks
/// before. Basic attacks restart their cooldown all the time, so they are
/// left out: a shot is attributed to a skill.
pub fn last_cast(tick: usize) -> Option<(&'static str, u64)> {
    let (slot, at) = (0..3)
        .map(|i| (i, RESTARTED[i].load(Ordering::Relaxed)))
        .max_by_key(|(_, at)| *at)?;
    (at > 0).then(|| (["Q", "W", "R"][slot], (tick as u64 + 1).saturating_sub(at)))
}

/// What a hook saw of one of the controlled athlete's shots, for the
/// practice readout: an Accuracy producer that marked it (with the word it
/// traces), or a hit test it reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShotEvent {
    pub producer: usize,
    /// True for an entity's action state, false for a projectile's kind.
    pub entity: bool,
    pub word: u64,
    pub hit_test: Option<HitTest>,
}

/// A collision consumer a marked shot reached: its shape and address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HitTest {
    /// 'A' (margin floored at 5 units) or 'B' (clamped at zero).
    pub shape: char,
    pub rva: usize,
}

/// Bits of the x86 flags register.
const CARRY: u64 = 1;
const ZERO_FLAG: u64 = 1 << 6;
const SIGN: u64 = 1 << 7;
const OVERFLOW: u64 = 1 << 11;
/// `flags` as a compare of a value above the other leaves them: not less, not
/// equal, no borrow. A following `cmovl` keeps its register, `cmovge` moves.
pub fn flags_above(flags: u64) -> u64 {
    flags & !(CARRY | ZERO_FLAG | SIGN | OVERFLOW)
}
/// Whether a shape-A margin is the marker minus the target's Dodging: the
/// controlled athlete's shot (natural values are Accuracy x 70 - Dodging).
pub fn floor_marked(margin: u64, dodging: u64) -> bool {
    margin.wrapping_add(dodging) == COLLISION_MARKER
}
/// The margin a marked shot gets at a shape-A floor, internal units.
pub fn floor_margin() -> u64 {
    u64::from(margin_units()) * 1000
}

/// Margin in game units that the skill preview adds to a projectile's radius
/// on each side, 0 when the rules are not in effect.
pub fn preview_margin() -> f32 {
    if in_effect() {
        margin_units() as f32
    } else {
        0.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_reproduce_the_agreed_table() {
        assert_eq!(reaction_band(-100, 6, 22), (6, 9));
        assert_eq!(reaction_band(0, 6, 22), (10, 14));
        assert_eq!(reaction_band(100, 6, 22), (16, 22));
    }

    #[test]
    fn the_band_is_monotonic_and_ordered() {
        let mut last = (0, 0);
        for gap in -100..=100 {
            let band = reaction_band(gap, 6, 22);
            assert!(band.0 <= band.1);
            assert!(band.0 >= last.0 && band.1 >= last.1, "gap {gap}");
            last = band;
        }
        // Out-of-range gaps clamp.
        assert_eq!(reaction_band(-500, 6, 22), reaction_band(-100, 6, 22));
        assert_eq!(reaction_band(500, 6, 22), reaction_band(100, 6, 22));
    }

    #[test]
    fn floor_and_ceiling_move_the_ends_and_scale_the_middle() {
        assert_eq!(reaction_band(-100, 10, 42).0, 10);
        assert_eq!(reaction_band(100, 10, 42).1, 42);
        // Span 32 is twice the default: the middle points double their offset.
        assert_eq!(reaction_band(0, 10, 42), (18, 26));
        // A ceiling below the floor collapses the band to the floor.
        for gap in [-100, 0, 100] {
            assert_eq!(reaction_band(gap, 12, 8), (12, 12));
        }
        // A floor of 0 is raised to 1 (a gate of 0 would react instantly).
        assert_eq!(reaction_band(-100, 0, 22).0, 1);
    }

    #[test]
    fn the_tick_inside_the_band_is_stable_per_pair() {
        for seed in [0, 1, 42, u64::MAX, 0xdead_beef_u64 << 20] {
            for gap in [-100, -30, 0, 55, 100] {
                let gate = reaction_gate(seed, gap, 6, 22);
                let (low, high) = reaction_band(gap, 6, 22);
                assert!((low..=high).contains(&gate));
                assert_eq!(gate, reaction_gate(seed, gap, 6, 22));
            }
        }
        // Different pairs spread over the band.
        let ticks: std::collections::BTreeSet<_> = (0..200u64)
            .map(|seed| reaction_gate(seed << 20, 0, 6, 22))
            .collect();
        assert!(ticks.len() >= 4);
    }

    #[test]
    fn splitmix_matches_the_known_sequence() {
        // Reference splitmix64 values for state 0 (first output).
        assert_eq!(splitmix(0), 0xe220_a839_7b1d_cdaf);
    }

    #[test]
    fn age_bits_are_dropped_only_where_the_game_mixes_them() {
        let seed = (7 << 40) | 0x1234;
        assert_eq!(gate_seed(seed, true), 0x1234);
        assert_eq!(gate_seed(seed, false), seed);
    }

    #[test]
    fn the_margin_delta_turns_the_marker_into_the_chosen_margin() {
        configure(true, 6, 22, 5);
        assert_eq!(COLLISION_MARKER.wrapping_add(margin_delta()), 5_000);
        for units in 0..=7u32 {
            configure(true, 6, 22, units);
            assert_eq!(
                COLLISION_MARKER.wrapping_add(margin_delta()),
                u64::from(units) * 1000
            );
        }
        // The slider's range is enforced.
        configure(true, 6, 22, 99);
        assert_eq!(margin_units(), 7);
        configure(true, 6, 22, MARGIN_DEFAULT);
    }

    #[test]
    fn a_shot_is_attributed_to_the_skill_cast_last_not_the_basic_attack() {
        observe_cooldowns([0, 0, 0, 0], 10);
        assert_eq!(last_cast(10), None);
        observe_cooldowns([0, 300, 0, 0], 11);
        assert_eq!(last_cast(15), Some(("Q", 4)));
        // Counting down changes nothing; basic attacks never take over.
        observe_cooldowns([40, 299, 0, 0], 12);
        observe_cooldowns([39, 298, 0, 0], 13);
        assert_eq!(last_cast(20), Some(("Q", 9)));
        observe_cooldowns([38, 297, 0, 900], 21);
        assert_eq!(last_cast(21), Some(("R", 0)));
    }

    #[test]
    fn the_marker_cannot_be_a_natural_accuracy_value() {
        for accuracy in 0..=100u64 {
            assert_ne!(accuracy * 70, COLLISION_MARKER);
        }
    }

    #[test]
    fn the_stamp_names_one_thread_and_one_actor() {
        clear();
        configure(true, 6, 22, MARGIN_DEFAULT);
        assert!(!active_on(5));
        stamp(5, 3);
        assert!(active_on(5) && !active_on(6));
        assert!(controlled(5, 3));
        assert!(!controlled(5, 4) && !controlled(6, 3));
        clear();
        assert!(!active_on(5) && !controlled(5, 3));
        // Turning the setting off ends a stamp and refuses new ones.
        stamp(5, 3);
        configure(false, 6, 22, MARGIN_DEFAULT);
        assert!(!active_on(5));
        stamp(5, 3);
        assert!(!active_on(5));
        configure(true, 6, 22, MARGIN_DEFAULT);
        clear();
    }

    #[test]
    fn a_floor_tells_the_marked_shot_from_natural_margins() {
        // The marker minus the target's Dodging (x 150) is the marked shot.
        assert!(floor_marked(5_000u64.wrapping_sub(7_500), 7_500));
        assert!(floor_marked(5_000, 0));
        // Accuracy x 70 is never 5000: 71 and 72 straddle it.
        for accuracy in 0..=100u64 {
            for dodging in [0u64, 1_500, 7_500, 15_000] {
                let margin = (accuracy * 70).wrapping_sub(dodging);
                assert!(!floor_marked(margin, dodging));
            }
        }
    }

    #[test]
    fn the_floor_flags_read_as_above() {
        // From a compare that came out less (sign set), equal, or with a borrow.
        for flags in [0x0202u64, 0x0282, 0x0242, 0x0a83, 0x0203] {
            let above = flags_above(flags);
            let sign = above & SIGN != 0;
            let overflow = above & OVERFLOW != 0;
            assert_eq!(sign, overflow, "cmovl must not fire");
            assert_eq!(above & ZERO_FLAG, 0);
            assert_eq!(above & CARRY, 0);
            // Every other bit (interrupts, direction, reserved 1) is kept.
            assert_eq!(
                above | CARRY | ZERO_FLAG | SIGN | OVERFLOW,
                flags | CARRY | ZERO_FLAG | SIGN | OVERFLOW
            );
        }
    }
}
