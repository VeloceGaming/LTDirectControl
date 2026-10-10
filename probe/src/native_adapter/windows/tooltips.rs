//! Native ChampionInfo descriptions, borrowed only during the viewer callback.
//! No patches, retained native pointers, or simulation objects. See the native
//! tooltip investigation for the 0.6.3 calling and ownership evidence.
use super::*;
use crate::native_tooltips;
use std::sync::atomic::AtomicUsize;

const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_TEXT_CAPACITY: usize = 1024 * 1024;

#[repr(C)]
#[derive(Default)]
pub(super) struct MemoryRegion {
    pub(super) base: usize,
    allocation_base: usize,
    allocation_protection: u32,
    partition_id: u16,
    padding: u16,
    pub(super) size: usize,
    state: u32,
    protection: u32,
    kind: u32,
}
#[link(name = "kernel32")]
extern "system" {
    pub(super) fn VirtualQuery(
        address: *const c_void,
        info: *mut MemoryRegion,
        size: usize,
    ) -> usize;
    fn HeapValidate(heap: *mut c_void, flags: u32, memory: *const c_void) -> i32;
}

pub(crate) fn accessible(address: usize, length: usize, writable: bool) -> bool {
    let Some(end) = address.checked_add(length) else {
        return false;
    };
    if address < 0x10000 || length == 0 {
        return false;
    }
    let mut cursor = address;
    while cursor < end {
        let mut region = MemoryRegion::default();
        let returned = unsafe {
            VirtualQuery(
                cursor as *const c_void,
                &mut region,
                std::mem::size_of::<MemoryRegion>(),
            )
        };
        let allowed = if writable {
            [0x04, 0x08, 0x40, 0x80].contains(&(region.protection & 0xff))
        } else {
            [0x02, 0x04, 0x08, 0x20, 0x40, 0x80].contains(&(region.protection & 0xff))
        };
        let Some(next) = region.base.checked_add(region.size) else {
            return false;
        };
        if returned != std::mem::size_of::<MemoryRegion>()
            || region.state != 0x1000
            || region.protection & 0x100 != 0
            || !allowed
            || region.base > cursor
            || next <= cursor
        {
            return false;
        }
        cursor = next;
    }
    true
}

unsafe fn profile_matches(base: usize) -> bool {
    TOOLTIP_BYTES.iter().all(|(rva, bytes)| {
        std::slice::from_raw_parts((base + rva) as *const u8, bytes.len()) == *bytes
    }) && TOOLTIP_POINTERS.iter().all(|(rva, target)| {
        std::ptr::read_unaligned((base + rva) as *const usize) == base + target
    }) && TOOLTIP_TABLE_LAYOUTS.iter().all(|(rva, size, alignment)| {
        std::ptr::read_unaligned((base + rva + 8) as *const usize) == *size
            && std::ptr::read_unaligned((base + rva + 16) as *const usize) == *alignment
    })
}

/// Unlike a Win64 C aggregate, this native Rust result occupies RAX/RDX.
/// Reserve the Win64 shadow space explicitly; never let a C sret consume RCX.
unsafe fn lookup_pair(target: usize, sheet: usize, assets: usize, id: &str) -> [usize; 2] {
    let allocation: usize;
    let table: usize;
    std::arch::asm!(
        "sub rsp, 32",
        "call r11",
        "add rsp, 32",
        in("r11") target,
        in("rcx") sheet,
        inlateout("rdx") assets => table,
        in("r8") id.as_ptr(),
        in("r9") id.len(),
        lateout("rax") allocation,
        clobber_abi("win64"),
    );
    [allocation, table]
}

struct InfoArc {
    words: [usize; 2],
    data: usize,
    base: usize,
}
impl InfoArc {
    unsafe fn checked(words: [usize; 2], base: usize) -> Result<Self, &'static str> {
        let [allocation, table] = words;
        if allocation == 0 {
            return Err("champion info unavailable");
        }
        // Built-in, data-defined and registered mod wrappers are reviewed.
        // An unknown implementation must never bypass the vtable guard.
        let (_, size, alignment) = TOOLTIP_TABLE_LAYOUTS
            .iter()
            .find(|(rva, _, _)| base + rva == table)
            .ok_or("unreviewed ChampionInfo implementation")?;
        let offset = arc_data_offset(*alignment).ok_or("invalid Arc alignment")?;
        let data = allocation
            .checked_add(offset)
            .ok_or("Arc address overflow")?;
        let length = offset.checked_add(*size).ok_or("Arc size overflow")?;
        if allocation % std::mem::align_of::<AtomicUsize>() != 0
            || !accessible(allocation, length, true)
            || HeapValidate(GetProcessHeap(), 0, allocation as *const c_void) == 0
        {
            return Err("invalid ChampionInfo allocation");
        }
        let strong = &*(allocation as *const AtomicUsize);
        let weak = &*((allocation + 8) as *const AtomicUsize);
        if !(1..=isize::MAX as usize).contains(&strong.load(Ordering::Acquire))
            || !(1..=isize::MAX as usize).contains(&weak.load(Ordering::Acquire))
        {
            return Err("invalid ChampionInfo reference counts");
        }
        Ok(Self { words, data, base })
    }
    unsafe fn description(&self, assets: usize, slot: usize) -> Result<String, &'static str> {
        let offset = *TOOLTIP_DESCRIPTION_SLOTS
            .get(slot)
            .ok_or("invalid skill slot")?;
        // The table and its method targets were checked before the lookup.
        let address = std::ptr::read_unaligned((self.words[1] + offset) as *const usize);
        let method: unsafe extern "system" fn(*mut RawText, usize, usize) -> usize =
            std::mem::transmute(address);
        let mut raw = RawText {
            capacity: 0,
            pointer: 1,
            length: 0,
        };
        let output = &mut raw as *mut RawText;
        let returned = method(output, self.data, assets);
        // Establish matching allocator ownership before making a Rust copy.
        let text = NativeText::checked(raw)?;
        if returned != output as usize {
            return Err("unexpected description return convention");
        }
        text.copy()
    }
}
impl Drop for InfoArc {
    fn drop(&mut self) {
        unsafe {
            let strong = &*(self.words[0] as *const AtomicUsize);
            if strong.fetch_sub(1, Ordering::AcqRel) == 1 {
                let drop_slow: unsafe extern "system" fn(*const [usize; 2]) =
                    std::mem::transmute(self.base + TOOLTIP_ARC_DROP);
                drop_slow(&self.words);
            }
        }
    }
}
fn arc_data_offset(alignment: usize) -> Option<usize> {
    alignment.is_power_of_two().then_some(())?;
    16usize.checked_add((alignment - 1) & !15)
}

#[repr(C)]
struct RawText {
    capacity: usize,
    pointer: usize,
    length: usize,
}
struct NativeText {
    raw: RawText,
    heap: *mut c_void,
}
impl NativeText {
    unsafe fn checked(raw: RawText) -> Result<Self, &'static str> {
        if !valid_text_shape(raw.capacity, raw.length) {
            return Err("invalid native String shape");
        }
        let heap = GetProcessHeap();
        if raw.capacity != 0 && HeapValidate(heap, 0, raw.pointer as *const c_void) == 0 {
            return Err("native String allocator mismatch");
        }
        Ok(Self { raw, heap })
    }
    fn copy(&self) -> Result<String, &'static str> {
        if self.raw.length == 0 {
            return Err("empty optional description");
        }
        if !accessible(self.raw.pointer, self.raw.length, false) {
            return Err("unreadable native description");
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(self.raw.pointer as *const u8, self.raw.length) };
        let text = std::str::from_utf8(bytes).map_err(|_| "description is not UTF-8")?;
        if text.trim().is_empty() || text.starts_with("asset/") || text.starts_with("#asset/") {
            return Err("untranslated native description");
        }
        Ok(text.to_owned())
    }
}
impl Drop for NativeText {
    fn drop(&mut self) {
        if self.raw.capacity != 0 {
            unsafe {
                HeapFree(self.heap, 0, self.raw.pointer as *mut c_void);
            }
        }
    }
}
fn valid_text_shape(capacity: usize, length: usize) -> bool {
    length <= capacity && length <= MAX_TEXT_BYTES && capacity <= MAX_TEXT_CAPACITY
}

unsafe fn resolve(
    assets: usize,
    request: &native_tooltips::Request,
) -> Result<String, &'static str> {
    let base = verified_base().ok_or("native adapter unavailable")?;
    if !profile_matches(base) {
        return Err("native tooltip profile mismatch");
    }
    if !accessible(assets, 0x70, false) {
        return Err("Assets context unavailable");
    }
    let get_sheet: unsafe extern "system" fn(usize, *const u8, usize) -> usize =
        std::mem::transmute(base + TOOLTIP_SHEET_GETTER);
    let path = b"asset/base/setting/champion_info";
    let sheet = get_sheet(assets, path.as_ptr(), path.len());
    if !accessible(sheet, 0x18, false) {
        return Err("champion sheet unavailable");
    }
    let words = lookup_pair(base + TOOLTIP_INFO_LOOKUP, sheet, assets, &request.champion);
    let info = InfoArc::checked(words, base)?;
    info.description(assets, request.slot)
}

pub(super) unsafe fn resolve_pending(view: usize, assets: usize, shared: &Shared) {
    if !shared.timing.client_controls(Some(view)) {
        return;
    }
    let generation = shared.timing.generation();
    let Some(request) = native_tooltips::take(generation) else {
        return;
    };
    shared.logger.write(&format!(
        "NATIVE TOOLTIP_CALL champion={} slot={} generation={generation}",
        request.champion, request.slot
    ));
    let result = catch_unwind(AssertUnwindSafe(|| resolve(assets, &request)))
        .unwrap_or(Err("native tooltip adapter panic"));
    let unresolved = result
        .as_ref()
        .map(|text| native_tooltips::unresolved_parameters(text))
        .unwrap_or_default();
    let audit = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "generation": generation,
        "champion": request.champion,
        "slot": request.slot,
        "source": if result.is_ok() { "game" } else { "fallback" },
        "unresolved_parameters": unresolved,
        "ellipsis_count": result.as_ref().map_or(0, |text| text.matches('…').count()),
        "bytes": result.as_ref().map_or(0, |text| text.len()),
        "reason": result.as_ref().err(),
    });
    shared
        .logger
        .write(&format!("NATIVE TOOLTIP_AUDIT {audit}"));
    match &result {
        Ok(text) => {
            shared.logger.write(&format!(
                "NATIVE TOOLTIP champion={} slot={} bytes={} unresolved={} source=game",
                request.champion,
                request.slot,
                text.len(),
                text.matches('{').count()
            ));
            shared.logger.write(&format!(
                "NATIVE TOOLTIP_TEXT champion={} slot={} text={text:?}",
                request.champion, request.slot
            ));
        }
        Err(reason) => shared.logger.write(&format!(
            "NATIVE TOOLTIP fallback champion={} slot={} reason={reason}",
            request.champion, request.slot
        )),
    }
    native_tooltips::publish(request, result.ok());
}

#[cfg(test)]
mod tests {
    use super::*;
    // Stand-in Rust-register result. The bridge must preserve the four input
    // registers and must not insert a C aggregate-return argument.
    std::arch::global_asm!(
        ".globl tooltip_pair_fixture",
        "tooltip_pair_fixture:",
        "mov [rsp + 8], rcx",
        "mov [rsp + 16], rdx",
        "mov [rsp + 24], r8",
        "mov [rsp + 32], r9",
        "mov r10, rsp",
        "and r10, 15",
        "cmp r10, 8",
        "jne tooltip_pair_bad_stack",
        "mov rax, [rsp + 8]",
        "add rax, [rsp + 24]",
        "mov rdx, [rsp + 16]",
        "add rdx, [rsp + 32]",
        "ret",
        "tooltip_pair_bad_stack:",
        "xor eax, eax",
        "xor edx, edx",
        "ret",
    );
    extern "system" {
        fn tooltip_pair_fixture();
    }
    #[test]
    fn native_pair_bridge_uses_rax_rdx_without_hidden_sret() {
        let id = "illusionist";
        let result = unsafe { lookup_pair(tooltip_pair_fixture as *const () as usize, 7, 11, id) };
        assert_eq!(result, [7 + id.as_ptr() as usize, 11 + id.len()]);
    }
    static ARC_DROPS: AtomicUsize = AtomicUsize::new(0);
    unsafe extern "system" fn arc_drop_fixture(words: *const [usize; 2]) {
        let allocation = (*words)[0];
        assert_eq!(
            (*(allocation as *const AtomicUsize)).load(Ordering::Acquire),
            0
        );
        ARC_DROPS.fetch_add(1, Ordering::Relaxed);
        HeapFree(GetProcessHeap(), 0, allocation as *mut c_void);
    }
    #[test]
    fn arc_cleanup_releases_one_strong_reference_and_drops_only_the_last_owner() {
        unsafe {
            let allocation = HeapAlloc(GetProcessHeap(), 0, 16) as usize;
            assert_ne!(allocation, 0);
            std::ptr::write(allocation as *mut AtomicUsize, AtomicUsize::new(2));
            std::ptr::write((allocation + 8) as *mut AtomicUsize, AtomicUsize::new(1));
            let base = arc_drop_fixture as *const () as usize - TOOLTIP_ARC_DROP;
            drop(InfoArc {
                words: [allocation, 0],
                data: 0,
                base,
            });
            assert_eq!(
                (*(allocation as *const AtomicUsize)).load(Ordering::Acquire),
                1
            );
            assert_eq!(ARC_DROPS.load(Ordering::Relaxed), 0);
            drop(InfoArc {
                words: [allocation, 0],
                data: 0,
                base,
            });
            assert_eq!(ARC_DROPS.load(Ordering::Relaxed), 1);
        }
    }
    #[test]
    fn native_string_bounds_and_arc_alignment_are_checked_before_copying() {
        assert!(valid_text_shape(0, 0));
        assert!(valid_text_shape(128, 72));
        assert!(!valid_text_shape(4, 5));
        assert!(!valid_text_shape(MAX_TEXT_CAPACITY + 1, 12));
        assert!(!valid_text_shape(MAX_TEXT_BYTES + 1, MAX_TEXT_BYTES + 1));
        assert_eq!(arc_data_offset(8), Some(16));
        assert_eq!(arc_data_offset(16), Some(16));
        assert_eq!(arc_data_offset(32), Some(32));
        assert_eq!(arc_data_offset(64), Some(64));
        assert_eq!(arc_data_offset(0), None);
        assert_eq!(arc_data_offset(3), None);
        assert_eq!(std::mem::size_of::<RawText>(), 24);
        assert_eq!(std::mem::size_of::<MemoryRegion>(), 48);
    }
    #[test]
    fn native_text_is_copied_and_freed_with_the_game_heap() {
        let source = "<#[color]>Taunt 1.5 秒<>";
        unsafe {
            let heap = GetProcessHeap();
            let pointer = HeapAlloc(heap, 0, source.len()) as *mut u8;
            assert!(!pointer.is_null());
            std::ptr::copy_nonoverlapping(source.as_ptr(), pointer, source.len());
            let owned = NativeText::checked(RawText {
                capacity: source.len(),
                pointer: pointer as usize,
                length: source.len(),
            })
            .unwrap();
            let copy = owned.copy().unwrap();
            drop(owned);
            assert_eq!(copy, source);
        }
    }
}
