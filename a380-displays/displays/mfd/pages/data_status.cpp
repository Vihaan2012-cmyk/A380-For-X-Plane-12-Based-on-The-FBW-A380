#include "pp_layout.h"
#include "page_model.h"

namespace a380::mfd {
namespace {
using namespace ui;
using nlohmann::json;

SkRect nudge(const SkCanvas& c, SkRect r, float dl, float dt, float dr, float db) {
    const float s = c.getTotalMatrix().getScaleX();
    return SkRect::MakeLTRB(r.fLeft + dl / s, r.fTop + dt / s, r.fRight + dr / s, r.fBottom + db / s);
}

void thick_rule(SkCanvas& c, const pp::Frame& f, float x0, float x1, float y, float dy, float thick, SkColor col) {
    const float s = c.getTotalMatrix().getScaleY();
    const float row = std::round(f.Y(y) * s) + dy;
    ui::fill(c, SkRect::MakeLTRB(f.X(x0), row / s, f.X(x1) + 2 / s, (row + thick) / s), col);
}

void star_button(SkCanvas& c, SkRect r, std::string_view label, bool disabled, float label_dx, float star_dx, float dy) {
    ButtonStyle bg;
    bg.state = disabled ? ButtonState::Disabled : ButtonState::Normal;
    button(c, r, "", bg);
    const float s = c.getTotalMatrix().getScaleX();
    TextStyle ts = label_style(disabled ? pal::dark_grey : pal::white, 22);
    ts.align = Align::Left;
    const float y = (r.top() + r.bottom()) * 0.5f - line_height(ts) * 0.5f;
    text(c, label, r.left() + label_dx / s, y + dy / s, ts);
    ts.align = Align::Right;
    text(c, "*", r.right() - star_dx / s, y + dy / s, ts);
}

void tx(SkCanvas& c, const pp::Frame& f, bool val, std::string_view t, float x, float y, float sz, SkColor col, ui::Align a,
        float dx, float dy) {
    const float k = pp::kScale / c.getTotalMatrix().getScaleX();
    if (val) f.val(t, x + dx * k, y + dy * k, sz, col, a);
    else f.text(t, x + dx * k, y + dy * k, sz, col, a);
}

void draw_data_status(SkCanvas& c, SkRect content, const json& page) {
    pm::GreyLayer grey(c);
    pp::Frame f{c, content, 1067.4f, 159.5f, 2.2f};
    const json& d = page;

    std::vector<std::string> tabs = {"ACFT STATUS", "FMS P/N"};
    TabStyle ts;
    ts.slant_deg = 25;
    ts.selected = pp::str(d, "tab", "ACFT STATUS") == "FMS P/N" ? 1 : 0;
    tab_navigator(c, SkRect::MakeLTRB(content.fLeft, content.fTop, content.fRight, content.fBottom - 26.13f), tabs, ts);
    if (ts.selected == 1) return;

    TextStyle big = value_bigger_style(pal::green);
    big.align = Align::Center;
    tx(c, f, true, pp::str(d, "airframe", "A380-800 / TRENT 972"), 1530, 260, 30, pal::green, ui::Align::Center, 5, -2);

    stroke_box(c, nudge(c, f.R(1093, 517, 1888, 589), 0, 1, 1, 0), pal::light_grey, 0.667f);
    tx(c, f, false, "IDLE", 1104, 553, 24, pal::white, ui::Align::Left, 0, 0);
    tx(c, f, true, pp::str(d, "idle", "+0.0"), 1210, 553, 30, pal::green, ui::Align::Left, -1, 0);
    tx(c, f, false, "PERF", 1372, 553, 24, pal::white, ui::Align::Left, -1, 0);
    tx(c, f, true, pp::str(d, "perf", "+0.0"), 1477, 553, 30, pal::green, ui::Align::Left, 0, 0);
    ButtonStyle dis;
    dis.state = ButtonState::Disabled;
    button(c, nudge(c, f.R(1683, 530, 1835, 576), 1, 0, 1, 0), "MODIFY", dis);

    stroke_box(c, nudge(c, f.R(1093, 601, 1554, 672), 0, 0, 1, 0), pal::light_grey, 0.667f);
    tx(c, f, false, "FUEL PENALTY", 1103, 637, 24, pal::white, ui::Align::Left, 1, -1);
    const json& fp = pp::sub(d, "fuelPenalty");
    FieldStyle fs = pp::field_style(fp);
    if (fs.unit_trailing.empty()) fs.unit_trailing = "%";
    input_field(c, nudge(c, f.R(1344, 614, 1532, 659), -1, 0, -1, 0), pp::str(fp, "v", "+000.0"), fs);

    thick_rule(c, f, 1093, 1982, 685, -1, 1.47f, SkColorSetRGB(242, 244, 255));
    tx(c, f, false, "NAV DATABASE", 1103, 722, 24, pal::white, ui::Align::Left, 0, 1);
    tx(c, f, true, pp::str(d, "navDb", "MS24060001"), 1354, 722, 30, pal::green, ui::Align::Left, 1, -2);

    stroke_box(c, nudge(c, f.R(1093, 755, 1382, 897), 0, 0, 1, 0), pal::light_grey, 0.667f);
    tx(c, f, false, "ACTIVE", 1236, 793, 24, pal::white, ui::Align::Center, -1, 0);
    tx(c, f, true, pp::str(d, "navActive", "13JUN-11JUL"), 1236, 853, 30, pal::green, ui::Align::Center, 2, -1);

    star_button(c, nudge(c, f.R(1467, 789, 1637, 863), 1, -1, 1, 0), "SWAP", false, 45, 21, 3);
    tx(c, f, false, "SECOND", 1826, 800.8f, 24, pal::white, ui::Align::Center, 0, 0);
    tx(c, f, true, pp::str(d, "navSecond", "13JUN-11JUL"), 1848, 858.5f, 25, pal::green, ui::Align::Center, -1, 1);

    thick_rule(c, f, 1093, 1982, 929, -2, 3, pal::light_grey);
    tx(c, f, false, "PILOT STORED ELEMENTS", 1114, 959.8f, 24, pal::white, ui::Align::Left, 0, 1);
    const json& p = pp::sub(d, "stored");
    tx(c, f, false, "WAYPOINTS", 1128, 1009.8f, 24, pal::white, ui::Align::Left, -2, 1);
    tx(c, f, true, pp::str(p, "waypoints", "00"), 1305, 1009.8f, 30, pal::green, ui::Align::Left, 0, 1);
    tx(c, f, false, "ROUTES", 1435, 1009.8f, 24, pal::white, ui::Align::Left, -1, 1);
    tx(c, f, true, pp::str(p, "routes", "00"), 1558, 1009.8f, 30, pal::green, ui::Align::Left, 0, 1);
    tx(c, f, false, "NAVAIDS", 1163, 1069.8f, 24, pal::white, ui::Align::Left, 0, 0);
    tx(c, f, true, pp::str(p, "navaids", "00"), 1305, 1069.8f, 30, pal::green, ui::Align::Left, 0, 0);
    tx(c, f, false, "RUNWAYS", 1417, 1069.8f, 24, pal::white, ui::Align::Left, -1, 0);
    tx(c, f, true, pp::str(p, "runways", "00"), 1558, 1069.8f, 30, pal::green, ui::Align::Left, 0, 0);
    star_button(c, nudge(c, f.R(1745, 1006, 1990, 1076), 0, -1, 1, 0), "DELETE ALL", true, 20, 23, 2);
}

A380_MFD_PAGE("fms/data/status", draw_data_status);
}  // namespace
}  // namespace a380::mfd
