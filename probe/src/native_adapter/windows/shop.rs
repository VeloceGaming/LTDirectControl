//! Native buyer hooks for Manual shopping, and reads of player items,
//! build plan, gold and the purchase-slot gate.
use super::*;

pub(crate) unsafe fn player_pointer(state: usize, table: usize, id: usize) -> Option<usize> {
    if PATCHES.get().is_none() || state == 0 || id == usize::MAX {
        return None;
    }
    let base = GetModuleHandleW(std::ptr::null()) as usize;
    if table == 0
        || std::ptr::read_unaligned(table as *const usize) < 240
        || std::ptr::read_unaligned((table + 232) as *const usize) != base + GOLD_GETTER
    {
        return None;
    }
    let object = std::ptr::read_unaligned(state as *const usize);
    let vtable = std::ptr::read_unaligned((state + 8) as *const usize);
    if object == 0 || vtable == 0 {
        return None;
    }
    let get: unsafe extern "system" fn(usize, usize) -> usize =
        std::mem::transmute(std::ptr::read_unaligned((vtable + 0x158) as *const usize));
    Some(get(object, id)).filter(|player| *player != 0)
}

// Purchase executor 1465e80 indexes owned items at +310 (pointer) and
// bounds them by +318 (length) before each upgrade (146b4b0/146b4cb).
pub unsafe fn player_owned_len(state: usize, table: usize, id: usize) -> Option<usize> {
    let player = player_pointer(state, table, id)?;
    let ptr = std::ptr::read_unaligned((player + 0x310) as *const usize);
    let len = std::ptr::read_unaligned((player + 0x318) as *const usize);
    (len <= 64 && (len == 0 || (ptr != 0 && ptr.is_multiple_of(8)))).then_some(len)
}

pub(crate) unsafe fn module_offset(address: usize) -> String {
    let mut module = std::ptr::null_mut();
    if GetModuleHandleExW(0x06, address as *const u16, &mut module) == 0 {
        return format!("unmapped@{address:x}");
    }
    let mut name = [0u16; 512];
    let length = GetModuleFileNameW(module, name.as_mut_ptr(), name.len() as u32) as usize;
    let path = OsString::from_wide(&name[..length.min(name.len())]);
    let filename = std::path::Path::new(&path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    format!("{filename}+0x{:x}", address - module as usize)
}

pub fn buyer_anchor_report() -> Vec<String> {
    if PATCHES.get().is_none() {
        return vec!["SHOP ANCHORS skipped: host identity not verified".into()];
    }
    unsafe {
        let base = GetModuleHandleW(std::ptr::null()) as usize;
        let mut lines = Vec::new();
        for (name, rva, expected) in BUYER_CODE {
            let live = std::slice::from_raw_parts((base + rva) as *const u8, expected.len());
            if live == expected {
                lines.push(format!("SHOP ANCHOR {name} rva=0x{rva:x} native"));
                continue;
            }
            // A leading E9/E8 is a redirect; name its destination.
            let target = matches!(live[0], 0xe8 | 0xe9).then(|| {
                let rel = std::ptr::read_unaligned((base + rva + 1) as *const i32);
                module_offset((base + rva + 5).wrapping_add_signed(rel as isize))
            });
            lines.push(format!(
                "SHOP ANCHOR {name} rva=0x{rva:x} CHANGED live={live:02x?} expected={expected:02x?} redirect={target:?}"
            ));
        }
        for (name, rva, thunk) in BUYER_SLOTS {
            let live = std::ptr::read_unaligned((base + rva) as *const usize);
            if live == base + thunk {
                lines.push(format!("SHOP ANCHOR {name} rva=0x{rva:x} native"));
            } else {
                lines.push(format!(
                    "SHOP ANCHOR {name} rva=0x{rva:x} CHANGED points_to={}",
                    module_offset(live)
                ));
            }
        }
        lines
    }
}

pub unsafe fn native_player(state: usize, table: usize, id: usize) -> Option<usize> {
    player_pointer(state, table, id)
}

pub(crate) static ORIGINAL_SHOP_UPGRADE: AtomicUsize = AtomicUsize::new(0);
pub(crate) static ORIGINAL_SHOP_NEW: AtomicUsize = AtomicUsize::new(0);
pub(crate) static SHOP_READY: AtomicBool = AtomicBool::new(false);
pub(crate) type ShopUpgradeFn =
    unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, usize) -> usize;

pub(crate) fn item_slot_capacity() -> Option<usize> {
    PATCHES.get()?;
    unsafe {
        let base = GetModuleHandleW(std::ptr::null()) as usize;
        let gate = std::slice::from_raw_parts((base + 0x146baf3) as *const u8, 6);
        // cmp rax, imm8 ; ja rel32
        (gate[..3] == [0x48, 0x83, 0xf8] && gate[4..6] == [0x0f, 0x87] && gate[3] < 64)
            .then(|| gate[3] as usize + 1)
    }
}
pub(crate) fn shop_ready() -> bool {
    SHOP_READY.load(Ordering::Acquire)
}

pub(crate) unsafe extern "system" fn shop_upgrade_hook(
    out: usize,
    this: usize,
    a3: usize,
    player: usize,
    a5: usize,
    a6: usize,
    a7: usize,
) -> usize {
    use crate::shop::Answer;
    let _watch = crate::worker_watch::hook(crate::worker_watch::Step::ShopHook);
    let answer =
        catch_unwind(|| crate::shop::SHOP.upgrade_answer(player)).unwrap_or(Answer::Native);
    let fields: [u64; 3] = match answer {
        Answer::Native => {
            let original: ShopUpgradeFn =
                std::mem::transmute(ORIGINAL_SHOP_UPGRADE.load(Ordering::Acquire));
            return original(out, this, a3, player, a5, a6, a7);
        }
        Answer::Upgrade { slot, item } => [1, slot as u64, item as u64],
        Answer::Nothing | Answer::New { .. } | Answer::AskGameFirst => [0, 0, 0],
    };
    for (i, v) in fields.into_iter().enumerate() {
        std::ptr::write_unaligned((out + i * 8) as *mut u64, v);
    }
    out
}

/// The build plan's length (+0x360 of the player object, as in
/// `player_build`), when it looks sane.
unsafe fn build_len(player: usize) -> Option<usize> {
    let len = std::ptr::read_unaligned((player + 0x360) as *const usize);
    (player != 0 && len <= 64).then_some(len)
}
std::thread_local! {
    /// Build length before the game's one-time decision (same worker thread).
    static BUILD_BEFORE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}
/// 0 = answer natively; 1 = `out` holds {kind, item} for RAX/RDX; 2 = call
/// the game's decision once with the same arguments, discard its answer and
/// ask again (`shop_new_after`).
pub(crate) unsafe extern "system" fn shop_new_decide(player: usize, out: *mut [u64; 2]) -> u32 {
    use crate::shop::Answer;
    let answer =
        catch_unwind(|| crate::shop::SHOP.new_item_answer(player)).unwrap_or(Answer::Native);
    let fields = match answer {
        Answer::Native => return 0,
        Answer::AskGameFirst => {
            BUILD_BEFORE.with(|b| b.set(build_len(player)));
            return 2;
        }
        Answer::New { item } => [1, item as u64],
        Answer::Nothing | Answer::Upgrade { .. } => [0, 0],
    };
    std::ptr::write_unaligned(out, fields);
    1
}
/// After the game's one-time decision: record it, then answer as usual.
pub(crate) unsafe extern "system" fn shop_new_after(
    player: usize,
    kind: u64,
    item: u64,
    out: *mut [u64; 2],
) -> u32 {
    let before = BUILD_BEFORE.with(|b| b.take());
    let after = build_len(player);
    let wanted = (kind == 1).then_some(item as usize);
    let _ = catch_unwind(|| crate::shop::SHOP.game_decided(wanted, (before, after)));
    match shop_new_decide(player, out) {
        1 => 1,
        // Never a second pass-through: "native" now means the answer the
        // game already gave; another one-time request declines.
        0 => {
            std::ptr::write_unaligned(out, [kind, item]);
            1
        }
        _ => {
            std::ptr::write_unaligned(out, [0, 0]);
            1
        }
    }
}

// Native ABI: RCX this, RDX, R8 player, R9, two stack args (call site
// 0x146b959 stores them at [rsp+0x20]/[rsp+0x28]); returns a pair in
// RAX:RDX. The shim keeps all argument registers and stack arguments for
// the pass-through jump, so the original runs exactly as if called. Stack:
// rcx/rdx/r8/r9 saved at +0x50/+0x48/+0x40/+0x38, the caller's stack
// arguments at +0x80/+0x88; `out` at +0x20; rsp is 16-aligned at each call.
#[unsafe(naked)]
pub(crate) unsafe extern "system" fn shop_new_hook() {
    std::arch::naked_asm!(
        "push rcx",
        "push rdx",
        "push r8",
        "push r9",
        "sub rsp, 0x38",
        "mov rcx, r8",
        "lea rdx, [rsp + 0x20]",
        "call {decide}",
        "cmp eax, 2",
        "jne 3f",
        // The game's decision with the caller's six arguments: four saved
        // registers and the two stack arguments above the return address.
        "mov rax, qword ptr [rsp + 0x80]",
        "mov qword ptr [rsp + 0x20], rax",
        "mov rax, qword ptr [rsp + 0x88]",
        "mov qword ptr [rsp + 0x28], rax",
        "mov rcx, qword ptr [rsp + 0x50]",
        "mov rdx, qword ptr [rsp + 0x48]",
        "mov r8, qword ptr [rsp + 0x40]",
        "mov r9, qword ptr [rsp + 0x38]",
        "call qword ptr [rip + {original}]",
        "mov rcx, qword ptr [rsp + 0x40]",
        "mov r8, rdx",
        "mov rdx, rax",
        "lea r9, [rsp + 0x20]",
        "call {after}",
        "3:",
        "test eax, eax",
        "jz 2f",
        "mov rax, qword ptr [rsp + 0x20]",
        "mov rdx, qword ptr [rsp + 0x28]",
        "add rsp, 0x58",
        "ret",
        "2:",
        "add rsp, 0x38",
        "pop r9",
        "pop r8",
        "pop rdx",
        "pop rcx",
        "jmp qword ptr [rip + {original}]",
        decide = sym shop_new_decide,
        after = sym shop_new_after,
        original = sym ORIGINAL_SHOP_NEW,
    );
}

/// Optional: a failure leaves Manual shopping unavailable and every
/// other hook installed.
pub(crate) unsafe fn install_shop(base: usize) -> Result<(), String> {
    for (site, expected) in [
        (SHOP_UPGRADE_THUNK, SHOP_UPGRADE_BYTES),
        (SHOP_NEW_THUNK, SHOP_NEW_BYTES),
    ] {
        if read_call(base + site) != expected {
            return Err(format!("buyer thunk 0x{site:x} differs (another mod?)"));
        }
    }
    for (slot, thunk) in SHOP_SLOTS {
        if std::ptr::read_unaligned((base + slot) as *const usize) != base + thunk {
            return Err(format!(
                "controller slot 0x{slot:x} no longer uses its thunk"
            ));
        }
    }
    let target = |site: usize| {
        let rel = std::ptr::read_unaligned((base + site + 1) as *const i32);
        (base + site + 5).wrapping_add_signed(rel as isize)
    };
    ORIGINAL_SHOP_UPGRADE.store(target(SHOP_UPGRADE_THUNK), Ordering::Release);
    ORIGINAL_SHOP_NEW.store(target(SHOP_NEW_THUNK), Ordering::Release);
    let mut patches = Vec::new();
    for (site, expected, hook) in [
        (
            SHOP_UPGRADE_THUNK,
            SHOP_UPGRADE_BYTES,
            shop_upgrade_hook as *const () as usize,
        ),
        (
            SHOP_NEW_THUNK,
            SHOP_NEW_BYTES,
            shop_new_hook as *const () as usize,
        ),
    ] {
        let site = base + site;
        let relay = relay(site, hook)?;
        patches.push((site, expected, relative_jump(site, relay)?));
    }
    for (site, expected, patch) in &patches {
        let mut old = 0;
        if VirtualProtect(*site as *mut c_void, 5, 0x40, &mut old) == 0 {
            return Err("cannot unprotect buyer thunk".into());
        }
        std::ptr::copy_nonoverlapping(patch.as_ptr(), *site as *mut u8, 5);
        let applied = read_call(*site) == *patch;
        if !applied {
            std::ptr::copy_nonoverlapping(expected.as_ptr(), *site as *mut u8, 5);
        }
        FlushInstructionCache(GetCurrentProcess(), *site as *const c_void, 5);
        let mut ignored = 0;
        VirtualProtect(*site as *mut c_void, 5, old, &mut ignored);
        if !applied {
            return Err("buyer thunk readback mismatch; original restored".into());
        }
    }
    SHOP_READY.store(true, Ordering::Release);
    Ok(())
}

pub unsafe fn player_build(state: usize, table: usize, id: usize) -> Option<Vec<usize>> {
    if PATCHES.get().is_none() || state == 0 || id == usize::MAX {
        return None;
    }
    let base = GetModuleHandleW(std::ptr::null()) as usize;
    if table == 0
        || std::ptr::read_unaligned(table as *const usize) < 240
        || std::ptr::read_unaligned((table + 232) as *const usize) != base + GOLD_GETTER
    {
        return None;
    }
    let object = std::ptr::read_unaligned(state as *const usize);
    let vtable = std::ptr::read_unaligned((state + 8) as *const usize);
    if object == 0 || vtable == 0 {
        return None;
    }
    let get: unsafe extern "system" fn(usize, usize) -> usize =
        std::mem::transmute(std::ptr::read_unaligned((vtable + 0x158) as *const usize));
    let player = get(object, id);
    if player == 0 {
        return None;
    }
    let cap = std::ptr::read_unaligned((player + 0x350) as *const usize);
    let ptr = std::ptr::read_unaligned((player + 0x358) as *const usize);
    let len = std::ptr::read_unaligned((player + 0x360) as *const usize);
    if len > 64 || cap < len || cap > 256 || ptr == 0 || !ptr.is_multiple_of(8) {
        return None;
    }
    Some(std::slice::from_raw_parts(ptr as *const usize, len).to_vec())
}
