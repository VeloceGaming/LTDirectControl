//! The UI background colour (Settings › Interface › Background colour). It
//! replaces the design's `#1c1a18` on every mod surface; dark text on bright
//! buttons keeps its fixed ink so it stays readable.
use mod_api_stable::StableClient;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Mutex,
};

/// The design's background, `#1c1a18`.
pub const DEFAULT: u32 = 0x1c_1a18;
static BACKGROUND: AtomicU32 = AtomicU32::new(DEFAULT);
/// Windows and the background (and mod language) they were built with.
static BUILT: Mutex<Vec<(&'static str, u64)>> = Mutex::new(Vec::new());

/// Once per frame on the client thread: pick up an applied change.
pub fn sync() {
    let v = crate::settings::option("background_color");
    BACKGROUND.store(v as u32 & 0xff_ffff, Ordering::Relaxed);
}
/// The background as 0xRRGGBB.
pub fn background() -> u32 {
    BACKGROUND.load(Ordering::Relaxed)
}
/// The background with `alpha`, as 0xRRGGBBAA.
pub fn rgba(alpha: u8) -> u32 {
    background() << 8 | u32::from(alpha)
}
/// The background with `alpha`, as "rrggbbaa" for UI templates.
pub fn hex(alpha: u8) -> String {
    format!("{:08x}", rgba(alpha))
}
/// A surface derived from the background: `delta` added to each of red,
/// green and blue (clamped), as "rrggbbaa". Keeps a design shade's relation
/// to the background, e.g. the shop strips `#22201e` = background + 6.
pub fn shade(delta: i32, alpha: u8) -> String {
    format!("{:08x}", shade_rgba(delta, alpha))
}
/// [`shade`] as 0xRRGGBBAA.
pub fn shade_rgba(delta: i32, alpha: u8) -> u32 {
    shaded(background(), delta) << 8 | u32::from(alpha)
}
/// A design grey (0xRRGGBBAA) moved with the background: each of red, green
/// and blue shifts by the background's distance from `#1c1a18` (clamped),
/// alpha kept. The default background returns the design colour unchanged.
pub fn tone(design: u32) -> u32 {
    toned(design, background())
}
/// "rrggbbaa" as 0xRRGGBBAA; a leading `~` marks a design grey (see
/// [`themed`]) and returns it toned.
pub fn color(text: &str) -> Option<u32> {
    match text.strip_prefix('~') {
        Some(design) => u32::from_str_radix(design, 16).ok().map(tone),
        None => u32::from_str_radix(text, 16).ok(),
    }
}
fn toned(design: u32, bg: u32) -> u32 {
    [24, 16, 8].into_iter().fold(design & 0xff, |out, shift| {
        let offset = (bg >> (shift - 8) & 0xff) as i32 - (DEFAULT >> (shift - 8) & 0xff) as i32;
        let channel = ((design >> shift & 0xff) as i32 + offset).clamp(0, 255) as u32;
        out | channel << shift
    })
}
/// Template and property text with each `#~rrggbbaa` written through
/// [`tone`]. Surfaces, frames and hover greys are written this way; text
/// inks keep a plain `#` so they stay readable on any background.
pub fn themed(source: &str) -> std::borrow::Cow<'_, str> {
    if !source.contains("#~") {
        return source.into();
    }
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(at) = rest.find("#~") {
        out.push_str(&rest[..at]);
        let digits = rest
            .get(at + 2..at + 10)
            .filter(|d| d.chars().all(|c| c.is_ascii_hexdigit()));
        match digits.and_then(|d| u32::from_str_radix(d, 16).ok()) {
            Some(design) => {
                out.push_str(&format!("#{:08x}", tone(design)));
                rest = &rest[at + 10..];
            }
            None => {
                out.push_str("#~");
                rest = &rest[at + 2..];
            }
        }
    }
    out.push_str(rest);
    out.into()
}
fn shaded(rgb: u32, delta: i32) -> u32 {
    [16, 8, 0].into_iter().fold(0, |out, shift| {
        let channel = ((rgb >> shift & 0xff) as i32 + delta).clamp(0, 255) as u32;
        out | channel << shift
    })
}
/// "#rrggbb" (or "rrggbb") as 0xRRGGBB; anything else is rejected.
pub fn parse(text: &str) -> Option<u32> {
    let digits = text.trim();
    let digits = digits.strip_prefix('#').unwrap_or(digits);
    (digits.len() == 6 && digits.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(digits, 16).ok())
        .flatten()
}
/// 0xRRGGBB as "#rrggbb".
pub fn format(rgb: u32) -> String {
    format!("#{:06x}", rgb & 0xff_ffff)
}
/// Call before a window's "does my root node exist" check: a window built
/// with an older background is removed, so it rebuilds with the current one
/// through its usual missing-node path.
pub fn refresh(ctx: &mut StableClient<'_>, path: &'static str) {
    // Window text is part of the template, so the language counts too.
    let now = u64::from(background()) | (crate::lang::generation() as u64) << 32;
    let Ok(mut built) = BUILT.lock() else {
        return;
    };
    let exists = ctx.ui_exists(path);
    match built.iter_mut().find(|(p, _)| *p == path) {
        Some((_, used)) if *used != now => {
            if exists {
                ctx.ui_remove_node(path);
            }
            *used = now;
        }
        Some(_) => {}
        None => built.push((path, now)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hex_input_is_six_digits_with_an_optional_hash() {
        assert_eq!(parse("#1c1a18"), Some(0x1c1a18));
        assert_eq!(parse(" 2A4B6c "), Some(0x2a4b6c));
        for bad in ["", "#12345", "#1234567", "#12345g", "red"] {
            assert_eq!(parse(bad), None, "{bad}");
        }
        assert_eq!(format(0x1c1a18), "#1c1a18");
        assert_eq!(super::DEFAULT << 8 | 0xff, 0x1c1a18ff);
        // The design's strip shade, and clamping at both ends.
        assert_eq!(shaded(DEFAULT, 6), 0x22201e);
        assert_eq!(shaded(0xfffa02, 6), 0xffff08);
        assert_eq!(shaded(0x030000, -6), 0x000000);
    }
    #[test]
    fn design_greys_follow_the_background() {
        // The default background (the test thread never syncs) keeps them.
        assert_eq!(tone(0x3a3837ff), 0x3a3837ff);
        assert_eq!(
            themed("color: #~3a3837ff; ink: #393939ff;"),
            "color: #3a3837ff; ink: #393939ff;"
        );
        assert_eq!(themed("#~1e1e1dd9 #~xyz"), "#1e1e1dd9 #~xyz");
        assert_eq!(color("~3a3837ff"), Some(0x3a3837ff));
        assert_eq!(color("fdee00ff"), Some(0xfdee00ff));
        // #151720 is (-7, -3, +8) from #1c1a18; clamped at 0.
        assert_eq!(toned(0x3a3837ff, 0x151720), 0x33353fff);
        assert_eq!(toned(0x0e0d0c80, 0x151720), 0x070a1480);
        assert_eq!(toned(0x0e0d0cff, 0x000000), 0x000000ff);
    }
}
