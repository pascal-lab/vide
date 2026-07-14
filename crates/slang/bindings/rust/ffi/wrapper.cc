#include "slang/bindings/rust/ffi.rs.h"

#include "slang/parsing/ExpectedSyntax.h"
#include "slang/parsing/ParserMetadata.h"
#include "slang/parsing/PreprocessorTrace.h"
#include "slang/syntax/AllSyntax.h"
#include "slang/syntax/SyntaxPrinter.h"

#include <algorithm>
#include <filesystem>
#include <limits>
#include <mutex>
#include <optional>
#include <unordered_map>
#include <unordered_set>

namespace wrapper {
namespace {

std::vector<std::string> to_std_strings(const rust::Vec<rust::String>& values) {
  std::vector<std::string> result;
  result.reserve(values.size());
  for (const auto& value : values)
    result.emplace_back(value.data(), value.size());
  return result;
}

std::string source_manager_path_key(std::string_view path) {
  std::filesystem::path raw{std::string(path)};
  std::error_code ec;
  auto canonical = std::filesystem::weakly_canonical(raw, ec);
  return ec ? raw.string() : canonical.string();
}

void apply_warning_options(slang::DiagnosticEngine& engine,
                           const rust::Vec<rust::String>& warning_options) {
  engine.setDefaultWarnings();

  auto options = to_std_strings(warning_options);
  (void)engine.setWarningOptions(options);
}

rust::Vec<rust::String> diagnostic_args(const Diagnostic& diag) {
  rust::Vec<rust::String> result;
  for (const auto& arg : diag.args) {
    std::visit(
      [&](auto&& value) {
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
      arg);
  }
  return result;
}

struct SyntaxTreeSourceInfo {
  const slang::SourceManager* sourceManager;
  const slang::parsing::PreprocessorTraceSnapshot* preprocessorTrace;
  slang::SourceLocation rootLocation;
};

struct LexedTokenAtOffset {
  slang::parsing::TokenKind tokenKind = slang::parsing::TokenKind::Unknown;
  slang::syntax::SyntaxKind directiveKind = slang::syntax::SyntaxKind::Unknown;
  std::string rawText;
  size_t start = 0;
  size_t end = 0;
  bool found = false;
};

std::mutex syntaxTreeSourceInfoMutex;
std::unordered_map<const slang::syntax::SyntaxNode*, SyntaxTreeSourceInfo> syntaxTreeSourceInfo;

const slang::syntax::SyntaxNode* findRoot(const slang::syntax::SyntaxNode& node) {
  const auto* root = &node;
  while (root->parent)
    root = root->parent;
  return root;
}

::RawLexedTokenAtOffset emptyTokenAtOffset() {
  ::RawLexedTokenAtOffset result;
  result.replacement_start = 0;
  result.replacement_end = 0;
  result.prefix = rust::String();
  result.token_kind = static_cast<uint16_t>(slang::parsing::TokenKind::Unknown);
  result.directive_kind = static_cast<uint16_t>(slang::syntax::SyntaxKind::Unknown);
  result.has_directive_kind = false;
  result.has_token = false;
  return result;
}

LexedTokenAtOffset lexTokenAtOffset(std::string_view text,
                                    std::string_view name,
                                    std::string_view path,
                                    size_t offset) {
  slang::SourceManager sourceManager;
  auto bufferPath = path.empty() ? (name.empty() ? std::string_view("source") : name) : path;
  auto buffer = sourceManager.assignText(bufferPath, text);
  if (!buffer)
    return {};

  slang::BumpAllocator alloc;
  slang::Diagnostics diagnostics;
  slang::parsing::Lexer lexer(buffer, alloc, diagnostics);

  while (true) {
    auto token = lexer.lex();
    if (token.kind == slang::parsing::TokenKind::EndOfFile)
      return {};

    auto range = token.range();
    if (!range.start().valid() || !range.end().valid() || range.start().buffer() != buffer.id)
      continue;

    auto start = range.start().offset();
    auto end = range.end().offset();
    if (offset < start)
      return {};
    if (offset > end)
      continue;

    LexedTokenAtOffset result;
    result.tokenKind = token.kind;
    result.directiveKind = token.kind == slang::parsing::TokenKind::Directive
                               ? token.directiveKind()
                               : slang::syntax::SyntaxKind::Unknown;
    result.rawText = std::string(token.rawText());
    result.start = start;
    result.end = end;
    result.found = true;
    return result;
  }
}

::RawSourceBufferRange empty_source_buffer_range() {
  ::RawSourceBufferRange result;
  result.buffer_id = 0;
  result.range_start = 0;
  result.range_end = 0;
  result.has_range = false;
  return result;
}

::RawSourceBufferRange to_rust_source_buffer_range(slang::SourceRange range) {
  auto result = empty_source_buffer_range();
  if (range == slang::SourceRange::NoLocation)
    return result;
  if (!range.start().valid() || !range.end().valid())
    return result;
  if (range.start().buffer() != range.end().buffer())
    return result;

  result.buffer_id = range.start().buffer().getId();
  result.range_start = range.start().offset();
  result.range_end = range.end().offset();
  result.has_range = true;
  return result;
}

constexpr uint8_t TRACE_TOKEN_ORIGIN_UNAVAILABLE = 0;
constexpr uint8_t TRACE_TOKEN_ORIGIN_SOURCE = 1;
constexpr uint8_t TRACE_TOKEN_ORIGIN_MACRO_BODY = 2;
constexpr uint8_t TRACE_TOKEN_ORIGIN_MACRO_ARGUMENT = 3;
constexpr uint8_t TRACE_TOKEN_ORIGIN_BUILTIN = 4;
constexpr uint8_t TRACE_TOKEN_ORIGIN_TOKEN_PASTE = 5;
constexpr uint8_t TRACE_TOKEN_ORIGIN_STRINGIFICATION = 6;

::RawPreprocessorTraceEmittedToken empty_preprocessor_trace_emitted_token() {
  ::RawPreprocessorTraceEmittedToken token;
  token.emitted_token_index = 0;
  token.has_emitted_token_index = false;
  token.raw_text = rust::String();
  token.value_text = rust::String();
  token.display_text = rust::String();
  token.token_kind = static_cast<uint16_t>(slang::parsing::TokenKind::Unknown);
  token.origin_kind = TRACE_TOKEN_ORIGIN_UNAVAILABLE;
  token.macro_name = rust::String();
  token.macro_call_id = 0;
  token.has_macro_call_id = false;
  token.macro_definition_id = 0;
  token.has_macro_definition_id = false;
  token.macro_expansion_id = 0;
  token.has_macro_expansion_id = false;
  token.parent_macro_expansion_id = 0;
  token.has_parent_macro_expansion_id = false;
  token.body_token_index = 0;
  token.has_body_token_index = false;
  token.argument_index = 0;
  token.has_argument_index = false;
  token.argument_token_index = 0;
  token.has_argument_token_index = false;
  token.token_range = empty_source_buffer_range();
  token.call_range = empty_source_buffer_range();
  token.body_token_range = empty_source_buffer_range();
  token.argument_token_range = empty_source_buffer_range();
  return token;
}

std::string emitted_token_display_text(slang::parsing::Token token) {
  return slang::syntax::SyntaxPrinter()
      .setSquashNewlines(false)
      .print(token)
      .str();
}

bool is_single_buffer_range(slang::SourceRange range) {
  return range != slang::SourceRange::NoLocation && range.start().valid() &&
         range.end().valid() && range.start().buffer() == range.end().buffer();
}

::RawSourceBufferRange to_rust_original_macro_loc_range(
    const slang::SourceManager& sourceManager,
    slang::SourceRange range) {
  if (!is_single_buffer_range(range) || !sourceManager.isMacroLoc(range.start()) ||
      !sourceManager.isMacroLoc(range.end())) {
    return empty_source_buffer_range();
  }
  auto start = sourceManager.getOriginalLoc(range.start());
  auto end = sourceManager.getOriginalLoc(range.end());
  return to_rust_source_buffer_range(slang::SourceRange(start, end));
}

enum class TraceExpansionRangeSpace {
  FileBackedSource,
  MacroExpansion,
  InvalidOrMixed,
};

TraceExpansionRangeSpace classify_expansion_range(
    const slang::SourceManager& sourceManager,
    slang::SourceRange range) {
  if (!is_single_buffer_range(range))
    return TraceExpansionRangeSpace::InvalidOrMixed;

  const bool startIsFile = sourceManager.isFileLoc(range.start());
  const bool endIsFile = sourceManager.isFileLoc(range.end());
  const bool startIsMacro = sourceManager.isMacroLoc(range.start());
  const bool endIsMacro = sourceManager.isMacroLoc(range.end());
  if (startIsFile && endIsFile)
    return TraceExpansionRangeSpace::FileBackedSource;
  if (startIsMacro && endIsMacro)
    return TraceExpansionRangeSpace::MacroExpansion;
  return TraceExpansionRangeSpace::InvalidOrMixed;
}

::RawSourceBufferRange to_rust_written_source_range(
    const slang::SourceManager& sourceManager,
    slang::SourceRange range) {
  switch (classify_expansion_range(sourceManager, range)) {
    case TraceExpansionRangeSpace::FileBackedSource:
      return to_rust_source_buffer_range(range);
    case TraceExpansionRangeSpace::MacroExpansion:
      return to_rust_original_macro_loc_range(sourceManager, range);
    case TraceExpansionRangeSpace::InvalidOrMixed:
      return empty_source_buffer_range();
  }
  SLANG_UNREACHABLE;
}

::RawSourceBufferRange to_rust_macro_callsite_range_from_macro_loc(
    const slang::SourceManager& sourceManager,
    slang::SourceLocation macroLocation) {
  if (!macroLocation.valid() || !sourceManager.isMacroLoc(macroLocation))
    return empty_source_buffer_range();

  return to_rust_written_source_range(sourceManager, sourceManager.getExpansionRange(macroLocation));
}

::RawSourceBufferRange to_rust_macro_argument_callsite_range(
    const slang::SourceManager& sourceManager,
    slang::SourceRange formalRange) {
  if (classify_expansion_range(sourceManager, formalRange) !=
      TraceExpansionRangeSpace::MacroExpansion) {
    return empty_source_buffer_range();
  }

  return to_rust_macro_callsite_range_from_macro_loc(sourceManager, formalRange.start());
}

struct TraceSourceLocationKey {
  uint32_t buffer_id = 0;
  size_t offset = 0;

  bool operator==(const TraceSourceLocationKey& other) const {
    return buffer_id == other.buffer_id && offset == other.offset;
  }
};

struct TraceSourceLocationKeyHash {
  size_t operator()(const TraceSourceLocationKey& key) const {
    auto lhs = std::hash<uint32_t>{}(key.buffer_id);
    auto rhs = std::hash<size_t>{}(key.offset);
    return lhs ^ (rhs + 0x9e3779b97f4a7c15ULL + (lhs << 6) + (lhs >> 2));
  }
};

std::optional<TraceSourceLocationKey> trace_source_location_key(slang::SourceLocation location) {
  if (!location.valid())
    return std::nullopt;

  return TraceSourceLocationKey{
      location.buffer().getId(),
      location.offset(),
  };
}

bool has_direct_macro_token_origin(
    const std::optional<slang::SourceManager::MacroTokenProvenance>& origin) {
  return origin && origin->expansionId != 0 && origin->callId != 0 &&
         origin->definitionId != 0;
}

bool has_builtin_macro_token_origin(
    const std::optional<slang::SourceManager::MacroTokenProvenance>& origin) {
  return origin && origin->expansionId != 0 && origin->callId != 0 &&
         !origin->builtinName.empty();
}

void apply_direct_macro_token_origin(
    ::RawPreprocessorTraceEmittedToken& token,
    const slang::SourceManager::MacroTokenProvenance& origin) {
  token.macro_call_id = origin.callId;
  token.has_macro_call_id = origin.callId != 0;
  token.macro_definition_id = origin.definitionId;
  token.has_macro_definition_id = origin.definitionId != 0;
  token.macro_expansion_id = origin.expansionId;
  token.has_macro_expansion_id = origin.expansionId != 0;
  token.parent_macro_expansion_id = origin.parentExpansionId;
  token.has_parent_macro_expansion_id = origin.parentExpansionId != 0;
  token.body_token_index = origin.bodyTokenIndex;
  token.has_body_token_index =
      origin.bodyTokenIndex != slang::SourceManager::MacroTokenProvenance::InvalidIndex;
  token.argument_index = origin.argumentIndex;
  token.has_argument_index =
      origin.argumentIndex != slang::SourceManager::MacroTokenProvenance::InvalidIndex;
  token.argument_token_index = origin.argumentTokenIndex;
  token.has_argument_token_index =
      origin.argumentTokenIndex != slang::SourceManager::MacroTokenProvenance::InvalidIndex;
}

bool apply_macro_operation_token_origin(
    ::RawPreprocessorTraceEmittedToken& result,
    const std::optional<slang::SourceManager::MacroTokenProvenance>& origin,
    uint8_t originKind) {
  if (!has_direct_macro_token_origin(origin))
    return false;

  apply_direct_macro_token_origin(result, *origin);
  result.origin_kind = originKind;
  return true;
}

bool apply_original_macro_loc_origin_for_nested_argument(
    ::RawPreprocessorTraceEmittedToken& result,
    slang::parsing::Token token,
    const slang::SourceManager& sourceManager,
    slang::SourceLocation location) {
  if (!sourceManager.isMacroArgLoc(location))
    return false;

  auto originalLocation = sourceManager.getOriginalLoc(location);
  if (!originalLocation.valid() || !sourceManager.isMacroLoc(originalLocation))
    return false;

  switch (sourceManager.getMacroExpansionKind(originalLocation)) {
    case slang::SourceManager::MacroExpansionKind::TokenPaste:
    case slang::SourceManager::MacroExpansionKind::Stringification:
      return false;
    case slang::SourceManager::MacroExpansionKind::Body:
    case slang::SourceManager::MacroExpansionKind::Argument:
      break;
  }

  auto originalOrigin = sourceManager.getMacroTokenProvenance(originalLocation);
  if (!has_direct_macro_token_origin(originalOrigin))
    return false;

  auto originalTokenRange =
      slang::SourceRange(originalLocation, originalLocation + token.rawText().length());
  result.macro_name = rust::String(std::string(sourceManager.getMacroName(originalLocation)));

  if (sourceManager.isMacroArgLoc(originalLocation)) {
    result.argument_token_range =
        to_rust_original_macro_loc_range(sourceManager, originalTokenRange);

    auto formalRange = sourceManager.getExpansionRange(originalLocation);
    result.body_token_range = to_rust_original_macro_loc_range(sourceManager, formalRange);
    result.call_range = to_rust_macro_argument_callsite_range(sourceManager, formalRange);

    if (originalOrigin->bodyTokenIndex !=
            slang::SourceManager::MacroTokenProvenance::InvalidIndex &&
        originalOrigin->argumentIndex !=
            slang::SourceManager::MacroTokenProvenance::InvalidIndex &&
        originalOrigin->argumentTokenIndex !=
            slang::SourceManager::MacroTokenProvenance::InvalidIndex &&
        result.call_range.has_range && result.body_token_range.has_range &&
        result.argument_token_range.has_range) {
      apply_direct_macro_token_origin(result, *originalOrigin);
      result.origin_kind = TRACE_TOKEN_ORIGIN_MACRO_ARGUMENT;
      return true;
    }
    return false;
  }

  result.call_range =
      to_rust_macro_callsite_range_from_macro_loc(sourceManager, originalLocation);
  result.body_token_range = to_rust_original_macro_loc_range(sourceManager, originalTokenRange);
  if (originalOrigin->bodyTokenIndex !=
          slang::SourceManager::MacroTokenProvenance::InvalidIndex &&
      result.call_range.has_range && result.body_token_range.has_range) {
    apply_direct_macro_token_origin(result, *originalOrigin);
    result.origin_kind = TRACE_TOKEN_ORIGIN_MACRO_BODY;
    return true;
  }

  return false;
}

::RawPreprocessorTraceToken empty_preprocessor_trace_token() {
  ::RawPreprocessorTraceToken token;
  token.raw_text = rust::String();
  token.value_text = rust::String();
  token.token_kind = static_cast<uint16_t>(slang::parsing::TokenKind::Unknown);
  token.range = empty_source_buffer_range();
  token.has_token = false;
  return token;
}

::RawPreprocessorTraceToken to_rust_preprocessor_trace_token(slang::parsing::Token token) {
  auto result = empty_preprocessor_trace_token();
  if (!token)
    return result;

  result.raw_text = rust::String(std::string(token.rawText()));
  result.value_text = rust::String(std::string(token.valueText()));
  result.token_kind = static_cast<uint16_t>(token.kind);
  result.range = to_rust_source_buffer_range(token.range());
  result.has_token = true;
  return result;
}

::RawPreprocessorTraceToken to_rust_preprocessor_trace_written_token(
    slang::parsing::Token token,
    const slang::SourceManager& sourceManager) {
  auto result = empty_preprocessor_trace_token();
  if (!token)
    return result;

  result.raw_text = rust::String(std::string(token.rawText()));
  result.value_text = rust::String(std::string(token.valueText()));
  result.token_kind = static_cast<uint16_t>(token.kind);
  result.range = to_rust_written_source_range(sourceManager, token.range());
  result.has_token = true;
  return result;
}

template<typename TTokens>
rust::Vec<::RawPreprocessorTraceToken> to_rust_preprocessor_trace_tokens(
    const TTokens& tokens) {
  rust::Vec<::RawPreprocessorTraceToken> result;
  for (auto token : tokens)
    result.emplace_back(to_rust_preprocessor_trace_token(token));
  return result;
}

template<typename TTokens>
rust::Vec<::RawPreprocessorTraceToken> to_rust_preprocessor_trace_written_tokens(
    const TTokens& tokens,
    const slang::SourceManager& sourceManager) {
  rust::Vec<::RawPreprocessorTraceToken> result;
  for (auto token : tokens)
    result.emplace_back(to_rust_preprocessor_trace_written_token(token, sourceManager));
  return result;
}

template<typename TTokens>
::RawSourceBufferRange to_rust_written_token_range(
    const TTokens& tokens,
    const slang::SourceManager& sourceManager) {
  std::optional<::RawSourceBufferRange> merged;
  for (auto token : tokens) {
    auto range = to_rust_written_source_range(sourceManager, token.range());
    if (!range.has_range)
      continue;

    if (!merged) {
      merged = range;
      continue;
    }

    if (merged->buffer_id != range.buffer_id)
      return empty_source_buffer_range();

    merged->range_start = std::min(merged->range_start, range.range_start);
    merged->range_end = std::max(merged->range_end, range.range_end);
  }

  if (merged && merged->range_start < merged->range_end)
    return *merged;
  return empty_source_buffer_range();
}

::RawPreprocessorTraceEmittedToken to_rust_preprocessor_trace_emitted_token(
    slang::parsing::Token token,
    const slang::SourceManager& sourceManager,
    std::optional<size_t> emittedTokenIndex = std::nullopt) {
  auto result = empty_preprocessor_trace_emitted_token();
  if (!token)
    return result;

  if (emittedTokenIndex && *emittedTokenIndex <= std::numeric_limits<uint32_t>::max()) {
    result.emitted_token_index = static_cast<uint32_t>(*emittedTokenIndex);
    result.has_emitted_token_index = true;
  }

  result.raw_text = rust::String(std::string(token.rawText()));
  result.value_text = rust::String(std::string(token.valueText()));
  result.display_text = rust::String(emitted_token_display_text(token));
  result.token_kind = static_cast<uint16_t>(token.kind);

  auto location = token.location();
  if (!location.valid())
    return result;

  if (sourceManager.isMacroLoc(location)) {
    switch (sourceManager.getMacroExpansionKind(location)) {
      case slang::SourceManager::MacroExpansionKind::TokenPaste:
        apply_macro_operation_token_origin(
            result, sourceManager.getMacroTokenProvenance(location),
            TRACE_TOKEN_ORIGIN_TOKEN_PASTE);
        return result;
      case slang::SourceManager::MacroExpansionKind::Stringification:
        apply_macro_operation_token_origin(
            result, sourceManager.getMacroTokenProvenance(location),
            TRACE_TOKEN_ORIGIN_STRINGIFICATION);
        return result;
      case slang::SourceManager::MacroExpansionKind::Body:
      case slang::SourceManager::MacroExpansionKind::Argument:
        break;
    }

    auto macroName = std::string(sourceManager.getMacroName(location));
    result.macro_name = rust::String(macroName);
    auto directOrigin = sourceManager.getMacroTokenProvenance(location);
    if (has_builtin_macro_token_origin(directOrigin)) {
      result.macro_name = rust::String(directOrigin->builtinName);
      apply_direct_macro_token_origin(result, *directOrigin);
      result.origin_kind = TRACE_TOKEN_ORIGIN_BUILTIN;
      return result;
    }

    if (apply_original_macro_loc_origin_for_nested_argument(
            result, token, sourceManager, location))
      return result;

    if (sourceManager.isMacroArgLoc(location)) {
      auto tokenRange = token.range();
      result.argument_token_range = to_rust_original_macro_loc_range(sourceManager, tokenRange);

      auto formalRange = sourceManager.getExpansionRange(location);
      result.body_token_range = to_rust_original_macro_loc_range(sourceManager, formalRange);
      result.call_range = to_rust_macro_argument_callsite_range(sourceManager, formalRange);

      if (has_direct_macro_token_origin(directOrigin) &&
          directOrigin->bodyTokenIndex !=
              slang::SourceManager::MacroTokenProvenance::InvalidIndex &&
          directOrigin->argumentIndex !=
              slang::SourceManager::MacroTokenProvenance::InvalidIndex &&
          directOrigin->argumentTokenIndex !=
              slang::SourceManager::MacroTokenProvenance::InvalidIndex &&
          result.call_range.has_range && result.body_token_range.has_range &&
          result.argument_token_range.has_range) {
        apply_direct_macro_token_origin(result, *directOrigin);
        result.origin_kind = TRACE_TOKEN_ORIGIN_MACRO_ARGUMENT;
      }
      return result;
    }

    result.call_range = to_rust_macro_callsite_range_from_macro_loc(sourceManager, location);
    result.body_token_range = to_rust_original_macro_loc_range(sourceManager, token.range());
    if (has_direct_macro_token_origin(directOrigin) &&
        directOrigin->bodyTokenIndex !=
            slang::SourceManager::MacroTokenProvenance::InvalidIndex &&
        result.call_range.has_range && result.body_token_range.has_range) {
      apply_direct_macro_token_origin(result, *directOrigin);
      result.origin_kind = TRACE_TOKEN_ORIGIN_MACRO_BODY;
    }
    return result;
  }

  result.token_range = to_rust_source_buffer_range(token.range());
  if (result.token_range.has_range)
    result.origin_kind = TRACE_TOKEN_ORIGIN_SOURCE;
  return result;
}

void collect_leaf_trace_tokens(const slang::syntax::SyntaxNode& node,
                               rust::Vec<::RawPreprocessorTraceToken>& tokens) {
  for (size_t i = 0; i < node.getChildCount(); i++) {
    if (auto token = node.childToken(i))
      tokens.emplace_back(to_rust_preprocessor_trace_token(token));
    if (auto* child = node.childNode(i))
      collect_leaf_trace_tokens(*child, tokens);
  }
}

template<typename TTokens>
rust::Vec<::RawSourceBufferRange> to_rust_trace_disabled_ranges(const TTokens& tokens) {
  rust::Vec<::RawSourceBufferRange> result;
  std::optional<::RawSourceBufferRange> merged;

  auto flush = [&]() {
    if (merged && merged->has_range && merged->range_start < merged->range_end)
      result.emplace_back(*merged);
  };

  for (auto token : tokens) {
    auto range = to_rust_source_buffer_range(token.range());
    if (!range.has_range)
      continue;

    if (!merged) {
      merged = range;
      continue;
    }

    if (merged->buffer_id != range.buffer_id) {
      flush();
      merged = range;
      continue;
    }

    merged->range_start = std::min(merged->range_start, range.range_start);
    merged->range_end = std::max(merged->range_end, range.range_end);
  }

  flush();
  return result;
}

// Directive syntax node ranges are payload ranges, not trace event ranges. For example,
// EndIf/Else ranges are based on disabledTokens and can also be empty. The trace contract
// needs the event's own source span, so anchor every directive event at
// DirectiveSyntax::directive and extend only through that event's semantic payload.
slang::SourceRange trace_event_source_range(const slang::syntax::SyntaxNode& syntax) {
  auto* directiveSyntax = syntax.as_if<slang::syntax::DirectiveSyntax>();
  if (!directiveSyntax)
    return slang::SourceRange::NoLocation;

  auto directiveRange = directiveSyntax->directive.range();
  if (directiveRange == slang::SourceRange::NoLocation || !directiveRange.start().valid() ||
      !directiveRange.end().valid())
    return slang::SourceRange::NoLocation;

  auto start = directiveRange.start();
  auto end = directiveRange.end();
  auto extend = [&](slang::SourceRange range) {
    if (range == slang::SourceRange::NoLocation || !range.end().valid())
      return;
    if (range.end().buffer() != start.buffer())
      return;
    if (!end.valid() || range.end().offset() > end.offset())
      end = range.end();
  };
  auto extendToken = [&](slang::parsing::Token token) {
    if (token)
      extend(token.range());
  };

  switch (syntax.kind) {
    case slang::syntax::SyntaxKind::DefineDirective: {
      const auto& define = syntax.as<slang::syntax::DefineDirectiveSyntax>();
      extendToken(define.name);
      if (define.formalArguments)
        extend(define.formalArguments->sourceRange());
      extend(define.body.sourceRange());
      break;
    }
    case slang::syntax::SyntaxKind::UndefDirective: {
      const auto& undef = syntax.as<slang::syntax::UndefDirectiveSyntax>();
      extendToken(undef.name);
      break;
    }
    case slang::syntax::SyntaxKind::IncludeDirective: {
      const auto& include = syntax.as<slang::syntax::IncludeDirectiveSyntax>();
      extendToken(include.fileName);
      break;
    }
    case slang::syntax::SyntaxKind::IfDefDirective:
    case slang::syntax::SyntaxKind::IfNDefDirective:
    case slang::syntax::SyntaxKind::ElsIfDirective: {
      const auto& branch = syntax.as<slang::syntax::ConditionalBranchDirectiveSyntax>();
      extend(branch.expr->sourceRange());
      break;
    }
    case slang::syntax::SyntaxKind::ElseDirective:
    case slang::syntax::SyntaxKind::EndIfDirective:
      break;
    default:
      extend(syntax.sourceRange());
      break;
  }

  if (!start.valid() || !end.valid() || start.buffer() != end.buffer())
    return slang::SourceRange::NoLocation;
  return slang::SourceRange(start, end);
}

::RawPreprocessorTraceMacroParam to_rust_trace_macro_param(
    const slang::syntax::MacroFormalArgumentSyntax& param) {
  ::RawPreprocessorTraceMacroParam result;
  result.name = to_rust_preprocessor_trace_token(param.name);
  result.default_tokens = rust::Vec<::RawPreprocessorTraceToken>();
  result.has_default = param.defaultValue != nullptr;
  result.range = to_rust_source_buffer_range(param.sourceRange());
  if (param.defaultValue)
    result.default_tokens = to_rust_preprocessor_trace_tokens(param.defaultValue->tokens);
  return result;
}

::RawPreprocessorTraceActualArgument empty_preprocessor_trace_actual_argument() {
  ::RawPreprocessorTraceActualArgument result;
  result.tokens = rust::Vec<::RawPreprocessorTraceToken>();
  result.range = empty_source_buffer_range();
  return result;
}

::RawPreprocessorTraceActualArgument to_rust_trace_actual_argument(
    const slang::syntax::MacroActualArgumentSyntax& argument,
    const slang::SourceManager& sourceManager) {
  auto result = empty_preprocessor_trace_actual_argument();
  result.tokens = to_rust_preprocessor_trace_written_tokens(argument.tokens, sourceManager);
  result.range = to_rust_written_token_range(argument.tokens, sourceManager);
  return result;
}

rust::Vec<::RawPreprocessorTraceActualArgument> to_rust_trace_actual_arguments(
    const slang::syntax::MacroActualArgumentListSyntax* arguments,
    const slang::SourceManager& sourceManager) {
  rust::Vec<::RawPreprocessorTraceActualArgument> result;
  if (!arguments)
    return result;

  for (const auto* argument : arguments->args) {
    if (!argument)
      continue;
    result.emplace_back(to_rust_trace_actual_argument(*argument, sourceManager));
  }
  return result;
}

::RawPreprocessorTraceEvent to_rust_preprocessor_trace_event(
    const slang::syntax::SyntaxNode& syntax,
    uint32_t eventId,
    uint32_t macroDefinitionId) {
  ::RawPreprocessorTraceEvent directive;
  directive.event_id = eventId;
  directive.kind = static_cast<uint16_t>(syntax.kind);
  directive.range = to_rust_source_buffer_range(trace_event_source_range(syntax));
  directive.macro_definition_id = 0;
  directive.has_macro_definition_id = false;
  directive.macro_call_id = 0;
  directive.has_macro_call_id = false;
  directive.macro_expansion_id = 0;
  directive.has_macro_expansion_id = false;
  directive.parent_macro_expansion_id = 0;
  directive.has_parent_macro_expansion_id = false;
  directive.directive = empty_preprocessor_trace_token();
  directive.name = empty_preprocessor_trace_token();
  directive.include_file_name = empty_preprocessor_trace_token();
  directive.params = rust::Vec<::RawPreprocessorTraceMacroParam>();
  directive.arguments = rust::Vec<::RawPreprocessorTraceActualArgument>();
  directive.body_tokens = rust::Vec<::RawPreprocessorTraceToken>();
  directive.expr_tokens = rust::Vec<::RawPreprocessorTraceToken>();
  directive.disabled_ranges = rust::Vec<::RawSourceBufferRange>();

  if (auto* directiveSyntax = syntax.as_if<slang::syntax::DirectiveSyntax>())
    directive.directive = to_rust_preprocessor_trace_token(directiveSyntax->directive);

  switch (syntax.kind) {
    case slang::syntax::SyntaxKind::DefineDirective: {
      const auto& define = syntax.as<slang::syntax::DefineDirectiveSyntax>();
      directive.macro_definition_id = macroDefinitionId;
      directive.has_macro_definition_id = macroDefinitionId != 0;
      directive.name = to_rust_preprocessor_trace_token(define.name);
      if (define.formalArguments) {
        for (auto* param : define.formalArguments->args)
          directive.params.emplace_back(to_rust_trace_macro_param(*param));
      }
      directive.body_tokens = to_rust_preprocessor_trace_tokens(define.body);
      break;
    }
    case slang::syntax::SyntaxKind::UndefDirective: {
      const auto& undef = syntax.as<slang::syntax::UndefDirectiveSyntax>();
      directive.name = to_rust_preprocessor_trace_token(undef.name);
      break;
    }
    case slang::syntax::SyntaxKind::IncludeDirective: {
      const auto& include = syntax.as<slang::syntax::IncludeDirectiveSyntax>();
      directive.include_file_name = to_rust_preprocessor_trace_token(include.fileName);
      break;
    }
    case slang::syntax::SyntaxKind::IfDefDirective:
    case slang::syntax::SyntaxKind::IfNDefDirective:
    case slang::syntax::SyntaxKind::ElsIfDirective: {
      const auto& branch = syntax.as<slang::syntax::ConditionalBranchDirectiveSyntax>();
      collect_leaf_trace_tokens(*branch.expr, directive.expr_tokens);
      directive.disabled_ranges = to_rust_trace_disabled_ranges(branch.disabledTokens);
      break;
    }
    case slang::syntax::SyntaxKind::ElseDirective:
    case slang::syntax::SyntaxKind::EndIfDirective: {
      const auto& branch = syntax.as<slang::syntax::UnconditionalBranchDirectiveSyntax>();
      directive.disabled_ranges = to_rust_trace_disabled_ranges(branch.disabledTokens);
      break;
    }
    default:
      break;
  }

  return directive;
}

::RawPreprocessorTraceEvent to_rust_preprocessor_trace_macro_usage_record(
    const slang::parsing::MacroUsageTraceRecord& record,
    uint32_t eventId,
    const slang::SourceManager& sourceManager) {
  ::RawPreprocessorTraceEvent directive;
  directive.event_id = eventId;
  directive.kind = static_cast<uint16_t>(slang::syntax::SyntaxKind::MacroUsage);
  directive.range = to_rust_written_source_range(sourceManager, record.range);
  directive.macro_definition_id = record.definitionId;
  directive.has_macro_definition_id = record.definitionId != 0;
  directive.macro_call_id = record.callId;
  directive.has_macro_call_id = record.callId != 0;
  directive.macro_expansion_id = record.expansionId;
  directive.has_macro_expansion_id = record.expansionId != 0;
  directive.parent_macro_expansion_id = record.parentExpansionId;
  directive.has_parent_macro_expansion_id = record.parentExpansionId != 0;
  directive.directive = to_rust_preprocessor_trace_written_token(record.directive, sourceManager);
  directive.name = directive.directive;
  directive.include_file_name = empty_preprocessor_trace_token();
  directive.params = rust::Vec<::RawPreprocessorTraceMacroParam>();
  directive.arguments = to_rust_trace_actual_arguments(record.actualArgs, sourceManager);
  directive.body_tokens = rust::Vec<::RawPreprocessorTraceToken>();
  directive.expr_tokens = rust::Vec<::RawPreprocessorTraceToken>();
  directive.disabled_ranges = rust::Vec<::RawSourceBufferRange>();
  return directive;
}

rust::Vec<::RawSourceBufferId> collectSourceBufferIds(
    const slang::SourceManager& sourceManager,
    const std::unordered_set<uint32_t>& predefineBufferIds);

::RawPreprocessorTrace empty_preprocessor_trace() {
  ::RawPreprocessorTrace result;
  result.root_buffer_id = 0;
  result.has_root_buffer_id = false;
  result.source_buffers = rust::Vec<::RawSourceBufferId>();
  result.events = rust::Vec<::RawPreprocessorTraceEvent>();
  result.include_edges = rust::Vec<::RawPreprocessorTraceIncludeEdge>();
  result.emitted_tokens = rust::Vec<::RawPreprocessorTraceEmittedToken>();
  return result;
}

std::unordered_set<uint32_t> predefine_buffer_ids(
    const slang::parsing::PreprocessorTraceSnapshot& trace) {
  std::unordered_set<uint32_t> bufferIds;
  for (const auto& event : trace.events) {
    if (event.kind != slang::parsing::PreprocessorTraceEvent::Kind::Directive ||
        !event.directive.isPredefine || !event.directive.syntax) {
      continue;
    }

    auto* directive = event.directive.syntax->as_if<slang::syntax::DirectiveSyntax>();
    if (!directive)
      continue;
    auto location = directive->directive.location();
    if (location.valid())
      bufferIds.insert(location.buffer().getId());
  }
  return bufferIds;
}

std::optional<size_t> emitted_token_index_for(
    const slang::parsing::PreprocessorTraceSnapshot* trace,
    slang::parsing::Token token) {
  if (!trace || !token)
    return std::nullopt;

  for (size_t index = 0; index < trace->emittedTokens.size(); index++) {
    if (trace->emittedTokens[index] == token)
      return index;
  }

  return std::nullopt;
}

::RawPreprocessorTrace to_rust_preprocessor_trace_snapshot(
    const slang::parsing::PreprocessorTraceSnapshot& trace,
    const slang::SourceManager& sourceManager) {
  auto result = empty_preprocessor_trace();
  if (!trace.rootBufferId)
    return result;

  result.root_buffer_id = *trace.rootBufferId;
  result.has_root_buffer_id = true;

  std::unordered_map<TraceSourceLocationKey, uint32_t, TraceSourceLocationKeyHash>
      includeEventIdsByLocation;
  for (const auto& event : trace.events) {
    switch (event.kind) {
      case slang::parsing::PreprocessorTraceEvent::Kind::Directive: {
        if (!event.directive.syntax)
          continue;

        if (event.directive.syntax->kind == slang::syntax::SyntaxKind::IncludeDirective) {
          const auto& include =
              event.directive.syntax->as<slang::syntax::IncludeDirectiveSyntax>();
          if (auto key = trace_source_location_key(include.directive.location()))
            includeEventIdsByLocation.emplace(*key, event.eventId);
        }

        result.events.emplace_back(to_rust_preprocessor_trace_event(
            *event.directive.syntax, event.eventId, event.directive.macroDefinitionId));
        break;
      }
      case slang::parsing::PreprocessorTraceEvent::Kind::MacroUsage:
        result.events.emplace_back(to_rust_preprocessor_trace_macro_usage_record(
            event.macroUsage, event.eventId, sourceManager));
        break;
    }
  }

  for (size_t index = 0; index < trace.emittedTokens.size(); index++) {
    auto token = trace.emittedTokens[index];
    result.emitted_tokens.emplace_back(
        to_rust_preprocessor_trace_emitted_token(token, sourceManager, index));
  }

  for (auto buffer : sourceManager.getAllBuffers()) {
    auto includedFrom = sourceManager.getIncludedFrom(buffer);
    auto key = trace_source_location_key(includedFrom);
    if (!key)
      continue;

    auto includeIt = includeEventIdsByLocation.find(*key);
    if (includeIt == includeEventIdsByLocation.end())
      continue;

    ::RawPreprocessorTraceIncludeEdge edge;
    edge.include_event_id = includeIt->second;
    edge.included_buffer_id = buffer.getId();
    result.include_edges.emplace_back(edge);
  }

  result.source_buffers = collectSourceBufferIds(sourceManager, predefine_buffer_ids(trace));
  return result;
}

std::optional<slang::SourceRange> mapSourceRangeToContext(
    const slang::DiagnosticEngine& engine,
    slang::SourceLocation context,
    slang::SourceRange range) {
  if (range == slang::SourceRange::NoLocation)
    return std::nullopt;

  slang::SmallVector<slang::SourceRange> mapped;
  engine.mapSourceRanges(context, std::span(&range, 1), mapped, false);
  if (mapped.empty())
    return std::nullopt;

  return mapped.front();
}

rust::Vec<::RawSourceBufferId> collectSourceBufferIds(
    const slang::SourceManager& sourceManager,
    const std::unordered_set<uint32_t>& predefineBufferIds = {}) {
  rust::Vec<::RawSourceBufferId> sourceBuffers;
  for (auto buffer : sourceManager.getAllBuffers()) {
    const auto& fullPath = sourceManager.getFullPath(buffer);
    if (fullPath.empty())
      continue;

    ::RawSourceBufferId sourceBuffer;
    sourceBuffer.path = rust::String(fullPath.string());
    sourceBuffer.text = rust::String();
    sourceBuffer.has_text = false;
    sourceBuffer.buffer_id = buffer.getId();
    if (predefineBufferIds.contains(buffer.getId())) {
      auto text = sourceManager.getSourceText(buffer);
      if (!text.empty() && text.back() == '\0')
        text.remove_suffix(1);
      sourceBuffer.text = rust::String(std::string(text));
      sourceBuffer.has_text = true;
      sourceBuffer.origin = 1;
    }
    else {
      sourceBuffer.origin = 0;
    }
    sourceBuffers.emplace_back(std::move(sourceBuffer));
  }

  return sourceBuffers;
}

::RawSyntaxTreeBufferIds collectSyntaxTreeBufferIds(const syntax::SyntaxTree& tree) {
  ::RawSyntaxTreeBufferIds ids;
  ids.root_buffer_id = tree.inner().root().sourceRange().start().buffer().getId();
  ids.source_buffers = collectSourceBufferIds(tree.inner().sourceManager());
  return ids;
}

std::unique_ptr<SourceRange> mapRawSourceRangeWithContext(
    slang::SourceRange rawRange,
    const SyntaxNode& context) {
  if (rawRange == SourceRange::NoLocation)
    return nullptr;

  const auto* root = findRoot(context);
  SyntaxTreeSourceInfo sourceInfo;
  {
    std::lock_guard lock(syntaxTreeSourceInfoMutex);
    auto it = syntaxTreeSourceInfo.find(root);
    if (it == syntaxTreeSourceInfo.end())
      return nullptr;
    sourceInfo = it->second;
  }

  slang::DiagnosticEngine engine(*sourceInfo.sourceManager);
  auto range = mapSourceRangeToContext(engine, sourceInfo.rootLocation, rawRange);
  if (!range)
    return nullptr;

  return std::make_unique<SourceRange>(*range);
}

::RawSyntaxDiagnostic to_rust_syntax_diagnostic(const Diagnostic& diag,
                                                 slang::DiagnosticEngine& engine,
                                                 const slang::SourceManager& sourceManager) {
  ::RawSyntaxDiagnostic rust_diag;
  rust_diag.code = diag.code.getCode();
  rust_diag.subsystem = static_cast<uint16_t>(diag.code.getSubsystem());
  rust_diag.severity = static_cast<uint8_t>(engine.getSeverity(diag.code, diag.location));
  rust_diag.message = rust::String(engine.formatMessage(diag));
  rust_diag.args = diagnostic_args(diag);
  rust_diag.name = rust::String(std::string(slang::toString(diag.code)));
  auto option_name = engine.getOptionName(diag.code);
  rust_diag.option_name = rust::String(std::string(option_name));
  rust_diag.groups = rust::Vec<rust::String>();
  rust_diag.primary_range_start = 0;
  rust_diag.primary_range_end = 0;
  rust_diag.has_primary_range = false;
  rust_diag.location = 0;
  rust_diag.has_location = false;
  rust_diag.buffer_id = 0;
  rust_diag.has_buffer_id = false;
  rust_diag.file_name = rust::String();

  if (!diag.ranges.empty() && diag.ranges.front() != SourceRange::NoLocation) {
    if (diag.location.valid()) {
      auto location = sourceManager.getFullyExpandedLoc(diag.location);
      auto range = mapSourceRangeToContext(engine, location, diag.ranges.front());
      if (range) {
        rust_diag.primary_range_start = range->start().offset();
        rust_diag.primary_range_end = range->end().offset();
        rust_diag.has_primary_range = true;
      }
    }
  }

  if (diag.location.valid()) {
    auto location = sourceManager.getFullyExpandedLoc(diag.location);
    rust_diag.location = location.offset();
    rust_diag.has_location = true;
    rust_diag.buffer_id = location.buffer().getId();
    rust_diag.has_buffer_id = true;
    const auto& fullPath = sourceManager.getFullPath(location.buffer());
    if (!fullPath.empty())
      rust_diag.file_name = rust::String(fullPath.string());
    else
      rust_diag.file_name = rust::String(std::string(sourceManager.getFileName(location)));
  }

  return rust_diag;
}

::RawExpectedSyntax to_rust_expected_syntax(const slang::parsing::ExpectedSyntax& expected) {
  ::RawExpectedSyntax rust_expected;
  rust_expected.code = expected.code.getCode();
  rust_expected.subsystem = static_cast<uint16_t>(expected.code.getSubsystem());
  rust_expected.name = rust::String(std::string(slang::toString(expected.code)));
  rust_expected.token_kind = static_cast<uint16_t>(expected.tokenKind);
  rust_expected.keyword_context = 0;
  rust_expected.has_keyword_context = false;
  rust_expected.location = 0;
  rust_expected.has_location = false;

  if (expected.keywordContext) {
    rust_expected.keyword_context = static_cast<uint8_t>(*expected.keywordContext);
    rust_expected.has_keyword_context = true;
  }

  if (expected.location.valid()) {
    rust_expected.location = expected.location.offset();
    rust_expected.has_location = true;
  }

  return rust_expected;
}

rust::Vec<::RawExpectedSyntax> collect_expected_syntax(
    const std::shared_ptr<syntax::SyntaxTree>& tree) {
  rust::Vec<::RawExpectedSyntax> rust_expected;
  if (!tree || !tree->sharedInner())
    return rust_expected;

  const auto& expectedSyntax = tree->inner().getMetadata().expectedSyntax;
  rust_expected.reserve(expectedSyntax.size());
  for (const auto& expected : expectedSyntax)
    rust_expected.emplace_back(to_rust_expected_syntax(expected));
  return rust_expected;
}

} // namespace

namespace syntax {

::RawSyntaxTreeBufferIds SyntaxTree_buffer_ids(const SyntaxTree& tree) {
  return collectSyntaxTreeBufferIds(tree);
}

SyntaxTree::SyntaxTree(std::shared_ptr<::slang::syntax::SyntaxTree> tree,
                       std::shared_ptr<SourceSession> sourceSession) :
    innerTree(std::move(tree)), sourceSession(std::move(sourceSession)) {
  if (!innerTree)
    return;

  auto& root = innerTree->root();
  auto rootRange = root.sourceRange();
  if (rootRange == SourceRange::NoLocation)
    return;

  auto rootLocation = innerTree->sourceManager().getFullyExpandedLoc(rootRange.start());
  if (!rootLocation.valid())
    return;

  std::lock_guard lock(syntaxTreeSourceInfoMutex);
  syntaxTreeSourceInfo.emplace(
      &root,
      SyntaxTreeSourceInfo{
          &innerTree->sourceManager(), innerTree->getPreprocessorTrace(), rootLocation});
}

SyntaxTree::~SyntaxTree() {
  if (!innerTree)
    return;

  std::lock_guard lock(syntaxTreeSourceInfoMutex);
  syntaxTreeSourceInfo.erase(&innerTree->root());
}

SourceSession::SourceSession() : sourceManager(std::make_shared<slang::SourceManager>()) {}

slang::SourceBuffer SourceSession::assignSourceBuffer(
    std::string_view bufferPath,
    std::string_view bufferText) {
  if (bufferPath.empty())
    return {};

  auto key = source_manager_path_key(bufferPath);
  auto it = assignedBuffers.find(key);
  if (it != assignedBuffers.end())
    return it->second;

  std::string ownedText(bufferText);
  auto buffer = sourceManager->assignText(key, ownedText);
  assignedBuffers.emplace(std::move(key), buffer);
  return buffer;
}

std::shared_ptr<SyntaxTree> SourceSession::parseText(
    std::string_view text,
    std::string_view name,
    std::string_view path,
    rust::Vec<rust::String> predefines,
    rust::Vec<rust::String> include_paths,
    rust::Vec<::RawSourceBuffer> include_buffers,
    std::optional<size_t> expectedSyntaxCursor,
    bool expandIncludes,
    bool collectPreprocessorTrace) {
  slang::Bag options;
  auto& ppOptions = options.insertOrGet<slang::parsing::PreprocessorOptions>();
  for (const auto& predefine : predefines)
    ppOptions.predefines.emplace_back(std::string(predefine));
  for (const auto& include_path : include_paths)
    ppOptions.additionalIncludePaths.emplace_back(std::string(include_path));
  ppOptions.expandIncludes = expandIncludes;

  if (expectedSyntaxCursor) {
    slang::parsing::ExpectedSyntaxOptions expectedOptions;
    expectedOptions.cursorOffset = *expectedSyntaxCursor;
    options.set(expectedOptions);
  }

  for (const auto& buffer : include_buffers) {
    assignSourceBuffer(std::string(buffer.path), std::string(buffer.text));
  }

  auto traceMode = collectPreprocessorTrace
                       ? slang::syntax::PreprocessorTraceMode::Enabled
                       : slang::syntax::PreprocessorTraceMode::Disabled;
  std::shared_ptr<::slang::syntax::SyntaxTree> tree;
  if (path.empty()) {
    tree = ::slang::syntax::SyntaxTree::fromText(
        text, *sourceManager, name, path, options, nullptr, traceMode);
  }
  else {
    auto buffer = assignSourceBuffer(path, text);
    if (!name.empty())
      sourceManager->addLineDirective(slang::SourceLocation(buffer.id, 0), 2, name, 0);
    tree = ::slang::syntax::SyntaxTree::fromBuffer(buffer, *sourceManager, options, {},
                                                   traceMode);
  }

  return std::make_shared<SyntaxTree>(std::move(tree), shared_from_this());
}

std::shared_ptr<SyntaxTree> SourceSession::parseLibraryMapText(
    std::string_view text,
    std::string_view name,
    std::string_view path,
    std::optional<size_t> expectedSyntaxCursor) {
  slang::Bag options;
  if (expectedSyntaxCursor) {
    slang::parsing::ExpectedSyntaxOptions expectedOptions;
    expectedOptions.cursorOffset = *expectedSyntaxCursor;
    options.set(expectedOptions);
  }

  return std::make_shared<SyntaxTree>(
      ::slang::syntax::SyntaxTree::fromLibraryMapText(text, *sourceManager, name, path, options),
      shared_from_this());
}

std::shared_ptr<SyntaxTree> SyntaxTree_fromText(
    std::string_view text,
    std::string_view name,
    std::string_view path) {
  auto session = std::make_shared<SourceSession>();
  rust::Vec<rust::String> predefines;
  rust::Vec<rust::String> include_paths;
  rust::Vec<::RawSourceBuffer> include_buffers;
  return session->parseText(
      text,
      name,
      path,
      std::move(predefines),
      std::move(include_paths),
      std::move(include_buffers));
}

std::shared_ptr<SyntaxTree> SyntaxTree_fromTextWithOptions(
    std::string_view text,
    std::string_view name,
    std::string_view path,
    rust::Vec<rust::String> predefines,
    rust::Vec<rust::String> include_paths,
    rust::Vec<::RawSourceBuffer> include_buffers,
    bool expandIncludes) {
  auto session = std::make_shared<SourceSession>();
  return session->parseText(
      text,
      name,
      path,
      std::move(predefines),
      std::move(include_paths),
      std::move(include_buffers),
      std::nullopt,
      expandIncludes);
}

std::shared_ptr<SyntaxTree> SyntaxTree_fromTextWithOptionsAndTrace(
    std::string_view text,
    std::string_view name,
    std::string_view path,
    rust::Vec<rust::String> predefines,
    rust::Vec<rust::String> include_paths,
    rust::Vec<::RawSourceBuffer> include_buffers,
    bool expandIncludes) {
  auto session = std::make_shared<SourceSession>();
  return session->parseText(
      text,
      name,
      path,
      std::move(predefines),
      std::move(include_paths),
      std::move(include_buffers),
      std::nullopt,
      expandIncludes,
      true);
}

std::shared_ptr<SyntaxTree> SyntaxTree_fromLibraryMapText(
    std::string_view text,
    std::string_view name,
    std::string_view path) {
  auto session = std::make_shared<SourceSession>();
  return session->parseLibraryMapText(text, name, path);
}

rust::Vec<::RawSyntaxDiagnostic> SyntaxTree_diagnostics(const SyntaxTree& tree) {
  auto& inner = const_cast<SyntaxTree&>(tree).inner();
  auto& diags = inner.diagnostics();
  slang::DiagnosticEngine engine(inner.sourceManager());
  rust::Vec<::RawSyntaxDiagnostic> rust_diags;
  rust_diags.reserve(diags.size());
  for (const auto& diag : diags)
    rust_diags.emplace_back(to_rust_syntax_diagnostic(diag, engine, inner.sourceManager()));
  return rust_diags;
}

rust::Vec<::RawSyntaxDiagnostic> SyntaxTree_diagnostics_with_options(
    const SyntaxTree& tree,
    rust::Vec<rust::String> warning_options) {
  auto& inner = const_cast<SyntaxTree&>(tree).inner();
  auto& diags = inner.diagnostics();
  slang::DiagnosticEngine engine(inner.sourceManager());
  apply_warning_options(engine, warning_options);
  rust::Vec<::RawSyntaxDiagnostic> rust_diags;
  rust_diags.reserve(diags.size());
  for (const auto& diag : diags)
    rust_diags.emplace_back(to_rust_syntax_diagnostic(diag, engine, inner.sourceManager()));
  return rust_diags;
}

rust::Vec<::RawExpectedSyntax> SyntaxTree_expectedSyntaxAtOffset(
    std::string_view text,
    std::string_view name,
    std::string_view path,
    size_t offset,
    rust::Vec<rust::String> predefines,
    rust::Vec<rust::String> includePaths,
    rust::Vec<::RawSourceBuffer> includeBuffers,
    bool expandIncludes) {
  auto session = std::make_shared<SourceSession>();
  auto tree = session->parseText(
      text,
      name,
      path,
      std::move(predefines),
      std::move(includePaths),
      std::move(includeBuffers),
      offset,
      expandIncludes);
  return collect_expected_syntax(std::move(tree));
}

rust::Vec<::RawExpectedSyntax> SyntaxTree_libraryMapExpectedSyntaxAtOffset(
    std::string_view text,
    std::string_view name,
    std::string_view path,
    size_t offset) {
  auto session = std::make_shared<SourceSession>();
  auto tree = session->parseLibraryMapText(text, name, path, offset);
  return collect_expected_syntax(std::move(tree));
}

::RawLexedTokenAtOffset SyntaxTree_directiveAtOffset(
    std::string_view text,
    std::string_view name,
    std::string_view path,
    size_t offset) {
  auto token = lexTokenAtOffset(text, name, path, offset);
  auto result = emptyTokenAtOffset();
  if (!token.found || token.tokenKind != slang::parsing::TokenKind::Directive)
    return result;

  auto prefix_len = offset - token.start;
  if (token.rawText.size() < 2 || token.rawText[0] != '`' || token.rawText[1] == '\\' ||
      prefix_len == 0 || prefix_len > token.rawText.size()) {
    return result;
  }

  result.replacement_start = token.start + 1;
  result.replacement_end = token.end;
  result.prefix = rust::String(std::string(token.rawText.substr(1, prefix_len - 1)));
  result.token_kind = static_cast<uint16_t>(token.tokenKind);
  result.directive_kind = static_cast<uint16_t>(token.directiveKind);
  result.has_directive_kind = true;
  result.has_token = true;
  return result;
}

::RawLexedTokenAtOffset SyntaxTree_tokenWordAtOffset(
    std::string_view text,
    std::string_view name,
    std::string_view path,
    size_t offset) {
  auto token = lexTokenAtOffset(text, name, path, offset);
  auto result = emptyTokenAtOffset();
  if (!token.found ||
      (token.tokenKind != slang::parsing::TokenKind::Identifier &&
       token.tokenKind != slang::parsing::TokenKind::SystemIdentifier)) {
    return result;
  }

  auto prefix_len = offset - token.start;
  if (prefix_len > token.rawText.size())
    return result;

  result.replacement_start = token.start;
  result.replacement_end = token.end;
  result.prefix = rust::String(std::string(token.rawText.substr(0, prefix_len)));
  result.token_kind = static_cast<uint16_t>(token.tokenKind);
  result.has_token = true;
  return result;
}

::RawPreprocessorTrace SyntaxTree_preprocessorTraceFromParsed(const SyntaxTree& tree) {
  auto* trace = tree.inner().getPreprocessorTrace();
  if (!trace)
    return empty_preprocessor_trace();

  return to_rust_preprocessor_trace_snapshot(*trace, tree.inner().sourceManager());
}

std::unique_ptr<SourceRange> SyntaxNode_range(const SyntaxNode& node) {
  return mapRawSourceRangeWithContext(node.sourceRange(), node);
}

std::unique_ptr<SourceRange> SyntaxNode_rangeWithContext(
    const SyntaxNode& node,
    const SyntaxNode& context) {
  return mapRawSourceRangeWithContext(node.sourceRange(), context);
}

std::unique_ptr<SourceRange> SyntaxToken_rangeWithContext(
    const wrapper::parsing::Token& token,
    const SyntaxNode& context) {
  return mapRawSourceRangeWithContext(token.range(), context);
}

::RawPreprocessorTraceEmittedToken SyntaxToken_preprocessorTraceOriginWithContext(
    const wrapper::parsing::Token& token,
    const SyntaxNode& context) {
  const auto* root = findRoot(context);
  SyntaxTreeSourceInfo sourceInfo;
  {
    std::lock_guard lock(syntaxTreeSourceInfoMutex);
    auto it = syntaxTreeSourceInfo.find(root);
    if (it == syntaxTreeSourceInfo.end())
      return empty_preprocessor_trace_emitted_token();
    sourceInfo = it->second;
  }

  return to_rust_preprocessor_trace_emitted_token(
      token, *sourceInfo.sourceManager,
      emitted_token_index_for(sourceInfo.preprocessorTrace, token));
}

} // namespace syntax

namespace ast {

std::shared_ptr<syntax::SyntaxTree> Compilation::parseSyntaxTreeFromText(
    std::string_view text,
    std::string_view name,
    std::string_view path,
    rust::Vec<rust::String> predefines,
    rust::Vec<rust::String> includePaths,
    rust::Vec<::RawSourceBuffer> includeBuffers) {
  return sourceSession->parseText(
      text,
      name,
      path,
      std::move(predefines),
      std::move(includePaths),
      std::move(includeBuffers),
      std::nullopt,
      true,
      true);
}

std::shared_ptr<syntax::SyntaxTree> Compilation::parseLibraryMapSyntaxTreeFromText(
    std::string_view text,
    std::string_view name,
    std::string_view path) {
  return sourceSession->parseLibraryMapText(text, name, path);
}

::RawSyntaxTreeBufferIds Compilation::addSyntaxTreeFromText(
    std::string_view text,
    std::string_view name,
    std::string_view path,
    rust::Vec<rust::String> predefines,
    rust::Vec<rust::String> includePaths,
    rust::Vec<::RawSourceBuffer> includeBuffers,
    bool expandIncludes) {
  auto tree = sourceSession->parseText(
      text,
      name,
      path,
      std::move(predefines),
      std::move(includePaths),
      std::move(includeBuffers),
      std::nullopt,
      expandIncludes);
  auto bufferIds = collectSyntaxTreeBufferIds(*tree);
  addSyntaxTree(std::move(tree));
  return bufferIds;
}

::RawSyntaxTreeBufferIds Compilation::addLibraryMapSyntaxTreeFromText(
    std::string_view text,
    std::string_view name,
    std::string_view path) {
  auto tree = sourceSession->parseLibraryMapText(text, name, path);
  auto bufferIds = collectSyntaxTreeBufferIds(*tree);
  addSyntaxTree(std::move(tree));
  return bufferIds;
}

::RawSyntaxTreeBufferIds Compilation_add_syntax_tree_from_text(
    Compilation& compilation,
    std::string_view text,
    std::string_view name,
    std::string_view path,
    rust::Vec<rust::String> predefines,
    rust::Vec<rust::String> includePaths,
    rust::Vec<::RawSourceBuffer> includeBuffers,
    bool expandIncludes) {
  return compilation.addSyntaxTreeFromText(
      text,
      name,
      path,
      std::move(predefines),
      std::move(includePaths),
      std::move(includeBuffers),
      expandIncludes);
}

::RawSyntaxTreeBufferIds Compilation_add_library_map_syntax_tree_from_text(
    Compilation& compilation,
    std::string_view text,
    std::string_view name,
    std::string_view path) {
  return compilation.addLibraryMapSyntaxTreeFromText(text, name, path);
}

rust::Vec<::RawSyntaxDiagnostic> Compilation_semantic_diagnostics(const Compilation& compilation) {
  auto& inner = const_cast<Compilation&>(compilation).inner();
  auto& diags = inner.getSemanticDiagnostics();
  auto source_manager = inner.getSourceManager();
  if (!source_manager)
    return {};
  slang::DiagnosticEngine engine(*source_manager);
  rust::Vec<::RawSyntaxDiagnostic> rust_diags;
  rust_diags.reserve(diags.size());
  for (const auto& diag : diags)
    rust_diags.emplace_back(to_rust_syntax_diagnostic(diag, engine, *source_manager));
  return rust_diags;
}

rust::Vec<::RawSyntaxDiagnostic> Compilation_parse_diagnostics_with_options(
    const Compilation& compilation,
    rust::Vec<rust::String> warning_options) {
  auto& inner = const_cast<Compilation&>(compilation).inner();
  auto& diags = inner.getParseDiagnostics();
  auto source_manager = inner.getSourceManager();
  if (!source_manager)
    return {};
  slang::DiagnosticEngine engine(*source_manager);
  apply_warning_options(engine, warning_options);
  rust::Vec<::RawSyntaxDiagnostic> rust_diags;
  rust_diags.reserve(diags.size());
  for (const auto& diag : diags)
    rust_diags.emplace_back(to_rust_syntax_diagnostic(diag, engine, *source_manager));
  return rust_diags;
}

rust::Vec<::RawSyntaxDiagnostic> Compilation_semantic_diagnostics_with_options(
    const Compilation& compilation,
    rust::Vec<rust::String> warning_options) {
  auto& inner = const_cast<Compilation&>(compilation).inner();
  auto& diags = inner.getSemanticDiagnostics();
  auto source_manager = inner.getSourceManager();
  if (!source_manager)
    return {};
  slang::DiagnosticEngine engine(*source_manager);
  apply_warning_options(engine, warning_options);
  rust::Vec<::RawSyntaxDiagnostic> rust_diags;
  rust_diags.reserve(diags.size());
  for (const auto& diag : diags)
    rust_diags.emplace_back(to_rust_syntax_diagnostic(diag, engine, *source_manager));
  return rust_diags;
}

} // namespace ast
} // namespace wrapper
