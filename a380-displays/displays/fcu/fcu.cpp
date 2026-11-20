#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <map>
#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "include/codec/SkCodec.h"
#include "include/core/SkData.h"
#include "include/core/SkBitmap.h"
#include "include/core/SkImage.h"
#include "include/core/SkPixmap.h"
#include "include/core/SkPaint.h"
#include "include/core/SkSamplingOptions.h"

#include "a380/assets.h"
#include "a380/display.h"
#include "a380/draw.h"
#include "a380/fonts.h"

namespace {

using a380::Align;

sk_sp<SkImage> load_image(const std::string& name) {
    thread_local std::map<std::string, sk_sp<SkImage>> cache;
    auto it = cache.find(name);
    if (it != cache.end()) return it->second;
    sk_sp<SkImage> img;
    auto data = a380::assets::image_data("fcu/" + name);
    if (data) {
        std::unique_ptr<SkCodec> codec = SkCodec::MakeFromData(data);
        if (codec) img = std::get<0>(codec->getImage());
    }
    cache[name] = img;
    return img;
}

sk_sp<SkImage> box_resize(const sk_sp<SkImage>& img, int w, int h) {
    SkBitmap src;
    src.allocN32Pixels(img->width(), img->height());
    SkPixmap sp;
    src.peekPixels(&sp);
    img->readPixels(nullptr, sp, 0, 0);
    SkBitmap dst;
    dst.allocN32Pixels(w, h);
    const float sx = (float)img->width() / w, sy = (float)img->height() / h;
    auto weights = [&](int n, float scale, int srcn, std::vector<std::vector<std::pair<int, float>>>& out) {
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
    weights(w, sx, img->width(), wx);
    weights(h, sy, img->height(), wy);
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

class Fcu : public a380::Display {
public:
    SkISize size() const override { return {2560, 1280}; }

    void draw(SkCanvas& canvas, const a380::Inputs& in) override {
        SkPaint bg;
        bg.setColor(SkColorSetRGB(0x1a, 0x05, 0x66));
        canvas.drawRect(SkRect::MakeWH(2560, 1280), bg);

        test_ = in.var("L:A32NX_OVHD_INTLT_ANN", {}, 1) == 0;
        canvas.save();
        canvas.scale(0.5f, 0.5f);
        if (in.var("L:A32NX_FCU_AFS_CP_ACTIVE") != 0 && in.var("L:A380X_FCU_BKUP_AFS_ACTIVE") == 0) afs(canvas, in);
        for (int side = 0; side < 2; side++) {
            const char* s = side == 0 ? "L" : "R";
            std::string p = std::string("L:A32NX_FCU_EFIS_") + s + "_";
            if (in.var(p + "CP_ACTIVE") == 0) continue;
            if (in.var(std::string("L:A380X_FCU_BKUP_EFIS_") + s + "_ACTIVE") != 0) continue;
            baro(canvas, in, p, side);
            nd(canvas, in, p, side);
        }
        canvas.restore();
    }

private:
    bool test_ = false;
    std::map<std::string, sk_sp<SkImage>> levels_;

    static void text(SkCanvas& c, const std::string& s, float x, float y, const SkFont& f, Align a = Align::Left) {
        SkPaint p;
        p.setAntiAlias(true);
        p.setColor(SK_ColorWHITE);
        a380::draw_text(c, s, x, y, f, p, a);
    }

    void afs(SkCanvas& c, const a380::Inputs& in) {
        SkFont label = a380::font("Poppins-SemiBold", 115.f, false, true);
        SkFont digit = a380::font("Digital", 275.f, false, true);
        const float top = 102.4f, h = 409.6f;
        const float ly = top + 0.20f * h, vy = top + 0.97f * h;

        bool mach = in.var("L:A32NX_FCU_AFS_DISPLAY_MACH_MODE") != 0;
        bool sdash = in.var("L:A32NX_FCU_AFS_DISPLAY_SPD_MACH_DASHES") != 0;
        double sv = in.var("L:A32NX_FCU_AFS_DISPLAY_SPD_MACH_VALUE");
        float sx = 128.f, sw = 768.f;
        if (!mach || test_) text(c, "SPD", sx + 0.47f * sw, ly, label);
        if (mach || test_) text(c, "MACH", sx, ly, label);
        std::string spd;
        char b[32];
        if (test_) spd = "8.8.8";
        else if (mach && !sdash) { std::snprintf(b, sizeof b, "%.2f", sv); spd = b; }
        else if (sdash) spd = "---";
        else { std::snprintf(b, sizeof b, "%03d", (int)std::lround(sv)); spd = b; }
        text(c, spd, sx + 0.06f * sw, vy, digit);
        if (!mach && (!sdash || test_)) text(c, "KT", sx + 0.79f * sw, vy, label);

        bool trk = in.var("L:A32NX_FCU_AFS_DISPLAY_TRK_FPA_MODE") != 0;
        bool tru = in.var("L:A32NX_FCU_AFS_DISPLAY_TRUE_MODE") != 0;
        bool hdash = in.var("L:A32NX_FCU_AFS_DISPLAY_HDG_TRK_DASHES") != 0;
        double hv = in.var("L:A32NX_FCU_AFS_DISPLAY_HDG_TRK_VALUE");
        float hx = 768.f, hw = 1280.f;
        if (tru || test_) text(c, "TRUE", hx + 0.23f * hw, ly, label);
        if (!trk || test_) text(c, "HDG", hx + 0.48f * hw, ly, label);
        if (trk || test_) text(c, "TRK", hx + 0.70f * hw, ly, label);
        std::string hd;
        if (test_) hd = "8.8.8";
        else if (hdash) hd = "---";
        else { std::snprintf(b, sizeof b, "%03d", (int)std::lround(hv)); hd = b; }
        text(c, hd, hx + 0.47f * hw, vy, digit);
        if (!hdash || test_) text(c, "\xC2\xB0", hx + 0.89f * hw, vy, digit);

        double av = in.var("L:A32NX_FCU_AFS_DISPLAY_ALT_VALUE");
        float ax = 3072.f;
        text(c, "ALT", ax + 0.27f * 1280.f, ly, label);
        std::string alt;
        if (test_) alt = "8.8.8.8.8";
        else { std::snprintf(b, sizeof b, "%05d", (int)std::lround(av)); alt = b; }
        text(c, alt, ax, vy, digit);

        bool vdash = in.var("L:A32NX_FCU_AFS_DISPLAY_VS_FPA_DASHES") != 0;
        double vv = in.var("L:A32NX_FCU_AFS_DISPLAY_VS_FPA_VALUE");
        float vx = ax + 0.95f * 1024.f, vw = 1024.f;
        if (!trk || test_) text(c, "V/S", vx + 0.78f * vw, ly, label, Align::Right);
        if (trk || test_) text(c, "FPA", vx + 0.54f * vw, ly, label, Align::Right);
        std::string vs;
        const char sign = vv < 0 ? '-' : '+';
        double av2 = std::fabs(vv);
        if (test_) vs = "+*.8.8.8";
        else if (vdash) vs = "~----";
        else if (trk) { std::snprintf(b, sizeof b, "~%c%.1f", sign, std::round(av2 * 10) / 10); vs = b; }
        else { std::snprintf(b, sizeof b, "%c%02doo", sign, (int)std::floor(av2 * 0.01)); vs = b; }
        text(c, vs, vx + 0.14f * vw, vy, digit);
    }

    void baro(SkCanvas& c, const a380::Inputs& in, const std::string& p, int side) {
        float bx = side == 0 ? 3072.f : 4096.f;
        const float bw = 691.2f, bh = 435.2f, by = 921.6f;
        SkFont label = a380::font("Poppins-SemiBold", 105.f, false, true);
        SkFont big = a380::font("Digital", 256.f, false, true);
        SkFont small = a380::font("Digital", 137.5f, false, true);
        bool inhg = in.var(p + "DISPLAY_BARO_IS_INHG") != 0;
        bool std_ = in.var(p + "DISPLAY_BARO_IS_STD") != 0;
        double val = in.var(p + "DISPLAY_BARO_VALUE");
        int mode = (int)in.var(p + "DISPLAY_BARO_MODE");
        bool preset = in.var(p + "DISPLAY_BARO_PRESET_VISIBLE") != 0;
        char b[32];
        std::string v, pre;
        if (test_) v = "8.8.8.8";
        else if (std_) v = "Std";
        else if (!inhg) { std::snprintf(b, sizeof b, "%04d", (int)std::lround(val)); v = b; }
        else { std::snprintf(b, sizeof b, "%.2f", val); v = b; }
        if (test_) pre = "8.p.8.8";
        else if (!std_ && mode == 2) pre = "qfe";
        else if (!inhg) { std::snprintf(b, sizeof b, "%04d", (int)std::lround(val)); pre = b; }
        else { std::snprintf(b, sizeof b, "%.2f", val); pre = b; }
        if (test_ || mode == 1) text(c, "QNH", bx + 0.06f * bw, by + 0.23f * bh, label);
        if (test_ || preset) text(c, pre, bx + 0.97f * bw, by + 0.30f * bh, small, Align::Right);
        text(c, v, bx + 0.04f * bw, by + 0.95f * bh, big);
    }

    void nd(SkCanvas& c, const a380::Inputs& in, const std::string& p, int side) {
        float ox = side == 0 ? 120.f : 150.f, oy = side == 0 ? 875.f : 1450.f;
        static const float xs[5] = {0.f, 445.f, 920.f, 1395.f, 1870.f};
        static const char* top[5] = {"CSTR", "WPT", "VORD", "NDB", "ARPT"};
        static const char* bot[5] = {"", "WX", "TERR", "TRAF", ""};
        auto navaid = [&](int n) {
            int m = (int)in.var(p + "NAVAID_" + std::to_string(n) + "_MODE");
            static const char* kind[3] = {"POINTER", "VOR", "ADF"};
            return std::string(kind[std::clamp(m, 0, 2)]) + std::to_string(n);
        };
        for (int row = 0; row < 2; row++)
            for (int i = 0; i < 5; i++) {
                std::string name = test_ ? "TEST" : row == 0 ? top[i] : i == 0 ? navaid(1) : i == 4 ? navaid(2) : bot[i];
                auto img = load_image(name + ".png");
                if (!img) continue;
                float w = 310.f, h = 88.f;
SkRect dst = SkRect::MakeXYWH(ox + xs[i], oy + (row ? 275.f : 0.f), w, h);
                SkRect dev = c.getTotalMatrix().mapRect(dst);
                std::string key = name + std::to_string(dev.left()) + "," + std::to_string(dev.top());
                auto it = levels_.find(key);
                if (it == levels_.end())
                    it = levels_.emplace(key, box_resize(img, (int)std::floor(dev.width()) + 1, 44)).first;
                c.save();
                c.resetMatrix();
                c.drawImageRect(it->second, dev, SkSamplingOptions(SkFilterMode::kLinear));
                c.restore();
            }
    }
};

std::unique_ptr<a380::Display> make(std::string_view) { return std::make_unique<Fcu>(); }
A380_REGISTER_DISPLAY("fcu", make);

}  // namespace
