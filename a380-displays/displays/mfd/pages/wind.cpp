#include "include/core/SkPathBuilder.h"
#include "page_model.h"

namespace a380::mfd {
namespace {

using namespace pm;

float kTop = 131.33f;
float kTxt = 1.33f;

SkRect ab(SkRect content, float x0, float y0, float x1, float y1) { return box(content, x0, y0 - kTop, x1, y1 - kTop); }

void dev_hline(SkCanvas& c, SkRect content, float x0, float x1, int row, int n = 1) {
    const float s = c.getTotalMatrix().getScaleY();
    ui::fill(c, SkRect::MakeLTRB(content.left() + x0, content.top() + row / s - kTop, content.left() + x1,
                                 content.top() + (row + n) / s - kTop), 0xFF4F5467);
}

void dev_vline(SkCanvas& c, SkRect content, int col, int row0, int row1) {
    const float s = c.getTotalMatrix().getScaleX();
    ui::fill(c, SkRect::MakeLTRB(content.left() + col / s, content.top() + row0 / s - kTop, content.left() + (col + 1) / s,
                                 content.top() + row1 / s - kTop), 0xFF4F5467);
}

void chevrons(SkCanvas& c, SkRect r, bool up, SkColor col) {
    const float x0 = r.centerX() - 17.5f, y0 = r.centerY() - 17.5f;
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(col);
    for (int k = 0; k < 2; k++) {
        const float t = k == 0 ? 0.f : 17.5f;
        SkPathBuilder pb;
        if (up) {
            pb.moveTo(x0, y0 + t + 17.5f);
            pb.lineTo(x0 + 17.5f, y0 + t);
            pb.lineTo(x0 + 35, y0 + t + 17.5f);
        } else {
            pb.moveTo(x0, y0 + 17.5f - t);
            pb.lineTo(x0 + 17.5f, y0 + 35 - t);
            pb.lineTo(x0 + 35, y0 + 17.5f - t);
        }
        pb.close();
        c.drawPath(pb.detach(), p);
    }
}

void centered_menu(SkCanvas& c, SkRect r, const json& d) {
    if (!flag(d, "open")) return;
    std::vector<std::string> items;
    for (const auto& it : at(d, "items")) items.push_back(str(it, "text"));
    const float ih = ui::dropdown_menu_item_height();
    const float h = std::min(275.f, ih * items.size() + 2.67f);
    const SkRect box = SkRect::MakeXYWH(r.left(), r.bottom(), r.width(), h);
    ui::fill(c, box, ui::pal::darker_grey);
    ui::bevel_border(c, box, ui::Bevel::Outset, ui::pal::light_grey);
    ui::TextStyle ts = ui::label_style(ui::pal::white, 20);
    ts.align = ui::Align::Center;
    float y = box.top() + 2;
    for (const auto& t : items) {
        if (y + ih > box.bottom() + 1.f) break;
        ui::text(c, t, box.centerX(), y + 8, ts);
        y += ih;
    }
}

void draw_alt_row(SkCanvas& c, SkRect content, const json& a) {
    ui::TextStyle big = ui::label_bigger_style();
    ui::text(c, str(a, "label"), content.left() + 154.67f, content.top() + 782.67f - kTxt - kTop, big);
    ui::stroke_box(c, ab(content, 372.f, 800.8f, 602.67f, 855.5f), 0xFF4F5467, 1);
    ui::value_with_unit(c, str(a, "fl"), flag(a, "fl_unit_visible") ? str(a, "fl_unit") : std::string(), content.left() + 162.f,
                        content.top() + 828.8f - kTop, ui::pal::green, true, 28.f, 24.f);
    pm::field(c, ab(content, 377.33f, 807.43f, 475.33f, 849.43f), at(a, "dir"), ui::Align::Center, 54);
    pm::field(c, ab(content, 480.67f, 807.43f, 596.0f, 849.43f), at(a, "spd"), ui::Align::Center, 54);
}

void draw_crz(SkCanvas& c, SkRect content, const json& cz) {
    ui::TextStyle big = ui::label_bigger_style();
    ui::text(c, str(cz, "label"), content.left() + 144, content.top() + 226 - kTxt - kTop + 0.8f, big);
    const json& dd = at(cz, "dropdown");
    const SkRect dr = ab(content, 331.2f, 211.7f, 523.2f, 256.f);
    if (dd.is_object()) pm::dropdown_box(c, dr, dd);
    float x = 542.2f;
    for (const auto& ic : at(cz, "icons")) {
        const SkRect r = ab(content, x, 201.8f, x + 66, 263.8f);
        const bool dis = flag(ic, "disabled");
        ui::icon_button(c, r, dis);
        chevrons(c, r, flag(ic, "up"), dis ? 0xFF808080 : 0xFFFFFFFF);
        x += 72;
    }
    ui::TextStyle hdr = big;
    ui::text(c, "ALT", content.left() + 213.3f, content.top() + 760.43f - kTop, hdr);
    ui::text(c, "SAT", content.left() + 430.2f, content.top() + 760.43f - kTop, hdr);
    ui::stroke_box(c, ab(content, 146, 745.3f, 513, 859.3f), 0xFF4F5467, 1);
    if (at(cz, "alt").is_object()) pm::field(c, ab(content, 156.67f, 801.47f, 313.67f, 841.47f), at(cz, "alt"), ui::Align::Left, 54);
    if (at(cz, "sat").is_object()) pm::field(c, ab(content, 390.f, 800.8f, 503.f, 843.8f), at(cz, "sat"), ui::Align::Left, 54);
}

void draw_wind(SkCanvas& c, SkRect content, const json& page) {
    pm::GreyLayer grey(c);
    const bool is_crz = page.is_object() && page.contains("crz");
    const bool alt_page = page.is_object() && page.contains("alternate");
    kTop = is_crz ? 130.f : alt_page ? 130.67f : 131.33f;
    kTxt = is_crz ? 3.33f : alt_page ? 2.f : 1.33f;
    std::vector<std::string> tabs;
    for (const auto& t : at(page, "tabs")) tabs.push_back(t.get<std::string>());
    if (tabs.empty()) return;
    ui::TabStyle ts;
    const json& sel = at(page, "selected");
    ts.selected = sel.is_number() ? sel.get<int>() : 0;
    ui::tab_navigator(c, box(content, 0, 11.67f, 768, 756.33f), tabs, ts);

    ui::TextStyle big = ui::label_bigger_style();
    const float hk = is_crz ? 1.33f : kTxt;
    const json& crz = at(page, "crz");
    if (is_crz) draw_crz(c, content, crz);
    {
        ui::text(c, str(page, "title"), content.left() + 144, content.top() + 226 - kTxt - kTop, big);
    }
    {
        ui::text(c, "ALT", content.left() + 195.3f, content.top() + 299 - hk - kTop, big);
    }
    {
        ui::text(c, "T.WIND", content.left() + 453.3f, content.top() + 299 - hk - kTop, big);
    }

    const bool has_alt = page.is_object() && page.contains("alternate");
    const int sep_clb[5] = {517, 660, 787, 915, 1042};
    const int sep_des[5] = {517, 660, 785, 909, 1032};
    const int* sep = has_alt ? sep_des : sep_clb;
    const int nrows = std::clamp<int>(at(page, "rows").size() ? (int)at(page, "rows").size() : 5, 1, 5);
    const int last = has_alt ? 1156 : 1170 - (int)std::lround(127.5f * (5 - nrows));
    const int vend = has_alt ? 1283 : last + 1;
    dev_hline(c, content, 146, 610.67f, 424);
    dev_hline(c, content, 146, 610.67f, last);
    dev_vline(c, content, 219, 424, vend);
    dev_vline(c, content, 916, 424, vend);
    if (has_alt) dev_hline(c, content, 146, 610.67f, 1282);
    for (int i = 0; i < nrows; i++) dev_hline(c, content, 146, 610.67f, sep[i], 2);
    const float header_bottom = 345;
    float y = header_bottom;
    const json& rows = at(page, "rows");
    for (size_t i = 0; i < rows.size() && i < 5; i++) {
        const json& r = rows[i];
        const bool first = i == 0;
        const float h = first ? 95.f : has_alt ? 82.5f : 85.f;
        if (flag(r, "visible")) {
            const float ct = y + 1 + (first ? (has_alt ? 25.f : 25.67f) : has_alt ? 12.f : 14.f);
            const float at_off = (!first && has_alt) ? 10.f : (first && has_alt) ? 8.f : 8.f;
            pm::field(c, ab(content, 156.67f, ct + at_off, 314, ct + at_off + 40), at(r, "alt"), ui::Align::Left, 90);
            ui::stroke_box(c, ab(content, 372, ct, 602.67f, ct + 54.67f), 0xFF4F5467, 1);
            pm::field(c, ab(content, 377.33f, ct + 5.33f, 475.33f, ct + 47.33f), at(r, "dir"), ui::Align::Center, 54);
            pm::field(c, ab(content, 481.33f, ct + 5.33f, 596.67f, ct + 47.33f), at(r, "spd"), ui::Align::Center, 54);
        }
        y += h;
    }
    if (has_alt) draw_alt_row(c, content, at(page, "alternate"));
    const json& b = at(page, "button");
    if (b.is_object() && !flag(b, "hidden")) {
        json bb = b;
        bb["asterisk"] = true;
        const float bx = -0.67f;
        pm::button(c, ab(content, 190 + bx, 895.8f + kTop - 130.f, 368 + bx, 953.2f + kTop - 130.f), bb, 10.3f);
    }
    if (flag(page, "returnButton")) pm::button(c, ab(content, 4, 915.8f + kTop - 130.f, 129, 953.2f + kTop - 130.f), json{{"label", "RETURN"}}, 12);
    if (is_crz && at(crz, "dropdown").is_object())
    {
        const json& d = at(crz, "dropdown");
        pm::dropdown_open_overlay(c, ab(content, 331.2f, 211.7f, 523.2f, 256.f), json{{"open", flag(d, "open")}, {"text", str(d, "text")}, {"items", json::array()}});
        centered_menu(c, ab(content, 331.2f, 211.7f, 523.2f, 256.f), d);
    }
}

}  // namespace

A380_MFD_PAGE("fms/active/wind", draw_wind);

}  // namespace a380::mfd
