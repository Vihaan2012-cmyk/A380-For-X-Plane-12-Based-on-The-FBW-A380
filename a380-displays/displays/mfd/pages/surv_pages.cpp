#include "page_model.h"

namespace a380::mfd {
namespace {

using namespace pm;

SkRect R(SkRect content, float x0, float y0, float x1, float y1) { return box(content, x0, y0, x1, y1); }

void surv_button(SkCanvas& c, SkRect content, float x, float y, const json& b, float grow = 0) {
    SkRect r = R(content, x, y - grow, x + 100, y + 65);
    const bool dis = flag(b, "disabled");
    if (dis) {
        ui::fill(c, r, ui::pal::dark_grey);
    } else {
        ui::bevel_border(c, r, ui::Bevel::Inset, ui::pal::light_grey, 4);
    }
    ui::TextStyle big = ui::value_bigger_style(dis ? ui::pal::darker_grey : ui::pal::green);
    big.align = ui::Align::Center;
    ui::TextStyle small = ui::label_style(ui::pal::white, 20);
    small.align = ui::Align::Center;
    const float top = r.top() + grow + 10.2f;
    ui::text(c, str(b, "value"), r.centerX(), top, big);
    ui::text(c, str(b, "label"), r.centerX(), top + ui::line_height(big) - 0.67f, small);
}

void caption(SkCanvas& c, SkRect content, std::string_view s, float cx, float bottom, float size = 20) {
    ui::TextStyle ts = ui::label_style(ui::pal::white, size);
    ts.align = ui::Align::Center;
    std::vector<std::string> lines = split_lines(std::string(s));
    const float lh = ui::line_height(ts);
    float y = content.top() + bottom - lh * lines.size();
    for (auto& l : lines) {
        ui::text(c, l, content.left() + cx, y, ts);
        y += lh;
    }
}

bool checked(const json& radios, const char* name, size_t idx) {
    const json& g = at(radios, name);
    if (!g.is_array()) return idx == 0;
    bool any = false;
    for (const auto& r : g) any |= flag(r, "checked");
    if (!any) return idx == 0;
    return idx < g.size() && flag(g[idx], "checked");
}

void radio30(SkCanvas& c, float left, float top, std::string_view label, bool on) {
    const float cx = left + 15, cy = top + 15;
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(ui::pal::background);
    c.drawCircle(cx, cy, 15, p);
    SkPaint ring = p;
    ring.setStyle(SkPaint::kStroke_Style);
    ring.setStrokeWidth(2);
    ring.setColor(0xFF74757B);
    c.drawArc(SkRect::MakeXYWH(left + 1, top + 1, 28, 28), 135, 180, false, ring);
    ring.setColor(0xFFF2F4FF);
    c.drawArc(SkRect::MakeXYWH(left + 1, top + 1, 28, 28), -45, 180, false, ring);
    if (on) {
        p.setColor(ui::pal::green);
        c.drawCircle(cx, cy, 10, p);
    }
    ui::TextStyle ts{26, on ? ui::pal::green : ui::pal::white, ui::Face::Mfd, ui::Align::Left};
    ui::text(c, label, left + 45, cy - ui::line_height(ts) * 0.5f - 1.85f, ts);
}

void radios(SkCanvas& c, SkRect content, const json& rd, const char* name, float left, float top0, const float* tops, size_t n) {
    const json& g = at(rd, name);
    for (size_t i = 0; i < n; i++) {
        const std::string t = g.is_array() && i < g.size() ? str(g[i], "text") : "";
        radio30(c, content.left() + left, content.top() + tops[i] + top0, t, checked(rd, name, i));
    }
}

void draw_controls(SkCanvas& c, SkRect content, const json& page) {
    GreyLayer grey(c);
    const SkColor lg = ui::pal::light_grey;
    ui::fill(c, R(content, 306, 0, 308, 263.33f), lg);
    ui::fill(c, R(content, 766, 0, 768, 263.33f), lg);
    ui::fill(c, R(content, 591.33f, 73.33f, 593.33f, 230.67f), lg);
    ui::fill(c, R(content, 0, 263.33f, 768, 265.33f), lg);
    ui::fill(c, R(content, 0, 632.67f, 768, 634.67f), lg);
    ui::fill(c, R(content, 606, 634.67f, 608, 830), lg);

    ui::TextStyle head{28, ui::pal::white, ui::Face::Ecam, ui::Align::Left};
    ui::text(c, "XPDR", content.left() + 140.7f, content.top() + 1.67f, head);
    ui::text(c, "TCAS", content.left() + 463.3f, content.top() + 3.67f, head);
    ui::text(c, "WXR", content.left() + 189.3f, content.top() + 275.47f, head);
    ui::text(c, "TAWS", content.left() + 100.f, content.top() + 640.3f, head);

    ui::TextStyle big = ui::label_bigger_style();
    big.align = ui::Align::Center;
    ui::text(c, "SQWK", content.left() + 75.3f, content.top() + 54.f, big);
    ui::FieldStyle fs;
    fs.min_text_width = 72;
    ui::input_field(c, R(content, 25.33f, 75.33f, 125.33f, 112.33f), str(at(page, "squawk"), "text", "0000"), fs);
    ui::button(c, R(content, 25.33f, 117.33f, 125.33f, 154.67f), "IDENT");
    ui::text(c, "ALT RPTG", content.left() + 75.3f, content.top() + 174.35f, big);

    const json& bs = at(page, "buttons");
    auto B = [&](size_t i) -> const json& { return i < bs.size() ? bs[i] : at(page, "none"); };
    surv_button(c, content, 25.33f, 200.67f, B(0));

    const json& rd = at(page, "radios");
    const float xp[2] = {0, 86.67f};
    radios(c, content, rd, "CAPT_MFD_survControlsXpdrStatus", 163.33f, 97.33f, xp, 2);
    const float tc[3] = {0, 46.67f, 94.f};
    radios(c, content, rd, "CAPT_MFD_survControlsTcasTara", 390.67f, 88.67f, tc, 3);
    radios(c, content, rd, "CAPT_MFD_survControlsTcasNormAbvBlw", 605.33f, 88.67f, tc, 3);
    const float wx[3] = {0, 47.33f, 94.f};
    radios(c, content, rd, "CAPT_MFD_survControlswxrElevnTilt", 54.67f, 416.67f, wx, 3);
    ui::text(c, "ELEVN/TILT", content.left() + 114.67f, content.top() + 380.07f, big);

    const char* gl[6] = {"WXR", "PRED W/S", "TURB", "GAIN", "MODE", "WX ON VD"};
    const float gx[3] = {308.67f, 449.f, 589.33f};
    for (int i = 0; i < 6; i++) {
        const float x = gx[i % 3];
        const float by = i < 3 ? 345.33f : 470.f;
        caption(c, content, gl[i], x + 50, by - (i < 3 ? 2.83f : 2.5f));
        surv_button(c, content, x, by, B(1 + i));
    }

    ui::TextStyle lab = ui::label_style(ui::pal::white, 20);
    (void)lab;
    const char* tl[4] = {"TERR\nSYS", "GPWS", "G/S\nMODE", "FLAP\nMODE"};
    const float tx[4] = {80.f, 250.f, 370.f, 493.33f};
    for (int i = 0; i < 4; i++) {
        caption(c, content, tl[i], tx[i] + 50, 732.23f);
        surv_button(c, content, tx[i], 735.33f, B(7 + i), 0.67f);
    }
    caption(c, content, "SURV", 688, 738.5f);
    ui::button_lines(c, R(content, 618, 740.67f, 758, 797.33f), {"DEFAULT", "SETTINGS"});
}

float lh_dev(float size) {
    const float sz = size * 1.5f;
    return (std::round(0.8025f * sz) + std::round(0.0901f * sz)) / 1.5f;
}

void sys_box(SkCanvas& c, SkRect content, float x, float y, const json& b) {
    constexpr float bw = 0.667f;
    const float item = lh_dev(24);
    const json& groups = at(b, "groups");
    float h = bw * 2 + 50 + 10;
    for (size_t g = 0; g < groups.size(); g++) {
        const size_t n = at(groups[g], "items").size();
        h += bw * 2 + 10 + n * item + (n ? n - 1 : 0) * 10 + (g + 1 < groups.size() ? (g == 0 && groups.size() == 2 && n == 3 ? 10 : 5.67f) : 0);
    }
    SkRect r = R(content, x, y, x + 270, y + h);
    ui::stroke_box(c, r, ui::pal::light_grey, bw);
    float gy = y + bw + 50;
    for (size_t g = 0; g < groups.size(); g++) {
        const json& grp = groups[g];
        const json& items = at(grp, "items");
        const bool act = flag(grp, "active");
        const bool fail = flag(grp, "failed");
        const float gh = bw * 2 + 10 + items.size() * item + (items.size() ? items.size() - 1 : 0) * 10;
        const SkColor edge = fail ? ui::pal::amber : act ? ui::pal::green : ui::pal::light_grey;
        ui::stroke_box(c, R(content, x + bw + 10, gy, x + 270 - bw - 10, gy + gh), edge, bw);
        float iy = gy + bw + 5;
        for (const auto& it : items) {
            const SkColor col = flag(it, "failed") ? ui::pal::amber : flag(it, "active") ? ui::pal::green : ui::pal::white;
            ui::TextStyle ts = ui::label_style(col, 24);
            ui::text(c, str(it, "text"), content.left() + x + bw + 10 + bw + 5, content.top() + iy, ts);
            iy += item + 10;
        }
        gy += gh + (g == 0 && groups.size() == 2 && items.size() == 3 ? 10 : 5.67f);
    }
    const bool on = flag(b, "active");
    SkRect btn = R(content, x + 90 + 0.f, y - 34.f + bw, x + 180, y - 34.f + bw + 60);
    ui::fill(c, btn, ui::pal::light_grey);
    SkRect in = btn.makeInset(2, 2);
    ui::fill(c, in, 0xFF3C3C3C);
    SkRect top = SkRect::MakeLTRB(in.left(), in.top(), in.right(), in.top() + in.height() * 0.5f);
    ui::fill(c, top, ui::pal::background);
    const float bh = in.height() * 0.5f * 0.143f;
    const float gap = (in.height() * 0.5f - 3 * bh) / 4;
    for (int i = 0; i < 3; i++) {
        const float by = top.top() + gap + i * (bh + gap);
        ui::fill(c, SkRect::MakeLTRB(in.left() + 7, by, in.right() - 7, by + bh), on ? ui::pal::green : ui::pal::light_grey);
    }
    ui::TextStyle ts = ui::label_style(ui::pal::white, 22);
    ts.align = ui::Align::Center;
    ui::text(c, str(b, "label"), in.centerX(), top.bottom() + 22 + 3, ts);
}

void draw_status(SkCanvas& c, SkRect content, const json& page) {
    const json& boxes = at(page, "boxes");
    if (!boxes.is_array() || boxes.size() < 4) return;
    const float row2 = 50 + (0.667f * 2 + 50 + 10 + (0.667f * 2 + 10 + 3 * lh_dev(24) + 20) + 10 + (0.667f * 2 + 10 + 2 * lh_dev(24) + 10)) + 50 + 50;
    sys_box(c, content, 50, 50, boxes[0]);
    sys_box(c, content, 448, 50, boxes[1]);
    sys_box(c, content, 50, row2, boxes[2]);
    sys_box(c, content, 448, row2, boxes[3]);
    ui::TextStyle big = ui::label_bigger_style();
    big.align = ui::Align::Center;
    const float lb = lh_dev(24);
    ui::text(c, "WXR", content.left() + 384, content.top() + 50 + 55, big);
    ui::text(c, "TAWS", content.left() + 384, content.top() + 50 + 55 + lb + 90, big);
    ui::text(c, "XPDR", content.left() + 384, content.top() + row2 + 55, big);
    ui::text(c, "TCAS", content.left() + 384, content.top() + row2 + 55 + lb + 20, big);
}

void draw_surv(SkCanvas& c, SkRect content, const json& page) {
    if (str(page, "view") == "controls") draw_controls(c, content, page);
    else if (str(page, "view") == "status") draw_status(c, content, page);
}

}  // namespace

A380_MFD_PAGE("surv/", draw_surv);

}  // namespace a380::mfd
