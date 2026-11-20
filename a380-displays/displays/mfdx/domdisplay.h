#pragma once

#include "a380/profile.h"

#include <cstdint>
#include <string>
#include <tuple>
#include <vector>

#include "a380/display.h"
#include "a380/layout.h"
#include "include/core/SkImage.h"

namespace a380::mfdx {

struct ClassBackground {
    std::string cls;
    std::string image;
    std::string size;
    std::string position;
};

struct DomConfig {
    int width = 1646;
    int height = 1024;
    std::string topic;
    std::string sibling;
    std::string env;
    std::string model_topic;
    std::string scroll_topic;
    bool lcd_text = false;
    bool black_backdrop = false;
    bool css_stacking = false;
    bool css_effects = false;
    bool scrollbars = false;
    bool css_fidelity = false;
    std::string scrollbar_cls;
    uint32_t scrollbar_track = 0;
    uint32_t scrollbar_thumb = 0;
    float scrollbar_width = 0.f;
    std::vector<ClassBackground> class_backgrounds;
};

class DomDisplay : public Display {
public:
    explicit DomDisplay(DomConfig cfg);
    SkISize size() const override;
    void draw(SkCanvas& canvas, const Inputs& in) override;
    const std::vector<float>* element_boxes() const override { return &boxes_; }

protected:
    virtual int state(const Inputs&) const { return 0; }
    virtual void overlay(SkCanvas&, int) {}

private:
    bool ensure_tree(const Inputs& in, float dpr);
    void render(SkCanvas& c, float dpr, int mode, bool have_tree);

    sk_sp<SkImage> cached_;
    size_t cached_key_ = 0;
    float cached_dpr_ = 0.f;
    sk_sp<SkImage> gpu_cached_;
    size_t gpu_key_ = 0;
    float gpu_dpr_ = 0.f;
    DomConfig cfg_;
    void apply_scroll(const Inputs& in);

    layout::Tree tree_;
    size_t tree_key_ = 0;
    size_t scroll_key_ = 0;
    std::vector<std::tuple<layout::Node*, float, float>> scrolled_;
    std::vector<float> boxes_;
    a380::SectionProfile prof_{"dom"};
};

}  // namespace a380::mfdx
