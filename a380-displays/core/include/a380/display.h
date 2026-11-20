#pragma once

#include <memory>
#include <string>
#include <string_view>
#include <vector>

#include "include/core/SkCanvas.h"
#include "include/core/SkSize.h"

#include "a380/inputs.h"

namespace a380 {

// One cockpit screen. `size()` is in CSS pixels, the coordinate space
// FlyByWire's page lays out in; the renderer scales by the device pixel ratio.
// `draw` paints everything the page paints, on a transparent canvas: where the
// cockpit shows black the page is transparent, exactly like the Chromium page.
class Display {
public:
    virtual ~Display() = default;
    virtual SkISize size() const = 0;
    virtual void draw(SkCanvas& canvas, const Inputs& in) = 0;
    // Border boxes (x, y, w, h in CSS px) of the document's elements in document order after the last
    // draw, before scroll offsets; nullptr for displays that do not lay out a document.
    virtual const std::vector<float>* element_boxes() const { return nullptr; }
};

using DisplayFactory = std::unique_ptr<Display> (*)(std::string_view variant);

// Displays register themselves with A380_REGISTER_DISPLAY from their own .cpp.
// Ids match the golden view names: "nd" (variant "1" captain, "2" first
// officer), "mfd", "pfd", "ewd", "sd", "sdv2", "isislegacy", "fcu", "rmp",
// "bat", "rtpi", "clock", "oit", "snd", "ife".
bool register_display(const char* id, DisplayFactory factory);
std::unique_ptr<Display> create_display(std::string_view id, std::string_view variant = {});
std::vector<std::string> display_ids();

}  // namespace a380

#define A380_CAT_(a, b) a##b
#define A380_CAT(a, b) A380_CAT_(a, b)
#define A380_REGISTER_DISPLAY(id, factory) \
    static const bool A380_CAT(a380_registered_, __COUNTER__) = ::a380::register_display(id, factory)
