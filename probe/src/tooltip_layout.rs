//! Owned tooltip layout and whole-line scrolling. No native pointers retained.
use crate::hud_style;

pub const LINE: usize = 28;
pub const TOP: f32 = 66.;

#[derive(Default)]
pub struct Layout {
    source: Option<(String, String, bool, usize)>,
    pub width: usize,
    pub height: usize,
    pub title: String,
    pub title_lines: usize,
    pub extra: usize,
    pub first: usize,
    pub page: usize,
    lines: Vec<String>,
}
impl Layout {
    pub fn prepare(&mut self, title: &str, body: &str, art: bool, available: usize) {
        if self
            .source
            .as_ref()
            .is_some_and(|(t, b, a, h)| t == title && b == body && *a == art && *h == available)
        {
            return;
        }
        self.source = Some((title.into(), body.into(), art, available));
        self.first = 0;
        let left = if art { 76. } else { 16. };
        let mut wrapped = String::new();
        // Preserve the usual narrow card; widen only when it would run above
        // the battlefield. Both sizes and the final height are viewport-bound.
        for width in [529, 640, 760] {
            self.width = width;
            (self.title, self.title_lines) = hud_style::wrap(title, 27., width as f32 - left - 34.);
            self.extra = self.title_lines.saturating_sub(1) * 32;
            let (text, count) = hud_style::wrap(body, 20., width as f32 - 36.);
            wrapped = text;
            self.height = (count * LINE + 122 + self.extra).max(167);
            if self.height <= available {
                break;
            }
        }
        self.lines = colored_lines(&wrapped);
        self.page = (available.saturating_sub(122 + self.extra) / LINE)
            .max(1)
            .min(self.lines.len());
        self.height = (self.page * LINE + 122 + self.extra)
            .max(167)
            .min(available);
    }
    pub fn max(&self) -> usize {
        self.lines.len().saturating_sub(self.page)
    }
    pub fn scroll(&mut self, notches: i32) {
        let steps = (notches.unsigned_abs() as usize).saturating_mul(3);
        self.first = if notches > 0 {
            self.first.saturating_sub(steps)
        } else {
            self.first.saturating_add(steps).min(self.max())
        };
    }
    pub fn reset_scroll(&mut self) {
        self.first = 0;
    }
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .skip(self.first)
            .take(self.page)
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Each visible line starts with its inherited color, so scrolling into a
/// multi-line stat color does not lose the native formatter's emphasis.
fn colored_lines(text: &str) -> Vec<String> {
    let mut color = String::new();
    text.split('\n')
        .map(|line| {
            let mut result = format!("{color}{line}");
            let mut rest = line;
            while let Some(start) = rest.find('<') {
                rest = &rest[start..];
                let Some(end) = rest.find('>') else { break };
                let tag = &rest[..=end];
                if tag == "<>" {
                    color.clear();
                } else if tag.starts_with("<#") {
                    color = tag.into();
                }
                rest = &rest[end + 1..];
            }
            if !color.is_empty() {
                result.push_str("<>");
            }
            result
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grows_then_widens_then_scrolls_without_losing_the_last_line() {
        let mut layout = Layout::default();
        layout.prepare("Skill", "Short description.", true, 860);
        assert_eq!(layout.width, 529);
        let short = layout.height;
        layout.prepare("Skill", &"Damage and slow. ".repeat(30), true, 860);
        assert!(layout.height > short);
        assert_eq!(layout.width, 529);
        layout.prepare("Skill", &"Damage and slow. ".repeat(130), true, 860);
        assert!(layout.width > 529);
        let body = format!(
            "{}\nFINAL LINE",
            "<i#asset/base/icon:attack> 傷害 damage ".repeat(600)
        );
        layout.prepare("Skill", &body, true, 860);
        assert_eq!(layout.width, 760);
        assert!(layout.height <= 860 && layout.max() > 0);
        layout.scroll(i32::MIN);
        assert!(layout.text().ends_with("FINAL LINE"));
        layout.scroll(i32::MAX);
        assert_eq!(layout.first, 0);
    }
    #[test]
    fn scroll_preserves_colors_and_resets_when_native_text_arrives() {
        let mut layout = Layout::default();
        layout.prepare(
            "技能",
            &format!("<#ff642eff>{}END<>", "傷害\n".repeat(50)),
            true,
            300,
        );
        layout.scroll(i32::MIN);
        assert!(layout.text().starts_with("<#ff642eff>"));
        assert!(layout.text().ends_with("END<>"));
        let first = layout.first;
        let same = layout.source.clone().unwrap();
        layout.prepare(&same.0, &same.1, same.2, same.3);
        assert_eq!(layout.first, first);
        layout.prepare("技能", "Native result arrived.", true, 300);
        assert_eq!(layout.first, 0);
        assert_eq!(layout.max(), 0);
    }
    #[test]
    fn inline_icons_use_real_space_when_wrapping() {
        let icon = "<i#asset/base/icon:attack>";
        assert!(hud_style::width(icon, 20.) >= 20.);
        let source = format!("<#ff642eff>{}END<>", icon.repeat(30));
        let (wrapped, count) = hud_style::wrap(&source, 20., 120.);
        assert!(count > 1);
        assert!(wrapped
            .lines()
            .all(|line| hud_style::width(line, 20.) <= 120.));
        assert_eq!(wrapped.replace('\n', ""), source);
    }
}
