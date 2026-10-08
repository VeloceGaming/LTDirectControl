//! Diagnostic timestamps only; never influence simulation input or pacing.
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
#[derive(Clone, Copy, Debug)]
pub struct Stamp {
    pub id: u64,
    pub at: Instant,
    pub label: &'static str,
}
impl Stamp {
    pub fn new(label: &'static str) -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        Self {
            id: SERIAL.fetch_add(1, Ordering::Relaxed),
            at: Instant::now(),
            label,
        }
    }
}
#[derive(Clone, Copy)]
pub struct FrameTrace {
    pub stamp: Stamp,
    pub frame: usize,
    pub published: bool,
}
