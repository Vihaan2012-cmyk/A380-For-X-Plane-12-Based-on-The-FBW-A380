#include "a380/draw.h"
#include "a380/env.h"
#include "a380/raster.h"

#include "a380/fonts.h"
#include "a380/glyphs.h"

#include <algorithm>
#include <cstdint>
#include <string>
#include <unordered_map>
#include <cmath>
#include <cstdlib>
#include <cstdio>
#include <cstring>
#include <vector>

#include "include/core/SkBitmap.h"
#include "include/core/SkBlender.h"
#include "include/core/SkImage.h"
#include "include/core/SkM44.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkPathUtils.h"
#include "include/core/SkSamplingOptions.h"
#include "include/core/SkSurface.h"
#include "include/core/SkTextBlob.h"
#include "include/core/SkTypeface.h"
#include "include/effects/SkRuntimeEffect.h"
#ifdef A380_GPU
#include "include/gpu/ganesh/GrDirectContext.h"
#include "include/gpu/ganesh/GrRecordingContext.h"
#include "include/gpu/ganesh/SkImageGanesh.h"
#endif

namespace a380 {

namespace {

thread_local bool g_transparent_target = false;

bool direct_mode() {
    static const bool d = [] {
        const char* v = a380::getenv_cached("A380_TEXT_MODE");
        return v && !std::strcmp(v, "direct");
    }();
    return d;
}

float text_gamma() {
    static const float g = [] {
        const char* v = a380::getenv_cached("A380_TEXT_GAMMA_LCDGREY");
        return v ? (float)std::atof(v) : 2.4f;
    }();
    return g;
}

float env_f(const char* k, float d) {
    const char* v = a380::getenv_cached(k);
    return v ? (float)std::atof(v) : d;
}

struct MaskGammaCfg {
    bool on = true;
    float contrast = 1.0f, gamma = 2.2f;
    int per_channel = 2;
};

const MaskGammaCfg& mask_gamma_cfg() {
    static const MaskGammaCfg c = [] {
        MaskGammaCfg r;
        if (const char* v = a380::getenv_cached("A380_TEXT_MG")) {
            r.on = std::strcmp(v, "0") != 0;
            if (!r.on) return r;
            float a, b;
            if (std::sscanf(v, "%f,%f", &a, &b) == 2) r.contrast = a, r.gamma = b;
            if (std::strchr(v, 'p')) r.per_channel = 1;
            if (std::strchr(v, 'a')) r.per_channel = 0;
        }
        return r;
    }();
    return c;
}

float luma_to_linear(float gamma, float v) {
    if (gamma == 0.f) return v <= 0.04045f ? v / 12.92f : std::pow((v + 0.055f) / 1.055f, 2.4f);
    if (gamma == 1.f) return v;
    return std::pow(v, gamma);
}

float luma_from_linear(float gamma, float v) {
    if (gamma == 0.f) return v <= 0.0031308f ? v * 12.92f : 1.055f * std::pow(v, 1.f / 2.4f) - 0.055f;
    if (gamma == 1.f) return v;
    return std::pow(v, 1.f / gamma);
}

void build_mask_gamma_lut(uint8_t* table, int srcI, float contrast, float gamma) {
    const float src = srcI / 255.f;
    const float lin_src = luma_to_linear(gamma, src);
    const float dst = 1.f - src;
    const float lin_dst = luma_to_linear(gamma, dst);
    const float adj = contrast * lin_dst;
    const bool same = std::fabs(src - dst) < 1.f / 256.f;
    for (int i = 0; i < 256; i++) {
        float raw = (float)i / 255.f;
        float srca = raw + (1.f - raw) * adj * raw;
        if (same) {
            table[i] = (uint8_t)std::lround(255.f * srca);
            continue;
        }
        float lin_out = lin_src * srca + (1.f - srca) * lin_dst;
        float out = luma_from_linear(gamma, lin_out);
        float result = (out - dst) / (src - dst);
        table[i] = (uint8_t)std::min(255L, std::max(0L, std::lround(255.f * result)));
    }
}

int skia_luminance(SkColor c) {
    static const int mode = (int)env_f("A380_TEXT_LUM", 0.f);
    int r = SkColorGetR(c), g = SkColorGetG(c), b = SkColorGetB(c);
    if (mode == 0) return (r * 54 + g * 183 + b * 19) >> 8;
    if (mode == 3) return (r * 77 + g * 150 + b * 29) >> 8;
    const float gm = mode == 1 ? 0.f : 2.2f;
    float lin = 0.2126f * luma_to_linear(gm, r / 255.f) + 0.7152f * luma_to_linear(gm, g / 255.f) +
                0.0722f * luma_to_linear(gm, b / 255.f);
    return (int)std::lround(255.f * luma_from_linear(gm, lin));
}

const uint8_t* mask_gamma_table(SkColor c, float contrast, float gamma) {
    thread_local std::vector<uint8_t> cache;
    thread_local float kc = -1.f, kg = -1.f;
    if (cache.empty() || kc != contrast || kg != gamma) {
        cache.assign(8 * 256, 0);
        for (int i = 0; i < 8; i++) {
            int b = i << 5, lum = b;
            for (int k = 3; k < 8; k += 3) lum |= b >> k;
            build_mask_gamma_lut(&cache[(size_t)i * 256], lum, contrast, gamma);
        }
        kc = contrast;
        kg = gamma;
    }
    return &cache[(size_t)(skia_luminance(c) >> 5) * 256];
}

struct MaskEntry {
    sk_sp<SkImage> image;
    int left = 0, top = 0;
    size_t bytes = 0;
    std::vector<sk_sp<SkImage>> more;
};

struct MaskCache {
    std::unordered_map<std::string, MaskEntry> map;
    size_t bytes = 0;
};

MaskCache& mask_cache() {
    thread_local MaskCache c;
    return c;
}

// A cached mask kept as a texture on a GPU canvas, so each later draw skips the raster image lookup.
sk_sp<SkImage> mask_on_gpu(SkCanvas& canvas, sk_sp<SkImage> img) {
#ifdef A380_GPU
    if (GrRecordingContext* rc = canvas.recordingContext(); rc && rc->asDirectContext() && img)
        if (sk_sp<SkImage> tex = SkImages::TextureFromImage(rc->asDirectContext(), img)) return tex;
#endif
    (void)canvas;
    return img;
}

template <class T>
void key_put(std::string& k, const T& v) {
    k.append(reinterpret_cast<const char*>(&v), sizeof(T));
}

struct KernedRun {
    std::vector<SkGlyphID> glyphs;
    std::vector<SkPoint> pos;
    float min_d = 0.f, max_d = 0.f, advance = 0.f;
};

bool make_run(const SkFont& f, const void* text, size_t bytes, SkTextEncoding enc, KernedRun& r) {
    static const bool on = env_f("A380_TEXT_KERN", 1.f) != 0.f;
    if (!on || bytes == 0 || !has_kerning(f.getTypeface())) return false;
    size_t n;
    if (enc == SkTextEncoding::kGlyphID) {
        n = bytes / sizeof(SkGlyphID);
        r.glyphs.resize(n);
        std::memcpy(r.glyphs.data(), text, n * sizeof(SkGlyphID));
    } else {
        r.glyphs.resize(bytes);
        n = f.textToGlyphs(text, bytes, enc, SkSpan<SkGlyphID>(r.glyphs));
        r.glyphs.resize(n);
    }
    if (n < 2) return false;
    std::vector<SkScalar> w(n);
    f.getWidths(SkSpan<const SkGlyphID>(r.glyphs), SkSpan<SkScalar>(w));
    r.pos.resize(n);
    float x = 0.f, d = 0.f;
    bool any = false;
    for (size_t i = 0; i < n; i++) {
        r.pos[i] = {x, 0.f};
        x += w[i];
        if (i + 1 < n) {
            float k = kerning_px(f, r.glyphs[i], r.glyphs[i + 1]);
            if (k != 0.f) any = true;
            x += k;
            d += k;
            r.min_d = std::min(r.min_d, d);
            r.max_d = std::max(r.max_d, d);
        }
    }
    r.advance = x;
    return any;
}

constexpr bool kBgra = kN32_SkColorType == kBGRA_8888_SkColorType;
constexpr int kRShift = kBgra ? 16 : 0, kBShift = kBgra ? 0 : 16;

struct GlyphPlacement {
    std::vector<SkGlyphID> glyphs;
    std::vector<SkPoint> pos;
};

bool glyph_placement(const SkFont& f, const void* text, size_t bytes, SkTextEncoding enc, const KernedRun* kerned,
                     GlyphPlacement& out) {
    if (kerned && !kerned->glyphs.empty()) {
        out.glyphs = kerned->glyphs;
        out.pos = kerned->pos;
        return true;
    }
    if (enc == SkTextEncoding::kGlyphID) {
        size_t n = bytes / sizeof(SkGlyphID);
        out.glyphs.resize(n);
        std::memcpy(out.glyphs.data(), text, n * sizeof(SkGlyphID));
    } else {
        out.glyphs.resize(bytes + 1);
        size_t n = f.textToGlyphs(text, bytes, enc, SkSpan<SkGlyphID>(out.glyphs));
        out.glyphs.resize(n);
    }
    std::vector<SkScalar> w(out.glyphs.size());
    f.getWidths(SkSpan<const SkGlyphID>(out.glyphs), SkSpan<SkScalar>(w));
    out.pos.resize(out.glyphs.size());
    float x = 0.f;
    for (size_t i = 0; i < out.glyphs.size(); i++) {
        out.pos[i] = {x, 0.f};
        x += w[i];
    }
    return true;
}

bool compose_lcd_glyphs(SkBitmap& tmp, const SkFont& f, const GlyphPlacement& gp, float tx, float ty, bool light, int LG) {
    if (!f.isSubpixel() || f.getEdging() != SkFont::Edging::kSubpixelAntiAlias) return false;
    const int W = tmp.width(), H = tmp.height();
    const int bg = light ? 0 : 255, src = light ? 255 : LG;
    tmp.eraseColor(light ? SK_ColorBLACK : SK_ColorWHITE);
    for (size_t i = 0; i < gp.glyphs.size(); i++) {
        float fx = gp.pos[i].fX + tx + 0.125f, fy = gp.pos[i].fY + ty + 0.5f;
        int ix = (int)std::floor(fx), iy = (int)std::floor(fy);
        int sub = ((int)(fx * 65536.f) >> 14) & 3;
        glyphs::Mask m;
        if (!glyphs::get(f, gp.glyphs[i], sub, m)) return false;
        for (int r = 0; r < m.h; r++) {
            int y = iy + m.top + r;
            if (y < 0 || y >= H) continue;
            uint32_t* row = tmp.getAddr32(0, y);
            for (int c = 0; c < m.w; c++) {
                int x = ix + m.left + c;
                if (x < 0 || x >= W) continue;
                const uint8_t* q = m.rgb + ((size_t)r * m.w + c) * 3;
                if (!(q[0] | q[1] | q[2])) continue;
                uint32_t p = row[x];
                int d[3] = {(int)((p >> kRShift) & 255), (int)((p >> 8) & 255), (int)((p >> kBShift) & 255)};
                for (int k = 0; k < 3; k++) {
                    int mk = q[k], sc = mk + (mk >> 4);
                    d[k] = d[k] + (((src - d[k]) * sc) >> 5);
                }
                row[x] = 0xff000000u | ((uint32_t)d[0] << kRShift) | ((uint32_t)d[1] << 8) | ((uint32_t)d[2] << kBShift);
            }
        }
    }
    (void)bg;
    return true;
}

bool lcd_atlas_default() {
    static const bool on = [] {
        const char* v = a380::getenv_cached("A380_GLYPH_LCD");
        if (v) return std::strcmp(v, "0") != 0;
#ifdef _WIN32
        return false;
#else
        return true;
#endif
    }();
    return on;
}

const uint8_t* lcd_lut(int bucket, float contrast, float gamma) {
    thread_local std::vector<uint8_t> cache;
    thread_local int kc = -1, kg = -1;
    const int ic = (int)(contrast * 255.f + 0.5f), ig = (int)(gamma * 64.f);
    if (cache.empty() || kc != ic || kg != ig) {
        cache.assign(8 * 256, 0);
        for (int i = 0; i < 8; i++) {
            int lum = i << 5;
            for (int k = 3; k < 8; k += 3) lum |= (i << 5) >> k;
            build_mask_gamma_lut(&cache[(size_t)i * 256], lum, (float)ic / 255.f, (float)ig / 64.f);
        }
        kc = ic;
        kg = ig;
    }
    return &cache[(size_t)bucket * 256];
}

bool compose_lcd_coverage(std::vector<SkBitmap>& layers, int W, int H, const SkFont& f, const GlyphPlacement& gp, float tx, float ty,
                          const uint8_t* const lut[3], bool gpu) {
    if (!f.isSubpixel() || f.getEdging() != SkFont::Edging::kSubpixelAntiAlias) return false;
    std::vector<int8_t> top((size_t)W * H, -1);
    auto pack = [&](const uint8_t* q, int k, int pre_idx) {
        int pre = lut[k][q[pre_idx]];
        if (gpu) return k == 1 ? (pre & 0xfc) | (pre >> 6) : (pre & 0xf8) | (pre >> 5);
        int v = pre >> 3;
        return v + (v >> 4);
    };
    for (size_t i = 0; i < gp.glyphs.size(); i++) {
        float fx = gp.pos[i].fX + tx + 0.125f, fy = gp.pos[i].fY + ty + 0.5f;
        int ix = (int)std::floor(fx), iy = (int)std::floor(fy);
        int sub = ((int)(fx * 65536.f) >> 14) & 3;
        glyphs::Mask m;
        if (!glyphs::get(f, gp.glyphs[i], sub, m)) return false;
        if (m.w * m.h && !m.raw) return false;
        int layer = 0;
        for (int r = 0; r < m.h; r++) {
            int y = iy + m.top + r;
            if (y < 0 || y >= H) continue;
            for (int c = 0; c < m.w; c++) {
                int x = ix + m.left + c;
                if (x < 0 || x >= W) continue;
                const uint8_t* q = m.raw + ((size_t)r * m.w + c) * 3;
                if (!(pack(q, 0, 0) | pack(q, 1, 1) | pack(q, 2, 2))) continue;
                layer = std::max(layer, (int)top[(size_t)y * W + x] + 1);
            }
        }
        while ((int)layers.size() <= layer) {
            layers.emplace_back();
            layers.back().allocPixels(SkImageInfo::MakeN32Premul(W, H));
            layers.back().eraseColor(SK_ColorBLACK);
        }
        SkBitmap& tmp = layers[(size_t)layer];
        for (int r = 0; r < m.h; r++) {
            int y = iy + m.top + r;
            if (y < 0 || y >= H) continue;
            uint32_t* row = tmp.getAddr32(0, y);
            for (int c = 0; c < m.w; c++) {
                int x = ix + m.left + c;
                if (x < 0 || x >= W) continue;
                const uint8_t* q = m.raw + ((size_t)r * m.w + c) * 3;
                int v0 = pack(q, 0, 0), v1 = pack(q, 1, 1), v2 = pack(q, 2, 2);
                if (!(v0 | v1 | v2)) continue;
                row[x] = 0xff000000u | ((uint32_t)v0 << 16) | ((uint32_t)v1 << 8) | (uint32_t)v2;
                top[(size_t)y * W + x] = (int8_t)layer;
            }
        }
    }
    return true;
}

sk_sp<SkBlender> lcd_blender(SkColor c, bool gpu) {
    static sk_sp<SkRuntimeEffect> effect = [] {
        auto r = SkRuntimeEffect::MakeForBlender(SkString(
            "uniform float4 color; uniform float gpu;"
            "float hs(float x) {"
            " if (x < 1e-6) return 0.0;"
            " float u = exp2(floor(log2(x)) - 10.0);"
            " return floor(x / u + 0.5) * u; }"
            "float3 hr(float3 x) { return float3(hs(x.x), hs(x.y), hs(x.z)); }"
            "half4 main(half4 src, half4 dst) {"
            " float3 q = floor(float3(src.rgb) * 255.0 + 0.5);"
            " float3 d = floor(float3(dst.rgb) * 255.0 + 0.5);"
            " float3 k = floor(color.rgb * 255.0 + 0.5);"
            " float3 r;"
            " if (gpu > 0.5) {"
            "  float3 cov = hr(q / 255.0);"
            "  float3 prim = hr(hr(k / 255.0 * color.a) * cov);"
            "  float3 sec = hr(hr(float3(color.a)) * cov);"
            "  r = floor((prim + d / 255.0 * (1.0 - sec)) * 255.0 + 0.5);"
            " } else {"
            "  float3 sc = floor(q * (floor(color.a * 255.0 + 0.5) + 1.0) / 256.0);"
            "  r = d + floor((k - d) * sc / 32.0);"
            " }"
            " float3 cv = gpu > 0.5 ? q / 255.0 : q / 32.0;"
            " float a = max(float(dst.a), color.a * max(max(cv.r, cv.g), cv.b));"
            " return half4(half3(r / 255.0), half(a)); }"));
        return r.effect;
    }();
    if (!effect) return nullptr;
    SkRuntimeBlendBuilder b(effect);
    b.uniform("color") = SkV4{SkColorGetR(c) / 255.f, SkColorGetG(c) / 255.f, SkColorGetB(c) / 255.f, SkColorGetA(c) / 255.f};
    b.uniform("gpu") = gpu ? 1.f : 0.f;
    return b.makeBlender();
}

}  // namespace

namespace glyphs {
const uint8_t* lcd_preblend(int bucket, float contrast, float gamma) { return lcd_lut(bucket, contrast, gamma); }
}  // namespace glyphs

float kerning_px(const SkFont& font, SkGlyphID left, SkGlyphID right) {
    return kerning_em(font.getTypeface(), left, right) * font.getSize() * font.getScaleX();
}

float text_advance(const SkFont& font, const void* text, size_t bytes, SkTextEncoding encoding) {
    KernedRun run;
    if (make_run(font, text, bytes, encoding, run)) return run.advance;
    return font.measureText(text, bytes, encoding);
}

float svg_central_offset(const SkFont& font) {
    SkFontMetrics m;
    font.getMetrics(&m);
    float fallback = -(m.fAscent + m.fDescent) * 0.5f;
    SkTypeface* tf = font.getTypeface();
    if (!tf) return fallback;
    uint8_t h[8];
    if (tf->getTableData(SkSetFourByteTag('h', 'h', 'e', 'a'), 0, 8, h) != 8) return fallback;
    int upem = tf->getUnitsPerEm();
    if (upem <= 0) return fallback;
    int asc = (int16_t)((h[4] << 8) | h[5]), desc = (int16_t)((h[6] << 8) | h[7]);
    return (asc + desc) * 0.5f / (float)upem * font.getSize();
}

float layout_unit(float v) { return std::trunc(v * 64.f) / 64.f; }

float chromium_snap(float css_y, float dpr) { return std::floor(css_y * dpr + 0.5f) / dpr; }

HheaMetrics hhea_metrics(const SkFont& font) {
    HheaMetrics r;
    SkTypeface* tf = font.getTypeface();
    uint8_t h[10];
    int upem = tf ? tf->getUnitsPerEm() : 0;
    if (upem > 0 && tf->getTableData(SkSetFourByteTag('h', 'h', 'e', 'a'), 0, 10, h) == 10) {
        r.ascent = (int16_t)((h[4] << 8) | h[5]) / (float)upem;
        r.descent = -(int16_t)((h[6] << 8) | h[7]) / (float)upem;
        r.line_gap = (int16_t)((h[8] << 8) | h[9]) / (float)upem;
        r.valid = true;
        return r;
    }
    SkFontMetrics m;
    font.getMetrics(&m);
    float sz = font.getSize() > 0.f ? font.getSize() : 1.f;
    r.ascent = -m.fAscent / sz;
    r.descent = m.fDescent / sz;
    r.line_gap = m.fLeading / sz;
    return r;
}

float chromium_baseline(const SkFont& font, float line_top, float line_height, float dpr) {
    HheaMetrics m = hhea_metrics(font);
    static const bool skia_metrics = env_f("A380_BASELINE_SKIA", 0.f) != 0.f;
    if (skia_metrics) {
        SkFontMetrics sm;
        font.getMetrics(&sm);
        m.ascent = -sm.fAscent / font.getSize();
        m.descent = sm.fDescent / font.getSize();
        m.line_gap = sm.fLeading / font.getSize();
    }
    float sz = font.getSize() * dpr;
    float asc = std::round(m.ascent * sz), desc = std::round(m.descent * sz), gap = std::round(m.line_gap * sz);
    float top = layout_unit(line_top * dpr), lh = line_height > 0.f ? layout_unit(line_height * dpr) : asc + desc + gap;
    float half = layout_unit((lh - (asc + desc)) * 0.5f);
    return std::floor(top + half + asc + 0.5f) / dpr;
}

static bool draw_lcd_from_glyphs(SkCanvas& canvas, const void* text, size_t bytes, SkTextEncoding encoding, float x, float y,
                                 const SkFont& font, const SkPaint& paint, const SkMatrix& m) {
    float sx = m.getScaleX();
    float dx = x * sx + m.getTranslateX(), dy = y * m.getScaleY() + m.getTranslateY();
    SkFont f = font;
    f.setSize(font.getSize() * sx);
    std::string key;
    key.reserve(bytes + 64);
    key.push_back('L');
    {
        std::string tk = typeface_key(f.getTypeface());
        key.append(tk);
        key.push_back(0);
    }
    key_put(key, f.getSize());
    key_put(key, f.getScaleX());
    key_put(key, f.getSkewX());
    key_put(key, (uint8_t)((f.isEmbolden() ? 1 : 0) | (f.isLinearMetrics() ? 2 : 0) | (f.isBaselineSnap() ? 4 : 0) |
                           (f.isForceAutoHinting() ? 8 : 0) | (f.isEmbeddedBitmaps() ? 16 : 0)));
    key_put(key, (uint8_t)f.getHinting());
    key_put(key, (uint8_t)encoding);
    key_put(key, (float)(dx - std::floor(dx)));
    key_put(key, (float)(dy - std::floor(dy)));
    const SkSurfaceProps& props = canvas.getBaseProps();
    const bool gpu = canvas.recordingContext() != nullptr;
    const SkColor pcol = paint.getColor();
    const int lb[3] = {(int)(SkColorGetR(pcol) >> 5), (int)(SkColorGetG(pcol) >> 5), (int)(SkColorGetB(pcol) >> 5)};
    key_put(key, (uint8_t)(lb[0] | (lb[1] << 3) | (gpu ? 64 : 0)));
    key_put(key, (uint8_t)lb[2]);
    key_put(key, props.textContrast());
    key_put(key, props.textGamma());
    key.append(static_cast<const char*>(text), bytes);
    double ox = std::floor(dx), oy = std::floor(dy);
    MaskCache& cache = mask_cache();
    const MaskEntry* ent = nullptr;
    if (auto hit = cache.map.find(key); hit != cache.map.end()) {
        ent = &hit->second;
    } else {
        SkRect b;
        f.measureText(text, bytes, encoding, &b);
        KernedRun run;
        bool kerned = make_run(f, text, bytes, encoding, run);
        if (kerned) {
            b.fLeft += run.min_d;
            b.fRight += run.max_d;
        }
        const int pad = 4;
        int L = (int)std::floor(b.fLeft) - pad;
        int T = (int)std::floor(b.fTop) - pad;
        int W = (int)std::ceil(b.fRight) + pad - L, H = (int)std::ceil(b.fBottom) + pad - T;
        if (W <= 0 || H <= 0 || W > 4096 || H > 1024) return false;
        GlyphPlacement gp;
        glyph_placement(f, text, bytes, encoding, kerned ? &run : nullptr, gp);
        const uint8_t* luts[3] = {lcd_lut(lb[0], props.textContrast(), props.textGamma()),
                                  lcd_lut(lb[1], props.textContrast(), props.textGamma()),
                                  lcd_lut(lb[2], props.textContrast(), props.textGamma())};
        std::vector<SkBitmap> layers;
        if (!compose_lcd_coverage(layers, W, H, f, gp, (float)(dx - ox) - L, (float)(dy - oy) - T, luts, gpu)) return false;
        if (layers.empty()) {
            layers.emplace_back();
            layers.back().allocPixels(SkImageInfo::MakeN32Premul(W, H));
            layers.back().eraseColor(SK_ColorBLACK);
        }
        static const size_t kMaskBudget = 48u << 20;
        if (cache.bytes > kMaskBudget) {
            cache.map.clear();
            cache.bytes = 0;
        }
        MaskEntry e;
        e.left = L;
        e.top = T;
        for (size_t i = 0; i < layers.size(); i++) {
            layers[i].setImmutable();
            (i == 0 ? e.image : e.more.emplace_back()) = layers[i].asImage();
        }
        e.bytes = (size_t)W * (size_t)H * 4 * layers.size() + key.size() + 64;
        cache.bytes += e.bytes;
        ent = &cache.map.emplace(std::move(key), std::move(e)).first->second;
    }
    if (glyphs::baking()) return false;
    sk_sp<SkBlender> blender = lcd_blender(paint.getColor(), gpu);
    if (!blender) {
        return false;
    }
    SkPaint ip;
    ip.setBlender(blender);
    canvas.save();
    canvas.resetMatrix();
    canvas.drawImage(ent->image, (float)(ox + ent->left), (float)(oy + ent->top), SkSamplingOptions(), &ip);
    for (const auto& im : ent->more) canvas.drawImage(im, (float)(ox + ent->left), (float)(oy + ent->top), SkSamplingOptions(), &ip);
    canvas.restore();
    return true;
}

void set_transparent_target(bool on) { g_transparent_target = on; }

bool transparent_target() { return g_transparent_target; }

// One blob per string and font, kept so Ganesh reuses the text it already prepared.
static sk_sp<SkTextBlob> cached_blob(const SkFont& f, const void* text, size_t bytes, SkTextEncoding encoding) {
    std::string key;
    key.reserve(bytes + 40);
    key_put(key, f.getTypeface() ? f.getTypeface()->uniqueID() : 0u);
    key_put(key, f.getSize());
    key_put(key, f.getScaleX());
    key_put(key, f.getSkewX());
    key_put(key, (uint8_t)f.getEdging());
    key_put(key, (uint8_t)f.getHinting());
    key_put(key, (uint8_t)((f.isEmbolden() ? 1 : 0) | (f.isLinearMetrics() ? 2 : 0) | (f.isBaselineSnap() ? 4 : 0) |
                           (f.isForceAutoHinting() ? 8 : 0) | (f.isEmbeddedBitmaps() ? 16 : 0) | (f.isSubpixel() ? 32 : 0)));
    key_put(key, (uint8_t)encoding);
    key.append(static_cast<const char*>(text), bytes);
    thread_local std::unordered_map<std::string, sk_sp<SkTextBlob>> cache;
    if (auto hit = cache.find(key); hit != cache.end()) return hit->second;
    sk_sp<SkTextBlob> blob;
    KernedRun run;
    if (make_run(f, text, bytes, encoding, run)) {
        SkTextBlobBuilder b;
        const auto& rb = b.allocRunPos(f, (int)run.glyphs.size());
        std::memcpy(rb.glyphs, run.glyphs.data(), run.glyphs.size() * sizeof(SkGlyphID));
        std::memcpy(rb.pos, run.pos.data(), run.pos.size() * sizeof(SkPoint));
        blob = b.make();
    } else {
        blob = SkTextBlob::MakeFromText(text, bytes, f, encoding);
    }
    if (cache.size() > 8192) cache.clear();
    cache.emplace(std::move(key), blob);
    return blob;
}

// True when draw_simple_text would hand this text straight to Skia (its `direct` path) for any non-empty string.
static bool text_goes_direct(SkCanvas& canvas, const SkFont& font, const SkPaint& paint) {
    SkMatrix m = canvas.getLocalToDeviceAs3x3();
    float sx = m.getScaleX(), sy = m.getScaleY(), kx = m.getSkewX(), ky = m.getSkewY();
    bool rotated = kx != 0 || ky != 0;
    float scale = sy;
    if (rotated) {
        static const bool rot_on = env_f("A380_TEXT_ROT", 1.f) != 0.f;
        float s1 = std::hypot(sx, ky), s2 = std::hypot(kx, sy), det = sx * sy - kx * ky;
        if (!rot_on || m.hasPerspective() || std::fabs(s1 - s2) > 1e-3f * s1 || det <= 0.f || s1 <= 0.f) return true;
        scale = s1;
    }
    static const float big_px = env_f("A380_TEXT_BIG", 0.f);
    if (big_px > 0.f && !direct_mode() && font.getSize() * scale > big_px && font.getEdging() != SkFont::Edging::kSubpixelAntiAlias) return true;
    bool lcd_font = font.getEdging() == SkFont::Edging::kSubpixelAntiAlias;
    if (lcd_font && !direct_mode() && !rotated && !m.hasPerspective() && scale > 0 && std::fabs(sx - sy) <= 1e-4f && !paint.getShader() &&
        font.getSize() * scale <= 48.f && font.isSubpixel() && mask_gamma_cfg().on && canvas.getBaseProps().pixelGeometry() == kRGB_H_SkPixelGeometry &&
        (glyphs::baking() || lcd_atlas_default()) && glyphs::enabled())
        return false;
    return direct_mode() || (lcd_font && !g_transparent_target) || m.hasPerspective() || scale <= 0 || (!rotated && std::fabs(sx - sy) > 1e-4f) ||
           paint.getShader() || font.getSize() * scale > 400.f;
}

void draw_glyph_run(SkCanvas& canvas, const SkGlyphID* glyphs, const float* xs, size_t n, float y, const SkFont& font, const SkPaint& paint) {
    if (n == 0) return;
    if (n == 1 || !text_goes_direct(canvas, font, paint)) {
        for (size_t i = 0; i < n; i++) draw_simple_text(canvas, &glyphs[i], sizeof(SkGlyphID), SkTextEncoding::kGlyphID, xs[i], y, font, paint);
        return;
    }
    SkFont df = font;
    if (g_transparent_target && df.getEdging() == SkFont::Edging::kSubpixelAntiAlias) df.setEdging(SkFont::Edging::kAntiAlias);
    std::string key;
    key.reserve(48 + n * 6);
    key_put(key, df.getTypeface() ? df.getTypeface()->uniqueID() : 0u);
    key_put(key, df.getSize());
    key_put(key, df.getScaleX());
    key_put(key, df.getSkewX());
    key_put(key, (uint8_t)df.getEdging());
    key_put(key, (uint8_t)df.getHinting());
    key_put(key, (uint8_t)((df.isEmbolden() ? 1 : 0) | (df.isLinearMetrics() ? 2 : 0) | (df.isBaselineSnap() ? 4 : 0) |
                           (df.isForceAutoHinting() ? 8 : 0) | (df.isEmbeddedBitmaps() ? 16 : 0) | (df.isSubpixel() ? 32 : 0)));
    for (size_t i = 0; i < n; i++) {
        key_put(key, glyphs[i]);
        key_put(key, xs[i] - xs[0]);
    }
    thread_local std::unordered_map<std::string, sk_sp<SkTextBlob>> cache;
    sk_sp<SkTextBlob> blob;
    if (auto hit = cache.find(key); hit != cache.end()) {
        blob = hit->second;
    } else {
        SkTextBlobBuilder b;
        const auto& rb = b.allocRunPosH(df, (int)n, 0.f);
        std::memcpy(rb.glyphs, glyphs, n * sizeof(SkGlyphID));
        for (size_t i = 0; i < n; i++) rb.pos[i] = xs[i] - xs[0];
        blob = b.make();
        if (cache.size() > 8192) cache.clear();
        cache.emplace(std::move(key), blob);
    }
    if (blob) canvas.drawTextBlob(blob, xs[0], y, paint);
}

void draw_simple_text(SkCanvas& canvas, const void* text, size_t bytes, SkTextEncoding encoding, float x, float y,
                      const SkFont& font, const SkPaint& paint) {
    SkMatrix m = canvas.getLocalToDeviceAs3x3();
    const bool clear_dst = g_transparent_target;
    auto direct = [&] {
        SkFont df = font;
        if (clear_dst && df.getEdging() == SkFont::Edging::kSubpixelAntiAlias) df.setEdging(SkFont::Edging::kAntiAlias);
        if (sk_sp<SkTextBlob> blob = cached_blob(df, text, bytes, encoding)) canvas.drawTextBlob(blob, x, y, paint);
    };
    float sx = m.getScaleX(), sy = m.getScaleY();
    static const bool rot_on = env_f("A380_TEXT_ROT", 1.f) != 0.f;
    float kx = m.getSkewX(), ky = m.getSkewY();
    bool rotated = kx != 0 || ky != 0;
    float scale = sy;
    SkMatrix rot;
    if (rotated) {
        float s1 = std::hypot(sx, ky), s2 = std::hypot(kx, sy);
        float det = sx * sy - kx * ky;
        if (!rot_on || m.hasPerspective() || std::fabs(s1 - s2) > 1e-3f * s1 || det <= 0.f || s1 <= 0.f) {
            direct();
            return;
        }
        scale = s1;
        rot = SkMatrix::MakeAll(sx / s1, kx / s1, 0, ky / s1, sy / s1, 0, 0, 0, 1);
    }
    static const float big_px = env_f("A380_TEXT_BIG", 0.f);
    if (big_px > 0.f && !direct_mode() && font.getSize() * scale > big_px && font.getEdging() != SkFont::Edging::kSubpixelAntiAlias) {
        direct();
        return;
    }
    bool lcd_font = font.getEdging() == SkFont::Edging::kSubpixelAntiAlias;
    if (lcd_font && !direct_mode() && bytes && !rotated && !m.hasPerspective() && scale > 0 && std::fabs(sx - sy) <= 1e-4f &&
        !paint.getShader() && font.getSize() * scale <= 48.f && font.isSubpixel() && mask_gamma_cfg().on &&
        canvas.getBaseProps().pixelGeometry() == kRGB_H_SkPixelGeometry &&
        (glyphs::baking() || lcd_atlas_default()) && glyphs::enabled()) {
        if (draw_lcd_from_glyphs(canvas, text, bytes, encoding, x, y, font, paint, m)) return;
    }
    if (direct_mode() || (lcd_font && !clear_dst) || bytes == 0 || m.hasPerspective() || scale <= 0 ||
        (!rotated && std::fabs(sx - sy) > 1e-4f) || paint.getShader() || font.getSize() * scale > 400.f) {
        direct();
        return;
    }
    float dx, dy;
    if (rotated) {
        SkPoint pt = m.mapPoint({x, y});
        dx = pt.fX;
        dy = pt.fY;
    } else {
        dx = x * sx + m.getTranslateX();
        dy = y * sy + m.getTranslateY();
    }
    SkFont f = font;
    f.setSize(font.getSize() * scale);
    f.setEdging(SkFont::Edging::kSubpixelAntiAlias);
    f.setSubpixel(true);
    SkColor pc = paint.getColor();
    float lum = (0.2126f * SkColorGetR(pc) + 0.7152f * SkColorGetG(pc) + 0.0722f * SkColorGetB(pc));
    int LG = lum >= 128.f ? 255 : (int)std::lround(lum);
    bool light = LG >= 128;
    const MaskGammaCfg& mg = mask_gamma_cfg();
    float gamma = 0.f;
    if (!mg.on) {
        float lum0 = lum / 255.f;
        gamma = text_gamma() + (lum0 >= 0.5f ? std::min(0.4f, std::max(-0.2f, (lum0 - 0.79f) * 1.9f)) : 0.f);
        int r = SkColorGetR(pc), g = SkColorGetG(pc), b = SkColorGetB(pc);
        const char* key = nullptr;
        float fixed = 0.f;
        if (g > 200 && r < 100 && b < 100) key = "A380_GAMMA_GREEN", fixed = 1.7f;
        else if (r > 200 && b > 200 && g < 200 && g > 100) key = "A380_GAMMA_MAGENTA", fixed = 1.7f;
        else if (r > 200 && g > 100 && g < 200 && b < 100) key = "A380_GAMMA_AMBER";
        else if (r > 230 && g > 230 && b > 230) key = "A380_GAMMA_WHITE";
        else if (r < 80 && g > 200 && b > 200) key = "A380_GAMMA_CYAN";
        if (key) {
            if (fixed > 0.f) gamma = fixed;
            if (const char* v = a380::getenv_cached(key)) gamma = (float)std::atof(v);
        }
    }
    static const float subpixel = env_f("A380_TEXT_SUBPIXEL", 4.f);
    if (subpixel > 0.f) {
        dx = std::round(dx * subpixel) / subpixel;
        dy = std::round(dy * subpixel) / subpixel;
    }
    float frac_x = (float)(dx - std::floor(dx)), frac_y = (float)(dy - std::floor(dy));
    std::string mask_key;
    mask_key.reserve(bytes + 96);
    key_put(mask_key, f.getTypeface() ? f.getTypeface()->uniqueID() : 0u);
    key_put(mask_key, f.getSize());
    key_put(mask_key, f.getScaleX());
    key_put(mask_key, f.getSkewX());
    key_put(mask_key, (uint8_t)((f.isEmbolden() ? 1 : 0) | (f.isLinearMetrics() ? 2 : 0) | (f.isBaselineSnap() ? 4 : 0) |
                                (f.isForceAutoHinting() ? 8 : 0) | (f.isEmbeddedBitmaps() ? 16 : 0)));
    key_put(mask_key, (uint8_t)f.getHinting());
    key_put(mask_key, (uint8_t)encoding);
    key_put(mask_key, frac_x);
    key_put(mask_key, frac_y);
    key_put(mask_key, (uint8_t)rotated);
    if (rotated) {
        key_put(mask_key, rot.get(SkMatrix::kMScaleX));
        key_put(mask_key, rot.get(SkMatrix::kMSkewX));
        key_put(mask_key, rot.get(SkMatrix::kMSkewY));
        key_put(mask_key, rot.get(SkMatrix::kMScaleY));
    }
    key_put(mask_key, LG);
    key_put(mask_key, (uint8_t)light);
    key_put(mask_key, (uint8_t)(skia_luminance(pc) >> 5));
    key_put(mask_key, gamma);
    mask_key.append(static_cast<const char*>(text), bytes);
    MaskCache& cache = mask_cache();
    double ox = std::floor(dx), oy = std::floor(dy);
    if (auto hit = cache.map.find(mask_key); hit != cache.map.end()) {
        const MaskEntry& e = hit->second;
        canvas.save();
        canvas.resetMatrix();
        canvas.drawImage(e.image, (float)(ox + e.left), (float)(oy + e.top), SkSamplingOptions(), &paint);
        canvas.restore();
        return;
    }
    SkRect b;
    f.measureText(text, bytes, encoding, &b);
    KernedRun run;
    bool kerned = make_run(f, text, bytes, encoding, run);
    if (kerned) {
        b.fLeft += run.min_d;
        b.fRight += run.max_d;
    }
    if (rotated) b = rot.mapRect(b);
    const int pad = 4;
    int L = (int)std::floor(b.fLeft) - pad, T = (int)std::floor(b.fTop) - pad;
    int W = (int)std::ceil(b.fRight) + pad - L, H = (int)std::ceil(b.fBottom) + pad - T;
    if (W <= 0 || H <= 0 || W > 4096 || H > 1024) {
        direct();
        return;
    }
    SkMatrix tm = SkMatrix::Translate((float)(dx - ox) - L, (float)(dy - oy) - T);
    if (rotated) tm.preConcat(rot);

    static const float ss_px = env_f("A380_TEXT_SS", 0.f);
    const int ss = ss_px > 0.f && f.getSize() > ss_px && W * 4 <= 8192 && H * 4 <= 4096 ? 4 : 1;
    static const float path_px = env_f("A380_TEXT_PATH", 0.f);
    std::vector<float> pathsub;
    bool use_path = false;
    if (path_px > 0.f && f.getSize() > path_px && !(kerned && false)) {
        std::vector<SkGlyphID> gl(bytes + 1);
        size_t ng = f.textToGlyphs(text, bytes, encoding, SkSpan<SkGlyphID>(gl));
        gl.resize(ng);
        std::vector<SkScalar> wd(ng);
        f.getWidths(SkSpan<const SkGlyphID>(gl), SkSpan<SkScalar>(wd));
        SkPathBuilder pb;
        float cx = 0.f;
        SkMatrix m3 = tm;
        if (!rotated) m3 = SkMatrix::Translate(-(float)ox - L, -(float)oy - T);
        for (size_t i = 0; i < ng; i++) {
            if (auto gp = f.getPath(gl[i])) {
                if (!rotated) {
                    float qx = std::floor((float)(dx + cx) * 4.f + 0.5f) / 4.f, qy = std::floor((float)dy + 0.5f);
                    pb.addPath(*gp, qx, qy);
                } else {
                    pb.addPath(*gp, cx, 0.f);
                }
            }
            cx += wd[i] + (i + 1 < ng ? kerning_px(f, gl[i], gl[i + 1]) : 0.f);
        }
        m3.postScale(3.f, 1.f);
        use_path = chromium_path_coverage(pb.detach(), m3, W * 3, H, pathsub);
        static const float ppow = env_f("A380_TEXT_PATHPOW", 0.6f);
        if (use_path && ppow != 1.f)
            for (float& v : pathsub) v = std::pow(v, ppow);
    }
    SkImageInfo info = SkImageInfo::MakeN32Premul(W, H);
    SkBitmap tmp;
    tmp.allocPixels(info);
    SkSurfaceProps props(0, kRGB_H_SkPixelGeometry, mg.on ? 0.f : 0.5f, mg.on ? 1.f : gamma);
    tmp.eraseColor(light ? SK_ColorBLACK : SK_ColorWHITE);
    auto scaler_render = [&](SkBitmap& dstbm) {
        SkBitmap big;
        if (ss > 1) {
            big.allocPixels(SkImageInfo::MakeN32Premul(W * ss, H * ss));
            big.eraseColor(light ? SK_ColorBLACK : SK_ColorWHITE);
        }
        SkCanvas tc(ss > 1 ? big : dstbm, props);
        SkPaint wp;
        wp.setAntiAlias(true);
        wp.setColor(SkColorSetRGB(LG, LG, LG));
        SkMatrix tmx = tm;
        if (ss > 1) tmx.postScale((float)ss, (float)ss);
        tc.setMatrix(tmx);
        if (kerned)
            tc.drawGlyphs(SkSpan<const SkGlyphID>(run.glyphs), SkSpan<const SkPoint>(run.pos), {0.f, 0.f}, f, wp);
        else
            tc.drawSimpleText(text, bytes, encoding, 0.f, 0.f, f, wp);
        if (ss > 1) {
            for (int j = 0; j < H; j++) {
                uint32_t* d = dstbm.getAddr32(0, j);
                for (int i = 0; i < W; i++) {
                    int sr = 0, sg = 0, sb = 0;
                    for (int v = 0; v < ss; v++) {
                        const uint32_t* q = big.getAddr32(i * ss, j * ss + v);
                        for (int u = 0; u < ss; u++) {
                            sr += (q[u] >> 16) & 255;
                            sg += (q[u] >> 8) & 255;
                            sb += q[u] & 255;
                        }
                    }
                    const int n = ss * ss;
                    d[i] = 0xff000000u | ((uint32_t)((sr + n / 2) / n) << 16) | ((uint32_t)((sg + n / 2) / n) << 8) | (uint32_t)((sb + n / 2) / n);
                }
            }
        }
    };
    bool composed = false;
    if (!use_path && ss == 1 && !rotated && mask_gamma_cfg().on && glyphs::enabled()) {
        GlyphPlacement gp;
        glyph_placement(f, text, bytes, encoding, kerned ? &run : nullptr, gp);
        composed = compose_lcd_glyphs(tmp, f, gp, (float)(dx - ox) - L, (float)(dy - oy) - T, light, LG);
        if (composed && glyphs::verifying()) {
            SkBitmap ref;
            ref.allocPixels(info);
            ref.eraseColor(light ? SK_ColorBLACK : SK_ColorWHITE);
            scaler_render(ref);
            int bad = 0;
            for (int j = 0; j < H; j++) {
                const uint32_t* a = tmp.getAddr32(0, j);
                const uint32_t* c = ref.getAddr32(0, j);
                for (int i = 0; i < W; i++) bad += a[i] != c[i];
            }
            glyphs::note_verify(bad == 0, bad);
        }
    }
    if (!use_path && !composed) {
        tmp.eraseColor(light ? SK_ColorBLACK : SK_ColorWHITE);
        scaler_render(tmp);
    }
    SkBitmap a8;
    a8.allocPixels(SkImageInfo::MakeA8(W, H));
    {
        static const float k2 = env_f("A380_TEXT_K2", mask_gamma_cfg().on ? 0.f : 0.2f), k3 = env_f("A380_TEXT_K3", 0.f);
        const float norm = 3.f + 2.f * k2 + 2.f * k3;
        std::vector<float> sub((size_t)W * 3 + 12);
        const bool mg_pre = mask_gamma_cfg().on && (mask_gamma_cfg().per_channel == 1 || (mask_gamma_cfg().per_channel == 2 && f.getSize() > 48.f));
        const uint8_t* pre_lut = mg_pre ? mask_gamma_table(pc, mask_gamma_cfg().contrast, mask_gamma_cfg().gamma) : nullptr;
        for (int j = 0; j < H; j++) {
            const uint32_t* s = tmp.getAddr32(0, j);
            uint8_t* d = a8.getAddr8(0, j);
            std::fill(sub.begin(), sub.end(), 0.f);
            if (use_path) {
                for (int i = 0; i < W * 3; i++) sub[(size_t)i + 6] = pathsub[(size_t)j * W * 3 + i];
            }
            for (int i = 0; i < (use_path ? 0 : W); i++) {
                uint32_t p = s[i];
                int ch[3] = {(int)((p >> kRShift) & 255), (int)((p >> 8) & 255), (int)((p >> kBShift) & 255)};
                for (int q = 0; q < 3; q++) {
                    float cov = light ? ch[q] / (float)LG : (255.f - ch[q]) / (float)(255 - LG);
                    sub[(size_t)i * 3 + q + 6] = std::min(1.f, std::max(0.f, cov));
                }
            }
            if (mg_pre) {
                for (float& v : sub) v = pre_lut[(int)std::lround(std::min(1.f, std::max(0.f, v)) * 255.f)] / 255.f;
            }
            for (int i = 0; i < W; i++) {
                const float* c = &sub[(size_t)i * 3 + 7];
                float v = c[-1] + c[0] + c[1] + k2 * (c[-2] + c[2]) + k3 * (c[-3] + c[3]);
                d[i] = (uint8_t)std::lround(std::min(1.f, std::max(0.f, v / norm)) * 255.f);
            }
        }
        if (mask_gamma_cfg().on && !mg_pre) {
            const uint8_t* lut = mask_gamma_table(pc, mask_gamma_cfg().contrast, mask_gamma_cfg().gamma);
            for (int j = 0; j < H; j++) {
                uint8_t* d = a8.getAddr8(0, j);
                for (int i = 0; i < W; i++) d[i] = lut[d[i]];
            }
        }
    }
    {
        static const float hb = env_f("A380_TEXT_HBLUR", 0.f), pw = env_f("A380_TEXT_POW", 1.f);
        if (hb > 0.f || pw != 1.f) {
            std::vector<float> row(W);
            for (int j = 0; j < H; j++) {
                uint8_t* d = a8.getAddr8(0, j);
                for (int i = 0; i < W; i++) {
                    float c0 = d[i] / 255.f, l = i > 0 ? d[i - 1] / 255.f : 0.f, r = i + 1 < W ? d[i + 1] / 255.f : 0.f;
                    float c = (1.f - 2.f * hb) * c0 + hb * (l + r);
                    row[i] = std::pow(std::min(1.f, std::max(0.f, c)), pw);
                }
                for (int i = 0; i < W; i++) d[i] = (uint8_t)std::lround(row[i] * 255.f);
            }
        }
    }
    a8.setImmutable();
    sk_sp<SkImage> img = mask_on_gpu(canvas, a8.asImage());
    static const size_t kMaskBudget = 48u << 20;
    if (cache.bytes > kMaskBudget) {
        cache.map.clear();
        cache.bytes = 0;
    }
    size_t cost = (size_t)W * (size_t)H + mask_key.size() + 64;
    cache.map.emplace(std::move(mask_key), MaskEntry{img, L, T, cost});
    cache.bytes += cost;
    canvas.save();
    canvas.resetMatrix();
    canvas.drawImage(img, (float)(ox + L), (float)(oy + T), SkSamplingOptions(), &paint);
    canvas.restore();
}


float draw_stroked_text(SkCanvas& canvas, std::string_view text, float x, float y, const SkFont& font, SkColor fill,
                        SkColor stroke, float stroke_width) {
    if (text.empty()) return 0.f;
    float adv = text_advance(font, text.data(), text.size(), SkTextEncoding::kUTF8);
    SkPaint fp;
    fp.setAntiAlias(true);
    fp.setColor(fill);
    SkPaint sp;
    sp.setAntiAlias(true);
    sp.setColor(stroke);
    sp.setStyle(SkPaint::kStroke_Style);
    sp.setStrokeWidth(stroke_width);
    static const int env_mode = (int)env_f("A380_STROKE_MODE", -2.f);
    const int mode = env_mode != -2 ? env_mode : (fill == stroke ? 1 : 0);
    SkMatrix m = canvas.getLocalToDeviceAs3x3();
    float sx = m.getScaleX(), sy = m.getScaleY();
    bool plain = m.getSkewX() != 0 || m.getSkewY() != 0 || m.hasPerspective() || sx <= 0 || std::fabs(sx - sy) > 1e-4f ||
                 font.getSize() * sx > 400.f || stroke_width <= 0.f || mode < 0;
    std::vector<SkGlyphID> gl(text.size() + 1);
    size_t ng = font.textToGlyphs(text.data(), text.size(), SkTextEncoding::kUTF8, SkSpan<SkGlyphID>(gl));
    gl.resize(ng);
    if (plain) {
        if (fill != SK_ColorTRANSPARENT) draw_simple_text(canvas, text.data(), text.size(), SkTextEncoding::kUTF8, x, y, font, fp);
        std::vector<SkScalar> wd(ng);
        font.getWidths(SkSpan<const SkGlyphID>(gl), SkSpan<SkScalar>(wd));
        float cx = x;
        for (size_t i = 0; i < ng; i++) {
            if (auto gp = font.getPath(gl[i])) canvas.drawPath(gp->makeTransform(SkMatrix::Translate(cx, y)), sp);
            cx += wd[i] + (i + 1 < ng ? kerning_px(font, gl[i], gl[i + 1]) : 0.f);
        }
        return adv;
    }
    if (SkColorGetA(fill) != 0 && mode != 1) draw_simple_text(canvas, text.data(), text.size(), SkTextEncoding::kUTF8, x, y, font, fp);
    SkFont f = font;
    f.setSize(font.getSize() * sx);
    f.setSubpixel(true);
    f.setEdging(SkFont::Edging::kAntiAlias);
    std::vector<SkScalar> wd(ng);
    f.getWidths(SkSpan<const SkGlyphID>(gl), SkSpan<SkScalar>(wd));
    float dx = x * sx + m.getTranslateX(), dy = y * sy + m.getTranslateY();
    std::string mask_key;
    mask_key.reserve(text.size() + 96);
    key_put(mask_key, f.getTypeface() ? f.getTypeface()->uniqueID() : 0u);
    key_put(mask_key, f.getSize());
    key_put(mask_key, f.getScaleX());
    key_put(mask_key, f.getSkewX());
    key_put(mask_key, (uint8_t)((f.isEmbolden() ? 1 : 0) | (f.isLinearMetrics() ? 2 : 0) | (f.isBaselineSnap() ? 4 : 0) |
                                (f.isForceAutoHinting() ? 8 : 0) | (f.isEmbeddedBitmaps() ? 16 : 0)));
    key_put(mask_key, (uint8_t)f.getHinting());
    key_put(mask_key, dx);
    key_put(mask_key, dy);
    key_put(mask_key, stroke_width * sx);
    key_put(mask_key, mode);
    key_put(mask_key, (uint8_t)(skia_luminance(stroke) >> 5));
    mask_key.append(text.data(), text.size());
    MaskCache& cache = mask_cache();
    if (auto hit = cache.map.find(mask_key); hit != cache.map.end()) {
        const MaskEntry& e = hit->second;
        SkPaint hp;
        hp.setColor(stroke);
        canvas.save();
        canvas.resetMatrix();
        canvas.drawImage(e.image, (float)e.left, (float)e.top, SkSamplingOptions(), &hp);
        canvas.restore();
        return adv;
    }
    float qy = std::floor(dy + 0.5f), pen = dx;
    SkPathBuilder pb;
    for (size_t i = 0; i < ng; i++) {
        float qx = std::floor(pen * 4.f + 0.5f) / 4.f;
        if (auto gp = f.getPath(gl[i])) pb.addPath(*gp, qx, qy);
        pen += wd[i] + (i + 1 < ng ? kerning_px(f, gl[i], gl[i + 1]) : 0.f);
    }
    SkPath outline = pb.detach();
    SkPaint dp(sp);
    dp.setStrokeWidth(stroke_width * sx);
    SkPathBuilder ring;
    SkMatrix fine = SkMatrix::Scale(8.f, 8.f);
    if (!skpathutils::FillPathWithPaint(outline, dp, &ring, nullptr, fine)) return adv;
    SkPath cover = ring.detach();
    if (mode == 1) {
        SkPathBuilder both;
        both.addPath(outline);
        both.addPath(cover);
        cover = both.detach();
    }
    SkRect b = cover.getBounds();
    int L = (int)std::floor(b.fLeft) - 2, T = (int)std::floor(b.fTop) - 2;
    int W = (int)std::ceil(b.fRight) + 2 - L, H = (int)std::ceil(b.fBottom) + 2 - T;
    std::vector<float> sub;
    if (W <= 0 || H <= 0 || !chromium_path_coverage(cover, SkMatrix::Translate((float)-L, (float)-T).postScale(3.f, 1.f), W * 3, H, sub))
        return adv;
    const MaskGammaCfg& mg = mask_gamma_cfg();
    const uint8_t* lut = mg.on ? mask_gamma_table(stroke, mg.contrast, mg.gamma) : nullptr;
    const bool pre = lut && f.getSize() > 48.f;
    SkBitmap a8;
    a8.allocPixels(SkImageInfo::MakeA8(W, H));
    for (int j = 0; j < H; j++) {
        uint8_t* d = a8.getAddr8(0, j);
        for (int i = 0; i < W; i++) {
            float v = 0.f;
            for (int q = 0; q < 3; q++) {
                float c = std::min(1.f, std::max(0.f, sub[(size_t)j * W * 3 + (size_t)i * 3 + q]));
                if (pre) c = lut[(int)std::lround(c * 255.f)] / 255.f;
                v += c;
            }
            int a = (int)std::lround(std::min(1.f, v / 3.f) * 255.f);
            d[i] = (uint8_t)(lut && !pre ? lut[a] : a);
        }
    }
    a8.setImmutable();
    sk_sp<SkImage> mask_image = mask_on_gpu(canvas, a8.asImage());
    static const size_t kMaskBudget = 48u << 20;
    if (cache.bytes > kMaskBudget) {
        cache.map.clear();
        cache.bytes = 0;
    }
    size_t cost = (size_t)W * (size_t)H + mask_key.size() + 64;
    cache.map.emplace(std::move(mask_key), MaskEntry{mask_image, L, T, cost});
    cache.bytes += cost;
    SkPaint ip;
    ip.setColor(stroke);
    canvas.save();
    canvas.resetMatrix();
    canvas.drawImage(mask_image, (float)L, (float)T, SkSamplingOptions(), &ip);
    canvas.restore();
    return adv;
}

}  // namespace a380
