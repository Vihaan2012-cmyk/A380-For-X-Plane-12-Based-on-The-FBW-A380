#include <algorithm>
#include <cstdint>
#include <memory>
#include <mutex>
#include <unordered_map>
#include <vector>

#include "a380/fonts.h"
#include "include/core/SkTypeface.h"

namespace a380 {

namespace {

struct Bytes {
    std::vector<uint8_t> d;
    uint32_t u16(size_t o) const { return o + 2 <= d.size() ? (uint32_t)((d[o] << 8) | d[o + 1]) : 0u; }
    int i16(size_t o) const { return (int16_t)u16(o); }
    uint32_t u32(size_t o) const { return o + 4 <= d.size() ? (u16(o) << 16) | u16(o + 2) : 0u; }
};

int cov_index(const Bytes& b, size_t off, uint32_t g) {
    uint32_t fmt = b.u16(off), n = b.u16(off + 2);
    uint32_t lo = 0, hi = n;
    if (fmt == 1) {
        while (lo < hi) {
            uint32_t mid = (lo + hi) / 2, v = b.u16(off + 4 + 2 * mid);
            if (v == g) return (int)mid;
            if (v < g) lo = mid + 1;
            else hi = mid;
        }
    } else if (fmt == 2) {
        while (lo < hi) {
            uint32_t mid = (lo + hi) / 2;
            size_t r = off + 4 + 6 * (size_t)mid;
            uint32_t s = b.u16(r), e = b.u16(r + 2);
            if (g < s) hi = mid;
            else if (g > e) lo = mid + 1;
            else return (int)(b.u16(r + 4) + (g - s));
        }
    }
    return -1;
}

uint32_t class_of(const Bytes& b, size_t off, uint32_t g) {
    uint32_t fmt = b.u16(off);
    if (fmt == 1) {
        uint32_t start = b.u16(off + 2), n = b.u16(off + 4);
        return g >= start && g < start + n ? b.u16(off + 6 + 2 * (size_t)(g - start)) : 0u;
    }
    if (fmt == 2) {
        uint32_t n = b.u16(off + 2), lo = 0, hi = n;
        while (lo < hi) {
            uint32_t mid = (lo + hi) / 2;
            size_t r = off + 4 + 6 * (size_t)mid;
            uint32_t s = b.u16(r), e = b.u16(r + 2);
            if (g < s) hi = mid;
            else if (g > e) lo = mid + 1;
            else return b.u16(r + 4);
        }
    }
    return 0u;
}

int popcount16(uint32_t v) {
    int c = 0;
    for (; v; v >>= 1) c += (int)(v & 1u);
    return c;
}

struct KernData {
    Bytes gpos, kern;
    std::vector<std::vector<uint32_t>> subtables;
    bool any = false;
    float inv_upem = 0.f;
    std::unordered_map<uint32_t, float> cache;
    std::mutex mu;

    void load(SkTypeface* tf) {
        int upem = tf->getUnitsPerEm();
        if (upem <= 0) return;
        inv_upem = 1.f / (float)upem;
        auto fetch = [&](char a, char b, char c, char d, Bytes& out) {
            SkFontTableTag t = SkSetFourByteTag(a, b, c, d);
            size_t n = tf->getTableSize(t);
            if (!n) return;
            out.d.resize(n);
            if (tf->getTableData(t, 0, n, out.d.data()) != n) out.d.clear();
        };
        fetch('G', 'P', 'O', 'S', gpos);
        parse_gpos();
        if (!any) {
            fetch('k', 'e', 'r', 'n', kern);
            if (kern.d.size() > 4 && kern.u16(0) == 0) any = true;
        }
    }

    void parse_gpos() {
        const Bytes& b = gpos;
        if (b.d.size() < 10) return;
        size_t fl = b.u16(6), ll = b.u16(8);
        std::vector<uint32_t> lookups;
        uint32_t nf = b.u16(fl);
        for (uint32_t i = 0; i < nf; i++) {
            size_t rec = fl + 2 + 6 * (size_t)i;
            if (b.u16(rec) != 0x6b65 || b.u16(rec + 2) != 0x726e) continue;
            size_t ft = fl + b.u16(rec + 4);
            uint32_t n = b.u16(ft + 2);
            for (uint32_t k = 0; k < n; k++) lookups.push_back(b.u16(ft + 4 + 2 * (size_t)k));
        }
        std::sort(lookups.begin(), lookups.end());
        lookups.erase(std::unique(lookups.begin(), lookups.end()), lookups.end());
        uint32_t nl = b.u16(ll);
        for (uint32_t li : lookups) {
            if (li >= nl) continue;
            size_t lo = ll + b.u16(ll + 2 + 2 * (size_t)li);
            uint32_t type = b.u16(lo), ns = b.u16(lo + 4);
            std::vector<uint32_t> subs;
            for (uint32_t k = 0; k < ns; k++) {
                size_t so = lo + b.u16(lo + 6 + 2 * (size_t)k);
                uint32_t t = type;
                if (t == 9) {
                    t = b.u16(so + 2);
                    so += b.u32(so + 4);
                }
                if (t == 2) subs.push_back((uint32_t)so);
            }
            if (!subs.empty()) subtables.push_back(std::move(subs));
        }
        any = !subtables.empty();
    }

    bool pair_in(size_t s, uint32_t l, uint32_t r, int* out) const {
        const Bytes& b = gpos;
        uint32_t fmt = b.u16(s), vf1 = b.u16(s + 4), vf2 = b.u16(s + 6);
        int idx = cov_index(b, s + b.u16(s + 2), l);
        if (idx < 0) return false;
        size_t xoff = (size_t)popcount16(vf1 & 3u) * 2;
        size_t rec = 2 + 2 * (size_t)(popcount16(vf1) + popcount16(vf2));
        if (fmt == 1) {
            if ((uint32_t)idx >= b.u16(s + 8)) return false;
            size_t ps = s + b.u16(s + 10 + 2 * (size_t)idx);
            uint32_t n = b.u16(ps), lo = 0, hi = n;
            while (lo < hi) {
                uint32_t mid = (lo + hi) / 2;
                size_t ro = ps + 2 + rec * mid;
                uint32_t g = b.u16(ro);
                if (g == r) {
                    *out = (vf1 & 4u) ? b.i16(ro + 2 + xoff) : 0;
                    return true;
                }
                if (g < r) lo = mid + 1;
                else hi = mid;
            }
            return false;
        }
        if (fmt == 2) {
            uint32_t c1 = class_of(b, s + b.u16(s + 8), l), c2 = class_of(b, s + b.u16(s + 10), r);
            uint32_t n1 = b.u16(s + 12), n2 = b.u16(s + 14);
            if (c1 >= n1 || c2 >= n2) return false;
            size_t ro = s + 16 + ((size_t)c1 * n2 + c2) * (rec - 2);
            *out = (vf1 & 4u) ? b.i16(ro + xoff) : 0;
            return true;
        }
        return false;
    }

    int legacy(uint32_t l, uint32_t r) const {
        const Bytes& b = kern;
        uint32_t n = b.u16(2);
        size_t pos = 4;
        int sum = 0;
        for (uint32_t t = 0; t < n && pos + 14 <= b.d.size(); t++) {
            uint32_t len = b.u16(pos + 2), cov = b.u16(pos + 4);
            if ((cov & 1u) && !(cov & 4u) && (cov >> 8) == 0) {
                uint32_t np = b.u16(pos + 6), lo = 0, hi = np, key = (l << 16) | r;
                while (lo < hi) {
                    uint32_t mid = (lo + hi) / 2;
                    size_t ro = pos + 14 + 6 * (size_t)mid;
                    uint32_t k = b.u32(ro);
                    if (k == key) {
                        sum += b.i16(ro + 4);
                        break;
                    }
                    if (k < key) lo = mid + 1;
                    else hi = mid;
                }
            }
            if (!len) break;
            pos += len;
        }
        return sum;
    }

    float em(uint32_t l, uint32_t r) {
        uint32_t key = (l << 16) | r;
        std::lock_guard<std::mutex> lock(mu);
        auto c = cache.find(key);
        if (c != cache.end()) return c->second;
        int units = 0;
        if (!subtables.empty()) {
            for (auto& lookup : subtables)
                for (uint32_t s : lookup) {
                    int v = 0;
                    if (pair_in(s, l, r, &v)) {
                        units += v;
                        break;
                    }
                }
        } else if (!kern.d.empty()) {
            units = legacy(l, r);
        }
        float f = (float)units * inv_upem;
        cache[key] = f;
        return f;
    }
};

KernData* kern_data(const SkTypeface* tf) {
    static std::mutex mu;
    static std::unordered_map<uint32_t, std::unique_ptr<KernData>> all;
    if (!tf) return nullptr;
    std::lock_guard<std::mutex> lock(mu);
    auto& slot = all[tf->uniqueID()];
    if (!slot) {
        slot = std::make_unique<KernData>();
        slot->load(const_cast<SkTypeface*>(tf));
    }
    return slot->any ? slot.get() : nullptr;
}

}  // namespace

bool has_kerning(const SkTypeface* tf) { return kern_data(tf) != nullptr; }

float kerning_em(const SkTypeface* tf, SkGlyphID left, SkGlyphID right) {
    KernData* k = kern_data(tf);
    return k ? k->em(left, right) : 0.f;
}

}  // namespace a380
