#include "wrapper.h"

#include <string_view>

namespace slang_sys::syntax
{

std::shared_ptr<SyntaxTree> parse_syntax_tree(rust::Str text, rust::Str name, rust::Str path)
{
    auto source = std::string_view(text.data(), text.size());
    auto tree_name = std::string_view(name.data(), name.size());
    auto tree_path = std::string_view(path.data(), path.size());
    return SyntaxTree::fromText(source, tree_name, tree_path);
}

const SyntaxNode *syntax_tree_root(const SyntaxTree &tree)
{
    return &tree.root();
}

uint16_t syntax_node_kind(const SyntaxNode *node)
{
    return static_cast<uint16_t>(node->kind);
}

std::size_t syntax_node_child_count(const SyntaxNode *node)
{
    return node->getChildCount();
}

std::size_t syntax_node_list_child_count(const SyntaxNode *node)
{
    slang::SmallVector<slang::syntax::ListChildInfo, 2> info;
    // TODO: const_cast is a hack because slang upstream API doesn't have a const version of getChildListInfo
    //       Maybe we could add a const overload to the upstream.
    getChildListInfo(*const_cast<SyntaxNode *>(node), info);
    return info.size();
}

std::size_t syntax_node_list_child_size(const SyntaxNode *node, std::size_t index)
{
    slang::SmallVector<slang::syntax::ListChildInfo, 2> info;
    // TODO: const_cast is a hack because slang upstream API doesn't have a const version of getChildListInfo
    //       Maybe we could add a const overload to the upstream.
    getChildListInfo(*const_cast<SyntaxNode *>(node), info);
    return info[index].size;
}

const SyntaxNode *syntax_node_child_node(const SyntaxNode *node, std::size_t index)
{
    return node->childNode(index);
}

const SyntaxToken *syntax_node_child_token(const SyntaxNode *node, std::size_t index)
{
    // TODO: const_cast is a hack because slang upstream API doesn't have a const version of getChildListInfo
    //       Maybe we could add a const overload to the upstream.
    return const_cast<SyntaxNode *>(node)->childTokenPtr(index);
}

uint16_t syntax_token_kind(const SyntaxToken *token)
{
    return static_cast<uint16_t>(token->kind);
}

rust::String syntax_token_value_text(const SyntaxToken *token)
{
    auto text = token->valueText();
    return rust::String(text.data(), text.size());
}

} // namespace slang_sys::syntax
