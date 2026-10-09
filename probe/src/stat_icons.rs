//! Stat icons: a frame of the game's stat sheet where it has one, otherwise
//! of the mod's `ui/stat_icons` sheet (9x9, tools/generate_stat_icons.py).
//! Inline text images need a sheet and a frame name, so both are sheets.

pub const GAME_STAT_SHEET: &str = "asset/base/ui/banpick/champion_stat_icon";
pub const MOD_STAT_SHEET: &str = "asset/lt_direct_control_probe/ui/stat_icons";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StatIcon {
    Game(&'static str),
    Mod(&'static str),
}
impl StatIcon {
    /// A hidden image node showing this icon (pixel-crisp, no tint).
    pub fn node(self, name: &str, (x, y, size): (i32, i32, i32), z: i32) -> String {
        let (sheet, tag) = self.sheet();
        format!("#{name}:image {{ x: {x}px; y: {y}px; width: {size}px; height: {size}px; source: \"{sheet}\"; rect_tag: \"{tag}\"; sample_linear: false; color: #ffffffff; ignore_event: true; visible: false; z: {z}; }}\n")
    }
    /// The same icon inline in label text.
    pub fn inline(self) -> String {
        let (sheet, tag) = self.sheet();
        format!("<i#{sheet}:{tag}>")
    }
    pub fn sheet(self) -> (&'static str, &'static str) {
        match self {
            StatIcon::Game(tag) => (GAME_STAT_SHEET, tag),
            StatIcon::Mod(tag) => (MOD_STAT_SHEET, tag),
        }
    }
}
pub const AD: StatIcon = StatIcon::Game("ad_0");
pub const AP: StatIcon = StatIcon::Game("ap_0");
pub const ATTACK_SPEED: StatIcon = StatIcon::Game("attack_speed_0");
pub const ARMOR: StatIcon = StatIcon::Game("armor_0");
pub const MAGIC_RESIST: StatIcon = StatIcon::Game("magic resistance_0");
pub const HEALTH: StatIcon = StatIcon::Game("hp_0");
pub const MOVE_SPEED: StatIcon = StatIcon::Game("speed_0");
pub const RANGE: StatIcon = StatIcon::Game("range_0");
pub const CRIT: StatIcon = StatIcon::Mod("crit_0");
pub const HASTE: StatIcon = StatIcon::Mod("haste_0");
pub const LIFESTEAL: StatIcon = StatIcon::Mod("lifesteal_0");
pub const ARMOR_PEN: StatIcon = StatIcon::Mod("armor_pen_0");
pub const MAGIC_PEN: StatIcon = StatIcon::Mod("mr_pen_0");

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_mod_icon_is_a_frame_of_the_shipped_sheet() {
        let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui");
        let data = std::fs::read_to_string(ui.join("stat_icons#data.sprite_sheet")).unwrap();
        let data: serde_json::Value = serde_json::from_str(&data).unwrap();
        for icon in [CRIT, HASTE, LIFESTEAL, ARMOR_PEN, MAGIC_PEN] {
            let StatIcon::Mod(tag) = icon else {
                unreachable!()
            };
            assert!(data["images"].get(tag).is_some(), "{tag}");
        }
        assert_eq!(
            AD.inline(),
            "<i#asset/base/ui/banpick/champion_stat_icon:ad_0>"
        );
        assert_eq!(
            CRIT.inline(),
            "<i#asset/lt_direct_control_probe/ui/stat_icons:crit_0>"
        );
    }
}
