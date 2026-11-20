#include <cstdio>
#include <string>

#include "include/core/SkPaint.h"
#include "include/core/SkPathBuilder.h"

#include "sdsys.h"

namespace sdsys {
namespace {

constexpr auto End = a380::Align::Right;
constexpr auto Mid = a380::Align::Center;

TextStyle ts(float px, uint32_t c, a380::Align al = a380::Align::Left) {
    TextStyle s;
    s.px = px;
    s.color = c;
    s.align = al;
    if (al == a380::Align::Center) s.base = a380::Baseline::Middle;
    return s;
}

void arc(SkCanvas& cv, float x, float y, float r, float startAngle, float endAngle, uint32_t color, float width) {
    arc_path(cv, x, y, r, startAngle, endAngle, color, width);
}

void marker(SkCanvas& cv, float x, float y, float value, float mn, float mx, float r, uint32_t color, float width,
            bool indicator = false, float inner = 0.85f, float outer = 1.15f) {
    float pct = (value - mn) / (mx - mn);
    float ang = (-90.f + pct * 180.f - 90.f) * 3.14159265f / 180.f;
    float dx = std::cos(ang), dy = std::sin(ang);
    float e = indicator ? outer : 1.f;
    SkPaint p;
    p.setAntiAlias(true);
    p.setStyle(SkPaint::kStroke_Style);
    p.setColor(a380::rgb(color));
    p.setStrokeWidth(width);
    if (indicator) p.setStrokeCap(SkPaint::kRound_Cap);
    draw_line(cv, x + dx * r * inner, y + dy * r * inner, x + dx * r * e, y + dy * r * e, p);
}

void decimal(SkCanvas& cv, float x, float y, double value, bool active, float shift = 0.f) {
    if (value < 0) value = 0;
    if (!active) {
        text(cv, "XX", x, y - 4, ts(29, kAmber, Mid));
        return;
    }
    char buf[32];
    std::snprintf(buf, sizeof buf, "%.1f", value);
    std::string s = buf;
    size_t dot = s.find('.');
    float sx = x + shift;
    text(cv, s.substr(0, dot), sx + 8, y, ts(29, kGreen, End));
    text(cv, ".", sx + 14, y - 6, ts(25, kGreen, Mid));
    text(cv, s.substr(dot + 1), sx + 22, y, ts(25, kGreen));
}

void dash(SkCanvas& cv, int engine, float x, float y) {
    line(cv, engine > 2 ? x - 96 : x + 64, y, (engine > 2 ? x - 96 : x + 64) + 26, y, kWhite, 2.f);
}

void column(SkCanvas& cv, const a380::Inputs& in, float x, float y, int engine, bool ignition, bool anyRunning) {
    std::string e = std::to_string(engine);
    double n2 = v(in, "L:A32NX_ENGINE_N2_DISPLAY:" + e), n3 = v(in, "L:A32NX_ENGINE_N3_DISPLAY:" + e);
    bool starterOpen = b(in, "L:A32NX_PNEU_ENG_" + e + "_STARTER_VALVE_OPEN");
    bool starting = n2 < 58.5 && ignition && starterOpen;
    bool fadecMan = b(in, "L:A32NX_OVHD_FADEC_" + e);
    bool fire = b(in, "L:A32NX_FIRE_BUTTON_ENG" + e);
    bool separated = b(in, "L:A32NX_ENGINE_SEPARATED:" + e);
    bool powered = (ignition || anyRunning || fadecMan) && !fire && !separated;
    double ff = v(in, "L:A32NX_ENGINE_FF:" + e);
    double vib = v(in, "TURB ENG VIBRATION:" + e);
    double oilQty = v(in, "L:A32NX_ENGINE_OIL_QTY:" + e);
    double oilTemp = v(in, "GENERAL ENG OIL TEMPERATURE:" + e);
    double oilPress = v(in, "ENG OIL PRESSURE:" + e);
    int engState = (int)v(in, "L:A32NX_ENGINE_STATE:" + e);
    double n1 = v(in, "L:A32NX_ENGINE_N1:" + e), idle = v(in, "L:A32NX_ENGINE_IDLE_N1");

    if (ignition && n1 < std::floor(idle) - 1 && engState == 2) {
        SkPath a = svg_path("m " + std::to_string((int)(x - 63)) + " " + std::to_string((int)(y + 97)) + " l 0,-127 l 120,0 l 0,127");
        SkPath c = svg_path("m " + std::to_string((int)(x - 63)) + " " + std::to_string((int)(y + 458)) + " l 0,127 l 120,0 l 0,-127");
        stroke_path(cv, a, kWhite, 2.f);
        stroke_path(cv, c, kWhite, 2.f);
    }

    decimal(cv, x, y, n2, powered);
    dash(cv, engine, x, y - 10);
    if (starting) {
        SkPaint p;
        p.setColor(a380::rgb(kLightGrey, 51));
        draw_rect(cv, SkRect::MakeXYWH(x - 55, y + 10, 98, 34), p);
    }
    decimal(cv, x, y + 38, n3, powered);
    dash(cv, engine, x, y + 28);
    if (!powered) text(cv, "XX", x, y + 86, ts(29, kAmber, Mid));
    else text(cv, std::to_string((int)(std::ceil(ff / 10.0) * 10)), x + 30, y + 92, ts(29, kGreen, End));
    dash(cv, engine, x, y + 82);

    float oy = y + 206;
    arc(cv, x, oy, 53, -90, 90, kWhite, 2.f);
    if (!powered) {
        text(cv, "XX", x, oy - 4, ts(29, kAmber, Mid));
    } else {
        marker(cv, x, oy, 0, 0, 18.3f, 53, kWhite, 2.f);
        marker(cv, x, oy, 3.7f, 0, 18.3f, 53, kWhite, 2.f);
        marker(cv, x, oy, 18.3f / 2, 0, 18.3f, 53, kWhite, 2.f);
        marker(cv, x, oy, 18.3f, 0, 18.3f, 53, kWhite, 2.f);
        marker(cv, x, oy, (float)oilQty, 0, 18.3f, 53, kGreen, 4.f, true, 0.8f, 1.2f);
    }
    decimal(cv, x, oy, oilQty, powered);
    if (!powered) text(cv, "XX", x, y + 242, ts(29, kAmber, Mid));
    else
        text(cv, std::to_string(oilTemp < 0 ? 0 : js_round(oilTemp)), x + 20, y + 248,
             ts(29, oilTemp > 177 ? kAmber : kGreen, End));

    float py = y + 320;
    arc(cv, x, py, 45, -90, 90, kWhite, 2.f);
    if (!powered) {
        text(cv, "XX", x, py - 4, ts(29, kAmber, Mid));
    } else {
        arc(cv, x, py, 44, -90, -68, kRed, 6.f);
        marker(cv, x, py, 0, 0, 500, 45, kWhite, 2.f);
        marker(cv, x, py, 250, 0, 500, 45, kWhite, 2.f);
        marker(cv, x, py, 500, 0, 500, 45, kWhite, 2.f);
        double nv = oilPress <= 100 ? (225.0 / 100.0) * oilPress : 225 + (225.0 / 400.0) * oilPress;
        marker(cv, x, py, (float)js_round(nv), 0, 500, 45, oilPress < 25 ? kRed : kGreen, 4.f, true, 0.8f, 1.2f);
        text(cv, std::to_string(oilPress < 0 ? 0 : js_round(oilPress)), x + 28, py,
             ts(29, oilPress < 25 ? kRed : kGreen, End));
    }

    decimal(cv, x, y + 380, vib, powered, -14);
    dash(cv, engine, x, y + 370);
    decimal(cv, x, y + 416, vib, powered, -14);
    dash(cv, engine, x, y + 406);
    decimal(cv, x, y + 450, vib, powered, -14);
    dash(cv, engine, x, y + 440);

    float ny = y + 536;
    if (starting || ignition) {
        double pr = v(in, "L:A32NX_PNEU_ENG_" + e + "_REGULATED_TRANSDUCER_PRESSURE");
        double n2raw = v(in, "L:A32NX_ENGINE_N2:" + e);
        bool igniter = n2raw > 9 && n2raw < 25;
        SkPaint p;
        p.setAntiAlias(true);
        p.setStyle(SkPaint::kStroke_Style);
        p.setColor(a380::rgb(kGreen));
        p.setStrokeWidth(2.f);
        circle_stroke(cv, x, ny - 14, 24, kGreen, 2.f);
        if (starterOpen) draw_line(cv, x, ny - 38, x, ny + 10, p);
        else draw_line(cv, x - 24, ny - 14, x + 24, ny - 14, p);
        if (igniter && starterOpen) text(cv, "A", x - 10, ny - 60, ts(25, kGreen, Mid));
        text(cv, std::to_string(js_round(pr)), x, ny + 38, ts(29, (pr < 15 || pr > 60) ? kAmber : kGreen, Mid));
        stroke_path(cv, svg_path("M" + std::to_string((int)x) + "," + std::to_string((int)(ny + 11)) + " l 0,10"), kGreen, 2.f);
        if (starterOpen)
            stroke_path(cv, svg_path("M" + std::to_string((int)x) + "," + std::to_string((int)(ny - 37)) + " l 0,-10"), kGreen, 2.f);
        if (engine == 2) {
            text(cv, "IGN", x + 180, ny - 50, ts(25, kWhite, End));
            text(cv, "PSI", x + 180, ny + 44, ts(25, kCyan, End));
        }
    }
    if (!starterOpen && !ignition) {
        arc(cv, x, ny, 45, -90, 90, kWhite, 2.f);
        marker(cv, x, ny, 0, 0, 500, 45, kWhite, 2.f);
        marker(cv, x, ny, 500, 0, 500, 45, kWhite, 2.f);
        if (!powered) {
            text(cv, "XX", x, ny - 4, ts(29, kAmber, Mid));
        } else {
            marker(cv, x, ny, 300, 0, 500, 45, kWhite, 2.f);
            marker(cv, x, ny, 240, 0, 500, 45, kGreen, 4.f, true, 0.6f, 1.25f);
        }
        if (engine == 1) text(cv, "0", x - 50, ny + 29, ts(21, kWhite));
        if (engine == 4) text(cv, "500", x + 15, ny + 29, ts(21, kWhite));
        if (engine == 2) {
            text(cv, "NAC", x + 180, ny - 36, ts(25, kWhite, End));
            text(cv, "\xC2\xB0" "C", x + 170, ny - 10, ts(25, kCyan, End));
        }
    }
}

}  // namespace

void draw_eng(SkCanvas& cv, const a380::Inputs& in) {
    TextStyle t = ts(36, kWhite);
    t.underline = true;
    text(cv, "ENGINE", 6, 29, t);
    int sel = (int)v(in, "L:XMLVAR_ENG_MODE_SEL");
    bool ign = sel == 2;
    bool any = false;
    for (int i = 1; i <= 4; i++) any = any || v(in, "L:A32NX_ENGINE_STATE:" + std::to_string(i)) > 0;
    column(cv, in, 67, 68, 1, ign, any);
    column(cv, in, 230, 68, 2, ign, any);
    column(cv, in, 543, 68, 3, ign, any);
    column(cv, in, 701, 68, 4, ign, any);

    text(cv, "N2", 388, 68, ts(25, kWhite, End));
    text(cv, "%", 414, 68, ts(25, kCyan, End));
    text(cv, "N3", 388, 108, ts(25, kWhite, End));
    text(cv, "%", 414, 108, ts(25, kCyan, End));
    text(cv, "FF", 388, 148, ts(25, kWhite, End));
    text(cv, "KG/H", 410, 182, ts(25, kCyan, End));
    text(cv, "OIL", 410, 240, ts(25, kWhite, End));
    text(cv, "QT", 398, 276, ts(25, kCyan, End));
    text(cv, "\xC2\xB0" "C", 398, 316, ts(25, kCyan, End));
    text(cv, "PSI", 410, 386, ts(25, kCyan, End));
    text(cv, "VIB", 388, 452, ts(25, kWhite, End));
    text(cv, "N1", 428, 452, ts(25, kWhite, End));
    text(cv, "N2", 428, 486, ts(25, kWhite, End));
    text(cv, "N3", 428, 520, ts(25, kWhite, End));
}

}  // namespace sdsys
