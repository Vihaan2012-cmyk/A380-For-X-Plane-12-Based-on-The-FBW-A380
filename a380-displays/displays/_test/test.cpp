#include <cstdio>
#include "a380/env.h"
#include <cstdlib>

#include "a380/display.h"
#include "a380/draw.h"
#include "a380/fonts.h"

namespace {

class TestDisplay : public a380::Display {
public:
    SkISize size() const override { return {768, 1024}; }

    void draw(SkCanvas& canvas, const a380::Inputs& in) override {
        if (const char* name = a380::getenv_cached("A380_TEST_VAR")) {
            std::fprintf(stderr, "var %s = %g (has=%d) now_ms=%g dpr=%g\n", name, in.var(name), (int)in.has_var(name),
                         in.now_ms(), in.device_pixel_ratio());
            if (const char* b = a380::getenv_cached("A380_TEST_BUS")) {
                const auto* j = in.bus(b);
                std::fprintf(stderr, "bus %s = %s\n", b, j ? j->dump().substr(0, 200).c_str() : "(none)");
            }
            if (const char* g = a380::getenv_cached("A380_TEST_GENERIC")) {
                const auto* j = in.generic(g);
                std::fprintf(stderr, "generic %s = %s\n", g, j ? j->dump().substr(0, 200).c_str() : "(none)");
            }
            if (const char* c = a380::getenv_cached("A380_TEST_CALL")) {
                const auto* j = in.call_result(c);
                std::fprintf(stderr, "call %s = %s\n", c, j ? j->dump().substr(0, 200).c_str() : "(none)");
            }
        }
        if (const char* text = a380::getenv_cached("A380_TEST_TEXT")) {
            auto num = [](const char* k, float d) {
                const char* v = a380::getenv_cached(k);
                return v ? (float)std::atof(v) : d;
            };
            const char* fam = a380::getenv_cached("A380_TEST_FONT");
            SkPaint p;
            p.setAntiAlias(true);
            p.setColor(a380::rgb((uint32_t)std::strtoul(a380::getenv_cached("A380_TEST_COLOR") ? a380::getenv_cached("A380_TEST_COLOR") : "00ff00", nullptr, 16)));
            SkFont f = a380::font(fam ? fam : "Ecam", num("A380_TEST_PX", 25.f));
            a380::draw_text(canvas, text, num("A380_TEST_X", 20.f), num("A380_TEST_Y", 50.f), f, p, a380::Align::Left,
                            a380::Baseline::Alphabetic, num("A380_TEST_LS", 0.f));
            return;
        }
        SkPaint green;
        green.setAntiAlias(true);
        green.setColor(a380::rgb(0x00ff00));
        SkPaint white;
        white.setAntiAlias(true);
        white.setColor(SK_ColorWHITE);
        SkPaint cyan;
        cyan.setAntiAlias(true);
        cyan.setColor(a380::rgb(0x00ffff));

        SkFont ecam = a380::font("Ecam", 32.f);
        SkFont ecam_slashed = a380::font("FBW-Display-EIS-A380-SlashedZero", 32.f);
        SkFont inter = a380::font("Inter", 24.f);
        SkFont mono = a380::font("monospace", 20.f);

        a380::draw_text(canvas, "ECAM 0123456789 NM KT", 20.f, 60.f, ecam, green);
        a380::draw_text(canvas, "SLASHED 0123456789", 20.f, 110.f, ecam_slashed, white);
        a380::draw_text(canvas, "Inter regular text", 20.f, 160.f, inter, white);
        a380::draw_text(canvas, "monospace fallback", 20.f, 200.f, mono, cyan);
        a380::draw_text(canvas, "RIGHT", 748.f, 260.f, ecam, green, a380::Align::Right);
        a380::draw_text(canvas, "CENTER", 384.f, 300.f, ecam, green, a380::Align::Center, a380::Baseline::Middle);
        a380::stroke_rrect_inside(canvas, SkRect::MakeXYWH(20.f, 340.f, 200.f, 80.f), 8.f, 3.f, a380::rgb(0xffffff));
        a380::draw_text(canvas, "now " + std::to_string((long long)in.now_ms()), 20.f, 480.f, inter, white);
    }
};

std::unique_ptr<a380::Display> make_test(std::string_view) { return std::make_unique<TestDisplay>(); }

A380_REGISTER_DISPLAY("_test", make_test);

}  // namespace
