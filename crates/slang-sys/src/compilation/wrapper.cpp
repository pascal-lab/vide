#include "compilation/wrapper.h"

#include "slang/ast/SystemSubroutine.h"
#include "slang/parsing/KnownSystemName.h"

namespace slang_sys::compilation {

namespace {

::slang::Bag make_options(const std::vector<std::string>& top_modules) {
    ::slang::Bag options;
    auto& compilation_options = options.insertOrGet<::slang::ast::CompilationOptions>();
    for (const auto& top_module : top_modules)
        compilation_options.topModules.emplace(top_module);
    return options;
}

std::vector<std::string> to_std_strings(const rust::Vec<rust::String>& values) {
    std::vector<std::string> result;
    result.reserve(values.size());
    for (const auto& value : values)
        result.emplace_back(value.data(), value.size());
    return result;
}

} // namespace

Compilation::Compilation(std::vector<std::string> top_modules) :
    top_modules(std::move(top_modules)),
    inner(std::make_unique<::slang::ast::Compilation>(make_options(this->top_modules))) {}

std::unique_ptr<Compilation> new_compilation(rust::Vec<rust::String> top_modules) {
    return std::make_unique<Compilation>(to_std_strings(top_modules));
}

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
) {
    auto tree = syntax::tree::parse_syntax_tree(
        text,
        name,
        path,
        std::move(predefines),
        std::move(include_paths),
        std::move(include_buffer_paths),
        std::move(include_buffer_texts),
        expand_includes,
        collect_expected_syntax
    );
    compilation.inner->addSyntaxTree(tree->tree);
    return tree;
}

std::shared_ptr<syntax::SyntaxTree> parse_library_map_syntax_tree_from_text(
    Compilation& compilation,
    rust::Str text,
    rust::Str name,
    rust::Str path
) {
    auto tree = syntax::tree::parse_library_map_syntax_tree(text, name, path, false);
    compilation.inner->addSyntaxTree(tree->tree);
    return tree;
}

void add_syntax_tree(Compilation& compilation, std::shared_ptr<syntax::SyntaxTree> tree) {
    compilation.inner->addSyntaxTree(std::move(tree->tree));
}

rust::Vec<diagnostic::RawSyntaxDiagnostic> parse_diagnostics(
    const Compilation& compilation,
    rust::Vec<rust::String> warning_options
) {
    return diagnostic::diagnostics_to_rust(
        compilation.inner->getParseDiagnostics(),
        *compilation.inner->getSourceManager(),
        std::move(warning_options)
    );
}

rust::Vec<diagnostic::RawSyntaxDiagnostic> semantic_diagnostics(
    const Compilation& compilation,
    rust::Vec<rust::String> warning_options
) {
    return diagnostic::diagnostics_to_rust(
        compilation.inner->getSemanticDiagnostics(),
        *compilation.inner->getSourceManager(),
        std::move(warning_options)
    );
}

template<bool Tasks>
rust::Vec<rust::String> system_names() {
    ::slang::ast::Compilation compilation;
    rust::Vec<rust::String> result;
    for (auto known_name : ::slang::parsing::KnownSystemName_traits::values) {
        auto* subroutine = compilation.getSystemSubroutine(known_name);
        if (!subroutine)
            continue;
        const bool is_task = subroutine->kind == ::slang::ast::SubroutineKind::Task;
        if (is_task != Tasks)
            continue;
        result.emplace_back(rust::String(subroutine->name));
    }
    return result;
}

rust::Vec<rust::String> system_function_names() {
    return system_names<false>();
}

rust::Vec<rust::String> system_task_names() {
    return system_names<true>();
}

} // namespace slang_sys::compilation
