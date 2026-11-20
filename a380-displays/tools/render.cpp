#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <algorithm>
#include <cctype>
#include <chrono>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <string>
#include <vector>

#include "include/codec/SkCodec.h"
#include "include/core/SkBitmap.h"
#include "include/core/SkCanvas.h"
#include "include/core/SkData.h"
#include "include/core/SkImage.h"
#include "include/core/SkPixmap.h"
#include "include/core/SkSamplingOptions.h"
#include "include/core/SkStream.h"
#include "include/core/SkSurface.h"
#include "include/encode/SkPngEncoder.h"

#include "a380/display.h"
#include "a380/draw.h"
#include "a380/inputs.h"

#include "a380/gpu.h"
#include "sampler.h"

static int render_one(int argc, char** argv);
static int bench_main(int argc, char** argv);

int main(int argc, char** argv) {
    if (argc == 2 && !std::strcmp(argv[1], "--list")) {
        for (auto& d : a380::display_ids()) std::printf("%s\n", d.c_str());
        return 0;
    }
    if (argc >= 2 && (!std::strcmp(argv[1], "--bench") || !std::strcmp(argv[1], "--bench-all"))) return bench_main(argc, argv);
    if (argc == 3 && !std::strcmp(argv[1], "--batch")) {
        std::ifstream list(argv[2]);
        std::string line;
        while (std::getline(list, line)) {
            if (!line.empty() && line.back() == '\r') line.pop_back();
            if (line.empty()) continue;
            std::vector<std::string> parts{"a380disp_render"};
            size_t start = 0;
            while (start <= line.size()) {
                size_t tab = line.find('\t', start);
                if (tab == std::string::npos) tab = line.size();
                parts.push_back(line.substr(start, tab - start));
                start = tab + 1;
            }
            std::vector<char*> args;
            for (auto& p : parts) args.push_back(p.data());
            int rc = render_one((int)args.size(), args.data());
            std::printf("%d\t%s\n", rc, parts.size() > 4 ? parts[4].c_str() : "");
            std::fflush(stdout);
        }
        return 0;
    }
    return render_one(argc, argv);
}

static int render_one(int argc, char** argv) {
    if (argc < 5) {
        std::fprintf(stderr, "usage: a380disp_render <id> <variant> <state.json> <out.png> [--layer <layer.png>] [--model <model.json>]\n");
        return 2;
    }
    std::string id = argv[1], variant = argv[2], state = argv[3], out = argv[4];
    auto ends_with = [](const std::string& s, const char* suffix) {
        size_t n = std::strlen(suffix);
        if (s.size() < n) return false;
        return std::equal(s.end() - n, s.end(), suffix, [](char a, char b) { return std::tolower((unsigned char)a) == b; });
    };
    if (!ends_with(out, ".png") || ends_with(state, ".png")) {
        std::fprintf(stderr, "refusing to render: output must be a .png and the state must not be (%s -> %s)\n", state.c_str(), out.c_str());
        return 2;
    }
    std::string layer_path, model_path;
    for (int i = 5; i + 1 < argc; i++) {
        if (!std::strcmp(argv[i], "--layer")) layer_path = argv[++i];
        else if (!std::strcmp(argv[i], "--model")) model_path = argv[++i];
    }
    if (variant == "-" || variant == "none") variant.clear();

    try {
        a380::Inputs in = a380::Inputs::from_state_file(state);
        if (!model_path.empty()) {
            std::ifstream f(model_path, std::ios::binary);
            auto j = nlohmann::json::parse(f, nullptr, false);
            if (j.is_discarded()) {
                std::fprintf(stderr, "cannot parse model %s\n", model_path.c_str());
                return 1;
            }
            in.set_bus("a380.mfd.model", std::move(j));
        }
        auto display = a380::create_display(id, variant);
        if (!display) {
            std::fprintf(stderr, "no display registered as '%s'. registered:", id.c_str());
            for (auto& d : a380::display_ids()) std::fprintf(stderr, " %s", d.c_str());
            std::fprintf(stderr, "\n");
            return 3;
        }
        double dpr = in.device_pixel_ratio();
        SkISize sz = display->size();
        int w = (int)std::lround(sz.width() * dpr), h = (int)std::lround(sz.height() * dpr);
        SkImageInfo info = SkImageInfo::MakeN32Premul(w, h);
        auto env_num = [](const char* k, double d) {
            const char* v = std::getenv(k);
            return v ? std::atof(v) : d;
        };
        SkSurfaceProps props = a380::text_surface_props((float)env_num("A380_TEXT_CONTRAST", 1.0),
                                                       (float)env_num("A380_TEXT_GAMMA", 2.2));
        sk_sp<SkSurface> surface = a380::make_surface(info, &props);
        SkCanvas* canvas = surface->getCanvas();
        canvas->clear(SK_ColorBLACK);
        if (!layer_path.empty()) {
            auto data = SkData::MakeFromFileName(layer_path.c_str());
            std::unique_ptr<SkCodec> codec = data ? SkCodec::MakeFromData(data) : nullptr;
            if (codec) {
                auto [image, result] = codec->getImage();
                if (image) canvas->drawImageRect(image, SkRect::MakeWH((float)w, (float)h), SkSamplingOptions(SkFilterMode::kLinear));
            } else {
                std::fprintf(stderr, "warning: cannot decode layer %s\n", layer_path.c_str());
            }
        }
        canvas->save();
        canvas->scale((float)dpr, (float)dpr);
        display->draw(*canvas, in);
        canvas->restore();

        SkBitmap bm;
        bm.allocPixels(info);
        if (!surface->readPixels(bm, 0, 0)) return 4;
        SkPixmap pm;
        bm.peekPixels(&pm);
        SkPixmap opaque(info.makeAlphaType(kOpaque_SkAlphaType), pm.addr(), pm.rowBytes());
        SkFILEWStream stream(out.c_str());
        if (!stream.isValid()) {
            std::fprintf(stderr, "cannot write %s\n", out.c_str());
            return 5;
        }
        SkPngEncoder::Options opts;
        opts.fZLibLevel = 3;
        if (!SkPngEncoder::Encode(&stream, opaque, opts)) return 6;
        stream.flush();
    } catch (const std::exception& e) {
        std::fprintf(stderr, "error: %s\n", e.what());
        return 1;
    }
    return 0;
}

namespace {

struct BenchResult {
    bool ok = false;
    double build_ms = 0, first_ms = 0, mean_ms = 0, p95_ms = 0, sync_mean_ms = 0, sync_p95_ms = 0, record_ms = 0;
};

using BenchClock = std::chrono::steady_clock;

double ms_since(BenchClock::time_point t) {
    return std::chrono::duration<double, std::milli>(BenchClock::now() - t).count();
}

double percentile(std::vector<double> v, double q) {
    if (v.empty()) return 0;
    std::sort(v.begin(), v.end());
    size_t i = (size_t)std::ceil(q * (double)v.size()) - 1;
    return v[std::min(i, v.size() - 1)];
}

double mean_of(const std::vector<double>& v) {
    double s = 0;
    for (double x : v) s += x;
    return v.empty() ? 0 : s / (double)v.size();
}

BenchResult bench_one(const std::string& id, std::string variant, const std::string& state, const std::string& layer_path,
                      const std::string& model_path, int frames, bool sync) {
    BenchResult r;
    if (variant == "-" || variant == "none") variant.clear();
    a380::Inputs in = a380::Inputs::from_state_file(state);
    if (!model_path.empty()) {
        std::ifstream f(model_path, std::ios::binary);
        auto j = nlohmann::json::parse(f, nullptr, false);
        if (!j.is_discarded()) in.set_bus("a380.mfd.model", std::move(j));
    }
    auto t_build = BenchClock::now();
    auto display = a380::create_display(id, variant);
    if (!display) return r;
    r.build_ms = ms_since(t_build);
    double dpr = in.device_pixel_ratio();
    SkISize sz = display->size();
    int w = (int)std::lround(sz.width() * dpr), h = (int)std::lround(sz.height() * dpr);
    SkImageInfo info = SkImageInfo::MakeN32Premul(w, h);
    SkSurfaceProps props = a380::text_surface_props(1.0f, 2.2f);
    sk_sp<SkSurface> surface = a380::make_surface(info, &props);
    SkCanvas* canvas = surface->getCanvas();
    sk_sp<SkImage> layer;
    if (!layer_path.empty()) {
        auto data = SkData::MakeFromFileName(layer_path.c_str());
        std::unique_ptr<SkCodec> codec = data ? SkCodec::MakeFromData(data) : nullptr;
        if (codec) layer = std::get<0>(codec->getImage());
    }
    std::vector<std::pair<std::string, double>> packed;
    if (std::getenv("A380_BENCH_VARY")) {
        std::ifstream sf(state, std::ios::binary);
        auto sj = nlohmann::json::parse(sf, nullptr, false);
        if (sj.is_object() && sj.contains("vars") && sj["vars"].is_object())
            for (auto& [k, v] : sj["vars"].items())
                if (v.is_number() && v.get<double>() > 4294967296.0) packed.emplace_back(k, v.get<double>());
    }
    std::vector<double> cpu, wall, record;
    const bool sample = std::getenv("A380_SAMPLE") != nullptr;
    if (sample) sampler_start();
    for (int i = 0; i < frames; i++) {
        for (auto& [k, raw] : packed) {
            uint64_t bits = (uint64_t)raw;
            float f;
            uint32_t lo = (uint32_t)(bits & 0xffffffffull);
            std::memcpy(&f, &lo, 4);
            f += 0.37f * (float)(i % 50);
            std::memcpy(&lo, &f, 4);
            size_t bar = k.find('|');
            in.set_var(k.substr(0, bar), bar == std::string::npos ? "number" : k.substr(bar + 1), (double)((bits & ~0xffffffffull) | lo));
        }
        in.set_now_ms(1000.0 * i / 30.0);
        auto t0 = BenchClock::now();
        canvas->clear(SK_ColorBLACK);
        if (layer) canvas->drawImageRect(layer, SkRect::MakeWH((float)w, (float)h), SkSamplingOptions(SkFilterMode::kLinear));
        canvas->save();
        canvas->scale((float)dpr, (float)dpr);
        display->draw(*canvas, in);
        canvas->restore();
        record.push_back(ms_since(t0));
        a380::flush_surface(surface.get(), false);
        double c = ms_since(t0);
        if (sync) a380::flush_surface(surface.get(), true);
        double t = ms_since(t0);
        if (i == 0) r.first_ms = sync ? t : c;
        cpu.push_back(c);
        wall.push_back(t);
    }
    if (sample) sampler_report(stderr, std::atoi(std::getenv("A380_SAMPLE")) > 1 ? std::atoi(std::getenv("A380_SAMPLE")) : 40);
    if (frames > 1) {
        cpu.erase(cpu.begin());
        wall.erase(wall.begin());
        record.erase(record.begin());
    }
    r.record_ms = mean_of(record);
    r.mean_ms = mean_of(cpu);
    r.p95_ms = percentile(cpu, 0.95);
    r.sync_mean_ms = mean_of(wall);
    r.sync_p95_ms = percentile(wall, 0.95);
    r.ok = true;
    return r;
}

void print_bench_header(bool sync) {
    std::printf("%-14s %9s %9s %9s %9s %9s", "display", "build_ms", "first_ms", "record_ms", "mean_ms", "p95_ms");
    if (sync) std::printf(" %10s %10s", "sync_mean", "sync_p95");
    std::printf("\n");
}

void print_bench_row(const std::string& name, const BenchResult& r, bool sync) {
    if (!r.ok) {
        std::printf("%-14s failed\n", name.c_str());
        return;
    }
    std::printf("%-14s %9.2f %9.2f %9.3f %9.3f %9.3f", name.c_str(), r.build_ms, r.first_ms, r.record_ms, r.mean_ms, r.p95_ms);
    if (sync) std::printf(" %10.3f %10.3f", r.sync_mean_ms, r.sync_p95_ms);
    std::printf("\n");
    std::fflush(stdout);
}

}  // namespace

static int bench_main(int argc, char** argv) {
    bool sync = false;
    std::vector<std::string> pos;
    for (int i = 2; i < argc; i++) {
        if (!std::strcmp(argv[i], "--sync")) sync = true;
        else pos.push_back(argv[i]);
    }
    try {
        if (!std::strcmp(argv[1], "--bench")) {
            if (pos.size() < 4) {
                std::fprintf(stderr, "usage: a380disp_render --bench <id> <variant> <state.json> <frames> [--sync] [--layer l.png] [--model m.json]\n");
                return 2;
            }
            std::string layer, model;
            std::vector<std::string> rest(pos.begin() + 4, pos.end());
            for (size_t i = 0; i + 1 < rest.size(); i++) {
                if (rest[i] == "--layer") layer = rest[++i];
                else if (rest[i] == "--model") model = rest[++i];
            }
            BenchResult r = bench_one(pos[0], pos[1], pos[2], layer, model, std::max(1, std::atoi(pos[3].c_str())), sync);
            print_bench_header(sync);
            print_bench_row(pos[0] + (pos[1] == "-" ? "" : "-" + pos[1]), r, sync);
            return r.ok ? 0 : 3;
        }
        if (pos.empty()) {
            std::fprintf(stderr, "usage: a380disp_render --bench-all <capture dir> [frames] [--sync]\n");
            return 2;
        }
        namespace fs = std::filesystem;
        int frames = pos.size() > 1 ? std::max(1, std::atoi(pos[1].c_str())) : 120;
        std::vector<std::string> views;
        for (auto& e : fs::directory_iterator(pos[0])) {
            std::string n = e.path().filename().string();
            const std::string suffix = ".state.json";
            if (n.size() > suffix.size() && n.compare(n.size() - suffix.size(), suffix.size(), suffix) == 0)
                views.push_back(n.substr(0, n.size() - suffix.size()));
        }
        std::sort(views.begin(), views.end());
        print_bench_header(sync);
        double total_mean = 0, total_p95 = 0;
        for (auto& view : views) {
            std::string id = view, variant = "-";
            size_t dash = view.rfind('-');
            if (dash != std::string::npos && dash + 1 < view.size() && std::isdigit((unsigned char)view[dash + 1])) {
                id = view.substr(0, dash);
                variant = view.substr(dash + 1);
            }
            auto have = a380::display_ids();
            if (std::find(have.begin(), have.end(), id) == have.end()) continue;
            fs::path base = fs::path(pos[0]) / view;
            std::string layer = base.string() + ".layer.png", model = base.string() + ".model.json";
            if (!fs::exists(layer)) layer.clear();
            if (!fs::exists(model)) model.clear();
            BenchResult r = bench_one(id, variant, base.string() + ".state.json", layer, model, frames, sync);
            print_bench_row(view, r, sync);
            if (r.ok) {
                total_mean += r.mean_ms;
                total_p95 += r.p95_ms;
            }
        }
        std::printf("%-14s %9s %9s %9.3f %9.3f\n", "TOTAL", "", "", total_mean, total_p95);
    } catch (const std::exception& e) {
        std::fprintf(stderr, "error: %s\n", e.what());
        return 1;
    }
    return 0;
}
