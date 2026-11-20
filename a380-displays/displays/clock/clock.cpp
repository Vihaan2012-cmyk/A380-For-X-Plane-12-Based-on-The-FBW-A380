#include <algorithm>
#include <cmath>
#include <cstdio>
#include <string>

#include "include/core/SkPaint.h"

#include "a380/display.h"
#include "a380/draw.h"
#include "a380/fonts.h"

namespace {

void a8(SkCanvas& c, const std::string& t, float x, float y, const SkFont& f, const SkPaint& p) {
    if (!t.empty()) c.drawSimpleText(t.data(), t.size(), SkTextEncoding::kUTF8, x, y, f, p);
}

std::string pad2(long long v) {
    char b[32];
    std::snprintf(b, sizeof b, "%02lld", v);
    return b;
}

class Clock : public a380::Display {
public:
    SkISize size() const override { return {256, 256}; }

    void draw(SkCanvas& canvas, const a380::Inputs& in) override {
        bool ess = in.var("L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED", "bool", 1) != 0;
        if (!ess) return;
        bool hot1 = in.var("L:A32NX_ELEC_DC_HOT_1_BUS_IS_POWERED", "bool", 1) != 0;
        bool dc2 = in.var("L:A32NX_ELEC_DC_2_BUS_IS_POWERED", "bool", 0) != 0;
        bool lts = in.var("L:A32NX_OVHD_INTLT_ANN", "number", 1) == 0 && dc2;
        double tod = in.var("E:TIME OF DAY", "enum", 0);
        bool day = tod == 1 || tod == 2;
        double knob = in.var("L:A32NX_CHRONO_ET_SWITCH_POS", "number", 0);

        SkPaint bg;
        bg.setColor(a380::rgb(0x020208));
        canvas.drawRect(SkRect::MakeWH(256, 256), bg);

        double now = in.now_ms();
        double target = day ? 217.0 : 102.0;
        if (alpha_to_ < 0) alpha_from_ = alpha_to_ = target;
        double k = std::clamp((now - alpha_at_) / 5000.0, 0.0, 1.0);
        double alpha = alpha_from_ + (alpha_to_ - alpha_from_) * k;
        if (target != alpha_to_) {
            alpha_from_ = alpha;
            alpha_to_ = target;
            alpha_at_ = now;
        }
        SkPaint p;
        p.setAntiAlias(true);
        p.setColor(SkColorSetARGB((uint8_t)std::lround(alpha), 220, 220, 220));
        SkFont big = a380::font("AirbusChronometer", 65.f, false, true);
        SkFont small = a380::font("AirbusChronometer", 48.f, false, true);

        std::string chrono;
        if (lts) {
            chrono = "88:88";
        } else {
            double et = in.var("L:A32NX_CHRONO_ELAPSED_TIME", "number", -1);
            if (et != last_chrono_) {
                if (et >= 0 && last_chrono_ >= 0) chrono_change_ms_ = now;
                last_chrono_ = et;
            }
            if (et >= 0) {
                bool running = now - chrono_change_ms_ < 1000.0;
                double e = std::fmod(et, 99 * 60 + 59 + 1);
                chrono = pad2((long long)(e / 60)) + (running ? ":" : " ") + pad2((long long)std::fmod(e, 60));
            }
        }
        a8(canvas, chrono, 47.f, 60.f, big, p);

        std::string cb, cs;
        if (!hot1) {
        } else if (lts) {
            cb = "88:88";
            cs = "88";
        } else if (in.var("L:A32NX_CLOCK_DATE_MODE", "number", 0) != 0) {
            cb = pad2((long long)in.var("E:ZULU MONTH OF YEAR", "number", 1)) + ":" +
                 pad2((long long)in.var("E:ZULU DAY OF MONTH", "number", 1));
            std::string year = std::to_string((long long)in.var("E:ZULU YEAR", "number", 1970));
            cs = year.size() >= 4 ? year.substr(2, 2) : "";
        } else {
            double utc = in.var("E:ZULU TIME", "number", 0);
            long long s = (long long)std::floor(std::round(utc));
            cb = pad2(s / 3600) + ":" + pad2((s % 3600) / 60);
            cs = pad2(s % 60);
        }
        a8(canvas, cb, 6.f, 153.f, big, p);
        a380::draw_text(canvas, cs, 190.f, 147.f, small, p);

        std::string et;
        if (lts) {
            et = "88:88";
        } else {
            double fallback = knob == 0 ? 0 : -1;
            if (knob == 0 && in.has_env_number("connectedAt.perf") && in.has_env_number("frozenAt.perf"))
                fallback = std::max(0.0, (in.env_number("frozenAt.perf") - in.env_number("connectedAt.perf")) / 1000.0);
            double v = in.var("L:A32NX_CHRONO_ET_ELAPSED_TIME", "number", fallback);
            if (v >= 0) {
                double m = std::min(v, 99.0 * 3600 + 59 * 60);
                et = pad2((long long)(m / 3600)) + (knob == 0 ? ":" : " ") + pad2((long long)(std::fmod(m, 3600) / 60));
            }
        }
        a8(canvas, et, 47.f, 247.f, big, p);
    }

private:
    double alpha_from_ = 0.0, alpha_to_ = -1.0, alpha_at_ = 0.0;
    double last_chrono_ = -1.0, chrono_change_ms_ = -1e9;
};

std::unique_ptr<a380::Display> make(std::string_view) { return std::make_unique<Clock>(); }
A380_REGISTER_DISPLAY("clock", make);

}  // namespace
