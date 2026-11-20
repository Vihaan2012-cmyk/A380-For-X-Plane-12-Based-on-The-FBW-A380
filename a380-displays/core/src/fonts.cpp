#include "a380/fonts.h"
#include "a380/env.h"

#include <algorithm>
#include <cctype>
#include <cstdlib>
#include <filesystem>
#include <map>
#include <mutex>
#include <string>
#include <vector>

#include "include/core/SkFontTypes.h"

#include "a380/assets.h"
#include "platform.h"

namespace a380 {

namespace fs = std::filesystem;

namespace {

struct Face {
    int weight;
    const char* file;
};

std::string lower(std::string_view s) {
    std::string r(s);
    std::transform(r.begin(), r.end(), r.begin(), [](unsigned char c) { return (char)std::tolower(c); });
    return r;
}

const std::map<std::string, std::vector<Face>>& family_table() {
    static const std::map<std::string, std::vector<Face>> t = {
        {"a1000", {{400, "a1000.ttf"}}},
        {"airbuschronometer", {{400, "AirbusChronometer.ttf"}}},
        {"airbusrmp", {{400, "AirbusRMP.ttf"}}},
        {"airbusrtpi", {{400, "AirbusRTPI.ttf"}}},
        {"b612", {{400, "B612-Regular.ttf"}, {700, "B612-Bold.ttf"}}},
        {"digital", {{400, "A380X_FCU.ttf"}, {900, "AirbusBAT.ttf"}}},
        {"ecam", {{400, "FBW-Display-EIS-A380.ttf"}}},
        {"fbw-display-eis-a380", {{400, "FBW-Display-EIS-A380.ttf"}}},
        {"fbw-display-eis-a380-slashedzero", {{400, "FBW-Display-EIS-A380-SlashedZero.ttf"}}},
        {"honeywellmcdu", {{400, "HoneywellMCDU.ttf"}}},
        {"honeywellmcdusmall", {{400, "HoneywellMCDUSmall.ttf"}}},
        {"inter", {{400, "Inter-Regular.ttf"}, {600, "Inter-SemiBold.ttf"}}},
        {"oit", {{400, "Inter-Regular.ttf"}, {600, "Inter-SemiBold.ttf"}}},
        {"jetbrains mono", {{400, "JetBrainsMono-Regular.ttf"}}},
        {"oitmono", {{400, "JetBrainsMono-Regular.ttf"}}},
        {"manrope",
         {{300, "Manrope-Light.ttf"},
          {400, "Manrope-Regular.ttf"},
          {500, "Manrope-Medium.ttf"},
          {600, "Manrope-SemiBold.ttf"},
          {700, "Manrope-Bold.ttf"}}},
        {"oitmanrope", {{400, "Manrope-Regular.ttf"}, {600, "Manrope-SemiBold.ttf"}, {700, "Manrope-Bold.ttf"}}},
        {"ndchrono", {{400, "NDChrono.ttf"}}},
        {"notosansarabic", {{400, "NotoSansArabic-Regular.ttf"}, {700, "NotoSansArabic-Bold.ttf"}}},
        {"notosansdevanagari", {{400, "NotoSansDevanagari-Regular.ttf"}, {700, "NotoSansDevanagari-Bold.ttf"}}},
        {"notosanshebrew", {{400, "NotoSansHebrew-Regular.ttf"}, {700, "NotoSansHebrew-Bold.ttf"}}},
        {"notosansjp", {{400, "NotoSansJP-Regular.otf"}, {700, "NotoSansJP-Bold.otf"}}},
        {"notosanskr", {{400, "NotoSansKR-Regular.otf"}, {700, "NotoSansKR-Bold.otf"}}},
        {"notosanssc", {{400, "NotoSansSC-Regular.otf"}, {700, "NotoSansSC-Bold.otf"}}},
        {"notosansthai", {{400, "NotoSansThai-Regular.ttf"}, {700, "NotoSansThai-Bold.ttf"}}},
        {"openbus", {{400, "OpenBus.ttf"}}},
        {"openbushud", {{400, "OpenBusHud.ttf"}}},
        {"poppins-semibold", {{400, "Poppins-SemiBold.ttf"}}},
        {"rmp-10", {{400, "FBW-Display-RMP-10.ttf"}}},
        {"rmp-11", {{400, "FBW-Display-RMP-11.ttf"}}},
        {"rmp-13", {{400, "FBW-Display-RMP-13.ttf"}}},
        {"rmp-16", {{400, "FBW-Display-RMP-16.ttf"}}},
        {"rmp-19", {{400, "FBW-Display-RMP-19.ttf"}}},
        {"sndfont", {{400, "ISISFontTemporary.ttf"}}},
        {"isisfont", {{400, "ISISFontTemporary.ttf"}}},
        {"a380x-hud", {{400, "A380X-HUD.ttf"}}},
    };
    return t;
}

class Library {
public:
    static Library& get() {
        thread_local Library lib;
        return lib;
    }

    sk_sp<SkTypeface> lookup(std::string_view css_family, int weight) {
        std::lock_guard<std::mutex> lock(mu_);
        std::string key = std::string(css_family) + "|" + std::to_string(weight);
        auto c = cache_.find(key);
        if (c != cache_.end()) return c->second;
        sk_sp<SkTypeface> tf = resolve(css_family, weight);
        cache_[key] = tf;
        return tf;
    }

    std::string key_of(const SkTypeface* tf) {
        if (!tf) return {};
        std::lock_guard<std::mutex> lock(mu_);
        auto it = ids_.find(tf->uniqueID());
        return it == ids_.end() ? std::string() : it->second;
    }

private:
    Library() {
        mgr_ = platform::font_manager();
        fs::path dir = assets::fonts_dir();
        std::error_code ec;
        if (fs::exists(dir, ec)) {
            for (fs::recursive_directory_iterator it(dir, fs::directory_options::skip_permission_denied, ec), end;
                 !ec && it != end; it.increment(ec)) {
                if (!it->is_regular_file(ec)) continue;
                auto ext = lower(it->path().extension().string());
                if (ext != ".ttf" && ext != ".otf") continue;
                files_.emplace(lower(it->path().filename().string()), it->path().string());
                stems_.emplace(lower(it->path().stem().string()), it->path().string());
            }
        }
    }

    sk_sp<SkTypeface> load(const std::string& path) {
        auto it = loaded_.find(path);
        if (it != loaded_.end()) return it->second;
        sk_sp<SkTypeface> tf;
        if (mgr_) tf = mgr_->makeFromFile(path.c_str());
        loaded_[path] = tf;
        if (tf) ids_[tf->uniqueID()] = lower(fs::path(path).filename().string());
        return tf;
    }

    static bool is_trim(char c) { return std::isspace((unsigned char)c) || c == '"' || c == '\''; }

    static std::vector<std::string> split_families(std::string_view css) {
        std::vector<std::string> out;
        std::string cur;
        auto flush = [&] {
            size_t a = 0, b = cur.size();
            while (a < b && is_trim(cur[a])) a++;
            while (b > a && is_trim(cur[b - 1])) b--;
            if (b > a) out.push_back(cur.substr(a, b - a));
            cur.clear();
        };
        for (char ch : css) {
            if (ch == ',') flush();
            else cur += ch;
        }
        flush();
        return out;
    }

    sk_sp<SkTypeface> from_table(const std::string& fam, int want) {
        auto& t = family_table();
        auto f = t.find(lower(fam));
        if (f == t.end()) return nullptr;
        const Face* best = nullptr;
        for (auto& face : f->second)
            if (!best || std::abs(face.weight - want) < std::abs(best->weight - want)) best = &face;
        if (!best) return nullptr;
        auto p = files_.find(lower(best->file));
        if (p == files_.end()) return nullptr;
        return load(p->second);
    }

    sk_sp<SkTypeface> resolve(std::string_view css, int weight) {
        for (auto& fam : split_families(css)) {
            if (auto tf = from_table(fam, weight)) return tf;
            auto s = stems_.find(lower(fam));
            if (s != stems_.end())
                if (auto tf = load(s->second)) return tf;
        }
        if (!mgr_) return nullptr;
        SkFontStyle style(weight, SkFontStyle::kNormal_Width, SkFontStyle::kUpright_Slant);
        for (auto& fam : split_families(css)) {
            std::string name = fam;
            std::string l = lower(fam);
            if (l == "monospace" || l == "ui-monospace") name = "Consolas";
            else if (l == "sans-serif" || l == "system-ui") name = "Arial";
            else if (l == "serif") name = "Times New Roman";
            if (auto tf = mgr_->matchFamilyStyle(name.c_str(), style)) return tf;
        }
        if (auto tf = mgr_->matchFamilyStyle("Arial", style)) return tf;
        return mgr_->legacyMakeTypeface(nullptr, style);
    }

    sk_sp<SkFontMgr> mgr_;
    std::map<std::string, std::string> files_;
    std::map<std::string, std::string> stems_;
    std::map<std::string, sk_sp<SkTypeface>> loaded_;
    std::map<uint32_t, std::string> ids_;
    std::map<std::string, sk_sp<SkTypeface>> cache_;
    std::mutex mu_;
};

}  // namespace

std::string typeface_key(const SkTypeface* tf) { return Library::get().key_of(tf); }

sk_sp<SkTypeface> typeface(std::string_view css_family, bool bold) { return Library::get().lookup(css_family, bold ? 700 : 400); }

sk_sp<SkTypeface> typeface(std::string_view css_family, int weight) { return Library::get().lookup(css_family, weight); }

SkFont font(std::string_view css_family, float px, bool bold, bool lcd) {
    return font(css_family, px, bold ? 700 : 400, lcd ? TextMode::Lcd : TextMode::Greyscale);
}

SkFont font(std::string_view css_family, float px, int weight, TextMode mode) {
    bool lcd = mode == TextMode::Lcd;
    SkFont f(typeface(css_family, weight), px);
    f.setSubpixel(true);
    f.setEdging(lcd ? SkFont::Edging::kSubpixelAntiAlias : SkFont::Edging::kAntiAlias);
    static const SkFontHinting hint = [] {
        const char* v = a380::getenv_cached("A380_TEXT_HINT");
        if (!v) return SkFontHinting::kNone;
        std::string k = lower(v);
        return k == "slight" ? SkFontHinting::kSlight : k == "normal" ? SkFontHinting::kNormal : k == "full" ? SkFontHinting::kFull : SkFontHinting::kNone;
    }();
    f.setHinting(hint);
    f.setLinearMetrics(true);
    if (const char* b = a380::getenv_cached("A380_TEXT_BSNAP")) f.setBaselineSnap(std::atoi(b) != 0);
    return f;
}

TextMode chromium_text_mode(bool layer_opaque, bool axis_aligned, bool integer_translate) {
    static const int force = [] {
        const char* v = a380::getenv_cached("A380_TEXT_LCD");
        return v ? std::atoi(v) : -1;
    }();
    if (force >= 0) return force ? TextMode::Lcd : TextMode::Greyscale;
    return layer_opaque && axis_aligned && integer_translate ? TextMode::Lcd : TextMode::Greyscale;
}

TextMode chromium_text_mode(const TextLayerFacts& f) {
    bool opaque = f.opaque_background && f.opacity >= 1.f && !f.filter_or_blend && !f.will_change_non_opaque;
    return chromium_text_mode(opaque, !f.non_translate_transform, !f.fractional_translate);
}

SkFont font(std::string_view css_family, float px, bool bold, TextMode mode) {
    return font(css_family, px, bold ? 700 : 400, mode);
}

}  // namespace a380
