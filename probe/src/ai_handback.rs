//! AI control keeps the match paced a few ticks ahead, so F11 can take
//! control back within a frame. The game's "view result" button needs the
//! game to run the whole match ahead, so in AI control pressing it hands the
//! match back to the game completely (as F12 does): the result then works
//! natively, and the match can no longer be taken back.
//!
//! The game's speed buttons are not watched here: since 1.5.1-dev1 the AI
//! phase follows the game's speed field (`native_timing`). The old check
//! compared the buttons' selection state, which the SDK never reported
//! (`ui_selectable_selected` was None for every button), so it never fired.
use crate::{camera::Rect, native_timing, platform_input::Keys, Logger};
use mod_api_stable::StableClient;

/// The game's "view result now" buttons (compact panel, wide layout).
const VIEW_RESULT: [&str; 2] = [
    "ingame.option_buttons.view_result",
    "ingame.pad_view_result",
];

#[derive(Default)]
pub struct AiHandback {
    previous_click: bool,
}
impl AiHandback {
    /// Client, each frame after the session heartbeat.
    pub fn update(
        &mut self,
        ctx: &StableClient<'_>,
        timing: &native_timing::NativeTiming,
        keys: Keys,
        log: &Logger,
    ) {
        let click = keys.focused && keys.raw.0[1];
        let pressed = click && !self.previous_click;
        self.previous_click = click;
        let ai = timing.client_session(None) && timing.phase() == Some(native_timing::Phase::Ai);
        if !ai || !pressed {
            return;
        }
        let buttons =
            VIEW_RESULT.map(|path| shown(ctx, path).then(|| ctx.ui_node_rect(path)).flatten());
        if view_result_pressed(keys.cursor, &buttons) {
            timing.cancel(
                "View result pressed in AI control; match handed back to the game",
                log,
            );
        }
    }
}
/// A new click at `cursor` lands on one of the shown "view result" buttons
/// (`None`: hidden or not found).
fn view_result_pressed(
    cursor: Option<(f32, f32)>,
    buttons: &[Option<(f32, f32, f32, f32)>],
) -> bool {
    cursor.is_some_and(|p| {
        buttons
            .iter()
            .flatten()
            .any(|&(x, y, w, h)| Rect { x, y, w, h }.contains(p))
    })
}
/// The node and every ancestor are visible.
fn shown(ctx: &StableClient<'_>, path: &str) -> bool {
    ctx.ui_visible(path) == Some(true)
        && path
            .match_indices('.')
            .all(|(end, _)| ctx.ui_visible(&path[..end]) != Some(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::logger;
    #[test]
    fn a_click_on_a_shown_view_result_button_counts_and_others_do_not() {
        let compact = Some((100., 200., 50., 20.));
        assert!(view_result_pressed(Some((120., 210.)), &[compact, None]));
        assert!(view_result_pressed(Some((120., 210.)), &[None, compact]));
        assert!(!view_result_pressed(Some((10., 10.)), &[compact, None]));
        assert!(!view_result_pressed(Some((120., 210.)), &[None, None]));
        assert!(!view_result_pressed(None, &[compact, compact]));
    }
    #[test]
    fn view_result_still_hands_the_ai_match_back() {
        // What `update` does on a hit: the session is released for good.
        let log = logger("ai-view-result");
        let t = native_timing::NativeTiming::running_test_worker((1, 2, 3));
        t.cancel(
            "View result pressed in AI control; match handed back to the game",
            &log,
        );
        assert_eq!(
            t.worker_phase((1, 2, 3)),
            Some(native_timing::Phase::Released)
        );
    }
}
