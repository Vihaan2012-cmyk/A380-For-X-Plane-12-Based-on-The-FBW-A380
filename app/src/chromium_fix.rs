//! CEF 152.0.6 crashes the app a second after it starts when kernel32.dll's
//! file version reads older than Windows 10 (GitHub issues #1 and #4):
//! `embedder_support::GetUniversalApiContractVersion` sends such a version to
//! a function that only knows Windows 10 releases, which ends in NOTREACHED.
//! Its other branch reads the real contract version from the registry.

use std::ffi::c_void;

const JUMP_TO_OLD_WINDOWS_RVA: usize = 0x1FD8CCC;
const JUMP_TO_OLD_WINDOWS: [u8; 6] = [0x0F, 0x8E, 0xA2, 0x01, 0x00, 0x00];
const NOP: u8 = 0x90;
const PAGE_EXECUTE_READWRITE: u32 = 0x40;

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn VirtualProtect(address: *mut c_void, size: usize, protect: u32, old: *mut u32) -> i32;
    fn FlushInstructionCache(process: *mut c_void, address: *const c_void, size: usize) -> i32;
    fn GetCurrentProcess() -> *mut c_void;
}

#[link(name = "version")]
extern "system" {
    fn GetFileVersionInfoSizeW(name: *const u16, handle: *mut u32) -> u32;
    fn GetFileVersionInfoW(name: *const u16, handle: u32, size: u32, data: *mut c_void) -> i32;
    fn VerQueryValueW(block: *const c_void, sub_block: *const u16, value: *mut *mut c_void, size: *mut u32) -> i32;
}

#[link(name = "ntdll")]
extern "system" {
    fn RtlGetVersion(info: *mut OsVersionInfo) -> i32;
}

#[repr(C)]
struct OsVersionInfo {
    size: u32,
    major: u32,
    minor: u32,
    build: u32,
    platform: u32,
    service_pack: [u16; 128],
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Whether the jump is gone. False when libcef.dll is not the build the
/// offset belongs to, which leaves it untouched.
pub fn skip_old_windows_branch() -> bool {
    unsafe {
        let module = GetModuleHandleW(wide("libcef.dll").as_ptr());
        if module.is_null() {
            return false;
        }
        let jump = (module as *mut u8).add(JUMP_TO_OLD_WINDOWS_RVA);
        if std::slice::from_raw_parts(jump, JUMP_TO_OLD_WINDOWS.len()) != JUMP_TO_OLD_WINDOWS {
            return false;
        }
        let mut old = 0;
        if VirtualProtect(jump.cast(), JUMP_TO_OLD_WINDOWS.len(), PAGE_EXECUTE_READWRITE, &mut old) == 0 {
            return false;
        }
        std::ptr::write_bytes(jump, NOP, JUMP_TO_OLD_WINDOWS.len());
        VirtualProtect(jump.cast(), JUMP_TO_OLD_WINDOWS.len(), old, &mut old);
        FlushInstructionCache(GetCurrentProcess(), jump.cast(), JUMP_TO_OLD_WINDOWS.len());
        true
    }
}

/// kernel32.dll's file version, read the way Chromium reads it.
fn kernel32_file_version() -> Option<[u32; 4]> {
    let name = wide("kernel32.dll");
    unsafe {
        let mut handle = 0;
        let size = GetFileVersionInfoSizeW(name.as_ptr(), &mut handle);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(name.as_ptr(), handle, size, data.as_mut_ptr().cast()) == 0 {
            return None;
        }
        let (mut value, mut length) = (std::ptr::null_mut(), 0);
        if VerQueryValueW(data.as_ptr().cast(), wide("\\").as_ptr(), &mut value, &mut length) == 0 || (length as usize) < 16 {
            return None;
        }
        let fixed = value.cast::<u32>();
        let (high, low) = (fixed.add(2).read_unaligned(), fixed.add(3).read_unaligned());
        Some([high >> 16, high & 0xFFFF, low >> 16, low & 0xFFFF])
    }
}

pub fn windows_versions() -> String {
    let mut os = OsVersionInfo { size: std::mem::size_of::<OsVersionInfo>() as u32, major: 0, minor: 0, build: 0, platform: 0, service_pack: [0; 128] };
    unsafe { RtlGetVersion(&mut os) };
    format!("Windows {}.{}.{}, kernel32.dll reads {:?}", os.major, os.minor, os.build, kernel32_file_version())
}
