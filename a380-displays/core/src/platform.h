#pragma once

#include <string>

#include "include/core/SkFontMgr.h"
#include "include/core/SkRefCnt.h"

namespace a380::platform {

sk_sp<SkFontMgr> font_manager();
std::string default_font_dir();
std::string default_amdb_dir();

}  // namespace a380::platform
