//! Read enabled champion asset declarations; item settings come from the host.
//! Paths are references to installed assets, never copied textures or executable instructions.
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Icon {
    pub source: String,
    pub tag: Option<String>,
}
impl Icon {
    pub fn properties(&self) -> String {
        let mut text = format!("source: {:?};", self.source);
        if let Some(tag) = &self.tag {
            text.push_str(&format!(" rect_tag: {tag:?};"));
        }
        text
    }
}
pub(crate) fn json(path: &Path) -> Option<Value> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > 2_000_000 {
        return None;
    }
    serde_json::from_slice(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes)).ok()
}
pub(crate) fn declarations(root: &Path, depth: usize, files: &mut Vec<PathBuf>) {
    if depth > 8 || files.len() >= 512 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            declarations(&entry.path(), depth + 1, files);
        } else if entry
            .path()
            .extension()
            .is_some_and(|x| x == "data_champion")
        {
            files.push(entry.path());
        }
        if files.len() >= 512 {
            break;
        }
    }
}
fn skills(value: &Value, id: &str, root: &Path) -> Option<Vec<Icon>> {
    let values = value.get("skill_icons")?.as_array()?;
    if values.len() != 3 {
        return None;
    }
    values
        .iter()
        .map(|v| {
            let source = v.as_str()?;
            let relative = source.strip_prefix(&format!("asset/{id}/"))?;
            if relative
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == ".." || p.contains(['\\', ':']))
            {
                return None;
            }
            root.join(relative)
                .with_extension("png")
                .is_file()
                .then(|| Icon {
                    source: source.into(),
                    tag: None,
                })
        })
        .collect()
}
#[derive(Default)]
pub struct EnabledAssets {
    pub champions: HashMap<String, Vec<Icon>>,
    pub descriptions: HashMap<String, Value>,
    pub item_tags: HashSet<String>,
}
fn sheet_tags(value: &Value) -> HashSet<String> {
    value
        .get("images")
        .and_then(Value::as_object)
        .map_or(HashSet::new(), |images| images.keys().cloned().collect())
}
pub fn enabled_assets() -> EnabledAssets {
    let Ok(exe) = std::env::current_exe() else {
        return EnabledAssets::default();
    };
    let Some(game) = exe.parent() else {
        return EnabledAssets::default();
    };
    discover(game)
}
pub(crate) fn enabled_roots(game: &Path) -> Vec<(String, PathBuf)> {
    let Some(config) = json(&game.join("config/game/mods.json")) else {
        return Vec::new();
    };
    let mut roots = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(game.join("mods")) {
        for entry in entries.flatten() {
            if let Some(id) = json(&entry.path().join("mod.mod_info"))
                .and_then(|v| v.get("mod_id").and_then(Value::as_str).map(str::to_owned))
            {
                roots.insert(id, entry.path());
            }
        }
    }
    // Steam workshop sibling to steamapps/common/<game>.
    if let Some(steamapps) = game.parent().and_then(Path::parent) {
        let workshop = steamapps.join("workshop/content/3009300");
        if let Some(items) = config.get("known_workshop_items").and_then(Value::as_array) {
            for item in items {
                let Some(number) = item.get("published_file_id").and_then(Value::as_u64) else {
                    continue;
                };
                let root = workshop.join(number.to_string());
                if let Some(id) = json(&root.join("mod.mod_info"))
                    .and_then(|v| v.get("mod_id").and_then(Value::as_str).map(str::to_owned))
                {
                    roots.entry(id).or_insert(root);
                }
            }
        }
    }
    config
        .get("enabled_mods")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .filter_map(|id| roots.get(id).map(|root| (id.to_owned(), root.clone())))
        .collect()
}
fn discover(game: &Path) -> EnabledAssets {
    let mut result = EnabledAssets::default();
    for (id, root) in enabled_roots(game) {
        if let Some(overrides) = json(&root.join("mod.override_info")) {
            if let Some(rule) =
                overrides.get("asset/base/aseprite_resources/ingame/item_icons_18x18#data")
            {
                if rule.get("type").and_then(Value::as_str) == Some("override") {
                    if let Some(relative) = rule
                        .get("remapping")
                        .and_then(Value::as_str)
                        .and_then(|s| s.strip_prefix(&format!("asset/{id}/")))
                        .filter(|s| {
                            !s.split('/').any(|p| {
                                p.is_empty() || p == "." || p == ".." || p.contains(['\\', ':'])
                            })
                        })
                    {
                        if let Some(sheet) =
                            json(&root.join(relative).with_extension("sprite_sheet"))
                        {
                            result.item_tags = sheet_tags(&sheet);
                        }
                    }
                }
            }
        }
        let mut files = Vec::new();
        declarations(&root, 0, &mut files);
        files.sort();
        for file in files {
            let Some(value) = json(&file) else { continue };
            let Some(name) = value.get("id").and_then(Value::as_str) else {
                continue;
            };
            result.descriptions.insert(name.to_owned(), value.clone());
            if let Some(icons) = skills(&value, &id, &root) {
                result.champions.insert(name.to_owned(), icons);
            }
        }
    }
    result
}

/// The expansion overrides the base sheet through mod.override_info. Keep that
/// logical source so the host resolves enabled overrides, rather than bypassing them.
pub fn item_icon(value: &Value) -> Option<Icon> {
    let tag = value.get("icon")?.as_str()?;
    (!tag.is_empty()).then(|| Icon {
        source: "asset/base/aseprite_resources/ingame/item_icons_18x18".into(),
        tag: Some(tag.into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn item_settings_use_active_icon_tag_and_logical_overridden_sheet() {
        let icon = item_icon(&serde_json::json!({"icon":"new_item_tag"})).unwrap();
        assert_eq!(icon.tag.as_deref(), Some("new_item_tag"));
        assert!(icon.source.starts_with("asset/base/"));
        assert!(item_icon(&serde_json::json!({"icon":null})).is_none());
        assert!(item_icon(&serde_json::json!({"icon":""})).is_none());
        assert_eq!(
            Icon {
                source: "asset/pack/icons/q".into(),
                tag: None
            }
            .properties(),
            "source: \"asset/pack/icons/q\";"
        );
    }
    #[test]
    fn champion_png_references_require_owned_existing_files_and_reject_traversal() {
        let root = std::env::temp_dir().join(format!("lt-icon-test-{}", std::process::id()));
        std::fs::create_dir_all(root.join("icons")).unwrap();
        std::fs::write(root.join("icons/q.png"), []).unwrap();
        let valid = serde_json::json!({"skill_icons":["asset/pack/icons/q","asset/pack/icons/q","asset/pack/icons/q"]});
        assert_eq!(skills(&valid, "pack", &root).unwrap().len(), 3);
        for bad in [
            "asset/other/icons/q",
            "asset/pack/../icons/q",
            "asset/pack/C:/q",
            "asset/pack/missing",
        ] {
            assert!(skills(
                &serde_json::json!({"skill_icons":[bad,bad,bad]}),
                "pack",
                &root
            )
            .is_none());
        }
        // Remove only the explicitly created file/directory, without recursion.
        std::fs::remove_file(root.join("icons/q.png")).unwrap();
        std::fs::remove_dir(root.join("icons")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn enabled_pack_discovery_excludes_disabled_assets_and_reads_overridden_item_tags() {
        let game = std::env::temp_dir().join(format!("lt-pack-test-{}", std::process::id()));
        let pack = game.join("mods/pack");
        for dir in [
            game.join("config/game"),
            pack.join("icons"),
            pack.join("champion"),
            pack.join("sheet"),
        ] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let metadata = serde_json::json!({"mod_id":"pack"});
        std::fs::write(pack.join("mod.mod_info"), metadata.to_string()).unwrap();
        std::fs::write(pack.join("icons/q.png"), []).unwrap();
        std::fs::write(pack.join("champion/example.data_champion"),serde_json::json!({"id":"example","skill_icons":["asset/pack/icons/q","asset/pack/icons/q","asset/pack/icons/q"]}).to_string()).unwrap();
        std::fs::write(
            pack.join("sheet/items#data.sprite_sheet"),
            serde_json::json!({"images":{"registered_item":{"x":0}}}).to_string(),
        )
        .unwrap();
        std::fs::write(pack.join("mod.override_info"),serde_json::json!({"asset/base/aseprite_resources/ingame/item_icons_18x18#data":{"remapping":"asset/pack/sheet/items#data","type":"override"}}).to_string()).unwrap();
        let config = game.join("config/game/mods.json");
        std::fs::write(&config, "{\"enabled_mods\":[]}").unwrap();
        let disabled = discover(&game);
        assert!(disabled.champions.is_empty() && disabled.item_tags.is_empty());
        std::fs::write(&config, "{\"enabled_mods\":[\"pack\"]}").unwrap();
        let enabled = discover(&game);
        assert_eq!(enabled.champions["example"][0].source, "asset/pack/icons/q");
        assert!(enabled.item_tags.contains("registered_item"));
        assert!(!enabled.item_tags.contains("guessed_item"));
        for path in [
            config,
            pack.join("mod.mod_info"),
            pack.join("icons/q.png"),
            pack.join("champion/example.data_champion"),
            pack.join("sheet/items#data.sprite_sheet"),
            pack.join("mod.override_info"),
        ] {
            std::fs::remove_file(path).unwrap();
        }
        for dir in [
            pack.join("icons"),
            pack.join("champion"),
            pack.join("sheet"),
            pack,
            game.join("mods"),
            game.join("config/game"),
            game.join("config"),
            game,
        ] {
            std::fs::remove_dir(dir).unwrap();
        }
    }
}
