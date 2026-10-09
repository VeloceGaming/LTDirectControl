//! Cheap aggregate timing (0.65 performance check): per-section totals and
//! worst values, a client frame-interval histogram, lock and pacing waits and
//! native hook traffic. One summary line per window plus a few slow-frame
//! lines; nothing here changes behaviour.
//!
//! Hook counts are batched per thread so simulation threads never share a
//! hot counter (that contention is what the 0.65 fast paths remove).
use crate::Logger;
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::OnceLock;
use std::time::Instant;

#[derive(Clone, Copy)]
pub enum Section {
    // The mod's own client callbacks: these three are the mod total.
    Pre,
    Post,
    Render,
    // Breakdown inside post_update (already part of Post).
    Info,
    Hud,
    Team,
    Session,
    Settings,
    Shop,
    Effect,
    Audit,
    Hover,
    Cursor,
    // Native work around our hooks; not mod cost.
    Viewer,
    WorkerSend,
    WorkerAfterSend,
}
const SECTIONS: usize = 16;
const MOD_TOTAL: usize = 3;
const BREAKDOWN_END: usize = 13;
const SECTION_NAMES: [&str; SECTIONS] = [
    "pre_update",
    "post_update",
    "post_render",
    "info",
    "hud",
    "tab",
    "session",
    "settings",
    "shop",
    "effect",
    "audit",
    "hover",
    "cursor",
    "viewer_incl_native",
    "worker_send_native",
    "worker_after_send_incl_pacing",
];
#[derive(Clone, Copy)]
pub enum Wait {
    Logger,
    Abilities,
    Shop,
    /// The worker deliberately waiting for the viewer (one-frame lead).
    Pacing,
}
const WAITS: usize = 4;
const WAIT_NAMES: [&str; WAITS] = ["logger", "abilities", "shop", "worker_pacing"];
#[derive(Clone, Copy)]
pub enum Hook {
    Steer,
    OwnerFast,
    ShopLocked,
    ShopFast,
    Move,
    Attack,
    Skill,
    Aim,
    Outline,
}
const HOOKS: usize = 9;
const HOOK_NAMES: [&str; HOOKS] = [
    "steer",
    "owner_fast_reject",
    "shop_locked",
    "shop_fast_reject",
    "move",
    "attack",
    "skill",
    "aim",
    "outline",
];

struct Stat {
    total: AtomicU64,
    max: AtomicU64,
    count: AtomicU64,
}
impl Stat {
    const fn new() -> Self {
        Self {
            total: AtomicU64::new(0),
            max: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }
    fn add(&self, micros: u64) {
        self.total.fetch_add(micros, Relaxed);
        self.max.fetch_max(micros, Relaxed);
        self.count.fetch_add(1, Relaxed);
    }
    fn take(&self) -> (u64, u64, u64) {
        (
            self.total.swap(0, Relaxed),
            self.max.swap(0, Relaxed),
            self.count.swap(0, Relaxed),
        )
    }
}

static SECTION_STATS: [Stat; SECTIONS] = [const { Stat::new() }; SECTIONS];
/// Time per section since the last client frame (slow-frame lines).
static FRAME: [AtomicU64; SECTIONS] = [const { AtomicU64::new(0) }; SECTIONS];
static WAIT_STATS: [Stat; WAITS] = [const { Stat::new() }; WAITS];
/// [hook][0 = the viewed match's worker thread, 1 = any other thread].
static HOOK_COUNTS: [[AtomicU64; 2]; HOOKS] = [const { [const { AtomicU64::new(0) }; 2] }; HOOKS];
static WORKER_THREAD: AtomicU64 = AtomicU64::new(0);
/// Client frame intervals in whole milliseconds; the last bucket is 100+.
static INTERVALS: [AtomicU64; 101] = [const { AtomicU64::new(0) }; 101];
static INTERVAL_MAX: AtomicU64 = AtomicU64::new(0);
static LAST_FRAME: AtomicU64 = AtomicU64::new(0);
static LAST_REPORT: AtomicU64 = AtomicU64::new(0);
static SLOW_LINES: AtomicU64 = AtomicU64::new(0);
static LOG_BYTES: AtomicU64 = AtomicU64::new(0);
const WINDOW_MICROS: u64 = 10_000_000;
const SLOW_MICROS: u64 = 25_000;
const SLOW_LINES_PER_WINDOW: u64 = 8;
const FLUSH_EVERY: u64 = 1024;

// Opt-in timings are separate from client callbacks: overlapping/nested work
// must never be added to mod_total_ms. Histograms have fixed storage, no event
// queue, and report conservative power-of-two upper bounds in microseconds.
static CAPTURE: AtomicBool = AtomicBool::new(false);
#[derive(Clone, Copy)]
pub enum Work {
    Think,
    Units,
    Combat,
    OwnedAbilities,
    OwnedSteering,
    OutlineInclusive,
    LogIo,
    CaptureToPlayback,
    AttackInclusive,
    SkillInclusive,
    MoveInclusive,
    ShopDecision,
}
const WORK_NAMES: [&str; 12] = [
    "sdk_think_all_threads_incl_gate",
    "selected_unit_scan",
    "selected_combat",
    "owned_ability_observation",
    "owned_steering_mod",
    "outline_incl_native",
    "log_io_incl_rotation",
    "capture_to_playback",
    "attack_incl_native_all_threads",
    "skill_incl_native_all_threads",
    "move_incl_native_all_threads",
    "shop_answer_mod_incl_lock",
];
const WORK_BINS: usize = 25;
static WORK_STATS: [Stat; WORK_NAMES.len()] = [const { Stat::new() }; WORK_NAMES.len()];
static WORK_HIST: [[AtomicU64; WORK_BINS]; WORK_NAMES.len()] =
    [const { [const { AtomicU64::new(0) }; WORK_BINS] }; WORK_NAMES.len()];
pub fn set_capture(enabled: bool) {
    CAPTURE.store(enabled, Relaxed);
}
pub fn capture_enabled() -> bool {
    CAPTURE.load(Relaxed)
}
pub struct WorkTimer(Work, Option<Instant>);
pub fn work(kind: Work) -> WorkTimer {
    WorkTimer(kind, capture_enabled().then(Instant::now))
}
impl Drop for WorkTimer {
    fn drop(&mut self) {
        if let Some(at) = self.1 {
            record_work(self.0, at.elapsed().as_micros() as u64);
        }
    }
}
fn work_bin(us: u64) -> usize {
    if us <= 1 {
        0
    } else {
        (64 - (us - 1).leading_zeros() as usize).min(WORK_BINS - 1)
    }
}
fn record_work(kind: Work, us: u64) {
    WORK_STATS[kind as usize].add(us);
    WORK_HIST[kind as usize][work_bin(us)].fetch_add(1, Relaxed);
}
pub fn playback_latency(at: Instant) {
    if capture_enabled() {
        record_work(Work::CaptureToPlayback, at.elapsed().as_micros() as u64);
    }
}
fn work_percentile(bins: &[u64; WORK_BINS], count: u64, numerator: u64) -> String {
    if count == 0 {
        return "-".into();
    }
    let wanted = count.saturating_mul(numerator).div_ceil(100);
    let mut seen = 0;
    for (i, n) in bins.iter().enumerate() {
        seen += n;
        if seen >= wanted {
            return if i == WORK_BINS - 1 {
                format!(">{}", 1u64 << (i - 1))
            } else {
                format!("<= {}", 1u64 << i)
            };
        }
    }
    "-".into()
}
fn report_work(log: &Logger) {
    for (i, s) in WORK_STATS.iter().enumerate() {
        let (total, max, count) = s.take();
        let bins = std::array::from_fn(|j| WORK_HIST[i][j].swap(0, Relaxed));
        if let Some(avg) = total.checked_div(count) {
            log.write(&format!("PERF WORK {} count={count} avg_us={avg} max_us={max} p95_us={} p99_us={} total_us={total}; overlapping scopes, not additive CPU totals", WORK_NAMES[i], work_percentile(&bins, bins.iter().sum(), 95), work_percentile(&bins, bins.iter().sum(), 99)));
        }
    }
}

thread_local! {
    static LOCAL_COUNTS: [[Cell<u64>; 2]; HOOKS] =
        const { [const { [const { Cell::new(0) }; 2] }; HOOKS] };
    static LOCAL_PENDING: Cell<u64> = const { Cell::new(0) };
}

fn now() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_micros() as u64
}

pub struct Timer(Section, Instant);
impl Drop for Timer {
    fn drop(&mut self) {
        let micros = self.1.elapsed().as_micros() as u64;
        SECTION_STATS[self.0 as usize].add(micros);
        FRAME[self.0 as usize].fetch_add(micros, Relaxed);
    }
}
pub fn time(section: Section) -> Timer {
    Timer(section, Instant::now())
}
pub fn waited(wait: Wait, since: Instant) {
    WAIT_STATS[wait as usize].add(since.elapsed().as_micros() as u64);
}
/// The viewed match's worker thread (written only when it changes).
pub fn worker_thread() {
    let id = crate::platform_input::thread_id();
    if WORKER_THREAD.load(Relaxed) != id {
        WORKER_THREAD.store(id, Relaxed);
    }
}
/// True on the viewed match's simulation worker.
pub fn on_worker_thread() -> bool {
    crate::platform_input::thread_id() == WORKER_THREAD.load(Relaxed)
}
pub fn hook(hook: Hook) {
    let other = usize::from(crate::platform_input::thread_id() != WORKER_THREAD.load(Relaxed));
    let _ = LOCAL_COUNTS.try_with(|counts| {
        let c = &counts[hook as usize][other];
        c.set(c.get() + 1);
        let _ = LOCAL_PENDING.try_with(|pending| {
            let n = pending.get() + 1;
            if n < FLUSH_EVERY {
                pending.set(n);
                return;
            }
            pending.set(0);
            for (local, global) in counts.iter().zip(&HOOK_COUNTS) {
                for (l, g) in local.iter().zip(global) {
                    let v = l.replace(0);
                    if v > 0 {
                        g.fetch_add(v, Relaxed);
                    }
                }
            }
        });
    });
}
pub fn logged(bytes: usize) {
    LOG_BYTES.fetch_add(bytes as u64, Relaxed);
}

fn percentile(buckets: &[u64; 101], total: u64, p: f64) -> String {
    let wanted = ((total as f64 * p).ceil() as u64).max(1);
    let mut seen = 0;
    for (ms, n) in buckets.iter().enumerate() {
        seen += n;
        if seen >= wanted {
            return if ms == 100 {
                "100+".into()
            } else {
                ms.to_string()
            };
        }
    }
    "-".into()
}
fn listed(values: &[(usize, u64)], range: std::ops::Range<usize>) -> String {
    values
        .iter()
        .filter(|(i, _)| range.contains(i))
        .map(|(i, us)| format!("{}={us}", SECTION_NAMES[*i]))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Client thread, once per frame (start of post_update). On the battlefield a
/// slow frame is logged with what ran since the previous frame; a summary is
/// written per window.
pub fn frame(battlefield: bool, scene: &dyn std::fmt::Debug, log: &Logger) {
    let at = now();
    let last = LAST_FRAME.swap(at, Relaxed);
    let since: [(usize, u64); SECTIONS] = std::array::from_fn(|i| (i, FRAME[i].swap(0, Relaxed)));
    if last != 0 {
        let interval = at - last;
        INTERVALS[((interval / 1000) as usize).min(100)].fetch_add(1, Relaxed);
        INTERVAL_MAX.fetch_max(interval, Relaxed);
        if battlefield
            && interval >= SLOW_MICROS
            && crate::logging::enabled("PERF SLOW")
            && SLOW_LINES.fetch_add(1, Relaxed) < SLOW_LINES_PER_WINDOW
        {
            let total: u64 = since
                .iter()
                .filter(|(i, _)| *i < MOD_TOTAL)
                .map(|(_, us)| us)
                .sum();
            log.write(&format!(
                "PERF SLOW frame_ms={:.1} mod_us={total} [{}] breakdown [{}] native_us [{}]",
                interval as f64 / 1000.,
                listed(&since, 0..MOD_TOTAL),
                listed(&since, MOD_TOTAL..BREAKDOWN_END),
                listed(&since, BREAKDOWN_END..SECTIONS),
            ));
        }
    }
    let previous = LAST_REPORT.load(Relaxed);
    if previous == 0 {
        LAST_REPORT.store(at, Relaxed);
        return;
    }
    if at - previous < WINDOW_MICROS {
        return;
    }
    LAST_REPORT.store(at, Relaxed);
    SLOW_LINES.store(0, Relaxed);
    let seconds = (at - previous) as f64 / 1e6;
    let buckets: [u64; 101] = std::array::from_fn(|i| INTERVALS[i].swap(0, Relaxed));
    let frames: u64 = buckets.iter().sum();
    let slow: u64 = buckets[25..].iter().sum();
    let worst = INTERVAL_MAX.swap(0, Relaxed);
    let stats: Vec<(usize, (u64, u64, u64))> = SECTION_STATS
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.take()))
        .filter(|(_, (_, _, count))| *count > 0)
        .collect();
    let group = |range: std::ops::Range<usize>| {
        stats
            .iter()
            .filter(|(i, _)| range.contains(i))
            .map(|(i, (total, max, count))| {
                format!(
                    "{}={:.0}/{max}/{}",
                    SECTION_NAMES[*i],
                    *total as f64 / *count as f64,
                    total / 1000
                )
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mod_ms: u64 = stats
        .iter()
        .filter(|(i, _)| *i < MOD_TOTAL)
        .map(|(_, (total, _, _))| total / 1000)
        .sum();
    let waits = WAIT_STATS
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let (total, max, count) = s.take();
            (count > 0).then(|| format!("{}={count}/{total}/{max}", WAIT_NAMES[i]))
        })
        .collect::<Vec<_>>()
        .join(" ");
    let hooks = HOOK_COUNTS
        .iter()
        .enumerate()
        .map(|(i, [worker, other])| {
            format!(
                "{}={}+{}",
                HOOK_NAMES[i],
                worker.swap(0, Relaxed),
                other.swap(0, Relaxed)
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    log.write(&format!(
        "PERF window_s={seconds:.1} scene={scene:?} frames={frames} updates_per_s={:.1} frame_ms p50={} p95={} p99={} max={:.1} slow_25ms={slow} | mod_total_ms={mod_ms} mod avg_us/max_us/total_ms: {} | breakdown: {} | native (not mod cost): {} | waits count/total_us/max_us: {waits} | hook_calls worker+other_threads: {hooks} | log_bytes={}",
        frames as f64 / seconds,
        percentile(&buckets, frames, 0.5),
        percentile(&buckets, frames, 0.95),
        percentile(&buckets, frames, 0.99),
        worst as f64 / 1000.,
        group(0..MOD_TOTAL),
        group(MOD_TOTAL..BREAKDOWN_END),
        group(BREAKDOWN_END..SECTIONS),
        LOG_BYTES.swap(0, Relaxed),
    ));
    report_work(log);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn percentiles_read_whole_millisecond_buckets() {
        let mut b = [0u64; 101];
        b[16] = 90;
        b[33] = 9;
        b[100] = 1;
        assert_eq!(percentile(&b, 100, 0.5), "16");
        assert_eq!(percentile(&b, 100, 0.95), "33");
        assert_eq!(percentile(&b, 100, 1.0), "100+");
        assert_eq!(percentile(&[0; 101], 0, 0.5), "-");
    }
    #[test]
    fn section_groups_keep_mod_total_breakdown_and_native_apart() {
        assert_eq!(SECTION_NAMES[Section::Render as usize], "post_render");
        assert!((Section::Render as usize) < MOD_TOTAL);
        assert_eq!(Section::Info as usize, MOD_TOTAL);
        assert_eq!(Section::Cursor as usize + 1, BREAKDOWN_END);
        assert_eq!(Section::WorkerAfterSend as usize + 1, SECTIONS);
        assert_eq!(HOOK_NAMES[Hook::Outline as usize], "outline");
        assert_eq!(WAIT_NAMES[Wait::Pacing as usize], "worker_pacing");
    }
    #[test]
    fn work_histogram_bounds_do_not_understate_samples() {
        assert_eq!(work_bin(0), 0);
        assert_eq!(work_bin(1), 0);
        assert_eq!(work_bin(2), 1);
        assert_eq!(work_bin(3), 2);
        assert_eq!(work_bin(4), 2);
        assert_eq!(work_bin(u64::MAX), WORK_BINS - 1);
        let mut bins = [0; WORK_BINS];
        bins[4] = 95;
        bins[10] = 4;
        bins[WORK_BINS - 1] = 1;
        assert_eq!(work_percentile(&bins, 100, 95), "<= 16");
        assert_eq!(work_percentile(&bins, 100, 99), "<= 1024");
        assert_eq!(work_percentile(&bins, 100, 100), ">8388608");
    }
}
