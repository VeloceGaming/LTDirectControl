//! Hover/target sprite outlines: the native render-command hook, extra
//! outline passes and the death greyscale shader hook.
use super::*;

pub(crate) static ORIGINAL_SHADER: AtomicUsize = AtomicUsize::new(0);
pub(crate) static ORIGINAL_OUTLINE: AtomicUsize = AtomicUsize::new(0);
pub(crate) static OUTLINE_READY: AtomicBool = AtomicBool::new(false);
pub(crate) static OUTLINE_TARGETS: std::sync::Mutex<OutlineTargets> =
    std::sync::Mutex::new(OutlineTargets {
        hover: None,
        attack: None,
        click: None,
    });
pub(crate) static OUTLINE_MATCHES: AtomicUsize = AtomicUsize::new(0);
pub(crate) static OUTLINE_HOVER_DRAWS: AtomicUsize = AtomicUsize::new(0);
pub(crate) static OUTLINE_TARGET_DRAWS: AtomicUsize = AtomicUsize::new(0);
pub(crate) static OUTLINE_CLICK_DRAWS: AtomicUsize = AtomicUsize::new(0);
pub(crate) static OUTLINE_ID_MISSES: AtomicUsize = AtomicUsize::new(0);
pub(crate) static OUTLINE_UNSUPPORTED: AtomicUsize = AtomicUsize::new(0);
pub(crate) static OUTLINE_MIXED: AtomicUsize = AtomicUsize::new(0);
pub(crate) static OUTLINE_BAD_VEC: AtomicUsize = AtomicUsize::new(0);
pub(crate) static OUTLINE_NO_SPRITE: AtomicUsize = AtomicUsize::new(0);
pub(crate) static OUTLINE_TAG_COUNTS: [AtomicUsize; 20] = [const { AtomicUsize::new(0) }; 20];
pub(crate) static OUTLINE_UNKNOWN_TAG: AtomicUsize = AtomicUsize::new(usize::MAX);
pub(crate) fn clear_outline_targets() {
    if let Ok(mut targets) = OUTLINE_TARGETS.lock() {
        *targets = OutlineTargets::default();
    }
}
pub(crate) fn set_outline_targets(targets: OutlineTargets) {
    // Publish both identities, positions and teams together with no
    // intermediate empty target. Rendering copies and releases the lock
    // before calling any native renderer or command setter.
    if let Ok(mut current) = OUTLINE_TARGETS.lock() {
        *current = targets;
    }
}
pub(crate) fn outline_status(logger: &Logger) {
    let tags = OUTLINE_TAG_COUNTS
        .iter()
        .enumerate()
        .filter_map(|(tag, counter)| {
            let count = counter.swap(0, Ordering::Relaxed);
            (count != 0).then(|| {
                if tag == 19 {
                    format!(
                        "Unknown({:#x}):{count}",
                        OUTLINE_UNKNOWN_TAG.load(Ordering::Relaxed)
                    )
                } else {
                    format!("{}:{count}", outline_command_name(tag))
                }
            })
        })
        .collect::<Vec<_>>()
        .join(",");
    logger.write(&format!(
        "OUTLINE status={} hover_offset=1.2 drawn={} hover_drawn={} target_drawn={} click_drawn={} near_id_misses={} unsupported={} mixed={} bad_vec={} no_sprite={} commands=[{}]",
        OUTLINE_READY.load(Ordering::Relaxed),
        OUTLINE_MATCHES.swap(0, Ordering::Relaxed),
        OUTLINE_HOVER_DRAWS.swap(0, Ordering::Relaxed),
        OUTLINE_TARGET_DRAWS.swap(0, Ordering::Relaxed),
        OUTLINE_CLICK_DRAWS.swap(0, Ordering::Relaxed),
        OUTLINE_ID_MISSES.swap(0, Ordering::Relaxed),
        OUTLINE_UNSUPPORTED.swap(0, Ordering::Relaxed),
        OUTLINE_MIXED.swap(0, Ordering::Relaxed),
        OUTLINE_BAD_VEC.swap(0, Ordering::Relaxed),
        OUTLINE_NO_SPRITE.swap(0, Ordering::Relaxed),
        tags,
    ));
}
pub(crate) type ShaderFn = unsafe extern "system" fn(usize, usize, *const u8, usize) -> usize;
pub(crate) type OutlineRenderFn =
    unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, usize, usize) -> usize;
pub(crate) type FloatParamFn =
    unsafe extern "system" fn(usize, usize, *const u8, usize, f32) -> usize;
pub(crate) type ColorParamFn =
    unsafe extern "system" fn(usize, usize, *const u8, usize, usize) -> usize;

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub(crate) struct NativeCommandVec {
    pub(crate) capacity: usize,
    pub(crate) pointer: usize,
    pub(crate) length: usize,
}
// Source type and 0xd0-byte stride are checked against the game's own
// Vec grow sites. The alignment also satisfies the temporary command ABI.
#[repr(C, align(16))]
pub(crate) struct NativeCommand(pub(crate) [u8; 0xd0]);
pub(crate) const COMMAND_SIZE: usize = std::mem::size_of::<NativeCommand>();
pub(crate) const MAX_BODY_COMMANDS: usize = 64;
pub(crate) type DropCommandFn = unsafe extern "system" fn(usize);

pub(crate) fn outline_command_name(tag: usize) -> &'static str {
    [
        "SetCamera",
        "SetImageScale",
        "Svg",
        "Sprite",
        "NinePatch",
        "Mesh",
        "SpriteInstance",
        "Text",
        "DrawLine",
        "DrawLineEx",
        "RoundingBox",
        "FogOverlay",
        "Circle",
        "AnnularSector",
        "FilledPath",
        "StartMaskingLayer",
        "ApplyMaskingLayer",
        "AdjustCanvas",
        "RestoreCanvas",
    ]
    .get(tag)
    .copied()
    .unwrap_or("Unknown")
}

pub(crate) unsafe fn command_tag(command: *const u8) -> u64 {
    let raw = std::ptr::read_unaligned(command as *const u64);
    if raw & (1 << 63) != 0 {
        raw ^ (1 << 63)
    } else {
        7
    }
}
pub(crate) unsafe fn valid_command_vec(vector: NativeCommandVec) -> bool {
    vector.length <= MAX_BODY_COMMANDS
        && vector.capacity >= vector.length
        && vector.capacity <= 128
        && (vector.capacity == 0 || (vector.pointer >= 0x10000 && vector.pointer.is_multiple_of(8)))
}
pub(crate) unsafe fn supported_outline_commands(vector: NativeCommandVec) -> bool {
    (0..vector.length).all(|i| command_tag((vector.pointer + i * COMMAND_SIZE) as *const u8) <= 18)
}
pub(crate) unsafe fn record_outline_commands(vector: NativeCommandVec) {
    for i in 0..vector.length {
        let tag = command_tag((vector.pointer + i * COMMAND_SIZE) as *const u8) as usize;
        if tag > 18 {
            OUTLINE_UNKNOWN_TAG.store(tag, Ordering::Relaxed);
        }
        OUTLINE_TAG_COUNTS[tag.min(19)].fetch_add(1, Ordering::Relaxed);
    }
}
pub(crate) unsafe fn sprite_count(vector: NativeCommandVec) -> usize {
    (0..vector.length)
        .filter(|i| command_tag((vector.pointer + i * COMMAND_SIZE) as *const u8) == 3)
        .count()
}
/// Consume an independently rendered pass: move sprites to the supplied
/// destination and drop every other known command through the game. With
/// no destination, drop all commands for a normal-drawing fallback.
/// Unknown tags are never passed to a destructor that may interpret them
/// incorrectly. The caller disables the prototype on such a layout fault.
pub(crate) unsafe fn take_outline_commands(
    vector: NativeCommandVec,
    destination: Option<*mut u8>,
    drop_command: DropCommandFn,
) -> usize {
    let mut moved = 0;
    for i in 0..vector.length {
        let source = (vector.pointer + i * COMMAND_SIZE) as *const u8;
        let tag = command_tag(source);
        if let Some(destination) = destination.filter(|_| tag == 3) {
            std::ptr::copy_nonoverlapping(
                source,
                destination.add(moved * COMMAND_SIZE),
                COMMAND_SIZE,
            );
            moved += 1;
        } else if tag <= 18 {
            drop_command(source as usize);
        }
    }
    moved
}
pub(crate) unsafe fn free_command_buffer(vector: &mut NativeCommandVec, heap: *mut c_void) {
    if vector.capacity != 0 && HeapFree(heap, 0, vector.pointer as *mut c_void) == 0 {
        OUTLINE_READY.store(false, Ordering::Release);
    }
    *vector = NativeCommandVec::default();
}
pub(crate) unsafe fn discard_extra_passes(
    copies: &mut [NativeCommandVec],
    heap: *mut c_void,
    drop_command: DropCommandFn,
) {
    for copy in copies {
        // A malformed native Vec cannot be read or freed safely. Other
        // independently produced, valid passes are still released.
        if valid_command_vec(*copy) {
            take_outline_commands(*copy, None, drop_command);
            free_command_buffer(copy, heap);
        }
    }
}
pub(crate) unsafe fn rewrite_command(
    command: *mut u8,
    shader: &[u8],
    float_parameters: &[(&[u8], f32)],
    color: Option<[f32; 4]>,
) {
    let base = GetModuleHandleW(std::ptr::null()) as usize;
    let shader_fn: ShaderFn = std::mem::transmute(base + 0x1c91f0);
    let float_fn: FloatParamFn = std::mem::transmute(base + 0x21725a0);
    let color_fn: ColorParamFn = std::mem::transmute(base + 0x2171ae0);
    // Each setter consumes an owned command and writes its replacement.
    // Copying bytes to a temporary is a move here: the former source slot
    // is immediately overwritten and the temporary is never dropped.
    let mut temporary = std::mem::MaybeUninit::<NativeCommand>::uninit();
    let mut apply = |setter: unsafe extern "system" fn(usize, usize, *const u8, usize) -> usize,
                     name: &[u8]| {
        std::ptr::copy_nonoverlapping(command, temporary.as_mut_ptr() as *mut u8, COMMAND_SIZE);
        setter(
            command as usize,
            temporary.as_ptr() as usize,
            name.as_ptr(),
            name.len(),
        );
    };
    apply(shader_fn, shader);
    for (key, value) in float_parameters {
        std::ptr::copy_nonoverlapping(command, temporary.as_mut_ptr() as *mut u8, COMMAND_SIZE);
        float_fn(
            command as usize,
            temporary.as_ptr() as usize,
            key.as_ptr(),
            key.len(),
            *value,
        );
    }
    if let Some(color) = color {
        std::ptr::copy_nonoverlapping(command, temporary.as_mut_ptr() as *mut u8, COMMAND_SIZE);
        color_fn(
            command as usize,
            temporary.as_ptr() as usize,
            b"flash_color".as_ptr(),
            11,
            color.as_ptr() as usize,
        );
    }
}
pub(crate) unsafe fn restyle_outline(
    vector: NativeCommandVec,
    pass: usize,
    weight: f32,
    color: [f32; 4],
) {
    for i in 0..vector.length {
        let command = (vector.pointer + i * COMMAND_SIZE) as *mut u8;
        if command_tag(command) != 3 {
            continue;
        }
        rewrite_command(
            command,
            b"asset/base/shader/flash",
            &[(b"flash", 1.)],
            Some(color),
        );
        let (dx, dy) = [(-weight, 0.), (weight, 0.), (0., -weight), (0., weight)][pass];
        let x = command.add(0x78) as *mut f32;
        let y = command.add(0x7c) as *mut f32;
        *x += dx;
        *y += dy;
    }
}
pub(crate) unsafe extern "system" fn outline_hook(
    out: usize,
    view: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
    a7: usize,
    a8: usize,
) -> usize {
    let _profile = crate::perf::work(crate::perf::Work::OutlineInclusive);
    let original: OutlineRenderFn = std::mem::transmute(ORIGINAL_OUTLINE.load(Ordering::Acquire));
    let result = original(out, view, a3, a4, a5, a6, a7, a8);
    crate::perf::hook(crate::perf::Hook::Outline);
    if OUTLINE_READY.load(Ordering::Acquire) {
        let _ = catch_unwind(AssertUnwindSafe(|| record_draw(out, view)));
    }
    if !OUTLINE_READY.load(Ordering::Acquire) || view < 0x10000 {
        return result;
    }
    let Ok(targets) = OUTLINE_TARGETS.lock().map(|t| *t) else {
        return result;
    };
    if targets.hover.is_none() && targets.attack.is_none() {
        return result;
    }
    // 0.6.3's EntityView shifted 0x18 bytes from the preserved type map:
    // its own id is +0x100, while the hash-map key at view-8 is only valid
    // for the map-owned instance, not every champion-specific view copy.
    let x = std::ptr::read_unaligned((view + 0x17c) as *const f32);
    let y = std::ptr::read_unaligned((view + 0x180) as *const f32);
    let near = |unit: crate::combat::Unit| {
        x.is_finite()
            && y.is_finite()
            && (x - unit.position.0 as f32 / 1000.).abs() < 8.
            && (y - unit.position.1 as f32 / 1000.).abs() < 8.
    };
    if ![targets.hover, targets.attack]
        .into_iter()
        .flatten()
        .any(near)
    {
        return result;
    }
    let id = std::ptr::read_unaligned((view + 0x100) as *const usize);
    let Some((unit, role, weight)) = targets.for_unit(id, std::time::Instant::now()) else {
        OUTLINE_ID_MISSES.fetch_add(1, Ordering::Relaxed);
        return result;
    };
    if !near(unit) {
        return result;
    }
    let normal = std::ptr::read_unaligned(out as *const NativeCommandVec);
    if !valid_command_vec(normal) {
        OUTLINE_BAD_VEC.fetch_add(1, Ordering::Relaxed);
        OUTLINE_UNSUPPORTED.fetch_add(1, Ordering::Relaxed);
        return result;
    }
    record_outline_commands(normal);
    if !supported_outline_commands(normal) {
        OUTLINE_UNSUPPORTED.fetch_add(1, Ordering::Relaxed);
        return result;
    }
    let sprites = sprite_count(normal);
    if sprites == 0 {
        OUTLINE_NO_SPRITE.fetch_add(1, Ordering::Relaxed);
        return result;
    }
    if sprites < normal.length {
        OUTLINE_MIXED.fetch_add(1, Ordering::Relaxed);
    }
    let base = GetModuleHandleW(std::ptr::null()) as usize;
    let drop_command: DropCommandFn = std::mem::transmute(base + OUTLINE_DROP_COMMAND);
    let rgba = crate::combat::hover_color(unit);
    let color = [24, 16, 8, 0].map(|shift| ((rgba >> shift) & 255) as f32 / 255.);
    let count = 4;
    // Reserve for the bounded maximum BEFORE producing any owned extra
    // commands. No allocation/reallocation can then strand a valid pass.
    let capacity = normal.length + MAX_BODY_COMMANDS * count;
    let expected_bytes = capacity * COMMAND_SIZE;
    let heap = GetProcessHeap();
    let merged = HeapAlloc(heap, 0, expected_bytes) as *mut u8;
    if merged.is_null() {
        return result;
    }
    let mut copies = [NativeCommandVec::default(); 4];
    let mut failed = false;
    for copy in &mut copies {
        original(copy as *mut _ as usize, view, a3, a4, a5, a6, a7, a8);
        if !valid_command_vec(*copy) {
            OUTLINE_BAD_VEC.fetch_add(1, Ordering::Relaxed);
            OUTLINE_READY.store(false, Ordering::Release);
            failed = true;
            break;
        }
        if !supported_outline_commands(*copy) {
            record_outline_commands(*copy);
            OUTLINE_UNSUPPORTED.fetch_add(1, Ordering::Relaxed);
            OUTLINE_READY.store(false, Ordering::Release);
            failed = true;
            break;
        }
        if sprite_count(*copy) == 0 {
            OUTLINE_NO_SPRITE.fetch_add(1, Ordering::Relaxed);
            failed = true;
            break;
        }
    }
    if failed {
        // Original vector and commands have not been moved or altered.
        // Release independently owned extras instead of drawing duplicates.
        discard_extra_passes(&mut copies, heap, drop_command);
        HeapFree(heap, 0, merged as *mut c_void);
        return result;
    }
    let mut offset = 0;
    for (pass, copy) in copies.iter_mut().enumerate() {
        restyle_outline(*copy, pass, weight, color);
        offset +=
            take_outline_commands(*copy, Some(merged.add(offset * COMMAND_SIZE)), drop_command);
        free_command_buffer(copy, heap);
    }
    std::ptr::copy_nonoverlapping(
        normal.pointer as *const u8,
        merged.add(offset * COMMAND_SIZE),
        normal.length * COMMAND_SIZE,
    );
    if normal.capacity != 0 && HeapFree(heap, 0, normal.pointer as *mut c_void) == 0 {
        OUTLINE_READY.store(false, Ordering::Release);
    }
    std::ptr::write_unaligned(
        out as *mut NativeCommandVec,
        NativeCommandVec {
            capacity,
            pointer: merged as usize,
            length: offset + normal.length,
        },
    );
    OUTLINE_MATCHES.fetch_add(1, Ordering::Relaxed);
    match role {
        OutlineRole::Click => &OUTLINE_CLICK_DRAWS,
        OutlineRole::Hover => &OUTLINE_HOVER_DRAWS,
        OutlineRole::Attack => &OUTLINE_TARGET_DRAWS,
    }
    .fetch_add(1, Ordering::Relaxed);
    result
}
pub(crate) unsafe extern "system" fn shader_hook(
    out: usize,
    command: usize,
    name: *const u8,
    len: usize,
) -> usize {
    let original: ShaderFn = std::mem::transmute(ORIGINAL_SHADER.load(Ordering::Acquire));
    let grey = b"asset/base/shader/greyscale";
    if DEATH_GREYSCALE.load(Ordering::Acquire) && PATCHES.get().is_some() {
        original(out, command, grey.as_ptr(), grey.len())
    } else {
        original(out, command, name, len)
    }
}

/// Each unit's latest drawn placement for selection (crate::sprite_art):
/// drawn centre and up to four body sprites. Written on the render thread.
pub(crate) type DrawRecord = (
    usize,
    (f32, f32),
    Vec<crate::sprite_art::Drawn>,
    std::time::Instant,
);
pub(crate) static SPRITE_DRAWS: std::sync::Mutex<Vec<DrawRecord>> =
    std::sync::Mutex::new(Vec::new());
unsafe fn record_draw(out: usize, view: usize) {
    if view < 0x10000 {
        return;
    }
    let commands = std::ptr::read_unaligned(out as *const NativeCommandVec);
    if !valid_command_vec(commands) {
        return;
    }
    let f32_at = |a: usize| std::ptr::read_unaligned(a as *const f32);
    let sprites: Vec<crate::sprite_art::Drawn> = (0..commands.length)
        .map(|i| commands.pointer + i * COMMAND_SIZE)
        .filter(|c| command_tag(*c as *const u8) == 3)
        .take(4)
        .map(|c| crate::sprite_art::Drawn {
            uv: [
                f32_at(c + 0x68),
                f32_at(c + 0x6c),
                f32_at(c + 0x70),
                f32_at(c + 0x74),
            ],
            corner: (f32_at(c + 0x78), f32_at(c + 0x7c)),
        })
        .filter(|d| {
            d.uv.iter()
                .chain([d.corner.0, d.corner.1].iter())
                .all(|v| v.is_finite())
        })
        .collect();
    let id = std::ptr::read_unaligned((view + 0x100) as *const usize);
    let center = (f32_at(view + 0x17c), f32_at(view + 0x180));
    if sprites.is_empty() || !center.0.is_finite() || !center.1.is_finite() {
        return;
    }
    if let Ok(mut draws) = SPRITE_DRAWS.lock() {
        let entry = (id, center, sprites, std::time::Instant::now());
        if let Some(slot) = draws.iter_mut().find(|d| d.0 == id) {
            *slot = entry;
        } else if draws.len() < 512 {
            draws.push(entry);
        } else if let Some(oldest) = draws.iter_mut().min_by_key(|d| d.3) {
            *oldest = entry;
        }
    }
}
