#pragma once

#include <string>

#include "a380/layout.h"
#include "include/core/SkCanvas.h"
#include "include/core/SkColor.h"

namespace a380::mfdx {

bool parse_css_color(const std::string& s, SkColor current, SkColor& out);
void paint_svg(SkCanvas& c, const layout::Node& svg, SkColor current_color, bool extended = false);

}  // namespace a380::mfdx
