#include "page_model.h"
#include "pp_layout.h"

namespace a380::mfd {
namespace {
using namespace ui;
using nlohmann::json;

void draw_token_page(SkCanvas& c, SkRect content, const json& d) {
    std::vector<std::string> tabs;
    int sel = 0;
    std::string active = pp::str(d, "tab");
    if (d.contains("tabs") && d["tabs"].is_array())
        for (const json& t : d["tabs"]) {
            tabs.push_back(t.get<std::string>());
            if (tabs.back() == active) sel = static_cast<int>(tabs.size()) - 1;
        }
    SkRect in = content;
    if (!tabs.empty()) {
        TabStyle ts;
        ts.selected = sel;
        in = tab_navigator(c, content, tabs, ts);
    }
    if (!d.contains("tokens") || !d["tokens"].is_array()) return;

    float y = in.fTop + 8;
    float col = 0;
    float colW = in.width() * 0.5f;
    float rowH = 46;
    float x = in.fLeft;
    bool prev_label = false;
    for (const json& t : d["tokens"]) {
        std::string k = pp::str(t, "k");
        bool was_label = prev_label;
        prev_label = (k == "label");
        (void)was_label;
        float cx = in.fLeft + col;
        if (k == "label") {
            if (prev_label) y += 28;
            label(c, pp::str(t, "v"), cx + 6, y + 8, pal::white, 20);
        } else if (k == "value") {
            value_with_unit(c, pp::str(t, "v"), pp::str(t, "unit"), cx + colW * 0.55f, y + 6, pal::green);
            y += rowH;
        } else if (k == "input") {
            FieldStyle fs = pp::field_style(t);
            input_field(c, SkRect::MakeXYWH(cx + colW * 0.5f, y, colW * 0.45f, 37), pp::str(t, "v"), fs);
            y += rowH;
        } else if (k == "dropdown") {
            DropdownStyle ds;
            ds.disabled = pp::flag(t, "disabled");
            dropdown(c, SkRect::MakeXYWH(cx + colW * 0.5f, y, colW * 0.45f, 37), pp::str(t, "v"), ds);
            y += rowH;
        } else if (k == "button") {
            ButtonStyle bs;
            if (pp::flag(t, "disabled")) bs.state = ButtonState::Disabled;
            std::string v = pp::str(t, "v");
            auto nl = v.find('\n');
            SkRect r = SkRect::MakeXYWH(cx + 6, y, 170, nl == std::string::npos ? 41 : 60);
            if (nl != std::string::npos) button_lines(c, r, {v.substr(0, nl), v.substr(nl + 1)}, bs);
            else button(c, r, v, bs);
            y += rowH + (nl == std::string::npos ? 0 : 20);
        } else if (k == "radio") {
            radio_button(c, cx + 6, y, pp::str(t, "v"), pp::flag(t, "checked"), pal::cyan, pp::flag(t, "disabled"));
            y += rowH;
        }
        if (y > in.fBottom - rowH && col == 0) {
            y = in.fTop + 8;
            col = colW;
        }
    }
    (void)x;
}


struct TokCursor {
    const json& toks;
    size_t i = 0;
    const json* take(const char* kind) {
        while (i < toks.size()) {
            const json& t = toks[i++];
            if (pp::str(t, "k") == kind) return &t;
        }
        return nullptr;
    }
};

ui::FieldStyle navaid_field(const json* t) {
    ui::FieldStyle st;
    if (!t) return st;
    st.unit_leading = pp::str(*t, "lead");
    st.unit_trailing = pp::str(*t, "trail");
    if (pp::str(*t, "state") == "disabled") st.kind = ui::FieldKind::Disabled;
    if (pp::str(*t, "state") == "mandatory") st.kind = ui::FieldKind::Mandatory;
    return st;
}

void nav_label(SkCanvas& c, std::string_view s, float x, float cy, ui::Align a = ui::Align::Left) {
    ui::TextStyle ts = ui::label_style(ui::pal::white, 20);
    ts.align = a;
    ui::text(c, s, x, cy - ui::line_height(ts) * 0.5f, ts);
}

void nav_value(SkCanvas& c, std::string_view s, float x, float cy, ui::Align a) {
    ui::TextStyle ts = ui::value_style(ui::pal::green, 25);
    ts.align = a;
    ui::text(c, s, x, cy - ui::line_height(ts) * 0.5f, ts);
}

std::string tok_v(const json* t) { return t ? pp::str(*t, "v") : std::string(); }

void draw_pos_navaids(SkCanvas& c, SkRect content, const json& d) {
    std::vector<std::string> tabs;
    int sel = 0;
    const std::string active = pp::str(d, "tab");
    if (d.contains("tabs") && d["tabs"].is_array())
        for (const json& t : d["tabs"]) {
            tabs.push_back(t.get<std::string>());
            if (tabs.back() == active) sel = static_cast<int>(tabs.size()) - 1;
        }
    static const json none = json::array();
    const json& toks = d.contains("tokens") && d["tokens"].is_array() ? d["tokens"] : none;
    TokCursor tc{toks};
    const float t0 = content.top() + 47.67f;
    const float frame_h = sel == 0 ? 364.07f : 421.4f;
    SkRect nav = SkRect::MakeLTRB(content.left(), content.top(), content.right(), t0 + frame_h);
    ui::TabStyle tstyle;
    tstyle.selected = sel;
    tstyle.slant_deg = 25;
    SkRect in = ui::tab_navigator(c, nav, tabs, tstyle);
    const float line = 1.f / 1.5f;
    const float w = in.width();

    if (sel == 0) {
        const float cx = in.centerX();
        const float colx[2] = {cx - 133.75f, cx + 133.75f};
        for (int col = 0; col < 2; ++col) {
            nav_label(c, tok_v(tc.take("label")), colx[col], t0 + 25, ui::Align::Center);
            for (int r = 0; r < 3; ++r) {
                const json* f = tc.take("input");
                const float top = t0 + 50 * (r + 1) + 6.5f;
                ui::input_field(c, SkRect::MakeXYWH(colx[col] - 62.5f, top, 125, 37), tok_v(f), navaid_field(f));
            }
            nav_value(c, tok_v(tc.take("value")), colx[col], t0 + 225, ui::Align::Center);
            if (col == 0)
                for (int r = 0; r < 5; ++r) nav_label(c, tok_v(tc.take("label")), cx, t0 + 25 + 50 * r, ui::Align::Center);
        }
        ui::fill(c, SkRect::MakeLTRB(in.left(), t0 + 273.67f, in.right(), t0 + 275.67f), ui::pal::light_grey);
    } else {
        const float x0 = in.left(), tw = w * 0.9f;
        const float cw[3] = {tw * 25 / 90, tw * 32.5f / 90, tw * 32.5f / 90};
        const float cl[3] = {x0, x0 + cw[0], x0 + cw[0] + cw[1]};
        const char* head[3] = {"IDENT", "FREQ/CHAN", "CLASS"};
        for (int k = 0; k < 3; ++k) {
            nav_label(c, head[k], cl[k] + cw[k] * 0.5f, t0 + 20.67f, ui::Align::Center);
            tc.take("label");
        }
        ui::fill(c, SkRect::MakeLTRB(x0, t0 + 40 - line, x0 + tw, t0 + 40), ui::pal::light_grey);
        for (int k = 0; k < 2; ++k)
            ui::fill(c, SkRect::MakeLTRB(cl[k] + cw[k] - line, t0, cl[k] + cw[k], t0 + 160), ui::pal::light_grey);
        for (int r = 0; r < 3; ++r) {
            const float top = t0 + 40 + 40 * r;
            tc.take("label");
            const std::string bv = tok_v(tc.take("button"));
            const std::string fq = tok_v(tc.take("value"));
            const std::string cls = tok_v(tc.take("value"));
            if (!bv.empty()) {
                ui::ButtonStyle bs;
                bs.state = ui::ButtonState::Disabled;
                bs.arrow = true;
                ui::button(c, SkRect::MakeXYWH(cl[0] + cw[0] * 0.5f - 53.5f, top + 2, 107, 36), bv, bs);
            }
            nav_value(c, fq, cl[1] + cw[1] * 0.5f, top + 20, ui::Align::Center);
            nav_value(c, cls, cl[2] + cw[2] * 0.5f, top + 20, ui::Align::Center);
        }
        const std::string l1 = tok_v(tc.take("label")), v1 = tok_v(tc.take("value"));
        const std::string l2 = tok_v(tc.take("label")), v2 = tok_v(tc.take("value"));
        nav_label(c, l1, x0 + 80, t0 + 188.73f);
        nav_value(c, v1, x0 + 290, t0 + 188.73f, ui::Align::Left);
        nav_label(c, l2, x0 + 80, t0 + 226.73f);
        nav_value(c, v2, x0 + 290, t0 + 226.73f, ui::Align::Left);
        ui::fill(c, SkRect::MakeLTRB(x0, t0 + 259.4f - line, x0 + w, t0 + 259.4f), ui::pal::light_grey);
        nav_label(c, tok_v(tc.take("label")), x0 + 15, t0 + 283.33f);
        const float fw = 92, gap = (w * 0.45f - 3 * fw) * 0.5f;
        for (int r = 0; r < 2; ++r)
            for (int k = 0; k < 3; ++k) {
                const json* f = tc.take("input");
                ui::input_field(c, SkRect::MakeXYWH(x0 + k * (fw + gap), t0 + 302.7f + 46.67f * r, fw, 37), tok_v(f),
                                navaid_field(f));
            }
    }

    for (int k = 0; k < 6; ++k) tc.take("label");
    const std::string ls = tok_v(tc.take("label"));
    const float ls_top = nav.bottom() + (sel == 0 ? 2.17f : 0.17f);
    const float lw = ui::text_width("FREQ/CHAN", ui::label_style(ui::pal::white, 20));
    const float fx = content.left() + (content.width() - (lw + 385)) * 0.5f + lw + 20;
    nav_label(c, ls.empty() ? "LS" : ls, fx + 62.5f, ls_top + 25, ui::Align::Center);
    const char* names[5] = {"IDENT", "FREQ/CHAN", "CRS", "SLOPE", "CLASS"};
    for (int r = 0; r < 5; ++r) nav_label(c, names[r], fx - 20, ls_top + 75 + 50 * r, ui::Align::Right);
    for (int r = 0; r < 3; ++r) {
        const json* f = tc.take("input");
        ui::input_field(c, SkRect::MakeXYWH(fx, ls_top + 50 * (r + 1) + 6.5f, 125, 37), tok_v(f), navaid_field(f));
    }
    const std::string sv = tok_v(tc.take("value"));
    const std::string su = tok_v(tc.take("unit"));
    const std::string cv = tok_v(tc.take("value"));
    ui::TextStyle vs = ui::value_style(ui::pal::green, 25);
    const float sw = ui::text_width(sv, vs) + (su.empty() ? 0.f : ui::text_width(su, ui::unit_style(20)));
    ui::value_with_unit(c, sv, su, fx + 62.5f - sw * 0.5f, ls_top + 175 - ui::line_height(vs) * 0.5f);
    nav_value(c, cv, fx + 62.5f, ls_top + 225, ui::Align::Center);
    SkRect br = SkRect::MakeXYWH(fx + 125 + 15, ls_top + 150 - 28, 225, 56);
    pm::button(c, br, json{{"label", "DESELECT\nGLIDE"}, {"state", "disabled"}, {"asterisk", true}});
}

void draw_pos_monitor(SkCanvas& c, SkRect content, const json& d) {
    static const json none = json::array();
    const json& toks = d.contains("tokens") && d["tokens"].is_array() ? d["tokens"] : none;
    TokCursor tc{toks};
    const float oy = content.top() - 4.67f;
    auto Y = [&](float v) { return oy + v; };
    auto big_label = [&](std::string_view s, float x, float cy) {
        ui::TextStyle ts = ui::label_style(ui::pal::white, 24);
        ui::text(c, s, x, Y(cy) - ui::line_height(ts) * 0.5f, ts);
    };
    auto big_value = [&](std::string_view s, float x, float cy, SkColor col = ui::pal::green) {
        ui::TextStyle ts = ui::value_style(col, 30);
        ui::text(c, s, x, Y(cy) - ui::line_height(ts) * 0.5f, ts);
    };
    auto big_unit = [&](std::string_view s, float x, float cy) {
        ui::TextStyle ts = ui::unit_style(24);
        ui::text(c, s, x, Y(cy) - ui::line_height(ts) * 0.5f, ts);
    };
    auto rule = [&](float cy) {
        const float s = c.getTotalMatrix().getScaleY();
        const float top = std::round(Y(cy) * s) / s;
        ui::fill(c, SkRect::MakeLTRB(5, top, 5 + content.width() * 0.97f, top + 2.f / s), ui::pal::light_grey);
    };

    big_value(tok_v(tc.take("value")), 92.67f, 40);
    big_label(tok_v(tc.take("label")), 453.7f, 37);
    {
        const json* f = tc.take("input");
        ui::FieldStyle fs = navaid_field(f);
        ui::input_field(c, SkRect::MakeXYWH(514.6f, Y(19.2f), 154.5f, 37), tok_v(f), fs);
    }
    big_label(tok_v(tc.take("label")), 453.7f, 86.8f);
    big_value(tok_v(tc.take("value")), 535.85f, 86.8f + 0.67f);
    big_unit(tok_v(tc.take("unit")), 636, 86.8f);
    for (int r = 0; r < 2; ++r) {
        const float cy = 131.27f + 55.17f * r;
        big_label(tok_v(tc.take("label")), 16.f, cy);
        big_value(tok_v(tc.take("value")), 106, cy + 0.5f);
        big_value(tok_v(tc.take("value")), 700, cy + 0.5f);
    }
    rule(209.9f);
    big_label(tok_v(tc.take("label")), 472.9f, 300.5f);
    for (int r = 0; r < 3; ++r) {
        const float cy = 334.5f + 33.67f * r;
        const std::string lbl = tok_v(tc.take("label"));
        big_label(lbl, 472.9f + (lbl == "RNP" || lbl == "EPU" ? 0.73f : 0.f), cy);
        big_value(tok_v(tc.take("value")), 597.0f, cy);
        big_unit(tok_v(tc.take("unit")), 659.4f, cy);
    }
    tc.take("label");
    tc.take("label");
    auto split2 = [](const std::string& s, const char* tail) {
        std::string a = s;
        if (!a.empty() && a.back() == '*') a.pop_back();
        const size_t p = a.rfind(tail);
        std::string head = p == std::string::npos ? a : a.substr(0, p);
        return std::pair<std::string, std::string>(head, tail);
    };
    {
        auto [h, t] = split2(tok_v(tc.take("button")), "POS SENSORS");
        pm::button(c, SkRect::MakeXYWH(96.67f, Y(523.8f) + 0.67f, 217.5f, 57), json{{"label", h + "\n" + t}});
    }
    {
        auto [h, t] = split2(tok_v(tc.take("button")), "POS DATA");
        pm::button(c, SkRect::MakeXYWH(496.67f, Y(523.8f), 211, 57), json{{"label", h + "\n" + t}, {"asterisk", true}});
    }
    rule(600.6f);
    pm::button(c, SkRect::MakeXYWH(0, Y(629.3f), 138, 58), json{{"label", "POSITION\nUPDATE"}, {"state", "disabled"}});
    {
        ui::TextStyle ts = ui::label_style(ui::pal::white, 22);
        ui::text(c, tok_v(tc.take("label")), 409.69f, Y(636.7f) - ui::line_height(ts) * 0.5f, ts);
        const json* f = tc.take("input");
        ui::FieldStyle bs = navaid_field(f);
        bs.font_size = 22;
        ui::input_field(c, SkRect::MakeXYWH(600, Y(618.2f), 145, 37), tok_v(f), bs);
    }
    nav_value(c, tok_v(tc.take("value")), 351, Y(673.8f), ui::Align::Left);
    tc.take("unit");
    nav_value(c, tok_v(tc.take("value")), 449.11f, Y(673.8f), ui::Align::Left);
    nav_value(c, tok_v(tc.take("value")), 478.79f, Y(673.8f), ui::Align::Left);
    rule(721.7f);

    const float by = content.bottom() - 43 + 0.5f;
    ui::button(c, SkRect::MakeXYWH(335, by, 149, 43), "NAVAIDS");
    ui::ButtonStyle gs;
    gs.state = ui::ButtonState::Disabled;
    ui::button(c, SkRect::MakeXYWH(489, by, 133, 43), "GNSS", gs);
    ui::button(c, SkRect::MakeXYWH(627, by, 136, 43), "IRS");
}

struct DataEntry {
    float frame_bottom[2];
    float label_shift;
};

void draw_data_entry(SkCanvas& c, SkRect content, const json& d, const DataEntry& e) {
    std::vector<std::string> tabs;
    int sel = 0;
    const std::string active = pp::str(d, "tab");
    if (d.contains("tabs") && d["tabs"].is_array())
        for (const json& t : d["tabs"]) {
            tabs.push_back(t.get<std::string>());
            if (tabs.back() == active) sel = static_cast<int>(tabs.size()) - 1;
        }
    static const json none = json::array();
    const json& toks = d.contains("tokens") && d["tokens"].is_array() ? d["tokens"] : none;
    TokCursor tc{toks};
    ui::TabStyle ts;
    ts.selected = sel;
    ts.slant_deg = 25;
    const SkRect nav = SkRect::MakeLTRB(content.left(), content.top(), content.right(), e.frame_bottom[sel] + 2);
    const SkRect in = ui::tab_navigator(c, nav, tabs, ts);
    const float cx = in.centerX();
    if (sel == 0) {
        const std::string label = tok_v(tc.take("label"));
        const json* f = tc.take("input");
        const ui::FieldStyle fs = navaid_field(f);
        const std::string fv = tok_v(f);
        const float row_top = in.top() + 20;
        const float row_h = ui::button_height(22);
        const ui::TextStyle ls = ui::label_style(ui::pal::white, 20);
        const float lw = ui::text_width(label, ls);
        const float fw = std::max(92.f, ui::input_field_width(fv, fs));
        const float left = cx - (lw + fw) * 0.5f;
        nav_label(c, label, left - e.label_shift, row_top + row_h * 0.5f);
        ui::input_field(c, SkRect::MakeXYWH(left + lw - 60, row_top, fw, row_h), fv, fs);
        const json* rb = tc.take("button");
        if (rb && pp::str(*rb, "v") == "RETURN") {
            ui::ButtonStyle bs;
            ui::button(c, SkRect::MakeXYWH(content.left(), content.bottom() - row_h, 145.33f, row_h), "RETURN", bs);
        }
    } else {
        nav_label(c, tok_v(tc.take("label")), cx, in.top() + 10 + 45 + 6.28f, ui::Align::Center);
    }
}

void draw_waypoint(SkCanvas& c, SkRect r, const json& d) { draw_data_entry(c, r, d, {{602.67f, 606.f}, 65.f}); }
void draw_navaid(SkCanvas& c, SkRect r, const json& d) { draw_data_entry(c, r, d, {{602.67f, 583.33f}, 90.f}); }
void draw_airport(SkCanvas& c, SkRect r, const json& d) { draw_data_entry(c, r, d, {{644.f, 564.67f}, 90.f}); }
std::string demoji(std::string s) {
    const std::string bad = "\xC3\x82\xC2\xB0";
    for (size_t p; (p = s.find(bad)) != std::string::npos;) s.replace(p, bad.size(), "\xC2\xB0");
    return s;
}

struct IrsTokens {
    std::vector<json> t;
    size_t cur = 0;
    explicit IrsTokens(const json& d) {
        if (d.contains("tokens") && d["tokens"].is_array())
            for (const json& x : d["tokens"]) {
                json o = x;
                o["v"] = demoji(pm::str(x, "v"));
                t.push_back(o);
            }
    }
    json next(const char* kind) {
        while (cur < t.size()) {
            const json& x = t[cur++];
            if (pm::str(x, "k") == kind) return x;
        }
        return json::object();
    }
    std::vector<std::pair<std::string, bool>> parts() {
        std::vector<std::pair<std::string, bool>> out;
        while (cur < t.size()) {
            const std::string k = pm::str(t[cur], "k");
            if (k != "value" && k != "unit") break;
            out.push_back({pm::str(t[cur], "v"), k == "unit"});
            ++cur;
        }
        return out;
    }
};

void draw_pos_irs(SkCanvas& c, SkRect r, const json& d) {
    IrsTokens tk(d);
    auto sv = [](const json& t) { return pm::str(t, "v"); };
    const float W = r.width(), X = r.left(), Y = r.top();
    const TextStyle big = value_style(pal::green, 30);
    auto css_lh = [](float size) { return std::round(size * 0.8476f) + std::round(size * 0.0456f); };
    const float lh30 = 26.667f;
    const float lh20 = css_lh(20);
    const float bh1 = button_height(22, 1), bh2 = button_height(22, 2);
    auto two_line = [&](float x, float y, const json& b, const char* l1, const char* l2) {
        ButtonStyle st;
        if (pm::flag(b, "disabled")) st.state = ButtonState::Disabled;
        const TextStyle bs = label_style(pal::white, 22);
        const float w = std::max(text_width(l1, bs), text_width(l2, bs)) + 4 + 24;
        button_lines(c, SkRect::MakeXYWH(x, y, w, bh2), {l1, l2}, st);
    };
    auto parts_width = [&](const std::vector<std::pair<std::string, bool>>& ps) {
        float w = 0;
        for (auto& p : ps) w += text_width(p.first, p.second ? unit_style(20) : big) + (p.second ? 6.f : 0.f);
        return w;
    };
    auto draw_parts = [&](float x, float top, const std::vector<std::pair<std::string, bool>>& ps) {
        for (auto& p : ps) {
            const TextStyle st = p.second ? unit_style(20) : big;
            text(c, p.first, x, top + (lh30 - css_lh(p.second ? 20.f : 30.f)) * 0.5f, st);
            x += text_width(p.first, st);
        }
    };

    const json al = tk.next("label"), apos = tk.next("value");
    float y = Y + 15;
    const float vw = text_width(sv(apos), big);
    TextStyle lg = label_style(pal::amber, 20);
    text(c, sv(al), X + 15 + (W - 30 - vw - text_width(sv(al), lg)) * 0.5f, y + (lh30 - lh20) * 0.5f, lg);
    text(c, sv(apos), X + W - 15 - vw, y, big);
    y += lh30 + 15;

    two_line(X, y, tk.next("button"), "ALIGN ON", "OTHER REF");
    y += bh2 + 20;
    fill(c, SkRect::MakeXYWH(X, y, W, 2), 0xFFD3D3D3);
    y += 2 + 40;

    const float cw[4] = {0.12f * W, 0.20f * W, 0.38f * W, 0.30f * W};
    for (int row = 0; row < 3; ++row) {
        const float rh = 14 + lh30 + (row < 2 ? 2 : 0);
        std::string cells[4];
        cells[0] = sv(tk.next("label"));
        for (int ci = 1; ci < 4; ++ci) cells[ci] = sv(tk.next("value"));
        float x = X;
        for (int ci = 0; ci < 4; ++ci) {
            if (ci < 3) fill(c, SkRect::MakeXYWH(x + cw[ci] - 2, y, 2, rh), pal::dark_grey);
            if (row < 2) fill(c, SkRect::MakeXYWH(x, y + rh - 2, cw[ci], 2), pal::dark_grey);
            if (!cells[ci].empty()) {
                const float inner = cw[ci] - (ci < 3 ? 2 : 0);
                TextStyle st = ci == 0 ? label_style(pal::white, 20) : ci == 1 ? big : value_style(pal::green, 25);
                st.align = Align::Center;
                text(c, cells[ci], x + inner * 0.5f, y + 7 + (lh30 - line_height(st)) * 0.5f, st);
            }
            x += cw[ci];
        }
        y += rh;
    }
    y += 10 + 37 + 20;

    {
        float x = X + (W - 456) * 0.5f;
        for (int i = 0; i < 3; ++i) {
            const json b = tk.next("button");
            ButtonStyle st;
            if (pm::flag(b, "selected")) st.state = ButtonState::Selected;
            button(c, SkRect::MakeXYWH(x, y, 150, bh1), sv(b), st);
            x += 153;
        }
    }
    y += bh1 + 3;

    const float rowH = 14 + lh30;
    const float boxBottom = y + 4 + bh2 + 15 + 3 * rowH + 2 + 15 + 2 * rowH + 15 + 2;
    SkRect box = SkRect::MakeLTRB(X, y, X + W, boxBottom);
    bevel_border(c, box, Bevel::Outset, pal::light_grey);
    const float ix = X + 2 + 15;
    const float iw = W - 4 - 30;
    float by = y + 4;
    two_line(ix, by, tk.next("button"), "FREEZE", "ALL IRS");
    const float rowAbottom = by + bh2;
    const json plab = tk.next("label"), ppos = tk.next("value");
    {
        const float right = X + W - 2 - 15;
        const float pw = text_width(sv(ppos), big);
        text(c, sv(ppos), right - pw, rowAbottom - lh30, big);
        TextStyle pl = label_style(pal::white, 20);
        pl.align = Align::Right;
        text(c, sv(plab), right - pw - 20, rowAbottom - lh30 + 30 * 0.8476f - 20 * 0.8476f, pl);
    }
    by = rowAbottom + 15;
    const float e1 = 14 + (iw - 56) / 4.5f, e2 = e1 + 14 + (iw - 56) * 1.5f / 4.5f, e3 = e2 + 14 + (iw - 56) / 4.5f;
    for (int row = 0; row < 3; ++row) {
        const json l1 = tk.next("label");
        auto a = tk.parts();
        const json l2 = tk.next("label");
        auto b = tk.parts();
        TextStyle ls = label_style(pal::white, 20);
        ls.align = Align::Right;
        text(c, sv(l1), ix + e1 - 7, by + 7 + (lh30 - lh20) * 0.5f, ls);
        draw_parts(ix + e2 - 7 - parts_width(a), by + 7, a);
        text(c, sv(l2), ix + e3 - 7, by + 7 + (lh30 - lh20) * 0.5f, ls);
        draw_parts(ix + iw - 7 - parts_width(b), by + 7, b);
        by += rowH;
        if (row == 2) {
            fill(c, SkRect::MakeXYWH(ix, by + 0.67f, iw, 2), 0xFFD3D3D3);
            by += 2 + 15;
        }
    }
    {
        const float right = X + W - 2 - 15 - 15;
        const json gl = tk.next("label"), gv = tk.next("value");
        text(c, sv(gv), right - 7 - 325, by + 7, big);
        TextStyle ls = label_style(pal::white, 20);
        ls.align = Align::Right;
        text(c, sv(gl), right - 7 - 325 - 15, by + 7 + (lh30 - lh20) * 0.5f, ls);
        by += rowH;
        const json ll = tk.next("label"), lv = tk.next("value"), lu = tk.next("unit");
        TextStyle vs = big;
        vs.align = Align::Right;
        text(c, sv(lv), right - 7 - 25, by + 7, vs);
        text(c, sv(lu), right - 7 - 25, by + 7 + (lh30 - line_height(unit_style(20))) * 0.5f, unit_style(20));
        text(c, sv(ll), right - 7 - 25 - 300 - 15, by + 7 + (lh30 - lh20) * 0.5f, ls);
    }
    {
        const json rb = tk.next("button");
        ButtonStyle st;
        button(c, SkRect::MakeXYWH(X, r.bottom() - bh1, 150, bh1), sv(rb), st);
    }
}

A380_MFD_PAGE("fms/data/waypoint", draw_waypoint);
A380_MFD_PAGE("fms/data/navaid", draw_navaid);
A380_MFD_PAGE("fms/data/airport", draw_airport);
A380_MFD_PAGE("fms/position/monitor", draw_pos_monitor);
A380_MFD_PAGE("fms/position/navaids", draw_pos_navaids);
A380_MFD_PAGE("fms/position/irs", draw_pos_irs);
}  // namespace
}  // namespace a380::mfd
