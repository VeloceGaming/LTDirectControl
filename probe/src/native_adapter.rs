//! Experimental adapter for ONE fingerprinted executable, not a stable API.
//! Four decoded CALLs and one movement-consumer tail JMP are redirected; originals stay
//! intact. Installing is permitted only at the title screen, before a viewer
//! or its normal worker exists. No patch is made to the executable on disk.
use crate::{
    native_timing::{NativeTiming, ViewMode},
    Logger,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

struct Shared {
    timing: Arc<NativeTiming>,
    logger: Arc<Logger>,
    movement: Arc<crate::movement_test::MovementTest>,
    camera: Arc<crate::camera::CameraControl>,
    abilities: Arc<crate::abilities::Abilities>,
}
static SHARED: OnceLock<Shared> = OnceLock::new();
static WORKER_ENTRIES: AtomicUsize = AtomicUsize::new(0);
static VIEW_ENTRIES: AtomicUsize = AtomicUsize::new(0);
static INPUT_ENTRIES: AtomicUsize = AtomicUsize::new(0);

pub fn configure(
    timing: Arc<NativeTiming>,
    logger: Arc<Logger>,
    movement: Arc<crate::movement_test::MovementTest>,
    camera: Arc<crate::camera::CameraControl>,
    abilities: Arc<crate::abilities::Abilities>,
) -> Result<(), &'static str> {
    logger.write(&format!(
        "NATIVE CONFIG coordinator={:p} shared_slot={:p} os_thread={}",
        Arc::as_ptr(&timing),
        &SHARED,
        crate::platform_input::thread_id()
    ));
    SHARED
        .set(Shared {
            timing,
            logger,
            movement,
            camera,
            abilities,
        })
        .map_err(|_| "Native adapter already configured; restart game")
}
#[derive(Clone, Copy)]
struct StopTicket {
    key: crate::native_timing::MatchKey,
    actor: usize,
    hold: bool,
    cancel_recall: bool,
}
thread_local! {
    static STOP_TICKET: std::cell::Cell<Option<StopTicket>> = const { std::cell::Cell::new(None) };
}
pub fn arm_stop(
    key: crate::native_timing::MatchKey,
    actor: Option<usize>,
    hold: bool,
    cancel_recall: bool,
) {
    STOP_TICKET.set(actor.map(|actor| StopTicket {
        key,
        actor,
        hold,
        cancel_recall,
    }));
}

pub fn capture_trace(label: &str, logger: &Logger) {
    #[cfg(all(windows, target_arch = "x86_64"))]
    windows::capture_trace(label, logger);
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    logger.write(&format!(
        "NATIVE STACK label={label:?} unavailable on this platform"
    ));
}

/// Called from the SDK client, independent of whether a native hook arrives.
pub fn sample_status(logger: &Logger) -> bool {
    let worker = WORKER_ENTRIES.load(Ordering::Relaxed);
    let viewer = VIEW_ENTRIES.load(Ordering::Relaxed);
    let input = INPUT_ENTRIES.load(Ordering::Relaxed);
    #[cfg(all(windows, target_arch = "x86_64"))]
    let patches = windows::audit_patches();
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    let patches: Option<bool> = None;
    logger.write(&format!("NATIVE TRAFFIC worker_entries={worker} viewer_entries={viewer} input_entries={input} patch_bytes_match={patches:?} shared_configured={} os_thread={}",
        SHARED.get().is_some(), crate::platform_input::thread_id()));
    patches != Some(false)
}
pub fn install() -> Result<(), String> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        windows::install()
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        Err("Native adapter requires Windows x64".into())
    }
}
/// Forget a destroyed viewer's lease without dereferencing its old addresses.
/// Live release restoration still happens inside the original viewer borrow.
pub fn reset_session() {
    #[cfg(all(windows, target_arch = "x86_64"))]
    windows::reset_session();
}

#[cfg(all(windows, target_arch = "x86_64"))]
mod windows {
    use super::*;
    use std::ffi::{c_void, OsString};
    use std::os::windows::ffi::OsStringExt;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Foreground AI stacks from test 8 identify be42b0, whose tick CALL
    // be43f5 still owns the runner write lock. Intercept its later send CALL
    // instead, and wait only AFTER it returns, outside all three write locks.
    const WORKER_SITE: usize = 0xbe470a;
    const WORKER_ORIGINAL: usize = 0xa3fbe0;
    // Current 0.6.2 SDK scene conversion (2e45930/table 3cec520) maps
    // native tag 11 to InGame. Scene dispatch 9f8f73/table 3ac4790 selects
    // 9f9c1c: view = database+13e0, receiver = database+1908. Frames received
    // at 9fbbe5 enter the view queue before this a8b090 CALL. The outer
    // b240e9 candidate had ZERO entrances in test 9; SDK pointer proximity
    // did not establish that it played the actual battlefield.
    const VIEW_SITE: usize = 0x9fdaf0;
    const VIEW_ORIGINAL: usize = 0xa8b090;
    const MOVE_SITE: usize = 0x162f276;
    const MOVE_ORIGINAL: usize = 0x15c4210;
    const STOP_EVENT: usize = 0x1705840;
    // Native Return cancellation event (also used before native skills/attacks
    // when action == 1); &events, actor ID -> unit. No new branch redirect.
    const CANCEL_RECALL_EVENT: usize = 0x17069c0;
    const CANCEL_RECALL_PROLOGUE: [u8; 16] = [
        0x55, 0x56, 0x57, 0x48, 0x83, 0xec, 0x30, 0x48, 0x8d, 0x6c, 0x24, 0x30, 0x48, 0xc7, 0x45,
        0xf8,
    ];
    // GameClient::input_event: view/client, UI, system, f32 dt, event, window, database.
    // Native tag 6 = key press, tag 7 = key release, with key byte at event+8.
    // These two variants are POD; skipping them needs no native allocation drop.
    // Normal tag-11 InGame branch 9f9dfb passes database+13e0 to a26d60;
    // its shared input CALL forwards that SAME view as playback 9fdaf0.
    // The outer b234b6 uses a different view and is not this hook.
    const INPUT_SITE: usize = 0xa270cd;
    // Native Input::Attack variant: EntityData, &InputTarget, &events -> unit.
    // This observes POD skill metadata on attack ticks as well as move ticks.
    const ATTACK_SITE: usize = 0x162f387;
    const ATTACK_ORIGINAL: usize = 0x15b2700;
    const ATTACK_BYTES: [u8; 5] = [0xe8, 0x74, 0x33, 0xf8, 0xff];
    const ATTACK_PROLOGUE: [u8; 19] = [
        0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec,
        0x88, 0, 0, 0,
    ];
    const INPUT_ORIGINAL: usize = 0xcaf090;
    const INPUT_BYTES: [u8; 5] = [0xe8, 0xbe, 0x7f, 0x28, 0];
    const INPUT_PROLOGUE: [u8; 19] = [
        0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec,
        0x98, 1, 0, 0,
    ];
    const MOVE_BYTES: [u8; 5] = [0xe9, 0x95, 0x4f, 0xf9, 0xff];
    const MOVE_PROLOGUE: [u8; 16] = [
        0x56, 0x57, 0x4c, 0x8b, 0x91, 0xd0, 2, 0, 0, 0x4d, 0x85, 0xd2, 0x74, 0x34, 0x48, 0x8b,
    ];
    const STOP_PROLOGUE: [u8; 16] = [
        0x55, 0x56, 0x57, 0x48, 0x83, 0xec, 0x30, 0x48, 0x8d, 0x6c, 0x24, 0x30, 0x48, 0xc7, 0x45,
        0xf8,
    ];
    const WORKER_BYTES: [u8; 5] = [0xe8, 0xd1, 0xb4, 0xe5, 0xff];
    const VIEW_BYTES: [u8; 5] = [0xe8, 0x9b, 0xd5, 0x08, 0x00];
    const WORKER_PROLOGUE: [u8; 19] = [
        0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec,
        0xb8, 0x05, 0, 0,
    ];
    const VIEW_PROLOGUE: [u8; 19] = [
        0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec,
        0x58, 0x01, 0, 0,
    ];
    const EXPECTED_SHA: &str = "15df9eb3b6915cdcc4c2ebb3b7f5fa232b563c4cdd32208581634817b71adc23";
    static ORIGINAL_WORKER: AtomicUsize = AtomicUsize::new(0);
    static ORIGINAL_VIEW: AtomicUsize = AtomicUsize::new(0);
    static ORIGINAL_MOVE: AtomicUsize = AtomicUsize::new(0);
    static NOTIFY_STOP: AtomicUsize = AtomicUsize::new(0);
    static NOTIFY_CANCEL_RECALL: AtomicUsize = AtomicUsize::new(0);
    static ORIGINAL_INPUT: AtomicUsize = AtomicUsize::new(0);
    static ORIGINAL_ATTACK: AtomicUsize = AtomicUsize::new(0);
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
        fn VirtualProtect(address: *mut c_void, size: usize, protection: u32, old: *mut u32)
            -> i32;
        fn FlushInstructionCache(process: *mut c_void, address: *const c_void, size: usize) -> i32;
        fn GetCurrentProcess() -> *mut c_void;
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
                let length =
                    GetModuleFileNameW(module, name.as_mut_ptr(), name.len() as u32) as usize;
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

    fn sha256(bytes: &[u8]) -> Result<String, String> {
        let algorithm: Vec<u16> = "SHA256\0".encode_utf16().collect();
        let mut handle = std::ptr::null_mut();
        let mut digest = [0u8; 32];
        let size = u32::try_from(bytes.len()).map_err(|_| "Executable too large")?;
        unsafe {
            if BCryptOpenAlgorithmProvider(&mut handle, algorithm.as_ptr(), std::ptr::null(), 0) < 0
            {
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
    unsafe fn relay(site: usize, destination: usize) -> Result<usize, String> {
        // Reserve at an exact 64-KiB boundary within rel32 reach. Leaked on
        // purpose: call-site relays and the pinned DLL live until process exit.
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
                if block.is_null() {
                    continue;
                }
                let mut code = [0u8; 14];
                code[..6].copy_from_slice(&[0xff, 0x25, 0, 0, 0, 0]);
                code[6..].copy_from_slice(&destination.to_le_bytes());
                std::ptr::copy_nonoverlapping(code.as_ptr(), block.cast::<u8>(), code.len());
                let mut old = 0;
                if VirtualProtect(block, 0x1000, 0x20, &mut old) == 0 {
                    return Err("Cannot make relay executable".into());
                }
                if FlushInstructionCache(GetCurrentProcess(), block, code.len()) == 0 {
                    return Err("Cannot flush relay instruction cache".into());
                }
                relative_call(site, block as usize)?;
                return Ok(block as usize);
            }
        }
        Err("Cannot allocate a nearby relay".into())
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
                || std::ptr::read_unaligned((base + pe_offset + 8) as *const u32) != 0x6abc597e
                || std::ptr::read_unaligned((base + pe_offset + 0x50) as *const u32) != 0x52b8000
            {
                return Err("Loaded PE identity differs; no hooks installed".into());
            }
            for (rva, expected) in [
                (WORKER_ORIGINAL, WORKER_PROLOGUE.as_slice()),
                (VIEW_ORIGINAL, VIEW_PROLOGUE.as_slice()),
                (MOVE_ORIGINAL, MOVE_PROLOGUE.as_slice()),
                (STOP_EVENT, STOP_PROLOGUE.as_slice()),
                (CANCEL_RECALL_EVENT, CANCEL_RECALL_PROLOGUE.as_slice()),
                (INPUT_ORIGINAL, INPUT_PROLOGUE.as_slice()),
                (ATTACK_ORIGINAL, ATTACK_PROLOGUE.as_slice()),
            ] {
                if std::slice::from_raw_parts((base + rva) as *const u8, expected.len()) != expected
                {
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
            ];
            for (rva, expected, _, _) in specs {
                if read_call(base + rva) != expected {
                    return Err("Native branch bytes differ; no hooks installed".into());
                }
            }
            let mut pinned = std::ptr::null_mut();
            if GetModuleHandleExW(0x05, worker_hook as *const () as *const u16, &mut pinned) == 0 {
                return Err("Cannot pin adapter DLL for process lifetime".into());
            }
            let mut patches = Vec::new();
            for (rva, expected, destination, jump) in specs {
                let site = base + rva;
                let target = relay(site, destination)?;
                let patch = if jump {
                    relative_jump(site, target)?
                } else {
                    relative_call(site, target)?
                };
                patches.push((site, expected, patch, 0u32));
            }
            // Acquire all writable pages before changing any branch.
            for i in 0..patches.len() {
                let (site, _, _, old) = &mut patches[i];
                if VirtualProtect(*site as *mut c_void, 5, 0x40, old) == 0 {
                    for (site, _, _, old) in patches.iter().take(i) {
                        let mut ignored = 0;
                        VirtualProtect(*site as *mut c_void, 5, *old, &mut ignored);
                    }
                    return Err("Cannot protect all branch pages; none changed".into());
                }
            }
            ORIGINAL_WORKER.store(base + WORKER_ORIGINAL, Ordering::Release);
            ORIGINAL_VIEW.store(base + VIEW_ORIGINAL, Ordering::Release);
            ORIGINAL_MOVE.store(base + MOVE_ORIGINAL, Ordering::Release);
            NOTIFY_STOP.store(base + STOP_EVENT, Ordering::Release);
            NOTIFY_CANCEL_RECALL.store(base + CANCEL_RECALL_EVENT, Ordering::Release);
            ORIGINAL_INPUT.store(base + INPUT_ORIGINAL, Ordering::Release);
            ORIGINAL_ATTACK.store(base + ATTACK_ORIGINAL, Ordering::Release);
            for (site, _, patch, _) in &patches {
                std::ptr::copy_nonoverlapping(patch.as_ptr(), *site as *mut u8, 5);
            }
            let flushed = patches.iter().all(|(site, _, _, _)| {
                FlushInstructionCache(GetCurrentProcess(), *site as *const c_void, 5) != 0
            });
            if !flushed {
                for (site, expected, _, _) in &patches {
                    std::ptr::copy_nonoverlapping(expected.as_ptr(), *site as *mut u8, 5);
                    FlushInstructionCache(GetCurrentProcess(), *site as *const c_void, 5);
                }
            }
            let mut restored = true;
            for (site, _, _, old) in &patches {
                let mut ignored = 0;
                restored &= VirtualProtect(*site as *mut c_void, 5, *old, &mut ignored) != 0;
            }
            if !flushed {
                return Err("Cache flush failed; branches restored".into());
            }
            if !restored {
                return Err("Protection restore failed; relays remain pass-through".into());
            }
            if patches
                .iter()
                .any(|(site, _, patch, _)| read_call(*site) != *patch)
            {
                return Err("Branch readback mismatch; coordinator disabled".into());
            }
            let _ = PATCHES.set(PatchRecord {
                worker: patches[0].0,
                worker_bytes: patches[0].2,
                viewer: patches[1].0,
                viewer_bytes: patches[1].2,
                movement: patches[2].0,
                movement_bytes: patches[2].2,
                input: patches[3].0,
                input_bytes: patches[3].2,
                attack: patches[4].0,
                attack_bytes: patches[4].2,
            });
            if let Some(shared) = SHARED.get() {
                shared.logger.write(&format!("NATIVE PATCH_READBACK worker_rva={WORKER_SITE:x} viewer_rva={VIEW_SITE:x} move_rva={MOVE_SITE:x} input_rva={INPUT_SITE:x} attack_rva={ATTACK_SITE:x} verified=true"));
            }
            Ok(())
        }
    }

    // Sender::send: output Result pointer, sender pointer, owned frame pointer.
    // The native caller inspects the output memory, not RAX, after return.
    type WorkerFn = unsafe extern "system" fn(usize, usize, usize);
    // Verified: four GP arguments, four stack pointer/usize arguments, then
    // stack f32 dt. Native caller ignores the result (update returns unit).
    type ViewFn =
        unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, usize, usize, f32);
    type MoveFn = unsafe extern "system" fn(usize, u64, u64, usize);
    type StopEventFn = unsafe extern "system" fn(usize, usize);
    type AttackFn = unsafe extern "system" fn(usize, usize, usize);

    /// EntityData pointer is borrowed by this native input-consumer call only.
    /// Copy scalar values, never an Arc, vtable or pointer. No simulation writes.
    unsafe fn observe_abilities(shared: &Shared, entity: usize) -> Option<StopTicket> {
        let actor = std::ptr::read_unaligned((entity + 0x5c0) as *const usize);
        let ticket = STOP_TICKET.with(|slot| {
            slot.get()
                .filter(|t| t.actor == actor)
                .inspect(|_| slot.set(None))
        })?;
        if !shared.timing.allows_input(ticket.key)
            || std::ptr::read_unaligned((entity + 0x68) as *const u32) != 13
        {
            return None;
        }
        shared.abilities.observe_metadata(
            ticket.key,
            actor,
            read_ability_metadata(entity),
            &shared.logger,
        );
        shared.movement.observe_action(
            std::ptr::read_unaligned((entity + 0x70) as *const usize),
            &shared.logger,
        );
        shared.movement.observe_attack_range(
            ticket.key,
            read_effect_metadata(entity, 0x490)
                .map(|d| d.range)
                .filter(|r| *r > 0),
            &shared.logger,
        );
        Some(ticket)
    }
    unsafe fn read_ability_metadata(entity: usize) -> [Option<crate::abilities::Descriptor>; 3] {
        [0x4c8, 0x500, 0x538].map(|offset| read_effect_metadata(entity, offset))
    }
    unsafe fn read_effect_metadata(
        entity: usize,
        offset: usize,
    ) -> Option<crate::abilities::Descriptor> {
        let level = std::ptr::read_unaligned((entity + 0x5c8) as *const u64);
        let bonus = std::ptr::read_unaligned((entity + 0x438) as *const u64);
        let data = entity + offset;
        let casting = std::ptr::read_unaligned((data + 0x30) as *const u32);
        if casting > 3 {
            return None;
        }
        crate::abilities::Descriptor::from_fields(
            casting,
            std::ptr::read_unaligned((data + 0x28) as *const u32),
            std::ptr::read_unaligned((data + 0x10) as *const u64),
            std::ptr::read_unaligned((data + 0x18) as *const u64),
            level,
            bonus,
        )
    }
    unsafe extern "system" fn attack_hook(entity: usize, input: usize, events: usize) {
        let original: AttackFn = std::mem::transmute(ORIGINAL_ATTACK.load(Ordering::Acquire));
        if let Some(shared) = SHARED.get() {
            if catch_unwind(AssertUnwindSafe(|| {
                if let Some(ticket) = observe_abilities(shared, entity).filter(|t| t.cancel_recall)
                {
                    cancel_recall(shared, entity, ticket.actor, events);
                }
            }))
            .is_err()
            {
                shared
                    .timing
                    .cancel("Ability observation adapter panic", &shared.logger);
            }
        }
        original(entity, input, events);
    }

    unsafe fn can_stop_movement(entity: usize) -> bool {
        if std::ptr::read_unaligned((entity + 0x68) as *const u32) != 13
            || std::ptr::read_unaligned((entity + 0x70) as *const usize) != 2
        {
            return false;
        }
        let count = std::ptr::read_unaligned((entity + 0x2d0) as *const usize);
        if count > 512 {
            return false;
        }
        let effects = std::ptr::read_unaligned((entity + 0x2c8) as *const usize);
        if count != 0 && effects == 0 {
            return false;
        }
        // Preserve the native movement routine's effect gate, including forced
        // movement. Do not clear effects, attacks, casts or other action states.
        (0..count).all(|i| {
            let kind = std::ptr::read_unaligned((effects + i * 0x28) as *const u32);
            kind < 32 && (0x3b8u32 & (1u32 << kind)) != 0
        })
    }
    unsafe fn stop_movement(
        entity: usize,
        actor: usize,
        events: usize,
        notify: StopEventFn,
    ) -> bool {
        if std::ptr::read_unaligned((entity + 0x70) as *const usize) == 0 {
            return true;
        }
        if !can_stop_movement(entity) {
            return false;
        }
        notify(events, actor);
        std::ptr::write_unaligned((entity + 0x70) as *mut usize, 0);
        true
    }
    unsafe fn stop_recall(entity: usize, actor: usize, events: usize, notify: StopEventFn) -> bool {
        if std::ptr::read_unaligned((entity + 0x68) as *const u32) != 13
            || std::ptr::read_unaligned((entity + 0x70) as *const usize) != 1
        {
            return false;
        }
        notify(events, actor);
        std::ptr::write_unaligned((entity + 0x70) as *mut usize, 0);
        true
    }
    unsafe fn cancel_recall(shared: &Shared, entity: usize, actor: usize, events: usize) {
        let notify: StopEventFn = std::mem::transmute(NOTIFY_CANCEL_RECALL.load(Ordering::Acquire));
        if stop_recall(entity, actor, events, notify) {
            shared.movement.observe_action(0, &shared.logger);
            shared.logger.write(&format!("RECALL NATIVE_CANCEL actor={actor} action=1->0; native return cancellation event emitted"));
        }
    }
    unsafe extern "system" fn move_hook(entity: usize, x: u64, y: u64, events: usize) {
        let original: MoveFn = std::mem::transmute(ORIGINAL_MOVE.load(Ordering::Acquire));
        let Some(shared) = SHARED.get() else {
            original(entity, x, y, events);
            return;
        };
        let handled = catch_unwind(AssertUnwindSafe(|| {
            let Some(ticket) = observe_abilities(shared, entity) else {
                return false;
            };
            if ticket.cancel_recall { cancel_recall(shared,entity,ticket.actor,events); }
            if !ticket.hold { return false; }
            let actor = ticket.actor;
            let action = std::ptr::read_unaligned((entity + 0x70) as *const usize);
            let notify: StopEventFn = std::mem::transmute(NOTIFY_STOP.load(Ordering::Acquire));
            let stopped = stop_movement(entity, actor, events, notify);
            if stopped && action == 2 {
                shared.logger.write(&format!(
                    "MANUAL NATIVE_STOP actor={actor} action=2->0; native stop-animation event emitted"
                ));
            }
            stopped
        }))
        .unwrap_or_else(|_| {
            shared
                .timing
                .cancel("Movement stop adapter panic", &shared.logger);
            false
        });
        if !handled {
            original(entity, x, y, events);
        }
    }

    type NativeInputFn = unsafe extern "system" fn(usize, usize, usize, f32, usize, usize, usize);
    fn spectator_key_is_owned(tag: u64, key: u8) -> bool {
        matches!(tag, 0x8000000000000006 | 0x8000000000000007) && !matches!(key, 0x21 | 0x3d | 0x3e)
    }
    unsafe extern "system" fn input_hook(
        view: usize,
        ui: usize,
        system: usize,
        dt: f32,
        event: usize,
        window: usize,
        database: usize,
    ) {
        INPUT_ENTRIES.fetch_add(1, Ordering::Relaxed);
        let original: NativeInputFn = std::mem::transmute(ORIGINAL_INPUT.load(Ordering::Acquire));
        let Some(shared) = SHARED.get() else {
            original(view, ui, system, dt, event, window, database);
            return;
        };
        let owned = catch_unwind(AssertUnwindSafe(|| {
            if !shared.timing.client_controls(Some(view)) || !crate::platform_input::poll().focused
            {
                return false;
            }
            let tag = std::ptr::read_unaligned(event as *const u64);
            if !matches!(tag, 0x8000000000000006 | 0x8000000000000007) {
                return false;
            }
            let key = std::ptr::read((event + 8) as *const u8);
            if spectator_key_is_owned(tag, key) {
                shared.logger.write(&format!(
                    "NATIVE SPECTATOR_KEY suppressed tag={tag:x} key={key:x} bound_view={view:x}"
                ));
                true
            } else {
                false
            }
        }))
        .unwrap_or_else(|_| {
            shared
                .timing
                .cancel("Spectator input adapter panic", &shared.logger);
            false
        });
        if !owned {
            original(view, ui, system, dt, event, window, database);
        }
    }

    struct CameraLease {
        view: usize,
        config: usize,
        original: [u8; 16],
    }
    static CAMERA_LEASE: std::sync::Mutex<Option<CameraLease>> = std::sync::Mutex::new(None);
    pub(super) fn reset_session() {
        if let Ok(mut lease) = CAMERA_LEASE.lock() {
            *lease = None;
        }
    }
    unsafe fn camera_frame(view: usize, config: usize) -> Option<crate::camera::CameraFrame> {
        use crate::camera::{CameraFrame, Rect};
        if std::ptr::read((view + 0x130) as *const u8) != 0 {
            return None;
        }
        let f = |offset| std::ptr::read_unaligned((view + offset) as *const f32);
        let wide = std::ptr::read((config + 0x45) as *const u8) != 0;
        let left = std::ptr::read((config + 0x46) as *const u8) != 0;
        // Mirrored from the CURRENT renderer 1fd61cd..1fd63de and native
        // minimap input caf6d9..cb0061, not archived struct offsets alone.
        let viewport = if wide {
            Rect {
                x: 0.,
                y: 50.,
                w: 1920.,
                h: 974.,
            }
        } else {
            Rect {
                x: if left { f(0xf8) } else { 942. },
                y: f(0xfc),
                w: f(0x100),
                h: f(0x104),
            }
        };
        let minimap = if wide {
            Rect {
                x: 1581.,
                y: 740.,
                w: 320.,
                h: 320.,
            }
        } else {
            Rect {
                x: if left { 1015. } else { 585. },
                y: 731.,
                w: 320.,
                h: 320.,
            }
        };
        let frame = CameraFrame {
            viewport,
            minimap,
            center: (f(0x114), f(0x118)),
            extent: (f(0x11c), f(0x120)),
        };
        frame.valid().then_some(frame)
    }
    unsafe fn restore_camera(view: usize, config: usize, shared: &Shared) {
        if let Ok(mut lease) = CAMERA_LEASE.lock() {
            if lease
                .as_ref()
                .is_some_and(|l| l.view == view && l.config == config)
            {
                let saved = lease.take().unwrap();
                std::ptr::copy_nonoverlapping(
                    saved.original.as_ptr(),
                    (config + 0x18) as *mut u8,
                    16,
                );
                // Keys released while owned must not revive an old pan velocity.
                std::ptr::write_unaligned((view + 0x458) as *mut u64, 0);
                shared.camera.reset();
                shared
                    .logger
                    .write("CAMERA native camera selection restored on release");
            }
        }
    }
    unsafe fn set_follow_config(config: usize, side: usize, lane: u32) {
        // CURRENT 1fcfd5d reads +20 team and +1c lane to look up its player map.
        // Native own-mid shortcut cb020e writes the same pair (team, lane=2).
        std::ptr::write_unaligned((config + 0x18) as *mut u32, 2);
        std::ptr::write_unaligned((config + 0x1c) as *mut u32, lane);
        std::ptr::write_unaligned((config + 0x20) as *mut usize, side);
    }
    unsafe fn prepare_camera(view: usize, config: usize, mode: ViewMode, dt: f32, shared: &Shared) {
        let notches = crate::wheel::take();
        if notches != 0 {
            let current = std::ptr::read_unaligned((view + 0x110) as *const f32);
            if let Some(zoom) = crate::wheel::zoom(current, notches) {
                std::ptr::write_unaligned((view + 0x110) as *mut f32, zoom);
                shared.logger.write(&format!(
                    "CAMERA WHEEL notches={notches} zoom={current}->{zoom}"
                ));
            }
        }
        let Some((target, champion)) = shared.movement.camera_target() else {
            return;
        };
        let Some(frame) = camera_frame(view, config) else {
            return;
        };
        if let Ok(mut lease) = CAMERA_LEASE.lock() {
            if lease.is_none() {
                let mut original = [0u8; 16];
                std::ptr::copy_nonoverlapping(
                    (config + 0x18) as *const u8,
                    original.as_mut_ptr(),
                    16,
                );
                *lease = Some(CameraLease {
                    view,
                    config,
                    original,
                });
                shared.logger.write(&format!(
                    "CAMERA BOUND frame={frame:?}; native minimap/zoom retained"
                ));
            } else if !lease
                .as_ref()
                .is_some_and(|l| l.view == view && l.config == config)
            {
                return;
            }
        } else {
            return;
        }
        std::ptr::write_unaligned((view + 0x458) as *mut u64, 0);
        let native_mode = std::ptr::read_unaligned((config + 0x18) as *const u32);
        let request = shared.camera.step(
            frame,
            native_mode,
            crate::platform_input::poll(),
            target.player,
            champion,
            mode == ViewMode::Running,
            dt,
            &shared.logger,
        );
        match request {
            Some(crate::camera::Request::Free((x, y))) => {
                std::ptr::write_unaligned((config + 0x18) as *mut u32, 1);
                std::ptr::write_unaligned((config + 0x1c) as *mut f32, x);
                std::ptr::write_unaligned((config + 0x20) as *mut f32, y);
            }
            Some(crate::camera::Request::Follow(player)) => {
                let Some(champion) = champion else { return };
                if player != target.player || target.side > 1 || target.lane >= 5 {
                    return;
                }
                if native_mode != 2
                    || std::ptr::read_unaligned((config + 0x1c) as *const u32) != target.lane as u32
                    || std::ptr::read_unaligned((config + 0x20) as *const usize) != target.side
                {
                    // Recenter on entry even if playback dt is zero while held.
                    std::ptr::write_unaligned(
                        (view + 0x114) as *mut f32,
                        champion.0 as f32 / 1000.,
                    );
                    std::ptr::write_unaligned(
                        (view + 0x118) as *mut f32,
                        champion.1 as f32 / 1000.,
                    );
                }
                set_follow_config(config, target.side, target.lane as u32);
            }
            None => {}
        }
    }

    unsafe extern "system" fn worker_hook(output: usize, sender: usize, frame: usize) {
        STOP_TICKET.set(None);
        WORKER_ENTRIES.fetch_add(1, Ordering::Relaxed);
        let original: WorkerFn = std::mem::transmute(ORIGINAL_WORKER.load(Ordering::Acquire));
        // Publish before counting or waiting. This call site is AFTER runner,
        // highlights and highlight-segments write guards have been released.
        original(output, sender, frame);
        if let Some(shared) = SHARED.get() {
            if catch_unwind(AssertUnwindSafe(|| {
                let (accepted, trace) = shared.timing.hook_entry(true, &shared.logger);
                if trace {
                    capture_trace("native worker entrance", &shared.logger);
                }
                if accepted {
                    // Native be4710 checks the same niche sentinel: -1 means
                    // successful send; other values carry the unsent frame.
                    let sent = std::ptr::read_unaligned(output as *const usize) == usize::MAX;
                    if sent {
                        shared.timing.after_publication(sender, &shared.logger);
                    } else {
                        shared
                            .timing
                            .cancel("Native frame send failed", &shared.logger);
                    }
                }
            }))
            .is_err()
            {
                shared.timing.cancel("Worker adapter panic", &shared.logger);
            }
        }
    }

    /// Snapshot only these three config fields; their borrow belongs to the
    /// native caller and spans this call. Restore before that borrow is released.
    struct PlaybackOverride {
        config: usize,
        sync: u8,
        mode: u32,
        speed: u32,
    }
    impl PlaybackOverride {
        unsafe fn apply(config: usize) -> Self {
            let snapshot = Self {
                config,
                sync: std::ptr::read(config as *const u8),
                mode: std::ptr::read_unaligned((config + 0x10) as *const u32),
                speed: std::ptr::read_unaligned((config + 0x14) as *const u32),
            };
            std::ptr::write(config as *mut u8, 0);
            std::ptr::write_unaligned((config + 0x10) as *mut u32, 0);
            std::ptr::write_unaligned((config + 0x14) as *mut f32, 1.0);
            snapshot
        }
    }
    impl Drop for PlaybackOverride {
        fn drop(&mut self) {
            unsafe {
                std::ptr::write(self.config as *mut u8, self.sync);
                std::ptr::write_unaligned((self.config + 0x10) as *mut u32, self.mode);
                std::ptr::write_unaligned((self.config + 0x14) as *mut u32, self.speed);
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe extern "system" fn view_hook(
        view: usize,
        ui: usize,
        system: usize,
        config: usize,
        assets: usize,
        systems: usize,
        tps: usize,
        runner: usize,
        dt: f32,
    ) {
        VIEW_ENTRIES.fetch_add(1, Ordering::Relaxed);
        let original: ViewFn = std::mem::transmute(ORIGINAL_VIEW.load(Ordering::Acquire));
        let Some(shared) = SHARED.get() else {
            original(view, ui, system, config, assets, systems, tps, runner, dt);
            return;
        };
        let accepted = catch_unwind(AssertUnwindSafe(|| {
            let (accepted, trace) = shared.timing.hook_entry(false, &shared.logger);
            if trace {
                capture_trace("native viewer entrance", &shared.logger);
            }
            accepted
        }))
        .unwrap_or_else(|_| {
            shared
                .timing
                .cancel("Viewer entrance diagnostic panic", &shared.logger);
            false
        });
        if !accepted {
            restore_camera(view, config, shared);
            original(view, ui, system, config, assets, systems, tps, runner, dt);
            return;
        }
        let played = std::ptr::read_unaligned((view + 0x290) as *const usize);
        let queued = std::ptr::read_unaligned((view + 0x70) as *const usize);
        let mode = catch_unwind(AssertUnwindSafe(|| {
            shared
                .timing
                .before_view(view, played, queued, &shared.logger)
        }))
        .unwrap_or_else(|_| {
            shared.timing.cancel("Viewer adapter panic", &shared.logger);
            ViewMode::Native
        });
        if mode == ViewMode::Native {
            restore_camera(view, config, shared);
            original(view, ui, system, config, assets, systems, tps, runner, dt);
            return;
        }
        if tps != 60 || queued > 10000 {
            shared.timing.cancel(
                "Unexpected viewer frame rate or queue layout",
                &shared.logger,
            );
            original(view, ui, system, config, assets, systems, tps, runner, dt);
            return;
        }
        let snapshot = PlaybackOverride::apply(config);
        if mode != ViewMode::Bootstrap
            && catch_unwind(AssertUnwindSafe(|| {
                prepare_camera(view, config, mode, dt, shared)
            }))
            .is_err()
        {
            shared.timing.cancel("Camera adapter panic", &shared.logger);
        }
        let actual_dt = match mode {
            ViewMode::Bootstrap => {
                std::ptr::write_unaligned((view + 0x358) as *mut f32, 0.0);
                1.0 / tps as f32
            }
            ViewMode::Paused => {
                std::ptr::write_unaligned((view + 0x358) as *mut f32, 0.0);
                0.0
            }
            ViewMode::Running => dt.min(2.0 / tps as f32),
            ViewMode::Native => dt,
        };
        original(
            view, ui, system, config, assets, systems, tps, runner, actual_dt,
        );
        drop(snapshot);
        if let Some(frame) = camera_frame(view, config) {
            shared.camera.capture(frame);
        }
        let remaining = std::ptr::read_unaligned((view + 0x70) as *const usize);
        let played_after = std::ptr::read_unaligned((view + 0x290) as *const usize);
        if mode != ViewMode::Running {
            std::ptr::write_unaligned((view + 0x358) as *mut f32, 0.0);
        }
        if catch_unwind(AssertUnwindSafe(|| {
            shared
                .timing
                .after_view(mode, queued, remaining, played_after, &shared.logger)
        }))
        .is_err()
        {
            shared
                .timing
                .cancel("Viewer acknowledgement panic", &shared.logger);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        static RECEIVED: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());
        unsafe extern "system" fn fake_worker(a: usize, b: usize, c: usize) {
            *RECEIVED.lock().unwrap() = vec![a as u64, b as u64, c as u64];
            // Model send's sret output so forwarding verifies output memory too.
            std::ptr::write(a as *mut usize, usize::MAX);
        }
        unsafe extern "system" fn fake_move(a: usize, b: u64, c: u64, d: usize) {
            *RECEIVED.lock().unwrap() = vec![a as u64, b, c, d as u64];
        }
        unsafe extern "system" fn fake_attack(a: usize, b: usize, c: usize) {
            *RECEIVED.lock().unwrap() = vec![a as u64, b as u64, c as u64];
        }
        #[test]
        fn native_effect_reader_uses_three_skill_slots_and_rejects_absent_effects() {
            let mut bytes = [0xccu8; 0x6c0];
            // Set distinct metadata with deliberately meaningless Arc/vtable
            // bytes: the reader must never dereference or clone those fields.
            bytes[0x5c8..0x5d0].copy_from_slice(&3u64.to_le_bytes());
            bytes[0x438..0x440].copy_from_slice(&2_000u64.to_le_bytes());
            for (slot, offset) in [0x4c8, 0x500, 0x538].into_iter().enumerate() {
                bytes[offset + 0x10..offset + 0x18]
                    .copy_from_slice(&(70_000u64 + slot as u64 * 10_000).to_le_bytes());
                bytes[offset + 0x18..offset + 0x20].copy_from_slice(&5_000u64.to_le_bytes());
                bytes[offset + 0x28..offset + 0x2c]
                    .copy_from_slice(&(6u32 + slot as u32).to_le_bytes());
                bytes[offset + 0x30..offset + 0x34].copy_from_slice(&(slot as u32).to_le_bytes());
            }
            let values = unsafe { read_ability_metadata(bytes.as_ptr() as usize) };
            for (slot, value) in values.into_iter().enumerate() {
                assert_eq!(
                    value.unwrap(),
                    crate::abilities::Descriptor {
                        casting: slot as u32,
                        target: 6 + slot as u32,
                        range: 82_000 + slot as u64 * 10_000
                    }
                );
            }
            bytes[0x4f8..0x4fc].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(unsafe { read_ability_metadata(bytes.as_ptr() as usize) }[0].is_none());
        }
        #[test]
        fn basic_attack_metadata_uses_current_effect_growth_and_bonus_without_pointer_calls() {
            let mut bytes = [0xccu8; 0x6c0];
            bytes[0x5c8..0x5d0].copy_from_slice(&3u64.to_le_bytes());
            bytes[0x438..0x440].copy_from_slice(&2_000u64.to_le_bytes());
            bytes[0x4a0..0x4a8].copy_from_slice(&60_000u64.to_le_bytes());
            bytes[0x4a8..0x4b0].copy_from_slice(&5_000u64.to_le_bytes());
            bytes[0x4b8..0x4bc].copy_from_slice(&6u32.to_le_bytes());
            bytes[0x4c0..0x4c4].copy_from_slice(&0u32.to_le_bytes());
            let entity = bytes.as_ptr() as usize;
            assert_eq!(
                unsafe { read_effect_metadata(entity, 0x490) }
                    .unwrap()
                    .range,
                72_000
            );
            bytes[0x4a0..0x4a8].copy_from_slice(&u64::MAX.to_le_bytes());
            assert!(unsafe { read_effect_metadata(bytes.as_ptr() as usize, 0x490) }.is_none());
            bytes[0x4a0..0x4a8].copy_from_slice(&60_000u64.to_le_bytes());
            bytes[0x4c0..0x4c4].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(unsafe { read_effect_metadata(bytes.as_ptr() as usize, 0x490) }.is_none());
        }
        #[allow(clippy::too_many_arguments)]
        unsafe extern "system" fn fake_view(
            a: usize,
            b: usize,
            c: usize,
            d: usize,
            e: usize,
            f: usize,
            g: usize,
            h: usize,
            i: f32,
        ) {
            *RECEIVED.lock().unwrap() = vec![
                a as u64,
                b as u64,
                c as u64,
                d as u64,
                e as u64,
                f as u64,
                g as u64,
                h as u64,
                i.to_bits() as u64,
            ];
        }
        unsafe extern "system" fn fake_input(
            a: usize,
            b: usize,
            c: usize,
            dt: f32,
            e: usize,
            f: usize,
            g: usize,
        ) {
            *RECEIVED.lock().unwrap() = vec![
                a as u64,
                b as u64,
                c as u64,
                dt.to_bits() as u64,
                e as u64,
                f as u64,
                g as u64,
            ];
        }
        #[test]
        fn follow_payload_uses_team_and_lane_instead_of_sdk_player_id() {
            for side in 0..2 {
                for lane in 0..5 {
                    let mut config = [0xccu8; 0x60];
                    let addr = config.as_mut_ptr() as usize;
                    unsafe {
                        set_follow_config(addr, side, lane);
                    }
                    assert_eq!(
                        u32::from_le_bytes(config[0x18..0x1c].try_into().unwrap()),
                        2
                    );
                    assert_eq!(
                        u32::from_le_bytes(config[0x1c..0x20].try_into().unwrap()),
                        lane
                    );
                    assert_eq!(
                        usize::from_le_bytes(config[0x20..0x28].try_into().unwrap()),
                        side
                    );
                    assert!(config[..0x18]
                        .iter()
                        .chain(config[0x28..].iter())
                        .all(|b| *b == 0xcc));
                }
            }
        }
        #[test]
        fn spectator_filter_covers_press_release_and_preserves_mouse_zoom() {
            for tag in [0x8000000000000006, 0x8000000000000007] {
                assert!(spectator_key_is_owned(tag, 0x17)); // default S pause shortcut
                assert!(spectator_key_is_owned(tag, 4)); // default Tab info shortcut
                assert!(!spectator_key_is_owned(tag, 0x3d));
                assert!(!spectator_key_is_owned(tag, 0x3e));
                assert!(!spectator_key_is_owned(tag, 0x21));
            }
            for tag in [
                0,
                0x8000000000000001,
                0x8000000000000003,
                0x8000000000000004,
                0x8000000000000005,
                0x800000000000000d,
            ] {
                assert!(!spectator_key_is_owned(tag, 0x17));
            }
        }
        #[test]
        fn windows_hash_rel32_and_forwarding_abi() {
            assert_eq!(
                sha256(b"abc").unwrap(),
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
            );
            assert_eq!(
                relative_call(WORKER_SITE, WORKER_ORIGINAL).unwrap(),
                WORKER_BYTES
            );
            assert_eq!(relative_call(VIEW_SITE, VIEW_ORIGINAL).unwrap(), VIEW_BYTES);
            assert_eq!(relative_jump(MOVE_SITE, MOVE_ORIGINAL).unwrap(), MOVE_BYTES);
            assert_eq!(
                relative_call(INPUT_SITE, INPUT_ORIGINAL).unwrap(),
                INPUT_BYTES
            );
            assert_eq!(
                relative_call(ATTACK_SITE, ATTACK_ORIGINAL).unwrap(),
                ATTACK_BYTES
            );
            ORIGINAL_ATTACK.store(fake_attack as *const () as usize, Ordering::Release);
            unsafe {
                attack_hook(11, 22, 33);
            }
            assert_eq!(*RECEIVED.lock().unwrap(), vec![11, 22, 33]);
            let attack_relay = unsafe {
                relay(
                    fake_attack as *const () as usize,
                    fake_attack as *const () as usize,
                )
            }
            .unwrap();
            let relay_attack: AttackFn = unsafe { std::mem::transmute(attack_relay) };
            unsafe {
                relay_attack(44, 55, 66);
            }
            assert_eq!(*RECEIVED.lock().unwrap(), vec![44, 55, 66]);
            ORIGINAL_INPUT.store(fake_input as *const () as usize, Ordering::Release);
            unsafe {
                input_hook(1, 2, 3, 0.0125, 5, 6, 7);
            }
            assert_eq!(
                *RECEIVED.lock().unwrap(),
                vec![1, 2, 3, 0.0125f32.to_bits() as u64, 5, 6, 7]
            );
            let input_relay = unsafe {
                relay(
                    fake_input as *const () as usize,
                    fake_input as *const () as usize,
                )
            }
            .unwrap();
            let relay_input: NativeInputFn = unsafe { std::mem::transmute(input_relay) };
            unsafe {
                relay_input(7, 6, 5, 0.025, 3, 2, 1);
            }
            assert_eq!(
                *RECEIVED.lock().unwrap(),
                vec![7, 6, 5, 0.025f32.to_bits() as u64, 3, 2, 1]
            );
            assert!(relative_call(0, usize::MAX).is_err());
            ORIGINAL_WORKER.store(fake_worker as *const () as usize, Ordering::Release);
            ORIGINAL_VIEW.store(fake_view as *const () as usize, Ordering::Release);
            ORIGINAL_MOVE.store(fake_move as *const () as usize, Ordering::Release);
            unsafe {
                move_hook(11, 22, 33, 44);
            }
            assert_eq!(*RECEIVED.lock().unwrap(), vec![11, 22, 33, 44]);
            unsafe {
                let mut sent = 0usize;
                worker_hook(&mut sent as *mut usize as usize, 22, 33);
                assert_eq!(sent, usize::MAX);
                assert_eq!(
                    *RECEIVED.lock().unwrap(),
                    vec![&mut sent as *mut usize as u64, 22, 33]
                );
            }
            let worker_relay = unsafe {
                relay(
                    fake_worker as *const () as usize,
                    fake_worker as *const () as usize,
                )
            }
            .unwrap();
            let relay_worker: WorkerFn = unsafe { std::mem::transmute(worker_relay) };
            unsafe {
                let mut sent = 0usize;
                relay_worker(&mut sent as *mut usize as usize, 55, 66);
                assert_eq!(sent, usize::MAX);
                assert_eq!(
                    *RECEIVED.lock().unwrap(),
                    vec![&mut sent as *mut usize as u64, 55, 66]
                );
            }
            // SHARED is intentionally absent: passthrough preserves all nine
            // arguments, including the fifth pointer and ninth stack f32 slot.
            unsafe {
                view_hook(1, 2, 3, 4, 5, 6, 60, 8, 0.0125);
            }
            assert_eq!(
                *RECEIVED.lock().unwrap(),
                vec![1, 2, 3, 4, 5, 6, 60, 8, 0.0125f32.to_bits() as u64]
            );
            let view_relay = unsafe {
                relay(
                    fake_view as *const () as usize,
                    fake_view as *const () as usize,
                )
            }
            .unwrap();
            let relay_view: ViewFn = unsafe { std::mem::transmute(view_relay) };
            unsafe {
                relay_view(9, 8, 7, 6, 5, 4, 60, 2, 0.025);
            }
            assert_eq!(
                *RECEIVED.lock().unwrap(),
                vec![9, 8, 7, 6, 5, 4, 60, 2, 0.025f32.to_bits() as u64]
            );
        }
        #[test]
        fn stop_gate_preserves_nonmovement_actions_and_forced_movement() {
            let mut actor = vec![0u64; 0x6c0 / 8];
            let address = actor.as_mut_ptr() as usize;
            let mut effects = [0u64; 5];
            unsafe {
                std::ptr::write_unaligned((address + 0x68) as *mut u32, 13);
                for action in 0..7 {
                    std::ptr::write_unaligned((address + 0x70) as *mut usize, action);
                    assert_eq!(can_stop_movement(address), action == 2);
                }
                std::ptr::write_unaligned((address + 0x70) as *mut usize, 2);
                std::ptr::write_unaligned(
                    (address + 0x2c8) as *mut usize,
                    effects.as_mut_ptr() as usize,
                );
                std::ptr::write_unaligned((address + 0x2d0) as *mut usize, 1);
                std::ptr::write_unaligned(effects.as_mut_ptr(), 6);
                assert!(!can_stop_movement(address));
                std::ptr::write_unaligned(effects.as_mut_ptr(), 3);
                assert!(can_stop_movement(address));
                std::ptr::write_unaligned((address + 0x68) as *mut u32, 0);
                assert!(!can_stop_movement(address));
            }
        }
        #[test]
        fn stop_emits_native_event_once_and_clears_only_movement() {
            unsafe extern "system" fn notify(events: usize, actor: usize) {
                let seen = &mut *(events as *mut Vec<usize>);
                seen.push(actor);
            }
            let mut entity = vec![0u64; 0x6c0 / 8];
            let address = entity.as_mut_ptr() as usize;
            let mut seen = Vec::<usize>::new();
            let events = &mut seen as *mut Vec<usize> as usize;
            unsafe {
                std::ptr::write_unaligned((address + 0x68) as *mut u32, 13);
                std::ptr::write_unaligned((address + 0x70) as *mut usize, 2);
                assert!(stop_movement(address, 17, events, notify));
                assert_eq!(
                    std::ptr::read_unaligned((address + 0x70) as *const usize),
                    0
                );
                assert_eq!(seen, vec![17]);
                assert!(stop_movement(address, 17, events, notify));
                assert_eq!(seen, vec![17]);
                for action in 3..7 {
                    std::ptr::write_unaligned((address + 0x70) as *mut usize, action);
                    assert!(!stop_movement(address, 17, events, notify));
                    assert_eq!(
                        std::ptr::read_unaligned((address + 0x70) as *const usize),
                        action
                    );
                }
                assert_eq!(seen, vec![17]);
            }
        }
        #[test]
        fn recall_stop_emits_once_and_preserves_timer_effects_and_other_actions() {
            unsafe extern "system" fn notify(events: usize, actor: usize) {
                (&mut *(events as *mut Vec<usize>)).push(actor);
            }
            let mut entity = vec![0xccu8; 0x6c0];
            let addr = entity.as_mut_ptr() as usize;
            entity[0x68..0x6c].copy_from_slice(&13u32.to_le_bytes());
            entity[0x70..0x78].copy_from_slice(&1usize.to_le_bytes());
            let before = entity.clone();
            let mut seen = Vec::<usize>::new();
            let events = &mut seen as *mut Vec<usize> as usize;
            unsafe {
                assert!(stop_recall(addr, 17, events, notify));
                assert_eq!(seen, vec![17]);
                assert_eq!(
                    usize::from_le_bytes(entity[0x70..0x78].try_into().unwrap()),
                    0
                );
                assert_eq!(&entity[..0x70], &before[..0x70]);
                assert_eq!(&entity[0x78..], &before[0x78..]);
                assert!(!stop_recall(addr, 17, events, notify));
                for action in [2usize, 3, 4, 5, 6] {
                    std::ptr::write_unaligned((addr + 0x70) as *mut usize, action);
                    assert!(!stop_recall(addr, 17, events, notify));
                    assert_eq!(
                        std::ptr::read_unaligned((addr + 0x70) as *const usize),
                        action
                    );
                }
                assert_eq!(seen, vec![17]);
            }
        }
        #[test]
        fn scoped_config_restores_every_overridden_bit() {
            let mut config = [0u8; 0x60];
            config[0] = 1;
            config[0x10..0x14].copy_from_slice(&1u32.to_le_bytes());
            config[0x14..0x18].copy_from_slice(&3.0f32.to_bits().to_le_bytes());
            let before = config;
            let snapshot = unsafe { PlaybackOverride::apply(config.as_mut_ptr() as usize) };
            assert_eq!(config[0], 0);
            assert_eq!(
                u32::from_le_bytes(config[0x14..0x18].try_into().unwrap()),
                1.0f32.to_bits()
            );
            drop(snapshot);
            assert_eq!(config, before);
        }
        #[test]
        fn stack_capture_is_available_and_logs_module_relative_frames() {
            let log = crate::timing_test::tests::logger("native-stack");
            capture_trace("test stack", &log);
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("target/timing-native-stack.log");
            let text = std::fs::read_to_string(path).unwrap();
            assert!(text.contains("NATIVE STACK"));
            assert!(text.contains(".exe+0x"));
            assert!(!text.contains("frames=[]"));
        }
    }
}
