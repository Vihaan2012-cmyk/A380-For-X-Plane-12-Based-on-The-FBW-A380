#include <string>

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

std::string num0(double x) { return std::to_string(js_round(x)); }
uint32_t norm(bool ok) { return ok ? kGreen : kAmber; }
bool contactor(const a380::Inputs& in, const std::string& n) { return b(in, "L:A32NX_ELEC_CONTACTOR_" + n + "_IS_CLOSED"); }

void battery(SkCanvas& cv, const a380::Inputs& in, float x, float y, int bus) {
    std::string title, lvar;
    if (bus <= 2) { title = "BAT " + std::to_string(bus); lvar = std::to_string(bus); }
    else if (bus == 3) { title = "ESS BAT"; lvar = "ESS"; }
    else { title = "APU BAT"; lvar = "APU"; }
    double pot = v(in, "L:A32NX_ELEC_BAT_" + std::to_string(bus) + "_POTENTIAL");
    double cur = v(in, "L:A32NX_ELEC_BAT_" + std::to_string(bus) + "_CURRENT");
    bool pb = b(in, "L:A32NX_OVHD_ELEC_BAT_" + lvar + "_PB_IS_AUTO");
    bool curOk = true, voltOk = true;
    if (bus != 4) {
        curOk = cur > -5;
        voltOk = pot >= 20 && (pot > 25 || cur <= 0);
    }
    cv.save();
    cv.translate(x, y);
    stroke_path(cv, svg_path("M 0,0 l 0,107 l 127,0 l 0,-107 z"), kLightGrey, 3.f);
    TextStyle t = ts(25, (curOk && voltOk && pb) ? kWhite : kAmber, Mid, 1.f);
    t.ws = -8.f;
    text(cv, title, 65, 22, t);
    if (pb) {
        text(cv, "V", 77, 63, ts(22, kCyan));
        text(cv, "A", 77, 97, ts(22, kCyan));
        text(cv, num0(pot), 73, 62, ts(28, norm(voltOk), End, 1.f));
        text(cv, num0(cur), 73, 97, ts(28, norm(curOk), End, 1.f));
    } else {
        text(cv, "OFF", 44, 75, ts(25, kWhite));
    }
    cv.restore();
}

void bus_bar(SkCanvas& cv, const a380::Inputs& in, float x, float y, int bus) {
    bool one2 = bus <= 2;
    std::string name = one2 ? std::to_string(bus) : bus == 3 ? "ESS" : "APU";
    bool powered = b(in, std::string("L:A32NX_ELEC_") + (name == "APU" ? "" : "DC_") + (name == "APU" ? "309PP" : name) + "_BUS_IS_POWERED");
    cv.save();
    cv.translate(x, y);
    SkPath box = svg_path("M 0,0 l 0,34 l 128,0 l 0,-34 z");
    fill_path(cv, box, kGrey);
    stroke_path(cv, box, kLightGrey, 1.f);
    uint32_t col = norm(powered);
    text(cv, "DC", one2 ? 32 : 9, 29, ts(29, col));
    text(cv, name, one2 ? 78 : 65, one2 ? 31 : 29, ts(one2 ? 35 : 29, col, a380::Align::Left, 1.f));
    if (bus == 3) {
        triangle(cv, 98, 53, kWhite, 180);
        TextStyle t = ts(22, kWhite, a380::Align::Left, 1.f);
        t.ws = -8.f;
        text(cv, "STAT INV", 70, 77, t);
    }
    cv.restore();
}

void bat_wire(SkCanvas& cv, const a380::Inputs& in, float x, float y, int bus) {
    double cur = v(in, "L:A32NX_ELEC_BAT_" + std::to_string(bus) + "_CURRENT");
    uint32_t col = (cur > 0 || bus == 4) ? kGreen : kAmber;
    bool charging = cur > 0, discharging = cur < 0;
    std::string cn = bus <= 2 ? "990PB" + std::to_string(bus) : bus == 3 ? "6PB3" : "5PB";
    bool closed = contactor(in, cn);
    cv.save();
    cv.translate(x, y);
    if (bus == 1) {
        bool emer = contactor(in, "6PC1");
        float tx = 0, ty = 0, to = 0;
        if (charging) { tx = 64; ty = -164; to = 0; }
        else if (discharging && closed) { tx = 64; ty = 0; to = 180; }
        else if (discharging && emer) { tx = 154; ty = -92; to = 90; }
        if ((charging || discharging) && (emer || closed)) triangle(cv, tx, ty, col, to);
        if (closed)
            stroke_path(cv, svg_path(std::string("M 64,") + (discharging ? "-14" : "0") + " l 0," + (discharging ? "-78" : "-92")), col, 2.f);
        if (emer || closed) stroke_path(cv, svg_path(std::string("M 64,-92 l 0,") + (charging ? "-58" : "-76")), col, 2.f);
        if (emer) stroke_path(cv, svg_path(std::string("M 64,-92 l ") + (discharging ? "76" : "90") + ",0"), col, 2.f);
    } else if (closed) {
        std::string path;
        float tx, ty;
        if (bus == 3) {
            path = std::string("M 93,") + (discharging ? "-14" : "0") + " l 0," + ((charging || discharging) ? "-47" : "-65");
            tx = 93;
            ty = charging ? -61 : 0;
        } else {
            path = std::string("M 64,") + (discharging ? "-14" : "0") + "l 0," + ((charging || discharging) ? "-150" : "-167");
            tx = 64;
            ty = charging ? -164 : 0;
        }
        if (charging || discharging) triangle(cv, tx, ty, col, charging ? 0 : 180);
        stroke_path(cv, svg_path(path), col, 2.f);
    }
    cv.restore();
}

void tr_box(SkCanvas& cv, const a380::Inputs& in, float x, float y, int bus) {
    std::string sb = std::to_string(bus);
    double pot = v(in, "L:A32NX_ELEC_TR_" + sb + "_POTENTIAL"), cur = v(in, "L:A32NX_ELEC_TR_" + sb + "_CURRENT");
    std::string title = bus <= 2 ? "TR " + sb : bus == 3 ? "ESS TR" : "APU TR";
    std::string acTitle, acL;
    if (bus == 1) { acTitle = "AC 2"; acL = "2"; }
    else if (bus == 3) {
        acL = "ESS_SHED";
        if (contactor(in, "3XC1")) acTitle = "AC 1";
        else if (contactor(in, "3XC2")) acTitle = "AC 4";
        else if (contactor(in, "5XE")) acTitle = "EMER GEN";
        else acTitle = "AC 1";
    } else if (bus == 2) { acTitle = "AC 3"; acL = "3"; }
    else { acTitle = "AC 4"; acL = "4"; }
    bool acPow = b(in, "L:A32NX_ELEC_AC_" + acL + "_BUS_IS_POWERED");
    bool curOk = cur > 2, voltOk = pot > 27 && pot < 32;
    cv.save();
    cv.translate(x, y);
    stroke_path(cv, svg_path("M 0,0 l 0,106 l 112,0 l 0,-106 z"), kLightGrey, 3.f);
    TextStyle t = ts(25, (voltOk && curOk) ? kWhite : kAmber, Mid, 1.f);
    t.ws = -8.f;
    text(cv, title, 57, 22, t);
    text(cv, "V", 70, 63, ts(22, kCyan));
    text(cv, "A", 70, 97, ts(22, kCyan));
    text(cv, num0(pot), 65, 62, ts(28, norm(voltOk), End, 1.f));
    text(cv, num0(cur), 65, 97, ts(28, norm(curOk), End, 1.f));
    uint32_t ac = acPow ? kWhite : kAmber;
    triangle(cv, 56, 112, ac, 0);
    TextStyle a = ts(25, ac, Mid, 1.f);
    a.ws = -8.f;
    text(cv, acTitle, 58, 144, a);
    cv.restore();
}

void network(SkCanvas& cv, const a380::Inputs& in, float x, float y, int bus) {
    std::string trc = bus <= 2 ? "990PU" + std::to_string(bus) : bus == 3 ? "6PE" : "7PU";
    bool trClosed = contactor(in, trc);
    float xtr, ytr, xb, yb;
    if (bus == 1) { xtr = -12; ytr = 388; xb = -20; yb = 276; }
    else if (bus == 3) { xtr = -22; ytr = 334; xb = -30; yb = 173; }
    else { xtr = 8; ytr = 388; xb = 0; yb = 276; }
    cv.save();
    cv.translate(x, y);
    battery(cv, in, 0, 0, bus);
    bus_bar(cv, in, xb, yb, bus);
    bat_wire(cv, in, xb, yb, bus);
    if (trClosed)
        stroke_path(cv, svg_path("M " + std::to_string((int)(xtr + 56)) + "," + std::to_string((int)ytr) + " l 0,-" + (bus == 3 ? "127" : "78")),
                    kGreen, 2.f);
    tr_box(cv, in, xtr, ytr, bus);
    cv.restore();
}

}  // namespace

void draw_elec_dc(SkCanvas& cv, const a380::Inputs& in) {
    TextStyle t = ts(36, kWhite);
    t.underline = true;
    text(cv, "ELEC DC", 6, 29, t);
    network(cv, in, 37, 91, 1);
    network(cv, in, 199, 91, 3);
    if (contactor(in, "980PC"))
        stroke_path(cv, svg_path("M 145,384 l 60,0 m-10,10 l 20,-20 m -10,10 m 60,0 m-10,10 l 20,-20 m -10,10 l 132,0"), kGreen, 2.f);
    if (contactor(in, "6PC2")) stroke_path(cv, svg_path("M 120,367 l 0,-80 l 49,0"), kGreen, 2.f);
    network(cv, in, 397, 91, 2);
    network(cv, in, 625, 91, 4);
}

void draw_cb(SkCanvas& cv, const a380::Inputs&) {
    TextStyle t;
    t.px = 36;
    t.underline = true;
    text(cv, "C/B", 6, 29, t);
}

}  // namespace sdsys
