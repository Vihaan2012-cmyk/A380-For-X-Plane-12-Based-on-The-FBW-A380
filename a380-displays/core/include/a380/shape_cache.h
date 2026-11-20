#pragma once

#include <functional>
#include <string>

#include "include/core/SkCanvas.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPath.h"
#include "include/core/SkRect.h"

namespace a380 {

// A solid AA shape (`key`, drawn by `draw`) from a cached GPU coverage mask once seen twice; false means draw it directly.
bool draw_shape_cached(SkCanvas& canvas, const std::string& key, const SkRect& local_bounds, const SkPaint& paint,
                       const std::function<void(SkCanvas&, const SkPaint&)>& draw);

// drawPath through the cache for the paths that cost the most to draw directly.
void draw_path_cached(SkCanvas& canvas, const SkPath& path, const SkPaint& paint);

}  // namespace a380
