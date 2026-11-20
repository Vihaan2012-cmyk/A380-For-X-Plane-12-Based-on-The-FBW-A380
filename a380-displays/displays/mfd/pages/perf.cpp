#include "include/core/SkRRect.h"
#include "include/core/SkPaint.h"
#include "pp_layout.h"

namespace a380::mfd {
namespace {
using namespace ui;
using nlohmann::json;

const json& field(const json& d, const char* group, const char* key) {
    static const json empty = json::object();
    if (d.contains(group) && d[group].is_object() && d[group].contains(key)) return d[group][key];
    return empty;
}


float pvu(SkCanvas& c, std::string_view v, std::string_view u, float x, float top, SkColor color, bool leading = false);

struct PendingMenu {
    SkRect box;
    std::vector<std::string> items;
    std::vector<bool> dis;
};

void draw_dd(const pp::Frame& f, const json& j, SkRect r, std::vector<PendingMenu>& menus, const char* fallback) {
    DropdownStyle ds;
    ds.disabled = pp::flag(j, "disabled");
    std::string v = pp::str(j, "v", fallback);
    dropdown(f.c, r, v, ds);
    if (pp::flag(j, "selected")) {
        TextStyle ts = label_style(pal::background, 27);
        ts.align = Align::Center;
        SkRect inner = r.makeInset(2, 2);
        inner.fRight -= 25;
        const float tw = text_width(v, ts);
        SkRect sel = SkRect::MakeLTRB(inner.centerX() - tw * 0.5f - 2.2f, inner.centerY() - 15, inner.centerX() + tw * 0.5f + 2.2f, inner.centerY() + 15);
        SkPaint p;
        p.setAntiAlias(true);
        p.setColor(pal::white);
        f.c.drawRRect(SkRRect::MakeRectXY(sel, 5, 5), p);
        p.setColor(pal::cyan);
        f.c.drawRRect(SkRRect::MakeRectXY(sel.makeInset(2, 2), 3, 3), p);
        text_in(f.c, v, inner, ts);
    }
    if (pp::flag(j, "open") && j.contains("items") && j["items"].is_array()) {
        PendingMenu m;
        m.box = SkRect::MakeXYWH(r.fLeft, r.fBottom, r.width(), 0);
        for (const json& it : j["items"]) m.items.push_back(it.get<std::string>());
        if (j.contains("itemsDisabled"))
            for (const json& b : j["itemsDisabled"]) m.dis.push_back(b.get<bool>());
        menus.push_back(std::move(m));
    }
}

std::vector<PendingMenu>* g_menus = nullptr;

void draw_menus(SkCanvas& c, const std::vector<PendingMenu>& menus) {
    for (const PendingMenu& m : menus) {
        std::vector<MenuItem> items;
        for (size_t i = 0; i < m.items.size(); ++i) items.push_back({m.items[i], i < m.dis.size() && m.dis[i]});
        dropdown_menu(c, SkRect::MakeLTRB(m.box.fLeft, m.box.fTop, m.box.fRight, m.box.fTop + 275), items);
    }
}

void top_value(const pp::Frame& f, const json& d, const char* key, const char* label, float lx, float lend, float ux,
               float cy) {
    const json& v = field(d, "top", key);
    if (lend > 0) f.text(label, lend, cy, 20, pal::white, Align::Right);
    else f.text(label, lx, cy, 20, pal::white);
    float top = f.Y(cy) - 15;
    pp::Nudge top_row(f.c, 0, 6);
    value_with_unit(f.c, pp::str(v, "v", "---"), "FL", f.X(ux), top, pal::green, true);
}

void draw_to(const pp::Frame& f, const json& d, std::vector<PendingMenu>& menus) {
    SkCanvas& c = f.c;
    const json& fields = d.contains("fields") ? d["fields"] : json::object();
    auto fld = [&](const char* k) -> const json& {
        static const json empty = json::object();
        return fields.contains(k) ? fields[k] : empty;
    };

    f.text("RWY", 44, 332, 20, pal::white);
    f.val(pp::str(fld("RWY"), "v", "----"), 108, 332, 25, pal::green);
    f.text("T.O SHIFT", 763, 332, 20, pal::white, Align::Right);
    {
        const json& s = fld("T.O SHIFT");
        FieldStyle fs = pp::field_style(s);
        if (fs.unit_trailing.empty()) fs.unit_trailing = "M";
        input_field(c, f.R(782, 311, 908, 354), pp::str(s, "v", "----"), fs);
    }
    f.rule(28, 915, 362);

    const char* rows[3] = {"V1", "VR", "V2"};
    float cys[3] = {413, 474, 537};
    for (int i = 0; i < 3; ++i) {
        { pp::Nudge vlabel(c, 2, i == 1 ? 0 : -1); f.text(rows[i], 33, cys[i], 20, pal::white); }
        const json& s = fld(rows[i]);
        FieldStyle fs = pp::field_style(s);
        if (fs.unit_trailing.empty()) fs.unit_trailing = "KT";
        fs.text_dx = 1.33f;
        input_field(c, f.R(83, cys[i] - 22, 204, cys[i] + 22), pp::str(s, "v", "\xE2\x96\xAF\xE2\x96\xAF\xE2\x96\xAF"), fs);
    }
    const json& sp = d.contains("speeds") ? d["speeds"] : json::object();
    f.text("F", 371.5f, 413, 20, pal::white);
    pvu(c, pp::str(sp, "f", "---"), "KT", f.X(404.5f), f.Y(413) - 15, pal::green);
    f.text("S", 371, 474, 20, pal::white);
    pvu(c, pp::str(sp, "s", "---"), "KT", f.X(404.5f), f.Y(474) - 15, pal::green);
    {
        SkPaint ring;
        ring.setAntiAlias(true);
        ring.setStyle(SkPaint::kStroke_Style);
        ring.setStrokeWidth(2);
        ring.setColor(pal::green);
        c.drawCircle(f.X(378), f.Y(535), 5.5f, ring);
    }
    pvu(c, pp::str(sp, "o", "---"), "KT", f.X(405), f.Y(537) - 15, pal::green);
    ui::vline(c, f.X(521), f.Y(370), f.Y(566), pal::light_grey, 0.667f);

    float rcy[3] = {405, 468, 531};
    if (d.contains("thrust") && d["thrust"].is_array()) {
        int i = 0;
        for (const json& r : d["thrust"]) {
            if (i > 2) break;
            bool dis = pp::flag(r, "disabled");
            bool chk = pp::flag(r, "checked");
            SkColor lc = dis ? pal::dark_grey : (chk ? pal::cyan : pal::white);
            radio_button(c, f.X(547) - 5.33f, f.Y(rcy[i]) - radio_row_height * 0.5f - 2.7f, pp::str(r, "v"), chk, pal::cyan, dis, lc);
            ++i;
        }
    }
    f.rule(28, 915, 579);

    f.text("FLAPS", 24, 603, 20, pal::white);
    f.text("THS FOR", 151, 603, 20, pal::white);
    f.text("PACKS", 330, 603, 20, pal::white);
    f.text("ANTI ICE", 608, 603, 20, pal::white);
    {
        draw_dd(f, pp::sub(d, "flaps"), f.R(26, 633, 117, 685), menus, "-");
        const json& t = pp::sub(d, "ths");
        FieldStyle fs = pp::field_style(t);
        if (fs.unit_trailing.empty()) fs.unit_trailing = "%";
        input_field(c, f.R(154.4f, 637, 280.4f, 681), pp::str(t, "v", "â¯â¯.â¯"), fs);
        ui::vline(c, f.X(313), f.Y(592), f.Y(684), pal::light_grey, 0.667f);
        draw_dd(f, pp::sub(d, "packs"), f.R(331, 632, 573, 686), menus, "ON");
        draw_dd(f, pp::sub(d, "antiIce"), f.R(611.4f, 632, 883.4f, 686), menus, "OFF");
    }
    f.rule(28, 915, 704);

    auto computed = [&](const char* key, float l, float t, float r, float b) {
        const json& s = fld(key);
        FieldStyle fs = pp::field_style(s);
        if (fs.unit_trailing.empty()) fs.unit_trailing = "FT";
        fs.align = Align::Right;
        pp::Nudge cfield(c, -1, -1);
        fs.text_dx = 0.67f;
        input_field(c, f.R(l, t, r, b), pp::str(s, "v", "----"), fs);
    };
    f.text("THR RED", 177.4f, 748, 20, pal::white, Align::Right);
    computed("THR RED", 221, 726, 403, 770);
    f.text("ACCEL", 177.4f, 808, 20, pal::white, Align::Right);
    computed("ACCEL", 221, 787, 403, 831);
    ButtonStyle dis;
    dis.state = ButtonState::Disabled;
    button(c, f.R(221, 840, 339, 885), "NOISE", dis);

    f.rule(28, 915, 984);
    {
        pp::Nudge lbl(c, -1, -2);
        f.text("TRANS", 37, 1035.8f, 20, pal::white);
    }
    computed("TRANS", 132, 1013.8f, 314, 1057.8f);
    {
        pp::Nudge lbl(c, 0, -2);
        f.text("EO ACCEL", 349.6f, 1035.8f, 20, pal::white);
    }
    computed("EO ACCEL", 492.1f, 1013.8f, 673.1f, 1057.8f);
    button_lines(c, f.R(701, 1000.8f, 913, 1069.8f), {"CPNY T.O", "REQUEST"}, dis);
}

void draw_generic(const pp::Frame& f, const json& d) {
    if (!d.contains("tokens") || !d["tokens"].is_array()) return;
    float y = 332;
    std::string pending;
    float col = 0;
    for (const json& t : d["tokens"]) {
        std::string k = pp::str(t, "k");
        if (k == "label") {
            pending = pp::str(t, "v");
            f.text(pending, 40 + col, y, 20, pal::white);
        } else if (k == "value") {
            value_with_unit(f.c, pp::str(t, "v"), pp::str(t, "unit"), f.X(300 + col), f.Y(y) - 15, pal::green);
            y += 56;
        } else if (k == "input") {
            FieldStyle fs = pp::field_style(t);
            input_field(f.c, f.R(280 + col, y - 22, 480 + col, y + 22), pp::str(t, "v"), fs);
            y += 56;
        } else if (k == "dropdown") {
            DropdownStyle ds;
            dropdown(f.c, f.R(280 + col, y - 22, 480 + col, y + 22), pp::str(t, "v"), ds);
            y += 56;
        } else if (k == "button") {
            ButtonStyle bs;
            if (pp::flag(t, "disabled")) bs.state = ButtonState::Disabled;
            button(f.c, f.R(40 + col, y - 22, 240 + col, y + 22), pp::str(t, "v"), bs);
            y += 56;
        }
        if (y > 1000 && col == 0) {
            y = 332;
            col = 460;
        }
    }
}


float pvu(SkCanvas& c, std::string_view v, std::string_view u, float x, float top, SkColor color, bool leading) {
    TextStyle vt = value_style(color, 25);
    TextStyle ut = unit_style(20);
    const float centre = top + 15.0f;
    const float vtop = centre - 11.67f;
    const float utop = centre - 9.0f;
    if (u.empty()) {
        text(c, v, x, vtop, vt);
        return x + text_width(v, vt);
    }
    if (leading) {
        text(c, u, x, utop, ut);
        x += text_width(u, ut) + 6;
        text(c, v, x, vtop, vt);
        return x + text_width(v, vt);
    }
    text(c, v, x, vtop, vt);
    float xe = x + text_width(v, vt) + 6;
    text(c, u, xe, utop, ut);
    return xe + text_width(u, ut);
}


namespace flow {
constexpr float kDpr = 1.5f;
constexpr float kPageX = 11.0f;
constexpr float kPageW = 743.0f;
constexpr float kTop = 237.67f;
constexpr float kBorder = 1.0f / kDpr;
inline float q64(float v) { return std::round(v * 64.f) / 64.f; }
inline float line_box(float size) {
    return (std::round(0.8025f * size * kDpr) + std::round(0.0901f * size * kDpr)) / kDpr;
}
inline float floor_dev(float v) { return std::floor(v * kDpr) / kDpr; }
struct Columns {
    float x0 = kPageX, w = kPageW;
    float pct[5];
    float edge(int i) const { return q64(x0 + w * pct[i]); }
    float left(int i, float pad) const { return edge(i) + pad; }
    float right(int i, float pad, float border = 0) const { return edge(i + 1) - pad - border; }
    float centre(int i, float pl, float pr, float border = 0) const { return (left(i, pl) + right(i, pr, border)) * 0.5f; }
};
inline Columns columns(float a, float b, float c2, float d) {
    Columns k;
    float sum = 0;
    k.pct[0] = 0;
    const float w[4] = {a, b, c2, d};
    for (int i = 0; i < 4; ++i) {
        sum += w[i];
        k.pct[i + 1] = sum / 100.f;
    }
    return k;
}
struct Row {
    float top, height;
    float centre() const { return top + (height - 1) * 0.5f; }
    float bottom() const { return top + height; }
};
inline void label_c(SkCanvas& c, std::string_view s, float cx, float cy, ui::Align a, SkColor col, float size = 20) {
    ui::TextStyle st = ui::label_style(col, size);
    st.align = a;
    ui::text(c, s, cx, cy - line_box(size) * 0.5f, st);
}
inline void value_c(SkCanvas& c, std::string_view s, float x, float cy, ui::Align a, SkColor col, float size = 25) {
    ui::TextStyle st = ui::value_style(col, size);
    st.align = a;
    ui::text(c, s, x, cy - line_box(size) * 0.5f, st);
}
inline std::string collapse_ws(std::string_view s) {
    std::string out;
    for (char ch : s) {
        if (ch == ' ' && !out.empty() && out.back() == ' ') continue;
        out.push_back(ch);
    }
    return out;
}
inline float group_width(std::string_view v, std::string_view u, float vsize = 25) {
    ui::TextStyle vt = ui::value_style(ui::pal::green, vsize);
    float w = ui::text_width(v, vt);
    if (!u.empty()) w += 6 + ui::text_width(u, ui::unit_style(20));
    return w;
}
}  // namespace flow

void draw_des(SkCanvas& c, const json& d) {
    auto R = [](float l, float t, float r, float b) {
        const float k = 1.0f / 1.5f;
        return SkRect::MakeLTRB(l * k, (t + 191) * k, r * k, (b + 191) * k);
    };
    auto P = [](float v) { return v / 1.5f; };
    auto lab = [&](std::string_view t, float x, float cy, ui::Align a) {
        TextStyle st = label_style(pal::white, 20);
        st.align = a;
        text(c, t, P(x + 3.f), P(cy + 191 - 2.f) - line_height(st) * 0.5f, st);
    };
    std::vector<json> tk;
    if (d.contains("tokens") && d["tokens"].is_array())
        for (const json& t : d["tokens"]) tk.push_back(t);
    std::vector<const json*> inputs, vals, labels;
    const json* dd = nullptr;
    const json* btn = nullptr;
    for (const json& t : tk) {
        std::string k = pp::str(t, "k");
        if (k == "input") inputs.push_back(&t);
        else if (k == "value") vals.push_back(&t);
        else if (k == "label") labels.push_back(&t);
        else if (k == "dropdown" && !dd) dd = &t;
        else if (k == "button") btn = &t;
    }
    if (dd) {
        pp::Frame df{c, SkRect::MakeEmpty(), 0, 0};
        draw_dd(df, *dd, R(27, 184, 289, 249), *g_menus, "");
    }
    lab("CI", 311, 217, Align::Left);
    lab("DES CABIN RATE", 559, 217, Align::Left);
    auto in = [&](size_t i, SkRect r, const char* unit) {
        if (i >= inputs.size()) return;
        FieldStyle fs = pp::field_style(*inputs[i]);
        if (fs.unit_trailing.empty()) fs.unit_trailing = unit;
        fs.align = (i == 1 || i == 2) ? Align::Right : Align::Center;
        input_field(c, r.makeOffset(i == 3 ? 0.67f : 0.f, -0.67f), pp::str(*inputs[i], "v"), fs);
    };
    in(0, R(372, 190, 485, 245), "");
    in(1, R(847, 190, 1109, 245), "FT/MN");
    ui::vline(c, P(272), P(268 + 191), P(641 + 191), pal::light_grey, 0.667f);
    for (float y : {380.f, 468.f, 529.f}) ui::hline(c, P(17), P(1131), P(y + 191), pal::light_grey, 0.667f);
    lab("MODE", 143, 324, Align::Center);
    lab("MACH", 344, 324, Align::Center);
    lab("SPD", 539, 324, Align::Center);
    lab("PRED TO", 772, 296, Align::Left);
    in(2, R(797, 310, 1020, 365), "FT");
    lab("MANAGED", 255, 425, Align::Right);
    in(3, R(467, 397, 615, 452), "KT");
    if (vals.size() > 1) {
        const flow::Columns col = flow::columns(23, 13, 22, 42);
        const std::string pv = flow::collapse_ws(pp::str(*vals[1], "v"));
        const float gw = flow::group_width(pv, "");
        pvu(c, pv, "", col.centre(3, 10, 10) - gw * 0.5f, P(425 + 190.5f) - 15, pal::green);
    }
    {
        TextStyle st = label_style(pal::white, 24);
        text(c, "DEST", P(42), P(1105 + 191) - line_height(st) * 0.5f, st);
    }
    auto g = [&](std::string_view t, float x, float cy) {
        TextStyle st = value_style(pal::green, 24);
        text(c, t, P(x + (value ? 0.f : 2.f)), P(cy + 191 - (value ? 0.f : 1.f)) - line_height(st) * 0.5f, st);
    };
    for (size_t i = 0; i < labels.size(); ++i)
        if (pp::str(*labels[i], "v") == "DEST" && i + 2 < labels.size()) {
            g(pp::str(*labels[i + 1], "v"), 238, 1105);
            g(pp::str(*labels[i + 2], "v"), 432, 1105);
        }
    for (const json* v : vals)
        if (pp::str(*v, "unit") == "T") pvu(c, pp::str(*v, "v"), "T", P(660), P(1105 + 191) - 15, pal::green);
    if (btn) button(c, R(921, 1078, 1128, 1133), pp::str(*btn, "v"));
}


void draw_clb(SkCanvas& c, const json& d) {
    auto R = [](float l, float t, float r, float b) {
        const float k = 1.0f / 1.5f;
        return SkRect::MakeLTRB(l * k, (t + 191) * k, r * k, (b + 191) * k);
    };
    auto P = [](float v) { return v / 1.5f; };
    auto txt = [&](std::string_view t, float x, float cy, Align a, SkColor col, float size = 20, bool value = false) {
        TextStyle st = value ? value_style(col, size) : label_style(col, size);
        st.align = a;
        text(c, t, P(x + (value ? 0.f : 2.f)), P(cy + 191 - (value ? 0.f : 1.f)) - line_height(st) * 0.5f, st);
    };
    std::vector<const json*> inputs, dds;
    std::vector<std::string> vals;
    const json* btn = nullptr;
    const json* noise = nullptr;
    if (d.contains("tokens") && d["tokens"].is_array())
        for (const json& t : d["tokens"]) {
            std::string k = pp::str(t, "k");
            if (k == "input") inputs.push_back(&t);
            else if (k == "dropdown") dds.push_back(&t);
            else if (k == "value" && !pp::str(t, "v").empty()) vals.push_back(pp::str(t, "v"));
            else if (k == "button") (pp::str(t, "v") == "NOISE" ? noise : btn) = &t;
        }
    {
        using namespace flow;
        const Columns col = columns(23, 22, 13, 42);
        const float rowA = 14.f + 44.33f + 15.f;
        const float rowA_c = kTop + 7.f + 44.33f * 0.5f;
        const float rowA_r = kTop + rowA * 0.5f;
        const SkColor W = pal::white;
        pp::Frame df{c, SkRect::MakeEmpty(), 0, 0};
        if (dds.size() > 0) draw_dd(df, *dds[0], SkRect::MakeXYWH(kPageX + 7.f, kTop + 7.f, 175.f, 44.33f), *g_menus, "");
        const float ci_x = kPageX + 7.f + 175.f + 15.f;
        label_c(c, "CI", ci_x, rowA_c, Align::Left, W);
        if (!inputs.empty()) {
            FieldStyle fs = pp::field_style(*inputs[0]);
            fs.align = Align::Center;
            const float ix = ci_x + text_width("CI", label_style(W, 20)) + 15.f;
            input_field(c, SkRect::MakeXYWH(ix, rowA_c - 18.5f, 75.f, 37.f), pp::str(*inputs[0], "v"), fs);
        }
        const float dr_w = text_width("DERATED CLB", label_style(W, 20));
        const float dd_x = kPageX + kPageW - 7.f - 125.f;
        label_c(c, "DERATED CLB", dd_x - 15.f - dr_w, rowA_r, Align::Left, W);
        if (dds.size() > 1) {
            if (pp::flag(*dds[1], "disabled")) value_c(c, pp::str(*dds[1], "v"), dd_x + 50.f, rowA_r, Align::Center, pal::green, 27);
            else draw_dd(df, *dds[1], SkRect::MakeXYWH(dd_x, rowA_r - 22.165f, 125.f, 44.33f), *g_menus, "");
        }

        const Row r1{kTop + rowA, 76.f}, r2{r1.bottom(), 58.5f}, r3{r2.bottom(), 58.5f}, r4{r3.bottom(), 47.f};
        const float br = kBorder;
        const float xl = col.edge(0) + 0.33f, xr = col.edge(4);
        for (const Row* r : {&r1, &r2, &r3, &r4}) ui::hline(c, xl, xr, r->bottom() - br * 0.5f, pal::light_grey, br);
        ui::vline(c, floor_dev(col.edge(1)), r1.top, r4.bottom() + 40.f, pal::light_grey, br);

        const float c0 = col.centre(0, 10, 10, br), c1 = col.centre(1, 10, 10), c2 = col.centre(2, 10, 10), c3 = col.centre(3, 10, 10);
        label_c(c, "MODE", c0, r1.centre(), Align::Center, W);
        label_c(c, "SPD", c1, r1.centre(), Align::Center, W);
        label_c(c, "MACH", c2, r1.centre(), Align::Center, W);
        {
            const float bw = 15.f + 150.f;
            const float bl = c3 - bw * 0.5f;
            label_c(c, "PRED TO", bl, r1.top + 10.f + line_box(20) * 0.5f, Align::Left, W);
            if (inputs.size() > 1) {
                FieldStyle fs = pp::field_style(*inputs[1]);
                if (fs.unit_trailing.empty()) fs.unit_trailing = "FT";
                fs.align = Align::Right;
                const float top = r1.top + 10.f + line_box(20);
                input_field(c, SkRect::MakeXYWH(bl + 15.f, top, 150.f, 37.f), pp::str(*inputs[1], "v"), fs);
            }
        }
        const float pr = col.right(0, 10, br);
        label_c(c, "PRESEL", pr, r2.centre(), Align::Right, W);
        if (inputs.size() > 2) {
            FieldStyle fs = pp::field_style(*inputs[2]);
            if (fs.unit_trailing.empty()) fs.unit_trailing = "KT";
            fs.align = Align::Right;
            fs.min_text_width = 54;
            const float fw = input_field_width(pp::str(*inputs[2], "v"), fs);
            input_field(c, SkRect::MakeXYWH(c1 - fw * 0.5f, r2.centre() - 18.5f, fw, 37.f), pp::str(*inputs[2], "v"), fs);
        }
        label_c(c, "MANAGED", pr, r3.centre(), Align::Right, pal::green);
        auto centred_vu = [&](const std::string& v, const char* u, float cx, float cy) {
            const float gw = group_width(v, u);
            pvu(c, v, u, cx - gw * 0.5f, cy - 15.f, pal::green);
        };
        if (!vals.empty()) centred_vu(vals[0], "KT", c1, r3.centre());
        if (vals.size() > 1) centred_vu(collapse_ws(vals[1]), "", c3, r3.centre());
        label_c(c, "ECON", col.right(0, 15, br), r4.centre(), Align::Right, W);
        if (vals.size() > 2) centred_vu(vals[2], "KT", c1, r4.centre());
        if (vals.size() > 3) centred_vu(vals[3], "", c2, r4.centre());
    }
    auto in = [&](size_t i, SkRect r, const char* unit, Align a = Align::Center) {
        if (i >= inputs.size()) return;
        FieldStyle fs = pp::field_style(*inputs[i]);
        if (fs.unit_trailing.empty()) fs.unit_trailing = unit;
        fs.align = a;
        input_field(c, r, pp::str(*inputs[i], "v"), fs);
    };
    txt("THR RED", 225, 737, Align::Right, pal::white);
    in(3, R(313, 711, 537, 765), "FT", Align::Right);
    txt("ACCEL", 225, 812, Align::Right, pal::white);
    in(4, R(313, 786, 537, 840), "FT", Align::Right);
    txt("CLB SPD LIM", 262, 887, Align::Right, pal::white);
    if (vals.size() > 6) {
        pvu(c, vals[4], "KT", P(322), P(887 + 191) - 15, pal::green);
        txt("/", 454, 888, Align::Left, pal::unit_blue, 20, false);
        pvu(c, vals[6], "FL", P(484), P(888 + 191) - 15, pal::green, true);
    }
    if (noise) {
        ButtonStyle bs;
        bs.state = ButtonState::Disabled;
        button(c, R(338, 1068, 481, 1121), "NOISE", bs);
    }
    txt("TRANS", 566, 1095, Align::Left, pal::white);
    in(5, R(687, 1068, 910, 1121), "FT", Align::Right);
    if (btn) button(c, R(921, 1067, 1128, 1121), pp::str(*btn, "v"));
}


void draw_crz(SkCanvas& c, const json& d) {
    auto R = [](float l, float t, float r, float b) {
        const float k = 1.0f / 1.5f;
        return SkRect::MakeLTRB(l * k, (t + 191) * k, r * k, (b + 191) * k);
    };
    auto P = [](float v) { return v / 1.5f; };
    auto txt = [&](std::string_view t, float x, float cy, Align a, SkColor col, float size = 20, bool value = false) {
        TextStyle st = value ? value_style(col, size) : label_style(col, size);
        st.align = a;
        text(c, t, P(x + (value ? 0.f : 2.f)), P(cy + 191 - (value ? 0.f : 1.f)) - line_height(st) * 0.5f, st);
    };
    std::vector<const json*> inputs;
    std::vector<std::string> vals, labels;
    const json* dd = nullptr;
    bool cms_disabled = false;
    if (d.contains("tokens") && d["tokens"].is_array())
        for (const json& t : d["tokens"]) {
            std::string k = pp::str(t, "k");
            if (k == "input") inputs.push_back(&t);
            else if (k == "dropdown" && !dd) dd = &t;
            else if (k == "value" && !pp::str(t, "v").empty()) vals.push_back(pp::str(t, "v"));
            else if (k == "label") labels.push_back(pp::str(t, "v"));
            else if (k == "button" && pp::str(t, "v") == "CMS") cms_disabled = pp::flag(t, "disabled", true);
        }
    if (dd) {
        pp::Frame df{c, SkRect::MakeEmpty(), 0, 0};
        draw_dd(df, *dd, R(28, 178, 290, 242), *g_menus, "");
    }
    txt("CI", 311, 211, Align::Left, pal::white);
    auto in = [&](size_t i, SkRect r, const char* unit) {
        if (i >= inputs.size()) return;
        FieldStyle fs = pp::field_style(*inputs[i]);
        if (fs.unit_trailing.empty()) fs.unit_trailing = unit;
        input_field(c, r, pp::str(*inputs[i], "v"), fs);
    };
    in(0, R(372, 183, 485, 237), "");
    ui::vline(c, P(272), P(255 + 191), P(535 + 191), pal::light_grey, 0.667f);
    ui::vline(c, P(272), P(572 + 191), P(711 + 191), pal::light_grey, 0.667f);
    for (float y : {359.f, 447.f, 535.f, 572.f}) ui::hline(c, P(17), P(1131), P(y + 191), pal::light_grey, 0.667f);
    txt("MODE", 142, 307, Align::Center, pal::white);
    txt("MACH", 343, 307, Align::Center, pal::white);
    txt("SPD", 538, 307, Align::Center, pal::white);
    txt("AT", 885, 283, Align::Center, pal::white);
    txt("STEP TO", 784, 320, Align::Left, pal::white);
    txt("FL", 947, 320, Align::Left, pal::unit_blue);
    txt("PRESEL", 255, 404, Align::Right, pal::white);
    in(1, R(291, 376, 400, 430), "");
    in(2, R(467, 376, 615, 430), "KT");
    txt("MANAGED", 255, 492, Align::Right, pal::white);
    auto v = [&](size_t i) { return i < vals.size() ? vals[i] : std::string(); };
    auto vu = [&](const std::string& t, const char* unit, float x, float cy) {
        pvu(c, t, unit, P(x), P(cy + 191) - 15, pal::green);
    };
    vu(v(0), "", 311, 492);
    vu(v(1), "KT", 482, 492);
    {
        const flow::Columns col = flow::columns(23, 13, 22, 42);
        const std::string pv = flow::collapse_ws(v(2));
        const float gw = flow::group_width(pv, "NM");
        pvu(c, pv, "NM", col.centre(3, 10, 10) - gw * 0.5f, P(492 + 191) - 15, pal::green);
    }
    txt("LRC", 262, 608, Align::Right, pal::white);
    vu(v(3), "", 311, 608);
    vu(v(4), "", 500, 608);
    txt("MAX TURB", 262, 677, Align::Right, pal::white);
    vu(v(5), "", 311, 677);
    vu(v(6), "", 500, 677);
    {
        TextStyle st = label_style(pal::white, 24);
        text(c, "DEST", P(42), P(1105 + 191) - line_height(st) * 0.5f, st);
    }
    txt("OMDB", 202, 1105, Align::Left, pal::green, 24, true);
    txt("--:--", 362, 1105, Align::Left, pal::green, 24, true);
    vu(v(7), "T", 555, 1105);
    ButtonStyle bs;
    if (cms_disabled) bs.state = ButtonState::Disabled;
    button(c, R(782, 1078, 884, 1132), "CMS", bs);
    button(c, R(901, 1078, 1128, 1132), "STEP ALTs");
}


void draw_ga(SkCanvas& c, const json& d) {
    auto R = [](float l, float t, float r, float b) {
        const float k = 1.0f / 1.5f;
        return SkRect::MakeLTRB(l * k, (t + 191) * k, r * k, (b + 191) * k);
    };
    auto P = [](float v) { return v / 1.5f; };
    auto txt = [&](std::string_view t, float x, float cy, Align a, SkColor col) {
        TextStyle st = label_style(col, 20);
        st.align = a;
        text(c, t, P(x + 2.f), P(cy + 191 - 1.f) - line_height(st) * 0.5f, st);
    };
    std::vector<const json*> inputs;
    std::vector<std::string> vals;
    if (d.contains("tokens") && d["tokens"].is_array())
        for (const json& t : d["tokens"]) {
            std::string k = pp::str(t, "k");
            if (k == "input") inputs.push_back(&t);
            else if (k == "value" && !pp::str(t, "v").empty()) vals.push_back(pp::str(t, "v"));
        }
    auto v = [&](size_t i) { return i < vals.size() ? vals[i] : std::string(); };
    auto vu = [&](const std::string& t, const char* unit, float x, float cy) {
        pvu(c, t, unit, P(x), P(cy + 191) - 15, pal::green);
    };
    auto in = [&](size_t i, SkRect r) {
        if (i >= inputs.size()) return;
        FieldStyle fs = pp::field_style(*inputs[i]);
        if (fs.unit_trailing.empty()) fs.unit_trailing = "FT";
        fs.align = Align::Right;
        input_field(c, r, pp::str(*inputs[i], "v"), fs);
    };
    txt("F", 349, 299, Align::Center, pal::white);
    txt("S", 349, 352, Align::Center, pal::white);
    {
        SkPaint ring;
        ring.setAntiAlias(true);
        ring.setStyle(SkPaint::kStroke_Style);
        ring.setStrokeWidth(1.5f);
        ring.setColor(pal::green);
        c.drawCircle(P(349), P(406 + 191), 4.2f, ring);
    }
    vu(v(0), "KT", 383, 299);
    vu(v(1), "KT", 383, 353);
    vu(v(2), "KT", 383, 407);
    txt("THR RED", 254, 611, Align::Right, pal::white);
    in(0, R(280, 583, 504, 638));
    txt("ACCEL", 254, 689, Align::Right, pal::white);
    in(1, R(280, 661, 504, 716));
    txt("EO ACCEL", 749, 689, Align::Right, pal::white);
    in(2, R(775, 661, 999, 716));
    txt("TRANS", 265, 1111, Align::Right, pal::white);
    vu(v(3), "FT", 289.5f, 1111);
}

void draw_appr(SkCanvas& c, const json& d) {
    auto R = [](float l, float t, float r, float b) {
        const float k = 1.0f / 1.5f;
        return SkRect::MakeLTRB(l * k, (t + 191) * k, r * k, (b + 191) * k);
    };
    auto P = [](float v) { return v / 1.5f; };
    auto txt = [&](std::string_view t, float x, float cy, Align a, SkColor col, float size = 20, bool value = false) {
        TextStyle st = value ? value_style(col, size) : label_style(col, size);
        st.align = a;
        text(c, t, P(x + (value ? 0.f : 2.f)), P(cy + 191 - (value ? 0.f : 1.f)) - line_height(st) * 0.5f, st);
    };
    std::vector<const json*> inputs, radios;
    std::vector<std::string> vals;
    std::string minimum = "-----";
    if (d.contains("tokens") && d["tokens"].is_array())
        for (const json& t : d["tokens"]) {
            std::string k = pp::str(t, "k");
            if (k == "input") inputs.push_back(&t);
            else if (k == "radio") radios.push_back(&t);
            else if (k == "value" && !pp::str(t, "v").empty()) vals.push_back(pp::str(t, "v"));
            else if (k == "label" && !pp::str(t, "v").empty() && pp::str(t, "v")[0] == '-') minimum = pp::str(t, "v");
        }
    auto v = [&](size_t i) { return i < vals.size() ? vals[i] : std::string(); };
    auto vu = [&](const std::string& t, const char* unit, float x, float cy) {
        pvu(c, t, unit, P(x), P(cy + 191) - 15, pal::green);
    };
    auto in = [&](size_t i, SkRect r, const char* unit) {
        if (i >= inputs.size()) return;
        FieldStyle fs = pp::field_style(*inputs[i]);
        if (fs.unit_trailing.empty()) fs.unit_trailing = unit;
        fs.align = Align::Right;
        input_field(c, r, pp::str(*inputs[i], "v"), fs);
    };
    txt("APPR", 52, 221, Align::Left, pal::white, 20);
    txt(v(0), 153, 224, Align::Left, pal::green, 24, true);
    txt(v(1), 563, 220, Align::Left, pal::green, 25, true);
    txt("LW", 901, 221, Align::Left, pal::white, 20);
    vu(v(2), "T", 963, 221);
    ui::hline(c, P(33), P(1131), P(260 + 191), pal::light_grey, 0.667f);
    ui::stroke_box(c, R(33, 292, 589, 664), 0xFF4F5467, 1);
    txt("MAG WIND", 240, 346, Align::Right, pal::white);
    ui::stroke_box(c, R(265, 314, 566, 378), 0xFF4F5467, 1);
    in(0, R(270, 319, 398, 373), "°");
    in(1, R(414, 319, 561, 373), "KT");
    txt("HD", 113, 440, Align::Right, pal::white);
    vu(v(3), "KT", 138, 440);
    txt("CROSS", 392, 440, Align::Right, pal::white);
    vu(v(4), "KT", 416, 440);
    txt("OAT", 241, 537, Align::Right, pal::white);
    in(2, R(265, 508, 453, 563), "°" "C");
    txt("QNH", 241, 613, Align::Right, pal::white);
    in(3, R(265, 587, 453, 641), "");
    ui::labeled_box(c, R(33, 725, 589, 876), "MINIMUM");
    txt("BARO", 240.5f, 777, Align::Right, pal::white);
    in(4, R(266, 749, 490, 803), "FT");
    txt(minimum, 146.5f, 841, Align::Left, pal::white, 20, false);
    {
        SkPaint p;
        p.setAntiAlias(true);
        p.setStyle(SkPaint::kStroke_Style);
        p.setStrokeWidth(2);
        p.setColor(pal::green);
        c.drawCircle(P(848), P(331 + 191), P(7), p);
    }
    vu(v(5), "KT", 881, 332.67);
    txt("S", 848, 387, Align::Center, pal::white);
    vu(v(6), "KT", 881, 386.67);
    txt("F", 846, 441, Align::Center, pal::white);
    vu(v(7), "KT", 881, 440.67);
    txt("VREF", 856.4f, 507, Align::Right, pal::white);
    vu(v(8), "KT", 881, 506.67);
    ui::stroke_box(c, R(741, 565, 1039, 814), 0xFF4F5467, 1);
    for (size_t i = 0; i < radios.size() && i < 2; ++i) {
        const bool chk = pp::flag(*radios[i], "checked") || (i == 1);
        radio_button(c, P(787) - 5.93f, P((i == 0 ? 595.f : 673.f) + 191) - 5.83f, pp::str(*radios[i], "v"), chk, pal::cyan, false, chk ? pal::cyan : pal::white);
    }
    txt("VLS", 847, 786, Align::Right, pal::white);
    vu(v(9), "KT", 870, 786.67);
    txt("VAPP", 840, 872, Align::Right, pal::white);
    in(5, R(866, 846, 1013, 899), "KT");
    txt("TRANS", 231, 1095, Align::Right, pal::white);
    if (inputs.size() > 6) {
        FieldStyle fs = pp::field_style(*inputs[6]);
        fs.unit_leading = "FL";
        fs.align = Align::Left;
        pp::Nudge tf(c, -1, -1);
        input_field(c, R(256, 1068, 419, 1121), pp::str(*inputs[6], "v"), fs);
    }
    txt("VERT DEV", 940, 1095, Align::Right, pal::white);
    txt(v(10), 965, 1098, Align::Left, pal::green, 24, true);
}

void draw_perf(SkCanvas& c, SkRect content, const json& page) {
    pp::Frame f{c, content, -1.6f, 160.0f, 0.0f};
    pp::Frame fb{c, content, 0.0f, 159.6f, 0.0f};
    const json& d = page;

    f.text("CRZ", 29, 188, 20, pal::white);
    {
        const json& v = field(d, "top", "CRZ");
        FieldStyle fs = pp::field_style(v);
        if (fs.unit_leading.empty()) fs.unit_leading = "FL";
        fs.min_text_width = 54;
        input_field(c, f.R(93, 167, 212, 210), pp::str(v, "v", "---"), fs);
    }
    top_value(f, d, "OPT", "OPT", 385, 0, 450, 188);
    top_value(f, d, "REC MAX", "REC MAX", 0, 788, 806, 188);
    top_value(f, d, "EO MAX", "EO MAX", 0, 788, 806, 224);

    std::vector<std::string> tabs;
    int sel = 0;
    if (d.contains("tabs") && d["tabs"].is_array())
        for (const json& t : d["tabs"]) tabs.push_back(t.get<std::string>());
    if (tabs.empty()) tabs = {"T.O", "CLB", "CRZ", "DES", "APPR", "GA"};
    std::string active = pp::str(d, "tab", "T.O");
    for (size_t i = 0; i < tabs.size(); ++i)
        if (tabs[i] == active) sel = static_cast<int>(i);
    TabStyle ts;
    ts.selected = sel;
    SkRect nav = SkRect::MakeLTRB(content.fLeft, content.fTop + 69.6f, content.fRight, content.fBottom - 59.7f);
    tab_navigator(c, nav, tabs, ts);

    button(c, fb.R(3, 1118, 137, 1161), "RETURN");
    button(c, fb.R(334, 1118, 553, 1161), "POS MONITOR");
    std::vector<PendingMenu> menus;
    g_menus = &menus;
    if (active == "T.O") draw_to(fb, d, menus);
    else if (active == "DES") draw_des(c, d);
    else if (active == "CLB") draw_clb(c, d);
    else if (active == "CRZ") draw_crz(c, d);
    else if (active == "APPR") draw_appr(c, d);
    else if (active == "GA") draw_ga(c, d);
    else if (active != "CLB" && active != "CRZ" && active != "APPR" && active != "GA") draw_generic(fb, d);
    draw_menus(c, menus);
}

A380_MFD_PAGE("fms/active/perf", draw_perf);
}  // namespace
}  // namespace a380::mfd
