//! Log detail levels. Every log line is classified by its leading tag, so the
//! level is decided here in one place and call sites stay plain
//! `logger.write(..)`. A new tag counts as Normal until it is listed below.
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Install, fingerprint and release results, fail-closed shop decisions,
    /// and anything reporting a failure or panic. Always written.
    Safety = 0,
    /// Session, settings, shop and UI events used when testing a build.
    Normal = 1,
    /// Per-click, per-move and per-second traces and measurements.
    Verbose = 2,
}

/// Lines starting with one of these are always written.
const SAFETY: &[&str] = &[
    "INIT ",
    "TIMING ",
    "LOG ",
    "NATIVE install",
    "NATIVE CONFIG",
    "NATIVE RELEASE",
    "NATIVE PATCH_READBACK",
    "SHOP MODE",
    "SHOP UNEXPECTED",
    "SHOP ANCHOR",
    "SHOP HOOKS",
    "CURSOR native",
    "CURSOR window",
    "WORKER STALL",
    "WORKER RESUMED",
];
/// Lines starting with one of these are written only at Verbose.
const VERBOSE: &[&str] = &[
    "ABILITY AIMING",
    "ABILITY CHARGES",
    "ABILITY CONFIRM",
    "ABILITY EXECUTED",
    "ABILITY NATIVE_AIM",
    "ABILITY NATIVE_METADATA",
    "ABILITY PRESS",
    "ABILITY RESULT",
    "ABILITY WAIT",
    "AIM PRESERVED",
    "ATTACK ",
    "CAMERA WHEEL",
    "CAMERA Y",
    "CONTROL DIAGNOSTIC",
    "CURSOR HANDBACK",
    "CURSOR TRACE",
    "DISPLAY ",
    "HUD TOOLTIP",
    "INPUT TRACE",
    "KEY ",
    "MANUAL ATTACK_MOVE",
    "MANUAL CLICK",
    "MANUAL MOVE",
    "MANUAL NATIVE_STOP",
    "MANUAL input",
    "MOVEMENT STEERING ",
    "MOVEMENT TRACE",
    "NATIVE HOOK_ENTRY",
    "NATIVE SPECTATOR_KEY",
    "NATIVE STACK",
    "NATIVE TOOLTIP_TEXT",
    "NATIVE TRAFFIC",
    "NAVIGATION ",
    "OUTLINE ",
    "PACING ",
    "PERF ",
    "PLAYBACK ",
    "PLAYER HUD",
    "PREVIEW GEOMETRY",
    "PURCHASE TRACE",
    "RECALL NATIVE",
    "RECALL PRESS",
    "ROSTER ",
    "SHOP HOOK PROOF",
    "SIM ",
    "SPRITE PICKING",
    "TEAM STATUS",
];

pub fn level_of(line: &str) -> Level {
    let lower = |s: &str| s.to_ascii_lowercase();
    if SAFETY.iter().any(|p| line.starts_with(p))
        || ["panic", "failed", "failure"]
            .iter()
            .any(|w| lower(line).contains(w))
    {
        Level::Safety
    } else if VERBOSE.iter().any(|p| line.starts_with(p)) {
        Level::Verbose
    } else {
        Level::Normal
    }
}

/// The chosen level (Settings › Interface › Debug › Log detail).
// Tests see every line (they assert on traces).
static CHOSEN: AtomicU8 = AtomicU8::new(if cfg!(test) {
    Level::Verbose as u8
} else {
    Level::Normal as u8
});
pub fn set(setting: f64) {
    let level = match setting.round() as i64 {
        0 => Level::Safety,
        2 => Level::Verbose,
        _ => Level::Normal,
    };
    CHOSEN.store(level as u8, Ordering::Relaxed);
}
pub fn enabled(line: &str) -> bool {
    level_of(line) as u8 <= CHOSEN.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lines_are_classified_by_their_tag_and_failures_always_pass() {
        assert_eq!(level_of("NATIVE RELEASE reason=F12"), Level::Safety);
        assert_eq!(level_of("SHOP MODE first decision"), Level::Safety);
        assert_eq!(level_of("MANUAL CLICK player=2"), Level::Verbose);
        assert_eq!(level_of("MOVEMENT STEERING actor=20"), Level::Verbose);
        // A rejection warning is not the per-step steering trace.
        assert_eq!(level_of("MOVEMENT STEERING_REJECT actor=20"), Level::Normal);
        assert_eq!(level_of("SHOP STATE queue=[]"), Level::Normal);
        assert_eq!(level_of("SHOP UI spawn failed"), Level::Safety);
        assert_eq!(level_of("Worker adapter panic"), Level::Safety);
        assert_eq!(level_of("SOMETHING NEW from a modder"), Level::Normal);
    }
}
