#pragma once

namespace a380 {

// std::getenv read once per thread and key; the environment never changes while displays draw.
const char* getenv_cached(const char* key);

}  // namespace a380
