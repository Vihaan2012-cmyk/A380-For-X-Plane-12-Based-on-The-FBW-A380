#pragma once

#include <string>
#include <vector>

#include "include/core/SkCanvas.h"
#include "include/core/SkSize.h"

#include "a380/inputs.h"

namespace a380 {

// FlyByWire's CdsDisplayUnit (MsfsAvionicsCommon/CdsDisplayUnit.tsx): the power, brightness, boot-up and
// self-test states around a CDS page. `update` advances it and paints the boot-up and self-test screens; the
// page draws only when it returns true.
class CdsDisplayUnit {
public:
    enum class State { On, Off, ThalesBootup, Selftest, Standby };

    CdsDisplayUnit(std::string potentiometer, std::vector<std::string> buses);

    bool update(SkCanvas& canvas, const Inputs& in, SkISize size, bool failed = false);
    State state() const { return state_; }

private:
    void update_state(const Inputs& in);
    void set_timer(double now_ms, double seconds) { deadline_ms_ = now_ms + seconds * 1000.0; }

    std::string potentiometer_;
    std::vector<std::string> buses_;
    State state_ = State::Standby;
    bool started_ = false;
    double brightness_ = 0.0;
    bool powered_ = false;
    bool failed_ = false;
    double deadline_ms_ = -1.0;
};

}  // namespace a380
