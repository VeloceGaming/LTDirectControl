//! The take order's walk (1.7.1-dev22, the author's spec sections 15.1 and
//! 15.2): on a take at a pit (Take here at a pit, the armed click on a pit
//! monster or a pit) the mod walks the teammates to it itself, narrowly,
//! through the official per-player AI interface (`StablePlayerAi::think`,
//! the mod's one registered AI, `simulation.rs`). No hook, no game
//! address. The objective order of section 11.9 is still planted as before,
//! so the game's own play may take them up at the pit.
//!
//! - **Who walks:** every teammate who hears the take, except one whose
//!   plan is a Battle at the ping (busy) and one who is low at the ping
//!   (declined at once, [`low`]). The player and the dead never hear it.
//! - **Low** (1.7.1-dev23, spec section 15.6) means dying soon: his
//!   time-to-die, current hp over the hp lost per second in the last
//!   [`DAMAGE_WINDOW_TICKS`], under [`DIE_WITHIN_SECS`]; or hp under
//!   [`HP_FLOOR`] whatever the rate. The history is each teammate's hp
//!   percent, sampled with the team's sight every 6 ticks ([`record`]).
//!   Superseded 2026-11-04: dev22's flat 35 % (`WALK_HP`), decided at the
//!   first think.
//! - **What `think` does for a walker:** when the game's own input is a
//!   move, a return or nothing, it is replaced by a move to the pit's rally
//!   point (`places::walk_target`), checked with `is_valid_input`; an
//!   attack, a skill or an ultimate passes, so he fights back and casts on
//!   the way.
//! - **Hand back** (pass from then on) at the first of: within
//!   [`WALK_NEAR`] of the target, the pit no longer a place (its monster
//!   died and is not due within the gate, `places::is_place`), low (the
//!   same rule each tick), a newer order to him ([`end_for`]), control released
//!   ([`end_all`]), [`WALK_FOR_SECS`].
//!
//! Threads: the client writes the table at the ping ([`start`]); `think`
//! runs on the simulation's worker for each player and reads it under a
//! short lock, after one atomic check, and only for the viewed match. It
//! never calls into the client and keeps no pointer.
//!
//! Not built (spec section 15.2, "other mods' AIs"): the interface does not
//! tell an AI whether one of higher priority decided first, so the case is
//! not logged. The mod's AI has priority 1000.
use super::places;
use crate::native_adapter::ping_orders::Status;
use crate::native_timing::MatchKey;
use crate::Logger;
use mod_api_stable::{InputKindV1, InputV1, StableAiContext};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// World units (1000 per map unit).
type Point = (u64, u64);
/// Under this hp percentage he is low whatever the rate.
pub const HP_FLOOR: f32 = 15.;
/// Low when his time-to-die is under this many seconds (the game's own
/// 5 s unit: its "too far" travel limit, its run-away length).
pub const DIE_WITHIN_SECS: f32 = 5.;
/// The hp lost over this many ticks (3 s) gives the rate.
pub const DAMAGE_WINDOW_TICKS: usize = 3 * super::SECOND;
/// Time-to-die in seconds: `now` hp percent over the percent lost per
/// second between `then` and `now`, `span` ticks apart; infinite with no
/// loss.
pub fn time_to_die(now: f32, then: f32, span: usize, tps: usize) -> f32 {
    let secs = span as f32 / tps.max(1) as f32;
    let rate = (then - now) / secs.max(f32::EPSILON);
    if rate <= 0. {
        f32::INFINITY
    } else {
        now / rate
    }
}
/// Low: dying soon, or under the floor.
pub fn low(now: f32, ttd: f32) -> bool {
    now < HP_FLOOR || ttd < DIE_WITHIN_SECS
}
/// Each player's hp percent samples (tick, percent), the last
/// [`DAMAGE_WINDOW_TICKS`] and one before, by SDK player id.
static HEALTH: Mutex<[Vec<(usize, f32)>; 10]> = Mutex::new([const { Vec::new() }; 10]);
/// The viewed match's step (`pings::observe`, every 6 ticks): player
/// `player`'s hp percent at `tick`. A tick before the last clears (a new
/// match).
pub fn record(player: usize, tick: usize, percent: f32) {
    let Ok(mut h) = HEALTH.lock() else {
        return;
    };
    let Some(samples) = h.get_mut(player) else {
        return;
    };
    if samples.last().is_some_and(|(t, _)| *t > tick) {
        samples.clear();
    }
    samples.push((tick, percent));
    // Keep the newest sample at or before the window's start, and after.
    let start = tick.saturating_sub(DAMAGE_WINDOW_TICKS);
    let keep = samples.iter().rposition(|(t, _)| *t <= start).unwrap_or(0);
    samples.drain(..keep);
}
/// Player `player`'s hp percent and time-to-die now, from the samples.
pub fn health(player: usize) -> Option<(f32, f32)> {
    let h = HEALTH.lock().ok()?;
    let samples = h.get(player)?;
    let &(t, now) = samples.last()?;
    let &(t0, then) = samples.first()?;
    Some((
        now,
        time_to_die(now, then, t.saturating_sub(t0), super::SECOND),
    ))
}
/// Within this many world units of the target he is there: the pit's
/// radius (`places::PIT_RADIUS`, 250 map units).
pub const WALK_NEAR: u64 = 250_000;
/// The walk's cap: On my way's, twice the game's 5 s "too far" limit.
pub const WALK_FOR_SECS: usize = 10;
/// The research line's period, in ticks.
const REPORT_EVERY: usize = super::SECOND;

/// Why a walk ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Arrived,
    Monster,
    Hp,
    Replaced,
    Released,
    Cap,
}
impl End {
    pub fn text(self) -> &'static str {
        match self {
            End::Arrived => "arrived",
            End::Monster => "monster",
            End::Hp => "hp",
            End::Replaced => "replaced",
            End::Released => "released",
            End::Cap => "cap",
        }
    }
}

/// One teammate's walk.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Walk {
    serial: u64,
    index: u8,
    player: usize,
    tag: u8,
    target: Point,
    /// The tick of his first think in it.
    start: Option<usize>,
    ended: Option<End>,
    status: Status,
    replaced: u32,
    passed: u32,
    report: usize,
    /// His hp percent and time-to-die at the last think (the log's).
    hp: Option<(f32, f32)>,
}
struct Table {
    key: MatchKey,
    walks: Vec<Walk>,
    /// The teammates left out at the ping, with their rows.
    excluded: Vec<(u64, u8, Status)>,
}
static TABLE: Mutex<Option<Table>> = Mutex::new(None);
/// Some walk may be running: `think` looks at the table only then.
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// What one think decides for a walker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The game's own input stands (an attack, a skill, an ultimate).
    Pass,
    /// The move to the target.
    Walk,
    End(End),
}
/// A walker's think, pure: the checks in the spec's order, then the input.
pub fn decide(
    base: Option<InputV1>,
    since: usize,
    low: bool,
    distance: Option<u64>,
    place: bool,
    tps: usize,
) -> Step {
    if low {
        return Step::End(End::Hp);
    }
    if !place {
        return Step::End(End::Monster);
    }
    if distance.is_some_and(|d| d <= WALK_NEAR) {
        return Step::End(End::Arrived);
    }
    if since >= WALK_FOR_SECS * tps.max(1) {
        return Step::End(End::Cap);
    }
    let kind = base.and_then(|b| InputKindV1::from_code(b.kind));
    match kind {
        None | Some(InputKindV1::Move | InputKindV1::Return) => Step::Walk,
        Some(_) => Step::Pass,
    }
}

/// Client: a take at pit `tag`: `walkers` (teammate index, SDK player id)
/// walk to `target`; `excluded` (index, row) are not walked. Any running
/// walk is replaced.
pub fn start(
    key: MatchKey,
    serial: u64,
    tag: u8,
    target: Point,
    walkers: &[(u8, usize)],
    excluded: &[(u8, Status)],
    log: &Logger,
) {
    end_all(End::Replaced, log);
    let Ok(mut t) = TABLE.lock() else {
        return;
    };
    let walks: Vec<Walk> = walkers
        .iter()
        .map(|&(index, player)| Walk {
            serial,
            index,
            player,
            tag,
            target,
            start: None,
            ended: None,
            status: Status::Accepted,
            replaced: 0,
            passed: 0,
            report: 0,
            hp: None,
        })
        .collect();
    for w in &walks {
        log.write(&format!(
            "PING walk n={serial} to={} player={} start target={},{} pit={}",
            w.index,
            w.player,
            target.0,
            target.1,
            if tag == 0 { "Morgard" } else { "Serpen" }
        ));
    }
    *t = Some(Table {
        key,
        walks,
        excluded: excluded.iter().map(|&(i, s)| (serial, i, s)).collect(),
    });
    ACTIVE.store(true, Ordering::Release);
}
/// Client: the walks of the teammates in `to` end (a newer order to them).
pub fn end_for(to: [bool; 5], why: End, log: &Logger) {
    let pick = |w: &Walk| to.get(usize::from(w.index)).copied().unwrap_or(false);
    for line in finish(pick, why) {
        log.write(&line);
    }
}
/// Client: every walk ends (control released, a newer take).
pub fn end_all(why: End, log: &Logger) {
    for line in end_all_lines(why) {
        log.write(&line);
    }
}
/// Every walk ends; the lines for the caller's log (the step hook's, at a
/// control swap, spec section 15.5).
pub fn end_all_lines(why: End) -> Vec<String> {
    finish(|_| true, why)
}
/// Ends the running walks `pick` selects; a line for each.
fn finish(pick: impl Fn(&Walk) -> bool, why: End) -> Vec<String> {
    let mut lines = Vec::new();
    let Ok(mut t) = TABLE.lock() else {
        return lines;
    };
    let Some(table) = t.as_mut() else {
        return lines;
    };
    for w in table
        .walks
        .iter_mut()
        .filter(|w| w.ended.is_none() && pick(w))
    {
        w.ended = Some(why);
        lines.push(end_line(w, why, None));
    }
    if table.walks.iter().all(|w| w.ended.is_some()) {
        ACTIVE.store(false, Ordering::Release);
    }
    lines
}
fn end_line(w: &Walk, why: End, tick: Option<usize>) -> String {
    let ticks = w.start.zip(tick).map(|(s, t)| t.saturating_sub(s));
    let hp = match (why, w.hp) {
        (End::Hp, Some((hp, ttd))) => format!(" hp={hp:.0} ttd={}", ttd_text(ttd)),
        _ => String::new(),
    };
    format!(
        "PING walk n={} to={} end why={}{hp} ticks={} replaced={} passed={}",
        w.serial,
        w.index,
        why.text(),
        ticks.map_or("?".to_owned(), |t| t.to_string()),
        w.replaced,
        w.passed
    )
}
/// A time-to-die for the log: seconds, or `inf`.
pub fn ttd_text(ttd: f32) -> String {
    if ttd.is_finite() {
        format!("{ttd:.1}")
    } else {
        "inf".to_owned()
    }
}
/// Client: walk `serial`'s rows by teammate index (None: not his).
pub fn answers(serial: u64) -> [Option<Status>; 5] {
    let mut out = [None; 5];
    if let Ok(t) = TABLE.lock() {
        if let Some(table) = t.as_ref() {
            for w in table.walks.iter().filter(|w| w.serial == serial) {
                out[usize::from(w.index).min(4)] = Some(w.status);
            }
            for &(_, i, s) in table.excluded.iter().filter(|e| e.0 == serial) {
                out[usize::from(i).min(4)] = Some(s);
            }
        }
    }
    out
}

/// The simulation's worker, every think of every player (`simulation.rs`):
/// the move for a walker of the viewed match, else None (the game's own
/// input stands).
pub fn think(
    ctx: &mut StableAiContext<'_>,
    base: Option<InputV1>,
    log: &Logger,
) -> Option<InputV1> {
    if !ACTIVE.load(Ordering::Acquire) {
        return None;
    }
    let player = ctx.player_id();
    let (key, walk) = {
        let t = TABLE.lock().ok()?;
        let table = t.as_ref()?;
        let w = *table
            .walks
            .iter()
            .find(|w| w.player == player && w.ended.is_none())?;
        (table.key, w)
    };
    let tick = ctx.tick();
    // The backstop: the ping's rule each tick, his latest samples.
    let hp = health(player);
    let is_low = hp.is_some_and(|(now, ttd)| low(now, ttd));
    let (pos, viewed) = {
        let sim = ctx.sim()?;
        let origin = sim.sim_origin()?;
        let viewed = origin.kind == 2 && (sim.seed(), origin.match_id, origin.set_index) == key;
        let pos = sim
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| c.pos());
        (pos, viewed)
    };
    if !viewed {
        return None;
    }
    let distance = pos.map(|p| {
        let (dx, dy) = (
            u128::from(p.0.abs_diff(walk.target.0)),
            u128::from(p.1.abs_diff(walk.target.1)),
        );
        ((dx * dx + dy * dy) as f64).sqrt() as u64
    });
    let since = walk.start.map_or(0, |s| tick.saturating_sub(s));
    let step = decide(
        base,
        since,
        is_low,
        distance,
        places::is_place(walk.tag),
        super::SECOND,
    );
    let target = InputV1::move_to(walk.target.0, walk.target.1);
    let walking = step == Step::Walk && ctx.is_valid_input(&target);
    let Ok(mut t) = TABLE.lock() else {
        return None;
    };
    let w = t
        .as_mut()?
        .walks
        .iter_mut()
        .find(|w| w.player == player && w.serial == walk.serial && w.ended.is_none())?;
    w.start.get_or_insert(tick);
    w.hp = hp;
    match step {
        Step::End(why) => {
            w.ended = Some(why);
            log.write(&end_line(w, why, Some(tick)));
            return None;
        }
        _ if walking => w.replaced += 1,
        _ => w.passed += 1,
    }
    #[cfg(feature = "research")]
    if tick >= w.report + REPORT_EVERY {
        w.report = tick;
        log.write(&format!(
            "PING walk n={} to={} t={tick} replaced={} passed={} distance={}",
            w.serial,
            w.index,
            w.replaced,
            w.passed,
            distance.map_or("?".to_owned(), |d| (d / 1000).to_string())
        ));
    }
    #[cfg(not(feature = "research"))]
    let _ = REPORT_EVERY;
    walking.then_some(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(kind: InputKindV1) -> Option<InputV1> {
        Some(InputV1 {
            kind: kind.code(),
            ..InputV1::default()
        })
    }
    #[test]
    fn an_attack_or_a_skill_passes_a_move_or_nothing_walks() {
        let far = Some(600_000);
        for kind in [
            InputKindV1::Attack,
            InputKindV1::Skill,
            InputKindV1::Skill2,
            InputKindV1::Ult,
        ] {
            assert_eq!(
                decide(input(kind), 0, false, far, true, 60),
                Step::Pass,
                "{kind:?}"
            );
        }
        for base in [input(InputKindV1::Move), input(InputKindV1::Return), None] {
            assert_eq!(decide(base, 0, false, far, true, 60), Step::Walk);
        }
    }
    #[test]
    fn the_walk_hands_back_at_the_first_reason() {
        let mv = input(InputKindV1::Move);
        assert_eq!(
            decide(mv, 0, true, Some(600_000), true, 60),
            Step::End(End::Hp)
        );
        assert_eq!(
            decide(mv, 0, false, Some(600_000), false, 60),
            Step::End(End::Monster)
        );
        assert_eq!(
            decide(mv, 0, false, Some(WALK_NEAR), true, 60),
            Step::End(End::Arrived)
        );
        assert_eq!(
            decide(mv, 600, false, Some(600_000), true, 60),
            Step::End(End::Cap)
        );
        assert_eq!(
            decide(mv, 599, false, Some(WALK_NEAR + 1), true, 60),
            Step::Walk
        );
        // An attack still passes the checks first: low health ends it.
        assert_eq!(
            decide(input(InputKindV1::Attack), 0, true, None, true, 60),
            Step::End(End::Hp)
        );
    }
    #[test]
    fn low_means_dying_soon_or_under_the_floor() {
        let tps = 60;
        let window = DAMAGE_WINDOW_TICKS;
        // 35 % with no recent loss walks.
        let ttd = time_to_die(35., 35., window, tps);
        assert!(ttd.is_infinite() && !low(35., ttd));
        // 60 % losing 25 % a second (75 % over 3 s): 2.4 s, declined.
        let ttd = time_to_die(60., 135., window, tps);
        assert!((ttd - 2.4).abs() < 0.01 && low(60., ttd));
        // 14 %, whatever the rate.
        assert!(low(14., f32::INFINITY));
        // The samples: 3 s of history kept; the walk ends when the rate
        // rises mid-walk.
        for t in (0..=180).step_by(6) {
            record(7, t, 80.);
        }
        assert!(health(7).is_some_and(|(h, ttd)| h == 80. && !low(h, ttd)));
        for (k, t) in (186..=360).step_by(6).enumerate() {
            record(7, t, 80. - 1.5 * (k + 1) as f32);
        }
        let (now, ttd) = health(7).unwrap();
        assert!(low(now, ttd), "{now} {ttd}");
        let mv = input(InputKindV1::Move);
        assert_eq!(
            decide(mv, 30, low(now, ttd), Some(600_000), true, tps),
            Step::End(End::Hp)
        );
        // A new match (an earlier tick) starts the history again.
        record(7, 6, 50.);
        assert_eq!(health(7).map(|h| h.0), Some(50.));
    }
    #[test]
    fn the_rows_and_the_client_ends() {
        let log = crate::test_support::logger("walk");
        let key = (1, 2, 3);
        start(
            key,
            77,
            1,
            (600_000, 600_000),
            &[(0, 10), (3, 13)],
            &[(2, Status::Busy)],
            &log,
        );
        let rows = answers(77);
        assert_eq!(rows[0], Some(Status::Accepted));
        assert_eq!(rows[2], Some(Status::Busy), "fighting at the ping");
        assert_eq!(rows[1], None);
        assert!(ACTIVE.load(Ordering::Acquire));
        end_for([true, false, false, false, false], End::Replaced, &log);
        assert!(ACTIVE.load(Ordering::Acquire), "teammate 3 still walks");
        end_all(End::Released, &log);
        assert!(!ACTIVE.load(Ordering::Acquire));
        assert_eq!(answers(77)[3], Some(Status::Accepted), "rows stay");
    }
}
