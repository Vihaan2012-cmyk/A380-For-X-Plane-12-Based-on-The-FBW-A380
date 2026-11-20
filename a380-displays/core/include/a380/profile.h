#pragma once

#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <map>
#include <string>

namespace a380 {

void profile_emit(const std::string& line);

std::string take_profile();

/* Per-section CPU time of a display's draw: every 1200 frames into the host's diagnostics
   (a380d_host_diag), and every 240 to stderr as well when A380_PROFILE is set. A380_PROFILE_OFF stops it. */
class SectionProfile {
public:
    explicit SectionProfile(std::string name)
        : name_(std::move(name)), on_(std::getenv("A380_PROFILE_OFF") == nullptr), loud_(std::getenv("A380_PROFILE") != nullptr) {
        if (const char* s = std::getenv("A380_PROFILE_SKIP")) skip_ = std::string(",") + s + ",";
    }

    template <class F>
    void time(const char* section, F&& f) {
        if (!on_) {
            f();
            return;
        }
        if (!skip_.empty() && skip_.find(std::string(",") + section + ",") != std::string::npos) return;
        auto t0 = std::chrono::steady_clock::now();
        f();
        sum_[section] += std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - t0).count();
    }

    void frame() {
        if (!on_ || ++frames_ < (loud_ ? 240 : 1200)) return;
        double total = 0;
        for (auto& [k, v] : sum_) total += v;
        std::string line = name_ + " per frame: total " + fmt(total / frames_) + " ms |";
        for (auto& [k, v] : sum_) line += std::string(" ") + k + " " + fmt(v / frames_);
        if (loud_) std::fprintf(stderr, "%s\n", line.c_str());
        profile_emit(line);
        sum_.clear();
        frames_ = 0;
    }

private:
    static std::string fmt(double v) {
        char b[32];
        std::snprintf(b, sizeof b, "%.3f", v);
        return b;
    }

    std::string name_;
    bool on_;
    bool loud_;
    int frames_ = 0;
    std::map<const char*, double> sum_;
    std::string skip_;
};

}  // namespace a380
