#include <algorithm>
#include <array>
#include <cmath>
#include <cstdio>
#include <memory>
#include <string>

#include "include/core/SkFontMetrics.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPathBuilder.h"

#include "a380/display.h"
#include "a380/draw.h"
#include "a380/fonts.h"
#include "a380/inputs.h"

namespace {

using nlohmann::json;
constexpr SkColor kAmber = 0xFFE68000u;
constexpr SkColor kCyan = 0xFF00FFFFu;
constexpr SkColor kGreen = 0xFF00FF00u;
constexpr SkColor kWhite = SK_ColorWHITE;

constexpr float lu(float v) { return (float)((long long)(v * 64.f)) / 64.f; }
constexpr float kX = lu(16.64f), kY = lu(10.24f), kW = lu(1630.72f), kH = lu(1003.52f);
constexpr float kPagesH = kH - 94.f;
constexpr float kRowH = lu((kPagesH - 8.f) / 3.f);
constexpr float kXpdrRowH = lu(kPagesH / 3.f);

struct Metrics {
    float asc, desc, lh, half;
};

Metrics metrics(const SkFont& f) {
    SkFontMetrics m;
    f.getMetrics(&m);
    float a = std::round(-m.fAscent), d = 0.f;
    float lh = a + d + std::round(m.fLeading);
    return {a, d, lh, std::floor((lh - a - d) * 0.5f)};
}

SkRect snapped(float x, float y, float w, float h) {
    return SkRect::MakeLTRB(std::round(x), std::round(y), std::round(x + w), std::round(y + h));
}

struct RowView {
    std::string active_text = "---.---";
    bool active_amber = false;
    std::array<std::string, 6> digits{"-", "-", "-", "-", "-", "-"};
    std::array<bool, 6> pilot{};
    bool standby_amber = true;
    std::string title = "STBY";
    bool entry = false;
    bool standby_data = false;
    bool speaker_hidden = true;
    bool no_reception = false;
    bool transmitting = false;
    bool selected = false;
};

struct View {
    bool screen_on = false;
    bool xpdr_page = false;
    std::array<RowView, 3> rows;
    bool xpdr_auto = false;
    std::string xpdr_code = "----";
    bool xpdr_entry = false;
    bool xpdr_invalid = false;
    bool xpdr_ident = false;
    std::string message;
};

bool jb(const json& o, const char* k, bool d = false) {
    auto it = o.find(k);
    if (it == o.end()) return d;
    if (it->is_boolean()) return it->get<bool>();
    if (it->is_number()) return it->get<double>() != 0.0;
    return d;
}

std::string js(const json& o, const char* k, const std::string& d = {}) {
    auto it = o.find(k);
    return it != o.end() && it->is_string() ? it->get<std::string>() : d;
}

class Rmp : public a380::Display {
public:
    explicit Rmp(int index) : index_(index) {}

    SkISize size() const override { return {1664, 1024}; }

    void draw(SkCanvas& canvas, const a380::Inputs& in) override {
        View v = view(in);
        if (!v.screen_on) return;

        SkFont f19 = a380::font("RMP-19", 120.f, false, true);
        SkFont f16 = a380::font("RMP-16", 96.f, false, true);
        SkFont f13 = a380::font("RMP-13", 80.f, false, true);
        SkFont f11 = a380::font("RMP-11", 60.f, false, true);
        SkFont f10 = a380::font("RMP-10", 60.f, false, true);

        SkPaint bar;
        bar.setColor(kWhite);
        if (v.xpdr_page) {
            xpdr(canvas, v, f19, f16, f11, f10);
        } else {
            for (int i = 1; i < 3; i++) canvas.drawRect(snapped(kX, kY + i * kRowH + (i - 1) * 4.f, kW, 4.f), bar);
            for (int r = 0; r < 3; r++) row(canvas, v.rows[r], r, f19, f16, f13, f10);
        }
        canvas.drawRect(snapped(kX, kY + kH - 94.f, kW, 4.f), bar);
        Metrics mm = metrics(f10);
        put(canvas, v.message, kX + 32.f, kY + kH - 94.f + 4.f + 32.f + mm.half + mm.asc, f10, kWhite);
    }

private:
    int index_;
    mutable int prev_reception_[3] = {0, 0, 0};
    mutable double tx_flash_until_[3] = {0.0, 0.0, 0.0};

    View view(const a380::Inputs& in) const {
        const json* b = in.bus("a380.rmp.view." + std::to_string(index_));
        if (b) {
            if (b->is_object()) return from_json(*b);
            if (b->is_string()) {
                json p = json::parse(b->get<std::string>(), nullptr, false);
                if (p.is_object()) return from_json(p);
            }
        }
        return from_vars(in);
    }

    static View from_json(const json& o) {
        View v;
        v.screen_on = jb(o, "screen_on");
        v.xpdr_page = js(o, "page") == "Transponder" || js(o, "page") == "transponder";
        if (auto it = o.find("rows"); it != o.end() && it->is_array()) {
            for (size_t r = 0; r < 3 && r < it->size(); r++) {
                const json& j = (*it)[r];
                RowView& w = v.rows[r];
                w.active_text = js(j, "active_text", w.active_text);
                w.active_amber = jb(j, "active_amber");
                std::string d = js(j, "standby_digits", "------");
                for (size_t i = 0; i < 6; i++) w.digits[i] = i < d.size() ? std::string(1, d[i]) : std::string("-");
                if (auto p = j.find("standby_pilot_entered"); p != j.end() && p->is_array())
                    for (size_t i = 0; i < 6 && i < p->size(); i++) w.pilot[i] = (*p)[i].is_boolean() && (*p)[i].get<bool>();
                w.standby_amber = jb(j, "standby_amber");
                w.title = js(j, "standby_mode_title", "STBY");
                w.entry = jb(j, "standby_entry_in_progress");
                w.standby_data = jb(j, "standby_data");
                w.speaker_hidden = jb(j, "loudspeaker_hidden", true);
                w.no_reception = jb(j, "no_reception_cross");
                w.transmitting = jb(j, "transmitting");
                w.selected = jb(j, "selected");
            }
        }
        if (auto it = o.find("xpdr"); it != o.end() && it->is_object()) {
            v.xpdr_auto = jb(*it, "auto");
            v.xpdr_code = js(*it, "code", "----");
            v.xpdr_entry = jb(*it, "entry_in_progress");
            v.xpdr_invalid = jb(*it, "entry_invalid");
            v.xpdr_ident = jb(*it, "ident");
        }
        v.message = js(o, "message");
        return v;
    }

    View from_vars(const a380::Inputs& in) const {
        View v;
        std::string n = std::to_string(index_);
        const char* power = index_ == 3 ? "L:A32NX_ELEC_DC_1_BUS_IS_POWERED" : "L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED";
        v.screen_on = in.var(power) > 0 && in.var("L:A380X_RMP_" + n + "_BRIGHTNESS_KNOB") > 0;
        for (int r = 0; r < 3; r++) {
            const int vhf = r + 1;
            const std::string vs = std::to_string(vhf);
            RowView& w = v.rows[r];
            bool failed = radio_failed(in, vhf);
            bool activeData = in.var("L:FBW_RMP_MODE_ACTIVE_" + vs, {}, 0) == 1.0;
            uint32_t abcd = (uint32_t)(uint64_t)in.var("L:FBW_RMP_FREQUENCY_ACTIVE_" + vs, {}, 0);
            bool nul = abcd == 0 || failed;
            w.active_text = activeData ? (nul ? "----" : "DATA") : (nul ? "---.---" : bcd_text(abcd, "."));
            w.active_amber = failed;
            uint32_t sbcd = (uint32_t)(uint64_t)in.var("L:FBW_RMP_FREQUENCY_STANDBY_" + vs, {}, 0);
            w.standby_amber = sbcd == 0;
            std::string st = sbcd == 0 ? "------" : bcd_text(sbcd, "");
            for (int i = 0; i < 6; i++) w.digits[i] = st.substr(i, 1);
            int smode = (int)in.var("L:FBW_RMP_MODE_STANDBY_" + vs, {}, 0);
            w.standby_data = vhf == 3 && smode == 1;
            w.title = smode == 2 ? "EMER" : "STBY";
            bool rx = in.var("L:A380X_RMP_" + n + "_VHF_VOL_RX_SWITCH_" + vs) != 0.0;
            bool tx = in.var("L:A380X_RMP_" + n + "_VHF_TX_" + vs) != 0.0;
            int rmode = rx ? 1 : tx ? 2 : 0;
            double now = in.now_ms();
            if (rmode == 2 && prev_reception_[r] != 2) tx_flash_until_[r] = now + 2000.0;
            prev_reception_[r] = rmode;
            bool flash_on = ((long long)(now / 500.0) & 1) != 0;
            w.speaker_hidden = rmode == 0 || (rmode == 2 && now < tx_flash_until_[r] && !flash_on);
            w.no_reception = rmode == 2;
            w.transmitting = tx;
            w.selected = r == 0;
        }
        char code[8];
        std::snprintf(code, sizeof code, "%04x", (unsigned)in.var("TRANSPONDER CODE:1", {}, 0) & 0xffffu);
        int xs = (int)in.var("TRANSPONDER STATE:1", {}, 0);
        v.xpdr_auto = xs == 3 || xs == 4 || xs == 5;
        v.xpdr_code = code;
        v.message = std::string("SQUAWK : ") + code + (v.xpdr_auto ? "" : " - STBY");
        return v;
    }

    static void put(SkCanvas& c, const std::string& s, float x, float y, const SkFont& f, SkColor col) {
        SkPaint p;
        p.setAntiAlias(true);
        p.setColor(col);
        a380::draw_text(c, s, x, y, f, p);
    }

    static bool radio_failed(const a380::Inputs& in, int vhf) {
        double raw = in.var("L:FBW_VHF" + std::to_string(vhf) + "_FREQUENCY", {}, 0);
        return ((uint64_t)raw >> 32) == 0;
    }

    static std::string bcd_text(uint32_t bcd, const char* sep) {
        char b[16];
        std::snprintf(b, sizeof b, "%06x", (bcd >> 4) & 0xffffffu);
        std::string d = b;
        return d.substr(0, 3) + sep + d.substr(3);
    }

    static void loudspeaker(SkCanvas& c, float x, float y, bool cross) {
        const float s = std::min(58.f / 75.f, 89.f / 114.f);
        c.save();
        c.translate(x, y + (89.f - 114.f * s) * 0.5f);
        c.scale(s, s);
        SkPathBuilder pb;
        pb.moveTo(2, 37).lineTo(38, 37);
        float px = 38, py = 37;
        for (int i = 0; i < 5; i++) {
            pb.lineTo(px, py - 7).lineTo(px + 7, py - 7);
            px += 7;
            py -= 7;
        }
        pb.lineTo(73, 112);
        px = 73;
        py = 112;
        for (int i = 0; i < 5; i++) {
            pb.lineTo(px - 7, py).lineTo(px - 7, py - 7);
            px -= 7;
            py -= 7;
        }
        pb.lineTo(2, py).close();
        SkPaint p;
        p.setAntiAlias(true);
        p.setColor(kWhite);
        c.drawPath(pb.detach(), p);
        if (cross) {
            SkPaint q;
            q.setAntiAlias(true);
            q.setStyle(SkPaint::kStroke_Style);
            q.setStrokeWidth(6.f);
            q.setColor(kAmber);
            c.drawLine(2, 2, 73, 112, q);
            c.drawLine(73, 2, 2, 112, q);
        }
        c.restore();
    }

    void row(SkCanvas& c, const RowView& w, int r, const SkFont& f19, const SkFont& f16, const SkFont& f13,
             const SkFont& f10) {
        const int vhf = r + 1;
        const float top = kY + r * (kRowH + 4.f);
        const float c1 = kX, c2 = kX + 595.f, c3 = kX + 1035.f;

        Metrics m19 = metrics(f19), m16 = metrics(f16), m13 = metrics(f13), m10 = metrics(f10);
        const float cellTop = top + 54.f, cellH = kRowH - 54.f;

        float ah = m19.lh + 10.f;
        float ay = cellTop + lu((cellH - ah) * 0.5f);
        float aw = a380::text_width(f19, w.active_text);
        put(c, w.active_text, lu(c1 + (595.f - aw) * 0.5f), ay + 10.f + m19.half + m19.asc, f19,
            w.active_amber ? kAmber : kWhite);

        std::string name = "VHF" + std::to_string(vhf);
        float nw = a380::text_width(f13, name);
        float ih = m13.lh + 6.f + 60.f;
        float itemsW = 58.f + 12.f + nw;
        float free = 380.f - itemsW;
        float iy = cellTop + lu((cellH - ih) * 0.5f);
        float spkX = c2 + 30.f + free * 0.25f;
        float nameX = spkX + 58.f + free * 0.5f + 12.f;
        if (!w.speaker_hidden) loudspeaker(c, lu(spkX), iy + 24.f, w.no_reception);
        if (w.transmitting) {
            SkPaint g;
            g.setColor(kGreen);
            c.drawRect(snapped(lu(nameX - 12.f), iy + 30.f, 12.f + nw, 6.f + m13.lh), g);
        }
        put(c, name, lu(nameX), iy + 30.f + 6.f + m13.half + m13.asc, f13, w.transmitting ? SK_ColorBLACK : kWhite);

        const float bx = c3 + 64.f, by = top + 2.f, bw = 531.f, bh = kRowH - 4.f;
        if (w.selected) a380::stroke_rrect_inside(c, snapped(bx, by, bw, bh), 0.f, 4.f, kCyan);
        SkColor col = w.standby_amber ? kAmber : w.selected ? kCyan : kWhite;
        float inner = bh - 8.f;
        float stack = m10.lh + 120.f;
        float gap = (inner - stack) / 3.f;
        float my = by + 4.f + gap;
        if (w.selected) {
            float sw = a380::text_width(f10, w.title);
            put(c, w.title, lu(bx + (bw - sw) * 0.5f), my + m10.half + m10.asc, f10, kWhite);
        }
        float fy = my + m10.lh + gap + 120.f;
        if (vhf == 3 && w.standby_data) {
            float dw = a380::text_width(f19, "DATA");
            put(c, "DATA", lu(bx + (bw - dw) * 0.5f), fy - (m19.lh - m19.half - m19.asc), f19, col);
            return;
        }
        const SkFont& base = w.entry ? f16 : f13;
        const Metrics& mb = w.entry ? m16 : m13;
        std::string items[7] = {w.digits[0], w.digits[1], w.digits[2], ".", w.digits[3], w.digits[4], w.digits[5]};
        const SkFont* fonts[7];
        const Metrics* ms[7];
        for (int i = 0; i < 7; i++) {
            int d = i < 3 ? i : i - 1;
            bool big = i != 3 && w.pilot[d];
            fonts[i] = i == 3 ? &f13 : big ? &f19 : &base;
            ms[i] = i == 3 ? &m13 : big ? &m19 : &mb;
        }
        float total = 0;
        for (int i = 0; i < 7; i++) total += a380::text_width(*fonts[i], items[i]);
        float x = lu(bx + 4.f + ((bw - 8.f) - total) * 0.5f);
        for (int i = 0; i < 7; i++) {
            put(c, items[i], x, fy - (ms[i]->lh - ms[i]->half - ms[i]->asc), *fonts[i], col);
            x += a380::text_width(*fonts[i], items[i]);
        }
    }

    void xpdr(SkCanvas& c, const View& v, const SkFont& f19, const SkFont& f16, const SkFont& f11, const SkFont& f10) {
        Metrics m11 = metrics(f11), m10 = metrics(f10);
        std::string mode = v.xpdr_auto ? "AUTO" : "STBY";
        float mw = a380::text_width(f11, mode);
        float my = kY + lu((kXpdrRowH - m11.lh) * 0.5f);
        put(c, mode, lu(kX + (kW - mw) * 0.5f), my + m11.half + m11.asc, f11, v.xpdr_auto ? kGreen : kWhite);

        const float bw = 538.f, bh = 288.f;
        float bx = lu(kX + (kW - bw) * 0.5f), by = kY + kXpdrRowH + lu((kXpdrRowH - bh) * 0.5f);
        a380::stroke_rrect_inside(c, snapped(bx, by, bw, bh), 0.f, 4.f, kCyan);
        const SkFont& fc = v.xpdr_entry ? f16 : f19;
        Metrics mc = metrics(fc);
        float gap = (280.f - m10.lh - mc.lh) / 3.f;
        float t1 = by + 4.f + gap;
        float sw = a380::text_width(f10, "SQWK");
        put(c, "SQWK", lu(bx + 4.f + (530.f - sw) * 0.5f), t1 + m10.half + m10.asc, f10, kWhite);
        float t2 = t1 + m10.lh + gap;
        float cw = a380::text_width(fc, v.xpdr_code);
        put(c, v.xpdr_code, lu(bx + 4.f + (530.f - cw) * 0.5f), t2 + mc.half + mc.asc, fc,
            v.xpdr_invalid ? kAmber : kCyan);

        float ix = kX + 10.f, iy = kY + 640.f + m11.half + m11.asc;
        if (v.xpdr_ident) put(c, "IDENT", ix + a380::text_width(f11, " "), iy, f11, kCyan);
        else put(c, "*IDENT", ix, iy, f11, kCyan);
    }
};

std::unique_ptr<a380::Display> make(std::string_view variant) {
    int idx = variant == "2" ? 2 : variant == "3" ? 3 : 1;
    return std::make_unique<Rmp>(idx);
}
A380_REGISTER_DISPLAY("rmp", make);

}  // namespace
