#include "dom.h"
#include "a380/profile.h"

#include <algorithm>
#include <cctype>
#include <cstdint>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

namespace a380::mfdx {

namespace {

using json = nlohmann::json;

bool is_space(char c) { return c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\f'; }

std::string lower(std::string s) {
    for (char& c : s) c = (char)std::tolower((unsigned char)c);
    return s;
}

std::string trim(const std::string& s) {
    size_t a = 0, b = s.size();
    while (a < b && is_space(s[a])) a++;
    while (b > a && is_space(s[b - 1])) b--;
    return s.substr(a, b - a);
}

void put_utf8(std::string& out, uint32_t cp) {
    if (cp == 0 || cp > 0x10FFFF || (cp >= 0xD800 && cp <= 0xDFFF)) cp = 0xFFFD;
    if (cp < 0x80) out += (char)cp;
    else if (cp < 0x800) {
        out += (char)(0xC0 | (cp >> 6));
        out += (char)(0x80 | (cp & 0x3F));
    } else if (cp < 0x10000) {
        out += (char)(0xE0 | (cp >> 12));
        out += (char)(0x80 | ((cp >> 6) & 0x3F));
        out += (char)(0x80 | (cp & 0x3F));
    } else {
        out += (char)(0xF0 | (cp >> 18));
        out += (char)(0x80 | ((cp >> 12) & 0x3F));
        out += (char)(0x80 | ((cp >> 6) & 0x3F));
        out += (char)(0x80 | (cp & 0x3F));
    }
}

const std::unordered_map<std::string, uint32_t>& entities() {
    static const std::unordered_map<std::string, uint32_t> t = {
        {"amp", '&'}, {"lt", '<'}, {"gt", '>'}, {"quot", '"'}, {"apos", '\''}, {"nbsp", 0xA0}, {"copy", 0xA9}, {"reg", 0xAE},
        {"deg", 0xB0}, {"plusmn", 0xB1}, {"middot", 0xB7}, {"times", 0xD7}, {"divide", 0xF7}, {"laquo", 0xAB}, {"raquo", 0xBB},
        {"hellip", 0x2026}, {"ndash", 0x2013}, {"mdash", 0x2014}, {"minus", 0x2212}, {"larr", 0x2190}, {"rarr", 0x2192},
        {"uarr", 0x2191}, {"darr", 0x2193}, {"bull", 0x2022}, {"lsquo", 0x2018}, {"rsquo", 0x2019}, {"ldquo", 0x201C},
        {"rdquo", 0x201D}, {"thinsp", 0x2009}, {"ensp", 0x2002}, {"emsp", 0x2003}, {"micro", 0xB5}, {"sup2", 0xB2},
        {"sup3", 0xB3}, {"frac12", 0xBD}, {"frac14", 0xBC}, {"frac34", 0xBE}, {"trade", 0x2122}, {"euro", 0x20AC},
    };
    return t;
}

std::string unescape(const std::string& s) {
    if (s.find('&') == std::string::npos) return s;
    std::string out;
    out.reserve(s.size());
    for (size_t i = 0; i < s.size();) {
        if (s[i] != '&') {
            out += s[i++];
            continue;
        }
        size_t semi = s.find(';', i + 1);
        if (semi != std::string::npos && semi - i <= 32) {
            std::string name = s.substr(i + 1, semi - i - 1);
            if (!name.empty() && name[0] == '#') {
                bool hex = name.size() > 1 && (name[1] == 'x' || name[1] == 'X');
                std::string digits = name.substr(hex ? 2 : 1);
                bool ok = !digits.empty() && digits.size() <= 8;
                for (char c : digits) ok = ok && (hex ? std::isxdigit((unsigned char)c) : std::isdigit((unsigned char)c));
                if (ok) {
                    put_utf8(out, (uint32_t)std::stoul(digits, nullptr, hex ? 16 : 10));
                    i = semi + 1;
                    continue;
                }
            } else {
                auto it = entities().find(name);
                if (it != entities().end()) {
                    put_utf8(out, it->second);
                    i = semi + 1;
                    continue;
                }
            }
        }
        out += s[i++];
    }
    return out;
}

const char* const kVoid[] = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"};
const char* const kSkip[] = {"script", "style", "head", "title", "noscript"};
const char* const kLeaf[] = {"svg", "canvas", "img", "input", "textarea", "select"};

template <size_t N>
bool in_set(const char* const (&set)[N], const std::string& tag) {
    for (const char* s : set)
        if (tag == s) return true;
    return false;
}

struct Attr {
    std::string name;
    bool has_value = false;
    std::string value;
};
using Attrs = std::vector<Attr>;

std::unordered_map<std::string, std::string> parse_cs(const std::string& s) {
    std::unordered_map<std::string, std::string> out;
    size_t pos = 0;
    while (pos <= s.size()) {
        size_t semi = pos;
        int depth = 0;
        char quote = 0;
        for (; semi < s.size(); semi++) {
            char ch = s[semi];
            if (quote) {
                if (ch == quote) quote = 0;
            } else if (ch == '"' || ch == '\'') quote = ch;
            else if (ch == '(') depth++;
            else if (ch == ')') depth = depth > 0 ? depth - 1 : 0;
            else if (ch == ';' && depth == 0) break;
        }
        std::string part = s.substr(pos, semi - pos);
        size_t colon = part.find(':');
        if (colon != std::string::npos) out[trim(part.substr(0, colon))] = trim(part.substr(colon + 1));
        pos = semi + 1;
    }
    return out;
}

void fill_cs_defaults(std::unordered_map<std::string, std::string>& cs) {
    static const std::pair<const char*, const char*> initial[] = {
        {"position", "static"}, {"top", "auto"}, {"left", "auto"}, {"right", "auto"}, {"bottom", "auto"},
        {"min-width", "0px"}, {"min-height", "0px"}, {"max-width", "none"}, {"max-height", "none"},
        {"margin-top", "0px"}, {"margin-right", "0px"}, {"margin-bottom", "0px"}, {"margin-left", "0px"},
        {"padding-top", "0px"}, {"padding-right", "0px"}, {"padding-bottom", "0px"}, {"padding-left", "0px"},
        {"border-top-width", "0px"}, {"border-right-width", "0px"}, {"border-bottom-width", "0px"}, {"border-left-width", "0px"},
        {"border-top-style", "none"}, {"flex-direction", "row"}, {"flex-wrap", "nowrap"}, {"justify-content", "normal"},
        {"align-items", "normal"}, {"align-self", "auto"}, {"align-content", "normal"}, {"flex-grow", "0"}, {"flex-shrink", "1"},
        {"flex-basis", "auto"}, {"gap", "normal"}, {"row-gap", "normal"}, {"column-gap", "normal"},
        {"grid-template-columns", "none"}, {"grid-template-rows", "none"}, {"grid-column", "auto"}, {"grid-row", "auto"},
        {"order", "0"}, {"vertical-align", "baseline"}, {"text-decoration-line", "none"}, {"text-transform", "none"},
        {"background-color", "rgba(0, 0, 0, 0)"}, {"opacity", "1"}, {"visibility", "visible"}, {"overflow", "visible"},
        {"transform", "none"}, {"z-index", "auto"},
    };
    for (auto& kv : initial) cs.emplace(kv.first, kv.second);
    auto color = cs.find("color");
    if (color != cs.end())
        for (const char* side : {"top", "right", "bottom", "left"}) cs.emplace(std::string("border-") + side + "-color", color->second);
}

struct DNode {
    bool is_text = false;
    std::string text;
    std::string tag;
    std::string id;
    bool has_id = false;
    std::vector<std::string> cls;
    bool has_cls = false;
    std::unordered_map<std::string, std::string> cs;
    bool has_cs = false;
    std::string inline_style;
    std::unordered_map<std::string, std::string> svgattr;
    bool has_svgattr = false;
    std::string value;
    bool has_value = false;
    std::vector<json> shapes;
    std::vector<DNode> kids;

    DNode() = default;
    DNode(const DNode&) = default;
    DNode& operator=(const DNode&) = default;
    DNode(DNode&&) noexcept = default;
    DNode& operator=(DNode&&) noexcept = default;
};

class Builder {
public:
    DNode root;

    DNode& top() {
        DNode* n = &root;
        for (size_t idx : path_) n = &n->kids[idx];
        return *n;
    }

    void start(const std::string& tag, const Attrs& attrs) {
        if (tag == "body") in_body_ = true;
        if (!in_body_) return;
        bool is_void = in_set(kVoid, tag);
        if (skip_depth_ || in_set(kSkip, tag)) {
            if (!is_void) skip_depth_++;
            return;
        }
        auto find = [&](const char* name) -> const Attr* {
            const Attr* r = nullptr;
            for (auto& a : attrs)
                if (a.name == name) r = &a;
            return r;
        };
        if (leaf_depth_) {
            DNode& t = top();
            if (t.tag == "svg") {
                json a = json::object();
                for (auto& kv : attrs)
                    if (kv.has_value && kv.name != "data-cs") a[kv.name] = kv.value;
                a["_d"] = std::to_string(leaf_depth_ - 1);
                if (auto* cs = find("data-cs")) {
                    auto m = parse_cs(cs->value);
                    auto d = m.find("display");
                    if (d != m.end() && !d->second.empty()) a["_display"] = d->second;
                    for (const char* k : {"fill", "fill-opacity", "stroke", "stroke-width", "stroke-opacity", "stroke-dasharray", "stroke-linecap",
                                          "stroke-linejoin", "opacity", "transform"}) {
                        auto f = m.find(k);
                        if (f != m.end() && !f->second.empty()) a[std::string("_c_") + k] = f->second;
                    }
                    auto vis = m.find("visibility");
                    if (vis != m.end() && vis->second == "hidden") a["_display"] = "none";
                    if (tag == "text" || tag == "tspan")
                        for (const char* k : {"font-family", "font-size", "font-weight", "letter-spacing"}) {
                            auto f = m.find(k);
                            if (f != m.end() && !f->second.empty()) a[std::string("_") + k] = f->second;
                        }
                }
                t.shapes.push_back(json{{"t", tag}, {"a", std::move(a)}});
            }
            if (!is_void) leaf_depth_++;
            return;
        }
        DNode node;
        node.tag = tag;
        if (auto* v = find("id")) {
            node.has_id = true;
            node.id = v->value;
        }
        if (auto* v = find("class")) {
            node.has_cls = true;
            const std::string& s = v->value;
            size_t i = 0;
            while (i < s.size()) {
                while (i < s.size() && is_space(s[i])) i++;
                size_t j = i;
                while (j < s.size() && !is_space(s[j])) j++;
                if (j > i) node.cls.push_back(s.substr(i, j - i));
                i = j;
            }
        }
        if (auto* v = find("data-cs")) {
            node.has_cs = true;
            thread_local std::unordered_map<std::string, std::unordered_map<std::string, std::string>> parsed;
            auto it = parsed.find(v->value);
            if (it == parsed.end()) {
                if (parsed.size() > 4096) parsed.clear();
                auto cs = parse_cs(v->value);
                fill_cs_defaults(cs);
                it = parsed.emplace(v->value, std::move(cs)).first;
            }
            node.cs = it->second;
        }
        if (auto* v = find("style"))
            if (v->has_value) node.inline_style = v->value;
        if (tag == "svg") {
            for (auto& kv : attrs)
                if (kv.has_value && kv.name != "data-cs" && kv.name != "xmlns" && kv.name != "class") {
                    node.has_svgattr = true;
                    node.svgattr[kv.name] = kv.value;
                }
        }
        for (const char* k : {"data-st", "data-sl", "data-act", "data-label"})
            if (auto* v = find(k))
                if (v->has_value) {
                    node.has_svgattr = true;
                    node.svgattr[k] = v->value;
                }
        if (tag == "input")
            for (const char* k : {"type", "min", "max"})
                if (auto* v = find(k))
                    if (v->has_value) {
                        node.has_svgattr = true;
                        node.svgattr[k] = v->value;
                    }
        if (tag == "input")
            if (find("checked")) {
                node.has_svgattr = true;
                node.svgattr["checked"] = "1";
            }
        if (tag == "input" || tag == "textarea")
            if (auto* v = find("value")) {
                node.has_value = true;
                node.value = v->value;
            }
        DNode& parent = top();
        parent.kids.push_back(std::move(node));
        if (is_void) return;
        path_.push_back(parent.kids.size() - 1);
        if (in_set(kLeaf, tag)) leaf_depth_ = 1;
    }

    void end(const std::string& tag) {
        if (!in_body_) return;
        if (skip_depth_) {
            skip_depth_--;
            return;
        }
        if (leaf_depth_) {
            leaf_depth_--;
            if (leaf_depth_) return;
        }
        if (in_set(kVoid, tag)) return;
        if (!path_.empty()) path_.pop_back();
    }

    void data(const std::string& d) {
        if (in_body_ && !skip_depth_ && leaf_depth_ > 1 && !d.empty()) {
            DNode& t = top();
            if (t.tag == "svg" && !t.shapes.empty()) {
                json& last = t.shapes.back();
                std::string ty = last.value("t", "");
                if (ty == "text" || ty == "tspan") last["a"]["_text"] = last["a"].value("_text", "") + d;
            }
        }
        if (!in_body_ || skip_depth_ || leaf_depth_ || d.empty()) return;
        DNode t;
        t.is_text = true;
        t.text = d;
        top().kids.push_back(std::move(t));
    }

private:
    std::vector<size_t> path_;
    int skip_depth_ = 0;
    int leaf_depth_ = 0;
    bool in_body_ = false;
};

bool ieq_at(std::string_view s, size_t pos, std::string_view lit) {
    if (pos + lit.size() > s.size()) return false;
    for (size_t i = 0; i < lit.size(); i++)
        if (std::tolower((unsigned char)s[pos + i]) != lit[i]) return false;
    return true;
}

void parse_html(std::string_view s, Builder& b, bool keep_case) {
    size_t i = 0, n = s.size();
    std::string text;
    auto flush = [&] {
        if (!text.empty()) {
            b.data(unescape(text));
            text.clear();
        }
    };
    while (i < n) {
        if (s[i] != '<') {
            size_t lt = s.find('<', i);
            if (lt == std::string_view::npos) lt = n;
            text.append(s.substr(i, lt - i));
            i = lt;
            continue;
        }
        if (s.compare(i, 4, "<!--") == 0) {
            size_t e = s.find("-->", i + 4);
            i = e == std::string_view::npos ? n : e + 3;
            continue;
        }
        if (i + 1 < n && (s[i + 1] == '!' || s[i + 1] == '?')) {
            size_t e = s.find('>', i + 1);
            i = e == std::string_view::npos ? n : e + 1;
            continue;
        }
        if (i + 2 < n && s[i + 1] == '/' && std::isalpha((unsigned char)s[i + 2])) {
            size_t j = i + 2;
            while (j < n && !is_space(s[j]) && s[j] != '>' && s[j] != '/') j++;
            std::string tag = lower(std::string(s.substr(i + 2, j - i - 2)));
            size_t e = s.find('>', j);
            flush();
            b.end(tag);
            i = e == std::string_view::npos ? n : e + 1;
            continue;
        }
        if (i + 1 < n && std::isalpha((unsigned char)s[i + 1])) {
            size_t j = i + 1;
            while (j < n && !is_space(s[j]) && s[j] != '>' && s[j] != '/') j++;
            std::string tag = lower(std::string(s.substr(i + 1, j - i - 1)));
            Attrs attrs;
            bool self_closing = false;
            while (j < n) {
                while (j < n && is_space(s[j])) j++;
                if (j >= n) break;
                if (s[j] == '>') {
                    j++;
                    break;
                }
                if (s[j] == '/') {
                    if (j + 1 < n && s[j + 1] == '>') {
                        self_closing = true;
                        j += 2;
                        break;
                    }
                    j++;
                    continue;
                }
                size_t a0 = j++;
                while (j < n && !is_space(s[j]) && s[j] != '/' && s[j] != '=' && s[j] != '>') j++;
                Attr a;
                a.name = std::string(s.substr(a0, j - a0));
                if (!keep_case) a.name = lower(a.name);
                size_t k = j;
                while (k < n && is_space(s[k])) k++;
                if (k < n && s[k] == '=') {
                    while (k < n && s[k] == '=') k++;
                    while (k < n && is_space(s[k])) k++;
                    a.has_value = true;
                    if (k < n && (s[k] == '"' || s[k] == '\'')) {
                        char q = s[k];
                        size_t e = s.find(q, k + 1);
                        if (e == std::string_view::npos) e = n;
                        a.value = unescape(std::string(s.substr(k + 1, e - k - 1)));
                        k = e < n ? e + 1 : n;
                    } else {
                        size_t e = k;
                        while (e < n && s[e] != '>' && !is_space(s[e])) e++;
                        a.value = unescape(std::string(s.substr(k, e - k)));
                        k = e;
                    }
                    j = k;
                }
                attrs.push_back(std::move(a));
            }
            flush();
            b.start(tag, attrs);
            if (self_closing) b.end(tag);
            i = j;
            if (!self_closing && (tag == "script" || tag == "style")) {
                size_t e = i;
                std::string closing = "</" + tag;
                while (e < n && !ieq_at(s, e, closing)) e++;
                i = e;
            }
            continue;
        }
        text += '<';
        i++;
    }
    flush();
}

bool has_class(const DNode& d, const char* name) {
    for (const std::string& c : d.cls)
        if (c == name) return true;
    return false;
}

DNode make_pseudo(const DNode& owner, const std::string& label, bool wide) {
    DNode p;
    p.tag = "::before";
    p.has_cs = true;
    for (const char* k : {"font-family", "font-size", "font-weight", "line-height", "letter-spacing", "white-space", "color", "visibility"}) {
        auto it = owner.cs.find(k);
        if (it != owner.cs.end()) p.cs[k] = it->second;
    }
    p.cs["display"] = "block";
    p.cs["box-sizing"] = "content-box";
    if (wide) {
        p.cs["width"] = "120px";
        p.cs["text-align"] = "right";
        p.cs["margin-right"] = "20px";
    } else {
        p.cs["text-align"] = "left";
        p.cs["margin-right"] = "10px";
    }
    fill_cs_defaults(p.cs);
    DNode t;
    t.is_text = true;
    t.text = label;
    p.kids.push_back(std::move(t));
    return p;
}

void add_pseudo(DNode& d) {
    if (d.is_text) return;
    static const char* const kFieldLabels[] = {"RWY", "LAT/LONG", "ELEVATION", "LENGTH", "LS IDENT"};
    size_t field = 0;
    for (size_t i = 0; i < d.kids.size(); i++) {
        DNode& k = d.kids[i];
        if (k.is_text) continue;
        add_pseudo(k);
        if (!k.has_cs) continue;
        bool is_field = has_class(k, "mfd-data-airport-runway-field");
        bool is_course = has_class(k, "mfd-data-airport-runway-field-course");
        if (!is_field && !is_course) continue;
        std::string label = "CRS";
        if (is_field) {
            auto it = k.svgattr.find("data-label");
            label = it != k.svgattr.end() ? it->second : field < 5 ? kFieldLabels[field] : "";
            field++;
        }
        if (label.empty()) continue;
        k.kids.insert(k.kids.begin(), make_pseudo(k, label, is_field));
    }
}

std::unique_ptr<layout::Node> to_node(DNode& d, layout::Node* parent) {
    auto n = std::make_unique<layout::Node>();
    n->parent = parent;
    if (d.is_text) {
        n->is_text = true;
        n->text = std::move(d.text);
        return n;
    }
    n->tag = d.tag;
    if (d.has_id) n->id = d.id;
    if (d.has_cls) n->cls = d.cls;
    if (d.has_cs) n->css = std::move(d.cs);
    for (auto& sh : d.shapes) {
        layout::Node::Shape shape;
        shape.type = sh.value("t", "");
        if (sh.contains("a"))
            for (auto it = sh["a"].begin(); it != sh["a"].end(); ++it)
                shape.attr[it.key()] = it.value().is_string() ? it.value().get<std::string>() : it.value().dump();
        n->shapes.push_back(std::move(shape));
    }
    if (d.has_value) n->svgattr["value"] = d.value;
    if (d.has_svgattr)
        for (auto& kv : d.svgattr) n->svgattr[kv.first] = kv.second;
    for (auto& k : d.kids) {
        if (k.is_text && !n->kids.empty() && n->kids.back()->is_text) {
            n->kids.back()->text += k.text;
            continue;
        }
        n->kids.push_back(to_node(k, n.get()));
    }
    return n;
}

json to_json(const DNode& d) {
    if (d.is_text) return json{{"text", d.text}};
    json j = json::object();
    j["tag"] = d.tag;
    if (d.has_id) j["id"] = d.id;
    if (d.has_cls) j["cls"] = d.cls;
    if (d.has_cs) {
        json cs = json::object();
        for (auto& kv : d.cs) cs[kv.first] = kv.second;
        j["cs"] = std::move(cs);
    }
    if (!d.inline_style.empty()) j["inline"] = d.inline_style;
    if (d.has_svgattr) {
        json sa = json::object();
        for (auto& kv : d.svgattr) sa[kv.first] = kv.second;
        j["svgattr"] = std::move(sa);
    }
    if (d.has_value) j["value"] = d.value;
    if (!d.shapes.empty()) j["shapes"] = d.shapes;
    json kids = json::array();
    for (auto& k : d.kids) {
        if (k.is_text && !kids.empty() && !kids.back().contains("tag")) {
            kids.back()["text"] = kids.back()["text"].get<std::string>() + k.text;
            continue;
        }
        kids.push_back(to_json(k));
    }
    j["kids"] = std::move(kids);
    return j;
}

}  // namespace

layout::Tree dom_to_layout(std::string_view html, bool keep_attr_case, float dpr) {
    Builder b;
    parse_html(html, b, keep_attr_case);
    layout::Tree t;
    t.dpr = dpr;
    if (!b.root.kids.empty() && !b.root.kids[0].is_text) {
        add_pseudo(b.root.kids[0]);
        t.root = to_node(b.root.kids[0], nullptr);
    }
    return t;
}

json dom_to_tree(std::string_view html, bool keep_attr_case) {
    static a380::SectionProfile prof("dom_to_tree");
    Builder b;
    prof.time("parse_html", [&] { parse_html(html, b, keep_attr_case); });
    if (b.root.kids.empty()) return json::object();
    json out;
    prof.time("to_json", [&] {
        add_pseudo(b.root.kids[0]);
        out = to_json(b.root.kids[0]);
    });
    prof.frame();
    return out;
}

}  // namespace a380::mfdx
