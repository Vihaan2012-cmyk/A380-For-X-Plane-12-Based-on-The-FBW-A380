#pragma once

#include <cmath>
#include <cstdint>
#include <string>
#include <string_view>

#include "include/core/SkCanvas.h"
#include "include/core/SkImage.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPath.h"

#include "a380/draw.h"
#include "a380/fonts.h"
#include "a380/inputs.h"

namespace sdsys {

constexpr uint32_t kWhite = 0xffffff, kGreen = 0x00ff00, kAmber = 0xe68000, kCyan = 0x00ffff, kRed = 0xff0000,
                   kLightGrey = 0xc4c6cf, kGrey = 0x4f5467, kMagenta = 0xff94ff;

struct TextStyle {
    float px = 25.f;
    uint32_t color = kWhite;
    a380::Align align = a380::Align::Left;
    a380::Baseline base = a380::Baseline::Alphabetic;
    float ls = 0.f;
    float ws = 0.f;
    bool underline = false;
    float outline = 0.f;
    bool plain = false;
};

bool stroked_text_fir(SkCanvas& cv, std::string_view text, float x, float y, const SkFont& font, uint32_t color, float sw,
                      float ls = 0.f, float ws = 0.f);
float text(SkCanvas& cv, std::string_view s, float x, float y, const TextStyle& st);
SkPath svg_path(std::string_view d);
void stroke_path(SkCanvas& cv, const SkPath& p, uint32_t color, float width, bool round = false);
void fill_path(SkCanvas& cv, const SkPath& p, uint32_t color);
void triangle(SkCanvas& cv, float x, float y, uint32_t color, float orientation, float scale = 1.f);
void line(SkCanvas& cv, float x1, float y1, float x2, float y2, uint32_t color, float width);
int js_round(double v);
sk_sp<SkImage> load_image(const std::string& name);
sk_sp<SkImage> chromium_mip(const sk_sp<SkImage>& img, SkCanvas& cv, float css_w);
void draw_image_two_step(SkCanvas& cv, const sk_sp<SkImage>& img, const SkRect& dst);
void draw_hyd(SkCanvas& cv, const a380::Inputs& in);
void draw_press(SkCanvas& cv, const a380::Inputs& in);
void draw_wheel(SkCanvas& cv, const a380::Inputs& in);

inline double v(const a380::Inputs& in, std::string_view n) { return in.var(n); }
inline bool b(const a380::Inputs& in, std::string_view n) { return in.var(n) != 0.0; }

void draw_elec_ac(SkCanvas& cv, const a380::Inputs& in);
void draw_eng(SkCanvas& cv, const a380::Inputs& in);
void draw_elec_dc(SkCanvas& cv, const a380::Inputs& in);
void draw_cb(SkCanvas& cv, const a380::Inputs& in);
void draw_apu(SkCanvas& cv, const a380::Inputs& in);
void draw_bleed(SkCanvas& cv, const a380::Inputs& in);
void draw_cond(SkCanvas& cv, const a380::Inputs& in);
void draw_door(SkCanvas& cv, const a380::Inputs& in);
void draw_fuel(SkCanvas& cv, const a380::Inputs& in);

struct A429 {
    int ssm;
    float val;
    bool normal() const { return ssm == 3; }
};
A429 decode429(double raw);
inline A429 var429(const a380::Inputs& in, std::string_view n) { return decode429(in.var(n)); }

inline TextStyle T(float px, uint32_t c, a380::Align al = a380::Align::Left, float ls = 0.f) {
    TextStyle s;
    s.px = px;
    s.color = c;
    s.align = al;
    s.ls = ls;
    if (al == a380::Align::Center) s.base = a380::Baseline::Middle;
    return s;
}
inline TextStyle TM(float px, uint32_t c, float ls = 0.f) { return T(px, c, a380::Align::Center, ls); }

void circle_stroke(SkCanvas& cv, float cx, float cy, float r, uint32_t color, float width);
void circle_fill(SkCanvas& cv, float cx, float cy, float r, uint32_t color);
SkPaint sk_stroke(uint32_t color, float width, bool round = false);
void draw_path(SkCanvas& cv, const SkPath& path, const SkPaint& paint);
void draw_line(SkCanvas& cv, float x0, float y0, float x1, float y1, const SkPaint& paint);
void draw_rect(SkCanvas& cv, const SkRect& r, const SkPaint& paint);
void arc_path(SkCanvas& cv, float x, float y, float r, float a0, float a1, uint32_t color, float width);
void gauge_marker(SkCanvas& cv, float x, float y, float value, float mn, float mx, float r, float sa, float ea,
                  uint32_t color, float width, bool round, bool indicator = false, bool half = false,
                  float mi = 0.85f, float mo = 1.15f);
void gauge_dir(float value, float mn, float mx, float sa, float ea, float& dx, float& dy);
void valve(SkCanvas& cv, float x, float y, float r, bool vertical, uint32_t color, float width = 2.f);
void rect_stroke(SkCanvas& cv, float x, float y, float w, float h, uint32_t color, float width);
void rect_fill(SkCanvas& cv, float x, float y, float w, float h, uint32_t color);
bool bit429(const a380::Inputs& in, std::string_view n, int bit);

}  // namespace sdsys
