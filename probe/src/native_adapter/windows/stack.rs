//! Another thread's call stack, for the worker stall report
//! (crate::worker_watch). The thread is suspended only while its frames are
//! copied into a fixed array: nothing here allocates or logs until it runs
//! again, because it may hold the heap or log lock.
use super::tooltips::{MemoryRegion, VirtualQuery};
use super::*;

const FRAMES: usize = 48;
// CONTEXT (x64) is 1232 bytes, 16-aligned; only these fields are read.
const CONTEXT_FLAGS: usize = 0x30;
const CONTEXT_RSP: usize = 0x98;
const CONTEXT_RIP: usize = 0xf8;
const CONTEXT_CONTROL_INTEGER: u32 = 0x10_0003;
#[repr(C, align(16))]
struct Context([u8; 1232]);
impl Context {
    fn word(&self, offset: usize) -> u64 {
        u64::from_le_bytes(self.0[offset..offset + 8].try_into().unwrap())
    }
    fn set_word(&mut self, offset: usize, value: u64) {
        self.0[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
}
#[link(name = "kernel32")]
extern "system" {
    fn OpenThread(access: u32, inherit: i32, thread: u32) -> *mut c_void;
    fn SuspendThread(thread: *mut c_void) -> u32;
    fn ResumeThread(thread: *mut c_void) -> u32;
    fn GetThreadContext(thread: *mut c_void, context: *mut Context) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn RtlLookupFunctionEntry(pc: u64, image: *mut u64, history: *mut c_void) -> *mut c_void;
    fn RtlVirtualUnwind(
        handler: u32,
        image: u64,
        pc: u64,
        function: *mut c_void,
        context: *mut Context,
        handler_data: *mut *mut c_void,
        establisher: *mut u64,
        pointers: *mut c_void,
    ) -> *mut c_void;
}

/// The suspended thread's return addresses, innermost first.
unsafe fn walk(thread: *mut c_void, frames: &mut [u64; FRAMES]) -> usize {
    let mut context = Context([0; 1232]);
    context.0[CONTEXT_FLAGS..CONTEXT_FLAGS + 4]
        .copy_from_slice(&CONTEXT_CONTROL_INTEGER.to_le_bytes());
    if GetThreadContext(thread, &mut context) == 0 {
        return 0;
    }
    // The committed stack from the stack pointer up; every frame must stay
    // inside it, so a bad unwind ends the walk instead of reading elsewhere.
    let mut info = MemoryRegion::default();
    let start = context.word(CONTEXT_RSP);
    if VirtualQuery(start as *const c_void, &mut info, size_of::<MemoryRegion>()) == 0 {
        return 0;
    }
    let end = (info.base + info.size) as u64;
    let mut count = 0;
    while count < FRAMES {
        let (pc, sp) = (context.word(CONTEXT_RIP), context.word(CONTEXT_RSP));
        if pc == 0 || sp < start || sp.saturating_add(8) > end || !sp.is_multiple_of(8) {
            break;
        }
        frames[count] = pc;
        count += 1;
        let mut image = 0;
        let function = RtlLookupFunctionEntry(pc, &mut image, std::ptr::null_mut());
        if function.is_null() {
            // A leaf, or generated code without unwind data: its return
            // address is on top of the stack.
            context.set_word(CONTEXT_RIP, std::ptr::read_volatile(sp as *const u64));
            context.set_word(CONTEXT_RSP, sp + 8);
        } else {
            let (mut data, mut establisher) = (std::ptr::null_mut(), 0);
            RtlVirtualUnwind(
                0,
                image,
                pc,
                function,
                &mut context,
                &mut data,
                &mut establisher,
                std::ptr::null_mut(),
            );
            if context.word(CONTEXT_RSP) <= sp {
                break;
            }
        }
    }
    count
}

/// `thread`'s call stack as `module+0xoffset`, innermost first; empty when
/// it cannot be read (or is the calling thread).
pub(crate) fn thread_stack(thread: u64) -> Vec<String> {
    if thread == 0 || thread == crate::platform_input::thread_id() {
        return Vec::new();
    }
    let mut frames = [0; FRAMES];
    let count = unsafe {
        // THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION
        let handle = OpenThread(0x4a, 0, thread as u32);
        if handle.is_null() {
            return Vec::new();
        }
        let count = if SuspendThread(handle) == u32::MAX {
            0
        } else {
            let count = walk(handle, &mut frames);
            ResumeThread(handle);
            count
        };
        CloseHandle(handle);
        count
    };
    frames[..count]
        .iter()
        .map(|frame| unsafe { module_offset(*frame as usize) })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn another_threads_stack_is_read_and_it_keeps_running() {
        let (tell, told) = std::sync::mpsc::channel();
        let (stop, stopped) = std::sync::mpsc::channel::<()>();
        let thread = std::thread::spawn(move || {
            tell.send(crate::platform_input::thread_id()).unwrap();
            // Blocks in a wait, as a stuck worker might.
            stopped.recv().ok();
        });
        let id = told.recv().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let frames = thread_stack(id);
        assert!(frames.len() >= 3, "{frames:?}");
        assert!(frames[0].contains("ntdll"), "{frames:?}");
        assert!(
            frames.iter().any(|f| f.contains("lt_direct_control")),
            "{frames:?}"
        );
        assert!(thread_stack(crate::platform_input::thread_id()).is_empty());
        stop.send(()).unwrap();
        thread.join().unwrap();
    }
}
