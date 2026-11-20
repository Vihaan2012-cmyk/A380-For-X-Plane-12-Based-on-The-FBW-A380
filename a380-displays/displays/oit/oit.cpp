#include <memory>
#include "a380/env.h"
#include <string>

#include <cstdlib>

#include "a380/display.h"
#include "a380/draw.h"
#include "a380/fonts.h"
#include "include/core/SkImage.h"
#include "include/core/SkSurface.h"
#include "oit.h"

namespace {

using nlohmann::json;
using namespace a380::oit;

class Oit : public a380::Display {
public:
    explicit Oit(std::string v) : variant_(std::move(v)) {}
    SkISize size() const override { return {1333, 1000}; }

    void draw(SkCanvas& canvas, const a380::Inputs& in) override {
        float dpr = (float)in.device_pixel_ratio();
        int w = (int)std::lround(1333 * dpr), h = (int)std::lround(1000 * dpr);
        const json* model = in.bus("a380.mfd.model");
        int mode = !unit_on(in) ? 0 : ansu_failed(in) ? 1 : 2;
        if (!cached_ || cached_mode_ != mode || cached_dpr_ != dpr || cached_has_model_ != (model != nullptr) || (model && cached_model_ != *model)) {
            static const float contrast = a380::getenv_cached("A380_OIT_CONTRAST") ? (float)std::atof(a380::getenv_cached("A380_OIT_CONTRAST")) : 1.0f;
            SkSurfaceProps props = a380::text_surface_props(contrast, a380::getenv_cached("A380_OIT_GAMMA") ? (float)std::atof(a380::getenv_cached("A380_OIT_GAMMA")) : 2.2f);
            sk_sp<SkSurface> surface = SkSurfaces::Raster(SkImageInfo::MakeN32Premul(w, h), &props);
            if (!surface) {
                cached_ = nullptr;
                draw_page(canvas, in);
                return;
            }
            SkCanvas* c = surface->getCanvas();
            c->scale(dpr, dpr);
            draw_page(*c, in);
            cached_ = surface->makeImageSnapshot();
            cached_dpr_ = dpr;
            cached_mode_ = mode;
            cached_has_model_ = model != nullptr;
            if (model) cached_model_ = *model;
            else cached_model_ = json();
        }
        canvas.save();
        canvas.resetMatrix();
        SkPaint p;
        p.setBlendMode(SkBlendMode::kSrc);
        canvas.drawImage(cached_, 0, 0, SkSamplingOptions(), &p);
        canvas.restore();
    }

private:
    bool captain() const { return variant_ != "2"; }

    bool unit_on(const a380::Inputs& in) const {
        auto bus = [&](const char* n) { return in.var(n, "Bool", 0) != 0; };
        bool powered = captain() ? bus("L:A32NX_ELEC_AC_2_BUS_IS_POWERED") || bus("L:A32NX_ELEC_DC_2_BUS_IS_POWERED")
                                 : bus("L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED") || bus("L:A32NX_ELEC_108PH_BUS_IS_POWERED") ||
                                       bus("L:A32NX_ELEC_DC_1_BUS_IS_POWERED");
        return powered && in.var(captain() ? "LIGHT POTENTIOMETER:78" : "LIGHT POTENTIOMETER:79", "number", 0) != 0;
    }

    bool ansu_failed(const a380::Inputs& in) const {
        auto ok = [&](const char* n) { return in.var(n, "bool", 0) != 0; };
        bool nss_mode = in.var(captain() ? "L:A380X_SWITCH_OIT_SIDE_LEFT" : "L:A380X_SWITCH_OIT_SIDE_RIGHT", "bool", 0) != 0;
        if (nss_mode) return !ok("L:A32NX_NSS_ANSU_1_IS_HEALTHY") && !ok("L:A32NX_NSS_ANSU_2_IS_HEALTHY");
        return !ok("L:A32NX_FLTOPS_ANSU_1_IS_HEALTHY") ||
               !ok(captain() ? "L:A32NX_FLTOPS_LAPTOP_1_IS_HEALTHY" : "L:A32NX_FLTOPS_LAPTOP_2_IS_HEALTHY");
    }

    void draw_unavailable(SkCanvas& c) {
        Text t;
        t.size = 32;
        t.color = 0xFFE68000;
        t.align = Align::Center;
        SkFontMetrics fm;
        a380::font("OIT", t.size, false, true).getMetrics(&fm);
        float asc = std::round(-fm.fAscent * dpr()) / dpr(), desc = std::round(fm.fDescent * dpr()) / dpr();
        t.line_height = asc + desc;
        text(c, "Not available.", 666.5f, 500.f - asc, t);
    }

    void draw_page(SkCanvas& c, const a380::Inputs& in) {
        set_dpr((float)in.device_pixel_ratio());
        fill(c, SkRect::MakeWH(1333, 1000), pal::black);
        if (!unit_on(in)) return;
        if (ansu_failed(in)) {
            draw_unavailable(c);
            return;
        }
        const json* model = in.bus("a380.mfd.model");
        if (!model || !model->is_object()) return;
        if (model->contains("login") && (*model)["login"].is_object()) draw_login(c, (*model)["login"]);
        else if (model->contains("loading") && (*model)["loading"].is_object()) draw_loading(c, (*model)["loading"]);
        else if (model->contains("aoc") && (*model)["aoc"].is_object()) draw_aoc(c, (*model)["aoc"]);
        if (model->contains("menus") && (*model)["menus"].is_array()) draw_menus(c, (*model)["menus"]);
    }

    std::string variant_;
    sk_sp<SkImage> cached_;
    json cached_model_;
    float cached_dpr_ = 0.f;
    int cached_mode_ = -1;
    bool cached_has_model_ = false;
};

std::unique_ptr<a380::Display> make(std::string_view v) { return std::make_unique<Oit>(std::string(v)); }

}  // namespace

A380_REGISTER_DISPLAY("oit", make);
