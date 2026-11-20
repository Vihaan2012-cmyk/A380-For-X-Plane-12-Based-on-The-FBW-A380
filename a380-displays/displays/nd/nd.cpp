#include <algorithm>
#include <cmath>
#include <cstdlib>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <memory>
#include <string>
#include <vector>

#include "include/core/SkSurface.h"
#include "include/core/SkBitmap.h"
#include "include/core/SkCanvas.h"
#include "include/core/SkImage.h"
#include "include/core/SkMatrix.h"
#include "include/core/SkColor.h"
#include "include/core/SkFont.h"
#include "include/core/SkFontMetrics.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPath.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkRRect.h"
#include "include/effects/SkDashPathEffect.h"

#include "a380/display.h"
#include "a380/display_unit.h"
#include "a380/draw.h"
#include "nd/nd_prim.h"
#include "a380/fonts.h"
#include "a380/inputs.h"
#include "nd/nd_layers.h"

namespace a380::nd {

struct Word {
    int ssm = 0;
    double value = 0;
    bool normal() const { return ssm == 3; }
    bool ncd() const { return ssm == 1; }
    bool fw() const { return ssm == 0; }
    bool ft() const { return ssm == 2; }
};

namespace {
constexpr int kPm = prim::kLine;

constexpr float kTextDx = 0.0f;
constexpr double kPi = 3.14159265358979323846;
constexpr double kEarthNm = 3440.065;

double rad(double d) { return d * kPi / 180.0; }
double deg(double r) { return r * 180.0 / kPi; }

}  // namespace

Word arinc(double raw) {
    Word w;
    int64_t i = (int64_t)raw;
    w.ssm = (int)((i >> 32) & 3);
    uint32_t lo = (uint32_t)(i & 0xffffffffll);
    float f;
    std::memcpy(&f, &lo, 4);
    w.value = f;
    return w;
}

double gc_heading(double lat1, double lon1, double lat2, double lon2) {
    double p1 = rad(lat1), p2 = rad(lat2), dl = rad(lon2 - lon1);
    double y = std::sin(dl) * std::cos(p2);
    double x = std::cos(p1) * std::sin(p2) - std::sin(p1) * std::cos(p2) * std::cos(dl);
    double h = deg(std::atan2(y, x));
    return std::fmod(h + 360.0, 360.0);
}

double gc_distance_nm(double lat1, double lon1, double lat2, double lon2) {
    double p1 = rad(lat1), p2 = rad(lat2), dp = p2 - p1, dl = rad(lon2 - lon1);
    double a = std::sin(dp / 2) * std::sin(dp / 2) + std::cos(p1) * std::cos(p2) * std::sin(dl / 2) * std::sin(dl / 2);
    return 2 * kEarthNm * std::atan2(std::sqrt(a), std::sqrt(1 - a));
}

SkPoint Frame::project(double lat, double lon) const {
    double bearing = gc_heading(ref_lat, ref_lon, lat, lon) - up_true_deg - 90.0;
    double dist = gc_distance_nm(ref_lat, ref_lon, lat, lon);
    double x = dist * std::cos(rad(bearing));
    double y = dist * std::sin(rad(bearing));
    return SkPoint::Make((float)(centre.x() + x * px_per_nm), (float)(centre.y() + y * px_per_nm));
}

namespace {

const SkColor kGreen = SkColorSetRGB(0, 255, 0);
const SkColor kCyan = SkColorSetRGB(0, 255, 255);
const SkColor kWhite = SkColorSetRGB(255, 255, 255);
const SkColor kAmber = SkColorSetRGB(230, 128, 0);
const SkColor kYellow = SkColorSetRGB(255, 255, 0);
const SkColor kRed = SkColorSetRGB(255, 0, 0);
const SkColor kMagenta = SkColorSetRGB(255, 148, 255);
const SkColor kGrey = SkColorSetRGB(0x4f, 0x54, 0x67);
const SkColor kBlack = SkColorSetRGB(0, 0, 0);

constexpr float kFontLargest = 35.f, kFontLarge = 32.5f, kFontMedium = 30.f, kFontIntermediate = 27.5f,
                kFontSmall = 25.f, kFontSmallest = 22.5f, kFontTiny = 20.f;

SkPaint strokePaint(SkColor c, float w, SkPaint::Cap cap = SkPaint::kButt_Cap, SkPaint::Join join = SkPaint::kMiter_Join) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setStyle(SkPaint::kStroke_Style);
    p.setColor(c);
    p.setStrokeWidth(w);
    p.setStrokeCap(cap);
    p.setStrokeJoin(join);
    return p;
}

void strokeCircleImpl(SkCanvas& c, float cx, float cy, float r, float w, SkColor col) {
    chromium_stroke_circle(c, cx, cy, r, w, strokePaint(col, w));
}

SkPaint fillPaint(SkColor c) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setStyle(SkPaint::kFill_Style);
    p.setColor(c);
    return p;
}

enum class Align { Start, Middle, End };

void text(SkCanvas& c, const std::string& s, float x, float y, float px, SkColor col, Align a = Align::Start,
          bool shadow = false, const char* family = "Ecam", bool lcd = false) {
    if (s.empty()) return;
    SkFont f = a380::font(family, px, false, lcd);
    float w = f.measureText(s.data(), s.size(), SkTextEncoding::kUTF8);
    x += kTextDx;
    if (a == Align::Middle) x -= w / 2;
    else if (a == Align::End) x -= w;
    if (shadow) {
        SkPaint sp = strokePaint(kBlack, 1.f);
        a380::draw_simple_text(c, s.data(), s.size(), SkTextEncoding::kUTF8, x, y, f, sp);
    }
    a380::draw_simple_text(c, s.data(), s.size(), SkTextEncoding::kUTF8, x, y, f, fillPaint(col));
}

float centralBaseline(float y, float px) {
    SkFont f = a380::font("Ecam", px);
    SkFontMetrics m;
    f.getMetrics(&m);
    return y + (-m.fAscent - m.fDescent) / 2.f + 2.4f;
}

float textWidth(const std::string& t, float px) {
    SkFont f = a380::font("Ecam", px);
    return f.measureText(t.data(), t.size(), SkTextEncoding::kUTF8);
}

std::string num(double v) {
    char b[32];
    if (std::fabs(v - std::round(v)) < 1e-9) std::snprintf(b, sizeof b, "%d", (int)std::lround(v));
    else std::snprintf(b, sizeof b, "%g", v);
    return b;
}

std::string pad3(int v) {
    char b[16];
    std::snprintf(b, sizeof b, "%03d", v);
    return b;
}

void line(SkCanvas& c, float x1, float y1, float x2, float y2, const SkPaint& p) { prim::line(kPm, c, x1, y1, x2, y2, p); }

SkPath poly(std::initializer_list<SkPoint> pts) {
    SkPathBuilder p;
    bool first = true;
    for (auto& q : pts) {
        if (first) p.moveTo(q);
        else p.lineTo(q);
        first = false;
    }
    p.close();
    return p.detach();
}

double diffAngle(double a, double b) {
    double d = b - a;
    while (d > 180) d -= 360;
    while (d <= -180) d += 360;
    return d;
}

double smallestAngle(double a, double b) {
    double d = a - b;
    if (d > 180) d -= 360;
    else if (d < -180) d += 360;
    return d;
}

struct RoseCache {
    sk_sp<SkImage> image;
    float scale = 0;
};

struct Hz2 {
    double last = -1e12, v = 0;
    double sample(double now, double raw) {
        if (now - last >= 500.0 || now < last) {
            last = now;
            v = raw;
        }
        return v;
    }
};

struct Dyn {
    bool init = false;
    int cur_mode = 3, prev_range = -1;
    bool page_change = false, range_change = false;
    double page_end = 0, range_end = 0;
    uint32_t rng = 12345u;
    std::vector<int> fm_active;
    Hz2 wind_dir, wind_spd, wind_hdg, gs, tas, xte;
    double needle_filtered[2] = {0, 0};
    double needle_last = 0;
    bool needle_seen[2] = {false, false};
    bool ident_rcv[2] = {false, false}, ident_sched[2] = {false, false}, ident_vis[2] = {false, false};
    double ident_due[2] = {0, 0};
};

struct State {
    Dyn* dyn = nullptr;
    double now = 0;
    bool range_flag = false, mode_flag = false, recomputing = false;
    RoseCache* arc_rose = nullptr;
    RoseCache* rose_rose = nullptr;
    int side = 1;
    char sc = 'L';
    int mode = 3;
    int range_idx = 3;
    double range = 40;
    int ir = 3;
    Word heading, track, true_heading, true_track, lat, lon;
    bool true_ref = false;
    int tcas_mode = 0;
    bool ta_only = false, tcas_fault = false;
    bool oans = false;
    bool fms_failed = false;
    bool map_flag = false;
    double lr = 40.0;
};

double rangeFor(int idx) {
    static const double r[] = {10, 10, 20, 40, 80, 160, 320, 640};
    return r[std::clamp(idx, 0, 7)];
}

std::string sv(const Inputs& in, const char* fmt, char sc) {
    char b[160];
    std::snprintf(b, sizeof b, fmt, sc);
    return b;
}

void blackAreas(SkCanvas& c, int mode) {
    SkPaint b = fillPaint(kBlack);
    if (mode == 3) {
        prim::path(kPm, c, poly({{0, 625}, {122, 625}, {174, 683}, {174, 768}, {0, 768}}), b);
        prim::path(kPm, c, poly({{768, 562}, {648, 562}, {591, 625}, {591, 768}, {768, 768}}), b);
    } else if (mode <= 2) {
        prim::path(kPm, c, poly({{45, 625}, {122, 625}, {174, 683}, {174, 768}, {0, 768}, {0, 0}, {45, 0}}), b);
        prim::path(kPm, c, poly({{591, 768}, {591, 626}, {648, 562}, {723, 562}, {723, 0}, {768, 0}, {768, 768}}), b);
        SkPathBuilder top;
        top.moveTo(45, 0);
        top.lineTo(45, 155);
        top.lineTo(282, 155);
        top.arcTo(SkPoint::Make(250, 250), 0, SkPathBuilder::kSmall_ArcSize, SkPathDirection::kCW, SkPoint::Make(486, 155));
        top.lineTo(723, 155);
        top.lineTo(723, 0);
        top.close();
        prim::path(kPm, c, top.detach(), b);
    }
}

constexpr float kArcRingInset = 0.06f;

SkPath twoArcs(float cx, float cy, float r, float startDeg) {
    SkPathBuilder p;
    SkRect oval = SkRect::MakeLTRB(cx - r, cy - r, cx + r, cy + r);
    p.arcTo(oval, startDeg, -180, true);
    p.arcTo(oval, startDeg - 180, -180, false);
    return p.detach();
}

void dashedRing(SkCanvas& c, const SkPath& path, SkColor col, float on, float off, float phase) {
    SkPaint p = strokePaint(col, 2.f);
    float iv[2] = {on, off};
    p.setPathEffect(SkDashPathEffect::Make(iv, phase));
    prim::path(kPm, c, path, p);
}

void cached_rose(SkCanvas& c, RoseCache* cache, float cx, float cy, float half, float supersample, void (*draw)(SkCanvas&)) {
    const SkRect dst = SkRect::MakeLTRB(cx - half, cy - half, cx + half, cy + half);
    if (!cache) {
        draw(c);
        return;
    }
    const SkMatrix m = c.getTotalMatrix();
    float scale = std::round(std::hypot(m.getScaleX(), m.getSkewY()) * supersample * 1000.f) / 1000.f;
    if (!cache->image || cache->scale != scale) {
        int px = (int)std::ceil(half * 2 * scale);
        SkImageInfo info = SkImageInfo::MakeN32Premul(px, px);
        sk_sp<SkSurface> surf = c.makeSurface(info);
        if (!surf) surf = SkSurfaces::Raster(info);
        if (!surf) {
            draw(c);
            return;
        }
        SkCanvas* rc = surf->getCanvas();
        rc->clear(SK_ColorTRANSPARENT);
        rc->scale(scale, scale);
        rc->translate(-dst.left(), -dst.top());
        draw(*rc);
        cache->image = surf->makeImageSnapshot();
        cache->scale = scale;
    }
    c.drawImageRect(cache->image, dst, SkSamplingOptions(SkFilterMode::kLinear), nullptr);
}

void arc_rose(SkCanvas& c) {
    stroke_circle(c, 384, 620, 492.f - kArcRingInset, 2.f, kWhite);
    SkPaint tp = strokePaint(kWhite, 2.f);
    for (int i = 0; i < 72; i++) {
        float a = -60.f + 5.f * i;
        c.save();
        c.rotate(a, 384, 620);
        bool big = i % 2 == 0;
        line(c, 384, 128, 384, big ? 99 : 113, tp);
        if (big) {
            int label = i / 2;
            bool large = (i % 6) == 0;
            float y = (label == 7 || label == 8) ? 95.f : 91.f;
            text(c, std::to_string(label), 384, y, large ? 34.f : 22.f, kWhite, Align::Middle);
        }
        c.restore();
    }
}

void rose_rose(SkCanvas& c) {
    stroke_circle(c, 384, 384, 250, 2.f, kWhite);
    SkPaint tp = strokePaint(kWhite, 2.5f);
    for (int i = 0; i < 72; i++) {
        c.save();
        c.rotate(5.f * i, 384, 384);
        bool big = i % 2 == 0;
        line(c, 384, 134, 384, big ? 122 : 128, tp);
        if (i % 6 == 0) text(c, std::to_string(i / 2), 384, 112, 22.f, kWhite, Align::Middle);
        c.restore();
    }
}

void arcUnderlay(SkCanvas& c, const State& s) {
    bool valid = s.heading.normal();
    SkColor ring = valid ? kWhite : kRed;
    int tcas = s.tcas_mode;
    c.save();
    c.clipRect(SkRect::MakeLTRB(6, 0, 762, 768));
    c.save();
    double rot = valid ? diffAngle(s.heading.value, 60.0) : diffAngle(0, 60.0);
    c.rotate((float)rot, 384, 620);
    if (valid) cached_rose(c, s.arc_rose, 384, 620, 570, 1.25f, arc_rose);
    else stroke_circle(c, 384, 620, 492.f - kArcRingInset, 2.f, ring);
    c.restore();
    c.restore();

    auto clipPoly = [&](std::initializer_list<SkPoint> pts) {
        c.save();
        c.clipPath(poly(pts), true);
    };

    clipPoly({{0, 564}, {384, 709}, {768, 564}, {768, 0}, {0, 0}});
    dashedRing(c, twoArcs(384, 620, 369, 180), ring, 15.f, 10.5f, 15.f);
    c.restore();
    std::string r3 = num(s.lr / 4 * 3), r2 = num(s.lr / 4 * 2);
    text(c, r3, 58, 482, kFontSmallest, kCyan);
    text(c, r3, 709, 482, kFontSmallest, kCyan, Align::End);

    clipPoly({{0, 532}, {384, 687}, {768, 541}, {768, 29}, {0, 29}});
    dashedRing(c, twoArcs(384, 620, 246, 180), ring, 15.f, 10.f, -6.f);
    c.restore();
    text(c, r2, 175, 528, kFontSmallest, kCyan);
    text(c, r2, 592, 528, kFontSmallest, kCyan, Align::End);

    if (tcas == 0 || s.range > 10) {
        clipPoly({{0, 519}, {384, 664}, {768, 578}, {768, -2}, {0, -2}});
        dashedRing(c, twoArcs(384, 620, 123, 180), ring, 15.f, 10.f, -4.2f);
        c.restore();
    }
    SkPaint tk = strokePaint(kWhite, 2.f, SkPaint::kRound_Cap, SkPaint::kRound_Join);
    auto ticks = [&](float y1, float y2) {
        for (int a : {-60, -30, 0, 30, 60}) {
            c.save();
            c.rotate((float)a, 384, 620);
            line(c, 384, y1, 384, y2, tk);
            c.restore();
        }
    };
    if (tcas > 0 && s.range == 10) ticks(497 - 6, 497 + 6);
    if (tcas > 0 && s.range == 20) ticks(558 - 6, 558 + 6);
}

void roseUnderlay(SkCanvas& c, const State& s) {
    bool valid = s.heading.normal();
    SkColor ring = valid ? kWhite : kRed;
    c.save();
    double rot = valid ? diffAngle(s.heading.value, 0.0) : 0.0;
    c.rotate((float)rot, 384, 384);
    if (valid) cached_rose(c, s.rose_rose, 384, 384, 300, 1.5f, rose_rose);
    else stroke_circle(c, 384, 384, 250, 2.f, ring);
    c.restore();

    int tcas = s.tcas_mode;
    if (tcas == 0 || s.range > 10) {
        SkPathBuilder p;
        SkRect oval = SkRect::MakeLTRB(259, 259, 509, 509);
        p.arcTo(oval, 0, 180, true);
        p.arcTo(oval, 180, 180, true);
        dashedRing(c, p.detach(), ring, 15.f, 10.f, -4.2f);
    } else {
        SkPaint tk = strokePaint(kWhite, 2.f, SkPaint::kRound_Cap, SkPaint::kRound_Join);
        for (int a = 0; a < 360; a += 30) {
            c.save();
            c.rotate((float)a, 384, 384);
            line(c, 384, 264, 384, 254, tk);
            c.restore();
        }
    }
    if (tcas > 0 && s.range == 20) {
        SkPaint tk = strokePaint(kWhite, 2.f, SkPaint::kRound_Cap, SkPaint::kRound_Join);
        for (int a = 0; a < 360; a += 30) {
            c.save();
            c.rotate((float)a, 384, 384);
            line(c, 384, 327, 384, 317, tk);
            c.restore();
        }
    }
    text(c, num(s.lr / 2), 212, 556, 22.f, kCyan);
    text(c, num(s.lr / 4), 310, 474, 22.f, kCyan);
    if (valid) {
        SkPaint fp = fillPaint(kWhite);
        SkPaint sp = strokePaint(kWhite, 2.f);
        for (int a = 45; a < 360; a += 45) {
            c.save();
            c.rotate((float)a, 384, 384);
            SkPath t = poly({{384, 132}, {379, 123}, {389, 123}});
            prim::path(kPm, c, t, fp);
            prim::path(kPm, c, t, sp);
            c.restore();
        }
    }
}

void planUnderlay(SkCanvas& c, const State& s) {
    c.save();
    c.clipPath(poly({{45, 112}, {140, 112}, {280, 56}, {488, 56}, {628, 112}, {723, 112}, {723, 720}, {114, 720}, {114, 633}, {45, 633}}), true);
    stroke_circle(c, 384, 384, 250, 3.f, kWhite);
    SkPathBuilder p;
    SkRect oval = SkRect::MakeLTRB(259, 259, 509, 509);
    p.arcTo(oval, 180, -180, true);
    p.arcTo(oval, 0, -180, false);
    SkPaint dp = strokePaint(kWhite, 3.f);
    float iv[2] = {14.f, 13.f};
    dp.setPathEffect(SkDashPathEffect::Make(iv, 0));
    prim::path(kPm, c, p.detach(), dp);
    text(c, num(s.lr / 4), 310, 474, 22.f, kCyan);
    text(c, num(s.lr / 2), 212, 556, 22.f, kCyan);
    struct L { const char* t; float x, y; };
    for (L l : {L{"N", 384, 170}, L{"E", 598, 384}, L{"S", 384, 598}, L{"W", 170, 384}})
        text(c, l.t, l.x, centralBaseline(l.y, 25.f), 25.f, kWhite, Align::Middle);
    SkPaint fp = fillPaint(kWhite);
    prim::path(kPm, c, poly({{384, 141.5f}, {390, 151}, {378, 151}}), fp);
    prim::path(kPm, c, poly({{626.2f, 384}, {617, 390}, {617, 378}}), fp);
    prim::path(kPm, c, poly({{384, 626.5f}, {390, 617}, {378, 617}}), fp);
    prim::path(kPm, c, poly({{141.5f, 384}, {151, 390}, {151, 378}}), fp);
    c.restore();
}

void windIndicator(SkCanvas& c, const Inputs& in, const State& s) {
    Dyn& dy = *s.dyn;
    Word dir = arinc(dy.wind_dir.sample(s.now, in.var("L:A32NX_ADIRS_IR_1_WIND_DIRECTION_BNR")));
    Word spd = arinc(dy.wind_spd.sample(s.now, in.var("L:A32NX_ADIRS_IR_1_WIND_SPEED_BNR")));
    char hn[64];
    std::snprintf(hn, sizeof hn, "L:A32NX_ADIRS_IR_%d_TRUE_HEADING", s.ir);
    Word th = arinc(dy.wind_hdg.sample(s.now, in.var(hn)));
    c.save();
    c.translate(23, 58);
    std::string dt, st;
    if (dir.fw() || spd.fw()) {
    } else if (dir.ncd() || spd.ncd()) {
        dt = "---";
        st = "---";
    } else {
        double d = dir.value < 0 ? dir.value + 360 : dir.value;
        dt = pad3((int)std::lround(d));
        st = std::to_string((int)std::lround(spd.value));
    }
    text(c, dt, 25, 0, kFontSmall, kGreen, Align::End);
    text(c, "/", 31, -1, kFontSmallest, kWhite);
    text(c, st, 50, 0, kFontSmall, kGreen);
    if (dir.normal() && spd.normal() && th.normal() && spd.value > 2) {
        double d = dir.value < 0 ? dir.value + 360 : dir.value;
        double r = std::fmod(std::lround(d) - std::lround(th.value) + 180.0 + 720.0, 360.0);
        c.save();
        c.translate(3, 10);
        c.rotate((float)r, 0, 15);
        SkPathBuilder p;
        p.moveTo(0, 30);
        p.lineTo(0, 0);
        p.moveTo(-6.5f, 12);
        p.lineTo(0, 0);
        p.lineTo(6.5f, 12);
        prim::path(kPm, c, p.detach(), strokePaint(kGreen, 2.5f, SkPaint::kRound_Cap));
        c.restore();
    }
    c.restore();
}

void speedIndicator(SkCanvas& c, const Inputs& in, const State& s) {
    Word gs = arinc(s.dyn->gs.sample(s.now, in.var("L:A32NX_ADIRS_IR_1_GROUND_SPEED")));
    Word tas = arinc(s.dyn->tas.sample(s.now, in.var("L:A32NX_ADIRS_ADR_1_TRUE_AIRSPEED")));
    c.save();
    c.translate(2, 25);
    text(c, "GS", 0, 0, kFontSmallest, kWhite);
    if (gs.normal()) text(c, std::to_string((int)std::lround(gs.value)), 89, 0, kFontIntermediate, kGreen, Align::End);
    text(c, "TAS", 95, 0, kFontSmallest, kWhite);
    if (tas.normal()) text(c, std::to_string((int)std::lround(tas.value)), 201, 0, kFontIntermediate, kGreen, Align::End);
    c.restore();
}

std::string toWptIdent(const Inputs& in, char sc);

void toWaypoint(SkCanvas& c, const Inputs& in, const State& s) {
    if (s.mode == 0 || s.mode == 1) return;
    bool normalPos = s.lat.normal();
    double bearing = s.true_ref ? in.var(sv(in, "L:A32NX_EFIS_%c_TO_WPT_TRUE_BEARING", s.sc))
                                : in.var(sv(in, "L:A32NX_EFIS_%c_TO_WPT_BEARING", s.sc));
    double dist = in.var(sv(in, "L:A32NX_EFIS_%c_TO_WPT_DISTANCE", s.sc), {}, 0);
    double eta = in.var(sv(in, "L:A32NX_EFIS_%c_TO_WPT_ETA", s.sc), {}, 0);
    c.save();
    c.translate(690, 25);
    std::string ident = toWptIdent(in, s.sc);
    text(c, ident, -130, 0, kFontIntermediate, kWhite, Align::Start);
    if (normalPos && std::isfinite(bearing) && bearing != -1) {
        text(c, pad3((int)std::lround(bearing)), 57, 0, kFontIntermediate, kGreen, Align::End);
        if (s.true_ref) text(c, "T", 71, -3, kFontIntermediate, kCyan, Align::End);
        else text(c, "\xC2\xB0", 73, 2, kFontIntermediate, kCyan, Align::End);
    }
    if (dist > 0 && normalPos) {
        if (dist > 20) {
            text(c, std::to_string((int)std::lround(dist)), 39, 32, kFontIntermediate, kGreen, Align::End);
        } else {
            char b[16];
            std::snprintf(b, sizeof b, "%.1f", dist);
            std::string f = b;
            auto dot = f.find('.');
            text(c, f.substr(0, dot), 6, 32, kFontIntermediate, kGreen, Align::End);
            text(c, ".", 3, 32, kFontSmallest, kGreen);
            text(c, f.substr(dot + 1), 20, 32, kFontSmallest, kGreen);
        }
        text(c, "NM", 72, 32, kFontSmallest, kCyan, Align::End);
    }
    if (eta != -1 && normalPos) {
        int hh = (int)(eta / 3600), mm = (int)(std::fmod(eta, 3600) / 60);
        char b[16];
        std::snprintf(b, sizeof b, "%02d:%02d", hh, mm);
        text(c, b, 72, 66, kFontIntermediate, kGreen, Align::End);
    }
    c.restore();
}

void flagText(SkCanvas& c, const std::string& t, float x, float y, float px, SkColor col) {
    text(c, t, x, y, px, col, Align::Middle, true);
}

void airplane(SkCanvas& c, const State& s, SkPoint pos, double rotation, bool planeVisible, bool circle) {
    if (planeVisible) {
        c.save();
        c.translate(pos.x() - 41, pos.y());
        c.rotate((float)rotation, 41, 0);
        SkPathBuilder pb;
        SkPathBuilder& p = pb;
        p.moveTo(0, 0);
        p.lineTo(82, 0);
        p.moveTo(41, -29.5f);
        p.lineTo(41, 40.75f);
        p.moveTo(29.5f, 31);
        p.lineTo(53, 31);
        SkPath pp = pb.detach();
        prim::path(kPm, c, pp, strokePaint(kYellow, 5.f, SkPaint::kRound_Cap));
        c.restore();
        if (s.mode != 4) {
            c.save();
            c.rotate((float)rotation, 384, 626);
            float y1 = s.mode == 3 ? 108.f : 116.f, y2 = s.mode == 3 ? 148.f : 152.f;
            prim::line(kPm, c, 384, y1, 384, y2, strokePaint(kYellow, 5.f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
            c.restore();
        }
    }
    if (circle) stroke_circle(c, pos.x(), pos.y(), 9, 2.f, kRed);
}


std::string unpackStr(double a, double b) {
    std::string r;
    double vals[2] = {a, b};
    for (double v : vals) {
        uint64_t u = (uint64_t)std::max(0.0, v);
        for (int i = 0; i < 8; i++) {
            int code = (int)((u >> (i * 6)) & 0x3f);
            if (code > 0) r += (char)(code + 31);
        }
    }
    return r;
}

std::string toWptIdent(const Inputs& in, char sc) {
    return unpackStr(in.var(sv(in, "L:A32NX_EFIS_%c_TO_WPT_IDENT_0", sc), {}, 0), in.var(sv(in, "L:A32NX_EFIS_%c_TO_WPT_IDENT_1", sc), {}, 0));
}

std::string apprMessage(const Inputs& in, char sc) {
    return unpackStr(in.var(sv(in, "L:A32NX_EFIS_%c_APPR_MSG_0", sc), {}, 0), in.var(sv(in, "L:A32NX_EFIS_%c_APPR_MSG_1", sc), {}, 0));
}

void selectedHeadingBug(SkCanvas& c, const Inputs& in, const State& s) {
    if (in.var("L:A320_FCU_SHOW_SELECTED_HEADING") == 0 || !s.heading.normal() || s.mode == 4) return;
    double selected = in.var("L:A32NX_FCU_HEADING_SELECTED");
    double diff = smallestAngle(selected, s.heading.value);
    float cy = s.mode == 3 ? 620.f : 384.f;
    bool inRose = s.mode <= 2;
    if (inRose || diff <= 48) {
        c.save();
        c.rotate((float)diff, 384, cy);
        SkPath p = poly({{382, 126}, {370, 99}, {398, 99}, {386, 126}});
        prim::path(kPm, c, p, strokePaint(kBlack, 3.5f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
        prim::path(kPm, c, p, strokePaint(kCyan, 3.f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
        c.restore();
    }
    if (!inRose && std::fabs(diff) > 48) {
        c.save();
        c.rotate(diff < 0 ? -38.f : 38.f, 384, 620);
        text(c, pad3((int)std::lround(selected)), 384, 60, 22.f, kCyan, Align::Middle, true);
        c.restore();
    }
}

void trackBug(SkCanvas& c, const State& s) {
    if (s.mode == 4 || !s.heading.normal() || !s.track.normal()) return;
    double diff = smallestAngle(s.track.value, s.heading.value);
    if (!(diff <= 40)) return;
    bool arc = s.mode == 3;
    c.save();
    c.rotate((float)diff, 384, arc ? 620.f : 384.f);
    SkPath p = arc ? poly({{384, 128}, {378, 138}, {384, 148}, {390, 138}}) : poly({{384, 134}, {379, 143}, {384, 152}, {389, 143}});
    prim::path(kPm, c, p, strokePaint(kBlack, 4.5f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
    prim::path(kPm, c, p, strokePaint(kGreen, 3.f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
    c.restore();
}

bool activeVectorsTransmitted(const Inputs& in, char sc) {
    for (char d : {sc, sc == 'L' ? 'R' : 'L'}) {
        std::string topic = std::string("A32NX_EFIS_VECTORS_") + d + "_ACTIVE";
        const nlohmann::json* g = in.generic(topic);
        if (!g) g = in.bus(topic);
        if (g) {
            if (g->is_null()) return false;
            if (g->is_object()) {
                auto it = g->find("data");
                if (it != g->end() && it->is_null()) return false;
            }
            return true;
        }
        if (auto* eb = in.generic("EB_EVENTS"); eb && eb->is_object()) {
            auto dd = eb->find("data");
            if (dd != eb->end() && dd->is_array())
                for (auto& ev : *dd) {
                    auto t = ev.find("topic");
                    if (ev.is_object() && t != ev.end() && t->is_string() && t->get<std::string>() == topic) {
                        auto data = ev.find("data");
                        return !(data != ev.end() && data->is_null());
                    }
                }
        }
    }
    return true;
}

void trackLine(SkCanvas& c, const Inputs& in, const State& s) {
    if ((s.mode != 2 && s.mode != 3) || !s.heading.normal() || !s.track.normal()) return;
    if (activeVectorsTransmitted(in, s.sc)) return;
    float y = s.mode == 3 ? 620.f : 384.f;
    c.save();
    c.rotate((float)smallestAngle(s.track.value, s.heading.value), 384, y);
    SkPathBuilder pb;
    pb.moveTo(384, 149).lineTo(384, y);
    SkPath p = pb.detach();
    prim::path(kPm, c, p, strokePaint(kBlack, 3.f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
    prim::path(kPm, c, p, strokePaint(kGreen, 2.5f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
    c.restore();
}

void gridTrack(SkCanvas& c, const State& s, float y) {
    double gt = std::floor(s.true_track.value - (s.lat.value > 0 ? 1.0 : (s.lat.value < 0 ? -1.0 : 0.0)) * s.lon.value + 0.5);
    gt = std::fmod(std::fmod(gt, 360.0) + 360.0, 360.0);
    std::string head = std::string("◇") + pad3((int)gt);
    float w1 = textWidth(head, kFontSmallest), w2 = textWidth("°", kFontIntermediate), w3 = textWidth("G", kFontSmallest);
    float x0 = 384 + 45 - (w1 + (w2 - 5.f) + w3) / 2.f;
    c.save();
    prim::rect(kPm, c, SkRect::MakeXYWH(384, y - 20, 94, 23), strokePaint(kWhite, 1.5f));
    text(c, head, x0, y, kFontSmallest, kGreen);
    text(c, "°", x0 + w1 - 5.f, y + 8, kFontIntermediate, kCyan);
    text(c, "G", x0 + w1 - 5.f + w2, y, kFontSmallest, kCyan);
    c.restore();
}

void topMessages(SkCanvas& c, const Inputs& in, const State& s) {
    std::string appr = apprMessage(in, s.sc);
    if (!s.oans) text(c, appr, 384, 25, kFontIntermediate, kGreen, Align::Middle);
    bool trueVis = s.true_ref && s.mode != 4;
    const nlohmann::json* btv = in.bus("ndBtvMessage");
    if (btv && btv->is_string() && !btv->get<std::string>().empty() && !trueVis)
        text(c, btv->get<std::string>(), 384, 56, kFontSmallest, kGreen, Align::Middle);
    bool gridVis = trueVis && appr.empty() && s.lon.normal() && s.true_track.normal() && std::fabs(s.lat.normal() || s.lat.ft() ? s.lat.value : 0.0) > 65;
    if (trueVis) {
        float x = 384 + (gridVis ? -50 : 4), y = 56;
        text(c, "TRUE", x, y, kFontSmallest, kCyan, Align::Middle, true);
        if (appr.empty()) prim::rect(kPm, c, SkRect::MakeXYWH(x - 34, y - 20, 68, 23), strokePaint(kCyan, 1.5f));
    }
    if (gridVis) gridTrack(c, s, 56);
}

void flags(SkCanvas& c, const Inputs& in, const State& s) {
    if (s.mode != 4 && !s.heading.normal()) flagText(c, "HDG", 384, 241, kFontLarge, kRed);
    if ((s.mode == 3 || s.mode == 2) && (s.map_flag || s.fms_failed)) flagText(c, "MAP NOT AVAIL", 384, 320.6f, kFontLarge, kRed);
    if (s.range_flag) flagText(c, "ND RANGE CHANGE", 384, 315, 35.f, kGreen);
    else if (s.mode_flag) flagText(c, "ND MODE CHANGE", 384, 315, 35.f, kGreen);
}

void terrainThresholds(SkCanvas& c, const Inputs& in, char side) {
    double mx = in.var(sv(in, "L:A32NX_EGPWC_ND_%c_TERRAIN_MAX_ELEVATION", side), {}, -1);
    double mn = in.var(sv(in, "L:A32NX_EGPWC_ND_%c_TERRAIN_MIN_ELEVATION", side), {}, -1);
    if (mn < 0 || mx < 0) return;
    int mxm = (int)in.var(sv(in, "L:A32NX_EGPWC_ND_%c_TERRAIN_MAX_ELEVATION_MODE", side));
    int mnm = (int)in.var(sv(in, "L:A32NX_EGPWC_ND_%c_TERRAIN_MIN_ELEVATION_MODE", side));
    auto col = [](int m) { return m == 2 ? kRed : (m == 1 ? kYellow : kGreen); };
    text(c, "TERR", 688 + 5, 612 + 4, 25.f, kCyan);
    std::string up = mx != 0 ? std::to_string((int)std::lround(mx / 100 + 0.5)) : "0";
    float pad = textWidth("0", 22.f);
    text(c, up, 709 + pad * (float)std::max(0, 3 - (int)up.size()), 639, 22.f, col(mxm));
    prim::rect(kPm, c, SkRect::MakeXYWH(700, 619, 54, 24), strokePaint(kYellow, 1.f));
    if (mn > 0) {
        std::string lo = std::to_string((int)std::floor(mn / 100));
        text(c, lo, 709 + pad * (float)std::max(0, 3 - (int)lo.size()), 663, 22.f, col(mnm));
    }
    prim::rect(kPm, c, SkRect::MakeXYWH(700, 643, 54, 24), strokePaint(kYellow, 1.f));
}

void bigLittle(SkCanvas& c, const std::string& v, float xEnd, float y, SkColor col) {
    auto dot = v.find('.');
    if (dot == std::string::npos) {
        text(c, v, xEnd, y, 24.f, col, Align::End);
        return;
    }
    std::string frac = v.substr(dot);
    float wf = textWidth(frac, 20.f);
    text(c, frac, xEnd, y, 20.f, col, Align::End);
    text(c, v.substr(0, dot), xEnd - wf, y, 24.f, col, Align::End);
}

std::string tuningModeBit(const Inputs& in, int bit) {
    auto flag = [&](const char* t) {
        const auto* j = in.bus(t);
        if (!j) return false;
        if (j->is_boolean()) return j->get<bool>();
        if (j->is_number()) return j->get<double>() != 0;
        return false;
    };
    auto word = [&](const char* t) {
        const auto* j = in.bus(t);
        return j && j->is_number() ? arinc(j->get<double>()) : Word{};
    };
    Word w1 = word("fm.1.tuning_discrete_word"), w2 = word("fm.2.tuning_discrete_word");
    if ((!flag("fm.1.healthy_discrete") && !flag("fm.2.healthy_discrete")) || (!w1.normal() && !w2.normal())) return "R";
    auto set = [&](const Word& w) { return (w.normal() || w.ft()) && (((int64_t)w.value >> (bit - 1)) & 1); };
    return set(w1) || set(w2) ? "M" : "";
}

std::string tuningMode(const Inputs& in, int idx, bool adf) { return tuningModeBit(in, 10 + idx + (adf ? 2 : 0)); }

void underlinedEnd(SkCanvas& c, const std::string& t, float xEnd, float y) {
    if (t.empty()) return;
    float w = textWidth(t, 20.f);
    text(c, t, xEnd, y, 20.f, kWhite, Align::End);
    prim::rect(kPm, c, SkRect::MakeXYWH(xEnd - w, y + 1, w, 2), fillPaint(kWhite));
}

bool busBool(const Inputs& in, const char* topic) {
    const auto* j = in.bus(topic);
    if (!j) return false;
    if (j->is_boolean()) return j->get<bool>();
    if (j->is_number()) return j->get<double>() != 0;
    return false;
}

double stationLat(const Inputs& in, int idx) {
    char nm[32];
    std::snprintf(nm, sizeof nm, "nav%dLocation", idx);
    const auto* j = in.bus(nm);
    if (j && j->is_object()) {
        auto it = j->find("lat");
        if (it != j->end() && it->is_number()) return it->get<double>();
    }
    return 0;
}

struct StationFlags {
    bool corrected = false, mag_warn = false, true_warn = false;
};

StationFlags stationFlags(const Inputs& in, const State& s, int idx, bool available) {
    char nm[32];
    std::snprintf(nm, sizeof nm, "NAV MAGVAR:%d", idx);
    double decl = in.var(nm, "degrees", 0);
    bool stationTrue = std::fabs(stationLat(in, idx)) > 75 && decl < 2.220446049250313e-16;
    bool lsMode = s.mode == 0 || s.mode == 1;
    StationFlags f;
    f.corrected = s.true_ref != stationTrue && available && !lsMode;
    f.mag_warn = s.true_ref && !stationTrue && available && lsMode;
    f.true_warn = !s.true_ref && stationTrue && available && lsMode;
    return f;
}

void updateIdents(const Inputs& in, State& s) {
    Dyn& d = *s.dyn;
    for (int i = 0; i < 2; i++) {
        char nm[32];
        std::snprintf(nm, sizeof nm, "NAV IDENT:%d", i + 1);
        std::string ident = in.str(nm);
        std::snprintf(nm, sizeof nm, "NAV HAS NAV:%d", i + 1);
        bool hasNav = in.var(nm, {}, 0) != 0;
        std::snprintf(nm, sizeof nm, "NAV HAS DME:%d", i + 1);
        bool hasDme = in.var(nm, {}, 0) != 0;
        std::snprintf(nm, sizeof nm, "NAV DME:%d", i + 1);
        double dme = in.var(nm, "nautical mile", 0);
        bool rcv = (dme > 0 || hasNav || hasDme) && !ident.empty();
        if (rcv && !d.ident_rcv[i]) {
            d.rng = d.rng * 1664525u + 1013904223u;
            d.ident_due[i] = s.now + 5000.0 + (d.rng >> 8) / 16777216.0 * 10000.0;
            d.ident_sched[i] = true;
        } else if (!rcv && d.ident_rcv[i]) {
            d.ident_sched[i] = false;
            d.ident_vis[i] = false;
        }
        d.ident_rcv[i] = rcv;
        if (d.ident_sched[i] && s.now >= d.ident_due[i]) {
            d.ident_vis[i] = true;
            d.ident_sched[i] = false;
        }
    }
}

void radioNavInfo(SkCanvas& c, const Inputs& in, const State& s) {
    if (s.mode == 4) return;
    for (int idx = 1; idx <= 2; idx++) {
        char nm[64];
        std::snprintf(nm, sizeof nm, "L:A32NX_EFIS_%c_NAVAID_%d_MODE", s.sc, idx);
        int mode = (int)in.var(nm, {}, 0);
        if (mode != 1 && mode != 2) continue;
        float x = idx == 1 ? 37.f : 668.f;
        StationFlags sf;
        if (mode == 2) {
            std::snprintf(nm, sizeof nm, "NAV HAS NAV:%d", idx);
            sf = stationFlags(in, s, idx, in.var(nm, {}, 0) != 0);
        }
        SkPaint np = mode == 2 ? strokePaint(sf.corrected ? kMagenta : kWhite, 2.f, SkPaint::kRound_Cap, SkPaint::kRound_Join)
                               : strokePaint(kGreen, 1.f, SkPaint::kRound_Cap, SkPaint::kRound_Join);
        SkPathBuilder pb;
        if (mode == 2) {
            if (idx == 1) pb.moveTo(25, 675).lineTo(25, 680).lineTo(37, 696).lineTo(13, 696).lineTo(25, 680).moveTo(25, 696).lineTo(25, 719);
            else pb.moveTo(749, 719).lineTo(749, 696).lineTo(755, 696).lineTo(743, 680).lineTo(731, 696).lineTo(737, 696).lineTo(737, 719).moveTo(743, 680).lineTo(743, 675);
        } else {
            if (idx == 1) pb.moveTo(31, 686).lineTo(25, 680).lineTo(19, 686).moveTo(25, 680).lineTo(25, 719);
            else pb.moveTo(749, 719).lineTo(749, 696).lineTo(743, 690).lineTo(737, 696).lineTo(737, 719).moveTo(743, 690).lineTo(743, 675);
        }
        prim::path(kPm, c, pb.detach(), np);
        std::string tm = tuningMode(in, idx, mode != 2);
        if (!tm.empty()) {
            float tx = idx == 1 ? 138.f : 616.f;
            text(c, tm, tx, 720, 20.f, kWhite);
            prim::rect(kPm, c, SkRect::MakeXYWH(tx, 721, textWidth(tm, 20.f), 2), fillPaint(kWhite));
        }
        if (mode == 2) {
            text(c, "VOR" + std::to_string(idx), x, 692, 24.f, kWhite);
            if (sf.corrected) text(c, "CORR", x + (idx == 1 ? 61.f : -54.f), 692, kFontTiny, kMagenta);
            if (sf.mag_warn) text(c, "MAG", x + (idx == 1 ? 73.f : -54.f), 692, kFontTiny, kAmber);
            if (sf.true_warn) text(c, "TRUE", x + (idx == 1 ? 61.f : -54.f), 692, kFontTiny, kAmber);
            bool identVis = s.dyn->ident_vis[idx - 1];
            if (identVis) text(c, in.str((std::string("NAV IDENT:") + std::to_string(idx)).c_str()), x, 722, 24.f, kWhite);
            std::snprintf(nm, sizeof nm, "NAV ACTIVE FREQUENCY:%d", idx);
            double freq = in.var(nm, "MHz", 0);
            if (!identVis && freq > 1) {
                char b[16];
                std::snprintf(b, sizeof b, "%.2f", freq);
                std::string f = b;
                auto dot = f.find('.');
                float fx = idx == 2 ? x - 26 : x;
                text(c, f.substr(0, dot), fx, 722, 24.f, kWhite);
                text(c, f.substr(dot), fx + textWidth(f.substr(0, dot), 24.f), 722, 20.f, kWhite);
            }
            std::snprintf(nm, sizeof nm, "NAV HAS DME:%d", idx);
            bool hasDme = in.var(nm, {}, 0) != 0;
            std::snprintf(nm, sizeof nm, "NAV DME:%d", idx);
            double dme = in.var(nm, "nautical mile", 0);
            float dx = (idx == 1 ? -16.f : 0.f);
            float ax = (dme > 20 ? x + 46 : x + 58) + dx;
            if (!hasDme || dme <= 0) {
                text(c, "---", ax, 759, 24.f, kGreen, Align::End);
            } else if (dme >= 20) {
                char b[16];
                std::snprintf(b, sizeof b, "%.0f", dme);
                text(c, b, ax, 759, 24.f, kGreen, Align::End);
            } else {
                char b[16];
                std::snprintf(b, sizeof b, "%.1f", dme);
                bigLittle(c, b, ax, 759, kGreen);
            }
            text(c, "NM", x + 66 + dx, 759, 20.f, kCyan);
        } else {
            text(c, "ADF" + std::to_string(idx), x, 692, 24.f, kGreen);
            std::snprintf(nm, sizeof nm, "ADF ACTIVE FREQUENCY:%d", idx);
            double freq = in.var(nm, "KHz", 0);
            std::snprintf(nm, sizeof nm, "ADF SIGNAL:%d", idx);
            double sig = in.var(nm, {}, 0);
            if (sig > 0) text(c, in.str((std::string("ADF IDENT:") + std::to_string(idx)).c_str()), x, 722, 24.f, kGreen);
            else if (freq > 0) text(c, std::to_string((int)std::floor(freq)), x, 722, 24.f, kGreen);
        }
    }
}

SkPath svgLines(const char* d) {
    SkPathBuilder pb;
    const char* p = d;
    while (*p) {
        if (*p == 'M' || *p == 'L') {
            char cmd = *p++;
            char* e;
            float x = std::strtof(p, &e);
            p = e;
            if (*p == ',') p++;
            float y = std::strtof(p, &e);
            p = e;
            if (cmd == 'M') pb.moveTo(x, y);
            else pb.lineTo(x, y);
        } else {
            p++;
        }
    }
    return pb.detach();
}

SkPath arcMapClip() {
    SkPathBuilder p;
    p.moveTo(0, 312);
    p.arcTo(SkPoint::Make(492, 492), 0, SkPathBuilder::kSmall_ArcSize, SkPathDirection::kCW, SkPoint::Make(768, 312));
    p.lineTo(768, 562).lineTo(648, 562).lineTo(591, 625).lineTo(591, 768).lineTo(174, 768).lineTo(174, 683).lineTo(122, 625).lineTo(0, 625).lineTo(0, 312);
    return p.detach();
}

void updateNeedles(const Inputs& in, State& s) {
    Dyn& d = *s.dyn;
    double dt = std::min(s.now - d.needle_last, 100.0);
    d.needle_last = s.now;
    for (int idx = 1; idx <= 2; idx++) {
        char nm[64];
        std::snprintf(nm, sizeof nm, "L:A32NX_EFIS_%c_NAVAID_%d_MODE", s.sc, idx);
        int mode = (int)in.var(nm, {}, 0);
        std::snprintf(nm, sizeof nm, "NAV HAS NAV:%d", idx);
        bool avail = in.var(nm, {}, 0) != 0;
        bool shown = mode == 2 && avail && s.heading.normal() && s.track.normal() && s.mode != 4;
        std::snprintf(nm, sizeof nm, "NAV RELATIVE BEARING TO STATION:%d", idx);
        double raw = in.var(nm, "degrees", 0);
        double& f = d.needle_filtered[idx - 1];
        if (!shown) {
            d.needle_seen[idx - 1] = false;
            continue;
        }
        if (!d.needle_seen[idx - 1]) {
            d.needle_seen[idx - 1] = true;
            f = raw;
            continue;
        }
        double diff = diffAngle(f, raw);
        if (std::fabs(diff) < 0.1) continue;
        f = std::fmod(f + dt / 800.0 * diff, 360.0);
    }
}

void radioNeedles(SkCanvas& c, const Inputs& in, const State& s) {
    if (s.mode == 4) return;
    static const char* const vorArc[2] = {
        "M384,251 L384,179 M384,128 L384,155 L370,179 L398,179 L384,155 M384,1112 L384,1085 M384,989 L384,1061 L370,1085 L398,1085 L384,1061",
        "M377,251 L377,219 L370,219 L384,195 L398,219 L391,219 L391,251 M384,195 L384,128 M384,1112 L384,1045 M377,989 L377,1045 L391,1045 L391,989"};
    static const char* const vorRose[2] = {
        "M384,257 L384,185 M384,134 L384,161 L370,185 L398,185 L384,161 M384,634 L384,607 M384,511 L384,583 L370,607 L398,607 L384,583",
        "M377,257 L377,225 L370,225 L384,201 L398,225 L391,225 L391,256 M384,201 L384,134 M384,634 L384,567 M377,511 L377,567 L391,567 L391,511"};
    static const char* const adfArc[2] = {
        "M384,251 L384,128 M370,179 L384,155 L398,179 M384,1112 L384,989 M370,1085 L384,1061 L398,1085",
        "M370,251 L370,219 L384,195 L398,219 L398,251 M384,195 L384,128 M384,1112 L384,1023 M370,989 L370,1040 L384,1023 L398,1040 L398,989"};
    static const char* const adfRose[2] = {
        "M384,257 L384,134 M370,185 L384,161 L398,185 M384,634 L384,511 M370,607 L384,583 L398,607",
        "M370,257 L370,225 L384,201 L398,225 L398,257 M384,201 L384,134 M384,634 L384,545 M370,511 L370,562 L384,545 L398,562 L398,511"};
    bool arc = s.mode == 3;
    float cy = arc ? 620.f : 384.f;
    c.save();
    if (arc) c.clipPath(arcMapClip(), true);
    for (int idx = 1; idx <= 2; idx++) {
        char nm[64];
        std::snprintf(nm, sizeof nm, "L:A32NX_EFIS_%c_NAVAID_%d_MODE", s.sc, idx);
        int mode = (int)in.var(nm, {}, 0);
        SkPath path;
        SkColor col;
        double rot;
        if (mode == 2) {
            std::snprintf(nm, sizeof nm, "NAV HAS NAV:%d", idx);
            if (in.var(nm, {}, 0) == 0 || !s.heading.normal() || !s.track.normal()) continue;
            col = stationFlags(in, s, idx, true).corrected ? kMagenta : kWhite;
            rot = s.dyn->needle_filtered[idx - 1];
            path = svgLines(arc ? vorArc[idx - 1] : vorRose[idx - 1]);
        } else if (mode == 1) {
            std::snprintf(nm, sizeof nm, "ADF SIGNAL:%d", idx);
            if (in.var(nm, {}, 0) <= 0) continue;
            std::snprintf(nm, sizeof nm, "ADF RADIAL:%d", idx);
            rot = in.var(nm, "degrees", 0);
            col = kGreen;
            path = svgLines(arc ? adfArc[idx - 1] : adfRose[idx - 1]);
        } else {
            continue;
        }
        c.save();
        c.rotate((float)rot, 384, cy);
        prim::path(kPm, c, path, strokePaint(kBlack, 3.7f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
        prim::path(kPm, c, path, strokePaint(col, 3.2f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
        c.restore();
    }
    c.restore();
}

void rwyAheadAdvisory(SkCanvas& c, const Inputs& in) {
    Word w = arinc(in.var("L:A32NX_OANS_WORD_1"));
    if (!((w.normal() || w.ft()) && (((int64_t)w.value >> 10) & 1))) return;
    SkRect r = SkRect::MakeXYWH(273, 209, 210, 40);
    prim::rect(kPm, c, r, fillPaint(kBlack));
    prim::rect(kPm, c, r, strokePaint(kYellow, 3.f));
    const auto* q = in.bus("ndRwyAheadQfu");
    text(c, q && q->is_string() ? q->get<std::string>() : std::string(), 378, 241, kFontLarge, kYellow, Align::Middle, true);
}

void updateRecompute(State& s) {
    Dyn& d = *s.dyn;
    if (!d.init) {
        d.init = true;
        d.cur_mode = s.mode;
        d.prev_range = s.range_idx;
    }
    auto delay = [&] {
        d.rng = d.rng * 1664525u + 1013904223u;
        return 500.0 + (d.rng >> 8) / 16777216.0 * 70.0;
    };
    if (d.range_change && s.now >= d.range_end) d.range_change = false;
    if (d.page_change && s.now >= d.page_end) d.page_change = false;
    if (s.range_idx != d.prev_range) {
        d.prev_range = s.range_idx;
        if (d.cur_mode != 0 && d.cur_mode != 1 && !s.map_flag) {
            d.page_change = false;
            d.range_change = true;
            d.range_end = s.now + delay();
        } else {
            d.range_change = false;
        }
    }
    if (s.mode != d.cur_mode) {
        d.cur_mode = s.mode;
        if (s.mode != 0 && s.mode != 1 && !d.range_change && !s.map_flag) {
            d.page_change = true;
            d.page_end = s.now + delay();
        } else {
            d.page_change = false;
        }
    }
}

bool tcasMessageShown(const State& s) {
    if (s.mode == 4) return false;
    return s.tcas_fault || s.ta_only || s.tcas_mode == 0;
}

void fmMessages(SkCanvas& c, const Inputs& in, const State& s) {
    int64_t flags = (int64_t)in.var(sv(in, "L:A32NX_EFIS_%c_ND_FM_MESSAGE_FLAGS", s.sc), {}, 0);
    struct Msg { int bit; const char* text; SkColor col; int prio; };
    static const Msg all[] = {
        {1 << 0, "SELECT TRUE NORTH REF", kAmber, 1}, {1 << 1, "CHECK NORTH REF", kAmber, 1},
        {1 << 2, "NAV ACCUR DOWNGRAD", kAmber, 1},    {1 << 3, "NAV ACCUR UPGRADED", kWhite, 1},
        {1 << 4, "SPECIF VOR-D NOT AVAIL", kAmber, 1}, {1 << 6, "NAV PRIMARY", kWhite, 1},
        {1 << 7, "MAP PARTLY DISPLAYED", kAmber, 2},  {1 << 8, "OFFSIDE FM CONTROL", kAmber, 4},
        {1 << 9, "NAV PRIMARY LOST", kAmber, 7},
    };
    std::vector<int>& act = s.dyn->fm_active;
    for (int i = 0; i < (int)(sizeof all / sizeof all[0]); i++) {
        auto it = std::find(act.begin(), act.end(), i);
        if (flags & all[i].bit) {
            if (it != act.end()) continue;
            act.push_back(i);
            std::stable_sort(act.begin(), act.end(), [&](int a, int b) { return all[a].prio > all[b].prio; });
        } else if (it != act.end()) {
            act.erase(it);
        }
    }
    if (act.empty() || s.mode == 0 || s.mode == 1) return;
    std::vector<const Msg*> active;
    for (int i : act) active.push_back(&all[i]);
    c.save();
    c.translate(164, 707);
    if (s.mode == 3 || s.mode == 2) prim::rect(kPm, c, SkRect::MakeXYWH(0, 27, 440, 34), fillPaint(kBlack));
    SkRect r = SkRect::MakeXYWH(0, 0, 440, 27);
    prim::rect(kPm, c, r, fillPaint(kBlack));
    prim::rect(kPm, c, r, strokePaint(kGrey, 1.75f));
    const Msg* last = active.back();
    text(c, last->text, 210, 25, 27.f, last->col, Align::Middle, false, "Ecam", !tcasMessageShown(s));
    if (active.size() > 1) {
        SkPathBuilder p;
        p.moveTo(428, 2).lineTo(428, 20).lineTo(424, 20).lineTo(430, 28).lineTo(436, 20).lineTo(432, 20).lineTo(432, 2).lineTo(428, 2);
        prim::path(kPm, c, p.detach(), fillPaint(kGreen));
    }
    c.restore();
}

void lsCourseBug(SkCanvas& c, const Inputs& in, const State& s) {
    if ((s.mode != 3 && s.mode != 2) || !s.heading.normal()) return;
    if (in.var(sv(in, "L:A380X_EFIS_%c_LS_BUTTON_IS_ON", s.sc)) == 0) return;
    double course = in.var("L:A32NX_FM_LS_COURSE", {}, -1);
    if (course < 0) return;
    double diff = smallestAngle(course, s.heading.value);
    if (std::fabs(diff) > 48) return;
    c.save();
    c.rotate((float)diff, 384, s.mode == 3 ? 620.f : 384.f);
    SkPathBuilder p;
    p.moveTo(376, 114).lineTo(392, 114).moveTo(384, 122).lineTo(384, 74);
    SkPath pp = p.detach();
    prim::path(kPm, c, pp, strokePaint(kBlack, 2.5f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
    prim::path(kPm, c, pp, strokePaint(kMagenta, 2.5f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
    c.restore();
}

void crossTrackError(SkCanvas& c, const Inputs& in, const State& s) {
    double xte = s.dyn->xte.sample(s.now, in.var("L:A32NX_FG_CROSS_TRACK_ERROR", {}, 0));
    if (s.map_flag || s.mode > 4 || s.mode < 2) return;
    double rnp = in.var(sv(in, "L:A32NX_FMGC_%c_RNP", s.sc), {}, 0);
    double a = std::min(99.9, std::fabs(xte));
    std::string t;
    char b[16];
    if (rnp > 0 && rnp <= 0.3 + 1e-12 && a >= 0.02 - 1e-12 && a < 0.3 + 1e-12) {
        std::snprintf(b, sizeof b, "%.2f", a);
        t = b;
    } else if (a >= 0.1) {
        std::snprintf(b, sizeof b, "%.1f", a);
        t = b;
    }
    if (t.empty()) return;
    float x0 = s.mode == 4 ? 44.f : 390.f;
    float y = s.mode == 3 ? 646.f : (s.mode == 4 ? 690.f : 407.f);
    Align al = Align::Start;
    float x = x0;
    if (xte < 0) {
        t += "R";
        x = x0 + 34;
    } else {
        t += "L";
        al = Align::End;
        x = x0 - 38;
    }
    if (s.mode == 4) {
        x = x0;
        al = Align::Start;
    }
    text(c, t, x, y, kFontSmall, kGreen, al, true);
}


void ilsInfo(SkCanvas& c, const Inputs& in, const State& s) {
    int idx = s.sc == 'L' ? 2 : 1;
    int nav = idx + 2;
    char nm[64];
    std::snprintf(nm, sizeof nm, "NAV ACTIVE FREQUENCY:%d", nav);
    double freq = in.var(nm, "MHz", 0);
    bool valid = freq >= 108 && freq <= 112;
    std::snprintf(nm, sizeof nm, "NAV OBS:%d", nav);
    double course = in.var(nm, {}, 0);
    c.save();
    c.translate(748, 28);
    text(c, "ILS" + std::to_string(idx), -102, 0, 25.f, kWhite, Align::End);
    std::string ip = "---", dp = ".--";
    if (valid) {
        char b[16];
        std::snprintf(b, sizeof b, "%.2f", freq);
        std::string f = b;
        auto dot = f.find('.');
        ip = f.substr(0, dot);
        dp = f.substr(dot);
    }
    float wd = textWidth(dp, 20.f);
    text(c, dp, 0, 0, 20.f, kMagenta, Align::End);
    text(c, ip, -wd, 0, 25.f, kMagenta, Align::End);
    text(c, "CRS", -56, 30, 25.f, kWhite, Align::End);
    text(c, "°", 20, 30, 25.f, kCyan, Align::End);
    float wdeg = textWidth("°", 25.f);
    text(c, freq > 0 ? pad3((int)std::lround(course)) : "---", 20 - wdeg, 30, 25.f, kMagenta, Align::End);
    if (freq > 0) underlinedEnd(c, tuningModeBit(in, 14 + idx), -80, 58);
    text(c, in.str((std::string("NAV IDENT:") + std::to_string(nav)).c_str()), 0, 60, 25.f, kMagenta, Align::End);
    c.restore();
}

void glideSlope(SkCanvas& c, const Inputs& in) {
    if (busBool(in, "fm.1.backbeam")) return;
    c.save();
    c.translate(750, 384);
    for (int y : {-128, -64, 64, 128}) stroke_circle(c, 0, (float)y, 4, 2.5f, kWhite);
    prim::line(kPm, c, -12, 0, 12, 0, strokePaint(kYellow, 5.f));
    if (in.var("L:A32NX_RADIO_RECEIVER_GS_IS_VALID") != 0) {
        double dev = in.var("L:A32NX_RADIO_RECEIVER_GS_DEVIATION") / 0.8 * 128;
        SkPaint m = strokePaint(kMagenta, 2.5f, SkPaint::kRound_Cap, SkPaint::kRound_Join);
        if (dev < 128) {
            c.save();
            c.translate(0, (float)std::max(-128.0, dev));
            SkPathBuilder p;
            p.moveTo(10, 0).lineTo(0, -16).lineTo(-10, 0);
            prim::path(kPm, c, p.detach(), m);
            c.restore();
        }
        if (dev > -128) {
            c.save();
            c.translate(0, (float)std::min(128.0, dev));
            SkPathBuilder p;
            p.moveTo(-10, 0).lineTo(0, 16).lineTo(10, 0);
            prim::path(kPm, c, p.detach(), m);
            c.restore();
        }
    }
    c.restore();
}

void ilsCapture(SkCanvas& c, const Inputs& in, const State& s) {
    if (!s.heading.normal()) return;
    int nav = (s.sc == 'L' ? 2 : 1) + 2;
    char nm[64];
    std::snprintf(nm, sizeof nm, "NAV OBS:%d", nav);
    double course = in.var(nm, {}, 0);
    std::snprintf(nm, sizeof nm, "NAV ACTIVE FREQUENCY:%d", nav);
    double freq = in.var(nm, "MHz", 0);
    std::snprintf(nm, sizeof nm, "NAV RADIAL ERROR:%d", nav);
    double err = in.var(nm, {}, 0);
    c.save();
    c.rotate((float)(course - s.heading.value), 384, 384);
    for (int x : {236, 310, 458, 532}) stroke_circle(c, (float)x, 384, 4.5f, 2.f, kWhite);
    if (freq >= 108 && freq <= 112) {
        SkPathBuilder p;
        p.moveTo(352, 256).lineTo(416, 256).moveTo(384, 134).lineTo(384, 294).moveTo(384, 474).lineTo(384, 634);
        SkPath pp = p.detach();
        prim::path(kPm, c, pp, strokePaint(kBlack, 4.5f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
        prim::path(kPm, c, pp, strokePaint(kMagenta, 4.f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
    }
    bool backbeam = busBool(in, "fm.1.backbeam");
    if (backbeam) text(c, "B/C", 202, 416, kFontSmall, kMagenta);
    if (in.var("L:A32NX_RADIO_RECEIVER_LOC_IS_VALID") != 0) {
        double e = std::fmod(std::fmod(err, 360.0) + 360.0, 360.0);
        if (e >= 180) e -= 360;
        if (e < -90) e = -180 - e;
        else if (e > 90) e = 180 - e;
        double dots = (backbeam ? -1.0 : 1.0) * std::clamp(e / 0.8, -2.0, 2.0);
        c.save();
        c.translate((float)(dots * 74), 0);
        prim::line(kPm, c, 384, 304, 384, 464, strokePaint(kBlack, 4.5f, SkPaint::kRound_Cap));
        prim::line(kPm, c, 384, 304, 384, 464, strokePaint(kMagenta, 4.f, SkPaint::kRound_Cap));
        c.restore();
    }
    c.restore();
}


void vorInfo(SkCanvas& c, const Inputs& in, const State& s) {
    int idx = s.sc == 'L' ? 1 : 2;
    char nm[64];
    std::snprintf(nm, sizeof nm, "NAV ACTIVE FREQUENCY:%d", idx);
    double freq = in.var(nm, "MHz", 0);
    std::snprintf(nm, sizeof nm, "NAV OBS:%d", idx);
    double course = in.var(nm, {}, 0);
    c.save();
    c.translate(748, 28);
    text(c, "VOR" + std::to_string(idx), -102, 0, 25.f, kWhite, Align::End);
    std::string ip = "---", dp = ".--";
    if (freq > 0) {
        char b[16];
        std::snprintf(b, sizeof b, "%.2f", freq);
        std::string f = b;
        auto dot = f.find('.');
        ip = f.substr(0, dot);
        dp = f.substr(dot);
    }
    text(c, ip, -40, 0, 25.f, kWhite, Align::End);
    text(c, dp, 0, 0, 20.f, kWhite, Align::End);
    text(c, "CRS", -56, 30, 25.f, kWhite, Align::End);
    text(c, (freq > 0 ? pad3((int)std::lround(course)) : std::string("---")) + "°", 20, 30, 25.f, kCyan, Align::End);
    if (freq > 0) underlinedEnd(c, tuningModeBit(in, 10 + idx), -80, 58);
    text(c, in.str((std::string("NAV IDENT:") + std::to_string(idx)).c_str()), 0, 60, 25.f, kWhite, Align::End);
    c.restore();
}

void vorCapture(SkCanvas& c, const Inputs& in, const State& s) {
    if (!s.heading.normal()) return;
    int idx = s.sc == 'L' ? 1 : 2;
    char nm[64];
    std::snprintf(nm, sizeof nm, "NAV OBS:%d", idx);
    double course = in.var(nm, {}, 0);
    std::snprintf(nm, sizeof nm, "NAV ACTIVE FREQUENCY:%d", idx);
    double freq = in.var(nm, "MHz", 0);
    c.save();
    c.rotate((float)(course - s.heading.value), 384, 384);
    for (int x : {236, 310, 458, 532}) stroke_circle(c, (float)x, 384, 5, 3.f, kWhite);
    if (freq > 0) {
        SkPathBuilder p;
        p.moveTo(352, 256).lineTo(416, 256).moveTo(384, 134).lineTo(384, 294).moveTo(384, 474).lineTo(384, 634);
        SkPath pp = p.detach();
        prim::path(kPm, c, pp, strokePaint(kBlack, 4.5f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
        prim::path(kPm, c, pp, strokePaint(kCyan, 4.f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
    }
    std::snprintf(nm, sizeof nm, "NAV HAS NAV:%d", idx);
    if (in.var(nm, {}, 0) != 0) {
        std::snprintf(nm, sizeof nm, "NAV RADIAL ERROR:%d", idx);
        double dev = in.var(nm, {}, 0);
        bool toward = std::fabs(dev) <= 90;
        double cdi = toward ? dev : (dev < 0 ? -1.0 : 1.0) * -diffAngle(180, std::fabs(dev));
        float px = (float)(std::clamp(cdi, -12.0, 12.0) * 74.0 / 5.0);
        SkPathBuilder dp, lp;
        dp.moveTo(372, 322).lineTo(384, 304).lineTo(396, 322);
        lp.moveTo(384, 304).lineTo(384, 464);
        SkPath arrow = dp.detach(), bar = lp.detach();
        c.save();
        c.translate(px, toward ? 0.f : 160.f);
        if (!toward) c.rotate(180, 384, 304);
        prim::path(kPm, c, arrow, strokePaint(kBlack, 4.5f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
        prim::path(kPm, c, arrow, strokePaint(kCyan, 4.f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
        c.restore();
        c.save();
        c.translate(px, 0);
        prim::path(kPm, c, bar, strokePaint(kBlack, 4.5f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
        prim::path(kPm, c, bar, strokePaint(kCyan, 4.f, SkPaint::kRound_Cap, SkPaint::kRound_Join));
        c.restore();
    }
    c.restore();
}

void tcasMessages(SkCanvas& c, const State& s) {
    if (s.mode == 4) return;
    std::string left;
    SkColor col = kWhite;
    if (s.tcas_fault) {
        left = "TCAS";
        col = kAmber;
    } else if (s.ta_only) {
        left = "TA ONLY";
    } else if (s.tcas_mode == 0) {
        left = "TCAS STBY";
    }
    if (left.empty()) return;
    float y = (s.mode == 1 || s.mode == 0) ? 713.f : 680.f;
    c.save();
    c.translate(164, y);
    if (s.mode == 3 || s.mode == 2) prim::rect(kPm, c, SkRect::MakeXYWH(0, 0, 440, 88), fillPaint(kBlack));
    SkRect r = SkRect::MakeXYWH(0, 0, 440, 27);
    prim::rect(kPm, c, r, fillPaint(kBlack));
    prim::rect(kPm, c, r, strokePaint(kGrey, 1.75f));
    text(c, left, 8 + 214, 25, 27.f, col, Align::Middle);
    c.restore();
}

}  // namespace

void stroke_circle(SkCanvas& canvas, float cx, float cy, float r, float width, SkColor color) {
    strokeCircleImpl(canvas, cx, cy, r, width, color);
}

class NdDisplay : public Display {
public:
    explicit NdDisplay(int side)
        : side_(side), du_(side == 1 ? "89" : "91", side == 1 ? std::vector<std::string>{"108PH", "DC_1"} : std::vector<std::string>{"DC_1", "DC_2"}) {}
    SkISize size() const override { return SkISize::Make(768, 1024); }

    void draw(SkCanvas& c, const Inputs& in) override {
        if (!du_.update(c, in, size())) return;
        State s;
        s.arc_rose = &arc_rose_;
        s.rose_rose = &rose_rose_;
        s.side = side_;
        s.sc = side_ == 1 ? 'L' : 'R';
        char sc = s.sc;
        s.mode = (int)in.var(sv(in, "L:A32NX_EFIS_%c_ND_MODE", sc));
        s.range_idx = (int)in.var(sv(in, "L:A32NX_EFIS_%c_ND_RANGE", sc));
        s.range = rangeFor(s.range_idx);
        s.lr = s.range_idx == 0 ? -1.0 : s.range;
        double knob = in.var("L:A32NX_ATT_HDG_SWITCHING_KNOB");
        s.ir = side_ == 1 ? (knob == 0 ? 3 : 1) : (knob == 2 ? 3 : 2);
        char nm[96];
        auto irvar = [&](const char* suffix) {
            std::snprintf(nm, sizeof nm, "L:A32NX_ADIRS_IR_%d_%s", s.ir, suffix);
            return arinc(in.var(nm));
        };
        s.lat = irvar("LATITUDE");
        s.lon = irvar("LONGITUDE");
        Word mh = irvar("HEADING"), mt = irvar("TRACK");
        s.true_heading = irvar("TRUE_HEADING");
        s.true_track = irvar("TRUE_TRACK");
        Word maint = irvar("MAINT_WORD");
        bool maintOk = maint.normal() || maint.ft();
        bool req = maintOk && (((int64_t)maint.value >> 14) & 1);
        bool pb = in.var("L:A32NX_PUSH_TRUE_REF") != 0;
        s.true_ref = (req || pb) && !(maintOk && (((int64_t)maint.value >> 1) & 1));
        s.heading = s.true_ref ? s.true_heading : mh;
        s.track = s.true_ref ? s.true_track : mt;
        s.tcas_mode = (int)in.var("L:A32NX_TCAS_MODE");
        s.ta_only = in.var("L:A32NX_TCAS_TA_ONLY") != 0;
        s.tcas_fault = in.var("L:A32NX_TCAS_FAULT") != 0;
        if (!in.has_var("L:A32NX_TCAS_TA_ONLY")) {
            Word ra = arinc(in.var("L:A32NX_RA_1_RADIO_ALTITUDE"));
            Word pa = arinc(in.var("L:A32NX_ADIRS_ADR_1_ALTITUDE"));
            s.tcas_mode = 2;
            s.tcas_fault = !pa.normal() || in.var("L:A32NX_ELEC_DC_1_BUS_IS_POWERED") <= 0;
            if (s.tcas_fault) s.tcas_mode = 0;
            bool raFailed = ra.ssm == 0;
            s.ta_only = raFailed || (ra.normal() && ra.value < 1000);
        }
        s.fms_failed = in.var(sv(in, "L:A32NX_FMS_%c_FAILED", sc)) != 0;
        s.oans = oans_shown(in, side_);
        s.mode = std::clamp(s.mode, 0, 4);

        bool plan = s.mode == 4;
        bool rose = s.mode <= 2;
        s.map_flag = !plan && (!s.heading.normal() || !s.lat.normal() || !s.lon.normal());

        s.dyn = &dyn_;
        s.now = in.now_ms();
        updateRecompute(s);
        updateIdents(in, s);
        updateNeedles(in, s);
        bool recomputing = dyn_.range_change || dyn_.page_change;
        int reason = dyn_.range_change && dyn_.page_change ? 3 : (dyn_.range_change ? 1 : (dyn_.page_change ? 2 : 0));
        if (!bus_checked_) {
            bus_checked_ = true;
            bus_owned_ = in.bus("set_map_recomputing") == nullptr;
        }
        if (bus_owned_) {
            Inputs& mi = const_cast<Inputs&>(in);
            mi.set_bus("set_map_recomputing", recomputing);
            mi.set_bus("set_map_recomputing_reason", reason);
        } else {
            if (auto* b = in.bus("set_map_recomputing"); b && b->is_boolean()) recomputing = b->get<bool>();
            if (auto* b = in.bus("set_map_recomputing_reason"); b && b->is_number()) reason = b->get<int>();
        }
        s.recomputing = recomputing;
        s.range_flag = recomputing && (reason == 1 || reason == 3);
        s.mode_flag = recomputing && reason == 2;

        Frame f;
        f.side = side_;
        f.mode = s.mode;
        f.range_index = s.range_idx;
        f.range_nm = s.range;
        if (s.mode == 3) {
            f.centre = SkPoint::Make(384, 626);
            f.px_per_nm = (float)(498.0 / s.range);
        } else {
            f.centre = SkPoint::Make(384, 384);
            f.px_per_nm = (float)(250.0 / (s.range / 2));
        }
        if (plan) {
            Word mlat = arinc(in.var(sv(in, "L:A32NX_EFIS_%c_MRP_LAT", sc)));
            Word mlon = arinc(in.var(sv(in, "L:A32NX_EFIS_%c_MRP_LONG", sc)));
            f.ref_lat = mlat.value;
            f.ref_lon = mlon.value;
            f.up_true_deg = 0;
            f.map_available = mlat.normal() && mlon.normal();
        } else {
            f.ref_lat = s.lat.value;
            f.ref_lon = s.lon.value;
            f.up_true_deg = s.true_heading.value;
            f.map_available = !s.map_flag && s.true_heading.normal() && s.mode >= 2 && !s.fms_failed;
        }

        f.true_heading_deg = s.true_heading.value;

        bool planeVisible, circle;
        SkPoint planePos;
        double planeRot = 0;
        if (plan) {
            bool showPlane = s.true_heading.normal() && s.lat.normal() && s.lon.normal() && f.map_available;
            planeVisible = showPlane && s.heading.normal();
            circle = showPlane && !s.heading.normal();
            planePos = showPlane ? f.project(s.lat.value, s.lon.value) : SkPoint::Make(384, 384);
            planeRot = s.true_heading.value - f.up_true_deg;
        } else {
            planeVisible = s.heading.normal();
            circle = !s.heading.normal();
            planePos = SkPoint::Make(384, s.mode == 3 ? 626.f : 384.f);
        }

        if (s.oans) {
            draw_oans(c, in, f);
            if (!plan) {
                windIndicator(c, in, s);
                speedIndicator(c, in, s);
                topMessages(c, in, s);
            } else {
                speedIndicator(c, in, s);
            }
            tcasMessages(c, s);
            fmMessages(c, in, s);
            rwyAheadAdvisory(c, in);
            draw_vertical_display(c, in, f);
            draw_oans_menu(c, in, side_);
            return;
        }

        draw_terrain(c, in, f);
        blackAreas(c, s.mode);
        if (s.mode == 0) {
            ilsInfo(c, in, s);
            glideSlope(c, in);
        }
        if (s.mode == 1) vorInfo(c, in, s);
        if (s.mode == 3) arcUnderlay(c, s);
        else if (rose) roseUnderlay(c, s);
        else planUnderlay(c, s);
        if (s.mode == 0) ilsCapture(c, in, s);
        if (s.mode == 1) vorCapture(c, in, s);

        lsCourseBug(c, in, s);
        selectedHeadingBug(c, in, s);
        trackLine(c, in, s);
        trackBug(c, s);
        windIndicator(c, in, s);
        speedIndicator(c, in, s);
        toWaypoint(c, in, s);
        topMessages(c, in, s);
        flags(c, in, s);
        terrainThresholds(c, in, s.sc);
        radioNavInfo(c, in, s);

        if (f.map_available && !s.recomputing) draw_route(c, in, f);
        if (f.map_available && !s.recomputing) draw_traffic(c, in, f);

        airplane(c, s, planePos, planeRot, planeVisible, circle);
        tcasMessages(c, s);
        fmMessages(c, in, s);
        rwyAheadAdvisory(c, in);
        crossTrackError(c, in, s);
        radioNeedles(c, in, s);
        draw_vertical_display(c, in, f);
    }

private:
    RoseCache arc_rose_, rose_rose_;
    Dyn dyn_;
    bool bus_checked_ = false, bus_owned_ = false;
    int side_;
    CdsDisplayUnit du_;
};

}  // namespace a380::nd

static std::unique_ptr<a380::Display> make_nd(std::string_view variant) {
    return std::make_unique<a380::nd::NdDisplay>(variant == "2" ? 2 : 1);
}

A380_REGISTER_DISPLAY("nd", make_nd);
