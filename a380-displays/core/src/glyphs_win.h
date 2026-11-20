#pragma once

#include <cstdint>
#include <string>
#include <vector>

#include "include/core/SkFont.h"

namespace a380::glyphs::win {

struct Proof {
    long glyphs = 0, combos = 0, bad_combos = 0, pixels = 0, bad_pixels = 0, unsupported = 0;
    long grey_combos = 0, grey_bad = 0;
};

bool bake(const std::string& face, const SkFont& font, SkGlyphID glyph, int subpixel, int& left, int& top, int& w, int& h,
          std::vector<uint8_t>& rgb);

void prove(const SkFont& font, SkGlyphID glyph, int subpixel, int left, int top, int w, int h, const uint8_t* rgb, Proof& out);

}  // namespace a380::glyphs::win
