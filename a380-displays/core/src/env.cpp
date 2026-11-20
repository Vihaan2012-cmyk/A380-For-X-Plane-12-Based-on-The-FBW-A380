#include "a380/env.h"

#include <cstdlib>
#include <optional>
#include <string>
#include <string_view>
#include <unordered_map>

namespace a380 {

namespace {

struct Hash {
    using is_transparent = void;
    size_t operator()(std::string_view s) const { return std::hash<std::string_view>{}(s); }
};

}  // namespace

const char* getenv_cached(const char* key) {
    if (!key) return nullptr;
    thread_local std::unordered_map<std::string, std::optional<std::string>, Hash, std::equal_to<>> cache;
    auto it = cache.find(std::string_view(key));
    if (it == cache.end()) {
        const char* v = std::getenv(key);
        it = cache.emplace(key, v ? std::optional<std::string>(v) : std::nullopt).first;
    }
    return it->second ? it->second->c_str() : nullptr;
}

}  // namespace a380
