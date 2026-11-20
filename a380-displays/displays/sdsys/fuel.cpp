#include <algorithm>
#include "a380/shape_cache.h"
#include <cmath>
#include <optional>
#include <string>
#include <vector>

#include "include/core/SkMatrix.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkSamplingOptions.h"

#include "sdsys.h"

namespace sdsys {
namespace {

TextStyle TC(float px, uint32_t c) {
    TextStyle s = T(px, c, a380::Align::Center);
    s.base = a380::Baseline::Alphabetic;
    return s;
}

constexpr auto End = a380::Align::Right;
constexpr float kOpenThreshold = 0.1f;


double valve_open(const a380::Inputs& in, int n) { return v(in, "FUELSYSTEM VALVE OPEN:" + std::to_string(n)); }

bool pump_bit(const a380::Inputs& in, const char* word, int bit) {
    A429 a = var429(in, word);
    if (a.ssm != 3 && a.ssm != 2) return false;
    return (((int32_t)a.val >> (bit - 1)) & 1) != 0;
}

bool switch_off(const a380::Inputs& in, int circuit) { return v(in, "CIRCUIT CONNECTION ON:" + std::to_string(circuit)) == 0; }

std::optional<double> tank(const a380::Inputs& in, const char* name) {
    for (const char* src : {"FQMS_", "FQDC_1_", "FQDC_2_"}) {
        A429 a = var429(in, std::string("L:A32NX_") + src + name + "_TANK_QUANTITY");
        if (a.ssm == 3 || a.ssm == 2) return (double)a.val;
    }
    return std::nullopt;
}

void tank_quantity(SkCanvas& cv, float x, float y, std::optional<double> q, bool fault) {
    std::string s = q ? std::to_string((int)(std::floor(*q / 20.0) * 20)) : "XX\xC2\xA0";
    text(cv, s, x, y, T(26, (fault || !q) ? kAmber : kGreen, End));
}

void pump(SkCanvas& cv, float x, float y, bool running, bool fault, bool displayInactive = false) {
    uint32_t c;
    if (fault) c = kAmber;
    else if (running) c = kGreen;
    else if (displayInactive) c = kWhite;
    else return;
    SkPaint p = sk_stroke(c, 2.8f);
    p.setStrokeJoin(SkPaint::kRound_Join);
    cv.drawRect(SkRect::MakeXYWH(x - 14, y - 14, 28, 28), p);
    if (running) cv.drawLine(x, y - 14, x, y + 14, p);
    else cv.drawLine(x - 9, y, x + 9, y, p);
}

void fuel_valve(SkCanvas& cv, float x, float y, bool open, bool horizontal, bool normallyClosed) {
    uint32_t c = (!open && !normallyClosed) ? kAmber : kGreen;
    bool rot = open != !horizontal;
    circle_stroke(cv, x, y, 16, c, 2.8f);
    if (rot) cv.drawLine(x - 16, y, x + 16, y, sk_stroke(c, 2.8f));
    else cv.drawLine(x, y - 16, x, y + 16, sk_stroke(c, 2.8f));
}

void arrow(SkCanvas& cv, float x, float y, float rot, float size, bool in_, uint32_t c, bool fillIt) {
    SkPathBuilder pb;
    pb.moveTo(x - size / 2, y);
    pb.lineTo(x + size / 2, y);
    pb.lineTo(x, y - size);
    pb.close();
    SkMatrix m = SkMatrix::RotateDeg(rot, {x, y});
    if (in_) m.preTranslate(0, size);
    SkPath path = pb.detach().makeTransform(m);
    SkPaint p = sk_stroke(c, 3.f);
    p.setStrokeJoin(SkPaint::kRound_Join);
    if (fillIt) {
        SkPaint f;
        f.setAntiAlias(true);
        f.setColor(a380::rgb(c));
        a380::draw_path_cached(cv, path, f);
    }
    a380::draw_path_cached(cv, path, p);
}

void fuel_line(SkCanvas& cv, float x1, float y1, float x2, float y2, bool active, bool displayInactive, const char* endArrow = nullptr,
               float endSize = 10, bool fault = false) {
    uint32_t c;
    if (fault) c = kAmber;
    else if (active) c = kGreen;
    else if (displayInactive) c = kWhite;
    else return;
    SkPaint p = sk_stroke(c, 3.f, true);
    p.setStrokeJoin(SkPaint::kRound_Join);
    cv.drawLine(x1, y1, x2, y2, p);
    if (endArrow) {
        float rot = std::atan2(x2 - x1, y1 - y2) * 180.f / 3.14159265f;
        bool in_ = std::string(endArrow) == "in";
        if (in_) rot = std::fmod(rot + 180.f, 360.f);
        arrow(cv, x2, y2, rot, endSize, in_, c, false);
    }
}

struct FL {
    float x1, y1, x2, y2;
    char sa, ea;
    bool active;
    float sz = 10.f;
    bool fill = false;
    bool fault = false;
};

struct GPump {
    float x, y;
    bool running;
};

SkPoint rot_pt(float px, float py, float cx, float cy, float deg) {
    float a = deg * 3.14159265f / 180.f, sn = std::sin(a), cs = std::cos(a);
    float dx = px - cx, dy = py - cy;
    return {cx + dx * cs - dy * sn, cy + dx * sn + dy * cs};
}

void fuel_line_ex(SkCanvas& cv, const FL& l) {
    uint32_t c;
    if (l.fault) c = kAmber;
    else if (l.active) c = kGreen;
    else return;
    SkPaint p = sk_stroke(c, 3.f, true);
    p.setStrokeJoin(SkPaint::kRound_Join);
    float rot = std::atan2(l.x2 - l.x1, l.y1 - l.y2) * 180.f / 3.14159265f;
    float sRot = l.sa == 'o' ? std::fmod(rot + 180.f, 360.f) : rot;
    float eRot = l.ea == 'i' ? std::fmod(rot + 180.f, 360.f) : rot;
    auto brk = [&](float x, float y, float r, bool left) {
        SkPoint a = rot_pt(left ? x + 2.f : x - 2.f, y + 5.f, x, y, std::fmod(r + 90.f, 360.f));
        SkPoint b = rot_pt(left ? x - 2.f : x + 2.f, y - 5.f, x, y, std::fmod(r + 90.f, 360.f));
        cv.drawLine(a.fX, a.fY, b.fX, b.fY, p);
    };
    if (l.sa == 'i' || l.sa == 'o') arrow(cv, l.x1, l.y1, sRot, l.sz, l.sa == 'i', c, l.fill);
    if (l.sa == 'l' || l.sa == 'r') brk(l.x1, l.y1, sRot, l.sa == 'l');
    cv.drawLine(l.x1, l.y1, l.x2, l.y2, p);
    if (l.ea == 'l' || l.ea == 'r') brk(l.x2, l.y2, eRot, l.ea == 'l');
    if (l.ea == 'i' || l.ea == 'o') arrow(cv, l.x2, l.y2, eRot, l.sz, l.ea == 'i', c, l.fill);
}

void gallery(SkCanvas& cv, float gy, const std::vector<GPump>& pumps, const std::vector<FL>& valves, const std::vector<FL>& other) {
    const float kInf = 1e30f;
    float lastValve = -kInf, firstValve = kInf, lastActive = -kInf, firstActive = kInf;
    for (auto& pm : pumps)
        if (pm.running) {
            lastActive = std::max(lastActive, pm.x);
            firstActive = std::min(firstActive, pm.x);
        }
    for (auto& vl : valves)
        if (vl.active) {
            lastValve = std::max(lastValve, vl.x1);
            firstValve = std::min(firstValve, vl.x1);
        }
    lastActive = std::max(lastActive, lastValve);
    firstActive = std::min(firstActive, firstValve);
    const bool anyValve = firstValve < kInf;
    float prevX = -kInf, prevY = gy;
    size_t i = 0, j = 0, k = 0;
    while (i < pumps.size() || j < valves.size()) {
        const bool nextIsPump = j >= valves.size() || (i < pumps.size() && pumps[i].x < valves[j].x1);
        const float nx = nextIsPump ? pumps[i].x : valves[j].x1;
        const bool nact = nextIsPump ? pumps[i].running : valves[j].active;
        if (k < other.size() && other[k].x1 < nx) {
            FL ol = other[k++];
            if (prevX > -kInf) {
                fuel_line_ex(cv, FL{prevX, prevY, ol.x1, prevY, 0, 0, anyValve && prevX >= firstActive && ol.x1 <= lastActive});
                ol.active = anyValve && ol.x1 >= firstActive && ol.x2 <= lastActive;
                fuel_line_ex(cv, ol);
            }
            prevX = ol.x2;
            prevY = ol.y2;
        }
        if (prevX > -kInf && nx > prevX)
            fuel_line_ex(cv, FL{prevX, prevY, nx, prevY, 0, 0, anyValve && prevX >= firstActive && nx <= lastActive});
        if (nextIsPump) {
            float py = pumps[i].y;
            float y1 = prevY < py ? py - 14.f : py + 14.f;
            fuel_line_ex(cv, FL{nx, y1, pumps[i].x, prevY, 0, 0, anyValve && nact});
            i++;
        } else {
            j++;
        }
        prevX = nx;
    }
    for (auto& vl : valves) fuel_line_ex(cv, vl);
}

void engine(SkCanvas& cv, const a380::Inputs& in, float x, float y, int index) {
    bool running = v(in, "L:A32NX_ENGINE_N3:" + std::to_string(index)) > 50;
    if (auto img = load_image(index < 3 ? "SD_FUEL_ENG_L.png" : "SD_FUEL_ENG_R.png"))
        draw_image_two_step(cv, img, SkRect::MakeXYWH(x, y, 75, 96));
    text(cv, std::to_string(index), x + 8, y + 25, T(26, running ? kWhite : kAmber));
}

}  // namespace

void draw_fuel(SkCanvas& cv, const a380::Inputs& in) {
    double used[5];
    for (int i = 1; i <= 4; i++) used[i] = v(in, "L:A32NX_FUEL_USED:" + std::to_string(i));
    A429 apuUsed = var429(in, "L:A32NX_APU_FUEL_USED");
    double totalEng = used[1] + used[2] + used[3] + used[4];
    double total = apuUsed.normal() ? std::floor((totalEng + apuUsed.val) / 50.0) * 50.0 : std::floor(totalEng / 50.0) * 50.0;
    double ff = 0;
    for (int i = 1; i <= 4; i++) ff += v(in, "L:A32NX_ENGINE_FF:" + std::to_string(i));
    int ffShown = (int)(std::floor(ff / 60.0 / 10.0) * 10.0);

    TextStyle t = T(36, kWhite);
    t.underline = true;
    text(cv, "FUEL", 6, 29, t);
    {
        TextStyle d = T(24, kWhite, a380::Align::Left, -8.f);
        text(cv, "...", 110, 28, d);
        text(cv, "MORE", 137, 28, T(26, kWhite));
    }
    text(cv, "FU", 384, 56, TC(21, kWhite));
    text(cv, apuUsed.normal() ? "TOTAL" : "ALL ENG", 384, 79, TC(21, kWhite));
    text(cv, std::to_string((int)total), 384, 103, TC(24, kGreen));
    text(cv, "KG", 384, 126, TC(21, kCyan));

    struct E { float ex, ey, vx, vy, tx, ty, ly1, ly2; };
    const E es[4] = {{74, 105, 111, 150, 111, 84, 132, 124}, {236, 81, 273, 123, 273, 68, 105, 97},
                     {456, 81, 493, 123, 493, 68, 105, 97}, {618, 105, 655, 150, 655, 84, 132, 124}};
    for (int i = 0; i < 4; i++) {
        const E& e = es[i];
        engine(cv, in, e.ex, e.ey, i + 1);
        bool open = valve_open(in, i + 1) >= 0.5;
        fuel_valve(cv, e.vx, e.vy, open, false, false);
        fuel_line(cv, e.vx, e.ly1, e.vx, e.ly2, true, false);
        if (open) fuel_line(cv, e.vx, e.ly1, e.vx, e.ly2, true, false, "out", 12.f);
        text(cv, std::to_string((int)(std::floor(used[i + 1] / 50.0) * 50)), e.tx, e.ty, TC(24, kGreen));
    }

    if (auto img = load_image("SD_FUEL_BG.png"))
        draw_image_two_step(cv, img, SkRect::MakeXYWH(7, 168, 751, 310));

    auto feed = [&](int n) { return tank(in, ("FEED_" + std::to_string(n)).c_str()); };
    auto low = [](std::optional<double> q) { return !q || *q < 1375; };
    const char* L = "FQMS_LEFT_FUEL_PUMP_RUNNING_WORD";
    const char* R = "FQMS_RIGHT_FUEL_PUMP_RUNNING_WORD";
    std::string lw = std::string("L:A32NX_") + L, rw = std::string("L:A32NX_") + R;
    auto lb = [&](int bit) { return pump_bit(in, lw.c_str(), bit); };
    auto rb = [&](int bit) { return pump_bit(in, rw.c_str(), bit); };

    bool f1a = lb(12), f1b = lb(13), f2a = lb(14), f2b = lb(15), f3a = rb(12), f3b = rb(13), f4a = rb(14), f4b = rb(15);
    bool cf[5];
    for (int i = 1; i <= 4; i++) cf[i] = valve_open(in, 45 + i) >= 0.1;

    tank_quantity(cv, 154, 300, feed(1), low(feed(1)));
    pump(cv, 95, 227, f1a, switch_off(in, 2));
    pump(cv, 127, 227, f1b && !f1a, switch_off(in, 3));
    fuel_line(cv, 111, 212, 111, 164, true, false);
    fuel_line(cv, 111, 175, 139, 175, true, false);
    fuel_valve(cv, 154, 175, cf[1], true, true);

    tank_quantity(cv, 102, 434, tank(in, "LEFT_OUTER"), false);
    tank_quantity(cv, 202, 430, tank(in, "LEFT_MID"), false);
    tank_quantity(cv, 302, 430, tank(in, "LEFT_INNER"), false);

    tank_quantity(cv, 322, 288, feed(2), low(feed(2)));
    pump(cv, 258, 208, f2a, switch_off(in, 64));
    pump(cv, 290, 208, f2b && !f2a, switch_off(in, 65));
    fuel_line(cv, 273, 191, 273, 137, true, false);
    fuel_line(cv, 273, 148, 299, 148, true, false);
    fuel_valve(cv, 316, 148, cf[2], true, true);

    tank_quantity(cv, 528, 288, feed(3), low(feed(3)));
    pump(cv, 476, 208, f3a, switch_off(in, 66));
    pump(cv, 508, 208, f3b && !f3a, switch_off(in, 67));
    fuel_line(cv, 493, 191, 493, 137, true, false);
    fuel_line(cv, 467, 148, 493, 148, true, false);
    fuel_valve(cv, 450, 148, cf[3], true, true);

    tank_quantity(cv, 548, 430, tank(in, "RIGHT_INNER"), false);
    tank_quantity(cv, 648, 430, tank(in, "RIGHT_MID"), false);
    tank_quantity(cv, 748, 434, tank(in, "RIGHT_OUTER"), false);

    tank_quantity(cv, 696, 300, feed(4), low(feed(4)));
    pump(cv, 639, 227, f4a, switch_off(in, 68));
    pump(cv, 671, 227, f4b && !f4a, switch_off(in, 69));
    fuel_line(cv, 655, 212, 655, 164, true, false);
    fuel_line(cv, 629, 175, 655, 175, true, false);
    fuel_valve(cv, 612, 175, cf[4], true, true);

    text(cv, "ALL ENG FF", 10, 620, T(21, kWhite));
    text(cv, std::to_string(ffShown), 24, 644, T(21, kGreen));
    text(cv, "KG/MIN", 68, 644, T(21, kCyan));
    if (auto img = load_image("SD_FUEL_BG_TRIM.png"))
        draw_image_two_step(cv, img, SkRect::MakeXYWH(269, 571, 227, 80));

    bool sideToSide = (cf[1] || cf[2]) && (cf[3] || cf[4]);
    fuel_line_ex(cv, FL{171, 175, 258, 175, 0, 'r', cf[1]});
    fuel_line_ex(cv, FL{290, 175, 352, 175, 'r', 0, cf[1]});
    fuel_line(cv, 352, 175, 414, 175, sideToSide, false);
    fuel_line_ex(cv, FL{414, 175, 476, 175, 0, 'r', cf[4]});
    fuel_line_ex(cv, FL{508, 175, 595, 175, 'r', 0, cf[4]});
    fuel_line(cv, 333, 148, 352, 148, cf[2], false);
    fuel_line(cv, 352, 148, 352, 175, cf[2], false);
    fuel_line(cv, 433, 148, 414, 148, cf[3], false);
    fuel_line(cv, 414, 148, 414, 175, cf[3], false);

    bool master = b(in, "L:A32NX_OVHD_APU_MASTER_SW_PB_IS_ON");
    bool iso = master && valve_open(in, 50) >= 0.1, lp = master && valve_open(in, 51) >= 0.1;
    bool normal = iso == master && lp == master;
    if (!normal || iso || lp) text(cv, "APU", 705, 183, T(21, normal ? kWhite : kAmber));
    if (iso && lp) fuel_line(cv, 657, 175, 685, 175, true, false, "out", 12.f, !normal);

    struct GP { float x, y; int bit; bool left; int circuit; };
    for (GP g : {GP{84, 384, 16, true, 70}, GP{140, 384, 17, true, 71}, GP{232, 384, 19, true, 73},
                 GP{482, 384, 19, false, 78}, GP{584, 384, 17, false, 76}, GP{680, 384, 16, false, 75},
                 GP{182, 452, 18, true, 72}, GP{274, 452, 20, true, 74}, GP{524, 452, 20, false, 79},
                 GP{626, 452, 18, false, 77}})
        pump(cv, g.x, g.y, g.left ? lb(g.bit) : rb(g.bit), switch_off(in, g.circuit));

    auto vo = [&](int n) { return valve_open(in, n) >= 0.1; };
    const bool lOuterF = vo(19), lMidF = vo(18), lInnerF = vo(17), rInnerF = vo(20), rMidF = vo(21), rOuterF = vo(22);
    const bool feed1F = vo(5) || vo(11), feed2F = vo(6) || vo(8) || vo(12) || vo(14), feed3F = vo(7) || vo(9) || vo(13) || vo(15),
               feed4F = vo(10) || vo(16);
    const bool feed1A = vo(23) || vo(27), feed2A = vo(24) || vo(28), feed3A = vo(25) || vo(29), feed4A = vo(26) || vo(30);
    const bool lOuterA = vo(33) && vo(39), lMidA = vo(32) && vo(38), lInnerA = vo(31) && vo(37), rInnerA = vo(34) && vo(40),
               rMidA = vo(35) && vo(41), rOuterA = vo(36) && vo(42);
    fuel_line_ex(cv, FL{43, 293, 42, 293, 0, 'i', vo(52)});
    fuel_line_ex(cv, FL{719, 293, 720, 293, 0, 'i', vo(53)});
    gallery(cv, 362,
            {{84, 384, lb(16)}, {140, 384, lb(17)}, {232, 384, lb(19)}, {482, 384, rb(19)}, {584, 384, rb(17)}, {680, 384, rb(16)}},
            {FL{34, 362, 34, 382, 0, 'o', lOuterF}, FL{54, 382, 54, 362, 'i', 0, lOuterF}, FL{140, 362, 140, 342, 0, 'o', feed1F},
             FL{180, 366, 180, 346, 'i', 0, lMidF}, FL{192, 346, 192, 366, 0, 'o', lMidF}, FL{284, 346, 284, 326, 0, 'o', feed2F},
             FL{284, 346, 284, 366, 0, 'o', lInnerF}, FL{520, 346, 520, 326, 0, 'o', feed3F}, FL{520, 346, 520, 366, 0, 'o', rInnerF},
             FL{622, 362, 622, 342, 0, 'o', feed4F}, FL{622, 382, 622, 362, 'i', 0, rMidF}, FL{638, 362, 638, 382, 0, 'o', rMidF},
             FL{712, 382, 712, 362, 'i', 0, rOuterF}, FL{728, 362, 728, 382, 0, 'o', rOuterF}},
            {FL{164, 362, 174, 346, 0, 0, true}, FL{592, 346, 602, 362, 0, 0, true}});
    fuel_line_ex(cv, FL{386, 346, 386, 175, 0, 0, vo(56)});
    fuel_line_ex(cv, FL{438, 346, 438, 472, 0, 0, vo(54) || vo(55)});
    gallery(cv, 472, {{182, 452, lb(18)}, {274, 452, lb(20)}, {524, 452, rb(20)}, {626, 452, rb(18)}},
            {FL{60, 472, 60, 452, 0, 'o', lOuterA}, FL{84, 452, 84, 472, 'i', 0, lOuterA}, FL{111, 472, 111, 376, 0, 'l', feed1A},
             FL{111, 350, 111, 342, 'l', 'o', feed1A}, FL{132, 472, 132, 452, 0, 'o', lMidA}, FL{154, 452, 154, 472, 'i', 0, false},
             FL{232, 472, 232, 452, 0, 'o', lInnerA}, FL{314, 472, 314, 358, 0, 'l', feed2A}, FL{314, 332, 314, 322, 'l', 'o', feed2A},
             FL{448, 472, 448, 358, 0, 'l', feed3A}, FL{448, 332, 448, 322, 'l', 'o', feed3A}, FL{482, 472, 482, 452, 0, 'o', rInnerA},
             FL{578, 472, 578, 452, 0, 'o', rMidA}, FL{600, 452, 600, 472, 'i', 0, false}, FL{653, 472, 653, 376, 0, 'l', feed4A},
             FL{653, 350, 653, 342, 'l', 'o', feed4A}, FL{680, 452, 680, 472, 'i', 0, rOuterA}, FL{704, 472, 704, 452, 0, 'o', rOuterA}},
            {});
    const bool jetL = vo(57), jetR = vo(58);
    fuel_line_ex(cv, FL{190, 500, 190, 522, 'i', 'o', jetL, 16.f, true, jetL});
    if (jetL) text(cv, "JETTISON", 134, 562, T(21, kAmber));
    fuel_line_ex(cv, FL{570, 500, 570, 522, 'i', 'o', jetR, 16.f, true, jetR});
    if (jetR) text(cv, "JETTISON", 514, 562, T(21, kAmber));

    pump(cv, 298, 610, lb(21), switch_off(in, 80));
    pump(cv, 468, 610, rb(21), switch_off(in, 81));
    bool trimIsolated = valve_open(in, 44) < 0.1 && valve_open(in, 45) < 0.1 && valve_open(in, 43) < 0.1 &&
                        valve_open(in, 59) < 0.1;
    fuel_valve(cv, 386, 540, !trimIsolated, false, true);
    tank_quantity(cv, 418, 640, tank(in, "TRIM"), false);
}

}  // namespace sdsys
