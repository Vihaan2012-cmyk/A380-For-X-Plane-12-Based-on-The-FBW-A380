#include "domdisplay.h"

namespace a380::mfdx {

namespace {

std::unique_ptr<Display> make_mfdx(std::string_view) {
    DomConfig cfg;
    cfg.topic = "a380.mfd.dom";
    cfg.sibling = "mfd.dom.html";
    cfg.env = "MFDX_DOM";
    cfg.model_topic = "a380.mfd.model";
    cfg.css_stacking = true;
    cfg.css_fidelity = true;
    cfg.scrollbar_cls = "mfd-dropdown-menu";
    cfg.scrollbar_track = 0xFF272525;
    cfg.scrollbar_thumb = 0xFFC4C6CF;
    cfg.scrollbar_width = 20.f;
    return std::make_unique<DomDisplay>(std::move(cfg));
}

}  // namespace

A380_REGISTER_DISPLAY("mfdx", make_mfdx);

}  // namespace a380::mfdx
