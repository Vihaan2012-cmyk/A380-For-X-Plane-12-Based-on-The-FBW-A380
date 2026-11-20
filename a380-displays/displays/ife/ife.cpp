#include <algorithm>
#include <cmath>
#include <cstdlib>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>

#include "include/core/SkPaint.h"
#include "include/core/SkPathBuilder.h"
#include "include/effects/SkDashPathEffect.h"

#include "a380/assets.h"
#include "a380/display.h"
#include "a380/draw.h"
#include "a380/fonts.h"
#include "nlohmann/json.hpp"

namespace {

namespace fs = std::filesystem;
using a380::rgb;

constexpr float W = 512.f, H = 256.f;
constexpr double kRad = 3.14159265358979323846 / 180.0, kNm = 3440.065;
constexpr double kPageSeconds = 12.0;
constexpr double kDrawEveryMs = 1000.0;

struct Theme {
    std::string airline, welcome = "Welcome aboard";
    SkColor background = rgb(0x0b1626), accent = rgb(0xc9a45c), text = rgb(0xffffff), land = rgb(0x1f3b2c),
            sea = rgb(0x0d2340);
};

SkColor parse_color(const std::string& s, SkColor fallback) {
    if (s.size() == 7 && s[0] == '#') return rgb((uint32_t)std::strtoul(s.c_str() + 1, nullptr, 16));
    return fallback;
}

fs::path ife_dir() {
    for (auto& c : a380::assets::ife_dirs()) {
        std::error_code ec;
        if (fs::exists(fs::path(c) / "land.js", ec)) return c;
    }
    return {};
}

std::string slurp(const fs::path& p) {
    std::ifstream f(p, std::ios::binary);
    std::stringstream ss;
    ss << f.rdbuf();
    return ss.str();
}

const Theme& theme() {
    static Theme t = [] {
        Theme th;
        fs::path d = ife_dir();
        if (d.empty()) return th;
        std::string s = slurp(d / "theme" / "theme.json");
        auto j = nlohmann::json::parse(s, nullptr, false);
        if (!j.is_object()) return th;
        auto str = [&](const char* k, std::string def) { return j.contains(k) && j[k].is_string() ? j[k].get<std::string>() : def; };
        th.airline = str("airline", "");
        th.welcome = str("welcome", th.welcome);
        th.background = parse_color(str("background", ""), th.background);
        th.accent = parse_color(str("accent", ""), th.accent);
        th.text = parse_color(str("text", ""), th.text);
        th.land = parse_color(str("land", ""), th.land);
        th.sea = parse_color(str("sea", ""), th.sea);
        return th;
    }();
    return t;
}

const std::vector<std::vector<double>>& land() {
    static std::vector<std::vector<double>> rings = [] {
        std::vector<std::vector<double>> out;
        fs::path d = ife_dir();
        if (d.empty()) return out;
        std::string s = slurp(d / "land.js");
        size_t a = s.find('[');
        size_t b = s.rfind(']');
        if (a == std::string::npos || b == std::string::npos) return out;
        auto j = nlohmann::json::parse(s.substr(a, b - a + 1), nullptr, false);
        if (!j.is_array()) return out;
        for (auto& r : j) out.push_back(r.get<std::vector<double>>());
        return out;
    }();
    return rings;
}

double distance_nm(double la1, double lo1, double la2, double lo2) {
    double dp = (la2 - la1) * kRad, dl = (lo2 - lo1) * kRad;
    double h = std::pow(std::sin(dp / 2), 2) + std::cos(la1 * kRad) * std::cos(la2 * kRad) * std::pow(std::sin(dl / 2), 2);
    return 2 * kNm * std::asin(std::min(1.0, std::sqrt(h)));
}

double wrap_lon(double lon, double centre) { return std::fmod(std::fmod(lon - centre, 360.0) + 540.0, 360.0) - 180.0 + centre; }

std::string group(long long v) {
    std::string s = std::to_string(std::llabs(v));
    for (int i = (int)s.size() - 3; i > 0; i -= 3) s.insert(i, ",");
    return (v < 0 ? "-" : "") + s;
}

bool arinc_degrees(double raw, double& out) {
    if (raw < 1e4) {
        out = raw;
        return true;
    }
    uint64_t u = (uint64_t)raw;
    int ssm = (int)((u >> 32) & 3);
    uint32_t lo = (uint32_t)(u & 0xffffffffu);
    float f;
    std::memcpy(&f, &lo, 4);
    out = f;
    return ssm == 3 || ssm == 2;
}

long long jsround(double v) { return (long long)std::floor(v + 0.5); }

std::string fmt_time(double h) {
    if (!std::isfinite(h) || h <= 0) return "--:--";
    long long m = jsround(h * 60);
    char b[32];
    std::snprintf(b, sizeof b, "%lldh %02lldm", m / 60, m % 60);
    return b;
}

SkPaint fill(SkColor c) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(c);
    return p;
}

void text(SkCanvas& c, const std::string& s, float x, float y, float px, bool bold, SkColor col, a380::Align al) {
    SkFont f = a380::font("sans-serif", px, bold);
    f.setHinting(SkFontHinting::kNormal);
    a380::draw_text(c, s, x, y, f, fill(col), al);
}

class Ife : public a380::Display {
public:
    SkISize size() const override { return {512, 256}; }

    void draw(SkCanvas& canvas, const a380::Inputs& in) override {
        const Theme& t = theme();
        double lat = in.var("GPS POSITION LAT", "degrees", 0), lon = in.var("GPS POSITION LON", "degrees", 0);
        double dlat = 0, dlon = 0;
        bool has_dest = arinc_degrees(in.var("L:A32NX_FM1_DEST_LAT", "number", 0), dlat) &&
                        arinc_degrees(in.var("L:A32NX_FM1_DEST_LONG", "number", 0), dlon) && (dlat != 0 || dlon != 0);
        if (!has_dest) {
            has_dest = arinc_degrees(in.var("L:A32NX_EGPWC_DEST_LAT", "number", 0), dlat) &&
                       arinc_degrees(in.var("L:A32NX_EGPWC_DEST_LONG", "number", 0), dlon) && (dlat != 0 || dlon != 0);
        }
        double gs = in.var("GPS GROUND SPEED", "knots", 0);
        double alt = in.var("INDICATED ALTITUDE", "feet", 0);
        double hdg = in.var("PLANE HEADING DEGREES TRUE", "degrees", 0);
        bool has_oat = in.has_var("AMBIENT TEMPERATURE", "celsius");
        double oat = in.var("AMBIENT TEMPERATURE", "celsius", 0);
        bool belt = in.var("CABIN SEATBELTS ALERT SWITCH:1", "bool", 0) != 0;
        double nsm = in.var("L:XMLVAR_SWITCH_OVHD_INTLT_NOSMOKING_Position", "number", 0);
        bool gear_down = in.var("GEAR HANDLE POSITION", "bool", 0) != 0;
        bool smoking = nsm == 0 || (nsm == 1 && gear_down);

        if (started_ms_ < 0) started_ms_ = in.now_ms();
        double secs = in.has_env_number("connectedAt.perf") && in.has_env_number("frozenAt.perf")
                          ? (in.env_number("frozenAt.perf") - in.env_number("connectedAt.perf")) / 1000.0
                          : (in.now_ms() - started_ms_) / 1000.0;
        if (in.has_env_number("frozenAt.frames") && in.has_env_number("frozenAt.perf") && in.env_number("frozenAt.frames") > 0) {
            double frame_ms = in.env_number("frozenAt.perf") / in.env_number("frozenAt.frames");
            if (std::isfinite(frame_ms) && frame_ms > 0) {
                double interval = std::ceil(kDrawEveryMs / frame_ms) * frame_ms / 1000.0;
                secs = std::floor(secs / interval) * interval;
            }
        }
        if (!std::isfinite(secs) || secs < 0) secs = 0;
        int page = (int)(std::fmod(std::floor(secs / kPageSeconds), 4.0));
        static const char* pages[] = {"welcome", "map", "info", "map"};

        SkPaint bg = fill(t.background);
        canvas.drawRect(SkRect::MakeWH(W, H), bg);
        std::string pg = pages[page];
        if (pg == "welcome") draw_welcome(canvas, t);
        else if (pg == "map") draw_map(canvas, t, lat, lon, has_dest, dlat, dlon, gs, alt, hdg);
        else draw_info(canvas, t, lat, lon, has_dest, dlat, dlon, gs, alt, has_oat, oat);

        std::vector<std::string> items;
        if (belt) items.push_back("FASTEN SEAT BELT");
        if (smoking) items.push_back("NO SMOKING");
        if (!items.empty()) {
            std::string s;
            for (size_t i = 0; i < items.size(); i++) {
                if (i) s += "     \xE2\x80\xA2     ";
                s += items[i];
            }
            SkPaint p;
            p.setColor(SkColorSetARGB(184, 0, 0, 0));
            canvas.drawRect(SkRect::MakeXYWH(0, H - 30, W, 30), p);
            text(canvas, s, W / 2, H - 10, 14, true, t.accent, a380::Align::Center);
        }
    }

private:
    double started_ms_ = -1.0;

    void draw_welcome(SkCanvas& c, const Theme& t) {
        text(c, t.welcome, W / 2, 120, 26, true, t.text, a380::Align::Center);
        if (!t.airline.empty()) text(c, t.airline, W / 2, 155, 18, false, t.accent, a380::Align::Center);
    }

    void draw_info(SkCanvas& c, const Theme& t, double lat, double lon, bool has_dest, double dlat, double dlon, double gs,
                   double alt, bool has_oat, double oat) {
        bool togo_ok = has_dest;
        double togo = togo_ok ? distance_nm(lat, lon, dlat, dlon) : 0;
        std::vector<std::pair<std::string, std::string>> rows = {
            {"Altitude", group(jsround(alt)) + " ft  /  " + group(jsround(alt * 0.3048)) + " m"},
            {"Ground speed", std::to_string(jsround(gs)) + " kt  /  " + std::to_string(jsround(gs * 1.852)) + " km/h"},
            {"Outside air", has_oat ? std::to_string(jsround(oat)) + " \xC2\xB0" "C" : "--"},
            {"Distance to go", togo_ok ? group(jsround(togo)) + " nm" : "--"},
            {"Time to destination", togo_ok && gs > 50 ? fmt_time(togo / gs) : "--:--"},
        };
        c.drawRect(SkRect::MakeWH(W, 34), fill(t.accent));
        text(c, t.airline.empty() ? "Flight information" : t.airline + "  \xC2\xB7  Flight information", 14, 23, 18, true,
             t.background, a380::Align::Left);
        for (size_t i = 0; i < rows.size(); i++) {
            float y = 66.f + 38.f * (float)i;
            text(c, rows[i].first, 20, y, 16, false, t.accent, a380::Align::Left);
            text(c, rows[i].second, W - 20, y, 16, false, t.text, a380::Align::Right);
        }
    }

    void draw_map(SkCanvas& c, const Theme& t, double lat, double lon, bool has_dest, double dlat, double dlon, double gs,
                  double alt, double hdg) {
        double olat = lat, olon = lon;
        std::vector<double> lons{wrap_lon(lon, lon), wrap_lon(olon, lon)}, lats{lat, olat};
        if (has_dest) {
            lons.push_back(wrap_lon(dlon, lon));
            lats.push_back(dlat);
        }
        double lomax = *std::max_element(lons.begin(), lons.end()), lomin = *std::min_element(lons.begin(), lons.end());
        double lamax = *std::max_element(lats.begin(), lats.end()), lamin = *std::min_element(lats.begin(), lats.end());
        double span = std::max({lomax - lomin, (lamax - lamin) * 2, 12.0}) * 1.3;
        span = std::min(span, 360.0);
        double cx = (lomax + lomin) / 2, cy = (lamax + lamin) / 2;
        double k = W / span;
        auto px = [&](double lo) { return W / 2 + (wrap_lon(lo, cx) - cx) * k; };
        auto py = [&](double la) { return H / 2 - (la - cy) * k; };
        c.drawRect(SkRect::MakeWH(W, H), fill(t.sea));
        SkPaint lp = fill(t.land);
        for (auto& ring : land()) {
            SkPathBuilder b;
            bool have_prev = false;
            double prev = 0;
            for (size_t i = 0; i + 1 < ring.size(); i += 2) {
                double x = px(ring[i]), y = py(ring[i + 1]);
                if (!have_prev || std::fabs(x - prev) > W / 2) b.moveTo((float)x, (float)y);
                else b.lineTo((float)x, (float)y);
                prev = x;
                have_prev = true;
            }
            c.drawPath(b.detach(), lp);
        }
        if (has_dest) {
            SkPaint sp;
            sp.setAntiAlias(true);
            sp.setStyle(SkPaint::kStroke_Style);
            sp.setStrokeWidth(2);
            sp.setColor(t.accent);
            const SkScalar on_off[] = {6, 4};
            sp.setPathEffect(SkDashPathEffect::Make(SkSpan<const SkScalar>(on_off, 2), 0));
            SkPathBuilder b;
            b.moveTo((float)px(olon), (float)py(olat));
            b.lineTo((float)px(lon), (float)py(lat));
            b.lineTo((float)px(dlon), (float)py(dlat));
            c.drawPath(b.detach(), sp);
            SkPathBuilder d;
            d.addCircle((float)px(dlon), (float)py(dlat), 4);
            c.drawPath(d.detach(), fill(t.accent));
        }
        c.save();
        c.translate((float)px(lon), (float)py(lat));
        c.rotate((float)hdg);
        SkPathBuilder b;
        b.moveTo(0, -11).lineTo(3, -3).lineTo(11, 3).lineTo(3, 2).lineTo(2, 8).lineTo(5, 11).lineTo(-5, 11).lineTo(-2, 8).lineTo(-3, 2).lineTo(-11, 3).lineTo(-3, -3).close();
        c.drawPath(b.detach(), fill(t.text));
        c.restore();

        SkPaint strip;
        strip.setColor(SkColorSetARGB(140, 0, 0, 0));
        c.drawRect(SkRect::MakeXYWH(0, H - 30, W, 30), strip);
        double togo = has_dest ? distance_nm(lat, lon, dlat, dlon) : 0;
        std::string eta = has_dest && gs > 50 ? fmt_time(togo / gs) : "--:--";
        text(c, group(jsround(alt)) + " ft", 10, H - 10, 14, false, t.text, a380::Align::Left);
        text(c, std::to_string(jsround(gs)) + " kt", 140, H - 10, 14, false, t.text, a380::Align::Left);
        text(c, has_dest ? group(jsround(togo)) + " nm to go" : "", 240, H - 10, 14, false, t.text, a380::Align::Left);
        text(c, eta, W - 10, H - 10, 14, false, t.text, a380::Align::Right);
    }
};

std::unique_ptr<a380::Display> make(std::string_view) { return std::make_unique<Ife>(); }
A380_REGISTER_DISPLAY("ife", make);

}  // namespace
