#pragma once

#include "include/core/SkCanvas.h"
#include "a380/shape_cache.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPath.h"
#include "include/core/SkRect.h"
#include "include/effects/SkDashPathEffect.h"

#include "a380/raster.h"

namespace a380::nd::prim {

enum Cls { kLine = 1, kStrokePath = 2, kFillPath = 4, kRect = 8, kCircle = 16, kCurves = 32 };

inline bool stroked(const SkPaint& p) { return p.getStyle() != SkPaint::kFill_Style; }

inline void line(int m, SkCanvas& c, float x0, float y0, float x1, float y1, const SkPaint& p) {
    if (!(m & kLine) || p.getPathEffect()) {
        if ((m & kLine) && p.getPathEffect()) {
            SkPaint q(p);
            q.setStyle(SkPaint::kStroke_Style);
            chromium_stroke_path(c, SkPath::Line({x0, y0}, {x1, y1}), q);
            return;
        }
        c.drawLine(x0, y0, x1, y1, p);
        return;
    }
    chromium_stroke_line(c, x0, y0, x1, y1, p.getStrokeWidth(), p);
}

inline void dashed(int m, SkCanvas& c, float x0, float y0, float x1, float y1, const float* iv, int n, const SkPaint& p) {
    SkPaint q(p);
    q.setPathEffect(nullptr);
    if (m & kLine) chromium_stroke_line_dashed(c, x0, y0, x1, y1, p.getStrokeWidth(), iv, n, 0.f, q);
    else {
        q.setPathEffect(SkDashPathEffect::Make({iv, n}, 0));
        c.drawLine(x0, y0, x1, y1, q);
    }
}

inline void path(int m, SkCanvas& c, const SkPath& pa, const SkPaint& p) {
    if (stroked(p)) {
        bool curved = pa.getSegmentMasks() & (SkPath::kConic_SegmentMask | SkPath::kQuad_SegmentMask | SkPath::kCubic_SegmentMask);
        if ((m & kStrokePath) && !p.getPathEffect() && (!curved || (m & kCurves))) {
            chromium_stroke_path(c, pa, p);
            return;
        }
    } else if (m & kFillPath) {
        chromium_fill_path(c, pa, p);
        return;
    }
    a380::draw_path_cached(c, pa, p);
}

inline void rect(int m, SkCanvas& c, const SkRect& r, const SkPaint& p) {
    if (!(m & kRect)) {
        c.drawRect(r, p);
        return;
    }
    if (stroked(p)) chromium_stroke_rect(c, r, p.getStrokeWidth(), p);
    else chromium_fill_rect(c, r, p);
}

inline void circle(int m, SkCanvas& c, float cx, float cy, float r, const SkPaint& p) {
    if (!(m & kCircle)) {
        c.drawCircle(cx, cy, r, p);
        return;
    }
    if (stroked(p)) chromium_stroke_circle(c, cx, cy, r, p.getStrokeWidth(), p);
    else chromium_fill_circle(c, cx, cy, r, p);
}

}  // namespace a380::nd::prim

