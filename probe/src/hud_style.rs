//! UI lab typography and conservative wrapping using the packaged font advances.
use std::{collections::HashMap, sync::OnceLock};

pub fn fonts(source: String) -> String {
    source
        .replace(
            "@\"asset/base/style/main#bold_label\";",
            "font: \"asset/lt_direct_control_probe/font/numeric\";",
        )
        .replace(
            "@\"asset/base/style/main#label\";",
            "font: \"asset/lt_direct_control_probe/font/medium\";",
        )
}
fn advance(c: char, size: f32) -> f32 {
    static METRICS: OnceLock<HashMap<String, u16>> = OnceLock::new();
    let metrics = METRICS.get_or_init(|| {
        serde_json::from_str(include_str!("hud_font_metrics.json")).expect("packaged font advances")
    });
    f32::from(*metrics.get(&(c as u32).to_string()).unwrap_or(&1000)) * size / 1000.
}
pub fn width(text: &str, size: f32) -> f32 {
    crate::tooltips::plain(text)
        .chars()
        .map(|c| advance(c, size))
        .sum()
}
/// Break Latin at word boundaries and CJK between glyphs, retaining color tags.
/// A small margin accommodates native shaping/kerning differences.
pub fn wrap(text: &str, size: f32, available: f32) -> (String, usize) {
    let mut out = String::new();
    let mut line = 0.;
    let mut lines = 1;
    let mut token = String::new();
    let flush = |token: &mut String, out: &mut String, line: &mut f32, lines: &mut usize| {
        let w = width(token, size);
        if *line > 0. && *line + w > available - 8. {
            out.push('\n');
            *lines += 1;
            *line = 0.;
        }
        out.push_str(token);
        *line += w;
        token.clear();
    };
    let mut tag = false;
    for c in text.chars() {
        if c == '<' {
            tag = true;
            token.push(c);
            continue;
        }
        if tag {
            token.push(c);
            if c == '>' {
                tag = false;
            }
            continue;
        }
        if c == '\n' {
            flush(&mut token, &mut out, &mut line, &mut lines);
            out.push('\n');
            line = 0.;
            lines += 1;
        } else if c.is_whitespace() {
            flush(&mut token, &mut out, &mut line, &mut lines);
            if line > 0. {
                out.push(c);
                line += advance(c, size);
            }
        } else if (c as u32) >= 0x2e80 {
            flush(&mut token, &mut out, &mut line, &mut lines);
            token.push(c);
            flush(&mut token, &mut out, &mut line, &mut lines);
        } else {
            token.push(c);
            if width(&token, size) > available - 8. {
                flush(&mut token, &mut out, &mut line, &mut lines);
            }
        }
    }
    flush(&mut token, &mut out, &mut line, &mut lines);
    (out, lines)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrapping_preserves_color_runs_and_long_localized_content() {
        let source="Deals <#53b8e4ff>180<> physical damage and slows enemies.\n造成<#ff642eff>物理傷害<>，並使敵人緩速。";
        let (s, n) = wrap(source, 20., 180.);
        assert!(n > 2);
        assert!(s.contains("<#53b8e4ff>180<>") && s.contains("<#ff642eff>"));
        assert_eq!(
            crate::tooltips::plain(&s).replace(['\n', ' '], ""),
            crate::tooltips::plain(source).replace(['\n', ' '], "")
        );
        assert!(s.lines().all(|l| width(l, 20.) <= 180.));
    }
}
