//! Work moved to before the match starts (1.6.0-perf): nothing here changes
//! what the player sees.
//!
//! - Pre-spawn: the windows that 1.5.3 built from their template the first
//!   time they were needed mid-match (stats panels, state plate, screen
//!   effect, Tab, shop, Settings, the emote wheel) are built hidden while
//!   you choose your athlete (session phase Ready), all in the first such
//!   frame, each try at most once a second. The first opening then costs
//!   no template parse. Only after the HUD strip exists, so the windows join the game's
//!   UI tree in the order a first opening after control starts gives
//!   (their z values keep them apart anyway). Each window's own spawn code
//!   runs, with what it resets on a build (its write cache, click
//!   registration), so its first opening sends exactly what 1.5.3 sent.
//! - Font warm-up: once per game session, in the first frame drawn in a
//!   pre-match scene, one short text in each of the mod's two fonts and in
//!   the game's regular font (the mod's drawn notices) is drawn far off
//!   screen, so the game's text engine has the glyphs before the
//!   match. (That this avoids a first-use cost is a guess: whether the game
//!   rasterises text drawn off screen is not known.)
use crate::Logger;
use mod_api_stable::StableClient;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::{Duration, Instant};

/// A window that can be built hidden ahead of its first use.
pub trait Prespawn {
    /// Whether the window's root exists in the game's UI tree.
    fn built(&self, ctx: &StableClient<'_>) -> bool;
    /// Build the window hidden (it is missing); true on success.
    fn prespawn(&mut self, ctx: &mut StableClient<'_>, log: &Logger) -> bool;
}
/// The pre-spawn schedule.
#[derive(Default)]
pub struct Schedule {
    tried: [Option<Instant>; 8],
}
impl Schedule {
    /// While you choose your athlete: build every missing window of
    /// `windows`, in order, each tried at most once a second. All in one
    /// frame, since Ready can last only a few frames (about 90 ms between
    /// the lock and Start in the author's log of 2026-10-24).
    pub fn step(
        &mut self,
        ctx: &mut StableClient<'_>,
        windows: &mut [&mut dyn Prespawn],
        log: &Logger,
    ) {
        for (window, tried) in windows.iter_mut().zip(self.tried.iter_mut()) {
            if window.built(ctx)
                || tried.is_some_and(|t| crate::clock::since(t) < Duration::from_secs(1))
            {
                continue;
            }
            *tried = Some(crate::clock::now());
            window.prespawn(ctx, log);
        }
    }
}

/// The mod's fonts (HUD labels: `hud_style::fonts`; drawn numbers: the
/// input-delay readout) and the regular font of its drawn notices
/// (`Client::post_render_inner`: test, practice and release messages).
const FONTS: [&str; 3] = [
    "asset/lt_direct_control/font/numeric",
    "asset/lt_direct_control/font/medium",
    "asset/base/font/set/regular",
];
/// The text sizes the HUD and its panels use most.
const SIZES: [f32; 5] = [13., 14., 18., 20., 23.];
const SAMPLE: &str = "0123456789 / — % : + ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz";
static WARMED: AtomicBool = AtomicBool::new(false);
/// Once per game session, in a pre-match scene's drawing callback.
pub fn warm_fonts(ctx: &mut StableClient<'_>, log: &Logger) {
    if WARMED.swap(true, Relaxed) {
        return;
    }
    for font in FONTS {
        for (k, size) in SIZES.into_iter().enumerate() {
            // Far left of the screen (UI space is 1920 x 1080): never visible.
            ctx.draw_text(
                "UI",
                SAMPLE,
                font,
                (-4000., 40. * k as f32, 1600., 40.),
                0,
                size,
                0xffffffff,
                mod_api_stable::TextAlignXV1::Left,
                mod_api_stable::TextAlignYV1::Center,
            );
        }
    }
    log.write("PERF fonts warmed off screen (numeric, medium, regular)");
}
