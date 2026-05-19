use syntax::{SyntaxNodeExt, ast::AstNode, has_text_range::HasTextRange, token::TokenKindExt};

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, RepairKind,
};

const ID: CodeActionId = CodeActionId {
    name: "add_instance_parens",
    kind: CodeActionKind::Generate,
    repair: Some(RepairKind::AddInstanceParens),
};
const LABEL: &str = "Add empty instance port list";

pub(super) fn add_instance_parens(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    if !ctx.diagnostics.allows_repair(RepairKind::AddInstanceParens) {
        return None;
    }

    let token = ctx.compilation_unit()?.syntax().token_before_offset(ctx.range.end())?;
    if !token.kind().name_like() || token.text_range()?.end() != ctx.range.end() {
        return None;
    }

    let insert_offset = token.text_range()?.end();
    collector.add(ID, LABEL, ctx.range, |builder| {
        builder.insert(insert_offset, "()".to_owned());
    });

    Some(())
}
