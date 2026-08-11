#include "wrapper.h"

#include "slang-sys/src/syntax/ffi.rs.h"

#include <algorithm>
#include <filesystem>
#include <functional>
#include <optional>
#include <span>
#include <stdexcept>
#include <string>
#include <string_view>
#include <unordered_map>
#include <unordered_set>

namespace slang_sys::syntax::helper {

    static slang::SourceRange node_range(const SyntaxNode *node) {
        return node->sourceRange();
    }

    static slang::SourceRange token_range(const SyntaxToken *token) {
        return token->range();
    }

    static bool source_range_valid(slang::SourceRange range) {
        return range != slang::SourceRange::NoLocation && range.start().valid() &&
               range.end().valid();
    }

    static const SyntaxNode *find_root(const SyntaxNode *node) {
        while (node && node->parent.get())
            node = node->parent.get();
        return node;
    }

    static slang::SourceRange map_range_with_context(
        slang::SourceRange range,
        const SyntaxNode *context,
        const SyntaxTree &owner
    ) {
        if (!context || !source_range_valid(range))
            return slang::SourceRange::NoLocation;

        auto root = find_root(context);
        if (!root)
            return slang::SourceRange::NoLocation;

        if (root != &owner.root())
            throw std::invalid_argument("syntax context does not belong to its owner tree");

        auto root_range = root->sourceRange();
        if (!source_range_valid(root_range))
            return slang::SourceRange::NoLocation;

        slang::DiagnosticEngine engine(owner.session->source_manager);
        slang::SmallVector<slang::SourceRange> mapped;
        engine.mapSourceRanges(root_range.start(), std::span(&range, 1), mapped, false);
        if (mapped.empty())
            return slang::SourceRange::NoLocation;
        return mapped.front();
    }

    static slang::SourceRange node_range_with_context(
        const SyntaxNode *node,
        const SyntaxNode *context,
        const SyntaxTree &owner
    ) {
        return map_range_with_context(node_range(node), context, owner);
    }

    static slang::SourceRange token_range_with_context(
        const SyntaxToken *token,
        const SyntaxNode *context,
        const SyntaxTree &owner
    ) {
        return map_range_with_context(token_range(token), context, owner);
    }

    static SyntaxTrivia trivia_at(const SyntaxToken *token, std::size_t index) {
        auto trivia = token->trivia();
        if (index >= trivia.size())
            throw std::out_of_range("Slang token trivia index is out of bounds");
        return trivia[index];
    }

    static std::optional<slang::SourceLocation> explicit_location(const SyntaxTrivia &trivia) {
        return trivia.getExplicitLocation();
    }

} // namespace slang_sys::syntax::helper
namespace slang_sys::syntax {
    SourceSession::SourceSession() {
        source_manager.setDisableProximatePaths(true);
    }

    void SourceSession::assign_include_buffer(std::string path, std::string text) {
        auto [it, inserted] = include_buffers.emplace(path, text);
        if (!inserted) {
            if (it->second != text)
                throw std::logic_error("include buffer path was assigned conflicting text");
            return;
        }
        source_manager.assignText(it->first, it->second);
    }

    SyntaxTree::SyntaxTree(
        std::shared_ptr<::slang::syntax::SyntaxTree> tree,
        std::shared_ptr<SourceSession> session,
        uint32_t root_buffer_id
    )
        : tree(std::move(tree)), session(std::move(session)), root_buffer_id(root_buffer_id) {}

    SyntaxTree::~SyntaxTree() = default;

    const SyntaxNode &SyntaxTree::root() const {
        return tree->root();
    }
} // namespace slang_sys::syntax

namespace slang_sys::syntax::tree {

    std::shared_ptr<SyntaxTree> parse_syntax_tree(
        rust::Str text,
        rust::Str name,
        rust::Str path,
        rust::Vec<rust::String> predefines,
        rust::Vec<rust::String> include_paths,
        rust::Vec<rust::String> include_buffer_paths,
        rust::Vec<rust::String> include_buffer_texts,
        bool expand_includes,
        bool guess,
        bool collect_expected_syntax
    ) {
        return parse_syntax_tree_with_session(
            std::make_shared<SourceSession>(),
            text,
            name,
            path,
            std::move(predefines),
            std::move(include_paths),
            std::move(include_buffer_paths),
            std::move(include_buffer_texts),
            expand_includes,
            guess,
            collect_expected_syntax
        );
    }

    std::shared_ptr<SyntaxTree> parse_syntax_tree_with_session(
        const std::shared_ptr<SourceSession>& session,
        rust::Str text,
        rust::Str name,
        rust::Str path,
        rust::Vec<rust::String> predefines,
        rust::Vec<rust::String> include_paths,
        rust::Vec<rust::String> include_buffer_paths,
        rust::Vec<rust::String> include_buffer_texts,
        bool expand_includes,
        bool guess,
        bool collect_expected_syntax
    ) {
        auto source_storage = std::string(text.data(), text.size());
        auto source = std::string_view(source_storage);
        auto tree_name = std::string_view(name.data(), name.size());
        auto tree_path = std::string_view(path.data(), path.size());

        if (include_buffer_paths.size() != include_buffer_texts.size())
            throw std::invalid_argument("include buffer paths and texts must have equal lengths");

        slang::Bag options;
        auto &pp_options = options.insertOrGet<slang::parsing::PreprocessorOptions>();
        for (const auto &predefine : predefines)
            pp_options.predefines.emplace_back(std::string(predefine));
        for (const auto &include_path : include_paths)
            pp_options.additionalIncludePaths.emplace_back(
                std::filesystem::path(std::string(include_path))
            );
        if (!expand_includes)
            pp_options.maxIncludeDepth = 0;

        if (collect_expected_syntax) {
            auto &expected_options = options.insertOrGet<slang::parsing::ExpectedSyntaxOptions>();
            expected_options.recordAll = true;
        }

        for (std::size_t i = 0; i < include_buffer_paths.size(); i++) {
            session->assign_include_buffer(
                std::string(include_buffer_paths[i]),
                std::string(include_buffer_texts[i])
            );
        }
        std::shared_ptr<::slang::syntax::SyntaxTree> tree;
        if (guess) {
            tree = ::slang::syntax::SyntaxTree::fromText(
                source,
                session->source_manager,
                tree_name,
                tree_path,
                options
            );
        }
        else {
            tree = ::slang::syntax::SyntaxTree::fromFileInMemory(
                source,
                session->source_manager,
                tree_name,
                tree_path,
                options
            );
        }
        if (!tree)
            throw std::logic_error("Slang failed to create syntax tree");
        auto source_buffers = tree->getSourceBufferIds();
        if (source_buffers.empty())
            throw std::logic_error("Slang syntax tree has no root source buffer");
        auto root_buffer_id = source_buffers.front().getId();
        auto result = std::make_shared<SyntaxTree>(
            std::move(tree), std::move(session), root_buffer_id);
        if (!result->tree)
            throw std::logic_error("Slang syntax wrapper lost its syntax tree");
        return result;
    }

    const SyntaxNode *syntax_tree_root(const SyntaxTree &tree) {
        return &tree.root();
    }

    uint32_t syntax_tree_root_buffer_id(const SyntaxTree &tree) {
        return tree.root_buffer_id;
    }

    rust::Vec<RawExpectedSyntax> syntax_tree_expected_syntax_at(
        const SyntaxTree &tree,
        std::size_t offset
    ) {
        rust::Vec<RawExpectedSyntax> result;
        const auto root_range = tree.tree->root().sourceRange();
        if (root_range == ::slang::SourceRange::NoLocation || !root_range.start().valid())
            throw std::logic_error("Slang syntax tree root has no source location");

        const auto root_buffer = root_range.start().buffer();
        const auto &expected_syntax = tree.tree->getMetadata().expectedSyntax;
        for (const auto &expected : expected_syntax) {
            if (!expected.location.valid() || expected.location.buffer() != root_buffer)
                continue;

            const auto start = expected.location.offset();
            if (start > offset || expected.end < offset)
                continue;

            const auto duplicate = std::any_of(
                result.begin(), result.end(), [&](const RawExpectedSyntax &existing) {
                    return existing.code == expected.code.getCode() &&
                           existing.subsystem == static_cast<uint16_t>(expected.code.getSubsystem()) &&
                           existing.token_kind == static_cast<uint16_t>(expected.tokenKind) &&
                           existing.has_keyword_context == expected.keywordContext.has_value() &&
                           (!expected.keywordContext ||
                            existing.keyword_context ==
                                static_cast<uint8_t>(*expected.keywordContext));
                });
            if (duplicate)
                continue;

            RawExpectedSyntax raw {
                expected.code.getCode(),
                static_cast<uint16_t>(expected.code.getSubsystem()),
                static_cast<uint16_t>(expected.tokenKind),
                static_cast<uint8_t>(expected.keywordContext ?
                                         static_cast<uint8_t>(*expected.keywordContext) : 0),
                expected.keywordContext.has_value(),
                expected.location.offset(),
                expected.end,
                true,
                true,
                expected.location.buffer().getId(),
                true,
            };
            result.emplace_back(std::move(raw));
        }
        return result;
    }

    std::shared_ptr<SyntaxTree> parse_library_map_syntax_tree(
        rust::Str text,
        rust::Str name,
        rust::Str path,
        bool collect_expected_syntax
    ) {
        return parse_library_map_syntax_tree_with_session(
            std::make_shared<SourceSession>(), text, name, path, collect_expected_syntax);
    }

    std::shared_ptr<SyntaxTree> parse_library_map_syntax_tree_with_session(
        const std::shared_ptr<SourceSession>& session,
        rust::Str text,
        rust::Str name,
        rust::Str path,
        bool collect_expected_syntax
    ) {
        auto source = std::string_view(text.data(), text.size());
        auto tree_name = std::string_view(name.data(), name.size());
        auto tree_path = std::string_view(path.data(), path.size());
        slang::Bag options;
        if (collect_expected_syntax) {
            auto &expected_options = options.insertOrGet<slang::parsing::ExpectedSyntaxOptions>();
            expected_options.recordAll = true;
        }
        auto tree = ::slang::syntax::SyntaxTree::fromLibraryMapText(
            source,
            session->source_manager,
            tree_name,
            tree_path,
            options
        );
        if (!tree)
            throw std::logic_error("Slang failed to create library map syntax tree");
        auto source_buffers = tree->getSourceBufferIds();
        if (source_buffers.empty())
            throw std::logic_error("Slang library map tree has no root source buffer");
        auto root_buffer_id = source_buffers.front().getId();
        return std::make_shared<SyntaxTree>(
            std::move(tree), std::move(session), root_buffer_id);
    }

    namespace {

    RawTraceSourceRange empty_trace_range() {
        return RawTraceSourceRange { 0, 0, 0, false };
    }

    RawTraceSourceRange trace_range(slang::SourceRange range) {
        if (range == slang::SourceRange::NoLocation || !range.start().valid() ||
            !range.end().valid() || range.start().buffer() != range.end().buffer())
            return empty_trace_range();
        return RawTraceSourceRange {
            range.start().buffer().getId(),
            range.start().offset(),
            range.end().offset(),
            true,
        };
    }

    RawTraceToken empty_trace_token() {
        return RawTraceToken {
            rust::String(), rust::String(),
            static_cast<uint16_t>(slang::parsing::TokenKind::Unknown),
            empty_trace_range(), false,
        };
    }

    RawTraceToken trace_token(slang::parsing::Token token) {
        if (!token)
            return empty_trace_token();
        return RawTraceToken {
            rust::String(token.rawText().data(), token.rawText().size()),
            rust::String(token.valueText().data(), token.valueText().size()),
            static_cast<uint16_t>(token.kind), trace_range(token.range()), true,
        };
    }

    RawTraceToken trace_token_with_original_range(
        slang::parsing::Token token,
        const slang::SourceManager& source_manager
    ) {
        if (!token)
            return empty_trace_token();
        return RawTraceToken {
            rust::String(token.rawText().data(), token.rawText().size()),
            rust::String(token.valueText().data(), token.valueText().size()),
            static_cast<uint16_t>(token.kind),
            trace_range(source_manager.getFullyOriginalRange(token.range())),
            true,
        };
    }

    void append_trace_trivia(std::string& result, slang::parsing::Trivia trivia) {
        if (trivia.kind == slang::parsing::TriviaKind::Directive) {
            auto* syntax = trivia.syntax();
            if (!syntax)
                throw std::logic_error("Slang directive trivia has no syntax node");
            for (auto nested : syntax->getFirstToken().trivia())
                append_trace_trivia(result, nested);
            return;
        }
        if (trivia.kind == slang::parsing::TriviaKind::SkippedSyntax ||
            trivia.kind == slang::parsing::TriviaKind::SkippedTokens)
            return;
        auto raw = trivia.getRawText();
        result.append(raw.data(), raw.size());
    }

    std::string trace_display_text(slang::parsing::Token token) {
        std::string result;
        for (auto trivia : token.trivia())
            append_trace_trivia(result, trivia);
        auto raw = token.rawText();
        result.append(raw.data(), raw.size());
        return result;
    }

    RawTraceTokenOrigin empty_trace_origin() {
        return RawTraceTokenOrigin {
            0, rust::String(),
            0, false, 0, false, 0, false, 0, false,
            0, false, 0, false, 0, false,
            empty_trace_range(), empty_trace_range(), empty_trace_range(), empty_trace_range(),
        };
    }

    RawTraceEvent empty_trace_event(uint32_t event_id, const slang::syntax::SyntaxNode& node) {
        return RawTraceEvent {
            event_id,
            static_cast<uint16_t>(node.kind),
            trace_range(node.sourceRange()),
            0,
            0, false, 0, false, 0, false, 0, false,
            empty_trace_token(), empty_trace_token(), empty_trace_token(),
            rust::Vec<RawTraceMacroParam>(), rust::Vec<RawTraceActualArgument>(),
            rust::Vec<RawTraceToken>(), rust::Vec<RawTraceToken>(),
            rust::Vec<RawTraceSourceRange>(),
        };
    }

    void collect_nodes(
        slang::syntax::SyntaxNode& node,
        std::vector<const slang::syntax::SyntaxNode*>& nodes,
        std::unordered_set<const slang::syntax::SyntaxNode*>& seen
    ) {
        if (!seen.insert(&node).second)
            return;
        nodes.push_back(&node);
        for (size_t i = 0; i < node.getChildCount(); i++) {
            if (auto child = node.childNode(i))
                collect_nodes(*child, nodes, seen);
            if (auto* token = node.childTokenPtr(i)) {
                for (auto trivia : token->trivia()) {
                    if (auto* syntax = trivia.syntax())
                        collect_nodes(*syntax, nodes, seen);
                }
            }
        }
    }

    void append_leaf_trace_tokens(
        const slang::syntax::SyntaxNode& node,
        rust::Vec<RawTraceToken>& target
    ) {
        for (size_t i = 0; i < node.getChildCount(); i++) {
            if (auto child = node.childNode(i))
                append_leaf_trace_tokens(*child, target);
            else if (auto token = node.childToken(i))
                target.emplace_back(trace_token(token));
        }
    }

    template<typename Tokens>
    void append_trace_tokens(const Tokens& source, rust::Vec<RawTraceToken>& target) {
        for (auto token : source)
            target.emplace_back(trace_token(token));
    }

    RawTraceSourceRange token_list_range(const auto& tokens) {
        std::optional<slang::SourceRange> result;
        for (auto token : tokens) {
            auto range = token.range();
            if (range == slang::SourceRange::NoLocation || !range.start().valid() ||
                !range.end().valid())
                continue;
            if (!result)
                result = range;
            else if (result->start().buffer() == range.start().buffer())
                *result = slang::SourceRange(
                    std::min(result->start(), range.start()),
                    std::max(result->end(), range.end())
                );
        }
        return result ? trace_range(*result) : empty_trace_range();
    }

    RawTraceActualArgument trace_actual_argument_with_original_ranges(
        const slang::syntax::MacroActualArgumentSyntax& argument,
        const slang::SourceManager& source_manager
    ) {
        RawTraceActualArgument result {
            rust::Vec<RawTraceToken>(),
            trace_range(source_manager.getFullyOriginalRange(argument.sourceRange()))
        };
        for (auto token : argument.tokens)
            result.tokens.emplace_back(trace_token_with_original_range(token, source_manager));
        return result;
    }

    RawTraceMacroParam trace_macro_param(
        const slang::syntax::MacroFormalArgumentSyntax& param
    ) {
        RawTraceMacroParam result {
            trace_token(param.name), rust::Vec<RawTraceToken>(),
            param.defaultValue != nullptr, trace_range(param.sourceRange())
        };
        if (param.defaultValue)
            append_trace_tokens(param.defaultValue->tokens, result.default_tokens);
        return result;
    }

    void append_disabled_ranges(
        const auto& tokens,
        rust::Vec<RawTraceSourceRange>& target
    ) {
        std::optional<slang::SourceRange> combined;
        for (auto token : tokens) {
            auto range = token.range();
            if (range == slang::SourceRange::NoLocation || !range.start().valid() ||
                !range.end().valid())
                continue;
            if (!combined) {
                combined = range;
                continue;
            }
            if (combined->start().buffer() != range.start().buffer()) {
                target.emplace_back(trace_range(*combined));
                combined = range;
                continue;
            }
            *combined = slang::SourceRange(
                std::min(combined->start(), range.start()),
                std::max(combined->end(), range.end())
            );
        }
        if (combined)
            target.emplace_back(trace_range(*combined));
    }

    std::string trace_macro_name(slang::parsing::Token token) {
        auto value = token.valueText();
        if (!value.empty() && value.front() == '`')
            value.remove_prefix(1);
        if (!value.empty() && value.front() == '\\')
            value.remove_prefix(1);
        return std::string(value.data(), value.size());
    }

    struct TraceCallKey {
        uint32_t buffer_id;
        size_t start;
        size_t end;
        bool operator==(const TraceCallKey& other) const {
            return buffer_id == other.buffer_id && start == other.start && end == other.end;
        }
    };

    struct TraceCallKeyHash {
        size_t operator()(const TraceCallKey& key) const {
            return std::hash<uint32_t>()(key.buffer_id) ^
                   (std::hash<size_t>()(key.start) << 1) ^
                   (std::hash<size_t>()(key.end) << 2);
        }
    };

    struct TraceCallInfo {
        uint32_t call_id;
        uint32_t expansion_id;
        RawTraceSourceRange source_range;
    };

    TraceCallKey call_key(RawTraceSourceRange range) {
        return TraceCallKey { range.buffer_id, range.range_start, range.range_end };
    }

    const TraceCallInfo *find_call_at_location(
        slang::SourceLocation location,
        const std::unordered_map<TraceCallKey, TraceCallInfo, TraceCallKeyHash> &calls,
        RawTraceSourceRange &call_range
    ) {
        if (!location.valid())
            return nullptr;
        const TraceCallInfo *result = nullptr;
        for (const auto &[key, value] : calls) {
            if (key.buffer_id == location.buffer().getId() && key.start == location.offset()) {
                if (result)
                    throw std::logic_error("Slang macro location matches multiple macro calls");
                call_range = value.source_range;
                result = &value;
            }
        }
        return result;
    }

    const TraceCallInfo *find_call(
        const slang::SourceManager &source_manager,
        slang::SourceLocation location,
        const std::unordered_map<TraceCallKey, TraceCallInfo, TraceCallKeyHash> &calls,
        RawTraceSourceRange &call_range
    ) {
        auto current = location;
        while (source_manager.isMacroLoc(current)) {
            auto expansion = source_manager.getExpansionRange(current);
            auto range = trace_range(expansion);
            if (range.has_range) {
                auto it = calls.find(call_key(range));
                if (it != calls.end()) {
                    call_range = it->second.source_range;
                    return &it->second;
                }
            }
            current = expansion.start();
        }
        return find_call_at_location(current, calls, call_range);
    }

    const TraceCallInfo *find_parent_call(
        const slang::SourceManager &source_manager,
        slang::SourceLocation location,
        uint32_t child_call_id,
        const std::unordered_map<TraceCallKey, TraceCallInfo, TraceCallKeyHash> &calls,
        RawTraceSourceRange &call_range
    ) {
        auto current = location;
        while (source_manager.isMacroLoc(current)) {
            auto expansion = source_manager.getExpansionRange(current);
            auto range = trace_range(expansion);
            if (range.has_range) {
                auto it = calls.find(call_key(range));
                if (it != calls.end() && it->second.call_id != child_call_id) {
                    call_range = it->second.source_range;
                    return &it->second;
                }
            }
            current = expansion.start();
        }

        auto parent = find_call_at_location(current, calls, call_range);
        if (parent && parent->call_id != child_call_id)
            return parent;
        return nullptr;
    }

    RawTraceSourceRange original_token_range(
        const slang::SourceManager& source_manager,
        slang::SourceLocation location,
        size_t token_length
    ) {
        if (!location.valid())
            return empty_trace_range();
        auto original = source_manager.getFullyOriginalLoc(location);
        if (!original.valid())
            return empty_trace_range();
        return trace_range(slang::SourceRange(original, original + token_length));
    }

    RawTraceEmittedToken trace_emitted_token(
        slang::parsing::Token token,
        const slang::SourceManager& source_manager,
        uint32_t emitted_index,
        const std::unordered_map<TraceCallKey, TraceCallInfo, TraceCallKeyHash>& calls,
        const std::unordered_map<uint32_t, slang::parsing::MacroUsageOrigin>& call_origins,
        const std::unordered_map<uint32_t, uint32_t>& call_definitions,
        const std::unordered_map<uint32_t, std::string>& call_names
    ) {
        RawTraceTokenOrigin origin = empty_trace_origin();
        auto location = token.location();
        auto range = trace_range(token.range());
        auto macro_operation = token.macroOperation();
        auto set_id = [&](uint32_t& value, bool& present, uint32_t id) {
            value = id;
            present = id != 0;
        };

        if (location.valid() && source_manager.isMacroLoc(location)) {
            RawTraceSourceRange call_range = empty_trace_range();
            auto call = find_call(source_manager, location, calls, call_range);
            if (!call)
                throw std::logic_error("Slang macro token has no recorded macro call");
            auto call_origin = call_origins.find(call->call_id);
            if (call_origin == call_origins.end())
                throw std::logic_error("Slang macro call has no recorded origin");
            auto call_name = call_names.find(call->call_id);
            if (call_name == call_names.end())
                throw std::logic_error("Slang macro call has no recorded name");
            origin.macro_name = rust::String(call_name->second);
            set_id(origin.macro_call_id, origin.has_macro_call_id, call->call_id);
            set_id(origin.macro_expansion_id, origin.has_macro_expansion_id, call->expansion_id);
            if (auto definition = call_definitions.find(call->call_id);
                definition != call_definitions.end())
                set_id(origin.macro_definition_id, origin.has_macro_definition_id, definition->second);
            if (call_origin->second == slang::parsing::MacroUsageOrigin::Source &&
                !origin.has_macro_definition_id)
                throw std::logic_error(
                    "Slang source macro call has no definition id: " +
                    std::to_string(call->call_id)
                );
            origin.call_range = call_range;
            origin.token_range = range;
            auto original = source_manager.getOriginalLoc(location);
            auto original_range = original_token_range(source_manager, original, token.rawText().size());
            auto parent_call_range = empty_trace_range();
            auto parent_call = find_parent_call(
                source_manager, location, call->call_id, calls, parent_call_range);
            if (parent_call) {
                set_id(
                    origin.parent_macro_expansion_id,
                    origin.has_parent_macro_expansion_id,
                    parent_call->expansion_id
                );
            }
            if (source_manager.isMacroArgLoc(location)) {
                auto token_origin = token.macroOrigin();
                switch (call_origin->second) {
                    case slang::parsing::MacroUsageOrigin::Source:
                        origin.kind = 3;
                        break;
                    case slang::parsing::MacroUsageOrigin::Predefine:
                        origin.kind = 7;
                        break;
                    case slang::parsing::MacroUsageOrigin::BuiltIn:
                    case slang::parsing::MacroUsageOrigin::Intrinsic:
                        origin.kind = 4;
                        break;
                    case slang::parsing::MacroUsageOrigin::Unknown:
                        throw std::logic_error("Slang macro argument has unknown macro origin");
                }
                origin.argument_token_range = original_range;
                origin.body_token_range = trace_range(source_manager.getFullyOriginalRange(
                    source_manager.getExpansionRange(location)));
                origin.body_token_index = token_origin.bodyTokenIndex;
                origin.has_body_token_index = token_origin.hasBodyTokenIndex;
                origin.argument_index = token_origin.argumentIndex;
                origin.has_argument_index = token_origin.hasArgumentIndex;
                origin.argument_token_index = token_origin.argumentTokenIndex;
                origin.has_argument_token_index = token_origin.hasArgumentTokenIndex;
                if (macro_operation == slang::parsing::Token::MacroOperation::None &&
                    call_origin->second == slang::parsing::MacroUsageOrigin::Source &&
                    (!origin.has_body_token_index || !origin.has_argument_index ||
                     !origin.has_argument_token_index))
                    throw std::logic_error(
                        "Slang source macro argument has incomplete token origin metadata: " +
                        std::to_string(call->call_id)
                    );
            } else {
                auto token_origin = token.macroOrigin();
                switch (call_origin->second) {
                    case slang::parsing::MacroUsageOrigin::Source:
                        origin.kind = 2;
                        break;
                    case slang::parsing::MacroUsageOrigin::Predefine:
                        origin.kind = 7;
                        break;
                    case slang::parsing::MacroUsageOrigin::BuiltIn:
                    case slang::parsing::MacroUsageOrigin::Intrinsic:
                        origin.kind = 4;
                        break;
                    case slang::parsing::MacroUsageOrigin::Unknown:
                        throw std::logic_error("Slang macro body has unknown macro origin");
                }
                origin.body_token_range = original_range;
                origin.body_token_index = token_origin.bodyTokenIndex;
                origin.has_body_token_index = token_origin.hasBodyTokenIndex;
                if (macro_operation == slang::parsing::Token::MacroOperation::None &&
                    call_origin->second == slang::parsing::MacroUsageOrigin::Source &&
                    !origin.has_body_token_index)
                    throw std::logic_error(
                        "Slang source macro body has no token origin metadata: " +
                        std::to_string(call->call_id)
                    );
            }
            if (macro_operation == slang::parsing::Token::MacroOperation::TokenPaste)
                origin.kind = 5;
            else if (macro_operation == slang::parsing::Token::MacroOperation::Stringify)
                origin.kind = 6;
        } else if (range.has_range) {
            origin.kind = 1;
            origin.token_range = range;
        }

        return RawTraceEmittedToken {
            emitted_index, true,
            rust::String(token.rawText().data(), token.rawText().size()),
            rust::String(token.valueText().data(), token.valueText().size()),
            rust::String(trace_display_text(token)),
            static_cast<uint16_t>(token.kind), std::move(origin)
        };
    }

    } // namespace

    RawTrace syntax_tree_preprocessor_trace(const SyntaxTree& tree) {
        RawTrace result {
            syntax_tree_root_buffer_id(tree),
            rust::Vec<RawTraceSourceBuffer>(),
            rust::Vec<RawTraceEvent>(),
            rust::Vec<RawTraceIncludeEdge>(),
            rust::Vec<RawTraceEmittedToken>(),
        };

        std::vector<const slang::syntax::SyntaxNode*> nodes;
        std::unordered_set<const slang::syntax::SyntaxNode*> seen;
        collect_nodes(tree.tree->root(), nodes, seen);
        std::unordered_set<uint32_t> source_buffer_ids;
        auto add_source_range_buffer = [&](slang::SourceRange range) {
            if (range == slang::SourceRange::NoLocation || !range.start().valid() ||
                !range.end().valid())
                return;
            source_buffer_ids.insert(range.start().buffer().getId());
            source_buffer_ids.insert(range.end().buffer().getId());
        };
        for (auto buffer : tree.tree->getSourceBufferIds())
            source_buffer_ids.insert(buffer.getId());
        for (auto include : tree.tree->getIncludeDirectives())
            source_buffer_ids.insert(include.buffer.id.getId());
        for (auto* node : nodes) {
            add_source_range_buffer(node->sourceRange());
            add_source_range_buffer(
                tree.session->source_manager.getFullyOriginalRange(node->sourceRange()));
        }

        std::unordered_map<const slang::syntax::SyntaxNode*, uint32_t> event_ids;
        std::unordered_map<TraceCallKey, TraceCallInfo, TraceCallKeyHash> calls;
        std::unordered_map<const slang::syntax::DefineDirectiveSyntax*, uint32_t> definitions;
        std::unordered_map<const slang::syntax::SyntaxNode*, slang::parsing::MacroUsageOrigin>
            macro_origins;
        std::unordered_map<const slang::syntax::SyntaxNode*,
                           const slang::syntax::DefineDirectiveSyntax*>
            macro_definitions;
        std::unordered_map<uint32_t, slang::parsing::MacroUsageOrigin> call_origins;
        std::unordered_map<uint32_t, uint32_t> call_definitions;
        std::unordered_map<uint32_t, std::string> call_names;
        uint32_t next_definition = 1;
        for (auto* node : nodes) {
            if (node->kind != slang::syntax::SyntaxKind::DefineDirective)
                continue;
            auto& define = node->as<slang::syntax::DefineDirectiveSyntax>();
            definitions.emplace(&define, next_definition++);
        }
        for (const auto& usage : tree.tree->getMacroUsages()) {
            add_source_range_buffer(usage.syntax->sourceRange());
            add_source_range_buffer(
                tree.session->source_manager.getFullyOriginalRange(usage.syntax->sourceRange()));
            macro_origins.emplace(usage.syntax, usage.origin);
            macro_definitions.emplace(usage.syntax, usage.definition);
            if (usage.definition) {
                add_source_range_buffer(usage.definition->sourceRange());
                add_source_range_buffer(tree.session->source_manager.getFullyOriginalRange(
                    usage.definition->sourceRange()));
            }
            if (usage.definition && !definitions.contains(usage.definition))
                definitions.emplace(usage.definition, next_definition++);
        }
        for (const auto& usage : tree.tree->getMacroUsages()) {
            if (seen.insert(usage.syntax).second)
                nodes.push_back(usage.syntax);
        }
        uint32_t next_event = 1;
        uint32_t next_call = 1;
        for (auto* node : nodes) {
            auto kind = node->kind;
            bool is_event = kind == slang::syntax::SyntaxKind::DefineDirective ||
                            kind == slang::syntax::SyntaxKind::UndefDirective ||
                            kind == slang::syntax::SyntaxKind::IncludeDirective ||
                            kind == slang::syntax::SyntaxKind::IfDefDirective ||
                            kind == slang::syntax::SyntaxKind::IfNDefDirective ||
                            kind == slang::syntax::SyntaxKind::ElsIfDirective ||
                            kind == slang::syntax::SyntaxKind::ElseDirective ||
                            kind == slang::syntax::SyntaxKind::EndIfDirective ||
                            kind == slang::syntax::SyntaxKind::MacroUsage;
            if (!is_event)
                continue;
            uint32_t event_id = next_event++;
            event_ids.emplace(node, event_id);
            auto event = empty_trace_event(event_id, *node);
            if (auto* directive = node->as_if<slang::syntax::DirectiveSyntax>())
                event.directive = trace_token(directive->directive);

            if (kind == slang::syntax::SyntaxKind::DefineDirective) {
                auto& define = node->as<slang::syntax::DefineDirectiveSyntax>();
                auto definition = definitions.find(&define);
                if (definition == definitions.end())
                    throw std::logic_error("Slang define directive has no definition id");
                event.macro_definition_id = definition->second;
                event.has_macro_definition_id = true;
                event.name = trace_token(define.name);
                if (define.formalArguments) {
                    for (auto* param : define.formalArguments->args)
                        if (param)
                            event.params.emplace_back(trace_macro_param(*param));
                }
                append_trace_tokens(define.body, event.body_tokens);
            } else if (kind == slang::syntax::SyntaxKind::UndefDirective) {
                auto& undef = node->as<slang::syntax::UndefDirectiveSyntax>();
                event.name = trace_token(undef.name);
            } else if (kind == slang::syntax::SyntaxKind::IncludeDirective) {
                auto& include = node->as<slang::syntax::IncludeDirectiveSyntax>();
                event.include_file_name = trace_token(include.fileName);
            } else if (kind == slang::syntax::SyntaxKind::MacroUsage) {
                auto& usage = node->as<slang::syntax::MacroUsageSyntax>();
                auto name = trace_macro_name(usage.directive);
                event.range = trace_range(tree.session->source_manager.getFullyOriginalRange(
                    node->sourceRange()));
                event.name = trace_token_with_original_range(
                    usage.directive, tree.session->source_manager);
                auto call_id = next_call++;
                if (auto origin = macro_origins.find(node); origin != macro_origins.end()) {
                    event.macro_origin = static_cast<uint8_t>(origin->second);
                    call_origins[call_id] = origin->second;
                } else {
                    throw std::logic_error("Slang macro usage has no recorded origin");
                }
                call_names[call_id] = name;
                event.macro_call_id = call_id;
                event.has_macro_call_id = true;
                event.macro_expansion_id = call_id;
                event.has_macro_expansion_id = true;
                auto range = trace_range(node->sourceRange());
                if (!range.has_range)
                    throw std::logic_error("Slang macro usage has no source range");
                auto insertion =
                    calls.emplace(call_key(range), TraceCallInfo { call_id, call_id, event.range });
                if (!insertion.second) {
                    throw std::logic_error(
                        "Slang macro usage ranges are not unique: " + name + " at " +
                        std::to_string(range.buffer_id) + ":" +
                        std::to_string(range.range_start) + "-" +
                        std::to_string(range.range_end)
                    );
                }
                if (auto usage = macro_origins.find(node); usage != macro_origins.end() &&
                    usage->second == slang::parsing::MacroUsageOrigin::Source) {
                    auto definition = macro_definitions.find(node);
                    if (definition == macro_definitions.end() || !definition->second)
                        throw std::logic_error("Slang source macro usage has no definition");
                    auto definition_id = definitions.find(definition->second);
                    if (definition_id == definitions.end())
                        throw std::logic_error("Slang source macro usage definition is unindexed");
                    event.macro_definition_id = definition_id->second;
                    event.has_macro_definition_id = true;
                    call_definitions[call_id] = definition_id->second;
                }
                if (usage.args) {
                    for (auto* argument : usage.args->args)
                        if (argument)
                            event.arguments.emplace_back(trace_actual_argument_with_original_ranges(
                                *argument, tree.session->source_manager));
                }
            } else if (kind == slang::syntax::SyntaxKind::IfDefDirective ||
                       kind == slang::syntax::SyntaxKind::IfNDefDirective ||
                       kind == slang::syntax::SyntaxKind::ElsIfDirective) {
                auto& branch = node->as<slang::syntax::ConditionalBranchDirectiveSyntax>();
                append_leaf_trace_tokens(*branch.expr, event.expr_tokens);
                append_disabled_ranges(branch.disabledTokens, event.disabled_ranges);
            } else if (kind == slang::syntax::SyntaxKind::ElseDirective ||
                       kind == slang::syntax::SyntaxKind::EndIfDirective) {
                auto& branch = node->as<slang::syntax::UnconditionalBranchDirectiveSyntax>();
                append_disabled_ranges(branch.disabledTokens, event.disabled_ranges);
            }
            result.events.emplace_back(std::move(event));
        }

        for (auto include : tree.tree->getIncludeDirectives()) {
            auto event = event_ids.find(include.syntax);
            if (event == event_ids.end())
                continue;
            result.include_edges.emplace_back(RawTraceIncludeEdge {
                event->second, include.buffer.id.getId()
            });
        }

        for (const auto& token : tree.tree->getEmittedTokens()) {
            add_source_range_buffer(token.range());
            if (token.location().valid()) {
                auto original = tree.session->source_manager.getFullyOriginalLoc(token.location());
                if (original.valid())
                    source_buffer_ids.insert(original.buffer().getId());
                if (tree.session->source_manager.isMacroLoc(token.location()))
                    add_source_range_buffer(tree.session->source_manager.getFullyOriginalRange(
                        tree.session->source_manager.getExpansionRange(token.location())));
            }
            auto range = trace_range(token.range());
            if (!range.has_range)
                continue;
            result.emitted_tokens.emplace_back(trace_emitted_token(
                token, tree.session->source_manager,
                static_cast<uint32_t>(result.emitted_tokens.size()),
                calls, call_origins, call_definitions, call_names
            ));
        }
        std::vector<uint32_t> sorted_source_buffer_ids(
            source_buffer_ids.begin(), source_buffer_ids.end());
        std::sort(sorted_source_buffer_ids.begin(), sorted_source_buffer_ids.end());
        for (auto buffer_id : sorted_source_buffer_ids) {
            auto buffer = slang::BufferID(buffer_id, "");
            auto kind = tree.session->source_manager.getBufferKind(buffer);
            if (kind == ::slang::SourceManager::BufferKind::Unknown ||
                kind == ::slang::SourceManager::BufferKind::Macro ||
                kind == ::slang::SourceManager::BufferKind::MacroArg)
                continue;
            auto raw_path = tree.session->source_manager.getRawFileName(buffer);
            auto full_path = tree.session->source_manager.getFullPath(buffer);
            auto path = full_path.empty() ? std::string(raw_path) : full_path.string();
            auto text = tree.session->source_manager.getSourceText(buffer);
            auto origin = static_cast<uint8_t>(raw_path == "<api>" ? 1 : 0);
            result.source_buffers.emplace_back(RawTraceSourceBuffer {
                rust::String(path.data(), path.size()),
                rust::String(text.data(), text.size()),
                buffer_id, origin,
            });
        }
        return result;
    }

    std::optional<uint32_t> trace_emitted_token_index_for_target(
        const SyntaxToken *target,
        const SyntaxNode *context,
        const SyntaxTree &owner,
        const RawTrace &trace
    ) {
        auto root = helper::find_root(context);
        if (!root)
            throw std::invalid_argument("syntax context has no root");

        if (root != &owner.root())
            throw std::invalid_argument("syntax context does not belong to its owner tree");

        std::vector<slang::parsing::Token> emitted_tokens;
        for (auto token : owner.tree->getEmittedTokens()) {
            if (trace_range(token.range()).has_range)
                emitted_tokens.push_back(token);
        }
        if (emitted_tokens.size() != trace.emitted_tokens.size())
            throw std::logic_error("Slang trace token sequence is inconsistent");
        // Recovery and macro splicing can leave syntax-tree tokens that were
        // never emitted by the preprocessor. Only the requested target needs
        // an emitted identity; the two sequences are not required to be
        // positionally isomorphic.
        auto match = std::find(emitted_tokens.begin(), emitted_tokens.end(), *target);
        if (match == emitted_tokens.end())
            return std::nullopt;
        return static_cast<uint32_t>(match - emitted_tokens.begin());
    }

    RawTraceEmittedToken trace_emitted_token_for_target(
        const SyntaxToken *target,
        const SyntaxNode *context,
        const SyntaxTree &owner
    ) {
        auto trace = syntax_tree_preprocessor_trace(owner);
        auto index = trace_emitted_token_index_for_target(target, context, owner, trace);
        if (!index) {
            auto raw = target->rawText();
            auto range = trace_range(target->range());
            throw std::logic_error(
                "Slang target token is absent from emitted token stream: raw='" +
                std::string(raw.data(), raw.size()) + "' kind=" +
                std::to_string(static_cast<uint16_t>(target->kind)) + " range=" +
                std::to_string(range.buffer_id) + ":" +
                std::to_string(range.range_start) + "-" +
                std::to_string(range.range_end)
            );
        }
        return trace.emitted_tokens[*index];
    }

} // namespace slang_sys::syntax::tree

namespace slang_sys::syntax::node {

    uint16_t syntax_node_kind(const SyntaxNode *node) {
        return static_cast<uint16_t>(node->kind);
    }

    bool syntax_node_range_valid(const SyntaxNode *node) {
        return helper::source_range_valid(helper::node_range(node));
    }

    uint32_t syntax_node_range_start_buffer_id(const SyntaxNode *node) {
        return helper::node_range(node).start().buffer().getId();
    }

    std::size_t syntax_node_range_start_offset(const SyntaxNode *node) {
        return helper::node_range(node).start().offset();
    }

    uint32_t syntax_node_range_end_buffer_id(const SyntaxNode *node) {
        return helper::node_range(node).end().buffer().getId();
    }

    std::size_t syntax_node_range_end_offset(const SyntaxNode *node) {
        return helper::node_range(node).end().offset();
    }

    bool syntax_node_range_with_context_valid(
        const SyntaxNode *node,
        const SyntaxNode *context,
        const SyntaxTree &owner
    ) {
        return helper::source_range_valid(helper::node_range_with_context(node, context, owner));
    }

    uint32_t syntax_node_range_with_context_start_buffer_id(
        const SyntaxNode *node,
        const SyntaxNode *context,
        const SyntaxTree &owner
    ) {
        return helper::node_range_with_context(node, context, owner).start().buffer().getId();
    }

    std::size_t syntax_node_range_with_context_start_offset(
        const SyntaxNode *node,
        const SyntaxNode *context,
        const SyntaxTree &owner
    ) {
        return helper::node_range_with_context(node, context, owner).start().offset();
    }

    uint32_t syntax_node_range_with_context_end_buffer_id(
        const SyntaxNode *node,
        const SyntaxNode *context,
        const SyntaxTree &owner
    ) {
        return helper::node_range_with_context(node, context, owner).end().buffer().getId();
    }

    std::size_t syntax_node_range_with_context_end_offset(
        const SyntaxNode *node,
        const SyntaxNode *context,
        const SyntaxTree &owner
    ) {
        return helper::node_range_with_context(node, context, owner).end().offset();
    }

    const SyntaxNode *syntax_node_parent(const SyntaxNode *node) {
        return node->parent.get();
    }

    std::size_t syntax_node_child_count(const SyntaxNode *node) {
        return node->getChildCount();
    }

    std::size_t syntax_node_list_child_count(SyntaxNode *node) {
        slang::SmallVector<slang::syntax::ListChildInfo, 2> info;
        getChildListInfo(*node, info);
        return info.size();
    }

    std::size_t syntax_node_list_child_size(SyntaxNode *node, std::size_t index) {
        slang::SmallVector<slang::syntax::ListChildInfo, 2> info;
        getChildListInfo(*node, info);
        return info[index].size;
    }

    const SyntaxNode *syntax_node_child_node(const SyntaxNode *node, std::size_t index) {
        return node->childNode(index);
    }

    const SyntaxToken *syntax_node_child_token(SyntaxNode *node, std::size_t index) {
        return node->childTokenPtr(index);
    }

} // namespace slang_sys::syntax::node

namespace slang_sys::syntax::token {

    uint16_t syntax_token_kind(const SyntaxToken *token) {
        return static_cast<uint16_t>(token->kind);
    }

    bool syntax_token_range_valid(const SyntaxToken *token) {
        return helper::source_range_valid(helper::token_range(token));
    }

    uint32_t syntax_token_range_start_buffer_id(const SyntaxToken *token) {
        return helper::token_range(token).start().buffer().getId();
    }

    std::size_t syntax_token_range_start_offset(const SyntaxToken *token) {
        return helper::token_range(token).start().offset();
    }

    uint32_t syntax_token_range_end_buffer_id(const SyntaxToken *token) {
        return helper::token_range(token).end().buffer().getId();
    }

    std::size_t syntax_token_range_end_offset(const SyntaxToken *token) {
        return helper::token_range(token).end().offset();
    }

    bool syntax_token_range_with_context_valid(
        const SyntaxToken *token,
        const SyntaxNode *context,
        const SyntaxTree &owner
    ) {
        return helper::source_range_valid(helper::token_range_with_context(token, context, owner));
    }

    uint32_t syntax_token_range_with_context_start_buffer_id(
        const SyntaxToken *token,
        const SyntaxNode *context,
        const SyntaxTree &owner
    ) {
        return helper::token_range_with_context(token, context, owner).start().buffer().getId();
    }

    std::size_t syntax_token_range_with_context_start_offset(
        const SyntaxToken *token,
        const SyntaxNode *context,
        const SyntaxTree &owner
    ) {
        return helper::token_range_with_context(token, context, owner).start().offset();
    }

    uint32_t syntax_token_range_with_context_end_buffer_id(
        const SyntaxToken *token,
        const SyntaxNode *context,
        const SyntaxTree &owner
    ) {
        return helper::token_range_with_context(token, context, owner).end().buffer().getId();
    }

    std::size_t syntax_token_range_with_context_end_offset(
        const SyntaxToken *token,
        const SyntaxNode *context,
        const SyntaxTree &owner
    ) {
        return helper::token_range_with_context(token, context, owner).end().offset();
    }

    rust::String syntax_token_value_text(const SyntaxToken *token) {
        auto text = token->valueText();
        return rust::String(text.data(), text.size());
    }

    rust::String syntax_token_raw_text(const SyntaxToken *token) {
        auto text = token->rawText();
        return rust::String(text.data(), text.size());
    }

    RawSVInt syntax_token_int_value(const SyntaxToken *token) {
        auto value = token->intValue();
        auto base_text = [&](slang::LiteralBase base) {
            auto text = value.toString(base, false);
            return rust::String(text.data(), text.size());
        };
        RawSVInt result {
            value.getBitWidth(),
            value.isSigned(),
            value.hasUnknown(),
            0,
            false,
            base_text(slang::LiteralBase::Binary),
            base_text(slang::LiteralBase::Octal),
            base_text(slang::LiteralBase::Decimal),
            base_text(slang::LiteralBase::Hex),
        };
        if (value.isSingleWord() && !value.hasUnknown()) {
            result.single_word = *value.getRawPtr();
            result.has_single_word = true;
        }
        return result;
    }

    double syntax_token_real_value(const SyntaxToken *token) {
        return token->realValue();
    }

    uint8_t syntax_token_bit_value(const SyntaxToken *token) {
        return token->bitValue().value;
    }

    uint8_t syntax_token_literal_base(const SyntaxToken *token) {
        return static_cast<uint8_t>(token->numericFlags().base());
    }

    uint8_t syntax_token_time_unit(const SyntaxToken *token) {
        return static_cast<uint8_t>(token->numericFlags().unit());
    }

    RawOptionalU32 syntax_token_preprocessor_trace_emitted_token_index(
        const SyntaxToken *target,
        const SyntaxNode *context,
        const SyntaxTree &owner
    ) {
        auto trace = tree::syntax_tree_preprocessor_trace(owner);
        auto index = tree::trace_emitted_token_index_for_target(target, context, owner, trace);
        if (!index)
            return RawOptionalU32 { 0, false };
        return RawOptionalU32 { *index, true };
    }

    RawTraceEmittedToken syntax_token_preprocessor_trace_emitted_token(
        const SyntaxToken *target,
        const SyntaxNode *context,
        const SyntaxTree &owner
    ) {
        return tree::trace_emitted_token_for_target(target, context, owner);
    }

    rust::Vec<rust::String> syntax_token_keyword_table_for_version(rust::Str version) {
        rust::Vec<rust::String> result;
        auto keyword_version = slang::parsing::LexerFacts::getKeywordVersion(
            std::string_view(version.data(), version.size())
        );
        if (!keyword_version)
            return result;
        auto *table = slang::parsing::LexerFacts::getKeywordTable(*keyword_version);
        if (!table)
            return result;
        result.reserve(table->size());
        for (const auto &[text, _] : *table)
            result.emplace_back(text.data(), text.size());
        return result;
    }

    uint16_t syntax_token_keyword_kind_for_version(rust::Str version, rust::Str text) {
        auto keyword_version = slang::parsing::LexerFacts::getKeywordVersion(
            std::string_view(version.data(), version.size())
        );
        if (!keyword_version)
            return static_cast<uint16_t>(slang::parsing::TokenKind::Unknown);
        auto *table = slang::parsing::LexerFacts::getKeywordTable(*keyword_version);
        if (!table)
            return static_cast<uint16_t>(slang::parsing::TokenKind::Unknown);
        auto it = table->find(std::string_view(text.data(), text.size()));
        if (it == table->end())
            return static_cast<uint16_t>(slang::parsing::TokenKind::Unknown);
        return static_cast<uint16_t>(it->second);
    }

    uint16_t syntax_token_directive_kind(rust::Str text) {
        auto directive = std::string_view(text.data(), text.size());
        return static_cast<uint16_t>(slang::parsing::LexerFacts::getDirectiveKind(
            directive, false
        ));
    }

    rust::String syntax_token_directive_text(uint16_t kind) {
        auto text = slang::parsing::LexerFacts::getDirectiveText(
            static_cast<slang::syntax::SyntaxKind>(kind)
        );
        return rust::String(text.data(), text.size());
    }

    std::size_t syntax_token_trivia_count(const SyntaxToken *token) {
        return token->trivia().size();
    }

} // namespace slang_sys::syntax::token

namespace slang_sys::syntax::facts {

using TokenKind = slang::parsing::TokenKind;
using SyntaxKind = slang::syntax::SyntaxKind;

static SyntaxKind get_block_item_declaration_kind(TokenKind kind) {
    switch (kind) {
        case TokenKind::ParameterKeyword:
        case TokenKind::LocalParamKeyword:
            return SyntaxKind::ParameterDeclarationStatement;
        case TokenKind::TypedefKeyword:
            return SyntaxKind::TypedefDeclaration;
        case TokenKind::NetTypeKeyword:
            return SyntaxKind::NetTypeDeclaration;
        case TokenKind::LetKeyword:
            return SyntaxKind::LetDeclaration;
        case TokenKind::ImportKeyword:
            return SyntaxKind::PackageImportDeclaration;
        case TokenKind::VarKeyword:
        case TokenKind::StaticKeyword:
        case TokenKind::AutomaticKeyword:
            return SyntaxKind::DataDeclaration;
        default:
            if (slang::syntax::SyntaxFacts::getIntegerType(kind) != SyntaxKind::Unknown ||
                slang::syntax::SyntaxFacts::getKeywordType(kind) != SyntaxKind::Unknown ||
                slang::syntax::SyntaxFacts::isPossibleDataType(kind))
                return SyntaxKind::DataDeclaration;
            return SyntaxKind::Unknown;
    }
}

static SyntaxKind get_library_map_member_kind(TokenKind kind) {
    switch (kind) {
        case TokenKind::ConfigKeyword: return SyntaxKind::ConfigDeclaration;
        case TokenKind::IncludeKeyword: return SyntaxKind::LibraryIncludeStatement;
        case TokenKind::LibraryKeyword: return SyntaxKind::LibraryDeclaration;
        case TokenKind::Semicolon: return SyntaxKind::EmptyMember;
        default: return SyntaxKind::Unknown;
    }
}

static SyntaxKind get_specify_item_kind(TokenKind kind) {
    switch (kind) {
        case TokenKind::SpecParamKeyword: return SyntaxKind::SpecparamDeclaration;
        case TokenKind::PulseStyleOnDetectKeyword:
        case TokenKind::PulseStyleOnEventKeyword:
        case TokenKind::ShowCancelledKeyword:
        case TokenKind::NoShowCancelledKeyword:
            return SyntaxKind::PulseStyleDeclaration;
        case TokenKind::IfNoneKeyword: return SyntaxKind::IfNonePathDeclaration;
        case TokenKind::IfKeyword: return SyntaxKind::ConditionalPathDeclaration;
        case TokenKind::OpenParenthesis: return SyntaxKind::PathDeclaration;
        case TokenKind::SystemIdentifier: return SyntaxKind::SystemTimingCheck;
        default: return SyntaxKind::Unknown;
    }
}

static SyntaxKind get_config_header_item_kind(TokenKind kind) {
    switch (kind) {
        case TokenKind::DesignKeyword: return SyntaxKind::ConfigDeclaration;
        case TokenKind::LocalParamKeyword: return SyntaxKind::ParameterDeclarationStatement;
        default: return SyntaxKind::Unknown;
    }
}

static SyntaxKind get_config_rule_kind(TokenKind kind) {
    switch (kind) {
        case TokenKind::DefaultKeyword: return SyntaxKind::DefaultConfigRule;
        case TokenKind::CellKeyword: return SyntaxKind::CellConfigRule;
        case TokenKind::InstanceKeyword: return SyntaxKind::InstanceConfigRule;
        default: return SyntaxKind::Unknown;
    }
}

bool is_possible_statement(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isPossibleStatement(static_cast<TokenKind>(kind));
}

bool is_possible_expression(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isPossibleExpression(static_cast<TokenKind>(kind));
}

bool is_possible_data_type(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isPossibleDataType(static_cast<TokenKind>(kind));
}

bool is_possible_argument(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isPossibleArgument(static_cast<TokenKind>(kind));
}

bool is_possible_param_assignment(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isPossibleParamAssignment(static_cast<TokenKind>(kind));
}

bool is_possible_port_connection(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isPossiblePortConnection(static_cast<TokenKind>(kind));
}

bool is_possible_ansi_port(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isPossibleAnsiPort(static_cast<TokenKind>(kind));
}

bool is_possible_non_ansi_port(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isPossibleNonAnsiPort(static_cast<TokenKind>(kind));
}

bool is_possible_function_port(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isPossibleFunctionPort(static_cast<TokenKind>(kind));
}

bool is_possible_parameter(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isPossibleParameter(static_cast<TokenKind>(kind));
}

bool is_gate_type(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isGateType(static_cast<TokenKind>(kind));
}

bool is_edge_kind(uint16_t kind) {
    return slang::ast::SemanticFacts::getEdgeKind(static_cast<TokenKind>(kind)) !=
           slang::ast::EdgeKind::None;
}

bool is_port_direction(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isPortDirection(static_cast<TokenKind>(kind));
}

bool is_net_type(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isNetType(static_cast<TokenKind>(kind));
}

uint16_t get_integer_type(uint16_t kind) {
    return static_cast<uint16_t>(slang::syntax::SyntaxFacts::getIntegerType(static_cast<TokenKind>(kind)));
}

uint16_t get_keyword_type(uint16_t kind) {
    return static_cast<uint16_t>(slang::syntax::SyntaxFacts::getKeywordType(static_cast<TokenKind>(kind)));
}

uint16_t get_procedural_block_kind(uint16_t kind) {
    return static_cast<uint16_t>(slang::syntax::SyntaxFacts::getProceduralBlockKind(static_cast<TokenKind>(kind)));
}

uint16_t get_module_declaration_kind(uint16_t kind) {
    return static_cast<uint16_t>(slang::syntax::SyntaxFacts::getModuleDeclarationKind(static_cast<TokenKind>(kind)));
}

bool is_possible_member_kind(uint16_t raw_token_kind, uint16_t raw_member_kind) {
    auto token_kind = static_cast<TokenKind>(raw_token_kind);
    auto member_kind = static_cast<SyntaxKind>(raw_member_kind);
    if (slang::syntax::SyntaxFacts::getModuleDeclarationKind(token_kind) == member_kind ||
        slang::syntax::SyntaxFacts::getProceduralBlockKind(token_kind) == member_kind ||
        (is_gate_type(raw_token_kind) && member_kind == SyntaxKind::PrimitiveInstantiation) ||
        (is_port_direction(raw_token_kind) && member_kind == SyntaxKind::PortDeclaration) ||
        (token_kind == TokenKind::ConstKeyword && member_kind == SyntaxKind::PortDeclaration) ||
        (is_net_type(raw_token_kind) && member_kind == SyntaxKind::NetDeclaration) ||
        get_block_item_declaration_kind(token_kind) == member_kind)
        return true;

    switch (token_kind) {
        case TokenKind::GenerateKeyword: return member_kind == SyntaxKind::GenerateRegion;
        case TokenKind::BeginKeyword: return member_kind == SyntaxKind::GenerateBlock;
        case TokenKind::TimeUnitKeyword:
        case TokenKind::TimePrecisionKeyword:
            return member_kind == SyntaxKind::TimeUnitsDeclaration;
        case TokenKind::InterfaceKeyword: return member_kind == SyntaxKind::ClassDeclaration;
        case TokenKind::ModPortKeyword: return member_kind == SyntaxKind::ModportDeclaration;
        case TokenKind::BindKeyword: return member_kind == SyntaxKind::BindDirective;
        case TokenKind::SpecParamKeyword: return member_kind == SyntaxKind::SpecparamDeclaration;
        case TokenKind::AliasKeyword: return member_kind == SyntaxKind::NetAlias;
        case TokenKind::SpecifyKeyword: return member_kind == SyntaxKind::SpecifyBlock;
        case TokenKind::AssertKeyword:
        case TokenKind::AssumeKeyword:
        case TokenKind::CoverKeyword:
            return member_kind == SyntaxKind::ImmediateAssertionMember ||
                   member_kind == SyntaxKind::ConcurrentAssertionMember;
        case TokenKind::RestrictKeyword: return member_kind == SyntaxKind::ConcurrentAssertionMember;
        case TokenKind::AssignKeyword: return member_kind == SyntaxKind::ContinuousAssign;
        case TokenKind::ForKeyword: return member_kind == SyntaxKind::LoopGenerate;
        case TokenKind::IfKeyword: return member_kind == SyntaxKind::IfGenerate;
        case TokenKind::CaseKeyword: return member_kind == SyntaxKind::CaseGenerate;
        case TokenKind::GenVarKeyword: return member_kind == SyntaxKind::GenvarDeclaration;
        case TokenKind::TaskKeyword: return member_kind == SyntaxKind::TaskDeclaration;
        case TokenKind::FunctionKeyword: return member_kind == SyntaxKind::FunctionDeclaration;
        case TokenKind::CoverGroupKeyword: return member_kind == SyntaxKind::CovergroupDeclaration;
        case TokenKind::ClassKeyword:
        case TokenKind::VirtualKeyword:
            return member_kind == SyntaxKind::ClassDeclaration;
        case TokenKind::DefParamKeyword: return member_kind == SyntaxKind::DefParam;
        case TokenKind::ImportKeyword:
            return member_kind == SyntaxKind::PackageImportDeclaration || member_kind == SyntaxKind::DPIImport;
        case TokenKind::ExportKeyword:
            return member_kind == SyntaxKind::PackageExportDeclaration ||
                   member_kind == SyntaxKind::PackageExportAllDeclaration || member_kind == SyntaxKind::DPIExport;
        case TokenKind::Semicolon: return member_kind == SyntaxKind::EmptyMember;
        case TokenKind::PropertyKeyword: return member_kind == SyntaxKind::PropertyDeclaration;
        case TokenKind::SequenceKeyword: return member_kind == SyntaxKind::SequenceDeclaration;
        case TokenKind::CheckerKeyword: return member_kind == SyntaxKind::CheckerDeclaration;
        case TokenKind::GlobalKeyword: return member_kind == SyntaxKind::DefaultClockingReference;
        case TokenKind::DefaultKeyword:
            return member_kind == SyntaxKind::DefaultClockingReference ||
                   member_kind == SyntaxKind::DefaultDisableDeclaration;
        case TokenKind::ClockingKeyword: return member_kind == SyntaxKind::ClockingDeclaration;
        case TokenKind::ConstraintKeyword: return member_kind == SyntaxKind::ConstraintDeclaration;
        case TokenKind::SystemIdentifier: return member_kind == SyntaxKind::ElabSystemTask;
        case TokenKind::PrimitiveKeyword: return member_kind == SyntaxKind::UdpDeclaration;
        case TokenKind::RandKeyword: return member_kind == SyntaxKind::CheckerDataDeclaration;
        case TokenKind::ExternKeyword:
            return member_kind == SyntaxKind::ExternModuleDecl || member_kind == SyntaxKind::ExternUdpDecl ||
                   member_kind == SyntaxKind::ExternInterfaceMethod;
        case TokenKind::ConfigKeyword: return member_kind == SyntaxKind::ConfigDeclaration;
        default: return false;
    }
}

uint16_t get_block_item_declaration_kind(uint16_t kind) {
    return static_cast<uint16_t>(get_block_item_declaration_kind(static_cast<TokenKind>(kind)));
}

uint16_t get_library_map_member_kind(uint16_t kind) {
    return static_cast<uint16_t>(get_library_map_member_kind(static_cast<TokenKind>(kind)));
}

uint16_t get_specify_item_kind(uint16_t kind) {
    return static_cast<uint16_t>(get_specify_item_kind(static_cast<TokenKind>(kind)));
}

uint16_t get_config_header_item_kind(uint16_t kind) {
    return static_cast<uint16_t>(get_config_header_item_kind(static_cast<TokenKind>(kind)));
}

uint16_t get_config_rule_kind(uint16_t kind) {
    return static_cast<uint16_t>(get_config_rule_kind(static_cast<TokenKind>(kind)));
}

static bool is_keyword_candidate(uint8_t context, TokenKind kind) {
    auto allowed = [&](bool (*predicate)(SyntaxKind)) {
        for (auto syntax_kind : slang::syntax::SyntaxKind_traits::values)
            if (predicate(syntax_kind) && is_possible_member_kind(static_cast<uint16_t>(kind), static_cast<uint16_t>(syntax_kind)))
                return true;
        return false;
    };
    switch (context) {
        case 0: return allowed(slang::syntax::SyntaxFacts::isAllowedInCompilationUnit);
        case 1: return get_library_map_member_kind(static_cast<uint16_t>(kind)) != static_cast<uint16_t>(SyntaxKind::Unknown);
        case 2:
        case 11: return slang::syntax::SyntaxFacts::isPossibleAnsiPort(kind);
        case 3: return allowed(slang::syntax::SyntaxFacts::isAllowedInModule);
        case 4: return allowed(slang::syntax::SyntaxFacts::isAllowedInGenerate);
        case 5: return get_specify_item_kind(static_cast<uint16_t>(kind)) != static_cast<uint16_t>(SyntaxKind::Unknown);
        case 6: return get_config_header_item_kind(static_cast<uint16_t>(kind)) != static_cast<uint16_t>(SyntaxKind::Unknown);
        case 7: return get_config_rule_kind(static_cast<uint16_t>(kind)) != static_cast<uint16_t>(SyntaxKind::Unknown);
        case 8: return get_block_item_declaration_kind(static_cast<uint16_t>(kind)) != static_cast<uint16_t>(SyntaxKind::Unknown) ||
                       slang::syntax::SyntaxFacts::isPossibleStatement(kind);
        case 9: return slang::syntax::SyntaxFacts::isPossibleStatement(kind);
        case 10: return slang::syntax::SyntaxFacts::isPossibleParameter(kind);
        case 12: return slang::syntax::SyntaxFacts::isPossibleFunctionPort(kind);
        case 13: return slang::syntax::SyntaxFacts::isGateType(kind);
        default: return false;
    }
}

rust::Vec<rust::String> keyword_candidates_for_context(rust::Str version, uint8_t context) {
    rust::Vec<rust::String> result;
    auto keyword_version = slang::parsing::LexerFacts::getKeywordVersion(
        std::string_view(version.data(), version.size())
    );
    if (!keyword_version)
        return result;
    auto *table = slang::parsing::LexerFacts::getKeywordTable(*keyword_version);
    if (!table)
        return result;
    std::vector<std::string_view> candidates;
    for (const auto &[text, kind] : *table)
        if (is_keyword_candidate(context, kind))
            candidates.push_back(text);
    std::sort(candidates.begin(), candidates.end());
    candidates.erase(std::unique(candidates.begin(), candidates.end()), candidates.end());
    for (auto text : candidates)
        result.emplace_back(text.data(), text.size());
    return result;
}

bool is_allowed_in_compilation_unit(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isAllowedInCompilationUnit(static_cast<SyntaxKind>(kind));
}

bool is_allowed_in_generate(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isAllowedInGenerate(static_cast<SyntaxKind>(kind));
}

bool is_allowed_in_module(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isAllowedInModule(static_cast<SyntaxKind>(kind));
}

bool is_allowed_in_interface(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isAllowedInInterface(static_cast<SyntaxKind>(kind));
}

bool is_allowed_in_program(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isAllowedInProgram(static_cast<SyntaxKind>(kind));
}

bool is_allowed_in_package(uint16_t kind) {
    return slang::syntax::SyntaxFacts::isAllowedInPackage(static_cast<SyntaxKind>(kind));
}

} // namespace slang_sys::syntax::facts

namespace slang_sys::syntax::trivia {

    uint8_t syntax_trivia_kind(const SyntaxToken *token, std::size_t index) {
        return static_cast<uint8_t>(helper::trivia_at(token, index).kind);
    }

    rust::String syntax_trivia_raw_text(const SyntaxToken *token, std::size_t index) {
        auto text = helper::trivia_at(token, index).getRawText();
        return rust::String(text.data(), text.size());
    }

    const SyntaxNode *syntax_trivia_syntax(const SyntaxToken *token, std::size_t index) {
        return helper::trivia_at(token, index).syntax();
    }

    bool syntax_trivia_explicit_location_valid(const SyntaxToken *token, std::size_t index) {
        auto loc = helper::explicit_location(helper::trivia_at(token, index));
        return loc.has_value() && loc->valid();
    }

    uint32_t syntax_trivia_explicit_location_buffer_id(
        const SyntaxToken *token,
        std::size_t index
    ) {
        return helper::explicit_location(helper::trivia_at(token, index))->buffer().getId();
    }

    std::size_t syntax_trivia_explicit_location_offset(
        const SyntaxToken *token,
        std::size_t index
    ) {
        return helper::explicit_location(helper::trivia_at(token, index))->offset();
    }

} // namespace slang_sys::syntax::trivia
