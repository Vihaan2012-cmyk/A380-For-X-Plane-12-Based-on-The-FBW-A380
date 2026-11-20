#pragma once

#include <string>
#include <string_view>

#include "include/core/SkFont.h"
#include "include/core/SkFontTypes.h"
#include "include/core/SkRefCnt.h"
#include "include/core/SkTypeface.h"

namespace a380 {

// The aircraft's own fonts (html_ui/Fonts/fbw-a380x and the files FlyByWire's
// CSS @font-face rules name), by the family name the CSS uses, e.g. "Ecam",
// "FBW-Display-EIS-A380", "Inter". Falls back to the system font manager.
// The font folder is A380_FONT_DIR, or the installed aircraft's html_ui/Fonts.
sk_sp<SkTypeface> typeface(std::string_view css_family, bool bold = false);

// Lower-case file name of a typeface loaded from the font folder (the stable identity of a face across platforms),
// empty for system or unknown typefaces.
std::string typeface_key(const SkTypeface* tf);

// CSS font-weight (100..900): the nearest face the family has (Manrope has 400 / 600 / 700, Inter 400 / 600, ...).
sk_sp<SkTypeface> typeface(std::string_view css_family, int weight);

// An SkFont set up the way Chromium rasterises text on Windows (subpixel positioning, no hinting), at `px`
// CSS pixels. Default edging is greyscale AA, which a380::draw_text / a380::draw_simple_text (draw.h) render as
// Chromium's greyscale text (average of the ClearType subpixel coverages). Pass lcd=true only for text Chromium
// draws with LCD AA (opaque background in the same layer); the surface must then carry text_surface_props().
SkFont font(std::string_view css_family, float px, bool bold = false, bool lcd = false);

enum class TextMode { Greyscale, Lcd };

// What decides whether Chromium rasterises a text run with LCD or greyscale AA: the composited layer holding the text
// must be opaque where the text is (background painted in the same layer), axis-aligned (no rotate/skew/scale
// transform, no perspective) and integer-translated, and no ancestor may make a layer that is not opaque
// (opacity < 1, filter, backdrop-filter, will-change opacity/transform on a non-opaque layer, mix-blend-mode).
struct TextLayerFacts {
    bool opaque_background = true;
    float opacity = 1.f;
    bool non_translate_transform = false;
    bool fractional_translate = false;
    bool filter_or_blend = false;
    bool will_change_non_opaque = false;
};

TextMode chromium_text_mode(bool layer_opaque, bool axis_aligned = true, bool integer_translate = true);
TextMode chromium_text_mode(const TextLayerFacts& facts);

// font() with the edging chosen by chromium_text_mode(...). Env A380_TEXT_LCD=0 / 1 forces every helper-made font
// greyscale / LCD for experiments.
SkFont font(std::string_view css_family, float px, bool bold, TextMode mode);

// CSS font-weight overload: font("Manrope", 14.f, 600) is SemiBold, font("Manrope", 14.f, 700, TextMode::Lcd) Bold.
SkFont font(std::string_view css_family, float px, int weight, TextMode mode = TextMode::Greyscale);

// Pair kerning of a typeface (GPOS kern PairPos formats 1 and 2, else the legacy kern table), as HarfBuzz applies it in
// Chromium: the x advance added to `left` when `right` follows, as a fraction of the em.
bool has_kerning(const SkTypeface* tf);
float kerning_em(const SkTypeface* tf, SkGlyphID left, SkGlyphID right);

}  // namespace a380
