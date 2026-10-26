//! Tests of the hidden diagnosis switch `perf_pacing_off` (1.6.1-dev2).
use super::*;
use crate::test_support::logger;

/// A match bound with the switch as `on`, Loading, this thread being both
/// the worker and the client.
fn bound(on: bool, log: &Logger) -> NativeTiming {
    let t = NativeTiming::new(true);
    t.state.lock().unwrap().installed = true;
    t.configure_pacing_off(on, log);
    bind(&t, (5, 6, 7), log);
    t
}
fn bind(t: &NativeTiming, key: MatchKey, log: &Logger) {
    let me = crate::platform_input::thread_id();
    // The bind refuses a first tick on the client thread.
    t.state.lock().unwrap().client = Some(me.wrapping_add(1));
    t.observe_start(key, 1, false, false, log);
    let mut s = t.state.lock().unwrap();
    assert_eq!(s.phase, Phase::Loading);
    s.client = Some(me);
    s.battlefield = true;
    s.selected = true;
    s.heartbeat = Some(Instant::now());
}
#[test]
fn the_pacing_switch_is_read_each_frame_and_applies_from_the_next_match() {
    let log = logger("pacing-off-switch");
    let t = bound(false, &log);
    assert!(!t.pacing_off());
    // Read every frame; only a change is reported.
    assert!(t.configure_pacing_off(true, &log));
    assert!(!t.configure_pacing_off(true, &log));
    // Mid-match the bound match keeps its pacing and its lead.
    assert!(!t.pacing_off());
    assert_eq!(t.state.lock().unwrap().lead, 1);
    assert!(!t.untouched(ViewMode::Running));
    // The next match is bound with it.
    {
        let mut s = t.state.lock().unwrap();
        s.battlefield = false;
    }
    t.rearm(true, Keys::default(), &log);
    assert!(!t.pacing_off());
    bind(&t, (8, 9, 10), &log);
    assert!(t.pacing_off());
    assert_eq!(t.state.lock().unwrap().lead, PACING_OFF_LEAD);
    // Turning it off mid-match changes nothing until the next bind.
    assert!(t.configure_pacing_off(false, &log));
    assert!(t.pacing_off());
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-pacing-off-switch.log"),
    )
    .unwrap();
    assert_eq!(text.matches("PERF perf_pacing_off=true").count(), 1);
    assert_eq!(text.matches("PERF perf_pacing_off=false").count(), 1);
    assert_eq!(text.matches("TIMING pacing off for this match").count(), 1);
}
#[test]
fn with_pacing_off_running_frames_are_untouched_and_holds_and_control_stay() {
    let log = logger("pacing-off-session");
    let t = bound(true, &log);
    t.state.lock().unwrap().produced = 1;
    t.state.lock().unwrap().boundary_seen = true;
    // The start hold is the usual one: one initial frame, then held.
    assert_eq!(t.before_view(100, 0, 1, &log), ViewMode::Bootstrap);
    assert!(!t.untouched(ViewMode::Bootstrap));
    t.after_view(ViewMode::Bootstrap, 1, 0, 1, &log);
    assert_eq!(t.before_view(100, 1, 0, &log), ViewMode::Paused);
    assert!(!t.untouched(ViewMode::Paused));
    t.after_view(ViewMode::Paused, 0, 0, 1, &log);
    t.heartbeat_with_start(true, true, Keys::default(), false, &log);
    assert_eq!(t.ui_phase(), Some(Phase::Ready));
    let start = Keys {
        start: true,
        ..Keys::default()
    };
    t.heartbeat_with_start(true, true, start, false, &log);
    t.heartbeat_with_start(true, true, Keys::default(), false, &log);
    assert_eq!(t.ui_phase(), Some(Phase::Running));
    // Running: untouched. The game's own update may play the whole
    // queue (up to the fixed lead) in one frame without a release.
    let mut played = 1;
    for _ in 0..20 {
        assert_eq!(t.before_view(100, played, 2, &log), ViewMode::Running);
        assert!(t.untouched(ViewMode::Running));
        t.state.lock().unwrap().produced += 2;
        played += 2;
        t.after_view(ViewMode::Running, 2, 0, played, &log);
        t.heartbeat_with_start(true, true, Keys::default(), false, &log);
    }
    assert_eq!(t.ui_phase(), Some(Phase::Running));
    assert!(t.client_controls(None));
    assert!(t.allows_input((5, 6, 7)));
    // Pause holds the viewer as always (not untouched) without a release.
    t.apply_action(SessionAction::Pause, &log);
    assert_eq!(t.before_view(100, played, 0, &log), ViewMode::Paused);
    assert!(!t.untouched(ViewMode::Paused));
    t.after_view(ViewMode::Paused, 0, 0, played, &log);
    assert_eq!(t.ui_phase(), Some(Phase::Paused));
    t.apply_action(SessionAction::Resume, &log);
    assert_eq!(t.before_view(100, played, 0, &log), ViewMode::Running);
    assert!(t.untouched(ViewMode::Running));
    // AI control is running too: untouched as well.
    t.apply_action(SessionAction::ReturnAi, &log);
    assert_eq!(t.before_view(100, played, 0, &log), ViewMode::Running);
    assert!(t.untouched(ViewMode::Running));
    assert_ne!(t.state.lock().unwrap().phase, Phase::Released);
    // A paced match never goes untouched.
    let paced = bound(false, &log);
    paced.state.lock().unwrap().phase = Phase::Running;
    assert_eq!(paced.before_view(100, 0, 0, &log), ViewMode::Running);
    assert!(!paced.untouched(ViewMode::Running));
}
#[test]
fn with_pacing_off_the_worker_keeps_the_fixed_lead_and_the_frame_time_only_measures() {
    use std::sync::{mpsc, Arc};
    let log = Arc::new(logger("pacing-off-worker"));
    let t = Arc::new(bound(true, &log));
    {
        let mut s = t.state.lock().unwrap();
        s.phase = Phase::Running;
        s.boundary_seen = true;
        s.bootstrap_applied = true;
    }
    let (send, receive) = mpsc::channel();
    let (worker_t, worker_log) = (t.clone(), log.clone());
    let worker = std::thread::spawn(move || {
        worker_t.state.lock().unwrap().worker = Some(crate::platform_input::thread_id());
        // Queue 1 of 2: no wait.
        worker_t.after_publication(42, &worker_log);
        send.send(1).unwrap();
        // Queue 2 of 2: waits until a tick is played.
        worker_t.after_publication(42, &worker_log);
        send.send(2).unwrap();
    });
    assert_eq!(receive.recv_timeout(Duration::from_secs(1)).unwrap(), 1);
    assert!(receive.recv_timeout(Duration::from_millis(60)).is_err());
    t.heartbeat_with_start(true, true, Keys::default(), false, &log);
    t.after_view(ViewMode::Running, 2, 1, 2, &log);
    assert_eq!(receive.recv_timeout(Duration::from_secs(1)).unwrap(), 2);
    worker.join().unwrap();
    // 30 fps for a long while: the lead stays fixed, nothing is owed,
    // and the PACING line carries the owed time and the frame average.
    let mut now = Instant::now();
    for _ in 0..700 {
        now += Duration::from_micros(33_333);
        assert_eq!(t.frame_at(now, 60, &log), PACING_OFF_LEAD);
    }
    let s = t.state.lock().unwrap();
    assert_eq!((s.lead, s.lead_changes, s.owed), (PACING_OFF_LEAD, 0, 0.));
    assert!(s.lead_trace.is_none());
    drop(s);
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-pacing-off-worker.log"),
    )
    .unwrap();
    let line = text
        .lines()
        .find(|l| l.contains("playback "))
        .expect("a playback line");
    assert!(
        line.contains("average client frame 33.3 ms owed=0.00 ticks"),
        "{line}"
    );
    assert!(line.contains("pacing off (perf_pacing_off)"), "{line}");
    assert!(!text.contains("TIMING frame lead"));
}
