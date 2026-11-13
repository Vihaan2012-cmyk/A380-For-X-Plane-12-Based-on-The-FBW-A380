//! Where X-Plane's main thread spends its time, with `FBW_MAIN_THREAD_PROFILE=1`:
//! every minute, ten seconds of whole call stacks. The stack is copied while the
//! thread is suspended and unwound after it resumes: it may hold the heap or the
//! loader's lock, so nothing that could wait on them runs while it is stopped.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(10);
const EVERY: Duration = Duration::from_secs(60);
const FIRST_AFTER: Duration = Duration::from_secs(30);
const SAMPLE_EVERY_MS: u32 = 4;
const STACK_COPY_BYTES: usize = 128 << 10;
const STACK_SLACK_BYTES: usize = 128 << 10;
const MAX_FRAMES: usize = 64;

const THREAD_SUSPEND_RESUME: u32 = 0x0002;
const THREAD_GET_CONTEXT: u32 = 0x0008;
const THREAD_QUERY_INFORMATION: u32 = 0x0040;
const CONTEXT_CONTROL_INTEGER: u32 = 0x0010_0003;
const CONTEXT_FLAGS_AT: usize = 0x30;
const CONTEXT_RSP_AT: usize = 0x98;
const CONTEXT_RIP_AT: usize = 0xF8;
const CONTEXT_NONVOLATILE_AT: [usize; 8] = [0x90, 0xA0, 0xA8, 0xB0, 0xD8, 0xE0, 0xE8, 0xF0];

#[repr(C, align(16))]
struct Context([u8; 1232]);

#[repr(C)]
#[derive(Default)]
struct ModuleInfo {
    base: usize,
    size: u32,
    entry: usize,
}

#[repr(C)]
struct RuntimeFunction {
    begin: u32,
    end: u32,
    unwind: u32,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentThreadId() -> u32;
    fn GetCurrentProcess() -> *mut c_void;
    fn OpenThread(access: u32, inherit: i32, id: u32) -> *mut c_void;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn SuspendThread(thread: *mut c_void) -> u32;
    fn ResumeThread(thread: *mut c_void) -> u32;
    fn GetThreadContext(thread: *mut c_void, context: *mut Context) -> i32;
    fn Sleep(milliseconds: u32);
    fn K32EnumProcessModules(process: *mut c_void, modules: *mut *mut c_void, bytes: u32, needed: *mut u32) -> i32;
    fn K32GetModuleInformation(process: *mut c_void, module: *mut c_void, info: *mut ModuleInfo, bytes: u32) -> i32;
    fn GetModuleFileNameW(module: *mut c_void, name: *mut u16, size: u32) -> u32;
    fn RtlLookupFunctionEntry(pc: u64, image_base: *mut u64, history: *mut c_void) -> *const RuntimeFunction;
    fn RtlVirtualUnwind(kind: u32, image_base: u64, pc: u64, entry: *const RuntimeFunction, context: *mut Context, data: *mut *mut c_void, establisher: *mut u64, pointers: *mut c_void) -> *mut c_void;
}

struct Module {
    base: usize,
    end: usize,
    label: String,
}

fn label_of(path: &str) -> String {
    let parts: Vec<&str> = path.split(['\\', '/']).collect();
    let file = parts.last().copied().unwrap_or("?");
    if !file.eq_ignore_ascii_case("win.xpl") {
        return file.to_string();
    }
    match parts.iter().rposition(|p| p.eq_ignore_ascii_case("plugins")) {
        Some(i) if i + 1 < parts.len() => format!("{} plugin", parts[i + 1]),
        _ => file.to_string(),
    }
}

fn modules() -> Vec<Module> {
    let mut handles = vec![std::ptr::null_mut::<c_void>(); 2048];
    let mut needed = 0u32;
    let mut list = Vec::new();
    unsafe {
        let process = GetCurrentProcess();
        let bytes = (handles.len() * std::mem::size_of::<*mut c_void>()) as u32;
        if K32EnumProcessModules(process, handles.as_mut_ptr(), bytes, &mut needed) == 0 {
            return list;
        }
        let count = (needed as usize / std::mem::size_of::<*mut c_void>()).min(handles.len());
        for &handle in &handles[..count] {
            let mut info = ModuleInfo::default();
            if K32GetModuleInformation(process, handle, &mut info, std::mem::size_of::<ModuleInfo>() as u32) == 0 {
                continue;
            }
            let mut name = [0u16; 1024];
            let n = GetModuleFileNameW(handle, name.as_mut_ptr(), name.len() as u32) as usize;
            let label = label_of(&String::from_utf16_lossy(&name[..n.min(name.len())]));
            list.push(Module { base: info.base, end: info.base + info.size as usize, label });
        }
    }
    list.sort_by_key(|m| m.base);
    list
}

fn find(list: &[Module], address: usize) -> Option<usize> {
    let i = list.partition_point(|m| m.base <= address);
    (i > 0 && address < list[i - 1].end).then(|| i - 1)
}

fn word(context: &Context, at: usize) -> usize {
    usize::from_le_bytes(context.0[at..at + 8].try_into().unwrap_or([0; 8]))
}

fn set_word(context: &mut Context, at: usize, value: usize) {
    context.0[at..at + 8].copy_from_slice(&value.to_le_bytes());
}

/// `frames` gets (return address, start of its function), leaf first. `copy` is
/// `STACK_COPY_BYTES + STACK_SLACK_BYTES` long, so an unwind step that reads a
/// little past the copied part stays inside it.
unsafe fn capture(thread: *mut c_void, stack_top: usize, copy: &mut [u8], frames: &mut Vec<(usize, usize)>) {
    frames.clear();
    let mut context = Context([0; 1232]);
    context.0[CONTEXT_FLAGS_AT..CONTEXT_FLAGS_AT + 4].copy_from_slice(&CONTEXT_CONTROL_INTEGER.to_le_bytes());
    if SuspendThread(thread) == u32::MAX {
        return;
    }
    let got = GetThreadContext(thread, &mut context) != 0;
    let rsp = word(&context, CONTEXT_RSP_AT);
    let used = if got && rsp != 0 && rsp < stack_top { (stack_top - rsp).min(STACK_COPY_BYTES) } else { 0 };
    if used > 0 {
        std::ptr::copy_nonoverlapping(rsp as *const u8, copy.as_mut_ptr(), used);
    }
    ResumeThread(thread);
    if used == 0 {
        return;
    }
    let base = copy.as_ptr() as usize;
    let relocate = |context: &mut Context| {
        for at in CONTEXT_NONVOLATILE_AT {
            let v = word(context, at);
            if v >= rsp && v < rsp + used {
                set_word(context, at, v - rsp + base);
            }
        }
    };
    set_word(&mut context, CONTEXT_RSP_AT, base);
    relocate(&mut context);
    while frames.len() < MAX_FRAMES {
        let pc = word(&context, CONTEXT_RIP_AT);
        if pc == 0 {
            break;
        }
        let mut image = 0u64;
        let entry = RtlLookupFunctionEntry(pc as u64, &mut image, std::ptr::null_mut());
        frames.push((pc, if entry.is_null() { pc } else { image as usize + (*entry).begin as usize }));
        let sp = word(&context, CONTEXT_RSP_AT);
        if sp < base || sp + 8 > base + used {
            break;
        }
        if entry.is_null() {
            set_word(&mut context, CONTEXT_RIP_AT, std::ptr::read_unaligned(sp as *const usize));
            set_word(&mut context, CONTEXT_RSP_AT, sp + 8);
        } else {
            let (mut data, mut establisher) = (std::ptr::null_mut(), 0u64);
            RtlVirtualUnwind(0, image, pc as u64, entry, &mut context, &mut data, &mut establisher, std::ptr::null_mut());
        }
        relocate(&mut context);
    }
}

#[derive(Default)]
struct Stacks {
    total: u32,
    under_plugin: u32,
    under_chromium: u32,
    leaf: HashMap<usize, u32>,
    chromium_leaf: HashMap<usize, u32>,
    plugin_functions: HashMap<(usize, usize), u32>,
    system_callers: HashMap<usize, u32>,
    system_call_sites: HashMap<(usize, usize), u32>,
    driver_callers: HashMap<(usize, usize), u32>,
}

const DRIVER: [&str; 3] = ["nvoglv64.dll", "nvgpucomp64.dll", "nvoglv32.dll"];

const SYSTEM: [&str; 6] = ["ntdll.dll", "kernelbase.dll", "kernel32.dll", "win32u.dll", "ucrtbase.dll", "user32.dll"];

impl Stacks {
    /// `frames` from `capture`. A leaf outside every module counts under `list.len()`.
    fn add(&mut self, frames: &[(usize, usize)], list: &[Module], ours: Option<usize>, chromium: Option<usize>) {
        let Some(&(pc, _)) = frames.first() else { return };
        let leaf = find(list, pc).unwrap_or(list.len());
        self.total += 1;
        *self.leaf.entry(leaf).or_default() += 1;
        let in_module = |m: Option<usize>, a: usize| m.is_some_and(|m| a >= list[m].base && a < list[m].end);
        if frames.iter().any(|(pc, _)| in_module(chromium, *pc)) {
            self.under_chromium += 1;
            *self.chromium_leaf.entry(leaf).or_default() += 1;
        }
        let system = |m: usize| list.get(m).is_some_and(|m| SYSTEM.iter().any(|s| m.label.eq_ignore_ascii_case(s)));
        if system(leaf) {
            if let Some((m, start)) = frames.iter().find_map(|(pc, start)| find(list, *pc).filter(|m| !system(*m)).map(|m| (m, *start))) {
                *self.system_callers.entry(m).or_default() += 1;
                *self.system_call_sites.entry((m, start - list[m].base)).or_default() += 1;
            }
        }
        let driver = |m: usize| list.get(m).is_some_and(|m| DRIVER.iter().any(|s| m.label.eq_ignore_ascii_case(s)));
        if frames.iter().any(|(pc, _)| find(list, *pc).is_some_and(driver)) {
            if let Some((m, start)) = frames.iter().find_map(|(pc, start)| find(list, *pc).filter(|m| !system(*m) && !driver(*m)).map(|m| (m, *start))) {
                *self.driver_callers.entry((m, start - list[m].base)).or_default() += 1;
            }
        }
        if let Some(&(_, start)) = frames.iter().find(|(pc, _)| in_module(ours, *pc)) {
            self.under_plugin += 1;
            let rva = start - ours.map_or(0, |m| list[m].base);
            *self.plugin_functions.entry((rva, leaf)).or_default() += 1;
        }
    }

    fn report(&self, list: &[Module]) -> Vec<String> {
        if self.total == 0 {
            return Vec::new();
        }
        let pct = |n: u32| n as f64 * 100. / self.total as f64;
        let name = |m: usize| list.get(m).map_or("(no module)", |m| m.label.as_str());
        let by_module = |map: &HashMap<usize, u32>| {
            let mut rows: Vec<_> = map.iter().collect();
            rows.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
            rows.iter().take(16).map(|(m, n)| format!("{} {:.1}", name(**m), pct(**n))).collect::<Vec<_>>().join(", ")
        };
        let mut functions: Vec<_> = self.plugin_functions.iter().collect();
        functions.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
        let functions: Vec<String> = functions.iter().take(40).map(|((rva, leaf), n)| format!("rva={rva:x} leaf={} {:.1}", name(*leaf), pct(**n))).collect();
        vec![
            format!("main thread: {} samples, our plugin in the stack {:.1}%, Chromium {:.1}%; leaf: {}", self.total, pct(self.under_plugin), pct(self.under_chromium), by_module(&self.leaf)),
            format!("main thread under Chromium, by leaf: {}", by_module(&self.chromium_leaf)),
            format!("main thread in our plugin: {}", functions.join(", ")),
            format!("main thread in system code (ntdll and friends), by the module that called it: {}", by_module(&self.system_callers)),
            format!("main thread in system code, by call site: {}", {
                let mut sites: Vec<_> = self.system_call_sites.iter().collect();
                sites.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
                sites.iter().take(12).map(|((m, rva), n)| format!("{}+{rva:x} {:.1}", name(*m), pct(**n))).collect::<Vec<_>>().join(", ")
            }),
            format!("main thread in the graphics driver, by who called it: {}", {
                let mut sites: Vec<_> = self.driver_callers.iter().collect();
                sites.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
                sites.iter().take(16).map(|((m, rva), n)| format!("{}+{rva:x} {:.1}", name(*m), pct(**n))).collect::<Vec<_>>().join(", ")
            }),
        ]
    }
}

fn chromium_of(list: &[Module]) -> Option<usize> {
    list.iter().position(|m| m.label.eq_ignore_ascii_case("libcef.dll"))
}

fn ours() -> usize {
    ours as fn() -> usize as usize
}

struct Running {
    worker: std::thread::JoinHandle<()>,
}

static RUNNING: Mutex<Option<Running>> = Mutex::new(None);
static STOP: AtomicBool = AtomicBool::new(false);

fn stack_top_here() -> usize {
    let top: usize;
    unsafe { std::arch::asm!("mov {}, gs:[0x08]", out(reg) top, options(nostack, readonly, preserves_flags)) };
    top
}

fn open_this_thread() -> *mut c_void {
    unsafe { OpenThread(THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION, 0, GetCurrentThreadId()) }
}

/// On the main thread, on its own stack.
pub fn start() {
    static WANTED: OnceLock<bool> = OnceLock::new();
    let from_settings = || {
        crate::xp::system_path().and_then(|r| crate::efb_settings::load(&r).get("xphfbw.mainThreadProfile").map(|v| v.as_bool() == Some(true) || v.as_str() == Some("true")))
    };
    if !*WANTED.get_or_init(|| std::env::var("FBW_MAIN_THREAD_PROFILE").is_ok_and(|v| v == "1") || from_settings().unwrap_or(false)) {
        return;
    }
    let Ok(mut running) = RUNNING.lock() else { return };
    if running.is_some() {
        return;
    }
    let thread = open_this_thread();
    if thread.is_null() {
        return;
    }
    let (handle, stack_top) = (thread as usize, stack_top_here());
    STOP.store(false, Ordering::SeqCst);
    let worker = std::thread::Builder::new().name("fbw main thread profile".into()).spawn(move || {
        let thread = handle as *mut c_void;
        let mut copy = vec![0u8; STACK_COPY_BYTES + STACK_SLACK_BYTES];
        let mut frames = Vec::with_capacity(MAX_FRAMES);
        let mut next = Instant::now() + FIRST_AFTER;
        while !STOP.load(Ordering::Relaxed) {
            if Instant::now() < next {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            next = Instant::now() + EVERY;
            let list = modules();
            let (ours, chromium) = (find(&list, ours()), chromium_of(&list));
            let mut stacks = Stacks::default();
            let started = Instant::now();
            while started.elapsed() < WINDOW && !STOP.load(Ordering::Relaxed) {
                unsafe { capture(thread, stack_top, &mut copy, &mut frames) };
                stacks.add(&frames, &list, ours, chromium);
                unsafe { Sleep(SAMPLE_EVERY_MS) };
            }
            for line in stacks.report(&list) {
                super::log(&line);
            }
        }
        unsafe { CloseHandle(thread) };
    });
    if let Ok(worker) = worker {
        *running = Some(Running { worker });
    }
}

/// Before the plugin's DLL unloads.
pub fn stop() {
    STOP.store(true, Ordering::SeqCst);
    let running = RUNNING.lock().ok().and_then(|mut r| r.take());
    if let Some(r) = running {
        let _ = r.worker.join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[inline(never)]
    fn spin_leaf(stop: &AtomicBool) {
        while !stop.load(Ordering::Relaxed) {
            std::hint::spin_loop();
        }
    }

    #[inline(never)]
    fn spin_caller(stop: &AtomicBool) -> u32 {
        spin_leaf(stop);
        std::hint::black_box(7)
    }

    #[test]
    fn a_stack_is_unwound_through_its_callers() {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let spinner = std::thread::spawn(move || {
            tx.send((open_this_thread() as usize, stack_top_here())).unwrap();
            spin_caller(&flag);
        });
        let (handle, top) = rx.recv().unwrap();
        std::thread::sleep(Duration::from_millis(50));
        let mut copy = vec![0u8; STACK_COPY_BYTES + STACK_SLACK_BYTES];
        let mut frames = Vec::with_capacity(MAX_FRAMES);
        let mut found = false;
        for _ in 0..50 {
            unsafe { capture(handle as *mut c_void, top, &mut copy, &mut frames) };
            found |= frames.iter().any(|(_, start)| *start == spin_caller as *const () as usize);
        }
        stop.store(true, Ordering::Relaxed);
        spinner.join().unwrap();
        unsafe { CloseHandle(handle as *mut c_void) };
        assert!(found, "spin_caller is on the stack: {frames:x?}");
    }

    #[test]
    fn a_sample_counts_for_its_leaf_and_for_who_is_under_it() {
        let list = vec![
            Module { base: 0x1000, end: 0x2000, label: "x-plane".into() },
            Module { base: 0x3000, end: 0x4000, label: "chromium".into() },
            Module { base: 0x5000, end: 0x6000, label: "ours".into() },
            Module { base: 0x7000, end: 0x8000, label: "ntdll".into() },
        ];
        let (ours, chromium) = (Some(2), Some(1));
        let mut stacks = Stacks::default();
        stacks.add(&[(0x7010, 0x7000), (0x5020, 0x5010), (0x3000, 0x3000), (0x1000, 0x1000)], &list, ours, chromium);
        stacks.add(&[(0x1100, 0x1100)], &list, ours, chromium);
        stacks.add(&[(0x9000, 0x9000)], &list, ours, chromium);
        stacks.add(&[], &list, ours, chromium);
        assert_eq!((stacks.total, stacks.under_plugin, stacks.under_chromium), (3, 1, 1));
        assert_eq!(stacks.plugin_functions.get(&(0x10, 3)), Some(&1), "the deepest of our functions, by what ran under it");
        let lines = stacks.report(&list);
        assert!(lines[0].contains("(no module) 33.3"), "{}", lines[0]);
        assert!(lines[1].contains("ntdll 33.3"), "{}", lines[1]);
        assert!(lines[2].contains("rva=10 leaf=ntdll 33.3"), "{}", lines[2]);
    }

    #[test]
    fn other_plugins_are_named_by_their_folder() {
        assert_eq!(label_of(r"D:\X-Plane 12\Resources\plugins\AviTab\win_x64\win.xpl"), "AviTab plugin");
        assert_eq!(label_of(r"C:\Windows\System32\ntdll.dll"), "ntdll.dll");
    }
}
