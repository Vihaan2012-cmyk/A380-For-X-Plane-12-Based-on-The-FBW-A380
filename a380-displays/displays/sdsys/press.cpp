#include <cmath>
#include <cstdio>
#include <algorithm>
#include <cstring>
#include <string>

#include "include/core/SkPaint.h"
#include "include/core/SkPathBuilder.h"

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

SkPaint sp(uint32_t c, float w) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(a380::rgb(c));
    p.setStyle(SkPaint::kStroke_Style);
    p.setStrokeWidth(w);
    return p;
}

void arc(SkCanvas& cv, float x, float y, float r, float a0, float a1, uint32_t color, float width) {
    arc_path(cv, x, y, r, a0, a1, color, width);
}

void marker(SkCanvas& cv, float x, float y, float value, float mn, float mx, float r, float a0, float a1, uint32_t color,
            bool indicator) {
    float ang = (a0 + (value - mn) / (mx - mn) * (a1 - a0) - 90.f) * 3.14159265f / 180.f;
    float dx = std::cos(ang), dy = std::sin(ang);
    float s = indicator ? 0.f : 0.85f;
    draw_line(cv, x + dx * r * s, y + dy * r * s, x + dx * r, y + dy * r, sp(color, 2.f));
}

void bg_circle(SkCanvas& cv, float x, float y) {
    circle_fill(cv, x, y, 4, 0x000000);
    circle_stroke(cv, x, y, 4, kGreen, 3.f);
}

void extract_valve(SkCanvas& cv, float x, float y, float value, float mn, float mx) {
    marker(cv, x, y, value, mn, mx, 42, 90, 180, kGreen, true);
    bg_circle(cv, x, y);
}

void pack(SkCanvas& cv, const a380::Inputs& in, float x, float y, int n) {
    std::string s = std::to_string(n);
    bool open = b(in, "L:A32NX_COND_PACK_" + s + "_FLOW_VALVE_1_IS_OPEN") || b(in, "L:A32NX_COND_PACK_" + s + "_FLOW_VALVE_2_IS_OPEN");
    triangle(cv, x + 45, y - 50, open ? kGreen : kAmber, 0, 1.2f);
    TextStyle t = ts(26, open ? kWhite : kAmber);
    t.plain = true;
    text(cv, "PACK", x, y, t);
    text(cv, s, x + 75, y, t);
}


struct Arinc {
    int ssm;
    float val;
};

Arinc arinc(double raw) {
    double hi = std::floor(raw / 4294967296.0);
    uint32_t bits = (uint32_t)(uint64_t)(raw - hi * 4294967296.0);
    float f;
    std::memcpy(&f, &bits, 4);
    return {(int)hi, f};
}

int cpcs_system(const a380::Inputs& in) {
    for (int i = 1; i <= 4; i++)
        if (arinc(v(in, "L:A32NX_COND_CPIOM_B" + std::to_string(i) + "_CPCS_DISCRETE_WORD")).ssm == 3) return i;
    return 0;
}

void outflow(SkCanvas& cv, const a380::Inputs& in, float x, float y, int engine, int system) {
    std::string e = std::to_string(engine);
    double man = v(in, "L:A32NX_PRESS_MAN_OUTFLOW_VALVE_" + e + "_OPEN_PERCENTAGE");
    Arinc oa = system ? arinc(v(in, "L:A32NX_PRESS_OUTFLOW_VALVE_" + e + "_OPEN_PERCENTAGE_B" + std::to_string(system))) : Arinc{0, 0};
    double pct = oa.ssm == 3 ? oa.val : man;
    double phase = v(in, "L:A32NX_FWC_FLIGHT_PHASE");
    Arinc cw = arinc(v(in, "L:A32NX_COND_CPIOM_B" + e + "_CPCS_DISCRETE_WORD"));
    bool discreteFault = (cw.ssm == 3 || cw.ssm == 2) ? ((((int32_t)cw.val) >> 10) & 1) != 0 : true;
    bool fault = discreteFault || b(in, "L:A32NX_PRESS_OCSM_" + e + "_AUTO_PARTITION_FAILURE") ||
                 b(in, "L:A32NX_PRESS_OCSM_" + e + "_CHANNEL_2_FAILURE");
    text(cv, e, x - 32, y - 82, ts(26, fault ? kAmber : kGreen));
    const float r = 52;
    arc(cv, x, y, r, 270.f + (float)pct / 100.f * 90.f, 360, kWhite, 2.f);
    arc(cv, x, y + 2, r - 3, 352, 360, kAmber, 8.f);
    marker(cv, x, y, (float)pct, 0, 100, r, 270, 360,
           (phase >= 6 && phase <= 9 && pct > 95) ? kAmber : kGreen, true);
    for (float val : {0.f, 25.f, 50.f, 75.f, 100.f}) marker(cv, x, y, val, 0, 100, r, 270, 360, kWhite, false);
    bg_circle(cv, x, y);
}

void gauge_text(SkCanvas& cv, const std::string& s, float x, float y) {
    TextStyle t = ts(20, kWhite, Mid);
    t.base = a380::Baseline::SvgCentral;
    t.outline = 1.1f;
    text(cv, s, x, y, t);
}

void delta_p(SkCanvas& cv, const a380::Inputs& in, float x, float y, int system) {
    double man = v(in, "L:A32NX_PRESS_MAN_CABIN_DELTA_PRESSURE");
    Arinc a = system ? arinc(v(in, "L:A32NX_PRESS_CABIN_DELTA_PRESSURE_B" + std::to_string(system))) : Arinc{0, 0};
    double d = a.ssm == 3 ? a.val : man;
    bool na = a.ssm == 1;
    char buf[32];
    std::snprintf(buf, sizeof buf, "%.1f", d);
    std::string s = buf;
    size_t dot = s.find('.');
    uint32_t col = (d < -0.72 || d > 9.2) ? kRed : ((d >= -0.72 && d <= -0.2) || (d >= 8.92 && d <= 9.2)) ? kAmber : kGreen;
    TextStyle h = ts(29, kWhite, a380::Align::Left, 1.f);
    text(cv, "DELTA P", x - 51, y - 142, h);
    text(cv, "PSI", x - 18, y - 105, ts(24, kCyan));
    if (!na) {
        text(cv, s.substr(0, dot), x + 9, y + 48, ts(35, col, End));
        text(cv, ".", x + 29, y + 48, ts(35, col, End));
        text(cv, s.substr(dot + 1), x + 56, y + 48, ts(35, col, End));
    } else {
        text(cv, "XX", x + 53, y + 35, ts(35, kAmber, End));
    }
    const float r = 86, sa = 205, ea = 55;
    arc(cv, x, y, r, sa, ea, kWhite, 2.f);
    arc(cv, x, y, r - 2, ea - 15, ea, kRed, 8.f);
    arc(cv, x, y, r - 2, ea - 30, ea - 15, kAmber, 8.f);
    arc(cv, x, y, r - 2, sa, sa + 8, kRed, 8.f);
    arc(cv, x, y, r - 2, sa + 8, sa + 16, kAmber, 8.f);
    auto dir = [&](float val, float& dx, float& dy) {
        float pct = (val + 1.f) / 11.5f;
        float ang = (sa + pct * (360.f - sa + ea) - 90.f) * 3.14159265f / 180.f;
        dx = std::cos(ang);
        dy = std::sin(ang);
    };
    float dx, dy;
    for (float val : {10.f, 7.5f, 5.f, 2.5f, 0.f}) {
        dir(val, dx, dy);
        draw_line(cv, x + dx * r * 0.85f, y + dy * r * 0.85f, x + dx * r, y + dy * r, sp(kWhite, val == 0.f ? 1.f : 2.f));
    }
    dir(10, dx, dy);
    gauge_text(cv, "10", x + dx * r * 0.85f - 3, y + dy * r * 0.85f + 14);
    dir(0, dx, dy);
    gauge_text(cv, "0", x + dx * r * 0.85f + 10, y + dy * r * 0.85f - 6);
    if (!na) {
        dir((float)d, dx, dy);
        SkPaint p = sp(kGreen, 4.f);
        p.setStrokeCap(SkPaint::kRound_Cap);
        draw_line(cv, x, y, x + dx * r * 1.01f, y + dy * r * 1.01f, p);
    }
}

void gdir(float val, float mn, float mx, float sa, float ea, float& dx, float& dy) {
    float pct = (val - mn) / (mx - mn);
    float span = sa > ea ? 360.f - sa + ea : ea - sa;
    float ang = (sa + pct * span - 90.f) * 3.14159265f / 180.f;
    dx = std::cos(ang);
    dy = std::sin(ang);
}

void cab_alt(SkCanvas& cv, const a380::Inputs& in, float x, float y, int system) {
    double man = v(in, "L:A32NX_PRESS_MAN_CABIN_ALTITUDE");
    Arinc a = system ? arinc(v(in, "L:A32NX_PRESS_CABIN_ALTITUDE_B" + std::to_string(system))) : Arinc{0, 0};
    double alt = a.ssm == 3 ? a.val : man;
    double alt50 = std::floor(alt / 50.0 + 0.5) * 50.0;
    Arinc tg = system ? arinc(v(in, "L:A32NX_PRESS_CABIN_ALTITUDE_TARGET_B" + std::to_string(system))) : Arinc{0, 0};
    bool autoMode = b(in, "L:A32NX_OVHD_PRESS_MAN_ALTITUDE_PB_IS_AUTO");
    const float mn = -0.6f, mx = 12.5f, r = 87, sa = 212, ea = 89;
    text(cv, autoMode ? "AUTO" : "MAN", x - 117, y - 142, ts(29, kGreen, a380::Align::Left, 1.f));
    text(cv, "CAB ALT", x - 26, y - 142, ts(29, kWhite, a380::Align::Left, 1.f));
    text(cv, "FT", x - 10, y - 105, ts(24, kCyan));
    text(cv, std::to_string((int)alt50), x + 108, y + 66, ts(35, alt50 >= 9550 ? kRed : kGreen, End));
    arc(cv, x, y, r, sa, ea, kWhite, 2.f);
    arc(cv, x, y, r - 2, ea - 50, ea, kRed, 8.f);
    {
        TextStyle t = ts(20, kWhite);
        t.outline = 1.1f;
        text(cv, "17", x + 45, y + 6, t);
    }
    float dx, dy;
    struct M { float val; bool thin; const char* label; float nx, ny; };
    for (M m : {M{12.5f, false, nullptr, 0, 0}, M{10.f, true, "10", -3, 14}, M{7.5f, false, nullptr, 0, 0},
                M{5.f, true, "5", 13, 12}, M{2.5f, false, nullptr, 0, 0}, M{0.f, true, "0", 7, -4}}) {
        gdir(m.val, mn, mx, sa, ea, dx, dy);
        draw_line(cv, x + dx * r * 0.85f, y + dy * r * 0.85f, x + dx * r, y + dy * r, sp(kWhite, m.thin ? 1.f : 2.f));
        if (m.label) gauge_text(cv, m.label, x + dx * r * 0.85f + m.nx, y + dy * r * 0.85f + m.ny);
    }
    gdir((float)(alt50 / 1000.0), mn, mx, sa, ea, dx, dy);
    SkPaint p = sp(kGreen, 4.f);
    p.setStrokeCap(SkPaint::kRound_Cap);
    draw_line(cv, x, y, x + dx * r * 1.01f, y + dy * r * 1.01f, p);
    if (tg.ssm == 3) {
        double t50 = std::floor(tg.val / 50.0 + 0.5) * 50.0;
        float cl = (float)std::min((double)mx, std::max((double)mn, t50 / 1000.0));
        gdir(cl, mn, mx, sa, ea, dx, dy);
        circle_stroke(cv, x + dx * r * 1.1f, y + dy * r * 1.1f, 6, autoMode ? kMagenta : kCyan, 3.f);
    }
}

void cab_vs(SkCanvas& cv, const a380::Inputs& in, float x, float y, int system) {
    double man = v(in, "L:A32NX_PRESS_MAN_CABIN_VS");
    Arinc a = system ? arinc(v(in, "L:A32NX_PRESS_CABIN_VS_B" + std::to_string(system))) : Arinc{0, 0};
    double vs = a.ssm == 3 ? a.val : man;
    Arinc tg = system ? arinc(v(in, "L:A32NX_PRESS_CABIN_VS_TARGET_B" + std::to_string(system))) : Arinc{0, 0};
    bool autoMode = b(in, "L:A32NX_OVHD_PRESS_MAN_VS_CTL_PB_IS_AUTO");
    const float r = 88, mn = -2, mx = 2, sa = 180, ea = 0;
    text(cv, autoMode ? "AUTO" : "MAN", x - 109, y - 142, ts(29, kGreen, a380::Align::Left, 1.f));
    text(cv, "V/S", x - 15, y - 142, ts(29, kWhite, a380::Align::Left, 1.f));
    text(cv, "FT/MIN", x - 43, y - 105, ts(24, kCyan));
    text(cv, std::to_string((int)(std::floor(vs / 50.0 + 0.5) * 50)), x + 128, y + 13, ts(35, kGreen, End));
    arc(cv, x, y, r, 158, 22, kWhite, 2.f);
    struct M { float val; const char* label; float nx, ny; };
    float dx, dy;
    for (M m : {M{2.f, "2", 0, 14}, M{1.f, "1", 12, 12}, M{0.f, "0", 8, 2}, M{-1.f, "1", 12, -12}, M{-2.f, "2", 0, -11}}) {
        gdir(m.val, mn, mx, sa, ea, dx, dy);
        draw_line(cv, x + dx * r * 0.85f, y + dy * r * 0.85f, x + dx * r, y + dy * r, sp(kWhite, 1.f));
        gauge_text(cv, m.label, x + dx * r * 0.85f + m.nx, y + dy * r * 0.85f + m.ny);
    }
    float val = (float)(vs / 1000.0);
    if (std::fabs(val) > 2.25f) val = 2.25f;
    gdir(val, mn, mx, sa, ea, dx, dy);
    SkPaint p = sp(kGreen, 3.f);
    p.setStrokeCap(SkPaint::kRound_Cap);
    draw_line(cv, x, y, x + dx * r * 1.01f, y + dy * r * 1.01f, p);
    if (tg.ssm == 3) {
        gdir(tg.val / 1000.f, mn, mx, sa, ea, dx, dy);
        circle_stroke(cv, x + dx * r * 1.1f, y + dy * r * 1.1f, 6, autoMode ? kMagenta : kCyan, 3.f);
    }
}

}  // namespace

void draw_press(SkCanvas& cv, const a380::Inputs& in) {
    TextStyle t = ts(36, kWhite);
    t.underline = true;
    text(cv, "CAB PRESS", 6, 29, t);

    if (b(in, "L:A32NX_OVHD_PRESS_MAN_ALTITUDE_PB_IS_AUTO")) {
        double raw = v(in, "L:A32NX_FM1_LANDING_ELEVATION");
        double hi = std::floor(raw / 4294967296.0);
        uint32_t bits = (uint32_t)(uint64_t)(raw - hi * 4294967296.0);
        float f;
        std::memcpy(&f, &bits, 4);
        int elev = std::isfinite(f) ? (int)std::floor(f / 50.f + 0.5f) * 50 : 0;
        text(cv, "LDG ELEVN", 430, 32, ts(25, kWhite, a380::Align::Left, 1.f));
        text(cv, std::to_string(elev), 690, 32, ts(28, kGreen, End));
        text(cv, "FT", 698, 32, ts(24, kCyan));
    }

    int sys = cpcs_system(in);
    delta_p(cv, in, 132, 246, sys);
    cab_alt(cv, in, 369, 246, sys);
    cab_vs(cv, in, 613, 246, sys);
    line(cv, 230, 78, 230, 358, kWhite, 2.f);
    line(cv, 490, 78, 490, 358, kWhite, 2.f);

    cv.save();
    cv.translate(-1, -6);
    auto P = [&](const char* d) { stroke_path(cv, svg_path(d), kWhite, 2.f); };
    P("M612.71,558.4h19.82c13.76-12.88,22.02-31.39,24.77-53.51");
    P("M657.29,458.32c-3.7-19.5-11.14-38.27-26.76-55.48l-443.22,1c-78.61,2.62-117.25,19.8-151.6,70.66c-37.08,43.01-23.45,73.33,39.3,83.9");
    P("M308.52,558.4h33.69v-13.54c24.99-5.29,50.09-4.12,75.3,2.64v10.9h36.66");
    fill_path(cv, svg_path("M47.61,463.28L54.05,463.28L48.43,473.68L40.5,473.68z"), kWhite);
    fill_path(cv, svg_path("M55.86,463.28L50.58,473.68L59.99,478.14L59.99,463.28z"), kWhite);
    fill_path(cv, svg_path("M61.63,463.28L61.63,478.14L72.2,478.14L68.57,463.28z"), kWhite);
    for (const char* d : {"M115,558 l 46,0", "M220,558 l 30,0", "M512,558 l 38,0"}) {
        SkPaint p = sp(kWhite, 2.f);
        p.setStrokeCap(SkPaint::kRound_Cap);
        draw_path(cv, svg_path(d), p);
    }
    cv.restore();

    text(cv, "AUTO", 382, 452, ts(22, kWhite, Mid));
    text(cv, "CTL", 382, 474, ts(22, kWhite, Mid));
    outflow(cv, in, 214, 552, 1, sys);
    line(cv, 212, 460, 254, 460, kWhite, 2.f);
    outflow(cv, in, 303, 552, 2, sys);
    line(cv, 300, 460, 340, 460, kWhite, 2.f);
    outflow(cv, in, 509, 552, 3, sys);
    line(cv, 422, 460, 463, 460, kWhite, 2.f);
    outflow(cv, in, 604, 552, 4, sys);
    line(cv, 510, 460, 558, 460, kWhite, 2.f);
    text(cv, "AVNCS", 100, 485, ts(22, kWhite, Mid));
    text(cv, "EXTRACT", 100, 507, ts(22, kWhite, Mid));
    text(cv, "OVBD", 100, 529, ts(22, kWhite, Mid));
    extract_valve(cv, 72, 552, 0, 0, 4);
    text(cv, "CAB AIR", 712, 412, ts(22, kWhite, Mid, 1.f));
    text(cv, "EXTRACT", 712, 434, ts(22, kWhite, Mid, 1.f));
    extract_valve(cv, 656, 456, 4, 0, 4);
    pack(cv, in, 12, 641, 1);
    pack(cv, in, 647, 641, 2);
}

}  // namespace sdsys
