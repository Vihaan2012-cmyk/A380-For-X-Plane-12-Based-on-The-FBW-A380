//! Where the plugin's time goes on X-Plane's main thread: the frame's work
//! split at its `[slot ...]` markers, and the screens' draw callbacks,
//! summed and written to the log every ten seconds as milliseconds per frame.

use std::cell::RefCell;
use std::time::{Duration, Instant};

const REPORT_EVERY: Duration = Duration::from_secs(10);

/// Screen pixels copied into X-Plane's textures since the last report.
pub static UPLOADED_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

thread_local! {
    static SCREEN_BYTES: RefCell<Vec<(&'static str, u64)>> = const { RefCell::new(Vec::new()) };
}

/// Pixels one screen copied into its texture, for the report's per-screen split.
pub fn screen_bytes(id: &'static str, bytes: u64) {
    SCREEN_BYTES.with(|s| {
        let mut s = s.borrow_mut();
        match s.iter_mut().find(|(i, _)| *i == id) {
            Some(e) => e.1 += bytes,
            None => s.push((id, bytes)),
        }
    });
}

struct Laps {
    last: Option<(Instant, &'static str)>,
    totals: Vec<(&'static str, Duration)>,
    frames: u32,
    since: Instant,
}

thread_local! {
    static LAPS: RefCell<Laps> = RefCell::new(Laps { last: None, totals: Vec::new(), frames: 0, since: Instant::now() });
}

/// X-Plane's own frame and GPU times over the report, and the processes' CPU time at the last report.
struct SimFrames {
    refs: Option<(Option<crate::xp::DataRef>, Option<crate::xp::DataRef>)>,
    frame_s: f64,
    gpu_s: f64,
    worst_s: f64,
    frames: u32,
    cpu: Option<(Instant, Vec<u64>)>,
}

thread_local! {
    static SIM: RefCell<SimFrames> = const { RefCell::new(SimFrames { refs: None, frame_s: 0., gpu_s: 0., worst_s: 0., frames: 0, cpu: None }) };
}

/// Samples X-Plane's frame and GPU time once per frame, for the report.
pub fn sim_frame(xplm: &crate::xp::Xplm) {
    SIM.with(|s| {
        let mut s = s.borrow_mut();
        let (frame, gpu) = *s.refs.get_or_insert_with(|| (xplm.find("sim/time/framerate_period"), xplm.find("sim/time/gpu_time_per_frame_sec_approx")));
        if let Some(d) = frame {
            let v = xplm.get_f(d) as f64;
            s.frame_s += v;
            s.worst_s = s.worst_s.max(v);
        }
        if let Some(d) = gpu {
            s.gpu_s += xplm.get_f(d) as f64;
        }
        s.frames += 1;
    });
}

#[cfg(windows)]
const CPU_PROCESSES: [&str; 2] = ["X-Plane.exe", "XPHFBW.exe"];

fn sim_report() -> String {
    SIM.with(|s| {
        let mut s = s.borrow_mut();
        let n = s.frames.max(1) as f64;
        #[allow(unused_mut)]
        let mut out = format!(
            "X-Plane {:.1} fps (frame {:.1} ms, GPU {:.1} ms, worst frame {:.1} ms)",
            if s.frame_s > 0. { n / s.frame_s } else { 0. },
            s.frame_s * 1000. / n,
            s.gpu_s * 1000. / n,
            s.worst_s * 1000.
        );
        #[cfg(windows)]
        {
            let now = Instant::now();
            let cpu: Vec<u64> = crate::diag::process_stats(&CPU_PROCESSES).iter().map(|t| t.2).collect();
            if let Some((then, before)) = &s.cpu {
                let wall = now.duration_since(*then).as_secs_f64().max(1e-3);
                let parts: Vec<String> = CPU_PROCESSES
                    .iter()
                    .zip(cpu.iter().zip(before))
                    .map(|(name, (a, b))| format!("{} {:.0}% of a core", name.trim_end_matches(".exe"), a.saturating_sub(*b) as f64 / 1e7 / wall * 100.))
                    .collect();
                out.push_str(&format!(", CPU {}", parts.join(", ")));
            }
            s.cpu = Some((now, cpu));
        }
        s.frame_s = 0.;
        s.gpu_s = 0.;
        s.worst_s = 0.;
        s.frames = 0;
        out
    })
}

fn add(totals: &mut Vec<(&'static str, Duration)>, label: &'static str, d: Duration) {
    match totals.iter_mut().find(|(l, _)| *l == label) {
        Some(entry) => entry.1 += d,
        None => totals.push((label, d)),
    }
}

/// Ends the section before this point and starts `label`'s.
/// The section the frame is in now (the last [`lap`] label), for tracing.
pub fn current_section() -> &'static str {
    LAPS.with(|l| l.borrow().last.map_or("before the first lap", |(_, label)| label))
}

pub fn lap(label: &'static str) {
    LAPS.with(|l| {
        let mut l = l.borrow_mut();
        let now = Instant::now();
        if let Some((start, previous)) = l.last {
            add(&mut l.totals, previous, now - start);
        }
        l.last = Some((now, label));
    });
}

/// Ends the frame's last section; reports now and then.
pub fn end_frame() {
    LAPS.with(|l| {
        let mut l = l.borrow_mut();
        if let Some((start, previous)) = l.last.take() {
            add(&mut l.totals, previous, start.elapsed());
        }
        l.frames += 1;
        if l.since.elapsed() >= REPORT_EVERY {
            let frames = l.frames.max(1) as f64;
            let mut totals = std::mem::take(&mut l.totals);
            totals.sort_by(|a, b| b.1.cmp(&a.1));
            let sum: f64 = totals.iter().map(|(_, d)| d.as_secs_f64()).sum::<f64>() * 1000. / frames;
            let top: Vec<String> =
                totals.iter().take(12).map(|(label, d)| format!("{label} {:.2}", d.as_secs_f64() * 1000. / frames)).collect();
            let fps = frames / l.since.elapsed().as_secs_f64();
            let mb = UPLOADED_BYTES.swap(0, std::sync::atomic::Ordering::Relaxed) as f64 / frames / 1_048_576.;
            let mut screens = SCREEN_BYTES.with(|s| std::mem::take(&mut *s.borrow_mut()));
            screens.sort_by(|a, b| b.1.cmp(&a.1));
            let screens: Vec<String> = screens.iter().take(6).map(|(id, b)| format!("{} {:.1}", id.trim_start_matches("SCREEN_"), *b as f64 / frames / 1_048_576.)).collect();
            crate::log(&format!("perf: {}; {fps:.1} plugin frames/s, {sum:.2} ms per frame in the plugin, {mb:.1} MB of screen pixels uploaded per frame ({}); ms per frame: {}", sim_report(), screens.join(", "), top.join(", ")));
            l.frames = 0;
            l.since = Instant::now();
        }
    });
}

/// Times one call (a draw callback) into the current report.
pub fn time<R>(label: &'static str, f: impl FnOnce() -> R) -> R {
    let start = Instant::now();
    let r = f();
    let d = start.elapsed();
    crate::diag::DRAW_NS.fetch_add(d.as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
    LAPS.with(|l| add(&mut l.borrow_mut().totals, label, d));
    r
}
