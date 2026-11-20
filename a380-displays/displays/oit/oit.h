#pragma once

#include <string>
#include <string_view>
#include <vector>

#include "include/core/SkCanvas.h"
#include "include/core/SkColor.h"
#include "include/core/SkRect.h"

#include "nlohmann/json.hpp"

namespace a380::oit {

namespace pal {
constexpr SkColor white = 0xFFFFFFFF;
constexpr SkColor black = 0xFF000000;
constexpr SkColor cyan = 0xFF00FFFF;
constexpr SkColor light_grey = 0xFFC4C6CF;
constexpr SkColor dark_grey = 0xFF868A9C;
constexpr SkColor darker_grey = 0xFF272525;
constexpr SkColor grey = 0xFF4F5467;
}  // namespace pal

enum class Align { Left, Center, Right };
enum class Bevel { Outset, Inset };

struct Text {
    float size = 22;
    SkColor color = pal::white;
    bool bold = false;
    Align align = Align::Left;
    float line_height = 28;
    const char* family = "OIT";
    bool hinted = false;
};

float dpr();
void set_dpr(float v);
float text_width(std::string_view s, const Text& t);
void text(SkCanvas& c, std::string_view s, float x, float line_top, const Text& t);
void fill(SkCanvas& c, SkRect r, SkColor col);
void bevel(SkCanvas& c, SkRect r, Bevel kind, SkColor color = pal::light_grey, float w = 2);
void border(SkCanvas& c, SkRect r, SkColor col, float w = 1);

struct ButtonOpts {
    bool disabled = false;
    bool selected = false;
    float size = 20;
    float line_height = 28;
    float pad_x = 12, pad_y = 10;
    float text_cx = -1;
};
SkRect button(SkCanvas& c, SkRect r, const std::vector<std::string>& lines, const ButtonOpts& o = {});
float button_text_width(const std::vector<std::string>& lines, const ButtonOpts& o = {});

void draw_main(SkCanvas& c, const nlohmann::json& m);
void draw_aoc(SkCanvas& c, const nlohmann::json& m);
void draw_perf(SkCanvas& c, const nlohmann::json& ops);
void draw_charts(SkCanvas& c, const nlohmann::json& m);
void draw_loading(SkCanvas& c, const nlohmann::json& m);
void draw_menus(SkCanvas& c, const nlohmann::json& menus);
void draw_login(SkCanvas& c, const nlohmann::json& m);

}  // namespace a380::oit
