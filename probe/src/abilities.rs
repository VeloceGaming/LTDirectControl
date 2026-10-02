//! Physical key edges -> quickcasts or persistent normal-cast aiming.
//! Only quickcast presses / battlefield confirmation clicks enqueue a cast.
//! No native pointers leave a hook.
use crate::{
    camera::CameraFrame, combat::Unit, native_timing::MatchKey, platform_input::Keys, Logger,
};
use mod_api_stable::{InputKindV1, InputTargetV1, InputV1, StableClient};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const FRESH: Duration = Duration::from_millis(250);
const NAMES: [&str; 3] = ["Q", "W", "R"];
const KINDS: [InputKindV1; 3] = [InputKindV1::Skill, InputKindV1::Skill2, InputKindV1::Ult];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Descriptor {
    pub casting: u32,
    pub target: u32,
    pub range: u64,
}
impl Descriptor {
    // Verified native Option<Effect> POD fields; not the C ABI EffectSpecV1.
    // Native Arc/vtable words are deliberately neither copied nor dereferenced.
    pub fn from_fields(
        casting: u32,
        target: u32,
        range: u64,
        growth: u64,
        level: u64,
        bonus: u64,
    ) -> Option<Self> {
        if casting > 3 || target > 13 || !(1..=100).contains(&level) {
            return None;
        }
        let range = range
            .checked_add(growth.checked_mul(level - 1)?)?
            .checked_add(bonus)?;
        (range <= 10_000_000).then_some(Self {
            casting,
            target,
            range,
        })
    }
    fn label(self) -> &'static str {
        match self.casting {
            0 if self.target == 4 => "self target",
            0 => "unit target",
            1 => "ground target",
            2 => "direction",
            _ => "no cursor target",
        }
    }
}
#[derive(Clone, Copy)]
struct Aim {
    frame: CameraFrame,
    cursor: (f32, f32),
    world: (u64, u64),
}
#[derive(Clone, Copy)]
struct CastRequest {
    slot: usize,
    aim: Option<Aim>,
    at: Instant,
    normal_cast: bool,
    generation: u64,
    champion_only: bool,
}
#[derive(Clone, Copy, Default)]
pub struct ClientAction {
    pub left_reserved: bool,
    pub disarm_attack_move: bool,
    pub new_cast: bool,
}
#[derive(Clone, Copy, Default)]
pub struct HudSkills {
    pub aiming: Option<usize>,
    pub available: [Option<bool>; 3],
}
struct Snapshot {
    actor: usize,
    key: MatchKey,
    skills: [Option<Descriptor>; 3],
    at: Instant,
}
#[derive(Default)]
struct State {
    previous: [bool; 3],
    previous_right: bool,
    previous_left: bool,
    previous_recall: bool,
    previous_attack_move: bool,
    generation: u64,
    focused: bool,
    active: bool,
    updated: Option<Instant>,
    pending: Option<CastRequest>,
    preview: Option<usize>,
    aim: Option<Aim>,
    metadata: Option<Snapshot>,
    actor: Option<usize>,
    key: Option<MatchKey>,
    position: Option<(u64, u64)>,
    cooldowns: [usize; 3],
    level: Option<usize>,
    message: Option<(String, Instant)>,
}
#[derive(Default)]
pub struct Abilities(Mutex<State>);
impl State {
    fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.preview = None;
        self.pending = None;
    }
}

impl Abilities {
    pub fn reset_session(&self, keys: Keys) {
        if let Ok(mut s) = self.0.lock() {
            let generation = s.generation.wrapping_add(1);
            *s = State::default();
            s.generation = generation;
            s.previous = keys.abilities;
            s.previous_right = keys.right;
            s.previous_left = keys.left;
            s.previous_recall = keys.recall;
            s.previous_attack_move = keys.attack_move;
        }
    }
    pub fn clear_commands(&self) {
        if let Ok(mut s) = self.0.lock() {
            s.cancel();
            s.message = None;
        }
    }
    pub fn hud_skills(&self) -> HudSkills {
        let Ok(s) = self.0.lock() else {
            return HudSkills::default();
        };
        HudSkills {
            aiming: s.preview.filter(|_| s.active && s.focused),
            available: s
                .metadata
                .as_ref()
                .filter(|m| m.at.elapsed() <= FRESH)
                .map_or([None; 3], |m| m.skills.map(|d| Some(d.is_some()))),
        }
    }
    pub fn update(
        &self,
        keys: Keys,
        active: bool,
        camera: &crate::camera::CameraControl,
        log: &Logger,
    ) -> ClientAction {
        let Ok(mut s) = self.0.lock() else {
            return ClientAction::default();
        };
        let mut action = ClientAction::default();
        let armed = active && s.active && keys.focused && s.focused;
        let edges = std::array::from_fn::<_, 3, _>(|i| keys.abilities[i] && !s.previous[i]);
        let right_click = keys.right && !s.previous_right;
        let left_click = keys.left && !s.previous_left;
        let recall = keys.recall && !s.previous_recall;
        let attack_move = keys.attack_move && !s.previous_attack_move;
        s.previous_right = keys.right;
        s.previous_left = keys.left;
        s.previous_recall = keys.recall;
        s.previous_attack_move = keys.attack_move;
        s.previous = keys.abilities;
        s.active = active;
        s.focused = keys.focused;
        s.updated = Some(Instant::now());
        s.aim = keys
            .cursor
            .filter(|p| !camera.command_blocked(*p))
            .and_then(|cursor| {
                let frame = camera.frame()?;
                Some(Aim {
                    world: frame.unproject(cursor)?,
                    frame,
                    cursor,
                })
            });
        if !armed
            || keys.stop
            || keys.escape
            || keys.release
            || s.position.is_none()
            || right_click
            || recall
            || attack_move
        {
            s.cancel();
            return action;
        }
        let ability_pressed = edges.into_iter().any(|edge| edge);
        for (slot, edge) in edges.into_iter().enumerate() {
            if !edge {
                continue;
            }
            if keys.shift {
                s.cancel();
                s.preview = Some(slot);
                action.disarm_attack_move = true;
                log.write(&format!(
                    "ABILITY AIMING {} selected; release keeps mode; left-click confirms",
                    NAMES[slot]
                ));
            } else {
                s.cancel();
                action.disarm_attack_move = true;
                action.new_cast = true;
                // One pending cast; simultaneous polled edges use R > W > Q.
                s.pending = Some(CastRequest {
                    slot,
                    aim: s.aim,
                    at: Instant::now(),
                    normal_cast: false,
                    generation: s.generation,
                    champion_only: keys.champion_only,
                });
                log.write(&format!(
                    "ABILITY PRESS {} cursor={:?}",
                    NAMES[slot], keys.cursor
                ));
            }
        }
        if left_click && !ability_pressed && s.aim.is_some() {
            if let Some(slot) = s.preview {
                s.generation = s.generation.wrapping_add(1);
                s.pending = Some(CastRequest {
                    slot,
                    aim: s.aim,
                    at: Instant::now(),
                    normal_cast: true,
                    generation: s.generation,
                    champion_only: keys.champion_only,
                });
                action.left_reserved = true;
                action.new_cast = true;
                log.write(&format!(
                    "ABILITY CONFIRM {} cursor={:?}",
                    NAMES[slot], keys.cursor
                ));
            }
        }
        action
    }
    pub fn observe_actor(
        &self,
        key: MatchKey,
        actor: Option<usize>,
        position: Option<(u64, u64)>,
        cooldowns: [usize; 3],
    ) {
        if let Ok(mut s) = self.0.lock() {
            if s.key != Some(key) || s.actor != actor || position.is_none() {
                s.cancel();
                s.metadata = None;
                s.level = None;
            }
            s.key = Some(key);
            s.actor = actor;
            s.position = position;
            s.cooldowns = cooldowns;
        }
    }
    pub fn observe_level(&self, key: MatchKey, actor: usize, level: usize) {
        if let Ok(mut s) = self.0.lock() {
            if s.key == Some(key) && s.actor == Some(actor) {
                s.level = Some(level);
            }
        }
    }
    pub fn observe_metadata(
        &self,
        key: MatchKey,
        actor: usize,
        skills: [Option<Descriptor>; 3],
        log: &Logger,
    ) {
        if let Ok(mut s) = self.0.lock() {
            if s.key != Some(key) || s.actor != Some(actor) {
                return;
            }
            if s.metadata.as_ref().is_none_or(|old| old.skills != skills) {
                log.write(&format!(
                    "ABILITY NATIVE_METADATA actor={actor} key={key:?} Q/W/R={skills:?}"
                ));
            }
            s.metadata = Some(Snapshot {
                actor,
                key,
                skills,
                at: Instant::now(),
            });
        }
    }
    pub fn take_input(
        &self,
        key: MatchKey,
        actor: usize,
        position: (u64, u64),
        units: &[Unit],
        mut valid: impl FnMut(&InputV1) -> bool,
        log: &Logger,
    ) -> Option<InputV1> {
        let (request, desc, level, cooldown) = {
            let mut s = self.0.lock().ok()?;
            let request = s.pending.take()?;
            if !s.active
                || !s.focused
                || s.key != Some(key)
                || s.actor != Some(actor)
                || s.updated.is_none_or(|t| t.elapsed() > FRESH)
                || request.at.elapsed() > FRESH
            {
                return None;
            }
            let desc = s
                .metadata
                .as_ref()
                .filter(|m| m.actor == actor && m.key == key && m.at.elapsed() <= FRESH)
                .and_then(|m| m.skills[request.slot]);
            (request, desc, s.level, s.cooldowns[request.slot])
        };
        let result = desc.and_then(|d| {
            let make = |target| InputV1::action(KINDS[request.slot], target);
            if d.casting == 0 && d.target != 4 {
                let aim = request.aim?;
                // The native validator decides side, range, CC, cooldown,
                // targetability and restrictions for THIS skill. Apply the
                // user's champion-only filter before ranking overlapping hits.
                crate::combat::clicked_units(aim.frame, aim.cursor, units, request.champion_only)
                    .into_iter()
                    .map(|id| make(InputTargetV1::target(id)))
                    .find(|input| valid(input))
            } else {
                let target = match d.casting {
                    0 => InputTargetV1::target(actor),
                    1 => {
                        let aim = request.aim?;
                        InputTargetV1::pos(aim.world.0, aim.world.1)
                    }
                    2 => {
                        let aim = request.aim?;
                        let x = aim.world.0 as i64 - position.0 as i64;
                        let y = aim.world.1 as i64 - position.1 as i64;
                        if x == 0 && y == 0 {
                            return None;
                        }
                        InputTargetV1::dir(x, y)
                    }
                    _ => InputTargetV1::NONE,
                };
                let input = make(target);
                valid(&input).then_some(input)
            }
        });
        let message = if result.is_some() {
            format!("{} cast sent", NAMES[request.slot])
        } else if level.is_some_and(|l| l < [1, 3, 5][request.slot]) {
            format!("{}: Lv {}", NAMES[request.slot], [1, 3, 5][request.slot])
        } else if cooldown > 0 {
            format!("{}: {:.1}s", NAMES[request.slot], cooldown as f32 / 60.)
        } else if desc.is_none() {
            format!(
                "{} unavailable: current ability data not ready",
                NAMES[request.slot]
            )
        } else if desc.is_some_and(|d| d.casting == 0 && d.target == 2) {
            format!("{} needs an ally under crowd control", NAMES[request.slot])
        } else {
            format!(
                "{} not cast: invalid aim/target or native cast unavailable",
                NAMES[request.slot]
            )
        };
        if let Ok(mut s) = self.0.lock() {
            // A newer command can arrive while native validation is running.
            // Do not deliver the stale request or dismiss a newer aiming mode.
            if s.generation != request.generation {
                log.write(
                    "ABILITY CANCEL stale request; newer client command arrived during validation",
                );
                return None;
            }
            if result.is_some() && request.normal_cast {
                s.preview = None;
            }
            s.message = Some((message, Instant::now()));
        } else {
            return None;
        }
        log.write(&format!(
            "ABILITY RESULT {} normal_cast={} champion_only={} descriptor={desc:?} input={result:?}; one-shot consumed",
            NAMES[request.slot], request.normal_cast, request.champion_only
        ));
        result
    }
    pub fn status(&self) -> Option<String> {
        let s = self.0.lock().ok()?;
        if !s.active || !s.focused {
            return None;
        }
        if let Some(slot) = s.preview {
            let d = s
                .metadata
                .as_ref()
                .filter(|m| m.at.elapsed() <= FRESH)
                .and_then(|m| m.skills[slot]);
            return Some(d.map_or_else(
                || {
                    format!(
                        "{} aiming: ability data unavailable | Left-click retry / RMB cancel",
                        NAMES[slot]
                    )
                },
                |d| {
                    format!(
                        "{} aiming: {} | range {:.1} | CD {:.1}s | Left-click cast / RMB cancel",
                        NAMES[slot],
                        d.label(),
                        d.range as f32 / 1000.,
                        s.cooldowns[slot] as f32 / 60.
                    )
                },
            ));
        }
        s.message
            .as_ref()
            .filter(|(_, t)| t.elapsed() < Duration::from_secs(2))
            .map(|(msg, _)| msg.clone())
    }
    pub fn feedback(&self) -> Option<String> {
        let s = self.0.lock().ok()?;
        if !s.active || !s.focused {
            return None;
        }
        s.message
            .as_ref()
            .filter(|(m, t)| t.elapsed() < Duration::from_millis(1400) && !m.ends_with("cast sent"))
            .map(|(m, _)| m.clone())
    }
    pub fn draw(&self, ctx: &mut StableClient<'_>, camera: &crate::camera::CameraControl) {
        let data = self.0.lock().ok().and_then(|s| {
            if !s.active || !s.focused || s.updated.is_none_or(|t| t.elapsed() > FRESH) {
                return None;
            }
            let slot = s.preview?;
            let metadata = s.metadata.as_ref().filter(|m| m.at.elapsed() <= FRESH)?;
            Some((
                s.position?,
                metadata.skills[slot]?,
                s.aim,
                s.cooldowns[slot],
            ))
        });
        let Some((position, desc, aim, cooldown)) = data else {
            return;
        };
        let Some(frame) = camera.frame() else { return };
        let center = frame.project_unclipped(position.0 as f32 / 1000., position.1 as f32 / 1000.);
        let color = if cooldown == 0 {
            0xffd700cc
        } else {
            0x999999cc
        };
        let range = desc.range as f32 / 1000.;
        if range > 0. {
            for i in 0..128 {
                let point = |j: usize| {
                    let theta = j as f32 * std::f32::consts::TAU / 128.;
                    frame.project_unclipped(
                        position.0 as f32 / 1000. + range * theta.cos(),
                        position.1 as f32 / 1000. + range * theta.sin(),
                    )
                };
                draw_segment(ctx, frame, point(i), point(i + 1), color);
            }
        } else if let Some((x, y)) = frame.project(position) {
            ctx.draw_circle("UI", x, y, 7., 990, color);
        }
        if let Some(aim) = aim.filter(|_| matches!(desc.casting, 1 | 2)) {
            let end = if desc.casting == 2 {
                let dx = aim.world.0 as f32 - position.0 as f32;
                let dy = aim.world.1 as f32 - position.1 as f32;
                let length = dx.hypot(dy).max(1.);
                frame.project_unclipped(
                    position.0 as f32 / 1000. + dx / length * range,
                    position.1 as f32 / 1000. + dy / length * range,
                )
            } else {
                frame.project_unclipped(aim.world.0 as f32 / 1000., aim.world.1 as f32 / 1000.)
            };
            let length = (end.0 - center.0).hypot(end.1 - center.1);
            let steps = ((length / 16.).ceil() as usize).clamp(1, 512);
            for i in 0..steps {
                let p = |n: usize| {
                    let t = n as f32 / steps as f32;
                    (
                        center.0 + (end.0 - center.0) * t,
                        center.1 + (end.1 - center.1) * t,
                    )
                };
                draw_segment(ctx, frame, p(i), p(i + 1), color);
            }
            if frame.viewport.contains(end) && !frame.minimap.contains(end) {
                ctx.draw_circle("UI", end.0, end.1, 4., 991, color);
            }
        }
    }
}
pub(crate) fn draw_segment(
    ctx: &mut StableClient<'_>,
    frame: CameraFrame,
    a: (f32, f32),
    b: (f32, f32),
    color: u32,
) {
    if [a, b]
        .into_iter()
        .all(|p| frame.viewport.contains(p) && !frame.minimap.contains(p))
    {
        ctx.draw_line("UI", a.0, a.1, b.0, b.1, 2., 990, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timing_test::tests::logger;
    fn setup(casting: u32, target: u32) -> (Abilities, crate::camera::CameraControl, Logger) {
        let a = Abilities::default();
        let c = crate::camera::CameraControl::default();
        c.capture(CameraFrame {
            viewport: crate::camera::Rect {
                x: 0.,
                y: 50.,
                w: 1920.,
                h: 974.,
            },
            center: (200., 200.),
            extent: (1024., 1024.),
            minimap: crate::camera::Rect {
                x: 1581.,
                y: 740.,
                w: 320.,
                h: 320.,
            },
        });
        let log = logger("abilities");
        a.observe_actor((1, 33, 1), Some(7), Some((200_000, 200_000)), [0; 3]);
        a.observe_metadata(
            (1, 33, 1),
            7,
            [Descriptor::from_fields(casting, target, 100_000, 0, 1, 0); 3],
            &log,
        );
        a.update(keys(false, false), true, &c, &log);
        (a, c, log)
    }
    fn keys(pressed: bool, shift: bool) -> Keys {
        Keys {
            focused: true,
            abilities: [pressed, false, false],
            shift,
            cursor: Some((1160., 537.)),
            ..Keys::default()
        }
    }
    fn take(
        a: &Abilities,
        units: &[Unit],
        valid: impl FnMut(&InputV1) -> bool,
        log: &Logger,
    ) -> Option<InputV1> {
        a.take_input((1, 33, 1), 7, (200_000, 200_000), units, valid, log)
    }
    #[test]
    fn rejection_feedback_distinguishes_unlock_and_cooldown_without_changing_validator() {
        let (a, c, log) = setup(1, 5);
        a.observe_level((1, 33, 1), 7, 1);
        a.update(
            Keys {
                abilities: [false, true, false],
                ..keys(false, false)
            },
            true,
            &c,
            &log,
        );
        assert!(take(&a, &[], |_| false, &log).is_none());
        assert_eq!(a.feedback().as_deref(), Some("W: Lv 3"));
        a.update(keys(false, false), true, &c, &log);
        a.observe_actor((1, 33, 1), Some(7), Some((200_000, 200_000)), [120, 0, 0]);
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &[], |_| false, &log).is_none());
        assert_eq!(a.feedback().as_deref(), Some("Q: 2.0s"));
        a.clear_commands();
        assert!(a.feedback().is_none());
    }
    #[test]
    fn targeted_quick_and_normal_cast_use_the_upper_sprite_body_at_cursor() {
        let (a, c, log) = setup(0, 7);
        let unit = Unit {
            id: 29,
            position: (300_000, 200_000),
            radius: 5_000,
            is_champion: true,
            body: Some(crate::sprite_picking::Body {
                width: 24.,
                height: 40.,
            }),
        };
        let head = Keys {
            cursor: Some((1160., 467.)),
            ..keys(true, false)
        };
        a.update(head, true, &c, &log);
        assert_eq!(
            take(&a, &[unit], |i| i.target.target_id == 29, &log)
                .unwrap()
                .target
                .target_id,
            29
        );
        a.update(
            Keys {
                abilities: [false; 3],
                ..head
            },
            true,
            &c,
            &log,
        );
        a.update(
            Keys {
                shift: true,
                ..head
            },
            true,
            &c,
            &log,
        );
        a.update(
            Keys {
                abilities: [false; 3],
                shift: false,
                left: true,
                ..head
            },
            true,
            &c,
            &log,
        );
        assert_eq!(
            take(&a, &[unit], |i| i.target.target_id == 29, &log)
                .unwrap()
                .target
                .target_id,
            29
        );
    }
    #[test]
    fn ally_cc_requirement_reports_rejection_without_bypassing_native_validation() {
        let (a, c, log) = setup(0, 2);
        let units = [Unit {
            id: 29,
            position: (300_000, 200_000),
            radius: 10_000,
            is_champion: true,
            body: None,
        }];
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &units, |_| false, &log).is_none());
        assert!(a.status().unwrap().contains("ally under crowd control"));
        // An eligible ally appearing later does not replay the rejected press.
        assert!(take(&a, &units, |_| true, &log).is_none());
        a.update(keys(false, false), true, &c, &log);
        a.update(keys(true, false), true, &c, &log);
        assert_eq!(
            take(&a, &units, |i| i.target.target_id == 29, &log)
                .unwrap()
                .target
                .target_id,
            29
        );
    }
    #[test]
    fn session_reset_invalidates_cast_even_when_save_reuses_key_and_actor() {
        let (a, c, log) = setup(1, 6);
        a.update(keys(true, false), true, &c, &log);
        assert!(take(
            &a,
            &[],
            |_| {
                a.reset_session(keys(true, false));
                true
            },
            &log
        )
        .is_none());
        {
            let s = a.0.lock().unwrap();
            assert!(s.actor.is_none() && s.key.is_none() && s.metadata.is_none());
            assert!(s.pending.is_none() && s.preview.is_none() && s.position.is_none());
            assert!(s.previous[0] && !s.active);
        }
        a.observe_actor((1, 33, 1), Some(7), Some((200_000, 200_000)), [0; 3]);
        a.update(keys(true, false), true, &c, &log);
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &[], |_| true, &log).is_none());
    }
    #[test]
    fn shift_release_never_casts_and_holding_keys_never_repeats() {
        let (a, c, log) = setup(1, 6);
        a.update(keys(true, true), true, &c, &log);
        assert!(a.status().unwrap().contains("aiming"));
        assert!(take(&a, &[], |_| true, &log).is_none());
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &[], |_| true, &log).is_none());
        a.update(keys(false, false), true, &c, &log);
        a.update(keys(true, false), true, &c, &log);
        assert_eq!(
            take(&a, &[], |_| true, &log),
            Some(InputV1::action(
                InputKindV1::Skill,
                InputTargetV1::pos(300_000, 200_000)
            ))
        );
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &[], |_| true, &log).is_none());
    }
    #[test]
    fn target_skills_validate_only_cursor_hits_and_self_and_none_are_distinct() {
        let (a, c, log) = setup(0, 0);
        let units = [
            Unit {
                id: 2,
                position: (300_000, 200_000),
                radius: 10_000,
                is_champion: true,
                body: None,
            },
            Unit {
                id: 3,
                position: (300_000, 200_000),
                radius: 10_000,
                is_champion: true,
                body: None,
            },
            Unit {
                id: 4,
                position: (210_000, 200_000),
                radius: 10_000,
                is_champion: true,
                body: None,
            },
        ];
        a.update(keys(true, false), true, &c, &log);
        assert_eq!(
            take(&a, &units, |i| i.target.target_id == 3, &log)
                .unwrap()
                .target,
            InputTargetV1::target(3)
        );
        a.update(keys(false, false), true, &c, &log);
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &units, |i| i.target.target_id == 4, &log).is_none());
        for (casting, target, expected) in [
            (0, 4, InputTargetV1::target(7)),
            (3, 13, InputTargetV1::NONE),
            (2, 6, InputTargetV1::dir(100_000, 0)),
        ] {
            let (a, c, log) = setup(casting, target);
            a.update(keys(true, false), true, &c, &log);
            assert_eq!(take(&a, &[], |_| true, &log).unwrap().target, expected);
        }
    }
    #[test]
    fn pause_clears_pending_cast_and_held_skill_cannot_cast_on_resume() {
        let (a, c, log) = setup(1, 6);
        a.update(keys(true, false), true, &c, &log);
        a.clear_commands();
        a.update(keys(true, false), false, &c, &log);
        assert!(take(&a, &[], |_| true, &log).is_none());
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &[], |_| true, &log).is_none());
        a.update(keys(false, false), true, &c, &log);
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &[], |_| true, &log).is_some());
    }
    #[test]
    fn champion_only_quickcast_filters_overlaps_before_native_validation() {
        let (a, c, log) = setup(0, 0);
        let units = [
            Unit {
                id: 2,
                position: (300_000, 200_000),
                radius: 10_000,
                is_champion: false,
                body: None,
            },
            Unit {
                id: 3,
                position: (300_000, 200_000),
                radius: 10_000,
                is_champion: true,
                body: None,
            },
        ];
        let mut press = keys(true, false);
        press.champion_only = true;
        a.update(press, true, &c, &log);
        let mut validated = Vec::new();
        let input = take(
            &a,
            &units,
            |i| {
                validated.push(i.target.target_id);
                true
            },
            &log,
        )
        .unwrap();
        assert_eq!(input.target.target_id, 3);
        assert_eq!(validated, [3]);
        a.update(keys(false, false), true, &c, &log);
        a.update(press, true, &c, &log);
        // Native side/eligibility rejection cannot fall back to a minion.
        assert!(take(&a, &units, |i| i.target.target_id == 2, &log).is_none());
        assert!(take(&a, &units, |_| true, &log).is_none());
        a.update(keys(false, false), true, &c, &log);
        a.update(keys(true, false), true, &c, &log);
        assert_eq!(
            take(&a, &units, |_| true, &log).unwrap().target.target_id,
            2
        );
        // Ground, direction, self and no-cursor requests remain native requests.
        for (casting, target, expected) in [
            (1, 6, InputTargetV1::pos(300_000, 200_000)),
            (2, 6, InputTargetV1::dir(100_000, 0)),
            (0, 4, InputTargetV1::target(7)),
            (3, 13, InputTargetV1::NONE),
        ] {
            let (a, c, log) = setup(casting, target);
            a.update(press, true, &c, &log);
            assert_eq!(take(&a, &[], |_| true, &log).unwrap().target, expected);
        }
    }
    #[test]
    fn normal_cast_uses_mode_at_confirmation_and_minion_rejection_keeps_aiming() {
        let (a, c, log) = setup(0, 0);
        let units = [Unit {
            id: 2,
            position: (300_000, 200_000),
            radius: 10_000,
            is_champion: false,
            body: None,
        }];
        a.update(keys(true, true), true, &c, &log);
        let mut mode = keys(false, false);
        mode.champion_only = true;
        a.update(mode, true, &c, &log);
        assert_eq!(a.hud_skills().aiming, Some(0));
        mode.left = true;
        assert!(a.update(mode, true, &c, &log).left_reserved);
        assert!(take(&a, &units, |_| true, &log).is_none());
        assert_eq!(a.hud_skills().aiming, Some(0));
        assert!(take(&a, &units, |_| true, &log).is_none());
        mode.left = false;
        mode.champion_only = false;
        a.update(mode, true, &c, &log);
        mode.left = true;
        a.update(mode, true, &c, &log);
        assert_eq!(
            take(&a, &units, |_| true, &log).unwrap().target.target_id,
            2
        );
        assert_eq!(a.hud_skills().aiming, None);
    }
    #[test]
    fn native_rejection_drops_cast_and_cancellation_cannot_replay_held_keys() {
        let (a, c, log) = setup(1, 6);
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &[], |_| false, &log).is_none());
        assert!(take(&a, &[], |_| true, &log).is_none());
        a.update(keys(false, false), true, &c, &log);
        a.update(keys(true, false), true, &c, &log);
        a.update(
            Keys {
                focused: false,
                ..Keys::default()
            },
            true,
            &c,
            &log,
        );
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &[], |_| true, &log).is_none());
        a.update(keys(false, false), true, &c, &log);
        a.update(keys(true, false), true, &c, &log);
        a.observe_actor((1, 33, 1), None, None, [0; 3]);
        assert!(take(&a, &[], |_| true, &log).is_none());
    }
    #[test]
    fn expired_or_wrong_match_metadata_does_not_guess_or_queue_casts() {
        let (a, c, log) = setup(1, 6);
        a.0.lock().unwrap().metadata.as_mut().unwrap().at = Instant::now() - Duration::from_secs(1);
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &[], |_| true, &log).is_none());
        a.observe_metadata(
            (1, 33, 1),
            7,
            [Descriptor::from_fields(1, 6, 70_000, 0, 1, 0); 3],
            &log,
        );
        assert!(take(&a, &[], |_| true, &log).is_none());
        a.update(keys(false, false), true, &c, &log);
        a.update(keys(true, false), true, &c, &log);
        a.observe_actor((9, 34, 1), Some(7), Some((200_000, 200_000)), [0; 3]);
        assert!(take(&a, &[], |_| true, &log).is_none());
    }
    #[test]
    fn native_ranges_include_level_growth_and_bonus_and_reject_missing_effects() {
        assert_eq!(
            Descriptor::from_fields(1, 6, 70_000, 5_000, 3, 2_000)
                .unwrap()
                .range,
            82_000
        );
        assert!(Descriptor::from_fields(u32::MAX, 6, 70_000, 0, 1, 0).is_none());
        assert!(Descriptor::from_fields(1, 14, 70_000, 0, 1, 0).is_none());
        assert!(Descriptor::from_fields(1, 6, u64::MAX, 5_000, 3, 0).is_none());
    }
    #[test]
    fn held_right_button_does_not_block_new_casts_but_new_click_and_stop_cancel_pending() {
        let (a, c, log) = setup(1, 6);
        a.update(
            Keys {
                right: true,
                ..keys(false, false)
            },
            true,
            &c,
            &log,
        );
        a.update(
            Keys {
                right: true,
                ..keys(true, false)
            },
            true,
            &c,
            &log,
        );
        assert!(take(&a, &[], |_| true, &log).is_some());
        for cancellation in [
            Keys {
                stop: true,
                ..keys(false, false)
            },
            Keys {
                escape: true,
                ..keys(false, false)
            },
            Keys {
                right: true,
                ..keys(false, false)
            },
            Keys {
                release: true,
                ..keys(false, false)
            },
        ] {
            a.update(keys(false, false), true, &c, &log);
            a.update(keys(true, false), true, &c, &log);
            a.update(cancellation, true, &c, &log);
            assert!(take(&a, &[], |_| true, &log).is_none());
        }
    }
    #[test]
    fn all_slots_preview_only_and_normal_cast_once_with_snapshot_aim_at_zoom() {
        for slot in 0..3 {
            let (a, c, log) = setup(1, 6);
            let mut pressed = keys(false, true);
            pressed.abilities[slot] = true;
            a.update(pressed, true, &c, &log);
            assert!(take(&a, &[], |_| true, &log).is_none());
            assert!(a.status().unwrap().starts_with(NAMES[slot]));
            a.update(keys(false, false), true, &c, &log);
            pressed.shift = false;
            a.update(pressed, true, &c, &log);
            // Moving camera/cursor after the press must not change the cast aim.
            let mut frame = c.frame().unwrap();
            frame.extent = (512., 512.);
            frame.center = (400., 400.);
            c.capture(frame);
            a.update(
                Keys {
                    cursor: Some((1400., 600.)),
                    ..pressed
                },
                true,
                &c,
                &log,
            );
            assert_eq!(
                take(&a, &[], |_| true, &log),
                Some(InputV1::action(
                    KINDS[slot],
                    InputTargetV1::pos(300_000, 200_000)
                ))
            );
            assert!(take(&a, &[], |_| true, &log).is_none());
        }
    }
    #[test]
    fn normal_cast_survives_key_release_and_uses_confirmation_cursor_at_new_zoom() {
        let (a, c, log) = setup(1, 6);
        a.update(keys(true, true), true, &c, &log);
        a.update(keys(false, false), true, &c, &log);
        assert_eq!(a.0.lock().unwrap().preview, Some(0));
        assert!(take(&a, &[], |_| true, &log).is_none());
        let mut frame = c.frame().unwrap();
        frame.center = (400., 400.);
        frame.extent = (512., 512.);
        c.capture(frame);
        let action = a.update(
            Keys {
                left: true,
                cursor: Some((1360., 537.)),
                ..keys(false, false)
            },
            true,
            &c,
            &log,
        );
        assert!(action.left_reserved);
        assert_eq!(
            take(&a, &[], |_| true, &log),
            Some(InputV1::action(
                InputKindV1::Skill,
                InputTargetV1::pos(500_000, 400_000)
            ))
        );
        assert!(a.0.lock().unwrap().preview.is_none());
        a.update(
            Keys {
                left: true,
                ..keys(false, false)
            },
            true,
            &c,
            &log,
        );
        assert!(take(&a, &[], |_| true, &log).is_none());
    }
    #[test]
    fn invalid_normal_click_keeps_mode_without_autocasting_when_target_appears() {
        let (a, c, log) = setup(0, 6);
        a.update(keys(true, true), true, &c, &log);
        a.update(keys(false, false), true, &c, &log);
        assert!(
            a.update(
                Keys {
                    left: true,
                    ..keys(false, false)
                },
                true,
                &c,
                &log
            )
            .left_reserved
        );
        assert!(take(&a, &[], |_| true, &log).is_none());
        assert_eq!(a.0.lock().unwrap().preview, Some(0));
        let unit = [Unit {
            id: 29,
            position: (300_000, 200_000),
            radius: 10_000,
            is_champion: true,
            body: None,
        }];
        assert!(take(&a, &unit, |_| true, &log).is_none());
        a.update(keys(false, false), true, &c, &log);
        a.update(
            Keys {
                left: true,
                ..keys(false, false)
            },
            true,
            &c,
            &log,
        );
        assert_eq!(
            take(&a, &unit, |_| true, &log).unwrap().target,
            InputTargetV1::target(29)
        );
        assert!(a.0.lock().unwrap().preview.is_none());
    }
    #[test]
    fn new_commands_cancel_mode_and_switching_skill_does_not_cast() {
        for cancel in [
            Keys {
                right: true,
                ..keys(false, false)
            },
            Keys {
                attack_move: true,
                ..keys(false, false)
            },
            Keys {
                recall: true,
                ..keys(false, false)
            },
            Keys {
                stop: true,
                ..keys(false, false)
            },
            Keys {
                escape: true,
                ..keys(false, false)
            },
            Keys {
                focused: false,
                ..Keys::default()
            },
        ] {
            let (a, c, log) = setup(1, 6);
            a.update(keys(true, true), true, &c, &log);
            a.update(keys(false, false), true, &c, &log);
            a.update(cancel, true, &c, &log);
            assert!(a.0.lock().unwrap().preview.is_none());
            assert!(take(&a, &[], |_| true, &log).is_none());
        }
        let (a, c, log) = setup(1, 6);
        a.update(keys(true, true), true, &c, &log);
        a.update(
            Keys {
                shift: true,
                abilities: [false, true, false],
                ..keys(false, false)
            },
            true,
            &c,
            &log,
        );
        assert_eq!(a.0.lock().unwrap().preview, Some(1));
        assert!(take(&a, &[], |_| true, &log).is_none());
        a.update(keys(false, false), true, &c, &log);
        a.update(keys(true, false), true, &c, &log);
        assert!(a.0.lock().unwrap().preview.is_none());
        assert_eq!(
            take(&a, &[], |_| true, &log).unwrap().kind,
            InputKindV1::Skill.code()
        );
    }
    #[test]
    fn hud_click_leaves_aiming_open_and_camera_keys_do_not_cancel() {
        let (a, c, log) = setup(1, 6);
        a.update(keys(true, true), true, &c, &log);
        a.update(
            Keys {
                space: true,
                camera_toggle: true,
                team_info: true,
                ..keys(false, false)
            },
            true,
            &c,
            &log,
        );
        assert_eq!(a.0.lock().unwrap().preview, Some(0));
        c.set_command_blocked(vec![crate::camera::Rect {
            x: 1100.,
            y: 500.,
            w: 100.,
            h: 100.,
        }]);
        assert!(
            !a.update(
                Keys {
                    left: true,
                    ..keys(false, false)
                },
                true,
                &c,
                &log
            )
            .left_reserved
        );
        assert!(take(&a, &[], |_| true, &log).is_none());
        assert_eq!(a.0.lock().unwrap().preview, Some(0));
    }
    #[test]
    fn cancellation_during_native_validation_cannot_deliver_stale_cast() {
        let (a, c, log) = setup(1, 6);
        a.update(keys(true, false), true, &c, &log);
        assert!(take(
            &a,
            &[],
            |_| {
                a.update(
                    Keys {
                        recall: true,
                        ..keys(false, false)
                    },
                    true,
                    &c,
                    &log,
                );
                true
            },
            &log
        )
        .is_none());
    }
}
