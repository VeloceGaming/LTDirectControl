//! The HUD motion pass (experimental; design notes "HUD motion pass" and
//! "HUD motion, second pass", 2026-10-17). Every item of the notes is a
//! function here named after its letter and number (`a1_purchase_breathe`,
//! `b2_gold_roll`, `s2_pause_brackets`, ...).
//!
//! Pruning: every edit an item made outside this module carries the comment
//! `motion: <item>` (search for "motion: B5", for example); where an item
//! changed an existing expression, the comment says what to restore. To
//! prune an item, delete its functions here (and its `*_nodes` template
//! function or its part of a draw list) and every place its tag marks. Sites shared by two items name
//! both; keep the other item's part.
//!
//! | Item | Sites outside this module |
//! | --- | --- |
//! | A1 | purchase coin colours (player_hud) |
//! | A2 | `Movement::recalling`, one call in client.rs (which needs `HudUi::pass` crate-visible); drawn in hud_draw.rs |
//! | A3 | one call in the skill loop; drawn in hud_draw.rs |
//! | B1 | health trail write |
//! | B2 | gold text value |
//! | B4 | level colour, learned drain, shade write, icon brightness |
//! | B5 | item border, item icon tint, next-item opacity |
//! | B6, D1 | one call each near the top of `HudUi::apply`, drawn in hud_draw.rs; D1 now plays on kills only |
//! | B8 | session strip: sweep (drawn in session_draw.rs), portrait opacity; client.rs: swap landing |
//! | C1 | key cap colour, shade write (hover computed earlier in the skill loop) |
//! | C2 | item icon tint |
//! | D2 | minimap.rs corner points |
//! | S1 | the state plate's opacity (`StatePlate::apply`, player_hud) |
//! | S2, S7 | one call each in client.rs `post_render_inner`; S7 also `stats_panel::target_id` made crate-visible |
//! | S5 | screen_effect.rs: the strength the bands use |
//! | S6 | shop_ui.rs: fade only; settings_ui.rs: fade and sweep |
//!
//! B7 and S4 (match entrances) were removed on 2026-10-18 at the author's request.
//!
//! Dropping the whole pass: delete this module and everything tagged
//! `motion: ...`, including `pass` (the `Pass` fields of `HudUi`,
//! `StatePlate`, `SessionUi`, `ShopUi`, `SettingsUi` and `Effect`, their
//! `frame`/`reset` calls, and `Motion::seed`/`Motion::remove`, which only
//! this module uses) and `switch` (the "hud_motion" option, its +62 in the
//! settings layout test, and the "HUD motion" label and hint in the 16
//! `probe/lang` files). The state plate (S1) and the lighter death veil
//! (S3a) are features and stay: only their motion is here.
//!
//! Switch Off ("HUD motion" in Settings › Interface): every function
//! returns its input unchanged, and the ones that write their own nodes
//! write the end state (new nodes hidden, original colours), so the HUD is
//! as it was before the pass (with the state plate and the 15 % veil).
//! Turning it Off mid-animation lands on the end state.
//!
//! Motion language (second pass, for match events): beats of 300-600 ms,
//! fades 150-200 ms; ease (0.15, 0.8, 0.3, 1) for positions and wipes,
//! tonal (0.4, 0.1, 0.5, 1) for colour; ambient cycles 1-3 s. Transient
//! lines may sweep and veils may wipe; the fixed HUD never slides, scales
//! or shakes. Presentation only: no hit rectangle or input changes. The
//! tweens are `hud_motion::Motion`; the rest is timestamps fed to its two
//! curves.
//!
//! Items already present before this pass, with no function here: C3
//! (tooltips fade in over 117 ms in the HUD, the shop and the session
//! strip) and C4 (the session controls' hover tint and 83 ms press dim).
use crate::hud_motion::{easing, Motion};
use std::{collections::HashMap, sync::Mutex, time::Instant};

const TONAL: f32 = 0.1;
const FADE: f32 = 0.117;
const YELLOW: u32 = 0xfdee00ff;
/// The design sheet's `--signal-warning`.
const WARNING: u32 = 0xff642eff;
/// Post-render lines (S2, S7) sit above the HUD's nodes.
const DRAW_Z: i32 = 1130;

// ---- Shared helpers --------------------------------------------------------

/// Property writes for nodes under one root: (node, source).
pub type Writes = Vec<(String, String)>;

/// Writes through the host's property cache: unchanged fields cost nothing.
pub fn write(
    ctx: &mut mod_api_stable::StableClient<'_>,
    cache: &mut crate::ui_writes::PropCache,
    root: &str,
    writes: Writes,
) {
    for (node, source) in writes {
        crate::hud_motion::properties(ctx, cache, &format!("{root}.{node}"), &source);
    }
}

/// Per-host state shared by the items: the switch, one tween set, the last
/// value seen per item key, and event times. Keys start with the item id.
#[derive(Default)]
pub struct Pass {
    on: bool,
    motion: Motion,
    seen: HashMap<String, f64>,
    seen_text: HashMap<String, String>,
    marks: HashMap<String, Instant>,
}
impl Pass {
    /// Once per frame, before the items: read the switch.
    pub fn frame(&mut self) {
        self.set_on(crate::settings::option("hud_motion") == 1.);
    }
    fn set_on(&mut self, on: bool) {
        if !on {
            self.marks.clear();
            self.motion.reset();
        }
        self.on = on;
    }
    /// Another athlete or an inactive HUD: forget values and running items.
    pub fn reset(&mut self) {
        self.seen.clear();
        self.seen_text.clear();
        self.marks.clear();
        self.motion.reset();
    }
    /// The previous value when `value` differs from the last one seen. None
    /// the first time (nothing animates on first sight) and when unchanged.
    /// Tracked while Off too, so turning On never replays an old change.
    fn change(&mut self, key: &str, value: f64) -> Option<f64> {
        // As `insert(key, value).filter(|old| *old != value)`, without
        // making a new key every frame.
        match self.seen.get_mut(key) {
            Some(old) => Some(std::mem::replace(old, value)).filter(|old| *old != value),
            None => {
                self.seen.insert(key.into(), value);
                None
            }
        }
    }
    fn change_text(&mut self, key: &str, value: &str) -> bool {
        match self.seen_text.get_mut(key) {
            Some(old) if old == value => false,
            Some(old) => {
                value.clone_into(old);
                true
            }
            None => {
                self.seen_text.insert(key.into(), value.into());
                false
            }
        }
    }
    fn mark(&mut self, key: &str) {
        if self.on {
            self.marks.insert(key.into(), crate::clock::now());
        }
    }
    fn forget(&mut self, key: &str) {
        self.marks.remove(key);
    }
    /// Seconds since `key` was marked.
    fn age(&self, key: &str) -> Option<f32> {
        self.marks
            .get(key)
            .map(|at| crate::clock::since(at).as_secs_f32())
    }
    /// Seconds since `key` was marked, marking it now if it was not.
    fn since(&mut self, key: &str) -> f32 {
        if !self.marks.contains_key(key) {
            self.mark(key);
        }
        self.age(key).unwrap_or(0.)
    }
    /// An eased follow of `target` that starts at rest on its first value.
    fn follow(&mut self, key: &str, target: f32, seconds: f32) -> f32 {
        self.motion.seed(key, target);
        self.motion.value(key, target, seconds)
    }
}

fn ease(t: f32) -> f32 {
    easing(t.clamp(0., 1.), false)
}
fn tonal(t: f32) -> f32 {
    easing(t.clamp(0., 1.), true)
}
/// 0 → 1 → 0 over `period` seconds on the tonal curve, starting at 0.
pub fn breathe(seconds: f32, period: f32) -> f32 {
    let phase = (seconds / period).rem_euclid(1.);
    tonal(if phase < 0.5 {
        phase * 2.
    } else {
        2. - phase * 2.
    })
}
/// Full for `hold` seconds, then a tonal fade over `fade`: 1 → 0.
pub fn hold_fade(age: f32, hold: f32, fade: f32) -> f32 {
    if age < hold {
        1.
    } else {
        1. - tonal((age - hold) / fade)
    }
}
/// A changed number's brightness: 70 % at the change, 100 % after 100 ms.
pub fn lift(age: f32) -> f32 {
    0.7 + 0.3 * tonal(age / TONAL)
}
/// `color` (rrggbbaa) with its red, green and blue multiplied by `k`.
pub fn scale(color: u32, k: f32) -> u32 {
    let k = k.clamp(0., 1.);
    let channel = |shift: u32| ((((color >> shift) & 255) as f32 * k).round() as u32) << shift;
    channel(24) | channel(16) | channel(8) | (color & 255)
}
/// `color` with its alpha multiplied by `a`.
pub fn fade(color: u32, a: f32) -> u32 {
    (color & !255) | ((color & 255) as f32 * a.clamp(0., 1.)).round() as u32
}
fn mix(from: u32, to: u32, t: f32) -> u32 {
    u32::from_str_radix(&crate::hud_motion::color(from, to, t), 16).unwrap_or(from)
}
/// `v` to 0.01, as a node received it (`{v:.2}px`).
pub fn hundredths(v: f32) -> f32 {
    (v * 100.).round() / 100.
}
/// Image tint for an icon: brightness and opacity (1, 1 is untouched).
pub fn icon_tint(brightness: f32, alpha: f32) -> String {
    format!(
        "color: #{:08x};",
        fade(scale(0xffffffff, brightness), alpha)
    )
}
/// A plain node for the template, hidden until its item shows it.
fn node(name: &str, (x, y, w, h): (usize, isize, usize, usize), color: &str, z: usize) -> String {
    format!("#{name}:color {{ x: {x}px; y: {y}px; width: {w}px; height: {h}px; z: {z}; color: #{color}; visible: false; ignore_event: true; }}\n")
}
/// The post-render items (D2, S2, S7) keep their state here (client thread only).
static DRAW: Mutex<Option<Pass>> = Mutex::new(None);
fn draw_pass<T>(f: impl FnOnce(&mut Pass) -> T, otherwise: T) -> T {
    let Ok(mut pass) = DRAW.lock() else {
        return otherwise;
    };
    let pass = pass.get_or_insert_with(Pass::default);
    pass.frame();
    f(pass)
}
/// The minimap frame's corner tick (two legs joined by a 45° chamfer), `k`
/// times its size: the corner at `corner`, the legs running along `dir`
/// (±1, ±1), moved `out` px further out along both axes. Shared by S2, S7.
fn tick(
    ctx: &mut mod_api_stable::StableClient<'_>,
    (cx, cy): (f32, f32),
    (sx, sy): (f32, f32),
    k: f32,
    out: f32,
    width: f32,
    color: u32,
) {
    let point = |a: f32, b: f32| (cx + sx * (k * a - out), cy + sy * (k * b - out));
    for (a, b) in [
        ((0.75, 11.5), (0.75, 4.5)),
        ((0.75, 4.5), (4.5, 0.75)),
        ((4.5, 0.75), (11.5, 0.75)),
    ] {
        let a = point(a.0, a.1);
        let b = point(b.0, b.1);
        ctx.draw_line("UI", a.0, a.1, b.0, b.1, width, DRAW_Z, color);
    }
}
/// The four corners of a rectangle with the direction their legs run in.
fn corners((x, y, w, h): (f32, f32, f32, f32)) -> [((f32, f32), (f32, f32)); 4] {
    [
        ((x, y), (1., 1.)),
        ((x + w, y), (-1., 1.)),
        ((x, y + h), (1., -1.)),
        ((x + w, y + h), (-1., -1.)),
    ]
}

// ---- A. Ambient ------------------------------------------------------------

/// A1 Purchase slot breathe: while the next item is affordable, the
/// purchase coin and check breathe 70 % → 100 % on a 2.4 s tonal cycle.
pub fn a1_purchase_breathe(p: &mut Pass, ready: bool, color: u32) -> u32 {
    if !p.on || !ready {
        p.forget("a1");
        return color;
    }
    scale(color, 1. - 0.3 * breathe(p.since("a1"), 2.4))
}

/// A2 Recall progress: while the game reports your athlete recalling
/// (its action number 1, read from the code), the line shows and notches
/// march along it left to right, 0.3 s per pass. The client does not read
/// the recall's duration, so the line shows no progress fill.
pub fn a2_recall_progress(p: &mut Pass, recalling: bool, playing: bool) -> A2 {
    let shown = p.on && recalling && playing;
    let t = if shown {
        p.since("a2")
    } else {
        p.forget("a2");
        0.
    };
    A2 {
        shown,
        x: std::array::from_fn(|k| 702. + 17. * ((t / 0.3 + k as f32 * 0.5).rem_euclid(1.))),
    }
}
/// A2's nodes under the HUD root before 1.6.0-perf-dev3 (now drawn: a
/// 3 px line under the recall icon and two notches marching along it).
#[cfg(test)]
pub const A2_NODES: [&str; 3] = ["a2_recall_line", "a2_recall_notch0", "a2_recall_notch1"];
/// What A2 shows this frame.
#[derive(Clone, Copy)]
pub struct A2 {
    shown: bool,
    x: [f32; 2],
}
impl A2 {
    #[cfg(test)]
    fn line((shown,): (bool,)) -> String {
        format!("visible: {shown};")
    }
    #[cfg(test)]
    fn notch((shown, x): (bool, f32)) -> String {
        format!("visible: {shown}; x: {x:.2}px;")
    }
    /// For the draw list: the line shown, the notches' x (to 0.01 px, as
    /// the nodes received them).
    pub fn view(self) -> (bool, [f32; 2]) {
        (self.shown, self.x.map(hundredths))
    }
    #[cfg(test)]
    fn sources(self) -> Writes {
        let mut w = vec![(A2_NODES[0].into(), Self::line((self.shown,)))];
        for k in 0..2 {
            w.push((A2_NODES[k + 1].into(), Self::notch((self.shown, self.x[k]))));
        }
        w
    }
}

/// A3 Drain-edge comet: a second edge line at 60 % alpha follows the drain
/// edge on the ease curve over 150 ms (an eased follow, not an exact delay),
/// in the edge's warming colour. Hidden with the edge and during the ready
/// cue.
pub fn a3_drain_comet(p: &mut Pass, slot: usize, shown: bool, y: f32, color: u32) -> A3 {
    let key = A3_KEYS[slot];
    if !p.on || !shown {
        p.motion.remove(key);
        return A3(None);
    }
    A3(Some((p.follow(key, y, 0.15), color)))
}
const A3_KEYS: [&str; 3] = ["a3_0", "a3_1", "a3_2"];
/// A3's node in each skill slot before 1.6.0-perf-dev3 (now drawn).
#[cfg(test)]
pub const A3_NODES: [&str; 3] = ["skill0.a3_comet", "skill1.a3_comet", "skill2.a3_comet"];
/// What one slot's A3 comet shows: its y and the edge colour, or hidden.
#[derive(Clone, Copy)]
pub struct A3(Option<(f32, u32)>);
impl A3 {
    #[cfg(test)]
    fn source((shown, y, color): (bool, f32, u32)) -> String {
        if !shown {
            return "visible: false;".into();
        }
        format!(
            "visible: true; y: {y:.2}px; color: #{:08x};",
            fade(color, 0.6)
        )
    }
    #[cfg(test)]
    fn key(self) -> (bool, f32, u32) {
        self.0.map_or((false, 0., 0), |(y, c)| (true, y, c))
    }
    /// For the draw list: the comet's y and colour (at 60 %), or hidden.
    pub fn view(self) -> Option<(f32, u32)> {
        self.0.map(|(y, color)| (hundredths(y), fade(color, 0.6)))
    }
    #[cfg(test)]
    fn sources(self) -> Writes {
        vec![(A3_NODES[0].into(), Self::source(self.key()))]
    }
}

// ---- B. Event-driven -------------------------------------------------------

/// B1 Damage chunk: when your health drops, the health trail (what lingers
/// between the fill and the old health) takes the full warning colour for
/// 300 ms, then blends back into its own colour over the remaining 200 ms
/// of the trail's 500 ms. Every new drop restarts it. `health`: the target
/// health in percent (not the eased fill).
pub fn b1_damage_chunk(p: &mut Pass, health: f32, color: u32) -> u32 {
    if p.change("b1", f64::from(health))
        .is_some_and(|old| f64::from(health) < old)
    {
        p.mark("b1");
    }
    match p.age("b1").filter(|age| p.on && *age < 0.5) {
        Some(age) => mix(color, WARNING, hold_fade(age, 0.3, 0.2)),
        None => {
            p.forget("b1");
            color
        }
    }
}

/// B2 Gold roll: the shown gold counts to the new value over 400 ms ease,
/// whole gold each frame. One tween key.
pub fn b2_gold_roll(p: &mut Pass, gold: usize) -> usize {
    if !p.on {
        return gold;
    }
    p.follow("b2", gold as f32, 0.4).round().max(0.) as usize
}

/// B4 Level-up, the numeral: it shows at 70 % of its colour and lifts to
/// full over 100 ms tonal.
pub fn b4_level_lift(p: &mut Pass, level: usize, color: u32) -> u32 {
    if p.change("b4_level", level as f64).is_some() {
        p.mark("b4_level");
    }
    match p.age("b4_level").filter(|_| p.on) {
        Some(age) => scale(color, lift(age)),
        None => color,
    }
}
/// How long a newly learned slot's shade takes to drain away.
const B4_DRAIN: f32 = 0.3;
/// B4 Level-up, the newly learned slot: its shade drains away over 300 ms
/// ease instead of vanishing. Returns the shade's height while draining,
/// and true once when the drain ends, when the 1.3.1 ready cue takes over
/// (a learned slot was never cooling, so that cue would not play by itself).
pub fn b4_learned_drain(p: &mut Pass, slot: usize, learned: bool) -> (Option<f32>, bool) {
    let key = ["b4_slot0", "b4_slot1", "b4_slot2"][slot];
    if p.change(key, f64::from(u8::from(learned))) == Some(0.) {
        p.mark(key);
    }
    let Some(age) = p.age(key).filter(|_| p.on && learned) else {
        p.forget(key);
        return (None, false);
    };
    if age >= B4_DRAIN {
        p.forget(key);
        return (None, true);
    }
    (Some(78. * (1. - ease(age / B4_DRAIN))), false)
}

/// B5 Item bought: a new item's icon fades in over 150 ms and its slot
/// edge is signal yellow for 300 ms, then fades over 200 ms tonal. Returns
/// (icon opacity, edge yellow 0 to 1).
pub fn b5_item_bought(p: &mut Pass, slot: usize, item: Option<&str>) -> (f32, f32) {
    const KEYS: [&str; 8] = [
        "b5_slot0", "b5_slot1", "b5_slot2", "b5_slot3", "b5_slot4", "b5_slot5", "b5_slot6",
        "b5_slot7",
    ];
    let owned;
    let key = match KEYS.get(slot) {
        Some(key) => *key,
        None => {
            owned = format!("b5_slot{slot}");
            &owned
        }
    };
    if p.change_text(key, item.unwrap_or("")) && item.is_some() {
        p.mark(key);
    }
    match p.age(key).filter(|_| p.on) {
        Some(age) if age < 0.5 => (tonal(age / 0.15), hold_fade(age, 0.3, 0.2)),
        _ => {
            p.forget(key);
            (1., 0.)
        }
    }
}
/// B5 Item bought, the purchase slot: a new "next item" fades in over
/// 100 ms tonal. The slot has one image, so this is a fade-in of the new
/// icon rather than a cross-fade from the old one.
pub fn b5_next_item(p: &mut Pass, source: Option<&str>) -> f32 {
    if p.change_text("b5_next", source.unwrap_or("")) && source.is_some() {
        p.mark("b5_next");
    }
    match p.age("b5_next").filter(|_| p.on) {
        Some(age) => tonal(age / TONAL),
        None => 1.,
    }
}

/// B6 Death and respawn: a dark veil over each skill slot, over its icon
/// but under its cooldown number, key and lock (z 1004, the slot's shade;
/// drawn in hud_draw.rs). Like League, death darkens only the skills, not
/// the strip, items or panels (the author, 2026-10-20, with League
/// screenshots; a whole-strip veil before 1.4.9).
pub const B6_Z: i32 = 1004;
/// B6's veils darken and lift over this long; the skill slots lift this far apart.
const B6_DIM: f32 = 0.4;
const B6_STEP: f32 = 0.06;
/// How dark the B6 veils are (0 to 1) `age` seconds after death or respawn.
fn b6_dim(dead: bool, age: Option<f32>, delay: f32) -> f32 {
    match (dead, age) {
        (true, Some(age)) => tonal(age / B6_DIM),
        (true, None) => 1.,
        (false, Some(age)) => 1. - tonal((age - delay) / B6_DIM),
        (false, None) => 0.,
    }
}
/// B6 Death and respawn: the skill slots dim over 400 ms tonal (a 50 %
/// black veil, which multiplies the skills' 75 % cooling brightness instead
/// of replacing it). On respawn they lift back over 400 ms, 60 ms apart,
/// left to right. The state plate's fade is S1.
pub fn b6_death_dim(p: &mut Pass, alive: Option<bool>, playing: bool) -> B6 {
    let shown = p.on && playing && alive.is_some();
    let dead = alive == Some(false);
    match p.change("b6_alive", f64::from(u8::from(!dead))) {
        Some(1.) => {
            p.forget("b6_respawn");
            p.mark("b6_died");
        }
        Some(_) => {
            p.forget("b6_died");
            p.mark("b6_respawn");
        }
        None => {}
    }
    let age = p.age(if dead { "b6_died" } else { "b6_respawn" });
    B6(std::array::from_fn(|i| {
        if shown {
            b6_dim(dead, age, i as f32 * B6_STEP)
        } else {
            0.
        }
    }))
}
/// B6's veil over each skill slot before 1.6.0-perf-dev3 (now drawn).
#[cfg(test)]
pub const B6_NODES: [&str; 3] = ["b6_veil_skill0", "b6_veil_skill1", "b6_veil_skill2"];
/// Each skill slot's veil strength this frame (0 to 1).
#[derive(Clone, Copy)]
pub struct B6([f32; 3]);
impl B6 {
    #[cfg(test)]
    fn veil((a,): (f32,)) -> String {
        format!(
            "visible: {}; color: #000000{:02x};",
            a > 0.,
            (128. * a).round() as u8
        )
    }
    /// For the draw list: each veil shown, and its colour (black at up to
    /// 50 %).
    pub fn view(self) -> [(bool, u32); 3] {
        self.0
            .map(|a| (a > 0., u32::from((128. * a).round() as u8)))
    }
    #[cfg(test)]
    fn sources(self) -> Writes {
        (0..3)
            .map(|i| (B6_NODES[i].into(), Self::veil((self.0[i],))))
            .collect()
    }
}

static B8_LANDED: Mutex<Option<Instant>> = Mutex::new(None);
/// B8, the swap landing: called where a champion swap succeeds.
pub fn b8_swap_landed() {
    if let Ok(mut landed) = B8_LANDED.lock() {
        *landed = Some(crate::clock::now());
    }
}
const B8_STEP: f32 = 0.05;
const B8_SWEEP: f32 = 0.3;
/// B8 Swap stagger, the column: while it unfolds, the portrait images
/// appear 50 ms apart, nearest the button (support) first. Returns the
/// portrait's opacity; the tiles and their clicks are untouched.
pub fn b8_swap_stagger(p: &mut Pass, lane: usize, open: bool) -> f32 {
    if p.change("b8_open", f64::from(u8::from(open))) == Some(0.) {
        p.mark("b8_open");
    }
    match p.age("b8_open").filter(|_| p.on && open) {
        Some(age) if age < (4 - lane.min(4)) as f32 * B8_STEP => 0.,
        _ => 1.,
    }
}
/// B8 Swap stagger, the landing: a yellow hairline sweeps the swap
/// button's bottom edge over 300 ms ease, then fades over 117 ms. The
/// column folds away as the swap lands, so the button carries the sweep
/// rather than the chosen portrait.
pub fn b8_swap_sweep(p: &Pass, in_match: bool, width: f32) -> B8 {
    let age = B8_LANDED
        .lock()
        .ok()
        .and_then(|l| *l)
        .map(|at| crate::clock::since(at).as_secs_f32())
        .filter(|age| p.on && in_match && *age < B8_SWEEP + FADE);
    B8(age.map(|age| {
        (
            width * ease(age / B8_SWEEP),
            fade(YELLOW, 1. - tonal((age - B8_SWEEP) / FADE)),
        )
    }))
}
/// B8's sweep line under the session strip's root before 1.6.0-perf-dev3
/// (now drawn: a 2 px hairline on the swap button's bottom edge).
#[cfg(test)]
pub const B8_NODE: &str = "b8_sweep";
/// The sweep line's width and colour this frame, or hidden.
#[derive(Clone, Copy)]
pub struct B8(Option<(f32, u32)>);
impl B8 {
    #[cfg(test)]
    fn source((shown, width, color): (bool, f32, u32)) -> String {
        if !shown {
            return "visible: false;".into();
        }
        format!("visible: true; width: {width:.2}px; color: #{color:08x};")
    }
    #[cfg(test)]
    fn key(self) -> (bool, f32, u32) {
        self.0.map_or((false, 0., 0), |(w, c)| (true, w, c))
    }
    /// For the draw list: the sweep's width and colour, when shown.
    pub fn view(self) -> Option<(f32, u32)> {
        self.0.map(|(width, color)| (hundredths(width), color))
    }
    #[cfg(test)]
    fn sources(self) -> Writes {
        vec![(B8_NODE.into(), Self::source(self.key()))]
    }
}

// ---- C. Interaction --------------------------------------------------------

/// C1 Slot hover, skills: while the pointer is over a skill slot its dark
/// shade lightens and its key cap brightens, 100 ms tonal. Returns the
/// hover amount (0 to 1); item slots already responded to hover.
pub fn c1_slot_hover(p: &mut Pass, slot: usize, hover: bool) -> f32 {
    if !p.on {
        return 0.;
    }
    p.motion
        .tonal(["c1_0", "c1_1", "c1_2"][slot], f32::from(hover), TONAL)
}

/// C2 Slot press, items: a pressed item slot's icon dims to 85 % over
/// 83 ms (skill icons already did). Returns the icon brightness.
pub fn c2_item_press(p: &Pass, press: f32) -> f32 {
    if !p.on {
        return 1.;
    }
    1. - 0.15 * press
}

// ---- D. Decorative signatures ----------------------------------------------

/// D1's line runs the strip's full width.
const D1_WIDTH: f32 = 1560.;
const D1_SWEEP: f32 = 0.5;
/// D1 Stats hairline: a 2 px yellow line sweeps left to right across the
/// whole strip (1560 px) once per kill of your athlete, 500 ms ease,
/// then is gone. No match-start trigger.
pub fn d1_stats_hairline(p: &mut Pass, kills: Option<usize>, playing: bool) -> D1 {
    if let Some(kills) = kills {
        if p.change("d1_kills", kills as f64)
            .is_some_and(|old| kills as f64 > old)
        {
            p.mark("d1");
        }
    }
    let age = p.age("d1").filter(|age| p.on && playing && *age < D1_SWEEP);
    if age.is_none() {
        p.forget("d1");
    }
    D1(age.map(|age| D1_WIDTH * ease(age / D1_SWEEP)))
}
/// D1's line under the HUD root before 1.6.0-perf-dev3 (now drawn: 2 px,
/// just under the K/D/A and CS row).
#[cfg(test)]
pub const D1_NODE: &str = "d1_line";
/// The hairline's width this frame, or hidden.
#[derive(Clone, Copy)]
pub struct D1(Option<f32>);
impl D1 {
    #[cfg(test)]
    fn source((shown, width): (bool, f32)) -> String {
        if !shown {
            return "visible: false;".into();
        }
        format!("visible: true; width: {width:.2}px;")
    }
    #[cfg(test)]
    fn key(self) -> (bool, f32) {
        self.0.map_or((false, 0.), |w| (true, w))
    }
    /// For the draw list: the hairline's width, when shown.
    pub fn view(self) -> Option<f32> {
        self.0.map(hundredths)
    }
    #[cfg(test)]
    fn sources(self) -> Writes {
        vec![(D1_NODE.into(), Self::source(self.key()))]
    }
}

/// Camera centre changes up to this much per frame count as still (a guess:
/// it only has to ignore rounding drift).
const D2_STILL: f64 = 0.5;
/// D2 Minimap ticks settle: the minimap frame's corner ticks sit 2 px out
/// while the camera moves and settle to place over 167 ms ease when it
/// stops. "Moves" is a change of the camera frame's centre between frames
/// (the client reads no pan velocity); with camera lock on, that is
/// whenever your athlete walks. Returns the distance out.
pub fn d2_minimap_ticks(center: (f32, f32)) -> f32 {
    draw_pass(
        |p| {
            let step = |p: &mut Pass, key: &str, v: f32| {
                p.change(key, f64::from(v))
                    .is_some_and(|old| (old - f64::from(v)).abs() > D2_STILL)
            };
            let moved_x = step(p, "d2_x", center.0);
            let moved_y = step(p, "d2_y", center.1);
            let moved = moved_x || moved_y;
            if !p.on {
                return 0.;
            }
            2. * p.motion.value("d2", f32::from(moved), 0.167)
        },
        0.,
    )
}

// ---- S. Second pass (design note "HUD motion, second pass") ---------------

const S1_FADE: f32 = 0.15;
/// S1 State plate, the fade (the plate itself is a feature in player_hud.rs,
/// shown with the switch Off too): it fades in and out over 150 ms tonal,
/// the B6 banner fade lengthened. Returns its opacity; Off: 1 or 0.
pub fn s1_plate_fade(p: &mut Pass, shown: bool) -> f32 {
    let target = f32::from(u8::from(shown));
    if !p.on {
        return target;
    }
    p.motion.tonal("s1", target, S1_FADE)
}

/// S2's brackets sit this far in from the screen's edges.
const S2_INSET: f32 = 16.;
const S2_FADE: f32 = 0.2;
/// S2 Pause brackets: while the match is paused, four signal-yellow corner
/// brackets 16 px in from the screen's corners: the minimap frame's tick at
/// twice its size (legs 23 px), 2 px wide. They fade in over 200 ms tonal
/// and out on resume; steady while paused. Drawn lines, so they take no
/// clicks. Called after rendering.
pub fn s2_pause_brackets(ctx: &mut mod_api_stable::StableClient<'_>, paused: bool) {
    let alpha = draw_pass(
        |p| {
            if p.on {
                p.motion.tonal("s2", f32::from(u8::from(paused)), S2_FADE)
            } else {
                0.
            }
        },
        0.,
    );
    if alpha <= 0. {
        return;
    }
    let (x, y, w, h) = ctx.ui_node_rect("ingame").unwrap_or((0., 0., 1920., 1080.));
    let inset = (
        x + S2_INSET,
        y + S2_INSET,
        w - 2. * S2_INSET,
        h - 2. * S2_INSET,
    );
    for (corner, dir) in corners(inset) {
        tick(ctx, corner, dir, 2., 0., 2., fade(YELLOW, alpha));
    }
}

/// S5's breathing cycle at a severity (0.35 at the low-health threshold, 1
/// at no health): 3 s at the threshold down to 1 s at near-zero health.
/// The note's `3 - 2 x severity` applied to the severity's own 0.35-1
/// band, so both named end points hold.
pub fn s5_cycle(severity: f32) -> f32 {
    3. - 2. * ((severity - 0.35) / 0.65).clamp(0., 1.)
}
/// S5 Low-health heartbeat (replaces A4): the existing edge tint breathes
/// 70 % → 100 % of its strength on a cycle that shortens as health falls
/// (`s5_cycle`). The phase advances by each frame's share of the current
/// cycle, so a changing cycle never jumps. The tint's own setting still
/// decides whether it shows.
pub fn s5_low_health_heartbeat(p: &mut Pass, strength: f32) -> f32 {
    if !p.on || strength <= 0. {
        p.forget("s5_tick");
        p.seen.remove("s5_phase");
        return strength;
    }
    let dt = p.age("s5_tick").unwrap_or(0.);
    p.mark("s5_tick");
    let phase = p.seen.get("s5_phase").copied().unwrap_or(0.) as f32;
    let phase = (phase + dt / s5_cycle(strength)).rem_euclid(1.);
    p.seen.insert("s5_phase".into(), f64::from(phase));
    strength * (1. - 0.3 * breathe(phase, 1.))
}

const S6_FADE: f32 = 0.15;
const S6_SWEEP: f32 = 0.3;
const S6_LINE_FADE: f32 = 0.1;
/// A window hidden and shown again within this long does not replay.
const S6_REOPEN: f32 = 0.2;
/// S6 Window entrance, template nodes inside a window's root: a veil in
/// the background colour over the whole window and the hairline on its top
/// edge, above everything else in it (`z`).
pub fn s6_nodes((w, h): (usize, usize), z: usize) -> String {
    let bg = crate::ui_theme::hex(0xff);
    let mut s = node("s6_veil", (0, 0, w, h), &bg, z);
    s.push_str(&node("s6_line", (0, 0, 0, 2), "fdee00ff", z + 1));
    s
}
/// Shop opening keeps the fade, without a progress-like sweeping line.
pub fn s6_shop_nodes((w, h): (usize, usize), z: usize) -> String {
    node("s6_veil", (0, 0, w, h), &crate::ui_theme::hex(0xff), z)
}
pub fn s6_shop_entrance(p: &mut Pass, shown: bool) -> Writes {
    window_entrance(p, shown, None)
}
/// S6 Settings entrance: the frame a
/// window becomes visible, its content fades in over 150 ms out of the
/// window's own colour (the game's UI has no group opacity, so a veil in
/// the background colour fades out), while a 2 px signal-yellow
/// hairline sweeps its top edge over 300 ms ease, then fades over 100 ms.
/// Once per show: not on a page change, and not when the window was hidden
/// less than 200 ms ago. Call every frame with whether the window shows;
/// write the result only while it does.
pub fn s6_window_entrance(p: &mut Pass, shown: bool, width: f32) -> Writes {
    window_entrance(p, shown, Some(width))
}
fn window_entrance(p: &mut Pass, shown: bool, width: Option<f32>) -> Writes {
    match p.change("s6_shown", f64::from(u8::from(shown))) {
        Some(0.) if p.age("s6_hidden").is_none_or(|age| age >= S6_REOPEN) => p.mark("s6"),
        Some(1.) => p.mark("s6_hidden"),
        _ => {}
    }
    let age = p.age("s6").filter(|age| {
        p.on && shown
            && *age
                < if width.is_some() {
                    S6_SWEEP + S6_LINE_FADE
                } else {
                    S6_FADE
                }
    });
    if age.is_none() {
        p.forget("s6");
    }
    let Some(age) = age else {
        let mut writes = vec![("s6_veil".into(), "visible: false;".into())];
        if width.is_some() {
            writes.push(("s6_line".into(), "visible: false;".into()));
        }
        return writes;
    };
    let veil = 1. - tonal(age / S6_FADE);
    let mut writes = vec![(
        "s6_veil".into(),
        format!(
            "visible: {}; color: #{:08x};",
            veil > 0.,
            fade(crate::ui_theme::rgba(0xff), veil)
        ),
    )];
    if let Some(width) = width {
        writes.push((
            "s6_line".into(),
            format!(
                "visible: true; width: {:.2}px; color: #{:08x};",
                width * ease(age / S6_SWEEP),
                fade(YELLOW, 1. - tonal((age - S6_SWEEP) / S6_LINE_FADE))
            ),
        ));
    }
    writes
}

const S7_OUT: f32 = 10.;
const S7_CONVERGE: f32 = 0.3;
const S7_FADE: f32 = 0.15;
/// S7 bracket motion `age` seconds after a selection: (distance out,
/// opacity). In from 10 px out over 300 ms ease, then a 150 ms fade.
fn s7_motion(age: f32) -> (f32, f32) {
    (
        S7_OUT * (1. - ease(age / S7_CONVERGE)),
        1. - tonal((age - S7_CONVERGE) / S7_FADE),
    )
}
/// S7 Target panel brackets (B9 moved here): when the selected unit
/// changes, four corner brackets (the minimap frame's tick, 1.5 px,
/// `#817c77`) converge on the top-left target panel's corners, then fade.
/// `panel`: the panel's screen rectangle while it shows. Drawn lines, so
/// they take no clicks. Called after rendering.
pub fn s7_target_brackets(
    ctx: &mut mod_api_stable::StableClient<'_>,
    panel: Option<(f32, f32, f32, f32)>,
    target: Option<usize>,
) {
    let motion = draw_pass(
        |p| {
            if p.change("s7", target.map_or(-1., |id| id as f64)).is_some() && target.is_some() {
                p.mark("s7");
            }
            let age = p
                .age("s7")
                .filter(|age| p.on && *age < S7_CONVERGE + S7_FADE);
            if age.is_none() {
                p.forget("s7");
            }
            age.map(s7_motion)
        },
        None,
    );
    let (Some((out, alpha)), Some(panel)) = (motion, panel) else {
        return;
    };
    for (corner, dir) in corners(panel) {
        tick(ctx, corner, dir, 1., out, 1.5, fade(0x817c77ff, alpha));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn on() -> Pass {
        let mut p = Pass::default();
        p.set_on(true);
        p
    }
    fn off() -> Pass {
        let mut p = Pass::default();
        p.set_on(false);
        p
    }
    #[test]
    fn shared_timings_follow_the_motion_language() {
        // Breathing: starts at rest, peaks at half the period, stays in 0..1.
        assert_eq!(breathe(0., 2.4), 0.);
        assert!((breathe(1.2, 2.4) - 1.).abs() < 1e-4);
        assert!((breathe(2.4, 2.4)).abs() < 1e-4);
        for i in 0..100 {
            let b = breathe(i as f32 * 0.07, 3.);
            assert!((0. ..=1.).contains(&b));
        }
        // Held at full, then down.
        assert_eq!(hold_fade(0.25, 0.3, 0.2), 1.);
        assert_eq!(hold_fade(0.5, 0.3, 0.2), 0.);
        // The lift: 70 % at the change, full after 100 ms.
        assert!((lift(0.) - 0.7).abs() < 1e-5);
        assert_eq!(lift(TONAL), 1.);
        assert_eq!(scale(0xff642eff, 0.5), 0x803217ff);
        assert_eq!(fade(0xfdee00ff, 0.4), 0xfdee0066);
        assert_eq!(icon_tint(1., 1.), "color: #ffffffff;");
    }
    #[test]
    fn b6_dims_on_death_and_lifts_the_skills_60_ms_apart() {
        assert_eq!(b6_dim(true, Some(0.), 0.), 0.);
        assert_eq!(b6_dim(true, Some(0.4), 0.), 1.);
        assert_eq!(b6_dim(true, None, 0.), 1.);
        assert_eq!(b6_dim(false, Some(0.06), 0.06), 1.);
        assert_eq!(b6_dim(false, Some(0.46), 0.06), 0.);
        assert!(b6_dim(false, Some(0.2), 0.12) > b6_dim(false, Some(0.2), 0.));
        assert_eq!(b6_dim(false, None, 0.), 0.);
    }
    #[test]
    fn first_sight_never_animates_and_only_a_change_does() {
        let mut p = on();
        assert_eq!(b4_level_lift(&mut p, 4, 0xffffffff), 0xffffffff);
        assert_ne!(b4_level_lift(&mut p, 5, 0xffffffff), 0xffffffff);
        // Gold starts at rest on its first value.
        assert_eq!(b2_gold_roll(&mut p, 500), 500);
        // Learning a slot at first sight (match start, swap) drains nothing.
        assert_eq!(b4_learned_drain(&mut p, 2, true), (None, false));
        let mut q = on();
        b4_learned_drain(&mut q, 2, false);
        let (height, cue) = b4_learned_drain(&mut q, 2, true);
        assert!(height.is_some_and(|h| h > 70.) && !cue);
        // An item appearing in an empty slot fades in; the first sight does not.
        assert_eq!(b5_item_bought(&mut p, 0, Some("a")), (1., 0.));
        b5_item_bought(&mut p, 1, None);
        let (alpha, edge) = b5_item_bought(&mut p, 1, Some("b"));
        assert!(alpha < 0.05 && edge == 1.);
        // B1: a drop turns the trail the warning colour; healing does not.
        let mut b = on();
        assert_eq!(b1_damage_chunk(&mut b, 80., 0xefe6cfff), 0xefe6cfff);
        assert_eq!(b1_damage_chunk(&mut b, 90., 0xefe6cfff), 0xefe6cfff);
        assert_eq!(b1_damage_chunk(&mut b, 60., 0xefe6cfff), WARNING);
        // D1: the first kill sweeps, a death count staying the same does not.
        let mut d = on();
        d1_stats_hairline(&mut d, Some(0), true);
        assert_eq!(
            d1_stats_hairline(&mut d, Some(0), true).sources()[0].1,
            "visible: false;"
        );
        assert!(d1_stats_hairline(&mut d, Some(1), true).sources()[0]
            .1
            .starts_with("visible: true"));
    }
    #[test]
    fn off_returns_every_input_unchanged() {
        let mut p = off();
        assert_eq!(a1_purchase_breathe(&mut p, true, 0xfdee00ff), 0xfdee00ff);
        assert_eq!(s5_low_health_heartbeat(&mut p, 0.6), 0.6);
        b1_damage_chunk(&mut p, 90., 0xefe6cfff);
        assert_eq!(b1_damage_chunk(&mut p, 40., 0xefe6cfff), 0xefe6cfff);
        assert_eq!(b2_gold_roll(&mut p, 10), 10);
        assert_eq!(b2_gold_roll(&mut p, 900), 900);
        b4_level_lift(&mut p, 2, 0xffffffff);
        assert_eq!(b4_level_lift(&mut p, 3, 0xffffffff), 0xffffffff);
        b4_learned_drain(&mut p, 1, false);
        assert_eq!(b4_learned_drain(&mut p, 1, true), (None, false));
        b5_item_bought(&mut p, 0, None);
        assert_eq!(b5_item_bought(&mut p, 0, Some("x")), (1., 0.));
        b5_next_item(&mut p, Some("a"));
        assert_eq!(b5_next_item(&mut p, Some("b")), 1.);
        b8_swap_stagger(&mut p, 0, false);
        assert_eq!(b8_swap_stagger(&mut p, 0, true), 1.);
        assert_eq!(c1_slot_hover(&mut p, 0, true), 0.);
        assert_eq!(c2_item_press(&p, 1.), 1.);
        // The plate switches.
        assert_eq!(s1_plate_fade(&mut p, true), 1.);
        assert_eq!(s1_plate_fade(&mut p, false), 0.);
    }
    #[test]
    fn off_writes_the_end_state_of_every_new_node() {
        let mut p = off();
        s6_window_entrance(&mut p, false, 1360.);
        for (_, source) in a2_recall_progress(&mut p, true, true)
            .sources()
            .into_iter()
            .chain(a3_drain_comet(&mut p, 0, true, 30., 0xffffffff).sources())
            .chain(b8_swap_sweep(&p, true, 56.).sources())
            .chain(d1_stats_hairline(&mut p, Some(3), true).sources())
            .chain(s6_window_entrance(&mut p, true, 1360.))
        {
            assert!(source.starts_with("visible: false"), "{source}");
        }
        b6_death_dim(&mut p, Some(true), true);
        for (_, source) in b6_death_dim(&mut p, Some(false), true).sources() {
            assert!(source.starts_with("visible: false"), "{source}");
        }
    }
    #[test]
    fn death_darkens_only_the_skill_slots_under_their_cooldown_numbers() {
        // Under the slot's cooldown number, key and lock (z 1005 and up).
        assert_eq!(B6_Z, 1004);
        let mut p = Pass::default();
        p.set_on(true);
        b6_death_dim(&mut p, Some(true), true);
        let b6 = b6_death_dim(&mut p, Some(false), true);
        let writes = b6.sources();
        assert_eq!(writes.len(), 3);
        assert!(writes.iter().all(|(id, _)| id.starts_with("b6_veil_skill")));
        // The drawn veils carry what the nodes received.
        for ((_, source), (shown, color)) in writes.iter().zip(b6.view()) {
            assert_eq!(source, &format!("visible: {shown}; color: #{color:08x};"));
        }
    }
    #[test]
    fn turning_on_late_replays_nothing() {
        let mut p = off();
        b4_level_lift(&mut p, 1, 0xffffffff);
        b4_level_lift(&mut p, 2, 0xffffffff);
        p.set_on(true);
        assert_eq!(b4_level_lift(&mut p, 2, 0xffffffff), 0xffffffff);
    }
    #[test]
    fn s5_heartbeat_quickens_from_3_s_at_the_threshold_to_1_s_at_no_health() {
        assert_eq!(s5_cycle(0.35), 3.);
        assert_eq!(s5_cycle(1.), 1.);
        assert_eq!(s5_cycle(0.), 3.);
        assert!((s5_cycle(0.675) - 2.).abs() < 1e-5);
        // Depth 70-100 %: the first frame is at full strength.
        let mut p = on();
        assert_eq!(s5_low_health_heartbeat(&mut p, 0.8), 0.8);
        for _ in 0..20 {
            let s = s5_low_health_heartbeat(&mut p, 0.8);
            assert!((0.56 - 1e-4..=0.8).contains(&s));
        }
    }
    #[test]
    fn s6_plays_once_per_show_and_not_on_a_quick_reopen() {
        let mut p = on();
        s6_window_entrance(&mut p, false, 1360.);
        let shown = s6_window_entrance(&mut p, true, 1360.);
        assert!(shown[1].1.starts_with("visible: true"));
        // Still shown (a page change): it runs on, it does not restart.
        let at = p.age("s6");
        s6_window_entrance(&mut p, true, 1360.);
        assert!(p.age("s6") >= at);
        // Hidden and shown again at once: no restart.
        p.forget("s6");
        s6_window_entrance(&mut p, false, 1360.);
        s6_window_entrance(&mut p, true, 1360.);
        assert!(p.age("s6").is_none());
    }
    #[test]
    fn shop_opening_has_only_a_fade_in_every_state() {
        for mut p in [on(), off()] {
            assert!(!s6_shop_nodes((1360, 850), 1590).contains("s6_line"));
            for shown in [false, true, true, false] {
                let writes = s6_shop_entrance(&mut p, shown);
                assert_eq!(writes.len(), 1);
                assert_eq!(writes[0].0, "s6_veil");
            }
        }
    }
    #[test]
    fn s7_converges_from_10_px_then_fades() {
        assert_eq!(s7_motion(0.), (10., 1.));
        assert_eq!(s7_motion(S7_CONVERGE), (0., 1.));
        assert_eq!(s7_motion(S7_CONVERGE + S7_FADE).1, 0.);
    }
}
