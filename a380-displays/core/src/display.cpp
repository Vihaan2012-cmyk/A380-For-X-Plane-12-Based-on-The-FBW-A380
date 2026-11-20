#include "a380/display.h"

#include <map>

namespace a380 {

namespace {
std::map<std::string, DisplayFactory>& registry() {
    static std::map<std::string, DisplayFactory> r;
    return r;
}
}  // namespace

bool register_display(const char* id, DisplayFactory factory) {
    registry()[id] = factory;
    return true;
}

std::unique_ptr<Display> create_display(std::string_view id, std::string_view variant) {
    auto& r = registry();
    auto it = r.find(std::string(id));
    if (it == r.end() || !it->second) return nullptr;
    return it->second(variant);
}

std::vector<std::string> display_ids() {
    std::vector<std::string> ids;
    for (auto& [k, v] : registry()) ids.push_back(k);
    return ids;
}

}  // namespace a380
