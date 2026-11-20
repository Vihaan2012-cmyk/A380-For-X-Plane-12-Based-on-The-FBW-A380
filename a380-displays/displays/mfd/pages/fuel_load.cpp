#include "page_model.h"

namespace a380::mfd {
namespace {

using namespace pm;

constexpr float kTop = 131.33f;

SkRect ab(SkRect content, float x0, float y0, float x1, float y1) {
    return box(content, x0, y0 - kTop, x1, y1 - kTop);
}

void tx(SkCanvas& c, SkRect content, std::string_view s, float x, float cy, float size, SkColor color, ui::Align a) {
    ui::TextStyle ts = ui::label_style(color, size);
    ts.align = a;
    const float y = content.top() + cy - kTop - 0.67f;
    const float tw = ui::text_width(s, ts);
    ui::text_in_aligned(c, s, SkRect::MakeLTRB(content.left() + x - (a == ui::Align::Right ? 400 : 0) - (a == ui::Align::Center ? 200 : 0), y - 20,
                                               content.left() + x + (a == ui::Align::Left ? 400 : 0) + (a == ui::Align::Center ? 200 : 0), y + 20),
                        ts);
}

void vu(SkCanvas& c, SkRect content, const json& v, float x, float cy, bool centered = false, float right = -1) {
    if (flag(v, "hidden")) return;
    const std::string t = str(v, "text");
    const std::string u = str(v, "unit_trailing");
    SkColor col = ui::pal::green;
    for (const auto& k : at(v, "classes")) {
        const std::string s = k.get<std::string>();
        if (s == "white") col = ui::pal::white;
        else if (s == "amber") col = ui::pal::amber;
        else if (s == "cyan") col = ui::pal::cyan;
        else if (s == "magenta") col = ui::pal::magenta;
    }
    ui::TextStyle vs = ui::value_style(col), us = ui::unit_style();
    const float w = ui::text_width(t, vs) + (u.empty() ? 0.f : 6.f + ui::text_width(u, us));
    if (right >= 0) x = right - w;
    else if (centered) x -= w * 0.5f;
    const float lh = std::max(ui::line_height(vs), ui::line_height(us));
    ui::value_with_unit(c, t, u, content.left() + x, content.top() + cy - kTop - 0.67f - lh * 0.5f, col, false);
}

void big_green(SkCanvas& c, SkRect content, const json& t, float x, float cy, ui::Align a = ui::Align::Left) {
    tx(c, content, str(t, "text"), x, cy, 24, ui::pal::green, a);
}

void draw_fuel_load(SkCanvas& c, SkRect content, const json& page) {
    pm::GreyLayer grey(c);
    const json& p = page;
    SkRect r;

    tx(c, content, "GW", 32.3, 159.7, 20, ui::pal::white, ui::Align::Left);
    vu(c, content, at(p, "gw"), 72, 159.7);
    tx(c, content, "CG", 317, 159.7, 20, ui::pal::white, ui::Align::Left);
    vu(c, content, at(p, "cg"), 357, 159.7);
    tx(c, content, "FOB", 586.67, 159.7, 20, ui::pal::white, ui::Align::Left);
    vu(c, content, at(p, "fob"), 639, 159.7);

    r = ab(content, 190, 187.33, 340, 224.67);
    tx(c, content, "ZFW", 175, 206, 20, ui::pal::white, ui::Align::Right);
    pm::field(c, r, at(p, "zfw"), ui::Align::Right, 90);
    tx(c, content, "ZFWCG", 515, 206, 20, ui::pal::white, ui::Align::Right);
    pm::field(c, ab(content, 530, 187.33, 655.33, 224.67), at(p, "zfwcg"), ui::Align::Center, 72);

    tx(c, content, "BLOCK", 175, 267.7, 20, ui::pal::white, ui::Align::Right);
    pm::field(c, ab(content, 190, 249.33, 340, 286), at(p, "block"), ui::Align::Right, 90);
    pm::button(c, ab(content, 478, 239.33, 630, 296), at(p, "fuel_planning"), 2);

    thin_line(c, content, 0, 768, 316.0f);

    tx(c, content, "TAXI", 110.3, 351, 20, ui::pal::white, ui::Align::Right);
    pm::field(c, ab(content, 125.33, 332, 275.33, 368.67), at(p, "taxi"), ui::Align::Right, 90);
    tx(c, content, "TRIP", 110.3, 406.8, 20, ui::pal::white, ui::Align::Right);
    vu(c, content, at(p, "trip_fuel"), 0, 407, false, 273);
    vu(c, content, at(p, "trip_time"), 352.6, 407, true);
    tx(c, content, "RTE RSV", 110.3, 463.2, 20, ui::pal::white, ui::Align::Right);
    pm::field(c, ab(content, 125.33, 444.67, 275.33, 482), at(p, "rte_rsv"), ui::Align::Right, 90);
    pm::field(c, ab(content, 285.33, 444.67, 405.33, 482), at(p, "rte_rsv_pct"), ui::Align::Center, 72);
    tx(c, content, "ALTN", 110.3, 521, 20, ui::pal::white, ui::Align::Right);
    pm::field(c, ab(content, 125.33, 502, 275.33, 538.67), at(p, "altn_fuel"), ui::Align::Right, 90);
    vu(c, content, at(p, "altn_time"), 352.6, 521, true);
    tx(c, content, "FINAL", 110.3, 577.2, 20, ui::pal::white, ui::Align::Right);
    pm::field(c, ab(content, 125.33, 558.67, 275.33, 596), at(p, "final_fuel"), ui::Align::Right, 90);
    pm::field(c, ab(content, 285.33, 558.67, 405.33, 596), at(p, "final_time"), ui::Align::Center, 72);

    tx(c, content, "PAX NBR", 538.3, 351, 20, ui::pal::white, ui::Align::Right);
    pm::field(c, ab(content, 553.33, 332, 628, 368.67), at(p, "pax"), ui::Align::Center, 54);
    tx(c, content, "MODE", 538.3, 401.3, 20, ui::pal::white, ui::Align::Right);
    const SkRect mode_r = ab(content, 553.33, 379.33, 728, 423.33);
    dropdown_box(c, mode_r, at(p, "mode"));
    tx(c, content, "CI", 538.3, 452.4, 20, ui::pal::white, ui::Align::Right);
    pm::field(c, ab(content, 553.33, 433.33, 628, 470.67), at(p, "ci"), ui::Align::Center, 54);
    tx(c, content, "JTSN GW", 538.3, 499.7, 20, ui::pal::white, ui::Align::Right);
    pm::field(c, ab(content, 553.33, 480.67, 703.33, 518), at(p, "jtsn_gw"), ui::Align::Right, 90);
    tx(c, content, "TOW", 538.3, 546.4, 20, ui::pal::white, ui::Align::Right);
    vu(c, content, at(p, "tow"), 673, 546.4, true);
    tx(c, content, "LW", 538.3, 593.1, 20, ui::pal::white, ui::Align::Right);
    vu(c, content, at(p, "lw"), 673.7, 593, true);

    thin_line(c, content, 0, 768, 620.7f);

    ui::TextStyle vs = ui::value_style(ui::pal::green);
    ui::TextStyle us = ui::unit_style();
    const float w5 = ui::text_width("---.-", vs);
    const float wd = std::max(ui::text_width(str(at(p, "dest_efob"), "text"), vs), ui::text_width(str(at(p, "altn_efob"), "text"), vs));
    const float s = std::max(0.f, (w5 - wd) / 16.4f);

    tx(c, content, "TIME", 237.67 + (s > 0 ? 10.3f : 0.f), 681, 20, ui::pal::white, ui::Align::Left);
    tx(c, content, "EFOB", 376.67 + 6.f * s, 681, 20, ui::pal::white, ui::Align::Left);
    tx(c, content, "MIN FUEL AT DEST", 624.3, 681, 20, ui::pal::white, ui::Align::Center);
    thin_line(c, content, 0, 480, 699.3f);

    tx(c, content, "DEST", 23 + (s > 0 ? 2.4f : 0.f), s > 0 ? 732.33f : 733.03f, 20, ui::pal::white, ui::Align::Left);
    big_green(c, content, at(p, "dest_ident"), 118.7 + (s > 0 ? 6.6f : 0.f), 733.5);
    big_green(c, content, at(p, "dest_time"), 225 + 10.3f * s, 733.5);
    vu(c, content, at(p, "dest_efob"), 353 + 14.3f * s, 733.5);
    tx(c, content, "ALTN", 23 + (s > 0 ? 2.4f : 0.f), 785, 20, ui::pal::white, ui::Align::Left);
    big_green(c, content, at(p, "altn_ident"), 118.7 + (s > 0 ? 6.6f : 0.f), 784.7);
    big_green(c, content, at(p, "altn_time2"), 225 + 10.3f * s, 784.67f);
    vu(c, content, at(p, "altn_efob"), 353 + 14.3f * s - (s > 0 ? 2.f : 0.f), s > 0 ? 776.6f : 779.3f);

    pm::field(c, ab(content, 549.33, 709.33, 699.33, 746.67), at(p, "min_dest"), ui::Align::Right, 90);
    tx(c, content, "EXTRA", 624, 785.7, 20, ui::pal::white, ui::Align::Center);
    {
        const json& ef = at(p, "extra_fuel");
        const json& et = at(p, "extra_time");
        const float uw = ui::text_width("T", us);
        const float t0 = w5 + 6 + uw + 20 + ui::text_width("--:--", vs);
        const float vw = ui::text_width(str(ef, "text"), vs);
        const float t1 = vw + 6 + uw + 20 + ui::text_width(str(et, "text"), vs);
        const float start = 526.7f + (t0 - t1) * 0.5f;
        vu(c, content, ef, start, 817.7);
        vu(c, content, et, start + (vw - w5) + 124.f, 817.7);
    }

    pm::button(c, ab(content, 0, 922, 145, 959.33), at(p, "return"));
    dropdown_open_overlay(c, mode_r, at(p, "mode"));
}

}  // namespace

A380_MFD_PAGE("fms/active/fuel-load", draw_fuel_load);

}  // namespace a380::mfd
