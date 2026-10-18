//! Select by human-team athlete identity, never by blue/red ordering.
use std::collections::{BTreeMap, BTreeSet};
pub type MatchKey = (u64, u64, u64);

#[derive(Clone)]
pub struct PlayerIdentity {
    pub player: usize,
    pub athlete: usize,
    pub lane: usize,
    pub side: usize,
    pub champion: String,
}

#[derive(Default)]
pub struct OwnSelection {
    pub lane: usize,
    pub own_team: Option<usize>,
    pub team_name: String,
    pub athletes: BTreeSet<usize>,
    pub match_key: Option<MatchKey>,
    players: BTreeMap<usize, PlayerIdentity>,
}

impl OwnSelection {
    /// Cache only a complete ownership snapshot. Title/loading callbacks can
    /// expose placeholder team zero with no roster; later reads must retry.
    pub fn resolve_owner(&mut self, team: usize, name: String, athletes: BTreeSet<usize>) -> bool {
        if self.own_team.is_some() || name.is_empty() || athletes.is_empty() {
            return false;
        }
        self.own_team = Some(team);
        self.team_name = name;
        self.athletes = athletes;
        true
    }
    pub fn begin(&mut self, key: MatchKey) -> bool {
        if self.match_key.is_some() {
            return false;
        }
        self.match_key = Some(key);
        self.players.clear();
        true
    }
    pub fn register(&mut self, key: MatchKey, player: PlayerIdentity) {
        if self.match_key == Some(key) {
            self.players.insert(player.player, player);
        }
    }
    pub fn selected(&self) -> Option<&PlayerIdentity> {
        let mut candidates = self
            .players
            .values()
            .filter(|player| self.athletes.contains(&player.athlete) && player.lane == self.lane);
        let first = candidates.next()?;
        candidates.next().is_none().then_some(first)
    }
    pub fn choose(&mut self, lane: usize) -> bool {
        if lane >= 5 || self.match_key.is_some() {
            return false;
        }
        self.lane = lane;
        true
    }
    /// Caller must hold the client session gate and confirm READY. Only a
    /// unique own-team identity in this exact held battle may be rebound.
    pub fn choose_prepared(&mut self, key: MatchKey, lane: usize) -> bool {
        if self.match_key != Some(key) || lane >= 5 || self.lane == lane {
            return false;
        }
        let old = self.lane;
        self.lane = lane;
        if self.selected().is_none() {
            self.lane = old;
            return false;
        }
        true
    }
    pub fn own_players(&self) -> Vec<PlayerIdentity> {
        (0..5)
            .filter_map(|lane| {
                let mut matches = self
                    .players
                    .values()
                    .filter(|p| p.lane == lane && self.athletes.contains(&p.athlete));
                let first = matches.next()?;
                matches.next().is_none().then(|| first.clone())
            })
            .collect()
    }
}

/// The team an athlete's contract record names: `team_id` inside
/// `InContract` (or `in_contract`), else at the top level. Only that field
/// is read; every other value is skipped without building a JSON tree, as
/// this runs for every athlete in the save
/// (docs/investigation-takeover-cost-2026-10-18.md, section 2).
pub fn contract_team(json: &str) -> Option<usize> {
    let contract: ContractFields = serde_json::from_str(json).ok()?;
    let team_id = match (contract.upper, contract.snake) {
        (Some(inner), _) | (None, Some(inner)) => inner,
        (None, None) => contract.team_id,
    };
    usize::try_from(team_id?).ok()
}

/// The fields `contract_team` reads; a nested contract object is read the
/// same way, and only its `team_id` is used.
#[derive(Default)]
struct ContractFields {
    upper: Option<Option<u64>>,
    snake: Option<Option<u64>>,
    team_id: Option<u64>,
}
impl<'de> serde::Deserialize<'de> for ContractFields {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Fields;
        impl<'de> serde::de::Visitor<'de> for Fields {
            type Value = ContractFields;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a contract object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<ContractFields, A::Error> {
                let mut fields = ContractFields::default();
                while let Some(key) = map.next_key::<Key>()? {
                    match key {
                        Key::Upper => {
                            fields.upper = Some(map.next_value::<ContractFields>()?.team_id)
                        }
                        Key::Snake => {
                            fields.snake = Some(map.next_value::<ContractFields>()?.team_id)
                        }
                        Key::TeamId => fields.team_id = map.next_value::<Id>()?.0,
                        Key::Other => {
                            map.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(fields)
            }
        }
        d.deserialize_map(Fields)
    }
}
/// The object keys `contract_team` looks at, matched without allocating.
enum Key {
    Upper,
    Snake,
    TeamId,
    Other,
}
impl<'de> serde::Deserialize<'de> for Key {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Name;
        impl serde::de::Visitor<'_> for Name {
            type Value = Key;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a key")
            }
            fn visit_str<E>(self, key: &str) -> Result<Key, E> {
                Ok(match key {
                    "InContract" => Key::Upper,
                    "in_contract" => Key::Snake,
                    "team_id" => Key::TeamId,
                    _ => Key::Other,
                })
            }
        }
        d.deserialize_identifier(Name)
    }
}
/// A value read as an unsigned id; anything else (negative, fractional,
/// text, null, a list, an object) is None, as `Value::as_u64` gives.
struct Id(Option<u64>);
impl<'de> serde::Deserialize<'de> for Id {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Number;
        impl<'de> serde::de::Visitor<'de> for Number {
            type Value = Id;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an id")
            }
            fn visit_u64<E>(self, v: u64) -> Result<Id, E> {
                Ok(Id(Some(v)))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Id, E> {
                Ok(Id(u64::try_from(v).ok()))
            }
            fn visit_f64<E>(self, _: f64) -> Result<Id, E> {
                Ok(Id(None))
            }
            fn visit_bool<E>(self, _: bool) -> Result<Id, E> {
                Ok(Id(None))
            }
            fn visit_str<E>(self, _: &str) -> Result<Id, E> {
                Ok(Id(None))
            }
            fn visit_unit<E>(self) -> Result<Id, E> {
                Ok(Id(None))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Id, A::Error> {
                while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                Ok(Id(None))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Id, A::Error> {
                while map
                    .next_entry::<serde::de::IgnoredAny, serde::de::IgnoredAny>()?
                    .is_some()
                {}
                Ok(Id(None))
            }
        }
        d.deserialize_any(Number)
    }
}

/// Contracts read per client frame while finding your team's athletes.
pub const SCAN_CHUNK: usize = 50;
/// Finds which athletes of the save are on your team, reading at most
/// `SCAN_CHUNK` contracts per frame and keeping the partial result between
/// frames. It starts over when your team or the save's athlete list changes.
#[derive(Default)]
pub struct TeamScan {
    team: Option<usize>,
    ids: Vec<usize>,
    next: usize,
    found: BTreeSet<usize>,
}
impl TeamScan {
    /// One frame of the scan. `team_of` reads one athlete's contract team.
    /// Returns your team's athletes once every contract has been read
    /// (possibly none, while the save's data is not ready); the call after
    /// that starts a new scan.
    pub fn step(
        &mut self,
        team: usize,
        ids: &[usize],
        mut team_of: impl FnMut(usize) -> Option<usize>,
    ) -> Option<BTreeSet<usize>> {
        if self.team != Some(team) || self.ids != ids {
            *self = Self {
                team: Some(team),
                ids: ids.to_vec(),
                ..Self::default()
            };
        }
        let end = (self.next + SCAN_CHUNK).min(self.ids.len());
        for &id in &self.ids[self.next..end] {
            if team_of(id) == Some(team) {
                self.found.insert(id);
            }
        }
        self.next = end;
        if self.next < self.ids.len() {
            return None;
        }
        self.next = 0;
        Some(std::mem::take(&mut self.found))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prepared_choices_require_unique_own_players_and_the_current_match() {
        for side in [0, 1] {
            let mut s = OwnSelection::default();
            s.athletes.extend([60, 61]);
            s.begin((1, 33, 1));
            for (lane, athlete) in [(0, 60), (3, 61), (4, 22)] {
                s.register(
                    (1, 33, 1),
                    PlayerIdentity {
                        player: side * 5 + lane,
                        athlete,
                        lane,
                        side,
                        champion: "lancer".into(),
                    },
                );
            }
            assert_eq!(s.own_players().len(), 2);
            assert!(!s.choose_prepared((2, 33, 2), 3));
            assert!(!s.choose_prepared((1, 33, 1), 4));
            assert!(s.choose_prepared((1, 33, 1), 3));
            assert_eq!(s.selected().unwrap().athlete, 61);
            assert!(!s.choose(0)); // Ordinary in-match choices remain locked.
            s.register(
                (1, 33, 1),
                PlayerIdentity {
                    player: 99,
                    athlete: 61,
                    lane: 0,
                    side,
                    champion: "hunter".into(),
                },
            );
            assert!(!s.choose_prepared((1, 33, 1), 0));
            assert_eq!(s.lane, 3);
        }
    }
    #[test]
    fn placeholder_ownership_does_not_block_a_later_loaded_roster() {
        let mut selection = OwnSelection::default();
        assert!(!selection.resolve_owner(0, "team 0".into(), BTreeSet::new()));
        assert!(selection.own_team.is_none());
        assert!(selection.resolve_owner(7, "Cute and Stunny".into(), [54, 55].into()));
        assert_eq!(selection.own_team, Some(7));
        assert!(selection.athletes.contains(&55));
        assert!(!selection.resolve_owner(3, "opponent".into(), [23].into()));
        assert_eq!(selection.own_team, Some(7));
    }
    #[test]
    fn own_top_is_identified_on_either_side_from_athlete_identity() {
        for (side, player) in [(0, 0), (1, 5)] {
            let mut selection = OwnSelection::default();
            selection.athletes.insert(60);
            selection.begin((1, 33, 1));
            selection.register(
                (1, 33, 1),
                PlayerIdentity {
                    player: 5 - player,
                    athlete: 23,
                    lane: 0,
                    side: 1 - side,
                    champion: "enemy".into(),
                },
            );
            selection.register(
                (1, 33, 1),
                PlayerIdentity {
                    player,
                    athlete: 60,
                    lane: 0,
                    side,
                    champion: "own".into(),
                },
            );
            assert_eq!(selection.selected().unwrap().player, player);
            assert!(!selection.choose(4));
        }
    }
    #[test]
    fn missing_ambiguous_and_other_match_identities_cannot_select_an_enemy() {
        let mut selection = OwnSelection::default();
        selection.begin((1, 33, 1));
        let player = PlayerIdentity {
            player: 0,
            athlete: 60,
            lane: 0,
            side: 0,
            champion: "own".into(),
        };
        selection.register((1, 33, 1), player.clone());
        assert!(selection.selected().is_none());
        selection.athletes.insert(60);
        assert!(selection.selected().is_some());
        selection.register(
            (2, 33, 1),
            PlayerIdentity {
                player: 5,
                ..player.clone()
            },
        );
        assert_eq!(selection.selected().unwrap().player, 0);
        selection.register(
            (1, 33, 1),
            PlayerIdentity {
                player: 5,
                ..player
            },
        );
        assert!(selection.selected().is_none());
    }
    #[test]
    fn contract_ownership_uses_team_id_and_rejects_free_agents() {
        assert_eq!(
            contract_team(r#"{"InContract":{"team_id":4,"weekly_salary":300}}"#),
            Some(4)
        );
        assert_eq!(contract_team(r#"{"team_id":4}"#), Some(4));
        assert_eq!(
            contract_team(r#"{"FreeAgent":{"previous_team_id":4}}"#),
            None
        );
        assert_eq!(contract_team(r#"{"InContract":{"team_id":-1}}"#), None);
    }
    #[test]
    fn contract_team_reads_only_team_id_as_the_full_parse_did() {
        // InContract wins over in_contract and over a top-level team_id.
        assert_eq!(
            contract_team(
                r#"{"team_id":9,"in_contract":{"team_id":5},"InContract":{"team_id":4}}"#
            ),
            Some(4)
        );
        assert_eq!(
            contract_team(r#"{"team_id":9,"in_contract":{"team_id":5}}"#),
            Some(5)
        );
        // A contract without team_id names no team, even with one outside it.
        assert_eq!(
            contract_team(r#"{"team_id":9,"InContract":{"salary":1}}"#),
            None
        );
        // Other values of any shape are skipped.
        assert_eq!(
            contract_team(
                r#"{"x":[1,{"team_id":7}],"y":null,"InContract":{"perks":{"a":"b"},"team_id":12}}"#
            ),
            Some(12)
        );
        for not_an_id in ["1.5", "\"4\"", "null", "true", "[4]", "{\"id\":4}"] {
            let json = format!(r#"{{"InContract":{{"team_id":{not_an_id}}}}}"#);
            assert_eq!(contract_team(&json), None, "{json}");
        }
        for broken in ["", "4", "\"FreeAgent\"", "{", "[]"] {
            assert_eq!(contract_team(broken), None, "{broken}");
        }
    }
    #[test]
    fn the_team_scan_reads_50_contracts_a_frame_and_restarts_on_a_change() {
        let ids: Vec<usize> = (0..120).collect();
        let on_team = |id: usize| id.is_multiple_of(40).then_some(7);
        let mut scan = TeamScan::default();
        let mut reads = 0;
        let mut frame = |scan: &mut TeamScan| {
            scan.step(7, &ids, |id| {
                reads += 1;
                on_team(id)
            })
        };
        assert_eq!(frame(&mut scan), None);
        assert_eq!(frame(&mut scan), None);
        assert_eq!(frame(&mut scan), Some(BTreeSet::from([0, 40, 80])));
        assert_eq!(reads, 120);
        // A different team or save starts over from the first athlete.
        let mut scan = TeamScan::default();
        assert_eq!(scan.step(7, &ids, on_team), None);
        assert_eq!(scan.step(8, &ids, on_team), None);
        assert_eq!(scan.next, SCAN_CHUNK);
        let fewer = &ids[..60];
        assert_eq!(scan.step(8, fewer, on_team), None);
        assert_eq!(scan.next, SCAN_CHUNK);
        assert_eq!(scan.step(8, fewer, on_team), Some(BTreeSet::new()));
        // Done: the next call starts a new scan.
        assert_eq!(scan.step(8, fewer, on_team), None);
        assert_eq!(scan.next, SCAN_CHUNK);
    }
}
