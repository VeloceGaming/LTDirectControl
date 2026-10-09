//! Ten player status cards from the bound live worker, never replay candidates.
use crate::{own_selection::MatchKey, Logger};
use mod_api_stable::StableClient;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

pub const PATH: &str = "ingame.lt_team_status";

#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    pub id: usize,
    pub side: usize,
    pub lane: usize,
    pub champion: String,
    pub alive: Option<bool>,
    pub respawn: usize,
    pub level: usize,
    pub gold: usize,
    pub kda: (usize, usize, usize),
    pub cs: usize,
    pub items: Vec<String>,
    pub build: Option<Vec<String>>,
    pub fresh: bool,
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
                if p.build.is_none() {
                    p.build.clone_from(&old.build);
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
                p.fresh = false;
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Geometry {
    columns: usize,
    rows: usize,
    row_height: usize,
    side_width: usize,
    width: usize,
    height: usize,
}
fn geometry(count: usize) -> Geometry {
    let columns = count.min(crate::inventory::TAB_COLUMNS);
    let rows = count.div_ceil(crate::inventory::TAB_COLUMNS).max(1);
    let row_height = (rows * 36 + 16).max(68);
    let side_width = 439 + columns * 36;
    Geometry {
        columns,
        rows,
        row_height,
        side_width,
        width: 40 + 26 + side_width * 2,
        height: 32 + 34 + row_height * 5,
    }
}
fn label(s: &mut String, id: &str, x: usize, w: usize, size: usize, color: &str, right: bool) {
    s.push_str(&format!("#{id}:label {{ x: {x}px; width: {w}px; height: 100%; z: 1204; font: \"asset/lt_direct_control/font/numeric\"; size: {size}; color: #{color}; align_y: Center; align_x: {}; ignore_event: true; }}\n", if right { "Right" } else { "Left" }));
}
fn item_tile(i: usize) -> String {
    // Insets: background + 8.
    let inset = crate::ui_theme::shade(8, 0xff);
    format!("item{i}:color {{ width: 32px; height: 32px; z: 1203; color: #~4b4a49ff; rounding: Uniform {{ rounding: 2; }} ignore_event: true; #bg:color {{ x: 1px; y: 1px; width: 30px; height: 30px; z: 1204; color: #{inset}; ignore_event: true; }} #icon:image {{ x: 1px; y: 1px; width: 30px; height: 30px; z: 1205; sample_linear: false; visible: false; ignore_event: true; }} #unknown:label {{ font: \"asset/lt_direct_control/font/numeric\"; width: 100%; height: 100%; z: 1206; size: 18; text: \"?\"; align_x: Center; align_y: Center; color: #eeececff; visible: false; ignore_event: true; }} }}")
}
#[cfg(test)]
pub(crate) fn template_for_test() -> String {
    template()
}
fn template() -> String {
    let inset = crate::ui_theme::shade(8, 0xff);
    let mut s = String::from("lt_team_status:color { anchor_x: 0.5; pivot_x: 0.5; anchor_y: 0.5; pivot_y: 0.5; y: -80px; z: 1200; color: #~1e1e1df5; rounding: Uniform { rounding: 2; } ignore_event: true;\n#top:color { height: 1px; width: 100%; z: 1201; color: #~4b4a49ff; ignore_event: true; }\n#bottom:color { anchor_y: 1; pivot_y: 1; height: 1px; width: 100%; z: 1201; color: #~4b4a49ff; ignore_event: true; }\n");
    s.push_str("#left:color { width: 1px; height: 100%; z: 1201; color: #~4b4a49ff; ignore_event: true; }\n#right:color { anchor_x: 1; pivot_x: 1; width: 1px; height: 100%; z: 1201; color: #~4b4a49ff; ignore_event: true; }\n");
    for side in 0..2 {
        s.push_str(&format!("#side{side}:empty {{ y: 16px; ignore_event: true;\n#headers:empty {{ width: 100%; height: 34px; ignore_event: true;\n"));
        for (id, x, w) in [
            ("level", 64, 34),
            ("kda", 104, 122),
            ("cs", 234, 54),
            ("items", 294, 220),
            ("gold", 555, 88),
        ] {
            label(&mut s, id, x, w, 13, "989694ff", id == "gold");
            // Header text is set once after spawning, using the host text API.
        }
        s.push_str("}\n");
        for lane in 0..5 {
            s.push_str(&format!("#lane{lane}:color {{ color: #00000000; ignore_event: true; z: 1201;\n#selected:color {{ width: 2px; height: 100%; z: 1202; color: #fdee00ff; visible: false; ignore_event: true; }}\n#line:color {{ anchor_y: 1; pivot_y: 1; width: 100%; height: 1px; z: 1202; color: #~4b4a4973; ignore_event: true; }}\n#portrait:color {{ x: 6px; y: 8px; width: 48px; height: 52px; z: 1203; color: #~4b4a49ff; ignore_event: true; rounding: Uniform {{ rounding: 2; }} #bg:color {{ x: 1px; y: 1px; width: 46px; height: 50px; z: 1204; color: #{inset}; ignore_event: true; }} #icon:image {{ anchor_x: 0.5; pivot_x: 0.5; anchor_y: 0.5; pivot_y: 0.5; width: 44px; height: 48px; z: 1205; sample_linear: false; visible: false; ignore_event: true; }} #shade:color {{ width: 100%; height: 100%; z: 1206; color: #00000099; visible: false; ignore_event: true; }} #timer:label {{ font: \"asset/lt_direct_control/font/numeric\"; width: 100%; height: 100%; z: 1207; size: 25; align_x: Center; align_y: Center; color: #ffffffff; outline: 1; outline_color: #000000ff; ignore_event: true; }} }}\n"));
            for (id, x, w, size, color) in [
                ("level", 64, 34, 17, "cbc9c7ff"),
                ("kda", 104, 122, 22, "eeececff"),
                ("cs", 234, 54, 21, "eeececff"),
                ("gold", 555, 88, 21, "fdee00ff"),
            ] {
                label(&mut s, id, x, w, size, color, id == "gold");
            }
            for i in 0..6 {
                s.push('#');
                s.push_str(&crate::ui_theme::themed(&item_tile(i)));
                s.push('\n');
            }
            s.push_str("}\n");
        }
        s.push_str("}\n");
    }
    s.push('}');
    s
}
#[derive(Default)]
pub struct TeamUi {
    cache: HashMap<String, String>,
    spawn_at: Option<Instant>,
    allocated: usize,
    portraits: HashMap<String, (String, Instant, bool)>,
    layout: Option<Geometry>,
    failed: bool,
}
impl TeamUi {
    fn props(&mut self, ctx: &mut StableClient<'_>, path: &str, value: String, log: &Logger) {
        if !crate::hud_motion::properties(ctx, &mut self.cache, path, &value) && !self.failed {
            log.write(&format!("TAB property rejected path={path}"));
            self.failed = true;
        }
    }
    fn text(&mut self, ctx: &mut StableClient<'_>, path: &str, value: String) {
        let key = format!("text:{path}");
        if self.cache.get(&key) != Some(&value) && ctx.ui_set_text(path, &value) {
            self.cache.insert(key, value);
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &mut self,
        ctx: &mut StableClient<'_>,
        active: bool,
        players: &[Player],
        selected: Option<usize>,
        artwork: &crate::player_hud::HudUi,
        log: &Logger,
    ) {
        if !active {
            if ctx.ui_exists(PATH) {
                self.props(ctx, PATH, "visible: false;".into(), log);
            }
            self.spawn_at = None;
            return;
        }
        if !ctx.ui_exists(PATH) {
            if self
                .spawn_at
                .is_some_and(|at| at.elapsed() < Duration::from_secs(1))
            {
                return;
            }
            self.spawn_at = Some(Instant::now());
            let ok = ctx.ui_spawn_source("ingame", &crate::ui_theme::themed(&template()));
            log.write(&format!(
                "TAB styled panel spawn={ok} exists={}",
                ctx.ui_exists(PATH)
            ));
            if !ok || !ctx.ui_exists(PATH) {
                return;
            }
            self.cache.clear();
            self.portraits.clear();
            self.layout = None;
            self.allocated = 6;
            self.failed = false;
        }
        // The game buyer iterates this target vector; catalogue length does not
        // represent capacity. All rows share a column width, even for mixed plans.
        let count = players
            .iter()
            .map(|p| crate::inventory::slots(p.build.as_ref().map(Vec::len), p.items.len(), None))
            .max()
            .unwrap_or(0);
        for i in self.allocated..count {
            let mut ok = true;
            for side in 0..2 {
                for lane in 0..5 {
                    ok &= ctx.ui_spawn_source(
                        &format!("{PATH}.side{side}.lane{lane}"),
                        &crate::ui_theme::themed(&item_tile(i)),
                    );
                }
            }
            if !ok {
                log.write(&format!("TAB additional item node rejected index={i}"));
                break;
            }
            self.allocated = i + 1;
        }
        let g = geometry(count);
        if self.layout != Some(g) {
            log.write(&format!("TAB layout slots={count} geometry={g:?}"));
            self.layout = Some(g);
        }
        self.props(
            ctx,
            PATH,
            format!(
                "visible: true; width: {}px; height: {}px;",
                g.width, g.height
            ),
            log,
        );
        for side in 0..2 {
            let side_path = format!("{PATH}.side{side}");
            self.props(
                ctx,
                &side_path,
                format!(
                    "x: {}px; width: {}px; height: {}px;",
                    20 + side * (g.side_width + 26),
                    g.side_width,
                    g.height - 32
                ),
                log,
            );
            let gold_x = g.side_width - 100;
            for (id, text) in [
                ("level", "LV"),
                ("kda", "K / D / A"),
                ("cs", "CS"),
                ("items", crate::lang::tr("ITEMS")),
                ("gold", crate::lang::tr("GOLD")),
            ] {
                self.text(ctx, &format!("{side_path}.headers.{id}"), text.into());
            }
            self.props(
                ctx,
                &format!("{side_path}.headers.gold"),
                format!("x: {gold_x}px;"),
                log,
            );
            self.props(
                ctx,
                &format!("{side_path}.headers.items"),
                format!("visible: {};", count > 0),
                log,
            );
            for lane in 0..5 {
                let path = format!("{side_path}.lane{lane}");
                let Some(p) = slot(players, side, lane) else {
                    self.props(ctx, &path, "visible: false;".into(), log);
                    continue;
                };
                self.props(
                    ctx,
                    &path,
                    format!(
                        "visible: true; y: {}px; width: {}px; height: {}px;",
                        34 + lane * g.row_height,
                        g.side_width,
                        g.row_height
                    ),
                    log,
                );
                let mine = selected == Some(p.id);
                self.props(
                    ctx,
                    &format!("{path}.selected"),
                    format!("visible: {mine};"),
                    log,
                );
                self.props(
                    ctx,
                    &path,
                    format!("color: #{};", if mine { "fdee0009" } else { "00000000" }),
                    log,
                );
                let icon_path = format!("{path}.portrait.icon");
                let retry = self.portraits.get(&icon_path).is_none_or(|(name, at, ok)| {
                    name != &p.champion || (!ok && at.elapsed() >= Duration::from_secs(1))
                });
                if retry && !p.champion.is_empty() {
                    let ok = ctx.ui_set_champion_icon(&icon_path, &p.champion, 44., 48., 2.);
                    self.portraits
                        .insert(icon_path.clone(), (p.champion.clone(), Instant::now(), ok));
                }
                let visible = self
                    .portraits
                    .get(&icon_path)
                    .is_some_and(|(name, _, ok)| *ok && name == &p.champion);
                self.props(ctx, &icon_path, format!("visible: {visible};"), log);
                self.props(
                    ctx,
                    &format!("{path}.portrait"),
                    format!(
                        "color: #{};",
                        if side == 0 { "536979ff" } else { "79535dff" }
                    ),
                    log,
                );
                self.props(
                    ctx,
                    &format!("{path}.portrait.shade"),
                    format!("visible: {};", p.alive != Some(true)),
                    log,
                );
                self.text(ctx, &format!("{path}.portrait.timer"), caption(p));
                let number = |n: usize| if p.fresh { n.to_string() } else { "—".into() };
                self.text(ctx, &format!("{path}.level"), number(p.level));
                self.text(ctx, &format!("{path}.cs"), number(p.cs));
                self.text(ctx, &format!("{path}.gold"), number(p.gold));
                let (k, d, a) = p.kda;
                self.text(
                    ctx,
                    &format!("{path}.kda"),
                    if p.fresh {
                        kda_caption(k, d, a)
                    } else {
                        "—/—/—".into()
                    },
                );
                self.props(ctx, &format!("{path}.gold"), format!("x: {gold_x}px;"), log);
                let capacity =
                    crate::inventory::slots(p.build.as_ref().map(Vec::len), p.items.len(), None);
                for i in 0..self.allocated {
                    let item_path = format!("{path}.item{i}");
                    let visible = i < capacity;
                    self.props(
                        ctx,
                        &item_path,
                        format!(
                            "visible: {visible}; x: {}px; y: {}px;",
                            294 + i % crate::inventory::TAB_COLUMNS * 36,
                            18 + i / crate::inventory::TAB_COLUMNS * 36
                        ),
                        log,
                    );
                    if !visible {
                        continue;
                    }
                    let icon = p.items.get(i).and_then(|key| artwork.item_icon(key));
                    let props = icon.as_ref().map_or_else(
                        || "visible: false;".into(),
                        |icon| format!("visible: true; {}", icon.properties()),
                    );
                    self.props(ctx, &format!("{item_path}.icon"), props, log);
                    self.props(
                        ctx,
                        &format!("{item_path}.unknown"),
                        format!("visible: {};", p.items.get(i).is_some() && icon.is_none()),
                        log,
                    );
                }
            }
        }
        // No input-blocking rectangles: battlefield commands pass through Tab.
    }
}

fn kda_caption(kills: usize, deaths: usize, assists: usize) -> String {
    format!("<#eeececff>{kills}<#989694ff>/<#ff855bff>{deaths}<#989694ff>/<#eeececff>{assists}<>")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kda_uses_valid_native_rgba_runs_and_resets_colour() {
        let text = kda_caption(12, 3, 24);
        assert_eq!(crate::tooltips::rich(&text), text);
        assert_eq!(crate::tooltips::plain(&text), "12/3/24");
        assert!(text.ends_with("<>"));
    }
    fn player(side: usize, lane: usize, alive: bool, respawn: usize) -> Player {
        Player {
            id: side * 5 + lane,
            side,
            lane,
            champion: "harpy".into(),
            alive: Some(alive),
            respawn,
            level: 1,
            gold: 0,
            kda: (0, 0, 0),
            cs: 0,
            items: Vec::new(),
            build: None,
            fresh: true,
        }
    }
    #[test]
    fn adaptive_item_columns_stay_aligned_without_reducing_icon_size() {
        for (count, width) in [(4, 1232), (5, 1304), (6, 1376)] {
            let g = geometry(count);
            assert_eq!(g.width, width);
            assert_eq!(g.height, 406);
            assert_eq!(g.row_height, 68);
            assert!(294 + (count - 1) * 36 + 32 <= g.side_width - 100);
        }
        let g = geometry(12);
        assert_eq!(g.columns, 10);
        assert_eq!(g.row_height, 88);
        assert!(g.width < 1920);
        let source = template();
        assert!(!source.contains("Your team"));
        assert!(!source.contains("Opposing team"));
        assert!(source.contains("size: 22"));
    }
    #[test]
    fn missing_build_read_retains_only_same_match_capacity() {
        let team = TeamStatus::default();
        let key = (1, 33, 1);
        let log = crate::test_support::logger("team-capacity");
        assert!(team.needs_sample(key, 1));
        let mut first = player(0, 0, true, 0);
        first.build = Some(vec!["target".into(); 5]);
        team.observe(key, vec![first], &log);
        team.observe(key, vec![player(0, 0, true, 0)], &log);
        assert_eq!(
            team.snapshot(Some(key), false)[0]
                .build
                .as_ref()
                .unwrap()
                .len(),
            5
        );
        let next = (2, 34, 2);
        assert!(team.needs_sample(next, 1));
        team.observe(next, vec![player(0, 0, true, 0)], &log);
        assert!(team.snapshot(Some(next), false)[0].build.is_none());
    }
    #[test]
    fn dead_identity_survives_absent_entity_and_native_timer_freezes_on_pause() {
        let team = TeamStatus::default();
        let key = (1, 33, 1);
        let log = crate::test_support::logger("team-death");
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
        let log = crate::test_support::logger("team-session");
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
