#pragma once

#include "page_model.h"

namespace a380::mfd::proc {

using namespace pm;

constexpr float kTop = 131.33f;

struct Cell {
    const char* label;
    float x;
    float label_cy;
    float value_cy;
};

inline SkColor value_color(const json& v) {
    for (const auto& k : at(v, "classes")) {
        const std::string s = k.get<std::string>();
        if (s == "tmpy") return ui::pal::yellow;
        if (s == "white") return ui::pal::white;
        if (s == "cyan") return ui::pal::cyan;
        if (s == "amber") return ui::pal::amber;
        if (s == "magenta") return ui::pal::magenta;
    }
    return ui::pal::green;
}

inline void put_text(SkCanvas& c, SkRect content, std::string_view s, float x, float cy, const ui::TextStyle& ts) {
    const float y = content.top() + cy - kTop;
    ui::text_in_aligned(c, s, SkRect::MakeLTRB(content.left() + x, y - 20, content.left() + x + 400, y + 20), ts);
}

inline void cell(SkCanvas& c, SkRect content, const json& fields, const Cell& cell, const char* key = nullptr,
                 float value_x = -1, float value_cy = -1) {
    put_text(c, content, cell.label, cell.x, cell.label_cy, ui::label_style());
    const json& v = at(fields, key ? key : cell.label);
    if (!v.is_object()) return;
    const float vx = value_x >= 0 ? value_x : cell.x;
    const float vcy = value_cy >= 0 ? value_cy : cell.value_cy;
    const std::string unit = str(v, "unit_trailing");
    const SkColor col = value_color(v);
    const ui::TextStyle vs = ui::value_style(col), us = ui::unit_style();
    const float lh = std::max(ui::line_height(vs), ui::line_height(us));
    ui::value_with_unit(c, str(v, "text"), unit, content.left() + vx, content.top() + vcy - kTop - lh * 0.5f, col, false);
}

inline void box_frame(SkCanvas& c, SkRect content, const json& page, const char* title) {
    (void)page;
    const SkRect r = box(content, 0, 151.17f - kTop, 768, 293 - kTop);
    ui::stroke_box(c, r, ui::pal::light_grey, 1);
    const ui::TextStyle ts = ui::label_style(ui::pal::white, 20);
    const float tw = ui::text_width(title, ts);
    ui::fill(c, SkRect::MakeXYWH(r.left() + 10, r.top() - 10, tw + 5.33f, ui::line_height(ts)), ui::pal::background);
    ui::text(c, title, r.left() + 12.67f, r.top() - 9.33f, ts);
}

inline void dropdown_button(SkCanvas& c, SkRect content, const json& b, float x, float w) {
    ui::ButtonStyle st = button_style(b);
    st.arrow = true;
    const SkRect r = box(content, x, 312.67f - kTop, x + w, 350 - kTop);
    ui::button(c, r, "", st);
    ui::TextStyle ts = ui::label_style(st.state == ui::ButtonState::Disabled ? ui::pal::dark_grey : ui::pal::white, 22);
    ts.align = ui::Align::Center;
    const float lh = ui::line_height(ts);
    ui::text(c, str(b, "label"), r.centerX() - 10.33f, (r.top() + 2 + 9 + r.bottom() - 2 - 5) * 0.5f - lh * 0.5f, ts);
}

inline void open_menu(SkCanvas& c, SkRect content, const json& b, float x, float w) {
    if (!flag(b, "open")) return;
    std::vector<ui::MenuItem> items;
    for (const auto& it : at(b, "items")) items.push_back({str(it, "text"), flag(it, "disabled")});
    ui::dropdown_menu(c, box(content, x, 350 - kTop, x + w, 350 - kTop + 275), items);
}

inline void return_button(SkCanvas& c, SkRect content, const json& page) {
    const json& b = at(page, "return");
    if (!b.is_object() || flag(b, "hidden")) return;
    const std::string label = str(b, "label");
    const float w = ui::text_width(label, ui::label_style(ui::pal::white, 22)) + 28;
    const SkRect rr = box(content, 2, 920.0f - kTop, 2 + w, 957.33f - kTop);
    ui::button(c, rr, label, button_style(b));
}

inline void tmpy_button(SkCanvas& c, SkRect content, const json& page) {
    const json& b = at(page, "tmpy_fpln");
    if (!b.is_object() || flag(b, "hidden")) return;
    ui::ButtonStyle st = button_style(b);
    if (flag(b, "yellow")) st.text_color = ui::pal::yellow;
    const std::string label = str(b, "label");
    const float w = ui::text_width(label, ui::label_style(ui::pal::white, 22)) + 28;
    const SkRect rr = box(content, 766 - w, 920.0f - kTop, 766, 957.33f - kTop);
    ui::button(c, rr, label, st);
}

}  // namespace a380::mfd::proc
