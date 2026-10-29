use std::ffi::{c_char, c_int, c_void, CString};
use std::path::{Path, PathBuf};

/// A rendered PDF page: straight RGBA8, `width * 4` bytes per row.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Chart and document PDFs to images. PDFium (`pdfium.dll` / `libpdfium.so`
/// / `libpdfium.dylib`, downloaded by the installer) implements this as
/// [`Pdfium`]; without it [`NoPdf`] answers that it is missing.
pub trait PdfRenderer {
    fn page_count(&mut self, pdf: &[u8]) -> Result<usize, String>;
    /// Page `index` (0-based) scaled to fit `max_w` x `max_h` pixels.
    fn render(&mut self, pdf: &[u8], index: usize, max_w: u32, max_h: u32) -> Result<Page, String>;
}

pub struct NoPdf;

impl PdfRenderer for NoPdf {
    fn page_count(&mut self, _pdf: &[u8]) -> Result<usize, String> {
        Err("PDF charts need PDFium, which is not installed".into())
    }

    fn render(&mut self, _pdf: &[u8], _index: usize, _max_w: u32, _max_h: u32) -> Result<Page, String> {
        Err("PDF charts need PDFium, which is not installed".into())
    }
}

#[cfg(windows)]
mod dl {
    use std::ffi::{c_char, c_void};
    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
    }
    pub unsafe fn open(path: &std::path::Path) -> *mut c_void {
        use std::os::windows::ffi::OsStrExt;
        let w: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        LoadLibraryW(w.as_ptr())
    }
    pub unsafe fn sym(lib: *mut c_void, name: *const c_char) -> *mut c_void {
        GetProcAddress(lib, name)
    }
}

#[cfg(unix)]
mod dl {
    use std::ffi::{c_char, c_int, c_void};
    #[cfg_attr(target_os = "linux", link(name = "dl"))]
    extern "C" {
        fn dlopen(name: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(lib: *mut c_void, name: *const c_char) -> *mut c_void;
    }
    pub unsafe fn open(path: &std::path::Path) -> *mut c_void {
        use std::os::unix::ffi::OsStrExt;
        let Ok(c) = std::ffi::CString::new(path.as_os_str().as_bytes()) else { return std::ptr::null_mut() };
        dlopen(c.as_ptr(), 2)
    }
    pub unsafe fn sym(lib: *mut c_void, name: *const c_char) -> *mut c_void {
        dlsym(lib, name)
    }
}

type Doc = *mut c_void;
type Pg = *mut c_void;
type Bmp = *mut c_void;

#[allow(non_snake_case)]
struct Api {
    FPDF_InitLibrary: unsafe extern "C" fn(),
    FPDF_LoadMemDocument: unsafe extern "C" fn(*const c_void, c_int, *const c_char) -> Doc,
    FPDF_CloseDocument: unsafe extern "C" fn(Doc),
    FPDF_GetPageCount: unsafe extern "C" fn(Doc) -> c_int,
    FPDF_LoadPage: unsafe extern "C" fn(Doc, c_int) -> Pg,
    FPDF_ClosePage: unsafe extern "C" fn(Pg),
    FPDF_GetPageWidthF: unsafe extern "C" fn(Pg) -> f32,
    FPDF_GetPageHeightF: unsafe extern "C" fn(Pg) -> f32,
    FPDF_RenderPageBitmap: unsafe extern "C" fn(Bmp, Pg, c_int, c_int, c_int, c_int, c_int, c_int),
    FPDF_GetLastError: unsafe extern "C" fn() -> std::ffi::c_ulong,
    FPDFBitmap_Create: unsafe extern "C" fn(c_int, c_int, c_int) -> Bmp,
    FPDFBitmap_FillRect: unsafe extern "C" fn(Bmp, c_int, c_int, c_int, c_int, std::ffi::c_ulong) -> c_int,
    FPDFBitmap_GetBuffer: unsafe extern "C" fn(Bmp) -> *mut c_void,
    FPDFBitmap_GetStride: unsafe extern "C" fn(Bmp) -> c_int,
    FPDFBitmap_Destroy: unsafe extern "C" fn(Bmp),
}

const FPDF_ANNOT: c_int = 0x01;
const FPDF_LCD_TEXT: c_int = 0x02;

/// PDFium loaded at run time from the plugin folder, with rendered pages cached as PNG files.
pub struct Pdfium {
    api: Api,
    cache: Option<PathBuf>,
}

/// The PDFium library file name on this platform.
pub fn library_name() -> &'static str {
    if cfg!(windows) {
        "pdfium.dll"
    } else if cfg!(target_os = "macos") {
        "libpdfium.dylib"
    } else {
        "libpdfium.so"
    }
}

impl Pdfium {
    /// Loads PDFium from `dir` (the installer puts it beside the plugin binary) and keeps its PNG cache in `cache`.
    pub fn load(dir: &Path, cache: Option<PathBuf>) -> Result<Pdfium, String> {
        let path = dir.join(library_name());
        if !path.exists() {
            return Err(format!("{} not found", path.display()));
        }
        let lib = unsafe { dl::open(&path) };
        if lib.is_null() {
            return Err(format!("could not load {}", path.display()));
        }
        macro_rules! sym {
            ($name:ident) => {{
                let c = CString::new(stringify!($name)).unwrap();
                let p = unsafe { dl::sym(lib, c.as_ptr()) };
                if p.is_null() {
                    return Err(format!("{} lacks {}", path.display(), stringify!($name)));
                }
                unsafe { std::mem::transmute::<*mut c_void, _>(p) }
            }};
        }
        let api = Api {
            FPDF_InitLibrary: sym!(FPDF_InitLibrary),
            FPDF_LoadMemDocument: sym!(FPDF_LoadMemDocument),
            FPDF_CloseDocument: sym!(FPDF_CloseDocument),
            FPDF_GetPageCount: sym!(FPDF_GetPageCount),
            FPDF_LoadPage: sym!(FPDF_LoadPage),
            FPDF_ClosePage: sym!(FPDF_ClosePage),
            FPDF_GetPageWidthF: sym!(FPDF_GetPageWidthF),
            FPDF_GetPageHeightF: sym!(FPDF_GetPageHeightF),
            FPDF_RenderPageBitmap: sym!(FPDF_RenderPageBitmap),
            FPDF_GetLastError: sym!(FPDF_GetLastError),
            FPDFBitmap_Create: sym!(FPDFBitmap_Create),
            FPDFBitmap_FillRect: sym!(FPDFBitmap_FillRect),
            FPDFBitmap_GetBuffer: sym!(FPDFBitmap_GetBuffer),
            FPDFBitmap_GetStride: sym!(FPDFBitmap_GetStride),
            FPDFBitmap_Destroy: sym!(FPDFBitmap_Destroy),
        };
        unsafe { (api.FPDF_InitLibrary)() };
        if let Some(c) = &cache {
            let _ = std::fs::create_dir_all(c);
        }
        Ok(Pdfium { api, cache })
    }

    fn open(&self, pdf: &[u8]) -> Result<Doc, String> {
        let len = c_int::try_from(pdf.len()).map_err(|_| "PDF too large".to_string())?;
        let doc = unsafe { (self.api.FPDF_LoadMemDocument)(pdf.as_ptr() as *const c_void, len, std::ptr::null()) };
        if doc.is_null() {
            let e = unsafe { (self.api.FPDF_GetLastError)() };
            return Err(format!("PDFium could not open the document (error {e})"));
        }
        Ok(doc)
    }

    /// The page as PNG bytes, from the cache when this document page was rendered at this size before.
    pub fn render_png(&mut self, pdf: &[u8], index: usize, max_w: u32, max_h: u32) -> Result<Vec<u8>, String> {
        let file = self.cache.as_ref().map(|c| c.join(format!("{:016x}-{index}-{max_w}x{max_h}.png", fnv64(pdf))));
        if let Some(f) = &file {
            if let Ok(b) = std::fs::read(f) {
                return Ok(b);
            }
        }
        let p = self.render(pdf, index, max_w, max_h)?;
        let png = encode_png(p.width, p.height, &p.rgba);
        if let Some(f) = &file {
            let tmp = f.with_extension("tmp");
            if std::fs::write(&tmp, &png).is_ok() {
                let _ = std::fs::rename(&tmp, f);
            }
        }
        Ok(png)
    }
}

impl PdfRenderer for Pdfium {
    fn page_count(&mut self, pdf: &[u8]) -> Result<usize, String> {
        let doc = self.open(pdf)?;
        let n = unsafe { (self.api.FPDF_GetPageCount)(doc) };
        unsafe { (self.api.FPDF_CloseDocument)(doc) };
        Ok(n.max(0) as usize)
    }

    fn render(&mut self, pdf: &[u8], index: usize, max_w: u32, max_h: u32) -> Result<Page, String> {
        let a = &self.api;
        let doc = self.open(pdf)?;
        let count = unsafe { (a.FPDF_GetPageCount)(doc) }.max(0) as usize;
        if index >= count {
            unsafe { (a.FPDF_CloseDocument)(doc) };
            return Err(format!("page {} of {count}", index + 1));
        }
        let page = unsafe { (a.FPDF_LoadPage)(doc, index as c_int) };
        if page.is_null() {
            unsafe { (a.FPDF_CloseDocument)(doc) };
            return Err(format!("PDFium could not load page {}", index + 1));
        }
        let (pw, ph) = unsafe { ((a.FPDF_GetPageWidthF)(page) as f64, (a.FPDF_GetPageHeightF)(page) as f64) };
        let (w, h) = fit(pw, ph, max_w, max_h);
        let bmp = unsafe { (a.FPDFBitmap_Create)(w as c_int, h as c_int, 1) };
        if bmp.is_null() {
            unsafe {
                (a.FPDF_ClosePage)(page);
                (a.FPDF_CloseDocument)(doc);
            }
            return Err(format!("PDFium could not allocate a {w}x{h} bitmap"));
        }
        let rgba = unsafe {
            (a.FPDFBitmap_FillRect)(bmp, 0, 0, w as c_int, h as c_int, 0xFFFF_FFFF);
            (a.FPDF_RenderPageBitmap)(bmp, page, 0, 0, w as c_int, h as c_int, 0, FPDF_ANNOT | FPDF_LCD_TEXT);
            let stride = (a.FPDFBitmap_GetStride)(bmp).max(0) as usize;
            let buf = (a.FPDFBitmap_GetBuffer)(bmp) as *const u8;
            let bgra = std::slice::from_raw_parts(buf, stride * h as usize);
            let out = bgra_rows_to_rgba(bgra, stride, w as usize, h as usize);
            (a.FPDFBitmap_Destroy)(bmp);
            (a.FPDF_ClosePage)(page);
            (a.FPDF_CloseDocument)(doc);
            out
        };
        Ok(Page { width: w, height: h, rgba })
    }
}

/// Pixel size of a `pw` x `ph` point page scaled to fit `max_w` x `max_h`, keeping its shape.
pub fn fit(pw: f64, ph: f64, max_w: u32, max_h: u32) -> (u32, u32) {
    if !(pw > 0.0 && ph > 0.0) || max_w == 0 || max_h == 0 {
        return (max_w.max(1), max_h.max(1));
    }
    let s = (max_w as f64 / pw).min(max_h as f64 / ph);
    (((pw * s).round() as u32).clamp(1, max_w), ((ph * s).round() as u32).clamp(1, max_h))
}

pub fn bgra_rows_to_rgba(bgra: &[u8], stride: usize, w: usize, h: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let row = &bgra[y * stride..y * stride + w * 4];
        for p in row.chunks_exact(4) {
            out.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
    }
    out
}

fn fnv64(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for x in b {
        h ^= *x as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn crc32(data: &[&[u8]]) -> u32 {
    let mut table = [0u32; 256];
    for (i, t) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    let mut c = 0xFFFF_FFFFu32;
    for part in data {
        for b in *part {
            c = table[((c ^ *b as u32) & 0xFF) as usize] ^ (c >> 8);
        }
    }
    c ^ 0xFFFF_FFFF
}

/// RGBA8 to a PNG file's bytes.
pub fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut raw = Vec::with_capacity((w as usize * 4 + 1) * h as usize);
    for row in rgba.chunks_exact((w as usize * 4).max(1)).take(h as usize) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    let _ = z.write_all(&raw);
    let idat = z.finish().unwrap_or_default();
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut chunk = |kind: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        out.extend_from_slice(&crc32(&[kind, data]).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &idat);
    chunk(b"IEND", &[]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_keeping_aspect() {
        assert_eq!(fit(612.0, 792.0, 1000, 1000), (773, 1000));
        assert_eq!(fit(792.0, 612.0, 1000, 500), (647, 500));
    }

    #[test]
    fn png_has_valid_crc_and_size() {
        let png = encode_png(2, 1, &[255, 0, 0, 255, 0, 0, 255, 255]);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(crc32(&[b"IEND"]), 0xAE42_6082);
        assert_eq!(&png[png.len() - 4..], &0xAE42_6082u32.to_be_bytes());
    }

    #[test]
    fn swaps_bgra() {
        assert_eq!(bgra_rows_to_rgba(&[1, 2, 3, 4, 9, 9, 9, 9], 8, 1, 1), vec![3, 2, 1, 4]);
    }

    #[test]
    fn missing_library_is_an_error() {
        assert!(Pdfium::load(Path::new("no-such-dir"), None).is_err());
    }
}
