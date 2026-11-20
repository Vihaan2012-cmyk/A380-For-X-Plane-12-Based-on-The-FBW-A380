#include "a380/input.h"

#include <cmath>
#include <cstdlib>
#include <functional>
#include <utility>

#include <nlohmann/json.hpp>

namespace a380 {
namespace {

struct Button {
    double x0, y0, x1, y1;
    std::vector<Write> writes;
    std::function<bool()> shown;

    bool contains(double x, double y) const { return x >= x0 && x < x1 && y >= y0 && y < y1; }
};

bool bus_true(const Inputs& in, std::string_view topic) {
    const nlohmann::json* v = in.bus(topic);
    return v && ((v->is_boolean() && v->get<bool>()) || (v->is_number() && v->get<double>() != 0.0));
}

bool bus_nonempty(const Inputs& in, std::string_view topic) {
    const nlohmann::json* v = in.bus(topic);
    return v && v->is_array() && !v->empty();
}

std::vector<Write> momentary(const char* lvar) {
    return {{'L', lvar, 1.0, 0.0}, {'L', lvar, 0.0, 50.0}};
}

// DOM 'click': press and release on the same element.
class Buttons : public InputHandler {
public:
    explicit Buttons(bool owns) : owns_(owns) {}

    void observe(const Inputs& in) override {
        soft_keys_ = in.str("CONFIG_A380X_SHOW_ECL_SOFTKEYS") == "1";
        observe_more(in);
    }

    bool pointer(double x, double y, PointerKind kind, int, std::vector<Write>& out) override {
        int hit = -1;
        for (size_t i = 0; i < buttons_.size(); i++)
            if (buttons_[i].contains(x, y) && (!buttons_[i].shown || buttons_[i].shown())) hit = (int)i;
        switch (kind) {
            case PointerKind::Down:
                pressed_ = hit;
                return hit >= 0;
            case PointerKind::Up: {
                int was = pressed_;
                pressed_ = -1;
                if (hit < 0 || hit != was) return hit >= 0;
                out.insert(out.end(), buttons_[hit].writes.begin(), buttons_[hit].writes.end());
                return true;
            }
            default:
                return hit >= 0;
        }
    }

    bool key(int, int, bool, std::vector<Write>&) override { return false; }
    bool owns_input() const override { return owns_; }

protected:
    virtual void observe_more(const Inputs&) {}

    std::vector<Button> buttons_;
    bool soft_keys_ = false;

private:
    bool owns_;
    int pressed_ = -1;
};

// EWD EclSoftKeys: .ewd-main 768x1024, column at top 855 right 3, 50x50 keys, 5 px apart.
class Ewd : public Buttons {
public:
    Ewd() : Buttons(true) {
        auto shown = [this] { return soft_keys_ && procedures_; };
        buttons_.push_back({715, 855, 765, 905, momentary("L:A32NX_BTN_CHECK_LH"), shown});
        buttons_.push_back({715, 910, 765, 960, momentary("L:A32NX_BTN_UP"), shown});
        buttons_.push_back({715, 965, 765, 1015, momentary("L:A32NX_BTN_DOWN"), shown});
    }

private:
    void observe_more(const Inputs& in) override {
        procedures_ = bus_true(in, "fws_show_normal_checklists") || bus_true(in, "fws_show_abn_sensed") ||
                      bus_true(in, "fws_show_abn_non_sensed");
    }

    bool procedures_ = false;
};

// SDv2 STATUS page SdSoftKey: .sd-sts-top-div 768x665, key at top 605 right 3, 50x50.
class Sdv2 : public Buttons {
public:
    Sdv2() : Buttons(true) {
        buttons_.push_back({715, 605, 765, 655, {{'H', "H:A32NX_SD_STS_NEXT_PAGE", 1.0, 0.0}}, [this] { return soft_keys_ && status_abnormal_; }});
    }

private:
    void observe_more(const Inputs& in) override {
        const bool page = (int)in.var("L:A32NX_ECAM_SD_CURRENT_PAGE_INDEX") == 14;
        bool any = false;
        for (const char* t : {"fws_limitations_all_phases", "fws_limitations_appr_ldg", "fws_deferred_procedures", "fws_information",
                              "fws_inop_sys_all_phases", "fws_inop_sys_appr_ldg", "fws_inop_sys_redundancy_loss"})
            any = any || bus_nonempty(in, t);
        status_abnormal_ = page && any;
    }

    bool status_abnormal_ = false;
};

// ND OANS: click targets {"r":[x,y,w,h],"ev":name} from the host's a380.oans.panel.<side> model.
class Nd : public InputHandler {
public:
    explicit Nd(std::string_view variant) : side_(variant == "2" ? 2 : 1) {}

    void observe(const Inputs& in) override {
        targets_.clear();
        const nlohmann::json* m = in.bus(side_ == 2 ? "a380.oans.panel.2" : "a380.oans.panel.1");
        owns_ = m && m->is_object();
        if (!owns_) return;
        const char* s = side_ == 2 ? "R" : "L";
        const int range = (int)std::lround(in.var(std::string("L:A32NX_EFIS_") + s + "_ND_RANGE", {}, 3));
        const int mode = (int)std::lround(in.var(std::string("L:A32NX_EFIS_") + s + "_ND_MODE", {}, 3));
        if (!(range == 0 && (mode == 2 || mode == 3 || mode == 4))) return;
        auto it = m->find("hits");
        if (it == m->end() || !it->is_array()) return;
        for (const auto& h : *it) {
            if (!h.is_object() || !h.contains("r") || !h["r"].is_array() || h["r"].size() != 4 || !h.contains("ev") || !h["ev"].is_string()) continue;
            const auto& r = h["r"];
            Target t{r[0].get<double>(), r[1].get<double>(), r[0].get<double>() + r[2].get<double>(), r[1].get<double>() + r[3].get<double>(), h["ev"].get<std::string>(),
                     h.value("wu", std::string()), h.value("wd", std::string())};
            targets_.push_back(std::move(t));
        }
    }

    bool pointer(double x, double y, PointerKind kind, int delta, std::vector<Write>& out) override {
        int hit = -1;
        for (size_t i = 0; i < targets_.size(); i++)
            if (x >= targets_[i].x0 && x < targets_[i].x1 && y >= targets_[i].y0 && y < targets_[i].y1) hit = (int)i;
        if (kind == PointerKind::Wheel) {
            if (hit < 0 || delta == 0) return hit >= 0;
            const std::string& ev = delta > 0 ? targets_[hit].wheel_up : targets_[hit].wheel_down;
            for (int n = std::abs(delta); !ev.empty() && n > 0; n--) out.push_back({'H', "H:" + ev, 1.0, 0.0});
            return true;
        }
        if (hit >= 0 && targets_[hit].ev.empty()) hit = -1;
        if (kind == PointerKind::Down) {
            pressed_ = hit >= 0 ? targets_[hit].ev : std::string();
            return hit >= 0;
        }
        if (kind == PointerKind::Up) {
            std::string was = std::move(pressed_);
            pressed_.clear();
            if (hit < 0 || targets_[hit].ev != was) return hit >= 0;
            out.push_back({'H', "H:" + was, 1.0, 0.0});
            return true;
        }
        return hit >= 0;
    }

    bool key(int, int, bool, std::vector<Write>&) override { return false; }
    bool owns_input() const override { return owns_; }

private:
    struct Target {
        double x0, y0, x1, y1;
        std::string ev, wheel_up, wheel_down;
    };
    int side_;
    bool owns_ = false;
    std::vector<Target> targets_;
    std::string pressed_;
};

}  // namespace

std::unique_ptr<InputHandler> make_input_handler(std::string_view id, std::string_view variant) {
    if (id == "ewd") return std::make_unique<Ewd>();
    if (id == "sdv2") return std::make_unique<Sdv2>();
    if (id == "nd") return std::make_unique<Nd>(variant);
    // Page logic still in the page: MFD/OIT/EFB pages.
    const bool page_owned = id == "mfd" || id == "mfdx" || id == "oit" || id == "oitx" || id == "efbx";
    return std::make_unique<Buttons>(!page_owned);
}

}  // namespace a380
