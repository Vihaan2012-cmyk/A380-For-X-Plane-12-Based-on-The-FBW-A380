#pragma once

#include <string>
#include <vector>

#include "include/core/SkData.h"
#include "include/core/SkRefCnt.h"

namespace a380::assets {

// kind: "html_ui" (sets fonts, images and ife below it), "fonts", "images", "ife", "amdb", "glyphs" (the glyph atlas file).
// An empty path clears the setting. Returns false for an unknown kind.
// Settings win over the environment variables of the offline tools.
bool set(const std::string& kind, const std::string& path);

std::string fonts_dir();
std::vector<std::string> images_dirs();
std::vector<std::string> ife_dirs();
std::string amdb_dir();

// The baked glyph atlas: set("glyphs"), else A380_GLYPH_ATLAS, else a380-glyphs.atl in the font folder.
std::string glyphs_file();

sk_sp<SkData> image_data(const std::string& relative);

}  // namespace a380::assets
