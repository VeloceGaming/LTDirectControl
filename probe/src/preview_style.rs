//! The skill-preview look: every colour, width, opacity and size of the
//! design (design/previews/concept.html), kept apart from the geometry.
//! Defaults are `preview_style.json`, built in; any subset of its keys can
//! be overridden in `%LOCALAPPDATA%\LTDirectControl\preview_style.json`
//! (read once at start-up).
use crate::Logger;
use serde_json::Value;
use std::sync::OnceLock;

macro_rules! style {
    (colors { $($color:ident),* $(,)? } numbers { $($number:ident),* $(,)? }) => {
        /// Colours are 0xRRGGBB00 (opacity is added where drawn).
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct Style {
            $(pub $color: u32,)*
            $(pub $number: f32,)*
        }
        impl Style {
            const COLORS: &'static [&'static str] = &[$(stringify!($color)),*];
            const NUMBERS: &'static [&'static str] = &[$(stringify!($number)),*];
            /// `base` with every valid key of `v` applied; invalid or
            /// unknown values leave the base value.
            fn merged(mut self, v: &Value) -> Self {
                $(if let Some(c) = v.get(stringify!($color)).and_then(color) {
                    self.$color = c;
                })*
                $(if let Some(n) = v
                    .get(stringify!($number))
                    .and_then(Value::as_f64)
                    .filter(|n| n.is_finite() && *n >= 0. && *n <= 10_000.)
                {
                    self.$number = n as f32;
                })*
                self
            }
            fn zero() -> Self {
                Self { $($color: 0,)* $($number: 0.,)* }
            }
        }
    };
}

style! {
    colors { yellow, white, orange, gray, ink, enemy, ally }
    numbers {
        strip_step,
        range_ink_width, range_ink_opacity, range_width, range_opacity,
        rim_width, rim_ink_extra, rim_ink_opacity, rim_glow_extra, rim_glow_opacity,
        rim_opacity, rim_core_width, rim_core_opacity,
        area_fill, accent_inset, accent_offset, accent_span, accent_width, accent_opacity,
        grip_size, grip_rim_width, grip_cross, grip_cross_width,
        corridor_fill, corridor_edge_width, corridor_edge_opacity,
        cone_spine_inset, cone_spine_width, cone_spine_opacity,
        cone_inner_arc, cone_inner_width, cone_inner_opacity,
        wall_end_width, wall_end_opacity, wall_grip_size,
        arrow_start, skillshot_head, skillshot_wing, dash_head, dash_wing,
        dash_short, dash_short_grip, arrow_ink_extra, arrow_ink_opacity,
        arrow_outline_opacity, arrow_body_width, arrow_body_fill, arrow_head_width,
        arrow_upper_facet_fill, arrow_lower_facet_fill, arrow_edge_opacity,
        arrow_tip_width, arrow_grip_width, arrow_grip_opacity,
        blink_rx, blink_ry, blink_gap, blink_width, blink_core_radius, blink_core_fill,
        blink_pin_height, blink_pin_width, blink_pin_stroke,
        blink_tick_offset, blink_tick_half, blink_tick_width,
        target_rx, target_ry, target_arc_gap, target_front_width, target_front_opacity,
        target_back_width, target_back_opacity, target_marker_lift, target_marker_half,
        target_marker_height, target_marker_fill, target_marker_rim,
        beyond_width, beyond_opacity, beyond_dash, beyond_gap, beyond_ring,
    }
}

/// "#rrggbb" as 0xRRGGBB00.
fn color(v: &Value) -> Option<u32> {
    let hex = v.as_str()?.strip_prefix('#')?;
    (hex.len() == 6)
        .then(|| u32::from_str_radix(hex, 16).ok())
        .flatten()
        .map(|rgb| rgb << 8)
}
/// A colour at opacity 0..1.
pub fn alpha(rgb: u32, opacity: f32) -> u32 {
    (rgb & 0xffff_ff00) | (opacity.clamp(0., 1.) * 255.).round() as u32
}

impl Default for Style {
    /// The built-in design values.
    fn default() -> Self {
        let base: Value =
            serde_json::from_str(include_str!("preview_style.json")).expect("preview style");
        Self::zero().merged(&base)
    }
}

static STYLE: OnceLock<Style> = OnceLock::new();
/// The active style (built-in values until `load` runs).
pub fn style() -> &'static Style {
    STYLE.get_or_init(Style::default)
}
/// Apply the user's override file, if any, once at start-up.
pub fn load(log: &Logger) {
    let path = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .map(|root| root.join("LTDirectControl").join("preview_style.json"));
    let user = path
        .as_ref()
        .and_then(|p| std::fs::read(p).ok())
        .map(|bytes| serde_json::from_slice::<Value>(&bytes));
    let style = match user {
        Some(Ok(v)) => {
            let known = v
                .as_object()
                .map(|o| {
                    o.keys()
                        .filter(|k| {
                            Style::COLORS.contains(&k.as_str())
                                || Style::NUMBERS.contains(&k.as_str())
                        })
                        .count()
                })
                .unwrap_or(0);
            log.write(&format!("PREVIEW STYLE user overrides={known}"));
            Style::default().merged(&v)
        }
        Some(Err(e)) => {
            log.write(&format!("PREVIEW STYLE user file ignored: {e}"));
            Style::default()
        }
        None => Style::default(),
    };
    let _ = STYLE.set(style);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_built_in_file_sets_every_key_and_overrides_are_partial() {
        let base: Value = serde_json::from_str(include_str!("preview_style.json")).unwrap();
        for key in Style::COLORS {
            assert!(base.get(*key).and_then(color).is_some(), "{key}");
        }
        for key in Style::NUMBERS {
            assert!(base.get(*key).and_then(Value::as_f64).is_some(), "{key}");
        }
        let s = Style::default();
        assert_eq!(s.yellow, 0xfdee_0000);
        assert_eq!(s.area_fill, 0.065);
        // A partial override changes only its own keys; bad values are ignored.
        let o = s.merged(&serde_json::json!({
            "yellow": "#00ff00", "rim_width": 4, "area_fill": -1, "gray": "nope"
        }));
        assert_eq!((o.yellow, o.rim_width), (0x00ff_0000, 4.));
        assert_eq!((o.area_fill, o.gray), (s.area_fill, s.gray));
        assert_eq!(alpha(s.yellow, 0.065), 0xfdee_0011);
    }
}
