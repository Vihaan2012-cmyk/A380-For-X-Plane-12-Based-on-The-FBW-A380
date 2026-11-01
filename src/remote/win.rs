//! The few Windows primitives the systems bridge needs: a named shared
//! memory block, named auto-reset events, and waiting on another process.
//! On Linux and macOS the same names live inside this process, which is where
//! the systems and the browser server run there.

use std::ffi::c_void;

pub type Handle = *mut c_void;

const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
const PAGE_READWRITE: u32 = 0x04;
const FILE_MAP_ALL_ACCESS: u32 = 0x000F_001F;
const EVENT_MODIFY_STATE: u32 = 0x0002;
const SYNCHRONIZE: u32 = 0x0010_0000;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
pub const WAIT_OBJECT_0: u32 = 0;
pub const WAIT_TIMEOUT: u32 = 0x102;
pub const INFINITE: u32 = 0xFFFF_FFFF;

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn CreateFileMappingW(file: Handle, attributes: *mut c_void, protect: u32, size_high: u32, size_low: u32, name: *const u16) -> Handle;
    fn OpenFileMappingW(access: u32, inherit: i32, name: *const u16) -> Handle;
    fn MapViewOfFile(mapping: Handle, access: u32, offset_high: u32, offset_low: u32, bytes: usize) -> *mut c_void;
    fn UnmapViewOfFile(address: *const c_void) -> i32;
    fn CreateEventW(attributes: *mut c_void, manual_reset: i32, initial: i32, name: *const u16) -> Handle;
    fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> Handle;
    fn SetEvent(event: Handle) -> i32;
    fn WaitForSingleObject(object: Handle, milliseconds: u32) -> u32;
    fn WaitForMultipleObjects(count: u32, objects: *const Handle, wait_all: i32, milliseconds: u32) -> u32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn GetExitCodeProcess(process: Handle, code: *mut u32) -> i32;
    fn CloseHandle(object: Handle) -> i32;
}

/// The MSFS wasm build compiles this module, though nothing there runs a second
/// process. Each call answers as the real one does when the object cannot be
/// had: a null handle, a failed wait. `Shared::create`, `Event::open` and the
/// rest then return `None` through their existing checks, and
/// `remote::Systems::Remote` is simply never constructed.
#[cfg(target_arch = "wasm32")]
#[allow(non_snake_case)]
mod stand_ins {
    use super::Handle;
    use std::ffi::c_void;
    const WAIT_FAILED: u32 = 0xFFFF_FFFF;
    pub unsafe fn CreateFileMappingW(_: Handle, _: *mut c_void, _: u32, _: u32, _: u32, _: *const u16) -> Handle { std::ptr::null_mut() }
    pub unsafe fn OpenFileMappingW(_: u32, _: i32, _: *const u16) -> Handle { std::ptr::null_mut() }
    pub unsafe fn MapViewOfFile(_: Handle, _: u32, _: u32, _: u32, _: usize) -> *mut c_void { std::ptr::null_mut() }
    pub unsafe fn UnmapViewOfFile(_: *const c_void) -> i32 { 0 }
    pub unsafe fn CreateEventW(_: *mut c_void, _: i32, _: i32, _: *const u16) -> Handle { std::ptr::null_mut() }
    pub unsafe fn OpenEventW(_: u32, _: i32, _: *const u16) -> Handle { std::ptr::null_mut() }
    pub unsafe fn SetEvent(_: Handle) -> i32 { 0 }
    pub unsafe fn WaitForSingleObject(_: Handle, _: u32) -> u32 { super::WAIT_TIMEOUT }
    pub unsafe fn WaitForMultipleObjects(_: u32, _: *const Handle, _: i32, _: u32) -> u32 { WAIT_FAILED }
    pub unsafe fn OpenProcess(_: u32, _: i32, _: u32) -> Handle { std::ptr::null_mut() }
    pub unsafe fn GetExitCodeProcess(_: Handle, _: *mut u32) -> i32 { 0 }
    pub unsafe fn CloseHandle(_: Handle) -> i32 { 0 }
}
#[cfg(target_arch = "wasm32")]
use stand_ins::*;

/// The calls this module and the bridge's named mutex make, over objects found
/// by name in this process: a handle holds one reference, and an object goes
/// away with its last handle, as on Windows.
#[cfg(all(not(windows), not(target_arch = "wasm32")))]
#[allow(non_snake_case)]
pub mod in_process {
    use super::{Handle, INFINITE, WAIT_OBJECT_0, WAIT_TIMEOUT};
    use std::alloc::{alloc_zeroed, dealloc, Layout};
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::sync::{Arc, Condvar, Mutex, Weak};
    use std::thread::ThreadId;
    use std::time::{Duration, Instant};

    const WAIT_FAILED: u32 = 0xFFFF_FFFF;

    enum Object {
        Memory { ptr: *mut u8, layout: Layout },
        Event { set: Mutex<bool>, changed: Condvar },
        Lock { owner: Mutex<Option<(ThreadId, u32)>>, changed: Condvar },
    }

    unsafe impl Send for Object {}
    unsafe impl Sync for Object {}

    impl Drop for Object {
        fn drop(&mut self) {
            if let Object::Memory { ptr, layout } = self {
                unsafe { dealloc(*ptr, *layout) };
            }
        }
    }

    static NAMED: Mutex<Option<HashMap<String, Weak<Object>>>> = Mutex::new(None);

    unsafe fn name_of(name: *const u16) -> String {
        let mut n = 0;
        while *name.add(n) != 0 {
            n += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(name, n))
    }

    fn handle_of(object: Arc<Object>) -> Handle {
        Arc::into_raw(object) as Handle
    }

    fn found(name: &str) -> Option<Arc<Object>> {
        let named = NAMED.lock().unwrap_or_else(|e| e.into_inner());
        named.as_ref().and_then(|m| m.get(name)).and_then(Weak::upgrade)
    }

    fn find_or_make(name: &str, make: impl FnOnce() -> Option<Object>) -> Handle {
        let mut named = NAMED.lock().unwrap_or_else(|e| e.into_inner());
        let map = named.get_or_insert_with(HashMap::new);
        if let Some(existing) = map.get(name).and_then(Weak::upgrade) {
            return handle_of(existing);
        }
        map.retain(|_, w| w.strong_count() > 0);
        let Some(object) = make() else { return std::ptr::null_mut() };
        let object = Arc::new(object);
        map.insert(name.to_string(), Arc::downgrade(&object));
        handle_of(object)
    }

    unsafe fn object<'a>(handle: Handle) -> &'a Object {
        &*(handle as *const Object)
    }

    pub unsafe fn CreateFileMappingW(_: Handle, _: *mut c_void, _: u32, high: u32, low: u32, name: *const u16) -> Handle {
        let bytes = ((high as usize) << 32) | low as usize;
        find_or_make(&name_of(name), || {
            let layout = Layout::from_size_align(bytes.max(1), 4096).ok()?;
            let ptr = alloc_zeroed(layout);
            (!ptr.is_null()).then_some(Object::Memory { ptr, layout })
        })
    }

    pub unsafe fn OpenFileMappingW(_: u32, _: i32, name: *const u16) -> Handle {
        found(&name_of(name)).map_or(std::ptr::null_mut(), handle_of)
    }

    pub unsafe fn MapViewOfFile(mapping: Handle, _: u32, _: u32, _: u32, _: usize) -> *mut c_void {
        match object(mapping) {
            Object::Memory { ptr, .. } => *ptr as *mut c_void,
            _ => std::ptr::null_mut(),
        }
    }

    pub unsafe fn UnmapViewOfFile(_: *const c_void) -> i32 {
        1
    }

    pub unsafe fn CreateEventW(_: *mut c_void, _: i32, _: i32, name: *const u16) -> Handle {
        find_or_make(&name_of(name), || Some(Object::Event { set: Mutex::new(false), changed: Condvar::new() }))
    }

    pub unsafe fn OpenEventW(_: u32, _: i32, name: *const u16) -> Handle {
        found(&name_of(name)).map_or(std::ptr::null_mut(), handle_of)
    }

    pub unsafe fn SetEvent(event: Handle) -> i32 {
        if let Object::Event { set, changed } = object(event) {
            *set.lock().unwrap_or_else(|e| e.into_inner()) = true;
            changed.notify_one();
            return 1;
        }
        0
    }

    pub unsafe fn CreateMutexW(_: *mut c_void, _: i32, name: *const u16) -> Handle {
        find_or_make(&name_of(name), || Some(Object::Lock { owner: Mutex::new(None), changed: Condvar::new() }))
    }

    pub unsafe fn OpenMutexW(_: u32, _: i32, name: *const u16) -> Handle {
        found(&name_of(name)).map_or(std::ptr::null_mut(), handle_of)
    }

    pub unsafe fn ReleaseMutex(mutex: Handle) -> i32 {
        if let Object::Lock { owner, changed } = object(mutex) {
            let mut o = owner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((thread, depth)) = *o {
                if thread == std::thread::current().id() {
                    *o = (depth > 1).then_some((thread, depth - 1));
                    changed.notify_one();
                    return 1;
                }
            }
        }
        0
    }

    /// Waits on `changed` until `ready` says yes or the time is up.
    fn wait_until<T>(guard: &Mutex<T>, changed: &Condvar, milliseconds: u32, mut ready: impl FnMut(&mut T) -> bool) -> u32 {
        let deadline = (milliseconds != INFINITE).then(|| Instant::now() + Duration::from_millis(milliseconds as u64));
        let mut value = guard.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if ready(&mut value) {
                return WAIT_OBJECT_0;
            }
            match deadline {
                None => value = changed.wait(value).unwrap_or_else(|e| e.into_inner()),
                Some(at) => {
                    let left = at.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return WAIT_TIMEOUT;
                    }
                    value = changed.wait_timeout(value, left).unwrap_or_else(|e| e.into_inner()).0;
                }
            }
        }
    }

    pub unsafe fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32 {
        if handle.is_null() {
            return WAIT_FAILED;
        }
        match object(handle) {
            Object::Event { set, changed } => wait_until(set, changed, milliseconds, |s| std::mem::take(s)),
            Object::Lock { owner, changed } => {
                let me = std::thread::current().id();
                wait_until(owner, changed, milliseconds, |o| match *o {
                    None => {
                        *o = Some((me, 1));
                        true
                    }
                    Some((thread, depth)) if thread == me => {
                        *o = Some((thread, depth + 1));
                        true
                    }
                    Some(_) => false,
                })
            }
            Object::Memory { .. } => WAIT_FAILED,
        }
    }

    pub unsafe fn WaitForMultipleObjects(count: u32, objects: *const Handle, _: i32, milliseconds: u32) -> u32 {
        let objects = std::slice::from_raw_parts(objects, count as usize);
        if let [only] = objects {
            return WaitForSingleObject(*only, milliseconds);
        }
        let started = Instant::now();
        loop {
            for (i, o) in objects.iter().enumerate() {
                if WaitForSingleObject(*o, 0) == WAIT_OBJECT_0 {
                    return WAIT_OBJECT_0 + i as u32;
                }
            }
            if milliseconds != INFINITE && started.elapsed() >= Duration::from_millis(milliseconds as u64) {
                return WAIT_TIMEOUT;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    pub unsafe fn OpenProcess(_: u32, _: i32, _: u32) -> Handle {
        std::ptr::null_mut()
    }

    pub unsafe fn GetExitCodeProcess(_: Handle, _: *mut u32) -> i32 {
        0
    }

    pub unsafe fn CloseHandle(handle: Handle) -> i32 {
        if handle.is_null() {
            return 0;
        }
        drop(Arc::from_raw(handle as *const Object));
        1
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn wide(s: &str) -> Vec<u16> {
            s.encode_utf16().chain(std::iter::once(0)).collect()
        }

        #[test]
        fn a_name_finds_the_same_memory_until_its_last_handle_closes() {
            let n = wide("in_process_memory");
            unsafe {
                let a = CreateFileMappingW(std::ptr::null_mut(), std::ptr::null_mut(), 0, 0, 64, n.as_ptr());
                let b = OpenFileMappingW(0, 0, n.as_ptr());
                assert_eq!(MapViewOfFile(a, 0, 0, 0, 64), MapViewOfFile(b, 0, 0, 0, 64));
                CloseHandle(a);
                CloseHandle(b);
                assert!(OpenFileMappingW(0, 0, n.as_ptr()).is_null());
            }
        }

        #[test]
        fn an_event_resets_when_a_wait_takes_it() {
            let n = wide("in_process_event");
            unsafe {
                let e = CreateEventW(std::ptr::null_mut(), 0, 0, n.as_ptr());
                assert_eq!(WaitForSingleObject(e, 0), WAIT_TIMEOUT);
                SetEvent(e);
                assert_eq!(WaitForSingleObject(e, 0), WAIT_OBJECT_0);
                assert_eq!(WaitForSingleObject(e, 5), WAIT_TIMEOUT);
                CloseHandle(e);
            }
        }

        #[test]
        fn a_mutex_can_be_taken_again_by_its_owner_only() {
            let n = wide("in_process_mutex");
            unsafe {
                let m = CreateMutexW(std::ptr::null_mut(), 0, n.as_ptr());
                assert_eq!(WaitForSingleObject(m, 0), WAIT_OBJECT_0);
                assert_eq!(WaitForSingleObject(m, 0), WAIT_OBJECT_0);
                let other = m as usize;
                let got = std::thread::spawn(move || WaitForSingleObject(other as Handle, 0)).join().unwrap();
                assert_eq!(got, WAIT_TIMEOUT);
                ReleaseMutex(m);
                ReleaseMutex(m);
                let got = std::thread::spawn(move || {
                    let r = WaitForSingleObject(other as Handle, 0);
                    ReleaseMutex(other as Handle);
                    r
                })
                .join()
                .unwrap();
                assert_eq!(got, WAIT_OBJECT_0);
                CloseHandle(m);
            }
        }
    }
}
#[cfg(all(not(windows), not(target_arch = "wasm32")))]
use in_process::*;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// A named block of memory both processes map.
pub struct Shared {
    mapping: Handle,
    view: *mut u8,
    pub len: usize,
}

unsafe impl Send for Shared {}

impl Shared {
    pub fn create(name: &str, len: usize) -> Option<Self> {
        let n = wide(name);
        let mapping = unsafe {
            CreateFileMappingW(INVALID_HANDLE_VALUE, std::ptr::null_mut(), PAGE_READWRITE, (len as u64 >> 32) as u32, len as u32, n.as_ptr())
        };
        Self::map(mapping, len)
    }

    pub fn open(name: &str, len: usize) -> Option<Self> {
        let n = wide(name);
        let mapping = unsafe { OpenFileMappingW(FILE_MAP_ALL_ACCESS, 0, n.as_ptr()) };
        Self::map(mapping, len)
    }

    fn map(mapping: Handle, len: usize) -> Option<Self> {
        if mapping.is_null() {
            return None;
        }
        let view = unsafe { MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, len) } as *mut u8;
        if view.is_null() {
            unsafe { CloseHandle(mapping) };
            return None;
        }
        Some(Self { mapping, view, len })
    }

    pub fn ptr(&self) -> *mut u8 {
        self.view
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        unsafe {
            UnmapViewOfFile(self.view as *const c_void);
            CloseHandle(self.mapping);
        }
    }
}

/// A named auto-reset event.
pub struct Event(Handle);

unsafe impl Send for Event {}

impl Event {
    pub fn create(name: &str) -> Option<Self> {
        let n = wide(name);
        let h = unsafe { CreateEventW(std::ptr::null_mut(), 0, 0, n.as_ptr()) };
        (!h.is_null()).then_some(Self(h))
    }

    pub fn open(name: &str) -> Option<Self> {
        let n = wide(name);
        let h = unsafe { OpenEventW(EVENT_MODIFY_STATE | SYNCHRONIZE, 0, n.as_ptr()) };
        (!h.is_null()).then_some(Self(h))
    }

    pub fn set(&self) {
        unsafe { SetEvent(self.0) };
    }

    /// `WAIT_OBJECT_0` when signalled, `WAIT_TIMEOUT` otherwise.
    pub fn wait(&self, milliseconds: u32) -> u32 {
        unsafe { WaitForSingleObject(self.0, milliseconds) }
    }

    pub fn handle(&self) -> Handle {
        self.0
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

/// Another process, to notice when it has gone.
pub struct Process(Handle);

unsafe impl Send for Process {}

impl Process {
    pub fn open(pid: u32) -> Option<Self> {
        let h = unsafe { OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        (!h.is_null()).then_some(Self(h))
    }

    pub fn handle(&self) -> Handle {
        self.0
    }

    pub fn exit_code(&self) -> Option<u32> {
        let mut code = 0;
        (unsafe { GetExitCodeProcess(self.0, &mut code) } != 0).then_some(code)
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

/// Wait for the first of `objects`; its index, or `None` on timeout/failure.
pub fn wait_any(objects: &[Handle], milliseconds: u32) -> Option<usize> {
    let r = unsafe { WaitForMultipleObjects(objects.len() as u32, objects.as_ptr(), 0, milliseconds) };
    ((r as usize) < objects.len()).then_some(r as usize)
}
