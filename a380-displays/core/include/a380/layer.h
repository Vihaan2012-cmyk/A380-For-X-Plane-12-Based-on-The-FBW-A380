#pragma once

#include "include/core/SkCanvas.h"
#include "include/core/SkRect.h"

namespace a380 {

// A Chromium composited layer (an SVG element with a CSS 3D transform / will-change): the layer is rasterised on a pixel
// grid anchored at its origin, which is the layer bounds rounded out to whole user units, and then drawn bilinearly at the
// origin's fractional device position. Construct it with the canvas matrix of the element's local space already applied
// (after any translate for the element's transform) and the element's visual bounds in that space (stroke included);
// everything drawn until the object goes out of scope is composited that way. Needs a uniform-scale + translate matrix,
// otherwise it draws straight through.
class CompositedLayer {
public:
    CompositedLayer(SkCanvas& canvas, const SkRect& user_bounds);
    ~CompositedLayer();
    CompositedLayer(const CompositedLayer&) = delete;
    CompositedLayer& operator=(const CompositedLayer&) = delete;

private:
    SkCanvas& canvas_;
    bool active_ = false;
};

}  // namespace a380
