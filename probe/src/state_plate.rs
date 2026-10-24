//! The state plate (S1): paused, dead or the AI driving, under the game's
//! clock. A feature, shown with "HUD motion" Off too. It takes no clicks
//! and is not in the camera's blocked rectangles. (Moved out of
//! player_hud.rs in 1.6.0; its writes are typed, see `ui_writes.rs`.)
use crate::ui_writes::{NodeId, PropCache};
use crate::{hud_motion_pass as pass, native_timing::Phase, player_hud::Snapshot, Logger};
use mod_api_stable::StableClient;
use std::time::{Duration, Instant};

/// What the state plate under the game's clock shows (design note "HUD
/// motion, second pass", S1). No words: a glyph, then a key cap or a count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plate {
    /// The match is paused: the pause glyph in signal yellow, then F11.
    Paused,
    /// Your athlete is dead: the skull, then whole seconds to respawn
    /// (simulation ticks, rounded up); None while unknown (the skull alone).
    Dead(Option<usize>),
    /// The game's AI drives your athlete: the processor glyph, then F11.
    Ai,
}
/// The plate for this frame: paused over dead over AI; nothing outside a
/// match you play or watch. The count comes from simulation ticks only, as
/// the strip's `respawn` label does (no wall-clock extrapolation).
pub fn plate_state(
    phase: Option<Phase>,
    alive: Option<bool>,
    respawn_ticks: usize,
) -> Option<Plate> {
    match phase? {
        Phase::Paused => Some(Plate::Paused),
        Phase::Running | Phase::Ai if alive == Some(false) => Some(Plate::Dead(
            (respawn_ticks > 0).then(|| respawn_ticks.div_ceil(60)),
        )),
        Phase::Ai => Some(Plate::Ai),
        _ => None,
    }
}

const WIDTH: usize = 1920;
/// The state plate's own root: it shows while the AI drives your athlete,
/// when the HUD strip is hidden, so it cannot live under the strip's root.
pub const PLATE_PATH: &str = "ingame.lt_state_plate";
/// Centred under the game's clock (60 x 28 at y 44); its bottom edge (148)
/// stays above the kill callouts, which start at y 150.
const PLATE_Y: usize = 100;
const PLATE_H: usize = 48;
/// Glyph and key cap (or count): 116 px. The skull alone: 48 px.
const PLATE_WIDE: usize = 116;
const PLATE_NARROW: usize = 48;
/// The glyph is 30 px, inset 9 px from the top; 20 px from the left on the
/// wide plate (glyph, 10 px gap, 36 px cap, 20 px), 9 px on the narrow one.
const PLATE_GLYPH: usize = 30;
const PLATE_PAD_WIDE: usize = 20;
const PLATE_PAD_NARROW: usize = 9;
fn plate_template() -> String {
    let x = WIDTH / 2 - PLATE_WIDE / 2;
    let mut s = format!("lt_state_plate:color {{ x: {x}px; y: {PLATE_Y}px; width: {PLATE_WIDE}px; height: {PLATE_H}px; z: 1120; color: #~1e1e1df0; visible: false; ignore_event: true; rounding: Uniform {{ rounding: 2; }}\n");
    s.push_str(&crate::ui_graphics::image(
        "glyph",
        "ef_skull",
        PLATE_PAD_WIDE,
        (PLATE_H - PLATE_GLYPH) / 2,
        PLATE_GLYPH,
        1121,
    ));
    // The key cap is drawn like the strip's Tab cap; key names are not translated.
    s.push_str("#key:color { x: 60px; y: 11px; width: 36px; height: 26px; z: 1121; color: #eeececff; visible: false; ignore_event: true; rounding: Uniform { rounding: 2; } #text:label { @\"asset/base/style/main#bold_label\"; width: 100%; height: 100%; size: 14; align_x: Center; align_y: Center; color: #393939ff; text: \"F11\"; ignore_event: true; z: 1122; } }\n");
    s.push_str("#count:label { @\"asset/base/style/main#bold_label\"; x: 56px; y: 8px; width: 44px; height: 32px; size: 23; align_x: Center; align_y: Center; color: #ffffffff; text: \"\"; visible: false; ignore_event: true; z: 1122; }\n}\n");
    crate::hud_style::fonts(s)
}
#[cfg(test)]
pub(crate) fn plate_template_for_test() -> String {
    plate_template()
}
/// The plate's nodes in its write cache.
struct Nodes {
    root: NodeId,
    glyph: NodeId,
    key: NodeId,
    key_text: NodeId,
    count: NodeId,
}
const GLYPHS: [&str; 3] = ["ef_pause", "ef_cpu", "ef_skull"];
#[derive(Default)]
pub struct StatePlate {
    cache: PropCache,
    nodes: Option<Nodes>,
    spawn_attempted: Option<Instant>,
    /// The last plate shown, kept while it fades out.
    last: Option<Plate>,
    // motion: pass
    /// The HUD motion pass (hud_motion_pass.rs).
    pass: pass::Pass,
}
impl StatePlate {
    fn nodes(&mut self) -> &Nodes {
        let cache = &mut self.cache;
        self.nodes.get_or_insert_with(|| Nodes {
            root: cache.node(PLATE_PATH),
            glyph: cache.node(&format!("{PLATE_PATH}.glyph")),
            key: cache.node(&format!("{PLATE_PATH}.key")),
            key_text: cache.node(&format!("{PLATE_PATH}.key.text")),
            count: cache.node(&format!("{PLATE_PATH}.count")),
        })
    }
    /// Spawn the plate (hidden, as its template is) if it is missing; false
    /// while it does not exist. One try a second.
    pub fn ensure_spawned(&mut self, ctx: &mut StableClient<'_>, log: &Logger) -> bool {
        if ctx.ui_exists(PLATE_PATH) {
            return true;
        }
        if self
            .spawn_attempted
            .is_some_and(|t| crate::clock::since(t) < Duration::from_secs(1))
        {
            return false;
        }
        self.spawn_attempted = Some(crate::clock::now());
        self.spawn(ctx, log)
    }
    fn spawn(&mut self, ctx: &mut StableClient<'_>, log: &Logger) -> bool {
        self.cache.clear();
        let spawned = ctx.ui_spawn_source("ingame", &crate::ui_theme::themed(&plate_template()));
        log.write(&format!(
            "STATE PLATE spawn={spawned} exists={}",
            ctx.ui_exists(PLATE_PATH)
        ));
        spawned && ctx.ui_exists(PLATE_PATH)
    }
    /// Per client frame. `phase`: the session phase while you play or
    /// watch (None otherwise); `snapshot`: your athlete, fresh or None.
    pub fn apply(
        &mut self,
        ctx: &mut StableClient<'_>,
        phase: Option<Phase>,
        snapshot: Option<&Snapshot>,
        log: &Logger,
    ) {
        let state = plate_state(
            phase,
            snapshot.map(|s| s.alive),
            snapshot.map_or(0, |s| s.respawn),
        );
        if phase.is_none() {
            self.last = None;
            self.pass.reset(); // motion: pass
        }
        if state.is_some() {
            self.last = state;
        }
        // motion: pass
        self.pass.frame();
        // motion: S1 (restore `let alpha = f32::from(u8::from(state.is_some()));`)
        let alpha = pass::s1_plate_fade(&mut self.pass, state.is_some());
        let shown = self.last.filter(|_| alpha > 0.);
        let Some(plate) = shown.filter(|_| !crate::ui_root::hidden()) else {
            // Write only on a change (this runs every frame).
            crate::ui_root::hide(ctx, PLATE_PATH);
            return;
        };
        if !self.ensure_spawned(ctx, log) {
            return;
        }
        crate::ui_root::show(ctx, PLATE_PATH);
        let (glyph, ink, key, count) = match plate {
            Plate::Paused => (0u8, 0xfdee00ff, true, None),
            Plate::Ai => (1, 0xeeececff, true, None),
            Plate::Dead(seconds) => (2, 0xeeececff, false, seconds),
        };
        let width = if key || count.is_some() {
            PLATE_WIDE
        } else {
            PLATE_NARROW
        };
        // The plate's own alpha (it must not need the motion module).
        let a = |color: u32| (color & !255) | ((color & 255) as f32 * alpha).round() as u32;
        let back = (240. * alpha).round() as u8;
        let n = self.nodes();
        let (root, glyph_node, key_node, key_text, count_node) =
            (n.root, n.glyph, n.key, n.key_text, n.count);
        let c = &mut self.cache;
        c.props_keyed(ctx, root, 0, (width, back), |(width, back)| {
            format!(
                "x: {}px; width: {width}px; color: #~1e1e1d{back:02x};",
                WIDTH / 2 - width / 2,
            )
        });
        c.props_keyed(
            ctx,
            glyph_node,
            0,
            (width, glyph, a(ink)),
            |(width, glyph, ink)| {
                format!(
                    "x: {}px; source: \"asset/lt_direct_control/ui/{}\"; color: #{ink:08x};",
                    if width == PLATE_WIDE {
                        PLATE_PAD_WIDE
                    } else {
                        PLATE_PAD_NARROW
                    },
                    GLYPHS[glyph as usize],
                )
            },
        );
        c.props_keyed(ctx, key_node, 0, (key, a(0xeeececff)), |(key, ink)| {
            format!("visible: {key}; color: #{ink:08x};")
        });
        c.props_keyed(ctx, key_text, 0, (a(0x393939ff),), |(ink,)| {
            format!("color: #{ink:08x};")
        });
        c.props_keyed(
            ctx,
            count_node,
            0,
            (count.is_some(), a(0xffffffff)),
            |(on, ink)| format!("visible: {on}; color: #{ink:08x};"),
        );
        c.text_keyed(ctx, count_node, (count,), |(count,)| {
            count.map_or(String::new(), |n| n.to_string())
        });
    }
}

impl crate::prewarm::Prespawn for StatePlate {
    fn built(&self, ctx: &StableClient<'_>) -> bool {
        ctx.ui_exists(PLATE_PATH)
    }
    /// The template's root is hidden.
    fn prespawn(&mut self, ctx: &mut StableClient<'_>, log: &Logger) -> bool {
        self.spawn(ctx, log)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_plate_is_centred_under_the_clock_and_clear_of_the_kill_callouts() {
        let plate = plate_template();
        assert!(plate.contains("#key:color") && plate.contains("text: \"F11\""));
        // Lower and bigger (1.3.6): still centred, still clear of the kill callouts.
        assert!(plate.contains("x: 902px; y: 100px; width: 116px; height: 48px;"));
        assert!(plate.contains("width: 36px; height: 26px;"));
        const { assert!(PLATE_Y + PLATE_H < 150) };
        const { assert!(PLATE_PAD_WIDE * 2 + PLATE_GLYPH + 10 + 36 == PLATE_WIDE) };
    }
}
