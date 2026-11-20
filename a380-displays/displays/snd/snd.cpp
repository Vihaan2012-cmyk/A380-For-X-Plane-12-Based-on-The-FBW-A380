#include <algorithm>
#include "a380/shape_cache.h"
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <optional>
#include <string>

#include "include/core/SkPaint.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkSurface.h"

#include "a380/display.h"
#include "a380/draw.h"
#include "a380/fonts.h"

namespace {

using a380::rgb;
constexpr float CX = 256.f, CY = 262.f, R = 166.f, CLIP_Y = 346.f;
constexpr double kRad = 3.14159265358979323846 / 180.0;

const SkColor kW = rgb(0xffffff), kG = rgb(0x00ff00), kC = rgb(0x00ffff), kM = rgb(0xff94ff), kY = rgb(0xffff00),
              kR = rgb(0xff0000);

std::optional<float> arinc(const a380::Inputs& in, const std::string& name) {
    double raw = in.var(name, "number", 0.0);
    if (!(raw > 0)) return std::nullopt;
    uint64_t u = (uint64_t)raw;
    if (((u >> 32) & 3) != 3) return std::nullopt;
    uint32_t lo = (uint32_t)(u & 0xffffffffu);
    float f;
    std::memcpy(&f, &lo, 4);
    return f;
}

double wrapn(double v, double n) { return std::fmod(std::fmod(v, n) + n, n); }

std::string pad(int n, int w) {
    char b[32];
    std::snprintf(b, sizeof b, "%0*d", w, n);
    return b;
}

std::string long_pos(double lat, double lon) {
    auto part = [](double v, int w, char h) {
        double a = std::fabs(v);
        int d = (int)std::floor(a);
        double m = std::min(599.0, std::floor((a - d) * 600.0 + 1e-4)) / 10.0;
        char mb[32];
        std::snprintf(mb, sizeof mb, "%.1f", m);
        std::string ms = mb;
        while (ms.size() < 4) ms = "0" + ms;
        return std::string(1, h) + " " + pad(d, w) + " " + ms;
    };
    return part(lat, 2, lat < 0 ? 'S' : 'N') + " / " + part(lon, 3, lon < 0 ? 'W' : 'E');
}

double round2(double a) {
    char b[32];
    std::snprintf(b, sizeof b, "%.2f", a);
    return std::atof(b);
}

struct Rot {
    SkCanvas& c;
    Rot(SkCanvas& canvas, double a, float x = CX, float y = CY) : c(canvas) {
        c.save();
        c.rotate((float)round2(a), x, y);
    }
    ~Rot() { c.restore(); }
};

SkPaint fill(SkColor col) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(col);
    return p;
}

SkPaint stroke(SkColor col, float w, SkPaint::Cap cap = SkPaint::kButt_Cap) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setStyle(SkPaint::kStroke_Style);
    p.setStrokeWidth(w);
    p.setColor(col);
    p.setStrokeCap(cap);
    return p;
}

void line(SkCanvas& c, float x1, float y1, float x2, float y2, SkColor col, float w) {
    c.drawLine(x1, y1, x2, y2, stroke(col, w));
}

void text(SkCanvas& c, const std::string& s, float x, float y, float size, SkColor col,
          a380::Align al = a380::Align::Left) {
    SkFont f = a380::font("SndFont", size, false, true);
    a380::draw_text(c, s, x, y, f, fill(col), al);
}

SkPath wedge(double deg) {
    float r = R + 30.f;
    double sx = r * std::sin(deg * kRad), sy = -r * std::cos(deg * kRad);
    SkPathBuilder b;
    b.moveTo(CX, CY);
    b.lineTo(CX - (float)sx, CY + (float)sy);
    b.arcTo(SkRect::MakeLTRB(CX - r, CY - r, CX + r, CY + r), (float)(270.0 - deg), (float)(2 * deg), false);
    b.close();
    return b.detach();
}

constexpr float kRoseHalf = R + 4.f;
constexpr float kRoseSupersample = 1.5f;

void rose_ticks(SkCanvas& canvas) {
    for (int a = 0; a < 360; a += 5) {
        bool lg = a % 10 == 0;
        Rot t(canvas, a);
        line(canvas, CX, CY - R, CX, CY - R + (lg ? 18.f : 10.f), kW, lg ? 3.f : 2.f);
    }
}

void rose_labels(SkCanvas& canvas) {
    for (int a = 0; a < 360; a += 30) {
        Rot t(canvas, a);
        text(canvas, std::to_string(a / 10), CX, CY - R + 46, 28, kW, a380::Align::Center);
    }
}

sk_sp<SkImage> rose_image(SkCanvas& canvas, float scale, void (*draw)(SkCanvas&)) {
    int px = (int)std::ceil(kRoseHalf * 2 * scale);
    SkImageInfo info = SkImageInfo::MakeN32Premul(px, px);
    sk_sp<SkSurface> surf = canvas.makeSurface(info);
    if (!surf) surf = SkSurfaces::Raster(info);
    if (!surf) return nullptr;
    SkCanvas* rc = surf->getCanvas();
    rc->clear(SK_ColorTRANSPARENT);
    rc->scale(scale, scale);
    rc->translate(-(CX - kRoseHalf), -(CY - kRoseHalf));
    draw(*rc);
    return surf->makeImageSnapshot();
}

class Snd : public a380::Display {
public:
    SkISize size() const override { return {512, 512}; }

    sk_sp<SkImage> ticks_, labels_;
    float rose_scale_ = 0;

    void draw(SkCanvas& canvas, const a380::Inputs& in) override {
        bool live = in.var("L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED", "bool", 0) != 0 ||
                    (in.var("AIRSPEED INDICATED", "knots", 0) > 50 && in.var("L:A32NX_ELEC_DC_HOT_1_BUS_IS_POWERED", "bool", 0) != 0);
        bool cold = in.var("L:A32NX_COLD_AND_DARK_SPAWN", "bool", 0) != 0;
        double now = in.now_ms();
        if (!started_) {
            started_ = true;
            power_ = cold ? Power::Off : Power::Standby;
            if (power_ == Power::Off && live && in.has_env_number("frozenAt.perf")) {
                double since = in.env_number("frozenAt.perf") - in.env_number("connectedAt.perf");
                power_ = since >= kPowerUpMs ? Power::On : Power::Test;
                deadline_ms_ = power_ == Power::Test ? now + kPowerUpMs - since : -1.0;
            }
        }
        if (deadline_ms_ >= 0 && now >= deadline_ms_) {
            deadline_ms_ = -1.0;
            if (power_ == Power::Standby) power_ = Power::Off;
            else if (power_ == Power::Test) power_ = Power::On;
        }
        if (power_ == Power::On && !live) {
            power_ = Power::Standby;
            deadline_ms_ = now + 10000.0;
        } else if (power_ == Power::Standby && live) {
            power_ = Power::On;
            deadline_ms_ = -1.0;
        } else if (power_ == Power::Off && live) {
            power_ = Power::Test;
            deadline_ms_ = now + kPowerUpMs;
        } else if (power_ == Power::Test && !live) {
            power_ = Power::Off;
            deadline_ms_ = -1.0;
        }
        if (power_ == Power::Off) return;
        if (power_ == Power::Test) {
            draw_test(canvas, (deadline_ms_ - now) / 1000.0);
            dim(canvas, in);
            return;
        }

        bool true_ref = in.var("L:A32NX_PUSH_TRUE_REF", "bool", 0) != 0;
        int ir = 0;
        for (int n : {3, 1})
            if (arinc(in, "L:A32NX_ADIRS_IR_" + std::to_string(n) + "_HEADING")) {
                ir = n;
                break;
            }
        std::optional<float> hdg, trk, gs;
        if (ir) {
            std::string p = "L:A32NX_ADIRS_IR_" + std::to_string(ir) + "_";
            hdg = arinc(in, p + "HEADING");
            if (true_ref) hdg = arinc(in, p + "TRUE_HEADING");
            trk = arinc(in, p + (true_ref ? "TRUE_TRACK" : "TRACK"));
            gs = arinc(in, p + "GROUND_SPEED");
        }

        std::optional<std::pair<double, double>> pos;
        std::string pos_src;
        for (int n : {3, 1}) {
            auto la = arinc(in, "L:A32NX_ADIRS_IR_" + std::to_string(n) + "_LATITUDE");
            auto lo = arinc(in, "L:A32NX_ADIRS_IR_" + std::to_string(n) + "_LONGITUDE");
            if (la && lo) {
                pos = {(double)*la, (double)*lo};
                pos_src = "GPIR" + std::to_string(n);
                break;
            }
        }
        if (!pos) {
            double la = in.var("GPS POSITION LAT", "degrees", 0), lo = in.var("GPS POSITION LON", "degrees", 0);
            if (la != 0 || lo != 0) {
                pos = {la, lo};
                pos_src = "GPS";
            }
        }
        double gsv = gs ? (double)*gs : 0;
        if (!gs && pos) gsv = in.var("GPS GROUND SPEED", "knots", 0);

        text(canvas, "GS", 16, 46, 24, kW);
        text(canvas, gs || pos ? std::to_string((int)std::lround(gsv)) : "---", 56, 46, 30, kG);
        if (ir) {
            canvas.drawRect(SkRect::MakeXYWH(220, 19, 72, 34), stroke(kW, 2));
            text(canvas, "IR" + std::to_string(ir), CX, 46, 26, kW, a380::Align::Center);
        }
        if (true_ref) text(canvas, "TRUE", CX, 76, 20, kC, a380::Align::Center);

        if (hdg) {
            double h = *hdg;
            const SkMatrix m = canvas.getTotalMatrix();
            float scale = std::round(std::hypot(m.getScaleX(), m.getSkewY()) * kRoseSupersample * 1000.f) / 1000.f;
            if (!ticks_ || !labels_ || scale != rose_scale_) {
                ticks_ = rose_image(canvas, scale, rose_ticks);
                labels_ = rose_image(canvas, scale, rose_labels);
                rose_scale_ = scale;
            }
            const SkRect dst = SkRect::MakeXYWH(CX - kRoseHalf, CY - kRoseHalf, kRoseHalf * 2, kRoseHalf * 2);
            const SkSamplingOptions linear(SkFilterMode::kLinear);
            canvas.save();
            canvas.clipPath(wedge(122), true);
            {
                Rot r(canvas, -h);
                if (ticks_) canvas.drawImageRect(ticks_, dst, linear, nullptr);
                else rose_ticks(canvas);
            }
            canvas.restore();
            canvas.save();
            canvas.clipPath(wedge(136), true);
            {
                Rot r(canvas, -h);
                if (labels_) canvas.drawImageRect(labels_, dst, linear, nullptr);
                else rose_labels(canvas);
            }
            canvas.restore();

            {
                SkPathBuilder b;
                b.moveTo(CX - 12, CY - R - 22).lineTo(CX + 12, CY - R - 22).lineTo(CX, CY - R - 3).close();
                a380::draw_path_cached(canvas, b.detach(), fill(kY));
            }
            for (int a : {-90, -45, 45, 90}) {
                Rot t(canvas, a);
                SkPathBuilder b;
                float x = CX, y = CY - R - 4;
                b.moveTo(x, y).lineTo(x - 8, y - 14).lineTo(x + 8, y - 14).close();
                a380::draw_path_cached(canvas, b.detach(), fill(kW));
            }
            if (trk) {
                Rot t(canvas, *trk - h);
                SkPathBuilder b;
                b.moveTo(CX, CY - R + 6).lineTo(CX + 7, CY - R + 15).lineTo(CX, CY - R + 24).lineTo(CX - 7, CY - R + 15).close();
                a380::draw_path_cached(canvas, b.detach(), stroke(kG, 3));
            }
        }

        {
            SkPaint p = stroke(kY, 5, SkPaint::kRound_Cap);
            SkPathBuilder b;
            b.moveTo(CX, CY - 20).lineTo(CX, CY + 24).moveTo(CX - 26, CY - 3).lineTo(CX + 26, CY - 3).moveTo(CX - 10, CY + 20).lineTo(CX + 10, CY + 20);
            a380::draw_path_cached(canvas, b.detach(), p);
        }

        if (!hdg) {
            canvas.drawRect(SkRect::MakeXYWH(CX - 46, CY - 70, 92, 42), stroke(kR, 3));
            text(canvas, "HDG", CX, CY - 38, 30, kR, a380::Align::Center);
        }

        if (pos) {
            canvas.drawRect(SkRect::MakeXYWH(10, 353, 84, 32), stroke(kW, 2));
            text(canvas, pos_src, 52, 378, 22, kW, a380::Align::Center);
        }
        text(canvas, pos ? long_pos(pos->first, pos->second) : "POS", 102, 379, 26, pos ? kG : kR);
        line(canvas, 8, 394, 504, 394, kW, 2);

        dim(canvas, in);
    }

private:
    enum class Power { On, Off, Test, Standby };
    static constexpr double kPowerUpMs = 90000.0;
    Power power_ = Power::Standby;
    bool started_ = false;
    double deadline_ms_ = -1.0;

    static void dim(SkCanvas& canvas, const a380::Inputs& in) {
        double raw = in.var("GLASSCOCKPIT AUTOMATIC BRIGHTNESS", "percent", 0) / 100.0;
        double autob = 0.2 + 0.79 * std::clamp(raw, 0.0, 1.0);
        double off = in.var("L:A380X_SND_MANUAL_BRIGHTNESS_OFFSET", "number", 0);
        double b = std::clamp(autob + off, 0.03, 0.99);
        char ob[16];
        std::snprintf(ob, sizeof ob, "%.3f", 1 - b);
        SkPaint paint;
        paint.setColor(SK_ColorBLACK);
        paint.setAlphaf((float)std::atof(ob));
        canvas.drawRect(SkRect::MakeWH(512, 512), paint);
    }

    static void draw_test(SkCanvas& canvas, double timer) {
        struct Box {
            float x, y, w;
            const char* label;
        };
        for (const Box& b : {Box{14, 26, 84, "GS"}, Box{CX - 50, CY - 70, 100, "HDG"}, Box{10, 350, 104, "POS"},
                             Box{CX - 90, CY + 40, 180, "INIT"}}) {
            canvas.drawRect(SkRect::MakeXYWH(b.x, b.y, b.w, 40), fill(kY));
            text(canvas, b.label, b.x + b.w / 2, b.y + 31, 30, SK_ColorBLACK, a380::Align::Center);
        }
        char t[16];
        std::snprintf(t, sizeof t, "%ds", (int)std::max(0.0, std::ceil(timer)));
        text(canvas, t, CX + 84, CY + 71, 30, SK_ColorBLACK, a380::Align::Right);
    }
};

std::unique_ptr<a380::Display> make(std::string_view) { return std::make_unique<Snd>(); }
A380_REGISTER_DISPLAY("snd", make);

}  // namespace
