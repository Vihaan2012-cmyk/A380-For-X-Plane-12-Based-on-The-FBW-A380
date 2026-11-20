#pragma once

#include <vector>

#include "include/core/SkCanvas.h"
#include "include/core/SkMatrix.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPath.h"
#include "include/core/SkRect.h"

namespace a380 {

// Primitives that reproduce Chromium's edge coverage instead of Skia's default AA (see NOTES/text.md, raster lab).
// All take user-space coordinates and honour a uniform-scale + translate canvas matrix; the paint supplies colour,
// alpha and blend mode (style / width / cap are given explicitly or ignored).

void set_gpu_exact(bool on);

void chromium_stroke_circle(SkCanvas& canvas, float cx, float cy, float r, float width, const SkPaint& paint);
void chromium_fill_circle(SkCanvas& canvas, float cx, float cy, float r, const SkPaint& paint);
// Exact-area (signed accumulation) coverage of `path` mapped by `m` into a w x h float buffer (origin 0,0).
bool chromium_path_coverage(const SkPath& path, const SkMatrix& m, int w, int h, std::vector<float>& cov);
void chromium_fill_path(SkCanvas& canvas, const SkPath& path, const SkPaint& paint);
// Same as chromium_path_coverage but with the old Skia supersampling scan converter's arithmetic (4 sub-scanlines, x rounded to 1/4 px,
// 16 per covered sub-pixel, 64/63 per full pixel). Experimental; env A380_FILL_SAA=1 routes chromium_fill_path through it.
bool chromium_path_coverage_saa(const SkPath& path, const SkMatrix& m, int w, int h, std::vector<float>& cov);

void chromium_fill_polygon(SkCanvas& canvas, const SkPoint* pts, int count, const SkPaint& paint);
void chromium_stroke_line(SkCanvas& canvas, float x0, float y0, float x1, float y1, float width, const SkPaint& paint);
void chromium_stroke_line_dashed(SkCanvas& canvas, float x0, float y0, float x1, float y1, float width, const float* dashes,
                                 int count, float phase, const SkPaint& paint);
void chromium_fill_rect(SkCanvas& canvas, const SkRect& rect, const SkPaint& paint);
void chromium_stroke_rect(SkCanvas& canvas, const SkRect& rect, float width, const SkPaint& paint);
void chromium_stroke_path(SkCanvas& canvas, const SkPath& path, const SkPaint& paint);

// Circular arc the way Blink + Skia build it: split into cubics of at most 90 degrees (handles 4/3 tan(theta/4) * r) and
// flattened into 2^k-free uniform segments per cubic with Skia's cubic subdivision count at the canvas scale (error 0.17 px).
// `start` is the angle in radians of the first point (x = cx + r cos, y = cy + r sin, y down), `sweep` the signed sweep.
SkPath chromium_arc_path(float cx, float cy, float r, float start, float sweep, float device_scale);
// Strokes that arc through chromium_stroke_path; the paint supplies width, cap and join.
void chromium_stroke_arc(SkCanvas& canvas, float cx, float cy, float r, float start, float sweep, const SkPaint& paint);

}  // namespace a380
