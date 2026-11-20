#include "svg.h"

#include <algorithm>
#include <cctype>
#include <cmath>
#include <cstdlib>
#include <cstring>
#include <vector>

#include "a380/raster.h"
#include "include/core/SkMatrix.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPath.h"
#include "include/core/SkPathBuilder.h"
#include "include/effects/SkDashPathEffect.h"
#include "a380/draw.h"
#include "a380/fonts.h"

namespace a380::mfdx {

namespace {

using Attr = std::unordered_map<std::string, std::string>;

std::string trim(std::string s) {
    while (!s.empty() && std::isspace((unsigned char)s.front())) s.erase(s.begin());
    while (!s.empty() && std::isspace((unsigned char)s.back())) s.pop_back();
    return s;
}

std::string style_prop(const std::string& style, const std::string& key) {
    size_t p = 0;
    while (p < style.size()) {
        size_t e = style.find(';', p);
        if (e == std::string::npos) e = style.size();
        size_t c = style.find(':', p);
        if (c != std::string::npos && c < e && trim(style.substr(p, c - p)) == key) return trim(style.substr(c + 1, e - c - 1));
        p = e + 1;
    }
    return "";
}

std::string prop(const Attr& a, const char* key) {
    auto cs = a.find(std::string("_c_") + key);
    if (cs != a.end()) return trim(cs->second);
    auto st = a.find("style");
    if (st != a.end()) {
        std::string v = style_prop(st->second, key);
        if (!v.empty()) return v;
    }
    auto it = a.find(key);
    return it == a.end() ? std::string() : trim(it->second);
}

std::string attr(const Attr& a, const char* key) {
    auto it = a.find(key);
    return it == a.end() ? std::string() : it->second;
}

struct Named {
    const char* name;
    uint32_t rgb;
};

const Named kNamed[] = {{"white", 0xffffff}, {"black", 0x000000}, {"grey", 0x808080}, {"gray", 0x808080}, {"lightgrey", 0xd3d3d3},
                        {"lightgray", 0xd3d3d3}, {"darkgrey", 0xa9a9a9}, {"darkgray", 0xa9a9a9}, {"red", 0xff0000}, {"green", 0x008000},
                        {"lime", 0x00ff00}, {"blue", 0x0000ff}, {"yellow", 0xffff00}, {"cyan", 0x00ffff}, {"aqua", 0x00ffff},
                        {"magenta", 0xff00ff}, {"fuchsia", 0xff00ff}, {"orange", 0xffa500}, {"silver", 0xc0c0c0}, {"dimgrey", 0x696969},
                        {"dimgray", 0x696969}, {"purple", 0x800080}, {"brown", 0xa52a2a}, {"pink", 0xffc0cb}};

}  // namespace

bool parse_css_color(const std::string& in, SkColor current, SkColor& out) {
    std::string v = trim(in);
    if (v.empty() || v == "none" || v == "transparent") return false;
    if (v == "currentColor" || v == "currentcolor") {
        out = current;
        return true;
    }
    if (v[0] == '#') {
        std::string h = v.substr(1);
        if (h.size() == 3) h = std::string{h[0], h[0], h[1], h[1], h[2], h[2]};
        if (h.size() != 6) return false;
        out = 0xFF000000u | (uint32_t)std::strtoul(h.c_str(), nullptr, 16);
        return true;
    }
    if (v.rfind("rgb", 0) == 0) {
        const char* p = std::strchr(v.c_str(), '(');
        if (!p) return false;
        float c[4] = {0, 0, 0, 1};
        int n = 0;
        p++;
        while (*p && n < 4) {
            char* e = nullptr;
            float x = std::strtof(p, &e);
            if (e == p) break;
            c[n++] = x;
            p = e;
            while (*p == ',' || *p == ' ' || *p == '/') p++;
        }
        auto cl = [](float x) { return (uint8_t)std::lround(std::clamp(x, 0.f, 255.f)); };
        out = SkColorSetARGB(cl(c[3] * 255.f), cl(c[0]), cl(c[1]), cl(c[2]));
        return true;
    }
    std::string l = v;
    for (auto& ch : l) ch = (char)std::tolower((unsigned char)ch);
    for (auto& nm : kNamed)
        if (l == nm.name) {
            out = 0xFF000000u | nm.rgb;
            return true;
        }
    return false;
}

namespace {

struct State {
    std::string fill = "black", stroke = "none";
    float stroke_width = 1.f, fill_opacity = 1.f, stroke_opacity = 1.f, opacity = 1.f;
    SkPaint::Cap cap = SkPaint::kButt_Cap;
    SkPaint::Join join = SkPaint::kMiter_Join;
    SkColor color = SK_ColorBLACK;
    bool hidden = false;
    std::vector<float> dash;
    float dash_offset = 0.f;
    std::string anchor;
    SkMatrix m;
};

float num(const std::string& s, float dflt = 0.f) {
    if (s.empty()) return dflt;
    return std::strtof(s.c_str(), nullptr);
}

std::vector<float> numbers(const std::string& s) {
    std::vector<float> v;
    const char* p = s.c_str();
    while (*p) {
        while (*p == ',' || std::isspace((unsigned char)*p)) p++;
        if (!*p) break;
        char* e = nullptr;
        float x = std::strtof(p, &e);
        if (e == p) break;
        v.push_back(x);
        p = e;
        while (std::isalpha((unsigned char)*p) || *p == '%') p++;
    }
    return v;
}

SkMatrix parse_transform(const std::string& s) {
    SkMatrix m;
    size_t p = 0;
    while (p < s.size()) {
        while (p < s.size() && (std::isspace((unsigned char)s[p]) || s[p] == ',')) p++;
        size_t lp = s.find('(', p);
        if (lp == std::string::npos) break;
        size_t rp = s.find(')', lp);
        if (rp == std::string::npos) break;
        std::string name = trim(s.substr(p, lp - p));
        std::vector<float> a = numbers(s.substr(lp + 1, rp - lp - 1));
        SkMatrix t;
        if (name == "translate") t = SkMatrix::Translate(a.size() > 0 ? a[0] : 0.f, a.size() > 1 ? a[1] : 0.f);
        else if (name == "scale") t = SkMatrix::Scale(a.size() > 0 ? a[0] : 1.f, a.size() > 1 ? a[1] : (a.size() > 0 ? a[0] : 1.f));
        else if (name == "rotate") {
            float deg = a.size() > 0 ? a[0] : 0.f;
            t = a.size() >= 3 ? SkMatrix::RotateDeg(deg, {a[1], a[2]}) : SkMatrix::RotateDeg(deg);
        } else if (name == "matrix" && a.size() >= 6) t = SkMatrix::MakeAll(a[0], a[2], a[4], a[1], a[3], a[5], 0, 0, 1);
        else if (name == "skewX" && !a.empty()) t = SkMatrix::Skew(std::tan(a[0] * 3.14159265f / 180.f), 0.f);
        else if (name == "skewY" && !a.empty()) t = SkMatrix::Skew(0.f, std::tan(a[0] * 3.14159265f / 180.f));
        m = SkMatrix::Concat(m, t);
        p = rp + 1;
    }
    return m;
}

SkPath parse_path(const std::string& d) {
    SkPathBuilder b;
    const char* p = d.c_str();
    char cmd = 0;
    float cx = 0, cy = 0, sx = 0, sy = 0, lcx = 0, lcy = 0;
    bool have_move = false;
    auto skip = [&]() {
        while (*p == ',' || std::isspace((unsigned char)*p)) p++;
    };
    auto read = [&](float& out) {
        skip();
        char* e = nullptr;
        float x = std::strtof(p, &e);
        if (e == p) return false;
        out = x;
        p = e;
        return true;
    };
    auto flag = [&](bool& out) {
        skip();
        if (*p != '0' && *p != '1') return false;
        out = *p == '1';
        p++;
        return true;
    };
    for (;;) {
        skip();
        if (!*p) break;
        if (std::isalpha((unsigned char)*p)) cmd = *p++;
        else if (!cmd) break;
        bool rel = std::islower((unsigned char)cmd);
        char C = (char)std::toupper((unsigned char)cmd);
        float a[7];
        if (C == 'Z') {
            b.close();
            cx = sx;
            cy = sy;
            have_move = false;
            continue;
        }
        if (C == 'M' || C == 'L' || C == 'T') {
            if (!read(a[0]) || !read(a[1])) break;
            float x = rel ? cx + a[0] : a[0], y = rel ? cy + a[1] : a[1];
            if (C == 'M') {
                b.moveTo(x, y);
                sx = x;
                sy = y;
                cmd = rel ? 'l' : 'L';
            } else b.lineTo(x, y);
            cx = x;
            cy = y;
        } else if (C == 'H') {
            if (!read(a[0])) break;
            cx = rel ? cx + a[0] : a[0];
            b.lineTo(cx, cy);
        } else if (C == 'V') {
            if (!read(a[0])) break;
            cy = rel ? cy + a[0] : a[0];
            b.lineTo(cx, cy);
        } else if (C == 'C') {
            bool ok = true;
            for (int i = 0; i < 6; i++) ok = ok && read(a[i]);
            if (!ok) break;
            float ox = rel ? cx : 0.f, oy = rel ? cy : 0.f;
            b.cubicTo(a[0] + ox, a[1] + oy, a[2] + ox, a[3] + oy, a[4] + ox, a[5] + oy);
            lcx = a[2] + ox;
            lcy = a[3] + oy;
            cx = a[4] + ox;
            cy = a[5] + oy;
        } else if (C == 'S') {
            bool ok = true;
            for (int i = 0; i < 4; i++) ok = ok && read(a[i]);
            if (!ok) break;
            float ox = rel ? cx : 0.f, oy = rel ? cy : 0.f;
            b.cubicTo(2 * cx - lcx, 2 * cy - lcy, a[0] + ox, a[1] + oy, a[2] + ox, a[3] + oy);
            lcx = a[0] + ox;
            lcy = a[1] + oy;
            cx = a[2] + ox;
            cy = a[3] + oy;
        } else if (C == 'Q') {
            bool ok = true;
            for (int i = 0; i < 4; i++) ok = ok && read(a[i]);
            if (!ok) break;
            float ox = rel ? cx : 0.f, oy = rel ? cy : 0.f;
            b.quadTo(a[0] + ox, a[1] + oy, a[2] + ox, a[3] + oy);
            cx = a[2] + ox;
            cy = a[3] + oy;
        } else if (C == 'A') {
            bool la = false, sw = false;
            if (!read(a[0]) || !read(a[1]) || !read(a[2]) || !flag(la) || !flag(sw) || !read(a[5]) || !read(a[6])) break;
            float x = rel ? cx + a[5] : a[5], y = rel ? cy + a[6] : a[6];
            b.arcTo({a[0], a[1]}, a[2], la ? SkPathBuilder::kLarge_ArcSize : SkPathBuilder::kSmall_ArcSize,
                    sw ? SkPathDirection::kCW : SkPathDirection::kCCW, {x, y});
            cx = x;
            cy = y;
        } else break;
    }
    (void)have_move;
    return b.detach();
}

float uniform_scale(const SkMatrix& m) { return std::sqrt(std::fabs(m.getScaleX() * m.getScaleY() - m.getSkewX() * m.getSkewY())); }

bool axis_aligned(const SkMatrix& m) { return m.getSkewX() == 0.f && m.getSkewY() == 0.f && !m.hasPerspective(); }

struct Ctx {
    SkCanvas& c;
    float vw, vh;
};

SkPaint make_paint(SkColor col, float alpha) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(SkColorSetA(col, (uint8_t)std::lround(std::clamp(SkColorGetA(col) / 255.f * alpha, 0.f, 1.f) * 255.f)));
    return p;
}

void fill_path(Ctx& cx, const State& s, const SkPath& page) {
    SkColor col;
    if (!parse_css_color(s.fill, s.color, col)) return;
    chromium_fill_path(cx.c, page, make_paint(col, s.fill_opacity * s.opacity));
}

void stroke_path(Ctx& cx, const State& s, const SkPath& page) {
    SkColor col;
    if (!parse_css_color(s.stroke, s.color, col)) return;
    SkPaint p = make_paint(col, s.stroke_opacity * s.opacity);
    p.setStyle(SkPaint::kStroke_Style);
    p.setStrokeWidth(s.stroke_width * uniform_scale(s.m));
    p.setStrokeCap(s.cap);
    p.setStrokeJoin(s.join);
    p.setStrokeMiter(4.f);
    if (s.dash.size() >= 2) {
        float k = uniform_scale(s.m);
        std::vector<float> d = s.dash;
        if (d.size() % 2) d.insert(d.end(), s.dash.begin(), s.dash.end());
        for (float& v : d) v *= k;
        p.setPathEffect(SkDashPathEffect::Make(d, s.dash_offset * k));
        cx.c.drawPath(page, p);
        return;
    }
    chromium_stroke_path(cx.c, page, p);
}

float length(const std::string& v, float ref) {
    if (v.empty()) return 0.f;
    float x = num(v);
    return v.back() == '%' ? x * ref / 100.f : x;
}

}  // namespace

void paint_svg(SkCanvas& c, const layout::Node& svg, SkColor current_color, bool extended) {
    if (svg.shapes.empty()) return;
    float bx = svg.box.x, by = svg.box.y, bw = svg.box.w, bh = svg.box.h;
    SkMatrix base = SkMatrix::Translate(bx, by);
    float vw = bw, vh = bh;
    auto vb = svg.svgattr.find("viewBox");
    if (vb != svg.svgattr.end()) {
        std::vector<float> v = numbers(vb->second);
        if (v.size() == 4 && v[2] > 0 && v[3] > 0) {
            vw = v[2];
            vh = v[3];
            float sx = bw / v[2], sy = bh / v[3];
            std::string par = attr(svg.svgattr, "preserveAspectRatio");
            if (par.rfind("none", 0) == 0) base = SkMatrix::Concat(base, SkMatrix::MakeAll(sx, 0, -v[0] * sx, 0, sy, -v[1] * sy, 0, 0, 1));
            else {
                float s = std::min(sx, sy);
                float tx = (bw - v[2] * s) * 0.5f - v[0] * s, ty = (bh - v[3] * s) * 0.5f - v[1] * s;
                if (par.find("xMin") != std::string::npos) tx = -v[0] * s;
                if (par.find("xMax") != std::string::npos) tx = bw - v[2] * s - v[0] * s;
                if (par.find("YMin") != std::string::npos) ty = -v[1] * s;
                if (par.find("YMax") != std::string::npos) ty = bh - v[3] * s - v[1] * s;
                base = SkMatrix::Concat(base, SkMatrix::MakeAll(s, 0, tx, 0, s, ty, 0, 0, 1));
            }
        }
    }
    Ctx cx{c, vw, vh};

    auto inherit = [&](State s, const Attr& a, bool is_root) {
        std::string v;
        if (!(v = prop(a, "fill")).empty()) s.fill = v;
        if (!(v = prop(a, "stroke")).empty()) s.stroke = v;
        if (!(v = prop(a, "stroke-width")).empty()) s.stroke_width = num(v, 1.f);
        if (!(v = prop(a, "fill-opacity")).empty()) s.fill_opacity = num(v, 1.f);
        if (!(v = prop(a, "stroke-opacity")).empty()) s.stroke_opacity = num(v, 1.f);
        if (!(v = prop(a, "opacity")).empty()) s.opacity *= num(v, 1.f);
        if (!(v = prop(a, "stroke-linecap")).empty()) s.cap = v == "round" ? SkPaint::kRound_Cap : v == "square" ? SkPaint::kSquare_Cap : SkPaint::kButt_Cap;
        if (!(v = prop(a, "stroke-linejoin")).empty()) s.join = v == "round" ? SkPaint::kRound_Join : v == "bevel" ? SkPaint::kBevel_Join : SkPaint::kMiter_Join;
        if (!(v = prop(a, "color")).empty()) {
            SkColor col;
            if (parse_css_color(v, s.color, col)) s.color = col;
        }
        if (prop(a, "display") == "none") s.hidden = true;
        auto d = a.find("_display");
        if (d != a.end() && d->second == "none") s.hidden = true;
        if (extended) {
            if (!(v = prop(a, "stroke-dasharray")).empty() && v != "none") s.dash = numbers(v);
            if (!(v = prop(a, "stroke-dashoffset")).empty()) s.dash_offset = num(v);
            if (!(v = prop(a, "text-anchor")).empty()) s.anchor = v;
        }
        if (!is_root) {
            auto t = a.find("_c_transform");
            if (t == a.end()) t = a.find("transform");
            if (t != a.end()) s.m = SkMatrix::Concat(s.m, parse_transform(t->second));
        }
        return s;
    };

    State root;
    root.color = current_color;
    root.m = base;
    root = inherit(root, svg.svgattr, true);

    c.save();
    c.clipRect(SkRect::MakeXYWH(bx, by, bw, bh));
    std::vector<State> stack{root};
    for (auto& sh : svg.shapes) {
        int d = std::atoi(attr(sh.attr, "_d").c_str());
        if (d > (int)stack.size() - 1) d = (int)stack.size() - 1;
        stack.resize(d + 1);
        State s = inherit(stack.back(), sh.attr, false);
        stack.push_back(s);
        const std::string& t = sh.type;
        if (extended && (t == "defs" || t == "clipPath" || t == "mask" || t == "symbol" || t == "marker" || t == "pattern" ||
                         t == "linearGradient" || t == "radialGradient" || t == "title")) {
            stack.back().hidden = true;
            continue;
        }
        if (s.hidden) continue;
        if (extended && (t == "text" || t == "tspan")) {
            std::string txt = attr(sh.attr, "_text");
            if (txt.empty()) continue;
            SkColor col;
            if (!parse_css_color(s.fill, s.color, col)) continue;
            std::string family = attr(sh.attr, "_font-family");
            if (family.empty()) family = prop(sh.attr, "font-family");
            float size = num(attr(sh.attr, "_font-size"), 0.f);
            if (size <= 0.f) size = num(prop(sh.attr, "font-size"), 16.f);
            std::string fw = attr(sh.attr, "_font-weight");
            if (fw.empty()) fw = prop(sh.attr, "font-weight");
            int weight = fw.empty() || fw == "normal" ? 400 : fw == "bold" ? 700 : std::atoi(fw.c_str());
            family.erase(std::remove(family.begin(), family.end(), '"'), family.end());
            SkFont f = a380::font(family.empty() ? std::string("sans-serif") : family, size, weight, TextMode::Greyscale);
            SkPaint p = make_paint(col, s.fill_opacity * s.opacity);
            std::vector<float> xs = numbers(attr(sh.attr, "x")), ys = numbers(attr(sh.attr, "y"));
            float x = xs.empty() ? 0.f : xs[0], y = ys.empty() ? 0.f : ys[0];
            x += num(attr(sh.attr, "dx"));
            y += num(attr(sh.attr, "dy"));
            Align al = s.anchor == "middle" ? Align::Center : s.anchor == "end" ? Align::Right : Align::Left;
            c.save();
            c.concat(s.m);
            draw_text(c, txt, x, y, f, p, al, Baseline::Alphabetic);
            c.restore();
            continue;
        }
        if (extended && t == "ellipse") {
            float rx = length(attr(sh.attr, "rx"), vw), ry = length(attr(sh.attr, "ry"), vh);
            if (rx <= 0 || ry <= 0) continue;
            float ecx = length(attr(sh.attr, "cx"), vw), ecy = length(attr(sh.attr, "cy"), vh);
            SkPath pg = SkPath::Oval(SkRect::MakeLTRB(ecx - rx, ecy - ry, ecx + rx, ecy + ry)).makeTransform(s.m);
            fill_path(cx, s, pg);
            stroke_path(cx, s, pg);
            continue;
        }
        if (t == "polygon" || t == "polyline") {
            std::vector<float> v = numbers(attr(sh.attr, "points"));
            std::vector<SkPoint> pts;
            for (size_t i = 0; i + 1 < v.size(); i += 2) pts.push_back(s.m.mapPoint({v[i], v[i + 1]}));
            if (pts.size() < 2) continue;
            SkColor col;
            if (parse_css_color(s.fill, s.color, col) && pts.size() >= 3) {
                if (t == "polygon") chromium_fill_polygon(c, pts.data(), (int)pts.size(), make_paint(col, s.fill_opacity * s.opacity));
                else {
                    SkPathBuilder pb;
                    pb.moveTo(pts[0]);
                    for (size_t i = 1; i < pts.size(); i++) pb.lineTo(pts[i]);
                    fill_path(cx, s, pb.detach());
                }
            }
            SkPathBuilder pb;
            pb.moveTo(pts[0]);
            for (size_t i = 1; i < pts.size(); i++) pb.lineTo(pts[i]);
            if (t == "polygon") pb.close();
            stroke_path(cx, s, pb.detach());
        } else if (t == "line") {
            SkColor col;
            if (!parse_css_color(s.stroke, s.color, col)) continue;
            SkPoint p0 = s.m.mapPoint({num(attr(sh.attr, "x1")), num(attr(sh.attr, "y1"))});
            SkPoint p1 = s.m.mapPoint({num(attr(sh.attr, "x2")), num(attr(sh.attr, "y2"))});
            if (extended && s.dash.size() >= 2) {
                stroke_path(cx, s, SkPathBuilder().moveTo(p0).lineTo(p1).detach());
                continue;
            }
            SkPaint p = make_paint(col, s.stroke_opacity * s.opacity);
            p.setStrokeCap(s.cap);
            chromium_stroke_line(c, p0.fX, p0.fY, p1.fX, p1.fY, s.stroke_width * uniform_scale(s.m), p);
        } else if (t == "rect") {
            float x = length(attr(sh.attr, "x"), vw), y = length(attr(sh.attr, "y"), vh);
            float w = length(attr(sh.attr, "width"), vw), h = length(attr(sh.attr, "height"), vh);
            if (w <= 0 || h <= 0) continue;
            SkRect r = SkRect::MakeXYWH(x, y, w, h);
            if (axis_aligned(s.m)) {
                SkRect dr = s.m.mapRect(r);
                SkColor col;
                if (parse_css_color(s.fill, s.color, col)) chromium_fill_rect(c, dr, make_paint(col, s.fill_opacity * s.opacity));
                if (parse_css_color(s.stroke, s.color, col)) {
                    SkPaint p = make_paint(col, s.stroke_opacity * s.opacity);
                    p.setStyle(SkPaint::kStroke_Style);
                    p.setStrokeWidth(s.stroke_width * uniform_scale(s.m));
                    p.setStrokeJoin(SkPaint::kMiter_Join);
                    p.setStrokeMiter(4.f);
                    chromium_stroke_path(c, SkPath::Rect(dr), p);
                }
            } else {
                SkPath pg = SkPath::Rect(r).makeTransform(s.m);
                fill_path(cx, s, pg);
                stroke_path(cx, s, pg);
            }
        } else if (t == "circle") {
            float r = num(attr(sh.attr, "r"));
            if (r <= 0) continue;
            SkPoint ctr = s.m.mapPoint({num(attr(sh.attr, "cx")), num(attr(sh.attr, "cy"))});
            float sc = uniform_scale(s.m);
            SkColor col;
            if (parse_css_color(s.fill, s.color, col)) chromium_fill_circle(c, ctr.fX, ctr.fY, r * sc, make_paint(col, s.fill_opacity * s.opacity));
            if (parse_css_color(s.stroke, s.color, col))
                chromium_stroke_circle(c, ctr.fX, ctr.fY, r * sc, s.stroke_width * sc, make_paint(col, s.stroke_opacity * s.opacity));
        } else if (t == "path") {
            SkPath pg = parse_path(attr(sh.attr, "d")).makeTransform(s.m);
            fill_path(cx, s, pg);
            stroke_path(cx, s, pg);
        }
    }
    c.restore();
}

}  // namespace a380::mfdx
