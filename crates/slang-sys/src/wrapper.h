#pragma once

#include <cstdint>

#include "rust/cxx.h"

namespace slang_sys
{

uint16_t parse_root_kind(rust::Str text);

} // namespace slang_sys
