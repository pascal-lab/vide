#include "wrapper.h"

#include <algorithm>
#include <filesystem>
#include <mutex>
#include <optional>
#include <span>
#include <string>
#include <string_view>
#include <unordered_map>

namespace slang_sys::syntax
{

namespace helper
{

// TODO: This map is just for backward compatibility. Once we update the slan-sys public API, we should remove this map
// and the associated functions.
static std::mutex syntax_tree_sessions_mutex;
static std::unordered_map<const SyntaxNode *, std::weak_ptr<SourceSession>> syntax_tree_sessions;

static void register_syntax_tree_session(const SyntaxTree &tree)
{
    if (!tree.tree || !tree.session)
        return;

    std::lock_guard lock(syntax_tree_sessions_mutex);
    syntax_tree_sessions.emplace(&tree.tree->root(), tree.session);
}

static void unregister_syntax_tree_session(const SyntaxTree &tree)
{
    if (!tree.tree)
        return;

    std::lock_guard lock(syntax_tree_sessions_mutex);
    syntax_tree_sessions.erase(&tree.tree->root());
}

static std::shared_ptr<SourceSession> source_session_for_context(const SyntaxNode *context)
{
    if (!context)
        return {};

    auto root = context;
    while (root && root->parent.get())
        root = root->parent.get();
    if (!root)
        return {};

    std::lock_guard lock(syntax_tree_sessions_mutex);
    auto it = syntax_tree_sessions.find(root);
    if (it == syntax_tree_sessions.end())
        return {};
    return it->second.lock();
}

static slang::SourceRange node_range(const SyntaxNode *node)
{
    return node->sourceRange();
}

static slang::SourceRange token_range(const SyntaxToken *token)
{
    return token->range();
}

static bool source_range_valid(slang::SourceRange range)
{
    return range.start().valid() && range.end().valid();
}

static const SyntaxNode *find_root(const SyntaxNode *node)
{
    while (node && node->parent.get())
        node = node->parent.get();
    return node;
}

static slang::SourceRange map_range_with_context(slang::SourceRange range, const SyntaxNode *context)
{
    if (!context || !source_range_valid(range))
        return slang::SourceRange::NoLocation;

    auto root = find_root(context);
    if (!root)
        return slang::SourceRange::NoLocation;

    auto root_range = root->sourceRange();
    if (!source_range_valid(root_range))
        return slang::SourceRange::NoLocation;

    auto session = source_session_for_context(root);
    if (!session)
        return slang::SourceRange::NoLocation;

    slang::DiagnosticEngine engine(session->source_manager);
    slang::SmallVector<slang::SourceRange> mapped;
    engine.mapSourceRanges(root_range.start(), std::span(&range, 1), mapped, false);
    if (mapped.empty())
        return slang::SourceRange::NoLocation;
    return mapped.front();
}

static slang::SourceRange node_range_with_context(const SyntaxNode *node, const SyntaxNode *context)
{
    return map_range_with_context(node_range(node), context);
}

static slang::SourceRange token_range_with_context(const SyntaxToken *token, const SyntaxNode *context)
{
    return map_range_with_context(token_range(token), context);
}

static std::optional<slang::SourceLocation> explicit_location(const SyntaxTrivia *trivia)
{
    return trivia->getExplicitLocation();
}

} // namespace helper

SourceSession::SourceSession()
{
    source_manager.setDisableProximatePaths(true);
}

SyntaxTree::SyntaxTree(std::shared_ptr<::slang::syntax::SyntaxTree> tree, std::shared_ptr<SourceSession> session)
    : tree(std::move(tree)), session(std::move(session))
{
    helper::register_syntax_tree_session(*this);
}

SyntaxTree::~SyntaxTree()
{
    helper::unregister_syntax_tree_session(*this);
}

const SyntaxNode &SyntaxTree::root() const
{
    return tree->root();
}

namespace tree
{

std::shared_ptr<SyntaxTree> parse_syntax_tree(rust::Str text, rust::Str name, rust::Str path)
{
    auto source_storage = std::string(text.data(), text.size());
    auto source = std::string_view(source_storage);
    auto tree_name = std::string_view(name.data(), name.size());
    // Use an unnamed buffer to avoid path cache collisions in the shared source manager.
    (void)path;
    auto session = std::make_shared<SourceSession>();
    auto tree = ::slang::syntax::SyntaxTree::fromText(source, session->source_manager, tree_name, std::string_view());
    return std::make_shared<SyntaxTree>(std::move(tree), std::move(session));
}

std::shared_ptr<SyntaxTree> parse_syntax_tree_with_options(rust::Str text, rust::Str name, rust::Str path,
                                                           rust::Vec<rust::String> predefines,
                                                           rust::Vec<rust::String> include_paths,
                                                           rust::Vec<rust::String> include_buffer_paths,
                                                           rust::Vec<rust::String> include_buffer_texts,
                                                           bool expand_includes)
{
    auto source_storage = std::string(text.data(), text.size());
    auto source = std::string_view(source_storage);
    auto tree_name = std::string_view(name.data(), name.size());
    auto tree_path = std::string_view(path.data(), path.size());

    auto session = std::make_shared<SourceSession>();

    slang::Bag options;
    auto &pp_options = options.insertOrGet<slang::parsing::PreprocessorOptions>();
    for (const auto &predefine : predefines)
        pp_options.predefines.emplace_back(std::string(predefine));
    for (const auto &include_path : include_paths)
        pp_options.additionalIncludePaths.emplace_back(std::filesystem::path(std::string(include_path)));
    if (!expand_includes)
        pp_options.maxIncludeDepth = 0;

    auto include_buffer_count = std::min(include_buffer_paths.size(), include_buffer_texts.size());
    for (std::size_t i = 0; i < include_buffer_count; i++)
    {
        session->source_manager.assignText(std::string(include_buffer_paths[i]), std::string(include_buffer_texts[i]));
    }

    auto tree =
        ::slang::syntax::SyntaxTree::fromFileInMemory(source, session->source_manager, tree_name, tree_path, options);
    return std::make_shared<SyntaxTree>(std::move(tree), std::move(session));
}

const SyntaxNode *syntax_tree_root(const SyntaxTree &tree)
{
    return &tree.root();
}

} // namespace tree

namespace node
{

uint16_t syntax_node_kind(const SyntaxNode *node)
{
    return static_cast<uint16_t>(node->kind);
}

bool syntax_node_range_valid(const SyntaxNode *node)
{
    return helper::source_range_valid(helper::node_range(node));
}

uint32_t syntax_node_range_start_buffer_id(const SyntaxNode *node)
{
    return helper::node_range(node).start().buffer().getId();
}

std::size_t syntax_node_range_start_offset(const SyntaxNode *node)
{
    return helper::node_range(node).start().offset();
}

uint32_t syntax_node_range_end_buffer_id(const SyntaxNode *node)
{
    return helper::node_range(node).end().buffer().getId();
}

std::size_t syntax_node_range_end_offset(const SyntaxNode *node)
{
    return helper::node_range(node).end().offset();
}

bool syntax_node_range_with_context_valid(const SyntaxNode *node, const SyntaxNode *context)
{
    return helper::source_range_valid(helper::node_range_with_context(node, context));
}

uint32_t syntax_node_range_with_context_start_buffer_id(const SyntaxNode *node, const SyntaxNode *context)
{
    return helper::node_range_with_context(node, context).start().buffer().getId();
}

std::size_t syntax_node_range_with_context_start_offset(const SyntaxNode *node, const SyntaxNode *context)
{
    return helper::node_range_with_context(node, context).start().offset();
}

uint32_t syntax_node_range_with_context_end_buffer_id(const SyntaxNode *node, const SyntaxNode *context)
{
    return helper::node_range_with_context(node, context).end().buffer().getId();
}

std::size_t syntax_node_range_with_context_end_offset(const SyntaxNode *node, const SyntaxNode *context)
{
    return helper::node_range_with_context(node, context).end().offset();
}

const SyntaxNode *syntax_node_parent(const SyntaxNode *node)
{
    return node->parent.get();
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
    // TODO: const_cast is a hack because slang upstream API doesn't have a const version of childTokenPtr
    //       Maybe we could add a const overload to the upstream.
    return const_cast<SyntaxNode *>(node)->childTokenPtr(index);
}

} // namespace node

namespace token
{

uint16_t syntax_token_kind(const SyntaxToken *token)
{
    return static_cast<uint16_t>(token->kind);
}

bool syntax_token_range_valid(const SyntaxToken *token)
{
    return helper::source_range_valid(helper::token_range(token));
}

uint32_t syntax_token_range_start_buffer_id(const SyntaxToken *token)
{
    return helper::token_range(token).start().buffer().getId();
}

std::size_t syntax_token_range_start_offset(const SyntaxToken *token)
{
    return helper::token_range(token).start().offset();
}

uint32_t syntax_token_range_end_buffer_id(const SyntaxToken *token)
{
    return helper::token_range(token).end().buffer().getId();
}

std::size_t syntax_token_range_end_offset(const SyntaxToken *token)
{
    return helper::token_range(token).end().offset();
}

bool syntax_token_range_with_context_valid(const SyntaxToken *token, const SyntaxNode *context)
{
    return helper::source_range_valid(helper::token_range_with_context(token, context));
}

uint32_t syntax_token_range_with_context_start_buffer_id(const SyntaxToken *token, const SyntaxNode *context)
{
    return helper::token_range_with_context(token, context).start().buffer().getId();
}

std::size_t syntax_token_range_with_context_start_offset(const SyntaxToken *token, const SyntaxNode *context)
{
    return helper::token_range_with_context(token, context).start().offset();
}

uint32_t syntax_token_range_with_context_end_buffer_id(const SyntaxToken *token, const SyntaxNode *context)
{
    return helper::token_range_with_context(token, context).end().buffer().getId();
}

std::size_t syntax_token_range_with_context_end_offset(const SyntaxToken *token, const SyntaxNode *context)
{
    return helper::token_range_with_context(token, context).end().offset();
}

rust::String syntax_token_value_text(const SyntaxToken *token)
{
    auto text = token->valueText();
    return rust::String(text.data(), text.size());
}

std::size_t syntax_token_trivia_count(const SyntaxToken *token)
{
    return token->trivia().size();
}

const SyntaxTrivia *syntax_token_trivia(const SyntaxToken *token, std::size_t index)
{
    auto trivia = token->trivia();
    if (index >= trivia.size())
        return nullptr;
    return &trivia[index];
}

} // namespace token

namespace trivia
{

uint8_t syntax_trivia_kind(const SyntaxTrivia *trivia)
{
    return static_cast<uint8_t>(trivia->kind);
}

rust::String syntax_trivia_raw_text(const SyntaxTrivia *trivia)
{
    auto text = trivia->getRawText();
    return rust::String(text.data(), text.size());
}

const SyntaxNode *syntax_trivia_syntax(const SyntaxTrivia *trivia)
{
    return trivia->syntax();
}

bool syntax_trivia_explicit_location_valid(const SyntaxTrivia *trivia)
{
    auto loc = helper::explicit_location(trivia);
    return loc.has_value() && loc->valid();
}

uint32_t syntax_trivia_explicit_location_buffer_id(const SyntaxTrivia *trivia)
{
    return helper::explicit_location(trivia)->buffer().getId();
}

std::size_t syntax_trivia_explicit_location_offset(const SyntaxTrivia *trivia)
{
    return helper::explicit_location(trivia)->offset();
}

} // namespace trivia

} // namespace slang_sys::syntax
