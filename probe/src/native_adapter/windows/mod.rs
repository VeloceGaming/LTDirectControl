//! Native hooks for the one fingerprinted 0.6.3 executable: patch
//! installation and shared helpers. Hook families live in the child modules.
use super::*;
use crate::native_profile::{EXPECTED_SHA, IMAGE_SIZE, PE_TIMESTAMP};
use std::ffi::{c_void, OsString};
use std::os::windows::ffi::OsStringExt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// One file per hook family; `layout` holds every 0.6.3 address.
mod combat;
mod input;
mod layout;
mod movement;
mod outline;
mod shop;
#[cfg(test)]
mod tests;
mod tooltips;
mod view;
pub(super) use combat::attack_ranges;
use combat::*;
use input::*;
use layout::*;
use movement::*;
use outline::*;
// Used by the public layer above; explicit, so they win over its own
// same-named wrappers that `use super::*` brings in.
pub(super) use outline::SPRITE_DRAWS;
pub(super) use outline::{clear_outline_targets, outline_status, set_outline_targets};
use shop::*;
pub(super) use shop::{
    buyer_anchor_report, item_slot_capacity, native_player, player_build, player_owned_len,
    shop_ready,
};
pub(super) use view::reset_session;
use view::*;

pub(super) fn verified_base() -> Option<usize> {
    PATCHES.get()?;
    let base = unsafe { GetModuleHandleW(std::ptr::null()) } as usize;
    (base != 0).then_some(base)
}

struct PatchRecord {
    worker: usize,
    viewer: usize,
    worker_bytes: [u8; 5],
    viewer_bytes: [u8; 5],
    movement: usize,
    movement_bytes: [u8; 5],
    input: usize,
    input_bytes: [u8; 5],
    attack: usize,
    attack_bytes: [u8; 5],
    shader: usize,
    shader_bytes: [u8; 5],
    skills: [(usize, [u8; 5]); 3],
    steering: (usize, [u8; 5]),
    auto_attack: (usize, [u8; 5]),
    aim: (usize, [u8; 5]),
    outlines: Vec<(usize, [u8; 5])>,
    minimap: Vec<(usize, Vec<u8>)>,
    minimap_constants: (usize, Vec<u8>),
}
static PATCHES: OnceLock<PatchRecord> = OnceLock::new();

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn GetModuleHandleExW(flags: u32, name: *const u16, module: *mut *mut c_void) -> i32;
    fn GetModuleFileNameW(module: *mut c_void, name: *mut u16, size: u32) -> u32;
    fn VirtualAlloc(
        address: *mut c_void,
        size: usize,
        allocation: u32,
        protection: u32,
    ) -> *mut c_void;
    fn VirtualProtect(address: *mut c_void, size: usize, protection: u32, old: *mut u32) -> i32;
    fn FlushInstructionCache(process: *mut c_void, address: *const c_void, size: usize) -> i32;
    fn GetCurrentProcess() -> *mut c_void;
    fn GetProcessHeap() -> *mut c_void;
    fn HeapAlloc(heap: *mut c_void, flags: u32, size: usize) -> *mut c_void;
    fn HeapFree(heap: *mut c_void, flags: u32, memory: *mut c_void) -> i32;
    fn RtlCaptureStackBackTrace(
        skip: u32,
        count: u32,
        frames: *mut *mut c_void,
        hash: *mut u32,
    ) -> u16;
}

pub(super) fn capture_trace(label: &str, logger: &Logger) {
    let mut frames = [std::ptr::null_mut(); 64];
    let count =
        unsafe { RtlCaptureStackBackTrace(1, 64, frames.as_mut_ptr(), std::ptr::null_mut()) }
            as usize;
    let frames = frames[..count]
        .iter()
        .map(|frame| unsafe {
            let mut module = std::ptr::null_mut();
            if GetModuleHandleExW(0x06, *frame as *const u16, &mut module) == 0 {
                return format!("unmapped@{:x}", *frame as usize);
            }
            let mut name = [0u16; 512];
            let length = GetModuleFileNameW(module, name.as_mut_ptr(), name.len() as u32) as usize;
            let path = OsString::from_wide(&name[..length.min(name.len())]);
            let filename = std::path::Path::new(&path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            format!("{filename}+0x{:x}", *frame as usize - module as usize)
        })
        .collect::<Vec<_>>();
    logger.write(&format!(
        "NATIVE STACK label={label:?} os_thread={} rust_thread={:?} frames={frames:?}",
        crate::platform_input::thread_id(),
        std::thread::current().id()
    ));
}
unsafe fn read_call(site: usize) -> [u8; 5] {
    let mut bytes = [0; 5];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = std::ptr::read_volatile((site + i) as *const u8);
    }
    bytes
}
pub(super) fn audit_patches() -> Option<bool> {
    // These addresses belong to the resident executable, not a transient view.
    PATCHES.get().map(|p| unsafe {
        read_call(p.worker) == p.worker_bytes
            && read_call(p.viewer) == p.viewer_bytes
            && read_call(p.movement) == p.movement_bytes
            && read_call(p.input) == p.input_bytes
            && read_call(p.attack) == p.attack_bytes
            && read_call(p.shader) == p.shader_bytes
            && p.skills
                .iter()
                .all(|(site, bytes)| read_call(*site) == *bytes)
            && read_call(p.steering.0) == p.steering.1
            && read_call(p.auto_attack.0) == p.auto_attack.1
            && read_call(p.aim.0) == p.aim.1
            && p.outlines
                .iter()
                .all(|(site, bytes)| read_call(*site) == *bytes)
            && p.minimap.iter().all(|(site, bytes)| {
                std::slice::from_raw_parts(*site as *const u8, bytes.len()) == bytes
            })
            && std::slice::from_raw_parts(
                p.minimap_constants.0 as *const u8,
                p.minimap_constants.1.len(),
            ) == p.minimap_constants.1
    })
}
#[link(name = "bcrypt")]
extern "system" {
    fn BCryptOpenAlgorithmProvider(
        handle: *mut *mut c_void,
        algorithm: *const u16,
        implementation: *const u16,
        flags: u32,
    ) -> i32;
    fn BCryptHash(
        handle: *mut c_void,
        secret: *const u8,
        secret_size: u32,
        input: *const u8,
        input_size: u32,
        output: *mut u8,
        output_size: u32,
    ) -> i32;
    fn BCryptCloseAlgorithmProvider(handle: *mut c_void, flags: u32) -> i32;
}

pub(super) fn sha256(bytes: &[u8]) -> Result<String, String> {
    let algorithm: Vec<u16> = "SHA256\0".encode_utf16().collect();
    let mut handle = std::ptr::null_mut();
    let mut digest = [0u8; 32];
    let size = u32::try_from(bytes.len()).map_err(|_| "Executable too large")?;
    unsafe {
        if BCryptOpenAlgorithmProvider(&mut handle, algorithm.as_ptr(), std::ptr::null(), 0) < 0 {
            return Err("SHA256 provider unavailable".into());
        }
        let result = BCryptHash(
            handle,
            std::ptr::null(),
            0,
            bytes.as_ptr(),
            size,
            digest.as_mut_ptr(),
            32,
        );
        BCryptCloseAlgorithmProvider(handle, 0);
        if result < 0 {
            return Err("Executable SHA256 failed".into());
        }
    }
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}

fn relative_call(site: usize, destination: usize) -> Result<[u8; 5], String> {
    let relative = i32::try_from(destination as i128 - (site as i128 + 5))
        .map_err(|_| "Relay outside CALL rel32 range")?;
    let mut bytes = [0xe8, 0, 0, 0, 0];
    bytes[1..].copy_from_slice(&relative.to_le_bytes());
    Ok(bytes)
}
fn relative_jump(site: usize, destination: usize) -> Result<[u8; 5], String> {
    let mut bytes = relative_call(site, destination)?;
    bytes[0] = 0xe9;
    Ok(bytes)
}
unsafe fn allocate_near(site: usize) -> Result<usize, String> {
    // Resident private pages: never patch shared game constants in .rdata.
    let anchor = site & !0xffff;
    for distance in (0x10000..0x70000000usize).step_by(0x10000) {
        for address in [anchor.checked_sub(distance), anchor.checked_add(distance)]
            .into_iter()
            .flatten()
        {
            if address < 0x10000 {
                continue;
            }
            let block = VirtualAlloc(address as *mut c_void, 0x1000, 0x3000, 0x04);
            if !block.is_null() {
                return Ok(block as usize);
            }
        }
    }
    Err("Cannot allocate nearby native page".into())
}
unsafe fn relay(site: usize, destination: usize) -> Result<usize, String> {
    let block = allocate_near(site)?;
    let mut code = [0u8; 14];
    code[..6].copy_from_slice(&[0xff, 0x25, 0, 0, 0, 0]);
    code[6..].copy_from_slice(&destination.to_le_bytes());
    std::ptr::copy_nonoverlapping(code.as_ptr(), block as *mut u8, code.len());
    let mut old = 0;
    if VirtualProtect(block as *mut c_void, 0x1000, 0x20, &mut old) == 0 {
        return Err("Cannot make relay executable".into());
    }
    if FlushInstructionCache(GetCurrentProcess(), block as *const c_void, code.len()) == 0 {
        return Err("Cannot flush relay instruction cache".into());
    }
    relative_call(site, block)?;
    Ok(block)
}
pub(super) fn install() -> Result<(), String> {
    unsafe {
        let module = GetModuleHandleW(std::ptr::null());
        if module.is_null() {
            return Err("Executable module unavailable".into());
        }
        let mut path = vec![0u16; 32768];
        let length = GetModuleFileNameW(module, path.as_mut_ptr(), path.len() as u32) as usize;
        if length == 0 || length >= path.len() {
            return Err("Executable path unavailable".into());
        }
        let bytes = std::fs::read(OsString::from_wide(&path[..length]))
            .map_err(|e| format!("Cannot verify executable: {e}"))?;
        if sha256(&bytes)? != EXPECTED_SHA {
            return Err("Executable fingerprint differs; no hooks installed".into());
        }
        let base = module as usize;
        let pe_offset = std::ptr::read_unaligned((base + 0x3c) as *const u32) as usize;
        if pe_offset > 0x1000
            || std::ptr::read_unaligned((base + pe_offset) as *const u32) != 0x4550
            || std::ptr::read_unaligned((base + pe_offset + 8) as *const u32) != PE_TIMESTAMP
            || std::ptr::read_unaligned((base + pe_offset + 0x50) as *const u32) != IMAGE_SIZE
        {
            return Err("Loaded PE identity differs; no hooks installed".into());
        }
        if !crate::native_profile::verify_layout(base) {
            return Err("Native layout consumers differ; no hooks installed".into());
        }
        for (rva, expected) in [
            (WORKER_ORIGINAL, WORKER_PROLOGUE.as_slice()),
            (VIEW_ORIGINAL, VIEW_PROLOGUE.as_slice()),
            (MOVE_ORIGINAL, MOVE_PROLOGUE.as_slice()),
            (STOP_EVENT, STOP_PROLOGUE.as_slice()),
            (CANCEL_RECALL_EVENT, CANCEL_RECALL_PROLOGUE.as_slice()),
            (INPUT_ORIGINAL, INPUT_PROLOGUE.as_slice()),
            (ATTACK_ORIGINAL, ATTACK_PROLOGUE.as_slice()),
            (SHADER_ORIGINAL, SHADER_PROLOGUE.as_slice()),
            (GOLD_GETTER, GOLD_GETTER_BYTES.as_slice()),
            (BUILD_LEN_SITE, BUILD_LEN_BYTES.as_slice()),
            (BUILD_PTR_SITE, BUILD_PTR_BYTES.as_slice()),
            (SKILL_ORIGINALS[0], SKILL_PROLOGUE.as_slice()),
            (SKILL_ORIGINALS[1], SKILL_PROLOGUE.as_slice()),
            (SKILL_ORIGINALS[2], SKILL_PROLOGUE.as_slice()),
            SKILL_QUEUE_ANCHORS[0],
            SKILL_QUEUE_ANCHORS[1],
            SKILL_QUEUE_ANCHORS[2],
            (STEER_ORIGINAL, STEER_PROLOGUE.as_slice()),
            (DIRECT_STEP, DIRECT_PROLOGUE.as_slice()),
            (AIM_ORIGINAL, AIM_PROLOGUE.as_slice()),
            (OUTLINE_ORIGINAL, OUTLINE_PROLOGUE.as_slice()),
            (OUTLINE_LINE_SITE, OUTLINE_LINE_BYTES.as_slice()),
            (OUTLINE_DROP_COMMAND, OUTLINE_DROP_BYTES.as_slice()),
            (OUTLINE_DROP_TABLE, OUTLINE_DROP_TABLE_BYTES.as_slice()),
            (OUTLINE_DROP_CALLER, OUTLINE_DROP_CALL_BYTES.as_slice()),
            (OUTLINE_NINEPATCH_SITE, OUTLINE_NINEPATCH_BYTES.as_slice()),
            (0x21725a0, OUTLINE_PARAM_PROLOGUE.as_slice()),
            (0x2171ae0, OUTLINE_PARAM_PROLOGUE.as_slice()),
            AIM_WRITES[0],
            AIM_WRITES[1],
        ] {
            if std::slice::from_raw_parts((base + rva) as *const u8, expected.len()) != expected {
                return Err("Native entry differs or another mod hooked it".into());
            }
        }
        let specs = [
            (
                WORKER_SITE,
                WORKER_BYTES,
                worker_hook as *const () as usize,
                false,
            ),
            (
                VIEW_SITE,
                VIEW_BYTES,
                view_hook as *const () as usize,
                false,
            ),
            (MOVE_SITE, MOVE_BYTES, move_hook as *const () as usize, true),
            (
                INPUT_SITE,
                INPUT_BYTES,
                input_hook as *const () as usize,
                false,
            ),
            (
                ATTACK_SITE,
                ATTACK_BYTES,
                attack_hook as *const () as usize,
                false,
            ),
            (
                SHADER_SITE,
                SHADER_BYTES,
                shader_hook as *const () as usize,
                false,
            ),
            (
                SKILL_SITES[0],
                SKILL_BYTES[0],
                skill_q_hook as *const () as usize,
                false,
            ),
            (
                SKILL_SITES[1],
                SKILL_BYTES[1],
                skill_w_hook as *const () as usize,
                false,
            ),
            (
                SKILL_SITES[2],
                SKILL_BYTES[2],
                skill_r_hook as *const () as usize,
                false,
            ),
            (
                STEER_SITE,
                STEER_BYTES,
                steer_hook as *const () as usize,
                false,
            ),
            (
                AUTO_ATTACK_SITE,
                AUTO_ATTACK_BYTES,
                auto_attack_hook as *const () as usize,
                false,
            ),
            (AIM_SITE, AIM_BYTES, aim_hook as *const () as usize, false),
        ];
        for (rva, expected, _, _) in specs {
            if read_call(base + rva) != expected {
                return Err("Native branch bytes differ; no hooks installed".into());
            }
        }
        for (rva, expected) in OUTLINE_SITES {
            if read_call(base + rva) != expected {
                return Err("Unit renderer CALL bytes differ; no hooks installed".into());
            }
        }
        let mut pinned = std::ptr::null_mut();
        if GetModuleHandleExW(0x05, worker_hook as *const () as *const u16, &mut pinned) == 0 {
            return Err("Cannot pin adapter DLL for process lifetime".into());
        }
        let constants_address = allocate_near(base + SHADER_SITE)?;
        let minimap_plan = crate::minimap::native::plan(base, constants_address)?;
        if !minimap_plan.verify() {
            return Err(
                "Minimap operands or shared source constants differ; no hooks installed".into(),
            );
        }
        std::ptr::copy_nonoverlapping(
            minimap_plan.constants.as_ptr(),
            constants_address as *mut u8,
            minimap_plan.constants.len(),
        );
        let mut constants_old = 0;
        if VirtualProtect(
            constants_address as *mut c_void,
            0x1000,
            0x02,
            &mut constants_old,
        ) == 0
        {
            return Err("Cannot protect private minimap constants".into());
        }
        let mut patches: Vec<(usize, Vec<u8>, Vec<u8>)> = Vec::new();
        for (rva, expected, destination, jump) in specs {
            let site = base + rva;
            let target = relay(site, destination)?;
            let patch = if jump {
                relative_jump(site, target)?
            } else {
                relative_call(site, target)?
            };
            patches.push((site, expected.to_vec(), patch.to_vec()));
        }
        for (rva, expected) in OUTLINE_SITES {
            let site = base + rva;
            let target = relay(site, outline_hook as *const () as usize)?;
            let patch = relative_call(site, target)?;
            patches.push((site, expected.to_vec(), patch.to_vec()));
        }
        let outline_end = patches.len();
        for p in &minimap_plan.patches {
            patches.push((p.site, p.expected.clone(), p.bytes.clone()));
        }
        // Protect each page once. Multiple edits in one page must not save
        // an already-writable protection and leave that page writable.
        let mut pages = std::collections::BTreeSet::new();
        for (site, _, bytes) in &patches {
            pages.insert(*site & !0xfff);
            pages.insert((*site + bytes.len() - 1) & !0xfff);
        }
        let mut protected: Vec<(usize, u32)> = Vec::new();
        for page in pages {
            let mut old = 0;
            if VirtualProtect(page as *mut c_void, 0x1000, 0x40, &mut old) == 0 {
                for (page, old) in &protected {
                    let mut ignored = 0;
                    VirtualProtect(*page as *mut c_void, 0x1000, *old, &mut ignored);
                }
                return Err("Cannot protect all native patch pages; none changed".into());
            }
            protected.push((page, old));
        }
        ORIGINAL_WORKER.store(base + WORKER_ORIGINAL, Ordering::Release);
        ORIGINAL_VIEW.store(base + VIEW_ORIGINAL, Ordering::Release);
        ORIGINAL_MOVE.store(base + MOVE_ORIGINAL, Ordering::Release);
        NOTIFY_STOP.store(base + STOP_EVENT, Ordering::Release);
        NOTIFY_CANCEL_RECALL.store(base + CANCEL_RECALL_EVENT, Ordering::Release);
        ORIGINAL_INPUT.store(base + INPUT_ORIGINAL, Ordering::Release);
        ORIGINAL_ATTACK.store(base + ATTACK_ORIGINAL, Ordering::Release);
        ORIGINAL_SHADER.store(base + SHADER_ORIGINAL, Ordering::Release);
        ORIGINAL_STEER.store(base + STEER_ORIGINAL, Ordering::Release);
        ORIGINAL_DIRECT_STEP.store(base + DIRECT_STEP, Ordering::Release);
        ORIGINAL_AIM.store(base + AIM_ORIGINAL, Ordering::Release);
        ORIGINAL_OUTLINE.store(base + OUTLINE_ORIGINAL, Ordering::Release);
        for i in 0..3 {
            ORIGINAL_SKILLS[i].store(base + SKILL_ORIGINALS[i], Ordering::Release);
        }
        for (site, _, patch) in &patches {
            std::ptr::copy_nonoverlapping(patch.as_ptr(), *site as *mut u8, patch.len());
        }
        let flushed = patches.iter().all(|(site, _, bytes)| {
            FlushInstructionCache(GetCurrentProcess(), *site as *const c_void, bytes.len()) != 0
        });
        if !flushed {
            for (site, expected, _) in &patches {
                std::ptr::copy_nonoverlapping(expected.as_ptr(), *site as *mut u8, expected.len());
                FlushInstructionCache(GetCurrentProcess(), *site as *const c_void, expected.len());
            }
        }
        let mut restored = true;
        for (page, old) in &protected {
            let mut ignored = 0;
            restored &= VirtualProtect(*page as *mut c_void, 0x1000, *old, &mut ignored) != 0;
        }
        if !flushed {
            return Err("Cache flush failed; branches restored".into());
        }
        if !restored {
            return Err("Protection restore failed; relays remain pass-through".into());
        }
        if patches.iter().any(|(site, _, patch)| {
            std::slice::from_raw_parts(*site as *const u8, patch.len()) != patch
        }) {
            return Err("Branch readback mismatch; coordinator disabled".into());
        }
        let _ = PATCHES.set(PatchRecord {
            worker: patches[0].0,
            worker_bytes: patches[0].2.as_slice().try_into().unwrap(),
            viewer: patches[1].0,
            viewer_bytes: patches[1].2.as_slice().try_into().unwrap(),
            movement: patches[2].0,
            movement_bytes: patches[2].2.as_slice().try_into().unwrap(),
            input: patches[3].0,
            input_bytes: patches[3].2.as_slice().try_into().unwrap(),
            attack: patches[4].0,
            attack_bytes: patches[4].2.as_slice().try_into().unwrap(),
            shader: patches[5].0,
            shader_bytes: patches[5].2.as_slice().try_into().unwrap(),
            skills: std::array::from_fn(|i| {
                (
                    patches[6 + i].0,
                    patches[6 + i].2.as_slice().try_into().unwrap(),
                )
            }),
            steering: (patches[9].0, patches[9].2.as_slice().try_into().unwrap()),
            auto_attack: (patches[10].0, patches[10].2.as_slice().try_into().unwrap()),
            aim: (patches[11].0, patches[11].2.as_slice().try_into().unwrap()),
            outlines: patches[12..outline_end]
                .iter()
                .map(|(site, _, bytes)| (*site, bytes.as_slice().try_into().unwrap()))
                .collect(),
            minimap: patches[outline_end..]
                .iter()
                .map(|(site, _, bytes)| (*site, bytes.clone()))
                .collect(),
            minimap_constants: (constants_address, minimap_plan.constants),
        });
        if let Some(shared) = SHARED.get() {
            shared.logger.write("MINIMAP installed content=352 padding=4 wide_origin=1564,724 frame=360; native markers/fog/camera/input scaled; private constants read-only");
            shared.logger.write(&format!("NATIVE PATCH_READBACK worker_rva={WORKER_SITE:x} viewer_rva={VIEW_SITE:x} move_rva={MOVE_SITE:x} input_rva={INPUT_SITE:x} attack_rva={ATTACK_SITE:x} auto_attack_rva={AUTO_ATTACK_SITE:x} shader_rva={SHADER_SITE:x} skill_rvas={SKILL_SITES:x?} steering_rva={STEER_SITE:x} aim_rva={AIM_SITE:x} outline_sites={} verified=true", OUTLINE_SITES.len()));
        }
        OUTLINE_READY.store(true, Ordering::Release);
        let shop = install_shop(base);
        if let Some(shared) = SHARED.get() {
            shared.logger.write(&match &shop {
                Ok(()) => format!("SHOP HOOKS installed upgrade_thunk_rva={SHOP_UPGRADE_THUNK:x} new_thunk_rva={SHOP_NEW_THUNK:x}; native answers unless Manual shopping applies"),
                Err(e) => format!("SHOP HOOKS unavailable: {e}; Manual shopping disabled, other hooks unaffected"),
            });
        }
        Ok(())
    }
}

fn owned_key(shared: &Shared, actor: usize) -> Option<crate::native_timing::MatchKey> {
    let key = shared.abilities.selected_key(actor)?;
    shared.timing.allows_input(key).then_some(key)
}
unsafe fn owned_entity(
    shared: &Shared,
    entity: usize,
) -> Option<(crate::native_timing::MatchKey, usize)> {
    let actor = std::ptr::read_unaligned((entity + 0x5b8) as *const usize);
    let key = owned_key(shared, actor)?;
    (std::ptr::read_unaligned((entity + 0x68) as *const u32) == 15).then_some((key, actor))
}
