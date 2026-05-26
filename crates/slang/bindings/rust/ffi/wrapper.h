#pragma once
#include <algorithm>
#include <atomic>
#include <cassert>
#include <cstddef>
#include <filesystem>
#include <iostream>
#include <memory>
#include <optional>
#include <unordered_map>
#include <unordered_set>
#include <string>
#include <string_view>
#include <utility>
#include <vector>
#include "slang/syntax/SyntaxTree.h"
#include "slang/syntax/SyntaxNode.h"
#include "slang/syntax/SyntaxFacts.h"
#include "slang/ast/SemanticFacts.h"
#include "slang/numeric/SVInt.h"
#include "slang/parsing/Lexer.h"
#include "slang/parsing/LexerFacts.h"
#include "slang/parsing/Preprocessor.h"
#include "slang/syntax/SyntaxPrinter.h"
#include "slang/text/SourceLocation.h"
#include "slang/text/SourceManager.h"
#include "slang/ast/Compilation.h"
#include "slang/diagnostics/DiagnosticEngine.h"
#include "slang/diagnostics/Diagnostics.h"
#include "slang/util/Bag.h"
#include "rust/cxx.h"

struct RawSyntaxDiagnostic;
struct RawSourceBuffer;
struct RawSourceBufferId;
struct RawSyntaxTreeBufferIds;
struct RawExpectedSyntax;
struct RawLexedTokenAtOffset;
struct RawPreprocessorDirective;
struct RawTextRange;

namespace wrapper {
  using Diagnostic = ::slang::Diagnostic;
  using SourceLocation = ::slang::SourceLocation;
  using SourceRange = ::slang::SourceRange;
  using SVInt = ::slang::SVInt;
  using logic_t = ::slang::logic_t;

  using SyntaxTrivia = ::slang::parsing::Trivia;
  using SyntaxToken = ::slang::parsing::Token;
  using SyntaxTree = ::slang::syntax::SyntaxTree;
  using SyntaxNode = ::slang::syntax::SyntaxNode;
  using Compilation = ::slang::ast::Compilation;

  namespace parsing {
    using Trivia = ::slang::parsing::Trivia;
    using Token = ::slang::parsing::Token;
  }

  namespace ast {
    class Compilation;
  }

  namespace syntax {
    using SyntaxNode = ::slang::syntax::SyntaxNode;

    class SourceSession;

    class SyntaxTree {
    public:
      SyntaxTree(std::shared_ptr<::slang::syntax::SyntaxTree> tree,
                 std::shared_ptr<SourceSession> sourceSession);
      ~SyntaxTree();

      ::slang::syntax::SyntaxTree& inner() { return *innerTree; }
      const ::slang::syntax::SyntaxTree& inner() const { return *innerTree; }
      std::shared_ptr<::slang::syntax::SyntaxTree> sharedInner() const { return innerTree; }

    private:
      std::shared_ptr<::slang::syntax::SyntaxTree> innerTree;
      std::shared_ptr<SourceSession> sourceSession;
    };

    class SourceSession : public std::enable_shared_from_this<SourceSession> {
    public:
      SourceSession();

      std::shared_ptr<SyntaxTree> parseText(
          std::string_view text,
          std::string_view name,
          std::string_view path,
          rust::Vec<rust::String> predefines,
          rust::Vec<rust::String> includePaths,
          rust::Vec<::RawSourceBuffer> includeBuffers,
          std::optional<size_t> expectedSyntaxCursor = std::nullopt,
          bool expandIncludes = true);

      std::shared_ptr<SyntaxTree> parseLibraryMapText(
          std::string_view text,
          std::string_view name,
          std::string_view path,
          std::optional<size_t> expectedSyntaxCursor = std::nullopt);

    private:
      friend class wrapper::ast::Compilation;

      slang::SourceBuffer assignSourceBuffer(std::string_view bufferPath,
                                             std::string_view bufferText);

      std::shared_ptr<slang::SourceManager> sourceManager;
      std::unordered_map<std::string, slang::SourceBuffer> assignedBuffers;
    };
  }

  namespace ast {
    class Compilation {
    public:
      explicit Compilation(rust::Vec<rust::String> topModules) {
        slang::Bag options;
        auto& compilationOptions = options.insertOrGet<slang::ast::CompilationOptions>();
        for (const auto& top : topModules)
          topModuleStorage.emplace_back(std::string(top));
        for (const auto& top : topModuleStorage)
          compilationOptions.topModules.emplace(top);
        innerCompilation = std::make_unique<slang::ast::Compilation>(options);
      }

      slang::ast::Compilation& inner() { return *innerCompilation; }
      const slang::ast::Compilation& inner() const { return *innerCompilation; }

      void addSyntaxTree(std::shared_ptr<syntax::SyntaxTree> tree) {
        syntaxTreeStorage.push_back(tree);
        innerCompilation->addSyntaxTree(tree->sharedInner());
      }

      ::RawSyntaxTreeBufferIds addSyntaxTreeFromText(
          std::string_view text,
          std::string_view name,
          std::string_view path,
          rust::Vec<rust::String> predefines,
          rust::Vec<rust::String> includePaths,
          rust::Vec<::RawSourceBuffer> includeBuffers,
          bool expandIncludes);

      ::RawSyntaxTreeBufferIds addLibraryMapSyntaxTreeFromText(std::string_view text,
                                                               std::string_view name,
                                                               std::string_view path);

    private:
      std::vector<std::string> topModuleStorage;
      std::vector<std::shared_ptr<syntax::SyntaxTree>> syntaxTreeStorage;
      std::shared_ptr<syntax::SourceSession> sourceSession =
          std::make_shared<syntax::SourceSession>();
      std::unique_ptr<slang::ast::Compilation> innerCompilation;
    };
  }

  // SourceLocation
  inline static uint32_t source_location_buffer_id(const slang::SourceLocation& location) {
    return location.buffer().getId();
  }

  // SourceRange
  inline static size_t source_range_start(const slang::SourceRange& range) {
    return range.start().offset();
  }

  inline static size_t source_range_end(const slang::SourceRange& range) {
    return range.end().offset();
  }

  inline static uint32_t source_range_start_buffer_id(const slang::SourceRange& range) {
    return range.start().buffer().getId();
  }

  inline static uint32_t source_range_end_buffer_id(const slang::SourceRange& range) {
    return range.end().buffer().getId();
  }

  inline static uint8_t logic_t_value(const slang::logic_t& logic) {
    return logic.value;
  }

  inline static rust::string SVInt_toString(const SVInt& svint, size_t base) {
    switch (base) {
      case 2:
        return rust::String(svint.toString(slang::LiteralBase::Binary, false));
      case 8:
        return rust::String(svint.toString(slang::LiteralBase::Octal, false));
      case 16:
        return rust::String(svint.toString(slang::LiteralBase::Hex, false));
      case 10:
        return rust::String(svint.toString(slang::LiteralBase::Decimal, false));
      default:
        assert(false);
    }
  }

  inline static std::unique_ptr<slang::SVInt> SVInt_clone(const SVInt& svint) {
    return std::make_unique<SVInt>(svint);
  }

  inline static std::unique_ptr<slang::logic_t> SVInt_eq(const SVInt& lhs, const SVInt& rhs) {
    return std::make_unique<logic_t>(lhs == rhs);
  }

  namespace parsing {
    // Trivia
    inline static uint8_t SyntaxTrivia_kind(const SyntaxTrivia& trivia) {
      return static_cast<uint8_t>(trivia.kind);
    }

    inline static const SyntaxNode* SyntaxTrivia_syntax(const SyntaxTrivia& trivia) {
      return trivia.syntax();
    }

    inline static std::unique_ptr<SourceLocation> SyntaxTrivia_getExplicitLocation(
        const SyntaxTrivia& trivia) {
      auto location = trivia.getExplicitLocation();
      return location ? std::make_unique<SourceLocation>(*location) : nullptr;
    }

    // Token
    inline static size_t SyntaxToken_trivia_count(const SyntaxToken& token) {
      return token.trivia().size();
    }

    inline static const SyntaxTrivia* SyntaxToken_trivia(const SyntaxToken& token, size_t index) {
      return &token.trivia()[index];
    }

    inline static uint16_t SyntaxToken_kind(const SyntaxToken& token) {
      return static_cast<uint16_t>(token.kind);
    }

    inline static std::unique_ptr<SourceRange> SyntaxToken_range(const SyntaxToken& token) {
      auto range = token.range();
      return range == SourceRange::NoLocation ? nullptr : std::make_unique<SourceRange>(range);
    }

    inline static std::unique_ptr<SVInt> SyntaxToken_intValue(const SyntaxToken& token) {
      return std::make_unique<SVInt>(token.intValue());
    }

    inline static std::unique_ptr<logic_t> SyntaxToken_bitValue(const SyntaxToken& token) {
      return std::make_unique<logic_t>(token.bitValue());
    }

    inline static uint8_t SyntaxToken_base(const SyntaxToken& token) {
      return static_cast<uint8_t>(token.numericFlags().base());
    }

    inline static uint8_t SyntaxToken_unit(const SyntaxToken& token) {
      return static_cast<uint8_t>(token.numericFlags().unit());
    }

    inline static rust::Vec<rust::String> LexerFacts_keyword_table_for_version(std::string_view version) {
      rust::Vec<rust::String> keywords;
      auto keywordVersion = slang::parsing::LexerFacts::getKeywordVersion(version);
      if (!keywordVersion)
        return keywords;

      auto* table = slang::parsing::LexerFacts::getKeywordTable(*keywordVersion);
      if (!table)
        return keywords;

      keywords.reserve(table->size());
      for (const auto& [text, _] : *table)
        keywords.emplace_back(text.data(), text.size());

      return keywords;
    }

    inline static uint16_t LexerFacts_keyword_kind_for_version(std::string_view version,
                                                               std::string_view text) {
      auto keywordVersion = slang::parsing::LexerFacts::getKeywordVersion(version);
      if (!keywordVersion)
        return static_cast<uint16_t>(slang::parsing::TokenKind::Unknown);

      auto* table = slang::parsing::LexerFacts::getKeywordTable(*keywordVersion);
      if (!table)
        return static_cast<uint16_t>(slang::parsing::TokenKind::Unknown);

      auto it = table->find(text);
      if (it == table->end())
        return static_cast<uint16_t>(slang::parsing::TokenKind::Unknown);

      return static_cast<uint16_t>(it->second);
    }

    inline static rust::Vec<rust::String> LexerFacts_verilog_2005_keywords() {
      rust::Vec<rust::String> keywords;
      auto* table = slang::parsing::LexerFacts::getKeywordTable(slang::parsing::KeywordVersion::v1364_2005);
      if (!table)
        return keywords;

      keywords.reserve(table->size());
      for (const auto& [text, _] : *table)
        keywords.emplace_back(text.data(), text.size());

      return keywords;
    }

    inline static rust::String LexerFacts_directive_text(uint16_t kind) {
      return rust::String(std::string(
          slang::parsing::LexerFacts::getDirectiveText(static_cast<slang::syntax::SyntaxKind>(kind))));
    }

    inline static bool SyntaxFacts_is_possible_statement(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isPossibleStatement(
          static_cast<slang::parsing::TokenKind>(kind));
    }

    inline static bool SyntaxFacts_is_possible_expression(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isPossibleExpression(
          static_cast<slang::parsing::TokenKind>(kind));
    }

    inline static bool SyntaxFacts_is_possible_data_type(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isPossibleDataType(
          static_cast<slang::parsing::TokenKind>(kind));
    }

    inline static bool SyntaxFacts_is_possible_argument(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isPossibleArgument(
          static_cast<slang::parsing::TokenKind>(kind));
    }

    inline static bool SyntaxFacts_is_possible_param_assignment(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isPossibleParamAssignment(
          static_cast<slang::parsing::TokenKind>(kind));
    }

    inline static bool SyntaxFacts_is_possible_port_connection(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isPossiblePortConnection(
          static_cast<slang::parsing::TokenKind>(kind));
    }

    inline static bool SyntaxFacts_is_possible_ansi_port(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isPossibleAnsiPort(
          static_cast<slang::parsing::TokenKind>(kind));
    }

    inline static bool SyntaxFacts_is_possible_non_ansi_port(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isPossibleNonAnsiPort(
          static_cast<slang::parsing::TokenKind>(kind));
    }

    inline static bool SyntaxFacts_is_possible_function_port(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isPossibleFunctionPort(
          static_cast<slang::parsing::TokenKind>(kind));
    }

    inline static bool SyntaxFacts_is_possible_parameter(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isPossibleParameter(
          static_cast<slang::parsing::TokenKind>(kind));
    }

    inline static bool SyntaxFacts_is_gate_type(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isGateType(
          static_cast<slang::parsing::TokenKind>(kind));
    }

    inline static bool SemanticFacts_is_edge_kind(uint16_t kind) {
      return slang::ast::SemanticFacts::getEdgeKind(
          static_cast<slang::parsing::TokenKind>(kind)) != slang::ast::EdgeKind::None;
    }

    inline static bool SyntaxFacts_is_port_direction(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isPortDirection(
          static_cast<slang::parsing::TokenKind>(kind));
    }

    inline static bool SyntaxFacts_is_net_type(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isNetType(
          static_cast<slang::parsing::TokenKind>(kind));
    }

    inline static uint16_t SyntaxFacts_get_integer_type(uint16_t kind) {
      return static_cast<uint16_t>(slang::syntax::SyntaxFacts::getIntegerType(
          static_cast<slang::parsing::TokenKind>(kind)));
    }

    inline static uint16_t SyntaxFacts_get_keyword_type(uint16_t kind) {
      return static_cast<uint16_t>(slang::syntax::SyntaxFacts::getKeywordType(
          static_cast<slang::parsing::TokenKind>(kind)));
    }

    inline static uint16_t SyntaxFacts_get_procedural_block_kind(uint16_t kind) {
      return static_cast<uint16_t>(slang::syntax::SyntaxFacts::getProceduralBlockKind(
          static_cast<slang::parsing::TokenKind>(kind)));
    }

    inline static uint16_t SyntaxFacts_get_module_declaration_kind(uint16_t kind) {
      return static_cast<uint16_t>(slang::syntax::SyntaxFacts::getModuleDeclarationKind(
          static_cast<slang::parsing::TokenKind>(kind)));
    }

    inline static bool SyntaxFacts_is_possible_member_kind(uint16_t tokenKind, uint16_t memberKind) {
      return slang::syntax::SyntaxFacts::isPossibleMemberKind(
          static_cast<slang::parsing::TokenKind>(tokenKind),
          static_cast<slang::syntax::SyntaxKind>(memberKind));
    }

    inline static uint16_t SyntaxFacts_get_block_item_declaration_kind(uint16_t kind) {
      return static_cast<uint16_t>(slang::syntax::SyntaxFacts::getBlockItemDeclarationKind(
          static_cast<slang::parsing::TokenKind>(kind)));
    }

    inline static uint16_t SyntaxFacts_get_library_map_member_kind(uint16_t kind) {
      return static_cast<uint16_t>(slang::syntax::SyntaxFacts::getLibraryMapMemberKind(
          static_cast<slang::parsing::TokenKind>(kind)));
    }

    inline static uint16_t SyntaxFacts_get_specify_item_kind(uint16_t kind) {
      return static_cast<uint16_t>(slang::syntax::SyntaxFacts::getSpecifyItemKind(
          static_cast<slang::parsing::TokenKind>(kind)));
    }

    inline static uint16_t SyntaxFacts_get_config_header_item_kind(uint16_t kind) {
      return static_cast<uint16_t>(slang::syntax::SyntaxFacts::getConfigHeaderItemKind(
          static_cast<slang::parsing::TokenKind>(kind)));
    }

    inline static uint16_t SyntaxFacts_get_config_rule_kind(uint16_t kind) {
      return static_cast<uint16_t>(slang::syntax::SyntaxFacts::getConfigRuleKind(
          static_cast<slang::parsing::TokenKind>(kind)));
    }

    inline static rust::Vec<rust::String> SyntaxFacts_keyword_candidates_for_context(
        std::string_view version,
        uint8_t context) {
      rust::Vec<rust::String> keywords;
      auto keywordVersion = slang::parsing::LexerFacts::getKeywordVersion(version);
      if (!keywordVersion)
        return keywords;

      auto candidates = slang::syntax::SyntaxFacts::getKeywordCandidates(
          *keywordVersion,
          static_cast<slang::syntax::SyntaxKeywordContext>(context));
      keywords.reserve(candidates.size());
      for (const auto& keyword : candidates)
        keywords.emplace_back(keyword.data(), keyword.size());

      return keywords;
    }
  }

  namespace syntax {
    std::shared_ptr<SyntaxTree> SyntaxTree_fromText(
        std::string_view text,
        std::string_view name,
        std::string_view path);

    std::shared_ptr<SyntaxTree> SyntaxTree_fromTextWithOptions(
        std::string_view text,
        std::string_view name,
        std::string_view path,
        rust::Vec<rust::String> predefines,
        rust::Vec<rust::String> include_paths,
        rust::Vec<::RawSourceBuffer> include_buffers,
        bool expandIncludes);

    std::shared_ptr<SyntaxTree> SyntaxTree_fromLibraryMapText(
        std::string_view text,
        std::string_view name,
        std::string_view path);

    inline static const SyntaxNode* SyntaxTree_root(const SyntaxTree& tree) {
      return &tree.inner().root();
    }

    inline static uint32_t SyntaxTree_buffer_id(const SyntaxTree& tree) {
      return tree.inner().root().sourceRange().start().buffer().getId();
    }

    std::unique_ptr<SourceRange> SyntaxNode_range(const SyntaxNode& node);

    std::unique_ptr<SourceRange> SyntaxNode_rangeWithContext(
        const SyntaxNode& node,
        const SyntaxNode& context);

    std::unique_ptr<SourceRange> SyntaxToken_rangeWithContext(
        const wrapper::parsing::Token& token,
        const SyntaxNode& context);

    inline static const SyntaxToken* SyntaxNode_childToken(const SyntaxNode& node, size_t index) {
      // Since the function returns a const ptr, so we garentee the node won't be modified.
      return (const_cast<SyntaxNode&>(node)).childTokenPtr(index);
    }

    inline static const SyntaxNode* SyntaxNode_parent(const SyntaxNode& node) {
      return node.parent;
    }

    inline static uint16_t SyntaxNode_kind(const SyntaxNode& node) {
      return static_cast<uint16_t>(node.kind);
    }

    inline static bool SyntaxFacts_is_allowed_in_compilation_unit(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isAllowedInCompilationUnit(
          static_cast<slang::syntax::SyntaxKind>(kind));
    }

    inline static bool SyntaxFacts_is_allowed_in_generate(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isAllowedInGenerate(
          static_cast<slang::syntax::SyntaxKind>(kind));
    }

    inline static bool SyntaxFacts_is_allowed_in_module(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isAllowedInModule(
          static_cast<slang::syntax::SyntaxKind>(kind));
    }

    inline static bool SyntaxFacts_is_allowed_in_interface(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isAllowedInInterface(
          static_cast<slang::syntax::SyntaxKind>(kind));
    }

    inline static bool SyntaxFacts_is_allowed_in_program(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isAllowedInProgram(
          static_cast<slang::syntax::SyntaxKind>(kind));
    }

    inline static bool SyntaxFacts_is_allowed_in_package(uint16_t kind) {
      return slang::syntax::SyntaxFacts::isAllowedInPackage(
          static_cast<slang::syntax::SyntaxKind>(kind));
    }

    rust::Vec<::RawSyntaxDiagnostic> SyntaxTree_diagnostics(const SyntaxTree& tree);
    rust::Vec<::RawSyntaxDiagnostic> SyntaxTree_diagnostics_with_options(
        const SyntaxTree& tree,
        rust::Vec<rust::String> warning_options);

    rust::Vec<::RawExpectedSyntax> SyntaxTree_expectedSyntaxAtOffset(
        std::string_view text,
        std::string_view name,
        std::string_view path,
        size_t offset,
        rust::Vec<rust::String> predefines,
        rust::Vec<rust::String> includePaths,
        rust::Vec<::RawSourceBuffer> includeBuffers,
        bool expandIncludes);

    rust::Vec<::RawExpectedSyntax> SyntaxTree_libraryMapExpectedSyntaxAtOffset(
        std::string_view text,
        std::string_view name,
        std::string_view path,
        size_t offset);

    ::RawLexedTokenAtOffset SyntaxTree_directiveAtOffset(
        std::string_view text,
        std::string_view name,
        std::string_view path,
        size_t offset);

    ::RawLexedTokenAtOffset SyntaxTree_tokenWordAtOffset(
        std::string_view text,
        std::string_view name,
        std::string_view path,
        size_t offset);
    rust::Vec<::RawPreprocessorDirective> SyntaxTree_preprocessorDirectives(
        std::string_view text,
        std::string_view name,
        std::string_view path,
        rust::Vec<rust::String> predefines);
  }

  namespace ast {
    inline static std::unique_ptr<Compilation> Compilation_new() {
        return std::make_unique<Compilation>(rust::Vec<rust::String>());
    }

    inline static std::unique_ptr<Compilation> Compilation_new_with_top_modules(
        rust::Vec<rust::String> topModules) {
        return std::make_unique<Compilation>(std::move(topModules));
    }

    inline static rust::Vec<rust::String> Compilation_system_function_names() {
      slang::ast::Compilation compilation;
      rust::Vec<rust::String> names;
      auto systemNames = compilation.getSystemFunctionNames();
      names.reserve(systemNames.size());
      for (const auto& name : systemNames)
        names.emplace_back(name.data(), name.size());
      return names;
    }

    inline static rust::Vec<rust::String> Compilation_system_task_names() {
      slang::ast::Compilation compilation;
      rust::Vec<rust::String> names;
      auto systemNames = compilation.getSystemTaskNames();
      names.reserve(systemNames.size());
      for (const auto& name : systemNames)
        names.emplace_back(name.data(), name.size());
      return names;
    }

    inline static void Compilation_add_syntax_tree(Compilation& compilation, std::shared_ptr<syntax::SyntaxTree> tree) {
        compilation.addSyntaxTree(std::move(tree));
    }

    ::RawSyntaxTreeBufferIds Compilation_add_syntax_tree_from_text(
        Compilation& compilation,
        std::string_view text,
        std::string_view name,
        std::string_view path,
        rust::Vec<rust::String> predefines,
        rust::Vec<rust::String> includePaths,
        rust::Vec<::RawSourceBuffer> includeBuffers,
        bool expandIncludes);

    ::RawSyntaxTreeBufferIds Compilation_add_library_map_syntax_tree_from_text(
        Compilation& compilation,
        std::string_view text,
        std::string_view name,
        std::string_view path);

    rust::Vec<::RawSyntaxDiagnostic> Compilation_semantic_diagnostics(const Compilation& compilation);
    rust::Vec<::RawSyntaxDiagnostic> Compilation_parse_diagnostics_with_options(
        const Compilation& compilation,
        rust::Vec<rust::String> warning_options);
    rust::Vec<::RawSyntaxDiagnostic> Compilation_semantic_diagnostics_with_options(
        const Compilation& compilation,
        rust::Vec<rust::String> warning_options);
  }
}
