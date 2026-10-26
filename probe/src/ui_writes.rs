//! The property and text cache every mod window writes through (1.6.0-perf).
//!
//! A window sends the game a node's property source (`visible: true; x:
//! 4px;`) or a label's text. Most frames it would send what the node
//! already holds, so the cache decides, per write, whether the game needs
//! it. The rules are the 1.5.3 ones (`hud_motion::write_properties`), kept
//! exactly:
//!
//! - The source is compared field by field (top-level `key: value;`) with
//!   what the node last received, after theming (`ui_theme::themed`). If
//!   every field already holds its value, nothing is sent; otherwise the
//!   whole themed source is sent, and on success its fields are stored.
//! - Two whole-text slots per node answer "same text as before" without
//!   theming or parsing: the text last written, and one found already
//!   applied since, each with the background it was themed under.
//! - A refused write stores nothing, so the next frame tries again.
//! - A text is sent when it differs from the last text the node accepted.
//!
//! What 1.6.0 changes is the storage, and one more shortcut:
//!
//! - Each node path has a slot, found once (`node`) and then addressed by
//!   its index (`NodeId`), instead of string keys built with `format!` on
//!   every call (`field:{path}:{key}`, `whole:{path}`, `text:{node}`).
//! - Typed writes (`props_keyed`, `text_keyed`): a write site passes the
//!   plain values its source is made from (numbers, flags) and a function
//!   that formats them. When the values, the node and the background and
//!   language are all as they were the last time this site's write was
//!   accepted, the source is not even formatted: the rules above would
//!   send nothing. Any accepted write to the node (from another site),
//!   `clear` or `forget` drops the shortcut, so the rules decide again.
use mod_api_stable::StableClient;
use std::collections::HashMap;

/// A node's slot in one window's cache. Valid for the cache's lifetime:
/// `clear` empties slots but keeps them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeId(u32);

/// The plain values a typed write site formats its source from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Key([u64; 4]);
/// One value of a [`Key`]: exact bits, so equal bits mean equal text.
pub trait Part: Copy {
    fn bits(self) -> u64;
}
impl Part for f32 {
    fn bits(self) -> u64 {
        u64::from(self.to_bits())
    }
}
impl Part for bool {
    fn bits(self) -> u64 {
        u64::from(self)
    }
}
impl Part for u32 {
    fn bits(self) -> u64 {
        u64::from(self)
    }
}
impl Part for u8 {
    fn bits(self) -> u64 {
        u64::from(self)
    }
}
impl Part for usize {
    fn bits(self) -> u64 {
        self as u64
    }
}
impl Part for u64 {
    fn bits(self) -> u64 {
        self
    }
}
impl Part for Option<usize> {
    fn bits(self) -> u64 {
        // None and every value apart (usize::MAX is a marker elsewhere too).
        self.map_or(u64::MAX, |v| v as u64)
    }
}
impl Part for isize {
    fn bits(self) -> u64 {
        self as u64
    }
}
impl Part for f64 {
    fn bits(self) -> u64 {
        self.to_bits()
    }
}
impl Part for Option<f64> {
    fn bits(self) -> u64 {
        // A NaN payload no format produces stands for None.
        self.map_or(0x7ff8_dead_beef_0001, f64::to_bits)
    }
}
impl Part for Option<f32> {
    fn bits(self) -> u64 {
        self.map_or(1 << 40, |v| u64::from(v.to_bits()))
    }
}
impl Part for () {
    fn bits(self) -> u64 {
        0
    }
}
macro_rules! key_from {
    ($($t:ident $i:tt),+) => {
        impl<$($t: Part),+> From<($($t,)+)> for Key {
            fn from(v: ($($t,)+)) -> Self {
                let mut k = [0; 4];
                $(k[$i] = v.$i.bits();)+
                Key(k)
            }
        }
    };
}
impl From<()> for Key {
    fn from(_: ()) -> Self {
        Key([0; 4])
    }
}
key_from!(A 0);
key_from!(A 0, B 1);
key_from!(A 0, B 1, C 2);
key_from!(A 0, B 1, C 2, D 3);

/// What a typed site last had accepted: its values, the node's write count
/// and the background/language then.
#[derive(Clone, Copy)]
struct Memo {
    site: u8,
    key: Key,
    version: u32,
    env: u64,
}
#[derive(Default)]
struct Slot {
    path: Box<str>,
    /// The themed field values the node last received, in first-seen order.
    fields: Vec<(Box<str>, Box<str>)>,
    /// (background, source) last written, and one found applied since.
    whole: Option<(u32, Box<str>)>,
    whole2: Option<(u32, Box<str>)>,
    text: Option<Box<str>>,
    /// Accepted property writes to this node (drops other sites' memos).
    version: u32,
    memos: Vec<Memo>,
    text_memo: Option<(Key, u64)>,
}
impl Slot {
    fn empty(&mut self) {
        self.fields.clear();
        self.whole = None;
        self.whole2 = None;
        self.text = None;
        self.memos.clear();
        self.text_memo = None;
        self.version = self.version.wrapping_add(1);
    }
    fn holds(slot: &Option<(u32, Box<str>)>, bg: u32, source: &str) -> bool {
        slot.as_ref()
            .is_some_and(|(b, s)| *b == bg && &**s == source)
    }
    fn whole_unchanged(&self, bg: u32, source: &str) -> bool {
        Self::holds(&self.whole, bg, source) || Self::holds(&self.whole2, bg, source)
    }
    fn field(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(k, _)| &**k == key)
            .map(|(_, v)| &**v)
    }
    fn fields_unchanged(&self, source: &str) -> bool {
        let mut all = true;
        for_each_field(source, |key, value| {
            all &= self.field(key) == Some(value);
        });
        all
    }
    fn store_fields(&mut self, source: &str) {
        let fields = &mut self.fields;
        for_each_field(source, |key, value| {
            match fields.iter_mut().find(|(k, _)| &**k == key) {
                Some((_, v)) => {
                    if &**v != value {
                        *v = value.into();
                    }
                }
                None => fields.push((key.into(), value.into())),
            }
        });
    }
    fn memo(&self, site: u8) -> Option<&Memo> {
        self.memos.iter().find(|m| m.site == site)
    }
    fn set_memo(&mut self, memo: Memo) {
        match self.memos.iter_mut().find(|m| m.site == memo.site) {
            Some(m) => *m = memo,
            None => self.memos.push(memo),
        }
    }
}
/// Calls `f(key, value)` for each top-level `key: value` of a property
/// source, as 1.5.3's `hud_motion::fields` split them (quotes and braces
/// respected, both sides trimmed, a part without `:` skipped).
pub fn for_each_field<'a>(source: &'a str, mut f: impl FnMut(&'a str, &'a str)) {
    let (mut start, mut depth, mut quoted, mut escape) = (0, 0, false, false);
    let mut part = |text: &'a str| {
        if let Some((key, value)) = text.split_once(':') {
            f(key.trim(), value.trim());
        }
    };
    for (i, c) in source.char_indices() {
        if quoted {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                quoted = false;
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            '{' => depth += 1,
            '}' => depth -= 1,
            ';' if depth == 0 => {
                part(&source[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    part(&source[start..]);
}
/// The background and mod language: what theming and translated texts
/// depend on, besides a site's own values.
pub fn environment() -> u64 {
    u64::from(crate::ui_theme::background()) | (crate::lang::generation() as u64) << 32
}

#[cfg(test)]
thread_local! {
    /// Tests: typed writes on this thread, and how many were formatted.
    pub static COUNTS: std::cell::Cell<(u64, u64)> = const { std::cell::Cell::new((0, 0)) };
}
/// One window's write cache. See the module notes.
#[derive(Default)]
pub struct PropCache {
    index: HashMap<Box<str>, u32>,
    slots: Vec<Slot>,
}
impl PropCache {
    /// The slot for `path`, made on first use. Look it up once (at spawn or
    /// in a node table), not on every write.
    pub fn node(&mut self, path: &str) -> NodeId {
        if let Some(&i) = self.index.get(path) {
            return NodeId(i);
        }
        let i = self.slots.len() as u32;
        self.slots.push(Slot {
            path: path.into(),
            ..Slot::default()
        });
        self.index.insert(path.into(), i);
        NodeId(i)
    }
    pub fn path(&self, id: NodeId) -> &str {
        &self.slots[id.0 as usize].path
    }
    /// A count that changes whenever the node is written (or forgotten).
    pub fn version(&self, id: NodeId) -> u32 {
        self.slots[id.0 as usize].version
    }
    /// Forget everything sent (the window was rebuilt). Node ids stay valid.
    pub fn clear(&mut self) {
        self.slots.iter_mut().for_each(Slot::empty);
    }
    /// Forget what was sent to every node whose path contains `part`.
    pub fn forget(&mut self, part: &str) {
        self.slots
            .iter_mut()
            .filter(|s| s.path.contains(part))
            .for_each(Slot::empty);
    }
    /// 1.5.3's `hud_motion::unchanged`: whether `source` (not themed) would
    /// be sent to `path`.
    pub fn unchanged(&self, path: &str, source: &str) -> bool {
        match self.index.get(path) {
            Some(&i) => {
                let s = &self.slots[i as usize];
                s.whole_unchanged(crate::ui_theme::background(), source)
                    || s.fields_unchanged(source)
            }
            // Nothing sent yet: only a source without fields is "applied".
            None => Slot::default().fields_unchanged(source),
        }
    }
    /// The 1.5.3 rules for one property write; true once the node holds
    /// `source` (written now or before).
    pub fn props(&mut self, ctx: &mut StableClient<'_>, id: NodeId, source: &str) -> bool {
        let slot = &mut self.slots[id.0 as usize];
        write(slot, source, |path, value| {
            ctx.ui_set_properties(path, value)
        })
    }
    /// Tests: the value `path` last received for one field.
    #[cfg(test)]
    pub fn field(&self, path: &str, key: &str) -> Option<&str> {
        self.index
            .get(path)
            .and_then(|&i| self.slots[i as usize].field(key))
    }
    /// [`props`](Self::props) by path (windows without a node table).
    pub fn props_at(&mut self, ctx: &mut StableClient<'_>, path: &str, source: &str) -> bool {
        let id = self.node(path);
        self.props(ctx, id, source)
    }
    /// A typed property write: `render(key)` is the source, formatted only
    /// when this site's values (or the node, background or language)
    /// changed since its last accepted write. `site` tells apart several
    /// sites writing one node.
    pub fn props_keyed<K: Into<Key> + Copy>(
        &mut self,
        ctx: &mut StableClient<'_>,
        id: NodeId,
        site: u8,
        values: K,
        render: impl FnOnce(K) -> String,
    ) -> bool {
        self.props_keyed_with(id, site, values, render, |path, value| {
            ctx.ui_set_properties(path, value)
        })
    }
    fn props_keyed_with<K: Into<Key> + Copy>(
        &mut self,
        id: NodeId,
        site: u8,
        values: K,
        render: impl FnOnce(K) -> String,
        send: impl FnMut(&str, &str) -> bool,
    ) -> bool {
        let key = values.into();
        let env = environment();
        #[cfg(test)]
        COUNTS.with(|c| c.set((c.get().0 + 1, c.get().1)));
        let slot = &mut self.slots[id.0 as usize];
        if slot
            .memo(site)
            .is_some_and(|m| m.key == key && m.version == slot.version && m.env == env)
        {
            return true;
        }
        #[cfg(test)]
        COUNTS.with(|c| c.set((c.get().0, c.get().1 + 1)));
        let ok = write(slot, &render(values), send);
        if ok {
            let version = slot.version;
            slot.set_memo(Memo {
                site,
                key,
                version,
                env,
            });
        } else {
            slot.memos.retain(|m| m.site != site);
        }
        ok
    }
    /// A label's text: sent when it differs from the last text accepted.
    pub fn text(&mut self, ctx: &mut StableClient<'_>, id: NodeId, text: &str) -> bool {
        let slot = &mut self.slots[id.0 as usize];
        text_write(slot, text, |path, value| ctx.ui_set_text(path, value))
    }
    /// [`text`](Self::text) by path.
    pub fn text_at(&mut self, ctx: &mut StableClient<'_>, path: &str, text: &str) -> bool {
        let id = self.node(path);
        self.text(ctx, id, text)
    }
    /// A typed text: `render(key)` formatted only when the values (or the
    /// language) changed since the last accepted text.
    #[allow(dead_code)] // The documented way to write a tree label (docs/modding.md); no window has one since 1.6.1-dev1.
    pub fn text_keyed<K: Into<Key> + Copy>(
        &mut self,
        ctx: &mut StableClient<'_>,
        id: NodeId,
        values: K,
        render: impl FnOnce(K) -> String,
    ) -> bool {
        self.text_keyed_with(id, values, render, |path, value| {
            ctx.ui_set_text(path, value)
        })
    }
    fn text_keyed_with<K: Into<Key> + Copy>(
        &mut self,
        id: NodeId,
        values: K,
        render: impl FnOnce(K) -> String,
        send: impl FnMut(&str, &str) -> bool,
    ) -> bool {
        let key = values.into();
        let env = environment();
        #[cfg(test)]
        COUNTS.with(|c| c.set((c.get().0 + 1, c.get().1)));
        let slot = &mut self.slots[id.0 as usize];
        if slot.text_memo == Some((key, env)) && slot.text.is_some() {
            return true;
        }
        #[cfg(test)]
        COUNTS.with(|c| c.set((c.get().0, c.get().1 + 1)));
        let ok = text_write(slot, &render(values), send);
        slot.text_memo = ok.then_some((key, env));
        ok
    }
}
fn write(slot: &mut Slot, source: &str, mut send: impl FnMut(&str, &str) -> bool) -> bool {
    let bg = crate::ui_theme::background();
    // The same text as before needs neither theming nor parsing.
    if slot.whole_unchanged(bg, source) {
        return true;
    }
    // Design greys (`#~rrggbbaa`) follow the background colour.
    let themed = crate::ui_theme::themed(source);
    // Compare top-level fields: a geometry change must not resend the
    // unchanged visibility or source setters.
    if slot.fields_unchanged(&themed) {
        slot.whole2 = Some((bg, source.into()));
        return true;
    }
    if !send(&slot.path, &themed) {
        return false;
    }
    slot.store_fields(&themed);
    slot.whole = Some((bg, source.into()));
    slot.whole2 = None;
    slot.version = slot.version.wrapping_add(1);
    true
}
fn text_write(slot: &mut Slot, text: &str, mut send: impl FnMut(&str, &str) -> bool) -> bool {
    if slot.text.as_deref() == Some(text) {
        return true;
    }
    // 1.5.3 kept the last accepted text on a refusal: the same text later
    // is still "unchanged" and a different one is tried again.
    if !send(&slot.path, text) {
        return false;
    }
    slot.text = Some(text.into());
    // A typed site's values no longer describe the node's text.
    slot.text_memo = None;
    true
}

#[cfg(test)]
pub(crate) mod legacy {
    //! 1.5.3's string-keyed cache (`hud_motion::write_properties` and the
    //! windows' `text` helpers), kept only to prove the new one equal.
    use std::collections::HashMap;
    fn fields(source: &str) -> Vec<(&str, &str)> {
        let mut result = Vec::new();
        super::for_each_field(source, |k, v| result.push((k, v)));
        result
    }
    fn slot_holds(cache: &HashMap<String, String>, tag: &str, path: &str, source: &str) -> bool {
        cache.get(&format!("{tag}{path}")).is_some_and(|stored| {
            stored.len() == 6 + source.len()
                && stored.get(6..) == Some(source)
                && stored
                    .get(..6)
                    .and_then(|bg| u32::from_str_radix(bg, 16).ok())
                    == Some(crate::ui_theme::background())
        })
    }
    fn slot_value(source: &str) -> String {
        format!("{:06x}{source}", crate::ui_theme::background())
    }
    fn whole_unchanged(cache: &HashMap<String, String>, path: &str, source: &str) -> bool {
        slot_holds(cache, "whole:", path, source) || slot_holds(cache, "whole2:", path, source)
    }
    fn fields_unchanged(cache: &HashMap<String, String>, path: &str, source: &str) -> bool {
        fields(source).iter().all(|(key, value)| {
            cache
                .get(&format!("field:{path}:{key}"))
                .is_some_and(|old| old == value)
        })
    }
    pub fn unchanged(cache: &HashMap<String, String>, path: &str, source: &str) -> bool {
        whole_unchanged(cache, path, source) || fields_unchanged(cache, path, source)
    }
    pub fn write_properties(
        cache: &mut HashMap<String, String>,
        path: &str,
        source: &str,
        mut write: impl FnMut(&str) -> bool,
    ) -> bool {
        if whole_unchanged(cache, path, source) {
            return true;
        }
        let themed = crate::ui_theme::themed(source);
        if fields_unchanged(cache, path, &themed) {
            cache.insert(format!("whole2:{path}"), slot_value(source));
            return true;
        }
        if !write(&themed) {
            return false;
        }
        for (key, value) in fields(&themed) {
            cache.insert(format!("field:{path}:{key}"), value.into());
        }
        cache.insert(format!("whole:{path}"), slot_value(source));
        cache.remove(&format!("whole2:{path}"));
        true
    }
    pub fn text(
        cache: &mut HashMap<String, String>,
        path: &str,
        text: &str,
        write: impl FnOnce(&str) -> bool,
    ) -> bool {
        let key = format!("text:{path}");
        if cache.get(&key).map(String::as_str) == Some(text) {
            return true;
        }
        if write(text) {
            cache.insert(key, text.into());
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A small deterministic random sequence (xorshift).
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn pick<'a>(&mut self, from: &[&'a str]) -> &'a str {
            from[(self.next() % from.len() as u64) as usize]
        }
    }
    const PATHS: [&str; 3] = ["w.a", "w.b", "w.a.c"];
    const SOURCES: [&str; 12] = [
        "visible: true;",
        "visible: false;",
        "x: 1px;",
        "x: 2px;",
        "visible: true; x: 1px;",
        "visible: true; x: 2px; color: #~3a3837ff;",
        "color: #~3a3837ff;",
        "color: #3a3837ff;",
        "source: \"a;b\"; btn: { color: #fff; stroke: 1; }",
        "btn: { color: #fff; stroke: 2; }",
        "x: 1px; x: 2px;",
        "",
    ];
    const TEXTS: [&str; 4] = ["", "1", "12", "—"];
    /// The Settings background changing (this thread only).
    fn set_background(rng: &mut Rng) {
        let bg = [None, Some(0x151720), Some(0x000000)][(rng.next() % 3) as usize];
        crate::ui_theme::TEST_BACKGROUND.with(|b| b.set(bg));
    }

    /// Random writes, refusals, texts, clears and forgets through 1.5.3's
    /// cache and the new one give the same sends and the same answers.
    #[test]
    fn the_indexed_cache_sends_exactly_what_the_1_5_3_cache_sent() {
        for seed in 1..40u64 {
            let mut rng = Rng(0x9e3779b97f4a7c15 ^ seed);
            let mut old = HashMap::new();
            let mut new = PropCache::default();
            for step in 0..3000 {
                let path = rng.pick(&PATHS);
                let accept = !rng.next().is_multiple_of(7);
                let (mut sent_old, mut sent_new) = (Vec::new(), Vec::new());
                match rng.next() % 20 {
                    0 => {
                        old.clear();
                        new.clear();
                    }
                    1 => {
                        old.retain(|k: &String, _| !k.contains("w.a"));
                        new.forget("w.a");
                    }
                    7 => set_background(&mut rng),
                    2..=4 => {
                        let text = rng.pick(&TEXTS);
                        let a = legacy::text(&mut old, path, text, |t| {
                            sent_old.push(t.to_owned());
                            accept
                        });
                        let id = new.node(path);
                        let b = text_write(&mut new.slots[id.0 as usize], text, |_, t| {
                            sent_new.push(t.to_owned());
                            accept
                        });
                        assert_eq!(a, b, "seed {seed} step {step}");
                    }
                    5..=6 => {
                        let source = rng.pick(&SOURCES);
                        assert_eq!(
                            legacy::unchanged(&old, path, source),
                            new.unchanged(path, source),
                            "seed {seed} step {step} unchanged {path} {source:?}"
                        );
                    }
                    _ => {
                        let source = rng.pick(&SOURCES);
                        let a = legacy::write_properties(&mut old, path, source, |v| {
                            sent_old.push(v.to_owned());
                            accept
                        });
                        let id = new.node(path);
                        let b = write(&mut new.slots[id.0 as usize], source, |_, v| {
                            sent_new.push(v.to_owned());
                            accept
                        });
                        assert_eq!(a, b, "seed {seed} step {step}");
                    }
                }
                assert_eq!(sent_old, sent_new, "seed {seed} step {step} {path}");
            }
        }
        crate::ui_theme::TEST_BACKGROUND.with(|b| b.set(None));
    }
    /// Typed writes send what plain writes of the rendered source send,
    /// including when another site overwrites a shared field.
    #[test]
    fn typed_writes_send_what_plain_writes_send() {
        for seed in 1..40u64 {
            let mut rng = Rng(0x2545f4914f6cdd1d ^ seed);
            let mut plain = PropCache::default();
            let mut typed = PropCache::default();
            for step in 0..3000 {
                let path = rng.pick(&PATHS);
                let accept = !rng.next().is_multiple_of(9);
                let (mut a_sent, mut b_sent) = (Vec::new(), Vec::new());
                // Site 0 writes visibility and x; site 1 writes x and colour.
                let site = (rng.next() % 2) as u8;
                let (v, x) = (rng.next().is_multiple_of(2), (rng.next() % 3) as usize);
                let render = move |(v, x): (bool, usize)| {
                    if site == 0 {
                        format!("visible: {v}; x: {x}px;")
                    } else {
                        format!("x: {x}px; color: #~3a38{x:02x}ff;")
                    }
                };
                if rng.next().is_multiple_of(50) {
                    plain.clear();
                    typed.clear();
                }
                if rng.next().is_multiple_of(40) {
                    set_background(&mut rng);
                }
                let id = plain.node(path);
                let a = write(&mut plain.slots[id.0 as usize], &render((v, x)), |_, s| {
                    a_sent.push(s.to_owned());
                    accept
                });
                let id = typed.node(path);
                let b = typed.props_keyed_with(id, site, (v, x), render, |_, s| {
                    b_sent.push(s.to_owned());
                    accept
                });
                assert_eq!((a, &a_sent), (b, &b_sent), "seed {seed} step {step}");
                let n = (rng.next() % 3) as usize;
                let (mut a_sent, mut b_sent) = (Vec::new(), Vec::new());
                let a = text_write(&mut plain.slots[id.0 as usize], &n.to_string(), |_, s| {
                    a_sent.push(s.to_owned());
                    accept
                });
                let b = typed.text_keyed_with(
                    id,
                    (n,),
                    |(n,)| n.to_string(),
                    |_, s| {
                        b_sent.push(s.to_owned());
                        accept
                    },
                );
                assert_eq!((a, &a_sent), (b, &b_sent), "text seed {seed} step {step}");
            }
        }
        crate::ui_theme::TEST_BACKGROUND.with(|b| b.set(None));
    }
    fn put(
        cache: &mut PropCache,
        path: &str,
        source: &str,
        send: impl FnMut(&str) -> bool,
    ) -> bool {
        let id = cache.node(path);
        let mut send = send;
        write(&mut cache.slots[id.0 as usize], source, |_, v| send(v))
    }
    // The four 1.5.3 cache tests of hud_motion.rs, on the new storage.
    #[test]
    fn repeated_panel_close_reopen_updates_host_and_cache_together() {
        let mut cache = PropCache::default();
        let mut host_visible = false;
        let mut writes = 0;
        for active in [true, true, false, false, true, false, true] {
            let source = format!("visible: {active};");
            assert!(put(&mut cache, "panel", &source, |value| {
                host_visible = value.contains("visible: true;");
                writes += 1;
                true
            }));
            assert_eq!(host_visible, active);
        }
        assert_eq!(writes, 5);
        assert!(!put(&mut cache, "panel", "visible: false;", |_| false));
        assert!(host_visible);
        assert!(!cache.unchanged("panel", "visible: false;"));
    }
    #[test]
    fn equal_text_writes_nothing_and_a_different_text_once() {
        let mut cache = PropCache::default();
        let mut writes = 0;
        let source = "visible: true; x: 4px;";
        for _ in 0..2 {
            assert!(put(&mut cache, "p", source, |_| {
                writes += 1;
                true
            }));
        }
        assert!(cache.unchanged("p", source));
        assert_eq!(writes, 1);
        for _ in 0..3 {
            assert!(put(&mut cache, "p", "visible: false;", |_| {
                writes += 1;
                true
            }));
        }
        assert_eq!(writes, 2);
    }
    #[test]
    fn alternating_texts_hit_and_geometry_does_not_resend_visibility() {
        let mut cache = PropCache::default();
        let mut sent = Vec::new();
        for x in [1, 1, 1, 2, 2, 2] {
            for source in ["visible: true;".to_owned(), format!("x: {x}px;")] {
                assert!(put(&mut cache, "p", &source, |value| {
                    sent.push(value.to_owned());
                    true
                }));
            }
        }
        assert_eq!(sent, ["visible: true;", "x: 1px;", "x: 2px;"]);
        let mut sent = Vec::new();
        for source in ["visible: true; x: 2px;", "visible: true; x: 3px;"] {
            put(&mut cache, "p", source, |value| {
                sent.push(value.to_owned());
                true
            });
        }
        assert_eq!(sent, ["visible: true; x: 3px;"]);
    }
    #[test]
    fn failed_write_retries_and_another_background_misses_the_slot() {
        let mut cache = PropCache::default();
        let mut writes = 0;
        for ok in [false, false, true, true] {
            put(&mut cache, "p", "visible: true;", |_| {
                writes += 1;
                ok
            });
        }
        assert_eq!(writes, 3);
        // A slot written under another background is no answer: the field
        // check decides (and here finds the text applied).
        let id = cache.node("p");
        let slot = &mut cache.slots[id.0 as usize];
        slot.whole = Some((0, "visible: true;".into()));
        assert!(!slot.whole_unchanged(crate::ui_theme::background(), "visible: true;"));
        assert!(slot.fields_unchanged("visible: true;"));
        slot.whole = Some((crate::ui_theme::background(), "visible: true;".into()));
        assert!(slot.whole_unchanged(crate::ui_theme::background(), "visible: true;"));
        assert!(!slot.whole_unchanged(crate::ui_theme::background(), "visible: true; "));
    }
    #[test]
    fn a_site_skips_formatting_only_while_nothing_changed() {
        use std::cell::Cell;
        let mut cache = PropCache::default();
        let id = cache.node("w.a");
        let (rendered, sent) = (Cell::new(0), Cell::new(0));
        let go = |cache: &mut PropCache, x: usize| {
            cache.props_keyed_with(
                id,
                0,
                (x,),
                |(x,)| {
                    rendered.set(rendered.get() + 1);
                    format!("x: {x}px;")
                },
                |_, _| {
                    sent.set(sent.get() + 1);
                    true
                },
            )
        };
        go(&mut cache, 1);
        go(&mut cache, 1);
        go(&mut cache, 1);
        assert_eq!(rendered.get(), 1);
        go(&mut cache, 2);
        assert_eq!(rendered.get(), 2);
        cache.clear();
        go(&mut cache, 2);
        assert_eq!(rendered.get(), 3);
        assert_eq!(sent.get(), 3);
    }
}
