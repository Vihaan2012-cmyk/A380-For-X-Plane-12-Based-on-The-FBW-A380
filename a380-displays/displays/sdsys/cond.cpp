#include <cstdio>
#include <string>

#include "include/core/SkSamplingOptions.h"

#include "sdsys.h"

namespace sdsys {
namespace {

void path(SkCanvas& cv, const std::string& d, uint32_t c, float w = 2.f) { draw_path(cv, svg_path(d), sk_stroke(c, w)); }

std::string fixed0(double x) {
    char buf[32];
    std::snprintf(buf, sizeof buf, "%.0f", x);
    return buf;
}

std::string pt(float x, float y) { return std::to_string(x) + "," + std::to_string(y); }

void extract_valve(SkCanvas& cv, float x, float y, float value, float mn, float mx, float r, uint32_t color,
                   float sa = 90, float ea = 180) {
    gauge_marker(cv, x, y, value, mn, mx, r, sa, ea, color, 2.f, false, true, false, 0.85f, 1.f);
    circle_fill(cv, x, y, 4, 0x000000);
    circle_stroke(cv, x, y, 4, color, 3.f);
}

void fan(SkCanvas& cv, float x, float y, uint32_t c) {
    path(cv, "M" + pt(x, y) + " l 50,8 l 0,-8 l -50,8 Z", c);
}

const std::string* vcs_name(const a380::Inputs& in, std::string& out) {
    for (int i = 1; i <= 4; i++) {
        out = "L:A32NX_COND_CPIOM_B" + std::to_string(i) + "_VCS_DISCRETE_WORD";
        if (var429(in, out).normal()) return &out;
    }
    out = "L:A32NX_COND_CPIOM_B4_VCS_DISCRETE_WORD";
    return &out;
}

bool tcs_bit(const a380::Inputs& in, int bit) {
    for (int i = 1; i <= 4; i++) {
        std::string n = "L:A32NX_COND_CPIOM_B" + std::to_string(i) + "_TCS_DISCRETE_WORD";
        if (var429(in, n).normal()) return bit429(in, n, bit);
    }
    return bit429(in, "L:A32NX_COND_CPIOM_B4_TCS_DISCRETE_WORD", bit);
}

bool ags_bit(const a380::Inputs& in, int bit) {
    for (int i = 1; i <= 4; i++) {
        std::string n = "L:A32NX_COND_CPIOM_B" + std::to_string(i) + "_AGS_DISCRETE_WORD";
        A429 a = var429(in, n);
        if (a.ssm == 3 || a.ssm == 2) return bit429(in, n, bit);
    }
    return false;
}

void controller(SkCanvas& cv, float x, float y, const char* l1, const char* l2, bool f1, bool f2) {
    if (l1) text(cv, l1, x, y - 25, TM(23, kWhite));
    text(cv, l2, x, y, TM(23, kWhite));
    text(cv, "1", x - 20, y + 25, TM(23, f1 ? kAmber : kGreen));
    text(cv, "2", x + 20, y + 25, TM(23, f2 ? kAmber : kGreen));
    path(cv, "M" + pt(x - 10, y + 15) + " l 0,22 l -25,0", kWhite);
    path(cv, "M" + pt(x + 30, y + 15) + " l 0,22 l -25,0", kWhite);
}

}  // namespace

void draw_cond(SkCanvas& cv, const a380::Inputs& in) {
    std::string vname;
    vcs_name(in, vname);
    bool fwdIso = bit429(in, vname, 14), bulkIso = bit429(in, vname, 16);
    bool fwdFan = bit429(in, vname, 13), bulkFan = bit429(in, vname, 15), cabinFans = bit429(in, vname, 17);
    bool fanFail[4];
    for (int i = 0; i < 4; i++) fanFail[i] = bit429(in, vname, 18 + i);
    A429 vw = var429(in, vname);
    bool bulkHeater = (vw.ssm == 3 || vw.ssm == 2) ? bit429(in, vname, 22) : true;
    bool cabinExtract = b(in, "L:A32NX_VENT_OVERPRESSURE_RELIEF_VALVE_IS_OPEN");
    bool ram = b(in, "L:A32NX_OVHD_COND_RAM_AIR_PB_IS_ON");

    TextStyle t = T(36, kWhite);
    t.underline = true;
    text(cv, "COND", 6, 29, t);
    text(cv, "\xC2\xB0" "C", 100, 34, T(24, kCyan));

    if (auto img = load_image("SD_COND.png"))
        draw_image_two_step(cv, img, SkRect::MakeXYWH(16, 60, 725, 460));
    struct L { const char* s; float x, y; };
    for (L l : {L{"AVNCS", 77, 142}, L{"TO", 458, 152}, L{"TO", 442, 249}, L{"AVNCS", 77, 284}, L{"CRG", 195, 305},
                L{"REST", 340, 305}, L{"CRG", 535, 305}, L{"BULK", 680, 305}, L{"MIX AIR", 395, 501},
                L{"RAM AIR", 395, 530}, L{"1", 330, 585}, L{"2", 465, 585}, L{"HOT AIR", 395, 585}})
        text(cv, l.s, l.x, l.y, TM(23, kWhite));

    path(cv, "M265,294 l 0,-20", kGreen);
    triangle(cv, 265, 310, kGreen, 180, 1.1f);
    path(cv, "M265,294 l 0,-26", kGreen);
    triangle(cv, 265, 310, kGreen, 180, 1.1f);
    path(cv, "M116,235 l 80,0", kGreen);
    triangle(cv, 100, 235, kGreen, 270, 1.1f);
    triangle(cv, 210, 235, kGreen, 90, 1.1f);
    path(cv, "M196,145 l -40,0", kGreen);
    triangle(cv, 210, 145, kGreen, 90, 1.1f);
    path(cv, "M395,484 l 0,-217 l -240,0 l 0,-122", kGreen);
    path(cv, "M300,408 l 0,-126", kWhite);
    path(cv, "M638,395 l 0,-97", kWhite);

    auto temp = [&](const char* n) { return v(in, std::string("L:A32NX_COND_") + n + "_TEMP"); };
    double minMain = temp("MAIN_DECK_1"), maxMain = minMain;
    for (int i = 1; i <= 8; i++) {
        double tv = temp(("MAIN_DECK_" + std::to_string(i)).c_str());
        if (tv > maxMain) maxMain = tv;
        else if (tv < minMain) minMain = tv;
    }
    double minUp = temp("UPPER_DECK_1"), maxUp = minUp;
    for (int i = 1; i <= 7; i++) {
        double tv = temp(("UPPER_DECK_" + std::to_string(i)).c_str());
        if (tv > maxUp) maxUp = tv;
        else if (tv < minUp) minUp = tv;
    }
    text(cv, fixed0(v(in, "L:A32NX_COND_CKPT_TEMP")), 65, 240, T(26, kGreen));
    text(cv, fixed0(minUp), 386, 158, T(26, kGreen));
    text(cv, fixed0(maxUp), 498, 158, T(26, kGreen));
    text(cv, fixed0(minMain), 373, 255, T(26, kGreen));
    text(cv, fixed0(maxMain), 482, 255, T(26, kGreen));
    if (b(in, "L:A32NX_COND_TADD_CHANNEL_1_FAILURE") || b(in, "L:A32NX_COND_TADD_CHANNEL_2_FAILURE"))
        controller(cv, 680, 20, nullptr, "TEMP CTL", b(in, "L:A32NX_COND_TADD_CHANNEL_1_FAILURE"),
                   b(in, "L:A32NX_COND_TADD_CHANNEL_2_FAILURE"));

    text(cv, fixed0(v(in, "L:A32NX_COND_CARGO_FWD_TEMP")), 250, 360, T(26, kGreen));
    text(cv, fixed0(v(in, "L:A32NX_COND_CARGO_BULK_TEMP")), 660, 360, T(26, kGreen));
    if (bulkHeater) text(cv, "HEATER", 680, 329, TM(22, kAmber));

    text(cv, "VENT", 49, 313, T(23, kGreen));
    for (int k = 0; k < 2; k++) {
        const char* vcm = k == 0 ? "FWD" : "AFT";
        bool f1 = b(in, std::string("L:A32NX_VENT_") + vcm + "_VCM_CHANNEL_1_FAILURE");
        bool f2 = b(in, std::string("L:A32NX_VENT_") + vcm + "_VCM_CHANNEL_2_FAILURE");
        if (f1 || f2) controller(cv, k == 0 ? 49 + 31 : 49 + 641, 313 + 257, vcm, "VENT CTL", f1, f2);
    }

    extract_valve(cv, 248, 408, fwdIso ? 1.f : 0.f, 0, 3, 42, fwdIso ? kGreen : kAmber);
    extract_valve(cv, 548, 408, bulkIso ? 1.f : 0.f, 0, 3, 42, bulkIso ? kGreen : kAmber);
    extract_valve(cv, 724, 97, cabinExtract ? 1.f : 0.f, 0, 1, 42, kGreen, 172, 140);
    extract_valve(cv, 327, 512, ram ? 1.f : 0.f, 0, 1, 42, kGreen, 270, 240);
    extract_valve(cv, 505, 512, ram ? 1.f : 0.f, 0, 1, 42, kGreen, 270, 240);
    extract_valve(cv, 96, 457, 0, 0, 3, 42, kGreen);
    extract_valve(cv, 154, 446, 0, 0, 1, 42, kGreen, 90, 180);

    for (int n = 1; n <= 2; n++) {
        float x = n == 1 ? 300 : 490, y = 649;
        bool anyPack = b(in, "L:A32NX_COND_PACK_" + std::to_string(n) + "_FLOW_VALVE_1_IS_OPEN");
        bool hot = tcs_bit(in, 14 + n), dis = tcs_bit(in, 12 + n);
        path(cv, "M" + pt(x, y) + " l 0,-30", anyPack ? kGreen : kAmber);
        path(cv, "M" + pt(x, y - 58) + " l 0,-16", hot ? kGreen : kAmber);
        triangle(cv, x, y - 90, hot ? kGreen : kAmber, 0, 1.1f);
        valve(cv, x, y - 39, 17, hot, (hot && !dis) ? kGreen : kAmber);
    }
    for (int n = 1; n <= 2; n++) {
        float x = n == 1 ? 200 : 580, y = 600;
        bool op = ags_bit(in, 12 + n);
        path(cv, "M" + pt(x, y) + " l 0,-70", op ? kGreen : kAmber);
        triangle(cv, x, y - 88, op ? kGreen : kAmber, 0, 1.1f);
        text(cv, "PACK", x - 35, y + 30, T(23, op ? kWhite : kAmber));
        text(cv, std::to_string(n), x + 30, y + 30, T(26, op ? kGreen : kAmber));
    }

    bool anyFail = fanFail[0] || fanFail[1] || fanFail[2] || fanFail[3];
    if (!(cabinFans && !anyFail)) {
        fan(cv, 315, 415, cabinFans && !fanFail[0] ? kGreen : kAmber);
        fan(cv, 315, 430, cabinFans && !fanFail[1] ? kGreen : kAmber);
        fan(cv, 435, 415, cabinFans && !fanFail[2] ? kGreen : kAmber);
        fan(cv, 435, 430, cabinFans && !fanFail[3] ? kGreen : kAmber);
        uint32_t c = cabinFans ? kGreen : kAmber;
        path(cv, "M340,468 l 0,-32", c);
        triangle(cv, 340, 484, c, 180, 1.1f);
        path(cv, "M460,468 l 0,-32", c);
        triangle(cv, 460, 484, c, 180, 1.1f);
    }
    if (!fwdFan) fan(cv, 240, 390, kAmber);
    if (!bulkFan) fan(cv, 540, 390, kAmber);
}

}  // namespace sdsys
