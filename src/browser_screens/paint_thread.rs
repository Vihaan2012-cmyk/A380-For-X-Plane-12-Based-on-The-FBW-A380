//! Paints copied off X-Plane's main thread. With software compositing CEF's paint
//! buffer stays mapped between paints, so the copy can happen after `on_paint` has
//! returned. Chromium can replace that mapping without telling us, for example on a
//! reload, so it is read through `ReadProcessMemory`, which fails rather than faults;
//! the next paint then goes whole. A paint arriving during a read can tear one frame
//! at worst: the rows it wrote are in its own damage, and are copied again.

use std::ffi::c_int;
#[cfg(windows)]
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::paint_copy::copy_changed;
use crate::xphfbw_bridge::{ScreenBlock, MAX_DIRTY_RECTS};

#[cfg(windows)]
extern "system" {
    fn ReadProcessMemory(process: *mut c_void, base: *const c_void, buffer: *mut c_void, size: usize, read: *mut usize) -> i32;
    fn GetCurrentProcess() -> *mut c_void;
}

/// `to.len()` bytes of this process from `from`, or false where that memory is gone (no fault).
unsafe fn read_own(from: usize, to: &mut [u8]) -> bool {
    #[cfg(windows)]
    {
        let mut read = 0usize;
        ReadProcessMemory(GetCurrentProcess(), from as *const c_void, to.as_mut_ptr().cast(), to.len(), &mut read) != 0 && read == to.len()
    }
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            static mach_task_self_: u32;
            fn mach_vm_read_overwrite(task: u32, address: u64, size: u64, data: u64, out: *mut u64) -> c_int;
        }
        let mut read = 0u64;
        mach_vm_read_overwrite(mach_task_self_, from as u64, to.len() as u64, to.as_mut_ptr() as u64, &mut read) == 0 && read == to.len() as u64
    }
}

static READ_FAILURES: AtomicU64 = AtomicU64::new(0);
static COPIES: AtomicU64 = AtomicU64::new(0);
static COPY_NS: AtomicU64 = AtomicU64::new(0);
static REPORTED_BYTES: AtomicU64 = AtomicU64::new(0);
static CHANGED_BYTES: AtomicU64 = AtomicU64::new(0);

pub fn read_failures() -> u64 {
    READ_FAILURES.load(Ordering::Relaxed)
}

/// What the copies did since the last call.
pub fn take_report() -> String {
    let copies = COPIES.swap(0, Ordering::Relaxed);
    let ns = COPY_NS.swap(0, Ordering::Relaxed);
    let reported = REPORTED_BYTES.swap(0, Ordering::Relaxed);
    let changed = CHANGED_BYTES.swap(0, Ordering::Relaxed);
    format!(
        "{copies} copies, {:.2} ms each, {:.0} MB reported, {:.0}% of it changed, {} failed reads",
        ns as f64 / copies.max(1) as f64 / 1e6,
        reported as f64 / 1e6,
        changed as f64 * 100. / reported.max(1) as f64,
        read_failures()
    )
}

#[derive(Default)]
struct Wake {
    due: Mutex<bool>,
    changed: Condvar,
}

impl Wake {
    fn notify(&self) {
        *self.due.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.changed.notify_one();
    }

    fn wait(&self) {
        let mut due = self.due.lock().unwrap_or_else(|e| e.into_inner());
        if !*due {
            due = self.changed.wait_timeout(due, Duration::from_millis(100)).unwrap_or_else(|e| e.into_inner()).0;
        }
        *due = false;
    }
}

#[derive(Default)]
struct Pending {
    buffer: usize,
    rects: Vec<[u32; 4]>,
    whole: bool,
}

pub struct PaintSlot {
    block: ScreenBlock,
    pending: Mutex<Pending>,
    wake: Arc<Wake>,
}

unsafe impl Send for PaintSlot {}
unsafe impl Sync for PaintSlot {}

impl PaintSlot {
    fn size(&self) -> (u32, u32) {
        let h = self.block.header();
        (h.width.load(Ordering::Relaxed), h.height.load(Ordering::Relaxed))
    }

    pub fn fits(&self, width: c_int, height: c_int) -> bool {
        self.size() == (width as u32, height as u32)
    }

    /// `src` is a whole frame.
    pub fn copy_now(&self, src: &[u8], rects: &[[u32; 4]]) {
        let (width, _) = self.size();
        self.copy_rows(src, 0, rects, width as usize * 4);
    }

    pub fn hand_over(&self, buffer: usize, rects: &[[u32; 4]]) {
        let mut p = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        p.buffer = buffer;
        p.rects.extend_from_slice(rects);
        drop(p);
        self.wake.notify();
    }

    /// `rows` holds frame rows from `first_row` on.
    fn copy_rows(&self, rows: &[u8], first_row: u32, rects: &[[u32; 4]], stride: usize) {
        let started = Instant::now();
        let h = self.block.header();
        let (width, height) = self.size();
        let dst = &mut self.block.pixels()[first_row as usize * stride..];
        let mut published = Vec::with_capacity(8);
        h.writing.store(1, Ordering::Relaxed);
        for [x, y, w, rect_h] in rects.iter().copied() {
            let left = MAX_DIRTY_RECTS.saturating_sub(published.len());
            copy_changed(rows, dst, stride, [x, y - first_row, w, rect_h], left, &mut published);
        }
        published.iter_mut().for_each(|r| r[1] += first_row);
        let bytes = |list: &[[u32; 4]]| list.iter().map(|r| r[2] as u64 * r[3] as u64 * 4).sum::<u64>();
        REPORTED_BYTES.fetch_add(bytes(rects), Ordering::Relaxed);
        CHANGED_BYTES.fetch_add(bytes(&published), Ordering::Relaxed);
        if h.frame.load(Ordering::Relaxed) == 0 || published.len() >= MAX_DIRTY_RECTS {
            published.clear();
            published.push([0, 0, width, height]);
        }
        if !published.is_empty() {
            h.publish(&published);
        }
        h.writing.store(0, Ordering::Relaxed);
        COPIES.fetch_add(1, Ordering::Relaxed);
        COPY_NS.fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
    }

    fn run_pending(&self, scratch: &mut Vec<u8>) {
        let (buffer, mut rects, whole) = {
            let mut p = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            if p.rects.is_empty() {
                return;
            }
            (p.buffer, std::mem::take(&mut p.rects), std::mem::take(&mut p.whole))
        };
        let (width, height) = self.size();
        if whole {
            rects = vec![[0, 0, width, height]];
        }
        let stride = width as usize * 4;
        let y0 = rects.iter().map(|r| r[1]).min().unwrap_or(0);
        let y1 = rects.iter().map(|r| r[1] + r[3]).max().unwrap_or(0).min(height);
        if y1 <= y0 {
            return;
        }
        let bytes = (y1 - y0) as usize * stride;
        scratch.resize(bytes, 0);
        let ok = unsafe { read_own(buffer + y0 as usize * stride, scratch) };
        if !ok {
            READ_FAILURES.fetch_add(1, Ordering::Relaxed);
            self.pending.lock().unwrap_or_else(|e| e.into_inner()).whole = true;
            return;
        }
        self.copy_rows(scratch, y0, &rects, stride);
    }
}

pub struct PaintThread {
    wake: Arc<Wake>,
    stop: Arc<AtomicBool>,
    worker: JoinHandle<()>,
}

impl PaintThread {
    /// One slot per block, in order.
    pub fn start(blocks: Vec<ScreenBlock>) -> (Self, Vec<Arc<PaintSlot>>) {
        let wake = Arc::new(Wake::default());
        let stop = Arc::new(AtomicBool::new(false));
        let slots: Vec<Arc<PaintSlot>> = blocks.into_iter().map(|block| Arc::new(PaintSlot { block, pending: Mutex::new(Pending::default()), wake: wake.clone() })).collect();
        let worker = {
            let (slots, wake, stop) = (slots.clone(), wake.clone(), stop.clone());
            std::thread::Builder::new()
                .name("fbw screen copies".into())
                .spawn(move || {
                    let mut scratch = Vec::new();
                    while !stop.load(Ordering::Relaxed) {
                        wake.wait();
                        for slot in &slots {
                            slot.run_pending(&mut scratch);
                        }
                    }
                })
                .expect("the thread starts")
        };
        (Self { wake, stop, worker }, slots)
    }

    pub fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        self.wake.notify();
        let _ = self.worker.join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: u32 = 32;
    const H: u32 = 32;

    fn start(tag: &str) -> (PaintThread, Arc<PaintSlot>, ScreenBlock) {
        let block = ScreenBlock::create(tag, "SCREEN_DU_PFDL", W, H).unwrap();
        let view = ScreenBlock::open(tag, "SCREEN_DU_PFDL", W, H).unwrap();
        let (thread, mut slots) = PaintThread::start(vec![block]);
        (thread, slots.remove(0), view)
    }

    fn wait_for_frame(view: &ScreenBlock, frame: u64) {
        let until = Instant::now() + Duration::from_secs(5);
        while view.header().frame.load(Ordering::Acquire) < frame {
            assert!(Instant::now() < until, "no paint published");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn a_handed_over_paint_reaches_the_block() {
        let (thread, slot, view) = start(&format!("pt_a_{}", std::process::id()));
        let buffer = vec![7u8; (W * H * 4) as usize];
        slot.hand_over(buffer.as_ptr() as usize, &[[0, 0, W, H]]);
        wait_for_frame(&view, 1);
        assert_eq!(view.pixels()[..], buffer[..]);
        thread.stop();
    }

    #[test]
    fn a_repeat_of_the_same_pixels_publishes_nothing() {
        let (thread, slot, view) = start(&format!("pt_b_{}", std::process::id()));
        let buffer = vec![7u8; (W * H * 4) as usize];
        slot.hand_over(buffer.as_ptr() as usize, &[[0, 0, W, H]]);
        wait_for_frame(&view, 1);
        slot.hand_over(buffer.as_ptr() as usize, &[[0, 0, W, H]]);
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(view.header().frame.load(Ordering::Acquire), 1);
        thread.stop();
    }

    #[test]
    fn rows_below_the_first_land_where_they_belong() {
        let (thread, slot, view) = start(&format!("pt_d_{}", std::process::id()));
        let mut buffer = vec![0u8; (W * H * 4) as usize];
        slot.hand_over(buffer.as_ptr() as usize, &[[0, 0, W, H]]);
        wait_for_frame(&view, 1);
        let at = ((10 * W + 3) * 4) as usize;
        buffer[at] = 9;
        slot.hand_over(buffer.as_ptr() as usize, &[[0, 8, W, 6]]);
        wait_for_frame(&view, 2);
        assert_eq!(view.pixels()[..], buffer[..]);
        assert_eq!(view.header().changed_since(1, 2), Some(vec![[3, 10, 1, 1]]));
        thread.stop();
    }

    #[test]
    fn a_buffer_that_is_gone_is_survived_and_the_next_paint_is_whole() {
        let (thread, slot, view) = start(&format!("pt_c_{}", std::process::id()));
        let first = vec![7u8; (W * H * 4) as usize];
        slot.hand_over(first.as_ptr() as usize, &[[0, 0, W, H]]);
        wait_for_frame(&view, 1);
        let before = read_failures();
        slot.hand_over(1 << 40, &[[0, 0, W, H]]);
        let until = Instant::now() + Duration::from_secs(5);
        while read_failures() == before {
            assert!(Instant::now() < until, "the bad read was not noticed");
            std::thread::sleep(Duration::from_millis(1));
        }
        let second = vec![9u8; (W * H * 4) as usize];
        slot.hand_over(second.as_ptr() as usize, &[[0, 0, 1, 1]]);
        wait_for_frame(&view, 2);
        assert_eq!(view.pixels()[..], second[..]);
        thread.stop();
    }
}
