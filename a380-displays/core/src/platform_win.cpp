#include "platform.h"

#include <string>

#include <windows.h>
#include <objbase.h>

#include "include/ports/SkTypeface_win.h"

namespace a380::platform {

sk_sp<SkFontMgr> font_manager() {
    static bool com = [] {
        CoInitializeEx(nullptr, COINIT_MULTITHREADED);
        return true;
    }();
    (void)com;
    return SkFontMgr_New_DirectWrite();
}

std::string default_font_dir() {
    return "D:/Steam Games/steamapps/common/X-Plane 12/Aircraft/A380 for X-Plane 12/html_ui/Fonts";
}

std::string default_amdb_dir() {
    return "D:/Steam Games/steamapps/common/X-Plane 12/Output/fbw-a380x/amdb";
}

}  // namespace a380::platform
