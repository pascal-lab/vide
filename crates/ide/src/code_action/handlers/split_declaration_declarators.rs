use std::ops::Range;

use hir::base_db::source_db::SourceDb;
use itertools::Itertools;
use syntax::{
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};
use utils::text_edit::TextRange;

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, line_indent,
};

const ID: CodeActionId = CodeActionId {
    name: "split_declaration_declarators",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const LABEL: &str = "Split declaration";

pub(super) fn split_declaration_declarators(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    split_data_declaration(collector, ctx).or_else(|| split_net_declaration(collector, ctx))
}

fn split_data_declaration(collector: &mut CodeActionCollector, ctx: &CodeActionCtx) -> Option<()> {
    let decl = ctx.find_node_at_offset::<ast::DataDeclaration>()?;
    split_declaration(collector, ctx, decl.syntax(), decl.declarators().children())
}

fn split_net_declaration(collector: &mut CodeActionCollector, ctx: &CodeActionCtx) -> Option<()> {
    let decl = ctx.find_node_at_offset::<ast::NetDeclaration>()?;
    split_declaration(collector, ctx, decl.syntax(), decl.declarators().children())
}

fn split_declaration<'a>(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
    syntax: syntax::SyntaxNode<'a>,
    declarators: impl Iterator<Item = ast::Declarator<'a>>,
) -> Option<()> {
    let decl_range = syntax.text_range()?;
    let declarators = declarators.collect::<Vec<_>>();
    if declarators.len() < 2 {
        return None;
    }

    let text = ctx.sema().db.file_text(ctx.file_id());
    let first_range = declarators.first()?.syntax().text_range()?;

    let prefix_range = TextRange::new(decl_range.start(), first_range.start());
    let prefix = text.get(Range::from(prefix_range))?;

    let new_declarators = declarators
        .iter()
        .flat_map(|d| d.syntax().text_range())
        .map(|range| text.get(Range::from(range)))
        .collect::<Option<Vec<_>>>()?;

    collector.add(ID, LABEL, decl_range, |builder| {
        let indent = line_indent(&text, decl_range.start());
        let replacement = new_declarators
            .into_iter()
            .map(|declarator| format!("{prefix}{};", declarator.trim()))
            .join(&format!("\n{indent}"));
        builder.replace(decl_range, replacement);
    });

    Some(())
}
