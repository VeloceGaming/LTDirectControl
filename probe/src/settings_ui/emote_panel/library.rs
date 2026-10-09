//! Library navigation and slot assignment edit only the Settings draft.
use super::*;
use crate::emote_library::{self, Art, SLOT_NAMES};
const CELLS: usize = 24;
const PAGE_SIZE: usize = 24;
#[derive(Clone, Copy)]
pub(in crate::settings_ui) enum Action {
    Select(usize),
    Slot(usize),
    Assign,
    Previous,
    Next,
    Refresh,
    OpenFolder,
    Issue,
}
#[derive(Default)]
pub(super) struct Panel {
    selected: Option<String>,
    slot: usize,
    page: usize,
    cells: Vec<String>,
    issue: usize,
    message: String,
    revision: u64,
}
fn event(action: Action) -> Event {
    Event::Emote(super::Action::Library(action))
}
pub(super) fn events() -> Vec<(String, Event)> {
    let mut out: Vec<_> = [
        ("assign", Action::Assign),
        ("previous", Action::Previous),
        ("next", Action::Next),
        ("refresh", Action::Refresh),
        ("open", Action::OpenFolder),
        ("issue", Action::Issue),
    ]
    .into_iter()
    .map(|(n, a)| (format!("emote_panel.library.{n}"), event(a)))
    .collect();
    out.extend((0..CELLS).map(|i| {
        (
            format!("emote_panel.library.cell{i}"),
            event(Action::Select(i)),
        )
    }));
    out.extend((0..5).map(|i| {
        (
            format!("emote_panel.library.slot{i}"),
            event(Action::Slot(i)),
        )
    }));
    out
}
pub(super) fn template() -> String {
    let mut s = String::from("#library:empty { x: 0px; y: 64px; width: 1042px; height: 456px; visible: false; ignore_event: true; z: 2004;");
    for (name, at, copy) in [
        ("open", (0, 0, 150, 32), tr("Open folder").to_owned()),
        ("refresh", (162, 0, 136, 32), tr("Refresh").to_owned()),
        ("previous", (364, 380, 44, 30), "‹".to_owned()),
        ("next", (416, 380, 44, 30), "›".to_owned()),
        (
            "assign",
            (590, 338, 452, 40),
            trf("Assign to {slot}", &[("slot", &tr(SLOT_NAMES[0]))]),
        ),
        ("issue", (0, 420, 90, 28), tr("Issues").to_owned()),
    ] {
        s.push_str(&raised_button(name, at, 2, &copy, 15, 2006, ""));
    }
    for (name, at, size, copy, color) in [
        (
            "guide",
            (0, 40, 548, 22),
            13,
            tr("Static PNG · up to 256 × 256 · 1 MiB · 64 imports"),
            "a3a19fff",
        ),
        ("count", (0, 382, 350, 26), 14, "", "cbc9c7ff"),
        ("issue_text", (102, 418, 446, 34), 12, "", "e9b16cff"),
        (
            "wheel_title",
            (590, 0, 452, 26),
            19,
            tr("Choose a wheel slot"),
            "eeececff",
        ),
        ("selected", (590, 300, 452, 30), 19, "", "eeececff"),
        ("status", (590, 388, 452, 26), 13, "", "fdee00ff"),
        (
            "workflow",
            (590, 418, 452, 34),
            12,
            tr("Refresh imports, assign, Apply, then restart the game."),
            "a3a19fff",
        ),
    ] {
        s.push_str(&label(
            name,
            at,
            size,
            copy,
            name == "wheel_title",
            color,
            "Left",
            2006,
        ));
    }
    for i in 0..CELLS {
        let children = format!(
            "{}{}",
            image("face", "emote_gg", (4, 4, 56), "ffffffff", 2008),
            label(
                "pending",
                (2, 20, 60, 25),
                11,
                tr("Restart"),
                false,
                "e9b16cff",
                "Center",
                2009
            )
        );
        s.push_str(&raised_button(
            &format!("cell{i}"),
            ((i % 6) as i32 * 76, 70 + (i / 6) as i32 * 76, 64, 64),
            2,
            "",
            14,
            2006,
            &children,
        ));
    }
    for (i, (x, y)) in [(782, 120), (782, 32), (870, 120), (782, 208), (694, 120)]
        .into_iter()
        .enumerate()
    {
        let children = format!(
            "{}{}",
            image("face", "emote_gg", (12, 4, 44), "ffffffff", 2008),
            label(
                "title",
                (0, 48, 68, 20),
                11,
                tr(SLOT_NAMES[i]),
                false,
                "eeececff",
                "Center",
                2009
            )
        );
        s.push_str(&raised_button(
            &format!("slot{i}"),
            (x, y, 68, 68),
            2,
            "",
            14,
            2006,
            &children,
        ));
    }
    s.push_str(&rect("divider", (570, 0, 1, 448), "~4b4a49ff", 2005));
    s.push('}');
    s
}
fn clip(text: &str, width: f32, size: f32) -> String {
    if crate::hud_style::width(text, size) <= width {
        return text.into();
    }
    let mut out = text.to_owned();
    while !out.is_empty() && crate::hud_style::width(&format!("{out}…"), size) > width {
        out.pop();
    }
    out.push('…');
    out
}
impl Panel {
    pub fn handle(&mut self, action: Action, draft: &mut Values) {
        let library = emote_library::snapshot();
        match action {
            Action::Select(i) => {
                self.selected = self.cells.get(i).filter(|s| !s.is_empty()).cloned();
            }
            Action::Slot(i) if i < 5 => self.slot = i,
            Action::Assign => {
                if let Some(id) = self.selected.as_deref() {
                    emote_library::assign(draft, self.slot, id, &library);
                }
            }
            Action::Previous => self.page = self.page.saturating_sub(1),
            Action::Next => {
                self.page = (self.page + 1).min(library.entries.len().saturating_sub(1) / PAGE_SIZE)
            }
            Action::Refresh => {
                self.message = if emote_library::refresh() {
                    tr("Importing…")
                } else {
                    tr("Refresh is unavailable or already running")
                }
                .into();
            }
            Action::OpenFolder => {
                self.message = emote_library::open_folder()
                    .err()
                    .unwrap_or_else(|| tr("Add PNG files here, then press Refresh").into());
            }
            Action::Issue => self.issue = self.issue.wrapping_add(1),
            _ => {}
        }
    }
    pub fn preview(&self, values: &Values) -> Art {
        let library = emote_library::snapshot();
        self.selected
            .as_deref()
            .and_then(|id| library.find(id))
            .filter(|e| e.ready)
            .map(|e| e.art.clone())
            .unwrap_or_else(|| library.playable(values, self.slot).clone())
    }
    pub fn render(
        &mut self,
        ui: &mut SettingsUi,
        ctx: &mut StableClient<'_>,
        cursor: Option<(f32, f32)>,
    ) {
        let library = emote_library::snapshot();
        if self.revision != library.revision {
            self.revision = library.revision;
            self.message.clear();
        }
        self.page = self
            .page
            .min(library.entries.len().saturating_sub(1) / PAGE_SIZE);
        if self
            .selected
            .as_deref()
            .and_then(|id| library.find(id))
            .is_none()
        {
            self.selected = Some(library.entries[0].art.id.clone());
        }
        self.cells.resize(CELLS, String::new());
        for i in 0..CELLS {
            let node = format!("emote_panel.library.cell{i}");
            let entry = library.entries.get(self.page * PAGE_SIZE + i);
            ui.visible(ctx, &node, entry.is_some());
            let Some(entry) = entry else {
                self.cells[i].clear();
                continue;
            };
            self.cells[i] = entry.art.id.clone();
            let selected = self.selected.as_deref() == Some(&entry.art.id);
            ui.paint(
                ctx,
                &node,
                true,
                cursor,
                (crate::ui_theme::tone(0x292726ff), 0x707070ff, 0x5b5b5bff),
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
            ui.visible(ctx, &format!("{node}.face"), entry.ready);
            ui.visible(ctx, &format!("{node}.pending"), !entry.ready);
            if entry.ready {
                ui.props(
                    ctx,
                    &format!("{node}.face"),
                    format!("source: {};", json(&entry.art.source)),
                );
            }
        }
        for i in 0..5 {
            let node = format!("emote_panel.library.slot{i}");
            let entry = library.assigned(&ui.draft, i);
            ui.paint(
                ctx,
                &node,
                true,
                cursor,
                (crate::ui_theme::tone(0x393939ff), 0x707070ff, 0x5b5b5bff),
                (
                    if self.slot == i {
                        0xfdee00ff
                    } else {
                        crate::ui_theme::tone(0x6d6c6aff)
                    },
                    if self.slot == i { 2 } else { 1 },
                ),
                0xeeececff,
            );
            ui.props(
                ctx,
                &format!("{node}.face"),
                format!(
                    "source: {};",
                    json(if entry.ready && !library.missing(&ui.draft, i) {
                        &entry.art.source
                    } else {
                        "asset/lt_direct_control/ui/ef_clock"
                    })
                ),
            );
        }
        let selected = self
            .selected
            .as_deref()
            .and_then(|id| library.find(id))
            .unwrap_or(&library.entries[0]);
        ui.text(
            ctx,
            "emote_panel.library.selected",
            &clip(&selected.art.label, 448., 19.),
        );
        let current = !library.missing(&ui.draft, self.slot)
            && library.assigned(&ui.draft, self.slot).art.id == selected.art.id;
        ui.props(
            ctx,
            "emote_panel.library.assign",
            format!(
                "text: {{ text: {}; size: 15; color: #eeececff; }}",
                json(&if current {
                    trf(
                        "Assigned to {slot}",
                        &[("slot", &tr(SLOT_NAMES[self.slot]))],
                    )
                } else {
                    trf("Assign to {slot}", &[("slot", &tr(SLOT_NAMES[self.slot]))])
                })
            ),
        );
        ui.text(
            ctx,
            "emote_panel.library.count",
            &trf(
                "Emotes: {count} · page {page} / {pages}",
                &[
                    ("count", &library.entries.len()),
                    ("page", &(self.page + 1)),
                    ("pages", &library.entries.len().div_ceil(PAGE_SIZE)),
                ],
            ),
        );
        let busy = emote_library::busy();
        for (name, enabled) in [
            ("open", true),
            ("refresh", !busy),
            ("previous", self.page > 0),
            ("next", (self.page + 1) * PAGE_SIZE < library.entries.len()),
            ("assign", !current),
            ("issue", !library.errors.is_empty()),
        ] {
            ui.paint(
                ctx,
                &format!("emote_panel.library.{name}"),
                enabled,
                cursor,
                (crate::ui_theme::tone(0x393939ff), 0x707070ff, 0x5b5b5bff),
                (crate::ui_theme::tone(0x6d6c6aff), 1),
                0xeeececff,
            );
        }
        let status = if busy {
            tr("Importing…")
        } else if !self.message.is_empty() {
            &self.message
        } else if library.missing(&ui.draft, self.slot) {
            tr("Slot image is missing; built-in fallback is active")
        } else if !selected.ready {
            tr("Restart the game to load this image")
        } else if current {
            tr("Assignment is in your draft; Apply saves it")
        } else {
            tr("Select a slot, then assign this emote")
        };
        ui.text(ctx, "emote_panel.library.status", &clip(status, 448., 13.));
        let issue = library
            .errors
            .get(self.issue % library.errors.len().max(1))
            .map_or("", String::as_str);
        ui.text(
            ctx,
            "emote_panel.library.issue_text",
            &clip(issue, 440., 12.),
        );
    }
}
