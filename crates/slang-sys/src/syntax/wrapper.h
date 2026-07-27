#pragma once

#include <cstdint>
#include <memory>

#include "cxx.h"

#include "slang/parsing/Token.h"
#include "slang/syntax/SyntaxListInfo.h"
#include "slang/syntax/SyntaxNode.h"
#include "slang/syntax/SyntaxTree.h"

namespace slang_sys::syntax
{

using SyntaxTree = ::slang::syntax::SyntaxTree;
using SyntaxNode = ::slang::syntax::SyntaxNode;
using SyntaxToken = ::slang::parsing::Token;

std::shared_ptr<SyntaxTree> parse_syntax_tree(rust::Str text, rust::Str name, rust::Str path);
const SyntaxNode *syntax_tree_root(const SyntaxTree &tree);

uint16_t syntax_node_kind(const SyntaxNode *node);
std::size_t syntax_node_child_count(const SyntaxNode *node);
std::size_t syntax_node_list_child_count(const SyntaxNode *node);
std::size_t syntax_node_list_child_size(const SyntaxNode *node, std::size_t index);
const SyntaxNode *syntax_node_child_node(const SyntaxNode *node, std::size_t index);
const SyntaxToken *syntax_node_child_token(const SyntaxNode *node, std::size_t index);

uint16_t syntax_token_kind(const SyntaxToken *token);
rust::String syntax_token_value_text(const SyntaxToken *token);

} // namespace slang_sys::syntax
