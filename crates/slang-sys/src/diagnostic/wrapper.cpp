#include "diagnostic/wrapper.h"

#include <optional>
#include <span>
#include <string>
#include <type_traits>
#include <variant>

#include "slang/diagnostics/Diagnostics.h"
#include "slang/text/SourceManager.h"

#include "slang-sys/src/diagnostic/ffi.rs.h"

namespace slang_sys::diagnostic::helper {
    static std::optional<slang::SourceRange> map_source_range_to_context(
        const slang::DiagnosticEngine &engine,
        slang::SourceLocation context,
        slang::SourceRange range
    ) {
        if (range == ::slang::SourceRange::NoLocation)
            return std::nullopt;

        slang::SmallVector<slang::SourceRange> mapped;
        engine.mapSourceRanges(context, std::span(&range, 1), mapped, false);
        if (mapped.empty())
            return std::nullopt;

        return mapped.front();
    }

    static ::slang::Diagnostics apply_warning_options(
        slang::DiagnosticEngine &engine,
        const rust::Vec<rust::String> &warning_options
    ) {
        auto options = slang_sys::helper::to_std_strings(warning_options);
        if (options.empty())
            return {};
        return engine.setWarningOptions(options);
    }

    static rust::Vec<rust::String> diagnostic_args(const ::slang::Diagnostic &diag) {
        rust::Vec<rust::String> result;
        for (const auto &arg : diag.args) {
            std::visit(
                [&](auto &&value) {
                    using T = std::decay_t<decltype(value)>;
                    if constexpr (std::is_same_v<T, std::string>)
                        result.emplace_back(rust::String(value));
                    else if constexpr (std::is_same_v<T, int64_t> || std::is_same_v<T, uint64_t>)
                        result.emplace_back(rust::String(std::to_string(value)));
                    else if constexpr (std::is_same_v<T, char>)
                        result.emplace_back(rust::String(std::string(1, value)));
                    else if constexpr (std::is_same_v<T, slang::ConstantValue>)
                        result.emplace_back(rust::String(value.toString()));
                    else
                        result.emplace_back(rust::String());
                },
                arg
            );
        }
        return result;
    }

    static ::slang_sys::diagnostic::RawSyntaxDiagnostic to_rust_syntax_diagnostic(
        const ::slang::Diagnostic &diag,
        slang::DiagnosticEngine &engine,
        const slang::SourceManager &source_manager
    ) {
        ::slang_sys::diagnostic::RawSyntaxDiagnostic rust_diag;
        rust_diag.code = diag.code.getCode();
        rust_diag.subsystem = static_cast<uint16_t>(diag.code.getSubsystem());
        rust_diag.severity = static_cast<uint8_t>(engine.getSeverity(diag.code, diag.location));
        rust_diag.message = rust::String(engine.formatMessage(diag));
        rust_diag.args = diagnostic_args(diag);
        rust_diag.name = rust::String(std::string(slang::toString(diag.code)));
        rust_diag.option_name = rust::String(std::string(engine.getOptionName(diag.code)));
        rust_diag.primary_range_start = 0;
        rust_diag.primary_range_end = 0;
        rust_diag.has_primary_range = false;
        rust_diag.location = 0;
        rust_diag.has_location = false;
        rust_diag.buffer_id = 0;
        rust_diag.has_buffer_id = false;
        rust_diag.file_name = rust::String();

        if (!diag.ranges.empty() && diag.ranges.front() != ::slang::SourceRange::NoLocation &&
            diag.location.valid()) {
            auto location = source_manager.getFullyExpandedLoc(diag.location);
            auto range = map_source_range_to_context(engine, location, diag.ranges.front());
            if (range) {
                rust_diag.primary_range_start = range->start().offset();
                rust_diag.primary_range_end = range->end().offset();
                rust_diag.has_primary_range = true;
            }
        }

        if (diag.location.valid()) {
            auto location = source_manager.getFullyExpandedLoc(diag.location);
            rust_diag.location = location.offset();
            rust_diag.has_location = true;
            rust_diag.buffer_id = location.buffer().getId();
            rust_diag.has_buffer_id = true;
            const auto &full_path = source_manager.getFullPath(location.buffer());
            if (!full_path.empty())
                rust_diag.file_name = rust::String(full_path.string());
        }

        return rust_diag;
    }

} // namespace slang_sys::diagnostic::helper

namespace slang_sys::diagnostic::tree {

    rust::Vec<::slang_sys::diagnostic::RawSyntaxDiagnostic> syntax_tree_diagnostics(
        const syntax::SyntaxTree &tree,
        rust::Vec<rust::String> warning_options
    ) {
        auto &inner = *tree.tree;
        auto &diags = inner.diagnostics();
        return diagnostics_to_rust(diags, tree.session->source_manager, std::move(warning_options));
    }

} // namespace slang_sys::diagnostic::tree

namespace slang_sys::diagnostic {

rust::Vec<RawSyntaxDiagnostic> diagnostics_to_rust(
    const ::slang::Diagnostics& diagnostics,
    const ::slang::SourceManager& source_manager,
    rust::Vec<rust::String> warning_options
) {
    slang::DiagnosticEngine engine(source_manager);
    auto option_diagnostics = helper::apply_warning_options(engine, warning_options);
    auto pragma_diagnostics = engine.setMappingsFromPragmas();
    rust::Vec<RawSyntaxDiagnostic> result;
    result.reserve(option_diagnostics.size() + pragma_diagnostics.size() + diagnostics.size());
    for (const auto &diag : option_diagnostics)
        result.emplace_back(helper::to_rust_syntax_diagnostic(diag, engine, source_manager));
    for (const auto &diag : pragma_diagnostics)
        result.emplace_back(helper::to_rust_syntax_diagnostic(diag, engine, source_manager));
    for (const auto& diag : diagnostics)
        result.emplace_back(helper::to_rust_syntax_diagnostic(diag, engine, source_manager));
    return result;
}

} // namespace slang_sys::diagnostic
