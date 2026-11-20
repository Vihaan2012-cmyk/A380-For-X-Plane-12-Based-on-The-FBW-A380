#include "a380/layout.h"

#include <algorithm>
#include <climits>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <functional>
#include <map>

#include "a380/draw.h"
#include "a380/fonts.h"

namespace a380::layout {

namespace {
float g_zoom = 1.5f;
constexpr int kHuge = 64 * 200000;
constexpr int kNA = INT_MIN;

int to_lu(float css) { return (int)std::lround(css * g_zoom * 64.f); }
float to_css(int v) { return v / 64.f / g_zoom; }
int floor_px(int v) { return (int)std::floor(v / 64.0) * 64; }
int lu_div(int v, int d) { return v / d; }

struct Len {
    int k = 0;
    int px = 0;
    float pct = 0;
    bool is_auto() const { return k == 0; }
};

Len parse_len(const std::string& s) {
    Len l;
    if (s.empty()) return l;
    char c = s[0];
    if (!(std::isdigit((unsigned char)c) || c == '-' || c == '.' || c == '+')) return l;
    char* end = nullptr;
    float v = std::strtof(s.c_str(), &end);
    if (end && *end == '%') {
        l.k = 2;
        l.pct = v;
    } else {
        l.k = 1;
        l.px = to_lu(v);
    }
    return l;
}

int resolve(const Len& l, int base) {
    if (l.k == 1) return l.px;
    if (l.k == 2) return base == kNA ? kNA : (int)((long long)base * l.pct / 100.f);
    return kNA;
}

enum class Disp { None, Block, Inline, InlineBlock, Flex, InlineFlex, Grid, Contents };
enum class Pos { Static, Relative, Absolute, Fixed };
enum class Just { Normal, Start, End, Center, Between, Around, Evenly };
enum class Align { Auto, Normal, Start, End, Center, Stretch, Baseline };
enum class TA { Start, Left, Center, Right, End };

}  // namespace

struct ComputedStyle {
    Disp display = Disp::Block;
    Pos pos = Pos::Static;
    Len width, height, minw, minh, maxw, maxh, off[4];
    float aspect = 0;
    Len margin[4], padding[4];
    int border[4] = {0, 0, 0, 0};
    bool border_box = true;
    bool column = false, reverse = false, wrap = false;
    Just justify = Just::Normal;
    Just align_content = Just::Normal;
    Align align_items = Align::Normal, align_self = Align::Auto;
    float grow = 0, shrink = 1;
    Len basis;
    int row_gap = 0, col_gap = 0;
    int order = 0;
    std::string family;
    float size_css = 16;
    int weight = 400;
    float ls_css = 0;
    Len line_height;
    TA ta = TA::Start;
    bool nowrap = false, pre = false, pre_newlines = false;
    bool overflow_clip = false;
    std::vector<int> grid_cols;
    std::string grid_col_place, grid_row_place;
};

namespace {

using Style = ComputedStyle;

const std::string kEmpty;

}  // namespace

std::string Node::get(const std::string& key) const {
    auto it = css.find(key);
    return it == css.end() ? kEmpty : it->second;
}
bool Node::has_class(const std::string& c) const {
    for (auto& k : cls)
        if (k == c) return true;
    return false;
}
std::string Node::class_string() const {
    std::string s;
    for (auto& c : cls) s += (s.empty() ? "" : " ") + c;
    return s;
}
std::string Node::own_text() const {
    std::string s;
    for (auto& k : kids)
        if (k->is_text) s += k->text;
    return s;
}

namespace {

const char* kInherited[] = {"font-family", "font-size", "font-weight", "line-height", "letter-spacing", "text-align",
                            "white-space", "color", "visibility", "text-transform"};

std::string inherited_get(const Node& n, const std::string& key) {
    for (const Node* p = &n; p; p = p->parent) {
        auto it = p->css.find(key);
        if (it != p->css.end()) return it->second;
    }
    return kEmpty;
}

bool is_inherited(const std::string& k) {
    for (auto* s : kInherited)
        if (k == s) return true;
    return false;
}

std::string prop(const Node& n, const std::string& key) {
    if (is_inherited(key)) return inherited_get(n, key);
    return n.get(key);
}

Just parse_just(const std::string& s) {
    if (s == "flex-start" || s == "start" || s == "left") return Just::Start;
    if (s == "flex-end" || s == "end" || s == "right") return Just::End;
    if (s == "center") return Just::Center;
    if (s == "space-between") return Just::Between;
    if (s == "space-around") return Just::Around;
    if (s == "space-evenly") return Just::Evenly;
    return Just::Normal;
}

Align parse_align(const std::string& s) {
    if (s == "flex-start" || s == "start" || s == "self-start") return Align::Start;
    if (s == "flex-end" || s == "end" || s == "self-end") return Align::End;
    if (s == "center") return Align::Center;
    if (s == "stretch") return Align::Stretch;
    if (s == "baseline" || s == "first baseline") return Align::Baseline;
    if (s == "normal") return Align::Normal;
    return Align::Auto;
}

std::vector<int> parse_tracks(const std::string& s) {
    std::vector<int> v;
    if (s.empty() || s == "none") return v;
    const char* p = s.c_str();
    while (*p) {
        while (*p == ' ') p++;
        if (!*p) break;
        char* end = nullptr;
        float f = std::strtof(p, &end);
        if (end == p) {
            v.push_back(kNA);
            while (*p && *p != ' ') p++;
            continue;
        }
        v.push_back(to_lu(f));
        p = end;
        while (*p && *p != ' ') p++;
    }
    return v;
}

void compute_style(Node& n) {
    auto s = std::make_shared<Style>();
    auto g = [&](const char* k) { return prop(n, k); };
    std::string d = g("display");
    if (d == "none") s->display = Disp::None;
    else if (d == "inline") s->display = Disp::Inline;
    else if (d == "inline-block") s->display = Disp::InlineBlock;
    else if (d == "flex") s->display = Disp::Flex;
    else if (d == "inline-flex") s->display = Disp::InlineFlex;
    else if (d == "grid" || d == "inline-grid") s->display = Disp::Grid;
    else if (d == "contents") s->display = Disp::Contents;
    else s->display = (d.empty() && (n.tag == "span" || n.tag == "b" || n.tag == "i" || n.tag == "a")) ? Disp::Inline : Disp::Block;
    std::string p = g("position");
    s->pos = p == "relative" ? Pos::Relative : p == "absolute" ? Pos::Absolute : p == "fixed" ? Pos::Fixed : Pos::Static;
    s->width = parse_len(g("width"));
    s->height = parse_len(g("height"));
    s->minw = parse_len(g("min-width"));
    s->minh = parse_len(g("min-height"));
    s->maxw = parse_len(g("max-width"));
    s->maxh = parse_len(g("max-height"));
    {
        std::string ar = g("aspect-ratio");
        size_t slash = ar.find('/');
        float a = std::strtof(ar.c_str(), nullptr), b = slash == std::string::npos ? 1.f : std::strtof(ar.c_str() + slash + 1, nullptr);
        if (a > 0.f && b > 0.f) s->aspect = a / b;
    }
    if (n.tag == "svg") {
        auto attr = [&](const char* k) {
            auto it = n.svgattr.find(k);
            return it == n.svgattr.end() ? Len{} : parse_len(it->second);
        };
        if (s->width.is_auto()) s->width = attr("width");
        if (s->height.is_auto()) s->height = attr("height");
    }
    if (s->aspect <= 0.f && n.tag == "svg") {
        auto vb = n.svgattr.find("viewBox");
        if (vb == n.svgattr.end()) vb = n.svgattr.find("viewbox");
        if (vb != n.svgattr.end()) {
            float v[4] = {0, 0, 0, 0};
            const char* p = vb->second.c_str();
            for (int i = 0; i < 4 && *p; i++) {
                char* end = nullptr;
                v[i] = std::strtof(p, &end);
                if (end == p) break;
                p = end;
                while (*p == ' ' || *p == ',') p++;
            }
            if (v[2] > 0.f && v[3] > 0.f) s->aspect = v[2] / v[3];
        }
    }
    const char* sides[] = {"top", "right", "bottom", "left"};
    for (int i = 0; i < 4; i++) {
        s->off[i] = parse_len(g(sides[i]));
        std::string mv = g((std::string("margin-") + sides[i]).c_str()), pv = g((std::string("padding-") + sides[i]).c_str());
        s->margin[i] = mv.empty() ? Len{1, 0, 0} : parse_len(mv);
        s->padding[i] = pv.empty() ? Len{1, 0, 0} : parse_len(pv);
        std::string bw = g((std::string("border-") + sides[i] + "-width").c_str());
        std::string bs = g((std::string("border-") + sides[i] + "-style").c_str());
        float f = bw.empty() ? 0.f : std::strtof(bw.c_str(), nullptr);
        s->border[i] = (bs == "none" || bs == "hidden") ? 0 : (int)std::floor(f * g_zoom + 1e-3) * 64;
    }
    s->border_box = g("box-sizing") != "content-box";
    std::string fd = g("flex-direction");
    s->column = fd == "column" || fd == "column-reverse";
    s->reverse = fd == "row-reverse" || fd == "column-reverse";
    s->wrap = g("flex-wrap") != "nowrap" && !g("flex-wrap").empty();
    s->justify = parse_just(g("justify-content"));
    s->align_content = parse_just(g("align-content"));
    s->align_items = parse_align(g("align-items"));
    s->align_self = parse_align(g("align-self"));
    std::string fg = g("flex-grow"), fs = g("flex-shrink");
    s->grow = fg.empty() ? 0.f : std::strtof(fg.c_str(), nullptr);
    s->shrink = fs.empty() ? 1.f : std::strtof(fs.c_str(), nullptr);
    s->basis = parse_len(g("flex-basis"));
    std::string rg = g("row-gap"), cg = g("column-gap");
    s->row_gap = rg.empty() || rg == "normal" ? 0 : to_lu(std::strtof(rg.c_str(), nullptr));
    s->col_gap = cg.empty() || cg == "normal" ? 0 : to_lu(std::strtof(cg.c_str(), nullptr));
    std::string o = g("order");
    s->order = o.empty() ? 0 : std::atoi(o.c_str());
    std::string fam = g("font-family");
    fam.erase(std::remove(fam.begin(), fam.end(), '"'), fam.end());
    fam.erase(std::remove(fam.begin(), fam.end(), '\''), fam.end());
    s->family = fam.empty() ? "Times New Roman" : fam;
    std::string fz = g("font-size");
    s->size_css = fz.empty() ? 16.f : std::strtof(fz.c_str(), nullptr);
    std::string fw = g("font-weight");
    s->weight = fw == "bold" ? 700 : fw.empty() || fw == "normal" ? 400 : std::atoi(fw.c_str());
    std::string ls = g("letter-spacing");
    s->ls_css = (ls.empty() || ls == "normal") ? 0.f : std::strtof(ls.c_str(), nullptr);
    s->line_height = parse_len(g("line-height"));
    std::string ta = g("text-align");
    s->ta = ta == "center" ? TA::Center : ta == "right" ? TA::Right : ta == "end" ? TA::End : ta == "left" ? TA::Left : TA::Start;
    std::string ws = g("white-space");
    s->nowrap = ws == "nowrap" || ws == "pre";
    s->pre = ws == "pre" || ws == "pre-wrap";
    s->pre_newlines = ws == "pre" || ws == "pre-wrap" || ws == "pre-line";
    std::string ov = g("overflow");
    s->overflow_clip = ov == "hidden" || ov == "clip" || ov == "auto" || ov == "scroll";
    s->grid_cols = parse_tracks(n.get("grid-template-columns"));
    n.st = s;
}

struct FontInfo {
    SkFont font;
    int asc = 0, desc = 0, gap = 0;
};

FontInfo& font_info(const Style& s) {
    thread_local std::map<std::tuple<std::string, int, int, int>, FontInfo> cache;
    auto key = std::make_tuple(s.family, (int)std::lround(s.size_css * 1000), s.weight, (int)std::lround(g_zoom * 1000));
    auto it = cache.find(key);
    if (it != cache.end()) return it->second;
    FontInfo fi;
    float dev = s.size_css * g_zoom;
    fi.font = a380::font(s.family, dev, declared_font_weight(s.family, s.weight, nullptr), TextMode::Greyscale);
    HheaMetrics m = hhea_metrics(fi.font);
    fi.asc = (int)std::lround(m.ascent * dev);
    fi.desc = (int)std::lround(m.descent * dev);
    fi.gap = (int)std::lround(m.line_gap * dev);
    return cache.emplace(key, fi).first->second;
}

std::string normalise_spaces(const std::string& t) {
    if (t.find("\xC2\xA0") == std::string::npos) return t;
    std::string o;
    o.reserve(t.size());
    for (size_t i = 0; i < t.size(); i++) {
        if ((unsigned char)t[i] == 0xC2 && i + 1 < t.size() && (unsigned char)t[i + 1] == 0xA0) {
            o += ' ';
            i++;
        } else o += t[i];
    }
    return o;
}

int text_w(const Style& s, const std::string& raw) {
    if (raw.empty()) return 0;
    const std::string t = normalise_spaces(raw);
    FontInfo& fi = font_info(s);
    float adv = text_advance(fi.font, t.data(), t.size(), SkTextEncoding::kUTF8);
    if (s.ls_css != 0.f) {
        size_t glyphs = fi.font.textToGlyphs(t.data(), t.size(), SkTextEncoding::kUTF8, {});
        adv += s.ls_css * g_zoom * (float)glyphs;
    }
    return (int)std::ceil(adv * 64.f - 1e-3f);
}

int line_height_lu(const Style& s) {
    FontInfo& fi = font_info(s);
    if (s.line_height.k == 1) return s.line_height.px;
    return (fi.asc + fi.desc + fi.gap) * 64;
}

void metrics_above_below(const Style& s, int& above, int& below, int& asc_px, int& desc_px) {
    FontInfo& fi = font_info(s);
    int lh = line_height_lu(s);
    int content = (fi.asc + fi.desc) * 64;
    int half = floor_px(lu_div(lh - content, 2));
    above = fi.asc * 64 + half;
    below = lh - above;
    asc_px = fi.asc;
    desc_px = fi.desc;
}

bool blank(const std::string& s) {
    for (char c : s)
        if (!(c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\f')) return false;
    return true;
}

bool is_abs(const Node& n) { return !n.is_text && n.st && (n.st->pos == Pos::Absolute || n.st->pos == Pos::Fixed); }
bool is_none(const Node& n) { return !n.is_text && n.st && n.st->display == Disp::None; }
bool inline_level(const Node& n) {
    if (n.is_text) return true;
    if (!n.st || is_abs(n)) return false;
    Disp d = n.st->display;
    return d == Disp::Inline || d == Disp::InlineBlock || d == Disp::InlineFlex;
}

struct Cons {
    int fw = kNA, fh = kNA;
    bool fh_definite = false;
    bool shrink = false;
    int avail = kNA;
    int cbw = kNA, cbh = kNA;
};

struct AbsEntry {
    Node* n;
    Node* parent;
    int sx, sy;
};

std::vector<AbsEntry>* g_abs = nullptr;

void layout_node(Node& n, const Cons& c);
int max_content(Node& n);
int min_content(Node& n);
void wrap_anonymous(Node& n);

int pad_side(const Node& n, int i, int cbw) {
    int v = resolve(n.st->padding[i], cbw);
    return v == kNA ? 0 : std::max(0, v);
}
int margin_side(const Node& n, int i, int cbw) {
    int v = resolve(n.st->margin[i], cbw);
    return v == kNA ? 0 : v;
}
bool margin_auto(const Node& n, int i) { return n.st->margin[i].is_auto(); }

int clamp_w(const Node& n, int w, int cbw, int hpb) {
    const Style& s = *n.st;
    int mx = resolve(s.maxw, cbw), mn = resolve(s.minw, cbw);
    if (mx != kNA) w = std::min(w, s.border_box ? mx : mx + hpb);
    if (mn != kNA) w = std::max(w, s.border_box ? mn : mn + hpb);
    return std::max(w, hpb);
}
int clamp_h(const Node& n, int h, int cbh, int vpb) {
    const Style& s = *n.st;
    int mx = resolve(s.maxh, cbh), mn = resolve(s.minh, cbh);
    if (mx != kNA) h = std::min(h, s.border_box ? mx : mx + vpb);
    if (mn != kNA) h = std::max(h, s.border_box ? mn : mn + vpb);
    return std::max(h, vpb);
}

struct Tok {
    int kind;
    Node* owner;
    Node* tnode;
    std::string s;
    int w = 0;
    Node* atom = nullptr;
    bool collapsible = true;
};

void tokenise_text(Node* t, std::vector<Tok>& out) {
    Node* p = t->parent;
    const Style& ps = *p->st;
    const std::string& s = t->text;
    size_t i = 0;
    if (ps.pre) {
        while (i < s.size()) {
            if (s[i] == '\n') {
                out.push_back({5, p, t, "", 0, nullptr, false});
                i++;
            } else if (s[i] == ' ' || s[i] == '\t') {
                size_t j = i;
                while (j < s.size() && (s[j] == ' ' || s[j] == '\t')) j++;
                std::string sp = s.substr(i, j - i);
                if (ps.nowrap) out.push_back({0, p, t, sp, 0, nullptr, false});
                else
                    for (char ch : sp) out.push_back({1, p, t, std::string(1, ch), 0, nullptr, false});
                i = j;
            } else {
                size_t j = i;
                while (j < s.size() && s[j] != '\n' && s[j] != ' ' && s[j] != '\t') j++;
                out.push_back({0, p, t, s.substr(i, j - i), 0, nullptr, false});
                i = j;
            }
        }
        return;
    }
    while (i < s.size()) {
        char c = s[i];
        bool ws = c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\f';
        if (ws && ps.pre_newlines && c == '\n') {
            out.push_back({5, p, t, "", 0, nullptr, false});
            i++;
            continue;
        }
        if (ws) {
            while (i < s.size() && (s[i] == ' ' || s[i] == '\t' || s[i] == '\n' || s[i] == '\r' || s[i] == '\f')) i++;
            out.push_back({1, p, t, " ", 0, nullptr, true});
        } else {
            size_t j = i;
            while (j < s.size() && !(s[j] == ' ' || s[j] == '\t' || s[j] == '\n' || s[j] == '\r' || s[j] == '\f')) j++;
            out.push_back({0, p, t, s.substr(i, j - i), 0, nullptr, true});
            i = j;
        }
    }
}

void flatten(Node* n, std::vector<Tok>& out, int cbw, bool measure);

int inline_edge(const Node& n, int side, int cbw) {
    return margin_side(n, side, cbw) + n.st->border[side] + pad_side(n, side, cbw);
}

void flatten_one(Node* k, std::vector<Tok>& out, int cbw, bool measure) {
    if (k->is_text) {
        tokenise_text(k, out);
        return;
    }
    if (!k->st || k->st->display == Disp::None || is_abs(*k)) return;
    if (k->tag == "br") {
        out.push_back({5, k, nullptr, "", 0, nullptr, false});
        return;
    }
    Disp d = k->st->display;
    bool replaced = k->tag == "svg" || k->tag == "img" || k->tag == "input" || k->tag == "canvas" || k->tag == "textarea" || k->tag == "select";
    if (d == Disp::Inline && !replaced) {
        Tok o{3, k, nullptr, "", inline_edge(*k, 3, cbw)};
        out.push_back(o);
        for (auto& kk : k->kids) flatten_one(kk.get(), out, cbw, measure);
        Tok e{4, k, nullptr, "", inline_edge(*k, 1, cbw)};
        out.push_back(e);
    } else {
        Tok a{2, k, nullptr, "", 0, k, false};
        out.push_back(a);
    }
}

struct Piece {
    int kind;
    Node* owner;
    Node* tnode;
    std::string s;
    int w;
    Node* atom;
    bool collapsible;
    int x = 0;
};

struct LineOut {
    std::vector<Piece> pieces;
    int w = 0;
};

int merged_w(const std::vector<Piece>& ps, const Tok* extra) {
    int total = 0;
    size_t i = 0;
    size_t n = ps.size() + (extra ? 1 : 0);
    auto at = [&](size_t k) -> Piece {
        if (k < ps.size()) return ps[k];
        return Piece{extra->kind, extra->owner, extra->tnode, extra->s, extra->w, extra->atom, extra->collapsible};
    };
    while (i < n) {
        Piece p = at(i);
        if (p.kind == 0 || p.kind == 1) {
            std::string s;
            size_t j = i;
            while (j < n) {
                Piece q = at(j);
                if (!((q.kind == 0 || q.kind == 1) && q.tnode == p.tnode)) break;
                s += q.s;
                j++;
            }
            total += text_w(*p.owner->st, s);
            i = j;
        } else {
            total += p.w;
            i++;
        }
    }
    return total;
}

void layout_inline_run(Node& blk, std::vector<Node*>& run, int cw, int origin_x, int top_y, bool measure, int* out_h, int* out_maxw,
                       int* first_base) {
    const Style& bs = *blk.st;
    std::vector<Tok> toks;
    for (Node* k : run) flatten_one(k, toks, cw, measure);

    for (auto& t : toks) {
        if (t.kind == 0 || t.kind == 1) t.w = text_w(*t.owner->st, t.s);
        else if (t.kind == 2) {
            Node& a = *t.atom;
            if (measure) {
                t.w = max_content(a) + margin_side(a, 1, cw) + margin_side(a, 3, cw);
            } else {
                Cons c;
                c.shrink = true;
                int ml = margin_side(a, 3, cw), mr = margin_side(a, 1, cw);
                c.avail = std::max(0, cw - ml - mr);
                c.cbw = cw;
                layout_node(a, c);
                t.w = a.lw + ml + mr;
            }
        }
    }

    std::vector<LineOut> lines(1);
    bool has_content = false;
    size_t break_at = 0;
    auto trim = [&](LineOut& l) {
        while (!l.pieces.empty() && l.pieces.back().kind == 1 && l.pieces.back().collapsible) {
            l.w -= l.pieces.back().w;
            l.pieces.pop_back();
        }
    };
    auto newline = [&] {
        trim(lines.back());
        lines.emplace_back();
        has_content = false;
        break_at = 0;
    };
    auto wrap_at_break = [&]() -> bool {
        LineOut& cur = lines.back();
        if (break_at == 0 || break_at >= cur.pieces.size()) return false;
        LineOut tail;
        tail.pieces.assign(cur.pieces.begin() + break_at, cur.pieces.end());
        for (auto& p : tail.pieces) tail.w += p.w;
        cur.pieces.erase(cur.pieces.begin() + break_at, cur.pieces.end());
        cur.w -= tail.w;
        trim(cur);
        lines.push_back(tail);
        break_at = 0;
        has_content = true;
        return true;
    };
    int avail = measure ? kHuge : cw;
    for (auto& t : toks) {
        LineOut& cur = lines.back();
        const Style* os = t.owner && t.owner->st ? t.owner->st.get() : &bs;
        switch (t.kind) {
            case 3:
            case 4:
                cur.pieces.push_back({t.kind, t.owner, nullptr, "", t.w, nullptr, false});
                cur.w += t.w;
                break;
            case 5:
                newline();
                break;
            case 2: {
                if (!measure && !bs.nowrap && has_content && cur.w + t.w > avail) {
                    if (!wrap_at_break()) newline();
                }
                LineOut& c2 = lines.back();
                c2.pieces.push_back({2, t.owner, nullptr, "", t.w, t.atom, false});
                c2.w += t.w;
                has_content = true;
                break_at = c2.pieces.size();
                break;
            }
            case 1: {
                if (t.collapsible && !has_content) break;
                if (t.collapsible && !cur.pieces.empty() && cur.pieces.back().kind == 1 && cur.pieces.back().collapsible) break;
                cur.pieces.push_back({1, t.owner, t.tnode, t.s, t.w, nullptr, t.collapsible});
                cur.w += t.w;
                break_at = cur.pieces.size();
                break;
            }
            case 0: {
                if (!os->nowrap && !measure && has_content && merged_w(cur.pieces, &t) > avail) {
                    if (!wrap_at_break()) {
                        bool glued = break_at == 0;
                        if (!glued) newline();
                    }
                }
                LineOut& c2 = lines.back();
                c2.pieces.push_back({0, t.owner, t.tnode, t.s, t.w, nullptr, false});
                c2.w += t.w;
                has_content = true;
                break;
            }
        }
    }
    trim(lines.back());

    int y = top_y;
    int maxw = 0;
    int bsa, bsb, basc, bdesc;
    metrics_above_below(bs, bsa, bsb, basc, bdesc);
    bool first = true;
    for (auto& L : lines) {
        struct Frag {
            int kind;
            Node* owner;
            std::string s;
            int w;
            Node* atom;
            int x;
        };
        std::vector<Frag> frags;
        for (size_t i = 0; i < L.pieces.size();) {
            Piece& p = L.pieces[i];
            if (p.kind == 0 || p.kind == 1) {
                size_t j = i;
                std::string s;
                while (j < L.pieces.size() && (L.pieces[j].kind == 0 || L.pieces[j].kind == 1) && L.pieces[j].tnode == p.tnode) {
                    s += L.pieces[j].s;
                    j++;
                }
                frags.push_back({0, p.owner, s, text_w(*p.owner->st, s), nullptr, 0});
                i = j;
            } else {
                frags.push_back({p.kind, p.owner, "", p.w, p.atom, 0});
                i++;
            }
        }
        int lw = 0;
        for (auto& f : frags) lw += f.w;
        maxw = std::max(maxw, lw);
        if (measure) {
            continue;
        }
        int above = bsa, below = bsb;
        bool any_text = false;
        for (auto& f : frags) {
            if (f.kind == 0) {
                int a, b, ap, dp;
                metrics_above_below(*f.owner->st, a, b, ap, dp);
                above = std::max(above, a);
                below = std::max(below, b);
                any_text = true;
            } else if (f.kind == 2) {
                Node& a = *f.atom;
                int mt = margin_side(a, 0, cw), mb = margin_side(a, 2, cw);
                int base = a.lbase == kNA ? a.lh : a.lbase;
                int ab = base + mt, be = a.lh + mt + mb - ab;
                above = std::max(above, ab);
                below = std::max(below, be);
            }
        }
        (void)any_text;
        int lh = above + below;
        int free_space = std::max(0, cw - lw);
        int off = 0;
        TA ta = bs.ta;
        if (ta == TA::Center) off = lu_div(free_space, 2);
        else if (ta == TA::Right || ta == TA::End) off = free_space;
        int x = origin_x + off;
        int baseline = y + above;
        for (auto& f : frags) {
            if (f.kind == 3) x += f.w;
            else if (f.kind == 4) x += f.w;
            else if (f.kind == 0) {
                FontInfo& fi = font_info(*f.owner->st);
                blk.frags.push_back({f.owner, x, baseline - fi.asc * 64, f.w, (fi.asc + fi.desc) * 64, baseline, f.s});
                x += f.w;
            } else {
                Node& a = *f.atom;
                int ml = margin_side(a, 3, cw), mt = margin_side(a, 0, cw);
                int base = a.lbase == kNA ? a.lh : a.lbase;
                a.lx = x + ml;
                a.ly = baseline - base;
                (void)mt;
                x += f.w;
            }
        }
        if (first) {
            *first_base = baseline - top_y + top_y;
            first = false;
        }
        y += lh;
    }
    *out_h = measure ? 0 : y - top_y;
    *out_maxw = maxw;
}

Node* first_flow_child(Node& n) {
    for (auto& k : n.kids) {
        if (k->is_text) {
            if (blank(k->text)) continue;
            return nullptr;
        }
        if (!k->st || is_none(*k) || is_abs(*k)) continue;
        if (inline_level(*k)) return nullptr;
        return k.get();
    }
    return nullptr;
}

bool collapses_top(Node& n) {
    if (!n.st || !n.parent || !n.parent->st) return false;
    const Style& s = *n.st;
    Disp pd = n.parent->st->display;
    if (pd != Disp::Block) return false;
    if (s.display != Disp::Block || s.overflow_clip || s.border[0] != 0) return false;
    if (s.pos == Pos::Absolute || s.pos == Pos::Fixed) return false;
    if (s.padding[0].k == 1 && s.padding[0].px != 0) return false;
    if (s.padding[0].k == 2) return false;
    return first_flow_child(n) != nullptr;
}

bool collapses_bottom(Node& n) {
    if (!n.st || !n.parent || !n.parent->st) return false;
    const Style& s = *n.st;
    if (n.parent->st->display != Disp::Block) return false;
    if (s.display != Disp::Block || s.overflow_clip || s.border[2] != 0) return false;
    if (s.pos == Pos::Absolute || s.pos == Pos::Fixed) return false;
    if (s.padding[2].k == 1 && s.padding[2].px != 0) return false;
    if (s.padding[2].k == 2) return false;
    if (s.minh.k == 1 && s.minh.px > 0) return false;
    return true;
}

int combine_margins(int a, int b) {
    if (a >= 0 && b >= 0) return std::max(a, b);
    if (a < 0 && b < 0) return std::min(a, b);
    return a + b;
}

int eff_mt(Node& c, int cbw) {
    int mt = margin_side(c, 0, cbw);
    if (collapses_top(c)) mt = combine_margins(mt, eff_mt(*first_flow_child(c), cbw));
    return mt;
}

int block_content(Node& n, int cw, int origin_x, int origin_y, bool measure, int* maxw, int* first_base, int* trail = nullptr) {
    int y = origin_y;
    int mw = 0;
    int pending_mb = kNA;
    size_t i = 0;
    auto& kids = n.kids;
    while (i < kids.size()) {
        Node* k = kids[i].get();
        if (!k->is_text && !k->st) {
            i++;
            continue;
        }
        if (!k->is_text && is_none(*k)) {
            i++;
            continue;
        }
        if (is_abs(*k)) {
            if (g_abs && !measure) g_abs->push_back({k, &n, origin_x, y});
            i++;
            continue;
        }
        if (inline_level(*k)) {
            std::vector<Node*> run;
            size_t j = i;
            while (j < kids.size()) {
                Node* kk = kids[j].get();
                if (!kk->is_text && (!kk->st || is_none(*kk))) {
                    j++;
                    continue;
                }
                if (!kk->is_text && is_abs(*kk)) {
                    if (g_abs && !measure) g_abs->push_back({kk, &n, origin_x, y});
                    j++;
                    continue;
                }
                if (!inline_level(*kk)) break;
                run.push_back(kk);
                j++;
            }
            bool all_blank = true;
            for (Node* r : run)
                if (!r->is_text || !blank(r->text) || r->parent->st->pre) all_blank = false;
            if (!all_blank) {
                if (pending_mb != kNA) {
                    y += pending_mb;
                    pending_mb = kNA;
                }
                int h = 0, w = 0, fb = kNA;
                layout_inline_run(n, run, cw, origin_x, y, measure, &h, &w, &fb);
                mw = std::max(mw, w);
                if (*first_base == kNA && fb != kNA) *first_base = fb;
                y += h;
            }
            i = j;
            continue;
        }
        Node& c = *k;
        int ml = margin_side(c, 3, cw), mr = margin_side(c, 1, cw), mt = eff_mt(c, cw), mb = margin_side(c, 2, cw);
        if (!measure && &c == first_flow_child(n) && collapses_top(n)) mt = 0;
        if (measure) {
            mw = std::max(mw, max_content(c) + ml + mr);
            i++;
            continue;
        }
        Cons cc;
        cc.avail = std::max(0, cw - ml - mr);
        cc.cbw = cw;
        layout_node(c, cc);
        if (c.esc_mb != 0) mb = combine_margins(mb, c.esc_mb);
        int gap;
        if (pending_mb == kNA) gap = mt;
        else if (pending_mb >= 0 && mt >= 0) gap = std::max(pending_mb, mt);
        else if (pending_mb < 0 && mt < 0) gap = std::min(pending_mb, mt);
        else gap = pending_mb + mt;
        y += gap;
        int x = origin_x + ml;
        bool al = margin_auto(c, 3), ar = margin_auto(c, 1);
        int freew = cw - c.lw - (al ? 0 : ml) - (ar ? 0 : mr);
        if (al && ar) x = origin_x + lu_div(std::max(0, freew), 2);
        else if (al) x = origin_x + std::max(0, freew);
        c.lx = x;
        c.ly = y;
        if (c.st->pos == Pos::Relative) {
            int l = resolve(c.st->off[3], cw), r = resolve(c.st->off[1], cw), t = resolve(c.st->off[0], kNA),
                b = resolve(c.st->off[2], kNA);
            if (l != kNA) c.lx += l;
            else if (r != kNA) c.lx -= r;
            if (t != kNA) c.ly += t;
            else if (b != kNA) c.ly -= b;
        }
        if (*first_base == kNA && c.lbase != kNA) *first_base = y + c.lbase;
        y += c.lh;
        pending_mb = mb;
        mw = std::max(mw, c.lw + ml + mr);
        i++;
    }
    if (pending_mb != kNA) {
        if (trail) *trail = pending_mb;
        else y += pending_mb;
    }
    *maxw = mw;
    return y - origin_y;
}

int max_content(Node& n) {
    if (n.max_content_cache != kNA) return n.max_content_cache;
    if (!n.st || n.st->display == Disp::None) return n.max_content_cache = 0;
    const Style& s = *n.st;
    int hpb = 0;
    for (int i : {1, 3}) hpb += pad_side(n, i, 0) + s.border[i];
    int result;
    if (s.width.k == 1) {
        result = s.border_box ? s.width.px : s.width.px + hpb;
    } else {
        int mw = 0;
        if (s.display == Disp::Flex || s.display == Disp::InlineFlex) {
            wrap_anonymous(n);
            int sum = 0, gaps = 0, cnt = 0;
            for (auto& k : n.kids) {
                if (k->is_text) {
                    if (blank(k->text)) continue;
                    Style& ps = *n.st;
                    std::vector<Tok> tk;
                    tokenise_text(k.get(), tk);
                    int w = 0;
                    for (auto& t : tk) w += (t.kind == 5 ? 0 : text_w(ps, t.s));
                    if (s.column) mw = std::max(mw, w);
                    else sum += w;
                    cnt++;
                    continue;
                }
                if (!k->st || is_none(*k) || is_abs(*k)) continue;
                int w = max_content(*k) + margin_side(*k, 1, 0) + margin_side(*k, 3, 0);
                if (s.column) mw = std::max(mw, w);
                else sum += w;
                cnt++;
            }
            if (!s.column && cnt > 1) gaps = (cnt - 1) * s.col_gap;
            result = (s.column ? mw : sum + gaps) + hpb;
        } else {
            int fb = kNA;
            int w = block_content(n, kHuge, 0, 0, true, &mw, &fb);
            (void)w;
            result = mw + hpb;
        }
    }
    result = clamp_w(n, result, 0, hpb);
    return n.max_content_cache = result;
}

int min_content(Node& n) {
    if (n.min_content_cache != kNA) return n.min_content_cache;
    if (!n.st || n.st->display == Disp::None) return n.min_content_cache = 0;
    const Style& s = *n.st;
    int hpb = 0;
    for (int i : {1, 3}) hpb += pad_side(n, i, 0) + s.border[i];
    int result;
    if (s.width.k == 1) {
        result = s.border_box ? s.width.px : s.width.px + hpb;
    } else if (s.nowrap) {
        result = max_content(n);
    } else {
        int mw = 0;
        bool flex = s.display == Disp::Flex || s.display == Disp::InlineFlex;
        if (flex) wrap_anonymous(n);
        int sum = 0, cnt = 0;
        std::vector<Tok> toks;
        for (auto& k : n.kids) {
            if (!k->is_text && (!k->st || is_none(*k) || is_abs(*k))) continue;
            if (!flex && (k->is_text || inline_level(*k))) {
                flatten_one(k.get(), toks, 0, true);
                continue;
            }
            if (k->is_text) continue;
            int w = min_content(*k) + margin_side(*k, 1, 0) + margin_side(*k, 3, 0);
            if (flex && !s.column && !s.wrap) sum += w;
            else mw = std::max(mw, w);
            cnt++;
        }
        for (auto& t : toks) {
            if (t.kind == 0) mw = std::max(mw, text_w(*t.owner->st, t.s));
            else if (t.kind == 2) mw = std::max(mw, min_content(*t.atom) + margin_side(*t.atom, 1, 0) + margin_side(*t.atom, 3, 0));
        }
        if (flex && !s.column && !s.wrap) mw = sum + (cnt > 1 ? (cnt - 1) * s.col_gap : 0);
        result = mw + hpb;
    }
    result = clamp_w(n, result, 0, hpb);
    return n.min_content_cache = result;
}

struct FlexItem {
    Node* n;
    int base = 0, hyp = 0, main = 0, cross = 0;
    int mm0 = 0, mm1 = 0, cm0 = 0, cm1 = 0;
    bool am0 = false, am1 = false, acm0 = false, acm1 = false;
    int min_main = 0, max_main = INT_MAX;
    bool frozen = false;
    int pos = 0, cpos = 0;
};

Align item_align(const Node& item, const Style& cont) {
    Align a = item.st->align_self;
    if (a == Align::Auto || a == Align::Normal) a = cont.align_items;
    if (a == Align::Normal || a == Align::Auto) a = Align::Stretch;
    return a;
}

void wrap_anonymous(Node& n) {
    for (size_t i = 0; i < n.kids.size(); i++) {
        Node* k = n.kids[i].get();
        if (k->is_text) {
            if (blank(k->text)) {
                n.kids.erase(n.kids.begin() + i);
                i--;
                continue;
            }
            auto w = std::make_unique<Node>();
            w->tag = "anon";
            w->anonymous = true;
            w->parent = &n;
            w->css["display"] = "block";
            auto tn = std::move(n.kids[i]);
            tn->parent = w.get();
            w->kids.push_back(std::move(tn));
            compute_style(*w);
            w->kids[0]->st = w->st;
            n.kids[i] = std::move(w);
        }
    }
}

void layout_flex(Node& n, int cw, int def_h, bool h_indef, int* content_h, int* first_base) {
    const Style& s = *n.st;
    bool col = s.column;
    std::vector<FlexItem> items;
    for (auto& k : n.kids) {
        if (k->is_text || !k->st || is_none(*k)) continue;
        if (is_abs(*k)) {
            if (g_abs) g_abs->push_back({k.get(), &n, 0, 0});
            continue;
        }
        FlexItem it;
        it.n = k.get();
        items.push_back(it);
    }
    std::stable_sort(items.begin(), items.end(), [](const FlexItem& a, const FlexItem& b) { return a.n->st->order < b.n->st->order; });
    int main_gap = col ? s.row_gap : s.col_gap, cross_gap = col ? s.col_gap : s.row_gap;
    int cross_def = col ? cw : def_h;
    int main_def = col ? def_h : cw;

    for (auto& it : items) {
        Node& c = *it.n;
        const Style& cs = *c.st;
        int ml = margin_side(c, 3, cw), mr = margin_side(c, 1, cw), mt = margin_side(c, 0, cw), mb = margin_side(c, 2, cw);
        it.am0 = col ? margin_auto(c, 0) : margin_auto(c, 3);
        it.am1 = col ? margin_auto(c, 2) : margin_auto(c, 1);
        it.acm0 = col ? margin_auto(c, 3) : margin_auto(c, 0);
        it.acm1 = col ? margin_auto(c, 1) : margin_auto(c, 2);
        it.mm0 = it.am0 ? 0 : (col ? mt : ml);
        it.mm1 = it.am1 ? 0 : (col ? mb : mr);
        it.cm0 = it.acm0 ? 0 : (col ? ml : mt);
        it.cm1 = it.acm1 ? 0 : (col ? mr : mb);
        int hpb = pad_side(c, 1, cw) + pad_side(c, 3, cw) + cs.border[1] + cs.border[3];
        int vpb = pad_side(c, 0, cw) + pad_side(c, 2, cw) + cs.border[0] + cs.border[2];
        int mpb = col ? vpb : hpb;
        Len mainlen = col ? cs.height : cs.width;
        int basis = resolve(cs.basis, main_def);
        bool pct_content = col && h_indef && cs.basis.k == 2;
        if (pct_content) basis = kNA;
        int base;
        if (basis != kNA) base = cs.border_box ? basis : basis + mpb;
        else {
            int v = pct_content ? kNA : resolve(mainlen, main_def);
            if (v != kNA) base = cs.border_box ? v : v + mpb;
            else if (!col) base = max_content(c);
            else {
                Cons cc;
                cc.avail = std::max(0, cw - ml - mr);
                cc.cbw = cw;
                if (item_align(c, s) != Align::Stretch) cc.shrink = true;
                layout_node(c, cc);
                base = c.lh;
            }
        }
        base = std::max(base, mpb);
        int mnv = resolve(col ? cs.minh : cs.minw, main_def), mxv = resolve(col ? cs.maxh : cs.maxw, main_def);
        it.min_main = mnv == kNA ? 0 : (cs.border_box ? mnv : mnv + mpb);
        if (mnv == kNA && !col && !cs.overflow_clip && cs.minw.is_auto()) {
            int content_min = min_content(c);
            int spec = resolve(cs.width, main_def);
            if (spec != kNA) content_min = std::min(content_min, cs.border_box ? spec : spec + mpb);
            it.min_main = content_min;
        }
        it.max_main = mxv == kNA ? INT_MAX : (cs.border_box ? mxv : mxv + mpb);
        it.base = base;
        it.hyp = std::min(std::max(base, it.min_main), it.max_main);
    }

    std::vector<std::vector<size_t>> flines;
    {
        std::vector<size_t> cur;
        int sum = 0;
        for (size_t i = 0; i < items.size(); i++) {
            int outer = items[i].hyp + items[i].mm0 + items[i].mm1;
            if (s.wrap && main_def != kNA && !cur.empty() && sum + main_gap + outer > main_def) {
                flines.push_back(cur);
                cur.clear();
                sum = 0;
            }
            sum += (cur.empty() ? 0 : main_gap) + outer;
            cur.push_back(i);
        }
        if (!cur.empty()) flines.push_back(cur);
    }

    std::vector<int> line_cross(flines.size(), 0), line_main_used(flines.size(), 0);
    int total_cross = 0;
    for (size_t li = 0; li < flines.size(); li++) {
        auto& L = flines[li];
        int outer_sum = 0, hyp_sum = 0;
        for (size_t i : L) {
            outer_sum += items[i].mm0 + items[i].mm1;
            hyp_sum += items[i].hyp;
        }
        outer_sum += main_gap * (int)(L.size() - 1);
        int avail_main = main_def;
        for (size_t i : L) {
            items[i].main = items[i].hyp;
            items[i].frozen = false;
        }
        if (avail_main != kNA) {
            bool growing = (long long)hyp_sum + outer_sum < avail_main;
            for (size_t i : L) {
                FlexItem& it = items[i];
                float factor = growing ? it.n->st->grow : it.n->st->shrink;
                if (factor == 0.f || (growing && it.base > it.hyp) || (!growing && it.base < it.hyp)) {
                    it.main = it.hyp;
                    it.frozen = true;
                }
            }
            for (int iter = 0; iter < 12; iter++) {
                long long used = outer_sum;
                double denom = 0;
                bool any = false;
                for (size_t i : L) {
                    FlexItem& it = items[i];
                    used += it.frozen ? it.main : it.base;
                    if (!it.frozen) {
                        any = true;
                        denom += growing ? it.n->st->grow : (double)it.n->st->shrink * it.base;
                    }
                }
                if (!any) break;
                long long free_space = (long long)avail_main - used;
                long long viol = 0;
                std::vector<int> ti(items.size(), 0), cl(items.size(), 0);
                for (size_t i : L) {
                    FlexItem& it = items[i];
                    if (it.frozen) continue;
                    double t = it.base;
                    if (denom > 0) t += free_space * ((growing ? it.n->st->grow : (double)it.n->st->shrink * it.base) / denom);
                    ti[i] = (int)std::floor(t);
                    cl[i] = std::max(0, std::min(std::max(ti[i], it.min_main), it.max_main));
                    viol += cl[i] - ti[i];
                }
                for (size_t i : L) {
                    FlexItem& it = items[i];
                    if (it.frozen) continue;
                    if (viol == 0 || (viol > 0 && cl[i] > ti[i]) || (viol < 0 && cl[i] < ti[i])) it.frozen = true;
                    it.main = cl[i];
                }
                if (viol == 0) break;
            }
        }
        long long used_main = outer_sum;
        for (size_t i : L) used_main += items[i].main;
        line_main_used[li] = (int)used_main;

        int lc = 0;
        for (size_t i : L) {
            FlexItem& it = items[i];
            Node& c = *it.n;
            int ml = margin_side(c, 3, cw), mr = margin_side(c, 1, cw);
            Cons cc;
            cc.cbw = cw;
            if (!col) {
                cc.fw = it.main;
                cc.cbh = def_h;
                cc.avail = it.main;
                Len hl = c.st->height;
                bool stretch = item_align(c, s) == Align::Stretch && hl.is_auto();
                if (stretch && cross_def != kNA && !s.wrap) {
                    cc.fh = std::max(0, cross_def - it.cm0 - it.cm1);
                }
                layout_node(c, cc);
                it.cross = c.lh;
            } else {
                cc.fh = it.main;
                bool stretch = item_align(c, s) == Align::Stretch && c.st->width.is_auto();
                cc.avail = std::max(0, cw - ml - mr);
                if (stretch) cc.fw = std::max(0, cw - ml - mr);
                else cc.shrink = true;
                cc.cbh = def_h;
                layout_node(c, cc);
                it.cross = c.lw;
            }
            lc = std::max(lc, it.cross + it.cm0 + it.cm1);
        }
        if (!s.wrap && flines.size() == 1 && cross_def != kNA) lc = cross_def;
        line_cross[li] = lc;
        total_cross += lc;
    }
    total_cross += cross_gap * (int)(flines.size() > 0 ? flines.size() - 1 : 0);

    int cross_total_def = cross_def;
    int extra_cross = 0;
    if (cross_total_def != kNA && flines.size() > 1 && total_cross < cross_total_def) {
        extra_cross = (cross_total_def - total_cross) / (int)flines.size();
        for (auto& lc : line_cross) lc += extra_cross;
    }
    int cross_cursor = 0;
    int first_base_local = kNA;
    for (size_t li = 0; li < flines.size(); li++) {
        auto& L = flines[li];
        int lc = line_cross[li];
        int free_main = main_def == kNA ? 0 : main_def - line_main_used[li];
        int auto_cnt = 0;
        for (size_t i : L) auto_cnt += (items[i].am0 ? 1 : 0) + (items[i].am1 ? 1 : 0);
        int lead = 0, between = main_gap;
        if (auto_cnt > 0 && free_main > 0) {
            int per = free_main / auto_cnt;
            for (size_t i : L) {
                if (items[i].am0) items[i].mm0 = per;
                if (items[i].am1) items[i].mm1 = per;
            }
        } else if (free_main != 0) {
            int cnt = (int)L.size();
            switch (s.justify) {
                case Just::Center: lead = lu_div(free_main, 2); break;
                case Just::End: lead = free_main; break;
                case Just::Between:
                    if (free_main > 0 && cnt > 1) between = main_gap + free_main / (cnt - 1);
                    break;
                case Just::Around:
                    if (free_main > 0) {
                        int each = free_main / cnt;
                        lead = lu_div(each, 2);
                        between = main_gap + each;
                    }
                    break;
                case Just::Evenly:
                    if (free_main > 0) {
                        int each = free_main / (cnt + 1);
                        lead = each;
                        between = main_gap + each;
                    }
                    break;
                default: break;
            }
        }
        if (s.reverse && free_main > 0 && (s.justify == Just::Normal || s.justify == Just::Start)) lead = free_main;
        int p = lead;
        std::vector<size_t> order = L;
        if (s.reverse) std::reverse(order.begin(), order.end());
        for (size_t i : order) {
            FlexItem& it = items[i];
            Node& c = *it.n;
            p += it.mm0;
            it.pos = p;
            p += it.main + it.mm1 + between;
            Align a = item_align(c, s);
            if (a == Align::Stretch && ((col && c.st->width.is_auto()) || (!col && c.st->height.is_auto())) &&
                (it.cross + it.cm0 + it.cm1 != lc) && !it.acm0 && !it.acm1) {
                Cons cc;
                cc.cbw = cw;
                int ml = margin_side(c, 3, cw), mr = margin_side(c, 1, cw);
                if (!col) {
                    cc.fw = it.main;
                    cc.avail = it.main;
                    cc.fh = std::max(0, lc - it.cm0 - it.cm1);
                } else {
                    cc.fh = it.main;
                    cc.fw = std::max(0, lc - ml - mr);
                    cc.avail = cc.fw;
                }
                cc.cbh = def_h;
                c.max_content_cache = kNA;
                layout_node(c, cc);
                it.cross = col ? c.lw : c.lh;
            }
            int space = lc - it.cross - it.cm0 - it.cm1;
            int cp = it.cm0;
            if (it.acm0 && it.acm1) cp += lu_div(std::max(0, space), 2);
            else if (it.acm0) cp += std::max(0, space);
            else if (!it.acm1) {
                switch (a) {
                    case Align::Center: cp += lu_div(space, 2); break;
                    case Align::End: cp += space; break;
                    default: break;
                }
            }
            it.cpos = cp;
        }
        {
            int maxb = 0;
            bool anyb = false;
            for (size_t i : L) {
                FlexItem& it = items[i];
                if (item_align(*it.n, s) == Align::Baseline && !col) {
                    int b = it.n->lbase == kNA ? it.n->lh : it.n->lbase;
                    maxb = std::max(maxb, it.cm0 + b);
                    anyb = true;
                }
            }
            if (anyb)
                for (size_t i : L) {
                    FlexItem& it = items[i];
                    if (item_align(*it.n, s) == Align::Baseline && !col) {
                        int b = it.n->lbase == kNA ? it.n->lh : it.n->lbase;
                        it.cpos = maxb - b;
                    }
                }
        }
        for (size_t i : L) {
            FlexItem& it = items[i];
            Node& c = *it.n;
            int ox = s.border[3] + pad_side(n, 3, cw), oy = s.border[0] + pad_side(n, 0, cw);
            if (!col) {
                c.lx = ox + it.pos;
                c.ly = oy + cross_cursor + it.cpos;
            } else {
                c.ly = oy + it.pos;
                c.lx = ox + cross_cursor + it.cpos;
            }
            if (c.st->pos == Pos::Relative) {
                int l = resolve(c.st->off[3], cw), r = resolve(c.st->off[1], cw), t = resolve(c.st->off[0], kNA),
                    b = resolve(c.st->off[2], kNA);
                if (l != kNA) c.lx += l;
                else if (r != kNA) c.lx -= r;
                if (t != kNA) c.ly += t;
                else if (b != kNA) c.ly -= b;
            }
            if (first_base_local == kNA && li == 0 && c.lbase != kNA) first_base_local = c.ly + c.lbase;
        }
        cross_cursor += lc + cross_gap;
    }
    cross_total_def = cross_total_def == kNA ? kNA : cross_total_def;
    int cross_used = flines.empty() ? 0 : cross_cursor - cross_gap;
    int main_used = 0;
    for (size_t li = 0; li < flines.size(); li++) main_used = std::max(main_used, line_main_used[li]);
    if (col) *content_h = def_h != kNA ? def_h : main_used;
    else *content_h = def_h != kNA ? def_h : cross_used;
    *first_base = first_base_local;
}

struct Track {
    int min = 0, max = 0;
    float fr = 0.f;
    bool min_auto = false, max_auto = false;
};

bool flexible_tracks(const std::string& s) {
    return s.find("fr") != std::string::npos || s.find("minmax(") != std::string::npos || s.find("repeat(") != std::string::npos ||
           s.find('%') != std::string::npos;
}

std::vector<std::string> track_tokens(const std::string& s) {
    std::vector<std::string> out;
    std::string cur;
    int depth = 0;
    for (char ch : s) {
        if (ch == '(') depth++;
        if (ch == ')') depth--;
        if ((ch == ' ' || ch == ',') && depth == 0) {
            if (!cur.empty()) out.push_back(cur);
            cur.clear();
            continue;
        }
        cur += ch;
    }
    if (!cur.empty()) out.push_back(cur);
    return out;
}

void track_breadth(const std::string& t, int ref, int& lu, float& fr, bool& is_auto) {
    is_auto = false;
    fr = 0.f;
    lu = 0;
    char* e = nullptr;
    float v = std::strtof(t.c_str(), &e);
    if (t == "auto" || t == "min-content" || t == "max-content" || e == t.c_str()) {
        is_auto = true;
        return;
    }
    std::string unit = e;
    if (unit == "fr") fr = v;
    else if (unit == "%") lu = ref == kNA ? 0 : (int)((long long)ref * v / 100.f);
    else lu = to_lu(v);
}

Track one_track(const std::string& t, int ref) {
    Track tr;
    if (t.rfind("minmax(", 0) == 0) {
        std::vector<std::string> a = track_tokens(t.substr(7, t.size() - 8));
        if (a.size() == 2) {
            float f0;
            track_breadth(a[0], ref, tr.min, f0, tr.min_auto);
            if (f0 > 0.f) tr.min_auto = true;
            track_breadth(a[1], ref, tr.max, tr.fr, tr.max_auto);
            return tr;
        }
    }
    track_breadth(t, ref, tr.min, tr.fr, tr.min_auto);
    tr.max = tr.min;
    tr.max_auto = tr.min_auto;
    if (tr.fr > 0.f) tr.min_auto = true;
    return tr;
}

std::vector<Track> parse_track_list(const std::string& s, int ref, int gap) {
    std::vector<Track> out;
    for (const std::string& t : track_tokens(s)) {
        if (t.rfind("repeat(", 0) == 0) {
            std::vector<std::string> a = track_tokens(t.substr(7, t.size() - 8));
            if (a.size() < 2) continue;
            std::vector<Track> unit;
            for (size_t i = 1; i < a.size(); i++) unit.push_back(one_track(a[i], ref));
            int count = std::atoi(a[0].c_str());
            if (a[0] == "auto-fill" || a[0] == "auto-fit") {
                long long per = 0;
                for (auto& u : unit) per += (u.min_auto ? (u.max_auto || u.fr > 0.f ? 0 : u.max) : u.min) + gap;
                count = per > 0 && ref != kNA ? (int)std::max(1LL, ((long long)ref + gap) / per) : 1;
            }
            for (int k = 0; k < std::max(0, count); k++) out.insert(out.end(), unit.begin(), unit.end());
            continue;
        }
        out.push_back(one_track(t, ref));
    }
    return out;
}

std::vector<int> size_tracks(const std::vector<Track>& tr, int avail, int gap, const std::function<int(size_t, bool)>& content) {
    std::vector<int> size(tr.size(), 0);
    float total_fr = 0.f;
    long long fixed = (long long)gap * (tr.empty() ? 0 : (long long)tr.size() - 1);
    for (size_t i = 0; i < tr.size(); i++) {
        const Track& t = tr[i];
        int base = t.min_auto ? content(i, t.fr > 0.f) : t.min;
        if (t.fr > 0.f) {
            size[i] = base;
            total_fr += t.fr;
            continue;
        }
        int mx = t.max_auto ? std::max(base, content(i, false)) : std::max(base, t.max);
        size[i] = mx;
        fixed += mx;
    }
    if (total_fr > 0.f && avail != kNA) {
        std::vector<bool> inflex(tr.size(), false);
        for (int iter = 0; iter < 8; iter++) {
            long long free_space = (long long)avail - fixed;
            float flex = 0.f;
            for (size_t i = 0; i < tr.size(); i++) {
                if (tr[i].fr <= 0.f) continue;
                if (inflex[i]) free_space -= size[i];
                else flex += tr[i].fr;
            }
            if (flex <= 0.f) break;
            double unit = std::max(0.0, (double)free_space / std::max(1.f, flex));
            bool again = false;
            for (size_t i = 0; i < tr.size(); i++) {
                if (tr[i].fr <= 0.f || inflex[i]) continue;
                int want = (int)(unit * tr[i].fr);
                int base = tr[i].min_auto ? content(i, true) : tr[i].min;
                if (base > want) {
                    inflex[i] = true;
                    size[i] = base;
                    again = true;
                }
            }
            if (!again) {
                for (size_t i = 0; i < tr.size(); i++)
                    if (tr[i].fr > 0.f && !inflex[i]) size[i] = (int)(unit * tr[i].fr);
                break;
            }
        }
    }
    return size;
}

void layout_grid(Node& n, int cw, int def_h, int* content_h, int* first_base) {
    const Style& s = *n.st;
    std::vector<Node*> items;
    for (auto& k : n.kids) {
        if (k->is_text || !k->st || is_none(*k)) continue;
        if (is_abs(*k)) {
            if (g_abs) g_abs->push_back({k.get(), &n, 0, 0});
            continue;
        }
        items.push_back(k.get());
    }
    std::vector<int> cols = s.grid_cols;
    const std::string gtc = n.get("grid-template-columns");
    const bool flex_cols = flexible_tracks(gtc);
    if (flex_cols) {
        std::vector<Track> tr = parse_track_list(gtc, cw, s.col_gap);
        size_t nt = tr.size();
        cols = size_tracks(tr, cw, s.col_gap, [&](size_t i, bool minimum) {
            int need = 0;
            for (size_t j = i; nt && j < items.size(); j += nt) {
                Node& it = *items[j];
                if (minimum && it.st->overflow_clip) continue;
                need = std::max(need, (minimum ? min_content(it) : max_content(it)) + margin_side(it, 1, cw) + margin_side(it, 3, cw));
            }
            return need;
        });
    }
    if (cols.empty()) cols.push_back(cw);
    int ox = s.border[3] + pad_side(n, 3, cw), oy = s.border[0] + pad_side(n, 0, cw);
    if (!flex_cols) {
        int autos = 0;
        long long fixed = 0;
        for (size_t i = 0; i < cols.size(); i++) {
            if (cols[i] == kNA) {
                int need = 0;
                for (size_t j = i; j < items.size(); j += cols.size())
                    need = std::max(need, max_content(*items[j]) + margin_side(*items[j], 1, cw) + margin_side(*items[j], 3, cw));
                cols[i] = need;
                autos++;
            }
            fixed += cols[i];
        }
        long long free_space = (long long)cw - fixed - (long long)s.col_gap * (cols.size() - 1);
        if (autos > 0 && free_space > 0 && s.justify == Just::Normal) {
            int per = (int)(free_space / autos);
            for (size_t i = 0; i < cols.size(); i++)
                if (s.grid_cols[i] == kNA) cols[i] += per;
        }
    }
    int cgap = s.col_gap;
    int x = 0;
    {
        long long used = 0;
        for (int c : cols) used += c;
        used += (long long)s.col_gap * ((long long)cols.size() - 1);
        long long fs = (long long)cw - used;
        int cnt = (int)cols.size();
        if (fs > 0) {
            int f = (int)fs;
            switch (s.justify) {
                case Just::Center: x = lu_div(f, 2); break;
                case Just::End: x = f; break;
                case Just::Between:
                    if (cnt > 1) cgap += f / (cnt - 1);
                    break;
                case Just::Around: {
                    int each = f / cnt;
                    x = lu_div(each, 2);
                    cgap += each;
                    break;
                }
                case Just::Evenly: {
                    int each = f / (cnt + 1);
                    x = each;
                    cgap += each;
                    break;
                }
                default: break;
            }
        }
    }
    std::vector<int> col_x;
    for (size_t i = 0; i < cols.size(); i++) {
        col_x.push_back(x);
        x += cols[i] + cgap;
    }
    std::vector<int> rows = parse_tracks(n.get("grid-template-rows"));
    auto span_of = [](const std::string& v) {
        size_t p = v.find("span");
        return p == std::string::npos ? 1 : std::max(1, std::atoi(v.c_str() + p + 4));
    };
    struct Cell {
        Node* n;
        size_t r, c, rs, cs;
    };
    std::vector<Cell> cells;
    std::vector<std::vector<bool>> used;
    for (Node* it : items) {
        size_t rs = (size_t)span_of(it->get("grid-row")), csp = std::min((size_t)span_of(it->get("grid-column")), cols.size());
        size_t r = 0, c = 0;
        for (;; r++) {
            if (used.size() < r + rs) used.resize(r + rs, std::vector<bool>(cols.size(), false));
            bool placed = false;
            for (c = 0; c + csp <= cols.size() && !placed; c++) {
                bool free_ = true;
                for (size_t rr = r; rr < r + rs && free_; rr++)
                    for (size_t cc = c; cc < c + csp; cc++)
                        if (used[rr][cc]) free_ = false;
                if (free_) {
                    for (size_t rr = r; rr < r + rs; rr++)
                        for (size_t cc = c; cc < c + csp; cc++) used[rr][cc] = true;
                    placed = true;
                    break;
                }
            }
            if (placed) break;
        }
        cells.push_back({it, r, c, rs, csp});
    }
    size_t nrows = used.size();
    std::vector<int> row_h(nrows, 0);
    for (auto& ce : cells) {
        Node& it = *ce.n;
        int ml = margin_side(it, 3, cw), mr = margin_side(it, 1, cw);
        int tw = 0;
        for (size_t k = ce.c; k < ce.c + ce.cs; k++) tw += cols[k] + (k > ce.c ? cgap : 0);
        Cons cc;
        cc.avail = std::max(0, tw - ml - mr);
        cc.cbw = cw;
        layout_node(it, cc);
        if (ce.rs == 1) row_h[ce.r] = std::max(row_h[ce.r], it.lh + margin_side(it, 0, cw) + margin_side(it, 2, cw));
    }
    const std::string gtr = n.get("grid-template-rows");
    if (flexible_tracks(gtr)) {
        std::vector<Track> tr = parse_track_list(gtr, def_h, s.row_gap);
        if (tr.size() > nrows) {
            nrows = tr.size();
            row_h.resize(nrows, 0);
        }
        std::vector<int> content_rows = row_h;
        tr.resize(nrows, Track{0, 0, 0.f, true, true});
        row_h = size_tracks(tr, def_h, s.row_gap, [&](size_t i, bool) { return content_rows[i]; });
        rows.clear();
    }
    for (size_t r = 0; r < nrows; r++)
        if (r < rows.size() && rows[r] != kNA) row_h[r] = rows[r];
    std::vector<int> row_y(nrows, 0);
    int y = 0;
    for (size_t r = 0; r < nrows; r++) {
        row_y[r] = y;
        y += row_h[r] + s.row_gap;
    }
    if (nrows) y -= s.row_gap;
    for (auto& ce : cells) {
        ce.n->lx = ox + col_x[ce.c] + margin_side(*ce.n, 3, cw);
        int rh = 0;
        for (size_t k = ce.r; k < ce.r + ce.rs && k < nrows; k++) rh += row_h[k] + (k > ce.r ? s.row_gap : 0);
        Align a = item_align(*ce.n, s);
        if (a == Align::Stretch && ce.n->st && ce.n->st->height.is_auto()) {
            int target = rh - margin_side(*ce.n, 0, cw) - margin_side(*ce.n, 2, cw);
            if (target > ce.n->lh) {
                int tw = 0;
                for (size_t k = ce.c; k < ce.c + ce.cs; k++) tw += cols[k] + (k > ce.c ? cgap : 0);
                Cons cc;
                cc.avail = std::max(0, tw - margin_side(*ce.n, 3, cw) - margin_side(*ce.n, 1, cw));
                cc.cbw = cw;
                cc.fh = target;
                cc.fh_definite = true;
                layout_node(*ce.n, cc);
            }
        }
        int outer = ce.n->lh + margin_side(*ce.n, 0, cw) + margin_side(*ce.n, 2, cw);
        int off = 0;
        if (a == Align::Center) off = lu_div(rh - outer, 2);
        else if (a == Align::End) off = rh - outer;
        ce.n->ly = oy + row_y[ce.r] + off + margin_side(*ce.n, 0, cw);
    }
    *content_h = def_h != kNA ? def_h : y;
    *first_base = kNA;
}

void layout_node(Node& n, const Cons& c) {
    if (!n.st) compute_style(n);
    const Style& s = *n.st;
    n.frags.clear();
    n.lbase = kNA;
    n.esc_mb = 0;
    if (s.display == Disp::None) {
        n.lw = n.lh = 0;
        return;
    }
    int cbw = c.cbw == kNA ? 0 : c.cbw;
    int pl = pad_side(n, 3, cbw), pr = pad_side(n, 1, cbw), pt = pad_side(n, 0, cbw), pb = pad_side(n, 2, cbw);
    int hpb = pl + pr + s.border[1] + s.border[3];
    int vpb = pt + pb + s.border[0] + s.border[2];
    int w;
    if (c.fw != kNA) w = c.fw;
    else {
        int v = resolve(s.width, c.cbw);
        if (v != kNA) w = s.border_box ? v : v + hpb;
        else if (c.shrink) w = std::min(c.avail == kNA ? kHuge : c.avail, max_content(n));
        else w = c.avail == kNA ? 0 : c.avail;
        w = clamp_w(n, w, c.cbw, hpb);
    }
    w = std::max(w, hpb);
    int content_w = w - hpb;
    int def_h = kNA;
    if (c.fh != kNA) def_h = std::max(0, c.fh - vpb);
    else {
        int v = resolve(s.height, c.cbh);
        if (v != kNA) def_h = std::max(0, s.border_box ? v - vpb : v);
        else if (s.aspect > 0.f) {
            int bh = clamp_h(n, (int)std::lround(w / s.aspect), c.cbh, vpb);
            def_h = std::max(0, bh - vpb);
        }
    }
    int ox = s.border[3] + pl, oy = s.border[0] + pt;
    int ch = 0, fb = kNA;
    bool leaf = n.tag == "svg" || n.tag == "img" || n.tag == "input" || n.tag == "canvas" || n.tag == "textarea" || n.tag == "select";
    if (leaf) ch = def_h == kNA ? 0 : def_h;
    else if (s.display == Disp::Flex || s.display == Disp::InlineFlex) {
        wrap_anonymous(n);
        layout_flex(n, content_w, def_h, c.fh != kNA && !c.fh_definite, &ch, &fb);
    } else if (s.display == Disp::Grid) {
        wrap_anonymous(n);
        layout_grid(n, content_w, def_h, &ch, &fb);
    } else {
        int mw = 0;
        int first = kNA;
        int trail = 0;
        ch = block_content(n, content_w, ox, oy, false, &mw, &first, &trail);
        if (trail != 0 && collapses_bottom(n) && (def_h == kNA || std::abs(def_h - ch) <= 1)) n.esc_mb = trail;
        else ch += trail;
        fb = first;
        if (fb != kNA) fb -= 0;
    }
    int h;
    if (def_h != kNA) h = def_h + vpb;
    else h = ch + vpb;
    h = clamp_h(n, h, c.cbh, vpb);
    n.lw = w;
    n.lh = h;
    if (fb != kNA) {
        n.lbase = fb;
        if (s.display == Disp::Flex || s.display == Disp::InlineFlex || s.display == Disp::Grid) n.lbase = fb;
    }
    if (s.display != Disp::Flex && s.display != Disp::InlineFlex && s.display != Disp::Grid && !leaf) {
        if (n.lbase != kNA) n.lbase = n.lbase;
    }
}

void finalize(Node& n, int px, int py) {
    n.abs_x = px + n.lx;
    n.abs_y = py + n.ly;
    n.box = {to_css(n.abs_x), to_css(n.abs_y), to_css(n.lw), to_css(n.lh)};
    n.lines.clear();
    for (auto& k : n.kids) {
        if (k->is_text || !k->st) continue;
        if (k->st->display == Disp::None) {
            k->lw = k->lh = 0;
            continue;
        }
        finalize(*k, n.abs_x, n.abs_y);
    }
    for (auto& f : n.frags) {
        TextLine tl;
        tl.box = {to_css(n.abs_x + f.x), to_css(n.abs_y + f.y), to_css(f.w), to_css(f.h)};
        tl.baseline = to_css(n.abs_y + f.baseline);
        {
            size_t a = 0, b = f.text.size();
            auto is_ws = [&](size_t i, size_t& len) {
                unsigned char ch = (unsigned char)f.text[i];
                if (ch == ' ' || ch == '\t' || ch == '\n') { len = 1; return true; }
                if (ch == 0xC2 && i + 1 < f.text.size() && (unsigned char)f.text[i + 1] == 0xA0) { len = 2; return true; }
                return false;
            };
            size_t len = 0;
            while (a < b && is_ws(a, len)) a += len;
            while (b > a) {
                if (f.text[b - 1] == ' ') b--;
                else if (b >= a + 2 && (unsigned char)f.text[b - 2] == 0xC2 && (unsigned char)f.text[b - 1] == 0xA0) b -= 2;
                else break;
            }
            if (f.owner->anonymous) a = 0;
            if (a > 0 || b < f.text.size()) {
                const Style& os = *f.owner->st;
                int lw_ = text_w(os, f.text.substr(0, a));
                int tw_ = text_w(os, f.text.substr(0, b));
                tl.trim_x = to_css(lw_);
                tl.trim_w = to_css(tw_ - lw_);
            } else {
                tl.trim_x = 0;
                tl.trim_w = tl.box.w;
            }
        }
        tl.text = f.text;
        tl.owner = f.owner;
        Node* ow = f.owner;
        while (ow->anonymous && ow->parent) ow = ow->parent;
        tl.owner = ow;
        ow->lines.push_back(tl);
    }
}

void inline_boxes(Node& n) {
    for (auto& k : n.kids) {
        if (k->is_text || !k->st) continue;
        inline_boxes(*k);
    }
    if (n.st && n.st->display == Disp::Inline && !n.lines.empty()) {
        float x0 = 1e9f, y0 = 1e9f, x1 = -1e9f, y1 = -1e9f;
        for (auto& l : n.lines) {
            x0 = std::min(x0, l.box.x);
            y0 = std::min(y0, l.box.y);
            x1 = std::max(x1, l.box.x + l.box.w);
            y1 = std::max(y1, l.box.y + l.box.h);
        }
        n.box = {x0, y0, x1 - x0, y1 - y0};
    }
}

Node* positioned_ancestor(Node* n) {
    if (n->st && n->st->pos == Pos::Fixed) return nullptr;
    for (Node* p = n->parent; p; p = p->parent)
        if (p->st && p->st->pos != Pos::Static) return p;
    return nullptr;
}

void place_abs(Node& root, AbsEntry e) {
    Node& n = *e.n;
    Node* par = e.parent;
    if (!n.st) compute_style(n);
    const Style& s = *n.st;
    Node* cb = positioned_ancestor(&n);
    int cbx, cby, cbw_, cbh_;
    if (cb) {
        cbx = cb->abs_x + cb->st->border[3];
        cby = cb->abs_y + cb->st->border[0];
        cbw_ = cb->lw - cb->st->border[3] - cb->st->border[1];
        cbh_ = cb->lh - cb->st->border[0] - cb->st->border[2];
    } else {
        cbx = root.abs_x;
        cby = root.abs_y;
        cbw_ = root.lw;
        cbh_ = root.lh;
    }
    int l = resolve(s.off[3], cbw_), r = resolve(s.off[1], cbw_), t = resolve(s.off[0], cbh_), b = resolve(s.off[2], cbh_);
    int ml = margin_side(n, 3, cbw_), mr = margin_side(n, 1, cbw_), mt = margin_side(n, 0, cbw_), mb = margin_side(n, 2, cbw_);
    Cons c;
    c.cbw = cbw_;
    c.cbh = cbh_;
    int wv = resolve(s.width, cbw_), hv = resolve(s.height, cbh_);
    if (wv == kNA && l != kNA && r != kNA) c.fw = std::max(0, cbw_ - l - r - ml - mr);
    else if (wv == kNA) {
        c.shrink = true;
        c.avail = std::max(0, cbw_ - (l == kNA ? 0 : l) - (r == kNA ? 0 : r) - ml - mr);
    }
    if (hv == kNA && t != kNA && b != kNA) {
        c.fh = std::max(0, cbh_ - t - b - mt - mb);
        c.fh_definite = true;
    }
    n.max_content_cache = kNA;
    layout_node(n, c);
    int ax, ay;
    if (l != kNA) ax = cbx + l + ml;
    else if (r != kNA) ax = cbx + cbw_ - r - n.lw - mr;
    else {
        const Style& ps = *par->st;
        int pw = par->lw - par->st->border[3] - par->st->border[1] - pad_side(*par, 3, 0) - pad_side(*par, 1, 0);
        int px0 = par->abs_x + ps.border[3] + pad_side(*par, 3, 0);
        int sx = px0;
        if ((ps.display == Disp::Flex || ps.display == Disp::InlineFlex) && !ps.column) {
            int fr = pw - n.lw - ml - mr;
            if (ps.justify == Just::Center) sx = px0 + lu_div(fr, 2);
            else if (ps.justify == Just::End) sx = px0 + fr;
        } else if ((ps.display == Disp::Flex || ps.display == Disp::InlineFlex) && ps.column) {
            Align a = item_align(n, ps);
            int fr = pw - n.lw - ml - mr;
            if (a == Align::Center) sx = px0 + lu_div(fr, 2);
            else if (a == Align::End) sx = px0 + fr;
        }
        ax = sx + ml;
        if (!(ps.display == Disp::Flex || ps.display == Disp::InlineFlex)) ax = par->abs_x + e.sx + ml;
    }
    if (t != kNA) ay = cby + t + mt;
    else if (b != kNA) ay = cby + cbh_ - b - n.lh - mb;
    else {
        const Style& ps = *par->st;
        int ph = par->lh - par->st->border[0] - par->st->border[2] - pad_side(*par, 0, 0) - pad_side(*par, 2, 0);
        int py0 = par->abs_y + ps.border[0] + pad_side(*par, 0, 0);
        int sy = py0;
        if ((ps.display == Disp::Flex || ps.display == Disp::InlineFlex) && !ps.column) {
            Align a = item_align(n, ps);
            int fr = ph - n.lh - mt - mb;
            if (a == Align::Center) sy = py0 + lu_div(fr, 2);
            else if (a == Align::End) sy = py0 + fr;
        } else if ((ps.display == Disp::Flex || ps.display == Disp::InlineFlex) && ps.column) {
            int fr = ph - n.lh - mt - mb;
            if (ps.justify == Just::Center) sy = py0 + lu_div(fr, 2);
            else if (ps.justify == Just::End) sy = py0 + fr;
        }
        ay = sy + mt;
        if (!(ps.display == Disp::Flex || ps.display == Disp::InlineFlex)) ay = par->abs_y + e.sy + mt;
    }
    n.lx = ax - par->abs_x;
    n.ly = ay - par->abs_y;
    finalize(n, par->abs_x, par->abs_y);
}

void prepare(Node& n) {
    if (!n.is_text) compute_style(n);
    for (auto& k : n.kids) {
        k->parent = &n;
        prepare(*k);
    }
    if (n.is_text) n.st = n.parent ? n.parent->st : nullptr;
}

std::unique_ptr<Node> build(const nlohmann::json& j, Node* parent) {
    auto n = std::make_unique<Node>();
    n->parent = parent;
    if (j.contains("text") && !j.contains("tag")) {
        n->is_text = true;
        n->text = j["text"].get<std::string>();
        return n;
    }
    n->tag = j.value("tag", "div");
    n->id = j.value("id", "");
    if (j.contains("cls"))
        for (auto& c : j["cls"]) n->cls.push_back(c.get<std::string>());
    if (j.contains("cs"))
        for (auto it = j["cs"].begin(); it != j["cs"].end(); ++it) n->css[it.key()] = it.value().get<std::string>();
    if (j.contains("shapes"))
        for (auto& sh : j["shapes"]) {
            Node::Shape shape;
            shape.type = sh.value("t", "");
            for (auto it = sh["a"].begin(); it != sh["a"].end(); ++it) shape.attr[it.key()] = it.value().get<std::string>();
            n->shapes.push_back(std::move(shape));
        }
    if (j.contains("value") && j["value"].is_string()) n->svgattr["value"] = j["value"].get<std::string>();
    if (j.contains("svgattr"))
        for (auto it = j["svgattr"].begin(); it != j["svgattr"].end(); ++it) n->svgattr[it.key()] = it.value().get<std::string>();
    if (j.contains("data") && j["data"].is_object())
        for (auto it = j["data"].begin(); it != j["data"].end(); ++it)
            n->svgattr["data-" + it.key()] = it.value().is_string() ? it.value().get<std::string>() : it.value().dump();
    if (j.contains("kids"))
        for (auto& k : j["kids"]) n->kids.push_back(build(k, n.get()));
    return n;
}

void table_apply(Node& n, const nlohmann::json& table) {
    if (!n.is_text && n.css.empty()) {
        std::string key = n.tag;
        const nlohmann::json* best = nullptr;
        if (table.contains("*")) best = &table["*"];
        if (table.contains(n.tag)) best = &table[n.tag];
        std::string acc = n.tag;
        for (auto& c : n.cls) {
            acc += "." + c;
            if (table.contains(acc)) best = &table[acc];
        }
        if (best)
            for (auto it = best->begin(); it != best->end(); ++it) n.css[it.key()] = it.value().get<std::string>();
    }
    for (auto& k : n.kids) table_apply(*k, table);
}

}  // namespace

Tree load_tree(const nlohmann::json& j, float dpr) {
    Tree t;
    t.dpr = dpr;
    g_zoom = dpr;
    t.root = build(j, nullptr);
    return t;
}

void apply_style_table(Tree& t, const nlohmann::json& table) { table_apply(*t.root, table); }

void run_layout(Tree& t, float vw, float vh) {
    g_zoom = t.dpr;
    Node& root = *t.root;
    prepare(root);
    std::vector<AbsEntry> abs;
    g_abs = &abs;
    Cons c;
    c.fw = to_lu(vw);
    c.fh = to_lu(vh);
    c.fh_definite = true;
    c.avail = c.fw;
    c.cbw = c.fw;
    c.cbh = c.fh;
    layout_node(root, c);
    root.lx = root.ly = 0;
    finalize(root, 0, 0);
    for (size_t i = 0; i < abs.size(); i++) {
        bool later = false;
        for (size_t j = i + 1; j < abs.size() && !later; j++)
            if (abs[j].n == abs[i].n) later = true;
        if (!later) place_abs(root, abs[i]);
    }
    g_abs = nullptr;
    inline_boxes(root);
}

void walk(const Node& n, void (*fn)(const Node&, void*), void* user) {
    fn(n, user);
    for (auto& k : n.kids)
        if (!k->is_text) walk(*k, fn, user);
}

namespace {
std::map<std::string, std::vector<int>>& declared_weights() {
    thread_local std::map<std::string, std::vector<int>> m;
    return m;
}

std::string family_key(const std::string& f) {
    std::string k;
    for (char c : f) k += (char)std::tolower((unsigned char)c);
    return k;
}
}  // namespace

void declare_font_weights(const std::string& family, const std::vector<int>& weights) { declared_weights()[family_key(family)] = weights; }

int declared_font_weight(const std::string& family, int weight, bool* synthetic_bold) {
    if (synthetic_bold) *synthetic_bold = false;
    auto it = declared_weights().find(family_key(family));
    if (it == declared_weights().end() || it->second.empty()) return weight;
    int best = it->second.front();
    for (int w : it->second)
        if (std::abs(w - weight) < std::abs(best - weight)) best = w;
    if (synthetic_bold) *synthetic_bold = weight >= 600 && best < 600;
    return best;
}

}  // namespace a380::layout
