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
    "WORKER ENDED",
];
/// Lines starting with one of these are written only at Verbose.
const VERBOSE: &[&str] = &[
    "STATRULE collision site=",
    "STATRULE gate site=",
    "ABILITY AIMING",
    "ABILITY CANCEL",
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
    // Successful Q/W/R presses (the author, 2026-10-21: split by outcome);
    // denied, refused, cancelled and dropped presses stay at Normal.
    "CAST TIMING Q outcome=cast",
    "CAST TIMING W outcome=cast",
    "CAST TIMING R outcome=cast",
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
    "PATCH FIELDS",
    "PERF ",
    "PLAYBACK ",
    "PLAYER HUD",
    "POINTER ",
    // Skill preview research (1.5.0-dev20 audit): start-up and per-match
    // diagnostics, and the research build's recording traces. The shape
    // table is written whatever the level; tools/audit_previews.py needs
    // Verbose.
    "PREVIEW BASE",
    "PREVIEW COVERAGE",
    "PREVIEW DECLARED",
    "PREVIEW FILE",
    "PREVIEW GEOMETRY",
    "PREVIEW ROSTER",
    "PREVIEW TREE",
    "PREVIEW declared_slots",
    "PURCHASE TRACE",
    "RECALL NATIVE",
    "RECALL PRESS",
    "ROSTER ",
    "SHAPE NUMBERS",
    "SHAPE RECORD",
    "SHOP HOOK PROOF",
    // Every purchase by all ten champions.
    "SHOP PURCHASE",
    "SIM ",
    "SPRITE PICKING",
    // The stats panel's raw numbers every 10 s (an investigation, backlog item 5).
    "STATS own raw",
    "TEAM STATUS",
];

pub fn level_of(line: &str) -> Level {
    if SAFETY.iter().any(|p| line.starts_with(p))
        || ["panic", "failed", "failure"].iter().any(|w| {
            line.as_bytes()
                .windows(w.len())
                .any(|part| part.eq_ignore_ascii_case(w.as_bytes()))
        })
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
    if line.starts_with("PERF ") && crate::perf::capture_enabled() {
        return true;
    }
    level_of(line) as u8 <= CHOSEN.load(Ordering::Relaxed)
}
pub fn verbose() -> bool {
    CHOSEN.load(Ordering::Relaxed) >= Level::Verbose as u8
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
        // The 1.5.0-dev20 audit: per-press, per-purchase and research traces
        // are Verbose; refusals and once-per-match preview lines stay Normal.
        for line in [
            "CAST TIMING Q outcome=cast target=cursor",
            "CAST TIMING R outcome=cast press_action=2",
            "ABILITY CANCEL R reason=newer-skill",
            "SHOP PURCHASE tick=3 player=0",
            "STATS own raw attack=100",
            "PREVIEW FILE champion=amazon",
            "PREVIEW TREE viewed=false",
            "SHAPE NUMBERS champion=amazon",
            "PATCH FIELDS champion=amazon",
            // Each reaction gate's per-shot line; the first-fire line stays Normal.
            "STATRULE gate site=0xfa1d2c caster=3 A=80 D=30",
        ] {
            assert_eq!(level_of(line), Level::Verbose, "{line}");
        }
        for line in [
            "ABILITY REJECTED W reason=native-consumer-did-not-spend-cooldown",
            "CAST TIMING W outcome=denied reason=on-cooldown",
            "CAST TIMING W outcome=refused reason=native-consumer-did-not-spend-cooldown",
            "CAST TIMING Q outcome=cancelled reason=right-click",
            "PREVIEW NAMES champion=hammerer slot=W read 1 of 1",
            "PREVIEW TABLE champions=96",
            "PREVIEW COMPARE champion=hammerer slot=W",
            "SHOP STATE queue=[]",
            "STATRULE installed sites=35 at the title screen",
            "STATRULE gate: first reaction of athlete 3 gated at 12 ticks (game 9)",
        ] {
            assert_eq!(level_of(line), Level::Normal, "{line}");
        }
    }
}
