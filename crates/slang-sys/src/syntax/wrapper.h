#pragma once

#include <cstdint>
#include <memory>
#include <string>
#include <unordered_map>

#include "cxx.h"

#include "slang/parsing/Preprocessor.h"
#include "slang/parsing/LexerFacts.h"
#include "slang/parsing/Token.h"
#include "slang/ast/SemanticFacts.h"
#include "slang/syntax/SyntaxFacts.h"
#include "slang/syntax/SyntaxListInfo.h"
#include "slang/syntax/SyntaxNode.h"
#include "slang/syntax/SyntaxTree.h"
#include "slang/syntax/AllSyntax.h"
#include "slang/text/SourceManager.h"
#include "slang/util/Bag.h"

#include "../wrapper.h"

namespace slang_sys::syntax {

    struct RawTrace;
    struct RawTraceEmittedToken;
    struct RawSVInt;
    struct RawOptionalU32;
    struct RawExpectedSyntax;

    using SyntaxNode = ::slang::syntax::SyntaxNode;
    using SyntaxToken = ::slang::parsing::Token;
    using SyntaxTrivia = ::slang::parsing::Trivia;

    class SourceSession {
      public:
        slang::SourceManager source_manager;

        SourceSession();
        void assign_include_buffer(std::string path, std::string text);

      private:
        std::unordered_map<std::string, std::string> include_buffers;
    };

    // TODO: Maybe we should expose this data structure to the rust side, rather
    // than pretendint it as a SyntaxTree.
    class SyntaxTree {
      public:
        std::shared_ptr<::slang::syntax::SyntaxTree> tree;
        std::shared_ptr<SourceSession> session;
        uint32_t root_buffer_id;

        SyntaxTree(
            std::shared_ptr<::slang::syntax::SyntaxTree> tree,
            std::shared_ptr<SourceSession> session,
            uint32_t root_buffer_id
        );
        ~SyntaxTree();

        const SyntaxNode &root() const;
    };

    namespace tree {
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
        );
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
        );
        const SyntaxNode *syntax_tree_root(const SyntaxTree &tree);
        rust::Vec<uint32_t> syntax_tree_buffer_ids(const SyntaxTree &tree);
        uint32_t syntax_tree_root_buffer_id(const SyntaxTree &tree);
        rust::String syntax_tree_buffer_path(const SyntaxTree &tree, uint32_t buffer_id);
        rust::String syntax_tree_buffer_text(const SyntaxTree &tree, uint32_t buffer_id);
        uint8_t syntax_tree_buffer_origin(const SyntaxTree &tree, uint32_t buffer_id);
        rust::Vec<RawExpectedSyntax> syntax_tree_expected_syntax_at(
            const SyntaxTree &tree,
            std::size_t offset
        );
        std::shared_ptr<SyntaxTree> parse_library_map_syntax_tree(
            rust::Str text,
            rust::Str name,
            rust::Str path,
            bool collect_expected_syntax
        );
        std::shared_ptr<SyntaxTree> parse_library_map_syntax_tree_with_session(
            const std::shared_ptr<SourceSession>& session,
            rust::Str text,
            rust::Str name,
            rust::Str path,
            bool collect_expected_syntax
        );
        RawTrace syntax_tree_preprocessor_trace(const SyntaxTree &tree);
    } // namespace tree

    namespace node {
        uint16_t syntax_node_kind(const SyntaxNode *node);
        bool syntax_node_range_valid(const SyntaxNode *node);
        uint32_t syntax_node_range_start_buffer_id(const SyntaxNode *node);
        std::size_t syntax_node_range_start_offset(const SyntaxNode *node);
        uint32_t syntax_node_range_end_buffer_id(const SyntaxNode *node);
        std::size_t syntax_node_range_end_offset(const SyntaxNode *node);
        bool syntax_node_range_with_context_valid(
            const SyntaxNode *node,
            const SyntaxNode *context,
            const SyntaxTree &owner
        );
        uint32_t syntax_node_range_with_context_start_buffer_id(
            const SyntaxNode *node,
            const SyntaxNode *context,
            const SyntaxTree &owner
        );
        std::size_t syntax_node_range_with_context_start_offset(
            const SyntaxNode *node,
            const SyntaxNode *context,
            const SyntaxTree &owner
        );
        uint32_t syntax_node_range_with_context_end_buffer_id(
            const SyntaxNode *node,
            const SyntaxNode *context,
            const SyntaxTree &owner
        );
        std::size_t syntax_node_range_with_context_end_offset(
            const SyntaxNode *node,
            const SyntaxNode *context,
            const SyntaxTree &owner
        );
        const SyntaxNode *syntax_node_parent(const SyntaxNode *node);
        std::size_t syntax_node_child_count(const SyntaxNode *node);
        std::size_t syntax_node_list_child_count(SyntaxNode *node);
        std::size_t syntax_node_list_child_size(SyntaxNode *node, std::size_t index);
        const SyntaxNode *syntax_node_child_node(const SyntaxNode *node, std::size_t index);
        const SyntaxToken *syntax_node_child_token(SyntaxNode *node, std::size_t index);
    } // namespace node

    namespace token {
        uint16_t syntax_token_kind(const SyntaxToken *token);
        bool syntax_token_range_valid(const SyntaxToken *token);
        uint32_t syntax_token_range_start_buffer_id(const SyntaxToken *token);
        std::size_t syntax_token_range_start_offset(const SyntaxToken *token);
        uint32_t syntax_token_range_end_buffer_id(const SyntaxToken *token);
        std::size_t syntax_token_range_end_offset(const SyntaxToken *token);
        bool syntax_token_range_with_context_valid(
            const SyntaxToken *token,
            const SyntaxNode *context,
            const SyntaxTree &owner
        );
        uint32_t syntax_token_range_with_context_start_buffer_id(
            const SyntaxToken *token,
            const SyntaxNode *context,
            const SyntaxTree &owner
        );
        std::size_t syntax_token_range_with_context_start_offset(
            const SyntaxToken *token,
            const SyntaxNode *context,
            const SyntaxTree &owner
        );
        uint32_t syntax_token_range_with_context_end_buffer_id(
            const SyntaxToken *token,
            const SyntaxNode *context,
            const SyntaxTree &owner
        );
        std::size_t syntax_token_range_with_context_end_offset(
            const SyntaxToken *token,
            const SyntaxNode *context,
            const SyntaxTree &owner
        );
        rust::String syntax_token_value_text(const SyntaxToken *token);
        rust::String syntax_token_raw_text(const SyntaxToken *token);
        RawSVInt syntax_token_int_value(const SyntaxToken *token);
        double syntax_token_real_value(const SyntaxToken *token);
        uint8_t syntax_token_bit_value(const SyntaxToken *token);
        uint8_t syntax_token_literal_base(const SyntaxToken *token);
        uint8_t syntax_token_time_unit(const SyntaxToken *token);
        RawOptionalU32 syntax_token_preprocessor_trace_emitted_token_index(
            const SyntaxToken *token,
            const SyntaxNode *context,
            const SyntaxTree &owner
        );
        RawTraceEmittedToken syntax_token_preprocessor_trace_emitted_token(
            const SyntaxToken *token,
            const SyntaxNode *context,
            const SyntaxTree &owner
        );
        rust::Vec<rust::String> syntax_token_keyword_table_for_version(rust::Str version);
        uint16_t syntax_token_keyword_kind_for_version(rust::Str version, rust::Str text);
        uint16_t syntax_token_directive_kind(rust::Str text);
        rust::String syntax_token_directive_text(uint16_t kind);
        std::size_t syntax_token_trivia_count(const SyntaxToken *token);
        const SyntaxTrivia *syntax_token_trivia(const SyntaxToken *token, std::size_t index);
    } // namespace token

    namespace facts {
        bool is_possible_statement(uint16_t kind);
        bool is_possible_expression(uint16_t kind);
        bool is_possible_data_type(uint16_t kind);
        bool is_possible_argument(uint16_t kind);
        bool is_possible_param_assignment(uint16_t kind);
        bool is_possible_port_connection(uint16_t kind);
        bool is_possible_ansi_port(uint16_t kind);
        bool is_possible_non_ansi_port(uint16_t kind);
        bool is_possible_function_port(uint16_t kind);
        bool is_possible_parameter(uint16_t kind);
        bool is_gate_type(uint16_t kind);
        bool is_edge_kind(uint16_t kind);
        bool is_port_direction(uint16_t kind);
        bool is_net_type(uint16_t kind);
        uint16_t get_integer_type(uint16_t kind);
        uint16_t get_keyword_type(uint16_t kind);
        uint16_t get_procedural_block_kind(uint16_t kind);
        uint16_t get_module_declaration_kind(uint16_t kind);
        bool is_possible_member_kind(uint16_t token_kind, uint16_t member_kind);
        uint16_t get_block_item_declaration_kind(uint16_t kind);
        uint16_t get_library_map_member_kind(uint16_t kind);
        uint16_t get_specify_item_kind(uint16_t kind);
        uint16_t get_config_header_item_kind(uint16_t kind);
        uint16_t get_config_rule_kind(uint16_t kind);
        rust::Vec<rust::String> keyword_candidates_for_context(rust::Str version, uint8_t context);
        bool is_allowed_in_compilation_unit(uint16_t kind);
        bool is_allowed_in_generate(uint16_t kind);
        bool is_allowed_in_module(uint16_t kind);
        bool is_allowed_in_interface(uint16_t kind);
        bool is_allowed_in_program(uint16_t kind);
        bool is_allowed_in_package(uint16_t kind);
    } // namespace facts

    namespace trivia {
        uint8_t syntax_trivia_kind(const SyntaxTrivia *trivia);
        rust::String syntax_trivia_raw_text(const SyntaxTrivia *trivia);
        const SyntaxNode *syntax_trivia_syntax(const SyntaxTrivia *trivia);
        bool syntax_trivia_explicit_location_valid(const SyntaxTrivia *trivia);
        uint32_t syntax_trivia_explicit_location_buffer_id(const SyntaxTrivia *trivia);
        std::size_t syntax_trivia_explicit_location_offset(const SyntaxTrivia *trivia);
    } // namespace trivia

} // namespace slang_sys::syntax
