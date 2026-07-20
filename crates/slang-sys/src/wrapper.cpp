#include "wrapper.h"

#include <string_view>

#include "slang/syntax/SyntaxTree.h"

namespace slang_sys
{

uint16_t parse_root_kind(rust::Str text)
{
    auto source = std::string_view(text.data(), text.size());
    auto name = std::string_view("demo");
    auto path = std::string_view("demo.sv");

    auto tree = slang::syntax::SyntaxTree::fromText(source, name, path);
    return static_cast<uint16_t>(tree->root().kind);
}

} // namespace slang_sys
