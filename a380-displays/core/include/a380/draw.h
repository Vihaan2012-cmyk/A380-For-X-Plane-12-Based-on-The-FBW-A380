#pragma once

#include <cmath>
#include <string>
#include <string_view>
#include <vector>

#include "include/core/SkCanvas.h"
#include "include/core/SkColor.h"
#include "include/core/SkFont.h"
#include "include/core/SkFontMetrics.h"
#include "include/core/SkFontTypes.h"
#include "include/core/SkPaint.h"
#include "include/core/SkRRect.h"
#include "include/core/SkSurfaceProps.h"
#include "include/core/SkRect.h"

namespace a380 {

// Surface properties that make Skia's text coverage match the golden's Chromium
// text on black (the golden uses LCD text, RGB stripe order).
inline SkSurfaceProps text_surface_props(float contrast = 1.0f, float gamma = 2.2f) {
    return SkSurfaceProps(0, kRGB_H_SkPixelGeometry, contrast, gamma);
}

// While on (this thread), the canvas is a page with a transparent background: LCD text that would go through Skia's own
// LCD blit (which writes opaque pixels) is drawn as Chromium's greyscale text instead.
void set_transparent_target(bool on);
bool transparent_target();

// Draws text the way Chromium rasterises greyscale text on Windows: the glyph mask is the average of the
// RGB-stripe (ClearType) subpixel coverages, positioned in device space with 1/4 px x subpixel steps and a
// device-snapped baseline. Honours the canvas matrix (uniform scale + translate; anything else falls back to
// a plain drawSimpleText). A380_TEXT_MODE=direct switches back to Skia's own A8 text.
void draw_simple_text(SkCanvas& canvas, const void* text, size_t bytes, SkTextEncoding encoding, float x, float y,
                      const SkFont& font, const SkPaint& paint);

// Glyphs at their own x (baseline y), as one draw when draw_simple_text would draw each straight through Skia.
void draw_glyph_run(SkCanvas& canvas, const SkGlyphID* glyphs, const float* xs, size_t n, float y, const SkFont& font, const SkPaint& paint);

// SVG text with fill and stroke: glyph outlines (+ stroke_width in user units) as one mask: exact-area coverage at 3x
// horizontal, averaged, SkMaskGamma LUT of the stroke colour. When fill != stroke the fill is drawn first as ordinary
// text and only the stroke ring is masked. Returns the advance. A380_STROKE_MODE=0|1 forces sequential|union.
float draw_stroked_text(SkCanvas& canvas, std::string_view text, float x, float y, const SkFont& font, SkColor fill,
                        SkColor stroke, float stroke_width);

enum class Align { Left, Center, Right };

// Where the y passed to draw_text sits on the text, like canvas textBaseline
// and SVG dominant-baseline: Middle is half an x-height above the baseline,
// Central is the middle of the em box.
enum class Baseline { Alphabetic, Middle, Central, Hanging, Top, Bottom, SvgCentral };

constexpr SkColor rgb(uint32_t hex, uint8_t a = 255) {
    return SkColorSetARGB(a, (hex >> 16) & 0xff, (hex >> 8) & 0xff, hex & 0xff);
}

// Advance width of a run including pair kerning (GPOS/kern, applied like HarfBuzz in Chromium). Equal to
// font.measureText for fonts without kerning pairs.
float text_advance(const SkFont& font, const void* text, size_t bytes, SkTextEncoding encoding);

// Kerning adjustment in px between two glyphs of `font` (0 when the typeface has no pair for them).
float kerning_px(const SkFont& font, SkGlyphID left, SkGlyphID right);

inline float text_width(const SkFont& font, std::string_view text, float letter_spacing = 0.f) {
    if (text.empty()) return 0.f;
    float w = text_advance(font, text.data(), text.size(), SkTextEncoding::kUTF8);
    if (letter_spacing != 0.f) {
        size_t glyphs = font.textToGlyphs(text.data(), text.size(), SkTextEncoding::kUTF8, {});
        w += letter_spacing * (float)glyphs;
    }
    return w;
}

float svg_central_offset(const SkFont& font);

inline float baseline_offset(const SkFont& font, Baseline b) {
    SkFontMetrics m;
    font.getMetrics(&m);
    switch (b) {
        case Baseline::Alphabetic: return 0.f;
        case Baseline::Middle: {
            float xh = m.fXHeight > 0 ? m.fXHeight : -m.fAscent * 0.5f;
            return xh * 0.5f;
        }
        case Baseline::Central: return -(m.fAscent + m.fDescent) * 0.5f;
        case Baseline::SvgCentral: return svg_central_offset(font);
        case Baseline::Hanging: return -m.fAscent * 0.8f;
        case Baseline::Top: return -m.fAscent;
        case Baseline::Bottom: return -m.fDescent;
    }
    return 0.f;
}

// Baseline y of a CSS line box starting at `top` with the given line-height:
// Chromium rounds ascent and descent to whole pixels and splits the leading evenly.
inline float css_baseline(const SkFont& font, float top, float line_height) {
    SkFontMetrics m;
    font.getMetrics(&m);
    float asc = std::round(-m.fAscent), desc = std::round(m.fDescent);
    return top + std::floor((line_height - (asc + desc)) * 0.5f) + asc;
}

// Blink LayoutUnit: values are truncated to 1/64 px.
float layout_unit(float v);

// Baseline y (CSS px, exactly on a device pixel) where Chromium paints the first baseline of a CSS line box
// whose top is `line_top` (CSS px) with the given line-height, for `font` created at its CSS size. Works in
// device space like zoom-for-dsf: top and line-height become 1/64 LayoutUnits of device pixels, ascent and
// descent are rounded to whole device pixels, the half leading is (line_height - ascent - descent) / 2 in LayoutUnits, and the result is rounded to a device
// pixel. line_height <= 0 means `line-height: normal` (rounded ascent + descent + line gap, the gap split evenly). For a stack of lines, pass each line's own top (first_top + i * line_height, accumulated in LayoutUnits
// by the caller, e.g. layout_unit(line_height * dpr) per line).
float chromium_baseline(const SkFont& font, float line_top, float line_height, float dpr);

// The hhea ascent / descent (positive) / lineGap of the font's typeface as fractions of the em; Chromium (Blink
// SimpleFontData) lays HTML lines out with these. Falls back to Skia's metrics when the typeface has no hhea table.
struct HheaMetrics {
    float ascent = 0.f, descent = 0.f, line_gap = 0.f;
    bool valid = false;
};
HheaMetrics hhea_metrics(const SkFont& font);

// Rounds a CSS y to the device pixel Chromium paints HTML text at (round half up). SVG text is not snapped.
float chromium_snap(float css_y, float dpr);

// Draws one line. Returns its advance width (including letter spacing).
inline float draw_text(SkCanvas& canvas, std::string_view text, float x, float y, const SkFont& font,
                       const SkPaint& paint, Align align = Align::Left, Baseline base = Baseline::Alphabetic,
                       float letter_spacing = 0.f) {
    if (text.empty()) return 0.f;
    float w = text_width(font, text, letter_spacing);
    float x0 = align == Align::Left ? x : align == Align::Center ? x - w * 0.5f : x - w;
    float by = y + baseline_offset(font, base);
    if (letter_spacing == 0.f) {
        draw_simple_text(canvas, text.data(), text.size(), SkTextEncoding::kUTF8, x0, by, font, paint);
        return w;
    }
    std::vector<SkGlyphID> glyphs(text.size());
    size_t n = font.textToGlyphs(text.data(), text.size(), SkTextEncoding::kUTF8, SkSpan<SkGlyphID>(glyphs));
    glyphs.resize(n);
    std::vector<SkScalar> widths(n);
    font.getWidths(SkSpan<const SkGlyphID>(glyphs), SkSpan<SkScalar>(widths));
    std::vector<float> xs(n);
    float cx = x0;
    for (size_t i = 0; i < n; i++) {
        xs[i] = cx;
        cx += widths[i] + letter_spacing + (i + 1 < n ? kerning_px(font, glyphs[i], glyphs[i + 1]) : 0.f);
    }
    draw_glyph_run(canvas, glyphs.data(), xs.data(), n, by, font, paint);
    return w;
}

// SVG <text>/<tspan> run: Blink snaps the run's first glyph origin to a whole device pixel (round half up), the following
// glyphs keep the unrounded pen advance. Measured on sdv2 STS (letter-spacing 1.75): per-glyph x offsets vs golden go from
// 0.15..0.4 px to 0.0 and the page 92.0 -> 94.8. `x` is the anchor in user space (no fudge offsets), `align` the text-anchor.
inline float draw_svg_text(SkCanvas& canvas, std::string_view text, float x, float y, const SkFont& font,
                           const SkPaint& paint, Align align = Align::Left, Baseline base = Baseline::Alphabetic,
                           float letter_spacing = 0.f) {
    if (text.empty()) return 0.f;
    float w = text_width(font, text, letter_spacing);
    float x0 = align == Align::Left ? x : align == Align::Center ? x - w * 0.5f : x - w;
    SkMatrix m = canvas.getLocalToDeviceAs3x3();
    float sx = m.getScaleX(), tx = m.getTranslateX();
    if (sx > 0.f && m.getSkewX() == 0.f && m.getSkewY() == 0.f) x0 = (std::floor(x0 * sx + tx + 0.5f) - tx) / sx;
    return draw_text(canvas, text, x0, y, font, paint, Align::Left, base, letter_spacing);
}

inline void fill_rrect(SkCanvas& canvas, const SkRect& r, float radius, SkColor color) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(color);
    if (radius > 0) canvas.drawRRect(SkRRect::MakeRectXY(r, radius, radius), p);
    else canvas.drawRect(r, p);
}

// CSS border: the stroke lies inside `r`.
inline void stroke_rrect_inside(SkCanvas& canvas, const SkRect& r, float radius, float width, SkColor color) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setStyle(SkPaint::kStroke_Style);
    p.setStrokeWidth(width);
    p.setColor(color);
    SkRect rr = r.makeInset(width * 0.5f, width * 0.5f);
    float rad = radius > width * 0.5f ? radius - width * 0.5f : 0.f;
    if (rad > 0) canvas.drawRRect(SkRRect::MakeRectXY(rr, rad, rad), p);
    else canvas.drawRect(rr, p);
}

// SVG/canvas stroke: centred on the path through `r`.
inline void stroke_rrect(SkCanvas& canvas, const SkRect& r, float radius, float width, SkColor color) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setStyle(SkPaint::kStroke_Style);
    p.setStrokeWidth(width);
    p.setColor(color);
    if (radius > 0) canvas.drawRRect(SkRRect::MakeRectXY(r, radius, radius), p);
    else canvas.drawRect(r, p);
}

}  // namespace a380
