//! "Show input delay" (Advanced > Debug, Off by default): the measured time
//! from the mod reading one of your commands (a click or a key) to the client
//! frame that plays the simulation tick carrying it, averaged over your last
//! commands. Not included: Windows' handling before the mod reads the input,
//! and rendering and the display after the frame. Measured by the existing
//! command traces (`native_timing`, `input_trace`); display only.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Commands averaged.
const SAMPLES: usize = 20;
/// Older measurements are not shown.
const FRESH: Duration = Duration::from_secs(30);

static SHOWN: AtomicBool = AtomicBool::new(false);
static RECENT: Mutex<VecDeque<(Instant, Duration)>> = Mutex::new(VecDeque::new());

pub fn set_shown(on: bool) {
    SHOWN.store(on, Ordering::Relaxed);
}
pub fn shown() -> bool {
    SHOWN.load(Ordering::Relaxed)
}

/// Viewer thread: a command's tick was played `delay` after it was read.
pub fn record(delay: Duration) {
    if !shown() {
        return;
    }
    if let Ok(mut recent) = RECENT.lock() {
        recent.push_back((Instant::now(), delay));
        while recent.len() > SAMPLES {
            recent.pop_front();
        }
    }
}

/// The number drawn top right while the setting is on, like a ping
/// display: `19ms`, or `--ms` before a fresh measurement.
pub fn text() -> String {
    let recent = RECENT.lock().map(|r| r.clone()).unwrap_or_default();
    line(&recent, Instant::now())
}

fn line(recent: &VecDeque<(Instant, Duration)>, now: Instant) -> String {
    let fresh: Vec<Duration> = recent
        .iter()
        .filter(|(at, _)| now.saturating_duration_since(*at) <= FRESH)
        .map(|(_, d)| *d)
        .collect();
    if fresh.is_empty() {
        return "--ms".to_owned();
    }
    let average = fresh.iter().sum::<Duration>() / fresh.len() as u32;
    format!("{}ms", average.as_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_averages_recent_commands_and_ignores_old_ones() {
        let now = Instant::now() + Duration::from_secs(60);
        let mut recent = VecDeque::new();
        assert_eq!(line(&recent, now), "--ms");
        recent.push_back((now - Duration::from_secs(45), Duration::from_millis(500)));
        assert_eq!(line(&recent, now), "--ms");
        recent.push_back((now - Duration::from_secs(2), Duration::from_millis(18)));
        recent.push_back((now - Duration::from_secs(1), Duration::from_millis(22)));
        assert_eq!(line(&recent, now), "20ms");
    }
}
