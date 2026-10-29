use std::ffi::c_void;
use std::path::Path;

pub struct Library(*mut c_void);

unsafe impl Send for Library {}
unsafe impl Sync for Library {}

#[cfg(windows)]
mod sys {
    use std::ffi::c_void;

    #[link(name = "kernel32")]
    extern "system" {
        pub fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
        pub fn GetProcAddress(module: *mut c_void, name: *const std::ffi::c_char) -> *mut c_void;
        pub fn GetLastError() -> u32;
    }

    pub const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x8;
}

#[cfg(unix)]
mod sys {
    use std::ffi::{c_char, c_void};

    #[cfg_attr(target_os = "linux", link(name = "dl"))]
    extern "C" {
        pub fn dlopen(name: *const c_char, flags: i32) -> *mut c_void;
        pub fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
        pub fn dlerror() -> *const c_char;
    }

    pub const RTLD_NOW: i32 = 2;
}

impl Library {
    #[cfg(windows)]
    pub fn open(path: &Path) -> Result<Self, String> {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let handle = unsafe { sys::LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), sys::LOAD_WITH_ALTERED_SEARCH_PATH) };
        if handle.is_null() {
            return Err(format!("cannot load {} (Windows error {})", path.display(), unsafe { sys::GetLastError() }));
        }
        Ok(Self(handle))
    }

    #[cfg(unix)]
    pub fn open(path: &Path) -> Result<Self, String> {
        use std::os::unix::ffi::OsStrExt;
        let name = std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        let handle = unsafe { sys::dlopen(name.as_ptr(), sys::RTLD_NOW) };
        if handle.is_null() {
            let why = unsafe { sys::dlerror() };
            let why = if why.is_null() { String::new() } else { unsafe { std::ffi::CStr::from_ptr(why) }.to_string_lossy().into_owned() };
            return Err(format!("cannot load {}: {why}", path.display()));
        }
        Ok(Self(handle))
    }

    pub fn sym(&self, name: &str) -> Option<*mut c_void> {
        let name = std::ffi::CString::new(name).ok()?;
        #[cfg(windows)]
        let p = unsafe { sys::GetProcAddress(self.0, name.as_ptr()) };
        #[cfg(unix)]
        let p = unsafe { sys::dlsym(self.0, name.as_ptr()) };
        (!p.is_null()).then_some(p)
    }
}

#[cfg(windows)]
pub fn log_native_faults() {
    use std::cell::Cell;
    use std::ffi::c_void;

    #[repr(C)]
    struct ExceptionRecord {
        code: u32,
        flags: u32,
        chained: *mut ExceptionRecord,
        address: *mut c_void,
        parameters: u32,
        information: [usize; 15],
    }

    #[repr(C)]
    struct ExceptionPointers {
        record: *mut ExceptionRecord,
        context: *mut c_void,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn AddVectoredExceptionHandler(first: u32, handler: unsafe extern "system" fn(*mut ExceptionPointers) -> i32) -> *mut c_void;
        fn GetModuleHandleExW(flags: u32, address: *const c_void, module: *mut *mut c_void) -> i32;
        fn GetModuleFileNameW(module: *mut c_void, name: *mut u16, size: u32) -> u32;
        fn RtlCaptureStackBackTrace(skip: u32, count: u32, frames: *mut *mut c_void, hash: *mut u32) -> u16;
    }

    thread_local! {
        static REPORTING: Cell<bool> = const { Cell::new(false) };
    }

    fn locate(address: *const c_void) -> String {
        let mut module = std::ptr::null_mut();
        let mut name = [0u16; 260];
        unsafe {
            if GetModuleHandleExW(0x4 | 0x2, address, &mut module) == 0 {
                return format!("{address:p}");
            }
            let len = GetModuleFileNameW(module, name.as_mut_ptr(), name.len() as u32) as usize;
            let file = String::from_utf16_lossy(&name[..len]);
            let file = file.rsplit(['\\', '/']).next().unwrap_or("").to_owned();
            format!("{file}+{:#x}", address as usize - module as usize)
        }
    }

    unsafe extern "system" fn handler(info: *mut ExceptionPointers) -> i32 {
        let record = &*(*info).record;
        let fatal = matches!(record.code, 0xC000_0005 | 0xC000_00FD | 0xC000_001D | 0xC000_0094 | 0xC000_0374 | 0xC000_0409);
        if !fatal || REPORTING.with(|r| r.replace(true)) {
            return 0;
        }
        let mut frames = [std::ptr::null_mut::<c_void>(); 24];
        let n = RtlCaptureStackBackTrace(0, frames.len() as u32, frames.as_mut_ptr(), std::ptr::null_mut()) as usize;
        let trace: Vec<String> = frames[..n].iter().map(|f| locate(*f)).collect();
        let access = if record.code == 0xC000_0005 && record.parameters >= 2 {
            format!(" ({} address {:#x})", ["reading", "writing", "executing"].get(record.information[0]).unwrap_or(&"touching"), record.information[1])
        } else {
            String::new()
        };
        crate::log(&format!("native displays: fatal exception {:#x}{access} at {}; stack: {}", record.code, locate(record.address), trace.join(" < ")));
        REPORTING.with(|r| r.set(false));
        0
    }

    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        AddVectoredExceptionHandler(1, handler);
    });
}

#[cfg(not(windows))]
pub fn log_native_faults() {}
