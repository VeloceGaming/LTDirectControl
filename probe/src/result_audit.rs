//! Read-only evidence for the full-battle test; never edits match history.
use crate::{native_timing::MatchKey, player_hud::Snapshot, Logger};
use mod_api_stable::{ClientSceneKindV1, RecordKindV1, StableClient};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    sync::Mutex,
    time::{Duration, Instant},
};

// Diagnostic for docs/investigation-played-result.md, at Verbose log detail
// only: the game simulates a fixture more than once (an AI-only server
// pre-simulation and the match on screen). Keep every player's latest numbers
// from each run, and save the management records around a played match, to
// see which run the season totals follow. Read-only.

/// One player's numbers in one simulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunPlayer {
    pub player: usize,
    pub side: usize,
    pub kda: (usize, usize, usize),
    pub cs: usize,
    pub level: usize,
    pub gold: usize,
}
/// The latest sample of one simulation of a fixture.
#[derive(Clone, Debug)]
struct Run {
    seed: u64,
    match_id: u64,
    set_index: u64,
    /// `SimOriginKindV1` code: 1 server pre-simulation, 2 the match on screen.
    kind: u32,
    tick: usize,
    players: Vec<RunPlayer>,
}
static RUNS: Mutex<Vec<Run>> = Mutex::new(Vec::new());
const RUN_LIMIT: usize = 256;
/// Ticks between samples: the final numbers can be this far from the end.
pub const RUN_SAMPLE_TICKS: usize = 30;

/// Called from any player's `think` of a simulation; `players` is read only
/// for the first caller of a sampled tick.
pub fn observe_run(
    key: MatchKey,
    kind: u32,
    tick: usize,
    players: impl FnOnce() -> Vec<RunPlayer>,
) {
    let Ok(mut runs) = RUNS.lock() else { return };
    let at = runs
        .iter()
        .position(|r| (r.seed, r.match_id, r.set_index, r.kind) == (key.0, key.1, key.2, kind));
    if at.is_some_and(|i| runs[i].tick == tick) {
        return;
    }
    let run = Run {
        seed: key.0,
        match_id: key.1,
        set_index: key.2,
        kind,
        tick,
        players: players(),
    };
    match at {
        Some(i) => runs[i] = run,
        None => {
            if runs.len() >= RUN_LIMIT {
                runs.remove(0);
            }
            runs.push(run);
        }
    }
}
/// Every recorded simulation of the fixture with this seed and match id (the
/// pre-simulation reports another set index than the match on screen).
fn runs(key: MatchKey) -> Value {
    let Ok(runs) = RUNS.lock() else {
        return Value::Null;
    };
    runs.iter()
        .filter(|r| (r.seed, r.match_id) == (key.0, key.1))
        .map(|r| {
            json!({
                "origin_kind": r.kind, "set_index": r.set_index, "last_sampled_tick": r.tick,
                "players": r.players.iter().map(|p| json!({
                    "player": p.player, "side": p.side, "kda": [p.kda.0, p.kda.1, p.kda.2],
                    "cs": p.cs, "level": p.level, "gold": p.gold,
                })).collect::<Vec<_>>(),
            })
        })
        .collect()
}
/// Whole records of one kind, bounded: `ids`, or every record of the kind.
fn records(ctx: &StableClient<'_>, kind: RecordKindV1, ids: Option<&[usize]>) -> Value {
    const COUNT: usize = 32;
    const BYTES: usize = 12 * 1024 * 1024;
    let all;
    let ids = match ids {
        Some(ids) => ids,
        None => {
            all = ctx.record_ids(kind);
            &all
        }
    };
    let mut bytes = 0;
    let mut out = serde_json::Map::new();
    for id in ids.iter().take(COUNT) {
        let Some(text) = ctx.record_get_json(kind, *id, "") else {
            continue;
        };
        bytes += text.len();
        if bytes > BYTES {
            out.insert("truncated".into(), json!(true));
            break;
        }
        out.insert(
            id.to_string(),
            serde_json::from_str(&text).unwrap_or(Value::Null),
        );
    }
    out.insert("record_count".into(), json!(ids.len()));
    Value::Object(out)
}

#[derive(Default)]
pub struct ResultAudit {
    key: Option<MatchKey>,
    generation: u64,
    baseline: BTreeSet<usize>,
    was_battlefield: bool,
    deadline: Option<Instant>,
    next_read: Option<Instant>,
    last_report: Option<Value>,
    last_player: Option<Snapshot>,
    result_screen: Option<Value>,
    /// Own athletes of the audited match, and which record captures are done.
    athletes: Vec<usize>,
    started: Option<Instant>,
    captured_start: bool,
    captured_during: bool,
    captured_after: bool,
}
fn result_labels(ctx: &StableClient<'_>) -> Value {
    let mut pending = vec![("main.contents".to_owned(), 0)];
    let mut labels = serde_json::Map::new();
    let mut visited = 0;
    while let Some((path, depth)) = pending.pop() {
        visited += 1;
        if visited > 320 {
            break;
        }
        if let Some(text) = ctx.ui_text(&path).filter(|s| !s.is_empty()) {
            labels.insert(
                path.clone(),
                Value::String(text.chars().take(800).collect()),
            );
        }
        if depth < 8 {
            for child in ctx.ui_child_names(&path).into_iter().take(60) {
                pending.push((format!("{path}.{child}"), depth + 1));
            }
        }
    }
    Value::Object(labels)
}
fn replay_ids(record: &Value) -> BTreeSet<usize> {
    record
        .get("replays")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .filter_map(|n| usize::try_from(n).ok())
        .collect()
}
impl ResultAudit {
    fn observe_player(&mut self, key: MatchKey, battlefield: bool, player: Option<&Snapshot>) {
        if battlefield {
            if let Some(player) = player.filter(|p| p.key == key) {
                self.last_player = Some(player.clone());
            }
        }
    }
    /// Save the own athletes' records (and, unless `light`, the team and the
    /// league competitions) with every recorded simulation of the fixture.
    fn capture_records(
        &self,
        ctx: &StableClient<'_>,
        key: MatchKey,
        label: &str,
        light: bool,
        log: &Logger,
    ) {
        if !crate::logging::verbose() {
            return;
        }
        let Some(root) = log.directory() else { return };
        let reports = root.join("research");
        if std::fs::create_dir_all(&reports).is_err() {
            return;
        }
        let team = ctx.player_team_id();
        let mut report = json!({
            "version": env!("CARGO_PKG_VERSION"), "label": label,
            "controlled_key": {"seed": key.0, "match_id": key.1, "set_index": key.2},
            "game_time": ctx.game_time().map(|t| json!([t.0, t.1, t.2, t.3, t.4])),
            "seconds_since_key_bound": self.started.map(|at| at.elapsed().as_secs_f32()),
            "player_team_id": team,
            "simulations": runs(key),
            "athletes": records(ctx, RecordKindV1::Athlete, Some(&self.athletes)),
        });
        if !light {
            report["team"] = records(ctx, RecordKindV1::Team, team.as_slice().into());
            report["league_competitions"] = records(ctx, RecordKindV1::LeagueCompetition, None);
        }
        let path = reports.join(format!(
            "session-records-{}-set{}-seed{}-{label}.json",
            key.1, key.2, key.0
        ));
        let bytes = serde_json::to_vec(&report).unwrap_or_default();
        let size = bytes.len();
        let written = std::fs::write(&path, bytes).is_ok();
        log.write(&format!(
            "RESULT RECORDS label={label} file={} written={written} bytes={size} athletes={:?}",
            path.display(),
            self.athletes
        ));
    }
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        ctx: &StableClient<'_>,
        key: Option<MatchKey>,
        generation: u64,
        battlefield: bool,
        last_player: Option<&Snapshot>,
        own_athletes: &[usize],
        log: &Logger,
    ) {
        // Continue the prior result's bounded polling between battles. A new
        // generation only replaces it once a new foreground key is bound.
        let Some(audit_key) = key.or(self.key) else {
            return;
        };
        if key.is_some() && (self.key != key || self.generation != generation) {
            self.generation = generation;
            let key = audit_key;
            self.key = Some(key);
            self.baseline = ctx
                .record_ids(RecordKindV1::MatchReplay)
                .into_iter()
                .collect();
            self.was_battlefield = false;
            self.deadline = None;
            self.next_read = None;
            self.last_report = None;
            self.last_player = None;
            self.result_screen = None;
            self.athletes.clear();
            self.started = Some(Instant::now());
            self.captured_start = false;
            self.captured_during = false;
            self.captured_after = false;
        }
        let key = audit_key;
        self.observe_player(key, battlefield, last_player);
        // The roster is registered on the match's first tick, just after the
        // key is bound.
        if battlefield && !self.captured_start && !own_athletes.is_empty() {
            self.athletes = own_athletes.to_vec();
            self.captured_start = true;
            self.capture_records(ctx, key, "start", false, log);
        }
        // One minute in, the pre-simulation has long finished: does the
        // athlete's record already hold its numbers?
        if battlefield
            && !self.captured_during
            && self
                .started
                .is_some_and(|at| at.elapsed() >= Duration::from_secs(60))
        {
            self.captured_during = true;
            self.capture_records(ctx, key, "during", true, log);
        }
        if self.was_battlefield && !battlefield {
            self.deadline = Some(Instant::now() + Duration::from_secs(30));
            log.write("RESULT AUDIT battlefield exit; read-only history capture pending (authority unverified)");
            self.capture_records(ctx, key, "exit", false, log);
        }
        if !battlefield
            && !self.captured_after
            && self.deadline.is_some_and(|at| Instant::now() > at)
        {
            self.captured_after = true;
            self.capture_records(ctx, key, "after", false, log);
        }
        self.was_battlefield = battlefield;
        if battlefield
            || self.deadline.is_none_or(|at| Instant::now() > at)
            || self.next_read.is_some_and(|at| Instant::now() < at)
        {
            return;
        }
        self.next_read = Some(Instant::now() + Duration::from_secs(1));
        if ctx.client_scene_kind() == Some(ClientSceneKindV1::MatchResult) {
            self.result_screen = Some(result_labels(ctx));
        }
        let mut matches = Vec::new();
        let mut correlated = BTreeSet::new();
        if let Ok(id) = usize::try_from(key.1) {
            for kind in [
                RecordKindV1::MatchNormal,
                RecordKindV1::MatchPractice,
                RecordKindV1::MatchTutorial,
                RecordKindV1::MatchSoloRank,
                RecordKindV1::Match,
            ] {
                let record = ctx
                    .record_get_json(kind, id, "")
                    .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                    .or_else(|| {
                        ctx.record_get_json(kind, id, "replays")
                            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                            .map(|replays| json!({"replays": replays}))
                    });
                if let Some(record) = record {
                    correlated.extend(replay_ids(&record));
                    matches.push(
                        json!({"record_kind": kind.code(), "match_id": id, "record": record}),
                    );
                }
            }
        }
        let current = ctx
            .record_ids(RecordKindV1::MatchReplay)
            .into_iter()
            .collect::<BTreeSet<_>>();
        let candidates = current
            .difference(&self.baseline)
            .copied()
            .chain(correlated.iter().copied())
            .collect::<BTreeSet<_>>();
        let replays = candidates.into_iter().take(8).map(|id| {
            let record = ctx.record_get_json(RecordKindV1::MatchReplay, id, "")
                .and_then(|s| serde_json::from_str::<Value>(&s).ok());
                json!({"replay_id": id, "referenced_by_same_id_record": correlated.contains(&id), "record": record})
        }).collect::<Vec<_>>();
        let player = self.last_player.as_ref().filter(|p| p.key == key).map(|p| {
            json!({
                "player": p.player, "champion": p.champion, "level": p.level, "gold": p.gold,
                "kda": [p.kda.0,p.kda.1,p.kda.2], "cs": p.cs, "alive": p.alive,
                "note": "last original-worker sample retained before battlefield exit; not guaranteed to be terminal tick"
            })
        });
        let report = json!({"version": env!("CARGO_PKG_VERSION"), "session_generation": self.generation, "controlled_key": {
            "seed": key.0, "match_id": key.1, "set_index": key.2 },
            "last_selected_player": player, "result_screen_labels": self.result_screen,
            "matches": matches, "replays": replays,
            "authority": "UNVERIFIED: compare saved records with the played battle. Same numeric match id across categories and newly created replays are candidates, not proof of the controlled worker's result."});
        if self.last_report.as_ref() == Some(&report) {
            return;
        }
        let Some(root) = log.directory() else { return };
        let reports = root.join("research");
        if let Err(reason) = std::fs::create_dir_all(&reports) {
            log.write(&format!(
                "RESULT AUDIT diagnostic folder unavailable: {reason}"
            ));
            return;
        }
        let path = reports.join(format!(
            "session-result-{}-set{}-seed{}-run{}-{}.json",
            key.1,
            key.2,
            key.0,
            self.generation,
            env!("CARGO_PKG_VERSION")
        ));
        let written = serde_json::to_vec_pretty(&report)
            .ok()
            .and_then(|bytes| std::fs::write(&path, bytes).ok())
            .is_some();
        log.write(&format!("RESULT AUDIT file={} written={written} matching_tables={} replay_candidates={}; authority unverified",path.display(),matches.len(),replays.len()));
        if written {
            self.last_report = Some(report);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_simulation_of_a_fixture_keeps_its_own_latest_numbers() {
        let sample = |kills| {
            vec![RunPlayer {
                player: 7,
                side: 1,
                kda: (kills, 0, 0),
                cs: 0,
                level: 1,
                gold: 0,
            }]
        };
        // The pre-simulation reports another set index than the match on screen.
        observe_run((91, 612, u64::MAX), 1, 30, || sample(2));
        observe_run((91, 612, 1), 2, 30, || sample(5));
        observe_run((91, 612, 1), 2, 60, || sample(6));
        // A second caller on the same tick does not read the players again.
        observe_run((91, 612, 1), 2, 60, || panic!("sampled twice"));
        observe_run((92, 613, 1), 2, 30, || sample(9));
        let found = runs((91, 612, 1));
        let found = found.as_array().unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0]["origin_kind"], 1);
        assert_eq!(found[0]["players"][0]["kda"][0], 2);
        assert_eq!(found[1]["last_sampled_tick"], 60);
        assert_eq!(found[1]["players"][0]["kda"][0], 6);
    }
    #[test]
    fn history_references_are_not_inferred_from_unrelated_fields_or_set_indices() {
        assert_eq!(
            replay_ids(&json!({"replays":[7,9,"11",null,-1],"winner":3})),
            BTreeSet::from([7, 9])
        );
        assert!(replay_ids(&json!({"set_index":7,"replay_id":9})).is_empty());
        assert!(replay_ids(&Value::Null).is_empty());
    }
    #[test]
    fn last_battle_sample_freezes_at_exit_and_rejects_other_match_data() {
        let mut audit = ResultAudit::default();
        let player = Snapshot {
            key: (1, 2, 3),
            player: 7,
            champion: "lancer".into(),
            level: 5,
            hp: None,
            alive: false,
            respawn: 120,
            gold: 100,
            kda: (1, 2, 3),
            cs: 30,
            cooldowns: [0; 3],
            items: vec![],
            build: None,
        };
        audit.observe_player((1, 2, 3), true, Some(&player));
        let mut replay = player.clone();
        replay.kda = (0, 0, 0);
        audit.observe_player((1, 2, 3), false, Some(&replay));
        assert_eq!(audit.last_player.as_ref().unwrap().kda, (1, 2, 3));
        replay.key = (1, 2, 4);
        audit.observe_player((1, 2, 3), true, Some(&replay));
        assert_eq!(audit.last_player.as_ref().unwrap().key, (1, 2, 3));
    }
}
