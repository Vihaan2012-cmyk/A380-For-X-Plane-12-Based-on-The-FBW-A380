#include "a380/layer.h"
#include "a380/env.h"

#include <cmath>
#include <cstdlib>

#include "include/core/SkImageFilter.h"
#include "include/core/SkMatrix.h"
#include "include/core/SkPaint.h"
#include "include/core/SkSamplingOptions.h"
#include "include/effects/SkImageFilters.h"

namespace a380 {

CompositedLayer::CompositedLayer(SkCanvas& canvas, const SkRect& b) : canvas_(canvas) {
    static const bool off = a380::getenv_cached("A380_LAYER_OFF") != nullptr;
    SkMatrix m = canvas.getLocalToDeviceAs3x3();
    float sx = m.getScaleX(), sy = m.getScaleY();
    if (off || sx <= 0.f || sy <= 0.f || m.getSkewX() != 0.f || m.getSkewY() != 0.f || m.hasPerspective() || b.isEmpty()) return;
    float lx = std::floor(b.fLeft), ly = std::floor(b.fTop);
    float px = m.getTranslateX() + sx * lx, py = m.getTranslateY() + sy * ly;
    float ix = std::floor(px), iy = std::floor(py);
    float fx = px - ix, fy = py - iy;
    active_ = true;
    canvas.save();
    canvas.setMatrix(SkMatrix::Translate(ix, iy).preScale(sx, sy).preTranslate(-lx, -ly));
    SkPaint p;
    p.setImageFilter(SkImageFilters::MatrixTransform(SkMatrix::Translate(fx / sx, fy / sy), SkSamplingOptions(SkFilterMode::kLinear), nullptr));
    canvas.saveLayer(SkCanvas::SaveLayerRec(nullptr, &p, 0));
}

CompositedLayer::~CompositedLayer() {
    if (!active_) return;
    canvas_.restore();
    canvas_.restore();
}

}  // namespace a380
