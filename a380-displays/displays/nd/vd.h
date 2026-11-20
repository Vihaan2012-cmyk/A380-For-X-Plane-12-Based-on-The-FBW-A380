#pragma once

#include "a380/inputs.h"
#include "nd/nd_layers.h"

namespace a380::nd {

// What the VD publishes as L:A32NX_VD_<side>_RANGE_LOWER / _UPPER (feet) for the EGPWC terrain profile.
struct VdVerticalRange {
    double lower = 0.0;
    double upper = 0.0;
};

VdVerticalRange vd_vertical_range(const Inputs& in, const Frame& f);

}  // namespace a380::nd
