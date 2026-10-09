//! User-owned originals, bounded static PNG normalization and restart-only assets.
//! No filesystem reads, image decoding or SDK calls occur during wheel rendering.
use crate::{emotes::CATALOGUE, settings::Values, Logger};
mod bundled;
use std::{
    collections::HashSet,
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};

const MAX_BYTES: usize = 1_048_576;
const MAX_DIMENSION: u32 = 256;
const MAX_IMPORTS: usize = 64;
const MAX_DIRECTORY_ENTRIES: usize = 1024;
const CANVAS: usize = 256;
const ART_SIZE: usize = 224;
pub const SLOT_NAMES: [&str; 5] = ["Centre", "Up", "Right", "Down", "Left"];

// Curated assets are shipped separately from the DLL. Keep filename-derived import
// identities so existing user assignments also resolve to their bundled copies.
struct Bundled {
    filename: &'static str,
    label: &'static str,
    glyph: &'static str,
    id: &'static str,
    aliases: &'static [&'static str],
    source_sha256: &'static str,
}
impl Bundled {
    fn entry(&self) -> Entry {
        Entry {
            art: Art {
                id: self.id.into(),
                label: self.label.into(),
                source: format!("asset/lt_direct_control/ui/{}", self.glyph),
            },
            ready: true,
            builtin: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Art {
    pub id: String,
    pub label: String,
    pub source: String,
}
#[derive(Clone)]
pub struct Entry {
    pub art: Art,
    pub ready: bool,
    pub builtin: bool,
}
#[derive(Clone)]
pub struct Snapshot {
    pub entries: Vec<Entry>,
    pub errors: Vec<String>,
    pub revision: u64,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            entries: CATALOGUE
                .iter()
                .map(|e| Entry {
                    art: Art {
                        id: format!("builtin:{}", e.glyph),
                        label: e.name.into(),
                        source: format!("asset/lt_direct_control/ui/{}", e.glyph),
                    },
                    ready: true,
                    builtin: true,
                })
                .chain(bundled::ENTRIES.iter().map(Bundled::entry))
                .collect(),
            errors: Vec::new(),
            revision: 0,
        }
    }
}
impl Snapshot {
    pub fn find(&self, id: &str) -> Option<&Entry> {
        let id = canonical_id(id);
        self.entries.iter().find(|e| e.art.id == id)
    }
    pub fn assigned<'a>(&'a self, values: &Values, slot: usize) -> &'a Entry {
        values
            .0
            .get("emote_slots")
            .and_then(|v| v.get(slot))
            .and_then(|v| v.as_str())
            .and_then(|id| self.find(id))
            .unwrap_or(&self.entries[slot.min(4)])
    }
    pub fn missing(&self, values: &Values, slot: usize) -> bool {
        stored_id(values, slot).is_some_and(|id| self.find(id).is_none())
    }
    pub fn playable<'a>(&'a self, values: &Values, slot: usize) -> &'a Art {
        let entry = self.assigned(values, slot);
        if entry.ready {
            &entry.art
        } else {
            &self.entries[slot.min(4)].art
        }
    }
}
fn canonical_id(id: &str) -> &str {
    bundled::ENTRIES
        .iter()
        .find(|entry| entry.aliases.contains(&id))
        .map_or(id, |entry| entry.id)
}
fn stored_id(values: &Values, slot: usize) -> Option<&str> {
    values.0.get("emote_slots")?.get(slot)?.as_str()
}
pub fn assign(values: &mut Values, slot: usize, id: &str, library: &Snapshot) {
    if slot >= 5 || library.find(id).is_none() {
        return;
    }
    let mut ids: Vec<_> = (0..5)
        // Keep missing assignments so replacing a file restores its wheel slot.
        .map(|i| {
            stored_id(values, i)
                .unwrap_or(&library.entries[i].art.id)
                .to_owned()
        })
        .collect();
    ids[slot] = id.into();
    values.0["emote_slots"] = serde_json::json!(ids);
}
pub fn reset_slots(values: &mut Values) {
    if let Some(object) = values.0.as_object_mut() {
        object.remove("emote_slots");
    }
}

pub struct Library {
    root: Option<PathBuf>,
    cache: Option<PathBuf>,
    /// Only files already staged when this DLL was initialized are eligible this run.
    loaded: HashSet<String>,
    state: Mutex<Arc<Snapshot>>,
    refreshing: AtomicBool,
    logger: Option<Arc<Logger>>,
}
static GLOBAL: OnceLock<Arc<Library>> = OnceLock::new();
static DEFAULT: OnceLock<Arc<Snapshot>> = OnceLock::new();

pub fn snapshot() -> Arc<Snapshot> {
    GLOBAL
        .get()
        .and_then(|s| s.state.lock().ok().map(|s| s.clone()))
        .unwrap_or_else(|| {
            DEFAULT
                .get_or_init(|| Arc::new(Snapshot::default()))
                .clone()
        })
}
pub fn busy() -> bool {
    GLOBAL
        .get()
        .is_some_and(|s| s.refreshing.load(Ordering::Relaxed))
}
pub fn folder() -> Option<PathBuf> {
    GLOBAL.get().and_then(|s| s.root.clone())
}
pub fn initialize(user_root: Option<&Path>, log: &Arc<Logger>) {
    let mut library = Library::new(
        user_root.map(|p| p.join("emotes")),
        module_directory().map(|p| p.join("ui/imported")),
    );
    library.logger = Some(log.clone());
    let library = Arc::new(library);
    // Startup is outside a match. A refresh from Settings uses the worker below.
    library.scan();
    let _ = GLOBAL.set(library);
}
pub fn refresh() -> bool {
    let Some(library) = GLOBAL.get() else {
        return false;
    };
    if library.refreshing.swap(true, Ordering::AcqRel) {
        return false;
    }
    let worker = library.clone();
    if std::thread::Builder::new()
        .name("lt-emote-import".into())
        .spawn(move || {
            // A malformed image is isolated from the client loop. Decoder errors are normal Results.
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| worker.scan())).is_err() {
                if let Ok(mut state) = worker.state.lock() {
                    let mut next = (**state).clone();
                    next.errors
                        .push("Import worker failed; existing library retained".into());
                    next.revision += 1;
                    *state = Arc::new(next);
                }
            }
            worker.refreshing.store(false, Ordering::Release);
        })
        .is_err()
    {
        library.refreshing.store(false, Ordering::Release);
        return false;
    }
    true
}
pub fn open_folder() -> Result<(), String> {
    let root = folder().ok_or("User emote folder is unavailable")?;
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    // Spawn Explorer directly with one path argument; never interpret filenames as shell code.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer.exe")
            .arg(&root)
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("Open folder is available on the supported Windows host".into())
    }
}
impl Library {
    fn new(root: Option<PathBuf>, cache: Option<PathBuf>) -> Self {
        let loaded = cache
            .as_ref()
            .and_then(|p| read_bounded(&p.join("catalogue.json"), 65536).ok())
            .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
            .unwrap_or_default()
            .into_iter()
            .take(MAX_IMPORTS)
            .filter(|name| cache_name(name))
            .collect();
        Self {
            root,
            cache,
            loaded,
            state: Mutex::new(Arc::new(Snapshot::default())),
            refreshing: AtomicBool::new(false),
            logger: None,
        }
    }
    fn scan(&self) {
        let mut next = Snapshot::default();
        let result = self.scan_into(&mut next);
        if let Err(error) = result {
            next.errors.push(error);
        }
        if let Some(log) = &self.logger {
            log.write(&format!(
                "EMOTE LIBRARY {} imports, {} rejected; new assets require game restart",
                next.entries.iter().filter(|e| !e.builtin).count(),
                next.errors.len()
            ));
            for error in &next.errors {
                log.write(&format!("EMOTE IMPORT rejected: {error}"));
            }
        }
        if let Ok(mut state) = self.state.lock() {
            next.revision = state.revision + 1;
            *state = Arc::new(next);
        }
    }
    fn scan_into(&self, next: &mut Snapshot) -> Result<(), String> {
        let root = self
            .root
            .as_ref()
            .ok_or("User emote folder is unavailable")?;
        let cache = self
            .cache
            .as_ref()
            .ok_or("Installed mod folder is unavailable")?;
        fs::create_dir_all(root).map_err(|e| format!("Cannot create emote folder: {e}"))?;
        fs::create_dir_all(cache)
            .map_err(|e| format!("Cannot write image cache in mod folder: {e}"))?;
        let listing = fs::read_dir(root).map_err(|e| format!("Cannot read emote folder: {e}"))?;
        let mut paths = Vec::new();
        let mut count = 0;
        for item in listing.take(MAX_DIRECTORY_ENTRIES + 1) {
            count += 1;
            if count > MAX_DIRECTORY_ENTRIES {
                next.errors.push(
                    "Folder has more than 1024 entries; only the first 1024 were scanned".into(),
                );
                break;
            }
            let Ok(item) = item else {
                next.errors.push("An emote file could not be listed".into());
                continue;
            };
            if item
                .path()
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("png"))
            {
                paths.push(item.path());
            }
        }
        paths.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
        if paths.len() > MAX_IMPORTS {
            next.errors
                .push("At most 64 PNG imports are supported; remaining files were skipped".into());
            paths.truncate(MAX_IMPORTS);
        }
        for path in paths {
            match self.import(&path, cache) {
                Ok(entry) => {
                    // A personal variant can replace a bundled image under the same
                    // filename identity. An unchanged original reuses the ready asset.
                    if let Some(existing) =
                        next.entries.iter_mut().find(|e| e.art.id == entry.art.id)
                    {
                        *existing = entry;
                    } else {
                        next.entries.push(entry);
                    }
                }
                Err(reason) => next.errors.push(format!(
                    "{}: {reason}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                )),
            }
        }
        let current: HashSet<String> = next
            .entries
            .iter()
            .filter(|e| !e.builtin)
            .filter_map(|e| e.art.source.rsplit('/').next().map(|s| format!("{s}.png")))
            .collect();
        // Never delete a texture that this running process might still display.
        for item in fs::read_dir(cache)
            .map_err(|e| e.to_string())?
            .take(1024)
            .flatten()
        {
            let name = item.file_name().to_string_lossy().into_owned();
            if cache_name(&name)
                && !current.contains(&name)
                && !self.loaded.contains(&name)
                && item.file_type().is_ok_and(|t| t.is_file())
            {
                let _ = fs::remove_file(item.path());
            }
        }
        let mut staged: Vec<_> = current.into_iter().collect();
        staged.sort();
        let bytes = serde_json::to_vec(&staged).map_err(|e| e.to_string())?;
        fs::write(cache.join("catalogue.json"), bytes)
            .map_err(|e| format!("Cannot save import catalogue: {e}"))?;
        Ok(())
    }
    fn import(&self, path: &Path, cache: &Path) -> Result<Entry, String> {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("Filename must be valid Unicode")?;
        if name.chars().count() > 68 || name.chars().any(char::is_control) {
            return Err("Filename is too long or contains control characters".into());
        }
        let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if !ordinary_file(&metadata) {
            return Err("Use a regular PNG file, not a link or folder".into());
        }
        if metadata.len() > MAX_BYTES as u64 {
            return Err("PNG exceeds 1 MiB".into());
        }
        let mut bytes = Vec::new();
        fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err("PNG exceeds 1 MiB".into());
        }
        let file_id = format!(
            "custom:{}",
            crate::native_adapter::asset_digest(name.to_lowercase().as_bytes())?
        );
        let id = canonical_id(&file_id);
        if let Some(entry) = bundled::ENTRIES
            .iter()
            .find(|e| e.filename.eq_ignore_ascii_case(name) || e.id == id)
        {
            if crate::native_adapter::asset_digest(&bytes)? == entry.source_sha256 {
                return Ok(entry.entry());
            }
        }
        let normalized = normalize(&bytes)?;
        let digest = crate::native_adapter::asset_digest(&normalized)?;
        let filename = format!("emote_{digest}.png");
        let target = cache.join(&filename);
        // Versioned content names avoid changing a texture the game may have cached.
        if fs::symlink_metadata(&target).is_ok_and(|m| !ordinary_file(&m)) {
            return Err("Image cache target is not a regular file".into());
        }
        let existing = read_bounded(&target, MAX_BYTES)
            .ok()
            .is_some_and(|b| b == normalized);
        if !existing {
            fs::write(&target, &normalized)
                .map_err(|e| format!("Cannot stage normalized image: {e}"))?;
        }
        Ok(Entry {
            art: Art {
                id: id.into(),
                label: path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                source: format!("asset/lt_direct_control/ui/imported/emote_{digest}"),
            },
            ready: existing && self.loaded.contains(&filename),
            builtin: false,
        })
    }
}
fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("File exceeds its size limit".into());
    }
    Ok(bytes)
}
fn ordinary_file(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.is_file() && metadata.file_attributes() & 0x400 == 0
    }
    #[cfg(not(windows))]
    {
        metadata.is_file() && !metadata.file_type().is_symlink()
    }
}
fn cache_name(name: &str) -> bool {
    name.strip_prefix("emote_")
        .and_then(|s| s.strip_suffix(".png"))
        .is_some_and(|s| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit()))
}
#[cfg(windows)]
fn module_directory() -> Option<PathBuf> {
    use std::{
        ffi::{c_void, OsString},
        os::windows::ffi::OsStringExt,
    };
    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleExW(flags: u32, address: *const u16, module: *mut *mut c_void) -> i32;
        fn GetModuleFileNameW(module: *mut c_void, buffer: *mut u16, size: u32) -> u32;
    }
    let mut module = std::ptr::null_mut();
    let mut buffer = vec![0u16; 32768];
    unsafe {
        if GetModuleHandleExW(6, module_directory as *const () as *const u16, &mut module) == 0 {
            return None;
        }
        let len = GetModuleFileNameW(module, buffer.as_mut_ptr(), buffer.len() as u32) as usize;
        if len == 0 || len >= buffer.len() {
            return None;
        }
        PathBuf::from(OsString::from_wide(&buffer[..len]))
            .parent()
            .map(Path::to_path_buf)
    }
}
#[cfg(not(windows))]
fn module_directory() -> Option<PathBuf> {
    None
}

/// Decode to RGBA8, trim alpha margins, then fit visible artwork into a padded canvas.
pub fn normalize(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() > MAX_BYTES {
        return Err("PNG exceeds 1 MiB".into());
    }
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits {
        bytes: 4 * 1024 * 1024,
    });
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("Invalid PNG: {e}"))?;
    let info = reader.info();
    if info.width == 0
        || info.height == 0
        || info.width > MAX_DIMENSION
        || info.height > MAX_DIMENSION
    {
        return Err("Image dimensions must be 1–256 pixels on each side".into());
    }
    if info.animation_control.is_some() {
        return Err("Animated PNG is not supported; use a static PNG".into());
    }
    let mut decoded = vec![0; reader.output_buffer_size()];
    let frame = reader
        .next_frame(&mut decoded)
        .map_err(|e| format!("Invalid PNG pixels: {e}"))?;
    let channels = match frame.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Grayscale => 1,
        _ => return Err("Unsupported PNG color type".into()),
    };
    let (w, h) = (frame.width as usize, frame.height as usize);
    let mut rgba = vec![0u8; w * h * 4];
    for (pixel, source) in rgba
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(decoded[..frame.buffer_size()].chunks_exact(channels))
    {
        match channels {
            4 => pixel.copy_from_slice(source),
            3 => {
                pixel[..3].copy_from_slice(source);
                pixel[3] = 255;
            }
            2 => {
                pixel[..3].fill(source[0]);
                pixel[3] = source[1];
            }
            _ => {
                pixel[..3].fill(source[0]);
                pixel[3] = 255;
            }
        }
    }
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if rgba[(y * w + x) * 4 + 3] != 0 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    if x0 >= x1 || y0 >= y1 {
        return Err("Image is fully transparent".into());
    }
    let (cw, ch) = (x1 - x0, y1 - y0);
    let scale = ART_SIZE as f64 / cw.max(ch) as f64;
    let (dw, dh) = (
        (cw as f64 * scale).round().max(1.) as usize,
        (ch as f64 * scale).round().max(1.) as usize,
    );
    let (ox, oy) = ((CANVAS - dw) / 2, (CANVAS - dh) / 2);
    let mut output = vec![0u8; CANVAS * CANVAS * 4];
    // Premultiplied-alpha bilinear filtering avoids dark fringes around transparent pixels.
    for y in 0..dh {
        for x in 0..dw {
            let sx = ((x as f64 + 0.5) * cw as f64 / dw as f64 - 0.5).clamp(0., (cw - 1) as f64);
            let sy = ((y as f64 + 0.5) * ch as f64 / dh as f64 - 0.5).clamp(0., (ch - 1) as f64);
            let (ix, iy) = (sx.floor() as usize, sy.floor() as usize);
            let (fx, fy) = (sx.fract(), sy.fract());
            let mut color = [0.; 4];
            for (px, py, weight) in [
                (ix, iy, (1. - fx) * (1. - fy)),
                ((ix + 1).min(cw - 1), iy, fx * (1. - fy)),
                (ix, (iy + 1).min(ch - 1), (1. - fx) * fy),
                ((ix + 1).min(cw - 1), (iy + 1).min(ch - 1), fx * fy),
            ] {
                let src = &rgba[((y0 + py) * w + x0 + px) * 4..][..4];
                let alpha = src[3] as f64 / 255.;
                color[3] += alpha * weight;
                for c in 0..3 {
                    color[c] += src[c] as f64 * alpha * weight;
                }
            }
            let dst = &mut output[((oy + y) * CANVAS + ox + x) * 4..][..4];
            if color[3] > 0. {
                for c in 0..3 {
                    dst[c] = (color[c] / color[3]).round().clamp(0., 255.) as u8;
                }
                dst[3] = (color[3] * 255.).round() as u8;
            }
        }
    }
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, CANVAS as u32, CANVAS as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer
            .write_image_data(&output)
            .map_err(|e| e.to_string())?;
    }
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn png(w: u32, h: u32, kind: png::ColorType, pixels: &[u8], animated: bool) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut e = png::Encoder::new(&mut bytes, w, h);
            e.set_color(kind);
            e.set_depth(png::BitDepth::Eight);
            if animated {
                e.set_animated(1, 0).unwrap();
            }
            e.write_header().unwrap().write_image_data(pixels).unwrap();
        }
        bytes
    }
    fn decode(bytes: &[u8]) -> Vec<u8> {
        let mut reader = png::Decoder::new(Cursor::new(bytes)).read_info().unwrap();
        assert_eq!((reader.info().width, reader.info().height), (256, 256));
        let mut pixels = vec![0; reader.output_buffer_size()];
        reader.next_frame(&mut pixels).unwrap();
        pixels
    }
    fn bounds(pixels: &[u8]) -> (usize, usize, usize, usize) {
        let (mut x0, mut y0, mut x1, mut y1) = (256, 256, 0, 0);
        for y in 0..256 {
            for x in 0..256 {
                if pixels[(y * 256 + x) * 4 + 3] > 0 {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x + 1);
                    y1 = y1.max(y + 1);
                }
            }
        }
        (x0, y0, x1, y1)
    }
    fn solid(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        png(
            w,
            h,
            png::ColorType::Rgba,
            &rgba.repeat((w * h) as usize),
            false,
        )
    }
    #[test]
    fn transparent_margins_do_not_change_visible_size_or_cache_content() {
        let tight = solid(20, 10, [220, 50, 30, 255]);
        let mut padded = vec![0; 128 * 128 * 4];
        for y in 41..51 {
            for x in 69..89 {
                padded[(y * 128 + x) * 4..][..4].copy_from_slice(&[220, 50, 30, 255]);
            }
        }
        let padded = png(128, 128, png::ColorType::Rgba, &padded, false);
        let a = normalize(&tight).unwrap();
        assert_eq!(a, normalize(&padded).unwrap());
        assert_eq!(bounds(&decode(&a)), (16, 72, 240, 184));
    }
    #[test]
    fn tall_art_preserves_aspect_and_centres_on_transparent_canvas() {
        let p = decode(&normalize(&solid(8, 32, [10, 20, 30, 255])).unwrap());
        assert_eq!(bounds(&p), (100, 16, 156, 240));
        assert_eq!(&p[..4], &[0, 0, 0, 0]);
    }
    #[test]
    fn transparent_colours_do_not_bleed_into_filtered_edges() {
        let source = png(
            3,
            1,
            png::ColorType::Rgba,
            &[255, 0, 0, 255, 0, 0, 255, 0, 255, 0, 0, 255],
            false,
        );
        let p = decode(&normalize(&source).unwrap());
        assert!(p.as_chunks::<4>().0.iter().any(|v| v[3] > 0 && v[3] < 255));
        for v in p.as_chunks::<4>().0.iter().filter(|v| v[3] > 0) {
            assert_eq!(&v[..3], &[255, 0, 0]);
        }
    }
    #[test]
    fn rgb_gray_and_gray_alpha_are_decoded_to_rgba() {
        for (kind, data, expected) in [
            (png::ColorType::Rgb, vec![20, 40, 60], [20, 40, 60, 255]),
            (png::ColorType::Grayscale, vec![90], [90, 90, 90, 255]),
            (
                png::ColorType::GrayscaleAlpha,
                vec![70, 128],
                [70, 70, 70, 128],
            ),
        ] {
            let p = decode(&normalize(&png(1, 1, kind, &data, false)).unwrap());
            assert_eq!(&p[(128 * 256 + 128) * 4..][..4], &expected);
        }
    }
    #[test]
    fn invalid_oversize_empty_and_animated_images_are_rejected() {
        assert!(normalize(b"not png").unwrap_err().contains("Invalid PNG"));
        assert!(normalize(&vec![0; MAX_BYTES + 1])
            .unwrap_err()
            .contains("1 MiB"));
        assert!(normalize(&solid(257, 1, [1, 2, 3, 255]))
            .unwrap_err()
            .contains("dimensions"));
        assert!(normalize(&solid(10, 10, [0, 0, 0, 0]))
            .unwrap_err()
            .contains("transparent"));
        assert!(
            normalize(&png(1, 1, png::ColorType::Rgba, &[1, 2, 3, 255], true))
                .unwrap_err()
                .contains("Animated")
        );
    }
    #[test]
    fn assignment_keeps_missing_ids_and_uses_safe_fallbacks() {
        let library = Snapshot::default();
        let mut v = Values::default();
        v.0["emote_slots"] = serde_json::json!(["custom:missing", null, 42]);
        assert!(library.missing(&v, 0));
        assert_eq!(library.playable(&v, 0), &library.entries[0].art);
        assign(&mut v, 2, &library.entries[4].art.id, &library);
        assert_eq!(stored_id(&v, 0), Some("custom:missing"));
        assert_eq!(library.assigned(&v, 2).art.id, library.entries[4].art.id);
        let before = v.0.clone();
        assign(&mut v, 99, "invalid", &library);
        assign(&mut v, 1, "invalid", &library);
        assert_eq!(v.0, before);
        reset_slots(&mut v);
        assert!(!library.missing(&v, 0));
        assert!(v.0.get("emote_slots").is_none());
    }
    struct Folder(PathBuf);
    impl Folder {
        fn new(label: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join(format!(
                    "emote-test-{label}-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
            fs::create_dir_all(path.join("originals")).unwrap();
            fs::create_dir_all(path.join("cache")).unwrap();
            Self(path)
        }
        fn library(&self) -> Library {
            Library::new(Some(self.0.join("originals")), Some(self.0.join("cache")))
        }
        fn save(&self, name: &str, bytes: &[u8]) {
            fs::write(self.0.join("originals").join(name), bytes).unwrap();
        }
        fn state(library: &Library) -> Arc<Snapshot> {
            library.state.lock().unwrap().clone()
        }
    }
    impl Drop for Folder {
        fn drop(&mut self) {
            let base = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .canonicalize()
                .unwrap();
            if self.0.canonicalize().is_ok_and(|p| p.starts_with(base)) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
    }
    #[cfg(windows)]
    #[test]
    fn imports_need_restart_updates_keep_id_and_old_loaded_textures() {
        let f = Folder::new("lifecycle");
        let base = Snapshot::default().entries.len();
        let original = solid(12, 8, [1, 2, 3, 255]);
        f.save("你好.PNG", &original);
        let first = f.library();
        first.scan();
        let s = Folder::state(&first);
        assert_eq!(s.entries.len(), base + 1);
        assert!(!s.entries[base].ready);
        assert_eq!(fs::read(f.0.join("originals/你好.PNG")).unwrap(), original);
        first.scan();
        assert!(!Folder::state(&first).entries[base].ready);
        let running = f.library();
        running.scan();
        let s = Folder::state(&running);
        assert!(s.entries[base].ready);
        let old = s.entries[base].art.clone();
        f.save("你好.PNG", &solid(12, 8, [90, 20, 30, 255]));
        running.scan();
        let s = Folder::state(&running);
        assert!(!s.entries[base].ready);
        assert_eq!(s.entries[base].art.id, old.id);
        assert_ne!(s.entries[base].art.source, old.source);
        let old_file =
            f.0.join("cache")
                .join(format!("{}.png", old.source.rsplit('/').next().unwrap()));
        assert!(old_file.exists(), "live textures remain until a later boot");
        let reboot = f.library();
        reboot.scan();
        assert!(Folder::state(&reboot).entries[base].ready);
        assert!(
            !old_file.exists(),
            "unreferenced old cache is cleaned after restart"
        );
        fs::remove_file(f.0.join("originals/你好.PNG")).unwrap();
        reboot.scan();
        assert_eq!(Folder::state(&reboot).entries.len(), base);
        let mut v = Values::default();
        v.0["emote_slots"] = serde_json::json!([old.id]);
        assert!(Folder::state(&reboot).missing(&v, 0));
        f.save("你好.PNG", &original);
        reboot.scan();
        assert!(!Folder::state(&reboot).missing(&v, 0));
    }
    #[cfg(windows)]
    #[test]
    fn bundled_assignments_survive_personal_variants_without_duplicates() {
        let f = Folder::new("bundled-variant");
        let bundled = &bundled::ENTRIES[0];
        assert_eq!(
            bundled.id,
            format!(
                "custom:{}",
                crate::native_adapter::asset_digest(bundled.filename.to_lowercase().as_bytes())
                    .unwrap()
            )
        );
        let mut values = Values::default();
        values.0["emote_slots"] =
            serde_json::json!([bundled.aliases.first().copied().unwrap_or(bundled.id)]);
        let library = f.library();
        library.scan();
        let initial = Folder::state(&library);
        let count = initial.entries.len();
        assert_eq!(count, CATALOGUE.len() + 4);
        assert!(initial.assigned(&values, 0).ready);
        assert_eq!(initial.assigned(&values, 0).art, bundled.entry().art);
        f.save(bundled.filename, &solid(1, 1, [1, 2, 3, 255]));
        library.scan();
        let variant = Folder::state(&library);
        assert_eq!(variant.entries.len(), count);
        assert!(!variant.assigned(&values, 0).builtin);
        assert!(!variant.assigned(&values, 0).ready);
        fs::remove_file(f.0.join("originals").join(bundled.filename)).unwrap();
        library.scan();
        let restored = Folder::state(&library);
        assert_eq!(restored.entries.len(), count);
        assert_eq!(restored.assigned(&values, 0).art, bundled.entry().art);
        assert!(restored.assigned(&values, 0).ready);
    }
    #[cfg(windows)]
    #[test]
    fn old_clown_filename_reuses_the_canonical_entry() {
        let f = Folder::new("old-clown-name");
        f.save("Clown_Son_0.2.png", &solid(1, 1, [30, 60, 90, 255]));
        let library = f.library();
        library.scan();
        let state = Folder::state(&library);
        let entry = &bundled::ENTRIES[1];
        assert_eq!(
            state.entries.len(),
            CATALOGUE.len() + bundled::ENTRIES.len()
        );
        let legacy = state.find(entry.aliases[0]).unwrap();
        assert_eq!(legacy.art.id, entry.id);
        assert!(!legacy.builtin);
    }
    #[cfg(windows)]
    #[test]
    fn rejected_files_are_reported_and_import_count_and_cache_cleanup_are_bounded() {
        let f = Folder::new("limits");
        f.save("00-invalid.png", b"not png");
        fs::create_dir(f.0.join("originals/01-folder.png")).unwrap();
        for i in 0..65 {
            f.save(&format!("image{i:02}.png"), &solid(1, 1, [i, 0, 0, 255]));
        }
        let unrelated = f.0.join("cache/user-file.png");
        fs::write(&unrelated, b"keep").unwrap();
        let owned =
            f.0.join("cache")
                .join(format!("emote_{}.png", "f".repeat(64)));
        fs::write(&owned, b"old").unwrap();
        let library = f.library();
        library.scan();
        let s = Folder::state(&library);
        assert!(s.entries.len() <= MAX_IMPORTS + Snapshot::default().entries.len());
        assert!(s.errors.iter().any(|e| e.contains("At most 64")));
        assert!(s
            .errors
            .iter()
            .any(|e| e.contains("00-invalid.png: Invalid PNG")));
        assert!(s.errors.iter().any(|e| e.contains("regular PNG")));
        assert!(unrelated.exists());
        assert!(!owned.exists());
        assert!(!cache_name("../emote_foo.png"));
    }
    #[test]
    fn unavailable_folders_leave_builtins_usable() {
        let library = Library::new(None, None);
        library.scan();
        let s = Folder::state(&library);
        assert_eq!(s.entries.len(), CATALOGUE.len() + bundled::ENTRIES.len());
        assert_eq!(s.errors.len(), 1);
        assert!(s.entries.iter().all(|e| e.ready && e.builtin));
    }
}
