#include <algorithm>
#include "a380/env.h"
#include <cctype>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <functional>
#include <iterator>
#include <string>
#include <tuple>
#include <unordered_map>
#include <vector>

#include "a380/assets.h"
#include "a380/display.h"
#include "a380/draw.h"
#include "a380/fonts.h"
#include "a380/layout.h"
#include "a380/raster.h"
#include "dom.h"
#include "domdisplay.h"
#include "svg.h"
#include "include/codec/SkCodec.h"
#include "include/core/SkData.h"
#include "include/core/SkImage.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkRRect.h"
#include "include/core/SkSamplingOptions.h"
#include "include/effects/SkGradient.h"
#include "include/core/SkSurface.h"
#include "include/core/SkBlurTypes.h"
#include "include/core/SkColorFilter.h"
#include "include/core/SkFontMetrics.h"
#include "include/core/SkMaskFilter.h"
#include "include/effects/SkColorMatrix.h"
#include "include/effects/SkImageFilters.h"

namespace a380::mfdx {

namespace {

using layout::Node;
using json = nlohmann::json;

struct Rgba {
    float r = 0, g = 0, b = 0, a = 0;
};

Rgba parse_color(const std::string& s) {
    Rgba c;
    if (s.empty()) return c;
    if (s[0] == '#') {
        std::string h = s.substr(1);
        auto hx = [&](size_t i, size_t len) { return (float)std::strtol(h.substr(i, len).c_str(), nullptr, 16); };
        if (h.size() == 3 || h.size() == 4) {
            c.r = hx(0, 1) * 17.f;
            c.g = hx(1, 1) * 17.f;
            c.b = hx(2, 1) * 17.f;
            c.a = h.size() == 4 ? hx(3, 1) * 17.f / 255.f : 1.f;
        } else if (h.size() == 6 || h.size() == 8) {
            c.r = hx(0, 2);
            c.g = hx(2, 2);
            c.b = hx(4, 2);
            c.a = h.size() == 8 ? hx(6, 2) / 255.f : 1.f;
        }
        return c;
    }
    if (s == "white") return Rgba{255, 255, 255, 1};
    if (s == "black") return Rgba{0, 0, 0, 1};
    const char* p = std::strchr(s.c_str(), '(');
    if (!p) return c;
    float v[4] = {0, 0, 0, 1};
    int n = 0;
    p++;
    while (*p && n < 4) {
        char* end = nullptr;
        float val = std::strtof(p, &end);
        if (end == p) break;
        v[n++] = val;
        p = end;
        while (*p == ',' || *p == ' ' || *p == '/') p++;
    }
    c.r = v[0];
    c.g = v[1];
    c.b = v[2];
    c.a = n >= 4 ? v[3] : 1.f;
    return c;
}

SkColor to_sk(const Rgba& c, float opacity = 1.f) {
    auto cl = [](float x) { return (uint8_t)std::lround(std::clamp(x, 0.f, 255.f)); };
    return SkColorSetARGB(cl(c.a * opacity * 255.f), cl(c.r), cl(c.g), cl(c.b));
}

float px_of(const std::string& s) { return s.empty() ? 0.f : std::strtof(s.c_str(), nullptr); }

void escape(std::string& out, const std::string& s) {
    for (char c : s) {
        if (c == '"' || c == '\\') out += '\\';
        if ((unsigned char)c < 0x20) continue;
        out += c;
    }
}


std::string image_relative(std::string url) {
    size_t a = url.find("url(");
    if (a != std::string::npos) {
        url = url.substr(a + 4);
        size_t b = url.rfind(')');
        if (b != std::string::npos) url.resize(b);
    }
    url.erase(std::remove(url.begin(), url.end(), '"'), url.end());
    url.erase(std::remove(url.begin(), url.end(), '\''), url.end());
    const std::string key = "Images/fbw-a380x/";
    size_t k = url.find(key);
    return k == std::string::npos ? url : url.substr(k + key.size());
}

sk_sp<SkImage> load_image(const std::string& relative) {
    thread_local std::unordered_map<std::string, sk_sp<SkImage>> cache;
    auto it = cache.find(relative);
    if (it != cache.end()) return it->second;
    sk_sp<SkImage> img;
    if (auto data = a380::assets::image_data(relative)) {
        if (auto codec = SkCodec::MakeFromData(data)) img = std::get<0>(codec->getImage());
    }
    cache[relative] = img;
    return img;
}

sk_sp<SkImage> load_data_url(const std::string& url) {
    thread_local std::unordered_map<size_t, sk_sp<SkImage>> cache;
    size_t key = std::hash<std::string>{}(url);
    auto it = cache.find(key);
    if (it != cache.end()) return it->second;
    sk_sp<SkImage> img;
    size_t comma = url.find(',');
    size_t b64 = url.find(";base64");
    if (comma != std::string::npos && b64 != std::string::npos && b64 < comma) {
        static const std::string abc = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        int table[256];
        std::fill(std::begin(table), std::end(table), -1);
        for (int i = 0; i < 64; i++) table[(unsigned char)abc[i]] = i;
        std::vector<uint8_t> out;
        out.reserve((url.size() - comma) * 3 / 4);
        uint32_t acc = 0;
        int bits = 0;
        for (size_t i = comma + 1; i < url.size(); i++) {
            int v = table[(unsigned char)url[i]];
            if (v < 0) continue;
            acc = (acc << 6) | (uint32_t)v;
            bits += 6;
            if (bits >= 8) {
                bits -= 8;
                out.push_back((uint8_t)(acc >> bits));
            }
        }
        if (auto codec = SkCodec::MakeFromData(SkData::MakeWithCopy(out.data(), out.size()))) img = std::get<0>(codec->getImage());
    }
    cache[key] = img;
    return img;
}

std::vector<std::string> split_top(const std::string& s);

struct Shadow {
    bool inset = false;
    float x = 0, y = 0, blur = 0, spread = 0;
    SkColor col = 0;
};

std::vector<std::string> tokens(const std::string& s) {
    std::vector<std::string> t;
    size_t i = 0;
    while (i < s.size()) {
        while (i < s.size() && s[i] == ' ') i++;
        size_t j = i;
        int depth = 0;
        while (j < s.size() && (depth > 0 || s[j] != ' ')) {
            if (s[j] == '(') depth++;
            if (s[j] == ')') depth--;
            j++;
        }
        if (j > i) t.push_back(s.substr(i, j - i));
        i = j;
    }
    return t;
}

std::vector<Shadow> parse_shadows(const std::string& v) {
    std::vector<Shadow> out;
    if (v.empty() || v == "none") return out;
    for (const std::string& part : split_top(v)) {
        Shadow sh;
        float len[4] = {0, 0, 0, 0};
        int nl = 0;
        bool have_col = false;
        Rgba col;
        for (const std::string& tok : tokens(part)) {
            if (tok == "inset") sh.inset = true;
            else if (tok.rfind("rgb", 0) == 0) {
                col = parse_color(tok);
                have_col = true;
            } else if (std::isdigit((unsigned char)tok[0]) || tok[0] == '-' || tok[0] == '.') {
                if (nl < 4) len[nl++] = std::strtof(tok.c_str(), nullptr);
            }
        }
        if (nl < 2 || !have_col || col.a <= 0.f) continue;
        sh.x = len[0];
        sh.y = len[1];
        sh.blur = std::max(0.f, len[2]);
        sh.spread = len[3];
        sh.col = to_sk(col);
        out.push_back(sh);
    }
    return out;
}

bool css_number(const char*& p, float& v, std::string& unit) {
    while (*p == ' ' || *p == ',') p++;
    char* e = nullptr;
    float f = std::strtof(p, &e);
    if (e == p) return false;
    p = e;
    unit.clear();
    while (*p && (std::isalpha((unsigned char)*p) || *p == '%')) unit += *p++;
    v = f;
    return true;
}

SkMatrix css_transform(const std::string& t, float w, float h) {
    SkMatrix m;
    if (t.empty() || t == "none") return m;
    size_t p = 0;
    while (p < t.size()) {
        while (p < t.size() && t[p] == ' ') p++;
        size_t lp = t.find('(', p);
        if (lp == std::string::npos) break;
        size_t rp = t.find(')', lp);
        if (rp == std::string::npos) break;
        std::string name = t.substr(p, lp - p);
        std::string args = t.substr(lp + 1, rp - lp - 1);
        std::vector<float> a;
        std::vector<std::string> u;
        const char* q = args.c_str();
        float v;
        std::string unit;
        while (css_number(q, v, unit)) {
            a.push_back(v);
            u.push_back(unit);
        }
        auto len = [&](size_t i, float ref) { return i < a.size() ? (u[i] == "%" ? a[i] * ref / 100.f : a[i]) : 0.f; };
        auto ang = [&](size_t i) {
            if (i >= a.size()) return 0.f;
            if (u[i] == "rad") return a[i] * 180.f / 3.14159265f;
            if (u[i] == "turn") return a[i] * 360.f;
            if (u[i] == "grad") return a[i] * 0.9f;
            return a[i];
        };
        auto tan_deg = [](float d) { return std::tan(d * 3.14159265f / 180.f); };
        SkMatrix f;
        if (name == "matrix" && a.size() >= 6) f = SkMatrix::MakeAll(a[0], a[2], a[4], a[1], a[3], a[5], 0, 0, 1);
        else if (name == "matrix3d" && a.size() >= 16) f = SkMatrix::MakeAll(a[0], a[4], a[12], a[1], a[5], a[13], 0, 0, 1);
        else if (name == "translate") f = SkMatrix::Translate(len(0, w), len(1, h));
        else if (name == "translateX") f = SkMatrix::Translate(len(0, w), 0);
        else if (name == "translateY") f = SkMatrix::Translate(0, len(0, h));
        else if (name == "scale") f = SkMatrix::Scale(a.empty() ? 1.f : a[0], a.size() > 1 ? a[1] : (a.empty() ? 1.f : a[0]));
        else if (name == "scaleX") f = SkMatrix::Scale(a.empty() ? 1.f : a[0], 1.f);
        else if (name == "scaleY") f = SkMatrix::Scale(1.f, a.empty() ? 1.f : a[0]);
        else if (name == "rotate" || name == "rotateZ") f = SkMatrix::RotateDeg(ang(0));
        else if (name == "skew") f = SkMatrix::Skew(tan_deg(ang(0)), tan_deg(ang(1)));
        else if (name == "skewX") f = SkMatrix::Skew(tan_deg(ang(0)), 0.f);
        else if (name == "skewY") f = SkMatrix::Skew(0.f, tan_deg(ang(0)));
        m = SkMatrix::Concat(m, f);
        p = rp + 1;
    }
    return m;
}

float position_component(const std::string& tok, float free) {
    if (tok == "left" || tok == "top") return 0.f;
    if (tok == "center") return free * 0.5f;
    if (tok == "right" || tok == "bottom") return free;
    char* e = nullptr;
    float v = std::strtof(tok.c_str(), &e);
    if (e == tok.c_str()) return 0.f;
    return *e == '%' ? free * v / 100.f : v;
}

void css_position(const std::string& pos, float free_x, float free_y, float& x, float& y) {
    std::vector<std::string> t = tokens(pos);
    x = y = 0.f;
    if (t.empty()) return;
    if (t.size() == 1) {
        if (t[0] == "top" || t[0] == "bottom") t.insert(t.begin(), "center");
        else t.push_back("center");
    }
    if (t[0] == "top" || t[0] == "bottom" || t[1] == "left" || t[1] == "right") std::swap(t[0], t[1]);
    x = position_component(t[0], free_x);
    y = position_component(t[1], free_y);
}

struct Radii {
    float x[4] = {0, 0, 0, 0}, y[4] = {0, 0, 0, 0};
    bool any = false;
};

Radii read_radii(const Node& n, float w, float h, bool corners_follow_first = false) {
    static const char* names[4] = {"border-top-left-radius", "border-top-right-radius", "border-bottom-right-radius", "border-bottom-left-radius"};
    Radii r;
    for (int i = 0; i < 4; i++) {
        std::string v = n.get(names[i]);
        if (corners_follow_first && i > 0 && v.empty()) v = n.get(names[0]);
        float t[2] = {0, 0};
        bool pct[2] = {false, false};
        int cnt = 0;
        const char* p = v.c_str();
        while (*p && cnt < 2) {
            while (*p == ' ') p++;
            if (!*p) break;
            char* e = nullptr;
            float f = std::strtof(p, &e);
            if (e == p) break;
            t[cnt] = f;
            pct[cnt] = *e == '%';
            cnt++;
            p = e;
            while (*p && *p != ' ') p++;
        }
        if (!cnt) continue;
        if (cnt == 1) {
            t[1] = t[0];
            pct[1] = pct[0];
        }
        r.x[i] = std::max(0.f, pct[0] ? t[0] * w / 100.f : t[0]);
        r.y[i] = std::max(0.f, pct[1] ? t[1] * h / 100.f : t[1]);
    }
    float f = 1.f;
    auto side = [&](float a, float b, float len) {
        if (a + b > len && a + b > 0.f) f = std::min(f, len / (a + b));
    };
    side(r.x[0], r.x[1], w);
    side(r.x[3], r.x[2], w);
    side(r.y[0], r.y[3], h);
    side(r.y[1], r.y[2], h);
    for (int i = 0; i < 4; i++) {
        r.x[i] *= f;
        r.y[i] *= f;
        if (r.x[i] > 0.f && r.y[i] > 0.f) r.any = true;
    }
    return r;
}

SkRRect make_rrect(float x, float y, float w, float h, const Radii& r, float il, float it, float ir, float ib) {
    SkVector v[4];
    const float dx[4] = {il, ir, ir, il}, dy[4] = {it, it, ib, ib};
    for (int i = 0; i < 4; i++) {
        float rx = std::max(0.f, r.x[i] - dx[i]), ry = std::max(0.f, r.y[i] - dy[i]);
        if (rx <= 0.f || ry <= 0.f) rx = ry = 0.f;
        v[i] = {rx, ry};
    }
    SkRRect rr;
    rr.setRectRadii(SkRect::MakeXYWH(x + il, y + it, std::max(0.f, w - il - ir), std::max(0.f, h - it - ib)), v);
    return rr;
}

std::vector<std::string> split_top(const std::string& s) {
    std::vector<std::string> out;
    int depth = 0;
    std::string cur;
    for (char ch : s) {
        if (ch == '(') depth++;
        if (ch == ')') depth--;
        if (ch == ',' && depth == 0) {
            out.push_back(cur);
            cur.clear();
        } else cur += ch;
    }
    out.push_back(cur);
    for (auto& a : out) {
        size_t b = a.find_first_not_of(' ');
        size_t e = a.find_last_not_of(' ');
        a = b == std::string::npos ? "" : a.substr(b, e - b + 1);
    }
    return out;
}

void content_extent(const Node& n, const Node& scroller, float& right, float& bottom);

float device_border(float bw, float dpr) {
    float d = std::floor(bw * dpr + 1e-3f);
    if (bw > 0.f && d < 1.f) d = 1.f;
    return d / dpr;
}

struct Painter {
    SkCanvas& c;
    float dpr;
    bool lcd_all = false;
    const std::vector<ClassBackground>* class_backgrounds = nullptr;

    void rect(float x, float y, float w, float h, SkColor col) {
        if (w <= 0 || h <= 0 || SkColorGetA(col) == 0) return;
        rects++;
        SkPaint p;
        p.setColor(col);
        p.setAntiAlias(true);
        chromium_fill_rect(c, SkRect::MakeXYWH(x, y, w, h), p);
    }

    struct Snap;

    void rrect_fill(const SkRRect& rr, SkColor col) {
        if (SkColorGetA(col) == 0) return;
        rects++;
        SkPaint p;
        p.setColor(col);
        p.setAntiAlias(true);
        c.drawRRect(rr, p);
    }

    static bool angle_arg(const std::string& a, float* deg) {
        char* e = nullptr;
        float v = std::strtof(a.c_str(), &e);
        if (e == a.c_str()) return false;
        std::string u = e;
        if (u == "deg") *deg = v;
        else if (u == "rad") *deg = v * 180.f / 3.14159265f;
        else if (u == "grad") *deg = v * 0.9f;
        else if (u == "turn") *deg = v * 360.f;
        else return false;
        return true;
    }

    static bool color_stops(const std::vector<std::string>& args, size_t first, float len, std::vector<SkColor4f>& cols, std::vector<float>& pos) {
        std::vector<bool> has;
        for (size_t i = first; i < args.size(); i++) {
            std::string a2 = args[i];
            std::string col = a2, rest;
            size_t close = a2.find(')');
            if (a2.rfind("rgb", 0) == 0 && close != std::string::npos) {
                col = a2.substr(0, close + 1);
                rest = a2.substr(close + 1);
            } else {
                size_t sp = a2.find(' ');
                col = a2.substr(0, sp);
                rest = sp == std::string::npos ? "" : a2.substr(sp);
            }
            Rgba rc = col == "transparent" ? Rgba{0, 0, 0, 0} : parse_color(col);
            SkColor4f c4{rc.r / 255.f, rc.g / 255.f, rc.b / 255.f, rc.a};
            std::vector<float> ps;
            const char* p = rest.c_str();
            while (*p) {
                while (*p == ' ') p++;
                if (!*p) break;
                char* en = nullptr;
                float v = std::strtof(p, &en);
                if (en == p) break;
                ps.push_back(*en == '%' ? v / 100.f : v / len);
                p = en;
                while (*p && *p != ' ') p++;
            }
            if (ps.empty()) {
                cols.push_back(c4);
                pos.push_back(0.f);
                has.push_back(false);
            }
            for (float v : ps) {
                cols.push_back(c4);
                pos.push_back(v);
                has.push_back(true);
            }
        }
        size_t cnt = cols.size();
        if (cnt < 2) return false;
        if (!has[0]) pos[0] = 0.f, has[0] = true;
        if (!has[cnt - 1]) pos[cnt - 1] = 1.f, has[cnt - 1] = true;
        float prev = pos[0];
        for (size_t i = 1; i < cnt; i++)
            if (has[i]) {
                pos[i] = std::max(pos[i], prev);
                prev = pos[i];
            }
        for (size_t i = 1; i < cnt; i++) {
            if (has[i]) continue;
            size_t j = i;
            while (!has[j]) j++;
            for (size_t k = i; k < j; k++) pos[k] = pos[i - 1] + (pos[j] - pos[i - 1]) * (float)(k - i + 1) / (float)(j - i + 1);
            i = j;
        }
        return true;
    }

    void radial_gradient(const Node& n, const Snap& s, const Radii& rad, const std::string& layer) {
        size_t o = layer.find('(');
        size_t e = layer.rfind(')');
        if (o == std::string::npos || e == std::string::npos || e <= o) return;
        std::vector<std::string> args = split_top(layer.substr(o + 1, e - o - 1));
        if (args.size() < 2) return;
        float bl = px_of(n.get("border-left-width")), bt = px_of(n.get("border-top-width"));
        float gx = s.x + bl, gy = s.y + bt, gw = s.w - bl - px_of(n.get("border-right-width")), gh = s.h - bt - px_of(n.get("border-bottom-width"));
        if (gw <= 0 || gh <= 0) return;
        size_t first = 0;
        bool circle = false;
        std::string extent = "farthest-corner";
        float ex[2] = {0, 0};
        int nex = 0;
        float cx = gw * 0.5f, cy = gh * 0.5f;
        const std::string& a0 = args[0];
        bool is_color = a0.rfind("rgb", 0) == 0 || a0.rfind("#", 0) == 0 || a0.rfind("transparent", 0) == 0 || a0.rfind("hsl", 0) == 0;
        if (!is_color) {
            first = 1;
            std::vector<std::string> t = tokens(a0);
            size_t at = t.size();
            for (size_t i = 0; i < t.size(); i++)
                if (t[i] == "at") at = i;
            for (size_t i = 0; i < at; i++) {
                if (t[i] == "circle") circle = true;
                else if (t[i] == "ellipse") circle = false;
                else if (std::isalpha((unsigned char)t[i][0])) extent = t[i];
                else if (nex < 2) {
                    float v = std::strtof(t[i].c_str(), nullptr);
                    ex[nex] = t[i].back() == '%' ? v * (nex == 0 ? gw : gh) / 100.f : v;
                    nex++;
                    extent.clear();
                }
            }
            if (nex == 1) circle = true;
            if (at < t.size()) {
                std::string pos;
                for (size_t i = at + 1; i < t.size(); i++) pos += (pos.empty() ? "" : " ") + t[i];
                css_position(pos, gw, gh, cx, cy);
            }
        }
        float l = cx, r = gw - cx, t = cy, b = gh - cy;
        float rx = 0, ry = 0;
        if (extent.empty()) {
            rx = ex[0];
            ry = nex > 1 ? ex[1] : ex[0];
        } else {
            bool closest = extent.rfind("closest", 0) == 0;
            float sx = closest ? std::min(std::fabs(l), std::fabs(r)) : std::max(std::fabs(l), std::fabs(r));
            float sy = closest ? std::min(std::fabs(t), std::fabs(b)) : std::max(std::fabs(t), std::fabs(b));
            bool corner = extent.find("corner") != std::string::npos;
            if (circle) rx = ry = corner ? std::hypot(sx, sy) : (closest ? std::min(sx, sy) : std::max(sx, sy));
            else if (corner) {
                rx = sx * 1.41421356f;
                ry = sy * 1.41421356f;
            } else {
                rx = sx;
                ry = sy;
            }
        }
        if (rx <= 0.f || ry <= 0.f) return;
        std::vector<SkColor4f> cols;
        std::vector<float> pos;
        if (!color_stops(args, first, rx, cols, pos)) return;
        SkGradient::Interpolation interp;
        interp.fInPremul = SkGradient::Interpolation::InPremul::kYes;
        SkGradient grad(SkGradient::Colors(cols, pos, SkTileMode::kClamp, SkColorSpace::MakeSRGB()), interp);
        SkMatrix lm = SkMatrix::Scale(1.f, ry / rx);
        lm.postTranslate(gx + cx, gy + cy);
        SkPaint p;
        p.setAntiAlias(true);
        p.setShader(SkShaders::RadialGradient({0, 0}, rx, grad, &lm));
        rects++;
        if (rad.any) c.drawRRect(make_rrect(s.x, s.y, s.w, s.h, rad, 0, 0, 0, 0), p);
        else c.drawRect(SkRect::MakeXYWH(s.x, s.y, s.w, s.h), p);
    }

    void background_layers(const Node& n, const Snap& s, const Radii& rad) {
        std::string bi = n.get("background-image");
        std::vector<std::string> layers = bi.empty() || bi == "none" ? std::vector<std::string>{} : split_top(bi);
        if (layers.empty()) {
            background_image(n, s);
            return;
        }
        for (size_t i = layers.size(); i-- > 0;) {
            const std::string& l = layers[i];
            if (l.rfind("linear-gradient(", 0) == 0) gradient_image(n, s, rad, l);
            else if (l.rfind("radial-gradient(", 0) == 0) radial_gradient(n, s, rad, l);
            else if (l.rfind("url(", 0) == 0) background_image(n, s, &l);
        }
    }

    void gradient_image(const Node& n, const Snap& s, const Radii& rad, const std::string& bi) {
        size_t a = bi.find("linear-gradient(");
        if (a == std::string::npos) return;
        size_t o = a + 16, e = o;
        for (int depth = 1; e < bi.size(); e++) {
            if (bi[e] == '(') depth++;
            if (bi[e] == ')' && --depth == 0) break;
        }
        std::vector<std::string> args = split_top(bi.substr(o, e - o));
        if (args.size() < 2) return;
        float bl = px_of(n.get("border-left-width")), bt = px_of(n.get("border-top-width"));
        float br = px_of(n.get("border-right-width")), bb = px_of(n.get("border-bottom-width"));
        float gx = s.x + bl, gy = s.y + bt, gw = s.w - bl - br, gh = s.h - bt - bb;
        if (gw <= 0 || gh <= 0) return;
        float dx = 0.f, dy = 1.f;
        size_t first = 0;
        float deg = 180.f;
        if (angle_arg(args[0], &deg)) {
            first = 1;
            float t = deg * 3.14159265f / 180.f;
            dx = std::sin(t);
            dy = -std::cos(t);
        } else if (args[0].rfind("to ", 0) == 0) {
            first = 1;
            bool top = args[0].find("top") != std::string::npos, bot = args[0].find("bottom") != std::string::npos;
            bool lef = args[0].find("left") != std::string::npos, rig = args[0].find("right") != std::string::npos;
            if ((top || bot) && (lef || rig)) {
                dx = (rig ? 1.f : -1.f) * gh;
                dy = (bot ? 1.f : -1.f) * gw;
                float m = std::hypot(dx, dy);
                dx /= m;
                dy /= m;
            } else if (top) dx = 0.f, dy = -1.f;
            else if (rig) dx = 1.f, dy = 0.f;
            else if (lef) dx = -1.f, dy = 0.f;
            else dx = 0.f, dy = 1.f;
        }
        float len = std::fabs(gw * dx) + std::fabs(gh * dy);
        if (len <= 0.f) return;
        std::vector<SkColor4f> cols;
        std::vector<float> pos;
        if (!color_stops(args, first, len, cols, pos)) return;
        float cx = gx + gw * 0.5f, cy = gy + gh * 0.5f;
        SkPoint pts[2] = {{cx - dx * len * 0.5f, cy - dy * len * 0.5f}, {cx + dx * len * 0.5f, cy + dy * len * 0.5f}};
        SkGradient::Interpolation interp;
        interp.fInPremul = SkGradient::Interpolation::InPremul::kYes;
        SkGradient grad(SkGradient::Colors(cols, pos, SkTileMode::kClamp, SkColorSpace::MakeSRGB()), interp);
        SkPaint p;
        p.setAntiAlias(true);
        p.setShader(SkShaders::LinearGradient(pts, grad));
        rects++;
        if (rad.any) c.drawRRect(make_rrect(s.x, s.y, s.w, s.h, rad, 0, 0, 0, 0), p);
        else c.drawRect(SkRect::MakeXYWH(s.x, s.y, s.w, s.h), p);
    }

    static SkColor border_side_color(Rgba col, const std::string& style, int side) {
        if (style != "inset" && style != "outset") return to_sk(col);
        bool darken = ((side == 0 || side == 3) == (style == "inset"));
        float r = col.r, g = col.g, b = col.b;
        float v = std::max({r, g, b}) / 255.f;
        if (darken) {
            float dr = r, dg = g, db = b;
            float dist = dr * dr + dg * dg + db * db, base = 3.f * 32.f * 32.f;
            if (dist > base && v > 0.f) {
                float m = std::max(0.f, (v - 0.33f) / v);
                col.r = std::round(r * m), col.g = std::round(g * m), col.b = std::round(b * m);
            }
        } else {
            float dist = (255 - r) * (255 - r) + (255 - g) * (255 - g) + (255 - b) * (255 - b), base = 3.f * 20.f * 20.f;
            if (dist > base) {
                if (v <= 0.f) col.r = col.g = col.b = 0x54;
                else {
                    float m = std::min(1.f, v + 0.33f) / v;
                    col.r = std::min(255.f, std::round(r * m)), col.g = std::min(255.f, std::round(g * m)), col.b = std::min(255.f, std::round(b * m));
                }
            }
        }
        return to_sk(col);
    }

    struct Snap {
        float x, y, w, h;
    };

    Snap snapped(const Node& n) const {
        float x0 = std::round(n.box.x * dpr), y0 = std::round(n.box.y * dpr);
        float x1 = std::round((n.box.x + n.box.w) * dpr), y1 = std::round((n.box.y + n.box.h) * dpr);
        return {x0 / dpr, y0 / dpr, (x1 - x0) / dpr, (y1 - y0) / dpr};
    }

    void background_image(const Node& n, const Snap& s, const std::string* layer = nullptr) {
        std::string url = layer ? *layer : n.get("background-image"), size = n.get("background-size");
        std::string position = effects ? n.get("background-position") : std::string();
        if ((url.empty() || url == "none" || (effects && (size.empty() || position.empty()))) && class_backgrounds) {
            bool no_url = url.empty() || url == "none";
            for (auto& cb : *class_backgrounds) {
                for (auto& k : n.cls)
                    if (k == cb.cls) {
                        if (no_url) url = cb.image;
                        if (no_url || size.empty()) size = cb.size;
                        if (position.empty()) position = cb.position;
                    }
            }
        }
        if (url.empty() || url == "none" || url.find("gradient(") != std::string::npos) return;
        if (url.find("data:") != std::string::npos && !effects) return;
        std::string rel = image_relative(url);
        sk_sp<SkImage> img = rel.rfind("data:", 0) == 0 ? load_data_url(rel) : load_image(rel);
        if (!img) return;
        float bl = px_of(n.get("border-left-width")), bt = px_of(n.get("border-top-width"));
        float bx = s.x + bl, by = s.y + bt;
        float bw = s.w - bl - px_of(n.get("border-right-width")), bh = s.h - bt - px_of(n.get("border-bottom-width"));
        float iw = (float)img->width() / dpr, ih = (float)img->height() / dpr;
        if (effects && rel.rfind("data:", 0) == 0) iw = (float)img->width(), ih = (float)img->height();
        float w = iw, h = ih;
        if (size == "contain" || size == "cover") {
            float k = size == "contain" ? std::min(bw / iw, bh / ih) : std::max(bw / iw, bh / ih);
            w = iw * k;
            h = ih * k;
        }
        if (w <= 0 || h <= 0) return;
        float ox = 0.f, oy = 0.f;
        if (!position.empty()) css_position(position, bw - w, bh - h, ox, oy);
        SkPaint p;
        p.setAntiAlias(true);
        c.save();
        c.clipRect(SkRect::MakeXYWH(bx, by, bw, bh));
        c.drawImageRect(img, SkRect::MakeXYWH(bx + ox, by + oy, w, h), SkSamplingOptions(SkFilterMode::kLinear, SkMipmapMode::kLinear), &p);
        c.restore();
    }

    float shadow_sigma(float blur) const { return blur * 0.5f; }

    void outer_shadows(const Node& n, const Snap& s, const Radii& rad) {
        std::vector<Shadow> list = parse_shadows(n.get("box-shadow"));
        if (list.empty()) return;
        SkRRect border = rad.any ? make_rrect(s.x, s.y, s.w, s.h, rad, 0, 0, 0, 0) : SkRRect::MakeRect(SkRect::MakeXYWH(s.x, s.y, s.w, s.h));
        for (size_t i = list.size(); i-- > 0;) {
            const Shadow& sh = list[i];
            if (sh.inset) continue;
            SkRRect r = border;
            if (sh.spread != 0.f) r.outset(sh.spread, sh.spread);
            if (r.isEmpty()) continue;
            r.offset(sh.x, sh.y);
            SkPaint p;
            p.setAntiAlias(true);
            p.setColor(sh.col);
            if (sh.blur > 0.f) p.setMaskFilter(SkMaskFilter::MakeBlur(kNormal_SkBlurStyle, shadow_sigma(sh.blur)));
            c.save();
            c.clipRRect(border, SkClipOp::kDifference, true);
            c.drawRRect(r, p);
            c.restore();
        }
    }

    void inset_shadows(const Node& n, const Snap& s, const Radii& rad) {
        std::vector<Shadow> list = parse_shadows(n.get("box-shadow"));
        if (list.empty()) return;
        auto bwid = [&](const char* k) { return device_border(px_of(n.get(k)), dpr); };
        float bl = bwid("border-left-width"), bt = bwid("border-top-width"), br = bwid("border-right-width"), bb = bwid("border-bottom-width");
        SkRRect inner = make_rrect(s.x, s.y, s.w, s.h, rad, bl, bt, br, bb);
        if (inner.isEmpty()) return;
        for (size_t i = list.size(); i-- > 0;) {
            const Shadow& sh = list[i];
            if (!sh.inset) continue;
            SkRRect hole = inner;
            hole.inset(sh.spread, sh.spread);
            hole.offset(sh.x, sh.y);
            SkRRect outer = inner;
            float grow = sh.blur * 2.f + std::fabs(sh.x) + std::fabs(sh.y) + std::fabs(sh.spread) + 2.f;
            outer.outset(grow, grow);
            SkPaint p;
            p.setAntiAlias(true);
            p.setColor(sh.col);
            if (sh.blur > 0.f) p.setMaskFilter(SkMaskFilter::MakeBlur(kNormal_SkBlurStyle, shadow_sigma(sh.blur)));
            c.save();
            c.clipRRect(inner, true);
            if (hole.isEmpty()) c.drawRRect(outer, p);
            else c.drawDRRect(outer, hole, p);
            c.restore();
        }
    }

    void borders(const Node& n, const Snap& s, const Radii& rad) {
        const char* sides[4] = {"top", "right", "bottom", "left"};
        float bw[4], dev[4];
        int visible = 0;
        for (int i = 0; i < 4; i++) {
            bw[i] = px_of(n.get(std::string("border-") + sides[i] + "-width"));
            dev[i] = device_border(bw[i], dpr);
            if (dev[i] > 0) visible++;
        }
        if (!visible) return;
        std::string top = n.get("border-top-style");
        std::string st[4];
        for (int i = 0; i < 4; i++) {
            st[i] = n.get(std::string("border-") + sides[i] + "-style");
            if (st[i].empty()) st[i] = top;
            if (st[i] == "none" || st[i].empty()) st[i] = visible > 1 && dev[i] * dpr >= 1.5f ? "outset" : "solid";
            if (st[i] == "hidden") dev[i] = 0;
        }
        float x = s.x, y = s.y, w = s.w, h = s.h;
        SkColor col[4];
        for (int i = 0; i < 4; i++) col[i] = border_side_color(parse_color(n.get(std::string("border-") + sides[i] + "-color")), st[i], i);
        float ix = x + dev[3], iy = y + dev[0], ir = x + w - dev[1], ib = y + h - dev[2], r = x + w, bt = y + h;
        auto same = [&](int a, int b) { return col[a] == col[b] && dev[a] > 0 && dev[b] > 0; };
        bool mTL = same(0, 3), mTR = same(0, 1), mBR = same(2, 1), mBL = same(2, 3);
        SkPoint q[4][4] = {
            {{x, y}, {r, y}, {mTR ? r : ir, iy}, {mTL ? x : ix, iy}},
            {{r, mTR ? iy : y}, {ir, iy}, {ir, ib}, {r, mBR ? ib : bt}},
            {{r, bt}, {x, bt}, {mBL ? x : ix, ib}, {mBR ? r : ir, ib}},
            {{x, mTL ? iy : y}, {ix, iy}, {ix, ib}, {x, mBL ? ib : bt}},
        };
        if (rad.any) {
            SkRRect outer = make_rrect(x, y, w, h, rad, 0, 0, 0, 0);
            SkRRect inner = make_rrect(x, y, w, h, rad, dev[3], dev[0], dev[1], dev[2]);
            bool uniform = true;
            for (int i = 1; i < 4; i++)
                if (col[i] != col[0] || dev[i] <= 0) uniform = false;
            for (int i = 0; i < 4; i++) {
                if (dev[i] <= 0 || SkColorGetA(col[i]) == 0) continue;
                SkPaint p;
                p.setAntiAlias(true);
                p.setColor(col[i]);
                int saved = c.save();
                if (!uniform) c.clipPath(SkPathBuilder().addPolygon(q[i], true).detach(), true);
                c.drawDRRect(outer, inner, p);
                c.restoreToCount(saved);
                if (uniform) break;
            }
            return;
        }
        for (int i = 0; i < 4; i++) {
            if (dev[i] <= 0 || SkColorGetA(col[i]) == 0) continue;
            SkPaint p;
            p.setAntiAlias(true);
            p.setColor(col[i]);
            chromium_fill_polygon(c, q[i], 4, p);
        }
    }

    static bool lcd_layer(const Node& n) {
        for (const Node* p = &n; p; p = p->parent) {
            const std::string ov = p->get("overflow");
            if ((ov == "auto" || ov == "scroll") && parse_color(p->get("background-color")).a >= 1.f) return true;
        }
        return false;
    }

    static float spaced_width(const SkFont& f, const std::string& t, float lsp) {
        size_t glyphs = 0;
        for (unsigned char ch : t)
            if ((ch & 0xC0) != 0x80) glyphs++;
        return text_width(f, t) + lsp * (float)glyphs;
    }

    static bool underlined(const Node& n) {
        for (const Node* p = &n; p; p = p->parent) {
            if (p->get("text-decoration-line").find("underline") != std::string::npos) return true;
            const std::string d = p->get("display");
            if (!d.empty() && d != "inline") break;
        }
        return false;
    }

    void ellipsis(const Node& n, float x, const SkFont& f, float lsp, std::string& t) {
        const Node* blk = &n;
        while (blk->parent && blk->get("display").rfind("inline", 0) == 0 && blk->get("display") != "inline-block") blk = blk->parent;
        if (blk->get("text-overflow") != "ellipsis") return;
        const std::string ov = blk->get("overflow");
        if (ov.empty() || ov == "visible") return;
        float right = blk->box.x + blk->box.w - px_of(blk->get("border-right-width")) - px_of(blk->get("padding-right"));
        float avail = right - x;
        if (spaced_width(f, t, lsp) <= avail + 0.01f) return;
        const std::string dots = "\xE2\x80\xA6";
        while (!t.empty()) {
            size_t cut = t.size() - 1;
            while (cut > 0 && ((unsigned char)t[cut] & 0xC0) == 0x80) cut--;
            t.resize(cut);
            if (spaced_width(f, t + dots, lsp) <= avail + 0.01f) break;
        }
        t += dots;
    }

    void text(const Node& n) {
        if (n.get("visibility") == "hidden") return;
        for (auto& l : n.lines) {
            texts++;
            std::string family = n.get("font-family");
            const Node* src = &n;
            for (const Node* p = &n; p && family.empty(); p = p->parent) family = p->get("font-family");
            std::string color;
            for (const Node* p = &n; p && color.empty(); p = p->parent) color = p->get("color");
            std::string fs, fw, ls;
            for (const Node* p = &n; p; p = p->parent) {
                if (fs.empty()) fs = p->get("font-size");
                if (fw.empty()) fw = p->get("font-weight");
                if (ls.empty()) ls = p->get("letter-spacing");
            }
            (void)src;
            family.erase(std::remove(family.begin(), family.end(), '"'), family.end());
            float size = px_of(fs);
            int weight = fw.empty() || fw == "normal" ? 400 : fw == "bold" ? 700 : std::atoi(fw.c_str());
            bool synthetic_bold = false;
            weight = layout::declared_font_weight(family, weight, &synthetic_bold);
            SkFont f = a380::font(family, size, weight, lcd_all || lcd_layer(n) ? TextMode::Lcd : TextMode::Greyscale);
            if (synthetic_bold) f.setEmbolden(true);
            SkPaint p;
            p.setAntiAlias(true);
            p.setColor(to_sk(parse_color(color)));
            float lsp = (ls.empty() || ls == "normal") ? 0.f : px_of(ls);
            std::string t = l.text;
            for (size_t k = t.find("\xC2\xA0"); k != std::string::npos; k = t.find("\xC2\xA0", k + 1)) t.replace(k, 2, " ");
            if (a380::getenv_cached("MFDX_DEBUG") && texts < 4) std::fprintf(stderr, "text '%s' x %.2f y %.2f size %.1f col %s fam %s w %d tw %.2f argb %08x\n", t.c_str(), l.box.x, l.baseline, size, color.c_str(), family.c_str(), weight, text_width(f, t), (unsigned)p.getColor());
            float by = std::floor(l.baseline * dpr + 0.5f) / dpr;
            if (effects) ellipsis(n, l.box.x, f, lsp, t);
            draw_text(c, t, l.box.x, by, f, p, Align::Left, Baseline::Alphabetic, lsp);
            if (effects && underlined(n)) {
                SkFontMetrics fm;
                f.getMetrics(&fm);
                float pos = 0.f, thick = 0.f;
                if (!fm.hasUnderlinePosition(&pos)) pos = size * 0.1f;
                if (!fm.hasUnderlineThickness(&thick)) thick = size / 14.f;
                thick = std::max(1.f / dpr, std::round(thick * dpr) / dpr);
                float uy = std::round((by + pos) * dpr) / dpr;
                c.drawRect(SkRect::MakeXYWH(l.box.x + l.trim_x, uy, l.trim_w, thick), p);
            }
        }
    }

    int nodes = 0, texts = 0, rects = 0;

    static bool positioned(const Node& n) { return n.get("position") != "static" && !n.get("position").empty(); }
    static bool hidden(const Node& n) {
        if (n.get("display") == "none") return true;
        const std::string op = n.get("opacity");
        return !op.empty() && std::strtof(op.c_str(), nullptr) == 0.f;
    }
    bool clip_scrollers = false;

    bool clips(const Node& n) const {
        const std::string ov = n.get("overflow");
        return ov == "hidden" || ov == "clip" || (clip_scrollers && (ov == "auto" || ov == "scroll"));
    }

    void clip_to(const Node& n) {
        auto bwid = [&](const char* k) { return device_border(px_of(n.get(k)), dpr); };
        Snap s = snapped(n);
        float bl = bwid("border-left-width"), bt = bwid("border-top-width"), br = bwid("border-right-width"), bb = bwid("border-bottom-width");
        Radii rad = read_radii(n, s.w, s.h, effects);
        if (rad.any) c.clipRRect(make_rrect(s.x, s.y, s.w, s.h, rad, bl, bt, br, bb), true);
        else c.clipRect(SkRect::MakeXYWH(s.x + bl, s.y + bt, s.w - bl - br, s.h - bt - bb));
    }

    static std::string attr_of(const Node& n, const char* k) {
        auto it = n.svgattr.find(k);
        return it == n.svgattr.end() ? std::string() : it->second;
    }

    void range_input(const Node& n) {
        auto num = [&](const char* k, float d) {
            std::string v = attr_of(n, k);
            return v.empty() ? d : std::strtof(v.c_str(), nullptr);
        };
        float lo = num("min", 0.f), hi = num("max", 100.f), v = num("value", (lo + hi) * 0.5f);
        float frac = hi > lo ? std::clamp((v - lo) / (hi - lo), 0.f, 1.f) : 0.f;
        const SkColor accent = SkColorSetRGB(0x00, 0x75, 0xFF), track = SkColorSetRGB(0xEF, 0xEF, 0xEF);
        const float track_h = 8.f, thumb_r = 8.f;
        float cy = n.box.y + n.box.h * 0.5f;
        float tx = n.box.x + thumb_r + (n.box.w - 2.f * thumb_r) * frac;
        SkRRect t = SkRRect::MakeRectXY(SkRect::MakeXYWH(n.box.x, cy - track_h * 0.5f, n.box.w, track_h), track_h * 0.5f, track_h * 0.5f);
        SkPaint p;
        p.setAntiAlias(true);
        p.setColor(track);
        c.drawRRect(t, p);
        c.save();
        c.clipRect(SkRect::MakeLTRB(n.box.x, cy - track_h, tx, cy + track_h), true);
        p.setColor(accent);
        c.drawRRect(t, p);
        c.restore();
        c.drawCircle(tx, cy, thumb_r, p);
    }

    Snap snap_box(float x, float y, float w, float h) const {
        float x0 = std::round(x * dpr), y0 = std::round(y * dpr), x1 = std::round((x + w) * dpr), y1 = std::round((y + h) * dpr);
        return {x0 / dpr, y0 / dpr, (x1 - x0) / dpr, (y1 - y0) / dpr};
    }

    static void gather_lines(const Node& n, std::vector<const layout::TextLine*>& out) {
        for (auto& l : n.lines) out.push_back(&l);
        for (auto& k : n.kids)
            if (!k->is_text && k->tag != "svg" && k->get("display") == "inline") gather_lines(*k, out);
    }

    void inline_box(const Node& n) {
        Rgba bg = parse_color(n.get("background-color"));
        const char* sides[4] = {"top", "right", "bottom", "left"};
        float edge[4], pad[4];
        bool bordered = false;
        for (int i = 0; i < 4; i++) {
            edge[i] = device_border(px_of(n.get(std::string("border-") + sides[i] + "-width")), dpr);
            pad[i] = px_of(n.get(std::string("padding-") + sides[i]));
            bordered = bordered || edge[i] > 0.f;
        }
        if (bg.a <= 0.f && !bordered) return;
        std::vector<const layout::TextLine*> ls;
        gather_lines(n, ls);
        struct Row {
            float x0, x1, y0, y1, base;
        };
        std::vector<Row> rows;
        for (const layout::TextLine* l : ls) {
            Row* hit = nullptr;
            for (Row& r : rows)
                if (std::fabs(r.base - l->baseline) < 0.5f) hit = &r;
            if (!hit) {
                rows.push_back({l->box.x, l->box.x + l->box.w, l->box.y, l->box.y + l->box.h, l->baseline});
                continue;
            }
            hit->x0 = std::min(hit->x0, l->box.x);
            hit->x1 = std::max(hit->x1, l->box.x + l->box.w);
            hit->y0 = std::min(hit->y0, l->box.y);
            hit->y1 = std::max(hit->y1, l->box.y + l->box.h);
        }
        if (rows.empty()) return;
        std::sort(rows.begin(), rows.end(), [](const Row& a, const Row& b) { return a.base < b.base; });
        rows.front().x0 -= pad[3] + edge[3];
        rows.back().x1 += pad[1] + edge[1];
        for (Row& r : rows) {
            r.y0 -= pad[0] + edge[0];
            r.y1 += pad[2] + edge[2];
            Snap s = snap_box(r.x0, r.y0, r.x1 - r.x0, r.y1 - r.y0);
            Radii rad = read_radii(n, s.w, s.h);
            if (bg.a > 0.f) {
                if (rad.any) rrect_fill(make_rrect(s.x, s.y, s.w, s.h, rad, 0, 0, 0, 0), to_sk(bg));
                else rect(s.x, s.y, s.w, s.h, to_sk(bg));
            }
            if (rows.size() == 1) borders(n, s, rad);
        }
    }

    void radio_ring(const Node& n, const Snap& s, const Radii& rad) {
        if (n.tag != "input" || attr_of(n, "type") != "radio" || attr_of(n, "checked").empty()) return;
        auto bwid = [&](const char* k) { return device_border(px_of(n.get(k)), dpr); };
        SkRRect inner = make_rrect(s.x, s.y, s.w, s.h, rad, bwid("border-left-width"), bwid("border-top-width"), bwid("border-right-width"),
                                   bwid("border-bottom-width"));
        if (inner.isEmpty()) return;
        SkRRect hole = inner;
        hole.inset(4.f, 4.f);
        SkPaint p;
        p.setAntiAlias(true);
        p.setColor(SK_ColorBLACK);
        if (hole.isEmpty()) c.drawRRect(inner, p);
        else c.drawDRRect(inner, hole, p);
    }

    void webkit_scrollbar(const Node& n) {
        if (!fidelity || sb_width <= 0.f || sb_cls.empty() || !n.has_class(sb_cls)) return;
        const std::string ov = n.get("overflow");
        if (ov != "auto" && ov != "scroll") return;
        float bl = px_of(n.get("border-left-width")), bt = px_of(n.get("border-top-width"));
        float px0 = n.box.x + bl, py0 = n.box.y + bt;
        float cw = n.box.w - bl - px_of(n.get("border-right-width")), ch = n.box.h - bt - px_of(n.get("border-bottom-width"));
        float sy = 0.f;
        if (scrolled)
            for (auto& t : *scrolled)
                if (std::get<0>(t) == &n) sy = std::get<2>(t);
        float right = px0, bottom = py0;
        content_extent(n, n, right, bottom);
        float content = bottom - py0 + sy + px_of(n.get("padding-bottom"));
        if (ch <= 0.f || content <= ch + 0.5f) return;
        float tx = px0 + cw - sb_width;
        float th = std::max(sb_width, ch * ch / content);
        float ty = py0 + (ch - th) * std::clamp(sy / (content - ch), 0.f, 1.f);
        rect(tx, py0, sb_width, ch, sb_track);
        rect(tx, ty, sb_width, th, sb_thumb);
    }

    void decorate(const Node& n) {
        if (n.get("visibility") == "hidden") return;
        if (n.get("display") == "inline") {
            if (fidelity) inline_box(n);
            return;
        }
        if (effects && n.tag == "input" && attr_of(n, "type") == "range") {
            range_input(n);
            return;
        }
        Rgba bg = parse_color(n.get("background-color"));
        Snap s = snapped(n);
        Radii rad = read_radii(n, s.w, s.h, effects);
        if (effects) outer_shadows(n, s, rad);
        if (bg.a > 0) {
            if (rad.any) rrect_fill(make_rrect(s.x, s.y, s.w, s.h, rad, 0, 0, 0, 0), to_sk(bg));
            else rect(s.x, s.y, s.w, s.h, to_sk(bg));
        }
        if (effects) {
            background_layers(n, s, rad);
            inset_shadows(n, s, rad);
        } else {
            gradient_image(n, s, rad, n.get("background-image"));
            background_image(n, s);
        }
        if (fidelity) radio_ring(n, s, rad);
        borders(n, s, rad);
    }

    void boxes(const Node& n, bool is_root, bool self = true) {
        if (hidden(n)) return;
        if (!is_root && lifted(n)) return;
        nodes++;
        if (self) decorate(n);
        int saved = c.save();
        if (clips(n)) clip_to(n);
        for (auto& k : n.kids)
            if (!k->is_text) boxes(*k, false);
        if (n.tag == "svg" && !(fidelity && n.get("visibility") == "hidden")) {
            std::string color;
            for (const Node* p = &n; p && color.empty(); p = p->parent) color = p->get("color");
            paint_svg(c, n, to_sk(parse_color(color)), effects || fidelity);
        }
        webkit_scrollbar(n);
        c.restoreToCount(saved);
    }

    void input_text(const Node& n) {
        auto v = n.svgattr.find("value");
        if (v == n.svgattr.end() || v->second.empty() || n.get("visibility") == "hidden") return;
        if (effects) {
            const std::string type = attr_of(n, "type");
            if (type == "range" || type == "checkbox" || type == "radio" || type == "hidden" || type == "color") return;
        }
        std::string family = n.get("font-family"), color = n.get("color");
        family.erase(std::remove(family.begin(), family.end(), '"'), family.end());
        std::string fw = n.get("font-weight");
        int weight = fw.empty() || fw == "normal" ? 400 : fw == "bold" ? 700 : std::atoi(fw.c_str());
        bool synthetic_bold = false;
        weight = layout::declared_font_weight(family, weight, &synthetic_bold);
        SkFont f = a380::font(family, px_of(n.get("font-size")), weight, lcd_all || lcd_layer(n) ? TextMode::Lcd : TextMode::Greyscale);
        if (synthetic_bold) f.setEmbolden(true);
        SkPaint p;
        p.setAntiAlias(true);
        p.setColor(to_sk(parse_color(color)));
        float left = px_of(n.get("border-left-width")) + px_of(n.get("padding-left"));
        float top = px_of(n.get("border-top-width")) + px_of(n.get("padding-top"));
        float inner_h = n.box.h - top - px_of(n.get("border-bottom-width")) - px_of(n.get("padding-bottom"));
        float by = chromium_baseline(f, n.box.y + top, inner_h, dpr);
        draw_text(c, v->second, n.box.x + left, by, f, p);
    }

    void texts_of(const Node& n, bool is_root) {
        if (hidden(n)) return;
        if (!is_root && lifted(n)) return;
        int saved = c.save();
        if (clips(n)) clip_to(n);
        text(n);
        if (input_values && n.tag == "input") input_text(n);
        for (auto& k : n.kids)
            if (!k->is_text) texts_of(*k, false);
        c.restoreToCount(saved);
    }

    void collect(const Node& n, std::vector<const Node*>& out, bool is_root) {
        if (hidden(n)) return;
        if (!is_root && lifted(n)) {
            out.push_back(&n);
            if (!stacking || makes_context(n)) return;
        }
        for (auto& k : n.kids)
            if (!k->is_text) collect(*k, out, false);
    }

    bool stacking = false;
    bool effects = false;

    bool layered(const Node& n) const {
        if (!effects && !fidelity) return false;
        const std::string t = n.get("transform");
        if (!t.empty() && t != "none") return true;
        const std::string op = n.get("opacity");
        if (!op.empty() && std::strtof(op.c_str(), nullptr) < 1.f) return true;
        const std::string f = n.get("filter");
        if (!f.empty() && f != "none") return true;
        const std::string bf = n.get("backdrop-filter");
        return !bf.empty() && bf != "none";
    }

    bool lifted(const Node& n) const { return positioned(n) || layered(n); }

    void clip_ancestors(const Node& n) {
        bool absolute = n.get("position") == "absolute";
        if ((effects || fidelity) && n.get("position") == "fixed") return;
        bool inside = fidelity && absolute;
        for (const Node* a = n.parent; a && a->parent; a = a->parent) {
            if (inside && (positioned(*a) || layered(*a))) inside = false;
            if (!inside && clips(*a)) clip_to(*a);
            if (!fidelity && absolute && positioned(*a)) break;
            if (layered(*a)) break;
        }
    }
    bool input_values = false;
    bool fidelity = false;
    const std::vector<std::tuple<Node*, float, float>>* scrolled = nullptr;
    std::string sb_cls;
    SkColor sb_track = 0, sb_thumb = 0;
    float sb_width = 0.f;

    bool makes_context(const Node& n) const {
        std::string z = n.get("z-index");
        return (positioned(n) && !z.empty() && z != "auto") || layered(n);
    }

    static int zindex(const Node& n) {
        std::string z = n.get("z-index");
        return z.empty() || z == "auto" || !positioned(n) ? 0 : std::atoi(z.c_str());
    }

    static sk_sp<SkColorFilter> filter_of(const std::string& f, sk_sp<SkImageFilter>* blur) {
        sk_sp<SkColorFilter> out;
        if (f.empty() || f == "none") return out;
        size_t p = 0;
        while (p < f.size()) {
            while (p < f.size() && f[p] == ' ') p++;
            size_t lp = f.find('(', p), rp = f.find(')', lp == std::string::npos ? p : lp);
            if (lp == std::string::npos || rp == std::string::npos) break;
            std::string name = f.substr(p, lp - p);
            std::string arg = f.substr(lp + 1, rp - lp - 1);
            char* e = nullptr;
            float v = std::strtof(arg.c_str(), &e);
            if (e != arg.c_str() && *e == '%') v /= 100.f;
            if (e == arg.c_str()) v = name == "blur" ? 0.f : 1.f;
            sk_sp<SkColorFilter> cf;
            if (name == "brightness") {
                SkColorMatrix m;
                m.setScale(v, v, v, 1.f);
                cf = SkColorFilters::Matrix(m);
            } else if (name == "saturate") {
                SkColorMatrix m;
                m.setSaturation(v);
                cf = SkColorFilters::Matrix(m);
            } else if (name == "opacity") {
                SkColorMatrix m;
                m.setScale(1.f, 1.f, 1.f, v);
                cf = SkColorFilters::Matrix(m);
            } else if (name == "blur" && blur && v > 0.f) {
                *blur = SkImageFilters::Blur(v, v, SkTileMode::kClamp, *blur);
            }
            if (cf) {
                if (blur && *blur) *blur = SkImageFilters::ColorFilter(cf, *blur);
                out = out ? cf->makeComposed(out) : cf;
            }
            p = rp + 1;
        }
        return out;
    }

    void begin_layer(const Node& n) {
        if (!layered(n)) return;
        Snap s = snapped(n);
        const std::string t = n.get("transform");
        if (!t.empty() && t != "none") {
            float ox = n.box.x + n.box.w * 0.5f, oy = n.box.y + n.box.h * 0.5f;
            std::string origin = n.get("transform-origin");
            if (!origin.empty()) {
                css_position(origin, n.box.w, n.box.h, ox, oy);
                ox += n.box.x;
                oy += n.box.y;
            }
            SkMatrix m = css_transform(t, n.box.w, n.box.h);
            m.preTranslate(-ox, -oy);
            m.postTranslate(ox, oy);
            c.concat(m);
        }
        const std::string bf = n.get("backdrop-filter");
        if (!bf.empty() && bf != "none") {
            sk_sp<SkImageFilter> chain;
            sk_sp<SkColorFilter> cf = filter_of(bf, &chain);
            if (!chain && cf) chain = SkImageFilters::ColorFilter(cf, nullptr);
            if (chain) {
                Radii rad = read_radii(n, s.w, s.h, effects);
                c.save();
                if (rad.any) c.clipRRect(make_rrect(s.x, s.y, s.w, s.h, rad, 0, 0, 0, 0), true);
                else c.clipRect(SkRect::MakeXYWH(s.x, s.y, s.w, s.h), true);
                c.saveLayer(SkCanvas::SaveLayerRec(nullptr, nullptr, chain.get(), 0));
                c.restore();
                c.restore();
            }
        }
        const std::string op = n.get("opacity");
        float alpha = op.empty() ? 1.f : std::clamp(std::strtof(op.c_str(), nullptr), 0.f, 1.f);
        sk_sp<SkColorFilter> cf = filter_of(n.get("filter"), nullptr);
        if (alpha < 1.f || cf) {
            SkPaint lp;
            lp.setAlphaf(alpha);
            lp.setColorFilter(cf);
            c.saveLayer(nullptr, &lp);
        }
    }

    void paint(const Node& n, bool context = true) {
        if (!effects) {
            boxes(n, true);
            texts_of(n, true);
        }
        if (!context) {
            if (effects) {
                boxes(n, true);
                texts_of(n, true);
            }
            return;
        }
        std::vector<const Node*> pos;
        collect(n, pos, true);
        std::stable_sort(pos.begin(), pos.end(), [](const Node* a, const Node* b) { return zindex(*a) < zindex(*b); });
        auto one = [&](const Node* p) {
            int saved = c.save();
            if (stacking) clip_ancestors(*p);
            begin_layer(*p);
            paint(*p, !stacking || makes_context(*p));
            c.restoreToCount(saved);
        };
        size_t i = 0;
        if (effects) {
            decorate(n);
            for (; i < pos.size() && zindex(*pos[i]) < 0; i++) one(pos[i]);
            boxes(n, true, false);
            texts_of(n, true);
        }
        for (; i < pos.size(); i++) one(pos[i]);
    }
};

void dump(const layout::Tree& t, const std::string& path) {
    std::string out = "[";
    bool first = true;
    layout::walk(*t.root, [](const Node& n, void* u) {
        auto& pr = *(std::pair<std::string*, bool*>*)u;
        if ((n.lines.empty() && !a380::getenv_cached("MFDX_ALL")) || n.get("visibility") == "hidden") return;
        std::string& o = *pr.first;
        if (!*pr.second) o += ",";
        *pr.second = false;
        o += "{\"tag\":\"" + n.tag + "\",\"cls\":\"";
        std::string cls;
        escape(cls, n.class_string());
        o += cls + "\",\"box\":[" + std::to_string(n.box.x) + "," + std::to_string(n.box.y) + "," + std::to_string(n.box.w) + "," +
             std::to_string(n.box.h) + "],\"lines\":[";
        for (size_t i = 0; i < n.lines.size(); i++) {
            auto& l = n.lines[i];
            char buf[256];
            std::snprintf(buf, sizeof buf, "%s[%.6f,%.6f,%.6f,%.6f,%.6f]", i ? "," : "", l.box.x + l.trim_x, l.box.y, l.box.w - l.trim_x, l.box.h,
                          l.baseline);
            o += buf;
        }
        o += "],\"text\":\"";
        std::string tx;
        for (auto& l : n.lines) tx += l.text;
        { std::string e2; escape(e2, tx); tx = e2; }
        o += tx + "\"}";
    }, new std::pair<std::string*, bool*>(&out, &first));
    out += "]";
    std::ofstream f(path, std::ios::binary);
    f << out;
}

bool read_file(const std::string& path, std::string& out) {
    std::ifstream f(path, std::ios::binary);
    if (!f) return false;
    out.assign(std::istreambuf_iterator<char>(f), std::istreambuf_iterator<char>());
    return true;
}

std::string sibling_dom_path(const DomConfig& cfg) {
    if (const char* e = a380::getenv_cached(cfg.env.c_str())) return e;
    std::vector<std::string> args;
#ifdef _WIN32
    for (int i = 1; i < __argc; i++) args.push_back(__argv[i]);
#else
    std::ifstream cl("/proc/self/cmdline", std::ios::binary);
    std::string a;
    while (std::getline(cl, a, '\0')) args.push_back(a);
#endif
    for (auto& a : args) {
        const std::string suffix = ".state.json";
        if (a.size() > suffix.size() && a.compare(a.size() - suffix.size(), suffix.size(), suffix) == 0) {
            size_t cut = a.find_last_of("/\\");
            return (cut == std::string::npos ? std::string() : a.substr(0, cut + 1)) + cfg.sibling;
        }
    }
    return {};
}

void transform_case(std::string& t, const std::string& mode) {
    bool start = true;
    for (size_t i = 0; i < t.size(); i++) {
        unsigned char ch = (unsigned char)t[i];
        bool upper = mode == "uppercase" || (mode == "capitalize" && start);
        bool lower = mode == "lowercase";
        if (ch < 0x80) {
            if (upper) t[i] = (char)std::toupper(ch);
            else if (lower) t[i] = (char)std::tolower(ch);
            start = !std::isalnum(ch) && ch != '\'';
        } else if (ch == 0xC3 && i + 1 < t.size()) {
            unsigned char nx = (unsigned char)t[i + 1];
            if (upper && nx >= 0xA0 && nx <= 0xBE && nx != 0xB7) t[i + 1] = (char)(nx - 0x20);
            else if (lower && nx >= 0x80 && nx <= 0x9E && nx != 0x97) t[i + 1] = (char)(nx + 0x20);
            start = false;
            i++;
        } else if ((ch & 0xC0) == 0xC0) {
            start = false;
        }
    }
}

void apply_text_transform(Node& n) {
    for (auto& k : n.kids) {
        if (k->is_text) {
            std::string mode;
            for (const Node* p = &n; p && mode.empty(); p = p->parent) mode = p->get("text-transform");
            if (!mode.empty() && mode != "none") transform_case(k->text, mode);
        } else {
            apply_text_transform(*k);
        }
    }
}

bool is_positioned(const Node& n) {
    const std::string p = n.get("position");
    return !p.empty() && p != "static";
}

bool escapes_scroller(const Node& k, const Node& scroller) {
    const std::string pos = k.get("position");
    if (pos == "fixed") return true;
    if (pos != "absolute") return false;
    for (const Node* a = k.parent; a; a = a->parent) {
        if (is_positioned(*a)) return false;
        if (a == &scroller) return true;
    }
    return true;
}

void shift_lines(Node& n, float dx, float dy) {
    n.box.x += dx;
    n.box.y += dy;
    for (auto& l : n.lines) {
        l.box.x += dx;
        l.box.y += dy;
        l.baseline += dy;
    }
}

void shift_content(Node& n, const Node& scroller, float dx, float dy) {
    for (auto& k : n.kids) {
        if (k->is_text || escapes_scroller(*k, scroller)) continue;
        shift_lines(*k, dx, dy);
        shift_content(*k, scroller, dx, dy);
    }
}

void content_extent(const Node& n, const Node& scroller, float& right, float& bottom) {
    for (auto& k : n.kids) {
        if (k->is_text || escapes_scroller(*k, scroller) || k->get("display") == "none") continue;
        right = std::max(right, k->box.x + k->box.w + px_of(k->get("margin-right")));
        bottom = std::max(bottom, k->box.y + k->box.h + px_of(k->get("margin-bottom")));
        for (auto& l : k->lines) {
            right = std::max(right, l.box.x + l.box.w);
            bottom = std::max(bottom, l.box.y + l.box.h);
        }
        if (k->get("overflow") == "visible" || k->get("overflow").empty()) content_extent(*k, scroller, right, bottom);
    }
}

void collect_scrollers(Node& n, const json* by_id, std::vector<std::tuple<Node*, float, float>>& out) {
    float st = 0.f, sl = 0.f;
    bool any = false;
    auto a = n.svgattr.find("data-st");
    if (a != n.svgattr.end()) st = std::strtof(a->second.c_str(), nullptr), any = true;
    a = n.svgattr.find("data-sl");
    if (a != n.svgattr.end()) sl = std::strtof(a->second.c_str(), nullptr), any = true;
    if (by_id && !n.id.empty() && by_id->contains(n.id)) {
        const json& v = (*by_id)[n.id];
        if (v.is_number()) st = v.get<float>(), any = true;
        else if (v.is_object()) {
            if (v.contains("top") && v["top"].is_number()) st = v["top"].get<float>(), any = true;
            if (v.contains("left") && v["left"].is_number()) sl = v["left"].get<float>(), any = true;
        }
    }
    if (any) out.emplace_back(&n, sl, st);
    for (auto& k : n.kids)
        if (!k->is_text) collect_scrollers(*k, by_id, out);
}

void collect_boxes(const Node& n, std::vector<float>& out) {
    if (!n.anonymous && n.tag.rfind("::", 0) != 0) out.insert(out.end(), {n.box.x, n.box.y, n.box.w, n.box.h});
    for (auto& k : n.kids)
        if (!k->is_text) collect_boxes(*k, out);
}

}  // namespace

DomDisplay::DomDisplay(DomConfig cfg) : cfg_(std::move(cfg)) { prof_ = a380::SectionProfile(cfg_.topic.empty() ? "dom" : cfg_.topic); }

SkISize DomDisplay::size() const { return SkISize::Make(cfg_.width, cfg_.height); }

bool DomDisplay::ensure_tree(const Inputs& in, float dpr) {
    const json* model = cfg_.model_topic.empty() ? nullptr : in.bus(cfg_.model_topic);
    std::string dom;
    bool laid_out = false;
    if (!model || !model->is_object() || !model->contains("tag")) {
        model = nullptr;
        const json* bus_dom = in.bus(cfg_.topic);
        if (bus_dom && bus_dom->is_string()) dom = bus_dom->get<std::string>();
        else {
            std::string path = sibling_dom_path(cfg_);
            if (path.empty() || !read_file(path, dom)) return false;
        }
        if (dom.empty()) return false;
    }
    if (model) {
        tree_ = layout::load_tree(*model, dpr);
        tree_key_ = 0;
    } else {
        size_t key = std::hash<std::string>{}(dom) ^ ((size_t)std::lround(dpr * 1000.f) << 1);
        if (!tree_.root || key != tree_key_) {
            static const bool via_json = a380::getenv_cached("MFDX_VIA_JSON") != nullptr;
            if (via_json) {
                json parsed = dom_to_tree(dom, a380::getenv_cached("MFDX_LOWERCASE") == nullptr);
                if (!parsed.is_object() || !parsed.contains("tag")) return false;
                tree_ = layout::load_tree(parsed, dpr);
            } else {
                layout::Tree parsed;
                prof_.time("parse", [&] { parsed = dom_to_layout(dom, a380::getenv_cached("MFDX_LOWERCASE") == nullptr, dpr); });
                if (!parsed.root) return false;
                tree_ = std::move(parsed);
            }
            tree_key_ = key;
        } else {
            laid_out = true;
        }
    }
    layout::Tree& t = tree_;
    if (!laid_out) {
        if (const char* tp = a380::getenv_cached("MFDX_TABLE")) {
            std::ifstream tf(tp, std::ios::binary);
            json table = json::parse(tf, nullptr, false);
            if (!table.is_discarded()) layout::apply_style_table(t, table);
        }
        if (cfg_.css_effects) apply_text_transform(*t.root);
        prof_.time("run_layout", [&] { layout::run_layout(t, (float)cfg_.width, (float)cfg_.height); });
        boxes_.clear();
        collect_boxes(*t.root, boxes_);
        scrolled_.clear();
        scroll_key_ = 0;
        if (cfg_.css_effects || cfg_.css_fidelity) apply_scroll(in);
        if (const char* d = a380::getenv_cached("MFDX_DUMP")) dump(t, d);
    } else if (cfg_.css_effects || cfg_.css_fidelity) {
        apply_scroll(in);
    }
    return true;
}

void DomDisplay::apply_scroll(const Inputs& in) {
    if (!tree_.root) return;
    const json* by_id = cfg_.scroll_topic.empty() ? nullptr : in.bus(cfg_.scroll_topic);
    if (by_id && !by_id->is_object()) by_id = nullptr;
    std::vector<std::tuple<Node*, float, float>> want;
    collect_scrollers(*tree_.root, by_id, want);
    size_t key = 1469598103934665603ull;
    for (auto& [n, sl, st] : want) key = (key ^ std::hash<const void*>{}(n) ^ std::hash<float>{}(sl) * 31 ^ std::hash<float>{}(st) * 131) * 1099511628211ull;
    if (key == scroll_key_) return;
    for (auto it = scrolled_.rbegin(); it != scrolled_.rend(); ++it) shift_content(*std::get<0>(*it), *std::get<0>(*it), std::get<1>(*it), std::get<2>(*it));
    scrolled_.clear();
    for (auto& [n, sl, st] : want) {
        float bl = px_of(n->get("border-left-width")), bt = px_of(n->get("border-top-width"));
        float br = px_of(n->get("border-right-width")), bb = px_of(n->get("border-bottom-width"));
        float px0 = n->box.x + bl, py0 = n->box.y + bt;
        float cw = n->box.w - bl - br, chh = n->box.h - bt - bb;
        float right = px0, bottom = py0;
        content_extent(*n, *n, right, bottom);
        float sw = right - px0 + px_of(n->get("padding-right")), sh = bottom - py0 + px_of(n->get("padding-bottom"));
        float x = std::clamp(sl, 0.f, std::max(0.f, sw - cw)), y = std::clamp(st, 0.f, std::max(0.f, sh - chh));
        if (x == 0.f && y == 0.f) continue;
        shift_content(*n, *n, -x, -y);
        scrolled_.emplace_back(n, x, y);
    }
    scroll_key_ = key;
}

void DomDisplay::render(SkCanvas& c, float dpr, int mode, bool have_tree) {
    if (cfg_.black_backdrop) {
        SkPaint bp;
        bp.setColor(SK_ColorBLACK);
        c.drawRect(SkRect::MakeWH((float)cfg_.width, (float)cfg_.height), bp);
    }
    if (mode != 0) {
        overlay(c, mode);
        return;
    }
    if (!have_tree) return;
    if (a380::getenv_cached("MFDX_TEST")) { SkPaint tp; tp.setColor(SK_ColorWHITE); c.drawRect(SkRect::MakeXYWH(100, 100, 200, 50), tp); SkFont tf = a380::font("Ecam", 24.f); draw_text(c, "TEST", 120, 200, tf, tp); }
    Painter p{c, dpr};
    p.lcd_all = cfg_.lcd_text;
    p.stacking = cfg_.css_stacking;
    p.input_values = cfg_.css_stacking;
    p.clip_scrollers = cfg_.css_stacking;
    p.class_backgrounds = &cfg_.class_backgrounds;
    p.effects = cfg_.css_effects;
    p.fidelity = cfg_.css_fidelity;
    p.scrolled = &scrolled_;
    p.sb_cls = cfg_.scrollbar_cls;
    p.sb_track = cfg_.scrollbar_track;
    p.sb_thumb = cfg_.scrollbar_thumb;
    p.sb_width = cfg_.scrollbar_width;
    p.paint(*tree_.root);
    if (cfg_.scrollbars) {
        for (auto& [n, x, y] : scrolled_) {
            float right = n->box.x, bottom = n->box.y;
            content_extent(*n, *n, right, bottom);
            float content = bottom + y - n->box.y, view = n->box.h;
            if (content <= view || view <= 0) continue;
            float bar = std::max(24.f, view * view / content), top = n->box.y + (view - bar) * (y / (content - view));
            SkPaint sp;
            sp.setAntiAlias(true);
            sp.setColor(SkColorSetARGB(0x55, 0xFF, 0xFF, 0xFF));
            c.drawRRect(SkRRect::MakeRectXY(SkRect::MakeXYWH(n->box.x + n->box.w - 6.f, top, 4.f, bar), 2.f, 2.f), sp);
        }
    }
    if (a380::getenv_cached("MFDX_DEBUG")) std::fprintf(stderr, "mfdx nodes %d texts %d rects %d\n", p.nodes, p.texts, p.rects);
}

void DomDisplay::draw(SkCanvas& c, const Inputs& in) {
    float dpr = (float)in.device_pixel_ratio();
    int mode = state(in);
    static const bool no_cache = a380::getenv_cached("MFDX_NOCACHE") != nullptr;
    if (no_cache) tree_key_ = 0, gpu_cached_ = nullptr;
    bool have = false;
    prof_.time("tree+layout", [&] { have = mode == 0 && ensure_tree(in, dpr); });
    size_t key = (have ? tree_key_ : 0) ^ ((size_t)mode << 60) ^ (have ? 1u : 0u) ^ (scroll_key_ * 0x9E3779B97F4A7C15ull);
    bool reusable = tree_key_ != 0 || !have;
    if (!cfg_.lcd_text) {
        if (!gpu_cached_ || !reusable || key != gpu_key_ || dpr != gpu_dpr_) {
            int w = (int)std::lround(cfg_.width * dpr), h = (int)std::lround(cfg_.height * dpr);
            sk_sp<SkSurface> surface = c.makeSurface(SkImageInfo::MakeN32Premul(w, h));
            if (!surface) {
                render(c, dpr, mode, have);
                return;
            }
            surface->getCanvas()->clear(SK_ColorTRANSPARENT);
            surface->getCanvas()->scale(dpr, dpr);
            prof_.time("paint", [&] { render(*surface->getCanvas(), dpr, mode, have); });
            gpu_cached_ = surface->makeImageSnapshot();
            gpu_key_ = key;
            gpu_dpr_ = dpr;
        }
        c.save();
        c.resetMatrix();
        c.drawImage(gpu_cached_, 0, 0);
        c.restore();
        prof_.frame();
        return;
    }
    if (!cached_ || !reusable || key != cached_key_ || dpr != cached_dpr_) {
        int w = (int)std::lround(cfg_.width * dpr), h = (int)std::lround(cfg_.height * dpr);
        SkSurfaceProps props = a380::text_surface_props();
        sk_sp<SkSurface> surface = SkSurfaces::Raster(SkImageInfo::MakeN32Premul(w, h), &props);
        if (!surface) {
            render(c, dpr, mode, have);
            return;
        }
        surface->getCanvas()->scale(dpr, dpr);
        render(*surface->getCanvas(), dpr, mode, have);
        cached_ = surface->makeImageSnapshot();
        cached_key_ = key;
        cached_dpr_ = dpr;
    }
    c.save();
    c.resetMatrix();
    SkPaint bp;
    bp.setBlendMode(SkBlendMode::kSrc);
    c.drawImage(cached_, 0, 0, SkSamplingOptions(), &bp);
    c.restore();
}

}  // namespace a380::mfdx
