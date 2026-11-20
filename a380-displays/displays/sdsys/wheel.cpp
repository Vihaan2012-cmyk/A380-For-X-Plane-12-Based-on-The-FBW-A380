#include <algorithm>
#include <string>

#include "include/core/SkPaint.h"

#include "sdsys.h"

namespace sdsys {
namespace {

constexpr auto End = a380::Align::Right;

enum class Pos { Nose, Left, Right };
enum class Type { Nose, WLG, BLG };

TextStyle ts(float px, uint32_t c, a380::Align al = a380::Align::Left, float ls = 0.f) {
    TextStyle s;
    s.px = px;
    s.color = c;
    s.align = al;
    s.ls = ls;
    return s;
}

SkPaint stroke_paint(uint32_t c, float w, bool round = false) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(a380::rgb(c));
    p.setStyle(SkPaint::kStroke_Style);
    p.setStrokeWidth(w);
    if (round) {
        p.setStrokeCap(SkPaint::kRound_Cap);
        p.setStrokeJoin(SkPaint::kRound_Join);
    }
    return p;
}

void circle(SkCanvas& cv, float x, float y, float r, uint32_t c, float w) { circle_stroke(cv, x, y, r, c, w); }

std::string fnum(float v) {
    char b[32];
    std::snprintf(b, sizeof b, "%g", v);
    return b;
}

void power_supply(SkCanvas& cv, const a380::Inputs& in, float x, float y, bool eha, const std::string& source) {
    bool hyd = source == "GREEN" || source == "YELLOW";
    bool avail = b(in, hyd ? "L:A32NX_HYD_" + source + "_SYSTEM_1_SECTION_PRESSURE_SWITCH"
                           : "L:A32NX_ELEC_" + source + "_BUS_IS_POWERED");
    uint32_t col = avail ? kGreen : kAmber;
    cv.save();
    cv.translate(x, y);
    fill_path(cv, svg_path("m0,0 h 21 v 21 h-21 z"), kGrey);
    if (!eha) text(cv, source == "GREEN" ? "G" : "Y", 5, 19, ts(22, col));
    else stroke_path(cv, svg_path("m14,4 l -7,7 h 7 l-7,7"), col, 4.f, true);
    cv.restore();
}

void position_indicator(SkCanvas& cv, int system, uint32_t color) {
    if (system == 1) {
        stroke_path(cv, svg_path("m 0 0 h 29 v 26 z"), color, 2.f);
        stroke_path(cv, svg_path("M 13 0 v 12 M 21 0 v 19"), color, 2.f);
    } else {
        stroke_path(cv, svg_path("m 67 0 h -29 v 26 z"), color, 2.f);
        stroke_path(cv, svg_path("M 46 0 v 19 M 54 0 v 12"), color, 2.f);
    }
}

void gear_door(SkCanvas& cv, const a380::Inputs& in, Pos pos, Type type) {
    const char* nm = pos == Pos::Nose ? "CENTER" : pos == Pos::Left ? "LEFT" : "RIGHT";
    double dp = v(in, std::string("L:A32NX_GEAR_DOOR_") + nm + "_POSITION");
    bool transit = dp > 2;
    uint32_t c = transit ? kAmber : kGreen;
    float yp = type == Type::BLG ? -8.f : -6.f;
    auto rot = [&](float deg, float px, float py, const std::string& d) {
        cv.save();
        cv.rotate(deg, px, py);
        stroke_path(cv, svg_path(d), c, 2.f);
        cv.restore();
    };
    if (type == Type::BLG || type == Type::Nose) {
        rot(transit ? 55.f : 0.f, -15, -8, "m -14 " + fnum(yp) + " h 46");
        rot(transit ? -55.f : 0.f, 82, -8, "m 81 " + fnum(yp) + " h -46");
        circle(cv, -15, yp, 3, c, 2.f);
        circle(cv, 82, yp, 3, c, 2.f);
    } else {
        bool l = pos == Pos::Left;
        rot(transit ? (l ? -55.f : 55.f) : 0.f, l ? 84.f : -17.f, -7, std::string("m ") + (l ? "83" : "-16") + " -7 l " + (l ? "-" : "") + "94 -5");
        circle(cv, l ? 84.f : -17.f, -7, 3, c, 2.f);
    }
}

void gear(SkCanvas& cv, const a380::Inputs& in, float x, float y, Pos pos, Type type) {
    const char* nm = pos == Pos::Nose ? "CENTER" : pos == Pos::Left ? "LEFT" : "RIGHT";
    std::string src = (type == Type::WLG || type == Type::Nose) ? "GREEN" : "YELLOW";
    double gp = v(in, std::string("L:A32NX_GEAR_") + nm + "_POSITION");
    cv.save();
    cv.translate(x, y);
    power_supply(cv, in, 23, type == Type::Nose ? -32 : -36, false, src);
    if (gp >= 2) {
        uint32_t c = gp > 98 ? kGreen : kRed;
        position_indicator(cv, 1, c);
        position_indicator(cv, 2, c);
    }
    gear_door(cv, in, pos, type);
    cv.restore();
}

void wheel(SkCanvas& cv, const a380::Inputs& in, float x, float y, int number, bool left, bool hasBrake) {
    std::string ns = left ? "-" : "", rs = left ? "" : "-";
    cv.save();
    cv.translate(x, y);
    stroke_path(cv, svg_path("m" + ns + "51,-28 v-10 c" + rs + "10,-6 " + rs + "30,-5, " + rs + "35,0 v10"), kGrey, 2.f);
    stroke_path(cv, svg_path("m" + ns + "51,40 c" + rs + "10,4 " + rs + "30,3, " + rs + "35,-1 v-10"), kGrey, 2.f);
    if (hasBrake) {
        stroke_path(cv, svg_path("m " + ns + "15,18 v -36 M " + ns + "21,18 v -36 M " + ns + "27,18 v -36 M " + ns + "33,18 v -36"), kGrey, 2.f);
        fill_path(cv, svg_path(std::string("m ") + (left ? "-18" : "70") + ",-13 h -50 v -24 h 50 z"), 0x000000);
        double t = v(in, "L:A32NX_REPORTED_BRAKE_TEMPERATURE_" + std::to_string(number));
        int rt = (int)std::min(995.0, std::max(0.0, std::floor(t / 5.0 + 0.5) * 5.0));
        text(cv, std::to_string(rt), left ? -16 : 72, -16, ts(26, t > 300 ? kAmber : kGreen, End));
    }
    if (number > 0) {
        TextStyle s = ts(22, kWhite, left ? End : a380::Align::Left);
        text(cv, std::to_string(number), left ? -38 : 42, 7, s);
    }
    cv.restore();
}

void bogey(SkCanvas& cv, const a380::Inputs& in, float x, float y, Pos pos, Type type) {
    int n = type == Type::Nose ? 2 : type == Type::WLG ? 4 : 6;
    cv.save();
    cv.translate(x, y);
    if (type != Type::Nose)
        stroke_path(cv, svg_path(type == Type::WLG ? "m0,0 v 92 M -13,0 h 26 M -13,92 h 26" : "m0,0 v 176 M -13,0 h 26 M -13,91 h 26"), kGrey, 4.f);
    if (type == Type::Nose) {
        cv.save();
        cv.translate(0, 10);
        SkPaint p = stroke_paint(kGreen, 3.f, true);
        draw_path(cv, svg_path("m -19,0 h 13 M 19,0 h -13"), p);
        circle(cv, 0, 0, 5, kGreen, 3.f);
        power_supply(cv, in, -23, -30, false, "GREEN");
        power_supply(cv, in, 2, -30, true, "AC_3");
        cv.restore();
    }
    if (type == Type::BLG) {
        cv.save();
        cv.translate(0, 181);
        draw_path(cv, svg_path("m -10,0 h 4 M 10,0 h -4"), stroke_paint(kGreen, 3.f, true));
        circle(cv, 0, 0, 4, kGreen, 3.f);
        power_supply(cv, in, -11, -32, false, "YELLOW");
        cv.restore();
    }
    for (int i = 0; i < n; i++) {
        int wn;
        float xo = 0;
        if (type == Type::WLG) wn = i + (i / 2) * 2 + (pos == Pos::Left ? 1 : 3);
        else if (type == Type::BLG) wn = i + (i / 2) * 2 + (pos == Pos::Left ? 9 : 11);
        else { wn = i; xo = i == 0 ? 8.f : -5.f; }
        wheel(cv, in, xo, (i / 2) * (type == Type::BLG ? 90.f : 92.f), type != Type::Nose ? wn : 0, wn % 2 != 0,
              wn < 17 && type != Type::Nose);
    }
    auto accu = [&](float ax, float ay) {
        const char* l[] = {"A", "C", "C", "U"};
        for (int i = 0; i < 4; i++) text(cv, l[i], x - x + ax, ay + 22.f * i, ts(24, kGreen));
    };
    if (type == Type::BLG || type == Type::WLG) accu(-6, type == Type::BLG ? 24.f : -22.f);
    if (type == Type::WLG) accu(-6, 70.f);
    cv.restore();
}

void braking_supply(SkCanvas& cv, const a380::Inputs& in, float x, float y, Type type) {
    cv.save();
    cv.translate(x, y);
    power_supply(cv, in, -23, -30, false, type == Type::WLG ? "GREEN" : "YELLOW");
    power_supply(cv, in, 1, -30, true, type == Type::WLG ? "AC_4" : "AC_2");
    cv.restore();
}

}  // namespace

void draw_wheel(SkCanvas& cv, const a380::Inputs& in) {
    TextStyle t = ts(36, kWhite);
    t.underline = true;
    text(cv, "WHEEL", 6, 29, t);
    {
        TextStyle d = ts(24, kWhite, a380::Align::Left, -8.f);
        text(cv, "...", 130, 28, d);
        text(cv, "MORE", 157, 28, ts(26, kWhite));
    }
    gear(cv, in, 351, 59, Pos::Nose, Type::Nose);
    stroke_path(cv, svg_path("m 5,207 l 31,4 m 98,7 c 3,14 14,30 27,34 h 43 m 106,0 h 149 m 106,0 h43 m27,-34 c -3,14 -14,30 -27,34 m 27,-34 m 98,-7 l 31,-4"), kGrey, 2.f);
    gear(cv, in, 48, 223, Pos::Left, Type::WLG);
    gear(cv, in, 223, 260, Pos::Left, Type::BLG);
    gear(cv, in, 479, 260, Pos::Right, Type::BLG);
    gear(cv, in, 654, 223, Pos::Right, Type::WLG);

    text(cv, "\xC2\xB0" "C", 370, 580, ts(24, kCyan));
    text(cv, "PSI", 367, 634, ts(22, kCyan));
    bogey(cv, in, 384, 140, Pos::Nose, Type::Nose);
    bogey(cv, in, 81, 403, Pos::Left, Type::WLG);
    bogey(cv, in, 257, 417, Pos::Left, Type::BLG);
    bogey(cv, in, 513, 417, Pos::Right, Type::BLG);
    bogey(cv, in, 690, 403, Pos::Right, Type::WLG);

    text(cv, "BRK", 366, 320, ts(22, kWhite));
    stroke_path(cv, svg_path("m 145,311 h 46 m 110,0 h 40 m 88,0 h 40 m 110,0 h 46"), kWhite, 2.f);
    braking_supply(cv, in, 81, 328, Type::WLG);
    braking_supply(cv, in, 258, 328, Type::BLG);
    braking_supply(cv, in, 513, 328, Type::BLG);
    braking_supply(cv, in, 690, 328, Type::WLG);
}

}  // namespace sdsys
