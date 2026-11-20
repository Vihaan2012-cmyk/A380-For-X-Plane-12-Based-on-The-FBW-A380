#include "page_model.h"

namespace a380::mfd {
namespace {

using namespace pm;

void draw_init(SkCanvas& c, SkRect content, const json& page) {
    pm::GreyLayer grey(c);
    const json& f = at(page, "fields");
    const json& b = at(page, "buttons");
    const float L = 145;
    SkRect r;

    r = box(content, 150, 14.67, 350, 52);
    label_right(c, "FLT NBR", L, r);
    pm::field(c, r, at(f, "flt_nbr"), ui::Align::Center, 180);
    button(c, box(content, 355.33, 14.67, 555.33, 52), at(b, "acft_status"));
    button(c, box(content, 593.33, 5.33, 766, 62), at(b, "cpny_fpln_request"));

    r = box(content, 150, 72, 242, 108.67);
    label_right(c, "FROM", L, r);
    pm::field(c, r, at(f, "from"), ui::Align::Center, 72);
    label_left(c, "TO", 252, r);
    pm::field(c, box(content, 287.33, 72, 379.33, 108.67), at(f, "to"), ui::Align::Center, 72);
    label_left(c, "ALTN", 389.33, r);
    pm::field(c, box(content, 449.33, 72, 541.33, 108.67), at(f, "altn"), ui::Align::Center, 72);

    r = box(content, 150, 124, 350, 161.33);
    label_right(c, "CPNY RTE", L, SkRect::MakeLTRB(r.left(), r.top() - 0.67f, r.right(), r.bottom() - 0.67f));
    pm::field(c, r, at(f, "cpny_rte"), ui::Align::Center, 180);
    button(c, box(content, 355.33, 124, 555.33, 161.33), at(b, "rte_sel"));

    r = box(content, 150, 166, 350, 203.33);
    label_right(c, "ALTN RTE", L, r);
    pm::field(c, r, at(f, "altn_rte"), ui::Align::Center, 180);
    button(c, box(content, 355.33, 166, 555.33, 203.33), at(b, "altn_rte_sel"));

    thin_line(c, content, 0, 768, 360.0f);

    r = box(content, 150, 259.33, 249.33, 296);
    label_right(c, "CRZ FL", L, r);
    pm::field(c, r, at(f, "crz_fl"), ui::Align::Center, 54);
    label_left(c, "CRZ TEMP", 350, r);
    pm::field(c, box(content, 455.33, 259.33, 565.33, 296), at(f, "crz_temp"), ui::Align::Center, 54);

    r = box(content, 150, 306, 325.33, 350);
    label_right(c, "MODE", L, r);
    const json& mode = at(page, "mode");
    const SkRect mode_r = r;
    dropdown_box(c, r, mode);
    label_left(c, "TROPO", 390, r);
    pm::field(c, box(content, 458, 309.33, 592.67, 346.67), at(f, "tropo"), ui::Align::Right, 90);

    r = box(content, 150, 360, 220, 397.33);
    label_right(c, "CI", L, r);
    pm::field(c, r, at(f, "ci"), ui::Align::Center, 54);
    r = box(content, 150, 407.33, 275.33, 444);
    label_right(c, "TRIP WIND", L, SkRect::MakeLTRB(r.left(), r.top() - 1.33f, r.right(), r.bottom() - 1.33f));
    pm::field(c, r, at(f, "trip_wind"), ui::Align::Left, 90);
    button(c, box(content, 355.33, 407.33, 438, 444.67), at(b, "wind"));
    button(c, box(content, 580, 370.67, 758, 428.67), at(b, "cpny_wind_request"));

    thin_line(c, content, 0, 768, 610.7f);

    { pp::Nudge left_column(c, 0, -1); button(c, box(content, 150, 495.33, 310, 532.67), at(b, "irs")); }
    { pp::Nudge left_column(c, 0, -1); button(c, box(content, 150, 542.67, 310, 580), at(b, "departure")); }
    button(c, box(content, 360, 542.67, 540, 580), at(b, "rte_summary"));
    { pp::Nudge left_column(c, 0, -1); button(c, box(content, 150, 590, 310, 627.33), at(b, "navaids")); }
    { pp::Nudge left_column(c, 0, -1); button(c, box(content, 150, 637.33, 310, 674.67), at(b, "fuel_load")); }
    { pp::Nudge left_column(c, 0, -1); button(c, box(content, 150, 684.67, 310, 724.67), at(b, "to_perf")); }
    button(c, box(content, 583.33, 684.67, 758, 741.33), at(b, "cpny_to_request"));
    dropdown_open_overlay(c, mode_r, mode);
}

}  // namespace

A380_MFD_PAGE("fms/active/init", draw_init);

}  // namespace a380::mfd
