#include <algorithm>
#include <cstdlib>
#include <string>
#include <vector>

#include "include/core/SkPaint.h"
#include "include/core/SkPathBuilder.h"

#include "page_model.h"

namespace a380::mfd {
namespace {

using namespace pm;
using ui::Align;

constexpr float W = 768;
constexpr float kColIdent = 0.25f * W;
constexpr float kColTime = 0.115f * W;
constexpr float kColSpeed = 0.15f * W;
constexpr float kColAlt = 0.21f * W;
constexpr float kColConn = kColIdent + kColTime + kColSpeed + kColAlt;
constexpr float kColTrk = kColConn + 35;
constexpr float kColDist = kColTrk + 0.09f * W;
constexpr float kColFpa = kColDist + 0.06f * W;
constexpr float kLine = 72;

SkColor parse_color(const std::string& s, SkColor def) {
    if (s.size() == 7 && s[0] == '#') return 0xFF000000u | (SkColor)std::strtoul(s.c_str() + 1, nullptr, 16);
    if (s == "#000") return ui::pal::background;
    if (s == "#fff") return ui::pal::white;
    return def;
}

float text_h(float size) {
    ui::TextStyle ts;
    ts.size = size;
    return ui::line_height(ts);
}

void put(SkCanvas& c, const std::string& s, float x, float box_top, float box_h, float size, SkColor color,
         Align a = Align::Left) {
    if (s.empty()) return;
    ui::TextStyle ts;
    ts.size = size;
    ts.color = color;
    ts.align = a;
    ui::text(c, s, x, box_top + (box_h - ui::line_height(ts)) * 0.5f, ts);
}

void put_top(SkCanvas& c, const std::string& s, float x, float top, float size, SkColor color, Align a) {
    if (s.empty()) return;
    ui::TextStyle ts;
    ts.size = size;
    ts.color = color;
    ts.align = a;
    ui::text(c, s, x, top, ts);
}

float tw(const std::string& s, float size) {
    ui::TextStyle ts;
    ts.size = size;
    return ui::text_width(s, ts);
}

SkPaint stroke_paint(SkColor col) {
    SkPaint p;
    p.setColor(col);
    p.setStyle(SkPaint::kStroke_Style);
    p.setStrokeWidth(2);
    p.setAntiAlias(true);
    return p;
}

void seg(SkCanvas& c, float ox, float oy, float x0, float y0, float x1, float y1, const SkPaint& p) {
    c.drawLine(ox + x0, oy + y0, ox + x1, oy + y1, p);
}

void diamond(SkCanvas& c, float ox, float oy, const SkPaint& p) {
    SkPathBuilder path;
    path.moveTo(ox + 15, oy + 63);
    path.lineTo(ox + 8, oy + 51);
    path.lineTo(ox + 15, oy + 37);
    path.lineTo(ox + 22, oy + 51);
    path.close();
    c.drawPath(path.detach(), p);
}

void tail_gap(SkCanvas& c, float ox, float oy) {
    SkPaint p;
    p.setColor(ui::pal::background);
    c.drawRect(SkRect::MakeLTRB(ox + 13, oy + 68.67f, ox + 17, oy + 72.67f), p);
}

void connector(SkCanvas& c, float ox, float oy, const json& k, SkColor line_color) {
    const std::string kind = str(k, "kind");
    const SkColor col = parse_color(str(k, "color"), line_color);
    const bool end_black = flag(k, "end_black");
    const bool active = flag(k, "active");
    const SkColor tail = end_black ? ui::pal::background : col;
    if (kind == "first") {
        SkPaint pt = stroke_paint(tail);
        seg(c, ox, oy, 15, 40, 15, 30, pt);
        const bool green = col == ui::pal::green;
        SkPaint ps = stroke_paint(active && green ? ui::pal::white : col);
        seg(c, ox, oy, 8, 18, 0, 18, ps);
        SkPathBuilder path;
        SkPaint pp = stroke_paint(active && col != ui::pal::yellow ? ui::pal::white : col);
        path.moveTo(ox + 15, oy + 31);
        path.lineTo(ox + 8, oy + 19);
        path.lineTo(ox + 15, oy + 5);
        path.lineTo(ox + 22, oy + 19);
        path.close();
        c.drawPath(path.detach(), pp);
        return;
    }
    if (kind == "hold") {
        seg(c, ox, oy, 15, 72, 15, 0, stroke_paint(col));
        return;
    }
    if (kind == "pseudo") {
        seg(c, ox, oy, 15, 72, 15, 50, stroke_paint(tail));
        tail_gap(c, ox, oy);
        SkPaint p = stroke_paint(col);
        seg(c, ox, oy, 15, 0, 15, 50, p);
        seg(c, ox, oy, 15, 50, 0, 50, p);
        seg(c, ox, oy, 15, 10, 30, 10, p);
        return;
    }
    if (kind == "active_leg") {
        seg(c, ox, oy, 15, 72, 15, 62, stroke_paint(col));
        tail_gap(c, ox, oy);
        SkPaint p = stroke_paint(col == ui::pal::green ? ui::pal::white : col);
        seg(c, ox, oy, 15, 9, 15, 37, p);
        diamond(c, ox, oy, p);
        seg(c, ox, oy, 8, 50, 0, 50, p);
        seg(c, ox, oy, 15, 10, 30, 10, p);
        return;
    }
    seg(c, ox, oy, 15, 72, 15, 63, stroke_paint(tail));
    tail_gap(c, ox, oy);
    SkPaint p = stroke_paint(col);
    if (kind == "normal") {
        seg(c, ox, oy, 15, 0, 15, 37, p);
        seg(c, ox, oy, 15, 10, 30, 10, p);
    }
    diamond(c, ox, oy, p);
    seg(c, ox, oy, 8, 50, 0, 50, p);
}

void field_cell(SkCanvas& c, const json& f, float content_x, float row_top, SkColor color, bool is_alt) {
    const std::string text = str(f, "text");
    if (text.empty() && str(f, "cstr").empty()) return;
    const std::string cstr = str(f, "cstr");
    const bool indent = flag(f, "indent");
    float x = content_x;
    if (cstr == "respected" || cstr == "missed") {
        const SkColor cc = cstr == "respected" ? ui::pal::magenta : ui::pal::amber;
        put(c, "*", x + 10, row_top - 1, 40, 30, cc, Align::Center);
        x += 20;
        put(c, text, x, row_top - 2, 40, 22, color);
        return;
    }
    if (cstr == "only") {
        if (indent) x += 20;
        put(c, text, x, row_top - 2, 40, flag(f, "small") ? 25 : 22, ui::pal::magenta);
        return;
    }
    (void)is_alt;
    if (indent) x += 20;
    put(c, text, x, row_top - 2, 40, 22, color);
}

void draw_leg(SkCanvas& c, float ox, float y, const json& l) {
    const bool first = flag(l, "first");
    const SkColor lc = parse_color(str(l, "color"), ui::pal::green);
    const bool active = flag(l, "active");
    const bool tmpy = lc == ui::pal::yellow;
    const SkColor field_color = (active && !tmpy && lc == ui::pal::green) ? ui::pal::white : lc;
    const SkColor ident_color = field_color;
    const SkColor ann_color = lc == ui::pal::green ? ui::pal::white : lc;
    const float h = first ? 40 : kLine;
    const float lower = first ? y : y + 32;
    const float lower_h = 40;
    const float upper = y;

    if (!first) put_top(c, str(l, "annotation"), ox + kColIdent - 0.13f, upper - 0.67f, 22, ann_color, Align::Right);
    const std::string ident = str(l, "ident");
    if (flag(l, "selected")) {
        float iw = std::max(tw(ident, 30) + 4, 4.f);
        c.drawRect(SkRect::MakeXYWH(ox, lower, iw, lower_h), [] { SkPaint p; p.setColor(ui::pal::dark_grey); return p; }());
    }
    put(c, ident, ox + 2, lower, lower_h, 30, flag(l, "selected") ? ui::pal::white : ident_color);
    if (flag(l, "overfly")) put(c, "@", ox + 2 + tw(ident, 30), lower, lower_h, 24, ident_color);

    put(c, str(l, "time_annotation"), ox + kColIdent + kColTime * 0.5f, upper + 1.3f, 32, 22, field_color, Align::Center);
    put(c, str(l, "time"), ox + kColIdent + kColTime * 0.5f - 0.16f, lower - 2 + (first ? 1.33f : 0.f), lower_h, 22, field_color, Align::Center);

    const float sx = ox + kColIdent + kColTime;
    const float ax = sx + kColSpeed;
    if (l.contains("wind") || l.contains("efob")) {
        put(c, str(l, "efob"), sx + 12, lower - 2, lower_h, 22, field_color);
        const json& w = at(l, "wind");
        if (w.is_object()) {
            const float right = ax + kColAlt - 22;
            const float slash = tw("/", 22);
            const float wtot = 45 + slash + 45;
            const float x0 = right - wtot;
            put(c, str(w, "dir"), x0 + 22.5f, lower - 2, lower_h, 22, field_color, Align::Center);
            put(c, "/", x0 + 45, lower - 2, lower_h, 22, field_color);
            put(c, str(w, "spd"), x0 + 45 + slash + 22.5f, lower - 2, lower_h, 22, field_color, Align::Center);
        } else {
            const float right = ax + kColAlt - 22;
            put(c, "---°/---", right, lower - 2, lower_h, 22, field_color, Align::Right);
        }
    } else {
        field_cell(c, at(l, "speed"), sx + 42 - 0.17f, lower + (first ? 1.33f : 0.f), field_color, false);
        field_cell(c, at(l, "alt"), ax + 22 - 0.20f, lower + (first ? 1.33f : 0.f), field_color, true);
    }

    const json& k = at(l, "connector");
    if (k.is_object()) connector(c, ox + kColConn - 0.1f, y + 1.71f, k, lc);

    if (!first) {
        put_top(c, str(l, "trk"), ox + kColTrk + 1.68f, upper + 1.3f, 22, field_color, Align::Left);
        put_top(c, str(l, "dist"), ox + kColDist + 0.06f * W - 2.36f, upper + 1.3f, 22, field_color, Align::Right);
        put_top(c, str(l, "fpa"), ox + kColFpa + 0.08f * W - 2.36f, upper + 1.3f, 22, field_color, Align::Right);
    }
    (void)h;
}

void draw_special(SkCanvas& c, float ox, float y, const json& l) {
    const bool first = flag(l, "first");
    const std::string label = str(l, "label");
    const std::string delim = label.size() > 13 ? "- - - - -" : "- - - - - - ";
    ui::TextStyle ts;
    ts.size = 30;
    ts.color = parse_color(str(l, "color"), ui::pal::white);
    ts.color = ui::pal::white;
    const float dw = ui::text_width(delim, ts);
    const float lw = ui::text_width(label, ts);
    const float total = dw * 2 + lw + 30;
    float x = ox + (W - total) * 0.5f;
    const float h = first ? 40 : kLine;
    const float top = y + (h - ui::line_height(ts)) * 0.5f;
    ui::text(c, delim, x + (label.size() > 13 ? 0.f : ui::text_width(" ", ts)), top, ts);
    ui::text(c, label, x + dw + 15, top, ts);
    ui::text(c, delim, x + dw + 30 + lw, top, ts);
}

float btn_w(const std::string& s, float size) { return tw(s, size) + 28; }

float btn_h(float size) { return 22 + 0.71f * size; }

void double_chevron(SkCanvas& c, SkRect r, bool down, SkColor col) {
    const float cx = r.centerX() - 17.5f, cy = r.centerY() - 17.5f;
    SkPaint p;
    p.setColor(col);
    p.setAntiAlias(true);
    c.save();
    if (down) c.rotate(180, r.centerX(), r.centerY());
    SkPathBuilder a, b;
    a.moveTo(cx, cy + 17.5f);
    a.lineTo(cx + 17.5f, cy);
    a.lineTo(cx + 35, cy + 17.5f);
    a.close();
    b.moveTo(cx, cy + 35);
    b.lineTo(cx + 17.5f, cy + 17.5f);
    b.lineTo(cx + 35, cy + 35);
    b.close();
    c.drawPath(a.detach(), p);
    c.drawPath(b.detach(), p);
    c.restore();
}

void draw_dest_line(SkCanvas& c, float ox, float top, const json& d) {
    ui::hline(c, ox, ox + W, top, ui::pal::light_grey, 1);
    const float y = top + 0.33f, h = 70;
    const std::string ident = str(d, "ident");
    const bool has_btn = flag(d, "button", true);
    const float w1 = has_btn ? 155.f : 153.f;
    const std::string time = str(d, "time");
    const std::string efob = str(d, "efob"), eu_full = str(d, "efob_unit");
    const std::string eu = efob.rfind("-", 0) == 0 ? "" : eu_full;
    const std::string dist = str(d, "dist"), du_full = str(d, "dist_unit");
    const std::string du = dist.rfind("-", 0) == 0 ? "" : du_full;
    const float w2 = tw(time, 20);
    const float w3 = 14 + tw(efob, 20) + 6 + tw(eu_full, 20);
    const float w4 = 14 + tw(dist, 20) + 6 + tw(du_full, 20);
    const bool has_dest_btn = flag(d, "dest_button", true);
    const std::string dbt = str(d, "dest_button_text", "DEST");
    const float dbw = dbt == "DEST" ? 83.f : btn_w(dbt, 22);
    const float w5 = 120 + (has_dest_btn ? dbw + 5 : 0);
    const float gap = (W - (w1 + w2 + w3 + w4 + w5)) / 4;
    float x = ox;
    if (has_btn) {
        ui::ButtonStyle st;
        st.font_size = 30;
        const float bh = btn_h(30);
        ui::button(c, SkRect::MakeXYWH(x, y + (h - bh) * 0.5f, 150, bh), ident, st);
    } else {
        put(c, ident, x + 75, y, h, 20, ui::pal::green, Align::Center);
    }
    x += w1 + gap;
    put(c, time, x, y, h, 20, ui::pal::white);
    x += w2 + gap;
    put(c, efob, x + 7, y, h, 20, ui::pal::white);
    put(c, eu, x + 7 + tw(efob, 20) + 6, y, h, 20, ui::pal::unit_blue);
    x += w3 + gap;
    put(c, dist, x + 7, y, h, 20, ui::pal::white);
    put(c, du, x + 7 + tw(dist, 20) + 6, y, h, 20, ui::pal::unit_blue);
    x += w4 + gap;
    const float by = y + 5;
    SkRect r1 = SkRect::MakeXYWH(x, by, 60, 60), r2 = SkRect::MakeXYWH(x + 60, by, 60, 60);
    const bool down = flag(d, "down_enabled"), up = flag(d, "up_enabled");
    ui::icon_button(c, r1, !down);
    double_chevron(c, r1, true, down ? ui::pal::white : 0xFF808080);
    ui::icon_button(c, r2, !up);
    double_chevron(c, r2, false, up ? ui::pal::white : 0xFF808080);
    if (has_dest_btn) {
        ui::ButtonStyle st;
        const float bh = 60;
        ui::button(c, SkRect::MakeXYWH(x + 120, by, dbw, bh), dbt, st);
    }
}

void draw_footer(SkCanvas& c, float ox, float top, const json& f, bool tmpy_info) {
    ui::hline(c, ox, ox + W, top, ui::pal::light_grey, 1);
    const float by = top + 10.33f;
    const float bh = btn_h(22);
    const std::string left = str(f, "left");
    if (!left.empty()) ui::button(c, SkRect::MakeXYWH(ox, by, 125, bh), left, {});
    const std::string info = str(f, "info");
    if (!info.empty()) {
        const float iw = tw(info, 22) + 58;
        ui::ButtonStyle st;
        st.arrow = true;
        st.arrow_up = !flag(f, "info_open");
        st.arrow_label_right = 12;
        if (flag(f, "info_open")) st.state = ui::ButtonState::Selected;
        ui::button(c, SkRect::MakeXYWH(ox + 125 + (W - 250 - iw) * 0.5f, by, iw, bh), info, st);
    }
    if (flag(f, "dir_to")) {
        const std::string t = str(f, "dir_to_text", "DIR TO");
        const float bw = btn_w(t, 22);
        ui::button(c, SkRect::MakeXYWH(ox + W - 125 + (125 - bw - 5) * 0.5f, by, bw, bh), t, {});
    }
    (void)tmpy_info;
}

void draw_tmpy_buttons(SkCanvas& c, float ox, float bottom) {
    ui::ButtonStyle st;
    st.asterisk = true;
    st.text_color = ui::pal::amber;
    const float bh = 55.3f;
    bottom -= 9.2f;
    ui::button_lines(c, SkRect::MakeXYWH(ox, bottom - bh, 110, bh), {"ERASE", "TMPY"}, st);
    ui::button_lines(c, SkRect::MakeXYWH(ox + W - 123, bottom - bh, 123, bh), {"INSERT", "TMPY"}, st);
}

void draw_menu(SkCanvas& c, float ox, float top, float bottom, const json& m, const json& page) {
    std::vector<ui::MenuItem> items;
    for (const json& it : at(m, "items")) items.push_back({str(it, "text"), flag(it, "disabled")});
    if (items.empty()) return;
    const std::string owner = str(m, "owner");
    if (owner == "spd_alt") {
        SkRect r = SkRect::MakeXYWH(ox + 316, top + 5 + 43, 260, 0);
        ui::dropdown_menu(c, r, items);
    } else if (owner == "info") {
        const float h = ui::dropdown_menu_item_height() * items.size();
        const float w = ui::dropdown_menu_natural_width(items);
        const float iw = tw(str(at(page, "footer"), "info"), 22) + 58;
        const float x = ox + 125 + (W - 250 - iw) * 0.5f;
        ui::dropdown_menu(c, SkRect::MakeXYWH(x, bottom - 45 - h, std::max(w, iw), h), items);
    }
}

float num(const json& j, const char* k, float d) {
    const json& v = at(j, k);
    return v.is_number() ? v.get<float>() : d;
}

void draw_revisions(SkCanvas& c, float ox, const json& rv) {
    std::vector<ui::MenuItem> items;
    for (const json& it : at(rv, "items")) items.push_back({str(it, "text"), flag(it, "disabled")});
    if (items.empty()) return;
    const float x = ox + num(rv, "left", 195.3f), y = num(rv, "top", 183.3f);
    const float ih = 38.667f, w = 247.3f;
    SkRect box = SkRect::MakeXYWH(x, y, w, ih * items.size() + 4);
    ui::fill(c, box, ui::pal::darker_grey);
    ui::fill(c, SkRect::MakeLTRB(box.left(), box.top(), box.right(), box.top() + 2), 0xFFF2F4FF);
    ui::fill(c, SkRect::MakeLTRB(box.left(), box.top() + 2, box.left() + 2, box.bottom()), 0xFFF2F4FF);
    ui::fill(c, SkRect::MakeLTRB(box.left() + 2, box.bottom() - 2, box.right(), box.bottom()), 0xFF74757B);
    ui::fill(c, SkRect::MakeLTRB(box.right() - 2, box.top() + 2, box.right(), box.bottom() - 2), 0xFF74757B);
    float iy = y + 2;
    for (auto& it : items) {
        ui::TextStyle ts{22, it.disabled ? ui::pal::dark_grey : ui::pal::white, ui::Face::Mfd, ui::Align::Left};
        ui::text_in_aligned(c, it.text, SkRect::MakeXYWH(x + 2, iy - 0.67f, w - 4, ih), ts, 4);
        iy += ih;
    }
}

void draw_insert_wpt(SkCanvas& c, float ox, const json& d) {
    const float L = ox + 180, T = 186.67f, R = ox + 680, B = 811.f;
    SkRect box = SkRect::MakeLTRB(L, T, R, B);
    ui::fill(c, box, ui::pal::background);
    ui::fill(c, SkRect::MakeLTRB(L, T, R, T + 2), 0xFFF2F4FF);
    ui::fill(c, SkRect::MakeLTRB(L, T + 2, L + 2, B), 0xFFF2F4FF);
    ui::fill(c, SkRect::MakeLTRB(L + 2, B - 2, R, B), 0xFF74757B);
    ui::fill(c, SkRect::MakeLTRB(R - 2, T + 2, R, B - 2), 0xFF74757B);
    const json& texts = at(d, "texts");
    const std::string ident = texts.size() > 0 && texts[0].is_string() ? texts[0].get<std::string>() : "";
    const std::string coords = texts.size() > 1 && texts[1].is_string() ? texts[1].get<std::string>() : "";
    const float ix = L + 16.3f;
    ui::TextStyle big{30, ui::pal::green, ui::Face::Mfd, ui::Align::Left};
    ui::TextStyle small{22, ui::pal::white, ui::Face::Mfd, ui::Align::Left};
    const std::string title = str(d, "title");
    const std::string head = title.size() > ident.size() ? title.substr(0, title.size() - ident.size()) : title;
    ui::text(c, head, ix, T + 13.93f, small);
    ui::text(c, ident, ix + ui::text_width(head, small), T + 7.23f, big);
    ui::text(c, coords, ix + 50, T + 43.2f, big);
    SkRect dd = SkRect::MakeXYWH(ox + 247.33f, 266.67f, 175, 43.3f);
    ui::dropdown(c, dd, "", {});
    SkRect cy = SkRect::MakeXYWH(ox + 257.3f, 272.7f, 122, 32);
    const bool editing = flag(d, "editing", true);
    if (editing) {
        SkPaint pw;
        pw.setAntiAlias(true);
        pw.setColor(ui::pal::white);
        c.drawRRect(SkRRect::MakeRectXY(cy, 3, 3), pw);
        pw.setColor(ui::pal::background);
        c.drawRRect(SkRRect::MakeRectXY(cy.makeInset(1.33f, 1.33f), 2, 2), pw);
        pw.setColor(ui::pal::cyan);
        c.drawRect(cy.makeInset(2.67f, 2.67f), pw);
    }
    const json& fields = at(d, "fields");
    if (fields.size() > 0 && fields[0].is_string())
        ui::text_in_aligned(c, fields[0].get<std::string>(), cy,
                            {27, editing ? ui::pal::background : ui::pal::cyan, ui::Face::Mfd, ui::Align::Left}, 4);
    std::vector<ui::MenuItem> items;
    for (const json& it : at(d, "items")) if (it.is_string()) items.push_back({it.get<std::string>(), false});
    if (!items.empty()) ui::dropdown_menu(c, SkRect::MakeXYWH(dd.left(), dd.bottom(), dd.width(), 0), items);
    const json& btns = at(d, "buttons");
    if (btns.size() > 0 && btns[0].is_string()) {
        const std::string t = btns[0].get<std::string>();
        const float h = ui::button_height();
        ui::button(c, SkRect::MakeXYWH(L + 6.67f, 804.6f - h, tw(t, 22) + 28, h), t, {});
    }
}

void draw_new_dest(SkCanvas& c, float ox, const json& d) {
    const float L = ox + 200, T = 361.33f, R = L + 500, B = T + 299.5f;
    ui::fill(c, SkRect::MakeLTRB(L, T, R, B), ui::pal::background);
    ui::fill(c, SkRect::MakeLTRB(L, T, R, T + 2), 0xFFF2F4FF);
    ui::fill(c, SkRect::MakeLTRB(L, T + 2, L + 2, B), 0xFFF2F4FF);
    ui::fill(c, SkRect::MakeLTRB(L + 2, B - 2, R, B), 0xFF74757B);
    ui::fill(c, SkRect::MakeLTRB(R - 2, T + 2, R, B - 2), 0xFF74757B);
    const json& texts = at(d, "texts");
    const std::string ident = texts.size() > 0 && texts[0].is_string() ? texts[0].get<std::string>() : "";
    const std::string title = str(d, "title");
    const std::string head = title.size() > ident.size() ? title.substr(0, title.size() - ident.size()) : title;
    ui::TextStyle big{30, ui::pal::green, ui::Face::Mfd, ui::Align::Left};
    ui::TextStyle small{22, ui::pal::white, ui::Face::Mfd, ui::Align::Left};
    const float ix = L + 21.3f;
    const float cy = 424.5f;
    ui::text(c, head, ix, cy - ui::line_height(small) * 0.5f + 1.33f, small);
    ui::text(c, ident, ix + 1.33f + ui::text_width(head, small), cy - ui::line_height(big) * 0.5f - 2.5f, big);
    const json& fields = at(d, "fields");
    const std::string ft = fields.size() > 0 && fields[0].is_string() ? fields[0].get<std::string>() : "";
    ui::FieldStyle fs;
    fs.align = ui::Align::Center;
    const float fw = ui::input_field_width(ft, fs);
    ui::input_field(c, SkRect::MakeXYWH(L + 16.33f + (485 - fw) * 0.5f, 484.96f, fw, 37), ft, fs);
    const json& btns = at(d, "buttons");
    if (btns.size() > 0 && btns[0].is_string()) {
        const std::string t = btns[0].get<std::string>();
        const float h = ui::button_height();
        ui::button(c, SkRect::MakeXYWH(L + 7.2f, B - 5.9f - h, tw(t, 22) + 28, h), t, {});
    }
}

void draw_fpln(SkCanvas& c, SkRect content, const json& page) {
    const float ox = content.left(), T = content.top(), B = content.bottom();
    c.save();
    const float hdr_y = T + 5;
    ui::label(c, "FROM", ox + 3, hdr_y + (45 - text_h(20)) * 0.5f, ui::pal::white, 20);
    put(c, "TIME", ox + kColIdent + kColTime * 0.5f, hdr_y, 45, 20, ui::pal::white, Align::Center);
    {
        ui::ButtonStyle st;
        st.arrow = true;
        st.pad_right = 5;
        const json& menu = at(page, "menu");
        if (menu.is_object() && str(menu, "owner") == "spd_alt") st.state = ui::ButtonState::Selected;
        st.arrow_up = false;
        SkRect br = SkRect::MakeXYWH(ox + kColIdent + kColTime + 35, hdr_y, 260, 43);
        ui::button(c, br, "", st);
        put(c, str(page, "spd_alt_label", "SPD    ALT"), br.left() + 100.4f, br.top(), 43, 22, ui::pal::white, Align::Center);
    }
    put(c, "TRK", ox + kColTrk - 0.8f, hdr_y, 45, 20, ui::pal::white);
    put(c, "DIST", ox + kColDist + 0.06f * W - 0.47f, hdr_y, 45, 20, ui::pal::white, Align::Right);
    put(c, "FPA", ox + kColFpa + 0.08f * W - 0.47f, hdr_y, 45, 20, ui::pal::white, Align::Right);
    ui::hline(c, ox, ox + W, T + 50.f, ui::pal::light_grey, 1);

    const float dest_top = B - 123.33f;
    const float foot_top = B - 52.67f;
    c.save();
    c.clipRect(SkRect::MakeLTRB(ox, T + 51, ox + W, dest_top));
    float y = T + 51;
    for (const json& l : at(page, "lines")) {
        const bool first = flag(l, "first");
        const float yy = first ? y + 16 : y;
        if (str(l, "kind") == "special") draw_special(c, ox, yy, l);
        else draw_leg(c, ox, yy, l);
        y += first ? 56 : kLine;
    }
    c.restore();

    if (flag(page, "tmpy_buttons")) draw_tmpy_buttons(c, ox, dest_top - 2);
    draw_dest_line(c, ox, dest_top, at(page, "dest"));
    draw_footer(c, ox, foot_top, at(page, "footer"), false);
    const json& menu = at(page, "menu");
    const json& rv = at(page, "revisions");
    if (rv.is_object()) draw_revisions(c, ox, rv);
    for (const json& d : at(page, "dialogs")) {
        if (str(d, "kind") == "insert_next_wpt") draw_insert_wpt(c, ox, d);
        else if (str(d, "kind") == "new_dest") draw_new_dest(c, ox, d);
    }
    if (menu.is_object()) draw_menu(c, ox, T, B, menu, page);
    c.restore();
}

std::vector<ui::MenuItem> menu_of(const json& d) {
    std::vector<ui::MenuItem> v;
    for (const json& i : at(d, "items")) v.push_back({str(i, "text"), flag(i, "disabled")});
    return v;
}

ui::FieldStyle vr_field_style(const json& f, float min_w) {
    ui::FieldStyle fs;
    fs.kind = ui::FieldKind::Entered;
    if (str(f, "kind") == "mandatory") fs.kind = ui::FieldKind::Mandatory;
    else if (str(f, "kind") == "disabled") fs.kind = ui::FieldKind::Disabled;
    else if (str(f, "kind") == "tmpy") fs.kind = ui::FieldKind::Tmpy;
    else if (str(f, "kind") == "computed") fs.font_size = 22;
    fs.unit_trailing = str(f, "unit_trailing");
    fs.unit_leading = str(f, "unit_leading");
    fs.align = Align::Right;
    fs.min_text_width = min_w;
    return fs;
}

void vr_field(SkCanvas& c, float x, float y, const json& f, float min_w, Align al = Align::Right) {
    ui::FieldStyle fs = vr_field_style(f, min_w);
    fs.align = al;
    const float w = ui::input_field_width(str(f, "text"), fs);
    ui::input_field(c, SkRect::MakeXYWH(x, y, w, 37), str(f, "text"), fs);
}

void vr_radios(SkCanvas& c, float x, float cy0, const json& list, float pitch, float text_dx = 30, float text_dy = -1) {
    int i = 0;
    for (const json& r : list) {
        const bool amber = str(r, "color") == "amber";
        const bool chk = flag(r, "checked");
        const SkColor cc = amber ? ui::pal::amber : ui::pal::cyan;
        ui::radio_button(c, x + 2, cy0 + i * pitch - 19, "", chk, cc, false);
        ui::TextStyle ts{26, chk || amber ? cc : ui::pal::white, ui::Face::Mfd, Align::Left};
        ui::text(c, str(r, "text"), x + 2 + 19 + text_dx, cy0 + i * pitch - ui::line_height(ts) * 0.5f + text_dy, ts);
        ++i;
    }
}

void vr_delete(SkCanvas& c, float right, float top, const json& b, float min_w = 0) {
    if (!b.is_object()) return;
    std::vector<std::string> lines;
    const std::string label = str(b, "label");
    size_t a = 0;
    float mw = 0;
    while (true) {
        size_t e = label.find('\n', a);
        lines.push_back(label.substr(a, e == std::string::npos ? e : e - a));
        mw = std::max(mw, tw(lines.back(), 22));
        if (e == std::string::npos) break;
        a = e + 1;
    }
    ui::ButtonStyle st;
    st.asterisk = true;
    st.state = str(b, "state") == "disabled" ? ui::ButtonState::Disabled : ui::ButtonState::Normal;
    const float w = std::max(min_w, mw + 10 + tw("*", 22) + 24 + 4);
    const float h = ui::button_height(22, (int)lines.size());
    ui::button_lines(c, SkRect::MakeXYWH(right - w, top, w, h), lines, st);
}

void vr_wpt_row(SkCanvas& c, SkRect C, float y, const std::string& label, const json& wpt) {
    constexpr float dd_w = 175, dd_h = 43;
    const float lw = tw(label, 24);
    const float total = 15 + lw + 15 + dd_w;
    const float x0 = C.centerX() - total * 0.5f;
    const float ddx = x0 + 15 + lw + 15;
    put(c, label, x0 + 15, y, dd_h, 24, ui::pal::white);
    SkRect dd = SkRect::MakeXYWH(ddx, y, dd_w, dd_h);
    ui::DropdownStyle ds;
    ds.align = Align::Left;
    const bool open = flag(wpt, "open");
    ui::dropdown(c, dd, open ? "" : str(wpt, "text"), ds);
    if (open) {
        const std::string t = str(wpt, "text");
        const float w = tw(t, 27);
        ui::stroke_box(c, SkRect::MakeXYWH(dd.left() + 10, dd.top() + 6.7f, w + 4, 30.6f), ui::pal::light_grey, 2);
        ui::fill(c, SkRect::MakeXYWH(dd.left() + 12.7f, dd.top() + 9.5f, w, 25), ui::pal::cyan);
        ui::TextStyle ts{27, ui::pal::background, ui::Face::Mfd, Align::Left};
        ui::text(c, t, dd.left() + 12.7f, dd.top() + 9.5f + (25 - ui::line_height(ts)) * 0.5f, ts);
    }
}

void vr_wpt_menu(SkCanvas& c, SkRect C, float y, const std::string& label, const json& wpt) {
    if (!flag(wpt, "open")) return;
    const float lw = tw(label, 24);
    const float total = 15 + lw + 15 + 175;
    const float ddx = C.centerX() - total * 0.5f + 15 + lw + 15;
    ui::dropdown_menu(c, SkRect::MakeXYWH(ddx, y + 43, 175, 0), menu_of(wpt));
}

void draw_vr_spd(SkCanvas& c, SkRect C, float top, const json& t) {
    const float y0 = top + 235;
    vr_wpt_row(c, C, y0, "SPD CSTR AT", at(t, "wpt"));
    const float line_top = y0 + 43 + 50;
    vr_field(c, C.left() + 10, line_top + 39.5f, at(t, "input"), 54);
    if (flag(at(t, "clbdes"), "visible")) vr_radios(c, C.left() + 253.3f, line_top + 40.2f, at(at(t, "clbdes"), "radios"), 42, 25.3f, -2);
    vr_delete(c, C.right() - 1 + 1.33f, line_top + 29.37f, at(t, "delete_cstr"));
    ui::hline(c, C.left(), C.right(), line_top + 165, ui::pal::light_grey, 2);
    const float y1 = line_top + 183.7f;
    put(c, str(t, "limit_header"), C.left() + 2.6f, y1, 30, 24, ui::pal::white);
    const float y2 = y1 + 65;
    vr_field(c, C.left() + 10, y2 - 0.67f, at(t, "limit_speed"), 54);
    put(c, str(t, "limit_text"), C.left() + 10 + 99 + 49.3f, y2, 37, 24, ui::pal::white);
    vr_field(c, C.left() + 372.7f, y2 - 0.67f, at(t, "limit_alt"), 90);
    vr_delete(c, C.right() - 1 + 0.67f, y2 - 10.2f, at(t, "delete_limit"));
    vr_wpt_menu(c, C, y0, "SPD CSTR AT", at(t, "wpt"));
}

void draw_vr_alt(SkCanvas& c, SkRect C, float top, const json& t) {
    const float y0 = top + 235;
    vr_wpt_row(c, C, y0, "ALT CSTR AT", at(t, "wpt"));
    const float line_top = y0 + 43 + 50 + 12;
    vr_radios(c, C.left() + 11, line_top + 13.5f, at(t, "radios"), 42);
    ui::vline(c, C.left() + 268, line_top - 7, line_top + 176, ui::pal::light_grey, 2);
    const float rb_w = 4 + 34 + 15 + tw("CLB CSTR", 26) + 2;
    vr_delete(c, C.right() - 20 + 0.67f, line_top - 11.37f, at(t, "delete_cstr"), rb_w);
    if (flag(at(t, "clbdes"), "visible")) vr_radios(c, C.right() - 20 - rb_w + 2, line_top + 83.63f, at(at(t, "clbdes"), "radios"), 42, 30, 0);
    ui::hline(c, C.left(), C.right(), line_top + 180, ui::pal::light_grey, 2);
    vr_wpt_menu(c, C, y0, "ALT CSTR AT", at(t, "wpt"));
}

void draw_vr_step(SkCanvas& c, SkRect C, float top, const json& t) {
    const float bl = C.left() + 0.1f, br = C.right() + 1.1f;
    const float bt = top + 216.1f;
    const float gx = C.left() + 14, gw = C.width() - 28;
    const float c1 = gx + 0.35f * gw, c2 = gx + 0.6f * gw, c3 = gx + 0.8f * gw;
    const float hy = top + 229.4f;
    const SkColor line_c = 0xFF777777;
    ui::stroke_box(c, SkRect::MakeLTRB(bl, bt, br, top + 605.1f), ui::pal::light_grey, 1);
    {
        const float lx = bl + 15 + 11 + 2;
        const float w1 = tw("STEP ALTs FROM CRZ", 20);
        const float x_fl = lx + w1 + 15;
        const float w_fl = tw("FL", 20);
        const float x_v = x_fl + w_fl + 6;
        const std::string v = str(t, "crz_fl");
        ui::fill(c, SkRect::MakeLTRB(lx - 4, bt - 12, x_v + tw(v, 25) + 4, bt + 12), ui::pal::background);
        put(c, "STEP ALTs FROM CRZ", lx, bt - 9.3f, 24, 20, ui::pal::white);
        put(c, "FL", x_fl, bt - 11.3f, 24, 20, ui::pal::unit_blue);
        put(c, v, x_v, bt - 11.3f, 24, 25, ui::pal::green);
    }
    const float hb = hy + 60.67f;
    put(c, "WPT", gx + 62, -0.67f + hb - 10 - text_h(20), text_h(20), 20, ui::pal::white);
    put(c, "ALT", c1 + 25, hb - 10 - text_h(20), text_h(20), 20, ui::pal::white);
    put(c, "DIST", c2 + 30, hb - 10 - text_h(20), text_h(20), 20, ui::pal::white);
    put(c, str(t, "time_header"), c3 + 30, hb - 10 - text_h(20), text_h(20), 20, ui::pal::white);
    ui::hline(c, gx, gx + gw, hb - 1, line_c, 1);
    ui::vline(c, c2 - 1, hb - 31, hb + 300 - 1, line_c, 1);
    const SkRect* open_rect = nullptr;
    SkRect open_dd;
    const json* open_item = nullptr;
    int ri = 0;
    for (const json& r : at(t, "rows")) {
        const float ry = hb + ri * 60;
        if (!flag(r, "wpt_hidden")) {
            SkRect dd = SkRect::MakeXYWH(gx + 0.175f * gw - 86.2f, ry + 8.5f, 175, 43);
            ui::DropdownStyle ds;
            ds.align = Align::Left;
            ui::dropdown(c, dd, str(at(r, "wpt"), "text"), ds);
            if (flag(at(r, "wpt"), "open")) {
                open_dd = dd;
                open_rect = &open_dd;
                open_item = &at(r, "wpt");
            }
        }
        if (!flag(r, "alt_hidden")) {
            const json& f = at(r, "alt");
            ui::FieldStyle fs = vr_field_style(f, 90);
            fs.align = Align::Center;
            ui::input_field(c, SkRect::MakeXYWH(c1 + 0.125f * gw - 75 - 0.5f, ry + 11.5f, 150, 37), str(f, "text"), fs);
        }
        if (!str(r, "msg").empty()) put(c, str(r, "msg"), c2 + 0.2f * gw, ry, 60, 20, ui::pal::white, Align::Center);
        if (!flag(r, "dist_hidden")) put(c, str(r, "dist"), c3 - 6 - tw("NM", 20), ry, 60, 25, ui::pal::green, Align::Right);
        if (!flag(r, "time_hidden")) put(c, str(r, "time"), c3 + 0.1f * gw, ry, 60, 25, ui::pal::green, Align::Center);
        ++ri;
    }
    const json& ic = at(t, "icons");
    const bool dd_dis = ic.size() > 0 ? flag(ic[0], "disabled") : true;
    const bool up_dis = ic.size() > 1 ? flag(ic[1], "disabled") : true;
    SkRect b1 = SkRect::MakeXYWH(C.centerX() - 60.7f, top + 627.2f, 58, 56.5f);
    SkRect b2 = b1.makeOffset(64, 0);
    ui::icon_button(c, b1, dd_dis);
    double_chevron(c, b1, true, dd_dis ? 0xFF808080 : ui::pal::white);
    ui::icon_button(c, b2, up_dis);
    double_chevron(c, b2, false, up_dis ? 0xFF808080 : ui::pal::white);
    const float ot = top + 730.0f, ob = top + 868.1f;
    ui::stroke_box(c, SkRect::MakeLTRB(bl, ot, br, ob), ui::pal::light_grey, 1);
    ui::fill(c, SkRect::MakeLTRB(bl + 10, ot - 12, bl + 20 + tw("OPTIMUM STEP POINT", 20), ot + 12), ui::pal::background);
    put(c, "OPTIMUM STEP POINT", bl + 12, ot - 12, 24, 20, ui::pal::white);
    const float oy = top + 780 - 13.7f;
    put(c, "TO", C.left() + 25.7f, oy, 30, 20, ui::pal::white);
    put(c, "FL", C.left() + 25 + tw("TO", 20) + 10, oy, 30, 20, ui::pal::unit_blue);
    put(c, str(t, "opt_fl"), C.left() + 25 + tw("TO", 20) + 10 + tw("FL", 20) + 6, oy, 30, 25, ui::pal::green);
    put(c, str(t, "opt_msg"), C.centerX() - 22.3f, top + 813 - 13.7f, 30, 20, ui::pal::white, Align::Center);
    if (open_rect && open_item) ui::dropdown_menu(c, SkRect::MakeXYWH(open_rect->left(), open_rect->bottom(), 175, 0), menu_of(*open_item));
}

void draw_vert_rev(SkCanvas& c, SkRect content, const json& page) {
    const float top = content.top() - 131.33f;
    std::vector<std::string> tabs;
    for (const json& t : at(page, "tabs")) tabs.push_back(t.get<std::string>());
    if (!tabs.empty()) {
        ui::TabStyle ts;
        const json& sel = at(page, "selected");
        ts.selected = sel.is_number() ? sel.get<int>() : 0;
        SkRect C = ui::tab_navigator(c, SkRect::MakeLTRB(content.left(), top + 146.67f, content.right(), top + 917), tabs, ts);
        C.fTop += 1.33f;
        const json& tab = at(page, "tab");
        if (str(tab, "kind") == "spd") draw_vr_spd(c, C, top, tab);
        else if (str(tab, "kind") == "alt") draw_vr_alt(c, C, top, tab);
        else if (str(tab, "kind") == "cms") put(c, str(tab, "text"), C.centerX(), top + 194, 20, 20, ui::pal::white, Align::Center);
        else if (str(tab, "kind") == "step") draw_vr_step(c, C, top, tab);
    }
    ui::button(c, SkRect::MakeXYWH(content.left() + 2.7f, top + 920.7f, tw("RETURN", 22) + 28, ui::button_height()), "RETURN", {});
}

void draw_hold(SkCanvas& c, SkRect content, const json& page) {
    const float L = content.left();
    const float top = content.top() - 131.33f;
    const float ft = top + 152.1f, fb = top + 874.8f;
    const float fl = L, fr = L + 602.6f;
    ui::stroke_box(c, SkRect::MakeLTRB(fl, ft, fr, fb), ui::pal::light_grey, 1);
    const std::string fix = str(page, "fix");
    {
        const float w1 = tw("COMPUTED HOLD AT ", 20);
        const float w2 = tw(fix, 24);
        ui::fill(c, SkRect::MakeLTRB(fl + 8, ft - 12, fl + 15 + w1 + w2 + 4, ft + 12), ui::pal::background);
        put(c, "COMPUTED HOLD AT ", L + 12.8f, ft - 12, 24, 20, ui::pal::white);
        put(c, fix, L + 12.8f + w1, ft - 10, 24, 24, ui::pal::green);
    }
    put(c, "INBOUND CRS", L + 15.5f, top + 226 - 11.3f, 24, 20, ui::pal::white);
    const json& fields = at(page, "fields");
    auto fld = [&](int i, float x, float y, float min_w) {
        if (i >= (int)fields.size() || flag(fields[i], "hidden")) return;
        vr_field(c, L + x, top + y, fields[i], min_w, Align::Center);
    };
    fld(0, 90.4f, 255.03f, 72);
    put(c, "TURN", L + 16.3f, top + 351 - 11.3f, 24, 20, ui::pal::white);
    const json& rg = at(page, "radios");
    auto radios = [&](int g, float cy0) {
        if (g >= (int)rg.size()) return;
        int i = 0;
        for (const json& r : rg[g]) {
            const bool chk = flag(r, "checked");
            const SkColor cc = ui::pal::yellow;
            ui::radio_button(c, L + 112.6f - 19, top + cy0 + i * 42 - 19, "", chk, cc, false);
            ui::TextStyle ts{26, chk ? cc : ui::pal::white, ui::Face::Mfd, Align::Left};
            ui::text(c, str(r, "text"), L + 142.7f, top + cy0 + i * 42 - ui::line_height(ts) * 0.5f - 1, ts);
            ++i;
        }
    };
    radios(0, 399.6f);
    put(c, "LEG DEFINING PARAMETER", L + 15.5f, top + 533.6f - 11.3f, 24, 20, ui::pal::white);
    radios(1, 582.f);
    fld(1, 248.67f, 573.67f, 54);
    fld(2, 248.67f, 573.67f, 72);
    ui::hline(c, L + 15.6f, L + 588, top + 708.3f, ui::pal::light_grey, 1);
    put(c, str(page, "exit_label"), L + 15.5f, top + 767 - 11.3f, 24, 20, ui::pal::white);
    put(c, "AT", L + 73.7f, top + 815 - 11.3f, 24, 20, ui::pal::white);
    put(c, "UTC", L + 187.9f, top + 815 - 11.3f, 24, 20, ui::pal::white);
    put(c, "EFOB", L + 359, top + 815 - 11.3f, 24, 20, ui::pal::white);
    const json& gv = at(page, "grid_values");
    if (gv.size() > 0) put(c, gv[0].get<std::string>(), L + 186.6f, top + 842.8f - 14, 28, 25, ui::pal::magenta);
    if (gv.size() > 1) {
        put(c, gv[1].get<std::string>(), L + 366, top + 842.8f - 14, 28, 25, ui::pal::magenta);
        put(c, str(page, "grid_unit"), L + 366 + tw(gv[1].get<std::string>(), 25) + 6, top + 842.8f - 14, 28, 20, ui::pal::unit_blue);
    }
    float by[2] = {176.8f, 250.f};
    int bi = 0;
    for (const json& b : at(page, "buttons")) {
        if (flag(b, "hidden")) continue;
        const std::string lb = str(b, "label");
        ui::ButtonStyle st;
        st.state = str(b, "state") == "disabled" ? ui::ButtonState::Disabled : ui::ButtonState::Normal;
        if (flag(b, "tmpy")) {
            st.tmpy = true;
            ui::button(c, SkRect::MakeXYWH(L + 600, top + 919, 164.6f, 36.4f), lb, st);
        } else if (bi < 2) {
            ui::button(c, SkRect::MakeXYWH(L + 609.1f, top + by[bi], 153.8f, 62.5f), lb, st);
            ++bi;
        }
    }
}

void draw_airways(SkCanvas& c, SkRect content, const json& page) {
    const float L = content.left();
    const float top = content.top() - 131.33f;
    put(c, "AIRWAYS FROM", L + 14.8f, top + 159.5f - 12, 24, 20, ui::pal::white);
    put(c, str(page, "fix"), L + 185.7f, top + 160 - 15, 30, 30, ui::pal::yellow);
    ui::bevel_border(c, SkRect::MakeLTRB(L + 30.4f, top + 184.2f, L + 737.5f, top + 812.4f), ui::Bevel::Outset, ui::pal::light_grey, 2);
    put(c, "VIA", L + 46.9f, top + 234 - 12 - 0.667f, 24, 20, ui::pal::white);
    put(c, "TO", L + 545, top + 234 - 12 - 0.667f, 24, 20, ui::pal::white);
    const json& fields = at(page, "fields");
    auto fld = [&](int i, float x, float w) {
        if (i >= (int)fields.size()) return;
        ui::FieldStyle fs = vr_field_style(fields[i], 0);
        fs.align = Align::Center;
        ui::input_field(c, SkRect::MakeXYWH(L + x, top + 215.37f, w, 36.2f), str(fields[i], "text"), fs);
    };
    fld(0, 88.8f, 110.2f);
    fld(1, 574.7f, 146.3f);
    const json& ic = at(page, "icons");
    const bool d1 = ic.size() > 0 ? flag(ic[0], "disabled") : true;
    const bool d2 = ic.size() > 1 ? flag(ic[1], "disabled") : true;
    SkRect b1 = SkRect::MakeXYWH(L + 314.7f, top + 818.6f, 59.3f, 59.2f);
    SkRect b2 = SkRect::MakeXYWH(L + 394.5f, top + 818.6f, 59.3f, 59.2f);
    ui::icon_button(c, b1, d1);
    double_chevron(c, b1, true, d1 ? 0xFF808080 : ui::pal::white);
    ui::icon_button(c, b2, d2);
    double_chevron(c, b2, false, d2 ? 0xFF808080 : ui::pal::white);
    for (const json& b : at(page, "buttons")) {
        if (flag(b, "hidden") || !flag(b, "tmpy")) continue;
        ui::ButtonStyle st;
        st.tmpy = true;
        ui::button(c, SkRect::MakeXYWH(L + 600, top + 919, 164.6f, 36.4f), str(b, "label"), st);
    }
}

void draw_direct_to(SkCanvas& c, SkRect content, const json& page) {
    const float L = content.left();
    const float T = content.top();
    const json& wpt = at(page, "wpt");
    const float lw = tw("DIRECT TO", 20);
    constexpr float dd_w = 175, dd_h = 43;
    const float row_top = T + 10;
    put(c, "DIRECT TO", L, row_top + 0.75f, dd_h, 20, ui::pal::white);
    SkRect dd = SkRect::MakeXYWH(L + lw + 15, row_top + 0.29f, dd_w, dd_h);
    ui::DropdownStyle ds;
    ds.align = Align::Left;
    const bool open = flag(wpt, "open");
    const bool tmpy = str(wpt, "kind") == "tmpy";
    if (tmpy) ds.text_color = ui::pal::yellow;
    ui::dropdown(c, dd, open ? "" : str(wpt, "text"), ds);
    if (open) {
        const std::string t = str(wpt, "text");
        const float w = tw(t, 27);
        ui::stroke_box(c, SkRect::MakeXYWH(dd.left() + 10, dd.top() + 6.7f, w + 4, 30.6f), ui::pal::light_grey, 2);
        ui::fill(c, SkRect::MakeXYWH(dd.left() + 12.7f, dd.top() + 9.5f, w, 25), ui::pal::cyan);
        ui::TextStyle ts{27, ui::pal::background, ui::Face::Mfd, Align::Left};
        ui::text(c, t, dd.left() + 12.7f, dd.top() + 9.5f + (25 - ui::line_height(ts)) * 0.5f, ts);
    }
    const SkColor vcol = flag(page, "yellow") ? ui::pal::yellow : ui::pal::green;
    const std::string utc = str(page, "utc"), dist = str(page, "dist");
    const float c1 = tw("DIST", 24) + 10;
    const float c2 = std::max(tw(utc, 30), tw(dist, 30));
    const float c3 = tw("NM", 20);
    const float gw = c1 + c2 + c3;
    const float gx = L + 192 - gw * 0.5f;
    const float r1 = row_top + dd_h + 40;
    put(c, "UTC", gx + c1 - 10, r1, 27, 24, ui::pal::white, Align::Right);
    put(c, utc, gx + c1 + c2, r1, 27, 30, vcol, Align::Right);
    put(c, "DIST", gx + c1 - 10, r1 + 37.3f, 27, 24, ui::pal::white, Align::Right);
    put(c, dist, gx + c1 + c2, r1 + 37.3f, 27, 30, vcol, Align::Right);
    put(c, "NM", gx + c1 + c2, r1 + 37.3f, 27, 20, ui::pal::unit_blue);

    const float bx = L + 384, by = T + 10;
    ui::stroke_box(c, SkRect::MakeXYWH(bx, by, 384, 285.3f), 0xFF808080, 2);
    put_top(c, "OPTIONS", bx + 12, by + 7, 20, ui::pal::white, Align::Left);
    const json& rd = at(page, "radios");
    bool any = false;
    for (const json& r : rd) any = any || flag(r, "checked");
    bool first_enabled_done = any;
    float ry = by + 2 + 5 + 18 + 45;
    for (const json& r : rd) {
        const bool dis = flag(r, "disabled");
        bool chk = flag(r, "checked");
        if (!first_enabled_done && !dis) {
            chk = true;
            first_enabled_done = true;
        }
        const SkColor cc = flag(r, "yellow") ? ui::pal::yellow : ui::pal::cyan;
        ui::radio_button(c, bx + 30.4f, ry, "", chk, cc, dis);
        ui::TextStyle rts{26, dis ? ui::pal::dark_grey : chk ? cc : ui::pal::white, ui::Face::Mfd, Align::Left};
        ui::text(c, str(r, "text"), bx + 79.04f, ry + 18.6f - ui::line_height(rts) * 0.5f, rts);
        ry += 41.8f;
    }

    const json& bl = at(page, "buttons");
    for (size_t i = 0; i < bl.size(); ++i) {
        const json& b = bl[i];
        if (!flag(b, "visible")) continue;
        std::vector<std::string> lines;
        const std::string label = str(b, "label");
        size_t a = 0;
        float mw = 0;
        while (true) {
            size_t e = label.find(char(10), a);
            lines.push_back(label.substr(a, e == std::string::npos ? e : e - a));
            mw = std::max(mw, tw(lines.back(), 22));
            if (e == std::string::npos) break;
            a = e + 1;
        }
        ui::ButtonStyle st;
        st.asterisk = flag(b, "asterisk");
        st.tmpy = flag(b, "tmpy");
        const float w = mw + (st.asterisk ? tw("*", 22) : 0) + 24 + 4;
        const float h = ui::button_height(22, (int)lines.size());
        const float x = i == 2 ? L + 768 - 2 - w : L + 2;
        ui::button_lines(c, SkRect::MakeXYWH(x, 959.3f - 2 - h, w, h), lines, st);
    }
    if (open) ui::dropdown_menu(c, SkRect::MakeXYWH(dd.left(), dd.bottom(), dd_w, 0), menu_of(wpt));
}

void draw_hold_sec(SkCanvas& c, SkRect r, const json& p) { draw_hold(c, r, p); }
void draw_hold_tmpy(SkCanvas& c, SkRect r, const json& p) { draw_hold(c, r, p); }
void draw_airways_sec(SkCanvas& c, SkRect r, const json& p) { draw_airways(c, r, p); }
void draw_airways_tmpy(SkCanvas& c, SkRect r, const json& p) { draw_airways(c, r, p); }

void draw_fpln_active(SkCanvas& c, SkRect r, const json& p) { draw_fpln(c, r, p); }
void draw_fpln_sec(SkCanvas& c, SkRect r, const json& p) { draw_fpln(c, r, p); }
void draw_fpln_tmpy(SkCanvas& c, SkRect r, const json& p) { draw_fpln(c, r, p); }

}  // namespace

A380_MFD_PAGE("fms/active/f-pln", draw_fpln_active);
A380_MFD_PAGE("fms/sec/f-pln", draw_fpln_sec);
A380_MFD_PAGE("fms/active/f-pln/vert-rev", draw_vert_rev);
A380_MFD_PAGE("fms/temporary/f-pln", draw_fpln_tmpy);
A380_MFD_PAGE("fms/active/f-pln/hold", draw_hold);
A380_MFD_PAGE("fms/sec/f-pln/hold", draw_hold_sec);
A380_MFD_PAGE("fms/temporary/f-pln/hold", draw_hold_tmpy);
A380_MFD_PAGE("fms/active/f-pln/airways", draw_airways);
A380_MFD_PAGE("fms/active/f-pln/direct-to", draw_direct_to);
A380_MFD_PAGE("fms/sec/f-pln/airways", draw_airways_sec);
A380_MFD_PAGE("fms/temporary/f-pln/airways", draw_airways_tmpy);

}  // namespace a380::mfd
