#pragma once

#include "include/core/SkImageInfo.h"
#include "include/core/SkRefCnt.h"
#include "include/core/SkSurface.h"
#include "include/core/SkSurfaceProps.h"

#include <string>

namespace a380 {

sk_sp<SkSurface> make_surface(const SkImageInfo& info, const SkSurfaceProps* props);

bool gpu_available();

void flush_surface(SkSurface* surface, bool wait_for_gpu);

int gpu_share(int n);

bool gpu_shared();

sk_sp<SkSurface> make_shared_surface(const SkImageInfo& info, const SkSurfaceProps* props);

unsigned surface_texture(SkSurface* surface);

void* gpu_fence();

void gpu_wait(void* fence);

sk_sp<SkSurface> host_texture_surface(unsigned texture, int w, int h, const SkSurfaceProps* props, std::string* why);

void host_flush(SkSurface* surface);

/* GPU time between begin() and end() on the GL context current on this thread, from timestamp
   query pairs read back on later calls; report() returns mean ms and count since the last report. */
class GpuStopwatch {
public:
    void begin();
    void end();
    bool report(double* mean_ms, int* count);

private:
    void harvest();
    unsigned q_[8][2] = {};
    bool pending_[8] = {};
    int open_ = -1;
    int made_ = 0;
    double sum_ms_ = 0;
    int n_ = 0;
};

void host_save();

void host_restore();

void* host_gl();

void host_begin();

sk_sp<SkSurface> host_render_surface(int w, int h, const SkSurfaceProps* props, std::string* why);

void host_mipmaps(unsigned texture);

std::string host_texture_diag(unsigned texture, int w, int h);

}
