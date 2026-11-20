#pragma once

#include "include/core/SkCanvas.h"
#include "include/core/SkColor.h"
#include "include/core/SkPoint.h"

#include "a380/inputs.h"

namespace a380::nd {

void stroke_circle(SkCanvas& canvas, float cx, float cy, float r, float width, SkColor color);  // nd.cpp

// The ND's map geometry for one frame, as FlyByWire's ND computes it from the
// EFIS mode/range and the aircraft position (or the PLAN centre). Built by
// nd.cpp; every layer projects through it so they all line up.
struct Frame {
    int side = 1;                // 1 captain, 2 first officer
    int mode = 3;                // 0 LS, 1 VOR, 2 NAV (rose), 3 ARC, 4 PLAN
    int range_index = 3;         // A32NX_EFIS_{L,R}_ND_RANGE: 0 ZOOM, 1..7 = 10..640 NM
    double range_nm = 40.0;      // the range the outer ring/arc stands for
    SkPoint centre{384.f, 620.f};  // map reference point in CSS px
    float px_per_nm = 1.f;
    double up_true_deg = 0.0;    // true bearing pointing up the screen
    double true_heading_deg = 0.0;
    double ref_lat = 0.0;        // latitude/longitude at `centre`
    double ref_lon = 0.0;
    bool map_available = true;   // false when the map must not be drawn (flags)

    SkPoint project(double lat, double lon) const;
};

// Layers, each in its own file and owned by one author. nd.cpp draws the
// frame (rose/arc/plan, rings, heading/track, aircraft, labels, flags,
// messages) and calls these in FlyByWire's paint order.
void draw_route(SkCanvas& canvas, const Inputs& in, const Frame& f);          // route.cpp
void draw_traffic(SkCanvas& canvas, const Inputs& in, const Frame& f);        // route.cpp
void draw_vertical_display(SkCanvas& canvas, const Inputs& in, const Frame& f);  // vd.cpp
bool oans_shown(const Inputs& in, int side);                                   // oans.cpp
void draw_oans(SkCanvas& canvas, const Inputs& in, const Frame& f);           // oans.cpp
void draw_oans_menu(SkCanvas& canvas, const Inputs& in, int side);           // oans.cpp
void draw_terrain(SkCanvas& canvas, const Inputs& in, const Frame& f);        // terrain.cpp

}  // namespace a380::nd
