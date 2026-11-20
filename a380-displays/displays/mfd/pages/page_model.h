#pragma once
#include <cmath>
#include <algorithm>

#include <string>
#include <vector>

#include "include/core/SkCanvas.h"
#include "include/core/SkPaint.h"
#include "include/core/SkRRect.h"
#include "include/core/SkRect.h"

#include "nlohmann/json.hpp"

#include "../mfd_pages.h"
#include "../widgets.h"
#include "pp_layout.h"

namespace a380::mfd::pm {

using json = nlohmann::json;
namespace ui = a380::mfd::ui;

struct GreyLayer {
    explicit GreyLayer(SkCanvas&) {}
};

inline const json& at(const json& j, const char* key) {
    static const json none = json::object();
    if (!j.is_object()) return none;
    auto it = j.find(key);
    return it == j.end() ? none : *it;
}

inline std::string str(const json& j, const char* key, const std::string& def = {}) {
    const json& v = at(j, key);
    return v.is_string() ? v.get<std::string>() : def;
}

inline bool flag(const json& j, const char* key, bool def = false) {
    const json& v = at(j, key);
    return v.is_boolean() ? v.get<bool>() : def;
}

inline ui::FieldStyle field_style(const json& f) {
    ui::FieldStyle st;
    const std::string kind = str(f, "kind", "entered");
    if (kind == "mandatory") st.kind = ui::FieldKind::Mandatory;
    else if (kind == "computed") st.kind = ui::FieldKind::Computed;
    else if (kind == "disabled") st.kind = ui::FieldKind::Disabled;
    else if (kind == "inactive") st.kind = ui::FieldKind::Inactive;
    else if (kind == "validating") st.kind = ui::FieldKind::Validating;
    else if (kind == "selected") st.kind = ui::FieldKind::Selected;
    else if (kind == "editing") st.kind = ui::FieldKind::Editing;
    else if (kind == "tmpy") st.kind = ui::FieldKind::Tmpy;
    st.unit_leading = str(f, "unit_leading");
    st.unit_trailing = str(f, "unit_trailing");
    for (const auto& k : at(f, "classes"))
        if (k.is_string() && k.get<std::string>() == "computedByFms") st.font_size = 22;
    if (st.kind == ui::FieldKind::Computed) {
        st.font_size = 22;
        st.text_color = ui::pal::cyan;
    }
    if (st.kind == ui::FieldKind::Inactive) st.border = false;
    return st;
}

inline void field(SkCanvas& c, SkRect rect, const json& f, ui::Align align = ui::Align::Center, float min_text = 0,
                  float text_dy = 0) {
    ui::FieldStyle st = field_style(f);
    st.text_dy = text_dy;
    st.align = align;
    st.min_text_width = min_text;
    ui::input_field(c, rect, str(f, "text"), st);
}

inline ui::ButtonStyle button_style(const json& b) {
    ui::ButtonStyle st;
    const std::string s = str(b, "state", "normal");
    if (s == "disabled") st.state = ui::ButtonState::Disabled;
    else if (s == "selected") st.state = ui::ButtonState::Selected;
    else if (s == "highlighted") st.state = ui::ButtonState::Highlighted;
    st.asterisk = flag(b, "asterisk");
    return st;
}

inline void dropdown_box(SkCanvas& c, SkRect r, const json& d) {
    const std::string text = str(d, "text");
    if (!flag(d, "disabled")) {
        ui::DropdownStyle ds;
        ds.inactive = flag(d, "inactive");
        ui::dropdown(c, r, text, ds);
        return;
    }
    ui::fill(c, r, ui::pal::darker_grey);
    ui::bevel_border(c, r, ui::Bevel::Outset, ui::pal::light_grey);
    SkRect inner = r.makeInset(2, 2);
    inner.fRight -= 25;
    ui::fill(c, inner, ui::pal::darker_grey);
    ui::triangle_down(c, r.right() - 2 - 25 + 5, r.centerY() - 7.5f, 0xFF808080);
    ui::TextStyle ts = ui::label_style(ui::pal::dark_grey, 27);
    ts.align = ui::Align::Center;
    ui::text_in_aligned(c, text, inner, ts, 10);
}

inline void dropdown_open_overlay(SkCanvas& c, SkRect r, const json& d) {
    if (!flag(d, "open")) return;
    const std::string text = str(d, "text");
    ui::TextStyle ts = ui::label_style(ui::pal::background, 27);
    ts.align = ui::Align::Center;
    SkRect inner = r.makeInset(2, 2);
    inner.fRight -= 25;
    const float tw = ui::text_width(text, ts);
    SkRect sel = SkRect::MakeLTRB(inner.centerX() - tw * 0.5f - 2.2f, inner.centerY() - 15, inner.centerX() + tw * 0.5f + 2.2f, inner.centerY() + 15);
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(ui::pal::white);
    c.drawRRect(SkRRect::MakeRectXY(sel, 5, 5), p);
    p.setColor(ui::pal::cyan);
    c.drawRRect(SkRRect::MakeRectXY(sel.makeInset(2, 2), 3, 3), p);
    ui::text_in(c, text, inner, ts);
    std::vector<ui::MenuItem> items;
    for (const auto& it : at(d, "items"))
        items.push_back({str(it, "text"), flag(it, "disabled")});
    ui::dropdown_menu(c, SkRect::MakeLTRB(r.left(), r.bottom(), r.right(), r.bottom() + 275), items);
}

inline std::vector<std::string> split_lines(const std::string& s) {
    std::vector<std::string> out;
    size_t a = 0;
    for (;;) {
        size_t b = s.find('\n', a);
        out.push_back(s.substr(a, b == std::string::npos ? b : b - a));
        if (b == std::string::npos) break;
        a = b + 1;
    }
    return out;
}

inline void thin_line(SkCanvas& c, SkRect content, float x0, float x1, float y_abs, float page_top = 131.33f,
                      SkColor color = ui::pal::light_grey) {
    const float s = c.getTotalMatrix().getScaleY();
    const float row = std::round((content.top() + y_abs - page_top) * s);
    ui::fill(c, SkRect::MakeLTRB(content.left() + x0, row / s, content.left() + x1, (row + 1) / s), color);
}

inline void button(SkCanvas& c, SkRect rect, const json& b, float pad_right = 12, float text_dy = 0) {
    if (flag(b, "hidden")) return;
    const std::string label = str(b, "label");
    ui::ButtonStyle st = button_style(b);
    if (label.find('\n') == std::string::npos && !st.asterisk) {
        ui::button(c, rect, label, st);
        return;
    }
    if (!st.asterisk) {
        ui::button_lines(c, rect, split_lines(label), st);
        return;
    }
    ui::ButtonStyle bg = st;
    bg.asterisk = false;
    ui::button(c, rect, "", bg);
    const auto lines = split_lines(label);
    SkColor col = st.state == ui::ButtonState::Disabled ? ui::pal::dark_grey : ui::pal::white;
    ui::TextStyle ts = ui::label_style(col, 22);
    ts.align = ui::Align::Left;
    const float lh = ui::line_height(ts);
    float tw = 0;
    for (auto& l : lines) tw = std::max(tw, ui::text_width(l, ts));
    const float sw = ui::text_width("*", ts);
    const float cl = rect.left() + 2 + st.pad_left, cr = rect.right() - 2 - pad_right;
    const float ct = rect.top() + 2 + st.pad_top, cb = rect.bottom() - 2 - st.pad_bottom;
    const float left = cl + ((cr - cl) - (tw + 10 + sw)) * 0.5f;
    float y = (ct + cb) * 0.5f - lh * lines.size() * 0.5f + text_dy;
    for (auto& l : lines) {
        ts.align = ui::Align::Center;
        ui::text(c, l, left + tw * 0.5f, y, ts);
        y += lh;
    }
    ts.align = ui::Align::Left;
    ui::text(c, "*", left + tw + 10 + 1.4f, (ct + cb) * 0.5f - lh * 0.5f + text_dy, ts);
}

inline void label_right(SkCanvas& c, std::string_view s, float right, const SkRect& row, SkColor color = ui::pal::white,
                        float size = 20) {
    ui::TextStyle ts = ui::label_style(color, size);
    ts.align = ui::Align::Right;
    ui::text_in_aligned(c, s, SkRect::MakeLTRB(right - 400, row.top(), right, row.bottom()), ts);
}

inline void label_left(SkCanvas& c, std::string_view s, float left, const SkRect& row, SkColor color = ui::pal::white,
                       float size = 20) {
    ui::TextStyle ts = ui::label_style(color, size);
    ts.align = ui::Align::Left;
    ui::text_in_aligned(c, s, SkRect::MakeLTRB(left, row.top(), left + 600, row.bottom()), ts);
}

inline SkRect box(SkRect content, float x0, float y0, float x1, float y1) {
    return SkRect::MakeLTRB(content.left() + x0, content.top() + y0, content.left() + x1, content.top() + y1);
}

}  // namespace a380::mfd::pm
