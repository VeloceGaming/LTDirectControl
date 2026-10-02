//! Bounded diagnostic pacing policy. No timers, threads, or game callbacks here.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    Finished,
    DisplayThread,
    MissingHeartbeat,
    TickRewound,
    StartupTimeout,
    LeftBattlefield,
    LateStart,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaceDecision {
    Proceed,
    WaitMicros(u64),
    Stop(StopReason),
}

#[derive(Clone, Copy)]
pub struct TimingPolicy {
    pub ticks_per_second: u64,
    pub max_duration_micros: u64,
    pub max_heartbeat_age_micros: u64,
}

impl TimingPolicy {
    /// Generate one simulation second before holding the worker. The tick-3
    /// barrier prevented battlefield loading in the recorded 0.6.2 test.
    /// This bootstrap is a runtime experiment, not a proven publication boundary.
    pub fn startup_barrier(
        self,
        tick: usize,
        elapsed_micros: u64,
        heartbeat_known: bool,
        is_display_thread: bool,
        grace_micros: u64,
    ) -> PaceDecision {
        if is_display_thread {
            return PaceDecision::Stop(StopReason::DisplayThread);
        }
        if !heartbeat_known {
            return PaceDecision::Stop(StopReason::MissingHeartbeat);
        }
        if elapsed_micros >= grace_micros {
            return PaceDecision::Stop(StopReason::StartupTimeout);
        }
        if tick as u128 <= u128::from(self.ticks_per_second) {
            PaceDecision::Proceed
        } else {
            PaceDecision::WaitMicros(2_000)
        }
    }

    /// Startup can load assets without calling the display extension. This
    /// grace is bounded separately and ends as soon as the battlefield appears.
    #[allow(clippy::too_many_arguments)]
    pub fn decide_starting(
        self,
        start_tick: usize,
        tick: usize,
        elapsed_micros: u64,
        heartbeat_age_micros: Option<u64>,
        is_display_thread: bool,
        battlefield_seen: bool,
        startup_grace_micros: u64,
    ) -> PaceDecision {
        if is_display_thread {
            return PaceDecision::Stop(StopReason::DisplayThread);
        }
        if !battlefield_seen && elapsed_micros >= startup_grace_micros {
            return PaceDecision::Stop(StopReason::StartupTimeout);
        }
        let policy = if battlefield_seen {
            self
        } else {
            Self {
                // The loading deadline starts at worker entry, independently
                // of how old the last pre-match callback already was.
                max_heartbeat_age_micros: u64::MAX,
                ..self
            }
        };
        policy.decide(
            start_tick,
            tick,
            elapsed_micros,
            heartbeat_age_micros,
            false,
        )
    }

    pub fn decide(
        self,
        start_tick: usize,
        tick: usize,
        elapsed_micros: u64,
        heartbeat_age_micros: Option<u64>,
        is_display_thread: bool,
    ) -> PaceDecision {
        if is_display_thread {
            return PaceDecision::Stop(StopReason::DisplayThread);
        }
        if heartbeat_age_micros.is_none_or(|age| age > self.max_heartbeat_age_micros) {
            return PaceDecision::Stop(StopReason::MissingHeartbeat);
        }
        let Some(delta) = tick.checked_sub(start_tick) else {
            return PaceDecision::Stop(StopReason::TickRewound);
        };
        if elapsed_micros >= self.max_duration_micros || self.ticks_per_second == 0 {
            return PaceDecision::Stop(StopReason::Finished);
        }
        let due = (delta as u128 * 1_000_000 / u128::from(self.ticks_per_second))
            .min(u128::from(self.max_duration_micros)) as u64;
        if due > elapsed_micros {
            // Recheck heartbeat and budget at least every 2 ms.
            PaceDecision::WaitMicros((due - elapsed_micros).min(2_000))
        } else {
            PaceDecision::Proceed
        }
    }
}
