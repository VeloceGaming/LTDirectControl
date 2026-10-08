//! Physical key edges -> quickcasts or persistent normal-cast aiming.
//! Only quickcast presses / battlefield confirmation clicks enqueue a cast.
//! No native pointers leave a hook.
use crate::{
    camera::CameraFrame, combat::Unit, native_timing::MatchKey, platform_input::Keys, Logger,
};
use mod_api_stable::{InputKindV1, InputTargetV1, InputV1, StableClient};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const FRESH: Duration = Duration::from_millis(250);
const BUFFER: Duration = Duration::from_secs(1);
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
    stamp: crate::input_trace::Stamp,
    normal_cast: bool,
    generation: u64,
    champion_only: bool,
    self_cast: bool,
    bound: Option<InputTargetV1>,
    waiting: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Charges {
    pub capacity: usize,
    pub cost: usize,
    pub uses: usize,
}
impl Charges {
    pub fn remaining(self, cooldown: usize) -> usize {
        (self.capacity.saturating_sub(cooldown) / self.cost.max(1)).min(self.uses)
    }
    pub fn multi(self) -> bool {
        self.cost > 0 && self.uses > 1
    }
    pub fn wait(self, cooldown: usize) -> usize {
        cooldown.saturating_sub(self.capacity.saturating_sub(self.cost))
    }
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
    pub uses: [Option<usize>; 3],
    pub ready: [Option<bool>; 3],
    pub wait: [Option<usize>; 3],
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
    dispatched: Option<CastRequest>,
    preview: Option<usize>,
    hud_hover: Option<(usize, Instant)>, // Visual only; never a cast request.
    aim: Option<Aim>,
    metadata: Option<Snapshot>,
    actor: Option<usize>,
    key: Option<MatchKey>,
    position: Option<(u64, u64)>,
    cooldowns: [usize; 3],
    level: Option<usize>,
    champion: Option<String>,
    champion_only: bool,
    message: Option<(String, Instant)>,
    native: Option<(usize, [Option<Charges>; 3], Instant)>,
    wait_state: Option<(u64, Option<usize>, bool)>,
    attack_cycle: Option<(usize, usize, Instant)>, // hit tick, starting cooldown, start; no native pointers
    last_uses: [Option<(usize, Instant)>; 3],
    units: Option<(Vec<Unit>, Instant)>,
    geometry: Option<([crate::skill_preview::Geometry; 3], Instant)>,
    geometry_reports: usize,
    release_slot: Option<usize>,
}
#[derive(Default)]
/// The third field mirrors `State::actor` (0 = none) so native hooks can
/// turn away other entities without taking the lock.
pub struct Abilities(Mutex<State>, bool, AtomicUsize);
impl State {
    /// The observed attack's hit tick has passed and its cooldown is the one
    /// started by that attack; ownership and data are current.
    fn attack_committed(
        &self,
        key: MatchKey,
        actor: usize,
        elapsed: usize,
        cooldown: usize,
    ) -> bool {
        let Some((hit, starting_cooldown, _)) = self.attack_cycle else {
            return false;
        };
        self.active
            && self.focused
            && self.key == Some(key)
            && self.actor == Some(actor)
            && self.updated.is_some_and(|at| at.elapsed() <= FRESH)
            && elapsed > hit
            && cooldown <= starting_cooldown
    }
    fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.preview = None;
        self.pending = None;
        self.dispatched = None;
        self.release_slot = None;
        self.wait_state = None;
    }
    fn cancel_logged(&mut self, reason: &str, log: &Logger) {
        if let Some(request) = self.pending.or(self.dispatched) {
            log.write(&format!(
                "ABILITY CANCEL {} reason={reason} trace_id={} age_ms={} waiting={} bound={:?}",
                NAMES[request.slot],
                request.stamp.id,
                request.at.elapsed().as_millis(),
                request.waiting,
                request.bound
            ));
        }
        self.cancel();
    }
    fn bind_cursor(&self, slot: usize, champion_only: bool) -> Option<InputTargetV1> {
        let d = self
            .metadata
            .as_ref()
            .filter(|m| m.at.elapsed() <= FRESH)?
            .skills[slot]?;
        if d.casting != 0 || d.target == 4 {
            return None;
        }
        let (units, _) = self
            .units
            .as_ref()
            .filter(|(_, at)| at.elapsed() <= FRESH)?;
        let aim = self.aim?;
        let actor = self.actor?;
        let picked = crate::combat::clicked_units(aim.frame, aim.cursor, units, champion_only)
            .into_iter()
            .find(|id| {
                units
                    .iter()
                    .any(|u| u.id == *id && eligible(d.target, actor, u))
            });
        // Misses cannot turn into hits when another unit crosses the cursor.
        Some(InputTargetV1::target(picked.unwrap_or(usize::MAX)))
    }
}
fn eligible(kind: u32, actor: usize, u: &Unit) -> bool {
    match kind {
        0 => u.friendly,
        1 => u.friendly && u.is_champion,
        2 => u.friendly && u.is_champion && u.in_cc,
        3 => u.friendly && u.id != actor,
        4 => u.id == actor,
        5 => !u.friendly,
        6 => !u.friendly && !u.is_tower,
        7 | 9 => !u.friendly && u.is_champion,
        8 => !u.friendly && u.is_champion && u.in_cc,
        10 => true,
        11 => !u.is_tower,
        12 => u.is_champion,
        _ => false,
    }
}
fn request_has_target(
    request: CastRequest,
    desc: Descriptor,
    actor: usize,
    units: &[Unit],
) -> bool {
    if desc.casting == 0 {
        let Some(target) = request.bound else {
            return false;
        };
        if target.target_id == actor {
            return matches!(desc.target, 0 | 1 | 4 | 10 | 11 | 12);
        }
        desc.target != 9
            && units
                .iter()
                .any(|u| u.id == target.target_id && eligible(desc.target, actor, u))
    } else {
        (request.self_cast && desc.casting == 1)
            || (!request.self_cast && (desc.casting == 3 || request.aim.is_some()))
    }
}

impl Abilities {
    pub fn selected_key(&self, actor: usize) -> Option<MatchKey> {
        // Lock-free rejection first: hooks see every entity of every running
        // simulation; only the observed actor goes on to the full check.
        if actor == 0 || self.2.load(Ordering::Acquire) != actor {
            crate::perf::hook(crate::perf::Hook::OwnerFast);
            return None;
        }
        let since = Instant::now();
        let s = self.0.lock().ok()?;
        crate::perf::waited(crate::perf::Wait::Abilities, since);
        (s.active && s.actor == Some(actor) && s.position.is_some())
            .then_some(s.key)
            .flatten()
    }
    pub fn observe_attack_started(
        &self,
        key: MatchKey,
        actor: usize,
        delay: usize,
        cooldown: usize,
    ) {
        if let Ok(mut s) = self.0.lock() {
            if s.active && s.key == Some(key) && s.actor == Some(actor) {
                s.attack_cycle = Some((delay, cooldown, Instant::now()));
            }
        }
    }
    /// Only a retained, learned and ready cast may request an attack backswing
    /// interruption. The native adapter separately proves the effect committed.
    pub fn attack_interrupt(
        &self,
        key: MatchKey,
        actor: usize,
        elapsed: usize,
        cooldown: usize,
    ) -> bool {
        self.0.lock().is_ok_and(|s| {
            if !s.attack_committed(key, actor, elapsed, cooldown) {
                return false;
            }
            let Some(request) = s.pending else {
                return false;
            };
            request.at.elapsed() <= BUFFER
                && request.waiting
                && s.level.is_some_and(|l| l >= [1, 3, 5][request.slot])
                && s.native
                    .filter(|(_, _, at)| at.elapsed() <= FRESH)
                    .and_then(|(_, c, _)| c[request.slot])
                    .is_some_and(|c| c.remaining(s.cooldowns[request.slot]) > 0)
        })
    }
    /// Start time of the observed attack once its hit tick has passed, for a
    /// newer manual move/stop. No skill request is required on this path.
    pub fn committed_attack_start(
        &self,
        key: MatchKey,
        actor: usize,
        elapsed: usize,
        cooldown: usize,
    ) -> Option<Instant> {
        let s = self.0.lock().ok()?;
        s.attack_committed(key, actor, elapsed, cooldown)
            .then(|| s.attack_cycle.map(|(_, _, at)| at))
            .flatten()
    }
    pub fn new(cast_on_release: bool) -> Self {
        Self(Mutex::default(), cast_on_release, AtomicUsize::new(0))
    }
    pub fn reset_session(&self, keys: Keys) {
        if let Ok(mut s) = self.0.lock() {
            let generation = s.generation.wrapping_add(1);
            *s = State::default();
            self.2.store(0, Ordering::Release);
            s.generation = generation;
            s.previous = keys.abilities;
            s.previous_right = keys.right || keys.attack_click;
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
            uses: std::array::from_fn(|i| {
                s.native
                    .filter(|(_, _, at)| at.elapsed() <= FRESH)
                    .and_then(|(_, c, _)| c[i])
                    .map(|c| c.multi().then(|| c.remaining(s.cooldowns[i])))
                    .unwrap_or_else(|| {
                        s.last_uses[i]
                            .filter(|(_, at)| at.elapsed() <= Duration::from_secs(1))
                            .map(|(n, _)| n)
                    })
            }),
            ready: std::array::from_fn(|i| {
                s.native
                    .filter(|(_, _, at)| at.elapsed() <= FRESH)
                    .and_then(|(_, c, _)| c[i])
                    .map(|c| c.remaining(s.cooldowns[i]) > 0)
            }),
            wait: std::array::from_fn(|i| {
                s.native
                    .filter(|(_, _, at)| at.elapsed() <= FRESH)
                    .and_then(|(_, c, _)| c[i])
                    .map(|c| c.wait(s.cooldowns[i]))
            }),
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
        let released = std::array::from_fn::<_, 3, _>(|i| !keys.abilities[i] && s.previous[i]);
        let right_down = keys.right || keys.attack_click;
        let right_click = right_down && !s.previous_right;
        let left_click = keys.left && !s.previous_left;
        let recall = keys.recall && !s.previous_recall;
        let attack_move = keys.attack_move && !s.previous_attack_move;
        s.previous_right = keys.right || keys.attack_click;
        s.previous_left = keys.left;
        s.previous_recall = keys.recall;
        s.previous_attack_move = keys.attack_move;
        s.previous = keys.abilities;
        s.active = active;
        s.focused = keys.focused;
        s.champion_only = keys.champion_only;
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
            || recall
            || attack_move
        {
            let reason = if !armed {
                "control-inactive-or-focus-changed"
            } else if keys.stop {
                "stop"
            } else if keys.escape {
                "escape"
            } else if keys.release {
                "return-to-ai"
            } else if s.position.is_none() {
                "actor-unavailable"
            } else if recall {
                "recall"
            } else {
                "attack-move"
            };
            s.cancel_logged(reason, log);
            return action;
        }
        let ability_pressed = edges.into_iter().any(|edge| edge);
        // Physical polling cannot order two edges inside the same frame. A new
        // skill wins this tie; a later click still cancels the retained skill.
        if right_click && !ability_pressed {
            s.cancel_logged("later-right-click", log);
            return action;
        }
        for (slot, edge) in edges.into_iter().enumerate() {
            if !edge {
                continue;
            }
            let preview = if keys.mapped {
                keys.previews[slot] || keys.cast_modes[slot] != 0
            } else {
                keys.shift || self.1
            };
            let self_cast = if keys.mapped {
                keys.self_casts[slot]
            } else {
                keys.alt
            };
            if preview && !self_cast {
                s.cancel_logged("new-aiming-command", log);
                s.preview = Some(slot);
                if (keys.mapped && keys.cast_modes[slot] == 2 && !keys.previews[slot])
                    || (!keys.mapped && self.1 && !keys.shift)
                {
                    s.release_slot = Some(slot);
                }
                action.disarm_attack_move = true;
                log.write(&format!(
                    "ABILITY AIMING {} selected; release keeps mode; left-click confirms",
                    NAMES[slot]
                ));
            } else {
                s.cancel_logged("newer-skill", log);
                action.disarm_attack_move = true;
                action.new_cast = true;
                // One pending cast; simultaneous polled edges use R > W > Q.
                s.pending = Some(CastRequest {
                    stamp: crate::input_trace::Stamp::new(NAMES[slot]),
                    slot,
                    aim: s.aim,
                    at: Instant::now(),
                    normal_cast: false,
                    generation: s.generation,
                    champion_only: keys.champion_only,
                    self_cast: if keys.mapped {
                        keys.self_casts[slot]
                    } else {
                        keys.alt
                    },
                    waiting: false,
                    bound: if (keys.mapped && keys.self_casts[slot]) || (!keys.mapped && keys.alt) {
                        None
                    } else {
                        s.bind_cursor(slot, keys.champion_only)
                    },
                });
                log.write(&format!(
                    "ABILITY PRESS {} cursor={:?}",
                    NAMES[slot], keys.cursor
                ));
            }
        }
        if let Some(slot) = s.release_slot.filter(|slot| released[*slot]) {
            s.release_slot = None;
            s.generation = s.generation.wrapping_add(1);
            s.pending = Some(CastRequest {
                stamp: crate::input_trace::Stamp::new(NAMES[slot]),
                slot,
                aim: s.aim,
                at: Instant::now(),
                normal_cast: true,
                generation: s.generation,
                champion_only: keys.champion_only,
                self_cast: false,
                waiting: false,
                bound: s.bind_cursor(slot, keys.champion_only),
            });
            action.new_cast = true;
            log.write(&format!(
                "ABILITY RELEASE {} cursor={:?}",
                NAMES[slot], keys.cursor
            ));
        }
        if left_click && !ability_pressed && s.aim.is_some() {
            if let Some(slot) = s.preview {
                s.release_slot = None;
                s.generation = s.generation.wrapping_add(1);
                s.pending = Some(CastRequest {
                    stamp: crate::input_trace::Stamp::new(NAMES[slot]),
                    slot,
                    aim: s.aim,
                    at: Instant::now(),
                    normal_cast: true,
                    generation: s.generation,
                    champion_only: keys.champion_only,
                    self_cast: if keys.mapped {
                        keys.self_casts[slot]
                    } else {
                        keys.alt
                    },
                    waiting: false,
                    bound: if (keys.mapped && keys.self_casts[slot]) || (!keys.mapped && keys.alt) {
                        None
                    } else {
                        s.bind_cursor(slot, keys.champion_only)
                    },
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
            if s.key != Some(key) || s.actor != actor {
                s.geometry_reports = 0;
            }
            if s.key != Some(key) || s.actor != actor || position.is_none() {
                s.cancel();
                s.metadata = None;
                s.level = None;
                s.champion = None;
                s.native = None;
                s.attack_cycle = None;
                s.last_uses = [None; 3];
                s.units = None;
                s.geometry = None;
            }
            s.key = Some(key);
            s.actor = actor;
            if self.2.load(Ordering::Relaxed) != actor.unwrap_or(0) {
                self.2.store(actor.unwrap_or(0), Ordering::Release);
            }
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
    pub fn observe_champion(&self, key: MatchKey, actor: usize, name: Option<String>) {
        if let Ok(mut s) = self.0.lock() {
            if s.key == Some(key) && s.actor == Some(actor) {
                if s.champion != name {
                    // Refresh diagnostics after identity/declaration discovery.
                    s.geometry = None;
                }
                s.champion = name;
            }
        }
    }
    pub fn observe_units(&self, key: MatchKey, actor: usize, units: &[Unit]) {
        if let Ok(mut s) = self.0.lock() {
            if s.key == Some(key) && s.actor == Some(actor) {
                s.units = Some((units.to_vec(), Instant::now()));
            }
        }
    }
    pub fn geometry_due(&self, key: MatchKey, actor: usize) -> bool {
        self.0.lock().is_ok_and(|s| {
            s.key == Some(key)
                && s.actor == Some(actor)
                && s.geometry
                    .as_ref()
                    .is_none_or(|(_, at)| at.elapsed() >= Duration::from_millis(100))
        })
    }
    pub fn observe_geometry(
        &self,
        key: MatchKey,
        actor: usize,
        geometry: [crate::skill_preview::Geometry; 3],
        log: &Logger,
    ) {
        if let Ok(mut s) = self.0.lock() {
            if s.key != Some(key) || s.actor != Some(actor) {
                return;
            }
            let changed = s.geometry.as_ref().is_none_or(|(old, _)| old != &geometry);
            if changed && s.geometry_reports < 24 {
                for (slot, g) in geometry.iter().enumerate() {
                    let declared = crate::skill_preview::geometry(s.champion.as_deref(), slot);
                    let source = if !g.footprints.is_empty() {
                        "runtime"
                    } else if !declared.footprints.is_empty() {
                        "declaration-fallback"
                    } else {
                        "limited-guide"
                    };
                    log.write(&format!("PREVIEW GEOMETRY actor={actor} key={key:?} slot={} source={source} native={g:?} declared={declared:?}",NAMES[slot]));
                }
                s.geometry_reports += 1;
            }
            s.geometry = Some((geometry, Instant::now()));
        }
    }
    pub fn observe_native(
        &self,
        key: MatchKey,
        actor: usize,
        action: usize,
        charges: [Option<Charges>; 3],
    ) -> bool {
        if let Ok(mut s) = self.0.lock() {
            if s.key == Some(key) && s.actor == Some(actor) {
                let changed = s.native.is_none_or(|(_, old, _)| old != charges);
                for (i, c) in charges.iter().enumerate() {
                    if let Some(c) = c {
                        s.last_uses[i] = c
                            .multi()
                            .then(|| (c.remaining(s.cooldowns[i]), Instant::now()));
                    }
                }
                if action != 3 {
                    s.attack_cycle = None;
                }
                s.native = Some((action, charges, Instant::now()));
                return changed;
            }
        }
        false
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
        let (mut request, desc, level, cooldown, native) = {
            let mut s = self.0.lock().ok()?;
            let request = s.pending.take()?;
            if !s.active
                || !s.focused
                || s.key != Some(key)
                || s.actor != Some(actor)
                || s.updated.is_none_or(|t| t.elapsed() > FRESH)
                || request.at.elapsed() > BUFFER
            {
                let reason = if !s.active || !s.focused {
                    "control-inactive-or-focus-changed"
                } else if s.key != Some(key) || s.actor != Some(actor) {
                    "identity-changed"
                } else if request.at.elapsed() > BUFFER {
                    "buffer-expired"
                } else {
                    "client-heartbeat-stale"
                };
                log.write(&format!(
                    "ABILITY DROP {} trace_id={} reason={reason} age_ms={} waiting={} bound={:?}",
                    NAMES[request.slot],
                    request.stamp.id,
                    request.at.elapsed().as_millis(),
                    request.waiting,
                    request.bound
                ));
                s.wait_state = None;
                return None;
            }
            let desc = s
                .metadata
                .as_ref()
                .filter(|m| m.actor == actor && m.key == key && m.at.elapsed() <= FRESH)
                .and_then(|m| m.skills[request.slot]);
            (
                request,
                desc,
                s.level,
                s.cooldowns[request.slot],
                s.native.filter(|(_, _, at)| at.elapsed() <= FRESH),
            )
        };
        let mut approaching = None;
        let busy = native.is_some_and(|(action, _, _)| action > 2);
        let charged = native
            .and_then(|(_, c, _)| c[request.slot])
            .map_or(cooldown == 0, |c| c.remaining(cooldown) > 0);
        let learned = level.is_none_or(|l| l >= [1, 3, 5][request.slot]);
        let mut reason = "metadata-unavailable-or-stale";
        let result = desc.and_then(|d| {
            reason = "native-validation-rejected";
            let make = |target| InputV1::action(KINDS[request.slot], target);
            let target = if let Some(bound) = request.bound {
                bound
            } else {
                let target = if request.self_cast {
                    match d.casting {
                        0 => InputTargetV1::target(actor),
                        1 => InputTargetV1::pos(position.0, position.1),
                        _ => {
                            reason = "self-cast-unsupported";
                            return None;
                        }
                    }
                } else {
                    match d.casting {
                        0 if d.target == 4 => InputTargetV1::target(actor),
                        0 => {
                            let aim = request.aim.or_else(|| {
                                reason = "aim-unavailable";
                                None
                            })?;
                            let hits = crate::combat::clicked_units(
                                aim.frame,
                                aim.cursor,
                                units,
                                request.champion_only,
                            );
                            // Try native-valid overlaps first; only semantically
                            // eligible hits may become a busy/approach intent.
                            let id = hits
                                .iter()
                                .copied()
                                .find(|id| valid(&make(InputTargetV1::target(*id))))
                                .or_else(|| {
                                    hits.into_iter().find(|id| {
                                        units
                                            .iter()
                                            .any(|u| u.id == *id && eligible(d.target, actor, u))
                                    })
                                })
                                .or_else(|| {
                                    reason = "cursor-missed-eligible-target";
                                    None
                                })?;
                            InputTargetV1::target(id)
                        }
                        1 => {
                            let aim = request.aim.or_else(|| {
                                reason = "aim-unavailable";
                                None
                            })?;
                            InputTargetV1::pos(aim.world.0, aim.world.1)
                        }
                        2 => {
                            let aim = request.aim.or_else(|| {
                                reason = "aim-unavailable";
                                None
                            })?;
                            let x = aim.world.0 as i64 - position.0 as i64;
                            let y = aim.world.1 as i64 - position.1 as i64;
                            if x == 0 && y == 0 {
                                reason = "zero-direction";
                                return None;
                            }
                            InputTargetV1::dir(x, y)
                        }
                        _ => InputTargetV1::NONE,
                    }
                };
                // Retain a unit identity or world point, never a cursor hit
                // search on each retry. Directions are rebuilt at cast start.
                if d.casting != 2 {
                    request.bound = Some(target);
                }
                target
            };
            if target.kind == 1
                && target.target_id != actor
                && !units.iter().any(|u| u.id == target.target_id)
            {
                reason = "bound-target-disappeared";
                return None;
            }
            let input = make(target);
            if !busy && valid(&input) {
                reason = "dispatched";
                return Some(input);
            }
            if !learned {
                reason = "unlearned";
            } else if !charged {
                reason = "no-charge-or-cooldown";
            } else if busy {
                reason = "native-action-busy";
            }
            if learned && charged && d.casting == 0 && target.target_id != actor {
                let unit = units
                    .iter()
                    .find(|u| u.id == target.target_id && eligible(d.target, actor, u))
                    .or_else(|| {
                        reason = "bound-target-no-longer-eligible";
                        None
                    })?;
                let reach = d.range.saturating_add(unit.radius);
                let dx = position.0.abs_diff(unit.position.0) as u128;
                let dy = position.1.abs_diff(unit.position.1) as u128;
                // Recently-attacked restrictions have no stable query. Never
                // start chasing such a target from an unvalidated request.
                if d.target != 9 && dx * dx + dy * dy > u128::from(reach).pow(2) {
                    approaching = Some(unit.position);
                    reason = "walking-into-range";
                }
            }
            None
        });
        let retain = result.is_none()
            && desc.is_some()
            && learned
            && charged
            && (approaching.is_some()
                || ((busy || (native.is_some() && !request.waiting))
                    && request_has_target(request, desc.unwrap(), actor, units)));
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
            if retain {
                let wait_state = (
                    request.stamp.id,
                    native.map(|(action, _, _)| action),
                    approaching.is_some(),
                );
                if s.wait_state != Some(wait_state) {
                    log.write(&format!("ABILITY WAIT {} trace_id={} reason={reason} native_action={:?} busy={busy} approaching={approaching:?} bound={:?} world={:?}", NAMES[request.slot], request.stamp.id, wait_state.1, request.bound, request.aim.map(|a| a.world)));
                    s.wait_state = Some(wait_state);
                }
                request.waiting = true;
                if approaching.is_some() && !busy {
                    request.at = Instant::now();
                }
                s.pending = Some(request);
            }
            if (result.is_some() || retain) && request.normal_cast {
                s.preview = None;
            }
            if !retain {
                s.wait_state = None;
                s.message = Some((message, Instant::now()));
            }
            if result.is_some() {
                s.dispatched = Some(request);
            }
        } else {
            return None;
        }
        if !retain {
            log.write(&format!(
                "ABILITY RESULT {} trace_id={} reason={reason} native_action={:?} cooldown={cooldown} charged={charged} learned={learned} waiting={} bound={:?} normal_cast={} self_cast={} champion_only={} descriptor={desc:?} input={result:?} aim_world={:?} actor_position={position:?}; dispatched requests await native acknowledgement",
                NAMES[request.slot], request.stamp.id, native.map(|(action, _, _)| action), request.waiting, request.bound, request.normal_cast, request.self_cast, request.champion_only, request.aim.map(|a| a.world)
            ));
        }
        result.or_else(|| {
            retain.then(|| {
                let goal = if busy {
                    position
                } else {
                    approaching.unwrap_or(position)
                };
                InputV1::move_to(goal.0, goal.1)
            })
        })
    }

    /// Native point/direction variants initialize all three target words.
    /// Do not read inactive payload words of a target/None enum for tracing.
    pub fn trace_point_stamp(&self, slot: usize) -> Option<crate::input_trace::Stamp> {
        let s = self.0.lock().ok()?;
        let request = s
            .dispatched
            .filter(|r| r.slot == slot && r.generation == s.generation)?;
        let descriptor = s.metadata.as_ref()?.skills[slot]?;
        matches!(descriptor.casting, 1 | 2).then_some(request.stamp)
    }
    pub fn dispatched_stamp(&self, kind: u32) -> Option<crate::input_trace::Stamp> {
        let s = self.0.lock().ok()?;
        let request = s.dispatched?;
        (KINDS[request.slot].code() == kind).then_some(request.stamp)
    }
    /// Called immediately around the real native skill consumer. Validation
    /// accepts a command shape; spending cooldown confirms execution.
    pub fn acknowledge_native(
        &self,
        key: MatchKey,
        actor: usize,
        slot: usize,
        spent: bool,
        action: usize,
        log: &Logger,
    ) {
        let Ok(mut s) = self.0.lock() else { return };
        let Some(mut request) = s.dispatched else {
            return;
        };
        if s.key != Some(key)
            || s.actor != Some(actor)
            || request.slot != slot
            || request.generation != s.generation
        {
            return;
        }
        s.dispatched = None;
        if spent {
            log.write(&format!(
                "ABILITY EXECUTED {} native cooldown spent; action={action} trace_id={} capture_age_us={}",
                NAMES[slot], request.stamp.id, request.stamp.at.elapsed().as_micros()
            ));
        } else if action > 2 && request.at.elapsed() <= BUFFER {
            request.waiting = true;
            s.pending = Some(request);
            log.write(&format!(
                "ABILITY RETRY {} trace_id={} native busy action={action}; original intent retained",
                NAMES[slot], request.stamp.id
            ));
        } else {
            s.message = Some((format!("{} could not start", NAMES[slot]), Instant::now()));
            log.write(&format!(
                "ABILITY REJECTED {} trace_id={} reason=native-consumer-did-not-spend-cooldown action={action} age_ms={} waiting={} bound={:?}",
                NAMES[slot], request.stamp.id, request.at.elapsed().as_millis(), request.waiting, request.bound
            ));
        }
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
    pub fn set_hud_hover(&self, slot: Option<usize>) {
        if let Ok(mut s) = self.0.lock() {
            s.hud_hover = slot.filter(|i| *i < 3).map(|i| (i, Instant::now()));
        }
    }
    fn preview_data(&self) -> Option<(crate::skill_preview::Preview, bool)> {
        let s = self.0.lock().ok()?;
        if !s.active || !s.focused || s.updated.is_none_or(|t| t.elapsed() > FRESH) {
            return None;
        }
        let hover = s.preview.is_none();
        let slot = s.preview.or_else(|| {
            s.hud_hover
                .filter(|(_, at)| at.elapsed() <= FRESH)
                .map(|(i, _)| i)
        })?;
        let metadata = s.metadata.as_ref().filter(|m| m.at.elapsed() <= FRESH)?;
        let desc = metadata.skills[slot]?;
        let ready = s.level.is_some_and(|level| level >= [1, 3, 5][slot])
            && s.native
                .filter(|(_, _, at)| at.elapsed() <= FRESH)
                .and_then(|(_, c, _)| c[slot])
                .map_or(s.cooldowns[slot] == 0, |c| {
                    c.remaining(s.cooldowns[slot]) > 0
                });
        let target = if !hover && desc.casting == 0 {
            let picked = if desc.target == 4 {
                s.actor.map(InputTargetV1::target)
            } else {
                s.bind_cursor(slot, s.champion_only)
            };
            picked.and_then(|t| {
                s.units
                    .as_ref()
                    .filter(|(_, at)| at.elapsed() <= FRESH)?
                    .0
                    .iter()
                    .find(|u| u.id == t.target_id)
                    .copied()
            })
        } else {
            None
        };
        Some((
            crate::skill_preview::Preview {
                origin: s.position?,
                aim: (!hover).then(|| s.aim.map(|a| a.world)).flatten(),
                casting: desc.casting,
                self_target: desc.target == 4,
                range: desc.range,
                ready,
                geometry: crate::skill_preview::resolve(
                    s.geometry
                        .as_ref()
                        .filter(|(_, at)| at.elapsed() <= FRESH)
                        .map(|(g, _)| &g[slot]),
                    crate::skill_preview::geometry(s.champion.as_deref(), slot),
                ),
                target,
            },
            hover,
        ))
    }
    pub fn draw(&self, ctx: &mut StableClient<'_>, camera: &crate::camera::CameraControl) {
        let data = self.preview_data();
        let (Some(data), Some(frame)) = (data, camera.frame()) else {
            return;
        };
        let mut blockers = camera.overlay_blockers();
        blockers.push(frame.minimap);
        let (data, hover) = data;
        let drawing = if hover {
            crate::skill_preview::hover_drawing(frame, data)
        } else {
            crate::skill_preview::drawing(frame, data)
        };
        drawing.render(ctx, frame, &blockers);
    }
    /// Unit-target aiming replaces passive hover with that skill's eligible
    /// winner. Some(None) deliberately clears hover when aiming misses.
    pub fn hover_target(&self, self_cast: bool) -> Option<Option<Unit>> {
        let s = self.0.lock().ok()?;
        let slot = s.preview?;
        let desc = s
            .metadata
            .as_ref()
            .filter(|m| m.at.elapsed() <= FRESH)?
            .skills[slot]?;
        if desc.casting != 0 {
            return None;
        }
        if self_cast || desc.target == 4 {
            return Some(None);
        }
        Some(s.bind_cursor(slot, s.champion_only).and_then(|target| {
            let (units, _) = s.units.as_ref().filter(|(_, at)| at.elapsed() <= FRESH)?;
            units.iter().find(|u| u.id == target.target_id).copied()
        }))
    }
    pub fn cursor_feedback(&self, self_cast: bool) -> Option<crate::cursor::SkillCursor> {
        use crate::cursor::SkillCursor;
        let s = self.0.lock().ok()?;
        let slot = s.preview?;
        let valid = (|| {
            let desc = s
                .metadata
                .as_ref()
                .filter(|m| m.at.elapsed() <= FRESH)?
                .skills[slot]?;
            let ready = s.level.is_some_and(|l| l >= [1, 3, 5][slot])
                && s.native
                    .filter(|(_, _, at)| at.elapsed() <= FRESH)
                    .and_then(|(_, c, _)| c[slot])
                    .map_or(s.cooldowns[slot] == 0, |c| {
                        c.remaining(s.cooldowns[slot]) > 0
                    });
            if !ready {
                return Some(SkillCursor::Invalid);
            }
            if desc.casting != 0 {
                return Some(SkillCursor::Free);
            }
            if desc.target == 4 {
                return Some(SkillCursor::Valid);
            }
            if self_cast {
                let actor = s.actor?;
                let (units, _) = s.units.as_ref().filter(|(_, at)| at.elapsed() <= FRESH)?;
                return Some(
                    if units
                        .iter()
                        .any(|u| u.id == actor && eligible(desc.target, actor, u))
                    {
                        SkillCursor::Valid
                    } else {
                        SkillCursor::Invalid
                    },
                );
            }
            Some(
                if s.bind_cursor(slot, s.champion_only)
                    .is_some_and(|t| t.target_id != usize::MAX)
                {
                    SkillCursor::Valid
                } else {
                    SkillCursor::Invalid
                },
            )
        })();
        Some(valid.unwrap_or(SkillCursor::Invalid))
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
    #[test]
    fn normal_cast_cursor_uses_ally_eligibility_and_preserves_out_of_range_targets() {
        use crate::cursor::SkillCursor;
        let (a, c, log) = setup(0, 1);
        a.observe_level((1, 33, 1), 7, 5);
        let ally = Unit {
            friendly: true,
            ..enemy(29, (300_000, 200_000))
        };
        let hostile = Unit {
            id: 30,
            friendly: false,
            ..ally
        };
        a.observe_units((1, 33, 1), 7, &[hostile, ally]);
        a.update(keys(true, true), true, &c, &log);
        assert_eq!(a.cursor_feedback(false), Some(SkillCursor::Valid));
        assert_eq!(a.hover_target(false).flatten().unwrap().id, ally.id);
        // Known range does not turn an approach-to-cast order into a rejected click.
        a.0.lock().unwrap().metadata.as_mut().unwrap().skills[0]
            .as_mut()
            .unwrap()
            .range = 10_000;
        assert_eq!(a.cursor_feedback(false), Some(SkillCursor::Valid));
        a.observe_units((1, 33, 1), 7, &[enemy(29, ally.position)]);
        assert_eq!(a.cursor_feedback(false), Some(SkillCursor::Invalid));
        assert!(matches!(a.hover_target(false), Some(None)));
        a.observe_units(
            (1, 33, 1),
            7,
            &[Unit {
                id: 7,
                position: (200_000, 200_000),
                ..ally
            }],
        );
        assert_eq!(a.cursor_feedback(true), Some(SkillCursor::Valid));
        a.0.lock().unwrap().metadata.as_mut().unwrap().skills[0]
            .as_mut()
            .unwrap()
            .target = 2;
        assert_eq!(a.cursor_feedback(true), Some(SkillCursor::Invalid));
    }
    #[test]
    fn simultaneous_click_skill_keeps_one_cast_but_a_later_click_cancels() {
        let (a, c, log) = setup(2, 5);
        let press = Keys {
            right: true,
            ..keys(true, false)
        };
        a.update(press, true, &c, &log);
        assert!(take(&a, &[], |_| true, &log).is_some());
        assert!(take(&a, &[], |_| true, &log).is_none());
        a.update(keys(false, false), true, &c, &log);
        budget(&a, 3);
        a.update(press, true, &c, &log);
        assert_eq!(
            take(&a, &[], |_| false, &log).unwrap().kind,
            InputKindV1::Move.code()
        );
        a.update(keys(false, false), true, &c, &log);
        a.update(
            Keys {
                right: true,
                ..keys(false, false)
            },
            true,
            &c,
            &log,
        );
        budget(&a, 0);
        assert!(take(&a, &[], |_| true, &log).is_none());
    }
    #[test]
    fn backswing_requires_observed_attack_commit_and_retained_ready_cast() {
        let (a, c, log) = setup(2, 5);
        a.observe_level((1, 33, 1), 7, 1);
        budget(&a, 3);
        a.observe_attack_started((1, 33, 1), 7, 5, 60);
        a.update(keys(true, false), true, &c, &log);
        take(&a, &[], |_| false, &log);
        assert!(!a.attack_interrupt((1, 33, 1), 7, 5, 55));
        assert!(a.attack_interrupt((1, 33, 1), 7, 6, 54));
        assert!(!a.attack_interrupt((1, 33, 1), 8, 6, 54));
        assert!(!a.attack_interrupt((1, 34, 1), 7, 6, 54));
        assert!(!a.attack_interrupt((1, 33, 1), 7, 6, 61));
        a.clear_commands();
        assert!(!a.attack_interrupt((1, 33, 1), 7, 6, 54));
    }
    #[test]
    fn manual_release_needs_only_the_passed_hit_tick_and_current_attack() {
        let (a, _c, _log) = setup(2, 5);
        budget(&a, 3);
        assert!(a.committed_attack_start((1, 33, 1), 7, 20, 30).is_none());
        a.observe_attack_started((1, 33, 1), 7, 13, 50);
        // Hit tick 13 must be strictly passed; no skill request is needed.
        assert!(a.committed_attack_start((1, 33, 1), 7, 13, 37).is_none());
        assert!(a.committed_attack_start((1, 33, 1), 7, 14, 36).is_some());
        assert!(a.committed_attack_start((1, 33, 1), 8, 14, 36).is_none());
        assert!(a.committed_attack_start((1, 34, 1), 7, 14, 36).is_none());
        // A cooldown above the attack's own start belongs to a later attack.
        assert!(a.committed_attack_start((1, 33, 1), 7, 14, 51).is_none());
        // Leaving action 3 forgets the attack.
        budget(&a, 0);
        assert!(a.committed_attack_start((1, 33, 1), 7, 14, 36).is_none());
    }
    #[test]
    fn declared_uses_prevent_rounding_phantom_counts_and_reject_single_use() {
        let c = Charges {
            capacity: 5,
            cost: 1,
            uses: 3,
        };
        assert_eq!(c.remaining(0), 3); // capacity/cost alone would report five.
        assert!(!Charges {
            capacity: 3,
            cost: 1,
            uses: 1
        }
        .multi());
        let (a, _camera, _log) = setup(2, 5);
        budget(&a, 0);
        a.0.lock().unwrap().native.as_mut().unwrap().2 =
            Instant::now() - Duration::from_millis(260);
        assert_eq!(a.hud_skills().uses[0], Some(3));
        assert_eq!(a.hud_skills().ready[0], None); // display retention never authorizes a cast.
        a.observe_native(
            (1, 33, 1),
            7,
            0,
            [Some(Charges {
                capacity: 3,
                cost: 1,
                uses: 1,
            }); 3],
        );
        assert_eq!(a.hud_skills().uses[0], None);
        a.reset_session(Keys::default());
        assert_eq!(a.hud_skills().uses[0], None);
    }
    fn budget(a: &Abilities, action: usize) {
        a.observe_native(
            (1, 33, 1),
            7,
            action,
            [Some(Charges {
                capacity: 360,
                cost: 120,
                uses: 3,
            }); 3],
        );
    }
    #[test]
    fn optional_release_cast_uses_release_cursor_but_shift_stays_normal_cast() {
        let (mut a, c, log) = setup(1, 5);
        a.1 = true;
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &[], |_| true, &log).is_none());
        let release = Keys {
            cursor: Some((1000., 537.)),
            ..keys(false, false)
        };
        a.update(release, true, &c, &log);
        assert_eq!(
            take(&a, &[], |_| true, &log).unwrap().target,
            InputTargetV1::pos(220_000, 200_000)
        );
        a.update(keys(true, true), true, &c, &log);
        a.update(release, true, &c, &log);
        assert!(take(&a, &[], |_| true, &log).is_none());
        assert_eq!(a.hud_skills().aiming, Some(0));
        a.update(
            Keys {
                left: true,
                ..release
            },
            true,
            &c,
            &log,
        );
        assert!(take(&a, &[], |_| true, &log).is_some());
        a.update(keys(true, false), true, &c, &log);
        a.update(
            Keys {
                right: true,
                ..release
            },
            true,
            &c,
            &log,
        );
        assert!(take(&a, &[], |_| true, &log).is_none());
    }
    #[test]
    fn target_is_picked_at_press_and_a_miss_cannot_become_a_later_hit() {
        let (a, c, log) = setup(0, 7);
        let u = enemy(29, (300_000, 200_000));
        a.observe_units((1, 33, 1), 7, &[u]);
        a.update(keys(true, false), true, &c, &log);
        let moved = enemy(29, (280_000, 200_000));
        assert_eq!(
            take(&a, &[enemy(30, u.position), moved], |_| true, &log)
                .unwrap()
                .target
                .target_id,
            29
        );
        a.update(keys(false, false), true, &c, &log);
        a.observe_units((1, 33, 1), 7, &[]);
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &[u], |_| true, &log).is_none());
    }
    fn enemy(id: usize, position: (u64, u64)) -> Unit {
        Unit {
            id,
            position,
            radius: 10_000,
            is_champion: true,
            is_minion: false,
            friendly: false,
            in_cc: false,
            is_tower: false,
            body: None,
        }
    }
    #[test]
    fn busy_cast_keeps_target_identity_and_casts_once_after_animation() {
        let (a, c, log) = setup(0, 7);
        budget(&a, 3);
        a.update(keys(true, false), true, &c, &log);
        let u = enemy(29, (300_000, 200_000));
        assert_eq!(
            take(&a, &[u], |_| false, &log),
            Some(InputV1::move_to(200_000, 200_000))
        );
        a.update(
            Keys {
                cursor: Some((800., 537.)),
                ..keys(false, false)
            },
            true,
            &c,
            &log,
        );
        budget(&a, 0);
        let moved = enemy(29, (280_000, 200_000));
        let other = enemy(30, u.position);
        let cast = take(&a, &[moved, other], |_| true, &log).unwrap();
        assert_eq!(cast.target.target_id, 29);
        assert!(take(&a, &[moved, other], |_| true, &log).is_none());
    }
    #[test]
    fn busy_native_action_cannot_consume_valid_cast_and_acknowledgement_prevents_replay() {
        let (a, c, log) = setup(1, 5);
        budget(&a, 3);
        a.update(keys(true, false), true, &c, &log);
        assert_eq!(
            take(&a, &[], |_| true, &log),
            Some(InputV1::move_to(200_000, 200_000))
        );
        budget(&a, 0);
        let cast = take(&a, &[], |_| true, &log).unwrap();
        a.acknowledge_native((1, 33, 1), 7, 1, false, 3, &log); // Different slot cannot dismiss Q.
        assert!(a.0.lock().unwrap().dispatched.is_some());
        a.acknowledge_native((1, 33, 1), 7, 0, false, 3, &log);
        budget(&a, 0);
        assert_eq!(take(&a, &[], |_| true, &log), Some(cast));
        a.acknowledge_native((1, 33, 1), 7, 0, true, 4, &log);
        assert!(take(&a, &[], |_| true, &log).is_none());
        assert!(a.0.lock().unwrap().dispatched.is_none());
    }
    #[test]
    fn busy_queue_expires_and_cooldown_rejection_never_waits_for_recharge() {
        let (a, c, log) = setup(1, 5);
        budget(&a, 4);
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &[], |_| false, &log).is_some());
        a.0.lock().unwrap().pending.as_mut().unwrap().at = Instant::now() - Duration::from_secs(2);
        assert!(take(&a, &[], |_| true, &log).is_none());
        a.update(keys(false, false), true, &c, &log);
        a.observe_actor((1, 33, 1), Some(7), Some((200_000, 200_000)), [360, 0, 0]);
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &[], |_| false, &log).is_none());
        a.observe_actor((1, 33, 1), Some(7), Some((200_000, 200_000)), [0; 3]);
        assert!(take(&a, &[], |_| true, &log).is_none());
    }
    #[test]
    fn newly_started_action_gets_one_refresh_but_other_rejections_do_not_queue() {
        let (a, c, log) = setup(1, 5);
        budget(&a, 0); // Last observation predates a newly started action.
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &[], |_| false, &log).is_some());
        budget(&a, 4);
        assert!(take(&a, &[], |_| false, &log).is_some());
        budget(&a, 0);
        assert!(take(&a, &[], |_| true, &log).is_some());
        a.update(keys(false, false), true, &c, &log);
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &[], |_| false, &log).is_some());
        assert!(take(&a, &[], |_| false, &log).is_none());
        assert!(take(&a, &[], |_| true, &log).is_none());
    }
    #[test]
    fn newer_skill_replaces_buffer_and_direction_uses_frozen_world_aim() {
        let (a, c, log) = setup(2, 5);
        budget(&a, 3);
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &[], |_| false, &log).is_some());
        a.update(
            Keys {
                abilities: [false, true, false],
                ..keys(false, false)
            },
            true,
            &c,
            &log,
        );
        assert!(take(&a, &[], |_| false, &log).is_some());
        a.update(
            Keys {
                cursor: Some((700., 700.)),
                ..keys(false, false)
            },
            true,
            &c,
            &log,
        );
        budget(&a, 0);
        let cast = a
            .take_input(
                (1, 33, 1),
                7,
                (220_000, 210_000),
                &[enemy(99, (301_000, 200_000))],
                |_| true,
                &log,
            )
            .unwrap();
        assert_eq!(cast.kind, InputKindV1::Skill2.code());
        assert_eq!(cast.target, InputTargetV1::dir(80_000, -10_000));
    }
    #[test]
    fn targeted_cast_approaches_then_casts_on_same_unit_and_cancels_on_order() {
        let (a, c, log) = setup(0, 7);
        budget(&a, 0);
        a.update(
            Keys {
                cursor: Some((1660., 537.)),
                ..keys(true, false)
            },
            true,
            &c,
            &log,
        );
        let u = enemy(29, (550_000, 200_000));
        assert_eq!(
            take(&a, &[u], |_| false, &log),
            Some(InputV1::move_to(550_000, 200_000))
        );
        let moved = enemy(29, (300_000, 200_000));
        assert_eq!(
            take(&a, &[moved], |_| true, &log).unwrap().target.target_id,
            29
        );
        a.update(keys(false, false), true, &c, &log);
        a.update(
            Keys {
                cursor: Some((1660., 537.)),
                ..keys(true, false)
            },
            true,
            &c,
            &log,
        );
        assert!(take(&a, &[u], |_| false, &log).is_some());
        a.update(
            Keys {
                right: true,
                ..keys(false, false)
            },
            true,
            &c,
            &log,
        );
        assert!(take(&a, &[u], |_| true, &log).is_none());
    }
    #[test]
    fn mapped_normal_and_release_casts_do_not_require_shift() {
        let (a, c, log) = setup(0, 1);
        budget(&a, 3);
        let press = Keys {
            mapped: true,
            cast_modes: [1, 0, 0],
            abilities: [true, false, false],
            ..keys(false, false)
        };
        a.update(press, true, &c, &log);
        assert_eq!(a.0.lock().unwrap().preview, Some(0));
        assert!(a.0.lock().unwrap().pending.is_none());
        a.update(
            Keys {
                mapped: true,
                left: true,
                ..keys(false, false)
            },
            true,
            &c,
            &log,
        );
        assert!(a.0.lock().unwrap().pending.is_some());
        a.clear_commands();
        a.update(
            Keys {
                mapped: true,
                ..keys(false, false)
            },
            true,
            &c,
            &log,
        );
        a.update(
            Keys {
                cast_modes: [2, 0, 0],
                ..press
            },
            true,
            &c,
            &log,
        );
        assert_eq!(a.0.lock().unwrap().release_slot, Some(0));
        a.update(
            Keys {
                mapped: true,
                ..keys(false, false)
            },
            true,
            &c,
            &log,
        );
        assert!(a.0.lock().unwrap().pending.is_some());
    }
    #[test]
    fn alt_self_cast_needs_no_cursor_and_respects_native_rejection() {
        let (a, c, log) = setup(0, 1);
        budget(&a, 3);
        let press = Keys {
            alt: true,
            cursor: None,
            abilities: [false, true, false],
            ..keys(false, false)
        };
        a.update(press, true, &c, &log);
        assert!(take(&a, &[], |_| false, &log).is_some());
        budget(&a, 0);
        assert_eq!(
            take(&a, &[], |i| i.target.target_id == 7, &log)
                .unwrap()
                .target,
            InputTargetV1::target(7)
        );
        assert!(take(&a, &[], |_| true, &log).is_none());
        let (a, c, log) = setup(0, 7);
        a.update(press, true, &c, &log);
        assert!(take(&a, &[], |_| false, &log).is_none());
    }
    #[test]
    fn native_charge_budget_keeps_multi_use_readiness_until_last_cast() {
        let (a, _c, _log) = setup(2, 5);
        budget(&a, 0);
        for (cooldown, uses, ready, wait) in [
            (0, 3, true, 0),
            (120, 2, true, 0),
            (240, 1, true, 0),
            (360, 0, false, 120),
            (241, 0, false, 1),
            (240, 1, true, 0),
        ] {
            a.observe_actor(
                (1, 33, 1),
                Some(7),
                Some((200_000, 200_000)),
                [cooldown, 0, 0],
            );
            let hud = a.hud_skills();
            assert_eq!(hud.uses[0], Some(uses));
            assert_eq!(hud.ready[0], Some(ready));
            assert_eq!(hud.wait[0], Some(wait));
        }
    }
    use crate::test_support::logger;
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
    #[test]
    fn hud_hover_is_visual_only_and_normal_cast_keeps_priority() {
        let (a, c, log) = setup(2, 5);
        a.observe_level((1, 33, 1), 7, 5);
        a.set_hud_hover(Some(1));
        let (preview, hover) = a.preview_data().unwrap();
        assert!(hover && preview.ready);
        assert!(preview.aim.is_none() && preview.target.is_none());
        assert_eq!(a.hud_skills().aiming, None);
        assert!(a.cursor_feedback(false).is_none());
        assert!(take(&a, &[], |_| true, &log).is_none());
        a.update(keys(true, true), true, &c, &log);
        a.set_hud_hover(Some(2));
        let (preview, hover) = a.preview_data().unwrap();
        assert!(!hover && preview.aim.is_some());
        assert_eq!(a.hud_skills().aiming, Some(0));
        assert!(take(&a, &[], |_| true, &log).is_none());
        a.set_hud_hover(None);
        assert!(!a.preview_data().unwrap().1);
    }
    #[test]
    fn hud_hover_clears_on_leave_and_requires_fresh_alive_focused_state() {
        let (a, c, log) = setup(1, 5);
        a.observe_level((1, 33, 1), 7, 1);
        a.set_hud_hover(Some(1));
        assert!(!a.preview_data().unwrap().0.ready); // W not learned.
        a.set_hud_hover(None);
        assert!(a.preview_data().is_none());
        a.set_hud_hover(Some(3));
        assert!(a.preview_data().is_none());
        a.set_hud_hover(Some(0));
        a.0.lock().unwrap().hud_hover.as_mut().unwrap().1 =
            Instant::now() - FRESH - Duration::from_millis(1);
        assert!(a.preview_data().is_none());
        a.set_hud_hover(Some(0));
        assert!(a.preview_data().unwrap().0.ready);
        a.update(
            Keys {
                focused: false,
                ..keys(false, false)
            },
            true,
            &c,
            &log,
        );
        assert!(a.preview_data().is_none());
        a.update(keys(false, false), true, &c, &log);
        a.observe_actor((1, 33, 1), Some(7), None, [0; 3]);
        assert!(a.preview_data().is_none());
    }
    #[test]
    fn geometry_sampling_is_identity_scoped_throttled_and_resets_on_death() {
        let (a, _, log) = setup(2, 0);
        let key = (1, 33, 1);
        assert!(a.geometry_due(key, 7));
        assert!(!a.geometry_due(key, 8));
        a.observe_geometry((2, 33, 1), 7, Default::default(), &log);
        assert!(a.geometry_due(key, 7));
        a.observe_geometry(key, 7, Default::default(), &log);
        assert!(!a.geometry_due(key, 7));
        let mut s = a.0.lock().unwrap();
        s.geometry.as_mut().unwrap().1 = Instant::now() - Duration::from_millis(101);
        drop(s);
        assert!(a.geometry_due(key, 7));
        a.observe_actor(key, Some(7), None, [0; 3]);
        assert!(a.0.lock().unwrap().geometry.is_none());
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
    fn diagnostics_identify_later_click_expiry_and_target_miss() {
        let (a, c, _) = setup(1, 5);
        let log = logger("cast-diagnostics-36");
        budget(&a, 3);
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &[], |_| false, &log).is_some());
        a.update(
            Keys {
                right: true,
                ..keys(false, false)
            },
            true,
            &c,
            &log,
        );
        a.update(keys(false, false), true, &c, &log);
        a.update(keys(true, false), true, &c, &log);
        a.0.lock().unwrap().pending.as_mut().unwrap().at =
            Instant::now() - BUFFER - Duration::from_millis(1);
        assert!(take(&a, &[], |_| false, &log).is_none());
        let (targeted, camera, _) = setup(0, 6);
        targeted.update(keys(true, false), true, &camera, &log);
        assert!(take(&targeted, &[], |_| false, &log).is_none());
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-cast-diagnostics-36.log");
        let lines = std::fs::read_to_string(path).unwrap();
        for reason in [
            "reason=later-right-click",
            "reason=buffer-expired",
            "reason=cursor-missed-eligible-target",
        ] {
            assert!(lines.contains(reason), "missing {reason}: {lines}");
        }
    }
    #[test]
    fn wait_diagnostics_record_action_changes_without_logging_each_retry() {
        let (a, c, _) = setup(1, 5);
        let log = logger("wait-diagnostics-36");
        budget(&a, 3);
        a.update(keys(true, false), true, &c, &log);
        assert!(take(&a, &[], |_| true, &log).is_some());
        assert!(take(&a, &[], |_| true, &log).is_some());
        budget(&a, 4);
        assert!(take(&a, &[], |_| true, &log).is_some());
        budget(&a, 0);
        let input = take(&a, &[], |_| true, &log).unwrap();
        assert_eq!(input.kind, KINDS[0].code());
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-wait-diagnostics-36.log");
        let lines = std::fs::read_to_string(path).unwrap();
        assert_eq!(lines.matches("ABILITY WAIT").count(), 2);
        assert!(lines.contains("native_action=Some(3)"));
        assert!(lines.contains("native_action=Some(4)"));
        assert!(lines.contains("reason=dispatched"));
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
            is_minion: false,
            friendly: false,
            in_cc: false,
            is_tower: false,
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
            is_minion: false,
            friendly: false,
            in_cc: false,
            is_tower: false,
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
                is_minion: false,
                friendly: false,
                in_cc: false,
                is_tower: false,
                body: None,
            },
            Unit {
                id: 3,
                position: (300_000, 200_000),
                radius: 10_000,
                is_champion: true,
                is_minion: false,
                friendly: false,
                in_cc: false,
                is_tower: false,
                body: None,
            },
            Unit {
                id: 4,
                position: (210_000, 200_000),
                radius: 10_000,
                is_champion: true,
                is_minion: false,
                friendly: false,
                in_cc: false,
                is_tower: false,
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
                is_minion: false,
                friendly: false,
                in_cc: false,
                is_tower: false,
                body: None,
            },
            Unit {
                id: 3,
                position: (300_000, 200_000),
                radius: 10_000,
                is_champion: true,
                is_minion: false,
                friendly: false,
                in_cc: false,
                is_tower: false,
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
        assert!(!validated.is_empty() && validated.iter().all(|id| *id == 3));
        a.update(keys(false, false), true, &c, &log);
        a.update(press, true, &c, &log);
        // Native side/eligibility rejection cannot fall back to a minion.
        assert!(take(&a, &units, |i| i.target.target_id == 2, &log).is_none());
        assert!(take(&a, &units, |_| true, &log).is_none());
        a.update(keys(false, false), true, &c, &log);
        a.update(keys(true, false), true, &c, &log);
        assert_eq!(
            take(&a, &units, |_| true, &log).unwrap().target.target_id,
            3 // Champion priority also applies when champion-only mode is off.
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
            is_minion: false,
            friendly: false,
            in_cc: false,
            is_tower: false,
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
            is_minion: false,
            friendly: false,
            in_cc: false,
            is_tower: false,
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
