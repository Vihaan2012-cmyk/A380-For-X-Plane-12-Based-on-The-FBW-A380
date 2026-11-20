#include "oit.h"

#include <algorithm>
#include <cmath>

#include "include/core/SkPaint.h"
#include "include/core/SkRRect.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkPath.h"

namespace a380::oit {

namespace {
std::string str(const nlohmann::json& j, const char* k) {
    return j.contains(k) && j[k].is_string() ? j[k].get<std::string>() : std::string();
}

void triangle(SkCanvas& c, float x, float y, float w, float h, bool down, SkColor col) {
    SkPathBuilder b;
    if (down) {
        b.moveTo(x, y);
        b.lineTo(x + w, y);
        b.lineTo(x + w * 0.5f, y + h);
    } else {
        b.moveTo(x, y + h);
        b.lineTo(x + w, y + h);
        b.lineTo(x + w * 0.5f, y);
    }
    b.close();
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(col);
    c.drawPath(b.detach(), p);
}

float selector(SkCanvas& c, float x_left, float x_right, float y, const std::string& label) {
    Text t;
    t.size = 28;
    float tw = text_width(label, t);
    float w = tw + 43;
    float x = x_right >= 0 ? x_right - w : x_left;
    SkRect r = SkRect::MakeXYWH(x, y, w, 54);
    fill(c, r, pal::darker_grey);
    fill(c, SkRect::MakeXYWH(r.left(), r.top(), r.width(), 2), pal::light_grey);
    fill(c, SkRect::MakeXYWH(r.left(), r.top(), 2, r.height()), pal::light_grey);
    SkColor dim = SkColorSetRGB(0x71, 0x70, 0x70);
    fill(c, SkRect::MakeXYWH(r.left() + 2, r.bottom() - 2, r.width() - 2, 2), dim);
    fill(c, SkRect::MakeXYWH(r.right() - 2, r.top() + 2, 2, r.height() - 2), dim);
    text(c, label, r.left() + 6, y + 13, t);
    triangle(c, r.right() - 2 - 8 - 15, y + 27 - 7.f, 15, 15, true, pal::white);
    return w;
}

float num(const nlohmann::json& j, const char* k, float d = 0) {
    return j.contains(k) && j[k].is_number() ? j[k].get<float>() : d;
}

SkColor css_color(const std::string& s, SkColor d) {
    if (s.size() == 7 && s[0] == '#') return 0xFF000000 | (SkColor)std::stoul(s.substr(1), nullptr, 16);
    return d;
}

void draw_folder(SkCanvas& c, const nlohmann::json& m) {
    if (!m.contains("folder") || !m["folder"].is_object()) return;
    const auto& f = m["folder"];
    Text item;
    item.size = 20;
    text(c, str(f, "label"), 95, 173, item);
    fill(c, SkRect::MakeXYWH(28, 181.65625f, 10, 10), pal::white);
    Text op;
    op.size = 10;
    op.line_height = 28;
    op.color = pal::black;
    text(c, str(f, "opener").empty() ? "-" : str(f, "opener"), 30.6875f, 172.65625f, op);
    const auto& svg = f["svg"];
    float k = num(svg, "w", 40) / svg["vb"][2].get<float>();
    c.save();
    c.translate(45, 170.3f);
    c.scale(k, k);
    for (auto& sh : svg["shapes"]) {
        SkPaint fp, sp;
        fp.setAntiAlias(true);
        sp.setAntiAlias(true);
        sp.setStyle(SkPaint::kStroke_Style);
        sp.setStrokeWidth(1);
        std::string fc = str(sh, "fill"), sc = str(sh, "stroke");
        bool dofill = fc != "none" && !fc.empty(), dostroke = sc != "none" && !sc.empty();
        fp.setColor(css_color(fc, pal::black));
        sp.setColor(css_color(sc, pal::black));
        if (sh.contains("segs")) {
            SkPathBuilder b;
            for (auto& sg : sh["segs"]) {
                std::string op = sg[0].get<std::string>();
                if (op == "m") b.moveTo(sg[1].get<float>(), sg[2].get<float>());
                else if (op == "l") b.lineTo(sg[1].get<float>(), sg[2].get<float>());
                else if (op == "c")
                    b.cubicTo(sg[1].get<float>(), sg[2].get<float>(), sg[3].get<float>(), sg[4].get<float>(),
                              sg[5].get<float>(), sg[6].get<float>());
                else b.close();
            }
            SkPath path = b.detach();
            if (dofill) c.drawPath(path, fp);
            if (dostroke) c.drawPath(path, sp);
        } else if (sh.contains("ell")) {
            const auto& e = sh["ell"];
            SkRect r = SkRect::MakeLTRB(e[0].get<float>() - e[2].get<float>(), e[1].get<float>() - e[3].get<float>(),
                                        e[0].get<float>() + e[2].get<float>(), e[1].get<float>() + e[3].get<float>());
            if (dofill) c.drawOval(r, fp);
            if (dostroke) c.drawOval(r, sp);
        } else if (sh.contains("line")) {
            const auto& l = sh["line"];
            c.drawLine(l[0].get<float>(), l[1].get<float>(), l[2].get<float>(), l[3].get<float>(), sp);
        }
    }
    c.restore();
}

void draw_right(SkCanvas& c, const nlohmann::json& ops) {
    c.save();
    c.clipRect(SkRect::MakeXYWH(310, 70, 456, 870));
    for (auto& o : ops) {
        std::string t = str(o, "t");
        float x = num(o, "x"), y = num(o, "y"), w = num(o, "w"), h = num(o, "h");
        if (t == "vline") {
            fill(c, SkRect::MakeXYWH(x, y, 1, h), SkColorSetRGB(0x5a, 0x5a, 0x5a));
        } else if (t == "text") {
            Text tx;
            tx.size = num(o, "size", 24);
            text(c, str(o, "s"), x, y, tx);
        } else if (t == "button") {
            button(c, SkRect::MakeXYWH(x, y, w, h), {str(o, "s")}, ButtonOpts{});
        } else if (t == "input" || t == "area") {
            fill(c, SkRect::MakeXYWH(x, y, w, h), pal::white);
            std::string ph = str(o, "ph");
            if (!ph.empty()) {
                Text tx;
                tx.size = num(o, "size", 24);
                tx.color = SkColorSetRGB(0xa9, 0xa9, 0xa9);
                text(c, ph, x, y, tx);
            }
            if (t == "area") {
                SkPaint p;
                p.setAntiAlias(true);
                p.setColor(SkColorSetRGB(0x60, 0x60, 0x60));
                c.drawLine(x + w - 2, y + h - 10, x + w - 10, y + h - 2, p);
                c.drawLine(x + w - 2, y + h - 6, x + w - 6, y + h - 2, p);
            }
        } else if (t == "th") {
            fill(c, SkRect::MakeXYWH(x, y, w, h), pal::grey);
            fill(c, SkRect::MakeXYWH(x, y + h - 1, w, 1), pal::light_grey);
            fill(c, SkRect::MakeXYWH(x + w - 1, y, 1, h), pal::light_grey);
            Text tx;
            tx.size = 20;
            tx.align = Align::Left;
            float tw = text_width(str(o, "s"), tx);
            c.save();
            c.clipRect(SkRect::MakeXYWH(x, y, w - 1, h));
            text(c, str(o, "s"), tw > w ? x : x + (w - tw) / 2, y, tx);
            c.restore();
        }
    }
    c.restore();
}
}  // namespace

void draw_sts(SkCanvas& c, const nlohmann::json& s) {
    const SkColor amber = 0xFFE68000;
    border(c, SkRect::MakeLTRB(20, 80, 1313, 930), pal::light_grey, 3);
    fill(c, SkRect::MakeXYWH(33, 640, 1267, 3), pal::light_grey);
    Text lab;
    lab.size = 28;
    lab.align = Align::Right;
    Text val = lab;
    val.align = Align::Center;
    val.color = pal::cyan;
    auto label = [&](const char* t, float cy) { text(c, t, 389.286f, cy - 14, lab); };
    label("ACFT REGISTRATION", 259);
    label("MSN", 393);
    label("OIS VERSION", 501);
    label("FLT NUMBER", 701.5f);
    label("FROM", 818.5f);
    Text to = lab;
    to.align = Align::Left;
    text(c, "TO", 704.286f, 804.5f, to);
    SkRect reg = SkRect::MakeLTRB(409, 227, 1009, 291);
    fill(c, reg, pal::black);
    bevel(c, reg, Bevel::Outset, SkColorSetRGB(0xC4, 0xC6, 0xCF));
    fill(c, SkRect::MakeLTRB(982, 229, 1007, 289), pal::darker_grey);
    triangle(c, 994.5f - 7.5f, 259 - 7.5f, 15, 15, true, pal::white);
    auto flag = [&](const char* k, size_t i) {
        return s.contains(k) && s[k].is_array() && i < s[k].size() && s[k][i].get<bool>();
    };
    Text inp = val;
    inp.size = 27;
    inp.line_height = 27;
    auto input_text = [&](const std::string& v, float cx, float top, bool sel, bool edit, SkColor col, size_t idx) {
        Text t = inp;
        t.color = col;
        if (sel) {
            t.color = pal::black;
            float w = text_width(v, t);
            fill(c, SkRect::MakeXYWH(std::round(cx - w * 0.5f), top, std::round(w), 27), pal::cyan);
        }
        text(c, v, cx, top, t);
        bool last = true;
        for (size_t k = idx + 1; k < 4; k++) last = last && !(flag("sel", k) && flag("edit", k));
        if (edit && sel && last) {
            float tw = text_width(v, inp);
            SkRect ring = SkRect::MakeXYWH(std::round(cx - tw * 0.5f - 2), top - 5, std::round(tw + 4), 37);
            SkPaint rp;
            rp.setAntiAlias(true);
            rp.setStyle(SkPaint::kStroke_Style);
            rp.setColor(0xFF101010);
            rp.setStrokeWidth(2);
            c.drawRRect(SkRRect::MakeRectXY(ring.makeInset(2, 2), 1.5f, 1.5f), rp);
            rp.setColor(pal::white);
            rp.setStrokeWidth(1);
            c.drawRRect(SkRRect::MakeRectXY(ring.makeInset(0.5f, 0.5f), 2.5f, 2.5f), rp);
        }
    };
    input_text(str(s, "reg"), 696.79f, 245, flag("sel", 0), flag("edit", 0), pal::cyan, 0);
    if (s.contains("menu") && s["menu"].is_array()) {
        float my = 291;
        for (auto& e : s["menu"]) {
            SkRect mr = SkRect::MakeXYWH(409, my, 600, 48);
            fill(c, mr, pal::black);
            bevel(c, mr, Bevel::Outset);
            Text mt;
            mt.size = 20;
            mt.align = Align::Center;
            text(c, e.get<std::string>(), 709, my + 10, mt);
            my += 44;
        }
    }
    ButtonOpts bo;
    button(c, SkRect::MakeLTRB(1049, 219, 1224, 299), {"SYNCHRO", "AVIONICS"}, bo);
    text(c, str(s, "msn"), 854.64f, 379, val);
    Text ver = val;
    ver.color = pal::white;
    text(c, str(s, "ver"), 854.64f, 487, ver);
    auto field = [&](SkRect r, const std::string& v, SkColor col, size_t idx) {
        fill(c, r, pal::black);
        bevel(c, r, Bevel::Inset, SkColorSetRGB(0xC4, 0xC6, 0xCF));
        col = v.find("[]") != std::string::npos ? amber : pal::cyan;
        input_text(v, r.centerX() + 0.29f, std::floor(r.centerY() - 13.5f), flag("sel", idx), flag("edit", idx), col, idx);
    };
    field(SkRect::MakeLTRB(409, 683, 1009, 720), str(s, "flt"), amber, 1);
    field(SkRect::MakeLTRB(409, 800, 659, 837), str(s, "from"), pal::cyan, 2);
    field(SkRect::MakeLTRB(764, 800, 1014, 837), str(s, "to"), pal::cyan, 3);
}

void draw_ops_menu(SkCanvas& c, const nlohmann::json& om) {
    const auto& cols = om["cols"];
    float colw = 1313.f / (float)cols.size();
    Text title;
    title.size = 32;
    title.align = Align::Center;
    ButtonOpts bo;
    for (size_t i = 0; i < cols.size(); i++) {
        float cx = 10 + colw * ((float)i + 0.5f);
        text(c, str(cols[i], "title"), cx, 130, title);
        float y = 188;
        for (auto& b : cols[i]["btns"]) {
            float left = cx - 150;
            SkRect r = SkRect::MakeLTRB(std::round(left), y, std::round(left + 300), y + 52);
            ButtonOpts o = bo;
            o.text_cx = cx;
            button(c, r, {b.get<std::string>()}, o);
            y += 72;
        }
    }
    std::string ex = str(om, "exit");
    if (!ex.empty()) {
        ButtonOpts o = bo;
        button(c, SkRect::MakeLTRB(1023, 868, 1323, 920), {ex}, o);
    }
}

void draw_main(SkCanvas& c, const nlohmann::json& m) {
    const float y0 = 10;
    Text w28;
    w28.size = 28;
    Text cy = w28;
    cy.color = pal::cyan;
    cy.align = Align::Center;

    float menu_w = selector(c, 10, -1, y0, "MENU");
    text(c, str(m, "title"), 10 + menu_w + 122.5f, y0 + 13, cy);
    selector(c, 0, 615.9375f, y0, "FUNCTIONS");
    Text msg = w28;
    msg.align = Align::Center;
    text(c, str(m, "msg"), 615.9375f + 62.5f, y0 + 13, msg);
    SkRect box = SkRect::MakeXYWH(741, y0, 430, 60);
    fill(c, box, pal::darker_grey);
    bevel(c, box, Bevel::Outset, pal::light_grey, 4);
    SkRect ib = SkRect::MakeXYWH(1171, y0, 60, 60);
    fill(c, ib, pal::darker_grey);
    bevel(c, ib, Bevel::Outset);
    triangle(c, ib.centerX() - 17.5f, ib.centerY() - 7, 35, 17.5f, true, 0xFF808080);
    ButtonOpts clr;
    clr.disabled = true;
    button(c, SkRect::MakeLTRB(1232, y0, 1323, y0 + 60), {"CLEAR"}, clr);

    const float by = 940;
    SkRect lb = SkRect::MakeXYWH(10, by, 60, 50);
    SkRect rb = SkRect::MakeXYWH(1263, by, 60, 50);
    for (auto* r : {&lb, &rb}) {
        fill(c, *r, pal::darker_grey);
        bevel(c, *r, Bevel::Outset);
    }
    auto arrow = [&](SkRect r, bool left) {
        SkPathBuilder b;
        float cx = r.centerX(), cy2 = r.centerY();
        if (left) {
            b.moveTo(cx - 9.1f, cy2);
            b.lineTo(cx + 9.1f, cy2 - 10.5f);
            b.lineTo(cx + 9.1f, cy2 + 10.5f);
        } else {
            b.moveTo(cx + 9.1f, cy2);
            b.lineTo(cx - 9.1f, cy2 - 10.5f);
            b.lineTo(cx - 9.1f, cy2 + 10.5f);
        }
        b.close();
        SkPaint p;
        p.setAntiAlias(true);
        p.setColor(pal::white);
        c.drawPath(b.detach(), p);
    };
    arrow(lb, true);
    arrow(rb, false);
    if (m.contains("tabs")) {
        float x = 70;
        for (auto& t : m["tabs"]) {
            button(c, SkRect::MakeXYWH(x, by, 225, 50), {t.get<std::string>()}, ButtonOpts{.pad_y = 10});
            x += 225;
        }
    }
}

void draw_menus(SkCanvas& c, const nlohmann::json& menus) {
    Text item;
    item.size = 20;
    for (auto& m : menus) {
        std::string anchor = str(m, "anchor");
        float x = anchor == "MENU" ? 10.f : anchor == "FUNCTIONS" ? 615.9375f - (text_width("FUNCTIONS", item.size == 20 ? Text{.size = 28} : item) + 43) : -1.f;
        if (x < 0) continue;
        float w = m.value("width", 300);
        float y = 64;
        const auto& items = m["items"];
        float h = 4 + 44.f * items.size();
        SkRect r = SkRect::MakeXYWH(x, y, w, h);
        fill(c, r, pal::black);
        bevel(c, r, Bevel::Outset);
        float iy = y + 2;
        for (auto& it : items) {
            Text t = item;
            if (it.value("disabled", false)) t.color = pal::dark_grey;
            text(c, str(it, "text"), x + 2 + 3 + 16, iy + 3 + 5, t);
            if (it.value("separator", false)) {
                SkPaint sp;
                sp.setAntiAlias(true);
                sp.setColor(pal::light_grey);
                c.drawRect(SkRect::MakeXYWH(x + 3.5f, iy + 41, w - 7, 3), sp);
            }
            iy += 44;
        }
    }
}

void draw_aoc(SkCanvas& c, const nlohmann::json& m) {
    draw_main(c, m);
    if (m.value("main_only", false)) {
        if (m.contains("opsmenu") && m["opsmenu"].is_object()) draw_ops_menu(c, m["opsmenu"]);
        if (m.contains("sts") && m["sts"].is_object()) draw_sts(c, m["sts"]);
        if (m.contains("perf") && m["perf"].is_array()) draw_perf(c, m["perf"]);
        if (m.contains("charts") && m["charts"].is_object()) draw_charts(c, m["charts"]);
        return;
    }
    const float lx = 10, ly = 80, lw = 300, lh = 795;
    border(c, SkRect::MakeXYWH(lx, ly, lw, lh), pal::light_grey, 3);
    SkRect head = SkRect::MakeXYWH(lx + 3, 93, lw - 6, 62);
    fill(c, SkRect::MakeXYWH(head.left(), head.top(), head.width(), 2), pal::light_grey);
    fill(c, SkRect::MakeXYWH(head.left(), head.bottom() - 2, head.width(), 2), pal::light_grey);
    fill(c, SkRect::MakeXYWH(head.right() - 2, head.top(), 2, head.height()), pal::light_grey);
    Text h26;
    h26.size = 26;
    h26.align = Align::Center;
    text(c, "COMPANY COM", head.centerX() - 1, 105, h26);

    Text item;
    item.size = 20;
    draw_folder(c, m);
    int nchild = 0;
    if (m.contains("items"))
        for (auto& it : m["items"])
            if (it.value("child", false)) nchild++;
    float top_child = 208.328125f;
    float top_file = nchild ? top_child + 38.f * nchild + 5.f : 213.328125f;
    if (m.contains("items"))
        for (auto& it : m["items"]) {
            std::string label = str(it, "label");
            bool child = it.value("child", false);
            float& top = child ? top_child : top_file;
            float x = child ? 53.f : 38.f;
            if (it.value("selected", false)) {
                float tw = text_width(label, item);
                fill(c, SkRect::MakeXYWH(x, top, tw, 28), pal::light_grey);
            }
            text(c, label, x, top, item);
            top += 38.f;
        }
    ButtonOpts o;
    button(c, SkRect::MakeXYWH(lx + 3, 820, lw - 6, 52), {"CLEAR ALL"}, o);
    if (m.contains("right") && m["right"].is_array()) draw_right(c, m["right"]);
}

}  // namespace a380::oit
