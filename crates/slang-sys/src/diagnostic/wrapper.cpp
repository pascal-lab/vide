#include "diagnostic/wrapper.h"

#include <algorithm>
#include <memory>
#include <optional>
#include <span>
#include <string>
#include <unordered_map>
#include <vector>

#include "slang/diagnostics/DiagnosticClient.h"
#include "slang/diagnostics/DiagnosticEngine.h"
#include "slang/diagnostics/Diagnostics.h"
#include "slang/text/SourceManager.h"

#include "slang-sys/src/diagnostic/ffi.rs.h"

namespace slang_sys::diagnostic::helper {
    static ::slang_sys::diagnostic::RawDiagnosticLocation raw_location(
        ::slang::SourceLocation location,
        const ::slang::SourceManager &source_manager
    ) {
        ::slang_sys::diagnostic::RawDiagnosticLocation result;
        result.offset = 0;
        result.buffer_id = 0;
        result.has_location = false;
        result.file_name = rust::String();
        if (!location.valid())
            return result;

        result.offset = location.offset();
        result.buffer_id = location.buffer().getId();
        result.has_location = true;
        const auto &full_path = source_manager.getFullPath(location.buffer());
        if (!full_path.empty())
            result.file_name = rust::String(full_path.string());
        else {
            const auto raw_name = source_manager.getRawFileName(location.buffer());
            result.file_name = rust::String(raw_name.data(), raw_name.size());
        }
        return result;
    }

    static ::slang_sys::diagnostic::RawDiagnosticRange raw_range(
        ::slang::SourceRange range
    ) {
        ::slang_sys::diagnostic::RawDiagnosticRange result;
        result.start = 0;
        result.end = 0;
        result.start_buffer_id = 0;
        result.end_buffer_id = 0;
        result.has_range = false;
        if (range == ::slang::SourceRange::NoLocation || !range.start().valid() ||
            !range.end().valid())
            return result;

        result.start = range.start().offset();
        result.end = range.end().offset();
        result.start_buffer_id = range.start().buffer().getId();
        result.end_buffer_id = range.end().buffer().getId();
        result.has_range = true;
        return result;
    }

    static rust::Vec<::slang_sys::diagnostic::RawDiagnosticRange> mapped_ranges(
        const slang::DiagnosticEngine &engine,
        slang::SourceLocation context,
        std::span<const slang::SourceRange> ranges
    ) {
        slang::SmallVector<slang::SourceRange> mapped;
        if (context.valid())
            engine.mapSourceRanges(context, ranges, mapped, false);
        else
            mapped.insert(mapped.end(), ranges.begin(), ranges.end());

        rust::Vec<::slang_sys::diagnostic::RawDiagnosticRange> result;
        result.reserve(mapped.size());
        for (const auto &range : mapped) {
            auto raw = raw_range(range);
            if (raw.has_range)
                result.emplace_back(std::move(raw));
        }
        return result;
    }

    static std::optional<slang::SourceRange> map_source_ranges_to_context(
        const slang::DiagnosticEngine &engine,
        slang::SourceLocation context,
        std::span<const slang::SourceRange> ranges
    ) {
        if (ranges.empty())
            return std::nullopt;

        slang::SmallVector<slang::SourceRange> mapped;
        engine.mapSourceRanges(context, ranges, mapped, false);
        if (mapped.empty())
            return std::nullopt;

        auto total = mapped.front();
        const auto buffer_id = total.start().buffer().getId();
        if (total.end().buffer().getId() != buffer_id)
            return std::nullopt;

        for (const auto &range : mapped) {
            if (range.start().buffer().getId() != buffer_id ||
                range.end().buffer().getId() != buffer_id) {
                // The Rust diagnostic model has one primary range and one
                // source buffer. Do not guess when Slang maps related ranges
                // across buffers; the server reports this as unsupported too.
                return std::nullopt;
            }
            total.start() = std::min(total.start(), range.start());
            total.end() = std::max(total.end(), range.end());
        }

        return total;
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

    static rust::Vec<rust::String> diagnostic_args(
        const ::slang::Diagnostic &diag,
        const ::slang::DiagnosticEngine &engine
    ) {
        rust::Vec<rust::String> result;
        result.reserve(diag.args.size());
        for (const auto &arg : diag.args) {
            // Use Slang's formatter for every variant, including custom
            // argument types. An empty string would silently corrupt the
            // Rust-side diagnostic arguments when a new Slang argument type
            // is introduced or a formatter is missing.
            result.emplace_back(rust::String(engine.formatArg(arg)));
        }
        return result;
    }

    static ::slang_sys::diagnostic::RawSyntaxDiagnostic to_rust_reported_diagnostic(
        const ::slang::ReportedDiagnostic &reported,
        const slang::DiagnosticEngine &engine,
        const slang::SourceManager &source_manager,
        std::span<const slang::SourceLocation> include_stack,
        uint32_t diagnostic_id,
        uint32_t parent_diagnostic_id
    ) {
        ::slang_sys::diagnostic::RawSyntaxDiagnostic rust_diag;
        const auto &diag = reported.originalDiagnostic;
        rust_diag.code = diag.code.getCode();
        rust_diag.subsystem = static_cast<uint16_t>(diag.code.getSubsystem());
        rust_diag.severity = static_cast<uint8_t>(reported.severity);
        rust_diag.message = rust::String(
            reported.formattedMessage.data(), reported.formattedMessage.size());
        rust_diag.args = diagnostic_args(diag, engine);
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
        rust_diag.ranges = mapped_ranges(engine, reported.location, reported.ranges);
        rust_diag.expansion_locations = rust::Vec<
            ::slang_sys::diagnostic::RawDiagnosticExpansion>();
        rust_diag.include_stack = rust::Vec<::slang_sys::diagnostic::RawDiagnosticLocation>();
        rust_diag.diagnostic_id = diagnostic_id;
        rust_diag.parent_diagnostic_id = parent_diagnostic_id;

        if (!reported.ranges.empty()) {
            auto range = map_source_ranges_to_context(
                engine, reported.location, reported.ranges);
            if (range) {
                rust_diag.primary_range_start = range->start().offset();
                rust_diag.primary_range_end = range->end().offset();
                rust_diag.has_primary_range = true;
            }
        }

        if (reported.location.valid()) {
            auto location = reported.location;
            rust_diag.location = location.offset();
            rust_diag.has_location = true;
            rust_diag.buffer_id = location.buffer().getId();
            rust_diag.has_buffer_id = true;
            const auto &full_path = source_manager.getFullPath(location.buffer());
            if (!full_path.empty())
                rust_diag.file_name = rust::String(full_path.string());
        }

        for (const auto &location : reported.expansionLocs) {
            const auto macro_name = source_manager.getMacroName(location);
            rust_diag.expansion_locations.emplace_back(
                ::slang_sys::diagnostic::RawDiagnosticExpansion {
                    raw_location(location, source_manager),
                    raw_location(source_manager.getFullyOriginalLoc(location), source_manager),
                    rust::String(macro_name.data(), macro_name.size()),
                }
            );
        }
        for (const auto &location : include_stack)
            rust_diag.include_stack.emplace_back(raw_location(location, source_manager));

        return rust_diag;
    }

} // namespace slang_sys::diagnostic::helper

namespace slang_sys::diagnostic {

class CollectingDiagnosticClient final : public ::slang::DiagnosticClient {
  public:
    void report(const ::slang::ReportedDiagnostic &reported) override {
        const auto *diagnostic = &reported.originalDiagnostic;
        auto id = diagnostic_ids.find(diagnostic);
        uint32_t diagnostic_id;
        uint32_t parent_diagnostic_id = 0;
        if (id == diagnostic_ids.end()) {
            diagnostic_id = next_diagnostic_id++;
            diagnostic_ids.emplace(diagnostic, diagnostic_id);
        } else {
            diagnostic_id = id->second;
            auto parent = parent_ids.find(diagnostic);
            if (parent != parent_ids.end())
                parent_diagnostic_id = parent->second;
        }

        reserve_note_ids(diagnostic->notes, diagnostic_id);

        slang::SmallVector<slang::SourceLocation> include_stack;
        if (reported.shouldShowIncludeStack && reported.location.valid())
            getIncludeStack(reported.location.buffer(), include_stack);

        diagnostics.emplace_back(helper::to_rust_reported_diagnostic(
            reported,
            *engine,
            *sourceManager,
            include_stack,
            diagnostic_id,
            parent_diagnostic_id
        ));
    }

    rust::Vec<RawSyntaxDiagnostic> take() {
        rust::Vec<RawSyntaxDiagnostic> result;
        result.reserve(diagnostics.size());
        for (auto &diagnostic : diagnostics)
            result.emplace_back(std::move(diagnostic));
        return result;
    }

  private:
    void reserve_note_ids(
        const std::vector<::slang::Diagnostic> &notes,
        uint32_t parent_diagnostic_id
    ) {
        for (const auto &note : notes) {
            const auto *diagnostic = &note;
            if (diagnostic_ids.contains(diagnostic))
                continue;
            const auto diagnostic_id = next_diagnostic_id++;
            diagnostic_ids.emplace(diagnostic, diagnostic_id);
            parent_ids.emplace(diagnostic, parent_diagnostic_id);
            reserve_note_ids(note.notes, diagnostic_id);
        }
    }

    std::vector<RawSyntaxDiagnostic> diagnostics;
    std::unordered_map<const ::slang::Diagnostic *, uint32_t> diagnostic_ids;
    std::unordered_map<const ::slang::Diagnostic *, uint32_t> parent_ids;
    uint32_t next_diagnostic_id = 1;
};

} // namespace slang_sys::diagnostic

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
    auto client = std::make_shared<CollectingDiagnosticClient>();
    engine.addClient(client);
    auto option_diagnostics = helper::apply_warning_options(engine, warning_options);
    auto pragma_diagnostics = engine.setMappingsFromPragmas();
    engine.issue(option_diagnostics);
    engine.issue(pragma_diagnostics);
    engine.issue(diagnostics);
    return client->take();
}

} // namespace slang_sys::diagnostic
