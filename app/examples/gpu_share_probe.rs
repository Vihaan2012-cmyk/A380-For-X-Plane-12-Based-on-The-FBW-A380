use std::ffi::c_void;

use windows::core::{s, w, Interface, PCSTR};
use windows::Win32::Foundation::{HANDLE, HMODULE};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{IDXGIKeyedMutex, IDXGIResource1, DXGI_SHARED_RESOURCE_READ, DXGI_SHARED_RESOURCE_WRITE};
use windows::Win32::Graphics::Gdi::GetDC;
use windows::Win32::Graphics::OpenGL::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{CreateWindowExW, WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW};

const W: u32 = 64;
const H: u32 = 32;
const NAME: &str = "Local\\xphfbw_gpu_share_probe";

const GL_TEXTURE_2D: u32 = 0x0DE1;
const GL_RGBA8: u32 = 0x8058;
const GL_BGRA: u32 = 0x80E1;
const GL_UNSIGNED_BYTE: u32 = 0x1401;
const GL_EXTENSIONS: u32 = 0x1F03;
const GL_NUM_EXTENSIONS: u32 = 0x821D;
const GL_HANDLE_TYPE_D3D11_IMAGE_EXT: u32 = 0x958B;
const GL_DEDICATED_MEMORY_OBJECT_EXT: u32 = 0x9581;

type CreateMemoryObjects = unsafe extern "system" fn(i32, *mut u32);
type MemoryObjectParameteriv = unsafe extern "system" fn(u32, u32, *const i32);
type ImportMemoryWin32Name = unsafe extern "system" fn(u32, u64, u32, *const c_void);
type TexStorageMem2D = unsafe extern "system" fn(u32, i32, u32, i32, i32, u32, u64);
type AcquireKeyedMutex = unsafe extern "system" fn(u32, u64, u32) -> u8;
type ReleaseKeyedMutex = unsafe extern "system" fn(u32, u64) -> u8;
type GetStringi = unsafe extern "system" fn(u32, u32) -> *const u8;

fn pattern() -> Vec<u8> {
    let mut px = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            let (b, g, r) = match (x < W / 2, y < H / 2) {
                (true, true) => (0, 0, 255),
                (false, true) => (0, 255, 0),
                (true, false) => (255, 0, 0),
                (false, false) => (255, 255, 255),
            };
            px[i..i + 4].copy_from_slice(&[b, g, r, 255]);
        }
    }
    px
}

unsafe fn gl_proc<T>(name: PCSTR) -> Option<T> {
    let p = wglGetProcAddress(name)?;
    Some(std::mem::transmute_copy(&p))
}

fn main() -> windows::core::Result<()> {
    unsafe {
        let mut device = None;
        D3D11CreateDevice(None, D3D_DRIVER_TYPE_HARDWARE, HMODULE::default(), D3D11_CREATE_DEVICE_BGRA_SUPPORT, None, D3D11_SDK_VERSION, Some(&mut device), None, None)?;
        let device = device.unwrap();
        let desc = D3D11_TEXTURE2D_DESC {
            Width: W,
            Height: H,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: (D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0 | D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX.0) as u32,
        };
        let px = pattern();
        let init = D3D11_SUBRESOURCE_DATA { pSysMem: px.as_ptr() as *const c_void, SysMemPitch: W * 4, SysMemSlicePitch: 0 };
        let mut tex = None;
        device.CreateTexture2D(&desc, Some(&init), Some(&mut tex))?;
        let tex = tex.unwrap();
        let res: IDXGIResource1 = tex.cast()?;
        let _handle: HANDLE = res.CreateSharedHandle(None, (DXGI_SHARED_RESOURCE_READ | DXGI_SHARED_RESOURCE_WRITE).0, w!("Local\\xphfbw_gpu_share_probe"))?;
        let km: IDXGIKeyedMutex = tex.cast()?;
        km.AcquireSync(0, u32::MAX)?;
        km.ReleaseSync(1)?;
        println!("d3d11: shared texture {W}x{H} as {NAME}");

        let hwnd = CreateWindowExW(WINDOW_EX_STYLE(0), w!("STATIC"), w!("probe"), WS_OVERLAPPEDWINDOW, 0, 0, 16, 16, None, None, Some(GetModuleHandleW(None)?.into()), None)?;
        let dc = GetDC(Some(hwnd));
        let pfd = PIXELFORMATDESCRIPTOR {
            nSize: std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,
            nVersion: 1,
            dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
            iPixelType: PFD_TYPE_RGBA,
            cColorBits: 32,
            ..Default::default()
        };
        let format = ChoosePixelFormat(dc, &pfd);
        SetPixelFormat(dc, format, &pfd)?;
        let rc = wglCreateContext(dc)?;
        wglMakeCurrent(dc, rc)?;
        let vendor = std::ffi::CStr::from_ptr(glGetString(GL_VENDOR) as *const i8).to_string_lossy().into_owned();
        let renderer = std::ffi::CStr::from_ptr(glGetString(GL_RENDERER) as *const i8).to_string_lossy().into_owned();
        println!("gl: {vendor} / {renderer}");

        let mut have = Vec::new();
        if let Some(get_stringi) = gl_proc::<GetStringi>(s!("glGetStringi")) {
            let mut n = 0;
            glGetIntegerv(GL_NUM_EXTENSIONS, &mut n);
            for i in 0..n as u32 {
                have.push(std::ffi::CStr::from_ptr(get_stringi(GL_EXTENSIONS, i) as *const i8).to_string_lossy().into_owned());
            }
        }
        for e in ["GL_EXT_memory_object", "GL_EXT_memory_object_win32", "GL_EXT_win32_keyed_mutex"] {
            println!("gl: {e}: {}", have.iter().any(|h| h == e));
        }

        let create: CreateMemoryObjects = gl_proc(s!("glCreateMemoryObjectsEXT")).expect("glCreateMemoryObjectsEXT");
        let param: MemoryObjectParameteriv = gl_proc(s!("glMemoryObjectParameterivEXT")).expect("glMemoryObjectParameterivEXT");
        let import: ImportMemoryWin32Name = gl_proc(s!("glImportMemoryWin32NameEXT")).expect("glImportMemoryWin32NameEXT");
        let storage: TexStorageMem2D = gl_proc(s!("glTexStorageMem2DEXT")).expect("glTexStorageMem2DEXT");
        let acquire: AcquireKeyedMutex = gl_proc(s!("glAcquireKeyedMutexWin32EXT")).expect("glAcquireKeyedMutexWin32EXT");
        let release: ReleaseKeyedMutex = gl_proc(s!("glReleaseKeyedMutexWin32EXT")).expect("glReleaseKeyedMutexWin32EXT");

        let mut mem = 0u32;
        create(1, &mut mem);
        let dedicated = 1i32;
        param(mem, GL_DEDICATED_MEMORY_OBJECT_EXT, &dedicated);
        let name: Vec<u16> = NAME.encode_utf16().chain(Some(0)).collect();
        import(mem, (W * H * 4) as u64, GL_HANDLE_TYPE_D3D11_IMAGE_EXT, name.as_ptr() as *const c_void);
        println!("gl: import error {:#x}", glGetError());
        let mut gltex = 0u32;
        glGenTextures(1, &mut gltex);
        glBindTexture(GL_TEXTURE_2D, gltex);
        storage(GL_TEXTURE_2D, 1, GL_RGBA8, W as i32, H as i32, mem, 0);
        println!("gl: storage error {:#x}", glGetError());
        let got = acquire(mem, 1, u32::MAX);
        println!("gl: keyed mutex acquired {}", got != 0);
        let mut back = vec![0u8; (W * H * 4) as usize];
        glGetTexImage(GL_TEXTURE_2D, 0, GL_BGRA, GL_UNSIGNED_BYTE, back.as_mut_ptr() as *mut c_void);
        println!("gl: readback error {:#x}", glGetError());
        release(mem, 0);
        let at = |x: u32, y: u32| {
            let i = ((y * W + x) * 4) as usize;
            [back[i + 2], back[i + 1], back[i]]
        };
        println!("gl row 0 (first row in memory): left {:?} right {:?}", at(2, 2), at(W - 3, 2));
        println!("gl row {} (last row in memory): left {:?} right {:?}", H - 1, at(2, H - 3), at(W - 3, H - 3));
        println!("d3d11 wrote: top-left red, top-right green, bottom-left blue, bottom-right white (as RGB)");
        let ok = at(2, 2) == [255, 0, 0] && at(W - 3, 2) == [0, 255, 0] && at(2, H - 3) == [0, 0, 255];
        println!("match without flip or swizzle: {ok}");
    }
    Ok(())
}
