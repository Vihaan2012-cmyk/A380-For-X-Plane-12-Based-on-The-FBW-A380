#include <string>

#include "include/core/SkRRect.h"

#include "sdsys.h"

namespace sdsys {
namespace {

constexpr auto End = a380::Align::Right;

void rrect(SkCanvas& cv, float x, float y, float w, float h, float rx, bool fill, uint32_t fillColor, bool stroke,
           uint32_t strokeColor, float sw) {
    SkRRect rr = SkRRect::MakeRectXY(SkRect::MakeXYWH(x, y, w, h), rx, rx);
    if (fill) {
        SkPaint p;
        p.setAntiAlias(true);
        p.setColor(a380::rgb(fillColor));
        cv.drawRRect(rr, p);
    }
    if (stroke) cv.drawRRect(rr, sk_stroke(strokeColor, sw));
}

void fill(SkCanvas& cv, const char* d, uint32_t c) { fill_path(cv, svg_path(d), c); }
void stroke(SkCanvas& cv, const char* d, uint32_t c, float w, bool round = true) {
    SkPaint p = sk_stroke(c, w, round);
    draw_path(cv, svg_path(d), p);
}

void cabin_door(SkCanvas& cv, const a380::Inputs& in, float x, float y, bool left, bool upper, int num, bool engineRunning,
                int point, bool beacon) {
    bool open = v(in, "INTERACTIVE POINT OPEN:" + std::to_string(point)) > 20;
    bool armed = !open && beacon;
    std::string msg;
    if (engineRunning && open) {
        std::string tag = std::string(upper ? "UPPER " : "MAIN ") + std::to_string(num) + (left ? "L" : "R");
        msg = left ? tag + " ----" : "---- " + tag;
    }
    float xpos = left ? (upper ? -193.f : -180.f) : (upper ? 33.f : 30.f);
    cv.save();
    cv.translate(x, y);
    if (open) rrect(cv, 0, 0, 18, 26, 5, true, kAmber, true, kAmber, 3.f);
    else rrect(cv, 0, 0, 18, 26, 5, true, 0x000000, true, kGreen, 3.f);
    text(cv, std::to_string(num), 3, 21, T(22, open ? 0x000000 : kGreen));
    if (!msg.empty()) text(cv, msg, xpos, 12, T(22, kAmber));
    if (armed) text(cv, "S", left ? -6 : 23, 38, T(30, kWhite, left ? End : a380::Align::Left));
    cv.restore();
}

void cargo_door(SkCanvas& cv, float x, float y, const char* label, float w, float h, bool engineRunning, bool closed) {
    bool open = !closed;
    bool avncs = std::string(label) == "AVNCS";
    std::string msg;
    if (open && (engineRunning || avncs)) msg = avncs ? std::string(label) + " ----" : std::string("--") + label;
    cv.save();
    cv.translate(x, y);
    rrect(cv, 0, 0, w, h, 4, true, 0x000000, true, open ? kAmber : kGreen, 2.f);
    if (open) rrect(cv, 3, 4, w - 6, h - 8, 4, true, kAmber, true, kAmber, 2.f);
    else rrect(cv, 3, 4, w - 6, h - 8, 4, false, 0, true, kGreen, 2.f);
    if (!msg.empty()) text(cv, msg, avncs ? -160.f : 35.f, avncs ? 18.f : 20.f, T(24, kAmber));
    cv.restore();
}

}  // namespace

void draw_door(SkCanvas& cv, const a380::Inputs& in) {
    bool windowLeft = v(in, "L:CPT_SLIDING_WINDOW") == 0, windowRight = v(in, "L:FO_SLIDING_WINDOW") == 0;
    bool beacon = b(in, "LIGHT BEACON ON");
    bool engineRunning = false;
    for (int i = 1; i <= 4; i++) engineRunning = engineRunning || v(in, "L:A32NX_ENGINE_STATE:" + std::to_string(i)) == 1;

    TextStyle t = T(36, kWhite);
    t.underline = true;
    text(cv, "DOOR", 6, 29, t);
    text(cv, "OXYGEN", 599, 29, t);

    cv.save();
    cv.translate(634, 108);
    text(cv, "CKPT", 0, 0, T(22, kWhite, End, 1.f));
    text(cv, "1829", 86, 0, T(25, kGreen, End, 1.f));
    text(cv, "PSI", 91, 0, T(22, kCyan, a380::Align::Left, 1.f));
    stroke(cv, "M-26,57 l 122,0", kWhite, 2.f);
    text(cv, "CABIN", 15, 124, T(22, kWhite, End, 1.f));
    text(cv, "1854", 86, 124, T(26, kGreen, End));
    text(cv, "PSI", 91, 124, T(22, kCyan, a380::Align::Left, 1.f));
    cv.restore();

    stroke(cv, "M567,2 l 0,659", kWhite, 3.f);
    text(cv, "MAIN", 285, 159, TM(22, kWhite, 1.f));

    stroke(cv, "m224,110 c-19,22,-31,46,-39,73 c-3,13,-5,27,-5,41 l0,408 c0,9,3,16,5,24", kWhite, 3.f);
    stroke(cv, "m65,400 l116, -24", kWhite, 3.f);
    stroke(cv, "m344,110 c19,22,31,46,39,73 c3,13,5,27,5,41 l0,408 c0,9,-3,16,-5,24", kWhite, 3.f);
    stroke(cv, "m503,400 l-116, -24", kWhite, 3.f);
    fill(cv, "M263,184 c-11,10,-26,34,-26,49 v380 c0,5,0,14,5,22 h82 c0,-5,0,14,6,-22 v-380 c0,-9,0,-20,-29,-49 z", kGrey);
    stroke(cv, "M263,184 c-11,10,-26,34,-26,49 v380 c0,5,0,14,5,22", kWhite, 2.f);
    stroke(cv, "M301,184 c12,9,29,34,29,49 v380 c0,5,0,14,-6,22", kWhite, 2.f);
    fill(cv, "m261,261 h43 l3,21 h-50 z", 0x000000);
    for (const char* d : {"m257,282 h50 l-2,-6 h-46 z", "m262,276 h40 l-1,-4 h-38 z", "m265,272 h34 l-2,-4 h-30 z",
                          "m269,268 h26 l-1,-2 h-24 z", "m272,266 h20 l-1,-2 h-18 z"})
        fill(cv, d, kGrey);
    for (const char* d : {"M282,75 l-33,21 l2,26 l31,-22 z", "M285,75 l33,21 l-2,26 l-31,-22 z",
                          "M243,138 h-15 l -7,19 h1 l16,-12 z", "M325,138 h15 l 7,19 h-1 l-16,-12 z",
                          "M244,102 l-14,30 h14 l2,-5 z", "M323,102 l14,30 h-14 l-2,-5 z"})
        fill(cv, d, kGrey);
    {
        SkPaint p = sk_stroke(kGreen, 3.f);
        p.setStrokeJoin(SkPaint::kRound_Join);
        if (windowLeft) draw_path(cv, svg_path("M244,102 l-14,30 h14 l2,-5 z"), p);
        else fill(cv, "M244,102 l-14,30 h14 l2,-5 z", kAmber);
        if (windowRight) draw_path(cv, svg_path("M323,102 l14,30 h-14 l-2,-5 z"), p);
        else fill(cv, "M323,102 l14,30 h-14 l-2,-5 z", kAmber);
    }
    text(cv, "UPPER", 285, 216, TM(22, kWhite, 1.f));

    struct D { float x, y; int num, pt; };
    for (D d : {D{183, 201, 1, 0}, D{183, 314, 2, 2}, D{183, 402, 3, 4}, D{183, 477, 4, 6}, D{183, 621, 5, 8}})
        cabin_door(cv, in, d.x, d.y, true, false, d.num, engineRunning, d.pt, beacon);
    for (D d : {D{368, 201, 1, 1}, D{368, 314, 2, 3}, D{368, 402, 3, 5}, D{368, 477, 4, 7}, D{368, 621, 5, 9}})
        cabin_door(cv, in, d.x, d.y, false, false, d.num, engineRunning, d.pt, beacon);
    for (D d : {D{239, 350, 1, 10}, D{239, 440, 2, 12}, D{239, 556, 3, 14}})
        cabin_door(cv, in, d.x, d.y, true, true, d.num, engineRunning, d.pt, beacon);
    for (D d : {D{310, 350, 1, 11}, D{310, 440, 2, 13}, D{310, 556, 3, 15}})
        cabin_door(cv, in, d.x, d.y, false, true, d.num, engineRunning, d.pt, beacon);

    cargo_door(cv, 222, 165, "AVNCS", 27, 20, engineRunning, true);
    cargo_door(cv, 359, 250, "FWD CARGO", 26, 46, engineRunning, b(in, "L:A32NX_FWD_DOOR_CARGO_LOCKED"));
    cargo_door(cv, 359, 515, "AFT CARGO", 26, 42, engineRunning, b(in, "L:A32NX_AFT_DOOR_CARGO_LOCKED"));
    cargo_door(cv, 359, 590, "BULK", 26, 26, engineRunning, true);
}

}  // namespace sdsys
