#include <algorithm>
#include "a380/shape_cache.h"
#include <cctype>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <string>

#include "include/core/SkPaint.h"
#include "include/core/SkPath.h"
#include "include/core/SkPathBuilder.h"

#include "a380/display.h"
#include "a380/draw.h"
#include "a380/fonts.h"

namespace {

using a380::Align;
using a380::Baseline;
using a380::rgb;

constexpr uint32_t kWhite = 0xffffff, kYellow = 0xffff00, kCyan = 0x00ffff, kGreen = 0x00ff00, kSky = 0x0698ff,
                   kEarth = 0x9c480c, kBlack = 0x000000, kAmber = 0xe68000, kMagenta = 0xff94ff, kRed = 0xff0000;

SkPaint fill(uint32_t c) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(rgb(c));
    return p;
}

SkPaint stroke(uint32_t c, float w) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setStyle(SkPaint::kStroke_Style);
    p.setStrokeWidth(w);
    p.setColor(rgb(c));
    return p;
}

void line(SkCanvas& c, float x1, float y1, float x2, float y2, const SkPaint& p) { c.drawLine(x1, y1, x2, y2, p); }

SkPath poly(std::initializer_list<SkPoint> pts) {
    SkPathBuilder b;
    bool first = true;
    for (auto& p : pts) {
        if (first) b.moveTo(p);
        else b.lineTo(p);
        first = false;
    }
    b.close();
    return b.detach();
}

double js_round(double v) { return std::floor(v + 0.5); }

SkPath svg_path(const char* d) {
    SkPathBuilder b;
    const char* p = d;
    char cmd = 0;
    float x = 0, y = 0;
    auto skip = [&] {
        while (*p == ' ' || *p == ',') p++;
    };
    auto num = [&] {
        char* e;
        float v = std::strtof(p, &e);
        if (e == p) e++;
        p = e;
        return v;
    };
    while (true) {
        skip();
        if (!*p) break;
        if (std::isalpha((unsigned char)*p)) {
            cmd = *p++;
            if (cmd == 'z' || cmd == 'Z') b.close();
            continue;
        }
        switch (cmd) {
            case 'M':
                x = num();
                skip();
                y = num();
                b.moveTo(x, y);
                cmd = 'L';
                break;
            case 'L':
                x = num();
                skip();
                y = num();
                b.lineTo(x, y);
                break;
            case 'l': {
                float dx = num();
                skip();
                float dy = num();
                x += dx;
                y += dy;
                b.lineTo(x, y);
                break;
            }
            case 'h':
                x += num();
                b.lineTo(x, y);
                break;
            case 'v':
                y += num();
                b.lineTo(x, y);
                break;
            default:
                return b.detach();
        }
    }
    return b.detach();
}

struct Bug {
    bool active;
    double value;
};

Bug read_bug(const a380::Inputs& in, int index) {
    std::string kind = index < 2 ? "ALT" : "SPD";
    std::string n = std::to_string(index < 2 ? index : index - 2);
    return {in.var("L:A32NX_ISIS_BUGS_" + kind + "_ACTIVE:" + n, "boolean", 0) != 0,
            in.var("L:A32NX_ISIS_BUGS_" + kind + "_VALUE:" + n, "knots", 0)};
}

double correct_loc_error(double e) {
    double n = std::fmod(std::fmod(e, 360.0) + 360.0, 360.0);
    if (n >= 180) n -= 360;
    if (n < -90) return -180 - n;
    if (n > 90) return 180 - n;
    return n;
}

void ls_indicator(SkCanvas& canvas, double deviation, bool available, float sw) {
    double dots = deviation / 0.4;
    bool below = dots < -2, above = dots > 2;
    canvas.drawRect(SkRect::MakeXYWH(0, 0, 178.26f, 18.326f), fill(kBlack));
    SkPaint ring = stroke(kWhite, sw);
    for (float cx : {14.161f, 51.646f, 126.616f, 164.101f}) canvas.drawCircle(cx, 9.1625f, 2.9155f, ring);
    if (available) {
        if (!(above || below)) {
            canvas.save();
            canvas.translate((float)(dots * 37.485), 0.f);
            a380::draw_path_cached(canvas, svg_path("M 75.6 9.1 v 0.8 l 8.3443 6.7 h 10.0132 l 8.3443 -6.7 v -1.6 l -8.3443 -6.7 h -10.0132 l -8.3443 6.7 z"),
                            fill(kMagenta));
            canvas.restore();
        } else if (below) {
            a380::draw_path_cached(canvas, svg_path("M 0.75 9.1 v 0.8 l 8.3443 6.7 h 5.0066 v -15 h -5.0066 l -8.3443 6.7 z"), fill(kMagenta));
        } else {
            a380::draw_path_cached(canvas, svg_path("M 177.5 9.1 v 0.8 l -8.3443 6.7 h 5.0066 v -15 h -5.0066 l 8.3443 6.7 z"), fill(kMagenta));
        }
    }
    line(canvas, 89.131f, 2.499f, 89.131f, 15.827f, stroke(kYellow, sw));
}

void bug_box(SkCanvas& canvas, float x, float y, double value, bool speed) {
    float w = speed ? 75.f : 108.f;
    SkRect r = SkRect::MakeXYWH(x - w / 2, y - 34, w, 40);
    canvas.drawRect(r, fill(kBlack));
    canvas.drawRect(r, stroke(kWhite, 2.f));
    char b[32];
    std::snprintf(b, sizeof b, "%0*ld", speed ? 3 : 5, (long)value);
    a380::draw_text(canvas, b, x, y, a380::font("ISISFont", 32.f), fill(kWhite), Align::Center);
}

void draw_bug_setup(SkCanvas& canvas, const a380::Inputs& in) {
    SkFont f = a380::font("ISISFont", 32.f);
    SkFont sm = a380::font("ISISFont", 16.f);
    SkPaint white = fill(kWhite), cyan = fill(kCyan);
    a380::draw_text(canvas, "EXIT\xe2\x86\x91", 60.f, 60.f, f, cyan);
    a380::draw_text(canvas, "BUGS", 256.f, 60.f, f, white, Align::Center);
    a380::draw_text(canvas, "SPD", 150.f, 100.f, f, white, Align::Center);
    a380::draw_text(canvas, "ALT", 335.f, 100.f, f, white, Align::Center);
    a380::draw_text(canvas, "(+)", 196.f, 126.f, sm, white);
    a380::draw_text(canvas, "(-)", 110.f, 180.f, sm, white);
    a380::draw_path_cached(canvas, svg_path("M150 136 v225 h195 v-225z"), stroke(kWhite, 2.f));
    struct Slot {
        int bug;
        float x, y, off_x;
    };
    for (const Slot& s : {Slot{5, 150, 150, 35}, Slot{4, 150, 225, 35}, Slot{3, 150, 300, 35}, Slot{2, 150, 375, 35},
                          Slot{0, 344, 225, 409}, Slot{1, 344, 300, 409}}) {
        Bug b = read_bug(in, s.bug);
        if (!b.active) a380::draw_text(canvas, "OFF", s.off_x, s.y, f, white);
        bug_box(canvas, s.x, s.y, b.value, s.bug >= 2);
    }
    a380::draw_text(canvas, "SET/SELECT\xe2\x86\x92", 170.f, 450.f, f, cyan);
}

void draw_baro(SkCanvas& canvas, const a380::Inputs& in) {
    double baro_mode = in.var("L:A32NX_ISIS_BARO_MODE", "enum", 0);
    double hpa = in.var("A:KOHLSMAN SETTING MB:3", "millibars", 0);
    std::string text;
    float px = 256.f, fs = 28.f;
    const std::string nb = " ";
    if (baro_mode == 1) {
        text = "STD";
        px = 250.f;
        fs = 36.f;
    } else {
        char b[16];
        std::snprintf(b, sizeof b, "%ld", (long)std::lround(hpa));
        std::string s = b;
        while (s.size() < 4) s = nb + s;
        text = s + nb + nb + nb + nb;
    }
    a380::draw_text(canvas, text, px, 450.f, a380::font("ISISFont", fs), fill(kCyan), Align::Center);
}

void draw_dim(SkCanvas& canvas, const a380::Inputs& in) {
    double raw = in.var("GLASSCOCKPIT AUTOMATIC BRIGHTNESS", "number", 0);
    if (raw > 1) raw /= 100;
    double autob = 0.2 + (0.99 - 0.2) * std::clamp(raw, 0.0, 1.0);
    double off = std::clamp(in.var("L:A32NX_ISIS_MANUAL_BRIGHTNESS_OFFSET", "number", 0), 0.03 - autob, 0.99 - autob);
    double target = std::clamp(autob + off, 0.03, 0.99);
    SkPaint dim;
    dim.setColor(SkColorSetARGB((uint8_t)std::lround((1 - target) * 255), 0, 0, 0));
    canvas.drawRect(SkRect::MakeWH(512, 512), dim);
}

void draw_selftest(SkCanvas& canvas, const a380::Inputs& in, double timer) {
    SkFont f = a380::font("Ecam", 30.f);
    SkPaint black = fill(kBlack);
    struct Flag {
        float x, y, w;
        const char* label;
        float tx, ty;
    };
    for (const Flag& g : {Flag{190, 174, 110, "ATT", 245, 206}, Flag{40, 244, 84, "SPD", 82, 277},
                          Flag{358, 244, 110, "ALT", 415, 277}}) {
        canvas.drawRect(SkRect::MakeXYWH(g.x, g.y, g.w, 40), fill(kYellow));
        a380::draw_text(canvas, g.label, g.tx, g.ty, f, black, Align::Center);
    }
    canvas.drawRect(SkRect::MakeXYWH(150, 332, 180, 40), fill(kYellow));
    a380::draw_text(canvas, "INIT", 160, 365, f, black, Align::Left);
    char b[16];
    std::snprintf(b, sizeof b, "%ds", (int)std::max(0.0, std::ceil(timer)));
    a380::draw_text(canvas, b, 325, 365, f, black, Align::Right);
    draw_baro(canvas, in);
    draw_dim(canvas, in);
}

class Isis : public a380::Display {
public:
    SkISize size() const override { return {512, 512}; }

    void draw(SkCanvas& canvas, const a380::Inputs& in) override {
        canvas.drawRect(SkRect::MakeWH(512, 512), fill(kBlack));

        double ias = in.var("AIRSPEED INDICATED", "knots", 0);
        bool ess = in.var("L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED", "bool", 0) != 0;
        bool hot = in.var("L:A32NX_ELEC_DC_HOT_1_BUS_IS_POWERED", "bool", 0) != 0;
        bool powered = (ias > 50 && hot) || ess;
        double now = in.now_ms();
        if (!started_) {
            started_ = true;
            state_ = in.var("L:A32NX_COLD_AND_DARK_SPAWN", "Bool", 0) != 0 ? Unit::Off : Unit::Standby;
            if (state_ == Unit::Off && powered && in.has_env_number("frozenAt.perf")) {
                double since = in.env_number("frozenAt.perf") - in.env_number("connectedAt.perf");
                state_ = since >= kPowerUpMs ? Unit::On : Unit::Selftest;
                deadline_ms_ = state_ == Unit::Selftest ? now + kPowerUpMs - since : -1.0;
            }
        }
        if (deadline_ms_ >= 0 && now >= deadline_ms_) {
            deadline_ms_ = -1.0;
            if (state_ == Unit::Standby) state_ = Unit::Off;
            else if (state_ == Unit::Selftest) state_ = Unit::On;
        }
        if (state_ == Unit::On && !powered) {
            state_ = Unit::Standby;
            deadline_ms_ = now + 10000.0;
        } else if (state_ == Unit::Standby && powered) {
            state_ = Unit::On;
            deadline_ms_ = -1.0;
        } else if (state_ == Unit::Off && powered) {
            state_ = Unit::Selftest;
            deadline_ms_ = now + kPowerUpMs;
        } else if (state_ == Unit::Selftest && !powered) {
            state_ = Unit::Off;
            deadline_ms_ = -1.0;
        }
        if (state_ == Unit::Selftest) {
            draw_selftest(canvas, in, (deadline_ms_ - now) / 1000.0);
            return;
        }
        if (state_ == Unit::Off) return;

        if (in.var("L:A32NX_ISIS_BUGS_ACTIVE", "Boolean", 0) != 0) {
            draw_bug_setup(canvas, in);
            draw_dim(canvas, in);
            return;
        }

        double pitch = in.var("PLANE PITCH DEGREES", "degrees", 0);
        double roll = in.var("PLANE BANK DEGREES", "degrees", 0);
        double alt = std::floor(in.var("INDICATED ALTITUDE:3", "feet", 0));
        double mach = in.var("AIRSPEED MACH", "mach", 0);
        double baro_mode = in.var("L:A32NX_ISIS_BARO_MODE", "enum", 0);
        double hpa = in.var("A:KOHLSMAN SETTING MB:3", "millibars", 0);
        double mda = in.var("L:AIRLINER_MINIMUM_DESCENT_ALTITUDE", "feet", 0);
        Bug bugs[6];
        for (int i = 0; i < 6; i++) bugs[i] = read_bug(in, i);

        if (mach > 0.5 && !mach_shown_) mach_shown_ = true;
        else if (mach < 0.45 && mach_shown_) mach_shown_ = false;

        {
            double acc = std::clamp(std::round(in.var("ACCELERATION BODY X", "G Force", 0) * 100) / 100, -0.3, 0.3);
            att_moved_ = att_moved_ || pitch != last_pitch_ || roll != last_roll_;
            last_pitch_ = pitch;
            last_roll_ = roll;
            if (ss_at_ < 0 || now - ss_at_ >= (att_moved_ ? 200.0 : 500.0)) {
                double input = -acc * 30.0 / 0.3;
                ss_out_ = (input + ss_in_) * 0.4 / 2.4 + (1.6 / 2.4) * ss_out_;
                ss_in_ = input;
                ss_at_ = now;
                att_moved_ = false;
            }
        }

        canvas.save();
        canvas.scale(1.1f, 1.1f);
        canvas.translate(-30.f, -30.f);
        const float stroke_w = 3.332f;
        const float pdp = 6.7f;

        canvas.save();
        canvas.rotate((float)roll, 256.f, 256.f);
        canvas.save();
        canvas.translate(0.f, (float)(-pitch * pdp));
        canvas.drawRect(SkRect::MakeXYWH(-256, -498, 1024, 768), fill(kSky));
        canvas.drawRect(SkRect::MakeXYWH(-256, 270, 1024, 768), fill(kEarth));
        {
            SkPaint sw = stroke(kWhite, stroke_w);
            SkPaint tw = fill(kWhite);
            SkFont f = a380::font("ISISFont", 28.f);
            const float center = 270.f;
            for (int d = -30; d <= 30; d += 10) {
                float y = center - d * pdp;
                if (d == 0) {
                    line(canvas, -256, y, 768, y, sw);
                    continue;
                }
                char b[8];
                std::snprintf(b, sizeof b, "%d", std::abs(d));
                a380::draw_text(canvas, b, 225.f, y, f, tw, Align::Right, Baseline::Middle);
                line(canvas, 231.843f, y, 280.157f, y, sw);
            }
            for (int d = -25; d <= 25; d += 10) {
                float y = center - d * pdp;
                line(canvas, 243.5f, y, 268.5f, y, sw);
            }
            for (float d = -27.5f; d <= 27.5f; d += 5.f) {
                float y = center - d * pdp;
                line(canvas, 248.5f, y, 263.5f, y, sw);
            }
        }
        {
            SkPaint red = stroke(kRed, 2.f);
            for (const char* d : {"M198 -258 h 46 v 12 h -16 l 28 28 l 28 -28 h -16 v -12 h 46 l -58 58z",
                                  "M216 -162 h 16 l 24 42 l 24 -42 h 16 l -40 70z",
                                  "M220 -66 h12 l24 42 l24 -42 h12 l-35.5 62z",
                                  "M220 578 h12 l24 -42 l24 42 h12 l-35.5 -62z",
                                  "M216 674 h 16 l 24 -42 l 24 42 h 16 l -40 -70z",
                                  "M198 770 h 46 v -12 h -16 l 28 -28 l 28 28 h -16 v 12 h 46 l -58 -58z"})
                a380::draw_path_cached(canvas, svg_path(d), red);
        }
        canvas.restore();

        canvas.drawRect(SkRect::MakeXYWH(-256, -256, 1024, 409), fill(kSky));
        {
            SkPath tri = poly({{256, 131}, {266, 149}, {246, 149}});
            a380::draw_path_cached(canvas, tri, fill(kBlack));
            a380::draw_path_cached(canvas, tri, stroke(kWhite, stroke_w));
            SkPath ss = poly({{246, 149}, {266, 149}, {271, 158}, {241, 158}});
            canvas.save();
            canvas.translate((float)ss_out_, 0.f);
            a380::draw_path_cached(canvas, ss, fill(kBlack));
            a380::draw_path_cached(canvas, ss, stroke(kWhite, stroke_w));
            canvas.restore();
        }
        canvas.drawRect(SkRect::MakeXYWH(-256, 388, 1024, 396), fill(kEarth));
        canvas.restore();

        {
            SkPaint wl = stroke(kWhite, 3.f);
            wl.setStrokeCap(SkPaint::kSquare_Cap);
            const float R = 138.f;
            float dx = R * std::sin(30.f * 3.14159265f / 180.f), dy = R * std::cos(30.f * 3.14159265f / 180.f);
            SkPathBuilder b;
            b.moveTo(256 - dx, 269 - dy);
            b.arcTo(SkPoint{R, R}, 0, SkPathBuilder::kSmall_ArcSize, SkPathDirection::kCW, SkPoint{256 + dx, 269 - dy});
            a380::draw_path_cached(canvas, b.detach(), wl);
            SkPaint tick = stroke(kWhite, 3.f);
            for (int a : {-60, -45, -30, -20, -10, 10, 20, 30, 45, 60}) {
                float s = std::sin(a * 3.14159265f / 180.f), c = std::cos(a * 3.14159265f / 180.f);
                float x = 256 + R * s, y = 269 - R * c;
                line(canvas, x, y, x + 20 * s, y - 20 * c, tick);
            }
            a380::draw_path_cached(canvas, poly({{256, 130}, {241, 108}, {271, 108}}), fill(kYellow));
        }

        {
            SkPathBuilder b;
            b.setFillType(SkPathFillType::kEvenOdd);
            b.moveTo(0, 0);
            b.lineTo(0, 512);
            b.lineTo(512, 512);
            b.lineTo(512, 0);
            b.close();
            b.moveTo(122.1f, 123.5f);
            b.cubicTo(122.1f + 43.9f, 123.5f - 23.5f, 122.1f + 217.9f, 123.5f - 25.5f, 122.1f + 271.4f, 123.5f);
            b.lineTo(393.5f, 123.5f + 266.f);
            b.cubicTo(393.5f - 43.9f, 389.5f + 23.5f, 393.5f - 217.9f, 389.5f + 25.5f, 393.5f - 271.4f, 389.5f);
            b.close();
            a380::draw_path_cached(canvas, b.detach(), fill(kBlack));
        }

        {
            SkFont f = a380::font("ISISFont", 30.f);
            SkPaint tw = fill(kWhite);
            canvas.save();
            canvas.clipRect(SkRect::MakeXYWH(12, 112, 108, 296));
            canvas.translate(12.f, 112.f);
            double clamped = std::max(std::min(ias, 660.0), 30.0);
            double lowest = std::max(std::round((clamped - 55) / 5) * 5, 30.0);
            if (lowest < ias - 55) lowest += 5;
            canvas.translate(0.f, (float)(clamped * 16.5 / 5));
            for (int i = 0; i < 22; i++) {
                double v = lowest + i * 5;
                if (v > 660) break;
                float off = (float)(-v * 16.5 / 5);
                long iv = (long)v;
                if (v > 250 && iv % 10 == 5) continue;
                float tl = iv % 20 == 10 ? 13.5f : 5.f;
                if (iv % 20 == 0) {
                    char b[16];
                    std::snprintf(b, sizeof b, "%ld", iv);
                    a380::draw_text(canvas, b, 104.f, off + 170.f, f, tw, Align::Right);
                }
                line(canvas, 108 - tl, off + 158, 108, off + 158, stroke(kWhite, 4.f));
            }
            for (int i = 2; i < 6; i++) {
                if (!bugs[i].active) continue;
                float off = (float)(-bugs[i].value * 16.5 / 5);
                line(canvas, 86, off + 158, 108, off + 158, stroke(kCyan, 7.f));
            }
            canvas.restore();
            canvas.drawRect(SkRect::MakeXYWH(121.887f, 250.841f, 23.324f, 35.4025f), fill(kBlack));
            a380::draw_path_cached(canvas, poly({{143.545f, 285.827f}, {143.545f, 254.173f}, {123.553f, 270.f}}), fill(kYellow));
        }

        {
            SkFont f = a380::font("ISISFont", 30.f);
            SkPaint tw = fill(kWhite);
            canvas.save();
            canvas.clipRect(SkRect::MakeXYWH(395, 112, 108, 296));
            canvas.translate(395.f, 112.f);
            double clamped = std::max(std::min(alt, 50000.0), -2000.0);
            double lowest = std::max(std::round((clamped - 1000) / 100) * 100, -2000.0);
            if (lowest < alt - 1000) lowest += 100;
            canvas.translate(0.f, (float)(clamped * 21.5 / 100));
            for (int i = 0; i < 20; i++) {
                double v = lowest + i * 100;
                if (v > 50000) break;
                float off = (float)(-v * 21.5 / 100);
                bool txt = std::fmod(v, 500.0) == 0;
                float tl = txt ? 5.f : 19.f;
                float ls = txt ? 0.5f : 0.f;
                line(canvas, ls, off + 158, ls + tl, off + 158, stroke(kWhite, txt ? 6.f : stroke_w));
                if (txt) {
                    char b[16];
                    std::snprintf(b, sizeof b, "%05ld", (long)std::fabs(v));
                    b[3] = 0;
                    a380::draw_text(canvas, b, 7.f, off + 170.f, f, tw);
                }
            }
            for (int i = 0; i < 2; i++) {
                if (!bugs[i].active) continue;
                canvas.save();
                canvas.translate(0.f, (float)(-bugs[i].value * 21.5 / 100));
                if (std::fmod(bugs[i].value, 500.0) == 0) {
                    a380::draw_path_cached(canvas, svg_path("M3,151 v-9 h65 v32 h-65 v-9"), stroke(kCyan, stroke_w));
                    a380::draw_path_cached(canvas, poly({{0, 150}, {10, 142}, {0, 142}}), fill(kCyan));
                    a380::draw_path_cached(canvas, poly({{0, 165}, {10, 173}, {0, 173}}), fill(kCyan));
                } else {
                    line(canvas, 0, 158, 25, 158, stroke(kCyan, 5.f));
                }
                canvas.restore();
            }
            canvas.restore();
        }

        {
            SkPath box = poly({{466.749f, 243.344f}, {417.602f, 243.344f}, {417.602f, 251.674f}, {340.966f, 251.674f},
                               {340.966f, 288.326f}, {417.602f, 288.326f}, {417.602f, 296.656f}, {466.749f, 296.656f}});
            a380::draw_path_cached(canvas, box, fill(kBlack));
            uint32_t col = mda != 0 && alt < mda ? kAmber : kGreen;
            double ca = std::max(std::min(alt, 50000.0), -1500.0);
            double tens = std::fmod(ca, 100.0);
            double hv = std::trunc(std::fmod(ca / 100, 10.0));
            double hpos = 0;
            if (std::fabs(tens) > 80) hpos = alt < 0 ? tens / 20 + 4 : tens / 20 - 4;
            double tv = std::trunc(std::fmod(ca / 1000, 10.0));
            double tpos = std::fabs(hv) >= 9 ? hpos : 0;
            double ttv = std::trunc(std::fmod(ca / 10000, 10.0));
            double ttpos = std::fabs(tv) >= 9 ? tpos : 0;
            SkFont big = a380::font("ISISFont", 36.f);
            auto drum1 = [&](double value, double pos, float x, bool showZero, bool mirror) {
                double hp = js_round(pos + 1);
                if (hp > pos + 1) hp -= 1;
                if (hp == 0) hp += 1;
                double hval = js_round(value + 1);
                if (hval > value + 1) hval -= 1;
                bool neg = std::signbit(value);
                if (!mirror) hval += neg ? -100 : 100;
                for (int i = 0; i < 3; i++) {
                    double ep = hp - i;
                    float off = (float)(-ep * 42);
                    double ev = hval - i;
                    if (!mirror) ev = std::fmod(ev, 100.0);
                    if (!showZero && ev == 0) continue;
                    long n = (long)std::fabs(ev);
                    char b[16];
                    std::snprintf(b, sizeof b, "%ld", n >= 10 ? n % 10 : n);
                    a380::draw_text(canvas, b, 325.f + x, 252.f + off + (float)(pos * 42) + 32.f, big, fill(col));
                }
            };
            canvas.save();
            canvas.clipRect(SkRect::MakeXYWH(325, 252, 100, 37));
            drum1(ttv, ttpos, 22.f, false, false);
            drum1(tv, tpos, 48.f, ttv != 0, false);
            drum1(hv, hpos, 73.5f, true, std::fabs(alt) < 500);
            canvas.restore();
            canvas.save();
            canvas.clipRect(SkRect::MakeXYWH(400, 243.344f, 100, 53.312f));
            SkFont tf = a380::font("ISISFont", 28.f);
            bool mirror = std::fabs(alt) < 50;
            double hvt = js_round((tens + 30) / 20) * 20;
            if (hvt > tens + 30) hvt -= 20;
            double hpt = js_round((tens + 30) / 20) * 20;
            if (hpt > tens + 30) hpt -= 20;
            if (hpt == 0) hpt++;
            if (!mirror) hvt += std::signbit(tens) ? -100 : 100;
            for (int i = 0; i < 3; i++) {
                double ep = hpt - i * 20;
                float off = (float)(-ep * 40 / 20);
                double ev = mirror ? hvt - i * 20 : std::fmod(hvt - i * 20, 100.0);
                long iv = (long)std::fabs(ev);
                std::string t = std::to_string(iv >= 100 ? iv - 100 : iv);
                while (t.size() < 2) t += '0';
                a380::draw_text(canvas, t, 400.f + 24.f, 243.344f + off + (float)(tens * 2) + 40.f, tf, fill(col),
                                Align::Left, a380::Baseline::Alphabetic, 3.1f);
            }
            canvas.restore();
            SkPathBuilder ob;
            ob.moveTo(466.749f, 243.344f);
            for (SkPoint q : {SkPoint{417.602f, 243.344f}, SkPoint{417.602f, 251.674f}, SkPoint{340.966f, 251.674f},
                              SkPoint{340.966f, 288.326f}, SkPoint{417.602f, 288.326f}, SkPoint{417.602f, 296.656f},
                              SkPoint{466.749f, 296.656f}})
                ob.lineTo(q);
            bool near_bug = false;
            for (int i = 0; i < 2; i++) near_bug = near_bug || (bugs[i].active && std::fabs(bugs[i].value - alt) < 100);
            a380::draw_path_cached(canvas, ob.detach(), stroke(near_bug ? kCyan : kYellow, 5.f));
            if (alt < 0) {
                SkFont neg = a380::font("ISISFont", 36.f);
                a380::draw_text(canvas, "N", 343.f, 243.f, neg, fill(kWhite));
                a380::draw_text(canvas, "E", 343.f, 284.f, neg, fill(kWhite));
                a380::draw_text(canvas, "G", 343.f, 325.f, neg, fill(kWhite));
            }
        }

        {
            SkPaint yb = stroke(kYellow, 5.f);
            SkPaint bg = fill(kBlack);
            SkPath a = poly({{176.032f, 263.336f}, {222.68f, 263.336f}, {222.68f, 279.996f}, {212.684f, 279.996f},
                             {212.684f, 273.332f}, {176.032f, 273.332f}});
            SkPath b2 = poly({{249.336f, 263.336f}, {262.664f, 263.336f}, {262.664f, 276.664f}, {249.336f, 276.664f}});
            SkPath c = poly({{335.968f, 263.336f}, {289.32f, 263.336f}, {289.32f, 279.996f}, {299.316f, 279.996f},
                             {299.316f, 273.332f}, {335.968f, 273.332f}});
            for (const SkPath* p : {&a, &b2, &c}) {
                a380::draw_path_cached(canvas, *p, bg);
                a380::draw_path_cached(canvas, *p, yb);
            }
        }

        if (in.var("L:A32NX_ISIS_LS_ACTIVE", "Boolean", 0) != 0) {
            canvas.save();
            canvas.translate(166.869f, 376.62f);
            ls_indicator(canvas, correct_loc_error(in.var("L:A32NX_RADIO_RECEIVER_LOC_DEVIATION", "number", 0)) / 2,
                         in.var("L:A32NX_RADIO_RECEIVER_LOC_IS_VALID", "number", 0) != 0, stroke_w);
            canvas.restore();
            canvas.save();
            canvas.translate(340.133f, 180.869f);
            canvas.rotate(90.f);
            ls_indicator(canvas, in.var("L:A32NX_RADIO_RECEIVER_GS_DEVIATION", "number", 0),
                         in.var("L:A32NX_RADIO_RECEIVER_GS_IS_VALID", "number", 0) != 0, stroke_w);
            canvas.restore();
        }

        {
            std::string text;
            float px = 256.f, fs = 28.f;
            const std::string nb = "\xc2\xa0";
            if (baro_mode == 1) {
                text = "STD";
                px = 250.f;
                fs = 36.f;
            } else {
                char b[16];
                std::snprintf(b, sizeof b, "%ld", (long)std::lround(hpa));
                std::string s = b;
                while (s.size() < 4) s = nb + s;
                text = s + nb + nb + nb + nb;
            }
            SkFont f = a380::font("ISISFont", fs);
            a380::draw_text(canvas, text, px, 450.f, f, fill(kCyan), Align::Center);
        }

        if (mach_shown_) {
            char b[16];
            std::snprintf(b, sizeof b, "%.2f", mach);
            a380::draw_text(canvas, b + 1, 68.f, 445.f, a380::font("ISISFont", 30.f), fill(kGreen));
        }
        canvas.restore();

        double raw = in.var("GLASSCOCKPIT AUTOMATIC BRIGHTNESS", "number", 0);
        if (raw > 1) raw /= 100;
        double autob = 0.2 + (0.99 - 0.2) * std::clamp(raw, 0.0, 1.0);
        double off = std::clamp(in.var("L:A32NX_ISIS_MANUAL_BRIGHTNESS_OFFSET", "number", 0), 0.03 - autob, 0.99 - autob);
        double target = std::clamp(autob + off, 0.03, 0.99);
        SkPaint dim;
        dim.setColor(SkColorSetARGB((uint8_t)std::lround((1 - target) * 255), 0, 0, 0));
        canvas.drawRect(SkRect::MakeWH(512, 512), dim);
    }

private:
    enum class Unit { On, Off, Selftest, Standby };
    static constexpr double kPowerUpMs = 90000.0;
    Unit state_ = Unit::Standby;
    bool started_ = false;
    double deadline_ms_ = -1.0;
    bool mach_shown_ = false;
    bool att_moved_ = false;
    double ss_out_ = 0.0, ss_in_ = 0.0, ss_at_ = -1.0, last_pitch_ = 0.0, last_roll_ = 0.0;
};

std::unique_ptr<a380::Display> make(std::string_view) { return std::make_unique<Isis>(); }
A380_REGISTER_DISPLAY("isislegacy", make);

}  // namespace
