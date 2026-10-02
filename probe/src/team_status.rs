//! Ten player status cards from the bound live worker, never replay candidates.
use crate::{camera::Rect, own_selection::MatchKey, Logger};
use mod_api_stable::StableClient;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

const PATH: &str = "ingame.lt_team_status";
const WIDTH: usize = 1080;
const GROUP: usize = 224;
const STEP: usize = 46;

#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    pub id: usize,
    pub side: usize,
    pub lane: usize,
    pub champion: String,
    pub alive: Option<bool>,
    pub respawn: usize,
}
#[derive(Default)]
struct State {
    key: Option<MatchKey>,
    sampled: Option<usize>,
    at: Option<Instant>,
    players: Vec<Player>,
}
#[derive(Default)]
pub struct TeamStatus(Mutex<State>);
impl TeamStatus {
    pub fn reset_session(&self) {
        if let Ok(mut s) = self.0.lock() {
            *s = State::default();
        }
    }
    pub fn needs_sample(&self, key: MatchKey, tick: usize) -> bool {
        self.0.lock().is_ok_and(|mut s| {
            if s.key != Some(key) {
                *s = State {
                    key: Some(key),
                    ..State::default()
                };
            }
            if s.sampled == Some(tick) || (s.at.is_some() && !tick.is_multiple_of(6)) {
                return false;
            }
            s.sampled = Some(tick);
            true
        })
    }
    pub fn observe(&self, key: MatchKey, mut players: Vec<Player>, log: &Logger) {
        let Ok(mut s) = self.0.lock() else { return };
        if s.key != Some(key) {
            return;
        }
        for p in &mut players {
            if let Some(old) = s
                .players
                .iter()
                .find(|old| old.id == p.id && old.side == p.side && old.lane == p.lane)
            {
                if p.champion.is_empty() {
                    p.champion.clone_from(&old.champion);
                }
                if old.alive != p.alive {
                    log.write(&format!("TEAM STATUS key={key:?} player={} side={} lane={} alive={:?} respawn_ticks={}", p.id, p.side, p.lane, p.alive, p.respawn));
                }
            }
        }
        s.players = players;
        s.at = Some(Instant::now());
    }
    pub fn snapshot(&self, key: Option<MatchKey>, running: bool) -> Vec<Player> {
        let Ok(s) = self.0.lock() else {
            return Vec::new();
        };
        if key.is_none() || s.key != key {
            return Vec::new();
        }
        let mut players = s.players.clone();
        if running
            && s.at
                .is_none_or(|at| at.elapsed() > Duration::from_millis(250))
        {
            // All actors may stop calling think. Preserve dead identities but
            // hide stale countdowns; do not advance native timers by wall time.
            for p in &mut players {
                if p.alive != Some(false) {
                    p.alive = None;
                }
                p.respawn = 0;
            }
        }
        players
    }
}

fn slot(players: &[Player], side: usize, lane: usize) -> Option<&Player> {
    let mut matches = players.iter().filter(|p| p.side == side && p.lane == lane);
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}
fn caption(p: &Player) -> String {
    match p.alive {
        Some(true) => String::new(),
        Some(false) if p.respawn > 0 => p.respawn.div_ceil(60).to_string(),
        Some(false) => "—".into(),
        None => "?".into(),
    }
}
fn template() -> String {
    let mut source = format!("lt_team_status:empty {{ width: {WIDTH}px; height: 40px; anchor_x: 0.5; pivot_x: 0.5; y: 64px; ignore_event: true;\n");
    for side in 0..2 {
        let x = if side == 0 { 0 } else { WIDTH - GROUP };
        source.push_str(&format!(
            "#side{side}:empty {{ x: {x}px; width: {GROUP}px; height: 40px; ignore_event: true;\n"
        ));
        for lane in 0..5 {
            source.push_str(&format!("#lane{lane}:color {{ x: {}px; width: 40px; height: 40px; z: 1001; color: #555555ff; ignore_event: true; rounding: Uniform {{ rounding: 5; }} #background:color {{ x: 2px; y: 2px; width: 36px; height: 36px; z: 1002; color: #141414ff; ignore_event: true; }} #portrait:image {{ x: 3px; y: 3px; width: 34px; height: 34px; z: 1003; sample_linear: false; ignore_event: true; visible: false; }} #shade:color {{ x: 2px; y: 2px; width: 36px; height: 36px; z: 1004; color: #141414cc; ignore_event: true; visible: false; }} #timer:label {{ @\"asset/base/style/main#bold_label\"; width: 100%; height: 100%; z: 1005; size: 16; align_x: Center; align_y: Center; color: #e8e8e8ff; outline: 1; outline_color: #000000ff; ignore_event: true; }} }}\n",lane * STEP));
        }
        source.push_str("}\n");
    }
    source.push('}');
    source
}
#[derive(Default)]
pub struct TeamUi {
    cache: HashMap<String, String>,
    portraits: HashMap<String, String>,
    attempted: HashMap<String, (String, Instant)>,
    spawn_at: Option<Instant>,
}
impl TeamUi {
    fn set(&mut self, ctx: &mut StableClient<'_>, path: &str, value: String, text: bool) {
        let key = format!("{path}:{text}");
        if self.cache.get(&key) == Some(&value) {
            return;
        }
        let success = if text {
            ctx.ui_set_text(path, &value)
        } else {
            ctx.ui_set_properties(path, &value)
        };
        if success {
            self.cache.insert(key, value);
        }
    }
    pub fn apply(
        &mut self,
        ctx: &mut StableClient<'_>,
        active: bool,
        players: &[Player],
        selected: Option<usize>,
        log: &Logger,
    ) -> Vec<Rect> {
        if !active {
            if ctx.ui_exists(PATH) {
                ctx.ui_set_visible(PATH, false);
            }
            self.spawn_at = None;
            return Vec::new();
        }
        if !ctx.ui_exists(PATH) {
            if self
                .spawn_at
                .is_some_and(|t| t.elapsed() < Duration::from_secs(1))
            {
                return Vec::new();
            }
            self.spawn_at = Some(Instant::now());
            self.cache.clear();
            self.portraits.clear();
            self.attempted.clear();
            let spawned = ctx.ui_spawn_source("ingame", &template());
            log.write(&format!(
                "TEAM STATUS UI spawn={spawned} exists={}",
                ctx.ui_exists(PATH)
            ));
            if !spawned || !ctx.ui_exists(PATH) {
                return Vec::new();
            }
        }
        ctx.ui_set_visible(PATH, true);
        for side in 0..2 {
            for lane in 0..5 {
                let path = format!("{PATH}.side{side}.lane{lane}");
                let Some(p) = slot(players, side, lane) else {
                    self.set(ctx, &path, "visible: false;".into(), false);
                    continue;
                };
                let color = if selected == Some(p.id) {
                    "ffd700ff"
                } else {
                    "555555ff"
                };
                self.set(
                    ctx,
                    &path,
                    format!("visible: true; color: #{color};"),
                    false,
                );
                let portrait = format!("{path}.portrait");
                if !p.champion.is_empty()
                    && self.portraits.get(&portrait) != Some(&p.champion)
                    && self.attempted.get(&portrait).is_none_or(|(name, at)| {
                        name != &p.champion || at.elapsed() >= Duration::from_secs(1)
                    })
                {
                    self.attempted
                        .insert(portrait.clone(), (p.champion.clone(), Instant::now()));
                    let applied = ctx.ui_set_champion_icon(&portrait, &p.champion, 34., 34., 2.);
                    log.write(&format!(
                        "TEAM STATUS portrait player={} champion={} applied={applied}",
                        p.id, p.champion
                    ));
                    if applied {
                        self.portraits.insert(portrait.clone(), p.champion.clone());
                    }
                }
                self.set(
                    ctx,
                    &portrait,
                    format!(
                        "visible: {};",
                        !p.champion.is_empty()
                            && self.portraits.get(&portrait) == Some(&p.champion)
                    ),
                    false,
                );
                self.set(
                    ctx,
                    &format!("{path}.shade"),
                    format!("visible: {};", p.alive != Some(true)),
                    false,
                );
                self.set(ctx, &format!("{path}.timer"), caption(p), true);
            }
        }
        (0..2)
            .filter_map(|side| {
                ctx.ui_node_rect(&format!("{PATH}.side{side}"))
                    .filter(|(_, _, w, h)| *w > 0. && *h > 0.)
                    .or_else(|| {
                        ctx.ui_node_rect("ingame").map(|(x, y, w, _)| {
                            (
                                x + (w - WIDTH as f32) / 2.
                                    + if side == 0 {
                                        0.
                                    } else {
                                        (WIDTH - GROUP) as f32
                                    },
                                y + 64.,
                                GROUP as f32,
                                40.,
                            )
                        })
                    })
            })
            .map(|(x, y, w, h)| Rect { x, y, w, h })
            .filter(|r| r.valid())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn player(side: usize, lane: usize, alive: bool, respawn: usize) -> Player {
        Player {
            id: side * 5 + lane,
            side,
            lane,
            champion: "harpy".into(),
            alive: Some(alive),
            respawn,
        }
    }
    #[test]
    fn dead_identity_survives_absent_entity_and_native_timer_freezes_on_pause() {
        let team = TeamStatus::default();
        let key = (1, 33, 1);
        let log = crate::timing_test::tests::logger("team-death");
        assert!(team.needs_sample(key, 1));
        team.observe(key, vec![player(1, 2, true, 0)], &log);
        let mut dead = player(1, 2, false, 121);
        dead.champion.clear();
        team.observe(key, vec![dead], &log);
        team.0.lock().unwrap().at = Some(Instant::now() - Duration::from_secs(2));
        let paused = team.snapshot(Some(key), false);
        assert_eq!(paused[0].champion, "harpy");
        assert_eq!(caption(&paused[0]), "3");
        assert_eq!(caption(&team.snapshot(Some(key), true)[0]), "—");
        team.observe(key, vec![player(1, 2, true, 0)], &log);
        assert_eq!(caption(&team.snapshot(Some(key), true)[0]), "");
    }
    #[test]
    fn sides_lanes_and_new_session_do_not_reuse_old_identity_or_sampling() {
        let team = TeamStatus::default();
        let key = (1, 33, 1);
        let log = crate::timing_test::tests::logger("team-session");
        assert!(team.needs_sample(key, 1));
        assert!(!team.needs_sample(key, 1));
        let players = vec![player(1, 0, false, 60), player(0, 0, true, 0)];
        team.observe(key, players.clone(), &log);
        assert!(!team.needs_sample(key, 2));
        assert!(team.needs_sample(key, 6));
        assert!(!team.needs_sample(key, 6));
        assert_eq!(slot(&players, 0, 0).unwrap().id, 0);
        assert_eq!(slot(&players, 1, 0).unwrap().id, 5);
        assert!(team.snapshot(Some((2, 33, 2)), false).is_empty());
        team.reset_session();
        assert!(team.snapshot(Some(key), false).is_empty());
        assert!(team.needs_sample(key, 1));
        let mut new = player(0, 0, false, 60);
        new.champion.clear();
        team.observe(key, vec![new], &log);
        assert!(team.snapshot(Some(key), false)[0].champion.is_empty());
        let mut ambiguous = players.clone();
        ambiguous.push(player(0, 0, true, 0));
        assert!(slot(&ambiguous, 0, 0).is_none());
    }
}
