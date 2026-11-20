#include "oit.h"

#include <algorithm>
#include <cmath>

namespace a380::oit {

namespace {
std::string str(const nlohmann::json& j, const char* k) {
    return j.contains(k) && j[k].is_string() ? j[k].get<std::string>() : std::string();
}
std::vector<std::string> split_lines(const std::string& s) {
    std::vector<std::string> out;
    size_t p = 0;
    while (true) {
        size_t q = s.find('\n', p);
        out.push_back(s.substr(p, q == std::string::npos ? q : q - p));
        if (q == std::string::npos) break;
        p = q + 1;
    }
    return out;
}
}  // namespace

void draw_login(SkCanvas& c, const nlohmann::json& m) {
    const float W = 1333, H = 1000;
    fill(c, SkRect::MakeWH(W, H), pal::black);
    Text head;
    head.size = 30;
    head.bold = true;
    head.align = Align::Center;
    text(c, str(m, "heading"), W / 2, H * 0.25f, head);
    Text sub;
    sub.size = 24;
    sub.align = Align::Center;
    text(c, str(m, "sub"), W / 2, H * 0.32f, sub);
    text(c, str(m, "subsub"), W / 2, H * 0.39f, sub);

    float fx = W * 0.20f, x = std::round(fx), y = std::round(H * 0.65f);
    ButtonOpts o;
    float bw = 0;
    if (m.contains("buttons"))
        for (auto& b : m["buttons"]) bw = std::max(bw, button_text_width(split_lines(str(b, "label")), o));
    bw += 2 * o.pad_x + 4;
    bool firstb = true;
    if (m.contains("buttons"))
        for (auto& b : m["buttons"]) {
            auto lines = split_lines(str(b, "label"));
            float h = o.line_height * lines.size() + 2 * o.pad_y + 4;
            if (!firstb) y += 20;
            ButtonOpts bo = o;
            bo.disabled = b.value("disabled", false);
            bo.text_cx = fx + bw * 0.5f;
            button(c, SkRect::MakeLTRB(x, y, std::round(fx + bw), y + h), lines, bo);
            y += h;
            firstb = false;
        }
    std::string sw = str(m, "switch");
    if (!sw.empty()) {
        auto lines = split_lines(sw);
        float w = button_text_width(lines, o) + 2 * o.pad_x + 4;
        float h = o.line_height * lines.size() + 2 * o.pad_y + 4;
        button(c, SkRect::MakeXYWH(10, H - 40 - h, w, h), lines, o);
    }
}

}  // namespace a380::oit
