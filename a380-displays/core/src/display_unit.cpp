#include "a380/display_unit.h"

#include <algorithm>
#include <cstdio>
#include <cstdlib>

#include "a380/draw.h"
#include "a380/fonts.h"
#include "a380/profile.h"

namespace a380 {

namespace {
const char* state_name(CdsDisplayUnit::State s) {
    switch (s) {
        case CdsDisplayUnit::State::On: return "on";
        case CdsDisplayUnit::State::Off: return "off";
        case CdsDisplayUnit::State::ThalesBootup: return "booting";
        case CdsDisplayUnit::State::Selftest: return "self test";
        case CdsDisplayUnit::State::Standby: return "standby";
    }
    return "?";
}

double self_test_s(const Inputs& in) {
    std::string configured = in.str("CONFIG_SELF_TEST_TIME");
    return configured.empty() ? 15.0 : std::atof(configured.c_str());
}
}  // namespace

CdsDisplayUnit::CdsDisplayUnit(std::string potentiometer, std::vector<std::string> buses)
    : potentiometer_(std::move(potentiometer)), buses_(std::move(buses)) {}

void CdsDisplayUnit::update_state(const Inputs& in) {
    double now = in.now_ms();
    bool bright = brightness_ != 0.0;
    if (state_ != State::Off && failed_) {
        state_ = State::Off;
        deadline_ms_ = -1.0;
    } else if (state_ == State::On && (!bright || !powered_)) {
        state_ = State::Standby;
        set_timer(now, 10.0);
    } else if (state_ == State::Standby && bright && powered_) {
        state_ = State::On;
        deadline_ms_ = -1.0;
    } else if (state_ == State::Off && bright && powered_ && !failed_) {
        state_ = State::ThalesBootup;
        set_timer(now, 0.35);
    } else if (state_ == State::ThalesBootup && bright && powered_ && !failed_) {
        state_ = State::Selftest;
        set_timer(now, self_test_s(in));
    } else if ((state_ == State::Selftest || state_ == State::ThalesBootup) && (!bright || !powered_)) {
        state_ = State::Off;
        deadline_ms_ = -1.0;
    }
}

bool CdsDisplayUnit::update(SkCanvas& canvas, const Inputs& in, SkISize size, bool failed) {
    const State before = state_;
    const bool first = !started_;
    std::string pot = "LIGHT POTENTIOMETER:" + potentiometer_;
    double brightness = in.has_var(pot) ? in.var(pot) : 1.0;
    bool recorded = false;
    bool powered = false;
    for (const std::string& b : buses_) {
        std::string name = "L:A32NX_ELEC_" + b + "_BUS_IS_POWERED";
        recorded = recorded || in.has_var(name);
        powered = powered || in.var(name) != 0.0;
    }
    powered = powered || !recorded;
    if (!started_) {
        started_ = true;
        state_ = in.var("L:A32NX_COLD_AND_DARK_SPAWN") != 0.0 ? State::Off : State::Standby;
        brightness_ = brightness;
        powered_ = powered;
        failed_ = failed;
        update_state(in);
        if (state_ == State::ThalesBootup && in.has_env_number("frozenAt.perf")) {
            double since = in.env_number("frozenAt.perf") - in.env_number("connectedAt.perf");
            double test_ms = self_test_s(in) * 1000.0;
            if (since >= 350.0 + test_ms) {
                state_ = State::On;
                deadline_ms_ = -1.0;
            } else if (since >= 350.0) {
                state_ = State::Selftest;
                deadline_ms_ = in.now_ms() + 350.0 + test_ms - since;
            } else {
                deadline_ms_ = in.now_ms() + 350.0 - since;
            }
        }
    }
    if (brightness != brightness_ || powered != powered_ || failed != failed_) {
        brightness_ = brightness;
        powered_ = powered;
        failed_ = failed;
        update_state(in);
    }
    if (deadline_ms_ >= 0.0 && in.now_ms() >= deadline_ms_) {
        deadline_ms_ = -1.0;
        if (state_ == State::Standby) state_ = State::Off;
        if (state_ == State::Selftest) state_ = State::On;
        update_state(in);
    }
    if (first || state_ != before) {
        char line[200];
        std::snprintf(line, sizeof line, "display unit %s: %s (brightness %.2f%s, powered %d%s, cold and dark %d)", potentiometer_.c_str(), state_name(state_), brightness_,
                      in.has_var(pot) ? "" : " unset", powered_ ? 1 : 0, powered_ && buses_.size() && !in.has_var("L:A32NX_ELEC_" + buses_[0] + "_BUS_IS_POWERED") ? " unset" : "",
                      in.var("L:A32NX_COLD_AND_DARK_SPAWN") != 0.0 ? 1 : 0);
        profile_emit(line);
    }

    if (state_ != State::Selftest && state_ != State::ThalesBootup) return state_ == State::On;

    float scale = std::min(size.width() / 768.f, size.height() / 1024.f);
    canvas.save();
    canvas.translate((size.width() - 768.f * scale) / 2.f, (size.height() - 1024.f * scale) / 2.f);
    canvas.scale(scale, scale);
    SkPaint black;
    black.setColor(SK_ColorBLACK);
    canvas.drawRect(SkRect::MakeWH(768.f, 1024.f), black);
    if (state_ == State::Selftest) {
        SkPaint green;
        green.setAntiAlias(true);
        green.setColor(rgb(0x00ff00));
        SkFont f = font("Ecam", 24.f);
        draw_svg_text(canvas, "SAFETY TEST IN PROGRESS", 384.f, 512.f, f, green, Align::Center);
        draw_svg_text(canvas, "(MAX 30 SECONDS)", 384.f, 552.96f, f, green, Align::Center);
    } else {
        SkPaint white;
        white.setColor(SK_ColorWHITE);
        canvas.drawRect(SkRect::MakeXYWH(84.f, 880.f, 600.f, 70.f), white);
        SkPaint blue;
        blue.setColor(rgb(0x4d4dff));
        canvas.drawRect(SkRect::MakeXYWH(89.f, 885.f, 35.f, 60.f), blue);
    }
    canvas.restore();
    return false;
}

}  // namespace a380
