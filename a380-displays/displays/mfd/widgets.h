#pragma once

#include <string>
#include <string_view>
#include <vector>

#include "include/core/SkCanvas.h"
#include "include/core/SkColor.h"
#include "include/core/SkRect.h"

namespace a380::mfd::ui {

// All coordinates are CSS pixels in the side's content space, y down.
// "top" always means the top of the CSS line box (what a CSS block would
// start at), so a page can translate a CSS layout box by box.

namespace pal {
constexpr SkColor white = 0xFFFFFFFF;
constexpr SkColor grey = 0xFF4F5467;
constexpr SkColor amber = 0xFFE68000;
constexpr SkColor dark_grey = 0xFF868A9C;
constexpr SkColor light_grey = 0xFFC4C6CF;
constexpr SkColor cyan = 0xFF00FFFF;
constexpr SkColor green = 0xFF00FF00;
constexpr SkColor magenta = 0xFFFF94FF;
constexpr SkColor red = 0xFFFF0000;
constexpr SkColor yellow = 0xFFFFFF00;
constexpr SkColor unit_blue = 0xFF0C66F7;
constexpr SkColor darker_grey = 0xFF272525;
constexpr SkColor background = 0xFF000000;
constexpr SkColor title_text = 0xFF222222;
}  // namespace pal

enum class Align { Left, Center, Right };
enum class Face { Mfd, Ecam };  // "FBW-Display-EIS-A380-SlashedZero" / "Ecam"

struct TextStyle {
    float size = 20;
    SkColor color = pal::white;
    Face face = Face::Mfd;
    Align align = Align::Left;
};

// CSS presets. mfd-label: 20 (bigger 24, biggest 28); mfd-value: 25 (smaller 20,
// bigger 30); mfd-label-unit: 20 blue (bigger 24).
TextStyle label_style(SkColor color = pal::white, float size = 20);
TextStyle label_bigger_style(SkColor color = pal::white);
TextStyle label_biggest_style(SkColor color = pal::white);
TextStyle value_style(SkColor color = pal::green, float size = 25);
TextStyle value_smaller_style(SkColor color = pal::green);
TextStyle value_bigger_style(SkColor color = pal::green);
TextStyle unit_style(float size = 20);

float text_width(std::string_view text, const TextStyle& style);
// Greedy word wrap like CSS normal wrapping (newlines count as spaces).
std::vector<std::string> wrap_text(std::string_view text, const TextStyle& style, float max_width);
float line_height(const TextStyle& style);

// Draws `text` with the line box starting at `top`. x is the left edge, centre
// or right edge according to style.align.
void text(SkCanvas& c, std::string_view text, float x, float top, const TextStyle& style);
// Centred in a rect (both axes, like flex centring).
void text_in(SkCanvas& c, std::string_view text, SkRect box, const TextStyle& style);
// Aligned horizontally inside box, line box vertically centred.
void text_in_aligned(SkCanvas& c, std::string_view text, SkRect box, const TextStyle& style, float pad_x = 0);

void label(SkCanvas& c, std::string_view s, float x, float top, SkColor color = pal::white, float size = 20,
           Align a = Align::Left);
void value(SkCanvas& c, std::string_view s, float x, float top, SkColor color = pal::green, float size = 25,
           Align a = Align::Left);
// Value followed by a blue unit (trailing) or preceded by one (leading); returns the x past the end.
float value_with_unit(SkCanvas& c, std::string_view value_text, std::string_view unit_text, float x, float top,
                      SkColor color = pal::green, bool leading = false, float value_size = 25, float unit_size = 20);

// ---- primitives ----
enum class Bevel { Outset, Inset, Flat };
// CSS "2px outset/inset <colour>" border drawn inside rect.
void bevel_border(SkCanvas& c, SkRect rect, Bevel kind, SkColor color = pal::light_grey, float width = 2);
void fill(SkCanvas& c, SkRect rect, SkColor color);
void stroke_box(SkCanvas& c, SkRect rect, SkColor color, float width = 1);  // CSS border, inside rect
void hline(SkCanvas& c, float x0, float x1, float y, SkColor color, float width = 1);
void vline(SkCanvas& c, float x, float y0, float y1, SkColor color, float width = 1);
void triangle_down(SkCanvas& c, float x, float y, SkColor color = pal::white, float size = 15);  // x,y top-left of 15x15
void triangle_up(SkCanvas& c, float x, float y, SkColor color = pal::white, float size = 15);
void triangle_left(SkCanvas& c, float x, float y, SkColor color = pal::white, float size = 15);
void asterisk(SkCanvas& c, float cx, float cy, SkColor color, float size = 20);

// ---- buttons (.mfd-button) ----
enum class ButtonState { Normal, Disabled, Selected /* inset, dark-grey fill (opened) */, Highlighted /* white fill */ };
struct ButtonStyle {
    ButtonState state = ButtonState::Normal;
    bool asterisk = false;       // trailing "*" glyph after the label
    bool arrow = false;          // dropdown arrow after the label
    bool arrow_up = false;       // menu opens upwards
    bool tmpy = false;           // yellow label (temporary flight plan)
    SkColor text_color = 0;      // 0 = derive from state
    float font_size = 22;
    float pad_top = 9, pad_bottom = 5, pad_left = 12, pad_right = 12;
    float arrow_label_right = 5;
};
void button(SkCanvas& c, SkRect rect, std::string_view label, const ButtonStyle& st = {});
// Natural height of an auto-height button: border 2 + padding 9/5 + line box(es).
float button_height(float font_size = 22, int lines = 1);
// Two-line label split on '\n' (CLEAR INFO).
void button_lines(SkCanvas& c, SkRect rect, const std::vector<std::string>& lines, const ButtonStyle& st = {});
// Icon button (.mfd-icon-button): bevelled box, caller draws the icon inside.
void icon_button(SkCanvas& c, SkRect rect, bool disabled = false);

// ---- input fields (.mfd-input-field-*) ----
enum class FieldKind {
    Entered,    // cyan text, bordered: an editable field with a value
    Mandatory,  // amber text, bordered
    Computed,   // 22px, green, bordered (computedByFms)
    Inactive,   // green (or white) text, no border: a read only value
    Disabled,   // dark-grey text on darker grey
    Tmpy,       // yellow text, bordered
    Selected,   // cyan fill, black text (value selected)
    Editing,    // cyan text, caret
    Validating  // light-grey text
};
struct FieldStyle {
    FieldKind kind = FieldKind::Entered;
    SkColor text_color = 0;      // 0 = by kind
    float font_size = 27;        // 22 when Computed
    std::string unit_leading;    // blue unit before the value
    std::string unit_trailing;   // blue unit after the value
    Align align = Align::Center;  // text alignment inside the container
    bool border = true;           // false for the inactive look
    float min_text_width = 0;     // CSS min-width of the text container
    bool yellow_when_disabled = false;
    float text_dy = 0;
    float text_dx = 0;
};
// rect is the outer box of .mfd-input-field-container (height normally 37).
void input_field(SkCanvas& c, SkRect rect, std::string_view text, const FieldStyle& st = {});
// Natural width of a field: min_text_width or text width, plus units, padding, border.
float input_field_width(std::string_view text, const FieldStyle& st);
// Boxed amber placeholder like "-----" or "□□□□" is just a Mandatory field with that text.

// ---- dropdowns ----
struct DropdownStyle {
    bool inactive = false;     // .inactive: no border, no arrow
    bool disabled = false;
    SkColor text_color = pal::cyan;
    float font_size = 27;
    Align align = Align::Center;
    bool open = false;
};
// .mfd-dropdown-outer: bevelled darker-grey frame around an inner black box showing `text`, arrow at right.
void dropdown(SkCanvas& c, SkRect rect, std::string_view text, const DropdownStyle& st = {});
struct MenuItem {
    std::string text;
    bool disabled = false;
};
// .mfd-dropdown-menu: item font 20, each item 5/16 padding + 3px border. rect.x/y top-left, width fixed by rect.
// Returns the height used. selected < 0: none highlighted. Menu height is limited to 275 (scrolls) like FBW.
float dropdown_menu(SkCanvas& c, SkRect rect, const std::vector<MenuItem>& items, int hover = -1);
float dropdown_menu_item_height();
float dropdown_menu_natural_width(const std::vector<MenuItem>& items);
// .mfd-context-menu: item font 22, 7/2 padding + 3 border.
float context_menu(SkCanvas& c, float x, float y, float width, const std::vector<MenuItem>& items, int hover = -1);
float context_menu_item_height();

// ---- page selector row cell (ACTIVE, POSITION ...) ----
void page_selector(SkCanvas& c, SkRect rect, std::string_view label, bool active_boxed, bool open = false);
struct PageSelectorStyle {
    bool arrow = true;        // false when the menu has no entries (arrow hidden, label centred on the cell)
    SkColor color = pal::white;
    SkColor background = 0;   // label background over its content box (EMER)
};
// Label may hold several lines separated by newline.
void page_selector_ex(SkCanvas& c, SkRect rect, std::string_view label, bool active_boxed, const PageSelectorStyle& st);

// ---- title bar (.mfd-title-bar-*) ----
struct TitleBar {
    std::string title;       // "ACTIVE/F-PLN"
    bool penalty = false;    // white PENALTY tag
    bool eo = false;         // amber EO tag
    bool tmpy = false;       // yellow TMPY tag
    float gap = 2;           // margin-left of the EO and TMPY sections (0 on ATCCOM pages)
};
// rect: full width, height is derived (returns used height, normally 44).
float title_bar(SkCanvas& c, SkRect rect, const TitleBar& tb);

// ---- top tab navigator ----
struct TabStyle {
    float bar_height = 36;
    float slant_deg = 30;
    int small_font = -1;  // -1: auto (more than 2 non-empty titles), 0/1 force
    int selected = 0;
    int highlighted = -1;                  // label drawn green
    SkColor selected_text_color = pal::white;
    float additional_right_space = 0;
    std::vector<int> disabled;
};
// rect is the margin box of .mfd-top-tab-navigator-container (the 2px/9px margins are applied inside).
// Draws the slanted tab bar and the content frame below it. Empty titles are skipped.
// Returns the content rect (inside border and 10px padding).
SkRect tab_navigator(SkCanvas& c, SkRect rect, const std::vector<std::string>& titles, const TabStyle& st);

// ---- radio buttons ----
void radio_button(SkCanvas& c, float x, float top, std::string_view label, bool checked,
                  SkColor checked_color = pal::cyan, bool disabled = false, SkColor label_color = 0, float label_dx = 0);
constexpr float radio_row_height = 34;
constexpr float radio_row_pitch = 42;

// ---- labelled boxes / separators ----
// .mfd-fms-fpln-labeled-box-container: 1px light-grey frame with a label breaking the top edge.
void labeled_box(SkCanvas& c, SkRect rect, std::string_view label);
// .mfd-label-value-container etc: simple horizontal rule across the content.
void separator(SkCanvas& c, float x0, float x1, float y, SkColor color = pal::light_grey, float width = 2);

// ---- F-PLN line parts (MfdFmsFpln.scss) ----
struct FplnLineColors {
    SkColor ident = pal::green;       // .mfd-fms-fpln-line-ident
    SkColor annotation = pal::white;
    SkColor small = pal::green;       // .mfd-fms-fpln-label-small
};
FplnLineColors fpln_colors(bool secondary, bool temporary, bool altn);
// Line box 72px high: annotation row 32 on top (22px, right aligned), ident row 40 (30px).
void fpln_ident(SkCanvas& c, float x, float y, float width, std::string_view ident, SkColor color,
                bool selected = false);
void fpln_annotation(SkCanvas& c, float x, float y, float width, std::string_view s, SkColor color = pal::white);
void fpln_small_label(SkCanvas& c, SkRect rect, std::string_view s, SkColor color = pal::green, float size = 22);
constexpr float fpln_line_height = 72;

// ---- misc ----
void amber_message(SkCanvas& c, SkRect rect, std::string_view s);  // .mfd-amber-error-message
// Footer message line: white box with black 26px text.
void message_box(SkCanvas& c, SkRect rect, std::string_view s, SkColor bg = pal::white, SkColor fg = pal::background);

}  // namespace a380::mfd::ui
