#if defined(_WIN32) && defined(A380_GLYPH_BAKE_TOOL)

#include "glyphs_win.h"
#include "a380/env.h"

#include <algorithm>
#include <cctype>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <map>
#include <memory>

#include <windows.h>
#include <dwrite_2.h>
#include <wrl/client.h>

#include "include/core/SkFontTypes.h"
#include "include/core/SkPaint.h"
#include "include/core/SkSurfaceProps.h"
#include "include/core/SkTypeface.h"
#include "src/core/SkArenaAlloc.h"
#include "src/core/SkDescriptor.h"
#include "src/core/SkGlyph.h"
#include "src/core/SkScalerContext.h"

#include "a380/assets.h"
#include "a380/glyphs.h"

namespace fs = std::filesystem;
using Microsoft::WRL::ComPtr;

namespace a380::glyphs::win {

namespace {

struct Dw {
    ComPtr<IDWriteFactory2> factory;
    std::map<std::string, ComPtr<IDWriteFontFace>> faces;
    std::map<std::string, std::string> paths;
    bool scanned = false;
};

Dw& dw() {
    thread_local Dw d;
    return d;
}

std::string lower(std::string s) {
    std::transform(s.begin(), s.end(), s.begin(), [](unsigned char c) { return (char)std::tolower(c); });
    return s;
}

IDWriteFontFace* face_for(const std::string& key) {
    Dw& d = dw();
    if (!d.factory) {
        if (FAILED(DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory2), reinterpret_cast<IUnknown**>(d.factory.GetAddressOf()))))
            return nullptr;
    }
    auto it = d.faces.find(key);
    if (it != d.faces.end()) return it->second.Get();
    if (!d.scanned) {
        d.scanned = true;
        std::error_code ec;
        for (fs::recursive_directory_iterator i(assets::fonts_dir(), fs::directory_options::skip_permission_denied, ec), end; !ec && i != end;
             i.increment(ec)) {
            if (!i->is_regular_file(ec)) continue;
            d.paths.emplace(lower(i->path().filename().string()), i->path().string());
        }
    }
    ComPtr<IDWriteFontFace> face;
    auto p = d.paths.find(key);
    if (p != d.paths.end()) {
        std::wstring w = fs::path(p->second).wstring();
        ComPtr<IDWriteFontFile> file;
        if (SUCCEEDED(d.factory->CreateFontFileReference(w.c_str(), nullptr, &file))) {
            BOOL supported = FALSE;
            DWRITE_FONT_FILE_TYPE ft;
            DWRITE_FONT_FACE_TYPE fct;
            UINT32 n = 0;
            if (SUCCEEDED(file->Analyze(&supported, &ft, &fct, &n)) && supported) {
                IDWriteFontFile* files[1] = {file.Get()};
                if (FAILED(d.factory->CreateFontFace(fct, 1, files, 0, DWRITE_FONT_SIMULATIONS_NONE, &face))) face.Reset();
            }
        }
    }
    d.faces[key] = face;
    return face.Get();
}

}  // namespace

bool bake(const std::string& face_key, const SkFont& f, SkGlyphID g, int sub, int& left, int& top, int& w, int& h, std::vector<uint8_t>& rgb) {
    if (f.getSize() > 48.f || f.getScaleX() != 1.f || f.getSkewX() != 0.f || f.getHinting() != SkFontHinting::kNone || f.isEmbolden()) return false;
    IDWriteFontFace* face = face_for(face_key);
    if (!face) return false;
    Dw& d = dw();
    FLOAT advance = 0.f;
    UINT16 index = g;
    DWRITE_GLYPH_OFFSET offset{0.f, 0.f};
    DWRITE_GLYPH_RUN run;
    run.fontFace = face;
    run.fontEmSize = f.getSize();
    run.glyphCount = 1;
    run.glyphIndices = &index;
    run.glyphAdvances = &advance;
    run.glyphOffsets = &offset;
    run.isSideways = FALSE;
    run.bidiLevel = 0;
    DWRITE_MATRIX xf{1.f, 0.f, 0.f, 1.f, 0.25f * (float)sub, 0.f};
    ComPtr<IDWriteGlyphRunAnalysis> gra;
    if (FAILED(d.factory->CreateGlyphRunAnalysis(&run, &xf, DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC, DWRITE_MEASURING_MODE_NATURAL,
                                                 DWRITE_GRID_FIT_MODE_DISABLED, DWRITE_TEXT_ANTIALIAS_MODE_CLEARTYPE, 0.f, 0.f, &gra)))
        return false;
    RECT bb{};
    if (FAILED(gra->GetAlphaTextureBounds(DWRITE_TEXTURE_CLEARTYPE_3x1, &bb))) return false;
    if (bb.left >= bb.right || bb.top >= bb.bottom) {
        left = top = w = h = 0;
        rgb.clear();
        return true;
    }
    const int bw = bb.right - bb.left, bh = bb.bottom - bb.top;
    std::vector<uint8_t> buf((size_t)bw * bh * 3);
    if (FAILED(gra->CreateAlphaTexture(DWRITE_TEXTURE_CLEARTYPE_3x1, &bb, buf.data(), (UINT32)buf.size()))) return false;
    int x0 = bw, y0 = bh, x1 = -1, y1 = -1;
    for (int y = 0; y < bh; y++)
        for (int x = 0; x < bw; x++) {
            const uint8_t* q = &buf[((size_t)y * bw + x) * 3];
            if (q[0] | q[1] | q[2]) {
                x0 = std::min(x0, x);
                y0 = std::min(y0, y);
                x1 = std::max(x1, x);
                y1 = std::max(y1, y);
            }
        }
    if (x1 < 0) {
        left = top = w = h = 0;
        rgb.clear();
        return true;
    }
    left = bb.left + x0;
    top = bb.top + y0;
    w = x1 - x0 + 1;
    h = y1 - y0 + 1;
    rgb.resize((size_t)w * h * 3);
    for (int y = 0; y < h; y++) std::memcpy(&rgb[(size_t)y * w * 3], &buf[((size_t)(y0 + y) * bw + x0) * 3], (size_t)w * 3);
    return true;
}

namespace {

struct Cfg {
    float contrast, gamma;
};

const Cfg kCfgs[] = {{1.f, 2.2f}, {0.5f, 2.2f}, {1.f, 1.f}, {0.f, 1.f}, {0.5f, 1.4f}};

const SkColor kColors[] = {0xff070707, 0xff272727, 0xff484848, 0xff696969, 0xff8a8a8a, 0xffababab, 0xffcccccc, 0xffeeeeee,
                           0xffff8020, 0xff20ff80, 0xff00c0ff, 0xff80ff00, 0xffd020d0, 0xffe0a000, 0xff12e8c7, 0xff9a3ff0};

struct Ctx {
    std::unique_ptr<SkScalerContext> ctx;
};

SkScalerContext* context(const SkFont& f, const std::string& id, int cfg, int col) {
    thread_local std::map<std::string, Ctx> cache;
    std::string key = id;
    key.push_back((char)cfg);
    key.push_back((char)col);
    auto it = cache.find(key);
    if (it != cache.end()) return it->second.ctx.get();
    SkFont sf = f;
    sf.setEdging(SkFont::Edging::kSubpixelAntiAlias);
    sf.setSubpixel(true);
    SkPaint paint;
    paint.setAntiAlias(true);
    paint.setColor(kColors[col]);
    SkSurfaceProps props(0, kRGB_H_SkPixelGeometry, kCfgs[cfg].contrast, kCfgs[cfg].gamma);
    SkScalerContextRec rec;
    SkScalerContextEffects eff;
    SkScalerContext::MakeRecAndEffects(sf, paint, props, SkScalerContextFlags::kFakeGammaAndBoostContrast, SkMatrix::I(), &rec, &eff);
    SkAutoDescriptor ad;
    SkDescriptor* desc = SkScalerContext::AutoDescriptorGivenRecAndEffects(rec, eff, &ad);
    Ctx c;
    c.ctx = sf.getTypeface()->createScalerContext(eff, desc);
    SkScalerContext* p = c.ctx.get();
    cache.emplace(std::move(key), std::move(c));
    return p;
}

}  // namespace

void prove(const SkFont& f, SkGlyphID g, int sub, int left, int top, int w, int h, const uint8_t* raw, Proof& out) {
    out.glyphs++;
    std::string id;
    {
        float v[3] = {f.getSize(), f.getScaleX(), f.getSkewX()};
        id.append(reinterpret_cast<const char*>(v), sizeof v);
        uint32_t tid = f.getTypeface()->uniqueID();
        id.append(reinterpret_cast<const char*>(&tid), 4);
    }
    for (int ci = 0; ci < (int)(sizeof kCfgs / sizeof kCfgs[0]); ci++) {
        for (int col = 0; col < (int)(sizeof kColors / sizeof kColors[0]); col++) {
            SkScalerContext* ctx = context(f, id, ci, col);
            if (!ctx) {
                out.unsupported++;
                continue;
            }
            SkArenaAlloc alloc(4096);
            SkGlyph glyph = ctx->makeGlyph(SkPackedGlyphID(g, (uint32_t)sub, 0u), &alloc);
            const bool empty = glyph.isEmpty();
            if (!empty && glyph.maskFormat() != SkMask::kLCD16_Format) {
                if (ci == 0 && col == 0 && a380::getenv_cached("A380_GLYPH_PROOF_DEBUG"))
                    std::fprintf(stderr, "unsupported glyph %d size %.2f sub %d format %d w %d h %d mine %dx%d\n", (int)g, f.getSize(), sub,
                                 (int)glyph.maskFormat(), glyph.width(), glyph.height(), w, h);
                out.unsupported++;
                continue;
            }
            int sl = 0, st = 0, sw = 0, sh = 0;
            const uint16_t* img = nullptr;
            size_t rb = 0;
            if (!empty) {
                if (!glyph.setImage(&alloc, ctx)) {
                    out.unsupported++;
                    continue;
                }
                sl = glyph.left();
                st = glyph.top();
                sw = glyph.width();
                sh = glyph.height();
                img = static_cast<const uint16_t*>(glyph.image());
                rb = glyph.rowBytes();
            }
            const int x0 = std::min(left, sl), y0 = std::min(top, st);
            const int x1 = std::max(left + w, sl + sw), y1 = std::max(top + h, st + sh);
            const SkColor c = kColors[col];
            const uint8_t* lut[3] = {lcd_preblend((int)(SkColorGetR(c) >> 5), kCfgs[ci].contrast, kCfgs[ci].gamma),
                                     lcd_preblend((int)(SkColorGetG(c) >> 5), kCfgs[ci].contrast, kCfgs[ci].gamma),
                                     lcd_preblend((int)(SkColorGetB(c) >> 5), kCfgs[ci].contrast, kCfgs[ci].gamma)};
            int bad = 0;
            for (int y = y0; y < y1; y++)
                for (int x = x0; x < x1; x++) {
                    unsigned mine = 0, theirs = 0;
                    if (x >= left && x < left + w && y >= top && y < top + h) {
                        const uint8_t* q = raw + ((size_t)(y - top) * w + (x - left)) * 3;
                        mine = ((lut[0][q[0]] >> 3) << 11) | ((lut[1][q[1]] >> 2) << 5) | (lut[2][q[2]] >> 3);
                    }
                    if (img && x >= sl && x < sl + sw && y >= st && y < st + sh)
                        theirs = reinterpret_cast<const uint16_t*>(reinterpret_cast<const uint8_t*>(img) + (size_t)(y - st) * rb)[x - sl];
                    out.pixels++;
                    if (mine != theirs) bad++;
                }
            out.combos++;
            out.bad_pixels += bad;
            if (bad) out.bad_combos++;
            if (kCfgs[ci].contrast == 0.f && kCfgs[ci].gamma == 1.f) {
                out.grey_combos++;
                if (bad) out.grey_bad++;
            }
        }
    }
}

}  // namespace a380::glyphs::win

#endif
