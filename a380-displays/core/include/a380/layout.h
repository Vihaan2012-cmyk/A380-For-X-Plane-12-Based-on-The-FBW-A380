#pragma once

#include <memory>
#include <string>
#include <unordered_map>
#include <vector>

#include "nlohmann/json.hpp"

namespace a380::layout {

struct Rect {
    float x = 0, y = 0, w = 0, h = 0;
};

struct Node;

struct TextLine {
    Rect box;
    float baseline = 0;
    float trim_x = 0, trim_w = 0;
    std::string text;
    const Node* owner = nullptr;
};

struct ComputedStyle;

struct Node {
    std::string tag, id;
    std::vector<std::string> cls;
    std::string text;
    bool is_text = false;
    bool anonymous = false;
    std::unordered_map<std::string, std::string> css;
    struct Shape {
        std::string type;
        std::unordered_map<std::string, std::string> attr;
    };
    std::vector<Shape> shapes;
    std::unordered_map<std::string, std::string> svgattr;
    std::vector<std::unique_ptr<Node>> kids;
    Node* parent = nullptr;

    Rect box;
    std::vector<TextLine> lines;

    std::shared_ptr<ComputedStyle> st;
    int lx = 0, ly = 0, lw = 0, lh = 0, lbase = INT32_MIN;
    int abs_x = 0, abs_y = 0;
    int esc_mb = 0;
    int max_content_cache = INT32_MIN;
    int min_content_cache = INT32_MIN;
    int static_x = 0, static_y = 0;
    struct Frag {
        Node* owner;
        int x, y, w, h, baseline;
        std::string text;
    };
    std::vector<Frag> frags;

    std::string get(const std::string& key) const;
    bool has_class(const std::string& c) const;
    std::string class_string() const;
    std::string own_text() const;
};

struct Tree {
    std::unique_ptr<Node> root;
    float dpr = 1.5f;
};

Tree load_tree(const nlohmann::json& j, float dpr);

// Style table: key "tag.class1.class2" (classes in DOM order) -> {css property: value}. Nodes without their own "cs"
// take the table entry with the longest matching class list, then "tag", then "*".
void apply_style_table(Tree& t, const nlohmann::json& table);

// Lays the tree out in Blink LayoutUnits (1/64 device px); the root border box is forced to vw x vh CSS px.
void run_layout(Tree& t, float vw, float vh);

void declare_font_weights(const std::string& family, const std::vector<int>& weights);
int declared_font_weight(const std::string& family, int weight, bool* synthetic_bold);

void walk(const Node& n, void (*fn)(const Node&, void*), void* user);

}  // namespace a380::layout
