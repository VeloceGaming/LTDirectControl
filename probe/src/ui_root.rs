//! Showing and hiding the mod's window roots (1.6.0-perf), and the hidden
//! diagnostic switch `perf_hide_mod_ui`.
//!
//! A window runs every frame, so its root's visibility is written only when
//! the game's own state differs (`show`, `hide`): an unchanged root costs a
//! read, not a write.
//!
//! `perf_hide_mod_ui` (controls.json only, no control in the Settings
//! window, default off) keeps every mod window hidden while the rest of the
//! mod runs unchanged: state, timers, sounds, input and the battlefield
//! drawing. It is for an A/B measurement of what the game spends on the
//! mod's UI-tree nodes. While it is on, the HUD's rectangles stop blocking
//! battlefield clicks (nothing is shown there), and an open Settings window
//! still pauses the match, invisibly.
use mod_api_stable::StableClient;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

static HIDE: AtomicBool = AtomicBool::new(false);

/// Once per frame (client thread): read the switch; log when it changes.
pub fn configure(on: bool, log: &crate::Logger) {
    if HIDE.swap(on, Relaxed) != on {
        log.write(&format!(
            "PERF perf_hide_mod_ui={on}: every mod window {}",
            if on {
                "stays hidden (state, sounds, input and battlefield drawing continue)"
            } else {
                "shows again"
            }
        ));
    }
}
/// True while `perf_hide_mod_ui` keeps every mod window hidden.
pub fn hidden() -> bool {
    HIDE.load(Relaxed)
}
/// Hide `path` if it exists and the game does not already hold it hidden.
pub fn hide(ctx: &mut StableClient<'_>, path: &str) {
    if ctx.ui_visible(path) != Some(false) && ctx.ui_exists(path) {
        ctx.ui_set_visible(path, false);
    }
}
/// Show `path` unless the game already holds it shown (or the switch keeps
/// mod windows hidden, then hide it).
pub fn show(ctx: &mut StableClient<'_>, path: &str) {
    if hidden() {
        hide(ctx, path);
    } else if ctx.ui_visible(path) != Some(true) {
        ctx.ui_set_visible(path, true);
    }
}
