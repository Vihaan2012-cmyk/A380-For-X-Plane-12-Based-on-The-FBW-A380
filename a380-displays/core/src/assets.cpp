#include "a380/assets.h"
#include "a380/env.h"

#include <cstdlib>
#include <mutex>

#include "platform.h"

namespace a380::assets {

namespace {

struct Config {
    std::mutex mu;
    std::string fonts, images, ife, amdb, glyphs;
};

Config& config() {
    static Config c;
    return c;
}

std::string env(const char* key) {
    const char* v = a380::getenv_cached(key);
    return v && *v ? v : "";
}

std::string trim_slashes(std::string s) {
    while (!s.empty() && (s.back() == '/' || s.back() == '\\')) s.pop_back();
    return s;
}

std::string parent(const std::string& s) {
    std::string t = trim_slashes(s);
    size_t cut = t.find_last_of("/\\");
    return cut == std::string::npos ? "." : t.substr(0, cut);
}

void add(std::vector<std::string>& v, const std::string& s) {
    if (s.empty()) return;
    std::string t = trim_slashes(s);
    for (auto& e : v)
        if (e == t) return;
    v.push_back(std::move(t));
}

}  // namespace

bool set(const std::string& kind, const std::string& path) {
    Config& c = config();
    std::lock_guard<std::mutex> lock(c.mu);
    std::string p = trim_slashes(path);
    if (kind == "html_ui") {
        c.fonts = p.empty() ? p : p + "/Fonts";
        c.images = p.empty() ? p : p + "/Images/fbw-a380x";
        c.ife = p.empty() ? p : p + "/Pages/VCockpit/Instruments/A380X/IFE";
    } else if (kind == "fonts") c.fonts = p;
    else if (kind == "images") c.images = p;
    else if (kind == "ife") c.ife = p;
    else if (kind == "amdb") c.amdb = p;
    else if (kind == "glyphs") c.glyphs = p;
    else return false;
    return true;
}

std::string fonts_dir() {
    Config& c = config();
    {
        std::lock_guard<std::mutex> lock(c.mu);
        if (!c.fonts.empty()) return c.fonts;
    }
    std::string e = env("A380_FONT_DIR");
    return e.empty() ? platform::default_font_dir() : e;
}

std::vector<std::string> images_dirs() {
    std::vector<std::string> out;
    {
        Config& c = config();
        std::lock_guard<std::mutex> lock(c.mu);
        add(out, c.images);
    }
    add(out, env("A380_IMAGE_DIR"));
    add(out, parent(fonts_dir()) + "/Images/fbw-a380x");
    return out;
}

std::vector<std::string> ife_dirs() {
    std::vector<std::string> out;
    {
        Config& c = config();
        std::lock_guard<std::mutex> lock(c.mu);
        add(out, c.ife);
    }
    add(out, env("A380_IFE_DIR"));
    add(out, parent(fonts_dir()) + "/Pages/VCockpit/Instruments/A380X/IFE");
    add(out, "html_ui/Pages/VCockpit/Instruments/A380X/IFE");
    return out;
}

std::string amdb_dir() {
    Config& c = config();
    {
        std::lock_guard<std::mutex> lock(c.mu);
        if (!c.amdb.empty()) return c.amdb;
    }
    std::string e = env("A380_AMDB_DIR");
    return e.empty() ? platform::default_amdb_dir() : e;
}

std::string glyphs_file() {
    {
        Config& c = config();
        std::lock_guard<std::mutex> lock(c.mu);
        if (!c.glyphs.empty()) return c.glyphs;
    }
    std::string e = env("A380_GLYPH_ATLAS");
    return e.empty() ? trim_slashes(fonts_dir()) + "/a380-glyphs.atl" : e;
}

sk_sp<SkData> image_data(const std::string& relative) {
    for (auto& dir : images_dirs()) {
        if (auto data = SkData::MakeFromFileName((dir + "/" + relative).c_str())) return data;
    }
    return nullptr;
}

}  // namespace a380::assets
