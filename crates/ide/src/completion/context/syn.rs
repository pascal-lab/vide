use syntax::{
    SyntaxAncestors, SyntaxNodeExt,
    ast::{self},
    has_text_range::HasTextRange,
};

use super::{
    AfterDot, AfterHash, AtKind, DotKind, HashKind, InParenList, InPortList, ParenListKind,
    PortListKind, Qualifier, SynContext, TriggerChar, caret::CaretSnapshot, util::in_parens,
};

pub(super) fn detect_syn_context(
    caret: &CaretSnapshot<'_>,
    trigger: Option<TriggerChar>,
) -> (SynContext, Option<Qualifier>) {
    let base = base_syn_context(caret);

    if trigger == Some(TriggerChar::At) {
        let qualifier = Qualifier::AfterAt(AtKind::EventControl);
        return (syn_context_for_qualifier(base, qualifier), Some(qualifier));
    }

    if trigger == Some(TriggerChar::Backtick) {
        let qualifier = Qualifier::AfterBacktick;
        return (syn_context_for_qualifier(base, qualifier), Some(qualifier));
    }

    let qualifier = qualifier_after_dot(caret)
        .or_else(|| qualifier_after_hash(caret))
        .or_else(|| qualifier_in_named_conn_expr(caret))
        .or_else(|| qualifier_in_paren_list(caret))
        .or_else(|| qualifier_in_port_list(caret));

    if let Some(qualifier) = qualifier {
        return (syn_context_for_qualifier(base, qualifier), Some(qualifier));
    }

    if is_in_sensitivity_list(caret) {
        return (SynContext::SensitivityList, None);
    }

    (base, None)
}

fn base_syn_context(caret: &CaretSnapshot<'_>) -> SynContext {
    let Some(node) = caret.covering_node() else {
        return SynContext::TopLevel;
    };

    if SyntaxAncestors::start_from(node).any(|n| n.kind() == syntax::SyntaxKind::MODULE_HEADER) {
        return SynContext::ModuleHeader;
    }

    if SyntaxAncestors::start_from(node).any(|n| n.kind() == syntax::SyntaxKind::MODULE_DECLARATION)
    {
        return SynContext::ModuleItem;
    }

    SynContext::TopLevel
}

fn qualifier_after_dot(caret: &CaretSnapshot<'_>) -> Option<Qualifier> {
    let offset = caret.offset;

    if let Some(named) = caret.root.find_node_at_offset::<ast::NamedPortConnection<'_>>(offset) {
        let dot = named.dot()?;
        let dot_range = dot.text_range()?;
        let zone_end = named
            .open_paren()
            .and_then(|t| t.text_range())
            .map(|r| r.start())
            .or_else(|| named.name().and_then(|t| t.text_range()).map(|r| r.end()))
            .unwrap_or_else(|| dot_range.end());
        if offset >= dot_range.end() && offset <= zone_end {
            return Some(Qualifier::AfterDot(AfterDot { kind: DotKind::NamedPort }));
        }
    }

    if let Some(named) = caret.root.find_node_at_offset::<ast::NamedParamAssignment<'_>>(offset) {
        let dot = named.dot()?;
        let dot_range = dot.text_range()?;
        let zone_end = named
            .open_paren()
            .and_then(|t| t.text_range())
            .map(|r| r.start())
            .or_else(|| named.name().and_then(|t| t.text_range()).map(|r| r.end()))
            .unwrap_or_else(|| dot_range.end());
        if offset >= dot_range.end() && offset <= zone_end {
            return Some(Qualifier::AfterDot(AfterDot { kind: DotKind::NamedParam }));
        }
    }

    let prev = caret.root.token_before_offset(offset)?;
    (prev.kind() == syntax::Token![.])
        .then_some(Qualifier::AfterDot(AfterDot { kind: DotKind::Member }))
}

fn qualifier_after_hash(caret: &CaretSnapshot<'_>) -> Option<Qualifier> {
    let prev = caret.root.token_before_offset(caret.offset)?;
    if prev.kind() != syntax::Token![#] {
        return None;
    }

    let offset = caret.offset;

    if let Some(params) =
        caret.root.find_node_at_offset::<ast::ParameterValueAssignment<'_>>(offset)
        && params.hash().and_then(|t| t.text_range()).is_some_and(|r| r.end() == offset)
    {
        return Some(Qualifier::AfterHash(AfterHash { kind: HashKind::ParamValueAssignment }));
    }

    if let Some(params) = caret.root.find_node_at_offset::<ast::ParameterPortList<'_>>(offset)
        && params.hash().and_then(|t| t.text_range()).is_some_and(|r| r.end() == offset)
    {
        return Some(Qualifier::AfterHash(AfterHash { kind: HashKind::ParameterPortList }));
    }

    None
}

fn qualifier_in_paren_list(caret: &CaretSnapshot<'_>) -> Option<Qualifier> {
    let offset = caret.offset;

    if let Some(list) = caret.root.find_node_at_offset::<ast::ParameterValueAssignment<'_>>(offset)
        && in_parens(offset, list.open_paren(), list.close_paren())
    {
        return Some(Qualifier::InParenList(InParenList {
            kind: ParenListKind::ParamValueAssignment,
        }));
    }

    if let Some(list) = caret.root.find_node_at_offset::<ast::ParameterPortList<'_>>(offset)
        && in_parens(offset, list.open_paren(), list.close_paren())
    {
        return Some(Qualifier::InParenList(InParenList {
            kind: ParenListKind::ParameterPortList,
        }));
    }

    if let Some(list) = caret.root.find_node_at_offset::<ast::HierarchicalInstance<'_>>(offset)
        && in_parens(offset, list.open_paren(), list.close_paren())
    {
        return Some(Qualifier::InParenList(InParenList { kind: ParenListKind::PortConnections }));
    }

    if let Some(list) = caret.root.find_node_at_offset::<ast::ArgumentList<'_>>(offset)
        && in_parens(offset, list.open_paren(), list.close_paren())
    {
        return Some(Qualifier::InParenList(InParenList { kind: ParenListKind::Arguments }));
    }

    None
}

fn qualifier_in_port_list(caret: &CaretSnapshot<'_>) -> Option<Qualifier> {
    let offset = caret.offset;

    if let Some(list) = caret.root.find_node_at_offset::<ast::AnsiPortList<'_>>(offset)
        && in_parens(offset, list.open_paren(), list.close_paren())
    {
        return Some(Qualifier::InPortList(InPortList { kind: PortListKind::Ansi }));
    }

    if let Some(list) = caret.root.find_node_at_offset::<ast::NonAnsiPortList<'_>>(offset)
        && in_parens(offset, list.open_paren(), list.close_paren())
    {
        return Some(Qualifier::InPortList(InPortList { kind: PortListKind::NonAnsi }));
    }

    if let Some(list) = caret.root.find_node_at_offset::<ast::FunctionPortList<'_>>(offset)
        && in_parens(offset, list.open_paren(), list.close_paren())
    {
        return Some(Qualifier::InPortList(InPortList { kind: PortListKind::Ansi }));
    }

    None
}

fn qualifier_in_named_conn_expr(caret: &CaretSnapshot<'_>) -> Option<Qualifier> {
    let offset = caret.offset;

    if let Some(conn) = caret.root.find_node_at_offset::<ast::NamedPortConnection<'_>>(offset)
        && conn.name().is_some()
        && in_parens(offset, conn.open_paren(), conn.close_paren())
    {
        return Some(Qualifier::InNamedPortConnExpr);
    }

    if let Some(conn) = caret.root.find_node_at_offset::<ast::NamedParamAssignment<'_>>(offset)
        && conn.name().is_some()
        && in_parens(offset, conn.open_paren(), conn.close_paren())
    {
        return Some(Qualifier::InNamedParamAssignExpr);
    }

    None
}

fn is_in_sensitivity_list(caret: &CaretSnapshot<'_>) -> bool {
    let Some(node) = caret.covering_node() else {
        return false;
    };

    SyntaxAncestors::start_from(node).any(|n| {
        matches!(
            n.kind(),
            syntax::SyntaxKind::EVENT_CONTROL
                | syntax::SyntaxKind::EVENT_CONTROL_WITH_EXPRESSION
                | syntax::SyntaxKind::IMPLICIT_EVENT_CONTROL
                | syntax::SyntaxKind::REPEATED_EVENT_CONTROL
        )
    })
}

fn syn_context_for_qualifier(base: SynContext, qualifier: Qualifier) -> SynContext {
    match qualifier {
        Qualifier::AfterDot(AfterDot { kind: DotKind::NamedPort | DotKind::NamedParam }) => {
            SynContext::Instantiation
        }
        Qualifier::AfterDot(AfterDot { kind: DotKind::Member }) => SynContext::HierRef,
        Qualifier::AfterHash(AfterHash { kind: HashKind::ParamValueAssignment }) => {
            SynContext::Instantiation
        }
        Qualifier::AfterHash(AfterHash { kind: HashKind::ParameterPortList }) => {
            SynContext::ModuleHeader
        }
        Qualifier::InParenList(InParenList {
            kind: ParenListKind::ParamValueAssignment | ParenListKind::PortConnections,
        }) => SynContext::Instantiation,
        Qualifier::InParenList(InParenList { kind: ParenListKind::ParameterPortList }) => {
            SynContext::ModuleHeader
        }
        Qualifier::InParenList(InParenList { kind: ParenListKind::Arguments }) => {
            SynContext::ModuleItem
        }
        Qualifier::InPortList(_) => SynContext::ModuleHeader,
        Qualifier::AfterAt(AtKind::EventControl) => SynContext::SensitivityList,
        Qualifier::AfterBacktick => base,
        Qualifier::InNamedPortConnExpr | Qualifier::InNamedParamAssignExpr => {
            SynContext::Instantiation
        }
    }
}
