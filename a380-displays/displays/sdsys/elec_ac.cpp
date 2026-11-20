#include <cstdlib>
#include <string>
#include <vector>

#include "include/codec/SkCodec.h"
#include "include/core/SkData.h"
#include "include/core/SkImage.h"
#include "include/core/SkMatrix.h"
#include "include/core/SkPaint.h"
#include "include/core/SkSamplingOptions.h"

#include "a380/assets.h"
#include "sdsys.h"

namespace sdsys {
namespace {

sk_sp<SkImage> engine_image() {
    static sk_sp<SkImage> img;
    static bool tried = false;
    if (tried) return img;
    tried = true;
    for (auto& dir : a380::assets::images_dirs()) {
        auto data = SkData::MakeFromFileName((dir + "/SD_ELEC_AC_ENG.png").c_str());
        if (!data) continue;
        auto codec = SkCodec::MakeFromData(data);
        if (!codec) continue;
        auto [image, result] = codec->getImage();
        if (image) {
            img = image;
            break;
        }
    }
    return img;
}

std::string num0(double x) { return std::to_string(js_round(x)); }

uint32_t norm(bool ok) { return ok ? kGreen : kAmber; }

TextStyle ts(float px, uint32_t c, a380::Align al = a380::Align::Left, float ls = 0.f) {
    TextStyle s;
    s.px = px;
    s.color = c;
    s.align = al;
    s.ls = ls;
    return s;
}

constexpr auto End = a380::Align::Right;
constexpr auto Mid = a380::Align::Center;

void tr_indicator(SkCanvas& cv, const a380::Inputs& in, float x, float y, int bus) {
    std::string trName, busName;
    if (bus == 2) { trName = "1"; busName = "2"; }
    else if (bus == 3) { trName = "2"; busName = "3"; }
    else if (bus == 4) { trName = "APU"; busName = "4"; }
    else { trName = "ESS"; busName = "ESS_SHED"; }
    bool powered = b(in, "L:A32NX_ELEC_AC_" + busName + "_BUS_IS_POWERED");
    uint32_t col = powered ? kWhite : kAmber;
    bool dbl = bus > 3;
    TextStyle s = ts(26, col, Mid, 1.f);
    s.base = a380::Baseline::Middle;
    text(cv, trName, x + (dbl ? 2 : 21), y + (dbl ? -35 : -11), s);
    text(cv, "TR", x + (dbl ? 1 : -11), y - 11, s);
    triangle(cv, x, y, col, 0);
}

void bus_bar(SkCanvas& cv, const a380::Inputs& in, float x, float y, int bus) {
    bool normal = bus <= 4;
    std::string name = normal ? "AC " + std::to_string(bus) : (bus == 5 ? "AC ESS" : "AC EMER");
    std::string lvar = bus == 5 ? "ESS_SHED" : "ESS";
    bool powered = b(in, "L:A32NX_ELEC_AC_" + (normal ? std::to_string(bus) : lvar) + "_BUS_IS_POWERED");
    SkPath box = svg_path(normal ? "M 0,0 l 0,34 l 106,0 l 0,-34 z" : "M 0,0 l 0,37 l 138,0 l 0,-37 z");
    cv.save();
    cv.translate(x, y);
    fill_path(cv, box, kGrey);
    stroke_path(cv, box, kLightGrey, 1.f);
    uint32_t col = norm(powered);
    if (normal) {
        text(cv, "AC", 20, 29, ts(30, col));
        TextStyle s = ts(35, col, a380::Align::Left, 1.f);
        s.ws = -10.f;
        text(cv, std::to_string(bus), 67, 30, s);
    } else {
        TextStyle s = ts(29, col, Mid, 1.f);
        s.ws = -10.f;
        s.base = a380::Baseline::Middle;
        text(cv, name, 70, 22, s);
    }
    if (bus != 1 && bus != 6) {
        float tx = (bus == 2 || bus == 3) ? 13 : bus == 4 ? 92 : 103;
        tr_indicator(cv, in, tx, -19, bus);
    }
    cv.restore();
}

void ext_power(SkCanvas& cv, const a380::Inputs& in, float x, float y, int bus) {
    double pot = v(in, "L:A32NX_ELEC_EXT_PWR_POTENTIAL"), fr = v(in, "L:A32NX_ELEC_EXT_PWR_FREQUENCY");
    bool freqOk = fr > 390 && fr < 410, voltOk = pot > 110 && pot < 120;
    bool closed = v(in, "L:A32NX_ELEC_CONTACTOR_990XG" + std::to_string(bus) + "_IS_CLOSED") != 0;
    bool conn = b(in, "L:A32NX_EXT_PWR_AVAIL:" + std::to_string(bus));
    bool other = bus % 2 == 1 && b(in, "L:A32NX_EXT_PWR_AVAIL:" + std::to_string(bus + 1));
    bool ground = b(in, "L:A32NX_LGCIU_1_NOSE_GEAR_COMPRESSED") || b(in, "L:A32NX_LGCIU_2_NOSE_GEAR_COMPRESSED");
    cv.save();
    cv.translate(x, y);
    if (bus % 2 == 1 && (conn || other)) {
        text(cv, "V", 94, 38, ts(22, kCyan));
        text(cv, "HZ", 87, 64, ts(22, kCyan));
    }
    if (ground || conn) {
        text(cv, "EXT", 8, 10, ts(22, kWhite, a380::Align::Left, 1.f));
        text(cv, std::to_string(bus), 57, 10, ts(22, kWhite));
        if (conn) {
            stroke_path(cv, svg_path("M 5,0 l -5,0 l 0,70 l 5,0"), kLightGrey, 3.f, true);
            stroke_path(cv, svg_path("M 71,0 l 5,0 l 0,70 l -5,0"), kLightGrey, 3.f, true);
            text(cv, num0(pot), 69, 38, ts(29, norm(freqOk), End, 1.f));
            text(cv, num0(fr), 69, 65, ts(29, norm(voltOk), End, 1.f));
        }
        float tx = bus <= 2 ? 40 : 36;
        triangle(cv, tx, -29, closed ? kGreen : kWhite, 0);
        if (closed) stroke_path(cv, svg_path("M " + std::to_string((int)tx) + ",-29 l 0,-43"), kGreen, 2.f);
    }
    cv.restore();
}

void eng_gen(SkCanvas& cv, const a380::Inputs& in, float x, float y, int bus) {
    std::string sb = std::to_string(bus);
    double pot = v(in, "L:A32NX_ELEC_ENG_GEN_" + sb + "_POTENTIAL"), load = v(in, "L:A32NX_ELEC_ENG_GEN_" + sb + "_LOAD");
    bool loadOk = load < 108, voltOk = pot > 110 && pot < 120;
    bool closed = v(in, "L:A32NX_ELEC_CONTACTOR_990XU" + sb + "_IS_CLOSED") != 0;
    bool pb = v(in, "GENERAL ENG MASTER ALTERNATOR:" + sb) != 0;
    int state = (int)v(in, "L:A32NX_ENGINE_STATE:" + sb);
    cv.save();
    cv.translate(x, y);
    if (auto img = engine_image()) {
        draw_image_two_step(cv, img, SkRect::MakeXYWH(-18, -19, 111, 185));
    }
    fill_path(cv, svg_path("M 0,0 l 0,84 l 76,0 l 0,-84 z"), 0x000000);
    uint32_t head = (pb && state == 1) ? kWhite : kAmber;
    text(cv, "GEN", 5, 23, ts(26, head));
    text(cv, std::to_string(bus), 59, 23, ts(28, head));
    if (bus % 2 == 1) {
        text(cv, "%", 94, 52, ts(22, kCyan));
        text(cv, "V", 94, 81, ts(22, kCyan));
    }
    if (pb) {
        text(cv, num0(load), 67, 52, ts(29, norm(loadOk), End, 1.f));
        text(cv, num0(pot), 67, 80, ts(29, norm(voltOk), End, 1.f));
    } else {
        text(cv, "OFF", 67, 62, ts(29, kWhite, End, 1.f));
    }
    if (closed) {
        triangle(cv, 38, -14, kGreen, 0);
        stroke_path(cv, svg_path("M 38,-14 l 0,-151"), kGreen, 2.f);
    }
    cv.restore();
}

void network(SkCanvas& cv, const a380::Inputs& in, float x, float y, int bus) {
    cv.save();
    cv.translate(x, y);
    bus_bar(cv, in, 0, 0, bus);
    ext_power(cv, in, bus <= 2 ? -30 : 60, 106, bus);
    eng_gen(cv, in, 14, 199, bus);
    cv.restore();
}

void apu_gen(SkCanvas& cv, const a380::Inputs& in, float x, float y, int pos) {
    std::string sp = std::to_string(pos);
    double load = v(in, "L:A32NX_ELEC_APU_GEN_" + sp + "_LOAD"), pot = v(in, "L:A32NX_ELEC_APU_GEN_" + sp + "_POTENTIAL"),
           fr = v(in, "L:A32NX_ELEC_APU_GEN_" + sp + "_FREQUENCY");
    bool loadOk = load < 108, freqOk = fr > 390 && fr < 410, voltOk = pot > 110 && pot < 120;
    bool closed = v(in, "L:A32NX_ELEC_CONTACTOR_990XS" + sp + "_IS_CLOSED") != 0;
    bool pb = v(in, "APU GENERATOR SWITCH:" + sp) != 0;
    bool master = b(in, "L:A32NX_OVHD_APU_MASTER_SW_PB_IS_ON");
    cv.save();
    cv.translate(x, y);
    uint32_t head = (pb || !master) ? kWhite : kAmber;
    text(cv, "GEN", 10, 9, ts(22, head));
    text(cv, pos == 1 ? "A" : "B", 60, 9, ts(22, head));
    if (master)
        stroke_path(cv, svg_path(pos == 1 ? "M 5,0 l -5,0 l 0,100 l 83,0 l 0,-5 m 0,-77 l 0,-18 l -5,0"
                                          : "M 5,0 l -5,0 l 0,18 m 0,77 l 0,5 l 83,0 l 0,-100 l -5,0"),
                    kLightGrey, 3.f, true);
    if (master && pb) {
        text(cv, num0(load), 71, 38, ts(29, norm(loadOk), End, 1.f));
        text(cv, num0(pot), 71, 64, ts(29, norm(freqOk), End, 1.f));
        text(cv, num0(fr), 71, 91, ts(29, norm(voltOk), End, 1.f));
    }
    if (!(pb || !master)) text(cv, "OFF", 72, 62, ts(29, kWhite, End, 1.f));
    triangle(cv, 42, -29, closed ? kGreen : kWhite, 0);
    if (closed) stroke_path(cv, svg_path("M 42,-29 l 0,-193"), kGreen, 2.f);
    cv.restore();
}

}  // namespace

void draw_elec_ac(SkCanvas& cv, const a380::Inputs& in) {
    {
        TextStyle s = ts(36, kWhite);
        s.underline = true;
        text(cv, "ELEC AC", 6, 29, s);
    }
    bus_bar(cv, in, 316, 28, 6);
    bus_bar(cv, in, 316, 137, 5);

    if (b(in, "L:A32NX_ELEC_CONTACTOR_980XU5_IS_CLOSED") && b(in, "L:A32NX_ELEC_CONTACTOR_980XU6_IS_CLOSED"))
        stroke_path(cv, svg_path("M 234, 275 l 248, 0"), kGreen, 2.f);

    auto c = [&](const char* n) { return b(in, std::string("L:A32NX_ELEC_CONTACTOR_") + n + "_IS_CLOSED"); };
    bool b1 = c("980XU1"), b2 = c("980XU2"), b3 = c("980XU3"), b4 = c("980XU4"), sic = c("900XU"), a1 = c("990XS1"),
         a2 = c("990XS2");
    bool s1 = b1 && (b2 || a1 || sic), s2 = b2 && (b1 || a1 || sic), s3 = b3 && (b4 || a2 || sic),
         s4 = b4 && (b3 || a2 || sic);
    bool s12 = s1, s34 = b4 && (b3 || a2 || sic);
    bool s2a = (b1 || b2) && (a1 || sic), s3a = (b3 || b4) && (a2 || sic);
    if (s1) stroke_path(cv, svg_path("M 140,258l 0,-60"), kGreen, 2.f);
    if (s2) stroke_path(cv, svg_path("M 234,258l 0,-60"), kGreen, 2.f);
    if (s3) stroke_path(cv, svg_path("M 535,258l 0,-60"), kGreen, 2.f);
    if (s4) stroke_path(cv, svg_path("M 629,258l 0,-60"), kGreen, 2.f);
    if (s12) stroke_path(cv, svg_path("M 140,198 l 94,0"), kGreen, 2.f);
    if (s34) stroke_path(cv, svg_path("M 535,198 l 94,0"), kGreen, 2.f);
    if (s2a) stroke_path(cv, svg_path("M 234,198 l 103,0"), kGreen, 2.f);
    if (s3a) stroke_path(cv, svg_path("M 432,198 l 103,0"), kGreen, 2.f);
    if (sic) stroke_path(cv, svg_path("M 337,198 l 95,0"), kGreen, 2.f);

    if (b(in, "L:A32NX_ELEC_CONTACTOR_3XB.1_IS_CLOSED")) stroke_path(cv, svg_path("M 385 137 l 0 -72"), kGreen, 2.f);

    {
        cv.save();
        cv.translate(386, 389);
        bool master = b(in, "L:A32NX_OVHD_APU_MASTER_SW_PB_IS_ON");
        TextStyle s = ts(26, kWhite, Mid, 1.f);
        s.base = a380::Baseline::Middle;
        text(cv, "APU", 0, 0, s);
        if (master) {
            TextStyle u = ts(22, kCyan, Mid);
            u.base = a380::Baseline::Middle;
            text(cv, "%", 0, 62, u);
            text(cv, "V", 0, 90, u);
            text(cv, "HZ", 0, 117, u);
        }
        apu_gen(cv, in, -91, 31, 1);
        apu_gen(cv, in, 5, 31, 2);
        cv.restore();
    }

    {
        cv.save();
        cv.translate(497, 47);
        double pot = v(in, "L:A32NX_ELEC_STAT_INV_POTENTIAL"), fr = v(in, "L:A32NX_ELEC_STAT_INV_FREQUENCY");
        bool freqOk = fr > 390 && fr < 410, voltOk = pot > 110 && pot < 120;
        bool dc = b(in, "L:A32NX_ELEC_CONTACTOR_7XB_IS_CLOSED"), em = b(in, "L:A32NX_ELEC_CONTACTOR_3XB.2_IS_CLOSED");
        uint32_t tc = kWhite;
        if (dc && (!freqOk || !voltOk || !em)) tc = kAmber;
        else if (dc && em) tc = kGreen;
        bool show = dc || em;
        TextStyle s = ts(25, kWhite, a380::Align::Left, 1.f);
        s.ws = -6.f;
        text(cv, "STAT INV", 24, show ? -17 : 10, s);
        if (show) {
            stroke_path(cv, svg_path("M 20,-41 l 0,86 l 133,0 l 0,-86 z"), kLightGrey, 3.f);
            text(cv, num0(pot), 110, 11, ts(29, norm(freqOk), End, 1.f));
            text(cv, num0(fr), 110, 39, ts(29, norm(voltOk), End, 1.f));
            text(cv, "V", 114, 11, ts(22, kCyan));
            text(cv, "HZ", 114, 39, ts(22, kCyan));
        }
        triangle(cv, 0, 0, tc, -90);
        if (dc && em) stroke_path(cv, svg_path("M 0,0 l -43,0"), kGreen, 2.f);
        cv.restore();
    }

    {
        cv.save();
        cv.translate(290, 95);
        double pot = v(in, "L:A32NX_ELEC_EMER_GEN_POTENTIAL"), load = v(in, "L:A32NX_ELEC_EMER_GEN_LOAD");
        bool loadOk = load < 108, voltOk = pot > 110 && pot < 120;
        bool closed = v(in, "L:A32NX_ELEC_CONTACTOR_5XE_IS_CLOSED") != 0;
        bool ratOut = v(in, "L:A32NX_RAT_STOW_POSITION") > 0.9;
        bool show = closed || ratOut;
        text(cv, "RAT", show ? -85 : -69, show ? -18 : 10, ts(25, kWhite, a380::Align::Left, 1.f));
        if (show) {
            stroke_path(cv, svg_path("M -102,-43 l 0,86 l 83,0 l 0,-86 z"), kLightGrey, 3.f);
            stroke_path(cv, svg_path("M -102,0 l -20,0 l -10,43 l 20,0 l -20,-86 l 20,0 l-10,43"),
                        (closed && !ratOut) ? kAmber : kLightGrey, 3.f);
            text(cv, num0(load), -38, 11, ts(29, norm(loadOk), End, 1.f));
            text(cv, num0(pot), -38, 39, ts(29, norm(voltOk), End, 1.f));
            text(cv, "%", -35, 11, ts(22, kCyan));
            text(cv, "V", -35, 39, ts(22, kCyan));
        }
        triangle(cv, 0, 0, closed ? kGreen : kWhite, 90);
        if (closed) stroke_path(cv, svg_path("M 0,0 l 55,0 l 0,42"), kGreen, 2.f);
        cv.restore();
    }

    network(cv, in, 58, 258, 1);
    network(cv, in, 181, 258, 2);
    network(cv, in, 482, 258, 3);
    network(cv, in, 605, 258, 4);

    if (b(in, "L:A32NX_ELEC_CONTACTOR_3XC1_IS_CLOSED")) stroke_path(cv, svg_path("M 111 258 l 0 -102 l 205 0"), kGreen, 2.f);
    if (b(in, "L:A32NX_ELEC_CONTACTOR_3XC2_IS_CLOSED")) stroke_path(cv, svg_path("M 658 258 l 0 -102 l -205 0"), kGreen, 2.f);
}

}  // namespace sdsys
