#include <cctype>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <iterator>
#include <set>
#include <sstream>
#include <string>
#include <tuple>
#include <vector>

#include "include/codec/SkCodec.h"
#include "include/core/SkData.h"
#include "include/core/SkPixmap.h"
#include "include/core/SkStream.h"
#include "include/encode/SkPngEncoder.h"

#include "nlohmann/json.hpp"

#include "a380/c_api.h"

using json = nlohmann::json;

namespace {

bool read_file(const std::string& path, std::string& out) {
    std::ifstream f(path, std::ios::binary);
    if (!f) return false;
    std::stringstream ss;
    ss << f.rdbuf();
    out = ss.str();
    return true;
}

bool check(int rc, const char* what) {
    if (rc >= 0) return true;
    std::fprintf(stderr, "%s failed (%d): %s\n", what, rc, a380d_last_error());
    return false;
}

void flatten_numbers(const json& j, const std::string& prefix, std::vector<std::pair<std::string, double>>& out) {
    if (j.is_number()) {
        out.emplace_back(prefix, j.get<double>());
        return;
    }
    if (!j.is_object()) return;
    for (auto& [k, v] : j.items()) {
        if (k.find('.') != std::string::npos) continue;
        flatten_numbers(v, prefix.empty() ? k : prefix + "." + k, out);
    }
}

bool feed_state(a380d_inputs* in, const json& s) {
    std::set<std::string> named;
    if (auto it = s.find("vars"); it != s.end() && it->is_object()) {
        for (auto& [k, v] : it->items()) {
            if (!v.is_number() && !v.is_boolean()) continue;
            auto bar = k.rfind('|');
            std::string name = bar == std::string::npos ? k : k.substr(0, bar);
            std::string unit = bar == std::string::npos ? std::string() : k.substr(bar + 1);
            double value = v.is_boolean() ? (v.get<bool>() ? 1.0 : 0.0) : v.get<double>();
            if (!check(a380d_set_var(in, name.c_str(), unit.c_str(), value), "set_var")) return false;
            named.insert(name);
        }
    }
    if (auto it = s.find("strings"); it != s.end() && it->is_object()) {
        for (auto& [k, v] : it->items())
            if (v.is_string() && !check(a380d_set_string(in, k.c_str(), v.get_ref<const std::string&>().c_str()), "set_string")) return false;
    }
    if (auto it = s.find("bus"); it != s.end() && it->is_object()) {
        for (auto& [k, v] : it->items()) {
            const json& d = (v.is_object() && v.contains("data")) ? v["data"] : v;
            int rc = d.is_string() ? a380d_set_bus_string(in, k.c_str(), d.get_ref<const std::string&>().data(), d.get_ref<const std::string&>().size())
                                   : a380d_set_bus(in, k.c_str(), d.dump().c_str());
            if (!check(rc, "set_bus")) return false;
        }
    }
    if (auto it = s.find("genericData"); it != s.end() && it->is_object()) {
        for (auto& [k, v] : it->items()) {
            const json& d = (v.is_object() && v.contains("data")) ? v["data"] : v;
            std::string text = d.is_string() ? d.get<std::string>() : d.dump();
            if (!check(a380d_set_generic(in, k.c_str(), text.c_str()), "set_generic")) return false;
        }
    }
    if (auto it = s.find("fetched"); it != s.end() && it->is_object()) {
        for (auto& [k, v] : it->items()) {
            if (!v.is_object() || !v.contains("body")) continue;
            const json& b = v["body"];
            std::string body = b.is_string() ? b.get<std::string>() : b.dump();
            if (!check(a380d_set_fetched(in, k.c_str(), body.data(), body.size()), "set_fetched")) return false;
        }
    }
    if (auto it = s.find("env"); it != s.end() && it->is_object()) {
        if (it->contains("dpr") && (*it)["dpr"].is_number() && !check(a380d_set_dpr(in, (*it)["dpr"].get<double>()), "set_dpr")) return false;
        std::vector<std::pair<std::string, double>> nums;
        flatten_numbers(*it, "", nums);
        for (auto& [p, v] : nums)
            if (!check(a380d_set_env_number(in, p.c_str(), v), "set_env_number")) return false;
    }
    if (auto it = s.find("frozenAt"); it != s.end() && it->is_object()) {
        std::vector<std::pair<std::string, double>> nums;
        flatten_numbers(*it, "frozenAt", nums);
        for (auto& [p, v] : nums)
            if (!check(a380d_set_env_number(in, p.c_str(), v), "set_env_number")) return false;
    }
    if (auto it = s.find("animations"); it != s.end() && it->is_array()) {
        for (const auto& a : *it) {
            if (!a.is_object() || !a.contains("name") || !a["name"].is_string() || !a.contains("time") || !a["time"].is_number()) continue;
            std::string name = "A380_ANIM:" + a["name"].get<std::string>();
            if (named.count(name)) continue;
            if (!check(a380d_set_var(in, name.c_str(), "", a["time"].get<double>()), "set_var")) return false;
            named.insert(name);
        }
    }
    double now = 0.0;
    bool got = false;
    if (auto it = s.find("vars"); it != s.end() && it->is_object()) {
        for (auto& [k, v] : it->items()) {
            if (!v.is_number() && !v.is_boolean()) continue;
            auto bar = k.rfind('|');
            if (bar == std::string::npos || k.substr(0, bar) != "E:SIMULATION TIME") continue;
            std::string unit = k.substr(bar + 1);
            for (auto& c : unit) c = (char)std::tolower((unsigned char)c);
            if (unit != "seconds") continue;
            now = (v.is_boolean() ? (v.get<bool>() ? 1.0 : 0.0) : v.get<double>()) * 1000.0;
            got = true;
        }
    }
    if (!got) {
        if (auto it = s.find("frozenAt"); it != s.end() && it->is_object() && it->contains("perf") && (*it)["perf"].is_number())
            now = (*it)["perf"].get<double>();
    }
    return check(a380d_set_now(in, now), "set_now");
}

bool write_png(const std::string& path, const uint8_t* rgba, int w, int h, int stride) {
    SkImageInfo info = SkImageInfo::Make(w, h, kRGBA_8888_SkColorType, kOpaque_SkAlphaType);
    SkPixmap pm(info, rgba, (size_t)stride);
    if (path.size() < 4 || path.compare(path.size() - 4, 4, ".png") != 0) return false;
    SkFILEWStream stream(path.c_str());
    if (!stream.isValid()) return false;
    SkPngEncoder::Options opts;
    opts.fZLibLevel = 3;
    if (!SkPngEncoder::Encode(&stream, pm, opts)) return false;
    stream.flush();
    return true;
}

bool decode_png(const std::string& path, std::vector<uint8_t>& px, int& w, int& h) {
    auto data = SkData::MakeFromFileName(path.c_str());
    std::unique_ptr<SkCodec> codec = data ? SkCodec::MakeFromData(data) : nullptr;
    if (!codec) return false;
    SkImageInfo info = SkImageInfo::Make(codec->dimensions().width(), codec->dimensions().height(), kRGBA_8888_SkColorType, kUnpremul_SkAlphaType);
    w = info.width();
    h = info.height();
    px.assign((size_t)w * h * 4, 0);
    return codec->getPixels(info, px.data(), (size_t)w * 4) == SkCodec::kSuccess;
}

struct Layer {
    std::string id, variant, state_path;
    a380d_display* display = nullptr;
    a380d_inputs* inputs = nullptr;
};

bool parse_layer(const std::string& spec, Layer& l) {
    auto a = spec.find(':');
    auto b = spec.find(':', a == std::string::npos ? a : a + 1);
    if (a == std::string::npos || b == std::string::npos) return false;
    l.id = spec.substr(0, a);
    l.variant = spec.substr(a + 1, b - a - 1);
    if (l.variant == "-") l.variant.clear();
    l.state_path = spec.substr(b + 1);
    return true;
}

struct Click {
    double x, y;
};

std::vector<a380d_write> click(a380d_display* d, Click c, Click up, int* took) {
    *took = a380d_pointer(d, c.x, c.y, A380D_POINTER_DOWN, 0);
    a380d_pointer(d, up.x, up.y, A380D_POINTER_UP, 0);
    std::vector<a380d_write> w(16);
    int n = a380d_take_writes(d, w.data(), (int)w.size());
    w.resize(n < 0 ? 0 : n);
    return w;
}

bool draw_once(a380d_display* d, a380d_inputs* in) {
    int cw = 0, ch = 0;
    a380d_size(d, &cw, &ch);
    std::vector<uint8_t> px((size_t)cw * ch * 4);
    return a380d_draw(d, in, px.data(), cw, ch, cw * 4) >= 0;
}

int input_tests() {
    int failed = 0, passed = 0;
    auto expect = [&](bool ok, const std::string& what) {
        std::printf("%s %s\n", ok ? "ok  " : "FAIL", what.c_str());
        ok ? passed++ : failed++;
    };
    auto same = [](const std::vector<a380d_write>& w, std::vector<std::tuple<char, std::string, double, double>> want) {
        if (w.size() != want.size()) return false;
        for (size_t i = 0; i < w.size(); i++)
            if (w[i].kind != std::get<0>(want[i]) || std::get<1>(want[i]) != w[i].name || w[i].value != std::get<2>(want[i]) ||
                w[i].delay_ms != std::get<3>(want[i]))
                return false;
        return true;
    };
    if (a380d_gpu_init() < 0) return 1;
    expect(a380d_abi_version() == 3, "abi version 3");

    a380d_display* ewd = a380d_create("ewd", "");
    a380d_inputs* in = a380d_inputs_new();
    a380d_set_dpr(in, 1.0);
    a380d_set_bus(in, "fws_show_normal_checklists", "true");
    draw_once(ewd, in);
    int took = 0;
    auto w = click(ewd, {740, 880}, {740, 880}, &took);
    expect(took == 0 && w.empty(), "ewd: soft keys hidden while CONFIG_A380X_SHOW_ECL_SOFTKEYS is not 1");
    a380d_set_string(in, "CONFIG_A380X_SHOW_ECL_SOFTKEYS", "1");
    draw_once(ewd, in);
    w = click(ewd, {740, 880}, {740, 880}, &took);
    expect(took == 1 && same(w, {{'L', "L:A32NX_BTN_CHECK_LH", 1, 0}, {'L', "L:A32NX_BTN_CHECK_LH", 0, 50}}), "ewd: CHECK soft key");
    w = click(ewd, {740, 935}, {740, 935}, &took);
    expect(same(w, {{'L', "L:A32NX_BTN_UP", 1, 0}, {'L', "L:A32NX_BTN_UP", 0, 50}}), "ewd: UP soft key");
    w = click(ewd, {740, 990}, {740, 990}, &took);
    expect(same(w, {{'L', "L:A32NX_BTN_DOWN", 1, 0}, {'L', "L:A32NX_BTN_DOWN", 0, 50}}), "ewd: DOWN soft key");
    w = click(ewd, {740, 880}, {740, 990}, &took);
    expect(w.empty(), "ewd: press on CHECK, release on DOWN is no click");
    w = click(ewd, {400, 300}, {400, 300}, &took);
    expect(took == 0 && w.empty(), "ewd: engine area takes no click");
    expect(a380d_input_native(ewd) == 1, "ewd: input fully native");
    a380d_set_bus(in, "fws_show_normal_checklists", "false");
    draw_once(ewd, in);
    w = click(ewd, {740, 880}, {740, 880}, &took);
    expect(w.empty(), "ewd: no checklist shown, no soft keys");
    a380d_destroy(ewd);
    a380d_inputs_free(in);

    a380d_display* sd = a380d_create("sdv2", "");
    in = a380d_inputs_new();
    a380d_set_dpr(in, 1.0);
    a380d_set_string(in, "CONFIG_A380X_SHOW_ECL_SOFTKEYS", "1");
    a380d_set_var(in, "L:A32NX_ECAM_SD_CURRENT_PAGE_INDEX", "number", 14);
    a380d_set_bus(in, "fws_information", "[\"000000001\"]");
    draw_once(sd, in);
    w = click(sd, {740, 630}, {740, 630}, &took);
    expect(took == 1 && same(w, {{'H', "H:A32NX_SD_STS_NEXT_PAGE", 1, 0}}), "sdv2: STATUS next-page soft key");
    a380d_set_var(in, "L:A32NX_ECAM_SD_CURRENT_PAGE_INDEX", "number", 11);
    draw_once(sd, in);
    w = click(sd, {740, 630}, {740, 630}, &took);
    expect(w.empty(), "sdv2: F/CTL page has no soft key");
    a380d_destroy(sd);
    a380d_inputs_free(in);

    {
        a380d_display* nd = a380d_create("nd", "1");
        a380d_inputs* ni = a380d_inputs_new();
        a380d_set_dpr(ni, 1.0);
        a380d_set_var(ni, "L:A32NX_EFIS_L_ND_RANGE", "number", 0);
        a380d_set_var(ni, "L:A32NX_EFIS_L_ND_MODE", "number", 3);
        draw_once(nd, ni);
        expect(a380d_input_native(nd) == 0, "nd: page owns input until the host publishes a panel model");
        a380d_set_bus(ni, "a380.oans.panel.1", "{\"open\":false,\"hits\":[{\"r\":[717,768,49,45],\"ev\":\"A380D_OANS_L_TOGGLE\"}]}");
        draw_once(nd, ni);
        w = click(nd, {740, 790}, {740, 790}, &took);
        expect(a380d_input_native(nd) == 1 && took == 1 && same(w, {{'H', "H:A380D_OANS_L_TOGGLE", 1, 0}}), "nd: OANS panel toggle writes the host event");
        w = click(nd, {740, 790}, {300, 300}, &took);
        expect(w.empty(), "nd: press and release on different targets writes nothing");
        a380d_set_var(ni, "L:A32NX_EFIS_L_ND_RANGE", "number", 2);
        draw_once(nd, ni);
        w = click(nd, {740, 790}, {740, 790}, &took);
        expect(took == 0 && w.empty(), "nd: no OANS targets outside ZOOM");
        a380d_destroy(nd);
        a380d_inputs_free(ni);
    }

    {
        a380d_display* efb = a380d_create("efbx", "");
        a380d_inputs* ei = a380d_inputs_new();
        a380d_set_dpr(ei, 1.0);
        const std::string dom =
            "<html><body data-cs=\"display:block;margin-top:0px;margin-left:0px\">"
            "<div data-cs=\"display:block;width:200px;height:50px\">a<span data-cs=\"display:inline\">b</span></div>"
            "<svg data-cs=\"display:block;width:20px;height:20px\" viewBox=\"0 0 20 20\"><g><path d=\"M0 0L5 5\"></path></g></svg>"
            "<div data-cs=\"display:block;height:30px\"><input data-cs=\"display:block;width:40px;height:10px\" value=\"x\"></div>"
            "</body></html>";
        a380d_set_bus_string(ei, "a380.efb.dom", dom.data(), dom.size());
        draw_once(efb, ei);
        float b[8 * 4] = {};
        int n = a380d_boxes(efb, b, 8);
        expect(n == 6, "efbx: boxes for body, div, span, svg, div, input (got " + std::to_string(n) + ")");
        expect(n >= 6 && b[0] == 0 && b[2] == 1430 && b[6] == 200 && b[7] == 50 && b[13] == 50 && b[17] == 70 && b[22] == 40 && b[23] == 10,
               "efbx: border boxes in document order");
        expect(a380d_boxes(efb, nullptr, 0) == n, "efbx: count without a buffer");
        a380d_display* pfd = a380d_create("pfd", "1");
        expect(a380d_boxes(pfd, b, 8) == 0, "pfd: no document, no boxes");
        a380d_destroy(pfd);
        a380d_destroy(efb);
        a380d_inputs_free(ei);
    }

    for (auto [id, variant, native] : std::vector<std::tuple<const char*, const char*, int>>{
             {"pfd", "1", 1}, {"nd", "1", 0}, {"mfdx", "", 0}, {"oitx", "1", 0}, {"isislegacy", "", 1}, {"clock", "", 1}, {"rmp", "1", 1}}) {
        a380d_display* d = a380d_create(id, variant);
        if (!d) {
            expect(false, std::string(id) + ": create");
            continue;
        }
        w = click(d, {100, 100}, {100, 100}, &took);
        expect(a380d_input_native(d) == native && took == 0 && w.empty(), std::string(id) + ": no page element takes clicks, native=" + std::to_string(native));
        a380d_destroy(d);
    }
    std::printf("input tests: %d passed, %d failed\n", passed, failed);
    return failed ? 1 : 0;
}

}  // namespace

int main(int argc, char** argv) {
    if (argc == 2 && !std::strcmp(argv[1], "--input")) return input_tests();
    if (argc < 5) {
        std::fprintf(stderr, "usage: c_api_test <id> <variant|-> <state.json> <out.png> [--model m.json] [--dom d.html] [--ref ref.png]\n");
        return 2;
    }
    std::string id = argv[1], variant = argv[2], state_path = argv[3], out_path = argv[4], model_path, dom_path, ref_path, layer_path;
    std::vector<Layer> stack;
    for (int i = 5; i + 1 < argc; i++) {
        if (!std::strcmp(argv[i], "--model")) model_path = argv[++i];
        else if (!std::strcmp(argv[i], "--dom")) dom_path = argv[++i];
        else if (!std::strcmp(argv[i], "--ref")) ref_path = argv[++i];
        else if (!std::strcmp(argv[i], "--layer")) layer_path = argv[++i];
        else if (!std::strcmp(argv[i], "--stack")) {
            Layer l;
            if (!parse_layer(argv[++i], l)) {
                std::fprintf(stderr, "bad --stack %s\n", argv[i]);
                return 2;
            }
            stack.push_back(std::move(l));
        }
    }
    if (variant == "-" || variant == "none") variant.clear();

    int gpu = a380d_gpu_init();
    if (!check(gpu, "gpu_init")) return 1;

    std::string text;
    if (!read_file(state_path, text)) {
        std::fprintf(stderr, "cannot read %s\n", state_path.c_str());
        return 1;
    }
    json state = json::parse(text, nullptr, false);
    if (state.is_discarded()) {
        std::fprintf(stderr, "cannot parse %s\n", state_path.c_str());
        return 1;
    }

    a380d_inputs* in = a380d_inputs_new();
    if (!in || !feed_state(in, state)) return 1;
    if (!model_path.empty()) {
        std::string m;
        if (!read_file(model_path, m) || !check(a380d_set_bus(in, "a380.mfd.model", m.c_str()), "set_bus model")) return 1;
    }
    if (!dom_path.empty()) {
        std::string d;
        if (!read_file(dom_path, d) || !check(a380d_set_bus_string(in, "a380.mfd.dom", d.data(), d.size()), "set_bus dom")) return 1;
    }

    a380d_display* display = a380d_create(id.c_str(), variant.empty() ? nullptr : variant.c_str());
    if (!display) {
        std::fprintf(stderr, "create failed: %s\n", a380d_last_error());
        return 1;
    }
    int cw = 0, ch = 0;
    a380d_size(display, &cw, &ch);
    double dpr = state.contains("env") && state["env"].is_object() && state["env"].contains("dpr") && state["env"]["dpr"].is_number()
                     ? state["env"]["dpr"].get<double>()
                     : 1.5;
    int w = (int)std::lround(cw * dpr), h = (int)std::lround(ch * dpr);
    int stride = w * 4;
    std::vector<uint8_t> a((size_t)stride * h), b((size_t)stride * h);

    if (!layer_path.empty()) {
        std::vector<uint8_t> px;
        int lw = 0, lh = 0;
        if (!decode_png(layer_path, px, lw, lh)) {
            std::fprintf(stderr, "cannot decode %s\n", layer_path.c_str());
            return 1;
        }
        if (!check(a380d_set_underlay(display, px.data(), lw, lh, lw * 4), "set_underlay")) return 1;
    }

    for (auto& l : stack) {
        std::string t;
        if (!read_file(l.state_path, t)) {
            std::fprintf(stderr, "cannot read %s\n", l.state_path.c_str());
            return 1;
        }
        json js = json::parse(t, nullptr, false);
        l.inputs = a380d_inputs_new();
        if (js.is_discarded() || !l.inputs || !feed_state(l.inputs, js)) return 1;
        l.display = a380d_create(l.id.c_str(), l.variant.empty() ? nullptr : l.variant.c_str());
        if (!l.display) {
            std::fprintf(stderr, "create %s failed: %s\n", l.id.c_str(), a380d_last_error());
            return 1;
        }
    }

    int r1, r2;
    if (stack.empty()) {
        r1 = a380d_draw(display, in, a.data(), w, h, stride);
    } else {
        std::vector<a380d_display*> ds{display};
        std::vector<const a380d_inputs*> is{in};
        for (auto& l : stack) {
            ds.push_back(l.display);
            is.push_back(l.inputs);
        }
        std::vector<uint8_t> single((size_t)stride * h);
        int rs = a380d_draw_layers(ds.data(), is.data(), 1, single.data(), w, h, stride);
        if (!check(rs, "draw_layers single")) return 1;
        int rd = a380d_draw(display, in, b.data(), w, h, stride);
        if (!check(rd, "draw")) return 1;
        std::printf("single layer through draw_layers identical to draw: %s\n", single == b ? "yes" : "NO");
        r1 = a380d_draw_layers(ds.data(), is.data(), (int)ds.size(), a.data(), w, h, stride);
    }
    if (!check(r1, "draw 1")) return 1;
    if (stack.empty()) {
        r2 = a380d_draw(display, in, b.data(), w, h, stride);
    } else {
        std::vector<a380d_display*> ds{display};
        std::vector<const a380d_inputs*> is{in};
        for (auto& l : stack) {
            ds.push_back(l.display);
            is.push_back(l.inputs);
        }
        r2 = a380d_draw_layers(ds.data(), is.data(), (int)ds.size(), b.data(), w, h, stride);
    }
    if (!check(r2, "draw 2")) return 1;
    bool same_again = a == b;
    std::printf("%s%s%s gpu=%d %dx%d draw1=%d draw2=%d redraw_identical=%s\n", id.c_str(), variant.empty() ? "" : "-", variant.c_str(), gpu, w, h, r1, r2,
                same_again ? "yes" : "NO");

    if (!write_png(out_path, a.data(), w, h, stride)) {
        std::fprintf(stderr, "cannot write %s\n", out_path.c_str());
        return 1;
    }
    int rc = r1 == 1 ? 0 : 3;

    if (!ref_path.empty()) {
        std::string x, y;
        if (!read_file(out_path, x) || !read_file(ref_path, y)) {
            std::fprintf(stderr, "cannot read png for comparison\n");
            return 1;
        }
        if (x == y) {
            std::printf("BYTE-IDENTICAL to %s (%zu bytes)\n", ref_path.c_str(), x.size());
        } else {
            std::printf("DIFFERENT from %s (%zu vs %zu bytes)\n", ref_path.c_str(), x.size(), y.size());
            rc = 4;
        }
    }

    for (auto& l : stack) {
        a380d_destroy(l.display);
        a380d_inputs_free(l.inputs);
    }
    a380d_destroy(display);
    a380d_inputs_free(in);
    return rc;
}
