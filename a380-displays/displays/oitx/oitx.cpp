#include <cmath>
#include <string>

#include "a380/display.h"
#include "a380/draw.h"
#include "a380/fonts.h"
#include "mfdx/domdisplay.h"

namespace a380::mfdx {

namespace {

class OitxDisplay : public DomDisplay {
public:
    OitxDisplay(DomConfig cfg, std::string variant) : DomDisplay(std::move(cfg)), variant_(std::move(variant)) {}

protected:
    int state(const Inputs& in) const override { return !unit_on(in) ? 1 : failed_shown(in) ? 2 : 0; }

    void overlay(SkCanvas& c, int mode) override {
        if (mode == 2) draw_unavailable(c);
    }

private:
    bool captain() const { return variant_ != "2"; }

    bool unit_on(const Inputs& in) const {
        auto bus = [&](const char* n) { return in.var(n, "Bool", 0) != 0; };
        bool powered = captain() ? bus("L:A32NX_ELEC_AC_2_BUS_IS_POWERED") || bus("L:A32NX_ELEC_DC_2_BUS_IS_POWERED")
                                 : bus("L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED") || bus("L:A32NX_ELEC_108PH_BUS_IS_POWERED") ||
                                       bus("L:A32NX_ELEC_DC_1_BUS_IS_POWERED");
        return powered && in.var(captain() ? "LIGHT POTENTIOMETER:78" : "LIGHT POTENTIOMETER:79", "number", 0) != 0;
    }

    // OitDisplayUnit shows "Not available." only in its On state; the published document says when.
    bool failed_shown(const Inputs& in) const {
        const nlohmann::json* dom = in.bus("a380.oit.dom." + variant_);
        if (!dom || !dom->is_string()) return ansu_failed(in);
        const std::string& s = dom->get_ref<const std::string&>();
        size_t at = s.find("class=\"AnsuFailed\"");
        if (at == std::string::npos) return ansu_failed(in);
        return s.find("display: block", at) < s.find('>', at);
    }

    bool ansu_failed(const Inputs& in) const {
        auto ok = [&](const char* n) { return in.var(n, "bool", 0) != 0; };
        bool nss_mode = in.var(captain() ? "L:A380X_SWITCH_OIT_SIDE_LEFT" : "L:A380X_SWITCH_OIT_SIDE_RIGHT", "bool", 0) != 0;
        if (nss_mode) return !ok("L:A32NX_NSS_ANSU_1_IS_HEALTHY") && !ok("L:A32NX_NSS_ANSU_2_IS_HEALTHY");
        return !ok("L:A32NX_FLTOPS_ANSU_1_IS_HEALTHY") ||
               !ok(captain() ? "L:A32NX_FLTOPS_LAPTOP_1_IS_HEALTHY" : "L:A32NX_FLTOPS_LAPTOP_2_IS_HEALTHY");
    }

    static void draw_unavailable(SkCanvas& c) {
        SkFont f = a380::font("OIT", 32.f, false, true);
        SkPaint p;
        p.setAntiAlias(true);
        p.setColor(0xFFE68000);
        draw_text(c, "Not available.", 666.5f, 500.f, f, p, Align::Center);
    }

    std::string variant_;
};

std::unique_ptr<Display> make_oitx(std::string_view variant) {
    std::string v = variant.empty() ? "1" : std::string(variant);
    layout::declare_font_weights("OIT", {400});
    DomConfig cfg;
    cfg.width = 1333;
    cfg.height = 1000;
    cfg.topic = "a380.oit.dom." + v;
    cfg.sibling = "oit-" + v + ".dom.html";
    cfg.env = "OITX_DOM";
    cfg.lcd_text = true;
    cfg.black_backdrop = true;
    cfg.css_stacking = true;
    cfg.class_backgrounds = {
        {"FbwTail", "Common/fbw-tail.png", "contain"},
        {"oit-avncs-navigator-folder-icon", "oit/folder.png", "contain"},
        {"oit-avncs-icon-folder", "oit/folder.png", "contain"},
        {"oit-a380x-systems-app-ldg-cap", "oit/a380x-systems-app-ldg-cap.png", "contain"},
    };
    return std::make_unique<OitxDisplay>(std::move(cfg), std::move(v));
}

}  // namespace

A380_REGISTER_DISPLAY("oitx", make_oitx);

}  // namespace a380::mfdx
