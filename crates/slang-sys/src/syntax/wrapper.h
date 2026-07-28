#pragma once

#include <cstdint>
#include <memory>

#include "cxx.h"

#include "slang/diagnostics/DiagnosticEngine.h"
#include "slang/parsing/Preprocessor.h"
#include "slang/parsing/Token.h"
#include "slang/syntax/SyntaxListInfo.h"
#include "slang/syntax/SyntaxNode.h"
#include "slang/syntax/SyntaxTree.h"
#include "slang/text/SourceManager.h"
#include "slang/util/Bag.h"

namespace slang_sys::syntax
{

using SyntaxNode = ::slang::syntax::SyntaxNode;
using SyntaxToken = ::slang::parsing::Token;
using SyntaxTrivia = ::slang::parsing::Trivia;

struct SourceSession
{
    slang::SourceManager source_manager;

    SourceSession();
};

// TODO: Maybe we should expose this data structure to the rust side, rather than pretendint it as a SyntaxTree.
struct SyntaxTree
{
    std::shared_ptr<::slang::syntax::SyntaxTree> tree;
    std::shared_ptr<SourceSession> session;

    SyntaxTree(std::shared_ptr<::slang::syntax::SyntaxTree> tree, std::shared_ptr<SourceSession> session);
    ~SyntaxTree();

    const SyntaxNode &root() const;
};

namespace tree
{
std::shared_ptr<SyntaxTree> parse_syntax_tree(rust::Str text, rust::Str name, rust::Str path);
std::shared_ptr<SyntaxTree> parse_syntax_tree_with_options(rust::Str text, rust::Str name, rust::Str path,
                                                           rust::Vec<rust::String> predefines,
                                                           rust::Vec<rust::String> include_paths,
                                                           rust::Vec<rust::String> include_buffer_paths,
                                                           rust::Vec<rust::String> include_buffer_texts,
                                                           bool expand_includes);
const SyntaxNode *syntax_tree_root(const SyntaxTree &tree);
} // namespace tree

namespace node
{
uint16_t syntax_node_kind(const SyntaxNode *node);
bool syntax_node_range_valid(const SyntaxNode *node);
uint32_t syntax_node_range_start_buffer_id(const SyntaxNode *node);
std::size_t syntax_node_range_start_offset(const SyntaxNode *node);
uint32_t syntax_node_range_end_buffer_id(const SyntaxNode *node);
std::size_t syntax_node_range_end_offset(const SyntaxNode *node);
bool syntax_node_range_with_context_valid(const SyntaxNode *node, const SyntaxNode *context);
uint32_t syntax_node_range_with_context_start_buffer_id(const SyntaxNode *node, const SyntaxNode *context);
std::size_t syntax_node_range_with_context_start_offset(const SyntaxNode *node, const SyntaxNode *context);
uint32_t syntax_node_range_with_context_end_buffer_id(const SyntaxNode *node, const SyntaxNode *context);
std::size_t syntax_node_range_with_context_end_offset(const SyntaxNode *node, const SyntaxNode *context);
const SyntaxNode *syntax_node_parent(const SyntaxNode *node);
std::size_t syntax_node_child_count(const SyntaxNode *node);
std::size_t syntax_node_list_child_count(const SyntaxNode *node);
std::size_t syntax_node_list_child_size(const SyntaxNode *node, std::size_t index);
const SyntaxNode *syntax_node_child_node(const SyntaxNode *node, std::size_t index);
const SyntaxToken *syntax_node_child_token(const SyntaxNode *node, std::size_t index);
} // namespace node

namespace token
{
uint16_t syntax_token_kind(const SyntaxToken *token);
bool syntax_token_range_valid(const SyntaxToken *token);
uint32_t syntax_token_range_start_buffer_id(const SyntaxToken *token);
std::size_t syntax_token_range_start_offset(const SyntaxToken *token);
uint32_t syntax_token_range_end_buffer_id(const SyntaxToken *token);
std::size_t syntax_token_range_end_offset(const SyntaxToken *token);
bool syntax_token_range_with_context_valid(const SyntaxToken *token, const SyntaxNode *context);
uint32_t syntax_token_range_with_context_start_buffer_id(const SyntaxToken *token, const SyntaxNode *context);
std::size_t syntax_token_range_with_context_start_offset(const SyntaxToken *token, const SyntaxNode *context);
uint32_t syntax_token_range_with_context_end_buffer_id(const SyntaxToken *token, const SyntaxNode *context);
std::size_t syntax_token_range_with_context_end_offset(const SyntaxToken *token, const SyntaxNode *context);
rust::String syntax_token_value_text(const SyntaxToken *token);
std::size_t syntax_token_trivia_count(const SyntaxToken *token);
const SyntaxTrivia *syntax_token_trivia(const SyntaxToken *token, std::size_t index);
} // namespace token

namespace trivia
{
uint8_t syntax_trivia_kind(const SyntaxTrivia *trivia);
rust::String syntax_trivia_raw_text(const SyntaxTrivia *trivia);
const SyntaxNode *syntax_trivia_syntax(const SyntaxTrivia *trivia);
bool syntax_trivia_explicit_location_valid(const SyntaxTrivia *trivia);
uint32_t syntax_trivia_explicit_location_buffer_id(const SyntaxTrivia *trivia);
std::size_t syntax_trivia_explicit_location_offset(const SyntaxTrivia *trivia);
} // namespace trivia

} // namespace slang_sys::syntax
