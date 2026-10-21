//! Each skill's current numbers by name, as the game's own balance patch
//! engine and database editor read them (found 2026-10-21; step 4 of the
//! preview plan). `named` reads them for the controlled athlete, so the
//! shipped preview table can size its shapes (`preview_table`, stage B,
//! 1.5.0-dev17); `log` is the research log of 1.5.0-dev13 that, since
//! 1.5.0-dev15, also stores them in the shape table with the shapes read at
//! the same moment (stage A); the log lives in `research.rs` (research
//! builds only).
//!
//! Read from the code (0.6.4; the same table shape in 0.6.3 and 0.7.0): the
//! skill objects at `entity + 0x578/0x588/0x598` (object, vtable), whose
//! cooldown (+0x90) and use count (+0xa8) `read_charges` already calls,
//! start their vtable with the patch methods:
//! - +0x18 the skill's patch name (for example "hammerer_skill2"), returned
//!   as (pointer, length) in rax/rdx;
//! - +0x20 get(self, name pointer, name length): found in al, value in rdx.
//!
//! Both return two registers in the game's own convention, so they are
//! called through `asm!` below rather than a declared `extern` signature.
//! Like `read_charges`: only learned skills, nothing retained past the hook.
//!
//! Safety, in this order: the verifier and the start-up guards check, per
//! game version, that Hammerer W's method table holds these methods at
//! these positions (`identity::LAYOUT_POINTERS`, `SKILL_PATCH_*_SLOT`);
//! then, before its numbers are used, a skill's name must be a short
//! lowercase identifier and its `cooltime` must equal what its cooldown
//! method returns.
use super::*;
use std::cell::RefCell;
use std::collections::HashMap;

/// The champion's stack (Dancer R's blades grow with her kills); the live
/// reader reads it here too.
pub(super) const ENTITY_STACK: usize = 0x648;
pub(super) const SLOTS: [usize; 3] = [0x578, 0x588, 0x598];
/// The level that unlocks each skill, as `read_charges` uses it.
pub(super) const UNLOCK: [usize; 3] = [1, 3, 5];

/// What became of reading one skill's numbers in this match.
#[derive(Clone)]
enum Named {
    Read(HashMap<String, i64>),
    Refused(&'static str),
}
/// A controlled athlete in one match: (match, actor).
pub(super) type Owner = (crate::native_timing::MatchKey, usize);
type Outcomes = (Option<Owner>, [Option<Named>; 3]);
thread_local! {
    /// The controlled athlete's skills read in this match and each slot's
    /// outcome. Read once; a skill not learned yet is tried again.
    static NAMED: RefCell<Outcomes> = const { RefCell::new((None, [None, None, None])) };
}
/// The controlled athlete's numbers named `names` for its skill in `slot`,
/// read once per match: `None` while the skill is not learned or when it was
/// refused (then the table's recorded values are used). Logs the outcome
/// once per skill and match.
pub(super) unsafe fn named(
    shared: &Shared,
    owner: Owner,
    entity: usize,
    base: usize,
    slot: usize,
    champion: &str,
    names: &[String],
) -> Option<HashMap<String, i64>> {
    let cached = NAMED.with(|n| {
        let mut n = n.borrow_mut();
        if n.0 != Some(owner) {
            *n = (Some(owner), [None, None, None]);
        }
        n.1[slot].clone()
    });
    let outcome = match cached {
        Some(outcome) => outcome,
        None => {
            let outcome = match read_named(entity, base, slot, names) {
                Ok(values) => Named::Read(values),
                // Not learned yet: no outcome, try again later.
                Err(None) => return None,
                Err(Some(reason)) => Named::Refused(reason),
            };
            let line = match &outcome {
                Named::Read(values) => format!("read {} of {}", values.len(), names.len()),
                Named::Refused(reason) => format!("refused={reason}"),
            };
            shared.logger.write(&format!(
                "PREVIEW NAMES champion={champion} slot={} {line}",
                ["Q", "W", "R"][slot]
            ));
            NAMED.with(|n| n.borrow_mut().1[slot] = Some(outcome.clone()));
            outcome
        }
    };
    match outcome {
        Named::Read(values) => Some(values),
        Named::Refused(_) => None,
    }
}
/// `Err(None)`: not learned yet; `Err(Some(reason))`: refused.
unsafe fn read_named(
    entity: usize,
    base: usize,
    slot: usize,
    names: &[String],
) -> Result<HashMap<String, i64>, Option<&'static str>> {
    let level = std::ptr::read_unaligned((entity + 0x5c0) as *const usize);
    let object = std::ptr::read_unaligned((entity + SLOTS[slot]) as *const usize);
    let table = std::ptr::read_unaligned((entity + SLOTS[slot] + 8) as *const usize);
    if level < UNLOCK[slot] || object == 0 {
        return Err(None);
    }
    let end = image_end(base).ok_or(Some("image size unreadable"))?;
    if !super::tooltips::accessible(table, 0x98, false) {
        return Err(Some("method table unreadable"));
    }
    let name_fn = std::ptr::read_unaligned((table + SKILL_PATCH_NAME_SLOT) as *const usize);
    let get_fn = std::ptr::read_unaligned((table + SKILL_PATCH_GET_SLOT) as *const usize);
    let cool_fn = std::ptr::read_unaligned((table + 0x90) as *const usize);
    if ![name_fn, get_fn, cool_fn]
        .iter()
        .all(|f| (base..end).contains(f))
    {
        return Err(Some("methods outside the game"));
    }
    patch_name(name_fn, object).ok_or(Some("unreadable patch name"))?;
    type Cooldown = unsafe extern "system" fn(usize, usize) -> usize;
    let cooldown: Cooldown = std::mem::transmute(cool_fn);
    if get(get_fn, object, "cooltime") != Some(cooldown(object, entity) as i64) {
        return Err(Some("cooltime differs from the cooldown method"));
    }
    Ok(names
        .iter()
        .filter_map(|name| Some((name.clone(), get(get_fn, object, name)?)))
        .collect())
}
/// One past the executable image's last byte (`SizeOfImage`).
pub(super) unsafe fn image_end(base: usize) -> Option<usize> {
    if !super::tooltips::accessible(base, 0x400, false) {
        return None;
    }
    let header = std::ptr::read_unaligned((base + 0x3c) as *const u32) as usize;
    if header > 0x300 {
        return None;
    }
    // PE signature, file header (0x14), then SizeOfImage at +0x38.
    let size = std::ptr::read_unaligned((base + header + 0x18 + 0x38) as *const u32);
    base.checked_add(size as usize)
}
/// The skill's patch name: a short lowercase identifier, or `None`.
pub(super) unsafe fn patch_name(function: usize, object: usize) -> Option<String> {
    let pointer: usize;
    let length: usize;
    std::arch::asm!(
        "sub rsp, 32",
        "call r10",
        "add rsp, 32",
        inout("r10") function => _,
        inout("rcx") object => _,
        lateout("rax") pointer,
        lateout("rdx") length,
        clobber_abi("win64"),
    );
    if !(1..=64).contains(&length) || !super::tooltips::accessible(pointer, length, false) {
        return None;
    }
    let bytes = std::slice::from_raw_parts(pointer as *const u8, length);
    bytes
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_')
        .then(|| String::from_utf8_lossy(bytes).into_owned())
}
/// The skill's number called `name`, as the game stores it (distances in
/// thousandths of a world unit), or `None` when the skill has no such name.
pub(super) unsafe fn get(function: usize, object: usize, name: &str) -> Option<i64> {
    let found: usize;
    let value: usize;
    std::arch::asm!(
        "sub rsp, 32",
        "call r10",
        "add rsp, 32",
        inout("r10") function => _,
        inout("rcx") object => _,
        inout("rdx") name.as_ptr() => value,
        inout("r8") name.len() => _,
        lateout("rax") found,
        clobber_abi("win64"),
    );
    (found & 1 == 1).then_some(value as i64)
}
