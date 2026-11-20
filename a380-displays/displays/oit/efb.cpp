#include "oit.h"
#include "a380/env.h"

#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <memory>
#include <tuple>

#include "include/codec/SkCodec.h"
#include "include/core/SkData.h"
#include "include/core/SkImage.h"
#include "include/core/SkPaint.h"
#include "include/core/SkSamplingOptions.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkRRect.h"
#include "include/effects/SkGradient.h"

#include "a380/assets.h"
#include "a380/draw.h"
#include "a380/fonts.h"

namespace a380::oit {

namespace {
std::string str(const nlohmann::json& j, const char* k) {
    return j.contains(k) && j[k].is_string() ? j[k].get<std::string>() : std::string();
}

SkColor rgba(uint32_t hex, uint8_t a = 255) { return SkColorSetARGB(a, (hex >> 16) & 255, (hex >> 8) & 255, hex & 255); }

void rrect(SkCanvas& c, SkRect r, float rad, SkColor col) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(col);
    c.drawRRect(SkRRect::MakeRectXY(r, rad, rad), p);
}

void rrect_stroke(SkCanvas& c, SkRect r, float rad, SkColor col) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(col);
    p.setStyle(SkPaint::kStroke_Style);
    p.setStrokeWidth(1);
    c.drawRRect(SkRRect::MakeRectXY(r.makeInset(0.5f, 0.5f), rad, rad), p);
}

float label(SkCanvas& c, const std::string& s, float x, float cy, float size, int weight, SkColor col,
            const char* family = "OitManrope", Align al = Align::Left, float ls = 0.f) {
    SkFont f = a380::font(family, size, weight, a380::TextMode::Lcd);
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(col);
    a380::Align a = al == Align::Left ? a380::Align::Left : al == Align::Center ? a380::Align::Center : a380::Align::Right;
    static const bool tlog = a380::getenv_cached("OIT_TEXTLOG") != nullptr;
    if (tlog) {
        float w = a380::text_width(f, s);
        std::fprintf(stderr, "TXT\t%s\t%.3f\t%.3f\t%.3f\t%.1f\n", s.c_str(), a == a380::Align::Left ? x : a == a380::Align::Center ? x - w / 2 : x - w, cy, w, size);
    }
    return a380::draw_text(c, s, x, cy, f, p, a, a380::Baseline::Central, ls);
}

float width_of(const std::string& s, float size, int weight, const char* family = "OitManrope") {
    return a380::text_width(a380::font(family, size, weight, a380::TextMode::Lcd), s);
}
}  // namespace

sk_sp<SkImage> tail_image() {
    thread_local sk_sp<SkImage> img;
    thread_local bool tried = false;
    if (tried) return img;
    tried = true;
    auto data = a380::assets::image_data("Common/fbw-tail.png");
    if (data) {
        std::unique_ptr<SkCodec> codec = SkCodec::MakeFromData(data);
        if (codec) img = std::get<0>(codec->getImage());
    }
    return img;
}

void draw_loading(SkCanvas& c, const nlohmann::json& m) {
    if (auto img = tail_image()) {
        SkRect dst = SkRect::MakeXYWH(592.f, 378.f, 160, 150);
        SkPaint ip;
        ip.setAntiAlias(true);
        c.drawImageRect(img, dst, SkSamplingOptions(SkFilterMode::kLinear, SkMipmapMode::kLinear), &ip);
        float fw = 1333.f * m.value("fill", 0.f) / 100.f;
        if (fw > 0) {
            SkRect r = SkRect::MakeXYWH(399.9f, 597.5f, fw, 25);
            fill(c, r, rgba(0xffffff));
            fill(c, r.makeInset(1, 1), rgba(0x00e0fe));
        }
        border(c, SkRect::MakeXYWH(399.9f, 597.5f, 533.2f, 25), rgba(0xffffff), 1);
        return;
    }
    SkPathBuilder b;
    b.moveTo(752, 378);
    b.lineTo(722, 527);
    b.lineTo(594, 527);
    b.cubicTo(615, 525, 633, 510, 646, 488);
    b.lineTo(696, 395);
    b.cubicTo(700, 385, 706, 378, 718, 378);
    b.close();
    SkPaint p;
    p.setAntiAlias(true);
    SkPoint pts[2] = {{594, 0}, {752, 0}};
    SkColor4f cols[2] = {SkColor4f::FromColor(rgba(0x00c6ff)), SkColor4f::FromColor(rgba(0x00e4ff))};
    p.setShader(SkShaders::LinearGradient(pts, SkGradient(SkGradient::Colors(SkSpan<const SkColor4f>(cols, 2), SkTileMode::kClamp), {})));
    c.drawPath(b.detach(), p);
    float fw = 1333.f * m.value("fill", 0.f) / 100.f;
    if (fw > 0) {
        SkRect r = SkRect::MakeXYWH(399.9f, 597.5f, fw + 2, 27);
        fill(c, r, rgba(0xffffff));
        fill(c, r.makeInset(1, 1), rgba(0x00e0fe));
    }
    border(c, SkRect::MakeXYWH(399.9f, 597.5f, 533.2f + 2, 27), rgba(0xffffff), 1);
}

void draw_charts(SkCanvas& c, const nlohmann::json& m) {
    const SkColor card = rgba(0x2a2c30), ctl = rgba(0x3a3c40), field = rgba(0x2c2e32), dim = rgba(0xc4c6ca),
                  head = rgba(0x9a9da2), text = rgba(0xf2f2f2), sel = rgba(0xff4d6a);
    rrect(c, SkRect::MakeXYWH(10, 70, 1313, 870), 18, rgba(0x1c1d20));
    const bool wide_bar = m.value("has_input", false) || !m["tabs"].empty();
    const float bar_h = wide_bar ? 48.f : 46.f;
    const float cy = 85 + bar_h * 0.5f;
    const float body_top = 85 + bar_h + 13;
    float x = 24;
    x += label(c, str(m, "title"), x, cy, 28, 700, text, "OitManrope", Align::Left, 0.5f) + 6 + 12;
    if (!m["tabs"].empty()) {
        float sx = x;
        float w = 6 + 2 * (m["tabs"].size() - 1);
        for (auto& t : m["tabs"]) w += width_of(t["s"].get<std::string>(), 17, 600) + 32;
        rrect(c, SkRect::MakeXYWH(sx, 85, w, 48), 12, field);
        float ix = sx + 3;
        for (auto& t : m["tabs"]) {
            std::string s = t["s"].get<std::string>();
            float iw = width_of(s, 17, 600) + 32;
            bool on = t.value("on", false);
            if (on) rrect(c, SkRect::MakeXYWH(ix, 88, iw, 42), 10, ctl);
            label(c, s, ix + iw * 0.5f, cy, 17, 600, on ? text : dim, "OitManrope", Align::Center);
            ix += iw + 2;
        }
        x = sx + w + 12;
    }
    label(c, str(m, "sub"), x, cy, 18, 600, head);
    float btn_left = 1309;
    {
        float bx = 1309;
        for (size_t i = m["btns"].size(); i-- > 0;) {
            const auto& bt = m["btns"][i];
            std::string bs = bt["s"].get<std::string>();
            float w = width_of(bs, 19, 600) + 36;
            SkRect b = SkRect::MakeXYWH(bx - w, 85, w, bar_h);
            if (bt.value("primary", false)) {
                SkPaint p;
                p.setAntiAlias(true);
                SkPoint pts[2] = {{b.left(), 0}, {b.right(), 0}};
                SkColor4f cols[2] = {SkColor4f::FromColor(rgba(0xf0703a)), SkColor4f::FromColor(rgba(0xff3f7a))};
                p.setShader(SkShaders::LinearGradient(pts, SkGradient(SkGradient::Colors(SkSpan<const SkColor4f>(cols, 2), SkTileMode::kClamp), {})));
                c.drawRRect(SkRRect::MakeRectXY(b, 12, 12), p);
            } else {
                rrect(c, b, 12, ctl);
            }
            label(c, bs, b.centerX(), cy, 19, 600, text, "OitManrope", Align::Center);
            bx -= w + 12;
            btn_left = b.left();
        }
    }
    label(c, str(m, "sub"), x, cy, 18, 600, head);
    float chip_right = m["btns"].empty() ? 1309.f : btn_left - 12;
    if (m.value("has_input", false)) {
        float chx = chip_right - 110;
        SkRect r = SkRect::MakeXYWH(chx, 85, 110, 48);
        rrect(c, r, 10, field);
        rrect_stroke(c, r, 10, rgba(0xffffff, 31));
        label(c, str(m, "input"), chx + 13, cy, 22, 400, text, "OitMono");
        chip_right = chx - 12;
    }
    {
        float total = 0;
        std::vector<float> ws;
        for (auto& ch : m["chips"]) {
            float w = 26 + width_of(ch[0].get<std::string>(), 14, 600) + 8 + width_of(ch[1].get<std::string>(), 20, 400, "OitMono");
            ws.push_back(w);
            total += w + 8;
        }
        float cx = chip_right - (total - 8);
        size_t i = 0;
        for (auto& ch : m["chips"]) {
            SkRect r = SkRect::MakeXYWH(cx, 85, ws[i], 48);
            rrect(c, r, 12, card);
            rrect_stroke(c, r, 12, rgba(0xffffff, 31));
            float kw = label(c, ch[0].get<std::string>(), cx + 13, cy + 0.5f, 14, 600, head);
            label(c, ch[1].get<std::string>(), cx + 13 + kw + 8, cy - 1, 20, 400, text, "OitMono");
            cx += ws[i] + 8;
            i++;
        }
    }
    const bool has_side = !m["side"].empty();
    if (has_side) rrect(c, SkRect::MakeXYWH(24, body_top, 380, 22.75f), 12, field);
    if (has_side) {
        float iw = 370.f / 3;
        for (size_t i = 0; i < m["side"].size(); i++) {
            bool on = m["side"][i].value("on", false);
            float ix = 27 + (iw + 2) * i;
            if (on) rrect(c, SkRect::MakeXYWH(ix, body_top + 3, iw, 16.75f), 10, ctl);
            label(c, m["side"][i]["s"].get<std::string>(), ix + iw * 0.5f, body_top + 24.5f, 17, 600, on ? text : dim,
                  "OitManrope", Align::Center);
        }
    }
    const bool has_tree = !m["tree"].empty();
    const float ty = has_side ? body_top + 32.75f : body_top;
    if (has_tree) rrect(c, SkRect::MakeXYWH(24, ty, 380, 926 - ty), 16, card);
    c.save();
    c.clipRRect(SkRRect::MakeRectXY(SkRect::MakeXYWH(24, ty, 380, 926 - ty), 16, 16), true);
    float y = ty + 8;
    for (auto& t : m["tree"]) {
        std::string s = t["s"].get<std::string>();
        if (t["k"] == "heading") {
            label(c, s, 44, y + 24, 17, 700, text);
            y += 42;
        } else if (t["k"] == "group") {
            label(c, s, 44, y + 27.5f, 15, 700, head, "OitManrope", Align::Left, 1.f);
            y += 48;
        } else {
            bool on = t.value("sel", false);
            if (on) {
                rrect(c, SkRect::MakeXYWH(32, y, 364, 48), 10, rgba(0xff4d6a, 41));
                SkPaint p;
                p.setAntiAlias(true);
                p.setColor(sel);
                c.save();
                c.clipRRect(SkRRect::MakeRectXY(SkRect::MakeXYWH(32, y, 364, 48), 10, 10), true);
                c.drawRect(SkRect::MakeXYWH(32, y, 3, 48), p);
                c.restore();
            }
            label(c, s, t.value("l1", false) ? 60.f : 44.f, y + 24, 18, 600, on ? text : dim);
            y += 48;
        }
    }
    c.restore();
    const float vx = has_tree ? 416.f : 24.f, vw = 1309.f - vx;
    rrect(c, SkRect::MakeXYWH(vx, body_top, vw, 926 - body_top), 16, card);
    label(c, str(m, "msg"), 440, body_top + 38, 20, 600, rgba(0xff9a5a));
    if (m.contains("mono")) {
        c.save();
        c.clipRRect(SkRRect::MakeRectXY(SkRect::MakeXYWH(vx, body_top, vw, 926 - body_top), 16, 16), true);
        float ly = body_top + 18;
        for (auto& ln : m["mono"]) {
            label(c, ln.get<std::string>(), vx + 18, ly + 11.f, 17, 400, text, "OitMono");
            ly += 22.95f;
        }
        c.restore();
    }
    {
        std::vector<float> ws;
        float total = 0;
        for (auto& t : m["tools"]) {
            float w = std::max(52.f, width_of(t.get<std::string>(), 19, 600) + 28);
            ws.push_back(w);
            total += w + 8;
        }
        total -= 8;
        float bx = 1289 - total;
        rrect(c, SkRect::MakeXYWH(bx - 6, 856, total + 12, 56), 14, rgba(0x212225, 247));
        size_t i = 0;
        for (auto& t : m["tools"]) {
            rrect(c, SkRect::MakeXYWH(bx, 862, ws[i], 44), 12, ctl);
            label(c, t.get<std::string>(), bx + ws[i] * 0.5f, 884, 19, 600, text, "OitManrope", Align::Center);
            bx += ws[i] + 8;
            i++;
        }
    }
}

}  // namespace a380::oit
