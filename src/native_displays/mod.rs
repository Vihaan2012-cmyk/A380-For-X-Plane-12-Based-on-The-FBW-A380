//! The cockpit screens drawn by the native C++/Skia displays of
//! `a380-displays`, loaded at run time from `a380_displays.dll` next to the
//! plugin through its C ABI (`core/include/a380/c_api.h`).
//!
//! The renderer owns its own GL context on one thread. [`gpu_init`] makes it
//! and every other call here must come from that thread; [`Display`] and
//! [`Inputs`] hold raw handles and so are neither `Send` nor `Sync`.

pub mod input;
pub mod inputs;
pub mod popout;
pub mod screen;

use std::ffi::{c_char, c_double, c_int, c_uint, c_void, CStr, CString};
use std::path::PathBuf;
use std::ptr::{null, null_mut, NonNull};
use std::sync::OnceLock;

pub(crate) mod dylib;

/// What the native screens cost: main-thread time (input publishing, texture
/// uploads, the quads), render-thread time per pass and the bytes uploaded,
/// summed between reports.
pub mod cost {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    pub const REPORT_EVERY: Duration = Duration::from_secs(60);

    static MAIN_NS: AtomicU64 = AtomicU64::new(0);
    static RENDER_NS: AtomicU64 = AtomicU64::new(0);
    static RENDER_PASSES: AtomicU64 = AtomicU64::new(0);
    static UPLOAD_BYTES: AtomicU64 = AtomicU64::new(0);

    pub fn main_since(started: Instant) {
        MAIN_NS.fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
    }

    pub fn render(took: Duration) {
        RENDER_NS.fetch_add(took.as_nanos() as u64, Ordering::Relaxed);
        RENDER_PASSES.fetch_add(1, Ordering::Relaxed);
    }

    pub fn uploaded(bytes: u64) {
        UPLOAD_BYTES.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Averages over one report period.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Report {
        /// Main-thread ms per sim frame.
        pub main_ms: f64,
        /// Render-thread ms per render pass.
        pub render_ms: f64,
        pub upload_mb_s: f64,
    }

    impl Report {
        pub fn from_totals(main_ns: u64, sim_frames: u64, render_ns: u64, passes: u64, bytes: u64, seconds: f64) -> Self {
            let per = |ns: u64, n: u64| if n == 0 { 0. } else { ns as f64 / 1e6 / n as f64 };
            Self { main_ms: per(main_ns, sim_frames), render_ms: per(render_ns, passes), upload_mb_s: if seconds > 0. { bytes as f64 / 1e6 / seconds } else { 0. } }
        }

        pub fn line(&self) -> String {
            format!("native displays: frame cost main={:.3} render={:.2} upload={:.2}", self.main_ms, self.render_ms, self.upload_mb_s)
        }
    }

    /// The period's averages, starting the next period from zero.
    pub fn take(sim_frames: u64, seconds: f64) -> Report {
        let main = MAIN_NS.swap(0, Ordering::Relaxed);
        let render = RENDER_NS.swap(0, Ordering::Relaxed);
        let passes = RENDER_PASSES.swap(0, Ordering::Relaxed);
        let bytes = UPLOAD_BYTES.swap(0, Ordering::Relaxed);
        Report::from_totals(main, sim_frames, render, passes, bytes, seconds)
    }

    #[cfg(test)]
    mod tests {
        use super::Report;

        #[test]
        fn the_report_averages_per_frame_and_per_second() {
            let r = Report::from_totals(30_000_000, 60, 120_000_000, 60, 12_000_000, 60.);
            assert_eq!(r.line(), "native displays: frame cost main=0.500 render=2.00 upload=0.20");
            assert_eq!(Report::from_totals(5, 0, 5, 0, 5, 0.), Report { main_ms: 0., render_ms: 0., upload_mb_s: 0. });
        }
    }
}

pub const ABI_VERSION: c_int = 3;

#[cfg(windows)]
pub const LIBRARY_NAME: &str = "a380_displays.dll";
#[cfg(target_os = "macos")]
pub const LIBRARY_NAME: &str = "liba380_displays.dylib";
#[cfg(all(unix, not(target_os = "macos")))]
pub const LIBRARY_NAME: &str = "liba380_displays.so";

macro_rules! api {
    ($($name:ident($($arg:ident: $ty:ty),*) $(-> $ret:ty)? = $fallback:expr;)*) => {
        #[allow(non_snake_case, dead_code)]
        struct Api {
            _library: dylib::Library,
            $($name: unsafe extern "C" fn($($ty),*) $(-> $ret)?,)*
        }

        impl Api {
            fn bind(library: dylib::Library) -> Result<Api, String> {
                $(
                    let $name = library.sym(stringify!($name)).ok_or_else(|| format!("{} has no {}", LIBRARY_NAME, stringify!($name)))?;
                    let $name: unsafe extern "C" fn($($ty),*) $(-> $ret)? = unsafe { std::mem::transmute($name) };
                )*
                Ok(Api { $($name,)* _library: library })
            }
        }

        #[allow(non_snake_case, dead_code, clippy::missing_safety_doc)]
        mod ffi {
            use super::*;
            $(
                pub unsafe fn $name($($arg: $ty),*) $(-> $ret)? {
                    match api() {
                        Some(a) => (a.$name)($($arg),*),
                        None => $fallback,
                    }
                }
            )*
        }
    };
}

api! {
    a380d_gpu_init() -> c_int = -100;
    a380d_last_error() -> *const c_char = null();
    a380d_set_asset_dir(kind: *const c_char, path: *const c_char) -> c_int = -100;
    a380d_create(id: *const c_char, variant: *const c_char) -> *mut c_void = null_mut();
    a380d_destroy(display: *mut c_void) = ();
    a380d_size(display: *const c_void, css_w: *mut c_int, css_h: *mut c_int) = ();
    a380d_inputs_new() -> *mut c_void = null_mut();
    a380d_inputs_free(inputs: *mut c_void) = ();
    a380d_set_var(inputs: *mut c_void, name: *const c_char, unit: *const c_char, value: c_double) -> c_int = -100;
    a380d_set_string(inputs: *mut c_void, name: *const c_char, utf8: *const c_char) -> c_int = -100;
    a380d_set_bus(inputs: *mut c_void, topic: *const c_char, json: *const c_char) -> c_int = -100;
    a380d_set_bus_string(inputs: *mut c_void, topic: *const c_char, utf8: *const c_char, len: usize) -> c_int = -100;
    a380d_set_generic(inputs: *mut c_void, key: *const c_char, json: *const c_char) -> c_int = -100;
    a380d_set_fetched(inputs: *mut c_void, url: *const c_char, body: *const c_char, len: usize) -> c_int = -100;
    a380d_set_env_number(inputs: *mut c_void, path: *const c_char, value: c_double) -> c_int = -100;
    a380d_set_now(inputs: *mut c_void, ms: c_double) -> c_int = -100;
    a380d_set_dpr(inputs: *mut c_void, dpr: c_double) -> c_int = -100;
    a380d_draw(display: *mut c_void, inputs: *const c_void, rgba: *mut u8, w: c_int, h: c_int, stride: c_int) -> c_int = -100;
    a380d_draw_layers(
        displays: *const *mut c_void,
        inputs: *const *const c_void,
        n: c_int,
        rgba: *mut u8,
        w: c_int,
        h: c_int,
        stride: c_int
    ) -> c_int = -100;
    a380d_set_underlay(display: *mut c_void, rgba: *const u8, w: c_int, h: c_int, stride: c_int) -> c_int = -100;
    a380d_pointer(display: *mut c_void, x: c_double, y: c_double, kind: c_int, button: c_int) -> c_int = -100;
    a380d_key(display: *mut c_void, key: c_int, mods: c_int, down: c_int) -> c_int = -100;
    a380d_take_writes(display: *mut c_void, out: *mut input::RawWrite, cap: c_int) -> c_int = -100;
    a380d_input_native(display: *const c_void) -> c_int = 0;
    a380d_boxes(display: *const c_void, xywh: *mut f32, cap: c_int) -> c_int = 0;
    a380d_gpu_share(n: c_int) -> c_int = -100;
    a380d_gpu_shared() -> c_int = 0;
    a380d_draw_layers_gpu(
        displays: *const *mut c_void,
        inputs: *const *const c_void,
        n: c_int,
        w: c_int,
        h: c_int,
        texture: *mut c_uint,
        fence: *mut *mut c_void
    ) -> c_int = -100;
    a380d_gpu_wait(fence: *mut c_void) -> c_int = -100;
    a380d_draw_layers_host(displays: *const *mut c_void, inputs: *const *const c_void, n: c_int, w: c_int, h: c_int, texture: *mut c_uint) -> c_int = -100;
    a380d_host_diag(out: *mut c_char, cap: c_int) -> c_int = 0;
}

static API: OnceLock<Result<Api, String>> = OnceLock::new();

fn api() -> Option<&'static Api> {
    API.get()?.as_ref().ok()
}

/// Loads the display library once and keeps it for the process: from
/// `A380_DISPLAYS_DLL`, else next to this plugin's `win.xpl`. A missing file
/// or symbol, or a different ABI version, is logged once and returned as the
/// error every later call repeats; the screens then show black.
pub fn load() -> Result<(), &'static str> {
    API.get_or_init(|| {
        let result = open_library();
        match &result {
            Ok(_) => crate::log("native displays: display library loaded"),
            Err(e) => crate::log(&screen::failure_line(screen::ALL_SCREENS, e)),
        }
        result
    })
    .as_ref()
    .map(|_| ())
    .map_err(String::as_str)
}

fn library_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = std::env::var_os("A380_DISPLAYS_DLL").filter(|p| !p.is_empty()) {
        paths.push(PathBuf::from(path));
    }
    if let Some(dir) = crate::js_bridge::plugin_dir() {
        paths.push(dir.join("64").join(LIBRARY_NAME));
    }
    #[cfg(test)]
    if let Some(path) = option_env!("A380_DISPLAYS_DLL_BUILT") {
        paths.push(PathBuf::from(path));
    }
    paths
}

fn open_library() -> Result<Api, String> {
    let candidates = library_candidates();
    let Some(path) = candidates.iter().find(|p| p.is_file()) else {
        return Err(format!("{LIBRARY_NAME} not found (looked in {candidates:?})"));
    };
    dylib::log_native_faults();
    let library = dylib::Library::open(path)?;
    let version = library.sym("a380d_abi_version").ok_or_else(|| format!("{} has no a380d_abi_version", path.display()))?;
    let version: unsafe extern "C" fn() -> c_int = unsafe { std::mem::transmute(version) };
    let found = unsafe { version() };
    if found != ABI_VERSION {
        return Err(format!("{} speaks ABI {found}, this plugin needs {ABI_VERSION}", path.display()));
    }
    let api = Api::bind(library)?;
    configure_assets(&api);
    Ok(api)
}

fn set_asset_dir(api: &Api, kind: &str, path: &std::path::Path) -> bool {
    let (Ok(kind), Ok(path)) = (CString::new(kind), CString::new(path.to_string_lossy().as_bytes())) else { return false };
    unsafe { (api.a380d_set_asset_dir)(kind.as_ptr(), path.as_ptr()) == 0 }
}

/// Points the library at this aircraft's `html_ui` (fonts, images and the
/// IFE pages below it) and the local AMDB folder.
fn configure_assets(api: &Api) {
    let html_ui = std::env::var_os("A380_HTML_UI")
        .map(PathBuf::from)
        .or_else(|| crate::js_bridge::plugin_dir().and_then(|d| Some(d.parent()?.parent()?.join("html_ui"))));
    match html_ui.filter(|p| p.is_dir()) {
        Some(dir) => {
            if !set_asset_dir(api, "html_ui", &dir) {
                crate::log(&format!("native displays: the library refused html_ui {}", dir.display()));
            }
        }
        None => crate::log("native displays: no html_ui next to the plugin; the library keeps its own search paths"),
    }
    let amdb = crate::oans::plugin::data_root();
    if amdb.is_dir() {
        set_asset_dir(api, "amdb", &amdb);
    }
}

/// The library's text for the last failure on this thread.
pub fn host_diag() -> String {
    let mut buf = vec![0 as c_char; 16384];
    let n = unsafe { ffi::a380d_host_diag(buf.as_mut_ptr(), buf.len() as c_int) };
    if n <= 0 {
        return String::new();
    }
    unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned()
}

pub fn last_error() -> String {
    let p = unsafe { ffi::a380d_last_error() };
    if p.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

#[derive(Debug, PartialEq, Eq)]
pub enum GpuInit {
    Gpu,
    CpuOnly,
}

/// Creates the render thread's GL context. Call it once, on the thread that
/// makes every later call; it does nothing a second time.
pub fn gpu_init() -> Result<GpuInit, String> {
    load().map_err(str::to_string)?;
    match unsafe { ffi::a380d_gpu_init() } {
        1 => Ok(GpuInit::Gpu),
        0 => Ok(GpuInit::CpuOnly),
        code => Err(format!("a380d_gpu_init returned {code}: {}", last_error())),
    }
}

/// Main thread, X-Plane's GL context current: GL contexts for the next `n`
/// render threads that share textures with it. How many were made.
pub fn gpu_share(n: usize) -> Result<usize, String> {
    load().map_err(str::to_string)?;
    match unsafe { ffi::a380d_gpu_share(n as c_int) } {
        made if made >= 0 => Ok(made as usize),
        code => Err(format!("a380d_gpu_share returned {code}: {}", last_error())),
    }
}

/// Whether this render thread's GPU context shares textures with X-Plane's.
pub fn gpu_shared() -> bool {
    unsafe { ffi::a380d_gpu_shared() == 1 }
}

/// Makes this thread's GL context wait on `fence` (a GLsync, 0 for none) and deletes it.
pub fn gpu_wait(fence: usize) {
    if fence != 0 {
        unsafe { ffi::a380d_gpu_wait(fence as *mut c_void) };
    }
}

/// [`draw_layers`] into a GL texture X-Plane's context can read: its name and
/// the GLsync to wait on before reading it.
pub fn draw_layers_gpu(layers: &mut [(&mut Display, &Inputs)], width: u32, height: u32) -> Result<(u32, usize), DrawError> {
    if layers.is_empty() || width == 0 || height == 0 {
        return Err(DrawError::Buffer);
    }
    let displays: Vec<*mut c_void> = layers.iter().map(|(d, _)| d.ptr.as_ptr()).collect();
    let inputs: Vec<*const c_void> = layers.iter().map(|(_, i)| i.ptr.as_ptr().cast_const()).collect();
    let mut texture: c_uint = 0;
    let mut fence: *mut c_void = null_mut();
    let code = unsafe { ffi::a380d_draw_layers_gpu(displays.as_ptr(), inputs.as_ptr(), layers.len() as c_int, width as c_int, height as c_int, &mut texture, &mut fence) };
    match code {
        1 => Ok((texture, fence as usize)),
        code => Err(DrawError::Failed(code, last_error())),
    }
}

/// [`draw_layers`] on the GL context current on this thread (X-Plane's, in a
/// screen's draw callback), into a texture with mips the first layer owns;
/// returns that texture's name.
pub fn draw_layers_host(layers: &mut [(&mut Display, &Inputs)], width: u32, height: u32) -> Result<u32, DrawError> {
    if layers.is_empty() || width == 0 || height == 0 {
        return Err(DrawError::Buffer);
    }
    let displays: Vec<*mut c_void> = layers.iter().map(|(d, _)| d.ptr.as_ptr()).collect();
    let inputs: Vec<*const c_void> = layers.iter().map(|(_, i)| i.ptr.as_ptr().cast_const()).collect();
    let mut texture: c_uint = 0;
    match unsafe { ffi::a380d_draw_layers_host(displays.as_ptr(), inputs.as_ptr(), layers.len() as c_int, width as c_int, height as c_int, &mut texture) } {
        1 => Ok(texture),
        code => Err(DrawError::Failed(code, last_error())),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DrawError {
    /// The buffer is smaller than `stride * height`, or the stride is less
    /// than a row of pixels.
    Buffer,
    Failed(i32, String),
}

pub struct Display {
    ptr: NonNull<c_void>,
}

impl Display {
    pub fn new(id: &str, variant: &str) -> Option<Self> {
        let id = CString::new(id).ok()?;
        let variant = CString::new(variant).ok()?;
        NonNull::new(unsafe { ffi::a380d_create(id.as_ptr(), variant.as_ptr()) }).map(|ptr| Self { ptr })
    }

    pub(crate) fn raw(&self) -> *mut c_void {
        self.ptr.as_ptr()
    }

    /// The display's size in CSS pixels.
    pub fn size(&self) -> (i32, i32) {
        let (mut w, mut h) = (0, 0);
        unsafe { ffi::a380d_size(self.ptr.as_ptr(), &mut w, &mut h) };
        (w, h)
    }

    /// The picture drawn scaled to the whole surface before this display:
    /// straight RGBA8888, `width` x `height` pixels, copied. `None` removes it.
    pub fn set_underlay(&mut self, picture: Option<(&[u8], u32, u32)>) -> bool {
        match picture {
            None => unsafe { ffi::a380d_set_underlay(self.ptr.as_ptr(), null(), 0, 0, 0) == 0 },
            Some((rgba, width, height)) => {
                if width == 0 || height == 0 || rgba.len() < width as usize * height as usize * 4 {
                    return false;
                }
                unsafe { ffi::a380d_set_underlay(self.ptr.as_ptr(), rgba.as_ptr(), width as c_int, height as c_int, width as c_int * 4) == 0 }
            }
        }
    }

    /// Border boxes of the last drawn document's elements (`a380d_boxes`):
    /// x, y, w, h in CSS pixels, document order; empty without a document.
    pub fn boxes(&self) -> Vec<[f32; 4]> {
        let n = unsafe { ffi::a380d_boxes(self.ptr.as_ptr(), std::ptr::null_mut(), 0) };
        if n <= 0 {
            return Vec::new();
        }
        let mut out = vec![[0f32; 4]; n as usize];
        let got = unsafe { ffi::a380d_boxes(self.ptr.as_ptr(), out.as_mut_ptr().cast(), n) };
        out.truncate(got.clamp(0, n) as usize);
        out
    }

    /// Draws into `rgba` (premultiplied RGBA8888, `stride` bytes a row,
    /// `width` x `height` pixels: the display's CSS size times the inputs'
    /// device pixel ratio, rounded). `true` when the picture changed since
    /// the last draw of this display.
    pub fn draw(&mut self, inputs: &Inputs, rgba: &mut [u8], width: u32, height: u32, stride: usize) -> Result<bool, DrawError> {
        if width == 0 || height == 0 || stride < width as usize * 4 || rgba.len() < stride * height as usize {
            return Err(DrawError::Buffer);
        }
        let code = unsafe { ffi::a380d_draw(self.ptr.as_ptr(), inputs.ptr.as_ptr(), rgba.as_mut_ptr(), width as c_int, height as c_int, stride as c_int) };
        match code {
            1 => Ok(true),
            0 => Ok(false),
            code => Err(DrawError::Failed(code, last_error())),
        }
    }
}

/// Draws `layers` in order onto one surface: the first on black, each later
/// one over the result with its own inputs. The surface and the change
/// detection belong to the first layer's display. Sizes and the return value
/// are [`Display::draw`]'s.
pub fn draw_layers(layers: &mut [(&mut Display, &Inputs)], rgba: &mut [u8], width: u32, height: u32, stride: usize) -> Result<bool, DrawError> {
    if layers.is_empty() || width == 0 || height == 0 || stride < width as usize * 4 || rgba.len() < stride * height as usize {
        return Err(DrawError::Buffer);
    }
    let displays: Vec<*mut c_void> = layers.iter().map(|(d, _)| d.ptr.as_ptr()).collect();
    let inputs: Vec<*const c_void> = layers.iter().map(|(_, i)| i.ptr.as_ptr().cast_const()).collect();
    let code = unsafe { ffi::a380d_draw_layers(displays.as_ptr(), inputs.as_ptr(), layers.len() as c_int, rgba.as_mut_ptr(), width as c_int, height as c_int, stride as c_int) };
    match code {
        1 => Ok(true),
        0 => Ok(false),
        code => Err(DrawError::Failed(code, last_error())),
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        unsafe { ffi::a380d_destroy(self.ptr.as_ptr()) };
    }
}

pub struct Inputs {
    ptr: NonNull<c_void>,
}

impl Inputs {
    pub fn new() -> Self {
        Self::try_new().expect("a380d_inputs_new returned null")
    }

    pub fn try_new() -> Option<Self> {
        NonNull::new(unsafe { ffi::a380d_inputs_new() }).map(|ptr| Self { ptr })
    }

    pub fn set_var(&mut self, name: &str, unit: &str, value: f64) -> bool {
        let Ok(name) = CString::new(name) else { return false };
        let Ok(unit) = CString::new(unit) else { return false };
        let unit = if unit.as_bytes().is_empty() { null() } else { unit.as_ptr() };
        unsafe { ffi::a380d_set_var(self.ptr.as_ptr(), name.as_ptr(), unit, value) == 0 }
    }

    pub fn set_string(&mut self, name: &str, value: &str) -> bool {
        let (Ok(name), Ok(value)) = (CString::new(name), CString::new(value)) else { return false };
        unsafe { ffi::a380d_set_string(self.ptr.as_ptr(), name.as_ptr(), value.as_ptr()) == 0 }
    }

    pub fn set_bus(&mut self, topic: &str, json: &str) -> bool {
        let (Ok(topic), Ok(json)) = (CString::new(topic), CString::new(json)) else { return false };
        unsafe { ffi::a380d_set_bus(self.ptr.as_ptr(), topic.as_ptr(), json.as_ptr()) == 0 }
    }

    pub fn set_bus_string(&mut self, topic: &str, text: &str) -> bool {
        let Ok(topic) = CString::new(topic) else { return false };
        unsafe { ffi::a380d_set_bus_string(self.ptr.as_ptr(), topic.as_ptr(), text.as_ptr().cast(), text.len()) == 0 }
    }

    pub fn set_generic(&mut self, key: &str, json: &str) -> bool {
        let (Ok(key), Ok(json)) = (CString::new(key), CString::new(json)) else { return false };
        unsafe { ffi::a380d_set_generic(self.ptr.as_ptr(), key.as_ptr(), json.as_ptr()) == 0 }
    }

    pub fn set_fetched(&mut self, url: &str, body: &[u8]) -> bool {
        let Ok(url) = CString::new(url) else { return false };
        unsafe { ffi::a380d_set_fetched(self.ptr.as_ptr(), url.as_ptr(), body.as_ptr().cast(), body.len()) == 0 }
    }

    pub fn set_env_number(&mut self, path: &str, value: f64) -> bool {
        let Ok(path) = CString::new(path) else { return false };
        unsafe { ffi::a380d_set_env_number(self.ptr.as_ptr(), path.as_ptr(), value) == 0 }
    }

    pub fn set_now(&mut self, ms: f64) -> bool {
        unsafe { ffi::a380d_set_now(self.ptr.as_ptr(), ms) == 0 }
    }

    pub fn set_dpr(&mut self, dpr: f64) -> bool {
        unsafe { ffi::a380d_set_dpr(self.ptr.as_ptr(), dpr) == 0 }
    }
}

impl Default for Inputs {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Inputs {
    fn drop(&mut self) {
        unsafe { ffi::a380d_inputs_free(self.ptr.as_ptr()) };
    }
}

impl inputs::InputSink for Inputs {
    fn set_var(&mut self, name: &str, unit: &str, value: f64) {
        Inputs::set_var(self, name, unit, value);
    }
    fn set_string(&mut self, name: &str, value: &str) {
        Inputs::set_string(self, name, value);
    }
    fn set_bus(&mut self, topic: &str, json: &str) {
        Inputs::set_bus(self, topic, json);
    }
    fn set_generic(&mut self, key: &str, json: &str) {
        Inputs::set_generic(self, key, json);
    }
    fn set_fetched(&mut self, url: &str, body: &[u8]) {
        Inputs::set_fetched(self, url, body);
    }
    fn set_bus_string(&mut self, topic: &str, text: &str) {
        Inputs::set_bus_string(self, topic, text);
    }
    fn set_env_number(&mut self, path: &str, value: f64) {
        Inputs::set_env_number(self, path, value);
    }
    fn set_now(&mut self, ms: f64) {
        Inputs::set_now(self, ms);
    }
    fn set_dpr(&mut self, dpr: f64) {
        Inputs::set_dpr(self, dpr);
    }
}

#[cfg(test)]
mod tests;
