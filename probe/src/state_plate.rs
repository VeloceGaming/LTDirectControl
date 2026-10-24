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
/// The wide plate's width and its cap's width for key `text` (shown when
/// `key`): 116 and 36 at F11; a wider key widens both by the same whole
/// pixels, so the plate stays centred under the clock and the cap keeps
/// its padding.
fn wide_plate(text: &str, key: bool) -> (usize, f32) {
    let (_, _, base, _) = crate::plate_draw::KEY;
    let cap = if key {
        crate::key_cap::width(text, base, crate::plate_draw::KEY_SIZE, "F11")
    } else {
        base
    };
    (PLATE_WIDE + (cap - base) as usize, cap)
}
fn plate_template() -> String {
    // The plate, key cap and count are drawn (`plate_draw.rs`); the root
    // keeps the plate's place and visibility for the glyph, whose tint fades.
    let x = WIDTH / 2 - PLATE_WIDE / 2;
    let mut s = format!("lt_state_plate:empty {{ x: {x}px; y: {PLATE_Y}px; width: {PLATE_WIDE}px; height: {PLATE_H}px; z: 1120; visible: false; ignore_event: true;\n");
    s.push_str(&crate::ui_graphics::image(
        "glyph",
        "ef_skull",
        PLATE_PAD_WIDE,
        (PLATE_H - PLATE_GLYPH) / 2,
        PLATE_GLYPH,
        1121,
    ));
    s.push_str("}\n");
    s
}
#[cfg(test)]
pub(crate) fn plate_template_for_test() -> String {
    plate_template()
}
/// The plate's nodes in its write cache.
struct Nodes {
    root: NodeId,
    glyph: NodeId,
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
    /// This frame's commands (`plate_draw.rs`), sent in `post_render`.
    pub(crate) list: crate::draw_list::DrawList,
    /// `draw_list::check_origin`'s state.
    origin_checked: u8,
    /// The key the cap names: the start binding as bound now (1.6.0-dev4).
    key: Option<crate::key_cap::KeyText>,
}
impl StatePlate {
    fn nodes(&mut self) -> &Nodes {
        let cache = &mut self.cache;
        self.nodes.get_or_insert_with(|| Nodes {
            root: cache.node(PLATE_PATH),
            glyph: cache.node(&format!("{PLATE_PATH}.glyph")),
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
        self.list.clear();
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
        // The cap names the start key as bound now; none while unbound.
        let key_text = self
            .key
            .get_or_insert_with(|| crate::key_cap::KeyText::new("start"));
        key_text.follow(&crate::settings::current_shared(), "start");
        let bound = !key_text.text().is_empty();
        let (glyph, ink, key, count) = match plate {
            Plate::Paused => (0u8, 0xfdee00ff, bound, None),
            Plate::Ai => (1, 0xeeececff, bound, None),
            Plate::Dead(seconds) => (2, 0xeeececff, false, seconds),
        };
        let (wide, cap) = wide_plate(key_text.text(), key);
        let width = if key || count.is_some() {
            wide
        } else {
            PLATE_NARROW
        };
        // The plate's own alpha (it must not need the motion module).
        let a = |color: u32| (color & !255) | ((color & 255) as f32 * alpha).round() as u32;
        let back = (240. * alpha).round() as u8;
        let n = self.nodes();
        let (root, glyph_node) = (n.root, n.glyph);
        let c = &mut self.cache;
        c.props_keyed(ctx, root, 0, (width,), |(width,)| {
            format!("x: {}px; width: {width}px;", WIDTH / 2 - width / 2)
        });
        c.props_keyed(
            ctx,
            glyph_node,
            0,
            (width, glyph, a(ink)),
            |(width, glyph, ink)| {
                format!(
                    "x: {}px; source: \"asset/lt_direct_control/ui/{}\"; color: #{ink:08x};",
                    if width != PLATE_NARROW {
                        PLATE_PAD_WIDE
                    } else {
                        PLATE_PAD_NARROW
                    },
                    GLYPHS[glyph as usize],
                )
            },
        );
        let view = crate::plate_draw::PlateView {
            x: (WIDTH / 2 - width / 2) as f32,
            width: width as f32,
            back: crate::ui_theme::tone(0x1e1e_1d00 | u32::from(back)),
            key: key.then(|| (a(0xeeececff), a(0x393939ff))),
            cap,
            count: count.map(|n| (n, a(0xffffffff))),
        };
        let origin = ctx.ui_node_rect("ingame").map_or((0., 0.), |r| (r.0, r.1));
        let key_text = self.key.as_ref().map_or("", |k| k.text());
        crate::plate_draw::build(&view, key_text, origin, &mut self.list);
        let drawn = (origin.0 + view.x, origin.1 + crate::plate_draw::PLATE_Y);
        let root = ctx.ui_node_rect(PLATE_PATH);
        crate::draw_list::check_origin("state plate", drawn, root, &mut self.origin_checked, log);
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
        // Lower and bigger (1.3.6): still centred, still clear of the kill callouts.
        assert!(plate.contains("x: 902px; y: 100px; width: 116px; height: 48px;"));
        assert_eq!(PLATE_Y as f32, crate::plate_draw::PLATE_Y);
        assert_eq!(PLATE_H as f32, crate::plate_draw::PLATE_H);
        const { assert!(PLATE_Y + PLATE_H < 150) };
        const { assert!(PLATE_PAD_WIDE * 2 + PLATE_GLYPH + 10 + 36 == PLATE_WIDE) };
    }
    #[test]
    fn the_plate_widens_with_a_wider_start_key_and_stays_centred() {
        assert_eq!(wide_plate("F11", true), (PLATE_WIDE, 36.));
        assert_eq!(wide_plate("F9", true), (PLATE_WIDE, 36.));
        let (width, cap) = wide_plate("C+S+F11", true);
        assert!(cap > 36. && cap.fract() == 0.);
        assert_eq!(width, PLATE_WIDE + (cap - 36.) as usize);
        let plate = crate::plate_draw::PlateView {
            x: (WIDTH / 2 - width / 2) as f32,
            width: width as f32,
            back: 0x1e1e1dff,
            key: Some((0xeeececff, 0x393939ff)),
            cap,
            count: None,
        };
        let mut list = crate::draw_list::DrawList::default();
        crate::plate_draw::build(&plate, "C+S+F11", (0., 0.), &mut list);
        let rects: Vec<_> = list
            .commands()
            .iter()
            .filter_map(|c| match *c {
                crate::draw_list::Cmd::Rect { rect, .. } => Some(rect),
                _ => None,
            })
            .collect();
        // Centred plate; the cap 60 px in, 20 px clear of the right edge.
        assert!((rects[0].0 + rects[0].2 / 2. - 960.).abs() <= 0.5);
        assert_eq!(rects[1].0 + rects[1].2 + 20., rects[0].0 + rects[0].2);
    }
}
