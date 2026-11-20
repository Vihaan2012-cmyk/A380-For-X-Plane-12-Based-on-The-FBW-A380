#include <algorithm>
#include <cmath>
#include "a380/shape_cache.h"
#include "a380/env.h"
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <exception>
#include <map>
#include <string>
#include <vector>

#include "include/core/SkPaint.h"
#include "include/core/SkPath.h"
#include "include/core/SkPathBuilder.h"

#include "a380/profile.h"
#include "a380/display.h"
#include "a380/display_unit.h"
#include "a380/draw.h"
#include "a380/fonts.h"
#include "a380/raster.h"

#include "ecam_data.inc"

namespace {

using a380::Align;
using a380::Baseline;
using a380::rgb;
using nlohmann::json;
constexpr double kPi = 3.14159265358979323846;

constexpr uint32_t kWhite = 0xffffff, kGreen = 0x00ff00, kAmber = 0xe68000, kCyan = 0x00ffff, kRed = 0xff0000,
                   kGrey = 0x4f5467, kLight = 0xc4c6cf, kMagenta = 0xff94ff, kYellow = 0xffff00;

float kDpr = 1.5f;
float kEclBorder = 4.f / 1.5f;
float kLineDev = 35.f, kAscDev = 31.f;
float kLineH = 32.89f;
float kTopDy = 0.f, kTextDy = 0.f, kStsDy = 0.f;
float kRowOff[18] = {}, kRowH[18] = {};
bool kChromiumBase = false;
bool kAbnFit = false;
float kBaseGap = 2.f;
constexpr float kInt = 510.42f;
bool kAllBase = true;
float kSvgDy = 0.f;
constexpr float kWdTop = 379.f;
constexpr int kNumLines = 18;

const json& ecam() {
    static const json j = json::parse(reinterpret_cast<const char*>(kEcamJson));
    return j;
}

struct TimerSeen {
    double ts = 0.0;
    double seenMs = 0.0;
};

struct EwdState {
    bool idleVisible = false;
    double idleStartMs = 0.0;
    double nowMs = 0.0;
    std::map<std::string, TimerSeen> timers;
};

EwdState* gState = nullptr;

constexpr uint32_t kAmberDim = 0xa15a00, kAmberBright = 0xffa83b, kGreenDim = 0x008000;

bool pulse_bright(double nowMs) { return std::fmod(nowMs / 1000.0, 1.0) >= 0.5; }

bool jtruthy(const json& j) {
    if (j.is_boolean()) return j.get<bool>();
    if (j.is_number()) {
        double d = j.get<double>();
        return d != 0.0 && !std::isnan(d);
    }
    if (j.is_string()) return !j.get<std::string>().empty();
    return false;
}

bool jflag(const json& o, const char* k) {
    auto it = o.find(k);
    return it != o.end() && jtruthy(*it);
}

bool jarr(const json& o, const char* k, size_t i) {
    auto it = o.find(k);
    return it != o.end() && it->is_array() && i < it->size() && jtruthy((*it)[i]);
}

std::string jid(const json& o, const char* k) {
    auto it = o.find(k);
    if (it == o.end()) return "";
    if (it->is_string()) return it->get<std::string>();
    if (it->is_number()) return std::to_string(it->get<long long>());
    return "";
}

struct Pt {
    float x, y;
};

Pt polar(float cx, float cy, float r, float deg) {
    float a = (deg - 90.f) * float(kPi) / 180.f;
    return {cx + r * std::cos(a), cy + r * std::sin(a)};
}

SkPaint stroke_paint(uint32_t c, float w, SkPaint::Cap cap = SkPaint::kButt_Cap) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setStyle(SkPaint::kStroke_Style);
    p.setStrokeWidth(w);
    p.setColor(rgb(c));
    p.setStrokeCap(cap);
    return p;
}

SkPaint fill_paint(uint32_t c) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(rgb(c));
    return p;
}

void arc(SkCanvas& cv, float x, float y, float r, float start, float end, const SkPaint& p) {
    float span = end >= start ? end - start : 360.f - start + end;
    a380::chromium_stroke_arc(cv, x, y, r, (end - 90.f) * float(kPi) / 180.f, -span * float(kPi) / 180.f, p);
}


struct ArcSeg {
    float cx, cy, theta, sweep;
};

ArcSeg svg_arc(float x1, float y1, float x2, float y2, float r, bool largeArc, bool sweepPos) {
    double hx = (x1 - x2) * 0.5, hy = (y1 - y2) * 0.5;
    double d2 = hx * hx + hy * hy;
    double rr = r;
    if (d2 > rr * rr) rr = std::sqrt(d2);
    double coef = d2 > 0 ? std::sqrt(std::max(0.0, (rr * rr - d2) / d2)) : 0.0;
    double sg = largeArc != sweepPos ? 1.0 : -1.0;
    double cxp = sg * coef * hy, cyp = -sg * coef * hx;
    double cx = cxp + (x1 + x2) * 0.5, cy = cyp + (y1 + y2) * 0.5;
    double t1 = std::atan2(y1 - cy, x1 - cx), t2 = std::atan2(y2 - cy, x2 - cx);
    double dt = t2 - t1;
    if (sweepPos && dt < 0) dt += 2 * kPi;
    if (!sweepPos && dt > 0) dt -= 2 * kPi;
    return {float(cx), float(cy), float(t1), float(dt)};
}

float round2(float v) { return std::round(v * 100.f) / 100.f; }

void line(SkCanvas& cv, float x1, float y1, float x2, float y2, const SkPaint& p) {
    a380::chromium_stroke_line(cv, x1, y1, x2, y2, p.getStrokeWidth(), p);
}

void srect(SkCanvas& cv, const SkRect& r, float w, uint32_t c) {
    SkPaint p = fill_paint(c);
    a380::chromium_stroke_rect(cv, r, w, p);
}

struct Gauge {
    float x, y, r, start, end, mn, mx;
    Pt dir(float v) const {
        float pct = (v - mn) / (mx - mn);
        float span = start > end ? 360.f - start + end : end - start;
        float a = (start + pct * span - 90.f) * float(kPi) / 180.f;
        return {std::cos(a), std::sin(a)};
    }
};

void marker(SkCanvas& cv, const Gauge& g, float v, const SkPaint& p, float inner = 0.85f) {
    Pt d = g.dir(v);
    line(cv, g.x + d.x * g.r * inner, g.y + d.y * g.r * inner, g.x + d.x * g.r, g.y + d.y * g.r, p);
}

void indicator(SkCanvas& cv, const Gauge& g, float v, const SkPaint& p, float outer, bool half = false,
               float inner = 0.f) {
    Pt d = g.dir(v);
    float sx = g.x, sy = g.y;
    if (half) {
        sx = g.x + d.x * g.r * inner;
        sy = g.y + d.y * g.r * inner;
    }
    line(cv, sx, sy, g.x + d.x * g.r * outer, g.y + d.y * g.r * outer, p);
}

void donut(SkCanvas& cv, const Gauge& g, float v) {
    Pt d = g.dir(v);
    a380::chromium_stroke_circle(cv, g.x + d.x * g.r * 1.12f, g.y + d.y * g.r * 1.12f, 4.f, 2.f, stroke_paint(kCyan, 2.f));
}

struct Arinc {
    int ssm = 0;
    uint32_t bits = 0;
    bool normal() const { return ssm == 3; }
    bool valid() const { return ssm == 3 || ssm == 2; }
    float f32() const {
        float f;
        std::memcpy(&f, &bits, 4);
        return f;
    }
    bool bit(int n) const {
        if (!valid()) return false;
        float f = f32();
        if (!std::isfinite(f)) return false;
        return ((int32_t(int64_t(f)) >> (n - 1)) & 1) != 0;
    }
};

Arinc arinc(double raw) {
    Arinc a;
    double hi = std::floor(raw / 4294967296.0);
    a.ssm = int(hi);
    a.bits = uint32_t(uint64_t(raw - hi * 4294967296.0));
    return a;
}

std::string fmt1(double v, int& dec) {
    char buf[32];
    std::snprintf(buf, sizeof buf, "%.1f", v);
    std::string s = buf;
    size_t dot = s.find('.');
    dec = dot == std::string::npos ? 0 : int(s.size() - dot - 1);
    return s;
}

void split1(double v, std::string& a, std::string& b) {
    char buf[32];
    std::snprintf(buf, sizeof buf, "%.1f", v);
    std::string s = buf;
    size_t dot = s.find('.');
    a = s.substr(0, dot);
    b = dot == std::string::npos ? "" : s.substr(dot + 1);
}

bool lcd_mode(const SkCanvas& cv) {
    static const bool grey = a380::getenv_cached("EWD_GREY") != nullptr;
    return !grey && cv.getLocalToDeviceAs3x3().getScaleX() > 1.25f;
}

struct Ctx {
    SkCanvas& cv;
    const a380::Inputs& in;
    SkFont f26 = a380::font("Ecam", 26.f, false, lcd_mode(cv));
    SkFont f20 = a380::font("Ecam", 20.f, false, lcd_mode(cv));
    SkFont f30 = a380::font("Ecam", 30.f, false, lcd_mode(cv));
    SkFont f24 = a380::font("Ecam", 24.f, false, lcd_mode(cv));
    SkFont f23 = a380::font("Ecam", 23.f, false, lcd_mode(cv));
    SkFont f22 = a380::font("Ecam", 22.f, false, lcd_mode(cv));

    void text(const std::string& s, float x, float y, const SkFont& f, uint32_t c, Align al = Align::Left,
              float ls = 0.f, Baseline bl = Baseline::Alphabetic) {
        a380::draw_text(cv, s, x, y, f, fill_paint(c), al, bl, ls);
    }
    double v(const std::string& name, double fb = 0.0) const { return in.var(name, {}, fb); }
    double vi(const char* fmt, int n) const {
        char buf[160];
        std::snprintf(buf, sizeof buf, fmt, n);
        return in.var(buf, {}, 0.0);
    }
};

bool bus_bool(const a380::Inputs& in, const char* topic, bool fb) {
    const json* j = in.bus(topic);
    if (!j || !j->is_boolean()) return fb;
    return j->get<bool>();
}

const json* bus_data(const a380::Inputs& in, const char* topic) {
    return in.bus(topic);
}

int bus_int(const a380::Inputs& in, const char* topic, int fb) {
    const json* j = in.bus(topic);
    return j && j->is_number() ? int(j->get<double>()) : fb;
}

void draw_engines(Ctx& c) {
    SkCanvas& cv = c.cv;
    cv.save();
    cv.translate(8.f, 0.f);

    int st[4];
    bool anyRunning = false;
    for (int i = 0; i < 4; i++) {
        st[i] = int(c.vi("L:A32NX_ENGINE_STATE:%d", i + 1));
        if (st[i] != 0) anyRunning = true;
    }
    bool runOrIgn = anyRunning || int(c.v("L:XMLVAR_ENG_MODE_SEL")) == 2;
    bool revSel = c.v("L:A32NX_AUTOTHRUST_REVERSE:2") != 0 || c.v("L:A32NX_AUTOTHRUST_REVERSE:3") != 0;
    int athrMsg = int(c.v("L:A32NX_AUTOTHRUST_MODE_MESSAGE"));
    bool thrLock = athrMsg == 1;
    int athrMode = int(c.v("L:A32NX_AUTOTHRUST_MODE"));

    if (athrMode == 13 || athrMode == 14) c.text("A.FLOOR", 20, 30, c.f26, kAmber);
    if (thrLock) c.text("THR LK", 387, 30, c.f30, pulse_bright(c.in.now_ms()) ? kAmberBright : kAmberDim, Align::Right);

    int limType = int(c.v("L:A32NX_AUTOTHRUST_THRUST_LIMIT_TYPE"));
    double limN1 = c.v("L:A32NX_AUTOTHRUST_THRUST_LIMIT");
    double flex = c.v("L:A32NX_AIRLINER_TO_FLEX_TEMP");
    double idleLim = c.v("L:A32NX_AUTOTHRUST_THRUST_LIMIT_IDLE");
    double togaLim = c.v("L:A32NX_AUTOTHRUST_THRUST_LIMIT_TOGA");
    Arinc ags = arinc(c.v("L:A32NX_COND_CPIOM_B1_AGS_DISCRETE_WORD"));
    bool packs = ags.bit(13) || ags.bit(14);
    double maxLim = packs ? togaLim + 0.6 : togaLim;
    auto thrPct = [&](double n1, double off) {
        double d = maxLim - idleLim;
        double p = d != 0 ? std::max(0.0, (n1 - idleLim) / d) : 0.0;
        return std::min(1.0, p * (1 - off) + off) * 100.0;
    };

    if (!(revSel || thrLock)) {
        if (!runOrIgn) {
            c.text("XX", 312, 30, c.f26, kAmber, Align::Center);
        } else {
            static const char* names[] = {"", "CLB", "MCT", "FLX", "TOGA", "MREV"};
            if (limType >= 0 && limType < 6) c.text(names[limType], 330, 30, c.f30, kCyan, Align::Right);
            std::string a, b;
            split1(thrPct(limN1, 0.042), a, b);
            c.text(a, 399, 28, c.f26, kGreen, Align::Right, 1.6f);
            c.text(".", 416, 28, c.f26, kGreen, Align::Right);
            c.text(b, 431, 28, c.f20, kGreen, Align::Right);
            c.text("%", 447, 28, c.f20, kCyan, Align::Right);
            double sat = arinc(c.v("L:A32NX_ADIRS_ADR_1_STATIC_AIR_TEMPERATURE")).f32();
            if (flex != 0 && flex >= sat - 10 && limType == 3) {
                char buf[24];
                std::snprintf(buf, sizeof buf, "%d\xC2\xB0""C", int(std::lround(flex)));
                c.text(buf, 484, 30, c.f20, kCyan);
            }
        }
    }

    {
        bool onGround = c.v("L:A32NX_LGCIU_1_NOSE_GEAR_COMPRESSED") != 0 || c.v("L:A32NX_LGCIU_2_NOSE_GEAR_COMPRESSED") != 0;
        bool oneRunning = false;
        for (int i = 0; i < 4; i++) oneRunning |= st[i] == 1;
        bool bleedVis = !revSel && ((onGround && oneRunning) || (!onGround && limType >= 2));
        if (bleedVis) {
            std::string t;
            auto add = [&](const char* s) {
                if (!t.empty()) t += "/";
                t += s;
            };
            if (packs) add("PACKS");
            bool nai = false;
            for (int i = 1; i <= 4; i++) nai |= c.vi("A:ENG ANTI ICE:%d", i) != 0;
            if (nai) add("NAI");
            if (c.v("A:STRUCTURAL DEICE SWITCH") != 0) add("WAI");
            c.text(t, 750, 30, c.f26, kGreen, Align::Right);
        }
    }

    double n1Idle = c.v("L:A32NX_ENGINE_IDLE_N1");
    int phase = int(c.v("L:A32NX_FWC_FLIGHT_PHASE"));
    int athrStatus = int(c.v("L:A32NX_AUTOTHRUST_STATUS"));
    {
        int idleCount = 0;
        for (int i = 1; i <= 4; i++) idleCount += std::round(c.vi("L:A32NX_ENGINE_N1:%d", i)) <= std::round(n1Idle) + 2;
        bool idleVis = idleCount >= 3 && phase >= 6 && phase <= 9 && athrStatus != 0;
        double nowMs = c.in.now_ms();
        if (gState) {
            if (idleVis && !gState->idleVisible) gState->idleStartMs = nowMs;
            gState->idleVisible = idleVis;
        }
        if (idleVis) {
            double since = gState ? nowMs - gState->idleStartMs : 1e9;
            bool dim = since >= 0.0 && since < 10000.0 && std::fmod(since / 1000.0, 1.0) < 0.5;
            c.text("IDLE", 386, 90, c.f26, dim ? kGreenDim : kGreen, Align::Center);
        }
    }
    c.text("THR", 386, 121, c.f26, kWhite, Align::Center);
    c.text("%", 386, 142, c.f20, kCyan, Align::Center);
    c.text("N1", 386, 220, c.f26, kWhite, Align::Center);
    c.text("%", 386, 243, c.f20, kCyan, Align::Center);
    c.text("EGT", 384, 316, c.f26, kWhite, Align::Center);
    c.text("\xC2\xB0""C", 384, 339, c.f20, kCyan, Align::Center);

    static const float ex[4] = {93, 262, 497, 668};
    bool active[4], linked[4];
    for (int i = 0; i < 4; i++) {
        bool fire = c.vi("L:A32NX_FIRE_BUTTON_ENG%d", i + 1) != 0;
        bool sep = c.vi("L:A32NX_ENGINE_SEPARATED:%d", i + 1) != 0;
        active[i] = runOrIgn && !fire && !sep;
        linked[i] = runOrIgn && !sep;
    }
    {
        SkPaint lg = stroke_paint(kLight, 3.f, SkPaint::kRound_Cap);
        if (linked[0] && linked[1]) line(cv, 171, 228, 195, 226, lg);
        if (linked[1]) line(cv, 335, 216, 355, 214, lg);
        if (linked[2]) line(cv, 416, 216, 436, 218, lg);
        if (linked[2] && linked[3]) line(cv, 576, 226, 600, 228, lg);
    }

    double airspeed = 0.0;
    for (int k = 1; k <= 3; k++) {
        Arinc a = arinc(c.v("L:A32NX_ADIRS_ADR_" + std::to_string(k) + "_COMPUTED_AIRSPEED"));
        if (a.ssm != 0 || k == 3) {
            airspeed = a.valid() ? double(a.f32()) : 0.0;
            break;
        }
    }

    const json* normalAg = bus_data(c.in, "fws_normal_attention_getter_eng");
    const json* abnormalAg = bus_data(c.in, "fws_abnormal_primary_engine_parameters_attention_getter");
    auto agBit = [](const json* j, int i) { return j && j->is_array() && (int)j->size() > i && (*j)[i].is_boolean() && (*j)[i].get<bool>(); };
    for (int i = 0; i < 4; i++) {
        int e = i + 1;
        float x = ex[i], y = 126.f;
        bool agNormal = agBit(normalAg, i), agAbnormal = agBit(abnormalAg, i);
        if (active[i] && (agNormal || agAbnormal)) {
            SkPaint p = stroke_paint(agNormal ? kWhite : kAmber, 2.f);
            SkPathBuilder b;
            b.moveTo(x - 74, y - 13).lineTo(x - 74, y - 85).lineTo(x + 88, y - 85).lineTo(x + 88, y - 13);
            b.moveTo(x - 74, y + 168).lineTo(x - 74, y + 240).lineTo(x + 88, y + 240).lineTo(x + 88, y + 168);
            a380::draw_path_cached(cv, b.detach(), p);
        }
        double n1 = c.vi("L:A32NX_ENGINE_N1:%d", e);
        double tlaN1 = c.vi("L:A32NX_AUTOTHRUST_TLA_N1:%d", e);
        double n1Cmd = c.vi("L:A32NX_AUTOTHRUST_N1_COMMANDED:%d", e);
        double tla = c.vi("L:A32NX_AUTOTHRUST_TLA:%d", e);
        double egt = c.vi("L:A32NX_ENGINE_EGT:%d", e);
        if (egt == 0) egt = c.vi("L:A32NX_ENGINE_EGT_DISPLAY:%d", e);
        bool gp = int(c.v("L:A32NX_A380X_ENGINE_TYPE")) == 1;
        bool act = active[i];
        bool athr = athrStatus != 0;
        bool revDeploying = c.vi("L:A32NX_REVERSER_%d_DEPLOYING", e) != 0;
        bool revDeployed = c.vi("L:A32NX_REVERSER_%d_DEPLOYED", e) != 0;
        bool revSelected = c.vi("L:A32NX_AUTOTHRUST_REVERSE:%d", e) != 0;
        bool revVisible = revDeployed || revDeploying || revSelected;
        double revLim = c.v("L:A32NX_AUTOTHRUST_THRUST_LIMIT_REV");

        float tx = x + 2, ty = y - 3;
        Gauge th{tx, ty, 64, 230, 90, 0, 10};
        Gauge rg{tx, ty, 58, 130, 230, 0, 3};
        if (!act) {
            arc(cv, tx, ty + 20, 64, 320, 40, stroke_paint(kAmber, 2.f));
            c.text("THR XX", tx + 55, ty - 48, c.f26, kAmber, Align::Right, 1.6f);
        } else {
            double idleOff = st[i] == 1 ? 0.042 : 0.0;
            double thr = thrPct(n1, idleOff);
            double throttleTarget = (maxLim - idleLim) != 0 ? ((tlaN1 - idleLim) / (maxLim - idleLim)) * (1 - idleOff) + idleOff : 0;
            double autoTarget = (maxLim - idleLim) != 0 ? ((n1Cmd - idleLim) / (maxLim - idleLim)) * (1 - idleOff) + idleOff : 0;
            double revDen = -revLim - idleLim;
            double thrRev = revDen != 0 ? std::min(1.0, std::max(0.0, (n1 - idleLim) / revDen)) * 100.0 : 0.0;
            double throttleTargetRev = revDen != 0 ? (tlaN1 - idleLim) / revDen : 0.0;
            std::string a, b;
            split1(thr, a, b);
            bool avail = n1 > std::floor(idleLim) && st[i] == 2;
            double maxAvail = airspeed < 35 ? std::max(idleOff * 100, thrPct(76.5, idleOff)) / 10 : 10;
            auto availRev = [&]() {
                cv.drawRect(SkRect::MakeXYWH(tx - 18 - 28, ty - 14 - 13, 90, 24), fill_paint(0x000000));
                srect(cv, SkRect::MakeXYWH(tx - 18 - 28, ty - 14 - 13, 90, 24), 2.f, kGrey);
                if (avail) c.text("AVAIL", tx - 44, ty - 5, c.f26, kGreen, Align::Left, 1.6f);
                else c.text("REV " + std::to_string(e), tx - 43, ty - 5, c.f26, revDeployed ? kGreen : kAmber, Align::Left, 1.6f);
            };
            if (!revVisible) {
                c.text(a, tx + 48, ty + 47, c.f26, kGreen, Align::Right);
                c.text(".", tx + 62, ty + 47, c.f26, kGreen, Align::Right);
                c.text(b, tx + 78, ty + 47, c.f20, kGreen, Align::Right);
            }
            if (!revVisible && (avail || st[i] == 1)) {
                float sc = float(cv.getLocalToDeviceAs3x3().getScaleX());
                auto angOf = [](float v) { return 230.f + v / 10.f * 220.f; };
                float aI = angOf(0.42f), aM = angOf(float(maxAvail));
                Pt s = polar(tx, ty, 64, aI), e2 = polar(tx, ty, 64, aM);
                float sx = round2(s.x), sy = round2(s.y), ex2 = round2(e2.x), ey2 = round2(e2.y);
                ArcSeg as = svg_arc(sx, sy, ex2, ey2, 64.f, !(std::fabs(aM - aI) < 180.f), true);
                SkPath arcPath = a380::chromium_arc_path(as.cx, as.cy, 64.f, as.theta, as.sweep, sc);
                SkPathBuilder pb;
                pb.moveTo(tx, ty);
                pb.lineTo(sx, sy);
                pb.addPath(arcPath, SkPath::kExtend_AddPathMode);
                pb.close();
                a380::chromium_fill_path(cv, pb.detach(), fill_paint(kGrey));
            }
            if (!revVisible && avail) availRev();
            if (!revVisible && athr) {
                for (float k : {0.8f, 0.65f, 0.5f, 0.35f}) {
                    float aA = 230.f + float(thr / 10) / 10.f * 220.f, aT = 230.f + float(autoTarget * 10) / 10.f * 220.f;
                    float d = aT - aA;
                    if (std::fabs(d) < 1e-6f) continue;
                    Pt s = polar(tx, ty, 64 * k, aA), e2 = polar(tx, ty, 64 * k, aT);
                    ArcSeg as = svg_arc(s.x, s.y, e2.x, e2.y, 64 * k, std::fabs(d) > 180.f, d >= 0.f);
                    a380::chromium_stroke_arc(cv, as.cx, as.cy, 64 * k, as.theta, as.sweep, stroke_paint(kGreen, 1.f, SkPaint::kRound_Cap));
                }
                Pt t2 = th.dir(float(autoTarget * 10));
                line(cv, tx, ty, tx + t2.x * 0.8f * 64, ty + t2.y * 0.8f * 64, stroke_paint(kGreen, 1.f, SkPaint::kRound_Cap));
            }
            SkPaint wp = stroke_paint(kWhite, 2.f);
            arc(cv, tx, ty, 64, 230, 90, wp);
            SkPaint tp = stroke_paint(kWhite, 2.f, SkPaint::kRound_Cap);
            struct M {
                float v, nx, ny;
                bool show;
            } ms[] = {{0, 10, -2, true}, {2.5f, 13, 6, false}, {5, 5, 15, true}, {7.5f, 0, 0, false}, {10, -27, 7, true}};
            SkPaint tl = stroke_paint(kWhite, 2.f, SkPaint::kSquare_Cap);
            for (auto& m : ms) {
                marker(cv, th, m.v, tl, 0.9f);
                if (m.show) {
                    Pt d = th.dir(m.v);
                    char buf[8];
                    std::snprintf(buf, sizeof buf, "%d", int(std::lround(m.v)));
                    c.text(buf, tx + d.x * 64 * 0.9f + m.nx, ty + d.y * 64 * 0.9f + m.ny, c.f20, kWhite, Align::Left);
                }
            }
            if (!revVisible) {
                srect(cv, SkRect::MakeXYWH(tx - 11, ty + 21, 96, 30), 2.f, kGrey);
                indicator(cv, th, float(thr / 10), stroke_paint(kGreen, 3.f, SkPaint::kRound_Cap), 1.1f);
            }
            if (!revSelected) donut(cv, th, float(throttleTarget * 10));
            else donut(cv, rg, float(3.0 - throttleTargetRev * 3.0));
            if (revVisible) {
                if (revDeploying || st[i] == 1) {
                    float sc = float(cv.getLocalToDeviceAs3x3().getScaleX());
                    auto angOf = [](float v) { return 130.f + v / 3.f * 100.f; };
                    float aI = angOf(0.04f), aM = angOf(2.6f);
                    Pt s = polar(tx, ty, 58, aI), e2 = polar(tx, ty, 58, aM);
                    float sx = round2(s.x), sy = round2(s.y), ex2 = round2(e2.x), ey2 = round2(e2.y);
                    ArcSeg as = svg_arc(sx, sy, ex2, ey2, 58.f, false, true);
                    SkPath arcPath = a380::chromium_arc_path(as.cx, as.cy, 58.f, as.theta, as.sweep, sc);
                    SkPathBuilder pb;
                    pb.moveTo(tx, ty);
                    pb.lineTo(sx, sy);
                    pb.addPath(arcPath, SkPath::kExtend_AddPathMode);
                    pb.close();
                    a380::chromium_fill_path(cv, pb.detach(), fill_paint(kGrey));
                }
                arc(cv, tx, ty, 58, 130, 230, wp);
                if (avail || revDeployed || revDeploying) availRev();
                SkPaint rl = stroke_paint(kWhite, 2.f, SkPaint::kSquare_Cap);
                marker(cv, rg, 0, rl, 1.1f);
                Pt d0 = rg.dir(0);
                c.text("MAX", tx + d0.x * 58 * 1.1f - 24, ty + d0.y * 58 * 1.1f - 6, c.f20, kWhite, Align::Left);
                marker(cv, rg, 1.5f, rl, 1.1f);
                indicator(cv, rg, float(3.0 - thrRev * 0.03), stroke_paint(kGreen, 3.f, SkPaint::kRound_Cap), 1.1f);
            }
        }

        float nx = x, ny = y + 65;
        Gauge ng{nx, ny, 64, 230, 90, 2, 11.1f};
        if (!act) {
            arc(cv, nx, ny, 64, 230, 90, stroke_paint(kWhite, 2.f));
            c.text("XX", nx + 48, ny + 45, c.f26, kAmber, Align::Right, 1.6f);
        } else {
            std::string a, b;
            split1(n1, a, b);
            c.text(a, nx + 6, ny + 45, c.f26, kGreen, Align::Right);
            c.text(".", nx + 20, ny + 45, c.f26, kGreen, Align::Right);
            c.text(b, nx + 36, ny + 45, c.f20, kGreen, Align::Right);
        }

        float gx = x + 4, gy = y + 212;
        Gauge eg{gx, gy, 68, 270, 90, 0, gp ? 1100.f : 1000.f};
        if (!act) {
            arc(cv, gx, gy, 68, 270, 90, stroke_paint(kWhite, 2.f));
            c.text("XX", gx + 17, gy + 11.7f, c.f26, kAmber, Align::Right);
        } else {
            uint32_t col = egt >= (gp ? 1002 : 900) ? kRed : (egt > (gp ? 970 : 850) && tla < 3) ? kAmber : kGreen;
            bool tlaToga = tla == 3.0 || tla == 4.0;
            double egtCap = gp ? (tlaToga ? 1002.0 : 970.0) : (tlaToga ? 900.0 : 850.0);
            double shown = std::min(egtCap, std::round(egt));
            char buf[16];
            std::snprintf(buf, sizeof buf, "%d", int(shown));
            c.text(buf, gx + 33, gy + 11.7f, c.f23, col, Align::Right);
            arc(cv, gx, gy, 68, 270, 90, stroke_paint(kWhite, 2.f));
            arc(cv, gx, gy, 66, 90 - 20, 90, stroke_paint(kRed, 6.f));
            SkPaint tl = stroke_paint(kWhite, 2.f, SkPaint::kSquare_Cap);
            marker(cv, eg, 0, tl);
            marker(cv, eg, 500, tl);
            marker(cv, eg, eg.mx, stroke_paint(kRed, 2.f, SkPaint::kSquare_Cap));
            if (tla < 33) {
                Pt d = eg.dir(gp ? 970.f : 850.f);
                SkPathBuilder pb;
                float ang = std::atan2(d.y, d.x);
                float px = gx + d.x * 68, py = gy + d.y * 68;
                float ca = std::cos(ang), sa = std::sin(ang);
                auto rot = [&](float lx, float ly) { return Pt{px + lx * ca - ly * sa, py + lx * sa + ly * ca}; };
                Pt p0 = rot(-6, 0), p1 = rot(6, 0), p2 = rot(6, 4), p3 = rot(-2, 4);
                pb.moveTo(p0.x, p0.y);
                pb.lineTo(p1.x, p1.y);
                pb.lineTo(p2.x, p2.y);
                pb.lineTo(p3.x, p3.y);
                pb.close();
                SkPaint fp = fill_paint(kAmber);
                a380::chromium_fill_path(cv, pb.snapshot(), fp);
                a380::chromium_stroke_path(cv, pb.detach(), stroke_paint(kAmber, 3.f));
            }
            srect(cv, SkRect::MakeXYWH(gx - 36, gy - 11, 72, 26), 2.f, kGrey);
            double trimmed = std::min(egtCap, egt);
            uint32_t ic = col;
            indicator(cv, eg, float(trimmed), stroke_paint(ic, 3.f, SkPaint::kRound_Cap), 1.15f, true, 0.75f);
        }
    }
    cv.restore();
}

struct Seg {
    std::string text;
    uint32_t color;
    bool underline, frame;
    int startCol, cols;
};

uint32_t color_of(const std::string& code) {
    if (code == "<1m") return kYellow;
    if (code == "<2m") return kRed;
    if (code == "<3m") return kGreen;
    if (code == "<4m") return kAmber;
    if (code == "<5m") return kCyan;
    if (code == "<6m") return kMagenta;
    return kWhite;
}

void fwc_text(Ctx& c, const std::string& msg, float x, float y) {
    const float decoDy = std::round((y - 24.f) * 1.5f) / 1.5f - (y - 24.f);
    uint32_t color = kWhite;
    bool underlined = false, framed = false;
    std::vector<Seg> segs;
    std::string buf;
    int startCol = 0, col = 0;
    float yOff = 0;
    auto flush_line = [&]() {
        float cx = x;
        const float segGap = 0.f;
        for (auto& s : segs) {
            c.text(s.text, cx, y + yOff, c.f24, s.color, Align::Left, 1.75f);
            cx += a380::text_width(c.f24, s.text, 1.75f) + segGap;
            float dx = x + s.startCol * 16.f;
            if (s.underline)
                line(c.cv, dx, y + yOff + 4 + decoDy, dx + s.cols * 16 + 5, y + yOff + 4 + decoDy, stroke_paint(s.color, 2.f));
            if (s.frame) {
                float w = s.cols * 16 + 12;
                float fy = y + yOff + decoDy;
                SkPathBuilder pb;
                pb.moveTo(dx, fy - 22);
                pb.lineTo(dx + w, fy - 22);
                pb.lineTo(dx + w, fy + 5);
                pb.lineTo(dx, fy + 5);
                pb.lineTo(dx, fy - 22);
                a380::draw_path_cached(c.cv, pb.detach(), stroke_paint(s.color, 2.f));
            }
        }
        segs.clear();
        yOff += 30;
        col = 0;
        startCol = 0;
    };
    auto close_part = [&]() {
        if (!buf.empty()) {
            segs.push_back({buf, color, underlined, framed, startCol, col - startCol});
            buf.clear();
            startCol = col;
        }
    };
    size_t n = msg.size();
    for (size_t i = 0; i < n; i++) {
        char ch = msg[i];
        if (ch == '\x1b' || ch == '\r') {
            close_part();
            if (ch == '\x1b') {
                std::string ctrl;
                for (i++; i < n; i++) {
                    ctrl += msg[i];
                    bool match = true;
                    if (ctrl == "m") underlined = framed = false;
                    else if (ctrl == "4m") underlined = true;
                    else if (ctrl == ")m") {
                    } else if (ctrl == "'m") framed = true;
                    else if (ctrl.size() == 3 && ctrl[0] == '<' && ctrl[2] == 'm') color = color_of(ctrl);
                    else match = false;
                    if (match) break;
                }
                continue;
            }
            flush_line();
            continue;
        }
        unsigned char u = (unsigned char)ch;
        buf += ch;
        if ((u & 0xC0) != 0x80) col++;
    }
    close_part();
    if (!segs.empty()) flush_line();
}

struct WdLine {
    bool abnormal = false, active = false, sensed = false, checked = false;
    std::string text;
    std::string style = "Standard";
    bool first = false, last = false, inactive = false;
    int origIdx = INT32_MIN;
    bool emptySpecial = false;
};

std::string nbsp(int n) {
    std::string s;
    for (int i = 0; i < n; i++) s += "\xC2\xA0";
    return s;
}

size_t u8len(const std::string& s) {
    size_t n = 0;
    for (unsigned char c : s) n += (c & 0xC0) != 0x80;
    return n;
}

std::string u8sub(const std::string& s, size_t count) {
    size_t n = 0, i = 0;
    for (; i < s.size(); i++) {
        if (((unsigned char)s[i] & 0xC0) != 0x80) {
            if (n == count) break;
            n++;
        }
    }
    return s.substr(0, i);
}

double elapsed_s(const std::string& key, double ts) {
    if (!gState) return 0.0;
    auto it = gState->timers.find(key);
    if (it == gState->timers.end() || it->second.ts != ts) it = gState->timers.insert_or_assign(key, TimerSeen{ts, gState->nowMs}).first;
    return (gState->nowMs - it->second.seenMs) / 1000.0;
}

bool timed_text(const json& proc, size_t idx, const json& item, const std::string& pid, bool cond, std::string& out) {
    auto ts = proc.find("itemsTimeStamp");
    if (ts == proc.end() || !ts->is_array()) return false;
    int time = item.value("time", 0);
    int seconds = time;
    if (idx < ts->size() && (*ts)[idx].is_number()) {
        int diff = int(std::floor(elapsed_s(pid + "#" + std::to_string(idx), (*ts)[idx].get<double>())));
        if (diff < time) seconds = time - diff;
        else if (cond && item.value("appendTimeIfElapsed", false)) seconds = time;
        else return false;
    }
    out = " AFTER " + std::to_string(seconds) + " S :";
    return true;
}

std::string action_text(const json& item, bool done, const json& proc, size_t idx, const std::string& pid) {
    int level = item.value("level", 0);
    std::string name = item.value("name", "");
    std::string text = nbsp(level) + "-" + name;
    std::string lnc = item.value("labelNotCompleted", "");
    std::string lc = item.value("labelCompleted", "");
    bool colon = item.value("colonIfCompleted", true);
    if (done && !lc.empty()) text += (colon ? " : " : " ") + lc;
    else if (done && !lnc.empty()) text += (colon ? " : " : " ") + lnc;
    else if (!done && !lnc.empty()) {
        if (item.contains("time")) {
            std::string t;
            if (timed_text(proc, idx, item, pid, false, t)) text += t;
        }
        int pad = std::max<int>(0, 40 - int(u8len(lnc) + u8len(text) + level + 2));
        text += " " + std::string(pad, '.') + lnc;
    }
    return text;
}

std::string condition_text(const json& item, bool done, const json& proc, size_t idx, const std::string& pid) {
    int level = item.value("level", 0);
    std::string name = item.value("name", "");
    std::string text = nbsp(level);
    bool dc = !name.empty() && name.back() == ':';
    if (name.compare(0, 4, "WHEN") == 0) {
        text += "." + name + " " + (dc ? "" : ":");
    } else {
        std::string rest = name.compare(0, 2, "IF") == 0 ? name.substr(2) : name;
        bool timedItem = item.contains("time");
        std::string timed;
        bool hasTimed = timedItem && timed_text(proc, idx, item, pid, true, timed);
        bool as = done || (timedItem && !hasTimed);
        text += std::string(as ? ".AS " : ".IF ") + rest + " " + (hasTimed ? timed : std::string(dc ? " " : " :"));
    }
    return text;
}

enum class ProcType { Abnormal, Fallback, Normal, Deferred };

void proc_lines(ProcType type, const std::string& pid, const json& def, const json& proc, bool isActive, bool isLast,
                std::vector<WdLine>& out) {
    const bool abnormalOrDeferred = type != ProcType::Normal;
    const bool isAbnormal = type == ProcType::Abnormal || type == ProcType::Fallback;
    const bool isDeferred = type == ProcType::Deferred;
    const bool fallback = type == ProcType::Fallback;
    const bool notSensed = type == ProcType::Abnormal && def.value("sensed", true) == false;
    const bool activated = jflag(proc, "procedureActivated");
    const bool completed = jflag(proc, "procedureCompleted");
    static const json kNoItems = json::array();
    const json& items = def.contains("items") ? def["items"] : kNoItems;

    WdLine h;
    h.abnormal = abnormalOrDeferred;
    h.active = isActive;
    h.sensed = true;
    h.style = "Headline";
    const std::string title = def.value("title", "UNDEFINED");
    if (isDeferred) {
        h.text = std::string(completed ? "\x1b<7m> " : "\x1b<4m> ") + title;
    } else {
        h.text = title;
        h.first = true;
        h.last = !isActive;
    }
    out.push_back(h);

    if (!isAbnormal || isActive || fallback) {
        std::string rec = type == ProcType::Abnormal ? def.value("recommendation", "") : "";
        if (!rec.empty()) {
            WdLine r;
            r.abnormal = true;
            r.active = isActive;
            r.sensed = true;
            r.text = rec;
            r.style = rec == "LAND ASAP" ? "LandAsap" : "LandAnsa";
            out.push_back(r);
        }
        if ((isDeferred && !completed) || notSensed) {
            WdLine a;
            a.abnormal = abnormalOrDeferred;
            a.active = isActive;
            a.text = nbsp(31) + "ACTIVATE";
            a.style = "ChecklistItem";
            a.checked = activated;
            a.origIdx = -2;
            out.push_back(a);
        }
        for (size_t i = 0; i < items.size(); i++) {
            if ((isAbnormal && !jarr(proc, "itemsToShow", i)) || (isDeferred && completed)) continue;
            const json& it = items[i];
            std::string st = it.value("style", "ChecklistItem");
            std::string text = it.value("level", 0) && st != "CenteredSubHeadline" ? nbsp(it.value("level", 0)) : "";
            bool inactive = !jarr(proc, "itemsActive", i) || !activated;
            bool done = jarr(proc, "itemsChecked", i);
            bool isCond = false;
            if (it.contains("labelNotCompleted")) {
                text = action_text(it, done, proc, i, pid);
                if (inactive && st == "ChecklistItem") st = "ChecklistItemInactive";
            } else if (it.contains("condition")) {
                isCond = true;
                st = "ChecklistCondition";
                text = condition_text(it, done, proc, i, pid);
            } else if (st == "CenteredSubHeadline") {
                text += ".";
                text += it.value("name", "");
            } else {
                text += "\xC2\xA0" + it.value("name", "");
            }
            bool itemSensed = it.value("sensed", false);
            WdLine l;
            l.abnormal = abnormalOrDeferred;
            l.active = isActive;
            l.sensed = isCond ? true : itemSensed;
            l.checked = done;
            l.text = u8sub(text, 40);
            l.style = st;
            l.first = l.last = (!isActive && isAbnormal) || fallback;
            l.origIdx = (!isCond || itemSensed) ? int(i) : INT32_MIN;
            l.inactive = inactive;
            l.emptySpecial = st == "Empty";
            out.push_back(l);
            if (isCond && !itemSensed) {
                std::string nm = it.value("name", "");
                if (!nm.empty() && nm.back() == ':') nm.pop_back();
                nm = nm.compare(0, 2, "IF") == 0 ? nm.substr(2) : nm.compare(0, 4, "WHEN") == 0 ? nm.substr(4) : nm;
                WdLine cf = l;
                cf.text = nbsp(it.value("level", 0)) + "CONFIRM " + nm;
                cf.sensed = false;
                cf.first = cf.last = !isActive && isAbnormal;
                cf.origIdx = int(i);
                cf.emptySpecial = false;
                out.push_back(cf);
            }
        }
        if (isAbnormal && !fallback) {
            WdLine cl;
            cl.abnormal = true;
            cl.active = isActive;
            cl.text = nbsp(34) + "CLEAR";
            cl.style = "ChecklistItem";
            cl.last = isLast;
            cl.origIdx = -7;
            out.push_back(cl);
        } else if (type == ProcType::Normal) {
            WdLine cc;
            cc.active = isActive;
            cc.checked = completed;
            cc.text = nbsp(27) + "C/L COMPLETE";
            cc.style = "ChecklistItem";
            cc.origIdx = -5;
            out.push_back(cc);
            WdLine rs;
            rs.active = isActive;
            rs.text = nbsp(34) + "RESET";
            rs.style = "ChecklistItem";
            rs.last = true;
            rs.origIdx = -6;
            out.push_back(rs);
        } else if (isDeferred) {
            WdLine df;
            df.active = isActive;
            df.style = "ChecklistItem";
            df.inactive = !activated;
            if (completed) {
                df.text = nbsp(19) + "DEFERRED PROC RECALL";
                df.origIdx = -4;
            } else {
                df.text = nbsp(17) + "DEFERRED PROC COMPLETE";
                df.checked = completed;
                df.origIdx = -3;
            }
            out.push_back(df);
        }
    } else if (!items.empty()) {
        WdLine d;
        d.abnormal = abnormalOrDeferred;
        d.sensed = true;
        d.text = "...";
        d.style = "OmissionDots";
        d.first = d.last = true;
        out.push_back(d);
    }
    WdLine e;
    e.abnormal = abnormalOrDeferred;
    e.active = isActive || isDeferred;
    e.sensed = true;
    e.style = "ChecklistItem";
    e.first = e.last = !isDeferred;
    out.push_back(e);
}

uint32_t line_color(const WdLine& d) {
    const std::string& s = d.style;
    if (d.inactive || s == "ChecklistItemInactive" || s == "CompletedChecklist") return kGrey;
    if (d.abnormal && d.checked && (s == "Green" || s == "GreenTable" || s == "Cyan" || s == "Amber" || s == "Red")) return kWhite;
    if (s == "Headline" || s == "SubHeadline" || s == "CenteredSubHeadline") return kWhite;
    if (s == "Green" || (s == "GreenTable" && d.active && !d.inactive)) return kGreen;
    if (s == "Cyan") return kCyan;
    if (s == "Amber" || s == "LandAnsa" || s == "DeferredProcedure") return kAmber;
    if (s == "CompletedDeferredProcedure") return kWhite;
    if (s == "White" || s == "ChecklistCondition" || s == "OmissionDots") return kWhite;
    if (s == "Red" || s == "LandAsap") return kRed;
    if (d.abnormal) return d.checked ? kWhite : kCyan;
    return d.checked ? kGreen : kCyan;
}

constexpr float kMarkerH = 34.f;

void draw_line(Ctx& c, const WdLine& d, int idx, bool selected) {
    SkCanvas& cv = c.cv;
    float top = 391.f + kTopDy + kRowOff[idx];
    float kLineH = kRowH[idx];
    float x0 = 48.f;
    uint32_t col = line_color(d);
    SkFont& f = c.f26;
    SkFontMetrics m;
    f.getMetrics(&m);
    float A = std::round(-m.fAscent), D = std::round(m.fDescent);
    float base = top + kTextDy + 3 + (kLineH - 6 - (A + D)) * 0.5f + A + 2.f + (kLineH < 32.f ? 0.4f : 0.f);

    if (kChromiumBase || kAllBase) base = std::round(top * kDpr + (kLineH * kDpr - kLineDev) * 0.5f + kAscDev + kBaseGap) / kDpr;
    bool hidesBox = d.style == "Headline" || d.style == "GreenTable" || d.style == "OmissionDots" || d.style == "LandAnsa" ||
                    d.style == "LandAsap";
    bool dotsEmpty = d.emptySpecial || (d.first && d.last && d.text.empty());
    float tDev = std::round(top * kDpr), bDev = std::round((top + kLineH) * kDpr);
    auto box = [&](float x0d, float y0d, float x1d, float y1d, uint32_t colr) {
        cv.drawRect(SkRect::MakeLTRB(x0d / kDpr, y0d / kDpr, x1d / kDpr, y1d / kDpr), fill_paint(colr));
    };
    if (selected) {
        float l = std::round(x0 * kDpr), r = 1103.5f;
        box(l, tDev, r, tDev + 4.f, kCyan);
        box(l, bDev - 4.f, r, bDev, kCyan);
        box(l, tDev, l + 4.f, bDev, kCyan);
    }
    bool markerVisible = !(d.first && d.last) && !d.emptySpecial;
    if (markerVisible) {
        float l = std::round(732.f * kDpr), r = std::round(760.f * kDpr);
        float mDev = std::round((top + kMarkerH) * kDpr);
        if (!selected) box(l, tDev, l + 4.f, mDev, kWhite);
        box(r - 4.f, tDev, r, mDev, kWhite);
        if (d.first) box(l, tDev, r, tDev + 4.f, kWhite);
        if (d.last) box(l, mDev - 4.f, r, mDev, kWhite);
    }
    (void)dotsEmpty;
    if (d.abnormal && d.style == "Headline") {
        float svgTop = top + kTextDy + 3 + (kLineH - 6 - 33.f) * 0.5f - kSvgDy;
        fwc_text(c, d.text, x0 + kEclBorder + 10, svgTop + 24);
        return;
    }
    if (!hidesBox && !d.emptySpecial) {
        bool invisible = d.sensed || (d.first && d.last);
        if (!invisible) {
            uint32_t cc = line_color(d);
            c.text(d.checked ? "\xE2\x96\xA0" : "\xE2\x96\xA1", x0 + kEclBorder + 12, base, f, cc, Align::Center);
        }
    }
    const std::string shown = u8sub(d.text, 39);
    float tx = hidesBox ? x0 + kEclBorder : x0 + kEclBorder + 24 + 20;
    if (d.style == "SubHeadline" || d.style == "CenteredSubHeadline") {
        float w = a380::text_width(f, shown);
        line(cv, tx, top + kLineH - 4.5f, tx + w, top + kLineH - 4.5f, stroke_paint(col, 2.f));
    }
    if (d.style == "LandAnsa" || d.style == "LandAsap") {
        c.text(shown, 737.f - kEclBorder, base, f, col, Align::Right);
        return;
    }
    if (d.style == "Headline") {
        c.text(shown, tx, base, f, col);
        float w = a380::text_width(f, shown);
        float bd = std::round(base * kDpr);
        a380::chromium_fill_rect(cv, SkRect::MakeLTRB(tx, (bd + 2.f) / kDpr, tx + w, (bd + 5.f) / kDpr), fill_paint(col));
        return;
    }
    if (d.style == "OmissionDots") {
        c.text(shown, tx, base, f, kWhite);
        return;
    }
    c.text(shown, tx, base, f, col);
}

float flex_pitch(const std::vector<WdLine>& lines, int showFrom, bool stsArea) {
    const float kEmpty = 29.667f, kMarker = 3.5136f, kSvg = 6.f, kBorder = 3.f, kMenuBorder = 1.0f, kLand = 7.9f, kTail = 6.2f;
    float mins[kNumLines];
    int n = 0;
    bool tail = false;
    for (int i = showFrom; i < int(lines.size()) && n < kNumLines; i++, n++) {
        const WdLine& l = lines[i];
        tail |= l.abnormal && l.first && l.last && l.style == "ChecklistItem";
        bool markerHidden = l.first && l.last;
        float c = kEmpty;
        if (l.abnormal && l.style == "Headline") c += kSvg;
        else if (!markerHidden) c += kMarker;
        if (!markerHidden) c += (l.abnormal && l.style == "Headline" ? kBorder : kMenuBorder) * ((l.first ? 1 : 0) + (l.last ? 1 : 0));
        if (l.style == "LandAsap" || l.style == "LandAnsa") c += kLand;
        mins[n] = c;
    }
    for (int i = n; i < kNumLines; i++) mins[i] = kEmpty;
    if (tail)
        for (int i = 0; i < kNumLines; i++) mins[i] += kTail / kNumLines;
    float sumMin = 0.f;
    for (int i = 0; i < kNumLines; i++) {
        sumMin += mins[i];
    }
    if (a380::getenv_cached("EWD_DBG")) {
        for (int i = showFrom; i < int(lines.size()) && i < showFrom + kNumLines; i++)
            std::fprintf(stderr, "L %s|%d%d%d|%s ", lines[i].style.c_str(), lines[i].first, lines[i].last, lines[i].abnormal, lines[i].text.substr(0, 8).c_str());
        std::fprintf(stderr, "total %d from %d tail %d \n", int(lines.size()), showFrom, int(tail));
        std::fprintf(stderr, "pitch %f \n", sumMin / kNumLines);
    }
    if (kAbnFit) {
        float nM = 0.f, nH = 0.f, nL = 0.f;
        for (int i = showFrom, k = 0; i < int(lines.size()) && k < kNumLines; i++, k++) {
            const WdLine& l = lines[i];
            if (!(l.first && l.last)) nM += 1.f;
            if (l.abnormal && l.style == "Headline") nH += 1.f;
            if (l.style == "LandAsap" || l.style == "LandAnsa") nL += 1.f;
        }
        float h = kInt + 5.38f * nM + 9.66f * nH - 0.14f * nL;
        if (a380::getenv_cached("EWD_DBG")) std::fprintf(stderr, "abn nM %g nH %g nL %g h %f pitch %f\n", nM, nH, nL, h, h / kNumLines);
        float pitch = h / kNumLines;
        for (int i = 0; i < kNumLines; i++) {
            kRowOff[i] = i * pitch;
            kRowH[i] = pitch;
        }
        kStsDy = h - 575.07f;
        return pitch;
    }
    const float avail = 574.656f;
    (void)stsArea;
    float height = stsArea ? std::min(sumMin, 578.44f) : sumMin;
    float pitch = height / kNumLines;
    for (int i = 0; i < kNumLines; i++) {
        kRowOff[i] = i * pitch;
        kRowH[i] = pitch;
    }
    kStsDy = height - avail;
    return height / kNumLines;
}

void draw_wd(Ctx& c) {
    SkCanvas& cv = c.cv;
    kDpr = cv.getLocalToDeviceAs3x3().getScaleX();
    kEclBorder = std::floor(3.f * kDpr) / kDpr;
    kLineDev = kDpr > 1.25f ? 35.f : 23.f;
    kAscDev = kDpr > 1.25f ? 31.f : 21.f;
    const a380::Inputs& in = c.in;
    bool fwsHealthy1 = c.v("L:A32NX_FWS1_IS_HEALTHY", 1) != 0, fwsHealthy2 = c.v("L:A32NX_FWS2_IS_HEALTHY", 1) != 0;
    bool fwsAvail = (fwsHealthy1 && (c.v("L:A32NX_AFDX_3_3_REACHABLE", 1) != 0 || c.v("L:A32NX_AFDX_13_13_REACHABLE", 1) != 0)) ||
                    (fwsHealthy2 && (c.v("L:A32NX_AFDX_4_3_REACHABLE", 1) != 0 || c.v("L:A32NX_AFDX_14_13_REACHABLE", 1) != 0));
    bool showNormal = bus_bool(in, "fws_show_normal_checklists", false) && fwsAvail;
    bool showAbn = bus_bool(in, "fws_show_abn_sensed", false);
    bool showNonSensed = bus_bool(in, "fws_show_abn_non_sensed", false) && fwsAvail;
    bool memosVisible = !(bus_bool(in, "fws_show_normal_checklists", false) || showAbn || bus_bool(in, "fws_show_abn_non_sensed", false) || !fwsAvail);
    auto cpiomFailed = [&](char letter, std::initializer_list<int> idx) {
        for (int k : idx)
            if (c.v(std::string("L:A32NX_CPIOM_") + letter + std::to_string(k) + "_AVAIL", 1.0) != 0) return false;
        return true;
    };
    bool cpiomCFailed = cpiomFailed('C', {1, 2});
    bool cpiomOtherFailed = cpiomFailed('A', {1, 2, 3, 4}) || cpiomFailed('B', {1, 2, 3, 4}) || cpiomFailed('D', {1, 3}) ||
                            cpiomFailed('E', {1, 2}) || cpiomFailed('F', {1, 2, 3, 4}) || cpiomFailed('G', {1, 2, 3, 4});
    bool cpiomFallback = cpiomCFailed && cpiomOtherFailed;
    bool stsArea = !(((showAbn || !fwsAvail) && !cpiomFallback) || showNonSensed);

    if (memosVisible) {
        float y = kWdTop + 12.f;
        SkFont& f24 = c.f24;
        SkFontMetrics m;
        f24.getMetrics(&m);
        float lh24 = std::round(-m.fAscent) + std::round(m.fDescent);
        const json* la = bus_data(in, "fws_limitations_all_phases");
        const json* lr = bus_data(in, "fws_limitations_appr_ldg");
        size_t nl = la && la->is_array() ? la->size() : 0, nr = lr && lr->is_array() ? lr->size() : 0;
        const json& lim = ecam()["EcamLimitations"];
        auto join = [&](const json* arr, const json& table) {
            std::string s;
            if (!arr || !arr->is_array()) return s;
            bool first = true;
            for (auto& id : *arr) {
                std::string k = id.is_string() ? id.get<std::string>() : std::to_string(id.get<long long>());
                if (!first) s += "\r";
                first = false;
                if (table.contains(k)) s += table[k].get<std::string>();
            }
            return s;
        };
        if (std::max(nl, nr) > 0) {
            size_t lines = std::max(nl, nr);
            c.text("LIMITATIONS", 8 + 376.f, y + std::round(-m.fAscent), f24, kWhite, Align::Center, 1.75f);
            float hw = a380::text_width(f24, "LIMITATIONS", 1.75f);
            line(cv, 8 + 376.f - hw * 0.5f, y + std::round(-m.fAscent) + 3, 8 + 376.f + hw * 0.5f, y + std::round(-m.fAscent) + 3, stroke_paint(kWhite, 1.f));
            float ry = y + lh24 + 4;
            c.text("ALL PHASES", 8 + 188.f, ry + std::round(-m.fAscent), f24, kWhite, Align::Center, 1.75f);
            c.text("APPR & LDG", 8 + 376.f + 8 + 184.f, ry + std::round(-m.fAscent), f24, kWhite, Align::Center, 1.75f);
            float sy = ry + lh24 + 4;
            fwc_text(c, join(la, lim), 8.f, sy + 24);
            fwc_text(c, join(lr, lim), 8.f + 376.f + 8.f, sy + 24);
            float colH = lh24 + 4 + lines * 30 + 3 + 4;
            line(cv, 8 + 375.f, ry, 8 + 375.f, ry + colH, stroke_paint(0xd3d3d3, 2.f));
            float bottom = ry + colH + 12;
            line(cv, 8, bottom + 1, 760, bottom + 1, stroke_paint(kLight, 2.f));
            y = bottom + 2 + 12;
        }
        const json& memos = ecam()["EcamMemos"];
        auto memo_str = [&](const char* fmt, std::vector<int>& count) {
            std::string s;
            bool first = true;
            for (int i = 1; i <= 10; i++) {
                char nm[64];
                std::snprintf(nm, sizeof nm, fmt, i);
                long long v = (long long)c.v(nm);
                if (!v) continue;
                char k[16];
                std::snprintf(k, sizeof k, "%09lld", v);
                if (!first) s += "\r";
                first = false;
                if (memos.contains(k)) s += memos[k].get<std::string>();
                count.push_back(i);
            }
            return s;
        };
        std::vector<int> cl, cr;
        std::string ml = memo_str("L:A32NX_EWD_LOWER_LEFT_LINE_%d", cl), mr = memo_str("L:A32NX_EWD_LOWER_RIGHT_LINE_%d", cr);
        fwc_text(c, ml, 8.f, y + 24);
        fwc_text(c, mr, 8.f + 376.f + 8.f, y + 24);
        float h = std::max(cl.size(), cr.size()) * 30.f + 3.f;
        if (h > 3.f) line(cv, 8 + 375.f, y, 8 + 375.f, y + h + 4, stroke_paint(0xd3d3d3, 2.f));
    }

    kLineH = 32.89f;
    kAllBase = true;
    kSvgDy = 0.f;
    kBaseGap = 2.f;
    kStsDy = 0.f;
    kTopDy = kTextDy = 0.f;
    kChromiumBase = false;
    bool noseOnGround = c.v("L:A32NX_LGCIU_1_NOSE_GEAR_COMPRESSED") != 0 || c.v("L:A32NX_LGCIU_2_NOSE_GEAR_COMPRESSED") != 0;
    double groundSpeed = 0.0;
    for (int i = 1; i <= 3; i++) {
        Arinc g = arinc(c.v("L:A32NX_ADIRS_IR_" + std::to_string(i) + "_GROUND_SPEED"));
        if (g.valid() || i == 3) {
            groundSpeed = g.valid() ? double(g.f32()) : 0.0;
            break;
        }
    }
    bool fwsFallback = !fwsAvail && !cpiomFallback && !(noseOnGround && groundSpeed >= 50.0);
    if (fwsFallback) {
        std::vector<WdLine> lines;
        bool fcdc12Failed = !arinc(c.v("L:A32NX_FCDC_1_DISCRETE_WORD_1")).valid() && !arinc(c.v("L:A32NX_FCDC_2_DISCRETE_WORD_1")).valid();
        const char* fbId = (cpiomCFailed || (!fwsAvail && fcdc12Failed)) && !cpiomOtherFailed ? "314800003" : "314800004";
        json t10 = json::array(), f10 = json::array();
        for (int k = 0; k < 10; k++) {
            t10.push_back(true);
            f10.push_back(false);
        }
        json fb = {{"procedureActivated", true}, {"procedureCompleted", false}, {"itemsActive", t10}, {"itemsChecked", f10}, {"itemsToShow", t10}};
        const json& allAbn = ecam()["EcamAbnormalSensedProcedures"];
        if (allAbn.contains(fbId)) proc_lines(ProcType::Fallback, fbId, allAbn[fbId], fb, false, false, lines);
        float pitch = kDpr > 1.25f ? 32.89f : 35.77f;
        for (int i = 0; i < kNumLines; i++) {
            kRowOff[i] = i * pitch;
            kRowH[i] = pitch;
        }
        kLineH = pitch;
        kBaseGap = kDpr > 1.25f ? 2.f : -0.5f;
        kSvgDy = kDpr > 1.25f ? 0.f : -1.f;
        for (int i = 0; i < int(lines.size()) && i < kNumLines; i++) draw_line(c, lines[i], i, false);
    } else if (showAbn && fwsAvail && !cpiomFallback) {
        std::vector<WdLine> lines;
        const json* procs = bus_data(in, "fws_abn_sensed_procedures");
        std::string activeId;
        if (const json* a = bus_data(in, "fws_active_procedure"))
            activeId = a->is_string() ? a->get<std::string>() : (a->is_number() ? std::to_string(a->get<long long>()) : std::string());
        int activeItem = bus_int(in, "fws_active_item", 0), showFrom = bus_int(in, "fws_show_from_line", 0);
        const json& allAbn = ecam()["EcamAbnormalSensedProcedures"];
        if (procs && procs->is_array())
            for (size_t i = 0; i < procs->size(); i++) {
                const json& p = (*procs)[i];
                std::string pid = jid(p, "id");
                if (!allAbn.contains(pid)) continue;
                proc_lines(ProcType::Abnormal, pid, allAbn[pid], p, pid == activeId, i == 0, lines);
            }
        int total = int(lines.size());
        kAbnFit = true;
        kLineH = flex_pitch(lines, showFrom, false);
        kAbnFit = false;
        int slot = 0;
        for (int i = 0; i < total && slot < kNumLines; i++) {
            if (i < showFrom) continue;
            const WdLine& d = lines[i];
            bool sel = d.origIdx != INT32_MIN && d.active && d.origIdx == activeItem;
            draw_line(c, d, slot, sel);
            slot++;
        }
        SkPaint g = fill_paint(kGreen);
        auto arrow = [&](float top, bool up) {
            cv.save();
            cv.translate(8.f, top);
            cv.scale(30.f / 36.f, 30.f / 36.f);
            if (up) {
                cv.translate(18, 20);
                cv.rotate(180);
                cv.translate(-18, -20);
            }
            cv.drawRect(SkRect::MakeXYWH(13.25f, 1.62f, 9.53f, 17.42f), g);
            SkPathBuilder pb;
            pb.moveTo(0.137f, 14.207f);
            pb.lineTo(17.824f, 38.233f);
            pb.lineTo(35.855f, 14.193f);
            pb.close();
            a380::draw_path_cached(cv, pb.detach(), g);
            cv.restore();
        };
        if (showFrom > 0) arrow(391.f + 50.f, true);
        if (total > kNumLines && total - kNumLines > showFrom) arrow(361.f + kNumLines * kLineH, false);
    } else if (showNonSensed) {
        kAllBase = false;
        kSvgDy = 0.f;
        kLineH = 34.3f;
        kTopDy = 0.33f;
        kTextDy = 0.f;
        struct Ov {
            const char* id;
            const char* cat;
        };
        static const Ov ov[] = {{"260900097", ""}, {"990900006", ""}, {"990900005", ""}, {"990900004", ""}, {"990900007", ""},
                                {"340900003", ""}, {"700900001", "ENG"}, {"700900002", "ENG"}, {"270900001", "F/CTL"},
                                {"270900004", "F/CTL"}, {"270900005", "F/CTL"}, {"270900002", "F/CTL"}, {"270900003", "F/CTL"},
                                {"280900001", "FUEL"}, {"280900002", "FUEL"}, {"280900003", "FUEL"}, {"320900001", "L/G"},
                                {"320900002", "L/G"}, {"320900003", "L/G"}, {"320900004", "L/G"}, {"320900005", "L/G"},
                                {"320900006", "L/G"}, {"340900001", "NAV"}, {"340900002", "NAV"}, {"990900011", "MISCELLANEOUS"},
                                {"990900009", "MISCELLANEOUS"}, {"990900002", "MISCELLANEOUS"}, {"990900003", "MISCELLANEOUS"},
                                {"990900001", "MISCELLANEOUS"}, {"990900010", "MISCELLANEOUS"}, {"990900008", "MISCELLANEOUS"}};
        static const char* cats[] = {"ENG", "F/CTL", "L/G", "NAV", "FUEL", "MISCELLANEOUS"};
        int nsId = bus_int(in, "fws_abn_non_sensed_id", 0), activeItem = bus_int(in, "fws_active_item", 0),
            showFrom = bus_int(in, "fws_show_from_line", 0);
        const json& all = ecam()["EcamAbnormalSensedProcedures"];
        auto title = [&](const char* id) {
            std::string t = all.contains(id) ? all[id].value("title", "") : "";
            size_t a = t.find("" "4m"), b = t.rfind("" "m ");
            if (a != std::string::npos && b != std::string::npos && b > a) t.erase(a, b + 3 - a);
            return t;
        };
        auto mk = [&](const std::string& text, bool first, bool last, int idx) {
            WdLine l;
            l.abnormal = l.active = l.sensed = true;
            l.text = text;
            l.style = "Headline";
            l.first = first;
            l.last = last;
            l.origIdx = idx;
            return l;
        };
        std::vector<WdLine> lines;
        const std::string head = "" "4mABNORMAL PROC" "m";
        if (nsId == 0) {
            lines.push_back(mk(head, true, false, INT32_MIN));
            int nNull = 0;
            for (auto& o : ov) nNull += o.cat[0] == 0;
            int numItems = nNull + 6;
            int index = 0;
            for (auto& o : ov) {
                if (!o.cat[0]) lines.push_back(mk(nbsp(2) + title(o.id), false, index == numItems - 1, index));
                index++;
            }
            for (int i = 0; i < 6; i++) lines.push_back(mk(nbsp(2) + cats[i], false, i == 5, i + nNull));
        } else if (nsId > 0 && nsId <= 10) {
            std::string cat = nsId <= 6 ? cats[nsId - 1] : "";
            lines.push_back(mk(head, true, false, INT32_MIN));
            lines.push_back(mk(nbsp(2) + "" "4m" + cat + "" "m", false, false, INT32_MIN));
            int n = 0;
            for (auto& o : ov) n += cat == o.cat;
            int i = 0;
            for (auto& o : ov)
                if (cat == o.cat) {
                    lines.push_back(mk(nbsp(4) + title(o.id), false, i == n - 1, i));
                    i++;
                }
        } else if (all.contains(std::to_string(nsId))) {
            const json& abn = all[std::to_string(nsId)];
            const json& items = abn["items"];
            bool nsActive = bus_bool(in, "fws_abn_non_sensed_current_active", false);
            lines.push_back(mk(abn.value("title", ""), true, false, INT32_MIN));
            WdLine ac;
            ac.abnormal = ac.active = true;
            ac.checked = nsActive;
            ac.text = nbsp(31) + "ACTIVATE";
            ac.style = "ChecklistItem";
            ac.last = items.empty();
            ac.origIdx = -1;
            lines.push_back(ac);
            for (size_t k = 0; k < items.size(); k++) {
                const json& it = items[k];
                int level = it.value("level", 0);
                std::string name = it.value("name", "");
                std::string text = nbsp(level);
                if (it.contains("labelNotCompleted")) {
                    text += "-" + name;
                    std::string lnc = it.value("labelNotCompleted", "");
                    if (!lnc.empty()) {
                        int pad = std::max<int>(0, 39 - int(u8len(lnc) + u8len(name) + level + 2));
                        text += " " + std::string(pad, '.') + lnc;
                    }
                } else if (it.contains("condition")) {
                    if (name.compare(0, 4, "WHEN") == 0) text += "." + name;
                    else text += ".IF " + (name.compare(0, 2, "IF") == 0 ? name.substr(2) : name);
                } else {
                    text += name;
                }
                WdLine l;
                l.abnormal = l.active = true;
                l.sensed = it.value("sensed", false);
                l.text = u8sub(text, 39);
                l.style = it.value("style", "ChecklistItem");
                l.last = k + 1 == items.size();
                l.origIdx = int(k) + 1;
                l.inactive = !nsActive;
                lines.push_back(l);
            }
        }
        kLineH = flex_pitch(lines, showFrom, false);
        kTopDy = 0.f;
        int slot = 0;
        for (int i = showFrom; i < int(lines.size()) && slot < kNumLines; i++, slot++) {
            const WdLine& d = lines[i];
            draw_line(c, d, slot, d.origIdx != INT32_MIN && d.origIdx == activeItem);
        }
    } else if (showNormal) {
        kLineH = 31.925f;
        kChromiumBase = true;
        kBaseGap = 2.f;
        const json* cls = bus_data(in, "fws_normal_checklists");
        int clId = bus_int(in, "fws_normal_checklists_id", 0), activeItem = bus_int(in, "fws_active_item", 0),
            showFrom = bus_int(in, "fws_show_from_line", 0);
        std::vector<WdLine> lines;
        if (clId == 0 && cls && cls->is_array()) {
            const json& np = ecam()["EcamNormalProcedures"];
            std::vector<json> sorted;
            for (auto& st : *cls)
                if (st.value("id", "0") != "0") sorted.push_back(st);
            std::sort(sorted.begin(), sorted.end(), [](const json& a, const json& b) {
                return std::stoll(a.value("id", "0")) < std::stoll(b.value("id", "0"));
            });
            WdLine h;
            h.active = h.sensed = true;
            h.text = "CHECKLISTS";
            h.style = "Headline";
            h.first = true;
            lines.push_back(h);
            const json& dids = ecam()["deferredProcedureIds"];
            for (size_t i = 0; i < sorted.size(); i++) {
                std::string id = sorted[i].value("id", "0");
                if (!np.contains(id)) continue;
                int dtype = -1;
                for (size_t k = 0; k < dids.size(); k++)
                    if (std::to_string(dids[k].get<long long>()) == id) dtype = int(k);
                bool done = sorted[i].value("procedureCompleted", false);
                WdLine l;
                l.active = l.sensed = true;
                l.checked = done;
                l.text = np[id].value("title", "");
                if (l.text == "<<DEPARTURE CHANGE>>") l.text = "<>";
                l.style = done ? "CompletedChecklist" : "ChecklistItem";
                if (dtype >= 0) {
                    bool has = false, allDone = true;
                    const json* dp = bus_data(in, "fws_deferred_procedures");
                    const json& dtab = ecam()["EcamDeferredProcedures"];
                    if (dp && dp->is_array())
                        for (auto& p : *dp) {
                            std::string pid = p.is_string() ? p.get<std::string>() : p.value("id", "");
                            int ty = dtab.contains(pid) ? dtab[pid].value("type", -1) : -1;
                            has |= ty == dtype;
                            allDone &= ty == dtype && p.value("procedureCompleted", false);
                        }
                    if (!has) continue;
                    l.checked = allDone;
                    l.style = done ? "CompletedDeferredProcedure" : "DeferredProcedure";
                }
                l.last = i + 1 == sorted.size();
                l.origIdx = int(i);
                lines.push_back(l);
            }
        } else if (clId != 0 && cls && cls->is_array()) {
            const json& np = ecam()["EcamNormalProcedures"];
            const json& dids = ecam()["deferredProcedureIds"];
            const std::string sid = std::to_string(clId);
            const json* clState = nullptr;
            for (auto& st : *cls)
                if (jid(st, "id") != "0" && jid(st, "id") == sid) {
                    clState = &st;
                    break;
                }
            int deferredType = -1;
            for (size_t k = 0; k < dids.size(); k++)
                if (std::to_string(dids[k].get<long long>()) == sid) deferredType = int(k);
            if (clState && np.contains(sid) && deferredType < 0) {
                proc_lines(ProcType::Normal, sid, np[sid], *clState, true, false, lines);
            } else if (clState && np.contains(sid)) {
                WdLine hd;
                hd.abnormal = hd.active = hd.sensed = true;
                hd.text = std::string(jflag(*clState, "procedureCompleted") ? "\x1b<7m" : "\x1b<4m") + np[sid].value("title", "") + " \x1bm";
                hd.style = "Headline";
                hd.first = true;
                lines.push_back(hd);
                WdLine gap;
                gap.abnormal = gap.active = gap.sensed = true;
                gap.style = "ChecklistItem";
                lines.push_back(gap);
                std::string activeDeferred = "0";
                if (const json* a = bus_data(in, "fws_active_procedure"))
                    activeDeferred = a->is_string() ? a->get<std::string>() : (a->is_number() ? std::to_string(a->get<long long>()) : std::string("0"));
                const json& dtab = ecam()["EcamDeferredProcedures"];
                std::vector<const json*> visible;
                if (const json* dp = bus_data(in, "fws_deferred_procedures"))
                    if (dp->is_array())
                        for (auto& p : *dp) {
                            std::string pid = jid(p, "id");
                            if (dtab.contains(pid) && dtab[pid].value("type", -1) == deferredType) visible.push_back(&p);
                        }
                for (size_t k = 0; k < visible.size(); k++) {
                    std::string pid = jid(*visible[k], "id");
                    proc_lines(ProcType::Deferred, pid, dtab[pid], *visible[k], pid == activeDeferred, k + 1 == visible.size(), lines);
                }
                WdLine cl;
                cl.abnormal = cl.active = true;
                cl.text = nbsp(34) + "CLEAR";
                cl.style = "ChecklistItem";
                cl.last = true;
                cl.origIdx = -99;
                lines.push_back(cl);
            }
        }
        kLineH = flex_pitch(lines, showFrom, true);
        if (lines.empty()) kStsDy = 0.f;
        int slot = 0;
        for (int i = showFrom; i < int(lines.size()) && slot < kNumLines; i++, slot++) {
            const WdLine& d = lines[i];
            draw_line(c, d, slot, d.origIdx != INT32_MIN && d.origIdx == activeItem);
        }
    }

    if (stsArea) {
        bool pending = bus_bool(in, "fws_show_failure_pending", false) && fwsAvail;
        bool sts = bus_bool(in, "fws_show_sts_indication", false) && fwsAvail;
        bool adv = bus_bool(in, "fws_show_adv_indication", false) && fwsAvail;
        auto box = [&](const std::string& t, float x) {
            float w = a380::text_width(c.f24, t) + 4 + 4;
            float top = 981.333333f;
            auto dev = [&](float css) { return std::round(css * 1.5f); };
            float l = dev(x), r = dev(x + w), tp = dev(top + kStsDy), bt = dev(top + 27.333333f + kStsDy);
            auto rc = [&](float x0d, float y0d, float x1d, float y1d, uint32_t colr) {
                cv.drawRect(SkRect::MakeLTRB(x0d / 1.5f, y0d / 1.5f, x1d / 1.5f, y1d / 1.5f), fill_paint(colr));
            };
            rc(l, tp, r, tp + 3.f, kWhite);
            rc(l, tp, l + 3.f, bt, kWhite);
            rc(l, bt - 3.f, r, bt, 0xababab);
            rc(r - 3.f, tp, r, bt, 0xababab);
            c.text(t, x + 4, top + kStsDy + 22.f + 4.f / 3.f, c.f24, kWhite);
        };
        const json* d = bus_data(in, "fws_deferred_procedures");
        std::string stsText = d && d->is_array() && d->size() > 0 ? "STS & DEFRD PROC" : "STS";
        float wFp = a380::text_width(c.f24, "FAILURE PENDING") + 8.f;
        float wSts = a380::text_width(c.f24, stsText) + 8.f;
        if (pending) box("FAILURE PENDING", 48.f);
        if (sts) box(stsText, 8.f + wFp + 150.f);
        if (adv) box("ADV", 8.f + wFp + wSts + 360.f);
    }
}

class Ewd : public a380::Display {
public:
    SkISize size() const override { return {768, 1024}; }
    void draw(SkCanvas& canvas, const a380::Inputs& in) override {
        Ctx c{canvas, in};
        if (!du_.update(canvas, in, size())) return;
        state_.nowMs = in.now_ms();
        gState = &state_;
        prof_.time("engines", [&] { draw_engines(c); });
        {
            float sepDpr = canvas.getLocalToDeviceAs3x3().getScaleX();
            canvas.drawRect(SkRect::MakeXYWH(8, std::round(375.6f * sepDpr) / sepDpr, 752, 4.f), fill_paint(kLight));
        }
        prof_.time("wd", [&] {
            int saves = canvas.getSaveCount();
            try {
                draw_wd(c);
            } catch (const std::exception&) {
                canvas.restoreToCount(saves);
            }
        });
        prof_.frame();
    }

private:
    a380::SectionProfile prof_{"ewd"};
    EwdState state_;
    a380::CdsDisplayUnit du_{"92", {"DC_ESS"}};
};

std::unique_ptr<a380::Display> make(std::string_view) { return std::make_unique<Ewd>(); }

}  // namespace

A380_REGISTER_DISPLAY("ewd", make);
