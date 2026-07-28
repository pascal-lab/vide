#pragma once

#include <string>
#include <vector>

#include "cxx.h"

#include "slang/diagnostics/DiagnosticEngine.h"
#include "slang/syntax/SyntaxTree.h"
#include "slang/text/SourceManager.h"

namespace slang_sys::helper {
    inline std::vector<std::string> to_std_strings(const rust::Vec<rust::String> &values) {
        std::vector<std::string> result;
        result.reserve(values.size());
        for (const auto &value : values)
            result.emplace_back(value.data(), value.size());
        return result;
    }

} // namespace slang_sys::helper