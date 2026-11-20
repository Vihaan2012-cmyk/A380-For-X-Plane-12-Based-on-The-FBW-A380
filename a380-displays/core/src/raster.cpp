#include "a380/raster.h"
#include "a380/env.h"
#include "a380/shape_cache.h"

#include <algorithm>
#include <cmath>
#include <cstdlib>
#include <vector>

#include "include/core/SkBitmap.h"
#include "include/core/SkImage.h"
#include "include/core/SkImageInfo.h"
#include "include/core/SkMatrix.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkPathUtils.h"
#include "include/effects/SkDashPathEffect.h"

namespace a380 {
namespace {

struct Mask {
    int x0 = 0, y0 = 0, w = 0, h = 0;
    std::vector<uint8_t> a;
    Mask(int x, int y, int ww, int hh) : x0(x), y0(y), w(ww), h(hh), a((size_t)std::max(ww, 0) * std::max(hh, 0), 0) {}
    uint8_t& at(int x, int y) { return a[(size_t)y * w + x]; }
};

inline uint8_t cov8(float c) { return (uint8_t)std::lround(std::clamp(c, 0.f, 1.f) * 255.f); }

void blit(SkCanvas& canvas, const Mask& m, const SkPaint& paint) {
    if (m.w <= 0 || m.h <= 0) return;
    SkBitmap bm;
    bm.allocPixels(SkImageInfo::MakeA8(m.w, m.h));
    for (int y = 0; y < m.h; y++) std::copy_n(&m.a[(size_t)y * m.w], m.w, bm.getAddr8(0, y));
    sk_sp<SkImage> img = bm.asImage();
    SkPaint p(paint);
    p.setStyle(SkPaint::kFill_Style);
    canvas.save();
    canvas.resetMatrix();
    canvas.drawImage(img, (float)m.x0, (float)m.y0, SkSamplingOptions(), &p);
    canvas.restore();
}

struct Poly {
    std::vector<std::vector<SkPoint>> contours;
};


int clz32(uint32_t v) {
    if (v == 0) return 32;
    int n = 0;
    while (!(v & 0x80000000u)) { v <<= 1; n++; }
    return n;
}

int edge_fdot6(float v) { return (int)(v * 64.f); }

int edge_steps(int dx, int dy, int accuracy) {
    dx = std::abs(dx);
    dy = std::abs(dy);
    int dist = dx > dy ? dx + dy / 2 : dy + dx / 2;
    dist = (dist + (1 << (2 + accuracy))) >> (3 + accuracy);
    return (32 - clz32((uint32_t)dist)) >> 1;
}

int cubic_delta(int a, int b, int c, int d) {
    int one = (a * 8 - b * 15 + 6 * c + d) * 19 >> 9;
    int two = (a + 6 * b - c * 15 + d * 8) * 19 >> 9;
    return std::max(std::abs(one), std::abs(two));
}

struct EdgeMode {
    int on = 0;
    int quad_acc = 0;
    int cubic_acc = 2;
    int cubic_add = 1;
    int snap_y = 0;
    int max_shift = 6;
};

const EdgeMode& edge_mode() {
    static const EdgeMode m = [] {
        EdgeMode e;
        if (const char* v = a380::getenv_cached("A380_SKEDGE")) {
            e.on = std::atoi(v);
            if (const char* q = a380::getenv_cached("A380_SKEDGE_QACC")) e.quad_acc = std::atoi(q);
            if (const char* q = a380::getenv_cached("A380_SKEDGE_CACC")) e.cubic_acc = std::atoi(q);
            if (const char* q = a380::getenv_cached("A380_SKEDGE_CADD")) e.cubic_add = std::atoi(q);
            if (const char* q = a380::getenv_cached("A380_SKEDGE_SNAP")) e.snap_y = std::atoi(q);
        }
        return e;
    }();
    return m;
}

SkPoint lerp_pt(SkPoint a, SkPoint b, float t) { return {a.fX + (b.fX - a.fX) * t, a.fY + (b.fY - a.fY) * t}; }

void chop_quad(const SkPoint* p, float t, SkPoint* l, SkPoint* r) {
    SkPoint ab = lerp_pt(p[0], p[1], t), bc = lerp_pt(p[1], p[2], t), m = lerp_pt(ab, bc, t);
    l[0] = p[0]; l[1] = ab; l[2] = m;
    r[0] = m; r[1] = bc; r[2] = p[2];
}

void chop_cubic(const SkPoint* p, float t, SkPoint* l, SkPoint* r) {
    SkPoint ab = lerp_pt(p[0], p[1], t), bc = lerp_pt(p[1], p[2], t), cd = lerp_pt(p[2], p[3], t);
    SkPoint abc = lerp_pt(ab, bc, t), bcd = lerp_pt(bc, cd, t), m = lerp_pt(abc, bcd, t);
    l[0] = p[0]; l[1] = ab; l[2] = abc; l[3] = m;
    r[0] = m; r[1] = bcd; r[2] = cd; r[3] = p[3];
}

void edge_quad(const SkPoint* q, const EdgeMode& em, std::vector<SkPoint>& cur) {
    int x0 = edge_fdot6(q[0].fX), y0 = edge_fdot6(q[0].fY), x1 = edge_fdot6(q[1].fX), y1 = edge_fdot6(q[1].fY);
    int x2 = edge_fdot6(q[2].fX), y2 = edge_fdot6(q[2].fY);
    int shift = edge_steps((2 * x1 - x0 - x2) >> 2, (2 * y1 - y0 - y2) >> 2, em.quad_acc);
    shift = shift == 0 ? 1 : std::min(shift, 6);
    int n = 1 << shift;
    for (int i = 1; i < n; i++) {
        float t = (float)i / n, u = 1 - t;
        cur.push_back({u * u * q[0].fX + 2 * u * t * q[1].fX + t * t * q[2].fX, u * u * q[0].fY + 2 * u * t * q[1].fY + t * t * q[2].fY});
    }
    cur.push_back(q[2]);
}

void edge_cubic(const SkPoint* c, const EdgeMode& em, std::vector<SkPoint>& cur) {
    int x[4], y[4];
    for (int i = 0; i < 4; i++) { x[i] = edge_fdot6(c[i].fX); y[i] = edge_fdot6(c[i].fY); }
    int shift = edge_steps(cubic_delta(x[0], x[1], x[2], x[3]), cubic_delta(y[0], y[1], y[2], y[3]), em.cubic_acc) + em.cubic_add;
    shift = std::clamp(shift, 1, 6);
    int n = 1 << shift;
    for (int i = 1; i < n; i++) {
        float t = (float)i / n, u = 1 - t;
        float a = u * u * u, b = 3 * u * u * t, cc = 3 * u * t * t, d = t * t * t;
        cur.push_back({a * c[0].fX + b * c[1].fX + cc * c[2].fX + d * c[3].fX, a * c[0].fY + b * c[1].fY + cc * c[2].fY + d * c[3].fY});
    }
    cur.push_back(c[3]);
}

void edge_flatten_quad(const SkPoint* q, const EdgeMode& em, std::vector<SkPoint>& cur) {
    float den = q[0].fY - 2 * q[1].fY + q[2].fY;
    float t = den != 0.f ? (q[0].fY - q[1].fY) / den : -1.f;
    if (t > 0.f && t < 1.f) {
        SkPoint l[3], r[3];
        chop_quad(q, t, l, r);
        l[2].fY = r[0].fY = l[2].fY;
        edge_quad(l, em, cur);
        edge_quad(r, em, cur);
    } else {
        edge_quad(q, em, cur);
    }
}

void edge_flatten_cubic(const SkPoint* c, const EdgeMode& em, std::vector<SkPoint>& cur) {
    float a = -c[0].fY + 3 * c[1].fY - 3 * c[2].fY + c[3].fY, b = 2 * (c[0].fY - 2 * c[1].fY + c[2].fY), cc = c[1].fY - c[0].fY;
    float ts[2];
    int nt = 0;
    if (std::fabs(a) < 1e-9f) {
        if (std::fabs(b) > 1e-9f) ts[nt++] = -cc / b;
    } else {
        float disc = b * b - 4 * a * cc;
        if (disc >= 0.f) {
            float sq = std::sqrt(disc);
            ts[nt++] = (-b - sq) / (2 * a);
            ts[nt++] = (-b + sq) / (2 * a);
        }
    }
    std::sort(ts, ts + nt);
    SkPoint rest[4] = {c[0], c[1], c[2], c[3]};
    float done = 0.f;
    for (int i = 0; i < nt; i++) {
        if (!(ts[i] > done + 1e-6f && ts[i] < 1.f - 1e-6f)) continue;
        float lt = (ts[i] - done) / (1.f - done);
        SkPoint l[4], r[4];
        chop_cubic(rest, lt, l, r);
        edge_cubic(l, em, cur);
        std::copy(r, r + 4, rest);
        done = ts[i];
    }
    edge_cubic(rest, em, cur);
}

void flatten(const SkPath& path, const SkMatrix& m, Poly& out) {
    std::vector<SkPoint> cur;
    auto flush = [&] {
        if (cur.size() >= 2) out.contours.push_back(cur);
        cur.clear();
    };
    SkPath::Iter it(path, true);
    SkPoint pts[4];
    for (;;) {
        SkPath::Verb v = it.next(pts);
        if (v == SkPath::kDone_Verb) break;
        switch (v) {
            case SkPath::kMove_Verb: flush(); cur.push_back(m.mapPoint(pts[0])); break;
            case SkPath::kLine_Verb: cur.push_back(m.mapPoint(pts[1])); break;
            case SkPath::kQuad_Verb:
                if (edge_mode().on) {
                    SkPoint dp[3] = {m.mapPoint(pts[0]), m.mapPoint(pts[1]), m.mapPoint(pts[2])};
                    edge_flatten_quad(dp, edge_mode(), cur);
                    break;
                }
                for (int i = 1; i <= 16; i++) {
                    float t = i / 16.f, u = 1 - t;
                    cur.push_back(m.mapPoint({u * u * pts[0].fX + 2 * u * t * pts[1].fX + t * t * pts[2].fX,
                                              u * u * pts[0].fY + 2 * u * t * pts[1].fY + t * t * pts[2].fY}));
                }
                break;
            case SkPath::kCubic_Verb:
                if (edge_mode().on) {
                    SkPoint dp[4] = {m.mapPoint(pts[0]), m.mapPoint(pts[1]), m.mapPoint(pts[2]), m.mapPoint(pts[3])};
                    edge_flatten_cubic(dp, edge_mode(), cur);
                    break;
                }
                for (int i = 1; i <= 24; i++) {
                    float t = i / 24.f, u = 1 - t;
                    float a = u * u * u, b = 3 * u * u * t, c = 3 * u * t * t, d = t * t * t;
                    cur.push_back(m.mapPoint({a * pts[0].fX + b * pts[1].fX + c * pts[2].fX + d * pts[3].fX,
                                              a * pts[0].fY + b * pts[1].fY + c * pts[2].fY + d * pts[3].fY}));
                }
                break;
            case SkPath::kConic_Verb: {
                float wt = it.conicWeight();
                for (int i = 1; i <= 16; i++) {
                    float t = i / 16.f, u = 1 - t;
                    float b0 = u * u, b1 = 2 * u * t * wt, b2 = t * t, den = b0 + b1 + b2;
                    cur.push_back(m.mapPoint({(b0 * pts[0].fX + b1 * pts[1].fX + b2 * pts[2].fX) / den,
                                              (b0 * pts[0].fY + b1 * pts[1].fY + b2 * pts[2].fY) / den}));
                }
                break;
            }
            case SkPath::kClose_Verb: flush(); break;
            default: break;
        }
    }
    flush();
    if (edge_mode().snap_y)
        for (auto& c : out.contours)
            for (auto& q : c) q.fY = std::round(q.fY * 4.f) * 0.25f;
}


void saa_coverage(const Poly& poly, int x0, int y0, int w, int h, std::vector<int>& out) {
    out.assign((size_t)w * h, 0);
    struct E { float xa, ya, xb, yb; int dir; };
    std::vector<E> edges;
    for (auto& c : poly.contours) {
        size_t n = c.size();
        for (size_t i = 0; i < n; i++) {
            SkPoint a = c[i], b = c[(i + 1) % n];
            a.fX -= x0; a.fY -= y0; b.fX -= x0; b.fY -= y0;
            if (a.fY == b.fY) continue;
            if (a.fY < b.fY) edges.push_back({a.fX, a.fY, b.fX, b.fY, 1});
            else edges.push_back({b.fX, b.fY, a.fX, a.fY, -1});
        }
    }
    std::vector<std::pair<int, int>> xs;
    for (int j = 0; j < h * 4; j++) {
        float y = (j + 0.5f) * 0.25f;
        xs.clear();
        for (auto& e : edges) {
            if (y < e.ya || y >= e.yb) continue;
            float x = e.xa + (e.xb - e.xa) * (y - e.ya) / (e.yb - e.ya);
            xs.push_back({(int)std::floor(x * 4.f + 0.5f), e.dir});
        }
        std::sort(xs.begin(), xs.end());
        int wind = 0;
        int row = j >> 2, sub = j & 3;
        int full = 64 - ((sub + 1) >> 2);
        for (size_t k = 0; k + 1 < xs.size(); k++) {
            wind += xs[k].second;
            if (wind == 0) continue;
            int start = std::max(xs[k].first, 0), stop = std::min(xs[k + 1].first, w * 4);
            if (stop <= start) continue;
            int fb = start & 3, fe = stop & 3, n = (stop >> 2) - (start >> 2) - 1;
            int px = start >> 2;
            int* r = &out[(size_t)row * w];
            if (n < 0) {
                r[px] += (fe - fb) * 16;
                continue;
            }
            if (fb == 0) n += 1; else { r[px] += (4 - fb) * 16; px++; }
            for (int q = 0; q < n; q++) r[px++] += full;
            if (fe && px < w) r[px] += fe * 16;
        }
    }
}

void accumulate_line(std::vector<float>& acc, int w, int h, float x0, float y0, float x1, float y1) {
    if (y0 == y1) return;
    float dir = y0 < y1 ? 1.f : -1.f;
    if (y0 > y1) { std::swap(x0, x1); std::swap(y0, y1); }
    float dxdy = (x1 - x0) / (y1 - y0);
    float x = x0;
    if (y0 < 0) { x -= y0 * dxdy; y0 = 0; }
    int ys = (int)y0, ye = std::min((int)std::ceil(y1), h);
    for (int y = ys; y < ye; y++) {
        float dy = std::min((float)(y + 1), y1) - std::max((float)y, y0);
        float xnext = x + dxdy * dy;
        float d = dy * dir;
        float xa = std::min(x, xnext), xb = std::max(x, xnext);
        float* row = &acc[(size_t)y * (w + 2)];
        float x0f = std::floor(xa);
        int x0i = (int)x0f;
        float x1c = std::ceil(xb);
        int x1i = (int)x1c;
        if (x1i <= x0i + 1) {
            float xmf = 0.5f * (x + xnext) - x0f;
            if (x0i >= 0 && x0i < w + 1) {
                row[x0i] += d - d * xmf;
                row[x0i + 1] += d * xmf;
            } else if (x0i < 0) row[0] += d;
        } else {
            float s = 1.f / (xb - xa);
            float x0fr = xa - x0f;
            float a0 = 0.5f * s * (1.f - x0fr) * (1.f - x0fr);
            float x1fr = xb - x1c + 1.f;
            float am = 0.5f * s * x1fr * x1fr;
            auto put = [&](int i, float v) {
                if (i >= 0 && i < w + 1) row[i] += v;
                else if (i < 0) row[0] += v;
            };
            put(x0i, d * a0);
            if (x1i == x0i + 2) {
                put(x0i + 1, d * (1.f - a0 - am));
            } else {
                float a1 = s * (1.5f - x0fr);
                put(x0i + 1, d * (a1 - a0));
                for (int xi = x0i + 2; xi < x1i - 1; xi++) put(xi, d * s);
                float a2 = a1 + (float)(x1i - x0i - 3) * s;
                put(x1i - 1, d * (1.f - a2 - am));
            }
            put(x1i, d * am);
        }
        x = xnext;
    }
}

}  // namespace

namespace {

bool g_gpu_exact = false;

bool gpu_direct(SkCanvas& canvas) {
    return g_gpu_exact && canvas.recordingContext() != nullptr;
}

SkPaint direct_paint(const SkPaint& paint, SkPaint::Style style, float width) {
    SkPaint p(paint);
    p.setAntiAlias(true);
    p.setStyle(style);
    if (style == SkPaint::kStroke_Style) p.setStrokeWidth(width);
    return p;
}

SkPath svg_arc_path(float cx, float cy, float r, float start, float sweep) {
    const float kDeg = 57.29577951308232f;
    SkPathBuilder b;
    b.arcTo(SkRect::MakeLTRB(cx - r, cy - r, cx + r, cy + r), start * kDeg, sweep * kDeg, true);
    return b.detach();
}

}  // namespace

void set_gpu_exact(bool on) { g_gpu_exact = on; }

bool chromium_path_coverage(const SkPath& path, const SkMatrix& m, int w, int h, std::vector<float>& cov) {
    if (w <= 0 || h <= 0 || w > 16384 || h > 8192) return false;
    Poly poly;
    flatten(path, m, poly);
    std::vector<float> acc((size_t)h * (w + 2), 0.f);
    for (auto& c : poly.contours) {
        size_t n = c.size();
        for (size_t i = 0; i < n; i++) {
            SkPoint a = c[i], e = c[(i + 1) % n];
            accumulate_line(acc, w, h, a.fX, a.fY, e.fX, e.fY);
        }
    }
    cov.assign((size_t)w * h, 0.f);
    for (int y = 0; y < h; y++) {
        float run = 0.f;
        for (int x = 0; x < w; x++) {
            run += acc[(size_t)y * (w + 2) + x];
            cov[(size_t)y * w + x] = std::min(1.f, std::fabs(run));
        }
    }
    return true;
}

bool chromium_path_coverage_saa(const SkPath& path, const SkMatrix& m, int w, int h, std::vector<float>& cov) {
    if (w <= 0 || h <= 0 || w > 16384 || h > 8192) return false;
    Poly poly;
    flatten(path, m, poly);
    std::vector<int> sc;
    saa_coverage(poly, 0, 0, w, h, sc);
    cov.resize((size_t)w * h);
    for (size_t i = 0; i < cov.size(); i++) cov[i] = std::min(255, sc[i]) / 255.f;
    return true;
}

void chromium_fill_path(SkCanvas& canvas, const SkPath& path, const SkPaint& paint) {
    if (gpu_direct(canvas)) {
        draw_path_cached(canvas, path, direct_paint(paint, SkPaint::kFill_Style, 0.f));
        return;
    }
    SkMatrix m = canvas.getLocalToDeviceAs3x3();
    Poly poly;
    flatten(path, m, poly);
    SkRect b = SkRect::MakeEmpty();
    bool first = true;
    for (auto& c : poly.contours)
        for (auto& p : c) {
            if (first) b = SkRect::MakeLTRB(p.fX, p.fY, p.fX, p.fY), first = false;
            else b = SkRect::MakeLTRB(std::min(b.fLeft, p.fX), std::min(b.fTop, p.fY), std::max(b.fRight, p.fX), std::max(b.fBottom, p.fY));
        }
    if (first) return;
    int x0 = (int)std::floor(b.fLeft) - 1, y0 = (int)std::floor(b.fTop) - 1;
    int w = (int)std::ceil(b.fRight) + 1 - x0, h = (int)std::ceil(b.fBottom) + 1 - y0;
    if (w <= 0 || h <= 0 || w > 8192 || h > 8192) return;
    std::vector<float> acc((size_t)h * (w + 2), 0.f);
    for (auto& c : poly.contours) {
        size_t n = c.size();
        for (size_t i = 0; i < n; i++) {
            SkPoint a = c[i], e = c[(i + 1) % n];
            accumulate_line(acc, w, h, a.fX - x0, a.fY - y0, e.fX - x0, e.fY - y0);
        }
    }
    Mask mk(x0, y0, w, h);
    static const bool saa = a380::getenv_cached("A380_FILL_SAA") != nullptr;
    if (saa) {
        std::vector<int> sc;
        saa_coverage(poly, x0, y0, w, h, sc);
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++) mk.at(x, y) = (uint8_t)std::min(255, sc[(size_t)y * w + x]);
        blit(canvas, mk, paint);
        return;
    }
    for (int y = 0; y < h; y++) {
        float run = 0.f;
        for (int x = 0; x < w; x++) {
            run += acc[(size_t)y * (w + 2) + x];
            mk.at(x, y) = cov8(std::min(1.f, std::fabs(run)));
        }
    }
    blit(canvas, mk, paint);
}

void chromium_stroke_circle(SkCanvas& canvas, float cx, float cy, float r, float width, const SkPaint& paint) {
    if (gpu_direct(canvas)) {
        canvas.drawCircle(cx, cy, r, direct_paint(paint, SkPaint::kStroke_Style, width));
        return;
    }
    SkMatrix m = canvas.getLocalToDeviceAs3x3();
    SkPoint dc = m.mapPoint({cx, cy});
    float sc = std::sqrt(m.getScaleX() * m.getScaleX() + m.getSkewY() * m.getSkewY());
    float outer = (r + width * 0.5f) * sc, inner = std::max(0.f, (r - width * 0.5f) * sc);
    int x0 = (int)std::floor(dc.fX - outer - 1), y0 = (int)std::floor(dc.fY - outer - 1);
    int n = (int)std::ceil(2 * outer) + 3;
    Mask mk(x0, y0, n, n);
    for (int y = 0; y < n; y++)
        for (int x = 0; x < n; x++) {
            float dx = x0 + x + 0.5f - dc.fX, dy = y0 + y + 0.5f - dc.fY;
            float d = std::sqrt(dx * dx + dy * dy);
            mk.at(x, y) = cov8(std::clamp(outer - d + 0.5f, 0.f, 1.f) * std::clamp(d - inner + 0.5f, 0.f, 1.f));
        }
    blit(canvas, mk, paint);
}

void chromium_fill_circle(SkCanvas& canvas, float cx, float cy, float r, const SkPaint& paint) {
    if (gpu_direct(canvas)) {
        canvas.drawCircle(cx, cy, r, direct_paint(paint, SkPaint::kFill_Style, 0.f));
        return;
    }
    SkMatrix m = canvas.getLocalToDeviceAs3x3();
    SkPoint dc = m.mapPoint({cx, cy});
    float sc = std::sqrt(m.getScaleX() * m.getScaleX() + m.getSkewY() * m.getSkewY());
    float outer = r * sc;
    int x0 = (int)std::floor(dc.fX - outer - 1), y0 = (int)std::floor(dc.fY - outer - 1);
    int n = (int)std::ceil(2 * outer) + 3;
    Mask mk(x0, y0, n, n);
    for (int y = 0; y < n; y++)
        for (int x = 0; x < n; x++) {
            float dx = x0 + x + 0.5f - dc.fX, dy = y0 + y + 0.5f - dc.fY;
            mk.at(x, y) = cov8(outer - std::sqrt(dx * dx + dy * dy) + 0.5f);
        }
    blit(canvas, mk, paint);
}

void chromium_fill_polygon(SkCanvas& canvas, const SkPoint* pts, int n, const SkPaint& paint) {
    if (gpu_direct(canvas)) {
        SkPathBuilder b;
        b.addPolygon(SkSpan<const SkPoint>(pts, (size_t)n), true);
        draw_path_cached(canvas, b.detach(), direct_paint(paint, SkPaint::kFill_Style, 0.f));
        return;
    }
    if (n < 3) return;
    SkMatrix m = canvas.getLocalToDeviceAs3x3();
    std::vector<SkPoint> d(n);
    for (int i = 0; i < n; i++) d[i] = m.mapPoint(pts[i]);
    float area2 = 0.f;
    for (int i = 0; i < n; i++) area2 += d[i].fX * d[(i + 1) % n].fY - d[(i + 1) % n].fX * d[i].fY;
    bool convex = true, axis = true;
    float sign = area2 >= 0.f ? 1.f : -1.f;
    for (int i = 0; i < n; i++) {
        SkPoint a = d[i], b = d[(i + 1) % n], c = d[(i + 2) % n];
        float cr = (b.fX - a.fX) * (c.fY - b.fY) - (b.fY - a.fY) * (c.fX - b.fX);
        if (cr * sign < -1e-4f) convex = false;
        if (std::fabs(b.fX - a.fX) > 1e-4f && std::fabs(b.fY - a.fY) > 1e-4f) axis = false;
    }
    SkPathBuilder pb;
    pb.moveTo(pts[0]);
    for (int i = 1; i < n; i++) pb.lineTo(pts[i]);
    pb.close();
    SkPaint p(paint);
    p.setStyle(SkPaint::kFill_Style);
    if (!convex || axis || std::fabs(area2) < 1e-3f) {
        chromium_fill_path(canvas, pb.detach(), p);
        return;
    }
    float minx = d[0].fX, maxx = d[0].fX, miny = d[0].fY, maxy = d[0].fY;
    for (auto& q : d) minx = std::min(minx, q.fX), maxx = std::max(maxx, q.fX), miny = std::min(miny, q.fY), maxy = std::max(maxy, q.fY);
    int x0 = (int)std::floor(minx) - 1, y0 = (int)std::floor(miny) - 1;
    int w = (int)std::ceil(maxx) + 1 - x0, h = (int)std::ceil(maxy) + 1 - y0;
    if (w <= 0 || h <= 0 || w > 8192 || h > 8192) return;
    struct Edge { float nx, ny, c; };
    std::vector<Edge> es;
    for (int i = 0; i < n; i++) {
        SkPoint a = d[i], b = d[(i + 1) % n];
        float ex = b.fX - a.fX, ey = b.fY - a.fY, len = std::hypot(ex, ey);
        if (len < 1e-6f) continue;
        float nx = -ey / len * sign, ny = ex / len * sign;
        es.push_back({nx, ny, -(nx * a.fX + ny * a.fY)});
    }
    Mask mk(x0, y0, w, h);
    for (int y = 0; y < h; y++)
        for (int x = 0; x < w; x++) {
            float px = x0 + x + 0.5f, py = y0 + y + 0.5f, mn = 1.f;
            for (auto& e : es) mn = std::min(mn, std::clamp((e.nx * px + e.ny * py + e.c) + 0.5f, 0.f, 1.f));
            mk.at(x, y) = cov8(mn);
        }
    blit(canvas, mk, p);
}

void chromium_stroke_path(SkCanvas& canvas, const SkPath& path, const SkPaint& paint) {
    if (gpu_direct(canvas)) {
        draw_path_cached(canvas, path, direct_paint(paint, SkPaint::kStroke_Style, paint.getStrokeWidth()));
        return;
    }
    SkPaint p(paint);
    if (p.getStyle() == SkPaint::kFill_Style) {
        chromium_fill_path(canvas, path, p);
        return;
    }
    SkPathBuilder out;
    SkMatrix fine = canvas.getLocalToDeviceAs3x3();
    float fres = 8.f;
    if (const char* v = a380::getenv_cached("A380_STROKE_RES")) fres = (float)std::atof(v);
    fine.postScale(fres, fres);
    bool fill = skpathutils::FillPathWithPaint(path, p, &out, nullptr, fine);
    if (!fill) {
        canvas.drawPath(path, paint);
        return;
    }
    SkPaint q(paint);
    q.setStyle(SkPaint::kFill_Style);
    q.setPathEffect(nullptr);
    chromium_fill_path(canvas, out.detach(), q);
}

void chromium_stroke_line(SkCanvas& canvas, float x0, float y0, float x1, float y1, float width, const SkPaint& paint) {
    if (gpu_direct(canvas)) {
        canvas.drawLine(x0, y0, x1, y1, direct_paint(paint, SkPaint::kStroke_Style, width));
        return;
    }
    SkPaint p(paint);
    p.setStyle(SkPaint::kStroke_Style);
    p.setStrokeWidth(width);
    SkMatrix m = canvas.getLocalToDeviceAs3x3();
    float sc = std::sqrt(m.getScaleX() * m.getScaleX() + m.getSkewY() * m.getSkewY());
    float len = std::hypot(x1 - x0, y1 - y0);
    if (paint.getStrokeCap() == SkPaint::kButt_Cap && (x0 == x1 || y0 == y1) && len > 0.f) {
        chromium_fill_rect(canvas, SkRect::MakeLTRB(std::min(x0, x1) - (y0 == y1 ? 0.f : width * 0.5f), std::min(y0, y1) - (x0 == x1 ? 0.f : width * 0.5f), std::max(x0, x1) + (y0 == y1 ? 0.f : width * 0.5f), std::max(y0, y1) + (x0 == x1 ? 0.f : width * 0.5f)), p);
        return;
    }
    if (paint.getStrokeCap() == SkPaint::kButt_Cap && width * sc >= 1.f && len > 0.f) {
        float nx = -(y1 - y0) / len * width * 0.5f, ny = (x1 - x0) / len * width * 0.5f;
        SkPoint q[4] = {{x0 + nx, y0 + ny}, {x1 + nx, y1 + ny}, {x1 - nx, y1 - ny}, {x0 - nx, y0 - ny}};
        chromium_fill_polygon(canvas, q, 4, p);
        return;
    }
    chromium_stroke_path(canvas, SkPath::Line({x0, y0}, {x1, y1}), p);
}

void chromium_stroke_line_dashed(SkCanvas& canvas, float x0, float y0, float x1, float y1, float width, const float* dashes, int count,
                                 float phase, const SkPaint& paint) {
    if (gpu_direct(canvas)) {
        SkPaint p = direct_paint(paint, SkPaint::kStroke_Style, width);
        p.setPathEffect(SkDashPathEffect::Make(SkSpan<const SkScalar>(dashes, (size_t)count), phase));
        canvas.drawLine(x0, y0, x1, y1, p);
        return;
    }
    SkPaint p(paint);
    p.setStyle(SkPaint::kStroke_Style);
    p.setStrokeWidth(width);
    p.setPathEffect(SkDashPathEffect::Make(SkSpan<const SkScalar>(dashes, count), phase));
    chromium_stroke_path(canvas, SkPath::Line({x0, y0}, {x1, y1}), p);
}

namespace {

bool axis_aligned_rect(SkCanvas& canvas, const SkRect& r, SkRect& dev) {
    static const bool exact = a380::getenv_cached("A380_RECT_EXACT") != nullptr;
    SkMatrix m = canvas.getLocalToDeviceAs3x3();
    if (exact || m.getSkewX() != 0.f || m.getSkewY() != 0.f || m.hasPerspective() || m.getScaleX() <= 0.f || m.getScaleY() <= 0.f) return false;
    dev = m.mapRect(r);
    return true;
}

inline float axis_overlap(int p, float lo, float hi) { return std::clamp(std::min((float)p + 1.f, hi) - std::max((float)p, lo), 0.f, 1.f); }

}  // namespace

void chromium_fill_rect(SkCanvas& canvas, const SkRect& rect, const SkPaint& paint) {
    if (gpu_direct(canvas)) {
        canvas.drawRect(rect, direct_paint(paint, SkPaint::kFill_Style, 0.f));
        return;
    }
    SkPaint p(paint);
    p.setStyle(SkPaint::kFill_Style);
    SkRect d;
    if (!axis_aligned_rect(canvas, rect, d) || d.isEmpty()) {
        chromium_fill_path(canvas, SkPath::Rect(rect), p);
        return;
    }
    int X0 = (int)std::floor(d.fLeft), X1 = (int)std::ceil(d.fRight), Y0 = (int)std::floor(d.fTop), Y1 = (int)std::ceil(d.fBottom);
    Mask mask(X0, Y0, X1 - X0, Y1 - Y0);
    for (int y = Y0; y < Y1; y++) {
        float cy = axis_overlap(y, d.fTop, d.fBottom);
        for (int x = X0; x < X1; x++) mask.at(x - X0, y - Y0) = cov8(std::min(cy, axis_overlap(x, d.fLeft, d.fRight)));
    }
    blit(canvas, mask, p);
}

void chromium_stroke_rect(SkCanvas& canvas, const SkRect& rect, float width, const SkPaint& paint) {
    if (gpu_direct(canvas)) {
        canvas.drawRect(rect, direct_paint(paint, SkPaint::kStroke_Style, width));
        return;
    }
    SkPaint p(paint);
    p.setStyle(SkPaint::kFill_Style);
    SkRect outer = rect.makeOutset(width * 0.5f, width * 0.5f), inner = rect.makeInset(width * 0.5f, width * 0.5f);
    bool has_inner = inner.fRight > inner.fLeft && inner.fBottom > inner.fTop;
    SkRect od, id = SkRect::MakeEmpty();
    if (!axis_aligned_rect(canvas, outer, od) || (has_inner && !axis_aligned_rect(canvas, inner, id))) {
        SkPathBuilder b;
        b.moveTo(outer.fLeft, outer.fTop).lineTo(outer.fRight, outer.fTop).lineTo(outer.fRight, outer.fBottom).lineTo(outer.fLeft, outer.fBottom).close();
        if (has_inner)
            b.moveTo(inner.fLeft, inner.fTop).lineTo(inner.fLeft, inner.fBottom).lineTo(inner.fRight, inner.fBottom).lineTo(inner.fRight, inner.fTop).close();
        chromium_fill_path(canvas, b.detach(), p);
        return;
    }
    int X0 = (int)std::floor(od.fLeft), X1 = (int)std::ceil(od.fRight), Y0 = (int)std::floor(od.fTop), Y1 = (int)std::ceil(od.fBottom);
    Mask mask(X0, Y0, X1 - X0, Y1 - Y0);
    for (int y = Y0; y < Y1; y++) {
        float oy = axis_overlap(y, od.fTop, od.fBottom), iy = has_inner ? axis_overlap(y, id.fTop, id.fBottom) : 0.f;
        for (int x = X0; x < X1; x++) {
            float o = std::min(oy, axis_overlap(x, od.fLeft, od.fRight));
            float in = has_inner ? std::min(iy, axis_overlap(x, id.fLeft, id.fRight)) : 0.f;
            mask.at(x - X0, y - Y0) = cov8(o - in);
        }
    }
    blit(canvas, mask, p);
}

SkPath chromium_arc_path(float cx, float cy, float r, float start, float sweep, float device_scale) {
    if (g_gpu_exact) return svg_arc_path(cx, cy, r, start, sweep);
    const double kPi = 3.14159265358979323846;
    int segments = std::max(1, (int)std::ceil(std::fabs((double)sweep) / (kPi / 2 + 0.001)));
    SkPathBuilder b;
    b.moveTo(cx + r * (float)std::cos(start), cy + r * (float)std::sin(start));
    for (int i = 0; i < segments; i++) {
        double s = start + i * (double)sweep / segments, e = start + (i + 1) * (double)sweep / segments;
        double t = (4.0 / 3.0) * std::tan(0.25 * (e - s));
        double ss = std::sin(s), cs = std::cos(s), se = std::sin(e), ce = std::cos(e);
        SkPoint p0{cx + r * (float)cs, cy + r * (float)ss};
        SkPoint p1{cx + r * (float)(cs - t * ss), cy + r * (float)(ss + t * cs)};
        SkPoint p2{cx + r * (float)(ce + t * se), cy + r * (float)(se - t * ce)};
        SkPoint p3{cx + r * (float)ce, cy + r * (float)se};
        SkPoint d1 = p0 - p1 * 2 + p2, d2 = p1 - p2 * 2 + p3;
        float m = std::max(d1.length(), d2.length()) * device_scale;
        static const float tol = a380::getenv_cached("A380_ARC_TOL") ? (float)std::atof(a380::getenv_cached("A380_ARC_TOL")) : 0.17f;
        int n = std::max(1, (int)std::ceil(std::sqrt(0.75f * m / tol)));
        for (int k = 1; k <= n; k++) {
            float u = (float)k / n, w = 1 - u;
            b.lineTo(w * w * w * p0.fX + 3 * w * w * u * p1.fX + 3 * w * u * u * p2.fX + u * u * u * p3.fX,
                     w * w * w * p0.fY + 3 * w * w * u * p1.fY + 3 * w * u * u * p2.fY + u * u * u * p3.fY);
        }
    }
    return b.detach();
}

namespace {

struct ArcTess {
    float precision = 4.f;
    int mode = 0;
};

const ArcTess& arc_tess() {
    static const ArcTess t = [] {
        ArcTess a;
        if (const char* v = a380::getenv_cached("A380_ARC_TESS")) {
            a.mode = 1;
            a.precision = (float)std::atof(v);
        }
        return a;
    }();
    return t;
}

bool tessellated_arc(SkCanvas& canvas, float cx, float cy, float r, float start, float sweep, const SkPaint& paint, float precision) {
    SkMatrix m = canvas.getLocalToDeviceAs3x3();
    float sc = std::sqrt(m.getScaleX() * m.getScaleX() + m.getSkewY() * m.getSkewY());
    if (m.getSkewX() != 0.f || m.getSkewY() != 0.f || m.hasPerspective() || sc <= 0.f || std::fabs(sweep) >= 6.2831f) return false;
    const SkPaint::Cap cap = paint.getStrokeCap();
    if (cap != SkPaint::kButt_Cap && !a380::getenv_cached("A380_ARC_CAPS")) return false;
    const double kPi = 3.14159265358979323846;
    int segments = std::max(1, (int)std::ceil(std::fabs((double)sweep) / (kPi / 2 + 0.001)));
    float hw = paint.getStrokeWidth() * 0.5f;
    SkPoint tan_first{0, 0}, tan_last{0, 0}, centre_first{0, 0}, centre_last{0, 0};
    std::vector<SkPoint> outer, inner;
    for (int i = 0; i < segments; i++) {
        double s = start + i * (double)sweep / segments, e = start + (i + 1) * (double)sweep / segments;
        double t = (4.0 / 3.0) * std::tan(0.25 * (e - s));
        double ss = std::sin(s), cs = std::cos(s), se = std::sin(e), ce = std::cos(e);
        SkPoint p0{cx + r * (float)cs, cy + r * (float)ss};
        SkPoint p1{cx + r * (float)(cs - t * ss), cy + r * (float)(ss + t * cs)};
        SkPoint p2{cx + r * (float)(ce + t * se), cy + r * (float)(se - t * ce)};
        SkPoint p3{cx + r * (float)ce, cy + r * (float)se};
        SkPoint d1 = (p0 - p1 * 2 + p2) * sc, d2 = (p1 - p2 * 2 + p3) * sc;
        float mm = std::max(d1.length(), d2.length());
        int n = std::max(1, (int)std::ceil(std::sqrt(0.75f * mm * precision)));
        if (const char* fn = a380::getenv_cached("A380_ARC_N")) n = std::atoi(fn);
        std::vector<float> ts;
        for (int k = 0; k <= n; k++) ts.push_back((float)k / n);
        if (a380::getenv_cached("A380_ARC_RAD")) {
            float hwd = hw * sc;
            float perrad = 1.f / std::acos(1.f - 1.f / (std::max(1.f, (float)std::atof(a380::getenv_cached("A380_ARC_RAD"))) * std::max(hwd, 0.3f)));
            float total = (float)std::fabs(e - s);
            int nr = std::max(1, (int)std::ceil(total * perrad));
            auto ang = [&](float u) {
                float w = 1 - u;
                float dxv = 3 * w * w * (p1.fX - p0.fX) + 6 * w * u * (p2.fX - p1.fX) + 3 * u * u * (p3.fX - p2.fX);
                float dyv = 3 * w * w * (p1.fY - p0.fY) + 6 * w * u * (p2.fY - p1.fY) + 3 * u * u * (p3.fY - p2.fY);
                float l1 = std::hypot(dxv, dyv), l0 = std::hypot(p1.fX - p0.fX, p1.fY - p0.fY);
                return std::acos(std::clamp((dxv * (p1.fX - p0.fX) + dyv * (p1.fY - p0.fY)) / (l1 * l0), -1.f, 1.f));
            };
            for (int k = 1; k < nr; k++) {
                float target = total * k / nr, lo = 0.f, hi = 1.f;
                for (int it = 0; it < 30; it++) {
                    float mid = 0.5f * (lo + hi);
                    if (ang(mid) < target) lo = mid; else hi = mid;
                }
                ts.push_back(0.5f * (lo + hi));
            }
            std::sort(ts.begin(), ts.end());
        }
        for (size_t kk = (i == 0 ? 0 : 1); kk < ts.size(); kk++) {
            float u = ts[kk], w = 1 - u;
            SkPoint pt{w * w * w * p0.fX + 3 * w * w * u * p1.fX + 3 * w * u * u * p2.fX + u * u * u * p3.fX,
                       w * w * w * p0.fY + 3 * w * w * u * p1.fY + 3 * w * u * u * p2.fY + u * u * u * p3.fY};
            SkPoint d{3 * w * w * (p1.fX - p0.fX) + 6 * w * u * (p2.fX - p1.fX) + 3 * u * u * (p3.fX - p2.fX),
                      3 * w * w * (p1.fY - p0.fY) + 6 * w * u * (p2.fY - p1.fY) + 3 * u * u * (p3.fY - p2.fY)};
            float len = d.length();
            if (len <= 0.f) continue;
            SkPoint nrm{-d.fY / len, d.fX / len};
            static const float eo = a380::getenv_cached("A380_ARC_DO") ? (float)std::atof(a380::getenv_cached("A380_ARC_DO")) / 1.5f : 0.f;
            static const float ei = a380::getenv_cached("A380_ARC_DI") ? (float)std::atof(a380::getenv_cached("A380_ARC_DI")) / 1.5f : 0.f;
            if (outer.empty()) { tan_first = {d.fX / len, d.fY / len}; centre_first = pt; }
            tan_last = {d.fX / len, d.fY / len};
            centre_last = pt;
            outer.push_back({pt.fX + nrm.fX * (hw + eo), pt.fY + nrm.fY * (hw + eo)});
            inner.push_back({pt.fX - nrm.fX * (hw + ei), pt.fY - nrm.fY * (hw + ei)});
        }
    }
    if (outer.size() < 2) return false;
    if (cap != SkPaint::kButt_Cap) {
        std::vector<SkPoint> capEnd, capStart;
        auto semicircle = [&](SkPoint centre, SkPoint tg, std::vector<SkPoint>& dst, bool atEnd) {
            SkPoint nrm{-tg.fY, tg.fX};
            if (cap == SkPaint::kSquare_Cap) {
                float sgn = atEnd ? 1.f : -1.f;
                SkPoint a{centre.fX + nrm.fX * hw + tg.fX * hw * sgn, centre.fY + nrm.fY * hw + tg.fY * hw * sgn};
                SkPoint c{centre.fX - nrm.fX * hw + tg.fX * hw * sgn, centre.fY - nrm.fY * hw + tg.fY * hw * sgn};
                if (atEnd) { dst.push_back(a); dst.push_back(c); } else { dst.push_back(c); dst.push_back(a); }
                return;
            }
            float hwd = hw * sc;
            int m = std::max(2, (int)std::ceil(3.14159265f / std::acos(std::clamp(1.f - 1.f / (4.f * std::max(hwd, 0.3f)), -1.f, 1.f))));
            for (int k = 1; k < m; k++) {
                float a = 3.14159265f * k / m;
                float ca = std::cos(a), sa = std::sin(a);
                SkPoint dir = atEnd ? SkPoint{nrm.fX * ca + tg.fX * sa, nrm.fY * ca + tg.fY * sa} : SkPoint{-nrm.fX * ca - tg.fX * sa, -nrm.fY * ca - tg.fY * sa};
                dst.push_back({centre.fX + dir.fX * hw, centre.fY + dir.fY * hw});
            }
        };
        semicircle(centre_last, tan_last, capEnd, true);
        semicircle(centre_first, tan_first, capStart, false);
        std::vector<SkPoint> poly(outer);
        poly.insert(poly.end(), capEnd.begin(), capEnd.end());
        for (size_t i = inner.size(); i-- > 0;) poly.push_back(inner[i]);
        poly.insert(poly.end(), capStart.begin(), capStart.end());
        SkPathBuilder b;
        b.moveTo(poly[0]);
        for (size_t i = 1; i < poly.size(); i++) b.lineTo(poly[i]);
        b.close();
        SkPaint q(paint);
        q.setStyle(SkPaint::kFill_Style);
        chromium_fill_path(canvas, b.detach(), q);
        return true;
    }
    if (a380::getenv_cached("A380_ARC_SD")) {
        std::vector<SkPoint> poly(outer);
        for (size_t i = inner.size(); i-- > 0;) poly.push_back(inner[i]);
        std::vector<SkPoint> dev(poly.size());
        for (size_t i = 0; i < poly.size(); i++) dev[i] = m.mapPoint(poly[i]);
        float minx = dev[0].fX, maxx = minx, miny = dev[0].fY, maxy = miny;
        for (auto& q : dev) { minx = std::min(minx, q.fX); maxx = std::max(maxx, q.fX); miny = std::min(miny, q.fY); maxy = std::max(maxy, q.fY); }
        int X0 = (int)std::floor(minx) - 2, Y0 = (int)std::floor(miny) - 2, X1 = (int)std::ceil(maxx) + 2, Y1 = (int)std::ceil(maxy) + 2;
        Mask mask(X0, Y0, X1 - X0, Y1 - Y0);
        size_t np = dev.size();
        for (int y = Y0; y < Y1; y++)
            for (int x = X0; x < X1; x++) {
                float px = x + 0.5f, py = y + 0.5f, best = 1e30f;
                bool inside = false;
                for (size_t i = 0; i < np; i++) {
                    SkPoint a = dev[i], c = dev[(i + 1) % np];
                    float ex = c.fX - a.fX, ey = c.fY - a.fY, l2 = ex * ex + ey * ey;
                    float t = l2 > 0.f ? std::clamp(((px - a.fX) * ex + (py - a.fY) * ey) / l2, 0.f, 1.f) : 0.f;
                    float qx = a.fX + t * ex - px, qy = a.fY + t * ey - py;
                    best = std::min(best, qx * qx + qy * qy);
                    if ((a.fY > py) != (c.fY > py) && px < a.fX + (py - a.fY) * ex / ey) inside = !inside;
                }
                float sd = (inside ? 1.f : -1.f) * std::sqrt(best);
                mask.at(x - X0, y - Y0) = cov8(sd + 0.5f);
            }
        SkPaint q(paint);
        q.setStyle(SkPaint::kFill_Style);
        blit(canvas, mask, q);
        return true;
    }
    SkPathBuilder b;
    b.moveTo(outer[0]);
    for (size_t i = 1; i < outer.size(); i++) b.lineTo(outer[i]);
    for (size_t i = inner.size(); i-- > 0;) b.lineTo(inner[i]);
    b.close();
    SkPaint q(paint);
    q.setStyle(SkPaint::kFill_Style);
    chromium_fill_path(canvas, b.detach(), q);
    return true;
}


void fill_polygon_distance(SkCanvas& canvas, const std::vector<SkPoint>& poly, const SkPaint& paint, float half_ramp) {
    SkMatrix m = canvas.getLocalToDeviceAs3x3();
    std::vector<SkPoint> dev(poly.size());
    for (size_t i = 0; i < poly.size(); i++) dev[i] = m.mapPoint(poly[i]);
    float minx = dev[0].fX, maxx = minx, miny = dev[0].fY, maxy = miny;
    for (auto& q : dev) { minx = std::min(minx, q.fX); maxx = std::max(maxx, q.fX); miny = std::min(miny, q.fY); maxy = std::max(maxy, q.fY); }
    int X0 = (int)std::floor(minx) - 2, Y0 = (int)std::floor(miny) - 2, X1 = (int)std::ceil(maxx) + 2, Y1 = (int)std::ceil(maxy) + 2;
    Mask mask(X0, Y0, X1 - X0, Y1 - Y0);
    size_t np = dev.size();
    for (int y = Y0; y < Y1; y++)
        for (int x = X0; x < X1; x++) {
            float px = x + 0.5f, py = y + 0.5f, best = 1e30f;
            bool inside = false;
            for (size_t i = 0; i < np; i++) {
                SkPoint a = dev[i], c = dev[(i + 1) % np];
                float ex = c.fX - a.fX, ey = c.fY - a.fY, l2 = ex * ex + ey * ey;
                float t = l2 > 0.f ? std::clamp(((px - a.fX) * ex + (py - a.fY) * ey) / l2, 0.f, 1.f) : 0.f;
                float qx = a.fX + t * ex - px, qy = a.fY + t * ey - py;
                best = std::min(best, qx * qx + qy * qy);
                if ((a.fY > py) != (c.fY > py) && px < a.fX + (py - a.fY) * ex / ey) inside = !inside;
            }
            float sd = (inside ? 1.f : -1.f) * std::sqrt(best);
            mask.at(x - X0, y - Y0) = cov8(0.5f + sd * (0.5f / half_ramp));
        }
    SkPaint q(paint);
    q.setStyle(SkPaint::kFill_Style);
    blit(canvas, mask, q);
}

namespace gst {


struct V2 { float x, y; };
inline V2 operator+(V2 a, V2 b) { return {a.x + b.x, a.y + b.y}; }
inline V2 operator-(V2 a, V2 b) { return {a.x - b.x, a.y - b.y}; }
inline V2 operator*(V2 a, float s) { return {a.x * s, a.y * s}; }
inline bool operator==(V2 a, V2 b) { return a.x == b.x && a.y == b.y; }
inline float dot(V2 a, V2 b) { return a.x * b.x + a.y * b.y; }
inline float cross(V2 a, V2 b) { return a.x * b.y - a.y * b.x; }
inline V2 fma2(V2 a, V2 b, V2 c) { return {std::fma(a.x, b.x, c.x), std::fma(a.y, b.y, c.y)}; }
inline V2 fmas(V2 a, float s, V2 c) { return {std::fma(a.x, s, c.x), std::fma(a.y, s, c.y)}; }
inline V2 normalize(V2 a) { float l = std::sqrt(dot(a, a)); return {a.x / l, a.y / l}; }
inline V2 robust_normalize_diff(V2 a, V2 b) {
    V2 d = a - b;
    if (d.x == 0.f && d.y == 0.f) return {0.f, 0.f};
    float inv = 1.f / std::max(std::fabs(d.x), std::fabs(d.y));
    return normalize(d * inv);
}
inline V2 umix(V2 a, V2 b, float t) { return fmas(b - a, t, a); }

struct Edge { V2 pos, tan; };

constexpr float kPrecision = 4.f;
constexpr float kPi = 3.141592653589793238f;

inline float radial_per_radian(float dev_radius) {
    return .5f / std::acos(std::max(1.f - (1.f / kPrecision) / dev_radius, -1.f));
}

inline float wang_cubic(V2 p0, V2 p1, V2 p2, V2 p3, float a, float b, float c, float d) {
    auto xf = [&](V2 v) { return V2{a * v.x + c * v.y, b * v.x + d * v.y}; };
    V2 d0 = xf(fma2({-2.f, -2.f}, p1, p2) + p0), d1 = xf(fma2({-2.f, -2.f}, p2, p3) + p1);
    float m = std::max(dot(d0, d0), dot(d1, d1));
    return std::max(std::ceil(std::sqrt(0.75f * kPrecision * std::sqrt(m))), 1.f);
}

inline void stroke_edges(V2 p0, V2 p1, V2 p2, V2 p3, float a, float b, float c, float d, float stroke_radius,
                         float rad_per_rad, std::vector<Edge>& out) {
    float P = (p0 == p1 && p2 == p3) ? 1.f : wang_cubic(p0, p1, p2, p3, a, b, c, d);
    V2 tan0 = robust_normalize_diff((p0 == p1) ? ((p1 == p2) ? p3 : p2) : p1, p0);
    V2 tan1 = robust_normalize_diff(p3, (p3 == p2) ? ((p2 == p1) ? p0 : p1) : p2);
    if (tan0.x == 0.f && tan0.y == 0.f) { tan0 = {1.f, 0.f}; tan1 = {-1.f, 0.f}; }
    float turn = cross(p2 - p0, p3 - p1);
    float cosTheta = std::clamp(dot(tan0, tan1), -1.f, 1.f);
    float rotation = std::acos(cosTheta);
    if (turn < 0.f) rotation = -rotation;
    float R = std::max(std::ceil(std::fabs(rotation) * rad_per_rad), 1.f);
    float rps = rotation / R;
    float numCombined = P + R - 1.f;
    V2 A, B, C = p1 - p0;
    V2 D = p3 - p0;
    V2 E = p2 - p1;
    B = E - C;
    A = fma2({-3.f, -3.f}, E, D);
    V2 B_ = B * (P * 2.f);
    V2 C_ = C * (P * P);
    float angle0 = std::acos(std::clamp(tan0.x, -1.f, 1.f));
    angle0 = tan0.y >= 0.f ? angle0 : -angle0;
    for (int e = 0; e <= (int)numCombined; e++) {
        float eid = (float)e;
        Edge ed;
        if (e != 0 && e < (int)numCombined) {
            float lastP = 0.f;
            float maxP = std::min(P - 1.f, eid);
            float negAbs = -std::fabs(rps);
            float maxRot0 = (1.f + eid) * std::fabs(rps);
            for (int ex = 4; ex >= 0; --ex) {
                float test = lastP + std::exp2((float)ex);
                if (test <= maxP) {
                    V2 tt = fmas(A, test, B_);
                    tt = fmas(tt, test, C_);
                    float cosRot = dot(normalize(tt), tan0);
                    float maxRot = std::fma(test, negAbs, maxRot0);
                    maxRot = std::min(maxRot, kPi);
                    if (cosRot >= std::cos(maxRot)) lastP = test;
                }
            }
            float parT = lastP / P;
            float lastR = eid - lastP;
            float radialAngle = std::fma(lastR, rps, angle0);
            V2 tangent{std::cos(radialAngle), std::sin(radialAngle)};
            V2 norm{-tangent.y, tangent.x};
            float qa = dot(norm, A), bo2 = dot(norm, B), qc = dot(norm, C);
            float discr = std::max(bo2 * bo2 - qa * qc, 0.f);
            float q = std::sqrt(discr);
            if (bo2 > 0.f) q = -q;
            q -= bo2;
            float _5qa = -.5f * q * qa;
            bool pickQA = std::fabs(std::fma(q, q, _5qa)) < std::fabs(std::fma(qa, qc, _5qa));
            float rs = pickQA ? q : qc, rt = pickQA ? qa : q;
            float radialT = (rt != 0.f) ? rs / rt : 0.f;
            radialT = std::clamp(radialT, 0.f, 1.f);
            if (lastR == 0.f) radialT = 0.f;
            float T = std::max(parT, radialT);
            V2 ab = umix(p0, p1, T), bc = umix(p1, p2, T), cd = umix(p2, p3, T);
            V2 abc = umix(ab, bc, T), bcd = umix(bc, cd, T), abcd = umix(abc, bcd, T);
            if (T != radialT) tangent = robust_normalize_diff(bcd, abc);
            ed.pos = abcd;
            ed.tan = tangent;
        } else {
            ed.tan = (e == 0) ? tan0 : tan1;
            ed.pos = (e == 0) ? p0 : p3;
        }
        out.push_back(ed);
    }
}

}  // namespace gst

struct TessCubic {
    SkPoint p[4];
    SkPoint pos(float t) const {
        float w = 1 - t;
        return {w * w * w * p[0].fX + 3 * w * w * t * p[1].fX + 3 * w * t * t * p[2].fX + t * t * t * p[3].fX,
                w * w * w * p[0].fY + 3 * w * w * t * p[1].fY + 3 * w * t * t * p[2].fY + t * t * t * p[3].fY};
    }
    SkPoint tan(float t) const {
        float w = 1 - t;
        SkPoint d{3 * w * w * (p[1].fX - p[0].fX) + 6 * w * t * (p[2].fX - p[1].fX) + 3 * t * t * (p[3].fX - p[2].fX),
                  3 * w * w * (p[1].fY - p[0].fY) + 6 * w * t * (p[2].fY - p[1].fY) + 3 * t * t * (p[3].fY - p[2].fY)};
        float l = d.length();
        if (l <= 0.f) {
            d = {p[3].fX - p[0].fX, p[3].fY - p[0].fY};
            l = d.length();
        }
        return l > 0.f ? SkPoint{d.fX / l, d.fY / l} : SkPoint{1, 0};
    }
};

float signed_rotation(SkPoint a, SkPoint b) { return std::atan2(a.fX * b.fY - a.fY * b.fX, a.fX * b.fX + a.fY * b.fY); }

const float* tess_param(const char* name, float def) {
    thread_local float v[8];
    thread_local bool init = false;
    if (!init) {
        init = true;
        v[0] = 4.f; v[1] = 1.f; v[2] = 2.f;
        if (const char* e = a380::getenv_cached("A380_ARC_P")) v[0] = (float)std::atof(e);
        if (const char* e = a380::getenv_cached("A380_ARC_RF")) v[1] = (float)std::atof(e);
        if (const char* e = a380::getenv_cached("A380_ARC_RP")) v[2] = (float)std::atof(e);
    }
    (void)name; (void)def;
    return v;
}

void tess_cubic_vertices(const TessCubic& c, float scale, float hw_dev, std::vector<float>& ts) {
    const float* prm = tess_param("", 0.f);
    SkPoint d1 = c.p[0] - c.p[1] * 2 + c.p[2], d2 = c.p[1] - c.p[2] * 2 + c.p[3];
    float M = std::max(d1.length(), d2.length()) * scale;
    int P = std::max(1, (int)std::ceil(std::sqrt(0.75f * prm[0] * M)));
    SkPoint t0 = c.tan(0.f);
    float total = std::fabs(signed_rotation(t0, c.tan(1.f)));
    float per_rad = prm[1] / std::acos(std::max(1.f - 1.f / (prm[2] * std::max(hw_dev, 1e-3f)), -1.f));
    int Nr = std::max(1, (int)std::ceil(total * per_rad));
    float rps = total / Nr;
    std::vector<float> rot(P + 1);
    for (int i = 0; i <= P; i++) rot[i] = std::fabs(signed_rotation(t0, c.tan((float)i / P)));
    int E = P + Nr - 1;
    for (int e = 0; e <= E; e++) {
        int p = 0;
        for (int q = std::min(P - 1, e); q >= 0; q--)
            if (rot[q] <= (e - q + 1) * rps + 1e-6f) { p = q; break; }
        float target = (e - p) * rps;
        float T;
        if (e >= E) T = 1.f;
        else if (rot[p] >= target) T = (float)p / P;
        else {
            float lo = (float)p / P, hi = (float)(p + 1) / P;
            for (int it = 0; it < 40; it++) {
                float mid = 0.5f * (lo + hi);
                if (std::fabs(signed_rotation(t0, c.tan(mid))) < target) lo = mid; else hi = mid;
            }
            T = 0.5f * (lo + hi);
        }
        ts.push_back(T);
    }
}

bool tess_stroke_cubics(SkCanvas& canvas, const std::vector<TessCubic>& cubics, const SkPaint& paint) {
    SkMatrix m = canvas.getLocalToDeviceAs3x3();
    float sc = std::sqrt(m.getScaleX() * m.getScaleX() + m.getSkewY() * m.getSkewY());
    if (m.getSkewX() != 0.f || m.getSkewY() != 0.f || m.hasPerspective() || sc <= 0.f || cubics.empty()) return false;
    const float hw = paint.getStrokeWidth() * 0.5f;
    const float hw_dev = hw * sc;
    std::vector<SkPoint> outer, inner;
    SkPoint first_tan{1, 0}, last_tan{1, 0}, first_pt{0, 0}, last_pt{0, 0};
    bool have = false;
    static const bool gpu_edges = a380::getenv_cached("A380_ARC_NOGPU") == nullptr;
    for (size_t ci = 0; ci < cubics.size(); ci++) {
        if (gpu_edges) {
            const auto& cb = cubics[ci];
            std::vector<gst::Edge> eds;
            gst::stroke_edges({cb.p[0].fX, cb.p[0].fY}, {cb.p[1].fX, cb.p[1].fY}, {cb.p[2].fX, cb.p[2].fY}, {cb.p[3].fX, cb.p[3].fY},
                              m.getScaleX(), m.getSkewY(), m.getSkewX(), m.getScaleY(), hw, gst::radial_per_radian(sc * hw), eds);
            for (size_t k = (ci == 0 ? 0 : 1); k < eds.size(); k++) {
                SkPoint pt{eds[k].pos.x, eds[k].pos.y}, tg{eds[k].tan.x, eds[k].tan.y};
                SkPoint nr{tg.fY, -tg.fX};
                outer.push_back({pt.fX - nr.fX * hw, pt.fY - nr.fY * hw});
                inner.push_back({pt.fX + nr.fX * hw, pt.fY + nr.fY * hw});
                if (!have) { first_tan = tg; first_pt = pt; have = true; }
                last_tan = tg;
                last_pt = pt;
            }
            continue;
        }
        std::vector<float> ts;
        tess_cubic_vertices(cubics[ci], sc, hw_dev, ts);
        for (size_t k = (ci == 0 ? 0 : 1); k < ts.size(); k++) {
            SkPoint pt = cubics[ci].pos(ts[k]), tg = cubics[ci].tan(ts[k]);
            SkPoint nr{-tg.fY, tg.fX};
            outer.push_back({pt.fX + nr.fX * hw, pt.fY + nr.fY * hw});
            inner.push_back({pt.fX - nr.fX * hw, pt.fY - nr.fY * hw});
            if (!have) { first_tan = tg; first_pt = pt; have = true; }
            last_tan = tg;
            last_pt = pt;
        }
    }
    if (outer.size() < 2) return false;
    const SkPaint::Cap cap = paint.getStrokeCap();
    std::vector<SkPoint> cap_end, cap_start;
    auto make_cap = [&](SkPoint centre, SkPoint tg, bool at_end, std::vector<SkPoint>& dst) {
        SkPoint nr{-tg.fY, tg.fX};
        float sgn = at_end ? 1.f : -1.f;
        if (cap == SkPaint::kSquare_Cap) {
            SkPoint a{centre.fX + nr.fX * hw + tg.fX * hw * sgn, centre.fY + nr.fY * hw + tg.fY * hw * sgn};
            SkPoint b{centre.fX - nr.fX * hw + tg.fX * hw * sgn, centre.fY - nr.fY * hw + tg.fY * hw * sgn};
            if (at_end) { dst.push_back(a); dst.push_back(b); } else { dst.push_back(b); dst.push_back(a); }
        } else if (cap == SkPaint::kRound_Cap) {
            const float* prm = tess_param("", 0.f);
            float per_rad = prm[1] / std::acos(std::max(1.f - 1.f / (prm[2] * std::max(hw_dev, 1e-3f)), -1.f));
            int n = std::max(1, (int)std::ceil(3.14159265f * per_rad));
            for (int k = 1; k < n; k++) {
                float a = 3.14159265f * k / n, ca = std::cos(a), sa = std::sin(a);
                SkPoint dir = at_end ? SkPoint{nr.fX * ca + tg.fX * sa, nr.fY * ca + tg.fY * sa} : SkPoint{-nr.fX * ca - tg.fX * sa, -nr.fY * ca - tg.fY * sa};
                dst.push_back({centre.fX + dir.fX * hw, centre.fY + dir.fY * hw});
            }
        }
    };
    make_cap(last_pt, last_tan, true, cap_end);
    make_cap(first_pt, first_tan, false, cap_start);
    SkPathBuilder b;
    b.moveTo(outer[0]);
    for (size_t i = 1; i < outer.size(); i++) b.lineTo(outer[i]);
    for (auto& q : cap_end) b.lineTo(q);
    for (size_t i = inner.size(); i-- > 0;) b.lineTo(inner[i]);
    for (auto& q : cap_start) b.lineTo(q);
    b.close();
    if (const char* cv = a380::getenv_cached("A380_ARC_COV")) {
        std::vector<SkPoint> poly(outer);
        for (auto& q : cap_end) poly.push_back(q);
        for (size_t i = inner.size(); i-- > 0;) poly.push_back(inner[i]);
        for (auto& q : cap_start) poly.push_back(q);
        fill_polygon_distance(canvas, poly, paint, (float)std::atof(cv));
        return true;
    }
    SkPaint q(paint);
    q.setStyle(SkPaint::kFill_Style);
    chromium_fill_path(canvas, b.detach(), q);
    return true;
}

std::vector<TessCubic> arc_cubics(float cx, float cy, float r, float start, float sweep) {
    const double kPi = 3.14159265358979323846;
    int segments = std::max(1, (int)std::ceil(std::fabs((double)sweep) / (kPi / 2 + 0.001)));
    std::vector<TessCubic> out;
    for (int i = 0; i < segments; i++) {
        double s = start + i * (double)sweep / segments, e = start + (i + 1) * (double)sweep / segments;
        double t = (4.0 / 3.0) * std::tan(0.25 * (e - s));
        double ss = std::sin(s), cs = std::cos(s), se = std::sin(e), ce = std::cos(e);
        TessCubic c;
        c.p[0] = {cx + r * (float)cs, cy + r * (float)ss};
        c.p[1] = {cx + r * (float)(cs - t * ss), cy + r * (float)(ss + t * cs)};
        c.p[2] = {cx + r * (float)(ce + t * se), cy + r * (float)(se - t * ce)};
        c.p[3] = {cx + r * (float)ce, cy + r * (float)se};
        out.push_back(c);
    }
    return out;
}

}  // namespace

void chromium_stroke_arc(SkCanvas& canvas, float cx, float cy, float r, float start, float sweep, const SkPaint& paint) {
    if (gpu_direct(canvas)) {
        draw_path_cached(canvas, svg_arc_path(cx, cy, r, start, sweep), direct_paint(paint, SkPaint::kStroke_Style, paint.getStrokeWidth()));
        return;
    }
    static const bool tess_model = a380::getenv_cached("A380_ARC_MODEL") != nullptr;
    if (tess_model && std::fabs(sweep) < 6.2831f && tess_stroke_cubics(canvas, arc_cubics(cx, cy, r, start, sweep), paint)) return;
    if (arc_tess().mode && tessellated_arc(canvas, cx, cy, r, start, sweep, paint, arc_tess().precision)) return;
    SkMatrix m = canvas.getLocalToDeviceAs3x3();
    float sc = std::sqrt(m.getScaleX() * m.getScaleX() + m.getSkewY() * m.getSkewY());
    SkPaint p(paint);
    p.setStyle(SkPaint::kStroke_Style);
    bool plain = m.getSkewX() == 0.f && m.getSkewY() == 0.f && !m.hasPerspective() && m.getScaleX() == m.getScaleY() && sc > 0.f &&
                 std::fabs(sweep) < 6.2831f && paint.getStrokeJoin() != SkPaint::kBevel_Join;
    static const bool analytic = a380::getenv_cached("A380_ARC_ANALYTIC") != nullptr;
    if (!plain || !analytic) {
        chromium_stroke_path(canvas, chromium_arc_path(cx, cy, r, start, sweep, sc), p);
        return;
    }
    const float w = paint.getStrokeWidth() * sc, R = r * sc;
    const float ox = cx * sc + m.getTranslateX(), oy = cy * sc + m.getTranslateY();
    const float outer = R + w * 0.5f, inner = std::max(0.f, R - w * 0.5f);
    const float dir = sweep >= 0.f ? 1.f : -1.f, span = std::fabs(sweep);
    const float a0 = start, a1 = start + sweep;
    const SkPaint::Cap cap = paint.getStrokeCap();
    const float ext = cap == SkPaint::kSquare_Cap ? w * 0.5f : 0.f;
    SkPoint e0{ox + R * std::cos(a0), oy + R * std::sin(a0)}, e1{ox + R * std::cos(a1), oy + R * std::sin(a1)};
    const float t0x = -std::sin(a0) * dir, t0y = std::cos(a0) * dir;
    const float t1x = -std::sin(a1) * dir, t1y = std::cos(a1) * dir;
    int X0 = (int)std::floor(ox - outer - w) - 1, X1 = (int)std::ceil(ox + outer + w) + 1;
    int Y0 = (int)std::floor(oy - outer - w) - 1, Y1 = (int)std::ceil(oy + outer + w) + 1;
    Mask mask(X0, Y0, X1 - X0, Y1 - Y0);
    auto sat = [](float v) { return std::clamp(v, 0.f, 1.f); };
    for (int y = Y0; y < Y1; y++) {
        for (int x = X0; x < X1; x++) {
            float px = x + 0.5f - ox, py = y + 0.5f - oy;
            float d = std::sqrt(px * px + py * py);
            float radial = sat(outer - d + 0.5f) * sat(d - inner + 0.5f);
            float ang = 0.f;
            if (d > 1e-4f) {
                float phi = std::atan2(py, px) - a0;
                phi = dir * phi;
                while (phi < -3.14159265f) phi += 6.2831853f;
                while (phi > 3.14159265f) phi -= 6.2831853f;
                float ds = px * t0x + py * t0y + ext;
                float de = -(px * t1x + py * t1y) + ext;
                if (span <= 3.14159265f) {
                    ang = (phi >= -1.5707963f && phi <= span + 1.5707963f) ? sat(ds + 0.5f) * sat(de + 0.5f) : 0.f;
                    if (phi < -1.5707963f || phi > span + 1.5707963f) ang = 0.f;
                } else {
                    float inside = (phi >= 0.f && phi <= span) ? 1.f : 0.f;
                    ang = inside;
                }
            }
            float c = radial * ang;
            if (cap == SkPaint::kRound_Cap) {
                float rw = w * 0.5f;
                float d0 = std::hypot(x + 0.5f - e0.fX, y + 0.5f - e0.fY), d1 = std::hypot(x + 0.5f - e1.fX, y + 0.5f - e1.fY);
                c = std::max({c, sat(rw - d0 + 0.5f), sat(rw - d1 + 0.5f)});
            }
            mask.at(x - X0, y - Y0) = cov8(c);
        }
    }
    blit(canvas, mask, p);
}

}  // namespace a380
