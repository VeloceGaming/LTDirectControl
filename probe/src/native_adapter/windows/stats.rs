//! Athlete stat rules: instruction-level hooks for manually played matches.
//!
//! Each site of `STAT_SITES` (this game version's table) replaces one or two
//! whole instructions with a call into a private relay. The relay replays the
//! displaced bytes, pushes the site's number and jumps to the bridge, which
//! saves every register and calls `adjust`. `adjust` may change one register
//! or one frame slot (at a shape-A floor, also the flags) and returns; the
//! original code then carries on, so a site that does nothing leaves the
//! game exactly as it was.
//!
//! Threads: every simulation thread runs these sites, hundreds of thousands
//! of times a second. `adjust` therefore asks `athlete_rules::active_on`
//! first (one atomic load and a compare) and returns before it touches game
//! memory when the thread is not the controlled athlete's match. No hook
//! takes a lock or writes the log: lines are queued per thread and written
//! by the controlled player's tick. Nothing a hook is given is kept after it
//! returns.
use super::*;
use crate::athlete_rules::{self as rules, Caster, Kind, Register, Rule, Source};
use std::cell::Cell;

/// What a gate's capture site remembers for its complete site. The two sites
/// are straight-line code with no native call between them.
#[derive(Clone, Copy)]
struct Pending {
    capture: usize,
    seed: u64,
    accuracy: u32,
    dodging: u32,
    actor: u64,
}
thread_local! {
    static PENDING: Cell<Option<Pending>> = const { Cell::new(None) };
    /// Recently logged gate pairs, so a pair is logged once, not every tick.
    static LOGGED: Cell<([u64; 32], usize)> = const { Cell::new(([0; 32], 0)) };
    /// Log lines a hook produced. A hook runs inside the game's code, so it
    /// never takes the log's lock or writes the file: the controlled
    /// player's tick, on this same thread, writes them (`stat_rules_report`).
    static NOTES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}
/// Most lines kept between two ticks; more are dropped.
const NOTES_MAX: usize = 32;
thread_local! {
    /// The last Accuracy producer that marked a shot on this thread and the
    /// word it traces (see `Kind::AccuracyProducer`), for the hit test that
    /// follows it.
    static SHOT: Cell<Option<(usize, u64)>> = const { Cell::new(None) };
    /// Trace combinations already reported, so each is reported once.
    static TRACED: Cell<([u64; 64], usize)> = const { Cell::new(([0; 64], 0)) };
    /// Every marked shot and shape-B hit since the last tick, for the
    /// practice readout (`crate::practice`).
    static EVENTS: std::cell::RefCell<Vec<rules::ShotEvent>> = const { std::cell::RefCell::new(Vec::new()) };
}
const EVENTS_MAX: usize = 64;
fn event(e: rules::ShotEvent) {
    EVENTS.with(|events| {
        if let Ok(mut events) = events.try_borrow_mut() {
            if events.len() < EVENTS_MAX {
                events.push(e);
            }
        }
    });
}
/// The game stores an entity's action state as a niche-encoded enum: the
/// variant is the word with its top bit cleared when the top bit is set,
/// and 4 otherwise (read from the code at the switch after the producers).
fn trace_value(offset: usize, word: u64) -> String {
    if offset == STAT_TRACE_ACTION_STATE {
        let state = if word >> 63 == 1 { word ^ (1 << 63) } else { 4 };
        format!("state={state}")
    } else {
        format!("kind={word}")
    }
}
/// True the first time `key` is seen among the last few trace keys.
fn first_trace(key: u64) -> bool {
    let (mut ring, mut next) = TRACED.get();
    if ring.contains(&key) {
        return false;
    }
    ring[next % ring.len()] = key;
    next += 1;
    TRACED.set((ring, next));
    true
}
/// The slot of each collision site in the counters, or `u8::MAX`.
static SLOTS: OnceLock<Vec<u8>> = OnceLock::new();
static STATS_PATCHES: OnceLock<Vec<(usize, Vec<u8>)>> = OnceLock::new();
static STATS_READY: AtomicBool = AtomicBool::new(false);

#[repr(C)]
struct Registers {
    values: [u64; 15],
    flags: u64,
    site: usize,
    return_address: usize,
}
impl Registers {
    fn get(&self, r: Register) -> u64 {
        self.values[r as usize]
    }
    fn set(&mut self, r: Register, value: u64) {
        self.values[r as usize] = value;
    }
}

/// A u64 at `base + offset`, when `base` looks like a game object.
unsafe fn read(base: u64, offset: usize) -> Option<u64> {
    (base != 0 && base.is_multiple_of(8))
        .then(|| std::ptr::read_unaligned((base as usize + offset) as *const u64))
}
unsafe fn caster_id(c: &Registers, caster: Caster) -> Option<u64> {
    match caster {
        Caster::Field { reg, offset } => read(c.get(reg), offset),
        Caster::Shifted { reg, shift } => Some(c.get(reg) >> shift),
        Caster::Mixed {
            reg,
            other,
            offset,
            shift,
        } => Some((c.get(reg) ^ read(c.get(other), offset)?) >> shift),
    }
}
fn log(line: &str) {
    NOTES.with(|notes| {
        if let Ok(mut notes) = notes.try_borrow_mut() {
            if notes.len() < NOTES_MAX {
                notes.push(line.to_owned());
            }
        }
    });
}

unsafe extern "system" fn adjust(registers: *mut Registers) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let c = &mut *registers;
        let Some(site) = STAT_SITES.get(c.site) else {
            return;
        };
        let thread = crate::platform_input::thread_id();
        if !rules::active_on(thread) {
            return;
        }
        match site.kind {
            Kind::AccuracyProducer {
                frame,
                caster,
                trace,
            } => {
                let Some(actor) = caster_id(c, caster) else {
                    return;
                };
                let base = match caster {
                    Caster::Field { reg, .. } => c.get(reg),
                    _ => 0,
                };
                let word = read(base, trace);
                let controlled = rules::controlled(thread, actor);
                if let Some(word) = word {
                    let key = ((c.site as u64) << 56) ^ (u64::from(controlled) << 55) ^ word;
                    if first_trace(key) {
                        log(&format!(
                            "STATRULE trace producer={:#x} {} caster={}",
                            site.rva,
                            trace_value(trace, word),
                            if controlled { "you" } else { "other" }
                        ));
                    }
                }
                if !controlled {
                    SHOT.set(None);
                    return;
                }
                SHOT.set(word.map(|w| (c.site, w)));
                if let Some(word) = word {
                    event(rules::ShotEvent {
                        producer: site.rva,
                        entity: trace == STAT_TRACE_ACTION_STATE,
                        word,
                        hit_test: None,
                    });
                }
                std::ptr::write_unaligned(
                    (c.get(Register::Rbp) as usize + frame) as *mut u64,
                    rules::COLLISION_MARKER,
                );
                if rules::applied(Rule::Accuracy) {
                    log(&format!(
                        "STATRULE accuracy: first shot of athlete {actor} marked as {} units",
                        rules::COLLISION_MARKER / 1000
                    ));
                }
            }
            Kind::DodgingProducer { output, entity } => {
                let Some(actor) = read(c.get(entity), STAT_ENTITY_ID) else {
                    return;
                };
                if !rules::controlled(thread, actor) {
                    return;
                }
                c.set(output, 0);
                if rules::applied(Rule::Dodging) {
                    log(&format!(
                        "STATRULE dodging: derived Dodging of athlete {actor} set to 0"
                    ));
                }
            }
            Kind::Capture {
                accuracy,
                inverse_dodge,
                caster,
                age_mixed,
            } => {
                let actor = caster_id(c, caster).filter(|a| rules::controlled(thread, *a));
                let Some(actor) = actor else {
                    PENDING.set(None);
                    return;
                };
                let inverse = match inverse_dodge {
                    Source::Register(r) => c.get(r) as u32,
                    Source::Frame(offset) => std::ptr::read_unaligned(
                        (c.get(Register::Rbp) as usize + offset) as *const u32,
                    ),
                };
                PENDING.set(Some(Pending {
                    capture: c.site,
                    seed: rules::gate_seed(c.get(Register::Rax), age_mixed),
                    accuracy: (c.get(accuracy) as u32).min(100),
                    dodging: 100 - inverse.min(100),
                    actor,
                }));
            }
            Kind::Complete { capture, output } => {
                let Some(p) = PENDING.take().filter(|p| p.capture == capture) else {
                    return;
                };
                let game = c.get(output);
                let gap = p.accuracy as i32 - p.dodging as i32;
                let (floor, ceiling) = rules::limits();
                let gate = rules::reaction_gate(p.seed, gap, floor, ceiling);
                c.set(output, u64::from(gate));
                if rules::applied(Rule::Gate) {
                    log(&format!(
                        "STATRULE gate: first reaction of athlete {} gated at {gate} ticks (game {game})",
                        p.actor
                    ));
                }
                if crate::logging::verbose() && first_time(p.seed ^ capture as u64) {
                    log(&format!(
                        "STATRULE gate caster={} A={} D={} gap={gap} gate={gate} game_gate={game} seed={:#x}",
                        p.actor, p.accuracy, p.dodging, p.seed
                    ));
                }
            }
            Kind::Collision {
                output,
                entity,
                accuracy: (source, offset),
            } => {
                let Some(dodging) = read(c.get(entity), STAT_DERIVED_DODGING) else {
                    return;
                };
                let marked = read(c.get(source), offset) == Some(rules::COLLISION_MARKER);
                if marked {
                    // The shot carries the marker: restore the value the
                    // replayed `sub` took the target's Dodging from, and turn
                    // the marker's 5 units into the chosen margin.
                    c.set(
                        output,
                        c.get(output)
                            .wrapping_add(dodging)
                            .wrapping_add(rules::margin_delta()),
                    );
                    marked_hit(c.site, 'B');
                }
                rules::collision_seen(slot_of(c.site), marked);
            }
            Kind::FlooredCollision { output, entity } => {
                let Some(dodging) = read(c.get(entity), STAT_DERIVED_DODGING) else {
                    return;
                };
                let marked = rules::floor_marked(c.get(output), dodging);
                if marked {
                    // The controlled athlete's shot: the chosen margin, and
                    // flags that keep the game's floor from raising it.
                    c.set(output, rules::floor_margin());
                    c.flags = rules::flags_above(c.flags);
                    marked_hit(c.site, 'A');
                }
                rules::collision_seen(slot_of(c.site), marked);
            }
        }
    }));
}

/// The counter slot of a collision site.
fn slot_of(site: usize) -> usize {
    SLOTS
        .get()
        .and_then(|s| s.get(site))
        .map_or(usize::MAX, |s| usize::from(*s))
}

/// A marked shot reached the collision consumer `site` (table index) of
/// `shape`: the first-fire line, the readout event and the trace line.
fn marked_hit(site: usize, shape: char) {
    let Some(rva) = STAT_SITES.get(site).map(|s| s.rva) else {
        return;
    };
    if rules::applied(Rule::Collision) {
        log(&format!(
            "STATRULE collision: first shot of the controlled athlete skipped the target's Dodging, margin {} units",
            rules::margin_units()
        ));
    }
    let Some((producer, word)) = SHOT.get() else {
        return;
    };
    let Some(rules::Site {
        rva: from,
        kind: Kind::AccuracyProducer { trace, .. },
        ..
    }) = STAT_SITES.get(producer)
    else {
        return;
    };
    event(rules::ShotEvent {
        producer: *from,
        entity: *trace == STAT_TRACE_ACTION_STATE,
        word,
        hit_test: Some(rules::HitTest { shape, rva }),
    });
    let key = (1 << 63) ^ ((site as u64) << 48) ^ ((producer as u64) << 40) ^ word;
    if first_trace(key) {
        log(&format!(
            "STATRULE trace hit_test={rva:#x} (shape {shape}) producer={from:#x} {}",
            trace_value(*trace, word)
        ));
    }
}

/// True the first time `key` is seen among the last few logged pairs.
fn first_time(key: u64) -> bool {
    let (mut ring, mut next) = LOGGED.get();
    if ring.contains(&key) {
        return false;
    }
    ring[next % ring.len()] = key;
    next += 1;
    LOGGED.set((ring, next));
    true
}

/// Called from an instruction site, not a function entry. Saves every GPR and
/// flags; Win64 preserves XMM6..15, and XMM0..5 are saved here. RSP is aligned
/// dynamically rather than assuming the callers' stack alignment.
#[unsafe(naked)]
unsafe extern "system" fn bridge() {
    std::arch::naked_asm!(
        "pushfq", "push rax", "push rbx", "push rcx", "push rdx",
        "push r8", "push r9", "push r10", "push r11", "push rsi",
        "push rdi", "push rbp", "push r12", "push r13", "push r14", "push r15",
        "mov rsi, rsp", "sub rsp, 0x80", "and rsp, -16",
        "movdqu [rsp + 0x20], xmm0", "movdqu [rsp + 0x30], xmm1",
        "movdqu [rsp + 0x40], xmm2", "movdqu [rsp + 0x50], xmm3",
        "movdqu [rsp + 0x60], xmm4", "movdqu [rsp + 0x70], xmm5",
        "mov rcx, rsi", "call {adjust}",
        "movdqu xmm0, [rsp + 0x20]", "movdqu xmm1, [rsp + 0x30]",
        "movdqu xmm2, [rsp + 0x40]", "movdqu xmm3, [rsp + 0x50]",
        "movdqu xmm4, [rsp + 0x60]", "movdqu xmm5, [rsp + 0x70]",
        "mov rsp, rsi",
        "pop r15", "pop r14", "pop r13", "pop r12", "pop rbp", "pop rdi",
        "pop rsi", "pop r11", "pop r10", "pop r9", "pop r8", "pop rdx",
        "pop rcx", "pop rbx", "pop rax", "popfq", "lea rsp, [rsp + 8]", "ret",
        adjust = sym adjust,
    );
}

/// Relay for one site: the displaced bytes, `push site`, `jmp bridge`.
unsafe fn make_relay(site: usize, bytes: &[u8], id: usize, target: usize) -> Result<usize, String> {
    let relay = allocate_near(site)?;
    let mut code = bytes.to_vec();
    code.push(0x68); // push imm32: the site's number, leaving every register intact
    code.extend_from_slice(&(id as u32).to_le_bytes());
    code.extend_from_slice(&[0xff, 0x25, 0, 0, 0, 0]);
    code.extend_from_slice(&target.to_le_bytes());
    std::ptr::copy_nonoverlapping(code.as_ptr(), relay as *mut u8, code.len());
    let mut old = 0;
    if VirtualProtect(relay as *mut c_void, 0x1000, 0x20, &mut old) == 0
        || FlushInstructionCache(GetCurrentProcess(), relay as *const c_void, code.len()) == 0
    {
        return Err("cannot protect or flush a stat rule relay".into());
    }
    Ok(relay)
}

/// The stats family is reviewed for this executable and the other hooks are
/// in; set by `install`.
static STATS_ALLOWED: AtomicBool = AtomicBool::new(false);
/// Patching was attempted (it is attempted at most once a session).
static STATS_TRIED: AtomicBool = AtomicBool::new(false);
pub(crate) fn allow_stats() {
    STATS_ALLOWED.store(true, Ordering::Release);
}
/// Supported here but not attempted yet: the next title screen installs it.
pub(in crate::native_adapter) fn stats_pending() -> bool {
    STATS_ALLOWED.load(Ordering::Acquire) && !STATS_TRIED.load(Ordering::Acquire)
}
/// Patches the stat rules in, at most once a session, when the family is
/// allowed. Call only at the title screen, where no match runs, so no thread
/// can be inside the instructions being replaced (the same reason every
/// other hook is installed there). None when there is nothing to do.
pub(in crate::native_adapter) fn install_stats_at_title() -> Option<Result<usize, String>> {
    if !STATS_ALLOWED.load(Ordering::Acquire) || STATS_TRIED.swap(true, Ordering::AcqRel) {
        return None;
    }
    let base = unsafe { GetModuleHandleW(std::ptr::null()) } as usize;
    Some(unsafe { install_stats(base) })
}

/// Optional, all or none: a failed check or write leaves the game's code as
/// it was, the rules unavailable and every other hook installed.
unsafe fn install_stats(base: usize) -> Result<usize, String> {
    for site in STAT_SITES {
        let live = std::slice::from_raw_parts((base + site.rva) as *const u8, site.bytes.len());
        if live != site.bytes {
            return Err(format!(
                "site {:#x} differs (another mod, or a changed game)",
                site.rva
            ));
        }
    }
    let mut slots = Vec::new();
    let mut next = 0u8;
    for site in STAT_SITES {
        slots.push(
            if matches!(
                site.kind,
                Kind::Collision { .. } | Kind::FlooredCollision { .. }
            ) {
                next += 1;
                next - 1
            } else {
                u8::MAX
            },
        );
    }
    if usize::from(next) > rules::COLLISION_SLOTS {
        return Err("more collision sites than counter slots".into());
    }
    let mut patches = Vec::new();
    for (id, site) in STAT_SITES.iter().enumerate() {
        let address = base + site.rva;
        let relay = make_relay(address, site.bytes, id, bridge as *const () as usize)?;
        let mut patch = relative_call(address, relay)?.to_vec();
        patch.resize(site.bytes.len(), 0x90);
        patches.push((address, site.bytes.to_vec(), patch));
    }
    let _ = SLOTS.set(slots);
    let mut applied = 0;
    let mut failed = None;
    for (address, _, patch) in &patches {
        let mut old = 0;
        if VirtualProtect(*address as *mut c_void, patch.len(), 0x40, &mut old) == 0 {
            failed = Some("cannot unprotect a stat rule site");
            break;
        }
        std::ptr::copy_nonoverlapping(patch.as_ptr(), *address as *mut u8, patch.len());
        let flushed =
            FlushInstructionCache(GetCurrentProcess(), *address as *const c_void, patch.len()) != 0;
        let readback = std::slice::from_raw_parts(*address as *const u8, patch.len()) == patch;
        let mut ignored = 0;
        VirtualProtect(*address as *mut c_void, patch.len(), old, &mut ignored);
        applied += 1;
        if !(flushed && readback) {
            failed = Some("a stat rule site did not read back");
            break;
        }
    }
    if let Some(reason) = failed {
        for (address, original, _) in patches.iter().take(applied) {
            let mut old = 0;
            if VirtualProtect(*address as *mut c_void, original.len(), 0x40, &mut old) != 0 {
                std::ptr::copy_nonoverlapping(
                    original.as_ptr(),
                    *address as *mut u8,
                    original.len(),
                );
                FlushInstructionCache(
                    GetCurrentProcess(),
                    *address as *const c_void,
                    original.len(),
                );
                let mut ignored = 0;
                VirtualProtect(*address as *mut c_void, original.len(), old, &mut ignored);
            }
        }
        return Err(format!("{reason}; originals restored"));
    }
    let _ = STATS_PATCHES.set(
        patches
            .iter()
            .map(|(address, _, patch)| (*address, patch.clone()))
            .collect(),
    );
    STATS_READY.store(true, Ordering::Release);
    rules::set_installed();
    Ok(patches.len())
}

/// True when every stat rule site is patched and still reads back.
pub(crate) fn stats_intact() -> Option<bool> {
    STATS_PATCHES.get().map(|patches| unsafe {
        patches.iter().all(|(address, bytes)| {
            std::slice::from_raw_parts(*address as *const u8, bytes.len()) == bytes
        })
    })
}
fn stat_rules_ready() -> bool {
    STATS_READY.load(Ordering::Acquire)
}

/// The lines the hooks queued on this thread, then the aggregated collision
/// counts, at most every five seconds and only when something changed
/// (Verbose). Called from the controlled player's tick, the thread the
/// hooks for that match run on.
pub(in crate::native_adapter) fn stat_rules_report(
    logger: &Logger,
    context: &dyn Fn() -> String,
) -> Vec<rules::ShotEvent> {
    static LAST_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static SHOWN: std::sync::Mutex<Vec<(u64, u64)>> = std::sync::Mutex::new(Vec::new());
    if !stat_rules_ready() {
        return Vec::new();
    }
    let events = EVENTS.with(|e| e.try_borrow_mut().map(|mut e| std::mem::take(&mut *e)));
    let events = events.unwrap_or_default();
    let notes = NOTES.with(|notes| notes.try_borrow_mut().map(|mut n| std::mem::take(&mut *n)));
    for line in notes.unwrap_or_default() {
        // Your champion and last cast describe your shots only.
        if line.starts_with("STATRULE trace") && !line.contains("caster=other") {
            logger.write(&format!("{line} {}", context()));
        } else {
            logger.write(&line);
        }
    }
    if !crate::logging::verbose() {
        return events;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    if now.saturating_sub(LAST_MS.load(Ordering::Relaxed)) < 5000 {
        return events;
    }
    LAST_MS.store(now, Ordering::Relaxed);
    let (_, counts) = rules::totals();
    let Ok(mut shown) = SHOWN.lock() else {
        return events;
    };
    shown.resize(counts.len(), (0, 0));
    let slots = SLOTS.get();
    for (slot, (seen, marked)) in counts.iter().enumerate() {
        if shown[slot] == (*seen, *marked) || *seen == 0 {
            continue;
        }
        shown[slot] = (*seen, *marked);
        let rva = slots.and_then(|s| s.iter().position(|v| usize::from(*v) == slot));
        let rva = rva.and_then(|i| STAT_SITES.get(i)).map_or(0, |s| s.rva);
        logger.write(&format!(
            "STATRULE collision site={rva:#x} seen={seen} marked={marked} margin={} units",
            rules::margin_units()
        ));
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    #[unsafe(naked)]
    unsafe extern "system" fn exercise(_out: *mut [u64; 6], _relay: usize) {
        std::arch::naked_asm!(
            "sub rsp, 0x28",
            "mov eax, 123",
            "mov r8d, 456",
            "mov r9d, 789",
            "mov r11d, 1011",
            "movq xmm0, rax",
            "cmp rax, rax",
            "call rdx",
            "mov [rcx], rax",
            "mov [rcx + 8], r8",
            "mov [rcx + 16], r9",
            "mov [rcx + 24], r11",
            "pushfq",
            "pop rax",
            "mov [rcx + 32], rax",
            "movq rax, xmm0",
            "mov [rcx + 40], rax",
            "add rsp, 0x28",
            "ret",
        );
    }
    #[test]
    fn instruction_bridge_preserves_volatile_registers_flags_and_xmm() {
        unsafe {
            // An unknown site number runs the whole bridge with no game state.
            let relay = make_relay(
                bridge as *const () as usize,
                &[],
                0x7fff_ffff,
                bridge as *const () as usize,
            )
            .unwrap();
            let mut out = [0; 6];
            exercise(&mut out, relay);
            assert_eq!(&out[..4], &[123, 456, 789, 1011]);
            assert_eq!(out[4] & 0x41, 0x40); // ZF preserved, CF clear.
            assert_eq!(out[5], 123);
        }
    }
}
