#include "nd_layers.h"
#include "a380/shape_cache.h"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <string>
#include <vector>

#include "include/core/SkColor.h"
#include "include/core/SkFont.h"
#include "include/core/SkFontMetrics.h"
#include "include/core/SkImage.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPath.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkSamplingOptions.h"
#include "include/core/SkSurface.h"
#include "include/effects/SkDashPathEffect.h"

#include "a380/draw.h"
#include "nd/nd_prim.h"
#include "a380/fonts.h"

namespace a380::nd {
namespace {
constexpr int kPm = prim::kFillPath | prim::kRect | prim::kCircle;

using nlohmann::json;

enum SymFlags : uint32_t {
    kVor = 1u << 0,
    kVorDme = 1u << 1,
    kNdb = 1u << 2,
    kWaypoint = 1u << 3,
    kAirport = 1u << 4,
    kRunway = 1u << 5,
    kTuned = 1u << 6,
    kActiveLegTermination = 1u << 7,
    kDme = 1u << 9,
    kConstraint = 1u << 10,
    kFixInfo = 1u << 11,
    kFlightPlan = 1u << 12,
    kCourseReversalLeft = 1u << 13,
    kCourseReversalRight = 1u << 14,
    kCyan = 1u << 15,
    kAmber = 1u << 16,
    kMagentaFlag = 1u << 17,
    kSecondaryFlightPlan = 1u << 20,
};

enum PwpFlags : uint32_t {
    kPwpEndOfVd = 1u << 0,
    kPwpDecel = 1u << 1,
    kPwpTopOfDescent = 1u << 2,
    kPwpSpeedChange = 1u << 3,
    kPwpClimbLevelOff = 1u << 4,
    kPwpDescentLevelOff = 1u << 5,
    kPwpStartOfClimb = 1u << 6,
    kPwpInterceptProfile = 1u << 7,
    kPwpTimeMarker = 1u << 8,
    kPwpCdaFlap1 = 1u << 9,
    kPwpCdaFlap2 = 1u << 10,
};

constexpr float kCanvas = 768.f;
constexpr float kDeg2Rad = 3.14159265358979f / 180.f;
constexpr SkColor kGreen = SkColorSetRGB(0x00, 0xff, 0x00);
constexpr SkColor kYellow = SkColorSetRGB(0xff, 0xff, 0x00);
constexpr SkColor kCyanC = SkColorSetRGB(0x00, 0xff, 0xff);
constexpr SkColor kWhite = SK_ColorWHITE;
constexpr SkColor kMagenta = SkColorSetRGB(0xff, 0x94, 0xff);
constexpr SkColor kAmberC = SkColorSetRGB(0xe6, 0x80, 0x00);
constexpr SkColor kBlack = SK_ColorBLACK;

struct Symbol {
    std::string ident;
    bool has_location = false;
    double lat = 0, lon = 0;
    uint32_t type = 0;
    uint32_t type_pwp = 0;
    bool has_direction = false;
    double direction = 0;
    double length = 0;
    bool has_distance = false;
    double distance = 0;
    std::vector<std::string> constraints;
    std::vector<double> radials;
    std::vector<double> radii;
};

struct Vec {
    int type = 0;
    double slat = 0, slon = 0, elat = 0, elon = 0, clat = 0, clon = 0, sweep = 0;
};

double num(const json& j, const char* key, double fb = 0.0) {
    auto it = j.find(key);
    return (it != j.end() && it->is_number()) ? it->get<double>() : fb;
}

const json* find_topic(const Inputs& in, const std::string& topic) {
    if (auto* g = in.generic(topic)) return g;
    if (auto* b = in.bus(topic)) {
        if (b->is_object()) {
            auto it = b->find("data");
            if (it != b->end()) return &*it;
        }
        return b;
    }
    if (auto* eb = in.generic("EB_EVENTS"); eb && eb->is_object()) {
        auto d = eb->find("data");
        if (d != eb->end() && d->is_array()) {
            const json* found = nullptr;
            for (auto& ev : *d) {
                if (!ev.is_object()) continue;
                auto t = ev.find("topic");
                if (t != ev.end() && t->is_string() && t->get<std::string>() == topic) {
                    auto dd = ev.find("data");
                    if (dd != ev.end()) found = &*dd;
                }
            }
            return found;
        }
    }
    return nullptr;
}

std::vector<Vec> parse_vectors(const json* j) {
    std::vector<Vec> out;
    if (!j || !j->is_array()) return out;
    out.reserve(j->size());
    for (auto& v : *j) {
        if (!v.is_object()) continue;
        Vec x;
        x.type = (int)num(v, "type", -1);
        if (auto s = v.find("startPoint"); s != v.end() && s->is_object()) {
            x.slat = num(*s, "lat");
            x.slon = num(*s, "long");
        }
        if (auto s = v.find("endPoint"); s != v.end() && s->is_object()) {
            x.elat = num(*s, "lat");
            x.elon = num(*s, "long");
        }
        if (auto s = v.find("centrePoint"); s != v.end() && s->is_object()) {
            x.clat = num(*s, "lat");
            x.clon = num(*s, "long");
        }
        x.sweep = num(v, "sweepAngle");
        out.push_back(x);
    }
    return out;
}

std::vector<Symbol> parse_symbols(const json* j) {
    std::vector<Symbol> out;
    if (!j || !j->is_array()) return out;
    for (auto& v : *j) {
        if (!v.is_object()) continue;
        Symbol s;
        if (auto it = v.find("ident"); it != v.end() && it->is_string()) s.ident = it->get<std::string>();
        if (auto it = v.find("location"); it != v.end() && it->is_object()) {
            s.has_location = true;
            s.lat = num(*it, "lat");
            s.lon = num(*it, "long");
        }
        s.type = (uint32_t)num(v, "type");
        s.type_pwp = (uint32_t)num(v, "typePwp");
        if (auto it = v.find("direction"); it != v.end() && it->is_number()) {
            s.has_direction = true;
            s.direction = it->get<double>();
        }
        s.length = num(v, "length");
        if (auto it = v.find("distanceFromAirplane"); it != v.end() && it->is_number()) {
            s.has_distance = true;
            s.distance = it->get<double>();
        }
        if (auto it = v.find("constraints"); it != v.end() && it->is_array())
            for (auto& c : *it)
                if (c.is_string()) s.constraints.push_back(c.get<std::string>());
        if (auto it = v.find("radials"); it != v.end() && it->is_array())
            for (auto& c : *it) s.radials.push_back(c.is_number() ? c.get<double>() : NAN);
        if (auto it = v.find("radii"); it != v.end() && it->is_array())
            for (auto& c : *it) s.radii.push_back(c.is_number() ? c.get<double>() : NAN);
        out.push_back(std::move(s));
    }
    return out;
}

double dist_nm(double lat1, double lon1, double lat2, double lon2) {
    double p1 = lat1 * kDeg2Rad, p2 = lat2 * kDeg2Rad;
    double dp = p2 - p1, dl = (lon2 - lon1) * kDeg2Rad;
    double a = std::sin(dp / 2) * std::sin(dp / 2) + std::cos(p1) * std::cos(p2) * std::sin(dl / 2) * std::sin(dl / 2);
    return 3440.065 * 2.0 * std::atan2(std::sqrt(a), std::sqrt(1 - a));
}

double arinc_value(const Inputs& in, const std::string& name) {
    double raw = in.var(name, "number", 0.0);
    uint64_t u = (uint64_t)raw;
    uint32_t bits = (uint32_t)(u & 0xffffffffu);
    float f;
    std::memcpy(&f, &bits, 4);
    return f;
}

double diff_angle(double a, double b) {
    double d = std::fmod(b - a, 360.0);
    if (d > 180) d -= 360;
    if (d < -180) d += 360;
    return d;
}

SkPaint stroke_paint(SkColor c, float w) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setStyle(SkPaint::kStroke_Style);
    p.setColor(c);
    p.setStrokeWidth(w);
    p.setStrokeMiter(10.f);
    return p;
}

SkPaint fill_paint(SkColor c) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setStyle(SkPaint::kFill_Style);
    p.setColor(c);
    return p;
}

// Minimal SVG path reader for the relative/absolute M, L, H, V commands the ND's glyph paths use.
SkPath svg_path(const char* d) {
    SkPathBuilder path;
    const char* p = d;
    char cmd = 0;
    float cx = 0, cy = 0, sx = 0, sy = 0;
    auto skip = [&] {
        while (*p == ' ' || *p == ',') ++p;
    };
    auto read = [&](float& v) {
        skip();
        char* end;
        v = std::strtof(p, &end);
        p = end;
    };
    while (*p) {
        skip();
        if (!*p) break;
        if (std::isalpha((unsigned char)*p)) cmd = *p++;
        float a, b;
        switch (cmd) {
            case 'M':
                read(a); read(b); cx = sx = a; cy = sy = b; path.moveTo(cx, cy); cmd = 'L'; break;
            case 'm':
                read(a); read(b); cx += a; cy += b; sx = cx; sy = cy; path.moveTo(cx, cy); cmd = 'l'; break;
            case 'L': read(a); read(b); cx = a; cy = b; path.lineTo(cx, cy); break;
            case 'l': read(a); read(b); cx += a; cy += b; path.lineTo(cx, cy); break;
            case 'H': read(a); cx = a; path.lineTo(cx, cy); break;
            case 'h': read(a); cx += a; path.lineTo(cx, cy); break;
            case 'V': read(a); cy = a; path.lineTo(cx, cy); break;
            case 'v': read(a); cy += a; path.lineTo(cx, cy); break;
            case 'z': case 'Z': path.close(); cx = sx; cy = sy; cmd = 0; break;
            default: ++p; break;
        }
    }
    return path.detach();
}

SkPath clip_for_mode(int mode) {
    SkPathBuilder p;
    if (mode == 3) {
        p.moveTo(0, 312);
        p.rArcTo({492, 492}, 0, SkPathBuilder::kSmall_ArcSize, SkPathDirection::kCW, {768, 0});
        for (auto [x, y] : {std::pair{768.f, 562.f}, {648.f, 562.f}, {591.f, 625.f}, {591.f, 768.f}, {174.f, 768.f},
                            {174.f, 683.f}, {122.f, 625.f}, {0.f, 625.f}, {0.f, 312.f}})
            p.lineTo(x, y);
    } else if (mode == 4) {
        p.moveTo(45, 112);
        for (auto [x, y] : {std::pair{140.f, 112.f}, {280.f, 56.f}, {488.f, 56.f}, {628.f, 112.f}, {723.f, 112.f},
                            {723.f, 720.f}, {114.f, 720.f}, {114.f, 633.f}, {45.f, 633.f}})
            p.lineTo(x, y);
        p.close();
    } else {
        p.moveTo(45, 155);
        p.lineTo(282, 155);
        p.rArcTo({250, 250}, 0, SkPathBuilder::kSmall_ArcSize, SkPathDirection::kCW, {204, 0});
        for (auto [x, y] : {std::pair{723.f, 155.f}, {723.f, 562.f}, {648.f, 562.f}, {591.f, 625.f}, {591.f, 768.f},
                            {174.f, 768.f}, {174.f, 683.f}, {122.f, 625.f}, {45.f, 625.f}, {45.f, 155.f}})
            p.lineTo(x, y);
    }
    return p.detach();
}

struct Ctx {
    SkCanvas& c;
    const Frame& f;
    SkFont font21;
    SkFont font20;

    SkPoint pt(const Symbol& s) const { return f.project(s.lat, s.lon); }
    double rotation(double bearing) const { return bearing - f.up_true_deg; }

    void text(bool color_layer, float x, float y, const std::string& t, SkColor col, const SkFont& font) {
        if (t.empty()) return;
        if (!color_layer) {
            SkPaint p = stroke_paint(kBlack, 2.25f);
            a380::draw_simple_text(c, t.data(), t.size(), SkTextEncoding::kUTF8, x, y, font, p);
        } else {
            a380::draw_simple_text(c, t.data(), t.size(), SkTextEncoding::kUTF8, x, y, font, fill_paint(col));
        }
    }
};

void paint_airport_shape(SkCanvas& c, float x, float y, SkColor col, float w) {
    SkPaint p = stroke_paint(col, w);
    c.save();
    c.translate(x, y);
    prim::line(kPm, c, -13, 0, 13, 0, p);
    prim::line(kPm, c, 0, -13, 0, 13, p);
    c.rotate(45);
    prim::line(kPm, c, 0, -13, 0, 13, p);
    c.rotate(90);
    prim::line(kPm, c, 0, -13, 0, 13, p);
    c.restore();
}

void paint_waypoint_shape(SkCanvas& c, float x, float y, SkColor col, float w) {
    SkPaint p = stroke_paint(col, w);
    c.save();
    c.translate(x, y);
    c.rotate(45);
    prim::rect(kPm, c, SkRect::MakeLTRB(-4.5f, -4.5f, 4.5f, 4.5f), p);
    c.restore();
}

void paint_navaid_shape(SkCanvas& c, float x, float y, uint32_t type, SkColor col, float w) {
    SkPaint p = stroke_paint(col, w);
    c.save();
    c.translate(x, y);
    if (type & kVorDme) {
        prim::circle(kPm, c, 0, 0, 7, p);
        prim::line(kPm, c, 0, -15, 0, -7, p);
        prim::line(kPm, c, 0, 15, 0, 7, p);
        prim::line(kPm, c, -15, 0, -7, 0, p);
        prim::line(kPm, c, 15, 0, 7, 0, p);
    } else if (type & kVor) {
        prim::line(kPm, c, 0, -15, 0, 15, p);
        prim::line(kPm, c, -15, 0, 15, 0, p);
    } else if (type & kDme) {
        prim::circle(kPm, c, 0, 0, 7, p);
    } else if (type & kNdb) {
        SkPathBuilder t;
        t.moveTo(-10, 10);
        t.lineTo(0, -10);
        t.lineTo(10, 10);
        t.close();
        prim::path(kPm, c, t.detach(), p);
    }
    c.restore();
}

void paint_course_reversal(Ctx& cx, float x, float y, const Symbol& s) {
    bool left = s.type & kCourseReversalLeft;
    float arc_end = left ? -42.f : 42.f;
    double rot = s.has_direction ? cx.rotation(s.direction) : cx.rotation(0.0);
    SkPaint p = stroke_paint(kWhite, 1.75f);
    cx.c.save();
    cx.c.translate(x, y);
    cx.c.rotate((float)rot);
    SkPathBuilder head;
    head.moveTo(arc_end, 0);
    head.lineTo(arc_end - 4, -4);
    head.moveTo(arc_end, 0);
    head.lineTo(arc_end + 4, -4);
    prim::path(kPm, cx.c, head.detach(), p);
    SkPathBuilder arc;
    arc.moveTo(0, 0);
    arc.arcTo({21, 21}, 0, SkPathBuilder::kSmall_ArcSize, left ? SkPathDirection::kCCW : SkPathDirection::kCW, {left ? -42.f : 42.f, 0});
    prim::path(kPm, cx.c, arc.detach(), p);
    cx.c.restore();
}

void paint_waypoint_layer(Ctx& cx, const std::vector<Symbol>& data, bool color_layer) {
    const float w = color_layer ? 1.75f : 3.25f;
    for (auto& s : data) {
        SkPoint pt = cx.pt(s);
        float x = pt.x(), y = pt.y();
        if (s.type & (kAirport | kRunway)) {
            SkColor main = (s.type & kFlightPlan) ? kWhite : kMagenta;
            paint_airport_shape(cx.c, x, y, color_layer ? main : kBlack, w);
            cx.text(color_layer, x + 13, y + 18, s.ident, main, cx.font21);
        } else if (s.type & (kVorDme | kVor | kDme | kNdb)) {
            SkColor main = kBlack;
            if (color_layer) main = (s.type & kTuned) ? kCyanC : kMagenta;
            paint_navaid_shape(cx.c, x, y, s.type, main, w);
            cx.text(color_layer, x + 13, y + 17, s.ident, main, cx.font21);
        } else if (s.type & (kFixInfo | kFlightPlan)) {
            SkColor main = (s.type & (kActiveLegTermination | kSecondaryFlightPlan)) ? kWhite : kGreen;
            paint_waypoint_shape(cx.c, x, y, color_layer ? main : kBlack, w);
            cx.text(color_layer, x + 15, y + 17, s.ident, main, cx.font21);
            if (s.type & (kCourseReversalLeft | kCourseReversalRight)) paint_course_reversal(cx, x, y, s);
        } else {
            paint_waypoint_shape(cx.c, x, y, color_layer ? kMagenta : kBlack, w);
            cx.text(color_layer, x + 15, y + 17, s.ident, kMagenta, cx.font21);
        }
        if (color_layer && !s.constraints.empty()) {
            for (size_t i = 0; i < s.constraints.size(); ++i)
                cx.text(true, x + 15, y + 35 + 18.f * i, s.constraints[i], kMagenta, cx.font21);
        }
    }
}

void paint_constraint_layer(Ctx& cx, const std::vector<Symbol>& data, bool color_layer) {
    for (auto& s : data) {
        SkPoint pt = cx.pt(s);
        SkColor col = kBlack;
        if (color_layer) col = (s.type & kMagentaFlag) ? kMagenta : (s.type & kAmber) ? kAmberC : kWhite;
        prim::circle(kPm, cx.c, pt.x(), pt.y(), 14.f, stroke_paint(col, 1.75f));
    }
}

void paint_fixinfo_layer(Ctx& cx, const std::vector<Symbol>& data, bool color_layer) {
    const float dashes[2] = {15.f, 12.f};
    for (auto& s : data) {
        SkPoint pt = cx.pt(s);
        SkPaint p = stroke_paint(color_layer ? kCyanC : kBlack, color_layer ? 1.75f : 3.25f);
        p.setPathEffect(SkDashPathEffect::Make(dashes, 0));
        for (double radial : s.radials) {
            if (!std::isfinite(radial)) continue;
            double rot = cx.rotation(radial) * kDeg2Rad;
            prim::line(kPm, cx.c, pt.x(), pt.y(), pt.x() + (float)(std::sin(rot) * 800), pt.y() - (float)(std::cos(rot) * 800), p);
        }
        for (double radius : s.radii) {
            if (!std::isfinite(radius)) continue;
            prim::circle(kPm, cx.c, pt.x(), pt.y(), (float)(radius * cx.f.px_per_nm), p);
        }
    }
}

void paint_runway_layer(Ctx& cx, const std::vector<Symbol>& data, bool color_layer) {
    for (auto& s : data) {
        SkPoint pt = cx.pt(s);
        float x = pt.x(), y = pt.y();
        double rotation = cx.rotation(s.direction);
        SkPaint p = stroke_paint(color_layer ? kWhite : kBlack, color_layer ? 1.75f : 3.25f);
        cx.c.save();
        cx.c.translate(x, y);
        cx.c.rotate((float)rotation);
        cx.c.translate(-x, -y);
        if (cx.f.range_nm < 80) {
            float length = (float)(s.length * cx.f.px_per_nm);
            prim::line(kPm, cx.c, x - 5, y, x - 5, y - length, p);
            prim::line(kPm, cx.c, x + 5, y, x + 5, y - length, p);
        } else {
            prim::rect(kPm, cx.c, SkRect::MakeLTRB(x - 5, y - 12.5f, x + 5, y + 12.5f), p);
        }
        std::string icao = s.ident.substr(0, std::min<size_t>(4, s.ident.size()));
        std::string rwy = s.ident.size() > 4 ? s.ident.substr(4) : "";
        size_t cps = 0;
        for (unsigned char ch : rwy) cps += (ch & 0xC0) != 0x80;
        while (cps++ < 4) rwy += "\xC2\xA0";
        cx.c.save();
        cx.c.translate(x + 40, y - 20);
        cx.c.rotate((float)-rotation);
        cx.c.translate(-(x + 40), -(y - 20));
        SkFontMetrics m;
        cx.font20.getMetrics(&m);
        float mid = -(m.fAscent + m.fDescent) / 2.f + 4.f;
        for (int line = 0; line < 2; ++line) {
            const std::string& t = line == 0 ? icao : rwy;
            float ty = (line == 0 ? y - 30 : y - 10) + mid;
            float wdt = cx.font20.measureText(t.c_str(), t.size(), SkTextEncoding::kUTF8);
            float tx = x + 40 - wdt / 2;
            cx.text(color_layer, tx, ty, t, kWhite, cx.font20);
        }
        cx.c.restore();
        cx.c.restore();
    }
}

SkColor pwp_color(uint32_t type) {
    if (type & kMagentaFlag) return kMagenta;
    if (type & kAmber) return kAmberC;
    if (type & kCyan) return kCyanC;
    return kWhite;
}

SkPath decel_path() {
    SkPathBuilder p;
    p.addCircle(0, 0, 14.5125f);
    p.moveTo(2.3625f, -9.7875f);
    p.lineTo(-5.4f, -9.7875f);
    p.lineTo(-5.4f, 8.4375f);
    p.lineTo(2.3625f, 8.4375f);
    p.lineTo(5.0625f, 5.0625f);
    p.lineTo(5.0625f, -6.4125f);
    p.close();
    return p.detach();
}

void paint_pwp(Ctx& cx, const Symbol& s, bool color_layer, float x, float y, bool has_rot, double rot) {
    SkColor col = color_layer ? pwp_color(s.type) : kBlack;
    SkPaint p = stroke_paint(col, 1.75f);
    auto path = [&](const SkPath& pa, bool rotate) {
        cx.c.save();
        cx.c.translate(x, y);
        if (rotate && has_rot && rot != 0) cx.c.rotate((float)rot);
        prim::path(kPm, cx.c, pa, p);
        cx.c.restore();
    };
    auto cda = [&](const char* label, SkColor tcol) {
        prim::circle(kPm, cx.c, x, y, 14.f, p);
        cx.text(true, x - 5.5f, y + 6.5f, label, tcol, cx.font21);
    };
    static const SkPath soc = svg_path("M 0 0 h 22.2 l 19.8 -16.2 m -6 0 h 6 v 6");
    static const SkPath loc = svg_path("M -42 16.2 l 19.8 -16.2 h 22.2 m -4.2 -4.2 l 4.2 4.2 l -4.2 4.2");
    static const SkPath tod = svg_path("M 0 0 h 22.2 l 19.8 16.2 m -6 0 h 6 v -6");
    static const SkPath lod = svg_path("M -42 -16.2 l 19.8 16.2 h 22.2 m -4.2 -4.2 l 4.2 4.2 l -4.2 4.2");
    static const SkPath icp = svg_path("M -38 0 l 14 -17 v 34 l 14 -17 h 10 m -5 -5 l 5 5 l -5 5");
    static const SkPath eov = svg_path("M -15 15 v -15 h 30 v 15");
    static const SkPath dec = decel_path();
    uint32_t t = s.type_pwp;
    if (t & kPwpStartOfClimb) path(soc, false);
    else if (t & kPwpClimbLevelOff) path(loc, false);
    else if (t & kPwpTopOfDescent) path(tod, false);
    else if (t & kPwpDescentLevelOff) path(lod, false);
    else if (t & kPwpInterceptProfile) path(icp, false);
    else if (t & kPwpCdaFlap1) { if (color_layer) cda("1", col); else prim::circle(kPm, cx.c, x, y, 14.f, p); }
    else if (t & kPwpCdaFlap2) { if (color_layer) cda("2", col); else prim::circle(kPm, cx.c, x, y, 14.f, p); }
    else if (t & kPwpDecel) path(dec, false);
    else if (t & kPwpTimeMarker) { if (color_layer) { p.setColor(kGreen); prim::circle(kPm, cx.c, x, y, 14.f, p); } else prim::circle(kPm, cx.c, x, y, 14.f, p); }
    else if (t & kPwpSpeedChange) { if (color_layer) prim::circle(kPm, cx.c, x, y, 8.f, fill_paint(col)); }
    else if ((t & kPwpEndOfVd) && (s.type & kCyan)) path(eov, true);
}

void paint_pwp_layer(Ctx& cx, const std::vector<Symbol>& data, bool color_layer, double rot_track) {
    for (auto& s : data) {
        if (s.has_distance && s.distance != 0 && !s.has_location) {
            float dy = (float)(s.distance * cx.f.px_per_nm);
            cx.c.save();
            cx.c.translate(384.f, color_layer ? cx.f.centre.y() - 6.f : 620.f);
            cx.c.rotate((float)rot_track);
            paint_pwp(cx, s, color_layer, 0, -dy, false, 0);
            cx.c.restore();
        } else if (s.has_location) {
            SkPoint pt = cx.pt(s);
            bool has_rot = s.has_direction && s.direction != 0;
            paint_pwp(cx, s, color_layer, pt.x(), pt.y(), has_rot, has_rot ? cx.rotation(s.direction) : 0.0);
        }
    }
}

struct Group {
    const char* topic;
    SkColor color;
    bool dashed;
    int order;
};

void draw_vectors(Ctx& cx, const std::vector<Vec>& vs, SkColor col, bool dashed) {
    if (vs.empty()) return;
    const float dashes[2] = {15.f, 12.f};
    SkPaint p = stroke_paint(col, 1.75f);
    if (dashed) p.setPathEffect(SkDashPathEffect::Make(dashes, 0));
    SkPathBuilder lines;
    for (auto& v : vs) {
        if (v.type == 0) {
            SkPoint a = cx.f.project(v.slat, v.slon), b = cx.f.project(v.elat, v.elon);
            lines.moveTo(a);
            lines.lineTo(b);
        } else if (v.type == 1) {
            SkPoint a = cx.f.project(v.slat, v.slon), b = cx.f.project(v.elat, v.elon);
            float r = (float)(dist_nm(v.clat, v.clon, v.elat, v.elon) * cx.f.px_per_nm);
            SkPathBuilder arc;
            arc.moveTo(a);
            arc.arcTo({r, r}, 0, std::fabs(v.sweep) >= 180 ? SkPathBuilder::kLarge_ArcSize : SkPathBuilder::kSmall_ArcSize,
                      v.sweep > 0 ? SkPathDirection::kCW : SkPathDirection::kCCW, b);
            prim::path(kPm, cx.c, arc.detach(), p);
        }
    }
    prim::path(kPm, cx.c, lines.detach(), p);
}

}  // namespace

void draw_route(SkCanvas& canvas, const Inputs& in, const Frame& f) {
    if (!f.map_available) return;

    const char side = f.side == 2 ? 'R' : 'L';
    const std::string sd(1, side);

    const std::string other(1, side == 'L' ? 'R' : 'L');
    std::vector<Symbol> symbols = parse_symbols(find_topic(in, "A32NX_EFIS_" + sd + "_SYMBOLS"));
    if (symbols.empty()) symbols = parse_symbols(find_topic(in, "A32NX_EFIS_SYMBOLS_" + sd));
    if (symbols.empty()) symbols = parse_symbols(find_topic(in, "A32NX_EFIS_" + other + "_SYMBOLS"));
    if (symbols.empty()) {
        symbols = parse_symbols(find_topic(in, std::string("vdSymbols_") + side));
    }
    auto vec = [&](const char* suffix) {
        auto v = parse_vectors(find_topic(in, "A32NX_EFIS_VECTORS_" + sd + "_" + suffix));
        if (v.empty()) v = parse_vectors(find_topic(in, "A32NX_EFIS_VECTORS_" + other + "_" + suffix));
        return v;
    };
    std::vector<Vec> v_secondary = vec("SECONDARY"), v_active = vec("ACTIVE"), v_dashed = vec("DASHED"),
                     v_temp = vec("TEMPORARY"), v_missed = vec("MISSED"), v_alt = vec("ALTERNATE"),
                     v_eosid = vec("ACTIVE_EOSID");

    std::vector<Symbol> waypoints, fixinfo, constraints, runways, pwps;
    for (auto& s : symbols) {
        if (!s.constraints.empty() || (s.type & (kWaypoint | kFlightPlan | kFixInfo | kVorDme | kVor | kDme | kNdb | kAirport) && !(s.type & kRunway)))
            if (s.has_location) waypoints.push_back(s);
        if ((s.type & kFixInfo) && s.has_location) fixinfo.push_back(s);
        if ((s.type & kConstraint) && s.has_location) constraints.push_back(s);
        if ((s.type & kRunway) && s.has_location) runways.push_back(s);
        if ((s.type & (kCourseReversalLeft | kCourseReversalRight)) ||
            (s.type_pwp & (kPwpEndOfVd | kPwpStartOfClimb | kPwpClimbLevelOff | kPwpTopOfDescent | kPwpDescentLevelOff |
                           kPwpInterceptProfile | kPwpCdaFlap1 | kPwpCdaFlap2 | kPwpDecel | kPwpTimeMarker | kPwpSpeedChange)))
            pwps.push_back(s);
    }

    const bool any = !symbols.empty() || !v_active.empty() || !v_secondary.empty() || !v_dashed.empty() ||
                     !v_temp.empty() || !v_missed.empty() || !v_alt.empty() || !v_eosid.empty();
    if (!any) return;

    sk_sp<SkSurface> surf = SkSurfaces::Raster(SkImageInfo::MakeN32Premul(768, 768));
    if (!surf) return;
    SkCanvas& c = *surf->getCanvas();
    c.clear(SK_ColorTRANSPARENT);
    c.clipPath(clip_for_mode(f.mode), true);
    if (f.mode == 4 || f.mode == 3) c.clipPath(clip_for_mode(2), true);
    if (f.mode == 4) c.clipPath(clip_for_mode(3), true);

    Ctx cx{c, f, a380::font("Ecam", 21.f), a380::font("Ecam", 20.f)};

    double rot_track = 0;
    {
        std::string n = std::string("L:A32NX_ADIRS_IR_") + (f.side == 2 ? "2" : "1");
        rot_track = diff_angle(arinc_value(in, n + "_HEADING"), arinc_value(in, n + "_TRACK"));
        if (!std::isfinite(rot_track)) rot_track = 0;
    }

    paint_constraint_layer(cx, constraints, false);
    paint_constraint_layer(cx, constraints, true);

    draw_vectors(cx, v_secondary, kWhite, false);
    draw_vectors(cx, v_active, kGreen, false);
    draw_vectors(cx, v_dashed, kGreen, true);
    draw_vectors(cx, v_temp, kYellow, true);
    draw_vectors(cx, v_missed, kCyanC, false);
    draw_vectors(cx, v_alt, kCyanC, true);
    draw_vectors(cx, v_eosid, kYellow, false);

    paint_waypoint_layer(cx, waypoints, false);
    paint_waypoint_layer(cx, waypoints, true);
    paint_fixinfo_layer(cx, fixinfo, false);
    paint_fixinfo_layer(cx, fixinfo, true);
    paint_runway_layer(cx, runways, false);
    paint_runway_layer(cx, runways, true);
    paint_pwp_layer(cx, pwps, false, rot_track);
    paint_pwp_layer(cx, pwps, true, rot_track);

    sk_sp<SkImage> img = surf->makeImageSnapshot();
    canvas.drawImage(img, 0, 0, SkSamplingOptions(SkFilterMode::kLinear), nullptr);
}

namespace {

bool inside(float x, float y, const std::vector<SkPoint>& poly) {
    bool in = false;
    for (size_t i = 0, j = poly.size() - 1; i < poly.size(); j = i++) {
        const SkPoint &a = poly[i], &b = poly[j];
        if (((a.fY > y) != (b.fY > y)) && (x < (b.fX - a.fX) * (y - a.fY) / (b.fY - a.fY) + a.fX)) in = !in;
    }
    return in;
}

bool towards_centre(float x, float y, const std::vector<SkPoint>& poly, SkPoint& out) {
    for (size_t i = 0, j = poly.size() - 1; i < poly.size(); j = i++) {
        const SkPoint &a = poly[j], &b = poly[i];
        float rx = -x, ry = -y, sx = b.fX - a.fX, sy = b.fY - a.fY;
        float d = rx * sy - ry * sx;
        if (std::fabs(d) < 1e-6f) continue;
        float qx = a.fX - x, qy = a.fY - y;
        float t = (qx * sy - qy * sx) / d;
        float u = (qx * ry - qy * rx) / d;
        if (t >= 0.f && t <= 1.f && u >= 0.f && u <= 1.f) {
            out = {x + t * rx, y + t * ry};
            return true;
        }
    }
    return false;
}

const std::vector<SkPoint>& arc_mask() {
    static const std::vector<SkPoint> m = {{-384, -83}, {-384, 227}, {-264, 227}, {-210, 286}, {-210, 370}, {210, 370}, {210, 227}, {267, 166},
                                           {384, 166}, {384, -83}, {340, -128}, {300, -163}, {240, -204.5f}, {180, -233}, {100, -255}, {0, -265},
                                           {-100, -255}, {-180, -233}, {-240, -204.5f}, {-300, -163}, {-340, -128}, {-384, -83}};
    return m;
}

const std::vector<SkPoint>& rose_mask() {
    static const std::vector<SkPoint> m = {{-340, -227}, {-103, -227}, {-50, -244}, {0, -250}, {50, -244}, {103, -227}, {340, -227}, {340, 180},
                                           {267, 180},   {210, 241},   {210, 383}, {-210, 383}, {-210, 300}, {-264, 241}, {-340, 241}, {-340, -227}};
    return m;
}

void paint_intruder(SkCanvas& c, bool color_layer, float x, float y, int level, double vs, double rel_alt, const SkFont& font) {
    constexpr float kDh = 18.f, kDw = 12.f;
    SkColor col = level == 2 ? SkColorSetRGB(0xe3, 0x8c, 0x56) : level == 3 ? SkColorSetRGB(0xff, 0x00, 0x00) : SK_ColorWHITE;
    SkColor draw = color_layer ? col : SkColorSetRGB(0x04, 0x04, 0x05);
    float lw = color_layer ? 1.6f : 3.5f;
    SkPaint st = stroke_paint(draw, lw);
    SkPaint fl = fill_paint(draw);
    c.save();
    c.translate(x, y);
    SkPathBuilder diamond;
    diamond.moveTo(0, -kDh / 2).lineTo(-kDw / 2, 0).lineTo(0, kDh / 2).lineTo(kDw / 2, 0).lineTo(0, -kDh / 2);
    switch (level) {
        case 0:
            a380::draw_path_cached(c, diamond.detach(), st);
            break;
        case 1: {
            SkPath p = diamond.detach();
            a380::draw_path_cached(c, p, st);
            a380::draw_path_cached(c, p, fl);
            break;
        }
        case 2:
            c.drawCircle(0, 0, 7.8f, fl);
            c.drawCircle(0, 0, 7.8f, st);
            break;
        case 3:
            c.drawRect(SkRect::MakeXYWH(-7.8f, -7.8f, 15.6f, 15.6f), fl);
            c.drawRect(SkRect::MakeXYWH(-7.8f, -7.8f, 15.6f, 15.6f), st);
            break;
        default:
            break;
    }
    constexpr float kOff = 7.f, kAw = 5.f, kAh = 5.5f;
    float ax = kDw / 2 + kOff;
    if (vs <= -500 || vs >= 500) {
        SkPathBuilder l;
        l.moveTo(ax - lw / 4, kDh / 2 - kAh / 2).lineTo(ax - lw / 4, -kDh / 2 + kAh / 2);
        a380::draw_path_cached(c, l.detach(), st);
        SkPathBuilder h;
        if (vs <= -500) h.moveTo(ax, kDh / 2).lineTo(ax - kAw / 2, kDh / 2 - kAh).lineTo(ax + kAw / 2, kDh / 2 - kAh).close();
        else h.moveTo(ax, -kDh / 2).lineTo(ax - kAw / 2, -kDh / 2 + kAh).lineTo(ax + kAw / 2, -kDh / 2 + kAh).close();
        SkPath hp = h.detach();
        a380::draw_path_cached(c, hp, fl);
        a380::draw_path_cached(c, hp, st);
    }
    c.restore();
    int a = (int)std::lround(std::fabs(rel_alt));
    std::string t = std::string(rel_alt > 0 ? "+" : "-") + (a < 10 ? "0" : "") + std::to_string(a);
    float ty = y + (rel_alt > 0 ? -12.f : 25.5f);
    if (!color_layer) a380::draw_simple_text(c, t.data(), t.size(), SkTextEncoding::kUTF8, x - 24, ty, font, stroke_paint(kBlack, 2.25f));
    else a380::draw_simple_text(c, t.data(), t.size(), SkTextEncoding::kUTF8, x - 24, ty, font, fill_paint(col));
}

}  // namespace

void draw_traffic(SkCanvas& canvas, const Inputs& in, const Frame& f) {
    if (f.mode == 4) return;
    const json* list = find_topic(in, f.side == 2 ? "A32NX_TCAS_R_TRAFFIC" : "A32NX_TCAS_L_TRAFFIC");
    if (!list || !list->is_array() || list->empty()) return;
    const std::vector<SkPoint>& mask = f.mode == 3 ? arc_mask() : rose_mask();
    struct Shown {
        SkPoint p;
        int level;
        double vs, alt;
    };
    std::vector<Shown> shown;
    for (const json& t : *list) {
        if (!t.is_object()) continue;
        SkPoint p = f.project(num(t, "lat"), num(t, "lon"));
        float x = p.fX - f.centre.fX, y = p.fY - f.centre.fY;
        int level = (int)num(t, "intrusionLevel");
        if (!inside(x, y, mask)) {
            if (level < 2) continue;
            SkPoint q;
            if (towards_centre(x, y, mask, q)) x = q.fX, y = q.fY;
        }
        shown.push_back({{x + f.centre.fX, y + f.centre.fY}, level, num(t, "vertSpeed"), num(t, "relativeAlt")});
    }
    SkFont font = a380::font("Ecam", 21.f);
    for (bool color_layer : {false, true})
        for (auto& s : shown) paint_intruder(canvas, color_layer, s.p.fX, s.p.fY, s.level, s.vs, s.alt, font);
}

}  // namespace a380::nd
