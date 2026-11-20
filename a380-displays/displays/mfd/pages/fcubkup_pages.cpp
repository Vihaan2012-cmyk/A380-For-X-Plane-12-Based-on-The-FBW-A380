#include "include/core/SkPaint.h"
#include "include/core/SkPathBuilder.h"

#include "page_model.h"

namespace a380::mfd {
namespace {
using namespace pm;

std::vector<std::string> wrap_hyphen(const std::string& s, const ui::TextStyle& ts, float maxw) {
    std::vector<std::string> out;
    std::string line;
    std::string word;
    auto trim = [](std::string& t) {
        while (!t.empty() && t.back() == ' ') t.pop_back();
    };
    auto flush_word = [&]() {
        if (word.empty()) return;
        if (!line.empty() && ui::text_width(line + word, ts) > maxw) {
            trim(line);
            out.push_back(line);
            line.clear();
        }
        line += word;
        word.clear();
    };
    for (char ch : s) {
        word += ch;
        if (ch == ' ' || ch == '-') flush_word();
    }
    flush_word();
    trim(line);
    if (!line.empty()) out.push_back(line);
    return out;
}

void stroke_path(SkCanvas& c, const SkPath& p) {
    SkPaint paint;
    paint.setAntiAlias(true);
    paint.setStyle(SkPaint::kStroke_Style);
    paint.setStrokeWidth(2);
    paint.setColor(ui::pal::white);
    c.drawPath(p, paint);
}

void draw_light(SkCanvas& c, float x, float y, const json& it) {
    const bool below = flag(it, "below");
    const bool on = flag(it, "on");
    const std::string label = str(it, "label");
    ui::TextStyle ts = ui::label_style(ui::pal::white, 22);
    ts.align = ui::Align::Center;
    const float w = below ? 96.f : 76.f;
    const float h = below ? 56.f : 84.f;
    float lx = x;
    if (below) lx = x + (std::max(w, ui::text_width(label, ts)) - w) * 0.5f;
    SkRect r = SkRect::MakeXYWH(lx, y, w, h);
    ui::fill(c, r, ui::pal::darker_grey);
    ui::bevel_border(c, r, ui::Bevel::Outset, ui::pal::light_grey);
    const float bw = (w - 4) * 0.7f;
    const float bx = r.centerX() - bw * 0.5f;
    for (int i = 0; i < 3; ++i)
        ui::fill(c, SkRect::MakeXYWH(bx, r.top() + 2 + 8 + i * 9, bw, 5), on ? ui::pal::green : 0xFF3A3A3A);
    if (below) ui::text(c, label, r.centerX(), r.bottom() + 6, ts);
    else ui::text(c, label, r.centerX(), r.bottom() - 2 - 6 - ui::line_height(ts), ts);
}

void draw_window(SkCanvas& c, float x, float y, float w, const json& it) {
    SkRect r = SkRect::MakeXYWH(x, y, w, 50);
    ui::fill(c, r, ui::pal::background);
    ui::bevel_border(c, r, ui::Bevel::Inset, ui::pal::light_grey);
    ui::TextStyle tv = ui::label_style(ui::pal::cyan, 32);
    ui::TextStyle tu = ui::label_style(ui::pal::cyan, 20);
    const std::string v = str(it, "text"), u = str(it, "unit");
    const float wv = ui::text_width(v, tv);
    const float total = wv + (u.empty() ? 0.f : 8.f + ui::text_width(u, tu));
    const float cx = r.centerX() - total * 0.5f;
    ui::text(c, v, cx, r.centerY() - ui::line_height(tv) * 0.5f, tv);
    if (!u.empty()) ui::text(c, u, cx + wv + 8, r.centerY() - ui::line_height(tu) * 0.5f, tu);
}

float draw_button(SkCanvas& c, float x, float y, float w, const json& b) {
    ui::ButtonStyle st = button_style(b);
    const ui::TextStyle ts = ui::label_style(ui::pal::white, 22);
    auto lines = wrap_hyphen(str(b, "label"), ts, w - 4 - st.pad_left - st.pad_right);
    if (lines.empty()) lines.push_back("");
    const float h = ui::button_height(22, static_cast<int>(lines.size()));
    SkRect r = SkRect::MakeXYWH(x, y, w, h);
    if (lines.size() == 1) ui::button(c, r, lines[0], st);
    else ui::button_lines(c, r, lines, st);
    return h;
}

void draw_fcubkup(SkCanvas& c, SkRect content, const json& page) {
    const float ox = content.left();
    const float oy = content.top();
    if (at(page, "lines").is_array()) {
        for (const json& l : page["lines"]) {
            SkPathBuilder pb;
            if (l.contains("rect")) {
                const auto& r = l["rect"];
                pb.addRect(SkRect::MakeXYWH(ox + r[0].get<float>(), oy + r[1].get<float>(), r[2].get<float>(),
                                            r[3].get<float>()));
            } else if (l.contains("poly")) {
                const auto& p = l["poly"];
                for (size_t i = 0; i + 1 < p.size(); i += 2) {
                    const float px = ox + p[i].get<float>(), py = oy + p[i + 1].get<float>();
                    if (i == 0) pb.moveTo(px, py);
                    else pb.lineTo(px, py);
                }
            }
            stroke_path(c, pb.detach());
        }
    }
    if (!at(page, "items").is_array()) return;
    for (const json& it : page["items"]) {
        const std::string k = str(it, "k");
        const float x = ox + it.value("left", 0.f);
        const float y = oy + it.value("top", 0.f);
        const float w = it.value("width", 0.f);
        if (k == "light") draw_light(c, x, y, it);
        else if (k == "window") draw_window(c, x, y, w, it);
        else if (k == "button") draw_button(c, x, y, w, it);
        else if (k == "label") {
            ui::TextStyle ts = ui::label_style(ui::pal::white, 24);
            ts.align = ui::Align::Center;
            float ly = y;
            for (const auto& s : wrap_hyphen(str(it, "text"), ts, w)) {
                ui::text(c, s, x + w * 0.5f, ly, ts);
                ly += ui::line_height(ts);
            }
        } else if (k == "stack") {
            float by = y;
            for (const json& b : it["buttons"]) by += draw_button(c, x, by, 150, b) + 4;
        } else if (k == "radios") {
            float ry = y;
            for (const json& r : it["items"]) {
                ui::radio_button(c, x, ry, str(r, "label"), flag(r, "checked"), ui::pal::green, false, 0, 1.0f);
                ry += 42;
            }
        }
    }
}

}  // namespace

A380_MFD_PAGE("fcubkup/", draw_fcubkup);

}  // namespace a380::mfd
