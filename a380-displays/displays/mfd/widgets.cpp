#include "mfd/widgets.h"
#include "a380/env.h"

#include <algorithm>
#include <cmath>
#include <cstdlib>
#include <cstdio>

#include "include/core/SkFont.h"
#include "include/core/SkFontMetrics.h"
#include "include/core/SkMatrix.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPath.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkRRect.h"
#include "include/core/SkTypeface.h"
#include "include/core/SkFourByteTag.h"

#include "a380/draw.h"
#include "a380/fonts.h"
#include "a380/raster.h"

namespace a380::mfd::ui {

namespace {

const char* family(Face f) { return f == Face::Ecam ? "Ecam" : "FBW-Display-EIS-A380-SlashedZero"; }

bool g_lcd = false;
struct LcdScope {
    bool prev;
    explicit LcdScope(bool on) : prev(g_lcd) { g_lcd = on; }
    ~LcdScope() { g_lcd = prev; }
};

SkFont make_font(const TextStyle& s) {
    if (g_lcd) return a380::font(family(s.face), s.size, false, a380::TextMode::Lcd);
    return a380::font(family(s.face), s.size);
}

SkPaint fill_paint(SkColor c) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(c);
    return p;
}

SkColor chromium_light(SkColor c) {
    float r = SkColorGetR(c), g = SkColorGetG(c), b = SkColorGetB(c);
    float v = std::max({r, g, b}) / 255.f;
    if (v <= 0) return SkColorSetRGB(0x54, 0x54, 0x54);
    float m = std::min(1.f, v + 0.33f) / v;
    auto cl = [&](float x) { return (uint8_t)std::lround(std::min(255.f, x * m)); };
    return SkColorSetRGB(cl(r), cl(g), cl(b));
}

SkColor chromium_dark(SkColor c) {
    float r = SkColorGetR(c), g = SkColorGetG(c), b = SkColorGetB(c);
    float v = std::max({r, g, b}) / 255.f;
    if (v <= 0) return SkColorSetRGB(0, 0, 0);
    float m = std::max(0.f, (v - 0.33f) / v);
    auto cl = [&](float x) { return (uint8_t)std::lround(x * m); };
    return SkColorSetRGB(cl(r), cl(g), cl(b));
}

float device_scale(SkCanvas& c, bool vertical) {
    SkMatrix mx = c.getLocalToDeviceAs3x3();
    float s = vertical ? mx.getScaleY() : mx.getScaleX();
    return s > 0 ? s : 1.f;
}

void rect_fill(SkCanvas& c, SkRect r, SkColor col) {
    SkPaint p;
    p.setColor(col);
    p.setAntiAlias(false);
    c.drawRect(r, p);
}

void poly(SkCanvas& c, std::initializer_list<SkPoint> pts, SkColor col, bool aa = true) {
    SkPathBuilder b;
    bool first = true;
    for (auto& pt : pts) {
        if (first) b.moveTo(pt), first = false;
        else b.lineTo(pt);
    }
    b.close();
    SkPaint pp = fill_paint(col);
    pp.setAntiAlias(aa);
    c.drawPath(b.detach(), pp);
}

}  // namespace

TextStyle label_style(SkColor color, float size) { return {size, color, Face::Mfd, Align::Left}; }
TextStyle label_bigger_style(SkColor color) { return label_style(color, 24); }
TextStyle label_biggest_style(SkColor color) { return label_style(color, 28); }
TextStyle value_style(SkColor color, float size) { return {size, color, Face::Mfd, Align::Left}; }
TextStyle value_smaller_style(SkColor color) { return value_style(color, 20); }
TextStyle value_bigger_style(SkColor color) { return value_style(color, 30); }
TextStyle unit_style(float size) { return {size, pal::unit_blue, Face::Mfd, Align::Left}; }

float text_width(std::string_view t, const TextStyle& s) {
    if (t.empty()) return 0;
    SkFont f = make_font(s);
    return f.measureText(t.data(), t.size(), SkTextEncoding::kUTF8);
}

std::vector<std::string> wrap_text(std::string_view t, const TextStyle& s, float max_width) {
    std::vector<std::string> out;
    std::string cur;
    size_t a = 0;
    std::string src(t);
    while (a <= src.size()) {
        size_t b = src.find_first_of(" \n", a);
        if (b == std::string::npos) b = src.size();
        std::string word = src.substr(a, b - a);
        a = b + 1;
        if (word.empty()) continue;
        std::string trial = cur.empty() ? word : cur + " " + word;
        if (!cur.empty() && text_width(trial, s) > max_width) {
            out.push_back(cur);
            cur = word;
        } else {
            cur = trial;
        }
    }
    if (!cur.empty()) out.push_back(cur);
    if (out.empty()) out.emplace_back();
    return out;
}

float line_height(const TextStyle& s) { return s.size * (s.face == Face::Ecam ? 0.85f : 0.8932f); }

namespace {
unsigned g_bit = 0;
unsigned css_mask() {
    static unsigned m = [] {
        const char* e = a380::getenv_cached("MFD_CSS");
        return e ? (unsigned)std::strtoul(e, nullptr, 0) : 6115u;
    }();
    return m;
}
bool css_on() { return g_bit && (css_mask() & g_bit); }
struct Scope {
    unsigned prev;
    explicit Scope(unsigned b) : prev(g_bit) { g_bit = b; }
    ~Scope() { g_bit = prev; }
};
float cur_dpr(SkCanvas& c) { return device_scale(c, true); }
struct Hhea {
    float asc = 0.8f, desc = 0.2f, gap = 0.f;
};
Hhea hhea_of(const SkFont& f) {
    Hhea r;
    SkTypeface* tf = f.getTypeface();
    uint8_t h[10];
    if (!tf || tf->getTableData(SkSetFourByteTag('h', 'h', 'e', 'a'), 0, 10, h) != 10) return r;
    int upem = tf->getUnitsPerEm();
    if (upem <= 0) return r;
    r.asc = (int16_t)((h[4] << 8) | h[5]) / (float)upem;
    r.desc = -(int16_t)((h[6] << 8) | h[7]) / (float)upem;
    r.gap = (int16_t)((h[8] << 8) | h[9]) / (float)upem;
    return r;
}
float lh_c_dpr(const TextStyle& s, float dpr) {
    SkFont f = make_font(s);
    Hhea m = hhea_of(f);
    float sz = s.size * dpr;
    return (std::round(m.asc * sz) + std::round(m.desc * sz) + std::round(m.gap * sz)) / dpr;
}
float css_baseline_y(const SkFont& f, float top, float dpr) {
    Hhea m = hhea_of(f);
    float sz = f.getSize() * dpr;
    float a = std::round(m.asc * sz), d = std::round(m.desc * sz), g = std::round(m.gap * sz);
    float t = a380::layout_unit(top * dpr);
    float half = a380::layout_unit(g * 0.5f);
    return std::floor(t + half + a + 0.5f) / dpr;
}
float g_dpr = 1.5f;
float lh_c(const TextStyle& s) { return css_on() ? lh_c_dpr(s, g_dpr) : line_height(s); }
}  // namespace

void text(SkCanvas& c, std::string_view t, float x, float top, const TextStyle& s) {
    if (t.empty()) return;
    SkFont f = make_font(s);
    float baseline = top + s.size * 0.8476f;
    g_dpr = cur_dpr(c);
    if (css_on()) baseline = css_baseline_y(f, top, g_dpr);
    else {
        SkMatrix mx = c.getLocalToDeviceAs3x3();
        float sy = mx.getScaleY(), ty = mx.getTranslateY();
        if (sy > 0) baseline = (std::round(ty + baseline * sy) - ty) / sy;
    }
    float w = f.measureText(t.data(), t.size(), SkTextEncoding::kUTF8);
    float px = x;
    if (s.align == Align::Center) px = x - w * 0.5f;
    else if (s.align == Align::Right) px = x - w;
    static const bool log = a380::getenv_cached("MFD_TEXTLOG") != nullptr;
    if (log) {
        SkMatrix lm = c.getLocalToDeviceAs3x3();
        std::fprintf(stderr, "TXT\t%s\t%.3f\t%.3f\t%.3f\t%.1f\n", std::string(t).c_str(), lm.mapPoint({px, baseline}).fX, lm.mapPoint({px, baseline}).fY,
                     w * lm.getScaleX(), s.size);
    }
    a380::draw_simple_text(c, t.data(), t.size(), SkTextEncoding::kUTF8, px, baseline, f, fill_paint(s.color));
}

void text_in(SkCanvas& c, std::string_view t, SkRect box, const TextStyle& s) {
    TextStyle cs = s;
    cs.align = Align::Center;
    text(c, t, box.centerX(), box.centerY() - lh_c(s) * 0.5f, cs);
}

void text_in_aligned(SkCanvas& c, std::string_view t, SkRect box, const TextStyle& s, float pad_x) {
    float x = s.align == Align::Left ? box.left() + pad_x : s.align == Align::Right ? box.right() - pad_x : box.centerX();
    text(c, t, x, box.centerY() - lh_c(s) * 0.5f, s);
}

void label(SkCanvas& c, std::string_view s, float x, float top, SkColor color, float size, Align a) {
    TextStyle st = label_style(color, size);
    st.align = a;
    text(c, s, x, top, st);
}

void value(SkCanvas& c, std::string_view s, float x, float top, SkColor color, float size, Align a) {
    TextStyle st = value_style(color, size);
    st.align = a;
    text(c, s, x, top, st);
}

float value_with_unit(SkCanvas& c, std::string_view v, std::string_view u, float x, float top, SkColor color,
                      bool leading, float vs, float us) {
    TextStyle vt = value_style(color, vs), ut = unit_style(us);
    float lh = std::max(lh_c(vt), lh_c(ut));
    float vt_top = top + (lh - lh_c(vt)) * 0.5f;
    float ut_top = top + (lh - lh_c(ut)) * 0.5f;
    if (leading) {
        text(c, u, x, ut_top, ut);
        x += text_width(u, ut) + 6;
        text(c, v, x, vt_top, vt);
        return x + text_width(v, vt);
    }
    text(c, v, x, vt_top, vt);
    x += text_width(v, vt) + 6;
    text(c, u, x, ut_top, ut);
    return x + text_width(u, ut);
}

void bevel_border(SkCanvas& c, SkRect r, Bevel kind, SkColor color, float w) {
    SkColor tl = color, br = color;
    if (kind == Bevel::Outset) tl = chromium_light(color), br = chromium_dark(color);
    else if (kind == Bevel::Inset) tl = chromium_dark(color), br = chromium_light(color);
    float sx = device_scale(c, false), sy = device_scale(c, true);
    float l = std::round(r.left() * sx), t = std::round(r.top() * sy);
    float rr = std::round(r.right() * sx), b = std::round(r.bottom() * sy);
    float wx = std::max(1.f, std::floor(w * sx + 0.001f)), wy = std::max(1.f, std::floor(w * sy + 0.001f));
    auto X = [&](float v) { return v / sx; };
    auto Y = [&](float v) { return v / sy; };
    SkPoint o0{X(l), Y(t)}, o1{X(rr), Y(t)}, o2{X(rr), Y(b)}, o3{X(l), Y(b)};
    SkPoint i0{X(l + wx), Y(t + wy)}, i1{X(rr - wx), Y(t + wy)}, i2{X(rr - wx), Y(b - wy)}, i3{X(l + wx), Y(b - wy)};
    poly(c, {o0, o1, i1, i0}, tl, false);
    poly(c, {o0, i0, i3, o3}, tl, false);
    poly(c, {o3, i3, i2, o2}, br, false);
    poly(c, {o1, o2, i2, i1}, br, false);
}

void fill(SkCanvas& c, SkRect r, SkColor col) { rect_fill(c, r, col); }

void stroke_box(SkCanvas& c, SkRect r, SkColor col, float w) {
    if (w <= 1.001f) {
        float sx = device_scale(c, false), sy = device_scale(c, true);
        float l = std::round(r.left() * sx), t = std::round(r.top() * sy);
        float rr = std::round(r.right() * sx), b = std::round(r.bottom() * sy);
        rect_fill(c, SkRect::MakeLTRB(l / sx, t / sy, rr / sx, (t + 1) / sy), col);
        rect_fill(c, SkRect::MakeLTRB(l / sx, (b - 1) / sy, rr / sx, b / sy), col);
        rect_fill(c, SkRect::MakeLTRB(l / sx, (t + 1) / sy, (l + 1) / sx, (b - 1) / sy), col);
        rect_fill(c, SkRect::MakeLTRB((rr - 1) / sx, (t + 1) / sy, rr / sx, (b - 1) / sy), col);
        return;
    }
    rect_fill(c, SkRect::MakeLTRB(r.left(), r.top(), r.right(), r.top() + w), col);
    rect_fill(c, SkRect::MakeLTRB(r.left(), r.bottom() - w, r.right(), r.bottom()), col);
    rect_fill(c, SkRect::MakeLTRB(r.left(), r.top() + w, r.left() + w, r.bottom() - w), col);
    rect_fill(c, SkRect::MakeLTRB(r.right() - w, r.top() + w, r.right(), r.bottom() - w), col);
}

void hline(SkCanvas& c, float x0, float x1, float y, SkColor col, float w) {
    if (w <= 1.001f) {
        float s = device_scale(c, true);
        float row = std::round(y * s);
        rect_fill(c, SkRect::MakeLTRB(x0, row / s, x1, (row + 1) / s), col);
        return;
    }
    rect_fill(c, SkRect::MakeLTRB(x0, y, x1, y + w), col);
}

void vline(SkCanvas& c, float x, float y0, float y1, SkColor col, float w) {
    if (w <= 1.001f) {
        float s = device_scale(c, false);
        float col_px = std::round(x * s);
        rect_fill(c, SkRect::MakeLTRB(col_px / s, y0, (col_px + 1) / s, y1), col);
        return;
    }
    rect_fill(c, SkRect::MakeLTRB(x, y0, x + w, y1), col);
}

void triangle_down(SkCanvas& c, float x, float y, SkColor col, float s) {
    poly(c, {{x, y}, {x + s, y}, {x + s * 0.5f, y + s}}, col);
}

void triangle_up(SkCanvas& c, float x, float y, SkColor col, float s) {
    poly(c, {{x + s * 0.5f, y}, {x + s, y + s}, {x, y + s}}, col);
}

void triangle_left(SkCanvas& c, float x, float y, SkColor col, float s) {
    poly(c, {{x, y + s * 0.5f}, {x + s, y + s}, {x + s, y}}, col);
}

void asterisk(SkCanvas& c, float cx, float cy, SkColor col, float size) {
    TextStyle st = label_style(col, size);
    st.align = Align::Center;
    text(c, "*", cx, cy - lh_c(st) * 0.5f, st);
}

void button(SkCanvas& c, SkRect r, std::string_view lbl, const ButtonStyle& st) {
    Scope scope_(1);
    bool disabled = st.state == ButtonState::Disabled;
    bool sel = st.state == ButtonState::Selected;
    bool hi = st.state == ButtonState::Highlighted;
    rect_fill(c, r, sel ? pal::dark_grey : hi ? pal::white : pal::darker_grey);
    bevel_border(c, r, sel ? Bevel::Inset : Bevel::Outset, pal::light_grey);
    SkColor col = st.text_color;
    if (!col) col = disabled ? pal::dark_grey : hi ? pal::darker_grey : st.tmpy ? pal::yellow : pal::white;
    TextStyle ts{st.font_size, col, Face::Mfd, Align::Left};
    float lh = lh_c(ts);
    float cl = r.left() + 2 + st.pad_left, cr = r.right() - 2 - (st.arrow ? 5 : st.pad_right);
    float ct = r.top() + 2 + st.pad_top, cb = r.bottom() - 2 - st.pad_bottom;
    float ty = (ct + cb) * 0.5f - lh * 0.5f;
    if (r.height() > button_height(st.font_size) + 5.f) ty -= 1.f / cur_dpr(c);
    if (st.arrow) {
        float aw = 15;
        float labw = text_width(lbl, ts);
        float region_l = cl, region_r = r.right() - 2 - st.arrow_label_right - aw;
        ts.align = Align::Center;
        text(c, lbl, (region_l + 5 + region_r - 10) * 0.5f + (labw > 0 ? 0 : 0), ty, ts);
        float ay = (ct + cb) * 0.5f - 7.5f;
        SkColor ac = disabled ? pal::dark_grey : st.tmpy ? pal::yellow : pal::white;
        if (st.arrow_up) triangle_up(c, cr - aw, ay, ac);
        else triangle_down(c, cr - aw, ay, ac);
    } else if (st.asterisk) {
        float lw = text_width(lbl, ts);
        float sw = text_width("*", ts);
        float left = cl + ((cr - cl) - (lw + 10 + sw)) * 0.5f;
        text(c, lbl, left, ty, ts);
        text(c, "*", left + lw + 10, ty, ts);
    } else {
        ts.align = Align::Center;
        text(c, lbl, (cl + cr) * 0.5f, ty, ts);
    }
}

float button_height(float fs, int lines) {
    TextStyle ts{fs, pal::white, Face::Mfd, Align::Left};
    float lh = css_mask() & 1 ? lh_c_dpr(ts, 1.5f) : line_height(ts);
    return 2 + 9 + lh * lines + 5 + 2;
}

void button_lines(SkCanvas& c, SkRect r, const std::vector<std::string>& lines, const ButtonStyle& st) {
    Scope scope_(1);
    bool disabled = st.state == ButtonState::Disabled;
    bool sel = st.state == ButtonState::Selected;
    rect_fill(c, r, sel ? pal::dark_grey : st.state == ButtonState::Highlighted ? pal::white : pal::darker_grey);
    bevel_border(c, r, sel ? Bevel::Inset : Bevel::Outset, pal::light_grey);
    TextStyle ts{st.font_size, st.text_color ? st.text_color : disabled ? pal::dark_grey : pal::white, Face::Mfd,
                 Align::Center};
    float lh = lh_c(ts);
    float ct = r.top() + 2 + st.pad_top, cb = r.bottom() - 2 - st.pad_bottom;
    float total = lh * lines.size();
    if (st.asterisk) {
        TextStyle as = ts;
        as.align = Align::Left;
        float tw = 0;
        for (auto& l : lines) tw = std::max(tw, text_width(l, as));
        float sw = text_width("*", as);
        float cl = r.left() + 2 + st.pad_left, cr = r.right() - 2 - st.pad_right;
        float left = cl + ((cr - cl) - (tw + 10 + sw)) * 0.5f;
        float yy = (ct + cb) * 0.5f - total * 0.5f;
        for (auto& l : lines) {
            ts.align = Align::Center;
            text(c, l, left + tw * 0.5f, yy, ts);
            yy += lh;
        }
        text(c, "*", left + tw + 10, (ct + cb) * 0.5f - lh * 0.5f, as);
        return;
    }
    float y = (ct + cb) * 0.5f - total * 0.5f - 0.67f;
    for (auto& l : lines) {
        text(c, l, r.centerX() - 0.25f, y, ts);
        y += lh;
    }
}

void icon_button(SkCanvas& c, SkRect r, bool) {
    rect_fill(c, r, pal::darker_grey);
    bevel_border(c, r, Bevel::Outset, pal::light_grey);
}

float input_field_width(std::string_view t, const FieldStyle& st) {
    TextStyle ts{st.kind == FieldKind::Computed ? 22.f : st.font_size, pal::white, Face::Mfd, Align::Left};
    float w = std::max(st.min_text_width, text_width(t, ts));
    if (st.kind == FieldKind::Editing) w += 18;
    TextStyle us = unit_style();
    float lw = text_width(st.unit_leading, us), tw = text_width(st.unit_trailing, us);
    return 4 + 4 + lw + 6 + w + 6 + tw;
}

void input_field(SkCanvas& c, SkRect r, std::string_view t, const FieldStyle& st) {
    Scope scope_(2);
    bool inactive = st.kind == FieldKind::Inactive || !st.border;
    bool disabled = st.kind == FieldKind::Disabled;
    rect_fill(c, r, disabled ? pal::darker_grey : pal::background);
    if (!inactive) bevel_border(c, r, Bevel::Inset, pal::light_grey);
    SkColor col = st.text_color;
    if (!col) {
        switch (st.kind) {
            case FieldKind::Entered: col = pal::cyan; break;
            case FieldKind::Mandatory: col = pal::amber; break;
            case FieldKind::Computed: col = pal::green; break;
            case FieldKind::Inactive: col = pal::green; break;
            case FieldKind::Disabled: col = st.yellow_when_disabled ? pal::yellow : pal::dark_grey; break;
            case FieldKind::Tmpy: col = pal::yellow; break;
            case FieldKind::Selected: col = pal::background; break;
            case FieldKind::Editing: col = pal::cyan; break;
            case FieldKind::Validating: col = pal::light_grey; break;
        }
    }
    float fs = st.kind == FieldKind::Computed ? 22.f : st.font_size;
    TextStyle ts{fs, col, Face::Mfd, Align::Left};
    TextStyle us = unit_style();
    float inner_l = r.left() + 2 + 2, inner_r = r.right() - 2 - 2;
    float lw = text_width(st.unit_leading, us), tw = text_width(st.unit_trailing, us);
    float cy = r.centerY();
    float uy = cy - lh_c(us) * 0.5f;
    text(c, st.unit_leading, inner_l, uy, us);
    text(c, st.unit_trailing, inner_r - tw, uy, us);
    float tl = inner_l + lw + 6, trr = inner_r - tw - 6;
    float w = text_width(t, ts);
    float x = (st.align == Align::Left ? tl : st.align == Align::Right ? trr - w : (tl + trr) * 0.5f - w * 0.5f) + st.text_dx;
    float ty = cy - lh_c(ts) * 0.5f - (fs > 24 ? 1.f / cur_dpr(c) : 0.f) + st.text_dy;
    if (st.kind == FieldKind::Selected) {
        rect_fill(c, SkRect::MakeLTRB(x, ty, x + w, ty + lh_c(ts)), pal::cyan);
    }
    text(c, t, x, ty, ts);
    if (st.kind == FieldKind::Editing) rect_fill(c, SkRect::MakeXYWH(x + w, cy - 13.5f, 18, 27), pal::cyan);
}

void dropdown(SkCanvas& c, SkRect r, std::string_view t, const DropdownStyle& st) {
    Scope scope_(4);
    if (st.disabled && !st.inactive) {
        rect_fill(c, r, pal::darker_grey);
        bevel_border(c, r, Bevel::Outset, pal::light_grey);
        SkRect in = r.makeInset(2, 2);
        in.fRight -= 25;
        rect_fill(c, in, pal::darker_grey);
        triangle_down(c, r.right() - 2 - 25 + 5, r.centerY() - 7.5f, 0xFF808080);
        TextStyle ts{st.font_size, pal::dark_grey, Face::Mfd, st.align};
        text_in_aligned(c, t, in, ts, 10);
        return;
    }
    if (st.inactive) {
        rect_fill(c, r, pal::background);
    } else {
        rect_fill(c, r, pal::darker_grey);
        bevel_border(c, r, Bevel::Outset, pal::light_grey);
    }
    SkRect inner = r.makeInset(2, 2);
    inner.fRight -= 25;
    rect_fill(c, inner, pal::background);
    if (!st.inactive) triangle_down(c, r.right() - 2 - 25 + 5, r.centerY() - 7.5f, pal::white);
    TextStyle ts{st.font_size, st.text_color, Face::Mfd, st.align};
    text_in_aligned(c, t, inner, ts, 10);
}

float dropdown_menu_item_height() { return 100.f / 3.f; }

float dropdown_menu_natural_width(const std::vector<MenuItem>& items) {
    TextStyle ts{20, pal::white, Face::Mfd, Align::Left};
    float w = 0;
    for (auto& i : items) w = std::max(w, text_width(i.text, ts));
    return w + 32 + 6 + 4;
}

float dropdown_menu(SkCanvas& c, SkRect r, const std::vector<MenuItem>& items, int hover) {
    Scope scope_(8);
    LcdScope lcd_(a380::chromium_text_mode(true) == a380::TextMode::Lcd);
    float ih = dropdown_menu_item_height();
    float h = std::min(275.f, ih * items.size() + 2.67f);
    SkRect box = SkRect::MakeXYWH(r.left(), r.top(), r.width(), h);
    rect_fill(c, box, pal::darker_grey);
    bevel_border(c, box, Bevel::Outset, pal::light_grey);
    TextStyle ts{20, pal::white, Face::Mfd, Align::Left};
    float y = box.top() + 2;
    for (size_t i = 0; i < items.size(); ++i) {
        if (y + ih > box.bottom() + 1.f) break;
        ts.color = items[i].disabled ? pal::dark_grey : pal::white;
        if ((int)i == hover) stroke_box(c, SkRect::MakeXYWH(box.left() + 2, y, box.width() - 4, ih), pal::cyan, 3);
        float sx = device_scale(c, false);
        float tx = (std::round(box.left() * sx) + std::floor(2 * sx + 0.001f) + std::floor(3 * sx + 0.001f) + 16 * sx) / sx;
        text(c, items[i].text, tx, y + 3 + 5, ts);
        y += ih;
    }
    return h;
}

float context_menu_item_height() { return 42; }

float context_menu(SkCanvas& c, float x, float y, float width, const std::vector<MenuItem>& items, int hover) {
    Scope scope_(8);
    LcdScope lcd_(a380::chromium_text_mode(true) == a380::TextMode::Lcd);
    float ih = context_menu_item_height();
    float h = ih * items.size() + 4;
    SkRect box = SkRect::MakeXYWH(x, y, width, h);
    rect_fill(c, box, pal::darker_grey);
    bevel_border(c, box, Bevel::Outset, pal::light_grey);
    TextStyle ts{22, pal::white, Face::Mfd, Align::Left};
    float iy = y + 2;
    for (size_t i = 0; i < items.size(); ++i) {
        ts.color = items[i].disabled ? pal::dark_grey : pal::white;
        if ((int)i == hover && !items[i].disabled) stroke_box(c, SkRect::MakeXYWH(x + 2, iy, width - 4, ih), pal::cyan, 3);
        text(c, items[i].text, x + 2 + 3 + 2, iy + 3 + 7 + (ih - 20 - lh_c(ts)) * 0.5f, ts);
        iy += ih;
    }
    return h;
}

void page_selector(SkCanvas& c, SkRect r, std::string_view lbl, bool active_boxed, bool) {
    page_selector_ex(c, r, lbl, active_boxed, {});
}

void page_selector_ex(SkCanvas& c, SkRect r, std::string_view lbl, bool active_boxed, const PageSelectorStyle& st) {
    Scope scope_(16);
    rect_fill(c, r, pal::darker_grey);
    SkColor soft = SkColorSetARGB(110, 211, 211, 211);
    rect_fill(c, SkRect::MakeLTRB(r.left(), r.top(), r.right(), r.top() + 2), pal::light_grey);
    rect_fill(c, SkRect::MakeLTRB(r.left(), r.top() + 2, r.left() + 2, r.bottom()), pal::light_grey);
    SkPaint p;
    p.setColor(soft);
    c.drawRect(SkRect::MakeLTRB(r.left() + 2, r.bottom() - 2, r.right(), r.bottom()), p);
    c.drawRect(SkRect::MakeLTRB(r.right() - 2, r.top() + 2, r.right(), r.bottom() - 2), p);
    float tri_w = st.arrow ? 31 : 0;
    SkRect lab = SkRect::MakeLTRB(r.left() + 2, r.top() + 2, r.right() - 2 - tri_w, r.bottom() - 2);
    TextStyle ts{22, st.color, Face::Ecam, Align::Center};
    float lh = lh_c(ts);
    std::vector<std::string> lines;
    {
        std::string s(lbl);
        size_t a = 0;
        while (true) {
            size_t b = s.find('\n', a);
            std::string part = s.substr(a, b == std::string::npos ? b : b - a);
            size_t f = part.find_first_not_of(' ');
            lines.push_back(f == std::string::npos ? "" : part.substr(f));
            if (b == std::string::npos) break;
            a = b + 1;
        }
    }
    if (lbl.find('\n') == std::string_view::npos) {
        float avail = lab.width() - 28 - 4 + 4;
        if (text_width(lbl, ts) > avail) lines = wrap_text(lbl, ts, avail);
    }
    float cy = lab.centerY();
    float top = cy - 0.67f - lh * lines.size() * 0.5f;
    for (size_t i = 0; i < lines.size(); ++i) {
        float tw = text_width(lines[i], ts);
        float y = top + lh * i;
        if (st.background) rect_fill(c, SkRect::MakeXYWH(lab.centerX() - tw * 0.5f, y, tw, lh), st.background);
        text(c, lines[i], lab.centerX(), y, ts);
    }
    if (active_boxed && lines.size() > 1) {
        float avail = lab.width() - 28;
        float bh = lh * lines.size() + 4;
        stroke_box(c, SkRect::MakeXYWH(lab.centerX() - (avail + 10) * 0.5f, top + lh * lines.size() * 0.5f - bh * 0.5f, avail + 10, bh),
                   pal::white, 1);
    }
    if (active_boxed && lines.size() == 1) {
        float tw = text_width(lines[0], ts);
        stroke_box(c, SkRect::MakeXYWH(lab.centerX() - tw * 0.5f - 4, cy - 13.15f, tw + 8, 26.3f), pal::white, 1);
    }
    if (st.arrow) triangle_down(c, r.right() - 2 - 8 - 15, r.centerY() - 7.5f, pal::white);
}

float title_bar(SkCanvas& c, SkRect r, const TitleBar& tb) {
    Scope scope_(32);
    TextStyle ts{30, pal::title_text, Face::Mfd, Align::Left};
    float h = 36.6f;
    float tmpy_w = 100, eo_w = 70, gap = tb.gap;
    float title_r = r.right() - tmpy_w - eo_w - 2 * gap;
    rect_fill(c, SkRect::MakeLTRB(r.left(), r.top(), title_r, r.top() + h), pal::dark_grey);
    rect_fill(c, SkRect::MakeLTRB(title_r + gap, r.top(), title_r + gap + eo_w, r.top() + h), pal::dark_grey);
    rect_fill(c, SkRect::MakeLTRB(r.right() - tmpy_w, r.top(), r.right(), r.top() + h), pal::dark_grey);
    text(c, tb.title + " ", r.left() + 7, r.top() + 3, ts);
    auto tag = [&](SkRect area, const char* s, SkColor bg) {
        TextStyle t2{30, pal::title_text, Face::Mfd, Align::Center};
        float w = text_width(s, t2) + 4;
        SkRect b = SkRect::MakeXYWH(area.centerX() - w * 0.5f, area.centerY() - (lh_c(t2) + 4) * 0.5f, w,
                                    lh_c(t2) + 4);
        rect_fill(c, b, bg);
        text(c, s, area.centerX(), b.top() + 2, t2);
    };
    if (tb.penalty) tag(SkRect::MakeLTRB(title_r - 100, r.top(), title_r - 3, r.top() + h), "PENALTY", pal::white);
    if (tb.eo) tag(SkRect::MakeLTRB(title_r + gap, r.top(), title_r + gap + eo_w, r.top() + h), "EO", pal::amber);
    if (tb.tmpy) tag(SkRect::MakeLTRB(r.right() - tmpy_w, r.top(), r.right(), r.top() + h), "TMPY", pal::yellow);
    return h;
}

SkRect tab_navigator(SkCanvas& c, SkRect rect, const std::vector<std::string>& titles, const TabStyle& st) {
    Scope scope_(64);
    SkRect box = SkRect::MakeLTRB(rect.left() + 9, rect.top() + 2, rect.right() - 2, rect.bottom() - 2);
    {
        float dpr = device_scale(c, true);
        auto snap = [&](float v) {
            float d = v * dpr, r = std::round(d);
            return std::fabs(d - r) < 0.08f ? r / dpr : v;
        };
        box.fTop = snap(box.fTop);
        box.fBottom = snap(box.fBottom);
    }
    float bh = st.bar_height;
    float tw = bh * std::tan(st.slant_deg * 3.14159265f / 180.f);
    std::vector<int> idx;
    for (size_t i = 0; i < titles.size(); ++i)
        if (!titles[i].empty()) idx.push_back((int)i);
    bool small = st.small_font < 0 ? idx.size() > 2 : st.small_font != 0;
    float fs = std::round(bh * (small ? 0.58f : 0.66f));
    float avail = box.width() - st.additional_right_space;
    int n = (int)idx.size();
    float elem_w = n ? (avail + 15 * (n - 1)) / n : 0;
    float bar_top = box.top(), bar_bot = box.top() + bh;
    float x = box.left();
    SkColor light = 0xFFD3D3D3;
    // bottom rule of the bar (inactive tabs and the spacer); the selected tab leaves a gap
    std::vector<int> order;
    for (int k = 0; k < n; ++k)
        if (idx[k] != st.selected) order.push_back(k);
    for (int k = 0; k < n; ++k)
        if (idx[k] == st.selected) order.push_back(k);
    for (int kk = 0; kk < n; ++kk) {
        int k = order[kk];
        int i = idx[k];
        bool sel = i == st.selected;
        x = box.left() + k * (elem_w - 15);
        float x0 = x, x1 = x + elem_w;
        float z_l = x0 + tw, z_r = x1 - tw;
        SkColor fillc = sel ? pal::background : pal::darker_grey;
        SkColor edge = light;
        rect_fill(c, SkRect::MakeLTRB(z_l, bar_top, z_r, bar_bot), fillc);
        rect_fill(c, SkRect::MakeLTRB(z_l, bar_top, z_r, bar_top + 2), pal::light_grey);
        if (!sel) hline(c, x0, x1, bar_bot - 2, pal::light_grey, 2);
        {
            float dx = device_scale(c, false), dy = device_scale(c, true);
            auto sx = [&](float v) { return std::round(v * dx) / dx; };
            auto sy = [&](float v) { return std::round(v * dy) / dy; };
            float yt = sy(bar_top), yb = sy(bar_bot);
            SkPaint fp = fill_paint(fillc);
            SkPaint lp = fill_paint(edge);
            lp.setStrokeCap(SkPaint::kButt_Cap);
            for (int side = 0; side < 2; ++side) {
                float ox = sx(side == 0 ? x0 : x1 - tw);
                float ow = sx((side == 0 ? x0 : x1 - tw) + tw) - ox;
                float h = yb - yt;
                c.save();
                c.clipRect(SkRect::MakeXYWH(ox, yt, ow, h), false);
                SkPoint tri[3];
                float ax, ay, bx, by;
                if (side == 0) {
                    tri[0] = {ox, yb}; tri[1] = {ox + tw, yt}; tri[2] = {ox + tw, yb};
                    ax = ox; ay = yb; bx = ox + tw; by = yt;
                } else {
                    tri[0] = {ox, yt}; tri[1] = {ox + tw, yb}; tri[2] = {ox, yb};
                    ax = ox; ay = yt; bx = ox + tw; by = yb;
                }
                chromium_fill_polygon(c, tri, 3, fp);
                chromium_stroke_line(c, ax, ay, bx, by, 2, lp);
                if (!sel) chromium_stroke_line(c, ox, yb - 1, ox + tw, yb - 1, 2, lp);
                c.restore();
            }
        }
        SkColor tc = (i == st.highlighted) ? pal::green : sel ? st.selected_text_color : pal::white;
        TextStyle ts{fs, tc, Face::Mfd, Align::Center};
        float lh = lh_c(ts);
        float ly = bar_top + 2 + 6 + (bh - 2 - 6 - 2 - lh) * 0.5f - 1.33f + (sel ? 1.f : 0.f);
        if (a380::getenv_cached("MFD_TEXTLOG"))
            std::fprintf(stderr, "TAB\t%s\t%d\t%.3f\n", titles[i].c_str(), sel ? 1 : 0, c.getLocalToDeviceAs3x3().mapPoint({0, bar_top}).fY);
        text(c, titles[i], (z_l + z_r) * 0.5f, ly, ts);
    }
    if (st.additional_right_space > 0) hline(c, box.right() - st.additional_right_space, box.right(), bar_bot - 2, pal::light_grey, 2);
    SkRect frame = SkRect::MakeLTRB(box.left(), bar_bot, box.right(), box.bottom());
    // content frame: 2px outset, no top border
    SkColor tl = chromium_light(pal::light_grey), br = chromium_dark(pal::light_grey);
    rect_fill(c, SkRect::MakeLTRB(frame.left(), frame.top(), frame.left() + 2, frame.bottom()), tl);
    rect_fill(c, SkRect::MakeLTRB(frame.right() - 2, frame.top(), frame.right(), frame.bottom()), br);
    rect_fill(c, SkRect::MakeLTRB(frame.left(), frame.bottom() - 2, frame.right(), frame.bottom()), br);
    (void)tl;
    return SkRect::MakeLTRB(frame.left() + 12, frame.top() + 10, frame.right() - 12, frame.bottom() - 12);
}

void radio_button(SkCanvas& c, float x, float top, std::string_view lbl, bool checked, SkColor checked_color,
                  bool disabled, SkColor label_color, float label_dx) {
    Scope scope_(128);
    float cx = x + 2 + 15, cy = top + 2 + 17;
    SkPaint p = fill_paint(pal::background);
    c.drawCircle(cx, cy, 17, p);
    SkPaint ring = fill_paint(chromium_dark(pal::light_grey));
    ring.setStyle(SkPaint::kStroke_Style);
    ring.setStrokeWidth(2);
    c.drawArc(SkRect::MakeXYWH(cx - 16, cy - 16, 32, 32), 135, 180, false, ring);
    ring.setColor(chromium_light(pal::light_grey));
    c.drawArc(SkRect::MakeXYWH(cx - 16, cy - 16, 32, 32), -45, 180, false, ring);
    if (disabled) c.drawCircle(cx, cy, 14, fill_paint(pal::dark_grey));
    else if (checked) c.drawCircle(cx, cy, 11, fill_paint(checked_color));
    SkColor lc = label_color ? label_color : disabled ? pal::dark_grey : checked ? checked_color : pal::white;
    TextStyle ts{26, lc, Face::Mfd, Align::Left};
    text(c, lbl, x + 2 + 34 + 15 + label_dx, cy - lh_c(ts) * 0.5f, ts);
}

void labeled_box(SkCanvas& c, SkRect r, std::string_view lbl) {
    Scope scope_(256);
    stroke_box(c, r, pal::light_grey, 1);
    TextStyle ts{20, pal::white, Face::Mfd, Align::Left};
    float lh = lh_c(ts);
    float w = text_width(lbl, ts) + 15 + 4;
    rect_fill(c, SkRect::MakeXYWH(r.left() + 10, r.top() - 10, w, lh), pal::background);
    text(c, lbl, r.left() + 12, r.top() - 10, ts);
}

void separator(SkCanvas& c, float x0, float x1, float y, SkColor col, float w) { hline(c, x0, x1, y, col, w); }

FplnLineColors fpln_colors(bool secondary, bool temporary, bool altn) {
    FplnLineColors f;
    if (secondary) f.ident = f.annotation = f.small = pal::white;
    if (temporary) f.ident = f.annotation = f.small = pal::yellow;
    if (altn) f.ident = f.annotation = f.small = pal::cyan;
    return f;
}

void fpln_ident(SkCanvas& c, float x, float y, float width, std::string_view s, SkColor color, bool selected) {
    Scope scope_(512);
    SkRect box = SkRect::MakeXYWH(x, y + 32, width, 40);
    if (selected) rect_fill(c, box, pal::dark_grey);
    TextStyle ts{30, selected ? pal::white : color, Face::Mfd, Align::Left};
    text(c, s, x + 2, box.centerY() - lh_c(ts) * 0.5f, ts);
}

void fpln_annotation(SkCanvas& c, float x, float y, float width, std::string_view s, SkColor color) {
    Scope scope_(512);
    TextStyle ts{22, color, Face::Mfd, Align::Right};
    text(c, s, x + width, y + (32 - lh_c(ts)) * 0.5f, ts);
}

void fpln_small_label(SkCanvas& c, SkRect r, std::string_view s, SkColor color, float size) {
    Scope scope_(512);
    TextStyle ts{size, color, Face::Mfd, Align::Center};
    text_in(c, s, r, ts);
}

void amber_message(SkCanvas& c, SkRect r, std::string_view s) {
    Scope scope_(4096);
    TextStyle ts{24, pal::amber, Face::Mfd, Align::Center};
    text_in(c, s, r, ts);
}

void message_box(SkCanvas& c, SkRect r, std::string_view s, SkColor bg, SkColor fg) {
    Scope scope_(1024);
    rect_fill(c, r, bg);
    TextStyle ts{26, fg, Face::Mfd, Align::Left};
    text(c, s, r.left(), r.top(), ts);
}

}  // namespace a380::mfd::ui
