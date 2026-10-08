//! Read-only evidence for the full-battle test; never edits match history.
use crate::{native_timing::MatchKey, player_hud::Snapshot, Logger};
use mod_api_stable::{ClientSceneKindV1, RecordKindV1, StableClient};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

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
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        ctx: &StableClient<'_>,
        key: Option<MatchKey>,
        generation: u64,
        battlefield: bool,
        last_player: Option<&Snapshot>,
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
        }
        let key = audit_key;
        self.observe_player(key, battlefield, last_player);
        if self.was_battlefield && !battlefield {
            self.deadline = Some(Instant::now() + Duration::from_secs(30));
            log.write("RESULT AUDIT battlefield exit; read-only history capture pending (authority unverified)");
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
