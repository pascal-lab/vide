use syntax::{
    SyntaxAncestors, SyntaxNode, SyntaxNodeExt, SyntaxToken,
    ast::{self, AstNode},
    ast_ext::NamedConnectionDotZoneExt,
    has_text_range::HasTextRange,
};
use utils::line_index::{TextRange, TextSize};

use super::{
    CompletionExpectation, ExpectationSource, ExpectedSyntax, caret::CaretSnapshot, util::in_parens,
};
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

pub(super) fn detect_completion_expectation(
    caret: &CaretSnapshot<'_>,
    source_text: Option<&str>,
    replacement: TextRange,
) -> Option<CompletionExpectation> {
    punctuated_expectation(caret)
        .or_else(|| sensitivity_list_expectation(caret))
        .or_else(|| module_header_expectation(caret))
        .or_else(|| statement_keyword_expectation(caret))
        .or_else(|| expression_expectation(caret))
        .or_else(|| procedural_item_expectation(caret))
        .or_else(|| {
            source_text.and_then(|source_text| {
                token_prediction_item_expectation(caret, source_text, replacement)
            })
        })
        .or_else(|| generate_item_expectation(caret))
        .or_else(|| specify_item_expectation(caret))
        .or_else(|| module_item_expectation(caret))
        .or_else(|| config_item_expectation(caret))
        .or_else(|| compilation_unit_item_expectation(caret))
}

fn expectation(syntax: ExpectedSyntax, source: ExpectationSource) -> CompletionExpectation {
    CompletionExpectation { syntax, source }
}

fn node_expectation(syntax: ExpectedSyntax, node: SyntaxNode<'_>) -> CompletionExpectation {
    expectation(syntax, ExpectationSource::Ast(node.kind()))
}

fn token_expectation(syntax: ExpectedSyntax, token: SyntaxToken<'_>) -> CompletionExpectation {
    expectation(syntax, ExpectationSource::Token(token.kind()))
}

fn node_at_offset_in_parens<'a, N>(caret: &CaretSnapshot<'a>) -> Option<N>
where
    N: AstParens<'a>,
{
    let offset = caret.offset;
    let node = caret.root.find_node_at_offset::<N>(offset)?;
    in_parens(offset, node.open_paren(), node.close_paren()).then_some(node)
}

fn punctuated_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    expectation_after_dot(caret)
        .or_else(|| expectation_after_hash(caret))
        .or_else(|| expectation_after_at(caret))
        .or_else(|| expectation_in_named_conn_expr(caret))
        .or_else(|| expectation_in_paren_list(caret))
        .or_else(|| expectation_in_port_list(caret))
}

fn expectation_after_dot(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let offset = caret.offset;

    if let Some(named) = caret.root.find_node_at_offset::<ast::NamedPortConnection<'_>>(offset)
        && named.dot_name_zone_contains(offset)
    {
        return Some(node_expectation(ExpectedSyntax::PortConnectionName, named.syntax()));
    }

    if let Some(named) = caret.root.find_node_at_offset::<ast::NamedParamAssignment<'_>>(offset)
        && named.dot_name_zone_contains(offset)
    {
        return Some(node_expectation(ExpectedSyntax::ParameterAssignmentName, named.syntax()));
    }

    let prev = caret.root.token_before_offset(offset)?;
    (prev.kind() == syntax::Token![.])
        .then_some(token_expectation(ExpectedSyntax::MemberName, *prev))
}

fn expectation_after_hash(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let prev = caret.root.token_before_offset(caret.offset)?;
    if prev.kind() != syntax::Token![#] {
        return None;
    }

    let offset = caret.offset;

    if let Some(params) =
        caret.root.find_node_at_offset::<ast::ParameterValueAssignment<'_>>(offset)
        && params.hash().and_then(|t| t.text_range()).is_some_and(|r| r.end() == offset)
    {
        return Some(node_expectation(
            ExpectedSyntax::AfterParamValueAssignmentHash,
            params.syntax(),
        ));
    }

    if let Some(params) = caret.root.find_node_at_offset::<ast::ParameterPortList<'_>>(offset)
        && params.hash().and_then(|t| t.text_range()).is_some_and(|r| r.end() == offset)
    {
        return Some(node_expectation(ExpectedSyntax::AfterParameterPortListHash, params.syntax()));
    }

    None
}

fn expectation_after_at(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let prev = caret.root.token_before_offset(caret.offset)?;
    (prev.kind() == syntax::Token![@])
        .then_some(token_expectation(ExpectedSyntax::EventControl { wrap_in_parens: true }, *prev))
}

fn expectation_in_paren_list(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    if let Some(node) = node_at_offset_in_parens::<ast::ParameterValueAssignment<'_>>(caret) {
        return Some(node_expectation(ExpectedSyntax::ParamValueAssignment, node.syntax()));
    }

    if let Some(node) = node_at_offset_in_parens::<ast::ParameterPortList<'_>>(caret) {
        return Some(node_expectation(ExpectedSyntax::ParameterPortListItem, node.syntax()));
    }

    if let Some(node) = node_at_offset_in_parens::<ast::HierarchicalInstance<'_>>(caret) {
        return Some(node_expectation(ExpectedSyntax::PortConnection, node.syntax()));
    }

    if let Some(node) = node_at_offset_in_parens::<ast::ArgumentList<'_>>(caret) {
        return Some(node_expectation(ExpectedSyntax::ArgumentExpr, node.syntax()));
    }

    None
}

fn expectation_in_port_list(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    if let Some(node) = node_at_offset_in_parens::<ast::AnsiPortList<'_>>(caret) {
        return Some(node_expectation(ExpectedSyntax::AnsiPortItem, node.syntax()));
    }

    if let Some(node) = node_at_offset_in_parens::<ast::NonAnsiPortList<'_>>(caret) {
        return Some(node_expectation(ExpectedSyntax::NonAnsiPortName, node.syntax()));
    }

    if let Some(node) = node_at_offset_in_parens::<ast::FunctionPortList<'_>>(caret) {
        return Some(node_expectation(ExpectedSyntax::FunctionPortItem, node.syntax()));
    }

    None
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

fn module_header_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let offset = caret.offset;
    let module = caret.root.find_node_at_offset::<ast::ModuleDeclaration<'_>>(offset)?;
    let header = module.header();
    header
        .syntax()
        .text_range()
        .is_some_and(|r| r.contains(offset) || r.end() == offset)
        .then_some(node_expectation(ExpectedSyntax::ModuleHeaderItem, header.syntax()))
}

fn statement_keyword_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let stmt = caret.root.find_node_at_offset::<ast::Statement<'_>>(caret.offset)?;
    let stmt_range = stmt.syntax().text_range()?;

    let (replacement, prefix) = caret.replacement_and_prefix();
    if prefix.is_empty()
        || !(stmt_range.contains(replacement.start()) || stmt_range.start() == replacement.start())
    {
        return None;
    }

    stmt.syntax().token_before_offset(replacement.start()).is_none().then(|| {
        procedural_item_expectation(caret)
            .unwrap_or_else(|| node_expectation(ExpectedSyntax::Statement, stmt.syntax()))
    })
}

fn procedural_item_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let offset = caret.offset;

    if let Some(block) = caret.root.find_node_at_offset::<ast::BlockStatement<'_>>(offset)
        && let Some(zone) = item_zone(block.begin(), block.end(), block.syntax())
        && range_touches(zone, offset)
    {
        return Some(node_expectation(
            ExpectedSyntax::BlockItem {
                declarations_allowed: block_declarations_allowed_before(block, offset),
            },
            block.syntax(),
        ));
    }

    if let Some(func) = caret.root.find_node_at_offset::<ast::FunctionDeclaration<'_>>(offset)
        && let Some(zone) = item_zone(func.semi(), func.end(), func.syntax())
        && range_touches(zone, offset)
    {
        return Some(node_expectation(
            ExpectedSyntax::BlockItem {
                declarations_allowed: function_declarations_allowed_before(func, offset),
            },
            func.syntax(),
        ));
    }

    None
}

fn token_prediction_item_expectation(
    caret: &CaretSnapshot<'_>,
    source_text: &str,
    replacement: TextRange,
) -> Option<CompletionExpectation> {
    let prefix =
        source_text.get(usize::from(replacement.start())..usize::from(caret.offset)).unwrap_or("");
    [
        ExpectedSyntax::ConfigItem { rules_allowed: false },
        ExpectedSyntax::ConfigItem { rules_allowed: true },
        ExpectedSyntax::SpecifyItem,
        ExpectedSyntax::GenerateItem,
    ]
    .into_iter()
    .find(|expected| {
        syntax_keywords::predicts_source_expected_keyword(
            *expected,
            source_text,
            replacement,
            prefix,
        )
    })
    .map(|expected| expectation(expected, ExpectationSource::ParserRecovery))
}

fn generate_item_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let offset = caret.offset;

    if let Some(region) = caret.root.find_node_at_offset::<ast::GenerateRegion<'_>>(offset)
        && let Some(zone) = item_zone(region.keyword(), region.endgenerate(), region.syntax())
        && range_touches(zone, offset)
    {
        return Some(node_expectation(ExpectedSyntax::GenerateItem, region.syntax()));
    }

    if let Some(block) = caret.root.find_node_at_offset::<ast::GenerateBlock<'_>>(offset)
        && let Some(zone) = item_zone(block.begin(), block.end(), block.syntax())
        && range_touches(zone, offset)
    {
        return Some(node_expectation(ExpectedSyntax::GenerateItem, block.syntax()));
    }

    None
}

fn specify_item_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let offset = caret.offset;
    let block = caret.root.find_node_at_offset::<ast::SpecifyBlock<'_>>(offset)?;
    let zone = item_zone(block.specify(), block.endspecify(), block.syntax())?;
    range_touches(zone, offset)
        .then_some(node_expectation(ExpectedSyntax::SpecifyItem, block.syntax()))
}

fn module_item_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let offset = caret.offset;
    let module = caret.root.find_node_at_offset::<ast::ModuleDeclaration<'_>>(offset)?;
    let header = module.header();
    let start = header.semi().and_then(|semi| semi.text_range()).map(|r| r.end())?;
    let end = module
        .endmodule()
        .and_then(|tok| tok.text_range())
        .map(|range| range.start())
        .or_else(|| module.syntax().text_range().map(|range| range.end()))?;

    range_touches(TextRange::new(start, end), offset)
        .then_some(node_expectation(ExpectedSyntax::ModuleItem, module.syntax()))
}

fn config_item_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let offset = caret.offset;
    let config = caret.root.find_node_at_offset::<ast::ConfigDeclaration<'_>>(offset)?;
    let start = config.semi_1().and_then(|semi| semi.text_range()).map(|range| range.end())?;
    let end = config
        .endconfig()
        .and_then(|tok| tok.text_range())
        .map(|range| range.start())
        .or_else(|| config.syntax().text_range().map(|range| range.end()))?;

    if !range_touches(TextRange::new(start, end), offset) {
        return None;
    }

    let rules_allowed = config
        .semi_2()
        .and_then(|semi| semi.text_range())
        .is_some_and(|range| range.end() <= offset);
    Some(node_expectation(ExpectedSyntax::ConfigItem { rules_allowed }, config.syntax()))
}

fn compilation_unit_item_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let offset = caret.offset;
    let unit = caret.root.find_node_at_offset::<ast::CompilationUnit<'_>>(offset)?;
    let end = unit
        .end_of_file()
        .and_then(|tok| tok.text_range())
        .map(|range| range.start())
        .or_else(|| unit.syntax().text_range().map(|range| range.end()))?;

    let range = TextRange::new(TextSize::new(0), end);
    range_touches(range, offset)
        .then_some(node_expectation(ExpectedSyntax::CompilationUnitItem, unit.syntax()))
}

fn expression_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let elem = caret.root.covering_element(utils::line_index::TextRange::empty(caret.offset));
    let node = elem.as_node().or_else(|| elem.parent())?;

    let expr_node =
        SyntaxAncestors::start_from(node).find(|n| ast::Expression::can_cast(n.kind()))?;

    let in_expr_context = SyntaxAncestors::start_from(expr_node)
        .skip(1)
        .any(|n| ast::Statement::can_cast(n.kind()) || ast::ContinuousAssign::can_cast(n.kind()))
        || SyntaxAncestors::start_from(expr_node)
            .skip(1)
            .any(|n| ast::EqualsValueClause::can_cast(n.kind()));

    in_expr_context.then_some(node_expectation(ExpectedSyntax::Expression, expr_node))
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
