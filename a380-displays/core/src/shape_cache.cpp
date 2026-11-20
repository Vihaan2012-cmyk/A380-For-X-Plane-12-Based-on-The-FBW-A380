#include "a380/shape_cache.h"

#include <cmath>
#include <cstring>
#include <unordered_map>

#include "a380/env.h"
#include "include/core/SkImage.h"
#include "include/core/SkPathEffect.h"
#include "include/core/SkSurface.h"
#include "src/core/SkPathEffectBase.h"

#ifdef A380_GPU
#include "include/gpu/ganesh/GrRecordingContext.h"
#include "include/gpu/ganesh/SkSurfaceGanesh.h"
#endif

namespace a380 {

namespace {

template <class T>
void put(std::string& k, const T& v) {
    k.append(reinterpret_cast<const char*>(&v), sizeof v);
}

struct Mask {
    sk_sp<SkImage> image;
    int left = 0, top = 0;
    size_t bytes = 0;
};

struct Cache {
    std::unordered_map<std::string, Mask> masks;
    std::unordered_map<std::string, int> seen;
    size_t bytes = 0;
};

constexpr int kMaxSide = 1024;
constexpr size_t kBudget = 48u << 20;

// A dash is the one path effect allowed: its intervals and phase go in the key.
bool simple_paint(const SkPaint& p, std::string* dash) {
    if (!p.isAntiAlias() || p.getShader() || p.getColorFilter() || p.getMaskFilter() || p.getImageFilter() ||
        p.asBlendMode() != SkBlendMode::kSrcOver || p.getStyle() == SkPaint::kStrokeAndFill_Style)
        return false;
    if (!p.getPathEffect()) return true;
    auto info = as_PEB(p.getPathEffect())->asADash();
    if (!info) return false;
    dash->push_back('D');
    put(*dash, info->fPhase);
    for (SkScalar v : info->fIntervals) put(*dash, v);
    return true;
}

bool enabled() {
    static const bool on = [] {
        const char* v = getenv_cached("A380_SHAPE_CACHE");
        return !v || std::strcmp(v, "0") != 0;
    }();
    return on;
}

// Paths Ganesh triangulates every frame: stroked curves or over 10 verbs, up to the 100 past which it rasterizes on the CPU.
bool worth_caching(const SkPath& path, const SkPaint& paint) {
    if (path.countVerbs() > 100) return false;
    if (path.countVerbs() > 10) return true;
    if (paint.getStyle() != SkPaint::kStroke_Style) return false;
    for (SkPathVerb v : path.verbs())
        if (v == SkPathVerb::kQuad || v == SkPathVerb::kConic || v == SkPathVerb::kCubic) return true;
    return false;
}

}  // namespace

bool draw_shape_cached(SkCanvas& canvas, const std::string& key, const SkRect& local_bounds, const SkPaint& paint,
                       const std::function<void(SkCanvas&, const SkPaint&)>& draw) {
#ifdef A380_GPU
    GrRecordingContext* ctx = canvas.recordingContext();
    std::string dash;
    if (!ctx || !enabled() || !simple_paint(paint, &dash) || !paint.canComputeFastBounds()) return false;
    const SkMatrix m = canvas.getLocalToDeviceAs3x3();
    if (m.hasPerspective()) return false;
    SkRect storage;
    const SkRect dev = m.mapRect(paint.computeFastBounds(local_bounds, &storage));
    if (!dev.isFinite() || dev.width() > kMaxSide || dev.height() > kMaxSide) return false;
    const float tx = m.getTranslateX(), ty = m.getTranslateY();
    const float ix = std::floor(tx), iy = std::floor(ty);
    std::string k = key + dash;
    put(k, m.getScaleX()), put(k, m.getSkewX()), put(k, m.getSkewY()), put(k, m.getScaleY());
    put(k, tx - ix), put(k, ty - iy);
    put(k, (uint8_t)paint.getStyle()), put(k, paint.getStrokeWidth()), put(k, (uint8_t)paint.getStrokeCap());
    put(k, (uint8_t)paint.getStrokeJoin()), put(k, paint.getStrokeMiter());
    thread_local Cache cache;
    const Mask* mask = nullptr;
    if (auto hit = cache.masks.find(k); hit != cache.masks.end()) {
        mask = &hit->second;
    } else {
        int& count = cache.seen[k];
        if (++count < 2) {
            if (cache.seen.size() > 16384) cache.seen.clear();
            return false;
        }
        cache.seen.erase(k);
        const int left = (int)std::floor(dev.fLeft - ix) - 2, top = (int)std::floor(dev.fTop - iy) - 2;
        const int w = (int)std::ceil(dev.fRight - ix) + 2 - left, h = (int)std::ceil(dev.fBottom - iy) + 2 - top;
        if (w <= 0 || h <= 0) return false;
        sk_sp<SkSurface> surface = SkSurfaces::RenderTarget(ctx, skgpu::Budgeted::kYes, SkImageInfo::MakeA8(w, h));
        if (!surface) return false;
        SkCanvas* mc = surface->getCanvas();
        mc->clear(SK_ColorTRANSPARENT);
        SkMatrix mm = m;
        mm.setTranslateX(tx - ix - (float)left);
        mm.setTranslateY(ty - iy - (float)top);
        mc->setMatrix(mm);
        SkPaint white(paint);
        white.setColor(SK_ColorWHITE);
        draw(*mc, white);
        Mask made;
        made.image = surface->makeImageSnapshot();
        if (!made.image) return false;
        made.left = left;
        made.top = top;
        made.bytes = (size_t)w * (size_t)h + k.size() + 64;
        if (cache.bytes + made.bytes > kBudget) {
            cache.masks.clear();
            cache.bytes = 0;
        }
        cache.bytes += made.bytes;
        mask = &cache.masks.emplace(std::move(k), std::move(made)).first->second;
    }
    SkPaint p;
    p.setColor4f(paint.getColor4f());
    canvas.save();
    canvas.resetMatrix();
    canvas.drawImage(mask->image, ix + (float)mask->left, iy + (float)mask->top, SkSamplingOptions(), &p);
    canvas.restore();
    return true;
#else
    (void)canvas, (void)key, (void)local_bounds, (void)paint, (void)draw;
    return false;
#endif
}

void draw_path_cached(SkCanvas& canvas, const SkPath& path, const SkPaint& paint) {
    if (path.isInverseFillType() || !worth_caching(path, paint)) {
        canvas.drawPath(path, paint);
        return;
    }
    std::string k;
    const auto verbs = path.verbs();
    const auto pts = path.points();
    const auto w = path.conicWeights();
    k.reserve(16 + verbs.size() + pts.size() * sizeof(SkPoint) + w.size() * sizeof(float));
    k.push_back('P');
    put(k, (uint8_t)path.getFillType());
    put(k, (uint32_t)verbs.size());
    k.append(reinterpret_cast<const char*>(verbs.data()), verbs.size() * sizeof(SkPathVerb));
    k.append(reinterpret_cast<const char*>(pts.data()), pts.size() * sizeof(SkPoint));
    k.append(reinterpret_cast<const char*>(w.data()), w.size() * sizeof(float));
    if (!draw_shape_cached(canvas, k, path.getBounds(), paint, [&](SkCanvas& c, const SkPaint& p) { c.drawPath(path, p); }))
        canvas.drawPath(path, paint);
}

}  // namespace a380
