#include <algorithm>
#ifdef A380_GPU
#include "include/gpu/ganesh/GrDirectContext.h"
#include "include/gpu/ganesh/GrRecordingContext.h"
#include "include/gpu/ganesh/SkImageGanesh.h"
#endif
#include "a380/shape_cache.h"
#include "a380/env.h"
#include <cctype>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <unordered_map>
#include <vector>

#include "include/codec/SkCodec.h"
#include "include/core/SkBitmap.h"
#include "include/core/SkData.h"
#include "include/core/SkImage.h"

#include "include/core/SkMatrix.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPixmap.h"
#include "include/core/SkPathUtils.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkSurface.h"

#include "a380/assets.h"
#include "a380/display.h"
#include "a380/display_unit.h"
#include "a380/raster.h"
#include "sdsys.h"

namespace sdsys {

int js_round(double x) { return (int)std::floor(x + 0.5); }

sk_sp<SkImage> load_image(const std::string& name) {
    static std::unordered_map<std::string, sk_sp<SkImage>> cache;
    auto it = cache.find(name);
    if (it != cache.end()) return it->second;
    sk_sp<SkImage> img;
    for (auto& dir : a380::assets::images_dirs()) {
        auto data = SkData::MakeFromFileName((dir + "/" + name).c_str());
        if (!data) continue;
        auto codec = SkCodec::MakeFromData(data);
        if (!codec) continue;
        auto [image, result] = codec->getImage();
        if (image) {
            img = image;
            break;
        }
    }
    cache[name] = img;
    return img;
}

static sk_sp<SkImage> box_resize(const sk_sp<SkImage>& img, int w, int h) {
    SkBitmap src;
    src.allocN32Pixels(img->width(), img->height());
    SkPixmap sp;
    src.peekPixels(&sp);
    img->readPixels(nullptr, sp, 0, 0);
    SkBitmap dst;
    dst.allocN32Pixels(w, h);
    auto weights = [&](int n, int srcn, std::vector<std::vector<std::pair<int, float>>>& out) {
        float scale = (float)srcn / n;
        out.assign(n, {});
        for (int i = 0; i < n; i++) {
            float lo = i * scale, hi = (i + 1) * scale;
            for (int k = (int)std::floor(lo); k < std::min(srcn, (int)std::ceil(hi)); k++) {
                float ov = std::min(hi, (float)k + 1) - std::max(lo, (float)k);
                if (ov > 0) out[i].push_back({k, ov / scale});
            }
        }
    };
    std::vector<std::vector<std::pair<int, float>>> wx, wy;
    weights(w, img->width(), wx);
    weights(h, img->height(), wy);
    for (int y = 0; y < h; y++)
        for (int x = 0; x < w; x++) {
            float acc[4] = {0, 0, 0, 0};
            for (auto& [ky, vy] : wy[y])
                for (auto& [kx, vx] : wx[x]) {
                    uint32_t px = *src.getAddr32(kx, ky);
                    for (int k = 0; k < 4; k++) acc[k] += vx * vy * (float)((px >> (8 * k)) & 0xff);
                }
            uint32_t o = 0;
            for (int k = 0; k < 4; k++) o |= (uint32_t)std::clamp((int)std::lround(acc[k]), 0, 255) << (8 * k);
            *dst.getAddr32(x, y) = o;
        }
    dst.setImmutable();
    return dst.asImage();
}

void draw_image_two_step(SkCanvas& cv, const sk_sp<SkImage>& img, const SkRect& dst) {
    SkRect dev = cv.getTotalMatrix().mapRect(dst);
    if (dev.width() >= img->width() && dev.height() >= img->height()) {
        cv.drawImageRect(img, dst, SkSamplingOptions(SkFilterMode::kLinear));
        return;
    }
    static std::unordered_map<std::string, sk_sp<SkImage>> cache;
    std::string key = std::to_string((uintptr_t)img.get()) + "," + std::to_string(dev.width()) + "," + std::to_string(dev.height());
    auto it = cache.find(key);
    if (it == cache.end()) {
        sk_sp<SkImage> cur = img;
        int tw = (int)std::ceil(dev.width()), th = (int)std::ceil(dev.height());
        while ((cur->width() + 1) / 2 >= tw && (cur->height() + 1) / 2 >= th && cur->width() >= 2 && cur->height() >= 2)
            cur = box_resize(cur, (cur->width() + 1) / 2, (cur->height() + 1) / 2);
        it = cache.emplace(key, cur).first;
    }
    cv.drawImageRect(it->second, dst, SkSamplingOptions(SkFilterMode::kLinear, SkMipmapMode::kNearest));
}

sk_sp<SkImage> chromium_mip(const sk_sp<SkImage>& img, SkCanvas& cv, float css_w) {
    float dest = css_w * cv.getTotalMatrix().getScaleX();
    sk_sp<SkImage> cur = img;
    while (cur->width() / 2 >= dest && cur->width() >= 2 && cur->height() >= 2) {
        int w = cur->width() / 2, h = cur->height() / 2;
        auto surf = SkSurfaces::Raster(SkImageInfo::MakeN32Premul(w, h));
        surf->getCanvas()->drawImageRect(cur, SkRect::MakeWH(w, h), SkSamplingOptions(SkFilterMode::kLinear));
        cur = surf->makeImageSnapshot();
    }
    return cur;
}

static SkPaint solid(uint32_t c) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(a380::rgb(c));
    return p;
}


static float envf(const char* n, float d) {
    const char* e = a380::getenv_cached(n);
    return e ? (float)std::atof(e) : d;
}

static const uint8_t* mask_lut(uint32_t color, bool fir = false) {
    static std::vector<uint8_t> cache_a, cache_b;
    std::vector<uint8_t>& cache = fir ? cache_b : cache_a;
    const float contrast = fir ? envf("SD_CT", 0.0f) : 1.0f, gamma = fir ? envf("SD_GM", 0.0f) : 2.2f;
    if (cache.empty()) {
        cache.assign(8 * 256, 0);
        auto to_lin = [&](float v) { return gamma == 0.f ? (v <= 0.04045f ? v / 12.92f : std::pow((v + 0.055f) / 1.055f, 2.4f)) : std::pow(v, gamma); };
        auto from_lin = [&](float v) { return gamma == 0.f ? (v <= 0.0031308f ? v * 12.92f : 1.055f * std::pow(v, 1.f / 2.4f) - 0.055f) : std::pow(v, 1.f / gamma); };
        for (int k = 0; k < 8; k++) {
            int b = k << 5, lum = b;
            for (int q = 3; q < 8; q += 3) lum |= b >> q;
            float src = lum / 255.f, dst = 1.f - src, ls = to_lin(src), ld = to_lin(dst), adj = contrast * ld;
            for (int i = 0; i < 256; i++) {
                float raw = i / 255.f, sa = raw + (1.f - raw) * adj * raw;
                float out = from_lin(ls * sa + (1.f - sa) * ld);
                float r = (out - dst) / (src - dst);
                cache[(size_t)k * 256 + i] = (uint8_t)std::min(255L, std::max(0L, std::lround(255.f * r)));
            }
        }
    }
    int lum = (((color >> 16) & 255) * 54 + ((color >> 8) & 255) * 183 + (color & 255) * 19) >> 8;
    return &cache[(size_t)(lum >> 5) * 256];
}

static void stroked_mask(SkCanvas& cv, const std::vector<SkPath>& paths, const SkPaint& paint) {
    if (paths.empty()) return;
    SkMatrix m = cv.getTotalMatrix();
    SkRect b = SkRect::MakeEmpty();
    for (auto& pa : paths) b.join(pa.getBounds());
    SkRect db = m.mapRect(b);
    int x0 = (int)std::floor(db.fLeft) - 3, y0 = (int)std::floor(db.fTop) - 3;
    int w = (int)std::ceil(db.fRight) + 3 - x0, h = (int)std::ceil(db.fBottom) + 3 - y0;
    SkBitmap bm;
    bm.allocPixels(SkImageInfo::MakeA8(w, h));
    bm.eraseColor(0);
    SkCanvas mc(bm);
    mc.translate(-(float)x0, -(float)y0);
    mc.concat(m);
    SkPaint wp(paint);
    wp.setColor(SK_ColorWHITE);
    for (auto& pa : paths) mc.drawPath(pa, wp);
    const uint8_t* lut = mask_lut(paint.getColor() & 0xffffff);
    for (int j = 0; j < h; j++) {
        uint8_t* d = bm.getAddr8(0, j);
        for (int i = 0; i < w; i++) d[i] = lut[d[i]];
    }
    cv.save();
    cv.resetMatrix();
    SkPaint ip;
    ip.setColor(paint.getColor());
    cv.drawImage(bm.asImage(), (float)x0, (float)y0, SkSamplingOptions(), &ip);
    cv.restore();
}

bool stroked_text_fir(SkCanvas& cv, std::string_view text, float x, float y, const SkFont& font, uint32_t color,
                             float sw, float ls, float ws) {
    SkMatrix m = cv.getLocalToDeviceAs3x3();
    float sx = m.getScaleX(), sy = m.getScaleY();
    if (m.getSkewX() != 0 || m.getSkewY() != 0 || m.hasPerspective() || sx <= 0 || std::fabs(sx - sy) > 1e-4f) return false;
    std::vector<SkGlyphID> gl(text.size() + 1);
    size_t ng = font.textToGlyphs(text.data(), text.size(), SkTextEncoding::kUTF8, SkSpan<SkGlyphID>(gl));
    gl.resize(ng);
    SkFont f = font;
    f.setSize(font.getSize() * sx);
    f.setSubpixel(true);
    f.setEdging(SkFont::Edging::kAntiAlias);
    std::vector<SkScalar> wd(ng);
    f.getWidths(SkSpan<const SkGlyphID>(gl), SkSpan<SkScalar>(wd));
    float dx = x * sx + m.getTranslateX(), dy = y * sy + m.getTranslateY();
    struct FirEntry {
        sk_sp<SkImage> image;
        int left, top;
    };
    static std::unordered_map<std::string, FirEntry> fir_cache;
    std::string fir_key;
    {
        auto put = [&](const auto& v) { fir_key.append(reinterpret_cast<const char*>(&v), sizeof v); };
        uint32_t tf = f.getTypeface() ? f.getTypeface()->uniqueID() : 0u;
        float fsz = f.getSize(), fsx = f.getScaleX(), fsk = f.getSkewX(), swd = sw * sx;
        uint8_t fl = (uint8_t)((f.isEmbolden() ? 1 : 0) | (f.isLinearMetrics() ? 2 : 0) | (f.isBaselineSnap() ? 4 : 0) | (f.isForceAutoHinting() ? 8 : 0));
        uint8_t hint = (uint8_t)f.getHinting();
        put(tf), put(fsz), put(fsx), put(fsk), put(fl), put(hint), put(dx), put(dy), put(swd), put(ls), put(ws), put(color);
        fir_key.append(text.data(), text.size());
    }
    if (auto hit = fir_cache.find(fir_key); hit != fir_cache.end()) {
        SkPaint hp;
        hp.setColor(a380::rgb(color));
        cv.save();
        cv.resetMatrix();
        cv.drawImage(hit->second.image, (float)hit->second.left, (float)hit->second.top, SkSamplingOptions(), &hp);
        cv.restore();
        return true;
    }
    float qy = std::floor(dy + 0.5f), pen = dx;
    SkPathBuilder pb;
    for (size_t i = 0; i < ng; i++) {
        float qx = std::floor(pen * 4.f + 0.5f) / 4.f;
        if (auto gp = f.getPath(gl[i])) pb.addPath(*gp, qx, qy);
        pen += wd[i];
        if (ls != 0.f || ws != 0.f) {
            pen += ls * sx + (text[i] == ' ' ? ws * sx : 0.f);
        } else if (i + 1 < ng) {
            pen += a380::kerning_px(f, gl[i], gl[i + 1]);
        }
    }
    SkPath outline = pb.detach();
    SkPaint dp;
    dp.setAntiAlias(true);
    dp.setStyle(SkPaint::kStroke_Style);
    dp.setStrokeWidth(sw * sx);
    SkPathBuilder ring;
    if (!skpathutils::FillPathWithPaint(outline, dp, &ring, nullptr, SkMatrix::Scale(8.f, 8.f))) return true;
    SkPathBuilder both;
    both.addPath(outline);
    both.addPath(ring.detach());
    SkPath cover = both.detach();
    SkRect b = cover.getBounds();
    int L = (int)std::floor(b.fLeft) - 3, T = (int)std::floor(b.fTop) - 2;
    int W = (int)std::ceil(b.fRight) + 3 - L, H = (int)std::ceil(b.fBottom) + 2 - T;
    std::vector<float> sub;
    if (W <= 0 || H <= 0 ||
        !a380::chromium_path_coverage(cover, SkMatrix::Translate((float)-L, (float)-T).postScale(3.f, 1.f), W * 3, H, sub))
        return true;
    static const float k1 = envf("SD_K1", 0.5f), k2 = envf("SD_K2", 0.1f);
    const float norm = 3.f + 2.f * k1 + 2.f * k2;
    const uint8_t* lut = mask_lut(color, true);
    SkBitmap a8;
    a8.allocPixels(SkImageInfo::MakeA8(W, H));
    for (int j = 0; j < H; j++) {
        const float* r = &sub[(size_t)j * W * 3];
        uint8_t* d = a8.getAddr8(0, j);
        auto at = [&](int q) { return q < 0 || q >= W * 3 ? 0.f : std::min(1.f, std::max(0.f, r[q])); };
        for (int i = 0; i < W; i++) {
            float v = 0.f;
            for (int q = 0; q < 3; q++) {
                int c = i * 3 + q;
                v += at(c) + k1 * (at(c - 1) + at(c + 1)) + k2 * (at(c - 2) + at(c + 2));
            }
            d[i] = lut[(int)std::lround(std::min(1.f, v / (3.f * (1.f + 2.f * k1 + 2.f * k2))) * 255.f)];
        }
    }
    a8.setImmutable();
    sk_sp<SkImage> fir_image = a8.asImage();
#ifdef A380_GPU
    if (GrRecordingContext* rc = cv.recordingContext(); rc && rc->asDirectContext())
        if (sk_sp<SkImage> tex = SkImages::TextureFromImage(rc->asDirectContext(), fir_image)) fir_image = tex;
#endif
    if (fir_cache.size() > 4096) fir_cache.clear();
    fir_cache.emplace(std::move(fir_key), FirEntry{fir_image, L, T});
    SkPaint ip;
    ip.setColor(a380::rgb(color));
    cv.save();
    cv.resetMatrix();
    cv.drawImage(fir_image, (float)L, (float)T, SkSamplingOptions(), &ip);
    cv.restore();
    return true;
}

float text(SkCanvas& cv, std::string_view s, float x, float y, const TextStyle& st) {
    SkFont f = a380::font("Ecam", st.px);
    if (st.px * cv.getTotalMatrix().getScaleY() > 48.f) f.setEdging(SkFont::Edging::kAntiAlias);
    std::vector<SkGlyphID> g(s.size());
    size_t n = f.textToGlyphs(s.data(), s.size(), SkTextEncoding::kUTF8, SkSpan<SkGlyphID>(g));
    g.resize(n);
    std::vector<SkScalar> wd(n);
    f.getWidths(SkSpan<const SkGlyphID>(g), SkSpan<SkScalar>(wd));
    SkGlyphID space = 0;
    f.textToGlyphs(" ", 1, SkTextEncoding::kUTF8, SkSpan<SkGlyphID>(&space, 1));
    float tw = 0;
    for (size_t i = 0; i < n; i++) {
        wd[i] += st.ls + (g[i] == space ? st.ws : 0.f);
        tw += wd[i];
    }
    float cx = st.align == a380::Align::Left ? x : st.align == a380::Align::Center ? x - tw * 0.5f : x - tw;
    float by = y + a380::baseline_offset(f, st.base);
    const float by_raw = by;
    if (a380::getenv_cached("SDDBG")) std::fprintf(stderr, "TXT\t%s\t%f\t%f\t%f\t%f\n", std::string(s).c_str(), (cx * cv.getTotalMatrix().getScaleX() + cv.getTotalMatrix().getTranslateX()) / 1.5f, (by_raw * cv.getTotalMatrix().getScaleY() + cv.getTotalMatrix().getTranslateY()) / 1.5f, st.px, tw);
    {
        SkMatrix m = cv.getTotalMatrix();
        float sy = m.getScaleY(), ty = m.getTranslateY();
        if (sy > 0.f && !st.underline) by = (std::floor(by * sy + ty + 0.5f) - ty) / sy;
    }
    bool stroked = !st.plain && (st.outline > 0.f || st.color == kGreen || st.color == kAmber || st.color == kLightGrey);
    SkPaint p = solid(st.color);
    const float sdx = 0.f, sdy = 0.f;
    float x0 = cx;
    if (stroked) {
        p.setStyle(SkPaint::kStrokeAndFill_Style);
        const float sw = st.outline > 0.f ? st.outline : envf("SD_SW", 1.1f);
        p.setStrokeWidth(sw);
        if (!stroked_text_fir(cv, s, x0, by, f, st.color, sw, st.ls, st.ws)) {
            if (st.ls == 0.f && st.ws == 0.f) {
                a380::draw_stroked_text(cv, s, x0, by, f, a380::rgb(st.color), a380::rgb(st.color), sw);
            } else {
                std::vector<SkPath> paths;
                for (size_t i = 0; i < n; i++) {
                    if (auto path = f.getPath(g[i])) paths.push_back(path->makeTransform(SkMatrix::Translate(cx + sdx, by + sdy)));
                    cx += wd[i];
                }
                stroked_mask(cv, paths, p);
            }
        }
    } else {
        std::vector<float> xs(n);
        for (size_t i = 0; i < n; i++) {
            xs[i] = cx;
            cx += wd[i];
        }
        a380::draw_glyph_run(cv, g.data(), xs.data(), n, by, f, p);
    }
    if (st.underline) {
        SkFontMetrics m;
        f.getMetrics(&m);
        float th = 0.1f * st.px;
        float uy = by + st.px / 18.f;
        SkPaint u = solid(st.color);
        u.setAntiAlias(true);
        draw_rect(cv, SkRect::MakeXYWH(x0, uy, tw, th), u);
    }
    return tw;
}

SkPath svg_path(std::string_view d) {
    SkPathBuilder pb;
    size_t i = 0;
    char cmd = 0;
    float cx = 0, cy = 0, sx = 0, sy = 0;
    bool bad = false;
    auto skip = [&] {
        while (i < d.size() && (d[i] == ' ' || d[i] == ',' || d[i] == '\t' || d[i] == '\n')) i++;
    };
    auto num = [&]() {
        skip();
        char* end = nullptr;
        std::string tmp(d.substr(i, 32));
        float v = std::strtof(tmp.c_str(), &end);
        if (end == tmp.c_str()) bad = true;
        i += (size_t)(end - tmp.c_str());
        return std::isfinite(v) ? v : (bad = true, 0.f);
    };
    while (!bad) {
        skip();
        if (i >= d.size()) break;
        if (std::isalpha((unsigned char)d[i])) cmd = d[i++];
        bool rel = std::islower((unsigned char)cmd);
        switch (std::toupper((unsigned char)cmd)) {
            case 'M': {
                float px = num(), py = num();
                if (rel) { px += cx; py += cy; }
                pb.moveTo(px, py);
                cx = sx = px; cy = sy = py;
                cmd = rel ? 'l' : 'L';
                break;
            }
            case 'L': {
                float px = num(), py = num();
                if (rel) { px += cx; py += cy; }
                pb.lineTo(px, py);
                cx = px; cy = py;
                break;
            }
            case 'H': {
                float px = num();
                if (rel) px += cx;
                pb.lineTo(px, cy);
                cx = px;
                break;
            }
            case 'V': {
                float py = num();
                if (rel) py += cy;
                pb.lineTo(cx, py);
                cy = py;
                break;
            }
            case 'C': {
                float x1 = num(), y1 = num(), x2 = num(), y2 = num(), px = num(), py = num();
                if (rel) { x1 += cx; y1 += cy; x2 += cx; y2 += cy; px += cx; py += cy; }
                pb.cubicTo(x1, y1, x2, y2, px, py);
                cx = px; cy = py;
                break;
            }
            case 'Z':
                pb.close();
                cx = sx; cy = sy;
                break;
            default:
                return pb.detach();
        }
    }
    return pb.detach();
}

void stroke_path(SkCanvas& cv, const SkPath& path, uint32_t color, float width, bool round) {
    SkPaint p = solid(color);
    p.setStyle(SkPaint::kStroke_Style);
    p.setStrokeWidth(width);
    if (round) {
        p.setStrokeCap(SkPaint::kRound_Cap);
        p.setStrokeJoin(SkPaint::kRound_Join);
    }
    draw_path(cv, path, p);
}

void fill_path(SkCanvas& cv, const SkPath& path, uint32_t color) { draw_path(cv, path, solid(color)); }

void line(SkCanvas& cv, float x1, float y1, float x2, float y2, uint32_t color, float width) {
    SkPaint p = solid(color);
    p.setStyle(SkPaint::kStroke_Style);
    p.setStrokeWidth(width);
    draw_line(cv, x1, y1, x2, y2, p);
}

void triangle(SkCanvas& cv, float x, float y, uint32_t color, float orientation, float scale) {
    SkPathBuilder pb;
    pb.moveTo(scale * 8, 2 * scale * 7);
    pb.lineTo(0, 0);
    pb.lineTo(-(scale * 8), 2 * scale * 7);
    pb.close();
    SkPath p = pb.detach();
    SkMatrix m = SkMatrix::Translate(x, y);
    m.preRotate(orientation);
    SkPaint sp = solid(color);
    sp.setStyle(SkPaint::kStroke_Style);
    sp.setStrokeWidth(2.f);
    a380::draw_path_cached(cv, p.makeTransform(m), sp);
}

namespace {

struct SdSys : a380::Display {
    SkISize size() const override { return {768, 1024}; }
    void draw(SkCanvas& cv, const a380::Inputs& in) override {
        if (!du_.update(cv, in, size())) return;
        int page = (int)in.var("L:A32NX_ECAM_SD_PAGE_TO_SHOW");
        switch (page) {
            case 0: draw_eng(cv, in); break;
            case 6: draw_elec_ac(cv, in); break;
            case 7: draw_elec_dc(cv, in); break;
            case 10: draw_hyd(cv, in); break;
            case 1: draw_apu(cv, in); break;
            case 2: draw_bleed(cv, in); break;
            case 3: draw_cond(cv, in); break;
            case 5: draw_door(cv, in); break;
            case 8: draw_fuel(cv, in); break;
            case 4: draw_press(cv, in); break;
            case 9: draw_wheel(cv, in); break;
            case 12: draw_cb(cv, in); break;
            default: break;
        }
    }
    a380::CdsDisplayUnit du_{"93", {"DC_2"}};
};

std::unique_ptr<a380::Display> make(std::string_view) { return std::make_unique<SdSys>(); }

}  // namespace

A380_REGISTER_DISPLAY("sd", make);

}  // namespace sdsys
