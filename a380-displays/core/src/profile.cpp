#include "a380/profile.h"

#include <mutex>

namespace a380 {

namespace {
std::mutex g_mutex;
std::string g_lines;
}  // namespace

void profile_emit(const std::string& line) {
    std::lock_guard<std::mutex> lock(g_mutex);
    if (g_lines.size() < 64 * 1024) g_lines += line + "\n";
}

std::string take_profile() {
    std::lock_guard<std::mutex> lock(g_mutex);
    std::string out;
    out.swap(g_lines);
    return out;
}

}  // namespace a380
