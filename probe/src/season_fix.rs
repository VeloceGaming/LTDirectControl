//! Season statistics follow the match that was played.
//!
//! The game simulates each fixture twice: an AI-only server pre-simulation,
//! and the match on screen, which the player controls. A few seconds into the
//! match it adds every athlete's pre-simulated numbers to the athlete record
//! (`season_statistics`); at the end of a series the league competition
//! record's per-athlete statistics move as well. The stored replay of the
//! game holds what was played. See docs/investigation-played-result.md.
//!
//! The game stores the replays of a series only when the series ends, and
//! the league takes the series in at the same moment. So the correction is
//! made once per series: the client copies the records when the series
//! starts, and once the replay of every game exists replaces the
//! pre-simulated games with the played ones. Only a server extension may
//! write records, so the client sends each correction as a command. Every
//! write is compare-and-set: the server applies it only to the value the
//! client saw, so a late or repeated command never overwrites a newer game.
use crate::{native_timing::MatchKey, Logger};
use mod_api_stable::{
    CommandResultV1, RecordKindV1, StableClient, StableCommand, StableServerCtx,
    StableServerExtension,
};
use serde_json::{json, Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const COMMAND: &str = "lt_direct_control.season_fix";
const DUMP: &str = "lt_direct_control.season_dump";
/// Mod save key set once the past of this save is repaired.
const REPAIRED: &str = "season_repair_v1";
const REPAIR_DONE: &str = "lt_direct_control.season_repair_done";

// ---------------------------------------------------------------------------
// Arithmetic on statistics documents (pure; tested)

fn number(v: &Value) -> Option<Value> {
    v.as_i64()
        .map(Value::from)
        .or_else(|| v.as_f64().map(Value::from))
}
fn combine(a: &Value, b: &Value, sign: i64) -> Value {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            let mut out = Map::new();
            for key in a.keys().chain(b.keys()) {
                if out.contains_key(key) {
                    continue;
                }
                let x = a.get(key).unwrap_or(&Value::Null);
                let y = b.get(key).unwrap_or(&Value::Null);
                out.insert(key.clone(), combine(x, y, sign));
            }
            Value::Object(out)
        }
        (Value::Null, Value::Object(_)) => combine(&json!({}), b, sign),
        (Value::Object(_), Value::Null) => a.clone(),
        _ => {
            let x = number(a).unwrap_or(json!(0));
            let y = number(b).unwrap_or(json!(0));
            match (x.as_i64(), y.as_i64()) {
                (Some(x), Some(y)) => json!(x + sign * y),
                _ => json!(x.as_f64().unwrap_or(0.) + sign as f64 * y.as_f64().unwrap_or(0.)),
            }
        }
    }
}
/// `a + b` over numeric leaves; objects merge, a missing field counts as 0.
pub fn add(a: &Value, b: &Value) -> Value {
    combine(a, b, 1)
}
/// `a - b` over numeric leaves.
pub fn sub(a: &Value, b: &Value) -> Value {
    combine(a, b, -1)
}
fn is_zero(v: &Value) -> bool {
    match v {
        Value::Object(m) => m.values().all(is_zero),
        Value::Null => true,
        _ => v.as_f64() == Some(0.),
    }
}
/// Equal as statistics: every numeric leaf the same, absent counting as 0.
pub fn same(a: &Value, b: &Value) -> bool {
    is_zero(&sub(a, b)) && is_zero(&sub(b, a))
}

/// One athlete's line in the stored replay of the played game.
#[derive(Clone, Debug, PartialEq)]
pub struct Played {
    pub athlete: usize,
    pub position: String,
    pub champion: String,
    pub statistics: Value,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Fix {
    pub path: String,
    pub expected: Value,
    pub value: Value,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Replace the pre-simulated games (`presim`, all positions summed) with
    /// the played ones (`played`).
    Write {
        fixes: Vec<Fix>,
        presim: Value,
        played: Value,
    },
    /// The played games equal the pre-simulated ones (AI-played games).
    Same {
        presim: Value,
    },
    Skip(&'static str),
}

/// The correction of one athlete's `season_statistics` for a series:
/// `before` copied when the series started, `now` after it, and his lines
/// in the series' stored replays. The pre-simulation's contribution is
/// `now - before` by definition; it is accepted only if, in each position,
/// it is exactly the replays' number of games of the replays' champions.
pub fn athlete_fix(before: &Value, now: &Value, lines: &[&Played]) -> Outcome {
    let (Some(before), Some(now)) = (before.as_array(), now.as_array()) else {
        return Outcome::Skip("season statistics unreadable");
    };
    if lines.is_empty() {
        return Outcome::Skip("not in the stored replays");
    }
    let year = |e: &Value| e.get("year").and_then(Value::as_i64);
    let changed = now
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            before
                .iter()
                .find(|b| year(b) == year(entry))
                .is_none_or(|b| b != *entry)
        })
        .collect::<Vec<_>>();
    let [(index, entry)] = changed[..] else {
        return Outcome::Skip(if changed.is_empty() {
            "no pre-simulated game recorded since the series started"
        } else {
            "more than one season changed"
        });
    };
    let empty = json!({});
    let base_entry = before
        .iter()
        .find(|b| year(b) == year(entry))
        .unwrap_or(&empty);
    let (Some(now_positions), base_positions) = (
        entry.get("by_position").and_then(Value::as_object),
        base_entry.get("by_position"),
    ) else {
        return Outcome::Skip("season statistics have no positions");
    };
    // What was played, per position: the summed statistics and the number
    // of games of each champion.
    let mut played: BTreeMap<&str, (Value, BTreeMap<&str, i64>)> = BTreeMap::new();
    for line in lines {
        let (sum, champions) = played
            .entry(line.position.as_str())
            .or_insert_with(|| (json!({}), BTreeMap::new()));
        *sum = add(sum, &line.statistics);
        *champions.entry(line.champion.as_str()).or_insert(0) += 1;
    }
    let moved = now_positions
        .iter()
        .filter(|(k, v)| base_positions.and_then(|b| b.get(k.as_str())) != Some(*v))
        .map(|(k, _)| k.as_str())
        .collect::<Vec<_>>();
    if !moved.iter().copied().eq(played.keys().copied()) {
        return Outcome::Skip("the changed positions are not the ones played");
    }
    let (mut fixes, mut presim_all, mut played_all) = (Vec::new(), json!({}), json!({}));
    for (position, (sum, champions)) in &played {
        let current = &now_positions[*position];
        let base = base_positions
            .and_then(|b| b.get(*position))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let presim = sub(current, &base);
        let games = champions.values().sum::<i64>();
        let these_games = presim.get("matches").and_then(Value::as_i64) == Some(games)
            && champions.iter().all(|(champion, n)| {
                presim
                    .pointer(&format!("/played_champion/{champion}"))
                    .and_then(Value::as_i64)
                    == Some(*n)
            });
        if !these_games {
            return Outcome::Skip("the change is not the series' games of the played champions");
        }
        let value = add(&base, sum);
        presim_all = add(&presim_all, &presim);
        played_all = add(&played_all, sum);
        if !same(&value, current) {
            fixes.push(Fix {
                path: format!("season_statistics.{index}.by_position.{position}"),
                expected: current.clone(),
                value,
            });
        }
    }
    if fixes.is_empty() {
        return Outcome::Same { presim: presim_all };
    }
    Outcome::Write {
        fixes,
        presim: presim_all,
        played: played_all,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LeagueOutcome {
    /// The league has not taken the series in yet.
    Pending,
    /// It moved by the played numbers (it copies the corrected athlete record).
    Right,
    Write(Fix),
    Skip(&'static str),
}
/// The league's per-athlete statistics for the series: `start` when the
/// series began, `now`, and the summed pre-simulated and played games.
pub fn league_fix(
    competition_path: String,
    start: &Value,
    now: &Value,
    presim: &Value,
    played: &Value,
) -> LeagueOutcome {
    if same(now, start) {
        return LeagueOutcome::Pending;
    }
    let moved = sub(now, start);
    if same(&moved, played) {
        return LeagueOutcome::Right;
    }
    if !same(&moved, presim) {
        return LeagueOutcome::Skip(
            "the league moved by neither the played nor the pre-simulated games",
        );
    }
    let base = if start.is_null() {
        json!({})
    } else {
        start.clone()
    };
    LeagueOutcome::Write(Fix {
        path: competition_path,
        expected: now.clone(),
        value: add(&base, played),
    })
}

/// What the stored replays of the season add up to, per athlete.
#[derive(Default, Debug)]
pub struct Sums {
    /// Athlete record: (athlete, position) -> summed statistics.
    pub positions: BTreeMap<(usize, String), Value>,
    /// League competition -> athlete -> summed statistics.
    pub leagues: BTreeMap<usize, BTreeMap<usize, Value>>,
    /// Athletes who shared a game with the player's team (with or against).
    pub involved: BTreeSet<usize>,
}
impl Sums {
    pub fn add_replay(&mut self, lines: &[Played], competition: Option<usize>, involved: bool) {
        for line in lines {
            let key = (line.athlete, line.position.clone());
            let sum = self.positions.entry(key).or_insert_with(|| json!({}));
            *sum = add(sum, &line.statistics);
            if let Some(c) = competition {
                let sum = self
                    .leagues
                    .entry(c)
                    .or_default()
                    .entry(line.athlete)
                    .or_insert_with(|| json!({}));
                *sum = add(sum, &line.statistics);
            }
            if involved {
                self.involved.insert(line.athlete);
            }
        }
    }
}
/// The repair of one athlete's current-year entry: `entry` is
/// `season_statistics[index]`. Positions he did not play in the replays must
/// be empty.
pub fn position_fixes(
    athlete: usize,
    index: usize,
    entry: &Value,
    sums: &Sums,
) -> (bool, Vec<Fix>) {
    let empty = Map::new();
    let positions = entry
        .get("by_position")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let mut exact = true;
    let mut fixes = Vec::new();
    let names = positions
        .keys()
        .cloned()
        .chain(
            sums.positions
                .keys()
                .filter(|(a, _)| *a == athlete)
                .map(|(_, p)| p.clone()),
        )
        .collect::<BTreeSet<_>>();
    for position in names {
        let current = positions.get(&position).cloned().unwrap_or(Value::Null);
        let expected = sums
            .positions
            .get(&(athlete, position.clone()))
            .cloned()
            .unwrap_or_else(|| json!({}));
        if same(&current, &expected) {
            continue;
        }
        exact = false;
        fixes.push(Fix {
            path: format!("season_statistics.{index}.by_position.{position}"),
            expected: current,
            value: expected,
        });
    }
    (exact, fixes)
}

/// The athlete lines of a stored replay.
pub fn replay_lines(replay: &Value) -> Vec<Played> {
    ["blue_team", "red_team"]
        .into_iter()
        .filter_map(|side| replay.get(side)?.as_array())
        .flatten()
        .filter_map(|p| {
            Some(Played {
                athlete: usize::try_from(p.get("athlete_id")?.as_u64()?).ok()?,
                position: p.get("position")?.as_str()?.to_owned(),
                champion: p.get("champion")?.as_str()?.to_owned(),
                statistics: p.get("statistics")?.clone(),
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Athletes of the match on screen, from the simulation worker

static ATHLETES: Mutex<Vec<(MatchKey, BTreeMap<usize, usize>)>> = Mutex::new(Vec::new());

/// Called from the watched match's `think` for each player early on.
pub fn observe_athlete(key: MatchKey, player: usize, athlete: usize) {
    let Ok(mut all) = ATHLETES.lock() else { return };
    if let Some((_, map)) = all.iter_mut().find(|(k, _)| *k == key) {
        map.insert(player, athlete);
        return;
    }
    if all.len() >= 16 {
        all.remove(0);
    }
    all.push((key, BTreeMap::from([(player, athlete)])));
}
fn athletes(key: MatchKey) -> Vec<usize> {
    ATHLETES.lock().map_or_else(
        |_| Vec::new(),
        |all| {
            all.iter()
                .find(|(k, _)| *k == key)
                .map(|(_, map)| map.values().copied().collect())
                .unwrap_or_default()
        },
    )
}

// ---------------------------------------------------------------------------
// Client side

/// A series being played: the records when it started and its games.
struct Series {
    match_id: u64,
    /// Seeds of its games, in order.
    seeds: Vec<u64>,
    competition: Option<usize>,
    athletes: Vec<usize>,
    start: BTreeMap<usize, Value>,
    league_start: BTreeMap<usize, Value>,
    exited: Option<Instant>,
    next_look: Option<Instant>,
}
/// A corrected series whose league statistics are still to move.
struct League {
    competition: usize,
    start: BTreeMap<usize, Value>,
    presim: BTreeMap<usize, Value>,
    played: BTreeMap<usize, Value>,
    until: Instant,
    next_look: Option<Instant>,
}
struct Sent {
    record: &'static str,
    id: usize,
    path: String,
    value: Value,
    deadline: Instant,
    next_look: Instant,
}

/// Rebuilding this save's current season from its stored replays, read a
/// few records per frame.
struct Repair {
    year: i64,
    team: usize,
    matches: Vec<usize>,
    /// Replay id -> (league competition, the player's team played in it).
    replays: Vec<(usize, Option<usize>)>,
    competitions: BTreeMap<usize, BTreeSet<usize>>,
    sums: Sums,
    athletes: Vec<usize>,
    records: BTreeMap<usize, Value>,
    started: Instant,
}

#[derive(Default)]
pub struct SeasonFix {
    seen: BTreeSet<MatchKey>,
    was_battlefield: bool,
    series: Option<Series>,
    league: Option<League>,
    sent: Vec<Sent>,
    repair: Option<Repair>,
    repair_tried: Option<Instant>,
    repaired: bool,
}

fn read(ctx: &StableClient<'_>, kind: RecordKindV1, id: usize, path: &str) -> Value {
    ctx.record_get_json(kind, id, path)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null)
}

impl SeasonFix {
    pub fn update(
        &mut self,
        ctx: &mut StableClient<'_>,
        key: Option<MatchKey>,
        battlefield: bool,
        log: &Logger,
    ) {
        if battlefield {
            if let Some(key) = key.filter(|k| !self.seen.contains(k)) {
                self.begin(ctx, key, log);
            }
        }
        if self.was_battlefield && !battlefield {
            if let Some(series) = self.series.as_mut() {
                series.exited = Some(Instant::now());
                if crate::logging::verbose() {
                    ctx.send_command(DUMP, series.match_id.to_string().as_bytes());
                }
            }
        }
        self.was_battlefield = battlefield;
        if !battlefield {
            self.finish_series(ctx, log);
        }
        self.watch_league(ctx, log);
        self.confirm(ctx, log);
        self.repair_past(ctx, battlefield, log);
    }

    /// Once per save: rebuild the current season of every athlete who shared
    /// a game with the player's team from the stored replays, which hold the
    /// played games. Only in the management screens, never while a series
    /// waits for its correction. The rule is checked first on every athlete
    /// who never met the player's team: their records must equal their
    /// replays exactly, or nothing is written.
    fn repair_past(&mut self, ctx: &mut StableClient<'_>, battlefield: bool, log: &Logger) {
        let quiet = !battlefield
            && self.series.is_none()
            && self.league.is_none()
            && self.sent.is_empty()
            && ctx.client_scene_kind() == Some(mod_api_stable::ClientSceneKindV1::Main);
        if !quiet {
            if self.repair.take().is_some() {
                log.write("SEASON repair interrupted; it starts again later");
            }
            return;
        }
        if self.repaired {
            return;
        }
        if self.repair.is_none() {
            // Wait for a loaded save: the management scene, a real date and
            // its matches. The client reports an empty world before that.
            if ctx.scene_kind() != Some(mod_api_stable::SceneKindV1::InGame) {
                return;
            }
            let (Some((year, ..)), Some(team)) = (ctx.game_time(), ctx.player_team_id()) else {
                return;
            };
            let mut matches = ctx.record_ids(RecordKindV1::MatchNormal);
            if year < 2000 || matches.is_empty() {
                return;
            }
            if self
                .repair_tried
                .is_some_and(|at| at.elapsed() < Duration::from_secs(300))
            {
                return;
            }
            self.repair_tried = Some(Instant::now());
            if ctx.save_get_string(REPAIRED).is_some() {
                self.repaired = true;
                return;
            }
            matches.sort_unstable();
            log.write(&format!(
                "SEASON repair started: season {year}, {} matches to read",
                matches.len()
            ));
            self.repair = Some(Repair {
                year: i64::from(year),
                team,
                matches,
                replays: Vec::new(),
                competitions: BTreeMap::new(),
                sums: Sums::default(),
                athletes: Vec::new(),
                records: BTreeMap::new(),
                started: Instant::now(),
            });
            return;
        }
        let Some(r) = self.repair.as_mut() else {
            return;
        };
        // 1. League competitions of the season (once).
        if r.competitions.is_empty() {
            for id in ctx.record_ids(RecordKindV1::LeagueCompetition) {
                if read(ctx, RecordKindV1::LeagueCompetition, id, "finalized") != Value::Bool(false)
                {
                    continue;
                }
                let matches = read(ctx, RecordKindV1::LeagueCompetition, id, "matches");
                let set = matches
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|m| usize::try_from(m.as_u64()?).ok())
                    .collect::<BTreeSet<_>>();
                r.competitions.insert(id, set);
            }
            r.competitions.insert(usize::MAX, BTreeSet::new());
            return;
        }
        // 2. Matches of this year, a few dozen per frame.
        if !r.matches.is_empty() {
            for _ in 0..40 {
                let Some(id) = r.matches.pop() else { break };
                let record = read(ctx, RecordKindV1::MatchNormal, id, "");
                let year = record
                    .get("date")
                    .and_then(Value::as_str)
                    .and_then(|d| d.get(..4)?.parse::<i64>().ok());
                if year != Some(r.year) || record.get("is_practice") == Some(&Value::Bool(true)) {
                    continue;
                }
                let competition = r
                    .competitions
                    .iter()
                    .find(|(_, m)| m.contains(&id))
                    .map(|(c, _)| *c);
                for replay in record
                    .get("replays")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|v| usize::try_from(v.as_u64()?).ok())
                {
                    r.replays.push((replay, competition));
                }
            }
            return;
        }
        // 3. Their replays.
        if !r.replays.is_empty() {
            for _ in 0..20 {
                let Some((id, competition)) = r.replays.pop() else {
                    break;
                };
                let replay = read(ctx, RecordKindV1::MatchReplay, id, "");
                let involved = [&replay["blue_team_id"], &replay["red_team_id"]]
                    .iter()
                    .any(|t| t.as_u64() == Some(r.team as u64));
                r.sums
                    .add_replay(&replay_lines(&replay), competition, involved);
            }
            if r.replays.is_empty() {
                r.athletes = r
                    .sums
                    .positions
                    .keys()
                    .map(|(a, _)| *a)
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
            }
            return;
        }
        // 4. The athletes' records.
        if r.records.len() < r.athletes.len() {
            let next = r.athletes[r.records.len()..]
                .iter()
                .take(40)
                .copied()
                .collect::<Vec<_>>();
            for id in next {
                let v = read(ctx, RecordKindV1::Athlete, id, "season_statistics");
                r.records.insert(id, v);
            }
            return;
        }
        // 5. Check the rule on the athletes who never met the player's team,
        //    then write.
        let Some(r) = self.repair.take() else { return };
        let mut fixes: Vec<(&'static str, usize, Fix)> = Vec::new();
        let mut clean = 0;
        let mut broken = Vec::new();
        for (athlete, record) in &r.records {
            let entry = record.as_array().and_then(|a| {
                a.iter()
                    .enumerate()
                    .find(|(_, e)| e.get("year").and_then(Value::as_i64) == Some(r.year))
            });
            let (exact, found) = match entry {
                Some((index, entry)) => position_fixes(*athlete, index, entry, &r.sums),
                None => (false, Vec::new()),
            };
            if r.sums.involved.contains(athlete) {
                fixes.extend(found.into_iter().map(|f| ("athlete", *athlete, f)));
            } else if exact {
                clean += 1;
            } else {
                broken.push(format!("athlete {athlete}"));
            }
        }
        for (competition, athletes) in &r.sums.leagues {
            let current = read(
                ctx,
                RecordKindV1::LeagueCompetition,
                *competition,
                "statistics",
            );
            for (athlete, expected) in athletes {
                let now = current
                    .get(athlete.to_string())
                    .cloned()
                    .unwrap_or(Value::Null);
                if same(&now, expected) {
                    if !r.sums.involved.contains(athlete) {
                        clean += 1;
                    }
                    continue;
                }
                if r.sums.involved.contains(athlete) {
                    fixes.push((
                        "league",
                        *competition,
                        Fix {
                            path: format!("statistics.{athlete}"),
                            expected: now,
                            value: expected.clone(),
                        },
                    ));
                } else {
                    broken.push(format!("league {competition} athlete {athlete}"));
                }
            }
        }
        if !broken.is_empty() || clean < 10 {
            log.write(&format!(
                "SEASON repair not applied: the stored replays do not add up to the records of {} athletes who never met your team (first: {:?}); {clean} did. Nothing written",
                broken.len(),
                broken.iter().take(5).collect::<Vec<_>>()
            ));
            return;
        }
        log.write(&format!(
            "SEASON repair checked: {clean} records of athletes who never met your team equal their stored replays; {} records to repair; read in {:.1} s",
            fixes.len(),
            r.started.elapsed().as_secs_f32()
        ));
        for (record, id, fix) in fixes {
            self.send(ctx, record, id, fix, log);
        }
        let done = ctx.send_command(REPAIR_DONE, b"");
        self.repaired = done;
        log.write(&format!("SEASON repair sent; done flag sent={done}"));
    }

    /// A game of the match on screen starts. The first game of a series
    /// copies the records, before its pre-simulation lands.
    fn begin(&mut self, ctx: &StableClient<'_>, key: MatchKey, log: &Logger) {
        let ids = athletes(key);
        if ids.is_empty() {
            return;
        }
        self.seen.insert(key);
        if let Some(series) = self.series.as_mut().filter(|s| s.match_id == key.1) {
            series.seeds.push(key.0);
            series.exited = None;
            log.write(&format!(
                "SEASON game {} of series match={} key={key:?}",
                series.seeds.len(),
                key.1
            ));
            return;
        }
        if let Some(old) = self.series.take() {
            log.write(&format!(
                "SEASON series match={} skipped: a new series started before its replays were stored",
                old.match_id
            ));
        }
        let start = ids
            .iter()
            .map(|id| {
                let path = "season_statistics";
                (*id, read(ctx, RecordKindV1::Athlete, *id, path))
            })
            .collect();
        let competition = ctx.player_team_id().and_then(|team| {
            ctx.record_ids(RecordKindV1::LeagueCompetition)
                .into_iter()
                .find(|id| {
                    read(ctx, RecordKindV1::LeagueCompetition, *id, "finalized")
                        == Value::Bool(false)
                        && !read(
                            ctx,
                            RecordKindV1::LeagueCompetition,
                            *id,
                            &format!("standings.{team}"),
                        )
                        .is_null()
                })
        });
        let league_start = competition.map_or_else(BTreeMap::new, |c| {
            ids.iter()
                .map(|id| {
                    let path = format!("statistics.{id}");
                    (*id, read(ctx, RecordKindV1::LeagueCompetition, c, &path))
                })
                .collect()
        });
        log.write(&format!(
            "SEASON series match={} game 1 key={key:?} league_competition={competition:?} athletes={ids:?}; records copied before the pre-simulation lands",
            key.1
        ));
        self.series = Some(Series {
            match_id: key.1,
            seeds: vec![key.0],
            competition,
            athletes: ids,
            start,
            league_start,
            exited: None,
            next_look: None,
        });
    }

    /// Between games: once the replay of every game of the series is
    /// stored (when the series ends), correct the athletes' records.
    fn finish_series(&mut self, ctx: &mut StableClient<'_>, log: &Logger) {
        let Some(series) = self.series.as_mut() else {
            return;
        };
        let Some(exited) = series.exited else { return };
        if series.next_look.is_some_and(|at| Instant::now() < at) {
            return;
        }
        series.next_look = Some(Instant::now() + Duration::from_secs(1));
        let mut ids = ctx.record_ids(RecordKindV1::MatchReplay);
        ids.sort_unstable();
        let mut replays = BTreeMap::new();
        for id in ids.iter().rev().take(48) {
            let seed = read(ctx, RecordKindV1::MatchReplay, *id, "seed").as_u64();
            if let Some(seed) = seed.filter(|s| series.seeds.contains(s)) {
                replays
                    .entry(seed)
                    .or_insert_with(|| read(ctx, RecordKindV1::MatchReplay, *id, ""));
            }
        }
        if replays.len() < series.seeds.len() {
            // A series in progress stores nothing yet. Give up only long
            // after a game that would have ended it.
            if exited.elapsed() > Duration::from_secs(600) {
                log.write(&format!(
                    "SEASON series match={} skipped: replays of {} of its {} games stored after 10 minutes",
                    series.match_id,
                    replays.len(),
                    series.seeds.len()
                ));
                self.series = None;
            }
            return;
        }
        let Some(series) = self.series.take() else {
            return;
        };
        let lines = replays.values().flat_map(replay_lines).collect::<Vec<_>>();
        let mut league = League {
            competition: series.competition.unwrap_or(usize::MAX),
            start: BTreeMap::new(),
            presim: BTreeMap::new(),
            played: BTreeMap::new(),
            until: Instant::now() + Duration::from_secs(120),
            next_look: None,
        };
        for athlete in &series.athletes {
            let now = read(ctx, RecordKindV1::Athlete, *athlete, "season_statistics");
            let mine = lines
                .iter()
                .filter(|l| l.athlete == *athlete)
                .collect::<Vec<_>>();
            let before = series.start.get(athlete).unwrap_or(&Value::Null);
            let outcome = athlete_fix(before, &now, &mine);
            let league_start = series.league_start.get(athlete).cloned();
            match outcome {
                Outcome::Write {
                    fixes,
                    presim,
                    played,
                } => {
                    if let Some(start) = league_start {
                        league.start.insert(*athlete, start);
                        league.presim.insert(*athlete, presim);
                        league.played.insert(*athlete, played);
                    }
                    for fix in fixes {
                        self.send(ctx, "athlete", *athlete, fix, log);
                    }
                }
                Outcome::Same { .. } => log.write(&format!(
                    "SEASON athlete={athlete} series match={} unchanged: the played games equal the pre-simulation",
                    series.match_id
                )),
                Outcome::Skip(reason) => log.write(&format!(
                    "SEASON athlete={athlete} series match={} skipped: {reason}",
                    series.match_id
                )),
            }
        }
        if series.competition.is_some() && !league.start.is_empty() {
            self.league = Some(league);
        }
    }

    /// After a corrected series: the league's per-athlete statistics.
    fn watch_league(&mut self, ctx: &mut StableClient<'_>, log: &Logger) {
        let Some(league) = self.league.as_mut() else {
            return;
        };
        if Instant::now() > league.until {
            log.write("SEASON league statistics did not move within 2 minutes of the series");
            self.league = None;
            return;
        }
        if league.next_look.is_some_and(|at| Instant::now() < at) {
            return;
        }
        league.next_look = Some(Instant::now() + Duration::from_secs(2));
        let mut fixes = Vec::new();
        let mut pending = false;
        for (athlete, start) in &league.start {
            let path = format!("statistics.{athlete}");
            let now = read(
                ctx,
                RecordKindV1::LeagueCompetition,
                league.competition,
                &path,
            );
            let none = json!({});
            let outcome = league_fix(
                path,
                start,
                &now,
                league.presim.get(athlete).unwrap_or(&none),
                league.played.get(athlete).unwrap_or(&none),
            );
            match outcome {
                LeagueOutcome::Pending => pending = true,
                LeagueOutcome::Write(fix) => fixes.push((*athlete, fix)),
                LeagueOutcome::Right => log.write(&format!(
                    "SEASON league athlete={athlete} already follows the played games"
                )),
                LeagueOutcome::Skip(reason) => log.write(&format!(
                    "SEASON league athlete={athlete} skipped: {reason}"
                )),
            }
        }
        if pending {
            // Taken in all at once: wait until every athlete moved.
            return;
        }
        let competition = league.competition;
        self.league = None;
        for (_, fix) in fixes {
            self.send(ctx, "league", competition, fix, log);
        }
    }

    fn send(
        &mut self,
        ctx: &mut StableClient<'_>,
        record: &'static str,
        id: usize,
        fix: Fix,
        log: &Logger,
    ) {
        let payload = json!({"record": record, "id": id, "path": fix.path, "expected": fix.expected, "value": fix.value});
        let sent = ctx.send_command(COMMAND, payload.to_string().as_bytes());
        log.write(&format!(
            "SEASON {record}={id} {} correction sent={sent}: {} -> {}",
            fix.path,
            summary(&fix.expected),
            summary(&fix.value)
        ));
        if sent {
            self.sent.push(Sent {
                record,
                id,
                path: fix.path,
                value: fix.value,
                deadline: Instant::now() + Duration::from_secs(15),
                next_look: Instant::now() + Duration::from_secs(1),
            });
        }
    }

    /// Re-read each corrected value until the client's copy shows it.
    fn confirm(&mut self, ctx: &StableClient<'_>, log: &Logger) {
        let now = Instant::now();
        self.sent.retain_mut(|s| {
            if now < s.next_look {
                return true;
            }
            s.next_look = now + Duration::from_secs(1);
            let kind = if s.record == "athlete" {
                RecordKindV1::Athlete
            } else {
                RecordKindV1::LeagueCompetition
            };
            if same(&read(ctx, kind, s.id, &s.path), &s.value) {
                log.write(&format!(
                    "SEASON {}={} {} correction applied",
                    s.record, s.id, s.path
                ));
                return false;
            }
            if now > s.deadline {
                log.write(&format!(
                    "SEASON {}={} {} correction not seen by the client after 15 s",
                    s.record, s.id, s.path
                ));
                return false;
            }
            true
        });
    }
}

/// K/D/A, MVP and games of a statistics document, for the log.
fn summary(v: &Value) -> String {
    let n = |k: &str| v.get(k).and_then(Value::as_i64).unwrap_or(0);
    format!(
        "{}/{}/{} mvp={} games={} wins={}",
        n("kills"),
        n("deaths"),
        n("assists"),
        n("mvp"),
        n("matches"),
        n("wins")
    )
}

// ---------------------------------------------------------------------------
// Server side: applies a correction only to the value the client saw

pub struct SeasonServer {
    pub logger: Arc<Logger>,
}
impl SeasonServer {
    fn apply(&self, ctx: &mut StableServerCtx<'_>, payload: &[u8]) {
        let Some(p) = serde_json::from_slice::<Value>(payload).ok() else {
            self.logger
                .write("SEASON server refused: unreadable command");
            return;
        };
        let (Some(record), Some(id), Some(path), Some(expected), Some(value)) = (
            p.get("record").and_then(Value::as_str),
            p.get("id")
                .and_then(Value::as_u64)
                .and_then(|id| usize::try_from(id).ok()),
            p.get("path").and_then(Value::as_str),
            p.get("expected"),
            p.get("value"),
        ) else {
            self.logger
                .write("SEASON server refused: incomplete command");
            return;
        };
        let current = match record {
            "athlete" => ctx.athlete_get_json(id, path),
            "league" => ctx.record_get_json(RecordKindV1::LeagueCompetition, id, path),
            _ => None,
        }
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .unwrap_or(Value::Null);
        if !same(&current, expected) {
            self.logger.write(&format!(
                "SEASON server refused {record}={id} {path}: it changed since the client read it"
            ));
            return;
        }
        let text = value.to_string();
        let written = match record {
            "athlete" => ctx.athlete_set_json(id, path, &text),
            _ => ctx.record_set_json(RecordKindV1::LeagueCompetition, id, path, &text),
        };
        self.logger.write(&format!(
            "SEASON server {record}={id} {path} written={written}{}",
            if written {
                ""
            } else {
                "; the game's schema check failed"
            }
        ));
    }
    /// Verbose diagnostic: the server-only match record of a series.
    fn dump(&self, ctx: &StableServerCtx<'_>, payload: &[u8]) {
        let Some(id) = std::str::from_utf8(payload)
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
        else {
            return;
        };
        let Some(root) = self.logger.directory() else {
            return;
        };
        let text = ctx
            .record_get_json(RecordKindV1::Match, id, "")
            .unwrap_or_default();
        let path = root
            .join("research")
            .join(format!("server-match-{id}.json"));
        let written = std::fs::create_dir_all(root.join("research")).is_ok()
            && std::fs::write(&path, &text).is_ok();
        self.logger.write(&format!(
            "SEASON server match record {id} bytes={} written={written}",
            text.len()
        ));
    }
}
impl StableServerExtension for SeasonServer {
    fn handle_command(
        &self,
        ctx: &mut StableServerCtx<'_>,
        command: &StableCommand<'_>,
    ) -> CommandResultV1 {
        match command.command {
            COMMAND => self.apply(ctx, command.payload),
            DUMP => self.dump(ctx, command.payload),
            REPAIR_DONE => {
                let saved = ctx.save_set_string(REPAIRED, "1");
                self.logger.write(&format!(
                    "SEASON server repair recorded in the save={saved}"
                ));
            }
            _ => return CommandResultV1::Pass,
        }
        CommandResultV1::Handled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Series 616 game 2, athlete 599 (Sunna), as read in game on 2026-10-14:
    // the record when the game started, one minute in (the pre-simulation
    // landed: 1/2/3, no MVP), and the stored replay 681 (played 12/3/7, MVP).
    fn start() -> Value {
        json!([{"year": 2026, "by_position": {"Mid": {
            "assists": 62, "deaths": 22, "kills": 101, "matches": 11, "wins": 10, "mvp": 7,
            "level": 146, "rating": 835, "solo_kill": 26, "solo_killed": 5,
            "dealing": 298325, "tanking": 146050, "healing": 46814, "gold": 154709,
            "played_champion": {"fighter": 1, "nullifier": 3},
            "champion_detail": {"fighter": {"dealing": 10422, "healing": 0, "matches": 1, "rating": 70, "tanking": 10497, "wins": 1}}
        }}}])
    }
    fn presim() -> Value {
        json!({"assists": 3, "deaths": 2, "kills": 1, "matches": 1, "wins": 1, "level": 11,
            "rating": 71, "dealing": 6812, "tanking": 4997, "gold": 8178,
            "played_champion": {"fighter": 1},
            "champion_detail": {"fighter": {"dealing": 6812, "matches": 1, "rating": 71, "tanking": 4997, "wins": 1}}})
    }
    fn during() -> Value {
        let mut v = start();
        v[0]["by_position"]["Mid"] = add(&v[0]["by_position"]["Mid"], &presim());
        v
    }
    fn played() -> Played {
        Played {
            athlete: 599,
            position: "Mid".into(),
            champion: "fighter".into(),
            statistics: json!({"assists": 7, "champion_detail": {"fighter": {"dealing": 23316, "healing": 2278, "matches": 1, "rating": 84, "tanking": 8896, "wins": 1}}, "dealing": 23316, "deaths": 3, "gold": 13811, "healing": 2278, "kills": 12, "level": 13, "matches": 1, "mvp": 1, "played_champion": {"fighter": 1}, "rating": 84, "solo_kill": 7, "solo_killed": 1, "tanking": 8896, "wins": 1}),
        }
    }

    fn write(outcome: Outcome) -> (Vec<Fix>, Value, Value) {
        let Outcome::Write {
            fixes,
            presim,
            played,
        } = outcome
        else {
            panic!("expected a write, got {outcome:?}");
        };
        (fixes, presim, played)
    }
    #[test]
    fn the_pre_simulated_game_is_replaced_by_the_played_one() {
        let during = during();
        assert_eq!(during[0]["by_position"]["Mid"]["kills"], 102);
        let (fixes, presim, played) = write(athlete_fix(&start(), &during, &[&self::played()]));
        assert!(same(&presim, &self::presim()));
        assert!(same(&played, &self::played().statistics));
        let [fix] = &fixes[..] else { panic!() };
        assert_eq!(fix.path, "season_statistics.0.by_position.Mid");
        assert_eq!(fix.expected, during[0]["by_position"]["Mid"]);
        let v = &fix.value;
        assert_eq!(
            (
                v["kills"].clone(),
                v["deaths"].clone(),
                v["assists"].clone()
            ),
            (json!(113), json!(25), json!(69))
        );
        assert_eq!(
            (v["mvp"].clone(), v["matches"].clone(), v["wins"].clone()),
            (json!(8), json!(12), json!(11))
        );
        assert_eq!(v["level"], 159);
        assert_eq!(v["healing"], 46814 + 2278);
        assert_eq!(v["played_champion"]["fighter"], 2);
        assert_eq!(v["played_champion"]["nullifier"], 3);
        assert_eq!(v["champion_detail"]["fighter"]["matches"], 2);
        assert_eq!(v["champion_detail"]["fighter"]["healing"], 2278);
    }
    #[test]
    fn a_whole_series_is_replaced_at_once() {
        // Series 620 as read in game: both pre-simulated games landed (the
        // first one a loss) before any replay was stored; played 14/2/5 and
        // 12/0/6, both won.
        let game = |kills, deaths, assists, wins, champion: &str| {
            json!({"kills": kills, "deaths": deaths, "assists": assists, "matches": 1,
                "wins": wins, "played_champion": {champion: 1}})
        };
        let presim = add(&game(4, 4, 6, 0, "nullifier"), &game(5, 0, 8, 1, "fighter"));
        let mut now = start();
        now[0]["by_position"]["Mid"] = add(&now[0]["by_position"]["Mid"], &presim);
        let first = Played {
            champion: "nullifier".into(),
            statistics: add(&game(14, 2, 5, 1, "nullifier"), &json!({"mvp": 1})),
            ..played()
        };
        let second = Played {
            statistics: add(&game(12, 0, 6, 1, "fighter"), &json!({"mvp": 1})),
            ..played()
        };
        let (fixes, got_presim, _) = write(athlete_fix(&start(), &now, &[&first, &second]));
        assert!(same(&got_presim, &presim));
        let v = &fixes[0].value;
        assert_eq!(
            [
                &v["kills"],
                &v["deaths"],
                &v["assists"],
                &v["matches"],
                &v["wins"],
                &v["mvp"]
            ],
            [
                &json!(127),
                &json!(24),
                &json!(73),
                &json!(13),
                &json!(12),
                &json!(9)
            ]
        );
        // Only one of the two games accounted for: not this series' change.
        assert!(matches!(
            athlete_fix(&start(), &now, &[&second]),
            Outcome::Skip(_)
        ));
    }
    #[test]
    fn nothing_is_written_without_exactly_the_series_pre_simulated_games() {
        // Copy taken after the pre-simulation landed: nothing changed since.
        assert_eq!(
            athlete_fix(&during(), &during(), &[&played()]),
            Outcome::Skip("no pre-simulated game recorded since the series started")
        );
        // Two games landed for one replay.
        let mut twice = during();
        twice[0]["by_position"]["Mid"] = add(&twice[0]["by_position"]["Mid"], &presim());
        assert!(matches!(
            athlete_fix(&start(), &twice, &[&played()]),
            Outcome::Skip(_)
        ));
        // Another position or champion than the replay says, or no replay.
        let other = Played {
            position: "Top".into(),
            ..played()
        };
        assert!(matches!(
            athlete_fix(&start(), &during(), &[&other]),
            Outcome::Skip(_)
        ));
        let other = Played {
            champion: "gunner".into(),
            ..played()
        };
        assert!(matches!(
            athlete_fix(&start(), &during(), &[&other]),
            Outcome::Skip(_)
        ));
        assert!(matches!(
            athlete_fix(&start(), &during(), &[]),
            Outcome::Skip(_)
        ));
    }
    #[test]
    fn an_ai_played_game_and_a_first_game_in_a_position_are_handled() {
        // The game on screen was the AI's: same numbers, nothing to write.
        let ai = Played {
            statistics: presim(),
            ..played()
        };
        assert!(matches!(
            athlete_fix(&start(), &during(), &[&ai]),
            Outcome::Same { .. }
        ));
        // First game in a position (and in a year): the base is empty.
        let fresh_start = json!([]);
        let fresh_now = json!([{"year": 2026, "by_position": {"Mid": presim()}}]);
        let (fixes, _, _) = write(athlete_fix(&fresh_start, &fresh_now, &[&played()]));
        assert!(same(&fixes[0].value, &played().statistics));
    }
    #[test]
    fn the_league_is_corrected_only_when_it_took_the_pre_simulated_games() {
        let start = json!({"kills": 96, "matches": 10});
        let presim = json!({"kills": 2, "matches": 2});
        let played = json!({"kills": 22, "matches": 2});
        let path = || "statistics.599".to_owned();
        assert_eq!(
            league_fix(path(), &start, &start, &presim, &played),
            LeagueOutcome::Pending
        );
        let took_presim = add(&start, &presim);
        assert_eq!(
            league_fix(path(), &start, &took_presim, &presim, &played),
            LeagueOutcome::Write(Fix {
                path: path(),
                expected: took_presim.clone(),
                value: json!({"kills": 118, "matches": 12}),
            })
        );
        let took_played = add(&start, &played);
        assert_eq!(
            league_fix(path(), &start, &took_played, &presim, &played),
            LeagueOutcome::Right
        );
        let other = json!({"kills": 50, "matches": 11});
        assert!(matches!(
            league_fix(path(), &start, &other, &presim, &played),
            LeagueOutcome::Skip(_)
        ));
    }
    #[test]
    fn the_repair_compares_whole_entries_with_the_replay_sums() {
        let line = |athlete, position: &str, kills| Played {
            athlete,
            position: position.into(),
            champion: "fighter".into(),
            statistics: json!({"kills": kills, "matches": 1, "played_champion": {"fighter": 1}}),
        };
        let mut sums = Sums::default();
        sums.add_replay(&[line(599, "Mid", 12), line(1, "Top", 3)], Some(6), true);
        sums.add_replay(&[line(599, "Mid", 14), line(2, "Top", 4)], Some(6), false);
        assert!(sums.involved.contains(&599) && !sums.involved.contains(&2));
        assert_eq!(sums.leagues[&6][&599]["kills"], 26);
        // Matches its replays: nothing to write.
        let entry = json!({"year": 2026, "by_position": {"Top": {"kills": 4, "matches": 1, "played_champion": {"fighter": 1}}}});
        assert_eq!(position_fixes(2, 0, &entry, &sums), (true, vec![]));
        // The pre-simulated numbers: rebuilt from the replays.
        let entry = json!({"year": 2026, "by_position": {"Mid": {"kills": 9, "matches": 2, "played_champion": {"fighter": 2}}}});
        let (exact, fixes) = position_fixes(599, 3, &entry, &sums);
        assert!(!exact);
        assert_eq!(fixes[0].path, "season_statistics.3.by_position.Mid");
        assert_eq!(fixes[0].value["kills"], 26);
        // A position with numbers but no replays is not exact either.
        let entry = json!({"year": 2026, "by_position": {"Top": {"kills": 4, "matches": 1, "played_champion": {"fighter": 1}}, "Mid": {"kills": 1, "matches": 1}}});
        assert!(!position_fixes(2, 0, &entry, &sums).0);
    }
    #[test]
    fn replay_lines_name_athlete_position_and_champion() {
        let replay = json!({"blue_team": [{"athlete_id": 579, "position": "Top", "champion": "amazon", "statistics": {"kills": 1}}],
            "red_team": [{"athlete_id": 599, "position": "Mid", "champion": "fighter", "statistics": {"kills": 12}}, {"id": 3}]});
        let lines = replay_lines(&replay);
        assert_eq!(lines.len(), 2);
        assert_eq!((lines[1].athlete, lines[1].position.as_str()), (599, "Mid"));
    }
}
