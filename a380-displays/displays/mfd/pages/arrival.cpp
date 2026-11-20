#include "procedure_common.h"

namespace a380::mfd {
namespace {

using namespace proc;

void draw_arrival(SkCanvas& c, SkRect content, const json& page) {
    pm::GreyLayer grey(c);
    const json& f = at(page, "fields");
    box_frame(c, content, page, "SELECTED ARRIVAL");

    cell(c, content, f, {"TO", 15.3f, 192.3f, 192.3f}, "TO", 56.f);
    cell(c, content, f, {"LS", 162.7f, 176.3f, 206.3f});
    cell(c, content, f, {"RWY", 310.f, 176.3f, 206.3f});
    cell(c, content, f, {"LENGTH", 457.3f, 176.3f, 206.3f});
    cell(c, content, f, {"CRS", 604.7f, 176.3f, 206.3f});
    cell(c, content, f, {"APPR", 15.3f, 236.3f, 266.3f});
    cell(c, content, f, {"FREQ/CHAN", 162.7f, 236.3f, 266.3f});
    cell(c, content, f, {"VIA", 310.f, 236.3f, 266.3f});
    cell(c, content, f, {"STAR", 457.3f, 236.3f, 266.3f});
    cell(c, content, f, {"TRANS", 604.7f, 236.3f, 266.3f});

    const json& bs = at(page, "buttons");
    const float xs[5] = {0, 200.75f, 371.5f, 507.25f, 643};
    const float ws[5] = {190, 160, 125, 125, 125};
    if (bs.is_array())
        for (size_t i = 0; i < bs.size() && i < 5; i++) dropdown_button(c, content, bs[i], xs[i], ws[i]);

    return_button(c, content, page);
    tmpy_button(c, content, page);
    if (bs.is_array())
        for (size_t i = 0; i < bs.size() && i < 5; i++) open_menu(c, content, bs[i], xs[i], ws[i]);
}

}  // namespace

A380_MFD_PAGE("fms/active/f-pln/arrival", draw_arrival);

}  // namespace a380::mfd
