#include "compilation/wrapper.h"
#include "slang-sys/src/compilation/ffi.rs.h"

#include "slang/ast/SystemSubroutine.h"
#include "slang/parsing/KnownSystemName.h"

#include <stdexcept>

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
    session(std::make_shared<syntax::SourceSession>()),
    inner(std::make_unique<::slang::ast::Compilation>(make_options(this->top_modules))) {}

std::unique_ptr<Compilation> new_compilation(rust::Vec<rust::String> top_modules) {
    return std::make_unique<Compilation>(to_std_strings(top_modules));
}

std::shared_ptr<syntax::SyntaxTree> parse_syntax_tree_from_text(
    Compilation& compilation,
    rust::Str text,
    rust::Str name,
    rust::Str path,
    ParseSyntaxTreeOptions options
) {
    auto tree = syntax::tree::parse_syntax_tree_with_session(
        compilation.session,
        text,
        name,
        path,
        std::move(options.predefines),
        std::move(options.include_paths),
        std::move(options.include_buffer_paths),
        std::move(options.include_buffer_texts),
        options.expand_includes,
        false,
        options.collect_expected_syntax,
        options.expected_syntax_offset,
        options.has_expected_syntax_offset
    );
    compilation.inner->addSyntaxTree(tree->tree);
    return tree;
}

void register_source_buffers(
    Compilation& compilation,
    rust::Vec<rust::String> paths,
    rust::Vec<rust::String> texts
) {
    if (paths.size() != texts.size())
        throw std::invalid_argument("source buffer paths and texts must have equal lengths");

    for (std::size_t i = 0; i < paths.size(); i++) {
        compilation.session->assign_source_buffer(
            std::string(paths[i].data(), paths[i].size()),
            std::string(texts[i].data(), texts[i].size())
        );
    }
}

std::shared_ptr<syntax::SyntaxTree> parse_syntax_tree_from_buffer(
    Compilation& compilation,
    rust::Str name,
    rust::Str path,
    ParseSyntaxTreeOptions options
) {
    if (!options.include_buffer_paths.empty() || !options.include_buffer_texts.empty())
        throw std::invalid_argument(
            "buffer parsing requires source buffers to be registered on the compilation"
        );

    auto tree = syntax::tree::parse_syntax_tree_from_buffer_with_session(
        compilation.session,
        name,
        path,
        std::move(options.predefines),
        std::move(options.include_paths),
        options.expand_includes,
        options.collect_expected_syntax,
        options.expected_syntax_offset,
        options.has_expected_syntax_offset
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
    auto tree = syntax::tree::parse_library_map_syntax_tree_with_session(
        compilation.session, text, name, path, false, 0, false);
    compilation.inner->addSyntaxTree(tree->tree);
    return tree;
}

std::shared_ptr<syntax::SyntaxTree> parse_library_map_syntax_tree_from_buffer(
    Compilation& compilation,
    rust::Str name,
    rust::Str path,
    bool collect_expected_syntax,
    std::size_t expected_syntax_offset,
    bool has_expected_syntax_offset
) {
    auto tree = syntax::tree::parse_library_map_syntax_tree_from_buffer_with_session(
        compilation.session,
        name,
        path,
        collect_expected_syntax,
        expected_syntax_offset,
        has_expected_syntax_offset
    );
    compilation.inner->addSyntaxTree(tree->tree);
    return tree;
}

void add_syntax_tree(Compilation& compilation, std::shared_ptr<syntax::SyntaxTree> tree) {
    if (!tree || !tree->tree || !tree->session)
        throw std::invalid_argument("syntax tree and source session must be valid");

    auto *source_manager = compilation.inner->getSourceManager();
    if (source_manager && source_manager != &tree->session->source_manager)
        throw std::logic_error(
            "all syntax trees added to a compilation must share one source session"
        );

    // Slang keeps references to the tree's SourceManager after addSyntaxTree
    // returns. Make the compilation own that session when the first externally
    // parsed tree is attached, so dropping the Rust SyntaxTree wrapper cannot
    // leave Slang with a dangling source manager.
    if (!source_manager)
        compilation.session = tree->session;

    compilation.inner->addSyntaxTree(tree->tree);
}

rust::Vec<diagnostic::RawSyntaxDiagnostic> parse_diagnostics(
    const Compilation& compilation,
    rust::Vec<rust::String> warning_options
) {
    auto *source_manager = compilation.inner->getSourceManager();
    if (!source_manager)
        return {};

    return diagnostic::diagnostics_to_rust(
        compilation.inner->getParseDiagnostics(),
        *source_manager,
        std::move(warning_options)
    );
}

rust::Vec<diagnostic::RawSyntaxDiagnostic> semantic_diagnostics(
    const Compilation& compilation,
    rust::Vec<rust::String> warning_options
) {
    auto *source_manager = compilation.inner->getSourceManager();
    if (!source_manager)
        return {};

    return diagnostic::diagnostics_to_rust(
        compilation.inner->getSemanticDiagnostics(),
        *source_manager,
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
