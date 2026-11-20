#include "procedure_common.h"

namespace a380::mfd {
namespace {

using namespace proc;

void draw_departure(SkCanvas& c, SkRect content, const json& page) {
    pm::GreyLayer grey(c);
    const json& f = at(page, "fields");
    box_frame(c, content, page, "SELECTED DEPARTURE");

    cell(c, content, f, {"FROM", 15.3f, 192.3f, 192.3f}, "FROM", 80.7f);
    cell(c, content, f, {"LS", 261.3f, 176.3f, 206.3f});
    cell(c, content, f, {"RWY", 384.f, 176.3f, 206.3f});
    cell(c, content, f, {"LENGTH", 506.7f, 176.3f, 206.3f});
    cell(c, content, f, {"CRS", 666.f, 176.3f, 206.3f});
    cell(c, content, f, {"EOSID", 15.3f, 236.3f, 266.3f});
    cell(c, content, f, {"FREQ/CHAN", 200.f, 236.3f, 266.3f});
    cell(c, content, f, {"SID", 420.7f, 236.3f, 266.3f});
    cell(c, content, f, {"TRANS", 604.7f, 236.3f, 266.3f});

    const json& bs = at(page, "buttons");
    const float xs[3] = {50, 400, 590}, ws[3] = {250, 140, 130};
    if (bs.is_array())
        for (size_t i = 0; i < bs.size() && i < 3; i++) dropdown_button(c, content, bs[i], xs[i], ws[i]);

    return_button(c, content, page);
    tmpy_button(c, content, page);
    if (bs.is_array())
        for (size_t i = 0; i < bs.size() && i < 3; i++) open_menu(c, content, bs[i], xs[i], ws[i]);
}

}  // namespace

A380_MFD_PAGE("fms/active/f-pln/departure", draw_departure);

}  // namespace a380::mfd
