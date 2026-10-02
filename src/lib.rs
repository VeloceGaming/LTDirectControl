//! Input and targeting policy, independent of executable hooks.
//!
//! This is not a loadable game mod. The future 0.6.2 adapter must supply
//! current cursor coordinates, authoritative eligibility and runtime ability
//! descriptors, then dispatch these intents through native input validation.

pub mod pacing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: u64,
    pub y: u64,
}

impl Point {
    fn distance_squared(self, other: Self) -> u128 {
        // Saturation also keeps deliberately invalid/extreme coordinates safe.
        let dx = u128::from(self.x.abs_diff(other.x));
        let dy = u128::from(self.y.abs_diff(other.y));
        (dx * dx).saturating_add(dy * dy)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub id: usize,
    pub position: Point,
    pub is_champion: bool,
    /// Evaluated by the adapter for THIS action, including side, visibility,
    /// targetability, range, death, and the ability's native restrictions.
    pub eligible: bool,
    /// Native screen/world hit testing mapped into the current cursor space.
    /// The adapter must account for camera transforms and non-circular hitboxes.
    pub cursor_hit: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbilitySlot {
    Q,
    W,
    R,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastKind {
    Position,
    Direction,
    Unit,
    SelfCast,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastTarget {
    Position(Point),
    /// Cursor endpoint; the adapter constructs the engine's native direction.
    DirectionToward(Point),
    Unit(usize),
    SelfCast,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    /// An attack-move order remains active in the adapter. This is the immediate
    /// targeting decision; the adapter re-evaluates after movement/target death.
    Attack(usize),
    Move(Point),
    Cast {
        slot: AbilitySlot,
        target: CastTarget,
    },
}

fn nearest(
    cursor: Point,
    candidates: &[Candidate],
    champion_only: bool,
    require_cursor_hit: bool,
) -> Option<usize> {
    candidates
        .iter()
        .filter(|candidate| {
            candidate.eligible
                && (!champion_only || candidate.is_champion)
                && (!require_cursor_hit || candidate.cursor_hit)
        })
        // Stable tie-breaking makes the choice independent of enumeration order.
        .min_by_key(|candidate| (cursor.distance_squared(candidate.position), candidate.id))
        .map(|candidate| candidate.id)
}

/// Prefer the eligible target nearest the click, not nearest the controlled
/// champion. With no eligible target, advance toward the clicked location.
/// Eligibility/acquisition bounds must come from the native game adapter.
pub fn attack_move(cursor: Point, candidates: &[Candidate], champion_only: bool) -> Intent {
    nearest(cursor, candidates, champion_only, false)
        .map(Intent::Attack)
        .unwrap_or(Intent::Move(cursor))
}

/// Resolve on key press: point and direction skills do not need a second click.
/// Unit skills need a valid cursor hit; do not silently acquire a distant unit.
pub fn quick_cast(
    slot: AbilitySlot,
    kind: CastKind,
    cursor: Point,
    candidates: &[Candidate],
    champion_only: bool,
) -> Option<Intent> {
    let target = match kind {
        CastKind::Position => CastTarget::Position(cursor),
        CastKind::Direction => CastTarget::DirectionToward(cursor),
        CastKind::SelfCast => CastTarget::SelfCast,
        CastKind::Unit => CastTarget::Unit(nearest(cursor, candidates, champion_only, true)?),
    };
    Some(Intent::Cast { slot, target })
}

/// Client-only preview state. Call with physical key transitions, ignoring
/// key-repeat events. Never share this directly with background simulation.
#[derive(Default, Debug)]
pub struct AbilityControls {
    preview: Option<AbilitySlot>,
}

impl AbilityControls {
    pub fn preview(&self) -> Option<AbilitySlot> {
        self.preview
    }

    pub fn press(
        &mut self,
        slot: AbilitySlot,
        shift_held: bool,
        kind: CastKind,
        cursor: Point,
        candidates: &[Candidate],
        champion_only: bool,
    ) -> Option<Intent> {
        if shift_held {
            self.preview = Some(slot);
            return None;
        }
        self.preview = None;
        quick_cast(slot, kind, cursor, candidates, champion_only)
    }

    /// Releasing an ability can only dismiss its preview; it never casts.
    pub fn release(&mut self, slot: AbilitySlot) {
        if self.preview == Some(slot) {
            self.preview = None;
        }
    }

    /// Use for Shift release, Escape, focus loss, champion changes, death,
    /// AI release, and match exit. None of those events dispatches a cast.
    pub fn cancel(&mut self) {
        self.preview = None;
    }
}
