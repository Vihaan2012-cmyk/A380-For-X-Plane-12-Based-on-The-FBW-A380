#include "a380/inputs.h"

#include <algorithm>
#include <cctype>
#include <fstream>
#include <sstream>
#include <stdexcept>

namespace a380 {

namespace {

std::string lower(std::string_view s) {
    std::string r(s);
    std::transform(r.begin(), r.end(), r.begin(), [](unsigned char c) { return (char)std::tolower(c); });
    return r;
}

std::string var_key(std::string_view name, std::string_view unit) {
    std::string k(name);
    k += '|';
    k += lower(unit);
    return k;
}

// var_key into a reused per-thread buffer: the per-frame lookups allocate nothing.
const std::string& scratch_key(std::string_view name, std::string_view unit) {
    thread_local std::string k;
    k.assign(name);
    k += '|';
    for (unsigned char c : unit) k += (char)std::tolower(c);
    return k;
}

const std::string& scratch_name(std::string_view name) {
    thread_local std::string k;
    k.assign(name);
    return k;
}

nlohmann::json parse_maybe(const nlohmann::json& v) {
    if (v.is_string()) {
        auto parsed = nlohmann::json::parse(v.get_ref<const std::string&>(), nullptr, false);
        if (!parsed.is_discarded()) return parsed;
    }
    return v;
}

}  // namespace

Inputs Inputs::from_state_file(const std::string& path) {
    std::ifstream f(path, std::ios::binary);
    if (!f) throw std::runtime_error("cannot open " + path);
    std::stringstream ss;
    ss << f.rdbuf();
    auto j = nlohmann::json::parse(ss.str(), nullptr, false);
    if (j.is_discarded()) throw std::runtime_error("cannot parse " + path);
    return from_state_json(j);
}

Inputs Inputs::from_state_json(const nlohmann::json& s) {
    Inputs in;
    if (auto it = s.find("vars"); it != s.end() && it->is_object()) {
        for (auto& [k, v] : it->items()) {
            if (!v.is_number() && !v.is_boolean()) continue;
            auto bar = k.rfind('|');
            std::string name = bar == std::string::npos ? k : k.substr(0, bar);
            std::string unit = bar == std::string::npos ? std::string() : k.substr(bar + 1);
            in.vars_[var_key(name, unit)] = v.is_boolean() ? (v.get<bool>() ? 1.0 : 0.0) : v.get<double>();
            in.unit_of_.emplace(name, unit);
        }
    }
    if (auto it = s.find("strings"); it != s.end() && it->is_object()) {
        for (auto& [k, v] : it->items())
            if (v.is_string()) in.strings_[k] = v.get<std::string>();
    }
    if (auto it = s.find("bus"); it != s.end() && it->is_object()) {
        for (auto& [k, v] : it->items()) {
            if (v.is_object() && v.contains("data")) in.bus_[k] = v["data"];
            else in.bus_[k] = v;
        }
    }
    if (auto it = s.find("genericData"); it != s.end() && it->is_object()) {
        for (auto& [k, v] : it->items()) {
            if (v.is_object() && v.contains("data")) in.generic_[k] = parse_maybe(v["data"]);
            else in.generic_[k] = parse_maybe(v);
        }
    }
    if (auto it = s.find("fetched"); it != s.end() && it->is_object()) {
        for (auto& [k, v] : it->items()) {
            if (v.is_object() && v.contains("body")) {
                const auto& b = v["body"];
                in.fetched_[k] = b.is_string() ? b.get<std::string>() : b.dump();
            }
        }
    }
    if (auto it = s.find("calls"); it != s.end() && it->is_array()) in.calls_ = *it;
    if (auto it = s.find("env"); it != s.end() && it->is_object()) {
        if (it->contains("dpr") && (*it)["dpr"].is_number()) in.dpr_ = (*it)["dpr"].get<double>();
        in.env_ = *it;
    }
    if (auto it = s.find("frozenAt"); it != s.end() && it->is_object()) in.frozen_ = *it;
    if (auto it = s.find("animations"); it != s.end() && it->is_array()) {
        for (const auto& a : *it) {
            if (!a.is_object() || !a.contains("name") || !a["name"].is_string() || !a.contains("time") || !a["time"].is_number()) continue;
            std::string name = "A380_ANIM:" + a["name"].get<std::string>();
            if (!in.has_var(name)) in.set_var(name, "", a["time"].get<double>());
        }
    }
    bool got = false;
    auto st = in.vars_.find(var_key("E:SIMULATION TIME", "seconds"));
    if (st != in.vars_.end()) {
        in.now_ms_ = st->second * 1000.0;
        got = true;
    }
    if (!got) {
        if (auto it = s.find("frozenAt"); it != s.end() && it->is_object() && it->contains("perf") && (*it)["perf"].is_number())
            in.now_ms_ = (*it)["perf"].get<double>();
    }
    return in;
}

double Inputs::var(std::string_view name, std::string_view unit, double fallback) const {
    if (unit.empty()) {
        auto u = unit_of_.find(scratch_name(name));
        if (u == unit_of_.end()) return fallback;
        auto v = vars_.find(scratch_key(name, u->second));
        return v == vars_.end() ? fallback : v->second;
    }
    auto v = vars_.find(scratch_key(name, unit));
    if (v != vars_.end()) return v->second;
    auto u = unit_of_.find(scratch_name(name));
    if (u != unit_of_.end()) {
        auto w = vars_.find(scratch_key(name, u->second));
        if (w != vars_.end()) return w->second;
    }
    return fallback;
}

bool Inputs::has_var(std::string_view name, std::string_view unit) const {
    if (unit.empty()) return unit_of_.count(scratch_name(name)) > 0;
    return vars_.count(scratch_key(name, unit)) > 0;
}

std::string Inputs::str(std::string_view name) const {
    auto it = strings_.find(scratch_name(name));
    return it == strings_.end() ? std::string() : it->second;
}

const nlohmann::json* Inputs::bus(std::string_view topic) const {
    auto it = bus_.find(scratch_name(topic));
    return it == bus_.end() ? nullptr : &it->second;
}

const nlohmann::json* Inputs::generic(std::string_view key) const {
    auto it = generic_.find(scratch_name(key));
    return it == generic_.end() ? nullptr : &it->second;
}

const std::string* Inputs::fetched(std::string_view url_part) const {
    for (auto& [k, v] : fetched_)
        if (k.find(url_part) != std::string::npos) return &v;
    return nullptr;
}

const nlohmann::json* Inputs::call_result(std::string_view name) const {
    for (auto& c : calls_) {
        if (!c.is_object()) continue;
        auto n = c.find("name");
        if (n == c.end() || !n->is_string() || n->get<std::string>() != name) continue;
        auto r = c.find("result");
        if (r == c.end()) return nullptr;
        if (r->is_string()) {
            thread_local nlohmann::json holder;
            auto p = nlohmann::json::parse(r->get_ref<const std::string&>(), nullptr, false);
            holder = p.is_discarded() ? *r : std::move(p);
            return &holder;
        }
        return &*r;
    }
    return nullptr;
}

bool Inputs::has_env_number(std::string_view path) const {
    const nlohmann::json* cur = path.rfind("frozenAt.", 0) == 0 ? &frozen_ : &env_;
    if (path.rfind("frozenAt.", 0) == 0) path.remove_prefix(9);
    while (!path.empty()) {
        auto dot = path.find('.');
        std::string key(path.substr(0, dot));
        if (!cur->is_object()) return false;
        auto it = cur->find(key);
        if (it == cur->end()) return false;
        cur = &*it;
        if (dot == std::string_view::npos) break;
        path.remove_prefix(dot + 1);
    }
    return cur->is_number();
}

double Inputs::env_number(std::string_view path, double fallback) const {
    if (!has_env_number(path)) return fallback;
    const nlohmann::json* cur = path.rfind("frozenAt.", 0) == 0 ? &frozen_ : &env_;
    if (path.rfind("frozenAt.", 0) == 0) path.remove_prefix(9);
    while (!path.empty()) {
        auto dot = path.find('.');
        cur = &cur->at(std::string(path.substr(0, dot)));
        if (dot == std::string_view::npos) break;
        path.remove_prefix(dot + 1);
    }
    return cur->get<double>();
}

double Inputs::now_ms() const { return now_ms_; }

void Inputs::set_var(std::string_view name, std::string_view unit, double value) {
    vars_[var_key(name, unit)] = value;
    unit_of_.emplace(std::string(name), std::string(unit));
}

void Inputs::set_bus(std::string_view topic, nlohmann::json data) { bus_[std::string(topic)] = std::move(data); }

void Inputs::set_string(std::string_view name, std::string_view value) { strings_[std::string(name)] = std::string(value); }

void Inputs::set_generic(std::string_view key, nlohmann::json data) { generic_[std::string(key)] = std::move(data); }

void Inputs::set_fetched(std::string_view url, std::string body) { fetched_[std::string(url)] = std::move(body); }

void Inputs::set_env_number(std::string_view path, double value) {
    nlohmann::json* cur = &env_;
    if (path.rfind("frozenAt.", 0) == 0) {
        cur = &frozen_;
        path.remove_prefix(9);
    }
    while (!path.empty()) {
        auto dot = path.find('.');
        std::string key(path.substr(0, dot));
        if (!cur->is_object()) *cur = nlohmann::json::object();
        if (dot == std::string_view::npos) {
            (*cur)[key] = value;
            return;
        }
        cur = &(*cur)[key];
        path.remove_prefix(dot + 1);
    }
}

}  // namespace a380
