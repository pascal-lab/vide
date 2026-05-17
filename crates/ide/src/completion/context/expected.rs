use syntax::{
    SyntaxAncestors, SyntaxKeywordContext, SyntaxNode, SyntaxNodeExt, SyntaxToken,
    SyntaxTokenWithParent,
    ast::{self, AstNode},
    ast_ext::NamedConnectionDotZoneExt,
    has_text_range::{HasTextRange, HasTextRangeIn},
};
use utils::line_index::{TextRange, TextSize};

use super::{
    CompletionExpectation, ExpectationSource, ExpectedSyntax, caret::CaretSnapshot, util::in_parens,
};

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
) -> Option<CompletionExpectation> {
    ExpectationEngine { caret }.detect()
}

struct ExpectationEngine<'a, 'tree> {
    caret: &'a CaretSnapshot<'tree>,
}

impl ExpectationEngine<'_, '_> {
    fn detect(&self) -> Option<CompletionExpectation> {
        self.structural_expectation()
            .or_else(|| self.item_expectation())
            .or_else(|| self.statement_keyword_expectation())
            .or_else(|| self.expression_expectation())
            .or_else(|| self.procedural_item_expectation())
    }

    fn structural_expectation(&self) -> Option<CompletionExpectation> {
        punctuated_expectation(self.caret).or_else(|| sensitivity_list_expectation(self.caret))
    }

    fn item_expectation(&self) -> Option<CompletionExpectation> {
        module_header_expectation(self.caret)
            .or_else(|| generate_item_expectation(self.caret))
            .or_else(|| specify_item_expectation(self.caret))
            .or_else(|| config_item_expectation(self.caret))
            .or_else(|| module_item_expectation(self.caret))
            .or_else(|| compilation_unit_item_expectation(self.caret))
            .or_else(|| library_map_item_expectation(self.caret))
    }

    fn statement_keyword_expectation(&self) -> Option<CompletionExpectation> {
        statement_keyword_expectation(self.caret)
    }

    fn expression_expectation(&self) -> Option<CompletionExpectation> {
        expression_expectation(self.caret)
    }

    fn procedural_item_expectation(&self) -> Option<CompletionExpectation> {
        procedural_item_expectation(self.caret)
    }
}

fn expectation(syntax: ExpectedSyntax, source: ExpectationSource) -> CompletionExpectation {
    CompletionExpectation { syntax, source }
}

fn keyword_expectation(
    context: SyntaxKeywordContext,
    source: ExpectationSource,
) -> CompletionExpectation {
    expectation(ExpectedSyntax::Keyword(context), source)
}

fn node_keyword_expectation(
    context: SyntaxKeywordContext,
    node: SyntaxNode<'_>,
) -> CompletionExpectation {
    keyword_expectation(context, ExpectationSource::Ast(node.kind()))
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
    in_parens(offset, node.open_paren(), node.close_paren(), node.syntax()).then_some(node)
}

fn punctuated_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    expectation_after_dot(caret)
        .or_else(|| expectation_after_scope_resolution(caret))
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

fn expectation_after_scope_resolution(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let offset = caret.offset;
    let prev = caret.root.token_before_offset(offset)?;
    if prev.kind() == syntax::Token![::] {
        return Some(token_expectation(ExpectedSyntax::MemberName, *prev));
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
    header.syntax().text_range().is_some_and(|r| r.contains(offset) || r.end() == offset).then_some(
        node_keyword_expectation(SyntaxKeywordContext::ModuleHeaderItem, header.syntax()),
    )
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
        procedural_item_expectation(caret).unwrap_or_else(|| {
            node_keyword_expectation(SyntaxKeywordContext::Statement, stmt.syntax())
        })
    })
}

fn procedural_item_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let offset = caret.offset;

    if let Some(block) = caret.root.find_node_at_offset::<ast::BlockStatement<'_>>(offset)
        && let Some(zone) = item_zone(block.begin(), block.end(), block.syntax())
        && range_touches(zone, offset)
    {
        let context = if block_declarations_allowed_before(block, offset) {
            SyntaxKeywordContext::BlockItem
        } else {
            SyntaxKeywordContext::Statement
        };
        return Some(node_keyword_expectation(context, block.syntax()));
    }

    if let Some(func) = caret.root.find_node_at_offset::<ast::FunctionDeclaration<'_>>(offset)
        && let Some(zone) = item_zone(func.semi(), func.end(), func.syntax())
        && range_touches(zone, offset)
    {
        let context = if function_declarations_allowed_before(func, offset) {
            SyntaxKeywordContext::BlockItem
        } else {
            SyntaxKeywordContext::Statement
        };
        return Some(node_keyword_expectation(context, func.syntax()));
    }

    None
}

fn generate_item_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let offset = caret.offset;

    if let Some(region) = caret.root.find_node_at_offset::<ast::GenerateRegion<'_>>(offset)
        && let Some(zone) = item_zone(region.keyword(), region.endgenerate(), region.syntax())
        && item_start_in_owner(region.syntax(), zone, caret)
    {
        return Some(node_keyword_expectation(
            SyntaxKeywordContext::GenerateMember,
            region.syntax(),
        ));
    }

    if let Some(block) = caret.root.find_node_at_offset::<ast::GenerateBlock<'_>>(offset)
        && let Some(zone) = item_zone(block.begin(), block.end(), block.syntax())
        && item_start_in_owner(block.syntax(), zone, caret)
    {
        return Some(node_keyword_expectation(
            SyntaxKeywordContext::GenerateMember,
            block.syntax(),
        ));
    }

    None
}

fn specify_item_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let offset = caret.offset;
    let block = caret.root.find_node_at_offset::<ast::SpecifyBlock<'_>>(offset)?;
    let zone = item_zone(block.specify(), block.endspecify(), block.syntax())?;
    item_start_in_owner(block.syntax(), zone, caret)
        .then_some(node_keyword_expectation(SyntaxKeywordContext::SpecifyItem, block.syntax()))
}

fn module_item_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let offset = caret.offset;
    let module = caret.root.find_node_at_offset::<ast::ModuleDeclaration<'_>>(offset)?;
    let header = module.header();
    let start =
        header.semi().and_then(|semi| semi.text_range_in(header.syntax())).map(|r| r.end())?;
    let end = module
        .endmodule()
        .and_then(|tok| tok.text_range_in(module.syntax()))
        .map(|range| range.start())
        .or_else(|| module.syntax().text_range().map(|range| range.end()))?;

    let zone = TextRange::new(start, end);
    item_start_in_owner(module.syntax(), zone, caret)
        .then_some(node_keyword_expectation(SyntaxKeywordContext::ModuleMember, module.syntax()))
}

fn config_item_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let offset = caret.offset;
    let config = caret.root.find_node_at_offset::<ast::ConfigDeclaration<'_>>(offset)?;
    let start = config
        .semi_1()
        .and_then(|semi| semi.text_range_in(config.syntax()))
        .map(|range| range.end())?;
    let end = config.syntax().text_range().map(|range| range.end())?;

    if !range_touches(TextRange::new(start, end), offset) {
        return None;
    }

    let replacement_start = caret.replacement_and_prefix().0.start();
    let rules_allowed = config
        .semi_2()
        .and_then(|semi| semi.text_range_in(config.syntax()))
        .is_some_and(|range| range.end() <= replacement_start);
    Some(node_keyword_expectation(
        if rules_allowed {
            SyntaxKeywordContext::ConfigRule
        } else {
            SyntaxKeywordContext::ConfigHeaderItem
        },
        config.syntax(),
    ))
}

fn compilation_unit_item_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    let offset = caret.offset;
    let unit = caret.root.find_node_at_offset::<ast::CompilationUnit<'_>>(offset)?;
    let end = unit
        .end_of_file()
        .and_then(|tok| tok.text_range_in(unit.syntax()))
        .map(|range| range.start())
        .or_else(|| unit.syntax().text_range().map(|range| range.end()))?;

    let range = TextRange::new(TextSize::new(0), end);
    item_start_in_owner(unit.syntax(), range, caret).then_some(node_keyword_expectation(
        SyntaxKeywordContext::CompilationUnitMember,
        unit.syntax(),
    ))
}

fn library_map_item_expectation(caret: &CaretSnapshot<'_>) -> Option<CompletionExpectation> {
    (caret.root.kind() == syntax::SyntaxKind::LIBRARY_MAP).then_some(())?;

    let end = caret.root.text_range()?.end();
    let range = TextRange::new(TextSize::new(0), end);
    item_start_in_owner(caret.root, range, caret)
        .then_some(node_keyword_expectation(SyntaxKeywordContext::LibraryMapMember, caret.root))
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
    let start = open?.text_range_in(owner)?.end();
    let end = close
        .and_then(|tok| tok.text_range_in(owner).map(|range| range.start()))
        .or_else(|| owner.text_range().map(|range| range.end()))?;
    Some(TextRange::new(start, end))
}

fn range_touches(range: TextRange, offset: TextSize) -> bool {
    range.contains(offset) || range.end() == offset
}

fn item_start_in_owner(owner: SyntaxNode<'_>, zone: TextRange, caret: &CaretSnapshot<'_>) -> bool {
    let (replacement, _) = caret.replacement_and_prefix();
    let replacement_start = replacement.start();
    if !range_touches(zone, replacement_start) && !range_touches(zone, caret.offset) {
        return false;
    }

    let Some(item) = direct_list_item_at(owner, zone, replacement_start, caret.offset) else {
        return true;
    };
    item.text_range().is_some_and(|range| range.start() == replacement_start || range.is_empty())
}

fn direct_list_item_at(
    owner: SyntaxNode<'_>,
    zone: TextRange,
    replacement_start: TextSize,
    offset: TextSize,
) -> Option<SyntaxNode<'_>> {
    owner
        .children()
        .filter_map(|elem| elem.as_node())
        .filter(|node| node.kind().is_list())
        .filter(|node| node.text_range().is_none_or(|range| ranges_overlap(range, zone)))
        .flat_map(|list| list.children().filter_map(|elem| elem.as_node()))
        .find(|node| {
            node.text_range().is_some_and(|range| {
                range_touches(range, replacement_start) || range_touches(range, offset)
            })
        })
}

fn ranges_overlap(left: TextRange, right: TextRange) -> bool {
    left.start() <= right.end() && right.start() <= left.end()
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
