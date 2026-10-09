//! Selection bodies measured from the installed art (nothing is copied).
//!
//! Each unit kind gets a body: the opaque pixels of its standing frames,
//! with sideways protrusions trimmed (a column much shorter than the body,
//! such as a held gun or spear). Heads, hats and raised staffs stay. The
//! body is stored around the frame centre in world units (one sprite pixel
//! is one world unit) and placed on screen with the game's own draw data:
//! the drawn frame is found by its atlas position, which gives the true
//! anchor and facing. One sheet can hold several kinds (the minion sheet:
//! melee/ranged, each side, Morgard): animations named `<kind>_<tag>` form a
//! kind when a `<kind>_idle`/`_run`/`_walk` exists, and each frame uses its
//! kind's body. Mod champions shipped only as `.aseprite` files (the game
//! packs those itself, so atlas positions are unknown) get one body around
//! the canvas centre, placed on the drawn centre: champions are drawn centred
//! on the unit. See docs/investigation-preview-selection.md.
use crate::Logger;
use std::{
    collections::HashMap,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::OnceLock,
};

/// One sheet: atlas size, every frame's atlas rectangle with the index of
/// its kind's body, and the bodies (x0, y0, x1, y1) around the frame centre.
/// No frames: a centred champion from an `.aseprite` file, one body around
/// the drawn centre.
#[derive(Clone, Debug, PartialEq)]
pub struct Art {
    pub atlas: (u32, u32),
    pub frames: Vec<([u16; 4], u16)>,
    pub bodies: Vec<[f32; 4]>,
}
/// A unit type's sheets (a tower is its base and its orb).
pub type ArtSet = Vec<Art>;

static ARTS: OnceLock<(HashMap<String, u16>, Vec<ArtSet>)> = OnceLock::new();

/// Index of a unit type's art set ("fighter", "ingame/serpen", …), once loaded.
pub fn index(key: &str) -> Option<u16> {
    ARTS.get()?.0.get(key).copied()
}
/// Whether the background load has finished.
pub fn loaded() -> bool {
    ARTS.get().is_some()
}
pub fn set(index: u16) -> Option<&'static ArtSet> {
    ARTS.get()?.1.get(index as usize)
}

/// One body sprite as drawn this frame: atlas position/size as fractions
/// (u0, v0, du, dv) and the world corner it was drawn from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawn {
    pub uv: [f32; 4],
    pub corner: (f32, f32),
}

/// World rectangle (x0, y0, x1, y1) of a unit's body, from its drawn sprites.
/// None when no drawn sprite matches its art (fallback: the old envelope).
pub fn placed(set: &ArtSet, center: (f32, f32), sprites: &[Drawn]) -> Option<[f32; 4]> {
    if let [Art { frames, bodies, .. }] = set.as_slice() {
        if frames.is_empty() {
            let [x0, y0, x1, y1] = *bodies.first()?;
            let flipped = sprites.first()?.corner.0 > center.0 + 0.5;
            let (x0, x1) = if flipped { (-x1, -x0) } else { (x0, x1) };
            return Some([center.0 + x0, center.1 + y0, center.0 + x1, center.1 + y1]);
        }
    }
    let mut out: Option<[f32; 4]> = None;
    for sprite in sprites {
        for art in set {
            let (w_atlas, h_atlas) = (art.atlas.0 as f32, art.atlas.1 as f32);
            let at = (sprite.uv[0] * w_atlas, sprite.uv[1] * h_atlas);
            let size = (sprite.uv[2].abs() * w_atlas, sprite.uv[3].abs() * h_atlas);
            let Some((frame, kind)) = art.frames.iter().find(|(f, _)| {
                (f[0] as f32 - at.0).abs() <= 1.
                    && (f[1] as f32 - at.1).abs() <= 1.
                    && (f[2] as f32 - size.0).abs() <= 1.5
                    && (f[3] as f32 - size.1).abs() <= 1.5
            }) else {
                continue;
            };
            let (w, h) = (frame[2] as f32, frame[3] as f32);
            // Facing the other way, the sprite is drawn leftward from its corner.
            let flipped = sprite.corner.0 > center.0 + 0.5;
            let frame_center = (
                if flipped {
                    sprite.corner.0 - w / 2.
                } else {
                    sprite.corner.0 + w / 2.
                },
                sprite.corner.1 + h / 2.,
            );
            let Some(&[x0, y0, x1, y1]) = art.bodies.get(*kind as usize) else {
                continue;
            };
            let (x0, x1) = if flipped { (-x1, -x0) } else { (x0, x1) };
            let rect = [
                frame_center.0 + x0,
                frame_center.1 + y0,
                frame_center.0 + x1,
                frame_center.1 + y1,
            ];
            out = Some(match out {
                Some(o) => [
                    o[0].min(rect[0]),
                    o[1].min(rect[1]),
                    o[2].max(rect[2]),
                    o[3].max(rect[3]),
                ],
                None => rect,
            });
            break;
        }
    }
    out
}

/// The body of one frame from its alpha values (row-major, `w` wide):
/// opaque pixels (alpha ≥ 128) in columns at least a quarter as tall as the
/// tallest column, as (x0, y0, x1, y1) in frame pixels (end exclusive).
pub fn frame_body(alpha: &[u8], w: usize, h: usize) -> Option<[u16; 4]> {
    let opaque = |x: usize, y: usize| alpha[y * w + x] >= 128;
    let heights: Vec<usize> = (0..w)
        .map(|x| (0..h).filter(|y| opaque(x, *y)).count())
        .collect();
    let tallest = *heights.iter().max()?;
    if tallest == 0 {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for x in (0..w).filter(|x| heights[*x] * 4 >= tallest) {
        for y in (0..h).filter(|y| opaque(x, *y)) {
            x0 = x0.min(x);
            x1 = x1.max(x + 1);
            y0 = y0.min(y);
            y1 = y1.max(y + 1);
        }
    }
    (x1 > x0).then_some([x0 as u16, y0 as u16, x1 as u16, y1 as u16])
}

/// Atlas alpha channel from PNG bytes: (width, height, alpha).
fn alpha(png_bytes: &[u8]) -> Option<(usize, usize, Vec<u8>)> {
    let mut decoder = png::Decoder::new(png_bytes);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let (w, h) = (reader.info().width as usize, reader.info().height as usize);
    if w == 0 || h == 0 || w > 16384 || h > 16384 {
        return None;
    }
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).ok()?;
    let channels = info.color_type.samples();
    let alpha_at = match info.color_type {
        png::ColorType::Rgba => Some(3),
        png::ColorType::GrayscaleAlpha => Some(1),
        _ => None,
    };
    let line = info.line_size;
    let mut out = vec![255u8; w * h];
    if let Some(a) = alpha_at {
        for y in 0..h {
            for x in 0..w {
                out[y * w + x] = buffer[y * line + x * channels + a];
            }
        }
    }
    Some((w, h, out))
}

/// Build one sheet's art from its animation JSON and PNG.
pub fn art(fanim: &serde_json::Value, png_bytes: &[u8]) -> Option<Art> {
    let (aw, ah, alpha) = alpha(png_bytes)?;
    let anims = fanim.get("anims")?.as_object()?;
    let rect = |frame: &serde_json::Value| -> Option<[u16; 4]> {
        let d = frame.get("data")?;
        let n = |k| {
            d.get(k)
                .and_then(serde_json::Value::as_f64)
                .map(|v| v as u16)
        };
        Some([n("x")?, n("y")?, n("w")?, n("h")?])
    };
    // A kind is an animation-name prefix with its own standing animation
    // ("melee_blue" for "melee_blue_run"); "" is the sheet's own kind.
    const STANDING: [&str; 3] = ["idle", "run", "walk"];
    let kind_of = |name: &str| -> String {
        let mut cut = name;
        while let Some((prefix, _)) = cut.rsplit_once('_') {
            if STANDING
                .iter()
                .any(|tag| anims.contains_key(&format!("{prefix}_{tag}")))
            {
                return prefix.to_owned();
            }
            cut = prefix;
        }
        String::new()
    };
    let mut kinds: Vec<String> = Vec::new();
    let mut frames = Vec::new();
    for (name, anim) in anims {
        let kind = kind_of(name);
        let k = match kinds.iter().position(|n| *n == kind) {
            Some(k) => k,
            None => {
                kinds.push(kind);
                kinds.len() - 1
            }
        };
        for f in anim.get("frames")?.as_array()?.iter().take(512) {
            if let Some(r) = rect(f) {
                if !frames.iter().any(|(f, _)| *f == r) && k < u16::MAX as usize {
                    frames.push((r, k as u16));
                }
            }
        }
    }
    let mut bodies = Vec::new();
    for kind in &kinds {
        // Standing frames decide the body; fall back to running, then to
        // the kind's (or sheet's) first animation.
        let named = |tag: &str| {
            if kind.is_empty() {
                tag.to_owned()
            } else {
                format!("{kind}_{tag}")
            }
        };
        let standing = STANDING
            .iter()
            .find_map(|tag| anims.get(&named(tag)))
            .or_else(|| {
                anims
                    .iter()
                    .find(|(n, _)| kind_of(n) == *kind)
                    .map(|(_, a)| a)
            })?;
        bodies.push(standing_body(standing, rect, (aw, ah), &alpha)?);
    }
    Some(Art {
        atlas: (aw as u32, ah as u32),
        frames,
        bodies,
    })
}

/// Union of the bodies of one animation's frames, around each frame centre.
fn standing_body(
    anim: &serde_json::Value,
    rect: impl Fn(&serde_json::Value) -> Option<[u16; 4]>,
    (aw, ah): (usize, usize),
    alpha: &[u8],
) -> Option<[f32; 4]> {
    let mut body: Option<[f32; 4]> = None;
    for f in anim.get("frames")?.as_array()?.iter().take(512) {
        let Some([x, y, w, h]) = rect(f) else {
            continue;
        };
        let (x, y, w, h) = (x as usize, y as usize, w as usize, h as usize);
        if w == 0 || h == 0 || x + w > aw || y + h > ah {
            continue;
        }
        let cut: Vec<u8> = (0..h)
            .flat_map(|r| {
                alpha[(y + r) * aw + x..(y + r) * aw + x + w]
                    .iter()
                    .copied()
            })
            .collect();
        let Some([x0, y0, x1, y1]) = frame_body(&cut, w, h) else {
            continue;
        };
        let (cx, cy) = (w as f32 / 2., h as f32 / 2.);
        let b = [
            x0 as f32 - cx,
            y0 as f32 - cy,
            x1 as f32 - cx,
            y1 as f32 - cy,
        ];
        body = Some(match body {
            Some(o) => [
                o[0].min(b[0]),
                o[1].min(b[1]),
                o[2].max(b[2]),
                o[3].max(b[3]),
            ],
            None => b,
        });
    }
    body
}

/// A centred champion's art from an `.aseprite` file (32-bit RGBA, per
/// https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md):
/// the body of its idle frames (else run, else the first frame), with layer
/// and cel opacity applied, around the canvas centre.
pub fn aseprite_art(b: &[u8]) -> Option<Art> {
    let word = |at: usize| Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?));
    let dword = |at: usize| Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?));
    if b.len() < 128 || word(4)? != 0xa5e0 || word(12)? != 32 {
        return None;
    }
    let (count, w, h) = (word(6)? as usize, word(8)? as usize, word(10)? as usize);
    if count == 0 || count > 1024 || w == 0 || h == 0 || w > 1024 || h > 1024 {
        return None;
    }
    // Per frame: (layer, x, y, width, height, cel opacity, compressed pixels).
    type Cel<'a> = (u16, i32, i32, usize, usize, u8, &'a [u8]);
    let mut frames: Vec<Vec<Cel>> = Vec::new();
    let mut layers: Vec<u8> = Vec::new(); // opacity; 0 = hidden or unsupported
    let mut tags: Vec<(String, usize, usize)> = Vec::new();
    let mut offset = 128usize;
    for _ in 0..count {
        let length = dword(offset)? as usize;
        let end = offset.checked_add(length)?;
        if length < 16 || end > b.len() || word(offset + 4)? != 0xf1fa {
            return None;
        }
        let chunks = match dword(offset + 12)? {
            0 => word(offset + 6)? as u32,
            n => n,
        };
        let mut cels = Vec::new();
        let mut c = offset + 16;
        for _ in 0..chunks.min(4096) {
            let size = dword(c)? as usize;
            if size < 6 || c.checked_add(size)? > end {
                return None;
            }
            let data = b.get(c + 6..c + size)?;
            let dw = |at: usize| Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?));
            match word(c + 4)? {
                0x2004 if layers.len() < 256 => {
                    let normal = dw(2)? == 0 && dw(0)? & 1 != 0;
                    layers.push(if normal { *data.get(12)? } else { 0 });
                }
                0x2005 => {
                    let layer = dw(0)?;
                    let (x, y) = (dw(2)? as i16 as i32, dw(4)? as i16 as i32);
                    let opacity = *data.get(6)?;
                    match dw(7)? {
                        2 => cels.push((
                            layer,
                            x,
                            y,
                            dw(16)? as usize,
                            dw(18)? as usize,
                            opacity,
                            data.get(20..)?,
                        )),
                        1 => {
                            let linked = dw(16)? as usize;
                            if let Some(cel) = frames
                                .get(linked)
                                .and_then(|f| f.iter().find(|c| c.0 == layer))
                            {
                                cels.push((layer, x, y, cel.3, cel.4, opacity, cel.6));
                            }
                        }
                        _ => {}
                    }
                }
                0x2018 => {
                    let mut p = 10;
                    for _ in 0..dw(0)?.min(256) {
                        let (from, to) = (dw(p)? as usize, dw(p + 2)? as usize);
                        let length = dw(p + 17)? as usize;
                        let name = std::str::from_utf8(data.get(p + 19..p + 19 + length)?).ok()?;
                        tags.push((name.to_owned(), from, to.min(count - 1)));
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
    let (from, to) = ["idle", "run", "walk"]
        .iter()
        .find_map(|t| tags.iter().find(|(n, _, _)| n == t))
        .map_or((0, 0), |(_, f, t)| (*f, *t));
    let mut body: Option<[f32; 4]> = None;
    for frame in frames.get(from..=to.max(from))?.iter().take(64) {
        let mut alpha = vec![0u8; w * h];
        for &(layer, x, y, cw, ch, opacity, pixels) in frame {
            let layer_opacity = layers.get(layer as usize).copied().unwrap_or(0) as u32;
            if layer_opacity == 0 || opacity == 0 || cw * ch > 1 << 22 {
                continue;
            }
            let Ok(rgba) =
                miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(pixels, cw * ch * 4)
            else {
                continue;
            };
            for cy in 0..ch {
                for cx in 0..cw {
                    let (px, py) = (x + cx as i32, y + cy as i32);
                    if px < 0 || py < 0 || px as usize >= w || py as usize >= h {
                        continue;
                    }
                    let a = *rgba.get((cy * cw + cx) * 4 + 3).unwrap_or(&0) as u32;
                    let a = (a * opacity as u32 * layer_opacity / (255 * 255)) as u8;
                    let at = py as usize * w + px as usize;
                    alpha[at] = alpha[at].max(a);
                }
            }
        }
        let Some([x0, y0, x1, y1]) = frame_body(&alpha, w, h) else {
            continue;
        };
        let (cx, cy) = (w as f32 / 2., h as f32 / 2.);
        let r = [
            x0 as f32 - cx,
            y0 as f32 - cy,
            x1 as f32 - cx,
            y1 as f32 - cy,
        ];
        body = Some(match body {
            Some(o) => [
                o[0].min(r[0]),
                o[1].min(r[1]),
                o[2].max(r[2]),
                o[3].max(r[3]),
            ],
            None => r,
        });
    }
    Some(Art {
        atlas: (w as u32, h as u32),
        frames: Vec::new(),
        bodies: vec![body?],
    })
}

/// Unit type → the sheets that draw its body.
fn sheets_for(key: &str) -> Vec<String> {
    match key {
        "ingame/blue_tower" => vec![key.into(), "ingame/blue_tower_orb".into()],
        "ingame/blue_nexus" => vec![key.into(), "ingame/blue_nexus_orb".into()],
        _ => vec![key.into()],
    }
}

/// Where a mod champion's sprite comes from.
pub enum Source {
    /// Its own `<base>#anim.fanim` and `<base>#sheet.png`.
    Files(PathBuf),
    /// A base-game champion sheet, by file name.
    Base(String),
}
/// Load on a background thread: base sheets from the game bundle, then mod
/// champions by name (overriding a base champion of the same name).
pub fn start(game: PathBuf, mods: Vec<(String, Source)>, log: std::sync::Arc<Logger>) {
    let _ = std::thread::Builder::new()
        .name("lt-sprite-art".into())
        .spawn(move || {
            let started = std::time::Instant::now();
            let mut sheets: HashMap<String, Art> = bundle_sheets(&game.join("bundle.game_data"));
            for (name, source) in &mods {
                let built = match source {
                    Source::Files(base) => {
                        let fanim = PathBuf::from(format!("{}#anim.fanim", base.display()));
                        let png = PathBuf::from(format!("{}#sheet.png", base.display()));
                        std::fs::read(&fanim)
                            .ok()
                            .and_then(|b| serde_json::from_slice(&b).ok())
                            .zip(std::fs::read(&png).ok())
                            .and_then(|(f, p)| art(&f, &p))
                            .or_else(|| {
                                // Only the Aseprite source: the game packs it.
                                let source = base.with_extension("aseprite");
                                (std::fs::metadata(&source).ok()?.len() <= 8_000_000)
                                    .then(|| std::fs::read(&source).ok())
                                    .flatten()
                                    .and_then(|b| aseprite_art(&b))
                            })
                    }
                    Source::Base(file) => sheets.get(file).cloned(),
                };
                match built {
                    Some(a) => {
                        sheets.insert(name.clone(), a);
                    }
                    // Unreadable art must not keep an old champion's body.
                    None => {
                        sheets.remove(name);
                    }
                }
            }
            let mut keys = HashMap::new();
            let mut sets = Vec::new();
            let mut names: Vec<&String> = sheets.keys().filter(|k| !k.ends_with("_orb")).collect();
            names.sort();
            for key in names {
                let set: ArtSet = sheets_for(key)
                    .iter()
                    .filter_map(|k| sheets.get(k).cloned())
                    .collect();
                if !set.is_empty() && sets.len() < u16::MAX as usize {
                    keys.insert(key.clone(), sets.len() as u16);
                    sets.push(set);
                }
            }
            log.write(&format!(
                "SPRITE ART bodies={} sheets={} ms={}",
                sets.len(),
                sheets.len(),
                started.elapsed().as_millis()
            ));
            let _ = ARTS.set((keys, sets));
        });
}

/// Champion, in-game unit and minion sheets from the base bundle, keyed as
/// "<champion>", "ingame/<name>" or "ui/minion".
fn bundle_sheets(path: &Path) -> HashMap<String, Art> {
    let mut out = HashMap::new();
    let Ok(mut file) = std::fs::File::open(path) else {
        return out;
    };
    let word = |f: &mut std::fs::File| -> Option<u32> {
        let mut b = [0u8; 4];
        f.read_exact(&mut b).ok()?;
        Some(u32::from_le_bytes(b))
    };
    let text = |f: &mut std::fs::File, n: u32| -> Option<String> {
        let mut b = vec![0u8; n as usize];
        f.read_exact(&mut b).ok()?;
        String::from_utf8(b).ok()
    };
    let Some(count) = word(&mut file).filter(|c| *c <= 100_000) else {
        return out;
    };
    let mut fanims: HashMap<String, serde_json::Value> = HashMap::new();
    let mut pngs: HashMap<String, Vec<u8>> = HashMap::new();
    for _ in 0..count {
        let Some(extension) = word(&mut file)
            .filter(|n| *n < 64)
            .and_then(|n| text(&mut file, n))
        else {
            break;
        };
        let Some(path) = word(&mut file)
            .filter(|n| *n < 1024)
            .and_then(|n| text(&mut file, n))
        else {
            break;
        };
        let Some(size) = word(&mut file) else {
            break;
        };
        let key = path
            .strip_prefix("asset/base/aseprite_resources/champions/")
            .map(str::to_owned)
            .or_else(|| {
                path.strip_prefix("asset/base/aseprite_resources/ingame/")
                    .map(|n| format!("ingame/{n}"))
            })
            .or_else(|| {
                // Every minion kind shares one sheet.
                path.strip_prefix("asset/base/aseprite_resources/UI_aseprite/")
                    .filter(|n| n.starts_with("minion#"))
                    .map(|n| format!("ui/{n}"))
            });
        let wanted = key.as_ref().and_then(|k| {
            k.strip_suffix("#anim")
                .filter(|_| extension == "fanim")
                .map(|k| (k.to_owned(), true))
                .or_else(|| {
                    k.strip_suffix("#sheet")
                        .filter(|_| extension == "png")
                        .map(|k| (k.to_owned(), false))
                })
        });
        match wanted {
            Some((key, is_anim)) if size <= 16_000_000 => {
                let mut bytes = vec![0u8; size as usize];
                if file.read_exact(&mut bytes).is_err() {
                    break;
                }
                if is_anim {
                    if let Ok(v) = serde_json::from_slice(&bytes) {
                        fanims.insert(key, v);
                    }
                } else {
                    pngs.insert(key, bytes);
                }
            }
            _ => {
                if file.seek(SeekFrom::Current(size as i64)).is_err() {
                    break;
                }
            }
        }
    }
    for (key, fanim) in fanims {
        if let Some(a) = pngs.get(&key).and_then(|p| art(&fanim, p)) {
            out.insert(key, a);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn body_keeps_heads_and_drops_a_held_gun() {
        // 10 wide, 8 tall: a 3-wide body (columns 2..5, rows 1..8), a 1-wide
        // hat column above it (column 3, row 0), and a gun one pixel tall
        // sticking out to the right (columns 5..10, row 4).
        let (w, h) = (10, 8);
        let mut a = vec![0u8; w * h];
        for y in 1..8 {
            for x in 2..5 {
                a[y * w + x] = 255;
            }
        }
        a[3] = 255; // hat
        for x in 5..10 {
            a[4 * w + x] = 255; // gun
        }
        assert_eq!(frame_body(&a, w, h), Some([2, 0, 5, 8]));
        assert_eq!(frame_body(&[0; 4], 2, 2), None);
    }
    #[test]
    fn placement_uses_the_drawn_frame_corner_and_mirrors_when_flipped() {
        // A 20x40 frame at atlas (40, 0) in a 100x40 atlas; body 8 wide,
        // 30 tall, shifted 2 to the right of the frame centre.
        let art = Art {
            atlas: (100, 40),
            frames: vec![([40, 0, 20, 40], 0)],
            bodies: vec![[-2., -15., 6., 15.]],
        };
        let uv = [0.4, 0., 0.2, 1.];
        // Facing right: drawn from corner (90, 80), unit at (100, 100).
        let r = placed(
            &vec![art.clone()],
            (100., 100.),
            &[Drawn {
                uv,
                corner: (90., 80.),
            }],
        );
        assert_eq!(r, Some([98., 85., 106., 115.]));
        // Facing left: corner to the right of the unit; body mirrored.
        let r = placed(
            &vec![art.clone()],
            (100., 100.),
            &[Drawn {
                uv,
                corner: (110., 80.),
            }],
        );
        assert_eq!(r, Some([94., 85., 102., 115.]));
        // An unknown frame (another sheet) gives nothing.
        let other = [0.9, 0., 0.05, 1.];
        assert!(placed(
            &vec![art],
            (100., 100.),
            &[Drawn {
                uv: other,
                corner: (90., 80.)
            }]
        )
        .is_none());
    }
    #[test]
    fn each_kind_on_a_shared_sheet_gets_its_own_body() {
        // A 20x10 atlas: "small_idle" is a 4x4 block in frame (0,0,10,10),
        // "big_idle" a 10x10 block filling frame (10,0,10,10); "big_attack"
        // belongs to "big" and has no body of its own.
        let (w, h) = (20u32, 10u32);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                if (3..7).contains(&x) && (3..7).contains(&y) || x >= 10 {
                    rgba[((y * w + x) * 4 + 3) as usize] = 255;
                }
            }
        }
        let mut png_bytes = Vec::new();
        {
            let mut e = png::Encoder::new(&mut png_bytes, w, h);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(&rgba).unwrap();
        }
        let frame = |x: u16| serde_json::json!({"data": {"x": x, "y": 0, "w": 10, "h": 10}});
        let fanim = serde_json::json!({"anims": {
            "small_idle": {"frames": [frame(0)]},
            "big_idle": {"frames": [frame(10)]},
            "big_attack": {"frames": [frame(10)]},
        }});
        let art = art(&fanim, &png_bytes).unwrap();
        let body_of = |x: u16| {
            let (_, k) = art.frames.iter().find(|(f, _)| f[0] == x).unwrap();
            art.bodies[*k as usize]
        };
        assert_eq!(body_of(0), [-2., -2., 2., 2.]);
        assert_eq!(body_of(10), [-5., -5., 5., 5.]);
    }
    #[test]
    fn aseprite_bodies_read_pixels_and_place_on_the_drawn_centre() {
        // A 20x20 canvas, one visible layer, one frame tagged "idle" whose
        // cel is a 4x6 opaque block at (8, 6): body x 8..12, y 6..12.
        fn chunk(kind: u16, data: Vec<u8>) -> Vec<u8> {
            let mut b = ((data.len() + 6) as u32).to_le_bytes().to_vec();
            b.extend(kind.to_le_bytes());
            b.extend(data);
            b
        }
        let mut layer = vec![0u8; 18];
        layer[0] = 1; // visible
        layer[12] = 255; // opacity
        let mut cel = vec![0u8; 16];
        cel[2..4].copy_from_slice(&8u16.to_le_bytes());
        cel[4..6].copy_from_slice(&6u16.to_le_bytes());
        cel[6] = 255;
        cel[7..9].copy_from_slice(&2u16.to_le_bytes()); // compressed image
        cel.extend(4u16.to_le_bytes());
        cel.extend(6u16.to_le_bytes());
        cel.extend(miniz_oxide::deflate::compress_to_vec_zlib(
            &[255u8; 4 * 6 * 4],
            6,
        ));
        let mut tag = vec![0u8; 10];
        tag[0] = 1; // one tag
        let mut entry = vec![0u8; 17];
        entry.extend(4u16.to_le_bytes());
        entry.extend(b"idle");
        tag.extend(entry);
        let chunks = [chunk(0x2004, layer), chunk(0x2005, cel), chunk(0x2018, tag)].concat();
        let mut frame = ((16 + chunks.len()) as u32).to_le_bytes().to_vec();
        frame.extend(0xf1fau16.to_le_bytes());
        frame.extend(3u16.to_le_bytes());
        frame.extend([0u8; 8]);
        frame.extend(chunks);
        let mut file = vec![0u8; 128];
        file[4..6].copy_from_slice(&0xa5e0u16.to_le_bytes());
        file[6..8].copy_from_slice(&1u16.to_le_bytes());
        file[8..10].copy_from_slice(&20u16.to_le_bytes());
        file[10..12].copy_from_slice(&20u16.to_le_bytes());
        file[12..14].copy_from_slice(&32u16.to_le_bytes());
        file.extend(frame);
        let len = file.len() as u32;
        file[0..4].copy_from_slice(&len.to_le_bytes());
        let art = aseprite_art(&file).unwrap();
        assert_eq!(art.bodies, vec![[-2., -4., 2., 2.]]);
        // Drawn centred on (100, 100): facing right, then flipped.
        let at = |corner| {
            placed(
                &vec![art.clone()],
                (100., 100.),
                &[Drawn {
                    uv: [0.; 4],
                    corner,
                }],
            )
        };
        assert_eq!(at((90., 90.)), Some([98., 96., 102., 102.]));
        let wide = Art {
            bodies: vec![[-3., -4., 1., 2.]],
            ..art
        };
        let flipped = placed(
            &vec![wide],
            (100., 100.),
            &[Drawn {
                uv: [0.; 4],
                corner: (110., 90.),
            }],
        );
        assert_eq!(flipped, Some([99., 96., 103., 102.]));
    }
    #[test]
    fn installed_bodies_report_when_explicitly_requested() {
        if std::env::var_os("LT_ART_REPORT").is_none() {
            return;
        }
        let game = std::path::Path::new(
            r"C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2",
        );
        let sheets = bundle_sheets(&game.join("bundle.game_data"));
        for key in [
            "soldier",
            "lancer",
            "gunner",
            "harpy",
            "ingame/serpen",
            "ingame/epic",
            "ingame/blue_tower",
            "ingame/blue_tower_orb",
            "ingame/blue_nexus",
            "ingame/stump",
            "ui/minion",
        ] {
            match sheets.get(key) {
                Some(a) => println!(
                    "{key:22} atlas={:?} frames={} bodies={:?}",
                    a.atlas,
                    a.frames.len(),
                    a.bodies
                ),
                None => println!("{key:22} missing"),
            }
        }
        println!("total sheets {}", sheets.len());
        let leef = std::path::Path::new(
            r"C:\Program Files (x86)\Steam\steamapps\workshop\content\3009300\3798323185\aseprite_resources\champions",
        );
        for name in ["harpy", "nullifier", "candygel", "mistguide", "tale"] {
            let art = std::fs::read(leef.join(format!("{name}.aseprite")))
                .ok()
                .and_then(|b| aseprite_art(&b));
            println!("{name:22} aseprite {:?}", art.map(|a| a.bodies));
        }
    }
}
