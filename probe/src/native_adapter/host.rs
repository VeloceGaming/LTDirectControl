//! Helpers that touch no game address: SHA-256, and the fingerprint of the
//! running executable that decides which game version's hooks install.
use std::ffi::{c_void, OsString};
use std::os::windows::ffi::OsStringExt;

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn GetModuleFileNameW(module: *mut c_void, name: *mut u16, size: u32) -> u32;
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
/// The SHA-256 of the running executable's file on disk.
pub(super) fn executable_sha() -> Result<String, String> {
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
        sha256(&bytes)
    }
}
