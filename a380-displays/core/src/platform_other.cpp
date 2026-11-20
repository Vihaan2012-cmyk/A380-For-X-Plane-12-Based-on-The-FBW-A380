#include "platform.h"

#include "include/ports/SkFontMgr_empty.h"

namespace a380::platform {

sk_sp<SkFontMgr> font_manager() { return SkFontMgr_New_Custom_Empty(); }

std::string default_font_dir() { return "html_ui/Fonts"; }

std::string default_amdb_dir() { return "Output/fbw-a380x/amdb"; }

}  // namespace a380::platform
