#include "page_model.h"

namespace a380::mfd {
namespace {

using namespace pm;

constexpr float kTop = 131.33f;

SkRect R(SkRect content, float x0, float y0, float x1, float y1) { return box(content, x0, y0, x1, y1); }


void light_line(SkCanvas& c, SkRect content, float y_rel, SkColor color) {
    thin_line(c, content, 0, 768, y_rel + kTop, kTop, color);
}

void draw_datis(SkCanvas& c, SkRect content, const json& page) {
    GreyLayer grey(c);
    const json& blocks = at(page, "blocks");
    json open_menu;
    json type_menu;
    SkRect type_rect = SkRect::MakeEmpty();
    SkRect open_rect;
    for (size_t i = 0; i < blocks.size() && i < 3; i++) {
        const json& b = blocks[i];
        const float y0 = 250.f * i;
        ui::FieldStyle fs;
        fs.kind = ui::FieldKind::Entered;
        fs.font_size = 27;
        fs.min_text_width = 72;
        ui::input_field(c, R(content, 10, y0 + 12, 116, y0 + 52), str(b, "airport"), fs);
        ui::DropdownStyle ds;
        ds.align = ui::Align::Left;
        ui::dropdown(c, R(content, 120, y0 + 10, 226, y0 + 54), str(b, "type"), ds);
        if (flag(at(b, "typeDropdown"), "open")) {
            json d = at(b, "typeDropdown");
            d["open"] = true;
            type_menu = d;
            type_rect = R(content, 120, y0 + 10, 226, y0 + 54);
        }
        light_line(c, content, y0 + 249.67f, ui::pal::light_grey);
        for (const auto& bt : at(b, "buttons")) {
            if (flag(bt, "hidden")) continue;
            const float x = static_cast<float>(at(bt, "x").is_number() ? at(bt, "x").get<float>() : 600.f);
            const bool wide = str(bt, "label") == "UPDATE\nOR PRINT";
            ui::ButtonStyle st = button_style(bt);
            st.arrow = flag(bt, "arrow");
            if (st.state == ui::ButtonState::Highlighted) st.text_color = ui::pal::background;
            st.pad_left = 5;
            st.pad_right = 5;
            st.pad_top = 3;
            st.pad_bottom = 3;
            const float w = x < 600.f ? 131.f : 159.f;
            const float bh = st.state == ui::ButtonState::Highlighted ? 54.f : 48.67f;
            const float btn_dy = st.arrow && st.state != ui::ButtonState::Highlighted ? 0.67f : 0.f;
            SkRect r = R(content, x, y0 + 7 + btn_dy, x + w, y0 + 7 + bh);
            (void)wide;
            if (st.arrow) {
                ui::ButtonStyle bg = st;
                bg.arrow = false;
                ui::button_lines(c, r, {""}, bg);
                ui::TextStyle ts = ui::label_style(st.state == ui::ButtonState::Disabled ? ui::pal::dark_grey : ui::pal::white, 22);
                ts.align = ui::Align::Center;
                const auto lines = split_lines(str(bt, "label"));
                const float lh = ui::line_height(ts);
                float yy = r.centerY() - lh * lines.size() * 0.5f - 0.33f;
                for (auto& l : lines) {
                    ui::text(c, l, r.centerX() - 10.f, yy, ts);
                    yy += lh;
                }
            } else {
                ui::button_lines(c, r, split_lines(str(bt, "label")), st);
            }
            if (st.arrow) {
                ui::triangle_down(c, r.right() - 2 - 5 - 15 + 0.f, r.centerY() - 7.5f,
                                  st.state == ui::ButtonState::Disabled ? 0xFF808080 : ui::pal::white);
            }
            if (at(bt, "items").is_array() && !at(bt, "items").empty()) {
                open_menu = at(bt, "items");
                open_rect = R(content, x - 20, y0 + 7 + bh, x + w, y0 + 7 + bh + 275);
            }
        }
    }
    if (type_menu.is_object()) dropdown_open_overlay(c, type_rect, type_menu);
    if (open_menu.is_array()) {
        std::vector<ui::MenuItem> items;
        for (const auto& it : open_menu) items.push_back({str(it, "text"), flag(it, "disabled")});
        ui::dropdown_menu(c, open_rect, items);
    }
    const json& foot = at(page, "footer");
    for (size_t i = 0; i < foot.size() && i < 2; i++) {
        if (flag(foot[i], "hidden")) continue;
        const float x1 = 768.f - 190.f * (1 - i);
        ui::ButtonStyle st = button_style(foot[i]);
        ui::button_lines(c, R(content, x1 - 190, 753.f, x1, 753.f + ui::button_height(22, 2)),
                         split_lines(str(foot[i], "label")), st);
    }
}

void draw_request(SkCanvas& c, SkRect content, const json& page) {
    GreyLayer grey(c);
    const float tops[7] = {10.f, 47.33f, 84.67f, 122.f, 159.33f, 216.f, 293.33f};
    const float hs[7] = {37.33f, 37.33f, 37.33f, 37.33f, 56.67f, 37.33f, 37.33f};
    const json& menu = at(page, "menu");
    for (size_t i = 0; i < menu.size() && i < 7; i++) {
        SkRect r = R(content, 578, tops[i], 758, tops[i] + hs[i]);
        ui::ButtonStyle st;
        ui::button_lines(c, r, {""}, st);
        ui::triangle_left(c, r.left() + 2 + 12, r.centerY() - 6.3f, ui::pal::white);
        ui::TextStyle ts = ui::label_style(ui::pal::white, 22);
        ts.align = ui::Align::Right;
        const auto lines = split_lines(menu[i].get<std::string>());
        const float lh = ui::line_height(ts);
        float y = r.centerY() - lh * lines.size() * 0.5f - 0.67f;
        for (auto& l : lines) {
            ui::text(c, l, r.right() - 2 - 9.7f, y + 2.4f, ts);
            y += lh;
        }
    }
    const json& foot = at(page, "footer");
    for (size_t i = 0; i < foot.size() && i < 2; i++) {
        if (flag(foot[i], "hidden")) continue;
        const float x0 = i == 0 ? 0.f : 578.f;
        ui::ButtonStyle st = button_style(foot[i]);
        std::string label = str(foot[i], "label");
        auto lines = split_lines(label);
        for (auto& l : lines) {
            while (!l.empty() && l[0] == ' ') l.erase(0, 1);
        }
        ui::button_lines(c, R(content, x0, 761.33f, x0 + 190, 825.33f), lines, st);
    }
}

void draw_msg_record(SkCanvas& c, SkRect content, const json& page) {
    GreyLayer grey(c);
    ui::fill(c, R(content, 0, 90, 768, 91), ui::pal::white);
    ui::fill(c, R(content, 0, 364.67f, 768, 365.33f), ui::pal::white);
    const json& bs = at(page, "buttons");
    const float ys[2] = {141.f, 256.f};
    for (size_t i = 0; i < bs.size() && i < 2; i++) {
        ui::ButtonStyle st = button_style(bs[i]);
        ui::button(c, R(content, 65, ys[i], 270, ys[i] + 64), str(bs[i], "label"), st);
    }
}

float lh_dev(float size) {
    const float sz = size * 1.5f;
    return (std::round(0.8025f * sz) + std::round(0.0901f * sz)) / 1.5f;
}

void connect_adsc(SkCanvas& c, SkRect r, bool armed) {
    ui::bevel_border(c, r, ui::Bevel::Inset, ui::pal::light_grey, 4);
    SkRect in = r.makeInset(4, 4);
    in.inset(5, 5);
    const float h1 = lh_dev(20), h2 = lh_dev(24);
    const float top = in.centerY() - (h1 + h2) * 0.5f;
    ui::TextStyle up{20, armed ? ui::pal::green : ui::pal::dark_grey, ui::Face::Ecam, ui::Align::Center};
    ui::text(c, "ARMED", in.centerX(), top, up);
    ui::TextStyle lo = ui::label_bigger_style(armed ? ui::pal::dark_grey : ui::pal::white);
    lo.align = ui::Align::Center;
    ui::text(c, armed ? "" : "OFF", in.centerX(), top + h1, lo);
}

void draw_connect(SkCanvas& c, SkRect content, const json&) {
    content.offset(0, -0.67f);
    const float l22 = lh_dev(22), v25 = lh_dev(25);
    ui::TextStyle lab = ui::label_style(ui::pal::white, 22);
    lab.align = ui::Align::Right;
    ui::TextStyle val = ui::value_style(ui::pal::green, 25);
    const float dd_h = 44.67f;
    const float ly = -0.67f;
    const float row1 = 30.f;
    ui::text_in_aligned(c, "NOTIFY TO ATC :", R(content, 0, row1 + ly + 0.33f, 285, row1 + dd_h + ly + 0.33f), lab);
    ui::DropdownStyle ds;
    ds.text_color = ui::pal::cyan;
    ui::dropdown(c, R(content, 305, row1, 455, row1 + dd_h), "----", ds);
    ui::ButtonStyle dis;
    dis.state = ui::ButtonState::Disabled;
    const float bh = ui::button_height(22, 1);
    ui::button(c, R(content, 555, row1 + (dd_h - bh) * 0.5f, 675, row1 + (dd_h + bh) * 0.5f), "NOTIFY", dis);

    const float row2 = row1 + dd_h + 10;
    ui::TextStyle left = lab;
    left.align = ui::Align::Left;
    ui::text(c, "NOTIFY", content.left() + 305, content.top() + row2 + ly + 0.33f, left);

    const float sec = row2 + l22 + 60;
    const float bh2 = ui::button_height(22, 2);
    const float col3 = 20 + bh + 30 + bh2;
    const float shrink = 28.f / 796.f;
    const float x3 = 768.f - 296.f * (1 - shrink);
    const float w3 = 296.f * (1 - shrink);
    const float bx = x3 + (w3 - 260) * 0.5f;
    for (int i = 0; i < 2; i++) {
        const float top = sec + i * (v25 + 30);
        ui::text_in_aligned(c, i ? "NEXT ATC :" : "ACTIVE ATC :", R(content, 0, top + ly, 285, top + v25 + ly), lab);
        ui::text(c, "XXXX", content.left() + 305, content.top() + top - 0.67f, val);
    }
    ui::button(c, R(content, bx, sec + 20, bx + 260, sec + 20 + bh), "DISCONNECT ALL", dis);
    ui::button_lines(c, R(content, bx, sec + 20 + bh + 30, bx + 260, sec + col3), {"MODIFY", "MAX UPLINK DELAY"}, dis);
    const float sec_end = sec + col3 + 20;
    ui::fill(c, R(content, 0, sec_end, 768, sec_end + 2), ui::pal::light_grey);

    const float adsc = sec_end + 2 + 30;
    ui::TextStyle mid = ui::label_style(ui::pal::white, 22);
    mid.align = ui::Align::Center;
    ui::text(c, "ADS-C", content.left() + 84 + 150, content.top() + adsc + ly, mid);
    ui::text(c, "ADS-C EMERGENCY", content.left() + 84 + 450, content.top() + adsc + ly, mid);
    const float btop = adsc + l22 + 10;
    connect_adsc(c, R(content, 84 + 50, btop, 84 + 250, btop + 70), false);
    connect_adsc(c, R(content, 84 + 395, btop, 84 + 505, btop + 70), false);

    const float gs = btop + 70 + 30;
    ui::text(c, "ADS-C CONNECTED GROUND STATIONS :", content.left() + 110, content.top() + gs + ly, left);
    const float nx = 110 + ui::text_width("ADS-C CONNECTED GROUND STATIONS :", left) + 23;
    ui::TextStyle none = ui::label_style(ui::pal::white, 20);
    ui::text(c, "NONE", content.left() + nx, content.top() + gs + 0.67f, none);
    const float lt = gs + l22 + 10;
    const float lx = (768.f - 386.f) * 0.5f + 26;
    ui::stroke_box(c, R(content, lx, lt, lx + 360, lt + 265), ui::pal::light_grey, 2);

    ui::fill(c, R(content, 0, 0.67f, 768, 818.67f), 0xB3000000);
    ui::TextStyle amber{35, 0xFFE68000, ui::Face::Mfd, ui::Align::Center};
    ui::text_in(c, "NOT YET IMPLEMENTED", R(content, 0, 0.67f, 768, 818.67f), amber);
}

void draw_atccom(SkCanvas& c, SkRect content, const json& page) {
    const std::string v = str(page, "view");
    if (v == "connect") draw_connect(c, content, page);
    else if (v == "datis") draw_datis(c, content, page);
    else if (v == "request") draw_request(c, content, page);
    else if (v == "msgrecord") draw_msg_record(c, content, page);
}

}  // namespace

A380_MFD_PAGE("atccom/", draw_atccom);

}  // namespace a380::mfd
