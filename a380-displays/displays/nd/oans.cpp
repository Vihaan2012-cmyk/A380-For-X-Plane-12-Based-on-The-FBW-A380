#include <algorithm>
#include "a380/env.h"
#include <functional>
#include <chrono>
#include <cmath>
#include <cstdlib>
#include <cstring>
#include <cstdio>
#include <fstream>
#include <map>
#include <memory>
#include <mutex>
#include <sstream>
#include <string>
#include <unordered_map>
#include <vector>

#include "include/codec/SkCodec.h"
#include "include/core/SkBitmap.h"
#include "include/core/SkData.h"
#include "include/core/SkSamplingOptions.h"
#include "include/core/SkImage.h"
#include "include/core/SkCanvas.h"
#include "include/core/SkMatrix.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPath.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkRect.h"
#include "include/effects/SkDashPathEffect.h"

#include "a380/assets.h"
#include "a380/display.h"
#include "a380/draw.h"
#include "a380/fonts.h"
#include "a380/inputs.h"
#include "nd/nd_layers.h"

namespace a380::nd {
namespace {

using json = nlohmann::json;

constexpr double kSphereRadiusM = 6371008.8;
constexpr double kPi = 3.14159265358979323846;
constexpr double kNmPerKm = 0.539957;
constexpr double kZoomRangeNm[5] = {0.2, 0.5, 1.0, 2.0, 5.0};

constexpr SkColor kYellow = SkColorSetRGB(255, 255, 0);
constexpr SkColor kCyan = SkColorSetRGB(0, 255, 255);
constexpr SkColor kMagenta = SkColorSetRGB(255, 148, 255);
constexpr SkColor kWhite = SkColorSetRGB(255, 255, 255);

struct Feature {
    std::string layer;
    std::vector<SkPath> paths;
    std::vector<SkPoint> pts;
    std::vector<SkPoint> cpts;
    std::string idlin, idstd, idrwy, idthr, termref, ident;
    int plysttyp = 0;
    bool centroid_ok = true;
};

struct Label {
    std::string text;
    int style = 0;  // 0 taxiway, 1 exit line, 2 terminal/stand, 3 runway axis, 4 runway end
    SkPoint pos{};
    SkPoint start{};
    float rotation = 0;
    bool centreline = false;
    bool exit_line = false;
    std::string termref;
};

struct Airport {
    std::string icao, iata, name;
    double arp_lat = 0, arp_lon = 0;
    std::vector<Feature> features;
    std::vector<Label> labels;
    SkRect bounds = SkRect::MakeEmpty();
    bool ok = false;
};

SkPoint azeq(double lat0, double lon0, double lat, double lon) {
    double p0 = lat0 * kPi / 180, l0 = lon0 * kPi / 180, p = lat * kPi / 180, l = lon * kPi / 180;
    double dl = l - l0;
    double cc = std::sin(p0) * std::sin(p) + std::cos(p0) * std::cos(p) * std::cos(dl);
    cc = std::max(-1.0, std::min(1.0, cc));
    double c = std::acos(cc);
    double k = c < 1e-12 ? 1.0 : c / std::sin(c);
    double e = kSphereRadiusM * k * std::cos(p) * std::sin(dl);
    double n = kSphereRadiusM * k * (std::cos(p0) * std::sin(p) - std::sin(p0) * std::cos(p) * std::cos(dl));
    return {(float)e, (float)n};
}

void from_azeq(double lat0, double lon0, double east, double north, double& lat, double& lon) {
    double p0 = lat0 * kPi / 180, l0 = lon0 * kPi / 180;
    double rho = std::hypot(east, north);
    if (rho == 0) { lat = lat0; lon = lon0; return; }
    double c = rho / kSphereRadiusM;
    lat = std::asin(std::cos(c) * std::sin(p0) + north * std::sin(c) * std::cos(p0) / rho) * 180 / kPi;
    lon = (l0 + std::atan2(east * std::sin(c), rho * std::cos(p0) * std::cos(c) - north * std::sin(p0) * std::sin(c))) * 180 / kPi;
}

struct Projector {
    bool metres = false;
    bool retarget = false;
    double lat0 = 0, lon0 = 0;
    double tlat = 0, tlon = 0;
    SkPoint operator()(const json& c) const {
        double a = c[0].get<double>(), b = c[1].get<double>();
        if (metres) {
            if (!retarget) return {(float)a, (float)b};
            double la, lo;
            from_azeq(lat0, lon0, a, b, la, lo);
            return azeq(tlat, tlon, la, lo);
        }
        return azeq(tlat, tlon, b, a);
    }
};

void add_ring(std::vector<SkPath>& out, const json& ring, const Projector& pr, std::vector<SkPoint>& pts, bool close) {
    SkPathBuilder path;
    bool first = true;
    for (auto& c : ring) {
        if (!c.is_array() || c.size() < 2) continue;
        SkPoint p = pr(c);
        pts.push_back(p);
        if (first) path.moveTo(p.fX, p.fY);
        else path.lineTo(p.fX, p.fY);
        first = false;
    }
    if (close) path.close();
    out.push_back(path.detach());
}

std::string jstr(const json& props, const char* key) {
    auto it = props.find(key);
    if (it == props.end() || it->is_null()) return {};
    if (it->is_string()) return it->get<std::string>();
    if (it->is_number()) return std::to_string(it->get<long long>());
    return {};
}

std::string read_file(const std::string& path) {
    std::ifstream f(path, std::ios::binary);
    if (!f) return {};
    std::stringstream ss;
    ss << f.rdbuf();
    return ss.str();
}

std::string upper(std::string s) {
    for (auto& c : s) c = (char)std::toupper((unsigned char)c);
    return s;
}

std::string pad_designator(const std::string& d) {
    int digits = 0;
    for (char c : d)
        if (c >= '0' && c <= '9') digits++;
    return digits < 2 ? "0" + d : d;
}

void generate_labels(Airport& a) {
    std::vector<const Feature*> order;
    for (auto& f : a.features) order.push_back(&f);
    std::stable_sort(order.begin(), order.end(), [](const Feature* x, const Feature* y) { return x->layer < y->layer; });
    for (const Feature* fp : order) {
        const Feature& f = *fp;
        const std::string& L = f.layer;
        if (L == "paintedcenterline") {
            if (f.idrwy.empty() || f.pts.size() < 2) continue;
            std::vector<std::string> des;
            std::stringstream ss(f.idrwy);
            std::string part;
            while (std::getline(ss, part, '.')) des.push_back(pad_designator(part));
            if (des.empty()) continue;
            SkPoint s = f.pts.front(), e = f.pts.back();
            double brg = std::fmod(-std::atan2(s.fY - e.fY, s.fX - e.fX) * 180 / kPi + 90 + 720, 360.0);
            double recip = std::fmod(brg + 180, 360.0);
            if (des.size() == 2) {
                int n0 = std::atoi(des[0].substr(0, 2).c_str());
                if (std::fabs(n0 * 10.0 - recip) > 90) std::swap(des[0], des[1]);
            }
            Label l1{des[0], 4, s, s, (float)recip};
            l1.centreline = true;
            a.labels.push_back(l1);
            if (des.size() > 1) {
                Label l2{des[1], 4, e, s, (float)brg};
                l2.centreline = true;
                a.labels.push_back(l2);
                Label l3{des[0] + "-" + des[1], 3, e, s, (float)brg};
                l3.centreline = true;
                a.labels.push_back(l3);
            } else {
                Label l3{des[0], 3, e, s, (float)brg};
                l3.centreline = true;
                a.labels.push_back(l3);
            }
            continue;
        }
        bool want = L == "taxiwayelement" || L == "parkingstandlocation" || L == "runwayexitline" ||
                    (L == "verticalpolygonalstructure" && (f.plysttyp == 0 || f.plysttyp == 1));
        if (!want) continue;
        if (L == "taxiwayelement" && f.idlin.empty()) continue;
        std::string text = !f.idlin.empty() ? f.idlin : !f.idstd.empty() ? f.idstd : f.ident;
        if (text.empty()) continue;
        if (L == "parkingstandlocation" && text.find('_') != std::string::npos) continue;
        SkPoint pos{0, 0};
        if (f.centroid_ok && !f.cpts.empty()) {
            double sx = 0, sy = 0;
            for (auto& p : f.cpts) { sx += p.fX; sy += p.fY; }
            pos = {(float)(sx / f.cpts.size()), (float)(sy / f.cpts.size())};
        }
        if (L == "parkingstandlocation") {
            bool dup = false;
            for (auto& o : a.labels)
                if (o.text == text && o.termref == f.termref) { dup = true; break; }
            if (dup) continue;
        }
        text = upper(text);
        Label l;
        l.text = text;
        l.pos = pos;
        l.style = L == "runwayexitline" ? 1 : (L == "taxiwayelement" ? 0 : 2);
        l.exit_line = L == "runwayexitline";
        l.termref = f.termref;
        a.labels.push_back(l);
    }
}

std::shared_ptr<Airport> parse_airport_unsafe(const std::string& text, const std::string& icao, std::shared_ptr<Airport> base) {
    auto ap = base ? base : std::make_shared<Airport>();
    ap->icao = icao;
    json doc = json::parse(text, nullptr, false);
    if (!doc.is_object()) return ap;
    Projector pr;
    double base_lat = ap->arp_lat, base_lon = ap->arp_lon;
    const json* layers = &doc;
    if (doc.contains("arp") && doc.contains("layers")) {
        pr.metres = true;
        ap->arp_lat = doc["arp"][0].get<double>();
        ap->arp_lon = doc["arp"][1].get<double>();
        layers = &doc["layers"];
    } else {
        auto it = doc.find("aerodromereferencepoint");
        if (it == doc.end() || !(*it).contains("features") || (*it)["features"].empty()) return ap;
        auto& c = (*it)["features"][0]["geometry"]["coordinates"];
        ap->arp_lon = c[0].get<double>();
        ap->arp_lat = c[1].get<double>();
    }
    if (base) {
        pr.lat0 = ap->arp_lat;
        pr.lon0 = ap->arp_lon;
        ap->arp_lat = base_lat;
        ap->arp_lon = base_lon;
        pr.retarget = true;
    }
    pr.tlat = ap->arp_lat;
    pr.tlon = ap->arp_lon;
    if (!base) {
        pr.lat0 = ap->arp_lat;
        pr.lon0 = ap->arp_lon;
    }
    if (!base) if (auto it = layers->find("aerodromereferencepoint"); it != layers->end() && (*it)["features"].size()) {
        auto& props = (*it)["features"][0]["properties"];
        ap->name = jstr(props, "name");
        ap->iata = jstr(props, "iata");
    }
    static const char* wanted[] = {"taxiwayelement", "taxiwayshoulder", "serviceroad", "runwayelement",
                                   "runwayintersection", "blastpad", "runwaydisplacedarea", "stopway",
                                   "runwaymarking", "runwayshoulder", "taxiwayguidanceline", "runwayexitline",
                                   "taxiwayholdingposition", "apronelement", "parkingstandarea",
                                   "verticalpolygonalstructure", "standguidanceline", "paintedcenterline",
                                   "parkingstandlocation"};
    for (const char* name : wanted) {
        if (base && std::strcmp(name, "verticalpolygonalstructure") != 0 && std::strcmp(name, "serviceroad") != 0) continue;
        auto it = layers->find(name);
        if (it == layers->end() || !it->contains("features")) continue;
        for (auto& jf : (*it)["features"]) {
            if (!jf.contains("geometry") || jf["geometry"].is_null()) continue;
            Feature f;
            f.layer = name;
            auto& props = jf["properties"];
            f.idlin = jstr(props, "idlin");
            f.idstd = jstr(props, "idstd");
            f.idrwy = jstr(props, "idrwy");
            f.idthr = jstr(props, "idthr");
            f.termref = jstr(props, "termref");
            f.ident = jstr(props, "ident");
            if (props.contains("plysttyp") && props["plysttyp"].is_number()) f.plysttyp = props["plysttyp"].get<int>();
            auto& g = jf["geometry"];
            std::string type = g.value("type", "");
            auto& co = g["coordinates"];
            if (type == "Point") {
                SkPoint p = pr(co);
                f.pts.push_back(p);
                f.cpts = f.pts;
            } else if (type == "LineString") {
                add_ring(f.paths, co, pr, f.pts, false);
                f.cpts = f.pts;
            } else if (type == "Polygon") {
                bool firstRing = true;
                for (auto& ring : co) {
                    std::vector<SkPoint> tmp;
                    add_ring(f.paths, ring, pr, firstRing ? f.pts : tmp, true);
                    const std::vector<SkPoint>& rp = firstRing ? f.pts : tmp;
                    if (!rp.empty()) f.cpts.insert(f.cpts.end(), rp.begin(), rp.end() - 1);
                    firstRing = false;
                }
            } else if (type == "MultiPolygon") {
                f.centroid_ok = false;
                for (auto& poly : co)
                    for (auto& ring : poly) {
                        std::vector<SkPoint> tmp;
                        add_ring(f.paths, ring, pr, f.pts.empty() ? f.pts : tmp, true);
                    }
            } else if (type == "MultiLineString") {
                f.centroid_ok = false;
                for (auto& ring : co) add_ring(f.paths, ring, pr, f.pts, false);
            } else {
                continue;
            }
            ap->features.push_back(std::move(f));
        }
    }
    {
        bool first = true;
        for (auto& ft : ap->features)
            for (auto& pt : ft.pts) {
                if (first) { ap->bounds = SkRect::MakeLTRB(pt.fX, pt.fY, pt.fX, pt.fY); first = false; }
                else ap->bounds.joinPossiblyEmptyRect(SkRect::MakeLTRB(pt.fX, pt.fY, pt.fX, pt.fY));
            }
    }
    if (!base) generate_labels(*ap);
    ap->ok = true;
    return ap;
}

std::shared_ptr<Airport> parse_airport(const std::string& text, const std::string& icao, std::shared_ptr<Airport> base = nullptr) {
    try {
        return parse_airport_unsafe(text, icao, base);
    } catch (...) {
        auto ap = std::make_shared<Airport>();
        ap->icao = icao;
        return ap;
    }
}

std::string amdb_dir() { return a380::assets::amdb_dir(); }

std::shared_ptr<Airport> load_airport(const Inputs& in, const std::string& icao) {
    static std::mutex mu;
    static std::unordered_map<std::string, std::shared_ptr<Airport>> cache;
    std::lock_guard<std::mutex> lock(mu);
    static std::unordered_map<std::string, std::chrono::steady_clock::time_point> failed;
    if (auto it = cache.find(icao); it != cache.end() && it->second->ok) return it->second;
    if (auto fit = failed.find(icao); fit != failed.end() && std::chrono::steady_clock::now() - fit->second < std::chrono::seconds(5))
        return cache[icao];
    std::string text;
    bool from_fetched = false;
    if (const std::string* g = in.fetched("/v1/" + icao + "?projection=NAVIGRAPH:ARP_AZEQ&format=geojson&exclude=&include=taxiwayelement")) {
        const std::string* a = in.fetched("/v1/" + icao + "?projection=EPSG:4326&format=geojson&exclude=&include=aerodromereferencepoint");
        json geo = json::parse(*g, nullptr, false);
        json arp = a ? json::parse(*a, nullptr, false) : json();
        if (geo.is_object() && arp.is_object() && arp.contains("aerodromereferencepoint") &&
            !arp["aerodromereferencepoint"]["features"].empty()) {
            auto& c = arp["aerodromereferencepoint"]["features"][0]["geometry"]["coordinates"];
            json doc = {{"arp", json::array({c[1], c[0]})}, {"layers", std::move(geo)}};
            text = doc.dump();
            from_fetched = true;
        }
    } else if (const std::string* f = in.fetched(icao)) {
        text = *f;
    }
    std::shared_ptr<Airport> ap;
    if (!text.empty()) ap = parse_airport(text, icao);
    if (!ap || !ap->ok) {
        std::vector<std::string> paths = {amdb_dir() + "/" + icao + ".json"};
        if (const char* e = a380::getenv_cached("A380_AMDB_FALLBACK")) paths.push_back(std::string(e) + "/" + icao + ".json");
        paths.push_back("E:/A380/fbw-build/a380-displays/nd-oans/data/" + icao + "_gateway.json");
        for (auto& p : paths) {
            text = read_file(p);
            if (text.empty()) continue;
            ap = parse_airport(text, icao);
            if (ap->ok) break;
        }
    }
    if (!ap) ap = std::make_shared<Airport>();
    if (ap->ok && !from_fetched) {
        std::vector<std::string> opaths = {amdb_dir() + "/" + icao + "_osm.json",
                                           "E:/A380/fbw-build/a380-displays/nd-oans/data/" + icao + "_osm.json"};
        for (auto& p : opaths) {
            std::string t = read_file(p);
            if (t.empty()) continue;
            size_t n = ap->features.size();
            parse_airport(t, icao, ap);
            std::vector<Label> extra;
            Airport tmp;
            tmp.features.assign(ap->features.begin() + n, ap->features.end());
            generate_labels(tmp);
            ap->labels.insert(ap->labels.end(), tmp.labels.begin(), tmp.labels.end());
            break;
        }
    }
    cache[icao] = ap;
    if (!ap->ok) failed[icao] = std::chrono::steady_clock::now();
    return ap;
}

std::string airport_icao(const Inputs& in) {
    if (const json* b = in.bus("oans_display_airport")) {
        if (b->is_string()) return upper(b->get<std::string>());
    }
    return {};
}

constexpr float kCanvasScaleShift = 0.6f;
constexpr float kLayoutShiftScale1 = 1.f;
constexpr float kLayoutShiftX25 = 1.5f;
constexpr float kLayoutShiftY25 = 2.5f;
constexpr float kWorldShift25 = 1.f;


thread_local SkRect g_view = SkRect::MakeLTRB(-1e9f, -1e9f, 1e9f, 1e9f);

bool visible(const SkPath& p) { return SkRect::Intersects(p.getBounds().makeOutset(10.f, 10.f), g_view); }

struct Style {
    bool fill;
    SkColor color;
    float width;
};

void fill_features(SkCanvas& c, const Airport& a, const char* layer, SkColor color, int plysttyp = -1) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setStyle(SkPaint::kFill_Style);
    p.setColor(color);
    for (auto& f : a.features) {
        if (f.layer != layer) continue;
        if (plysttyp >= 0 && f.plysttyp != plysttyp) continue;
        for (auto& path : f.paths)
            if (visible(path)) c.drawPath(path, p);
    }
}

void stroke_features(SkCanvas& c, const Airport& a, std::initializer_list<const char*> layers, SkColor color, float width) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setStyle(SkPaint::kStroke_Style);
    p.setColor(color);
    p.setStrokeWidth(width);
    for (const char* layer : layers)
        for (auto& f : a.features) {
            if (f.layer != layer) continue;
            for (auto& path : f.paths)
                if (visible(path)) c.drawPath(path, p);
        }
}

bool label_visible(const Label& l, bool arc, int zoom, const std::string& btv_rwy) {
    if (!btv_rwy.empty() && l.centreline && l.text == btv_rwy) return true;
    if (arc) {
        switch (zoom) {
            case 3:
            case 4: return false;
            case 2: return l.centreline || (l.text.size() < 2 && !l.exit_line);
            default: return !l.exit_line;
        }
    }
    (void)zoom;
    return l.centreline || l.exit_line || zoom == 0;
}

void draw_html_label(SkCanvas& c, const Label& l, float x, float y, SkColor color) {
    const float dpr = c.getLocalToDeviceAs3x3().getScaleX();
    SkFont font = a380::font("Ecam", 19.f);
    font.setEdging(SkFont::Edging::kAntiAlias);
    float tw = font.measureText(l.text.data(), l.text.size(), SkTextEncoding::kUTF8) * dpr;
    float w = std::ceil(tw * 64.f) / 64.f + dpr;
    float left = a380::layout_unit(x * dpr) - w / 2, top = a380::layout_unit(y * dpr) - 8.5f * dpr;
    SkPaint bg;
    bg.setColor(SK_ColorBLACK);
    c.save();
    c.scale(1.f / dpr, 1.f / dpr);
    c.drawRect(SkRect::MakeXYWH(left, top, w, 17.f * dpr), bg);
    SkPaint tp;
    tp.setAntiAlias(true);
    tp.setColor(color);
    a380::HheaMetrics hm = a380::hhea_metrics(font);
    float em = font.getSize() * dpr;
    float asc = std::round(hm.ascent * em), gap = std::round(hm.line_gap * em);
    float by = std::floor(top + asc + std::floor(gap * 0.5f) + 0.5f);
    c.scale(dpr, dpr);
    a380::draw_simple_text(c, l.text.data(), l.text.size(), SkTextEncoding::kUTF8, (left + dpr) / dpr, by / dpr, font, tp);
    c.restore();
}

void draw_label_box(SkCanvas& c, const Label& l, float x, float y, SkColor color) {
    SkFont font = a380::font("Ecam", 19.f);
    font.setEdging(SkFont::Edging::kAntiAlias);
    float w = font.measureText(l.text.data(), l.text.size(), SkTextEncoding::kUTF8) + 1.f;
    SkPaint bg;
    bg.setColor(SK_ColorBLACK);
    c.drawRect(SkRect::MakeXYWH(x - w / 2, y - 8.5f, w, 17.f), bg);
    SkPaint tp;
    tp.setAntiAlias(true);
    tp.setColor(color);
    a380::draw_simple_text(c, l.text.data(), l.text.size(), SkTextEncoding::kUTF8, x - w / 2 + 1.f, y + 7.2f, font, tp);
}


float wrap180(double a) {
    a = std::fmod(a, 360.0);
    if (a >= 180) a -= 360;
    if (a < -180) a += 360;
    return (float)a;
}

void ring_text(SkCanvas& c, const std::string& t, float baseline, float size, bool shadow_pass) {
    SkFont font = a380::font("Ecam", size);
    font.setEdging(SkFont::Edging::kAntiAlias);
    float w = font.measureText(t.data(), t.size(), SkTextEncoding::kUTF8);
    SkPaint p;
    p.setAntiAlias(true);
    if (shadow_pass) {
        p.setStyle(SkPaint::kStroke_Style);
        p.setStrokeWidth(1.5f);
        p.setColor(SK_ColorBLACK);
    } else {
        p.setColor(kWhite);
    }
    a380::draw_simple_text(c, t.data(), t.size(), SkTextEncoding::kUTF8, -w / 2, baseline, font, p);
}

void draw_ring(SkCanvas& c, bool arc, SkPoint centre, float rot_deg) {
    float R = arc ? 492.f : 250.f;
    float major = arc ? 29.f : 12.f, minor = arc ? 15.f : 6.f;
    float text_r = arc ? 529.f : 272.f;
    float size_big = arc ? 34.f : 26.f;
    for (int pass = 0; pass < 2; pass++) {
        bool shadow = pass == 0;
        SkPaint lp;
        lp.setAntiAlias(true);
        lp.setStyle(SkPaint::kStroke_Style);
        lp.setStrokeCap(SkPaint::kRound_Cap);
        lp.setStrokeWidth(shadow ? 3.5f : 2.f);
        lp.setColor(shadow ? SK_ColorBLACK : kWhite);
        c.save();
        if (arc) c.clipRect(SkRect::MakeLTRB(6, 0, 762, 768));
        c.translate(centre.fX, centre.fY);
        c.rotate(rot_deg);
        if (arc) {
            SkPathBuilder rb;
            rb.moveTo(-R, 0);
            rb.rArcTo({R, R}, 0, SkPathBuilder::kLarge_ArcSize, SkPathDirection::kCCW, {2 * R, 0});
            rb.rArcTo({R, R}, 0, SkPathBuilder::kLarge_ArcSize, SkPathDirection::kCCW, {-2 * R, 0});
            c.drawPath(rb.detach(), lp);
        } else
            stroke_circle(c, 0, 0, R, shadow ? 3.5f : 2.f, shadow ? SK_ColorBLACK : kWhite);
        for (int k = 0; k < 72; k++) {
            float base = arc ? -60.f + 5.f * k : 5.f * k;
            bool is_major = (k % 2) == 0;
            c.save();
            c.rotate(base);
            c.drawLine(0, -R, 0, -(R + (is_major ? major : minor)), lp);
            if (is_major) {
                int n = k / 2;
                bool label = arc ? true : (n % 3 == 0);
                if (label) {
                    bool big = (n % 3) == 0;
                    float by = arc ? ((n == 7 || n == 8) ? 529.f - 4.f : 529.f) : text_r;
                    c.save();
                    c.translate(0, -by);
                    ring_text(c, std::to_string(n), 0.f, big ? size_big : 22.f, shadow);
                    c.restore();
                }
            }
            c.restore();
        }
        c.restore();
    }
}

void draw_dashed_circle(SkCanvas& c, SkPoint centre, float r, bool arc) {
    float dash[2] = {10.f, 6.f};
    for (int pass = 0; pass < 2; pass++) {
        SkPaint p;
        p.setAntiAlias(true);
        p.setStyle(SkPaint::kStroke_Style);
        p.setStrokeCap(SkPaint::kRound_Cap);
        p.setStrokeWidth(pass == 0 ? 3.5f : 2.f);
        p.setColor(pass == 0 ? SK_ColorBLACK : kWhite);
        p.setPathEffect(SkDashPathEffect::Make(SkSpan<const SkScalar>(dash, 2), arc ? 0.f : -4.2f));
        c.save();
        if (arc) {
            SkPathBuilder clip;
            clip.moveTo(0, 532).lineTo(384, 687).lineTo(768, 541).lineTo(768, 29).lineTo(0, 29).close();
            c.clipPath(clip.detach());
        }
        SkPathBuilder rb;
        SkRect oval = SkRect::MakeLTRB(centre.fX - r, centre.fY - r, centre.fX + r, centre.fY + r);
        if (arc) {
            rb.arcTo(oval, 180, -180, true);
            rb.arcTo(oval, 0, -180, false);
        } else {
            rb.arcTo(oval, 0, 180, true);
            rb.arcTo(oval, 180, 180, true);
        }
        c.drawPath(rb.detach(), p);
        c.restore();
    }
}


bool seg_seg(SkPoint a, SkPoint b, SkPoint c, SkPoint d, SkPoint& out) {
    float den = (d.fY - c.fY) * (b.fX - a.fX) - (d.fX - c.fX) * (b.fY - a.fY);
    if (std::fabs(den) < 1e-9f) return false;
    float ua = ((d.fX - c.fX) * (a.fY - c.fY) - (d.fY - c.fY) * (a.fX - c.fX)) / den;
    float ub = ((b.fX - a.fX) * (a.fY - c.fY) - (b.fY - a.fY) * (a.fX - c.fX)) / den;
    if (ua < 0 || ua > 1 || ub < 0 || ub > 1) return false;
    out = {a.fX + ua * (b.fX - a.fX), a.fY + ua * (b.fY - a.fY)};
    return true;
}

std::vector<SkPoint> seg_rect(SkPoint a, SkPoint b, float w, float h) {
    std::vector<SkPoint> r;
    SkPoint o;
    SkPoint tl{0, 0}, tr{w, 0}, bl{0, h}, br{w, h};
    if (seg_seg(a, b, tl, bl, o)) r.push_back(o);
    if (seg_seg(a, b, tr, br, o)) r.push_back(o);
    if (seg_seg(a, b, tl, tr, o)) r.push_back(o);
    if (seg_seg(a, b, bl, br, o)) r.push_back(o);
    return r;
}

bool in_rect(SkPoint p, float w, float h) { return p.fX >= 0 && p.fX <= w && p.fY >= 0 && p.fY <= h; }

const unsigned char kBracketPng[] = {
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x6f, 0x00, 0x00, 0x00, 0x3f,
    0x08, 0x06, 0x00, 0x00, 0x00, 0xee, 0x4b, 0x96, 0x97, 0x00, 0x00, 0x00, 0x09, 0x70, 0x48, 0x59, 0x73, 0x00, 0x00, 0x0b, 0x13, 0x00, 0x00, 0x0b,
    0x13, 0x01, 0x00, 0x9a, 0x9c, 0x18, 0x00, 0x00, 0x00, 0x01, 0x73, 0x52, 0x47, 0x42, 0x00, 0xae, 0xce, 0x1c, 0xe9, 0x00, 0x00, 0x00, 0x04, 0x67,
    0x41, 0x4d, 0x41, 0x00, 0x00, 0xb1, 0x8f, 0x0b, 0xfc, 0x61, 0x05, 0x00, 0x00, 0x01, 0xf3, 0x49, 0x44, 0x41, 0x54, 0x78, 0x01, 0xed, 0x9a, 0xd1,
    0x6d, 0x83, 0x40, 0x10, 0x44, 0x4f, 0xa9, 0xc0, 0x25, 0xb8, 0x84, 0x74, 0x40, 0x3a, 0x48, 0x3a, 0xb0, 0x3b, 0x20, 0x1d, 0x38, 0x1d, 0xb8, 0x04,
    0x4a, 0x48, 0x09, 0x94, 0xe0, 0x12, 0x28, 0x81, 0x12, 0x26, 0x87, 0xd8, 0xc8, 0x0e, 0x09, 0xc7, 0x81, 0x30, 0xbe, 0xb9, 0xcc, 0x93, 0xf6, 0x03,
    0xc9, 0xc2, 0x16, 0x4f, 0xb3, 0xbe, 0x5d, 0xdb, 0xb9, 0x0c, 0x01, 0xf0, 0xe2, 0xeb, 0xec, 0xeb, 0xe2, 0xeb, 0xe0, 0x6b, 0xe7, 0x44, 0xba, 0x74,
    0x82, 0x7c, 0x95, 0xbe, 0x6a, 0xfc, 0xa6, 0xf5, 0x55, 0x75, 0x52, 0x9d, 0x48, 0x07, 0x4b, 0x59, 0x65, 0x82, 0x62, 0x68, 0x2c, 0x8d, 0x7b, 0x27,
    0xb6, 0xc7, 0x52, 0xf6, 0x61, 0x6d, 0x71, 0x8c, 0xda, 0xa4, 0x36, 0x81, 0xd7, 0x54, 0x4a, 0xe3, 0x46, 0x58, 0xca, 0xea, 0x80, 0x8c, 0x2e, 0x7d,
    0x27, 0x0c, 0xbe, 0xe3, 0xfc, 0xf5, 0xd1, 0xd7, 0x27, 0xc2, 0x69, 0x2c, 0x95, 0xc6, 0x95, 0xb9, 0x49, 0x59, 0x3b, 0x91, 0xb2, 0x22, 0xe2, 0x5e,
    0x7b, 0x93, 0xdb, 0x04, 0xee, 0xd5, 0x49, 0x7e, 0x73, 0x62, 0x19, 0x26, 0x6c, 0x2a, 0x65, 0x0d, 0xfe, 0x48, 0xd9, 0x8c, 0xf7, 0x28, 0xd0, 0xb7,
    0xcd, 0xd0, 0xfd, 0x2b, 0xa5, 0x31, 0x12, 0xff, 0xa0, 0x9e, 0x27, 0x52, 0xd6, 0x5a, 0x32, 0x0a, 0xb7, 0x12, 0x96, 0xc6, 0x23, 0xc2, 0x69, 0xac,
    0x7d, 0x1d, 0x9c, 0xf8, 0x09, 0xc2, 0x47, 0xfc, 0xdb, 0x87, 0x57, 0xe2, 0xce, 0xf3, 0x9a, 0xd2, 0x18, 0x09, 0xae, 0x83, 0x74, 0x28, 0x65, 0xe7,
    0x35, 0x53, 0x36, 0xe3, 0xb3, 0xed, 0x2c, 0x8d, 0xa1, 0xd3, 0xec, 0xff, 0x5a, 0x00, 0xa4, 0x94, 0xb2, 0x58, 0xd0, 0xb7, 0xf2, 0x0a, 0xe3, 0x6d,
    0x35, 0xef, 0x05, 0x00, 0xa6, 0x07, 0xe9, 0xef, 0x23, 0xfe, 0xe6, 0x29, 0x9b, 0x83, 0xa5, 0xb1, 0xc6, 0x38, 0x0d, 0x72, 0x58, 0x00, 0x20, 0x7e,
    0x90, 0x7e, 0x4d, 0x25, 0x65, 0xb1, 0xa0, 0x3f, 0xe4, 0x54, 0xc8, 0x6d, 0x01, 0x80, 0x85, 0x83, 0x34, 0x2b, 0x60, 0x5f, 0x00, 0x60, 0xc5, 0x41,
    0x9a, 0x15, 0x30, 0x2d, 0x00, 0xb0, 0xc1, 0x20, 0xcd, 0x0a, 0x52, 0x1d, 0x39, 0xf0, 0x80, 0x41, 0x9a, 0x15, 0xa4, 0xb0, 0x00, 0x00, 0xe1, 0x11,
    0x3f, 0x35, 0x36, 0x4f, 0x23, 0x12, 0x1e, 0xa4, 0x59, 0xc1, 0x3d, 0x17, 0x00, 0x4a, 0xd9, 0x76, 0x60, 0xad, 0x05, 0x00, 0x32, 0x19, 0xa4, 0x59,
    0xc1, 0x92, 0x05, 0x80, 0xbf, 0x78, 0x47, 0x86, 0x83, 0x34, 0x2b, 0x88, 0x5f, 0x00, 0xec, 0x1c, 0xc6, 0xff, 0xb0, 0x73, 0x92, 0xb0, 0xc7, 0x82,
    0xf0, 0x02, 0xa0, 0x18, 0xca, 0xbb, 0x40, 0x6d, 0x31, 0x39, 0x70, 0x5d, 0x00, 0xb4, 0x21, 0x79, 0xfa, 0xc1, 0x31, 0x61, 0x06, 0xad, 0xb4, 0x78,
    0x72, 0x82, 0x16, 0xc9, 0x23, 0x46, 0xf2, 0x88, 0x91, 0x3c, 0x62, 0x24, 0x8f, 0x18, 0xc9, 0x23, 0x46, 0xf2, 0x88, 0x91, 0x3c, 0x62, 0x24, 0x8f,
    0x18, 0xc9, 0x23, 0x46, 0xf2, 0x88, 0x91, 0x3c, 0x62, 0x24, 0x8f, 0x18, 0xc9, 0x23, 0x46, 0xf2, 0x88, 0x91, 0x3c, 0x62, 0x24, 0x8f, 0x18, 0xc9,
    0x23, 0x46, 0xf2, 0x88, 0x91, 0x3c, 0x62, 0x24, 0x8f, 0x18, 0xc9, 0x23, 0x46, 0xf2, 0x88, 0x91, 0x3c, 0x62, 0x24, 0x8f, 0x18, 0xc9, 0x23, 0x46,
    0xf2, 0x88, 0x91, 0x3c, 0x62, 0x24, 0x8f, 0x18, 0xc9, 0x23, 0x46, 0xf2, 0x88, 0x91, 0x3c, 0x62, 0x24, 0x8f, 0x18, 0xc9, 0x23, 0x46, 0xf2, 0x88,
    0x91, 0x3c, 0x62, 0x24, 0x8f, 0x18, 0xc9, 0x23, 0xe6, 0x0b, 0x5a, 0x44, 0x29, 0x4f, 0x4e, 0x18, 0x48, 0xa0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
};

const unsigned char kBracketSelPng[] = {
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x6f, 0x00, 0x00, 0x00, 0x3f,
    0x08, 0x06, 0x00, 0x00, 0x00, 0xee, 0x4b, 0x96, 0x97, 0x00, 0x00, 0x00, 0x01, 0x73, 0x52, 0x47, 0x42, 0x00, 0xae, 0xce, 0x1c, 0xe9, 0x00, 0x00,
    0x02, 0xef, 0x49, 0x44, 0x41, 0x54, 0x78, 0x5e, 0xed, 0x9a, 0xe1, 0x75, 0xdb, 0x30, 0x0c, 0x84, 0x0f, 0x1b, 0x74, 0x84, 0x8e, 0xd0, 0x0d, 0xea,
    0x0d, 0x9a, 0x0d, 0xe2, 0x0d, 0x92, 0x4e, 0x90, 0x74, 0x83, 0x74, 0x83, 0x64, 0x83, 0x74, 0x82, 0x26, 0x1b, 0x74, 0x83, 0x76, 0x84, 0x6e, 0x80,
    0x18, 0x31, 0xd5, 0x67, 0xd7, 0x92, 0x42, 0xf9, 0x51, 0x14, 0x4f, 0x39, 0xfe, 0xb1, 0x7f, 0xf0, 0xc9, 0xd0, 0x7d, 0x3a, 0x80, 0x80, 0x6c, 0x58,
    0xe1, 0x72, 0xf7, 0x0d, 0x80, 0x2f, 0x00, 0xe2, 0xf3, 0x0e, 0xc0, 0x0f, 0x33, 0xfb, 0xbb, 0xb6, 0x5b, 0xb5, 0xb5, 0xdc, 0x90, 0xbb, 0x7f, 0x00,
    0x70, 0x09, 0xe0, 0xc2, 0xf6, 0xd0, 0x8e, 0x96, 0x03, 0xf7, 0x00, 0x1e, 0xcc, 0xec, 0x69, 0x2d, 0xf7, 0x4c, 0x0f, 0x2f, 0xb9, 0xec, 0xd2, 0x80,
    0x6d, 0x0e, 0x14, 0x07, 0xfe, 0xec, 0xf6, 0xdd, 0x02, 0x78, 0x36, 0xb3, 0xf8, 0x4e, 0xbb, 0x28, 0xe1, 0x25, 0x97, 0x5d, 0x47, 0x6a, 0x34, 0xe0,
    0x53, 0x9f, 0xfa, 0x0e, 0x84, 0xc3, 0x02, 0xce, 0xc6, 0x80, 0x8f, 0x03, 0x7b, 0xa8, 0xdd, 0x48, 0x05, 0x2f, 0xb9, 0xec, 0xa6, 0x2f, 0x2d, 0x76,
    0x70, 0x7c, 0xef, 0xaa, 0xef, 0x87, 0x35, 0xce, 0xdd, 0xc3, 0x95, 0x01, 0xfa, 0x62, 0x00, 0x62, 0x40, 0xee, 0x6a, 0x23, 0x8d, 0x1b, 0x9b, 0x87,
    0xd7, 0xb9, 0xcc, 0x80, 0x9b, 0xa1, 0xfc, 0x96, 0x5c, 0x76, 0x6b, 0x66, 0xcf, 0x63, 0x39, 0xd0, 0xdd, 0xc3, 0x81, 0x51, 0x17, 0xb7, 0x23, 0x6e,
    0x7c, 0x4c, 0xb5, 0x31, 0x3e, 0x9b, 0x5e, 0x4d, 0xc2, 0x4b, 0xc0, 0x22, 0x1d, 0x0e, 0xba, 0x2c, 0xd5, 0xae, 0x48, 0x7b, 0x47, 0x2e, 0xcb, 0x55,
    0xdb, 0xdd, 0x3f, 0x27, 0x88, 0xbd, 0xb5, 0x32, 0x5d, 0x3f, 0x52, 0xef, 0xb7, 0x56, 0x6b, 0x63, 0x53, 0xf0, 0xdc, 0x3d, 0x80, 0xc5, 0x69, 0x71,
    0xcc, 0x65, 0xe1, 0x88, 0xbb, 0xb7, 0x5c, 0x36, 0x01, 0x62, 0xb8, 0x31, 0x4e, 0xa7, 0xf1, 0xa0, 0x0c, 0xd5, 0xc6, 0x80, 0x78, 0x6f, 0x66, 0x0f,
    0xb9, 0xd7, 0xad, 0xb1, 0x6f, 0x71, 0x78, 0x6f, 0x1d, 0xf1, 0x43, 0x84, 0x94, 0x16, 0xbb, 0x74, 0x36, 0x5b, 0xbf, 0xc6, 0xe6, 0xc6, 0xc5, 0xe0,
    0x75, 0x8d, 0xb4, 0x01, 0x71, 0x6a, 0xec, 0x5d, 0xbe, 0x3f, 0x44, 0x3c, 0x96, 0x72, 0x59, 0xae, 0x1b, 0xd2, 0x03, 0x15, 0x87, 0x9b, 0xab, 0x91,
    0xd3, 0xec, 0xaf, 0xa5, 0x07, 0x00, 0x55, 0xe1, 0xb5, 0xe4, 0xb2, 0x09, 0x20, 0x23, 0x95, 0x5f, 0xb5, 0xd8, 0x72, 0x54, 0x81, 0x97, 0xd3, 0x48,
    0xa7, 0x23, 0xfe, 0x53, 0x6d, 0x97, 0xe5, 0x42, 0x7c, 0x4d, 0xdf, 0xfb, 0x96, 0x23, 0x06, 0x02, 0x27, 0x13, 0x9c, 0x94, 0xde, 0xab, 0x0e, 0x00,
    0x66, 0x83, 0x37, 0xa1, 0x91, 0x8e, 0xd4, 0x18, 0xd3, 0x8e, 0xd9, 0x6a, 0xd9, 0x14, 0x40, 0x39, 0x7b, 0x53, 0xcb, 0x11, 0x87, 0xaa, 0x45, 0x07,
    0x00, 0xc5, 0xe1, 0x9d, 0xdb, 0x48, 0xe7, 0x88, 0xd6, 0xe2, 0x9e, 0x25, 0x07, 0x00, 0x45, 0xe0, 0x95, 0x6c, 0xa4, 0x5b, 0x04, 0x94, 0x13, 0xd3,
    0x12, 0x03, 0x80, 0xb3, 0xe1, 0xd5, 0x68, 0xa4, 0x73, 0x44, 0x6b, 0x71, 0x4f, 0xad, 0x96, 0x63, 0x32, 0xbc, 0x25, 0x1a, 0xe9, 0x16, 0x01, 0xe5,
    0xc4, 0x94, 0xdc, 0x38, 0xdb, 0x00, 0x20, 0x0b, 0x1e, 0xe3, 0x11, 0x3f, 0x47, 0xdc, 0x9a, 0x7b, 0xe6, 0x70, 0xe3, 0x28, 0xbc, 0x96, 0x1b, 0xe9,
    0x9a, 0xc2, 0x97, 0xfc, 0xad, 0x92, 0x03, 0x80, 0x13, 0x78, 0x72, 0x59, 0x49, 0x54, 0xe3, 0xd7, 0x4a, 0x25, 0xe8, 0xec, 0x01, 0xc0, 0x3f, 0x78,
    0x6b, 0x69, 0xa4, 0xeb, 0x49, 0x5f, 0xf6, 0x97, 0xce, 0x19, 0x00, 0x98, 0xbb, 0xc7, 0x6c, 0x31, 0xa6, 0x06, 0x63, 0x6f, 0xa4, 0xe9, 0x1a, 0xe9,
    0xb2, 0xd2, 0xd6, 0xbb, 0xda, 0x84, 0x01, 0xc0, 0xd7, 0x80, 0xf7, 0x73, 0xe0, 0x0f, 0x3b, 0x27, 0x6f, 0xa4, 0xeb, 0xdd, 0x82, 0x7e, 0xe9, 0x60,
    0x1c, 0xd7, 0xfb, 0x0f, 0x80, 0xdd, 0x9b, 0x96, 0xcd, 0x11, 0x3c, 0x07, 0x62, 0x52, 0x7e, 0xdd, 0xf2, 0x7c, 0xf1, 0x3d, 0x62, 0xed, 0x06, 0x00,
    0xbb, 0x1a, 0x17, 0x86, 0x7a, 0x5d, 0x7d, 0xf0, 0xb6, 0xad, 0xbd, 0x70, 0x7c, 0x8f, 0xb0, 0x86, 0xee, 0xd9, 0xdd, 0x7f, 0x77, 0x2f, 0x8c, 0x05,
    0x8f, 0xec, 0xc9, 0x10, 0x3c, 0x32, 0x60, 0x87, 0xe1, 0x0a, 0x9e, 0xe0, 0x11, 0x2b, 0x40, 0x1c, 0xba, 0x9c, 0x27, 0x78, 0xc4, 0x0a, 0x10, 0x87,
    0x2e, 0xe7, 0x09, 0x1e, 0xb1, 0x02, 0xc4, 0xa1, 0xcb, 0x79, 0x82, 0x47, 0xac, 0x00, 0x71, 0xe8, 0x72, 0x9e, 0xe0, 0x11, 0x2b, 0x40, 0x1c, 0xba,
    0x9c, 0x27, 0x78, 0xc4, 0x0a, 0x10, 0x87, 0x2e, 0xe7, 0x09, 0x1e, 0xb1, 0x02, 0xc4, 0xa1, 0xcb, 0x79, 0x82, 0x47, 0xac, 0x00, 0x71, 0xe8, 0x72,
    0x9e, 0xe0, 0x11, 0x2b, 0x40, 0x1c, 0xba, 0x9c, 0x27, 0x78, 0xc4, 0x0a, 0x10, 0x87, 0x2e, 0xe7, 0x09, 0x1e, 0xb1, 0x02, 0xc4, 0xa1, 0xcb, 0x79,
    0x82, 0x47, 0xac, 0x00, 0x71, 0xe8, 0x72, 0x9e, 0xe0, 0x11, 0x2b, 0x40, 0x1c, 0xba, 0x9c, 0x27, 0x78, 0xc4, 0x0a, 0x10, 0x87, 0x2e, 0xe7, 0x09,
    0x1e, 0xb1, 0x02, 0xc4, 0xa1, 0xcb, 0x79, 0x82, 0x47, 0xac, 0x00, 0x71, 0xe8, 0x72, 0x9e, 0xe0, 0x11, 0x2b, 0x40, 0x1c, 0xba, 0x9c, 0x27, 0x78,
    0xc4, 0x0a, 0x10, 0x87, 0xfe, 0xbf, 0xf3, 0x5e, 0x00, 0x2e, 0x42, 0xfb, 0x07, 0x77, 0xce, 0x67, 0xd5, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
    0x44, 0xae, 0x42, 0x60, 0x82,
};

sk_sp<SkImage> bracket_image(bool selected = false) {
    thread_local sk_sp<SkImage> imgs[2];
    sk_sp<SkImage>& img = imgs[selected ? 1 : 0];
    if (!img) img = [selected] {
        auto data = selected ? SkData::MakeWithCopy(kBracketSelPng, sizeof kBracketSelPng) : SkData::MakeWithCopy(kBracketPng, sizeof kBracketPng);
        std::unique_ptr<SkCodec> codec = SkCodec::MakeFromData(data);
        return codec ? std::get<0>(codec->getImage()) : sk_sp<SkImage>();
    }();
    return img;
}

void draw_bracket_image(SkCanvas& c, bool selected = false) {
    sk_sp<SkImage> img = bracket_image(selected);
    if (!img) return;
    float dpr = std::sqrt(c.getTotalMatrix().getScaleX() * c.getTotalMatrix().getScaleX() + c.getTotalMatrix().getSkewY() * c.getTotalMatrix().getSkewY());
    float bw = std::round(60.f * dpr), bh = std::round(35.f * dpr);
    float s = std::max(bw / img->width(), bh / img->height());
    float w = img->width() * s, h = img->height() * s;
    c.save();
    c.scale(1.f / dpr, 1.f / dpr);
    c.translate(-30.f * dpr, -17.5f * dpr);
    c.clipRect(SkRect::MakeWH(bw, bh), true);
    SkPaint ip;
    ip.setAntiAlias(true);
    c.drawImageRect(img, SkRect::MakeXYWH(0, 0, w, h), SkSamplingOptions(SkFilterMode::kLinear), &ip);
    c.restore();
}

SkPoint snap_device(const SkCanvas& c, SkPoint p) {
    const SkMatrix m = c.getTotalMatrix();
    SkMatrix inv;
    if (!m.rectStaysRect() || !m.invert(&inv)) return p;
    const SkPoint d = m.mapPoint(p);
    return inv.mapPoint({std::floor(d.fX + 0.5f), std::floor(d.fY + 0.5f)});
}

void draw_runway_end(SkCanvas& c, const Label& l, SkPoint p, float heading, bool selected = false) {
    float r = l.rotation - heading;
    float rad = r * (float)kPi / 180.f;
    float cx = p.fX - 50.f * std::sin(rad), cy = p.fY + 50.f * std::cos(rad);
    c.save();
    c.translate(cx, cy);
    c.rotate(r);
    draw_bracket_image(c, selected);
    SkFont font = a380::font("Ecam", 25.f);
    font.setEdging(SkFont::Edging::kAntiAlias);
    float w = font.measureText(l.text.data(), l.text.size(), SkTextEncoding::kUTF8);
    SkPaint tp;
    tp.setAntiAlias(true);
    tp.setColor(selected ? kCyan : kWhite);
    float dpr = std::sqrt(c.getTotalMatrix().getScaleX() * c.getTotalMatrix().getScaleX() + c.getTotalMatrix().getSkewY() * c.getTotalMatrix().getSkewY());
    a380::HheaMetrics hh = a380::hhea_metrics(font);
    float em = 25.f * dpr;
    float base_dev = (selected ? 10.f : 13.f) * dpr + std::round(hh.ascent * em) + std::floor(std::round(hh.line_gap * em) / 2.f);
    a380::draw_simple_text(c, l.text.data(), l.text.size(), SkTextEncoding::kUTF8, 0.5f - w / 2, (base_dev - 17.5f * dpr) / dpr, font, tp);
    if (selected) {
        float ow = std::floor(3.f * dpr) / dpr;
        SkPaint op;
        op.setAntiAlias(true);
        op.setStyle(SkPaint::kStroke_Style);
        op.setStrokeWidth(ow);
        op.setColor(kCyan);
        c.drawRect(SkRect::MakeXYWH(-30.f - ow / 2, -17.5f - ow / 2, 60.f + ow, 35.f + ow), op);
    }
    c.restore();
}

bool runway_axis_pos(const Label& l, const SkMatrix& m, float off_y, SkPoint& out, float& angle) {
    auto proj = [&](SkPoint q) {
        return SkPoint{m.getScaleX() * q.fX + m.getSkewX() * q.fY + m.getTranslateX(),
                       m.getSkewY() * q.fX + m.getScaleY() * q.fY + m.getTranslateY()};
    };
    SkPoint s1 = proj(l.start), s2 = proj(l.pos);
    SkPoint lc{(s1.fX + s2.fX) / 2, (s1.fY + s2.fY) / 2};
    auto in = seg_rect(s1, s2, 768, 768);
    float x, y;
    if (in.size() > 1) {
        SkPoint mid{(in[0].fX + in[1].fX) / 2, (in[0].fY + in[1].fY) / 2};
        auto lsci = seg_rect(mid, lc, 768, 768);
        if (!lsci.empty()) { x = in[0].fX; y = in[0].fY; }
        else { x = lc.fX; y = lc.fY; }
    } else if (!in.empty()) {
        SkPoint pt = in_rect(s1, 768, 768) ? s1 : s2;
        float xl = in[0].fX - pt.fX, yl = in[0].fY - pt.fY;
        float hyp = std::hypot(xl, yl) - 50.f;
        float ang = std::atan2(yl, xl);
        x = pt.fX + std::cos(ang) * hyp;
        y = pt.fY + std::sin(ang) * hyp;
    } else {
        return false;
    }
    out = {x, y + off_y};
    float pa = std::fmod(std::atan2(s2.fY - s1.fY, s2.fX - s1.fX) * 180.f / (float)kPi + 720.f, 360.f);
    float rwy = std::fmod(pa - 90.f + 720.f, 360.f);
    angle = rwy + (rwy > 180 ? 90.f : -90.f);
    return true;
}

}  // namespace

void panel_text(SkCanvas& c, const std::string& t, const char* family, float size, SkColor col, float x, float y, int align) {
    SkFont font = a380::font(family, size);
    font.setEdging(SkFont::Edging::kAntiAlias);
    float w = font.measureText(t.data(), t.size(), SkTextEncoding::kUTF8);
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(col);
    float x0 = align == 0 ? x : align == 1 ? x - w / 2 : x - w;
    a380::draw_simple_text(c, t.data(), t.size(), SkTextEncoding::kUTF8, x0, y, font, p);
}

void panel_text_fit(SkCanvas& c, const std::string& t, const char* family, float size, SkColor col, float x, float y, int align, float max_w) {
    float sz = size;
    while (sz > 14.f) {
        SkFont font = a380::font(family, sz);
        if (font.measureText(t.data(), t.size(), SkTextEncoding::kUTF8) <= max_w) break;
        sz -= 1.f;
    }
    panel_text(c, t, family, sz, col, x, y, align);
}

SkPoint pan_offset(const json* own, int side, const SkMatrix& m, SkPoint centre) {
    static std::mutex mu;
    static std::map<int, std::pair<long long, SkPoint>> pans;
    std::lock_guard<std::mutex> lock(mu);
    const json* pan = nullptr;
    if (own) {
        auto it = own->find("pan");
        if (it != own->end() && it->is_object()) pan = &*it;
    }
    if (!pan || !pan->contains("p") || !(*pan)["p"].is_array() || (*pan)["p"].size() != 2) {
        pans.erase(side);
        return {0, 0};
    }
    long long seq = pan->value("seq", 0LL);
    auto found = pans.find(side);
    if (found != pans.end() && found->second.first == seq) return found->second.second;
    SkPoint p{(*pan)["p"][0].get<float>(), (*pan)["p"][1].get<float>()};
    SkPoint s = m.mapPoint(p);
    SkPoint off{centre.fX - s.fX, centre.fY - s.fY};
    pans[side] = {seq, off};
    return off;
}

void draw_markers(SkCanvas& c, const json* own, const SkMatrix& m) {
    if (!own) return;
    auto it = own->find("markers");
    if (it == own->end() || !it->is_array() || it->empty()) return;
    auto image = [](const char* name) {
        sk_sp<SkImage> img;
        if (auto data = a380::assets::image_data(name))
            if (auto codec = SkCodec::MakeFromData(data)) img = std::get<0>(codec->getImage());
        return img;
    };
    static const sk_sp<SkImage> cross = image("oans/oans-cross.png");
    static const sk_sp<SkImage> flag = image("oans/oans-flag.png");
    for (const auto& mk : *it) {
        if (!mk.is_object() || !mk.contains("p") || !mk["p"].is_array() || mk["p"].size() != 2) continue;
        const bool is_flag = mk.value("k", "") == "f";
        const sk_sp<SkImage>& img = is_flag ? flag : cross;
        if (!img) continue;
        const float size = is_flag ? 40.f : 35.f;
        SkPoint s = m.mapPoint({mk["p"][0].get<float>(), mk["p"][1].get<float>()});
        c.drawImageRect(img, SkRect::MakeXYWH(s.fX - size / 2, s.fY - size / 2, size, size),
                        SkSamplingOptions(SkFilterMode::kLinear, SkMipmapMode::kNone));
    }
}

void draw_control_panel(SkCanvas& c, const json& panel) {
    c.save();
    c.translate(0, 768);
    SkPaint p;
    p.setColor(SK_ColorBLACK);
    c.drawRect(SkRect::MakeXYWH(0, 0, 768, 256), p);
    const SkColor border = SkColorSetRGB(196, 198, 207);
    const SkColor bg = SkColorSetRGB(39, 37, 37);
    const SkColor lgray = SkColorSetRGB(211, 211, 211);
    const SkColor light = SkColorSetRGB(242, 244, 255);
    const SkColor dark = SkColorSetRGB(116, 117, 123);
    auto rect = [&](float x, float y, float w, float h, SkColor col) {
        p.setColor(col);
        p.setAntiAlias(false);
        c.drawRect(SkRect::MakeXYWH(x, y, w, h), p);
    };
    std::vector<std::string> tabs;
    if (panel.contains("tabs") && panel["tabs"].is_array())
        for (const auto& t : panel["tabs"]) tabs.push_back(t.get<std::string>());
    int active = panel.value("active", 0);
    const float cw = 757.f, tw = 245.67f, sl = 25.98f, top = 2.f, th = 45.f;
    rect(9.333f, 47, 2, 207, light);
    rect(764, 47, 2, 207, dark);
    rect(9.333f, 252, 756.667f, 2, dark);
    for (int pass = 0; pass < 2; ++pass) {
        for (size_t i = 0; i < tabs.size(); ++i) {
            bool act = static_cast<int>(i) == active;
            if (act != (pass == 1)) continue;
            float x0 = 9.f + static_cast<float>(i) * (tw - 15.f);
            SkColor fill = act ? SK_ColorBLACK : bg;
            rect(x0 + sl, top, tw - 2 * sl, 2, border);
            rect(x0 + sl, top + 2, tw - 2 * sl, th - 4, fill);
            if (!act) rect(x0, top + th - 2, tw, 2, border);
            SkPaint ap;
            ap.setAntiAlias(true);
            ap.setColor(fill);
            SkPathBuilder l, r;
            l.moveTo(x0, top + th).lineTo(x0 + sl, top).lineTo(x0 + sl, top + th).close();
            r.moveTo(x0 + tw - sl, top).lineTo(x0 + tw, top + th).lineTo(x0 + tw - sl, top + th).close();
            c.drawPath(l.detach(), ap);
            c.drawPath(r.detach(), ap);
            ap.setColor(lgray);
            ap.setStyle(SkPaint::kStroke_Style);
            ap.setStrokeWidth(2.f);
            c.drawLine(x0, top + th, x0 + sl, top, ap);
            c.drawLine(x0 + tw - sl, top, x0 + tw, top + th, ap);
            if (!act) {
                ap.setAntiAlias(false);
                c.drawLine(x0, top + th - 1, x0 + sl, top + th - 1, ap);
                c.drawLine(x0 + tw - sl, top + th - 1, x0 + tw, top + th - 1, ap);
            }
            panel_text(c, tabs[i], "Ecam", 26.f, act ? SkColorSetRGB(0, 255, 255) : kWhite, x0 + tw / 2, act ? 38.f : 36.667f, 1);
        }
    }
    (void)cw;
    if (panel.contains("status") && active == 2) {
        const json& st = panel["status"];
        const char* fbw = "FBW-Display-EIS-A380-SlashedZero";
        const SkColor green = SkColorSetRGB(0, 255, 0);
        panel_text(c, "ACTIVE", fbw, 20.f, kWhite, 189.54f, 74.f, 1);
        panel_text(c, st.value("active", ""), fbw, 30.f, green, 189.7f, 109.333f, 1);
        panel_text(c, "SECOND", fbw, 20.f, kWhite, 585.54f, 74.f, 1);
        panel_text(c, st.value("second", ""), fbw, 20.f, green, 585.6f, 102.f, 1);
        rect(328, 57.333f, 119.333f, 63.333f, border);
        rect(328, 57.333f, 119.333f, 2, light);
        rect(328, 57.333f, 2, 63.333f, light);
        rect(445.333f, 59.333f, 2, 61.333f, dark);
        rect(330, 118.667f, 117.333f, 2, dark);
        rect(330, 59.333f, 115.333f, 59.333f, bg);
        panel_text(c, "SWAP", fbw, 22.f, st.value("swapDisabled", true) ? SkColorSetRGB(134, 138, 156) : kWhite, 387.67f, 97.333f, 1);
        rect(51.333f, 134, 672.667f, 2, lgray);
        panel_text(c, "AIRPORT DATABASE", fbw, 20.f, kWhite, 46.04f, 177.333f, 0);
        panel_text(c, st.value("database", ""), fbw, 25.f, green, 728.3f, 179.333f, 2);
        rect(36, 204, 703.333f, 2, lgray);
    }
    const char* fbw = "FBW-Display-EIS-A380-SlashedZero";
    const SkColor green = SkColorSetRGB(0, 255, 0);
    const SkColor cyan = SkColorSetRGB(0, 255, 255);
    const SkColor grey = SkColorSetRGB(134, 138, 156);
    const SkColor amber = SkColorSetRGB(230, 128, 0);
    const SkColor unit_blue = SkColorSetRGB(33, 33, 255);
    auto outset = [&](float x, float y, float w, float h, SkColor fill) {
        rect(x, y, w, h, border);
        rect(x, y, w, 2, light);
        rect(x, y, 2, h, light);
        rect(x + w - 2, y + 2, 2, h - 2, dark);
        rect(x + 2, y + h - 2, w - 2, 2, dark);
        rect(x + 2, y + 2, w - 4, h - 4, fill);
    };
    auto button = [&](float x, float y, float w, const std::string& label, bool disabled) {
        outset(x, y, w, 44, bg);
        panel_text_fit(c, label, fbw, 22.f, disabled ? grey : kWhite, x + w / 2, y + 30.f, 1, w - 16);
    };
    auto dropdown = [&](float x, float y, float w, const std::string& value, bool inactive) {
        if (inactive) {
            rect(x, y, w, 44, SK_ColorBLACK);
            panel_text_fit(c, value, fbw, 25.f, green, x + (w - 25) / 2, y + 31.f, 1, w - 33);
            return;
        }
        outset(x, y, w, 44, bg);
        rect(x + 2, y + 2, w - 29, 40, SK_ColorBLACK);
        panel_text_fit(c, value, fbw, 25.f, cyan, x + (w - 25) / 2, y + 31.f, 1, w - 33);
        SkPaint ap;
        ap.setAntiAlias(true);
        ap.setColor(kWhite);
        float ax = x + w - 14.5f, ay = y + 22;
        SkPathBuilder tri;
        tri.moveTo(ax - 7, ay - 5).lineTo(ax + 7, ay - 5).lineTo(ax, ay + 6).close();
        c.drawPath(tri.detach(), ap);
    };
    auto radio = [&](float x, float y, const std::string& label, bool checked) {
        SkPaint rp;
        rp.setAntiAlias(true);
        rp.setColor(checked ? cyan : SK_ColorBLACK);
        c.drawCircle(x + 15, y + 16, 13.f, rp);
        if (checked) {
            rp.setColor(SK_ColorBLACK);
            rp.setStyle(SkPaint::kStroke_Style);
            rp.setStrokeWidth(4.f);
            c.drawCircle(x + 15, y + 16, 11.f, rp);
        }
        rp.setStyle(SkPaint::kStroke_Style);
        rp.setStrokeWidth(2.f);
        rp.setColor(border);
        c.drawCircle(x + 15, y + 16, 14.f, rp);
        panel_text(c, label, fbw, 26.f, checked ? cyan : kWhite, x + 45, y + 25.f, 0);
    };
    auto str = [](const json& o, const char* k) {
        auto it = o.find(k);
        return it != o.end() && it->is_string() ? it->get<std::string>() : std::string();
    };
    if (active == 0 && panel.contains("mapData") && panel["mapData"].is_object()) {
        const json& md = panel["mapData"];
        const bool inactive = md.value("selectLandingRunway", false);
        dropdown(21.33f, 52, 168, str(md, "entity"), inactive);
        const int type = md.value("entityType", 0);
        const char* types[] = {"RWY", "TWY", "STAND", "OTHER"};
        for (int k = 0; k < 4; k++) radio(21.33f, 106.f + 35.f * k, types[k], k == type);
        rect(187.5f, 47, 2, 205, lgray);
        const bool off = md.value("buttonsDisabled", true);
        const std::string entity = str(md, "entity");
        if (!md.value("fallback", false)) {
            button(209.5f, 52, 171.5f, str(md, "cross"), off);
            button(391.f, 52, 171.5f, str(md, "flag"), off);
            button(572.5f, 52, 171.5f, "LDG SHIFT", true);
            button(326.75f, 106, 300, "CENTER MAP ON " + entity, off);
            if (!off && type == 0) {
                const float bx = 276.4f, by = 170, bw = 401, bh = 70;
                outset(bx, by, bw, bh, SkColorSetRGB(60, 60, 60));
                const float c1 = bx + 14, c2 = bx + 12 + (bw - 24) * 0.75f, c3 = c2;
                panel_text(c, "RWY: ", fbw, 20.f, kWhite, c1, by + 31, 0);
                panel_text(c, entity, fbw, 20.f, green, c1 + 60, by + 31, 0);
                panel_text(c, "TORA: ", fbw, 20.f, kWhite, c2 - 15, by + 31, 2);
                panel_text(c, str(md, "tora"), fbw, 20.f, green, c3, by + 31, 0);
                panel_text(c, "M", fbw, 20.f, unit_blue, c3 + 14.f * str(md, "tora").size(), by + 31, 0);
                panel_text(c, "LDA: ", fbw, 20.f, kWhite, c2 - 15, by + 59, 2);
                panel_text(c, str(md, "lda"), fbw, 20.f, green, c3, by + 59, 0);
                panel_text(c, "M", fbw, 20.f, unit_blue, c3 + 14.f * str(md, "lda").size(), by + 59, 0);
            } else if (!off && type == 2) {
                const float bx = 276.4f, by = 160, bw = 401, bh = 70;
                outset(bx, by, bw, bh, SkColorSetRGB(60, 60, 60));
                panel_text(c, "STAND:", fbw, 20.f, kWhite, bx + bw / 2 - 30, by + 31, 2);
                panel_text(c, entity, fbw, 20.f, green, bx + bw / 2 - 26, by + 31, 0);
                panel_text(c, "----", fbw, 20.f, green, bx + bw - 14, by + 59, 2);
            }
        } else {
            panel_text(c, "BTV MANUAL CONTROL / FALLBACK", fbw, 20.f, kWhite, 476.75f, 84, 1);
            if (!md.value("selectLandingRunway", false)) {
                panel_text(c, "RUNWAY LENGTH", fbw, 20.f, kWhite, 466.75f, 150, 2);
                panel_text(c, str(md, "lda"), fbw, 20.f, green, 476.75f, 150, 0);
                panel_text(c, "M", fbw, 20.f, unit_blue, 476.75f + 14.f * str(md, "lda").size(), 150, 0);
                panel_text(c, "BTV STOP DISTANCE", fbw, 20.f, kWhite, 466.75f, 200, 2);
                rect(476.75f, 176, 120, 37, border);
                rect(478.75f, 178, 116, 33, SK_ColorBLACK);
                auto it = md.find("reqStoppingDistance");
                std::string sd = it != md.end() && it->is_number() ? std::to_string((int)std::lround(it->get<double>())) : "----";
                panel_text(c, sd, fbw, 27.f, cyan, 580.f, 204, 2);
                panel_text(c, "M", fbw, 20.f, unit_blue, 582.f, 204, 0);
            } else {
                panel_text(c, "SELECT LANDING RUNWAY IN FMS", fbw, 20.f, amber, 476.75f, 175, 1);
            }
        }
    }
    if (active == 1 && panel.contains("arptSel") && panel["arptSel"].is_object()) {
        const json& as = panel["arptSel"];
        dropdown(21.33f, 52, 215.8f, str(as, "airportValue"), false);
        const int mode = as.value("mode", 0);
        const char* modes[] = {"ICAO", "IATA", "CITY NAME"};
        for (int k = 0; k < 3; k++) radio(21.33f, 116.f + 35.f * k, modes[k], k == mode);
        panel_text_fit(c, str(as, "name"), fbw, 25.f, green, 425.3f, 87, 1, 330);
        panel_text_fit(c, str(as, "idents"), fbw, 25.f, green, 425.3f, 120, 1, 330);
        panel_text_fit(c, str(as, "coords"), fbw, 25.f, green, 425.3f, 153, 1, 330);
        button(267.1f, 193, 316.4f, "DISPLAY AIRPORT", false);
        rect(613.5f, 67, 2, 165, lgray);
        const char* keys[] = {"origin", "dest", "altn"};
        for (int k = 0; k < 3; k++) button(628.75f, 67.f + 60.5f * k, 120, str(as, keys[k]), false);
    }
    c.restore();
}


void draw_oans_menu(SkCanvas& c, const Inputs& in, int side) {
    const json* own = in.bus(side == 2 ? "a380.oans.panel.2" : "a380.oans.panel.1");
    if (!own || !own->is_object() || !own->value("open", false)) return;
    const json& panel = *own;
    SkPaint p;
    const SkColor border = SkColorSetRGB(196, 198, 207);
    const SkColor bg = SkColorSetRGB(39, 37, 37);
    const SkColor light = SkColorSetRGB(242, 244, 255);
    const SkColor dark = SkColorSetRGB(116, 117, 123);
    const SkColor cyan = SkColorSetRGB(0, 255, 255);
    const char* fbw = "FBW-Display-EIS-A380-SlashedZero";
    auto mit = panel.find("menu");
    if (mit == panel.end() || !mit->is_object()) return;
    const json& m = *mit;
    const float x = m.value("x", 0.f), y = m.value("y", 0.f), w = m.value("w", 168.f), row = m.value("row", 100.f / 3.f);
    const float h = m.value("h", 0.f);
    const int first = m.value("first", 0), total = m.value("total", 0);
    const bool scroll = m.value("scroll", false);
    auto mrect = [&](float rx, float ry, float rw, float rh, SkColor col) {
        p.setColor(col);
        p.setAntiAlias(false);
        c.drawRect(SkRect::MakeXYWH(rx, ry, rw, rh), p);
    };
    c.save();
    c.clipRect(SkRect::MakeLTRB(0, 0, 768, 1024));
    mrect(x, y, w, h, border);
    mrect(x, y, w, 2, light);
    mrect(x, y, 2, h, light);
    mrect(x + w - 2, y + 2, 2, h - 2, dark);
    mrect(x + 2, y + h - 2, w - 2, 2, dark);
    mrect(x + 2, y + 2, w - 4, h - 4, bg);
    const float bottom = std::min(y + h, 1024.f);
    const float item_w = scroll ? w - 24.f : w - 4.f;
    int sel = m.contains("selected") && m["selected"].is_number() ? m["selected"].get<int>() : -1;
    c.save();
    c.clipRect(SkRect::MakeLTRB(x + 2, y + 2, x + 2 + item_w, bottom - 2));
    SkFont font = a380::font(fbw, 20.f);
    font.setEdging(SkFont::Edging::kAntiAlias);
    int k = 0;
    if (m.contains("items") && m["items"].is_array())
        for (const auto& it : m["items"]) {
            const float ry = y + 2 + row * k;
            const bool on = first + k == sel;
            if (on) {
                SkPaint sp;
                sp.setColor(cyan);
                sp.setStyle(SkPaint::kStroke_Style);
                sp.setStrokeWidth(3.f);
                c.drawRect(SkRect::MakeXYWH(x + 3.5f, ry + 1.5f, item_w - 3, row - 3), sp);
            }
            const std::string t = it.is_string() ? it.get<std::string>() : std::string();
            SkPaint tp;
            tp.setAntiAlias(true);
            tp.setColor(on ? cyan : kWhite);
            a380::draw_simple_text(c, t.data(), t.size(), SkTextEncoding::kUTF8, x + 2 + 3 + 16, a380::chromium_baseline(font, ry + 3 + 5, 0.f, 1.5f), font, tp);
            k++;
        }
    c.restore();
    if (scroll && total > 0) {
        const float track_y = y + 2, track_h = bottom - 2 - track_y;
        const float content = row * total;
        const float view = std::min(h - 4, bottom - 2 - track_y);
        mrect(x + w - 22, track_y, 20, track_h, bg);
        const float thumb_h = std::max(12.f, track_h * view / content);
        const float max_first = (float)std::max(1, total - 4);
        const float thumb_y = track_y + (track_h - thumb_h) * std::min(1.f, first / max_first);
        mrect(x + w - 22, thumb_y, 20, thumb_h, SkColorSetRGB(211, 211, 211));
    }
    c.restore();
}

bool oans_shown(const Inputs& in, int side) {
    const char* s = side == 2 ? "R" : "L";
    std::string pre = std::string("L:A32NX_EFIS_") + s;
    int range = (int)std::lround(in.var(pre + "_ND_RANGE", {}, 3));
    int mode = (int)std::lround(in.var(pre + "_ND_MODE", {}, 3));
    return range == 0 && (mode == 2 || mode == 3 || mode == 4);
}

void draw_oans(SkCanvas& canvas, const Inputs& in, const Frame& f) {
    const char* s = f.side == 2 ? "R" : "L";
    int zoom = (int)std::lround(in.var(std::string("L:A32NX_EFIS_") + s + "_OANS_RANGE", {}, 0));
    zoom = std::max(0, std::min(4, zoom));
    bool arc = f.mode == 3;
    bool rose = f.mode == 2;

    const SkPoint centre = arc ? SkPoint{384.f, 620.f} : SkPoint{384.f, 384.f};

    std::string icao = airport_icao(in);
    if (icao.empty()) {
        if (const json* b = in.bus("fmsOrigin"); b && b->is_string()) icao = upper(b->get<std::string>());
    }
    std::shared_ptr<Airport> ap = icao.empty() ? nullptr : load_airport(in, icao);

    SkPaint black;
    black.setColor(SK_ColorBLACK);

    auto centre_flag = [&](const char* text) {
        SkFont font = a380::font("Ecam", 32.f);
        font.setEdging(SkFont::Edging::kAntiAlias);
        SkPaint tp;
        tp.setAntiAlias(true);
        tp.setColor(SkColorSetRGB(255, 154, 0));
        std::string t = text;
        float w = font.measureText(t.data(), t.size(), SkTextEncoding::kUTF8);
        a380::draw_simple_text(canvas, t.data(), t.size(), SkTextEncoding::kUTF8, 384 - w / 2, 384 + 11.f, font, tp);
    };
    if (const json* na = in.bus("oans_not_avail"); na && na->is_boolean() && na->get<bool>()) {
        centre_flag("NOT AVAIL");
        return;
    }
    if (!f.map_available && f.mode != 4) {
        centre_flag("ARPT NAV POS LOST");
        return;
    }

    const json* own = in.bus(f.side == 2 ? "a380.oans.panel.2" : "a380.oans.panel.1");
    if (own && !own->is_object()) own = nullptr;

    bool ppos_ref = true;
    SkPoint ac_m{0, 0};
    SkPoint pan_px{0, 0};
    if (ap && ap->ok) {
        double scale = kNmPerKm / (2.0 * kZoomRangeNm[zoom]) * (rose ? 0.5 : 1.0);
        SkPoint ac = azeq(ap->arp_lat, ap->arp_lon, f.ref_lat, f.ref_lon);
        int phase = (int)std::lround(in.var("L:A32NX_FWC_FLIGHT_PHASE", {}, 1));
        bool on_ground = !(phase >= 6 && phase <= 9);
        bool within = ac.fX >= ap->bounds.fLeft && ac.fX <= ap->bounds.fRight && ac.fY >= ap->bounds.fTop && ac.fY <= ap->bounds.fBottom;
        bool use_ppos = arc || (on_ground && within);
        ppos_ref = use_ppos;
        ac_m = ac;
        if (!use_ppos) ac = {0.f, 0.f};
        double h = (f.mode == 4 ? 0.0 : f.up_true_deg) * kPi / 180;
        float ch = (float)std::cos(h), sh = (float)std::sin(h);
        float sc = (float)scale;
        // screen = centre + s * (E*cosH - N*sinH, -(E*sinH + N*cosH)) relative to aircraft
        SkMatrix m;
        m.setAll(sc * ch, -sc * sh, centre.fX - sc * (ac.fX * ch - ac.fY * sh),
                 -sc * sh, -sc * ch, centre.fY + sc * (ac.fX * sh + ac.fY * ch),
                 0, 0, 1);
        pan_px = pan_offset(own, f.side, m, centre);
        m.postTranslate(pan_px.fX, pan_px.fY);

        canvas.save();
        canvas.clipRect(SkRect::MakeLTRB(0, 60, 768, 736));
        canvas.save();
        {
            SkMatrix inv;
            if (m.invert(&inv)) g_view = inv.mapRect(SkRect::MakeLTRB(0, 60, 768, 736));
        }
        auto layer0 = [&](SkCanvas& cv) {
            fill_features(cv, *ap, "taxiwayelement", SkColorSetRGB(0x8f, 0x8f, 0x8f));
            fill_features(cv, *ap, "taxiwayshoulder", SkColorSetRGB(0x85, 0x45, 0x1d));
            fill_features(cv, *ap, "serviceroad", SkColorSetRGB(0xb5, 0x98, 0x24));
            SkColor grey = SkColorSetRGB(128, 128, 128);
            for (const char* l : {"runwayelement", "runwayintersection", "blastpad", "runwaydisplacedarea", "stopway"})
                fill_features(cv, *ap, l, grey);
            fill_features(cv, *ap, "runwaymarking", kWhite);
            fill_features(cv, *ap, "runwayshoulder", SkColorSetRGB(0x85, 0x45, 0x1d));
            stroke_features(cv, *ap, {"taxiwayguidanceline", "runwayexitline"}, kYellow, 1.85f);
            stroke_features(cv, *ap, {"taxiwayholdingposition"}, SkColorSetRGB(0xff, 0x2f, 0x00), 1.f);
        };
        auto layer1 = [&](SkCanvas& cv) {
            fill_features(cv, *ap, "apronelement", SkColorSetRGB(0x54, 0x54, 0x54));
            fill_features(cv, *ap, "parkingstandarea", SkColorSetRGB(0x77, 0x85, 0x85));
            fill_features(cv, *ap, "verticalpolygonalstructure", kCyan, 1);
            fill_features(cv, *ap, "verticalpolygonalstructure", SkColorSetRGB(0x32, 0x86, 0xda), 4);
        };
        auto layer2 = [&](SkCanvas& cv) {
            stroke_features(cv, *ap, {"taxiwayguidanceline", "runwayexitline"}, SkColorSetRGB(0x66, 0x66, 0x66), 8.f);
            for (const char* l : {"runwayelement", "runwayintersection", "runwaydisplacedarea", "stopway"})
                fill_features(cv, *ap, l, kWhite);
        };
        auto layer3 = [&](SkCanvas& cv) { stroke_features(cv, *ap, {"standguidanceline"}, kYellow, 1.85f); };
        canvas.concat(m);
        SkRect view = g_view;
        auto composite = [&](float rs, const std::function<void(SkCanvas&)>& draw) {
            SkRect win = view;
            if (!win.intersect(ap->bounds.makeOutset(4.f, 4.f))) return;
            float umin = ap->bounds.fLeft, nmax = ap->bounds.fBottom;
            float step = 1.f / rs;
            float ux0 = std::floor((win.fLeft - umin) / step) * step, ux1 = std::ceil((win.fRight - umin) / step) * step;
            float vy0 = std::floor((nmax - win.fBottom) / step) * step, vy1 = std::ceil((nmax - win.fTop) / step) * step;
            int w = std::max(1, (int)std::lround((ux1 - ux0) * rs)), h = std::max(1, (int)std::lround((vy1 - vy0) * rs));
            SkBitmap bm;
            bm.allocPixels(SkImageInfo::MakeN32Premul(w, h));
            bm.eraseColor(SK_ColorTRANSPARENT);
            SkCanvas cv(bm);
            SkMatrix mb;
            mb.setAll(rs, 0, -(umin + ux0) * rs, 0, -rs, (nmax - vy0) * rs, 0, 0, 1);
            cv.concat(mb);
            SkRect saved = g_view;
            g_view = SkRect::MakeLTRB(umin + ux0, nmax - vy1, umin + ux1, nmax - vy0);
            draw(cv);
            g_view = saved;
            SkMatrix inv;
            if (!mb.invert(&inv)) return;
            float wf = ap->bounds.width(), hf = ap->bounds.height();
            float dx = (std::floor(wf * rs) - wf * rs) * 0.5f;
            float dy = (std::floor(hf * rs) - hf * rs) * 0.5f;
            canvas.save();
            const float lx = rs < 1.f ? kLayoutShiftX25 : kLayoutShiftScale1, ly = rs < 1.f ? kLayoutShiftY25 : kLayoutShiftScale1;
            canvas.setMatrix(SkMatrix::Concat(SkMatrix::Translate(dx * lx, dy * ly), canvas.getTotalMatrix()));
            if (rs >= 1.f) canvas.concat(SkMatrix::Translate(-dx * kCanvasScaleShift, dy * kCanvasScaleShift));
            else canvas.concat(SkMatrix::Translate(-dx / rs * kWorldShift25, dy / rs * kWorldShift25));
            canvas.concat(inv);
            canvas.drawImage(bm.asImage(), 0, 0, SkSamplingOptions(SkFilterMode::kLinear));
            canvas.restore();
        };
        if (zoom <= 1) composite(1.f, layer0);
        composite(1.f, layer1);
        if (zoom >= 2) composite(0.25f, layer2);
        if (zoom == 0) composite(1.f, layer3);
        canvas.restore();

        auto rwy_of = [&](const char* topic) {
            const json* b = in.bus(topic);
            std::string v = b && b->is_string() ? b->get<std::string>() : std::string();
            return v.size() > 4 ? v.substr(4) : std::string("-");
        };
        std::string btv_rwy = rwy_of("oansSelectedLandingRunway");
        if (btv_rwy == "-") btv_rwy.clear();
        std::string fms_dep = rwy_of("fmsDepartureRunway"), fms_ldg = rwy_of("fmsLandingRunway");
        for (auto& l : ap->labels) {
            const bool vis = label_visible(l, arc, zoom, btv_rwy);
            if (!vis && l.style != 4) continue;
            if (l.style == 3) {
                if (!arc) continue;
                SkPoint ap;
                float ang;
                if (runway_axis_pos(l, m, arc ? 236.f : 0.f, ap, ang)) {
                    canvas.save();
                    canvas.translate(ap.fX, ap.fY);
                    canvas.rotate(ang);
                    draw_label_box(canvas, l, 0, 0, kWhite);
                    canvas.restore();
                }
                continue;
            }
            float x = m.getScaleX() * l.pos.fX + m.getSkewX() * l.pos.fY + m.getTranslateX();
            float y = m.getSkewY() * l.pos.fX + m.getScaleY() * l.pos.fY + m.getTranslateY();
            if (x < 0 || x > 768 || y < 0 || y > 768) continue;
            if (l.style == 4) {
                float hd = (float)(f.mode == 4 ? 0.0 : f.up_true_deg);
                const SkPoint sp = snap_device(canvas, {x, y});
                x = sp.fX;
                y = sp.fY;
                if (vis) draw_runway_end(canvas, l, {x, y}, hd, !btv_rwy.empty() && l.text == btv_rwy);
                if (!btv_rwy.empty() && l.text == btv_rwy) {
                    SkPathBuilder ab;
                    ab.moveTo(5, 5).lineTo(25, 30).lineTo(45, 5);
                    SkPath arrow = ab.detach();
                    SkPaint ap2;
                    ap2.setAntiAlias(true);
                    ap2.setStyle(SkPaint::kStroke_Style);
                    ap2.setStrokeCap(SkPaint::kRound_Cap);
                    ap2.setStrokeJoin(SkPaint::kRound_Join);
                    canvas.save();
                    canvas.translate(x, y);
                    canvas.rotate(l.rotation - hd);
                    canvas.translate(0, -100.f);
                    canvas.rotate(-180.f);
                    canvas.translate(-25.f, -17.5f);
                    ap2.setColor(SK_ColorBLACK);
                    ap2.setStrokeWidth(3.f);
                    canvas.drawPath(arrow, ap2);
                    ap2.setColor(kCyan);
                    ap2.setStrokeWidth(2.f);
                    canvas.drawPath(arrow, ap2);
                    canvas.restore();
                }
                if (l.text == fms_dep || l.text == fms_ldg) {
                    float r = l.rotation - hd;
                    float rad = r * (float)kPi / 180.f;
                    float cx = x - 82.5f * std::sin(rad), cy = y + 82.5f * std::cos(rad);
                    SkPathBuilder tb;
                    tb.moveTo(0, -13.75f).lineTo(-30, 13.75f).lineTo(30, 13.75f).close();
                    SkPaint gp;
                    gp.setAntiAlias(true);
                    gp.setColor(SkColorSetRGB(0, 255, 0));
                    canvas.save();
                    canvas.translate(cx, cy);
                    canvas.rotate(r);
                    canvas.drawPath(tb.detach(), gp);
                    canvas.restore();
                }
            } else {
                draw_html_label(canvas, l, x, y, l.style == 2 ? kCyan : kYellow);
            }
        }
        draw_markers(canvas, own, m);
        canvas.restore();
        if (ppos_ref && f.map_available) {
            static const char* kPosLayers[] = {"parkingstandarea", "taxiwayelement", "runwayelement", "stopway",
                                               "runwaydisplacedarea", "blastpad"};
            std::string where;
            for (const char* pl : kPosLayers) {
                for (auto& ft : ap->features) {
                    if (ft.layer != pl || ft.paths.empty()) continue;
                    if (!ft.paths[0].contains(ac_m.fX, ac_m.fY)) continue;
                    if (ft.layer == "parkingstandarea") where = ft.idstd;
                    else if (ft.layer == "taxiwayelement") where = ft.idlin;
                    else if (ft.layer == "runwayelement" || ft.layer == "stopway") {
                        where = ft.idrwy;
                        for (auto& ch : where) if (ch == '.') ch = '-';
                    } else where = ft.idthr;
                    break;
                }
                if (!where.empty()) break;
            }
            if (!where.empty()) {
                SkPaint mask;
                mask.setColor(SK_ColorBLACK);
                canvas.drawRect(SkRect::MakeXYWH(0, 736, 768, 32), mask);
                SkFont font = a380::font("Ecam", 24.f);
                font.setEdging(SkFont::Edging::kAntiAlias);
                float w = font.measureText(where.data(), where.size(), SkTextEncoding::kUTF8);
                float bw = w + 3.f + 1.f + 3.f;
                float bx = 384.f - bw / 2;
                SkPaint bp;
                bp.setAntiAlias(true);
                bp.setStyle(SkPaint::kStroke_Style);
                bp.setStrokeWidth(1.5f);
                bp.setColor(kYellow);
                canvas.drawRect(SkRect::MakeXYWH(bx + 0.75f, 740.75f, bw - 1.5f, 24.f), bp);
                SkPaint tp;
                tp.setAntiAlias(true);
                tp.setColor(kYellow);
                a380::draw_simple_text(canvas, where.data(), where.size(), SkTextEncoding::kUTF8, bx + 3.5f, 759.f, font, tp);
            }
        }
    }

    if (f.mode == 3 || f.mode == 2) {
        float rot = arc ? std::round(wrap180(60.0 - f.up_true_deg) * 100.f) / 100.f : wrap180(-f.up_true_deg);
        SkPoint cc = arc ? SkPoint{384.f, 620.f} : SkPoint{384.f, 384.f};
        draw_ring(canvas, arc, cc, rot);
        draw_dashed_circle(canvas, cc, arc ? 246.f : 125.f, arc);
        if (!arc) {
            static const char* kRng[] = {"0.1", "0.25", "0.5", "1", "2.5"};
            std::string t = kRng[zoom];
                        SkFont font = a380::font("Ecam", 22.f);
                        font.setEdging(SkFont::Edging::kAntiAlias);
            SkPaint tp;
            tp.setAntiAlias(true);
            tp.setColor(kCyan);
            canvas.drawRect(SkRect::MakeXYWH(212, 538, t.size() == 4 ? 55.f : 41.f, 21.f), black);
            a380::draw_simple_text(canvas, t.data(), t.size(), SkTextEncoding::kUTF8, 212, 556, font, tp);
        }
        SkPaint lub;
        lub.setAntiAlias(true);
        lub.setStyle(SkPaint::kStroke_Style);
        lub.setStrokeWidth(4.f);
        lub.setColor(kMagenta);
        canvas.drawLine(384.f, arc ? 106.f : 114.f, 384.f, arc ? 150.f : 153.f, lub);
    }

    canvas.save();
    SkPaint shadow;
    shadow.setAntiAlias(true);
    shadow.setStyle(SkPaint::kStroke_Style);
    shadow.setStrokeCap(SkPaint::kRound_Cap);
    shadow.setColor(SK_ColorBLACK);
    shadow.setStrokeWidth(10.f);
    SkPaint mag = shadow;
    mag.setColor(kMagenta);
    mag.setStrokeWidth(7.75f);
    SkPathBuilder ib;
    float ox = centre.fX - 45.f, oy = centre.fY - 39.625f + 4.5f;
    ib.moveTo(ox + 4, oy + 29.5f);
    ib.lineTo(ox + 86, oy + 29.5f);
    ib.moveTo(ox + 45, oy);
    ib.lineTo(ox + 45, oy + 70.25f);
    ib.moveTo(ox + 33.5f, oy + 60.5f);
    ib.lineTo(ox + 57.f, oy + 60.5f);
    SkPath icon = ib.detach();
    if (ppos_ref && ap && ap->ok) {
        canvas.translate(pan_px.fX, pan_px.fY);
        canvas.rotate((float)(f.true_heading_deg - (f.mode == 4 ? 0.0 : f.up_true_deg)), centre.fX, centre.fY);
        canvas.drawPath(icon, shadow);
        canvas.drawPath(icon, mag);
    }
    canvas.restore();

    const json* panel = nullptr;
    if (own) {
        if (own->value("open", false)) panel = own;
    } else if (const json* mdl = in.bus("a380.mfd.model")) {
        auto it = mdl->find("oans_panel");
        if (it != mdl->end() && it->is_object() && it->value("open", false)) panel = &*it;
    }
    if (panel) draw_control_panel(canvas, *panel);

    {
        SkPaint lp;
        lp.setColor(SkColorSetRGB(242, 244, 255));
        canvas.drawRect(SkRect::MakeXYWH(717, 768, 49, 45), lp);
        lp.setColor(SkColorSetRGB(116, 117, 123));
        canvas.drawRect(SkRect::MakeXYWH(764, 770, 2, 43), lp);
        canvas.drawRect(SkRect::MakeXYWH(719, 811, 47, 2), lp);
        lp.setColor(SkColorSetRGB(39, 37, 37));
        canvas.drawRect(SkRect::MakeXYWH(719, 770, 45, 41), lp);
        lp.setColor(kWhite);
        lp.setAntiAlias(true);
        for (float oy : {773.3f, 790.7f}) {
            SkPathBuilder tb;
            if (panel) tb.moveTo(724.f, oy + 17.4f).lineTo(758.7f, oy + 17.4f).lineTo(741.3f, oy).close();
            else tb.moveTo(724.f, oy).lineTo(758.7f, oy).lineTo(741.3f, oy + 17.4f).close();
            canvas.drawPath(tb.detach(), lp);
        }
    }

    if (ap && ap->ok) {
        std::string l1 = upper(ap->name);
        std::string l2 = ap->icao + "  " + ap->iata;
        SkFont font = a380::font("Ecam", 20.f);
        font.setEdging(SkFont::Edging::kAntiAlias);
        SkPaint tp;
        tp.setAntiAlias(true);
        tp.setColor(kWhite);
        float w1 = font.measureText(l1.data(), l1.size(), SkTextEncoding::kUTF8);
        float w2 = font.measureText(l2.data(), l2.size(), SkTextEncoding::kUTF8);
        canvas.drawRect(SkRect::MakeXYWH(768 - w1, 5, w1, 26), black);
        canvas.drawRect(SkRect::MakeXYWH(768 - w2, 31, w2, 26), black);
        float y1 = a380::chromium_baseline(font, 5.f, 0.f, 1.5f);
        float y2 = a380::chromium_baseline(font, 31.f, 0.f, 1.5f);
        a380::draw_simple_text(canvas, l1.data(), l1.size(), SkTextEncoding::kUTF8, 768 - w1, y1, font, tp);
        a380::draw_simple_text(canvas, l2.data(), l2.size(), SkTextEncoding::kUTF8, 768 - w2, y2, font, tp);
    }
}

}  // namespace a380::nd
