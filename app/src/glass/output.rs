use std::sync::atomic::Ordering;

use fbw_a380_systems::xphfbw_bridge::ScreenBlock;
use windows::core::{Interface, HSTRING};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HMODULE};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIAdapter, IDXGIFactory1, IDXGIKeyedMutex, IDXGIResource1, DXGI_SHARED_RESOURCE_READ, DXGI_SHARED_RESOURCE_WRITE};

pub struct Gpu {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
    pub adapter: String,
}

pub fn gpu() -> Option<Gpu> {
    let factory = unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }.ok()?;
    let mut best: Option<(u64, IDXGIAdapter, String)> = None;
    for i in 0.. {
        let Ok(adapter) = (unsafe { factory.EnumAdapters1(i) }) else { break };
        let Ok(desc) = (unsafe { adapter.GetDesc1() }) else { continue };
        if desc.Flags & 2 != 0 {
            continue;
        }
        let vram = desc.DedicatedVideoMemory as u64;
        let name = String::from_utf16_lossy(&desc.Description).trim_end_matches('\0').to_string();
        if best.as_ref().is_none_or(|b| vram > b.0) {
            best = Some((vram, adapter.cast().ok()?, name));
        }
    }
    let (_, adapter, name) = best?;
    let mut device = None;
    let mut context = None;
    unsafe {
        D3D11CreateDevice(&adapter, D3D_DRIVER_TYPE_UNKNOWN, HMODULE::default(), D3D11_CREATE_DEVICE_BGRA_SUPPORT, None, D3D11_SDK_VERSION, Some(&mut device), None, Some(&mut context)).ok()?;
    }
    Some(Gpu { device: device?, context: context?, adapter: name })
}

fn texture(gpu: &Gpu, width: u32, height: u32, misc: u32, staging: bool) -> Option<ID3D11Texture2D> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: if staging { D3D11_USAGE_STAGING } else { D3D11_USAGE_DEFAULT },
        BindFlags: if staging { 0 } else { (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32 },
        CPUAccessFlags: if staging { D3D11_CPU_ACCESS_READ.0 as u32 } else { 0 },
        MiscFlags: misc,
    };
    let mut out = None;
    unsafe { gpu.device.CreateTexture2D(&desc, None, Some(&mut out)) }.ok()?;
    out
}

struct Shared {
    tex: ID3D11Texture2D,
    mutex: IDXGIKeyedMutex,
    handle: HANDLE,
    generation: u64,
}

impl Drop for Shared {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

pub struct Output {
    pub block: ScreenBlock,
    pub width: u32,
    pub height: u32,
    pub draw: ID3D11Texture2D,
    pub target: Option<ID2D1Bitmap1>,
    shared: Option<Shared>,
    staging: Option<ID3D11Texture2D>,
    pub presented: u64,
}

impl Output {
    pub fn new(gpu: &Gpu, block: ScreenBlock) -> Option<Self> {
        let h = block.header();
        let (width, height) = (h.width.load(Ordering::Relaxed), h.height.load(Ordering::Relaxed));
        let draw = texture(gpu, width, height, 0, false)?;
        if std::env::var_os("XPHFBW_GLASS_CPU").is_some() {
            h.gpu_refused.store(1, Ordering::Relaxed);
        }
        Some(Self { block, width, height, draw, target: None, shared: None, staging: None, presented: 0 })
    }

    fn shared(&mut self, gpu: &Gpu) -> Option<&Shared> {
        if self.shared.is_none() {
            let generation = crate::gpu_share::next_generation();
            let (pw, ph) = ScreenBlock::gpu_padded(self.width, self.height);
            let tex = texture(gpu, pw, ph, (D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0 | D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX.0) as u32, false)?;
            let resource = tex.cast::<IDXGIResource1>().ok()?;
            let name = HSTRING::from(self.block.gpu_texture_name(generation));
            let handle = unsafe { resource.CreateSharedHandle(None, DXGI_SHARED_RESOURCE_READ.0 | DXGI_SHARED_RESOURCE_WRITE.0, &name) }.ok()?;
            let mutex = tex.cast::<IDXGIKeyedMutex>().ok()?;
            self.shared = Some(Shared { tex, mutex, handle, generation });
        }
        self.shared.as_ref()
    }

    pub fn cpu_path(&self) -> bool {
        self.block.header().gpu_refused.load(Ordering::Relaxed) != 0
    }

    pub fn present(&mut self, gpu: &Gpu) -> bool {
        let (w, ht) = (self.width, self.height);
        if self.block.header().gpu_refused.load(Ordering::Relaxed) == 0 {
            let draw = self.draw.clone();
            if self.shared(gpu).is_none() {
                self.block.header().gpu_refused.store(1, Ordering::Relaxed);
                return false;
            }
            let shared = self.shared.as_ref().expect("made above");
            let acquired = unsafe { (Interface::vtable(&shared.mutex).AcquireSync)(Interface::as_raw(&shared.mutex), 0, 4) };
            if acquired.0 != 0 {
                return false;
            }
            unsafe {
                let source = D3D11_BOX { left: 0, top: 0, front: 0, right: w, bottom: ht, back: 1 };
                gpu.context.CopySubresourceRegion(&shared.tex, 0, 0, 0, 0, &draw, 0, Some(&source));
                let _ = shared.mutex.ReleaseSync(0);
            }
            let generation = shared.generation;
            let h = self.block.header();
            h.gpu_generation.store(generation, Ordering::Release);
            h.publish(&[[0, 0, w, ht]]);
            self.presented += 1;
            return true;
        }
        let h = self.block.header();
        if self.staging.is_none() {
            self.staging = texture(gpu, w, ht, 0, true);
        }
        let Some(stage) = self.staging.as_ref() else { return false };
        unsafe {
            gpu.context.CopyResource(stage, &self.draw);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            if gpu.context.Map(stage, 0, D3D11_MAP_READ, 0, Some(&mut mapped)).is_err() {
                return false;
            }
            h.writing.store(1, Ordering::Relaxed);
            let dst = self.block.pixels();
            let row = w as usize * 4;
            for y in 0..ht as usize {
                let src = std::slice::from_raw_parts((mapped.pData as *const u8).add(y * mapped.RowPitch as usize), row);
                dst[y * row..(y + 1) * row].copy_from_slice(src);
            }
            gpu.context.Unmap(stage, 0);
            h.writing.store(0, Ordering::Relaxed);
        }
        h.publish(&[[0, 0, w, ht]]);
        self.presented += 1;
        true
    }
}
