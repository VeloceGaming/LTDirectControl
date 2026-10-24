//! A stand-in for the game's UI tree (test builds only, 1.6.0-perf).
//!
//! It answers the SDK's UI calls on the test's own thread and records
//! every call that changes the screen (properties, text, visibility,
//! spawn, removal, click registration) in order; a visibility write that
//! changes nothing is not recorded. Window tests drive a
//! sequence of frames through a window and compare the record with a
//! saved one (`ui_identity.rs`): the same record means the game received
//! the same writes, byte for byte.
//!
//! The model is deliberately small: a spawned window's root and every path
//! under it exist; a node is visible until a write hides it; node
//! rectangles are the whole 1920 x 1080 screen unless a test sets one.
use mod_api_stable::{ClientCtxV1, StableClient, UiVtableV1};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;

#[derive(Default)]
pub struct FakeUi {
    /// Every screen-changing call, in order.
    pub log: Vec<String>,
    roots: Vec<String>,
    visible: HashMap<String, bool>,
    rects: HashMap<String, (f32, f32, f32, f32)>,
    /// While set, property and text writes are refused (a failing host).
    pub refuse_writes: bool,
}
thread_local! {
    static UI: RefCell<FakeUi> = RefCell::new(FakeUi::default());
}
/// Start over: no windows, an empty record.
pub fn reset() {
    UI.with(|u| *u.borrow_mut() = FakeUi::default());
}
pub fn with<R>(f: impl FnOnce(&mut FakeUi) -> R) -> R {
    UI.with(|u| f(&mut u.borrow_mut()))
}
/// The record so far, emptied.
pub fn take_log() -> Vec<String> {
    with(|u| std::mem::take(&mut u.log))
}
pub fn set_rect(path: &str, rect: (f32, f32, f32, f32)) {
    with(|u| {
        u.rects.insert(path.into(), rect);
    });
}
/// A path that exists in the model (the game's own root "ingame" always).
pub fn exists(path: &str) -> bool {
    with(|u| u.exists(path))
}
impl FakeUi {
    fn exists(&self, path: &str) -> bool {
        path == "ingame"
            || self.roots.iter().any(|r| {
                path == r
                    || (path.len() > r.len()
                        && path.starts_with(r.as_str())
                        && path.as_bytes()[r.len()] == b'.')
            })
    }
    fn visible_of(&self, path: &str) -> bool {
        *self.visible.get(path).unwrap_or(&true)
    }
}
/// The node id a template source starts with (`id:runner { ... }`).
fn source_id(source: &str) -> Option<&str> {
    let id = source.trim_start().split(':').next()?.trim();
    (!id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).then_some(id)
}
/// The top-level `visible:` value of a property source, if any.
fn top_visible(source: &str) -> Option<bool> {
    let mut depth = 0;
    let mut start = 0;
    for (i, c) in source.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            ';' if depth == 0 => {
                if let Some(("visible", v)) = source[start..i]
                    .split_once(':')
                    .map(|(k, v)| (k.trim(), v.trim()))
                {
                    return Some(v == "true");
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    None
}
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    })
}
unsafe fn text<'a>(ptr: *const u8, len: usize) -> &'a str {
    std::str::from_utf8(std::slice::from_raw_parts(ptr, len)).unwrap_or("<bad utf8>")
}

unsafe extern "C" fn query_exists(_: *const c_void, p: *const u8, n: usize) -> bool {
    exists(text(p, n))
}
unsafe extern "C" fn get_visible(_: *const c_void, p: *const u8, n: usize, out: *mut bool) -> bool {
    let path = text(p, n);
    with(|u| {
        if !u.exists(path) {
            return false;
        }
        *out = u.visible_of(path);
        true
    })
}
unsafe extern "C" fn set_visible(_: *mut c_void, p: *const u8, n: usize, v: bool) -> bool {
    let path = text(p, n);
    with(|u| {
        if !u.exists(path) {
            return false;
        }
        // A write that leaves the visibility as it is changes nothing on
        // screen; 1.6.0 stops sending those, so the record leaves them out.
        if u.visible_of(path) != v {
            u.log.push(format!("V {path} {v}"));
            u.visible.insert(path.into(), v);
        }
        true
    })
}
unsafe extern "C" fn set_text(
    _: *mut c_void,
    p: *const u8,
    n: usize,
    t: *const u8,
    tn: usize,
) -> bool {
    let (path, value) = (text(p, n), text(t, tn));
    with(|u| {
        if !u.exists(path) || u.refuse_writes {
            return false;
        }
        u.log.push(format!("T {path} = {value}"));
        true
    })
}
unsafe extern "C" fn spawn_source(
    _: *mut c_void,
    p: *const u8,
    n: usize,
    s: *const u8,
    sn: usize,
) -> bool {
    let (parent, source) = (text(p, n), text(s, sn));
    with(|u| {
        let Some(id) = source_id(source).filter(|_| u.exists(parent)) else {
            return false;
        };
        let root = format!("{parent}.{id}");
        u.log.push(format!(
            "S {root} len={} fnv={:016x}",
            source.len(),
            fnv(source)
        ));
        let first_line = source.lines().next().unwrap_or_default();
        let header = first_line.split('#').next().unwrap_or(first_line);
        if header.contains("visible: false") {
            u.visible.insert(root.clone(), false);
        }
        u.roots.push(root);
        true
    })
}
unsafe extern "C" fn set_properties(
    _: *mut c_void,
    p: *const u8,
    n: usize,
    s: *const u8,
    sn: usize,
) -> bool {
    let (path, source) = (text(p, n), text(s, sn));
    with(|u| {
        if !u.exists(path) || u.refuse_writes {
            return false;
        }
        u.log.push(format!("P {path} | {source}"));
        if let Some(v) = top_visible(source) {
            u.visible.insert(path.into(), v);
        }
        true
    })
}
unsafe extern "C" fn remove_node(_: *mut c_void, p: *const u8, n: usize) -> bool {
    let path = text(p, n).to_owned();
    with(|u| {
        if !u.exists(&path) {
            return false;
        }
        u.log.push(format!("X {path}"));
        let under = |q: &str| q == path || q.starts_with(&format!("{path}."));
        u.roots.retain(|r| !under(r));
        u.visible.retain(|q, _| !under(q));
        true
    })
}
#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn node_rect(
    _: *const c_void,
    p: *const u8,
    n: usize,
    _contents: bool,
    x: *mut f32,
    y: *mut f32,
    w: *mut f32,
    h: *mut f32,
) -> bool {
    let path = text(p, n);
    with(|u| {
        if !u.exists(path) {
            return false;
        }
        let r = u.rects.get(path).copied().unwrap_or((0., 0., 1920., 1080.));
        (*x, *y, *w, *h) = r;
        true
    })
}
unsafe extern "C" fn register_path_events(
    _: *mut c_void,
    p: *const u8,
    n: usize,
    _cb: Option<mod_api_stable::UiClickHandlerFn>,
    _userdata: *mut c_void,
) -> bool {
    let path = text(p, n);
    with(|u| {
        u.log.push(format!("R {path}"));
        true
    })
}

struct Tables {
    ui: UiVtableV1,
}
thread_local! {
    static TABLES: &'static Tables = Box::leak(Box::new(Tables { ui: ui_table() }));
}
fn ui_table() -> UiVtableV1 {
    // SAFETY: every field is a size or an optional function pointer, for
    // which all-zero bits are 0 and None.
    let mut t: UiVtableV1 = unsafe { std::mem::zeroed() };
    t.size = std::mem::size_of::<UiVtableV1>();
    t.query_exists = Some(query_exists);
    t.get_visible = Some(get_visible);
    t.set_visible = Some(set_visible);
    t.set_text = Some(set_text);
    t.spawn_source = Some(spawn_source);
    t.set_properties = Some(set_properties);
    t.remove_node = Some(remove_node);
    t.node_rect = Some(node_rect);
    t.register_path_events = Some(register_path_events);
    t
}
/// Run `f` with a client context answered by this thread's fake UI. No
/// other table (scene, data, assets, drawing) is present: those calls
/// answer "unavailable", as in a scene without them.
pub fn frame<R>(f: impl FnOnce(&mut StableClient<'_>) -> R) -> R {
    let tables = TABLES.with(|t| *t);
    // SAFETY: as above, all-zero is null pointers and a zero size.
    let mut raw: ClientCtxV1 = unsafe { std::mem::zeroed() };
    raw.size = std::mem::size_of::<ClientCtxV1>();
    raw.ui = &tables.ui;
    // SAFETY: `raw` lives for the whole call.
    let mut ctx = unsafe { StableClient::from_raw(&mut raw) }.expect("fake client context");
    f(&mut ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_fake_tree_records_writes_and_answers_reads() {
        reset();
        frame(|ctx| {
            assert!(!ctx.ui_exists("ingame.w"));
            assert!(ctx.ui_spawn_source("ingame", "w:color { visible: false; #a:label { } }"));
            assert_eq!(ctx.ui_visible("ingame.w"), Some(false));
            assert!(ctx.ui_set_properties("ingame.w.a", "visible: false; x: 1px;"));
            assert_eq!(ctx.ui_visible("ingame.w.a"), Some(false));
            assert!(ctx.ui_set_text("ingame.w.a", "hi"));
            assert!(ctx.ui_remove_node("ingame.w"));
            assert!(!ctx.ui_set_text("ingame.w.a", "hi"));
        });
        let log = take_log();
        assert_eq!(log.len(), 4);
        assert_eq!(log[1], "P ingame.w.a | visible: false; x: 1px;");
        assert_eq!(log[3], "X ingame.w");
    }
}
