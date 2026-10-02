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

pub fn contract_team(json: &str) -> Option<usize> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let contract = value
        .get("InContract")
        .or_else(|| value.get("in_contract"))
        .unwrap_or(&value);
    usize::try_from(contract.get("team_id")?.as_u64()?).ok()
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
}
