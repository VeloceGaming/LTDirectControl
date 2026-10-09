//! AI control keeps the match paced at 1x, one frame ahead, so F11 can take
//! control back on the next frame. The game's speed buttons and its "view
//! result" button need the game to run ahead, so in AI control using either
//! hands the match back to the game completely (as F12 does): speed and skip
//! then work natively, and the match can no longer be taken back.
use crate::{camera::Rect, native_timing, platform_input::Keys, Logger};
use mod_api_stable::StableClient;

const SPEEDS: [&str; 6] = [
    "speed05x",
    "speed1x",
    "speed15x",
    "speed2x",
    "speed3x",
    "speed_highlight",
];
/// The game's "view result now" buttons (compact panel, wide layout).
const VIEW_RESULT: [&str; 2] = [
    "ingame.option_buttons.view_result",
    "ingame.pad_view_result",
];

#[derive(Default)]
pub struct AiHandback {
    /// Speed buttons' selection when AI control began; a change hands back.
    entry: Option<[Option<bool>; 6]>,
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
        if !ai {
            self.entry = None;
            return;
        }
        let speeds =
            SPEEDS.map(|n| ctx.ui_selectable_selected(&format!("ingame.speed_buttons.{n}")));
        let entry = *self.entry.get_or_insert(speeds);
        let result = pressed
            && keys.cursor.is_some_and(|p| {
                VIEW_RESULT.iter().any(|path| {
                    shown(ctx, path)
                        && ctx
                            .ui_node_rect(path)
                            .is_some_and(|(x, y, w, h)| Rect { x, y, w, h }.contains(p))
                })
            });
        let reason = if speeds != entry {
            "Game speed changed in AI control; match handed back to the game"
        } else if result {
            "View result pressed in AI control; match handed back to the game"
        } else {
            return;
        };
        self.entry = None;
        timing.cancel(reason, log);
    }
}
/// The node and every ancestor are visible.
fn shown(ctx: &StableClient<'_>, path: &str) -> bool {
    ctx.ui_visible(path) == Some(true)
        && path
            .match_indices('.')
            .all(|(end, _)| ctx.ui_visible(&path[..end]) != Some(false))
}
