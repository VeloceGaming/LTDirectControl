//! "Live rating" (Interface > Appearance, Off by default): your athlete's
//! match rating as the game keeps it, drawn small in the top right corner,
//! in the row under the input delay's place (it stays there when the input
//! delay is off). Display only: the number is the game's own `rating` from
//! the athlete's match statistics, read through the official mod tools
//! (`statistics_json`) during the HUD sample, and only while the setting is
//! on. The result screen shows the same number divided by 10 (66 is 6.60).
use crate::native_timing::MatchKey;
use mod_api_stable::{StableClient, StableSpriteParams, TextAlignXV1, TextAlignYV1};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;

/// The row under the input delay (1702, 56, 200, 36): its right edge.
const RIGHT: f32 = 1902.;
const TOP: f32 = 94.;
const HEIGHT: f32 = 24.;
/// The number: the mod's numeric font, smaller than the input delay's 28 px.
const SIZE: f32 = 20.;
/// The mod's muted text colour.
const GREY: u32 = 0xcbc9c7ff;
/// The input delay's shadow: 2 px down and right.
const SHADOW: (f32, u32) = (2., 0x000000c0);
const STAR: f32 = 18.;
/// Space between the star and the number.
const GAP: f32 = 6.;
const Z: i32 = 1000;

static SHOWN: AtomicBool = AtomicBool::new(false);
/// The latest reading: match, athlete and the rating as the game stores it.
static LATEST: Mutex<Option<(MatchKey, usize, f64)>> = Mutex::new(None);
/// Which read works: 0 not known yet, 1 the `rating` field, 2 the whole
/// statistics document (the read the SDK documents with an empty path).
static PATH: AtomicU8 = AtomicU8::new(0);
/// The match and athlete the last log line was about (one line each).
static LOGGED: Mutex<Option<(MatchKey, usize)>> = Mutex::new(None);

pub fn set_shown(on: bool) {
    SHOWN.store(on, Ordering::Relaxed);
}
pub fn shown() -> bool {
    SHOWN.load(Ordering::Relaxed)
}

/// HUD sample (match worker): read the rating of the controlled athlete.
/// `read` is `statistics_json` for a path.
pub fn sample(
    key: MatchKey,
    athlete: usize,
    read: impl Fn(&str) -> Option<String>,
    log: &crate::Logger,
) {
    if !shown() {
        return;
    }
    let field = || read("rating").and_then(|json| parse(&json));
    let whole = || read("").and_then(|json| parse(&json));
    let (rating, path) = match PATH.load(Ordering::Relaxed) {
        1 => (field(), 1),
        2 => (whole(), 2),
        _ => match field() {
            Some(r) => (Some(r), 1),
            None => (whole(), 2),
        },
    };
    if rating.is_some() {
        PATH.store(path, Ordering::Relaxed);
    }
    if let Ok(mut logged) = LOGGED.lock() {
        if *logged != Some((key, athlete)) {
            *logged = Some((key, athlete));
            log.write(&match rating {
                Some(r) => format!(
                    "RATING first reading {r} for athlete {athlete} (shown {}; read {})",
                    text(r),
                    if path == 1 { "field" } else { "document" }
                ),
                None => format!("RATING unreadable for athlete {athlete}: nothing is shown"),
            });
        }
    }
    if let (Some(r), Ok(mut latest)) = (rating, LATEST.lock()) {
        *latest = Some((key, athlete, r));
    }
}

/// The rating in a statistics fragment: the number itself, or the whole
/// document's `rating` field.
fn parse(json: &str) -> Option<f64> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    value
        .as_f64()
        .or_else(|| value.get("rating").and_then(serde_json::Value::as_f64))
}

/// As the result screen writes it: the rating divided by 10, two decimals.
fn text(rating: f64) -> String {
    format!("{:.2}", rating / 10.)
}

/// The number for the athlete you control now, if one was read this match.
fn current(identity: Option<(MatchKey, usize)>) -> Option<String> {
    let (key, athlete) = identity?;
    let latest = *LATEST.lock().ok()?;
    latest
        .filter(|(k, a, _)| *k == key && *a == athlete)
        .map(|(_, _, r)| text(r))
}

/// Client render step, while the setting is on and you play: the star and
/// the number, right-aligned, each with the input delay's shadow.
pub fn draw(ctx: &mut StableClient<'_>, identity: Option<(MatchKey, usize)>) {
    if !shown() {
        return;
    }
    let Some(number) = current(identity) else {
        return;
    };
    // The packaged advances are the medium weight; the numeric one is about
    // 2 % wider.
    let width = crate::hud_style::width(&number, SIZE) * 1.02;
    let star = (RIGHT - width - GAP - STAR, TOP + (HEIGHT - STAR) / 2.);
    for (offset, color, z) in [(SHADOW.0, SHADOW.1, Z), (0., GREY, Z + 1)] {
        if let Some(asset) = crate::drawn_glyphs::asset("ef_star", STAR as u32, color) {
            ctx.draw_sprite(
                "UI",
                asset,
                &StableSpriteParams {
                    x: star.0 + offset,
                    y: star.1 + offset,
                    z,
                    sample_nearest: false,
                    ..StableSpriteParams::default()
                },
            );
        }
        ctx.draw_text(
            "UI",
            &number,
            "asset/lt_direct_control/font/numeric",
            (RIGHT - 200. + offset, TOP + offset, 200., HEIGHT),
            z,
            SIZE,
            color,
            TextAlignXV1::Right,
            TextAlignYV1::Center,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_number_reads_like_the_result_screen() {
        assert_eq!(text(66.), "6.60");
        assert_eq!(text(75.), "7.50");
        assert_eq!(text(100.), "10.00");
        assert_eq!(text(35.), "3.50");
    }

    #[test]
    fn a_field_or_the_whole_document_gives_the_rating() {
        assert_eq!(parse("66"), Some(66.));
        assert_eq!(parse("{\"kill\": 2, \"rating\": 71}"), Some(71.));
        assert_eq!(parse("{\"kill\": 2}"), None);
        assert_eq!(parse("not json"), None);
    }

    #[test]
    fn both_star_colours_are_baked() {
        for color in [GREY, SHADOW.1] {
            assert!(crate::drawn_glyphs::asset("ef_star", STAR as u32, color).is_some());
        }
    }

    #[test]
    fn the_widest_number_leaves_room_for_the_star_on_screen() {
        let width = crate::hud_style::width("10.00", SIZE) * 1.02;
        assert!(RIGHT - width - GAP - STAR > 1702.);
    }
}
