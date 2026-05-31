use hir::base_db::source_db::SourceDb;
use syntax::{
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, line_indent, newline_style,
    text_at,
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
    let prefix_range = utils::text_edit::TextRange::new(decl_range.start(), first_range.start());
    let prefix = text_at(&text, prefix_range)?;
    let indent = line_indent(&text, decl_range.start());
    let newline = newline_style(&text);
    let mut lines = Vec::new();
    for declarator in declarators {
        let declarator_text = text_at(&text, declarator.syntax().text_range()?)?;
        lines.push(format!("{prefix}{};", declarator_text.trim()));
    }
    let replacement = lines.join(&format!("{newline}{indent}"));

    collector.add(ID, LABEL, decl_range, |builder| {
        builder.replace(decl_range, replacement);
    });

    Some(())
}
