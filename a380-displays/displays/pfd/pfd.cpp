#include <algorithm>
#include "a380/env.h"
#include "a380/shape_cache.h"
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <memory>
#include <string>
#include <unordered_map>
#include <vector>

#include "include/codec/SkCodec.h"
#include "include/codec/SkPngDecoder.h"
#include "include/core/SkCanvas.h"
#include "include/core/SkData.h"
#include "include/core/SkImage.h"
#include "include/core/SkPath.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkSurface.h"
#include "include/effects/SkImageFilters.h"
#include "include/utils/SkParsePath.h"

#include "a380/assets.h"
#include "a380/display.h"
#include "a380/display_unit.h"
#include "a380/draw.h"
#include "a380/layer.h"
#include "a380/raster.h"
#include "a380/fonts.h"
#include "a380/inputs.h"
#include "a380/profile.h"
#include "pfd/barberpoles.h"
#include "pfd/limitations.h"

namespace a380::pfd {
namespace {

constexpr float kS = 768.f / 158.75f;
constexpr float kMm = 3.7795f;

const SkColor kGreen = 0xFF00FF00, kCyan = 0xFF00FFFF, kWhite = 0xFFFFFFFF, kAmber = 0xFFE68000, kYellow = 0xFFFFFF00,
              kRed = 0xFFFF0000, kGrey = 0xFF4F5467, kBlack = 0xFF000000, kSky = 0xFF005BC4, kEarth = 0xFF65290A,
              kMagenta = 0xFFFF94FF, kDimCyan = 0xFF00B3B3;

struct Word {
    int ssm = 0;
    double value = 0;
    bool normal() const { return ssm == 3; }
    bool ncd() const { return ssm == 1; }
    bool fw() const { return ssm == 0; }
    bool ft() const { return ssm == 2; }
    double or0() const { return normal() ? value : 0.0; }
};

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

bool bit(double raw, int n) {
    double v = arinc(raw).value;
    if (!(std::fabs(v) < 2147483648.0)) return false;
    return (((int32_t)v >> (n - 1)) & 1) != 0;
}

bool bit_or(double raw, int n, bool def = false) {
    Word w = arinc(raw);
    return (w.normal() || w.ft()) ? bit(raw, n) : def;
}

double js_round(double v) { return std::floor(v + 0.5); }

enum class A { Start, Middle, End };

struct Ctx {
    SkCanvas& c;
    const Inputs& in;
    int adiru;
    int prim;
    bool fo;

    Word w(const char* fmt, int n) const {
        char b[160];
        std::snprintf(b, sizeof b, fmt, n);
        return arinc(in.var(b));
    }
    double v(const char* name, double fb = 0) const { return in.var(name, {}, fb); }
};

SkPath svg(const std::string& d) {
    thread_local std::unordered_map<std::string, SkPath> cache;
    auto it = cache.find(d);
    if (it == cache.end()) {
        SkPath p;
        if (auto r = SkParsePath::FromSVGString(d.c_str())) p = *r;
        if (cache.size() > 8192) cache.clear();
        it = cache.emplace(d, p).first;
    }
    return it->second;
}

SkPaint stroke_paint(SkColor col, float w, bool joinRound = false) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setStyle(SkPaint::kStroke_Style);
    p.setColor(col);
    p.setStrokeWidth(w);
    p.setStrokeCap(SkPaint::kRound_Cap);
    if (joinRound) p.setStrokeJoin(SkPaint::kRound_Join);
    return p;
}

SkPaint fill_paint(SkColor col) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(col);
    return p;
}

constexpr float kSmall = 0.10f * kMm, kNormal = 0.16f * kMm, kThick = 0.22f * kMm, kHuge = 0.32f * kMm;
constexpr float kNormalOut = 0.21f * kMm, kSmallOut = 0.09f * kMm;

void stroke(SkCanvas& c, const char* d, SkColor col, float w = kNormal, bool joinRound = false) {
    a380::chromium_stroke_path(c, svg(d), stroke_paint(col, w, joinRound));
}
void outlined(SkCanvas& c, const char* d, SkColor col, float w = kNormal, float ow = kNormalOut, bool joinRound = false) {
    SkPath p = svg(d);
    a380::draw_path_cached(c, p, stroke_paint(kBlack, ow, joinRound));
    a380::draw_path_cached(c, p, stroke_paint(col, w, joinRound));
}
void fill(SkCanvas& c, const char* d, SkColor col) {
    a380::draw_path_cached(c, svg(d), fill_paint(col));
}
void fill_eo(SkCanvas& c, const char* d, SkColor col) {
    a380::draw_path_cached(c, svg(d).makeFillType(SkPathFillType::kEvenOdd), fill_paint(col));
}

SkRect layer_bounds(const char* d, float w) {
    SkRect r = svg(d).getBounds();
    return r.makeOutset(w * 0.5f + 0.5f, w * 0.5f + 0.5f);
}

SkPaint soft_layer_paint(float ka) {
    const SkScalar kern[3] = {ka, 1.f - 2 * ka, ka};
    SkPaint p;
    p.setImageFilter(SkImageFilters::MatrixConvolution(SkISize::Make(3, 1), kern, 1.f, 0.f, SkIPoint::Make(1, 0),
                                                       SkTileMode::kDecal, true, nullptr));
    return p;
}

void text(SkCanvas& c, const std::string& s, float x, float y, float size, SkColor col, A a = A::Start) {
    if (s.empty()) return;
    SkFont f = a380::font("Ecam", size);
    float w = f.measureText(s.data(), s.size(), SkTextEncoding::kUTF8);
    float x0 = a == A::Start ? x : a == A::Middle ? x - w * 0.5f : x - w;
    a380::draw_simple_text(c, s.data(), s.size(), SkTextEncoding::kUTF8, x0, y, f, fill_paint(col));
}

void text_outline(SkCanvas& c, const std::string& s, float x, float y, float size, SkColor col, A a = A::Start) {
    if (s.empty()) return;
    SkFont f = a380::font("Ecam", size);
    float w = f.measureText(s.data(), s.size(), SkTextEncoding::kUTF8);
    float x0 = a == A::Start ? x : a == A::Middle ? x - w * 0.5f : x - w;
    a380::draw_stroked_text(c, s, x0, y, f, SkColorSetARGB(0, 0, 0, 0), kBlack, 0.05f * kMm);
    a380::draw_simple_text(c, s.data(), s.size(), SkTextEncoding::kUTF8, x0, y, f, fill_paint(col));
}

constexpr float kLargest = 7, kLarge = 6.5f, kMedium = 6, kInterm = 5.5f, kMediumSmaller = 5.4f, kSmallest = 4.5f, kTiny = 4.f;

float horizon_offset(double pitch) {
    if (pitch > -5 && pitch <= 20) return (float)(pitch * 1.8);
    if (pitch > 20 && pitch <= 30) return (float)(-0.04 * pitch * pitch + 3.4 * pitch - 16);
    if (pitch > 30) return (float)(20 + pitch);
    if (pitch < -5 && pitch >= -15) return (float)(0.04 * pitch * pitch + 2.2 * pitch + 1);
    return (float)(pitch - 8);
}

float roll_vertical_offset(double roll) {
    float off = 0;
    if (std::fabs(roll) > 60) off = (float)std::max(0.0, 41 - 35.87 / std::sin(std::fabs(roll) / 180 * 3.14159265358979));
    return off;
}

struct FmaTrack {
    bool have = false;
    uint64_t key[8] = {};
    double at[8] = {};
};

struct HorizonCache {
    sk_sp<SkImage> image;
    int law = -1;
    float scale = 0;
};

struct Edge {
    bool prev = false;
    double at = 0;
};

struct Lag {
    double in = 0, out = 0;
    bool init = false;
    double step(double x, double dt, double rate) {
        if (std::isnan(x)) x = 0;
        if (!init) {
            init = true;
            in = out = x;
            return x;
        }
        double k = dt * rate, sum = k + 2;
        double o = (x + in) * k / sum + (2 - k) / sum * out;
        in = x;
        if (!std::isfinite(o)) return 0;
        out = o;
        return o;
    }
    void reset() {
        init = true;
        in = out = 0;
    }
};

struct RateLim {
    double out = 0;
    bool init = false;
    double step(double x, double dt, double up, double down) {
        if (std::isnan(x)) x = 0;
        if (!init) {
            init = true;
            out = x;
            return x;
        }
        out += std::max(std::min(dt * up, x - out), dt * down);
        return out;
    }
};

struct Mem {
    Edge blink[32], flash[8];
    Lag vs, ra, loc, gs;
    RateLim trend;
    double altOut = 0;
    bool altInit = false;
    double trendSampMs = -1e18, trendSamp = 0;
    bool trendShow = false;
    double attEvalMs = -1e18;
    bool attExcessive = false;
    bool airborne = false;
    double airborneAt = 0;
    bool showMach = false;
    double latAccMs = -1e18, latAcc = 0;
    bool dhShown = false;
    int bc3Prev = 0;
    double bc3At = 0;
    int markerPrev = 0;
    double markerAt = 0;
    int ldState = 0;
    float ldPx = 0;
    double rtMs = -1e18, rtDelayAt = 0, rtConfTimer = 0;
    bool rtCond = false, rtStatus = false, rtEngFailed = false, rtConfPrevIn = false, rtConfOut = false;
};

struct State {
    FmaTrack* fma = nullptr;
    HorizonCache* horizon = nullptr;
    Mem* mem = nullptr;
    Word pitch, roll, heading, track, speed, mach, vs, alt, ra, gs;
    double beta = 0, betaTarget = 0, betaActive = 0, latAcc = 0;
    double slip = 0;
    bool onGround = true;
    float yOff = 0;
    double vsF = 0;
    bool vsMoved = false;
    double now = 0, dt = 0, raF = 0;
    bool offline = false, attExcessive = false, tailStrike = false, showVls = false;
};

enum { kBlSpd, kBlSpdLim, kBlAlt, kBlTcas, kBlAtt, kBlMach, kBlFpv, kBlRa, kBlBaro, kBlAfloor, kBlTogaLk, kBlWarn };
enum { kFlFd, kFlLs, kFlAltSel, kFlSpdSel };

bool clock_high(double now) { return std::fmod(now, 1000.0) >= 500.0; }

bool css_blink(const State& s, Edge& e, bool active, double cycles) {
    if (!active) {
        e.prev = false;
        return false;
    }
    if (!e.prev) {
        e.prev = true;
        e.at = s.now;
    }
    if (s.offline) return true;
    double t = s.now - e.at;
    if (cycles > 0 && t >= cycles * 1000.0) return true;
    return std::fmod(t, 1000.0) >= 505.0;
}

bool blink9(const State& s, int id, bool active) { return css_blink(s, s.mem->blink[id], active, 9); }
bool blink_inf(const State& s, int id, bool active) { return css_blink(s, s.mem->blink[id], active, 0); }

bool flash9(const State& s, int id, bool active) {
    Edge& e = s.mem->flash[id];
    bool rising = active && !e.prev;
    e.prev = active;
    if (rising && s.now >= e.at) e.at = s.now + 9000.0;
    if (!active) return false;
    if (s.offline) return true;
    return !(s.now < e.at && clock_high(s.now));
}

bool flash_inf(const State& s, bool visible, bool flashing) {
    return visible && (s.offline || !(flashing && clock_high(s.now)));
}

bool normal_law(const Ctx& x) {
    return bit_or(x.v("L:A32NX_FCDC_1_DISCRETE_WORD_1"), 11) || bit_or(x.v("L:A32NX_FCDC_2_DISCRETE_WORD_1"), 11);
}

int fm_index(const Ctx& x) {
    bool h1 = x.v("L:A32NX_FM1_HEALTHY_DISCRETE") != 0, h2 = x.v("L:A32NX_FM2_HEALTHY_DISCRETE") != 0;
    bool only1 = h1 && !h2, only2 = h2 && !h1;
    return x.fo ? (only1 ? 1 : 2) : (only2 ? 2 : 1);
}

Word choose_ra(const Word r[3], bool fo) {
    bool valid[3];
    int vc = 0;
    for (int i = 0; i < 3; i++) {
        valid[i] = !r[i].fw() && !r[i].ncd();
        vc += valid[i] ? 1 : 0;
    }
    Word ch[2] = {r[0], r[1]};
    if (vc == 3) {
        Word h[3] = {r[0], r[1], r[2]};
        std::sort(h, h + 3, [](const Word& a, const Word& b) { return a.value < b.value; });
        ch[0] = ch[1] = h[1];
    } else if (vc == 2) {
        if (!valid[0]) {
            ch[0] = r[2];
            ch[1] = r[1];
        } else if (!valid[1]) {
            ch[0] = r[0];
            ch[1] = r[2];
        }
    } else if (vc == 1) {
        const Word& w = valid[0] ? r[0] : valid[1] ? r[1] : r[2];
        ch[0] = ch[1] = w;
    } else {
        bool nf[3];
        int nfc = 0;
        for (int i = 0; i < 3; i++) {
            nf[i] = !r[i].fw();
            nfc += nf[i] ? 1 : 0;
        }
        if (nfc == 2) {
            if (!nf[0]) {
                ch[0] = r[2];
                ch[1] = r[1];
            } else if (!nf[1]) {
                ch[0] = r[0];
                ch[1] = r[2];
            }
        } else if (nfc == 1) {
            const Word& w = nf[0] ? r[0] : nf[1] ? r[1] : r[2];
            ch[0] = ch[1] = w;
        }
    }
    return fo ? ch[1] : ch[0];
}

void draw_radio_alt(Ctx& x, State& s);

const SkRect kLadder = SkRect::MakeLTRB(23.f, -80.f, 115.f, 224.5f);
constexpr float kLadderSupersample = 1.5f;

void draw_ladder(SkCanvas& c, bool normal) {
    fill(c, "m23.906 80.823v-160h90v160z", kSky);
    fill(c, "m113.91 223.82h-90v-143h90z", kEarth);
    static const char* lines[] = {
        "m66.406 85.323h5", "m64.406 89.823h9", "m66.406 94.073h5", "m59.406 97.823h19", "m64.406 103.82h9",
        "m59.406 108.82h19", "m55.906 118.82h26", "m52.906 138.82h32", "m47.906 168.82h42", "m66.406 76.323h5",
        "m64.406 71.823h9", "m66.406 67.323h5", "m59.406 62.823h19", "m66.406 58.323h5", "m64.406 53.823h9",
        "m66.406 49.323h5", "m59.406 44.823h19", "m66.406 40.573h5", "m64.406 36.823h9", "m66.406 33.573h5",
        "m55.906 30.823h26", "m52.906 10.823h32", "m47.906-19.177h42"};
    for (auto d : lines) stroke(c, d, kWhite);
    if (normal) {
        stroke(c, "m51.506 31.523h4m-4-1.4h4", kGreen);
        stroke(c, "m86.306 31.523h-4m4-1.4h-4", kGreen);
        stroke(c, "m59.946 104.52h4m-4-1.4h4", kGreen);
        stroke(c, "m77.867 104.52h-4m4-1.4h-4", kGreen);
    } else {
        stroke(c, "m52.699 30.116 1.4142 1.4142m-1.4142 0 1.4142-1.4142", kAmber);
        stroke(c, "m85.114 31.53-1.4142-1.4142m1.4142 0-1.4142 1.4142", kAmber);
        stroke(c, "m61.199 103.12 1.4142 1.4142m-1.4142 0 1.4142-1.4142", kAmber);
        stroke(c, "m76.614 104.53-1.4142-1.4142m1.4142 0-1.4142 1.4142", kAmber);
    }
    static const char* reds[] = {
        "m68.906 121.82-8.0829 14h2.8868l5.1962-9 5.1962 9h2.8868z",
        "m57.359 163.82 11.547-20 11.547 20h-4.0414l-7.5056-13-7.5056 13z",
        "m71.906 185.32v3.5h15l-18-18-18 18h15v-3.5h-6.5l9.5-9.5 9.5 9.5z",
        "m60.824 13.823h2.8868l5.1962 9 5.1962-9h2.8868l-8.0829 14z",
        "m61.401-13.177h-4.0414l11.547 20 11.547-20h-4.0414l-7.5056 13z",
        "m68.906-26.177-9.5-9.5h6.5v-3.5h-15l18 18 18-18h-15v3.5h6.5z"};
    for (auto d : reds) stroke(c, d, kRed);
    outlined(c, "m23.906 80.823h90", kWhite);
    struct L { float x, y; const char* t; };
    static const L labels[] = {
        {55.729935f, 64.812828f, "10"}, {88.618317f, 64.812714f, "10"}, {54.710766f, 46.931034f, "20"},
        {89.564583f, 46.930969f, "20"}, {50.867237f, 32.910896f, "30"}, {93.408119f, 32.910839f, "30"},
        {48.308414f, 12.690886f, "50"}, {96.054962f, 12.690853f, "50"}, {43.050652f, -17.138285f, "80"},
        {101.48304f, -17.138248f, "80"}, {55.781109f, 99.81395f, "10"}, {88.669487f, 99.813919f, "10"},
        {54.645519f, 110.8641f, "20"}, {89.892426f, 110.86408f, "20"}, {51.001217f, 120.96314f, "30"},
        {93.280037f, 120.96311f, "30"}, {48.220913f, 140.69778f, "50"}, {96.090324f, 140.69786f, "50"},
        {43.125065f, 170.80962f, "80"}, {101.38947f, 170.80959f, "80"}};
    for (auto& l : labels) text(c, l.t, l.x, l.y, kMediumSmaller, kWhite, A::End);
}

sk_sp<SkImage> ladder_image(SkCanvas& c, bool normal, float scale) {
    SkImageInfo info = SkImageInfo::MakeN32Premul((int)std::ceil(kLadder.width() * scale), (int)std::ceil(kLadder.height() * scale));
    sk_sp<SkSurface> surf = c.makeSurface(info);
    if (!surf) surf = SkSurfaces::Raster(info);
    if (!surf) return nullptr;
    SkCanvas* lc = surf->getCanvas();
    lc->clear(SK_ColorTRANSPARENT);
    lc->scale(scale, scale);
    lc->translate(-kLadder.left(), -kLadder.top());
    draw_ladder(*lc, normal);
    return surf->makeImageSnapshot();
}

void draw_horizon(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    if (!s.roll.normal()) return;
    c.save();
    c.rotate((float)-s.roll.value, 68.814f, 80.730f);
    if (s.pitch.normal()) {
        c.save();
        c.translate(0, horizon_offset(s.pitch.value));
        bool normal = normal_law(x);
        HorizonCache* hc = s.horizon;
        if (hc) {
            const SkMatrix m = c.getTotalMatrix();
            float scale = std::round(std::hypot(m.getScaleX(), m.getSkewY()) * kLadderSupersample * 1000.f) / 1000.f;
            if (!hc->image || hc->law != (int)normal || std::fabs(hc->scale - scale) > 1e-3f) {
                hc->image = ladder_image(c, normal, scale);
                hc->law = (int)normal;
                hc->scale = scale;
            }
        }
        if (hc && hc->image) c.drawImageRect(hc->image, kLadder, SkSamplingOptions(SkFilterMode::kLinear), nullptr);
        else draw_ladder(c, normal);
        if (s.tailStrike) stroke(c, "m72.682 50.223h2.9368l-6.7128 8-6.7128-8h2.9368l3.7759 4.5z", kAmber);
        c.restore();
    }
    fill(c, "m40.952 49.249v-20.562h55.908v20.562z", kSky);
    outlined(c, "m40.952 49.249v-20.562h55.908v20.562z", kWhite);

    {
        double slip = s.slip;
        float vo = roll_vertical_offset(std::round(s.roll.value * 100) / 100);
        SkColor col = s.betaActive == 1 ? kCyan : kYellow;
        c.save();
        c.translate(0, vo);
        {
            a380::CompositedLayer layer(c, layer_bounds("m66.074 43.983 2.8604-4.2333 2.8604 4.2333z", kNormal));
            stroke(c, "m66.074 43.983 2.8604-4.2333 2.8604 4.2333z", col, kNormal, true);
        }
        c.translate((float)slip, 0);
        {
            a380::CompositedLayer layer(c, layer_bounds("m73.974 47.208-1.4983-2.2175h-7.0828l-1.4983 2.2175z", kNormal));
            stroke(c, "m73.974 47.208-1.4983-2.2175h-7.0828l-1.4983 2.2175z", col, kNormal, true);
        }
        c.restore();
    }

    {
        double target = (s.ra.ncd() || s.ra.fw()) ? 200 : 0.1 * s.raF;
        float off = std::clamp(horizon_offset(s.pitch.value + target) - 31.563f, -63.093f, 0.f);
        off = std::round(off * 100.f) / 100.f;
        c.save();
        c.translate(0, off);
        outlined(c, "m113.95 157.74h-90.08v-45.357h90.08z", kWhite);
        fill(c, "m113.95 157.74h-90.08v-45.357h90.08z", kEarth);
        stroke(c, "m113.95 157.74h-90.08v-45.357h90.08z", kWhite);
        c.restore();
    }

    if (s.heading.normal()) {
        float tapeOff = (float)(std::fmod(-s.heading.value, 10.0) * 1.5);
        c.save();
        c.translate(tapeOff, s.yOff);
        for (int k = -6; k <= 6; k++) {
            char d[64];
            std::snprintf(d, sizeof d, "m%.3f 80.823v1.8", 68.906f + k * 15.f);
            stroke(c, d, kWhite);
        }
        c.restore();
        {
            char nb[64];
            std::snprintf(nb, sizeof nb, "L:A32NX_PRIM_%d_FG_DISCRETE_WORD_5", x.prim);
            double r5 = x.v(nb);
            Word w5 = arinc(r5);
            bool trk = (w5.normal() || w5.ft()) ? bit(r5, 11) : true;
            std::snprintf(nb, sizeof nb, "L:A32NX_PRIM_%d_FG_DISCRETE_WORD_1", x.prim);
            double r1 = x.v(nb);
            Word w1 = arinc(r1);
            bool anyFd = (w1.normal() || w1.ft()) && (bit(r1, 13) || bit(r1, 14));
            Word sel = x.w(trk ? "L:A32NX_PRIM_%d_SELECTED_TRACK" : "L:A32NX_PRIM_%d_SELECTED_HEADING", x.prim);
            double delta = std::fmod(sel.value - s.heading.value + 540, 360) - 180;
            if (!anyFd && (sel.normal() || sel.ft()) && std::fabs(delta) <= 45) {
                c.save();
                c.translate((float)(delta * 1.5), s.yOff);
                a380::draw_path_cached(c, svg("m68.906 80.823v-7"), stroke_paint(kBlack, 0.29f * kMm));
                a380::chromium_stroke_path(c, svg("m68.906 80.823v-7"), stroke_paint(kCyan, kThick));
                c.restore();
            }
        }
    }
    draw_radio_alt(x, s);
    c.restore();
}

void draw_bird(Ctx& x, State& s, int ir);

bool fd_engaged(const Ctx& x, int& idx) {
    char nb[64];
    std::snprintf(nb, sizeof nb, "L:A32NX_PRIM_%d_FG_DISCRETE_WORD_1", x.prim);
    double r1 = x.v(nb);
    Word w1 = arinc(r1);
    bool ok = w1.normal() || w1.ft();
    bool f1 = ok && bit(r1, 13), f2 = ok && bit(r1, 14);
    bool sel1 = (!x.fo && !f1 && !f2) || (!x.fo && f1) || (x.fo && !f2);
    idx = sel1 ? 1 : 2;
    return sel1 ? f1 : f2;
}

void draw_sidestick(Ctx& x) {
    SkCanvas& c = x.c;
    bool eng = false;
    for (int i = 1; i <= 4; i++) eng = eng || x.v(("GENERAL ENG COMBUSTION:" + std::to_string(i)).c_str()) > 0;
    bool onGround = x.v("L:A32NX_LGCIU_1_LEFT_GEAR_COMPRESSED") > 0 || x.v("L:A32NX_LGCIU_1_RIGHT_GEAR_COMPRESSED") > 0;
    if (!(onGround && eng)) return;
    float sx = (float)(x.v("L:A32NX_SIDESTICK_POSITION_X") * 29.56), sy = (float)(-x.v("L:A32NX_SIDESTICK_POSITION_Y") * 23.02);
    stroke(c, "m92.327 103.75h6.0441v-6.0476m-58.93 0v6.0476h6.0441m46.842-45.861h6.0441v6.0476m-58.93 0v-6.0476h6.0441", kWhite);
    c.save();
    c.translate(sx, sy);
    stroke(c, "m73.994 81.579h-4.3316v4.3341m-5.8426-4.3341h4.3316v4.3341m5.8426-5.846h-4.3316v-4.3341m-5.8426 4.3341h4.3316v-4.3341", kWhite);
    c.restore();
}

void draw_fd(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    int idx = 1;
    if (!fd_engaged(x, idx)) {
        flash9(s, kFlFd, false);
        return;
    }
    char nb[96];
    auto cmd = [&](const char* nm) {
        std::snprintf(nb, sizeof nb, nm, x.prim, idx);
        return arinc(x.v(nb));
    };
    Word roll = cmd("L:A32NX_PRIM_%d_ROLL_FD_COMMAND_%d");
    Word pitch = cmd("L:A32NX_PRIM_%d_PITCH_FD_COMMAND_%d");
    Word yaw = cmd("L:A32NX_PRIM_%d_YAW_FD_COMMAND_%d");
    auto valid = [](const Word& w) { return w.normal() || w.ft(); };
    char fn[64];
    std::snprintf(fn, sizeof fn, "L:A32NX_PRIM_%d_FG_DISCRETE_WORD_5", x.prim);
    double r5 = x.v(fn);
    bool latFlash = bit_or(r5, 27), lonFlash = bit_or(r5, 26);
    if (valid(yaw)) {
        float off = (float)(-std::clamp(yaw.value, -45.0, 45.0) * 0.44);
        c.save();
        c.translate(off, 0);
        fill(c, "m67.899 82.536v13.406h2.0147v-13.406l-1.0074-1.7135z", kBlack);
        stroke(c, "m67.899 82.536v13.406h2.0147v-13.406l-1.0074-1.7135z", kGreen);
        c.restore();
    }
    if (valid(roll) && flash_inf(s, true, latFlash)) {
        c.save();
        c.translate((float)(std::clamp(roll.value, -45.0, 45.0) * 0.44), 0);
        SkPaint lp = soft_layer_paint(0.22f);
        c.saveLayer(SkCanvas::SaveLayerRec(nullptr, &lp, 0));
        a380::chromium_stroke_path(c, svg("m68.903 61.672v38.302"), stroke_paint(kBlack, 0.29f * kMm));
        stroke(c, "m68.903 61.672v38.302", kGreen, kThick);
        c.restore();
        c.restore();
    }
    if (valid(pitch) && flash_inf(s, true, lonFlash)) {
        c.save();
        c.translate(0, (float)(std::clamp(pitch.value, -22.5, 22.5) * 0.89));
        a380::chromium_stroke_path(c, svg("m49.263 80.823h39.287"), stroke_paint(kBlack, 0.29f * kMm));
        stroke(c, "m49.263 80.823h39.287", kGreen, kThick);
        c.restore();
    }
    bool onGround = x.v("L:A32NX_LGCIU_1_LEFT_GEAR_COMPRESSED") > 0;
    if (flash9(s, kFlFd, roll.fw() || pitch.fw() || (yaw.fw() && onGround))) text(c, "FD", 52.702862f, 56.065434f, kLargest, kRed, A::End);
}

void draw_fixed_center(Ctx& x, State& s, int ir) {
    SkCanvas& c = x.c;
    if (!(s.pitch.normal() && s.roll.normal())) {
        flash9(s, kFlFd, false);
        blink9(s, kBlFpv, false);
        if (blink9(s, kBlAtt, true)) text(c, "ATT", 75.893127f, 83.136955f, kLargest, kRed, A::End);
        return;
    }
    blink9(s, kBlAtt, false);
    draw_sidestick(x);
    const char* box = "m67.647 82.083v-2.5198h2.5184v2.5198z";
    fill(c, box, kBlack);
    draw_bird(x, s, ir);
    if (s.attExcessive) flash9(s, kFlFd, false);
    else draw_fd(x, s);
    outlined(c, box, kYellow);
    const char* l = "m34.153 79.563h15.11v6.5516h-2.5184v-4.0317h-12.592z";
    const char* r = "m88.55 86.114h2.5184v-4.0317h12.592v-2.5198h-15.11z";
    for (auto d : {l, r}) {
        a380::draw_path_cached(c, svg(d), stroke_paint(kBlack, kNormalOut));
        fill(c, d, kBlack);
        stroke(c, d, kYellow);
    }
}

void draw_bird(Ctx& x, State& s, int ir) {
    SkCanvas& c = x.c;
    Word fpa = x.w("L:A32NX_ADIRS_IR_%d_FLIGHT_PATH_ANGLE", ir);
    Word da = x.w("L:A32NX_ADIRS_IR_%d_DRIFT_ANGLE", ir);
    char w5n[64];
    std::snprintf(w5n, sizeof w5n, "L:A32NX_PRIM_%d_FG_DISCRETE_WORD_5", x.prim);
    double w5raw = x.v(w5n);
    Word w5 = arinc(w5raw);
    bool trkFpa = (w5.normal() || w5.ft()) ? bit(w5raw, 11) : true;
    const char* w2n = x.fo ? "L:A32NX_FCU_EFIS_R_DISCRETE_WORD_2" : "L:A32NX_FCU_EFIS_L_DISCRETE_WORD_2";
    double w2raw = x.v(w2n);
    Word w2 = arinc(w2raw);
    bool vv = (w2.normal() || w2.ft()) ? bit(w2raw, 15) : true;
    bool requested = trkFpa || vv;
    bool rollPitchOk = s.pitch.normal() && s.roll.normal();
    bool daFpaOk = da.normal() && fpa.normal();
    if (blink9(s, kBlFpv, requested && rollPitchOk && !daFpaOk)) text(c, "FPV", 62.987099f, 89.42025f, kLargest, kRed, A::End);
    if (!(requested && rollPitchOk && daFpaOk)) return;
    int fdIdx = 1;
    bool small = fd_engaged(x, fdIdx);
    double daLim = std::clamp(da.value, -21.0, 21.0) * 15.0 / 10.0;
    double pitchSubFpa = horizon_offset(s.pitch.value) - horizon_offset(fpa.value);
    double rc = std::cos(s.roll.value * 3.14159265358979 / 180), rs = std::sin(-s.roll.value * 3.14159265358979 / 180);
    float xo = (float)(daLim * rc - pitchSubFpa * rs), yo = (float)(pitchSubFpa * rc + daLim * rs);
    c.save();
    c.translate(53.4f + xo, 65.3f + yo);
    const char* circle = small ? "m17 15.5 a1.5 1.5 0 1 0 -3 0 a1.5 1.5 0 1 0 3 0" : "m17.75 15.5 a2.25 2.25 0 1 0 -4.5 0 a2.25 2.25 0 1 0 4.5 0";
    const char* wings = small ? "m17 15.5 h2.5 m-5.5 0 h-2.5 m4 -1.5 v-1.5" : "m17.75 15.5 h5 m-9.5 0 h-5 m7.25 -2.25 v-2";
    SkColor col = trkFpa ? kGreen : kBlack;
    SkColor out = trkFpa ? kBlack : kWhite;
    for (auto d : {circle, wings}) a380::draw_path_cached(c, svg(d), stroke_paint(out, kNormalOut));
    for (auto d : {circle, wings}) a380::chromium_stroke_path(c, svg(d), stroke_paint(col, kNormal));
    c.restore();
}

void draw_fixed_upper(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    if (!(s.pitch.normal() && s.roll.normal())) return;
    if (normal_law(x)) {
        stroke(c, "m105.64 62.887 1.5716-0.8008m-1.5716-0.78293 1.5716-0.8008", kGreen, kSmall);
        stroke(c, "m32.064 61.303-1.5716-0.8008m1.5716 2.3845-1.5716-0.8008", kGreen, kSmall);
    } else {
        stroke(c, "m107.77 60.696-1.7808 1.7818m1.7808 0-1.7808-1.7818", kAmber);
        stroke(c, "m30.043 62.478 1.7808-1.7818m-1.7808 0 1.7808 1.7818", kAmber);
    }
    static const char* w[] = {
        "m98.645 51.067 2.8492-2.8509", "m39.168 51.067-2.8492-2.8509", "m90.858 44.839a42.133 42.158 0 0 0-43.904 0",
        "m89.095 43.819 1.8313-3.1738 1.7448 1.0079-1.8313 3.1738",
        "m84.259 41.563 0.90817-2.4967-1.8932-0.68946-0.90818 2.4966",
        "m75.229 39.142 0.46109-2.6165 1.9841 0.35005-0.46109 2.6165",
        "m60.6 39.492-0.46109-2.6165 1.9841-0.35005 0.46109 2.6165",
        "m53.553 41.563-0.90818-2.4967 0.9466-0.34474 0.9466-0.34472 0.90818 2.4966",
        "m46.973 44.827-1.8313-3.1738 1.7448-1.0079 1.8313 3.1738"};
    for (auto d : w) stroke(c, d, kWhite, kSmall);
    stroke(c, "m68.906 38.650-2.5184-3.7000h5.0367l-2.5184 3.7000", kYellow, kNormal, true);
}

void draw_warnings(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    double rop = x.v("L:A32NX_ROW_ROP_WORD_1");
    bool maxBrkReq = bit_or(rop, 11), wetShort = bit_or(rop, 14), rwyShort = bit_or(rop, 15);
    bool rwyAhead = bit_or(x.v("L:A32NX_OANS_WORD_1"), 11);
    double fwc = x.v("L:A32NX_FWC_1_DISCRETE_WORD_126");
    bool stall = bit_or(fwc, 17) && !bit(fwc, 28);
    bool windshear = bit_or(x.v("L:A32NX_EGPWS_ALERT_1_DISCRETE_WORD_2"), 15);
    double gp = x.v("L:A32NX_EGPWS_ALERT_1_DISCRETE_WORD_1");
    bool pullUp = bit_or(gp, 12), sink = bit_or(gp, 11), terrain = bit_or(gp, 13), dontSink = bit_or(gp, 14);
    bool tlGear = bit_or(gp, 15), tlFlaps = bit_or(gp, 16), tlTerrain = bit_or(gp, 17), glide = bit_or(gp, 18);
    bool maxRev = maxBrkReq && !wetShort && !rwyShort && !stall;
    bool maxBrk = maxBrkReq && !wetShort && !rwyShort && !stall;
    int n = kBlWarn;
    auto warn = [&](bool on, const char* t, float x0, float y0, float size, SkColor col) {
        if (blink9(s, n++, on)) text_outline(c, t, x0, y0, size, col, A::Middle);
    };
    warn(maxRev, "MAX REVERSE", 69.f, 78.f, kLarge, kRed);
    warn(maxBrk, "MAX BRAKING", 69.f, 70.25f, kLarge, kRed);
    warn(wetShort && !rwyShort && !stall, "IF WET:RWY TOO SHORT", 69.f, 70.25f, kMediumSmaller, kAmber);
    warn(rwyShort && !stall, "RWY TOO SHORT", 69.f, 70.25f, kLarge, kRed);
    warn(stall, "STALL\xC2\xA0\xC2\xA0\xC2\xA0STALL", 69.f, 78.f, kLarge, kRed);
    warn(windshear && !maxRev && !maxBrk, "WINDSHEAR", 69.f, 78.f, kLarge, kRed);
    if (rwyAhead) {
        c.drawRect(SkRect::MakeXYWH(50, 69, 38, 8), fill_paint(kBlack));
        c.drawRect(SkRect::MakeXYWH(50, 69, 38, 8), stroke_paint(kYellow, 1.f));
        text_outline(c, "RWY AHEAD", 69.f, 75.5f, kLarge, kYellow, A::Middle);
    }
    warn(pullUp, "PULL UP", 69.f, 100.f, kLargest, kRed);
    warn(sink && !dontSink && !glide && !terrain && !tlFlaps && !tlGear && !tlTerrain && !pullUp, "SINK RATE", 69.f, 100.f, kLargest, kAmber);
    warn(dontSink && !glide && !terrain && !tlFlaps && !tlGear && !tlTerrain && !pullUp, "DONT SINK", 69.f, 100.f, kLargest, kAmber);
    warn(tlGear && !tlTerrain && !pullUp, "TOO LOW GEAR", 69.f, 100.f, kLargest, kAmber);
    warn(tlTerrain && !pullUp, "TOO LOW TERRAIN", 69.f, 100.f, kLargest, kAmber);
    warn(tlFlaps && !tlGear && !tlTerrain && !pullUp, "TOO LOW FLAPS", 69.f, 100.f, kLargest, kAmber);
    warn(glide && !terrain && !tlFlaps && !tlGear && !tlTerrain && !pullUp, "GLIDE SLOPE", 69.f, 100.f, kLargest, kAmber);
    warn(terrain && !tlFlaps && !tlGear && !tlTerrain && !pullUp, "TERRAIN", 69.f, 100.f, kLargest, kAmber);
}

bool speed_failed(const State& s) { return s.speed.fw() || (s.speed.ncd() && !s.onGround); }
double speed_value(const State& s) { return s.speed.ncd() ? 30 : s.speed.value; }

double vmax_fallback(Ctx& x) {
    double fb = 340.0;
    Word sp = arinc(x.in.var("L:A32NX_ADIRS_ADR_1_CORRECTED_AVERAGE_STATIC_PRESSURE"));
    if (sp.normal()) {
        double mach = 0.89;
        double cas = 1479.1 * std::sqrt(std::pow((sp.value / 1013.25) * (std::pow(0.2 * mach * mach + 1, 3.5) - 1) + 1, 1 / 3.5) - 1);
        fb = std::min(fb, cas);
    }
    return fb;
}

void draw_speed_bars(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    Mem& m = *s.mem;
    double air = speed_value(s);
    auto bad = [](const Word& w) { return w.fw() || w.ncd(); };
    Word vmax = x.w("L:A32NX_PRIM_%d_V_MAX", x.prim);
    Word vap = x.w("L:A32NX_PRIM_%d_V_ALPHA_PROT", x.prim);
    Word vsw = x.w("L:A32NX_PRIM_%d_V_STALL_WARN", x.prim);
    Word vls = x.w("L:A32NX_PRIM_%d_V_LS", x.prim);
    Word val = x.w("L:A32NX_PRIM_%d_V_ALPHA_LIM", x.prim);
    Word trend = x.w("L:A32NX_PRIM_%d_SPEED_TREND", x.prim);
    c.save();
    c.clipRect(SkRect::MakeLTRB(0, 38.086f, 30, 123.56f));
    {
        auto bugY = [&](double v) { return (float)((air - v) * 1.0); };
        const char* circle = "m20.29 80.85a1.2592 1.2599 0 1 0-2.5184 0 1.2592 1.2599 0 1 0 2.5184 0z";
        Word vman = x.w("L:A32NX_PRIM_%d_V_MAN", x.prim);
        Word v3 = x.w("L:A32NX_PRIM_%d_V_3", x.prim);
        Word v4 = x.w("L:A32NX_PRIM_%d_V_4", x.prim);
        int phase = (int)x.v("L:A32NX_FWC_FLIGHT_PHASE");
        double v1 = x.v("L:AIRLINER_V1_SPEED"), vr = x.v("L:AIRLINER_VR_SPEED");
        if (vman.normal()) {
            c.save();
            c.translate(0, bugY(vman.value));
            a380::draw_path_cached(c, svg(circle), stroke_paint(kBlack, 0.29f * kMm));
            a380::chromium_stroke_path(c, svg(circle), stroke_paint(kGreen, kThick));
            c.restore();
        }
        for (auto pr : {std::pair<const Word*, const char*>{&v3, "F"}, {&v4, "S"}}) {
            if (!pr.first->normal()) continue;
            c.save();
            c.translate(0, bugY(pr.first->value));
            stroke(c, "m19.031 80.82h3.8279", kGreen);
            text(c, pr.second, 27.536509f, 83.327988f, kLarge, kGreen, A::Middle);
            c.restore();
        }
        if (phase <= 5 && v1 > 0) {
            c.save();
            c.translate(0, bugY(v1));
            stroke(c, "m16.613 80.82h5.4899", kCyan);
            text(c, "1", 26.205544f, 82.96f, kLarge, kCyan, A::Middle);
            c.restore();
        }
        if (phase <= 5 && vr > 0) {
            c.save();
            c.translate(0, bugY(vr));
            stroke(c, "m21.549 80.82a1.2592 1.2599 0 1 0-2.5184 0 1.2592 1.2599 0 1 0 2.5184 0z", kCyan);
            c.restore();
        }
        if (phase <= 5 && v1 > 0 && !s.speed.fw() && v1 - ((s.speed.normal() || s.speed.ft()) ? s.speed.value : 30.0) > 42)
            text(c, std::to_string((long long)v1), 21.271021f, 43.23f, kTiny, kCyan);
        Word vfeNext = x.w("L:A32NX_PRIM_%d_V_FE_NEXT", x.prim);
        if (vfeNext.normal()) {
            c.save();
            c.translate(0, bugY(vfeNext.value));
            stroke(c, "m19.031 81.34h-2.8709m0-1.0079h2.8709", kAmber);
            c.restore();
        }
        if (vmax.value > 240 && vmax.normal() && normal_law(x)) {
            c.save();
            c.translate(0, bugY(vmax.value + 10));
            outlined(c, "m13.994 81.289h3.022m-3.022-1.0079h3.022", kGreen);
            c.restore();
        }
        Word tgt = x.w("L:A32NX_PRIM_%d_PFD_SELECTED_SPEED", x.prim);
        char nb[64];
        std::snprintf(nb, sizeof nb, "L:A32NX_PRIM_%d_FG_DISCRETE_WORD_5", x.prim);
        double r5 = x.v(nb);
        Word w5 = arinc(r5);
        bool managed = (w5.normal() || w5.ft()) && bit(r5, 17);
        if ((tgt.normal() || tgt.ft()) && std::fabs(air - tgt.value) < 42) {
            c.save();
            c.translate(0, (float)((std::round(air * 100) / 100 - tgt.value) * 1.0));
            a380::draw_path_cached(c, svg("m19.274 81.895 5.3577 1.9512v -6.0476l -5.3577 1.9512z"),
                       stroke_paint(managed ? kMagenta : kCyan, kNormal, true));
            c.restore();
        }
        Word st = x.w("L:A32NX_PRIM_%d_PFD_SHORT_TERM_MANAGED_SPEED", x.prim);
        bool stValid = (st.normal() || st.ft()) && (tgt.normal() || tgt.ft());
        bool selActive = (w5.normal() || w5.ft()) && bit(r5, 18);
        if (s.speed.normal() && stValid && (selActive || std::fabs(st.value - tgt.value) > 2)) {
            double diff = std::fabs(s.speed.value - st.value);
            const char* path;
            float dy;
            if (diff < 42) {
                path = "m20.29 80.85a1.2592 1.2599 0 1 0-2.5184 0 1.2592 1.2599 0 1 0 2.5184 0z";
                dy = bugY(st.value);
            } else if (s.speed.value > st.value) {
                path = "m 17.91,80.60c 4.07e-4,0.6238 0.5384,1.1293 1.2019,1.1293 0.6635,0 1.2015,-0.5055 1.2019,-1.1293h -1.2019z";
                dy = 42.f;
            } else {
                path = "m 17.91,80.60c 4.07e-4,0.6743 0.5612,1.2207 1.2530,1.2207 0.6917,0 1.2525,-0.5464 1.2530,-1.2207h -1.2530z";
                dy = -42.f;
            }
            c.save();
            c.translate(0, dy);
            fill(c, path, kMagenta);
            c.restore();
        }
        if (x.v("L:A32NX_PFD_SHOW_SPEED_MARGINS") > 0 && s.speed.normal()) {
            for (const char* nm : {"L:A32NX_PFD_UPPER_SPEED_MARGIN", "L:A32NX_PFD_LOWER_SPEED_MARGIN"}) {
                double mg = x.v(nm);
                if (std::fabs(s.speed.value - mg) < 42) {
                    c.save();
                    c.translate(0, (float)(std::round(100 * (s.speed.value - mg)) / 100));
                    fill(c, "m19.7 80.5 h 5.3577 v 0.7 h-5.3577 z", kMagenta);
                    c.restore();
                }
            }
        }
    }
    {
        double vm = vmax.normal() ? vmax.value : vmax_fallback(x);
        if (air - vm >= -42) {
            float off = (float)(std::min(air - vm, 42.0) * 1.0);
            c.save();
            c.translate(0, off);
            a380::draw_path_cached(c, svg(kOverspeedPath), [] { SkPaint p = fill_paint(kRed); return p; }());
            a380::draw_path_cached(c, svg(kOverspeedPath), stroke_paint(kRed, 0.11f));
            c.restore();
        }
    }
    if (!(air - vap.value > 42 || bad(vap))) {
        float off = (float)(std::max(air - vap.value, -42.0) * 1.0);
        c.save();
        c.translate(0, off);
        a380::draw_path_cached(c, svg(kAlphaProtPath), fill_paint(kAmber));
        a380::draw_path_cached(c, svg(kAlphaProtPath), stroke_paint(kAmber, 0.13f));
        c.restore();
    }
    if (!(air - vsw.value > 42 || bad(vsw))) {
        float off = (float)(std::max(air - vsw.value, -42.0) * 1.0);
        c.save();
        c.translate(0, off);
        a380::draw_path_cached(c, svg(kStallWarnPath), fill_paint(kRed));
        a380::draw_path_cached(c, svg(kStallWarnPath), stroke_paint(kRed, 0.11f));
        c.restore();
    }
    if (s.showVls && vls.normal()) {
        bool apv = !bad(vap);
        float pos = (float)((air - vls.value) * 1.0 + 80.818);
        float off = (float)((vls.value - (apv ? vap.or0() : vsw.or0())) * 1.0);
        char d[96];
        std::snprintf(d, sizeof d, "m19.031 %.3f h 1.9748 v%.3f", pos, off);
        stroke(c, d, kAmber);
    }
    if (!(val.value - air < -42 || bad(val))) {
        float off = (float)((air - 42 - val.value) * 1.0);
        char d[96];
        std::snprintf(d, sizeof d, "m19.031 123.56h3.425v%.3fh-3.425z", off);
        fill(c, d, kRed);
    }
    {
        double sraw = x.v("L:A32NX_SFCC_1_SLAT_FLAP_SYSTEM_STATUS_WORD");
        double praw = x.v("L:A32NX_SFCC_1_SLAT_FLAP_ACTUAL_POSITION_WORD");
        bool lever1 = bit_or(sraw, 18) && !bit(sraw, 26);
        bool cfg1f = bit_or(praw, 11) && bit(praw, 13) && !bit(praw, 14) && !bit(praw, 15) && bit(praw, 18) && bit(praw, 20) &&
                     !bit(praw, 21) && !bit(praw, 22) && !bit(praw, 29) && !bit(praw, 25);
        if ((s.speed.normal() || s.speed.ft()) && lever1 && cfg1f && !vmax.fw()) {
            double a = s.speed.value;
            float arsPos = (float)((a - 212) * 1.0 + 80.818);
            float off = (float)(vmax.value < a + 42 ? (212 - vmax.value) : (212 - (a + 42)));
            char d[96];
            std::snprintf(d, sizeof d, "m19.031 %.3fh 1.9748v%.3f", arsPos, off);
            stroke(c, d, kGreen);
        }
    }
    {
        if (s.now - m.trendSampMs >= 100.0 || s.now < m.trendSampMs) {
            m.trendSampMs = s.now;
            m.trendSamp = trend.value;
            if (std::fabs(trend.value) < 1) m.trendShow = false;
            else if (std::fabs(trend.value) > 2) m.trendShow = true;
        }
        double lim = m.trend.step(m.trendSamp, s.dt, 12.0, -12.0);
        if (!bad(trend) && m.trendShow) {
            float off = (float)(-lim * 1.0);
            char d[96];
            std::snprintf(d, sizeof d, "m15.455 80.823v%.3f", off);
            stroke(c, d, kYellow);
            float ty = 80.823f + off;
            if (lim > 0)
                std::snprintf(d, sizeof d, "m15.455 %.3f l -1.2531 2.4607 M15.455 %.3f l 1.2531 2.4607", ty, ty);
            else
                std::snprintf(d, sizeof d, "m15.455 %.3f l 1.2531 -2.4607 M15.455 %.3f l -1.2531 -2.4607", ty, ty);
            stroke(c, d, kYellow);
        }
    }
    c.restore();
}

void draw_speed(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    fill(c, "m1.9058 123.56v-85.473h17.125v85.473z", kGrey);
    if (speed_failed(s)) {
        if (blink9(s, kBlSpd, true)) text(c, "SPD", 17.756115f, 83.386398f, kLargest, kRed, A::End);
        return;
    }
    blink9(s, kBlSpd, false);
    double cl = std::clamp(speed_value(s), 30.0, 660.0);
    int lo = (int)std::round((cl - 48) / 10) * 10;
    lo = std::max(lo, 30);
    if (lo < cl - 48) lo += 10;
    for (int v = lo; v <= 660 && v <= cl + 48; v += 10) {
        float y = 80.818f - (float)(v - cl);
        char d[64];
        std::snprintf(d, sizeof d, "m19.031 %.3fh-2.8206", y);
        stroke(c, d, kWhite);
        if (v % 20 == 0) {
            char t[8];
            std::snprintf(t, sizeof t, "%03d", v);
            text(c, t, 9.5348943f, y + 2.118f, kInterm, kWhite, A::Middle);
        }
    }
    draw_speed_bars(x, s);
}

void draw_speed_offtape(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    if (speed_failed(s)) {
        blink9(s, kBlSpdLim, false);
        stroke(c, "m1.9058 38.086h21.859", kRed);
        stroke(c, "m1.9058 123.56h21.859", kRed);
        return;
    }
    stroke(c, "m1.9058 38.086h21.859", kWhite);
    if (speed_value(s) > 72) stroke(c, "m1.9058 123.56h21.859", kWhite);
    if (x.v("L:A32NX_AUTOBRAKES_DECEL_LIGHT") > 0) text(c, "DECEL", 20.53927f, 129.06996f, kMedium, kGreen, A::End);
    const char* arrow = "m13.994 80.46v0.7257h6.5478l3.1228 1.1491v-3.0238l-3.1228 1.1491z";
    a380::draw_path_cached(c, svg(arrow), stroke_paint(kBlack, kSmallOut));
    fill(c, arrow, kYellow);
    fill(c, "m0.092604 81.185v-0.7257h2.0147v0.7257z", kYellow);
    if (blink9(s, kBlSpdLim, x.w("L:A32NX_PRIM_%d_V_MAX", x.prim).fw())) {
        text(c, "SPD", 32.077583f, 116.57941f, kMedium, kRed, A::End);
        text(c, "LIM", 32.107349f, 122.14585f, kMedium, kRed, A::End);
    }
}

void draw_alt(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    fill(c, "m130.85 123.56h-13.096v-85.473h13.096z", kGrey);
    {
        Word le = arinc(x.v("L:A32NX_FM1_LANDING_ELEVATION"));
        int lphase = (int)x.v("L:A32NX_FWC_FLIGHT_PHASE");
        double ldelta = s.alt.value - le.value;
        if (!(ldelta > 600 || (lphase != 9 && lphase != 10) || le.fw() || le.ncd())) {
            char d[64];
            std::snprintf(d, sizeof d, "m130.85 123.56h-13.096v%.3fh13.096z", (ldelta - 600) * 7.5 / 100);
            fill(c, d, kEarth);
        }
    }
    if (!(s.alt.normal() || s.alt.ft())) return;
    double cl = std::clamp(s.alt.value, -1500.0, 50000.0);
    int lo = (int)(std::round((cl - 600) / 100) * 100);
    if (lo < cl - 600) lo += 100;
    lo = std::max(lo, -1500);
    for (int v = lo; v <= 50000 && v <= cl + 600; v += 100) {
        float y = 80.819f - (float)((v - cl) * 0.075);
        char d[64];
        std::snprintf(d, sizeof d, "m130.85 %.3fh-2.0147", y);
        stroke(c, d, kWhite);
        if (v % 500 == 0) {
            char t[8];
            std::snprintf(t, sizeof t, "%03d", std::abs(v) / 100);
            text(c, t, 123.28826f, y + 1.821f, kMedium, kWhite, A::Middle);
            char dd[64];
            std::snprintf(dd, sizeof dd, "m115.79 %.3f 1.3316-1.0783-1.3316-1.0783", y + 1.0783f);
            stroke(c, dd, kWhite);
        }
    }
}

void draw_alt_offtape(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    if (!(s.alt.normal() || s.alt.ft())) {
        stroke(c, "m 117.75,38.09 h 13.10 6.73", kRed);
        stroke(c, "m 117.75,123.56 h 13.10 6.73", kRed);
        fill(c, "m131.35 85.308h-13.63v-8.9706h13.63z", kBlack);
        if (blink9(s, kBlAlt, true)) text(c, "ALT", 131.16769f, 83.433167f, kLargest, kRed, A::End);
        return;
    }
    blink9(s, kBlAlt, false);
    stroke(c, "m 117.75,38.09 h 13.10 6.73", kWhite);
    stroke(c, "m 117.75,123.56 h 13.10 6.73", kWhite);
    {
        int fm = fm_index(x);
        Word mdaW = arinc(x.v(fm == 1 ? "L:A32NX_FM1_MINIMUM_DESCENT_ALTITUDE" : "L:A32NX_FM2_MINIMUM_DESCENT_ALTITUDE"));
        int mphase = (int)x.v("L:A32NX_FWC_FLIGHT_PHASE");
        bool inLanding = mphase == 9 || mphase == 10;
        const char* mw2 = x.fo ? "L:A32NX_FCU_EFIS_R_DISCRETE_WORD_2" : "L:A32NX_FCU_EFIS_L_DISCRETE_WORD_2";
        double mw2raw = x.v(mw2);
        bool isQnh = bit_or(mw2raw, 12);
        bool isQfe = !isQnh && !bit_or(mw2raw, 11, true);
        Word le = arinc(x.v("L:A32NX_FM1_LANDING_ELEVATION"));
        bool raValid = !s.ra.fw() && !s.ra.ncd();
        bool qnhOk = !le.fw() && !le.ncd() && inLanding && isQnh;
        bool qfeOk = inLanding && isQfe;
        double altDelta = mdaW.value - s.alt.value;
        if ((raValid || qnhOk || qfeOk) && std::fabs(altDelta) <= 570 && !mdaW.fw() && !mdaW.ncd()) {
            char d[96];
            std::snprintf(d, sizeof d, "m 127.9276,%.4f h 5.80948 v 1.124908 h -5.80948 z", 80.249604 - altDelta * 7.5 / 100);
            fill(c, d, kAmber);
        }
    }
    double alt = s.alt.value;
    double a = std::fabs(std::clamp(alt, -1500.0, 50000.0));
    int tens = (int)std::fmod(a, 100.0);
    int hund = (int)std::floor(std::fmod(a / 100, 10));
    int thou = (int)std::floor(std::fmod(a / 1000, 10));
    int tenth = (int)std::floor(std::fmod(a / 10000, 10));
    double mda = x.v("L:AIRLINER_MINIMUM_DESCENT_ALTITUDE");
    SkColor col = (mda != 0 && alt < mda) ? kAmber : kGreen;
    fill(c, "m117.75 76.337h13.096v-2.671h8.8647v14.313h-8.8647v-2.671h-13.096z", kBlack);
    if (!(s.ra.fw() || s.ra.ncd()) && s.raF <= 600) {
        char d[64];
        std::snprintf(d, sizeof d, "m131.15 123.56h2.8709v%.3fh-2.8709z", (s.raF - 600) * 7.5 / 100);
        fill(c, d, kRed);
    }
    float hpos = tens > 80 ? (float)(tens / 20.0 - 4) : 0.f;
    float tpos = hund >= 9 ? hpos : 0.f;
    float ttpos = thou >= 9 ? tpos : 0.f;
    c.save();
    c.clipRect(SkRect::MakeXYWH(117.754f, 76.3374f, 14.5f, 8.9706f));
    c.translate(117.754f, 76.3374f);
    auto drum = [&](float tx, float pos, int cur, bool showZero) {
        for (int k = -1; k <= 1; k++) {
            int val = cur + k;
            if (val < 0) val += 10;
            if (val >= 10) val -= 10;
            if (!showZero && val == 0 && cur == 0) continue;
            char b[4];
            std::snprintf(b, sizeof b, "%d", val);
            text(c, b, tx, 6.8f + (pos - (float)k) * 7.f, kMedium, col, A::Middle);
        }
    };
    if (tenth != 0 || ttpos != 0) drum(6.05f, ttpos, tenth, false);
    drum(9.7f, tpos, thou, tenth != 0);
    drum(13.2f, hpos, hund, true);
    c.restore();
    c.save();
    c.clipRect(SkRect::MakeXYWH(130.85f, 73.6664f, 8.8647f, 14.313f));
    c.translate(130.85f, 73.6664f);
    for (int k = -2; k <= 3; k++) {
        int val = (int)(std::floor(tens / 20.0) * 20) + k * 20;
        float y = 8.9133f + (float)(tens - val) * (4.7f / 20.f);
        int tv = val < 0 ? val + 100 : val >= 100 ? val - 100 : val;
        char tb[8];
        std::snprintf(tb, sizeof tb, "%02d", tv);
        text(c, tb, 5.5f, y, kSmallest, col, A::Middle);
    }
    c.restore();
    stroke(c, "m117.75 76.337h13.096v-2.671h8.8647v14.313h-8.8647v-2.671h-13.096", kYellow);
    if (alt < 0) {
        text(c, "N", 121.51714f, 77.956947f, kLarge, kWhite, A::End);
        text(c, "E", 121.7f, 83.251389f, kLarge, kWhite, A::End);
        text(c, "G", 121.63675f, 88.486031f, kLarge, kWhite, A::End);
    }
}

void draw_selected(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    Word selW = x.w("L:A32NX_PRIM_%d_SELECTED_ALTITUDE", x.prim);
    Word shown = selW;
    SkColor tcol = kCyan;
    {
        char nb[64];
        std::snprintf(nb, sizeof nb, "L:A32NX_PRIM_%d_FG_DISCRETE_WORD_1", x.prim);
        double r1 = x.v(nb);
        std::snprintf(nb, sizeof nb, "L:A32NX_PRIM_%d_FG_DISCRETE_WORD_3", x.prim);
        double r3 = x.v(nb);
        bool ign = bit(r1, 23) || bit(r3, 21) || bit(r3, 22) || bit(r3, 23);
        Word cw = x.w("L:A32NX_PRIM_%d_FM_ALTITUDE_CONSTRAINT", x.prim);
        bool useC = !(ign || cw.fw() || cw.ncd());
        if (ign) tcol = 0xFFFFFFFF;
        else if (useC) {
            tcol = 0xFFFF94FF;
            shown = cw;
        }
    }
    bool selValid = shown.normal() || shown.ft();
    bool altOk = s.alt.normal() || s.alt.ft();
    if (flash9(s, kFlAltSel, altOk && (shown.ncd() || shown.fw()))) text(c, "ALT SEL", 136.22987f, 37.250134f, 5.f, kRed, A::End);
    {
        char mb[64];
        std::snprintf(mb, sizeof mb, "L:A32NX_PRIM_%d_FG_DISCRETE_WORD_5", x.prim);
        if (altOk && bit_or(x.v(mb), 14)) {
            double mdaV = x.v("L:AIRLINER_MINIMUM_DESCENT_ALTITUDE");
            stroke(c, "m116.56 140.22h29.213v7.0556h-29.213z", kYellow);
            text(c, "M", 142.03537f, 145.8689f, kMedium, kCyan, A::Middle);
            text(c, std::to_string((long long)(js_round(s.alt.value * 0.3048 / 10) * 10)), 128.64708f, 145.86191f, kMedium,
                 s.alt.value < mdaV ? kAmber : kGreen, A::Middle);
            text(c, std::to_string((long long)(js_round(shown.value * 0.3048 / 10) * 10)), 94.088852f, 37.926617f, kSmallest, tcol, A::Middle);
            text(c, "M", 105.25774f, 37.872921f, kSmallest, kCyan, A::Middle);
        }
    }
    if (selValid && (s.alt.normal() || s.alt.ft())) {
        const char* w2n = x.fo ? "L:A32NX_FCU_EFIS_R_DISCRETE_WORD_2" : "L:A32NX_FCU_EFIS_L_DISCRETE_WORD_2";
        double w2raw = x.v(w2n);
        Word w2 = arinc(w2raw);
        bool isStd = !(w2.normal() || w2.ft()) || bit(w2raw, 11);
        double tgt = shown.value, d = s.alt.value - tgt;
        char t[16];
        if (isStd) std::snprintf(t, sizeof t, "%03d", (int)std::lround(tgt / 100));
        else std::snprintf(t, sizeof t, "%5d", (int)std::lround(tgt));
        if (d > 600) {
            text(c, t, 137.7511f, 128.70299f, kMedium, tcol, A::End);
            if (isStd) text(c, "FL", 120.87094f, 128.71681f, 5.f, tcol, A::Middle);
        } else if (d < -600) {
            text(c, t, 138.22987f, 37.250134f, kMedium, tcol, A::End);
            if (isStd) text(c, "FL", 120.85925f, 37.125755f, 5.f, tcol, A::Middle);
        } else {
            c.save();
            c.translate(0, (float)(d * 7.5 / 100));
            char bf[64];
            std::snprintf(bf, sizeof bf, "m117.75 77.784h%.2fv6.0476h-%.2fz", isStd ? 12.5 : 19.14, isStd ? 12.5 : 19.14);
            fill(c, bf, kBlack);
            stroke(c, "m122.79 83.831v6.5516h-7.0514v-8.5675l2.0147-1.0079m4.8441-3.0238v-6.5516h-6.8588v8.5675l2.0147 1.0079", tcol, kNormal, true);
            text(c, t, 118.228f, 83.067062f, kMedium, tcol, A::Start);
            c.restore();
        }
    }
    Word tgt = x.w("L:A32NX_PRIM_%d_PFD_SELECTED_SPEED", x.prim);
    if (flash9(s, kFlSpdSel, !speed_failed(s) && tgt.fw())) text(c, "SPD SEL", 24.078989f, 36.670692f, 5.f, kRed, A::End);
    if ((tgt.normal() || tgt.ft()) && !speed_failed(s)) {
        char nb[64];
        std::snprintf(nb, sizeof nb, "L:A32NX_PRIM_%d_FG_DISCRETE_WORD_5", x.prim);
        double r5 = x.v(nb);
        Word w5 = arinc(r5);
        bool managed = (w5.normal() || w5.ft()) && bit(r5, 17);
        SkColor col = managed ? 0xFFFF94FF : kCyan;
        double air = speed_value(s);
        char t[16];
        std::snprintf(t, sizeof t, "%03d", (int)std::lround(tgt.value));
        if (air - tgt.value > 42) text(c, t, 24.078989f, 128.27917f, kSmallest, col, A::End);
        else if (air - tgt.value < -42 && !(x.v("L:A32NX_AUTOBRAKES_DECEL_LIGHT") > 0)) text(c, t, 24.113895f, 36.670692f, kSmallest, col, A::End);
    }
}

void draw_radio_alt(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    Mem& m = *s.mem;
    float vo = roll_vertical_offset(s.roll.or0());
    if (!s.attExcessive) {
        double rv = s.raF;
        double dh = x.v("L:AIRLINER_DECISION_HEIGHT");
        bool dhValid = dh >= 0;
        m.dhShown = !s.ra.fw() && dhValid && rv <= dh;
        int phase = (int)x.v("L:A32NX_FMGC_FLIGHT_PHASE");
        int fm = fm_index(x);
        bool useAlt = phase <= 3;
        Word tw = arinc(x.v(useAlt ? (fm == 1 ? "L:A32NX_FM1_TRANS_ALT" : "L:A32NX_FM2_TRANS_ALT")
                                   : (fm == 1 ? "L:A32NX_FM1_TRANS_LVL" : "L:A32NX_FM2_TRANS_LVL")));
        bool below = tw.normal() && !s.alt.ncd() && s.alt.value < (useAlt ? tw.value : tw.value * 100);
        std::string t;
        float size = kLarge;
        SkColor col = kAmber;
        if (!s.ra.ncd()) {
            if (!s.ra.fw()) {
                if (rv < 2500) {
                    if (rv > 400 || (rv > dh + 100 && dhValid)) col = kGreen;
                    if (rv < 400) size = kLargest;
                    if (rv < 5) t = std::to_string((long long)js_round(rv));
                    else if (rv <= 50) t = std::to_string((long long)(js_round(rv / 5) * 5));
                    else t = std::to_string((long long)(js_round(rv / 10) * 10));
                }
            } else {
                col = kRed;
                t = "RA";
            }
        }
        bool flagBlink = !s.ra.ncd() && s.ra.fw() && below;
        bool vis = blink9(s, kBlRa, flagBlink) || !flagBlink;
        c.save();
        c.translate(0, -vo);
        if (vis) text_outline(c, t, 69.202454f, 121.5f, size, col, A::Middle);
        c.restore();
    }
    if (m.dhShown) {
        c.save();
        c.translate(0, -vo);
        text_outline(c, "DH", 73.511879f, 115.f, kLargest, kAmber, A::End);
        c.restore();
    }
}

void draw_baro(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    const char* w2n = x.fo ? "L:A32NX_FCU_EFIS_R_DISCRETE_WORD_2" : "L:A32NX_FCU_EFIS_L_DISCRETE_WORD_2";
    if (!(s.alt.normal() || s.alt.ft())) {
        blink_inf(s, kBlBaro, false);
        return;
    }
    double w2raw = x.v(w2n);
    bool isQnh = bit_or(w2raw, 12);
    bool isStd = bit_or(w2raw, 11, true);
    bool isQfe = !isQnh && !isStd;
    bool hg = bit_or(w2raw, 13);
    int phase = (int)x.v("L:A32NX_FMGC_FLIGHT_PHASE");
    int fm = fm_index(x);
    Word ta = arinc(x.v(fm == 1 ? "L:A32NX_FM1_TRANS_ALT" : "L:A32NX_FM2_TRANS_ALT"));
    Word tl = arinc(x.v(fm == 1 ? "L:A32NX_FM1_TRANS_LVL" : "L:A32NX_FM2_TRANS_LVL"));
    bool stdMode = !isQfe && !isQnh;
    bool blinkOn = stdMode ? (phase > 3 && tl.normal() && 100 * tl.value > s.alt.value)
                           : (phase <= 3 && ta.normal() && ta.value < s.alt.value);
    if (!(blink_inf(s, kBlBaro, blinkOn) || !blinkOn)) return;
    if (stdMode) {
        stroke(c, "m124.79 131.74h13.096v7.0556h-13.096z", kYellow);
        text(c, "STD", 125.75785f, 137.36f, kMedium, kCyan);
        return;
    }
    if (isQfe) stroke(c, "m 116.83686,133.0668 h 13.93811 v 5.8933 h -13.93811 z", kWhite);
    text(c, isQfe ? "QFE" : "QNH", 118.23066f, 138.11342f, kMedium, kWhite);
    Word bw = arinc(x.v(hg ? (x.fo ? "L:A32NX_FCU_EFIS_R_BARO" : "L:A32NX_FCU_EFIS_L_BARO")
                           : (x.fo ? "L:A32NX_FCU_EFIS_R_BARO_HPA" : "L:A32NX_FCU_EFIS_L_BARO_HPA")));
    char t[16] = "";
    if (bw.normal() || bw.ft()) std::snprintf(t, sizeof t, hg ? "%.2f" : "%.0f", bw.value);
    text(c, t, 131.f, 138.09006f, kMedium, kCyan);
}

void draw_heading(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    fill(c, "m32.138 145.34h73.536v10.382h-73.536z", kGrey);
    if (!s.heading.normal()) {
        stroke(c, "m32.138 156.23v-10.886h73.536v10.886", kRed);
        text(c, "HDG", 75.926208f, 151.95506f, kLargest, kRed, A::End);
        return;
    }
    c.save();
    c.clipRect(SkRect::MakeLTRB(32.138f, 145.34f, 105.674f, 156.23f));
    double h = s.heading.value;
    double base = std::floor(h / 5) * 5;
    for (int k = -9; k <= 10; k++) {
        double hv = base + k * 5;
        float xx = 68.913f + (float)((hv - h) * 1.511);
        bool major = std::fmod(std::fmod(hv, 10) + 10, 10) == 0;
        char d[64];
        std::snprintf(d, sizeof d, "m%.3f 145.34v%.3f", xx, major ? 4.0 : 1.68);
        stroke(c, d, kWhite);
        if (major) {
            int hh = (int)(std::fmod(std::fmod(hv, 360) + 360, 360) / 10);
            char t[8];
            std::snprintf(t, sizeof t, "%d", hh);
            text(c, t, xx + 0.066f, 154.64206f, (((int)hv % 360 + 360) % 360) % 30 == 0 ? kMedium : kSmallest, kWhite, A::Middle);
        }
    }
    c.restore();
}

void draw_heading_offtape(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    if (!s.heading.normal()) return;
    double h = s.heading.value;
    stroke(c, "m32.138 156.23v-10.886h73.536v10.886", kWhite);
    bool trkMode = true;
    {
        double raw;
        {
            char nb[64];
            std::snprintf(nb, sizeof nb, "L:A32NX_PRIM_%d_FG_DISCRETE_WORD_5", x.prim);
            raw = x.v(nb);
        }
        Word w = arinc(raw);
        trkMode = (w.normal() || w.ft()) ? bit(raw, 11) : true;
    }
    Word sel = x.w(trkMode ? "L:A32NX_PRIM_%d_SELECTED_TRACK" : "L:A32NX_PRIM_%d_SELECTED_HEADING", x.prim);
    if (sel.normal() || sel.ft()) {
        double d = std::fmod(sel.value - h + 540, 360) - 180;
        char t[8];
        std::snprintf(t, sizeof t, "%03d", (int)std::lround(sel.value));
        if (std::fabs(d) < 24) {
            c.save();
            c.translate((float)(d * 1.511), 0);
            {
                a380::CompositedLayer layer(c, layer_bounds("m69.978 145.1 1.9501-5.3609h-6.0441l1.9501 5.3609", kNormal));
                stroke(c, "m69.978 145.1 1.9501-5.3609h-6.0441l1.9501 5.3609", kCyan, kNormal, true);
            }
            c.restore();
        } else if (d < 0) {
            text(c, t, 36.418198f, 144.32108f, kSmallest, kCyan, A::Middle);
        } else {
            text(c, t, 101.70432f, 144.34792f, kSmallest, kCyan, A::Middle);
        }
    }
    {
        const char* w2q = x.fo ? "L:A32NX_FCU_EFIS_R_DISCRETE_WORD_2" : "L:A32NX_FCU_EFIS_L_DISCRETE_WORD_2";
        double course = x.v("L:A32NX_FM_LS_COURSE");
        if (bit_or(x.v(w2q), 14, true) && course >= 0) {
            double dq = std::fmod(course - h + 540, 360) - 180;
            char qt[8];
            std::snprintf(qt, sizeof qt, "%03d", (int)js_round(course));
            if (std::fabs(dq) > 24) {
                bool right = dq > 0;
                const char* qbox = right ? "m100.57 149.68h12.088v6.5516h-12.088z" : "m26.094 156.18v-6.5516h12.088v6.5516z";
                fill(c, qbox, kBlack);
                stroke(c, qbox, kWhite);
                text(c, qt, right ? 106.95047f : 32.406616f, 155.22305f, kMedium, kMagenta, A::Middle);
            } else {
                c.save();
                c.translate((float)(dq * 1.511), 0);
                a380::draw_path_cached(c, svg("m66.992 152.82h3.8279m-1.914-6.5471v9.4518"), stroke_paint(kBlack, 0.29f * kMm));
                stroke(c, "m66.992 152.82h3.8279m-1.914-6.5471v9.4518", kMagenta, kThick);
                c.restore();
            }
        }
    }
    fill(c, "m69.61 147.31h-1.5119v-8.0635h1.5119z", kYellow);
    if (s.track.normal()) {
        double d = std::fmod(s.track.value - h + 540, 360) - 180;
        c.save();
        c.translate((float)(d * 1.511), 0);
        const char* bug = "m68.906 145.75-1.2592 1.7639 1.2592 1.7639 1.2592-1.7639z";
        {
            a380::CompositedLayer layer(c, layer_bounds(bug, 0.29f * kMm));
            a380::draw_path_cached(c, svg(bug), stroke_paint(kBlack, 0.29f * kMm, true));
            stroke(c, bug, kGreen, kThick, true);
        }
        c.restore();
    }
}

void draw_vsi(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    fill(c, "m151.84 131.72 4.1301-15.623v-70.556l-4.1301-15.623h-5.5404v101.8z", kGrey);
    bool tcasRa = (int)x.v("L:A32NX_TCAS_STATE") == 2;
    if (tcasRa && x.v("L:A32NX_TCAS_RA_CORRECTIVE") > 0) {
        auto vsY = [](double v) {
            double a = std::fabs(v), sn = v > 0 ? 1.0 : v < 0 ? -1.0 : 0.0;
            if (a < 1000) return v / 1000 * -27.22;
            if (a < 2000) return (v - sn * 1000) / 1000 * -10.1 - sn * 27.22;
            if (a < 6000) return (v - sn * 2000) / 4000 * -10.1 - sn * 37.32;
            return sn * -47.37;
        };
        auto zone = [&](double lo, double hi, SkColor col) {
            if (lo == -1 || hi == -1) return;
            double y1 = lo >= 6000 ? 29.92 : lo <= -6000 ? 131.72 : 80.822 + vsY(lo);
            double y2 = hi >= 6000 ? 29.92 : hi <= -6000 ? 131.72 : 80.822 + vsY(hi);
            double y3 = (std::fabs(hi) > 1750 && std::fabs(hi) > std::fabs(lo)) ? y2 : 80.822;
            double y4 = (std::fabs(lo) > 1750 && std::fabs(lo) > std::fabs(hi)) ? y1 : 80.822;
            char zd[160];
            std::snprintf(zd, sizeof zd, "m151.84,%.4f L151.84,%.4f L157.3804,%.4f L157.3804,%.4f L151.84,%.4f z", y1, y2, y3, y4, y1);
            fill(c, zd, col);
        };
        c.drawRect(SkRect::MakeXYWH(151.84f, 29.92f, 5.5404f, 101.8f), fill_paint(kGrey));
        zone(x.v("L:A32NX_TCAS_VSPEED_RED:1"), x.v("L:A32NX_TCAS_VSPEED_RED:2"), kRed);
        zone(x.v("L:A32NX_TCAS_VSPEED_GREEN:1"), x.v("L:A32NX_TCAS_VSPEED_GREEN:2"), kGreen);
    }
    if (!s.vs.normal()) {
        text(c, "V", 153.13206f, 77.501472f, kLargest, kRed, A::End);
        text(c, "/", 153.13406f, 83.211388f, kLargest, kRed, A::End);
        text(c, "S", 152.99374f, 88.870819f, kLargest, kRed, A::End);
        return;
    }
    for (auto d : {"m149.92 54.339v-1.4615h1.9151v1.4615z", "m149.92 44.26v-1.4615h1.9151v1.4615z",
                   "m149.92 34.054v-1.2095h1.9151v1.2095z", "m151.84 107.31h-1.9151v1.4615h1.9151z",
                   "m151.84 117.39h-1.9151v1.4615h1.9151z", "m151.84 127.59h-1.9151v1.2095h1.9151z"})
        fill(c, d, kWhite);
    for (auto d : {"m149.92 67.216h1.7135", "m151.84 48.569h-1.9151", "m151.84 38.489h-1.9151", "m149.92 94.43h1.7135",
                   "m151.84 113.08h-1.9151", "m151.84 123.16h-1.9151"})
        stroke(c, d, kWhite, kSmall);
    struct T { float x, y; const char* t; };
    for (auto& t : {T{148.47067f, 109.72845f, "1"}, T{148.24495f, 119.8801f, "2"}, T{148.27068f, 129.90607f, "6"},
                    T{148.49667f, 55.316456f, "1"}, T{148.26495f, 45.356102f, "2"}, T{148.21367f, 35.195072f, "6"}})
        text(c, t.t, t.x, t.y, kSmallest, kWhite, A::Middle);
    fill(c, "m145.79 80.067h6.0476v1.5119h-6.0476z", kYellow);
    if (!s.vs.normal()) return;
    double v = s.vsF, av = std::fabs(v), sg = v < 0 ? -1 : 1;
    float off;
    if (av < 1000) off = (float)(v / 1000 * -27.22);
    else if (av < 2000) off = (float)((v - sg * 1000) / 1000 * -10.1 - sg * 27.22);
    else if (av < 6000) off = (float)((v - sg * 2000) / 4000 * -10.1 - sg * 37.32);
    else off = (float)(sg * -47.37);
    SkColor col = kGreen;
    bool raOk = !s.ra.ncd() && !s.ra.fw();
    double ra = s.raF, vr = s.vs.value;
    if (!tcasRa && (av >= 6000 || (vr <= -2000 && raOk && ra <= 2500 && ra >= 1000) || (vr <= -1200 && raOk && ra <= 1000))) col = kAmber;
    SkColor needleCol = tcasRa ? kWhite : col;
    const float dxFull = 12, dxBorder = 5, cx = 162.74f, cy = 80.822f;
    char d[96];
    std::snprintf(d, sizeof d, "m%.3f %.3f l %.3f %.3f", cx - dxBorder, cy + dxBorder / dxFull * off, dxBorder - dxFull,
                  (1 - dxBorder / dxFull) * off);
    c.save();
    c.clipRect(SkRect::MakeLTRB(145.79f, 30.f, 158.f, 131.7f));
    if (s.vsMoved) {
        a380::draw_path_cached(c, svg(d), stroke_paint(kBlack, 0.39f * kMm));
        a380::chromium_stroke_path(c, svg(d), stroke_paint(needleCol, kHuge));
    }
    c.restore();
    double rawAbs = std::fabs(s.vs.value);
    if (rawAbs >= 200) {
        int n = (int)std::lround(rawAbs / 100);
        char tb[8];
        std::snprintf(tb, sizeof tb, "%s%d", n < 10 ? "0" : "", n);
        float ty = off - (s.vs.value < 0 ? -1.f : 1.f) * 2.4f;
        c.save();
        c.translate(0, ty);
        fill(c, "m158.4 83.011h-7.0514v-4.3989h7.0514z", kBlack);
        text(c, tb, 155.14055f, 82.554756f, kSmallest, col, A::Middle);
        c.restore();
    }
}

void draw_mach(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    Mem& m = *s.mem;
    bool failed = !s.mach.normal() && !s.onGround;
    if (blink9(s, kBlMach, failed)) text(c, "MACH", 5.4257932f, 136.88908f, kLargest, kRed);
    if (failed) return;
    int pm = (int)js_round(s.mach.or0() * 1000);
    if (m.showMach && pm < 450) m.showMach = false;
    else if (!m.showMach && pm > 500) m.showMach = true;
    if (m.showMach) {
        char t[16];
        std::snprintf(t, sizeof t, ".%d", pm);
        text(c, t, 5.566751f, 137.03004f, kLargest, kGreen);
    }
}

float text_w(const std::string& str, float size) {
    SkFont f = a380::font("Ecam", size);
    return f.measureText(str.data(), str.size(), SkTextEncoding::kUTF8);
}

void draw_ls(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    Mem& m = *s.mem;
    const char* w2n = x.fo ? "L:A32NX_FCU_EFIS_R_DISCRETE_WORD_2" : "L:A32NX_FCU_EFIS_L_DISCRETE_WORD_2";
    bool ls = bit_or(x.v(w2n), 14, true);
    auto fgw = [&](int n) {
        char nb[64];
        std::snprintf(nb, sizeof nb, "L:A32NX_PRIM_%d_FG_DISCRETE_WORD_%d", x.prim, n);
        return x.v(nb);
    };
    if (blink9(s, kBlTcas, x.v("L:A32NX_TCAS_FAULT") > 0)) {
        text(c, "T", 141.5f, 96.f, kLargest, kAmber, A::End);
        text(c, "C", 141.5f, 104.f, kLargest, kAmber, A::End);
        text(c, "A", 141.5f, 112.f, kLargest, kAmber, A::End);
        text(c, "S", 141.5f, 120.f, kLargest, kAmber, A::End);
    }
    {
        double r1 = fgw(1), r2 = fgw(2), r4 = fgw(4);
        bool approachSel = bit_or(r1, 23) || bit_or(r2, 28) || bit_or(r2, 23) || bit_or(r4, 13) || bit_or(r4, 14);
        if (flash9(s, kFlLs, approachSel && !ls)) text(c, "LS", 104.33f, 124.8f, kLargest, kAmber);
    }
    bool hasLoc = x.v("L:A32NX_RADIO_RECEIVER_LOC_IS_VALID") > 0, hasGs = x.v("L:A32NX_RADIO_RECEIVER_GS_IS_VALID") > 0;
    double locDots = 0, gsDots = 0;
    if (hasLoc) locDots = m.loc.step(x.v("L:A32NX_RADIO_RECEIVER_LOC_DEVIATION"), s.dt, 1.5) / 0.8;
    else m.loc.reset();
    if (hasGs) gsDots = m.gs.step(x.v("L:A32NX_RADIO_RECEIVER_GS_DEVIATION"), s.dt, 1.5) / 0.4;
    else m.gs.reset();
    if (!ls) {
        if (x.v(x.fo ? "L:A32NX_FMGC_R_LDEV_REQUEST" : "L:A32NX_FMGC_L_LDEV_REQUEST") > 0) {
            text(c, "L/DEV", 30.888f, 122.639f, 5.f, kGreen, A::End);
            for (auto d : {"m38.686 129.51v2.0158", "m53.796 129.51v2.0158", "m84.017 129.51v2.0158", "m99.127 129.51v2.0158"})
                stroke(c, d, kWhite);
            double dots = std::round(x.v("L:A32NX_FG_CROSS_TRACK_ERROR") * 1000) / 1000 / 0.1;
            if (dots > 2) stroke(c, "m99.127 127.99h2.0147v5.0397h-2.0147", kGreen);
            else if (dots < -2) stroke(c, "m38.686 127.99h-2.0147v5.0397h2.0147", kGreen);
            else {
                c.save();
                c.translate((float)(dots * 30.238 / 2), 0);
                stroke(c, "m66.892 127.99v5.0397h4.0294v-5.0397h-2.0147z", kGreen);
                c.restore();
            }
            fill(c, "m68.098 134.5v-8.0635h1.5119v8.0635z", kYellow);
        }
        m.markerPrev = 0;
        return;
    }
    {
        std::string ident = x.in.str("NAV IDENT:3");
        double nf = x.v("NAV FREQUENCY:3");
        double fr = js_round(nf * 1000) / 1000;
        long long whole = (long long)std::floor(fr);
        int frac = (int)js_round((fr - (double)whole) * 1000);
        char fb[8];
        std::snprintf(fb, sizeof fb, "%03d", frac);
        std::string tail = fb;
        while (!tail.empty() && tail.back() == '0') tail.pop_back();
        std::string lead = whole == 0 ? "" : std::to_string(whole);
        std::string trail = tail.empty() ? "" : "." + (tail.size() < 2 ? tail + "0" : tail);
        text(c, ident, 1.184f, 143.11522f, kLarge, kMagenta);
        text(c, lead, 1.3610243f, 149.11575f, kLarge, kMagenta);
        text(c, trail, 12.964463f, 149.24084f, kLarge, kMagenta);
        if (x.v("NAV HAS DME:3") > 0) {
            double dist = js_round(x.v("NAV DME:3") * 10) / 10;
            std::string dl, dt_;
            if (dist < 20) {
                long long iw = (long long)std::floor(dist);
                int d1 = (int)js_round((dist - (double)iw) * 10);
                dl = std::to_string(iw);
                dt_ = "." + std::to_string(d1);
            } else {
                dl = std::to_string((long long)js_round(dist));
            }
            text(c, dl, 1.3685881f, 155.26602f, kLarge, kMagenta);
            text(c, dt_, 1.3685881f + text_w(dl, kLarge), 155.26602f, kSmallest, kMagenta);
            text(c, "NM", 17.159119f, 155.22606f, kSmallest, kCyan);
        }
    }
    SkPaint p = stroke_paint(kWhite, kNormal);
    for (float cy : {50.585f, 65.704f, 95.942f, 111.06f}) c.drawCircle(109.7f, cy, 1.0074f, p);
    for (float cx : {53.797f, 38.686f, 84.017f, 99.123f}) c.drawCircle(cx, 130.51f, 1.0074f, p);
    if (hasLoc) {
        if (locDots > 2) stroke(c, "m99.127 133.03 3.7776-2.5198-3.7776-2.5198", kMagenta);
        else if (locDots < -2) stroke(c, "m38.686 133.03-3.7776-2.5198 3.7776-2.5198", kMagenta);
        else {
            c.save();
            c.translate((float)(locDots * 30.221 / 2), 0);
            stroke(c, "m65.129 130.51 3.7776 2.5198 3.7776-2.5198-3.7776-2.5198z", kMagenta);
            c.restore();
        }
    }
    if (hasGs) {
        if (gsDots > 2) stroke(c, "m107.19 111.06 2.5184 3.7798 2.5184-3.7798", kMagenta);
        else if (gsDots < -2) stroke(c, "m107.19 50.585 2.5184-3.7798 2.5184 3.7798", kMagenta);
        else {
            c.save();
            c.translate(0, (float)(gsDots * 30.238 / 2));
            stroke(c, "m109.7 77.043-2.5184 3.7798 2.5184 3.7798 2.5184-3.7798z", kMagenta);
            c.restore();
        }
    }
    {
        int mk = (int)x.v("MARKER BEACON STATE");
        if (mk != m.markerPrev) {
            m.markerPrev = mk;
            m.markerAt = s.now;
        }
        if (mk != 0) {
            double t = s.now - m.markerAt;
            bool on = true;
            if (!s.offline) {
                if (mk == 1) on = std::fmod(t, 460.0) / 460.0 >= 0.34;
                else if (mk == 2) {
                    double ph = std::fmod(t, 730.0) / 730.0;
                    on = (ph >= 0.11 && ph < 0.28) || ph >= 0.45;
                } else on = std::fmod(t, 200.0) / 200.0 >= 0.51;
            }
            if (on) text(c, mk == 1 ? "OM" : mk == 2 ? "MM" : "IM", 107.f, 133.f, kLarge, mk == 1 ? kCyan : mk == 2 ? kAmber : kWhite);
        }
    }
    if (hasLoc) text(c, "ILS", 104.f, 126.f, kLargest, kMagenta, A::Middle);
    fill(c, "m68.098 134.5v-8.0635h1.5119v8.0635z", kYellow);
}

void draw_linear_dev(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    Mem& m = *s.mem;
    bool active = x.v("L:A32NX_PFD_LINEAR_DEVIATION_ACTIVE") > 0;
    if (!s.alt.normal() || !active) {
        m.ldState = 0;
    } else {
        double dev = s.alt.value - x.v("L:A32NX_PFD_TARGET_ALTITUDE");
        m.ldPx = (float)(std::clamp(dev, -500.0, 500.0) * 40.5 / 500);
        if (dev > 540) m.ldState = 1;
        else if (dev > -500 && dev < 500) m.ldState = 2;
        else if (dev < -540) m.ldState = 3;
        char t[8];
        std::snprintf(t, sizeof t, "%02d", (int)std::min(99.0, js_round(std::fabs(dev) / 100)));
        if (m.ldState == 3) text(c, t, 110.f, 42.5f, kSmallest, kGreen);
        if (m.ldState == 1) text(c, t, 110.f, 123.f, kSmallest, kGreen);
    }
    c.save();
    c.translate(0, m.ldPx);
    if (m.ldState == 2) fill(c, "m119.26 80.796a1.511 1.5119 0 1 0-3.022 0 1.511 1.5119 0 1 0 3.022 0z", kGreen);
    if (m.ldState == 3) fill(c, "m116.24 80.796c4.9e-4 -0.83466 0.67686-1.511 1.511-1.511 0.83418 0 1.5105 0.67635 1.511 1.511h-1.511z", kGreen);
    if (m.ldState == 1) fill(c, "m116.24 80.796c4.9e-4 0.83465 0.67686 1.511 1.511 1.511 0.83418 0 1.5105-0.67636 1.511-1.511h-1.511z", kGreen);
    if (x.v("L:A32NX_PFD_VERTICAL_PROFILE_LATCHED") > 0) stroke(c, "m 119 78.3 h -3 v 5 h 3", kMagenta);
    c.restore();
}

void draw_slats_flaps(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    char b[96];
    auto rd = [&](const char* nm) {
        std::snprintf(b, sizeof b, nm, 1);
        return x.v(b);
    };
    double stRaw = rd("L:A32NX_SFCC_%d_SLAT_FLAP_SYSTEM_STATUS_WORD");
    Word st = arinc(stRaw);
    Word sw = arinc(rd("L:A32NX_SFCC_%d_SLAT_ACTUAL_POSITION_WORD"));
    Word fw = arinc(rd("L:A32NX_SFCC_%d_FLAP_ACTUAL_POSITION_WORD"));
    auto sb = [&](int n) { return (st.normal() || st.ft()) && bit(stRaw, n); };
    bool clean = sb(17), c1 = sb(18), c2 = sb(19), c3 = sb(20), full = sb(21), f1auto = sb(26);
    bool slatsFault = bit(stRaw, 11), flapsFault = bit(stRaw, 12), frelief = bit(stRaw, 22), alock = bit(stRaw, 24);
    bool slatsValid = bit(stRaw, 28), flapsValid = bit(stRaw, 29);
    double slats = sw.normal() || sw.ft() ? sw.value : 0, flaps = fw.normal() || fw.ft() ? fw.value : 0;
    bool slatsOut = slats > 6.1, flapsOut = flaps > 5.0;
    std::string target;
    if (clean) target = "0";
    else if (c1 && f1auto) target = "1";
    else if (c1) target = "1+F";
    else if (c2) target = "2";
    else if (c3) target = "3";
    else if (full) target = "FULL";
    int slatsTgt = -1, flapsTgt = -1;
    if (clean && slats > 6.1) slatsTgt = 0;
    else if ((c1 || c2) && (slats < 276 || slats > 296)) slatsTgt = 1;
    else if ((c3 || full) && (slats < 317 || slats > 337)) slatsTgt = 2;
    if ((clean || (c1 && f1auto)) && flapsOut) flapsTgt = 0;
    else if (c1 && !f1auto && (flaps < 210 || flaps > 220)) flapsTgt = 1;
    else if (c2 && (flaps < 254 || flaps > 264)) flapsTgt = 2;
    else if (c3 && (flaps < 292 || flaps > 302)) flapsTgt = 3;
    else if (full && (flaps < 333 || flaps > 343)) flapsTgt = 4;
    bool moving = slatsTgt >= 0 || flapsTgt >= 0;
    bool anyOut = slatsOut || flapsOut || !clean;
    if (anyOut) {
        text(c, target, 23.7f, 202.3f, kMedium, moving ? kCyan : kGreen, A::Middle);
        if (moving) a380::chromium_stroke_path(c, svg("M 15.4 196.8 v 6.2 h 16.2 v -6.2 z"), stroke_paint(kCyan, kNormal, true));
    }
    auto circ = [&](float cx, float cy, bool cyan) {
        SkPaint p = stroke_paint(cyan ? kCyan : kWhite, kNormal);
        SkPaint f = fill_paint(kCyan);
        if (cyan) c.drawCircle(cx, cy, 0.8f, f);
        c.drawCircle(cx, cy, 0.8f, p);
    };
    if (anyOut && slatsValid) {
        if (slatsTgt == 0) circ(14.1f, 194.5f, true);
        circ(9.6f, 195.4f, slatsTgt == 1);
        circ(5.f, 196.4f, slatsTgt == 2);
        if (!alock) text(c, "S", 3.8f, 191.1f, 5.f, slatsFault ? kAmber : kWhite);
    }
    if (anyOut && flapsValid) {
        static const char* fp[5] = {"M 32.3 193.7 v 1.7 h 1.9 z", "M 39.9 196.8 v 1.7 h 1.9 z", "M 47.3 199.9 v 1.7 h 1.9 z",
                                    "M 54.7 203 v 1.7 h 1.9 z", "M 62.1 206.1 v 1.7 h 1.9 z"};
        for (int i = 0; i < 5; i++) {
            if (i == 0 && flapsTgt != 0) continue;
            bool cy = flapsTgt == i;
            SkPath pth = svg(fp[i]);
            a380::draw_path_cached(c, pth, fill_paint(cy ? kCyan : kWhite));
            a380::chromium_stroke_path(c, pth, stroke_paint(cy ? kCyan : kWhite, kNormal, true));
        }
        if (!frelief) text(c, "F", 47.2f, 210.8f, 5.f, flapsFault ? kAmber : kWhite);
    }
    double pulseMs = x.v("A380_ANIM:green-pulse", -1);
    bool pulseLow = std::fmod(pulseMs >= 0 ? pulseMs : s.now, 1000.0) >= 500.0;
    SkColor pulseCol = pulseLow ? SkColorSetRGB(0, 102, 0) : kGreen;
    if (alock) text(c, "A LOCK", 0.f, 190.f, kSmallest, pulseCol);
    if (frelief) text(c, "F RELIEF", 32.5f, 210.8f, kSmallest, pulseCol);
    if (!slatsValid) text(c, "XX", 9.f, 195.5f, kSmallest, kAmber);
    if (!flapsValid) text(c, "XX", 32.6f, 195.5f, kSmallest, kAmber);

    SkColor sc = slatsFault ? kAmber : anyOut ? kGreen : kWhite;
    SkColor fc = flapsFault ? kAmber : anyOut ? kGreen : kWhite;
    {
        double sf = 0.081, so = 0, pf = 0, po = 0;
        if (slats >= 0 && slats < 286.584) { so = 0; pf = 0.491985; po = 0; }
        else if (slats >= 286.584 && slats < 355) { so = 13.22; pf = 3.41585; po = -22.69; }
        double v = (slats * sf - so) * pf + po;
        float sx = (float)(-0.43 * v + 15.2), sy = (float)(0.09 * v + 195.3);
        char d[96];
        if (slatsValid) {
            std::snprintf(d, sizeof d, "M %.4f,%.4f a 0.2 0.2 0 0 1 -1.3 -1.9 l 1.4 -0.7 z", sx, sy);
            a380::chromium_stroke_path(c, svg(d), stroke_paint(sc, kNormal, true));
        }
        std::snprintf(d, sizeof d, "M 15.2 195.4 L %.4f,%.4f", sx, sy);
        a380::chromium_stroke_path(c, svg(d), stroke_paint(sc, kNormal, true));
    }
    {
        double sf = 0.22, so = 0, pf = 0, po = 0;
        if (flaps >= 0 && flaps < 214.927) { so = 0; pf = 0.18575; po = 2.8942; }
        else if (flaps >= 214.927 && flaps < 259.019) { so = 7.92; pf = 0.90853; po = -12.768; }
        else if (flaps >= 259.019 && flaps < 297.523) { so = 18.11; pf = 1.0295; po = -6.2983; }
        else if (flaps >= 297.523 && flaps < 355) { so = 26.8; pf = 0.9315; po = 4.8816; }
        double v = std::max((flaps * sf - 15.88 - so) * pf + po, 0.0);
        float fx = (float)(0.87 * v + 31.8), fy = (float)(0.365 * v + 193.1);
        char d[96];
        if (flapsValid) {
            std::snprintf(d, sizeof d, "M%.4f,%.4f v 2.6 h 3.9 z", fx, fy);
            a380::chromium_stroke_path(c, svg(d), stroke_paint(fc, kNormal, true));
        }
        std::snprintf(d, sizeof d, "M 31.8 193.1 L %.4f,%.4f", fx, fy);
        a380::chromium_stroke_path(c, svg(d), stroke_paint(fc, kNormal, true));
    }
}

void draw_rudder_trim(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    Mem& m = *s.mem;
    bool gnd = bit_or(x.v("L:A32NX_FWC_1_DISCRETE_WORD_126"), 28);
    int phase = (int)x.v("L:A32NX_FWC_FLIGHT_PHASE");
    bool t1 = bit_or(x.v("L:A32NX_SEC_1_RUDDER_STATUS_WORD"), 28), t3 = bit_or(x.v("L:A32NX_SEC_3_RUDDER_STATUS_WORD"), 28);
    Word p1 = arinc(x.v("L:A32NX_SEC_1_RUDDER_ACTUAL_POSITION")), p3 = arinc(x.v("L:A32NX_SEC_3_RUDDER_ACTUAL_POSITION"));
    double v = t1 ? ((p1.normal() || p1.ft()) ? p1.value : 0.0) : ((p3.normal() || p3.ft()) ? p3.value : 0.0);
    if (s.now - m.rtMs >= 250.0 || s.now < m.rtMs) {
        double dtMs = m.rtMs < -1e17 ? 0.0 : s.now - m.rtMs;
        m.rtMs = s.now;
        double cas = (s.speed.normal() || s.speed.ft()) ? s.speed.value : 0.0;
        double cas61 = (s.speed.normal() || s.speed.ft()) ? s.speed.value : 61.0;
        bool fast = (!gnd || (gnd && cas > 60)) && std::fabs(v) > 1;
        bool slow = gnd && cas61 < 60 && std::fabs(v) > 0.3;
        bool engOut = false;
        for (int i = 1; i <= 4; i++) engOut = engOut || !(x.v(("GENERAL ENG COMBUSTION:" + std::to_string(i)).c_str()) > 0);
        if (!engOut) {
            m.rtConfTimer = 0;
            m.rtConfPrevIn = false;
            m.rtConfOut = false;
        } else if (m.rtConfTimer > 0) {
            m.rtConfTimer = std::max(m.rtConfTimer - dtMs / 1000.0, 0.0);
            m.rtConfPrevIn = true;
            m.rtConfOut = false;
        } else if (!m.rtConfPrevIn) {
            m.rtConfTimer = 5.0;
            m.rtConfPrevIn = true;
            m.rtConfOut = false;
        } else {
            m.rtConfOut = true;
        }
        if (phase >= 3 && !gnd && m.rtConfOut) m.rtEngFailed = true;
        else if (m.rtEngFailed && phase < 3) m.rtEngFailed = false;
        bool cond = slow || fast || m.rtEngFailed;
        if (cond && !m.rtCond) m.rtStatus = true;
        else if (m.rtCond && !cond) {
            m.rtDelayAt = s.now;
            m.rtStatus = true;
        }
        m.rtCond = cond;
        if (!m.rtCond && s.now - m.rtDelayAt > 7000.0 && !m.rtEngFailed) m.rtStatus = false;
    }
    if (!(m.rtStatus && (t1 || t3))) return;
    text(c, "RUD TRIM", 41.2f, 184.7f, kSmallest, kWhite);
    c.drawRect(SkRect::MakeXYWH(41, 186, 23, 3.5f), stroke_paint(kWhite, kNormalOut));
    c.drawLine(52.5f, 186.f, 52.5f, 189.5f, stroke_paint(kWhite, kNormalOut));
    float xo = (float)(-11.5 * (v / 25.5));
    SkPathBuilder pb;
    pb.moveTo(52.5f + xo, 186.25f).lineTo(54.f + xo, 187.75f).lineTo(52.5f + xo, 189.25f).lineTo(51.f + xo, 187.75f).close();
    a380::draw_path_cached(c, pb.detach(), fill_paint(kCyan));
    double av = std::fabs(v);
    if (av >= 0.3 && !(phase >= 6 && phase <= 9)) {
        char nb[16];
        std::snprintf(nb, sizeof nb, "%.1f", av);
        std::string num = nb;
        for (size_t pad = std::strlen(nb); pad < 5; pad++) num = "\xC2\xA0" + num;
        std::string line = std::string((v >= 0 || av < 0.01) ? "L" : "R") + num;
        text(c, line, 41.2f, 194.5f, kSmallest, av < 3.6 ? kAmber : kRed);
        text(c, "\xC2\xB0", 57.5f, 194.5f, kSmallest, kCyan);
    }
}

void draw_lower(Ctx& x, State& s) {
    SkCanvas& c = x.c;
    {
        const char* w2n = x.fo ? "L:A32NX_FCU_EFIS_R_DISCRETE_WORD_2" : "L:A32NX_FCU_EFIS_L_DISCRETE_WORD_2";
        Word w2 = arinc(x.v(w2n));
        bool lsOn = w2.normal() || w2.ft() ? bit(x.v(w2n), 14) : true;
        if (lsOn || s.alt.normal()) fill(c, "m115.52 80.067v1.5119h-8.9706v-1.5119z", kYellow);
    }
    stroke(c, "M 2.1 157.7 h 154.4", kWhite, kThick);
    stroke(c, "M 67 158 v 51.8", kWhite, kThick);
    draw_slats_flaps(x, s);
    stroke(c, "M 15.2 195.5 h 12.4 l 4.1 0.2 l -0.1 -2.6 l -4 -0.9 l -2 -0.3 l -3 -0.1 l -3.5 0.1 l -3.8 0.8 z", kWhite,
           kNormal, true);
    {
        double pos = x.v("L:A32NX_LEFT_SPOILER_1_COMMANDED_POSITION");
        bool armed = x.v("L:A32NX_SPOILERS_ARMED") > 0;
        if (pos > 5.15 || armed) {
            stroke(c, "M 25.3 187.5 l 2.1 -3", kWhite, kNormal, true);
            stroke(c, "M 27.5 189 l 3.1 -1.8", kWhite, kNormal, true);
            if (pos >= 5.15) {
                double a = std::min(90.0, 90.0 / (1.0 + std::pow(std::exp(-pos + 17), 0.28))) * 3.14159265358979 / 180;
                char d[96];
                std::snprintf(d, sizeof d, "M 23 191.6 l %.4f %.4f", 5 * std::cos(a), -5 * std::sin(a));
                stroke(c, d, kGreen, kNormal, true);
            }
            if (armed) {
                fill(c, "M 21.6 183.5 h 3 l -1.5 3 z", kCyan);
                stroke(c, "M 21.6 183.5 h 3 l -1.5 3 z", kCyan, kNormal, true);
            }
        }
    }
    double lg = x.v("L:A32NX_LGCIU_1_DISCRETE_WORD_1");
    if (bit(lg, 23) && bit(lg, 24) && bit(lg, 25))
        stroke(c, "M 18.4 204.3 h 10 l -5 5.5 z M 20.9 204.3 v 2.6 M 23.4 204.3 v 5.5 M 25.9 204.3 v 2.6", kGreen, kNormal,
               true);
    draw_rudder_trim(x, s);
}

void text_pair(SkCanvas& c, const std::string& a, SkColor ca, const std::string& b, SkColor cb, float xm, float y, float size) {
    SkFont f = a380::font("Ecam", size);
    float wa = f.measureText(a.data(), a.size(), SkTextEncoding::kUTF8);
    float wb = f.measureText(b.data(), b.size(), SkTextEncoding::kUTF8);
    float wt = b.empty() ? f.measureText(" ", 1, SkTextEncoding::kUTF8) : 0.f;
    float x0 = xm - (wa + wb + wt) * 0.5f;
    if (!a.empty()) a380::draw_simple_text(c, a.data(), a.size(), SkTextEncoding::kUTF8, x0, y, f, fill_paint(ca));
    if (!b.empty()) a380::draw_simple_text(c, b.data(), b.size(), SkTextEncoding::kUTF8, x0 + wa, y, f, fill_paint(cb));
}

void draw_fma_cells(Ctx& x, State& st, bool useMan) {
    SkCanvas& c = x.c;
    auto word = [&](const char* fmt, int n) {
        char b[96];
        std::snprintf(b, sizeof b, fmt, x.prim);
        double raw = x.v(b);
        Word w = arinc(raw);
        return (w.normal() || w.ft()) && bit(raw, n);
    };
    auto W1 = [&](int n) { return word("L:A32NX_PRIM_%d_FG_DISCRETE_WORD_1", n); };
    auto W2 = [&](int n) { return word("L:A32NX_PRIM_%d_FG_DISCRETE_WORD_2", n); };
    auto W3 = [&](int n) { return word("L:A32NX_PRIM_%d_FG_DISCRETE_WORD_3", n); };
    auto W4 = [&](int n) { return word("L:A32NX_PRIM_%d_FG_DISCRETE_WORD_4", n); };
    auto W5 = [&](int n) { return word("L:A32NX_PRIM_%d_FG_DISCRETE_WORD_5", n); };
    auto W6 = [&](int n) { return word("L:A32NX_PRIM_%d_FG_DISCRETE_WORD_6", n); };
    auto AT = [&](int n) { return word("L:A32NX_PRIM_%d_FG_ATS_DISCRETE_WORD", n); };
    auto AF = [&](int n) { return word("L:A32NX_PRIM_%d_FG_ATS_FMA_DISCRETE_WORD", n); };
    auto sel = [&](const char* nm) { return x.w(nm, x.prim); };
    bool boxCell[8] = {};
    {
        uint64_t d1d2Cap = 0;
        bool d1d2Appr = false;
        {
            double fr = x.v("L:A32NX_FCDC_1_FG_DISCRETE_WORD_1");
            Word fw = arinc(fr);
            if (!(fw.normal() || fw.ft())) fr = x.v("L:A32NX_FCDC_2_FG_DISCRETE_WORD_1");
            fw = arinc(fr);
            if (fw.normal() || fw.ft()) d1d2Cap = bit(fr, 24) ? 1 : bit(fr, 25) ? 2 : bit(fr, 26) ? 3 : 0;
            const char* ew = x.fo ? "L:A32NX_FCU_EFIS_R_DISCRETE_WORD_2" : "L:A32NX_FCU_EFIS_L_DISCRETE_WORD_2";
            Word lw = arinc(x.v(ew));
            bool ls = (lw.normal() || lw.ft()) ? bit(x.v(ew), 14) : true;
            d1d2Appr = ls && x.v("L:A32NX_RADIO_RECEIVER_GS_IS_VALID") > 0 && x.v("L:A32NX_RADIO_RECEIVER_LOC_IS_VALID") > 0;
        }
        auto keysOf = [&](auto&& get, uint64_t* k, int capOv = -1) {
            auto bits = [&](const char* nm, std::initializer_list<int> l) {
                char b[96];
                std::snprintf(b, sizeof b, nm, x.prim);
                double raw = get(b);
                uint64_t m = 0;
                Word w = arinc(raw);
                if (!(w.normal() || w.ft())) return m;
                int i = 0;
                for (int n : l) {
                    if (bit(raw, n)) m |= 1ull << i;
                    i++;
                }
                return m;
            };
            const char* f1 = "L:A32NX_PRIM_%d_FG_DISCRETE_WORD_1";
            const char* f2 = "L:A32NX_PRIM_%d_FG_DISCRETE_WORD_2";
            const char* f3 = "L:A32NX_PRIM_%d_FG_DISCRETE_WORD_3";
            const char* f4 = "L:A32NX_PRIM_%d_FG_DISCRETE_WORD_4";
            const char* at = "L:A32NX_PRIM_%d_FG_ATS_DISCRETE_WORD";
            const char* af = "L:A32NX_PRIM_%d_FG_ATS_FMA_DISCRETE_WORD";
            const uint64_t abK = x.v("L:A32NX_AUTOBRAKES_ACTIVE") > 0 ? (uint64_t)x.v("L:A32NX_AUTOBRAKES_ARMED_MODE") : 0;
            k[0] = bits(af, {11, 12, 13, 14, 15, 16, 17, 18, 19, 20}) | bits(at, {11, 12}) << 12 | abK << 16;
            k[1] = bits(f3, {11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 25, 28, 29});
            k[2] = bits(f4, {11, 12, 13, 14, 15, 16, 17, 18, 19, 29});
            k[3] = bits(f4, {26}) ? 1 : bits(f3, {24}) ? 2 : bits(f1, {23}) ? 3 : 0;
            const uint64_t cap = capOv >= 0 ? (uint64_t)capOv : d1d2Cap;
            k[4] = cap ? cap : ((bits(f2, {28}) | bits(f1, {23})) || d1d2Appr) ? 4 : 0;
            k[5] = bits(f1, {11, 12});
            k[6] = bits(f1, {11, 12, 13, 14});
            k[7] = bits(at, {11, 12});
        };
        uint64_t cur[8];
        keysOf([&](const char* nm) { return x.in.var(nm, {}, 0.0); }, cur);
        double age[8];
        for (double& a : age) a = -1;
        if (x.in.has_env_number("fmaChangedAt")) {
            double since = x.in.env_number("frozenAt.perf", 0) - x.in.env_number("fmaChangedAt", 0);
            uint64_t prev[8];
            bool havePrev = x.in.has_env_number("fmaPrev.L:A32NX_PRIM_1_FG_DISCRETE_WORD_1") ||
                            x.in.has_env_number("fmaPrev.L:A32NX_PRIM_2_FG_DISCRETE_WORD_1") ||
                            x.in.has_env_number("fmaPrev.L:A32NX_PRIM_3_FG_DISCRETE_WORD_1");
            if (havePrev) {
                int capPrev = -1;
                for (int fi = 1; fi <= 2 && capPrev < 0; fi++) {
                    std::string kn = std::string("fmaPrev.L:A32NX_FCDC_") + std::to_string(fi) + "_FG_DISCRETE_WORD_1";
                    if (!x.in.has_env_number(kn)) continue;
                    double fr = x.in.env_number(kn, 0.0);
                    Word fw = arinc(fr);
                    if (fw.normal() || fw.ft() || fi == 2) capPrev = (fw.normal() || fw.ft()) ? (bit(fr, 24) ? 1 : bit(fr, 25) ? 2 : bit(fr, 26) ? 3 : 0) : 0;
                }
                keysOf([&](const char* nm) { return x.in.env_number(std::string("fmaPrev.") + nm, 0.0); }, prev, capPrev);
                for (int i = 0; i < 8; i++)
                    if (prev[i] != cur[i]) age[i] = since;
            }
        } else if (st.fma) {
            double now = x.in.now_ms();
            if (!st.fma->have) {
                st.fma->have = true;
                for (int i = 0; i < 8; i++) {
                    st.fma->key[i] = cur[i];
                    st.fma->at[i] = -1e18;
                }
            } else {
                for (int i = 0; i < 8; i++)
                    if (st.fma->key[i] != cur[i]) {
                        st.fma->key[i] = cur[i];
                        st.fma->at[i] = now;
                    }
            }
            for (int i = 0; i < 8; i++) age[i] = now - st.fma->at[i];
        }
        for (int i = 0; i < 8; i++) boxCell[i] = age[i] >= 0 && age[i] < (i == 4 ? 9000 : 10000);
    }

    bool atEng = AT(11), atAct = AT(12);
    bool abActive = x.v("L:A32NX_AUTOBRAKES_ACTIVE") > 0;
    int abMode = (int)x.v("L:A32NX_AUTOBRAKES_ARMED_MODE");
    const char* a1 = nullptr;
    float a1size = kMedium;
    bool manBox = false, manAmber = false;
    const char* manLine2 = nullptr;
    if (AF(11)) { manBox = true; manLine2 = "TOGA"; }
    else if (AF(13)) { manBox = true; manLine2 = "FLX"; }
    else if (AF(12) && atEng && !atAct) { manBox = true; manLine2 = "MCT"; }
    else if (AF(15) && atEng && !atAct) { manBox = true; manAmber = true; manLine2 = "THR"; }
    else if (AF(19)) a1 = "SPEED";
    else if (AF(20)) a1 = "MACH";
    else if (AF(12)) a1 = "THR MCT";
    else if (AF(14)) a1 = "THR CLB";
    else if (AF(15) && atEng && atAct) a1 = "THR LVR";
    else if (AF(16)) { a1 = "THR IDLE"; a1size = kMediumSmaller; }
    else if (AF(17) && atEng && atAct) a1 = "A.FLOOR";
    else if (AF(18)) a1 = "TOGA LK";
    else if (abActive && abMode == 1) a1 = "BTV";
    else if (abActive && abMode == 2) a1 = "BRK LO";
    else if (abActive && abMode == 3) a1 = "BRK 2 ";
    else if (abActive && abMode == 4) a1 = "BRK 3 ";
    else if (abActive && abMode == 5) a1 = "BRK HI ";
    else if (abActive && abMode == 6) a1 = "BRK RTO";
    bool afOn = blink_inf(st, kBlAfloor, a1 && (std::strcmp(a1, "A.FLOOR") == 0));
    bool tlOn = blink_inf(st, kBlTogaLk, a1 && (std::strcmp(a1, "TOGA LK") == 0));
    if (manBox) {
        bool flx = std::strcmp(manLine2, "FLX") == 0;
        stroke(c, flx ? "m29.821 1.8143v13.506h-24.517v-13.506z" : "m25.114 1.8143v13.506h-16.952v-13.506z",
               manAmber ? kAmber : kWhite);
        text(c, "MAN", 17.052249f, 7.1280665f, kMedium, kWhite, A::Middle);
        if (flx) {
            text(c, "FLX", 11.669141f, 14.351689f, kMedium, kWhite, A::Middle);
            text(c, "+", 20.599141f, 14.851689f, kMedium, kCyan, A::Middle);
            text(c, std::to_string((long long)js_round(x.v("L:A32NX_AIRLINER_TO_FLEX_TEMP"))), 26.099141f, 14.351689f, kMedium, kCyan, A::Middle);
        } else {
            text(c, manLine2, 16.869141f, 14.351689f, kMedium, kWhite, A::Middle);
        }
    } else if (a1) {
        bool amberBox = std::strcmp(a1, "A.FLOOR") == 0 || std::strcmp(a1, "TOGA LK") == 0;
        if (amberBox) {
            if (afOn || tlOn) stroke(c, "m0.70556 1.8143h30.927v6.0476h-30.927z", kAmber);
        } else if (boxCell[0]) stroke(c, "m3.3 1.8143h27.127v6.0476h-27.127z", kWhite);
        text(c, a1, 16.782249f, 7.1280665f, a1size, kGreen, A::Middle);
    }

    {
        const char* ab = nullptr;
        switch (abMode) {
            case 1: ab = "BTV "; break;
            case 2: ab = "BRK LO "; break;
            case 3: ab = "BRK 2 "; break;
            case 4: ab = "BRK 3 "; break;
            case 5: ab = "BRK HI "; break;
            default: break;
        }
        if (ab && !abActive && !manBox) text(c, ab, 16.782249f, 14.329653f, kMediumSmaller, kCyan, A::Middle);
    }

    bool a3msg = false;
    {
        const char* a3 = nullptr;
        SkColor a3c = kWhite;
        float a3s = kMedium;
        if (AF(26)) a3 = "LVR CLB";
        else if (AF(27)) a3 = "LVR MCT";
        else if (AF(25)) { a3 = "LVR ASYM"; a3c = kAmber; }
        else if (!abActive && abMode == 6) { a3 = "BRK RTO"; a3c = kCyan; a3s = kMediumSmaller; }
        a3msg = a3 != nullptr;
        bool a3flash = a3 && std::strcmp(a3, "BRK RTO") != 0 && std::strcmp(a3, "LVR ASYM") != 0;
        if (a3 && flash_inf(st, true, a3flash)) text(c, a3, 16.989958f, 21.641243f, a3s, a3c, A::Middle);
    }

    {
        Word pm = sel("L:A32NX_PRIM_%d_PRESEL_MACH"), ps = sel("L:A32NX_PRIM_%d_PRESEL_SPEED");
        if (!a3msg && !st.attExcessive) {
            char t[32];
            if (ps.normal() && !pm.normal()) {
                std::snprintf(t, sizeof t, "SPEED SEL %d", (int)ps.value);
                text(c, t, 35.434673f, 21.656223f, kMedium, kCyan, A::Middle);
            } else if (!ps.normal() && pm.normal()) {
                std::snprintf(t, sizeof t, "MACH SEL %.2f", pm.value);
                text(c, t, 35.434673f, 21.656223f, kMedium, kCyan, A::Middle);
            }
        }
    }

    std::string b1msg;
    {
        bool alt = W3(28), crz = W3(29);
        std::string b1, extra;
        float b1s = kMedium;
        if (W3(22)) b1 = "G/S";
        else if (W3(21)) b1 = "G/S*";
        else if (W3(15) || W3(16)) b1 = "SRS";
        else if (W3(25)) b1 = "TCAS";
        else if (W3(23)) b1 = "APP-DES";
        else if (W3(12)) b1 = "DES";
        else if (W3(14)) b1 = "OP DES";
        else if (W3(11)) b1 = "CLB";
        else if (W3(13)) b1 = "OP CLB";
        else if (W3(20) && !alt && !crz) b1 = "ALT";
        else if (W3(19) && !alt && !crz) b1 = "ALT*";
        else if (W3(19) && alt && !crz) b1 = "ALT CST*";
        else if (W3(20) && alt && !crz) b1 = "ALT CST";
        else if (W3(19) && !alt && crz) { b1 = "ALT CRZ*"; b1s = kMediumSmaller; }
        else if (W3(20) && !alt && crz) b1 = "ALT CRZ";
        else if (W3(18)) {
            b1s = kMediumSmaller;
            Word fpa = sel("L:A32NX_PRIM_%d_SELECTED_FLIGHT_PATH_ANGLE");
            b1 = "FPA";
            char t[32];
            if (fpa.normal() || fpa.ft()) {
                if (fpa.value == 0) b1 += " ";
                std::snprintf(t, sizeof t, "%s%.1f\xC2\xB0", fpa.value > 0 ? "+" : "", std::round(fpa.value * 10) / 10);
                extra = t;
            } else extra = "-----";
        } else if (W3(17)) {
            b1s = kMediumSmaller;
            Word vs = sel("L:A32NX_PRIM_%d_SELECTED_VERTICAL_SPEED");
            b1 = "V/S";
            if (vs.normal() || vs.ft()) {
                char t[32];
                std::snprintf(t, sizeof t, "%s%d", vs.value > 0 ? "+" : "", (int)std::lround(vs.value));
                extra = std::string(t);
                while (extra.size() < 5) extra = "\xC2\xA0" + extra;
            } else extra = "-----";
        }
        b1msg = b1;
        bool inSp = W5(29) && !b1.empty();
        if (!st.attExcessive) {
            if (flash_inf(st, inSp, true)) stroke(c, "m34.656 1.8143h29.918v6.0476h-29.918z", kAmber);
            if (boxCell[1] && !b1.empty() && !inSp)
                stroke(c, (W6(11) && W3(25)) ? "m35.756 1.8143h27.918v13.506h-27.918z" : "m35.756 1.8143h27.918v6.0476h-27.918z", kWhite);
            if (!b1.empty()) text_pair(c, b1, kGreen, extra, flash_inf(st, true, inSp) ? kCyan : kDimCyan, 49.921795f, 7.1040988f, b1s);
        }
    }

    {
        bool altArm = W2(11), alt = W3(28), crz = W3(29), gsArm = W2(13);
        std::string t1, t2;
        SkColor c1 = (altArm && alt) ? 0xFFFF94FF : kCyan;
        if (W2(17)) t1 = "      OP CLB";
        else if (W2(15)) t1 = "      CLB";
        else if (W2(16)) t1 = gsArm ? "DES " : "      DES";
        else if (altArm && alt) t1 = gsArm ? "ALT " : "      ALT";
        else if (altArm && crz) t1 = "     ALT CRZ";
        else if (altArm) t1 = gsArm ? "ALT " : "      ALT";
        else if (gsArm) t1 = "      G/S";
        if (gsArm && t1 != "      G/S") t2 = "  G/S";
        else if (W2(14)) t2 = "APP-DES";
        if (!st.attExcessive) {
            text(c, t1, 40.777474f, 13.629653f, kMediumSmaller, c1, A::Middle);
            text(c, t2, 56.19803f, 13.629653f, kMediumSmaller, kCyan, A::Middle);
        }
    }

    {
        const char* c1 = nullptr;
        bool bb = W4(29);
        if (W4(15)) c1 = "GA TRK";
        else if (W4(13) && bb) c1 = "LOC B/C*";
        else if (W4(13)) c1 = "LOC *";
        else if (W4(16)) c1 = "HDG";
        else if (W4(11) && W4(18)) c1 = "RWY";
        else if (W4(11) && W4(19)) c1 = "RWY TRK";
        else if (W4(17)) c1 = "TRACK";
        else if (W4(14) && bb) c1 = "LOC B/C";
        else if (W4(14)) c1 = "LOC";
        else if (W4(12)) c1 = "NAV";
        bool rollOut = W4(26), flare = W3(24), land = W1(23);
        if ((rollOut || flare || land) && !st.attExcessive) {
            const char* t = rollOut ? "ROLL OUT" : flare ? "FLARE" : "LAND";
            if (boxCell[3]) stroke(c, "m50.178 1.8143h35.174v6.0476h-35.174z", kWhite);
            text(c, t, 67.9795f, 6.8893085f, kMedium, kGreen, A::Middle);
        }
        if (c1 && !st.attExcessive) {
            if (boxCell[2]) stroke(c, "m99.87 1.8143v6.0476h-31.075l1e-6 -6.0476z", kWhite);
            text(c, c1, 84.856567f, 6.9873109f, kMedium, kGreen, A::Middle);
        }
        std::string t2;
        bool navArm = W2(22), locArm = W2(23), rwyArm = W2(24);
        if (locArm && bb) t2 = "LOC B/C";
        else if (locArm) t2 = "LOC";
        else if (rwyArm) t2 = std::string("RWY") + (navArm ? "  NAV" : "");
        else if (navArm) t2 = "NAV";
        if (!st.attExcessive) text(c, t2, 84.234184f, 13.629653f, kMediumSmaller, kCyan, A::Middle);
    }

    {
        double f1raw = x.v("L:A32NX_FCDC_1_FG_DISCRETE_WORD_1");
        Word f1 = arinc(f1raw);
        if (!(f1.normal() || f1.ft())) f1raw = x.v("L:A32NX_FCDC_2_FG_DISCRETE_WORD_1");
        Word f = arinc(f1raw);
        bool fok = f.normal() || f.ft();
        bool l2 = fok && bit(f1raw, 24), l3s = fok && bit(f1raw, 25), l3d = fok && bit(f1raw, 26);
        const char* ew = x.fo ? "L:A32NX_FCU_EFIS_R_DISCRETE_WORD_2" : "L:A32NX_FCU_EFIS_L_DISCRETE_WORD_2";
        Word lw = arinc(x.v(ew));
        bool ls = (lw.normal() || lw.ft()) ? bit(x.v(ew), 14) : true;
        bool appr1 = ls && x.v("L:A32NX_RADIO_RECEIVER_GS_IS_VALID") > 0 && x.v("L:A32NX_RADIO_RECEIVER_LOC_IS_VALID") > 0;
        std::string t1, t2;
        if (l2) t1 = "LAND2";
        else if (l3s || l3d) { t1 = "LAND3"; t2 = l3s ? "SINGLE" : "DUAL"; }
        else if (W2(28) || W1(23) || appr1) t1 = "APPR1";
        if (!st.attExcessive) {
            if (boxCell[4] && !t1.empty())
                stroke(c, !t2.empty() ? "m107.1 1.8143h22.994v13.506h-22.994z" : "m108.1 1.8143h19.994v6.0476h-19.994z", kWhite);
            text(c, t1, 118.45866f, 7.125926f, kMedium, kWhite, A::Middle);
            text(c, t2, 118.39752f, 14.289783f, kMedium, kWhite, A::Middle);
        }
    }

    {
        bool ap1 = W1(11), ap2 = W1(12), fd1 = W1(13), fd2 = W1(14);
        if (boxCell[5] && (ap1 || ap2)) stroke(c, "m156.13 1.8143v6.0476h-20.81v-6.0476z", kWhite);
        if (boxCell[6] && (ap1 || ap2 || fd1 || fd2)) stroke(c, "m156.13 9.0715v6.0476h-20.81v-6.0476z", kWhite);
        if (boxCell[7] && AT(11) && !AT(12)) stroke(c, "m135.32 16.329h20.81v6.0476h-20.81z", kWhite);
        if (ap1 || ap2) text(c, ap1 && ap2 ? "AP1+2" : ap1 ? "AP1" : "AP2", 145.61546f, 6.9559975f, kMedium, kWhite, A::Middle);
        if (ap1 || ap2 || fd1 || fd2) {
            std::string t = std::string(fd1 ? "1" : "-") + "FD" + (fd2 ? "2" : "-");
            text(c, t, 145.95045f, 14.417698f, kMedium, kWhite, A::Middle);
        }
        if (AT(11)) text(c, "A/THR", 145.75578f, 21.434536f, kMedium, AT(12) ? kWhite : kCyan, A::Middle);
    }

    {
        const bool ax = st.attExcessive;
        Mem& m = *st.mem;
        bool ap1 = W1(11), ap2 = W1(12);
        Word le = arinc(x.v("L:A32NX_FM1_LANDING_ELEVATION"));
        double altV = (st.alt.normal() || st.alt.ft()) ? st.alt.value : INFINITY;
        double raLim = st.ra.normal() ? st.ra.value : altV - ((le.normal() || le.ft()) ? le.value : 0.0);
        bool b1set = b1msg == "DES" || b1msg == "OP DES" || b1msg == "V/S" || b1msg == "APP-DES" || b1msg.rfind("FPA", 0) == 0;
        bool disc = (ap1 || ap2) && raLim <= 150 && b1set;
        int id = 0;
        const char* msg = nullptr;
        SkColor col = kWhite;
        float size = kMediumSmaller;
        bool pulse = false;
        if (useMan) { id = 1; msg = "USE MAN PITCH TRIM"; col = kAmber; size = kMedium; pulse = true; }
        else if (disc) { id = 3; msg = "DISCONNECT AP FOR LDG"; col = kAmber; size = 5.f; pulse = true; }
        else if (W2(18) && !ax) { id = 4; msg = "TCAS           "; col = kCyan; }
        else if (W6(12) && !ax) { id = 5; msg = "TCAS RA INHIBITED"; }
        else if (W6(13) && !ax) { id = 6; msg = "TRK FPA DESELECTED"; }
        else if (x.v("L:A32NX_PFD_MSG_TD_REACHED") > 0) { id = 8; msg = "T/D REACHED"; }
        else if (x.v("L:A32NX_PFD_MSG_SET_HOLD_SPEED") > 0) { id = 12; msg = "SET HOLD SPD"; }
        else if (x.v("L:A32NX_BTV_EXIT_MISSED") > 0) { id = 13; msg = "EXIT MISSED"; }
        else if (W6(16) && !ax) { id = 14; msg = "FCU ALT BELOW A/C"; }
        else if (W6(15) && !ax) { id = 15; msg = "FCU ALT ABOVE A/C"; }
        if (id != m.bc3Prev) {
            m.bc3Prev = id;
            m.bc3At = st.now;
        }
        if (msg) {
            SkColor dc = col;
            if (pulse) {
                double pms = x.v("A380_ANIM:pulse-amber-fill", -1);
                if (pms < 0 && !st.offline) pms = st.now - m.bc3At;
                if (pms >= 0 && pms < 9000) dc = std::fmod(pms, 1000.0) < 500.0 ? SkColorSetRGB(161, 90, 0) : SkColorSetRGB(255, 168, 59);
            }
            text(c, msg, 68.087875f, 21.627102f, size, dc, A::Middle);
        }
        bool bc3any = msg != nullptr;
        bool shared = W4(26) || W3(24) || W1(23);
        Word pmW = sel("L:A32NX_PRIM_%d_PRESEL_MACH"), psW = sel("L:A32NX_PRIM_%d_PRESEL_SPEED");
        bool ab3 = (pmW.normal() || psW.normal()) && !bc3any && !a3msg;
        stroke(c, (ab3 && !ax) ? "m33.117 0.33732v15.766" : "m33.117 0.33732v20.864", kGrey);
        if (!(shared && !ax)) stroke(c, bc3any ? "m66.241 0.33732v15.766" : "m66.241 0.33732v20.864", kGrey);
        stroke(c, "m102.52 0.33732v20.864", kGrey);
        stroke(c, "m133.72 0.33732v20.864", kGrey);
        if (!ax) {
            int fm = fm_index(x);
            double eis = x.v(fm == 1 ? "L:A32NX_FM1_EIS_DISCRETE_WORD_2" : "L:A32NX_FM2_EIS_DISCRETE_WORD_2");
            Word mdaW = arinc(x.v(fm == 1 ? "L:A32NX_FM1_MINIMUM_DESCENT_ALTITUDE" : "L:A32NX_FM2_MINIMUM_DESCENT_ALTITUDE"));
            Word dhW = arinc(x.v("L:A32NX_FM1_DECISION_HEIGHT"));
            bool noDh = bit_or(eis, 29);
            std::string mode, val;
            if (noDh) mode = "NO DH";
            else if (!dhW.ncd() && !dhW.fw()) {
                mode = "RADIO";
                val = std::to_string((long long)js_round(dhW.value));
            } else if (!mdaW.ncd() && !mdaW.fw()) {
                mode = "BARO";
                val = std::to_string((long long)js_round(mdaW.value));
            }
            if (noDh) text(c, mode, 118.38384f, 21.104172f, kMedium, kWhite, A::Middle);
            else text(c, mode, 103.47f, 21.104172f, kSmallest, kWhite);
            if (!val.empty()) text(c, val, 133.425f, 21.104172f, kSmallest, kCyan, A::End);
        }
    }
}

void draw_fma(Ctx& x, State& s) {
    double f1raw = x.v("L:A32NX_FCDC_1_DISCRETE_WORD_1");
    Word f1 = arinc(f1raw);
    if (!(f1.normal() || f1.ft())) {
        f1raw = x.v("L:A32NX_FCDC_2_DISCRETE_WORD_1");
        f1 = arinc(f1raw);
    }
    int phase = (int)x.v("L:A32NX_FWC_FLIGHT_PHASE");
    bool phaseOk = phase >= 2 && phase <= 11 && !(phase >= 4 && phase <= 7);
    bool useMan = bit(f1raw, 15) && !f1.fw() && phaseOk;
    draw_fma_cells(x, s, useMan);
}

void draw_limitations(Ctx& x) {
    SkCanvas& c = x.c;
    for (int i = 0; i < 8; i++) {
        char nm[64];
        std::snprintf(nm, sizeof nm, "L:A32NX_PFD_LIMITATIONS_LINE_%d", i + 1);
        long long id = (long long)x.v(nm);
        if (id == 0) continue;
        for (auto& e : kLimitations) {
            if (e.id != id) continue;
            std::string t = e.text;
            SkColor col = kWhite;
            if (t.size() > 4 && t[0] == 27) {
                char code = t[2];
                col = code == '1' ? kYellow : code == '2' ? kRed : code == '3' ? kGreen : code == '4' ? kAmber
                    : code == '5' ? kCyan : code == '6' ? 0xFFFF94FF : kWhite;
                t = t.substr(4);
            }
            text(c, t, 70.f, 165.f + i * 6.1f, kMediumSmaller, col);
        }
    }
}

sk_sp<SkImage> trim_image() {
    thread_local sk_sp<SkImage> img;
    thread_local bool tried = false;
    if (tried) return img;
    tried = true;
    auto data = a380::assets::image_data("TRIM_INDICATOR.png");
    if (!data) return img;
    auto codec = SkPngDecoder::Decode(data, nullptr);
    if (!codec) return img;
    auto r = codec->getImage();
    if (std::get<0>(r)) img = std::get<0>(r);
    return img;
}

void draw_trim(Ctx& x, State& s, bool visible) {
    SkCanvas& c = x.c;
    if (!visible) {
        c.save();
        c.translate(0, (1024.f - 211.6f * kS) * 0.5f);
        c.scale(kS, kS);
        draw_limitations(x);
        c.restore();
        return;
    }
    double trim = x.v("ELEVATOR TRIM POSITION") * 180.0 / 3.14159265358979;
    double ths = trim > 4.68 ? 34.5 - (trim - 4.68) * 275 / 56 : 43 - (trim + 0.18) * 425 / 243;
    char t[16];
    std::snprintf(t, sizeof t, "%.1f", std::round(ths * 10) / 10);
    if (auto im = trim_image()) {
        SkPaint ip;
        ip.setAntiAlias(true);
        c.drawImageRect(im, SkRect::MakeXYWH(580, 765, 100, 257), SkSamplingOptions(SkFilterMode::kLinear, SkMipmapMode::kLinear), &ip);
    }
    c.save();
    c.translate(326, 765.f + 0.5f / 1.5f);
    auto px = [&](float sz, const char* str, float xx, float yy, SkColor col) {
        SkFont f = a380::font("Ecam", sz);
        a380::draw_simple_text(c, str, std::strlen(str), SkTextEncoding::kUTF8, xx, yy, f, fill_paint(col));
    };
    px(21.7f, "T.O THS", 2, 138, kWhite);
    bool engRun = x.v("GENERAL ENG COMBUSTION:1") > 0 || x.v("GENERAL ENG COMBUSTION:2") > 0 ||
                  x.v("GENERAL ENG COMBUSTION:3") > 0 || x.v("GENERAL ENG COMBUSTION:4") > 0;
    double tphase = x.v("L:A32NX_FWC_FLIGHT_PHASE");
    Word cgW = arinc(x.v("L:A32NX_FQMS_CENTER_OF_GRAVITY_MAC"));
    auto cgToTrim = [](double cg) {
        if (cg < 29) return 5.8;
        if (cg <= 34.5) return 5.8 - ((cg - 29) * 56) / 275;
        if (cg <= 43) return 4.68 - ((cg - 34.5) * 243) / 425;
        return -0.18;
    };
    bool outOfRange = trim > 5.8 || trim < -0.18;
    int tstat = (!engRun || tphase > 9) ? 0 : outOfRange ? 1 : (cgW.normal() && std::fabs(cgToTrim(cgW.value) - trim) < 1.5) ? 2 : 3;
    SkColor cgCol = tstat == 0 ? kWhite : tstat == 1 ? kRed : tstat == 2 ? kGreen : kAmber;
    if (tstat == 1) {
        px(26.6f, "OUT OF", 125, 130, kRed);
        px(26.6f, "RANGE", 130, 160, kRed);
    } else {
        px(21.7f, "FOR", 110, 138, kWhite);
        px(26.6f, t, 162, 140, cgCol);
        px(21.7f, "%", 232, 140, kCyan);
    }
    bool gwVis = cgW.normal() && engRun && tphase <= 10 &&
                 (x.v("L:A32NX_HYD_GREEN_SYSTEM_1_SECTION_PRESSURE_SWITCH") > 0 || x.v("L:A32NX_HYD_YELLOW_SYSTEM_1_SECTION_PRESSURE_SWITCH") > 0);
    double optimal = cgW.normal() ? cgToTrim(cgW.value) : 0.0;
    if (gwVis) {
        c.save();
        c.translate(0, (float)((trim - optimal) * 17.25 - 5));
        c.drawRect(SkRect::MakeXYWH(360, 117, 71, 30), fill_paint(kBlack));
        c.drawRect(SkRect::MakeXYWH(360, 117, 71, 30), stroke_paint(kMagenta, 2, false));
        char cgt[16];
        std::snprintf(cgt, sizeof cgt, "%.1f", cgW.value);
        px(21.7f, cgt, 362, 140, kMagenta);
        px(21.7f, "%", 416, 140, kCyan);
        c.restore();
    }
    float deg2px = (float)((10 - trim) * 17.25 + 103.5);
    auto shadow = [&](float x0, float y0, float w, float h) {
        for (float yy = 0; yy < h; yy += 0.5f) {
            float t = (yy + 0.25f) / h;
            float al = t < 0.1f ? 1.f - t / 0.1f : t > 0.9f ? (t - 0.9f) / 0.1f : 0.f;
            if (al <= 0.f) continue;
            SkPaint sp;
            sp.setColor(SkColorSetARGB((uint8_t)std::lround(al * 255), 0, 0, 0));
            c.drawRect(SkRect::MakeXYWH(x0, y0 + yy, w, 0.5f), sp);
        }
    };
    c.save();
    c.clipRect(SkRect::MakeXYWH(276, 23, 30, 207));
    c.save();
    c.translate(276, -(deg2px - 103.5f) + 23);
    c.drawRect(SkRect::MakeXYWH(0, 0, 28, 414), fill_paint(0xFF555F72));
    c.drawRect(SkRect::MakeXYWH(2, 103.5f, 25, 207), stroke_paint(kWhite, 2, false));
    c.drawRect(SkRect::MakeXYWH(3, 175.95f, 23, 103.5f), stroke_paint(kGreen, 4, false));
    if (gwVis) {
        float up = (float)((10 - std::min(optimal + 1.5, 5.8)) * 17.25 + 103.5);
        float ctr = (float)((10 - optimal) * 17.25 + 103.5);
        float lo = (float)((10 - std::max(optimal - 1.5, -0.2)) * 17.25 + 103.5);
        c.drawRect(SkRect::MakeXYWH(6, up, 16, ctr - up), stroke_paint(kMagenta, 4, false));
        c.drawRect(SkRect::MakeXYWH(6, ctr, 16, lo - ctr), stroke_paint(kMagenta, 4, false));
    }
    if (tphase >= 11 && s.gs.normal() && s.gs.value <= 30) c.drawRect(SkRect::MakeXYWH(5, 274, 20, 4), fill_paint(kWhite));
    c.restore();
    shadow(275, 23, 30, 207);
    c.restore();
    c.save();
    c.clipRect(SkRect::MakeXYWH(319, 23, 16, 207));
    float wy = (float)(std::fmod(trim * 140, 150.0) - 300 + 126);
    c.drawRect(SkRect::MakeXYWH(320, wy, 14, 600), fill_paint(kWhite));
    for (int i = 0; i < 16; i++) c.drawRect(SkRect::MakeXYWH(320, wy + i * 37.5f + 0.0006f * 600, 14, 15.6f), fill_paint(0xFF555F72));
    shadow(319, 23, 16, 207);
    c.restore();
    SkPathBuilder b;
    b.moveTo(265, 115).lineTo(265, 138).lineTo(277, 126.5f).close();
    a380::draw_path_cached(c, b.detach(), fill_paint(cgCol));
    struct Stop { float at; uint32_t col; };
    auto vgrad = [&](float x0, float y0, float w, float h, std::initializer_list<Stop> stops) {
        std::vector<Stop> st(stops);
        for (float yy = 0; yy < h; yy += 0.25f) {
            float t = (yy + 0.125f) / h;
            uint32_t col = st.back().col;
            if (t <= st.front().at) col = st.front().col;
            for (size_t i = 1; i < st.size(); i++) {
                if (t <= st[i].at) {
                    float u = (t - st[i - 1].at) / (st[i].at - st[i - 1].at);
                    auto ch = [&](int sh) {
                        float a = (st[i - 1].col >> sh) & 0xFF, b = (st[i].col >> sh) & 0xFF;
                        return (uint32_t)std::lround(a + (b - a) * u);
                    };
                    col = 0xFF000000u | ch(16) << 16 | ch(8) << 8 | ch(0);
                    break;
                }
            }
            c.drawRect(SkRect::MakeXYWH(x0, y0 + yy, w, std::min(0.25f, h - yy)), fill_paint(col));
        }
    };
    vgrad(277, 118, 28, 4, {{0.f, 0xFF8FADC0}, {0.9f, 0xFF6387A1}});
    vgrad(277, 130, 28, 4, {{0.f, 0xFF8FADC0}, {0.9f, 0xFF6387A1}});
    vgrad(318, 113, 2, 25, {{0.f, 0xFF8FADC0}, {0.6f, 0xFF444444}, {1.f, 0xFF8FADC0}});
    vgrad(334, 113, 2, 25, {{0.f, 0xFF8FADC0}, {0.6f, 0xFF444444}, {1.f, 0xFF8FADC0}});
    c.restore();
}

class Pfd : public Display {
public:
    explicit Pfd(bool fo) : prof_(fo ? "pfd-2" : "pfd-1"), fo_(fo), du_(fo ? "90" : "88", {fo ? "DC_2" : "108PH"}) {}
    SkISize size() const override { return {768, 1024}; }
    void draw(SkCanvas& canvas, const Inputs& in) override {
        if (!du_.update(canvas, in, size())) return;
        double now = in.now_ms();
        double dt = frameMs_ < -1e17 ? 0.0 : std::clamp((now - frameMs_) / 1000.0, 0.0, 0.5);
        frameMs_ = now;
        auto supplier = [&](double knob) { return fo_ ? (knob == 2 ? 3 : 2) : (knob == 0 ? 3 : 1); };
        int ir = supplier(in.var("L:A32NX_ATT_HDG_SWITCHING_KNOB"));
        int ad = supplier(in.var("L:A32NX_AIR_DATA_SWITCHING_KNOB"));
        int primSel = fo_ ? 2 : 1;
        primSel = 1;
        for (int n = 1; n <= 3; n++) {
            char b[64];
            std::snprintf(b, sizeof b, "L:A32NX_PRIM_%d_FCTL_LAW_STATUS_WORD", n);
            double raw = in.var(b);
            Word w = arinc(raw);
            if ((w.normal() || w.ft()) && bit(raw, 21)) {
                primSel = n;
                break;
            }
        }
        Ctx x{canvas, in, ad, primSel, fo_};
        State s;
        s.fma = &fma_;
        s.horizon = &horizon_;
        s.mem = &mem_;
        s.now = now;
        s.dt = dt;
        s.offline = in.has_env_number("frozenAt.perf");
        s.pitch = x.w("L:A32NX_ADIRS_IR_%d_PITCH", ir);
        s.roll = x.w("L:A32NX_ADIRS_IR_%d_ROLL", ir);
        s.heading = x.w("L:A32NX_ADIRS_IR_%d_HEADING", ir);
        s.track = x.w("L:A32NX_ADIRS_IR_%d_TRACK", ir);
        s.speed = x.w("L:A32NX_ADIRS_ADR_%d_COMPUTED_AIRSPEED", ad);
        s.mach = x.w("L:A32NX_ADIRS_ADR_%d_MACH", ad);
        {
            const char* w2n = fo_ ? "L:A32NX_FCU_EFIS_R_DISCRETE_WORD_2" : "L:A32NX_FCU_EFIS_L_DISCRETE_WORD_2";
            double w2raw = in.var(w2n);
            Word w2 = arinc(w2raw);
            bool stdAlt = !(w2.normal() || w2.ft()) || bit(w2raw, 11);
            s.alt = stdAlt ? arinc(in.var("L:A32NX_ADIRS_ADR_1_ALTITUDE"))
                           : x.w(fo_ ? "L:A32NX_ADIRS_ADR_%d_BARO_CORRECTED_ALTITUDE_2" : "L:A32NX_ADIRS_ADR_%d_BARO_CORRECTED_ALTITUDE_1", ad);
            if (std::isfinite(s.alt.value)) {
                if (!mem_.altInit) {
                    mem_.altInit = true;
                    mem_.altOut = s.alt.value;
                } else {
                    mem_.altOut = s.alt.value + (mem_.altOut - s.alt.value) * std::exp(-std::min(dt, 0.3) / 0.3);
                }
                s.alt.value = mem_.altOut;
            }
        }
        Word vsi = x.w("L:A32NX_ADIRS_IR_%d_VERTICAL_SPEED", ir);
        Word vsb = x.w("L:A32NX_ADIRS_ADR_%d_BAROMETRIC_VERTICAL_SPEED", ad);
        s.vs = vsi.normal() ? vsi : vsb;
        {
            Word r[3] = {x.w("L:A32NX_RA_%d_RADIO_ALTITUDE", 1), x.w("L:A32NX_RA_%d_RADIO_ALTITUDE", 2),
                         x.w("L:A32NX_RA_%d_RADIO_ALTITUDE", 3)};
            s.ra = choose_ra(r, fo_);
        }
        s.raF = mem_.ra.step(s.ra.value, dt, 5.0);
        s.gs = x.w("L:A32NX_ADIRS_IR_%d_GROUND_SPEED", ir);
        s.beta = in.var("INCIDENCE BETA");
        s.betaTarget = in.var("L:A32NX_BETA_TARGET");
        s.betaActive = in.var("L:A32NX_BETA_TARGET_ACTIVE");
        if (now - mem_.latAccMs >= 500.0 || now < mem_.latAccMs) {
            mem_.latAccMs = now;
            mem_.latAcc = in.var("ACCELERATION BODY X");
        }
        s.latAcc = mem_.latAcc;
        s.onGround = in.var("L:A32NX_LGCIU_1_LEFT_GEAR_COMPRESSED") > 0 || in.var("L:A32NX_LGCIU_1_RIGHT_GEAR_COMPRESSED") > 0;
        if (now - mem_.attEvalMs >= 1000.0 || now < mem_.attEvalMs) {
            mem_.attEvalMs = now;
            bool p = s.pitch.normal(), r = s.roll.normal();
            if (!mem_.attExcessive && ((p && (s.pitch.value > 25 || s.pitch.value < -13)) || (r && std::fabs(s.roll.value) > 45)))
                mem_.attExcessive = true;
            else if (mem_.attExcessive && p && s.pitch.value < 22 && s.pitch.value > -10 && r && std::fabs(s.roll.value) < 40)
                mem_.attExcessive = false;
        }
        s.attExcessive = mem_.attExcessive;
        if (s.onGround) {
            mem_.airborne = false;
        } else if (!mem_.airborne) {
            mem_.airborne = true;
            mem_.airborneAt = s.offline ? -1e18 : now;
        }
        s.showVls = mem_.airborne && now - mem_.airborneAt >= 10000.0;
        {
            bool tla = false;
            for (int i = 1; i <= 4; i++) tla = tla || in.var(("L:A32NX_AUTOTHRUST_TLA:" + std::to_string(i)).c_str()) >= 35;
            bool left = in.var("L:A32NX_LGCIU_1_LEFT_GEAR_COMPRESSED") > 0, right = in.var("L:A32NX_LGCIU_1_RIGHT_GEAR_COMPRESSED") > 0;
            bool approach = (int)in.var("L:A32NX_FMGC_FLIGHT_PHASE") == 5;
            s.tailStrike = (tla && left && right) || (approach && s.ra.value < 400 && s.speed.value > 50);
        }
        s.vsF = mem_.vs.step(s.vs.value, dt, 2.0);
        if (s.vsF != 0) vsMoved_ = true;
        s.vsMoved = vsMoved_;
        {
            double raw;
            if (s.onGround) {
                double acc = std::clamp(std::round(s.latAcc * 100) / 100, -0.3, 0.3);
                raw = -acc * 15 / 0.3;
            } else {
                raw = std::clamp(s.beta - s.betaTarget, -15.0, 15.0);
            }
            double sdt = slipMs_ < 0 ? 0.0 : std::max(0.0, (now - slipMs_) / 1000.0);
            slipMs_ = now;
            double input = std::isnan(raw) ? 0.0 : raw;
            double k = sdt * 0.8, sum0 = k + 2;
            double out = (input + slipIn_) * k / sum0 + (2 - k) / sum0 * slipOut_;
            slipIn_ = input;
            if (std::isfinite(out)) slipOut_ = out;
            s.slip = std::isfinite(out) ? out : 0.0;
        }
        if (s.pitch.normal())
            s.yOff = std::clamp(horizon_offset(std::round(s.pitch.value * 1000) / 1000), -31.563f, 31.563f);

        canvas.save();
        canvas.translate(0, (1024.f - 211.6f * kS) * 0.5f);
        canvas.scale(kS, kS);
        prof_.time("draw_horizon", [&] { draw_horizon(x, s); });
        prof_.time("draw_fixed_center", [&] { draw_fixed_center(x, s, ir); });
        prof_.time("masks", [&] { fill_eo(canvas,
             "m 32.138 101.25 c 7.4164 13.363 21.492 21.652 36.768 21.652 c 15.277 0 29.352 -8.2886 36.768 -21.652 v -40.859 "
             "c -7.4164 -13.363 -21.492 -21.652 -36.768 -21.652 c -15.277 0 -29.352 8.2886 -36.768 21.652 z m -32.046 110.498 "
             "h 158.66 v -211.75 h -158.66 z",
             kBlack); });
        prof_.time("draw_heading", [&] { draw_heading(x, s); });
        prof_.time("draw_alt", [&] { draw_alt(x, s); });
        prof_.time("draw_speed", [&] { draw_speed(x, s); });
        prof_.time("masks", [&] { fill_eo(canvas,
             "m 32.138 145.34 h 73.536 v 10.382 h -73.536 z m 0 -44.092 c 7.4164 13.363 21.492 21.652 36.768 21.652 c 15.277 0 "
             "29.352 -8.2886 36.768 -21.652 v -40.859 c -7.4164 -13.363 -21.492 -21.652 -36.768 -21.652 c -15.277 0 -29.352 "
             "8.2886 -36.768 21.652 z m -32.046 110.498 h 158.66 v -211.746 h -158.66 z m 115.14 -88.191 v -85.473 h 20.344 v "
             "85.473 z m -113.33 0 v -85.473 h 27.548 v 85.473 z",
             kBlack); });
        prof_.time("draw_speed_offtape", [&] { draw_speed_offtape(x, s); });
        prof_.time("draw_ls", [&] { draw_ls(x, s); });
        prof_.time("draw_heading_offtape", [&] { draw_heading_offtape(x, s); });
        prof_.time("draw_fixed_upper", [&] { draw_fixed_upper(x, s); });
        prof_.time("draw_warnings", [&] { draw_warnings(x, s); });
        prof_.time("draw_vsi", [&] { draw_vsi(x, s); });
        prof_.time("draw_selected", [&] { draw_selected(x, s); });
        prof_.time("draw_alt_offtape", [&] { draw_alt_offtape(x, s); });
        prof_.time("draw_linear_dev", [&] { draw_linear_dev(x, s); });
        prof_.time("draw_baro", [&] { draw_baro(x, s); });
        prof_.time("draw_mach", [&] { draw_mach(x, s); });
        prof_.time("draw_fma", [&] { draw_fma(x, s); });
        prof_.time("draw_lower", [&] { draw_lower(x, s); });
        canvas.restore();
        double gsv = s.gs.normal() ? s.gs.value : 0.0;
        int flapHandle = (int)std::lround(in.var("L:A32NX_FLAPS_HANDLE_INDEX"));
        bool flapsRetracted = prevFlapHandle_ > flapHandle;
        prevFlapHandle_ = flapHandle;
        int tlaOut = 0;
        for (int i = 1; i <= 4; i++) tlaOut += in.var(("L:A32NX_AUTOTHRUST_TLA:" + std::to_string(i)).c_str()) > 5;
        if (s.raF > 50) trimVisible_ = false;
        else if (gsv < 30) trimVisible_ = true;
        else if (s.onGround && gsv > 80 && (in.var("L:A32NX_SPOILERS_ARMED") == 0 || flapsRetracted || tlaOut > 2)) trimVisible_ = true;
        prof_.time("draw_trim", [&] { draw_trim(x, s, trimVisible_); });
        prof_.frame();
    }

private:
    a380::SectionProfile prof_;
    bool fo_;
    a380::CdsDisplayUnit du_;
    bool vsMoved_ = false;
    bool trimVisible_ = false;
    int prevFlapHandle_ = 0;
    FmaTrack fma_;
    HorizonCache horizon_;
    Mem mem_;
    double frameMs_ = -1e18;
    double slipIn_ = 0, slipOut_ = 0, slipMs_ = -1;
};

std::unique_ptr<Display> make(std::string_view variant) { return std::make_unique<Pfd>(variant == "2"); }
A380_REGISTER_DISPLAY("pfd", make);

}  // namespace
}  // namespace a380::pfd
