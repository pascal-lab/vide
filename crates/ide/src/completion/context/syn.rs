use syntax::{
    SyntaxAncestors, SyntaxNode, SyntaxNodeExt, SyntaxToken,
    ast::{self, AstNode},
    ast_ext::NamedConnectionDotZoneExt,
    has_text_range::HasTextRange,
};
use utils::line_index::{TextRange, TextSize};

use super::{CompletionSite, caret::CaretSnapshot, util::in_parens};

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
    ast::AnsiPortList<'a>,
    ast::ArgumentList<'a>,
    ast::FunctionPortList<'a>,
    ast::HierarchicalInstance<'a>,
    ast::NamedParamAssignment<'a>,
    ast::NamedPortConnection<'a>,
    ast::NonAnsiPortList<'a>,
    ast::ParameterPortList<'a>,
    ast::ParameterValueAssignment<'a>,
);

fn node_at_offset_in_parens<'a, N>(caret: &CaretSnapshot<'a>) -> Option<N>
where
    N: AstParens<'a>,
{
    let offset = caret.offset;
    let node = caret.root.find_node_at_offset::<N>(offset)?;
    in_parens(offset, node.open_paren(), node.close_paren()).then_some(node)
}

pub(super) fn detect_completion_site(caret: &CaretSnapshot<'_>) -> CompletionSite {
    if let Some(site) = punctuated_site(caret) {
        return site;
    }

    if is_in_sensitivity_list(caret) {
        return CompletionSite::SensitivityList;
    }

    if is_in_module_header(caret) {
        return CompletionSite::ModuleHeader;
    }

    if let Some(site) = statement_keyword_site(caret) {
        return site;
    }

    if is_expression_site(caret) {
        return CompletionSite::Expr;
    }

    if let Some(site) = procedural_item_site(caret) {
        return site;
    }

    if is_in_module(caret) {
        return CompletionSite::ModuleMemberStart;
    }

    CompletionSite::TopLevelItemStart
}

fn is_in_module(caret: &CaretSnapshot<'_>) -> bool {
    let offset = caret.offset;
    caret
        .root
        .find_node_at_offset::<ast::ModuleDeclaration<'_>>(offset)
        .and_then(|module| module.syntax().text_range())
        .is_some_and(|range| range.contains(offset))
}

fn is_in_module_header(caret: &CaretSnapshot<'_>) -> bool {
    let offset = caret.offset;
    let Some(module) = caret.root.find_node_at_offset::<ast::ModuleDeclaration<'_>>(offset) else {
        return false;
    };

    module.header().syntax().text_range().is_some_and(|r| r.contains(offset) || r.end() == offset)
}

fn site_after_dot(caret: &CaretSnapshot<'_>) -> Option<CompletionSite> {
    let offset = caret.offset;

    if let Some(named) = caret.root.find_node_at_offset::<ast::NamedPortConnection<'_>>(offset)
        && named.dot_name_zone_contains(offset)
    {
        return Some(CompletionSite::NamedPortName);
    }

    if let Some(named) = caret.root.find_node_at_offset::<ast::NamedParamAssignment<'_>>(offset)
        && named.dot_name_zone_contains(offset)
    {
        return Some(CompletionSite::NamedParamName);
    }

    let prev = caret.root.token_before_offset(offset)?;
    (prev.kind() == syntax::Token![.]).then_some(CompletionSite::MemberAccess)
}

fn site_after_hash(caret: &CaretSnapshot<'_>) -> Option<CompletionSite> {
    let prev = caret.root.token_before_offset(caret.offset)?;
    if prev.kind() != syntax::Token![#] {
        return None;
    }

    let offset = caret.offset;

    if let Some(params) =
        caret.root.find_node_at_offset::<ast::ParameterValueAssignment<'_>>(offset)
        && params.hash().and_then(|t| t.text_range()).is_some_and(|r| r.end() == offset)
    {
        return Some(CompletionSite::AfterParamValueAssignmentHash);
    }

    if let Some(params) = caret.root.find_node_at_offset::<ast::ParameterPortList<'_>>(offset)
        && params.hash().and_then(|t| t.text_range()).is_some_and(|r| r.end() == offset)
    {
        return Some(CompletionSite::AfterParameterPortListHash);
    }

    None
}

fn site_after_at(caret: &CaretSnapshot<'_>) -> Option<CompletionSite> {
    let prev = caret.root.token_before_offset(caret.offset)?;
    (prev.kind() == syntax::Token![@]).then_some(CompletionSite::AfterAtEventControl)
}

fn site_in_paren_list(caret: &CaretSnapshot<'_>) -> Option<CompletionSite> {
    if node_at_offset_in_parens::<ast::ParameterValueAssignment<'_>>(caret).is_some() {
        return Some(CompletionSite::ParamValueAssignment);
    }

    if node_at_offset_in_parens::<ast::ParameterPortList<'_>>(caret).is_some() {
        return Some(CompletionSite::ParameterPortList);
    }

    if node_at_offset_in_parens::<ast::HierarchicalInstance<'_>>(caret).is_some() {
        return Some(CompletionSite::PortConnections);
    }

    if node_at_offset_in_parens::<ast::ArgumentList<'_>>(caret).is_some() {
        return Some(CompletionSite::Arguments);
    }

    None
}

fn site_in_port_list(caret: &CaretSnapshot<'_>) -> Option<CompletionSite> {
    if node_at_offset_in_parens::<ast::AnsiPortList<'_>>(caret).is_some() {
        return Some(CompletionSite::AnsiPortList);
    }

    if node_at_offset_in_parens::<ast::NonAnsiPortList<'_>>(caret).is_some() {
        return Some(CompletionSite::NonAnsiPortList);
    }

    if node_at_offset_in_parens::<ast::FunctionPortList<'_>>(caret).is_some() {
        return Some(CompletionSite::AnsiPortList);
    }

    None
}

fn site_in_named_conn_expr(caret: &CaretSnapshot<'_>) -> Option<CompletionSite> {
    if let Some(conn) = node_at_offset_in_parens::<ast::NamedPortConnection<'_>>(caret)
        && conn.name().is_some()
    {
        return Some(CompletionSite::NamedPortConnExpr);
    }

    if let Some(conn) = node_at_offset_in_parens::<ast::NamedParamAssignment<'_>>(caret)
        && conn.name().is_some()
    {
        return Some(CompletionSite::NamedParamAssignExpr);
    }

    None
}

fn punctuated_site(caret: &CaretSnapshot<'_>) -> Option<CompletionSite> {
    site_after_dot(caret)
        .or_else(|| site_after_hash(caret))
        .or_else(|| site_after_at(caret))
        .or_else(|| site_in_named_conn_expr(caret))
        .or_else(|| site_in_paren_list(caret))
        .or_else(|| site_in_port_list(caret))
}

fn is_in_sensitivity_list(caret: &CaretSnapshot<'_>) -> bool {
    let offset = caret.offset;

    caret.root.find_node_at_offset::<ast::EventControl<'_>>(offset).is_some()
        || caret.root.find_node_at_offset::<ast::EventControlWithExpression<'_>>(offset).is_some()
        || caret.root.find_node_at_offset::<ast::ImplicitEventControl<'_>>(offset).is_some()
        || caret.root.find_node_at_offset::<ast::RepeatedEventControl<'_>>(offset).is_some()
}

fn statement_keyword_site(caret: &CaretSnapshot<'_>) -> Option<CompletionSite> {
    let stmt = caret.root.find_node_at_offset::<ast::Statement<'_>>(caret.offset)?;
    let stmt_range = stmt.syntax().text_range()?;

    let (replacement, prefix) = caret.replacement_and_prefix();
    if prefix.is_empty()
        || !(stmt_range.contains(replacement.start()) || stmt_range.start() == replacement.start())
    {
        return None;
    }

    stmt.syntax()
        .token_before_offset(replacement.start())
        .is_none()
        .then(|| procedural_item_site(caret).unwrap_or(CompletionSite::ProceduralStatementStart))
}

fn procedural_item_site(caret: &CaretSnapshot<'_>) -> Option<CompletionSite> {
    let offset = caret.offset;

    if let Some(block) = caret.root.find_node_at_offset::<ast::BlockStatement<'_>>(offset)
        && let Some(zone) = item_zone(block.begin(), block.end(), block.syntax())
        && range_touches(zone, offset)
    {
        return Some(procedural_item_site_for_decls(block_declarations_allowed_before(
            block, offset,
        )));
    }

    if let Some(func) = caret.root.find_node_at_offset::<ast::FunctionDeclaration<'_>>(offset)
        && let Some(zone) = item_zone(func.semi(), func.end(), func.syntax())
        && range_touches(zone, offset)
    {
        return Some(procedural_item_site_for_decls(function_declarations_allowed_before(
            func, offset,
        )));
    }

    None
}

fn procedural_item_site_for_decls(declarations_allowed: bool) -> CompletionSite {
    if declarations_allowed {
        CompletionSite::BlockDeclStart
    } else {
        CompletionSite::ProceduralStatementStart
    }
}

fn item_zone(
    open: Option<SyntaxToken<'_>>,
    close: Option<SyntaxToken<'_>>,
    owner: SyntaxNode<'_>,
) -> Option<TextRange> {
    let start = open?.text_range()?.end();
    let end = close
        .and_then(|tok| tok.text_range().map(|range| range.start()))
        .or_else(|| owner.text_range().map(|range| range.end()))?;
    Some(TextRange::new(start, end))
}

fn range_touches(range: TextRange, offset: TextSize) -> bool {
    range.contains(offset) || range.end() == offset
}

fn block_declarations_allowed_before(block: ast::BlockStatement<'_>, offset: TextSize) -> bool {
    !block.items().children().any(|item| item_before_statement(item.syntax(), offset))
}

fn function_declarations_allowed_before(
    func: ast::FunctionDeclaration<'_>,
    offset: TextSize,
) -> bool {
    !func.items().children().any(|item| item_before_statement(item.syntax(), offset))
}

fn item_before_statement(item: SyntaxNode<'_>, offset: TextSize) -> bool {
    let Some(range) = item.text_range() else {
        return false;
    };
    range.end() <= offset && ast::Statement::can_cast(item.kind())
}

fn is_expression_site(caret: &CaretSnapshot<'_>) -> bool {
    let elem = caret.root.covering_element(utils::line_index::TextRange::empty(caret.offset));
    let Some(node) = elem.as_node().or_else(|| elem.parent()) else {
        return false;
    };

    let Some(expr_node) =
        SyntaxAncestors::start_from(node).find(|n| ast::Expression::can_cast(n.kind()))
    else {
        return false;
    };

    SyntaxAncestors::start_from(expr_node)
        .skip(1)
        .any(|n| ast::Statement::can_cast(n.kind()) || ast::ContinuousAssign::can_cast(n.kind()))
        || SyntaxAncestors::start_from(expr_node)
            .skip(1)
            .any(|n| ast::EqualsValueClause::can_cast(n.kind()))
}
