//! The Tab panel's draw list (1.6.1-dev1), the same model as the HUD strip's
//! (`hud_draw.rs`): `TeamView` holds what the panel's drawn elements show,
//! set by `team_status.rs` where it used to write their nodes, and
//! [`build`] turns it into commands in the old template's node order, at
//! the old nodes' z values (1600-1607).
//!
//! Drawn: the panel, its four rules, the five headers per side, each row's
//! tint, selected bar, row line, portrait frame, inset and shade, the
//! respawn count, level, K/D/A, CS and gold, and each item tile's box,
//! inset and "?" mark. What stays in the tree under the panel's root
//! (`team_status::template`): the side and row containers, the champion
//! portraits (`ui_set_champion_icon` needs an image node) inside their
//! `portrait` containers, and the item icons (sprite-sheet images) inside
//! `item{i}` containers that keep each tile's place and visibility.
//!
//! The K/D/A was one label of rich text (`<#rrggbbaa>` colour runs), which
//! a draw call cannot show; it is five single-colour texts (kills, a slash,
//! deaths, a slash, assists) side by side, centred as the label centred it
//! ([`KdaView::set`]), by `hud_style::width`: the medium weight's advances,
//! so the numeric K/D/A is up to ~3 px narrower (CHANGELOG 1.6.1-dev1).
use crate::draw_list::{anchored, at, Align, DrawList, TextStyle};
use crate::team_status::{tile_at, CS, GOLD_W, ITEMS_HEADER_W, ITEMS_X, KDA, LEVEL};
use std::fmt::Write;

/// The panel's root: centred in `ingame`, 80 px above its centre.
pub const ROOT_OFFSET: (f32, f32) = (0., -80.);
/// The headers' row and the rows' first y in a side.
const HEADERS_H: f32 = 34.;
/// The K/D/A's size and its runs' colours: kills, slash, deaths, slash,
/// assists (the rich label's runs).
pub const KDA_SIZE: f32 = 22.;
pub const KDA_COLORS: [u32; 5] = [0xeeececff, 0x989694ff, 0xff855bff, 0x989694ff, 0xeeececff];
const GREY: u32 = 0x4b4a49ff;

/// A number shown as text, formatted again only when it changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NumberView {
    value: Option<usize>,
    pub text: String,
}
impl NumberView {
    pub fn set(&mut self, value: usize) {
        if self.value != Some(value) {
            self.value = Some(value);
            self.text.clear();
            let _ = write!(self.text, "{value}");
        }
    }
}
/// The respawn count's text, kept until its key (alive, respawn) changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TimerView {
    key: Option<(u8, usize)>,
    pub text: String,
}
impl TimerView {
    pub fn set(&mut self, key: (u8, usize), write: impl FnOnce(&mut String)) {
        if self.key != Some(key) {
            self.key = Some(key);
            self.text.clear();
            write(&mut self.text);
        }
    }
}
/// One run of the K/D/A: its bytes in the text, its left edge from the
/// K/D/A box's left edge and its width.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Run {
    pub range: (usize, usize),
    pub x: f32,
    pub width: f32,
}
/// The K/D/A as five runs of one text ("12/3/24").
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KdaView {
    key: Option<(usize, usize, usize)>,
    pub text: String,
    pub runs: [Run; 5],
}
impl KdaView {
    /// Lay out kills/deaths/assists centred in the K/D/A box: each run's
    /// width from the font's advances, the whole run of five centred, each
    /// run starting where the one before ends (as the rich label set them
    /// one after another).
    pub fn set(&mut self, (k, d, a): (usize, usize, usize)) {
        if self.key == Some((k, d, a)) {
            return;
        }
        self.key = Some((k, d, a));
        self.text.clear();
        let mut ranges = [(0, 0); 5];
        for (i, range) in ranges.iter_mut().enumerate() {
            let start = self.text.len();
            let _ = match i {
                0 => write!(self.text, "{k}"),
                2 => write!(self.text, "{d}"),
                4 => write!(self.text, "{a}"),
                _ => write!(self.text, "/"),
            };
            *range = (start, self.text.len());
        }
        let widths = ranges.map(|(s, e)| crate::hud_style::width(&self.text[s..e], KDA_SIZE));
        let total: f32 = widths.iter().sum();
        let mut x = (KDA.1 as f32 - total) / 2.;
        for ((run, range), width) in self.runs.iter_mut().zip(ranges).zip(widths) {
            *run = Run { range, x, width };
            x += width;
        }
    }
    pub fn run_text(&self, run: &Run) -> &str {
        &self.text[run.range.0..run.range.1]
    }
}
/// One item tile: shown (a slot of this athlete's inventory) and whether
/// it holds an item without an icon (the "?" mark).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TileView {
    pub shown: bool,
    pub unknown: bool,
}
/// One row: a player.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RowView {
    pub shown: bool,
    /// The athlete you control: the row's tint and the selected bar.
    pub mine: bool,
    /// The portrait frame's colour (blue or red by side).
    pub frame: u32,
    /// The dark shade over the portrait while dead or unknown.
    pub shade: bool,
    pub timer: TimerView,
    pub level: NumberView,
    pub kda: KdaView,
    pub cs: NumberView,
    pub gold: NumberView,
    /// Every tile the panel has built, shown or not.
    pub tiles: Vec<TileView>,
}
/// The panel as its nodes held it.
#[derive(Debug, Default, PartialEq)]
pub struct TeamView {
    /// The root is shown this frame (Tab held, the panel built).
    pub shown: bool,
    /// The panel's width and height.
    pub size: (f32, f32),
    pub side_width: f32,
    pub row_height: f32,
    /// LV, K/D/A, CS, ITEMS, GOLD (translated).
    pub headers: [&'static str; 5],
    /// The items header shows when any row has a slot.
    pub items_header: bool,
    /// The gold column's left edge in a side.
    pub gold_x: f32,
    pub rows: [[RowView; 5]; 2],
}
/// The panel's root corner on screen.
pub fn origin(view: &TeamView, ingame: Option<crate::draw_list::Rect>) -> (f32, f32) {
    anchored(ingame, view.size, (0.5, 0.5), (0.5, 0.5), ROOT_OFFSET)
}
/// A side's corner in the panel.
fn side_x(view: &TeamView, side: usize) -> f32 {
    20. + side as f32 * (view.side_width + 26.)
}
/// The panel's commands (nothing while its root is hidden).
pub fn build(view: &TeamView, ingame: Option<crate::draw_list::Rect>, list: &mut DrawList) {
    if !view.shown {
        return;
    }
    let o = origin(view, ingame);
    let (w, h) = view.size;
    let tone = crate::ui_theme::tone;
    list.rect(at(o, (0., 0., w, h)), 1600, 2., tone(0x1e1e1df5));
    for rule in [
        (0., 0., w, 1.),
        (0., h - 1., w, 1.),
        (0., 0., 1., h),
        (w - 1., 0., 1., h),
    ] {
        list.rect(at(o, rule), 1601, 0., tone(GREY));
    }
    let inset = crate::ui_theme::shade_rgba(8, 0xff);
    for side in 0..2 {
        let s = (o.0 + side_x(view, side), o.1 + 16.);
        let header = TextStyle::numeric(13., Align::Center);
        for (k, (x, width), style) in [
            (0, LEVEL, header),
            (1, KDA, header),
            (2, CS, header),
            (
                3,
                (ITEMS_X, ITEMS_HEADER_W),
                TextStyle::numeric(13., Align::Start),
            ),
            (4, (0, GOLD_W), TextStyle::numeric(13., Align::End)),
        ] {
            if k == 3 && !view.items_header {
                continue;
            }
            let x = if k == 4 { view.gold_x } else { x as f32 };
            list.text(
                view.headers[k],
                at(s, (x, 0., width as f32, HEADERS_H)),
                1604,
                style,
                0x989694ff,
            );
        }
        for (lane, row) in view.rows[side].iter().enumerate() {
            if row.shown {
                let r = (s.0, s.1 + HEADERS_H + lane as f32 * view.row_height);
                self::row(view, row, r, inset, list);
            }
        }
    }
}
fn row(view: &TeamView, row: &RowView, r: (f32, f32), inset: u32, list: &mut DrawList) {
    let (sw, rh) = (view.side_width, view.row_height);
    let tint = if row.mine { 0xfdee0009 } else { 0 };
    list.rect(at(r, (0., 0., sw, rh)), 1601, 0., tint);
    if row.mine {
        list.rect(at(r, (0., 0., 2., rh)), 1602, 0., 0xfdee00ff);
    }
    list.rect(
        at(r, (0., rh - 1., sw, 1.)),
        1602,
        0.,
        crate::ui_theme::tone(0x4b4a4973),
    );
    // The portrait's frame and inset; the portrait (z 1605) is a node.
    let p = at(r, PORTRAIT);
    list.rect(p, 1603, 2., row.frame);
    list.rect((p.0 + 1., p.1 + 1., 46., 50.), 1604, 0., inset);
    if row.shade {
        list.rect(p, 1606, 0., 0x00000099);
    }
    list.outlined_text(
        &row.timer.text,
        p,
        1607,
        TextStyle::numeric(25., Align::Center),
        (0xffffffff, 0x000000ff),
    );
    let column = |(x, w): (usize, usize)| at(r, (x as f32, 0., w as f32, rh));
    list.text(
        &row.level.text,
        column(LEVEL),
        1604,
        TextStyle::numeric(17., Align::Center),
        0xcbc9c7ff,
    );
    for (run, color) in row.kda.runs.iter().zip(KDA_COLORS) {
        // Left-aligned from the run's place; the box reaches the K/D/A
        // box's width so nothing could wrap.
        list.text(
            row.kda.run_text(run),
            at(r, (KDA.0 as f32 + run.x, 0., KDA.1 as f32, rh)),
            1604,
            TextStyle::numeric(KDA_SIZE, Align::Start),
            color,
        );
    }
    list.text(
        &row.cs.text,
        column(CS),
        1604,
        TextStyle::numeric(21., Align::Center),
        0xeeececff,
    );
    list.text(
        &row.gold.text,
        at(r, (view.gold_x, 0., GOLD_W as f32, rh)),
        1604,
        TextStyle::numeric(21., Align::End),
        0xfdee00ff,
    );
    for (i, tile) in row.tiles.iter().enumerate() {
        if !tile.shown {
            continue;
        }
        let (x, y) = tile_at(i);
        let b = at(r, (x as f32, y as f32, 32., 32.));
        list.rect(b, 1603, 2., crate::ui_theme::tone(GREY));
        list.rect((b.0 + 1., b.1 + 1., 30., 30.), 1604, 0., inset);
        if tile.unknown {
            list.text(
                "?",
                b,
                1606,
                TextStyle::numeric(18., Align::Center),
                0xeeececff,
            );
        }
    }
}
/// The portrait's box in a row (its frame; the image sits 2 px inside).
pub const PORTRAIT: crate::draw_list::Rect = (6., 8., 48., 52.);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw_list::Cmd;

    #[test]
    fn kda_runs_sit_side_by_side_centred_in_their_box() {
        let mut kda = KdaView::default();
        for value in [(0, 0, 0), (12, 3, 24), (99, 99, 99), (1, 10, 100)] {
            kda.set(value);
            let pieces: Vec<_> = kda.runs.iter().map(|r| kda.run_text(r)).collect();
            let (k, d, a) = value;
            assert_eq!(
                pieces,
                [
                    k.to_string(),
                    "/".into(),
                    d.to_string(),
                    "/".into(),
                    a.to_string()
                ]
            );
            // Contiguous: each run starts where the one before ends.
            for pair in kda.runs.windows(2) {
                assert!((pair[0].x + pair[0].width - pair[1].x).abs() < 1e-3);
            }
            // The whole is centred in the 110 px box, as the label centred it.
            let first = kda.runs[0].x;
            let last = kda.runs[4].x + kda.runs[4].width;
            assert!(((first + last) / 2. - KDA.1 as f32 / 2.).abs() < 1e-3);
            let whole = crate::hud_style::width(&kda.text, KDA_SIZE);
            assert!((last - first - whole).abs() < 1e-3);
            assert!(first >= 0. && last <= KDA.1 as f32, "{value:?}");
        }
        // Kills and assists light, slashes grey, deaths orange.
        assert_eq!(KDA_COLORS[0], KDA_COLORS[4]);
        assert_eq!(KDA_COLORS[1], KDA_COLORS[3]);
        assert_eq!(KDA_COLORS[2], 0xff855bff);
    }
    #[test]
    fn steady_values_keep_their_texts() {
        let mut kda = KdaView::default();
        kda.set((3, 1, 4));
        let (ptr, runs) = (kda.text.as_ptr(), kda.runs);
        kda.set((3, 1, 4));
        assert_eq!((kda.text.as_ptr(), kda.runs), (ptr, runs));
        let mut gold = NumberView::default();
        gold.set(12345);
        let capacity = gold.text.capacity();
        for v in 0..1000 {
            gold.set(v);
        }
        assert_eq!(gold.text, "999");
        assert_eq!(gold.text.capacity(), capacity);
    }
    fn view(columns: usize) -> TeamView {
        let g = crate::team_status::geometry_for_test(columns);
        let mut v = TeamView {
            shown: true,
            size: (g.0 as f32, g.1 as f32),
            side_width: g.2 as f32,
            row_height: g.3 as f32,
            headers: ["LV", "K / D / A", "CS", "ITEMS", "GOLD"],
            items_header: true,
            gold_x: crate::team_status::gold_x_for_test(g.2) as f32,
            ..TeamView::default()
        };
        for row in v.rows.iter_mut().flatten() {
            row.shown = true;
            row.level.set(30);
            row.kda.set((9, 9, 9));
            row.cs.set(999);
            row.gold.set(99999);
            row.tiles = vec![
                TileView {
                    shown: true,
                    unknown: true
                };
                columns
            ];
        }
        v
    }
    fn texts(list: &DrawList) -> Vec<(crate::draw_list::Rect, TextStyle, String)> {
        list.commands()
            .iter()
            .filter_map(|c| match *c {
                Cmd::Text {
                    rect, style, text, ..
                } => Some((rect, style, list.text_of(text).to_owned())),
                _ => None,
            })
            .collect()
    }
    #[test]
    fn headers_and_values_share_each_columns_box_and_alignment() {
        for columns in [4, 5, 6] {
            let v = view(columns);
            let mut list = DrawList::default();
            build(&v, None, &mut list);
            let t = texts(&list);
            let o = origin(&v, None);
            let s0 = (o.0 + 20., o.1 + 16.);
            let find = |text: &str| {
                t.iter()
                    .filter(|(_, _, s)| s == text)
                    .map(|(r, st, _)| ((r.0 - s0.0, r.2), st.align.0, st.size))
                    .next()
                    .unwrap()
            };
            let x = |(x, w): (usize, usize)| (x as f32, w as f32);
            assert_eq!(find("LV"), (x(LEVEL), Align::Center, 13.));
            assert_eq!(find("30"), (x(LEVEL), Align::Center, 17.));
            assert_eq!(find("K / D / A"), (x(KDA), Align::Center, 13.));
            assert_eq!(find("CS"), (x(CS), Align::Center, 13.));
            assert_eq!(find("999"), (x(CS), Align::Center, 21.));
            assert_eq!(
                find("ITEMS"),
                (x((ITEMS_X, ITEMS_HEADER_W)), Align::Start, 13.)
            );
            let gold = (v.gold_x, GOLD_W as f32);
            assert_eq!(find("GOLD"), (gold, Align::End, 13.));
            assert_eq!(find("99999"), (gold, Align::End, 21.));
            // Ten rows of 5 K/D/A runs (size 22), a level, CS and gold;
            // a "?" per tile; the outlined timer is empty while alive.
            assert_eq!(t.iter().filter(|x| x.1.size == KDA_SIZE).count(), 50);
            assert_eq!(t.iter().filter(|x| x.2 == "?").count(), 10 * columns);
            assert_eq!(t.len(), 10 + 10 * (5 + 3) + 10 * columns);
        }
    }
    #[test]
    fn a_hidden_panel_or_row_draws_nothing_and_the_selected_row_its_bar() {
        let mut v = view(4);
        let mut list = DrawList::default();
        v.shown = false;
        build(&v, None, &mut list);
        assert!(list.is_empty());
        v.shown = true;
        for row in v.rows.iter_mut().flatten() {
            row.shown = false;
        }
        build(&v, None, &mut list);
        // The panel, four rules and two sides' five headers.
        assert_eq!(list.commands().len(), 5 + 10);
        list.clear();
        v.rows[0][1].shown = true;
        v.rows[0][1].mine = true;
        v.rows[0][1].shade = true;
        v.rows[0][1].timer.set((0, 120), |s| s.push('2'));
        build(&v, None, &mut list);
        let rects: Vec<_> = list
            .commands()
            .iter()
            .filter_map(|c| match *c {
                Cmd::Rect { z, color, .. } => Some((z, color)),
                _ => None,
            })
            .collect();
        assert!(rects.contains(&(1601, 0xfdee0009)));
        assert!(rects.contains(&(1602, 0xfdee00ff)));
        assert!(rects.contains(&(1606, 0x00000099)));
        // The countdown: four dark copies and the white text.
        assert_eq!(texts(&list).iter().filter(|t| t.2 == "2").count(), 5);
    }
}
