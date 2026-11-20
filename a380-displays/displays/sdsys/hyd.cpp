#include <algorithm>
#include <string>

#include "include/core/SkPaint.h"
#include "include/core/SkSamplingOptions.h"

#include "sdsys.h"

namespace sdsys {
namespace {

constexpr auto End = a380::Align::Right;
constexpr auto Mid = a380::Align::Center;

TextStyle ts(float px, uint32_t c, a380::Align al = a380::Align::Left, float ls = 0.f) {
    TextStyle s;
    s.px = px;
    s.color = c;
    s.align = al;
    s.ls = ls;
    if (al == a380::Align::Center) s.base = a380::Baseline::Middle;
    return s;
}

SkPaint paint(uint32_t c, bool stroke, float w = 0.f, bool round = false) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(a380::rgb(c));
    if (stroke) {
        p.setStyle(SkPaint::kStroke_Style);
        p.setStrokeWidth(w);
        if (round) p.setStrokeCap(SkPaint::kRound_Cap);
    }
    return p;
}

void stroke_rect(SkCanvas& cv, float x, float y, float w, float h, uint32_t c, float sw) {
    draw_rect(cv, SkRect::MakeXYWH(x, y, w, h), paint(c, true, sw));
}

void fill_rect(SkCanvas& cv, float x, float y, float w, float h, uint32_t c) {
    draw_rect(cv, SkRect::MakeXYWH(x, y, w, h), paint(c, false));
}

void bg_rect(SkCanvas& cv, float x, float y, float w, float h, float sw) {
    fill_rect(cv, x, y, w, h, 0x000000);
    stroke_rect(cv, x, y, w, h, 0x000000, sw);
}

bool any_running(const a380::Inputs& in) {
    for (int i = 1; i <= 4; i++)
        if (v(in, "L:A32NX_ENGINE_N3:" + std::to_string(i)) > 50) return true;
    return false;
}

void enginePump(SkCanvas& cv, float x, float y, char label, bool lowP, bool pbAuto, bool disc) {
    const float size = 39;
    bool mirrored = label == 'B';
    uint32_t color = (!lowP && pbAuto && !disc) ? kGreen : kAmber;
    cv.save();
    cv.translate(x, y);
    stroke_rect(cv, 0, 0, size, size, color, 4.f);
    if (pbAuto && !disc && !lowP) line(cv, size / 2, 0, size / 2, size, color, 4.f);
    if (!pbAuto || disc) {
        SkPaint p = paint(kAmber, true, 4.f, true);
        draw_line(cv, 8, size / 2, size - 8, size / 2, p);
    }
    if (pbAuto && !disc && lowP) {
        TextStyle t = ts(25, kAmber);
        t.outline = 2.f;
        text(cv, "LO", 5, 30, t);
    }
    TextStyle t = ts(28, kWhite);
    t.outline = 2.f;
    text(cv, std::string(1, label), mirrored ? size + 5 : -20, 31, t);
    if (disc) text(cv, "DISC", mirrored ? 0 : -19, size + 25, ts(24, kAmber));
    cv.restore();
}

void engine(SkCanvas& cv, const a380::Inputs& in, float x, float y, int n) {
    bool inner = n == 2 || n == 3;
    std::string sys = n <= 2 ? "GREEN" : "YELLOW";
    int pumpIndex = (n - 1) * 2 + 1 - (n <= 2 ? 0 : 4);
    bool running = v(in, "L:A32NX_ENGINE_N3:" + std::to_string(n)) > 50;
    bool lowLevel = b(in, "L:A32NX_HYD_" + sys + "_RESERVOIR_LEVEL_IS_LOW");
    bool disc = b(in, "L:A32NX_HYD_ENG_" + std::to_string(n) + "AB_PUMP_DISC");
    bool aSw = b(in, "L:A32NX_HYD_" + sys + "_PUMP_" + std::to_string(pumpIndex) + "_SECTION_PRESSURE_SWITCH");
    bool aAuto = b(in, "L:A32NX_OVHD_HYD_ENG_" + std::to_string(n) + "A_PUMP_PB_IS_AUTO");
    bool bSw = b(in, "L:A32NX_HYD_" + sys + "_PUMP_" + std::to_string(pumpIndex + 1) + "_SECTION_PRESSURE_SWITCH");
    bool bAuto = b(in, "L:A32NX_OVHD_HYD_ENG_" + std::to_string(n) + "B_PUMP_PB_IS_AUTO");
    int fa = 1 + 2 * ((n - 1) % 2), fb = 2 + 2 * ((n - 1) % 2);
    bool fireOpen = b(in, "L:A32NX_HYD_" + sys + "_PUMP_" + std::to_string(fa) + "_FIRE_VALVE_OPENED") ||
                    b(in, "L:A32NX_HYD_" + sys + "_PUMP_" + std::to_string(fb) + "_FIRE_VALVE_OPENED");
    uint32_t labelColor = (disc || ((!aSw || !aAuto) && (!bSw || !bAuto))) ? kAmber : kGreen;
    cv.save();
    cv.translate(x, y);
    if (auto img = load_image("HYD_8-7_TRIMMED.png"))
        draw_image_two_step(cv, img, SkRect::MakeXYWH(0, 0, 146, 211));
    text(cv, std::to_string(n), 10, 47, ts(35, running ? kWhite : kAmber));
    enginePump(cv, 28, 67, 'A', !aSw, aAuto, disc);
    enginePump(cv, 80, 67, 'B', !bSw, bAuto, disc);
    {
        float vx = 56, vy = 164, r = 17;
        uint32_t c = (fireOpen && !lowLevel) ? kGreen : kAmber;
        circle_stroke(cv, vx + r, vy + r, r, c, 4.f);
        if (fireOpen) line(cv, vx + r, vy, vx + r, vy + 2 * r, c, 4.f);
        else line(cv, vx, vy + r, vx + 2 * r, vy + r, c, 4.f);
        draw_line(cv, vx + 17, vy - 55, vx + 17, vy - 1, paint(c, true, 4.f, true));
    }
    draw_line(cv, 73, 65, 73, 65 + (inner ? -123 : -165), paint(labelColor, true, 4.f, true));
    cv.restore();
}

void reservoir(SkCanvas& cv, const a380::Inputs& in, float x, float y, bool left) {
    std::string side = left ? "GREEN" : "YELLOW";
    const float height = 166, width = 19;
    auto px = [&](double l) { return (float)(l * height / 70.0); };
    double level = std::min(v(in, "L:A32NX_HYD_" + side + "_RESERVOIR_LEVEL") * 3.785411784, 70.0);
    bool low = b(in, "L:A32NX_HYD_" + side + "_RESERVOIR_LEVEL_IS_LOW");
    uint32_t c = low ? kAmber : kGreen;
    float normal = 45;
    cv.save();
    cv.translate(x, y);
    fill_rect(cv, 0.5f, height - px(level), width - 1, px(level), c);
    stroke_path(cv, svg_path("M 0, 0 v 166 h 19 v -166"), kWhite, 2.f, true);
    stroke_path(cv, svg_path("m 9 -37 v 202"), c, 4.f);
    stroke_path(cv, svg_path(std::string("m 9 ") + (left ? "-12" : "-13") + " h " + (left ? "157" : "-155") + " v -66"), c, 4.f);
    for (float off : {-16.f, 1.f}) {
        bg_rect(cv, width + 1, height - px(normal) + off, 7, 15, 3.f);
        fill_rect(cv, width + 1, height - px(normal) + off, 7, 15, kGreen);
    }
    bg_rect(cv, width, height - 33, 6, 33, 3.f);
    fill_rect(cv, width, height - 33, 6, 33, kAmber);
    bool lowAir = b(in, "L:A32NX_HYD_" + side + "_RESERVOIR_AIR_PRESSURE_IS_LOW");
    bool ovht = b(in, "L:A32NX_HYD_" + side + "_RESERVOIR_OVHT");
    if (lowAir) {
        text(cv, "AIR", left ? 70 : -50, 72, ts(24, kAmber, Mid));
        text(cv, "PRESS", left ? 70 : -50, 96, ts(24, kAmber, Mid));
        text(cv, "LOW", left ? 70 : -50, 120, ts(24, kAmber, Mid));
    }
    if (ovht) text(cv, "OVHT", left ? 32 : -70, 155, ts(24, kAmber));
    cv.restore();
}

void elec_pump(SkCanvas& cv, const a380::Inputs& in, float x, float y, char label, bool left, bool sysSw) {
    std::string side = left ? "GREEN" : "YELLOW";
    char s0 = side[0];
    bool active = b(in, std::string("L:A32NX_HYD_") + s0 + label + "_EPUMP_ACTIVE");
    bool pSw = b(in, "L:A32NX_HYD_" + side + "_PUMP_" + (label == 'A' ? "5" : "6") + "_SECTION_PRESSURE_SWITCH");
    bool offAuto = b(in, std::string("L:A32NX_OVHD_HYD_EPUMP") + s0 + label + "_OFF_PB_IS_AUTO");
    bool ovht = b(in, std::string("L:A32NX_HYD_") + s0 + label + "_EPUMP_OVHT");
    uint32_t tc = 0;
    bool has = true;
    if (offAuto && !active) tc = kWhite;
    else if (!offAuto || !pSw) tc = kAmber;
    else if (offAuto && active && pSw) tc = kGreen;
    else has = false;
    cv.save();
    cv.translate(x, y);
    text(cv, std::string(1, label), left ? 21 : -35, 10, ts(25, tc == kAmber ? kAmber : kWhite));
    if (has) {
        SkPath tri = svg_path(std::string("m 0 0 l ") + (left ? "" : "-") + "13 -9 l 0 18 z");
        if (active) fill_path(cv, tri, tc);
        stroke_path(cv, tri, tc, 3.f);
        if (active && sysSw) stroke_path(cv, svg_path(left ? "m 0 0 h -30" : "m 0 0 h 27"), tc, 2.f);
    }
    if (ovht) text(cv, "OVHT", -14, label == 'A' ? -14 : 34, ts(19, kAmber));
    cv.restore();
}

void system_label(SkCanvas& cv, const a380::Inputs& in, float x, float y, bool left, bool running) {
    std::string side = left ? "GREEN" : "YELLOW";
    double pressure = v(in, "L:A32NX_HYD_" + side + "_SYSTEM_1_SECTION_PRESSURE");
    bool sw = b(in, "L:A32NX_HYD_" + side + "_SYSTEM_1_SECTION_PRESSURE_SWITCH");
    int p100 = pressure >= 200 ? js_round(pressure / 100.0) * 100 : 0;
    uint32_t tcol = p100 > 2900 ? kGreen : kAmber;
    uint32_t scol = sw ? kGreen : kAmber;
    cv.save();
    cv.translate(x, y);
    {
        SkPaint p = paint(scol, true, 3.f);
        p.setStrokeJoin(SkPaint::kRound_Join);
        draw_path(cv, svg_path("m 114 -31 l 12 20 l -24 0 z"), p);
    }
    stroke_rect(cv, 0, 0, 228, left ? 38 : 36, scol, 3.f);
    text(cv, side, 7, 29, ts(23, sw ? kWhite : kAmber));
    text(cv, std::to_string(p100), left ? 172 : 179, 31, ts(30, tcol, End));
    text(cv, "PSI", left ? 175 : 181, 29, ts(23, kCyan));
    if (!running) {
        elec_pump(cv, in, left ? 258 : -26, left ? 4 : 3, 'A', left, sw);
        elec_pump(cv, in, left ? 258 : -26, left ? 36 : 34, 'B', left, sw);
    }
    cv.restore();
}

}  // namespace

void draw_hyd(SkCanvas& cv, const a380::Inputs& in) {
    TextStyle t = ts(36, kWhite);
    t.underline = true;
    text(cv, "HYD", 6, 29, t);
    bool running = any_running(in);
    engine(cv, in, 8, 231, 1);
    engine(cv, in, 165, 189, 2);
    reservoir(cv, in, 72, 467, true);
    system_label(cv, in, 38, 92, true, running);
    if (!running) {
        text(cv, "ELEC", 352, 110, ts(26, kWhite, a380::Align::Left, 1.f));
        text(cv, "PMP", 352, 135, ts(26, kWhite, a380::Align::Left, 1.f));
        text(cv, "S", 401, 135, ts(23, kWhite));
    }
    engine(cv, in, 612, 231, 4);
    engine(cv, in, 457, 189, 3);
    reservoir(cv, in, 676, 467, false);
    system_label(cv, in, 500, 94, false, running);
}

}  // namespace sdsys
