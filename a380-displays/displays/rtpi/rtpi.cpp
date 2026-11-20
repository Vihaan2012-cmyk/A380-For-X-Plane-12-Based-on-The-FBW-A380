#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#include "include/core/SkBitmap.h"
#include "include/core/SkImage.h"
#include "include/core/SkPaint.h"
#include "include/core/SkSamplingOptions.h"

#include "a380/display.h"
#include "a380/draw.h"
#include "a380/fonts.h"

namespace {

double phi(double z) { return 0.5 * std::erfc(-z / std::sqrt(2.0)); }

struct Arinc {
    int ssm = 0;
    uint32_t bits = 0;
    float f32() const {
        float f;
        std::memcpy(&f, &bits, 4);
        return f;
    }
    bool bit(int n) const {
        float f = f32();
        if (!std::isfinite(f) || std::fabs(f) >= 9.0e18f) return false;
        return ((uint32_t)(int64_t)std::trunc(f) >> (n - 1)) & 1;
    }
};

Arinc arinc(double raw) {
    Arinc a;
    double hi = std::floor(raw / 4294967296.0);
    a.ssm = int(hi) & 3;
    a.bits = uint32_t(uint64_t(raw - hi * 4294967296.0));
    return a;
}

void draw_panel(SkCanvas& canvas, float pw_css, int w, int h) {
    SkMatrix m = canvas.getTotalMatrix();
    float sc = std::max(1.f, m.getScaleX());
    int pw = (int)std::lround(w * sc), ph = (int)std::lround(h * sc);
    thread_local sk_sp<SkImage> cached;
    thread_local int cw = -1, ch = -1, cpw = -1, cph = -1;
    thread_local float csc = 0.f, cpcss = -1.f;
    if (cached && cw == w && ch == h && cpw == pw && cph == ph && csc == sc && cpcss == pw_css) {
        canvas.save();
        canvas.scale(1.f / sc, 1.f / sc);
        canvas.drawImage(cached, 0, 0, SkSamplingOptions());
        canvas.restore();
        return;
    }
    SkBitmap bm;
    bm.allocN32Pixels(pw, ph);
    const double sigma = 55.0, spread = 20.0;
    const double bg[3] = {9.0, 8.0, 1.0}, sh[3] = {255.0, 175.0, 0.0};
    std::vector<double> fx(pw), fy(ph);
    for (int i = 0; i < pw; i++) {
        double x = (i + 0.5) / sc;
        fx[i] = phi((x + spread) / sigma) - phi((x - (pw_css + spread)) / sigma);
    }
    for (int j = 0; j < ph; j++) {
        double y = (j + 0.5) / sc;
        fy[j] = phi((y + spread) / sigma) - phi((y - (h + spread)) / sigma);
    }
    for (int j = 0; j < ph; j++) {
        uint32_t* row = bm.getAddr32(0, j);
        for (int i = 0; i < pw; i++) {
            double a = 0.5 * (1.0 - fx[i] * fy[j]);
            uint8_t c[3];
            for (int k = 0; k < 3; k++) c[k] = (uint8_t)std::lround(bg[k] * (1 - a) + sh[k] * a);
            row[i] = SkColorSetARGB(255, c[0], c[1], c[2]);
        }
    }
    bm.setImmutable();
    cached = bm.asImage();
    cw = w, ch = h, cpw = pw, cph = ph, csc = sc, cpcss = pw_css;
    canvas.save();
    canvas.scale(1.f / sc, 1.f / sc);
    canvas.drawImage(cached, 0, 0, SkSamplingOptions());
    canvas.restore();
}

float a8_text(SkCanvas& canvas, const std::string& text, float x, float y, const SkFont& font, const SkPaint& paint,
              bool right, float ls) {
    std::vector<SkGlyphID> glyphs(text.size());
    size_t n = font.textToGlyphs(text.data(), text.size(), SkTextEncoding::kUTF8, SkSpan<SkGlyphID>(glyphs));
    glyphs.resize(n);
    std::vector<SkScalar> widths(n);
    font.getWidths(SkSpan<const SkGlyphID>(glyphs), SkSpan<SkScalar>(widths));
    float total = 0;
    for (size_t i = 0; i < n; i++) total += widths[i] + ls;
    float cx = right ? x - total : x;
    for (size_t i = 0; i < n; i++) {
        canvas.drawSimpleText(&glyphs[i], sizeof(SkGlyphID), SkTextEncoding::kGlyphID, cx, y, font, paint);
        cx += widths[i] + ls;
    }
    return total;
}

class Rtpi : public a380::Display {
public:
    SkISize size() const override { return {338, 128}; }

    void draw(SkCanvas& canvas, const a380::Inputs& in) override {
        if (in.var("L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED", "Boolean", 0) == 0) return;
        double lts = in.var("L:A32NX_OVHD_INTLT_ANN", "Bool", 1);
        Arinc st = arinc(in.var("L:A32NX_SEC_1_RUDDER_STATUS_WORD", "number", 0));
        int src = st.ssm >= 2 && st.bit(28) ? 1 : 3;
        Arinc pos = arinc(in.var("L:A32NX_SEC_" + std::to_string(src) + "_RUDDER_ACTUAL_POSITION", "number", 0));

        const float panel_w = 338.f * 1.05f;
        canvas.save();
        canvas.clipRect(SkRect::MakeWH(338.f, 128.f));
        SkPaint layer;
        layer.setAlphaf(0.5f);
        canvas.saveLayer(nullptr, &layer);
        draw_panel(canvas, panel_w, (int)std::ceil(panel_w), 128);
        if (pos.ssm != 0 || lts == 0) {
            float v = pos.f32();
            std::string dir = lts == 0 ? "T" : (v >= 0 ? "L" : "R");
            char b[32];
            std::snprintf(b, sizeof b, "%.1f", std::fabs(v));
            std::string val = lts == 0 ? "88.8" : b;
            SkFont f = a380::font("AirbusRTPI", 135.f, false, true);
            SkPaint p;
            p.setAntiAlias(true);
            p.setColor(a380::rgb(0xffaa00));
            float ox = 8.45f;
            a8_text(canvas, dir, ox, 110.f, f, p, false, 3.f);
            a8_text(canvas, val, ox + 330.f, 110.f, f, p, true, 3.f);
        }
        canvas.restore();
        canvas.restore();
    }
};

std::unique_ptr<a380::Display> make(std::string_view) { return std::make_unique<Rtpi>(); }
A380_REGISTER_DISPLAY("rtpi", make);

}  // namespace
