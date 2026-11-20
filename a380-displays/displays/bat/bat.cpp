#include <algorithm>
#include <cmath>
#include <cstdio>
#include <string>

#include "include/core/SkBitmap.h"
#include "include/core/SkImage.h"
#include "include/core/SkPaint.h"
#include "include/core/SkSamplingOptions.h"

#include "a380/display.h"
#include "a380/draw.h"
#include "a380/fonts.h"

namespace {

using a380::rgb;

double phi(double z) { return 0.5 * std::erfc(-z / std::sqrt(2.0)); }

void draw_blue_panel(SkCanvas& canvas, int w, int h) {
    SkMatrix m = canvas.getTotalMatrix();
    float sc = std::max(1.f, m.getScaleX());
    int pw = (int)std::lround(w * sc), ph = (int)std::lround(h * sc);
    thread_local sk_sp<SkImage> cached;
    thread_local int cw = -1, ch = -1, cpw = -1, cph = -1;
    thread_local float csc = 0.f;
    if (cached && cw == w && ch == h && cpw == pw && cph == ph && csc == sc) {
        canvas.save();
        canvas.scale(1.f / sc, 1.f / sc);
        canvas.drawImage(cached, 0, 0, SkSamplingOptions());
        canvas.restore();
        return;
    }
    SkBitmap bm;
    bm.allocN32Pixels(pw, ph);
    const double sigma = 55.0, spread = 20.0;
    const double r = 7.0 / 255, g = 7.0 / 255, b = 237.0 / 255;
    std::vector<double> fx(pw), fy(ph);
    for (int i = 0; i < pw; i++) {
        double x = (i + 0.5) / sc;
        fx[i] = phi((x + spread) / sigma) - phi((x - (w + spread)) / sigma);
    }
    for (int j = 0; j < ph; j++) {
        double y = (j + 0.5) / sc;
        fy[j] = phi((y + spread) / sigma) - phi((y - (h + spread)) / sigma);
    }
    for (int j = 0; j < ph; j++) {
        uint32_t* row = bm.getAddr32(0, j);
        for (int i = 0; i < pw; i++) {
            double k = fx[i] * fy[j];
            row[i] = SkColorSetARGB(255, (uint8_t)std::lround(r * k * 255), (uint8_t)std::lround(g * k * 255),
                                  (uint8_t)std::lround(b * k * 255));
        }
    }
    bm.setImmutable();
    cached = bm.asImage();
    cw = w, ch = h, cpw = pw, cph = ph, csc = sc;
    SkCanvas* c = &canvas;
    c->save();
    c->scale(1.f / sc, 1.f / sc);
    c->drawImage(cached, 0, 0, SkSamplingOptions());
    c->restore();
}

class Bat : public a380::Display {
public:
    SkISize size() const override { return {256, 128}; }

    void draw(SkCanvas& canvas, const a380::Inputs& in) override {
        int sel = (int)in.var("L:A380X_OVHD_ELEC_BAT_SELECTOR_KNOB", "Number", 0);
        if (sel < 0 || sel > 4) sel = 2;
        static const int map[] = {3, 4, 0, 1, 2};
        int bat = map[sel];

        double lts = in.var("L:A32NX_OVHD_INTLT_ANN", "Bool", 1);
        double dc2 = in.var("L:A32NX_ELEC_DC_2_BUS_IS_POWERED", "Bool", 0);
        double volts = in.var("L:A32NX_ELEC_BAT_" + std::to_string(bat) + "_POTENTIAL", "Volts", 0);
        std::string d;
        if (lts == 0 && dc2 != 0) {
            d = "888";
        } else {
            char b[32];
            std::snprintf(b, sizeof b, "%.1f", volts);
            double v = std::atof(b);
            int ip = (int)std::floor(v);
            char t[32];
            std::snprintf(t, sizeof t, "%.1f", v - ip);
            std::string ints = std::to_string(ip);
            while (ints.size() < 2) ints = "0" + ints;
            d = ints + std::string(1, t[2]);
        }
        if (in.has_var("E:ABSOLUTE TIME")) d = refresh(in.now_ms(), d, bat);
        if (bat == 0) return;
        std::string text = std::string(1, d[0]) + d[1] + "." + d[2] + "V";

        SkPaint layer;
        layer.setAlphaf(0.5f);
        canvas.saveLayer(nullptr, &layer);
        draw_blue_panel(canvas, 256, 128);
        const float s = 1.28f;
        SkFont f = a380::font("Digital", 56.f * s, true);
        SkPaint p;
        p.setAntiAlias(true);
        p.setColor(SK_ColorWHITE);
        a380::draw_text(canvas, text, 196.f * s, 48.f * s, f, p, a380::Align::Right, a380::Baseline::Alphabetic,
                        0.1f * 56.f * s);
        canvas.restore();
    }

private:
    std::string digits_ = "   ";
    double timer_ = -1.0, prev_ms_ = -1.0;
    int updated_ = 0;

    std::string refresh(double now, const std::string& target, int bat) {
        if (timer_ < 0) timer_ = bat == 1 ? 0.0 : 200.0;
        if (prev_ms_ >= 0) timer_ += std::max(0.0, now - prev_ms_);
        prev_ms_ = now;
        for (int i = 0; i < 3; i++) {
            if (timer_ >= 1000.0 + 130.0 * i && updated_ < i + 1) {
                digits_[i] = target[i];
                updated_ = i + 1;
            }
        }
        while (timer_ >= 1260.0) {
            timer_ -= 1260.0;
            updated_ = 0;
        }
        return digits_;
    }
};

std::unique_ptr<a380::Display> make(std::string_view) { return std::make_unique<Bat>(); }
A380_REGISTER_DISPLAY("bat", make);

}  // namespace
