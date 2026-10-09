//! Native settings window. The schema owns rows; this module owns draft editing and UI lifecycle.
//! Geometry and colours follow the approved HTML preview (the author's design, kept outside git),
//! measured in 1920x1080 coordinates; positions below are relative to the 1360x850 window.
use crate::{
    camera::Rect,
    hud_motion,
    lang::{tr, trf},
    native_timing::{MatchKey, NativeTiming, Phase},
    platform_input::Keys,
    settings::{self, Chord, Control, Values, BINDINGS, OPTIONS},
    Logger,
};
use mod_api_stable::{StableClient, UiEventKindV1};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
pub(crate) const PATH: &str = "ingame.lt_settings";
mod acquisition_panel;
mod emote_panel;
pub static SCROLL: AtomicI32 = AtomicI32::new(0);
const NAV_EMOTES: usize = 4;
const NAV_ADVANCED: usize = 5;
const PAGE_EMOTES: usize = 7;
/// Choices a select menu can show (the mod language has 18).
const POPUP_MAX: usize = 18;
pub(crate) const PAGES: [(&str, &str, &str); 6] = [
    (
        "Combat & casting",
        "Choose how movement, targeting and ability inputs behave.",
        "ef_swords",
    ),
    (
        "Keybinds",
        "Select a binding, then press a key or mouse combination.",
        "ef_keyboard",
    ),
    (
        "Camera",
        "Adjust how the camera follows and moves around the battlefield.",
        "ef_camera",
    ),
    (
        "Interface",
        "Cursor and battlefield feedback, alongside your existing HUD.",
        "ef_panels",
    ),
    (
        "Emotes",
        "Choose emotes and adjust placement, size and timing. Apply saves your choices.",
        "emote_gg",
    ),
    (
        "Advanced",
        "Fine-tune controls, acquisition and diagnostic tools.",
        "ef_settings",
    ),
];
// Scrolling content viewport (window coordinates).
const VIEW_TOP: f32 = 225.;
const VIEW_BOTTOM: f32 = 760.;
pub(crate) const ADVANCED: [&str; 3] = ["General", "Acquisition", "Debug"];
// Internal schema pages: General 4, Debug 5, Acquisition 6.
fn active_page(page: usize, advanced: usize) -> usize {
    if page == NAV_ADVANCED {
        [4, 6, 5][advanced]
    } else if page == NAV_EMOTES {
        PAGE_EMOTES
    } else {
        page
    }
}
fn view_top(page: usize) -> f32 {
    if (4..=6).contains(&page) {
        VIEW_TOP + 60.
    } else {
        VIEW_TOP
    }
}
const CONTENT_X: i32 = 273;
const CONTENT_W: i32 = 1042;
const CONTROL_X: i32 = 634; // Row-relative control column (386 px wide).
const ROWS: usize = 9;
const SECTIONS: usize = 5;
const HEADS: usize = 3;
const NOTCH: f32 = 90.;
pub(crate) const HINT_IDLE: &str =
    "Click a binding to change it. Esc cancels capture; Backspace clears it.";
pub(crate) const HINT_LISTEN: &str =
    "Listening for a key or mouse combination. Esc cancels; Backspace clears.";

#[derive(Clone, Copy, Debug, PartialEq)]
enum Entry {
    Hint,
    Section(&'static str),
    Head,
    Opt(usize),
    Bind(usize),
    Cursor,
    Acquisition,
    Emotes,
}
impl Entry {
    fn height(self) -> f32 {
        match self {
            Entry::Hint => 45.,
            Entry::Section(_) => 22.,
            Entry::Head => 29.,
            Entry::Opt(i) => match OPTIONS[i].control {
                Control::Toggle => 56.,
                _ => 84.,
            },
            Entry::Bind(_) => 70.,
            Entry::Cursor => 64.,
            Entry::Acquisition => acquisition_panel::HEIGHT as f32,
            Entry::Emotes => emote_panel::HEIGHT as f32,
        }
    }
    /// Margin below this entry; a following section uses at least 24 px.
    fn after(self) -> f32 {
        match self {
            Entry::Hint | Entry::Section(_) => 12.,
            Entry::Head | Entry::Cursor => 0.,
            Entry::Opt(_) | Entry::Bind(_) | Entry::Acquisition | Entry::Emotes => 6.,
        }
    }
}
/// Page content as (entry, top) in scroll coordinates, plus the content height.
fn layout(page: usize) -> (Vec<(Entry, f32)>, f32) {
    if page == PAGE_EMOTES {
        return (vec![(Entry::Emotes, 0.)], emote_panel::HEIGHT as f32);
    }
    if page == 6 {
        return (
            vec![(Entry::Acquisition, 0.)],
            acquisition_panel::HEIGHT as f32,
        );
    }
    let mut entries = Vec::new();
    if page == 1 {
        entries.push(Entry::Hint);
        let mut group = "";
        for (i, d) in BINDINGS.iter().enumerate() {
            if d.group != group {
                group = d.group;
                entries.push(Entry::Section(group));
                entries.push(Entry::Head);
            }
            entries.push(Entry::Bind(i));
        }
    } else {
        let mut section = "";
        for (i, d) in OPTIONS.iter().enumerate().filter(|(_, d)| d.page == page) {
            if d.section != section {
                section = d.section;
                entries.push(Entry::Section(section));
            }
            entries.push(Entry::Opt(i));
            if d.key == "cursor_size" {
                entries.push(Entry::Cursor);
            }
        }
    }
    let mut out = Vec::new();
    let mut y = 0.;
    let mut margin: f32 = 0.;
    for (n, e) in entries.into_iter().enumerate() {
        let top = if n == 0 {
            0.
        } else if matches!(e, Entry::Section(_)) {
            y + margin.max(24.)
        } else {
            y + margin
        };
        out.push((e, top));
        y = top + e.height();
        margin = e.after();
    }
    (out, y)
}
fn max_scroll(page: usize) -> f32 {
    if page == 6 || page == PAGE_EMOTES {
        return 0.;
    }
    // The design keeps 24 px of padding below the last item.
    (layout(page).1 + 24. - (VIEW_BOTTOM - view_top(page))).max(0.)
}

#[derive(Clone)]
enum Event {
    Acquisition(acquisition_panel::Action),
    Emote(emote_panel::Action),
    Page(usize),
    Advanced(usize),
    Seg(usize, usize),
    Toggle(usize),
    Field(usize),
    Popup(usize),
    Key(usize, usize),
    Apply,
    Cancel,
    Reset,
    Replace,
    Dismiss,
}
fn json(text: &str) -> String {
    serde_json::to_string(text).unwrap()
}
#[allow(clippy::too_many_arguments)]
fn label(
    name: &str,
    (x, y, w, h): (i32, i32, i32, i32),
    size: i32,
    text: &str,
    bold: bool,
    color: &str,
    align: &str,
    z: i32,
) -> String {
    let style = if bold { "bold_label" } else { "label" };
    format!("#{name}:label {{ @\"asset/base/style/main#{style}\"; x: {x}px; y: {y}px; width: {w}px; height: {h}px; size: {size}; text: {}; color: #{color}; align_x: {align}; align_y: Center; ignore_event: true; z: {z}; }}\n",json(text))
}
fn image(name: &str, glyph: &str, (x, y, s): (i32, i32, i32), color: &str, z: i32) -> String {
    format!("#{name}:image {{ x: {x}px; y: {y}px; width: {s}px; height: {s}px; source: \"asset/lt_direct_control/ui/{glyph}\"; color: #{color}; ignore_event: true; z: {z}; }}\n")
}
fn rect(name: &str, (x, y, w, h): (i32, i32, i32, i32), color: &str, z: i32) -> String {
    format!("#{name}:color {{ x: {x}px; y: {y}px; width: {w}px; height: {h}px; color: #{color}; ignore_event: true; z: {z}; }}\n")
}
/// A clickable surface; its visible styling is updated every frame.
#[allow(clippy::too_many_arguments)]
fn button(
    name: &str,
    (x, y, w, h): (i32, i32, i32, i32),
    rounding: i32,
    text: &str,
    size: i32,
    z: i32,
    children: &str,
) -> String {
    format!("#{name}:color_icon_button {{ x: {x}px; y: {y}px; width: {w}px; height: {h}px; z: {z}; btn: {{ color: #~4b4a49ff; back_color: #00000000; stroke: 0; rounding: Uniform {{ rounding: {rounding}; }} }} text: {{ @\"asset/base/style/main#bold_label\"; align_x: Center; align_y: Center; text: {}; size: {size}; color: #eeececff; }} {children}}}\n",json(text))
}

/// Native UI has no CSS blur. Two translucent plates give raised controls the
/// lab's contact shadow while the button stroke supplies its crisp outline.
#[allow(clippy::too_many_arguments)]
fn raised_button(
    name: &str,
    at: (i32, i32, i32, i32),
    rounding: i32,
    text: &str,
    size: i32,
    z: i32,
    children: &str,
) -> String {
    let (_, _, w, h) = at;
    let mut layers = format!(
        "#shadow_outer:color {{ x: -2px; y: 2px; width: {}px; height: {}px; color: #00000024; rounding: Uniform {{ rounding: {rounding}; }} ignore_event: true; z: {}; }}\n#shadow_contact:color {{ x: -1px; y: 3px; width: {}px; height: {}px; color: #00000050; rounding: Uniform {{ rounding: {rounding}; }} ignore_event: true; z: {}; }}\n",
        w + 4,
        h + 4,
        z - 2,
        w + 2,
        h + 1,
        z - 1,
    );
    layers.push_str(children);
    button(name, at, rounding, text, size, z, &layers)
}
fn row_template(i: usize) -> String {
    let mut c = String::new();
    c.push_str(&rect("edge", (0, 0, CONTENT_W, 1), "~4b4a49ff", 2004));
    c.push_str(&rect("shade", (0, 83, CONTENT_W, 1), "~292726ff", 2004));
    // A compact checkbox row is itself the hit target, as in the updated preview.
    c.push_str("#check_box:color { x: 24px; y: 13px; width: 30px; height: 30px; color: #201e1dff; rounding: Uniform { rounding: 2; } ignore_event: true; visible: false; z: 2006; #fill:color { x: 2px; y: 2px; width: 26px; height: 26px; color: #eeececff; ignore_event: true; z: 2007; } #rail:color { x: 4px; y: 6px; width: 3px; height: 18px; color: #fdee00ff; ignore_event: true; visible: false; z: 2008; } #mark:image { x: 6px; y: 6px; width: 18px; height: 18px; source: \"asset/lt_direct_control/ui/ef_check\"; color: #ffffffff; ignore_event: true; visible: false; z: 2008; } }\n");
    c.push_str(&label(
        "check_label",
        (72, 3, 282, 50),
        21,
        "",
        true,
        "ffffffff",
        "Left",
        2007,
    ));
    c.push_str(&label(
        "check_hint",
        (354, 4, 660, 48),
        14,
        "",
        false,
        "989694ff",
        "Left",
        2007,
    ));
    c.push_str(&button(
        "check_hit",
        (0, 0, CONTENT_W, 56),
        2,
        "",
        1,
        2009,
        "",
    ));
    c.push_str(&label(
        "label",
        (24, 13, 600, 30),
        21,
        "",
        true,
        "ffffffff",
        "Left",
        2005,
    ));
    c.push_str(&label(
        "hint",
        (24, 45, 590, 22),
        14,
        "",
        false,
        "989694ff",
        "Left",
        2005,
    ));
    // Segmented control: one track, one sliding indicator, two labels.
    c.push_str(&format!("#track:color {{ x: {CONTROL_X}px; y: 13px; width: 386px; height: 50px; color: #~5b5b5bff; rounding: Uniform {{ rounding: 25; }} ignore_event: true; z: 2005; #pill_shadow:color {{ x: -2px; y: -2px; width: 207px; height: 54px; color: #00000059; rounding: Uniform {{ rounding: 27; }} ignore_event: true; z: 2005; }} #ind:color {{ x: 0px; y: 0px; width: 203px; height: 50px; color: #eeececff; rounding: Uniform {{ rounding: 25; }} ignore_event: true; z: 2006; #upper:color {{ x: 20px; y: 0px; width: 163px; height: 1px; color: #ffffffff; ignore_event: true; z: 2007; }} #lower:color {{ x: 20px; y: 49px; width: 163px; height: 1px; color: #ffffffff; ignore_event: true; z: 2007; }} }} "));
    for j in 0..2 {
        c.push_str(&label(
            &format!("opt{j}"),
            (j * 193, 0, 193, 50),
            18,
            "",
            true,
            "b8b6b5ff",
            "Center",
            2007,
        ));
    }
    c.push_str("}\n");
    for j in 0..2 {
        c.push_str(&button(
            &format!("seg{j}"),
            (CONTROL_X + j * 193, 13, 193, 50),
            25,
            "",
            18,
            2008,
            "",
        ));
    }
    // Select field (the open menu is a separate popup group).
    let mut field = label(
        "value",
        (22, 0, 300, 50),
        18,
        "",
        false,
        "393939ff",
        "Left",
        2006,
    );
    field.push_str(&image(
        "caret",
        "ef_chevron_down",
        (344, 15, 20),
        "393939ff",
        2006,
    ));
    c.push_str(&raised_button(
        "field",
        (CONTROL_X, 13, 386, 50),
        25,
        "",
        18,
        2005,
        &field,
    ));
    // Slider: native input node with drawn track, fill and capsule thumb.
    c.push_str(&format!("#slider:slider {{ x: {CONTROL_X}px; y: 17px; width: 294px; height: 50px; z: 2006; background: Color {{ prop: {{ color: #00000000; }} }} foreground: Color {{ prop: {{ color: #00000000; }} }} view_min_ratio: 0.0; ratio: 0.0; #track:color {{ y: 6px; width: 100%; height: 6px; color: #~6d6c6aff; rounding: Uniform {{ rounding: 3; }} ignore_event: true; z: 2007; }} #fill:color {{ y: 6px; width: 50%; height: 6px; color: #eeececff; rounding: Uniform {{ rounding: 3; }} ignore_event: true; z: 2008; }} #thumb:color {{ x: 0px; y: 0px; width: 50px; height: 18px; pivot_x: 0.5; rounding: Uniform {{ rounding: 9; }} color: #eeececff; ignore_event: true; z: 2009; }} }}\n"));
    c.push_str(&label(
        "number",
        (CONTROL_X + 316, 12, 70, 28),
        21,
        "",
        true,
        "eeececff",
        "Right",
        2006,
    ));
    for j in 0..2 {
        c.push_str(&raised_button(
            &format!("key{j}"),
            (664 + j * 184, 14, 172, 42),
            2,
            "",
            18,
            2005,
            "",
        ));
    }
    // Colour: the shared text field plus a swatch of the typed colour.
    c.push_str(&format!(
        "#hex_box:empty {{ x: {CONTROL_X}px; y: 17px; width: 386px; height: 50px; visible: false; ignore_event: true; z: 2006;\n{}#swatch:color {{ x: 266px; y: 0px; width: 120px; height: 50px; color: #~6d6c6aff; back_color: #{}; stroke: 1; rounding: Uniform {{ rounding: 2; }} ignore_event: true; z: 2007; }}\n}}\n",
        acquisition_panel::edit("hex", (0, 0, 250, 50), "#1c1a18"),
        crate::ui_theme::hex(0xff)
    ));
    format!("#row{i}:color {{ x: {CONTENT_X}px; y: 0px; width: {CONTENT_W}px; height: 84px; color: #~3a3837ff; rounding: Uniform {{ rounding: 2; }} ignore_event: true; visible: false; z: 2003;\n{c}}}\n")
}
fn template() -> String {
    let mut s = String::from("lt_settings:color { width: 1920px; height: 1080px; z: 1999; color: #~100f0de0; ignore_event: false; visible: false;\n#window:color { x: 280px; y: 115px; width: 1360px; height: 850px; color: #~4b4a49ff; z: 2000; ignore_event: false; rounding: Uniform { rounding: 2; }\n");
    s.push_str(&rect(
        "fill",
        (1, 1, 1358, 848),
        &crate::ui_theme::hex(0xff),
        2001,
    ));
    // Header.
    s.push_str(&label(
        "eyebrow",
        (37, 27, 450, 18),
        13,
        "LT TAKEOVER / PREFERENCES",
        false,
        "989694ff",
        "Left",
        2015,
    ));
    s.push_str(&label(
        "title",
        (37, 45, 400, 42),
        34,
        tr("Settings"),
        true,
        "ffffffff",
        "Left",
        2015,
    ));
    s.push_str(&label(
        "pause",
        (855, 33, 400, 24),
        17,
        tr("Paused for settings"),
        false,
        "b3d543ff",
        "Right",
        2015,
    ));
    s.push_str(&label(
        "return",
        (855, 60, 400, 20),
        14,
        tr("Match resumes when you close this window"),
        false,
        "989694ff",
        "Right",
        2015,
    ));
    s.push_str(&raised_button(
        "close",
        (1283, 35, 44, 44),
        2,
        "",
        18,
        2015,
        &image("icon", "ef_x", (11, 11, 22), "cbc9c7ff", 2016),
    ));
    s.push_str(&rect("head_rule", (1, 112, 1358, 1), "~4b4a49ff", 2015));
    s.push_str(&rect("nav_rule", (238, 113, 1, 647), "~4b4a49ff", 2015));
    // Navigation.
    for (i, (_, _, glyph)) in PAGES.iter().enumerate() {
        let mut children = image("icon", glyph, (18, 24, 25), "cbc9c7ff", 2017);
        children.push_str(&label(
            "label",
            (59, 9, 140, 54),
            20,
            "",
            true,
            "cbc9c7ff",
            "Left",
            2017,
        ));
        s.push_str(&button(
            &format!("nav{i}"),
            (21, 141 + i as i32 * 80, 203, 72),
            2,
            "",
            20,
            2016,
            &children,
        ));
    }
    s.push_str(&label(
        "nav_note",
        (39, 664, 170, 50),
        14,
        "",
        false,
        "989694ff",
        "Left",
        2015,
    ));
    // Scrolling content. Masks above and below clip partially visible rows.
    s.push_str(&format!("#hintbox:color {{ x: {CONTENT_X}px; y: 0px; width: {CONTENT_W}px; height: 45px; color: #~3a3837ff; rounding: Uniform {{ rounding: 2; }} ignore_event: true; visible: false; z: 2003;\n{}{}}}\n",rect("bar",(0,0,3,45),"fdee00ff",2004),label("text",(16,0,1010,45),15,"",false,"cbc9c7ff","Left",2004)));
    for i in 0..SECTIONS {
        s.push_str(&format!("#sec{i}:color {{ x: {CONTENT_X}px; y: 0px; width: {CONTENT_W}px; height: 22px; color: #00000000; ignore_event: true; visible: false; z: 2003;\n{}{}{}}}\n",
            image("chev","ef_chevron_right",(0,2,18),"a3a19fff",2004),
            label("text",(30,-2,600,26),16,"",true,"cbc9c7ff","Left",2004),
            rect("line",(160,10,880,1),"~4b4a49ff",2004)));
    }
    for i in 0..HEADS {
        s.push_str(&format!(
            "#head{i}:color {{ x: {CONTENT_X}px; y: 0px; width: {CONTENT_W}px; height: 29px; color: #00000000; ignore_event: true; visible: false; z: 2003;\n{}{}{}}}\n",
            label("action", (24, 0, 300, 19), 14, tr("Action"), false, "989694ff", "Left", 2004),
            label("primary", (662, 0, 172, 19), 14, tr("Primary"), false, "989694ff", "Left", 2004),
            label("secondary", (846, 0, 172, 19), 14, tr("Secondary"), false, "989694ff", "Left", 2004)
        ));
    }
    for i in 0..ROWS {
        s.push_str(&row_template(i));
    }
    s.push_str(&acquisition_panel::template());
    s.push_str(&emote_panel::template());
    s.push_str(&format!("#cursor_ex:color {{ x: {CONTENT_X}px; y: 0px; width: {CONTENT_W}px; height: 64px; color: #00000000; ignore_event: true; visible: false; z: 2003;\n{}{}}}\n",
        "#img:image { x: 24px; y: 16px; width: 32px; height: 32px; source: \"asset/lt_direct_control/ui/cursor_preview\"; ignore_event: true; z: 2004; }\n",
        label("text",(72,21,300,22),14,tr("Cursor size preview"),false,"989694ff","Left",2004)));
    s.push_str(&"#mask_top:color { x: 239px; y: 113px; width: 1120px; height: 112px; color: #1c1a18ff; ignore_event: false; z: 2012; }\n#mask_bottom:color { x: 1px; y: 760px; width: 1358px; height: 89px; color: #1c1a18ff; ignore_event: false; z: 2012; }\n".replace("#1c1a18ff", &format!("#{}", crate::ui_theme::hex(0xff))));
    // The enclosing input layer must clear the clipping masks as well as
    // the buttons' draw layers. Declare this subtree after the blockers and
    // ignore events on its empty container so only its buttons receive clicks.
    s.push_str(&format!("#advanced_nav:empty {{ x: {CONTENT_X}px; y: 225px; width: {CONTENT_W}px; height: 54px; visible: false; ignore_event: true; z: 2013;"));
    for (i, title) in ADVANCED.iter().enumerate() {
        s.push_str(&raised_button(
            &format!("sub{i}"),
            (i as i32 * 190, 0, 180, 40),
            2,
            tr(title),
            17,
            2014,
            "",
        ));
    }
    s.push_str(&rect("line", (0, 53, CONTENT_W, 1), "~4b4a49ff", 2013));
    s.push('}');
    s.push_str(&rect("foot_rule", (1, 760, 1358, 1), "~4b4a49ff", 2015));
    s.push_str(&rect(
        "scroll_track",
        (1351, 225, 5, 535),
        "~3a3837ff",
        2013,
    ));
    s.push_str(&rect("scroll_thumb", (1351, 225, 5, 120), "989694ff", 2014));
    // Page heading.
    s.push_str(&label(
        "page_title",
        (273, 138, 700, 38),
        26,
        "",
        true,
        "ffffffff",
        "Left",
        2015,
    ));
    s.push_str(&label(
        "page_hint",
        (273, 180, 900, 24),
        15,
        "",
        false,
        "989694ff",
        "Left",
        2015,
    ));
    s.push_str(&label(
        "position",
        (1175, 139, 150, 20),
        13,
        "",
        false,
        "989694ff",
        "Right",
        2015,
    ));
    // Footer.
    s.push_str(&raised_button(
        "reset",
        (33, 781, 206, 48),
        2,
        tr("Restore this page"),
        18,
        2015,
        "",
    ));
    s.push_str(&label(
        "status",
        (255, 783, 700, 44),
        15,
        tr("No changes"),
        false,
        "989694ff",
        "Left",
        2015,
    ));
    s.push_str(&raised_button(
        "cancel",
        (1028, 781, 114, 48),
        2,
        tr("Cancel"),
        18,
        2015,
        "",
    ));
    s.push_str(&raised_button(
        "apply",
        (1159, 781, 168, 48),
        2,
        tr("Apply & close"),
        18,
        2015,
        "",
    ));
    // Select menu: opens from the field's vertical centre, behind a copy of the field.
    let mut popup = String::from("#popup:color { x: 907px; y: 0px; width: 386px; height: 50px; color: #00000000; ignore_event: true; visible: false; z: 2030;\n#shadow:color { x: -2px; y: 28px; width: 390px; height: 232px; color: #00000066; rounding: Uniform { rounding: 6; } ignore_event: true; z: 2029; }\n#menu:color { x: 0px; y: 25px; width: 386px; height: 230px; color: #d4d2d3ff; rounding: Uniform { rounding: 6; } ignore_event: false; z: 2030; }\n");
    popup.push_str(&rect("rule", (0, 62, 386, 1), "c6c4c5ff", 2031));
    // Up to three choices list under the field; longer lists (the mod
    // language) use a two-column grid, positioned when the menu opens.
    for k in 0..POPUP_MAX {
        let mut children = label(
            "text",
            (22, 0, 300, 63),
            17,
            "",
            false,
            "393939ff",
            "Left",
            2033,
        );
        children.push_str(&rect("bar", (3, 22, 2, 20), "ffffffff", 2033));
        children.push_str(&image("check", "ef_check", (346, 23, 18), "ffffffff", 2033));
        popup.push_str(&button(
            &format!("opt{k}"),
            (0, 63 + k as i32 % 3 * 64, 386, 63),
            0,
            "",
            17,
            2031,
            &children,
        ));
        if k < 2 {
            popup.push_str(&rect(
                &format!("div{k}"),
                (0, 126 + k as i32 * 64, 386, 1),
                "c6c4c5ff",
                2032,
            ));
        }
    }
    let mut field = label(
        "value",
        (22, 0, 300, 50),
        18,
        "",
        false,
        "393939ff",
        "Left",
        2035,
    );
    field.push_str(&image(
        "caret",
        "ef_chevron_up",
        (344, 15, 20),
        "393939ff",
        2035,
    ));
    popup.push_str(&format!("#field:color {{ x: 0px; y: 0px; width: 386px; height: 50px; color: #eeececff; rounding: Uniform {{ rounding: 25; }} ignore_event: true; z: 2034;\n{field}}}\n"));
    popup.push_str("}\n");
    s.push_str(&popup);
    // Binding conflict dialog.
    s.push_str(&"#conflict_shade:color { x: -280px; y: -115px; width: 1920px; height: 1080px; color: #~100f0dcc; ignore_event: false; visible: false; z: 2049; }\n#conflict:color { x: 390px; y: 235px; width: 580px; height: 250px; color: #989694ff; ignore_event: false; visible: false; z: 2050; #fill:color { x: 1px; y: 1px; width: 578px; height: 248px; color: #1c1a18ff; ignore_event: true; z: 2051; } }\n".replace("#1c1a18ff", &format!("#{}", crate::ui_theme::hex(0xff))));
    s.push_str(&label(
        "conflict_title",
        (422, 263, 516, 36),
        25,
        tr("Binding already used"),
        true,
        "ffffffff",
        "Left",
        2052,
    ));
    s.push_str(&label(
        "conflict_text",
        (422, 305, 516, 80),
        18,
        "",
        false,
        "cbc9c7ff",
        "Left",
        2052,
    ));
    s.push_str(&raised_button(
        "dismiss",
        (612, 405, 114, 48),
        2,
        tr("Cancel"),
        17,
        2052,
        "",
    ));
    s.push_str(&raised_button(
        "replace",
        (742, 405, 196, 48),
        2,
        tr("Replace binding"),
        17,
        2052,
        "",
    ));
    s.push_str("}\n}");
    crate::hud_style::fonts(s)
}
#[derive(Default)]
pub struct SettingsUi {
    acquisition: acquisition_panel::Panel,
    emotes: emote_panel::Panel,
    open: bool,
    page: usize,
    advanced: usize,
    scroll: f32,
    draft: Values,
    original: Values,
    bound: Option<(MatchKey, u64)>,
    paused_by_me: bool,
    events: Arc<Mutex<Vec<Event>>>,
    capture: Option<(usize, usize)>,
    capture_armed: bool,
    pending: Option<(usize, usize, Chord)>,
    previous: [u64; 4],
    /// Open select menu, by OPTIONS index.
    drop: Option<usize>,
    /// Entry shown in each row slot during the last frame; events resolve through it.
    slots: [Option<Entry>; ROWS],
    cache: HashMap<String, String>,
    spawn: Option<Instant>,
    registered: bool,
    pointer_down: bool,
    motion: crate::hud_motion::Motion,
    /// The colour field: (option index, its text, the draft colour it shows).
    hex: (Option<usize>, String, u32),
}
impl SettingsUi {
    pub fn open(
        &mut self,
        timing: &NativeTiming,
        store: &settings::Settings,
        keys: Keys,
        log: &Logger,
    ) {
        let Some(phase @ (Phase::Running | Phase::Paused)) = timing.phase() else {
            return;
        };
        let Some(key) = timing.match_key() else {
            return;
        };
        self.original = store.snapshot();
        self.draft = self.original.clone();
        self.acquisition = acquisition_panel::Panel::new();
        self.emotes = emote_panel::Panel::default();
        self.hex = (None, String::new(), 0);
        self.bound = Some((key, timing.generation()));
        self.paused_by_me = phase == Phase::Running;
        if self.paused_by_me {
            timing.request_action(true);
            if let Some(action) = timing.take_action() {
                timing.apply_action(action, log);
            }
            if timing.phase() != Some(Phase::Paused) {
                self.paused_by_me = false;
                return;
            }
        }
        log.write(&format!(
            "SETTINGS open auto_paused={} page={}",
            self.paused_by_me, self.page
        ));
        self.open = true;
        self.capture = None;
        self.pending = None;
        self.drop = None;
        self.scroll = 0.;
        self.slots = [None; ROWS];
        self.previous = bits(&keys.raw.0);
        crate::ui_state::SETTINGS_OPEN.store(true, Ordering::Relaxed);
    }
    fn close(
        &mut self,
        timing: &NativeTiming,
        store: &settings::Settings,
        apply: bool,
        log: &Logger,
    ) {
        if apply {
            if !self.draft.essential() || self.acquisition.invalid {
                return;
            }
            store.apply(self.draft.clone());
            store.flush(true, log);
        }
        if should_resume(
            self.paused_by_me,
            self.bound,
            timing.match_key().map(|k| (k, timing.generation())),
            timing.phase(),
        ) {
            timing.request_action(true)
        }
        log.write(&format!(
            "SETTINGS close applied={apply} resume={}",
            self.paused_by_me
        ));
        self.open = false;
        self.capture = None;
        self.pending = None;
        self.drop = None;
        crate::ui_state::SETTINGS_OPEN.store(false, Ordering::Relaxed);
    }
    fn props(&mut self, ctx: &mut StableClient<'_>, node: &str, text: String) {
        hud_motion::properties(
            ctx,
            &mut self.cache,
            &format!("{PATH}.window.{node}"),
            &text,
        );
    }
    fn text(&mut self, ctx: &mut StableClient<'_>, node: &str, text: &str) {
        // Raw text keeps real line breaks; source-escaped "\n" would render literally.
        let path = format!("{PATH}.window.{node}");
        let key = format!("{path}#text");
        if self.cache.get(&key).is_none_or(|t| t != text) && ctx.ui_set_text(&path, text) {
            self.cache.insert(key, text.into());
        }
    }
    fn visible(&mut self, ctx: &mut StableClient<'_>, node: &str, on: bool) {
        self.props(ctx, node, format!("visible: {on};"));
    }
    fn hovered(&self, ctx: &StableClient<'_>, node: &str, cursor: Option<(f32, f32)>) -> bool {
        cursor.is_some_and(|p| {
            ctx.ui_node_rect(&format!("{PATH}.window.{node}"))
                .is_some_and(|(x, y, w, h)| Rect { x, y, w, h }.contains(p))
        })
    }
    /// Tonal hover/press colour for a surface: (rest, hover, press).
    fn tone(&mut self, key: &str, hover: bool, colors: (u32, u32, u32)) -> u32 {
        let h = self
            .motion
            .tonal(&format!("h_{key}"), f32::from(hover), 0.1);
        let p = self.motion.tonal(
            &format!("p_{key}"),
            f32::from(hover && self.pointer_down),
            0.083,
        );
        let c =
            u32::from_str_radix(&hud_motion::color(colors.0, colors.1, h), 16).unwrap_or(colors.0);
        u32::from_str_radix(&hud_motion::color(c, colors.2, p), 16).unwrap_or(c)
    }
    #[allow(clippy::too_many_arguments)]
    fn paint(
        &mut self,
        ctx: &mut StableClient<'_>,
        node: &str,
        on: bool,
        cursor: Option<(f32, f32)>,
        colors: (u32, u32, u32),
        border: (u32, u32),
        text: u32,
    ) {
        let hover = on && self.hovered(ctx, node, cursor);
        let back = self.tone(node, hover, colors);
        self.props(ctx,node,format!("visible: true; ignore_event: {}; btn: {{ back_color: #{back:08x}; color: #{:08x}; stroke: {}; }} text: {{ color: #{text:08x}; }}",!on,border.0,border.1));
    }
    fn capture(&mut self, keys: Keys) {
        let Some((index, slot)) = self.capture else {
            return;
        };
        if !keys.focused {
            self.capture_armed = false;
            return;
        }
        let raw = keys.raw.0;
        let nonmod = |i: usize| !matches!(i,0x10..=0x12|0xa0..=0xa5);
        if !self.capture_armed {
            if !(1..256).any(|i| nonmod(i) && raw[i]) {
                self.capture_armed = true;
            }
            return;
        }
        let edge = (1..256).find(|&i| nonmod(i) && raw[i] && !bit(self.previous, i));
        let code = edge.or_else(|| {
            [0x10, 0x11, 0x12]
                .into_iter()
                .find(|&i| !raw[i] && bit(self.previous, i))
        });
        if let Some(code) = code {
            if code == 0x1b {
                self.capture = None;
                return;
            }
            if code == 8 {
                self.draft.bind(BINDINGS[index].key, slot, None);
                self.capture = None;
                return;
            }
            let chord = Chord {
                code: code as u8,
                mods: if matches!(code, 0x10..=0x12) {
                    0
                } else {
                    settings::modifiers(&raw)
                },
            };
            if self
                .draft
                .conflicts(chord, BINDINGS[index].key, slot)
                .is_empty()
            {
                self.draft.bind(BINDINGS[index].key, slot, Some(chord));
            } else {
                self.pending = Some((index, slot, chord));
            }
            self.capture = None;
        }
    }
    fn handle(
        &mut self,
        event: Event,
        keys: Keys,
        timing: &NativeTiming,
        store: &settings::Settings,
        log: &Logger,
    ) {
        if self.pending.is_some()
            && !matches!(event, Event::Replace | Event::Dismiss | Event::Cancel)
        {
            return;
        }
        if self.capture.is_some() && !matches!(event, Event::Cancel) {
            return;
        }
        match event {
            Event::Emote(action) => {
                if self.page == NAV_EMOTES {
                    self.emotes.handle(action, &mut self.draft);
                }
            }
            Event::Acquisition(action) => {
                if active_page(self.page, self.advanced) == 6 {
                    self.acquisition.handle(action, &mut self.draft);
                }
            }
            Event::Page(p) => {
                self.page = p;
                self.motion.reset();
                self.acquisition.hide();
                self.emotes.hide();
                self.scroll = 0.;
                self.drop = None;
                self.slots = [None; ROWS];
            }
            Event::Advanced(p) => {
                if self.page == NAV_ADVANCED && p < ADVANCED.len() {
                    self.advanced = p;
                    self.scroll = 0.;
                    self.motion.reset();
                    self.drop = None;
                    self.slots = [None; ROWS];
                    self.acquisition.hide();
                    log.write(&format!("SETTINGS advanced_subpage={}", ADVANCED[p]));
                }
            }
            Event::Seg(slot, choice) => {
                if let Some(Entry::Opt(i)) = self.slots[slot] {
                    if matches!(OPTIONS[i].control, Control::Choice(values) if values.len() == 2) {
                        self.draft.set(OPTIONS[i].key, choice as f64);
                    }
                }
                self.drop = None;
            }
            Event::Toggle(slot) => {
                if let Some(Entry::Opt(i)) = self.slots[slot] {
                    if matches!(OPTIONS[i].control, Control::Toggle) {
                        let key = OPTIONS[i].key;
                        self.draft.set(key, 1. - self.draft.number(key));
                    }
                }
                self.drop = None;
            }
            Event::Field(slot) => {
                if let Some(Entry::Opt(i)) = self.slots[slot] {
                    self.drop = if self.drop == Some(i) { None } else { Some(i) };
                }
            }
            Event::Popup(choice) => {
                if let Some(i) = self.drop.take() {
                    self.draft.set(OPTIONS[i].key, choice as f64);
                }
            }
            Event::Key(slot, j) => {
                if let Some(Entry::Bind(index)) = self.slots[slot] {
                    self.capture = Some((index, j));
                    self.capture_armed = false;
                    self.previous = bits(&keys.raw.0);
                    self.drop = None;
                }
            }
            Event::Reset => {
                self.draft.reset_page(active_page(self.page, self.advanced));
                if active_page(self.page, self.advanced) == 6 {
                    self.acquisition.reset_all();
                }
                self.drop = None;
            }
            Event::Apply => self.close(timing, store, true, log),
            Event::Cancel => self.close(timing, store, false, log),
            Event::Replace => {
                if let Some((index, slot, chord)) = self.pending.take() {
                    for (key, i) in self.draft.conflicts(chord, BINDINGS[index].key, slot) {
                        self.draft.bind(key, i, None);
                    }
                    self.draft.bind(BINDINGS[index].key, slot, Some(chord));
                }
            }
            Event::Dismiss => self.pending = None,
        }
    }
    fn register(&mut self, ctx: &mut StableClient<'_>) {
        let mut items = vec![
            ("close".to_string(), Event::Cancel),
            ("cancel".into(), Event::Cancel),
            ("apply".into(), Event::Apply),
            ("reset".into(), Event::Reset),
            ("replace".into(), Event::Replace),
            ("dismiss".into(), Event::Dismiss),
        ];
        for i in 0..PAGES.len() {
            items.push((format!("nav{i}"), Event::Page(i)));
        }
        for i in 0..ROWS {
            for j in 0..2 {
                items.push((format!("row{i}.seg{j}"), Event::Seg(i, j)));
                items.push((format!("row{i}.key{j}"), Event::Key(i, j)));
            }
            items.push((format!("row{i}.check_hit"), Event::Toggle(i)));
            items.push((format!("row{i}.field"), Event::Field(i)));
        }
        for k in 0..POPUP_MAX {
            items.push((format!("popup.opt{k}"), Event::Popup(k)));
        }
        for i in 0..ADVANCED.len() {
            items.push((format!("advanced_nav.sub{i}"), Event::Advanced(i)));
        }
        items.extend(acquisition_panel::events());
        items.extend(emote_panel::events());
        for (node, event) in items {
            let queue = self.events.clone();
            ctx.ui_register_path_events(&format!("{PATH}.window.{node}"), move |ctx| {
                if ctx
                    .ui_current_event()
                    .is_some_and(|e| e.kind == Some(UiEventKindV1::Click))
                {
                    if let Ok(mut q) = queue.lock() {
                        q.push(event.clone());
                    }
                }
            });
        }
    }
    pub fn apply(
        &mut self,
        ctx: &mut StableClient<'_>,
        timing: &NativeTiming,
        store: &settings::Settings,
        keys: Keys,
        log: &Logger,
    ) -> Vec<Rect> {
        if self.open
            && (self.bound != timing.match_key().map(|k| (k, timing.generation()))
                || !matches!(timing.phase(), Some(Phase::Running | Phase::Paused)))
        {
            self.close(timing, store, false, log);
        }
        let full = vec![Rect {
            x: 0.,
            y: 0.,
            w: 1920.,
            h: 1080.,
        }];
        if !self.open {
            if ctx.ui_exists(PATH) {
                ctx.ui_set_visible(PATH, false);
            }
            return Vec::new();
        }
        if !ctx.ui_exists(PATH) {
            if self
                .spawn
                .is_some_and(|t| t.elapsed() < Duration::from_secs(1))
            {
                return full;
            }
            self.spawn = Some(Instant::now());
            self.cache.clear();
            self.registered = false;
            if !ctx.ui_spawn_source("ingame", &crate::ui_theme::themed(&template()))
                || !ctx.ui_exists(PATH)
            {
                self.close(timing, store, false, log);
                log.write("SETTINGS window spawn failed; pause ownership restored");
                return Vec::new();
            }
        }
        if !self.registered {
            self.registered = true;
            self.register(ctx);
        }
        ctx.ui_set_visible(PATH, true);
        if active_page(self.page, self.advanced) == 6 {
            self.acquisition.sync(ctx, &mut self.draft);
        }
        let capturing = self.capture.is_some();
        self.capture(keys);
        let queued = self
            .events
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default();
        for event in queued {
            self.handle(event, keys, timing, store, log);
        }
        if !capturing && keys.escape && !bit(self.previous, 0x1b) {
            if self.pending.is_some() {
                self.pending = None;
            } else if self.drop.is_some() {
                self.drop = None;
            } else {
                self.close(timing, store, false, log);
            }
        }
        self.previous = bits(&keys.raw.0);
        if !self.open {
            ctx.ui_set_visible(PATH, false);
            return Vec::new();
        }
        let notches = SCROLL.swap(0, Ordering::Relaxed);
        if notches != 0 && self.capture.is_none() && self.pending.is_none() {
            if active_page(self.page, self.advanced) == 6 {
                if self.hovered(ctx, "acquisition.grid_hit", keys.cursor) {
                    self.acquisition.scroll(notches);
                }
            } else {
                self.scroll = (self.scroll - notches as f32 * NOTCH)
                    .clamp(0., max_scroll(active_page(self.page, self.advanced)));
            }
            self.drop = None;
        }
        self.scroll = self
            .scroll
            .clamp(0., max_scroll(active_page(self.page, self.advanced)));
        self.pointer_down = keys.focused && keys.raw.0[1];
        let cursor = keys.cursor.filter(|_| keys.focused);
        self.render_frame(ctx, cursor);
        self.render_content(ctx, cursor, keys);
        self.render_popup(ctx, cursor);
        self.render_conflict(ctx, cursor);
        full
    }
    fn render_frame(&mut self, ctx: &mut StableClient<'_>, cursor: Option<(f32, f32)>) {
        self.visible(ctx, "advanced_nav", self.page == NAV_ADVANCED);
        self.props(
            ctx,
            "mask_top",
            format!(
                "height: {}px;",
                if self.page == NAV_ADVANCED { 172 } else { 112 }
            ),
        );
        if self.page == NAV_ADVANCED {
            for i in 0..ADVANCED.len() {
                let selected = self.advanced == i;
                self.paint(
                    ctx,
                    &format!("advanced_nav.sub{i}"),
                    true,
                    cursor,
                    if selected {
                        (0xfdee00ff, 0xfdee00ff, 0xddd000ff)
                    } else {
                        (
                            crate::ui_theme::tone(0x3a3837ff),
                            crate::ui_theme::tone(0x5b5958ff),
                            crate::ui_theme::tone(0x4b4a49ff),
                        )
                    },
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
        }
        self.text(ctx, "page_title", tr(PAGES[self.page].0));
        self.text(ctx, "page_hint", tr(PAGES[self.page].1));
        self.text(
            ctx,
            "position",
            &format!("{:02} / {:02}", self.page + 1, PAGES.len()),
        );
        self.text(ctx, "nav_note", tr("Settings apply to every\nmatch."));
        self.text(
            ctx,
            "pause",
            if self.paused_by_me {
                tr("Paused for settings")
            } else {
                tr("Match already paused")
            },
        );
        self.text(
            ctx,
            "return",
            if self.paused_by_me {
                tr("Match resumes when you close this window")
            } else {
                tr("Match stays paused when you close this window")
            },
        );
        for (i, (title, _, _)) in PAGES.iter().enumerate() {
            let node = format!("nav{i}");
            let selected = self.page == i;
            let hover = !selected && self.hovered(ctx, &node, cursor);
            let back = if selected {
                0xfdee00ff
            } else {
                self.tone(
                    &node,
                    hover,
                    (
                        crate::ui_theme::rgba(0),
                        crate::ui_theme::tone(0x3a3837ff),
                        crate::ui_theme::tone(0x4b4a49ff),
                    ),
                )
            };
            let ink = if selected {
                "393939ff".into()
            } else {
                hud_motion::color(0xcbc9c7ff, 0xffffffff, if hover { 1. } else { 0. })
            };
            self.props(
                ctx,
                &node,
                format!("btn: {{ back_color: #{back:08x}; color: #00000000; stroke: 0; }}"),
            );
            self.props(ctx, &format!("{node}.icon"), format!("color: #{ink};"));
            self.props(
                ctx,
                &format!("{node}.label"),
                format!("color: #{ink}; line_height: 27;"),
            );
            // The design wraps the first title inside its 126 px label column.
            let title = tr(title).replacen(" & ", " &\n", 1);
            self.text(ctx, &format!("{node}.label"), &title);
        }
        self.paint(
            ctx,
            "close",
            true,
            cursor,
            (
                crate::ui_theme::rgba(0),
                crate::ui_theme::tone(0x3a3837ff),
                crate::ui_theme::tone(0x4b4a49ff),
            ),
            (crate::ui_theme::tone(0x4b4a49ff), 1),
            0xeeececff,
        );
        let close_hover = self.hovered(ctx, "close", cursor);
        self.props(
            ctx,
            "close.icon",
            format!(
                "color: #{};",
                if close_hover { "ffffffff" } else { "cbc9c7ff" }
            ),
        );
        let essential = self.draft.essential() && !self.acquisition.invalid;
        self.paint(
            ctx,
            "reset",
            true,
            cursor,
            (
                crate::ui_theme::rgba(0),
                crate::ui_theme::tone(0x4b4a49ff),
                crate::ui_theme::tone(0x5b5b5bff),
            ),
            (crate::ui_theme::tone(0x4b4a49ff), 1),
            0xcbc9c7ff,
        );
        self.paint(
            ctx,
            "cancel",
            true,
            cursor,
            (
                crate::ui_theme::tone(0x3a3837ff),
                crate::ui_theme::tone(0x4b4a49ff),
                crate::ui_theme::tone(0x5b5b5bff),
            ),
            (crate::ui_theme::tone(0x4b4a49ff), 1),
            0xeeececff,
        );
        if essential {
            self.paint(
                ctx,
                "apply",
                true,
                cursor,
                (0xeeececff, 0xb8b6b5ff, 0x989694ff),
                (0xffffffff, 1),
                0x393939ff,
            );
        } else {
            // Disabled: 40% opacity over the window surface.
            self.paint(
                ctx,
                "apply",
                false,
                cursor,
                (
                    crate::ui_theme::tone(0x6f6d6cff),
                    crate::ui_theme::tone(0x6f6d6cff),
                    crate::ui_theme::tone(0x6f6d6cff),
                ),
                (0x77757480, 1),
                crate::ui_theme::tone(0x3a3837ff),
            );
        }
        let dirty = self.draft.0 != self.original.0;
        self.text(
            ctx,
            "status",
            if self.acquisition.invalid {
                tr("Enter valid acquisition values in Advanced before applying.")
            } else if !essential {
                tr("Bind movement, start and return-to-AI before applying.")
            } else if self.capture.is_some() {
                tr("Listening… Esc cancels; Backspace clears.")
            } else if dirty {
                tr("Unsaved changes")
            } else {
                tr("No changes")
            },
        );
        self.props(
            ctx,
            "status",
            format!(
                "color: #{};",
                if dirty && essential {
                    "b3d543ff"
                } else {
                    "989694ff"
                }
            ),
        );
    }
    fn render_content(
        &mut self,
        ctx: &mut StableClient<'_>,
        cursor: Option<(f32, f32)>,
        keys: Keys,
    ) {
        let page = active_page(self.page, self.advanced);
        let top = view_top(page);
        let (entries, _) = layout(page);
        let scroll = if page == 6 {
            0.
        } else {
            self.motion.value("scroll", self.scroll, 0.167)
        };
        let (mut rows, mut sections, mut heads) = (0, 0, 0);
        let mut hint = false;
        let mut cursor_ex = false;
        let mut acquisition = false;
        let mut emotes = false;
        let mut slots = [None; ROWS];
        for (entry, top) in entries {
            let y = view_top(page) + top - scroll;
            if y + entry.height() <= view_top(page) || y >= VIEW_BOTTOM {
                continue;
            }
            let y = y.round() as i32;
            match entry {
                Entry::Hint => {
                    hint = true;
                    self.props(ctx, "hintbox", format!("visible: true; y: {y}px;"));
                    self.text(
                        ctx,
                        "hintbox.text",
                        if self.capture.is_some() {
                            tr(HINT_LISTEN)
                        } else {
                            tr(HINT_IDLE)
                        },
                    );
                }
                Entry::Section(title) if sections < SECTIONS => {
                    let node = format!("sec{sections}");
                    sections += 1;
                    self.props(ctx, &node, format!("visible: true; y: {y}px;"));
                    let title = tr(title);
                    self.text(ctx, &format!("{node}.text"), title);
                    let start = 30. + crate::hud_style::width(title, 16.) * 1.08 + 12.;
                    self.props(
                        ctx,
                        &format!("{node}.line"),
                        format!(
                            "x: {start:.0}px; width: {:.0}px;",
                            (CONTENT_W as f32 - start).max(0.)
                        ),
                    );
                }
                Entry::Head if heads < HEADS => {
                    self.props(
                        ctx,
                        &format!("head{heads}"),
                        format!("visible: true; y: {y}px;"),
                    );
                    heads += 1;
                }
                Entry::Opt(_) | Entry::Bind(_) if rows < ROWS => {
                    slots[rows] = Some(entry);
                    self.render_row(ctx, rows, entry, y, cursor, keys);
                    rows += 1;
                }
                Entry::Cursor => {
                    cursor_ex = true;
                    let size = self.draft.number("cursor_size") as i32;
                    self.props(ctx, "cursor_ex", format!("visible: true; y: {y}px;"));
                    self.props(
                        ctx,
                        "cursor_ex.img",
                        format!(
                            "width: {size}px; height: {size}px; y: {}px;",
                            (32 - size / 2).max(0)
                        ),
                    );
                    self.props(ctx, "cursor_ex.text", format!("x: {}px;", 24 + size + 16));
                }
                Entry::Acquisition => {
                    acquisition = true;
                    let mut panel = std::mem::take(&mut self.acquisition);
                    panel.render(self, ctx, y, cursor, keys);
                    self.acquisition = panel;
                }
                Entry::Emotes => {
                    emotes = true;
                    let mut panel = std::mem::take(&mut self.emotes);
                    panel.render(self, ctx, y, cursor, keys);
                    self.emotes = panel;
                }
                _ => {}
            }
        }
        self.slots = slots;
        self.visible(ctx, "hintbox", hint);
        self.visible(ctx, "cursor_ex", cursor_ex);
        self.visible(ctx, "acquisition", acquisition);
        self.visible(ctx, "emote_panel", emotes);
        if !emotes {
            self.emotes.hide();
        }
        if !acquisition {
            self.acquisition.hide();
        }
        for i in sections..SECTIONS {
            self.visible(ctx, &format!("sec{i}"), false);
        }
        for i in heads..HEADS {
            self.visible(ctx, &format!("head{i}"), false);
        }
        for i in rows..ROWS {
            self.props(ctx, &format!("row{i}"), "visible: false;".into());
            for node in [
                "seg0",
                "seg1",
                "field",
                "key0",
                "key1",
                "slider",
                "check_hit",
            ] {
                self.props(
                    ctx,
                    &format!("row{i}.{node}"),
                    "visible: false; ignore_event: true;".into(),
                );
            }
        }
        // Scrollbar.
        let max = max_scroll(active_page(self.page, self.advanced));
        let view = VIEW_BOTTOM - top;
        self.props(
            ctx,
            "scroll_track",
            format!("y: {top}px; height: {view}px;"),
        );
        self.visible(ctx, "scroll_track", max > 0.);
        self.visible(ctx, "scroll_thumb", max > 0.);
        if max > 0. {
            let thumb = (view * view / (view + max)).max(40.);
            let y = top + (view - thumb) * (scroll / max).clamp(0., 1.);
            self.props(
                ctx,
                "scroll_thumb",
                format!("y: {y:.1}px; height: {thumb:.1}px;"),
            );
        }
    }
    fn render_row(
        &mut self,
        ctx: &mut StableClient<'_>,
        i: usize,
        entry: Entry,
        y: i32,
        cursor: Option<(f32, f32)>,
        keys: Keys,
    ) {
        let row = format!("row{i}");
        let h = entry.height() as i32;
        self.props(
            ctx,
            &row,
            format!("visible: true; y: {y}px; height: {h}px;"),
        );
        self.props(ctx, &format!("{row}.shade"), format!("y: {}px;", h - 1));
        let hidden = "visible: false; ignore_event: true;";
        let mut show = [false; 6]; // segmented, field, slider, checkbox, colour, keys
        match entry {
            Entry::Bind(index) => {
                let def = &BINDINGS[index];
                self.text(ctx, &format!("{row}.label"), tr(def.label));
                self.props(
                    ctx,
                    &format!("{row}.label"),
                    "visible: true; y: 14px; height: 42px;".into(),
                );
                self.props(ctx, &format!("{row}.hint"), "visible: false;".into());
                show[5] = true;
                for j in 0..2 {
                    let node = format!("{row}.key{j}");
                    let capturing = self.capture == Some((index, j));
                    let chord = self.draft.binding(def.key)[j];
                    let text = if capturing {
                        tr("Press input…").to_string()
                    } else {
                        chord.map_or_else(|| tr("Unbound").into(), Chord::label)
                    };
                    self.props(ctx, &node, format!("text: {{ text: {}; }}", json(&text)));
                    let (colors, border, ink) = if capturing {
                        ((0xfdee00ff, 0xfdee00ff, 0xfdee00ff), 0xfdee00ff, 0x393939ff)
                    } else if chord.is_some() {
                        ((0xeeececff, 0xb8b6b5ff, 0x989694ff), 0xffffffff, 0x393939ff)
                    } else {
                        (
                            (
                                crate::ui_theme::tone(0x3a3837ff),
                                crate::ui_theme::tone(0x3a3837ff),
                                crate::ui_theme::tone(0x4b4a49ff),
                            ),
                            crate::ui_theme::tone(0x5b5b5bff),
                            0x989694ff,
                        )
                    };
                    self.paint(ctx, &node, true, cursor, colors, (border, 1), ink);
                }
            }
            Entry::Opt(index) => {
                let def = &OPTIONS[index];
                self.text(ctx, &format!("{row}.label"), tr(def.label));
                self.text(ctx, &format!("{row}.hint"), tr(def.hint));
                self.props(
                    ctx,
                    &format!("{row}.label"),
                    "y: 13px; height: 30px;".into(),
                );
                self.props(
                    ctx,
                    &format!("{row}.hint"),
                    format!("visible: {};", !matches!(def.control, Control::Toggle)),
                );
                self.props(
                    ctx,
                    &format!("{row}.label"),
                    format!("visible: {};", !matches!(def.control, Control::Toggle)),
                );
                let value = self.draft.number(def.key);
                match def.control {
                    Control::Toggle => {
                        show[3] = true;
                        let hit = format!("{row}.check_hit");
                        let hover = self.hovered(ctx, &hit, cursor);
                        let checked = value >= 0.5;
                        let fill = self.tone(
                            &format!("check_{}_{checked}", def.key),
                            hover,
                            if checked {
                                (0x434242ff, 0x383737ff, 0x2e2d2dff)
                            } else {
                                (0xeeececff, 0x8e8e8eff, 0x707070ff)
                            },
                        );
                        let border = if checked { "302f2eff" } else { "201e1dff" };
                        self.props(
                            ctx,
                            &format!("{row}.check_box"),
                            format!("visible: true; color: #{border};"),
                        );
                        self.props(
                            ctx,
                            &format!("{row}.check_box.fill"),
                            format!("color: #{fill:08x};"),
                        );
                        self.props(
                            ctx,
                            &format!("{row}.check_box.rail"),
                            format!(
                                "visible: {checked}; width: {}px;",
                                if hover { 4 } else { 3 }
                            ),
                        );
                        self.props(
                            ctx,
                            &format!("{row}.check_box.mark"),
                            format!("visible: {checked}; x: {}px;", if hover { 7 } else { 6 }),
                        );
                        self.text(ctx, &format!("{row}.check_label"), tr(def.label));
                        self.text(ctx, &format!("{row}.check_hint"), tr(def.hint));
                        for node in ["check_label", "check_hint"] {
                            self.visible(ctx, &format!("{row}.{node}"), true);
                        }
                        self.props(
                            ctx,
                            &hit,
                            "visible: true; ignore_event: false; btn: { back_color: #00000000; color: #00000000; stroke: 0; }".into(),
                        );
                    }
                    Control::Choice(options) if options.len() == 2 => {
                        show[0] = true;
                        let hover =
                            (0..2).any(|j| self.hovered(ctx, &format!("{row}.seg{j}"), cursor));
                        let at =
                            self.motion
                                .value(&format!("ind_{}", def.key), value as f32, 0.167);
                        let track = self.tone(
                            &format!("track_{}", def.key),
                            hover,
                            (crate::ui_theme::tone(0x5b5b5bff), 0x7f7f7fff, 0x6a6a6aff),
                        );
                        self.props(
                            ctx,
                            &format!("{row}.track"),
                            format!("visible: true; color: #{track:08x};"),
                        );
                        self.props(
                            ctx,
                            &format!("{row}.track.ind"),
                            format!("x: {:.1}px; color: #eeececff;", at * 183.),
                        );
                        self.props(
                            ctx,
                            &format!("{row}.track.pill_shadow"),
                            format!("x: {:.1}px;", at * 183. - 2.),
                        );
                        for (j, text) in options.iter().enumerate() {
                            let sel = self.motion.tonal(
                                &format!("sel_{}_{j}", def.key),
                                f32::from(value as usize == j),
                                0.1,
                            );
                            self.text(ctx, &format!("{row}.track.opt{j}"), text);
                            self.props(
                                ctx,
                                &format!("{row}.track.opt{j}"),
                                format!(
                                    "color: #{};",
                                    hud_motion::color(0xb8b6b5ff, 0x393939ff, sel)
                                ),
                            );
                            self.props(
                                ctx,
                                &format!("{row}.seg{j}"),
                                "visible: true; ignore_event: false; btn: { back_color: #00000000; color: #00000000; stroke: 0; }".into(),
                            );
                        }
                    }
                    Control::Choice(options) => {
                        show[1] = true;
                        let open = self.drop == Some(index);
                        let hover = !open && self.hovered(ctx, &format!("{row}.field"), cursor);
                        let back = self.tone(
                            &format!("field_{}", def.key),
                            hover,
                            (0xeeececff, 0x8e8e8eff, 0x707070ff),
                        );
                        let ink = if hover { "ffffffff" } else { "393939ff" };
                        self.props(ctx,&format!("{row}.field"),format!("visible: true; ignore_event: false; btn: {{ back_color: #{back:08x}; color: #~242221ff; stroke: 1; }}"));
                        self.text(
                            ctx,
                            &format!("{row}.field.value"),
                            tr(options[value as usize]),
                        );
                        self.props(
                            ctx,
                            &format!("{row}.field.value"),
                            format!("color: #{ink};"),
                        );
                        self.props(
                            ctx,
                            &format!("{row}.field.caret"),
                            format!("color: #{ink};"),
                        );
                    }
                    Control::Color => {
                        show[4] = true;
                        self.render_color(ctx, &row, index, value as u32);
                    }
                    Control::Slider(lo, hi, step, unit) => {
                        show[2] = true;
                        self.props(
                            ctx,
                            &format!("{row}.slider"),
                            "visible: true; ignore_event: false;".into(),
                        );
                        self.props(ctx, &format!("{row}.number"), "visible: true;".into());
                        let node = format!("{PATH}.window.{row}.slider");
                        let dragging = keys.focused
                            && keys.raw.0[1]
                            && self.drop.is_none()
                            && self.hovered(ctx, &format!("{row}.slider"), cursor);
                        let mut number = value;
                        if dragging {
                            if let Some(r) = ctx.ui_slider_ratio(&node).filter(|r| r.is_finite()) {
                                number = if def.key == "cursor_size" {
                                    crate::cursor::size_from_ratio(r) as f64
                                } else {
                                    ((lo + r.clamp(0., 1.) * (hi - lo)) / step).round() * step
                                };
                                self.draft.set(def.key, number);
                            }
                        } else {
                            ctx.ui_set_slider_ratio(&node, (value - lo) / (hi - lo));
                        }
                        let ratio = if def.key == "cursor_size" {
                            crate::cursor::ratio_from_size(number as u32)
                        } else {
                            (number - lo) / (hi - lo)
                        };
                        let hover = self.hovered(ctx, &format!("{row}.slider"), cursor);
                        let thumb = self.tone(
                            &format!("thumb_{}", def.key),
                            hover || dragging,
                            (0xeeececff, 0x8e8e8eff, 0x8e8e8eff),
                        );
                        self.props(
                            ctx,
                            &format!("{row}.slider.fill"),
                            format!("width: {:.2}%;", ratio * 100.),
                        );
                        self.props(
                            ctx,
                            &format!("{row}.slider.thumb"),
                            format!(
                                "x: {:.2}px; color: #{thumb:08x};",
                                (ratio * 294.).clamp(25., 269.)
                            ),
                        );
                        self.text(
                            ctx,
                            &format!("{row}.number"),
                            &if step >= 1. {
                                format!("{}{unit}", number as i32)
                            } else {
                                format!("{number:.1}{unit}")
                            },
                        );
                    }
                }
            }
            _ => {}
        }
        if !show[0] {
            self.props(ctx, &format!("{row}.track"), "visible: false;".into());
            for j in 0..2 {
                self.props(ctx, &format!("{row}.seg{j}"), hidden.into());
            }
        }
        if !show[3] {
            for node in ["check_box", "check_label", "check_hint"] {
                self.visible(ctx, &format!("{row}.{node}"), false);
            }
            self.props(ctx, &format!("{row}.check_hit"), hidden.into());
        }
        if !show[1] {
            self.props(ctx, &format!("{row}.field"), hidden.into());
        }
        if !show[2] {
            self.props(ctx, &format!("{row}.slider"), hidden.into());
            self.props(ctx, &format!("{row}.number"), "visible: false;".into());
        }
        if !show[5] {
            for j in 0..2 {
                self.props(ctx, &format!("{row}.key{j}"), hidden.into());
            }
        }
        if !show[4] {
            self.visible(ctx, &format!("{row}.hex_box"), false);
        }
    }
    /// The colour row: typed text updates the draft when it is a valid
    /// "#rrggbb"; a draft changed elsewhere (Reset) rewrites the text. The
    /// native field draws beneath the window, so its text is mirrored.
    fn render_color(&mut self, ctx: &mut StableClient<'_>, row: &str, index: usize, current: u32) {
        let def = &OPTIONS[index];
        self.visible(ctx, &format!("{row}.hex_box"), true);
        let node = format!("{PATH}.window.{row}.hex_box.hex");
        if let Some(text) = ctx.ui_text_edit_text(&node) {
            if self.hex.0 != Some(index) || (text == self.hex.1 && current != self.hex.2) {
                // First shown, or reset: show the draft colour.
                let shown = crate::ui_theme::format(current);
                ctx.ui_set_text_edit_text(&node, &shown);
                self.hex = (Some(index), shown, current);
            } else if text != self.hex.1 {
                if let Some(rgb) = crate::ui_theme::parse(&text) {
                    self.draft.set(def.key, f64::from(rgb));
                    self.hex.2 = rgb;
                }
                self.hex.1 = text;
            }
        }
        let valid = crate::ui_theme::parse(&self.hex.1).is_some();
        let editing = ctx
            .ui_state_json(&node)
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("is_editing").and_then(serde_json::Value::as_bool))
            .unwrap_or(false);
        let label = format!("{row}.hex_box.hex_value");
        let text = self.hex.1.clone();
        self.text(ctx, &label, &text);
        self.props(
            ctx,
            &label,
            format!("color: #{};", if valid { "eeececff" } else { "ff642eff" }),
        );
        self.props(
            ctx,
            &format!("{row}.hex_box.hex_focus"),
            format!("visible: {editing}; color: #fdee00ff;"),
        );
        self.props(
            ctx,
            &format!("{row}.hex_box.swatch"),
            // On a stroked node `color` is the edge, `back_color` the fill.
            format!("back_color: #{:06x}ff;", self.hex.2),
        );
    }
    fn render_popup(&mut self, ctx: &mut StableClient<'_>, cursor: Option<(f32, f32)>) {
        // Locate the open select's field in the current frame; scrolled-away menus close.
        let at = self.drop.and_then(|index| {
            let slot = self
                .slots
                .iter()
                .position(|s| *s == Some(Entry::Opt(index)))?;
            let row = ctx.ui_node_rect(&format!("{PATH}.window.row{slot}"))?;
            let win = ctx.ui_node_rect(&format!("{PATH}.window"))?;
            let scale = win.3 / 850.;
            let y = ((row.1 - win.1) / scale).round() as i32 + 13;
            (y >= VIEW_TOP as i32 && y + 50 <= VIEW_BOTTOM as i32).then_some((index, y))
        });
        if self.drop.is_some() && at.is_none() {
            self.drop = None;
        }
        let fade = self.motion.tonal(
            "popup",
            f32::from(at.is_some()),
            if at.is_some() { 0.117 } else { 0.05 },
        );
        self.props(
            ctx,
            "popup",
            format!("visible: {};", at.is_some() || fade > 0.02),
        );
        let Some((index, y)) = at else {
            for k in 0..POPUP_MAX {
                self.props(ctx, &format!("popup.opt{k}"), "ignore_event: true;".into());
            }
            return;
        };
        let def = &OPTIONS[index];
        let Control::Choice(options) = def.control else {
            return;
        };
        let value = self.draft.number(def.key) as usize;
        let grid = options.len() > 3;
        let (cols, row_h, col_w) = if grid { (2, 40, 193) } else { (1, 64, 386) };
        let rows = options.len().div_ceil(cols) as i32;
        // The field copy (38 px) plus the choices.
        let height = 38 + if grid { rows * row_h + 4 } else { 192 };
        // Open downward unless the menu would leave the window.
        let up = y + 25 + height > 850 - 8;
        let menu = if up { 25 - height } else { 25 };
        let first = if up { menu } else { 63 };
        let rule = if up { -13 } else { 62 };
        let alpha = (fade * 255.) as u8;
        self.props(ctx, "popup", format!("y: {y}px;"));
        self.props(
            ctx,
            "popup.menu",
            format!("y: {menu}px; height: {height}px; color: #d4d2d3{alpha:02x};"),
        );
        self.props(
            ctx,
            "popup.shadow",
            format!(
                "y: {}px; height: {}px; color: #000000{:02x};",
                menu + 3,
                height + 2,
                (f32::from(alpha) * 0.4) as u8
            ),
        );
        self.props(
            ctx,
            "popup.rule",
            format!("y: {rule}px; color: #c6c4c5{alpha:02x};"),
        );
        self.text(ctx, "popup.field.value", tr(options[value]));
        for k in 0..POPUP_MAX {
            let node = format!("popup.opt{k}");
            let present = k < options.len();
            let (col, row) = ((k % cols) as i32, (k / cols) as i32);
            let (ox, oy) = (col * col_w, first + row * row_h);
            if k < 2 {
                self.props(
                    ctx,
                    &format!("popup.div{k}"),
                    format!(
                        "y: {}px; visible: {}; color: #c6c4c5{alpha:02x};",
                        oy + 63,
                        !grid && k + 1 < options.len()
                    ),
                );
            }
            if !present {
                self.props(ctx, &node, "visible: false; ignore_event: true;".into());
                continue;
            }
            let selected = k == value;
            let hover = !selected && self.hovered(ctx, &node, cursor);
            let back = if selected {
                0x5f5f5fff
            } else {
                self.tone(&node, hover, (0xd4d2d300, 0x8a8989ff, 0x707070ff))
            };
            let back = (back & 0xffffff00) | ((back & 0xff) * u32::from(alpha) / 255);
            self.props(ctx,&node,format!("visible: true; ignore_event: false; x: {ox}px; y: {oy}px; width: {col_w}px; height: {}px; btn: {{ back_color: #{back:08x}; color: #00000000; stroke: 0; }}", row_h - 1));
            // List: the template's geometry; grid: smaller rows and text.
            let (text_w, inner, text_size, bar_y, check_y) = if grid {
                (
                    col_w - 66,
                    row_h - 1,
                    15,
                    (row_h - 21) / 2,
                    (row_h - 19) / 2,
                )
            } else {
                (300, 63, 17, 22, 23)
            };
            self.props(
                ctx,
                &format!("{node}.text"),
                format!("width: {text_w}px; height: {inner}px; size: {text_size};"),
            );
            self.props(ctx, &format!("{node}.bar"), format!("y: {bar_y}px;"));
            self.props(
                ctx,
                &format!("{node}.check"),
                format!("x: {}px; y: {check_y}px;", col_w - 40),
            );
            self.text(ctx, &format!("{node}.text"), tr(options[k]));
            let ink = if selected { "ffffff" } else { "393939" };
            self.props(
                ctx,
                &format!("{node}.text"),
                format!("color: #{ink}{alpha:02x};"),
            );
            self.props(
                ctx,
                &format!("{node}.bar"),
                format!("visible: {selected}; color: #ffffff{alpha:02x};"),
            );
            self.props(
                ctx,
                &format!("{node}.check"),
                format!("visible: {selected}; color: #ffffff{alpha:02x};"),
            );
        }
    }
    fn render_conflict(&mut self, ctx: &mut StableClient<'_>, cursor: Option<(f32, f32)>) {
        let on = self.pending.is_some();
        for node in [
            "conflict_shade",
            "conflict",
            "conflict_title",
            "conflict_text",
        ] {
            self.visible(ctx, node, on);
        }
        if !on {
            for node in ["replace", "dismiss"] {
                self.props(ctx, node, "visible: false; ignore_event: true;".into());
            }
            return;
        }
        self.paint(
            ctx,
            "dismiss",
            true,
            cursor,
            (
                crate::ui_theme::tone(0x3a3837ff),
                crate::ui_theme::tone(0x4b4a49ff),
                crate::ui_theme::tone(0x5b5b5bff),
            ),
            (crate::ui_theme::tone(0x4b4a49ff), 1),
            0xeeececff,
        );
        self.paint(
            ctx,
            "replace",
            true,
            cursor,
            (0xeeececff, 0xb8b6b5ff, 0x989694ff),
            (0xffffffff, 1),
            0x393939ff,
        );
        if let Some((index, slot, chord)) = self.pending {
            let names = self
                .draft
                .conflicts(chord, BINDINGS[index].key, slot)
                .into_iter()
                .map(|(key, _)| tr(BINDINGS.iter().find(|d| d.key == key).unwrap().label))
                .collect::<Vec<_>>()
                .join(", ");
            let copy = trf(
                "{key} is assigned to “{names}”. Replace it with “{action}”?",
                &[
                    ("key", &chord.label()),
                    ("names", &names),
                    ("action", &tr(BINDINGS[index].label)),
                ],
            );
            let (copy, _) = crate::hud_style::wrap(&copy, 18., 516.);
            self.text(ctx, "conflict_text", &copy);
        }
    }
}
fn should_resume(
    paused_by_me: bool,
    bound: Option<(MatchKey, u64)>,
    current: Option<(MatchKey, u64)>,
    phase: Option<Phase>,
) -> bool {
    paused_by_me && bound.is_some() && bound == current && phase == Some(Phase::Paused)
}
fn bits(raw: &[bool; 256]) -> [u64; 4] {
    std::array::from_fn(|word| (0..64).fold(0, |b, i| b | (u64::from(raw[word * 64 + i]) << i)))
}
fn bit(bits: [u64; 4], i: usize) -> bool {
    bits[i / 64] & (1u64 << (i % 64)) != 0
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resume_only_our_pause_in_the_same_session() {
        let bound = Some(((1, 2, 3), 4));
        assert!(should_resume(true, bound, bound, Some(Phase::Paused)));
        assert!(!should_resume(false, bound, bound, Some(Phase::Paused)));
        assert!(!should_resume(
            true,
            bound,
            Some(((1, 2, 3), 5)),
            Some(Phase::Paused)
        ));
        assert!(!should_resume(true, bound, bound, Some(Phase::Running)));
        assert!(!should_resume(true, bound, bound, Some(Phase::Released)));
    }
    #[test]
    fn capture_waits_for_opening_click_and_handles_conflict_and_escape() {
        let mut ui = SettingsUi {
            capture: Some((6, 0)),
            ..Default::default()
        };
        let mut keys = Keys {
            focused: true,
            ..Keys::default()
        };
        keys.raw.0[1] = true;
        ui.capture(keys);
        assert!(!ui.capture_armed);
        keys.raw.0[1] = false;
        ui.capture(keys);
        assert!(ui.capture_armed);
        keys.raw.0[0x57] = true;
        ui.capture(keys);
        assert_eq!(
            ui.pending,
            Some((
                6,
                0,
                Chord {
                    code: 0x57,
                    mods: 0
                }
            ))
        );
        assert_eq!(
            ui.draft.binding("q")[0],
            Some(Chord {
                code: 0x51,
                mods: 0
            })
        );
        ui.capture = Some((6, 0));
        ui.pending = None;
        keys.raw.0[0x57] = false;
        keys.raw.0[0x1b] = true;
        ui.capture(keys);
        assert!(ui.capture.is_none());
        assert!(ui.pending.is_none());
    }
    #[test]
    fn layout_matches_measured_design_spacing() {
        // Advanced has its own fixed body; Combat retains its original spacing.
        let (combat, _) = layout(0);
        assert_eq!(combat[0], (Entry::Section("Targeting"), 0.));
        assert_eq!(combat[1].1, 34.);
        assert_eq!(combat[2].1, 124.);
        assert_eq!(combat[4], (Entry::Section("Ability casting"), 322.));
        assert_eq!(layout(6), (vec![(Entry::Acquisition, 0.)], 475.));
        assert_eq!(max_scroll(4), 0.);
        assert!(!combat.iter().any(|(e, _)| *e == Entry::Section("Attacks")));
        assert!(layout(4)
            .0
            .iter()
            .any(|(e, _)| *e == Entry::Section("Attacks")));
        // Keybinds: hint, section at 69, header at 103, first row at 132.
        let (keys, _) = layout(1);
        assert_eq!(keys[0], (Entry::Hint, 0.));
        assert_eq!(keys[1].1, 69.);
        assert_eq!(keys[2], (Entry::Head, 103.));
        assert_eq!(keys[3], (Entry::Bind(0), 132.));
        assert_eq!(
            keys.iter()
                .filter(|(e, _)| matches!(e, Entry::Bind(_)))
                .count(),
            BINDINGS.len()
        );
        // Interface: the Language section (142 px) comes first; the cursor
        // preview follows the size row; next section at 212 + 142.
        let (iface, _) = layout(3);
        assert_eq!(iface[0], (Entry::Section("Language"), 0.));
        assert_eq!(iface[4], (Entry::Cursor, 124. + 142.));
        assert_eq!(iface[5].1, 212. + 142.);
        // Camera fits; the new Interface debug section uses the existing
        // scrollbar and retains the design's 24 px bottom padding.
        assert!(max_scroll(1) > 0.);
        assert!(max_scroll(0) > 0.);
        assert_eq!(max_scroll(2), 0.);
        // Emotes have a fixed body; Interface retains its scrolling viewport.
        assert_eq!(max_scroll(3), 57. + 142.);
        assert_eq!(max_scroll(PAGE_EMOTES), 0.);
        let (camera, _) = layout(2);
        assert_eq!(camera[1].1, 34.);
        assert_eq!(camera[1].0.height(), 56.);
    }
    #[test]
    fn visible_rows_never_exceed_the_slot_pools() {
        for page in 0..=7 {
            let (entries, _) = layout(page);
            let mut scroll = 0.;
            while scroll <= max_scroll(page) {
                let shown: Vec<_> = entries
                    .iter()
                    .filter(|(e, top)| {
                        let y = view_top(page) + top - scroll;
                        y + e.height() > view_top(page) && y < VIEW_BOTTOM
                    })
                    .collect();
                let rows = shown
                    .iter()
                    .filter(|(e, _)| matches!(e, Entry::Opt(_) | Entry::Bind(_)))
                    .count();
                let sections = shown
                    .iter()
                    .filter(|(e, _)| matches!(e, Entry::Section(_)))
                    .count();
                let heads = shown
                    .iter()
                    .filter(|(e, _)| matches!(e, Entry::Head))
                    .count();
                assert!(
                    rows <= ROWS && sections <= SECTIONS && heads <= HEADS,
                    "page {page} at {scroll}"
                );
                scroll += 7.;
            }
        }
    }
    #[test]
    fn events_resolve_through_the_slot_shown_last_frame() {
        let mut ui = SettingsUi::default();
        let q = OPTIONS.iter().position(|d| d.key == "cast_q").unwrap();
        let hold = OPTIONS
            .iter()
            .position(|d| d.key == "champion_mode")
            .unwrap();
        ui.slots[0] = Some(Entry::Opt(hold));
        ui.slots[1] = Some(Entry::Opt(q));
        let keys = Keys::default();
        let timing = NativeTiming::new(false);
        let store = settings::Settings::new(None);
        let log = crate::test_support::logger("settings-ui-events");
        ui.handle(Event::Seg(0, 1), keys, &timing, &store, &log);
        assert_eq!(ui.draft.number("champion_mode"), 1.);
        let attack_cancel = OPTIONS
            .iter()
            .position(|d| d.key == "attack_cancel")
            .unwrap();
        ui.slots[0] = Some(Entry::Opt(attack_cancel));
        ui.handle(Event::Toggle(0), keys, &timing, &store, &log);
        assert_eq!(ui.draft.number("attack_cancel"), 0.);
        ui.handle(Event::Seg(0, 1), keys, &timing, &store, &log);
        assert_eq!(ui.draft.number("attack_cancel"), 0.);
        ui.handle(Event::Field(1), keys, &timing, &store, &log);
        assert_eq!(ui.drop, Some(q));
        ui.handle(Event::Popup(2), keys, &timing, &store, &log);
        assert_eq!(ui.draft.number("cast_q"), 2.);
        assert_eq!(ui.drop, None);
        // A click on an empty or non-matching slot changes nothing.
        ui.handle(Event::Key(1, 0), keys, &timing, &store, &log);
        assert!(ui.capture.is_none());
    }
    #[test]
    fn advanced_navigation_clears_clipping_mask_and_switches_all_subpages() {
        let source = template();
        let mask = source.find("#mask_bottom:color").unwrap();
        let nav = source.find("#advanced_nav:empty").unwrap();
        assert!(
            nav > mask,
            "navigation subtree must follow the blocking masks"
        );
        let header = &source[nav..source[nav..].find("#sub0:").unwrap() + nav];
        assert!(header.contains("ignore_event: true; z: 2013;"));
        // Mask z=2012 and child buttons z=2014. Keep the container itself above
        // the mask too; visible drawing alone does not establish click access.
        assert!(source.contains("ignore_event: false; z: 2012;"));
        let timing = NativeTiming::new(false);
        let store = settings::Settings::new(None);
        let log = crate::test_support::logger("advanced-subpages");
        let mut ui = SettingsUi {
            page: NAV_ADVANCED,
            ..Default::default()
        };
        crate::acquisition::set_custom(&mut ui.draft, "ghost", Some(85.));
        for (subpage, schema) in [(1, 6), (2, 5), (0, 4), (1, 6)] {
            ui.scroll = 170.;
            ui.slots[0] = Some(Entry::Opt(0));
            ui.handle(
                Event::Advanced(subpage),
                Keys::default(),
                &timing,
                &store,
                &log,
            );
            assert_eq!(active_page(ui.page, ui.advanced), schema);
            assert_eq!(ui.scroll, 0.);
            assert!(ui.slots.iter().all(Option::is_none));
            assert_eq!(crate::acquisition::custom(&ui.draft, "ghost"), Some(85.));
            if schema == 6 {
                assert_eq!(layout(schema).0, vec![(Entry::Acquisition, 0.)]);
            } else {
                assert!(layout(schema)
                    .0
                    .iter()
                    .any(|(e, _)| matches!(e, Entry::Opt(_))));
            }
        }
        ui.handle(Event::Page(3), Keys::default(), &timing, &store, &log);
        ui.handle(Event::Advanced(2), Keys::default(), &timing, &store, &log);
        assert_eq!(
            (ui.page, ui.advanced),
            (3, 1),
            "hidden subpage events cannot change other pages"
        );
    }
    #[test]
    fn emote_draft_reset_cancel_and_apply_remain_transactional() {
        let timing = NativeTiming::new(false);
        let store = settings::Settings::new(None);
        let log = crate::test_support::logger("emote-settings-transaction");
        let mut ui = SettingsUi {
            open: true,
            page: NAV_EMOTES,
            draft: store.snapshot(),
            ..Default::default()
        };
        ui.draft.set("emote_height", 180.);
        ui.draft.set("emote_scale", 150.);
        ui.draft.set("emote_cooldown", 0.5);
        let library = crate::emote_library::Snapshot::default();
        crate::emote_library::assign(&mut ui.draft, 0, &library.entries[3].art.id, &library);
        assert_eq!(store.number("emote_height"), 108.);
        ui.close(&timing, &store, false, &log);
        assert_eq!(store.number("emote_scale"), 100.);
        assert!(store.snapshot().0.get("emote_slots").is_none());
        ui.open = true;
        ui.draft = store.snapshot();
        ui.draft.set("emote_scale", 200.);
        crate::emote_library::assign(&mut ui.draft, 1, &library.entries[4].art.id, &library);
        ui.handle(Event::Reset, Keys::default(), &timing, &store, &log);
        assert_eq!(ui.draft.number("emote_scale"), 100.);
        assert_eq!(ui.draft.number("emote_cooldown"), 1.5);
        assert!(ui.draft.0.get("emote_slots").is_none());
        ui.draft.set("emote_height", 120.);
        ui.draft.set("emote_scale", 150.);
        ui.draft.set("emote_zoom", 1.);
        ui.draft.set("emote_cooldown", 0.25);
        crate::emote_library::assign(&mut ui.draft, 2, &library.entries[0].art.id, &library);
        ui.close(&timing, &store, true, &log);
        assert_eq!(store.number("emote_height"), 120.);
        assert_eq!(store.number("emote_scale"), 150.);
        assert_eq!(store.number("emote_zoom"), 1.);
        assert_eq!(store.number("emote_cooldown"), 0.25);
        assert_eq!(
            library.assigned(&store.snapshot(), 2).art.id,
            library.entries[0].art.id
        );
    }
    #[test]
    fn export_window() {
        if let Ok(dir) = std::env::var("LT_HUD_EXPORT_DIR") {
            std::fs::write(std::path::Path::new(&dir).join("settings.ui"), template()).unwrap();
        }
        let t = template();
        assert!(t.contains("1360px"));
        assert!(!t.contains("\\n"), "escaped line breaks render literally");
        for i in 0..ROWS {
            assert!(t.contains(&format!("#row{i}:color")));
        }
    }
    #[test]
    fn acquisition_draft_cancel_apply_and_invalid_input_follow_window_transaction() {
        let timing = NativeTiming::new(false);
        let store = settings::Settings::new(None);
        let log = crate::test_support::logger("acquisition-settings-transaction");
        let mut ui = SettingsUi {
            open: true,
            draft: store.snapshot(),
            ..Default::default()
        };
        crate::acquisition::set_custom(&mut ui.draft, "mod/hero", Some(145.));
        assert_eq!(
            crate::acquisition::custom(&store.snapshot(), "mod/hero"),
            None
        );
        ui.close(&timing, &store, false, &log);
        assert_eq!(
            crate::acquisition::custom(&store.snapshot(), "mod/hero"),
            None
        );
        ui.open = true;
        ui.acquisition.invalid = true;
        ui.close(&timing, &store, true, &log);
        assert!(ui.open);
        assert_eq!(
            crate::acquisition::custom(&store.snapshot(), "mod/hero"),
            None
        );
        ui.acquisition.invalid = false;
        ui.close(&timing, &store, true, &log);
        assert!(!ui.open);
        assert_eq!(
            crate::acquisition::custom(&store.snapshot(), "mod/hero"),
            Some(145.)
        );
    }
    #[test]
    fn raw_edges_do_not_confuse_high_keys() {
        let mut raw = [false; 256];
        raw[0xc0] = true;
        let b = bits(&raw);
        assert!(bit(b, 0xc0));
        assert!(!bit(b, 1));
    }
}
