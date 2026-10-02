use lt_direct_control_core::pacing::*;

fn policy() -> TimingPolicy {
    TimingPolicy {
        ticks_per_second: 60,
        max_duration_micros: 20_000_000,
        max_heartbeat_age_micros: 250_000,
    }
}

#[test]
fn display_thread_is_never_delayed() {
    assert_eq!(
        policy().decide(1, 2, 0, Some(0), true),
        PaceDecision::Stop(StopReason::DisplayThread)
    );
}

#[test]
fn failed_or_stalled_client_opens_the_gate() {
    for heartbeat in [None, Some(250_001)] {
        assert_eq!(
            policy().decide(1, 2, 0, heartbeat, false),
            PaceDecision::Stop(StopReason::MissingHeartbeat)
        );
    }
}

#[test]
fn pacing_is_bounded_and_rechecks_often() {
    assert_eq!(
        policy().decide(1, 601, 0, Some(0), false),
        PaceDecision::WaitMicros(2_000)
    );
    assert_eq!(
        policy().decide(1, 1201, 20_000_000, Some(0), false),
        PaceDecision::Stop(StopReason::Finished)
    );
}

#[test]
fn native_computation_time_counts_toward_tick_budget() {
    assert_eq!(
        policy().decide(1, 61, 1_000_000, Some(0), false),
        PaceDecision::Proceed
    );
    assert_eq!(
        policy().decide(1, 61, 999_000, Some(0), false),
        PaceDecision::WaitMicros(1_000)
    );
}

#[test]
fn rewind_cannot_accidentally_create_a_large_wait() {
    assert_eq!(
        policy().decide(100, 1, 0, Some(0), false),
        PaceDecision::Stop(StopReason::TickRewound)
    );
}

#[test]
fn startup_loading_gap_has_a_separate_finite_budget() {
    assert_eq!(
        policy().decide_starting(1, 61, 900_000, Some(900_000), false, false, 2_000_000),
        PaceDecision::WaitMicros(2_000)
    );
    // Even fresh callbacks outside the battlefield cannot extend startup.
    assert_eq!(
        policy().decide_starting(1, 121, 2_000_000, Some(0), false, false, 2_000_000),
        PaceDecision::Stop(StopReason::StartupTimeout)
    );
}

#[test]
fn battlefield_readiness_immediately_restores_strict_heartbeat_limit() {
    assert_eq!(
        policy().decide_starting(1, 61, 900_000, Some(250_001), false, true, 2_000_000),
        PaceDecision::Stop(StopReason::MissingHeartbeat)
    );
    assert_eq!(
        policy().decide_starting(1, 61, 900_000, None, false, false, 2_000_000),
        PaceDecision::Stop(StopReason::MissingHeartbeat)
    );
    assert_eq!(
        policy().decide_starting(1, 61, 0, Some(0), true, false, 2_000_000),
        PaceDecision::Stop(StopReason::DisplayThread)
    );
}

#[test]
fn startup_deadline_is_not_shortened_by_a_preexisting_heartbeat_gap() {
    assert_eq!(
        policy().decide_starting(1, 160, 2_600_000, Some(2_623_000), false, false, 5_000_000),
        PaceDecision::WaitMicros(2_000)
    );
    assert_eq!(
        policy().decide_starting(1, 310, 5_000_000, Some(23_000), false, false, 5_000_000),
        PaceDecision::Stop(StopReason::StartupTimeout)
    );
}

#[test]
fn startup_barrier_does_not_spend_the_running_match_budget() {
    assert_eq!(
        policy().startup_barrier(60, 3_000_000, true, false, 5_000_000),
        PaceDecision::Proceed
    );
    assert_eq!(
        policy().startup_barrier(61, 3_000_000, true, false, 5_000_000),
        PaceDecision::WaitMicros(2_000)
    );
    assert_eq!(
        policy().startup_barrier(61, 5_000_000, true, false, 5_000_000),
        PaceDecision::Stop(StopReason::StartupTimeout)
    );
    assert_eq!(
        policy().startup_barrier(61, 0, false, false, 5_000_000),
        PaceDecision::Stop(StopReason::MissingHeartbeat)
    );
    assert_eq!(
        policy().startup_barrier(61, 0, true, true, 5_000_000),
        PaceDecision::Stop(StopReason::DisplayThread)
    );
}
