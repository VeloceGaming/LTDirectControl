//! The glyph PNGs the drawn windows show (1.6.0-perf-dev3).
//!
//! `draw_sprite` draws a PNG at its own pixel size and cannot tint it, and
//! `draw_svg` fills the glyphs' strokes (both seen in game 2026-10-24). So
//! every glyph image that left the UI tree ships once per size and tint it
//! is shown at: the shipped 32 px glyph scaled down like the node scaled it
//! and multiplied by the node's tint (`node tools/generate_endfield_glyphs.cjs
//! --drawn`, listed in `tools/records/drawn-glyphs.json`). A glyph whose
//! tint animates (the purchase coin and check, the vision eyes, the state
//! plate's glyph) stays an image node in the tree.

/// (glyph, size in px, tint 0xRRGGBBAA, asset). The tint is the node's
/// image colour; white means none.
pub const BAKED: [(&str, u32, u32, &str); 20] = [
    (
        "ef_swords",
        26,
        0xeeececff,
        "asset/lt_direct_control/ui/drawn_ef_swords_26_eeecec",
    ),
    (
        "hud_minion",
        26,
        0xffffffff,
        "asset/lt_direct_control/ui/drawn_hud_minion_26",
    ),
    (
        "ef_skull",
        22,
        0xeeececff,
        "asset/lt_direct_control/ui/drawn_ef_skull_22_eeecec",
    ),
    (
        "lock",
        24,
        0xffffffff,
        "asset/lt_direct_control/ui/drawn_lock_24",
    ),
    (
        "ef_unavailable",
        20,
        0xeeececff,
        "asset/lt_direct_control/ui/drawn_ef_unavailable_20_eeecec",
    ),
    (
        "ef_coin",
        17,
        0xeeececff,
        "asset/lt_direct_control/ui/drawn_ef_coin_17_eeecec",
    ),
    (
        "ef_coin",
        17,
        0xfdee00ff,
        "asset/lt_direct_control/ui/drawn_ef_coin_17_fdee00",
    ),
    (
        "ef_recall",
        20,
        0xeeececff,
        "asset/lt_direct_control/ui/drawn_ef_recall_20_eeecec",
    ),
    (
        "ef_play",
        26,
        0xeeececff,
        "asset/lt_direct_control/ui/drawn_ef_play_26_eeecec",
    ),
    (
        "ef_play",
        26,
        0x1c1a18ff,
        "asset/lt_direct_control/ui/drawn_ef_play_26_1c1a18",
    ),
    (
        "ef_pause",
        26,
        0xeeececff,
        "asset/lt_direct_control/ui/drawn_ef_pause_26_eeecec",
    ),
    (
        "ef_pause",
        26,
        0x1c1a18ff,
        "asset/lt_direct_control/ui/drawn_ef_pause_26_1c1a18",
    ),
    (
        "ef_eye",
        26,
        0xeeececff,
        "asset/lt_direct_control/ui/drawn_ef_eye_26_eeecec",
    ),
    (
        "ef_cpu",
        26,
        0xeeececff,
        "asset/lt_direct_control/ui/drawn_ef_cpu_26_eeecec",
    ),
    (
        "ef_camera",
        26,
        0xeeececff,
        "asset/lt_direct_control/ui/drawn_ef_camera_26_eeecec",
    ),
    (
        "ef_camera",
        26,
        0xfdee00ff,
        "asset/lt_direct_control/ui/drawn_ef_camera_26_fdee00",
    ),
    (
        "ef_camera",
        26,
        0xcbc9c7ff,
        "asset/lt_direct_control/ui/drawn_ef_camera_26_cbc9c7",
    ),
    (
        "ef_settings",
        26,
        0xeeececff,
        "asset/lt_direct_control/ui/drawn_ef_settings_26_eeecec",
    ),
    (
        "ef_list",
        26,
        0xeeececff,
        "asset/lt_direct_control/ui/drawn_ef_list_26_eeecec",
    ),
    (
        "ef_swap",
        32,
        0xeeececff,
        "asset/lt_direct_control/ui/drawn_ef_swap_32_eeecec",
    ),
];
/// The baked PNG for `glyph` at `size` px tinted `tint`. A 32 px glyph with
/// no tint is the shipped PNG itself.
pub fn asset(glyph: &str, size: u32, tint: u32) -> Option<&'static str> {
    if size == 32 && tint == 0xffffffff {
        return match glyph {
            "hourglass" => Some("asset/lt_direct_control/ui/hourglass"),
            _ => None,
        };
    }
    BAKED
        .iter()
        .find(|(g, s, t, _)| *g == glyph && *s == size && *t == tint)
        .map(|v| v.3)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_table_matches_the_baked_files_and_their_record() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let record: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("../tools/records/drawn-glyphs.json")).unwrap(),
        )
        .unwrap();
        let variants = record["variants"].as_array().unwrap();
        assert_eq!(variants.len(), BAKED.len());
        for (v, (glyph, size, tint, asset)) in variants.iter().zip(BAKED) {
            assert_eq!(v["glyph"], glyph);
            assert_eq!(v["size"], size);
            assert_eq!(v["tint"], format!("{tint:08x}"));
            let file = asset.strip_prefix("asset/lt_direct_control/").unwrap();
            assert!(root.join(format!("{file}.png")).is_file(), "{file}");
            assert!(
                record["files"].get(format!("{file}.png")).is_some(),
                "{file}"
            );
        }
        assert_eq!(asset("ef_coin", 17, 0xfdee00ff), Some(BAKED[6].3));
        assert_eq!(asset("ef_coin", 18, 0xfdee00ff), None);
        assert_eq!(
            asset("hourglass", 32, 0xffffffff),
            Some("asset/lt_direct_control/ui/hourglass")
        );
    }
}
