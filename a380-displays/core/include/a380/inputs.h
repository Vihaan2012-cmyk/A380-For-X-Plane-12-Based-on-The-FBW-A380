#pragma once

#include <string>
#include <string_view>
#include <unordered_map>

#include "nlohmann/json.hpp"

namespace a380 {

// Everything a display reads, named exactly as FlyByWire's page reads it.
// Offline it comes from a golden's <view>.state.json; in X-Plane the plugin
// fills it from datarefs and the systems plugin's bus each frame.
class Inputs {
public:
    static Inputs from_state_file(const std::string& path);
    static Inputs from_state_json(const nlohmann::json& state);

    // A simulator or local variable as FlyByWire names it, e.g.
    // "L:A32NX_EFIS_L_ND_MODE" or "PLANE HEADING DEGREES TRUE". With an empty
    // unit, the first recorded unit for that name is used.
    double var(std::string_view name, std::string_view unit = {}, double fallback = 0.0) const;
    bool has_var(std::string_view name, std::string_view unit = {}) const;

    std::string str(std::string_view name) const;

    // The latest event published on the shared bus under `topic`
    // (state.json "bus"), or nullptr.
    const nlohmann::json* bus(std::string_view topic) const;

    // The latest generic-data payload sent under `key` (state.json
    // "genericData"), parsed when it is JSON text, or nullptr.
    const nlohmann::json* generic(std::string_view key) const;

    // A recorded network response body by URL substring (state.json
    // "fetched"), or nullptr.
    const std::string* fetched(std::string_view url_part) const;

    // A recorded Coherent call result by call name, first match (state.json
    // "calls"), or nullptr.
    const nlohmann::json* call_result(std::string_view name) const;

    // Simulation time in milliseconds when the picture was taken.
    double now_ms() const;

    double device_pixel_ratio() const { return dpr_; }

    // A number from the recording environment by dotted path, e.g.
    // "connectedAt.perf" (env) or "frozenAt.perf"; fallback when absent.
    double env_number(std::string_view path, double fallback = 0.0) const;
    bool has_env_number(std::string_view path) const;

    void set_var(std::string_view name, std::string_view unit, double value);
    void set_bus(std::string_view topic, nlohmann::json data);
    void set_string(std::string_view name, std::string_view value);
    void set_generic(std::string_view key, nlohmann::json data);
    void set_fetched(std::string_view url, std::string body);
    void set_env_number(std::string_view path, double value);
    void set_now_ms(double ms) { now_ms_ = ms; }
    void set_dpr(double dpr) { dpr_ = dpr; }

private:
    std::unordered_map<std::string, double> vars_;
    std::unordered_map<std::string, std::string> unit_of_;
    std::unordered_map<std::string, std::string> strings_;
    std::unordered_map<std::string, nlohmann::json> bus_;
    std::unordered_map<std::string, nlohmann::json> generic_;
    std::unordered_map<std::string, std::string> fetched_;
    nlohmann::json calls_ = nlohmann::json::array();
    nlohmann::json env_ = nlohmann::json::object();
    nlohmann::json frozen_ = nlohmann::json::object();
    double now_ms_ = 0.0;
    double dpr_ = 1.5;
};

}  // namespace a380
