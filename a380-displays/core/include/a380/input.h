#pragma once

#include <memory>
#include <string>
#include <string_view>
#include <vector>

#include "a380/inputs.h"

namespace a380 {

enum class PointerKind { Down = 0, Drag = 1, Up = 2, Move = 3, Wheel = 4 };

struct Write {
    char kind = 'L';
    std::string name;
    double value = 0.0;
    double delay_ms = 0.0;
};

// Clicks and keys on a display's page, in CSS pixels from the top-left corner. `observe` sees the
// inputs of every draw so visibility follows what is on screen.
class InputHandler {
public:
    virtual ~InputHandler() = default;
    virtual void observe(const Inputs& in) = 0;
    virtual bool pointer(double x, double y, PointerKind kind, int button, std::vector<Write>& out) = 0;
    virtual bool key(int key, int mods, bool down, std::vector<Write>& out) = 0;
    // false while part of the page's input logic still runs in the page itself.
    virtual bool owns_input() const = 0;
};

std::unique_ptr<InputHandler> make_input_handler(std::string_view id, std::string_view variant);

}  // namespace a380
