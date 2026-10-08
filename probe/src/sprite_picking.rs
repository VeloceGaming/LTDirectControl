//! Bounded body-pose envelopes. No animation pointers or textures retained.
use crate::{hud_icons, Logger};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::OnceLock,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub width: f32,
    pub height: f32,
}
impl Default for Body {
    fn default() -> Self {
        Self {
            width: 28.,
            height: 32.,
        }
    }
}
impl Body {
    fn pixels(w: f32, h: f32) -> Option<Self> {
        // Retain the tested map scale. Frames cover body animations, excluding
        // separate effect/end tags; this is an envelope, not live alpha picking.
        (w.is_finite() && h.is_finite() && (4. ..=256.).contains(&w) && (4. ..=256.).contains(&h))
            .then_some(Self {
                width: w * 0.5,
                height: h * 0.5,
            })
    }
}
static PROFILES: OnceLock<HashMap<String, Body>> = OnceLock::new();
static BASE: OnceLock<Value> = OnceLock::new();
pub fn body(name: Option<&str>) -> Body {
    name.and_then(|name| PROFILES.get()?.get(name).copied())
        .unwrap_or_default()
}
pub fn entity_body(
    name: Option<&str>,
    champion: bool,
    minion: bool,
    tower: bool,
    radius: u64,
) -> Option<Body> {
    if champion {
        return Some(body(name));
    }
    if minion {
        // Largest normal base minion pose (including Morgard minions), without
        // using its combat collision radius to inflate the mouse target.
        return Some(Body {
            width: 12.5,
            height: 13.5,
        });
    }
    let name = name.map(|n| n.rsplit('/').next().unwrap_or(n).trim_end_matches("#anim"));
    // Native entity names are generic; the renderer maps them to side-specific
    // art. Both sides have matching body dimensions. Keep this mapping here.
    let name = name.map(|n| match n {
        "tower" => "blue_tower",
        "nexus" => "blue_nexus",
        other => other,
    });
    let structure = tower
        || matches!(
            name,
            Some("blue_tower" | "red_tower" | "blue_nexus" | "red_nexus")
        );
    if let Some(profile) = name.and_then(|name| {
        PROFILES
            .get()
            .and_then(|p| p.get(&format!("ingame/{name}")).copied())
            .or_else(|| {
                // Baked base metadata also works before runtime initialization.
                let p = BASE.get_or_init(|| {
                    serde_json::from_str(include_str!("picking_assets.json"))
                        .expect("base sprite dimensions")
                });
                let p = p.get(format!("ingame/{name}"))?;
                Body::pixels(p.get(0)?.as_f64()? as f32, p.get(1)?.as_f64()? as f32)
            })
    }) {
        // Structures need their full visible body, rather than the half-scale
        // champion/monster baseline. The final bounds add only modest padding.
        return Some(if structure {
            Body {
                width: profile.width * 2.,
                height: profile.height * 2.,
            }
        } else {
            profile
        });
    }
    if structure {
        return Some(Body {
            width: 48.,
            height: 80.,
        });
    }
    // Unknown/modded monsters have no declared champion art. Use a bounded
    // envelope until their art can be identified, keeping champion priority.
    let width = (radius as f32 / 500.).clamp(24., 96.);
    Some(Body {
        width,
        height: width * 1.25,
    })
}
pub fn initialize(log: &Logger) {
    let Some(game) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_owned))
    else {
        return;
    };
    let profiles = load(&game, log);
    let _ = PROFILES.set(profiles);
}
fn normal(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "idle",
        "run",
        "walk",
        "hit",
        "attack",
        "skill",
        "skill2",
        "ult",
        "skill_pre",
        "skill2_pre",
        "ult_pre",
    ]
    .into_iter()
    .any(|s| name == s || name.ends_with(&format!("_{s}")))
}
fn stable(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ["idle", "run", "walk"]
        .into_iter()
        .any(|s| name == s || name.ends_with(&format!("_{s}")))
}
fn envelope(poses: &[(f32, f32)], standing: &[(f32, f32)]) -> Option<Body> {
    if poses.is_empty() {
        return None;
    }
    let dimension = |axis: usize| {
        let value = |p: &(f32, f32)| if axis == 0 { p.0 } else { p.1 };
        let largest = poses.iter().map(value).fold(0f32, f32::max);
        let baseline = if standing.is_empty() {
            // Missing idle/walk tags: use the median pose, not a lone effect outlier.
            let mut values = poses.iter().map(value).collect::<Vec<_>>();
            values.sort_by(f32::total_cmp);
            values[(values.len() - 1) / 2]
        } else {
            standing.iter().map(value).fold(0f32, f32::max)
        };
        largest.min(baseline * 1.15)
    };
    Body::pixels(dimension(0), dimension(1))
}
fn fanim(value: &Value) -> Option<Body> {
    let mut poses = Vec::new();
    let mut standing = Vec::new();
    for (name, animation) in value.get("anims")?.as_object()? {
        if !normal(name) {
            continue;
        }
        for frame in animation.get("frames")?.as_array()?.iter().take(512) {
            let d = frame.get("data")?;
            let (fw, fh) = (d.get("w")?.as_f64()? as f32, d.get("h")?.as_f64()? as f32);
            Body::pixels(fw, fh)?;
            poses.push((fw, fh));
            if stable(name) {
                standing.push((fw, fh));
            }
        }
    }
    envelope(&poses, &standing)
}
fn word(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}
fn dword(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn aseprite(b: &[u8]) -> Option<Body> {
    // Metadata only, per https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md
    // Cel dimensions include transparent holes; grouped/tilemap layers fall back.
    if b.len() < 128 || word(b, 4)? != 0xa5e0 || dword(b, 0)? as usize != b.len() {
        return None;
    }
    let count = word(b, 6)? as usize;
    if count == 0 || count > 1024 {
        return None;
    }
    let mut frames: Vec<HashMap<u16, (i16, i16, u16, u16)>> = Vec::new();
    let mut layers = Vec::new();
    let mut tags = Vec::new();
    let mut offset = 128usize;
    for index in 0..count {
        let length = dword(b, offset)? as usize;
        let end = offset.checked_add(length)?;
        if length < 16 || end > b.len() || word(b, offset + 4)? != 0xf1fa {
            return None;
        }
        let chunks = dword(b, offset + 12)?;
        let chunks = if chunks == 0 {
            word(b, offset + 6)? as u32
        } else {
            chunks
        };
        if chunks > 2048 {
            return None;
        }
        let mut c = offset + 16;
        let mut cels = HashMap::new();
        for _ in 0..chunks {
            let size = dword(b, c)? as usize;
            if size < 6 || c.checked_add(size)? > end {
                return None;
            }
            let data = b.get(c + 6..c + size)?;
            match word(b, c + 4)? {
                0x2004 => {
                    if index != 0
                        || layers.len() >= 64
                        || word(data, 2)? != 0
                        || word(data, 4)? != 0
                    {
                        return None;
                    }
                    layers.push(word(data, 0)? & 1 != 0 && *data.get(12)? > 0);
                }
                0x2005 => {
                    let layer = word(data, 0)?;
                    if layers.get(layer as usize).copied() == Some(true) && *data.get(6)? > 0 {
                        let x = word(data, 2)? as i16;
                        let y = word(data, 4)? as i16;
                        let dimensions = match word(data, 7)? {
                            0 | 2 => Some((x, y, word(data, 16)?, word(data, 18)?)),
                            1 => {
                                let linked = word(data, 16)? as usize;
                                if linked >= index {
                                    return None;
                                }
                                frames
                                    .get(linked)?
                                    .get(&layer)
                                    .map(|(_, _, w, h)| (x, y, *w, *h))
                            }
                            _ => return None,
                        };
                        if let Some(d) = dimensions {
                            cels.insert(layer, d);
                        }
                    }
                }
                0x2018 => {
                    let n = word(data, 0)? as usize;
                    if n > 128 {
                        return None;
                    }
                    let mut p = 10;
                    for _ in 0..n {
                        let (from, to) = (word(data, p)? as usize, word(data, p + 2)? as usize);
                        let length = word(data, p + 17)? as usize;
                        let name = std::str::from_utf8(data.get(p + 19..p + 19 + length)?).ok()?;
                        if from > to || to >= count {
                            return None;
                        }
                        if normal(name) {
                            tags.push((from, to, stable(name)));
                        }
                        p += 19 + length;
                    }
                }
                _ => {}
            }
            c += size;
        }
        frames.push(cels);
        offset = end;
    }
    let mut poses = Vec::new();
    let mut standing = Vec::new();
    for (from, to, stable) in tags {
        for frame in &frames[from..=to] {
            let left = frame.values().map(|(x, _, _, _)| *x as i32).min()?;
            let top = frame.values().map(|(_, y, _, _)| *y as i32).min()?;
            let right = frame
                .values()
                .map(|(x, _, w, _)| *x as i32 + *w as i32)
                .max()?;
            let bottom = frame
                .values()
                .map(|(_, y, _, h)| *y as i32 + *h as i32)
                .max()?;
            let size = ((right - left) as f32, (bottom - top) as f32);
            Body::pixels(size.0, size.1)?;
            poses.push(size);
            if stable {
                standing.push(size);
            }
        }
    }
    envelope(&poses, &standing)
}
fn owned(root: &Path, id: &str, source: &str) -> Option<PathBuf> {
    let relative = source.strip_prefix(&format!("asset/{id}/"))?;
    if relative
        .split('/')
        .any(|p| p.is_empty() || p == "." || p == ".." || p.contains(['\\', ':']))
    {
        return None;
    }
    Some(root.join(relative))
}
fn sprite(root: &Path, id: &str, source: &str) -> Option<Body> {
    let base = owned(root, id, source)?;
    let animation = PathBuf::from(format!("{}#anim.fanim", base.display()));
    if animation.is_file() {
        return fanim(&hud_icons::json(&animation)?);
    }
    let source = base.with_extension("aseprite");
    if std::fs::metadata(&source).ok()?.len() > 4_000_000 {
        return None;
    }
    aseprite(&std::fs::read(source).ok()?)
}
fn load(game: &Path, log: &Logger) -> HashMap<String, Body> {
    let value: Value =
        serde_json::from_str(include_str!("picking_assets.json")).expect("base sprite dimensions");
    let mut profiles: HashMap<_, _> = value
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(name, d)| {
            Some((
                name.clone(),
                Body::pixels(d.get(0)?.as_f64()? as f32, d.get(1)?.as_f64()? as f32)?,
            ))
        })
        .collect();
    log.write(&format!("SPRITE PICKING base_profiles={} idle/walk baseline + capped 15% champion pose growth; monsters unchanged; no live animation handles",profiles.len()));
    for (id, root) in hud_icons::enabled_roots(game) {
        let mut files = Vec::new();
        hud_icons::declarations(&root, 0, &mut files);
        files.sort();
        for file in files {
            let Some(v) = hud_icons::json(&file) else {
                continue;
            };
            let Some(name) = v.get("id").and_then(Value::as_str) else {
                continue;
            };
            let profile = v
                .get("sprite")
                .and_then(Value::as_str)
                .and_then(|s| sprite(&root, &id, s))
                .or_else(|| {
                    v.get("sprite")
                        .and_then(Value::as_str)
                        .and_then(|s| s.strip_prefix("asset/base/aseprite_resources/champions/"))
                        .and_then(|s| profiles.get(s).copied())
                });
            // An overridden champion with unreadable art must not retain old art dimensions.
            profiles.insert(name.into(), profile.unwrap_or_default());
            log.write(&format!(
                "SPRITE PICKING champion={name} pack={id} body={:?} fallback={}",
                profile.unwrap_or_default(),
                profile.is_none()
            ));
        }
    }
    profiles
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unidentified_structures_never_fall_back_to_small_combat_collision() {
        let fallback = entity_body(None, false, false, true, 1000).unwrap();
        assert_eq!(
            fallback,
            Body {
                width: 48.,
                height: 80.
            }
        );
        // Nexus may not have the tower flag; known structure art still uses
        // structure sizing, including full logical resource names.
        assert_eq!(
            entity_body(
                Some("asset/base/aseprite_resources/ingame/blue_nexus#anim"),
                false,
                false,
                false,
                1000
            )
            .unwrap(),
            entity_body(Some("nexus"), false, false, false, 1000).unwrap()
        );
        assert_eq!(
            entity_body(Some("tower"), false, false, true, 1000),
            Some(Body {
                width: 31.,
                height: 63.
            })
        );
        assert_ne!(
            entity_body(Some("nexus"), false, false, false, 1000),
            Some(fallback)
        );
    }
    #[test]
    fn bundled_monsters_stay_unchanged_and_champions_cap_combat_poses() {
        let profiles: Value = serde_json::from_str(include_str!("picking_assets.json")).unwrap();
        assert_eq!(profiles["ingame/serpen"], serde_json::json!([59., 79.]));
        assert_eq!(profiles["ingame/rhino"], serde_json::json!([63., 51.]));
        assert_eq!(profiles["ingame/stump"], serde_json::json!([37., 41.]));
        assert_eq!(profiles["ingame/epic"], serde_json::json!([109., 133.]));
        assert_eq!(profiles["lancer"], serde_json::json!([63.25, 54.05]));
        assert_eq!(profiles["ogre"], serde_json::json!([58.65, 79.35]));
    }
    #[test]
    fn extreme_attack_poses_do_not_inflate_a_permanent_champion_box() {
        let body = envelope(&[(29., 51.), (163., 163.)], &[(29., 51.)]).unwrap();
        assert!((body.width - 16.675).abs() < 0.001);
        assert!((body.height - 29.325).abs() < 0.001);
        let fallback = envelope(&[(40., 60.), (42., 62.), (200., 200.)], &[]).unwrap();
        assert!((fallback.width - 24.15).abs() < 0.001);
        assert!((fallback.height - 35.65).abs() < 0.001);
    }
    #[test]
    fn aseprite_tags_visible_layers_links_and_bad_chunks_are_handled_without_pixels() {
        fn chunk(kind: u16, data: Vec<u8>) -> Vec<u8> {
            let mut b = ((data.len() + 6) as u32).to_le_bytes().to_vec();
            b.extend(kind.to_le_bytes());
            b.extend(data);
            b
        }
        fn layer(visible: bool) -> Vec<u8> {
            let mut d = vec![0; 18];
            d[0] = u8::from(visible);
            d[12] = 255;
            chunk(0x2004, d)
        }
        fn cel(layer: u16, w: u16, h: u16) -> Vec<u8> {
            let mut d = vec![0; 16];
            d[..2].copy_from_slice(&layer.to_le_bytes());
            d[6] = 255;
            d.extend(w.to_le_bytes());
            d.extend(h.to_le_bytes());
            // A valid raw image, opaque. Parser intentionally reads only metadata.
            d.extend(vec![255; w as usize * h as usize * 4]);
            chunk(0x2005, d)
        }
        let mut tags = vec![0; 10];
        tags[..2].copy_from_slice(&2u16.to_le_bytes());
        for (from, to, name) in [(0u16, 1u16, "idle"), (2, 2, "ult_effect")] {
            let mut tag = vec![0; 17];
            tag[..2].copy_from_slice(&from.to_le_bytes());
            tag[2..4].copy_from_slice(&to.to_le_bytes());
            tag.extend((name.len() as u16).to_le_bytes());
            tag.extend(name.bytes());
            tags.extend(tag);
        }
        let mut link = vec![0; 16];
        link[6] = 255;
        link[7] = 1;
        link.extend(0u16.to_le_bytes());
        let frames = vec![
            vec![
                layer(true),
                layer(false),
                chunk(0x2018, tags),
                cel(0, 40, 60),
                cel(1, 200, 200),
            ],
            vec![chunk(0x2005, link)],
            vec![cel(0, 200, 200)],
        ];
        let mut bytes = vec![0; 128];
        bytes[4..6].copy_from_slice(&0xa5e0u16.to_le_bytes());
        bytes[6..8].copy_from_slice(&3u16.to_le_bytes());
        for chunks in frames {
            let mut header = vec![0; 16];
            let length = 16 + chunks.iter().map(Vec::len).sum::<usize>();
            header[..4].copy_from_slice(&(length as u32).to_le_bytes());
            header[4..6].copy_from_slice(&0xf1fau16.to_le_bytes());
            header[6..8].copy_from_slice(&(chunks.len() as u16).to_le_bytes());
            bytes.extend(header);
            for chunk in chunks {
                bytes.extend(chunk);
            }
        }
        let length = bytes.len() as u32;
        bytes[..4].copy_from_slice(&length.to_le_bytes());
        assert_eq!(
            aseprite(&bytes),
            Some(Body {
                width: 20.,
                height: 30.
            })
        );
        // Corrupt chunk cannot cause an unbounded walk or oversized click box.
        bytes[144..148].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(aseprite(&bytes).is_none());
        for length in [0, 10, 128, 144] {
            assert!(aseprite(&bytes[..length]).is_none());
        }
    }
    #[test]
    fn normal_frames_ignore_large_effects_and_reject_bad_dimensions_and_paths() {
        let value = serde_json::json!({"anims":{"idle":{"frames":[{"data":{"w":40,"h":60}}]},"run":{"frames":[{"data":{"w":50,"h":55}}]},"attack":{"frames":[{"data":{"w":64,"h":55}}]},"ult_effect":{"frames":[{"data":{"w":200,"h":200}}]}}});
        assert_eq!(
            fanim(&value),
            Some(Body {
                width: 28.75,
                height: 30.
            })
        );
        assert!(Body::pixels(f32::NAN, 20.).is_none());
        assert!(Body::pixels(20000., 20.).is_none());
        assert!(owned(Path::new("pack"), "mod", "asset/mod/../other").is_none());
        assert!(owned(Path::new("pack"), "mod", "asset/other/sprite").is_none());
    }
    #[test]
    fn installed_profile_report_when_explicitly_requested() {
        let Ok(game) = std::env::var("LT_PICKING_VERIFY_GAME") else {
            return;
        };
        let log = crate::test_support::logger("installed-picking");
        let p = load(Path::new(&game), &log);
        assert!(p["ogre"].height > p["lancer"].height);
        assert_ne!(p["harpy"], Body::default());
        assert_ne!(p["cf_archangel"], Body::default());
        println!(
            "{} profiles; lancer={:?} ogre={:?} harpy={:?} cf_archangel={:?}",
            p.len(),
            p["lancer"],
            p["ogre"],
            p["harpy"],
            p["cf_archangel"]
        );
    }
}
