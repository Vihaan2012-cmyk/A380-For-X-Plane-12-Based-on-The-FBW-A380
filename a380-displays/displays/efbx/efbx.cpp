#include <cstdlib>
#include <string>

#include "a380/display.h"
#include "mfdx/domdisplay.h"

namespace a380::mfdx {

namespace {

std::unique_ptr<Display> make_efbx(std::string_view variant) {
    DomConfig cfg;
    cfg.width = 1430;
    cfg.height = 1000;
    if (!variant.empty() && variant != "-") {
        std::string v(variant);
        int w = std::atoi(v.c_str());
        size_t x = v.find('x');
        int h = x == std::string::npos ? 0 : std::atoi(v.c_str() + x + 1);
        if (w > 0 && h > 0) {
            cfg.width = w;
            cfg.height = h;
        }
    }
    cfg.topic = "a380.efb.dom";
    cfg.scroll_topic = "a380.efb.scroll";
    cfg.sibling = "efb.dom.html";
    cfg.env = "EFBX_DOM";
    cfg.css_stacking = true;
    cfg.css_effects = true;
    cfg.class_backgrounds = {
        {"efb-bg", "", "cover", "center"},
    };
    return std::make_unique<DomDisplay>(std::move(cfg));
}

}  // namespace

A380_REGISTER_DISPLAY("efbx", make_efbx);

}  // namespace a380::mfdx
