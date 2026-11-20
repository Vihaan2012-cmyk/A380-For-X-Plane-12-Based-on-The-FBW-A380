#pragma once

#include <string>
#include <string_view>

#include "nlohmann/json.hpp"

#include "../mfd_pages.h"
#include "../widgets.h"

namespace a380::mfd::pp {

constexpr float kScale = 2000.0f / 1646.0f;

struct Nudge {
    SkCanvas& c;
    Nudge(SkCanvas& cv, float dx_dev, float dy_dev) : c(cv) {
        c.save();
        const SkMatrix m = c.getTotalMatrix();
        c.translate(dx_dev / m.getScaleX(), dy_dev / m.getScaleY());
    }
    ~Nudge() { c.restore(); }
};

struct Frame {
    SkCanvas& c;
    SkRect content;
    float ox;
    float oy;
    float lx = 2.2f;
    float X(float d) const { return content.fLeft + (d - ox) / kScale; }
    float Y(float d) const { return content.fTop + (d - oy) / kScale; }
    float W(float d) const { return d / kScale; }
    SkRect R(float l, float t, float r, float b) const { return SkRect::MakeLTRB(X(l), Y(t), X(r), Y(b)); }

    void text(std::string_view s, float x, float cy, float size, SkColor color, ui::Align a = ui::Align::Left) const {
        ui::TextStyle st = ui::label_style(color, size);
        st.align = a;
        const float x0 = X(x + lx);
        ui::text(c, s, x0, Y(cy) - ui::line_height(st) * 0.5f, st);
    }
    void val(std::string_view s, float x, float cy, float size, SkColor color, ui::Align a = ui::Align::Left) const {
        ui::TextStyle st = ui::value_style(color, size);
        st.align = a;
        ui::text(c, s, X(x), Y(cy) - ui::line_height(st) * 0.5f, st);
    }
    void rule(float x0, float x1, float y, SkColor color = ui::pal::light_grey) const {
        ui::hline(c, X(x0), X(x1), Y(y), color, 0.667f);
    }
};

inline std::string str(const nlohmann::json& j, const char* key, const char* fallback = "") {
    if (j.is_object() && j.contains(key) && j[key].is_string()) return j[key].get<std::string>();
    return fallback;
}

inline bool flag(const nlohmann::json& j, const char* key, bool fallback = false) {
    if (j.is_object() && j.contains(key) && j[key].is_boolean()) return j[key].get<bool>();
    return fallback;
}

inline const nlohmann::json& sub(const nlohmann::json& j, const char* key) {
    static const nlohmann::json empty = nlohmann::json::object();
    if (j.is_object() && j.contains(key) && j[key].is_object()) return j[key];
    return empty;
}

inline ui::FieldStyle field_style(const nlohmann::json& f) {
    ui::FieldStyle st;
    std::string state = str(f, "state", "input");
    bool computed = false;
    if (f.is_object() && f.contains("cls") && f["cls"].is_array())
        for (auto& c : f["cls"])
            if (c == "computedByFms") computed = true;
    if (state == "mandatory") st.kind = ui::FieldKind::Mandatory;
    else if (state == "disabled") st.kind = ui::FieldKind::Disabled;
    else if (state == "inactive") { st.kind = ui::FieldKind::Inactive; st.border = false; }
    else if (state == "validating") st.kind = ui::FieldKind::Validating;
    else if (computed) { st.kind = ui::FieldKind::Entered; st.font_size = 22; }
    else st.kind = ui::FieldKind::Entered;
    st.unit_leading = str(f, "lead");
    st.unit_trailing = str(f, "trail");
    return st;
}

}  // namespace a380::mfd::pp
