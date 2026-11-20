#include <cmath>
#include "a380/shape_cache.h"
#include <cstdlib>
#include <cstring>

#include <algorithm>
#include <vector>

#include "include/core/SkBitmap.h"
#include "include/core/SkPathBuilder.h"

#include "a380/raster.h"
#include "sdsys.h"

namespace sdsys {

A429 decode429(double raw) {
    double hi = std::floor(raw / 4294967296.0);
    uint32_t bits = (uint32_t)(uint64_t)(raw - hi * 4294967296.0);
    float f;
    std::memcpy(&f, &bits, 4);
    return {(int)hi & 3, f};
}

bool bit429(const a380::Inputs& in, std::string_view n, int bit) {
    A429 a = var429(in, n);
    if (a.ssm != 3 && a.ssm != 2) return false;
    return (((int32_t)a.val >> (bit - 1)) & 1) != 0;
}

SkPaint sk_stroke(uint32_t color, float width, bool round) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(a380::rgb(color));
    p.setStyle(SkPaint::kStroke_Style);
    p.setStrokeWidth(width);
    if (round) p.setStrokeCap(SkPaint::kRound_Cap);
    return p;
}

void circle_stroke(SkCanvas& cv, float cx, float cy, float r, uint32_t color, float width) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(a380::rgb(color));
    a380::chromium_stroke_circle(cv, cx, cy, r, width, p);
}

void circle_fill(SkCanvas& cv, float cx, float cy, float r, uint32_t color) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(a380::rgb(color));
    a380::chromium_fill_circle(cv, cx, cy, r, p);
}

SkPath polyline_stroke(const std::vector<SkPoint>& pts, float width, bool closed) {
    size_t n = pts.size();
    if (n < 2) return SkPath();
    if (closed && n > 2 && pts.front() == pts.back()) {
        std::vector<SkPoint> c(pts.begin(), pts.end() - 1);
        return polyline_stroke(c, width, true);
    }
    n = pts.size();
    size_t segs = closed ? n : n - 1;
    std::vector<SkPoint> nrm(segs);
    for (size_t i = 0; i < segs; i++) {
        SkVector d = pts[(i + 1) % n] - pts[i];
        d.normalize();
        nrm[i] = {-d.fY, d.fX};
    }
    float h = width * 0.5f;
    std::vector<SkPoint> left, right;
    for (size_t i = 0; i < n; i++) {
        SkPoint a, b;
        if (closed) {
            a = nrm[(i + n - 1) % n];
            b = nrm[i];
        } else {
            a = i > 0 ? nrm[i - 1] : nrm[0];
            b = i + 1 < n ? nrm[i] : nrm[n - 2];
        }
        float dot = a.fX * b.fX + a.fY * b.fY, k = 1.f + dot;
        float cr = a.fX * b.fY - a.fY * b.fX;
        float miter = k > 1e-4f ? 1.f / std::sqrt(k * 0.5f) : 1e9f;
        SkPoint m{(a.fX + b.fX) * h / std::max(k, 1e-4f), (a.fY + b.fY) * h / std::max(k, 1e-4f)};
        SkPoint v = pts[i];
        bool bevel = miter > 4.f;
        if (!bevel) {
            left.push_back({v.fX + m.fX, v.fY + m.fY});
            right.push_back({v.fX - m.fX, v.fY - m.fY});
        } else {
            SkPoint la{v.fX + a.fX * h, v.fY + a.fY * h}, lb{v.fX + b.fX * h, v.fY + b.fY * h};
            SkPoint ra{v.fX - a.fX * h, v.fY - a.fY * h}, rb{v.fX - b.fX * h, v.fY - b.fY * h};
            if (cr > 0) {
                left.push_back({v.fX + m.fX, v.fY + m.fY});
                right.push_back(ra);
                right.push_back(rb);
            } else {
                right.push_back({v.fX - m.fX, v.fY - m.fY});
                left.push_back(la);
                left.push_back(lb);
            }
        }
    }
    SkPathBuilder pb;
    if (closed) {
        pb.moveTo(left[0]);
        for (size_t i = 1; i < left.size(); i++) pb.lineTo(left[i]);
        pb.close();
        pb.moveTo(right[0]);
        for (size_t i = 1; i < right.size(); i++) pb.lineTo(right[i]);
        pb.close();
        pb.setFillType(SkPathFillType::kEvenOdd);
        return pb.detach();
    }
    pb.moveTo(left[0]);
    for (size_t i = 1; i < left.size(); i++) pb.lineTo(left[i]);
    for (size_t i = right.size(); i-- > 0;) pb.lineTo(right[i]);
    pb.close();
    return pb.detach();
}

void arc_path(SkCanvas& cv, float x, float y, float r, float a0, float a1, uint32_t color, float width) {
    const double kPi = 3.14159265358979323846;
    double span = a1 >= a0 ? a1 - a0 : a1 + 360.0 - a0;
    SkPaint p = sk_stroke(color, width, false);
    a380::chromium_stroke_arc(cv, x, y, r, (float)((a1 - 90.0) * kPi / 180.0), (float)(-span * kPi / 180.0), p);
}

void gauge_dir(float value, float mn, float mx, float sa, float ea, float& dx, float& dy) {
    float pct = (value - mn) / (mx - mn);
    float span = sa > ea ? 360.f - sa + ea : ea - sa;
    float ang = (sa + pct * span - 90.f) * 3.14159265f / 180.f;
    dx = std::cos(ang);
    dy = std::sin(ang);
}

void gauge_marker(SkCanvas& cv, float x, float y, float value, float mn, float mx, float r, float sa, float ea,
                  uint32_t color, float width, bool round, bool indicator, bool half, float mi, float mo) {
    float dx, dy;
    gauge_dir(value, mn, mx, sa, ea, dx, dy);
    float x1 = x + dx * r * mi, y1 = y + dy * r * mi, x2 = x + dx * r, y2 = y + dy * r;
    if (indicator) {
        if (!half) { x1 = x; y1 = y; }
        x2 = x + dx * r * mo;
        y2 = y + dy * r * mo;
    }
    draw_line(cv, x1, y1, x2, y2, sk_stroke(color, width, round));
}

void valve(SkCanvas& cv, float x, float y, float r, bool vertical, uint32_t color, float width) {
    circle_stroke(cv, x, y, r, color, width);
    if (vertical) draw_line(cv, x, y - r, x, y + r, sk_stroke(color, width));
    else draw_line(cv, x - r, y, x + r, y, sk_stroke(color, width));
}

void rect_stroke(SkCanvas& cv, float x, float y, float w, float h, uint32_t color, float width) {
    draw_rect(cv, SkRect::MakeXYWH(x, y, w, h), sk_stroke(color, width));
}

void rect_fill(SkCanvas& cv, float x, float y, float w, float h, uint32_t color) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(a380::rgb(color));
    draw_rect(cv, SkRect::MakeXYWH(x, y, w, h), p);
}

}  // namespace sdsys

namespace sdsys {

void draw_path(SkCanvas& cv, const SkPath& path, const SkPaint& paint) { a380::draw_path_cached(cv, path, paint); }

void draw_line(SkCanvas& cv, float x0, float y0, float x1, float y1, const SkPaint& paint) {
    SkPaint p(paint);
    p.setStyle(SkPaint::kStroke_Style);
    if ((x0 == x1 || y0 == y1) && p.getStrokeCap() == SkPaint::kButt_Cap) {
        a380::chromium_stroke_line(cv, x0, y0, x1, y1, p.getStrokeWidth(), p);
        return;
    }
    a380::chromium_stroke_path(cv, SkPath::Line({x0, y0}, {x1, y1}), p);
}

void draw_rect(SkCanvas& cv, const SkRect& r, const SkPaint& paint) {
    if (paint.getStyle() == SkPaint::kFill_Style) a380::chromium_fill_rect(cv, r, paint);
    else if (paint.getStyle() == SkPaint::kStroke_Style) a380::chromium_stroke_rect(cv, r, paint.getStrokeWidth(), paint);
    else a380::chromium_stroke_path(cv, SkPath::Rect(r), paint);
}

}  // namespace sdsys
