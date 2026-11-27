#include "a380/gpu.h"
#include "a380/env.h"
#include "a380/raster.h"

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <mutex>
#include <string>
#include <vector>

#ifdef A380_GPU
#ifdef _WIN32
#include <windows.h>
#include <GL/gl.h>
#else
#include "gpu_egl.h"
#include <dlfcn.h>
#ifdef __APPLE__
#include <OpenGL/gl3.h>
#else
#include <GLES2/gl2.h>
#endif
#endif
#include "include/gpu/ganesh/GrContextOptions.h"
#include "include/core/SkColorSpace.h"
#include "include/gpu/ganesh/GrBackendSurface.h"
#include "include/gpu/ganesh/GrDirectContext.h"
#include "include/gpu/ganesh/SkSurfaceGanesh.h"
#include "include/gpu/ganesh/gl/GrGLDirectContext.h"
#include "include/gpu/ganesh/gl/GrGLAssembleInterface.h"
#include "include/gpu/ganesh/gl/GrGLBackendSurface.h"
#include "include/gpu/ganesh/gl/GrGLInterface.h"
#ifdef _WIN32
#include <dxgi.h>
#endif
#endif

namespace a380 {

#ifdef A380_GPU
#ifdef _WIN32
using EglProc = void* (__stdcall*)(const char*);
static EglProc g_egl_get_proc = nullptr;

static GrGLFuncPtr angle_get_proc(void*, const char name[]) {
    return (GrGLFuncPtr)g_egl_get_proc(name);
}

static bool angle_adapter_luid(const char* vendor, LUID* out) {
    unsigned want = !std::strcmp(vendor, "intel") ? 0x8086 : !std::strcmp(vendor, "nvidia") ? 0x10DE : !std::strcmp(vendor, "amd") ? 0x1002 : 0;
    if (!want) return false;
    IDXGIFactory1* f = nullptr;
    if (FAILED(CreateDXGIFactory1(__uuidof(IDXGIFactory1), (void**)&f))) return false;
    bool found = false;
    IDXGIAdapter1* a = nullptr;
    for (UINT i = 0; !found && f->EnumAdapters1(i, &a) != DXGI_ERROR_NOT_FOUND; i++) {
        DXGI_ADAPTER_DESC1 d;
        if (SUCCEEDED(a->GetDesc1(&d)) && d.VendorId == want) {
            *out = d.AdapterLuid;
            found = true;
        }
        a->Release();
    }
    f->Release();
    return found;
}

static sk_sp<const GrGLInterface> angle_interface(const char* dir) {
    std::string base(dir);
    HMODULE egl = LoadLibraryA((base + "/libEGL.dll").c_str());
    LoadLibraryA((base + "/libGLESv2.dll").c_str());
    if (!egl) return nullptr;
    g_egl_get_proc = (EglProc)GetProcAddress(egl, "eglGetProcAddress");
    if (!g_egl_get_proc) return nullptr;
    using GetPlatformDisplay = void* (__stdcall*)(unsigned, void*, const int*);
    using Initialize = unsigned (__stdcall*)(void*, int*, int*);
    using BindApi = unsigned (__stdcall*)(unsigned);
    using ChooseConfig = unsigned (__stdcall*)(void*, const int*, void**, int, int*);
    using CreatePbuffer = void* (__stdcall*)(void*, void*, const int*);
    using CreateContext = void* (__stdcall*)(void*, void*, void*, const int*);
    using MakeCurrent = unsigned (__stdcall*)(void*, void*, void*, void*);
    auto get_display = (GetPlatformDisplay)g_egl_get_proc("eglGetPlatformDisplayEXT");
    auto init = (Initialize)GetProcAddress(egl, "eglInitialize");
    auto bind = (BindApi)GetProcAddress(egl, "eglBindAPI");
    auto choose = (ChooseConfig)GetProcAddress(egl, "eglChooseConfig");
    auto pbuffer = (CreatePbuffer)GetProcAddress(egl, "eglCreatePbufferSurface");
    auto create = (CreateContext)GetProcAddress(egl, "eglCreateContext");
    auto make_current = (MakeCurrent)GetProcAddress(egl, "eglMakeCurrent");
    if (!get_display || !init || !bind || !choose || !pbuffer || !create || !make_current) return nullptr;
    std::vector<int> attrs = {0x3203, 0x3208};
    LUID luid;
    if (const char* v = a380::getenv_cached("A380_GPU_ADAPTER"); v && angle_adapter_luid(v, &luid)) {
        attrs.insert(attrs.end(), {0x34A0, (int)luid.HighPart, 0x34A1, (int)luid.LowPart});
    }
    attrs.push_back(0x3038);
    void* dpy = get_display(0x3202, nullptr, attrs.data());
    int major = 0, minor = 0;
    if (!dpy || !init(dpy, &major, &minor)) return nullptr;
    bind(0x30A0);
    const int cfg_attrs[] = {0x3040, 0x40, 0x3033, 0x0001, 0x3024, 8, 0x3023, 8, 0x3022, 8, 0x3021, 8, 0x3026, 8, 0x3038};
    void* cfg = nullptr;
    int n = 0;
    if (!choose(dpy, cfg_attrs, &cfg, 1, &n) || n < 1) return nullptr;
    const int pb_attrs[] = {0x3057, 16, 0x3056, 16, 0x3038};
    void* surf = pbuffer(dpy, cfg, pb_attrs);
    const int ctx_attrs[] = {0x3098, 3, 0x3038};
    void* ctx = create(dpy, cfg, nullptr, ctx_attrs);
    if (!surf || !ctx || !make_current(dpy, surf, surf, ctx)) return nullptr;
    return GrGLMakeAssembledInterface(nullptr, angle_get_proc);
}
#endif

#ifdef _WIN32
struct SharedGl {
    HDC dc;
    HGLRC rc;
};
static std::mutex g_share_mutex;
static std::vector<SharedGl> g_share_pool;
thread_local bool t_shared = false;

static HDC hidden_dc() {
    WNDCLASSA wc{};
    wc.style = CS_OWNDC;
    wc.lpfnWndProc = DefWindowProcA;
    wc.hInstance = GetModuleHandleA(nullptr);
    wc.lpszClassName = "a380disp_gl";
    RegisterClassA(&wc);
    HWND wnd = CreateWindowA(wc.lpszClassName, "", WS_OVERLAPPEDWINDOW, 0, 0, 16, 16, nullptr, nullptr, wc.hInstance, nullptr);
    return wnd ? GetDC(wnd) : nullptr;
}

static void choose_format(HDC dc) {
    PIXELFORMATDESCRIPTOR pfd{};
    pfd.nSize = sizeof(pfd);
    pfd.nVersion = 1;
    pfd.dwFlags = PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER;
    pfd.iPixelType = PFD_TYPE_RGBA;
    pfd.cColorBits = 32;
    pfd.cStencilBits = 8;
    pfd.iLayerType = PFD_MAIN_PLANE;
    SetPixelFormat(dc, ChoosePixelFormat(dc, &pfd), &pfd);
}
#endif

// The native GL interface with object labels made no-ops: Skia labels every texture it hands out, a driver call each.
static sk_sp<const GrGLInterface> quiet_interface() {
    sk_sp<const GrGLInterface> base = GrGLMakeNativeInterface();
    if (!base || a380::getenv_cached("A380_GPU_GLLABELS")) return base;
    auto copy = sk_make_sp<GrGLInterface>();
    copy->fStandard = base->fStandard;
    copy->fExtensions = base->fExtensions;
    copy->fFunctions = base->fFunctions;
    copy->fFunctions.fObjectLabel = [](GrGLenum, GrGLuint, GrGLsizei, const GrGLchar*) {};
    return copy;
}

static GrDirectContext* gpu_context() {
    thread_local sk_sp<GrDirectContext>& ctx = *new sk_sp<GrDirectContext>;
    thread_local bool tried = false;
    if (tried) return ctx.get();
    tried = true;
    const char* off = a380::getenv_cached("A380_GPU");
    if (off && !std::strcmp(off, "0")) return nullptr;
    GrContextOptions options;
    const char* ims = a380::getenv_cached("A380_GPU_IMSAA");
    options.fInternalMultisampleCount = ims ? std::atoi(ims) : 0;
    if (const char* v = a380::getenv_cached("A380_GPU_NODFPATHS")) options.fDisableDistanceFieldPaths = std::atoi(v) != 0;
    if (const char* v = a380::getenv_cached("A380_GPU_MASKCACHE")) options.fAllowPathMaskCaching = std::atoi(v) != 0;
    if (const char* v = a380::getenv_cached("A380_GPU_NOSTENCIL")) options.fAvoidStencilBuffers = std::atoi(v) != 0;
    // Chromium runs Skia without glGetError checks (its command buffer); each check is a driver round trip.
    options.fSkipGLErrorChecks = a380::getenv_cached("A380_GPU_GLERRORS") ? GrContextOptions::Enable::kNo : GrContextOptions::Enable::kYes;
#ifdef _WIN32
    if (const char* dir = a380::getenv_cached("A380_GPU_ANGLE")) {
        if (auto iface = angle_interface(dir)) ctx = GrDirectContexts::MakeGL(iface, options);
        if (!ctx) std::fprintf(stderr, "warning: ANGLE context failed, rendering on CPU\n");
        return ctx.get();
    }
    {
        std::lock_guard<std::mutex> lock(g_share_mutex);
        while (!g_share_pool.empty()) {
            SharedGl s = g_share_pool.back();
            g_share_pool.pop_back();
            if (!wglMakeCurrent(s.dc, s.rc)) continue;
            ctx = GrDirectContexts::MakeGL(quiet_interface(), options);
            if (ctx) {
                t_shared = true;
                return ctx.get();
            }
            wglMakeCurrent(nullptr, nullptr);
        }
    }
    HDC dc = hidden_dc();
    if (dc) choose_format(dc);
    HGLRC rc = dc ? wglCreateContext(dc) : nullptr;
    if (!rc || !wglMakeCurrent(dc, rc)) {
        std::fprintf(stderr, "warning: no GL context, rendering on CPU\n");
        return nullptr;
    }
    ctx = GrDirectContexts::MakeGL(quiet_interface(), options);
    if (!ctx) std::fprintf(stderr, "warning: Ganesh GL context failed, rendering on CPU\n");
    return ctx.get();
#else
    if (auto iface = egl_make_interface()) ctx = GrDirectContexts::MakeGL(iface, options);
    if (!ctx) std::fprintf(stderr, "warning: EGL Ganesh GL context failed, rendering on CPU\n");
    return ctx.get();
#endif
}
#endif

int gpu_share(int n) {
#if defined(A380_GPU) && defined(_WIN32)
    HGLRC host = wglGetCurrentContext();
    HDC host_dc = wglGetCurrentDC();
    if (!host || !host_dc) return -1;
    using CreateAttribs = HGLRC(WINAPI*)(HDC, HGLRC, const int*);
    auto create_attribs = (CreateAttribs)wglGetProcAddress("wglCreateContextAttribsARB");
    GLint major = 0, minor = 0, profile = 0;
    glGetIntegerv(0x821B, &major);
    glGetIntegerv(0x821C, &minor);
    glGetIntegerv(0x9126, &profile);
    while (glGetError() != 0) {}
    int format = GetPixelFormat(host_dc);
    PIXELFORMATDESCRIPTOR host_pfd{};
    if (format) DescribePixelFormat(host_dc, format, sizeof(host_pfd), &host_pfd);
    int made = 0;
    for (int i = 0; i < n; i++) {
        HDC dc = hidden_dc();
        if (!dc) break;
        if (!format || !SetPixelFormat(dc, format, &host_pfd)) choose_format(dc);
        HGLRC rc = nullptr;
        if (create_attribs) {
            std::vector<int> attrs;
            if (major >= 3) attrs.insert(attrs.end(), {0x2091, major, 0x2092, minor});
            if (profile) attrs.insert(attrs.end(), {0x9126, profile});
            attrs.push_back(0);
            rc = create_attribs(dc, host, attrs.data());
        }
        if (!rc) {
            rc = wglCreateContext(dc);
            if (rc && !wglShareLists(host, rc)) {
                wglDeleteContext(rc);
                rc = nullptr;
            }
        }
        if (!rc) break;
        std::lock_guard<std::mutex> lock(g_share_mutex);
        g_share_pool.push_back({dc, rc});
        made++;
    }
    wglMakeCurrent(host_dc, host);
    return made;
#else
    (void)n;
    return -1;
#endif
}

bool gpu_shared() {
#if defined(A380_GPU) && defined(_WIN32)
    return gpu_context() != nullptr && t_shared;
#else
    return false;
#endif
}

sk_sp<SkSurface> make_shared_surface(const SkImageInfo& info, const SkSurfaceProps* props) {
#if defined(A380_GPU) && defined(_WIN32)
    if (GrDirectContext* gpu = gpu_context(); gpu && t_shared) {
        return SkSurfaces::RenderTarget(gpu, skgpu::Budgeted::kNo, info, 0, kTopLeft_GrSurfaceOrigin, props);
    }
#endif
    (void)info;
    (void)props;
    return nullptr;
}

unsigned surface_texture(SkSurface* surface) {
#ifdef A380_GPU
    GrBackendTexture t = SkSurfaces::GetBackendTexture(surface, SkSurfaces::BackendHandleAccess::kFlushRead);
    GrGLTextureInfo gl;
    if (t.isValid() && GrBackendTextures::GetGLTextureInfo(t, &gl)) return gl.fID;
#endif
    (void)surface;
    return 0;
}

#if defined(A380_GPU) && defined(_WIN32)
using FenceSyncFn = void*(APIENTRY*)(unsigned, unsigned);
using WaitSyncFn = void(APIENTRY*)(void*, unsigned, unsigned long long);
using DeleteSyncFn = void(APIENTRY*)(void*);
thread_local FenceSyncFn t_fence_sync = nullptr;
thread_local WaitSyncFn t_wait_sync = nullptr;
thread_local DeleteSyncFn t_delete_sync = nullptr;

static bool sync_procs() {
    if (!t_fence_sync) {
        t_fence_sync = (FenceSyncFn)wglGetProcAddress("glFenceSync");
        t_wait_sync = (WaitSyncFn)wglGetProcAddress("glWaitSync");
        t_delete_sync = (DeleteSyncFn)wglGetProcAddress("glDeleteSync");
    }
    return t_fence_sync && t_wait_sync && t_delete_sync;
}
#endif

#ifdef A380_GPU
static void* current_gl() {
#ifdef _WIN32
    return (void*)wglGetCurrentContext();
#else
    return nullptr;
#endif
}

#if !defined(_WIN32) && !defined(__APPLE__)
// X-Plane's own GL context on Linux, whatever made it current: GLX first, then the process, then EGL.
static GrGLFuncPtr host_get_proc(void*, const char name[]) {
    using GlxProc = void* (*)(const unsigned char*);
    using EglProc = void* (*)(const char*);
    static GlxProc glx = [] {
        void* lib = dlopen("libGL.so.1", RTLD_LAZY | RTLD_NOLOAD);
        if (!lib) lib = dlopen("libGL.so.1", RTLD_LAZY);
        return lib ? (GlxProc)dlsym(lib, "glXGetProcAddressARB") : nullptr;
    }();
    static EglProc egl = [] {
        void* lib = dlopen("libEGL.so.1", RTLD_LAZY);
        return lib ? (EglProc)dlsym(lib, "eglGetProcAddress") : nullptr;
    }();
    if (glx)
        if (void* f = glx((const unsigned char*)name)) return (GrGLFuncPtr)f;
    if (void* f = dlsym(RTLD_DEFAULT, name)) return (GrGLFuncPtr)f;
    return egl ? (GrGLFuncPtr)egl(name) : nullptr;
}
#endif

static sk_sp<GrDirectContext> make_host_context(const GrContextOptions& options) {
#if !defined(_WIN32) && !defined(__APPLE__)
    if (auto iface = GrGLMakeAssembledInterface(nullptr, host_get_proc)) return GrDirectContexts::MakeGL(iface, options);
    return nullptr;
#else
    return GrDirectContexts::MakeGL(options);
#endif
}

struct HostContext {
    void* gl = nullptr;
    sk_sp<GrDirectContext> ctx;
    int made = 0;
};

static std::vector<HostContext>& host_contexts() {
    thread_local std::vector<HostContext>& all = *new std::vector<HostContext>;
    return all;
}

static HostContext& host_slot() {
    void* gl = current_gl();
    for (HostContext& h : host_contexts())
        if (h.gl == gl) return h;
    host_contexts().push_back(HostContext{gl, nullptr, 0});
    return host_contexts().back();
}

static GrDirectContext* host_context() {
    HostContext& h = host_slot();
    if ((!h.ctx || h.ctx->abandoned()) && h.made < 8) {
        h.made++;
        if (h.ctx) h.ctx->releaseResourcesAndAbandonContext();
        GrContextOptions options;
        h.ctx = make_host_context(options);
    }
    return h.ctx.get();
}

static std::string fbo_probe(unsigned texture) {
#ifdef _WIN32
    using GenFramebuffers = void(APIENTRY*)(int, unsigned*);
    using DeleteFramebuffers = void(APIENTRY*)(int, const unsigned*);
    using BindFramebuffer = void(APIENTRY*)(unsigned, unsigned);
    using FramebufferTexture2D = void(APIENTRY*)(unsigned, unsigned, unsigned, unsigned, int);
    using CheckFramebufferStatus = unsigned(APIENTRY*)(unsigned);
    auto gen = (GenFramebuffers)wglGetProcAddress("glGenFramebuffers");
    auto del = (DeleteFramebuffers)wglGetProcAddress("glDeleteFramebuffers");
    auto bind = (BindFramebuffer)wglGetProcAddress("glBindFramebuffer");
    auto attach = (FramebufferTexture2D)wglGetProcAddress("glFramebufferTexture2D");
    auto check = (CheckFramebufferStatus)wglGetProcAddress("glCheckFramebufferStatus");
    if (!gen || !del || !bind || !attach || !check) return "no fbo entry points";
    int bound = 0;
    glGetIntegerv(0x8CA6, &bound);
    unsigned fbo = 0;
    gen(1, &fbo);
    bind(0x8D40, fbo);
    attach(0x8D40, 0x8CE0, 0x0DE1, texture, 0);
    unsigned status = check(0x8D40);
    unsigned err = glGetError();
    bind(0x8D40, (unsigned)bound);
    del(1, &fbo);
    char buf[160];
    snprintf(buf, sizeof buf, "own fbo status 0x%X err %u, was bound %d, gl %p, contexts %zu", status, err, bound, current_gl(), host_contexts().size());
    return buf;
#else
    (void)texture;
    return "";
#endif
}
#endif

sk_sp<SkSurface> host_texture_surface(unsigned texture, int w, int h, const SkSurfaceProps* props, std::string* why) {
#ifdef A380_GPU
    bool was_abandoned = host_slot().ctx && host_slot().ctx->abandoned();
    GrDirectContext* ctx = host_context();
    if (!ctx) {
        if (why) *why = was_abandoned ? "context abandoned, remake failed" : "no GL context";
        return nullptr;
    }
    if (ctx->abandoned()) {
        if (why) *why = "context abandoned";
        return nullptr;
    }
    if (!texture) {
        if (why) *why = "texture 0";
        return nullptr;
    }
    ctx->resetContext();
    GrGLTextureInfo info;
    info.fTarget = 0x0DE1;
    info.fID = texture;
    info.fFormat = 0x8058;
    GrBackendTexture bt = GrBackendTextures::MakeGL(w, h, skgpu::Mipmapped::kNo, info);
    sk_sp<SkSurface> s = SkSurfaces::WrapBackendTexture(ctx, bt, kTopLeft_GrSurfaceOrigin, 0, kRGBA_8888_SkColorType, nullptr, props);
    if (!s && why) {
        unsigned err = glGetError();
        *why = "wrap refused: texture " + std::to_string(texture) + " " + std::to_string(w) + "x" + std::to_string(h) +
               ", glIsTexture " + std::to_string((int)glIsTexture(texture)) + ", glGetError " + std::to_string(err) +
               (was_abandoned ? ", context was remade" : "") + ", " + fbo_probe(texture);
    } else if (s && was_abandoned && why) {
        *why = "context was remade";
    }
    return s;
#else
    (void)texture;
    (void)w;
    (void)h;
    (void)props;
    if (why) *why = "built without GPU";
    return nullptr;
#endif
}

#if defined(A380_GPU) && defined(_WIN32)
namespace {
struct QueryFns {
    void(APIENTRY* gen)(int, unsigned*) = nullptr;
    void(APIENTRY* counter)(unsigned, unsigned) = nullptr;
    void(APIENTRY* get_iv)(unsigned, unsigned, int*) = nullptr;
    void(APIENTRY* get_ui64)(unsigned, unsigned, unsigned long long*) = nullptr;
    bool ok = false;
};
const QueryFns& query_fns() {
    thread_local QueryFns f = [] {
        QueryFns f;
        f.gen = (void(APIENTRY*)(int, unsigned*))wglGetProcAddress("glGenQueries");
        f.counter = (void(APIENTRY*)(unsigned, unsigned))wglGetProcAddress("glQueryCounter");
        f.get_iv = (void(APIENTRY*)(unsigned, unsigned, int*))wglGetProcAddress("glGetQueryObjectiv");
        f.get_ui64 = (void(APIENTRY*)(unsigned, unsigned, unsigned long long*))wglGetProcAddress("glGetQueryObjectui64v");
        f.ok = f.gen && f.counter && f.get_iv && f.get_ui64;
        return f;
    }();
    return f;
}
}  // namespace
#endif

void GpuStopwatch::harvest() {
#if defined(A380_GPU) && defined(_WIN32)
    const QueryFns& f = query_fns();
    if (!f.ok) return;
    for (int i = 0; i < made_; i++) {
        if (!pending_[i]) continue;
        int ready = 0;
        f.get_iv(q_[i][1], 0x8867, &ready);
        if (!ready) continue;
        unsigned long long a = 0, b = 0;
        f.get_ui64(q_[i][0], 0x8866, &a);
        f.get_ui64(q_[i][1], 0x8866, &b);
        if (b > a) sum_ms_ += (double)(b - a) / 1e6;
        n_++;
        pending_[i] = false;
    }
#endif
}

void GpuStopwatch::begin() {
#if defined(A380_GPU) && defined(_WIN32)
    const QueryFns& f = query_fns();
    open_ = -1;
    if (!f.ok) return;
    harvest();
    int slot = -1;
    for (int i = 0; i < made_; i++)
        if (!pending_[i]) {
            slot = i;
            break;
        }
    if (slot < 0 && made_ < 8) {
        f.gen(2, q_[made_]);
        slot = made_++;
    }
    if (slot < 0) return;
    f.counter(q_[slot][0], 0x8E28);
    open_ = slot;
#endif
}

void GpuStopwatch::end() {
#if defined(A380_GPU) && defined(_WIN32)
    if (open_ < 0) return;
    query_fns().counter(q_[open_][1], 0x8E28);
    pending_[open_] = true;
    open_ = -1;
#endif
}

bool GpuStopwatch::report(double* mean_ms, int* count) {
    harvest();
    if (n_ == 0) return false;
    *mean_ms = sum_ms_ / n_;
    *count = n_;
    sum_ms_ = 0;
    n_ = 0;
    return true;
}

void* host_gl() {
#ifdef A380_GPU
    return current_gl();
#else
    return nullptr;
#endif
}

sk_sp<SkSurface> host_render_surface(int w, int h, const SkSurfaceProps* props, std::string* why) {
#ifdef A380_GPU
    GrDirectContext* ctx = host_context();
    if (!ctx || ctx->abandoned()) {
        if (why) *why = "no usable GL context";
        return nullptr;
    }
    ctx->resetContext();
    SkImageInfo info = SkImageInfo::Make(w, h, kRGBA_8888_SkColorType, kPremul_SkAlphaType);
    sk_sp<SkSurface> s = SkSurfaces::RenderTarget(ctx, skgpu::Budgeted::kNo, info, 0, kTopLeft_GrSurfaceOrigin, props, true);
    if (!s && why) *why = "render target " + std::to_string(w) + "x" + std::to_string(h) + " refused, glGetError " + std::to_string(glGetError());
    return s;
#else
    (void)w;
    (void)h;
    (void)props;
    if (why) *why = "built without GPU";
    return nullptr;
#endif
}

void host_begin() {
#ifdef A380_GPU
    if (GrDirectContext* ctx = host_context()) ctx->resetContext();
#endif
}

void host_mipmaps(unsigned texture) {
#if defined(A380_GPU) && defined(_WIN32)
    using GenerateMipmap = void(APIENTRY*)(unsigned);
    using ActiveTexture = void(APIENTRY*)(unsigned);
    static auto generate = (GenerateMipmap)wglGetProcAddress("glGenerateMipmap");
    static auto active_texture = (ActiveTexture)wglGetProcAddress("glActiveTexture");
    if (!texture || !generate) return;
    if (active_texture) active_texture(0x84C0);
    glBindTexture(0x0DE1, texture);
    glTexParameteri(0x0DE1, 0x2801, 0x2703);
    glTexParameteri(0x0DE1, 0x2800, 0x2601);
    glTexParameteri(0x0DE1, 0x2802, 0x812F);
    glTexParameteri(0x0DE1, 0x2803, 0x812F);
    generate(0x0DE1);
    glBindTexture(0x0DE1, 0);
#else
    (void)texture;
#endif
}

std::string host_texture_diag(unsigned texture, int w, int h) {
#if defined(A380_GPU) && defined(_WIN32)
    while (glGetError()) {
    }
    glBindTexture(0x0DE1, texture);
    int base = -1, max = -1, minf = 0, immutable = -1, levels = -1;
    glGetTexParameteriv(0x0DE1, 0x813C, &base);
    glGetTexParameteriv(0x0DE1, 0x813D, &max);
    glGetTexParameteriv(0x0DE1, 0x2801, &minf);
    glGetTexParameteriv(0x0DE1, 0x912F, &immutable);
    glGetTexParameteriv(0x0DE1, 0x82DF, &levels);
    std::string out = "texture " + std::to_string(texture) + " base " + std::to_string(base) + " max " + std::to_string(max) +
                      " min 0x" + [&] { char b[16]; snprintf(b, sizeof b, "%X", minf); return std::string(b); }() +
                      " immutable " + std::to_string(immutable) + "/" + std::to_string(levels);
    for (int level : {0, 2}) {
        int lw = 0, lh = 0;
        glGetTexLevelParameteriv(0x0DE1, level, 0x1000, &lw);
        glGetTexLevelParameteriv(0x0DE1, level, 0x1001, &lh);
        if (lw <= 0 || lh <= 0) {
            out += ", level " + std::to_string(level) + " empty";
            continue;
        }
        std::vector<uint8_t> px((size_t)lw * lh * 4);
        glPixelStorei(0x0D05, 4);
        glGetTexImage(0x0DE1, level, 0x1908, 0x1401, px.data());
        size_t lit = 0;
        unsigned alpha = 0;
        for (size_t i = 0; i < px.size(); i += 4) {
            if (px[i] | px[i + 1] | px[i + 2]) lit++;
            alpha = alpha > px[i + 3] ? alpha : px[i + 3];
        }
        out += ", level " + std::to_string(level) + " " + std::to_string(lw) + "x" + std::to_string(lh) + " lit " + std::to_string(lit) +
               " max alpha " + std::to_string(alpha);
    }
    out += ", glGetError " + std::to_string(glGetError());
    glBindTexture(0x0DE1, 0);
    (void)w;
    (void)h;
    return out;
#else
    (void)texture;
    (void)w;
    (void)h;
    return "";
#endif
}

void host_flush(SkSurface* surface) {
#ifdef A380_GPU
    GrDirectContext* ctx = host_context();
    if (!ctx) return;
    ctx->flushAndSubmit(surface, GrSyncCpu::kNo);
#else
    (void)surface;
#endif
}

#if defined(A380_GPU) && defined(_WIN32)
namespace {

constexpr int kUnits = 16;
constexpr int kAttribs = 16;

struct Attrib {
    int enabled, size, type, normalized, stride, buffer, integer, divisor;
    void* pointer;
};

struct GlState {
    int program, vao, array_buffer, element_buffer, pack_buffer, unpack_buffer, uniform_buffer;
    int draw_fb, read_fb, renderbuffer, active_texture;
    int textures[kUnits], samplers[kUnits];
    int viewport[4], scissor_box[4];
    unsigned char scissor, blend, depth, stencil, cull, multisample, srgb, dither;
    int blend_src_rgb, blend_dst_rgb, blend_src_a, blend_dst_a, blend_eq_rgb, blend_eq_a;
    float blend_color[4];
    int depth_func;
    unsigned char depth_mask;
    unsigned char color_mask[4];
    int stencil_func, stencil_ref, stencil_value_mask, stencil_fail, stencil_zfail, stencil_zpass, stencil_write_mask;
    int back_stencil_func, back_stencil_ref, back_stencil_value_mask, back_stencil_fail, back_stencil_zfail, back_stencil_zpass, back_stencil_write_mask;
    int cull_face, front_face, polygon_mode[2];
    int unpack_alignment, unpack_row_length, unpack_skip_rows, unpack_skip_pixels, pack_alignment, pack_row_length;
    Attrib attribs[kAttribs];
};

struct Fns {
    void(APIENTRY* use_program)(unsigned);
    void(APIENTRY* bind_vao)(unsigned);
    void(APIENTRY* bind_buffer)(unsigned, unsigned);
    void(APIENTRY* bind_framebuffer)(unsigned, unsigned);
    void(APIENTRY* bind_renderbuffer)(unsigned, unsigned);
    void(APIENTRY* active_texture)(unsigned);
    void(APIENTRY* bind_sampler)(unsigned, unsigned);
    void(APIENTRY* blend_func_separate)(unsigned, unsigned, unsigned, unsigned);
    void(APIENTRY* blend_equation_separate)(unsigned, unsigned);
    void(APIENTRY* blend_color)(float, float, float, float);
    void(APIENTRY* stencil_func_separate)(unsigned, unsigned, int, unsigned);
    void(APIENTRY* stencil_op_separate)(unsigned, unsigned, unsigned, unsigned);
    void(APIENTRY* stencil_mask_separate)(unsigned, unsigned);
    void(APIENTRY* get_attrib_iv)(unsigned, unsigned, int*);
    void(APIENTRY* get_attrib_pointer)(unsigned, unsigned, void**);
    void(APIENTRY* attrib_pointer)(unsigned, int, unsigned, unsigned char, int, const void*);
    void(APIENTRY* attrib_ipointer)(unsigned, int, unsigned, int, const void*);
    void(APIENTRY* attrib_divisor)(unsigned, unsigned);
    void(APIENTRY* enable_attrib)(unsigned);
    void(APIENTRY* disable_attrib)(unsigned);
    bool ok;
};

template <class T>
void load(T& f, const char* name) {
    f = (T)wglGetProcAddress(name);
}

const Fns& fns() {
    thread_local Fns f = [] {
        Fns f{};
        load(f.use_program, "glUseProgram");
        load(f.bind_vao, "glBindVertexArray");
        load(f.bind_buffer, "glBindBuffer");
        load(f.bind_framebuffer, "glBindFramebuffer");
        load(f.bind_renderbuffer, "glBindRenderbuffer");
        load(f.active_texture, "glActiveTexture");
        load(f.bind_sampler, "glBindSampler");
        load(f.blend_func_separate, "glBlendFuncSeparate");
        load(f.blend_equation_separate, "glBlendEquationSeparate");
        load(f.blend_color, "glBlendColor");
        load(f.stencil_func_separate, "glStencilFuncSeparate");
        load(f.stencil_op_separate, "glStencilOpSeparate");
        load(f.stencil_mask_separate, "glStencilMaskSeparate");
        load(f.get_attrib_iv, "glGetVertexAttribiv");
        load(f.get_attrib_pointer, "glGetVertexAttribPointerv");
        load(f.attrib_pointer, "glVertexAttribPointer");
        load(f.attrib_ipointer, "glVertexAttribIPointer");
        load(f.attrib_divisor, "glVertexAttribDivisor");
        load(f.enable_attrib, "glEnableVertexAttribArray");
        load(f.disable_attrib, "glDisableVertexAttribArray");
        f.ok = f.use_program && f.bind_vao && f.bind_buffer && f.bind_framebuffer && f.bind_renderbuffer && f.active_texture &&
               f.bind_sampler && f.blend_func_separate && f.blend_equation_separate && f.blend_color && f.stencil_func_separate &&
               f.stencil_op_separate && f.stencil_mask_separate && f.get_attrib_iv && f.get_attrib_pointer && f.attrib_pointer &&
               f.attrib_ipointer && f.attrib_divisor && f.enable_attrib && f.disable_attrib;
        return f;
    }();
    return f;
}

thread_local GlState g_saved;
thread_local bool g_have_saved = false;

int geti(unsigned e) {
    int v = 0;
    glGetIntegerv(e, &v);
    return v;
}

void set_cap(unsigned cap, unsigned char on) {
    if (on)
        glEnable(cap);
    else
        glDisable(cap);
}

}  // namespace
#endif

void host_save() {
#if defined(A380_GPU) && defined(_WIN32)
    const Fns& f = fns();
    g_have_saved = f.ok;
    if (!f.ok) return;
    GlState& s = g_saved;
    s.program = geti(0x8B8D);
    s.vao = geti(0x85B5);
    s.array_buffer = geti(0x8894);
    s.pack_buffer = geti(0x88ED);
    s.unpack_buffer = geti(0x88EF);
    s.uniform_buffer = geti(0x8A28);
    s.draw_fb = geti(0x8CA6);
    s.read_fb = geti(0x8CAA);
    s.renderbuffer = geti(0x8CA7);
    s.active_texture = geti(0x84E0);
    for (int u = 0; u < kUnits; u++) {
        f.active_texture(0x84C0 + u);
        s.textures[u] = geti(0x8069);
        s.samplers[u] = geti(0x8919);
    }
    f.active_texture(s.active_texture);
    glGetIntegerv(0x0BA2, s.viewport);
    glGetIntegerv(0x0C10, s.scissor_box);
    s.scissor = glIsEnabled(0x0C11);
    s.blend = glIsEnabled(0x0BE2);
    s.depth = glIsEnabled(0x0B71);
    s.stencil = glIsEnabled(0x0B90);
    s.cull = glIsEnabled(0x0B44);
    s.multisample = glIsEnabled(0x809D);
    s.srgb = glIsEnabled(0x8DB9);
    s.dither = glIsEnabled(0x0BD0);
    s.blend_src_rgb = geti(0x80C9);
    s.blend_dst_rgb = geti(0x80C8);
    s.blend_src_a = geti(0x80CB);
    s.blend_dst_a = geti(0x80CA);
    s.blend_eq_rgb = geti(0x8009);
    s.blend_eq_a = geti(0x883D);
    glGetFloatv(0x8005, s.blend_color);
    s.depth_func = geti(0x0B74);
    glGetBooleanv(0x0B72, &s.depth_mask);
    glGetBooleanv(0x0C23, s.color_mask);
    s.stencil_func = geti(0x0B92);
    s.stencil_ref = geti(0x0B97);
    s.stencil_value_mask = geti(0x0B93);
    s.stencil_fail = geti(0x0B94);
    s.stencil_zfail = geti(0x0B95);
    s.stencil_zpass = geti(0x0B96);
    s.stencil_write_mask = geti(0x0B98);
    s.back_stencil_func = geti(0x8800);
    s.back_stencil_ref = geti(0x8CA3);
    s.back_stencil_value_mask = geti(0x8CA4);
    s.back_stencil_fail = geti(0x8801);
    s.back_stencil_zfail = geti(0x8802);
    s.back_stencil_zpass = geti(0x8803);
    s.back_stencil_write_mask = geti(0x8CA5);
    s.cull_face = geti(0x0B45);
    s.front_face = geti(0x0B46);
    glGetIntegerv(0x0B40, s.polygon_mode);
    s.unpack_alignment = geti(0x0CF5);
    s.unpack_row_length = geti(0x0CF2);
    s.unpack_skip_rows = geti(0x0CF3);
    s.unpack_skip_pixels = geti(0x0CF4);
    s.pack_alignment = geti(0x0D05);
    s.pack_row_length = geti(0x0D02);
    f.bind_vao(0);
    s.element_buffer = geti(0x8895);
    for (unsigned i = 0; i < kAttribs; i++) {
        Attrib& a = s.attribs[i];
        f.get_attrib_iv(i, 0x8622, &a.enabled);
        f.get_attrib_iv(i, 0x8623, &a.size);
        f.get_attrib_iv(i, 0x8625, &a.type);
        f.get_attrib_iv(i, 0x886A, &a.normalized);
        f.get_attrib_iv(i, 0x8624, &a.stride);
        f.get_attrib_iv(i, 0x889F, &a.buffer);
        f.get_attrib_iv(i, 0x88FD, &a.integer);
        f.get_attrib_iv(i, 0x88FE, &a.divisor);
        f.get_attrib_pointer(i, 0x8645, &a.pointer);
    }
    f.bind_vao((unsigned)s.vao);
#endif
}

void host_restore() {
#if defined(A380_GPU) && defined(_WIN32)
    if (!g_have_saved) return;
    g_have_saved = false;
    const Fns& f = fns();
    const GlState& s = g_saved;
    f.bind_vao(0);
    for (unsigned i = 0; i < kAttribs; i++) {
        const Attrib& a = s.attribs[i];
        f.bind_buffer(0x8892, (unsigned)a.buffer);
        if (a.buffer || a.pointer) {
            if (a.integer)
                f.attrib_ipointer(i, a.size, (unsigned)a.type, a.stride, a.pointer);
            else
                f.attrib_pointer(i, a.size, (unsigned)a.type, (unsigned char)a.normalized, a.stride, a.pointer);
        }
        f.attrib_divisor(i, (unsigned)a.divisor);
        if (a.enabled)
            f.enable_attrib(i);
        else
            f.disable_attrib(i);
    }
    f.bind_buffer(0x8893, (unsigned)s.element_buffer);
    f.bind_vao((unsigned)s.vao);
    f.bind_buffer(0x8892, (unsigned)s.array_buffer);
    f.bind_buffer(0x88EB, (unsigned)s.pack_buffer);
    f.bind_buffer(0x88EC, (unsigned)s.unpack_buffer);
    f.bind_buffer(0x8A11, (unsigned)s.uniform_buffer);
    f.use_program((unsigned)s.program);
    f.bind_framebuffer(0x8CA9, (unsigned)s.draw_fb);
    f.bind_framebuffer(0x8CA8, (unsigned)s.read_fb);
    f.bind_renderbuffer(0x8D41, (unsigned)s.renderbuffer);
    for (int u = 0; u < kUnits; u++) {
        f.active_texture(0x84C0 + u);
        glBindTexture(0x0DE1, (unsigned)s.textures[u]);
        f.bind_sampler((unsigned)u, (unsigned)s.samplers[u]);
    }
    f.active_texture((unsigned)s.active_texture);
    glViewport(s.viewport[0], s.viewport[1], s.viewport[2], s.viewport[3]);
    glScissor(s.scissor_box[0], s.scissor_box[1], s.scissor_box[2], s.scissor_box[3]);
    set_cap(0x0C11, s.scissor);
    set_cap(0x0BE2, s.blend);
    set_cap(0x0B71, s.depth);
    set_cap(0x0B90, s.stencil);
    set_cap(0x0B44, s.cull);
    set_cap(0x809D, s.multisample);
    set_cap(0x8DB9, s.srgb);
    set_cap(0x0BD0, s.dither);
    f.blend_func_separate((unsigned)s.blend_src_rgb, (unsigned)s.blend_dst_rgb, (unsigned)s.blend_src_a, (unsigned)s.blend_dst_a);
    f.blend_equation_separate((unsigned)s.blend_eq_rgb, (unsigned)s.blend_eq_a);
    f.blend_color(s.blend_color[0], s.blend_color[1], s.blend_color[2], s.blend_color[3]);
    glDepthFunc((unsigned)s.depth_func);
    glDepthMask(s.depth_mask);
    glColorMask(s.color_mask[0], s.color_mask[1], s.color_mask[2], s.color_mask[3]);
    f.stencil_func_separate(0x0404, (unsigned)s.stencil_func, s.stencil_ref, (unsigned)s.stencil_value_mask);
    f.stencil_op_separate(0x0404, (unsigned)s.stencil_fail, (unsigned)s.stencil_zfail, (unsigned)s.stencil_zpass);
    f.stencil_mask_separate(0x0404, (unsigned)s.stencil_write_mask);
    f.stencil_func_separate(0x0405, (unsigned)s.back_stencil_func, s.back_stencil_ref, (unsigned)s.back_stencil_value_mask);
    f.stencil_op_separate(0x0405, (unsigned)s.back_stencil_fail, (unsigned)s.back_stencil_zfail, (unsigned)s.back_stencil_zpass);
    f.stencil_mask_separate(0x0405, (unsigned)s.back_stencil_write_mask);
    glCullFace((unsigned)s.cull_face);
    glFrontFace((unsigned)s.front_face);
    glPolygonMode(0x0408, (unsigned)s.polygon_mode[0]);
    glPixelStorei(0x0CF5, s.unpack_alignment);
    glPixelStorei(0x0CF2, s.unpack_row_length);
    glPixelStorei(0x0CF3, s.unpack_skip_rows);
    glPixelStorei(0x0CF4, s.unpack_skip_pixels);
    glPixelStorei(0x0D05, s.pack_alignment);
    glPixelStorei(0x0D02, s.pack_row_length);
#endif
}

void* gpu_fence() {
#if defined(A380_GPU) && defined(_WIN32)
    if (!sync_procs()) return nullptr;
    void* f = t_fence_sync(0x9117, 0);
    glFlush();
    return f;
#else
    return nullptr;
#endif
}

void gpu_wait(void* fence) {
#if defined(A380_GPU) && defined(_WIN32)
    if (!fence || !sync_procs()) return;
    t_wait_sync(fence, 0, 0xFFFFFFFFFFFFFFFFull);
    t_delete_sync(fence);
#else
    (void)fence;
#endif
}

sk_sp<SkSurface> make_surface(const SkImageInfo& info, const SkSurfaceProps* props) {
#ifdef A380_GPU
    if (GrDirectContext* gpu = gpu_context()) {
        const char* s = a380::getenv_cached("A380_GPU_SAMPLES");
        int samples = s ? std::atoi(s) : 0;
        SkSurfaceProps p = props ? *props : SkSurfaceProps();
        const char* dm = a380::getenv_cached("A380_GPU_DMSAA");
        if (dm && std::atoi(dm)) p = SkSurfaceProps(p.flags() | SkSurfaceProps::kDynamicMSAA_Flag, p.pixelGeometry(), p.textContrast(), p.textGamma());
        if (auto surf = SkSurfaces::RenderTarget(gpu, skgpu::Budgeted::kNo, info, samples, kTopLeft_GrSurfaceOrigin, &p)) {
            set_gpu_exact(a380::getenv_cached("A380_GPU_EMULATE") == nullptr);
            return surf;
        }
    }
#endif
    return SkSurfaces::Raster(info, props);
}

bool gpu_available() {
#ifdef A380_GPU
    return gpu_context() != nullptr;
#else
    return false;
#endif
}

void flush_surface(SkSurface* surface, bool wait_for_gpu) {
#ifdef A380_GPU
    if (GrDirectContext* gpu = gpu_context()) {
        gpu->flushAndSubmit(surface, wait_for_gpu ? GrSyncCpu::kYes : GrSyncCpu::kNo);
        return;
    }
#endif
    (void)surface;
    (void)wait_for_gpu;
}

}
