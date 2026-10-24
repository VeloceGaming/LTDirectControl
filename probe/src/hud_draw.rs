//! The HUD strip's draw list (1.6.0-perf-dev3).
//!
//! `HudView` holds what the strip's drawn elements show, field by field as
//! their nodes held it: `player_hud.rs` sets a field exactly where it used
//! to write the node, and a field it does not set this frame keeps its last
//! value, as a node kept its last write (the defaults are the old
//! template's). [`build`] turns the view into commands, in the old
//! template's node order, at the old nodes' z values.
//!
//! What stays in the tree under the strip's root: the skill icons and their
//! PNG twins, the item icons, the next purchase's icon (all sprite-sheet
//! images), their containers (`skill{i}`, `item{i}`, `next`: same place and
//! visibility, nothing drawn), the purchase coin and check (their tint
//! breathes, A1) and the tooltip (wrapped, scrolled, rich text).
use crate::draw_list::{anchored, at, Align, DrawList, Rect, TextStyle};
use crate::drawn_glyphs::asset;

/// The strip's root: 1920 x 146, bottom-centred in `ingame`.
pub const ROOT: (f32, f32) = (1920., 146.);
pub const SKILL_X: f32 = 832.;
pub const SKILL_Y: f32 = 4.;
pub const SKILL_STEP: f32 = 88.;
pub const NEXT: (f32, f32) = (1390., 100.);
pub const SHOP_HIT: Rect = (1386., 90., 148., 52.);
pub const COMBAT: Rect = (832., 4., 256., 80.);
pub const STRIP: Rect = (0., 90., 1560., 56.);
pub const FEEDBACK: Rect = (700., -42., 520., 30.);
pub const DENIED_BANDS: usize = 10;
pub const CHARGE_MARKS: usize = 6;
pub const HEALTH_TICKS: usize = 32;
pub const CHARGE_MARK: (f32, f32, f32) = (64., 10., 8.);

/// A label: shown, its text and its colour.
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub shown: bool,
    pub text: String,
    pub color: u32,
}
impl Label {
    fn new(color: u32) -> Self {
        Self {
            shown: true,
            text: String::new(),
            color,
        }
    }
    /// Set the text without allocating once the label has held as much.
    pub fn set(&mut self, text: &str) {
        if self.text != text {
            self.text.clear();
            self.text.push_str(text);
        }
    }
    /// Set the text from a format (`label.put(format_args!(...))`).
    pub fn put(&mut self, args: std::fmt::Arguments<'_>) {
        use std::fmt::Write;
        self.text.clear();
        let _ = self.text.write_fmt(args);
    }
}
/// A box that moves or recolours: shown, y and colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub shown: bool,
    pub y: f32,
    pub color: u32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChargeMark {
    pub shown: bool,
    pub x: f32,
    pub fill: u32,
}
/// One skill slot's drawn parts (its icon images stay in the tree).
#[derive(Clone, Debug, PartialEq)]
pub struct SkillView {
    /// The slot's box, shown while you play (`skill{i}` stays in the tree
    /// as the icons' container with this visibility).
    pub shown: bool,
    pub border: u32,
    /// The inset of the dark background (2 px inside a yellow border).
    pub inset: f32,
    /// The cooldown shade: shown, height, colour.
    pub shade: (bool, f32, u32),
    pub drain_edge: Bar,
    /// A3: the drain edge's comet.
    pub comet: Bar,
    pub keycap: u32,
    pub cooldown: Label,
    pub uses: Label,
    /// The denied or ready rings: shown, then each band's colour.
    pub glow: (bool, [u32; DENIED_BANDS]),
    pub charges: [ChargeMark; CHARGE_MARKS],
    pub lock: bool,
}
impl Default for SkillView {
    fn default() -> Self {
        Self {
            shown: true,
            border: crate::ui_theme::tone(0x4b4a49ff),
            inset: 1.,
            shade: (false, 78., 0x0a0908cc),
            drain_edge: Bar {
                shown: false,
                y: 1.,
                color: 0xffffffff,
            },
            comet: Bar {
                shown: false,
                y: 1.,
                color: 0xffffff99,
            },
            keycap: 0xeeececff,
            cooldown: Label::new(0xffffffff),
            uses: Label::new(0xfdee00ff),
            glow: (false, [0x8f162400; DENIED_BANDS]),
            charges: [ChargeMark {
                shown: false,
                x: 0.,
                fill: 0x141311ff,
            }; CHARGE_MARKS],
            lock: true,
        }
    }
}
/// An inventory tile's drawn parts (its container and icon stay).
#[derive(Clone, Debug, PartialEq)]
pub struct ItemView {
    pub shown: bool,
    pub x: f32,
    pub y: f32,
    pub border: u32,
    pub background: u32,
    pub unknown: bool,
}
impl Default for ItemView {
    fn default() -> Self {
        Self {
            shown: false,
            x: 0.,
            y: 100.,
            border: crate::ui_theme::tone(0x4b4a49ff),
            background: crate::ui_theme::shade_rgba(5, 0xff),
            unknown: false,
        }
    }
}
/// The next purchase tile's drawn parts (its container and icon stay).
#[derive(Clone, Debug, PartialEq)]
pub struct NextView {
    pub shown: bool,
    pub border: u32,
    pub background: u32,
    /// The progress bar's fill, in percent.
    pub progress: f32,
    /// The 16 gap strips that dash the border while not affordable.
    pub gaps: bool,
}
/// Everything the strip draws, as its nodes held it.
#[derive(Clone, Debug, PartialEq)]
pub struct HudView {
    /// The strip's root is shown (nothing draws otherwise).
    pub shown: bool,
    /// `combat` (no look; its rectangle blocks battlefield clicks).
    pub combat: bool,
    pub edge: bool,
    pub kda_icon: bool,
    pub cs_icon: bool,
    pub kills: Label,
    pub slash: [Label; 2],
    pub deaths: Label,
    pub assists: Label,
    pub cs: Label,
    pub level: Label,
    /// The health bar: shown, then the trail and fill (width %, colour).
    pub health: (bool, (f32, u32), (f32, u32)),
    /// Each tick's x, when shown.
    pub ticks: [Option<f32>; HEALTH_TICKS],
    pub hp_current: Label,
    pub hp_max: Label,
    pub hourglass: bool,
    pub respawn: Label,
    pub skills: [SkillView; 3],
    pub items: Vec<ItemView>,
    pub next: NextView,
    pub next_unavailable: bool,
    /// The coin beside the gold: shown, tint.
    pub coin: (bool, u32),
    pub gold: Label,
    /// The shop button's hover tint: shown, colour.
    pub shop_hit: (bool, u32),
    pub shop_key: bool,
    pub purchase: Label,
    pub recall_key: bool,
    pub recall_icon: bool,
    /// A2: the recall line shown, and its two notches' x.
    pub recall: (bool, [f32; 2]),
    /// B6: each skill slot's death veil (shown, colour).
    pub veils: [(bool, u32); 3],
    /// D1: the stats hairline's width, when shown.
    pub hairline: Option<f32>,
    pub feedback: Label,
}
impl Default for HudView {
    fn default() -> Self {
        let white = 0xffffffff;
        Self {
            shown: false,
            combat: true,
            edge: true,
            kda_icon: true,
            cs_icon: true,
            kills: Label::new(white),
            slash: [Label::new(0x989694ff), Label::new(0x989694ff)],
            deaths: Label::new(0xff642eff),
            assists: Label::new(white),
            cs: Label::new(white),
            level: Label::new(white),
            health: (true, (0., 0xefe6cfff), (0., 0x2e7d46ff)),
            ticks: [None; HEALTH_TICKS],
            hp_current: Label::new(white),
            hp_max: Label::new(white),
            hourglass: true,
            respawn: Label::new(white),
            skills: Default::default(),
            items: Vec::new(),
            next: NextView {
                shown: true,
                border: 0x989694ff,
                background: 0x141311ff,
                progress: 0.,
                gaps: true,
            },
            next_unavailable: true,
            coin: (true, 0xeeececff),
            gold: Label::new(0xfdee00ff),
            shop_hit: (true, 0x00000000),
            shop_key: true,
            purchase: Label::new(0xcbc9c7ff),
            recall_key: true,
            recall_icon: true,
            recall: (false, [702., 702.]),
            veils: [(false, 0); 3],
            hairline: None,
            feedback: Label {
                shown: false,
                ..Label::new(0xcbc9c7ff)
            },
        }
    }
}
impl HudView {
    /// A new build of the strip: every element as the template made it,
    /// with `items` tiles.
    pub fn reset(&mut self, items: usize) {
        *self = Self {
            items: vec![ItemView::default(); items],
            ..Self::default()
        };
    }
    /// The elements shown with the strip while you play (the old
    /// `SHOWN_WHILE_PLAYING` nodes); the purchase line and the next tile
    /// not while Manual shopping has nothing queued (`blank`).
    pub fn shown_while_playing(&mut self, playing: bool, blank: bool) {
        self.combat = playing;
        self.edge = playing;
        self.kda_icon = playing;
        self.cs_icon = playing;
        let [slash1, slash2] = &mut self.slash;
        for label in [
            &mut self.kills,
            &mut self.deaths,
            &mut self.assists,
            slash1,
            slash2,
            &mut self.cs,
            &mut self.level,
            &mut self.gold,
        ] {
            label.shown = playing;
        }
        self.health.0 = playing;
        self.coin.0 = playing;
        self.next.shown = playing && !blank;
        self.purchase.shown = playing && !blank;
        for skill in &mut self.skills {
            skill.shown = playing;
        }
        self.recall_key = playing;
        self.recall_icon = playing;
        self.shop_hit.0 = playing;
        self.shop_key = playing;
    }
    /// The elements hidden while the strip shows without your athlete (the
    /// old `HIDDEN_WHILE_WATCHING` nodes).
    pub fn hidden_while_watching(&mut self) {
        self.feedback.shown = false;
        self.respawn.shown = false;
        self.hourglass = false;
        self.hp_current.shown = false;
        self.hp_max.shown = false;
        self.next_unavailable = false;
    }
    /// Inventory tiles `0..count` exist (new ones as the template makes them).
    pub fn grow_items(&mut self, count: usize) {
        if self.items.len() < count {
            self.items.resize(count, ItemView::default());
        }
    }
}
/// The strip root's corner on screen.
pub fn origin(ingame: Option<Rect>) -> (f32, f32) {
    anchored(ingame, ROOT, (0.5, 1.), (0.5, 1.), (0., 0.))
}
/// A strip rectangle on screen.
pub fn screen(ingame: Option<Rect>, r: Rect) -> Rect {
    at(origin(ingame), r)
}
const fn numeric(size: f32, align: Align) -> TextStyle {
    TextStyle::numeric(size, align)
}
fn label(list: &mut DrawList, l: &Label, rect: Rect, z: i32, style: TextStyle) {
    if l.shown {
        list.text(&l.text, rect, z, style, l.color);
    }
}
fn outlined(list: &mut DrawList, l: &Label, rect: Rect, style: TextStyle) {
    if l.shown && !l.text.is_empty() {
        list.outlined_text(&l.text, rect, 1005, style, (l.color, 0x0e0d0cff));
    }
}
/// A glyph image at its node's place, size and tint.
fn glyph(
    list: &mut DrawList,
    o: (f32, f32),
    name: &str,
    (x, y, size): (f32, f32, u32),
    z: i32,
    tint: u32,
) {
    match asset(name, size, tint) {
        Some(a) => list.sprite(a, (o.0 + x, o.1 + y), z, false),
        None => debug_assert!(false, "no baked glyph {name} {size} {tint:08x}"),
    }
}
/// A key cap with its letter (shop P, recall B).
fn key_cap(list: &mut DrawList, o: (f32, f32), (x, y): (f32, f32), key: &str) {
    let rect = at(o, (x, y, 18., 20.));
    list.rect(rect, 1002, 2., 0xeeececff);
    list.text(key, rect, 1003, numeric(12., Align::Center), 0x393939ff);
}
/// One skill slot (its icons are tree nodes at z 1003).
fn skill(list: &mut DrawList, o: (f32, f32), i: usize, s: &SkillView) {
    if !s.shown {
        return;
    }
    let o = (o.0 + SKILL_X + SKILL_STEP * i as f32, o.1 + SKILL_Y);
    let bg = crate::ui_theme::shade_rgba(5, 0xff);
    list.rect(at(o, (0., 0., 80., 80.)), 1001, 2., s.border);
    list.rect(at(o, (0., 3., 80., 80.)), 1000, 0., 0x00000066);
    let inset = s.inset;
    list.rect(
        at(o, (inset, inset, 80. - 2. * inset, 80. - 2. * inset)),
        1002,
        0.,
        bg,
    );
    let (shade, height, shade_color) = s.shade;
    if shade {
        list.rect(at(o, (1., 1., 78., height)), 1004, 0., shade_color);
    }
    if s.drain_edge.shown {
        list.rect(
            at(o, (1., s.drain_edge.y, 78., 2.)),
            1005,
            0.,
            s.drain_edge.color,
        );
    }
    list.rect(at(o, (3., 3., 22., 22.)), 1005, 2., s.keycap);
    label(
        list,
        &s.cooldown,
        at(o, (2., 25., 76., 34.)),
        1005,
        numeric(30., Align::Center),
    );
    if s.comet.shown {
        list.rect(at(o, (1., s.comet.y, 78., 2.)), 1005, 0., s.comet.color);
    }
    label(
        list,
        &s.uses,
        at(o, (4., 61., 72., 18.)),
        1005,
        numeric(15., Align::Center),
    );
    if s.glow.0 {
        for (band, color) in s.glow.1.iter().enumerate() {
            let inset = 1. + band as f32;
            let size = 78. - 2. * band as f32;
            for r in [
                (inset, inset, size, 1.),
                (inset, inset + size - 1., size, 1.),
                (inset, inset + 1., 1., size - 2.),
                (inset + size - 1., inset + 1., 1., size - 2.),
            ] {
                list.rect(at(o, r), 1007, 0., *color);
            }
        }
    }
    let (y, w, h) = CHARGE_MARK;
    for mark in &s.charges {
        if mark.shown {
            list.rect(at(o, (mark.x, y, w, h)), 1008, 0., 0x817c77ff);
            list.rect(
                at(o, (mark.x + 1., y + 1., w - 2., h - 2.)),
                1009,
                0.,
                mark.fill,
            );
        }
    }
    if s.lock {
        glyph(list, o, "lock", (28., 28., 24), 1005, 0xffffffff);
    }
    list.text(
        ["Q", "W", "R"][i],
        at(o, (3., 3., 22., 22.)),
        1006,
        numeric(14., Align::Center),
        0x393939ff,
    );
}
/// An inventory tile (its icon is a tree node at z 1003).
fn item(list: &mut DrawList, o: (f32, f32), t: &ItemView) {
    if !t.shown {
        return;
    }
    let o = (o.0 + t.x, o.1 + t.y);
    list.rect(at(o, (0., 0., 36., 36.)), 1001, 2., t.border);
    list.rect(at(o, (1., 1., 34., 34.)), 1002, 0., t.background);
    if t.unknown {
        list.text(
            "?",
            at(o, (0., 0., 36., 36.)),
            1005,
            TextStyle::medium(16., Align::Center),
            0xffffffff,
        );
    }
}
fn next(list: &mut DrawList, o: (f32, f32), n: &NextView) {
    if !n.shown {
        return;
    }
    let o = (o.0 + NEXT.0, o.1 + NEXT.1);
    list.rect(at(o, (0., 0., 36., 36.)), 1001, 0., n.border);
    list.rect(at(o, (1., 1., 34., 34.)), 1002, 0., n.background);
    list.rect(at(o, (0., 40., 36., 3.)), 1001, 0., 0xffffff26);
    list.rect(
        at(o, (0., 40., 36. * n.progress / 100., 3.)),
        1002,
        0.,
        0xfdee00ff,
    );
    if n.gaps {
        let bg = crate::ui_theme::rgba(0xff);
        for side in 0..4 {
            for j in 0..4 {
                let t = (6 + j * 8) as f32;
                let r = match side {
                    0 => (t, 0., 4., 1.),
                    1 => (t, 35., 4., 1.),
                    2 => (0., t, 1., 4.),
                    _ => (35., t, 1., 4.),
                };
                list.rect(at(o, r), 1002, 0., bg);
            }
        }
    }
}
/// The strip's commands for one frame (none while its root is hidden).
pub fn build(v: &HudView, ingame: Option<Rect>, list: &mut DrawList) {
    list.clear();
    if !v.shown {
        return;
    }
    let o = origin(ingame);
    let bg = crate::ui_theme::rgba(0xff);
    let edge = crate::ui_theme::tone(0x4b4a49ff);
    list.rect(at(o, STRIP), 1000, 0., bg);
    if v.edge {
        list.rect(at(o, (0., 90., 1560., 1.)), 1001, 0., edge);
    }
    for x in [0., 1544.] {
        list.rect(at(o, (x, 90., 16., 2.)), 1002, 0., 0xcbc9c7ff);
    }
    if v.kda_icon {
        glyph(list, o, "ef_swords", (356., 105., 26), 1003, 0xeeececff);
    }
    if v.cs_icon {
        glyph(list, o, "hud_minion", (510., 105., 26), 1003, 0xffffffff);
    }
    let row = |x: f32, w: f32| at(o, (x, 102., w, 32.));
    label(
        list,
        &v.kills,
        row(386., 30.),
        1005,
        numeric(24., Align::Start),
    );
    label(
        list,
        &v.slash[0],
        row(416., 10.),
        1005,
        numeric(24., Align::Start),
    );
    label(
        list,
        &v.deaths,
        row(426., 30.),
        1005,
        numeric(24., Align::Start),
    );
    label(
        list,
        &v.slash[1],
        row(456., 10.),
        1005,
        numeric(24., Align::Start),
    );
    label(
        list,
        &v.assists,
        row(466., 30.),
        1005,
        numeric(24., Align::Start),
    );
    label(
        list,
        &v.cs,
        row(538., 50.),
        1005,
        numeric(24., Align::Start),
    );
    label(
        list,
        &v.level,
        at(o, (730., 100., 40., 36.)),
        1005,
        numeric(24., Align::Center),
    );
    let (health, (trail, trail_color), (fill, fill_color)) = v.health;
    if health {
        list.rect(
            at(o, (780., 106., 360., 24.)),
            1001,
            0.,
            crate::ui_theme::tone(0x0e0d0cff),
        );
        list.rect(
            at(o, (780., 106., 360. * trail / 100., 24.)),
            1002,
            0.,
            trail_color,
        );
        list.rect(
            at(o, (780., 106., 360. * fill / 100., 24.)),
            1003,
            0.,
            fill_color,
        );
    }
    for x in v.ticks.iter().flatten() {
        list.rect(at(o, (*x, 106., 1., 24.)), 1004, 0., 0x00000073);
    }
    outlined(
        list,
        &v.hp_current,
        at(o, (944., 106., 84., 24.)),
        numeric(18., Align::Start),
    );
    outlined(
        list,
        &v.hp_max,
        at(o, (1030., 106., 104., 24.)),
        numeric(18., Align::Start),
    );
    if v.hourglass {
        glyph(list, o, "ef_skull", (919., 106., 22), 1005, 0xeeececff);
    }
    label(
        list,
        &v.respawn,
        at(o, (953., 106., 56., 24.)),
        1005,
        numeric(22., Align::Center),
    );
    for (i, s) in v.skills.iter().enumerate() {
        skill(list, o, i, s);
    }
    for t in v.items.iter().take(6) {
        item(list, o, t);
    }
    next(list, o, &v.next);
    if v.next_unavailable {
        glyph(
            list,
            o,
            "ef_unavailable",
            (1398., 108., 20),
            1004,
            0xeeececff,
        );
    }
    if v.coin.0 {
        glyph(list, o, "ef_coin", (1430., 99., 17), 1003, v.coin.1);
    }
    label(
        list,
        &v.gold,
        at(o, (1450., 94., 58., 26.)),
        1005,
        numeric(21., Align::Start),
    );
    if v.shop_hit.0 {
        list.rect(at(o, SHOP_HIT), 1001, 2., v.shop_hit.1);
    }
    if v.shop_key {
        key_cap(list, o, (1512., 97.), "P");
    }
    label(
        list,
        &v.purchase,
        at(o, (1450., 117., 58., 23.)),
        1005,
        numeric(16., Align::Start),
    );
    if v.recall_key {
        key_cap(list, o, (682., 108.), "B");
    }
    if v.recall_icon {
        glyph(list, o, "ef_recall", (702., 109., 20), 1003, 0xeeececff);
    }
    if v.recall.0 {
        list.rect(at(o, (702., 131., 20., 3.)), 1004, 0., 0xfdee00ff);
        for x in v.recall.1 {
            list.rect(at(o, (x, 131., 3., 3.)), 1005, 0., bg);
        }
    }
    for (i, (shown, color)) in v.veils.iter().enumerate() {
        if *shown {
            let x = SKILL_X + SKILL_STEP * i as f32;
            list.rect(
                at(o, (x, SKILL_Y, 80., 80.)),
                crate::hud_motion_pass::B6_Z,
                0.,
                *color,
            );
        }
    }
    if let Some(width) = v.hairline {
        list.rect(at(o, (0., 138., width, 2.)), 1004, 0., 0xfdee00ff);
    }
    if v.feedback.shown {
        list.rect(at(o, FEEDBACK), 1112, 0., crate::ui_theme::tone(0x1e1e1df0));
        // The label sets no alignment: the runner's default (left, top; a
        // guess from the game's own templates, which spell out every other).
        list.text(
            &v.feedback.text,
            at(o, (710., -42., 500., 30.)),
            1113,
            TextStyle {
                font: crate::draw_list::Font::Medium,
                size: 16.,
                align: (Align::Start, Align::Start),
            },
            v.feedback.color,
        );
    }
    // Tiles beyond the template's six were spawned after the tooltip.
    for t in v.items.iter().skip(6) {
        item(list, o, t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw_list::Cmd;
    fn rects(v: &HudView, z: i32) -> Vec<(Rect, u32)> {
        let mut list = DrawList::default();
        build(v, None, &mut list);
        list.commands()
            .iter()
            .filter_map(|c| match *c {
                Cmd::Rect {
                    rect, z: cz, color, ..
                } if cz == z => Some((rect, color)),
                _ => None,
            })
            .collect()
    }
    #[test]
    fn a_ready_purchase_turns_the_next_tile_yellow_without_its_dashes() {
        // The scripted HUD frames never reach a ready purchase.
        let mut v = HudView {
            shown: true,
            ..Default::default()
        };
        let top = 934.;
        let tile = ((1390., top + 100., 36., 36.), 0x989694ff);
        let gaps = |v: &HudView| {
            rects(v, 1002)
                .iter()
                .filter(|(r, _)| (r.2 == 4. && r.3 == 1.) || (r.2 == 1. && r.3 == 4.))
                .count()
        };
        assert!(rects(&v, 1001).contains(&tile));
        assert_eq!(gaps(&v), 16);
        v.next.border = 0xfdee00ff;
        v.next.gaps = false;
        assert!(rects(&v, 1001).contains(&(tile.0, 0xfdee00ff)));
        assert_eq!(gaps(&v), 0);
        // Hidden while Manual shopping has nothing queued.
        v.next.shown = false;
        assert!(!rects(&v, 1001).iter().any(|(r, _)| r.0 == 1390.));
    }
}
