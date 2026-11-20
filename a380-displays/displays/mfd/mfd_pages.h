#pragma once

#include <string_view>

#include "include/core/SkCanvas.h"
#include "include/core/SkRect.h"

#include "nlohmann/json.hpp"

namespace a380::mfd {

// The MFD draws from a page model: a JSON object that names what each page
// shows (values, colours, which fields are open or disabled). In X-Plane the
// Rust FMS publishes it on the bus topic "a380.mfd.model" with one object per
// side: {"uri": "fms/active/f-pln", "header": {...}, "page": {...}, "footer": {...}}.
// Offline, tools/mfd_model/ extracts the same model from a golden's mfd.dom.html.
//
// mfd.cpp (chrome owner) draws the header, title bar, footer and message line,
// then the page registered for the model's "uri" into `content` (CSS px,
// already offset for the side).
using PageDrawer = void (*)(SkCanvas& canvas, SkRect content, const nlohmann::json& page);

bool register_page(const char* uri_prefix, PageDrawer drawer);
PageDrawer find_page(std::string_view uri);

}  // namespace a380::mfd

#define A380_MFD_PAGE(uri, drawer) static const bool a380_mfd_page_##drawer = ::a380::mfd::register_page(uri, drawer)
