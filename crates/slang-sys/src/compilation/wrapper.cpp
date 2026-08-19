#include "compilation/wrapper.h"
#include "slang-sys/src/compilation/ffi.rs.h"

#include "slang/ast/ASTVisitor.h"
#include "slang/ast/expressions/CallExpression.h"
#include "slang/ast/expressions/MiscExpressions.h"
#include "slang/ast/expressions/SelectExpressions.h"
#include "slang/ast/symbols/ClassSymbols.h"
#include "slang/ast/symbols/CompilationUnitSymbols.h"
#include "slang/ast/symbols/InstanceSymbols.h"
#include "slang/ast/symbols/SubroutineSymbols.h"
#include "slang/ast/symbols/VariableSymbols.h"
#include "slang/ast/types/AllTypes.h"
#include "slang/text/SourceManager.h"
#include "slang/util/String.h"

#include <optional>
#include <stdexcept>
#include <type_traits>
#include <variant>

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

namespace {

// Path we handed `assignText`. `getRawFileName` is not that: SourceSession
// sets disableProximatePaths, so cacheBuffer stores only path.filename()
// in FileData::name. FileData::fullPath is the assigned spelling.
std::string assigned_path(const slang::SourceManager& sm, slang::BufferID buffer) {
    auto full = sm.getFullPath(buffer);
    if (!full.empty())
        return slang::getU8Str(full);
    return std::string(sm.getRawFileName(buffer));
}

// Resolve the query path once. Per-symbol string compares were the T4 slice
// cost; a live compilation has thousands of symbols and that does not scale.
std::optional<slang::BufferID> buffer_for_path(
    const slang::SourceManager& sm,
    std::string_view want
) {
    for (auto buffer : sm.getAllBuffers()) {
        auto kind = sm.getBufferKind(buffer);
        if (kind == slang::SourceManager::BufferKind::Macro ||
            kind == slang::SourceManager::BufferKind::MacroArg)
            continue;
        if (assigned_path(sm, buffer) == want)
            return buffer;
    }
    return std::nullopt;
}

bool in_buffer(
    const slang::SourceManager& sm,
    slang::SourceLocation loc,
    slang::BufferID buffer
) {
    if (!loc.valid())
        return false;
    if (loc.buffer() == buffer)
        return true;
    auto original = sm.getFullyOriginalLoc(loc);
    return original.valid() && original.buffer() == buffer;
}

bool offset_in_symbol(const slang::ast::Symbol& symbol, std::size_t offset) {
    auto loc = symbol.location;
    if (loc.valid() && loc.offset() == offset)
        return true;
    if (const auto* syntax = symbol.getSyntax()) {
        auto range = syntax->sourceRange();
        if (range.start().valid() && range.end().valid()) {
            auto start = range.start().offset();
            auto end = range.end().offset();
            if (offset >= start && offset <= end)
                return true;
        }
    }
    auto name_end = loc.valid() ? loc.offset() + symbol.name.size() : 0;
    return loc.valid() && offset >= loc.offset() && offset < name_end;
}

std::vector<std::string> inheritance_of(const slang::ast::ClassType& cls) {
    std::vector<std::string> chain;
    const slang::ast::Type* base = cls.getBaseClass();
    while (base) {
        chain.emplace_back(std::string(base->name));
        if (const auto* base_cls = base->as_if<slang::ast::ClassType>())
            base = base_cls->getBaseClass();
        else
            break;
    }
    return chain;
}

std::string member_type_name(const slang::ast::Symbol& symbol) {
    if (const auto* value = symbol.as_if<slang::ast::ValueSymbol>())
        return value->getType().toString();
    if (const auto* sub = symbol.as_if<slang::ast::SubroutineSymbol>())
        return sub->getReturnType().toString();
    return {};
}

bool consider_member(
    const slang::ast::Symbol& symbol,
    const slang::ast::ClassType& owner,
    const slang::SourceManager& sm,
    slang::BufferID buffer,
    std::size_t offset,
    ClassMemberAnswer& out
) {
    auto loc = symbol.location;
    if (const auto* syntax = symbol.getSyntax(); syntax && !in_buffer(sm, loc, buffer))
        loc = syntax->sourceRange().start();
    if (!in_buffer(sm, loc, buffer) || !offset_in_symbol(symbol, offset))
        return false;
    out.found = true;
    out.type_name = rust::String(member_type_name(symbol));
    out.owner_class = rust::String(std::string(owner.name));
    for (auto& name : inheritance_of(owner))
        out.inheritance.push_back(rust::String(std::move(name)));
    return true;
}

bool walk_scope(
    const slang::ast::Scope& scope,
    const slang::SourceManager& sm,
    slang::BufferID buffer,
    std::size_t offset,
    ClassMemberAnswer& out
) {
    for (const auto& member : scope.members()) {
        if (const auto* cls = member.as_if<slang::ast::ClassType>()) {
            for (const auto& child : cls->members()) {
                if (consider_member(child, *cls, sm, buffer, offset, out))
                    return true;
            }
            if (walk_scope(*cls, sm, buffer, offset, out))
                return true;
        } else if (const auto* pkg = member.as_if<slang::ast::PackageSymbol>()) {
            if (walk_scope(*pkg, sm, buffer, offset, out))
                return true;
        } else if (const auto* cu = member.as_if<slang::ast::CompilationUnitSymbol>()) {
            if (walk_scope(*cu, sm, buffer, offset, out))
                return true;
        } else if (const auto* inst = member.as_if<slang::ast::InstanceSymbol>()) {
            if (walk_scope(inst->body, sm, buffer, offset, out))
                return true;
        } else if (const auto* body = member.as_if<slang::ast::InstanceBodySymbol>()) {
            if (walk_scope(*body, sm, buffer, offset, out))
                return true;
        }
    }
    return false;
}

} // namespace

ClassMemberAnswer lookup_class_member(
    Compilation& compilation,
    rust::Str path,
    std::size_t offset
) {
    ClassMemberAnswer out;
    out.found = false;
    if (!compilation.inner)
        return out;
    const auto& root = compilation.inner->getRoot();
    const auto* sm = compilation.inner->getSourceManager();
    if (!sm)
        return out;
    std::string path_owned(path.data(), path.size());
    auto buffer = buffer_for_path(*sm, path_owned);
    if (!buffer)
        return out;
    walk_scope(root, *sm, *buffer, offset, out);
    return out;
}

namespace {

std::string type_of_symbol(const slang::ast::Symbol& symbol) {
    if (const auto* value = symbol.as_if<slang::ast::ValueSymbol>())
        return value->getType().toString();
    if (const auto* sub = symbol.as_if<slang::ast::SubroutineSymbol>())
        return sub->getReturnType().toString();
    if (const auto* type = symbol.as_if<slang::ast::Type>())
        return type->toString();
    if (const auto* inst = symbol.as_if<slang::ast::InstanceSymbol>())
        return std::string(inst->getDefinition().name);
    return {};
}

void fill_symbol(
    const slang::ast::Symbol& symbol,
    const slang::SourceManager& sm,
    SymbolAnswer& out
) {
    out.found = true;
    out.name = rust::String(std::string(symbol.name));
    out.kind = rust::String(std::string(toString(symbol.kind)));
    out.type_name = rust::String(type_of_symbol(symbol));
    if (symbol.location.valid()) {
        out.def_file = rust::String(assigned_path(sm, symbol.location.buffer()));
        out.def_offset = symbol.location.offset();
    }
    if (const auto* scope = symbol.getParentScope()) {
        if (const auto* cls = scope->asSymbol().as_if<slang::ast::ClassType>()) {
            out.owner_class = rust::String(std::string(cls->name));
            for (auto& name : inheritance_of(*cls))
                out.inheritance.push_back(rust::String(std::move(name)));
        }
    }
}

struct FindAtOffset : slang::ast::ASTVisitor<FindAtOffset, slang::ast::VisitFlags::AllGood> {
    const slang::SourceManager& sm;
    slang::BufferID buffer;
    std::size_t offset;
    const slang::ast::Symbol* best = nullptr;
    std::size_t best_span = static_cast<std::size_t>(-1);

    FindAtOffset(const slang::SourceManager& sm, slang::BufferID buffer, std::size_t offset) :
        sm(sm), buffer(buffer), offset(offset) {}

    void consider(const slang::ast::Symbol& symbol, slang::SourceRange range) {
        if (!range.start().valid() || !range.end().valid())
            return;
        if (!in_buffer(sm, range.start(), buffer) && !in_buffer(sm, symbol.location, buffer))
            return;
        auto start = range.start().offset();
        auto end = range.end().offset();
        if (offset < start || offset > end)
            return;
        auto span = end - start;
        if (span < best_span) {
            best_span = span;
            best = &symbol;
        }
    }

    void consider_symbol(const slang::ast::Symbol& symbol) {
        if (!symbol.location.valid() || !in_buffer(sm, symbol.location, buffer))
            return;
        auto end = slang::SourceLocation(
            symbol.location.buffer(),
            symbol.location.offset() + symbol.name.size());
        consider(symbol, slang::SourceRange(symbol.location, end));
        if (const auto* syntax = symbol.getSyntax())
            consider(symbol, syntax->sourceRange());
    }

    template<typename T>
    void handle(const T& node) {
        if constexpr (std::is_same_v<T, slang::ast::NamedValueExpression> ||
                      std::is_same_v<T, slang::ast::HierarchicalValueExpression>) {
            consider(node.symbol, node.sourceRange);
        } else if constexpr (std::is_same_v<T, slang::ast::CallExpression>) {
            if (auto* sub = std::get_if<const slang::ast::SubroutineSymbol*>(&node.subroutine))
                consider(**sub, node.sourceRange);
        } else if constexpr (std::is_same_v<T, slang::ast::MemberAccessExpression>) {
            consider(node.member, node.sourceRange);
        } else if constexpr (std::is_base_of_v<slang::ast::Symbol, T>) {
            consider_symbol(node);
        }
        visitDefault(node);
    }
};

} // namespace

SymbolAnswer lookup_symbol(
    Compilation& compilation,
    rust::Str path,
    std::size_t offset
) {
    SymbolAnswer out;
    out.found = false;
    out.def_offset = 0;
    if (!compilation.inner)
        return out;
    const auto& root = compilation.inner->getRoot();
    const auto* sm = compilation.inner->getSourceManager();
    if (!sm)
        return out;
    std::string path_owned(path.data(), path.size());
    auto buffer = buffer_for_path(*sm, path_owned);
    if (!buffer)
        return out;
    FindAtOffset finder(*sm, *buffer, offset);
    root.visit(finder);
    if (finder.best)
        fill_symbol(*finder.best, *sm, out);
    return out;
}

namespace {

void collect_instances(
    const slang::ast::Scope& scope,
    const slang::SourceManager& sm,
    rust::Vec<HierInstanceAnswer>& out
) {
    for (const auto& member : scope.members()) {
        if (const auto* inst = member.as_if<slang::ast::InstanceSymbol>()) {
            HierInstanceAnswer row;
            row.path = rust::String(inst->getHierarchicalPath());
            if (inst->location.valid()) {
                row.file = rust::String(assigned_path(sm, inst->location.buffer()));
                row.offset = inst->location.offset();
            }
            out.push_back(std::move(row));
            collect_instances(inst->body, sm, out);
        } else if (const auto* pkg = member.as_if<slang::ast::PackageSymbol>()) {
            collect_instances(*pkg, sm, out);
        } else if (const auto* cu = member.as_if<slang::ast::CompilationUnitSymbol>()) {
            collect_instances(*cu, sm, out);
        } else if (const auto* body = member.as_if<slang::ast::InstanceBodySymbol>()) {
            collect_instances(*body, sm, out);
        }
    }
}

} // namespace

rust::Vec<HierInstanceAnswer> list_instances(Compilation& compilation) {
    rust::Vec<HierInstanceAnswer> out;
    if (!compilation.inner)
        return out;
    const auto& root = compilation.inner->getRoot();
    const auto* sm = compilation.inner->getSourceManager();
    if (!sm)
        return out;
    collect_instances(root, *sm, out);
    return out;
}

} // namespace slang_sys::compilation
