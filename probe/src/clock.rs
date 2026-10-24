//! The client thread's clock for the mod's windows (1.6.0-perf).
//!
//! In a build for the game `now()` is `Instant::now()` and `since(at)` is
//! `at.elapsed()`: nothing changes. Tests can stop the clock on their own
//! thread and move it by hand (`test_clock`), so a sequence of frames
//! through a window gives the same writes on every run.
use std::time::{Duration, Instant};

#[cfg(not(test))]
#[inline(always)]
pub fn now() -> Instant {
    Instant::now()
}
#[cfg(test)]
pub fn now() -> Instant {
    test_clock::FAKE
        .with(|f| f.get())
        .unwrap_or_else(Instant::now)
}
/// Time since `at` (`at.elapsed()` with this clock); zero for a time ahead.
#[inline(always)]
pub fn since(at: impl std::borrow::Borrow<Instant>) -> Duration {
    now().saturating_duration_since(*at.borrow())
}

#[cfg(test)]
pub mod test_clock {
    use std::cell::Cell;
    use std::time::{Duration, Instant};
    thread_local! {
        pub(super) static FAKE: Cell<Option<Instant>> = const { Cell::new(None) };
    }
    /// Stop this thread's clock at an arbitrary fixed point.
    pub fn stop() {
        FAKE.with(|f| f.set(Some(Instant::now())));
    }
    /// Move this thread's stopped clock forward.
    pub fn advance(by: Duration) {
        FAKE.with(|f| f.set(f.get().map(|t| t + by)));
    }
    /// The real clock again.
    pub fn release() {
        FAKE.with(|f| f.set(None));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_stopped_clock_moves_only_by_hand() {
        test_clock::stop();
        let a = now();
        test_clock::advance(Duration::from_millis(16));
        assert_eq!(since(a), Duration::from_millis(16));
        assert_eq!(since(a + Duration::from_secs(1)), Duration::ZERO);
        test_clock::release();
    }
}
