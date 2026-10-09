//! Fixed Emotes body: schema-backed controls and an isolated, silent draft preview.
use super::*;
use crate::emotes::Config;
mod library;

pub(super) const HEIGHT: i32 = 520;
const PREVIEW_SCALE: f32 = 0.35;
const LEFT_W: i32 = 548;
const PREVIEW_X: i32 = 590;
const PREVIEW_W: f32 = 452.;
const PREVIEW_H: f32 = 260.;
const ZOOMS: [f32; 4] = [0.5, 1., 2., 3.];

#[derive(Clone, Copy)]
pub(super) enum Action {
    Tab(usize),
    Library(library::Action),
    Choice(usize, usize),
    Replay,
    Zoom(usize),
}
pub(super) struct Panel {
    started: Instant,
    zoom: usize,
    visible: bool,
    drag: Option<usize>,
    tab: usize,
    library: library::Panel,
}
impl Default for Panel {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            zoom: 1,
            visible: false,
            drag: None,
            tab: 0,
            library: library::Panel::default(),
        }
    }
}
fn definitions() -> impl Iterator<Item = (usize, &'static settings::OptionDef)> {
    OPTIONS
        .iter()
        .enumerate()
        .filter(|(_, d)| d.page == PAGE_EMOTES)
}
pub(super) fn events() -> Vec<(String, Event)> {
    let mut events = vec![(
        "emote_panel.display.replay".into(),
        Event::Emote(Action::Replay),
    )];
    for i in 0..2 {
        events.push((format!("emote_panel.tab{i}"), Event::Emote(Action::Tab(i))));
    }
    events.extend(library::events());
    for (i, def) in definitions() {
        if matches!(def.control, Control::Toggle | Control::Choice(_)) {
            for j in 0..2 {
                events.push((
                    format!("emote_panel.display.opt{i}.choice{j}"),
                    Event::Emote(Action::Choice(i, j)),
                ));
            }
        }
    }
    events.extend((0..ZOOMS.len()).map(|i| {
        (
            format!("emote_panel.display.zoom{i}"),
            Event::Emote(Action::Zoom(i)),
        )
    }));
    events
}
pub(super) fn template() -> String {
    let bg = crate::ui_theme::hex(0xff);
    let mut s = format!("#emote_panel:color {{ x: {CONTENT_X}px; y: 225px; width: {CONTENT_W}px; height: {HEIGHT}px; color: #{bg}; ignore_event: true; visible: false; z: 2003;");
    for (i, title) in ["Library", "Display"].iter().enumerate() {
        s.push_str(&raised_button(
            &format!("tab{i}"),
            (i as i32 * 190, 0, 180, 40),
            2,
            title,
            17,
            2006,
            "",
        ));
    }
    s.push_str(&rect("line", (0, 53, CONTENT_W, 1), "~4b4a49ff", 2004));
    s.push_str(&library::template());
    s.push_str("#display:empty { x: 0px; y: 64px; width: 1042px; height: 456px; ignore_event: true; visible: false; z: 2004;");
    let mut y = 0;
    for (i, def) in definitions() {
        let toggle = matches!(def.control, Control::Toggle);
        let height = if toggle { 52 } else { 76 };
        s.push_str(&format!("#opt{i}:color {{ x: 0px; y: {y}px; width: {LEFT_W}px; height: {height}px; color: #~3a3837ff; ignore_event: true; z: 2004;"));
        s.push_str(&label(
            "name",
            (16, 6, if toggle { 290 } else { 516 }, 24),
            18,
            def.label,
            true,
            "eeececff",
            "Left",
            2006,
        ));
        if def.key != "emote_zoom" {
            // Longer explanations use the page footer; control hints remain readable.
            let hint = match def.key {
                "emotes" => "Hold T · release to show",
                "emote_sound" => "Shared confirmation sound",
                "emote_height" => "Above champion's ground position",
                "emote_scale" => "100% = 88 px before zoom and pop",
                "emote_zoom" => "Scale image and height with camera zoom",
                "emote_cooldown" => "A new emote replaces the previous image",
                _ => def.hint,
            };
            s.push_str(&label(
                "hint",
                (
                    16,
                    if toggle { 31 } else { 28 },
                    if toggle { 290 } else { 516 },
                    19,
                ),
                12,
                hint,
                false,
                "a3a19fff",
                "Left",
                2006,
            ));
        }
        match def.control {
            Control::Slider(..) => {
                s.push_str("#slider:slider { x: 20px; y: 43px; width: 378px; height: 28px; z: 2006; background: Color { prop: { color: #00000000; } } foreground: Color { prop: { color: #00000000; } } view_min_ratio: 0.0; ratio: 0.0;");
                s.push_str(&rect("track", (0, 11, 378, 6), "~6d6c6aff", 2007));
                s.push_str(&rect("fill", (0, 11, 190, 6), "fdee00ff", 2008));
                s.push_str("#thumb:color { x: 190px; y: 5px; width: 18px; height: 18px; pivot_x: 0.5; color: #eeececff; rounding: Uniform { rounding: 9; } ignore_event: true; z: 2009; }}");
                s.push_str(&label(
                    "value",
                    (410, 42, 118, 29),
                    18,
                    "",
                    true,
                    "eeececff",
                    "Right",
                    2008,
                ));
            }
            Control::Toggle | Control::Choice(_) => {
                let (x, cy, w) = if toggle { (326, 9, 100) } else { (16, 35, 254) };
                let options = match def.control {
                    Control::Choice(v) => v,
                    _ => &["Off", "On"],
                };
                for (j, copy) in options.iter().enumerate() {
                    s.push_str(&raised_button(
                        &format!("choice{j}"),
                        (x + j as i32 * (w + 8), cy, w, 34),
                        17,
                        copy,
                        15,
                        2006,
                        "",
                    ));
                }
            }
            _ => {}
        }
        s.push('}');
        y += height + 6;
    }
    s.push_str(&rect("divider", (570, 0, 1, 446), "~4b4a49ff", 2004));
    s.push_str(&label(
        "preview_title",
        (PREVIEW_X, 0, 310, 28),
        20,
        "Live preview",
        true,
        "eeececff",
        "Left",
        2005,
    ));
    s.push_str(&raised_button(
        "replay",
        (PREVIEW_X + 354, 0, 98, 30),
        2,
        "Replay",
        15,
        2006,
        "",
    ));
    s.push_str(&format!("#scene:color {{ x: {PREVIEW_X}px; y: 42px; width: {PREVIEW_W}px; height: {PREVIEW_H}px; color: #~242221ff; ignore_event: true; z: 2004;"));
    s.push_str(&rect("ground", (26, 232, 400, 1), "~6d6c6aff", 2005));
    // A simple champion marker makes the anchor clear without loading a game entity.
    s.push_str("#champion:empty { x: 0px; y: 0px; width: 452px; height: 260px; ignore_event: true; z: 2006; #head:color { x: 18px; y: 0px; width: 28px; height: 28px; color: #eeececff; ignore_event: true; z: 2006; } #body:color { x: 8px; y: 32px; width: 48px; height: 44px; color: #989694ff; ignore_event: true; z: 2006; } #health:color { x: 0px; y: -12px; width: 64px; height: 5px; color: #9cbc68ff; ignore_event: true; z: 2006; }}");
    s.push_str(&image("emote", "emote_gg", (0, 0, 88), "ffffffff", 2007));
    s.push('}');
    s.push_str(&label(
        "metrics",
        (PREVIEW_X, 310, 452, 25),
        14,
        "",
        false,
        "eeececff",
        "Left",
        2005,
    ));
    s.push_str(&label(
        "zoom_label",
        (PREVIEW_X, 340, 452, 22),
        14,
        "Preview camera zoom",
        true,
        "cbc9c7ff",
        "Left",
        2005,
    ));
    for (i, zoom) in ZOOMS.iter().enumerate() {
        s.push_str(&raised_button(
            &format!("zoom{i}"),
            (PREVIEW_X + i as i32 * 115, 368, 107, 34),
            2,
            &format!("{}%", (zoom * 100.) as i32),
            15,
            2006,
            "",
        ));
    }
    s.push_str(&label(
        "note",
        (PREVIEW_X, 410, 452, 18),
        12,
        "Silent preview · fixed scale; pans at extreme heights",
        false,
        "989694ff",
        "Left",
        2005,
    ));
    s.push_str(&label(
        "note_apply",
        (PREVIEW_X, 430, 452, 18),
        12,
        "Apply saves changes; Cancel keeps your current settings.",
        false,
        "989694ff",
        "Left",
        2005,
    ));
    s.push_str("}}");
    s
}

/// Pan, rather than shrink, to keep tall samples visible at one fixed preview scale.
fn preview_pan(config: Config, zoom: f32) -> f32 {
    let (peak, _) = config.geometry((0., 0.), 0.12, zoom);
    let (settled, _) = config.geometry((0., 0.), 1., zoom);
    let top = (-peak.y).max(-settled.y);
    (top * PREVIEW_SCALE - (PREVIEW_H - 36.)).max(0.)
}
impl Panel {
    pub fn hide(&mut self) {
        self.visible = false;
        self.drag = None;
    }
    pub fn handle(&mut self, action: Action, draft: &mut Values) {
        match action {
            Action::Tab(tab) if tab < 2 => {
                self.tab = tab;
                self.drag = None;
                self.started = Instant::now();
            }
            Action::Library(action) if self.tab == 0 => self.library.handle(action, draft),
            Action::Choice(index, choice) => {
                if self.tab != 1 {
                    return;
                }
                if let Some(def) = OPTIONS.get(index).filter(|d| d.page == PAGE_EMOTES) {
                    if choice < 2 && matches!(def.control, Control::Toggle | Control::Choice(_)) {
                        draft.set(def.key, choice as f64);
                    }
                }
            }
            Action::Zoom(i) if i < ZOOMS.len() && self.tab == 1 => {
                self.zoom = i;
            }
            Action::Replay if self.tab == 1 => self.started = Instant::now(),
            _ => {}
        }
    }
    pub fn render(
        &mut self,
        ui: &mut SettingsUi,
        ctx: &mut StableClient<'_>,
        y: i32,
        cursor: Option<(f32, f32)>,
        keys: Keys,
    ) {
        if !self.visible {
            self.started = Instant::now();
            self.visible = true;
        }
        if !keys.focused || !keys.raw.0[1] {
            self.drag = None;
        }
        ui.props(ctx, "emote_panel", format!("visible: true; y: {y}px;"));
        for i in 0..2 {
            ui.paint(
                ctx,
                &format!("emote_panel.tab{i}"),
                true,
                cursor,
                (
                    if self.tab == i {
                        0xfdee00ff
                    } else {
                        crate::ui_theme::tone(0x393939ff)
                    },
                    0x707070ff,
                    0x5b5b5bff,
                ),
                (crate::ui_theme::tone(0x6d6c6aff), 1),
                if self.tab == i {
                    0x393939ff
                } else {
                    0xeeececff
                },
            );
        }
        ui.visible(ctx, "emote_panel.display", self.tab == 1);
        ui.visible(ctx, "emote_panel.library", self.tab == 0);
        if self.tab == 0 {
            self.drag = None;
            self.library.render(ui, ctx, cursor);
            return;
        }
        for (i, def) in definitions() {
            let row = format!("emote_panel.display.opt{i}");
            if def.key == "emotes" {
                ui.text(
                    ctx,
                    &format!("{row}.hint"),
                    &format!(
                        "Hold {} · release to show",
                        ui.draft
                            .binding("emote")
                            .into_iter()
                            .flatten()
                            .next()
                            .map_or_else(|| "unbound".into(), |b| b.label())
                    ),
                );
            }
            match def.control {
                Control::Toggle | Control::Choice(_) => {
                    for j in 0..2 {
                        ui.paint(
                            ctx,
                            &format!("{row}.choice{j}"),
                            true,
                            cursor,
                            (
                                if ui.draft.number(def.key) == j as f64 {
                                    0xeeececff
                                } else {
                                    crate::ui_theme::tone(0x393939ff)
                                },
                                0x707070ff,
                                0x5b5b5bff,
                            ),
                            (crate::ui_theme::tone(0x6d6c6aff), 1),
                            if ui.draft.number(def.key) == j as f64 {
                                0x393939ff
                            } else {
                                0xeeececff
                            },
                        );
                    }
                }
                Control::Slider(lo, hi, step, unit) => {
                    let node = format!("{row}.slider");
                    if keys.focused
                        && keys.raw.0[1]
                        && self.drag.is_none()
                        && ui.hovered(ctx, &node, cursor)
                    {
                        self.drag = Some(i);
                    }
                    let mut value = ui.draft.number(def.key);
                    let path = format!("{PATH}.window.{node}");
                    if self.drag == Some(i) {
                        if let Some(r) = ctx.ui_slider_ratio(&path).filter(|r| r.is_finite()) {
                            ui.draft.set(def.key, lo + r.clamp(0., 1.) * (hi - lo));
                            value = ui.draft.number(def.key);
                        }
                    } else {
                        ctx.ui_set_slider_ratio(&path, (value - lo) / (hi - lo));
                    }
                    let ratio = (value - lo) / (hi - lo);
                    ui.props(
                        ctx,
                        &format!("{node}.fill"),
                        format!("width: {:.2}%;", ratio * 100.),
                    );
                    ui.props(
                        ctx,
                        &format!("{node}.thumb"),
                        format!("x: {:.2}px;", (ratio * 378.).clamp(9., 369.)),
                    );
                    ui.text(
                        ctx,
                        &format!("{row}.value"),
                        &if step < 1. {
                            format!("{value:.2}{unit}")
                        } else {
                            format!("{}{unit}", value as i32)
                        },
                    );
                }
                _ => {}
            }
        }
        ui.paint(
            ctx,
            "emote_panel.display.replay",
            true,
            cursor,
            (crate::ui_theme::tone(0x393939ff), 0x707070ff, 0x5b5b5bff),
            (crate::ui_theme::tone(0x6d6c6aff), 1),
            0xeeececff,
        );
        for i in 0..ZOOMS.len() {
            ui.paint(
                ctx,
                &format!("emote_panel.display.zoom{i}"),
                true,
                cursor,
                (
                    if self.zoom == i {
                        0xfdee00ff
                    } else {
                        crate::ui_theme::tone(0x393939ff)
                    },
                    0x707070ff,
                    0x5b5b5bff,
                ),
                (crate::ui_theme::tone(0x6d6c6aff), 1),
                if self.zoom == i {
                    0x393939ff
                } else {
                    0xeeececff
                },
            );
        }
        let config = Config::from_values(&ui.draft);
        let zoom = ZOOMS[self.zoom];
        let fit = PREVIEW_SCALE;
        let age = self.started.elapsed().as_secs_f32() % 2.6;
        let (r, alpha) = config.geometry((0., 0.), age, zoom);
        let pan = preview_pan(config, zoom);
        let anchor = (PREVIEW_W / 2., PREVIEW_H - 28. + pan);
        let art = self.library.preview(&ui.draft);
        ui.props(
            ctx,
            "emote_panel.display.scene.emote",
            format!("source: {};", json(&art.source)),
        );
        ui.props(ctx, "emote_panel.display.scene.emote", format!("x: {:.2}px; y: {:.2}px; width: {:.2}px; height: {:.2}px; color: #ffffff{alpha:02x};", anchor.0 + r.x * fit, anchor.1 + r.y * fit, r.w * fit, r.h * fit));
        // Scaling a parent does not scale children in this UI. Size each marker part explicitly.
        ui.visible(ctx, "emote_panel.display.scene.champion", pan == 0.);
        ui.visible(ctx, "emote_panel.display.scene.ground", pan == 0.);
        for (node, (x, y, w, h)) in [
            ("head", (18., 0., 28., 28.)),
            ("body", (8., 32., 48., 44.)),
            ("health", (0., -12., 64., 5.)),
        ] {
            let factor = zoom * fit;
            ui.props(
                ctx,
                &format!("emote_panel.display.scene.champion.{node}"),
                format!(
                    "x: {:.2}px; y: {:.2}px; width: {:.2}px; height: {:.2}px;",
                    anchor.0 + (x - 32.) * factor,
                    anchor.1 + (y - 76.) * factor,
                    w * factor,
                    h * factor
                ),
            );
        }
        let (settled, _) = config.geometry((0., 0.), 1., zoom);
        ui.text(
            ctx,
            "emote_panel.display.metrics",
            &format!(
                "Image {:.0} px · height {:.0} px{}",
                settled.w,
                -settled.y - settled.h,
                if pan > 0. {
                    " · champion below preview"
                } else {
                    ""
                }
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_preview_layout() {
        let Ok(dir) = std::env::var("LT_HUD_EXPORT_DIR") else {
            return;
        };
        let mut views = Vec::new();
        for (name, height, scale, follow, zoom) in [
            ("default", 108., 100., 0., 1.),
            ("fixed-zoom-out", 108., 100., 0., 0.5),
            ("follow-zoom-out", 108., 100., 1., 0.5),
            ("follow-zoom-200", 108., 100., 1., 2.),
            ("follow-zoom-in", 108., 100., 1., 3.),
            ("largest", 240., 200., 1., 3.),
            ("lowest", 0., 50., 0., 1.),
        ] {
            let mut values = Values::default();
            values.set("emote_height", height);
            values.set("emote_scale", scale);
            values.set("emote_zoom", follow);
            let config = Config::from_values(&values);
            let fit = PREVIEW_SCALE;
            let (r, _) = config.geometry((0., 0.), 1., zoom);
            let pan = preview_pan(config, zoom);
            let anchor = (PREVIEW_W / 2., PREVIEW_H - 28. + pan);
            views.push(serde_json::json!({"name": name, "height": height, "scale": scale, "follow": follow, "zoom": zoom, "fit": fit,
                "anchor": [anchor.0,anchor.1], "pan":pan,
                "rect": [anchor.0+r.x*fit, anchor.1+r.y*fit, r.w*fit, r.h*fit],
                "options": definitions().map(|(i,d)| serde_json::json!({"index":i,"key":d.key,"value":values.number(d.key),"choices":match d.control { Control::Toggle => Some(&["Off","On"][..]),Control::Choice(v)=>Some(v),_=>None },"slider":match d.control {Control::Slider(lo,hi,_,_)=>Some([lo,hi]),_=>None}})).collect::<Vec<_>>() }));
        }
        std::fs::write(
            std::path::Path::new(&dir).join("emote-previews.json"),
            serde_json::to_vec_pretty(&views).unwrap(),
        )
        .unwrap();
    }
    #[test]
    fn fixed_scale_preview_pans_to_fit_all_limits_without_hiding_zoom_differences() {
        let mut values = Values::default();
        for height in [0., 108., 240.] {
            for scale in [50., 100., 200.] {
                for mode in [0., 1.] {
                    for zoom in ZOOMS {
                        values.set("emote_height", height);
                        values.set("emote_scale", scale);
                        values.set("emote_zoom", mode);
                        let config = Config::from_values(&values);
                        let fit = PREVIEW_SCALE;
                        let pan = preview_pan(config, zoom);
                        assert!(fit.is_finite() && fit > 0.);
                        for n in 0..=120 {
                            let (r, _) = config.geometry((0., 0.), n as f32 / 60., zoom);
                            assert!(
                                PREVIEW_H - 28. + pan + r.y * fit >= 0.,
                                "height={height} scale={scale} mode={mode} zoom={zoom}"
                            );
                            assert!(r.w * fit <= PREVIEW_W - 48.);
                            assert!(PREVIEW_H - 28. + pan + (r.y + r.h) * fit <= PREVIEW_H);
                        }
                    }
                }
            }
        }
        values.set("emote_zoom", 1.);
        let config = Config::from_values(&values);
        assert!(
            config.geometry((0., 0.), 1., 3.).0.w * PREVIEW_SCALE
                > config.geometry((0., 0.), 1., 2.).0.w * PREVIEW_SCALE
        );
    }
    #[test]
    fn preview_zoom_does_not_edit_game_settings_and_hidden_choices_are_bounded() {
        let mut panel = Panel {
            tab: 1,
            ..Default::default()
        };
        let mut draft = Values::default();
        let before = draft.clone();
        panel.handle(Action::Zoom(3), &mut draft);
        panel.handle(Action::Replay, &mut draft);
        assert_eq!(draft.0, before.0);
        panel.handle(Action::Choice(0, 1), &mut draft);
        assert_eq!(draft.0, before.0);
        panel.handle(Action::Zoom(99), &mut draft);
        assert_eq!(panel.zoom, 3);
    }
    #[test]
    fn hidden_tab_events_cannot_edit_emote_options_or_assignments() {
        let mut panel = Panel::default();
        let mut draft = Values::default();
        let emotes = definitions().find(|(_, d)| d.key == "emotes").unwrap().0;
        panel.handle(Action::Choice(emotes, 0), &mut draft);
        assert_eq!(draft.number("emotes"), 1.);
        panel.handle(Action::Tab(1), &mut draft);
        panel.handle(Action::Choice(emotes, 0), &mut draft);
        assert_eq!(draft.number("emotes"), 0.);
        let before = draft.0.clone();
        panel.handle(Action::Library(library::Action::Assign), &mut draft);
        panel.handle(Action::Tab(99), &mut draft);
        assert_eq!(draft.0, before);
        assert_eq!(panel.tab, 1);
    }
}
