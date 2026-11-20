#include "a380/c_api.h"
#include "a380/env.h"

#include <algorithm>
#include <vector>
#include <cmath>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <deque>
#include <exception>
#include <memory>
#include <mutex>
#include <string>
#include <thread>

#include "include/core/SkCanvas.h"
#include "include/core/SkColor.h"
#include "include/core/SkImage.h"
#include "include/core/SkImageInfo.h"
#include "include/core/SkPixmap.h"
#include "include/core/SkRect.h"
#include "include/core/SkSamplingOptions.h"
#include "include/core/SkSurface.h"

#include "a380/assets.h"
#include "a380/display.h"
#include "a380/draw.h"
#include "a380/gpu.h"
#include "a380/profile.h"
#include "a380/input.h"
#include "a380/inputs.h"

struct a380d_display {
    std::thread::id owner = std::this_thread::get_id();
    std::unique_ptr<a380::Display> display;
    sk_sp<SkSurface> surface;
    sk_sp<SkSurface> ring[3];
    sk_sp<SkSurface> host;
    void* host_gl = nullptr;
    unsigned host_texture = 0;
    int host_draws = 0;
    std::string id;
    a380::GpuStopwatch gpu_watch;
    int gpu_draws = 0;
    int probes = 0;
    int ring_at = 0;
    int ring_w = 0;
    int ring_h = 0;
    int w = 0;
    int h = 0;
    bool drawn = false;
    uint64_t hash = 0;
    sk_sp<SkImage> underlay;
    std::mutex input_mutex;
    std::unique_ptr<a380::InputHandler> input;
    std::deque<a380::Write> writes;
};

struct a380d_inputs {
    a380::Inputs in;
};

namespace {

thread_local std::string g_error;

int fail(int code, const std::string& message) {
    g_error = message;
    return code;
}

bool on_owner_thread() {
    return true;
}

bool owns(const a380d_display* d) {
    return d->owner == std::this_thread::get_id();
}

uint64_t hash_rows(const uint8_t* p, int w, int h, int stride) {
    uint64_t a = 0xcbf29ce484222325ull;
    uint64_t b = 0x9e3779b97f4a7c15ull;
    const size_t row = (size_t)w * 4;
    for (int y = 0; y < h; y++) {
        const uint8_t* r = p + (size_t)y * (size_t)stride;
        size_t i = 0;
        for (; i + 8 <= row; i += 8) {
            uint64_t v;
            std::memcpy(&v, r + i, 8);
            a = (a ^ v) * 0x100000001b3ull;
            a ^= a >> 29;
            b = (b + v) * 0xff51afd7ed558ccdull;
            b ^= b >> 31;
        }
        for (; i < row; i++) a = (a ^ r[i]) * 0x100000001b3ull;
    }
    return a ^ (b * 0xc4ceb9fe1a85ec53ull);
}

double env_double(const char* key, double fallback) {
    const char* v = a380::getenv_cached(key);
    return v ? std::atof(v) : fallback;
}

template <class F>
int guarded(F&& f) {
    try {
        g_error.clear();
        return f();
    } catch (const std::exception& e) {
        return fail(-1, e.what());
    } catch (...) {
        return fail(-1, "unknown exception");
    }
}

}  // namespace

extern "C" {

const char* a380d_last_error(void) { return g_error.c_str(); }

int a380d_abi_version(void) { return A380D_ABI_VERSION; }

int a380d_set_asset_dir(const char* kind, const char* path) {
    return guarded([&] {
        if (!kind || !a380::assets::set(kind, path ? path : "")) return fail(-2, "unknown asset kind");
        return 0;
    });
}

int a380d_gpu_init(void) {
    return guarded([&] {
        if (!on_owner_thread()) return fail(-3, "a380d_gpu_init called from a second thread");
        return a380::gpu_available() ? 1 : 0;
    });
}

a380d_display* a380d_create(const char* id, const char* variant) {
    a380d_display* result = nullptr;
    guarded([&] {
        if (!id) return fail(-2, "null display id");
        if (!on_owner_thread()) return fail(-3, "a380d_create called from a thread other than the a380d_gpu_init thread");
        auto display = a380::create_display(id, variant ? variant : "");
        if (!display) return fail(-4, std::string("no display registered as '") + id + "' (link the library whole-archive so the displays register)");
        auto* d = new a380d_display();
        d->display = std::move(display);
        d->id = std::string(id) + (variant && *variant ? std::string("-") + variant : std::string());
        d->input = a380::make_input_handler(id, variant ? variant : "");
        result = d;
        return 0;
    });
    return result;
}

void a380d_destroy(a380d_display* display) {
    try {
        delete display;
    } catch (...) {
    }
}

void a380d_size(const a380d_display* display, int* css_w, int* css_h) {
    int w = 0, h = 0;
    try {
        if (display && display->display) {
            SkISize s = display->display->size();
            w = s.width();
            h = s.height();
        }
    } catch (...) {
    }
    if (css_w) *css_w = w;
    if (css_h) *css_h = h;
}

a380d_inputs* a380d_inputs_new(void) {
    try {
        return new a380d_inputs();
    } catch (...) {
        fail(-1, "out of memory");
        return nullptr;
    }
}

void a380d_inputs_free(a380d_inputs* inputs) {
    try {
        delete inputs;
    } catch (...) {
    }
}

int a380d_set_var(a380d_inputs* inputs, const char* name, const char* unit, double value) {
    return guarded([&] {
        if (!inputs || !name) return fail(-2, "null argument");
        inputs->in.set_var(name, unit ? unit : "", value);
        return 0;
    });
}

int a380d_set_string(a380d_inputs* inputs, const char* name, const char* utf8) {
    return guarded([&] {
        if (!inputs || !name) return fail(-2, "null argument");
        inputs->in.set_string(name, utf8 ? utf8 : "");
        return 0;
    });
}

int a380d_set_bus(a380d_inputs* inputs, const char* topic, const char* json) {
    return guarded([&] {
        if (!inputs || !topic || !json) return fail(-2, "null argument");
        auto parsed = nlohmann::json::parse(json, nullptr, false);
        if (parsed.is_discarded()) return fail(-5, std::string("bus '") + topic + "': payload is not valid JSON");
        inputs->in.set_bus(topic, std::move(parsed));
        return 0;
    });
}

int a380d_set_bus_string(a380d_inputs* inputs, const char* topic, const char* utf8, size_t len) {
    return guarded([&] {
        if (!inputs || !topic || (!utf8 && len)) return fail(-2, "null argument");
        inputs->in.set_bus(topic, nlohmann::json(std::string(utf8 ? utf8 : "", len)));
        return 0;
    });
}

int a380d_set_generic(a380d_inputs* inputs, const char* key, const char* json) {
    return guarded([&] {
        if (!inputs || !key || !json) return fail(-2, "null argument");
        auto parsed = nlohmann::json::parse(json, nullptr, false);
        if (parsed.is_discarded()) parsed = nlohmann::json(std::string(json));
        inputs->in.set_generic(key, std::move(parsed));
        return 0;
    });
}

int a380d_set_fetched(a380d_inputs* inputs, const char* url, const char* body, size_t len) {
    return guarded([&] {
        if (!inputs || !url || (!body && len)) return fail(-2, "null argument");
        inputs->in.set_fetched(url, std::string(body ? body : "", len));
        return 0;
    });
}

int a380d_set_env_number(a380d_inputs* inputs, const char* path, double value) {
    return guarded([&] {
        if (!inputs || !path) return fail(-2, "null argument");
        inputs->in.set_env_number(path, value);
        return 0;
    });
}

int a380d_set_now(a380d_inputs* inputs, double ms) {
    return guarded([&] {
        if (!inputs) return fail(-2, "null argument");
        inputs->in.set_now_ms(ms);
        return 0;
    });
}

int a380d_set_dpr(a380d_inputs* inputs, double dpr) {
    return guarded([&] {
        if (!inputs) return fail(-2, "null argument");
        if (!(dpr > 0.0)) return fail(-2, "dpr must be positive");
        inputs->in.set_dpr(dpr);
        return 0;
    });
}

namespace {

int prepare_surface(a380d_display* display, int w, int h) {
    if (display->surface && display->w == w && display->h == h) return 0;
    SkImageInfo info = SkImageInfo::MakeN32Premul(w, h);
    SkSurfaceProps props = a380::text_surface_props((float)env_double("A380_TEXT_CONTRAST", 1.0),
                                                   (float)env_double("A380_TEXT_GAMMA", 2.2));
    display->surface = a380::make_surface(info, &props);
    if (!display->surface) return fail(-7, "cannot create surface");
    display->w = w;
    display->h = h;
    display->drawn = false;
    return 0;
}

int check_size(const a380d_display* display, const a380d_inputs* inputs, int w, int h, double* dpr_out) {
    const double dpr = inputs->in.device_pixel_ratio();
    const SkISize css = display->display->size();
    const int ew = (int)std::lround(css.width() * dpr);
    const int eh = (int)std::lround(css.height() * dpr);
    if (w != ew || h != eh)
        return fail(-6, "buffer is " + std::to_string(w) + "x" + std::to_string(h) + ", display needs " + std::to_string(ew) + "x" + std::to_string(eh));
    *dpr_out = dpr;
    return 0;
}

bool page_transparent() {
    static const bool t = env_double("A380_PAGE_TRANSPARENT", 0.0) != 0.0;
    return t;
}

SkColor page_clear() { return page_transparent() ? SK_ColorTRANSPARENT : SK_ColorBLACK; }

struct TargetScope {
    TargetScope() { a380::set_transparent_target(page_transparent()); }
    ~TargetScope() { a380::set_transparent_target(false); }
};

void draw_layer(a380d_display* display, const a380d_inputs* inputs, SkCanvas* canvas, int w, int h, double dpr) {
    TargetScope target;
    if (display->input) {
        std::lock_guard<std::mutex> lock(display->input_mutex);
        display->input->observe(inputs->in);
    }
    if (display->underlay) canvas->drawImageRect(display->underlay, SkRect::MakeWH((float)w, (float)h), SkSamplingOptions(SkFilterMode::kLinear));
    canvas->save();
    canvas->scale((float)dpr, (float)dpr);
    display->display->draw(*canvas, inputs->in);
    canvas->restore();
}

int finish_draw(a380d_display* display, uint8_t* rgba, int w, int h, int stride) {
    SkImageInfo out = SkImageInfo::Make(w, h, kRGBA_8888_SkColorType, kPremul_SkAlphaType);
    if (!display->surface->readPixels(out, rgba, (size_t)stride, 0, 0)) return fail(-8, "readPixels failed");
    uint64_t hash = hash_rows(rgba, w, h, stride);
    bool changed = !display->drawn || hash != display->hash;
    display->drawn = true;
    display->hash = hash;
    return changed ? 1 : 0;
}

}  // namespace

int a380d_draw(a380d_display* display, const a380d_inputs* inputs, uint8_t* rgba, int w, int h, int stride) {
    return guarded([&] {
        if (!display || !display->display || !inputs || !rgba) return fail(-2, "null argument");
        if (!owns(display)) return fail(-3, "a380d_draw called from a thread other than the one that created the display");
        double dpr = 1.0;
        if (int rc = check_size(display, inputs, w, h, &dpr)) return rc;
        if (stride < w * 4) return fail(-6, "stride smaller than width * 4");
        if (int rc = prepare_surface(display, w, h)) return rc;
        SkCanvas* canvas = display->surface->getCanvas();
        canvas->clear(page_clear());
        draw_layer(display, inputs, canvas, w, h, dpr);
        return finish_draw(display, rgba, w, h, stride);
    });
}

int a380d_draw_layers(a380d_display* const* displays, const a380d_inputs* const* inputs, int n, uint8_t* rgba, int w, int h, int stride) {
    return guarded([&] {
        if (!displays || !inputs || !rgba || n < 1) return fail(-2, "null argument");
        if (!on_owner_thread()) return fail(-3, "a380d_draw_layers called from a thread other than the a380d_gpu_init thread");
        double dpr = 1.0;
        for (int i = 0; i < n; i++) {
            if (!displays[i] || !displays[i]->display || !inputs[i]) return fail(-2, "null layer");
            if (!owns(displays[i])) return fail(-3, "a380d_draw_layers called from a thread other than the one that created the display");
            double d = 1.0;
            if (int rc = check_size(displays[i], inputs[i], w, h, &d)) return rc;
            if (i && d != dpr) return fail(-6, "layers have different dpr");
            dpr = d;
        }
        if (stride < w * 4) return fail(-6, "stride smaller than width * 4");
        a380d_display* base = displays[0];
        if (int rc = prepare_surface(base, w, h)) return rc;
        SkCanvas* canvas = base->surface->getCanvas();
        canvas->clear(page_clear());
        for (int i = 0; i < n; i++) draw_layer(displays[i], inputs[i], canvas, w, h, dpr);
        return finish_draw(base, rgba, w, h, stride);
    });
}

int a380d_gpu_share(int n) {
    return guarded([&] {
        int made = a380::gpu_share(n);
        if (made < 0) return fail(-10, "no current GL context to share");
        return made;
    });
}

int a380d_gpu_shared(void) {
    return guarded([&] { return a380::gpu_shared() ? 1 : 0; });
}

// How much of a finished frame is lit: the first few GPU frames of each display, to tell a dark page from a lost texture.
static void probe_lit(a380d_display* d, SkSurface* surface, int w, int h) {
    d->probes++;
    std::vector<uint8_t> px((size_t)w * (size_t)h * 4);
    SkImageInfo info = SkImageInfo::Make(w, h, kRGBA_8888_SkColorType, kPremul_SkAlphaType);
    if (!surface->readPixels(info, px.data(), (size_t)w * 4, 0, 0)) return;
    size_t lit = 0;
    for (size_t i = 0; i < px.size(); i += 4)
        if (std::max({px[i], px[i + 1], px[i + 2]}) > 40) lit++;
    char line[160];
    std::snprintf(line, sizeof line, "lit %s: %.1f%% of %dx%d", d->id.c_str(), 100.0 * (double)lit / ((double)w * h), w, h);
    a380::profile_emit(line);
}

int a380d_draw_layers_gpu(a380d_display* const* displays, const a380d_inputs* const* inputs, int n, int w, int h, unsigned* texture, void** fence) {
    return guarded([&] {
        if (!displays || !inputs || !texture || !fence || n < 1) return fail(-2, "null argument");
        double dpr = 1.0;
        for (int i = 0; i < n; i++) {
            if (!displays[i] || !displays[i]->display || !inputs[i]) return fail(-2, "null layer");
            if (!owns(displays[i])) return fail(-3, "a380d_draw_layers_gpu called from a thread other than the one that created the display");
            double d = 1.0;
            if (int rc = check_size(displays[i], inputs[i], w, h, &d)) return rc;
            if (i && d != dpr) return fail(-6, "layers have different dpr");
            dpr = d;
        }
        a380d_display* base = displays[0];
        if (base->ring_w != w || base->ring_h != h) {
            for (auto& s : base->ring) s.reset();
            base->ring_w = w;
            base->ring_h = h;
        }
        base->ring_at = (base->ring_at + 1) % 3;
        sk_sp<SkSurface>& surface = base->ring[base->ring_at];
        if (!surface) {
            SkImageInfo info = SkImageInfo::Make(w, h, kRGBA_8888_SkColorType, kPremul_SkAlphaType);
            SkSurfaceProps props = a380::text_surface_props((float)env_double("A380_TEXT_CONTRAST", 1.0),
                                                           (float)env_double("A380_TEXT_GAMMA", 2.2));
            surface = a380::make_shared_surface(info, &props);
            if (!surface) return fail(-7, "cannot create shared GPU surface");
        }
        SkCanvas* canvas = surface->getCanvas();
        base->gpu_watch.begin();
        canvas->clear(page_clear());
        for (int i = 0; i < n; i++) draw_layer(displays[i], inputs[i], canvas, w, h, dpr);
        a380::flush_surface(surface.get(), false);
        base->gpu_watch.end();
        if (base->probes < 3 && (base->gpu_draws == 30 || base->gpu_draws == 299)) probe_lit(base, surface.get(), w, h);
        if (++base->gpu_draws >= 300) {
            base->gpu_draws = 0;
            double ms = 0;
            int count = 0;
            if (base->gpu_watch.report(&ms, &count)) {
                char line[160];
                std::snprintf(line, sizeof line, "gpu %s: %.3f ms per draw over %d draws", base->id.c_str(), ms, count);
                a380::profile_emit(line);
            }
        }
        *texture = a380::surface_texture(surface.get());
        if (!*texture) return fail(-8, "shared surface has no GL texture");
        *fence = a380::gpu_fence();
        base->drawn = true;
        return 1;
    });
}

static std::mutex g_diag_mutex;
static std::string g_diag;

int a380d_host_diag(char* out, int cap) {
    std::lock_guard<std::mutex> lock(g_diag_mutex);
    g_diag += a380::take_profile();
    if (g_diag.empty() || !out || cap < 2) return 0;
    int n = (int)std::min<size_t>(g_diag.size(), (size_t)cap - 1);
    memcpy(out, g_diag.data(), n);
    out[n] = 0;
    g_diag.clear();
    return n;
}

int a380d_draw_layers_host(a380d_display* const* displays, const a380d_inputs* const* inputs, int n, int w, int h, unsigned* texture) {
    return guarded([&] {
        if (!displays || !inputs || !texture || n < 1) return fail(-2, "null argument");
        double dpr = 1.0;
        for (int i = 0; i < n; i++) {
            if (!displays[i] || !displays[i]->display || !inputs[i]) return fail(-2, "null layer");
            double d = 1.0;
            if (int rc = check_size(displays[i], inputs[i], w, h, &d)) return rc;
            if (i && d != dpr) return fail(-6, "layers have different dpr");
            dpr = d;
        }
        SkSurfaceProps props = a380::text_surface_props((float)env_double("A380_TEXT_CONTRAST", 1.0), (float)env_double("A380_TEXT_GAMMA", 2.2));
        a380d_display* own = displays[0];
        a380::host_save();
        struct Restore {
            ~Restore() { a380::host_restore(); }
        } restore;
        void* gl = a380::host_gl();
        if (!own->host || own->host_gl != gl || own->host->width() != w || own->host->height() != h) {
            std::string why;
            own->host = a380::host_render_surface(w, h, &props, &why);
            own->host_gl = gl;
            own->host_texture = own->host ? a380::surface_texture(own->host.get()) : 0;
            if (!own->host || !own->host_texture) {
                own->host = nullptr;
                return fail(-7, ("cannot make the screen texture: " + why).c_str());
            }
        }
        SkSurface* surface = own->host.get();
        a380::host_begin();
        SkCanvas* canvas = surface->getCanvas();
        canvas->clear(page_clear());
        for (int i = 0; i < n; i++) draw_layer(displays[i], inputs[i], canvas, w, h, dpr);
        a380::host_flush(surface);
        a380::host_mipmaps(own->host_texture);
        own->host_draws++;
        if (own->host_draws == 1 || own->host_draws == 60) {
            std::string d = a380::host_texture_diag(own->host_texture, w, h);
            std::lock_guard<std::mutex> lock(g_diag_mutex);
            g_diag += "draw " + std::to_string(own->host_draws) + " " + std::to_string(w) + "x" + std::to_string(h) + ": " + d + "\n";
        }
        *texture = own->host_texture;
        own->drawn = true;
        return 1;
    });
}

int a380d_gpu_wait(void* fence) {
    return guarded([&] {
        a380::gpu_wait(fence);
        return 0;
    });
}

int a380d_set_underlay(a380d_display* display, const uint8_t* rgba, int w, int h, int stride) {
    return guarded([&] {
        if (!display) return fail(-2, "null argument");
        if (!rgba) {
            display->underlay.reset();
            return 0;
        }
        if (w <= 0 || h <= 0 || stride < w * 4) return fail(-6, "bad underlay size");
        SkImageInfo info = SkImageInfo::Make(w, h, kRGBA_8888_SkColorType, kUnpremul_SkAlphaType);
        auto image = SkImages::RasterFromPixmapCopy(SkPixmap(info, rgba, (size_t)stride));
        if (!image) return fail(-9, "cannot copy underlay");
        display->underlay = std::move(image);
        return 0;
    });
}

int a380d_pointer(a380d_display* display, double x, double y, int kind, int button) {
    return guarded([&] {
        if (!display || !display->input) return fail(-2, "null argument");
        if (kind < A380D_POINTER_DOWN || kind > A380D_POINTER_WHEEL) return fail(-2, "unknown pointer kind");
        std::lock_guard<std::mutex> lock(display->input_mutex);
        std::vector<a380::Write> out;
        bool took = display->input->pointer(x, y, (a380::PointerKind)kind, button, out);
        display->writes.insert(display->writes.end(), out.begin(), out.end());
        return took ? 1 : 0;
    });
}

int a380d_key(a380d_display* display, int key, int mods, int down) {
    return guarded([&] {
        if (!display || !display->input) return fail(-2, "null argument");
        std::lock_guard<std::mutex> lock(display->input_mutex);
        std::vector<a380::Write> out;
        bool took = display->input->key(key, mods, down != 0, out);
        display->writes.insert(display->writes.end(), out.begin(), out.end());
        return took ? 1 : 0;
    });
}

int a380d_take_writes(a380d_display* display, a380d_write* out, int cap) {
    return guarded([&] {
        if (!display || (!out && cap > 0)) return fail(-2, "null argument");
        std::lock_guard<std::mutex> lock(display->input_mutex);
        int n = 0;
        while (n < cap && !display->writes.empty()) {
            const a380::Write& w = display->writes.front();
            a380d_write& o = out[n++];
            std::memset(&o, 0, sizeof o);
            o.kind = w.kind;
            std::strncpy(o.name, w.name.c_str(), sizeof o.name - 1);
            o.value = w.value;
            o.delay_ms = w.delay_ms;
            display->writes.pop_front();
        }
        return n;
    });
}

int a380d_boxes(const a380d_display* display, float* xywh, int cap) {
    return guarded([&] {
        if (!display || !display->display) return fail(-2, "null display");
        const std::vector<float>* b = display->display->element_boxes();
        if (!b) return 0;
        int n = (int)(b->size() / 4);
        if (xywh && cap > 0) std::memcpy(xywh, b->data(), sizeof(float) * 4 * (size_t)std::min(n, cap));
        return n;
    });
}

int a380d_input_native(const a380d_display* display) {
    try {
        return display && display->input && display->input->owns_input() ? 1 : 0;
    } catch (...) {
        return 0;
    }
}

}
