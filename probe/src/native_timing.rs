//! No waiting from an SDK callback. The only wait is between published frames.
use crate::{platform_input::Keys, Logger};
use std::collections::{BTreeSet, VecDeque};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

pub type MatchKey = (u64, u64, u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Armed,
    Loading,
    Ready,
    Running,
    Paused,
    Released,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionAction {
    Start,
    Pause,
    Resume,
    ReturnAi,
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
    key: Option<MatchKey>,
    worker: Option<u64>,
    sender: Option<usize>,
    view: Option<usize>,
    began: Instant,
    running: Option<Instant>,
    produced: usize,
    consumed: usize,
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
                key: None,
                worker: None,
                sender: None,
                view: None,
                began: Instant::now(),
                running: None,
                produced: 0,
                consumed: 0,
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
            }),
        }
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
        let Ok(mut s) = self.state.lock() else { return };
        self.wake.notify_all();
        let was_battlefield = s.battlefield;
        s.client = Some(crate::platform_input::thread_id());
        s.heartbeat = Some(Instant::now());
        s.battlefield = battlefield;
        s.selected = selected;
        // The deadline must work even if neither native hook ever arrives.
        Self::check_limits(&mut s, log);
        if keys.release {
            Self::release(&mut s, "F12", log);
        }
        if was_battlefield && !battlefield && s.key.is_some() {
            Self::release(&mut s, "Left battlefield", log);
        }
        if s.phase == Phase::Loading
            && s.boundary_seen
            && s.bootstrap_applied
            && s.pause_acknowledged
            && battlefield
            && selected
        {
            s.phase = Phase::Ready;
            log.write(&format!(
                "NATIVE READY produced={} consumed={} played_tick={} load_ms={}",
                s.produced,
                s.consumed,
                s.played_tick,
                s.began.elapsed().as_millis()
            ));
        }
        if keys.start && !s.previous_start && matches!(s.phase, Phase::Ready | Phase::Paused) {
            s.phase = Phase::Running;
            s.running.get_or_insert_with(Instant::now);
            log.write("NATIVE START/RESUME backup F11; playback=1x maximum frame lead=1");
        }
        s.previous_start = keys.start;
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
    pub fn match_key(&self) -> Option<MatchKey> {
        self.state.lock().ok()?.key
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
        s.sender = None;
        s.view = None;
        s.selected = false;
        s.previous_start = keys.start;
        s.began = Instant::now();
        s.running = None;
        s.produced = 0;
        s.consumed = 0;
        s.played_tick = 0;
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
            SessionAction::ReturnAi
        } else {
            match s.phase {
                Phase::Ready => SessionAction::Start,
                Phase::Running => SessionAction::Pause,
                Phase::Paused => SessionAction::Resume,
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
            (SessionAction::ReturnAi, _) => Self::release(&mut s, "Return to AI button", log),
            (SessionAction::Start, Phase::Ready) | (SessionAction::Resume, Phase::Paused) => {
                s.phase = Phase::Running;
                s.running.get_or_insert_with(Instant::now);
                log.write(&format!(
                    "SESSION {action:?}; full-match pacing=1x frame_lead=1"
                ));
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
        s.phase = Phase::Loading;
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
                log.write(
                    "NATIVE PUBLISHED first initialization frame; holding before next run_tick_ext",
                );
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
            if keys.release {
                Self::release(&mut s, "F12 at worker boundary", log);
            }
            Self::check_limits(&mut s, log);
            let wait = match s.phase {
                Phase::Released | Phase::Armed => false,
                Phase::Loading | Phase::Ready | Phase::Paused => true,
                Phase::Running => s.produced.saturating_sub(s.consumed) >= 1,
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
            if waited.is_err() {
                return;
            }
        }
    }
    fn check_limits(s: &mut State, log: &Logger) {
        match s.phase {
            Phase::Loading if s.began.elapsed() >= Duration::from_secs(15) => {
                Self::release(s, "Startup interception not confirmed within 15s", log)
            }
            Phase::Ready | Phase::Running | Phase::Paused
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
        if s.phase == Phase::Running {
            ViewMode::Running
        } else if !s.bootstrap_applied && queued > 0 {
            ViewMode::Bootstrap
        } else {
            ViewMode::Paused
        }
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
        let previous_played = s.played_tick;
        s.played_tick = played;
        let used = before.saturating_sub(after);
        s.consumed += used;
        let consumed = s.consumed;
        while s
            .traces
            .front()
            .is_some_and(|t| t.published && t.frame <= consumed)
        {
            let trace = s.traces.pop_front().unwrap();
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
        if used > 2 || s.consumed > s.produced + 1 {
            Self::release(&mut s, "Unexpected publication/playback frame counts", log);
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
            || s.trace_count >= 300
        {
            return;
        }
        s.trace_count += 1;
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
            .is_ok_and(|s| Self::client_owned(&s, view))
    }
    fn client_owned(s: &State, view: Option<usize>) -> bool {
        matches!(s.phase, Phase::Ready | Phase::Running | Phase::Paused)
            && s.battlefield
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
        assert_eq!(t.state.lock().unwrap().reason, "Return to AI button");
        assert_eq!(t.before_view(100, 1, 2, &log), ViewMode::Native);
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
        assert_eq!(t.state.lock().unwrap().phase, Phase::Released);
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
    fn played_tick_change_without_queue_consumption_is_not_a_pause_ack() {
        let t = active();
        let log = logger("native-false-pause");
        t.after_view(ViewMode::Bootstrap, 1, 0, 1, &log);
        t.after_view(ViewMode::Paused, 0, 0, 2, &log);
        let s = t.state.lock().unwrap();
        assert_eq!(s.phase, Phase::Released);
        assert!(!s.pause_acknowledged);
    }
}
