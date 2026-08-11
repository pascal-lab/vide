#pragma once

#include <memory>
#include <string>
#include <vector>

#include "cxx.h"

#include "../diagnostic/wrapper.h"
#include "../syntax/wrapper.h"
#include "../wrapper.h"
#include "slang/ast/Compilation.h"

namespace slang_sys::compilation {

class Compilation {
  public:
    explicit Compilation(std::vector<std::string> top_modules);

    std::vector<std::string> top_modules;
    std::shared_ptr<syntax::SourceSession> session;
    std::unique_ptr<::slang::ast::Compilation> inner;
};

std::unique_ptr<Compilation> new_compilation(rust::Vec<rust::String> top_modules);
std::shared_ptr<syntax::SyntaxTree> parse_syntax_tree_from_text(
    Compilation& compilation,
    rust::Str text,
    rust::Str name,
    rust::Str path,
    rust::Vec<rust::String> predefines,
    rust::Vec<rust::String> include_paths,
    rust::Vec<rust::String> include_buffer_paths,
    rust::Vec<rust::String> include_buffer_texts,
    bool expand_includes,
    bool collect_expected_syntax
);
std::shared_ptr<syntax::SyntaxTree> parse_library_map_syntax_tree_from_text(
    Compilation& compilation,
    rust::Str text,
    rust::Str name,
    rust::Str path
);
void add_syntax_tree(Compilation& compilation, std::shared_ptr<syntax::SyntaxTree> tree);
rust::Vec<diagnostic::RawSyntaxDiagnostic> parse_diagnostics(
    const Compilation& compilation,
    rust::Vec<rust::String> warning_options
);
rust::Vec<diagnostic::RawSyntaxDiagnostic> semantic_diagnostics(
    const Compilation& compilation,
    rust::Vec<rust::String> warning_options
);
rust::Vec<rust::String> system_function_names();
rust::Vec<rust::String> system_task_names();

} // namespace slang_sys::compilation
