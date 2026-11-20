#include "oit.h"
#include "a380/env.h"

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <unordered_map>
#include <utility>
#include <vector>

#include "include/core/SkPaint.h"
#include "include/core/SkPathBuilder.h"

#include "a380/draw.h"
#include "a380/fonts.h"

namespace a380::oit {

namespace {
float g_dpr = 1.f;

SkColor light_of(SkColor c) {
    float r = SkColorGetR(c), g = SkColorGetG(c), b = SkColorGetB(c);
    float v = std::max({r, g, b}) / 255.f;
    if (v <= 0) return SkColorSetRGB(0x54, 0x54, 0x54);
    float m = std::min(1.f, v + 0.33f) / v;
    auto cl = [&](float x) { return (uint8_t)std::lround(std::min(255.f, x * m)); };
    return SkColorSetRGB(cl(r), cl(g), cl(b));
}

SkColor dark_of(SkColor c) {
    float r = SkColorGetR(c), g = SkColorGetG(c), b = SkColorGetB(c);
    float v = std::max({r, g, b}) / 255.f;
    if (v <= 0) return SkColorSetRGB(0, 0, 0);
    float m = std::max(0.f, (v - 0.33f) / v);
    auto cl = [&](float x) { return (uint8_t)std::lround(x * m); };
    return SkColorSetRGB(cl(r), cl(g), cl(b));
}

SkPaint paint(SkColor c) {
    SkPaint p;
    p.setAntiAlias(false);
    p.setColor(c);
    return p;
}

void poly(SkCanvas& c, std::initializer_list<SkPoint> pts, SkColor col) {
    SkPathBuilder b;
    bool first = true;
    for (auto& p : pts) {
        if (first) b.moveTo(p);
        else b.lineTo(p);
        first = false;
    }
    b.close();
    SkPaint p = paint(col);
    c.drawPath(b.detach(), p);
}

SkFont make_font(const Text& t) {
    SkFont f = a380::font(t.family, t.size, false, true);
    if (t.hinted) f.setHinting(SkFontHinting::kSlight);
    return f;
}
}  // namespace

float dpr() { return g_dpr; }
void set_dpr(float v) { g_dpr = v > 0 ? v : 1.f; }

namespace {
struct Run {
    std::vector<SkGlyphID> glyphs;
    std::vector<SkScalar> widths;
    float total = 0;
};

Run shape(const SkFont& f, std::string_view s) {
    Run r;
    r.glyphs.resize(s.size());
    size_t n = f.textToGlyphs(s.data(), s.size(), SkTextEncoding::kUTF8, SkSpan<SkGlyphID>(r.glyphs));
    r.glyphs.resize(n);
    r.widths.resize(n);
    f.getWidths(SkSpan<const SkGlyphID>(r.glyphs), SkSpan<SkScalar>(r.widths));
    for (size_t i = 0; i + 1 < n; i++) r.widths[i] += a380::kerning_px(f, r.glyphs[i], r.glyphs[i + 1]);
    for (float w : r.widths) r.total += w;
    return r;
}

constexpr std::string_view kBox = "\xE2\x96\xAF";

bool has_box(std::string_view s, const Text& t) { return std::string_view(t.family) == "OIT" && s.find(kBox) != std::string_view::npos; }

std::vector<std::pair<std::string_view, bool>> split_boxes(std::string_view s) {
    std::vector<std::pair<std::string_view, bool>> segs;
    size_t i = 0;
    while (i < s.size()) {
        bool box = s.compare(i, kBox.size(), kBox) == 0;
        size_t j = i + (box ? kBox.size() : 0);
        while (!box && j < s.size() && s.compare(j, kBox.size(), kBox) != 0) j++;
        segs.emplace_back(s.substr(i, j - i), box);
        i = j;
    }
    return segs;
}

Text segment_style(const Text& t, bool box) {
    Text st = t;
    st.align = Align::Left;
    if (box) {
        st.family = "Cambria";
        st.hinted = true;
    }
    return st;
}
}  // namespace

float text_width(std::string_view s, const Text& t) {
    if (has_box(s, t)) {
        float total = 0;
        for (auto& sg : split_boxes(s)) total += text_width(sg.first, segment_style(t, sg.second));
        return total;
    }
    return shape(make_font(t), s).total;
}

void text(SkCanvas& c, std::string_view s, float x, float line_top, const Text& t) {
    if (s.empty()) return;
    if (has_box(s, t)) {
        float total = text_width(s, t);
        float cx = t.align == Align::Left ? x : t.align == Align::Center ? x - total * 0.5f : x - total;
        for (auto& sg : split_boxes(s)) {
            Text st = segment_style(t, sg.second);
            text(c, sg.first, cx, line_top, st);
            cx += text_width(sg.first, st);
        }
        return;
    }
    SkFont f = make_font(t);
    Run run = shape(f, s);
    float w = run.total;
    float x0 = t.align == Align::Left ? x : t.align == Align::Center ? x - w * 0.5f : x - w;
    SkFontMetrics fm;
    a380::font("OIT", t.size, false, true).getMetrics(&fm);
    float asc = std::round(-fm.fAscent * g_dpr), desc = std::round(fm.fDescent * g_dpr);
    float y = std::round(a380::layout_unit(line_top * g_dpr) + std::floor((a380::layout_unit(t.line_height * g_dpr) - asc - desc) * 0.5f) + asc) / g_dpr;
    static const bool tlog = a380::getenv_cached("OIT_TEXTLOG") != nullptr;
    if (tlog) std::fprintf(stderr, "TXT\t%.*s\t%.3f\t%.3f\t%.3f\t%.1f\n", (int)s.size(), s.data(), x0, y - asc / g_dpr + (asc + desc) / 2 / g_dpr, w, t.size);
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(t.color);
    SkFont fd = f;
    if (t.bold) fd.setEmbolden(true);
    float cx = x0;
    for (size_t i = 0; i < run.glyphs.size(); i++) {
        a380::draw_simple_text(c, &run.glyphs[i], sizeof(SkGlyphID), SkTextEncoding::kGlyphID, cx, y, fd, p);
        cx += run.widths[i];
    }
}

void fill(SkCanvas& c, SkRect r, SkColor col) { c.drawRect(r, paint(col)); }

void bevel(SkCanvas& c, SkRect r, Bevel kind, SkColor color, float w) {
    SkColor tl = kind == Bevel::Outset ? light_of(color) : dark_of(color);
    SkColor br = kind == Bevel::Outset ? dark_of(color) : light_of(color);
    SkRect in = r.makeInset(w, w);
    poly(c, {{r.left(), r.top()}, {r.right(), r.top()}, {in.right(), in.top()}, {in.left(), in.top()}}, tl);
    poly(c, {{r.left(), r.top()}, {in.left(), in.top()}, {in.left(), in.bottom()}, {r.left(), r.bottom()}}, tl);
    poly(c, {{r.left(), r.bottom()}, {in.left(), in.bottom()}, {in.right(), in.bottom()}, {r.right(), r.bottom()}}, br);
    poly(c, {{r.right(), r.top()}, {r.right(), r.bottom()}, {in.right(), in.bottom()}, {in.right(), in.top()}}, br);
}

void border(SkCanvas& c, SkRect r, SkColor col, float w) {
    fill(c, SkRect::MakeLTRB(r.left(), r.top(), r.right(), r.top() + w), col);
    fill(c, SkRect::MakeLTRB(r.left(), r.bottom() - w, r.right(), r.bottom()), col);
    fill(c, SkRect::MakeLTRB(r.left(), r.top() + w, r.left() + w, r.bottom() - w), col);
    fill(c, SkRect::MakeLTRB(r.right() - w, r.top() + w, r.right(), r.bottom() - w), col);
}

float button_text_width(const std::vector<std::string>& lines, const ButtonOpts& o) {
    Text t;
    t.size = o.size;
    float w = 0;
    for (auto& l : lines) w = std::max(w, text_width(l, t));
    return w;
}

SkRect button(SkCanvas& c, SkRect r, const std::vector<std::string>& lines, const ButtonOpts& o) {
    fill(c, r, pal::darker_grey);
    bevel(c, r, o.selected ? Bevel::Inset : Bevel::Outset);
    if (o.selected) fill(c, r.makeInset(2, 2), pal::dark_grey);
    Text t;
    t.size = o.size;
    t.align = Align::Center;
    t.color = o.disabled ? pal::dark_grey : pal::white;
    t.line_height = o.line_height;
    float h = o.line_height * lines.size();
    float top = r.centerY() - h * 0.5f;
    for (size_t i = 0; i < lines.size(); i++) text(c, lines[i], o.text_cx >= 0 ? o.text_cx : r.centerX(), top + i * o.line_height, t);
    return r;
}

}  // namespace a380::oit
