//! Native screens as a texture source (plan Task 4): which screens the
//! native displays draw, the render thread that owns the display library's
//! GL context, and the hand-over of finished CPU buffers to
//! `display/mod.rs`'s upload path.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::{Display, Inputs};
use crate::mapdata::terrain::types::Side;
use crate::display::screens::{self, SCREENS};

pub const ENV: &str = "FBW_NATIVE_SCREENS";

/// How long the render thread waits for a sim frame before drawing anyway,
/// so the screens still move while the flight loop is not ticking (paused).
pub const IDLE_WAIT: Duration = Duration::from_millis(33);
const REPORT_EVERY: Duration = Duration::from_secs(10);
const SLOW_SCREEN_PERIOD: Duration = Duration::from_millis(33);
const PASS_BUDGET: Duration = Duration::from_millis(28);
const STALE_AFTER: Duration = Duration::from_millis(150);

/// Render threads, each with its own GPU context (`FBW_NATIVE_THREADS`, default 4).
pub fn render_threads() -> usize {
    std::env::var("FBW_NATIVE_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(4).clamp(1, 16)
}

fn full_rate(screen: &str) -> bool {
    matches!(screen, "SCREEN_DU_PFDL" | "SCREEN_DU_PFDR" | "SCREEN_DU_NDL" | "SCREEN_DU_NDR")
}

/// The sim frame counter the main thread bumps once a frame and the render
/// thread waits on: one render pass per sim frame.
#[derive(Default)]
pub struct Tick {
    frame: Mutex<u64>,
    signal: Condvar,
}

impl Tick {
    pub fn bump(&self) {
        *self.frame.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        self.signal.notify_all();
    }

    /// The frame number once it differs from `seen`, or after `timeout`.
    pub fn wait_past(&self, seen: u64, timeout: Duration) -> u64 {
        let guard = self.frame.lock().unwrap_or_else(|e| e.into_inner());
        let (guard, _) = self.signal.wait_timeout_while(guard, timeout, |f| *f == seen).unwrap_or_else(|e| e.into_inner());
        *guard
    }
}

const TILE_W: usize = 64;
const TILE_H: usize = 32;

/// A display drawn over a screen's base display, with inputs of its own
/// under the feed key `key`.
#[derive(Debug, PartialEq, Eq)]
pub struct Over {
    pub key: &'static str,
    pub display: &'static str,
    pub variant: &'static str,
}

/// A screen and the native display that draws it. Ids and variants are
/// a380-displays' (`a380disp_render --list`, the golden view names). `over`
/// are drawn on top of the base display, in order, on the same surface.
#[derive(Debug, PartialEq, Eq)]
pub struct Target {
    pub screen: &'static str,
    pub display: &'static str,
    pub variant: &'static str,
    pub over: &'static [Over],
}

fn read_set_name(display: &str, variant: &str) -> String {
    match (display, variant) {
        ("mfdx", _) => "mfd".into(),
        ("oitx", v) => format!("oit-{v}"),
        (d @ ("nd" | "pfd" | "rmp" | "oit"), v) => format!("{d}-{v}"),
        (d, _) => d.into(),
    }
}

impl Target {
    /// The golden view name `inputs::read_set` knows this display by.
    pub fn read_set(&self) -> String {
        read_set_name(self.display, self.variant)
    }

    /// Every layer as (feed key, display, variant, read set), base first.
    /// The base layer's feed key is the screen id.
    pub fn layers(&self) -> Vec<(&'static str, &'static str, &'static str, String)> {
        let mut layers = vec![(self.screen, self.display, self.variant, self.read_set())];
        layers.extend(self.over.iter().map(|o| (o.key, o.display, o.variant, read_set_name(o.display, o.variant))));
        layers
    }
}

const fn t(screen: &'static str, display: &'static str, variant: &'static str) -> Target {
    Target { screen, display, variant, over: &[] }
}

const SD_OVER: &[Over] = &[Over { key: "SCREEN_DU_SD+sdv2", display: "sdv2", variant: "" }];

pub static TARGETS: &[Target] = &[
    t("SCREEN_DU_PFDL", "pfd", "1"),
    t("SCREEN_DU_NDL", "nd", "1"),
    t("SCREEN_DU_PFDR", "pfd", "2"),
    t("SCREEN_DU_NDR", "nd", "2"),
    t("SCREEN_DU_EWD", "ewd", ""),
    Target { screen: "SCREEN_DU_SD", display: "sd", variant: "", over: SD_OVER },
    t("SCREEN_DU_MFD", "mfdx", ""),
    t("FCU", "fcu", ""),
    t("SCREEN_ISIS_1", "isislegacy", ""),
    t("Clock", "clock", ""),
    t("RTPI", "rtpi", ""),
    t("BAT", "bat", ""),
    t("SCREEN_DU_RMP_1", "rmp", "1"),
    t("SCREEN_DU_RMP_2", "rmp", "2"),
    t("SCREEN_DU_RMP_3", "rmp", "3"),
    t("SCREEN_OIT_LEFT", "oitx", "1"),
    t("SCREEN_OIT_RIGHT", "oitx", "2"),
    t("SCREEN_ISIS_2", "snd", ""),
    t("SCREEN_IFE", "ife", ""),
    t("SCREEN_EFB", "efbx", ""),
];

const NAMES: &[(&str, &[&str])] = &[
    ("pfd", &["SCREEN_DU_PFDL", "SCREEN_DU_PFDR"]),
    ("pfdl", &["SCREEN_DU_PFDL"]),
    ("pfdr", &["SCREEN_DU_PFDR"]),
    ("nd", &["SCREEN_DU_NDL", "SCREEN_DU_NDR"]),
    ("ndl", &["SCREEN_DU_NDL"]),
    ("ndr", &["SCREEN_DU_NDR"]),
    ("ewd", &["SCREEN_DU_EWD"]),
    ("efb", &["SCREEN_EFB"]),
    ("sd", &["SCREEN_DU_SD"]),
    ("mfd", &["SCREEN_DU_MFD"]),
    ("fcu", &["FCU"]),
    ("isis", &["SCREEN_ISIS_1"]),
    ("isis1", &["SCREEN_ISIS_1"]),
    ("snd", &["SCREEN_ISIS_2"]),
    ("isis2", &["SCREEN_ISIS_2"]),
    ("clock", &["Clock"]),
    ("rtpi", &["RTPI"]),
    ("bat", &["BAT"]),
    ("rmp", &["SCREEN_DU_RMP_1", "SCREEN_DU_RMP_2", "SCREEN_DU_RMP_3"]),
    ("rmp1", &["SCREEN_DU_RMP_1"]),
    ("rmp2", &["SCREEN_DU_RMP_2"]),
    ("rmp3", &["SCREEN_DU_RMP_3"]),
    ("oit", &["SCREEN_OIT_LEFT", "SCREEN_OIT_RIGHT"]),
    ("oitl", &["SCREEN_OIT_LEFT"]),
    ("oitr", &["SCREEN_OIT_RIGHT"]),
    ("ife", &["SCREEN_IFE"]),
];

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub targets: Vec<&'static Target>,
    pub unknown: Vec<String>,
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    pub fn target_for(&self, screen_id: &str) -> Option<&'static Target> {
        let key = crate::display::text::screen_key(screen_id);
        self.targets.iter().copied().find(|t| t.screen.eq_ignore_ascii_case(&key))
    }
}

/// `FBW_NATIVE_SCREENS` value: names separated by commas, semicolons or
/// spaces, case-insensitive. A name is a group (`nd`, `pfd`, `rmp`, `oit`),
/// one screen (`ndl`, `rmp2`), a screen id (`SCREEN_DU_NDL`), or `all`.
/// `off`, `none` and `0` select nothing. Unknown names are reported back.
pub fn parse(spec: &str) -> Selection {
    let mut picked = vec![false; TARGETS.len()];
    let mut unknown = Vec::new();
    for word in spec.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).filter(|w| !w.is_empty()) {
        let key = word.trim_start_matches('$').to_ascii_lowercase();
        match key.as_str() {
            "off" | "none" | "0" | "false" | "no" => continue,
            "all" => {
                picked.iter_mut().for_each(|p| *p = true);
                continue;
            }
            _ => {}
        }
        let screens: Vec<&str> = match NAMES.iter().find(|(name, _)| *name == key) {
            Some((_, ids)) => ids.to_vec(),
            None => TARGETS.iter().filter(|t| t.screen.eq_ignore_ascii_case(&key)).map(|t| t.screen).collect(),
        };
        if screens.is_empty() {
            unknown.push(word.to_string());
        }
        for id in screens {
            if let Some(i) = TARGETS.iter().position(|t| t.screen == id) {
                picked[i] = true;
            }
        }
    }
    Selection { targets: TARGETS.iter().zip(picked).filter(|(_, p)| *p).map(|(t, _)| t).collect(), unknown }
}

/// What an unset `FBW_NATIVE_SCREENS` and `nativeScreens` select.
pub const DEFAULT_SPEC: &str = "all";

/// `FBW_NATIVE_SCREENS` when set, else the `nativeScreens` setting, else
/// [`DEFAULT_SPEC`]. Blank counts as unset; `off` is the explicit opt-out.
pub fn resolve_spec(env: Option<String>, setting: impl FnOnce() -> String) -> String {
    env.filter(|v| !v.trim().is_empty())
        .or_else(|| Some(setting()).filter(|v| !v.trim().is_empty()))
        .unwrap_or_else(|| DEFAULT_SPEC.to_string())
}

pub fn selection() -> &'static Selection {
    static SELECTION: OnceLock<Selection> = OnceLock::new();
    SELECTION.get_or_init(|| parse(&resolve_spec(std::env::var(ENV).ok(), crate::app_settings::native_screens_raw)))
}

/// The screen name the whole-library failures are logged under.
pub const ALL_SCREENS: &str = "all screens";

pub fn failure_line(screen: &str, reason: &str) -> String {
    format!("native displays: {screen} failed: {reason}; showing black")
}

/// Why a native screen shows black.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FailureKind {
    Library,
    Create,
    Inputs,
    Draw,
    Panic,
}

/// Which failures have been logged: each screen's each kind once.
#[derive(Default)]
pub struct Failures {
    logged: std::collections::HashSet<(String, FailureKind)>,
}

impl Failures {
    /// The log line for this failure, or `None` when this screen already
    /// logged one of this kind.
    pub fn report(&mut self, screen: &str, kind: FailureKind, reason: &str) -> Option<String> {
        self.logged.insert((screen.to_string(), kind)).then(|| failure_line(screen, reason))
    }

    fn log(&mut self, screen: &str, kind: FailureKind, reason: &str) {
        if let Some(line) = self.report(screen, kind, reason) {
            crate::log(&line);
            crate::app_settings::native_mode::failure_notice(screen, reason);
        }
    }
}

/// An opaque black picture, `width` x `height`, in RGBA (and BGRA).
pub fn black_frame(width: u32, height: u32) -> Vec<u8> {
    [0, 0, 0, 255].repeat(width as usize * height as usize)
}

fn publish_black(slot: &Mutex<Slot>, width: u32, height: u32) {
    publish(slot, &mut black_frame(width, height), width, height);
}

fn black_out(slots: &[Mutex<Slot>], sizes: impl IntoIterator<Item = (u32, u32)>) {
    for (slot, (w, h)) in slots.iter().zip(sizes) {
        publish_black(slot, w, h);
    }
}

/// Where a native display's inputs come from. The render thread calls it;
/// `revision` changes whenever something the screen reads changed, and
/// `fill` then pushes the changes into the screen's persistent inputs.
pub trait Feed: Send + 'static {
    /// A screen's size in device pixels and its device pixel ratio, before
    /// its first `revision`.
    fn begin(&mut self, _screen: &str, _width_px: u32, _height_px: u32, _dpr: f64) {}
    fn revision(&mut self, screen: &str) -> u64;
    fn fill(&mut self, screen: &str, inputs: &mut Inputs);
    /// Whether the screen's variables have been resolved, so its first draw
    /// does not latch a power state from zeros.
    fn ready(&mut self, _screen: &str) -> bool {
        true
    }
}

pub struct EmptyFeed;

impl Feed for EmptyFeed {
    fn revision(&mut self, _screen: &str) -> u64 {
        0
    }
    fn fill(&mut self, _screen: &str, _inputs: &mut Inputs) {}
}

#[derive(Default)]
struct Slot {
    seq: u64,
    width: u32,
    height: u32,
    bgra: Vec<u8>,
    /// Rectangles changed since the consumer last finished an upload; `None`
    /// is the whole picture.
    dirty: Option<Vec<[u32; 4]>>,
    /// The newest frame as a shared GL texture and the GLsync to wait on
    /// before reading it (0 once taken), instead of `bgra`.
    gpu: Option<(u32, usize)>,
    /// The GLsync after the consumer's last copy, which the render thread
    /// waits on before drawing into a texture again.
    release: usize,
}

/// Which full-rate screens a render pass is still drawing, so the screen
/// callback can wait for this frame's picture instead of showing the last one.
#[derive(Default)]
struct Pace {
    due: Vec<AtomicBool>,
    published: Vec<Condvar>,
}

impl Pace {
    fn new(n: usize) -> Self {
        Self { due: (0..n).map(|_| AtomicBool::new(false)).collect(), published: (0..n).map(|_| Condvar::new()).collect() }
    }

    fn done(&self, slots: &[Mutex<Slot>], i: usize) {
        if self.due[i].load(Ordering::Relaxed) {
            let _g = slots[i].lock().unwrap_or_else(|e| e.into_inner());
            self.due[i].store(false, Ordering::Relaxed);
        }
        self.published[i].notify_all();
    }
}

pub struct Native {
    pace: Arc<Pace>,
    slots: Arc<Vec<Mutex<Slot>>>,
    screens: Vec<&'static str>,
    stop: Arc<AtomicBool>,
    frames: Arc<Vec<AtomicU64>>,
    tick: Arc<Tick>,
    threads: Vec<JoinHandle<()>>,
}

/// A finished frame the consumer is about to upload. The slot stays locked
/// while this lives, so the render thread waits to publish its next one.
pub struct Pending<'a> {
    slot: MutexGuard<'a, Slot>,
}

impl Pending<'_> {
    pub fn seq(&self) -> u64 {
        self.slot.seq
    }

    pub fn size(&self) -> (u32, u32) {
        (self.slot.width, self.slot.height)
    }

    pub fn bgra(&self) -> &[u8] {
        &self.slot.bgra
    }

    /// The rectangles to upload, through the same planner XPHFBW's dirty
    /// rectangles go through.
    pub fn rects(&self) -> Vec<[u32; 4]> {
        crate::display::plan_upload(self.slot.dirty.clone(), self.slot.width, self.slot.height)
    }

    /// The consumer has uploaded everything this frame holds.
    pub fn finish(mut self) {
        self.slot.dirty = Some(Vec::new());
    }

    /// The frame's shared GL texture and its ready GLsync, which the caller
    /// now owns.
    pub fn gpu(&mut self) -> Option<(u32, usize)> {
        let (texture, ready) = self.slot.gpu?;
        self.slot.gpu = Some((texture, 0));
        Some((texture, ready))
    }

    /// Hands the render thread the GLsync after the copy; returns the one it
    /// replaces, which the caller deletes.
    pub fn released(mut self, fence: usize) -> usize {
        self.slot.dirty = Some(Vec::new());
        std::mem::replace(&mut self.slot.release, fence)
    }
}

impl Native {
    /// Starts the render thread for `selection` with `feed`. The sizes are
    /// each screen's device pixels (`xphfbw_bridge_views::device_size`).
    /// The selected screens' slots with no render thread: nothing ever lands.
    pub fn idle(selection: &Selection) -> Option<Self> {
        let screens: Vec<&'static str> = selection.targets.iter().map(|t| t.screen).collect();
        (!screens.is_empty()).then(|| Self {
            slots: Arc::new(screens.iter().map(|_| Mutex::new(Slot::default())).collect()),
            frames: Arc::new(screens.iter().map(|_| AtomicU64::new(0)).collect()),
            pace: Arc::new(Pace::new(screens.len())),
            screens,
            stop: Arc::new(AtomicBool::new(false)),
            tick: Arc::new(Tick::default()),
            threads: Vec::new(),
        })
    }

    pub fn start(selection: &Selection, feed: impl Feed) -> Option<Self> {
        Self::start_with(selection, vec![Box::new(feed)])
    }

    /// One render thread per feed, each with its own GPU context; screens are
    /// spread across them, the full-rate ones first so they land on separate threads.
    pub fn start_with(selection: &Selection, feeds: Vec<Box<dyn Feed>>) -> Option<Self> {
        if selection.is_empty() || feeds.is_empty() {
            return None;
        }
        let specs: Vec<Spec> = selection
            .targets
            .iter()
            .filter_map(|target| {
                let def = &SCREENS[screens::find(target.screen)?];
                let (width, height) = crate::xphfbw_bridge_views::device_size(def.id, def.width, def.height);
                Some(Spec { target, width, height, dpr: width as f64 / def.width as f64 })
            })
            .collect();
        let slots: Arc<Vec<Mutex<Slot>>> = Arc::new(specs.iter().map(|_| Mutex::new(Slot { dirty: None, ..Slot::default() })).collect());
        let frames: Arc<Vec<AtomicU64>> = Arc::new(specs.iter().map(|_| AtomicU64::new(0)).collect());
        let stop = Arc::new(AtomicBool::new(false));
        let screens: Vec<&'static str> = specs.iter().map(|s| s.target.screen).collect();
        let sizes: Vec<(u32, u32)> = specs.iter().map(|s| (s.width, s.height)).collect();
        let tick = Arc::new(Tick::default());
        let pace = Arc::new(Pace::new(specs.len()));
        let n = feeds.len().min(specs.len()).max(1);
        let mut order: Vec<(usize, Spec)> = specs.into_iter().enumerate().collect();
        order.sort_by_key(|(_, s)| !full_rate(s.target.screen));
        let mut groups: Vec<Vec<(usize, Spec)>> = (0..n).map(|_| Vec::new()).collect();
        for (k, item) in order.into_iter().enumerate() {
            groups[k % n].push(item);
        }
        let mut threads = Vec::new();
        for (g, (group, feed)) in groups.into_iter().zip(feeds).enumerate() {
            if group.is_empty() {
                continue;
            }
            let (slots, frames, stop, tick, pace) = (slots.clone(), frames.clone(), stop.clone(), tick.clone(), pace.clone());
            match std::thread::Builder::new().name(format!("fbw-native-displays-{g}")).spawn(move || render_thread(group, feed, slots, frames, stop, tick, pace)) {
                Ok(t) => threads.push(t),
                Err(e) => crate::log(&failure_line(ALL_SCREENS, &format!("no render thread ({e})"))),
            }
        }
        if threads.is_empty() {
            black_out(&slots, sizes);
        }
        Some(Self { pace, slots, screens, stop, frames, tick, threads })
    }

    /// One sim frame has passed: the render thread draws what changed.
    pub fn frame(&self) {
        self.tick.bump();
    }

    pub fn slot_of(&self, screen_id: &str) -> Option<usize> {
        let key = crate::display::text::screen_key(screen_id);
        self.screens.iter().position(|s| s.eq_ignore_ascii_case(&key))
    }

    /// The frame number last published into `slot`; 0 before the first.
    pub fn seq(&self, slot: usize) -> u64 {
        self.slots.get(slot).map_or(0, |s| s.lock().unwrap_or_else(|e| e.into_inner()).seq)
    }

    /// Waits, up to `max`, while a render pass is still drawing `slot`'s
    /// picture for this frame.
    pub fn wait_due(&self, slot: usize, max: Duration) {
        let Some(due) = self.pace.due.get(slot) else { return };
        if !due.load(Ordering::Relaxed) {
            return;
        }
        let guard = self.slots[slot].lock().unwrap_or_else(|e| e.into_inner());
        let _ = self.pace.published[slot].wait_timeout_while(guard, max, |_| due.load(Ordering::Relaxed));
    }

    /// The slot's frame when it is newer than `uploaded`.
    pub fn pending(&self, slot: usize, uploaded: u64) -> Option<Pending<'_>> {
        let guard = self.slots.get(slot)?.lock().unwrap_or_else(|e| e.into_inner());
        (guard.seq != uploaded && guard.seq != 0 && (!guard.bgra.is_empty() || guard.gpu.is_some())).then_some(Pending { slot: guard })
    }

    pub fn frames_drawn(&self, slot: usize) -> u64 {
        self.frames.get(slot).map_or(0, |f| f.load(Ordering::Relaxed))
    }
}

impl Drop for Native {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.tick.bump();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

struct Spec {
    target: &'static Target,
    width: u32,
    height: u32,
    dpr: f64,
}

struct LiveLayer {
    key: &'static str,
    display: Display,
    inputs: Inputs,
}

struct Live {
    spec: Spec,
    layers: Vec<LiveLayer>,
    side: Option<Side>,
    underlay: Option<(bool, u64)>,
    scratch: Vec<u8>,
    shared: bool,
    last_rev: Option<u64>,
    draw_time: Duration,
    drawn: u64,
    black: bool,
    last_at: Option<Instant>,
}

/// The ND whose terrain or weather picture lies under a screen.
pub fn underlay_side(screen: &str) -> Option<Side> {
    match screen {
        "SCREEN_DU_NDL" => Some(Side::Left),
        "SCREEN_DU_NDR" => Some(Side::Right),
        _ => None,
    }
}

type Underlay = (bool, u32, u32, u64, Arc<[u8]>);

/// The picture XPHFBW's compositor puts under nd.html: the weather radar's
/// when it is showing, else the terrain gauge's. `(is_wxr, w, h, generation, rgba)`.
fn underlay_picture(side: Side) -> Option<Underlay> {
    match crate::wxr::layer(side) {
        Some((w, h, generation, rgba)) => Some((true, w, h, generation, rgba)),
        None => crate::mapdata::plugin::terrain_layer(side).map(|(w, h, generation, rgba)| (false, w, h, generation, rgba)),
    }
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    let text = payload.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| payload.downcast_ref::<String>().cloned());
    format!("panic: {}", text.unwrap_or_else(|| "unknown".into()))
}

fn open_screen(spec: Spec, feed: &mut dyn Feed) -> Result<Live, (FailureKind, String)> {
    let mut layers = Vec::new();
    for (key, id, variant, _) in spec.target.layers() {
        let Some(display) = Display::new(id, variant) else {
            return Err((FailureKind::Create, format!("no display '{id}' ({variant}): {}", super::last_error())));
        };
        let Some(mut inputs) = Inputs::try_new() else {
            return Err((FailureKind::Inputs, format!("no inputs for '{id}' ({variant}): {}", super::last_error())));
        };
        feed.begin(key, spec.width, spec.height, spec.dpr);
        inputs.set_dpr(spec.dpr);
        layers.push(LiveLayer { key, display, inputs });
    }
    for l in &layers {
        super::input::register(spec.target.screen, &l.display);
    }
    let scratch = vec![0; spec.width as usize * spec.height as usize * 4];
    let side = underlay_side(spec.target.screen);
    Ok(Live { spec, layers, side, underlay: None, scratch, last_rev: None, draw_time: Duration::ZERO, drawn: 0, black: false, last_at: None, shared: false })
}

/// Whether the screens are drawn in X-Plane's screen callbacks on its own GL
/// context, as Laminar's are, instead of on render threads
/// (`FBW_NATIVE_INLINE=1` or `"nativeInline": true` in the app settings).
pub fn inline_mode() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| match std::env::var("FBW_NATIVE_INLINE") {
        Ok(v) => v.trim() == "1",
        Err(_) => crate::app_settings::native_inline(),
    })
}

/// The screens drawn on the main thread, each into its own texture during
/// its screen callback.
pub struct Inline {
    specs: Vec<Option<Spec>>,
    live: Vec<Option<Live>>,
    feed: Box<dyn Feed>,
    failures: Failures,
    reported: Instant,
}

impl Inline {
    pub fn new(selection: &Selection, feed: Box<dyn Feed>) -> Option<Self> {
        if selection.is_empty() || super::load().is_err() {
            return None;
        }
        let specs: Vec<Option<Spec>> = selection
            .targets
            .iter()
            .map(|target| {
                let def = &SCREENS[screens::find(target.screen)?];
                let (width, height) = crate::xphfbw_bridge_views::device_size(def.id, def.width, def.height);
                Some(Spec { target, width, height, dpr: width as f64 / def.width as f64 })
            })
            .collect();
        let live = specs.iter().map(|_| None).collect();
        crate::log("native displays: drawn in X-Plane's screen callbacks (inline)");
        Some(Self { specs, live, feed, failures: Failures::default(), reported: Instant::now() })
    }

    pub fn size(&self, slot: usize) -> (u32, u32) {
        self.specs.get(slot).and_then(|s| s.as_ref()).map_or((0, 0), |s| (s.width, s.height))
    }

    /// Draws `slot` when its inputs changed; `Ok(Some(texture))` when it drew.
    pub fn draw(&mut self, slot: usize) -> Result<Option<u32>, String> {
        if self.live.get(slot).is_some_and(Option::is_none) {
            let Some(spec) = self.specs[slot].take() else { return Err("no such screen".into()) };
            let id = spec.target.screen;
            match open_screen(spec, self.feed.as_mut()) {
                Ok(live) => self.live[slot] = Some(live),
                Err((kind, reason)) => {
                    self.failures.log(id, kind, &reason);
                    return Err(reason);
                }
            }
        }
        let Some(s) = self.live.get_mut(slot).and_then(Option::as_mut) else { return Err("not open".into()) };
        let feed = self.feed.as_mut();
        let id = s.spec.target.screen;
        let revision = s.layers.iter().fold(0u64, |sum, l| sum.wrapping_add(feed.revision(l.key)));
        if s.last_rev.is_none() && !s.layers.iter().fold(true, |ok, l| feed.ready(l.key) && ok) {
            return Ok(None);
        }
        let underlay = s.side.and_then(underlay_picture);
        let underlay_key = underlay.as_ref().map(|u| (u.0, u.3));
        let underlay_changed = underlay_key != s.underlay;
        if s.last_rev == Some(revision) && !underlay_changed && !s.black {
            return Ok(None);
        }
        if !full_rate(id) && s.last_at.is_some_and(|t| t.elapsed() < SLOW_SCREEN_PERIOD) {
            return Ok(None);
        }
        s.last_at = Some(Instant::now());
        if underlay_changed {
            let picture = underlay.as_ref().map(|u| (&u.4[..], u.1, u.2));
            if !s.layers[0].display.set_underlay(picture) {
                crate::log(&format!("native displays: {id} underlay refused: {}", super::last_error()));
            }
            s.underlay = underlay_key;
        }
        for l in s.layers.iter_mut() {
            feed.fill(l.key, &mut l.inputs);
        }
        let started = Instant::now();
        let mut refs: Vec<(&mut Display, &Inputs)> = s.layers.iter_mut().map(|l| (&mut l.display, &l.inputs)).collect();
        let drawn = super::draw_layers_host(&mut refs, s.spec.width, s.spec.height);
        s.draw_time += started.elapsed();
        s.last_rev = Some(revision);
        for line in super::host_diag().lines() {
            crate::log(&format!("native displays: {id} {line}"));
        }
        let texture = match drawn {
            Ok(texture) => {
                set_boxes(s);
                s.drawn += 1;
                s.black = false;
                texture
            }
            Err(e) => {
                self.failures.log(id, FailureKind::Draw, &format!("draw refused: {e:?}"));
                s.black = true;
                return Err(format!("{e:?}"));
            }
        };
        if self.reported.elapsed() >= REPORT_EVERY {
            for s in self.live.iter_mut().flatten() {
                if s.drawn > 0 {
                    crate::log(&format!("native displays: {} {} frames, {:.1} ms a frame", s.spec.target.screen, s.drawn, s.draw_time.as_secs_f64() * 1000. / s.drawn as f64));
                }
                s.drawn = 0;
                s.draw_time = Duration::ZERO;
            }
            self.reported = Instant::now();
        }
        Ok(Some(texture))
    }
}

/// Whether the screen was drawn (or a draw was attempted).
fn step(s: &mut Live, feed: &mut dyn Feed, slot: &Mutex<Slot>, frames: &AtomicU64, failures: &mut Failures) -> bool {
    let id = s.spec.target.screen;
    let revision = s.layers.iter().fold(0u64, |sum, l| sum.wrapping_add(feed.revision(l.key)));
    if s.last_rev.is_none() && !s.layers.iter().fold(true, |ok, l| feed.ready(l.key) && ok) {
        return false;
    }
    let underlay = s.side.and_then(underlay_picture);
    let underlay_key = underlay.as_ref().map(|u| (u.0, u.3));
    let underlay_changed = underlay_key != s.underlay;
    if s.last_rev == Some(revision) && !underlay_changed {
        return false;
    }
    if !full_rate(id) && s.last_at.is_some_and(|t| t.elapsed() < SLOW_SCREEN_PERIOD) {
        return false;
    }
    s.last_at = Some(Instant::now());
    if underlay_changed {
        let picture = underlay.as_ref().map(|u| (&u.4[..], u.1, u.2));
        if !s.layers[0].display.set_underlay(picture) {
            crate::log(&format!("native displays: {id} underlay refused: {}", super::last_error()));
        }
        s.underlay = underlay_key;
    }
    for l in s.layers.iter_mut() {
        feed.fill(l.key, &mut l.inputs);
    }
    let (w, h) = (s.spec.width, s.spec.height);
    let started = Instant::now();
    if s.shared {
        let release = std::mem::take(&mut slot.lock().unwrap_or_else(|e| e.into_inner()).release);
        super::gpu_wait(release);
        let mut refs: Vec<(&mut Display, &Inputs)> = s.layers.iter_mut().map(|l| (&mut l.display, &l.inputs)).collect();
        let drawn = super::draw_layers_gpu(&mut refs, w, h);
        s.draw_time += started.elapsed();
        s.last_rev = Some(revision);
        match drawn {
            Ok((texture, ready)) => {
                set_boxes(s);
                s.drawn += 1;
                frames.fetch_add(1, Ordering::Relaxed);
                let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
                if let Some((_, old)) = slot.gpu.replace((texture, ready)) {
                    super::gpu_wait(old);
                }
                slot.bgra = Vec::new();
                slot.width = w;
                slot.height = h;
                slot.seq += 1;
                s.black = false;
            }
            Err(e) => {
                failures.log(id, FailureKind::Draw, &format!("shared GPU draw refused, reading pixels back instead: {e:?}"));
                s.shared = false;
                slot.lock().unwrap_or_else(|e| e.into_inner()).gpu = None;
                s.last_rev = None;
            }
        }
        return true;
    }
    let changed = match s.layers.as_mut_slice() {
        [only] => only.display.draw(&only.inputs, &mut s.scratch, w, h, w as usize * 4),
        many => {
            let mut refs: Vec<(&mut Display, &Inputs)> = many.iter_mut().map(|l| (&mut l.display, &l.inputs)).collect();
            super::draw_layers(&mut refs, &mut s.scratch, w, h, w as usize * 4)
        }
    };
    s.draw_time += started.elapsed();
    s.last_rev = Some(revision);
    match changed {
        Ok(changed) => {
            set_boxes(s);
            s.drawn += 1;
            frames.fetch_add(1, Ordering::Relaxed);
            if changed || s.drawn == 1 || s.black {
                publish(slot, &mut s.scratch, w, h);
            }
            s.black = false;
        }
        Err(e) => {
            failures.log(id, FailureKind::Draw, &format!("draw refused: {e:?}"));
            publish_black(slot, w, h);
            s.black = true;
        }
    }
    true
}

fn set_boxes(s: &Live) {
    if s.spec.target.display == "efbx" {
        crate::efb_native::host::set_boxes(&s.layers[0].display.boxes());
    } else if s.spec.target.display == "mfdx" {
        crate::mfd_native_host::set_boxes(&s.layers[0].display.boxes());
    } else if s.spec.target.display == "oitx" {
        crate::oit_native_host::set_boxes(s.spec.target.variant, &s.layers[0].display.boxes());
    }
}

fn render_thread(specs: Vec<(usize, Spec)>, mut feed: Box<dyn Feed>, slots: Arc<Vec<Mutex<Slot>>>, frames: Arc<Vec<AtomicU64>>, stop: Arc<AtomicBool>, tick: Arc<Tick>, pace: Arc<Pace>) {
    let mut failures = Failures::default();
    let init = match super::load() {
        Err(_) => Err(None),
        Ok(()) => super::gpu_init().map_err(Some),
    };
    match init {
        Ok(kind) => crate::log(&format!(
            "native displays: render thread up, {}",
            if kind != super::GpuInit::Gpu {
                "CPU rasteriser only"
            } else if super::gpu_shared() {
                "GPU context sharing textures with X-Plane"
            } else {
                "GPU context, pixels read back"
            }
        )),
        Err(reason) => {
            if let Some(reason) = reason {
                failures.log(ALL_SCREENS, FailureKind::Library, &reason);
            }
            for (i, s) in &specs {
                publish_black(&slots[*i], s.width, s.height);
            }
            return;
        }
    }
    let shared = super::gpu_shared();
    let globals: Vec<usize> = specs.iter().map(|(i, _)| *i).collect();
    let mut live: Vec<Option<Live>> = specs
        .into_iter()
        .map(|(index, spec)| {
            let (id, w, h) = (spec.target.screen, spec.width, spec.height);
            let opened = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| open_screen(spec, feed.as_mut()))).unwrap_or_else(|p| Err((FailureKind::Panic, panic_text(&*p))));
            match opened {
                Ok(mut live) => {
                    live.shared = shared;
                    Some(live)
                }
                Err((kind, reason)) => {
                    failures.log(id, kind, &reason);
                    publish_black(&slots[index], w, h);
                    None
                }
            }
        })
        .collect();
    let mut reported = Instant::now();
    let mut seen = 0;
    let mut rotate = 0usize;
    while !stop.load(Ordering::Relaxed) {
        seen = tick.wait_past(seen, IDLE_WAIT);
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let started = Instant::now();
        let count = live.len();
        for (i, s) in live.iter().enumerate() {
            if s.as_ref().is_some_and(|s| full_rate(s.spec.target.screen)) {
                pace.due[globals[i]].store(true, Ordering::Relaxed);
            }
        }
        let order: Vec<usize> = (0..count)
            .filter(|&i| live[i].as_ref().is_some_and(|s| full_rate(s.spec.target.screen)))
            .chain((0..count).map(|k| (rotate + k) % count).filter(|&i| live[i].as_ref().is_some_and(|s| !full_rate(s.spec.target.screen))))
            .collect();
        let (mut slow_drew, mut forced) = (false, false);
        for index in order {
            let slow = live[index].as_ref().is_some_and(|s| !full_rate(s.spec.target.screen));
            let mut force = false;
            if slow && slow_drew && started.elapsed() >= PASS_BUDGET {
                let stale = live[index].as_ref().is_some_and(|s| s.last_at.map_or(true, |t| t.elapsed() > STALE_AFTER));
                if forced || !stale {
                    continue;
                }
                force = true;
            }
            if slow && !force {
                rotate = (index + 1) % count;
            }
            let screen = &mut live[index];
            let Some(s) = screen else { continue };
            let (id, w, h) = (s.spec.target.screen, s.spec.width, s.spec.height);
            let gi = globals[index];
            let stepped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| step(s, feed.as_mut(), &slots[gi], &frames[gi], &mut failures)));
            match stepped {
                Err(p) => {
                    failures.log(id, FailureKind::Panic, &panic_text(&*p));
                    super::input::unregister(id);
                    *screen = None;
                    publish_black(&slots[gi], w, h);
                }
                Ok(true) if slow => {
                    slow_drew = true;
                    forced |= force;
                }
                Ok(_) => {}
            }
            pace.done(&slots, gi);
        }
        super::cost::render(started.elapsed());
        if reported.elapsed() >= REPORT_EVERY {
            for s in live.iter_mut().flatten() {
                if s.drawn > 0 {
                    crate::log(&format!(
                        "native displays: {} {} frames, {:.1} ms a frame",
                        s.spec.target.screen,
                        s.drawn,
                        s.draw_time.as_secs_f64() * 1000. / s.drawn as f64
                    ));
                }
                s.drawn = 0;
                s.draw_time = Duration::ZERO;
            }
            for line in super::host_diag().lines() {
                crate::log(&format!("native displays: profile {line}"));
            }
            reported = Instant::now();
        }
    }
    super::input::unregister_all();
    live.clear();
}

/// The display library reads back RGBA; the upload path takes the BGRA
/// XPHFBW's Chromium frames are in.
pub fn rgba_to_bgra(pixels: &mut [u8]) {
    pixels.chunks_exact_mut(4).for_each(|p| p.swap(0, 2));
}

fn publish(slot: &Mutex<Slot>, scratch: &mut Vec<u8>, width: u32, height: u32) {
    rgba_to_bgra(scratch);
    let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
    let same_shape = slot.width == width && slot.height == height && slot.bgra.len() == scratch.len();
    if same_shape && slot.seq != 0 {
        let rects = diff_rects(&slot.bgra, scratch, width as usize, height as usize);
        if rects.is_empty() {
            return;
        }
        if let Some(dirty) = slot.dirty.as_mut() {
            dirty.extend(rects);
        }
    } else {
        slot.dirty = None;
    }
    slot.gpu = None;
    std::mem::swap(&mut slot.bgra, scratch);
    slot.width = width;
    slot.height = height;
    slot.seq += 1;
    if scratch.len() != slot.bgra.len() {
        *scratch = vec![0; slot.bgra.len()];
    }
}

/// Rectangles (x, y, w, h) covering every pixel that differs between two
/// equally sized BGRA pictures, as runs of changed 64 x 32 tiles per band.
pub fn diff_rects(old: &[u8], new: &[u8], width: usize, height: usize) -> Vec<[u32; 4]> {
    let stride = width * 4;
    let mut rects = Vec::new();
    if old.len() != new.len() || new.len() < stride * height {
        return vec![[0, 0, width as u32, height as u32]];
    }
    let mut y = 0;
    while y < height {
        let rows = TILE_H.min(height - y);
        let mut run: Option<usize> = None;
        let mut x = 0;
        while x < width {
            let cols = TILE_W.min(width - x);
            let changed = (y..y + rows).any(|row| {
                let at = row * stride + x * 4;
                old[at..at + cols * 4] != new[at..at + cols * 4]
            });
            match (changed, run) {
                (true, None) => run = Some(x),
                (false, Some(start)) => {
                    rects.push([start as u32, y as u32, (x - start) as u32, rows as u32]);
                    run = None;
                }
                _ => {}
            }
            x += cols;
        }
        if let Some(start) = run {
            rects.push([start as u32, y as u32, (width - start) as u32, rows as u32]);
        }
        y += rows;
    }
    rects
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screens_of(spec: &str) -> Vec<&'static str> {
        parse(spec).targets.iter().map(|t| t.screen).collect()
    }

    #[test]
    fn off_selects_nothing() {
        assert!(parse("").is_empty());
        assert!(parse("  ").is_empty());
        assert!(parse("off").is_empty());
        assert!(parse("none, 0").is_empty());
        assert!(parse("off").unknown.is_empty());
    }

    #[test]
    fn unset_env_and_setting_select_every_screen() {
        assert_eq!(resolve_spec(None, String::new), "all");
        assert_eq!(resolve_spec(Some("  ".into()), || " ".into()), "all");
        assert_eq!(parse(&resolve_spec(None, String::new)).targets.len(), TARGETS.len());
    }

    #[test]
    fn env_beats_setting_and_off_is_an_explicit_opt_out() {
        assert_eq!(resolve_spec(Some("nd".into()), || panic!("setting read although the env is set")), "nd");
        assert_eq!(resolve_spec(None, || "pfd".into()), "pfd");
        assert_eq!(resolve_spec(Some("".into()), || "ewd".into()), "ewd");
        assert!(parse(&resolve_spec(Some("off".into()), String::new)).is_empty());
        assert!(parse(&resolve_spec(None, || "none".into())).is_empty());
    }

    #[test]
    fn each_failure_kind_logs_once_per_screen() {
        let mut f = Failures::default();
        assert_eq!(
            f.report("SCREEN_DU_NDL", FailureKind::Draw, "draw refused: Failed(-3, \"nan\")").as_deref(),
            Some("native displays: SCREEN_DU_NDL failed: draw refused: Failed(-3, \"nan\"); showing black")
        );
        assert_eq!(f.report("SCREEN_DU_NDL", FailureKind::Draw, "again"), None);
        assert!(f.report("SCREEN_DU_NDL", FailureKind::Panic, "panic: x").is_some());
        assert!(f.report("SCREEN_DU_NDR", FailureKind::Draw, "draw refused").is_some());
        assert_eq!(f.report("SCREEN_DU_NDR", FailureKind::Draw, "draw refused"), None);
        assert_eq!(
            f.report(ALL_SCREENS, FailureKind::Library, "a380_displays.dll not found").as_deref(),
            Some("native displays: all screens failed: a380_displays.dll not found; showing black")
        );
        assert_eq!(f.report(ALL_SCREENS, FailureKind::Library, "a380_displays.dll not found"), None);
    }

    #[test]
    fn the_black_frame_is_opaque_black() {
        let frame = black_frame(3, 2);
        assert_eq!(frame.len(), 3 * 2 * 4);
        assert!(frame.chunks_exact(4).all(|p| p == [0, 0, 0, 255]));
    }

    fn native_of(sizes: &[(u32, u32)]) -> Native {
        Native {
            pace: Arc::new(Pace::new(sizes.len())),
            slots: Arc::new(sizes.iter().map(|_| Mutex::new(Slot::default())).collect()),
            screens: sizes.iter().map(|_| "SCREEN_DU_NDL").collect(),
            stop: Arc::new(AtomicBool::new(false)),
            frames: Arc::new(sizes.iter().map(|_| AtomicU64::new(0)).collect()),
            tick: Arc::new(Tick::default()),
            threads: Vec::new(),
        }
    }

    #[test]
    fn a_failed_library_hands_every_screen_a_black_frame_to_upload() {
        let sizes = [(64, 32), (130, 70)];
        let native = native_of(&sizes);
        black_out(&native.slots, sizes);
        for (slot, (w, h)) in sizes.into_iter().enumerate() {
            let pending = native.pending(slot, 0).expect("a black frame lands, so Chromium paint goes off");
            assert_eq!(pending.size(), (w, h));
            assert!(pending.bgra().chunks_exact(4).all(|p| p == [0, 0, 0, 255]));
            assert_eq!(pending.rects(), vec![[0, 0, w, h]]);
        }
    }

    #[test]
    fn a_failed_draw_blacks_out_the_last_picture_and_recovery_replaces_it() {
        let (w, h) = (64u32, 32u32);
        let native = native_of(&[(w, h)]);
        let mut picture = vec![200u8; (w * h * 4) as usize];
        publish(&native.slots[0], &mut picture, w, h);
        assert_eq!(native.seq(0), 1);
        native.pending(0, 0).unwrap().finish();
        publish_black(&native.slots[0], w, h);
        assert_eq!(native.seq(0), 2);
        let pending = native.pending(0, 1).unwrap();
        assert!(pending.bgra().chunks_exact(4).all(|p| p == [0, 0, 0, 255]));
        pending.finish();
        publish_black(&native.slots[0], w, h);
        assert_eq!(native.seq(0), 2, "a second black frame publishes nothing");
        let mut picture = vec![200u8; (w * h * 4) as usize];
        publish(&native.slots[0], &mut picture, w, h);
        assert_eq!(native.seq(0), 3);
    }

    #[test]
    fn a_poisoned_slot_still_takes_the_black_frame() {
        let native = native_of(&[(64, 32)]);
        let slots = native.slots.clone();
        let _ = std::thread::spawn(move || {
            let _guard = slots[0].lock().unwrap();
            panic!("poison");
        })
        .join();
        publish_black(&native.slots[0], 64, 32);
        assert_eq!(native.seq(0), 1);
        assert!(native.pending(0, 0).is_some());
    }

    #[test]
    fn panic_text_reads_both_payload_kinds() {
        assert_eq!(panic_text(&*std::panic::catch_unwind(|| panic!("static")).unwrap_err()), "panic: static");
        assert_eq!(panic_text(&*std::panic::catch_unwind(|| panic!("{}", 7)).unwrap_err()), "panic: 7");
    }

    #[test]
    fn groups_pick_every_screen_of_that_display() {
        assert_eq!(screens_of("nd"), ["SCREEN_DU_NDL", "SCREEN_DU_NDR"]);
        assert_eq!(screens_of("PFD"), ["SCREEN_DU_PFDL", "SCREEN_DU_PFDR"]);
        assert_eq!(screens_of("rmp"), ["SCREEN_DU_RMP_1", "SCREEN_DU_RMP_2", "SCREEN_DU_RMP_3"]);
        assert_eq!(screens_of("oit"), ["SCREEN_OIT_LEFT", "SCREEN_OIT_RIGHT"]);
    }

    #[test]
    fn single_screens_and_ids_pick_one() {
        assert_eq!(screens_of("ndl"), ["SCREEN_DU_NDL"]);
        assert_eq!(screens_of("rmp2"), ["SCREEN_DU_RMP_2"]);
        assert_eq!(screens_of("$screen_du_ewd"), ["SCREEN_DU_EWD"]);
        assert_eq!(screens_of("clock"), ["Clock"]);
        assert_eq!(screens_of("CLOCK"), ["Clock"]);
        assert_eq!(screens_of("mfd"), ["SCREEN_DU_MFD"]);
    }

    #[test]
    fn a_list_is_deduplicated_and_in_table_order() {
        assert_eq!(screens_of("nd,pfd ndl;ewd"), ["SCREEN_DU_PFDL", "SCREEN_DU_NDL", "SCREEN_DU_PFDR", "SCREEN_DU_NDR", "SCREEN_DU_EWD"]);
    }

    #[test]
    fn all_picks_every_target_and_unknown_names_are_reported() {
        assert_eq!(parse("all").targets.len(), TARGETS.len());
        let s = parse("nd, hud, efb,xyz");
        assert_eq!(s.targets.len(), 3);
        assert_eq!(s.unknown, ["hud", "xyz"]);
    }

    #[test]
    fn target_lookup_ignores_case_and_dollar() {
        let s = parse("nd,mfd");
        assert_eq!(s.target_for("$SCREEN_DU_NDR").map(|t| (t.display, t.variant)), Some(("nd", "2")));
        assert_eq!(s.target_for("screen_du_mfd").map(|t| t.display), Some("mfdx"));
        assert!(s.target_for("SCREEN_DU_EWD").is_none());
    }

    #[test]
    fn every_target_is_a_known_screen_once() {
        for t in TARGETS {
            assert!(screens::find(t.screen).is_some(), "{} is not a screen", t.screen);
            assert_eq!(TARGETS.iter().filter(|o| o.screen == t.screen).count(), 1, "{} twice", t.screen);
        }
        for (_, ids) in NAMES {
            for id in *ids {
                assert!(TARGETS.iter().any(|t| t.screen == *id), "{id} has no target");
            }
        }
    }

    #[test]
    fn every_target_has_a_read_set() {
        for t in TARGETS {
            assert!(super::super::inputs::read_set(&t.read_set()).is_some(), "{} ({})", t.screen, t.read_set());
        }
    }

    #[test]
    fn the_sd_is_the_legacy_page_with_sdv2_drawn_over_it() {
        let sd = TARGETS.iter().find(|t| t.screen == "SCREEN_DU_SD").unwrap();
        let layers: Vec<(&str, &str, String)> = sd.layers().into_iter().map(|(key, display, _, read)| (key, display, read)).collect();
        assert_eq!(layers, [("SCREEN_DU_SD", "sd", "sd".to_string()), ("SCREEN_DU_SD+sdv2", "sdv2", "sdv2".to_string())]);
        assert_eq!(parse("sd").targets[0].over.len(), 1);
    }

    #[test]
    fn feed_keys_are_unique_and_every_layer_has_a_read_set() {
        let mut keys = std::collections::HashSet::new();
        for t in TARGETS {
            for (key, display, _, read) in t.layers() {
                assert!(keys.insert(key), "feed key {key} twice");
                assert!(super::super::inputs::read_set(&read).is_some(), "{display} ({read}) has no read set");
            }
        }
    }

    #[test]
    fn only_the_nds_have_an_underlay() {
        assert_eq!(underlay_side("SCREEN_DU_NDL"), Some(Side::Left));
        assert_eq!(underlay_side("SCREEN_DU_NDR"), Some(Side::Right));
        for t in TARGETS.iter().filter(|t| !t.screen.starts_with("SCREEN_DU_ND")) {
            assert_eq!(underlay_side(t.screen), None, "{}", t.screen);
        }
    }

    #[test]
    fn screens_without_a_native_display_are_never_picked() {
        for id in ["SCREEN_FAP", "SCREEN_HUD_L", "SCREEN_HUD_R"] {
            assert!(TARGETS.iter().all(|t| t.screen != id));
            assert!(parse("all").target_for(id).is_none());
        }
    }

    #[test]
    fn a_sim_frame_wakes_the_render_thread_at_once() {
        let tick = Arc::new(Tick::default());
        let waiter = {
            let tick = tick.clone();
            std::thread::spawn(move || {
                let started = Instant::now();
                (tick.wait_past(0, Duration::from_secs(5)), started.elapsed())
            })
        };
        std::thread::sleep(Duration::from_millis(20));
        tick.bump();
        let (frame, waited) = waiter.join().unwrap();
        assert_eq!(frame, 1);
        assert!(waited < Duration::from_secs(2), "woke after {waited:?}");
    }

    #[test]
    fn without_sim_frames_the_render_thread_still_draws_after_the_idle_wait() {
        let tick = Tick::default();
        let started = Instant::now();
        assert_eq!(tick.wait_past(0, Duration::from_millis(15)), 0);
        assert!(started.elapsed() >= Duration::from_millis(15));
        tick.bump();
        tick.bump();
        assert_eq!(tick.wait_past(0, IDLE_WAIT), 2, "missed frames collapse into one pass");
    }

    #[test]
    fn dropping_the_source_wakes_and_stops_its_thread() {
        let mut native = native_of(&[(64, 32)]);
        let (tick, stop) = (native.tick.clone(), native.stop.clone());
        native.threads.push(std::thread::spawn(move || {
            let mut seen = 0;
            while !stop.load(Ordering::Relaxed) {
                seen = tick.wait_past(seen, Duration::from_secs(30));
            }
        }));
        let started = Instant::now();
        drop(native);
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn rgba_becomes_bgra() {
        let mut p = [1, 2, 3, 4, 5, 6, 7, 8];
        rgba_to_bgra(&mut p);
        assert_eq!(p, [3, 2, 1, 4, 7, 6, 5, 8]);
    }

    #[test]
    fn identical_pictures_have_no_dirty_rects() {
        let a = vec![7u8; 200 * 70 * 4];
        assert!(diff_rects(&a, &a, 200, 70).is_empty());
    }

    #[test]
    fn a_changed_pixel_dirties_only_its_tile() {
        let (w, h) = (200usize, 70usize);
        let a = vec![0u8; w * h * 4];
        let mut b = a.clone();
        b[(40 * w + 70) * 4 + 1] = 9;
        assert_eq!(diff_rects(&a, &b, w, h), vec![[64, 32, 64, 32]]);
    }

    #[test]
    fn neighbouring_tiles_merge_and_edges_are_clamped() {
        let (w, h) = (200usize, 70usize);
        let a = vec![0u8; w * h * 4];
        let mut b = a.clone();
        b[(2 * w + 10) * 4] = 1;
        b[(2 * w + 70) * 4] = 1;
        b[(69 * w + 199) * 4 + 3] = 1;
        assert_eq!(diff_rects(&a, &b, w, h), vec![[0, 0, 128, 32], [192, 64, 8, 6]]);
    }

    #[test]
    fn publishing_accumulates_dirty_rects_until_finished() {
        let slot = Mutex::new(Slot { dirty: None, ..Slot::default() });
        let (w, h) = (128u32, 64u32);
        let mut scratch = vec![0u8; (w * h * 4) as usize];
        publish(&slot, &mut scratch, w, h);
        assert_eq!(slot.lock().unwrap().seq, 1);
        assert!(slot.lock().unwrap().dirty.is_none(), "the first frame is whole");
        slot.lock().unwrap().dirty = Some(Vec::new());
        let mut scratch = vec![0u8; (w * h * 4) as usize];
        scratch[0] = 5;
        publish(&slot, &mut scratch, w, h);
        let mut scratch = vec![0u8; (w * h * 4) as usize];
        scratch[(40 * w as usize + 100) * 4] = 5;
        publish(&slot, &mut scratch, w, h);
        let s = slot.lock().unwrap();
        assert_eq!(s.seq, 3);
        assert_eq!(s.dirty, Some(vec![[0, 0, 64, 32], [0, 0, 64, 32], [64, 32, 64, 32]]));
    }

    #[test]
    fn an_unchanged_picture_publishes_nothing() {
        let slot = Mutex::new(Slot { dirty: None, ..Slot::default() });
        let mut scratch = vec![3u8; 64 * 32 * 4];
        publish(&slot, &mut scratch, 64, 32);
        let mut again = vec![3u8; 64 * 32 * 4];
        publish(&slot, &mut again, 64, 32);
        assert_eq!(slot.lock().unwrap().seq, 1);
    }
}
