//! Pixel-identity tests for the mod's per-frame windows (1.6.0-perf).
//!
//! Each test drives a fixed sequence of frames (control start, cooldowns
//! ticking, a purchase, a death and respawn, hover and press on a skill,
//! pause, AI control, the low-health tint, Tab open and closed) through a
//! window on the fake UI tree (`fake_ui.rs`) with a stopped clock, and
//! compares every write the game would receive, in order, with the record
//! saved from 1.5.3 under `probe/testdata/ui_identity/`. The record was
//! made by running these tests on the unchanged 1.5.3 window code; the
//! 1.6.0 code must reproduce it byte for byte.
//!
//! To write a new record after a deliberate change to what a window shows:
//!     LT_UPDATE_UI_RECORDS=1 cargo test --release ui_identity
//! and review the record's diff like any other change.
//!
//! 1.6.0-perf-dev3: the fake tree lays nodes out from their templates
//! (`fake_layout.rs`) instead of answering the whole screen, the cursor
//! points sit on the real controls, and the records note the hover and
//! click sound requests. The records were made again on the unchanged
//! 1.6.0-perf-dev2 windows with this rig, then shrunk to the nodes that stay
//! in the tree (CHANGELOG, 1.6.0-perf-dev3). `LT_UI_EXPORT_DIR=<folder>`
//! also writes each record with every template it spawned, for the pixel
//! comparison (`tools/compare_draw_review.cjs`).
use crate::{
    abilities::HudSkills,
    clock::test_clock,
    fake_ui,
    native_timing::Phase,
    player_hud::{self, Artwork, HudUi, PlayerHud, Snapshot, StatePlate},
};
use std::path::PathBuf;
use std::time::Duration;

const FRAME: Duration = Duration::from_micros(16_667);

fn record_path(folder: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(folder)
        .join(format!("{name}.txt"))
}
/// Compare `lines` with the saved write record `name` (or save it).
pub(crate) fn check_record(name: &str, lines: &[String]) {
    if let Some(dir) = std::env::var_os("LT_UI_EXPORT_DIR") {
        let dir = PathBuf::from(dir).join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("record.txt"), lines.join("\n") + "\n").unwrap();
        for (hash, source) in fake_ui::with(|u| u.sources.clone()) {
            std::fs::write(dir.join(format!("{hash}.ui")), source).unwrap();
        }
    }
    check_saved("ui_identity", name, lines);
}
/// A draw record with every frame that draws exactly what the one before
/// drew saved as one line, `= as before`.
fn compact(lines: &[String]) -> Vec<String> {
    let mut frames: Vec<(&String, Vec<&String>)> = Vec::new();
    for line in lines {
        match frames.last_mut() {
            _ if line.starts_with("## ") => frames.push((line, Vec::new())),
            Some((_, body)) => body.push(line),
            None => {}
        }
    }
    let mut out = Vec::new();
    for (k, (header, body)) in frames.iter().enumerate() {
        out.push((*header).clone());
        if k > 0 && !body.is_empty() && *body == frames[k - 1].1 {
            out.push("= as before".to_owned());
        } else {
            out.extend(body.iter().map(|l| (*l).clone()));
        }
    }
    out
}
/// Compare `lines` with the saved draw record `name` (1.6.0-perf-dev3),
/// made in the same run as the write record `window`.
pub(crate) fn check_draw_record(window: &str, name: &str, lines: &[String]) {
    let compact = compact(lines);
    let lines = &compact;
    if let Some(dir) = std::env::var_os("LT_UI_EXPORT_DIR") {
        let dir = PathBuf::from(dir).join(window);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("draw_{name}.txt")),
            lines.join("\n") + "\n",
        )
        .unwrap();
    }
    check_saved("ui_draw", name, lines);
}
fn check_saved(folder: &str, name: &str, lines: &[String]) {
    let path = record_path(folder, name);
    let text = lines.join("\n") + "\n";
    if std::env::var_os("LT_UPDATE_UI_RECORDS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &text).unwrap();
        return;
    }
    let saved = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("no saved record {}: {e}", path.display()));
    let saved = saved.replace("\r\n", "\n");
    if saved != text {
        let actual = path.with_extension("actual.txt");
        std::fs::write(&actual, &text).unwrap();
        let first = saved
            .lines()
            .zip(text.lines())
            .position(|(a, b)| a != b)
            .unwrap_or(saved.lines().count().min(text.lines().count()));
        panic!(
            "{folder}/{name}: the record differs at line {}:\n  saved:  {:?}\n  actual: {:?}\n(full output in {})",
            first + 1,
            saved.lines().nth(first),
            text.lines().nth(first),
            actual.display()
        );
    }
}
/// What `lists` draw this frame, as `post_render` sends them.
fn drawn(lists: &[&crate::draw_list::DrawList]) -> Vec<String> {
    crate::fake_draw::take_log();
    crate::fake_draw::frame(|ctx| {
        for list in lists {
            list.submit(ctx);
        }
    });
    crate::fake_draw::take_log()
}

/// One HUD frame's inputs, as `Client::apply_windows` passes them.
#[derive(Clone)]
struct HudFrame {
    active: bool,
    playing: bool,
    phase: Option<Phase>,
    snapshot: Option<Snapshot>,
    artwork: Option<Artwork>,
    skills: HudSkills,
    status: String,
    cursor: Option<(f32, f32)>,
    pressed: bool,
    recalling: bool,
    /// Settings › Background colour (None: the default).
    background: Option<u32>,
    /// The mod's language (None: English).
    language: Option<usize>,
    /// Milliseconds to the next frame.
    step_ms: u64,
}
fn athlete() -> Snapshot {
    Snapshot {
        key: (1, 2, 3),
        player: 7,
        champion: "hammerer".into(),
        level: 1,
        hp: Some((900, 1000)),
        alive: true,
        respawn: 0,
        gold: 100,
        kda: (0, 0, 0),
        cs: 0,
        cooldowns: [0, 0, 0],
        items: vec!["iron_blade".into()],
        build: Some(vec![
            "ironsword".into(),
            "dagger".into(),
            "steel_armor".into(),
            "iron_blade".into(),
        ]),
    }
}
/// Skills as the abilities module reports them for cooldowns in ticks.
fn skills(cooldowns: [usize; 3], period: usize) -> HudSkills {
    HudSkills {
        available: [Some(true); 3],
        ready: cooldowns.map(|c| Some(c == 0)),
        wait: cooldowns.map(Some),
        period: [Some(period); 3],
        ..HudSkills::default()
    }
}
// The HUD root's rectangle on screen: the strip is 146 px high at the bottom.
const HUD_TOP: f32 = 1080. - 146.;
fn skill_point(slot: usize) -> (f32, f32) {
    (832. + 88. * slot as f32 + 40., HUD_TOP + 4. + 40.)
}
fn next_point() -> (f32, f32) {
    (1390. + 18., HUD_TOP + 100. + 18.)
}
/// The strip's shop button on screen (strip x 1386, y 90, 148 x 52).
const SHOP_HIT: (f32, f32, f32, f32) = (1386., HUD_TOP + 90., 148., 52.);

fn hud_frames() -> Vec<HudFrame> {
    let rest = HudFrame {
        active: false,
        playing: false,
        phase: None,
        snapshot: None,
        artwork: None,
        skills: HudSkills::default(),
        status: String::new(),
        cursor: None,
        pressed: false,
        recalling: false,
        background: None,
        language: None,
        step_ms: 17,
    };
    let mut frames = Vec::new();
    // Before control: nothing is spawned.
    for _ in 0..3 {
        frames.push(rest.clone());
    }
    // Champion locked, waiting for Start: HUD up, not playing, artwork only.
    let artwork = Artwork {
        identity: ((1, 2, 3), 7),
        champion: "hammerer".into(),
        items: vec!["iron_blade".into()],
    };
    for _ in 0..4 {
        frames.push(HudFrame {
            active: true,
            phase: Some(Phase::Ready),
            artwork: Some(artwork.clone()),
            ..rest.clone()
        });
    }
    // Control starts before the first sample arrives.
    for _ in 0..3 {
        frames.push(HudFrame {
            active: true,
            playing: true,
            phase: Some(Phase::Running),
            artwork: Some(artwork.clone()),
            ..rest.clone()
        });
    }
    let playing = HudFrame {
        active: true,
        playing: true,
        phase: Some(Phase::Running),
        snapshot: Some(athlete()),
        skills: skills([0, 0, 0], 120),
        ..rest.clone()
    };
    for _ in 0..5 {
        frames.push(playing.clone());
    }
    // Q and W used: their cooldowns tick down to ready (the chime), one
    // tick per frame, with the gold rising.
    for t in 0..130usize {
        let mut f = playing.clone();
        let q = 120usize.saturating_sub(t);
        let w = 90usize.saturating_sub(t);
        let s = f.snapshot.as_mut().unwrap();
        s.cooldowns = [q, w, 0];
        s.gold = 100 + t * 3;
        s.cs = t / 20;
        f.skills = skills([q, w, 0], 120);
        frames.push(f);
    }
    // Hover on Q (tooltip), a press and release on it, then on the next
    // purchase tile and off again.
    for t in 0..40 {
        let mut f = playing.clone();
        f.cursor = Some(skill_point(0));
        f.pressed = (10..14).contains(&t);
        frames.push(f);
    }
    for _ in 0..20 {
        let mut f = playing.clone();
        f.cursor = Some(next_point());
        frames.push(f);
    }
    for _ in 0..10 {
        let mut f = playing.clone();
        f.cursor = Some((300., 300.));
        frames.push(f);
    }
    // A press and release on the strip's shop button.
    for t in 0..12 {
        let mut f = playing.clone();
        f.cursor = Some((SHOP_HIT.0 + 10., SHOP_HIT.1 + 10.));
        f.pressed = (3..6).contains(&t);
        frames.push(f);
    }
    // A purchase: the gold drops, a second item appears; a level-up.
    for t in 0..30 {
        let mut f = playing.clone();
        let s = f.snapshot.as_mut().unwrap();
        s.gold = 490 - (t * 2).min(30);
        s.items = vec!["iron_blade".into(), "dagger".into()];
        s.level = if t > 10 { 3 } else { 1 };
        f.status = if t < 8 {
            "Not enough gold".into()
        } else {
            String::new()
        };
        frames.push(f);
    }
    // A cast refused (red feedback), a multi-use R with a refill, aiming W.
    let denied = crate::clock::now();
    for t in 0..40 {
        let mut f = playing.clone();
        let s = f.snapshot.as_mut().unwrap();
        s.level = 5;
        s.items = vec!["iron_blade".into(), "dagger".into()];
        f.skills.denied_at[0] = (t < 20).then_some(denied);
        f.skills.uses[2] = Some(if t < 25 { 1 } else { 2 });
        f.skills.uses_max[2] = Some(2);
        f.skills.refill[2] = (t < 25).then_some(60 - t);
        f.skills.aiming = (t > 30).then_some(1);
        frames.push(f);
    }
    // Low health, then a death with its respawn countdown, a recall, and
    // the respawn.
    for t in 0..20usize {
        let mut f = playing.clone();
        let s = f.snapshot.as_mut().unwrap();
        s.hp = Some((900 - t * 40, 1000));
        s.level = 5;
        frames.push(f);
    }
    for t in 0..50usize {
        let mut f = playing.clone();
        let s = f.snapshot.as_mut().unwrap();
        s.hp = Some((0, 1000));
        s.alive = false;
        s.respawn = 300 - t * 6;
        s.kda = (0, 1, 0);
        s.level = 5;
        frames.push(f);
    }
    for t in 0..30usize {
        let mut f = playing.clone();
        let s = f.snapshot.as_mut().unwrap();
        s.hp = Some((1000, 1000));
        s.kda = (1, 1, 0);
        s.level = 5;
        f.recalling = (5..20).contains(&t);
        frames.push(f);
    }
    // A new background colour, then another mod language: the HUD is
    // rebuilt each time (ui_theme::refresh) and every write follows.
    for t in 0..25 {
        let mut f = playing.clone();
        f.snapshot.as_mut().unwrap().level = 5;
        f.background = Some(0x151720);
        f.language = (t > 12).then_some(3);
        f.cursor = (t > 18).then(|| skill_point(1));
        frames.push(f);
    }
    // Pause, then AI control (HUD up, not playing), then back.
    for _ in 0..20 {
        frames.push(HudFrame {
            phase: Some(Phase::Paused),
            ..playing.clone()
        });
    }
    for _ in 0..20 {
        frames.push(HudFrame {
            playing: false,
            phase: Some(Phase::Ai),
            ..playing.clone()
        });
    }
    for _ in 0..10 {
        frames.push(playing.clone());
    }
    // Control returns to the AI for good; the HUD closes.
    for _ in 0..5 {
        frames.push(rest.clone());
    }
    frames
}

struct HudRig {
    hud: PlayerHud,
    ui: HudUi,
    plate: StatePlate,
    log: crate::Logger,
    /// The draw records: the HUD strip's and the state plate's commands.
    draws: [Vec<String>; 2],
    frames: usize,
}
impl HudRig {
    fn new() -> Self {
        Self {
            hud: PlayerHud::default(),
            ui: HudUi::default(),
            plate: StatePlate::default(),
            log: crate::test_support::logger("ui-identity-hud"),
            draws: Default::default(),
            frames: 0,
        }
    }
    /// The HUD part of `Client::apply_windows`, in its order.
    fn frame(&mut self, f: &HudFrame) -> Vec<String> {
        let chimes = crate::ui_sound::READY_CALLS.with(|n| n.get());
        crate::ui_theme::TEST_BACKGROUND.with(|b| b.set(f.background));
        crate::lang::set_for_test(f.language);
        let (hovered, bounds, shop_click) = fake_ui::frame(|ctx| {
            crate::ui_theme::refresh(ctx, player_hud::PATH);
            self.ui.apply(
                ctx,
                f.active,
                f.playing,
                f.snapshot.as_ref(),
                f.artwork.as_ref(),
                f.skills,
                &f.status,
                f.cursor,
                f.pressed,
                &self.hud,
                &self.log,
            );
            let alive = f.snapshot.as_ref().is_some_and(|s| s.alive);
            let hovered = (f.active && alive)
                .then(|| self.ui.hovered_skill(ctx, f.cursor))
                .flatten();
            crate::client::write_recall_progress(ctx, &mut self.ui, f.recalling, f.active && alive);
            self.plate.apply(
                ctx,
                f.phase.filter(|_| f.active),
                f.snapshot.as_ref(),
                &self.log,
            );
            let mut bounds = Vec::new();
            self.ui.bounds(ctx, &mut bounds);
            (hovered, bounds, self.ui.take_shop_click())
        });
        let mut lines = fake_ui::take_log();
        for (record, list) in self.draws.iter_mut().zip([&self.ui.list, &self.plate.list]) {
            record.push(format!("## {}", self.frames));
            record.extend(drawn(&[list]));
        }
        self.frames += 1;
        let chimes = crate::ui_sound::READY_CALLS.with(|n| n.get()) - chimes;
        let bounds: Vec<String> = bounds
            .iter()
            .map(|r| format!("{:.1},{:.1},{:.1},{:.1}", r.x, r.y, r.w, r.h))
            .collect();
        lines.push(format!(
            "# hovered={hovered:?} chimes={chimes} shop_click={shop_click} bounds=[{}]",
            bounds.join(" ")
        ));
        lines
    }
}

/// The player HUD strip, the A2 recall bar written from client.rs, the
/// state plate, the strip's blocked rectangles and the hovered skill.
#[test]
fn hud_strip_and_state_plate_writes_match_1_5_3() {
    fake_ui::reset();
    test_clock::stop();
    let mut rig = HudRig::new();
    let mut out = Vec::new();
    for (i, f) in hud_frames().iter().enumerate() {
        out.push(format!("## frame {i}"));
        out.extend(rig.frame(f));
        test_clock::advance(Duration::from_millis(f.step_ms));
    }
    // A failing host: nothing sticks, everything is retried next frame.
    fake_ui::with(|u| u.refuse_writes = true);
    let f = hud_frames()[200].clone();
    for i in 0..3 {
        out.push(format!("## refused {i}"));
        out.extend(rig.frame(&f));
        test_clock::advance(FRAME);
    }
    fake_ui::with(|u| u.refuse_writes = false);
    for i in 0..3 {
        out.push(format!("## accepted {i}"));
        out.extend(rig.frame(&f));
        test_clock::advance(FRAME);
    }
    test_clock::release();
    crate::ui_theme::TEST_BACKGROUND.with(|b| b.set(None));
    crate::lang::set_for_test(None);
    check_record("hud", &out);
    check_draw_record("hud", "hud", &rig.draws[0]);
    check_draw_record("hud", "plate", &rig.draws[1]);
}

fn lanes() -> Vec<crate::own_selection::PlayerIdentity> {
    ["hammerer", "ghost", "wind_mage"]
        .iter()
        .enumerate()
        .map(|(lane, champion)| crate::own_selection::PlayerIdentity {
            player: 5 + lane,
            athlete: 40 + lane,
            lane,
            side: 0,
            champion: (*champion).into(),
        })
        .collect()
}
fn rects(bounds: &[crate::camera::Rect]) -> String {
    bounds
        .iter()
        .map(|r| format!("{:.1},{:.1},{:.1},{:.1}", r.x, r.y, r.w, r.h))
        .collect::<Vec<_>>()
        .join(" ")
}
/// The session strip (and the pre-match chooser): Loading, Ready, playing
/// with the cursor over its buttons and a press, the swap column, pause,
/// AI control, and leaving the battlefield.
#[test]
fn session_strip_writes_match_1_5_3() {
    use crate::session_ui::{SessionUi, PATH};
    fake_ui::reset();
    test_clock::stop();
    let timing = std::sync::Arc::new(crate::native_timing::NativeTiming::new(true));
    let camera = std::sync::Arc::new(crate::camera::CameraControl::default());
    let settings = crate::settings::Settings::new(None);
    let log = crate::test_support::logger("ui-identity-session");
    let players = lanes();
    let mut ui = SessionUi::default();
    let mut out = Vec::new();
    let mut draws = Vec::new();
    // (phase, cursor, pressed, frames)
    type Step = (Option<Phase>, Option<(f32, f32)>, bool, usize);
    let script: Vec<Step> = vec![
        (None, None, false, 3),
        (Some(Phase::Loading), None, false, 5),
        (Some(Phase::Ready), None, false, 5),
        // Over lane 0 of the chooser (root 704, 938; tile 0, 4, 72 x 72).
        (Some(Phase::Ready), Some((740., 978.)), false, 10),
        (Some(Phase::Running), None, false, 10),
        (Some(Phase::Running), Some((30., 1050.)), false, 15),
        (Some(Phase::Running), Some((30., 1050.)), true, 3),
        (Some(Phase::Running), Some((126., 1050.)), false, 10),
        (Some(Phase::Running), Some((230., 1050.)), false, 10),
        (Some(Phase::Running), Some((320., 1050.)), false, 10),
        (Some(Phase::Running), Some((610., 1050.)), false, 10),
        (Some(Phase::Running), Some((30., 980.)), false, 20),
        (Some(Phase::Running), Some((900., 500.)), true, 2),
        (Some(Phase::Running), Some((900., 500.)), false, 15),
        (Some(Phase::Paused), Some((30., 1050.)), false, 10),
        (Some(Phase::Ai), None, false, 10),
        (Some(Phase::Ai), Some((60., 1050.)), false, 10),
        (Some(Phase::Running), None, false, 5),
        (Some(Phase::Released), None, false, 3),
        (None, None, false, 3),
    ];
    let mut i = 0;
    for (phase, cursor, pressed, frames) in script {
        for _ in 0..frames {
            timing.set_test_phase(phase);
            // The swap button's click (the game's event cannot be faked
            // here) opens the column at a fixed frame.
            if i == 105 {
                ui.open_swap_for_test();
            }
            let mut bounds = Vec::new();
            fake_ui::frame(|ctx| {
                crate::ui_theme::refresh(ctx, PATH);
                ui.apply(
                    ctx,
                    &timing,
                    &players,
                    Some(6),
                    &camera,
                    &settings,
                    cursor,
                    pressed,
                    &log,
                    &mut bounds,
                )
            });
            out.push(format!("## frame {i}"));
            out.extend(fake_ui::take_log());
            draws.push(format!("## {i}"));
            draws.extend(drawn(&[&ui.list]));
            let sounds = (
                crate::ui_sound::HOVER_CALLS.with(|n| n.replace(0)),
                crate::ui_sound::CLICK_CALLS.with(|n| n.replace(0)),
            );
            out.push(format!(
                "# bounds=[{}] hovers={} clicks={}",
                rects(&bounds),
                sounds.0,
                sounds.1
            ));
            test_clock::advance(FRAME);
            i += 1;
        }
    }
    test_clock::release();
    check_record("session", &out);
    check_draw_record("session", "session", &draws);
}

fn team_player(side: usize, lane: usize, t: usize) -> crate::team_status::Player {
    crate::team_status::Player {
        id: side * 5 + lane,
        side,
        lane,
        champion: ["hammerer", "ghost", "wind_mage", "dokkaebi", "werewolf"][lane].into(),
        alive: Some(!(side == 1 && lane == 2 && t < 10)),
        respawn: 300usize.saturating_sub(t * 6),
        level: 1 + t / 10,
        gold: 500 + t * 7,
        kda: (lane, side, t / 5),
        cs: t,
        items: if t > 5 {
            vec!["iron_blade".into(), "dagger".into()]
        } else {
            vec!["iron_blade".into()]
        },
        build: Some(vec![
            "ironsword".into(),
            "dagger".into(),
            "steel_armor".into(),
            "iron_blade".into(),
        ]),
    }
}
/// Tab: closed, held open while values change, closed, open again.
#[test]
fn team_panel_writes_match_1_5_3() {
    use crate::team_status::TeamUi;
    fake_ui::reset();
    test_clock::stop();
    let log = crate::test_support::logger("ui-identity-team");
    let hud = HudUi::default();
    let mut ui = TeamUi::default();
    let mut out = Vec::new();
    let mut draws = Vec::new();
    for i in 0..80usize {
        let active = (5..40).contains(&i) || (60..75).contains(&i);
        let roster: Vec<_> = if active {
            (0..2)
                .flat_map(|side| (0..5).map(move |lane| team_player(side, lane, i)))
                .collect()
        } else {
            Vec::new()
        };
        fake_ui::frame(|ctx| {
            crate::ui_theme::refresh(ctx, crate::team_status::PATH);
            ui.apply(ctx, active, &roster, Some(1), &hud, &log)
        });
        out.push(format!("## frame {i}"));
        out.extend(fake_ui::take_log());
        draws.push(format!("## {i}"));
        draws.extend(drawn(&[&ui.list]));
        test_clock::advance(FRAME);
    }
    test_clock::release();
    check_record("team", &out);
    check_draw_record("team", "team", &draws);
}

fn stats_unit(t: usize, champion: Option<&str>, friendly: bool) -> crate::stats_panel::Unit {
    crate::stats_panel::Unit {
        champion: champion.map(Into::into),
        kind: if champion.is_some() {
            crate::stats_panel::Kind::Champion
        } else {
            crate::stats_panel::Kind::Tower
        },
        friendly,
        level: 1 + t / 20,
        hp: (900 - t * 3, 1000),
        shield: if t % 30 < 10 { 40 } else { 0 },
        values: std::array::from_fn(|k| 10. * k as f64 + (t / 7) as f64 * 0.5),
    }
}
/// The own stats panel and the target frame: shown, values changing, a
/// target change, the target cleared, control lost.
#[test]
fn stats_panels_writes_match_1_5_3() {
    use crate::stats_panel::TEST_UNITS;
    use crate::stats_ui::StatsUi;
    fake_ui::reset();
    test_clock::stop();
    let log = crate::test_support::logger("ui-identity-stats");
    let mut ui = StatsUi::default();
    let mut out = Vec::new();
    let mut draws = Vec::new();
    for i in 0..90usize {
        let own = (i >= 3).then(|| stats_unit(i, Some("hammerer"), true));
        let target = match i {
            10..=40 => Some(stats_unit(i, Some("ghost"), false)),
            41..=60 => Some(stats_unit(i, None, false)),
            _ => None,
        };
        TEST_UNITS.with(|t| *t.borrow_mut() = Some((own, target)));
        // A new background colour rebuilds both panels (frames 30 to 34).
        crate::ui_theme::TEST_BACKGROUND.with(|b| b.set((30..35).contains(&i).then_some(0x151720)));
        let mut bounds = Vec::new();
        fake_ui::frame(|ctx| {
            crate::ui_theme::refresh(ctx, crate::stats_ui::OWN);
            crate::ui_theme::refresh(ctx, crate::stats_ui::TARGET);
            ui.apply(ctx, i < 85, i < 75, false, &log, &mut bounds)
        });
        out.push(format!("## frame {i}"));
        out.extend(fake_ui::take_log());
        draws.push(format!("## {i}"));
        draws.extend(drawn(&[&ui.list]));
        out.push(format!("# bounds=[{}]", rects(&bounds)));
        test_clock::advance(FRAME);
    }
    TEST_UNITS.with(|t| *t.borrow_mut() = None);
    crate::ui_theme::TEST_BACKGROUND.with(|b| b.set(None));
    test_clock::release();
    check_record("stats", &out);
    check_draw_record("stats", "stats", &draws);
}

/// The screen effect: off, the low-health tint rising and breathing, a
/// death veil fading in, the respawn, off again.
#[test]
fn screen_effect_writes_match_1_5_3() {
    use crate::camera::{CameraFrame, Rect};
    use crate::screen_effect::Effect;
    fake_ui::reset();
    test_clock::stop();
    let log = crate::test_support::logger("ui-identity-effect");
    let camera = crate::camera::CameraControl::default();
    let frame = CameraFrame {
        viewport: Rect {
            x: 0.,
            y: 50.,
            w: 1920.,
            h: 974.,
        },
        center: (480., 480.),
        extent: (1024., 1024.),
        zoom: 1.,
        minimap: Rect {
            x: 1581.,
            y: 740.,
            w: 320.,
            h: 320.,
        },
    };
    let mut effect = Effect::default();
    let mut out = Vec::new();
    let mut draws = Vec::new();
    for i in 0..150usize {
        camera.capture(frame);
        let mut s = athlete();
        s.hp = Some(match i {
            0..=9 => (900, 1000),
            10..=59 => (900 - (i - 10) * 16, 1000),
            60..=99 => (0, 1000),
            _ => (1000, 1000),
        });
        s.alive = !(60..100).contains(&i);
        effect.update(true, i < 140, Some(&s), 16_667);
        fake_ui::frame(|_| effect.apply(&camera, &log));
        out.push(format!("## frame {i}"));
        out.extend(fake_ui::take_log());
        draws.push(format!("## {i}"));
        draws.extend(drawn(&[&effect.list]));
        test_clock::advance(FRAME);
    }
    test_clock::release();
    check_record("effect", &out);
    check_draw_record("effect", "effect", &draws);
}

/// Steady play (nothing changes on screen): how many of the HUD's typed
/// writes still format their text. 1.5.3 formatted every one of them.
#[test]
fn steady_hud_frames_format_almost_nothing() {
    fake_ui::reset();
    test_clock::stop();
    let mut rig = HudRig::new();
    let frames = hud_frames();
    // Control start, then the athlete standing still: let tweens settle.
    for f in frames.iter().take(15) {
        rig.frame(f);
        test_clock::advance(FRAME);
    }
    let steady = frames[14].clone();
    for _ in 0..120 {
        rig.frame(&steady);
        test_clock::advance(FRAME);
    }
    crate::ui_writes::COUNTS.with(|c| c.set((0, 0)));
    for _ in 0..60 {
        let lines = rig.frame(&steady);
        // Nothing is sent to the game on a steady frame.
        assert_eq!(lines.len(), 1, "{lines:?}");
        test_clock::advance(FRAME);
    }
    let (calls, formatted) = crate::ui_writes::COUNTS.with(|c| c.get());
    test_clock::release();
    println!("steady HUD frames: {calls} typed writes, {formatted} formatted (60 frames)");
    // 1.6.0-perf-dev3: about 24 typed writes a frame remain (the tree nodes
    // that stay); the drawn elements are values in `HudView`, not writes.
    assert!(calls > 60 * 20, "{calls}");
    assert!(formatted * 100 <= calls, "{formatted} of {calls} formatted");
}
