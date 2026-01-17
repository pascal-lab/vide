use syntax::{
    SyntaxAncestors, SyntaxNode, SyntaxNodeExt,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};
use utils::line_index::TextSize;

use super::{
    AfterDot, AfterHash, AtKind, DotKind, HashKind, InParenList, InPortList, ParenListKind,
    PortListKind, Qualifier, SynContext, TriggerChar, caret::CaretSnapshot, util::in_parens,
};

pub(super) fn detect_syn_context(
    caret: &CaretSnapshot<'_>,
    trigger: Option<TriggerChar>,
) -> (SynContext, Option<Qualifier>) {
    if trigger == Some(TriggerChar::At) {
        return (SynContext::SensitivityList, Some(Qualifier::AfterAt(AtKind::EventControl)));
    }

    if trigger == Some(TriggerChar::Backtick) {
        return (base_syn_context(caret), Some(Qualifier::AfterBacktick));
    }

    if let Some(qualifier) = qualifier_after_dot(caret) {
        let syn = match qualifier {
            Qualifier::AfterDot(AfterDot { kind: DotKind::NamedPort | DotKind::NamedParam }) => {
                SynContext::Instantiation
            }
            Qualifier::AfterDot(AfterDot { kind: DotKind::Member }) => SynContext::HierRef,
            _ => unreachable!(),
        };
        return (syn, Some(qualifier));
    }

    if let Some(qualifier) = qualifier_after_hash(caret) {
        let syn = match qualifier {
            Qualifier::AfterHash(AfterHash { kind: HashKind::ParamValueAssignment }) => {
                SynContext::Instantiation
            }
            Qualifier::AfterHash(AfterHash { kind: HashKind::ParameterPortList }) => {
                SynContext::ModuleHeader
            }
            _ => unreachable!(),
        };
        return (syn, Some(qualifier));
    }

    if let Some(qualifier) = qualifier_in_named_conn_expr(caret) {
        let syn = match qualifier {
            Qualifier::InNamedPortConnExpr | Qualifier::InNamedParamAssignExpr => {
                SynContext::Instantiation
            }
            _ => unreachable!(),
        };
        return (syn, Some(qualifier));
    }

    if let Some(qualifier) = qualifier_in_paren_list(caret) {
        let syn = match qualifier {
            Qualifier::InParenList(InParenList {
                kind: ParenListKind::ParamValueAssignment | ParenListKind::PortConnections,
            }) => SynContext::Instantiation,
            Qualifier::InParenList(InParenList { kind: ParenListKind::ParameterPortList }) => {
                SynContext::ModuleHeader
            }
            Qualifier::InParenList(InParenList { kind: ParenListKind::Arguments }) => {
                SynContext::ModuleItem
            }
            _ => unreachable!(),
        };
        return (syn, Some(qualifier));
    }

    if let Some(qualifier) = qualifier_in_port_list(caret) {
        return (SynContext::ModuleHeader, Some(qualifier));
    }

    if is_in_sensitivity_list(caret) {
        return (SynContext::SensitivityList, None);
    }

    if let Some(qualifier) = qualifier_from_trigger(caret, trigger) {
        let syn = match qualifier {
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
            _ => base_syn_context(caret),
        };
        return (syn, Some(qualifier));
    }

    (base_syn_context(caret), None)
}

fn qualifier_from_trigger(
    caret: &CaretSnapshot<'_>,
    trigger: Option<TriggerChar>,
) -> Option<Qualifier> {
    match trigger {
        Some(TriggerChar::OpenParen | TriggerChar::Comma) => {}
        _ => return None,
    }

    let prev = caret.root.token_before_offset(caret.offset)?;
    let node = prev.parent;
    qualifier_in_paren_list_from_node(node.clone(), caret.offset)
        .or_else(|| qualifier_in_port_list_from_node(node, caret.offset))
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
    let Some(node) = caret.covering_node() else {
        return None;
    };

    for anc in SyntaxAncestors::start_from(node) {
        if let Some(named) = ast::NamedPortConnection::cast(anc) {
            let Some(dot) = named.dot() else {
                continue;
            };

            let Some(dot_range) = dot.text_range() else {
                continue;
            };

            let zone_end = named
                .open_paren()
                .and_then(|t| t.text_range())
                .map(|r| r.start())
                .or_else(|| named.name().and_then(|t| t.text_range()).map(|r| r.end()))
                .unwrap_or_else(|| dot_range.end());

            if caret.offset >= dot_range.end() && caret.offset <= zone_end {
                return Some(Qualifier::AfterDot(AfterDot { kind: DotKind::NamedPort }));
            }
        }

        if let Some(named) = ast::NamedParamAssignment::cast(anc) {
            let Some(dot) = named.dot() else {
                continue;
            };

            let Some(dot_range) = dot.text_range() else {
                continue;
            };

            let zone_end = named
                .open_paren()
                .and_then(|t| t.text_range())
                .map(|r| r.start())
                .or_else(|| named.name().and_then(|t| t.text_range()).map(|r| r.end()))
                .unwrap_or_else(|| dot_range.end());

            if caret.offset >= dot_range.end() && caret.offset <= zone_end {
                return Some(Qualifier::AfterDot(AfterDot { kind: DotKind::NamedParam }));
            }
        }
    }

    let prev = caret.root.token_before_offset(caret.offset)?;
    (prev.kind() == syntax::Token![.])
        .then_some(Qualifier::AfterDot(AfterDot { kind: DotKind::Member }))
}

fn qualifier_after_hash(caret: &CaretSnapshot<'_>) -> Option<Qualifier> {
    let prev = caret.root.token_before_offset(caret.offset)?;
    if prev.kind() != syntax::Token![#] {
        return None;
    }

    let Some(node) = caret.covering_node() else {
        return None;
    };

    for anc in SyntaxAncestors::start_from(node) {
        if let Some(params) = ast::ParameterValueAssignment::cast(anc) {
            let Some(hash) = params.hash() else {
                continue;
            };
            let Some(hash_range) = hash.text_range() else {
                continue;
            };
            if hash_range.end() == caret.offset {
                return Some(Qualifier::AfterHash(AfterHash {
                    kind: HashKind::ParamValueAssignment,
                }));
            }
        }

        if let Some(params) = ast::ParameterPortList::cast(anc) {
            let Some(hash) = params.hash() else {
                continue;
            };
            let Some(hash_range) = hash.text_range() else {
                continue;
            };
            if hash_range.end() == caret.offset {
                return Some(Qualifier::AfterHash(AfterHash { kind: HashKind::ParameterPortList }));
            }
        }
    }

    None
}

fn qualifier_in_paren_list(caret: &CaretSnapshot<'_>) -> Option<Qualifier> {
    let Some(node) = caret.covering_node() else {
        return None;
    };
    qualifier_in_paren_list_from_node(node, caret.offset)
}

fn qualifier_in_paren_list_from_node(node: SyntaxNode<'_>, offset: TextSize) -> Option<Qualifier> {
    for anc in SyntaxAncestors::start_from(node) {
        if let Some(list) = ast::ParameterValueAssignment::cast(anc) {
            if in_parens(offset, list.open_paren(), list.close_paren()) {
                return Some(Qualifier::InParenList(InParenList {
                    kind: ParenListKind::ParamValueAssignment,
                }));
            }
        }

        if let Some(list) = ast::ParameterPortList::cast(anc) {
            if in_parens(offset, list.open_paren(), list.close_paren()) {
                return Some(Qualifier::InParenList(InParenList {
                    kind: ParenListKind::ParameterPortList,
                }));
            }
        }

        if let Some(list) = ast::HierarchicalInstance::cast(anc) {
            if in_parens(offset, list.open_paren(), list.close_paren()) {
                return Some(Qualifier::InParenList(InParenList {
                    kind: ParenListKind::PortConnections,
                }));
            }
        }

        if let Some(list) = ast::ArgumentList::cast(anc) {
            if in_parens(offset, list.open_paren(), list.close_paren()) {
                return Some(Qualifier::InParenList(InParenList {
                    kind: ParenListKind::Arguments,
                }));
            }
        }
    }

    None
}

fn qualifier_in_port_list(caret: &CaretSnapshot<'_>) -> Option<Qualifier> {
    let Some(node) = caret.covering_node() else {
        return None;
    };
    qualifier_in_port_list_from_node(node, caret.offset)
}

fn qualifier_in_port_list_from_node(node: SyntaxNode<'_>, offset: TextSize) -> Option<Qualifier> {
    for anc in SyntaxAncestors::start_from(node) {
        if let Some(list) = ast::AnsiPortList::cast(anc) {
            if in_parens(offset, list.open_paren(), list.close_paren()) {
                return Some(Qualifier::InPortList(InPortList { kind: PortListKind::Ansi }));
            }
        }

        if let Some(list) = ast::NonAnsiPortList::cast(anc) {
            if in_parens(offset, list.open_paren(), list.close_paren()) {
                return Some(Qualifier::InPortList(InPortList { kind: PortListKind::NonAnsi }));
            }
        }

        if let Some(list) = ast::FunctionPortList::cast(anc) {
            if in_parens(offset, list.open_paren(), list.close_paren()) {
                return Some(Qualifier::InPortList(InPortList { kind: PortListKind::Ansi }));
            }
        }
    }

    None
}

fn qualifier_in_named_conn_expr(caret: &CaretSnapshot<'_>) -> Option<Qualifier> {
    let Some(node) = caret.covering_node() else {
        return None;
    };

    for anc in SyntaxAncestors::start_from(node) {
        if let Some(conn) = ast::NamedPortConnection::cast(anc) {
            if conn.name().is_none() {
                continue;
            }
            if in_parens(caret.offset, conn.open_paren(), conn.close_paren()) {
                return Some(Qualifier::InNamedPortConnExpr);
            }
        }

        if let Some(conn) = ast::NamedParamAssignment::cast(anc) {
            if conn.name().is_none() {
                continue;
            }
            if in_parens(caret.offset, conn.open_paren(), conn.close_paren()) {
                return Some(Qualifier::InNamedParamAssignExpr);
            }
        }
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
