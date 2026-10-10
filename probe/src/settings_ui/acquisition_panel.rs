//! Fixed Advanced body. Only complete portrait rows scroll; edits stay in the draft.
use super::*;
use crate::acquisition::{self, Champion};
const COLS: usize = 8;
pub(super) const HEIGHT: i32 = 475;
const ROWS: usize = 5;
const CELLS: usize = COLS * ROWS;
const PITCH: i32 = 64;
const GRID_W: i32 = (COLS as i32 - 1) * PITCH + 58;
const GRID_Y: i32 = 112;
#[derive(Clone, Copy)]
pub(super) enum Action {
    Select(usize),
    Up,
    Down,
    Automatic,
    Custom,
    Reset,
    SharedReset,
}
#[derive(Default)]
pub(super) struct Panel {
    name: String,
    query: String,
    offset: usize,
    slots: Vec<String>,
    faces: Vec<String>,
    reset: bool,
    visible: bool,
    last_number: String,
    reset_defaults: bool,
    shared_text: [String; 2],
    shared_invalid: [bool; 2],
    invalid_radius: bool,
    pub invalid: bool,
}
pub(super) fn events() -> Vec<(String, Event)> {
    let mut items: Vec<_> = [
        ("up", Action::Up),
        ("down", Action::Down),
        ("automatic", Action::Automatic),
        ("custom", Action::Custom),
        ("reset", Action::Reset),
        ("shared_reset", Action::SharedReset),
    ]
    .into_iter()
    .map(|(n, a)| (format!("acquisition.{n}"), Event::Acquisition(a)))
    .collect();
    items.extend((0..CELLS).map(|i| {
        (
            format!("acquisition.cell{i}"),
            Event::Acquisition(Action::Select(i)),
        )
    }));
    items
}
pub(super) fn edit(name: &str, (x, y, w, h): (i32, i32, i32, i32), placeholder: &str) -> String {
    // TextEdit's native commands use fixed z=100; its `z` property is ignored.
    // Keep native editing/IME state, and mirror committed text above the modal.
    let mut s = rect(
        &format!("{name}_shadow"),
        (x - 1, y + 3, w + 2, h + 1),
        "00000050",
        2006,
    );
    s.push_str(&format!("#{name}_plate:color {{ x: {x}px; y: {y}px; width: {w}px; height: {h}px; color: #~6d6c6aff; back_color: #~242221ff; stroke: 1; rounding: Uniform {{ rounding: 2; }} ignore_event: true; z: 2007; }}"));
    s.push_str(&format!("#{name}:text_edit {{ @\"asset/base/style/main#text_edit\"; x: {x}px; y: {y}px; width: {w}px; height: {h}px; size: 18; max_length: 64; align_y: Center; placeholder: {}; padding: {{ left: 14px; right: 14px; top: 4px; bottom: 4px; }} }}", json(placeholder)));
    s.push_str(&label(
        &format!("{name}_value"),
        (x + 14, y, w - 28, h),
        18,
        placeholder,
        false,
        "989694ff",
        "Left",
        2009,
    ));
    s
}
/// The plate of an `edit` field: a yellow frame and a lighter fill while it
/// has keyboard focus. The game reports no caret position, so none is drawn.
pub(super) fn edit_plate(editing: bool) -> String {
    if editing {
        "color: #fdee00ff; back_color: #~3a3836ff; stroke: 2;".into()
    } else {
        "color: #~6d6c6aff; back_color: #~242221ff; stroke: 1;".into()
    }
}
fn field_text(text: &str, width: f32) -> String {
    let mut text = text.to_owned();
    let mut clipped = false;
    while crate::hud_style::width(&text, 18.) > width - 18. && !text.is_empty() {
        text.drain(..text.chars().next().unwrap().len_utf8());
        clipped = true;
    }
    if clipped {
        text.insert(0, '…');
    }
    text
}
pub(super) fn template() -> String {
    let bg = crate::ui_theme::hex(0xff);
    let mut s = format!("#acquisition:color {{ x: {CONTENT_X}px; y: 0px; width: {CONTENT_W}px; height: {HEIGHT}px; color: #{bg}; ignore_event: true; visible: false; z: 2003;");
    for (name, at, size, copy, bold, color) in [
        (
            "search_label",
            (0, 0, 540, 26),
            17,
            tr("Find champion"),
            true,
            "eeececff",
        ),
        (
            "search_hint",
            (0, 82, 540, 22),
            13,
            tr("Search localized name, English name or internal ID"),
            false,
            "989694ff",
        ),
        (
            "shared_heading",
            (604, 0, 330, 24),
            16,
            tr("Automatic defaults · all champions"),
            true,
            "eeececff",
        ),
        (
            "minimum_label",
            (604, 28, 194, 20),
            14,
            tr("Minimum · game units"),
            false,
            "989694ff",
        ),
        (
            "buffer_label",
            (824, 28, 194, 20),
            14,
            tr("AA buffer · game units"),
            false,
            "989694ff",
        ),
        ("count", (0, 439, 400, 28), 14, "", false, "989694ff"),
        ("champion", (604, 113, 414, 28), 21, "", true, "ffffffff"),
        ("id", (604, 146, 282, 22), 13, "", false, "989694ff"),
        (
            "radius_label",
            (604, 239, 414, 24),
            17,
            tr("Acquisition radius · game units"),
            true,
            "eeececff",
        ),
        ("range", (604, 365, 202, 26), 17, "", false, "eeececff"),
        ("maximum", (816, 365, 202, 26), 17, "", false, "eeececff"),
        ("effective", (604, 397, 414, 26), 19, "", true, "fdee00ff"),
        ("note", (604, 433, 414, 38), 13, "", false, "989694ff"),
    ] {
        s.push_str(&label(name, at, size, copy, bold, color, "Left", 2009));
    }
    s.push_str(&edit(
        "search",
        (0, 32, GRID_W, 44),
        tr("Type a champion name…"),
    ));
    s.push_str(&rect("divider", (574, 0, 1, 470), "~4b4a49ff", 2004));
    s.push_str("#grid_hit:color { x: 0px; y: 112px; width: 540px; height: 314px; color: #00000000; ignore_event: true; z: 2003; }");
    for i in 0..CELLS {
        let x = (i % COLS) as i32 * PITCH;
        let y = GRID_Y + (i / COLS) as i32 * PITCH;
        let children = "#face:image { x: 29px; y: 29px; pivot_x: 0.5; pivot_y: 0.5; width: 52px; height: 52px; ignore_event: true; z: 2008; }";
        s.push_str(&button(
            &format!("cell{i}"),
            (x, y, 58, 58),
            2,
            "",
            14,
            2006,
            children,
        ));
    }
    for (name, at, round, copy, size) in [
        ("up", (440, 438, 44, 30), 2, "‹", 23),
        ("down", (496, 438, 44, 30), 2, "›", 23),
        ("automatic", (604, 184, 202, 40), 20, tr("Automatic"), 18),
        ("custom", (816, 184, 202, 40), 20, tr("Custom"), 18),
        ("reset", (914, 144, 104, 28), 2, tr("Reset"), 15),
        ("shared_reset", (950, 0, 68, 25), 2, tr("Reset"), 14),
    ] {
        s.push_str(&raised_button(name, at, round, copy, size, 2005, ""));
    }
    s.push_str(&edit("minimum", (604, 54, 194, 40), tr("Minimum")));
    s.push_str(&edit("buffer", (824, 54, 194, 40), tr("Buffer")));
    s.push_str(&rect("shared_line", (604, 104, 414, 1), "~4b4a49ff", 2004));
    s.push_str(&edit("number", (604, 270, 414, 42), tr("Enter radius")));
    s.push_str(&label(
        "automatic_value",
        (618, 270, 382, 42),
        20,
        "",
        true,
        "eeececff",
        "Left",
        2009,
    ));
    s.push_str("#slider:slider { x: 604px; y: 320px; width: 414px; height: 30px; z: 2006; background: Color { prop: { color: #00000000; } } foreground: Color { prop: { color: #00000000; } } view_min_ratio: 0.0; ratio: 0.0; #track:color { y: 12px; width: 100%; height: 6px; color: #~6d6c6aff; ignore_event: true; z: 2007; } #fill:color { y: 12px; width: 50%; height: 6px; color: #fdee00ff; ignore_event: true; z: 2008; } #thumb:color { x: 0px; y: 6px; width: 18px; height: 18px; pivot_x: 0.5; rounding: Uniform { rounding: 9; } color: #eeececff; ignore_event: true; z: 2009; } }");
    s.push_str("}\n");
    s
}
fn parse_radius(text: &str) -> Option<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && (0. ..=acquisition::MAX_RADIUS).contains(n))
}
fn last_row(len: usize) -> usize {
    len.div_ceil(COLS).saturating_sub(ROWS)
}
impl Panel {
    pub fn hide(&mut self) {
        self.visible = false;
    }
    pub fn new() -> Self {
        let name = acquisition::controlled().unwrap_or_default();
        let rows = acquisition::champions();
        let offset = rows
            .iter()
            .position(|r| r.name == name)
            .map(|i| (i / COLS).min(last_row(rows.len())))
            .unwrap_or(0);
        Self {
            name,
            offset,
            reset: true,
            reset_defaults: true,
            ..Self::default()
        }
    }
    pub fn reset_fields(&mut self) {
        self.reset = true;
        self.invalid_radius = false;
        self.validate();
    }
    pub fn reset_all(&mut self) {
        self.reset_defaults = true;
        self.shared_invalid = [false; 2];
        self.reset_fields();
    }
    fn validate(&mut self) {
        self.invalid = self.invalid_radius || self.shared_invalid.iter().any(|v| *v);
    }
    fn rows(&self) -> Vec<Champion> {
        acquisition::champions()
            .into_iter()
            .filter(|c| acquisition::matches(c, &self.query))
            .collect()
    }
    fn ensure_name(&mut self) {
        let rows = self.rows();
        self.offset = self.offset.min(last_row(rows.len()));
        if !rows.iter().any(|c| c.name == self.name) {
            let name = rows.first().map(|c| c.name.clone()).unwrap_or_default();
            if self.name != name {
                self.name = name;
                self.reset_fields();
            }
        }
    }
    pub fn scroll(&mut self, notches: i32) {
        self.offset = (self.offset as i64 - i64::from(notches))
            .clamp(0, last_row(self.rows().len()) as i64) as usize;
    }
    pub fn handle(&mut self, action: Action, draft: &mut Values) {
        if !self.visible {
            return;
        }
        // Select through last frame's slot map, not the newly filtered list.
        if let Action::Select(i) = action {
            if let Some(name) = self.slots.get(i).filter(|n| !n.is_empty()) {
                self.name = name.clone();
                self.reset_fields();
            }
            return;
        }
        if matches!(action, Action::Up | Action::Down) {
            self.scroll(if matches!(action, Action::Up) { 1 } else { -1 });
            return;
        }
        if matches!(action, Action::SharedReset) {
            acquisition::set_shared(draft, true, acquisition::BASELINE);
            acquisition::set_shared(draft, false, acquisition::BUFFER);
            self.reset_defaults = true;
            self.shared_invalid = [false; 2];
            self.validate();
            return;
        }
        self.ensure_name();
        if self.name.is_empty() {
            return;
        }
        match action {
            Action::Automatic | Action::Reset => acquisition::set_custom(draft, &self.name, None),
            Action::Custom if acquisition::custom(draft, &self.name).is_none() => {
                let c = acquisition::champion(&self.name);
                let n = acquisition::radius(
                    draft,
                    &self.name,
                    c.current.or(c.base).unwrap_or(0),
                    c.maximum,
                ) as f64
                    / 1000.;
                acquisition::set_custom(draft, &self.name, Some(n));
            }
            _ => {}
        }
        self.reset_fields();
    }
    pub fn sync(&mut self, ctx: &StableClient<'_>, draft: &mut Values) {
        if !self.visible {
            return;
        }
        if !self.reset_defaults {
            for (i, field) in ["minimum", "buffer"].into_iter().enumerate() {
                if let Some(text) =
                    ctx.ui_text_edit_text(&format!("{PATH}.window.acquisition.{field}"))
                {
                    if text != self.shared_text[i] {
                        self.shared_text[i] = text.clone();
                        let n = parse_radius(&text);
                        self.shared_invalid[i] = n.is_none();
                        if let Some(n) = n {
                            acquisition::set_shared(draft, i == 0, n);
                        }
                    }
                }
            }
        }
        if self.reset {
            self.validate();
            return;
        }
        if let Some(text) = ctx.ui_text_edit_text(&format!("{PATH}.window.acquisition.number")) {
            if text != self.last_number && acquisition::custom(draft, &self.name).is_some() {
                self.last_number = text.clone();
                let n = parse_radius(&text);
                self.invalid_radius = n.is_none();
                if let Some(n) = n {
                    acquisition::set_custom(draft, &self.name, Some(n));
                }
            }
        }
        if let Some(q) = ctx.ui_text_edit_text(&format!("{PATH}.window.acquisition.search")) {
            if q != self.query {
                self.query = q;
                self.offset = 0;
                self.ensure_name();
            }
        }
        self.validate();
    }
    fn render_portrait(
        &mut self,
        ui: &mut SettingsUi,
        ctx: &mut StableClient<'_>,
        i: usize,
        name: &str,
        cursor: Option<(f32, f32)>,
    ) {
        let node = format!("acquisition.cell{i}");
        if name.is_empty() {
            ui.props(ctx, &node, "visible: false; ignore_event: true;".into());
            ui.visible(ctx, &format!("{node}.face"), false);
            ui.text(ctx, &format!("{node}.text"), "");
            self.slots[i].clear();
            self.faces[i].clear();
            return;
        }
        let selected = name == self.name;
        ui.paint(
            ctx,
            &node,
            true,
            cursor,
            (
                crate::ui_theme::tone(0x292726ff),
                crate::ui_theme::tone(0x5b5958ff),
                crate::ui_theme::tone(0x4b4a49ff),
            ),
            (
                if selected {
                    0xfdee00ff
                } else {
                    crate::ui_theme::tone(0x6d6c6aff)
                },
                if selected { 2 } else { 1 },
            ),
            0xeeececff,
        );
        if self.faces[i] != name && !name.is_empty() {
            let ok =
                ctx.ui_set_champion_icon(&format!("{PATH}.window.{node}.face"), name, 52., 52., 2.);
            ui.visible(ctx, &format!("{node}.face"), ok);
            ui.text(ctx, &format!("{node}.text"), if ok { "" } else { "?" });
            self.faces[i] = name.into();
        }
        self.slots[i] = name.into();
    }
    pub fn render(
        &mut self,
        ui: &mut SettingsUi,
        ctx: &mut StableClient<'_>,
        y: i32,
        cursor: Option<(f32, f32)>,
        keys: Keys,
    ) {
        self.visible = true;
        self.ensure_name();
        ui.props(ctx, "acquisition", format!("visible: true; y: {y}px;"));
        let rows = self.rows();
        let present = !self.name.is_empty();
        let c = acquisition::champion(&self.name);
        let custom = acquisition::custom(&ui.draft, &self.name);
        let effective = acquisition::radius(
            &ui.draft,
            &self.name,
            c.current.or(c.base).unwrap_or(0),
            c.maximum,
        );
        let chosen = custom.unwrap_or(effective as f64 / 1000.);
        ui.text(
            ctx,
            "acquisition.champion",
            if present {
                if c.label.is_empty() {
                    &c.name
                } else {
                    &c.label
                }
            } else {
                tr("No matching champions")
            },
        );
        ui.text(ctx, "acquisition.id", &c.name);
        ui.text(
            ctx,
            "acquisition.count",
            &trf(
                "Champions: {count} · rows {first}–{last} / {total}",
                &[
                    ("count", &rows.len()),
                    ("first", &if rows.is_empty() { 0 } else { self.offset + 1 }),
                    ("last", &(self.offset + ROWS).min(rows.len().div_ceil(COLS))),
                    ("total", &rows.len().div_ceil(COLS)),
                ],
            ),
        );
        self.slots.resize(CELLS, String::new());
        self.faces.resize(CELLS, String::new());
        for i in 0..CELLS {
            let name = rows
                .get(self.offset * COLS + i)
                .map(|c| c.name.as_str())
                .unwrap_or("");
            self.render_portrait(ui, ctx, i, name, cursor);
        }
        for (name, selected, on) in [
            ("automatic", custom.is_none(), present),
            ("custom", custom.is_some(), present),
            ("reset", false, present),
            ("shared_reset", false, true),
            ("up", false, self.offset > 0),
            ("down", false, self.offset < last_row(rows.len())),
        ] {
            let colors = if selected {
                (0xeeececff, 0xb8b6b5ff, 0x989694ff)
            } else {
                (
                    crate::ui_theme::tone(0x3a3837ff),
                    crate::ui_theme::tone(0x5b5958ff),
                    crate::ui_theme::tone(0x4b4a49ff),
                )
            };
            ui.paint(
                ctx,
                &format!("acquisition.{name}"),
                on,
                cursor,
                colors,
                (
                    if selected {
                        0xfdee00ff
                    } else {
                        crate::ui_theme::tone(0x6d6c6aff)
                    },
                    1,
                ),
                if selected { 0x393939ff } else { 0xeeececff },
            );
        }
        let ceiling = ((chosen.max(effective as f64 / 1000.).max(150.) / 50.).ceil() * 50.)
            .min(acquisition::MAX_RADIUS);
        let ratio = (chosen / ceiling).clamp(0., 1.);
        let path = format!("{PATH}.window.acquisition.slider");
        let editable = present
            && custom.is_some()
            && ui.drop.is_none()
            && ui.pending.is_none()
            && ui.capture.is_none();
        if editable
            && keys.focused
            && keys.raw.0[1]
            && ui.hovered(ctx, "acquisition.slider", cursor)
        {
            if let Some(r) = ctx.ui_slider_ratio(&path).filter(|r| r.is_finite()) {
                acquisition::set_custom(
                    &mut ui.draft,
                    &self.name,
                    Some((r.clamp(0., 1.) * ceiling).round()),
                );
                self.reset_fields();
            }
        } else {
            ctx.ui_set_slider_ratio(&path, ratio);
        }
        ui.props(
            ctx,
            "acquisition.slider",
            format!("ignore_event: {};", !editable),
        );
        ui.props(
            ctx,
            "acquisition.slider.fill",
            format!(
                "width: {:.2}%; color: #{};",
                ratio * 100.,
                if editable { "fdee00ff" } else { "989694ff" }
            ),
        );
        ui.props(
            ctx,
            "acquisition.slider.thumb",
            format!("x: {:.2}px;", (ratio * 414.).clamp(9., 405.)),
        );
        ui.props(
            ctx,
            "acquisition.number",
            format!(
                "visible: {}; ignore_event: {};",
                custom.is_some(),
                !editable
            ),
        );
        ui.visible(ctx, "acquisition.automatic_value", custom.is_none());
        ui.text(
            ctx,
            "acquisition.automatic_value",
            &format!("{chosen:.1} · Automatic"),
        );
        if self.reset_defaults {
            self.shared_text = [
                format!("{:.1}", acquisition::baseline(&ui.draft, "")),
                format!("{:.1}", acquisition::buffer(&ui.draft)),
            ];
            for (i, name) in ["minimum", "buffer"].into_iter().enumerate() {
                ctx.ui_set_text_edit_text(
                    &format!("{PATH}.window.acquisition.{name}"),
                    &self.shared_text[i],
                );
            }
            self.reset_defaults = false;
        }
        if self.reset {
            self.last_number = format!(
                "{:.1}",
                acquisition::custom(&ui.draft, &self.name).unwrap_or(chosen)
            );
            ctx.ui_set_text_edit_text(
                &format!("{PATH}.window.acquisition.number"),
                &self.last_number,
            );
            ctx.ui_set_text_edit_text(&format!("{PATH}.window.acquisition.search"), &self.query);
            self.reset = false;
        }
        for (name, value, placeholder, width, shown) in [
            (
                "minimum",
                self.shared_text[0].as_str(),
                tr("Minimum"),
                166.,
                true,
            ),
            (
                "buffer",
                self.shared_text[1].as_str(),
                tr("Buffer"),
                166.,
                true,
            ),
            (
                "search",
                self.query.as_str(),
                tr("Type a champion name…"),
                (GRID_W - 28) as f32,
                true,
            ),
            (
                "number",
                self.last_number.as_str(),
                tr("Enter radius"),
                386.,
                custom.is_some(),
            ),
        ] {
            let path = format!("{PATH}.window.acquisition.{name}");
            let editing = ctx
                .ui_state_json(&path)
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|v| v.get("is_editing").and_then(serde_json::Value::as_bool))
                .unwrap_or(false);
            let label = format!("acquisition.{name}_value");
            ui.visible(ctx, &label, shown);
            ui.text(
                ctx,
                &label,
                &field_text(if value.is_empty() { placeholder } else { value }, width),
            );
            ui.props(
                ctx,
                &label,
                format!(
                    "color: #{};",
                    if value.is_empty() {
                        "989694ff"
                    } else {
                        "eeececff"
                    }
                ),
            );
            ui.props(
                ctx,
                &format!("acquisition.{name}_plate"),
                edit_plate(shown && editing),
            );
        }
        let reach = c
            .current
            .or(c.base)
            .map(|n| format!("{:.1}", n as f64 / 1000.))
            .unwrap_or_else(|| tr("Unavailable").into());
        let maximum = c
            .maximum
            .map(|n| format!("{:.1}", n as f64 / 1000.))
            .unwrap_or_else(|| tr("Unavailable").into());
        ui.text(
            ctx,
            "acquisition.range",
            &if c.current.is_some() {
                trf("Live AA: {value}", &[("value", &reach)])
            } else {
                trf("Base AA: {value}", &[("value", &reach)])
            },
        );
        ui.text(
            ctx,
            "acquisition.maximum",
            &trf("Max AA: {value}", &[("value", &maximum)]),
        );
        ui.text(
            ctx,
            "acquisition.effective",
            &trf(
                "Effective radius: {value}",
                &[("value", &format!("{:.1}", effective as f64 / 1000.))],
            ),
        );
        let note = if self.invalid {
            tr("Enter a number from 0 to 10,000 before applying.").into()
        } else if custom.is_some() {
            tr("Never below current AA reach. Your chosen value is preserved.").into()
        } else if c.maximum.is_none() {
            trf(
                "Maximum unknown: current AA reach + {buffer}, minimum {minimum}.",
                &[
                    ("buffer", &format!("{:.1}", acquisition::buffer(&ui.draft))),
                    (
                        "minimum",
                        &format!("{:.1}", acquisition::baseline(&ui.draft, &self.name)),
                    ),
                ],
            )
        } else {
            trf(
                "Automatic uses maximum AA reach + {buffer}, minimum {minimum}.",
                &[
                    ("buffer", &format!("{:.1}", acquisition::buffer(&ui.draft))),
                    (
                        "minimum",
                        &format!("{:.1}", acquisition::baseline(&ui.draft, &self.name)),
                    ),
                ],
            )
        };
        let (note, _) = crate::hud_style::wrap(&note, 13., 414.);
        ui.text(ctx, "acquisition.note", &note);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_portrait_rows_fit_and_scrolling_reaches_every_champion() {
        for i in 0..CELLS {
            assert!(GRID_Y + (i / COLS) as i32 * PITCH + 58 <= 438);
            assert!((i % COLS) as i32 * PITCH + 58 <= 540);
        }
        for len in 0usize..=1000 {
            assert!(last_row(len) * COLS < len.max(1));
            assert!(last_row(len) * COLS + CELLS >= len);
        }
    }
    #[test]
    fn recycled_portrait_slot_hides_and_clears_then_becomes_clickable_again() {
        unsafe extern "C" fn properties(
            _: *mut std::ffi::c_void,
            _: *const u8,
            _: usize,
            _: *const u8,
            _: usize,
        ) -> bool {
            true
        }
        unsafe extern "C" fn icon(
            _: *mut std::ffi::c_void,
            _: *const u8,
            _: usize,
            _: *const u8,
            _: usize,
            _: f32,
            _: f32,
            _: f32,
        ) -> bool {
            true
        }
        let mut host: mod_api_stable::UiVtableV1 = unsafe { std::mem::zeroed() };
        host.size = std::mem::size_of_val(&host);
        host.set_properties = Some(properties);
        host.set_text = Some(properties);
        host.set_champion_icon = Some(icon);
        let mut raw: mod_api_stable::ClientCtxV1 = unsafe { std::mem::zeroed() };
        raw.size = std::mem::size_of_val(&raw);
        raw.ui = &host;
        let mut ctx = unsafe { StableClient::from_raw(&mut raw) }.unwrap();
        let mut ui = SettingsUi::default();
        let mut panel = Panel {
            visible: true,
            slots: vec![String::new(); CELLS],
            faces: vec![String::new(); CELLS],
            ..Default::default()
        };
        let i = CELLS - 1;
        let path = format!("{PATH}.window.acquisition.cell{i}");
        panel.render_portrait(&mut ui, &mut ctx, i, "ghost", None);
        assert_eq!(panel.faces[i], "ghost");
        panel.render_portrait(&mut ui, &mut ctx, i, "", None);
        assert_eq!(ui.cache[&format!("field:{path}:visible")], "false");
        assert_eq!(ui.cache[&format!("field:{path}.face:visible")], "false");
        assert!(panel.slots[i].is_empty() && panel.faces[i].is_empty());
        panel.name = "soldier".into();
        panel.handle(Action::Select(i), &mut Values::default());
        assert_eq!(panel.name, "soldier");
        panel.render_portrait(&mut ui, &mut ctx, i, "ghost", None);
        assert_eq!(ui.cache[&format!("field:{path}:visible")], "true");
        assert_eq!(ui.cache[&format!("field:{path}:ignore_event")], "false");
        assert_eq!(ui.cache[&format!("field:{path}.face:visible")], "true");
        panel.handle(Action::Select(i), &mut Values::default());
        assert_eq!(panel.name, "ghost");
    }
    #[test]
    fn shared_reset_and_champion_changes_preserve_independent_values_and_validation() {
        let mut values = Values::default();
        acquisition::set_shared(&mut values, true, 90.);
        acquisition::set_shared(&mut values, false, 12.);
        acquisition::set_custom(&mut values, "ghost", Some(80.));
        let mut panel = Panel {
            visible: true,
            shared_invalid: [true, false],
            ..Default::default()
        };
        panel.reset_fields();
        assert!(
            panel.invalid,
            "changing champion cannot clear an invalid shared field"
        );
        panel.handle(Action::SharedReset, &mut values);
        assert!(!panel.invalid);
        assert_eq!(acquisition::baseline(&values, "ghost"), 120.);
        assert_eq!(acquisition::buffer(&values), 5.);
        assert_eq!(acquisition::custom(&values, "ghost"), Some(80.));
        for text in ["", "-1", "NaN", "inf", "10001"] {
            assert!(parse_radius(text).is_none(), "{text}");
        }
        for (text, value) in [("0", 0.), (" 72.5 ", 72.5), ("10000", 10000.)] {
            assert_eq!(parse_radius(text), Some(value));
        }
    }
    #[test]
    fn portrait_events_use_rendered_slot_even_if_search_changes() {
        let mut panel = Panel {
            visible: true,
            slots: vec!["soldier".into()],
            query: "something else".into(),
            ..Panel::default()
        };
        panel.handle(Action::Select(0), &mut Values::default());
        assert_eq!(panel.name, "soldier");
        panel.handle(Action::Select(50), &mut Values::default());
        assert_eq!(panel.name, "soldier");
    }
}
