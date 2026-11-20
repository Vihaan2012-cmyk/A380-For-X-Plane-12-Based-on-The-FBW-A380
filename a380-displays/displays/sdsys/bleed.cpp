#include <algorithm>
#include <optional>
#include <string>

#include "include/core/SkSamplingOptions.h"

#include "include/effects/SkImageFilters.h"

#include "sdsys.h"

namespace sdsys {
namespace {

struct Layer3d {
    SkCanvas& cv;
    Layer3d(SkCanvas& c, float x, float y, int) : cv(c) {
        cv.save();
        SkMatrix m = cv.getTotalMatrix();
        float sx = m.getScaleX(), sy = m.getScaleY();
        float ox = m.getTranslateX() + sx * x, oy = m.getTranslateY() + sy * y;
        float ix = ox, iy = std::floor(oy);
        cv.setMatrix(SkMatrix::Translate(ix, iy).preScale(sx, sy));
        SkPaint p;
        p.setImageFilter(SkImageFilters::MatrixTransform(SkMatrix::Translate((ox - ix) / sx, (oy - iy) / sy),
                                                         SkSamplingOptions(SkFilterMode::kLinear), nullptr));
        cv.saveLayer(SkCanvas::SaveLayerRec(nullptr, &p, 0));
    }
    ~Layer3d() {
        cv.restore();
        cv.restore();
    }
};

constexpr auto End = a380::Align::Right;

void path(SkCanvas& cv, const char* d, uint32_t c, float w = 2.f) { draw_path(cv, svg_path(d), sk_stroke(c, w)); }

bool agsBit(const a380::Inputs& in, int bit) {
    for (int i = 1; i <= 4; i++) {
        std::string n = "L:A32NX_COND_CPIOM_B" + std::to_string(i) + "_AGS_DISCRETE_WORD";
        A429 a = var429(in, n);
        if (a.ssm == 3 || a.ssm == 2) return bit429(in, n, bit);
    }
    return false;
}

bool tcsBit(const a380::Inputs& in, int bit) {
    for (int i = 1; i <= 4; i++) {
        std::string n = "L:A32NX_COND_CPIOM_B" + std::to_string(i) + "_TCS_DISCRETE_WORD";
        A429 a = var429(in, n);
        if (a.ssm == 3 || a.ssm == 2) return bit429(in, n, bit);
    }
    return false;
}

void pack_gauge(SkCanvas& cv, float x, float y, int engine, bool open, double rate) {
    const float r = 41;
    uint32_t c = open ? kGreen : kAmber;
    {
        Layer3d layer(cv, x, y, 0);
        path(cv, engine % 2 == 0 ? "M-1,-51 l 0,-13 l -69,0" : "M-1,-51 l 0,-13 l 69,0", c);
        arc_path(cv, 0, 0, r, -90, 90, kWhite, 2.f);
        for (float val : {0.f, 0.5f, 1.f}) gauge_marker(cv, 0, 0, val, 0, 1, r, -90, 90, kWhite, 2.f, false);
        gauge_marker(cv, 0, 0, (float)rate, 0, 1, r, -90, 90, kGreen, 3.f, true, true, true, 0.47f, 1.15f);
    }
    cv.save();
    cv.translate(x, y);
    valve(cv, 0, 0, 19.5f, open, c);
    cv.restore();
}

void bleed_engine(SkCanvas& cv, const a380::Inputs& in, float x, float y, int engine) {
    std::string e = std::to_string(engine);
    int packIndex = engine > 2 ? 2 : 1;
    int flowIdx = engine > 2 ? engine - 2 : engine;
    bool running = v(in, "L:A32NX_ENGINE_STATE:" + e) == 1;
    bool bleedOpen = b(in, "L:A32NX_PNEU_ENG_" + e + "_PR_VALVE_OPEN");
    bool hpOpen = b(in, "L:A32NX_PNEU_ENG_" + e + "_HP_VALVE_OPEN");
    double tout = v(in, "L:A32NX_PNEU_ENG_" + e + "_PRECOOLER_OUTLET_TEMPERATURE");
    int tout5 = (int)(std::floor(tout / 5.0 + 0.5) * 5);
    double pin = v(in, "L:A32NX_PNEU_ENG_" + e + "_REGULATED_TRANSDUCER_PRESSURE");
    int pin2 = (int)(std::floor(pin / 2.0 + 0.5) * 2);
    std::string ps = std::to_string(packIndex) + "_FLOW_VALVE_" + std::to_string(flowIdx);
    bool packOpen = b(in, "L:A32NX_COND_PACK_" + ps + "_IS_OPEN");
    double rate = v(in, "L:A32NX_PNEU_PACK_" + ps + "_FLOW_RATE");
    bool xl = b(in, "L:A32NX_PNEU_XBLEED_VALVE_L_OPEN"), xc = b(in, "L:A32NX_PNEU_XBLEED_VALVE_C_OPEN"),
         xr = b(in, "L:A32NX_PNEU_XBLEED_VALVE_R_OPEN");
    bool apu = b(in, "L:A32NX_APU_BLEED_AIR_VALVE_OPEN");
    bool hidden;
    if (engine == 1) hidden = !bleedOpen && !apu && !(xl || xc);
    else if (engine == 2) hidden = !bleedOpen && !xl;
    else if (engine == 3) hidden = !bleedOpen && !xc;
    else hidden = !bleedOpen && !(xc && xr);

    cv.save();
    cv.translate(x, y);
    text(cv, e, -18, 32, T(29, kWhite));
    if (auto img = load_image("Engine-Hyd-Bleed-Dithered.png"))
        draw_image_two_step(cv, img, SkRect::MakeXYWH(0, 0, 80, 124));
    if (engine == 1) {
        rect_fill(cv, 39, 85, 28, 20, 0x000000);
        rect_fill(cv, 7, 44, 28, 19, 0x000000);
        text(cv, "IP", 8, 63, T(23, kWhite));
        text(cv, "HP", 40, 105, T(23, kWhite));
    }
    uint32_t ok = (hpOpen || running) ? kGreen : kAmber;
    valve(cv, 32, -24, 19.5f, bleedOpen, ok);
    path(cv, "M 32,-4 l 0,46", ok);
    if (bleedOpen) path(cv, "M 32,-67 l 0,23", kGreen);
    valve(cv, 64, 45, 19.5f, hpOpen, ok);
    path(cv, "M 64,65 l 0,20", ok);
    if (hpOpen) path(cv, "M 64,25 v -12 h -31", kGreen);
    if (engine % 2 == 1) {
        text(cv, "PSI", 83, -113, T(22, kCyan));
        text(cv, "\xC2\xB0" "C", 85, -71, T(22, kCyan));
    }
    path(cv, "M -9,-141 h 83 v 73 h -83 v -7 z", kGrey);
    text(cv, pin2 < 0 ? std::string("XX") : std::to_string(pin2), 34, -122, TM(27, (pin2 <= 4 || pin2 > 60) ? kAmber : kGreen));
    text(cv, std::to_string(tout5), 34, -82, TM(27, (tout5 < 150 || tout5 > 257) ? kAmber : kGreen));
    if (!hidden) path(cv, "M 33,-237 v 95", kGreen);
    pack_gauge(cv, 33, -257, engine, packOpen, rate);
    cv.restore();
}

void bleed_pack(SkCanvas& cv, const a380::Inputs& in, float x, float y, int pack, bool operative) {
    std::string p = std::to_string(pack);
    double temp = v(in, "L:A32NX_COND_PACK_" + p + "_OUTLET_TEMPERATURE");
    uint32_t c = operative ? kGreen : kAmber;
    cv.save();
    cv.translate(x, y);
    path(cv, "M79,-21 l 0,-23", c);
    path(cv, "M78,115 l 0,-21", c);
    path(cv, "M 21,0 h -21 v 93 h 158 v -93 h -21", kGrey);
    text(cv, "PACK", 42, 0, T(22, operative ? kWhite : kAmber));
    text(cv, p, 108, 0, T(24, c));
    if (operative) {
        text(cv, std::to_string(js_round(temp)), 83, 57, T(28, kGreen, End));
        text(cv, "°" "C", 84, 57, T(22, kCyan));
    }
    bool f1 = b(in, "L:A32NX_COND_FDAC_" + p + "_CHANNEL_1_FAILURE"), f2 = b(in, "L:A32NX_COND_FDAC_" + p + "_CHANNEL_2_FAILURE");
    if (f1 || f2) {
        float ox = pack == 1 ? -50 : 210, oy = 25;
        text(cv, "CTL", ox, oy, TM(23, kWhite));
        text(cv, "1", ox - 20, oy + 25, TM(22, f1 ? kAmber : kGreen));
        text(cv, "2", ox + 20, oy + 25, TM(22, f2 ? kAmber : kGreen));
        path(cv, "M-31,35 h 18 v -21", kGrey);
        path(cv, "M10,35 h 18 v -21", kGrey);
    }
    cv.restore();
}

void hot_air(SkCanvas& cv, const a380::Inputs& in, float x, float y, int n) {
    std::string s = std::to_string(n);
    bool anyPack = b(in, "L:A32NX_COND_PACK_" + s + "_FLOW_VALVE_1_IS_OPEN");
    bool hot = tcsBit(in, 14 + n);
    float xo = n == 1 ? 68.f : -68.f;
    uint32_t c1 = anyPack ? kGreen : kAmber, c2 = (hot && anyPack) ? kGreen : kAmber;
    cv.save();
    cv.translate(x, y);
    path(cv, n == 1 ? "M0,0 l 68,0 l 0,-26" : "M0,0 l -68,0 l 0,-26", c1);
    draw_line(cv, xo, -66, xo, -88, sk_stroke(c2, 2.f));
    triangle(cv, xo, -108, c2, 0, 1.3f);
    valve(cv, xo, -46, 19.5f, hot, c2);
    text(cv, s, n == 1 ? 105.f : -101.f, -43, TM(22, kWhite));
    cv.restore();
}

}  // namespace

void draw_bleed(SkCanvas& cv, const a380::Inputs& in) {
    TextStyle t = T(36, kWhite);
    t.underline = true;
    text(cv, "BLEED", 6, 29, t);

    bool ram = b(in, "L:A32NX_OVHD_COND_RAM_AIR_PB_IS_ON");
    bool supplied = b(in, "L:A32NX_COND_PACK_1_FLOW_VALVE_1_IS_OPEN") || b(in, "L:A32NX_COND_PACK_1_FLOW_VALVE_2_IS_OPEN") ||
                    b(in, "L:A32NX_COND_PACK_2_FLOW_VALVE_1_IS_OPEN") || b(in, "L:A32NX_COND_PACK_2_FLOW_VALVE_2_IS_OPEN") || ram;
    cv.save();
    cv.translate(164, 34);
    path(cv, "M 105,14 h -105 v -14 h 441 v 14 h -98 m -54,0 h -130", kGrey);
    gauge_marker(cv, 152, 14, ram ? 1.f : 2.f, 0, 2, 44, 180, 270, kGreen, 3.f, true, true, true, 0.07f, 1.f);
    circle_stroke(cv, 152, 14, 4, kGreen, 2.f);
    gauge_marker(cv, 337, 15, ram ? 1.f : 2.f, 0, 2, 44, 180, 270, kGreen, 3.f, true, true, true, 0.07f, 1.f);
    circle_stroke(cv, 337, 15, 4, kGreen, 2.f);
    triangle(cv, 114, -23, supplied ? kGreen : kAmber, 0, 1.3f);
    triangle(cv, 328, -23, supplied ? kGreen : kAmber, 0, 1.3f);
    cv.restore();

    text(cv, "RAM", 356, 72, TM(22, kWhite));
    text(cv, "AIR", 415, 72, TM(22, kWhite));
    hot_air(cv, in, 247, 207, 1);
    hot_air(cv, in, 520, 207, 2);
    text(cv, "HOT", 355, 193, TM(22, kWhite));
    text(cv, "AIR", 414, 193, TM(22, kWhite));

    bleed_engine(cv, in, 78, 528, 1);
    bleed_engine(cv, in, 215, 528, 2);
    bleed_engine(cv, in, 490, 528, 3);
    bleed_engine(cv, in, 627, 528, 4);
    bleed_pack(cv, in, 100, 93, 1, agsBit(in, 13));
    bleed_pack(cv, in, 512, 93, 2, agsBit(in, 14));

    bool xl = b(in, "L:A32NX_PNEU_XBLEED_VALVE_L_OPEN"), xc = b(in, "L:A32NX_PNEU_XBLEED_VALVE_C_OPEN"),
         xr = b(in, "L:A32NX_PNEU_XBLEED_VALVE_R_OPEN");
    bool apu = b(in, "L:A32NX_APU_BLEED_AIR_VALVE_OPEN");
    cv.save();
    cv.translate(0, 330);
    valve(cv, 292, 0, 19.5f, !xl, kGreen);
    valve(cv, 395, 0, 19.5f, !xc, kGreen);
    valve(cv, 478, 0, 19.5f, !xr, kGreen);
    bool any = apu || xl || xc;
    if (xl) {
        path(cv, "M248,0 l 24,0", kGreen);
        path(cv, "M312,0 l 14,0", kGreen);
    }
    if (any) {
        path(cv, "M326,0 l 34,0", kGreen);
        path(cv, "M326,0 v 40 h -60,0 m-10,10 l 20,-20 m-10,10 m -35,0 m-10,10 l20,-20 m-10,10 h -119", kGreen);
        path(cv, "M360,0 l 15,0", kGreen);
    }
    if (xc) path(cv, "M415,0 l 24,0", kGreen);
    if (xr) path(cv, "M439,0 l 19,0", kGreen);
    if (xr || xc)
        path(cv, "M439,0 v 40 h 67,0 m-10,10 l 20,-20 m-10,10 m 35,0 m-10,10 l20,-20 m-10,10 h 118", kGreen);
    if (xr) path(cv, "M498,0 l 24,0", kGreen);
    cv.restore();

    bool onGround = b(in, "L:A32NX_LGCIU_1_LEFT_GEAR_COMPRESSED");
    A429 cas = var429(in, "L:A32NX_ADIRS_ADR_1_COMPUTED_AIRSPEED");
    if (onGround && cas.ssm != 0 && cas.val < 50) {
        triangle(cv, 343, 327, kWhite, 180, 1.3f);
        text(cv, "GND", 344, 297, TM(22, kWhite));
    }

    bool master = b(in, "L:A32NX_OVHD_APU_MASTER_SW_PB_IS_ON");
    cv.save();
    cv.translate(360, 602);
    valve(cv, 0, -150, 19.5f, apu, kGreen);
    if (apu) {
        path(cv, "M0,-69 l 0,-61", kGreen);
        path(cv, "M0,-170 l 0,-101", kGreen);
    }
    if (master) {
        valve(cv, 0, -49, 19.5f, apu, kGreen);
        path(cv, "M 0,0 l 0,-30", kGreen);
        text(cv, "APU", 1, 16, TM(23, kWhite));
    }
    cv.restore();
}

}  // namespace sdsys
