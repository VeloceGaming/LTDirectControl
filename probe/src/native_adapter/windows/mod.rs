//! Native hooks for one fingerprinted executable: patch installation and
//! shared helpers. Hook families live in the child modules. This folder is
//! built once for each supported game version (see the parent module), each
//! time against that version's tables: `layout`, `identity`, `effects` and
//! `items`. Nothing here may name an address or a game version itself.
use super::identity::*;
use super::items::ITEMS;
use super::layout::*;
use super::stat_sites::*;
use super::*;
use crate::native_profile::family;
use std::ffi::{c_void, OsString};
use std::os::windows::ffi::OsStringExt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// One file per hook family.
mod combat;
mod input;
mod movement;
mod outline;
#[cfg(feature = "research")]
mod preview;
#[cfg(feature = "research")]
mod research;
mod shop;
mod skill_numbers;
mod stack;
mod stats;
#[cfg(test)]
mod tests;
mod tooltips;
mod view;
pub(in crate::native_adapter) use combat::attack_ranges;
use combat::*;
use input::*;
use movement::*;
use outline::*;
// Used by the public layer above; explicit, so they win over its own
// same-named wrappers that `use super::*` brings in.
pub(in crate::native_adapter) use outline::SPRITE_DRAWS;
pub(in crate::native_adapter) use outline::{
    clear_outline_targets, outline_status, set_outline_targets,
};
use shop::*;
pub(in crate::native_adapter) use shop::{
    buyer_anchor_report, item_slot_capacity, native_player, player_build, player_owned_len,
    shop_ready,
};
pub(in crate::native_adapter) use stack::{thread_alive, thread_stack};
use stats::{allow_stats, stats_intact};
pub(in crate::native_adapter) use stats::{
    install_stats_at_title, stat_rules_report, stats_pending,
};
pub(in crate::native_adapter) use view::reset_session;
use view::*;

pub(in crate::native_adapter) fn verified_base() -> Option<usize> {
    PATCHES.get()?;
    let base = unsafe { GetModuleHandleW(std::ptr::null()) } as usize;
    (base != 0).then_some(base)
}
/// The executable's base when every one of `families` is installed.
pub(in crate::native_adapter) fn family_base(families: u32) -> Option<usize> {
    let installed = PATCHES.get()?.families;
    (installed & families == families)
        .then(verified_base)
        .flatten()
}
/// The executable's base and what the registered-item reader checks in it,
/// once the shop family (which that reader belongs to) is installed.
pub(in crate::native_adapter) fn item_reader() -> Option<(usize, crate::native_items::Addresses)> {
    Some((family_base(family::SHOP)?, ITEMS))
}
pub(in crate::native_adapter) fn installed(families: u32) -> bool {
    PATCHES
        .get()
        .is_some_and(|p| p.families & families == families)
}

/// What `install` changed: the families installed, each patched site with
/// the bytes written there, and the private minimap constants.
struct PatchRecord {
    families: u32,
    sites: Vec<(usize, Vec<u8>)>,
    constants: Option<(usize, Vec<u8>)>,
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

pub(in crate::native_adapter) fn capture_trace(label: &str, logger: &Logger) {
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
pub(in crate::native_adapter) fn audit_patches() -> Option<bool> {
    // These addresses belong to the resident executable, not a transient view.
    PATCHES.get().map(|p| unsafe {
        p.sites.iter().all(|(site, bytes)| {
            std::slice::from_raw_parts(*site as *const u8, bytes.len()) == bytes
        }) && p.constants.as_ref().is_none_or(|(address, bytes)| {
            std::slice::from_raw_parts(*address as *const u8, bytes.len()) == bytes
        }) && stats_intact().unwrap_or(true)
    })
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
/// `sha` is `host::executable_sha()`: the parent module computes it once and
/// offers it to the copy whose game version it names. `skip` lists families
/// the player turned off (the outline, for ground rings). Returns the
/// families installed: `ENABLED`, less `skip` and any that stepped aside for
/// another mod.
pub(in crate::native_adapter) fn install(sha: &str, skip: u32) -> Result<u32, String> {
    unsafe {
        let module = GetModuleHandleW(std::ptr::null());
        if module.is_null() {
            return Err("Executable module unavailable".into());
        }
        if sha != EXPECTED_SHA {
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
        let enabled = ENABLED & !skip;
        // Public builds carry no live skill reader (stage C of the preview
        // plan), so its family and its guards stay out.
        #[cfg(not(feature = "research"))]
        let enabled = enabled & !family::READERS;
        let on = |family: u32| enabled & family != 0;
        if !on(family::SESSION) {
            return Err(
                "No hook family is reviewed for this executable; no hooks installed".into(),
            );
        }
        // The attack and skill hooks emit the movement family's stop events.
        if on(family::COMBAT) && !on(family::MOVEMENT) {
            return Err("Combat hooks need the movement family; no hooks installed".into());
        }
        // Every function entry and byte pattern the hooks rely on, plus the
        // outline family's call sites: a family that may step aside drops
        // out if another mod holds one of its spots (`families_to_install`).
        let entries = [
            (
                family::SESSION,
                (WORKER_ORIGINAL, WORKER_PROLOGUE.as_slice()),
            ),
            (family::SESSION, (VIEW_ORIGINAL, VIEW_PROLOGUE.as_slice())),
            (family::MOVEMENT, (MOVE_ORIGINAL, MOVE_PROLOGUE.as_slice())),
            (family::MOVEMENT, (STOP_EVENT, STOP_PROLOGUE.as_slice())),
            (
                family::MOVEMENT,
                (CANCEL_RECALL_EVENT, CANCEL_RECALL_PROLOGUE.as_slice()),
            ),
            (family::SESSION, (INPUT_ORIGINAL, INPUT_PROLOGUE.as_slice())),
            (
                family::COMBAT,
                (ATTACK_ORIGINAL, ATTACK_PROLOGUE.as_slice()),
            ),
            (
                family::OUTLINE,
                (SHADER_ORIGINAL, SHADER_PROLOGUE.as_slice()),
            ),
            (family::SHOP, (GOLD_GETTER, GOLD_GETTER_BYTES.as_slice())),
            (family::SHOP, (BUILD_LEN_SITE, BUILD_LEN_BYTES.as_slice())),
            (family::SHOP, (BUILD_PTR_SITE, BUILD_PTR_BYTES.as_slice())),
            (
                family::COMBAT,
                (SKILL_ORIGINALS[0], SKILL_PROLOGUE.as_slice()),
            ),
            (
                family::COMBAT,
                (SKILL_ORIGINALS[1], SKILL_PROLOGUE.as_slice()),
            ),
            (
                family::COMBAT,
                (SKILL_ORIGINALS[2], SKILL_PROLOGUE.as_slice()),
            ),
            (family::COMBAT, SKILL_QUEUE_ANCHORS[0]),
            (family::COMBAT, SKILL_QUEUE_ANCHORS[1]),
            (family::COMBAT, SKILL_QUEUE_ANCHORS[2]),
            (
                family::MOVEMENT,
                (STEER_ORIGINAL, STEER_PROLOGUE.as_slice()),
            ),
            (family::MOVEMENT, (DIRECT_STEP, DIRECT_PROLOGUE.as_slice())),
            (family::COMBAT, (AIM_ORIGINAL, AIM_PROLOGUE.as_slice())),
            (
                family::OUTLINE,
                (OUTLINE_ORIGINAL, OUTLINE_PROLOGUE.as_slice()),
            ),
            (
                family::OUTLINE,
                (OUTLINE_LINE_SITE, OUTLINE_LINE_BYTES.as_slice()),
            ),
            (
                family::OUTLINE,
                (OUTLINE_DROP_COMMAND, OUTLINE_DROP_BYTES.as_slice()),
            ),
            (
                family::OUTLINE,
                (OUTLINE_DROP_TABLE, OUTLINE_DROP_TABLE_BYTES.as_slice()),
            ),
            (
                family::OUTLINE,
                (OUTLINE_DROP_CALLER, OUTLINE_DROP_CALL_BYTES.as_slice()),
            ),
            (
                family::OUTLINE,
                (OUTLINE_NINEPATCH_SITE, OUTLINE_NINEPATCH_BYTES.as_slice()),
            ),
            (
                family::OUTLINE,
                (OUTLINE_FLOAT_PARAM, OUTLINE_FLOAT_PROLOGUE.as_slice()),
            ),
            (
                family::OUTLINE,
                (OUTLINE_COLOR_PARAM, OUTLINE_COLOR_PROLOGUE.as_slice()),
            ),
            (family::COMBAT, AIM_WRITES[0]),
            (family::COMBAT, AIM_WRITES[1]),
        ];
        let mut spots: Vec<(u32, usize, bool)> = entries
            .iter()
            .map(|&(family, (rva, expected))| {
                let holds = !on(family)
                    || std::slice::from_raw_parts((base + rva) as *const u8, expected.len())
                        == expected;
                (family, rva, holds)
            })
            .collect();
        if on(family::OUTLINE) {
            spots.extend(OUTLINE_SITES.iter().map(|(rva, expected)| {
                (family::OUTLINE, *rva, read_call(base + rva) == *expected)
            }));
        }
        let (families, taken) =
            crate::native_profile::families_to_install(enabled, &spots).map_err(|rva| {
                format!("Native entry differs or another mod hooked it (rva {rva:#x}); no hooks installed")
            })?;
        let on = |family: u32| families & family != 0;
        if !GUARDS.verify(base, families) {
            return Err("Native layout consumers differ; no hooks installed".into());
        }
        // (family, call site, its bytes, our hook, tail jump).
        let specs = [
            (
                family::SESSION,
                WORKER_SITE,
                WORKER_BYTES,
                worker_hook as *const () as usize,
                false,
            ),
            (
                family::SESSION,
                VIEW_SITE,
                VIEW_BYTES,
                view_hook as *const () as usize,
                false,
            ),
            (
                family::MOVEMENT,
                MOVE_SITE,
                MOVE_BYTES,
                move_hook as *const () as usize,
                true,
            ),
            (
                family::SESSION,
                INPUT_SITE,
                INPUT_BYTES,
                input_hook as *const () as usize,
                false,
            ),
            (
                family::COMBAT,
                ATTACK_SITE,
                ATTACK_BYTES,
                attack_hook as *const () as usize,
                false,
            ),
            (
                family::COMBAT,
                SKILL_SITES[0],
                SKILL_BYTES[0],
                skill_q_hook as *const () as usize,
                false,
            ),
            (
                family::COMBAT,
                SKILL_SITES[1],
                SKILL_BYTES[1],
                skill_w_hook as *const () as usize,
                false,
            ),
            (
                family::COMBAT,
                SKILL_SITES[2],
                SKILL_BYTES[2],
                skill_r_hook as *const () as usize,
                false,
            ),
            (
                family::MOVEMENT,
                STEER_SITE,
                STEER_BYTES,
                steer_hook as *const () as usize,
                false,
            ),
            (
                family::COMBAT,
                AUTO_ATTACK_SITE,
                AUTO_ATTACK_BYTES,
                auto_attack_hook as *const () as usize,
                false,
            ),
            (
                family::COMBAT,
                AIM_SITE,
                AIM_BYTES,
                aim_hook as *const () as usize,
                false,
            ),
        ];
        let specs: Vec<_> = specs.into_iter().filter(|spec| on(spec.0)).collect();
        for (_, rva, expected, _, _) in &specs {
            if read_call(base + rva) != *expected {
                return Err("Native branch bytes differ; no hooks installed".into());
            }
        }
        let outline_sites: &[(usize, [u8; 5])] = if on(family::OUTLINE) {
            &OUTLINE_SITES
        } else {
            &[]
        };
        let mut pinned = std::ptr::null_mut();
        if GetModuleHandleExW(0x05, worker_hook as *const () as *const u16, &mut pinned) == 0 {
            return Err("Cannot pin adapter DLL for process lifetime".into());
        }
        let minimap_plan = if on(family::MINIMAP) {
            let constants_address = allocate_near(base + VIEW_SITE)?;
            let minimap_plan =
                crate::minimap::native::plan(MINIMAP_PROFILE, base, constants_address)?;
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
            Some((constants_address, minimap_plan))
        } else {
            None
        };
        let mut patches: Vec<(usize, Vec<u8>, Vec<u8>)> = Vec::new();
        for (_, rva, expected, destination, jump) in &specs {
            let site = base + rva;
            let target = relay(site, *destination)?;
            let patch = if *jump {
                relative_jump(site, target)?
            } else {
                relative_call(site, target)?
            };
            patches.push((site, expected.to_vec(), patch.to_vec()));
        }
        for (rva, expected) in outline_sites {
            let site = base + rva;
            let target = relay(site, outline_hook as *const () as usize)?;
            let patch = relative_call(site, target)?;
            patches.push((site, expected.to_vec(), patch.to_vec()));
        }
        if let Some((_, plan)) = &minimap_plan {
            for p in &plan.patches {
                patches.push((p.site, p.expected.clone(), p.bytes.clone()));
            }
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
        // A family that is not installed keeps its originals at zero: its
        // hooks never run and nothing may call through them.
        if on(family::SESSION) {
            ORIGINAL_WORKER.store(base + WORKER_ORIGINAL, Ordering::Release);
            ORIGINAL_VIEW.store(base + VIEW_ORIGINAL, Ordering::Release);
            ORIGINAL_INPUT.store(base + INPUT_ORIGINAL, Ordering::Release);
        }
        if on(family::MOVEMENT) {
            ORIGINAL_MOVE.store(base + MOVE_ORIGINAL, Ordering::Release);
            NOTIFY_STOP.store(base + STOP_EVENT, Ordering::Release);
            NOTIFY_CANCEL_RECALL.store(base + CANCEL_RECALL_EVENT, Ordering::Release);
            ORIGINAL_STEER.store(base + STEER_ORIGINAL, Ordering::Release);
            ORIGINAL_DIRECT_STEP.store(base + DIRECT_STEP, Ordering::Release);
        }
        if on(family::COMBAT) {
            ORIGINAL_ATTACK.store(base + ATTACK_ORIGINAL, Ordering::Release);
            ORIGINAL_AIM.store(base + AIM_ORIGINAL, Ordering::Release);
            for i in 0..3 {
                ORIGINAL_SKILLS[i].store(base + SKILL_ORIGINALS[i], Ordering::Release);
            }
        }
        if on(family::OUTLINE) {
            ORIGINAL_SHADER.store(base + SHADER_ORIGINAL, Ordering::Release);
            ORIGINAL_OUTLINE.store(base + OUTLINE_ORIGINAL, Ordering::Release);
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
        let shop = on(family::SHOP);
        let _ = PATCHES.set(PatchRecord {
            // The purchase hooks install below and report on their own.
            families,
            sites: patches
                .iter()
                .map(|(site, _, bytes)| (*site, bytes.clone()))
                .collect(),
            constants: minimap_plan.map(|(address, plan)| (address, plan.constants)),
        });
        if let Some(shared) = SHARED.get() {
            if on(family::MINIMAP) {
                shared.logger.write("MINIMAP installed content=352 padding=4 wide_origin=1564,724 frame=360; native markers/fog/camera/input scaled; private constants read-only");
            }
            if !taken.is_empty() {
                shared.logger.write(&format!(
                    "NATIVE OUTLINE stepped aside: another mod changed the unit renderer spots {taken:#x?}; hover and target outlines and the attack-click flash are off (ground rings stand in), every other family installed"
                ));
            }
            shared.logger.write(&format!("NATIVE PATCH_READBACK families={families:#x} sites={} worker_rva={WORKER_SITE:x} viewer_rva={VIEW_SITE:x} move_rva={MOVE_SITE:x} input_rva={INPUT_SITE:x} attack_rva={ATTACK_SITE:x} auto_attack_rva={AUTO_ATTACK_SITE:x} skill_rvas={SKILL_SITES:x?} steering_rva={STEER_SITE:x} aim_rva={AIM_SITE:x} outline_sites={} verified=true", patches.len(), outline_sites.len()));
        }
        OUTLINE_READY.store(on(family::OUTLINE), Ordering::Release);
        if shop {
            let shop = install_shop(base);
            if let Some(shared) = SHARED.get() {
                shared.logger.write(&match &shop {
                    Ok(()) => format!("SHOP HOOKS installed upgrade_thunk_rva={SHOP_UPGRADE_THUNK:x} new_thunk_rva={SHOP_NEW_THUNK:x}; native answers unless Manual shopping applies"),
                    Err(e) => format!("SHOP HOOKS unavailable: {e}; Manual shopping disabled, other hooks unaffected"),
                });
            }
        }
        // The stat rules are patched in only while their setting is On, at
        // this or a later visit to the title screen (`install_stat_rules`).
        if on(family::STATS) {
            allow_stats();
        }
        Ok(families)
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
