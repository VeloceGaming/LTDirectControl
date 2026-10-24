//! No waiting from an SDK callback. The only wait is between published frames.
use crate::{platform_input::Keys, Logger};
use std::collections::{BTreeSet, VecDeque};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

pub type MatchKey = (u64, u64, u64);

/// A match the player left mid-way: the game finishes it in the background
/// at full speed, and the mod's per-tick work for it is skipped from the
/// first step (read lock-free; 0 = none).
static LEFT_MATCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// After a start or resume, samples from before it are not yet stale.
/// Most ticks the worker may run ahead of playback, and the most one client
/// frame may play: the match keeps full speed down to 60 / MAX_LEAD fps.
pub const MAX_LEAD: usize = 4;
/// While the AI drives, the game's own speed field (0.5x to 3x) is followed:
/// the worker may run `ceil(multiplier)` frame leads ahead, at most this many
/// ticks (3x at the lowest frame rate the frame lead covers).
pub const MAX_SPECTATOR_LEAD: usize = 12;
/// `SPECTATOR fields` lines written per match.
const FIELD_LINES: u32 = 200;
/// The `SPECTATOR multiplier=` window.
const SPECTATOR_WINDOW: Duration = Duration::from_secs(10);
/// Share of each new frame time in the running average.
const FRAME_WEIGHT: f32 = 0.1;
/// The most one frame may exceed the running average by, in ticks.
const FRAME_STEP: f32 = 0.4;
/// A longer gap between running frames is a pause, not a frame.
const FRAME_GAP: Duration = Duration::from_millis(250);
/// Playback rate is reported over windows of this length.
const RATE_WINDOW: Duration = Duration::from_secs(10);
/// Lead changes and slow-playback windows logged per match at every level.
const REPORTS: u32 = 8;
/// Owed playback time is paid back at most this many ticks per frame (the
/// match then runs at most 25% fast for a few frames instead of playing two
/// ticks in every frame).
const PAYBACK: f32 = 0.25;
/// The most owed playback time kept, in ticks; beyond it the match drops
/// the time (it ran slow by up to the 5% the lead tolerates).
const MAX_OWED: f32 = 2.;
/// Frames whose played ticks are logged after a lead change.
const TRACE_FRAMES: usize = 60;

/// The game's viewer adds each frame's time to a clock and plays one queued
/// tick per tick of it; when the queue runs empty the rest stays in the
/// clock (read from the code), so time owed while the worker was one tick
/// ahead used to come back as two ticks in every frame once the lead rose.
/// Given the clock and the owed time (both in ticks) before a running frame:
/// whole ticks the last frame could not play move to `owed`, owed time
/// above [`MAX_OWED`] is dropped, and at most [`PAYBACK`] returns to the
/// clock. Returns (clock, owed, dropped).
fn smooth(clock: f32, owed: f32) -> (f32, f32, f32) {
    if !clock.is_finite() || clock < 0. {
        return (clock, owed, 0.);
    }
    let held = clock.floor();
    let owed = owed + held;
    let dropped = (owed - MAX_OWED).max(0.);
    let owed = owed - dropped;
    let back = owed.min(PAYBACK);
    (clock - held + back, owed - back, dropped)
}
/// How many ticks the worker may run ahead when a client frame lasts
/// `ticks` ticks on average. One frame can only play what is queued, so
/// below 60 fps a lead of 1 slows the match in proportion. The lead rises
/// once the match would run more than 5% slow, and falls again only when
/// the smaller lead is nearly enough, so it does not flip at a boundary.
fn lead_for(current: usize, ticks: f32) -> usize {
    let current = current.clamp(1, MAX_LEAD);
    let needed = |slack: f32| ((ticks - slack).ceil() as usize).clamp(1, MAX_LEAD);
    if ticks > current as f32 * 1.05 {
        needed(0.05 * current as f32)
    } else if ticks <= (current - 1) as f32 * 1.02 {
        needed(0.02)
    } else {
        current
    }
}
/// The game's playback speed field (`config+0x14`) as a multiplier: the
/// speed buttons write 0.5, 1, 1.5, 2 or 3 there (seen in game, 1.5.1-dev1,
/// 0.6.5). Anything outside 0.25..=4, or not a number, counts as 1. The
/// game's pause writes 0; [`NativeTiming::spectator_fields`] handles that
/// before this is asked.
pub fn spectator_multiplier(speed: f32) -> f32 {
    if speed.is_finite() && (0.25..=4.).contains(&speed) {
        speed
    } else {
        1.
    }
}
/// AI phase: ticks the worker may run ahead at `multiplier` when the frame
/// lead is `frame_lead`.
fn spectator_lead(frame_lead: usize, multiplier: f32) -> usize {
    let times = spectator_multiplier(multiplier).ceil() as usize;
    (times * frame_lead.clamp(1, MAX_LEAD)).clamp(1, MAX_SPECTATOR_LEAD)
}
/// Which of the game's playback fields the viewer hook overwrites for one
/// frame, besides sync off (`config+0`) and mode 0 (`config+0x10`), which
/// are always forced (mode 0 keeps the game's highlight button dead). Only
/// overwritten fields are put back afterwards, so a value the game writes to
/// a field the hook leaves alone is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaybackPlan {
    /// Speed 1.0 (`config+0x14`).
    pub force_speed: bool,
    /// Pause cleared (`config+0x48`).
    pub clear_pause: bool,
}
/// Control (and anything that is not the AI phase): speed and pause forced,
/// as before. AI phase: speed and pause left to the game.
pub fn playback_plan(phase: Option<Phase>) -> PlaybackPlan {
    let ai = phase == Some(Phase::Ai);
    PlaybackPlan {
        force_speed: !ai,
        clear_pause: !ai,
    }
}
fn key_id(key: MatchKey) -> u64 {
    // Never 0, so 0 can mean "none".
    (key.0 ^ key.1.rotate_left(21) ^ key.2.rotate_left(42)) | 1
}
/// Worker: this match was left by the player; do nothing for it.
pub fn left_match(key: MatchKey) -> bool {
    LEFT_MATCH.load(std::sync::atomic::Ordering::Relaxed) == key_id(key)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Armed,
    Loading,
    Ready,
    Running,
    Paused,
    Ai,
    Released,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionAction {
    Start,
    Pause,
    Resume,
    ReturnAi,
    TakeControl,
}
struct State {
    generation: u64,
    traces: VecDeque<crate::input_trace::FrameTrace>,
    trace_count: usize,
    native_trace_count: usize,
    retired: BTreeSet<MatchKey>,
    binding_window: bool,
    phase: Phase,
    reason: String,
    installed: bool,
    client: Option<u64>,
    heartbeat: Option<Instant>,
    battlefield: bool,
    selected: bool,
    previous_start: bool,
    previous_release: bool,
    key: Option<MatchKey>,
    worker: Option<u64>,
    sender: Option<usize>,
    view: Option<usize>,
    began: Instant,
    running: Option<Instant>,
    /// Whether the client saw the match running last frame, and since when.
    produced: usize,
    consumed: usize,
    /// Ticks the worker may run ahead of playback (1..=MAX_LEAD), from the
    /// average client frame time in ticks.
    lead: usize,
    frame_ticks: f32,
    /// Playback time the viewer could not play yet, held back to be paid
    /// back gently, and time dropped this match (both in ticks); see
    /// [`smooth`].
    owed: f32,
    dropped: f32,
    /// After a lead change: ticks played per frame, the most owed and the
    /// time dropped meanwhile, logged once [`TRACE_FRAMES`] frames are in.
    lead_trace: Option<LeadTrace>,
    /// The previous running frame; the playback-rate window (start,
    /// frames consumed then); log budgets.
    frame_at: Option<Instant>,
    rate: Option<(Instant, usize)>,
    lead_changes: u32,
    slow_reports: u32,
    startup_shop_pending: bool,
    role_fallback: bool,
    boundary_seen: bool,
    bootstrap_applied: bool,
    first_view_seen: bool,
    pause_acknowledged: bool,
    worker_calls: usize,
    viewer_calls: usize,
    worker_bind_base: usize,
    viewer_bind_base: usize,
    played_tick: usize,
    pending_action: Option<(MatchKey, Phase, SessionAction)>,
    /// Last worker progress while running (crate::worker_watch), and when a
    /// reported stall began.
    progress: Option<Instant>,
    stalled: Option<Instant>,
    /// When the stalled worker's stack was last logged, and how many times.
    stack_logged: Option<(Instant, u8)>,
    /// AI phase: the game's speed field as a multiplier (see
    /// [`spectator_multiplier`]); 1 outside the AI phase.
    spectator: f32,
    /// The most ticks that can be queued now: the highest lead the worker
    /// produced under until the queue fell back under the current lead. The
    /// frame-count guard allows this many, so a lower lead (F11, a slower
    /// speed button) never releases the session while the queue drains.
    peak_lead: usize,
    /// After F11 from the AI phase: the AI lead then and frames since,
    /// logged once the queue is back under the control lead.
    drain: Option<(usize, u32)>,
    /// The multiplier changed, or the game was paused, since the playback
    /// rate window began: that window is not judged slow (see `frame_at`).
    rate_unsteady: bool,
    /// The four raw playback fields last seen in the AI phase (sync, mode,
    /// speed bits, paused) and `SPECTATOR fields` lines written this match.
    fields_seen: Option<(u8, u32, u32, u8)>,
    field_lines: u32,
    window: SpectatorWindow,
}
/// AI-phase playback over one [`SPECTATOR_WINDOW`].
#[derive(Default)]
struct SpectatorWindow {
    start: Option<Instant>,
    frames: u32,
    ticks: usize,
    queue: usize,
    multiplier: f32,
    wait: Duration,
}
struct LeadTrace {
    change: (usize, usize),
    played: Vec<usize>,
    owed_peak: f32,
    dropped_from: f32,
}
pub struct NativeTiming {
    state: Mutex<State>,
    wake: Condvar,
    enabled: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Native,
    Bootstrap,
    Paused,
    Running,
}
impl NativeTiming {
    #[cfg(test)]
    pub(crate) fn running_test_worker(key: MatchKey) -> Self {
        let t = Self::new(true);
        {
            let mut s = t.state.lock().unwrap();
            s.phase = Phase::Running;
            s.key = Some(key);
            s.worker = Some(crate::platform_input::thread_id());
            s.selected = true;
            s.battlefield = true;
            s.installed = true;
            s.heartbeat = Some(Instant::now());
        }
        t
    }
    /// A session in `phase` on the battlefield, owned by this thread's
    /// client (window tests); None: off the battlefield.
    #[cfg(test)]
    pub(crate) fn set_test_phase(&self, phase: Option<Phase>) {
        let mut s = self.state.lock().unwrap();
        s.installed = true;
        s.battlefield = phase.is_some();
        s.client = Some(crate::platform_input::thread_id());
        s.key = Some((1, 2, 3));
        if let Some(phase) = phase {
            s.phase = phase;
        }
    }
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            wake: Condvar::new(),
            state: Mutex::new(State {
                generation: 0,
                traces: VecDeque::new(),
                trace_count: 0,
                native_trace_count: 0,
                retired: BTreeSet::new(),
                binding_window: true,
                phase: Phase::Armed,
                reason: String::new(),
                installed: false,
                client: None,
                heartbeat: None,
                battlefield: false,
                selected: false,
                previous_start: false,
                previous_release: false,
                key: None,
                worker: None,
                sender: None,
                view: None,
                began: Instant::now(),
                running: None,
                produced: 0,
                consumed: 0,
                lead: 1,
                frame_ticks: 1.,
                owed: 0.,
                dropped: 0.,
                lead_trace: None,
                frame_at: None,
                rate: None,
                lead_changes: 0,
                slow_reports: 0,
                startup_shop_pending: false,
                role_fallback: false,
                boundary_seen: false,
                bootstrap_applied: false,
                first_view_seen: false,
                pause_acknowledged: false,
                worker_calls: 0,
                viewer_calls: 0,
                worker_bind_base: 0,
                viewer_bind_base: 0,
                played_tick: 0,
                pending_action: None,
                progress: None,
                stalled: None,
                stack_logged: None,
                spectator: 1.,
                peak_lead: 1,
                drain: None,
                rate_unsteady: false,
                fields_seen: None,
                field_lines: 0,
                window: SpectatorWindow::default(),
            }),
        }
    }
    /// Ticks the worker may run ahead now: the frame lead, times the game's
    /// speed multiplier (rounded up) in the AI phase.
    fn effective_lead(s: &State) -> usize {
        if s.phase == Phase::Ai {
            spectator_lead(s.lead, s.spectator)
        } else {
            s.lead
        }
    }
    /// The most ticks one frame may play before the session is released:
    /// never fewer than before this build ([`MAX_LEAD`]), and never fewer
    /// than can be queued ([`State::peak_lead`]).
    fn frame_limit(s: &State) -> usize {
        MAX_LEAD.max(2).max(s.peak_lead)
    }
    /// Viewer: which playback fields to overwrite this frame.
    pub fn playback_plan(&self) -> PlaybackPlan {
        playback_plan(self.phase())
    }
    /// Viewer, each accepted frame, with the four raw playback fields as
    /// read before the hook wrote anything. In the AI phase the speed field
    /// becomes the multiplier, and every change is logged (Verbose).
    pub fn spectator_fields(&self, sync: u8, mode: u32, speed: u32, paused: u8, log: &Logger) {
        let Ok(mut s) = self.state.lock() else { return };
        if s.phase != Phase::Ai {
            s.fields_seen = None;
            s.spectator = 1.;
            return;
        }
        // The game's pause button writes speed 0 (seen in game, 1.5.1-dev1):
        // the multiplier and the lead stay as they were, and no line is
        // written, so resuming at the same speed is no change either.
        if f32::from_bits(speed) == 0. {
            s.rate_unsteady = true;
            return;
        }
        let multiplier = spectator_multiplier(f32::from_bits(speed));
        if multiplier != s.spectator {
            s.spectator = multiplier;
            s.rate_unsteady = true;
            // A higher lead lets the worker run further ahead at once.
            self.wake.notify_all();
        }
        let fields = (sync, mode, speed, paused);
        if s.fields_seen == Some(fields) {
            return;
        }
        s.fields_seen = Some(fields);
        if s.field_lines >= FIELD_LINES {
            return;
        }
        s.field_lines += 1;
        log.write(&format!(
            "SPECTATOR fields sync={sync} mode={mode} speed={} paused={paused} multiplier={multiplier} lead={}{}",
            f32::from_bits(speed),
            Self::effective_lead(&s),
            if s.field_lines == FIELD_LINES {
                "; later changes are not logged this match"
            } else {
                ""
            }
        ));
    }
    /// After a running frame in the AI phase: playback averages, written as
    /// one `SPECTATOR multiplier=` line per [`SPECTATOR_WINDOW`].
    fn spectator_window(s: &mut State, used: usize, queue: usize, log: &Logger) {
        let now = Instant::now();
        let start = *s.window.start.get_or_insert(now);
        s.window.frames += 1;
        s.window.ticks += used;
        s.window.queue += queue;
        s.window.multiplier += s.spectator;
        let length = now.duration_since(start);
        if length < SPECTATOR_WINDOW {
            return;
        }
        let w = std::mem::take(&mut s.window);
        let frames = w.frames.max(1) as f32;
        log.write(&format!(
            "SPECTATOR multiplier={:.2} ticks_per_frame={:.2} queue={:.2} worker_wait_pct={:.0} frames={} window_s={:.1} lead={} frame_lead={}",
            w.multiplier / frames,
            w.ticks as f32 / frames,
            w.queue as f32 / frames,
            100. * w.wait.as_secs_f32() / length.as_secs_f32(),
            w.frames,
            length.as_secs_f32(),
            Self::effective_lead(s),
            s.lead
        ));
    }
    pub fn installed(&self, ok: bool, reason: &str, log: &Logger) {
        let Ok(mut s) = self.state.lock() else { return };
        self.wake.notify_all();
        s.installed = ok;
        log.write(&format!("NATIVE install ok={ok} {reason}"));
        if !ok {
            Self::release(&mut s, reason, log);
        }
    }
    fn release(s: &mut State, reason: &str, log: &Logger) {
        if s.phase == Phase::Released {
            return;
        }
        s.phase = Phase::Released;
        s.pending_action = None;
        s.reason = reason.into();
        crate::athlete_rules::clear();
        log.write(&format!(
            "NATIVE RELEASE reason={reason} produced={} consumed={} played_tick={} elapsed_ms={}",
            s.produced,
            s.consumed,
            s.played_tick,
            s.began.elapsed().as_millis()
        ));
    }
    pub fn cancel(&self, reason: &str, log: &Logger) {
        if let Ok(mut s) = self.state.lock() {
            self.wake.notify_all();
            Self::release(&mut s, reason, log);
        }
    }
    pub fn heartbeat(&self, battlefield: bool, selected: bool, keys: Keys, log: &Logger) {
        let auto_start = crate::settings::option("start_mode") == 1.;
        self.heartbeat_with_start(battlefield, selected, keys, auto_start, log);
    }
    fn heartbeat_with_start(
        &self,
        battlefield: bool,
        selected: bool,
        keys: Keys,
        auto_start: bool,
        log: &Logger,
    ) {
        let Ok(mut s) = self.state.lock() else { return };
        self.wake.notify_all();
        let was_battlefield = s.battlefield;
        s.client = Some(crate::platform_input::thread_id());
        s.heartbeat = Some(Instant::now());
        s.battlefield = battlefield;
        s.selected = selected;
        // The deadline must work even if neither native hook ever arrives.
        Self::check_limits(&mut s, log);
        if keys.release && !s.previous_release {
            if matches!(s.phase, Phase::Loading | Phase::Armed) {
                Self::release(&mut s, "F12 during startup", log);
            } else if matches!(s.phase, Phase::Ready | Phase::Running | Phase::Paused) {
                if let Some(key) = s.key {
                    s.pending_action = Some((key, s.phase, SessionAction::ReturnAi));
                }
            }
        }
        if was_battlefield && !battlefield && s.key.is_some() {
            Self::release(&mut s, "Left battlefield", log);
            if let Some(key) = s.key {
                LEFT_MATCH.store(key_id(key), std::sync::atomic::Ordering::Relaxed);
                log.write("SESSION left mid-match; the game finishes it in the background without the mod's per-tick work");
            }
        }
        if s.phase == Phase::Loading
            && s.boundary_seen
            && s.bootstrap_applied
            && s.pause_acknowledged
            && battlefield
            && selected
        {
            s.phase = Phase::Ready;
            if auto_start && !s.role_fallback && s.pending_action.is_none() {
                if let Some(key) = s.key {
                    s.pending_action = Some((key, Phase::Ready, SessionAction::Start));
                }
            }
            log.write(&format!(
                "NATIVE READY produced={} consumed={} played_tick={} load_ms={}",
                s.produced,
                s.consumed,
                s.played_tick,
                s.began.elapsed().as_millis()
            ));
        }
        if keys.start && !s.previous_start && s.phase == Phase::Ai {
            if let Some(key) = s.key {
                s.pending_action = Some((key, Phase::Ai, SessionAction::TakeControl));
            }
        } else if keys.start
            && !s.previous_start
            && matches!(s.phase, Phase::Ready | Phase::Paused)
            && s.pending_action.is_none()
        {
            s.phase = Phase::Running;
            s.running.get_or_insert_with(Instant::now);
            log.write("NATIVE START/RESUME backup F11; playback=1x maximum frame lead=1");
        } else if keys.start && !s.previous_start && s.phase == Phase::Running {
            // F11 also pauses: queue the same action as the Pause button, so
            // pending orders are cleared exactly as for a click.
            if let (Some(key), None) = (s.key, s.pending_action) {
                s.pending_action = Some((key, Phase::Running, SessionAction::Pause));
                log.write("SESSION pause requested by F11");
            }
        }
        s.previous_start = keys.start;
        s.previous_release = keys.release;
    }
    pub fn phase(&self) -> Option<Phase> {
        let s = self.state.lock().ok()?;
        Self::client_owned(&s, None).then_some(s.phase)
    }
    pub fn ui_phase(&self) -> Option<Phase> {
        let s = self.state.lock().ok()?;
        (s.installed && s.battlefield && s.client == Some(crate::platform_input::thread_id()))
            .then_some(s.phase)
    }
    /// Claim the one startup opening; AI handoff permanently forfeits it.
    pub fn claim_startup_shop(&self) -> bool {
        let Ok(mut s) = self.state.lock() else {
            return false;
        };
        if !s.startup_shop_pending
            || !Self::client_owned(&s, None)
            || !matches!(s.phase, Phase::Ready | Phase::Running | Phase::Paused)
        {
            return false;
        }
        s.startup_shop_pending = false;
        true
    }
    /// The first complete publication has made the tick-one roster available.
    pub fn roster_ready(&self) -> bool {
        self.state.lock().is_ok_and(|s| {
            s.key.is_some() && s.boundary_seen && (s.bootstrap_applied || s.phase == Phase::Ai)
        })
    }
    pub fn mark_role_fallback(&self, key: MatchKey) {
        if let Ok(mut s) = self.state.lock() {
            if s.key == Some(key) && matches!(s.phase, Phase::Loading | Phase::Ai) {
                s.role_fallback = true;
            }
        }
    }
    pub fn match_key(&self) -> Option<MatchKey> {
        self.state.lock().ok()?.key
    }
    /// Cosmetic durations follow frames actually played, including pauses.
    pub fn presentation_tick(&self) -> usize {
        self.state.lock().map_or(0, |s| s.played_tick)
    }
    pub fn generation(&self) -> u64 {
        self.state.lock().map_or(0, |s| s.generation)
    }
    pub fn set_binding_window(&self, allowed: bool) {
        if let Ok(mut s) = self.state.lock() {
            s.binding_window = allowed;
        }
    }
    /// Only the client may prepare another battle, after release and outside
    /// the battlefield. Result/replay scenes are not pre-match boundaries.
    pub fn needs_rearm(&self, pre_match: bool, save_exit: bool) -> bool {
        self.state.lock().is_ok_and(|s| {
            s.installed && !s.battlefield && (save_exit || pre_match && s.phase == Phase::Released)
        })
    }
    /// Call after clearing session consumers, while the coordinator is still
    /// released. Native patches remain installed; old pointers are forgotten.
    pub fn rearm(&self, save_exit: bool, keys: Keys, log: &Logger) {
        let Ok(mut s) = self.state.lock() else { return };
        self.wake.notify_all();
        if !s.installed || s.battlefield || !save_exit && s.phase != Phase::Released {
            return;
        }
        if save_exit {
            s.retired.clear();
        } else if let Some(key) = s.key {
            s.retired.insert(key);
        }
        s.generation = s.generation.wrapping_add(1);
        s.traces.clear();
        s.trace_count = 0;
        s.native_trace_count = 0;
        s.phase = Phase::Armed;
        s.reason.clear();
        s.key = None;
        s.worker = None;
        crate::worker_watch::bind(0);
        s.progress = None;
        s.stalled = None;
        s.stack_logged = None;
        s.sender = None;
        s.view = None;
        s.selected = false;
        s.previous_start = keys.start;
        s.previous_release = keys.release;
        s.began = Instant::now();
        s.running = None;
        s.produced = 0;
        s.consumed = 0;
        s.lead = 1;
        s.frame_ticks = 1.;
        s.owed = 0.;
        s.dropped = 0.;
        s.lead_trace = None;
        s.frame_at = None;
        s.rate = None;
        s.lead_changes = 0;
        s.slow_reports = 0;
        s.spectator = 1.;
        s.peak_lead = 1;
        s.drain = None;
        s.rate_unsteady = false;
        s.fields_seen = None;
        s.field_lines = 0;
        s.window = SpectatorWindow::default();
        s.played_tick = 0;
        s.startup_shop_pending = false;
        s.role_fallback = false;
        s.boundary_seen = false;
        s.bootstrap_applied = false;
        s.first_view_seen = false;
        s.pause_acknowledged = false;
        s.pending_action = None;
        s.worker_bind_base = s.worker_calls;
        s.viewer_bind_base = s.viewer_calls;
        log.write(&format!("SESSION REARM generation={} save_exit={save_exit} retired_keys={}; waiting for new foreground worker tick 1", s.generation, s.retired.len()));
    }
    /// Session phase as seen by this match's simulation worker. `phase()`
    /// answers only on the bound client thread and is None on the worker.
    pub fn worker_phase(&self, key: MatchKey) -> Option<Phase> {
        let s = self.state.lock().ok()?;
        (s.key == Some(key) && s.worker == Some(crate::platform_input::thread_id()))
            .then_some(s.phase)
    }
    pub fn owns_worker(&self, key: MatchKey) -> bool {
        self.state.lock().is_ok_and(|s| {
            s.key == Some(key)
                && s.worker == Some(crate::platform_input::thread_id())
                && s.phase != Phase::Armed
                && (s.phase != Phase::Released || s.battlefield)
        })
    }
    /// UI callbacks retain only this coordinator, not a host context. Requests
    /// are consumed at client post-update and bound to their original phase/key.
    pub fn request_action(&self, primary: bool) {
        let Ok(mut s) = self.state.lock() else { return };
        if !Self::client_owned(&s, None) {
            return;
        }
        let action = if !primary {
            if s.phase == Phase::Ai {
                return;
            }
            SessionAction::ReturnAi
        } else {
            match s.phase {
                Phase::Ready => SessionAction::Start,
                Phase::Running => SessionAction::Pause,
                Phase::Paused => SessionAction::Resume,
                Phase::Ai => SessionAction::TakeControl,
                _ => return,
            }
        };
        // Return to AI wins over a primary-button request in the same update.
        if s.pending_action
            .is_some_and(|(_, _, a)| a == SessionAction::ReturnAi)
        {
            return;
        }
        if let Some(key) = s.key {
            s.pending_action = Some((key, s.phase, action));
        }
    }
    pub fn take_action(&self) -> Option<SessionAction> {
        let mut s = self.state.lock().ok()?;
        let (key, phase, action) = s.pending_action.take()?;
        (s.key == Some(key) && s.phase == phase && Self::client_owned(&s, None)).then_some(action)
    }
    pub fn apply_action(&self, action: SessionAction, log: &Logger) {
        let Ok(mut s) = self.state.lock() else { return };
        self.wake.notify_all();
        if !Self::client_owned(&s, None) {
            return;
        }
        match (action, s.phase) {
            (SessionAction::ReturnAi, Phase::Ready | Phase::Running | Phase::Paused) => {
                s.phase = Phase::Ai;
                crate::athlete_rules::clear();
                // Watching forfeits startup shopping even if the native build
                // plan only becomes available after a later takeover/base visit.
                s.startup_shop_pending = false;
                s.pending_action = None;
                s.running.get_or_insert_with(Instant::now);
                s.fields_seen = None;
                s.window = SpectatorWindow::default();
                log.write("SESSION AI control; same match/player retained; the game's speed buttons are followed; F11 reclaims control at 1x");
            }
            (SessionAction::Start, Phase::Ready)
            | (SessionAction::Resume, Phase::Paused)
            | (SessionAction::TakeControl, Phase::Ai) => {
                if action == SessionAction::TakeControl {
                    s.startup_shop_pending = false;
                    // Control runs at 1x with the frame lead again; ticks
                    // already queued play out at 1x while the worker waits.
                    let ai_lead = Self::effective_lead(&s);
                    let queue = s.produced.saturating_sub(s.consumed);
                    log.write(&format!(
                        "SPECTATOR take_control multiplier={} lead {ai_lead} -> {} queue={queue}",
                        s.spectator, s.lead
                    ));
                    s.drain = Some((ai_lead, 0));
                    s.spectator = 1.;
                    // The rate window now straddles a speed change.
                    s.rate_unsteady = true;
                    s.fields_seen = None;
                    s.window = SpectatorWindow::default();
                }
                s.phase = Phase::Running;
                s.running.get_or_insert_with(Instant::now);
                log.write(&format!("SESSION {action:?}; full-match pacing=1x"));
            }
            (SessionAction::Pause, Phase::Running) => {
                s.phase = Phase::Paused;
                s.pause_acknowledged = false;
                log.write(
                    "SESSION Pause; viewer held and worker waits at next publication boundary",
                );
            }
            _ => {}
        }
    }
    /// Called by the stable AI to identify a foreground run, never to wait.
    pub fn observe(&self, key: MatchKey, tick: usize, log: &Logger) {
        self.observe_start(
            key,
            tick,
            tick == 1 && crate::settings::option("start_mode") == 2.,
            tick == 1 && crate::settings::option("manual_shop") == 1.,
            log,
        );
    }
    fn observe_start(
        &self,
        key: MatchKey,
        tick: usize,
        spectate: bool,
        manual_shop: bool,
        log: &Logger,
    ) {
        if !self.enabled {
            return;
        }
        let Ok(mut s) = self.state.lock() else { return };
        if !s.installed || s.phase != Phase::Armed || !s.binding_window || s.retired.contains(&key)
        {
            return;
        }
        if tick != 1 || s.client == Some(crate::platform_input::thread_id()) {
            Self::release(
                &mut s,
                "First foreground callback was not worker tick 1",
                log,
            );
            return;
        }
        s.key = Some(key);
        s.worker = Some(crate::platform_input::thread_id());
        crate::worker_watch::bind(crate::platform_input::thread_id());
        s.phase = if spectate { Phase::Ai } else { Phase::Loading };
        s.startup_shop_pending = manual_shop && !spectate;
        if s.phase == Phase::Ai {
            s.running = Some(Instant::now());
            log.write("SESSION spectator start; no preparation hold; the game's speed buttons are followed; F11 takes control at 1x");
        }
        s.began = Instant::now();
        s.worker_bind_base = s.worker_calls;
        s.viewer_bind_base = s.viewer_calls;
        log.write(&format!(
            "NATIVE BIND key={key:?} worker_os={:?} SDK origin=ClientMatchView tick=1",
            s.worker
        ));
    }
    /// Entrance evidence is recorded BEFORE scope checks and memory reads.
    /// Returns (accepted by scope, capture a bounded entrance stack).
    pub fn hook_entry(&self, worker: bool, log: &Logger) -> (bool, bool) {
        let Ok(mut s) = self.state.lock() else {
            return (false, false);
        };
        let (total, since_bind) = if worker {
            s.worker_calls = s.worker_calls.saturating_add(1);
            (
                s.worker_calls,
                s.worker_calls.saturating_sub(s.worker_bind_base),
            )
        } else {
            s.viewer_calls = s.viewer_calls.saturating_add(1);
            (
                s.viewer_calls,
                s.viewer_calls.saturating_sub(s.viewer_bind_base),
            )
        };
        let expected = if worker { s.worker } else { s.client };
        let current = crate::platform_input::thread_id();
        let scope = if !s.installed {
            "not-installed"
        } else if s.key.is_none() {
            "no-foreground-bind"
        } else if s.phase == Phase::Released {
            "released"
        } else if expected != Some(current) {
            "thread-mismatch"
        } else {
            "accepted"
        };
        let sample = |n: usize| n <= 3 || n.is_power_of_two() && n <= 256;
        if sample(total) || s.key.is_some() && sample(since_bind) {
            log.write(&format!("NATIVE HOOK_ENTRY kind={} calls={total} since_bind={since_bind} scope={scope} os_thread={current} expected={expected:?} phase={:?} coordinator={:p}",
                if worker { "worker" } else { "viewer" }, s.phase, self));
        }
        (
            scope == "accepted",
            total == 1 || s.key.is_some() && since_bind == 1,
        )
    }
    /// Called after a successful frame send in the runtime-identified worker.
    /// The current tick has returned and all three output locks are released.
    pub fn after_publication(&self, sender: usize, log: &Logger) {
        crate::worker_watch::published();
        let _outside = crate::worker_watch::Outside;
        let generation = {
            let Ok(mut s) = self.state.lock() else { return };
            if !s.installed
                || s.worker != Some(crate::platform_input::thread_id())
                || matches!(s.phase, Phase::Released | Phase::Armed)
            {
                return;
            }
            if s.sender.is_some_and(|old| old != sender) {
                Self::release(&mut s, "Worker sender changed", log);
                return;
            }
            s.sender = Some(sender);
            s.produced += 1;
            s.progress = Some(Instant::now());
            s.stack_logged = None;
            if let Some(since) = s.stalled.take() {
                log.write(&format!(
                    "WORKER RESUMED after {} ms; produced={}",
                    since.elapsed().as_millis(),
                    s.produced
                ));
            }
            let published = s.produced;
            for trace in s
                .traces
                .iter_mut()
                .filter(|t| !t.published && t.frame <= published)
            {
                trace.published = true;
                log.write(&format!(
                    "INPUT TRACE published id={} kind={} frame={} capture_age_us={}",
                    trace.stamp.id,
                    trace.stamp.label,
                    trace.frame,
                    trace.stamp.at.elapsed().as_micros()
                ));
            }
            if !s.boundary_seen {
                s.boundary_seen = true;
                log.write(if s.phase == Phase::Ai {
                    "NATIVE PUBLISHED first initialization frame; spectator playback follows the game's speed field"
                } else {
                    "NATIVE PUBLISHED first initialization frame; holding before next run_tick_ext"
                });
            }
            s.generation
        };
        loop {
            let keys = crate::platform_input::poll();
            let Ok(mut s) = self.state.lock() else { return };
            // Check generation under the same lock used by the wait. A wake
            // from an old session must never mutate a newly armed match.
            if s.generation != generation
                || s.worker != Some(crate::platform_input::thread_id())
                || s.sender != Some(sender)
            {
                return;
            }
            if keys.release && s.phase == Phase::Loading {
                Self::release(&mut s, "F12 at worker boundary", log);
            }
            Self::check_limits(&mut s, log);
            let wait = match s.phase {
                Phase::Released | Phase::Armed => false,
                Phase::Loading | Phase::Ready | Phase::Paused => true,
                Phase::Running | Phase::Ai => !Self::may_produce(&mut s),
            };
            if !wait {
                return;
            }
            // The viewer/phase transitions signal immediately. The timeout
            // keeps emergency keys and heartbeat recovery working if the
            // client freezes; it is not the normal running wake-up policy.
            let since = Instant::now();
            let waited = self.wake.wait_timeout(s, Duration::from_millis(25));
            crate::perf::waited(crate::perf::Wait::Pacing, since);
            let Ok((mut s, _)) = waited else { return };
            if s.phase == Phase::Ai {
                s.window.wait += since.elapsed();
            }
        }
    }
    /// Running or AI phase: the worker may simulate another tick while the
    /// queue is under the effective lead.
    fn may_produce(s: &mut State) -> bool {
        let lead = Self::effective_lead(s);
        let may = s.produced.saturating_sub(s.consumed) < lead;
        if may {
            // The next tick may fill the queue up to this lead.
            s.peak_lead = s.peak_lead.max(lead);
        }
        may
    }
    fn check_limits(s: &mut State, log: &Logger) {
        match s.phase {
            Phase::Loading if s.began.elapsed() >= Duration::from_secs(15) => {
                Self::release(s, "Startup interception not confirmed within 15s", log)
            }
            Phase::Ready | Phase::Running | Phase::Paused | Phase::Ai
                if s.heartbeat
                    .is_none_or(|t| t.elapsed() >= Duration::from_secs(2)) =>
            {
                Self::release(s, "Client heartbeat missing for 2s", log)
            }
            _ => {}
        }
    }
    pub fn before_view(&self, view: usize, played: usize, queued: usize, log: &Logger) -> ViewMode {
        let Ok(mut s) = self.state.lock() else {
            return ViewMode::Native;
        };
        self.wake.notify_all();
        Self::check_limits(&mut s, log);
        if !s.installed
            || s.key.is_none()
            || s.phase == Phase::Released
            || s.client != Some(crate::platform_input::thread_id())
        {
            return ViewMode::Native;
        }
        if s.view.is_some_and(|old| old != view) {
            Self::release(&mut s, "Viewer changed", log);
            return ViewMode::Native;
        }
        s.view = Some(view);
        if !s.first_view_seen {
            s.first_view_seen = true;
            log.write(&format!(
                "NATIVE VIEW first played_tick={played} queued={queued} boundary_seen={}",
                s.boundary_seen
            ));
        }
        if matches!(s.phase, Phase::Running | Phase::Ai) {
            ViewMode::Running
        } else if !s.bootstrap_applied && queued > 0 {
            ViewMode::Bootstrap
        } else {
            ViewMode::Paused
        }
    }
    /// Viewer, each running frame at `tps` ticks a second: keeps the
    /// worker's lead matched to the frame rate. Returns the lead the frame
    /// time is capped by: the frame lead, times the speed multiplier
    /// (rounded up) in the AI phase.
    pub fn frame_time(&self, tps: usize, log: &Logger) -> usize {
        self.frame_at(Instant::now(), tps, log)
    }
    /// Frames are timed here, on the wall clock: the game's own frame time
    /// is not known to be unscaled and uncapped.
    fn frame_at(&self, now: Instant, tps: usize, log: &Logger) -> usize {
        let Ok(mut s) = self.state.lock() else {
            return 1;
        };
        let frame = s.frame_at.replace(now).map(|t| now.duration_since(t));
        // The first frame, and the first after a pause or a held window,
        // say nothing about the frame rate.
        let Some(frame) = frame.filter(|d| !d.is_zero() && *d < FRAME_GAP) else {
            s.rate = Some((now, s.consumed));
            s.rate_unsteady = false;
            s.owed = 0.;
            return Self::effective_lead(&s);
        };
        // One long frame (a hitch) moves the average by less than the 5%
        // that raises the lead; a lasting change still gets there.
        let ticks = (frame.as_secs_f32() * tps as f32).min(s.frame_ticks + FRAME_STEP);
        s.frame_ticks += (ticks - s.frame_ticks) * FRAME_WEIGHT;
        let lead = lead_for(s.lead, s.frame_ticks);
        let frame_ms = s.frame_ticks * 1000. / tps as f32;
        if lead != s.lead {
            s.lead_changes += 1;
            if s.lead_changes <= REPORTS {
                log.write(&format!(
                    "TIMING frame lead {} -> {lead}; average client frame {frame_ms:.1} ms ({:.2} ticks); change {} of this match{}",
                    s.lead,
                    s.frame_ticks,
                    s.lead_changes,
                    if s.lead_changes == REPORTS { "; later changes are not logged" } else { "" }
                ));
            }
            if s.lead_changes <= REPORTS {
                s.lead_trace = Some(LeadTrace {
                    change: (s.lead, lead),
                    played: Vec::with_capacity(TRACE_FRAMES),
                    owed_peak: s.owed,
                    dropped_from: s.dropped,
                });
            }
            s.lead = lead;
            self.wake.notify_all();
        }
        // Playback rate over each RATE_WINDOW of uninterrupted running.
        let consumed = s.consumed;
        let (since, from) = *s.rate.get_or_insert((now, consumed));
        let window = now.duration_since(since);
        if window >= RATE_WINDOW {
            let rate = s.consumed.saturating_sub(from) as f32 / window.as_secs_f32();
            // In the AI phase the game's speed button sets the expected rate,
            // so 0.5x is not reported as slow playback. A window in which the
            // speed changed or the game paused has no single expected rate:
            // it is logged but not judged (1.5.1-dev2).
            let target = tps as f32 * s.spectator;
            let unsteady = std::mem::take(&mut s.rate_unsteady);
            let line = format!(
                "playback {rate:.1} ticks/s of {target:.0} over {:.0} s; lead={lead} average client frame {frame_ms:.1} ms changes={} dropped={:.1} ticks this match{}",
                window.as_secs_f32(),
                s.lead_changes,
                s.dropped,
                if unsteady { "; speed changed or paused in this window, not judged" } else { "" }
            );
            if !unsteady && rate < target * 0.95 && s.slow_reports < REPORTS {
                s.slow_reports += 1;
                log.write(&format!("TIMING slow {line}"));
            } else {
                log.write(&format!("PACING {line}"));
            }
            s.rate = Some((now, s.consumed));
        }
        Self::effective_lead(&s)
    }
    /// Viewer, before a running frame plays: the game's frame clock (in
    /// ticks) with whole owed ticks held back and paid back gently.
    pub fn smooth_clock(&self, clock: f32) -> f32 {
        let Ok(mut s) = self.state.lock() else {
            return clock;
        };
        let (clock, owed, dropped) = smooth(clock, s.owed);
        s.owed = owed;
        s.dropped += dropped;
        if let Some(trace) = s.lead_trace.as_mut() {
            trace.owed_peak = trace.owed_peak.max(owed);
        }
        clock
    }
    pub fn after_view(
        &self,
        mode: ViewMode,
        before: usize,
        after: usize,
        played: usize,
        log: &Logger,
    ) {
        if mode == ViewMode::Native {
            return;
        }
        let Ok(mut s) = self.state.lock() else { return };
        self.wake.notify_all();
        Self::watch_worker(&mut s, log);
        let previous_played = s.played_tick;
        s.played_tick = played;
        let used = before.saturating_sub(after);
        let queue = s.produced.saturating_sub(s.consumed);
        s.consumed += used;
        if mode == ViewMode::Running {
            Self::trace_lead(&mut s, used, log);
            if s.phase == Phase::Ai {
                Self::spectator_window(&mut s, used, queue, log);
            }
        }
        let consumed = s.consumed;
        while s
            .traces
            .front()
            .is_some_and(|t| t.published && t.frame <= consumed)
        {
            let trace = s.traces.pop_front().unwrap();
            crate::perf::playback_latency(trace.stamp.at);
            crate::input_delay::record(trace.stamp.at.elapsed());
            log.write(&format!("INPUT TRACE played id={} kind={} frame={} played_tick={played} capture_to_playback_us={}; frame playback, not measured movement/projectile onset", trace.stamp.id, trace.stamp.label, trace.frame, trace.stamp.at.elapsed().as_micros()));
        }
        if mode == ViewMode::Running
            && before == 0
            && s.running
                .is_some_and(|t| t.elapsed() > Duration::from_millis(250))
        {
            // Bounded samples include empty queues: do not claim every empty
            // queue is a hitch without observing the host's rendered motion.
            if s.viewer_calls.is_multiple_of(60) {
                log.write(&format!(
                    "PACING empty_queue played_tick={played} produced={} consumed={} mode={mode:?}",
                    s.produced, s.consumed
                ));
            }
        }
        // Spectator start consumes its initial frame in Running mode. Record
        // it so a later F11 takeover/pause never bootstraps a second frame.
        if mode == ViewMode::Running && used > 0 {
            s.bootstrap_applied = true;
        }
        if mode == ViewMode::Bootstrap && used > 0 {
            s.bootstrap_applied = true;
            log.write(&format!("NATIVE INITIAL_FRAME applied={used} played_tick={played} queue_remaining={after}; zero-gameplay advance is NOT assumed"));
        }
        if mode == ViewMode::Paused {
            if used != 0 || s.bootstrap_applied && played != previous_played {
                Self::release(&mut s, "Playback advanced while held", log);
            } else if s.bootstrap_applied && !s.pause_acknowledged {
                s.pause_acknowledged = true;
                log.write(&format!(
                    "NATIVE PAUSE_ACK played_tick={played}; held update consumed zero frames"
                ));
            }
        }
        if used > Self::frame_limit(&s) || s.consumed > s.produced + 1 {
            Self::release(&mut s, "Unexpected publication/playback frame counts", log);
            return;
        }
        // The queue left now (plus a tick the worker may be sending) bounds
        // what later frames can play; once it is under the current lead, the
        // lead alone bounds it again.
        let lead = Self::effective_lead(&s);
        let queue = s.produced.saturating_sub(s.consumed);
        s.peak_lead = lead.max(queue + 1);
        if let Some((from, frames)) = s.drain {
            let frames = frames + 1;
            s.drain = Some((from, frames));
            if queue <= lead {
                s.drain = None;
                log.write(&format!(
                    "SPECTATOR drained lead {from} -> {lead} queue={queue} after {frames} frames"
                ));
            }
        }
    }
    /// After a lead change: one running frame played `used` ticks.
    fn trace_lead(s: &mut State, used: usize, log: &Logger) {
        let dropped = s.dropped;
        let Some(trace) = s.lead_trace.as_mut() else {
            return;
        };
        trace.played.push(used);
        if trace.played.len() < TRACE_FRAMES {
            return;
        }
        let played: String = trace
            .played
            .iter()
            .map(|n| (*n).min(9).to_string())
            .collect();
        log.write(&format!(
            "TIMING after lead {} -> {}: ticks per frame {played}; owed up to {:.2} ticks; dropped {:.2} ticks",
            trace.change.0,
            trace.change.1,
            trace.owed_peak,
            dropped - trace.dropped_from
        ));
        s.lead_trace = None;
    }
    /// Viewer, each frame: a running match whose worker has published
    /// nothing for crate::worker_watch::STALL is reported once (not while
    /// paused, and not while the worker legitimately waits for playback).
    fn watch_worker(s: &mut State, log: &Logger) {
        let now = Instant::now();
        if !matches!(s.phase, Phase::Running | Phase::Ai) || s.produced > s.consumed {
            s.progress = Some(now);
            // A reported stall stays reported (and its stack logged) while
            // the player pauses or hands back a match that is still frozen.
            if s.stalled.is_none() {
                return;
            }
        }
        let since = *s.progress.get_or_insert(now);
        if s.stalled.is_none() && now.duration_since(since) >= crate::worker_watch::STALL {
            s.stalled = Some(since);
            // A finished match ends its worker; the result screen can take
            // longer than the stall time to appear. That is not a stall.
            if s.worker
                .is_some_and(|worker| !crate::native_adapter::thread_alive(worker))
            {
                s.stack_logged = Some((now, 3));
                log.write(&format!(
                    "WORKER ENDED the match worker thread has exited (normal when a match finishes); played_tick={} produced={} consumed={} phase={:?}",
                    s.played_tick, s.produced, s.consumed, s.phase
                ));
                return;
            }
            let (step, player) = crate::worker_watch::last();
            let (begun, last_player) = crate::worker_watch::progress();
            log.write(&format!(
                "WORKER STALL no frame for {} ms; played_tick={} produced={} consumed={} phase={:?} last_step={step:?} player={player:?} players_begun={begun} last_player={last_player:?}",
                now.duration_since(since).as_millis(),
                s.played_tick,
                s.produced,
                s.consumed,
                s.phase
            ));
        }
        // The worker's call stack, three times two seconds apart: the same
        // stack each time is a wait, a changing one is a loop.
        let due = match s.stack_logged {
            None => true,
            Some((at, count)) => count < 3 && now.duration_since(at) >= Duration::from_secs(2),
        };
        if let (Some(_), true, Some(worker)) = (s.stalled, due, s.worker) {
            let count = s.stack_logged.map_or(1, |(_, count)| count + 1);
            s.stack_logged = Some((now, count));
            log.write(&match crate::native_adapter::thread_stack(worker) {
                Ok(frames) => format!(
                    "WORKER STALL stack {count}/3 step={:?} frames={frames:?}",
                    crate::worker_watch::last().0
                ),
                Err(why) => format!("WORKER STALL stack {count}/3 unavailable: {why}"),
            });
        }
    }
    pub fn trace_dispatch(
        &self,
        key: MatchKey,
        stamp: crate::input_trace::Stamp,
        input: &mod_api_stable::InputV1,
        log: &Logger,
    ) {
        let Ok(mut s) = self.state.lock() else { return };
        if s.key != Some(key)
            || s.worker != Some(crate::platform_input::thread_id())
            || s.phase != Phase::Running
            || (s.trace_count >= 300
                && !crate::perf::capture_enabled()
                && !crate::input_delay::shown())
        {
            return;
        }
        s.trace_count = s.trace_count.saturating_add(1);
        // A stalled viewer cannot grow the diagnostics queue without limit.
        if s.traces.len() >= 64 {
            s.traces.pop_front();
        }
        let frame = s.produced + 1;
        s.traces.push_back(crate::input_trace::FrameTrace {
            stamp,
            frame,
            published: false,
        });
        log.write(&format!(
            "INPUT TRACE dispatched id={} kind={} frame={frame} capture_age_us={} input={input:?}",
            stamp.id,
            stamp.label,
            stamp.at.elapsed().as_micros()
        ));
    }
    pub fn permit_native_trace(&self) -> bool {
        let Ok(mut s) = self.state.lock() else {
            return false;
        };
        if s.phase != Phase::Running
            || s.worker != Some(crate::platform_input::thread_id())
            || s.native_trace_count >= 600
        {
            return false;
        }
        s.native_trace_count += 1;
        true
    }
    pub fn allows_input(&self, key: MatchKey) -> bool {
        // Direction freshness is enforced by Movement with a hold command,
        // not by relinquishing input ownership. The publication/client guard
        // still explicitly releases after a two-second heartbeat loss.
        self.state.lock().is_ok_and(|s| {
            matches!(s.phase, Phase::Running | Phase::Paused)
                && s.key == Some(key)
                && s.worker == Some(crate::platform_input::thread_id())
                && s.battlefield
                && s.selected
        })
    }
    /// Read-only HUD/result observation continues on the original worker after
    /// Return to AI, but stops at battlefield exit and excludes later re-sims.
    pub fn accepts_sample(&self, key: MatchKey) -> bool {
        self.state.lock().is_ok_and(|s| {
            s.key == Some(key)
                && s.battlefield
                && s.worker == Some(crate::platform_input::thread_id())
        })
    }
    pub fn client_early_input(&self) -> bool {
        self.state.lock().is_ok_and(|s| {
            s.phase == Phase::Running && s.pending_action.is_none() && Self::client_owned(&s, None)
        })
    }
    pub fn client_running(&self) -> bool {
        self.state.lock().is_ok_and(|s| {
            s.phase == Phase::Running
                && s.battlefield
                && s.selected
                && s.client == Some(crate::platform_input::thread_id())
        })
    }
    /// Bound client only; never intercept another viewer or a released session.
    pub fn client_controls(&self, view: Option<usize>) -> bool {
        self.state
            .lock()
            .is_ok_and(|s| s.phase != Phase::Ai && Self::client_owned(&s, view))
    }
    /// AI handoff retains pacing and the same viewer, but releases manual input.
    pub fn client_session(&self, view: Option<usize>) -> bool {
        self.state
            .lock()
            .is_ok_and(|s| Self::client_owned(&s, view))
    }
    fn client_owned(s: &State, view: Option<usize>) -> bool {
        matches!(
            s.phase,
            Phase::Ready | Phase::Running | Phase::Paused | Phase::Ai
        ) && s.battlefield
            && s.selected
            && s.client == Some(crate::platform_input::thread_id())
            && view.is_none_or(|view| s.view == Some(view))
            && s.heartbeat
                .is_some_and(|t| t.elapsed() < Duration::from_secs(2))
    }
    pub fn describe(&self) -> String {
        let Ok(s) = self.state.lock() else {
            return "Native coordinator unavailable".into();
        };
        if !self.enabled {
            return "LT: observation only; native adapter disabled".into();
        }
        match s.phase {
            Phase::Armed => format!(
                "LT {}: native adapter armed; choose your lane before match",
                env!("CARGO_PKG_VERSION")
            ),
            Phase::Loading if !s.boundary_seen => format!(
                "Waiting for live worker hook | worker calls={} viewer calls={} | F12 cancels",
                s.worker_calls, s.viewer_calls
            ),
            Phase::Loading if !s.pause_acknowledged => format!(
                "Worker held; awaiting playback stop confirmation | published={} applied={}",
                s.produced, s.consumed
            ),
            Phase::Loading => {
                "Playback hold acknowledged; waiting for battlefield and your champion".into()
            }
            Phase::Ready => format!(
                "Ready: click Start control | native tick={} | F12 emergency release",
                s.played_tick
            ),
            Phase::Running => format!(
                "Control running {:.1}s | generated={} shown={} | Pause / Return to AI",
                s.running.map_or(0., |t| t.elapsed().as_secs_f32()),
                s.produced,
                s.consumed
            ),
            Phase::Paused => {
                "Control paused | Camera and Tab available | Resume / Return to AI".into()
            }
            Phase::Ai => "AI control | F11 or Take control returns to your champion".into(),
            Phase::Released => format!("Control inactive: {}", s.reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::logger;
    fn active() -> NativeTiming {
        let t = NativeTiming::new(true);
        let mut s = t.state.lock().unwrap();
        s.installed = true;
        s.phase = Phase::Loading;
        s.key = Some((1, 2, 3));
        s.client = Some(crate::platform_input::thread_id());
        drop(s);
        t
    }
    #[test]
    fn preparation_auto_start_keeps_shop_eligibility_but_requires_no_role_fallback() {
        let log = logger("start-modes");
        for (automatic, fallback, expected) in [
            (false, false, false),
            (true, false, true),
            (true, true, false),
        ] {
            let t = active();
            {
                let mut s = t.state.lock().unwrap();
                s.boundary_seen = true;
                s.bootstrap_applied = true;
                s.pause_acknowledged = true;
                s.role_fallback = fallback;
                s.startup_shop_pending = true;
            }
            t.heartbeat_with_start(true, true, Keys::default(), automatic, &log);
            assert_eq!(t.ui_phase(), Some(Phase::Ready));
            let action = t.take_action();
            assert_eq!(action.is_some(), expected);
            if let Some(action) = action {
                assert_eq!(action, SessionAction::Start);
                t.apply_action(action, &log);
                assert_eq!(t.ui_phase(), Some(Phase::Running));
                assert!(t.claim_startup_shop());
                assert!(!t.claim_startup_shop());
            }
        }
    }
    #[test]
    fn spectator_start_consumes_initial_frame_once_and_can_take_over_and_pause() {
        let log = logger("spectator-start");
        let t = NativeTiming::new(true);
        {
            let mut s = t.state.lock().unwrap();
            s.installed = true;
            s.client = Some(crate::platform_input::thread_id().wrapping_add(1));
        }
        let key = (31, 32, 33);
        t.observe_start(key, 1, true, true, &log);
        {
            let mut s = t.state.lock().unwrap();
            assert_eq!(s.phase, Phase::Ai);
            s.client = Some(crate::platform_input::thread_id());
            s.heartbeat = Some(Instant::now());
            s.battlefield = true;
            s.selected = true;
            s.produced = 1;
            s.boundary_seen = true;
        }
        t.after_view(ViewMode::Running, 1, 0, 1, &log);
        assert!(t.state.lock().unwrap().bootstrap_applied);
        t.request_action(true);
        let a = t.take_action().unwrap();
        assert_eq!(a, SessionAction::TakeControl);
        t.apply_action(a, &log);
        assert!(!t.claim_startup_shop());
        t.request_action(true);
        let a = t.take_action().unwrap();
        assert_eq!(a, SessionAction::Pause);
        t.apply_action(a, &log);
        t.after_view(ViewMode::Paused, 0, 0, 1, &log);
        assert_eq!(t.ui_phase(), Some(Phase::Paused));
        assert!(t.state.lock().unwrap().pause_acknowledged);
    }
    #[test]
    fn watch_or_handoff_discards_delayed_startup_shop_until_next_match() {
        let log = logger("startup-shop-handoff");
        for phase in [Phase::Ready, Phase::Running, Phase::Paused] {
            let t = active();
            {
                let mut s = t.state.lock().unwrap();
                s.phase = phase;
                s.battlefield = true;
                s.selected = true;
                s.heartbeat = Some(Instant::now());
                s.startup_shop_pending = true;
            }
            t.request_action(false);
            t.apply_action(t.take_action().unwrap(), &log);
            assert_eq!(t.ui_phase(), Some(Phase::Ai));
            t.request_action(true);
            t.apply_action(t.take_action().unwrap(), &log);
            assert_eq!(t.ui_phase(), Some(Phase::Running));
            assert!(!t.claim_startup_shop());
            // A later pause or a now-ready shop plan cannot recreate the claim.
            t.state.lock().unwrap().phase = Phase::Paused;
            assert!(!t.claim_startup_shop());
            t.state.lock().unwrap().battlefield = false;
            t.rearm(true, Keys::default(), &log);
            let key = (91, 92, 93);
            t.state.lock().unwrap().client =
                Some(crate::platform_input::thread_id().wrapping_add(1));
            t.observe_start(key, 1, false, true, &log);
            assert!(t.state.lock().unwrap().startup_shop_pending);
        }
    }
    #[test]
    fn a_left_match_is_recognised_by_key_only() {
        assert_ne!(key_id((0, 0, 0)), 0);
        LEFT_MATCH.store(key_id((7, 8, 9)), std::sync::atomic::Ordering::Relaxed);
        assert!(left_match((7, 8, 9)));
        assert!(!left_match((7, 8, 10)));
        LEFT_MATCH.store(0, std::sync::atomic::Ordering::Relaxed);
    }
    #[test]
    fn a_running_worker_without_frames_is_reported_once() {
        let log = logger("worker-stall");
        let t = active();
        let mut s = t.state.lock().unwrap();
        s.phase = Phase::Running;
        (s.produced, s.consumed) = (5, 5);
        let old = Instant::now() - Duration::from_secs(4);
        s.progress = Some(old);
        NativeTiming::watch_worker(&mut s, &log);
        assert_eq!(s.stalled, Some(old));
        NativeTiming::watch_worker(&mut s, &log);
        assert_eq!(s.stalled, Some(old));
        // Waiting for playback (a frame queued) or paused is not a stall.
        s.stalled = None;
        s.progress = Some(old);
        s.produced = 6;
        NativeTiming::watch_worker(&mut s, &log);
        assert!(s.stalled.is_none() && s.progress != Some(old));
        s.produced = 5;
        s.phase = Phase::Paused;
        s.progress = Some(old);
        NativeTiming::watch_worker(&mut s, &log);
        assert!(s.stalled.is_none() && s.progress != Some(old));
    }
    #[test]
    fn repeated_handoffs_keep_binding_pacing_and_never_reclaim_on_mouse_input() {
        let t = active();
        let log = logger("repeated-handoff");
        t.heartbeat(true, true, Keys::default(), &log);
        {
            let mut s = t.state.lock().unwrap();
            s.phase = Phase::Running;
            s.worker = Some(crate::platform_input::thread_id());
            s.view = Some(100);
            s.sender = Some(42);
            s.produced = 50;
            s.consumed = 49;
            s.played_tick = 49;
            s.bootstrap_applied = true;
        }
        for _ in 0..4 {
            t.request_action(false);
            t.apply_action(t.take_action().unwrap(), &log);
            assert_eq!(t.phase(), Some(Phase::Ai));
            assert!(t.client_session(Some(100)));
            assert!(!t.client_session(Some(200)));
            assert!(!t.client_controls(None));
            assert!(!t.client_early_input());
            assert!(!t.allows_input((1, 2, 3)));
            t.heartbeat(
                true,
                true,
                Keys {
                    left: true,
                    right: true,
                    ..Keys::default()
                },
                &log,
            );
            assert!(t.take_action().is_none());
            assert_eq!(t.before_view(100, 49, 1, &log), ViewMode::Running);
            t.heartbeat(
                true,
                true,
                Keys {
                    start: true,
                    ..Keys::default()
                },
                &log,
            );
            assert_eq!(t.take_action(), Some(SessionAction::TakeControl));
            t.apply_action(SessionAction::TakeControl, &log);
            assert!(t.client_controls(Some(100)));
            assert!(t.allows_input((1, 2, 3)));
            assert!(!t.allows_input((1, 2, 4)));
            t.heartbeat(true, true, Keys::default(), &log);
            let s = t.state.lock().unwrap();
            assert_eq!(
                (
                    s.generation,
                    s.key,
                    s.sender,
                    s.view,
                    s.produced,
                    s.consumed,
                    s.played_tick
                ),
                (0, Some((1, 2, 3)), Some(42), Some(100), 50, 49, 49)
            );
        }
    }
    #[test]
    fn f12_is_an_edge_and_ai_heartbeat_failure_cannot_be_reclaimed() {
        let t = active();
        let log = logger("handoff-edges");
        t.state.lock().unwrap().phase = Phase::Running;
        let f12 = Keys {
            release: true,
            ..Keys::default()
        };
        t.heartbeat(true, true, f12, &log);
        t.apply_action(t.take_action().unwrap(), &log);
        t.request_action(true);
        t.apply_action(t.take_action().unwrap(), &log);
        t.heartbeat(true, true, f12, &log);
        assert!(t.take_action().is_none()); // Held F12 cannot undo a button reclaim.
        t.request_action(false);
        t.apply_action(t.take_action().unwrap(), &log);
        t.state.lock().unwrap().heartbeat = Some(Instant::now() - Duration::from_secs(3));
        assert_eq!(t.before_view(100, 0, 0, &log), ViewMode::Native);
        assert_eq!(t.ui_phase(), Some(Phase::Released));
        t.heartbeat(
            true,
            true,
            Keys {
                start: true,
                ..Keys::default()
            },
            &log,
        );
        t.request_action(true);
        assert!(t.take_action().is_none());
        t.apply_action(SessionAction::TakeControl, &log);
        assert_eq!(t.ui_phase(), Some(Phase::Released));
    }
    #[test]
    fn f11_pauses_while_running_and_resumes_when_paused() {
        let t = active();
        let log = logger("f11-toggle");
        t.state.lock().unwrap().phase = Phase::Running;
        let f11 = Keys {
            start: true,
            ..Keys::default()
        };
        // Pressing F11 while running queues the Pause button's action.
        t.heartbeat(true, true, f11, &log);
        assert_eq!(t.take_action(), Some(SessionAction::Pause));
        t.apply_action(SessionAction::Pause, &log);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Paused);
        // Holding F11 does nothing more; a new press resumes.
        t.heartbeat(true, true, f11, &log);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Paused);
        t.heartbeat(true, true, Keys::default(), &log);
        t.heartbeat(true, true, f11, &log);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Running);
    }
    #[test]
    fn the_simulation_worker_sees_the_phase_the_client_only_api_hides() {
        let t = active();
        {
            let mut s = t.state.lock().unwrap();
            // A different client thread: this test thread is only the worker.
            s.client = Some(u64::MAX);
            s.worker = Some(crate::platform_input::thread_id());
            s.phase = Phase::Loading;
        }
        assert_eq!(t.phase(), None);
        assert_eq!(t.worker_phase((1, 2, 3)), Some(Phase::Loading));
        assert_eq!(t.worker_phase((9, 9, 9)), None);
    }
    #[test]
    fn pending_pause_prevents_early_capture_and_trace_does_not_cross_sessions() {
        let t = active();
        let log = logger("early-phase-and-trace");
        t.heartbeat(true, true, Keys::default(), &log);
        {
            let mut s = t.state.lock().unwrap();
            s.phase = Phase::Running;
            s.worker = Some(crate::platform_input::thread_id());
        }
        assert!(t.client_early_input());
        t.request_action(true);
        assert!(!t.client_early_input());
        let stamp = crate::input_trace::Stamp::new("right-click");
        t.trace_dispatch(
            (1, 2, 4),
            stamp,
            &mod_api_stable::InputV1::move_to(20, 20),
            &log,
        );
        assert!(t.state.lock().unwrap().traces.is_empty());
        t.trace_dispatch(
            (1, 2, 3),
            stamp,
            &mod_api_stable::InputV1::move_to(20, 20),
            &log,
        );
        assert_eq!(t.state.lock().unwrap().traces.len(), 1);
        {
            let mut s = t.state.lock().unwrap();
            s.produced = 1;
            s.traces[0].published = true;
        }
        t.after_view(ViewMode::Running, 1, 0, 1, &log);
        assert!(t.state.lock().unwrap().traces.is_empty());
        t.cancel("test", &log);
        t.heartbeat(false, false, Keys::default(), &log);
        t.rearm(false, Keys::default(), &log);
        assert_eq!(t.state.lock().unwrap().trace_count, 0);
        assert!(!t.client_early_input());
    }
    #[test]
    fn portrait_selection_waits_for_start_without_a_ready_expiry_but_keeps_heartbeat_guard() {
        let t = active();
        let log = logger("prepared-selection-wait");
        t.heartbeat(true, true, Keys::default(), &log);
        {
            let mut s = t.state.lock().unwrap();
            s.phase = Phase::Ready;
            s.began = Instant::now() - Duration::from_secs(3600);
            s.produced = 1;
            s.consumed = 1;
            s.played_tick = 1;
            s.bootstrap_applied = true;
            s.pause_acknowledged = true;
        }
        assert_eq!(t.before_view(100, 0, 1, &log), ViewMode::Paused);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Ready);
        assert_eq!(t.state.lock().unwrap().played_tick, 1);
        t.state.lock().unwrap().heartbeat = Some(Instant::now() - Duration::from_secs(3));
        assert_eq!(t.before_view(100, 0, 1, &log), ViewMode::Native);
    }
    #[test]
    fn next_battle_clears_binding_and_rejects_same_battle_reentry() {
        let t = active();
        let log = logger("next-battle");
        t.heartbeat(true, true, Keys::default(), &log);
        {
            let mut s = t.state.lock().unwrap();
            s.phase = Phase::Running;
            s.worker = Some(99);
            s.view = Some(100);
            s.sender = Some(42);
            s.produced = 50;
            s.consumed = 49;
            s.played_tick = 49;
            s.bootstrap_applied = true;
        }
        t.request_action(false);
        t.apply_action(t.take_action().unwrap(), &log);
        assert!(!t.needs_rearm(true, false)); // Still on the battlefield.
        t.heartbeat(false, false, Keys::default(), &log);
        assert!(!t.needs_rearm(false, false)); // Result/feedback/replay screen.
        assert!(t.needs_rearm(true, false));
        t.rearm(
            false,
            Keys {
                start: true,
                ..Keys::default()
            },
            &log,
        );
        {
            let s = t.state.lock().unwrap();
            assert_eq!(s.phase, Phase::Armed);
            assert!(
                s.key.is_none() && s.worker.is_none() && s.sender.is_none() && s.view.is_none()
            );
            assert_eq!((s.produced, s.consumed, s.played_tick), (0, 0, 0));
            assert!(!s.bootstrap_applied && !s.pause_acknowledged && !s.selected);
            assert!(s.previous_start && s.pending_action.is_none());
        }
        assert!(!t.needs_rearm(true, false));
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    t.observe((1, 2, 3), 1, &log); // Late same-battle analysis.
                    assert!(!t.owns_worker((1, 2, 3)));
                    t.observe((4, 2, 4), 1, &log); // Next set, same match id.
                    assert!(t.owns_worker((4, 2, 4)));
                })
                .join()
                .unwrap();
        });
        assert_eq!(t.match_key(), Some((4, 2, 4)));
        assert_eq!(t.state.lock().unwrap().phase, Phase::Loading);
        assert!(!t.client_controls(None)); // Must bootstrap/confirm again.
        assert!(!t.allows_input((1, 2, 3)));
    }
    #[test]
    fn title_boundary_allows_reloading_the_same_saved_battle_without_old_state() {
        let t = active();
        let log = logger("reload-save");
        t.cancel("Left battlefield", &log);
        t.rearm(false, Keys::default(), &log);
        assert!(t.state.lock().unwrap().retired.contains(&(1, 2, 3)));
        assert!(t.needs_rearm(false, true));
        t.rearm(true, Keys::default(), &log);
        assert_eq!(t.generation(), 2);
        t.set_binding_window(false);
        std::thread::scope(|scope| {
            scope
                .spawn(|| t.observe((1, 2, 3), 1, &log))
                .join()
                .unwrap();
        });
        assert!(t.match_key().is_none()); // No binding at title/result/feedback.
        t.set_binding_window(true);
        std::thread::scope(|scope| {
            scope
                .spawn(|| t.observe((1, 2, 3), 1, &log))
                .join()
                .unwrap();
        });
        assert_eq!(t.match_key(), Some((1, 2, 3)));
        assert_eq!(t.state.lock().unwrap().phase, Phase::Loading);
        assert!(!t.needs_rearm(true, false));
        t.installed(false, "adapter failure", &log);
        assert!(!t.needs_rearm(true, true)); // Never revive a failed install.
    }
    #[test]
    fn old_publication_wait_returns_after_rearm_without_touching_new_session() {
        use std::sync::{mpsc, Arc};
        let t = Arc::new(active());
        let log = Arc::new(logger("old-worker-generation"));
        let (done, receive) = mpsc::channel();
        let worker_t = t.clone();
        let worker_log = log.clone();
        let worker = std::thread::spawn(move || {
            worker_t.state.lock().unwrap().worker = Some(crate::platform_input::thread_id());
            worker_t.after_publication(42, &worker_log);
            done.send(()).unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(1);
        while !t.state.lock().unwrap().boundary_seen {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        // Keep the same OS worker id, so generation (not thread mismatch) must
        // break the old wait. Rearm and emulate the next bind under one lock.
        let old_worker = t.state.lock().unwrap().worker;
        t.cancel("Left battlefield", &log);
        t.rearm(false, Keys::default(), &log);
        {
            let mut s = t.state.lock().unwrap();
            s.key = Some((4, 2, 4));
            s.worker = old_worker;
            s.sender = Some(42);
            s.phase = Phase::Loading;
        }
        receive.recv_timeout(Duration::from_secs(1)).unwrap();
        worker.join().unwrap();
        assert_eq!(t.state.lock().unwrap().phase, Phase::Loading);
        assert_eq!(t.state.lock().unwrap().produced, 0);
    }
    #[test]
    fn spectator_ownership_is_bound_to_client_view_and_heartbeat_without_test_cutoff() {
        let t = active();
        let log = logger("spectator-scope");
        t.heartbeat(true, true, Keys::default(), &log);
        {
            let mut s = t.state.lock().unwrap();
            s.phase = Phase::Ready;
            s.view = Some(100);
        }
        assert!(t.client_controls(Some(100)));
        assert!(!t.client_controls(Some(200)));
        std::thread::scope(|scope| {
            assert!(!scope.spawn(|| t.client_controls(Some(100))).join().unwrap());
        });
        t.state.lock().unwrap().heartbeat = Some(Instant::now() - Duration::from_secs(3));
        assert!(!t.client_controls(Some(100)));
        {
            let mut s = t.state.lock().unwrap();
            s.heartbeat = Some(Instant::now());
            s.phase = Phase::Running;
            s.running = Some(Instant::now() - Duration::from_secs(61));
        }
        assert!(t.client_controls(Some(100))); // 61 seconds no longer ends control.
        t.cancel("release", &log);
        assert!(!t.client_controls(None));
    }
    #[test]
    fn read_only_sampling_keeps_original_worker_after_release_but_stops_at_exit() {
        let t = active();
        {
            let mut s = t.state.lock().unwrap();
            s.battlefield = true;
            s.phase = Phase::Released;
            s.worker = Some(crate::platform_input::thread_id());
        }
        assert!(t.accepts_sample((1, 2, 3)));
        assert!(!t.accepts_sample((1, 2, 4)));
        std::thread::scope(|s| {
            assert!(!s.spawn(|| t.accepts_sample((1, 2, 3))).join().unwrap());
        });
        t.state.lock().unwrap().battlefield = false;
        assert!(!t.accepts_sample((1, 2, 3)));
    }
    #[test]
    fn session_buttons_start_pause_resume_and_release_with_scoped_requests() {
        let t = active();
        let log = logger("session-buttons");
        t.heartbeat(true, true, Keys::default(), &log);
        {
            let mut s = t.state.lock().unwrap();
            s.phase = Phase::Ready;
            s.view = Some(100);
            s.worker = Some(crate::platform_input::thread_id());
            s.bootstrap_applied = true;
            s.played_tick = 1;
            s.produced = 3;
            s.consumed = 1;
        }
        t.request_action(true);
        assert_eq!(t.take_action(), Some(SessionAction::Start));
        t.apply_action(SessionAction::Start, &log);
        t.state.lock().unwrap().running = Some(Instant::now() - Duration::from_secs(3600));
        assert!(t.client_running());
        assert!(t.allows_input((1, 2, 3)));
        t.request_action(true);
        let pause = t.take_action().unwrap();
        assert_eq!(pause, SessionAction::Pause);
        t.apply_action(pause, &log);
        assert!(!t.client_running());
        assert!(t.client_controls(Some(100))); // HUD/camera remain owned.
        assert!(t.allows_input((1, 2, 3))); // In-flight tick cannot regain native AI.
        assert_eq!(t.before_view(100, 1, 2, &log), ViewMode::Paused);
        t.after_view(ViewMode::Paused, 2, 2, 1, &log);
        assert!(t.state.lock().unwrap().pause_acknowledged);
        t.request_action(true);
        let resume = t.take_action().unwrap();
        assert_eq!(resume, SessionAction::Resume);
        t.apply_action(resume, &log);
        assert_eq!(t.before_view(100, 1, 2, &log), ViewMode::Running);
        assert!(t.allows_input((1, 2, 3)));
        t.request_action(false);
        t.apply_action(t.take_action().unwrap(), &log);
        assert!(!t.client_controls(None));
        assert!(!t.allows_input((1, 2, 3)));
        assert_eq!(t.state.lock().unwrap().phase, Phase::Ai);
        assert_eq!(t.before_view(100, 1, 2, &log), ViewMode::Running);
        t.request_action(true);
        assert_eq!(t.take_action(), Some(SessionAction::TakeControl));
        t.apply_action(SessionAction::TakeControl, &log);
        assert!(t.client_controls(Some(100)));
        assert!(t.allows_input((1, 2, 3)));
        let s = t.state.lock().unwrap();
        assert_eq!(
            (s.key, s.view, s.produced, s.consumed, s.played_tick),
            (Some((1, 2, 3)), Some(100), 3, 1, 1)
        );
    }
    #[test]
    fn queued_button_actions_expire_on_phase_or_key_changes_and_pause_retains_guards() {
        let t = active();
        let log = logger("session-stale-buttons");
        t.heartbeat(true, true, Keys::default(), &log);
        t.request_action(true); // Loading clicks never queue Start.
        assert!(t.take_action().is_none());
        t.state.lock().unwrap().phase = Phase::Ready;
        t.request_action(true);
        t.state.lock().unwrap().phase = Phase::Running;
        assert!(t.take_action().is_none());
        t.request_action(true);
        t.state.lock().unwrap().key = Some((1, 2, 4));
        assert!(t.take_action().is_none());
        t.request_action(true);
        t.request_action(false);
        t.request_action(true);
        assert_eq!(t.take_action(), Some(SessionAction::ReturnAi));
        t.state.lock().unwrap().phase = Phase::Paused;
        t.state.lock().unwrap().began = Instant::now() - Duration::from_secs(3600);
        assert_eq!(t.before_view(100, 0, 0, &log), ViewMode::Paused); // No READY expiry while paused.
        t.state.lock().unwrap().heartbeat = Some(Instant::now() - Duration::from_secs(3));
        assert_eq!(t.before_view(100, 0, 0, &log), ViewMode::Native);
        assert_eq!(
            t.state.lock().unwrap().reason,
            "Client heartbeat missing for 2s"
        );
    }
    #[test]
    fn paused_worker_waits_at_publication_and_resume_or_return_ai_wakes_it() {
        use std::sync::{mpsc, Arc};
        let t = Arc::new(active());
        let log = Arc::new(logger("session-pause-boundary"));
        t.heartbeat(true, true, Keys::default(), &log);
        {
            let mut s = t.state.lock().unwrap();
            s.phase = Phase::Running;
            s.bootstrap_applied = true;
            s.played_tick = 1;
            s.produced = 1;
            s.consumed = 1;
        }
        t.apply_action(SessionAction::Pause, &log);
        let (send, receive) = mpsc::channel();
        let (go, proceed) = mpsc::channel();
        let worker_t = t.clone();
        let worker_log = log.clone();
        let worker = std::thread::spawn(move || {
            worker_t.state.lock().unwrap().worker = Some(crate::platform_input::thread_id());
            assert!(worker_t.allows_input((1, 2, 3)));
            worker_t.after_publication(42, &worker_log);
            send.send("resumed").unwrap();
            proceed.recv_timeout(Duration::from_secs(1)).unwrap();
            worker_t.after_publication(42, &worker_log);
            send.send("released").unwrap();
        });
        let wait_produced = |n| {
            let deadline = Instant::now() + Duration::from_secs(1);
            while t.state.lock().unwrap().produced < n {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
        };
        wait_produced(2);
        assert!(receive.recv_timeout(Duration::from_millis(10)).is_err());
        assert_eq!(t.before_view(100, 1, 1, &log), ViewMode::Paused);
        t.after_view(ViewMode::Paused, 1, 1, 1, &log);
        t.apply_action(SessionAction::Resume, &log);
        assert!(receive.recv_timeout(Duration::from_millis(10)).is_err());
        t.after_view(ViewMode::Running, 1, 0, 2, &log);
        assert_eq!(
            receive.recv_timeout(Duration::from_secs(1)).unwrap(),
            "resumed"
        );
        t.apply_action(SessionAction::Pause, &log);
        go.send(()).unwrap();
        wait_produced(3);
        assert!(receive.recv_timeout(Duration::from_millis(10)).is_err());
        t.apply_action(SessionAction::ReturnAi, &log);
        assert!(receive.recv_timeout(Duration::from_millis(10)).is_err());
        // AI remains paced; handoff never allows the worker to run ahead.
        t.after_view(ViewMode::Running, 1, 0, 3, &log);
        assert_eq!(
            receive.recv_timeout(Duration::from_secs(1)).unwrap(),
            "released"
        );
        worker.join().unwrap();
    }
    #[test]
    fn ready_needs_published_applied_selected_and_battlefield() {
        let t = active();
        let log = logger("native-ready");
        t.heartbeat(true, true, Keys::default(), &log);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Loading);
        {
            let mut s = t.state.lock().unwrap();
            s.boundary_seen = true;
            s.bootstrap_applied = true;
        }
        t.heartbeat(true, false, Keys::default(), &log);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Loading);
        t.heartbeat(true, true, Keys::default(), &log);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Loading);
        t.after_view(ViewMode::Paused, 0, 0, 0, &log);
        t.heartbeat(true, true, Keys::default(), &log);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Ready);
        t.heartbeat(
            true,
            true,
            Keys {
                start: true,
                ..Keys::default()
            },
            &log,
        );
        assert_eq!(t.state.lock().unwrap().phase, Phase::Running);
        t.heartbeat(
            true,
            true,
            Keys {
                release: true,
                ..Keys::default()
            },
            &log,
        );
        assert_eq!(t.take_action(), Some(SessionAction::ReturnAi));
        t.apply_action(SessionAction::ReturnAi, &log);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Ai);
    }
    #[test]
    fn viewer_bootstraps_once_then_holds_and_rejects_view_change() {
        let t = active();
        let log = logger("native-view");
        assert_eq!(t.before_view(100, 0, 0, &log), ViewMode::Paused);
        assert_eq!(t.before_view(100, 0, 1, &log), ViewMode::Bootstrap);
        t.after_view(ViewMode::Bootstrap, 1, 0, 1, &log);
        assert_eq!(t.before_view(100, 1, 1, &log), ViewMode::Paused);
        assert_eq!(t.before_view(200, 1, 1, &log), ViewMode::Native);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Released);
    }
    #[test]
    fn paused_consumer_and_loading_timeout_fail_open() {
        let t = active();
        let log = logger("native-guards");
        t.after_view(ViewMode::Paused, 1, 0, 1, &log);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Released);
        let t = active();
        t.state.lock().unwrap().began = Instant::now() - Duration::from_secs(16);
        assert_eq!(t.before_view(100, 0, 0, &log), ViewMode::Native);
    }

    #[test]
    fn sdk_observation_returns_before_ready_and_only_publication_boundary_waits() {
        use std::sync::{mpsc, Arc};
        let t = Arc::new(NativeTiming::new(true));
        let log = Arc::new(logger("native-boundary"));
        t.installed(true, "test", &log);
        t.heartbeat(false, false, Keys::default(), &log);
        let (send, receive) = mpsc::channel();
        let worker_t = t.clone();
        let worker_log = log.clone();
        let worker = std::thread::spawn(move || {
            worker_t.observe((1, 2, 3), 1, &worker_log);
            send.send("SDK returned").unwrap();
            worker_t.after_publication(42, &worker_log);
            assert!(worker_t.allows_input((1, 2, 3)));
            assert!(!worker_t.allows_input((1, 2, 4)));
            send.send("tick 2 permitted").unwrap();
            worker_t.after_publication(42, &worker_log);
            send.send("tick 3 permitted").unwrap();
            // No SDK callback is needed for the next publication to be gated.
            worker_t.after_publication(42, &worker_log);
            send.send("released").unwrap();
        });
        assert_eq!(
            receive.recv_timeout(Duration::from_secs(1)).unwrap(),
            "SDK returned"
        );
        let deadline = Instant::now() + Duration::from_secs(1);
        while !t.state.lock().unwrap().boundary_seen {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(receive.recv_timeout(Duration::from_millis(10)).is_err());
        assert_eq!(t.before_view(100, 0, 1, &log), ViewMode::Bootstrap);
        t.after_view(ViewMode::Bootstrap, 1, 0, 1, &log);
        t.after_view(ViewMode::Paused, 0, 0, 1, &log);
        t.heartbeat(true, true, Keys::default(), &log);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Ready);
        assert!(receive.recv_timeout(Duration::from_millis(10)).is_err());
        t.heartbeat(
            true,
            true,
            Keys {
                start: true,
                ..Keys::default()
            },
            &log,
        );
        assert_eq!(
            receive.recv_timeout(Duration::from_secs(1)).unwrap(),
            "tick 2 permitted"
        );
        assert!(receive.recv_timeout(Duration::from_millis(10)).is_err());
        t.after_view(ViewMode::Running, 1, 0, 2, &log);
        assert_eq!(
            receive.recv_timeout(Duration::from_secs(1)).unwrap(),
            "tick 3 permitted"
        );
        assert!(receive.recv_timeout(Duration::from_millis(10)).is_err());
        t.cancel("test cancellation", &log);
        assert_eq!(
            receive.recv_timeout(Duration::from_secs(1)).unwrap(),
            "released"
        );
        worker.join().unwrap();
        assert!(!t.allows_input((1, 2, 3)));
    }

    #[test]
    fn premature_start_does_not_queue_and_running_continues_past_sixty_seconds() {
        let t = active();
        let log = logger("native-start");
        let start = Keys {
            start: true,
            ..Keys::default()
        };
        t.heartbeat(false, true, start, &log);
        {
            let mut s = t.state.lock().unwrap();
            s.boundary_seen = true;
            s.bootstrap_applied = true;
        }
        t.heartbeat(true, true, start, &log);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Loading);
        t.after_view(ViewMode::Paused, 0, 0, 0, &log);
        t.heartbeat(true, true, start, &log);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Ready);
        t.heartbeat(true, true, Keys::default(), &log);
        t.heartbeat(true, true, start, &log);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Running);
        t.state.lock().unwrap().running = Some(Instant::now() - Duration::from_secs(61));
        assert_eq!(t.before_view(100, 1, 0, &log), ViewMode::Running);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Running);
    }

    #[test]
    fn client_deadline_releases_even_when_hooks_never_arrive() {
        let t = active();
        let log = logger("native-client-deadline");
        assert!(t.describe().contains("Waiting for live worker hook"));
        assert!(!t.describe().contains("paused"));
        t.state.lock().unwrap().began = Instant::now() - Duration::from_secs(16);
        t.heartbeat(true, true, Keys::default(), &log);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Released);
        assert!(t.describe().contains("Control inactive"));
        assert!(!t.allows_input((1, 2, 3)));
    }

    #[test]
    fn short_client_stall_keeps_control_until_the_explicit_heartbeat_guard_releases() {
        let t = active();
        let log = logger("native-persistent-control");
        {
            let mut s = t.state.lock().unwrap();
            s.phase = Phase::Running;
            s.worker = Some(crate::platform_input::thread_id());
            s.battlefield = true;
            s.selected = true;
            s.running = Some(Instant::now());
            s.heartbeat = Some(Instant::now() - Duration::from_millis(500));
        }
        assert!(t.allows_input((1, 2, 3)));
        assert!(!t.allows_input((1, 2, 4)));
        {
            let mut s = t.state.lock().unwrap();
            s.heartbeat = Some(Instant::now() - Duration::from_secs(3));
            NativeTiming::check_limits(&mut s, &log);
            assert_eq!(s.phase, Phase::Released);
            assert_eq!(s.reason, "Client heartbeat missing for 2s");
        }
        assert!(!t.allows_input((1, 2, 3)));
    }

    #[test]
    fn entrances_record_rejections_before_any_control() {
        let t = NativeTiming::new(true);
        let log = logger("native-entrances");
        assert_eq!(t.hook_entry(true, &log), (false, true));
        t.installed(true, "test", &log);
        assert!(!t.hook_entry(false, &log).0);
        t.heartbeat(false, false, Keys::default(), &log);
        {
            let mut s = t.state.lock().unwrap();
            s.key = Some((1, 2, 3));
            s.phase = Phase::Loading;
        }
        assert!(t.hook_entry(false, &log).0);
        // This call is on the client, not the bound simulation worker.
        assert!(!t.hook_entry(true, &log).0);
        let s = t.state.lock().unwrap();
        assert_eq!(s.worker_calls, 2);
        assert_eq!(s.viewer_calls, 2);
        assert_eq!(s.produced, 0);
        assert_eq!(s.consumed, 0);
        drop(s);
        t.cancel("test", &log);
        assert!(!t.hook_entry(false, &log).0);
    }

    #[test]
    fn owed_playback_time_comes_back_gently_and_is_bounded() {
        // A normal remainder is left alone.
        assert_eq!(smooth(0.4, 0.), (0.4, 0., 0.));
        // 1.5 ticks left over: one whole tick is held, a quarter returns.
        let (clock, owed, dropped) = smooth(1.5, 0.);
        assert!((clock - 0.75).abs() < 1e-6 && (owed - 0.75).abs() < 1e-6 && dropped == 0.);
        // Owed time beyond the bound is dropped.
        let (clock, owed, dropped) = smooth(3.2, 1.0);
        assert!((dropped - 2.0).abs() < 1e-6);
        assert!((owed - (MAX_OWED - PAYBACK)).abs() < 1e-6 && (clock - 0.45).abs() < 1e-5);
        // Not a clock: left as it is.
        assert!(smooth(f32::NAN, 1.).0.is_nan());
        assert_eq!(smooth(-1., 1.), (-1., 1., 0.));
    }
    #[test]
    fn a_frame_rate_just_under_sixty_never_plays_two_ticks_in_a_row_once_caught_up() {
        // The viewer: the clock gains each frame's time, one tick is played
        // per tick of clock while the queue has one. 58 fps (1.034 ticks a
        // frame); the worker keeps `lead` ticks queued.
        let simulate = |lead: usize, frames: usize, mut clock: f32, mut owed: f32| {
            let mut played = Vec::new();
            for _ in 0..frames {
                let smoothed = smooth(clock, owed);
                clock = smoothed.0 + 60. / 58.;
                owed = smoothed.1;
                let n = (clock.floor() as usize).min(lead);
                clock -= n as f32;
                played.push(n);
            }
            (played, clock, owed)
        };
        // Ten seconds at a lead of 1: the owed time stays bounded.
        let (_, clock, owed) = simulate(1, 580, 0., 0.);
        assert!(owed <= MAX_OWED && clock < 2.);
        // The lead rises to 2: no two frames in a row play two ticks.
        let (played, _, owed) = simulate(2, 60, clock, owed);
        assert!(played.windows(2).all(|w| w != [2, 2]), "{played:?}");
        assert!(owed < 0.01);
    }
    #[test]
    fn frame_lead_follows_the_frame_rate_without_flipping_at_a_boundary() {
        // 60 fps and faster: one tick ahead, as before.
        assert_eq!(lead_for(1, 1.0), 1);
        assert_eq!(lead_for(1, 0.42), 1);
        // Up to 5% slow is tolerated; beyond it the lead rises.
        assert_eq!(lead_for(1, 1.04), 1);
        assert_eq!(lead_for(1, 1.33), 2); // 45 fps
        assert_eq!(lead_for(1, 2.0), 2); // 30 fps
        assert_eq!(lead_for(2, 2.05), 2);
        assert_eq!(lead_for(2, 3.0), 3); // 20 fps
        assert_eq!(lead_for(1, 9.0), MAX_LEAD);
        // Falls back only when the smaller lead is nearly enough.
        assert_eq!(lead_for(2, 1.04), 2);
        assert_eq!(lead_for(2, 1.01), 1);
        assert_eq!(lead_for(4, 1.0), 1);
    }
    #[test]
    fn a_slow_client_raises_the_lead_and_one_long_frame_or_a_pause_does_not() {
        let t = NativeTiming::running_test_worker((1, 2, 3));
        let log = logger("native-lead");
        let mut now = Instant::now();
        let mut frames = |n: usize, us: u64| {
            let mut lead = 0;
            for _ in 0..n {
                now += Duration::from_micros(us);
                lead = t.frame_at(now, 60, &log);
            }
            lead
        };
        assert_eq!(frames(30, 16_667), 1);
        // A single 100 ms hitch at 60 fps, then a two-second pause.
        assert_eq!(frames(1, 100_000), 1);
        assert_eq!(frames(1, 16_667), 1);
        assert_eq!(frames(1, 2_000_000), 1);
        assert_eq!(frames(30, 16_667), 1);
        assert_eq!(t.state.lock().unwrap().lead_changes, 0);
        // Steady 30 fps, then 60 fps again.
        assert_eq!(frames(60, 33_333), 2);
        assert_eq!(frames(120, 16_667), 1);
        assert_eq!(t.state.lock().unwrap().lead_changes, 2);
    }
    #[test]
    fn playback_rate_windows_restart_after_a_pause() {
        let t = NativeTiming::running_test_worker((1, 2, 3));
        let log = logger("native-rate");
        let mut now = Instant::now();
        t.frame_at(now, 60, &log);
        // Five seconds at one tick in every two frames: slow.
        for _ in 0..300 {
            now += Duration::from_micros(16_667);
            t.frame_at(now, 60, &log);
        }
        t.state.lock().unwrap().consumed += 150;
        // A pause: the window restarts and the slow stretch is not reported.
        now += Duration::from_secs(3);
        t.frame_at(now, 60, &log);
        assert_eq!(t.state.lock().unwrap().rate.unwrap().1, 150);
        for _ in 0..660 {
            now += Duration::from_micros(16_667);
            t.state.lock().unwrap().consumed += 1;
            t.frame_at(now, 60, &log);
        }
        assert_eq!(t.state.lock().unwrap().slow_reports, 0);
        // Half speed for a full window is reported.
        for i in 0..660 {
            now += Duration::from_micros(16_667);
            t.state.lock().unwrap().consumed += i % 2;
            t.frame_at(now, 60, &log);
        }
        assert_eq!(t.state.lock().unwrap().slow_reports, 1);
    }
    #[test]
    fn played_tick_change_without_queue_consumption_is_not_a_pause_ack() {
        let t = active();
        let log = logger("native-false-pause");
        t.after_view(ViewMode::Bootstrap, 1, 0, 1, &log);
        t.after_view(ViewMode::Paused, 0, 0, 2, &log);
        let s = t.state.lock().unwrap();
        assert_eq!(s.phase, Phase::Released);
        assert!(!s.pause_acknowledged);
    }

    // 1.5.1-dev1: the AI phase follows the game's speed field.

    /// A bound session in the AI phase, owned by this test thread as both
    /// client and worker.
    fn spectating() -> NativeTiming {
        let t = NativeTiming::running_test_worker((1, 2, 3));
        {
            let mut s = t.state.lock().unwrap();
            s.phase = Phase::Ai;
            s.client = Some(crate::platform_input::thread_id());
            s.view = Some(100);
            s.boundary_seen = true;
            s.bootstrap_applied = true;
        }
        t
    }
    /// The game's viewer as the stage-one design assumes it (guess): the
    /// frame clock gains the capped frame time times the speed field, which
    /// the override forces to 1 outside the AI phase, and one queued tick is
    /// played per whole tick of clock. The worker fills the queue first.
    struct Viewer {
        now: Instant,
        clock: f32,
        played: usize,
    }
    impl Viewer {
        fn new() -> Self {
            Self {
                now: Instant::now(),
                clock: 0.,
                played: 0,
            }
        }
        /// One frame of `us` microseconds; returns ticks played.
        fn frame(&mut self, t: &NativeTiming, us: u64, speed: f32, log: &Logger) -> usize {
            {
                let mut s = t.state.lock().unwrap();
                s.heartbeat = Some(Instant::now());
                while NativeTiming::may_produce(&mut s) {
                    s.produced += 1;
                }
            }
            t.spectator_fields(0, 0, speed.to_bits(), 0, log);
            let speed = if t.playback_plan().force_speed {
                1.
            } else {
                speed
            };
            self.now += Duration::from_micros(us);
            let lead = t.frame_at(self.now, 60, log);
            self.clock = t.smooth_clock(self.clock);
            let dt = (us as f32 / 1e6).min(lead.max(2) as f32 / 60.);
            self.clock += dt * 60. * speed;
            let queued = {
                let s = t.state.lock().unwrap();
                s.produced - s.consumed
            };
            let n = (self.clock.floor() as usize).min(queued);
            self.clock -= n as f32;
            self.played += n;
            t.after_view(ViewMode::Running, queued, queued - n, self.played, log);
            n
        }
    }
    fn take_control(t: &NativeTiming, log: &Logger) {
        t.request_action(true);
        let action = t.take_action().unwrap();
        assert_eq!(action, SessionAction::TakeControl);
        t.apply_action(action, log);
    }
    #[test]
    fn the_speed_field_becomes_a_multiplier_and_odd_values_count_as_one() {
        for speed in [0.5, 1., 1.5, 2., 3.] {
            assert_eq!(spectator_multiplier(speed), speed);
        }
        assert_eq!(spectator_multiplier(0.25), 0.25);
        assert_eq!(spectator_multiplier(4.), 4.);
        for odd in [0., 0.2, 4.5, -1., 100., f32::NAN, f32::INFINITY] {
            assert_eq!(spectator_multiplier(odd), 1., "{odd}");
        }
        // Raw bits that are not a speed at all (an unknown layout).
        assert_eq!(spectator_multiplier(f32::from_bits(0xffff_ffff)), 1.);
    }
    #[test]
    fn the_ai_lead_is_the_frame_lead_times_the_rounded_up_speed_capped() {
        assert_eq!(spectator_lead(1, 0.5), 1);
        assert_eq!(spectator_lead(1, 1.), 1);
        assert_eq!(spectator_lead(1, 1.5), 2);
        assert_eq!(spectator_lead(1, 2.), 2);
        assert_eq!(spectator_lead(1, 3.), 3);
        assert_eq!(spectator_lead(2, 3.), 6);
        assert_eq!(spectator_lead(4, 3.), MAX_SPECTATOR_LEAD);
        assert_eq!(spectator_lead(4, 4.), MAX_SPECTATOR_LEAD);
        assert_eq!(spectator_lead(4, f32::NAN), 4);
        // Only the AI phase multiplies.
        let t = spectating();
        let mut s = t.state.lock().unwrap();
        (s.lead, s.spectator) = (2, 3.);
        assert_eq!(NativeTiming::effective_lead(&s), 6);
        s.phase = Phase::Running;
        assert_eq!(NativeTiming::effective_lead(&s), 2);
    }
    #[test]
    fn only_the_ai_phase_leaves_the_speed_and_pause_fields_to_the_game() {
        let all = PlaybackPlan {
            force_speed: true,
            clear_pause: true,
        };
        for phase in [
            None,
            Some(Phase::Ready),
            Some(Phase::Running),
            Some(Phase::Paused),
            Some(Phase::Loading),
        ] {
            assert_eq!(playback_plan(phase), all);
        }
        // Sync and mode are forced in every phase (the hook writes them
        // unconditionally; `windows/tests.rs` checks the bytes).
        let ai = playback_plan(Some(Phase::Ai));
        assert!(!ai.force_speed && !ai.clear_pause);
    }
    #[test]
    fn ai_fields_set_the_multiplier_and_each_change_is_logged_within_a_budget() {
        let log = logger("spectator-fields");
        let t = spectating();
        t.spectator_fields(0, 0, 3f32.to_bits(), 0, &log);
        assert_eq!(t.state.lock().unwrap().spectator, 3.);
        // Unchanged fields write no line; each change writes one.
        t.spectator_fields(0, 0, 3f32.to_bits(), 0, &log);
        assert_eq!(t.state.lock().unwrap().field_lines, 1);
        t.spectator_fields(0, 0, 3f32.to_bits(), 1, &log);
        assert_eq!(t.state.lock().unwrap().field_lines, 2);
        t.spectator_fields(0, 0, f32::NAN.to_bits(), 1, &log);
        assert_eq!(t.state.lock().unwrap().spectator, 1.);
        for i in 0..400u32 {
            t.spectator_fields(0, i, 2f32.to_bits(), 0, &log);
        }
        assert_eq!(t.state.lock().unwrap().field_lines, FIELD_LINES);
        assert_eq!(t.state.lock().unwrap().spectator, 2.);
        // Outside the AI phase nothing is followed.
        t.state.lock().unwrap().phase = Phase::Running;
        t.spectator_fields(0, 0, 3f32.to_bits(), 0, &log);
        let s = t.state.lock().unwrap();
        assert_eq!((s.spectator, s.fields_seen), (1., None));
    }
    #[test]
    fn the_frame_guard_follows_the_ai_lead_and_never_drops_below_the_old_limit() {
        let log = logger("spectator-guard");
        let t = spectating();
        {
            let mut s = t.state.lock().unwrap();
            assert_eq!(NativeTiming::frame_limit(&s), MAX_LEAD);
            (s.lead, s.spectator) = (4, 3.);
            assert!(NativeTiming::may_produce(&mut s));
            assert_eq!(NativeTiming::frame_limit(&s), MAX_SPECTATOR_LEAD);
            s.produced = 12;
        }
        // A frame may play the whole queue the AI lead allowed...
        t.after_view(ViewMode::Running, 12, 0, 12, &log);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Ai);
        // ...but not more than could have been queued.
        t.state.lock().unwrap().produced = 26;
        t.after_view(ViewMode::Running, 14, 0, 26, &log);
        assert_eq!(t.state.lock().unwrap().phase, Phase::Released);
    }
    #[test]
    fn ai_playback_follows_each_speed_without_a_release() {
        let log = logger("spectator-speeds");
        for (us, frame_lead) in [(16_667, 1), (33_333, 2), (66_667, 4)] {
            for speed in [0.5, 1., 1.5, 2., 3.] {
                let t = spectating();
                let mut viewer = Viewer::new();
                // Let the frame lead settle at this frame rate.
                for _ in 0..120 {
                    viewer.frame(&t, us, speed, &log);
                }
                let before = viewer.played;
                for _ in 0..120 {
                    viewer.frame(&t, us, speed, &log);
                }
                let s = t.state.lock().unwrap();
                assert_eq!(s.phase, Phase::Ai, "{us} us at {speed}x");
                assert_eq!(s.lead, frame_lead, "{us} us at {speed}x");
                let per_frame = (viewer.played - before) as f32 / 120.;
                let expected = speed * us as f32 * 60. / 1e6;
                assert!(
                    (per_frame - expected).abs() <= 0.05 * expected,
                    "{us} us at {speed}x: {per_frame} ticks per frame, expected {expected}"
                );
            }
        }
    }
    #[test]
    fn f11_at_3x_restores_control_pacing_and_drains_without_a_release() {
        let log = logger("spectator-take-control");
        // 60 fps: AI lead 3; after F11 no frame plays more than two ticks.
        // 15 fps: AI lead 12; after F11 the queue drains at 1x.
        for (us, max_after) in [(16_667, 2), (66_667, MAX_LEAD + 1)] {
            let t = spectating();
            let mut viewer = Viewer::new();
            for _ in 0..200 {
                viewer.frame(&t, us, 3., &log);
            }
            let frame_lead = t.state.lock().unwrap().lead;
            let queued = {
                // The worker has filled the queue when F11 arrives.
                let mut s = t.state.lock().unwrap();
                while NativeTiming::may_produce(&mut s) {
                    s.produced += 1;
                }
                s.produced - s.consumed
            };
            assert_eq!(queued, spectator_lead(frame_lead, 3.), "{us} us");
            take_control(&t, &log);
            {
                // The next frame forces 1x and the worker's lead is the frame lead.
                let s = t.state.lock().unwrap();
                assert_eq!(s.phase, Phase::Running);
                assert_eq!(s.spectator, 1.);
                assert_eq!(NativeTiming::effective_lead(&s), frame_lead);
                assert!(s.drain.is_some());
            }
            assert!(t.playback_plan().force_speed);
            let mut most = 0;
            for _ in 0..120 {
                most = most.max(viewer.frame(&t, us, 3., &log));
                let s = t.state.lock().unwrap();
                assert_eq!(s.phase, Phase::Running, "{us} us");
            }
            assert!(most <= max_after, "{us} us: {most} ticks in one frame");
            let s = t.state.lock().unwrap();
            assert!(s.drain.is_none(), "{us} us: queue never drained");
            assert!(s.produced - s.consumed <= frame_lead);
            assert_eq!(NativeTiming::frame_limit(&s), MAX_LEAD);
        }
    }
    #[test]
    fn a_slower_speed_button_at_a_low_frame_rate_drains_without_a_release() {
        let log = logger("spectator-downshift");
        let t = spectating();
        let mut viewer = Viewer::new();
        for _ in 0..200 {
            viewer.frame(&t, 66_667, 3., &log);
        }
        for speed in [0.5, 2., 1., 3., 0.5] {
            for _ in 0..60 {
                viewer.frame(&t, 66_667, speed, &log);
                assert_eq!(t.state.lock().unwrap().phase, Phase::Ai, "{speed}x");
            }
        }
    }
    #[test]
    fn the_spectator_window_averages_ai_frames() {
        let log = logger("spectator-window");
        let t = spectating();
        let mut s = t.state.lock().unwrap();
        s.spectator = 2.;
        NativeTiming::spectator_window(&mut s, 2, 3, &log);
        NativeTiming::spectator_window(&mut s, 2, 1, &log);
        assert_eq!((s.window.frames, s.window.ticks, s.window.queue), (2, 4, 4));
        // Once the window is over, one line is written and it starts again.
        s.window.start = Some(Instant::now() - SPECTATOR_WINDOW);
        NativeTiming::spectator_window(&mut s, 2, 2, &log);
        assert_eq!(s.window.frames, 0);
        assert!(s.window.start.is_none());
    }
    // 1.5.1-dev2: the game's pause, and the playback-rate window.

    #[test]
    fn the_games_pause_keeps_the_multiplier_and_the_lead_and_logs_nothing() {
        let log = logger("spectator-pause");
        let t = spectating();
        t.spectator_fields(0, 0, 2f32.to_bits(), 1, &log);
        let (lead, lines) = {
            let s = t.state.lock().unwrap();
            (NativeTiming::effective_lead(&s), s.field_lines)
        };
        assert_eq!(lead, 2);
        // The pause button writes speed 0 (seen in game, 1.5.1-dev1), +0 or -0.
        for zero in [0f32, -0f32] {
            t.spectator_fields(0, 0, zero.to_bits(), 1, &log);
            let s = t.state.lock().unwrap();
            assert_eq!(s.spectator, 2.);
            assert_eq!(NativeTiming::effective_lead(&s), lead);
            assert_eq!(s.field_lines, lines);
        }
        // Resuming at the same speed is no change: no line either.
        t.spectator_fields(0, 0, 2f32.to_bits(), 1, &log);
        assert_eq!(t.state.lock().unwrap().field_lines, lines);
        // A different speed after the pause is a change.
        t.spectator_fields(0, 0, 3f32.to_bits(), 1, &log);
        let s = t.state.lock().unwrap();
        assert_eq!((s.spectator, s.field_lines), (3., lines + 1));
    }
    #[test]
    fn pause_and_resume_at_2x_play_on_without_a_release() {
        let log = logger("spectator-pause-play");
        let t = spectating();
        let mut viewer = Viewer::new();
        for speed in [2., 0., 2.] {
            let before = viewer.played;
            for _ in 0..120 {
                viewer.frame(&t, 16_667, speed, &log);
                assert_eq!(t.state.lock().unwrap().phase, Phase::Ai, "{speed}x");
            }
            let played = viewer.played - before;
            if speed == 0. {
                // The test viewer's model of the game (a guess): at most
                // what its clock owed from before the pause still plays.
                assert!(played <= 2, "{played} ticks while paused");
            } else {
                assert!(played >= 230, "{played} ticks in 2 s at 2x");
            }
            assert_eq!(t.state.lock().unwrap().spectator, 2.);
        }
    }
    /// Runs the viewer's frame timing at 60 fps for `secs` seconds while the
    /// game plays `ticks_per_frame` ticks a frame, switching the speed field
    /// to the `(frame, speed)` values on the way. Returns slow reports.
    fn rate_windows(
        start: f32,
        ticks_per_frame: &[(usize, f32)],
        switches: &[(usize, f32)],
    ) -> u32 {
        let log = logger("spectator-rate");
        let t = spectating();
        t.spectator_fields(0, 0, start.to_bits(), 0, &log);
        let mut now = Instant::now();
        let mut owed = 0f32;
        let mut per_frame = ticks_per_frame[0].1;
        for frame in 0..=1200 {
            if let Some(&(_, speed)) = switches.iter().find(|(f, _)| *f == frame) {
                t.spectator_fields(0, 0, speed.to_bits(), 0, &log);
            }
            if let Some(&(_, n)) = ticks_per_frame.iter().find(|(f, _)| *f == frame) {
                per_frame = n;
            }
            owed += per_frame;
            let whole = owed.floor();
            owed -= whole;
            {
                let mut s = t.state.lock().unwrap();
                s.consumed += whole as usize;
                s.produced = s.consumed;
            }
            t.frame_at(now, 60, &log);
            now += Duration::from_micros(16_667);
        }
        let reports = t.state.lock().unwrap().slow_reports;
        reports
    }
    #[test]
    fn a_rate_window_in_which_the_speed_changed_is_not_judged_slow() {
        // The stage-one artifact: 0.5x for half a window, then 3x, judged
        // against 3x alone. Two windows of 600 frames; the switch is in the
        // first, the second is steady at 3x.
        assert_eq!(rate_windows(0.5, &[(0, 0.5), (300, 3.)], &[(300, 3.)]), 0);
        // A pause in the window (the game plays nothing) is skipped too.
        let paused = rate_windows(
            2.,
            &[(0, 2.), (100, 0.), (400, 2.)],
            &[(100, 0.), (400, 2.)],
        );
        assert_eq!(paused, 0);
        // F11 changes the expected rate as well.
        let log = logger("spectator-rate-f11");
        let t = spectating();
        t.spectator_fields(0, 0, 3f32.to_bits(), 0, &log);
        t.state.lock().unwrap().rate_unsteady = false;
        take_control(&t, &log);
        assert!(t.state.lock().unwrap().rate_unsteady);
    }
    #[test]
    fn a_steady_window_that_plays_slow_is_still_reported() {
        // 3x expected, 2 ticks a frame played: both windows are slow.
        assert_eq!(rate_windows(3., &[(0, 2.)], &[]), 2);
        // A skipped window does not hide the steady slow one after it.
        assert_eq!(rate_windows(0.5, &[(0, 0.5), (300, 2.)], &[(300, 3.)]), 1);
        // At the selected speed nothing is reported.
        for speed in [0.5, 1., 1.5, 2., 3.] {
            assert_eq!(rate_windows(speed, &[(0, speed)], &[]), 0, "{speed}x");
        }
    }
}
