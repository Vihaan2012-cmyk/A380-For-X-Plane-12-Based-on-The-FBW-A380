#include "include/core/SkPathBuilder.h"
#include "page_model.h"
#include "pp_layout.h"

namespace a380::mfd {
namespace {
using namespace ui;
using nlohmann::json;

void draw_sec_index(SkCanvas& c, SkRect content, const json& page) {
    const json& d = page;
    std::vector<std::string> tabs = {"SEC 1", "SEC 2", "SEC 3"};
    int sel = 0;
    std::string active = pp::str(d, "tab", "SEC 1");
    for (size_t i = 0; i < tabs.size(); ++i)
        if (tabs[i] == active) sel = static_cast<int>(i);
    TabStyle ts;
    ts.selected = sel;
    ts.slant_deg = 25;
    SkRect in = tab_navigator(c, content, tabs, ts);

    TextStyle t1 = label_style(pal::white, 28);
    t1.align = Align::Center;
    text(c, pp::str(d, "title1"), in.centerX(), in.fTop + 6, t1);
    label(c, pp::str(d, "title2"), in.fLeft + 4, in.fTop + 45, pal::white, 20);

    float top = in.fTop + 75.f;
    float leftW = in.width() * 0.75f;
    SkRect grid = SkRect::MakeLTRB(in.fLeft + 1.33f, top, in.fLeft + leftW + 1.33f, top + 500);
    stroke_box(c, grid, pal::light_grey, 0.667f);
    if (d.contains("route") && d["route"].is_array()) {
        float cw = (grid.width() - 10) / 4;
        int i = 0;
        for (const json& r : d["route"]) {
            if (!r.is_array() || r.size() < 2) continue;
            int col = (i % 2) * 2;
            int row = i / 2;
            float y = grid.fTop + 20 + row * 28;
            float x = grid.fLeft + 5 + col * cw;
            label(c, r[0].get<std::string>(), x, y, pal::white, 22);
            label(c, r[1].get<std::string>(), x + cw, y, pal::white, 26);
            hline(c, x, x + 2 * cw, y + 28, pal::light_grey, 0.667f);
            ++i;
        }
    }

    const float rx = in.fLeft + leftW + 1.33f;
    const float rw = in.right() - rx;
    struct Btn { std::string v; bool dis; };
    std::vector<Btn> btn;
    if (d.contains("buttons") && d["buttons"].is_array())
        for (const json& b : d["buttons"]) btn.push_back({pp::str(b, "v"), pp::flag(b, "disabled")});
    auto find = [&](const char* prefix) -> const Btn* {
        for (const Btn& b : btn)
            if (b.v.rfind(prefix, 0) == 0) return &b;
        return nullptr;
    };
    auto menu_for = [&](const std::string& owner) -> const json* {
        if (d.contains("menus") && d["menus"].is_array())
            for (const json& m : d["menus"])
                if (pp::str(m, "owner") == owner) return &m;
        return nullptr;
    };
    auto draw_btn = [&](const Btn& b, SkRect r, bool arrow) {
        const float sy = c.getTotalMatrix().getScaleY();
        r = SkRect::MakeXYWH(r.left(), std::round(r.top() * sy) / sy, r.width(), r.height());
        ButtonStyle bs;
        if (menu_for(b.v)) bs.state = ButtonState::Selected;
        if (b.dis) bs.state = ButtonState::Disabled;
        bs.arrow = arrow;
        if (arrow) bs.arrow_label_right = 3.5f;
        const float lift = arrow ? 1.33f : 0.67f;
        bs.pad_top -= lift;
        bs.pad_bottom += lift;
        std::string v = b.v;
        const auto nl = v.find('\n');
        if (!v.empty() && v.back() == '*') {
            bs.asterisk = true;
            v.pop_back();
        }
        if (nl != std::string::npos) button_lines(c, r, {v.substr(0, nl), v.substr(nl + 1)}, bs);
        else if (bs.asterisk) {
            json bj = {{"label", v}, {"asterisk", true}};
            if (b.dis) bj["state"] = "disabled";
            else if (menu_for(b.v)) bj["state"] = "selected";
            pm::button(c, r, bj, 5.2f, -1.07f);
        } else button(c, r, v, bs);
    };
    const float bh1 = 37.4f;
    const float bh2 = 43.3f;
    float y = top;
    SkRect import_rect = SkRect::MakeEmpty();
    if (const Btn* b = find("IMPORT")) {
        SkRect ir = SkRect::MakeXYWH(rx + (rw - 175) * 0.5f, y, 175, bh1);
        draw_btn(*b, ir, true);
        import_rect = ir;
        y += bh1 + 5;
    }
    SkRect second_rect = SkRect::MakeEmpty();
    const Btn* second = btn.size() > 1 && btn[0].v.rfind("IMPORT", 0) == 0 ? &btn[1] : find("CPNY");
    if (second) {
        second_rect = SkRect::MakeXYWH(rx + (rw - 175) * 0.5f, y, 175, 57.3f);
        draw_btn(*second, second_rect, false);
    }
    const char* stack[6] = {"F-PLN", "PERF", "WIND", "FUEL&LOAD", "INIT", "WHAT IF"};
    float by = top + 500.34f - bh1;
    for (int k = 5; k >= 0; --k) {
        if (const Btn* b = find(stack[k])) draw_btn(*b, SkRect::MakeXYWH(rx + (rw - 160) * 0.5f + 0.9f, by, 160, bh1), false);
        by -= 42.333f;
    }
    (void)bh2;

    const float lower = top + 520;
    const float colw = leftW / 3;
    if (const Btn* b = find("DELETE")) draw_btn(*b, SkRect::MakeXYWH(in.fLeft + 1.6f, lower, 160, 60), false);
    for (int k = 0; k < 2; ++k) {
        SkRect r = SkRect::MakeXYWH(in.fLeft + colw + (colw - 125) * 0.5f + k * 65, lower, 60, 60);
        icon_button(c, r, true);
        SkPathBuilder pb;
        const float cx = r.centerX();
        const float sgn = k == 0 ? 1.f : -1.f;
        for (int t = 0; t < 2; ++t) {
            const float y0 = r.centerY() - sgn * (t == 0 ? 17.f : 0.f) - (sgn > 0 ? 0.f : 17.f) + (t == 0 ? 0.f : 0.f);
            const float ya = sgn > 0 ? r.centerY() - 18 + t * 17 : r.centerY() + 18 - t * 17;
            (void)y0;
            pb.moveTo(cx - 17, ya);
            pb.lineTo(cx + 17, ya);
            pb.lineTo(cx, ya + sgn * 16);
            pb.close();
        }
        SkPaint p;
        p.setAntiAlias(true);
        p.setColor(pal::dark_grey);
        c.drawPath(pb.detach(), p);
    }
    if (const Btn* b = find("XFR")) {
        ButtonStyle bs;
        bs.state = b->dis ? ButtonState::Disabled : ButtonState::Normal;
        bs.font_size = 22;
        button(c, SkRect::MakeXYWH(in.fLeft + leftW - 160, lower + 90, 160, 60), b->v, bs);
    }
    if (const Btn* b = find("PRINT")) draw_btn(*b, SkRect::MakeXYWH(rx + (rw - 160) * 0.5f + 0.9f, in.bottom() - 75, 160, 60), false);
    if (second && second_rect.width() > 0)
        if (const json* m = menu_for(second->v)) {
            std::vector<MenuItem> items;
            for (const json& it : (*m)["items"]) items.push_back({it.get<std::string>(), false});
            dropdown_menu(c, SkRect::MakeLTRB(second_rect.left(), second_rect.bottom(), second_rect.right(), second_rect.bottom() + 275), items);
        }
    if (const json* m = menu_for("IMPORT")) {
        std::vector<MenuItem> items;
        for (const json& it : (*m)["items"]) items.push_back({it.get<std::string>(), false});
        dropdown_menu(c, SkRect::MakeLTRB(import_rect.left(), import_rect.bottom(), import_rect.right(), import_rect.bottom() + 275), items);
    }
}

A380_MFD_PAGE("fms/sec/index", draw_sec_index);
}  // namespace
}  // namespace a380::mfd
