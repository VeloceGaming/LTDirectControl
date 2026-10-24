//! Node geometry for the fake UI tree (test builds only, 1.6.0-perf-dev3).
//!
//! `fake_ui.rs` answers `ui_node_rect` from this: a spawned template is
//! parsed into its nodes, property writes update their `x`, `y`, `width`,
//! `height`, `anchor_*` and `pivot_*`, and a node's rectangle is laid out
//! from its parent's as the game's tree does it (a guess at the game's
//! rules, the ones the mod's own hit tests assume): position = parent's
//! corner + anchor × parent size + offset − pivot × own size; sizes in px or
//! % of the parent; a missing value is 0. The game's own root `ingame` is
//! the 1920 x 1080 UI map.
use std::collections::HashMap;

/// One node's layout fields (unset ones are `None`).
#[derive(Clone, Debug, Default)]
pub struct Layout {
    x: Option<Len>,
    y: Option<Len>,
    width: Option<Len>,
    height: Option<Len>,
    anchor: (f32, f32),
    pivot: (f32, f32),
}
#[derive(Clone, Copy, Debug)]
enum Len {
    Px(f32),
    Percent(f32),
}
impl Len {
    fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if let Some(px) = value.strip_suffix("px") {
            px.trim().parse().ok().map(Len::Px)
        } else if let Some(pc) = value.strip_suffix('%') {
            pc.trim().parse().ok().map(Len::Percent)
        } else {
            value.parse().ok().map(Len::Px)
        }
    }
    fn of(self, parent: f32) -> f32 {
        match self {
            Len::Px(v) => v,
            Len::Percent(p) => parent * p / 100.,
        }
    }
}
impl Layout {
    /// Apply the top-level fields of a property source.
    pub fn apply(&mut self, source: &str) {
        crate::ui_writes::for_each_field(source, |key, value| {
            let number = || value.trim().parse::<f32>().ok();
            match key {
                "x" => self.x = Len::parse(value),
                "y" => self.y = Len::parse(value),
                "width" => self.width = Len::parse(value),
                "height" => self.height = Len::parse(value),
                "anchor_x" => self.anchor.0 = number().unwrap_or(0.),
                "anchor_y" => self.anchor.1 = number().unwrap_or(0.),
                "pivot_x" => self.pivot.0 = number().unwrap_or(0.),
                "pivot_y" => self.pivot.1 = number().unwrap_or(0.),
                _ => {}
            }
        });
    }
    /// This node's rectangle inside `parent`.
    pub fn place(&self, (px, py, pw, ph): (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
        let w = self.width.map_or(0., |l| l.of(pw));
        let h = self.height.map_or(0., |l| l.of(ph));
        let x = self.x.map_or(0., |l| l.of(pw));
        let y = self.y.map_or(0., |l| l.of(ph));
        (
            px + pw * self.anchor.0 + x - w * self.pivot.0,
            py + ph * self.anchor.1 + y - h * self.pivot.1,
            w,
            h,
        )
    }
}

/// The index of the brace closing the one at `open` (quotes respected).
fn closing(s: &str, open: usize) -> Option<usize> {
    let (mut depth, mut quoted, mut escape) = (0, false, false);
    for (i, c) in s[open..].char_indices() {
        if quoted {
            match c {
                _ if escape => escape = false,
                '\\' => escape = true,
                '"' => quoted = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + i);
                }
            }
            _ => {}
        }
    }
    None
}
/// A child block's start at `at` (`#id:runner {`): (id, index of `{`).
fn child_at(s: &str, at: usize) -> Option<(&str, usize)> {
    let rest = s[at..].strip_prefix('#')?;
    let colon = rest.find(':')?;
    let id = &rest[..colon];
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let after = &rest[colon + 1..];
    let runner = after.len()
        - after
            .trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_')
            .len();
    if runner == 0 {
        return None;
    }
    let tail = &after[runner..];
    let brace = tail.len() - tail.trim_start().len();
    tail[brace..]
        .starts_with('{')
        .then_some((id, at + 1 + colon + 1 + runner + brace))
}
/// Every node of a template body as (path relative to the body's node,
/// own field text), children after their parent.
fn nodes(body: &str, prefix: &str, out: &mut Vec<(String, String)>) {
    let mut own = String::new();
    let mut children = Vec::new();
    let (mut i, mut quoted) = (0, false);
    let bytes = body.as_bytes();
    while i < body.len() {
        let c = bytes[i] as char;
        if quoted {
            own.push(c);
            if c == '"' && (i == 0 || bytes[i - 1] != b'\\') {
                quoted = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            quoted = true;
        } else if c == '#' {
            if let Some((id, open)) = child_at(body, i) {
                if let Some(close) = closing(body, open) {
                    children.push((id.to_owned(), &body[open + 1..close]));
                    i = close + 1;
                    continue;
                }
            }
        } else if c == '{' {
            // A braced value (`rounding: Uniform { ... }`, `btn: { ... }`).
            if let Some(close) = closing(body, i) {
                own.push_str(&body[i..=close]);
                i = close + 1;
                continue;
            }
        }
        // Multi-byte characters: copy the whole character.
        let ch = body[i..].chars().next().unwrap_or(' ');
        own.push(ch);
        i += ch.len_utf8();
    }
    out.push((prefix.to_owned(), own));
    for (id, child) in children {
        let path = if prefix.is_empty() {
            id
        } else {
            format!("{prefix}.{id}")
        };
        nodes(child, &path, out);
    }
}
/// A spawned template's nodes: (path under `parent`, layout).
pub fn parse(parent: &str, source: &str) -> Vec<(String, Layout)> {
    let Some(colon) = source.find(':') else {
        return Vec::new();
    };
    let root = source[..colon].trim();
    let Some(open) = source.find('{') else {
        return Vec::new();
    };
    let Some(close) = closing(source, open) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    nodes(&source[open + 1..close], "", &mut found);
    found
        .into_iter()
        .map(|(relative, own)| {
            let path = if relative.is_empty() {
                format!("{parent}.{root}")
            } else {
                format!("{parent}.{root}.{relative}")
            };
            let mut layout = Layout::default();
            layout.apply(&own);
            (path, layout)
        })
        .collect()
}
/// The rectangle of `path` from the layouts (`ingame` and anything not in
/// them: the whole UI map), with test overrides first.
pub fn rect(
    path: &str,
    layouts: &HashMap<String, Layout>,
    overrides: &HashMap<String, (f32, f32, f32, f32)>,
) -> (f32, f32, f32, f32) {
    if let Some(r) = overrides.get(path) {
        return *r;
    }
    let screen = (0., 0., 1920., 1080.);
    match (layouts.get(path), path.rfind('.')) {
        (Some(layout), Some(dot)) => layout.place(rect(&path[..dot], layouts, overrides)),
        _ => screen,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn templates_lay_out_like_the_tree() {
        let t = "w:empty { width: 200px; height: 100px; anchor_x: 0.5; pivot_x: 0.5; anchor_y: 1; pivot_y: 1; rounding: Uniform { rounding: 2; } #a:color { x: 10px; y: 5px; width: 50%; height: 20px; #b:label { width: 100%; height: 100%; text: \"#x:y {\"; } } #c:image { x: -4px; } }";
        let layouts: HashMap<_, _> = parse("ingame", t).into_iter().collect();
        assert_eq!(layouts.len(), 4);
        let none = HashMap::new();
        assert_eq!(rect("ingame.w", &layouts, &none), (860., 980., 200., 100.));
        assert_eq!(rect("ingame.w.a", &layouts, &none), (870., 985., 100., 20.));
        assert_eq!(
            rect("ingame.w.a.b", &layouts, &none),
            (870., 985., 100., 20.)
        );
        assert_eq!(rect("ingame.w.c", &layouts, &none), (856., 980., 0., 0.));
        let mut moved = layouts.clone();
        moved
            .get_mut("ingame.w.a")
            .unwrap()
            .apply("x: 0px; width: 10px;");
        assert_eq!(rect("ingame.w.a", &moved, &none), (860., 985., 10., 20.));
    }
}
