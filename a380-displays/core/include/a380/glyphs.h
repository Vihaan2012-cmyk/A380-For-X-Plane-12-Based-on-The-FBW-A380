#pragma once

#include <cstdint>

#include "include/core/SkFont.h"

namespace a380::glyphs {

struct Mask {
    int left = 0, top = 0, w = 0, h = 0;
    const uint8_t* rgb = nullptr;
    const uint8_t* raw = nullptr;
};

bool enabled();
bool baking();
bool verifying();

bool get(const SkFont& scaled_lcd_font, SkGlyphID glyph, int subpixel, Mask& out);

void note_verify(bool match, int differing_pixels);

const uint8_t* lcd_preblend(int bucket, float contrast, float gamma);

}  // namespace a380::glyphs
