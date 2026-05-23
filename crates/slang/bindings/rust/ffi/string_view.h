#pragma once
#include "rust/cxx.h"
#include <string_view>

static_assert(sizeof(std::string_view) == 2 * sizeof(void *), "");
static_assert(alignof(std::string_view) == alignof(void *), "");

inline std::string_view string_view_from_str(rust::Str s) {
  return {s.data(), s.size()};
}

inline rust::Slice<const char> string_view_as_bytes(std::string_view s) {
  return {s.data(), s.size()};
}
