#pragma once

#include <string_view>

#include "a380/layout.h"
#include "nlohmann/json.hpp"

namespace a380::mfdx {

nlohmann::json dom_to_tree(std::string_view html, bool keep_attr_case = false);

layout::Tree dom_to_layout(std::string_view html, bool keep_attr_case, float dpr);

}
