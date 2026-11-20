#include <algorithm>
#include <cstdio>
#include <string>

#include "sdsys.h"

namespace sdsys {
namespace {

constexpr auto End = a380::Align::Right;
constexpr auto Mid = a380::Align::Center;

struct ApuState {
    double lastDrawMs = -1e18;
    bool prevBleedPb = false;
    double bleedOnSinceMs = -1.0;
    double mismatchSinceMs = -1.0;
};

ApuState gApu;

void gauge_label(SkCanvas& cv, const char* s, float x, float y) {
    TextStyle t = T(22, kWhite, Mid, 1.f);
    t.base = a380::Baseline::SvgCentral;
    text(cv, s, x, y, t);
}

void generator(SkCanvas& cv, const a380::Inputs& in, float x, float y, int pos) {
    std::string p = std::to_string(pos);
    double load = v(in, "L:A32NX_ELEC_APU_GEN_" + p + "_LOAD");
    double pot = v(in, "L:A32NX_ELEC_APU_GEN_" + p + "_POTENTIAL");
    double fr = v(in, "L:A32NX_ELEC_APU_GEN_" + p + "_FREQUENCY");
    bool loadOk = load < 108, freqOk = fr > 390 && fr < 410, voltOk = pot > 110 && pot < 120;
    bool closed = b(in, "L:A32NX_ELEC_CONTACTOR_990XS" + p + "_IS_CLOSED");
    bool sw = b(in, "APU GENERATOR SWITCH:" + p);
    bool master = b(in, "L:A32NX_OVHD_APU_MASTER_SW_PB_IS_ON");
    cv.save();
    cv.translate(x, y);
    uint32_t c = (sw || !master) ? kWhite : kAmber;
    text(cv, "GEN", pos == 1 ? 8 : 7, 10, T(22, c));
    text(cv, pos == 1 ? "A" : "B", pos == 1 ? 65 : 64, 10, T(22, c));
    if (master) draw_path(cv, svg_path("M 5,0 l -5,0 l 0,100 l 83,0 l 0,-100 l -5,0"), sk_stroke(kWhite, 3.f, true));
    if (master && sw) {
        text(cv, std::to_string(js_round(load)), 73, 42, T(29, loadOk ? kGreen : kAmber, End, 1.f));
        text(cv, std::to_string(js_round(pot)), 73, 67, T(29, freqOk ? kGreen : kAmber, End, 1.f));
        text(cv, std::to_string(js_round(fr)), 73, 93, T(29, voltOk ? kGreen : kAmber, End, 1.f));
    }
    if (!sw && master) text(cv, "OFF", 72, 62, T(29, kWhite, End, 1.f));
    triangle(cv, pos == 1 ? 43 : 39, -29, closed ? kGreen : kWhite, 0, 1.f);
    cv.restore();
}

void apu_bleed(SkCanvas& cv, const a380::Inputs& in, float x, float y, bool pbConfirmed) {
    bool open = b(in, "L:A32NX_APU_BLEED_AIR_VALVE_OPEN");
    uint32_t color = !open && pbConfirmed ? kAmber : kGreen;
    A429 n = var429(in, "L:A32NX_APU_N");
    int adr = v(in, "L:A32NX_ATT_HDG_SWITCHING_KNOB") == 0 ? 3 : 1;
    A429 sp = var429(in, "L:A32NX_ADIRS_ADR_" + std::to_string(adr) + "_CORRECTED_AVERAGE_STATIC_PRESSURE");
    bool avail = n.normal() && sp.normal();
    double pr = v(in, "L:A32NX_PNEU_APU_BLEED_CONTAINER_PRESSURE") - 14.7;
    int shown = open ? (int)(std::floor(pr / 2.0 + 0.5) * 2) : 0;
    cv.save();
    cv.translate(x, y);
    if (open) {
        draw_line(cv, 60, -56, 60, -71, sk_stroke(kGreen, 2.f));
        draw_path(cv, svg_path("M50,-71 L60,-86 L70,-71 Z"), sk_stroke(kGreen, 2.f));
    }
    valve(cv, 61, -37, 19.5f, open, color);
    draw_line(cv, 60, 0, 60, -17, sk_stroke(color, 2.f));
    rect_stroke(cv, 0, 0, 118, 71, kWhite, 2.f);
    text(cv, "BLEED", 25, 27, T(22, kWhite, a380::Align::Left, 1.f));
    text(cv, avail ? std::to_string(shown) : "XX", 56, 63, T(27, avail ? kGreen : kAmber, End));
    text(cv, "PSI", 62, 62, T(22, kCyan));
    cv.restore();
}

void n_gauge(SkCanvas& cv, const a380::Inputs& in, float x, float y) {
    A429 n = var429(in, "L:A32NX_APU_N");
    A429 n2 = var429(in, "L:A32NX_APU_N2");
    uint32_t c1 = n.val < 105 ? kGreen : kRed;
    uint32_t c2 = n2.val < 102 ? kGreen : kRed;
    cv.save();
    cv.translate(x, y);
    const float r = 64, sa = 225, ea = 46;
    cv.save();
    cv.translate(0, -1);
    arc_path(cv, 0, 0, r, sa, ea, kWhite, 3.f);
    arc_path(cv, 0, 0, r - 1, ea - 21, ea, kRed, 6.f);
    gauge_marker(cv, 0, 0, 0, 0, 12, r, sa, ea, kWhite, 3.f, true);
    gauge_marker(cv, 0, 0, 5, 0, 12, r, sa, ea, kWhite, 3.f, true);
    gauge_marker(cv, 0, 0, 10, 0, 12, r, sa, ea, kWhite, 3.f, true);
    float dx, dy;
    gauge_dir(0, 0, 12, sa, ea, dx, dy);
    gauge_label(cv, "0", dx * r * 0.85f + 12, dy * r * 0.85f - 10);
    gauge_dir(10, 0, 12, sa, ea, dx, dy);
    gauge_label(cv, "10", dx * r * 0.85f - 2, dy * r * 0.85f + 17);
    if (n.normal())
        gauge_marker(cv, 0, 0, (float)std::floor(n.val + 0.5f), 0, 120, r, sa, ea, c1, 4.f, true, true);
    cv.restore();
    text(cv, "N1", 104, 18, TM(26, kWhite, 1.f));
    text(cv, "%", 101, 57, TM(22, kCyan));
    text(cv, "N2", 104, 97, TM(26, kWhite, 1.f));
    text(cv, n.normal() ? std::to_string(js_round(n.val)) : "XX", 72, 24, T(35, n.normal() ? c1 : kAmber, End, 1.f));
    text(cv, n2.normal() ? std::to_string(js_round(n2.val)) : "XX", 72, 103,
         T(35, n2.normal() ? c2 : kAmber, End, 1.f));
    cv.restore();
}

void egt_gauge(SkCanvas& cv, const a380::Inputs& in, float x, float y) {
    A429 egt = var429(in, "L:A32NX_APU_EGT");
    A429 warn = var429(in, "L:A32NX_APU_EGT_WARNING");
    int shown = (int)(std::floor(egt.val / 5.0 + 0.5) * 5);
    uint32_t needle = egt.val > warn.val ? kRed : kGreen;
    uint32_t numeric = egt.normal() ? needle : kAmber;
    const float r = 64, sa = 221, mid = 310, ea = 122, mx = 9.5f;
    cv.save();
    cv.translate(x, y);
    arc_path(cv, 0, 0, r, sa - 10, ea, kWhite, 3.f);
    float dx, dy;
    gauge_marker(cv, 0, 0, 0, 0, mx, r, sa, ea, kWhite, 3.f, true);
    gauge_dir(0, 0, mx, sa, ea, dx, dy);
    gauge_label(cv, "0", dx * r * 0.85f + 9, dy * r * 0.85f - 9);
    for (int k = 4; k <= 9; k++) {
        gauge_marker(cv, 0, 0, (float)k, 4, mx, r, mid, ea, kWhite, 3.f, true);
        if (k == 5 || k == 7 || k == 9) {
            gauge_dir((float)k, 4, mx, mid, ea, dx, dy);
            float nx = k == 5 ? 9 : k == 7 ? -8 : -9, ny = k == 5 ? 12 : k == 7 ? 10 : -7;
            gauge_label(cv, std::to_string(k).c_str(), dx * r * 0.85f + nx, dy * r * 0.85f + ny);
        }
    }
    if (warn.normal())
        arc_path(cv, 0, 0, r - 1, ea + ((950.f - warn.val) / (950.f - 400.f)) * (ea - mid), ea, kRed, 6.f);
    if (egt.normal()) {
        bool low = shown < 400;
        gauge_marker(cv, 0, 0, (float)shown, low ? 0.f : 400.f, low ? 400.f : 950.f, r, low ? sa : mid, low ? mid : ea,
                     needle, 4.f, true, true);
    }
    text(cv, "EGT", 77, 29, T(26, kWhite, a380::Align::Left, 1.f));
    text(cv, "\xC2\xB0" "C", 100, 60, TM(22, kCyan));
    text(cv, egt.normal() ? std::to_string(shown) : "XX", 83, 67, T(35, numeric, End, 1.f));
    cv.restore();
}

}  // namespace

void draw_apu(SkCanvas& cv, const a380::Inputs& in) {
    const double now = in.now_ms();
    if (now - gApu.lastDrawMs > 1500.0 || now < gApu.lastDrawMs) {
        gApu.prevBleedPb = false;
        gApu.bleedOnSinceMs = -1.0;
        gApu.mismatchSinceMs = -1.0;
    }
    gApu.lastDrawMs = now;
    const bool bleedPb = b(in, "L:A32NX_OVHD_PNEU_APU_BLEED_PB_IS_ON");
    if (bleedPb && !gApu.prevBleedPb) gApu.bleedOnSinceMs = now;
    if (!bleedPb) gApu.bleedOnSinceMs = -1.0;
    gApu.prevBleedPb = bleedPb;
    const bool bleedConfirmed = gApu.bleedOnSinceMs >= 0.0 && now - gApu.bleedOnSinceMs >= 10000.0;
    const double flapPct = v(in, "L:A32NX_APU_FLAP_OPEN_PERCENTAGE");
    const bool intakeMismatch = flapPct != 0 && !b(in, "L:A32NX_OVHD_APU_MASTER_SW_PB_IS_ON");
    if (intakeMismatch && gApu.mismatchSinceMs < 0.0) gApu.mismatchSinceMs = now;
    if (!intakeMismatch) gApu.mismatchSinceMs = -1.0;
    const double flashSince = gApu.mismatchSinceMs >= 0.0 ? now - gApu.mismatchSinceMs - 180000.0 : -1.0;

    TextStyle t = T(36, kWhite);
    t.underline = true;
    text(cv, "APU", 6, 29, t);
    if (b(in, "L:A32NX_OVHD_APU_START_PB_IS_AVAILABLE")) text(cv, "AVAIL", 329, 56, T(34, kGreen, a380::Align::Left, 2.f));

    cv.save();
    cv.translate(78, 107);
    generator(cv, in, 0, 0, 1);
    if (b(in, "L:A32NX_OVHD_APU_MASTER_SW_PB_IS_ON")) {
        text(cv, "%", 92, 43, T(22, kCyan));
        text(cv, "V", 92, 67, T(22, kCyan));
        text(cv, "HZ", 86, 92, T(22, kCyan));
    }
    generator(cv, in, 114, 0, 2);
    cv.restore();

    apu_bleed(cv, in, 558, 136, bleedConfirmed);
    draw_path(cv, svg_path("M29 263 v -29 h 712 v 29"), sk_stroke(kWhite, 4.f, true));
    {
        A429 fu = var429(in, "L:A32NX_APU_FUEL_USED");
        text(cv, "APU FU", 258, 271, T(26, kWhite, a380::Align::Left, 1.f));
        text(cv, std::to_string((int)(std::floor(fu.val / 10.0 + 0.5) * 10)), 416, 263, TM(28, kGreen));
        text(cv, "KG", 474, 271, T(23, kCyan, a380::Align::Left, 2.f));
    }
    n_gauge(cv, in, 165, 346);
    egt_gauge(cv, in, 165, 553);

    A429 lowfp = var429(in, "L:A32NX_APU_LOW_FUEL_PRESSURE_FAULT");
    if (lowfp.val != 0.f) text(cv, "FUEL PRESS LO", 484, 392, T(27, kAmber, a380::Align::Left, 2.f));
    double flap = flapPct;
    if (flap >= 10) {
        uint32_t flapColor = flashSince >= 0.0 && std::fmod(flashSince / 1000.0, 1.0) >= 0.5 ? 0x006600 : kGreen;
        text(cv, flap <= 90 ? "FLAP MOVING" : "FLAP OPEN", 484, 462, T(27, flapColor, a380::Align::Left, 2.f));
    }
}

}  // namespace sdsys
