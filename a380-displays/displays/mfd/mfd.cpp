#include <algorithm>
#include <cstdlib>
#include <string>
#include <utility>
#include <vector>

#include "a380/display.h"
#include "a380/display_unit.h"
#include "a380/fonts.h"
#include "mfd/mfd_pages.h"
#include "mfd/widgets.h"

namespace a380::mfd {

namespace {
std::vector<std::pair<std::string, PageDrawer>>& registry() {
    static std::vector<std::pair<std::string, PageDrawer>> r;
    return r;
}
}  // namespace

bool register_page(const char* uri_prefix, PageDrawer drawer) {
    registry().emplace_back(uri_prefix, drawer);
    return true;
}

PageDrawer find_page(std::string_view uri) {
    PageDrawer best = nullptr;
    size_t best_len = 0;
    for (auto& [prefix, fn] : registry()) {
        if (uri.size() >= prefix.size() && uri.compare(0, prefix.size(), prefix) == 0 &&
            (prefix.size() >= best_len)) {
            best = fn;
            best_len = prefix.size();
        }
    }
    return best;
}

namespace {

using nlohmann::json;
namespace ui = a380::mfd::ui;

constexpr float kSideW = 768, kH = 1024, kGap = 110;
constexpr float kSysH = 44, kPageRowTop = 44, kPageRowH = 50.7f;
constexpr float kTitleTop = kPageRowTop + kPageRowH;
constexpr float kFooterTop = 959.3f;
constexpr float kFooterBtnTop = 962.7f, kFooterBtnH = 56.6f;

std::string jstr(const json& j, const char* k, const std::string& d = {}) {
    auto it = j.find(k);
    return it != j.end() && it->is_string() ? it->get<std::string>() : d;
}

bool jbool(const json& j, const char* k, bool d = false) {
    auto it = j.find(k);
    return it != j.end() && it->is_boolean() ? it->get<bool>() : d;
}

SkColor parse_color(const std::string& s, SkColor d) {
    if (s.size() == 7 && s[0] == '#') return 0xFF000000u | (uint32_t)std::strtoul(s.c_str() + 1, nullptr, 16);
    return d;
}

std::vector<ui::MenuItem> menu_items(const json& j, const char* k) {
    std::vector<ui::MenuItem> out;
    auto it = j.find(k);
    if (it == j.end() || !it->is_array()) return out;
    for (auto& e : *it) {
        ui::MenuItem m;
        m.text = jstr(e, "text");
        m.disabled = jbool(e, "disabled");
        out.push_back(std::move(m));
    }
    return out;
}

std::vector<std::string> split_lines(const std::string& s) {
    std::vector<std::string> out;
    size_t a = 0;
    while (true) {
        size_t b = s.find('\n', a);
        out.push_back(s.substr(a, b == std::string::npos ? b : b - a));
        if (b == std::string::npos) break;
        a = b + 1;
    }
    return out;
}

struct PendingMenu {
    float x, y, w;
    std::vector<ui::MenuItem> items;
};

void draw_msg_list(SkCanvas& c, SkRect content, const json& dlg) {
    float x0 = content.left() + 75, x1 = content.right() - 75;
    float y = content.top() + 75;
    ui::fill(c, SkRect::MakeLTRB(x0, y, x1, y + 2), ui::pal::white);
    y += 2;
    ui::TextStyle ts{26, ui::pal::white, ui::Face::Mfd, ui::Align::Left};
    float lh = ui::line_height(ts);
    if (dlg.contains("messages") && dlg["messages"].is_array()) {
        for (auto& msg : dlg["messages"]) {
            auto lines = ui::wrap_text(msg.get<std::string>(), ts, x1 - x0 - 10);
            float ty = y + 5;
            for (auto& l : lines) {
                ui::text(c, l, x0 + 5, ty, ts);
                ty += lh;
            }
            ui::fill(c, SkRect::MakeLTRB(x0, y + 78, x1, y + 80), ui::pal::white);
            y += 80;
        }
    }
    std::string label = jstr(dlg, "button", "CLOSE");
    ui::ButtonStyle bs;
    float w = ui::text_width(label, ui::TextStyle{22, ui::pal::white}) + 24 + 4;
    float h = ui::button_height();
    ui::button(c, SkRect::MakeXYWH(content.left(), content.bottom() - 12.3f - h, w, h), label, bs);
}

void draw_side(SkCanvas& c, float ox, const json& m) {
    if (!m.is_object()) return;
    const json empty = json::object();
    const json& header = m.contains("header") ? m["header"] : empty;
    const json& footer = m.contains("footer") ? m["footer"] : empty;
    std::string uri = jstr(m, "uri");

    c.save();
    c.translate(ox, 0);
    ui::fill(c, SkRect::MakeXYWH(0, 0, kSideW, kH), ui::pal::background);

    std::vector<PendingMenu> menus;

    float sys_w = kSideW * 0.25f;
    ui::DropdownStyle ds;
    ds.align = ui::Align::Left;
    ds.text_color = ui::pal::cyan;
    std::string sys_text = jstr(header, "system", "FMS 2");
    if (jbool(header, "systemOpen")) {
        ds.text_color = ui::pal::background;
        ui::dropdown(c, SkRect::MakeXYWH(0, 0, sys_w, kSysH), "", ds);
        ui::TextStyle st{27, ui::pal::background, ui::Face::Mfd, ui::Align::Left};
        float tw = ui::text_width(sys_text, st);
        ui::stroke_box(c, SkRect::MakeXYWH(9.3f, 6.7f, tw + 4.f, 30.6f), ui::pal::light_grey, 2);
        ui::fill(c, SkRect::MakeXYWH(12, 9.5f, tw, 25), ui::pal::cyan);
        ui::text(c, sys_text, 12, 9.5f + (25 - ui::line_height(st)) * 0.5f, st);
    } else {
        ui::dropdown(c, SkRect::MakeXYWH(0, 0, sys_w, kSysH), sys_text, ds);
    }
    if (jbool(header, "systemOpen")) {
        auto items = menu_items(header, "systemItems");
        menus.push_back({0, kSysH, sys_w, items});
    }
    std::string callsign = jstr(header, "callsign");
    if (!callsign.empty()) {
        ui::TextStyle ts{24, ui::pal::white, ui::Face::Ecam, ui::Align::Left};
        ui::text(c, callsign, kSideW - sys_w + 10, 3.4f, ts);
    }

    if (header.contains("pages") && header["pages"].is_array()) {
        const auto& pages = header["pages"];
        size_t n = pages.size();
        float row_w = kSideW * (header.contains("rowWidthPct") ? header["rowWidthPct"].get<float>() : 100.f) / 100.f;
        std::vector<float> wd(n, 0.f), mn(n, 0.f), fl(n, 0.f);
        std::vector<bool> frozen(n, false);
        float used = 0;
        ui::TextStyle lts{22, ui::pal::white, ui::Face::Ecam};
        for (size_t i = 0; i < n; ++i) {
            const json& p = pages[i];
            float longest = 0;
            for (auto& line : split_lines(jstr(p, "label"))) {
                size_t a = 0;
                while (a < line.size()) {
                    size_t b = line.find(' ', a);
                    if (b == std::string::npos) b = line.size();
                    if (b > a) longest = std::max(longest, ui::text_width(line.substr(a, b - a), lts));
                    a = b + 1;
                }
            }
            mn[i] = longest + 28 + 4 + (jbool(p, "arrow", true) ? 31 : 0);
            if (p.contains("widthPct")) {
                wd[i] = p["widthPct"].get<float>() * kSideW / 100.f;
                frozen[i] = true;
                used += wd[i];
            } else {
                fl[i] = p.contains("flex") ? p["flex"].get<float>() : 1.f;
            }
        }
        for (int iter = 0; iter < 8; ++iter) {
            float free_w = row_w, fsum = 0;
            for (size_t i = 0; i < n; ++i) {
                if (frozen[i]) free_w -= wd[i];
                else fsum += fl[i];
            }
            bool changed = false;
            for (size_t i = 0; i < n; ++i) {
                if (frozen[i]) continue;
                float w = free_w * fl[i] / std::max(fsum, 0.001f);
                if (w < mn[i]) {
                    wd[i] = mn[i];
                    frozen[i] = true;
                    changed = true;
                } else {
                    wd[i] = w;
                }
            }
            if (!changed) break;
        }
        float x = 0;
        for (size_t i = 0; i < n; ++i) {
            const json& p = pages[i];
            SkRect r = SkRect::MakeXYWH(x, kPageRowTop, wd[i], kPageRowH);
            x += wd[i];
            bool open = jbool(p, "open");
            ui::PageSelectorStyle ps;
            ps.arrow = jbool(p, "arrow", true);
            ps.color = ui::pal::white;
            if (p.contains("labelColor")) ps.color = parse_color(jstr(p, "labelColor"), ui::pal::white);
            if (p.contains("labelBg")) ps.background = parse_color(jstr(p, "labelBg"), 0);
            ui::page_selector_ex(c, r, jstr(p, "label"), jbool(p, "active"), ps);
            if (open) {
                auto items = menu_items(p, "items");
                float w = std::max(wd[i], ui::dropdown_menu_natural_width(items));
                menus.push_back({r.left(), r.bottom(), w, items});
            }
        }
    }

    float content_top = kTitleTop;
    if (header.contains("title") && header["title"].is_object()) {
        const json& t = header["title"];
        ui::TitleBar tb;
        tb.title = jstr(t, "text");
        tb.penalty = jbool(t, "penalty");
        tb.eo = jbool(t, "eo");
        tb.tmpy = jbool(t, "tmpy");
        if (t.contains("gap") && t["gap"].is_number()) tb.gap = t["gap"].get<float>();
        content_top += ui::title_bar(c, SkRect::MakeXYWH(0, kTitleTop, kSideW, 40), tb);
    }

    SkRect content = SkRect::MakeLTRB(0, content_top, kSideW, kFooterTop);
    const json* dialog = m.contains("dialog") && m["dialog"].is_object() ? &m["dialog"] : nullptr;
    if (dialog && jstr(*dialog, "kind") == "msg-list") {
        draw_msg_list(c, content, *dialog);
    } else if (PageDrawer draw = find_page(uri)) {
        static const json no_page = json::object();
        const json& page = m.contains("page") ? m["page"] : no_page;
        c.save();
        c.translate(-ox, 0);
        SkRect abs = content.makeOffset(ox, 0);
        c.clipRect(abs);
        draw(c, abs, page);
        c.restore();
    }

    ui::fill(c, SkRect::MakeLTRB(0, kFooterTop, kSideW, kFooterTop + 2), ui::pal::darker_grey);
    ui::fill(c, SkRect::MakeLTRB(0, kFooterTop + 2, kSideW, kH), ui::pal::background);
    float bw = 100;
    if (footer.contains("buttonWidth") && footer["buttonWidth"].is_number()) bw = footer["buttonWidth"].get<float>();
    if (jbool(footer, "clearInfo", true)) {
        std::vector<std::string> lines{"CLEAR", "INFO"};
        if (footer.contains("buttonLabel") && footer["buttonLabel"].is_array()) {
            lines.clear();
            for (auto& l : footer["buttonLabel"]) lines.push_back(l.get<std::string>());
        }
        ui::ButtonStyle bs;
        bs.pad_left = bs.pad_right = 12;
        ui::button_lines(c, SkRect::MakeXYWH(5.3f, kFooterBtnTop, bw, kFooterBtnH), lines, bs);
    }
    std::string msg = jstr(footer, "message");
    if (!msg.empty()) {
        ui::TextStyle ts{26, ui::pal::background, ui::Face::Mfd, ui::Align::Left};
        float lh = ui::line_height(ts);
        float cy = kFooterBtnTop + kFooterBtnH * 0.5f;
        float x = 5.3f + bw + 15;
        float avail = kSideW - 5 - x;
        std::vector<std::string> lines;
        for (auto& part : split_lines(msg)) {
            auto wrapped = ui::wrap_text(part, ts, avail);
            lines.insert(lines.end(), wrapped.begin(), wrapped.end());
        }
        SkColor mbg = parse_color(jstr(footer, "messageBg"), ui::pal::white);
        float y = cy - lh * lines.size() * 0.5f;
        for (auto& l : lines) {
            ui::message_box(c, SkRect::MakeXYWH(x, y, ui::text_width(l, ts), lh), l, mbg, ui::pal::background);
            y += lh;
        }
    }

    for (auto& pm : menus) ui::dropdown_menu(c, SkRect::MakeXYWH(pm.x, pm.y, pm.w, 0), pm.items);
    c.restore();
}

class MfdDisplay : public Display {
public:
    SkISize size() const override { return SkISize::Make(1646, 1024); }

    void draw(SkCanvas& c, const Inputs& in) override {
        bool left_on = side_unit(c, du_left_, in, 0);
        bool right_on = side_unit(c, du_right_, in, kSideW + kGap);
        const json* model = in.bus("a380.mfd.model");
        if (!model) return;
        const json* left = nullptr;
        const json* right = nullptr;
        if (model->is_array()) {
            if (model->size() > 0) left = &(*model)[0];
            if (model->size() > 1) right = &(*model)[1];
        } else if (model->is_object()) {
            for (const char* k : {"left", "captain", "capt"})
                if (!left && model->contains(k)) left = &(*model)[k];
            for (const char* k : {"right", "fo", "first_officer"})
                if (!right && model->contains(k)) right = &(*model)[k];
        }
        if (left_on && left) draw_side(c, 0, *left);
        if (right_on && right) draw_side(c, kSideW + kGap, *right);
    }

private:
    static bool side_unit(SkCanvas& c, CdsDisplayUnit& du, const Inputs& in, float ox) {
        c.save();
        c.translate(ox, 0);
        bool on = du.update(c, in, SkISize::Make((int)kSideW, (int)kH));
        c.restore();
        return on;
    }

    CdsDisplayUnit du_left_{"98", {"DC_ESS", "DC_1"}};
    CdsDisplayUnit du_right_{"99", {"DC_1", "DC_2"}};
};

std::unique_ptr<Display> make_mfd(std::string_view) { return std::make_unique<MfdDisplay>(); }

}  // namespace

A380_REGISTER_DISPLAY("mfd", make_mfd);

}  // namespace a380::mfd
