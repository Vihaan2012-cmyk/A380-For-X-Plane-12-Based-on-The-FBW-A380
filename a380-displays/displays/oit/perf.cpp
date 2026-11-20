#include "oit.h"

#include <cstdlib>

#include "include/core/SkPaint.h"
#include "include/core/SkPathBuilder.h"

namespace a380::oit {

namespace {
std::string str(const nlohmann::json& j, const char* k) {
    return j.contains(k) && j[k].is_string() ? j[k].get<std::string>() : std::string();
}

float num(const nlohmann::json& j, const char* k, float d = 0) {
    return j.contains(k) && j[k].is_number() ? j[k].get<float>() : d;
}

SkColor named(const std::string& s) {
    if (s == "green") return 0xFF00FF00;
    if (s == "amber") return 0xFFE68000;
    if (s == "cyan") return pal::cyan;
    return pal::white;
}

void tri_down(SkCanvas& c, float cx, float cy, float s, SkColor col) {
    SkPathBuilder b;
    b.moveTo(cx - s * 0.5f, cy - s * 0.5f);
    b.lineTo(cx + s * 0.5f, cy - s * 0.5f);
    b.lineTo(cx, cy + s * 0.5f);
    b.close();
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(col);
    c.drawPath(b.detach(), p);
}

void draw_box(SkCanvas& c, const nlohmann::json& o) {
    SkRect r = SkRect::MakeXYWH(num(o, "x"), num(o, "y"), num(o, "w"), num(o, "h"));
    border(c, r, pal::light_grey, 1);
    Text t;
    t.size = 20;
    std::string s = str(o, "s");
    float tw = text_width(s, t);
    fill(c, SkRect::MakeXYWH(r.left() + 11, r.top() + 1 - 10, tw + 16, 28), pal::black);
    text(c, s, r.left() + 11 + 8, r.top() + 1 - 10, t);
}

void draw_field(SkCanvas& c, const nlohmann::json& o) {
    SkRect r = SkRect::MakeXYWH(num(o, "x"), num(o, "y"), num(o, "w"), num(o, "h"));
    SkRect box = SkRect::MakeLTRB(r.left(), r.top(), r.right(), r.bottom() + 1);
    fill(c, box, pal::black);
    bevel(c, box, Bevel::Inset, pal::light_grey);
    std::string unit = str(o, "unit");
    std::string s = str(o, "s");
    Text t;
    t.size = 28;
    Text ut = t;
    ut.color = 0xFF0066FF;
    ut.size = 20;
    ut.line_height = 24;
    float uw = unit.empty() ? 0.f : text_width(unit, ut);
    float cx = (r.left() + r.right() - uw) * 0.5f;
    if (o.value("mand", false)) {
        Text mt = t;
        mt.size = 27;
        mt.line_height = 27;
        mt.color = 0xFFE68000;
        mt.align = Align::Center;
        mt.hinted = true;
        mt.family = "Cambria";
        text(c, s, cx, r.top() + 5, mt);
    } else {
        t.color = pal::cyan;
        t.align = Align::Center;
        t.size = 27;
        t.line_height = 27;
        text(c, s, cx, r.top() + 5, t);
    }
    if (!unit.empty()) {
        ut.align = Align::Right;
        text(c, unit, r.right() - 4, r.centerY() - 12, ut);
    }
}

void draw_dd(SkCanvas& c, const nlohmann::json& o) {
    SkRect r = SkRect::MakeXYWH(num(o, "x"), num(o, "y"), num(o, "w"), num(o, "h"));
    fill(c, r, pal::black);
    bevel(c, r, Bevel::Outset, pal::light_grey);
    float aw = 25.f;
    if (num(o, "mw") > 0) aw = std::max(0.f, std::min(25.f, r.width() - 4 - (num(o, "mw") + 20)));
    SkRect ar = SkRect::MakeLTRB(r.right() - 2 - aw, r.top() + 2, r.right() - 2, r.bottom() - 2);
    fill(c, ar, pal::darker_grey);
    tri_down(c, ar.centerX(), r.centerY(), 15, pal::white);
    Text t;
    t.size = 27;
    t.line_height = 27;
    t.color = pal::cyan;
    t.align = Align::Center;
    float cx = (r.left() + 2 + ar.left()) * 0.5f, top = std::floor(r.centerY() - 12.5f);
    std::string v = str(o, "s");
    if (o.value("sel", false)) {
        float w = text_width(v, t);
        fill(c, SkRect::MakeXYWH(std::round(cx - w * 0.5f), top, std::round(w), 27), pal::cyan);
        t.color = pal::black;
    }
    text(c, v, cx, top, t);
}

void draw_menu(SkCanvas& c, const nlohmann::json& o) {
    float x = num(o, "x"), y = num(o, "y"), w = num(o, "w");
    const auto& items = o["items"];
    SkRect r = SkRect::MakeXYWH(x, y, w, 4 + 44.f * items.size());
    fill(c, r, pal::black);
    bevel(c, r, Bevel::Outset, pal::light_grey);
    Text t;
    t.size = 20;
    t.align = Align::Center;
    float iy = y + 2;
    for (auto& it : items) {
        text(c, it.get<std::string>(), x + w * 0.5f, iy + 8, t);
        iy += 44;
    }
}

void draw_radio(SkCanvas& c, const nlohmann::json& o) {
    float x = num(o, "x"), y = num(o, "y");
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(pal::cyan);
    p.setStyle(SkPaint::kStroke_Style);
    p.setStrokeWidth(2);
    c.drawCircle(x + 12, y + 14, 10, p);
    if (o.value("on", false)) {
        p.setStyle(SkPaint::kFill_Style);
        c.drawCircle(x + 12, y + 14, 5, p);
    }
    Text t;
    t.size = 24;
    text(c, str(o, "s"), x + 34, y, t);
}
}  // namespace

void draw_perf(SkCanvas& c, const nlohmann::json& ops) {
    c.save();
    c.clipRect(SkRect::MakeLTRB(0, 70, 1333, 940));
    for (auto& o : ops) {
        std::string t = str(o, "t");
        if (t == "box") draw_box(c, o);
        else if (t == "label") {
            Text tx;
            tx.size = num(o, "size", 28);
            tx.color = named(str(o, "col"));
            text(c, str(o, "s"), num(o, "x"), num(o, "y"), tx);
        } else if (t == "field") draw_field(c, o);
        else if (t == "dd") draw_dd(c, o);
        else if (t == "btn") {
            ButtonOpts bo;
            bo.disabled = o.value("dis", false);
            button(c, SkRect::MakeXYWH(num(o, "x"), num(o, "y"), num(o, "w"), num(o, "h")), {str(o, "s")}, bo);
        } else if (t == "radio") draw_radio(c, o);
        else if (t == "menu") draw_menu(c, o);
    }
    c.restore();
}

}  // namespace a380::oit
