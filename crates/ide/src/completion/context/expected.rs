use syntax::{
    SyntaxAncestors, SyntaxNodeExt, SyntaxToken, SyntaxTokenWithParent,
    ast::{self, AstNode},
    diagnostics::SyntaxKeywordContext,
    has_text_range::{HasTextRange, HasTextRangeIn},
};
use utils::line_index::TextSize;

use super::{CompletionExpectation, ExpectedSyntax, caret::CaretSnapshot, util::in_parens};
use crate::completion::syntax_keywords;

trait AstParens<'a>: AstNode<'a> {
    fn open_paren(&self) -> Option<SyntaxToken<'a>>;
    fn close_paren(&self) -> Option<SyntaxToken<'a>>;
}

macro_rules! impl_ast_parens {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl<'a> AstParens<'a> for $ty {
                fn open_paren(&self) -> Option<SyntaxToken<'a>> {
                    <$ty>::open_paren(self)
                }

                fn close_paren(&self) -> Option<SyntaxToken<'a>> {
                    <$ty>::close_paren(self)
                }
            }
        )+
    };
}

impl_ast_parens!(
    ast::NamedParamAssignment<'a>,
    ast::NamedPortConnection<'a>,
    ast::ParameterValueAssignment<'a>,
);

pub(super) fn detect_local(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    punctuated_expectation(caret)
        .or_else(|| keyword_prefix_expectation(caret))
        .or_else(|| else_clause_expectation(caret))
}

fn expectation(syntax: ExpectedSyntax) -> CompletionExpectation {
    CompletionExpectation { syntax }
}

fn node_expectation(
    syntax: ExpectedSyntax,
    _node: syntax::SyntaxNode<'_>,
) -> CompletionExpectation {
    expectation(syntax)
}

fn token_expectation(syntax: ExpectedSyntax, _token: SyntaxToken<'_>) -> CompletionExpectation {
    expectation(syntax)
}

fn node_at_offset_in_parens<'a, N>(caret: &CaretSnapshot<'a>) -> Option<N>
where
    N: AstParens<'a>,
{
    let offset = caret.offset;
    let node = caret.root.find_node_at_offset::<N>(offset)?;
    in_parens(offset, node.open_paren(), node.close_paren(), node.syntax()).then_some(node)
}

fn punctuated_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    expectation_after_dot(caret)
        .or_else(|| expectation_after_scope_resolution(caret))
        .or_else(|| expectation_after_hash(caret))
        .or_else(|| expectation_after_at(caret))
        .or_else(|| expectation_in_named_conn_expr(caret))
        .or_else(|| expectation_in_param_value_assignment(caret))
        .or_else(|| sensitivity_list_expectation(caret))
}

fn expectation_after_dot(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let offset = caret.offset;

    if let Some(named) = caret.root.find_node_at_offset::<ast::NamedPortConnection<'_>>(offset)
        && named_connection_dot_zone_contains(
            offset,
            named.syntax(),
            named.dot(),
            named.name(),
            named.open_paren(),
        )
    {
        return Some(node_expectation(ExpectedSyntax::PortConnectionName, named.syntax()));
    }

    if let Some(named) = caret.root.find_node_at_offset::<ast::NamedParamAssignment<'_>>(offset)
        && named_connection_dot_zone_contains(
            offset,
            named.syntax(),
            named.dot(),
            named.name(),
            named.open_paren(),
        )
    {
        return Some(node_expectation(ExpectedSyntax::ParameterAssignmentName, named.syntax()));
    }

    let prev = caret.root.token_before_offset(offset)?;
    (prev.kind() == syntax::Token![.])
        .then_some(token_expectation(ExpectedSyntax::MemberName, prev.tok))
}

fn named_connection_dot_zone_contains(
    offset: TextSize,
    context: syntax::SyntaxNode<'_>,
    dot: Option<SyntaxToken<'_>>,
    name: Option<SyntaxToken<'_>>,
    open_paren: Option<SyntaxToken<'_>>,
) -> bool {
    let text_range = |token: SyntaxToken<'_>| token.text_range_in(context);
    let Some(dot_range) = dot.and_then(text_range) else {
        return false;
    };
    let zone_end = open_paren
        .and_then(text_range)
        .map(|range| range.start())
        .or_else(|| name.and_then(text_range).map(|range| range.end()))
        .unwrap_or_else(|| dot_range.end());

    offset >= dot_range.end() && offset <= zone_end
}

fn expectation_after_scope_resolution(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let offset = caret.offset;
    let prev = caret.root.token_before_offset(offset)?;
    if prev.kind() == syntax::Token![::] {
        return Some(token_expectation(ExpectedSyntax::MemberName, prev.tok));
    }

    let replacement_start = caret.replacement_and_prefix().0.start();
    let scoped = SyntaxAncestors::start_from(prev.parent).find_map(ast::ScopedName::cast)?;
    let right = scoped_right_token(scoped)?;
    let range = right.text_range()?;
    (range.contains(replacement_start) || range.contains(offset) || range.end() == offset)
        .then_some(node_expectation(ExpectedSyntax::MemberName, scoped.syntax()))
}

fn scoped_right_token(scoped: ast::ScopedName<'_>) -> Option<SyntaxTokenWithParent<'_>> {
    use ast::Name::*;
    match scoped.right() {
        IdentifierName(ident) => {
            Some(SyntaxTokenWithParent { parent: ident.syntax(), tok: ident.identifier()? })
        }
        IdentifierSelectName(ident) => {
            Some(SyntaxTokenWithParent { parent: ident.syntax(), tok: ident.identifier()? })
        }
        _ => None,
    }
}

fn expectation_after_hash(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let prev = caret.root.token_before_offset(caret.offset)?;
    if prev.kind() != syntax::Token![#] {
        return None;
    }

    let offset = caret.offset;

    if let Some(params) =
        caret.root.find_node_at_offset::<ast::ParameterValueAssignment<'_>>(offset)
        && params
            .hash()
            .and_then(|t| t.text_range_in(params.syntax()))
            .is_some_and(|r| r.end() == offset)
    {
        return Some(node_expectation(
            ExpectedSyntax::AfterParamValueAssignmentHash,
            params.syntax(),
        ));
    }

    if let Some(params) = caret.root.find_node_at_offset::<ast::ParameterPortList<'_>>(offset)
        && params
            .hash()
            .and_then(|t| t.text_range_in(params.syntax()))
            .is_some_and(|r| r.end() == offset)
    {
        return Some(node_expectation(ExpectedSyntax::AfterParameterPortListHash, params.syntax()));
    }

    None
}

fn expectation_after_at(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let prev = caret.root.token_before_offset(caret.offset)?;
    (prev.kind() == syntax::Token![@]).then_some(token_expectation(
        ExpectedSyntax::EventControl { wrap_in_parens: true },
        prev.tok,
    ))
}

fn expectation_in_named_conn_expr(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    if let Some(conn) = node_at_offset_in_parens::<ast::NamedPortConnection<'_>>(caret)
        && conn.name().is_some()
    {
        return Some(node_expectation(ExpectedSyntax::PortConnectionExpr, conn.syntax()));
    }

    if let Some(conn) = node_at_offset_in_parens::<ast::NamedParamAssignment<'_>>(caret)
        && conn.name().is_some()
    {
        return Some(node_expectation(ExpectedSyntax::ParameterAssignmentExpr, conn.syntax()));
    }

    None
}

fn expectation_in_param_value_assignment(
    caret: &CaretSnapshot<'_>,
) -> Option<CompletionExpectation> {
    let node = node_at_offset_in_parens::<ast::ParameterValueAssignment<'_>>(caret)?;
    Some(node_expectation(ExpectedSyntax::ParamValueAssignment, node.syntax()))
}

fn sensitivity_list_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let offset = caret.offset;

    if let Some(node) = caret.root.find_node_at_offset::<ast::EventControl<'_>>(offset) {
        return Some(node_expectation(
            ExpectedSyntax::EventControl { wrap_in_parens: false },
            node.syntax(),
        ));
    }
    if let Some(node) =
        caret.root.find_node_at_offset::<ast::EventControlWithExpression<'_>>(offset)
    {
        return Some(node_expectation(
            ExpectedSyntax::EventControl { wrap_in_parens: false },
            node.syntax(),
        ));
    }
    if let Some(node) = caret.root.find_node_at_offset::<ast::ImplicitEventControl<'_>>(offset) {
        return Some(node_expectation(
            ExpectedSyntax::EventControl { wrap_in_parens: false },
            node.syntax(),
        ));
    }
    if let Some(node) = caret.root.find_node_at_offset::<ast::RepeatedEventControl<'_>>(offset) {
        return Some(node_expectation(
            ExpectedSyntax::EventControl { wrap_in_parens: false },
            node.syntax(),
        ));
    }

    None
}

fn keyword_prefix_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    keyword_prefix_in_node::<ast::ParameterPortList<'_>>(
        caret,
        ExpectedSyntax::ParameterPortListItem,
        SyntaxKeywordContext::ParameterPortListItem,
    )
    .or_else(|| {
        keyword_prefix_in_node::<ast::AnsiPortList<'_>>(
            caret,
            ExpectedSyntax::AnsiPortItem,
            SyntaxKeywordContext::AnsiPortItem,
        )
    })
    .or_else(|| {
        keyword_prefix_in_node::<ast::FunctionPortList<'_>>(
            caret,
            ExpectedSyntax::FunctionPortItem,
            SyntaxKeywordContext::FunctionPortItem,
        )
    })
}

fn keyword_prefix_in_node<'a, N>(
    caret: &CaretSnapshot<'a>,
    syntax: ExpectedSyntax,
    context: SyntaxKeywordContext,
) -> Option<CompletionExpectation>
where
    N: AstNode<'a>,
{
    let (_, prefix) = caret.replacement_and_prefix();
    if prefix.is_empty()
        || syntax_keywords::keyword_candidates_for_context(context, &prefix).labels().is_empty()
    {
        return None;
    }

    let node = caret.root.find_node_at_offset::<N>(caret.offset)?;
    Some(node_expectation(syntax, node.syntax()))
}

fn else_clause_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let (replacement, prefix) = caret.replacement_and_prefix();
    if !prefix.is_empty() && !"else".starts_with(&prefix) {
        return None;
    }

    let replacement_start = replacement.start();
    let prev = caret.root.token_before_offset(replacement_start)?;
    let prev_range = prev.text_range()?;
    let conditional =
        SyntaxAncestors::start_from(prev.parent).find_map(ast::ConditionalStatement::cast)?;
    if conditional.else_clause().is_some() {
        return None;
    }

    let then_range = conditional.statement().syntax().text_range()?;
    (then_range.end() == prev_range.end() && replacement_start >= then_range.end())
        .then_some(node_expectation(ExpectedSyntax::ElseClause, conditional.syntax()))
}
