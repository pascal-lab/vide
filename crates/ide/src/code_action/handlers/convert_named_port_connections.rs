use syntax::{
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
};
use utils::text_edit::TextRange;

use crate::code_action::{CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind};

const EXPAND_ID: CodeActionId = CodeActionId {
    name: "expand_named_port_connection_shorthand",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const EXPAND_LABEL: &str = "Expand named port shorthand";

const COLLAPSE_ID: CodeActionId = CodeActionId {
    name: "collapse_named_port_connection_shorthand",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const COLLAPSE_LABEL: &str = "Collapse named port to shorthand";

pub(super) fn convert_named_port_connection_shorthand(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    expand_named_port_connection_shorthand(collector, ctx)
        .or(collapse_named_port_connection_shorthand(collector, ctx))
}

fn expand_named_port_connection_shorthand(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    ctx.find_node_at_offset::<ast::NamedPortConnection>()?;
    let instance = ctx.find_node_at_offset::<ast::HierarchicalInstance>()?;
    let conns = named_port_connections(instance)?;
    let edits = conns
        .iter()
        .filter(|conn| conn.open_paren().is_none())
        .map(|conn| {
            let name = conn.name()?;
            Some((name.text_range_in(conn.syntax())?.end(), name.value_text().to_string()))
        })
        .collect::<Option<Vec<_>>>()?;
    if edits.is_empty() {
        return None;
    }

    let target = instance.syntax().text_range()?;

    collector.add(EXPAND_ID, EXPAND_LABEL, target, |builder| {
        for (insert_offset, name) in edits {
            builder.insert(insert_offset, format!("({name})"));
        }
    })
}

fn collapse_named_port_connection_shorthand(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    ctx.find_node_at_offset::<ast::NamedPortConnection>()?;
    let instance = ctx.find_node_at_offset::<ast::HierarchicalInstance>()?;
    let conns = named_port_connections(instance)?;
    let edits = conns
        .iter()
        .filter_map(|conn| collapsible_named_port_connection_range(*conn))
        .collect::<Vec<_>>();
    if edits.is_empty() {
        return None;
    }

    let target = instance.syntax().text_range()?;
    collector.add(COLLAPSE_ID, COLLAPSE_LABEL, target, |builder| {
        for remove_range in edits {
            builder.delete(remove_range);
        }
    })
}

fn named_port_connections(
    instance: ast::HierarchicalInstance<'_>,
) -> Option<Vec<ast::NamedPortConnection<'_>>> {
    let conns = instance
        .connections()
        .children()
        .map(|conn| conn.as_named_port_connection())
        .collect::<Option<Vec<_>>>()?;
    (!conns.is_empty()).then_some(conns)
}

fn collapsible_named_port_connection_range(
    conn: ast::NamedPortConnection<'_>,
) -> Option<TextRange> {
    let conn_name = conn.name()?;
    let port_name = conn_name.value_text().to_string();

    let expr = conn.expr()?.as_simple_property_expr()?.expr().as_simple_sequence_expr()?.expr();

    use ast::{Expression, Name};
    let actual = match expr {
        Expression::Name(Name::IdentifierName(ident)) => ident.identifier()?,
        Expression::Name(Name::IdentifierSelectName(ident))
            if ident.selectors().children().next().is_none() =>
        {
            ident.identifier()?
        }
        _ => return None,
    };
    if actual.value_text().to_string() != port_name {
        return None;
    }

    Some(TextRange::new(
        conn_name.text_range_in(conn.syntax())?.end(),
        conn.close_paren()?.text_range_in(conn.syntax())?.end(),
    ))
}
