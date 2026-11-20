#include "a380/glyphs.h"
#include "a380/env.h"

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <mutex>
#include <string>
#include <unordered_map>
#include <vector>

#include "include/codec/SkCodec.h"
#include "include/codec/SkPngDecoder.h"
#include "include/core/SkBitmap.h"
#include "include/core/SkCanvas.h"
#include "include/core/SkData.h"
#include "include/core/SkPaint.h"
#include "include/core/SkSurfaceProps.h"

#include "a380/assets.h"
#include "a380/fonts.h"
#include "glyphs_win.h"

#ifdef _WIN32
#include <process.h>
#define A380_GETPID _getpid
#else
#include <unistd.h>
#define A380_GETPID getpid
#endif

namespace a380::glyphs {

namespace {

struct Entry {
    int left = 0, top = 0, w = 0, h = 0;
    size_t off = 0;
    std::vector<uint8_t> own;
    std::vector<uint8_t> own5;
    bool has_raw = false;
};

struct Rec {
    int left, top, w, h;
    size_t off;
};

struct State {
    std::once_flag once;
    bool loaded = false;
    bool bake = false;
    bool verify = false;
    bool off = false;
    std::unordered_map<std::string, Rec> atlas;
    std::vector<uint8_t> sheet;
    std::vector<uint8_t> sheet5;
    std::string bake_dir;
    bool prove = false;
    long proof5_total = 0, proof5_bad = 0;
#ifdef A380_GLYPH_BAKE_TOOL
    win::Proof proof;
#endif
    std::mutex mu;
    std::unordered_map<std::string, Entry> scaled;
    std::unordered_map<std::string, std::string> scaled_face;
    std::unordered_map<std::string, int> scaled_info;
    long hits = 0, misses = 0, unsupported = 0;
    long runs = 0, bad_runs = 0, bad_px = 0;
    bool logged = false;
    ~State();
};

State& st() {
    thread_local State s;
    return s;
}

constexpr bool kBgra = kN32_SkColorType == kBGRA_8888_SkColorType;

template <class T>
void put(std::string& k, const T& v) {
    k.append(reinterpret_cast<const char*>(&v), sizeof(T));
}

uint8_t flag_bits(const SkFont& f) {
    return (uint8_t)((f.isEmbolden() ? 1 : 0) | (f.isLinearMetrics() ? 2 : 0) | (f.isBaselineSnap() ? 4 : 0) |
                     (f.isForceAutoHinting() ? 8 : 0) | (f.isEmbeddedBitmaps() ? 16 : 0));
}

std::string make_key(const std::string& face, const SkFont& f, SkGlyphID g, int sub) {
    std::string k;
    k.reserve(face.size() + 32);
    k.append(face);
    k.push_back('\0');
    put(k, f.getSize());
    put(k, f.getScaleX());
    put(k, f.getSkewX());
    put(k, flag_bits(f));
    put(k, (uint8_t)f.getHinting());
    put(k, (uint16_t)g);
    put(k, (uint8_t)sub);
    return k;
}

int mask_to_value(int m) {
    int s = m + (m >> 4);
    return (255 * s) >> 5;
}

bool decode_png(const uint8_t* data, size_t len, SkColorType ct, int& w, int& h, std::vector<uint8_t>& out) {
    auto sk = SkData::MakeWithCopy(data, len);
    std::unique_ptr<SkCodec> codec = SkPngDecoder::Decode(sk, nullptr);
    if (!codec) return false;
    w = codec->dimensions().width();
    h = codec->dimensions().height();
    SkImageInfo info = SkImageInfo::Make(w, h, ct, kOpaque_SkAlphaType);
    out.resize(info.computeMinByteSize());
    return codec->getPixels(info, out.data(), info.minRowBytes()) == SkCodec::kSuccess;
}

uint32_t rd32(const uint8_t* p) { return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24); }
uint16_t rd16(const uint8_t* p) { return (uint16_t)(p[0] | (p[1] << 8)); }

void load_atlas(State& s) {
    sk_sp<SkData> file = SkData::MakeFromFileName(assets::glyphs_file().c_str());
    if (!file || file->size() < 24) return;
    const uint8_t* p = static_cast<const uint8_t*>(file->data());
    if (std::memcmp(p, "A380GLY2", 8) != 0) return;
    uint32_t index_bytes = rd32(p + 8), sheet_w = rd32(p + 12), len1 = rd32(p + 16);
    if (24 + (size_t)len1 > file->size()) return;
    const uint8_t* png1 = p + 20;
    uint32_t len2 = rd32(png1 + len1);
    if (24 + (size_t)len1 + len2 > file->size()) return;
    const uint8_t* png2 = png1 + len1 + 4;
    std::vector<uint8_t> idx;
    int w, h;
    if (!decode_png(png1, len1, kGray_8_SkColorType, w, h, idx) || idx.size() < index_bytes) return;
    std::vector<uint8_t> rgba;
    if (!decode_png(png2, len2, kRGBA_8888_SkColorType, w, h, rgba)) return;
    (void)sheet_w;
    s.sheet.resize((size_t)w * h * 3);
    s.sheet5.resize((size_t)w * h * 3);
    for (size_t i = 0; i < (size_t)w * h; i++) {
        for (int k = 0; k < 3; k++) {
            s.sheet[i * 3 + k] = rgba[i * 4 + k];
            s.sheet5[i * 3 + k] = (uint8_t)(rgba[i * 4 + k] >> 3);
        }
    }
    const uint8_t* q = idx.data();
    uint32_t count = rd32(q), nfaces = rd32(q + 4);
    q += 8;
    std::vector<std::string> faces;
    for (uint32_t i = 0; i < nfaces; i++) {
        uint8_t n = *q++;
        faces.emplace_back(reinterpret_cast<const char*>(q), n);
        q += n;
    }
    s.atlas.reserve(count);
    for (uint32_t i = 0; i < count; i++) {
        uint16_t face = rd16(q);
        float size, sx, sk;
        std::memcpy(&size, q + 2, 4);
        std::memcpy(&sx, q + 6, 4);
        std::memcpy(&sk, q + 10, 4);
        uint8_t flags = q[14], hint = q[15];
        uint16_t glyph = rd16(q + 16);
        uint8_t sub = q[18];
        Rec r;
        r.left = (int16_t)rd16(q + 19);
        r.top = (int16_t)rd16(q + 21);
        r.w = rd16(q + 23);
        r.h = rd16(q + 25);
        r.off = rd32(q + 27);
        q += 31;
        if (face >= faces.size()) continue;
        std::string k = faces[face];
        k.push_back('\0');
        put(k, size);
        put(k, sx);
        put(k, sk);
        put(k, flags);
        put(k, hint);
        put(k, glyph);
        put(k, sub);
        s.atlas.emplace(std::move(k), r);
    }
    s.loaded = !s.atlas.empty();
}

void init(State& s) {
    std::call_once(s.once, [&s] {
        const char* off = a380::getenv_cached("A380_GLYPHS");
        if (off && !std::strcmp(off, "0")) {
            s.off = true;
            return;
        }
#ifdef A380_GLYPH_BAKE_TOOL
        if (const char* b = a380::getenv_cached("A380_GLYPH_BAKE"); b && *b) {
            s.bake = true;
            s.bake_dir = b;
        }
        if (const char* v = a380::getenv_cached("A380_GLYPH_PROOF"); v && std::strcmp(v, "0") != 0) s.prove = true;
#endif
        if (const char* v = a380::getenv_cached("A380_GLYPH_VERIFY"); v && std::strcmp(v, "0") != 0) s.verify = true;
        if (!s.bake) load_atlas(s);
    });
}

bool scale_glyph(const SkFont& font, SkGlyphID g, int sub, Entry& e) {
    SkFont f = font;
    f.setEdging(SkFont::Edging::kSubpixelAntiAlias);
    f.setSubpixel(true);
    const float sz = f.getSize();
    const int W = (int)std::ceil(2.f * sz) + 32, H = (int)std::ceil(2.4f * sz) + 32;
    if (W > 1600 || H > 1600) return false;
    const int ox = (int)std::ceil(0.5f * sz) + 16, oy = (int)std::ceil(1.4f * sz) + 16;
    SkBitmap bm;
    bm.allocPixels(SkImageInfo::MakeN32Premul(W, H));
    bm.eraseColor(SK_ColorBLACK);
    SkSurfaceProps props(0, kRGB_H_SkPixelGeometry, 0.f, 1.f);
    SkCanvas c(bm, props);
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(SK_ColorWHITE);
    SkPoint pt{(float)ox + 0.25f * (float)sub, (float)oy};
    c.drawGlyphs(SkSpan<const SkGlyphID>(&g, 1), SkSpan<const SkPoint>(&pt, 1), {0.f, 0.f}, f, p);
    int inv[256];
    std::fill(inv, inv + 256, -1);
    for (int m = 0; m < 32; m++) inv[mask_to_value(m)] = m;
    int x0 = W, y0 = H, x1 = -1, y1 = -1;
    std::vector<uint8_t> m3((size_t)W * H * 3);
    for (int y = 0; y < H; y++) {
        const uint32_t* row = bm.getAddr32(0, y);
        for (int x = 0; x < W; x++) {
            uint32_t v = row[x];
            int r = kBgra ? (int)((v >> 16) & 255) : (int)(v & 255), gg = (int)((v >> 8) & 255),
                b = kBgra ? (int)(v & 255) : (int)((v >> 16) & 255);
            int mr = inv[r], mg = inv[gg], mb = inv[b];
            if (mr < 0 || mg < 0 || mb < 0) return false;
            uint8_t* d = &m3[((size_t)y * W + x) * 3];
            d[0] = (uint8_t)mr;
            d[1] = (uint8_t)mg;
            d[2] = (uint8_t)mb;
            if (mr | mg | mb) {
                x0 = std::min(x0, x);
                y0 = std::min(y0, y);
                x1 = std::max(x1, x);
                y1 = std::max(y1, y);
            }
        }
    }
    if (x1 < 0) {
        e.left = e.top = e.w = e.h = 0;
        e.own5.clear();
        return true;
    }
    if (x0 == 0 || y0 == 0 || x1 == W - 1 || y1 == H - 1) return false;
    e.left = x0 - ox;
    e.top = y0 - oy;
    e.w = x1 - x0 + 1;
    e.h = y1 - y0 + 1;
    e.own5.resize((size_t)e.w * e.h * 3);
    for (int y = 0; y < e.h; y++) std::memcpy(&e.own5[(size_t)y * e.w * 3],&m3[((size_t)(y0 + y) * W + x0) * 3], (size_t)e.w * 3);
    return true;
}

void shard_put(std::vector<uint8_t>& o, const void* p, size_t n) {
    const uint8_t* b = static_cast<const uint8_t*>(p);
    o.insert(o.end(), b, b + n);
}

State::~State() {
    if (bake && !scaled.empty() && !bake_dir.empty()) {
        std::vector<uint8_t> o;
        for (auto& [key, e] : scaled) {
            size_t z = key.find('\0');
            if (z == std::string::npos || e.w < 0 || !e.has_raw) continue;
            uint8_t fl = (uint8_t)z;
            shard_put(o, &fl, 1);
            shard_put(o, key.data(), key.size());
            int16_t v[4] = {(int16_t)e.left, (int16_t)e.top, (int16_t)e.w, (int16_t)e.h};
            shard_put(o, v, sizeof v);
            shard_put(o, e.own.data(), e.own.size());
        }
        char name[512];
        std::snprintf(name, sizeof name, "%s/%d-%p.glyshard", bake_dir.c_str(), (int)A380_GETPID(), (void*)this);
        if (FILE* f = std::fopen(name, "wb")) {
            std::fwrite(o.data(), 1, o.size(), f);
            std::fclose(f);
        }
    }
    if (misses || unsupported) std::fprintf(stderr, "glyph atlas: %ld glyph hits, %ld misses, %ld unsupported\n", hits, misses, unsupported);
    if (verify || bake) std::fprintf(stderr, "glyph verify: %ld runs, %ld differing runs, %ld differing pixels\n", runs, bad_runs, bad_px);
#ifdef A380_GLYPH_BAKE_TOOL
    if (prove)
        std::fprintf(stderr,
                     "glyph proof: %ld glyphs, %ld mask comparisons, %ld differing masks, %ld pixels compared, %ld differing pixels, %ld unsupported, "
                     "unblended %ld masks %ld differing, raw>>3 vs scaler 5-bit masks %ld glyphs %ld differing\n",
                     proof.glyphs, proof.combos, proof.bad_combos, proof.pixels, proof.bad_pixels, proof.unsupported, proof.grey_combos,
                     proof.grey_bad, proof5_total, proof5_bad);
#endif
}

}  // namespace

bool enabled() {
    State& s = st();
    init(s);
    return !s.off && (s.loaded || s.bake);
}

bool baking() {
    State& s = st();
    init(s);
    return s.bake;
}

bool verifying() {
    State& s = st();
    init(s);
    return s.verify || s.bake;
}

bool get(const SkFont& font, SkGlyphID g, int sub, Mask& out) {
    State& s = st();
    init(s);
    std::string face = typeface_key(font.getTypeface());
    if (face.empty() || face.size() > 200) return false;
    std::string key = make_key(face, font, g, sub);
    if (!s.bake) {
        if (auto it = s.atlas.find(key); it != s.atlas.end()) {
            const Rec& r = it->second;
            out.left = r.left;
            out.top = r.top;
            out.w = r.w;
            out.h = r.h;
            out.rgb = r.w * r.h ? &s.sheet5[r.off * 3] : nullptr;
            out.raw = r.w * r.h ? &s.sheet[r.off * 3] : nullptr;
            std::lock_guard<std::mutex> lock(s.mu);
            s.hits++;
            return true;
        }
    }
    std::lock_guard<std::mutex> lock(s.mu);
    auto it = s.scaled.find(key);
    if (it == s.scaled.end()) {
        Entry e;
#ifdef A380_GLYPH_BAKE_TOOL
        if (s.bake) {
            if (!win::bake(face, font, g, sub, e.left, e.top, e.w, e.h, e.own)) {
                s.unsupported++;
                return false;
            }
            e.has_raw = true;
            e.own5.resize(e.own.size());
            for (size_t i = 0; i < e.own.size(); i++) e.own5[i] = (uint8_t)(e.own[i] >> 3);
            if (s.prove) {
                win::prove(font, g, sub, e.left, e.top, e.w, e.h, e.own.data(), s.proof);
                Entry old;
                if (scale_glyph(font, g, sub, old)) {
                    s.proof5_total++;
                    const int x0 = std::min(old.left, e.left), y0 = std::min(old.top, e.top);
                    const int x1 = std::max(old.left + old.w, e.left + e.w), y1 = std::max(old.top + old.h, e.top + e.h);
                    auto at = [](const Entry& q, int x, int y, int k) -> int {
                        return (x >= q.left && x < q.left + q.w && y >= q.top && y < q.top + q.h)
                                   ? q.own5[((size_t)(y - q.top) * q.w + (x - q.left)) * 3 + k]
                                   : 0;
                    };
                    bool same = true;
                    for (int y = y0; y < y1 && same; y++)
                        for (int x = x0; x < x1 && same; x++)
                            for (int k = 0; k < 3; k++)
                                if (at(old, x, y, k) != at(e, x, y, k)) same = false;
                    if (!same) s.proof5_bad++;
                }
            }
        } else
#endif
        if (!scale_glyph(font, g, sub, e)) {
            s.unsupported++;
            return false;
        }
        it = s.scaled.emplace(std::move(key), std::move(e)).first;
        if (!s.bake) {
            s.misses++;
            if (!s.logged) {
                s.logged = true;
                std::fprintf(stderr, "glyph atlas: miss (glyph %d, size %.3f, %s); falling back to the platform scaler\n", (int)g,
                             font.getSize(), face.c_str());
            }
        }
    }
    const Entry& e = it->second;
    out.left = e.left;
    out.top = e.top;
    out.w = e.w;
    out.h = e.h;
    out.rgb = e.own5.empty() ? nullptr : e.own5.data();
    out.raw = e.has_raw && !e.own.empty() ? e.own.data() : nullptr;
    return true;
}

void note_verify(bool match, int differing_pixels) {
    State& s = st();
    std::lock_guard<std::mutex> lock(s.mu);
    s.runs++;
    if (!match) {
        s.bad_runs++;
        s.bad_px += differing_pixels;
    }
}

}  // namespace a380::glyphs
