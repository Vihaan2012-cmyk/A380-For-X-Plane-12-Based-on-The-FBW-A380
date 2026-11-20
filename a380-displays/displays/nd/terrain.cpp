#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <string>
#include <vector>

#include "include/core/SkBitmap.h"
#include "include/core/SkCanvas.h"
#include "include/core/SkImage.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkRect.h"

#include "a380/inputs.h"
#include "nd/nd_layers.h"
#include "nd/vd.h"

namespace a380::nd {
namespace {

// Band codes from crates/terrain_map bands.rs.
enum Band : uint8_t {
    kClear = 0, kGreenLow, kGreenHigh, kGreenSolid, kAmberLow, kAmberHigh, kRedHigh,
    kWater, kUnknown, kCautionSolid, kWarningSolid, kBandCount
};

struct Style {
    uint32_t rgba;  // 0xRRGGBB
    int density16;  // GENERIC: share of a 4x4 ordered-dither cell that is lit
};

const std::array<Style, kBandCount> kStyles = {{
    {0x000000, 0},
    {0x00ff00, 4},
    {0x00ff00, 8},
    {0x00ff00, 16},
    {0xffff32, 4},
    {0xffff32, 8},
    {0xff0000, 8},
    {0x00ffff, 3},
    {0xff94ff, 8},
    {0xffff32, 16},
    {0xff0000, 16},
}};

const int kBayer4[4][4] = {{0, 8, 2, 10}, {12, 4, 14, 6}, {3, 11, 1, 9}, {15, 7, 13, 5}};

constexpr double kPi = 3.14159265358979323846;
constexpr float kMapTop = 128.f;
constexpr float kMapBottom = 720.f;
constexpr float kMapWidth = 768.f;

int b64(char c) {
    if (c >= 'A' && c <= 'Z') return c - 'A';
    if (c >= 'a' && c <= 'z') return c - 'a' + 26;
    if (c >= '0' && c <= '9') return c - '0' + 52;
    if (c == '+') return 62;
    if (c == '/') return 63;
    return -1;
}

std::vector<uint8_t> decode_base64(const std::string& s) {
    std::vector<uint8_t> out;
    out.reserve(s.size() * 3 / 4);
    uint32_t acc = 0;
    int bits = 0;
    for (char c : s) {
        int v = b64(c);
        if (v < 0) continue;
        acc = (acc << 6) | (uint32_t)v;
        bits += 6;
        if (bits >= 8) {
            bits -= 8;
            out.push_back((uint8_t)((acc >> bits) & 0xff));
        }
    }
    return out;
}

double num(const nlohmann::json& j, const char* k, double d = 0.0) {
    auto it = j.find(k);
    return it != j.end() && it->is_number() ? it->get<double>() : d;
}

struct Grid {
    double gen = -1;
    int n = 0;
    double lat0 = 0, lon0 = 0, dlat = 1, dlon = 1;
    std::vector<uint8_t> bands;
};

struct NdCache {
    Grid grid;
    double key[8] = {};
    sk_sp<SkImage> image;
    SkRect dst{};
};

NdCache g_cache[2];

bool load_grid(const nlohmann::json& j, Grid& g) {
    double gen = num(j, "gen", -1);
    if (gen == g.gen && !g.bands.empty()) return true;
    auto b = j.find("bands");
    if (b == j.end() || !b->is_string()) return false;
    g.gen = gen;
    g.n = (int)num(j, "n");
    g.lat0 = num(j, "lat0");
    g.lon0 = num(j, "lon0");
    g.dlat = num(j, "dlat", 1);
    g.dlon = num(j, "dlon", 1);
    g.bands = decode_base64(b->get<std::string>());
    return g.n > 0 && (int)g.bands.size() >= g.n * g.n && g.dlat > 0 && g.dlon > 0;
}

sk_sp<SkImage> rasterise(const Grid& g, const Frame& f, float radius_px, float scale, SkRect& dst) {
    dst = SkRect::MakeLTRB(0, kMapTop, kMapWidth, kMapBottom);
    const int w = (int)std::lround(dst.width() * scale), h = (int)std::lround(dst.height() * scale);
    if (w <= 0 || h <= 0) return nullptr;
    SkBitmap bm;
    bm.allocPixels(SkImageInfo::MakeN32Premul(w, h));
    bm.eraseColor(SK_ColorTRANSPARENT);
    const double up = f.up_true_deg * kPi / 180.0;
    const double su = std::sin(up), cu = std::cos(up);
    const double coslat = std::max(0.01, std::cos(f.ref_lat * kPi / 180.0));
    const double r2 = (double)radius_px * radius_px;
    const double wrap = 360.0 / g.dlon;
    for (int py = 0; py < h; ++py) {
        uint32_t* row = bm.getAddr32(0, py);
        const double y = dst.top() + (py + 0.5) / scale;
        const double dy = y - f.centre.y();
        for (int px = 0; px < w; ++px) {
            const double x = (px + 0.5) / scale;
            const double dx = x - f.centre.x();
            if (dx * dx + dy * dy > r2) continue;
            const double xn = dx / f.px_per_nm, yn = dy / f.px_per_nm;
            const double north = -(yn * cu + xn * su);
            const double east = xn * cu - yn * su;
            const double lat = f.ref_lat + north / 60.0;
            const double lon = f.ref_lon + east / (60.0 * coslat);
            const int j = (int)std::lround((lat - g.lat0) / g.dlat);
            double fi = std::round((lon - g.lon0) / g.dlon);
            if (fi < 0) fi += std::round(wrap);
            else if (fi >= std::round(wrap)) fi -= std::round(wrap);
            const int i = (int)fi;
            if (i < 0 || j < 0 || i >= g.n || j >= g.n) continue;
            const uint8_t band = g.bands[(size_t)j * g.n + i];
            if (band == kClear || band >= kBandCount) continue;
            const Style& s = kStyles[band];
            if (kBayer4[py & 3][px & 3] >= s.density16) continue;
            row[px] = SkPreMultiplyARGB(255, (s.rgba >> 16) & 0xff, (s.rgba >> 8) & 0xff, s.rgba & 0xff);
        }
    }
    bm.setImmutable();
    return bm.asImage();
}

void draw_nd_terrain(SkCanvas& canvas, const Inputs& in, const Frame& f) {
    if (f.mode == 4 || !f.map_available) return;
    const char side = f.side == 2 ? 'R' : 'L';
    const nlohmann::json* j = in.bus(std::string("a380.terrain.nd.") + side);
    if (!j || !j->is_object() || j->value("off", false)) return;
    NdCache& c = g_cache[f.side == 2 ? 1 : 0];
    if (!load_grid(*j, c.grid)) return;
    const float radius = f.mode == 3 ? 498.f : 250.f;
    const float scale = std::max(0.25f, canvas.getTotalMatrix().getScaleX());
    const double key[8] = {c.grid.gen, f.ref_lat, f.ref_lon, f.up_true_deg, f.px_per_nm, f.centre.y(), scale, (double)f.mode};
    if (!c.image || !std::equal(key, key + 8, c.key)) {
        std::copy(key, key + 8, c.key);
        c.image = rasterise(c.grid, f, radius, scale, c.dst);
    }
    if (!c.image) return;
    canvas.save();
    canvas.clipRect(SkRect::MakeLTRB(0, kMapTop, kMapWidth, kMapBottom));
    canvas.drawImageRect(c.image, c.dst, SkSamplingOptions(SkFilterMode::kNearest));
    canvas.restore();
}

void draw_vd_terrain(SkCanvas& canvas, const Inputs& in, const Frame& f) {
    static const double kRanges[] = {-1, 10, 20, 40, 80, 160, 320, 640};
    const double nd_range = kRanges[std::clamp(f.range_index, 0, 7)];
    if (f.mode < 2 || f.mode == 4 || nd_range < 0) return;
    const char side = f.side == 2 ? 'R' : 'L';
    const nlohmann::json* j = in.bus(std::string("a380.terrain.vd.") + side);
    if (!j || !j->is_object() || j->value("off", false)) return;
    auto elev = j->find("elev");
    auto kind = j->find("kind");
    if (elev == j->end() || !elev->is_array() || kind == j->end() || !kind->is_string()) return;
    const std::string k = kind->get<std::string>();
    const double step = num(*j, "stepNm", 0);
    if (step <= 0) return;
    const double grey = num(*j, "grey", -1);
    const double vd_range = f.mode == 3 ? std::max(10.0, std::min(nd_range, 160.0)) : std::max(5.0, std::min(nd_range / 2.0, 160.0));
    const VdVerticalRange vr = vd_vertical_range(in, f);
    if (vr.upper <= vr.lower) return;
    const double ft_per_px = (vr.upper - vr.lower) / 200.0;
    const float half = (float)(step / vd_range * 540.0 * 0.5);
    canvas.save();
    canvas.clipRect(SkRect::MakeXYWH(150, 800, 540, 200));
    SkPaint p;
    p.setAntiAlias(false);
    for (size_t i = 0; i < elev->size() && i < k.size(); ++i) {
        const double d = i * step;
        if (d > vd_range + step) break;
        const float x = (float)(150.0 + d / vd_range * 540.0);
        uint32_t rgb;
        float top;
        if (k[i] == 'U' || !(*elev)[i].is_number()) {
            rgb = 0xff94ff;
            top = 800.f;
        } else {
            const double h = (*elev)[i].get<double>();
            top = (float)(800.0 + (vr.upper - h) / ft_per_px);
            if (k[i] == 'W') rgb = 0x00ffff;
            else if (grey >= 0 && d >= grey) rgb = 0x4e4e61;
            else rgb = 0x6e330e;
        }
        p.setColor(SkColorSetRGB((rgb >> 16) & 0xff, (rgb >> 8) & 0xff, rgb & 0xff));
        canvas.drawRect(SkRect::MakeLTRB(x - half, std::max(800.f, top), x + half, 1000.f), p);
    }
    canvas.restore();
}

}  // namespace

void draw_terrain(SkCanvas& canvas, const Inputs& in, const Frame& f) {
    draw_nd_terrain(canvas, in, f);
    draw_vd_terrain(canvas, in, f);
}

}  // namespace a380::nd
