//! Temporary visibility override; native preference is restored on release.
#[derive(Default)]
pub struct TeamInfo {
    original: Option<bool>,
    hidden: Vec<(&'static str, bool)>,
    original_position: Option<(f32, f32)>,
    engaged: bool,
    layout_failure_logged: bool,
    hover_targets: Vec<String>,
    info_tooltip_checked: bool,
    info_tooltip_found: bool,
    info_tooltip_reports: usize,
    reported_visibility: Option<bool>,
    /// The centred scoreboard layout was written since engaging.
    layout_written: bool,
}
/// The scoreboard panel is centred on its parent (within a pixel): the
/// layout the mod writes. Rectangles are (x, y, width, height).
fn scoreboard_centred(panel: (f32, f32, f32, f32), parent: (f32, f32, f32, f32)) -> bool {
    let centre = |(x, y, w, h): (f32, f32, f32, f32)| (x + w / 2., y + h / 2.);
    let (a, b) = (centre(panel), centre(parent));
    (a.0 - b.0).abs() <= 1. && (a.1 - b.1).abs() <= 1.
}
/// Write the scoreboard's layout: once on engaging, then only when its
/// rectangle shows the game moved it off centre. Unknown rectangles (a
/// hidden panel) are not a reason to write again.
fn scoreboard_needs_write(
    written: bool,
    panel: Option<(f32, f32, f32, f32)>,
    parent: Option<(f32, f32, f32, f32)>,
) -> bool {
    !written
        || panel
            .zip(parent)
            .is_some_and(|(panel, parent)| !scoreboard_centred(panel, parent))
}
impl TeamInfo {
    pub fn apply(
        &mut self,
        ctx: &mut mod_api_stable::StableClient<'_>,
        active: bool,
        focused: bool,
        tab: bool,
        log: &crate::Logger,
    ) {
        const PANEL: &str = "ingame.player_info";
        if active {
            if !self.engaged {
                self.original_position = Some(
                    ctx.ui_node_rect(PANEL)
                        .filter(|&(x, y, w, h)| {
                            [x, y, w, h].into_iter().all(f32::is_finite) && w > 0. && h > 0.
                        })
                        .map(|(x, y, _, _)| (x, y))
                        // Hidden nodes may not have a computed rectangle yet. This
                        // position is from the installed 0.6.2 ingame.ui template.
                        .unwrap_or((994., 64.)),
                );
                for path in [
                    "ingame.wide_data",
                    "ingame.center_data",
                    "ingame.wide_info",
                    "ingame.strategy_info",
                    "ingame.player_detail",
                    "ingame.center_detail",
                    "ingame.wide_bottom",
                    "ingame.speed_buttons",
                    "ingame.time_control",
                    "ingame.champion_detail_slots",
                ] {
                    if let Some(visible) = ctx.ui_visible(path) {
                        self.hidden.push((path, visible));
                    }
                }
                self.engaged = true;
                // The 0.6.2 detail_slot template leaves these entries and their
                // icon_slot hover surfaces event-enabled even when hidden.
                // Only override known event-enabled nodes; bg/icon children
                // already ignore events and are left unchanged.
                const SLOTS: &str = "ingame.champion_detail_slots";
                if ctx.ui_exists(SLOTS) {
                    self.hover_targets.push(SLOTS.into());
                    for child in ctx.ui_child_names(SLOTS).into_iter().take(20) {
                        let path = format!("{SLOTS}.{child}");
                        self.hover_targets.push(path.clone());
                        let icon = format!("{path}.icon_slot");
                        if ctx.ui_exists(&icon) {
                            self.hover_targets.push(icon);
                        }
                    }
                }
                for path in &self.hover_targets {
                    ctx.ui_set_properties(path, "ignore_event: true;");
                }
                log.write(&format!(
                    "HUD takeover: spectator panels={:?}; scoreboard original_position={:?}",
                    self.hidden, self.original_position
                ));
            }
            for &(path, _) in &self.hidden {
                if ctx.ui_visible(path) != Some(false) {
                    ctx.ui_set_visible(path, false);
                }
            }
            // Native hover code can republish tooltips after its UI update.
            // Suppress only spectator tooltips; the player's HUD owns its own.
            // This larger popup is separate from the short champion_tooltip.
            // Check each frame because native code can create it on first hover.
            const INFO_TOOLTIP: &str = "ingame.champion_info_tooltip";
            let info_visible = ctx.ui_visible(INFO_TOOLTIP);
            if !self.info_tooltip_checked || (!self.info_tooltip_found && info_visible.is_some()) {
                log.write(&format!(
                    "HUD champion info popup path={INFO_TOOLTIP} exists={} visible={info_visible:?}",
                    ctx.ui_exists(INFO_TOOLTIP)
                ));
                self.info_tooltip_checked = true;
                self.info_tooltip_found = info_visible.is_some();
            }
            if info_visible == Some(true) {
                let applied = ctx.ui_set_visible(INFO_TOOLTIP, false);
                if self.info_tooltip_reports < 4 {
                    log.write(&format!(
                        "HUD champion info popup suppress applied={applied} after={:?}",
                        ctx.ui_visible(INFO_TOOLTIP)
                    ));
                    self.info_tooltip_reports += 1;
                }
            }
            for path in [
                "ingame.champion_tooltip",
                "ingame.item_tooltip",
                "ingame.tooltip",
            ] {
                if ctx.ui_visible(path) == Some(true) {
                    ctx.ui_set_visible(path, false);
                }
            }
            // Before 1.3.7 this was written every frame. Now: on engaging, and
            // again only when the game has moved the panel off centre. (The
            // per-frame write may have been deliberate; to be seen in game.)
            if scoreboard_needs_write(
                self.layout_written,
                ctx.ui_node_rect(PANEL),
                ctx.ui_node_rect("ingame"),
            ) {
                self.layout_written = true;
                if !ctx.ui_set_properties(PANEL, "x: 0px; y: 0px; anchor_x: 0.5; pivot_x: 0.5; anchor_y: 0.5; pivot_y: 0.5; ignore_event: true; color: #292929ff;") && !self.layout_failure_logged {
                    log.write("HUD centered scoreboard properties rejected");
                    self.layout_failure_logged = true;
                }
            }
        } else if self.engaged {
            for path in self.hover_targets.drain(..) {
                ctx.ui_set_properties(&path, "ignore_event: false;");
            }
            for (path, visible) in self.hidden.drain(..) {
                ctx.ui_set_visible(path, visible);
            }
            self.layout_written = false;
            if let Some((x, y)) = self.original_position.take() {
                ctx.ui_set_properties(PANEL, &format!("x: {x}px; y: {y}px; anchor_x: 0; pivot_x: 0; anchor_y: 0; pivot_y: 0; ignore_event: false; color: #161721ff;"));
            }
            self.engaged = false;
            self.layout_failure_logged = false;
            self.info_tooltip_checked = false;
            self.info_tooltip_found = false;
            self.info_tooltip_reports = 0;
            log.write("HUD native spectator visibility and scoreboard position restored");
        }
        let native = ctx.ui_visible(PANEL);
        if let Some(desired) = self.visibility(active, focused, tab, native) {
            if native != Some(desired) {
                let applied = ctx.ui_set_visible(PANEL, desired);
                if self.reported_visibility != Some(desired) {
                    self.reported_visibility = Some(desired);
                    log.write(&format!(
                        "HUD team info visible={desired} applied={applied}; centered hold Tab"
                    ));
                }
            }
        }
    }
    pub fn visibility(
        &mut self,
        active: bool,
        focused: bool,
        tab: bool,
        native: Option<bool>,
    ) -> Option<bool> {
        if active {
            let native = native?;
            self.original.get_or_insert(native);
            Some(focused && tab)
        } else {
            self.original.take()
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hold_release_focus_loss_and_native_visibility_restore() {
        for original in [false, true] {
            let mut info = TeamInfo::default();
            assert_eq!(
                info.visibility(true, true, false, Some(original)),
                Some(false)
            );
            assert_eq!(info.visibility(true, true, true, Some(false)), Some(true));
            assert_eq!(info.visibility(true, false, true, Some(true)), Some(false));
            assert_eq!(info.visibility(true, true, false, Some(false)), Some(false));
            assert_eq!(
                info.visibility(false, true, false, Some(false)),
                Some(original)
            );
            assert_eq!(info.visibility(false, true, true, Some(original)), None);
        }
        let mut info = TeamInfo::default();
        assert_eq!(info.visibility(true, true, true, None), None);
        assert_eq!(info.visibility(false, true, true, Some(true)), None);
    }
    #[test]
    fn the_scoreboard_is_written_on_engaging_and_again_only_when_moved() {
        let screen = Some((0., 0., 1920., 1080.));
        let centred = Some((660., 290., 600., 500.));
        let moved = Some((994., 64., 600., 500.));
        assert!(scoreboard_needs_write(false, centred, screen));
        assert!(scoreboard_needs_write(false, None, None));
        assert!(!scoreboard_needs_write(true, centred, screen));
        assert!(!scoreboard_needs_write(
            true,
            Some((660.6, 289.5, 600., 500.)),
            screen
        ));
        assert!(scoreboard_needs_write(true, moved, screen));
        // A hidden panel with no rectangle is not rewritten every frame.
        assert!(!scoreboard_needs_write(true, None, screen));
    }
}
