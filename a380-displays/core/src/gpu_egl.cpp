#if defined(A380_GPU) && !defined(_WIN32)

#include "gpu_egl.h"
#include "a380/env.h"

#include <EGL/egl.h>
#include <EGL/eglext.h>

#include <cstdio>
#include <cstdlib>
#include <cstring>

#include "include/gpu/ganesh/gl/GrGLAssembleInterface.h"

namespace a380 {

namespace {

GrGLFuncPtr egl_proc(void*, const char name[]) { return (GrGLFuncPtr)eglGetProcAddress(name); }

bool has_ext(const char* list, const char* name) { return list && std::strstr(list, name) != nullptr; }

EGLDisplay open_display() {
    const char* want = a380::getenv_cached("A380_EGL_PLATFORM");
    bool surfaceless = !want || !std::strcmp(want, "surfaceless");
    if (surfaceless) {
        const char* client = eglQueryString(EGL_NO_DISPLAY, EGL_EXTENSIONS);
        if (has_ext(client, "EGL_EXT_platform_base") || has_ext(client, "EGL_KHR_platform_base")) {
            auto get = (PFNEGLGETPLATFORMDISPLAYEXTPROC)eglGetProcAddress("eglGetPlatformDisplayEXT");
            if (get) {
                EGLDisplay d = get(0x31DD, EGL_DEFAULT_DISPLAY, nullptr);
                if (d != EGL_NO_DISPLAY) return d;
            }
        }
    }
    return eglGetDisplay(EGL_DEFAULT_DISPLAY);
}

}  // namespace

sk_sp<const GrGLInterface> egl_make_interface() {
    EGLDisplay dpy = open_display();
    EGLint major = 0, minor = 0;
    if (dpy == EGL_NO_DISPLAY || !eglInitialize(dpy, &major, &minor)) return nullptr;
    if (!eglBindAPI(EGL_OPENGL_ES_API)) return nullptr;

    const EGLint pb_cfg[] = {EGL_SURFACE_TYPE,    EGL_PBUFFER_BIT, EGL_RENDERABLE_TYPE, EGL_OPENGL_ES3_BIT, EGL_RED_SIZE, 8,
                             EGL_GREEN_SIZE,      8,               EGL_BLUE_SIZE,       8,                  EGL_ALPHA_SIZE, 8,
                             EGL_NONE};
    const EGLint bare_cfg[] = {EGL_RENDERABLE_TYPE, EGL_OPENGL_ES3_BIT, EGL_RED_SIZE, 8, EGL_GREEN_SIZE, 8,
                               EGL_BLUE_SIZE,       8,                  EGL_ALPHA_SIZE, 8, EGL_NONE};
    EGLConfig cfg = nullptr;
    EGLint n = 0;
    bool pbuffer_cfg = eglChooseConfig(dpy, pb_cfg, &cfg, 1, &n) && n >= 1;
    if (!pbuffer_cfg && !(eglChooseConfig(dpy, bare_cfg, &cfg, 1, &n) && n >= 1)) return nullptr;

    const EGLint ctx_attrs[] = {EGL_CONTEXT_CLIENT_VERSION, 3, EGL_NONE};
    EGLContext ctx = eglCreateContext(dpy, cfg, EGL_NO_CONTEXT, ctx_attrs);
    if (ctx == EGL_NO_CONTEXT) return nullptr;

    EGLSurface surf = EGL_NO_SURFACE;
    if (pbuffer_cfg) {
        const EGLint pb[] = {EGL_WIDTH, 16, EGL_HEIGHT, 16, EGL_NONE};
        surf = eglCreatePbufferSurface(dpy, cfg, pb);
    }
    if (!eglMakeCurrent(dpy, surf, surf, ctx)) return nullptr;
    if (a380::getenv_cached("A380_GPU_INFO")) {
        using GetString = const unsigned char* (*)(unsigned);
        if (auto gs = (GetString)eglGetProcAddress("glGetString"))
            std::fprintf(stderr, "gl: %s | %s | %s\n", (const char*)gs(0x1F01), (const char*)gs(0x1F02), (const char*)gs(0x1F00));
    }
    return GrGLMakeAssembledInterface(nullptr, egl_proc);
}

}  // namespace a380

#endif
