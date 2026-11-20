#include <algorithm>
#include "a380/env.h"
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#include "include/core/SkCanvas.h"
#include "include/core/SkColor.h"
#include "include/core/SkFont.h"
#include "include/core/SkFontMetrics.h"
#include "include/core/SkImage.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPath.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkPathEffect.h"
#include "include/core/SkSamplingOptions.h"
#include "include/core/SkSurface.h"
#include "include/effects/SkDashPathEffect.h"
#include "include/utils/SkParsePath.h"

#include "a380/draw.h"
#include "a380/fonts.h"
#include "a380/inputs.h"
#include "nd/nd_layers.h"
#include "nd/nd_prim.h"
#include "nd/vd.h"

namespace a380::nd {

namespace {
constexpr int kPm = prim::kLine;

constexpr double kMaxAlt = 70000.0;
constexpr double kMinAlt = -500.0;
constexpr float kCanvasW = 540.f;
constexpr float kCanvasH = 200.f;
const double kFpaToDisplay = (std::asin(200.0 / 540.0) * 180.0 / 3.14159265358979323846) / 4.0;
constexpr double kFeetPerNm = 6076.12;

constexpr int kSymWaypoint = 1 << 3;
constexpr int kSymAirport = 1 << 4;
constexpr int kSymRunway = 1 << 5;
constexpr int kSymActiveLegTerm = 1 << 7;
constexpr int kSymFixInfo = 1 << 11;
constexpr int kSymFlightPlan = 1 << 12;
constexpr int kSymCyan = 1 << 15;
constexpr int kSymAmber = 1 << 16;
constexpr int kSymMagenta = 1 << 17;
constexpr int kPwpDecel = 1 << 1;
constexpr int kPwpSpeedChange = 1 << 3;

struct Arinc {
    int ssm = 0;
    float value = 0.f;
    bool failure() const { return ssm == 0; }
    bool normal() const { return ssm == 3; }
    bool valid() const { return ssm == 3 || ssm == 2; }
    double value_or(double d) const { return valid() ? value : d; }
    bool bit_or(int bit, bool d) const {
        return valid() ? (((int)value >> (bit - 1)) & 1) != 0 : d;
    }
};

Arinc arinc(const Inputs& in, const std::string& name) {
    double raw = in.var(name, "number", 0.0);
    Arinc a;
    if (raw < 0) raw = 0;
    uint64_t u = (uint64_t)raw;
    a.ssm = (int)((u >> 32) & 3);
    uint32_t lo = (uint32_t)(u & 0xffffffffu);
    std::memcpy(&a.value, &lo, 4);
    return a;
}

struct Range {
    double lo, hi;
};

double alt_to_y_abs(double alt, const Range& r) { return 800.0 + (r.hi - alt) / ((r.hi - r.lo) / 200.0); }
double alt_to_y(double alt, const Range& r) { return alt_to_y_abs(alt, r) - 800.0; }
double dist_to_x(double d, double vd_range) { return d / vd_range * 540.0; }

Range min_max_vertical_range(double vd_range, double ego, bool have_path, double first, double last) {
    double extent = vd_range * kFeetPerNm * std::sin(4.0 * 3.14159265358979323846 / 180.0);
    if (!have_path) {
        double lower = std::max(kMinAlt, std::min(ego - 0.5 * extent, kMaxAlt - extent));
        return {lower, lower + extent};
    }
    double hi = std::max(first, last), lo = std::min(first, last);
    double lower = std::max(kMinAlt, (lo + hi - extent) / 2.0);
    double upper = lower + extent;
    if (ego < lower) {
        lower = std::max(kMinAlt, ego - 0.15 * extent);
        upper = lower + extent;
    } else if (ego > upper) {
        upper = std::min(kMaxAlt, ego + 0.15 * extent);
        lower = upper - extent;
    }
    return {lower, upper};
}

double tape_alt(int index, double vd_range, const Range& r) {
    double per = 500;
    int v = (int)std::lround(vd_range);
    switch (v) {
        case 5: per = 250; break;
        case 10: per = 500; break;
        case 20: per = 1000; break;
        case 40: per = 2000; break;
        case 80: per = 5000; break;
        case 160: per = 10000; break;
        default: break;
    }
    return std::max(0.0, std::ceil(r.lo / per) * per) + per * index;
}

std::string num(double v) {
    char b[48];
    std::snprintf(b, sizeof b, "%.6g", v);
    return b;
}

SkFont ecam(float px) {
    SkFont f = a380::font("Ecam", px);
    f.setEdging(SkFont::Edging::kAntiAlias);
    return f;
}

SkColor rgb(uint32_t c) { return SkColorSetRGB((c >> 16) & 255, (c >> 8) & 255, c & 255); }

enum class Align { Start, Middle, End };

SkPaint stroke_paint(SkColor col, float w);

void svg_text(SkCanvas& c, const std::string& s, float x, float y, const SkFont& font, SkColor color, Align a,
              bool fill) {
    if (s.empty()) return;
    float w = font.measureText(s.data(), s.size(), SkTextEncoding::kUTF8);
    if (a == Align::Middle) x -= w / 2;
    else if (a == Align::End) x -= w;
    SkPaint p;
    p.setAntiAlias(true);
    if (!fill) {
        SkPaint sp = stroke_paint(SK_ColorBLACK, 1.f);
        a380::draw_simple_text(c, s.data(), s.size(), SkTextEncoding::kUTF8, x, y, font, sp);
    }
    p.setColor(color);
    a380::draw_simple_text(c, s.data(), s.size(), SkTextEncoding::kUTF8, x, y, font, p);
}

SkPaint stroke_paint(SkColor col, float w) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setStyle(SkPaint::kStroke_Style);
    p.setStrokeWidth(w);
    p.setColor(col);
    return p;
}

struct Symbol {
    std::string ident;
    double dist = 0, alt = 0, length = 0.5;
    int type = 0, type_pwp = 0;
    bool has_loc = false;
    bool has_cst = false;
    bool met = false;
    std::string desc;
    double alt1 = 0, alt2 = 0;
};

double jnum(const nlohmann::json& j, const char* k, double d = 0.0) {
    auto it = j.find(k);
    return it != j.end() && it->is_number() ? it->get<double>() : d;
}

std::vector<Symbol> read_symbols(const nlohmann::json* arr) {
    std::vector<Symbol> out;
    if (!arr || !arr->is_array()) return out;
    for (auto& e : *arr) {
        if (!e.is_object()) continue;
        Symbol s;
        if (auto it = e.find("ident"); it != e.end() && it->is_string()) s.ident = it->get<std::string>();
        s.dist = jnum(e, "distanceFromAirplane");
        s.alt = jnum(e, "predictedAltitude");
        s.type = (int)jnum(e, "type");
        s.type_pwp = (int)jnum(e, "typePwp");
        s.length = jnum(e, "length", 0.5);
        s.has_loc = e.contains("location") && !e["location"].is_null();
        if (auto it = e.find("isAltitudeConstraintMet"); it != e.end() && it->is_boolean()) s.met = it->get<bool>();
        if (auto it = e.find("altConstraint"); it != e.end() && it->is_object()) {
            s.has_cst = true;
            if (auto d = it->find("altitudeDescriptor"); d != it->end() && d->is_string()) s.desc = d->get<std::string>();
            s.alt1 = jnum(*it, "altitude1");
            s.alt2 = jnum(*it, "altitude2");
        }
        out.push_back(std::move(s));
    }
    return out;
}

struct Ctx {
    SkCanvas* c;
    SkFont font;
    float line_width = 2.f;
    SkColor stroke_color = rgb(0x00ff00);
};

void canvas_text(Ctx& x, bool color_layer, float px, float py, const std::string& text, SkColor color,
                 Align align = Align::Start, bool middle = false) {
    float w = x.font.measureText(text.data(), text.size(), SkTextEncoding::kUTF8);
    float ox = 0, oy = 0;
    if (align == Align::Middle) ox = -w / 2;
    if (middle) {
        SkFontMetrics m;
        x.font.getMetrics(&m);
        oy = (-m.fAscent - m.fDescent) / 2.f + 4.2f;
    }
    SkPaint p;
    p.setAntiAlias(true);
    if (!color_layer) {
        p.setStyle(SkPaint::kStroke_Style);
        p.setStrokeWidth(3.f);
        p.setStrokeJoin(SkPaint::kMiter_Join);
        p.setColor(SK_ColorBLACK);
    } else {
        p.setColor(color);
    }
    a380::draw_simple_text(*x.c, text.data(), text.size(), SkTextEncoding::kUTF8, px + ox, py + oy, x.font, p);
}

void waypoint_shape(Ctx& x, float px, float py, SkColor col, float lw) {
    x.stroke_color = col;
    x.line_width = lw;
    SkPaint p = stroke_paint(col, lw);
    x.c->save();
    x.c->translate(px, py);
    x.c->rotate(45.f);
    prim::rect(kPm, *x.c, SkRect::MakeLTRB(-4.5f, -4.5f, 4.5f, 4.5f), p);
    x.c->restore();
}

void airport_shape(Ctx& x, float px, float py, SkColor col, float lw) {
    x.stroke_color = col;
    x.line_width = lw;
    SkPaint p = stroke_paint(col, lw);
    x.c->save();
    x.c->translate(px, py);
    SkPathBuilder b;
    b.moveTo(-13, 0);
    b.lineTo(13, 0);
    b.moveTo(0, -13);
    b.lineTo(0, 13);
    prim::path(kPm, *x.c, b.detach(), p);
    x.c->rotate(45.f);
    SkPathBuilder b2;
    b2.moveTo(0, -13);
    b2.lineTo(0, 13);
    prim::path(kPm, *x.c, b2.detach(), p);
    x.c->rotate(90.f);
    SkPathBuilder b3;
    b3.moveTo(0, -13);
    b3.lineTo(0, 13);
    prim::path(kPm, *x.c, b3.detach(), p);
    x.c->restore();
}

void constraint_path(Ctx& x, float px, float py, bool below, SkColor col) {
    SkPathBuilder b;
    if (below) {
        b.moveTo(0, 0);
        b.lineTo(5, -10);
        b.lineTo(-5, -10);
        b.lineTo(0, 0);
    } else {
        b.moveTo(0, 0);
        b.lineTo(5, 10);
        b.lineTo(-5, 10);
        b.lineTo(0, -0);
    }
    x.line_width = 2.f;
    x.stroke_color = col;
    SkPaint p = stroke_paint(col, 2.f);
    x.c->save();
    x.c->translate(px, py);
    prim::path(kPm, *x.c, b.detach(), p);
    x.c->restore();
}

void missed_constraint_line(Ctx& x, float px) {
    SkPaint p = stroke_paint(rgb(0xe68000), 2.f);
    const float iv[2] = {2.f, 2.f};
    p.setPathEffect(SkDashPathEffect::Make({iv, 2}, 0));
    x.line_width = 2.f;
    x.stroke_color = rgb(0xe68000);
    prim::dashed(kPm, *x.c, px, 0, px, 200, iv, 2, p);
}

void flight_plan_waypoint(Ctx& x, bool color_layer, float px, float py, const Symbol& s, const Range& vr,
                          bool selected_mode_vertical) {
    SkColor main = (s.type & kSymActiveLegTerm) ? rgb(0xffffff) : rgb(0x00ff00);
    waypoint_shape(x, px, py, color_layer ? main : SK_ColorBLACK, color_layer ? 1.75f : 3.25f);
    if (s.has_cst) {
        SkColor stroke = selected_mode_vertical ? rgb(0xffffff) : (s.met ? rgb(0xff94ff) : rgb(0xe68000));
        if (!selected_mode_vertical && !s.met) missed_constraint_line(x, px);
        const std::string& d = s.desc;
        auto cy = [&](double a) { return (float)alt_to_y(a, vr); };
        if (d == "@" || d == "I" || d == "X") {
            if (s.alt1 != 0) {
                constraint_path(x, px, cy(s.alt1), true, stroke);
                constraint_path(x, px, cy(s.alt1), false, rgb(0xff94ff));
            }
        } else if (d == "+" || d == "J" || d == "V") {
            if (s.alt1 != 0) constraint_path(x, px, cy(s.alt1), false, stroke);
        } else if (d == "-" || d == "Y") {
            if (s.alt1 != 0) constraint_path(x, px, cy(s.alt1), true, stroke);
        } else if (d == "B") {
            if (s.alt1 != 0 && s.alt2 != 0) {
                constraint_path(x, px, cy(s.alt1), true, stroke);
                constraint_path(x, px, cy(s.alt2), false, stroke);
            }
        } else if (d == "C") {
            if (s.alt2 != 0) constraint_path(x, px, cy(s.alt2), false, stroke);
        }
    }
    canvas_text(x, color_layer, px + 15, py + 17, s.ident, main);
}

void paint_waypoints(Ctx& x, bool color_layer, const std::vector<Symbol>& syms, double vd_range, const Range& vr,
                     bool selected_mode_vertical) {
    for (auto& s : syms) {
        if (s.dist == 0 || s.alt == 0) continue;
        float px = (float)dist_to_x(s.dist, vd_range);
        float py = (float)alt_to_y(s.alt, vr);
        if (s.type & (kSymAirport | kSymRunway)) {
            SkColor main = (s.type & kSymFlightPlan) ? rgb(0xffffff) : rgb(0xff94ff);
            airport_shape(x, px, py, color_layer ? main : SK_ColorBLACK, color_layer ? 1.75f : 3.25f);
            canvas_text(x, color_layer, px + 13, py + 18, s.ident, main);
        } else if (s.type & (kSymFixInfo | kSymFlightPlan)) {
            flight_plan_waypoint(x, color_layer, px, py, s, vr, selected_mode_vertical);
        } else {
            waypoint_shape(x, px, py, color_layer ? rgb(0xff94ff) : SK_ColorBLACK, color_layer ? 1.75f : 3.25f);
            canvas_text(x, color_layer, px + 15, py + 17, s.ident, rgb(0xff94ff));
        }
    }
}

void runway_ident(Ctx& x, bool color_layer, float px, float py, float rotation, const Symbol& s) {
    std::string icao = s.ident.substr(0, std::min<size_t>(4, s.ident.size()));
    std::string rwy = s.ident.size() > 4 ? s.ident.substr(4) : std::string();
    while (rwy.size() < 4) rwy += "\xc2\xa0";
    x.c->save();
    x.c->translate(px + 40, py - 20);
    x.c->rotate(-rotation);
    x.c->translate(-(px + 40), -(py - 20));
    canvas_text(x, color_layer, px + 40, py - 100, icao, SK_ColorWHITE, Align::Middle, true);
    canvas_text(x, color_layer, px + 40, py - 80, rwy, SK_ColorWHITE, Align::Middle, true);
    x.c->restore();
}

void paint_runways(Ctx& x, bool color_layer, const std::vector<Symbol>& syms, double vd_range, const Range& vr) {
    for (auto& s : syms) {
        if (s.dist == 0 || s.alt == 0) continue;
        float px = (float)dist_to_x(s.dist, vd_range);
        float py = (float)alt_to_y(s.alt, vr);
        x.c->save();
        x.c->translate(px, py);
        x.c->rotate(90.f);
        x.c->translate(-px, -py);
        float lw = color_layer ? 1.75f : 3.25f;
        SkColor col = color_layer ? SK_ColorWHITE : SK_ColorBLACK;
        x.line_width = lw;
        x.stroke_color = col;
        SkPaint p = stroke_paint(col, lw);
        if (vd_range < 160) {
            float len = (float)dist_to_x(s.length, vd_range);
            SkPathBuilder b;
            b.moveTo(px - 5, py);
            b.lineTo(px - 5, py - len);
            b.moveTo(px + 5, py);
            b.lineTo(px + 5, py - len);
            prim::path(kPm, *x.c, b.detach(), p);
        } else {
            prim::rect(kPm, *x.c, SkRect::MakeXYWH(px - 5, py - 12.5f, 10, 25), p);
        }
        runway_ident(x, color_layer, px, py, 90.f, s);
        x.c->restore();
    }
}

SkColor pwp_color(int type) {
    if (type & kSymMagenta) return rgb(0xff94ff);
    if (type & kSymAmber) return rgb(0xe68000);
    if (type & kSymCyan) return rgb(0x00ffff);
    return SK_ColorWHITE;
}

const char* kDecel =
    "m 14.5125 0 c 0 8.015 -6.4975 14.5125 -14.5125 14.5125 c -8.015 0 -14.5125 -6.4975 -14.5125 -14.5125 c 0 "
    "-8.015 6.4975 -14.5125 14.5125 -14.5125 c 8.015 0 14.5125 6.4975 14.5125 14.5125 z m -12.15 -9.7875 h "
    "-7.7625 v 18.225 h 7.7625 l 2.7 -3.375 v -11.475 l -2.7 -3.375 z";

void paint_pwps(Ctx& x, bool color_layer, const std::vector<Symbol>& syms, double vd_range, const Range& vr) {
    thread_local SkPath decel;
    thread_local bool parsed = false;
    if (!parsed) {
        SkParsePath::FromSVGString(kDecel, &decel);
        parsed = true;
    }
    for (auto& s : syms) {
        if (s.dist == 0 || s.alt == 0) continue;
        float px = (float)dist_to_x(s.dist, vd_range);
        float py = (float)alt_to_y(s.alt, vr);
        SkColor col = color_layer ? pwp_color(s.type) : SK_ColorBLACK;
        if (s.type_pwp & kPwpDecel) {
            SkPaint p = stroke_paint(col, x.line_width);
            x.c->save();
            x.c->translate(px, py);
            prim::path(kPm, *x.c, decel, p);
            x.c->restore();
        } else if (s.type_pwp & kPwpSpeedChange) {
            SkPaint p;
            p.setAntiAlias(true);
            p.setColor(col);
            prim::circle(kPm, *x.c, px, py, 8.f, p);
        }
    }
}

struct PathPoint {
    double d, a;
};

std::vector<PathPoint> read_profile(const nlohmann::json* arr) {
    std::vector<PathPoint> out;
    if (!arr || !arr->is_array()) return out;
    for (auto& e : *arr)
        if (e.is_object()) out.push_back({jnum(e, "distanceFromAircraft"), jnum(e, "altitude")});
    return out;
}

bool boolvar(const Inputs& in, const std::string& n) { return in.var(n, {}, 0.0) != 0.0; }

}  // namespace

static void vd_impl(SkCanvas* cv, const Inputs& in, const Frame& f, VdVerticalRange* out) {
    const bool left = f.side != 2;
    static const double kRanges[] = {-1, 10, 20, 40, 80, 160, 320, 640};
    int ri = std::clamp(f.range_index, 0, 7);
    double nd_range = kRanges[ri];
    const int mode = f.mode;
    const bool visible = !(mode == 0 || mode == 1 || mode == 4 || nd_range == -1);

    const double vd_range =
        mode == 3 ? std::max(10.0, std::min(nd_range, 160.0)) : std::max(5.0, std::min(nd_range / 2.0, 160.0));

    int att_knob = (int)in.var("L:A32NX_ATT_HDG_SWITCHING_KNOB", {}, 0);
    int air_knob = (int)in.var("L:A32NX_AIR_DATA_SWITCHING_KNOB", {}, 0);
    int ir = left ? (att_knob == 0 ? 3 : 1) : (att_knob == 2 ? 3 : 2);
    int adr = left ? (air_knob == 0 ? 3 : 1) : (air_knob == 2 ? 3 : 2);
    int disp = left ? 1 : 2;
    const char side_c = left ? 'L' : 'R';

    Arinc heading = arinc(in, "L:A32NX_ADIRS_IR_" + std::to_string(ir) + "_HEADING");
    Arinc track = arinc(in, "L:A32NX_ADIRS_IR_" + std::to_string(ir) + "_TRACK");
    Arinc fpa = arinc(in, "L:A32NX_ADIRS_IR_" + std::to_string(ir) + "_FLIGHT_PATH_ANGLE");
    Arinc lat = arinc(in, "L:A32NX_ADIRS_IR_" + std::to_string(ir) + "_LATITUDE");
    Arinc lon = arinc(in, "L:A32NX_ADIRS_IR_" + std::to_string(ir) + "_LONGITUDE");
    Arinc baro = arinc(in, "L:A32NX_ADIRS_ADR_" + std::to_string(adr) + "_BARO_CORRECTED_ALTITUDE_" +
                               std::to_string(disp));
    Arinc fcu_dw2 = arinc(in, std::string("L:A32NX_FCU_EFIS_") + side_c + "_DISCRETE_WORD_2");

    const bool vd_available = heading.normal() && track.normal();
    const SkColor line_color = vd_available ? SK_ColorWHITE : rgb(0xff0000);
    const bool fl_mode = fcu_dw2.bit_or(11, true);

    const nlohmann::json* target_j = in.bus("a32nx_fms_vertical_target_profile");
    const nlohmann::json* actual_j = in.bus("a32nx_fms_vertical_actual_profile");
    std::vector<PathPoint> target = read_profile(target_j);
    std::vector<PathPoint> actual = read_profile(actual_j);

    std::vector<PathPoint> est_path;
    {
        const auto& path = actual.size() > 1 ? actual : target;
        for (auto& p : path) {
            est_path.push_back(p);
            if (p.d > vd_range * 1.2) break;
        }
    }
    Range vr = min_max_vertical_range(vd_range, baro.value_or(0.0), !est_path.empty(),
                                      est_path.empty() ? 0 : est_path.front().a,
                                      est_path.empty() ? 0 : est_path.back().a);
    if (out) *out = {vr.lo, vr.hi};
    if (!visible || !cv) return;
    SkCanvas& canvas = *cv;

    bool recomputing = false;
    int recompute_reason = 0;
    if (auto* b = in.bus("set_map_recomputing"); b && b->is_boolean()) recomputing = b->get<bool>();
    if (auto* b = in.bus("set_map_recomputing_reason"); b && b->is_number()) recompute_reason = b->get<int>();

    int vmode = (int)in.var("L:A32NX_FMA_VERTICAL_MODE", {}, 0);
    double alt_cstr = in.var("L:A32NX_FG_ALTITUDE_CONSTRAINT", {}, 0);
    double selected_alt = in.var("AUTOPILOT ALTITUDE LOCK VAR:3", "feet", 0);
    bool managed = alt_cstr != 0 && (vmode == 23 || vmode == 22 || vmode == 20 || vmode == 21);
    double target_alt = (managed && alt_cstr != 0) ? alt_cstr : selected_alt;

    SkColor target_color = rgb(0x00ffff);
    if (managed && alt_cstr != 0) target_color = rgb(0xff94ff);
    else if (vmode == 24 || vmode == 30 || vmode == 31 || vmode == 32 || vmode == 33 || vmode == 34)
        target_color = rgb(0xffffff);

    SkFont small = ecam(22.5f);
    SkFont norm = ecam(25.f);

    bool canvas_invalid = lat.failure() || lon.failure() || baro.failure();
    bool path_visible = vd_available && !recomputing && !canvas_invalid;

    if (path_visible) {
        static const bool gpu_layer = !a380::getenv_cached("A380_VD_RASTER");
        const SkImageInfo vd_info = SkImageInfo::MakeN32Premul((int)kCanvasW, (int)kCanvasH);
        const SkSurfaceProps vd_props;
        sk_sp<SkSurface> surface = gpu_layer ? canvas.makeSurface(vd_info, &vd_props) : nullptr;
        if (!surface) surface = SkSurfaces::Raster(vd_info);
        SkCanvas* cc = surface->getCanvas();
        cc->clear(SK_ColorTRANSPARENT);
        Ctx x{cc, ecam(21.f)};

        bool dashed = actual.size() > 1;
        if (!target.empty()) {
            SkPaint p = stroke_paint(rgb(0x00ff00), 2.f);
            if (dashed) {
                const float iv[2] = {10.f, 10.f};
                p.setPathEffect(SkDashPathEffect::Make({iv, 2}, 0));
            }
            SkPathBuilder b;
            b.moveTo((float)dist_to_x(target[0].d, vd_range), (float)alt_to_y(target[0].a, vr));
            for (auto& pe : target) b.lineTo((float)dist_to_x(pe.d, vd_range), (float)alt_to_y(pe.a, vr));
            prim::path(kPm, *cc, b.detach(), p);
        }
        if (dashed) {
            SkPaint p = stroke_paint(rgb(0x00ff00), 2.f);
            SkPathBuilder b;
            b.moveTo((float)dist_to_x(actual[0].d, vd_range), (float)alt_to_y(actual[0].a, vr));
            for (auto& pe : actual) b.lineTo((float)dist_to_x(pe.d, vd_range), (float)alt_to_y(pe.a, vr));
            b.lineTo((float)dist_to_x(540, vd_range), (float)alt_to_y(actual.back().a, vr));
            prim::path(kPm, *cc, b.detach(), p);
        }
        x.line_width = 2.f;

        std::vector<Symbol> all =
            read_symbols(in.bus(std::string("vdSymbols_") + side_c));
        std::vector<Symbol> wps, rwys, pwps;
        for (auto& s : all) {
            if (s.has_cst || ((s.type & (kSymWaypoint | kSymFlightPlan)) && !(s.type & kSymRunway))) wps.push_back(s);
            if ((s.type & kSymRunway) && s.has_loc) rwys.push_back(s);
            if (s.type_pwp & (kPwpDecel | kPwpSpeedChange)) pwps.push_back(s);
        }
        bool sel_vert = !managed;
        paint_waypoints(x, false, wps, vd_range, vr, sel_vert);
        paint_waypoints(x, true, wps, vd_range, vr, sel_vert);
        paint_runways(x, false, rwys, vd_range, vr);
        paint_runways(x, true, rwys, vd_range, vr);
        paint_pwps(x, false, pwps, vd_range, vr);
        paint_pwps(x, true, pwps, vd_range, vr);

        sk_sp<SkImage> img = surface->makeImageSnapshot();
        canvas.drawImageRect(img, SkRect::MakeXYWH(150, 800, kCanvasW, kCanvasH),
                             SkSamplingOptions(SkFilterMode::kLinear));
    }

    canvas.save();
    {
        canvas.save();
        canvas.clipRect(SkRect::MakeLTRB(0, 800, 130, 1000));
        SkPaint lp = stroke_paint(line_color, 2.f);
        prim::line(kPm, canvas, 105, 800, 105, 1000, lp);
        for (int i = 0; i < 8; i++) {
            double dash_alt = tape_alt(i, vd_range, vr);
            float y = (float)alt_to_y_abs(dash_alt, vr);
            prim::line(kPm, canvas, 105, y, 120, y, lp);
            if (vd_available) {
                double per = tape_alt(1, vd_range, Range{0, 0});
                std::string t;
                if (std::fmod(dash_alt, per * 2) == 0) {
                    char b[32];
                    if (fl_mode) std::snprintf(b, sizeof b, "%d", (int)std::floor(dash_alt / 100));
                    else std::snprintf(b, sizeof b, "%d", (int)dash_alt);
                    t = b;
                }
                svg_text(canvas, t, 95, y + 7.5f, small, SK_ColorWHITE, Align::End, true);
            }
        }
        if (fl_mode && vd_available) svg_text(canvas, "FL", 5, 900, small, SK_ColorWHITE, Align::Start, true);
        if (baro.normal() && baro.value < kMaxAlt && vd_available) {
            float y = (float)alt_to_y_abs(baro.value, vr);
            SkPaint yp = stroke_paint(rgb(0xffff00), 4.f);
            prim::line(kPm, canvas, 97, y, 115, y, yp);
        }
        canvas.restore();
    }

    bool target_text_visible = (target_alt < vr.lo || target_alt > vr.hi) && vd_available;
    if (target_text_visible) {
        char b[32];
        if (fl_mode) std::snprintf(b, sizeof b, "FL %d", (int)std::floor(target_alt / 100));
        else std::snprintf(b, sizeof b, "%ld", std::lround(target_alt));
        svg_text(canvas, b, 100, 800, norm, target_color, Align::End, true);
    }

    if (!fpa.failure() && vd_available) {
        double y = alt_to_y_abs(baro.value_or(0.0), vr);
        canvas.save();
        canvas.translate(132, (float)(y - 14));
        canvas.rotate((float)(-fpa.value_or(0.0) * kFpaToDisplay), 20, 10);
        canvas.scale(0.9f, 0.9f);
        SkPaint lp = stroke_paint(rgb(0xffff00), 4.f);
        lp.setStrokeCap(SkPaint::kRound_Cap);
        prim::line(kPm, canvas, 5.31255f, 15.74998f, 33.81273f, 15.74998f, lp);
        SkPathBuilder b;
        b.moveTo(4.96875f, 15.81249f);
        b.lineTo(4.96875f - 0.03125f, 15.81249f - 11.0625f);
        b.lineTo(4.96875f - 0.03125f + 11.4375f, 15.81249f - 11.0625f + 11.0f);
        b.close();
        SkPaint fp;
        fp.setAntiAlias(true);
        fp.setColor(rgb(0xffff00));
        prim::path(kPm, canvas, b.snapshot(), fp);
        SkPaint sp = stroke_paint(rgb(0xffff00), 3.f);
        sp.setStrokeJoin(SkPaint::kRound_Join);
        prim::path(kPm, canvas, b.detach(), sp);
        canvas.restore();
    }

    if (!target_text_visible && vd_available) {
        float y = (float)alt_to_y_abs(target_alt, vr) - 19.f;
        canvas.save();
        canvas.clipRect(SkRect::MakeLTRB(0, 800, 130, 1000));
        canvas.translate(99, y);
        SkPathBuilder b;
        b.moveTo(1, 35);
        b.lineTo(1, 1);
        b.lineTo(13, 1);
        b.lineTo(13, 13);
        b.lineTo(7, 19);
        b.lineTo(13, 25);
        b.lineTo(13, 37);
        b.lineTo(1, 37);
        b.lineTo(1, 35);
        b.close();
        prim::path(kPm, canvas, b.detach(), stroke_paint(target_color, 2.f));
        canvas.restore();
    }

    {
        SkPaint lp = stroke_paint(line_color, 2.f);
        prim::line(kPm, canvas, 150, 800, 690, 800, lp);
        const float iv[2] = {8.f, 8.f};
        for (float xx : {150.f, 285.f, 420.f, 555.f}) prim::dashed(kPm, canvas, xx, 800, xx, 1000, iv, 2, lp);
        prim::line(kPm, canvas, 690, 800, 690, 1000, lp);
    }

    if (vd_available) {
        SkColor cy = rgb(0x00ffff);
        svg_text(canvas, "0", 150, 797, small, cy, Align::Middle, true);
        for (int i = 1; i <= 4; i++)
            svg_text(canvas, num(vd_range / 4 * i), 150.f + 135.f * i, 797, small, cy, Align::Middle, true);
        bool over = mode == 3 ? nd_range > 160 : nd_range > 320;
        if (over) {
            SkPaint ap = stroke_paint(cy, 2.f);
            canvas.save();
            canvas.translate(715, 777);
            prim::line(kPm, canvas, 0, 11.25f, 22.5f, 11.25f, ap);
            prim::line(kPm, canvas, 22.5f, 11.25f, 11.25f, 3, ap);
            prim::line(kPm, canvas, 22.5f, 11.25f, 11.25f, 19.5f, ap);
            canvas.restore();
        }
    }

    const bool terr_off = boolvar(in, "L:A32NX_GPWS_TERR_OFF");
    int sys_sel = (int)in.var("L:A32NX_WXR_TAWS_SYS_SELECTED", {}, 1);
    bool t1 = boolvar(in, "L:A32NX_TERR_1_FAILED"), t2 = boolvar(in, "L:A32NX_TERR_2_FAILED");
    bool w1 = boolvar(in, "L:A32NX_WXR_1_FAILED"), w2 = boolvar(in, "L:A32NX_WXR_2_FAILED");
    int overlay = (int)in.var(std::string("L:A380X_EFIS_") + side_c + "_ACTIVE_OVERLAY", {}, 0);
    bool terr_inop = sys_sel == 1 ? t1 : sys_sel == 2 ? t2 : true;
    bool wxr_failed = sys_sel == 1 ? w1 : sys_sel == 2 ? w2 : true;
    bool wxr_inop = overlay == 1 && wxr_failed;
    bool range_change = recomputing && (recompute_reason == 1 || recompute_reason == 3);
    bool mode_change = recomputing && recompute_reason == 2;
    bool traj_not_avail = canvas_invalid;
    bool no_terr_wx = terr_off;
    auto msg = [&](const char* t, float xx, float yy, SkColor col) {
        svg_text(canvas, t, xx, yy, norm, col, Align::Middle, false);
    };
    if (mode_change && !traj_not_avail && !no_terr_wx && !terr_inop && !wxr_inop) msg("VD MODE CHANGE", 418, 929, rgb(0x00ff00));
    if (range_change && !traj_not_avail && !no_terr_wx && !terr_inop && !wxr_inop) msg("VD RANGE CHANGE", 422, 929, rgb(0x00ff00));
    if (no_terr_wx && !terr_inop && !wxr_inop) msg("NO TERR AND WX DATA AVAILABLE", 422, 890, SK_ColorWHITE);
    if (terr_inop) msg("TERR INOP", 285, 960, rgb(0xe68000));
    if (wxr_inop) msg("WXR INOP", 565, 960, rgb(0xe68000));
    if (traj_not_avail && !no_terr_wx && !terr_inop && !wxr_inop) msg("TRAJ NOT AVAIL", 422, 990, rgb(0xe68000));

    canvas.restore();
}

void draw_vertical_display(SkCanvas& canvas, const Inputs& in, const Frame& f) { vd_impl(&canvas, in, f, nullptr); }

VdVerticalRange vd_vertical_range(const Inputs& in, const Frame& f) {
    VdVerticalRange r;
    vd_impl(nullptr, in, f, &r);
    return r;
}

}  // namespace a380::nd
