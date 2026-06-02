#include "slang/bindings/rust/ffi.rs.h"

#include "slang/parsing/ExpectedSyntax.h"
#include "slang/parsing/ParserMetadata.h"
#include "slang/syntax/AllSyntax.h"

#include <filesystem>
#include <mutex>

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

void set_rust_range(size_t& start, size_t& end, bool& hasRange, slang::SourceRange range) {
  start = 0;
  end = 0;
  hasRange = false;
  if (!range.start().valid() || !range.end().valid())
    return;

  start = range.start().offset();
  end = range.end().offset();
  hasRange = true;
}

::RawTextRange to_rust_text_range(slang::SourceRange range) {
  ::RawTextRange result;
  set_rust_range(result.range_start, result.range_end, result.has_range, range);
  return result;
}

::RawPreprocessorToken empty_preprocessor_token() {
  ::RawPreprocessorToken token;
  token.raw_text = rust::String();
  token.value_text = rust::String();
  token.range_start = 0;
  token.range_end = 0;
  token.has_range = false;
  token.has_token = false;
  return token;
}

::RawPreprocessorToken to_rust_preprocessor_token(slang::parsing::Token token) {
  auto result = empty_preprocessor_token();
  if (!token)
    return result;

  result.raw_text = rust::String(std::string(token.rawText()));
  result.value_text = rust::String(std::string(token.valueText()));
  set_rust_range(result.range_start, result.range_end, result.has_range, token.range());
  result.has_token = true;
  return result;
}

template<typename TTokens>
rust::Vec<::RawTextRange> to_rust_disabled_ranges(const TTokens& tokens) {
  rust::Vec<::RawTextRange> result;
  std::optional<::RawTextRange> merged;

  for (auto token : tokens) {
    auto range = to_rust_text_range(token.range());
    if (!range.has_range)
      continue;

    if (!merged) {
      merged = range;
      continue;
    }

    merged->range_start = std::min(merged->range_start, range.range_start);
    merged->range_end = std::max(merged->range_end, range.range_end);
  }

  if (merged && merged->range_start < merged->range_end)
    result.emplace_back(*merged);
  return result;
}

template<typename TTokens>
rust::Vec<::RawPreprocessorToken> to_rust_preprocessor_tokens(const TTokens& tokens) {
  rust::Vec<::RawPreprocessorToken> result;
  for (auto token : tokens)
    result.emplace_back(to_rust_preprocessor_token(token));
  return result;
}

void collect_leaf_tokens(const slang::syntax::SyntaxNode& node,
                         rust::Vec<::RawPreprocessorToken>& tokens) {
  for (size_t i = 0; i < node.getChildCount(); i++) {
    if (auto token = node.childToken(i))
      tokens.emplace_back(to_rust_preprocessor_token(token));
    if (auto* child = node.childNode(i))
      collect_leaf_tokens(*child, tokens);
  }
}

::RawPreprocessorMacroParam to_rust_macro_param(
    const slang::syntax::MacroFormalArgumentSyntax& param) {
  ::RawPreprocessorMacroParam result;
  result.name = to_rust_preprocessor_token(param.name);
  result.default_tokens = rust::Vec<::RawPreprocessorToken>();
  result.has_default = param.defaultValue != nullptr;
  result.range_start = 0;
  result.range_end = 0;
  result.has_range = false;
  set_rust_range(result.range_start, result.range_end, result.has_range, param.sourceRange());
  if (param.defaultValue)
    result.default_tokens = to_rust_preprocessor_tokens(param.defaultValue->tokens);
  return result;
}

::RawPreprocessorDirective to_rust_preprocessor_directive(
    const slang::syntax::SyntaxNode& syntax) {
  ::RawPreprocessorDirective directive;
  directive.kind = static_cast<uint16_t>(syntax.kind);
  directive.directive = empty_preprocessor_token();
  directive.name = empty_preprocessor_token();
  directive.include_file_name = empty_preprocessor_token();
  directive.params = rust::Vec<::RawPreprocessorMacroParam>();
  directive.body_tokens = rust::Vec<::RawPreprocessorToken>();
  directive.expr_tokens = rust::Vec<::RawPreprocessorToken>();
  directive.disabled_ranges = rust::Vec<::RawTextRange>();
  set_rust_range(directive.range_start, directive.range_end, directive.has_range,
                 syntax.sourceRange());

  if (auto* directiveSyntax = syntax.as_if<slang::syntax::DirectiveSyntax>())
    directive.directive = to_rust_preprocessor_token(directiveSyntax->directive);

  switch (syntax.kind) {
    case slang::syntax::SyntaxKind::DefineDirective: {
      const auto& define = syntax.as<slang::syntax::DefineDirectiveSyntax>();
      directive.name = to_rust_preprocessor_token(define.name);
      if (define.formalArguments) {
        for (auto* param : define.formalArguments->args)
          directive.params.emplace_back(to_rust_macro_param(*param));
      }
      directive.body_tokens = to_rust_preprocessor_tokens(define.body);
      break;
    }
    case slang::syntax::SyntaxKind::UndefDirective: {
      const auto& undef = syntax.as<slang::syntax::UndefDirectiveSyntax>();
      directive.name = to_rust_preprocessor_token(undef.name);
      break;
    }
    case slang::syntax::SyntaxKind::IncludeDirective: {
      const auto& include = syntax.as<slang::syntax::IncludeDirectiveSyntax>();
      directive.include_file_name = to_rust_preprocessor_token(include.fileName);
      break;
    }
    case slang::syntax::SyntaxKind::IfDefDirective:
    case slang::syntax::SyntaxKind::IfNDefDirective:
    case slang::syntax::SyntaxKind::ElsIfDirective: {
      const auto& branch = syntax.as<slang::syntax::ConditionalBranchDirectiveSyntax>();
      collect_leaf_tokens(*branch.expr, directive.expr_tokens);
      directive.disabled_ranges = to_rust_disabled_ranges(branch.disabledTokens);
      break;
    }
    case slang::syntax::SyntaxKind::ElseDirective:
    case slang::syntax::SyntaxKind::EndIfDirective: {
      const auto& branch = syntax.as<slang::syntax::UnconditionalBranchDirectiveSyntax>();
      directive.disabled_ranges = to_rust_disabled_ranges(branch.disabledTokens);
      break;
    }
    case slang::syntax::SyntaxKind::MacroUsage: {
      const auto& usage = syntax.as<slang::syntax::MacroUsageSyntax>();
      directive.name = to_rust_preprocessor_token(usage.directive);
      break;
    }
    default:
      break;
  }

  return directive;
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

::RawSyntaxTreeBufferIds collectSyntaxTreeBufferIds(const syntax::SyntaxTree& tree) {
  ::RawSyntaxTreeBufferIds ids;
  ids.root_buffer_id = tree.inner().root().sourceRange().start().buffer().getId();
  ids.source_buffers = rust::Vec<::RawSourceBufferId>();

  const auto& sourceManager = tree.inner().sourceManager();
  for (auto buffer : sourceManager.getAllBuffers()) {
    const auto& fullPath = sourceManager.getFullPath(buffer);
    if (fullPath.empty())
      continue;

    ::RawSourceBufferId sourceBuffer;
    sourceBuffer.path = rust::String(fullPath.string());
    sourceBuffer.buffer_id = buffer.getId();
    ids.source_buffers.emplace_back(std::move(sourceBuffer));
  }

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
      SyntaxTreeSourceInfo{&innerTree->sourceManager(), rootLocation});
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
    bool expandIncludes) {
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

  std::shared_ptr<::slang::syntax::SyntaxTree> tree;
  if (path.empty()) {
    tree = ::slang::syntax::SyntaxTree::fromText(text, *sourceManager, name, path, options);
  }
  else {
    auto buffer = assignSourceBuffer(path, text);
    if (!name.empty())
      sourceManager->addLineDirective(slang::SourceLocation(buffer.id, 0), 2, name, 0);
    tree = ::slang::syntax::SyntaxTree::fromBuffer(buffer, *sourceManager, options);
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

rust::Vec<::RawPreprocessorDirective> SyntaxTree_preprocessorDirectives(
    std::string_view text,
    std::string_view name,
    std::string_view path,
    rust::Vec<rust::String> predefines) {
  rust::Vec<::RawPreprocessorDirective> result;

  slang::SourceManager sourceManager;
  auto bufferPath = path.empty() ? (name.empty() ? std::string_view("source") : name) : path;
  auto buffer = sourceManager.assignText(bufferPath, text);
  if (!buffer)
    return result;

  slang::Bag options;
  auto& ppOptions = options.insertOrGet<slang::parsing::PreprocessorOptions>();
  ppOptions.expandIncludes = false;
  for (const auto& predefine : predefines)
    ppOptions.predefines.emplace_back(std::string(predefine));

  slang::BumpAllocator alloc;
  slang::Diagnostics diagnostics;
  slang::parsing::Preprocessor preprocessor(sourceManager, alloc, diagnostics, options);
  preprocessor.pushSource(buffer);

  while (true) {
    auto token = preprocessor.next();
    for (auto trivia : token.trivia()) {
      if (trivia.kind != slang::parsing::TriviaKind::Directive)
        continue;

      if (auto* syntax = trivia.syntax())
        result.emplace_back(to_rust_preprocessor_directive(*syntax));
    }

    if (token.kind == slang::parsing::TokenKind::EndOfFile)
      break;
  }

  return result;
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

} // namespace syntax

namespace ast {

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
